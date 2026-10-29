//! The Sentry ingest protocol: `POST /api/{project}/envelope` (one row per
//! envelope item, the first [`MAX_ITEMS_PER_REQUEST`] of them, the item
//! header's `type` as the kind; an envelope with no parseable item is kept
//! as one `envelope` row holding its header line, its decoded size and the
//! first [`MAX_RAW_ENVELOPE_BYTES`] of the text, never the whole body) and
//! the legacy `POST /api/{project}/store` (one `event` row). Both answer
//! `{id}` whatever happened, as a Sentry relay does; the admission
//! ([`super::IngestControl`]) decides what is stored.

use axum::Json;
use axum::body::Bytes;
use axum::extract::{Path, State};
use axum::http::HeaderMap;
use serde_json::{Value, json};

use super::ingest::Row;
use super::{MAX_ITEMS_PER_REQUEST, MAX_RAW_ENVELOPE_BYTES, SharedState, decode_body};

fn row(project: &str, kind: String, body: Value) -> Row {
    Row {
        source: "sentry",
        project: project.to_string(),
        kind,
        body,
        invalid_reason: None,
    }
}

/// The `envelope` fallback row's body: the header document, the decoded
/// size and a bounded prefix of the text (cut on a char boundary).
pub(crate) fn raw_envelope_body(header: Value, text: &str) -> Value {
    let mut cut = MAX_RAW_ENVELOPE_BYTES.min(text.len());
    while cut > 0 && !text.is_char_boundary(cut) {
        cut -= 1;
    }
    json!({
        "header": header,
        "raw": &text[..cut],
        "raw_bytes": text.len(),
        "truncated": cut < text.len(),
    })
}

pub async fn envelope(
    State(state): State<SharedState>,
    Path(project): Path<String>,
    headers: HeaderMap,
    body: Bytes,
) -> Json<Value> {
    let raw = decode_body(&headers, body);
    if !state.ingest.admit_bytes(raw.len()) {
        return Json(json!({ "id": "" }));
    }
    let text = String::from_utf8_lossy(&raw);
    let mut lines = text.split('\n').filter(|l| !l.trim().is_empty());

    let envelope_header: Value = lines
        .next()
        .and_then(|l| serde_json::from_str(l).ok())
        .unwrap_or_else(|| json!({}));
    let event_id = envelope_header
        .get("event_id")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let mut items: Vec<(String, Value)> = Vec::new();
    let mut pending_kind: Option<String> = None;
    for line in lines {
        if items.len() >= MAX_ITEMS_PER_REQUEST {
            break;
        }
        let value: Value = match serde_json::from_str(line) {
            Ok(v) => v,
            Err(_) => continue,
        };
        match pending_kind.take() {
            None => {
                pending_kind = Some(
                    value
                        .get("type")
                        .and_then(|v| v.as_str())
                        .unwrap_or("unknown")
                        .to_string(),
                );
            }
            Some(kind) => {
                items.push((kind, value));
            }
        }
    }

    if items.is_empty() {
        if state.ingest.admit(&project) {
            state.writer.push(row(
                &project,
                "envelope".into(),
                raw_envelope_body(envelope_header, &text),
            ));
        }
        return Json(json!({ "id": event_id }));
    }

    let admitted = state.ingest.admit_n(&project, items.len());
    let rows: Vec<Row> = items
        .into_iter()
        .take(admitted)
        .map(|(kind, value)| row(&project, kind, value))
        .collect();
    state.writer.push_all(rows);

    Json(json!({ "id": event_id }))
}

pub async fn store(
    State(state): State<SharedState>,
    Path(project): Path<String>,
    headers: HeaderMap,
    body: Bytes,
) -> Json<Value> {
    let raw = decode_body(&headers, body);
    if !state.ingest.admit_bytes(raw.len()) || !state.ingest.admit(&project) {
        return Json(json!({ "id": "" }));
    }
    let event: Value = serde_json::from_slice(&raw).unwrap_or_else(|_| json!({}));
    let event_id = event
        .get("event_id")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    state.writer.push(row(&project, "event".into(), event));
    Json(json!({ "id": event_id }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_raw_envelope_fallback_keeps_the_header_and_a_bounded_prefix() {
        let text = format!("{{\"event_id\":\"e\"}}\n{}", "x".repeat(20 * 1024));
        let body = raw_envelope_body(json!({ "event_id": "e" }), &text);
        assert_eq!(body["header"]["event_id"], "e");
        assert_eq!(body["raw"].as_str().unwrap().len(), MAX_RAW_ENVELOPE_BYTES);
        assert_eq!(body["raw_bytes"], text.len());
        assert_eq!(body["truncated"], true);
        let short = raw_envelope_body(json!({}), "é\n");
        assert_eq!(short["raw"], "é\n");
        assert_eq!(short["truncated"], false);
        let boundary = "é".repeat(MAX_RAW_ENVELOPE_BYTES);
        let cut = raw_envelope_body(json!({}), &boundary);
        assert!(
            cut["raw"].as_str().unwrap().len() <= MAX_RAW_ENVELOPE_BYTES,
            "cut on a char boundary, never inside one"
        );
    }
}
