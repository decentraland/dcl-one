//! Numeric conversions shared across the CLI: saturating where a bound is the
//! right answer, and the float conversions behind one allow each.

use std::time::Duration;

// A count or size as a float (rounded past 2^53), and a float truncated
// toward zero (saturating, NaN as 0).
pub use dcl_scene_authoring::casts::{js_number as approx, trunc_i64};

/// A signed coordinate or timestamp as a float (a JS number).
#[allow(
    clippy::cast_precision_loss,
    reason = "coordinates and millisecond timestamps, exact below 2^53"
)]
pub fn approx_signed(n: i64) -> f64 {
    n as f64
}

/// A float rounded to the nearest whole number: saturating, negatives and NaN as 0.
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "float-to-int `as` saturates and maps NaN to 0, the behaviour wanted"
)]
pub fn round_u64(v: f64) -> u64 {
    v.round() as u64
}

/// Whole milliseconds, saturated.
pub fn millis_u64(d: Duration) -> u64 {
    u64::try_from(d.as_millis()).unwrap_or(u64::MAX)
}

/// Whole milliseconds, saturated.
pub fn millis_i64(d: Duration) -> i64 {
    i64::try_from(d.as_millis()).unwrap_or(i64::MAX)
}

/// A count as `usize`, saturated (lossless where `usize` is 64 bits).
pub fn to_usize(n: u64) -> usize {
    usize::try_from(n).unwrap_or(usize::MAX)
}

/// A stored count as `usize`; a negative one is 0.
pub fn nonneg_usize(n: i64) -> usize {
    usize::try_from(n).unwrap_or(0)
}

/// A length as an `i64`, saturated (lossless where `usize` is 64 bits).
pub fn len_i64(n: usize) -> i64 {
    i64::try_from(n).unwrap_or(i64::MAX)
}

/// A child's pid as the `i32` kill(2) takes; `None` past `i32::MAX`.
pub fn pid_i32(pid: u32) -> Option<i32> {
    i32::try_from(pid).ok()
}
