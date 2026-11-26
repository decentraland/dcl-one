//! Checked integer conversions for the market's chain, database and request
//! values: a block, cursor or count outside the target type is an error or an
//! explicit saturation, never a silent wrap.

use std::time::Duration;

/// A value outside the range of the type it is converted to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("{0} is out of range for its column")]
pub struct OutOfRange(pub i128);

impl From<OutOfRange> for sqlx::Error {
    fn from(e: OutOfRange) -> Self {
        sqlx::Error::Encode(Box::new(e))
    }
}

/// A chain block or count as its `bigint` column.
pub fn to_i64(n: u64) -> Result<i64, OutOfRange> {
    i64::try_from(n).map_err(|_| OutOfRange(n.into()))
}

/// A stored block or count as its chain value; a negative one is an error.
pub fn to_u64(n: i64) -> Result<u64, OutOfRange> {
    u64::try_from(n).map_err(|_| OutOfRange(n.into()))
}

/// A chain or stored value as an `integer` column.
pub fn to_i32<T: Copy + Into<i128>>(n: T) -> Result<i32, OutOfRange> {
    let wide: i128 = n.into();
    i32::try_from(wide).map_err(|_| OutOfRange(wide))
}

/// Whole milliseconds, saturated (`u64` milliseconds outlast any process).
pub fn millis(d: Duration) -> u64 {
    u64::try_from(d.as_millis()).unwrap_or(u64::MAX)
}

/// A length as a `bigint`, saturated (lossless where `usize` is 64 bits).
pub fn len_i64(n: usize) -> i64 {
    i64::try_from(n).unwrap_or(i64::MAX)
}

/// A length or count as an `integer` column.
pub fn len_i32(n: usize) -> Result<i32, OutOfRange> {
    i32::try_from(n).map_err(|_| OutOfRange(i128::try_from(n).unwrap_or(i128::MAX)))
}

/// A count as `usize`, saturated (lossless where `usize` is 64 bits).
pub fn to_usize(n: u64) -> usize {
    usize::try_from(n).unwrap_or(usize::MAX)
}

/// A page bound as `usize`; a negative one is 0.
pub fn page(n: i64) -> usize {
    usize::try_from(n).unwrap_or(0)
}

/// A statement's row count as the counters keep it, saturated.
pub trait RowCount {
    fn rows_usize(&self) -> usize;
    fn rows_i64(&self) -> i64;
}

impl RowCount for sqlx::postgres::PgQueryResult {
    fn rows_usize(&self) -> usize {
        to_usize(self.rows_affected())
    }

    fn rows_i64(&self) -> i64 {
        i64::try_from(self.rows_affected()).unwrap_or(i64::MAX)
    }
}

/// A request or JSON number as an integer: the fraction dropped, out-of-range
/// values saturated, NaN as 0 (what `as` does, made explicit at one place).
#[allow(
    clippy::cast_possible_truncation,
    reason = "float-to-int `as` saturates; that is the conversion wanted"
)]
pub fn saturating_i64(v: f64) -> i64 {
    v as i64
}

/// `n` clamped to the `i32` range.
pub fn clamp_i32(n: i64) -> i32 {
    let clamped = n.clamp(i64::from(i32::MIN), i64::from(i32::MAX));
    i32::try_from(clamped).unwrap_or_default()
}

/// Seconds as a `double` bind, saturated at the `i32` range (68 years).
pub fn secs_f64(secs: i64) -> f64 {
    f64::from(clamp_i32(secs))
}
