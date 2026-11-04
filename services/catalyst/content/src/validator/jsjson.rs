//! Entity JSON as JavaScript holds and re-serializes it, for the hashes the
//! reference computes over `JSON.stringify` output (ADR-32 item hashes,
//! third-party merkle leaves): `JSON.parse` keeps a repeated key at its first
//! position with its last value, `JSON.stringify` writes integer-like keys
//! first in ascending order and then the rest in insertion order, and numbers
//! print as `Number.prototype.toString`. `serde_json::Value` without
//! `preserve_order` sorts keys, so it cannot stand in.

use std::fmt;

use serde::de::{Deserialize, Deserializer, MapAccess, SeqAccess, Visitor};

/// A JSON value as `JSON.parse` holds it.
#[derive(Debug, Clone, PartialEq)]
pub(super) enum JsValue {
    Null,
    Bool(bool),
    Number(f64),
    String(String),
    Array(Vec<JsValue>),
    Object(Vec<(String, JsValue)>),
}

impl JsValue {
    pub(super) fn as_object(&self) -> Option<&[(String, JsValue)]> {
        match self {
            JsValue::Object(entries) => Some(entries),
            _ => None,
        }
    }

    pub(super) fn get(&self, key: &str) -> Option<&JsValue> {
        self.as_object()?
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v)
    }
}

/// Assigns like a JS object: a repeated key keeps its first position and takes the last value.
fn set(entries: &mut Vec<(String, JsValue)>, key: String, value: JsValue) {
    match entries.iter_mut().find(|(k, _)| *k == key) {
        Some(slot) => slot.1 = value,
        None => entries.push((key, value)),
    }
}

impl<'de> Deserialize<'de> for JsValue {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct V;
        impl<'de> Visitor<'de> for V {
            type Value = JsValue;
            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("a JSON value")
            }
            fn visit_unit<E>(self) -> Result<JsValue, E> {
                Ok(JsValue::Null)
            }
            fn visit_none<E>(self) -> Result<JsValue, E> {
                Ok(JsValue::Null)
            }
            fn visit_some<D: Deserializer<'de>>(self, d: D) -> Result<JsValue, D::Error> {
                Deserialize::deserialize(d)
            }
            fn visit_bool<E>(self, v: bool) -> Result<JsValue, E> {
                Ok(JsValue::Bool(v))
            }
            fn visit_i64<E>(self, v: i64) -> Result<JsValue, E> {
                Ok(JsValue::Number(v as f64))
            }
            fn visit_u64<E>(self, v: u64) -> Result<JsValue, E> {
                Ok(JsValue::Number(v as f64))
            }
            fn visit_f64<E>(self, v: f64) -> Result<JsValue, E> {
                Ok(JsValue::Number(v))
            }
            fn visit_str<E>(self, v: &str) -> Result<JsValue, E> {
                Ok(JsValue::String(v.to_string()))
            }
            fn visit_string<E>(self, v: String) -> Result<JsValue, E> {
                Ok(JsValue::String(v))
            }
            fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<JsValue, A::Error> {
                let mut out = Vec::new();
                while let Some(v) = seq.next_element()? {
                    out.push(v);
                }
                Ok(JsValue::Array(out))
            }
            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<JsValue, A::Error> {
                let mut out = Vec::new();
                while let Some((k, v)) = map.next_entry::<String, JsValue>()? {
                    set(&mut out, k, v);
                }
                // serde_json's arbitrary-precision number, should the feature be on.
                if let [(key, JsValue::String(digits))] = out.as_slice()
                    && key == "$serde_json::private::Number"
                    && let Ok(n) = digits.parse::<f64>()
                {
                    return Ok(JsValue::Number(n));
                }
                Ok(JsValue::Object(out))
            }
        }
        deserializer.deserialize_any(V)
    }
}

/// The JS array-index value of a property key, if it is one.
fn array_index(key: &str) -> Option<u32> {
    let n: u32 = key.parse().ok()?;
    (n != u32::MAX && n.to_string() == key).then_some(n)
}

/// `JSON.stringify` of `value` appended to `out`.
pub(super) fn write(value: &JsValue, out: &mut String) {
    match value {
        JsValue::Null => out.push_str("null"),
        JsValue::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
        JsValue::Number(n) => out.push_str(&number(*n)),
        JsValue::String(s) => write_string(s, out),
        JsValue::Array(items) => {
            out.push('[');
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                write(item, out);
            }
            out.push(']');
        }
        JsValue::Object(entries) => write_object(entries.iter().map(|(k, v)| (k.as_str(), v)), out),
    }
}

/// `JSON.stringify` of an object built from `entries` (distinct keys) in
/// insertion order: integer-like keys first, ascending, then the rest.
pub(super) fn write_object<'a>(
    entries: impl IntoIterator<Item = (&'a str, &'a JsValue)>,
    out: &mut String,
) {
    let (mut indexed, rest): (Vec<_>, Vec<_>) = entries
        .into_iter()
        .partition(|(k, _)| array_index(k).is_some());
    indexed.sort_by_key(|(k, _)| array_index(k));
    out.push('{');
    for (i, (k, v)) in indexed.into_iter().chain(rest).enumerate() {
        if i > 0 {
            out.push(',');
        }
        write_string(k, out);
        out.push(':');
        write(v, out);
    }
    out.push('}');
}

/// A JSON string literal as `JSON.stringify` writes it.
pub(super) fn write_string(value: &str, out: &mut String) {
    out.push('"');
    for ch in value.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\u{0008}' => out.push_str("\\b"),
            '\u{000C}' => out.push_str("\\f"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
}

/// `Number.prototype.toString` for a double (`null` when not finite, as
/// `JSON.stringify` writes it).
fn number(value: f64) -> String {
    if value == 0.0 {
        return "0".to_string();
    }
    if !value.is_finite() {
        return "null".to_string();
    }
    if value < 0.0 {
        return format!("-{}", number(-value));
    }
    let formatted = format!("{value:e}");
    let Some((mantissa, exp)) = formatted.split_once('e') else {
        return formatted;
    };
    let Ok(exp) = exp.parse::<i32>() else {
        return formatted;
    };
    let digits: String = mantissa.chars().filter(|c| *c != '.').collect();
    let digits = digits.trim_end_matches('0');
    let digits = if digits.is_empty() { "0" } else { digits };
    let k = digits.len() as i32;
    let n = exp + 1;
    if k <= n && n <= 21 {
        format!("{digits}{}", "0".repeat((n - k) as usize))
    } else if 0 < n && n <= 21 {
        format!("{}.{}", &digits[..n as usize], &digits[n as usize..])
    } else if -6 < n && n <= 0 {
        format!("0.{}{digits}", "0".repeat((-n) as usize))
    } else {
        let e = n - 1;
        let sign = if e >= 0 { "+" } else { "-" };
        if k == 1 {
            format!("{digits}e{sign}{}", e.abs())
        } else {
            format!("{}.{}e{sign}{}", &digits[..1], &digits[1..], e.abs())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn roundtrip(json: &str) -> String {
        let mut out = String::new();
        write(&serde_json::from_str(json).unwrap(), &mut out);
        out
    }

    #[test]
    fn keys_follow_javascript_property_order() {
        assert_eq!(
            roundtrip(r#"{"b":1,"10":2,"a":3,"2":4,"01":5,"4294967295":6,"-1":7,"b":8}"#),
            r#"{"2":4,"10":2,"b":8,"a":3,"01":5,"4294967295":6,"-1":7}"#
        );
        let (one, two) = (JsValue::Number(1.0), JsValue::Number(2.0));
        let mut out = String::new();
        write_object([("z", &one), ("3", &two)], &mut out);
        assert_eq!(out, r#"{"3":2,"z":1}"#);
    }

    #[test]
    fn numbers_and_strings_print_as_javascript_does() {
        for (n, js) in [
            (1.0, "1"),
            (-0.0, "0"),
            (100.0, "100"),
            (0.1, "0.1"),
            (-12.25, "-12.25"),
            (1e21, "1e+21"),
            (1.5e-7, "1.5e-7"),
            (1e-7, "1e-7"),
            (0.0001, "0.0001"),
            (0.000001, "0.000001"),
            (123456789012345678000.0, "123456789012345680000"),
            (33.333333333333336, "33.333333333333336"),
            (2.5e300, "2.5e+300"),
        ] {
            assert_eq!(number(n), js, "{n}");
        }
        assert_eq!(roundtrip(r#""q\"é\n/\u0001""#), "\"q\\\"é\\n/\\u0001\"");
        assert_eq!(roundtrip("[1.0,null,true]"), "[1,null,true]");
    }
}
