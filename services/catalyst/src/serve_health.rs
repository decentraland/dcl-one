//! Required checks govern traffic to this replica. Diagnostics report shared
//! dependency state without withdrawing every replica from service.
use std::collections::BTreeMap;
use std::convert::Infallible;
use std::future::{Future, ready};
use std::sync::Arc;
use std::time::Duration;

use axum::http::{StatusCode, header};
use axum::{Json, Router};
use futures::future::{BoxFuture, join_all};
use serde::Serialize;

use super::routes::Routes;
use crate::shared::{cache::TtlMap, health::database_ready, routing::get};
use crate::{config::Config, land_authz, shop};

const CHECK_TIMEOUT: Duration = Duration::from_secs(2);
const SNAPSHOT_TTL: Duration = Duration::from_secs(1);

#[derive(Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
enum Status {
    Pass,
    Fail,
}

#[derive(Clone, Serialize)]
struct CheckResult {
    status: Status,
    required: bool,
}

#[derive(Clone, Serialize)]
struct Report {
    status: Status,
    details: BTreeMap<&'static str, CheckResult>,
}

type Check = Box<dyn Fn() -> BoxFuture<'static, bool> + Send + Sync>;

#[derive(Clone, Copy, PartialEq, Eq)]
enum CheckMode {
    Required,
    Diagnostic,
}

use CheckMode::{Diagnostic, Required};

#[derive(Default)]
struct Checks(BTreeMap<&'static str, (CheckMode, Check)>);

impl Checks {
    fn register<F, Fut>(&mut self, name: &'static str, mode: CheckMode, check: F)
    where
        F: Fn() -> Fut + Send + Sync + 'static,
        Fut: Future<Output = bool> + Send + 'static,
    {
        let check: Check = Box::new(move || Box::pin(check()));
        assert!(
            self.0.insert(name, (mode, check)).is_none(),
            "duplicate readiness check {name}"
        );
    }

    async fn report(&self) -> Report {
        let details: BTreeMap<_, _> =
            join_all(self.0.iter().map(|(&name, (mode, check))| async move {
                let passed = tokio::time::timeout(CHECK_TIMEOUT, check())
                    .await
                    .unwrap_or(false);
                (
                    name,
                    CheckResult {
                        required: *mode == Required,
                        status: if passed { Status::Pass } else { Status::Fail },
                    },
                )
            }))
            .await
            .into_iter()
            .collect();
        let status = if details
            .values()
            .all(|result| !result.required || result.status == Status::Pass)
        {
            Status::Pass
        } else {
            Status::Fail
        };
        Report { status, details }
    }

    fn router(self) -> Router {
        let checks = Arc::new(self);
        // All callers share the same bounded fetch, including during database outages.
        let snapshots = Arc::new(TtlMap::new(SNAPSHOT_TTL));
        Router::new().route(
            "/health",
            get(move || {
                let checks = checks.clone();
                let snapshots = snapshots.clone();
                async move {
                    let Ok(report) = snapshots
                        .get_or_fetch((), || async { Ok::<_, Infallible>(checks.report().await) })
                        .await;
                    let code = match report.status {
                        Status::Pass => StatusCode::OK,
                        Status::Fail => StatusCode::SERVICE_UNAVAILABLE,
                    };
                    (code, [(header::CACHE_CONTROL, "no-store")], Json(report))
                }
            }),
        )
    }
}

fn map_is_configured(land: &land_authz::Settings, market: &shop::market::Settings) -> bool {
    land.network == land_authz::NETWORK_MAINNET
        && land.is_configured()
        && market.market.indexes.nft_enabled
        && (market.data_writes_enabled || market.trades_sync_upstream_url.is_some())
}

pub(super) fn router(config: &Config, routes: &Routes<'_>) -> Router {
    let mut checks = Checks::default();
    let pool = routes.content.pool.clone();
    checks.register("postgres", Diagnostic, move || {
        let pool = pool.clone();
        async move { database_ready(&pool).await }
    });
    let listener = routes.archipelago_state.clone();
    checks.register("server", Required, move || ready(listener.listener_ready()));
    let rpc = routes.social.rpc.clone();
    checks.register("social-rpc", Required, move || {
        ready(rpc.initial_fanout_ready())
    });
    if !config.node.serve_unsynced {
        // Sync state can describe another replica, including during a lease handoff.
        let sync = routes.content.sync.clone();
        checks.register("content", Diagnostic, move || ready(sync.is_ready()));
    }
    if let Some(about_comms) = routes.content.about_comms().cloned() {
        checks.register("comms", Diagnostic, move || {
            ready(about_comms.probe().healthy)
        });
    }
    if map_is_configured(&routes.land.settings, &routes.market.settings) {
        let api = routes.explorer_api.clone();
        checks.register("map", Required, move || ready(api.map.is_ready()));
    }
    checks.router()
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::{Body, to_bytes},
        http::Request,
    };
    use serde_json::{Value, json};
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use tower::ServiceExt;

    #[test]
    fn node_readiness_requires_a_map_only_with_all_its_sources() {
        let mut land =
            land_authz::Settings::for_network("MAINNET", Some("https://rpc.example".into()))
                .unwrap();
        let mut market = shop::market::Settings::for_tests();
        land.index_enabled = true;
        market.market.indexes.nft_enabled = true;
        market.data_writes_enabled = false;
        market.trades_sync_upstream_url = None;
        assert!(!map_is_configured(&land, &market));
        market.data_writes_enabled = true;
        assert!(map_is_configured(&land, &market));
        market.data_writes_enabled = false;
        market.trades_sync_upstream_url = Some("https://market.example".into());
        assert!(map_is_configured(&land, &market));
        land.network = land_authz::NETWORK_SEPOLIA.into();
        assert!(!map_is_configured(&land, &market));
    }

    async fn probe(app: &Router) -> (StatusCode, Value) {
        let response = app
            .clone()
            .oneshot(Request::get("/health").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
        let status = response.status();
        let body = to_bytes(response.into_body(), 4096).await.unwrap();
        (status, serde_json::from_slice(&body).unwrap())
    }

    #[tokio::test(start_paused = true)]
    async fn node_readiness_aggregates_checks_and_recovers_after_a_shared_snapshot() {
        let calls = Arc::new(AtomicUsize::new(0));
        let available = Arc::new(AtomicBool::new(false));
        let mut checks = Checks::default();
        checks.register("server", Required, || ready(true));
        checks.register("reader", Required, {
            let calls = calls.clone();
            let available = available.clone();
            move || {
                calls.fetch_add(1, Ordering::SeqCst);
                let ready = available.load(Ordering::SeqCst);
                async move {
                    tokio::task::yield_now().await;
                    ready
                }
            }
        });
        let app = checks.router();
        let (first, second) = tokio::join!(probe(&app), probe(&app));
        assert_eq!(first, second);
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        assert_eq!(
            first,
            (
                StatusCode::SERVICE_UNAVAILABLE,
                json!({
                    "status": "fail",
                    "details": {
                        "reader": {"required": true, "status": "fail"},
                        "server": {"required": true, "status": "pass"}
                    }
                })
            )
        );
        available.store(true, Ordering::SeqCst);
        tokio::time::advance(SNAPSHOT_TTL).await;
        assert_eq!(
            probe(&app).await,
            (
                StatusCode::OK,
                json!({
                    "status": "pass",
                    "details": {
                        "reader": {"required": true, "status": "pass"},
                        "server": {"required": true, "status": "pass"}
                    }
                })
            )
        );
    }

    #[tokio::test(start_paused = true)]
    async fn node_readiness_stays_ready_when_diagnostics_time_out_together() {
        let mut checks = Checks::default();
        checks.register("first", Diagnostic, std::future::pending);
        checks.register("second", Diagnostic, std::future::pending);
        checks.register("server", Required, || ready(true));
        let started = tokio::time::Instant::now();
        let (status, body) = probe(&checks.router()).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["status"], "pass");
        assert_eq!(
            body["details"],
            json!({
                "first": {"required": false, "status": "fail"},
                "second": {"required": false, "status": "fail"},
                "server": {"required": true, "status": "pass"}
            })
        );
        assert_eq!(started.elapsed(), CHECK_TIMEOUT);
    }
}
