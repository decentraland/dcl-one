//! `WORLDS_WRITES_ENABLED`: the one switch in front of every worlds write.
//!
//! A world publish, an unpublish, a permission or settings edit, a storage
//! value and an operator block are local authority: peer sync does not back
//! them up, and DATABASE.md ("Bootstrap, schema versions and local writes") requires a restore of
//! this node's database, blobs and credentials to be demonstrated before
//! the node accepts them, the rule the social bundle already applies
//! (`social::writes`). Off (the default), a mutating request (`POST`, `PUT`,
//! `PATCH`, `DELETE`) to a worlds router answers 503 with the social error
//! envelope and a `Retry-After`; the reads keep answering.
//!
//! Two non-GET requests are reads and stay open: `POST /entities/active`
//! (the newest scene per named world) and `POST
//! /world/{name}/permissions/{perm}/parcels` (the addresses holding a grant
//! over the parcels); so does `POST /admin/federation/worlds/refresh`, a poll of the
//! admitted peers that writes mirror rows, never user authority. The scene
//! host (`/scene-state`: a WebSocket, an operator kick and a reload) and
//! archipelago (`/ws`, the challenge and heartbeat: admission and presence)
//! are not user authority and are not wrapped.

use axum::Router;
use axum::extract::{Request, State};
use axum::http::{Method, StatusCode, header};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};

use crate::shared::writes::{Refusal, refusal};

pub const DISABLED_MESSAGE: &str = "worlds writes are disabled on this node (WORLDS_WRITES_ENABLED=false): a backup restore has to be demonstrated before this node accepts world publishes, permission edits and storage writes";

/// Whether `method` on `path` (route-local, after any nest prefix) is a
/// write this gate refuses while disabled.
pub fn is_gated(method: &Method, path: &str) -> bool {
    if matches!(*method, Method::GET | Method::HEAD | Method::OPTIONS) {
        return false;
    }
    if *method == Method::POST
        && matches!(
            path,
            "/entities/active" | "/admin/federation/worlds/refresh"
        )
    {
        return false;
    }
    let segments: Vec<&str> = path.trim_start_matches('/').split('/').collect();
    // POST /world/{name}/permissions/{permission}/parcels lists addresses; the
    // per-address `/address/{a}/parcels` form is an edit.
    !(*method == Method::POST
        && segments.len() == 5
        && segments[0] == "world"
        && segments[2] == "permissions"
        && segments[4] == "parcels")
}

/// The 503 the gate answers: the social error envelope with a `Retry-After`.
pub fn disabled_response() -> Response {
    refused_response(&Refusal::Off)
}

/// The 503 for `refusal`: the switch's own message while it is off, the
/// restore stamp that holds it and its age while it is on.
pub fn refused_response(refusal: &Refusal) -> Response {
    let message = refusal.message("worlds", DISABLED_MESSAGE);
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

async fn gate(State(enabled): State<bool>, req: Request, next: Next) -> Response {
    if is_gated(req.method(), req.uri().path())
        && let Some(refusal) = refusal(enabled)
    {
        return refused_response(&refusal);
    }
    next.run(req).await
}

/// Wraps a worlds router in the gate. Applied per router, before nesting,
/// so the path the gate sees is the route-local one.
pub fn guard(router: Router, writes_enabled: bool) -> Router {
    router.layer(middleware::from_fn_with_state(writes_enabled, gate))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::routing::{get, post, put};
    use tower::ServiceExt;

    fn app(enabled: bool) -> Router {
        let inner = Router::new()
            .route("/entities", post(|| async { "published" }))
            .route("/entities/active", post(|| async { "active" }))
            .route(
                "/world/{w}/permissions/{p}/parcels",
                post(|| async { "addresses" }),
            )
            .route(
                "/world/{w}/permissions/{p}/address/{a}/parcels",
                post(|| async { "granted" }),
            )
            .route(
                "/values/{k}",
                get(|| async { "value" }).put(|| async { "stored" }),
            )
            .route(
                "/admin/federation/worlds/refresh",
                post(|| async { "polled" }),
            )
            .route(
                "/admin/federation/worlds/{p}/{w}/hidden",
                put(|| async { "hidden" }),
            );
        Router::new()
            .merge(guard(inner.clone(), enabled))
            .nest("/worlds-content-server", guard(inner, enabled))
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
    fn gated_is_every_mutating_method_but_the_read_by_post_and_presence_paths() {
        assert!(is_gated(&Method::POST, "/entities"));
        assert!(is_gated(&Method::DELETE, "/entities/foo.dcl.eth"));
        assert!(is_gated(&Method::PUT, "/world/foo.dcl.eth/settings"));
        assert!(is_gated(
            &Method::POST,
            "/world/foo.dcl.eth/permissions/access"
        ));
        assert!(is_gated(
            &Method::POST,
            "/world/foo.dcl.eth/permissions/deployment/address/0xabc/parcels"
        ));
        assert!(is_gated(&Method::PUT, "/values/k"));
        assert!(is_gated(&Method::DELETE, "/players"));
        assert!(is_gated(&Method::POST, "/admin/blocked/0xabc"));
        assert!(is_gated(
            &Method::PUT,
            "/admin/federation/worlds/peer/world/hidden"
        ));
        assert!(!is_gated(&Method::GET, "/worlds"));
        assert!(!is_gated(&Method::HEAD, "/status"));
        assert!(!is_gated(&Method::OPTIONS, "/entities"));
        assert!(!is_gated(&Method::POST, "/entities/active"));
        assert!(!is_gated(
            &Method::POST,
            "/world/foo.dcl.eth/permissions/deployment/parcels"
        ));
        assert!(!is_gated(&Method::POST, "/admin/federation/worlds/refresh"));
    }

    #[tokio::test]
    async fn disabled_answers_503_envelope_on_writes_and_serves_reads() {
        let app = app(false);
        let (status, body) = call(&app, "POST", "/entities").await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        let v: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(v["ok"], false);
        assert_eq!(v["error"], "Service Unavailable");
        assert!(
            v["message"]
                .as_str()
                .unwrap()
                .contains("WORLDS_WRITES_ENABLED=false")
        );
        let (status, _) = call(&app, "POST", "/worlds-content-server/entities").await;
        assert_eq!(
            status,
            StatusCode::SERVICE_UNAVAILABLE,
            "the nested copy is gated on its route-local path"
        );
        let (status, _) = call(&app, "PUT", "/values/k").await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        let (status, _) = call(&app, "PUT", "/admin/federation/worlds/p/w/hidden").await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(
            call(&app, "GET", "/values/k").await,
            (StatusCode::OK, "value".to_string())
        );
        assert_eq!(
            call(&app, "POST", "/entities/active").await,
            (StatusCode::OK, "active".to_string())
        );
        assert_eq!(
            call(&app, "POST", "/world/w/permissions/deployment/parcels").await,
            (StatusCode::OK, "addresses".to_string())
        );
        let (status, _) = call(
            &app,
            "POST",
            "/world/w/permissions/deployment/address/0xa/parcels",
        )
        .await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(
            call(&app, "POST", "/admin/federation/worlds/refresh").await,
            (StatusCode::OK, "polled".to_string())
        );
    }

    #[tokio::test]
    async fn enabled_passes_writes_through() {
        let app = app(true);
        assert_eq!(
            call(&app, "POST", "/entities").await,
            (StatusCode::OK, "published".to_string())
        );
        assert_eq!(
            call(&app, "PUT", "/worlds-content-server/values/k").await,
            (StatusCode::OK, "stored".to_string())
        );
    }

    #[test]
    fn the_refusal_carries_retry_after() {
        let res = disabled_response();
        assert_eq!(res.status(), StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(res.headers().get(header::RETRY_AFTER).unwrap(), "3600");
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
            let (status, _) = call(&app, "POST", "/entities").await;
            assert_eq!(status, StatusCode::OK, "fresh stamps let the write through");
            lab.advance(26 * HOUR);
            let (status, body) = call(&app, "POST", "/entities").await;
            assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
            let v: serde_json::Value = serde_json::from_str(&body).unwrap();
            assert_eq!(v["error"], "Service Unavailable");
            assert_eq!(
                v["message"],
                "worlds writes are held on this node: the last complete backup (backup.json) is 27 h old, past WRITES_BACKUP_MAX_AGE_HOURS=26"
            );
            let (status, _) = call(&app, "GET", "/values/k").await;
            assert_eq!(status, StatusCode::OK);
        })
        .await;
    }
}
