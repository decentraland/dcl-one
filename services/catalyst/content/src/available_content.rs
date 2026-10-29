use axum::Json;
use axum::extract::{Request, State};
use axum::response::IntoResponse;

use super::SharedState;
use super::errors::{AppError, AppResult};
use super::query_params::{parse_query_string, qs_get_array};
use super::wire_types::AvailableContentItem;

const MAX_AVAILABLE_CONTENT_CIDS: usize = 1000;

/// Asking is always safe: a bogus or unknown cid is `available: false`, in
/// input order; only a storage fault aborts the batch.
pub async fn get_available_content(
    State(state): State<SharedState>,
    request: Request,
) -> AppResult<impl IntoResponse> {
    let params = parse_query_string(request.uri().query().unwrap_or(""));
    let cids = qs_get_array(&params, "cid");

    if cids.is_empty() {
        return Err(AppError::invalid("Please set at least one cid."));
    }
    if cids.len() > MAX_AVAILABLE_CONTENT_CIDS {
        return Err(AppError::invalid(format!(
            "Too many cids requested; the maximum allowed is {MAX_AVAILABLE_CONTENT_CIDS}."
        )));
    }

    let available_cids: Vec<String> = cids
        .into_iter()
        .filter(|cid| !state.denylist.contains(cid))
        .collect();

    let existence = state.storage.exist_multiple(&available_cids).await?;

    let result: Vec<AvailableContentItem> = available_cids
        .iter()
        .map(|cid| AvailableContentItem {
            cid: cid.clone(),
            available: existence.get(cid).copied().unwrap_or(false),
        })
        .collect();

    Ok(Json(result))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::support::{TEST_CID, TestNode};
    use axum::http::StatusCode;
    use bytes::Bytes;
    use serde_json::json;

    fn request_for(cid: &str) -> Request {
        Request::builder()
            .uri(format!("/available-content?cid={cid}"))
            .body(axum::body::Body::empty())
            .unwrap()
    }

    async fn body(response: axum::response::Response) -> serde_json::Value {
        let bytes = axum::body::to_bytes(response.into_body(), 4096)
            .await
            .unwrap();
        serde_json::from_slice(&bytes).unwrap()
    }

    #[tokio::test]
    async fn storage_fault_errors_instead_of_answering_absent() {
        let node = TestNode::without_db("available-fault").await;
        node.store(TEST_CID, Bytes::from_static(b"x")).await;
        node.destroy_shard(TEST_CID).await;
        let err =
            match get_available_content(State(node.state.clone()), request_for(TEST_CID)).await {
                Ok(_) => panic!("a storage fault must error, not report \"available\": false"),
                Err(err) => err,
            };
        assert_eq!(
            err.into_response().status(),
            StatusCode::INTERNAL_SERVER_ERROR
        );
    }

    #[tokio::test]
    async fn missing_content_reports_available_false() {
        let node = TestNode::without_db("available-miss").await;
        let response = get_available_content(State(node.state.clone()), request_for(TEST_CID))
            .await
            .expect("a provable miss resolves normally")
            .into_response();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            body(response).await,
            json!([{ "cid": TEST_CID, "available": false }])
        );
    }

    #[tokio::test]
    async fn stored_content_reports_available_true() {
        let node = TestNode::without_db("available-hit").await;
        node.store(TEST_CID, Bytes::from_static(b"x")).await;
        let response = get_available_content(State(node.state.clone()), request_for(TEST_CID))
            .await
            .unwrap()
            .into_response();
        assert_eq!(
            body(response).await,
            json!([{ "cid": TEST_CID, "available": true }])
        );
    }
}
