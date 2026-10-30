use axum::extract::{Path, Query, State};
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use serde::Deserialize;

use crate::SharedState;
use dcl_auth::is_canonical_cid;

#[derive(Debug, Default, Deserialize)]
pub struct ContentParams {
    pub ts: Option<String>,
}

fn content_url(bucket: &str, hash: &str, params: &ContentParams) -> String {
    let bucket = bucket.trim_end_matches('/');
    let mut target = format!("{}/contents/{}", bucket, hash);
    if let Some(ts) = params.ts.as_deref().filter(|s| !s.is_empty()) {
        target.push_str("?ts=");
        target.push_str(ts);
    }
    target
}

const IMMUTABLE: &str = "public,max-age=31536000,immutable";

/// An upload this node accepted streams from the ONE store (PNG sniffed,
/// `nosniff`, immutable); anything else is the old 301 to the content
/// bucket with the `?ts=` passthrough, or 404 without a bucket. A hash the
/// store withholds (`content::Denylist`) is 404 first: the store refuses
/// its bytes itself, and the check keeps the bucket redirect off it.
pub async fn get_storage_content(
    State(state): State<SharedState>,
    Path(hash): Path<String>,
    Query(params): Query<ContentParams>,
) -> Response {
    if state.denylist.contains(&hash) {
        return StatusCode::NOT_FOUND.into_response();
    }
    if is_canonical_cid(&hash) {
        match state.items.open_uploaded_content(&hash).await {
            Ok(Some(mut opened)) => {
                let head = match opened.read_range(0, 7).await {
                    Ok(head) => head,
                    Err(e) => {
                        tracing::error!(error = %e, hash, "builder upload read failed");
                        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
                    }
                };
                let mime = if head.starts_with(b"\x89PNG\r\n\x1a\n") {
                    "image/png"
                } else {
                    "application/octet-stream"
                };
                let size = opened.size;
                let mut resp = Response::new(opened.into_body(head));
                let headers = resp.headers_mut();
                headers.insert(header::CONTENT_TYPE, HeaderValue::from_static(mime));
                headers.insert(header::CONTENT_LENGTH, HeaderValue::from(size));
                headers.insert(header::CACHE_CONTROL, HeaderValue::from_static(IMMUTABLE));
                headers.insert(
                    header::X_CONTENT_TYPE_OPTIONS,
                    HeaderValue::from_static("nosniff"),
                );
                return resp;
            }
            Ok(None) => {}
            Err(error) => return error.into_response(),
        }
    }
    let Some(bucket) = state.settings.content_bucket_url.as_deref() else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let target = content_url(bucket, &hash, &params);

    let mut resp = (StatusCode::MOVED_PERMANENTLY, ()).into_response();
    let headers = resp.headers_mut();
    if let Ok(v) = HeaderValue::from_str(&target) {
        headers.insert(header::LOCATION, v);
    }
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static(IMMUTABLE));
    resp
}

/// Builder stores predate the canonical-CID rule and still hold merely-alphanumeric keys
/// (shortened digests, uppercase hex); the length window is only a path guard.
fn is_legacy_storage_key(hash: &str) -> bool {
    (32..=128).contains(&hash.len()) && hash.bytes().all(|b| b.is_ascii_alphanumeric())
}

fn is_valid_content_hash(hash: &str) -> bool {
    is_canonical_cid(hash) || is_legacy_storage_key(hash)
}

fn exists_status(ok: bool) -> Response {
    if ok {
        StatusCode::OK.into_response()
    } else {
        StatusCode::NOT_FOUND.into_response()
    }
}

/// `200`/`404`: the local acceptance record first, then a HEAD against the
/// bucket memoised for five minutes (single-flight per hash); a withheld
/// hash is 404 like an unknown one.
pub async fn head_storage_content_exists(
    State(state): State<SharedState>,
    Path(hash): Path<String>,
    Query(params): Query<ContentParams>,
) -> Response {
    if !is_valid_content_hash(&hash) || state.denylist.contains(&hash) {
        return StatusCode::NOT_FOUND.into_response();
    }

    match state.items.uploaded_content_exists(&hash).await {
        Ok(true) => return exists_status(true),
        Ok(false) => {}
        Err(error) => return error.into_response(),
    }

    let Some(bucket) = state.settings.content_bucket_url.as_deref() else {
        return exists_status(false);
    };
    let target = content_url(bucket, &hash, &params);
    let http = state.http.clone();
    let ok = state
        .exists_memo
        .get_or_fetch(hash, || async move {
            Ok::<bool, std::convert::Infallible>(matches!(
                http.request(reqwest::Method::HEAD, &target).send().await,
                Ok(r) if r.status().is_success()
            ))
        })
        .await
        .unwrap_or(false);
    exists_status(ok)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_canonical_cidv0() {
        assert!(is_valid_content_hash(
            "QmYwAPJzv5CZsnA625s3Xf2nemtYgPpHdWEz79ojWnPbdG"
        ));
    }

    #[test]
    fn accepts_canonical_cidv1() {
        assert!(is_valid_content_hash(
            "bafkreihdwdcefgh4dqkjv67uzcmw7ojee6xedzdetojuzjevtenora7777"
        ));
    }

    #[test]
    fn accepts_legacy_sha256_hex() {
        assert!(is_valid_content_hash(
            "a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2"
        ));
    }

    #[test]
    fn accepts_legacy_uppercase_and_short_keys_deployed_stores_still_hold() {
        assert!(is_valid_content_hash(
            "A1B2C3D4E5F6A1B2C3D4E5F6A1B2C3D4E5F6A1B2C3D4E5F6A1B2C3D4E5F6A1B2"
        ));
        assert!(is_valid_content_hash(
            "a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1"
        ));
        assert!(is_valid_content_hash(&"a".repeat(40)));
    }

    #[test]
    fn rejects_keys_outside_the_legacy_length_window() {
        assert!(!is_valid_content_hash(&"a".repeat(31)));
        assert!(!is_valid_content_hash(&"a".repeat(129)));
    }

    #[test]
    fn rejects_non_alphanumeric_keys() {
        assert!(!is_valid_content_hash(&format!("{}/x", "a".repeat(40))));
        assert!(!is_valid_content_hash(&format!("{}.x", "a".repeat(40))));
    }

    #[test]
    fn rejects_path_traversal() {
        assert!(!is_valid_content_hash("../../../etc/passwd"));
    }

    #[test]
    fn rejects_empty() {
        assert!(!is_valid_content_hash(""));
    }

    #[test]
    fn rejects_arbitrary_string() {
        assert!(!is_valid_content_hash("hello world"));
    }

    #[test]
    fn the_redirect_keeps_the_ts_passthrough() {
        let params = ContentParams {
            ts: Some("123".into()),
        };
        assert_eq!(
            content_url("https://bucket.test/", "Qm1", &params),
            "https://bucket.test/contents/Qm1?ts=123"
        );
        assert_eq!(
            content_url("https://bucket.test", "Qm1", &ContentParams::default()),
            "https://bucket.test/contents/Qm1"
        );
    }
}
