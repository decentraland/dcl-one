//! Archipelago's ws-connector and stats (port of decentraland/archipelago-workers):
//! the `/ws` sockets that hand each wallet the island Pulse's
//! cluster feed assigns it, and the `/hot-scenes`, `/peers`, `/islands`,
//! `/parcels` and status surfaces. It computes nothing itself: Pulse
//! (`comms::pulse`, its own process) derives
//! the clusters and publishes them on the NATS broker (`NATS_URL`), and the
//! gatekeeper's island subscriber mints each island. This module is part of
//! the gatekeeper's process: it announces each session to the subscriber and
//! takes the islands it mints for its sockets, both by call (`feed`), and it
//! takes the topology snapshots and publishes control positions over the
//! gatekeeper's link (`gatekeeper::bus`, `serve`'s one NATS client). It lives in
//! `catalyst-comms` and runs inside `catalyst serve` beside the gatekeeper,
//! over the ONE pool (`/hot-scenes` reads
//! `content.active_pointers` + `content.deployments`), the gatekeeper's
//! LiveKit (it removes a kicked wallet's participant,
//! `CommsState::kick_island_participant`; the connector holds no LiveKit
//! credential), the worlds permission core through
//! [`interfaces::AccessPolicy`](crate::interfaces::AccessPolicy) (its
//! `is_banned` replaces the old HTTP hop to the gatekeeper's ban status; worlds'
//! `Policy` implements it) and the guarded client (`DENY_LIST_URL`). Its wire
//! messages are [`protocol::archipelago`](crate::protocol::archipelago).
//!
//! One process holds every socket: only the primary `serve`
//! (`FLEET_JOBS_ENABLED`) mounts `/ws` and the routes that read its
//! sockets ([`socket_router`]) and runs the island subscriber that mints for
//! them. An API replica keeps `/core-status`, `/status` and `/ping`, and the
//! feed job that carries Pulse's beacon to `/core-status` and `/about`
//! ([`mounts`]).
//!
//! The assignment authority (`ARCHIPELAGO_CONTROL_V4_AUDIENCE`) claims legacy sockets'
//! lanes in `social.archipelago_v4_*` (`schemas/40_social.sql`, the comms
//! section), the rows comms' realm-lane fence reads and commits island
//! assignments to; without the audience, or with that table unreadable,
//! configured legacy admission fails closed for ownership.

pub mod auth;
pub mod ban;
pub mod config;
pub mod content;
pub mod control_v4;
pub mod feed;
pub mod handlers;
pub mod peers;
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
use tower_http::cors::{Any, CorsLayer};

use crate::jobs::Jobs;

use crate::deny_list::DenyList;
use auth::ChallengeStore;
use ban::BanChecker;
use content::ContentResolver;
use control_v4::AssignmentAuthority;
use feed::{FeedCache, Outbox};
use peers::PeerDirectory;
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
    let peers = PeerDirectory::new(
        settings.cluster.clone(),
        Arc::clone(&gatekeeper),
        Arc::clone(&ban_checker),
        Arc::clone(&registry),
    );
    let challenges = ChallengeStore::new(settings.auth.clone());
    let content = ContentResolver::new(pool.clone(), settings.content_base_url.clone(), 10);
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

    let state = Arc::new(AppStateInner {
        sockets: Arc::new(tokio::sync::Semaphore::new(
            settings.server.max_sockets as usize,
        )),
        restart: tokio_util::sync::CancellationToken::new(),
        settings,
        peers,
        registry,
        feed: Arc::new(FeedCache::default()),
        outbox: Outbox::new(Arc::clone(&gatekeeper.bus)),
        gatekeeper,
        challenges,
        content,
        control_v4,
        replica_id,
        ban_checker,
        deny_list,
    });
    feed::start(&state, jobs);
    Ok(state)
}

/// The two directory sweeps (`archipelago-peer-expiry`, `archipelago-ban-sweep`).
pub fn spawn_jobs(state: &AppState, jobs: &Jobs) {
    state.peers.spawn_jobs(jobs);
}

/// The connector's routes at their old paths, where the old gateway served them at the root of
/// the `archipelago-*` vhosts. `serve` nests `stats` under `/archipelago-ea-stats` (the per-vhost
/// proxy forwards to it) and merges `root` at the root of the one app for the explorer, whose
/// `/ws` and `/hot-scenes` are root paths.
pub struct Mounts {
    /// The routes without `/status` and `/ping`, which the content server already answers at the
    /// root of the one app.
    pub root: Router,
    pub stats: Router,
}

/// The connector's two mounts. Every process serves `/core-status` at both, and `/status` and
/// `/ping` under `/archipelago-ea-stats`; the `primary` (`FLEET_JOBS_ENABLED`), the one process
/// that holds the sockets, also serves [`socket_router`] at both.
pub fn mounts(state: AppState, primary: bool) -> Mounts {
    let root = with_cors(handlers::core_status_routes()).with_state(state.clone());
    let stats = with_cors(handlers::status_routes().merge(handlers::core_status_routes()))
        .with_state(state.clone());
    if !primary {
        return Mounts { root, stats };
    }
    Mounts {
        root: root.merge(socket_router(state.clone())),
        stats: stats.merge(socket_router(state)),
    }
}

/// The routes only the primary mounts, the same at the root and under `/archipelago-ea-stats`:
/// `/ws` and the routes that read its own sockets' state (`/parcels`, `/peers`,
/// `/islands` and their `/comms` and `/stats` copies, `/hot-scenes`, `/stats/health`).
pub fn socket_router(state: AppState) -> Router {
    with_cors(
        Router::new()
            .merge(handlers::socket_routes())
            .merge(ws::routes()),
    )
    .with_state(state)
}

fn with_cors(router: Router<AppState>) -> Router<AppState> {
    router.layer(
        CorsLayer::new()
            .allow_origin(Any)
            .allow_methods(Any)
            .allow_headers(Any)
            .expose_headers(Any),
    )
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

    fn comms_state(pool: PgPool) -> CommsState {
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

/// `/about`'s comms probe: archipelago's `/core-status` as a module call.
impl crate::content::about_comms::CommsCore for state::AppStateInner {
    fn core_status(&self) -> (bool, u64) {
        (
            self.feed
                .is_core_healthy(chrono::Utc::now().timestamp_millis()),
            u64::from(self.feed.core_user_count()),
        )
    }

    fn parcel_counts(&self) -> Vec<((i32, i32), u32)> {
        self.peers.parcel_counts().into_iter().collect()
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use axum::Router;
    use axum::body::Body;
    use axum::http::{Method, Request, StatusCode};
    use tower::ServiceExt;

    use super::feed::IslandReport;
    use super::{AppState, Settings, mounts};
    use crate::gatekeeper::bus::InProcessBus;

    const PEER: &str = "0x000000000000000000000000000000000000000a";
    const ISLAND: &str = "I1";
    /// The island-token routes and the heartbeat write the connector used to serve over HTTP.
    const RETIRED_POSTS: [&str; 3] = ["/auth/challenge", "/auth/livekit-token", "/heartbeat"];

    /// The connector over an in-process link that knows one peer and one island, so a route
    /// that looks either up answers it instead of its own 404.
    async fn known_peer_and_island() -> AppState {
        let state =
            super::test_support::state_over(Settings::for_tests(), Arc::new(InProcessBus::new()))
                .await;
        state
            .peers
            .upsert_peer(PEER.into(), [0.0; 3], [0, 0], "main".into());
        state.feed.on_islands(vec![IslandReport {
            id: ISLAND.into(),
            peers: vec![PEER.into()],
            max_peers: 0,
            center: [0.0; 3],
            radius: 1.0,
        }]);
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
    /// without upgrade headers answers the upgrade extractor's rejection.
    #[tokio::test]
    async fn the_primary_serves_every_connector_route() {
        let mounts = mounts(known_peer_and_island().await, true);
        for (mount, router) in [("root", &mounts.root), ("stats", &mounts.stats)] {
            assert_eq!(
                status(router, Method::GET, "/ws/v4").await,
                StatusCode::NOT_FOUND
            );
            let mut gets = socket_gets();
            gets.push("/core-status".to_owned());
            for path in &gets {
                let answer = status(router, Method::GET, path).await;
                assert_ne!(answer, StatusCode::NOT_FOUND, "{mount}: GET {path}");
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
        let mounts = mounts(known_peer_and_island().await, true);
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
    async fn a_replica_serves_only_status_ping_and_core_status() {
        let mounts = mounts(known_peer_and_island().await, false);
        for path in ["/status", "/ping", "/core-status"] {
            assert_eq!(
                status(&mounts.stats, Method::GET, path).await,
                StatusCode::OK,
                "stats: GET {path}"
            );
        }
        for path in socket_gets() {
            assert_eq!(
                status(&mounts.stats, Method::GET, &path).await,
                StatusCode::NOT_FOUND,
                "stats: GET {path}"
            );
        }
        for path in RETIRED_POSTS {
            assert_eq!(
                status(&mounts.stats, Method::POST, path).await,
                StatusCode::NOT_FOUND,
                "stats: POST {path}"
            );
        }

        assert_eq!(
            status(&mounts.root, Method::GET, "/core-status").await,
            StatusCode::OK
        );
        for path in ["/ws", "/peers", "/hot-scenes", "/stats/health"] {
            assert_eq!(
                status(&mounts.root, Method::GET, path).await,
                StatusCode::NOT_FOUND,
                "root: GET {path}"
            );
        }
    }
}
