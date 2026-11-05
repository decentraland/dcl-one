//! The well-known-components status checks (`createStatusCheckComponent`) the
//! old services answered under their gateway prefix: `/health/live` answers
//! `alive`, `/health/ready` and `/health/startup` the pass document.

use axum::Router;
use axum::http::header::CONTENT_TYPE;
use axum::routing::get;

pub const PASS: &str = r#"{"details":{"httpServer":{"status":"pass"}},"status":"pass"}"#;

/// The three probes; mount it under the old service's prefix.
pub fn status_checks_router() -> Router {
    let pass = || async {
        (
            [(CONTENT_TYPE, "application/health+json; charset=utf-8")],
            PASS,
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
