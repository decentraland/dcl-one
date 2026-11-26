//! A nametag's text: laid out with the engine's text pipeline and drawn from a glyph atlas of
//! the nametags' own, as distance fields, so it stays sharp at any size on screen.
//!
//! [`GlyphShaper`] is the part any feature with a glyph atlas of its own shapes with: the
//! nametags through [`TextShaper`], the reactions for their emoji.

use bevy::{
    ecs::system::SystemParam,
    math::{Rect, Vec2},
    prelude::*,
    text::{CosmicFontSystem, LineBreak, LineHeight, TextPipeline},
};
use scene_runner::update_world::text_shape::{
    glyph_quads, layout_text, AtlasGlyphUse, AtlasKey, GlyphAtlas, GlyphAtlases, LaidOutText, Page,
    Pending, TextQuad, TextSpanData,
};

use super::{layout::LINE_HEIGHT, material::Quad, NametagAssets};

/// A glyph atlas of one feature's own, and how text is laid out for it.
pub struct TextAtlas {
    pub key: AtlasKey,
    /// The size every text is laid out and rastered at, whatever size it is drawn: the atlas
    /// holds each glyph once.
    pub raster_font_size: f32,
    /// A line's height, in font sizes.
    pub line_height: f32,
}

pub const ATLAS: TextAtlas = TextAtlas {
    key: AtlasKey::Engine("nametag"),
    raster_font_size: 32.0,
    line_height: LINE_HEIGHT,
};
/// Glyphs a frame may send to be rastered.
const MAX_RASTERS_PER_FRAME: usize = 64;

/// A run of one colour.
#[derive(Clone, Debug, PartialEq)]
pub struct Span {
    pub text: String,
    pub color: Color,
}

impl Span {
    pub fn new(text: impl Into<String>, color: Color) -> Self {
        Self {
            text: text.into(),
            color,
        }
    }
}

/// A laid out label.
#[derive(Clone)]
pub struct Shaped {
    text: LaidOutText,
    /// Tag units per raster pixel.
    scale: f32,
    /// In tag units.
    pub size: Vec2,
}

impl Shaped {
    pub fn laid_out(&self) -> &LaidOutText {
        &self.text
    }
}

/// Lays text out for a [`TextAtlas`] and reads its glyphs' quads from it.
#[derive(SystemParam)]
pub struct GlyphShaper<'w> {
    pipeline: ResMut<'w, TextPipeline>,
    fonts: Res<'w, Assets<Font>>,
    font_system: ResMut<'w, CosmicFontSystem>,
    atlases: ResMut<'w, GlyphAtlases>,
    images: ResMut<'w, Assets<Image>>,
}

impl GlyphShaper<'_> {
    /// Lay `spans` out in `font` on one line at `font_size`, or wrapped to stay within `wrap`
    /// wide. `None` when the text can't be laid out (yet).
    pub fn shape(
        &mut self,
        atlas: &TextAtlas,
        font: &Handle<Font>,
        entity: Entity,
        spans: &[Span],
        font_size: f32,
        wrap: Option<f32>,
    ) -> Option<Shaped> {
        let scale = font_size / atlas.raster_font_size;
        let font = TextFont {
            font: font.clone(),
            font_size: atlas.raster_font_size,
            line_height: LineHeight::RelativeToFont(atlas.line_height),
            ..Default::default()
        };
        let spans = spans
            .iter()
            .filter(|span| !span.text.is_empty())
            .map(|span| -> TextSpanData {
                (
                    TextSpan::new(span.text.as_str()),
                    font.clone(),
                    TextColor(span.color),
                    None,
                )
            })
            .collect::<Vec<_>>();
        if spans.is_empty() {
            return None;
        }

        let mut lay_out = |linebreak: LineBreak, width: Option<f32>| {
            layout_text(
                entity,
                &spans,
                JustifyText::Left,
                linebreak,
                width,
                // no line limit
                None,
                font_size,
                false,
                &mut self.pipeline,
                &self.fonts,
                &mut self.font_system,
            )
            .ok()
        };
        let mut text = lay_out(LineBreak::NoWrap, None)?;
        if let Some(wrap) = wrap.filter(|wrap| text.size.x * scale > *wrap) {
            text = lay_out(LineBreak::WordOrCharacter, Some(wrap / scale))?;
        }
        Some(Shaped {
            size: text.size * scale,
            text,
            scale,
        })
    }

    pub fn atlas(&mut self, atlas: &TextAtlas) -> &GlyphAtlas {
        let images = &mut *self.images;
        self.atlases.get_or_create(atlas.key.clone(), images)
    }

    /// The text's quads with their atlas rects, in raster pixels running down from its top
    /// left: those of the distance-field page, then those of the colour page. `Err` while
    /// glyphs are still being rastered: ask again next frame.
    pub fn glyphs(
        &mut self,
        atlas: &TextAtlas,
        shaped: &Shaped,
        budget: &mut GlyphBudget,
    ) -> Result<(Vec<TextQuad>, Vec<TextQuad>), Pending> {
        let images = &mut *self.images;
        let atlas = self.atlases.get_or_create(atlas.key.clone(), images);
        glyph_quads(
            &shaped.text,
            atlas,
            images,
            &mut self.font_system,
            &mut budget.0,
            false,
        )
    }
}

/// The nametags' text, in their font and their atlas.
#[derive(SystemParam)]
pub struct TextShaper<'w> {
    glyphs: GlyphShaper<'w>,
    assets: Res<'w, NametagAssets>,
}

impl TextShaper<'_> {
    /// Lay `spans` out on one line at `font_size`, or wrapped to stay within `wrap` wide.
    /// `None` when the text can't be laid out (yet).
    pub fn shape(
        &mut self,
        entity: Entity,
        spans: &[Span],
        font_size: f32,
        wrap: Option<f32>,
    ) -> Option<Shaped> {
        self.glyphs
            .shape(&ATLAS, &self.assets.font, entity, spans, font_size, wrap)
    }

    /// A single line that fits `width`, cut short with an ellipsis when the whole of `text`
    /// doesn't.
    pub fn shape_ellipsized(
        &mut self,
        entity: Entity,
        text: &str,
        color: Color,
        font_size: f32,
        width: f32,
    ) -> Option<Shaped> {
        let whole = self.shape(entity, &[Span::new(text, color)], font_size, None)?;
        if whole.size.x <= width {
            return Some(whole);
        }
        let ends = text
            .char_indices()
            .map(|(index, _)| index)
            .collect::<Vec<_>>();
        // the most characters that fit with the ellipsis
        let (mut fits, mut too_many) = (0, ends.len());
        let mut best = None;
        while fits + 1 < too_many {
            let count = (fits + too_many) / 2;
            let cut = format!("{}…", text[..ends[count]].trim_end());
            match self.shape(entity, &[Span::new(cut, color)], font_size, None) {
                Some(shaped) if shaped.size.x <= width => {
                    fits = count;
                    best = Some(shaped);
                }
                _ => too_many = count,
            }
        }
        best.or_else(|| self.shape(entity, &[Span::new("…", color)], font_size, None))
    }

    pub fn atlas(&mut self) -> &GlyphAtlas {
        self.glyphs.atlas(&ATLAS)
    }

    /// The label's quads, its top left at `origin`, kept to `clip`. `Err` while glyphs are
    /// still being rastered: ask again next frame.
    pub fn quads(
        &mut self,
        shaped: &Shaped,
        origin: Vec2,
        clip: Option<Rect>,
        budget: &mut GlyphBudget,
        quads: &mut Vec<Quad>,
    ) -> Result<(), Pending> {
        let (sdf, bitmap) = self.glyphs.glyphs(&ATLAS, shaped, budget)?;
        let scale = shaped.scale;
        for (glyphs, page) in [(sdf, Page::Sdf), (bitmap, Page::Color)] {
            quads.extend(glyphs.into_iter().filter_map(|glyph| {
                // text pixels run down from the block's top left
                let rect = Rect::new(
                    origin.x + glyph.min.x * scale,
                    origin.y - glyph.max.y * scale,
                    origin.x + glyph.max.x * scale,
                    origin.y - glyph.min.y * scale,
                );
                let quad = Quad::glyph(
                    rect,
                    glyph.uv_min,
                    glyph.uv_max,
                    page == Page::Color,
                    glyph.color,
                );
                match clip {
                    Some(clip) => quad.clipped(clip),
                    None => Some(quad),
                }
            }));
        }
        Ok(())
    }
}

/// The glyphs a frame may still send to be rastered.
pub struct GlyphBudget(usize);

impl GlyphBudget {
    pub fn frame() -> Self {
        Self(MAX_RASTERS_PER_FRAME)
    }

    pub fn of(rasters: usize) -> Self {
        Self(rasters)
    }
}

/// What keeps the glyphs of `texts` in the atlas when it is rebuilt.
pub fn glyph_use<'a>(
    atlas: &TextAtlas,
    texts: impl IntoIterator<Item = &'a Shaped>,
) -> AtlasGlyphUse {
    AtlasGlyphUse::new(atlas.key.clone(), texts.into_iter().map(Shaped::laid_out))
}

/// The atlas pages a material binds, distance fields then colour, and their sizes.
pub type BoundPages = [Option<(AssetId<Image>, u32)>; 2];

/// The atlas' textures are replaced when it is rebuilt, and resized as it grows. When they
/// are not the ones `bound`, `bound` takes them and they are returned for the material to
/// bind.
pub fn rebind(atlas: &GlyphAtlas, bound: &mut BoundPages) -> Option<[Option<Handle<Image>>; 2]> {
    let pages = [Page::Sdf, Page::Color].map(|page| atlas.page_image(page));
    let current = pages.map(|page| page.map(|(image, size)| (image.id(), size)));
    if *bound == current {
        return None;
    }
    *bound = current;
    Some(pages.map(|page| page.map(|(image, _)| image.clone())))
}
