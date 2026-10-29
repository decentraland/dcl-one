// from: src/social/writes.rs (the gate, applied to the creator plane)
//! `CREATE_WRITES_ENABLED`: the one switch in front of every creator write.
//!
//! The camera reel is local authority for what players upload (their
//! photos, visibility and the moderator's verdicts), and peer sync is not a
//! backup of that state, so README.md ("Acceptance and rollout") requires a
//! restore of this node's database, blobs and credentials to be demonstrated
//! before the switch is turned on. Off (the default), a mutating request
//! (`POST`, `PUT`, `PATCH`, `DELETE`) answers 503 with the camera-reel error
//! envelope and a `Retry-After`; the reads keep answering. The one `POST`
//! that is a read, the batch place gallery `/api/places/images`, stays open.
//! The middleware is `builder::writes` ([`Gate`]): one switch, one
//! implementation, this router's open path and envelope.

use axum::Router;
use axum::http::{Method, StatusCode, header};
use axum::response::{IntoResponse, Response};

use super::http::ResponseError;
use crate::shared::writes::{Gate, Refusal, guard_with};

/// The paths (as this router sees them, after any nest prefix is stripped)
/// whose non-GET methods stay open while writes are disabled.
const OPEN_WHILE_DISABLED: &[&str] = &["/api/places/images"];

pub const DISABLED_MESSAGE: &str = "creator writes are disabled on this node (CREATE_WRITES_ENABLED=false): a backup restore has to be demonstrated before this node accepts user writes";

fn gate(writes_enabled: bool) -> Gate {
    Gate {
        enabled: writes_enabled,
        open_while_disabled: OPEN_WHILE_DISABLED,
        refuse: refused_response,
        blobs: true,
    }
}

/// Whether `method` on `path` is a write this gate refuses while disabled.
pub fn is_gated(method: &Method, path: &str) -> bool {
    gate(false).refuses(method, path)
}

/// The 503 the gate answers: the camera-reel `{message}` envelope with a
/// `Retry-After` so a client does not hammer a node that will not change
/// its mind soon.
pub fn disabled_response() -> Response {
    refused_response(&Refusal::Off)
}

/// The 503 for `refusal`: the switch's own message while it is off, the
/// restore stamp that holds it and its age while it is on.
pub fn refused_response(refusal: &Refusal) -> Response {
    let message = refusal.message("creator", DISABLED_MESSAGE);
    let body =
        serde_json::to_string(&ResponseError::new(message)).expect("a string field serializes");
    (
        StatusCode::SERVICE_UNAVAILABLE,
        [
            (header::CONTENT_TYPE, "application/json"),
            (header::RETRY_AFTER, "3600"),
        ],
        body,
    )
        .into_response()
}

/// Wraps the router in the gate. Applied before `main.rs` nests it, so the
/// path the gate sees is the route-local one.
pub fn guard(router: Router, writes_enabled: bool) -> Router {
    guard_with(router, gate(writes_enabled))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::extract::Request;
    use axum::routing::{get, post};
    use tower::ServiceExt;

    fn app(enabled: bool) -> Router {
        let inner = Router::new()
            .route("/api/images", post(|| async { "uploaded" }))
            .route(
                "/api/images/{id}",
                get(|| async { "bytes" }).delete(|| async { "deleted" }),
            )
            .route("/api/places/images", post(|| async { "gallery" }));
        Router::new().nest("/camera-reel-service", guard(inner, enabled))
    }

    async fn call(app: &Router, method: &str, path: &str) -> (StatusCode, String) {
        let res = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(method)
                    .uri(path)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let status = res.status();
        let body = axum::body::to_bytes(res.into_body(), 1 << 16)
            .await
            .unwrap();
        (status, String::from_utf8_lossy(&body).into_owned())
    }

    #[test]
    fn gated_is_every_mutating_method_but_the_batch_gallery() {
        assert!(is_gated(&Method::POST, "/api/images"));
        assert!(is_gated(&Method::POST, "/api/images-json"));
        assert!(is_gated(&Method::PATCH, "/api/images/x/visibility"));
        assert!(is_gated(&Method::DELETE, "/api/images/x"));
        assert!(is_gated(&Method::PATCH, "/admin/images/x/review"));
        assert!(is_gated(&Method::DELETE, "/admin/images/x"));
        assert!(!is_gated(&Method::GET, "/api/users/x/images"));
        assert!(!is_gated(&Method::HEAD, "/health"));
        assert!(!is_gated(&Method::OPTIONS, "/api/images"));
        assert!(
            !is_gated(&Method::POST, "/api/places/images"),
            "the batch gallery is a read"
        );
        assert!(
            is_gated(&Method::POST, "/api/places/images/x"),
            "only the exact path is open"
        );
    }

    #[tokio::test]
    async fn disabled_answers_503_envelope_on_writes_and_serves_reads() {
        let app = app(false);
        let (status, body) = call(&app, "POST", "/camera-reel-service/api/images").await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        let v: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(
            v.as_object().unwrap().keys().collect::<Vec<_>>(),
            vec!["message"],
            "the camera-reel envelope"
        );
        assert!(
            v["message"]
                .as_str()
                .unwrap()
                .contains("CREATE_WRITES_ENABLED=false")
        );
        let (status, _) = call(&app, "DELETE", "/camera-reel-service/api/images/x").await;
        assert_eq!(
            status,
            StatusCode::SERVICE_UNAVAILABLE,
            "gated on the route-local path"
        );
        assert_eq!(
            call(&app, "GET", "/camera-reel-service/api/images/x").await,
            (StatusCode::OK, "bytes".to_string())
        );
        assert_eq!(
            call(&app, "POST", "/camera-reel-service/api/places/images").await,
            (StatusCode::OK, "gallery".to_string())
        );
    }

    #[tokio::test]
    async fn enabled_passes_writes_through() {
        let app = app(true);
        assert_eq!(
            call(&app, "POST", "/camera-reel-service/api/images").await,
            (StatusCode::OK, "uploaded".to_string())
        );
    }

    #[test]
    fn the_refusal_carries_retry_after() {
        let res = disabled_response();
        assert_eq!(res.status(), StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(res.headers().get(header::RETRY_AFTER).unwrap(), "3600");
        assert_eq!(
            res.headers().get(header::CONTENT_TYPE).unwrap(),
            "application/json"
        );
    }

    /// A switch that is on still holds writes while a restore stamp is
    /// stale, and the 503 names the stamp and its age; reads keep answering.
    #[tokio::test]
    async fn a_stale_restore_stamp_holds_writes_and_names_it() {
        use crate::shared::writes::restore::{
            lab::{DAY, StampLab},
            scoped,
        };
        let lab = StampLab::fresh();
        let app = app(true);
        scoped(lab.stamps.clone(), async {
            let (status, _) = call(&app, "POST", "/camera-reel-service/api/images").await;
            assert_eq!(status, StatusCode::OK, "fresh stamps let the write through");
            lab.write_aged(crate::shared::writes::restore::Stamp::Drill, 9 * DAY);
            let (status, body) = call(&app, "POST", "/camera-reel-service/api/images").await;
            assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
            let v: serde_json::Value = serde_json::from_str(&body).unwrap();
            assert_eq!(
                v["message"],
                "creator writes are held on this node: the last passing restore drill (drill.json) is 9 d old, past WRITES_DRILL_MAX_AGE_DAYS=8"
            );
            let (status, _) = call(&app, "GET", "/camera-reel-service/api/images/x").await;
            assert_eq!(status, StatusCode::OK);
        })
        .await;
    }
}
