//! Numeric conversions for scenario geometry, counters and reports, spelled
//! once instead of an `as` per site; the clock helpers come from comms.

pub use catalyst_comms::casts::{micros, millis, millis_i64, u64_f64, usize_f64};

/// A parcel coordinate, step or bot counter as a world-space float.
#[allow(
    clippy::cast_precision_loss,
    reason = "coordinates and counters stay far below 2^24, where f32 is exact"
)]
#[inline]
pub fn f32_from_i32(n: i32) -> f32 {
    n as f32
}

/// A step or bot counter as a world-space float.
#[allow(
    clippy::cast_precision_loss,
    reason = "counters stay far below 2^24, where f32 is exact"
)]
#[inline]
pub fn f32_from_u32(n: u32) -> f32 {
    n as f32
}

/// A bot index or grid count as a world-space float.
#[allow(
    clippy::cast_precision_loss,
    reason = "bot indices and grid counts stay far below 2^24, where f32 is exact"
)]
#[inline]
pub fn f32_from_usize(n: usize) -> f32 {
    n as f32
}

/// A world-space float as whole units, rounded toward zero; out-of-range
/// values saturate and NaN is 0, as float-to-int `as` does.
#[allow(
    clippy::cast_possible_truncation,
    reason = "float-to-int `as` saturates; that is the conversion wanted"
)]
#[inline]
pub fn trunc_i32(v: f32) -> i32 {
    v as i32
}

/// A bot index or count as the `u32` identities and notes take, saturated.
#[inline]
pub fn index_u32(n: usize) -> u32 {
    u32::try_from(n).unwrap_or(u32::MAX)
}

/// A JSON number as a whole integer, rounded toward zero; out-of-range values
/// saturate and NaN is 0, as float-to-int `as` does.
#[allow(
    clippy::cast_possible_truncation,
    reason = "float-to-int `as` saturates; that is the conversion wanted"
)]
#[inline]
pub fn trunc_i64(v: f64) -> i64 {
    v as i64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn float_truncation_saturates_and_maps_nan_to_zero() {
        assert_eq!(trunc_i32(-2.9), -2);
        assert_eq!(trunc_i32(f32::NAN), 0);
        assert_eq!(trunc_i32(f32::INFINITY), i32::MAX);
        assert_eq!(trunc_i32(f32::NEG_INFINITY), i32::MIN);
        assert_eq!(trunc_i64(3.99), 3);
        assert_eq!(trunc_i64(f64::NAN), 0);
        assert_eq!(trunc_i64(f64::INFINITY), i64::MAX);
        assert_eq!(trunc_i64(f64::NEG_INFINITY), i64::MIN);
    }

    #[test]
    fn indices_saturate() {
        assert_eq!(index_u32(9), 9);
        assert_eq!(index_u32(usize::MAX), u32::MAX);
    }
}
