//! `GET /worlds-content-server/available-content`: catalyst-content's handler
//! behind worlds-content-server's own query checks and 400 envelope.

use std::sync::Arc;

use axum::Json;
use axum::extract::{Request, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde_json::json;

use crate::content::ContentState;
use crate::content::available_content::get_available_content;
use crate::shared::query_params::{parse_query_string, qs_get_array};

const MAX_AVAILABLE_CONTENT_CIDS: usize = 500;

/// `@dcl/schemas` `IPFSv2.validate`: `^(ba)[a-zA-Z0-9]{57}$`.
fn is_ipfs_v2(cid: &str) -> bool {
    cid.len() == 59 && cid.starts_with("ba") && cid.bytes().all(|b| b.is_ascii_alphanumeric())
}

fn bad_request(message: &str) -> Response {
    (
        StatusCode::BAD_REQUEST,
        Json(json!({ "error": "Bad request", "message": message })),
    )
        .into_response()
}

pub async fn get(State(blobs): State<Arc<ContentState>>, request: Request) -> Response {
    let cids = qs_get_array(
        &parse_query_string(request.uri().query().unwrap_or("")),
        "cid",
    );
    if cids.is_empty() {
        return bad_request("At least one cid query parameter is required.");
    }
    if cids.len() > MAX_AVAILABLE_CONTENT_CIDS {
        return bad_request(&format!(
            "Too many cid query parameters. Maximum allowed is {MAX_AVAILABLE_CONTENT_CIDS}."
        ));
    }
    if !cids.iter().all(|cid| is_ipfs_v2(cid)) {
        return bad_request("Invalid cid format.");
    }
    get_available_content(State(blobs), request)
        .await
        .into_response()
}
