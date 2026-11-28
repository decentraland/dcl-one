//! Numeric conversions shared across comms: metric floats, saturated
//! durations and the unix clock, each spelled once instead of an `as` per site.

use std::time::Duration;

/// A count or size as a float, for metrics and ratios.
#[allow(
    clippy::cast_precision_loss,
    reason = "counts and sizes stay far below 2^53, where f64 is exact"
)]
#[inline]
pub fn usize_f64(n: usize) -> f64 {
    n as f64
}

/// A signed count or clock value as a float.
#[allow(
    clippy::cast_precision_loss,
    reason = "counts and unix milliseconds stay far below 2^53, where f64 is exact"
)]
#[inline]
pub fn i64_f64(n: i64) -> f64 {
    n as f64
}

/// An unsigned count or clock value as a float.
#[allow(
    clippy::cast_precision_loss,
    reason = "counts and unix milliseconds stay far below 2^53, where f64 is exact"
)]
#[inline]
pub fn u64_f64(n: u64) -> f64 {
    n as f64
}

/// Whole milliseconds, saturated (`u64` milliseconds outlast any process).
#[inline]
pub fn millis(d: Duration) -> u64 {
    u64::try_from(d.as_millis()).unwrap_or(u64::MAX)
}

/// Whole microseconds, saturated.
#[inline]
pub fn micros(d: Duration) -> u64 {
    u64::try_from(d.as_micros()).unwrap_or(u64::MAX)
}

/// Whole milliseconds as a signed clock offset, saturated.
#[inline]
pub fn millis_i64(d: Duration) -> i64 {
    i64::try_from(d.as_millis()).unwrap_or(i64::MAX)
}

/// Unix seconds now; a clock before the epoch reads 0.
#[inline]
pub fn unix_secs() -> u64 {
    u64::try_from(chrono::Utc::now().timestamp()).unwrap_or(0)
}

/// Unix milliseconds now; a clock before the epoch reads 0.
#[inline]
pub fn unix_millis() -> u64 {
    u64::try_from(chrono::Utc::now().timestamp_millis()).unwrap_or(0)
}

/// A non-negative stored or clock value as `u64`; a negative one reads 0.
#[inline]
pub fn non_negative(n: i64) -> u64 {
    u64::try_from(n).unwrap_or(0)
}

/// A length or count as a `bigint`, saturated (lossless where `usize` is 64 bits).
#[inline]
pub fn len_i64(n: usize) -> i64 {
    i64::try_from(n).unwrap_or(i64::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn durations_saturate_instead_of_wrapping() {
        assert_eq!(millis(Duration::from_millis(1500)), 1500);
        assert_eq!(micros(Duration::from_micros(7)), 7);
        assert_eq!(millis(Duration::MAX), u64::MAX);
        assert_eq!(micros(Duration::MAX), u64::MAX);
        assert_eq!(millis_i64(Duration::MAX), i64::MAX);
    }

    #[test]
    fn negative_values_read_as_zero() {
        assert_eq!(non_negative(-7), 0);
        assert_eq!(non_negative(i64::MIN), 0);
        assert_eq!(non_negative(0), 0);
        assert_eq!(non_negative(42), 42);
        assert_eq!(non_negative(i64::MAX), 9_223_372_036_854_775_807);
    }

    #[test]
    fn lengths_and_floats_keep_their_value() {
        assert_eq!(len_i64(3), 3);
        assert_eq!(usize_f64(1 << 20), 1_048_576.0);
        assert_eq!(i64_f64(-5), -5.0);
        assert_eq!(u64_f64(1 << 52), 4_503_599_627_370_496.0);
    }

    #[test]
    fn the_clock_reads_after_the_epoch() {
        assert!(unix_secs() > 1_700_000_000);
        assert!(unix_millis() > 1_700_000_000_000);
    }
}
