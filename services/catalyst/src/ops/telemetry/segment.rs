//! The Segment HTTP API: `POST /v1/{batch,import}` (one row per element of
//! `batch`, the first [`MAX_ITEMS_PER_REQUEST`] of them) and `POST
//! /v1/{track,identify,page,screen,group,alias}` (one row), every row
//! through the ONE bounded writer. The project is the write key: the
//! `Authorization: Basic` user, else the body's `writeKey`. Every call
//! answers `{success: true}`; the admission ([`super::IngestControl`]: the
//! bytes ceiling on the decoded body, the project's daily quota, the rows
//! ceiling) and the contract decide what is stored and what is flagged.

use axum::Json;
use axum::body::Bytes;
use axum::extract::State;
use axum::http::HeaderMap;
use serde_json::{Value, json};

use super::ingest::Row;
use super::{MAX_ITEMS_PER_REQUEST, SharedState, contract, decode_body};

fn base64_decode(input: &str) -> Option<Vec<u8>> {
    fn sextet(c: u8) -> Option<u32> {
        match c {
            b'A'..=b'Z' => Some((c - b'A') as u32),
            b'a'..=b'z' => Some((c - b'a' + 26) as u32),
            b'0'..=b'9' => Some((c - b'0' + 52) as u32),
            b'+' => Some(62),
            b'/' => Some(63),
            _ => None,
        }
    }
    let mut out = Vec::new();
    let mut acc = 0u32;
    let mut bits = 0u32;
    for &c in input.as_bytes() {
        if c == b'=' || c == b'\r' || c == b'\n' || c == b' ' || c == b'\t' {
            continue;
        }
        acc = (acc << 6) | sextet(c)?;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
        }
    }
    Some(out)
}

pub(crate) fn write_key(headers: &HeaderMap, body: &Value) -> String {
    if let Some(auth) = headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
    {
        let cred = auth.trim_start_matches("Basic ").trim();
        if let Some(decoded) = base64_decode(cred)
            && let Ok(text) = String::from_utf8(decoded)
        {
            let user = text.split(':').next().unwrap_or("");
            if !user.is_empty() {
                return user.to_string();
            }
        }
        return cred.to_string();
    }
    body.get("writeKey")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string()
}

fn row(state: &SharedState, key: &str, event: Value) -> Row {
    let kind = event
        .get("type")
        .and_then(|v| v.as_str())
        .unwrap_or("track")
        .to_string();

    let in_scope = contract::project_in_scope(state.settings.contract_projects.as_ref(), key);
    let invalid_reason: Option<String> = match &state.contract {
        Some(contract) if in_scope => match event.get("event").and_then(|v| v.as_str()) {
            Some(name) if !name.is_empty() => {
                let null = Value::Null;
                let props = event.get("properties").unwrap_or(&null);
                contract::validate_event(contract, name, props)
            }
            _ => None,
        },
        _ => None,
    };

    Row {
        source: "segment",
        project: key.to_string(),
        kind,
        body: event,
        invalid_reason,
    }
}

const SUCCESS: &str = "{\"success\":true}";

fn success() -> Json<Value> {
    Json(serde_json::from_str(SUCCESS).expect("static"))
}

pub async fn batch(
    State(state): State<SharedState>,
    headers: HeaderMap,
    body: Bytes,
) -> Json<Value> {
    let raw = decode_body(&headers, body);
    if !state.ingest.admit_bytes(raw.len()) {
        return success();
    }
    let mut payload: Value = serde_json::from_slice(&raw).unwrap_or_else(|_| json!({}));
    let key = write_key(&headers, &payload);

    let batch = match payload.get_mut("batch") {
        Some(v @ Value::Array(_)) => Some(std::mem::take(v)),
        _ => None,
    };
    if let Some(Value::Array(mut batch)) = batch {
        batch.truncate(MAX_ITEMS_PER_REQUEST);
        let admitted = state.ingest.admit_n(&key, batch.len());
        let rows: Vec<Row> = batch
            .into_iter()
            .take(admitted)
            .map(|event| row(&state, &key, event))
            .collect();
        state.writer.push_all(rows);
    } else if state.ingest.admit(&key) {
        state.writer.push(row(&state, &key, payload));
    }
    success()
}

pub async fn single(
    State(state): State<SharedState>,
    headers: HeaderMap,
    body: Bytes,
) -> Json<Value> {
    let raw = decode_body(&headers, body);
    if !state.ingest.admit_bytes(raw.len()) {
        return success();
    }
    let payload: Value = serde_json::from_slice(&raw).unwrap_or_else(|_| json!({}));
    let key = write_key(&headers, &payload);
    if state.ingest.admit(&key) {
        state.writer.push(row(&state, &key, payload));
    }
    success()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_write_key_is_the_basic_user_then_the_body_key() {
        let mut h = HeaderMap::new();
        h.insert("authorization", "Basic d2stMTIzOg==".parse().unwrap());
        assert_eq!(write_key(&h, &json!({ "writeKey": "body" })), "wk-123");
        assert_eq!(
            write_key(&HeaderMap::new(), &json!({ "writeKey": "body" })),
            "body"
        );
        assert_eq!(write_key(&HeaderMap::new(), &json!({})), "");
        let mut raw = HeaderMap::new();
        raw.insert("authorization", "Basic not-base64!".parse().unwrap());
        assert_eq!(
            write_key(&raw, &json!({})),
            "not-base64!",
            "an undecodable credential is kept verbatim"
        );
    }
}
