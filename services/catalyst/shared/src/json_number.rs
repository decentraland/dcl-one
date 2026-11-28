//! ajv's `type: 'number'` read as an integer: `1`, `1.0` and `1e0` are one
//! value; a fraction, or a value outside the target type, is refused.

use serde::Deserialize;

enum JsonNumber {
    Unsigned(u64),
    Signed(i64),
    Float(f64),
}

impl<'de> Deserialize<'de> for JsonNumber {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let number = serde_json::Number::deserialize(deserializer)?;
        if let Some(v) = number.as_u64() {
            Ok(Self::Unsigned(v))
        } else if let Some(v) = number.as_i64() {
            Ok(Self::Signed(v))
        } else {
            number
                .as_f64()
                .map(Self::Float)
                .ok_or_else(|| serde::de::Error::custom("number is outside the supported range"))
        }
    }
}

impl JsonNumber {
    fn integral<E: serde::de::Error>(value: f64) -> Result<f64, E> {
        if value.is_finite() && value.fract() == 0.0 {
            Ok(value)
        } else {
            Err(E::custom(format!("{value} is not an integer")))
        }
    }
    fn into_i64<E: serde::de::Error>(self) -> Result<i64, E> {
        match self {
            JsonNumber::Unsigned(v) => {
                i64::try_from(v).map_err(|_| E::custom(format!("{v} does not fit in an i64")))
            }
            JsonNumber::Signed(v) => Ok(v),
            JsonNumber::Float(v) => {
                let v = Self::integral(v)?;
                if v >= -(2f64.powi(63)) && v < 2f64.powi(63) {
                    Ok(crate::casts::f64_to_i64(v))
                } else {
                    Err(E::custom(format!("{v} does not fit in an i64")))
                }
            }
        }
    }
    fn into_u64<E: serde::de::Error>(self) -> Result<u64, E> {
        match self {
            JsonNumber::Unsigned(v) => Ok(v),
            JsonNumber::Signed(v) => {
                u64::try_from(v).map_err(|_| E::custom(format!("{v} is negative")))
            }
            JsonNumber::Float(v) => {
                let v = Self::integral(v)?;
                if (0.0..2f64.powi(64)).contains(&v) {
                    Ok(crate::casts::f64_to_u64(v))
                } else {
                    Err(E::custom(format!("{v} does not fit in a u64")))
                }
            }
        }
    }
}

/// A JSON number read as an `i64` (`137`, `137.0`, `1.37e2`).
pub fn integral_i64<'de, D: serde::Deserializer<'de>>(d: D) -> Result<i64, D::Error> {
    JsonNumber::deserialize(d)?.into_i64()
}

/// A JSON number read as a `u64` (`1`, `1.0`, `1e0`).
pub fn integral_u64<'de, D: serde::Deserializer<'de>>(d: D) -> Result<u64, D::Error> {
    JsonNumber::deserialize(d)?.into_u64()
}

/// A JSON value's integral `u64`, in any number form; `None` otherwise.
pub fn as_integral_u64(value: &serde_json::Value) -> Option<u64> {
    integral_u64(value).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_integral_number_form_reads_as_one_value() {
        let u = |text: &str| as_integral_u64(&serde_json::from_str(text).unwrap());
        for (text, expected) in [("1", 1), ("1.0", 1), ("1e0", 1), ("-0.0", 0), ("2e1", 20)] {
            assert_eq!(u(text), Some(expected), "{text}");
        }
        assert_eq!(u("18446744073709551615"), Some(u64::MAX));
        // 2^64 itself, as an integer and as a float, is just past the range.
        for text in [
            "1.5",
            "-1",
            "-1.0",
            "1e20",
            "18446744073709551616",
            "18446744073709551616.0",
            "\"1\"",
            "null",
        ] {
            assert_eq!(u(text), None, "{text}");
        }
        let i =
            |text: &str| integral_i64(&serde_json::from_str::<serde_json::Value>(text).unwrap());
        assert_eq!(i("-3.0").unwrap(), -3);
        assert_eq!(i("137").unwrap(), 137);
        assert!(i("1.5").is_err());
        assert!(i("9223372036854775808").is_err());
        assert_eq!(i("-9223372036854775808.0").unwrap(), i64::MIN);
        assert!(i("-1e19").is_err());
    }
}
