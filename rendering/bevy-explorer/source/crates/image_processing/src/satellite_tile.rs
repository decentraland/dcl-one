//! Satellite map tiles: the request path a scene may ask for and the KTX2 (Basis ETC1S or UASTC) to
//! JPEG conversion that makes a tile showable in an `<img>`.

use bevy::image::KTX2_MAGIC;

/// Sections along each axis of the satellite map.
const SECTIONS: u32 = 8;
const MIN_LEVEL: u32 = 3;
const MAX_LEVEL: u32 = 8;
/// The largest tile width or height taken; tiles are 1024 pixels square.
const MAX_TILE_SIZE: u32 = 2048;
const JPEG_QUALITY: u8 = 85;
/// KHR_DF_MODEL_ETC1S: Basis Universal ETC1S, stored BasisLZ-supercompressed.
const DF_MODEL_ETC1S: u8 = 163;
/// KHR_DF_MODEL_UASTC: Basis Universal UASTC, stored plain or Zstandard-supercompressed.
const DF_MODEL_UASTC: u8 = 166;
const SUPERCOMPRESSION_NONE: u32 = 0;
const SUPERCOMPRESSION_BASIS_LZ: u32 = 1;
const SUPERCOMPRESSION_ZSTD: u32 = 2;

/// The path a tile is requested by, relative to the satellite maps root:
/// `sections/<sx>%2C<sy>/<version>/<level>/<x>%2C<y>.ktx2` (a literal `,` is taken for `%2C`),
/// with sections below 8, a level from 3 to 8 and a tile inside its section. Returns the
/// path with `%2C` separators, or `None` for anything else.
pub fn validate_satellite_tile_path(path: &str) -> Option<String> {
    fn int(s: &str) -> Option<u32> {
        if s.is_empty() || s.len() > 10 || !s.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        s.parse().ok()
    }
    fn pair(s: &str) -> Option<(u32, u32)> {
        let (a, b) = s.split_once("%2C").or_else(|| s.split_once(','))?;
        Some((int(a)?, int(b)?))
    }

    let mut parts = path.split('/');
    let (sections, section, version, level, file) = (
        parts.next()?,
        parts.next()?,
        parts.next()?,
        parts.next()?,
        parts.next()?,
    );
    if parts.next().is_some() || sections != "sections" {
        return None;
    }
    let (sx, sy) = pair(section)?;
    if sx >= SECTIONS || sy >= SECTIONS {
        return None;
    }
    if version.is_empty()
        || version.len() > 64
        || version.chars().all(|c| c == '.')
        || !version
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'))
    {
        return None;
    }
    let level = int(level).filter(|l| (MIN_LEVEL..=MAX_LEVEL).contains(l))?;
    let (x, y) = pair(file.strip_suffix(".ktx2")?)?;
    // a tile is one of those subdividing the section it is requested under
    let shift = level - MIN_LEVEL;
    if x >> shift != sx || y >> shift != sy {
        return None;
    }
    Some(format!(
        "sections/{sx}%2C{sy}/{version}/{level}/{x}%2C{y}.ktx2"
    ))
}

/// Whether `bytes` start a single 2D Basis KTX2 image of a size this path takes: ETC1S, or UASTC
/// (the tile source publishes both). Checked
/// before `basisu` reads the file, so no other codec's tables are initialized for it.
fn check_tile_header(bytes: &[u8]) -> bool {
    let u32_at = |at: usize| -> Option<u32> {
        Some(u32::from_le_bytes(bytes.get(at..at + 4)?.try_into().ok()?))
    };
    let valid = || -> Option<bool> {
        let (format, width, height) = (u32_at(12)?, u32_at(20)?, u32_at(24)?);
        let (depth, layers, faces) = (u32_at(28)?, u32_at(32)?, u32_at(36)?);
        let (levels, scheme) = (u32_at(40)?, u32_at(44)?);
        let dfd = usize::try_from(u32_at(48)?).ok()?;
        let dfd_len = usize::try_from(u32_at(52)?).ok()?;
        let descriptor = bytes.get(dfd..dfd.checked_add(dfd_len)?)?;
        Some(
            bytes.starts_with(KTX2_MAGIC)
                && format == 0
                && matches!(dfd_len, 44 | 60)
                && match descriptor.get(12) {
                    Some(&DF_MODEL_ETC1S) => scheme == SUPERCOMPRESSION_BASIS_LZ,
                    Some(&DF_MODEL_UASTC) => {
                        matches!(scheme, SUPERCOMPRESSION_NONE | SUPERCOMPRESSION_ZSTD)
                    }
                    _ => false,
                }
                && (1..=MAX_TILE_SIZE).contains(&width)
                && (1..=MAX_TILE_SIZE).contains(&height)
                && depth == 0
                && layers <= 1
                && faces == 1
                && levels == 1,
        )
    };
    valid() == Some(true)
}

/// A satellite tile (single-level Basis KTX2) as a JPEG: transcoded to RGBA, alpha
/// dropped, encoded at quality 85. Takes tens of milliseconds for a 1024x1024 tile.
pub fn satellite_tile_jpeg(ktx2: &[u8]) -> Result<Vec<u8>, &'static str> {
    use basisu::{DecodeFlags, TargetFormat, Transcoder};

    if !check_tile_header(ktx2) {
        return Err("not a single-level Basis KTX2 tile");
    }
    let texture = Transcoder::new(ktx2).map_err(|_| "unreadable KTX2 tile")?;
    let info = texture
        .image_level_info(0)
        .map_err(|_| "unreadable KTX2 tile")?;
    let rgba = texture
        .transcode(0, TargetFormat::Rgba32, DecodeFlags::NONE)
        .map_err(|_| "tile transcode failed")?;
    let rgba = image::RgbaImage::from_raw(info.width, info.height, rgba)
        .ok_or("tile transcode returned the wrong size")?;
    let rgb = image::DynamicImage::ImageRgba8(rgba).to_rgb8();

    let mut jpeg = Vec::new();
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut jpeg, JPEG_QUALITY)
        .encode(
            rgb.as_raw(),
            rgb.width(),
            rgb.height(),
            image::ExtendedColorType::Rgb8,
        )
        .map_err(|_| "tile JPEG encoding failed")?;
    Ok(jpeg)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tile_paths_are_validated_strictly() {
        let ok = "sections/3%2C3/2026-11-09-143044/5/12%2C12.ktx2";
        assert_eq!(validate_satellite_tile_path(ok).as_deref(), Some(ok));
        assert_eq!(
            validate_satellite_tile_path("sections/3,3/v1.2_a/5/12,12.ktx2").as_deref(),
            Some("sections/3%2C3/v1.2_a/5/12%2C12.ktx2")
        );
        // the last tile of the finest level and the first of the coarsest
        assert!(validate_satellite_tile_path("sections/7%2C7/v/8/255%2C255.ktx2").is_some());
        assert!(validate_satellite_tile_path("sections/0%2C0/v/3/0%2C0.ktx2").is_some());

        for bad in [
            "",
            "https://example.com/sections/3%2C3/v/5/12%2C12.ktx2",
            "/sections/3%2C3/v/5/12%2C12.ktx2",
            "sections/3%2C3/v/5/12%2C12.ktx2/extra",
            "sections/3%2C3/v/5/12%2C12.png",
            "other/3%2C3/v/5/12%2C12.ktx2",
            "sections/8%2C0/v/5/12%2C12.ktx2",
            "sections/0%2C8/v/5/12%2C12.ktx2",
            "sections/-1%2C0/v/5/12%2C12.ktx2",
            "sections/+1%2C0/v/5/12%2C12.ktx2",
            "sections/3%2C3//5/12%2C12.ktx2",
            "sections/3%2C3/../5/12%2C12.ktx2",
            "sections/3%2C3/./5/12%2C12.ktx2",
            "sections/3%2C3/v%2F1/5/12%2C12.ktx2",
            "sections/3%2C3/v?x=1/5/12%2C12.ktx2",
            "sections/3%2C3/v/2/0%2C0.ktx2",
            "sections/3%2C3/v/9/0%2C0.ktx2",
            "sections/3%2C3/v/5/32%2C0.ktx2",
            "sections/3%2C3/v/5/0%2C32.ktx2",
            "sections/3%2C3/v/5/12%2C16.ktx2",
            "sections/3%2C3/v/3/2%2C3.ktx2",
            "sections/3%2C3/v/5/1.ktx2",
            "sections/3%2C3/v/5/1%2C1%2C1.ktx2",
            "sections/3%2C3/v/5/%2C1.ktx2",
        ] {
            assert_eq!(validate_satellite_tile_path(bad), None, "{bad}");
        }
        let long_version = "a".repeat(65);
        assert!(validate_satellite_tile_path(&format!(
            "sections/3%2C3/{long_version}/5/12%2C12.ktx2"
        ))
        .is_none());
    }

    // a header with a 44-byte descriptor at offset 80, as a 1024x1024 single-level ETC1S tile
    fn etc1s_header() -> Vec<u8> {
        let mut bytes = vec![0u8; 124];
        bytes[..12].copy_from_slice(KTX2_MAGIC);
        let mut put =
            |at: usize, value: u32| bytes[at..at + 4].copy_from_slice(&value.to_le_bytes());
        put(20, 1024); // width
        put(24, 1024); // height
        put(36, 1); // faces
        put(40, 1); // levels
        put(44, SUPERCOMPRESSION_BASIS_LZ);
        put(48, 80); // dfd offset
        put(52, 44); // dfd length
        bytes[80 + 12] = DF_MODEL_ETC1S;
        bytes
    }

    #[test]
    fn other_codecs_or_garbage_tiles_are_rejected() {
        assert!(check_tile_header(&etc1s_header()));

        let patched = |at: usize, value: u32| {
            let mut bytes = etc1s_header();
            bytes[at..at + 4].copy_from_slice(&value.to_le_bytes());
            bytes
        };
        assert!(!check_tile_header(&patched(12, 145))); // a declared GPU format
        assert!(!check_tile_header(&patched(44, 0))); // no BasisLZ
        assert!(!check_tile_header(&patched(20, 4096))); // oversized
        assert!(!check_tile_header(&patched(24, 0)));
        assert!(!check_tile_header(&patched(40, 2))); // a mip chain
        assert!(!check_tile_header(&patched(36, 6))); // a cube map
        assert!(!check_tile_header(&patched(48, u32::MAX))); // descriptor out of range
                                                             // UASTC is taken plain or with Zstandard, never with BasisLZ
        let uastc = |scheme: u32| {
            let mut bytes = patched(44, scheme);
            bytes[80 + 12] = DF_MODEL_UASTC;
            bytes
        };
        assert!(check_tile_header(&uastc(SUPERCOMPRESSION_NONE)));
        assert!(check_tile_header(&uastc(SUPERCOMPRESSION_ZSTD)));
        assert!(!check_tile_header(&uastc(SUPERCOMPRESSION_BASIS_LZ)));
        let mut other = etc1s_header();
        other[80 + 12] = 167; // another descriptor model
        assert!(!check_tile_header(&other));
        assert!(!check_tile_header(&etc1s_header()[..60]));

        assert!(satellite_tile_jpeg(&[]).is_err());
        assert!(satellite_tile_jpeg(b"not a ktx2 file at all").is_err());
        assert!(satellite_tile_jpeg(&other).is_err());
        // a header alone has no image data to transcode
        assert!(satellite_tile_jpeg(&uastc(SUPERCOMPRESSION_ZSTD)).is_err());
    }
}
