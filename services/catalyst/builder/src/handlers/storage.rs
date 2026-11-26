//! `GET`/`HEAD /v1/storage/contents/{hash}` and its `exists` check: an item
//! file by hash, to anyone once published and to its signers before.

use axum::Json;
use axum::body::Body;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, HeaderValue, Method, StatusCode, header};
use axum::response::{IntoResponse, Response};
use serde_json::json;

use crate::SharedState;
use crate::access::{self, FileRead};
use crate::auth::MaybeSigned;
use crate::content::formatters::{ParsedRange, parse_range_header};
use crate::http::ApiError;
use crate::storage::OpenedContent;

const PUBLIC: &str = "public,max-age=31536000,immutable";
const PRIVATE: &str = "private, no-store";
const NO_STORE: &str = "no-store";
const SNIFF_BYTES: u64 = 16;

/// The media type a file's first bytes carry: the binary signatures an item
/// file holds (images, glTF, video, audio, zip), else
/// `application/octet-stream`.
fn detect_content_type(first_bytes: &[u8]) -> &'static str {
    if first_bytes.len() >= 8 {
        if first_bytes.starts_with(&[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]) {
            return "image/png";
        }
        if first_bytes.starts_with(&[0x67, 0x6C, 0x54, 0x46]) {
            return "model/gltf-binary";
        }
        if &first_bytes[4..8] == b"ftyp" {
            return "video/mp4";
        }
    }
    if first_bytes.len() >= 3 && first_bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        return "image/jpeg";
    }
    if first_bytes.starts_with(b"GIF8") {
        return "image/gif";
    }
    if first_bytes.starts_with(b"ID3")
        || (first_bytes.len() >= 2
            && first_bytes[0] == 0xFF
            && matches!(first_bytes[1], 0xFB | 0xFA | 0xF3 | 0xF2 | 0xE3 | 0xE2))
    {
        return "audio/mpeg";
    }
    if first_bytes.starts_with(&[0x50, 0x4B, 0x03, 0x04]) {
        return "application/zip";
    }
    if first_bytes.len() >= 12 && first_bytes.starts_with(b"RIFF") {
        match &first_bytes[8..12] {
            b"WEBP" => return "image/webp",
            b"WAVE" => return "audio/wav",
            _ => {}
        }
    }
    if first_bytes.starts_with(b"OggS") {
        return "audio/ogg";
    }
    if first_bytes.starts_with(&[0x1A, 0x45, 0xDF, 0xA3]) {
        return "video/webm";
    }
    "application/octet-stream"
}

/// The one refusal of both routes, for a private file as for an absent one.
fn refused() -> Response {
    (StatusCode::NOT_FOUND, [(header::CACHE_CONTROL, NO_STORE)]).into_response()
}

/// `GET` and `HEAD /v1/storage/contents/:hash`: a file [`access::read_file`]
/// grants, streamed from the node's blob store, with `Range`. The type comes
/// from the file's bytes, never from its upload; a type that isn't a media
/// type, a document of any kind included, is an attachment, and every answer
/// is `nosniff` and sandboxed. An unsigned answer is cached publicly (a hash names its bytes
/// for good); a signed one is `private, no-store`; a refusal is a 404 kept
/// by no cache.
pub async fn get_storage_content(
    State(state): State<SharedState>,
    MaybeSigned(signer): MaybeSigned,
    Path(hash): Path<String>,
    method: Method,
    headers: HeaderMap,
) -> Response {
    let read = match access::read_file(&state, signer.as_ref(), &hash).await {
        Ok(Some(read)) => read,
        Ok(None) => return refused(),
        Err(error) => return error.into_response(),
    };
    match state.items.open_file(&read).await {
        Ok(Some(opened)) => serve(opened, &read, &method, &headers)
            .await
            .unwrap_or_else(IntoResponse::into_response),
        Ok(None) => refused(),
        Err(error) => error.into_response(),
    }
}

async fn serve(
    mut opened: OpenedContent,
    read: &FileRead,
    method: &Method,
    headers: &HeaderMap,
) -> Result<Response, ApiError> {
    let total = opened.size;
    let internal = |e: crate::storage::StorageError| ApiError::internal(e.to_string());
    let head = opened
        .read_range(0, SNIFF_BYTES - 1)
        .await
        .map_err(internal)?;
    let content_type = detect_content_type(&head);
    let range = headers
        .get(header::RANGE)
        .and_then(|v| v.to_str().ok())
        .filter(|r| !reversed(r));
    let head_only = method == Method::HEAD;
    let mut response = match parse_range_header(range, Some(total)) {
        Some(ParsedRange::Unsatisfiable) => {
            let mut response = (
                StatusCode::RANGE_NOT_SATISFIABLE,
                [(header::CACHE_CONTROL, NO_STORE)],
            )
                .into_response();
            response.headers_mut().insert(
                header::CONTENT_RANGE,
                HeaderValue::from_str(&format!("bytes */{total}")).expect("digits"),
            );
            return Ok(response);
        }
        Some(ParsedRange::Range { start, end }) => {
            let body = if head_only {
                Body::empty()
            } else {
                opened.into_range_body(start, end).await.map_err(internal)?
            };
            let mut response = (StatusCode::PARTIAL_CONTENT, body).into_response();
            let headers = response.headers_mut();
            headers.insert(
                header::CONTENT_RANGE,
                HeaderValue::from_str(&format!("bytes {start}-{end}/{total}")).expect("digits"),
            );
            headers.insert(header::CONTENT_LENGTH, HeaderValue::from(end - start + 1));
            response
        }
        None => {
            let body = if head_only {
                Body::empty()
            } else {
                opened.into_body(head)
            };
            let mut response = Response::new(body);
            response
                .headers_mut()
                .insert(header::CONTENT_LENGTH, HeaderValue::from(total));
            response
        }
    };
    let headers = response.headers_mut();
    headers.insert(header::CONTENT_TYPE, HeaderValue::from_static(content_type));
    if content_type.starts_with("application/") {
        headers.insert(
            header::CONTENT_DISPOSITION,
            HeaderValue::from_static("attachment"),
        );
    }
    headers.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    headers.insert(
        header::CONTENT_SECURITY_POLICY,
        HeaderValue::from_static("sandbox"),
    );
    headers.insert(header::ACCEPT_RANGES, HeaderValue::from_static("bytes"));
    headers.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static(if read.public() { PUBLIC } else { PRIVATE }),
    );
    Ok(response)
}

/// A `bytes=first-last` range with `last` before `first`, which RFC 9110
/// makes invalid, so the request is answered whole.
fn reversed(range: &str) -> bool {
    let bounds = range.strip_prefix("bytes=").and_then(|r| r.split_once('-'));
    match bounds.map(|(first, last)| (first.parse::<u64>(), last.parse::<u64>())) {
        Some((Ok(first), Ok(last))) => last < first,
        _ => false,
    }
}

/// `GET /v1/storage/contents/:hash/exists`: builder-server's `handleExists`
/// answer, the body always `{"ok":true}` and the status 200 for a file the
/// caller may read as [`get_storage_content`] would serve it, 404 for any
/// other.
pub async fn get_storage_content_exists(
    State(state): State<SharedState>,
    MaybeSigned(signer): MaybeSigned,
    Path(hash): Path<String>,
) -> Response {
    let found = match access::read_file(&state, signer.as_ref(), &hash).await {
        Ok(Some(read)) => match state.items.open_file(&read).await {
            Ok(opened) => opened.is_some(),
            Err(error) => return error.into_response(),
        },
        Ok(None) => false,
        Err(error) => return error.into_response(),
    };
    exists_status(found)
}

fn exists_status(found: bool) -> Response {
    let status = if found {
        StatusCode::OK
    } else {
        StatusCode::NOT_FOUND
    };
    (
        status,
        [(header::CACHE_CONTROL, NO_STORE)],
        Json(json!({ "ok": true })),
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn the_exists_probe_answers_upstreams_ok_body_with_its_status() {
        for (found, status) in [(true, StatusCode::OK), (false, StatusCode::NOT_FOUND)] {
            let resp = exists_status(found);
            assert_eq!(resp.status(), status);
            let bytes = axum::body::to_bytes(resp.into_body(), 64).await.unwrap();
            assert_eq!(
                serde_json::from_slice::<serde_json::Value>(&bytes).unwrap(),
                serde_json::json!({ "ok": true })
            );
        }
    }
}
