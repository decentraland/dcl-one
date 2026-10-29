// from: src/social/writes.rs (the switch's shape)
//! `OPS_WRITES_ENABLED`: the switch in front of every credits write. The
//! ledger, the checkouts, the authorizations and the pack catalogue are
//! local authority peer sync does not back up, so README.md ("Acceptance
//! and rollout") requires a restore of this node's database, blobs and
//! credentials to be demonstrated before the switch is turned on. Off (the
//! default), a mutating request (`POST`, `PUT`, `PATCH`, `DELETE`) to a
//! credits route answers 503 with the credits envelope and `Retry-After`;
//! the reads keep answering (a wallet's balance, the packs, a checkout's
//! status), and so does `POST /prices/quote`, a pure read of the pricing
//! that a client posts because its item list does not fit a query string.
//! The Stripe webhook is gated too: Stripe retries a 503 for days, and a
//! purchase granted on a node that cannot keep it is worse than a late one.

use axum::Router;
use axum::extract::{Request, State};
use axum::http::{Method, StatusCode, header};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};

use crate::shared::writes::{OPS_WRITES_ENABLED, Refusal, WRITES_DISABLED_MESSAGE, ops_refusal};

/// The route-local paths whose non-GET methods stay open while writes are
/// disabled: the price read, at its legacy and its canonical path.
const OPEN_WHILE_DISABLED: &[&str] = &["/prices/quote", "/v1/credit-quotes"];

/// Whether `method` on `path` is a write this gate refuses while disabled.
pub fn is_gated(method: &Method, path: &str) -> bool {
    if matches!(*method, Method::GET | Method::HEAD | Method::OPTIONS) {
        return false;
    }
    !OPEN_WHILE_DISABLED.contains(&path)
}

pub fn disabled_response() -> Response {
    refused_response(&Refusal::Off)
}

/// The 503 for `refusal`: the switch's own message while it is off, the
/// restore stamp that holds it and its age while it is on.
pub fn refused_response(refusal: &Refusal) -> Response {
    let message = refusal.message("operator-plane", WRITES_DISABLED_MESSAGE);
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
        && let Some(refusal) = ops_refusal(enabled)
    {
        return refused_response(&refusal);
    }
    next.run(req).await
}

/// Wraps the credits router in the gate. Applied before nesting, so the path
/// the gate sees is the route-local one.
pub fn guard(router: Router, writes_enabled: bool) -> Router {
    router.layer(middleware::from_fn_with_state(writes_enabled, gate))
}

/// The variable the envelope names, for the module docs and the tests.
pub const SWITCH: &str = OPS_WRITES_ENABLED;

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::routing::{get, post};
    use tower::ServiceExt;

    fn app(enabled: bool) -> Router {
        let inner = Router::new()
            .route("/packs", get(|| async { "packs" }))
            .route("/cart/items", post(|| async { "added" }))
            .route("/prices/quote", post(|| async { "quoted" }))
            .route("/stripe/webhook", post(|| async { "granted" }));
        Router::new().nest("/credits", guard(inner, enabled))
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
    fn gated_is_every_mutating_method_but_the_quote() {
        assert!(is_gated(&Method::POST, "/cart/items"));
        assert!(is_gated(&Method::DELETE, "/cart/items/0xabc/1"));
        assert!(is_gated(&Method::POST, "/stripe/webhook"));
        assert!(is_gated(&Method::PUT, "/admin/packs/x"));
        assert!(!is_gated(&Method::GET, "/packs"));
        assert!(!is_gated(&Method::POST, "/prices/quote"));
        assert!(
            is_gated(&Method::POST, "/prices/quote/x"),
            "only the exact path is open"
        );
    }

    #[tokio::test]
    async fn disabled_answers_503_envelope_on_writes_and_serves_reads() {
        let app = app(false);
        let (status, body) = call(&app, "POST", "/credits/cart/items").await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        let v: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(v["ok"], false);
        assert!(
            v["message"]
                .as_str()
                .unwrap()
                .contains("OPS_WRITES_ENABLED=false")
        );
        assert_eq!(
            call(&app, "GET", "/credits/packs").await,
            (StatusCode::OK, "packs".to_string())
        );
        assert_eq!(
            call(&app, "POST", "/credits/prices/quote").await,
            (StatusCode::OK, "quoted".to_string())
        );
        let (status, _) = call(&app, "POST", "/credits/stripe/webhook").await;
        assert_eq!(
            status,
            StatusCode::SERVICE_UNAVAILABLE,
            "a purchase grant is authoritative state"
        );
        let res = disabled_response();
        assert_eq!(res.headers().get(header::RETRY_AFTER).unwrap(), "3600");
    }

    #[tokio::test]
    async fn enabled_passes_writes_through() {
        let app = app(true);
        assert_eq!(
            call(&app, "POST", "/credits/cart/items").await,
            (StatusCode::OK, "added".to_string())
        );
        assert_eq!(
            call(&app, "POST", "/credits/stripe/webhook").await,
            (StatusCode::OK, "granted".to_string())
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
            let (status, _) = call(&app, "POST", "/credits/cart/items").await;
            assert_eq!(status, StatusCode::OK, "fresh stamps let the write through");
            lab.advance(26 * HOUR);
            let (status, body) = call(&app, "POST", "/credits/cart/items").await;
            assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
            let v: serde_json::Value = serde_json::from_str(&body).unwrap();
            assert_eq!(v["error"], "Service Unavailable");
            assert_eq!(
                v["message"],
                "operator-plane writes are held on this node: the last complete backup (backup.json) is 27 h old, past WRITES_BACKUP_MAX_AGE_HOURS=26"
            );
            let (status, _) = call(&app, "GET", "/credits/packs").await;
            assert_eq!(status, StatusCode::OK);
        })
        .await;
    }
}
