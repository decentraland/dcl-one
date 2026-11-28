//! Checked and saturating conversions for counts, pages and readings: a value
//! outside the target type saturates or is skipped, never silently wraps.

/// A length or count as a `bigint`, saturated (lossless where `usize` is 64 bits).
pub(crate) fn len_i64(n: usize) -> i64 {
    i64::try_from(n).unwrap_or(i64::MAX)
}

/// A length or count as an `integer`, saturated.
pub(crate) fn len_i32(n: usize) -> i32 {
    i32::try_from(n).unwrap_or(i32::MAX)
}

/// A row or event count as a `bigint`, saturated.
pub(crate) fn count_i64(n: u64) -> i64 {
    i64::try_from(n).unwrap_or(i64::MAX)
}

/// A count as `usize`, saturated (lossless where `usize` is 64 bits).
pub(crate) fn to_usize(n: u64) -> usize {
    usize::try_from(n).unwrap_or(usize::MAX)
}

/// A page bound, offset or stored count as `usize`; a negative one is 0, one
/// past `usize` saturates.
pub(crate) fn page(n: i64) -> usize {
    usize::try_from(n.max(0)).unwrap_or(usize::MAX)
}

/// A stored count as `u64`; a negative one is 0.
pub(crate) fn nonneg_u64(n: i64) -> u64 {
    u64::try_from(n).unwrap_or(0)
}

/// `n` clamped to the `i32` range.
pub(crate) fn clamp_i32(n: i64) -> i32 {
    i32::try_from(n.clamp(i64::from(i32::MIN), i64::from(i32::MAX))).unwrap_or_default()
}

/// A float as an integer: the fraction dropped, out-of-range values saturated,
/// NaN as 0 (what `as` does, made explicit at one place).
#[allow(
    clippy::cast_possible_truncation,
    reason = "float-to-int `as` saturates; that is the conversion wanted"
)]
pub(crate) fn f64_to_i64(v: f64) -> i64 {
    v as i64
}

/// As [`f64_to_i64`], with negatives as 0.
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "float-to-int `as` saturates; that is the conversion wanted"
)]
pub(crate) fn f64_to_u64(v: f64) -> u64 {
    v as u64
}

/// A count, size or timestamp as a reading or ratio operand.
#[allow(
    clippy::cast_precision_loss,
    reason = "exact below 2^53; larger values round to the nearest f64 by design"
)]
pub(crate) fn approx_f64(n: i64) -> f64 {
    n as f64
}

/// As [`approx_f64`], for an unsigned value.
#[allow(
    clippy::cast_precision_loss,
    reason = "exact below 2^53; larger values round to the nearest f64 by design"
)]
pub(crate) fn approx_u64_f64(n: u64) -> f64 {
    n as f64
}

/// As [`approx_f64`], for a length.
#[allow(
    clippy::cast_precision_loss,
    reason = "exact below 2^53; larger values round to the nearest f64 by design"
)]
pub(crate) fn approx_len_f64(n: usize) -> f64 {
    n as f64
}

/// A float as an `f32` wire or `real` column value, rounded to its precision.
#[allow(
    clippy::cast_possible_truncation,
    reason = "the target holds f32; rounding to it is the stored precision"
)]
pub(crate) fn narrow_f32(v: f64) -> f32 {
    v as f32
}
