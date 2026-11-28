//! Explicit numeric conversions: a value outside the target type saturates
//! or is reported, never silently wraps. Each helper names the one place a
//! lossy cast is intended.

use std::time::Duration;

/// Whole milliseconds, saturated (`u64` milliseconds outlast any process).
pub fn millis(d: Duration) -> u64 {
    u64::try_from(d.as_millis()).unwrap_or(u64::MAX)
}

/// Whole microseconds, saturated at `u64::MAX`.
pub fn micros(d: Duration) -> u64 {
    u64::try_from(d.as_micros()).unwrap_or(u64::MAX)
}

/// A length or count as a `bigint`, saturated (lossless where `usize` is 64 bits).
pub fn len_i64(n: usize) -> i64 {
    i64::try_from(n).unwrap_or(i64::MAX)
}

/// A count as `u64`, saturated (lossless where `usize` is at most 64 bits).
pub fn len_u64(n: usize) -> u64 {
    u64::try_from(n).unwrap_or(u64::MAX)
}

/// A count as `usize`, saturated (lossless where `usize` is 64 bits).
pub fn to_usize(n: u64) -> usize {
    usize::try_from(n).unwrap_or(usize::MAX)
}

/// A SQL count or row number as `u64`; a negative one (never produced by a
/// count) is 0.
pub fn count_u64(n: i64) -> u64 {
    u64::try_from(n).unwrap_or(0)
}

/// A non-negative index or count as `usize`; a negative one is 0, one past
/// `usize` saturates.
pub fn index_usize(n: i64) -> usize {
    if n < 0 {
        0
    } else {
        usize::try_from(n).unwrap_or(usize::MAX)
    }
}

/// A `u64` as a `bigint`, saturated at `i64::MAX`.
pub fn saturating_i64(n: u64) -> i64 {
    i64::try_from(n).unwrap_or(i64::MAX)
}

/// `n` clamped to the `i32` range.
pub fn clamp_i32(n: i64) -> i32 {
    let clamped = n.clamp(i64::from(i32::MIN), i64::from(i32::MAX));
    i32::try_from(clamped).unwrap_or_default()
}

/// A count as an `integer` column, saturated at `i32::MAX`.
pub fn sat_i32(n: u32) -> i32 {
    i32::try_from(n).unwrap_or(i32::MAX)
}

/// A statement's row count, saturated.
pub trait RowCount {
    fn rows_usize(&self) -> usize;
}

impl RowCount for sqlx::postgres::PgQueryResult {
    fn rows_usize(&self) -> usize {
        to_usize(self.rows_affected())
    }
}

/// A float as `u64` the way `as` converts it: the fraction dropped,
/// out-of-range values saturated, NaN as 0.
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "float-to-int `as` saturates; that is the conversion wanted"
)]
pub fn f64_to_u64(v: f64) -> u64 {
    v as u64
}

/// A float as `i64` the way `as` converts it (see [`f64_to_u64`]).
#[allow(
    clippy::cast_possible_truncation,
    reason = "float-to-int `as` saturates; that is the conversion wanted"
)]
pub fn f64_to_i64(v: f64) -> i64 {
    v as i64
}

/// A float as `usize` the way `as` converts it (see [`f64_to_u64`]).
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "float-to-int `as` saturates; that is the conversion wanted"
)]
pub fn f64_to_usize(v: f64) -> usize {
    v as usize
}

/// An integer as the nearest `f64`, for metrics, ratios and JSON numbers:
/// above 2^53 it rounds, which those tolerate.
pub trait ApproxF64 {
    fn approx_f64(self) -> f64;
}

macro_rules! approx_f64 {
    ($($t:ty),*) => {$(
        impl ApproxF64 for $t {
            #[allow(
                clippy::cast_precision_loss,
                reason = "rounding above 2^53 is acceptable for an approximate value"
            )]
            fn approx_f64(self) -> f64 {
                self as f64
            }
        }
    )*};
}

approx_f64!(u64, i64, usize, isize, u128, i128);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn float_conversions_match_as() {
        assert_eq!(f64_to_u64(f64::NAN), 0);
        assert_eq!(f64_to_u64(-1.5), 0);
        assert_eq!(f64_to_u64(1e30), u64::MAX);
        assert_eq!(f64_to_i64(-2.7), -2);
        assert_eq!(f64_to_usize(3.9), 3);
    }

    #[test]
    fn integer_conversions_saturate() {
        assert_eq!(count_u64(-1), 0);
        assert_eq!(index_usize(-5), 0);
        assert_eq!(index_usize(7), 7);
        assert_eq!(saturating_i64(u64::MAX), i64::MAX);
        assert_eq!(millis(Duration::MAX), u64::MAX);
    }
}
