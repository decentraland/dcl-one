use super::AppState;
use serde::Serialize;

pub fn diagnostics(state: &AppState) -> String {
    super::metrics::diagnostics(state)
}

/// The connector host has its own status count, methods, CORS and fallback.
pub fn connector(state: AppState, primary: bool) -> axum::Router {
    connector_configured(
        state,
        primary,
        std::env::var("CURRENT_VERSION").unwrap_or_default(),
        std::env::var("COMMIT_HASH").unwrap_or_default(),
    )
}

fn connector_configured(
    state: AppState,
    primary: bool,
    version: String,
    commit_hash: String,
) -> axum::Router {
    let http = std::sync::Arc::new(ConnectorHttp {
        state: state.clone(),
        version,
        commit_hash,
    });
    let sockets = if primary {
        super::ws::connector_routes(http.clone()).with_state(state)
    } else {
        axum::Router::new()
    };
    sockets.fallback(crate::shared::routing::erase(
        move |method, uri, extensions| {
            let http = http.clone();
            async move { http.respond(method, uri, extensions).await }
        },
    ))
}

pub(super) struct ConnectorHttp {
    state: AppState,
    version: String,
    commit_hash: String,
}
impl ConnectorHttp {
    pub(super) async fn respond(
        &self,
        method: axum::http::Method,
        uri: axum::http::Uri,
        extensions: axum::http::Extensions,
    ) -> axum::response::Response {
        use axum::{body::Body, response::Response};
        let Self {
            state,
            version,
            commit_hash,
        } = self;
        let path = uri.path();
        let method = method.as_str();
        let (code, body, mime, handler) = if path == "/status" && method == "GET" {
            #[derive(Serialize)]
            struct Status {
                version: String,
                #[serde(rename = "currentTime")]
                current_time: i64,
                #[serde(rename = "commitHash")]
                commit_hash: String,
                #[serde(rename = "userCount")]
                user_count: usize,
            }
            (
                200,
                crate::stats::render::json(&Status {
                    version: version.clone(),
                    current_time: chrono::Utc::now().timestamp_millis(),
                    commit_hash: commit_hash.clone(),
                    user_count: state.registry.authenticated_count(),
                }),
                Some("application/json"),
                "/status",
            )
        } else if path == "/health/live" {
            (200, "alive".into(), None, "/health/live")
        } else {
            (
                404,
                r#"{"error":"Not Found"}"#.into(),
                Some("application/json"),
                "unmatched",
            )
        };
        dcl_metrics::http::route(&extensions, "connector", handler);
        let mut response = Response::builder().status(code);
        if let Some(mime) = mime {
            response = response.header("content-type", mime);
        }
        response = response.header("access-control-allow-origin", "*");
        if method == "HEAD" {
            response = response.header("content-length", body.len());
        }
        response
            .body(if method == "HEAD" {
                Body::empty()
            } else {
                Body::from(body)
            })
            .expect("status and headers are valid constants")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{body::Body, http::Request};
    use tower::ServiceExt;

    #[tokio::test]
    async fn diagnostics_record_into_the_process_and_the_connector_scrape_is_gone() {
        let state = super::super::test_support::state_over(
            super::super::Settings::for_tests(),
            std::sync::Arc::new(crate::gatekeeper::bus::DisabledBus),
        )
        .await;
        let app = connector(state.clone(), true);
        let recorder = dcl_metrics::PrometheusBuilder::new().build_recorder();
        let _local = dcl_metrics::set_default_local_recorder(&recorder);
        state.feed.on_deduplicated();
        super::super::metrics::record_diagnostics(&state);
        for (path, expected) in [
            ("/metrics", 404),
            ("/ws", 404),
            ("/status", 200),
            ("/health/live", 200),
        ] {
            let response = app
                .clone()
                .oneshot(Request::builder().uri(path).body(Body::empty()).unwrap())
                .await
                .unwrap();
            assert_eq!(response.status(), expected, "{path}");
        }
        // A key-sized header claims the WebSocket path, then normal upgrade validation runs.
        let bad_upgrade = Request::builder()
            .uri("/ws")
            .header("sec-websocket-key", "dGhlIHNhbXBsZSBub25jZQ==")
            .body(Body::empty())
            .unwrap();
        let response = app.oneshot(bad_upgrade).await.unwrap();
        assert_eq!(response.status(), 400);
        assert!(
            !response
                .headers()
                .contains_key("access-control-allow-origin")
        );
        let body = recorder.handle().render();
        assert!(
            body.contains("dcl_ws_connector_island_changed_deduplicated_total 1\n"),
            "{body}"
        );
    }
}
