use axum::body::Body;
use axum::extract::{Path, Query, State};
use axum::http::header::ACCEPT_ENCODING;
use axum::http::{Extensions, HeaderMap, Method, StatusCode, Version};
use axum::response::{IntoResponse, Response};
use bytes::Bytes;
use serde::Deserialize;
use tower_http::compression::predicate::Predicate;
use tower_http::compression::{CompressionLayer, CompressionLevel};

use super::SharedState;
use super::errors::{AppError, AppResult};
use super::formatters::{
    ParsedRange, check_not_modified, content_file_headers, parse_range_header,
};

#[derive(Debug, Deserialize)]
pub struct ContentQuery {
    #[serde(rename = "includeMimeType")]
    pub include_mime_type: Option<String>,
}

/// The `?includeMimeType` answer for the head of a file: the binary
/// signatures of `file-type` that content files carry, else
/// `application/octet-stream` (`file-type` detects no text format, JSON
/// included).
pub fn detect_content_type(first_bytes: &[u8]) -> &'static str {
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

/// Marks a `/contents/{hash}` response [`compression_layer`] may compress on the
/// wire: a whole file (not a range, a 304, a HEAD or an X-Accel hand-off) of at
/// least [`MIN_COMPRESSED_SIZE`], asked for by a client that accepts br or gzip,
/// whose head is not an already-compressed format. Nothing else is compressed:
/// the layer sits on the content routes only, and these bodies are public,
/// immutable blobs with nothing of the requester's echoed into them, which is
/// what a BREACH-style length oracle needs.
#[derive(Clone, Copy, Debug)]
pub struct Compressible;

/// Below this a compressed body saves too little to pay for the framing.
const MIN_COMPRESSED_SIZE: u64 = 1024;

/// The client offers an encoding [`compression_layer`] produces. Only gates the
/// head read; the layer itself negotiates (q-values included).
fn accepts_compression(headers: &HeaderMap) -> bool {
    headers
        .get_all(ACCEPT_ENCODING)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|v| v.split(','))
        .filter_map(|coding| coding.split(';').next())
        .map(str::trim)
        .any(|name| {
            name.eq_ignore_ascii_case("br") || name.eq_ignore_ascii_case("gzip") || name == "*"
        })
}

/// A head that is not an already-compressed format: compressing PNG, JPEG,
/// WebP, KTX2, audio, video or archives costs CPU for nothing. GLB, glTF JSON,
/// scene code, `.bin` buffers and WAV are what shrink.
fn worth_compressing(head: &[u8]) -> bool {
    const KTX2: &[u8] = &[0xAB, 0x4B, 0x54, 0x58, 0x20, 0x32, 0x30, 0xBB];
    const GZIP: &[u8] = &[0x1F, 0x8B];
    const ZSTD: &[u8] = &[0x28, 0xB5, 0x2F, 0xFD];
    if head.starts_with(KTX2) || head.starts_with(GZIP) || head.starts_with(ZSTD) {
        return false;
    }
    matches!(
        detect_content_type(head),
        "model/gltf-binary" | "audio/wav" | "application/octet-stream"
    )
}

/// On-the-fly br/gzip for the `/contents/{hash}` routes, applied to responses
/// [`serve`] marked [`Compressible`] only. Quality 5 keeps the per-request cost
/// low (most of brotli's and gzip's gain on GLBs); a CDN in front caches each
/// encoding under the `Vary: Accept-Encoding` the layer adds. The layer leaves
/// ranged responses alone and drops `Content-Length`/`Accept-Ranges` from the
/// ones it compresses.
pub fn compression_layer() -> CompressionLayer<impl Predicate> {
    CompressionLayer::new()
        .quality(CompressionLevel::Precise(5))
        .compress_when(is_marked_compressible)
}

/// The [`compression_layer`] predicate: [`serve`] already applied every rule.
fn is_marked_compressible(
    _: StatusCode,
    _: Version,
    _: &HeaderMap,
    extensions: &Extensions,
) -> bool {
    extensions.get::<Compressible>().is_some()
}

/// nginx serves `<STORAGE_X_ACCEL_BASE>/<shard>/<hash>` with an alias onto the
/// storage tree, so the shard here must be the storage layout's.
pub(crate) fn x_accel_redirect_path(base: Option<&str>, hash: &str) -> Option<String> {
    let base = base?.trim_end_matches('/');
    if base.is_empty() {
        return None;
    }
    let shard = crate::storage::hex_prefix(hash);
    Some(format!("{base}/{shard}/{hash}"))
}

fn missing(hash: &str) -> AppError {
    AppError::not_found(format!("No content found with hash {hash}"))
}

fn apply_headers(response: &mut Response, headers: &[(&'static str, String)]) {
    let resp_headers = response.headers_mut();
    for (name, value) in headers {
        if let Ok(hv) = value.parse() {
            resp_headers.insert(*name, hv);
        }
    }
}

/// How a mount serves `/contents/{hash}`: the root content server's contract, or
/// worlds-content-server's (legacy SHA-256 hex keys, always sniffed).
#[derive(Clone, Copy)]
struct Mount {
    legacy_sha256: bool,
    always_sniff: bool,
}

impl Mount {
    /// The root answers `file-type`'s binary signatures only, as upstream's
    /// catalyst; the worlds mount also answers JSON as `application/json`,
    /// as worlds-content-server does (services/catalyst/docs/worlds-compatibility.md).
    fn sniff(self, head: &[u8]) -> &'static str {
        match detect_content_type(head) {
            "application/octet-stream" if self.always_sniff && looks_like_json(head) => {
                "application/json"
            }
            detected => detected,
        }
    }
}

/// A head that opens a JSON object or array, after an optional UTF-8 BOM.
fn looks_like_json(head: &[u8]) -> bool {
    let head = head.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(head);
    matches!(head.first(), Some(b'{' | b'['))
}

const ROOT: Mount = Mount {
    legacy_sha256: false,
    always_sniff: false,
};
const WORLDS: Mount = Mount {
    legacy_sha256: true,
    always_sniff: true,
};

/// GET and HEAD /contents/{hash}. A malformed hash is a 404 like any other
/// miss (the fixtures pin that); a storage fault is a 500, never a 404, so a
/// broken node gets retried instead of advertising the content as absent.
pub async fn get_content(
    State(state): State<SharedState>,
    Path(hash_id): Path<String>,
    Query(query): Query<ContentQuery>,
    method: Method,
    headers: HeaderMap,
) -> AppResult<Response> {
    let sniff = query.include_mime_type.is_some();
    serve(state, hash_id, sniff, method, headers, ROOT).await
}

/// The worlds mount's GET and HEAD /contents/{hash}: worlds-content-server
/// always sniffs the Content-Type and also answers a thumbnail's legacy
/// SHA-256 hex key, read here as the raw-leaf CIDv1 with that digest.
pub async fn get_world_content(
    State(state): State<SharedState>,
    Path(hash_id): Path<String>,
    method: Method,
    headers: HeaderMap,
) -> AppResult<Response> {
    serve(state, hash_id, true, method, headers, WORLDS).await
}

async fn serve(
    state: SharedState,
    hash_id: String,
    sniff: bool,
    method: Method,
    headers: HeaderMap,
    mount: Mount,
) -> AppResult<Response> {
    // `key` names the blob on disk; `hash_id` stays the client's ETag.
    let key = if crate::auth::is_canonical_cid(&hash_id) {
        hash_id.clone()
    } else if let Some(cid) = mount
        .legacy_sha256
        .then(|| crate::auth::raw_cid_v1_from_sha256_hex(&hash_id))
        .flatten()
    {
        cid
    } else {
        return Err(missing(&hash_id));
    };
    if state.denylist.contains(&hash_id) || state.denylist.contains(&key) {
        return Err(missing(&hash_id));
    }

    if let Some(not_modified_headers) = check_not_modified(&headers, &hash_id) {
        let mut response = StatusCode::NOT_MODIFIED.into_response();
        apply_headers(&mut response, &not_modified_headers);
        return Ok(response);
    }

    let mut opened = match state.storage.open(&key).await? {
        Some(opened) => opened,
        None => match state.read_through() {
            Some(read_through) if read_through.fill_content(&key).await => state
                .storage
                .open(&key)
                .await?
                .ok_or_else(|| missing(&hash_id))?,
            _ => return Err(missing(&hash_id)),
        },
    };

    let range_header = headers.get("range").and_then(|v| v.to_str().ok());
    let total = opened.size;
    let range = parse_range_header(range_header, Some(total));
    let mut base_headers = content_file_headers(&hash_id, Some(total));

    match range {
        Some(ParsedRange::Unsatisfiable) => {
            let mut response = StatusCode::RANGE_NOT_SATISFIABLE.into_response();
            apply_headers(
                &mut response,
                &[
                    ("Content-Range", format!("bytes */{total}")),
                    ("Access-Control-Expose-Headers", "Content-Range".to_string()),
                ],
            );
            return Ok(response);
        }
        Some(ParsedRange::Range { start, end }) => {
            if sniff {
                // The worlds mount sniffs the file's head, as WCS does; the root the range's.
                let from = if mount.always_sniff { 0 } else { start };
                let head = opened.read_range(from, from + 31).await?;
                set_content_type(&mut base_headers, mount.sniff(&head));
            }

            let body = if method == Method::HEAD {
                Body::empty()
            } else {
                opened.into_range_body(start, end).await?
            };

            let content_len = end - start + 1;
            let mut response = (StatusCode::PARTIAL_CONTENT, body).into_response();
            apply_headers(&mut response, &base_headers);
            apply_headers(
                &mut response,
                &[
                    ("Content-Range", format!("bytes {start}-{end}/{total}")),
                    ("Content-Length", content_len.to_string()),
                ],
            );
            return Ok(response);
        }
        None => {}
    }

    let accel = x_accel_redirect_path(state.settings.storage_x_accel_base.as_deref(), &key);
    // A whole file this process streams itself may be compressed on the wire
    // ([`Compressible`]); nginx streams an X-Accel hand-off, outside this layer.
    let compress = method == Method::GET
        && accel.is_none()
        && total >= MIN_COMPRESSED_SIZE
        && accepts_compression(&headers);
    // Upstream sniffs for HEAD as for GET: the headers are the same. The
    // worlds mount sniffs every HEAD, as WCS does.
    let sniff_type = sniff || (method == Method::HEAD && mount.always_sniff);
    let head = if sniff_type || compress {
        let head = opened.read_range(0, 31).await?;
        if sniff_type {
            set_content_type(&mut base_headers, mount.sniff(&head));
        }
        head
    } else {
        Bytes::new()
    };

    if method == Method::HEAD {
        let mut response = StatusCode::OK.into_response();
        apply_headers(&mut response, &base_headers);
        return Ok(response);
    }

    if let Some(accel) = accel {
        base_headers.retain(|(n, _)| *n != "Content-Length");
        let mut response = (StatusCode::OK, Body::empty()).into_response();
        apply_headers(&mut response, &base_headers);
        apply_headers(
            &mut response,
            &[
                ("X-Accel-Redirect", accel),
                ("Content-Length", "0".to_string()),
            ],
        );
        return Ok(response);
    }

    let compressible = compress && worth_compressing(&head);
    let mut response = Response::builder()
        .status(StatusCode::OK)
        .body(opened.into_body(head))
        .unwrap();
    apply_headers(&mut response, &base_headers);
    if compressible {
        response.extensions_mut().insert(Compressible);
    }
    Ok(response)
}

pub(crate) fn set_content_type(headers: &mut [(&'static str, String)], mime: &str) {
    for (name, value) in headers.iter_mut() {
        if *name == "Content-Type" {
            *value = mime.to_string();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::support::{TEST_CID, TestNode};

    #[test]
    fn x_accel_redirect_path_none_when_base_unset_or_empty() {
        assert_eq!(
            x_accel_redirect_path(None, "QmcoQSrVoi8CKSwiRyJ3MPYyN1AUiLjHiAtYCUGoBr8JM4"),
            None
        );
        assert_eq!(
            x_accel_redirect_path(Some(""), "QmcoQSrVoi8CKSwiRyJ3MPYyN1AUiLjHiAtYCUGoBr8JM4"),
            None
        );
    }

    #[test]
    fn x_accel_redirect_path_uses_storage_shard() {
        let got = x_accel_redirect_path(
            Some("/__protected_storage"),
            "QmcoQSrVoi8CKSwiRyJ3MPYyN1AUiLjHiAtYCUGoBr8JM4",
        );
        assert_eq!(
            got.as_deref(),
            Some("/__protected_storage/f049/QmcoQSrVoi8CKSwiRyJ3MPYyN1AUiLjHiAtYCUGoBr8JM4"),
        );
    }

    #[test]
    fn x_accel_redirect_path_strips_trailing_slash() {
        let got = x_accel_redirect_path(
            Some("/__protected_storage/"),
            "bafkreie4eisvkzyjuqrcendydk6vikqs2vco5lmib4nlzsxtjzofiqy2pa",
        );
        assert_eq!(
            got.as_deref(),
            Some(
                "/__protected_storage/f049/bafkreie4eisvkzyjuqrcendydk6vikqs2vco5lmib4nlzsxtjzofiqy2pa"
            ),
        );
    }

    #[test]
    fn detect_content_type_sniffs_video() {
        assert_eq!(
            detect_content_type(&[0x1A, 0x45, 0xDF, 0xA3, 0x01, 0x00]),
            "video/webm"
        );
        assert_eq!(
            detect_content_type(b"\0\0\0\x20ftypisom\0\0\x02\0"),
            "video/mp4"
        );
        assert_eq!(
            detect_content_type(b"plain bytes"),
            "application/octet-stream"
        );
    }

    #[test]
    fn detect_content_type_matches_file_type_on_content_formats() {
        assert_eq!(detect_content_type(b"GIF89a\x01\x00"), "image/gif");
        assert_eq!(
            detect_content_type(b"ID3\x04\x00\x00\x00\x00"),
            "audio/mpeg"
        );
        assert_eq!(detect_content_type(&[0xFF, 0xFB, 0x90, 0x64]), "audio/mpeg");
        assert_eq!(
            detect_content_type(b"RIFF\x24\x08\x00\x00WAVEfmt "),
            "audio/wav"
        );
        assert_eq!(
            detect_content_type(b"RIFF\x24\x08\x00\x00WEBPVP8 "),
            "image/webp"
        );
        assert_eq!(
            detect_content_type(b"PK\x03\x04\x14\x00"),
            "application/zip"
        );
        assert_eq!(detect_content_type(b"OggS\x00\x02"), "audio/ogg");
        // `file-type` has no text detector: JSON is not recognised.
        assert_eq!(
            detect_content_type(b"{\"version\":\"v3\"}"),
            "application/octet-stream"
        );
        assert_eq!(
            detect_content_type(b"\xEF\xBB\xBF[1]"),
            "application/octet-stream"
        );
        assert_eq!(detect_content_type(b""), "application/octet-stream");
    }

    async fn get(
        node: &TestNode,
        method: Method,
        mime: Option<&str>,
        range: Option<&str>,
    ) -> AppResult<Response> {
        let mut headers = HeaderMap::new();
        if let Some(r) = range {
            headers.insert("range", r.parse().unwrap());
        }
        get_content(
            State(node.state.clone()),
            Path(TEST_CID.to_string()),
            Query(ContentQuery {
                include_mime_type: mime.map(str::to_string),
            }),
            method,
            headers,
        )
        .await
    }

    #[tokio::test]
    async fn storage_fault_answers_500_not_404() {
        let node = TestNode::without_db("content-fault").await;
        node.store(TEST_CID, Bytes::from_static(b"x")).await;
        node.destroy_shard(TEST_CID).await;
        let err = get(&node, Method::GET, None, None)
            .await
            .expect_err("a storage fault must error, not resolve");
        assert_eq!(
            err.into_response().status(),
            StatusCode::INTERNAL_SERVER_ERROR,
            "a broken node must error (and get retried), not advertise the content as absent"
        );
    }

    #[tokio::test]
    async fn missing_content_still_answers_404() {
        let node = TestNode::without_db("content-miss").await;
        let err = get(&node, Method::GET, None, None)
            .await
            .expect_err("missing content is a 404");
        assert_eq!(err.into_response().status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn denylisted_content_is_a_404() {
        let node = TestNode::without_db("content-denied").await;
        node.store(TEST_CID, Bytes::from_static(b"x")).await;
        node.state.denylist.add(TEST_CID);
        let err = get(&node, Method::GET, None, None).await.unwrap_err();
        assert_eq!(err.into_response().status(), StatusCode::NOT_FOUND);
    }

    fn header(resp: &Response, name: &str) -> Option<String> {
        resp.headers()
            .get(name)
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string())
    }

    #[tokio::test]
    async fn include_mime_type_sniffs_and_replays_the_head() {
        let mut blob = vec![0u8; 200 * 1024];
        blob[..8].copy_from_slice(&[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]);
        for (i, b) in blob.iter_mut().enumerate().skip(8) {
            *b = (i % 251) as u8;
        }
        let blob = Bytes::from(blob);
        let node = TestNode::without_db("content-mime").await;
        node.store(TEST_CID, blob.clone()).await;

        let resp = get(&node, Method::GET, Some(""), None).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        assert_eq!(header(&resp, "content-type").unwrap(), "image/png");
        assert_eq!(
            header(&resp, "content-length").unwrap(),
            blob.len().to_string()
        );
        assert_eq!(header(&resp, "etag").unwrap(), format!("\"{TEST_CID}\""));
        let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        assert_eq!(body, blob);

        let resp = get(&node, Method::GET, None, None).await.unwrap();
        assert_eq!(
            header(&resp, "content-type").unwrap(),
            "application/octet-stream"
        );
        let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        assert_eq!(body, blob);

        // Upstream sniffs for HEAD too (`get-content-handler.ts:54`).
        let resp = get(&node, Method::HEAD, Some(""), None).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        assert_eq!(header(&resp, "content-type").unwrap(), "image/png");
        let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        assert!(body.is_empty());

        let resp = get(&node, Method::GET, Some(""), Some("bytes=0-9"))
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::PARTIAL_CONTENT);
        assert_eq!(header(&resp, "content-type").unwrap(), "image/png");
        assert_eq!(
            header(&resp, "content-range").unwrap(),
            format!("bytes 0-9/{}", blob.len())
        );
        let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        assert_eq!(&body[..], &blob[..10]);

        let resp = get(&node, Method::GET, None, Some("bytes=999999999-"))
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::RANGE_NOT_SATISFIABLE);
    }

    async fn body_of(resp: Response) -> Bytes {
        axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap()
    }

    #[tokio::test]
    async fn a_range_past_the_start_sniffs_from_the_range_start() {
        let mut blob = vec![b'x'; 4096];
        blob[1000..1008].copy_from_slice(&[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]);
        let blob = Bytes::from(blob);
        let node = TestNode::without_db("content-range-offset").await;
        node.store(TEST_CID, blob.clone()).await;

        let resp = get(&node, Method::GET, Some(""), Some("bytes=1000-1999"))
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::PARTIAL_CONTENT);
        assert_eq!(header(&resp, "content-type").unwrap(), "image/png");
        assert_eq!(header(&resp, "content-length").unwrap(), "1000");
        assert_eq!(
            header(&resp, "content-range").unwrap(),
            "bytes 1000-1999/4096"
        );
        assert_eq!(body_of(resp).await, blob.slice(1000..2000));
    }

    #[tokio::test]
    async fn head_with_a_range_sends_the_window_length_and_no_body() {
        let node = TestNode::without_db("content-range-head").await;
        node.store(TEST_CID, Bytes::from_static(b"0123456789"))
            .await;

        let resp = get(&node, Method::HEAD, None, Some("bytes=2-5"))
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::PARTIAL_CONTENT);
        assert_eq!(header(&resp, "content-length").unwrap(), "4");
        assert_eq!(header(&resp, "content-range").unwrap(), "bytes 2-5/10");
        assert!(body_of(resp).await.is_empty());
    }

    #[tokio::test]
    async fn an_empty_file_rejects_a_range_and_ignores_a_malformed_one() {
        let node = TestNode::without_db("content-range-empty").await;
        node.store(TEST_CID, Bytes::new()).await;

        let resp = get(&node, Method::GET, None, Some("bytes=0-"))
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::RANGE_NOT_SATISFIABLE);
        assert_eq!(header(&resp, "content-range").unwrap(), "bytes */0");

        for malformed in ["bytes=abc", "bytes=0-1,2-3"] {
            let resp = get(&node, Method::GET, None, Some(malformed))
                .await
                .unwrap();
            assert_eq!(resp.status(), StatusCode::OK, "{malformed}");
            assert!(body_of(resp).await.is_empty());
        }
    }

    #[tokio::test]
    async fn etag_match_short_circuits_to_304() {
        let node = TestNode::without_db("content-etag").await;
        node.store(TEST_CID, Bytes::from_static(b"x")).await;
        let mut headers = HeaderMap::new();
        headers.insert("if-none-match", format!("\"{TEST_CID}\"").parse().unwrap());
        let resp = get_content(
            State(node.state.clone()),
            Path(TEST_CID.to_string()),
            Query(ContentQuery {
                include_mime_type: None,
            }),
            Method::GET,
            headers,
        )
        .await
        .unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_MODIFIED);
        assert_eq!(header(&resp, "etag").unwrap(), format!("\"{TEST_CID}\""));
    }

    async fn world_get(
        node: &TestNode,
        hash: &str,
        method: Method,
        range: Option<&str>,
    ) -> AppResult<Response> {
        let mut headers = HeaderMap::new();
        if let Some(r) = range {
            headers.insert("range", r.parse().unwrap());
        }
        get_world_content(
            State(node.state.clone()),
            Path(hash.to_string()),
            method,
            headers,
        )
        .await
    }

    fn sha256_hex(data: &[u8]) -> String {
        use sha2::Digest;
        sha2::Sha256::digest(data)
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect()
    }

    #[tokio::test]
    async fn the_worlds_mount_always_sniffs_and_the_root_does_not() {
        let gif = Bytes::from_static(b"GIF89a\x01\x00\x01\x00rest-of-the-image");
        let png = Bytes::from_static(&[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 1, 2, 3]);
        let node = TestNode::without_db("content-world-sniff").await;
        let gif_cid = crate::auth::hash_bytes_v1(&gif);
        node.store(&gif_cid, gif.clone()).await;
        node.store(TEST_CID, png.clone()).await;

        for method in [Method::GET, Method::HEAD] {
            let resp = world_get(&node, TEST_CID, method.clone(), None)
                .await
                .unwrap();
            assert_eq!(
                header(&resp, "content-type").unwrap(),
                "image/png",
                "{method}"
            );
            let resp = world_get(&node, &gif_cid, method.clone(), None)
                .await
                .unwrap();
            assert_eq!(
                header(&resp, "content-type").unwrap(),
                "image/gif",
                "{method}"
            );
        }
        let resp = world_get(&node, TEST_CID, Method::GET, None).await.unwrap();
        assert_eq!(body_of(resp).await, png);
        // A range sniffs the file's head, not the window's.
        let resp = world_get(&node, TEST_CID, Method::GET, Some("bytes=8-10"))
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::PARTIAL_CONTENT);
        assert_eq!(header(&resp, "content-type").unwrap(), "image/png");
        assert_eq!(body_of(resp).await, png.slice(8..11));

        // Worlds JSON keeps WCS's application/json; the root, as upstream's
        // `file-type`, has no text detector.
        let json = Bytes::from_static(b"\xEF\xBB\xBF{\"version\":\"v3\"}");
        let json_cid = crate::auth::hash_bytes_v1(&json);
        node.store(&json_cid, json.clone()).await;
        for method in [Method::GET, Method::HEAD] {
            let resp = world_get(&node, &json_cid, method.clone(), None)
                .await
                .unwrap();
            assert_eq!(
                header(&resp, "content-type").unwrap(),
                "application/json",
                "{method}"
            );
        }
        assert_eq!(
            Mount::sniff(ROOT, &json),
            "application/octet-stream",
            "the root with includeMimeType"
        );
        assert_eq!(Mount::sniff(ROOT, b"GIF87a\x01\x00"), "image/gif");

        let resp = get(&node, Method::GET, None, None).await.unwrap();
        assert_eq!(
            header(&resp, "content-type").unwrap(),
            "application/octet-stream"
        );
        // The root sniffs only on `includeMimeType`, for HEAD as for GET
        // (upstream's `get-content-handler.ts:54`).
        let resp = get(&node, Method::HEAD, None, None).await.unwrap();
        assert_eq!(
            header(&resp, "content-type").unwrap(),
            "application/octet-stream"
        );
        let resp = get(&node, Method::HEAD, Some(""), None).await.unwrap();
        assert_eq!(header(&resp, "content-type").unwrap(), "image/png");
    }

    #[tokio::test]
    async fn the_worlds_mount_answers_a_legacy_sha256_key_and_the_root_does_not() {
        let thumb = Bytes::from_static(&[0xFF, 0xD8, 0xFF, 0xE0, 0, 0x10, b'J', b'F']);
        let node = TestNode::without_db("content-world-sha256").await;
        node.store(&crate::auth::hash_bytes_v1(&thumb), thumb.clone())
            .await;
        let hex = sha256_hex(&thumb);

        let resp = world_get(&node, &hex, Method::GET, None).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        assert_eq!(header(&resp, "content-type").unwrap(), "image/jpeg");
        assert_eq!(header(&resp, "etag").unwrap(), format!("\"{hex}\""));
        assert_eq!(body_of(resp).await, thumb);
        let resp = world_get(&node, &hex, Method::HEAD, None).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        for bad in [hex.to_uppercase(), hex[..63].to_string()] {
            let err = world_get(&node, &bad, Method::GET, None).await.unwrap_err();
            assert_eq!(err.into_response().status(), StatusCode::NOT_FOUND, "{bad}");
        }
        let err = get_content(
            State(node.state.clone()),
            Path(hex.clone()),
            Query(ContentQuery {
                include_mime_type: None,
            }),
            Method::GET,
            HeaderMap::new(),
        )
        .await
        .unwrap_err();
        assert_eq!(err.into_response().status(), StatusCode::NOT_FOUND);

        // The denylist withholds the blob under either name.
        node.state.denylist.add(&hex);
        let err = world_get(&node, &hex, Method::GET, None).await.unwrap_err();
        assert_eq!(err.into_response().status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn x_accel_base_hands_the_body_to_nginx() {
        let mut settings = crate::Settings::for_tests();
        settings.storage_x_accel_base = Some("/__protected_storage".into());
        let node = TestNode::without_db_settings("content-accel", settings).await;
        node.store(TEST_CID, Bytes::from_static(b"payload")).await;
        let resp = get(&node, Method::GET, None, None).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        assert_eq!(
            header(&resp, "x-accel-redirect").unwrap(),
            format!(
                "/__protected_storage/{}/{TEST_CID}",
                crate::storage::hex_prefix(TEST_CID)
            )
        );
        assert_eq!(header(&resp, "content-length").unwrap(), "0");
        let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        assert!(body.is_empty());
    }

    /// A 4 KiB GLB: the binary glTF magic, then repetitive bytes that compress.
    fn glb_blob() -> Bytes {
        let mut blob = b"glTF\x02\x00\x00\x00".to_vec();
        blob.extend(std::iter::repeat_n(b"vertex data ", 340).flatten());
        Bytes::from(blob)
    }

    async fn get_accepting(
        node: &TestNode,
        method: Method,
        accept_encoding: Option<&str>,
        range: Option<&str>,
    ) -> Response {
        let mut headers = HeaderMap::new();
        if let Some(a) = accept_encoding {
            headers.insert(ACCEPT_ENCODING, a.parse().unwrap());
        }
        if let Some(r) = range {
            headers.insert("range", r.parse().unwrap());
        }
        get_content(
            State(node.state.clone()),
            Path(TEST_CID.to_string()),
            Query(ContentQuery {
                include_mime_type: None,
            }),
            method,
            headers,
        )
        .await
        .unwrap()
    }

    fn marked(resp: &Response) -> bool {
        resp.extensions().get::<Compressible>().is_some()
    }

    #[test]
    fn accepts_compression_reads_the_offered_codings() {
        let offered = |v: &str| {
            let mut h = HeaderMap::new();
            h.insert(ACCEPT_ENCODING, v.parse().unwrap());
            accepts_compression(&h)
        };
        assert!(offered("gzip, deflate"));
        assert!(offered("br;q=1.0, gzip;q=0.8"));
        assert!(offered("GZIP"));
        assert!(offered("*"));
        assert!(!offered("identity"));
        assert!(!offered("deflate, zstd"));
        assert!(!accepts_compression(&HeaderMap::new()));
    }

    #[test]
    fn already_compressed_formats_are_not_worth_compressing() {
        assert!(worth_compressing(b"glTF\x02\x00\x00\x00"));
        assert!(worth_compressing(b"{\"asset\":{\"version\":\"2.0\"}}"));
        assert!(worth_compressing(b"RIFF\x24\x00\x00\x00WAVEfmt "));
        assert!(worth_compressing(b""));
        assert!(!worth_compressing(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]));
        assert!(!worth_compressing(&[0xFF, 0xD8, 0xFF, 0xE0]));
        assert!(!worth_compressing(b"RIFF\x24\x00\x00\x00WEBPVP8 "));
        assert!(!worth_compressing(&[0xAB, 0x4B, 0x54, 0x58, 0x20, 0x32, 0x30, 0xBB, 0x0D, 0x0A]));
        assert!(!worth_compressing(b"OggS\x00\x02"));
        assert!(!worth_compressing(&[0x1F, 0x8B, 0x08]));
        assert!(!worth_compressing(&[0x28, 0xB5, 0x2F, 0xFD]));
    }

    #[tokio::test]
    async fn only_whole_compressible_files_for_willing_clients_are_marked() {
        let node = TestNode::without_db("content-compress-mark").await;
        node.store(TEST_CID, glb_blob()).await;
        assert!(marked(&get_accepting(&node, Method::GET, Some("br, gzip"), None).await));
        // a client that offers nothing the layer produces
        assert!(!marked(&get_accepting(&node, Method::GET, None, None).await));
        assert!(!marked(&get_accepting(&node, Method::GET, Some("identity"), None).await));
        // a range is served as stored bytes
        let ranged = get_accepting(&node, Method::GET, Some("br"), Some("bytes=0-99")).await;
        assert_eq!(ranged.status(), StatusCode::PARTIAL_CONTENT);
        assert!(!marked(&ranged));
        // HEAD reports the stored representation
        assert!(!marked(&get_accepting(&node, Method::HEAD, Some("br"), None).await));
    }

    #[tokio::test]
    async fn small_and_precompressed_files_are_not_marked() {
        let node = TestNode::without_db("content-compress-small").await;
        node.store(TEST_CID, Bytes::from_static(b"glTF tiny")).await;
        assert!(!marked(&get_accepting(&node, Method::GET, Some("br"), None).await));

        let node = TestNode::without_db("content-compress-png").await;
        let mut png = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
        png.resize(4096, 0);
        node.store(TEST_CID, Bytes::from(png)).await;
        assert!(!marked(&get_accepting(&node, Method::GET, Some("br"), None).await));
    }

    #[tokio::test]
    async fn x_accel_hand_off_is_not_marked() {
        let mut settings = crate::Settings::for_tests();
        settings.storage_x_accel_base = Some("/__protected_storage".into());
        let node = TestNode::without_db_settings("content-compress-accel", settings).await;
        node.store(TEST_CID, glb_blob()).await;
        assert!(!marked(&get_accepting(&node, Method::GET, Some("br"), None).await));
    }

    /// Through the layer: a marked GLB goes out gzip-encoded with `Vary`, decodes
    /// to the stored bytes (the bytes its CID names), and loses `Content-Length`
    /// and `Accept-Ranges`; the ETag is the content's.
    #[tokio::test]
    async fn the_layer_compresses_marked_responses_on_the_wire() {
        use std::io::Read;
        use tower::ServiceExt;

        let node = TestNode::without_db("content-compress-layer").await;
        let blob = glb_blob();
        node.store(TEST_CID, blob.clone()).await;
        let app = axum::Router::new()
            .route(
                "/contents/{hashId}",
                axum::routing::get(get_content).route_layer(compression_layer()),
            )
            .with_state(node.state.clone());
        let request = |accept: Option<&str>| {
            let mut builder = axum::http::Request::get(format!("/contents/{TEST_CID}"));
            if let Some(accept) = accept {
                builder = builder.header(ACCEPT_ENCODING, accept);
            }
            builder.body(Body::empty()).unwrap()
        };

        let resp = app.clone().oneshot(request(Some("gzip"))).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        assert_eq!(header(&resp, "content-encoding").as_deref(), Some("gzip"));
        assert!(header(&resp, "vary").is_some_and(|v| v.to_ascii_lowercase().contains("accept-encoding")));
        assert_eq!(header(&resp, "content-length"), None);
        assert_eq!(header(&resp, "accept-ranges"), None);
        assert_eq!(header(&resp, "etag"), Some(crate::formatters::to_etag(TEST_CID)));
        let wire = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        assert!(wire.len() < blob.len());
        let mut decoded = Vec::new();
        flate2::read::GzDecoder::new(&wire[..])
            .read_to_end(&mut decoded)
            .unwrap();
        assert_eq!(decoded, blob);

        // without the offer, the stored bytes as they are
        let resp = app.oneshot(request(None)).await.unwrap();
        assert_eq!(header(&resp, "content-encoding"), None);
        let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        assert_eq!(body, blob);
    }
}
