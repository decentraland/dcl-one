//! `SOCIAL_WRITES_ENABLED`: the one switch in front of every social write.
//!
//! The social slice is the first where this node is the *authority* for what
//! users write (communities, memberships, friendships, blocks, settings,
//! moderation, notification state, badges). Peer sync is not a backup of
//! that state, so README.md ("Acceptance and rollout") requires a restore
//! of this node's database, blobs and credentials to be demonstrated before
//! the switch is turned on in production. Off (the default), a mutating
//! request (`POST`, `PUT`, `PATCH`, `DELETE`) to a social route answers 503
//! with the social error envelope; the reads keep answering, so a node in
//! catch-up serves the explorer's boot reads without accepting anything it
//! could lose. The social RPC socket applies the same switch to its mutating
//! procedures (`rpc::service`, `SocialError::WritesDisabled`).
//!
//! Admission and presence are not user authority: the LiveKit token mints
//! (`/get-scene-adapter`, `/get-server-scene-adapter`), the Pulse room
//! authority (`/internal/pulse/room-authority/v1`) and the media server's
//! webhook stay open, as does `/translate` (a cached read of an external
//! engine) and the two operator routes that change no durable state
//! (`/admin/social/disconnect` kicks a socket, `/admin/social/force-presence`
//! broadcasts a presence frame), so an operator can disconnect an abusive
//! socket on a read-only node; `/admin/social/reset-settings` is a durable
//! write and stays gated. Everything else that is not a `GET`/`HEAD`/`OPTIONS`
//! is gated.

use axum::Router;
use axum::extract::{Request, State};
use axum::http::{Method, StatusCode, header};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};

use crate::comms::writes::Switch;
use crate::shared::writes::{Refusal, refusal};

/// The paths (as the router sees them, after any nest prefix is stripped)
/// whose non-GET methods stay open while writes are disabled.
const OPEN_WHILE_DISABLED: &[&str] = &[
    "/get-scene-adapter",
    "/get-server-scene-adapter",
    "/island-refresh",
    "/livekit-webhook",
    crate::comms::protocol::room_authority::AUTHORITY_PATH,
    "/translate",
    "/admin/social/disconnect",
    "/admin/social/force-presence",
];

pub const DISABLED_MESSAGE: &str = "social writes are disabled on this node (SOCIAL_WRITES_ENABLED=false): a backup restore has to be demonstrated before this node accepts user writes";

/// Whether `method` on `path` is a write this gate refuses while disabled.
pub fn is_gated(method: &Method, path: &str) -> bool {
    if matches!(*method, Method::GET | Method::HEAD | Method::OPTIONS) {
        return false;
    }
    !OPEN_WHILE_DISABLED.contains(&path)
}

/// The 503 the gate answers: the social error envelope with a `Retry-After`
/// so a client does not hammer a node that will not change its mind soon.
pub fn disabled_response() -> Response {
    refused_response(&Refusal::Off)
}

/// The 503 for `refusal`: the switch's own message while it is off, the
/// restore stamp that holds it and its age while it is on.
pub fn refused_response(refusal: &Refusal) -> Response {
    let message = refusal.message("social", DISABLED_MESSAGE);
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

async fn gate(State(enabled): State<bool>, mut req: Request, next: Next) -> Response {
    if is_gated(req.method(), req.uri().path())
        && let Some(refusal) = refusal(enabled)
    {
        return refused_response(&refusal);
    }
    req.extensions_mut().insert(Switch(enabled));
    next.run(req).await
}

/// Wraps a social router in the gate. Applied per router, before nesting,
/// so the path the gate sees is the route-local one.
pub fn guard(router: Router, writes_enabled: bool) -> Router {
    router.layer(middleware::from_fn_with_state(writes_enabled, gate))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::routing::{get, post};
    use tower::ServiceExt;

    fn app(enabled: bool) -> Router {
        let inner = Router::new()
            .route(
                "/v1/communities",
                get(|| async { "list" }).post(|| async { "created" }),
            )
            .route("/get-scene-adapter", post(|| async { "token" }))
            .route("/translate", post(|| async { "hola" }))
            .route("/admin/social/disconnect", post(|| async { "kicked" }))
            .route("/admin/social/reset-settings", post(|| async { "reset" }));
        Router::new()
            .merge(guard(inner.clone(), enabled))
            .nest("/social-api", guard(inner, enabled))
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
    fn gated_is_every_mutating_method_but_the_admission_and_translate_paths() {
        assert!(is_gated(&Method::POST, "/v1/communities"));
        assert!(is_gated(&Method::PUT, "/notifications/read"));
        assert!(is_gated(&Method::PATCH, "/v1/communities/x/members/y"));
        assert!(is_gated(&Method::DELETE, "/scene-bans"));
        assert!(!is_gated(&Method::GET, "/v1/communities"));
        assert!(!is_gated(&Method::HEAD, "/status"));
        assert!(!is_gated(&Method::OPTIONS, "/v1/communities"));
        for open in OPEN_WHILE_DISABLED {
            assert!(!is_gated(&Method::POST, open), "{open} must stay open");
        }
        assert!(
            is_gated(&Method::POST, "/get-scene-adapter/x"),
            "only the exact admission paths are open"
        );
        assert!(
            is_gated(&Method::POST, "/admin/social/reset-settings"),
            "a durable operator write stays gated"
        );
        assert!(!is_gated(&Method::POST, "/admin/social/disconnect"));
        assert!(!is_gated(&Method::POST, "/admin/social/force-presence"));
    }

    #[tokio::test]
    async fn disabled_answers_503_envelope_on_writes_and_serves_reads() {
        let app = app(false);
        let (status, body) = call(&app, "POST", "/v1/communities").await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        let v: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(v["ok"], false);
        assert_eq!(v["error"], "Service Unavailable");
        assert!(
            v["message"]
                .as_str()
                .unwrap()
                .contains("SOCIAL_WRITES_ENABLED=false")
        );
        let (status, _) = call(&app, "POST", "/social-api/v1/communities").await;
        assert_eq!(
            status,
            StatusCode::SERVICE_UNAVAILABLE,
            "the nested copy is gated on its route-local path"
        );
        assert_eq!(
            call(&app, "GET", "/v1/communities").await,
            (StatusCode::OK, "list".to_string())
        );
        assert_eq!(
            call(&app, "GET", "/social-api/v1/communities").await,
            (StatusCode::OK, "list".to_string())
        );
        assert_eq!(
            call(&app, "POST", "/get-scene-adapter").await,
            (StatusCode::OK, "token".to_string())
        );
        assert_eq!(
            call(&app, "POST", "/social-api/translate").await,
            (StatusCode::OK, "hola".to_string())
        );
        assert_eq!(
            call(&app, "POST", "/admin/social/disconnect").await,
            (StatusCode::OK, "kicked".to_string())
        );
        let (status, _) = call(&app, "POST", "/admin/social/reset-settings").await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    }

    #[tokio::test]
    async fn enabled_passes_writes_through() {
        let app = app(true);
        assert_eq!(
            call(&app, "POST", "/v1/communities").await,
            (StatusCode::OK, "created".to_string())
        );
        assert_eq!(
            call(&app, "POST", "/social-api/v1/communities").await,
            (StatusCode::OK, "created".to_string())
        );
    }

    /// A refused upload, served as `serve` serves it, is still read to its
    /// end after the 503: behind nginx, a body the upstream closed on
    /// mid-send turned the 503 into a 502.
    #[tokio::test]
    async fn a_refused_write_body_is_drained_after_the_503() {
        use futures::StreamExt;
        use std::sync::Arc;
        use std::sync::atomic::{AtomicUsize, Ordering};
        let read = Arc::new(AtomicUsize::new(0));
        let counted = read.clone();
        let (ended, end) = tokio::sync::oneshot::channel();
        let mut ended = Some(ended);
        let chunks = futures::stream::iter((0..32).map(move |_| {
            counted.fetch_add(64 * 1024, Ordering::SeqCst);
            Ok::<_, std::io::Error>(axum::body::Bytes::from(vec![0u8; 64 * 1024]))
        }))
        .chain(futures::stream::poll_fn(move |_| {
            if let Some(ended) = ended.take() {
                let _ = ended.send(());
            }
            std::task::Poll::Ready(None)
        }));
        let req = Request::builder()
            .method("PUT")
            .uri("/v1/communities")
            .body(Body::from_stream(chunks))
            .unwrap();
        let served = crate::shared::drain::drain_unread(app(false), 11 * 1024 * 1024);
        let res = served.oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::SERVICE_UNAVAILABLE);
        tokio::time::timeout(std::time::Duration::from_secs(60), end)
            .await
            .expect("hang guard")
            .expect("the body is read to its end");
        assert_eq!(read.load(Ordering::SeqCst), 2 * 1024 * 1024);
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
            let (status, _) = call(&app, "POST", "/social-api/v1/communities").await;
            assert_eq!(status, StatusCode::OK, "fresh stamps let the write through");
            lab.advance(26 * HOUR);
            let (status, body) = call(&app, "POST", "/social-api/v1/communities").await;
            assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
            let v: serde_json::Value = serde_json::from_str(&body).unwrap();
            assert_eq!(v["error"], "Service Unavailable");
            assert_eq!(
                v["message"],
                "social writes are held on this node: the last complete backup (backup.json) is 27 h old, past WRITES_BACKUP_MAX_AGE_HOURS=26"
            );
            let (status, _) = call(&app, "GET", "/v1/communities").await;
            assert_eq!(status, StatusCode::OK);
        })
        .await;
    }
}
