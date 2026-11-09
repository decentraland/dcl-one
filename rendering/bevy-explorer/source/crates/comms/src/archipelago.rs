use anyhow::{anyhow, bail};
use bevy::{
    platform::collections::HashMap,
    prelude::*,
    tasks::{IoTaskPool, Task},
};
use futures_util::{pin_mut, select, FutureExt};
use http::HeaderValue;
use platform::IntoClientRequest;
use prost::Message;
use serde_json::json;
use tokio::sync::mpsc::{Receiver, Sender};

use common::{
    rpc::{RpcCall, RpcCallEvent, RpcEventSender},
    structs::{CurrentRealm, WorldHold},
    util::TaskExt,
};
use wallet::Wallet;

use crate::{
    global_crdt::GlobalCrdtState,
    room_connection::{
        AdmissionError, ConnectionHealth, CredentialRequest, FailureKind, NeedsAdapter,
        RoomRecovery, RoomSuspended,
    },
    AdapterManager, Transport, TransportType,
};

use super::NetworkMessage;

use dcl_component::{
    proto_components::{
        common::Position,
        kernel::comms::{
            rfc4,
            v3::{
                client_packet, server_packet, ChallengeRequestMessage, ChallengeResponseMessage,
                ClientPacket, ControlError, ControlErrorCode, Heartbeat, KickedReason,
                ServerPacket, SignedChallengeMessage,
            },
        },
    },
    DclReader,
};

pub struct ArchipelagoPlugin;

impl Plugin for ArchipelagoPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                reconnect_websocket,
                refresh_island.run_if(not(resource_exists::<WorldHold>)),
                start_archipelago,
                connect_websocket,
                manage_islands.run_if(not(resource_exists::<WorldHold>)),
                collect_realm_subscribers,
                settle_island.run_if(not(resource_exists::<WorldHold>)),
            )
                .chain(),
        );

        app.add_event::<StartArchipelago>();
        app.init_resource::<IslandChannel>();
        app.init_resource::<RealmSubscribers>();
    }
}

#[derive(Event)]
pub struct StartArchipelago {
    pub entity: Entity,
    pub address: String,
}

pub struct StartIsland {
    owner: Entity,
    generation: u64,
    connect_str: String,
    name: String,
}

#[derive(Resource)]
pub struct IslandChannel {
    sender: tokio::sync::mpsc::Sender<StartIsland>,
    receiver: tokio::sync::mpsc::Receiver<StartIsland>,
}

impl Default for IslandChannel {
    fn default() -> Self {
        let (sender, receiver) = tokio::sync::mpsc::channel(100);
        Self { sender, receiver }
    }
}

#[derive(Component)]
#[require(RoomRecovery, IslandAssignment, ConnectionHealth)]
pub struct ArchipelagoTransport {
    pub address: String,
    pub receiver: Option<Receiver<NetworkMessage>>,
    pub retries: usize,
    generation: u64,
}

#[derive(Component)]
pub struct ArchipelagoConnection {
    task: Task<(Receiver<NetworkMessage>, anyhow::Error)>,
    authenticated: bool,
}

pub fn start_archipelago(
    mut commands: Commands,
    mut archi_events: EventReader<StartArchipelago>,
    contexts: Query<Entity, With<GlobalCrdtState>>,
    existing: Query<&ArchipelagoTransport>,
    entities: Query<Entity>,
) {
    if let Some(ev) = archi_events
        .read()
        .filter(|event| entities.contains(event.entity))
        .last()
    {
        // archipelago is realm comms, primary-player driven: client-only (headless sets
        // DisableRealmComms), so the single shared context is its feed. Checked after
        // draining events so a failed lookup can't leave the reader cursor behind.
        let Ok(context) = contexts.single() else {
            return;
        };
        info!("starting archipelago protocol");
        let (sender, receiver) = tokio::sync::mpsc::channel(1000);

        let Ok(mut entity) = commands.get_entity(ev.entity) else {
            return;
        };
        entity.try_remove::<ArchipelagoConnection>();
        entity.try_insert((
            Transport {
                transport_type: TransportType::Archipelago,
                sender,
                control: None,
                context,
            },
            ArchipelagoTransport {
                address: ev.address.to_owned(),
                receiver: Some(receiver),
                retries: 0,
                generation: existing
                    .get(ev.entity)
                    .map_or(0, |transport| transport.generation.wrapping_add(1)),
            },
            ConnectionHealth::default(),
        ));
    }
}

#[derive(Component, Default)]
struct IslandAssignment {
    active: Option<Entity>,
    pending: Option<Entity>,
    // Credential refresh replaces the island entity, but must retain its retry budget.
    retries: HashMap<String, crate::recovery::RetryState>,
}

#[derive(Resource, Default)]
struct RealmSubscribers(Vec<RpcEventSender>);

fn collect_realm_subscribers(
    mut events: EventReader<RpcCallEvent>,
    mut subscribers: ResMut<RealmSubscribers>,
) {
    subscribers.0.retain(|sender| !sender.is_closed());
    for event in events.read() {
        if let RpcCall::SubscribeRealmChanged { sender } = &event.call {
            subscribers.0.push(sender.clone());
        }
    }
}

#[derive(Component)]
pub(crate) struct IslandOwner(pub Entity);

#[derive(Component)]
pub(crate) struct RefreshIsland;

#[derive(Component)]
struct IslandName(String);

#[derive(Component)]
struct IslandGeneration(u64);

fn island_connected(
    island: Entity,
    health: &Query<&ConnectionHealth>,
    #[cfg(feature = "livekit")] connected: &Query<(), With<crate::livekit::room::Connected>>,
) -> bool {
    #[cfg(feature = "livekit")]
    if connected.contains(island) {
        return true;
    }
    health.get(island).is_ok_and(ConnectionHealth::is_connected)
}

#[allow(clippy::too_many_arguments)]
fn manage_islands(
    mut commands: Commands,
    mut manager: AdapterManager,
    mut channel: ResMut<IslandChannel>,
    mut owners: Query<(&ArchipelagoTransport, &mut IslandAssignment)>,
    contexts: Query<Entity, With<GlobalCrdtState>>,
    islands: Query<(&IslandName, &IslandOwner)>,
    health: Query<&ConnectionHealth>,
    #[cfg(feature = "livekit")] connected: Query<(), With<crate::livekit::room::Connected>>,
    #[cfg(feature = "livekit")] connecting: Query<
        (),
        Or<(
            With<crate::livekit::room::Connecting>,
            With<crate::livekit::room::Reconnecting>,
        )>,
    >,
) {
    let mut latest = None;
    while let Ok(island) = channel.receiver.try_recv() {
        if owners
            .get(island.owner)
            .is_ok_and(|(transport, _)| island.generation == transport.generation)
        {
            latest = Some(island);
        }
    }
    let Some(island) = latest else {
        return;
    };
    let Ok((_, mut assignment)) = owners.get_mut(island.owner) else {
        return;
    };
    let Ok(context) = contexts.single() else {
        return;
    };
    if let Some(active) = assignment.active {
        if islands
            .get(active)
            .is_ok_and(|(name, owner)| name.0 == island.name && owner.0 == island.owner)
            && (island_connected(
                active,
                &health,
                #[cfg(feature = "livekit")]
                &connected,
            ) || {
                #[cfg(feature = "livekit")]
                {
                    connecting.contains(active)
                }
                #[cfg(not(feature = "livekit"))]
                {
                    false
                }
            })
        {
            if let Some(pending) = assignment
                .pending
                .take()
                .filter(|pending| *pending != active)
            {
                if let Ok(mut entity) = commands.get_entity(pending) {
                    entity.try_despawn();
                }
            }
            return;
        }
    }
    if let Some(pending) = assignment.pending {
        let same = islands
            .get(pending)
            .is_ok_and(|(name, owner)| name.0 == island.name && owner.0 == island.owner);
        let busy = island_connected(
            pending,
            &health,
            #[cfg(feature = "livekit")]
            &connected,
        ) || {
            #[cfg(feature = "livekit")]
            {
                connecting.contains(pending)
            }
            #[cfg(not(feature = "livekit"))]
            {
                false
            }
        };
        if same && busy {
            // This fresh assignment confirms the in-flight candidate without dialing a duplicate.
            commands
                .entity(pending)
                .try_insert(IslandGeneration(island.generation));
            return;
        }
    }
    if let Some(entity) = manager.connect(&island.connect_str, context) {
        if let Some(pending) = assignment.pending.replace(entity) {
            if Some(pending) != assignment.active {
                if let Ok(mut old) = commands.get_entity(pending) {
                    old.try_despawn();
                }
            }
        }
        commands.entity(entity).try_insert((
            RoomRecovery {
                retry: assignment
                    .retries
                    .get(&island.name)
                    .cloned()
                    .unwrap_or_default(),
                failure: None,
            },
            IslandOwner(island.owner),
            IslandGeneration(island.generation),
            IslandName(island.name),
            common::util::DespawnWith(island.owner),
        ));
    }
}

#[allow(clippy::too_many_arguments)]
fn settle_island(
    mut commands: Commands,
    mut owners: Query<(Entity, &ArchipelagoTransport, &mut IslandAssignment)>,
    health: Query<&ConnectionHealth>,
    #[cfg(feature = "livekit")] connected: Query<(), With<crate::livekit::room::Connected>>,
    islands: Query<(&IslandName, &IslandOwner, &IslandGeneration)>,
    current_realm: Res<CurrentRealm>,
    mut senders: ResMut<RealmSubscribers>,
) {
    for (owner, transport, mut assignment) in &mut owners {
        if assignment
            .active
            .is_some_and(|active| !islands.contains(active))
        {
            assignment.active = None;
        }
        let Some(pending) = assignment.pending else {
            continue;
        };
        let Ok((name, pending_owner, generation)) = islands.get(pending) else {
            assignment.pending = None;
            continue;
        };
        if pending_owner.0 != owner || generation.0 != transport.generation {
            assignment.pending = None;
            if pending_owner.0 == owner && Some(pending) != assignment.active {
                commands.entity(pending).try_despawn();
            }
            continue;
        }
        if !island_connected(
            pending,
            &health,
            #[cfg(feature = "livekit")]
            &connected,
        ) {
            continue;
        }
        if let Some(old) = assignment
            .active
            .replace(pending)
            .filter(|old| *old != pending)
        {
            if let Ok(mut entity) = commands.get_entity(old) {
                entity.try_despawn();
            }
        }
        assignment.pending = None;
        assignment.retries.clear();
        let server = current_realm
            .config
            .realm_name
            .as_deref()
            .unwrap_or_default();
        let message = json!({"serverName": server, "room": name.0, "displayName": format!("{server}-{}", name.0), "domain": current_realm.address}).to_string();
        senders.0.retain(|sender| {
            let _ = sender.send(message.clone());
            !sender.is_closed()
        });
    }
}

fn refresh_island(
    mut commands: Commands,
    mut owners: Query<
        (
            Entity,
            &mut ArchipelagoTransport,
            &ConnectionHealth,
            Has<CredentialRequest>,
            &mut IslandAssignment,
        ),
        With<RefreshIsland>,
    >,
    mut starts: EventWriter<StartArchipelago>,
    islands: Query<(&IslandName, &RoomRecovery)>,
) {
    for (entity, mut transport, health, refreshable, mut assignment) in &mut owners {
        for island in [assignment.active, assignment.pending]
            .into_iter()
            .flatten()
        {
            if let Ok((name, recovery)) = islands.get(island) {
                if recovery.failure == Some(FailureKind::Transient) {
                    assignment
                        .retries
                        .insert(name.0.clone(), recovery.retry.clone());
                }
            }
        }
        health.disconnected();
        transport.generation = transport.generation.wrapping_add(1);
        commands
            .entity(entity)
            .try_remove::<(RefreshIsland, ArchipelagoConnection)>();
        if refreshable {
            commands.entity(entity).try_insert(NeedsAdapter);
        } else {
            starts.write(StartArchipelago {
                entity,
                address: transport.address.clone(),
            });
        }
    }
}

#[allow(clippy::type_complexity)]
fn connect_websocket(
    mut commands: Commands,
    mut new_websockets: Query<
        (Entity, &mut ArchipelagoTransport, &RoomRecovery),
        (
            Without<ArchipelagoConnection>,
            Without<NeedsAdapter>,
            Without<RoomSuspended>,
        ),
    >,
    wallet: Res<Wallet>,
    island_channel: Res<IslandChannel>,
    time: Res<Time>,
) {
    if wallet.address().is_none() {
        return;
    }
    for (transport_id, mut new_transport, recovery) in new_websockets.iter_mut() {
        if !recovery.can_retry(time.elapsed_secs_f64()) {
            continue;
        }
        let Some(receiver) = new_transport.receiver.take() else {
            continue;
        };
        new_transport.generation = new_transport.generation.wrapping_add(1);
        let remote_address = new_transport.address.to_owned();
        let wallet = wallet.clone();
        let sender = island_channel.sender.clone();
        let health = ConnectionHealth::default();
        let task = IoTaskPool::get().spawn(archipelago_handler(
            transport_id,
            new_transport.generation,
            remote_address,
            wallet,
            receiver,
            sender,
            health.clone(),
        ));
        commands.entity(transport_id).try_insert((
            ArchipelagoConnection {
                task,
                authenticated: false,
            },
            health,
        ));
    }
}

fn reconnect_websocket(
    mut commands: Commands,
    mut websockets: Query<(
        Entity,
        &mut ArchipelagoTransport,
        &mut ArchipelagoConnection,
        &mut RoomRecovery,
        &ConnectionHealth,
        Has<CredentialRequest>,
    )>,
    time: Res<Time>,
) {
    for (entity, mut transport, mut connection, mut recovery, health, refreshable) in
        &mut websockets
    {
        if health.is_connected() && !connection.authenticated {
            connection.authenticated = true;
            transport.retries = 0;
            recovery.reset();
        }
        if let Some((receiver, error)) = connection.task.complete() {
            health.disconnected();
            transport.generation = transport.generation.wrapping_add(1);
            transport.receiver = Some(receiver);
            transport.retries = transport.retries.saturating_add(1);
            let error = error
                .downcast_ref::<AdmissionError>()
                .copied()
                .unwrap_or(AdmissionError::new(FailureKind::Transient));
            recovery.fail(time.elapsed_secs_f64(), error);
            commands
                .entity(entity)
                .try_remove::<ArchipelagoConnection>();
            if refreshable && !error.terminal() {
                commands.entity(entity).try_insert(NeedsAdapter);
            }
            warn!(?entity, kind = ?error.kind, "archipelago disconnected");
        }
    }
}

async fn archipelago_handler(
    transport_id: Entity,
    generation: u64,
    remote_address: String,
    wallet: Wallet,
    mut receiver: Receiver<NetworkMessage>,
    sender: Sender<StartIsland>,
    health: ConnectionHealth,
) -> (Receiver<NetworkMessage>, anyhow::Error) {
    let _health_lifetime = health.reset_on_drop();
    let res = archipelago_handler_inner(
        transport_id,
        generation,
        remote_address,
        wallet,
        &mut receiver,
        sender,
        &health,
    )
    .await;
    (receiver, res.err().unwrap_or(anyhow!("connection closed")))
}

async fn archipelago_handler_inner(
    transport_id: Entity,
    generation: u64,
    remote_address: String,
    wallet: Wallet,
    receiver: &mut Receiver<NetworkMessage>,
    sender: Sender<StartIsland>,
    health: &ConnectionHealth,
) -> Result<(), anyhow::Error> {
    debug!("connecting archipelago");

    let remote_address = if remote_address.starts_with("ws:") || remote_address.starts_with("wss:")
    {
        remote_address
    } else {
        format!("wss://{remote_address}")
    };

    let url = crate::adapter::validate_url(&remote_address)
        .map_err(|_| AdmissionError::new(FailureKind::Invalid))?;
    if !matches!(url.scheme(), "ws" | "wss") {
        return Err(AdmissionError::new(FailureKind::Invalid).into());
    }
    let mut request = remote_address
        .into_client_request()
        .map_err(|_| AdmissionError::new(FailureKind::Invalid))?;
    request.headers_mut().append(
        "Sec-WebSocket-Protocol",
        HeaderValue::from_static("archipelago"),
    );

    let mut stream = platform::websocket(request).await?;
    debug!("<< stream connected");

    // send peer identification
    let ident = ClientPacket {
        request_sequence: 0,
        message: Some(client_packet::Message::ChallengeRequest(
            ChallengeRequestMessage {
                address: format!(
                    "{:#x}",
                    wallet.address().ok_or(anyhow!("wallet not connected"))?
                ),
            },
        )),
    };
    stream.send(ident.encode_to_vec().into()).await?;
    debug!("archipelago challenge request sent");

    // challenge / welcome
    loop {
        let Some(response) = stream.next().await else {
            bail!("stream closed unexpectedly awaiting challenge")
        };
        let response = response?;
        let response = ServerPacket::decode(response.into_data().as_slice())?;
        let Some(message) = response.message else {
            bail!("received empty packet")
        };

        match message {
            server_packet::Message::Welcome(_) => {
                debug!("archipelago welcome received");
                health.connected();
                break;
            }
            server_packet::Message::ChallengeResponse(ChallengeResponseMessage {
                challenge_to_sign,
                already_connected,
                ..
            }) => {
                if already_connected {
                    return Err(AdmissionError::new(FailureKind::IdentityConflict).into());
                }
                // send challenge response
                debug!("archipelago challenge received");

                if !challenge_to_sign.starts_with("dcl-") {
                    return Err(AdmissionError::new(FailureKind::Invalid).into());
                }

                // sign challenge
                let chain = wallet.sign_message(challenge_to_sign).await?;
                let auth_chain_json = serde_json::to_string(&chain)?;
                debug!("archipelago auth chain created");

                // send response
                let message = ClientPacket {
                    request_sequence: 0,
                    message: Some(client_packet::Message::SignedChallenge(
                        SignedChallengeMessage { auth_chain_json },
                    )),
                };
                let message = message.encode_to_vec();
                stream.send(message.into()).await?;
                debug!(">> auth chain sent");
            }
            server_packet::Message::Kicked(kicked) => {
                return Err(kick_error(kicked.reason).into());
            }
            server_packet::Message::Error(error) => {
                return Err(control_error(&error).into());
            }
            _ => bail!("unexpected archipelago handshake packet"),
        }
    }

    let (mut write, mut read) = stream.split();

    // wrap and transmit outbound heartbeat
    let f_write = async move {
        while let Some(next) = receiver.recv().await {
            let Some(bytes) = next.message.to_rfc4() else {
                continue;
            };
            let Ok(rfc4::Packet {
                message: Some(rfc4::packet::Message::Position(pos)),
                ..
            }) = DclReader::new(&bytes).read()
            else {
                // skip non-position messages
                continue;
            };

            let packet = ClientPacket {
                request_sequence: 0,
                message: Some(client_packet::Message::Heartbeat(Heartbeat {
                    sample_sequence: None,
                    position: Some(Position {
                        x: pos.position_x,
                        y: pos.position_y,
                        z: pos.position_z,
                    }),
                    desired_room: None,
                })),
            };
            let mut buf = Vec::default();
            packet.encode(&mut buf)?;
            write.send(buf.into()).await?;
        }

        Ok::<(), anyhow::Error>(())
    }
    .fuse();

    // unwrap and forward inbound messages
    let f_read = async move {
        while let Some(next) = read.next().await {
            let next = next?;
            let next = ServerPacket::decode(next.into_data().as_slice())?;
            let Some(message) = next.message else {
                bail!("received empty packet")
            };

            match message {
                server_packet::Message::ChallengeResponse(_)
                | server_packet::Message::Welcome(_) => {
                    warn!("unexpected archipelago handshake packet after welcome");
                    continue;
                }
                server_packet::Message::IslandChanged(change) => {
                    sender
                        .send(StartIsland {
                            owner: transport_id,
                            generation,
                            connect_str: change.conn_str,
                            name: change.island_id,
                        })
                        .await?;
                }
                server_packet::Message::Kicked(kicked) => {
                    return Err(kick_error(kicked.reason).into());
                }
                server_packet::Message::LeftIsland(_)
                | server_packet::Message::JoinIsland(_)
                | server_packet::Message::Ack(_) => {}
                server_packet::Message::Error(error) => {
                    warn!(
                        code = error.code,
                        retry = error.retry,
                        "archipelago control error"
                    );
                    return Err(control_error(&error).into());
                }
            }
        }

        Ok(())
    }
    .fuse();

    // until either stream is broken
    pin_mut!(f_read, f_write);
    select! {
        read_res = f_read => read_res,
        write_res = f_write => write_res,
    }
}

fn kick_error(reason: i32) -> AdmissionError {
    AdmissionError::new(if reason == KickedReason::KrNewSession as i32 {
        FailureKind::IdentityConflict
    } else {
        FailureKind::Transient
    })
}

fn control_error(error: &ControlError) -> AdmissionError {
    AdmissionError::new(match error.code {
        code if code == ControlErrorCode::AuthFailed as i32 => FailureKind::Denied,
        code if [
            ControlErrorCode::InvalidFrame,
            ControlErrorCode::ProtocolViolation,
            ControlErrorCode::InvalidLane,
        ]
        .iter()
        .any(|kind| code == *kind as i32) =>
        {
            FailureKind::Invalid
        }
        _ => FailureKind::Transient,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{signed_login::StartSignedLogin, websocket_room::StartWsRoom};

    fn island_app() -> App {
        let mut app = App::new();
        app.add_plugins(ArchipelagoPlugin)
            .init_resource::<CurrentRealm>()
            .init_resource::<Wallet>()
            .init_resource::<Time>()
            .add_event::<StartWsRoom>()
            .add_event::<StartSignedLogin>()
            .add_event::<RpcCallEvent>();
        #[cfg(feature = "livekit")]
        app.add_event::<crate::livekit::StartLivekit>();
        app.world_mut().spawn(GlobalCrdtState::new(None));
        app
    }

    fn owner(world: &mut World, generation: u64) -> Entity {
        world
            .spawn(ArchipelagoTransport {
                address: "wss://example.org/ws".to_owned(),
                receiver: None,
                retries: 0,
                generation,
            })
            .id()
    }

    fn island(
        world: &mut World,
        owner: Entity,
        generation: u64,
        name: &str,
        connected: bool,
    ) -> Entity {
        let health = ConnectionHealth::default();
        if connected {
            health.connected();
        }
        world
            .spawn((
                IslandOwner(owner),
                IslandGeneration(generation),
                IslandName(name.to_owned()),
                health,
            ))
            .id()
    }

    fn assign(app: &App, owner: Entity, generation: u64, name: &str) {
        assert!(app
            .world()
            .resource::<IslandChannel>()
            .sender
            .try_send(StartIsland {
                owner,
                generation,
                connect_str: "ws-room:wss://example.org/room".to_owned(),
                name: name.to_owned(),
            })
            .is_ok());
    }

    #[cfg(feature = "livekit")]
    #[test]
    fn repeated_assignment_keeps_an_existing_sdk_attempt() {
        for active in [false, true] {
            let mut app = island_app();
            let owner = owner(app.world_mut(), 7);
            let room = island(app.world_mut(), owner, 7, "same", false);
            app.world_mut().entity_mut(room).insert((
                crate::livekit::LivekitTransport {
                    address: "wss://example.org?access_token=test".into(),
                    retries: 0,
                },
                crate::livekit::room::Connecting,
            ));
            let mut assignment = app.world_mut().get_mut::<IslandAssignment>(owner).unwrap();
            if active {
                assignment.active = Some(room);
            } else {
                assignment.pending = Some(room);
            }
            assign(&app, owner, 7, "same");
            app.update();
            let assignment = app.world().get::<IslandAssignment>(owner).unwrap();
            assert_eq!(
                if active {
                    assignment.active
                } else {
                    assignment.pending
                },
                Some(room)
            );
            assert!(app.world().resource::<Events<StartWsRoom>>().is_empty());
        }
    }

    #[test]
    fn failed_island_retry_budget_survives_assignment_refresh() {
        let mut app = island_app();
        let owner = owner(app.world_mut(), 7);
        let failed = island(app.world_mut(), owner, 7, "same", false);
        let mut recovery = RoomRecovery::default();
        recovery.fail(0.0, AdmissionError::new(FailureKind::Transient));
        recovery.fail(1.0, AdmissionError::new(FailureKind::Transient));
        app.world_mut().entity_mut(failed).insert(recovery);
        app.world_mut()
            .get_mut::<IslandAssignment>(owner)
            .unwrap()
            .pending = Some(failed);
        app.world_mut().entity_mut(owner).insert(RefreshIsland);
        app.update();
        let generation = app
            .world()
            .get::<ArchipelagoTransport>(owner)
            .unwrap()
            .generation;
        assign(&app, owner, generation, "same");
        app.update();
        let replacement = app
            .world()
            .get::<IslandAssignment>(owner)
            .unwrap()
            .pending
            .unwrap();
        let mut recovery = app
            .world_mut()
            .get_mut::<RoomRecovery>(replacement)
            .unwrap();
        assert_eq!(recovery.retry.attempts, 2);
        recovery.fail(3.0, AdmissionError::new(FailureKind::Transient));
        assert_eq!(recovery.retry.due, 7.0);
    }

    #[test]
    fn subscriptions_registered_during_hold_survive_until_handoff() {
        let mut app = island_app();
        app.insert_resource(WorldHold);
        let (sender, mut receiver) = RpcEventSender::channel();
        app.world_mut()
            .send_event(RpcCallEvent::engine(RpcCall::SubscribeRealmChanged {
                sender,
            }));
        for _ in 0..4 {
            app.update();
        }
        app.world_mut().remove_resource::<WorldHold>();
        let owner = owner(app.world_mut(), 7);
        let room = island(app.world_mut(), owner, 7, "arrived", true);
        app.world_mut()
            .get_mut::<IslandAssignment>(owner)
            .unwrap()
            .pending = Some(room);
        app.update();
        assert!(receiver.try_recv().unwrap().contains("arrived"));
    }

    #[test]
    fn stale_last_events_cannot_hide_a_current_assignment() {
        let mut app = island_app();
        let owner = owner(app.world_mut(), 7);
        let departed = app.world_mut().spawn_empty().id();
        app.world_mut().despawn(departed);
        assign(&app, owner, 7, "current");
        assign(&app, owner, 6, "old-generation");
        assign(&app, departed, 7, "departed-owner");

        app.update();

        let pending = app
            .world()
            .get::<IslandAssignment>(owner)
            .unwrap()
            .pending
            .unwrap();
        assert_eq!(app.world().get::<IslandName>(pending).unwrap().0, "current");
        assert!(app.world().resource::<IslandChannel>().receiver.is_empty());
    }

    #[test]
    fn handoff_keeps_the_active_island_until_the_replacement_is_connected() {
        let mut app = island_app();
        let owner = owner(app.world_mut(), 7);
        let active = island(app.world_mut(), owner, 7, "active", true);
        app.world_mut()
            .get_mut::<IslandAssignment>(owner)
            .unwrap()
            .active = Some(active);
        assign(&app, owner, 7, "next");

        app.update();

        let assignment = app.world().get::<IslandAssignment>(owner).unwrap();
        let pending = assignment.pending.unwrap();
        assert_eq!(assignment.active, Some(active));
        assert!(app.world().get_entity(active).is_ok());
        let health = ConnectionHealth::default();
        health.connected();
        app.world_mut().entity_mut(pending).insert(health);

        app.update();

        let assignment = app.world().get::<IslandAssignment>(owner).unwrap();
        assert_eq!(assignment.active, Some(pending));
        assert_eq!(assignment.pending, None);
        assert!(app.world().get_entity(active).is_err());
    }

    #[test]
    fn a_repeat_healthy_active_assignment_cancels_the_obsolete_handoff() {
        let mut app = island_app();
        let owner = owner(app.world_mut(), 7);
        let active = island(app.world_mut(), owner, 7, "active", true);
        let obsolete = island(app.world_mut(), owner, 7, "obsolete", false);
        app.world_mut().entity_mut(owner).insert(IslandAssignment {
            active: Some(active),
            pending: Some(obsolete),
            ..Default::default()
        });
        assign(&app, owner, 7, "active");

        app.update();

        let assignment = app.world().get::<IslandAssignment>(owner).unwrap();
        assert_eq!(assignment.active, Some(active));
        assert_eq!(assignment.pending, None);
        assert!(app.world().get_entity(active).is_ok());
        assert!(app.world().get_entity(obsolete).is_err());
        assert!(app.world().resource::<Events<StartWsRoom>>().is_empty());
    }

    #[test]
    fn disappeared_or_stale_pending_islands_never_replace_the_active_island() {
        for disappeared in [false, true] {
            let mut app = island_app();
            let owner = owner(app.world_mut(), 8);
            let active = island(app.world_mut(), owner, 7, "active", true);
            let pending = island(app.world_mut(), owner, 7, "stale", true);
            app.world_mut().entity_mut(owner).insert(IslandAssignment {
                active: Some(active),
                pending: Some(pending),
                ..Default::default()
            });
            if disappeared {
                app.world_mut().despawn(pending);
            }

            app.update();

            let assignment = app.world().get::<IslandAssignment>(owner).unwrap();
            assert_eq!(assignment.active, Some(active));
            assert_eq!(assignment.pending, None);
            assert!(app.world().get_entity(pending).is_err());
        }
    }

    #[test]
    fn world_hold_defers_assignments_and_ready_handoffs() {
        let mut app = island_app();
        let owner = owner(app.world_mut(), 7);
        let active = island(app.world_mut(), owner, 7, "active", true);
        let pending = island(app.world_mut(), owner, 7, "pending", true);
        app.world_mut().entity_mut(owner).insert(IslandAssignment {
            active: Some(active),
            pending: Some(pending),
            ..Default::default()
        });
        app.insert_resource(WorldHold);
        assign(&app, owner, 7, "latest");

        app.update();

        let assignment = app.world().get::<IslandAssignment>(owner).unwrap();
        assert_eq!(assignment.active, Some(active));
        assert_eq!(assignment.pending, Some(pending));
        app.world_mut().remove_resource::<WorldHold>();
        app.update();
        let assignment = app.world().get::<IslandAssignment>(owner).unwrap();
        assert_eq!(assignment.active, Some(active));
        assert_ne!(assignment.pending, Some(pending));
        assert_eq!(
            app.world()
                .get::<IslandName>(assignment.pending.unwrap())
                .unwrap()
                .0,
            "latest"
        );
        assert!(app.world().get_entity(pending).is_err());
    }

    #[test]
    fn world_hold_defers_credential_refresh_and_preserves_socket_health() {
        let mut app = island_app();
        let owner = owner(app.world_mut(), 7);
        app.world()
            .get::<ConnectionHealth>(owner)
            .unwrap()
            .connected();
        app.world_mut().entity_mut(owner).insert(RefreshIsland);
        app.insert_resource(WorldHold);

        app.update();

        assert!(app.world().get::<RefreshIsland>(owner).is_some());
        assert!(app
            .world()
            .get::<ConnectionHealth>(owner)
            .unwrap()
            .is_connected());
        assert_eq!(
            app.world()
                .get::<ArchipelagoTransport>(owner)
                .unwrap()
                .generation,
            7
        );
        app.world_mut().remove_resource::<WorldHold>();
        app.update();
        assert!(app.world().get::<RefreshIsland>(owner).is_none());
        assert!(!app
            .world()
            .get::<ConnectionHealth>(owner)
            .unwrap()
            .is_connected());
        assert!(
            app.world()
                .get::<ArchipelagoTransport>(owner)
                .unwrap()
                .generation
                > 7
        );
    }

    #[test]
    fn a_stale_start_event_does_not_hide_a_live_owner_start() {
        let mut app = App::new();
        app.add_event::<StartArchipelago>()
            .add_systems(Update, start_archipelago);
        app.world_mut().spawn(GlobalCrdtState::new(None));
        let current = app.world_mut().spawn_empty().id();
        let departed = app.world_mut().spawn_empty().id();
        app.world_mut().despawn(departed);
        for entity in [current, departed] {
            app.world_mut()
                .resource_mut::<Events<StartArchipelago>>()
                .send(StartArchipelago {
                    entity,
                    address: "wss://example.org/ws".to_owned(),
                });
        }

        app.update();

        assert!(app.world().get::<ArchipelagoTransport>(current).is_some());
        assert!(!app
            .world()
            .get::<ConnectionHealth>(current)
            .unwrap()
            .is_connected());
    }

    #[test]
    fn protocol_denial_and_identity_conflict_are_distinct_terminal_failures() {
        let conflict = kick_error(KickedReason::KrNewSession as i32);
        assert_eq!(conflict.kind, FailureKind::IdentityConflict);
        assert!(conflict.terminal());

        let denied = control_error(&ControlError {
            code: ControlErrorCode::AuthFailed as i32,
            retry: 0,
            public_detail: Some("private server response".to_owned()),
        });
        assert_eq!(denied.kind, FailureKind::Denied);
        assert!(denied.terminal());
        assert!(!denied.to_string().contains("private server response"));

        let unavailable = control_error(&ControlError {
            code: ControlErrorCode::AuthorityUnavailable as i32,
            retry: 0,
            public_detail: None,
        });
        assert_eq!(unavailable.kind, FailureKind::Transient);
        assert!(!unavailable.terminal());
    }
}
