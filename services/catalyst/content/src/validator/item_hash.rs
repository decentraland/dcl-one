//! The hashes an item's on-chain `contentHash` is compared with: the CIDv0
//! and CIDv1 of `JSON.stringify({ content, metadata })` (ADR-32), where
//! `content` is the entity's `{ key, hash }` pairs sorted by hash then key and
//! `metadata` is the entity file's metadata as JavaScript re-serializes it
//! (keys in document order, array-index keys first, JS number formatting).

use std::cmp::Ordering;
use std::fmt;

use serde::de::{Deserialize, Deserializer, MapAccess, SeqAccess, Visitor};

use super::types::{ContentMapping, Deployment};

/// A JSON value as `JSON.parse` holds it.
enum JsValue {
    Null,
    Bool(bool),
    Number(f64),
    String(String),
    Array(Vec<JsValue>),
    Object(Vec<(String, JsValue)>),
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
                let mut out: Vec<(String, JsValue)> = Vec::new();
                while let Some((k, v)) = map.next_entry::<String, JsValue>()? {
                    // A repeated key keeps its first position and takes the last value.
                    match out.iter_mut().find(|(key, _)| *key == k) {
                        Some(slot) => slot.1 = v,
                        None => out.push((k, v)),
                    }
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

/// `Number.prototype.toString` for a finite double.
fn js_number(n: f64) -> String {
    if n == 0.0 {
        return "0".to_string();
    }
    let abs = n.abs();
    if (1e-6..1e21).contains(&abs) {
        format!("{n}")
    } else {
        let s = format!("{n:e}");
        match s.split_once('e') {
            Some((mantissa, exp)) if !exp.starts_with('-') => format!("{mantissa}e+{exp}"),
            _ => s,
        }
    }
}

fn js_string(s: &str, out: &mut String) {
    out.push_str(&serde_json::to_string(s).unwrap_or_else(|_| "\"\"".to_string()));
}

fn stringify(v: &JsValue, out: &mut String) {
    match v {
        JsValue::Null => out.push_str("null"),
        JsValue::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
        JsValue::Number(n) if n.is_finite() => out.push_str(&js_number(*n)),
        JsValue::Number(_) => out.push_str("null"),
        JsValue::String(s) => js_string(s, out),
        JsValue::Array(items) => {
            out.push('[');
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                stringify(item, out);
            }
            out.push(']');
        }
        JsValue::Object(entries) => {
            let mut indexed: Vec<(u32, &(String, JsValue))> = entries
                .iter()
                .filter_map(|e| array_index(&e.0).map(|i| (i, e)))
                .collect();
            indexed.sort_by_key(|(i, _)| *i);
            let ordered = indexed
                .into_iter()
                .map(|(_, e)| e)
                .chain(entries.iter().filter(|e| array_index(&e.0).is_none()));
            out.push('{');
            for (i, (k, v)) in ordered.enumerate() {
                if i > 0 {
                    out.push(',');
                }
                js_string(k, out);
                out.push(':');
                stringify(v, out);
            }
            out.push('}');
        }
    }
}

/// JS `<` / `>` on strings: UTF-16 code unit order.
fn js_cmp(a: &str, b: &str) -> Ordering {
    a.encode_utf16().cmp(b.encode_utf16())
}

/// The bytes ADR-32 hashes, from the content mappings and the entity file.
fn hashed_payload(content: &[ContentMapping], entity_file: &[u8]) -> Option<Vec<u8>> {
    let JsValue::Object(fields) = serde_json::from_slice::<JsValue>(entity_file).ok()? else {
        return None;
    };
    let mut sorted: Vec<&ContentMapping> = content.iter().collect();
    sorted.sort_by(|a, b| js_cmp(&a.hash, &b.hash).then_with(|| js_cmp(&a.file, &b.file)));
    let mut out = String::from("{\"content\":[");
    for (i, c) in sorted.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        out.push_str("{\"key\":");
        js_string(&c.file, &mut out);
        out.push_str(",\"hash\":");
        js_string(&c.hash, &mut out);
        out.push('}');
    }
    out.push(']');
    if let Some((_, metadata)) = fields.iter().find(|(k, _)| k == "metadata") {
        out.push_str(",\"metadata\":");
        stringify(metadata, &mut out);
    }
    out.push('}');
    Some(out.into_bytes())
}

/// The CIDv0 and CIDv1 an item's `contentHash` may equal; `None` when the
/// entity file is not among the deployment's files or does not parse.
pub(super) fn deployment_hashes(deployment: &Deployment<'_>) -> Option<[String; 2]> {
    let file = deployment.files.get(&deployment.entity.id)?;
    let payload = hashed_payload(&deployment.entity.content, file)?;
    Some([
        crate::auth::hash_bytes(&payload),
        crate::auth::hash_bytes_v1(&payload),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mapping(file: &str, hash: &str) -> ContentMapping {
        ContentMapping {
            file: file.to_string(),
            hash: hash.to_string(),
        }
    }

    fn payload(content: &[ContentMapping], file: &str) -> String {
        String::from_utf8(hashed_payload(content, file.as_bytes()).unwrap()).unwrap()
    }

    #[test]
    fn content_is_sorted_by_hash_then_key_and_metadata_keeps_document_order() {
        let content = [
            mapping("b.png", "bafy2"),
            mapping("z.glb", "bafy1"),
            mapping("a.png", "bafy2"),
        ];
        let file = r#"{"type":"wearable","metadata":{"name":"hat","id":"x","data":{"tags":[],"2":1,"1":true}},"content":[]}"#;
        assert_eq!(
            payload(&content, file),
            r#"{"content":[{"key":"z.glb","hash":"bafy1"},{"key":"a.png","hash":"bafy2"},{"key":"b.png","hash":"bafy2"}],"metadata":{"name":"hat","id":"x","data":{"1":true,"2":1,"tags":[]}}}"#
        );
    }

    #[test]
    fn numbers_and_strings_print_as_javascript_does() {
        let file = r#"{"metadata":{"a":1.0,"b":-0.0,"c":1e21,"d":1.5e-7,"e":0.000001,"f":100,"g":"q\"é\n/","a":2}}"#;
        assert_eq!(
            payload(&[], file),
            "{\"content\":[],\"metadata\":{\"a\":2,\"b\":0,\"c\":1e+21,\"d\":1.5e-7,\"e\":0.000001,\"f\":100,\"g\":\"q\\\"é\\n/\"}}"
        );
    }

    #[test]
    fn missing_metadata_is_omitted_and_null_is_kept() {
        assert_eq!(payload(&[], r#"{"type":"wearable"}"#), r#"{"content":[]}"#);
        assert_eq!(
            payload(&[], r#"{"metadata":null}"#),
            r#"{"content":[],"metadata":null}"#
        );
    }
}
