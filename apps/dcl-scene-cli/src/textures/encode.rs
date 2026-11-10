//! What happens to one source image: decoded whatever its name says, resized
//! the way the Bevy explorer's own texture path resizes, written as a lossless
//! PNG and handed to an [`Encoder`].

use super::ColorMode;
use anyhow::Result;
use sha2::{Digest, Sha256};

/// Turns a PNG into KTX2 bytes. The production encoder is the pinned basisu
/// WASI module ([`super::basisu::WasmBasisu`]); tests use a stand-in.
pub trait Encoder: Sync {
    /// Names the encoder and every setting it applies: the same id and input
    /// must always give the same bytes, since it keys the cache.
    fn id(&self) -> &str;
    fn encode(&self, png: &[u8], mode: ColorMode) -> Result<Vec<u8>>;
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes).as_slice())
}

/// The cache key of one encode: the source bytes, the encoder (module and
/// arguments), the colour mode and the size cap.
pub fn cache_key(source_sha: &str, encoder_id: &str, mode: ColorMode, max_size: u32) -> String {
    sha256_hex(
        format!(
            "dcl-scene-cli optimized texture v1\n{source_sha}\n{encoder_id}\n{}\n{max_size}\n",
            mode.as_str()
        )
        .as_bytes(),
    )
}

/// The size the encoder is fed: unchanged when both sides are multiples of 4
/// within `max`, else scaled so the longer side fits `max` and each side
/// rounds to whole 4x4 blocks (at least one). Mirrors `compress_decoded` in
/// the explorer's `image_processing`, so a texture keeps the size the
/// explorer would have given its PNG.
pub fn target_size(width: u32, height: u32, max: u32) -> (u32, u32) {
    if width <= max && height <= max && width.is_multiple_of(4) && height.is_multiple_of(4) {
        return (width, height);
    }
    let ratio = (max as f32 / width.max(height) as f32).min(1.0);
    let side = |s: u32| ((s as f32 * ratio * 0.25).round() as u32).max(1) * 4;
    (side(width), side(height))
}

/// The PNG basisu encodes, or why this image is kept as it is. The format is
/// sniffed from the bytes, not the name (a JPEG named `.png` is common).
/// Images too small to gain anything keep their bytes, as in the explorer.
pub fn prepare(source: &[u8], max_size: u32) -> Result<Vec<u8>, String> {
    let img = image::load_from_memory(source).map_err(|e| format!("cannot decode: {e}"))?;
    let (w, h) = (img.width(), img.height());
    if w <= 2 || h <= 2 || (w as usize) * (h as usize) * 4 <= 1024 {
        return Err(format!("{w}x{h} is too small to gain from compression"));
    }
    let (tw, th) = target_size(w, h, max_size);
    let img = match (tw, th) == (w, h) {
        true => img,
        false => img.resize_exact(tw, th, image::imageops::FilterType::CatmullRom),
    };
    let img = match img.color().has_alpha() {
        true => image::DynamicImage::ImageRgba8(img.to_rgba8()),
        false => image::DynamicImage::ImageRgb8(img.to_rgb8()),
    };
    let mut png = std::io::Cursor::new(Vec::new());
    img.write_to(&mut png, image::ImageFormat::Png)
        .map_err(|e| format!("cannot write the PNG: {e}"))?;
    Ok(png.into_inner())
}

/// `f` over `items` on `workers` threads, results in item order. Threads are
/// always spawned, even for one item: the encoder must not run on a thread of
/// the caller's async runtime.
pub fn run_pool<T: Sync, U: Send>(
    items: &[T],
    workers: usize,
    f: impl Fn(&T) -> U + Sync,
) -> Vec<U> {
    let workers = workers.clamp(1, items.len().max(1));
    let next = std::sync::atomic::AtomicUsize::new(0);
    let mut slots: Vec<Option<U>> = std::iter::repeat_with(|| None).take(items.len()).collect();
    let done: Vec<Vec<(usize, U)>> = std::thread::scope(|s| {
        let handles: Vec<_> = (0..workers)
            .map(|_| {
                let (next, f) = (&next, &f);
                s.spawn(move || {
                    let mut out = Vec::new();
                    loop {
                        let i = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        let Some(item) = items.get(i) else { break };
                        out.push((i, f(item)));
                    }
                    out
                })
            })
            .collect();
        handles
            .into_iter()
            .map(|h| h.join().unwrap_or_else(|p| std::panic::resume_unwind(p)))
            .collect()
    });
    for (i, u) in done.into_iter().flatten() {
        slots[i] = Some(u);
    }
    slots
        .into_iter()
        .map(|u| u.expect("every item ran"))
        .collect()
}

/// One encoder per core: each instance is single-threaded, so the cores are
/// filled across textures rather than inside one.
pub fn workers() -> usize {
    std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn png(w: u32, h: u32, alpha: bool) -> Vec<u8> {
        let img = match alpha {
            true => image::DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(
                w,
                h,
                image::Rgba([10, 20, 30, 128]),
            )),
            false => image::DynamicImage::ImageRgb8(image::RgbImage::from_pixel(
                w,
                h,
                image::Rgb([10, 20, 30]),
            )),
        };
        let mut out = std::io::Cursor::new(Vec::new());
        img.write_to(&mut out, image::ImageFormat::Png).unwrap();
        out.into_inner()
    }

    fn jpeg(w: u32, h: u32) -> Vec<u8> {
        let img = image::DynamicImage::ImageRgb8(image::RgbImage::from_pixel(
            w,
            h,
            image::Rgb([200, 100, 50]),
        ));
        let mut out = std::io::Cursor::new(Vec::new());
        img.write_to(&mut out, image::ImageFormat::Jpeg).unwrap();
        out.into_inner()
    }

    fn dims(png: &[u8]) -> (u32, u32, bool) {
        let img = image::load_from_memory_with_format(png, image::ImageFormat::Png).unwrap();
        (img.width(), img.height(), img.color().has_alpha())
    }

    #[test]
    fn sizes_match_the_explorer_resize() {
        assert_eq!(target_size(512, 256, 1024), (512, 256));
        assert_eq!(target_size(1024, 1024, 1024), (1024, 1024));
        assert_eq!(target_size(2048, 1024, 1024), (1024, 512));
        assert_eq!(target_size(4096, 300, 1024), (1024, 76));
        assert_eq!(target_size(1023, 511, 1024), (1024, 512));
        assert_eq!(target_size(30, 18, 1024), (32, 20));
        assert_eq!(
            target_size(8192, 2, 1024),
            (1024, 4),
            "never rounds to zero"
        );
        for (w, h) in [(3000, 2001), (1025, 7), (999, 1001), (5, 4097)] {
            let (tw, th) = target_size(w, h, 1024);
            assert!(tw <= 1024 && th <= 1024, "{w}x{h} -> {tw}x{th}");
            assert!(tw.is_multiple_of(4) && th.is_multiple_of(4));
        }
    }

    #[test]
    fn prepare_sniffs_resizes_and_keeps_alpha() {
        let (w, h, alpha) = dims(&prepare(&png(2048, 1000, true), 1024).unwrap());
        assert_eq!((w, h, alpha), (1024, 500, true));
        // a JPEG, whatever it is named, decodes and becomes an RGB PNG
        let (w, h, alpha) = dims(&prepare(&jpeg(64, 30), 1024).unwrap());
        assert_eq!((w, h, alpha), (64, 32, false));
        assert!(prepare(&png(2, 512, false), 1024).is_err());
        assert!(prepare(&png(16, 16, false), 1024).is_err());
        assert!(prepare(b"not an image", 1024).is_err());
        assert_eq!(
            prepare(&png(100, 60, false), 1024),
            prepare(&png(100, 60, false), 1024),
            "the PNG is deterministic"
        );
    }

    #[test]
    fn cache_keys_change_with_every_input() {
        let base = cache_key("aa", "enc", ColorMode::Srgb, 1024);
        assert_eq!(base, cache_key("aa", "enc", ColorMode::Srgb, 1024));
        assert_ne!(base, cache_key("ab", "enc", ColorMode::Srgb, 1024));
        assert_ne!(base, cache_key("aa", "enc2", ColorMode::Srgb, 1024));
        assert_ne!(base, cache_key("aa", "enc", ColorMode::Linear, 1024));
        assert_ne!(base, cache_key("aa", "enc", ColorMode::Normal, 1024));
        assert_ne!(base, cache_key("aa", "enc", ColorMode::Srgb, 512));
        assert_eq!(base.len(), 64);
    }

    #[test]
    fn the_pool_keeps_order_and_covers_every_item() {
        let items: Vec<u32> = (0..100).collect();
        assert_eq!(
            run_pool(&items, 7, |n| n * 3),
            items.iter().map(|n| n * 3).collect::<Vec<_>>()
        );
        assert!(run_pool(&Vec::<u32>::new(), 4, |n| *n).is_empty());
        let caller = std::thread::current().id();
        assert_ne!(
            run_pool(&[1], 1, |_| std::thread::current().id())[0],
            caller
        );
    }
}
