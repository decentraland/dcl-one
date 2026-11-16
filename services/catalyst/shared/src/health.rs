//! The well-known-components status checks (`createStatusCheckComponent`) the
//! old services answered under their gateway prefix: `/health/live` answers
//! `alive`, `/health/ready` and `/health/startup` the pass document.
//!
//! `@dcl/http-server`'s `status-checks.js` keys the pass document's `details`
//! by the name each probe-capable component has in the service's component
//! map. The http server is the only such component in the services served
//! here, so the key is the name the service gave its server.

use axum::Router;
use axum::http::header::CONTENT_TYPE;
use axum::routing::get;

/// The pass document of a service whose component map names the server
/// `httpServer` (social-service-ea).
pub const PASS: &str = r#"{"details":{"httpServer":{"status":"pass"}},"status":"pass"}"#;

/// The pass document of a service whose component map names the server
/// `server` (asset-bundle-registry).
pub const SERVER_PASS: &str = r#"{"details":{"server":{"status":"pass"}},"status":"pass"}"#;

/// The three probes answering [`PASS`]; mount it under the old service's prefix.
pub fn status_checks_router() -> Router {
    status_checks_router_with(PASS)
}

/// The three probes answering `pass` ([`PASS`] or [`SERVER_PASS`]).
pub fn status_checks_router_with(pass: &'static str) -> Router {
    let pass = move || async move {
        (
            [(CONTENT_TYPE, "application/health+json; charset=utf-8")],
            pass,
        )
    };
    Router::new()
        .route(
            "/health/live",
            get(|| async { ([(CONTENT_TYPE, "text/plain; charset=utf-8")], "alive") }),
        )
        .route("/health/ready", get(pass))
        .route("/health/startup", get(pass))
}
