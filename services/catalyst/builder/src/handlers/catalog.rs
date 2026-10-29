use axum::extract::{Path, State};
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};

use crate::SharedState;
use crate::catalog::{ContentError, UpstreamError};
use crate::http::ApiError;

pub const UNBUILT: &str =
    "no asset catalogue in builder.asset_catalog: run `catalyst builder catalog <packs-dir>`";

pub async fn get_asset_packs(State(state): State<SharedState>) -> Response {
    let body = match state.catalog.asset_packs().await {
        Ok(Some(b)) => b,
        Ok(None) => return ApiError::http(503, UNBUILT).into_response(),
        Err(e) => {
            tracing::error!(error = %e, "builder catalog load failed");
            return ApiError::internal("builder catalog load failed").into_response();
        }
    };
    let mut resp = (StatusCode::OK, body).into_response();
    let headers = resp.headers_mut();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/json"),
    );
    headers.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("public, max-age=3600"),
    );
    resp
}

const RETRY_AFTER_SECS: &str = "30";

fn upstream_failure(hash: &str, e: &UpstreamError) -> Response {
    let (status, message) = match e {
        UpstreamError::Status(s) => (
            502,
            format!("builder content bucket answered {s} for {hash}"),
        ),
        UpstreamError::Unreachable(_) => (
            503,
            format!("builder content bucket unreachable while pulling {hash}"),
        ),
        UpstreamError::Rejected(_) => (
            502,
            format!("builder content bucket served unusable bytes for {hash}"),
        ),
    };
    let mut resp = ApiError::http(status, message).into_response();
    resp.headers_mut().insert(
        header::RETRY_AFTER,
        HeaderValue::from_static(RETRY_AFTER_SECS),
    );
    resp
}

/// The catalogue file, a builder upload or a pull-through from the content
/// bucket, by hash; a hash the node withholds (a rejected or deleted
/// camera-reel photo, `content::Denylist`) is a 404 like a miss.
pub async fn get_content(State(state): State<SharedState>, Path(hash): Path<String>) -> Response {
    if state.denylist.contains(&hash) {
        return ApiError::not_found(format!("no builder item content for {hash}")).into_response();
    }
    let content = match state.catalog.content(&hash).await {
        Ok(c) => c,
        Err(ContentError::InvalidHash) => {
            return ApiError::bad_request("hash is not a canonical CID").into_response();
        }
        Err(ContentError::NotFound) => {
            return ApiError::not_found(format!("no builder item content for {hash}"))
                .into_response();
        }
        Err(ContentError::Upstream(e)) => {
            tracing::warn!(error = %e, hash, "builder content pull-through failed");
            return upstream_failure(&hash, &e);
        }
        Err(ContentError::Io(e)) => {
            tracing::error!(error = %e, hash, "builder content store read failed");
            return ApiError::internal("builder content store read failed").into_response();
        }
    };
    let mut resp = (StatusCode::OK, content.bytes).into_response();
    let headers = resp.headers_mut();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static(content.content_type),
    );
    headers.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("public, max-age=31536000, immutable"),
    );
    if let Ok(etag) = HeaderValue::from_str(&format!("\"{hash}\"")) {
        headers.insert(header::ETAG, etag);
    }
    resp
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::write_catalog;
    use crate::tests::support::{state_with_db, state_without_db};
    use axum::body::to_bytes;
    use dcl_auth::hash_bytes_v1;
    use serde_json::{Value, json};
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    async fn body_json(resp: Response) -> Value {
        let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        serde_json::from_slice(&bytes).unwrap()
    }

    /// builder.asset_catalog is one row in the shared scratch database; the
    /// tests that write it hold this for their whole run.
    static CATALOG_ROW: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

    #[tokio::test]
    async fn asset_packs_is_503_until_the_catalog_is_built() {
        let _row = CATALOG_ROW.lock().await;
        let Some(st) = state_with_db(None).await else {
            return;
        };
        sqlx::query("DELETE FROM builder.asset_catalog")
            .execute(st.items.pool())
            .await
            .unwrap();
        let resp = get_asset_packs(State(st)).await;
        assert_eq!(resp.status(), StatusCode::SERVICE_UNAVAILABLE);
        let v = body_json(resp).await;
        assert!(
            v["message"]
                .as_str()
                .unwrap()
                .contains("catalyst builder catalog")
        );
    }

    #[tokio::test]
    async fn asset_packs_serves_the_builder_server_envelope_and_content_by_cid() {
        let _row = CATALOG_ROW.lock().await;
        let Some(st) = state_with_db(None).await else {
            return;
        };
        let model = b"glTF\x02\x00\x00\x00fake".to_vec();
        let hash = hash_bytes_v1(&model);
        st.items
            .blobs
            .store(&hash, model.clone().into())
            .await
            .unwrap();
        let catalog = json!([{
            "id": "pack-1", "title": "Pack", "assets": [{
                "id": "a1", "name": "Cube", "model": "cube.glb",
                "contents": { "cube.glb": hash }
            }]
        }]);
        write_catalog(st.items.pool(), catalog, "test")
            .await
            .unwrap();

        let resp = get_asset_packs(State(st.clone())).await;
        assert_eq!(resp.status(), StatusCode::OK);
        assert_eq!(
            resp.headers()[header::CACHE_CONTROL],
            "public, max-age=3600"
        );
        let v = body_json(resp).await;
        assert_eq!(v["ok"], json!(true));
        assert_eq!(
            v["data"][0]["assets"][0]["contents"]["cube.glb"],
            json!(hash)
        );

        let resp = get_content(State(st.clone()), Path(hash.clone())).await;
        assert_eq!(resp.status(), StatusCode::OK);
        assert_eq!(resp.headers()[header::CONTENT_TYPE], "model/gltf-binary");
        assert_eq!(resp.headers()[header::ETAG], format!("\"{hash}\""));
        let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        assert_eq!(bytes.as_ref(), model.as_slice());

        let png = b"\x89PNG\r\n\x1a\nrest".to_vec();
        let png_hash = dcl_auth::hash_bytes(&png);
        st.items.blobs.store(&png_hash, png.into()).await.unwrap();
        let resp = get_content(State(st.clone()), Path(png_hash)).await;
        assert_eq!(
            resp.headers()[header::CONTENT_TYPE],
            "image/png",
            "a file outside the catalogue is sniffed"
        );

        let resp = get_content(State(st.clone()), Path("not-a-cid".into())).await;
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
        let missing = hash_bytes_v1(b"absent");
        let resp = get_content(State(st.clone()), Path(missing)).await;
        assert_eq!(
            resp.status(),
            StatusCode::NOT_FOUND,
            "no pull-through without a bucket"
        );
        sqlx::query("DELETE FROM builder.asset_catalog")
            .execute(st.items.pool())
            .await
            .unwrap();
    }

    async fn bucket(hits: Arc<AtomicUsize>, status: StatusCode, served: &'static [u8]) -> String {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let app = axum::Router::new().route(
            "/contents/{hash}",
            axum::routing::get(move |Path(_): Path<String>| {
                let hits = hits.clone();
                async move {
                    hits.fetch_add(1, Ordering::SeqCst);
                    (status, served.to_vec())
                }
            }),
        );
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        format!("http://{addr}/")
    }

    const GLB: &[u8] = b"glTF\x02\x00\x00\x00model-bytes";

    #[tokio::test]
    async fn content_pulls_through_into_the_store_and_serves_from_it_afterwards() {
        let Some(st) = state_with_db(None).await else {
            return;
        };
        let hits = Arc::new(AtomicUsize::new(0));
        let st = st
            .with_bucket(bucket(hits.clone(), StatusCode::OK, GLB).await)
            .await;
        let hash = hash_bytes_v1(GLB);
        let _ = st.items.blobs.delete(&hash).await;

        let resp = get_content(State(st.clone()), Path(hash.clone())).await;
        assert_eq!(resp.status(), StatusCode::OK);
        assert_eq!(resp.headers()[header::CONTENT_TYPE], "model/gltf-binary");
        assert_eq!(
            to_bytes(resp.into_body(), usize::MAX)
                .await
                .unwrap()
                .as_ref(),
            GLB
        );
        assert!(st.items.blobs.exist(&hash).await.unwrap());

        let resp = get_content(State(st.clone()), Path(hash.clone())).await;
        assert_eq!(resp.status(), StatusCode::OK);
        assert_eq!(
            hits.load(Ordering::SeqCst),
            1,
            "the second read is served from the store"
        );

        let other = hash_bytes_v1(b"something else");
        let resp = get_content(State(st.clone()), Path(other.clone())).await;
        assert_eq!(
            resp.status(),
            StatusCode::BAD_GATEWAY,
            "bytes that do not hash to the CID are refused"
        );
        assert_eq!(resp.headers()[header::RETRY_AFTER], RETRY_AFTER_SECS);
        assert!(!st.items.blobs.exist(&other).await.unwrap());
        let _ = st.items.blobs.delete(&hash).await;
    }

    #[tokio::test]
    async fn upstream_5xx_is_a_502_with_retry_after_not_a_404() {
        let st = state_without_db(None).await;
        let st = st
            .with_bucket(
                bucket(
                    Arc::new(AtomicUsize::new(0)),
                    StatusCode::BAD_GATEWAY,
                    b"upstream down",
                )
                .await,
            )
            .await;
        let hash = hash_bytes_v1(b"not local");
        let resp = get_content(State(st), Path(hash.clone())).await;
        assert_eq!(resp.status(), StatusCode::BAD_GATEWAY);
        assert_eq!(resp.headers()[header::RETRY_AFTER], RETRY_AFTER_SECS);
        let v = body_json(resp).await;
        assert!(v["message"].as_str().unwrap().contains("answered 502"));
    }

    #[tokio::test]
    async fn unreachable_bucket_is_a_503_with_retry_after() {
        let st = state_without_db(None).await;
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let dead = format!("http://{}", listener.local_addr().unwrap());
        drop(listener);
        let st = st.with_bucket(dead).await;
        let hash = hash_bytes_v1(b"not local");
        let resp = get_content(State(st), Path(hash)).await;
        assert_eq!(resp.status(), StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(resp.headers()[header::RETRY_AFTER], RETRY_AFTER_SECS);
    }

    #[tokio::test]
    async fn a_miss_in_the_item_bucket_pulls_from_the_asset_packs_bucket() {
        let Some(st) = state_with_db(None).await else {
            return;
        };
        let items = Arc::new(AtomicUsize::new(0));
        let packs = Arc::new(AtomicUsize::new(0));
        let st = st
            .with_buckets(vec![
                bucket(items.clone(), StatusCode::FORBIDDEN, b"AccessDenied").await,
                bucket(packs.clone(), StatusCode::OK, GLB).await,
            ])
            .await;
        let hash = dcl_auth::hash_bytes_v0_unixfs(GLB);
        let _ = st.items.blobs.delete(&hash).await;

        let resp = get_content(State(st.clone()), Path(hash.clone())).await;
        assert_eq!(resp.status(), StatusCode::OK, "a template Qm model");
        assert_eq!(
            to_bytes(resp.into_body(), usize::MAX)
                .await
                .unwrap()
                .as_ref(),
            GLB
        );
        assert!(st.items.blobs.exist(&hash).await.unwrap());
        let resp = get_content(State(st.clone()), Path(hash.clone())).await;
        assert_eq!(resp.status(), StatusCode::OK);
        assert_eq!(
            (items.load(Ordering::SeqCst), packs.load(Ordering::SeqCst)),
            (1, 1),
            "stored after the first pull"
        );
        let _ = st.items.blobs.delete(&hash).await;
    }

    #[tokio::test]
    async fn every_bucket_missing_is_a_cached_404_and_a_failure_is_not_masked() {
        let st = state_without_db(None).await;
        let items = Arc::new(AtomicUsize::new(0));
        let packs = Arc::new(AtomicUsize::new(0));
        let both = st
            .with_buckets(vec![
                bucket(items.clone(), StatusCode::FORBIDDEN, b"").await,
                bucket(packs.clone(), StatusCode::NOT_FOUND, b"").await,
            ])
            .await;
        let hash = hash_bytes_v1(b"in neither bucket");
        for _ in 0..2 {
            let resp = get_content(State(both.clone()), Path(hash.clone())).await;
            assert_eq!(resp.status(), StatusCode::NOT_FOUND);
        }
        assert_eq!(
            (items.load(Ordering::SeqCst), packs.load(Ordering::SeqCst)),
            (1, 1)
        );

        let down = st
            .with_buckets(vec![
                bucket(Arc::new(AtomicUsize::new(0)), StatusCode::BAD_GATEWAY, b"").await,
                bucket(Arc::new(AtomicUsize::new(0)), StatusCode::NOT_FOUND, b"").await,
            ])
            .await;
        let resp = get_content(State(down), Path(hash)).await;
        assert_eq!(
            resp.status(),
            StatusCode::BAD_GATEWAY,
            "a failing bucket is not a miss"
        );
    }

    #[tokio::test]
    async fn upstream_404_is_a_cached_miss() {
        let st = state_without_db(None).await;
        let hits = Arc::new(AtomicUsize::new(0));
        let st = st
            .with_bucket(bucket(hits.clone(), StatusCode::NOT_FOUND, b"").await)
            .await;
        let hash = hash_bytes_v1(b"nowhere");
        for _ in 0..3 {
            let resp = get_content(State(st.clone()), Path(hash.clone())).await;
            assert_eq!(resp.status(), StatusCode::NOT_FOUND);
            assert!(resp.headers().get(header::RETRY_AFTER).is_none());
        }
        assert_eq!(hits.load(Ordering::SeqCst), 1);
    }
}
