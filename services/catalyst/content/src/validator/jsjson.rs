//! Entity JSON as JavaScript holds and re-serializes it, for the hashes the
//! reference computes over `JSON.stringify` output (ADR-32 item hashes,
//! third-party merkle leaves): `JSON.parse` keeps a repeated key at its first
//! position with its last value, `JSON.stringify` writes integer-like keys
//! first in ascending order and then the rest in insertion order, and numbers
//! print as `Number.prototype.toString`. `serde_json::Value` without
//! `preserve_order` sorts keys, so it cannot stand in.

use std::{collections::BTreeMap, fmt};

use serde::{Serialize, Serializer, ser::SerializeMap};

use serde::de::{Deserialize, Deserializer, MapAccess, SeqAccess, Visitor};

/// Ordered JSON. Strict parsing retains exact integers; legacy hashing uses binary64.
#[derive(Debug, Clone, PartialEq)]
pub enum JsValue<const LEGACY: bool = false> {
    Null,
    Bool(bool),
    Signed(i64),
    Unsigned(u64),
    Number(f64),
    String(String),
    Array(Vec<JsValue<LEGACY>>),
    Object(Vec<(String, JsValue<LEGACY>)>),
}

pub(crate) type LegacyValue = JsValue<true>;

impl<const LEGACY: bool> JsValue<LEGACY> {
    pub fn as_object(&self) -> Option<&[(String, Self)]> {
        match self {
            JsValue::Object(entries) => Some(entries),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Self::String(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_array(&self) -> Option<&[Self]> {
        match self {
            Self::Array(a) => Some(a),
            _ => None,
        }
    }

    pub fn is_null(&self) -> bool {
        matches!(self, Self::Null)
    }

    pub fn get(&self, key: &str) -> Option<&Self> {
        self.as_object()?
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v)
    }
}

impl<'de, const LEGACY: bool> Deserialize<'de> for JsValue<LEGACY> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct V<const LEGACY: bool>;
        impl<'de, const LEGACY: bool> Visitor<'de> for V<LEGACY> {
            type Value = JsValue<LEGACY>;
            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("a JSON value")
            }
            fn visit_unit<E>(self) -> Result<JsValue<LEGACY>, E> {
                Ok(JsValue::Null)
            }
            fn visit_none<E>(self) -> Result<JsValue<LEGACY>, E> {
                Ok(JsValue::Null)
            }
            fn visit_some<D: Deserializer<'de>>(self, d: D) -> Result<JsValue<LEGACY>, D::Error> {
                Deserialize::deserialize(d)
            }
            fn visit_bool<E>(self, v: bool) -> Result<JsValue<LEGACY>, E> {
                Ok(JsValue::Bool(v))
            }
            fn visit_i64<E>(self, v: i64) -> Result<JsValue<LEGACY>, E> {
                Ok(if LEGACY {
                    JsValue::Number(v as f64)
                } else {
                    JsValue::Signed(v)
                })
            }
            fn visit_u64<E>(self, v: u64) -> Result<JsValue<LEGACY>, E> {
                Ok(if LEGACY {
                    JsValue::Number(v as f64)
                } else {
                    JsValue::Unsigned(v)
                })
            }
            fn visit_f64<E>(self, v: f64) -> Result<JsValue<LEGACY>, E> {
                Ok(JsValue::Number(v))
            }
            fn visit_str<E>(self, v: &str) -> Result<JsValue<LEGACY>, E> {
                Ok(JsValue::String(v.to_string()))
            }
            fn visit_string<E>(self, v: String) -> Result<JsValue<LEGACY>, E> {
                Ok(JsValue::String(v))
            }
            fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<JsValue<LEGACY>, A::Error> {
                let mut out = Vec::new();
                while let Some(v) = seq.next_element()? {
                    out.push(v);
                }
                Ok(JsValue::Array(out))
            }
            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<JsValue<LEGACY>, A::Error> {
                let mut out = Vec::new();
                let mut slots = BTreeMap::new();
                while let Some((k, v)) = map.next_entry::<String, JsValue<LEGACY>>()? {
                    let next = out.len();
                    let slot = *slots.entry(k.clone()).or_insert(next);
                    if slot == next {
                        out.push((k, v));
                    } else {
                        out[slot].1 = v;
                    }
                }
                if LEGACY
                    && let [(key, JsValue::String(digits))] = out.as_slice()
                    && key == "$serde_json::private::Number"
                    && let Ok(n) = digits.parse::<f64>()
                {
                    return Ok(JsValue::Number(n));
                }
                Ok(JsValue::Object(out))
            }
        }
        deserializer.deserialize_any(V::<LEGACY>)
    }
}

impl Serialize for JsValue<false> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::Null => serializer.serialize_unit(),
            Self::Bool(v) => serializer.serialize_bool(*v),
            Self::Signed(v) => serializer.serialize_i64(*v),
            Self::Unsigned(v) => serializer.serialize_u64(*v),
            Self::Number(v) => serializer.serialize_f64(*v),
            Self::String(v) => serializer.serialize_str(v),
            Self::Array(v) => v.serialize(serializer),
            Self::Object(entries) => {
                let mut indexed: Vec<_> = entries
                    .iter()
                    .filter_map(|(k, v)| array_index(k).map(|index| (index, k, v)))
                    .collect();
                indexed.sort_unstable_by_key(|(index, _, _)| *index);
                let mut map = serializer.serialize_map(Some(entries.len()))?;
                for (_, key, value) in indexed {
                    map.serialize_entry(key, value)?;
                }
                for (key, value) in entries {
                    if array_index(key).is_none() {
                        map.serialize_entry(key, value)?;
                    }
                }
                map.end()
            }
        }
    }
}

/// The JS array-index value of a property key, if it is one.
fn array_index(key: &str) -> Option<u32> {
    let n: u32 = key.parse().ok()?;
    (n != u32::MAX && n.to_string() == key).then_some(n)
}

/// `JSON.stringify` of `value` appended to `out`.
pub(crate) fn write(value: &LegacyValue, out: &mut String) {
    match value {
        JsValue::Null => out.push_str("null"),
        JsValue::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
        JsValue::Signed(n) => out.push_str(&number(*n as f64)),
        JsValue::Unsigned(n) => out.push_str(&number(*n as f64)),
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

/// `items` (distinct keys) in JS property order, as `Object.keys` lists them:
/// integer-like keys first, ascending, then the rest in insertion order.
pub(super) fn property_order<T>(
    items: impl IntoIterator<Item = T>,
    key: impl Fn(&T) -> &str,
) -> Vec<T> {
    let (mut indexed, rest): (Vec<_>, Vec<_>) = items
        .into_iter()
        .partition(|item| array_index(key(item)).is_some());
    indexed.sort_by_key(|item| array_index(key(item)));
    indexed.extend(rest);
    indexed
}

/// `JSON.stringify` of an object built from `entries` (distinct keys), in JS
/// property order.
pub(super) fn write_object<'a>(
    entries: impl IntoIterator<Item = (&'a str, &'a LegacyValue)>,
    out: &mut String,
) {
    out.push('{');
    for (i, (k, v)) in property_order(entries, |(k, _)| k).into_iter().enumerate() {
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

    #[test]
    fn strict_serialization_retains_exact_integer_boundaries() {
        let input = "[9007199254740991,9007199254740992,9007199254740993,-9223372036854775808,9223372036854775807,18446744073709551615]";
        let value: crate::OrderedJson = serde_json::from_str(input).unwrap();
        assert_eq!(serde_json::to_string(&value).unwrap(), input);
    }

    #[test]
    fn strict_deserialization_preserves_literal_number_marker() {
        let value: JsValue =
            serde_json::from_str(r#"{"$serde_json::private::Number":"12"}"#).unwrap();
        assert!(matches!(value, JsValue::Object(_)));
    }

    #[test]
    fn strict_nested_markers_duplicates_and_index_keys_keep_json_semantics() {
        let marker = r#"{"$serde_json::private::Number":"12"}"#;
        let direct: crate::OrderedJson = serde_json::from_str(marker).unwrap();
        let optional: Option<crate::OrderedJson> = serde_json::from_str(marker).unwrap();
        let vector: Vec<crate::OrderedJson> = serde_json::from_str(&format!("[{marker}]")).unwrap();
        assert_eq!(serde_json::to_string(&direct).unwrap(), marker);
        assert_eq!(serde_json::to_string(&optional).unwrap(), marker);
        assert_eq!(
            serde_json::to_string(&vector).unwrap(),
            format!("[{marker}]")
        );
        let value: crate::OrderedJson = serde_json::from_str(r#"{"z":{"b":true,"a":null},"10":10,"2":2,"01":1,"0":0,"-0":0,"4294967294":4,"4294967295":5,"a":1,"\u0061":2,"z":[{"z":"first","a":"second"}],"__proto__":{"$serde_json::private::Number":"12"},"2":3}"#).unwrap();
        assert_eq!(
            serde_json::to_string(&value).unwrap(),
            r#"{"0":0,"2":3,"10":10,"4294967294":4,"z":[{"z":"first","a":"second"}],"01":1,"-0":0,"4294967295":5,"a":2,"__proto__":{"$serde_json::private::Number":"12"}}"#
        );
    }

    #[test]
    fn strict_parser_retains_existing_overflow_and_surrogate_refusals() {
        for input in [r#"{"v":1e400}"#, r#"{"v":"\ud800"}"#, r#"{"\udfff":0}"#] {
            assert!(serde_json::from_str::<serde_json::Value>(input).is_err());
            assert!(serde_json::from_str::<crate::OrderedJson>(input).is_err());
        }
        let value: crate::OrderedJson =
            serde_json::from_str(r#"["\ud83d\ude00","é\n/\u2028"]"#).unwrap();
        assert_eq!(
            serde_json::to_string(&value).unwrap(),
            "[\"😀\",\"é\\n/\u{2028}\"]"
        );
    }

    #[test]
    fn strict_wide_objects_keep_first_position_with_last_decoded_value() {
        let width = 4096;
        let mut text = String::from("{");
        for i in (0..width).rev() {
            if i != width - 1 {
                text.push(',');
            }
            text.push_str(&format!("\"long-common-prefix-{i:05}\":{i}"));
        }
        text.push_str(",\"long-common-prefix-04095\":-1}");
        let value: crate::OrderedJson = serde_json::from_str(&text).unwrap();
        let encoded = serde_json::to_string(&value).unwrap();
        assert!(
            encoded
                .starts_with("{\"long-common-prefix-04095\":-1,\"long-common-prefix-04094\":4094,")
        );
        assert_eq!(value.as_object().unwrap().len(), width);
    }

    fn roundtrip(json: &str) -> String {
        let mut out = String::new();
        write(
            &serde_json::from_str::<LegacyValue>(json).unwrap(),
            &mut out,
        );
        out
    }

    #[test]
    fn legacy_wide_objects_parse_in_linear_time_keeping_first_position_and_last_value() {
        let width = 100_000;
        let mut text = String::from("{");
        for i in 0..width {
            text.push_str(&format!("\"k{i:06}\":{i},"));
        }
        text.push_str("\"k000000\":-1}");
        let started = std::time::Instant::now();
        let value: LegacyValue = serde_json::from_str(&text).unwrap();
        let elapsed = started.elapsed();
        let entries = value.as_object().unwrap();
        assert_eq!(entries.len(), width);
        assert_eq!(entries[0], ("k000000".to_owned(), JsValue::Number(-1.0)));
        assert_eq!(
            entries[width - 1],
            (
                format!("k{:06}", width - 1),
                JsValue::Number((width - 1) as f64)
            )
        );
        assert!(elapsed < std::time::Duration::from_secs(3), "{elapsed:?}");
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
