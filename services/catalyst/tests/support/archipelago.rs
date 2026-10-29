//! The archipelago fixtures the integration tests share: settings without a
//! a wallet deny list, the gatekeeper state whose NATS link the connector uses
//! (the disabled link, or a test's bus), whose island subscriber hands the
//! connector its islands by call and whose LiveKit evicts the connector's
//! kicks (comms' placeholders, or configured credentials), the connector's
//! state over a pool that never connects (nothing but `/hot-scenes` and the
//! v4 fence reach the database), and one bound listener serving the
//! primary's stats mount (`comms::archipelago::mounts`).
#![allow(dead_code)]

use std::sync::Arc;

use catalyst::comms::archipelago::config::Settings;
use catalyst::comms::archipelago::{self, AppState};
use catalyst::comms::gatekeeper::bus::NatsBus;
use catalyst::comms::gatekeeper::{self, CommsState, LiveKit, SharedState};
use catalyst::comms::interfaces::AccessPolicy;
use catalyst::comms::interfaces::test_support::{StubPolicy, StubScenes};
use catalyst::http::Outbound;
use sqlx::PgPool;

pub fn settings() -> Settings {
    Settings::for_tests()
}

/// A pool that never connects: the state needs one for `/hot-scenes`, which these tests do not
/// call, and the v4 fence, which stays unavailable without an audience.
pub fn lazy_pool() -> PgPool {
    sqlx::postgres::PgPoolOptions::new()
        .max_connections(1)
        .acquire_timeout(std::time::Duration::from_millis(500))
        .connect_lazy("postgres://lazy@127.0.0.1:1/catalyst")
        .expect("lazy pool")
}

/// The gatekeeper the connector runs beside, over a pool that never connects: `bus` is its NATS
/// link (an in-process bus, or a broker), and without one it has the disabled link of a
/// deployment with no `NATS_URL`. Its island settings are the defaults, and
/// [`start_island_feed`] runs its island subscriber beside a connector, as the primary does. Its
/// LiveKit is comms' test credentials, the `devkey`/`devsecret` placeholders: a kick evicts
/// nobody.
pub fn gatekeeper(bus: Option<Arc<dyn NatsBus>>) -> SharedState {
    gatekeeper_with_livekit(bus, gatekeeper::Settings::for_tests().livekit)
}

/// [`gatekeeper`] with `livekit` in place of comms' test credentials.
pub fn gatekeeper_with_livekit(bus: Option<Arc<dyn NatsBus>>, livekit: LiveKit) -> SharedState {
    let state = comms_state(livekit);
    Arc::new(match bus {
        Some(bus) => state.with_bus(bus),
        None => state,
    })
}

/// LiveKit as a deployment with real credentials configures it: a kick reaches the RoomService
/// at `api_url`.
pub fn configured_livekit(api_url: &str) -> LiveKit {
    LiveKit {
        api_key: "APIkey".into(),
        api_secret: "secret".into(),
        client_url: "wss://lk.example".into(),
        api_url: api_url.into(),
        configured: true,
    }
}

fn comms_state(livekit: LiveKit) -> CommsState {
    let mut settings = gatekeeper::Settings::for_tests();
    settings.livekit = livekit;
    CommsState::new(
        lazy_pool(),
        Outbound::for_tests().allowing_private_hosts(),
        settings,
        None,
        None,
        Arc::new(StubPolicy::default()),
        Arc::new(StubScenes::default()),
        Arc::new(catalyst::comms::presence::Presence::new()),
        Arc::new(catalyst::comms::interfaces::test_support::StubDirectory::default()),
    )
    .with_ban_status(Arc::new(
        catalyst::comms::gatekeeper::moderation::test_support::StubBanStatus::default(),
    ))
}

/// Starts the gatekeeper's island subscriber on `jobs` with `state`'s sockets attached, as
/// `serve` does once the connector is built: a session the connector announces makes the
/// subscriber ask Pulse (`peer.{wallet}.cluster_lookup`) for its island.
pub fn start_island_feed(state: &AppState, jobs: &catalyst::jobs::Jobs) {
    state.gatekeeper.start_island_feed(jobs, state.clone());
}

pub fn ban_checker(
    policy: Arc<catalyst::worlds::policy::Policy>,
) -> Arc<catalyst::comms::archipelago::ban::BanChecker> {
    let bans = Arc::new(catalyst::comms::gatekeeper::moderation::Moderation::new(
        policy.pool().clone(),
    ));
    catalyst::comms::archipelago::ban::BanChecker::new(policy, bans)
}

pub fn allow_ban_checker() -> Arc<catalyst::comms::archipelago::ban::BanChecker> {
    catalyst::comms::archipelago::ban::BanChecker::new(
        allow_policy(),
        Arc::new(catalyst::comms::gatekeeper::moderation::test_support::StubBanStatus::default()),
    )
}

pub fn allow_policy() -> Arc<dyn AccessPolicy> {
    Arc::new(catalyst::comms::interfaces::test_support::StubPolicy::default())
}

/// The connector takes its dependencies from this gatekeeper, as production does.
pub async fn build(settings: Settings, gatekeeper: SharedState) -> AppState {
    archipelago::build_state(settings, gatekeeper, &catalyst::jobs::Jobs::new())
        .await
        .expect("state")
}

pub async fn state_with(settings: Settings, pool: PgPool, mut gatekeeper: SharedState) -> AppState {
    Arc::get_mut(&mut gatekeeper)
        .expect("unshared test gatekeeper")
        .pool = pool;
    build(settings, gatekeeper).await
}

pub async fn state(settings: Settings) -> AppState {
    state_with(settings, lazy_pool(), gatekeeper(None)).await
}

/// Binds a loopback listener for the primary's stats mount of the connector and returns its
/// port; the server task runs until the runtime ends.
pub async fn serve(state: AppState) -> u16 {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind");
    let port = listener.local_addr().unwrap().port();
    let app = archipelago::mounts(state, true).stats;
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    port
}
