use axum::body::Body;
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, Method, StatusCode};
use axum::response::{IntoResponse, Response};
use bytes::Bytes;
use serde::Deserialize;

use super::errors::{AppError, AppResult};
use super::file_type::{SAMPLE, Window, catalyst, file_type};
use super::formatters::{
    ParsedRange, check_not_modified, content_file_headers, parse_range_header,
};
use super::storage::OpenedContent;
use super::{ContentState, SharedState};

#[derive(Debug, Deserialize)]
pub struct ContentQuery {
    #[serde(rename = "includeMimeType")]
    pub include_mime_type: Option<String>,
}

/// [`sniff`]'s answer for `key`'s `window`, kept per window, since a cut stream can answer
/// differently from the whole file. Content is immutable by key, and a sniff can fetch up to
/// 16 MiB of the file. Whole-file answers and range answers are kept in separate caches, so a
/// client cycling through ranges cannot evict the whole-file answers. A kept answer reads nothing,
/// so the window stays unread. A storage fault is an error and is not kept.
pub(crate) async fn content_type(
    state: &ContentState,
    key: &str,
    window: &mut Window<'_>,
) -> AppResult<&'static str> {
    let (start, end) = window.bounds();
    if (start, end) == (0, window.file_size()) {
        kept_sniff(&state.sniffed, key.to_string(), window).await
    } else {
        kept_sniff(&state.sniffed_ranges, (key.to_string(), start, end), window).await
    }
}

async fn kept_sniff<K: std::hash::Hash + Eq>(
    cache: &parking_lot::Mutex<hashlink::LruCache<K, &'static str>>,
    cache_key: K,
    window: &mut Window<'_>,
) -> AppResult<&'static str> {
    if let Some(mime) = cache.lock().get(&cache_key).copied() {
        return Ok(mime);
    }
    let mime = sniff(window).await?;
    cache.lock().insert(cache_key, mime);
    Ok(mime)
}

/// The content server's Content-Type for the stream `window` reads, as the catalyst's
/// `createContentFileHeaders` answers it for `?includeMimeType` and the entity image and thumbnail
/// routes: `file-type@21.1.1`'s type, else `application/octet-stream`, which is also the answer
/// where upstream's sniff throws (a 500 there). `file-type` detects no text format, JSON
/// included. A storage fault is an error.
///
/// One deliberate, permanent difference from the catalyst (`content-reads.md` CR-7,
/// `docs/adr/2026-11-21-stored-files-served-inert.md`): a type a browser opens as a document
/// ([`inert`]) is `application/octet-stream`.
pub(crate) async fn sniff(window: &mut Window<'_>) -> AppResult<&'static str> {
    let size = window.file_size();
    Ok(catalyst::file_type(window, size)
        .await?
        .map_or("application/octet-stream", inert))
}

/// `mime`, or `application/octet-stream` for a type a browser opens as a document that can run
/// scripts: `text/*`, `*/xml`, `*+xml` and any HTML type. These routes share the node's origin,
/// where an XHTML or SVG document's scripts would run.
fn inert(mime: &'static str) -> &'static str {
    let document = mime.starts_with("text/")
        || mime.ends_with("/xml")
        || mime.ends_with("+xml")
        || mime.contains("html");
    if document {
        "application/octet-stream"
    } else {
        mime
    }
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
/// worlds-content-server's (`content-file-handler.ts`: legacy SHA-256 hex keys,
/// always sniffed, a `Range` on HEAD ignored, a bare 404).
#[derive(Clone, Copy)]
struct Mount {
    legacy_sha256: bool,
    always_sniff: bool,
    head_ignores_range: bool,
    bare_not_found: bool,
}

/// The worlds mount's Content-Type for `key`'s file cut at `end` (`ranged` when that is the
/// window a ranged GET sniffs), and the head read for it, with the reader positioned after it.
/// No head means the reader's position is unknown, so a whole GET reads the head again.
///
/// `file-type`'s answer, with two deliberate differences from worlds-content-server
/// (services/catalyst/docs/worlds-compatibility.md): a type a browser opens as a document
/// ([`inert`]) is `application/octet-stream`, since this mount shares the node's origin; and
/// a head that opens a JSON object or array is `application/json`. A storage fault while sniffing
/// answers `application/octet-stream`, as upstream's catch does, and no head, so a HEAD or ranged
/// GET answers without reading the file again. Content is immutable by key, so an answer is kept
/// and later reads of the file skip the sniff.
async fn worlds_content_type(
    state: &ContentState,
    key: &str,
    opened: &mut OpenedContent,
    end: u64,
    ranged: bool,
) -> AppResult<(&'static str, Option<Bytes>)> {
    let cache_key = (key.to_string(), ranged);
    if let Some(mime) = state.worlds_sniffed.lock().get(&cache_key).copied() {
        return Ok((mime, Some(Bytes::new())));
    }
    let mut window = Window::new(opened, end);
    let detected = match file_type(&mut window).await {
        Ok(detected) => detected,
        Err(error) => {
            tracing::warn!(key, %error, "worlds content sniff failed; answering octet-stream");
            return Ok(("application/octet-stream", None));
        }
    };
    let head = window.into_head().await?;
    let mime = match detected {
        Some(mime) => inert(mime),
        None if looks_like_json(&head) => "application/json",
        None => "application/octet-stream",
    };
    state.worlds_sniffed.lock().insert(cache_key, mime);
    Ok((mime, Some(head)))
}

/// A head that opens a JSON object or array, after an optional UTF-8 BOM.
fn looks_like_json(head: &[u8]) -> bool {
    let head = head.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(head);
    matches!(head.first(), Some(b'{' | b'['))
}

const ROOT: Mount = Mount {
    legacy_sha256: false,
    always_sniff: false,
    head_ignores_range: false,
    bare_not_found: false,
};
const WORLDS: Mount = Mount {
    legacy_sha256: true,
    always_sniff: true,
    head_ignores_range: true,
    bare_not_found: true,
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
/// SHA-256 hex key, read here as the raw-leaf CIDv1 with that digest. A miss
/// is a 404 without a body (`content-file-handler.ts` `{ status: 404 }`).
pub async fn get_world_content(
    State(state): State<SharedState>,
    Path(hash_id): Path<String>,
    method: Method,
    headers: HeaderMap,
) -> AppResult<Response> {
    match serve(state, hash_id, true, method, headers, WORLDS).await {
        Err(AppError::NotFound(_)) if WORLDS.bare_not_found => {
            Ok(StatusCode::NOT_FOUND.into_response())
        }
        answered => answered,
    }
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

    // WCS's `headContentFile` answers a HEAD's full headers whatever its `Range`.
    let range_header = headers
        .get("range")
        .filter(|_| !(method == Method::HEAD && mount.head_ignores_range))
        .and_then(|v| v.to_str().ok());
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
                // The worlds mount sniffs the file's first `SAMPLE` bytes, as WCS does; the root
                // the range, as the catalyst sniffs its ranged stream.
                let mime = if mount.always_sniff {
                    worlds_content_type(&state, &key, &mut opened, SAMPLE, true)
                        .await?
                        .0
                } else {
                    content_type(
                        &state,
                        &key,
                        &mut Window::range(&mut opened, start, end + 1),
                    )
                    .await?
                };
                set_content_type(&mut base_headers, mime);
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

    // nginx serves the body of a GET when the gateway is in front, so the app reads none of it.
    let accel = x_accel_redirect_path(state.settings.storage_x_accel_base.as_deref(), &key)
        .filter(|_| method != Method::HEAD);

    // Upstream sniffs for HEAD as for GET: the headers are the same. The
    // worlds mount sniffs every HEAD, as WCS does.
    let head = if sniff || (method == Method::HEAD && mount.always_sniff) {
        let (mime, head) = if mount.always_sniff {
            worlds_content_type(&state, &key, &mut opened, total, false).await?
        } else {
            let mut window = Window::new(&mut opened, total);
            let mime = content_type(&state, &key, &mut window).await?;
            let head = if method == Method::HEAD || accel.is_some() {
                Bytes::new()
            } else {
                window.into_head().await?
            };
            (mime, Some(head))
        };
        set_content_type(&mut base_headers, mime);
        head
    } else {
        Some(Bytes::new())
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

    // After a failed sniff the reader may be anywhere: replay the body from its start.
    let head = match head {
        Some(head) => head,
        None => opened.read_range(0, SAMPLE - 1).await?,
    };
    let mut response = Response::builder()
        .status(StatusCode::OK)
        .body(opened.into_body(head))
        .unwrap();
    apply_headers(&mut response, &base_headers);
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

    async fn get(
        node: &TestNode,
        method: Method,
        mime: Option<&str>,
        range: Option<&str>,
    ) -> AppResult<Response> {
        get_hash(node, TEST_CID, method, mime, range).await
    }

    async fn get_hash(
        node: &TestNode,
        hash: &str,
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
            Path(hash.to_string()),
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
        // A PNG signature, an IHDR chunk and an IDAT chunk that runs to the end of the file.
        let mut blob = vec![0u8; 200 * 1024];
        let idat_len = (blob.len() - 45) as u32;
        blob[..16].copy_from_slice(b"\x89PNG\r\n\x1a\n\0\0\0\x0dIHDR");
        blob[33..37].copy_from_slice(&idat_len.to_be_bytes());
        blob[37..41].copy_from_slice(b"IDAT");
        for (i, b) in blob.iter_mut().enumerate().skip(41) {
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

        // A range is sniffed as the stream it is: one that ends before the IDAT chunk header
        // has no type (upstream's sniff throws there).
        let resp = get(&node, Method::HEAD, Some(""), Some("bytes=0-40"))
            .await
            .unwrap();
        assert_eq!(header(&resp, "content-type").unwrap(), "image/png");
        let resp = get(&node, Method::GET, Some(""), Some("bytes=0-9"))
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::PARTIAL_CONTENT);
        assert_eq!(
            header(&resp, "content-type").unwrap(),
            "application/octet-stream"
        );
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

    /// `?includeMimeType` answers `file-type`'s strings, ISO-BMFF brands told apart, on GET and HEAD,
    /// and a GET still serves the whole file after the sniff.
    #[tokio::test]
    async fn include_mime_type_answers_file_type_strings() {
        let ftyp = |brand: &[u8]| [&b"\0\0\0\x18ftyp"[..], brand, b"\0\0\0\0isomiso2"].concat();
        let mut tar = b"a.txt".to_vec();
        tar.resize(257, 0);
        tar.extend_from_slice(b"ustar\x0000");
        let cases: [(&str, Vec<u8>); 9] = [
            ("image/avif", ftyp(b"avif")),
            ("image/heic", ftyp(b"heic")),
            ("image/heif", ftyp(b"mif1")),
            ("video/mp4", ftyp(b"isom")),
            ("image/bmp", b"BM".to_vec()),
            ("image/ktx", b"\xABKTX 11\xBB\r\n\x1A\n".to_vec()),
            ("application/wasm", b"\0asm\x01\0\0\0".to_vec()),
            ("application/pdf", b"%PDF-1.7\n".to_vec()),
            ("application/x-tar", tar),
        ];
        let node = TestNode::without_db("content-mime-formats").await;
        for (mime, mut bytes) in cases {
            bytes.resize(5000, 0);
            let bytes = Bytes::from(bytes);
            let cid = crate::auth::hash_bytes_v1(&bytes);
            node.store(&cid, bytes.clone()).await;
            for method in [Method::GET, Method::HEAD] {
                let resp = get_hash(&node, &cid, method.clone(), Some(""), None)
                    .await
                    .unwrap();
                assert_eq!(header(&resp, "content-type").unwrap(), mime, "{method}");
                if method == Method::GET {
                    assert_eq!(
                        body_of(resp).await,
                        bytes,
                        "{mime}: the body after the sniff"
                    );
                }
            }
        }
    }

    async fn body_of(resp: Response) -> Bytes {
        axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap()
    }

    #[tokio::test]
    async fn a_range_past_the_start_sniffs_from_the_range_start() {
        let mut blob = vec![b'x'; 4096];
        blob[1000..1012].copy_from_slice(b"\0\0\0\x18ftypavif");
        let blob = Bytes::from(blob);
        let node = TestNode::without_db("content-range-offset").await;
        node.store(TEST_CID, blob.clone()).await;

        let resp = get(&node, Method::GET, Some(""), Some("bytes=1000-1999"))
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::PARTIAL_CONTENT);
        assert_eq!(header(&resp, "content-type").unwrap(), "image/avif");
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
        // A signature, an IHDR chunk and a one-byte IDAT chunk: `file-type` reads to the IDAT.
        let png = Bytes::from(
            [
                &b"\x89PNG\r\n\x1a\n\0\0\0\x0dIHDR"[..],
                &[0; 17],
                b"\0\0\0\x01IDAT",
                &[0; 5],
            ]
            .concat(),
        );
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

        // The worlds mount answers JSON as application/json (an intended
        // difference: `file-type`, and so WCS, has no text detector); the root
        // answers as upstream's catalyst.
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
        let resp = get_hash(&node, &json_cid, Method::GET, Some(""), None)
            .await
            .unwrap();
        assert_eq!(
            header(&resp, "content-type").unwrap(),
            "application/octet-stream",
            "the root with includeMimeType"
        );

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

    /// The worlds mount reads past the sniff window where `file-type` does: a whole GET walks
    /// the PNG chunks and the PDF, a ranged GET only its first `SAMPLE` bytes. The body is the
    /// stored file whatever the sniff read.
    #[tokio::test]
    async fn the_worlds_sniff_reads_past_the_window_on_a_whole_get_only() {
        let chunk = |kind: &[u8], len: u32| {
            [&len.to_be_bytes()[..], kind, &vec![0; len as usize + 4]].concat()
        };
        let mut apng = b"\x89PNG\r\n\x1a\n".to_vec();
        apng.extend(chunk(b"IHDR", 13));
        apng.extend(chunk(b"tEXt", 5000));
        apng.extend(chunk(b"acTL", 8));
        let mut illustrator = b"%PDF-1.5\n".to_vec();
        illustrator.resize(70_000, b' ');
        illustrator.extend_from_slice(b"AIPrivateData1");
        let mut id3 = b"ID3\x04\x00\x00\x00\x00\x27\x08".to_vec(); // a 5000-byte tag
        id3.resize(10 + 5000, 0);
        id3.extend_from_slice(&[0xFF, 0xFB, 0x90, 0x64]);
        id3.resize(9000, 0);

        let node = TestNode::without_db("content-world-sniff-past-window").await;
        for (bytes, whole) in [
            (apng, "image/apng"),
            (illustrator, "application/postscript"),
            (id3, "audio/mpeg"),
        ] {
            let bytes = Bytes::from(bytes);
            let cid = crate::auth::hash_bytes_v1(&bytes);
            node.store(&cid, bytes.clone()).await;
            let resp = world_get(&node, &cid, Method::GET, None).await.unwrap();
            assert_eq!(header(&resp, "content-type").unwrap(), whole);
            let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
                .await
                .unwrap();
            assert_eq!(body, bytes, "{whole}: the body after the sniff");
            let resp = world_get(&node, &cid, Method::GET, Some("bytes=0-1"))
                .await
                .unwrap();
            assert_eq!(resp.status(), StatusCode::PARTIAL_CONTENT);
            let ranged = if whole == "application/postscript" {
                "application/pdf"
            } else {
                "application/octet-stream"
            };
            assert_eq!(header(&resp, "content-type").unwrap(), ranged, "{whole}");
        }
    }

    /// XML is octet-stream on the worlds mount (an XHTML or SVG document would run its scripts on
    /// the node's origin), and a sniffed answer is kept for the key: content does not change
    /// under it.
    #[tokio::test]
    async fn the_worlds_mount_serves_xml_as_octet_stream_and_keeps_its_answers() {
        let node = TestNode::without_db("content-world-sniff-xml").await;
        let xml = Bytes::from_static(
            b"<?xml version=\"1.0\"?><svg xmlns=\"http://www.w3.org/2000/svg\"/>",
        );
        let xml_cid = crate::auth::hash_bytes_v1(&xml);
        node.store(&xml_cid, xml.clone()).await;
        for range in [None, Some("bytes=0-1")] {
            let resp = world_get(&node, &xml_cid, Method::GET, range)
                .await
                .unwrap();
            assert_eq!(
                header(&resp, "content-type").unwrap(),
                "application/octet-stream",
                "{range:?}"
            );
        }

        let gif = Bytes::from_static(b"GIF89a\x01\x00\x01\x00");
        node.store(TEST_CID, gif.clone()).await;
        let resp = world_get(&node, TEST_CID, Method::HEAD, None)
            .await
            .unwrap();
        assert_eq!(header(&resp, "content-type").unwrap(), "image/gif");
        // Other bytes under the same key (a test can store them; a real key cannot change).
        node.store(TEST_CID, Bytes::from_static(b"BM\x36\x00\x00\x00"))
            .await;
        let resp = world_get(&node, TEST_CID, Method::GET, None).await.unwrap();
        assert_eq!(header(&resp, "content-type").unwrap(), "image/gif", "kept");
        assert_eq!(
            body_of(resp).await,
            Bytes::from_static(b"BM\x36\x00\x00\x00"),
            "the body is read whole after a kept answer"
        );
    }

    #[tokio::test]
    async fn a_worlds_head_ignores_its_range() {
        let node = TestNode::without_db("content-world-head-range").await;
        node.store(TEST_CID, Bytes::from_static(b"0123456789"))
            .await;
        let resp = world_get(&node, TEST_CID, Method::HEAD, Some("bytes=2-4"))
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        assert_eq!(header(&resp, "content-length").unwrap(), "10");
        assert!(header(&resp, "content-range").is_none());
        let resp = get(&node, Method::HEAD, None, Some("bytes=2-4"))
            .await
            .unwrap();
        assert_eq!(
            resp.status(),
            StatusCode::PARTIAL_CONTENT,
            "the root keeps it"
        );
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

        // worlds-content-server answers a miss with a bare 404.
        for bad in [hex.to_uppercase(), hex[..63].to_string()] {
            let resp = world_get(&node, &bad, Method::GET, None).await.unwrap();
            assert_eq!(resp.status(), StatusCode::NOT_FOUND, "{bad}");
            assert!(body_of(resp).await.is_empty(), "{bad}");
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
        let resp = world_get(&node, &hex, Method::GET, None).await.unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
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

    /// XML, after a BOM or in UTF-16 too, is `application/octet-stream` on GET, HEAD and a range
    /// that holds the declaration: these routes share the node's origin.
    #[tokio::test]
    async fn include_mime_type_serves_xml_as_octet_stream() {
        let utf16 = |bom: [u8; 2], big_endian: bool| {
            let mut bytes = bom.to_vec();
            for c in "<?xml version=\"1.0\"?><svg/>".bytes() {
                bytes.extend_from_slice(&if big_endian { [0, c] } else { [c, 0] });
            }
            bytes
        };
        let cases = [
            b"<?xml version=\"1.0\"?><svg xmlns=\"http://www.w3.org/2000/svg\"/>".to_vec(),
            b"\xEF\xBB\xBF<?xml version=\"1.0\"?><svg/>".to_vec(),
            utf16([0xFE, 0xFF], true),
            utf16([0xFF, 0xFE], false),
        ];
        let node = TestNode::without_db("content-mime-xml").await;
        for mut bytes in cases {
            bytes.resize(1000, b' ');
            let detected = catalyst::file_type(&mut &bytes[..], 1000).await.unwrap();
            assert_eq!(detected, Some("application/xml"), "{:?}", &bytes[..8]);
            let bytes = Bytes::from(bytes);
            let cid = crate::auth::hash_bytes_v1(&bytes);
            node.store(&cid, bytes.clone()).await;
            for (method, range) in [
                (Method::GET, None),
                (Method::HEAD, None),
                (Method::GET, Some("bytes=0-599")),
            ] {
                let resp = get_hash(&node, &cid, method.clone(), Some(""), range)
                    .await
                    .unwrap();
                assert_eq!(
                    header(&resp, "content-type").unwrap(),
                    "application/octet-stream",
                    "{method} {range:?} {:?}",
                    &bytes[..8]
                );
            }
        }
    }

    /// A sniffed type is kept per content key and window: a later GET, HEAD or the same range
    /// does not sniff again (other bytes under the key, which a real key cannot hold, keep the
    /// kept type), and a GET still serves the whole file. A range and the whole file are kept
    /// apart, in either order.
    #[tokio::test]
    async fn include_mime_type_keeps_the_sniffed_type_per_window() {
        let node = TestNode::without_db("content-mime-kept").await;
        // Past `SAMPLE`, so a kept GET serves the head and the rest of the file in one body.
        let file = |head: &[u8]| {
            let mut bytes: Vec<u8> = (0..5000).map(|i| (i % 251) as u8).collect();
            bytes[..head.len()].copy_from_slice(head);
            Bytes::from(bytes)
        };
        let gif = file(b"GIF89a\x01\x00\x01\x00");
        let bmp = file(b"BM\x36\x00\x00\x00");
        let type_of = |resp: &Response| header(resp, "content-type").unwrap();

        node.store(TEST_CID, gif.clone()).await;
        let cut = get(&node, Method::GET, Some(""), Some("bytes=0-1")).await;
        assert_eq!(type_of(&cut.unwrap()), "application/octet-stream");
        let whole = get(&node, Method::GET, Some(""), None).await.unwrap();
        assert_eq!(
            type_of(&whole),
            "image/gif",
            "a range is not the whole file"
        );

        node.store(TEST_CID, bmp.clone()).await;
        let whole = get(&node, Method::GET, Some(""), None).await.unwrap();
        assert_eq!(type_of(&whole), "image/gif", "kept");
        assert_eq!(
            body_of(whole).await,
            bmp,
            "the whole body after a kept type"
        );
        let head = get(&node, Method::HEAD, Some(""), None).await.unwrap();
        assert_eq!(type_of(&head), "image/gif", "kept for HEAD");
        let cut = get(&node, Method::GET, Some(""), Some("bytes=0-1")).await;
        assert_eq!(type_of(&cut.unwrap()), "application/octet-stream", "kept");
        let other = get(&node, Method::GET, Some(""), Some("bytes=0-4")).await;
        let other = other.unwrap();
        assert_eq!(type_of(&other), "image/bmp", "a new window is sniffed");
        assert_eq!(body_of(other).await, bmp.slice(..5));

        let cid = crate::auth::hash_bytes_v1(&gif);
        node.store(&cid, gif.clone()).await;
        let whole = get_hash(&node, &cid, Method::HEAD, Some(""), None).await;
        assert_eq!(type_of(&whole.unwrap()), "image/gif");
        let cut = get_hash(&node, &cid, Method::GET, Some(""), Some("bytes=0-1")).await;
        assert_eq!(
            type_of(&cut.unwrap()),
            "application/octet-stream",
            "a whole file is not a range"
        );
    }

    /// Ranges are kept apart from whole files: more distinct ranges than every cache holds leave
    /// a whole file's kept type in place (other bytes under the key, which a real key cannot
    /// hold, keep the kept type).
    #[tokio::test]
    async fn cycling_ranges_keeps_the_whole_file_type() {
        let node = TestNode::without_db("content-mime-ranges").await;
        let file = |head: &[u8]| {
            let mut bytes = vec![0u8; 5000];
            bytes[..head.len()].copy_from_slice(head);
            Bytes::from(bytes)
        };
        node.store(TEST_CID, file(b"GIF89a\x01\x00\x01\x00")).await;
        let whole = get(&node, Method::HEAD, Some(""), None).await.unwrap();
        assert_eq!(header(&whole, "content-type").unwrap(), "image/gif");

        let mut opened = node.state.storage.open(TEST_CID).await.unwrap().unwrap();
        let ranges = crate::SNIFFED_ENTRIES.max(crate::SNIFFED_RANGES) as u64 + 1;
        for i in 0..ranges {
            let (start, len) = (i % 4000, 1 + i / 4000);
            let mut window = Window::range(&mut opened, start, start + len);
            content_type(&node.state, TEST_CID, &mut window)
                .await
                .unwrap();
        }
        assert_eq!(
            node.state.sniffed_ranges.lock().len(),
            crate::SNIFFED_RANGES
        );

        node.store(TEST_CID, file(b"BM\x36\x00\x00\x00")).await;
        let whole = get(&node, Method::HEAD, Some(""), None).await.unwrap();
        assert_eq!(header(&whole, "content-type").unwrap(), "image/gif", "kept");
    }

    /// Behind the gateway a sniffed GET answers the sniffed type and the redirect, with no body,
    /// whether the type is sniffed or kept.
    #[tokio::test]
    async fn x_accel_base_serves_the_sniffed_type() {
        let mut settings = crate::Settings::for_tests();
        settings.storage_x_accel_base = Some("/__protected_storage".into());
        let node = TestNode::without_db_settings("content-accel-mime", settings).await;
        let mut bytes = vec![0u8; 5000];
        bytes[..6].copy_from_slice(b"GIF89a");
        node.store(TEST_CID, Bytes::from(bytes)).await;
        for pass in ["sniffed", "kept"] {
            let resp = get(&node, Method::GET, Some(""), None).await.unwrap();
            assert_eq!(
                header(&resp, "content-type").unwrap(),
                "image/gif",
                "{pass}"
            );
            assert!(header(&resp, "x-accel-redirect").is_some(), "{pass}");
            assert_eq!(header(&resp, "content-length").unwrap(), "0", "{pass}");
            assert!(body_of(resp).await.is_empty(), "{pass}");
        }
    }

    /// A type a browser opens as a document is `application/octet-stream`: `text/*`, `*/xml`,
    /// `*+xml` and any HTML type. Other types, Office documents included, pass.
    #[test]
    fn a_document_type_is_inert() {
        for mime in [
            "text/vtt",
            "text/calendar",
            "text/vcard",
            "text/html",
            "application/xml",
            "image/svg+xml",
            "application/xhtml+xml",
            "application/vnd.ms-htmlhelp",
        ] {
            assert_eq!(inert(mime), "application/octet-stream", "{mime}");
        }
        for mime in [
            "image/png",
            "application/pdf",
            "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
            "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
            "application/epub+zip",
            "model/3mf",
        ] {
            assert_eq!(inert(mime), mime);
        }
    }

    /// Every document type `file-type` detects is served as `application/octet-stream`, whole
    /// and ranged, with a sandbox policy and `nosniff` on each answer.
    #[tokio::test]
    async fn include_mime_type_serves_document_types_as_octet_stream() {
        let cases: [(&str, &[u8]); 5] = [
            ("text/vtt", b"WEBVTT\n\n00:00.000 --> 00:01.000\nhi\n"),
            ("text/vcard", b"BEGIN:VCARD\r\nVERSION:4.0\r\nEND:VCARD\r\n"),
            ("text/calendar", b"BEGIN:VCALENDAR\r\nEND:VCALENDAR\r\n"),
            (
                "application/vnd.ms-htmlhelp",
                b"ITSF\x03\x00\x00\x00\x60\x00\x00\x00",
            ),
            ("application/xml", b"<?xml version=\"1.0\"?><html/>"),
        ];
        let node = TestNode::without_db("content-mime-documents").await;
        for (detected, bytes) in cases {
            let mut bytes = bytes.to_vec();
            bytes.resize(1000, b' ');
            let found = catalyst::file_type(&mut &bytes[..], 1000).await.unwrap();
            assert_eq!(found, Some(detected));
            let bytes = Bytes::from(bytes);
            let cid = crate::auth::hash_bytes_v1(&bytes);
            node.store(&cid, bytes.clone()).await;
            for (method, range) in [
                (Method::GET, None),
                (Method::HEAD, None),
                (Method::GET, Some("bytes=0-599")),
            ] {
                let resp = get_hash(&node, &cid, method.clone(), Some(""), range)
                    .await
                    .unwrap();
                let at = format!("{detected} {method} {range:?}");
                assert_eq!(
                    header(&resp, "content-type").unwrap(),
                    "application/octet-stream",
                    "{at}"
                );
                assert_eq!(
                    header(&resp, "content-security-policy").unwrap(),
                    "sandbox",
                    "{at}"
                );
                assert_eq!(
                    header(&resp, "x-content-type-options").unwrap(),
                    "nosniff",
                    "{at}"
                );
            }
        }
    }

    /// A ZIP of tiny entries whose sizes follow in data descriptors: the walk fetches each byte
    /// of the file once, not a fresh scan window per entry, and stops at its entry bound.
    #[tokio::test]
    async fn a_zip_of_tiny_descriptor_entries_reads_the_file_once() {
        let mut blob = Vec::new();
        for _ in 0..2000 {
            blob.extend_from_slice(b"PK\x03\x04\x14\0\x08\0");
            blob.extend_from_slice(&[0; 18]);
            blob.extend_from_slice(b"\x01\0\0\0ax");
            blob.extend_from_slice(b"PK\x07\x08");
            blob.extend_from_slice(&[0; 12]);
        }
        // Room for a full scan window after every entry.
        blob.resize(blob.len() + 512 * 1024, 0);
        let blob = Bytes::from(blob);
        let node = TestNode::without_db("content-zip-descriptors").await;
        node.store(TEST_CID, blob.clone()).await;

        let mut opened = node.state.storage.open(TEST_CID).await.unwrap().unwrap();
        let mut window = Window::new(&mut opened, blob.len() as u64);
        assert_eq!(sniff(&mut window).await.unwrap(), "application/zip");
        let fetched = crate::file_type::Source::fetched(&window);
        assert!(fetched < 1 << 20, "fetched {fetched} bytes");

        let resp = get(&node, Method::GET, Some(""), None).await.unwrap();
        assert_eq!(header(&resp, "content-type").unwrap(), "application/zip");
        assert_eq!(body_of(resp).await, blob);
    }

    /// A ZIP walk stops once it has fetched 16 MiB of the file and answers `application/zip`:
    /// within one data-descriptor scan, or across skipped entries before one that decides.
    #[tokio::test]
    async fn a_zip_walk_fetches_at_most_16_mib() {
        // `skipped` stored entries of 64 KiB, then a `mimetype` entry that decides.
        let file = |skipped: usize| {
            let entry = |name: &[u8], data: &[u8]| {
                let mut entry = b"PK\x03\x04\x14\0\0\0\0\0".to_vec();
                entry.extend_from_slice(&[0; 8]);
                entry.extend_from_slice(&(data.len() as u32).to_le_bytes());
                entry.extend_from_slice(&[0; 4]);
                entry.extend_from_slice(&(name.len() as u16).to_le_bytes());
                entry.extend_from_slice(&[0; 2]);
                entry.extend_from_slice(name);
                entry.extend_from_slice(data);
                entry
            };
            let mut bytes = entry(b"a", &[0; 64 << 10]).repeat(skipped);
            bytes.extend_from_slice(&entry(b"mimetype", b"application/epub+zip"));
            Bytes::from(bytes)
        };
        let node = TestNode::without_db("content-zip-budget-entries").await;
        for (skipped, mime) in [(100, "application/epub+zip"), (300, "application/zip")] {
            let blob = file(skipped);
            let cid = crate::auth::hash_bytes_v1(&blob);
            node.store(&cid, blob.clone()).await;
            let mut opened = node.state.storage.open(&cid).await.unwrap().unwrap();
            let mut window = Window::new(&mut opened, blob.len() as u64);
            assert_eq!(sniff(&mut window).await.unwrap(), mime, "{skipped}");
        }

        let mut blob = b"PK\x03\x04\x14\0\x08\0".to_vec();
        blob.extend_from_slice(&[0; 18]);
        blob.extend_from_slice(b"\x01\0\0\0a");
        blob.resize(20 << 20, 0);
        let blob = Bytes::from(blob);
        let node = TestNode::without_db("content-zip-budget").await;
        node.store(TEST_CID, blob.clone()).await;

        let mut opened = node.state.storage.open(TEST_CID).await.unwrap().unwrap();
        let mut window = Window::new(&mut opened, blob.len() as u64);
        assert_eq!(sniff(&mut window).await.unwrap(), "application/zip");
        let fetched = crate::file_type::Source::fetched(&window);
        assert!(fetched <= (16 << 20) + (1 << 20), "fetched {fetched} bytes");
    }
}
