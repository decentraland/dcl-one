use axum::http::HeaderMap;

pub const IMMUTABLE_CACHE_CONTROL: &str = "public,max-age=31536000,s-maxage=31536000,immutable";

pub fn to_etag(hash: &str) -> String {
    format!("\"{}\"", hash)
}

pub fn check_not_modified(headers: &HeaderMap, hash: &str) -> Option<Vec<(&'static str, String)>> {
    let etag = to_etag(hash);
    let if_none_match = headers.get("if-none-match")?.to_str().ok()?;

    let not_modified_headers = vec![
        ("ETag", etag.clone()),
        ("Cache-Control", IMMUTABLE_CACHE_CONTROL.to_string()),
        ("Access-Control-Expose-Headers", "ETag".to_string()),
    ];

    if if_none_match == "*" {
        return Some(not_modified_headers);
    }

    let tags: Vec<String> = if_none_match
        .split(',')
        .map(|t| t.trim().trim_start_matches("W/").to_string())
        .collect();

    if tags.contains(&etag) {
        return Some(not_modified_headers);
    }

    None
}

#[derive(Debug, Clone)]
pub enum ParsedRange {
    Range { start: u64, end: u64 },
    Unsatisfiable,
}

/// A `Range` position as upstream's `^bytes=(\d*)-(\d*)$` and `parseInt` read
/// it: ASCII digits only (no sign), and a number past `u64` saturates, as a
/// JavaScript number past 2^53 stays larger than any file.
fn range_number(digits: &str) -> Option<u64> {
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    Some(digits.parse().unwrap_or(u64::MAX))
}

pub fn parse_range_header(
    range_header: Option<&str>,
    total_size: Option<u64>,
) -> Option<ParsedRange> {
    let header = range_header?;
    let total = total_size?;

    if let Some(rest) = header.strip_prefix("bytes=") {
        if let Some(suffix) = rest.strip_prefix('-') {
            let suffix_len = range_number(suffix)?;
            if suffix_len == 0 || total == 0 {
                return Some(ParsedRange::Unsatisfiable);
            }
            let start = total.saturating_sub(suffix_len);
            return Some(ParsedRange::Range {
                start,
                end: total - 1,
            });
        }

        let parts: Vec<&str> = rest.splitn(2, '-').collect();
        if parts.len() == 2 {
            let start = range_number(parts[0])?;
            let end: Option<u64> = if parts[1].is_empty() {
                None
            } else {
                Some(range_number(parts[1])?)
            };

            // `start >= total` also covers an empty file.
            if start >= total || end.is_some_and(|end| start > end) {
                return Some(ParsedRange::Unsatisfiable);
            }

            return Some(ParsedRange::Range {
                start,
                end: end.map_or(total - 1, |end| end.min(total - 1)),
            });
        }
    }

    None
}

pub fn content_file_headers(hash: &str, size: Option<u64>) -> Vec<(&'static str, String)> {
    let mut headers = vec![
        ("Content-Type", "application/octet-stream".to_string()),
        ("ETag", to_etag(hash)),
        (
            "Access-Control-Expose-Headers",
            "ETag, Content-Range, Accept-Ranges, Content-Length".to_string(),
        ),
        ("Accept-Ranges", "bytes".to_string()),
        ("Cache-Control", IMMUTABLE_CACHE_CONTROL.to_string()),
    ];

    if let Some(s) = size {
        headers.push(("Content-Length", s.to_string()));
    }

    headers
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntityField {
    Content,
    Pointers,
    Metadata,
}

impl EntityField {
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_uppercase().as_str() {
            "CONTENT" => Some(EntityField::Content),
            "POINTERS" => Some(EntityField::Pointers),
            "METADATA" => Some(EntityField::Metadata),
            _ => None,
        }
    }
}

/// An entity as `GET /entities/{type}` answers it: upstream's `maskEntity`
/// builds `{ version, id, type, timestamp, pointers, content, metadata }`, and
/// a struct serializes its fields in that order where a `Map` would sort them.
#[derive(Debug, serde::Serialize)]
pub struct MaskedEntity {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<serde_json::Value>,
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub entity_type: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timestamp: Option<serde_json::Value>,
    pub pointers: serde_json::Value,
    pub content: serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
}

pub fn mask_entity(entity: &serde_json::Value, fields: Option<&[EntityField]>) -> MaskedEntity {
    let field = |key: &str| entity.get(key).cloned();
    let include = |f: EntityField| fields.is_none_or(|fields| fields.contains(&f));
    let array_or_empty = |f: EntityField, key: &str| {
        include(f)
            .then(|| field(key))
            .flatten()
            .unwrap_or(serde_json::Value::Array(vec![]))
    };
    MaskedEntity {
        version: field("version"),
        id: field("id"),
        entity_type: field("type"),
        timestamp: field("timestamp"),
        pointers: array_or_empty(EntityField::Pointers, "pointers"),
        content: array_or_empty(EntityField::Content, "content"),
        metadata: include(EntityField::Metadata)
            .then(|| field("metadata"))
            .flatten(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn etag_format() {
        assert_eq!(to_etag("abc123"), "\"abc123\"");
    }

    #[test]
    fn parse_range_normal() {
        let r = parse_range_header(Some("bytes=0-499"), Some(1000));
        match r {
            Some(ParsedRange::Range { start, end }) => {
                assert_eq!(start, 0);
                assert_eq!(end, 499);
            }
            _ => panic!("Expected Range"),
        }
    }

    #[test]
    fn parse_range_on_empty_file_is_unsatisfiable() {
        for header in ["bytes=0-", "bytes=0-0", "bytes=-1"] {
            assert!(matches!(
                parse_range_header(Some(header), Some(0)),
                Some(ParsedRange::Unsatisfiable)
            ));
        }
    }

    #[test]
    fn parse_range_on_empty_file_ignores_a_malformed_header() {
        for header in ["bytes=abc", "bytes=0-1,2-3", "bytes=-1,2", "items=0-1"] {
            assert!(
                parse_range_header(Some(header), Some(0)).is_none(),
                "{header}"
            );
        }
    }

    /// `^bytes=(\d*)-(\d*)$` then `parseInt` (worlds-content-server
    /// `content-file-handler.ts` `parseRangeHeader`): a sign is not a digit, and an
    /// overflowing start is past the end while an overflowing end is clamped.
    #[test]
    fn parse_range_reads_digits_only_and_saturates() {
        for header in ["bytes=+5-", "bytes=5-+9", "bytes=-+5", "bytes= 5-9"] {
            assert!(
                parse_range_header(Some(header), Some(100)).is_none(),
                "{header}"
            );
        }
        let huge = "99999999999999999999999";
        assert!(matches!(
            parse_range_header(Some(&format!("bytes={huge}-")), Some(100)),
            Some(ParsedRange::Unsatisfiable)
        ));
        assert!(matches!(
            parse_range_header(Some(&format!("bytes=10-{huge}")), Some(100)),
            Some(ParsedRange::Range { start: 10, end: 99 })
        ));
        assert!(matches!(
            parse_range_header(Some(&format!("bytes=-{huge}")), Some(100)),
            Some(ParsedRange::Range { start: 0, end: 99 })
        ));
    }

    #[test]
    fn parse_range_suffix() {
        let r = parse_range_header(Some("bytes=-200"), Some(1000));
        match r {
            Some(ParsedRange::Range { start, end }) => {
                assert_eq!(start, 800);
                assert_eq!(end, 999);
            }
            _ => panic!("Expected Range"),
        }
    }

    #[test]
    fn parse_range_unsatisfiable() {
        let r = parse_range_header(Some("bytes=1000-2000"), Some(500));
        assert!(matches!(r, Some(ParsedRange::Unsatisfiable)));
    }

    #[test]
    fn mask_keeps_identity_and_blanks_unselected_arrays() {
        let entity = serde_json::json!({
            "version": "v3", "id": "bafy", "type": "scene", "timestamp": 1,
            "pointers": ["0,0"], "content": [{"file": "a", "hash": "b"}], "metadata": {"k": 1}
        });
        let masked =
            serde_json::to_value(mask_entity(&entity, Some(&[EntityField::Metadata]))).unwrap();
        assert_eq!(masked["pointers"], serde_json::json!([]));
        assert_eq!(masked["content"], serde_json::json!([]));
        assert_eq!(masked["metadata"], serde_json::json!({"k": 1}));
        assert_eq!(
            serde_json::to_value(mask_entity(&entity, None)).unwrap(),
            entity
        );
    }

    #[test]
    fn masked_keys_follow_upstream_mask_entity_order() {
        // decentraland/catalyst:src/controllers/handlers/get-entities-handler.ts:94; captured
        // in conformance/content-lambdas/captures/content-reads/CR-16-trailing-slash-entities.json.
        let entity = serde_json::json!({
            "content": [], "id": "bafy", "metadata": {"k": 1}, "pointers": ["0,0"],
            "timestamp": 1, "type": "scene", "version": "v3"
        });
        let bytes = serde_json::to_string(&mask_entity(&entity, None)).unwrap();
        assert_eq!(
            bytes,
            r#"{"version":"v3","id":"bafy","type":"scene","timestamp":1,"pointers":["0,0"],"content":[],"metadata":{"k":1}}"#
        );
    }
}
