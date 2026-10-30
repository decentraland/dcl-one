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
//! `POST /entities/active` included (a read over POST). The gate runs after
//! the multipart extraction, so a request that is not a deploy at all
//! still gets its 400 (the stage-1 fixture `create-entity-400-not-multipart`).
//!
//! On, the switch still waits for the restore stamps (`shared::writes`).
//! The console's read-only toggle (`ContentState::read_only`, the old
//! `READ_ONLY`) is the second gate, the old 403, checked after this one.

use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};

use crate::shared::writes::{Refusal, refusal};

pub const CONTENT_WRITES_ENABLED: &str = "CONTENT_WRITES_ENABLED";

pub const DISABLED_MESSAGE: &str = "content writes are disabled on this node (CONTENT_WRITES_ENABLED=false): a backup restore has to be demonstrated before this node accepts local deployments";

/// The old read-only gate's answer, byte for byte.
pub const READ_ONLY_MESSAGE: &str = "Content Server is in read-only mode; deployments are disabled";

pub const BOOTSTRAPPING_MESSAGE: &str =
    "Deployments are not allowed while the Catalyst is bootstrapping";

/// How long a client should wait before asking a disabled node again.
pub const RETRY_AFTER_SECS: &str = "3600";

/// How long a client should wait after a deploy this node could not decide.
pub const UNAVAILABLE_RETRY_AFTER_SECS: &str = "30";

pub fn disabled_response() -> Response {
    refused_response(&Refusal::Off)
}

/// The 503 for `refusal`: the switch's own message while it is off, the
/// restore stamp that holds it and its age while it is on.
pub fn refused_response(refusal: &Refusal) -> Response {
    let message = refusal.message("content", DISABLED_MESSAGE);
    let mut response = super::errors::json_error(StatusCode::SERVICE_UNAVAILABLE, &message);
    response.headers_mut().insert(
        header::RETRY_AFTER,
        header::HeaderValue::from_static(RETRY_AFTER_SECS),
    );
    response
}

pub fn read_only_response() -> Response {
    (StatusCode::FORBIDDEN, READ_ONLY_MESSAGE).into_response()
}

/// The switch and the restore stamps alone, for a content write that is
/// not a deploy (the forwarding lane's requeue): `None` lets it through.
pub fn refuse_switch(state: &super::ContentState) -> Option<Response> {
    refusal(state.writes_enabled()).map(|refusal| refused_response(&refusal))
}

/// Both gates, in order: `None` lets the write through.
pub fn refuse(state: &super::ContentState) -> Option<Response> {
    if let Some(refused) = refuse_switch(state) {
        return Some(refused);
    }
    if state.is_read_only() {
        return Some(read_only_response());
    }
    None
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
    async fn the_read_only_answer_is_the_old_plain_403() {
        let resp = read_only_response();
        assert_eq!(resp.status(), StatusCode::FORBIDDEN);
        let body = axum::body::to_bytes(resp.into_body(), 4096).await.unwrap();
        assert_eq!(&body[..], READ_ONLY_MESSAGE.as_bytes());
    }
}
