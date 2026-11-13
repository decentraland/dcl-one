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
        None => state.with_bus(statistics_bus()),
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
    state
        .started
        .store(true, std::sync::atomic::Ordering::Release);
    let mounts = archipelago::mounts(
        state,
        true,
        Default::default(),
        catalyst::comms::stats::http::test_support::matcher(),
        std::sync::Arc::new(catalyst::content::Denylist::default()),
    );
    let app = catalyst::comms::stats::http::merge_root(mounts.stats, mounts.statistics);
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    port
}

/// In-process broker fixture running the actual Pulse statistics store on observations.
#[derive(Default)]
struct StatisticsBus {
    bus: catalyst::comms::gatekeeper::bus::InProcessBus,
    statistics: std::sync::Mutex<catalyst::comms::pulse::statistics::Statistics>,
}
fn statistics_sequence() -> u64 {
    static SEQUENCE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
    SEQUENCE.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
}
#[async_trait::async_trait]
impl NatsBus for StatisticsBus {
    async fn statistics_control(
        &self,
        payload: Vec<u8>,
    ) -> Result<Vec<u8>, catalyst::comms::gatekeeper::bus::RequestError> {
        use catalyst::comms::protocol::bus::statistics::ProducerRequest;
        let request = ProducerRequest::decode(&payload)
            .ok_or(catalyst::comms::gatekeeper::bus::RequestError::Invalid)?;
        self.statistics
            .lock()
            .unwrap()
            .control(request)
            .encode()
            .ok_or(catalyst::comms::gatekeeper::bus::RequestError::Invalid)
    }
    fn is_enabled(&self) -> bool {
        true
    }
    fn is_connected(&self) -> bool {
        true
    }
    fn is_ready(&self) -> bool {
        true
    }
    fn connect(&self) {}
    fn subscribe(
        &self,
        subject: &str,
        queue: Option<&str>,
        handler: catalyst::comms::gatekeeper::bus::NatsHandler,
    ) -> catalyst::comms::gatekeeper::bus::NatsSubscription {
        self.bus.subscribe(subject, queue, handler)
    }
    async fn publish(
        &self,
        subject: &str,
        payload: Vec<u8>,
    ) -> catalyst::comms::gatekeeper::bus::PublishOutcome {
        use catalyst::comms::protocol::bus::statistics::*;
        if subject == OBSERVATIONS_SUBJECT {
            let mut store = self.statistics.lock().unwrap();
            if let Some(event) = Observation::decode(&payload) {
                store.observe(event, chrono::Utc::now().timestamp_millis());
                let sequence = statistics_sequence();
                for frame in frames(&store.snapshot(), uuid::Uuid::nil(), sequence, 65536).unwrap()
                {
                    self.bus.inject(SNAPSHOTS_SUBJECT, &frame);
                }
            }
        }
        self.bus.publish(subject, payload).await
    }
    async fn request(
        &self,
        subject: &str,
        payload: Vec<u8>,
    ) -> Result<Vec<u8>, catalyst::comms::gatekeeper::bus::RequestError> {
        self.bus.request(subject, payload).await
    }
}
/// Fixture writes still cross the typed complete-snapshot transport, never the reader's state.
pub async fn snapshot(
    state: &AppState,
    change: impl FnOnce(&mut catalyst::comms::protocol::bus::statistics::Snapshot),
) {
    use catalyst::comms::protocol::bus::statistics::*;
    let mut snapshot = (*state.statistics.snapshot()).clone();
    change(&mut snapshot);
    for frame in frames(&snapshot, uuid::Uuid::nil(), statistics_sequence(), 65536).unwrap() {
        assert_eq!(
            state.gatekeeper.bus.publish(SNAPSHOTS_SUBJECT, frame).await,
            catalyst::comms::gatekeeper::bus::PublishOutcome::Submitted
        );
    }
}
pub async fn heartbeat(
    state: &AppState,
    address: String,
    position: [f32; 3],
    _parcel: [i32; 2],
    _realm: String,
) {
    snapshot(state, |snapshot| {
        let peer = catalyst::comms::protocol::bus::statistics::Peer {
            address: address.clone(),
            position: position.map(f64::from),
            time: chrono::Utc::now().timestamp_millis(),
        };
        if let Some(held) = snapshot.peers.iter_mut().find(|p| p.address == address) {
            *held = peer;
        } else {
            snapshot.peers.push(peer);
        }
    })
    .await;
}
pub fn peer(
    state: &AppState,
    address: &str,
) -> Option<catalyst::comms::protocol::bus::statistics::Peer> {
    state
        .statistics
        .snapshot()
        .peers
        .iter()
        .find(|peer| peer.address == address)
        .cloned()
}
pub async fn disconnect(state: &AppState, address: &str) {
    snapshot(state, |snapshot| {
        snapshot.peers.retain(|peer| peer.address != address)
    })
    .await;
}
pub async fn topology(state: &AppState, bytes: Vec<u8>) {
    snapshot(state, |snapshot| snapshot.islands = bytes).await;
}
pub async fn discovery(state: &AppState, bytes: Vec<u8>) {
    snapshot(state, |snapshot| snapshot.discovery = bytes).await;
}

pub fn statistics_bus() -> Arc<dyn NatsBus> {
    Arc::new(StatisticsBus::default())
}
