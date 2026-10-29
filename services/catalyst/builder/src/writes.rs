// from: src/social/writes.rs (the gate, its 503 envelope and its tests, with the creator-plane switch and message)
//! `CREATE_WRITES_ENABLED`: the one switch in front of every creator-plane
//! write (the builder drafts, uploads, curation, publication state and the
//! newsletter opt-in; the camera-reel uploads, visibility and moderation
//! share it). Like the social slice, this is local authority peer sync does
//! not back up, so DATABASE.md ("Bootstrap, schema versions and local writes") requires a restore
//! of this node's database, blobs and credentials to be demonstrated before
//! the switch is turned on in production. Off (the default), every
//! `POST`/`PUT`/`PATCH`/`DELETE` answers 503 with the builder envelope and a
//! `Retry-After`; the reads (drafts, catalogue, blobs, on-chain lookups)
//! keep answering. Nothing is exempt: the builder has no admission mint or
//! stateless proxy on a mutating method. The middleware itself is shared:
//! [`Gate`] carries a router's open paths and its own refusal envelope, so
//! `camera_reel::writes` applies the same switch with the camera-reel
//! `{message}` envelope and its one read-shaped `POST` kept open.

use axum::Router;
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};

pub const DISABLED_MESSAGE: &str = "creator writes are disabled on this node (CREATE_WRITES_ENABLED=false): a backup restore has to be demonstrated before this node accepts user writes";

pub use crate::shared::writes::{Gate, Refusal, guard_with, is_gated};

/// The 503 the gate answers: the builder error envelope with a `Retry-After`
/// so a client does not hammer a node that will not change its mind soon.
pub fn disabled_response() -> Response {
    refused_response(&Refusal::Off)
}

/// The 503 for `refusal`: the switch's own message while it is off, the
/// restore stamp that holds it and its age while it is on.
pub fn refused_response(refusal: &Refusal) -> Response {
    let message = refusal.message("creator", DISABLED_MESSAGE);
    let body =
        serde_json::json!({ "ok": false, "error": "Service Unavailable", "message": message });
    (
        StatusCode::SERVICE_UNAVAILABLE,
        [
            (header::CONTENT_TYPE, "application/json"),
            (header::RETRY_AFTER, "3600"),
        ],
        body.to_string(),
    )
        .into_response()
}

/// The builder's gate: nothing open, the builder envelope.
pub fn guard(router: Router, writes_enabled: bool) -> Router {
    guard_with(
        router,
        Gate {
            enabled: writes_enabled,
            open_while_disabled: &[],
            refuse: refused_response,
            blobs: true,
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shared::test_http::call;
    use axum::http::Method;
    use axum::routing::{get, post};

    fn app(enabled: bool) -> Router {
        let inner = Router::new()
            .route("/v1/collections", get(|| async { "list" }))
            .route("/v1/newsletter", post(|| async { "subscribed" }))
            .route(
                "/v1/collections/{id}",
                get(|| async { "one" }).put(|| async { "saved" }),
            );
        Router::new()
            .merge(guard(inner.clone(), enabled))
            .nest("/builder-api", guard(inner, enabled))
    }

    #[test]
    fn gated_is_every_mutating_method() {
        assert!(is_gated(&Method::POST));
        assert!(is_gated(&Method::PUT));
        assert!(is_gated(&Method::PATCH));
        assert!(is_gated(&Method::DELETE));
        assert!(!is_gated(&Method::GET));
        assert!(!is_gated(&Method::HEAD));
        assert!(!is_gated(&Method::OPTIONS));
    }

    #[tokio::test]
    async fn disabled_answers_503_envelope_on_writes_and_serves_reads() {
        let app = app(false);
        let (status, body) = call(&app, "PUT", "/v1/collections/x").await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        let v: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(v["ok"], false);
        assert_eq!(v["error"], "Service Unavailable");
        assert!(
            v["message"]
                .as_str()
                .unwrap()
                .contains("CREATE_WRITES_ENABLED=false")
        );
        let (status, _) = call(&app, "POST", "/builder-api/v1/newsletter").await;
        assert_eq!(
            status,
            StatusCode::SERVICE_UNAVAILABLE,
            "the nested copy is gated too"
        );
        assert_eq!(
            call(&app, "GET", "/v1/collections").await,
            (StatusCode::OK, "list".to_string())
        );
        assert_eq!(
            call(&app, "GET", "/builder-api/v1/collections/x").await,
            (StatusCode::OK, "one".to_string())
        );
    }

    #[tokio::test]
    async fn enabled_passes_writes_through() {
        let app = app(true);
        assert_eq!(
            call(&app, "PUT", "/v1/collections/x").await,
            (StatusCode::OK, "saved".to_string())
        );
        assert_eq!(
            call(&app, "POST", "/builder-api/v1/newsletter").await,
            (StatusCode::OK, "subscribed".to_string())
        );
    }

    #[tokio::test]
    async fn a_gate_with_open_paths_and_its_own_envelope() {
        fn refuse(_: &Refusal) -> Response {
            (
                StatusCode::SERVICE_UNAVAILABLE,
                [(header::RETRY_AFTER, "1")],
                "{\"message\":\"off\"}",
            )
                .into_response()
        }
        let gate = Gate {
            enabled: false,
            open_while_disabled: &["/api/places/images"],
            refuse,
            blobs: true,
        };
        assert!(gate.refuses(&Method::POST, "/api/images"));
        assert!(!gate.refuses(&Method::POST, "/api/places/images"));
        assert!(!gate.refuses(&Method::GET, "/api/images"));
        assert!(
            !Gate {
                enabled: true,
                ..gate
            }
            .refuses(&Method::POST, "/api/images")
        );
        let inner = Router::new()
            .route("/api/images", post(|| async { "uploaded" }))
            .route("/api/places/images", post(|| async { "gallery" }));
        let app = Router::new().nest("/camera-reel-service", guard_with(inner, gate));
        let (status, body) = call(&app, "POST", "/camera-reel-service/api/images").await;
        assert_eq!(
            (status, body.as_str()),
            (StatusCode::SERVICE_UNAVAILABLE, "{\"message\":\"off\"}")
        );
        assert_eq!(
            call(&app, "POST", "/camera-reel-service/api/places/images").await,
            (StatusCode::OK, "gallery".to_string())
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
            lab::{HOUR, StampLab},
            scoped,
        };
        let lab = StampLab::fresh();
        let app = app(true);
        scoped(lab.stamps.clone(), async {
            let (status, _) = call(&app, "POST", "/builder-api/v1/newsletter").await;
            assert_eq!(status, StatusCode::OK, "fresh stamps let the write through");
            lab.advance(26 * HOUR);
            let (status, body) = call(&app, "POST", "/builder-api/v1/newsletter").await;
            assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
            let v: serde_json::Value = serde_json::from_str(&body).unwrap();
            assert_eq!(
                v["message"],
                "creator writes are held on this node: the last complete backup (backup.json) is 27 h old, past WRITES_BACKUP_MAX_AGE_HOURS=26"
            );
            let (status, _) = call(&app, "GET", "/builder-api/v1/collections").await;
            assert_eq!(status, StatusCode::OK);
        })
        .await;
    }
}
