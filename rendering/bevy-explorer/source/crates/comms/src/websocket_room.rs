use anyhow::{anyhow, bail};
use bevy::{
    prelude::*,
    tasks::{IoTaskPool, Task},
};
use bimap::BiMap;
use futures_util::{pin_mut, select, FutureExt};
use http::HeaderValue;
use platform::IntoClientRequest;
use prost::Message;
use tokio::sync::mpsc::{Receiver, Sender};

use common::util::{dcl_assert, AsH160, TaskExt};
use dcl_component::proto_components::kernel::comms::{
    rfc4,
    rfc5::{
        ws_packet, WsChallengeRequired, WsIdentification, WsPacket, WsPeerUpdate, WsSignedChallenge,
    },
};
use wallet::Wallet;

use crate::{
    global_crdt::{NetworkUpdate, PlayerMessage},
    profile::CurrentUserProfile,
    room_connection::{
        AdmissionError, ConnectionHealth, CredentialRequest, FailureKind, NeedsAdapter,
        RoomRecovery,
    },
    Transport, TransportType,
};

use super::{
    global_crdt::{GlobalCrdtState, PlayerUpdate},
    NetworkMessage,
};

pub struct WebsocketRoomPlugin;

impl Plugin for WebsocketRoomPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (connect_websocket, reconnect_websocket, start_ws_room),
        );
        app.add_event::<StartWsRoom>();
    }
}

#[derive(Event)]
pub struct StartWsRoom {
    pub entity: Entity,
    pub address: String,
    /// crdt context this room's transport feeds
    pub context: Entity,
}

#[derive(Component)]
#[require(RoomRecovery)]
pub struct WebsocketRoomTransport {
    pub address: String,
    pub receiver: Option<Receiver<NetworkMessage>>,
}

#[derive(Component)]
pub struct WebSocketConnection(Task<(Receiver<NetworkMessage>, anyhow::Error)>);

pub fn start_ws_room(
    mut commands: Commands,
    mut room_events: EventReader<StartWsRoom>,
    current_profile: Res<CurrentUserProfile>,
) {
    if let Some(ev) = room_events.read().last() {
        info!("starting ws-room protocol");
        let (sender, receiver) = tokio::sync::mpsc::channel(1000);

        // Never announce an edit still deploying.
        if let Some(current_profile) = current_profile.peer_profile() {
            let response = rfc4::Packet {
                message: Some(rfc4::packet::Message::ProfileVersion(
                    rfc4::AnnounceProfileVersion {
                        profile_version: current_profile.version,
                    },
                )),
                protocol_version: 100,
            };
            let _ = sender.try_send(NetworkMessage::reliable(&response));
        }

        let Ok(mut entity) = commands.get_entity(ev.entity) else {
            return;
        };
        entity.try_remove::<WebSocketConnection>();
        entity.try_insert((
            Transport {
                transport_type: TransportType::WebsocketRoom,
                sender,
                control: None,
                context: ev.context,
            },
            WebsocketRoomTransport {
                address: ev.address.to_owned(),
                receiver: Some(receiver),
            },
        ));
    }
}

#[allow(clippy::type_complexity)]
fn connect_websocket(
    mut commands: Commands,
    mut new_websockets: Query<
        (
            Entity,
            &mut WebsocketRoomTransport,
            &Transport,
            &RoomRecovery,
        ),
        (Without<WebSocketConnection>, Without<NeedsAdapter>),
    >,
    wallet: Res<Wallet>,
    contexts: Query<&GlobalCrdtState>,
    time: Res<Time>,
) {
    for (transport_id, mut new_transport, transport, recovery) in new_websockets.iter_mut() {
        if !recovery.can_retry(time.elapsed_secs_f64()) {
            continue;
        }
        let Ok(player_state) = contexts.get(transport.context) else {
            continue;
        };
        let remote_address = new_transport.address.to_owned();
        let wallet = wallet.clone();
        let Some(receiver) = new_transport.receiver.take() else {
            continue;
        };
        let sender = player_state.get_sender();
        let health = ConnectionHealth::default();
        let task = IoTaskPool::get().spawn(websocket_room_handler(
            transport_id,
            remote_address,
            wallet,
            receiver,
            sender,
            health.clone(),
        ));
        commands
            .entity(transport_id)
            .try_insert((WebSocketConnection(task), health));
    }
}

fn reconnect_websocket(
    mut commands: Commands,
    mut websockets: Query<(
        Entity,
        &mut WebsocketRoomTransport,
        &mut WebSocketConnection,
        &mut RoomRecovery,
        &ConnectionHealth,
        Has<CredentialRequest>,
    )>,
    time: Res<Time>,
) {
    for (entity, mut transport, mut connection, mut recovery, health, refreshable) in
        &mut websockets
    {
        if health.is_connected() && recovery.failure == Some(FailureKind::Transient) {
            recovery.failure = None;
        }
        if let Some((receiver, error)) = connection.0.complete() {
            transport.receiver = Some(receiver);
            let error = error
                .downcast_ref::<AdmissionError>()
                .cloned()
                .unwrap_or(AdmissionError::new(FailureKind::Transient));
            commands.entity(entity).try_remove::<WebSocketConnection>();
            if refreshable && !error.terminal() {
                commands.entity(entity).try_insert(NeedsAdapter);
            }
            warn!(?entity, kind = ?error.kind, "websocket room disconnected");
            recovery.fail(time.elapsed_secs_f64(), error);
        }
    }
}

async fn websocket_room_handler(
    transport_id: Entity,
    remote_address: String,
    wallet: Wallet,
    mut receiver: Receiver<NetworkMessage>,
    sender: Sender<NetworkUpdate>,
    health: ConnectionHealth,
) -> (Receiver<NetworkMessage>, anyhow::Error) {
    let _health_lifetime = health.reset_on_drop();
    let res = websocket_room_handler_inner(
        transport_id,
        remote_address,
        wallet,
        &mut receiver,
        sender,
        &health,
    )
    .await;
    (receiver, res.err().unwrap_or(anyhow!("connection closed")))
}

async fn websocket_room_handler_inner(
    transport_id: Entity,
    remote_address: String,
    wallet: Wallet,
    receiver: &mut Receiver<NetworkMessage>,
    sender: Sender<NetworkUpdate>,
    health: &ConnectionHealth,
) -> Result<(), anyhow::Error> {
    debug!("connecting websocket room");

    let remote_address = if remote_address.starts_with("ws:") || remote_address.starts_with("wss:")
    {
        remote_address
    } else {
        format!("wss://{remote_address}")
    };

    crate::adapter::validate_url(&remote_address)?;
    let mut request = remote_address.into_client_request()?;
    request
        .headers_mut()
        .append("Sec-WebSocket-Protocol", HeaderValue::from_static("rfc5"));

    let mut stream = platform::websocket(request).await?;
    debug!("<< stream connected");

    // send peer identification
    let ident = WsPacket {
        message: Some(ws_packet::Message::PeerIdentification(WsIdentification {
            address: format!(
                "{:#x}",
                wallet.address().ok_or(anyhow!("wallet not connected"))?
            ),
        })),
    };
    stream.send(ident.encode_to_vec().into()).await?;
    debug!(">> ident sent: {ident:?}");

    // challenge / welcome
    let from_alias;
    let mut foreign_aliases;
    loop {
        let Some(response) = stream.next().await else {
            bail!("stream closed unexpectedly awaiting challenge")
        };
        let response = response?;
        let response = WsPacket::decode(response.into_data().as_slice())?;
        let Some(message) = response.message else {
            bail!("received empty packet")
        };

        match message {
            ws_packet::Message::WelcomeMessage(welcome) => {
                health.connected();
                debug!("websocket welcome received");
                from_alias = welcome.alias;
                foreign_aliases = BiMap::from_iter(welcome.peer_identities.into_iter().flat_map(
                    |(alias, address)| {
                        if let Some(h160) = address.as_h160() {
                            Some((alias, h160))
                        } else {
                            warn!("failed to parse hash: {}", address);
                            None
                        }
                    },
                ));
                break;
            }
            ws_packet::Message::ChallengeMessage(WsChallengeRequired {
                challenge_to_sign, ..
            }) => {
                // send challenge response
                debug!("websocket challenge received");

                if !challenge_to_sign.starts_with("dcl-") {
                    error!("invalid challenge to sign");
                    return Err(anyhow!("invalid challenge to sign"));
                }

                // sign challenge
                let chain = wallet.sign_message(challenge_to_sign).await?;
                let auth_chain_json = serde_json::to_string(&chain)?;
                debug!("websocket auth chain created");

                // send response
                let message = WsPacket {
                    message: Some(ws_packet::Message::SignedChallengeForServer(
                        WsSignedChallenge { auth_chain_json },
                    )),
                };
                let message = message.encode_to_vec();
                stream.send(message.into()).await?;
                debug!(">> auth chain sent");
            }
            _ => bail!("unexpected message during handshake: {message:?}"),
        }
    }
    dcl_assert!(from_alias != u32::MAX);

    // Register everyone the welcome listed. Membership is otherwise implied by a `PlayerUpdate`
    // arriving, so a peer who joined before us and then says nothing would go unrecorded.
    // collected first: the `BiMap` iterator isn't `Send`, so it can't be held across the await
    let joined: Vec<_> = foreign_aliases.right_values().copied().collect();
    for address in joined {
        sender
            .send(
                PlayerUpdate {
                    transport_id,
                    message: PlayerMessage::Joined,
                    address,
                }
                .into(),
            )
            .await
            .map_err(|_| anyhow!("Send error"))?;
    }

    let (mut write, mut read) = stream.split();

    // wrap and transmit outbound messages
    let f_write = async move {
        while let Some(next) = receiver.recv().await {
            let Some(body) = next.message.to_rfc4() else {
                continue;
            };
            let packet = WsPacket {
                message: Some(ws_packet::Message::PeerUpdateMessage(WsPeerUpdate {
                    from_alias,
                    body,
                    unreliable: next.unreliable,
                    recipients: Vec::new(),
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
            let next = WsPacket::decode(next.into_data().as_slice())?;
            let Some(message) = next.message else {
                bail!("received empty packet")
            };

            match message {
                ws_packet::Message::ChallengeMessage(_)
                | ws_packet::Message::PeerIdentification(_)
                | ws_packet::Message::SignedChallengeForServer(_)
                | ws_packet::Message::WelcomeMessage(_) => {
                    warn!("unexpected bau message: {message:?}");
                    continue;
                }
                ws_packet::Message::PeerJoinMessage(peer) => {
                    debug!("peer joined: {} -> {}", peer.alias, peer.address);
                    if let Some(h160) = peer.address.as_h160() {
                        foreign_aliases.insert(peer.alias, h160);
                        // presence, without waiting for them to send anything
                        sender
                            .send(
                                PlayerUpdate {
                                    transport_id,
                                    message: PlayerMessage::Joined,
                                    address: h160,
                                }
                                .into(),
                            )
                            .await
                            .map_err(|_| anyhow!("Send error"))?;
                    } else {
                        warn!("failed to parse hash: {}", peer.address);
                    }
                }
                ws_packet::Message::PeerLeaveMessage(peer) => {
                    debug!(
                        "peer left: {} -> {:?}",
                        peer.alias,
                        foreign_aliases.get_by_left(&peer.alias)
                    );
                    if let Some((_, address)) = foreign_aliases.remove_by_left(&peer.alias) {
                        sender
                            .send(NetworkUpdate::PlayerLeft {
                                transport_id,
                                address,
                            })
                            .await
                            .map_err(|_| anyhow!("Send error"))?;
                    }
                }
                ws_packet::Message::PeerUpdateMessage(update) => {
                    let packet = match rfc4::Packet::decode(update.body.as_slice()) {
                        Ok(packet) => packet,
                        Err(e) => {
                            warn!("unable to parse packet body: {e}");
                            continue;
                        }
                    };
                    let Some(message) = packet.message else {
                        warn!("received empty packet body");
                        continue;
                    };

                    let Some(address) = foreign_aliases.get_by_left(&update.from_alias).cloned()
                    else {
                        debug!("received packet for unknown alias {}", update.from_alias);
                        continue;
                    };

                    debug!(
                        "[tid: {:?}] received message {:?} from {:?}",
                        transport_id, message, address
                    );
                    sender
                        .send(
                            PlayerUpdate {
                                transport_id,
                                message: PlayerMessage::PlayerData(message),
                                address,
                            }
                            .into(),
                        )
                        .await
                        .map_err(|_| anyhow!("Send error"))?;
                }
                ws_packet::Message::PeerKicked(_) => {
                    return Err(AdmissionError::new(FailureKind::Denied).into());
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn direct_websocket_reconnect_preserves_backoff_until_stable() {
        let mut app = App::new();
        app.init_resource::<Time>()
            .add_systems(Update, reconnect_websocket)
            .add_systems(
                PostUpdate,
                crate::room_connection::observe_stable_connections,
            );
        let health = ConnectionHealth::default();
        health.connected();
        let mut recovery = RoomRecovery::default();
        recovery.fail(0.0, AdmissionError::new(FailureKind::Transient));
        let task = IoTaskPool::get_or_init(bevy::tasks::TaskPool::new)
            .spawn(futures_lite::future::pending());
        let room = app
            .world_mut()
            .spawn((
                WebsocketRoomTransport {
                    address: "wss://rooms.example/direct".into(),
                    receiver: None,
                },
                WebSocketConnection(task),
                health,
                recovery,
            ))
            .id();
        app.update();
        assert_eq!(app.world().get::<RoomRecovery>(room).unwrap().failure, None);
        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(std::time::Duration::from_secs(10));
        app.update();
        {
            let mut recovery = app.world_mut().get_mut::<RoomRecovery>(room).unwrap();
            recovery.fail(10.0, AdmissionError::new(FailureKind::Transient));
            assert_eq!(recovery.retry.due, 12.0);
        }
        app.update();
        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(std::time::Duration::from_secs(30));
        app.update();
        let mut recovery = app.world_mut().get_mut::<RoomRecovery>(room).unwrap();
        recovery.fail(40.0, AdmissionError::new(FailureKind::Transient));
        assert_eq!(recovery.retry.due, 41.0);
    }
}
