use std::time::Duration;

use bevy::ecs::system::RunSystemOnce;
use tokio::sync::mpsc::error::TryRecvError;

use super::*;
use crate::pulse::transport::{PulseDisconnect, PulseDriverChannels};

const HOME: &str = "home";
const W1: &str = "0x0000000000000000000000000000000000000001";
const W2: &str = "0x0000000000000000000000000000000000000002";

fn address(wallet: &str) -> Address {
    wallet.parse().unwrap()
}

/// A session on a stub link, `Connecting` as a freshly spawned driver leaves it, and the driver's
/// end of that link, for the test to play.
fn session_with_link(role: PulseRole) -> (PulseSession, PulseDriverChannels) {
    let (link, driver) = transport::pulse_channels(16);
    let grid = PulseParcelGrid::default();
    let session = PulseSession {
        link: Some(link),
        _driver: None,
        decoder: PulseDecoder::new(grid),
        role,
        lsd_realm: LsdRealm::Unresolved { retry_at: 0.0 },
        realm_override: None,
        grid,
        transport_config: PulseTransportConfig {
            host: String::new(),
            port: 0,
            cert_hash: None,
        },
        server_id: String::new(),
        last_state: None,
        policy: ReconnectPolicy::default(),
        accepted: false,
        state: Connection::Connecting,
    };
    (session, driver)
}

/// The realm at `name`, called that too.
fn realm(name: &str) -> CurrentRealm {
    let mut realm = CurrentRealm {
        address: name.to_owned(),
        ..default()
    };
    realm.config.realm_name = Some(name.to_owned());
    realm
}

fn handshake_response(success: bool) -> pulse::server_message::Message {
    pulse::server_message::Message::Handshake(pulse::HandshakeResponse {
        success,
        ..default()
    })
}

/// `wallet` entering `HOME` as `subject_id`.
fn joined(subject_id: u32, wallet: &str) -> pulse::server_message::Message {
    pulse::server_message::Message::PlayerJoined(pulse::PlayerJoined {
        user_id: wallet.to_owned(),
        profile_version: 1,
        state: Some(pulse::PlayerStateFull {
            subject_id,
            state: Some(pulse::PlayerState::default()),
            ..default()
        }),
        realm: HOME.to_owned(),
    })
}

/// When a `Down` connection is dialled again; `None` in any other state.
fn respawn_at(session: &PulseSession) -> Option<f64> {
    match session.state {
        Connection::Down { respawn_at } => Some(respawn_at),
        _ => None,
    }
}

/// A player's session in a world on `HOME`. The test holds the far end of each of its channels: the
/// driver's end of the link, and the receiver behind the routing transport's sink, which stands in
/// for the engine. No wallet is signed in, so nothing here dials a real server.
struct Harness {
    world: World,
    driver: PulseDriverChannels,
    updates: mpsc::Receiver<NetworkUpdate>,
    routing: Entity,
}

impl Harness {
    /// A player whose connection is in `state`.
    fn player(state: Connection) -> Self {
        let mut world = World::new();
        let (sink, updates) = mpsc::channel(64);
        let (_, outbox) = mpsc::channel(1);
        let routing = world
            .spawn((PulseOutbox(outbox), PulseSink(sink.clone())))
            .id();
        let (mut session, driver) = session_with_link(PulseRole::Player(PlayerRole {
            context: Entity::PLACEHOLDER,
            sink,
            routing_transport: Some(routing),
            routing_realm: Some(HOME.to_owned()),
            replay_pending: false,
        }));
        session.state = state;
        world.insert_resource(session);
        world.insert_resource(realm(HOME));
        world.init_resource::<Wallet>();
        world.init_resource::<Time>();
        Self {
            world,
            driver,
            updates,
            routing,
        }
    }

    /// A player whose handshake the server accepted at `now`.
    fn established(now: f64) -> Self {
        let mut harness = Self::player(Connection::AwaitingResponse {
            timeout_at: now + HANDSHAKE_RESPONSE_TIMEOUT_SECS,
        });
        harness.receive(handshake_response(true));
        harness.pump_at(now);
        harness
    }

    fn session(&self) -> &PulseSession {
        self.world.resource::<PulseSession>()
    }

    /// The server sends `message`.
    fn receive(&self, message: pulse::server_message::Message) {
        let bytes = pulse::ServerMessage {
            message: Some(message),
        }
        .encode_to_vec();
        self.driver.inbound.try_send(bytes).unwrap();
    }

    /// The driver reports `status`.
    fn report(&self, status: PulseStatus) {
        self.driver.status.try_send(status).unwrap();
    }

    /// One `pump_pulse`, with the engine's clock at `now`.
    fn pump_at(&mut self, now: f64) {
        self.world
            .resource_mut::<Time>()
            .advance_to(Duration::from_secs_f64(now));
        self.world.run_system_once(pump_pulse).unwrap();
    }

    fn follow_realm(&mut self) {
        self.world.run_system_once(follow_realm).unwrap();
    }

    /// The peers reported as left since the last look, every one of them on the routing transport.
    fn left(&mut self) -> Vec<Address> {
        std::iter::from_fn(|| self.updates.try_recv().ok())
            .filter_map(|update| match update {
                NetworkUpdate::PlayerLeft {
                    transport_id,
                    address,
                } => {
                    assert_eq!(transport_id, self.routing);
                    Some(address)
                }
                _ => None,
            })
            .collect()
    }
}

#[test]
fn a_drop_reports_every_held_peer_as_left() {
    let mut harness = Harness::established(10.0);
    harness.receive(joined(1, W1));
    harness.receive(joined(2, W2));
    harness.pump_at(15.0);

    harness.report(PulseStatus::Disconnected(PulseDisconnect::None));
    harness.pump_at(20.0);

    assert_eq!(harness.left(), [address(W1), address(W2)]);
    assert_eq!(respawn_at(harness.session()), Some(23.0));
}

/// A Pulse server that restarts closes every client with GRACEFUL.
#[test]
fn a_server_restart_drops_peers_and_reconnects_after_three_seconds() {
    let mut harness = Harness::established(40.0);
    harness.receive(joined(1, W1));
    harness.pump_at(45.0);

    harness.report(PulseStatus::Disconnected(PulseDisconnect::Graceful));
    harness.pump_at(50.0);

    assert_eq!(harness.left(), [address(W1)]);
    assert_eq!(respawn_at(harness.session()), Some(53.0));
}

/// The servers refuse at connect, before any handshake, where unity-explorer's 60 s never applies.
#[test]
fn a_refusal_at_connect_waits_for_its_floor() {
    let mut harness = Harness::player(Connection::Connecting);

    harness.report(PulseStatus::Disconnected(PulseDisconnect::ServerFull));
    harness.pump_at(10.0);

    assert_eq!(respawn_at(harness.session()), Some(70.0));
}

#[test]
fn a_rejected_handshake_closes_the_connection_and_backs_off() {
    let mut harness = Harness::player(Connection::AwaitingResponse { timeout_at: 15.0 });

    harness.receive(handshake_response(false));
    harness.pump_at(5.0);

    assert!(harness.session().link.is_none());
    assert_eq!(respawn_at(harness.session()), Some(6.0));
    // closed, with nothing sent after the rejection: no second handshake goes out on it
    assert_eq!(
        harness.driver.outbound.try_recv().unwrap_err(),
        TryRecvError::Disconnected
    );
}

#[test]
fn an_unanswered_handshake_closes_the_connection() {
    let mut harness = Harness::player(Connection::AwaitingResponse { timeout_at: 10.0 });

    harness.pump_at(10.5);

    assert!(harness.session().link.is_none());
    assert_eq!(respawn_at(harness.session()), Some(11.5));
}

#[test]
fn a_terminal_reason_parks_until_the_next_realm_change() {
    let mut harness = Harness::established(1.0);

    harness.report(PulseStatus::Disconnected(PulseDisconnect::Banned));
    harness.pump_at(2.0);
    assert!(matches!(harness.session().state, Connection::Dead));

    harness.follow_realm();
    assert!(matches!(harness.session().state, Connection::Dead));

    harness.world.insert_resource(realm("away"));
    harness.follow_realm();
    assert_eq!(respawn_at(harness.session()), Some(0.0));
}
