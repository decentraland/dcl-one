//! The fixture file format: `{description, request{method,path,query,headers,body},
//! response{status,headers,body_json|body_bytes_b64}, captured_from, captured_at,
//! volatile_paths}`, the sentinel scrub of a volatile path on both sides, and the
//! header rules every replay shares (the headers never compared, the media type
//! without its parameters).

use serde::de::{Deserializer, MapAccess, Visitor};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Fixture {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
    pub request: RecordedRequest,
    pub response: RecordedResponse,
    #[serde(default)]
    pub captured_from: String,
    #[serde(default)]
    pub captured_at: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub volatile_paths: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordedRequest {
    pub method: String,
    pub path: String,
    #[serde(
        default,
        skip_serializing_if = "BTreeMap::is_empty",
        deserialize_with = "deserialize_query"
    )]
    pub query: BTreeMap<String, Vec<String>>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub headers: BTreeMap<String, String>,
    #[serde(default)]
    pub body: Option<serde_json::Value>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum StringOrSeq {
    One(String),
    Many(Vec<String>),
}

impl From<StringOrSeq> for Vec<String> {
    fn from(v: StringOrSeq) -> Self {
        match v {
            StringOrSeq::One(s) => vec![s],
            StringOrSeq::Many(v) => v,
        }
    }
}

fn deserialize_query<'de, D>(deserializer: D) -> Result<BTreeMap<String, Vec<String>>, D::Error>
where
    D: Deserializer<'de>,
{
    struct QueryVisitor;

    impl<'de> Visitor<'de> for QueryVisitor {
        type Value = BTreeMap<String, Vec<String>>;

        fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
            f.write_str("a map of query params where each value is a string or an array of strings")
        }

        fn visit_map<M>(self, mut access: M) -> Result<Self::Value, M::Error>
        where
            M: MapAccess<'de>,
        {
            let mut out = BTreeMap::new();
            while let Some((key, value)) = access.next_entry::<String, StringOrSeq>()? {
                out.insert(key, value.into());
            }
            Ok(out)
        }
    }

    deserializer.deserialize_map(QueryVisitor)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordedResponse {
    pub status: u16,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub headers: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub body_json: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub body_bytes_b64: Option<String>,
}

impl RecordedResponse {
    pub fn is_json(&self) -> bool {
        self.body_json.is_some()
    }
}

/// Response headers no replay compares: set per request or per process.
pub const VOLATILE_HEADERS: &[&str] = &["date", "server", "x-request-id", "x-trace-id", "etag"];

/// A `content-type` value without its parameters, lowercased: the replay
/// compares media types, not charsets.
pub fn media_type(value: &str) -> String {
    value
        .split(';')
        .next()
        .unwrap_or(value)
        .trim()
        .to_ascii_lowercase()
}

/// What a volatile leaf becomes on both sides of the diff. A baseline leaf
/// recorded as this value (the old corpus's hand-authored fixtures) is a mask
/// the differ matches against any candidate leaf, like a capture redaction.
pub const VOLATILE_SENTINEL: &str = "<<VOLATILE>>";

pub fn scrub_volatile_path(value: &mut serde_json::Value, path: &str) {
    if is_order_path(path) {
        return;
    }
    let parts: Vec<&str> = path.split('.').collect();
    scrub_recursive(value, &parts, VOLATILE_SENTINEL);
}

/// `path[]` (or `[]` alone for a top-level array) declares the ORDER of that
/// array volatile: both sides are sorted by canonical JSON before the diff.
pub fn is_order_path(path: &str) -> bool {
    path.ends_with("[]")
}

/// Every volatile path applied to one body: the leaf scrubs first, then the
/// order normalizations (so a live leaf never decides the sort).
pub fn apply_volatile_paths(value: &mut serde_json::Value, paths: &[String]) {
    for p in paths.iter().filter(|p| !is_order_path(p)) {
        scrub_volatile_path(value, p);
    }
    for p in paths.iter().filter(|p| is_order_path(p)) {
        let base = &p[..p.len() - 2];
        let parts: Vec<&str> = if base.is_empty() {
            Vec::new()
        } else {
            base.split('.').collect()
        };
        sort_recursive(value, &parts);
    }
}

fn sort_recursive(value: &mut serde_json::Value, parts: &[&str]) {
    match value {
        serde_json::Value::Array(arr) if parts.is_empty() => {
            arr.sort_by_cached_key(canonical);
        }
        serde_json::Value::Array(arr) => {
            for child in arr.iter_mut() {
                sort_recursive(child, parts);
            }
        }
        serde_json::Value::Object(map) if !parts.is_empty() => {
            if let Some(child) = map.get_mut(parts[0]) {
                sort_recursive(child, &parts[1..]);
            }
        }
        _ => {}
    }
}

/// A key-sorted serialization, so two objects with the same entries sort equal
/// whatever their key order.
pub fn canonical(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::Object(map) => {
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort();
            let inner: Vec<String> = keys
                .iter()
                .map(|k| {
                    format!(
                        "{}:{}",
                        serde_json::to_string(k).unwrap_or_default(),
                        canonical(&map[*k])
                    )
                })
                .collect();
            format!("{{{}}}", inner.join(","))
        }
        serde_json::Value::Array(arr) => format!(
            "[{}]",
            arr.iter().map(canonical).collect::<Vec<_>>().join(",")
        ),
        other => other.to_string(),
    }
}

fn scrub_recursive(value: &mut serde_json::Value, parts: &[&str], sentinel: &str) {
    if parts.is_empty() {
        *value = serde_json::Value::String(sentinel.to_string());
        return;
    }
    let (head, tail) = (parts[0], &parts[1..]);
    match value {
        serde_json::Value::Object(map) => {
            if let Some(child) = map.get_mut(head) {
                scrub_recursive(child, tail, sentinel);
            }
        }
        serde_json::Value::Array(arr) => {
            for child in arr.iter_mut() {
                scrub_recursive(child, parts, sentinel);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn scrub_simple_path() {
        let mut v = json!({"a": {"b": "secret"}});
        scrub_volatile_path(&mut v, "a.b");
        assert_eq!(v, json!({"a": {"b": "<<VOLATILE>>"}}));
    }

    #[test]
    fn scrub_missing_path_is_noop() {
        let mut v = json!({"a": 1});
        scrub_volatile_path(&mut v, "x.y.z");
        assert_eq!(v, json!({"a": 1}));
    }

    #[test]
    fn scrub_walks_into_arrays() {
        let mut v = json!({"items": [{"url": "a"}, {"url": "b"}]});
        scrub_volatile_path(&mut v, "items.url");
        assert_eq!(
            v,
            json!({"items": [{"url": "<<VOLATILE>>"}, {"url": "<<VOLATILE>>"}]})
        );
    }

    #[test]
    fn order_path_sorts_both_sides_after_the_leaf_scrubs() {
        let mut a = json!({"data": [{"id": "b", "n": 2}, {"id": "a", "n": 9}], "total": 2});
        let mut b = json!({"data": [{"id": "a", "n": 1}, {"id": "b", "n": 2}], "total": 2});
        let paths = vec!["data.n".to_string(), "data[]".to_string()];
        apply_volatile_paths(&mut a, &paths);
        apply_volatile_paths(&mut b, &paths);
        assert_eq!(a, b);
        let mut root = json!([3, 1, 2]);
        apply_volatile_paths(&mut root, &["[]".to_string()]);
        assert_eq!(root, json!([1, 2, 3]));
        let mut untouched = json!({"data": "x"});
        apply_volatile_paths(&mut untouched, &["data[]".to_string()]);
        assert_eq!(untouched, json!({"data": "x"}));
    }

    #[test]
    fn media_type_drops_the_charset_and_case() {
        assert_eq!(
            media_type("application/json; charset=utf-8"),
            "application/json"
        );
        assert_eq!(media_type("Text/Plain"), "text/plain");
    }

    #[test]
    fn query_values_take_a_string_or_a_list() {
        let f: Fixture = serde_json::from_str(
            r#"{"request":{"method":"GET","path":"/x","query":{"a":"1","b":["2","3"]}},
                "response":{"status":200,"body_json":{}},"captured_from":"u","captured_at":"t"}"#,
        )
        .unwrap();
        assert_eq!(f.request.query["a"], vec!["1"]);
        assert_eq!(f.request.query["b"], vec!["2", "3"]);
        assert!(f.response.is_json());
    }
}
