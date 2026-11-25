//! Video sizes: the bounds a size must meet.

use uuav::MAX_PIXELS;

/// The smallest edge a video may have; a smaller one is refused before it reaches the device.
const MIN_EDGE: u32 = 16;

/// A stream header's size when it has one: FFmpeg leaves an edge at 0 when the probe decoded
/// no frame (live HLS and RTMP, often), and the planes bring the size later.
pub(super) fn known_size((width, height): (u32, u32)) -> Option<(u32, u32)> {
    (width != 0 && height != 0).then_some((width, height))
}

/// Why a video of this size cannot be shown: an edge under [`MIN_EDGE`], over the device's
/// `max_dimension` (0 for a device not known yet: no edge limit), or more than the client's
/// [`MAX_PIXELS`].
pub(super) fn check_bounds(width: u32, height: u32, max_dimension: u32) -> Result<(), String> {
    if width < MIN_EDGE || height < MIN_EDGE {
        return Err(format!("{width}x{height} is under {MIN_EDGE} px"));
    }
    if max_dimension != 0 && (width > max_dimension || height > max_dimension) {
        return Err(format!(
            "{width}x{height} exceeds the device's {max_dimension} texture edge"
        ));
    }
    if u64::from(width) * u64::from(height) > MAX_PIXELS {
        return Err(format!(
            "{width}x{height} exceeds the {MAX_PIXELS} pixel cap"
        ));
    }
    Ok(())
}

/// Why planes of this size cannot become textures: out of [`check_bounds`], or not an NV12
/// size.
pub(super) fn check_size(width: u32, height: u32, max_dimension: u32) -> Result<(), String> {
    check_bounds(width, height, max_dimension)?;
    if !width.is_multiple_of(2) || !height.is_multiple_of(2) {
        return Err(format!("{width}x{height} is not an NV12 size"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_header_with_a_zero_edge_has_no_size_yet() {
        assert_eq!(known_size((0, 0)), None);
        assert_eq!(known_size((1920, 0)), None);
        assert_eq!(known_size((0, 1080)), None);
        assert_eq!(known_size((1920, 1080)), Some((1920, 1080)));
    }

    #[test]
    fn an_nv12_size_within_the_limits_passes() {
        assert_eq!(check_size(852, 480, 8192), Ok(()));
        assert_eq!(check_size(4096, 4096, 8192), Ok(()));
        assert_eq!(check_size(16, 16, 8192), Ok(()));
    }

    #[test]
    fn an_odd_size_is_refused_as_planes_but_passes_the_bounds() {
        assert!(check_size(853, 480, 8192).is_err());
        assert!(check_size(852, 481, 8192).is_err());
        assert_eq!(check_bounds(853, 480, 8192), Ok(()));
    }

    #[test]
    fn an_edge_under_16_px_is_refused() {
        assert!(
            check_bounds(8, 8, 8192)
                .unwrap_err()
                .contains("under 16 px")
        );
        assert!(check_bounds(1920, 8, 8192).is_err());
        assert!(check_size(0, 480, 8192).is_err());
    }

    #[test]
    fn an_unknown_device_limit_keeps_the_pixel_cap() {
        assert_eq!(check_bounds(4096, 4096, 0), Ok(()));
        assert!(
            check_bounds(8192, 4096, 0)
                .unwrap_err()
                .contains("pixel cap")
        );
    }

    #[test]
    fn a_size_past_the_device_or_the_pixel_cap_is_refused() {
        assert!(
            check_size(8194, 16, 8192)
                .unwrap_err()
                .contains("texture edge")
        );
        assert!(
            check_size(8192, 4096, 8192)
                .unwrap_err()
                .contains("pixel cap")
        );
    }
}
