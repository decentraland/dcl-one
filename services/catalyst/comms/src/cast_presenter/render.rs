//! PDF pages to video frames.
//!
//! The renderer is [hayro](https://github.com/LaurenzV/hayro) 0.9 (Apache-2.0
//! OR MIT), pure Rust: it needs no native library, so the Nix build of the
//! node takes it like any other crate. It bundles the PDFium Foxit standard
//! fonts (BSD-3-Clause) for unembedded fonts, a CC0 ICC profile, and its own
//! JPEG 2000, JBIG2 and CCITT decoders, the image kinds upstream's pdfjs
//! decodes with WebAssembly. The alternatives were refused: MuPDF is AGPL
//! and poppler GPL, and PDFium (`pdfium-render`) is a native library the
//! Nix build would have to provide.
//!
//! As upstream (`src/adapters/pdf-renderer/component.ts`,
//! `src/logic/render-size.ts`): a page fits 1920x1080 (1080x1920 when
//! portrait) with even sides of at least 2 px, on white; every link to a
//! video (YouTube, Drive, Docs files, Vimeo, or a `.gif`, `.mp4` or `.webm`
//! URL) is painted black with a play mark and listed with its box.

use std::sync::{Arc, LazyLock};

use hayro::hayro_interpret::InterpreterSettings;
use hayro::hayro_syntax::Pdf;
use hayro::hayro_syntax::object::dict::keys;
use hayro::hayro_syntax::object::{Array, Dict, Name, Rect};
use hayro::kurbo::Affine;
use hayro::vello_cpu::color::palette::css::WHITE;
use hayro::vello_cpu::{Pixmap, RasterizerSettings, RenderContext, Resources, TargetInit};
use hayro::{RenderCache, RenderSettings, render_into};
use regex::Regex;
use sha2::{Digest, Sha256};

use super::wire::{Geometry, SlideVideo};

const FULL_HD_LONG_SIDE: f64 = 1920.0;
const FULL_HD_SHORT_SIDE: f64 = 1080.0;
const MIN_SIDE: u32 = 2;

/// A slide's PNG under its content hash.
pub type Picture = (String, bytes::Bytes);

/// One rendered slide: straight RGBA, `width * height * 4` bytes.
#[derive(Clone, PartialEq, Eq)]
pub struct Frame {
    pub rgba: Arc<Vec<u8>>,
    pub width: u32,
    pub height: u32,
}

impl Frame {
    /// The frame as a PNG, and its name: the first 16 hex characters of the
    /// PNG's SHA-256 (upstream's `src/logic/slide-image.ts`).
    pub fn picture(&self) -> Result<Picture, String> {
        let png = tiny_skia::PixmapRef::from_bytes(&self.rgba, self.width, self.height)
            .ok_or("Invalid frame size")?
            .encode_png()
            .map_err(|e| e.to_string())?;
        Ok((hex::encode(&Sha256::digest(&png)[..8]), png.into()))
    }
}

impl std::fmt::Debug for Frame {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Frame({}x{})", self.width, self.height)
    }
}

/// JavaScript's `Math.round`: halves toward +infinity.
fn js_round(n: f64) -> f64 {
    (n + 0.5).floor()
}

/// `fitRenderSize`: the scale and the even render size, at least 2 px a
/// side, that fit the page inside 1920x1080, or 1080x1920 when portrait.
pub fn fit_render_size(page_width: f64, page_height: f64) -> Result<(f64, u32, u32), String> {
    let valid = |side: f64| side.is_finite() && side > 0.0;
    if !valid(page_width) || !valid(page_height) {
        return Err(format!("Invalid page size {page_width}x{page_height}"));
    }
    let (max_w, max_h) = if page_width >= page_height {
        (FULL_HD_LONG_SIDE, FULL_HD_SHORT_SIDE)
    } else {
        (FULL_HD_SHORT_SIDE, FULL_HD_LONG_SIDE)
    };
    let scale = (max_w / page_width).min(max_h / page_height);
    let even = |side: f64| ((js_round(side * scale) as u32) & !1).max(MIN_SIDE);
    Ok((scale, even(page_width), even(page_height)))
}

/// Whether a link points at a video (`isMediaUrl`).
pub fn is_media_url(url: &str) -> bool {
    static VIDEO: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"youtube\.com|youtu\.be|drive\.google\.com|docs\.google\.com/file|vimeo\.com")
            .unwrap()
    });
    static MEDIA: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"(?i)\.gif($|\?)|\.mp4($|\?)|\.webm($|\?)").unwrap());
    VIDEO.is_match(url) || MEDIA.is_match(url)
}

/// A link annotation's URL as pdfjs reports it: an absolute `http`, `https`,
/// `ftp`, `mailto` or `tel` URL, a bare `www.` host with two dots taken as
/// `http://`, normalized.
fn link_url(raw: &str) -> Option<String> {
    let dots = raw.matches('.').count();
    let candidate = if raw.starts_with("www.") && dots >= 2 {
        format!("http://{raw}")
    } else {
        raw.to_string()
    };
    let parsed = url::Url::parse(&candidate).ok()?;
    matches!(parsed.scheme(), "http" | "https" | "ftp" | "mailto" | "tel")
        .then(|| parsed.to_string())
}

/// A page's link annotations to videos: URL and PDF rectangle.
fn media_links(page: &Dict<'_>) -> Vec<(String, Rect)> {
    let Some(annots) = page.get::<Array<'_>>(keys::ANNOTS) else {
        return Vec::new();
    };
    annots
        .iter::<Dict<'_>>()
        .filter_map(|annot| {
            let subtype = annot.get::<Name<'_>>(keys::SUBTYPE)?;
            if subtype.as_ref() != b"Link" {
                return None;
            }
            let action = annot.get::<Dict<'_>>(keys::A)?;
            if action.get::<Name<'_>>(keys::S)?.as_ref() != b"URI" {
                return None;
            }
            let uri = action.get::<hayro::hayro_syntax::object::String<'_>>(keys::URI)?;
            let url = link_url(&String::from_utf8_lossy(uri.as_bytes()))?;
            let rect = annot.get::<Rect>(keys::RECT)?;
            is_media_url(&url).then_some((url, rect))
        })
        .collect()
}

/// `pdfRectToCanvas`: the link's box on the rendered page, in pixels.
fn rect_to_canvas(rect: &Rect, scale: f64, viewport_height: f64) -> Geometry {
    Geometry {
        x: js_round(rect.x0 * scale) as i64,
        y: js_round(viewport_height - rect.y1 * scale) as i64,
        width: js_round((rect.x1 - rect.x0) * scale) as i64,
        height: js_round((rect.y1 - rect.y0) * scale) as i64,
    }
}

/// A black box with a white ring and triangle over each video link.
fn paint_video_placeholders(rgba: &mut [u8], width: u32, height: u32, boxes: &[Geometry]) {
    use tiny_skia::{Color, FillRule, Paint, PathBuilder, PixmapMut, Stroke, Transform};
    let Some(mut canvas) = PixmapMut::from_bytes(rgba, width, height) else {
        return;
    };
    let mut black = Paint::default();
    black.set_color(Color::BLACK);
    let mut white = Paint::default();
    white.set_color(Color::from_rgba8(255, 255, 255, 230));
    white.anti_alias = true;
    for g in boxes {
        if let Some(rect) =
            tiny_skia::Rect::from_xywh(g.x as f32, g.y as f32, g.width as f32, g.height as f32)
        {
            canvas.fill_rect(rect, &black, Transform::identity(), None);
        }
        let cx = g.x as f32 + g.width as f32 / 2.0;
        let cy = g.y as f32 + g.height as f32 / 2.0;
        let radius = (g.width.min(g.height) as f32) * 0.12;
        if let Some(ring) = PathBuilder::from_circle(cx, cy, radius) {
            let stroke = Stroke {
                width: 2.5,
                ..Stroke::default()
            };
            canvas.stroke_path(&ring, &white, &stroke, Transform::identity(), None);
        }
        let tri = radius * 0.55;
        let mut pb = PathBuilder::new();
        pb.move_to(cx - tri * 0.35, cy - tri);
        pb.line_to(cx - tri * 0.35, cy + tri);
        pb.line_to(cx + tri * 0.8, cy);
        pb.close();
        if let Some(path) = pb.finish() {
            canvas.fill_path(
                &path,
                &white,
                FillRule::Winding,
                Transform::identity(),
                None,
            );
        }
    }
}

/// A loaded PDF.
pub struct PdfDocument {
    pdf: Pdf,
}

impl PdfDocument {
    /// Parses the file; an unreadable or encrypted PDF is an error.
    pub fn load(bytes: Vec<u8>) -> Result<Self, String> {
        let pdf = Pdf::new(bytes).map_err(|e| format!("Invalid PDF: {e:?}"))?;
        Ok(Self { pdf })
    }

    pub fn page_count(&self) -> usize {
        self.pdf.pages().len()
    }

    /// The slide at `index`, with its video links drawn, and those links.
    pub fn render(&self, index: usize) -> Result<(Frame, Vec<SlideVideo>), String> {
        let page = self
            .pdf
            .pages()
            .get(index)
            .ok_or_else(|| format!("Failed to render slide {index}: no such page"))?;
        let (page_w, page_h) = page.render_dimensions();
        let (scale, width, height) = fit_render_size(page_w as f64, page_h as f64)
            .map_err(|e| format!("Failed to render slide {index}: {e}"))?;
        let cache = RenderCache::new();
        let mut ctx = RenderContext::new(width as u16, height as u16);
        let transform = Affine::scale(scale) * page.initial_transform(true).to_kurbo_affine();
        render_into(
            page,
            &cache,
            &InterpreterSettings::default(),
            &RenderSettings::default(),
            &mut ctx,
            transform,
        );
        ctx.flush();
        let mut pixmap = Pixmap::new(width as u16, height as u16);
        ctx.render_with(
            &mut pixmap,
            &mut Resources::default(),
            RasterizerSettings {
                target_init: TargetInit::Clear(WHITE),
                ..Default::default()
            },
        );
        // Opaque on white, so premultiplied is straight alpha.
        let mut rgba = pixmap.data_as_u8_slice().to_vec();
        let viewport_height = page_h as f64 * scale;
        let videos: Vec<SlideVideo> = media_links(page.raw())
            .into_iter()
            .map(|(url, rect)| SlideVideo {
                url,
                geometry: rect_to_canvas(&rect, scale, viewport_height),
            })
            .collect();
        let boxes: Vec<Geometry> = videos.iter().map(|v| v.geometry).collect();
        paint_video_placeholders(&mut rgba, width, height, &boxes);
        Ok((
            Frame {
                rgba: Arc::new(rgba),
                width,
                height,
            },
            videos,
        ))
    }
}

/// The `TransformExt` conversion, named here so the call above reads plainly.
trait ToKurbo {
    fn to_kurbo_affine(self) -> Affine;
}

impl ToKurbo for hayro::hayro_syntax::transform::Transform {
    fn to_kurbo_affine(self) -> Affine {
        use hayro::hayro_interpret::util::TransformExt;
        self.to_kurbo()
    }
}
