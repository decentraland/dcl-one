//! `CONTENT_WRITES_ENABLED`: the switch in front of the content server's
//! local write path, `POST /entities` and `DELETE /scenes/{coord}`.
//!
//! A local deploy makes this node the authority for an entity no peer has
//! (a local scene carries its `content.local_entities` provenance, an
//! unpublish its tombstone), and peer sync is not a backup of that state,
//! so DATABASE.md ("Bootstrap, schema versions and local writes") requires a restore of this
//! node's database, blobs and credentials to be demonstrated before the
//! switch is turned on. Off (the default), both routes answer 503 with the
//! content error envelope and `Retry-After`; every read keeps answering,
//! `POST /entities/active` included (a read over POST). The gate runs before
//! the body is read, so a body that is not a form at all gets its 400 only
//! on a node that takes deploys (the fixture `create-entity-400-not-multipart`).
//!
//! The console's read-only toggle (`ContentState::read_only`, upstream's
//! `READ_ONLY`) is checked first: upstream does not register the write route
//! in that mode, so both routes answer this router's not-found, the answer a
//! client or forwarder reads as a read-only catalyst.

use axum::http::{StatusCode, header};
use axum::response::Response;

pub const CONTENT_WRITES_ENABLED: &str = "CONTENT_WRITES_ENABLED";

pub const DISABLED_MESSAGE: &str =
    "content writes are disabled on this node (CONTENT_WRITES_ENABLED=false)";

pub const BOOTSTRAPPING_MESSAGE: &str =
    "Deployments are not allowed while the Catalyst is bootstrapping";

/// How long a client should wait before deploying to a bootstrapping node
/// (upstream `middlewares.ts`).
pub const BOOTSTRAPPING_RETRY_AFTER_SECS: &str = "60";

/// How long a client should wait before asking a disabled node again.
pub const RETRY_AFTER_SECS: &str = "3600";

/// How long a client should wait after a deploy this node could not decide.
pub const UNAVAILABLE_RETRY_AFTER_SECS: &str = "30";

pub fn disabled_response() -> Response {
    let mut response = super::errors::json_error(StatusCode::SERVICE_UNAVAILABLE, DISABLED_MESSAGE);
    response.headers_mut().insert(
        header::RETRY_AFTER,
        header::HeaderValue::from_static(RETRY_AFTER_SECS),
    );
    response
}

/// The read-only answer: the router's own not-found, as for a route upstream
/// does not register in `READ_ONLY` mode.
pub fn read_only_response() -> Response {
    super::not_found_response()
}

/// The 503 while the node is bootstrapping, with upstream's `Retry-After`.
pub fn bootstrapping_response() -> Response {
    let mut response =
        super::errors::json_error(StatusCode::SERVICE_UNAVAILABLE, BOOTSTRAPPING_MESSAGE);
    response.headers_mut().insert(
        header::RETRY_AFTER,
        header::HeaderValue::from_static(BOOTSTRAPPING_RETRY_AFTER_SECS),
    );
    response
}

/// The switch alone, for a content write that is not a deploy (the
/// forwarding lane's requeue): `None` lets it through.
pub fn refuse_switch(state: &super::ContentState) -> Option<Response> {
    (!state.writes_enabled()).then(disabled_response)
}

/// Both gates, read-only first: `None` lets the write through.
pub fn refuse(state: &super::ContentState) -> Option<Response> {
    if state.is_read_only() {
        return Some(read_only_response());
    }
    refuse_switch(state)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn the_disabled_answer_is_the_content_envelope_with_retry_after() {
        let resp = disabled_response();
        assert_eq!(resp.status(), StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(resp.headers()[header::RETRY_AFTER], "3600");
        let body = axum::body::to_bytes(resp.into_body(), 4096).await.unwrap();
        let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert!(
            v["error"]
                .as_str()
                .unwrap()
                .contains("CONTENT_WRITES_ENABLED=false")
        );
    }

    #[tokio::test]
    async fn the_read_only_answer_is_the_routers_not_found() {
        let resp = read_only_response();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
        let body = axum::body::to_bytes(resp.into_body(), 4096).await.unwrap();
        assert_eq!(&body[..], b"Not found");
    }

    #[tokio::test]
    async fn the_bootstrapping_answer_carries_upstreams_retry_after() {
        let resp = bootstrapping_response();
        assert_eq!(resp.status(), StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(resp.headers()[header::RETRY_AFTER], "60");
        let body = axum::body::to_bytes(resp.into_body(), 4096).await.unwrap();
        let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(v, serde_json::json!({ "error": BOOTSTRAPPING_MESSAGE }));
    }
}
