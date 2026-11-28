//! The JS number conversions the encoders mirror, kept to one place.

/// A double as a `Float32Array` element: rounded to the nearest f32.
#[allow(
    clippy::cast_possible_truncation,
    reason = "f32 fields round like Float32Array; that is the encoding"
)]
pub fn to_f32(v: f64) -> f32 {
    v as f32
}

/// A double truncated toward zero: saturating, NaN as 0.
#[allow(
    clippy::cast_possible_truncation,
    reason = "float-to-int `as` saturates and maps NaN to 0, the behaviour wanted"
)]
pub fn trunc_i64(v: f64) -> i64 {
    v as i64
}

/// A double truncated toward zero: saturating, negatives and NaN as 0.
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "float-to-int `as` saturates and maps NaN to 0, the behaviour wanted"
)]
pub fn trunc_u64(v: f64) -> u64 {
    v as u64
}

/// An integer as a JS number: rounded past 2^53 as JS rounds it.
#[allow(
    clippy::cast_precision_loss,
    reason = "JS numbers are doubles; this is the rounding the JS original does"
)]
pub fn js_number(v: u64) -> f64 {
    v as f64
}
