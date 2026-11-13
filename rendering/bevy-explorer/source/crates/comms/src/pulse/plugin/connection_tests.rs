use std::time::{Duration, Instant};

use bevy::ecs::system::RunSystemOnce;
use bevy::tasks::TaskPool;
use tokio::sync::mpsc::error::TryRecvError;

use super::*;
use crate::profile::UserProfile;
use crate::pulse::from_movement;
use crate::pulse::transport::{PulseDisconnect, PulseDriverChannels};
use crate::{Broadcast, Emote, NetworkMessageRecipient};

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
        resyncs: [(0, 0); 60],
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
        local_emote: None,
        pacer: DiscreteEventPacer::default(),
        // Exact waits, so a test can name the time a reconnect is due; the policy's own tests pin
        // the jitter.
        policy: ReconnectPolicy::exact(),
        accepted: false,
        initial_realm: None,
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
    joined_at(subject_id, wallet, 0)
}

/// `wallet` entering `HOME` as `subject_id`, in the parcel the servers index as `parcel_index`.
fn joined_at(subject_id: u32, wallet: &str, parcel_index: i32) -> pulse::server_message::Message {
    pulse::server_message::Message::PlayerJoined(pulse::PlayerJoined {
        user_id: wallet.to_owned(),
        profile_version: 1,
        state: Some(pulse::PlayerStateFull {
            subject_id,
            state: Some(pulse::PlayerState {
                parcel_index,
                ..default()
            }),
            ..default()
        }),
        realm: HOME.to_owned(),
    })
}

/// The departures reported to `updates` since the last look: each peer, with the transport it left.
fn departures(updates: &mut mpsc::Receiver<NetworkUpdate>) -> Vec<(Entity, Address)> {
    std::iter::from_fn(|| updates.try_recv().ok())
        .filter_map(|update| match update {
            NetworkUpdate::PlayerLeft {
                transport_id,
                address,
            } => Some((transport_id, address)),
            _ => None,
        })
        .collect()
}

/// When a `Down` connection is dialled again; `None` in any other state.
fn respawn_at(session: &PulseSession) -> Option<f64> {
    match session.state {
        Connection::Down { respawn_at } => Some(respawn_at),
        _ => None,
    }
}

/// A pose the player could have sent last.
fn pose() -> pulse::PlayerState {
    pulse::PlayerState {
        parcel_index: 7,
        ..default()
    }
}

/// The local movement the engine broadcasts, at a spot of its own.
fn movement() -> rfc4::Movement {
    rfc4::Movement {
        position_x: 8.0,
        position_y: 1.0,
        position_z: 8.0,
        ..default()
    }
}

/// The local emote the engine broadcasts: a one-shot of 1.5 s on the upper body.
fn emote(stopping: bool) -> Emote {
    Emote {
        urn: "wave".to_owned(),
        incremental_id: 1,
        timestamp: 0.0,
        duration_ms: Some(1500),
        stopping,
        mask: EmoteMask::UpperBody,
    }
}

/// A frame the server was sent: the kind it rode in, and the message.
type Sent = (FrameKind, pulse::client_message::Message);

/// The announcement of a profile `version`, as it goes out.
fn profile_announcement(version: i32) -> Sent {
    (
        FrameKind::Control,
        pulse::client_message::Message::ProfileAnnouncement(pulse::ProfileVersionAnnouncement {
            version,
        }),
    )
}

/// Where a scene puts the player: the middle of parcel `x`, 0.
fn standing_in(x: i32) -> Vec3 {
    Vec3::new(16.0 * x as f32 + 8.0, 1.0, 0.0)
}

/// The index the servers give the parcel `standing_in(x)` is in, as a teleport names it.
fn parcel_index(x: i32) -> i32 {
    PulseParcelGrid::default()
        .encode_to_parcel(standing_in(x))
        .0
}

/// The parcel of each teleport among `sent`, in order.
fn teleported_parcels(sent: &[Sent]) -> Vec<i32> {
    sent.iter()
        .filter_map(|(_, message)| match message {
            pulse::client_message::Message::Teleport(teleport) => Some(teleport.parcel_index),
            _ => None,
        })
        .collect()
}

/// A player's session in a world on `HOME`. The test holds the far end of each of its channels: the
/// driver's end of the link, the receiver behind the routing transport's sink, which stands in for
/// the engine, and the sender behind its outbox, where the engine queues what it broadcasts. The
/// player stands in the world. No wallet is signed in, except by `idle` for a connection that never
/// goes `Down`, so nothing here dials a real server.
struct Harness {
    world: World,
    driver: PulseDriverChannels,
    updates: mpsc::Receiver<NetworkUpdate>,
    broadcasts: mpsc::Sender<NetworkMessage>,
    routing: Entity,
    user: Entity,
}

impl Harness {
    /// A player whose connection is in `state`.
    fn player(state: Connection) -> Self {
        let mut world = World::new();
        let (sink, updates) = mpsc::channel(64);
        let (broadcasts, outbox) = mpsc::channel(8);
        let routing = world
            .spawn((PulseOutbox(outbox), PulseSink(sink.clone())))
            .id();
        let user = world
            .spawn((PrimaryUser::default(), GlobalTransform::default()))
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
        world.init_resource::<Events<PlayerTeleported>>();
        Self {
            world,
            driver,
            updates,
            broadcasts,
            routing,
            user,
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

    /// A scene listener in the player's place, its handshake accepted, hosting a scene of `HOME` at
    /// each of `parcels`. Each scene has a Pulse transport of its own, and the receiver behind
    /// each one's sink, which stands in for that scene's crdt context, comes back with the
    /// transport, in the order of `parcels`.
    fn listener<const N: usize>(
        parcels: [IVec2; N],
    ) -> (Self, [(Entity, mpsc::Receiver<NetworkUpdate>); N]) {
        let mut harness = Self::player(Connection::Established);
        let scenes = parcels.map(|_| {
            let (sink, updates) = mpsc::channel(64);
            (harness.world.spawn(PulseSink(sink)).id(), updates)
        });
        let hosted = parcels
            .iter()
            .zip(&scenes)
            .map(|(parcel, (transport, _))| (*parcel, *transport))
            .collect();
        // What its handshake announced: a rect for each scene.
        let aoi = vec![pulse::SceneListenerAoi {
            realm: HOME.to_owned(),
            parcel_rects: parcels
                .iter()
                .map(|parcel| pulse::ParcelRect {
                    min_x: parcel.x,
                    min_z: parcel.y,
                    max_x: parcel.x,
                    max_z: parcel.y,
                })
                .collect(),
        }];
        let (mut session, driver) = session_with_link(PulseRole::Listener(ListenerRole {
            transport_by_parcel: [(HOME.to_owned(), hosted)].into_iter().collect(),
            announced: aoi.clone(),
            aoi,
            ..default()
        }));
        session.state = Connection::Established;
        session.accepted = true;
        harness.world.insert_resource(session);
        harness.driver = driver;
        (harness, scenes)
    }

    /// A player at `Idle`, ready to sign: the wallet has an identity, and the profile is at 7.
    fn idle() -> Self {
        IoTaskPool::get_or_init(TaskPool::default);
        let mut harness = Self::player(Connection::Idle);
        let mut wallet = Wallet::default();
        wallet.finalize_as_guest();
        harness.world.insert_resource(wallet);
        harness.profile(7);
        harness
    }

    fn session(&self) -> &PulseSession {
        self.world.resource::<PulseSession>()
    }

    fn session_mut(&mut self) -> Mut<'_, PulseSession> {
        self.world.resource_mut::<PulseSession>()
    }

    /// The session dials again and comes up in `state` on a fresh link, whose driver end replaces
    /// the harness's.
    fn redial(&mut self, state: Connection) {
        let (link, driver) = transport::pulse_channels(16);
        let mut session = self.world.resource_mut::<PulseSession>();
        session.link = Some(link);
        session.state = state;
        self.driver = driver;
    }

    /// The profile has loaded, at `version`.
    fn profile(&mut self, version: u32) {
        self.world.insert_resource(CurrentUserProfile {
            profile: Some(UserProfile {
                version,
                ..default()
            }),
            ..default()
        });
    }

    /// The player is behind the loading screen.
    fn leave_world(&mut self) {
        self.world.entity_mut(self.user).insert(OutOfWorld);
    }

    /// The player is placed in the world.
    fn enter_world(&mut self) {
        self.world.entity_mut(self.user).remove::<OutOfWorld>();
    }

    /// Pumps at `now` until the handshake is signed and sent — the IO pool does that off this
    /// thread — and returns what it asks. A connection that is not signing it is not going to
    /// send it: a sign error closes it.
    fn signed_handshake(&mut self, now: f64) -> pulse::HandshakeRequest {
        let deadline = Instant::now() + Duration::from_secs(30);
        self.pump_at(now);
        while !matches!(self.session().state, Connection::AwaitingResponse { .. }) {
            assert!(
                matches!(self.session().state, Connection::Signing(_)),
                "the handshake was not sent: the connection is {:?}",
                self.session().state
            );
            assert!(Instant::now() < deadline, "the handshake was never sent");
            std::thread::sleep(Duration::from_millis(1));
            self.pump_at(now);
        }
        let sent = self.sent();
        let [(FrameKind::Control, pulse::client_message::Message::Handshake(handshake))] =
            sent.as_slice()
        else {
            panic!("{sent:?}")
        };
        handshake.clone()
    }

    /// The player is instantly repositioned to `position`, and the engine says so.
    fn teleport_to(&mut self, position: Vec3) {
        self.world.send_event(PlayerTeleported { position });
        self.world
            .run_system_once(pulse_teleport_on_local_move)
            .unwrap();
    }

    /// The engine broadcasts `message` on the routing transport.
    fn broadcast(&self, message: impl Broadcast + 'static) {
        self.broadcasts
            .try_send(NetworkMessage {
                message: Box::new(message),
                unreliable: false,
                recipient: NetworkMessageRecipient::All,
                topic: None,
            })
            .unwrap();
    }

    /// What the driver was handed since the last look.
    fn sent(&mut self) -> Vec<Sent> {
        std::iter::from_fn(|| self.driver.outbound.try_recv().ok())
            .map(|frame| {
                let message = pulse::ClientMessage::decode(frame.bytes.as_slice())
                    .unwrap()
                    .message
                    .unwrap();
                (frame.kind, message)
            })
            .collect()
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

    fn advance_to(&mut self, now: f64) {
        self.world
            .resource_mut::<Time>()
            .advance_to(Duration::from_secs_f64(now));
    }

    /// One `pump_pulse`, with the engine's clock at `now`.
    fn pump_at(&mut self, now: f64) {
        self.advance_to(now);
        self.world.run_system_once(pump_pulse).unwrap();
    }

    /// One `drain_pulse_outbox`, with the engine's clock at `now`.
    fn drain_at(&mut self, now: f64) {
        self.advance_to(now);
        self.world.run_system_once(drain_pulse_outbox).unwrap();
    }

    fn follow_realm(&mut self) {
        self.world.run_system_once(follow_realm).unwrap();
    }

    /// The debug panel connects the player, or disconnects it.
    fn set_connection_enabled(&mut self, enabled: bool) {
        self.world
            .run_system_once(
                move |mut session: ResMut<PulseSession>, mut commands: Commands| {
                    session.set_connection_enabled(enabled, false, &mut commands)
                },
            )
            .unwrap()
            .unwrap();
    }

    /// The peers reported as left since the last look, every one of them on the routing transport.
    fn left(&mut self) -> Vec<Address> {
        departures(&mut self.updates)
            .into_iter()
            .map(|(transport, address)| {
                assert_eq!(transport, self.routing);
                address
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

/// A scene listener routes each peer to the scene it stands in, so a drop reports each departure
/// on that scene's own transport: presence is per context.
#[test]
fn a_listeners_peers_leave_their_scene_transports_on_a_drop() {
    let (mut harness, [(first, mut first_updates), (second, mut second_updates)]) =
        Harness::listener([IVec2::new(0, 0), IVec2::new(5, 0)]);
    harness.receive(joined_at(1, W1, parcel_index(0)));
    harness.receive(joined_at(2, W2, parcel_index(5)));
    harness.pump_at(5.0);

    harness.report(PulseStatus::Disconnected(PulseDisconnect::Graceful));
    harness.pump_at(10.0);

    assert_eq!(departures(&mut first_updates), [(first, address(W1))]);
    assert_eq!(departures(&mut second_updates), [(second, address(W2))]);
}

/// The servers refuse at connect, before any handshake, and the floor holds there too.
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

    // The routing entity is lost and built again in the same realm: no way out of `Dead`.
    harness.world.despawn(harness.routing);
    harness.follow_realm();
    assert!(matches!(harness.session().state, Connection::Dead));

    harness.world.insert_resource(realm("away"));
    harness.follow_realm();
    assert_eq!(respawn_at(harness.session()), Some(0.0));
}

/// The attempts that failed before an accepted connection are forgiven: the next failure is the
/// first of a new run, which waits the first second again.
#[test]
fn acceptance_resets_the_backoff() {
    let mut harness = Harness::player(Connection::Connecting);

    harness.report(PulseStatus::Failed("no route".to_owned()));
    harness.pump_at(10.0);
    assert_eq!(respawn_at(harness.session()), Some(11.0));

    harness.redial(Connection::Connecting);
    harness.report(PulseStatus::Failed("no route".to_owned()));
    harness.pump_at(20.0);
    assert_eq!(respawn_at(harness.session()), Some(22.0));

    harness.redial(Connection::AwaitingResponse { timeout_at: 40.0 });
    harness.receive(handshake_response(true));
    harness.pump_at(30.0);
    assert!(matches!(harness.session().state, Connection::Established));

    harness.report(PulseStatus::Failed("reset".to_owned()));
    harness.pump_at(31.0);
    assert_eq!(respawn_at(harness.session()), Some(32.0));
}

/// The attempts that failed before a terminal stop are forgiven when a realm change ends it: the
/// next failure is the first of a new run, which waits the first second again.
#[test]
fn a_realm_change_out_of_a_terminal_stop_starts_the_backoff_over() {
    let mut harness = Harness::player(Connection::Connecting);

    harness.report(PulseStatus::Failed("no route".to_owned()));
    harness.pump_at(10.0);
    harness.redial(Connection::Connecting);
    harness.report(PulseStatus::Failed("no route".to_owned()));
    harness.pump_at(20.0);
    assert_eq!(respawn_at(harness.session()), Some(22.0));

    harness.redial(Connection::Connecting);
    harness.report(PulseStatus::Disconnected(PulseDisconnect::Banned));
    harness.pump_at(30.0);
    assert!(matches!(harness.session().state, Connection::Dead));

    harness.world.insert_resource(realm("away"));
    harness.follow_realm();
    assert_eq!(respawn_at(harness.session()), Some(0.0));

    harness.redial(Connection::Connecting);
    harness.report(PulseStatus::Failed("no route".to_owned()));
    harness.pump_at(40.0);
    assert_eq!(respawn_at(harness.session()), Some(41.0));
}

/// A pause from the debug panel parks the session with no link, and nothing dials it. A resume is
/// the user's own fresh start, as a realm change out of a terminal stop is: it dials at once, and
/// the next failure waits the first second again.
#[test]
fn a_resume_dials_at_once_and_starts_the_backoff_over() {
    let mut harness = Harness::player(Connection::Connecting);

    harness.report(PulseStatus::Failed("no route".to_owned()));
    harness.pump_at(10.0);
    harness.redial(Connection::Connecting);
    harness.report(PulseStatus::Failed("no route".to_owned()));
    harness.pump_at(20.0);
    assert_eq!(respawn_at(harness.session()), Some(22.0));

    harness.set_connection_enabled(false);
    harness.pump_at(30.0);
    assert!(matches!(harness.session().state, Connection::Paused));
    assert!(harness.session().link.is_none());

    harness.set_connection_enabled(true);
    assert_eq!(respawn_at(harness.session()), Some(0.0));

    harness.redial(Connection::Connecting);
    harness.report(PulseStatus::Failed("no route".to_owned()));
    harness.pump_at(40.0);
    assert_eq!(respawn_at(harness.session()), Some(41.0));
}

#[test]
fn no_initial_state_out_of_world_or_without_a_realm_or_pose() {
    let pose = pose();

    assert_eq!(
        initial_state(Some(&pose), None, Some(HOME), false, 1.0),
        None
    );
    assert_eq!(initial_state(Some(&pose), None, None, true, 1.0), None);
    assert_eq!(initial_state(None, None, Some(HOME), true, 1.0), None);
}

#[test]
fn the_initial_state_carries_the_realm_and_the_last_pose() {
    let pose = pose();

    assert_eq!(
        initial_state(Some(&pose), None, Some(HOME), true, 1.0),
        Some(pulse::PlayerInitialState {
            state: Some(pose),
            realm: HOME.to_owned(),
            ..default()
        })
    );
}

#[test]
fn a_looping_emote_is_carried_with_its_elapsed_time() {
    let emote = LocalEmote {
        urn: "wave".to_owned(),
        duration_ms: None,
        mask: Some(1),
        started_at: 10.0,
    };

    let initial = initial_state(Some(&pose()), Some(&emote), Some(HOME), true, 12.5).unwrap();

    assert_eq!(initial.emote_id.as_deref(), Some("wave"));
    assert_eq!(initial.emote_start_offset_ms, Some(2500));
    assert_eq!(initial.emote_duration_ms, None);
    assert_eq!(initial.emote_mask, Some(1));
}

/// Still playing: it goes with its duration, and with no offset until some time has passed.
#[test]
fn a_one_shot_emote_within_its_duration_is_carried_with_it() {
    let emote = LocalEmote {
        urn: "wave".to_owned(),
        duration_ms: Some(3000),
        mask: None,
        started_at: 10.0,
    };

    let at_the_start = initial_state(Some(&pose()), Some(&emote), Some(HOME), true, 10.0).unwrap();
    assert_eq!(at_the_start.emote_id.as_deref(), Some("wave"));
    assert_eq!(at_the_start.emote_duration_ms, Some(3000));
    assert_eq!(at_the_start.emote_start_offset_ms, None);
    assert_eq!(at_the_start.emote_mask, None);

    let later = initial_state(Some(&pose()), Some(&emote), Some(HOME), true, 11.0).unwrap();
    assert_eq!(later.emote_id.as_deref(), Some("wave"));
    assert_eq!(later.emote_duration_ms, Some(3000));
    assert_eq!(later.emote_start_offset_ms, Some(1000));
}

#[test]
fn a_finished_one_shot_emote_is_not_carried() {
    let pose = pose();
    let emote = LocalEmote {
        urn: "wave".to_owned(),
        duration_ms: Some(1000),
        mask: Some(1),
        started_at: 10.0,
    };

    assert_eq!(
        initial_state(Some(&pose), Some(&emote), Some(HOME), true, 11.5),
        Some(pulse::PlayerInitialState {
            state: Some(pose),
            realm: HOME.to_owned(),
            ..default()
        })
    );
}

#[test]
fn an_accepted_initial_state_sends_no_teleport_and_announces_the_version() {
    let mut harness = Harness::player(Connection::AwaitingResponse { timeout_at: 15.0 });
    harness.session_mut().initial_realm = Some(HOME.to_owned());
    harness.profile(7);

    harness.receive(handshake_response(true));
    harness.pump_at(5.0);

    assert!(matches!(harness.session().state, Connection::Established));
    assert_eq!(harness.sent(), [profile_announcement(7)]);
}

#[test]
fn a_realm_change_while_signing_sends_a_teleport_on_acceptance() {
    let mut harness = Harness::player(Connection::AwaitingResponse { timeout_at: 15.0 });
    harness.session_mut().initial_realm = Some(HOME.to_owned());
    harness.world.insert_resource(realm("away"));
    harness.profile(7);

    harness.receive(handshake_response(true));
    harness.pump_at(5.0);

    let sent = harness.sent();
    let [(FrameKind::Teleport, pulse::client_message::Message::Teleport(teleport)), announcement] =
        sent.as_slice()
    else {
        panic!("{sent:?}")
    };
    assert_eq!(teleport.realm, "away");
    assert_eq!(*announcement, profile_announcement(7));
}

/// Out of the world the position is provisional, so the teleport waits for the player to be
/// placed; the version is no position, and goes out.
#[test]
fn an_acceptance_out_of_world_defers_the_teleport_but_announces_the_version() {
    let mut harness = Harness::player(Connection::AwaitingResponse { timeout_at: 15.0 });
    harness.leave_world();
    harness.profile(7);

    harness.receive(handshake_response(true));
    harness.pump_at(5.0);

    assert_eq!(harness.sent(), [profile_announcement(7)]);
}

#[test]
fn no_announcement_before_the_profile_loads() {
    let mut harness = Harness::player(Connection::AwaitingResponse { timeout_at: 15.0 });
    harness.world.insert_resource(CurrentUserProfile::default());

    harness.receive(handshake_response(true));
    harness.pump_at(5.0);

    let sent = harness.sent();
    assert!(
        matches!(sent.as_slice(), [(FrameKind::Teleport, _)]),
        "{sent:?}"
    );
}

#[test]
fn the_outbox_keeps_the_caches_while_disconnected() {
    // A link that is up but has not been accepted yet carries nothing either.
    for state in [
        Connection::Down { respawn_at: 100.0 },
        Connection::AwaitingResponse { timeout_at: 100.0 },
    ] {
        let mut harness = Harness::player(state);

        harness.broadcast(movement());
        harness.drain_at(5.0);

        assert_eq!(
            harness.session().last_state,
            Some(from_movement(&movement(), &PulseParcelGrid::default()))
        );
        assert_eq!(
            harness.driver.outbound.try_recv().unwrap_err(),
            TryRecvError::Empty
        );
    }
}

/// An emote is kept from its start to its stop, so a handshake in between can say what is playing.
#[test]
fn an_emote_is_kept_from_its_start_to_its_stop() {
    let mut harness = Harness::established(10.0);
    // Acceptance sent its teleport; the frames below are the ones to look at.
    harness.sent();

    // Before any pose is sent there is nothing to start it from: no frame, and nothing kept.
    harness.broadcast(emote(false));
    harness.drain_at(11.0);
    assert_eq!(harness.session().local_emote, None);
    assert!(harness.sent().is_empty());

    harness.broadcast(movement());
    harness.broadcast(emote(false));
    harness.drain_at(12.0);
    assert_eq!(
        harness.session().local_emote,
        Some(LocalEmote {
            urn: "wave".to_owned(),
            duration_ms: Some(1500),
            mask: Some(1),
            started_at: 12.0,
        })
    );
    let kinds: Vec<_> = harness.sent().iter().map(|(kind, _)| *kind).collect();
    assert_eq!(kinds, [FrameKind::Input, FrameKind::Emote]);

    harness.broadcast(emote(true));
    harness.drain_at(13.0);
    assert_eq!(harness.session().local_emote, None);
    let kinds: Vec<_> = harness.sent().iter().map(|(kind, _)| *kind).collect();
    assert_eq!(kinds, [FrameKind::Emote]);
}

/// An emote the bucket allows leaves in its place in the queue. A start carries the pose it was
/// made in, so behind a newer movement it would step the avatar back.
#[test]
fn an_emote_queued_before_a_movement_leaves_before_it() {
    let mut harness = Harness::player(Connection::Established);
    // The player has moved before, so the emote has a pose to start from.
    harness.session_mut().last_state = Some(pose());

    harness.broadcast(emote(false));
    harness.broadcast(movement());
    harness.drain_at(10.0);

    let kinds: Vec<_> = harness.sent().iter().map(|(kind, _)| *kind).collect();
    assert_eq!(kinds, [FrameKind::Emote, FrameKind::Input]);
}

/// The whole way: a player that has sent a pose and an emote connects, and its handshake says where
/// it is and what it plays; the server accepts that as placing it in the realm.
#[test]
fn a_handshake_carries_the_players_state_and_the_acceptance_trusts_it() {
    let mut harness = Harness::idle();
    harness.session_mut().last_state = Some(pose());
    harness.session_mut().local_emote = Some(LocalEmote {
        urn: "wave".to_owned(),
        duration_ms: None,
        mask: Some(1),
        started_at: 2.0,
    });

    let handshake = harness.signed_handshake(4.5);
    assert_eq!(handshake.profile_version, 7);
    assert_eq!(
        handshake.initial_state,
        Some(pulse::PlayerInitialState {
            state: Some(pose()),
            realm: HOME.to_owned(),
            emote_id: Some("wave".to_owned()),
            emote_start_offset_ms: Some(2500),
            emote_mask: Some(1),
            ..default()
        })
    );

    harness.receive(handshake_response(true));
    harness.pump_at(5.0);
    assert_eq!(harness.sent(), [profile_announcement(7)]);
}

/// Connecting behind the loading screen: the handshake carries nothing, and the realm the
/// connection before it carried is no reason to skip the teleport once the player is placed.
#[test]
fn a_handshake_out_of_world_carries_nothing_and_the_acceptance_teleports() {
    let mut harness = Harness::idle();
    harness.session_mut().last_state = Some(pose());
    harness.session_mut().initial_realm = Some(HOME.to_owned());
    harness.leave_world();

    let handshake = harness.signed_handshake(4.5);
    assert_eq!(handshake.initial_state, None);

    harness.enter_world();
    harness.receive(handshake_response(true));
    harness.pump_at(5.0);
    let sent = harness.sent();
    let [(FrameKind::Teleport, _), announcement] = sent.as_slice() else {
        panic!("{sent:?}")
    };
    assert_eq!(*announcement, profile_announcement(7));
}

/// A teleport the player makes while the handshake is in flight is held, not lost: nothing goes
/// before the connection is accepted, and then it goes ahead of the profile announcement.
#[test]
fn a_teleport_made_while_the_handshake_is_in_flight_goes_out_on_acceptance() {
    let mut harness = Harness::player(Connection::AwaitingResponse { timeout_at: 15.0 });
    // The handshake carried this realm, so acceptance sends no teleport of its own.
    harness.session_mut().initial_realm = Some(HOME.to_owned());
    harness.profile(7);

    harness.teleport_to(standing_in(3));
    harness.drain_at(2.0);
    assert!(harness.sent().is_empty());

    harness.receive(handshake_response(true));
    harness.pump_at(5.0);

    let sent = harness.sent();
    let [(FrameKind::Teleport, pulse::client_message::Message::Teleport(teleport)), announcement] =
        sent.as_slice()
    else {
        panic!("{sent:?}")
    };
    assert_eq!(teleport.realm, HOME);
    assert_eq!(teleport.parcel_index, parcel_index(3));
    assert_eq!(*announcement, profile_announcement(7));
}

/// The handshake's initial state is built when signing starts, from the pose the player has
/// broadcast by then. A teleport made before that is not held for the acceptance: let out after
/// it, it would put the player back where it was.
#[test]
fn a_teleport_made_before_signing_starts_is_not_sent_on_acceptance() {
    for state in [Connection::Connecting, Connection::Idle] {
        let label = format!("{state:?}");
        let mut harness = Harness::idle();
        harness.session_mut().state = state;
        harness.session_mut().last_state = Some(pose());

        harness.teleport_to(standing_in(3));
        harness.report(PulseStatus::Connected);
        let initial = harness.signed_handshake(4.5).initial_state;
        assert_eq!(initial.map(|initial| initial.realm).as_deref(), Some(HOME));

        harness.receive(handshake_response(true));
        harness.pump_at(5.0);
        assert_eq!(harness.sent(), [profile_announcement(7)], "{label}");
    }
}

/// A teleport held for the acceptance names the realm it was made in, so a realm change drops it.
/// Out of world at the acceptance, nothing is sent then: the player's placement in the new realm
/// announces it.
#[test]
fn a_realm_change_drops_the_teleport_held_for_the_acceptance() {
    let mut harness = Harness::player(Connection::AwaitingResponse { timeout_at: 15.0 });
    harness.session_mut().initial_realm = Some(HOME.to_owned());
    harness.profile(7);
    harness.teleport_to(standing_in(3));

    harness.world.insert_resource(realm("away"));
    harness.follow_realm();
    harness.leave_world();
    harness.receive(handshake_response(true));
    harness.pump_at(5.0);

    assert_eq!(harness.sent(), [profile_announcement(7)]);
}

/// The handshake carries the loop playing when it was signed, and the server plays it from the
/// acceptance on. A stop made while the handshake is in flight goes out after the acceptance, or
/// every observer would see the loop until the player emoted again.
#[test]
fn an_emote_stopped_while_the_handshake_is_in_flight_stops_on_acceptance() {
    let mut harness = Harness::idle();
    harness.session_mut().last_state = Some(pose());
    harness.session_mut().local_emote = Some(LocalEmote {
        urn: "wave".to_owned(),
        duration_ms: None,
        mask: Some(1),
        started_at: 2.0,
    });

    let initial = harness.signed_handshake(4.5).initial_state.unwrap();
    assert_eq!(initial.emote_id.as_deref(), Some("wave"));
    assert_eq!(initial.emote_duration_ms, None);

    harness.broadcast(emote(true));
    harness.drain_at(4.6);
    assert!(harness.sent().is_empty());

    harness.receive(handshake_response(true));
    harness.pump_at(5.0);

    let sent = harness.sent();
    let [(FrameKind::Emote, pulse::client_message::Message::EmoteStop(_)), announcement] =
        sent.as_slice()
    else {
        panic!("{sent:?}")
    };
    assert_eq!(*announcement, profile_announcement(7));
}

/// A scene that repositions the player every frame would empty the servers' bucket in under a
/// second, and an overrun ends the session. Of twenty teleports in an instant only a burst goes
/// out; the one after it goes as a token returns, and it is the latest.
#[test]
fn a_burst_of_teleports_is_paced_and_ends_at_the_last_one() {
    let mut harness = Harness::player(Connection::Established);

    for x in 0..20 {
        harness.teleport_to(standing_in(x));
        harness.drain_at(10.0);
    }
    assert_eq!(
        teleported_parcels(&harness.sent()),
        (0..8).map(parcel_index).collect::<Vec<_>>()
    );

    harness.drain_at(10.11);
    assert_eq!(teleported_parcels(&harness.sent()), [parcel_index(19)]);
}

/// Emotes ride the servers' bucket with the teleports: with it empty, an emote waits for a token,
/// and the latest command is the one that goes.
#[test]
fn an_emote_waits_while_the_bucket_is_empty() {
    let mut harness = Harness::player(Connection::Established);
    harness.broadcast(movement());
    for x in 0..8 {
        harness.teleport_to(standing_in(x));
        harness.drain_at(10.0);
    }
    // The pose and the eight teleports are not what is looked at here.
    harness.sent();

    harness.broadcast(emote(false));
    harness.drain_at(10.0);
    assert!(harness.sent().is_empty());

    // Stopped before it has gone out, the stop takes the start's place.
    harness.broadcast(emote(true));
    harness.drain_at(10.05);
    assert!(harness.sent().is_empty());

    harness.drain_at(10.11);
    let sent = harness.sent();
    assert!(
        matches!(
            sent.as_slice(),
            [(
                FrameKind::Emote,
                pulse::client_message::Message::EmoteStop(_)
            )]
        ),
        "{sent:?}"
    );
}

/// `follow_realm` knows no time, so a realm change only offers its teleport; the frame's drain
/// lets it out.
#[test]
fn a_realm_change_while_connected_teleports_with_the_drain() {
    let mut harness = Harness::player(Connection::Established);
    harness.world.insert_resource(realm("away"));

    harness.follow_realm();
    assert!(harness.sent().is_empty());

    harness.drain_at(5.0);
    let sent = harness.sent();
    let [(FrameKind::Teleport, pulse::client_message::Message::Teleport(teleport))] =
        sent.as_slice()
    else {
        panic!("{sent:?}")
    };
    assert_eq!(teleport.realm, "away");
}

fn begin_resync_gap(session: &mut PulseSession, now: Instant) -> pulse::ResyncRequest {
    session.decoder.handle_at(
        pulse::ServerMessage {
            message: Some(joined(42, W1)),
        },
        now,
    );
    let events = session.decoder.handle_at(
        pulse::ServerMessage {
            message: Some(pulse::server_message::Message::PlayerStateDelta(
                pulse::PlayerStateDeltaTier0 {
                    subject_id: 42,
                    baseline_seq: 1,
                    new_seq: 2,
                    ..default()
                },
            )),
        },
        now,
    );
    let [PulseEvent::Resync(request)] = events.as_slice() else {
        panic!("{events:?}");
    };
    request.clone()
}

#[test]
fn partial_queue_admission_rotates_retries_while_an_admitted_subject_never_answers() {
    let (mut session, _) = session_with_link(PulseRole::Listener(ListenerRole::default()));
    let (link, mut driver) = transport::pulse_channels(1);
    session.link = Some(link);
    session.state = Connection::Established;
    let start = Instant::now();
    // Each subject gets its own wallet and outstanding gap.
    for id in 42..45 {
        let wallet = format!("0x{id:040x}");
        session.decoder.handle_at(
            pulse::ServerMessage {
                message: Some(joined(id, &wallet)),
            },
            start,
        );
        let events = session.decoder.handle_at(
            pulse::ServerMessage {
                message: Some(pulse::server_message::Message::PlayerStateDelta(
                    pulse::PlayerStateDeltaTier0 {
                        subject_id: id,
                        baseline_seq: 1,
                        new_seq: 2,
                        ..default()
                    },
                )),
            },
            start,
        );
        for event in events {
            if let PulseEvent::Resync(request) = event {
                send_resync(
                    session.link.as_mut(),
                    &mut session.resyncs,
                    &mut session.decoder,
                    request,
                    0.0,
                );
            }
        }
    }
    driver.outbound.try_recv().unwrap(); // Only subject 42 fit initially.
    let mut admitted = Vec::new();
    for seconds in [1, 3, 7, 12, 17, 22] {
        let now = start + Duration::from_secs(seconds);
        retry_resyncs(&mut session, now, 0.0);
        let frame = driver.outbound.try_recv().unwrap();
        let Some(pulse::client_message::Message::Resync(request)) =
            pulse::ClientMessage::decode(frame.bytes.as_slice())
                .unwrap()
                .message
        else {
            panic!("expected a resync")
        };
        admitted.push(request.subject_id);
        // Subject 42 never responds. Others recover and leave the retry set.
        if request.subject_id != 42 {
            session.decoder.handle_at(
                pulse::ServerMessage {
                    message: Some(pulse::server_message::Message::PlayerStateFull(
                        pulse::PlayerStateFull {
                            subject_id: request.subject_id,
                            sequence: 2,
                            server_tick: 2000,
                            state: Some(pose()),
                        },
                    )),
                },
                now,
            );
        }
        retry_resyncs(&mut session, now, 0.0);
        assert!(driver.outbound.try_recv().is_err());
    }
    assert_eq!(&admitted[..3], &[42, 43, 44]);
    assert_eq!(session.decoder.pending_resync.len(), 1);
    assert!(session.decoder.pending_resync.contains_key(&42));
    assert_eq!(session.resyncs[0], (0, 7));
}

#[test]
fn a_full_send_queue_recovers_through_the_pump_without_inbound_or_game_time_progress() {
    let mut harness = Harness::player(Connection::Established);
    let (link, driver) = transport::pulse_channels(1);
    harness.session_mut().link = Some(link);
    harness.driver = driver;
    let start = Instant::now();
    {
        let mut session = harness.session_mut();
        let request = begin_resync_gap(&mut session, start);
        assert!(session.link.as_mut().unwrap().send(PulseFrame {
            bytes: Vec::new(),
            reliability: PulseReliability::Reliable,
            kind: FrameKind::Control,
        }));
        let session = &mut *session;
        assert!(!send_resync(
            session.link.as_mut(),
            &mut session.resyncs,
            &mut session.decoder,
            request,
            0.0
        ));
        assert_eq!(
            session.resyncs.iter().map(|(_, count)| count).sum::<u32>(),
            0
        );
    }
    harness.driver.outbound.try_recv().unwrap();

    // Make the request due without relying on an Instant before the clock's origin.
    harness
        .session_mut()
        .decoder
        .pending_resync
        .get_mut(&42)
        .unwrap()
        .retry_at = start;
    // The real pump has no inbound packet and its game clock remains at zero.
    harness.pump_at(0.0);
    let frame = harness.driver.outbound.try_recv().unwrap();
    assert_eq!(frame.reliability, PulseReliability::Reliable);
    assert_eq!(frame.kind, FrameKind::Control);
    assert!(matches!(
        pulse::ClientMessage::decode(frame.bytes.as_slice())
            .unwrap()
            .message,
        Some(pulse::client_message::Message::Resync(
            pulse::ResyncRequest {
                subject_id: 42,
                known_seq: 0,
            }
        ))
    ));
    assert_eq!(harness.session().resyncs[0], (0, 1));

    harness.receive(pulse::server_message::Message::PlayerStateFull(
        pulse::PlayerStateFull {
            subject_id: 42,
            sequence: 2,
            server_tick: 2000,
            state: Some(pose()),
        },
    ));
    harness.pump_at(0.0);
    assert!(harness.session().decoder.pending_resync.is_empty());
    retry_resyncs(
        &mut harness.session_mut(),
        Instant::now() + Duration::from_secs(60),
        0.0,
    );
    assert!(harness.sent().is_empty());
    assert_eq!(harness.session().resyncs[0], (0, 1));
}

#[test]
fn an_unanswered_resync_retries_over_the_link_only_when_due() {
    let (mut session, mut driver) = session_with_link(PulseRole::Listener(ListenerRole::default()));
    session.state = Connection::Established;
    let start = Instant::now();
    let request = begin_resync_gap(&mut session, start);
    assert!(send_resync(
        session.link.as_mut(),
        &mut session.resyncs,
        &mut session.decoder,
        request,
        0.0
    ));
    driver.outbound.try_recv().unwrap();
    retry_resyncs(&mut session, start + Duration::from_millis(999), 0.0);
    assert!(driver.outbound.try_recv().is_err());
    retry_resyncs(&mut session, start + Duration::from_secs(1), 0.0);
    let frame = driver.outbound.try_recv().unwrap();
    assert!(matches!(
        pulse::ClientMessage::decode(frame.bytes.as_slice())
            .unwrap()
            .message,
        Some(pulse::client_message::Message::Resync(
            pulse::ResyncRequest {
                subject_id: 42,
                known_seq: 0,
            }
        ))
    ));
    retry_resyncs(&mut session, start + Duration::from_secs(1), 0.0);
    assert!(driver.outbound.try_recv().is_err());
    assert_eq!(session.resyncs[0], (0, 2));
}

#[test]
fn retry_polling_waits_for_an_established_connection_and_a_link() {
    let (mut session, mut driver) = session_with_link(PulseRole::Listener(ListenerRole::default()));
    let start = Instant::now();
    begin_resync_gap(&mut session, start);
    let due = start + Duration::from_secs(1);
    for state in [
        Connection::Connecting,
        Connection::Idle,
        Connection::AwaitingResponse { timeout_at: 10.0 },
        Connection::Dead,
    ] {
        session.state = state;
        retry_resyncs(&mut session, due, 0.0);
        assert!(driver.outbound.try_recv().is_err());
    }
    session.state = Connection::Established;
    let link = session.link.take();
    retry_resyncs(&mut session, due, 0.0);
    session.link = link;
    retry_resyncs(&mut session, due, 0.0);
    assert!(driver.outbound.try_recv().is_ok());
    assert_eq!(session.resyncs[0], (0, 1));
}
