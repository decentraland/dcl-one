//! Authenticated `/ws` adapter beside Gatekeeper in the primary `serve` process.
//! Socket admission, assignment delivery, moderation room bookkeeping and bounded
//! ownership-controlled positions stay local. Every decoded authenticated position
//! and disconnect separately enters the statistics observation queue. Pulse alone
//! owns statistics; read-only HTTP views use its complete snapshots on the same bus.
//! Social retains the independent topology subscription. Gatekeeper alone holds
//! LiveKit credentials and validates room authority at every credential boundary.

pub mod auth;
pub mod ban;
pub mod config;
pub mod control_v4;
pub mod enforcement;
pub mod feed;
pub mod handlers;
pub mod observations;
mod reconciliation;
pub mod registry;
pub mod session;
pub mod state;
pub mod ws;

pub use config::Settings;
pub use state::{AppState, AppStateInner};

use std::sync::Arc;

use anyhow::Result;
use axum::Router;

use crate::jobs::Jobs;

use crate::deny_list::DenyList;
use auth::ChallengeStore;
use ban::BanChecker;
use control_v4::AssignmentAuthority;
use enforcement::Enforcement;
use feed::{FeedCache, Outbox};
use registry::PeersRegistry;

/// keccak256, the EIP-712 dynamic-value hash the v4 socket fingerprints its transcripts with.
pub fn hash_dynamic(bytes: &[u8]) -> [u8; 32] {
    alloy_primitives::keccak256(bytes).0
}

/// Wires the connector over the gatekeeper's NATS link: registers the feed's subscriptions on
/// it, dials it and spawns the feed job (`archipelago-feed`) on `jobs`, so shutdown stops it
/// with the rest. A test hands in a gatekeeper over an in-process bus and drives the feed
/// through it. The gatekeeper's island subscriber starts afterwards, with the returned state
/// as its sockets (`CommsState::start_island_feed`).
pub async fn build_state(
    settings: Settings,
    gatekeeper: crate::gatekeeper::SharedState,
    jobs: &Jobs,
) -> Result<AppState> {
    let pool = gatekeeper.pool.clone();
    let http = gatekeeper.http.clone();
    let ban_checker = BanChecker::new(gatekeeper.policy.clone(), gatekeeper.bans.clone());
    let deny_list = DenyList::new(settings.auth.deny_list_url.clone(), http.clone());
    if !deny_list.is_armed() {
        tracing::warn!(
            "DENY_LIST_URL is unset \u{2014} the wallet denylist is DISARMED and no address \
             will be blocked. This used to fall back to Decentraland's production \
             denylist; set DENY_LIST_URL to a list this deployment controls to re-arm it."
        );
    }
    let registry = PeersRegistry::new();
    let enforcement = Enforcement::new(
        settings.cluster.clone(),
        Arc::clone(&gatekeeper),
        Arc::clone(&ban_checker),
        Arc::clone(&registry),
    );
    let challenges = ChallengeStore::new(settings.auth.clone());
    let replica_id = uuid::Uuid::new_v4().to_string();
    let control_v4 = if let Some(audience) = &gatekeeper.cluster.control_v4_audience {
        match AssignmentAuthority::pg_with_namespace(pool, audience.clone()).await {
            Ok(authority) => authority,
            Err(error) => {
                tracing::error!(%error, "assignment authority fence table (social.archipelago_v4_*) is unavailable; configured legacy admission will fail closed");
                AssignmentAuthority::unavailable(&replica_id)
            }
        }
    } else {
        tracing::warn!(
            "ARCHIPELAGO_CONTROL_V4_AUDIENCE is unset \u{2014} legacy sockets use the local session registry"
        );
        AssignmentAuthority::unavailable(&replica_id)
    };
    tracing::info!(
        livekit_armed = gatekeeper.connector_livekit_armed(),
        deny_list_armed = deny_list.is_armed(),
        feed_armed = gatekeeper.bus.is_enabled(),
        control_v4_armed = control_v4.is_available(),
        "archipelago wired"
    );

    let statistics = Arc::new(crate::stats::Reader::default());
    let statistics_subscription = statistics.subscribe(gatekeeper.bus.as_ref());
    let observations = observations::Observations::new(gatekeeper.bus.clone(), jobs);
    let state = Arc::new(AppStateInner {
        statistics,
        _statistics_subscription: statistics_subscription,
        observations,
        connector_metrics: crate::stats::runtime::Metrics::from_env(),
        started_at: std::time::Instant::now(),
        started: std::sync::atomic::AtomicBool::new(false),
        sockets: Arc::new(tokio::sync::Semaphore::new(
            settings.server.max_sockets as usize,
        )),
        handlers: Arc::new(tokio::sync::Semaphore::new(
            settings.server.max_sockets as usize,
        )),
        restart: tokio_util::sync::CancellationToken::new(),
        settings,
        enforcement,
        registry,
        feed: Arc::new(FeedCache::default()),
        outbox: Outbox::new(Arc::clone(&gatekeeper.bus)),
        gatekeeper,
        challenges,
        control_v4,
        replica_id,
        ban_checker,
        deny_list,
    });
    feed::start(&state, jobs);
    Ok(state)
}

/// The connected-wallet moderation sweep.
pub fn spawn_jobs(state: &AppState, jobs: &Jobs) {
    state.enforcement.spawn_jobs(jobs);
}

/// Routes for the shared root, complete Stats host, and distinct connector host.
pub struct Mounts {
    pub root: Router,
    pub stats: Router,
    pub connector: Router,
    pub statistics: crate::stats::http::State,
}
pub fn mounts(
    state: AppState,
    primary: bool,
    settings: crate::stats::http::Settings,
    matcher: Arc<dyn crate::stats::http::MetricsMatcher>,
    denylist: Arc<crate::content::Denylist>,
) -> Mounts {
    let statistics = crate::stats::http::State::new(
        state.statistics.clone(),
        Arc::new(crate::stats::content::Content {
            pool: state.gatekeeper.pool.clone(),
            denylist,
        }),
        crate::stats::runtime::Runtime::new(state.clone()),
        settings,
        matcher,
    );
    let sockets = if primary {
        socket_router(state.clone())
    } else {
        Router::new()
    };
    let root = crate::stats::http::merge_root(sockets.clone(), statistics.clone());
    let stats = crate::stats::http::socket_alias(
        crate::stats::http::standalone(statistics.clone()).merge(sockets),
        statistics.clone(),
    );
    let connector = handlers::connector(state, primary);
    Mounts {
        root,
        stats,
        connector,
        statistics,
    }
}
/// Existing `/ws` aliases keep the same authenticated socket adapter.
pub fn socket_router(state: AppState) -> Router {
    ws::routes().with_state(state)
}

/// The connector over a test gatekeeper whose NATS link is a given bus, for the unit tests.
#[cfg(test)]
pub(crate) mod test_support {
    use std::sync::Arc;
    use std::time::Duration;

    use sqlx::PgPool;
    use sqlx::postgres::PgPoolOptions;

    use super::{AppState, Settings, build_state};
    use crate::gatekeeper::admission::cluster::subscriber::{ClusterGateway, GatewayError};
    use crate::gatekeeper::bus::NatsBus;
    use crate::gatekeeper::livekit::Removal;
    use crate::gatekeeper::{CommsState, SharedState};
    use crate::http::Outbound;
    use crate::interfaces::test_support::{StubPolicy, StubScenes};
    use crate::jobs::Jobs;

    /// `settings` over `bus`, with the feed job running on a registry nobody shuts down.
    pub(crate) async fn state_over(settings: Settings, bus: Arc<dyn NatsBus>) -> AppState {
        state_on(settings, bus, &Jobs::new()).await
    }

    /// `settings` over `bus`, with the feed job on `jobs`. The pool never connects: an
    /// audience leaves the v4 control authority unavailable within half a second.
    pub(crate) async fn state_on(
        settings: Settings,
        bus: Arc<dyn NatsBus>,
        jobs: &Jobs,
    ) -> AppState {
        let pool = lazy_pool();
        let gatekeeper = Arc::new(comms_state(pool).with_bus(bus).with_ban_status(Arc::new(
            crate::gatekeeper::moderation::test_support::StubBanStatus::default(),
        )));
        build_state(settings, gatekeeper, jobs)
            .await
            .expect("state")
    }

    /// `settings` over `bus` with the gatekeeper's island subscriber running on `jobs` and
    /// attached to the connector's sockets, as `serve` wires them: a session the connector
    /// announces makes the subscriber ask Pulse (`peer.{wallet}.cluster_lookup` on `bus`), and
    /// an answer that names the session's cluster becomes an island in that socket's mailbox.
    /// The subscriber mints through [`OpenGateway`], so no database or LiveKit is reached.
    pub(crate) async fn state_with_island_feed(
        settings: Settings,
        bus: Arc<dyn NatsBus>,
        jobs: &Jobs,
    ) -> AppState {
        let state = state_on(settings, bus, jobs).await;
        state
            .gatekeeper
            .start_island_feed_through(jobs, state.clone(), Arc::new(OpenGateway));
        state
    }

    /// The ban store and LiveKit as a test sees them: no wallet is banned, a token is minted
    /// offline, and no island room holds anyone yet.
    pub(crate) struct OpenGateway;

    #[async_trait::async_trait]
    impl ClusterGateway for OpenGateway {
        async fn is_denied(&self, _wallet: &str) -> Result<bool, GatewayError> {
            Ok(false)
        }

        async fn mint_connection_string(
            &self,
            _admission: &crate::gatekeeper::admission::cluster::fence::PulseAdmissionStamp,
            _wallet: &str,
            _session: Option<&str>,
            room: &str,
            _ttl_seconds: u64,
            _not_before_unix: Option<u64>,
        ) -> Result<String, GatewayError> {
            Ok(format!(
                "livekit:wss://sfu.invalid?access_token=token-for-{room}"
            ))
        }

        async fn remove_participant(
            &self,
            _room: &str,
            _identity: &str,
            _revoke_tokens_minted_before: Option<i64>,
        ) -> Result<Removal, GatewayError> {
            Ok(Removal::Absent)
        }

        async fn holds_participant(
            &self,
            _room: &str,
            _identity: &str,
        ) -> Result<bool, GatewayError> {
            Ok(false)
        }
    }

    /// The gatekeeper alone, with the disabled NATS link and comms' test LiveKit, whose
    /// `devkey`/`devsecret` placeholders evict nobody. A test without a runtime can hold it: its
    /// pool starts no maintenance task.
    pub(crate) fn gatekeeper() -> SharedState {
        Arc::new(comms_state(lazy_pool()))
    }

    fn lazy_pool() -> PgPool {
        PgPoolOptions::new()
            .max_connections(1)
            .acquire_timeout(Duration::from_millis(500))
            .idle_timeout(None)
            .max_lifetime(None)
            .connect_lazy("postgres://lazy@127.0.0.1:1/catalyst")
            .expect("lazy pool")
    }

    pub(super) fn comms_state(pool: PgPool) -> CommsState {
        CommsState::new(
            pool,
            Outbound::for_tests(),
            crate::gatekeeper::Settings::for_tests(),
            None,
            None,
            Arc::new(StubPolicy::default()),
            Arc::new(StubScenes::default()),
            Arc::new(crate::presence::Presence::new()),
            Arc::new(crate::interfaces::test_support::StubDirectory::default()),
        )
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use axum::Router;
    use axum::body::Body;
    use axum::http::{Method, Request, StatusCode};
    use tower::ServiceExt;

    use super::{AppState, Settings, mounts};
    use crate::gatekeeper::bus::InProcessBus;
    #[tokio::test]
    async fn mounted_hot_scenes_share_local_content_policy_for_get_and_head() {
        use crate::protocol::bus::statistics::{Peer, SNAPSHOTS_SUBJECT, Snapshot, frames};
        let pool = crate::db::scratch::pool(2)
            .await
            .expect("test database required");
        let tag = uuid::Uuid::new_v4();
        let coordinate = 1_800_000 + (tag.as_u128() % 100_000) as i32;
        let ids = [format!("allowed-{tag}"), format!("denied-{tag}")];
        // Synthetic binder rows isolate selected-source policy and metadata rendering.
        for (i, id) in ids.iter().enumerate() {
            let pointer = format!("{},0", coordinate + i as i32);
            let metadata = format!(
                r#"{{"v":{{"scene":{{"base":"{pointer}","parcels":["{pointer}"]}},"display":{{"title":{{"z":1,"a":2}}}}}}}}"#
            );
            sqlx::query("INSERT INTO content.deployments (deployer_address,version,entity_type,entity_id,entity_metadata,entity_timestamp,entity_pointers,local_timestamp,auth_chain) VALUES ('0x0000000000000000000000000000000000000001','v3','scene',$1,$2::json,to_timestamp(1),$3,to_timestamp(1),'[]'::json)").bind(id).bind(metadata).bind(vec![pointer.clone()]).execute(&pool).await.unwrap();
            sqlx::query("INSERT INTO content.active_pointers (pointer,entity_id,entity_type) VALUES ($1,$2,'scene')").bind(pointer).bind(id).execute(&pool).await.unwrap();
        }
        let bus = Arc::new(InProcessBus::new());
        let jobs = crate::jobs::Jobs::new();
        let gatekeeper =
            Arc::new(super::test_support::comms_state(pool.clone()).with_bus(bus.clone()));
        let state = super::build_state(Settings::for_tests(), gatekeeper, &jobs)
            .await
            .unwrap();
        let snapshot = Snapshot {
            peers: (0..2)
                .map(|i| Peer {
                    address: format!("peer-{i}"),
                    position: [f64::from(coordinate + i) * 16., 0., 0.],
                    time: 1,
                })
                .collect(),
            ..Default::default()
        };
        for frame in frames(&snapshot, uuid::Uuid::nil(), 1, 65536).unwrap() {
            bus.inject(SNAPSHOTS_SUBJECT, &frame);
        }
        let denylist = Arc::new(crate::content::Denylist::default());
        denylist.set_operator([ids[1].clone()]);
        let mounted = mounts(
            state,
            false,
            Default::default(),
            crate::stats::http::test_support::matcher(),
            denylist.clone(),
        );
        async fn request(app: Router, method: &str) -> (StatusCode, axum::http::HeaderMap, String) {
            let response = app
                .oneshot(
                    Request::builder()
                        .method(method)
                        .uri("/hot-scenes")
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            let status = response.status();
            let headers = response.headers().clone();
            let body = axum::body::to_bytes(response.into_body(), usize::MAX)
                .await
                .unwrap();
            (status, headers, String::from_utf8(body.to_vec()).unwrap())
        }
        let app = mounted.stats.clone();
        for app in [mounted.root, mounted.stats] {
            let (status, headers, body) = request(app.clone(), "GET").await;
            assert_eq!(status, StatusCode::OK);
            assert_eq!(
                body,
                format!(
                    r#"[{{"id":"{}","name":{{"z":1,"a":2}},"baseCoords":[{coordinate},0],"usersTotalCount":1,"parcels":[[{coordinate},0]]}}]"#,
                    ids[0]
                )
            );
            assert_eq!(headers["content-length"], body.len().to_string());
            let (status, _, body) = request(app.clone(), "HEAD").await;
            assert_eq!(status, StatusCode::OK);
            assert!(body.is_empty());
            denylist.set_operator(ids.clone());
            let (status, _, body) = request(app.clone(), "GET").await;
            assert_eq!(status, StatusCode::OK);
            assert_eq!(body, "[]");
            assert_eq!(request(app, "HEAD").await.0, StatusCode::OK);
            denylist.set_operator([ids[1].clone()]);
        }
        pool.close().await;
        for method in ["GET", "HEAD"] {
            assert_eq!(
                request(app.clone(), method).await.0,
                StatusCode::INTERNAL_SERVER_ERROR
            );
        }
        for frame in frames(&Snapshot::default(), uuid::Uuid::nil(), 2, 65536).unwrap() {
            bus.inject(SNAPSHOTS_SUBJECT, &frame);
        }
        let (status, _, body) = request(app.clone(), "GET").await;
        assert_eq!(
            status,
            StatusCode::OK,
            "empty population skips the closed SQL source"
        );
        assert_eq!(body, "[]");
        assert_eq!(request(app, "HEAD").await.0, StatusCode::OK);
        assert!(
            jobs.shutdown(std::time::Duration::from_secs(1))
                .await
                .is_empty()
        );
    }

    #[tokio::test]
    async fn shutdown_waits_for_handler_observations_after_writers_are_gone() {
        use crate::protocol::bus::statistics::{Event, OBSERVATIONS_SUBJECT, Observation};
        let bus = Arc::new(InProcessBus::new());
        let jobs = crate::jobs::Jobs::new();
        let state = super::test_support::state_on(Settings::for_tests(), bus.clone(), &jobs).await;
        let handler = state.handlers.clone().acquire_owned().await.unwrap();
        let shutting_down = state.clone();
        let done = tokio::spawn(async move {
            super::shutdown(&shutting_down).await;
        });
        state.restart.cancelled().await;
        assert!(!done.is_finished());
        let beat = Event::Heartbeat {
            address: "wallet".into(),
            position: [0.; 3],
        };
        let remove = Event::Disconnect {
            address: "wallet".into(),
        };
        assert!(state.observations.publish(beat.clone()));
        assert!(state.observations.publish(remove.clone()));
        drop(handler);
        done.await.unwrap();
        let events: Vec<_> = bus
            .published()
            .into_iter()
            .filter(|(subject, _)| subject == OBSERVATIONS_SUBJECT)
            .map(|(_, bytes)| Observation::decode(&bytes).unwrap().event)
            .collect();
        assert_eq!(events, [beat, remove]);
        assert_eq!(state.observations.dropped(), 0);
        assert!(
            jobs.shutdown(std::time::Duration::from_secs(1))
                .await
                .is_empty()
        );
    }

    #[tokio::test]
    async fn every_replica_serves_remote_pulse_statistics_without_local_sockets() {
        use crate::protocol::bus::statistics::{Peer, SNAPSHOTS_SUBJECT, Snapshot, frames};
        let bus = Arc::new(InProcessBus::new());
        let state = super::test_support::state_over(Settings::for_tests(), bus.clone()).await;
        let snapshot = Snapshot {
            peers: vec![Peer {
                address: "RemoteCase".into(),
                position: [17., 0., -1.],
                time: 123,
            }],
            ..Default::default()
        };
        for frame in frames(&snapshot, uuid::Uuid::new_v4(), 1, 65536).unwrap() {
            bus.inject(SNAPSHOTS_SUBJECT, &frame);
        }
        let mounts = mounts(
            state,
            false,
            Default::default(),
            crate::stats::http::test_support::matcher(),
            Arc::new(crate::content::Denylist::default()),
        );
        let response = mounts
            .root
            .oneshot(
                Request::builder()
                    .uri("/peers")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(
            response.status(),
            StatusCode::OK,
            "statistics must be available on replicas"
        );
        let body = axum::body::to_bytes(response.into_body(), 4096)
            .await
            .unwrap();
        let value: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(value["peers"][0]["address"], "RemoteCase");
        assert_eq!(value["peers"][0]["lastPing"], 123);
    }

    #[tokio::test]
    async fn connector_status_counts_authenticated_registrations_and_keeps_its_own_fallback() {
        let state =
            super::test_support::state_over(Settings::for_tests(), Arc::new(InProcessBus::new()))
                .await;
        let (old, _events, _) = state.registry.on_peer_connected("wallet", "session");
        let (_second, _events, _) = state.registry.on_peer_connected("wallet", "other");
        let (_replacement, _events, _) = state.registry.on_peer_connected("wallet", "session");
        state
            .registry
            .on_peer_disconnected("wallet", "session", old.id);
        let mounts = mounts(
            state,
            true,
            Default::default(),
            crate::stats::http::test_support::matcher(),
            Arc::new(crate::content::Denylist::default()),
        );
        let response = mounts
            .connector
            .clone()
            .oneshot(Request::get("/status").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(response.headers()["access-control-allow-origin"], "*");
        let body = axum::body::to_bytes(response.into_body(), 4096)
            .await
            .unwrap();
        let value: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(value["userCount"], 2);
        for path in ["/peers", "/health/ready", "/health/startup"] {
            let response = mounts
                .connector
                .clone()
                .oneshot(Request::get(path).body(Body::empty()).unwrap())
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::NOT_FOUND);
            assert_eq!(
                &axum::body::to_bytes(response.into_body(), 4096)
                    .await
                    .unwrap()[..],
                br#"{"error":"Not Found"}"#
            );
        }
        assert_eq!(
            status(&mounts.connector, Method::OPTIONS, "/health/live").await,
            StatusCode::OK
        );
        assert_eq!(
            status(&mounts.connector, Method::HEAD, "/status").await,
            StatusCode::NOT_FOUND
        );
        let response = mounts
            .stats
            .oneshot(Request::get("/status").body(Body::empty()).unwrap())
            .await
            .unwrap();
        let value: serde_json::Value = serde_json::from_slice(
            &axum::body::to_bytes(response.into_body(), 4096)
                .await
                .unwrap(),
        )
        .unwrap();
        assert!(value.get("userCount").is_none());
    }

    #[tokio::test]
    async fn running_outer_router_isolates_stats_and_connector_from_shared_normalization() {
        let state =
            super::test_support::state_over(Settings::for_tests(), Arc::new(InProcessBus::new()))
                .await;
        let mounts = mounts(
            state.clone(),
            true,
            Default::default(),
            crate::stats::http::test_support::matcher(),
            Arc::new(crate::content::Denylist::default()),
        );
        let shared = Router::new()
            .route("/status", axum::routing::get(|| async { "shared-status" }))
            .route("/other", axum::routing::get(|| async { "shared-other" }));
        let app = crate::stats::http::mount_services(
            shared,
            mounts.statistics,
            mounts.stats,
            mounts.connector,
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        state
            .started
            .store(true, std::sync::atomic::Ordering::Release);
        let task = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        let client = reqwest::Client::new();
        assert_eq!(
            client
                .get(format!("{base}/status"))
                .send()
                .await
                .unwrap()
                .text()
                .await
                .unwrap(),
            "shared-status"
        );
        assert_eq!(
            client
                .get(format!("{base}/other///"))
                .send()
                .await
                .unwrap()
                .text()
                .await
                .unwrap(),
            "shared-other"
        );
        let response = client
            .get(format!("{base}/archipelago-ea-stats/peers//"))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), reqwest::StatusCode::NOT_FOUND);
        assert_eq!(response.text().await.unwrap(), "Not found");
        let response = client
            .get(format!("{base}/archipelago-ea-ws-connector/status/"))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), reqwest::StatusCode::NOT_FOUND);
        assert_eq!(response.text().await.unwrap(), r#"{"error":"Not Found"}"#);
        let response = client
            .request(
                reqwest::Method::OPTIONS,
                format!("{base}/archipelago-ea-stats/peers"),
            )
            .header("Origin", "https://example.invalid")
            .send()
            .await
            .unwrap();
        assert_eq!(
            response.headers()["access-control-allow-origin"],
            "https://example.invalid"
        );
        let response = client
            .get(format!("{base}/archipelago-ea-stats/health/ready"))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), reqwest::StatusCode::OK);
        assert_eq!(
            response.text().await.unwrap(),
            r#"{"details":{"server":{"status":"pass"}},"status":"pass"}"#
        );
        task.abort();
    }

    const PEER: &str = "0x000000000000000000000000000000000000000a";
    const ISLAND: &str = "I1";
    /// The island-token routes and the heartbeat write the connector used to serve over HTTP.
    const RETIRED_POSTS: [&str; 3] = ["/auth/challenge", "/auth/livekit-token", "/heartbeat"];

    /// The connector over an in-process link that knows one peer and one island, so a route
    /// that looks either up answers it instead of its own 404.
    async fn known_peer_and_island() -> AppState {
        use crate::protocol::bus::statistics::{Peer, SNAPSHOTS_SUBJECT, Snapshot, frames};
        use prost::Message;
        let bus = Arc::new(InProcessBus::new());
        let state = super::test_support::state_over(Settings::for_tests(), bus.clone()).await;
        let snapshot = Snapshot {
            peers: vec![Peer {
                address: PEER.into(),
                position: [0.; 3],
                time: 123,
            }],
            islands: crate::protocol::archipelago::archipelago::IslandStatusMessage {
                data: vec![crate::protocol::archipelago::archipelago::IslandData {
                    id: ISLAND.into(),
                    peers: vec![PEER.into()],
                    max_peers: 0,
                    center: Some(crate::protocol::archipelago::Position {
                        x: 0.,
                        y: 0.,
                        z: 0.,
                    }),
                    radius: 1.,
                }],
            }
            .encode_to_vec(),
            ..Default::default()
        };
        for frame in frames(&snapshot, uuid::Uuid::new_v4(), 1, 65536).unwrap() {
            bus.inject(SNAPSHOTS_SUBJECT, &frame);
        }
        state
    }

    /// Every GET route that reads this process's sockets, at each of its prefixes.
    fn socket_gets() -> Vec<String> {
        let mut paths = vec!["/ws".to_owned()];
        for prefix in ["", "/comms", "/stats"] {
            for route in [
                "/parcels".to_owned(),
                "/peers".to_owned(),
                format!("/peers/{PEER}"),
                "/islands".to_owned(),
                format!("/islands/{ISLAND}"),
            ] {
                paths.push(format!("{prefix}{route}"));
            }
        }
        paths.push("/hot-scenes".to_owned());
        paths.push("/stats/health".to_owned());
        paths
    }

    async fn status(router: &Router, method: Method, path: &str) -> StatusCode {
        router
            .clone()
            .oneshot(
                Request::builder()
                    .method(method)
                    .uri(path)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap()
            .status()
    }

    /// The primary serves the connector's routes at both mounts: every route at the stats mount,
    /// and all but `/status` and `/ping` (the content server's at the root) at the root. `/ws`
    /// without upgrade headers uses the standalone Stats fallback at that mount.
    #[tokio::test]
    async fn the_primary_serves_every_connector_route() {
        let mounts = mounts(
            known_peer_and_island().await,
            true,
            Default::default(),
            crate::stats::http::test_support::matcher(),
            Arc::new(crate::content::Denylist::default()),
        );
        for (mount, router) in [("root", &mounts.root), ("stats", &mounts.stats)] {
            assert_eq!(
                status(router, Method::GET, "/ws/v4").await,
                StatusCode::NOT_FOUND
            );
            let mut gets = socket_gets();
            gets.push("/core-status".to_owned());
            for path in &gets {
                let answer = status(router, Method::GET, path).await;
                if mount == "stats" && (path.starts_with("/stats/") || path == "/ws") {
                    assert_eq!(answer, StatusCode::NOT_FOUND, "{mount}: GET {path}");
                } else {
                    assert_ne!(answer, StatusCode::NOT_FOUND, "{mount}: GET {path}");
                }
            }
        }
        for path in ["/status", "/ping"] {
            assert_eq!(
                status(&mounts.stats, Method::GET, path).await,
                StatusCode::OK,
                "stats: GET {path}"
            );
            assert_eq!(
                status(&mounts.root, Method::GET, path).await,
                StatusCode::NOT_FOUND,
                "root: GET {path}"
            );
        }
    }

    /// The primary answers none of the retired POST routes, at either mount: no route matches,
    /// so the method does not matter and a GET is not found either.
    #[tokio::test]
    async fn the_retired_routes_are_gone() {
        let mounts = mounts(
            known_peer_and_island().await,
            true,
            Default::default(),
            crate::stats::http::test_support::matcher(),
            Arc::new(crate::content::Denylist::default()),
        );
        for (mount, router) in [("root", &mounts.root), ("stats", &mounts.stats)] {
            for path in RETIRED_POSTS {
                for method in [Method::POST, Method::GET] {
                    assert_eq!(
                        status(router, method.clone(), path).await,
                        StatusCode::NOT_FOUND,
                        "{mount}: {method} {path}"
                    );
                }
            }
        }
    }

    #[tokio::test]
    async fn replicas_serve_statistics_but_never_sockets() {
        let mounts = mounts(
            known_peer_and_island().await,
            false,
            Default::default(),
            crate::stats::http::test_support::matcher(),
            Arc::new(crate::content::Denylist::default()),
        );
        assert_eq!(
            status(&mounts.root, Method::GET, "/stats/health").await,
            StatusCode::OK
        );
        assert_eq!(
            status(&mounts.stats, Method::GET, "/stats/health").await,
            StatusCode::NOT_FOUND
        );
        assert_eq!(
            status(&mounts.stats, Method::GET, "/ws").await,
            StatusCode::NOT_FOUND
        );
        for path in ["/status", "/ping", "/core-status", "/peers", "/islands"] {
            assert_eq!(
                status(&mounts.stats, Method::GET, path).await,
                StatusCode::OK,
                "{path}"
            );
        }
    }
}

/// Stop admission and drain handlers, writers and observations concurrently.
/// Jobs and the shared broker must stay alive until this completes.
pub async fn shutdown(state: &AppState) {
    state.restart.cancel();
    let cap = state.settings.server.max_sockets;
    let handlers_done = tokio_util::sync::CancellationToken::new();
    let drain = tokio::time::timeout(std::time::Duration::from_secs(18), async {
        let _handlers = state.handlers.clone().acquire_many_owned(cap).await;
        handlers_done.cancel();
        let _writers = state.sockets.clone().acquire_many_owned(cap).await;
    });
    let (drained, _) = tokio::join!(
        drain,
        state.observations.shutdown_after(handlers_done.cancelled())
    );
    let drained = drained.is_ok();
    if !drained {
        let remaining = cap as usize - state.handlers.available_permits();
        tracing::warn!(
            remaining,
            "socket teardown exceeded shutdown budget; disconnects may be lost"
        );
    }
}
