use axum::body::Body;
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, Method, StatusCode};
use axum::response::{IntoResponse, Response};
use bytes::Bytes;
use serde::Deserialize;

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
    if first_bytes.len() >= 4 {
        if first_bytes.starts_with(b"RIFF")
            && first_bytes.len() >= 12
            && &first_bytes[8..12] == b"WEBP"
        {
            return "image/webp";
        }
        if first_bytes.starts_with(b"OggS") {
            return "audio/ogg";
        }
        if first_bytes.starts_with(&[0x1A, 0x45, 0xDF, 0xA3]) {
            return "video/webm";
        }
    }
    let trimmed = if first_bytes.starts_with(&[0xEF, 0xBB, 0xBF]) {
        &first_bytes[3..]
    } else {
        first_bytes
    };
    if let Some(&first) = trimmed.first()
        && (first == b'{' || first == b'[')
    {
        return "application/json";
    }

    "application/octet-stream"
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
    if !crate::auth::is_canonical_cid(&hash_id) || state.denylist.contains(&hash_id) {
        return Err(missing(&hash_id));
    }

    if let Some(not_modified_headers) = check_not_modified(&headers, &hash_id) {
        let mut response = StatusCode::NOT_MODIFIED.into_response();
        apply_headers(&mut response, &not_modified_headers);
        return Ok(response);
    }

    let mut opened = match state.storage.open(&hash_id).await? {
        Some(opened) => opened,
        None => match state.read_through() {
            Some(read_through) if read_through.fill_content(&hash_id).await => state
                .storage
                .open(&hash_id)
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
            let content = if method == Method::HEAD && query.include_mime_type.is_none() {
                Bytes::new()
            } else {
                opened.read_range(start, end).await?
            };

            if query.include_mime_type.is_some() {
                set_content_type(&mut base_headers, detect_content_type(&content));
            }

            let body: Bytes = if method == Method::HEAD {
                Bytes::new()
            } else {
                content
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

    if method == Method::HEAD {
        let mut response = StatusCode::OK.into_response();
        apply_headers(&mut response, &base_headers);
        return Ok(response);
    }

    let head = if query.include_mime_type.is_some() {
        let head = opened.read_range(0, 31).await?;
        set_content_type(&mut base_headers, detect_content_type(&head));
        head
    } else {
        Bytes::new()
    };

    if let Some(accel) =
        x_accel_redirect_path(state.settings.storage_x_accel_base.as_deref(), &hash_id)
    {
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

        let resp = get(&node, Method::HEAD, Some(""), None).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
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
}
