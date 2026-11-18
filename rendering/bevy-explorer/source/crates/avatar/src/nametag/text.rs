//! A nametag's text: laid out with the engine's text pipeline and drawn from a glyph atlas of
//! the nametags' own, as distance fields, so it stays sharp at any size on screen.

use bevy::{
    ecs::system::SystemParam,
    math::{Rect, Vec2},
    prelude::*,
    text::{CosmicFontSystem, LineBreak, LineHeight, TextPipeline},
};
use scene_runner::update_world::text_shape::{
    glyph_quads, layout_text, AtlasGlyphUse, AtlasKey, GlyphAtlas, GlyphAtlases, LaidOutText, Page,
    Pending, TextSpanData,
};

use super::{layout::LINE_HEIGHT, material::Quad, NametagAssets};

pub const ATLAS: AtlasKey = AtlasKey::Engine("nametag");
/// The size every label is laid out and rastered at, whatever size it is drawn: the atlas
/// holds each glyph once.
const RASTER_FONT_SIZE: f32 = 32.0;
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

#[derive(SystemParam)]
pub struct TextShaper<'w> {
    pipeline: ResMut<'w, TextPipeline>,
    fonts: Res<'w, Assets<Font>>,
    font_system: ResMut<'w, CosmicFontSystem>,
    atlases: ResMut<'w, GlyphAtlases>,
    images: ResMut<'w, Assets<Image>>,
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
        let scale = font_size / RASTER_FONT_SIZE;
        let font = TextFont {
            font: self.assets.font.clone(),
            font_size: RASTER_FONT_SIZE,
            line_height: LineHeight::RelativeToFont(LINE_HEIGHT),
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
                // a nametag has no line limit
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
        let images = &mut *self.images;
        self.atlases.get_or_create(ATLAS, images)
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
        let images = &mut *self.images;
        let atlas = self.atlases.get_or_create(ATLAS, images);
        let (sdf, bitmap) = glyph_quads(
            &shaped.text,
            atlas,
            images,
            &mut self.font_system,
            &mut budget.0,
            false,
        )?;
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
}

/// What keeps a tag's glyphs in the atlas when it is rebuilt.
pub fn glyph_use<'a>(texts: impl IntoIterator<Item = &'a Shaped>) -> AtlasGlyphUse {
    AtlasGlyphUse::new(ATLAS, texts.into_iter().map(Shaped::laid_out))
}
