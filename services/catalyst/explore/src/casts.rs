//! Saturating conversions for explore's counts, pages and map arithmetic: a value
//! outside the target type saturates explicitly, never wraps silently.

/// A length or count as a `bigint`, saturated (lossless where `usize` is 64 bits).
pub fn len_i64(n: usize) -> i64 {
    i64::try_from(n).unwrap_or(i64::MAX)
}

/// A length or count as an `integer`, saturated.
pub fn len_i32(n: usize) -> i32 {
    i32::try_from(n).unwrap_or(i32::MAX)
}

/// A row or item count as a `bigint`, saturated.
pub fn count_i64(n: u64) -> i64 {
    i64::try_from(n).unwrap_or(i64::MAX)
}

/// A count as `usize`, saturated (lossless where `usize` is 64 bits).
pub fn to_usize(n: u64) -> usize {
    usize::try_from(n).unwrap_or(usize::MAX)
}

/// A stored count as `u64`; a negative one is 0.
pub fn count_u64(n: i64) -> u64 {
    u64::try_from(n).unwrap_or(0)
}

/// An upstream parcel coordinate; one outside `i32` is logged and dropped.
pub fn coord_i32(v: i64) -> Option<i32> {
    let c = i32::try_from(v).ok();
    if c.is_none() {
        tracing::warn!(coordinate = v, "dropping an out-of-range coordinate");
    }
    c
}

/// A stored page bound or offset as `usize`; a negative one is 0.
pub fn page(n: i64) -> usize {
    usize::try_from(n).unwrap_or(0)
}

/// `x as i32`: saturates at the bounds and maps NaN to 0.
#[allow(
    clippy::cast_possible_truncation,
    reason = "float-to-int `as` saturates and maps NaN to 0, as callers expect"
)]
pub fn f64_i32(x: f64) -> i32 {
    x as i32
}

/// `x as i64`: saturates at the bounds and maps NaN to 0.
#[allow(
    clippy::cast_possible_truncation,
    reason = "float-to-int `as` saturates and maps NaN to 0, as callers expect"
)]
pub fn f64_i64(x: f64) -> i64 {
    x as i64
}

/// `x as u32`: saturates at the bounds, so negatives and NaN are 0.
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "float-to-int `as` saturates and maps NaN to 0, as callers expect"
)]
pub fn f64_u32(x: f64) -> u32 {
    x as u32
}

/// A count, timestamp or score as `f64` for ranking math (rounds above 2^53).
#[allow(
    clippy::cast_precision_loss,
    reason = "scores and timestamps are far below 2^53; rounding beyond that is harmless"
)]
pub fn i64_f64(n: i64) -> f64 {
    n as f64
}
