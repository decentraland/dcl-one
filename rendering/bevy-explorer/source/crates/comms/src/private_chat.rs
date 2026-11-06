//! The private chat room: one LiveKit room per environment that every DM-capable client joins
//! at login and stays in across realm changes. DMs are addressed by data-packet topic and
//! destination identity on it; its participant list is the DM presence list. Community chat
//! rides the same room (see [`crate::community_chat`]): a community's channel is its topic, and
//! the packets go through the server's message router instead of to a peer.
//!
//! The room is deliberately not a [`Transport`](crate::Transport): nothing broadcast to the
//! realm's transports can reach it, and its participants never touch the avatar store.

use std::collections::HashSet;

use alloy_core::primitives::Address;
use bevy::{
    ecs::relationship::Relationship,
    prelude::*,
    tasks::{IoTaskPool, Task},
};
use bevy_console::ConsoleCommand;
use common::{
    base_domain::Service,
    structs::{AppConfig, DmPrivacy, PrimaryUser},
    util::{AsH160, TaskCompat, TaskExt},
};
use console::{AddDebugSection, DebugInput, DebugInputKind, DebugSection, DoAddConsoleCommand};
use dcl_component::proto_components::kernel::comms::rfc4;
use http::Uri;
use ipfs::IpfsAssetServer;
use prost::Message as _;
use serde::{Deserialize, Serialize};
use serde_json::json;
use system_bridge::SystemApi;
use tokio::sync::mpsc::Sender;
use wallet::Wallet;

use crate::{
    chat_reaction::{
        canonical_message_id, emoji_to_atlas_index, encode_wire_index, received_emoji,
        ChatReactionEvent,
    },
    community_chat::{
        channel_for, community_id, message_router_identity, resolve_author, PrivateChatTarget,
        CHANNEL_PREFIX,
    },
    global_crdt::{ChannelControl, ChatEvent},
    livekit::{
        participant::{
            plugin::InboundRateLimiter, HostedBy, HostingParticipants, LivekitParticipant,
            Local as LocalParticipant, ParticipantIndex, ParticipantMetadataChanged,
            ParticipantPayload,
        },
        room::{Connected, Connecting, Disconnected},
        standalone_room_components, LivekitTransport,
    },
    mint_gatekeeper_adapter, DisableRealmComms, DisableSceneRoomGatekeeper, NetworkMessage,
    NetworkMessageRecipient,
};

/// Reactions with a longer message id are dropped by the receiving client.
const MAX_REACTION_MESSAGE_ID_LEN: usize = 96;

pub struct PrivateChatPlugin;

impl Plugin for PrivateChatPlugin {
    fn build(&self, app: &mut App) {
        app.add_event::<PrivateChatReceived>();
        app.add_event::<PrivateChatPresenceChanged>();
        app.add_event::<PrivateChatRouterChanged>();
        app.add_event::<CommunityChatReceived>();
        app.init_resource::<CommunityChannels>();
        app.add_systems(
            Update,
            (
                connect_private_chat_room,
                apply_community_channels,
                send_private_chats,
                send_private_reactions,
            ),
        );
        app.add_observer(participant_added);
        app.add_observer(participant_removed);
        app.add_observer(participant_metadata_changed);
        app.add_observer(participant_payload);
        app.add_console_command::<DmRoomCommand, _>(dm_room);
        app.add_console_command::<DmCommand, _>(dm);
        app.add_console_command::<CommunityChatCommand, _>(community_chat);
        app.add_console_command::<CommunityChannelsCommand, _>(community_channels);
        app.add_console_command::<CommunityStateCommand, _>(community_state);
        app.add_debug_section(DebugSection {
            title: "Community chat",
            query: "/community_state",
            fields: &[
                ("Router identity", "routerIdentity"),
                ("Router in room", "routerInRoom"),
                ("Channels joined", "channels"),
                ("Messages sent", "sent"),
                ("Messages received", "received"),
                ("Reactions sent", "reactionsSent"),
                ("Reactions received", "reactionsReceived"),
                ("Dropped: not a member", "dropped.notMember"),
                ("Dropped: no author", "dropped.unattributed"),
                ("Dropped: rate limited", "dropped.rateLimited"),
                ("Last error", "lastError"),
            ],
            actions: &[],
            inputs: &[DebugInput {
                label: "Add channel (community id)",
                field: "",
                command: "/community_channels add {value}",
                kind: DebugInputKind::Text,
            }],
        });
    }
}

/// Marks the private chat room entity and holds its outgoing channel.
#[derive(Component)]
pub struct PrivateChatRoom {
    pub sender: Sender<NetworkMessage>,
    // keeps the room's control channel open; the room has no voice, so nothing is ever sent
    _control: Sender<ChannelControl>,
}

/// Community chat counters on the private chat room, for `/community_state`. No message bodies.
#[derive(Component, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommunityChatStats {
    sent: u64,
    received: u64,
    reactions_sent: u64,
    reactions_received: u64,
    dropped: DroppedCommunityPackets,
    last_error: Option<String>,
}

#[derive(Default, Serialize)]
#[serde(rename_all = "camelCase")]
struct DroppedCommunityPackets {
    /// On a community channel the local user is not in (or has closed).
    not_member: u64,
    /// From the router without a wallet in the stamped author, or not from the router at all.
    unattributed: u64,
    rate_limited: u64,
}

/// The community channels the local user listens to: the communities they are a member of,
/// minus those whose chat they closed, as lowercased ids. Pushed by the HUD
/// (`SystemApi::SetCommunityChannels`); packets on any other community channel are dropped
/// before they are recorded. Cleared with the identity.
#[derive(Resource, Default)]
pub struct CommunityChannels(pub HashSet<String>);

impl CommunityChannels {
    pub fn contains(&self, community_id: &str) -> bool {
        self.0.contains(&community_id.to_ascii_lowercase())
    }
}

/// On a private chat room participant: the DM privacy the server stamped into their participant
/// metadata, `None` until it arrives.
#[derive(Component, Debug)]
pub struct PrivateChatPrivacy(pub Option<DmPrivacy>);

/// A wallet joined or left the private chat room, or its privacy changed.
#[derive(Event, Debug)]
pub struct PrivateChatPresenceChanged {
    pub address: Address,
}

/// The server's message router joined or left the private chat room: community chat can or
/// cannot go anywhere.
#[derive(Event, Debug)]
pub struct PrivateChatRouterChanged;

/// Whether the server's message router is in the private chat room whose participants `index` lists.
pub fn router_in_room(index: &ParticipantIndex) -> bool {
    index.contains_key(&ParticipantIndex::key(&message_router_identity()))
}

/// A DM addressed to the local user, after the topic and rate checks.
#[derive(Event, Debug)]
pub struct PrivateChatReceived {
    pub from: Address,
    pub message: String,
    /// The sender's rfc4 timestamp, which names the message for reactions.
    pub timestamp: f64,
}

/// A community message relayed by the router on a channel the local user listens to, after the
/// author and rate checks.
#[derive(Event, Debug)]
pub struct CommunityChatReceived {
    /// Lowercased.
    pub community_id: String,
    pub from: Address,
    pub message: String,
    /// The author's rfc4 timestamp, which names the message for reactions.
    pub timestamp: f64,
}

#[derive(Deserialize)]
struct ParticipantMetadata {
    private_messages_privacy: String,
}

fn privacy_from_metadata(metadata: &str) -> Option<DmPrivacy> {
    let metadata = serde_json::from_str::<ParticipantMetadata>(metadata).ok()?;
    match metadata.private_messages_privacy.as_str() {
        "all" => Some(DmPrivacy::All),
        "only_friends" => Some(DmPrivacy::OnlyFriends),
        _ => None,
    }
}

/// Gatekeeper retry backoff: 5s doubling to 60s.
fn gatekeeper_retry_delay(attempt: u32) -> f64 {
    (5.0 * 2f64.powi(attempt.min(4) as i32)).min(60.0)
}

#[allow(clippy::too_many_arguments)]
fn connect_private_chat_room(
    mut commands: Commands,
    wallet: Res<Wallet>,
    config: Res<AppConfig>,
    ipfs: IpfsAssetServer,
    time: Res<Time>,
    rooms: Query<Entity, With<PrivateChatRoom>>,
    mut channels: ResMut<CommunityChannels>,
    mut gatekeeper_task: Local<Option<Task<Result<String, anyhow::Error>>>>,
    mut retry: Local<Option<(f64, u32)>>,
    disable_realm_comms: Option<Res<DisableRealmComms>>,
    disable_gatekeeper: Res<DisableSceneRoomGatekeeper>,
) {
    // headless servers never join (they would be a ghost participant), and orchestrated servers
    // never sign gatekeeper handshakes themselves
    if disable_realm_comms.is_some_and(|d| d.0) || disable_gatekeeper.0 {
        return;
    }

    let now = time.elapsed_secs_f64();
    let mint = if wallet.is_changed() {
        for room in rooms.iter() {
            commands.entity(room).despawn();
        }
        *gatekeeper_task = None;
        *retry = None;
        channels.0.clear();

        if wallet.address().is_none() {
            info!("private chat: no identity, not connecting");
            return;
        }
        true
    } else {
        gatekeeper_task.is_none() && retry.is_some_and(|(due, _)| now >= due)
    };

    if mint {
        let uri = Uri::try_from(common::base_domain::url(
            Service::CommsGatekeeper,
            "/private-messages/token",
        ))
        .unwrap();
        let meta = serde_json::json!({
            "signer": "dcl:explorer",
            "deviceIdentifier": config.user_id,
        })
        .to_string();
        let client = ipfs.ipfs().client();
        let wallet = wallet.clone();
        *gatekeeper_task = Some(IoTaskPool::get().spawn_compat(async move {
            mint_gatekeeper_adapter("GET", client, uri, &wallet, meta).await
        }));
    }

    if let Some(mut task) = gatekeeper_task.take() {
        match task.complete() {
            None => *gatekeeper_task = Some(task),
            Some(Err(e)) => {
                let attempt = retry.map_or(0, |(_, attempt)| attempt);
                let delay = gatekeeper_retry_delay(attempt);
                warn!(
                    "private chat: failed to get room from gatekeeper: {e}; retrying in {delay}s"
                );
                *retry = Some((now + delay, attempt + 1));
            }
            Some(Ok(adapter)) => {
                *retry = None;
                let Some(("livekit", address)) = adapter.split_once(':') else {
                    warn!("private chat: unsupported adapter `{adapter}`");
                    return;
                };
                let (components, sender, control) = standalone_room_components(address.to_owned());
                commands.spawn((
                    components,
                    PrivateChatRoom {
                        sender,
                        _control: control,
                    },
                    CommunityChatStats::default(),
                ));
                info!("private chat: connecting");
            }
        }
    }
}

/// The HUD's community channel set replaces the current one.
fn apply_community_channels(
    mut requests: EventReader<SystemApi>,
    mut channels: ResMut<CommunityChannels>,
) {
    for request in requests.read() {
        let SystemApi::SetCommunityChannels(ids) = request else {
            continue;
        };
        let mut next = HashSet::new();
        for id in ids {
            match community_id(&channel_for(id)) {
                Some(id) => {
                    next.insert(id.to_owned());
                }
                None => warn!(target: "comms::private_chat", "ignoring community channel `{id}`"),
            }
        }
        debug!(target: "comms::private_chat", "community channels: {next:?}");
        channels.0 = next;
    }
}

/// Stamps a private chat room participant with the privacy from their metadata.
fn participant_added(
    trigger: Trigger<OnAdd, LivekitParticipant>,
    mut commands: Commands,
    participants: Query<(&LivekitParticipant, &HostedBy)>,
    rooms: Query<(), With<PrivateChatRoom>>,
    mut presence: EventWriter<PrivateChatPresenceChanged>,
    mut router: EventWriter<PrivateChatRouterChanged>,
) {
    let Ok((participant, hosted_by)) = participants.get(trigger.target()) else {
        return;
    };
    if !rooms.contains(hosted_by.get()) {
        return;
    }
    let privacy = privacy_from_metadata(&participant.metadata());
    debug!(target: "comms::private_chat", "{} joined, privacy {privacy:?}", participant.identity());
    commands
        .entity(trigger.target())
        .try_insert(PrivateChatPrivacy(privacy));
    if let Some(address) = participant.identity().as_str().as_h160() {
        presence.write(PrivateChatPresenceChanged { address });
    } else if participant.identity().as_str() == message_router_identity() {
        router.write(PrivateChatRouterChanged);
    }
}

fn participant_removed(
    trigger: Trigger<OnRemove, PrivateChatPrivacy>,
    participants: Query<&LivekitParticipant>,
    mut presence: EventWriter<PrivateChatPresenceChanged>,
    mut router: EventWriter<PrivateChatRouterChanged>,
) {
    let Ok(participant) = participants.get(trigger.target()) else {
        return;
    };
    if let Some(address) = participant.identity().as_str().as_h160() {
        presence.write(PrivateChatPresenceChanged { address });
    } else if participant.identity().as_str() == message_router_identity() {
        router.write(PrivateChatRouterChanged);
    }
}

fn participant_metadata_changed(
    trigger: Trigger<ParticipantMetadataChanged>,
    rooms: Query<&ParticipantIndex, With<PrivateChatRoom>>,
    mut participants: Query<&mut PrivateChatPrivacy>,
    mut presence: EventWriter<PrivateChatPresenceChanged>,
) {
    let ParticipantMetadataChanged { participant, room } = trigger.event();
    let Ok(index) = rooms.get(*room) else {
        return;
    };
    let Some(entity) = index
        .get(&ParticipantIndex::key(participant.identity().as_str()))
        .copied()
    else {
        return;
    };
    let Ok(mut privacy) = participants.get_mut(entity) else {
        return;
    };
    privacy.0 = privacy_from_metadata(&participant.metadata());
    debug!(target: "comms::private_chat", "{} privacy {:?}", participant.identity(), privacy.0);
    if let Some(address) = participant.identity().as_str().as_h160() {
        presence.write(PrivateChatPresenceChanged { address });
    }
}

/// A packet on the private chat room: a DM or a reaction to one, addressed to us by topic and
/// sent by the partner themselves; or a community message or reaction, on a community channel's
/// topic, relayed by the router with the author stamped in. Anything else is dropped.
#[allow(clippy::too_many_arguments)]
fn participant_payload(
    trigger: Trigger<ParticipantPayload>,
    mut rooms: Query<&mut CommunityChatStats, With<PrivateChatRoom>>,
    channels: Res<CommunityChannels>,
    wallet: Res<Wallet>,
    time: Res<Time>,
    mut rate_limiter: ResMut<InboundRateLimiter>,
    mut received: EventWriter<PrivateChatReceived>,
    mut community_received: EventWriter<CommunityChatReceived>,
    mut reactions: EventWriter<ChatReactionEvent>,
) {
    let ParticipantPayload {
        room,
        identity,
        payload,
        topic,
    } = trigger.event();
    let Ok(mut stats) = rooms.get_mut(*room) else {
        return;
    };
    let identity = identity.to_string();
    let now = time.elapsed_secs_f64();
    let router = message_router_identity();
    // the router relays every community's traffic, so it is limited per stamped author below,
    // not as one sender
    let from_router = identity.eq_ignore_ascii_case(&router);
    if !from_router && !rate_limiter.allow(*room, &identity, now) {
        stats.dropped.rate_limited += 1;
        return;
    }
    let Some(me) = wallet.address() else {
        return;
    };
    let message = match rfc4::Packet::decode(payload.as_slice()) {
        Ok(rfc4::Packet {
            message: Some(message),
            ..
        }) => message,
        Ok(_) => {
            debug!(target: "comms::private_chat", "empty packet from {identity}");
            return;
        }
        Err(e) => {
            warn!(target: "comms::private_chat", "undecodable packet from {identity}: {e}");
            return;
        }
    };

    if let Some(id) = topic.as_deref().and_then(community_id) {
        let id = id.to_ascii_lowercase();
        match message {
            rfc4::packet::Message::Chat(chat) => {
                let Some(from) = resolve_author(&identity, &router, chat.forwarded_from.as_deref())
                else {
                    stats.dropped.unattributed += 1;
                    debug!(target: "comms::private_chat", "dropping community message from {identity} ({:?})", chat.forwarded_from);
                    return;
                };
                // the local user's own line is recorded as it is sent; an echo of it would be
                // shown, stored and pinged again
                if from == me {
                    debug!(target: "comms::private_chat", "dropping an echo of the local user's community message");
                    return;
                }
                if !channels.contains(&id) {
                    stats.dropped.not_member += 1;
                    debug!(target: "comms::private_chat", "dropping message on community {id}, not listening");
                    return;
                }
                if !rate_limiter.allow(*room, &format!("{CHANNEL_PREFIX}{from:#x}"), now) {
                    stats.dropped.rate_limited += 1;
                    return;
                }
                if !chat.timestamp.is_finite() {
                    warn!(target: "comms::private_chat", "dropping community message from {from:#x} with timestamp {}", chat.timestamp);
                    return;
                }
                debug!(target: "comms::private_chat", "community {id} message from {from:#x}: {}", chat.message);
                stats.received += 1;
                community_received.write(CommunityChatReceived {
                    community_id: id,
                    from,
                    message: chat.message,
                    timestamp: chat.timestamp,
                });
            }
            rfc4::packet::Message::ChatReaction(reaction) => {
                let Some(from) = resolve_author(&identity, &router, Some(&reaction.address)) else {
                    stats.dropped.unattributed += 1;
                    debug!(target: "comms::private_chat", "dropping community reaction from {identity} ({})", reaction.address);
                    return;
                };
                // the local user's reactions are sent from the event stream this writes to, so
                // an echo of one would be sent again
                if from == me {
                    debug!(target: "comms::private_chat", "dropping an echo of the local user's community reaction");
                    return;
                }
                if !channels.contains(&id) {
                    stats.dropped.not_member += 1;
                    return;
                }
                if !rate_limiter.allow(*room, &format!("{CHANNEL_PREFIX}{from:#x}"), now) {
                    stats.dropped.rate_limited += 1;
                    return;
                }
                if reaction.message_id.is_empty()
                    || reaction.message_id.len() > MAX_REACTION_MESSAGE_ID_LEN
                {
                    return;
                }
                debug!(target: "comms::private_chat", "community {id} reaction from {from:#x}: {reaction:?}");
                if let (Some(message_id), Some((emoji, remove))) = (
                    canonical_message_id(&reaction.message_id),
                    received_emoji(&reaction.emoji, reaction.emoji_index, reaction.remove),
                ) {
                    stats.reactions_received += 1;
                    reactions.write(ChatReactionEvent {
                        channel: channel_for(&id),
                        message_id,
                        emoji,
                        from,
                        remove,
                    });
                }
            }
            other => {
                debug!(target: "comms::private_chat", "ignoring non-chat packet on community {id} from {identity}: {other:?}");
            }
        }
        return;
    }

    // a DM: the topic must name us, and the sender is the LiveKit-authenticated partner, so a
    // packet addressed elsewhere (or to nobody), or relayed, is dropped
    let Some(from) = identity.as_h160() else {
        stats.dropped.unattributed += 1;
        debug!(target: "comms::private_chat", "dropping packet from {identity} with topic {topic:?}");
        return;
    };
    if topic
        .as_deref()
        .is_none_or(|topic| topic.as_h160() != Some(me))
    {
        warn!(target: "comms::private_chat", "dropping packet from {from:#x} with topic {topic:?}");
        return;
    }
    let message = match message {
        rfc4::packet::Message::Chat(chat) => chat,
        rfc4::packet::Message::ChatReaction(reaction) => {
            debug!(target: "comms::private_chat", "dm reaction from {from:#x}: {reaction:?}");
            if let (Some(message_id), Some((emoji, remove))) = (
                canonical_message_id(&reaction.message_id),
                received_emoji(&reaction.emoji, reaction.emoji_index, reaction.remove),
            ) {
                reactions.write(ChatReactionEvent {
                    channel: format!("{from:#x}"),
                    message_id,
                    emoji,
                    from,
                    remove,
                });
            }
            return;
        }
        other => {
            debug!(target: "comms::private_chat", "ignoring non-chat packet from {from:#x}: {other:?}");
            return;
        }
    };
    if !message.timestamp.is_finite() {
        warn!(target: "comms::private_chat", "dropping dm from {from:#x} with timestamp {}", message.timestamp);
        return;
    }
    debug!(target: "comms::private_chat", "dm from {from:#x}: {}", message.message);
    received.write(PrivateChatReceived {
        from,
        message: message.message,
        timestamp: message.timestamp,
    });
}

fn route(target: &PrivateChatTarget) -> (String, NetworkMessageRecipient) {
    match target {
        PrivateChatTarget::Dm(to) => (format!("{to:#x}"), NetworkMessageRecipient::Peer(*to)),
        PrivateChatTarget::Community(id) => {
            (channel_for(id), NetworkMessageRecipient::MessageRouter)
        }
    }
}

/// The local player's chat events whose channel is a wallet address go out as DMs on the private
/// chat room, addressed to that wallet by topic and destination identity; those on a community
/// channel go to the router on the community's topic.
fn send_private_chats(
    mut chats: EventReader<ChatEvent>,
    player: Query<Entity, With<PrimaryUser>>,
    mut room: Query<(&PrivateChatRoom, &mut CommunityChatStats)>,
    channels: Res<CommunityChannels>,
) {
    let Ok(player) = player.single() else {
        return;
    };
    for ev in chats.read().filter(|ev| ev.sender == player) {
        let Some(target) = PrivateChatTarget::parse(&ev.channel) else {
            continue;
        };
        let Ok((room, mut stats)) = room.single_mut() else {
            warn!(target: "comms::private_chat", "no private chat room, dropping message to {target:?}");
            continue;
        };
        if let PrivateChatTarget::Community(id) = &target {
            if !channels.contains(id) {
                warn!(target: "comms::private_chat", "sending to community {id}, which is not a listened channel");
            }
        }
        let packet = rfc4::Packet {
            message: Some(rfc4::packet::Message::Chat(rfc4::Chat {
                message: ev.message.clone(),
                timestamp: ev.timestamp,
                // Set by an SFU that forwards the message, never by the sender.
                forwarded_from: None,
            })),
            protocol_version: 100,
        };
        let (topic, recipient) = route(&target);
        let message = NetworkMessage {
            topic: Some(topic),
            ..NetworkMessage::targetted_reliable(&packet, recipient)
        };
        match room.sender.try_send(message) {
            Ok(()) => {
                debug!(target: "comms::private_chat", "to {target:?}: {}", ev.message);
                if matches!(target, PrivateChatTarget::Community(_)) {
                    stats.sent += 1;
                }
            }
            Err(e) => {
                warn!(target: "comms::private_chat", "failed to queue message to {target:?}: {e}");
                if matches!(target, PrivateChatTarget::Community(_)) {
                    stats.last_error = Some(format!("queue: {e}"));
                }
            }
        }
    }
}

/// The local user's reactions to DMs go to the partner the same way the DMs do, and those to
/// community messages to the router.
fn send_private_reactions(
    mut reactions: EventReader<ChatReactionEvent>,
    wallet: Res<Wallet>,
    mut room: Query<(&PrivateChatRoom, &mut CommunityChatStats)>,
) {
    let Some(me) = wallet.address() else {
        reactions.clear();
        return;
    };
    for ev in reactions.read().filter(|ev| ev.from == me) {
        let Some(target) = PrivateChatTarget::parse(&ev.channel) else {
            continue;
        };
        let Ok((room, mut stats)) = room.single_mut() else {
            warn!(target: "comms::private_chat", "no private chat room, dropping reaction to {target:?}");
            continue;
        };
        let packet = rfc4::Packet {
            message: Some(rfc4::packet::Message::ChatReaction(rfc4::ChatReaction {
                emoji_index: encode_wire_index(emoji_to_atlas_index(&ev.emoji), ev.remove),
                message_id: ev.message_id.clone(),
                address: format!("{me:#x}"),
                emoji: ev.emoji.clone(),
                remove: ev.remove,
            })),
            protocol_version: 100,
        };
        let (topic, recipient) = route(&target);
        let message = NetworkMessage {
            topic: Some(topic),
            ..NetworkMessage::targetted_reliable(&packet, recipient)
        };
        match room.sender.try_send(message) {
            Ok(()) => {
                if matches!(target, PrivateChatTarget::Community(_)) {
                    stats.reactions_sent += 1;
                }
            }
            Err(e) => {
                warn!(target: "comms::private_chat", "failed to queue reaction to {target:?}: {e}");
            }
        }
    }
}

/// `/dm <address> <message>` sends a DM, the same way the HUD does.
#[derive(clap::Parser, ConsoleCommand)]
#[command(name = "/dm")]
struct DmCommand {
    address: String,
    #[arg(trailing_var_arg = true, required = true)]
    message: Vec<String>,
}

fn dm(mut input: ConsoleCommand<DmCommand>, mut events: EventWriter<SystemApi>) {
    let Some(Ok(command)) = input.take() else {
        return;
    };
    if command.address.as_h160().is_none() {
        input.reply_failed(format!("`{}` is not a wallet address", command.address));
        return;
    }
    let message = command.message.join(" ");
    events.write(SystemApi::SendChat(
        message.clone(),
        command.address.clone(),
    ));
    input.reply_ok(format!("dm to {}: {message}", command.address));
}

/// `/community_chat <community id> <message>` sends a community message, the same way the HUD
/// does. The HUD must have listed the community (`/community_channels`) for replies to arrive.
#[derive(clap::Parser, ConsoleCommand)]
#[command(name = "/community_chat")]
struct CommunityChatCommand {
    community_id: String,
    #[arg(trailing_var_arg = true, required = true)]
    message: Vec<String>,
}

fn community_chat(
    mut input: ConsoleCommand<CommunityChatCommand>,
    mut events: EventWriter<SystemApi>,
) {
    let Some(Ok(command)) = input.take() else {
        return;
    };
    let channel = channel_for(&command.community_id);
    if community_id(&channel).is_none() {
        input.reply_failed(format!("`{}` is not a community id", command.community_id));
        return;
    }
    let message = command.message.join(" ");
    events.write(SystemApi::SendChat(message.clone(), channel.clone()));
    input.reply_ok(format!("[{channel}] {message}"));
}

/// `/community_channels [add <id> | remove <id> | clear]` lists or edits the community channels
/// listened to, as the HUD sets them.
#[derive(clap::Parser, ConsoleCommand)]
#[command(name = "/community_channels")]
struct CommunityChannelsCommand {
    action: Option<String>,
    community_id: Option<String>,
}

fn community_channels(
    mut input: ConsoleCommand<CommunityChannelsCommand>,
    mut channels: ResMut<CommunityChannels>,
) {
    let Some(Ok(command)) = input.take() else {
        return;
    };
    let id = command
        .community_id
        .as_deref()
        .map(|id| (channel_for(id), id));
    match (command.action.as_deref(), id) {
        (None, _) => (),
        (Some("clear"), _) => channels.0.clear(),
        (Some(action @ ("add" | "remove")), Some((channel, id))) => {
            let Some(valid) = community_id(&channel) else {
                input.reply_failed(format!("`{id}` is not a community id"));
                return;
            };
            if action == "add" {
                channels.0.insert(valid.to_owned());
            } else {
                channels.0.remove(valid);
            }
        }
        (Some(action), _) => {
            input.reply_failed(format!(
                "`{action}` is not `add <id>`, `remove <id>` or `clear`"
            ));
            return;
        }
    }
    let mut listed: Vec<_> = channels.0.iter().cloned().collect();
    listed.sort();
    input.reply_ok(format!("community channels: {listed:?}"));
}

/// `/community_state` prints community chat's routing and counters as JSON.
#[derive(clap::Parser, ConsoleCommand)]
#[command(name = "/community_state")]
struct CommunityStateCommand;

fn community_state(
    mut input: ConsoleCommand<CommunityStateCommand>,
    rooms: Query<(&ParticipantIndex, &CommunityChatStats), With<PrivateChatRoom>>,
    channels: Res<CommunityChannels>,
) {
    if input.take().is_none() {
        return;
    }
    let router = message_router_identity();
    let room = rooms.single().ok();
    let router_in_room = room.is_some_and(|(index, _)| router_in_room(index));
    let mut listed: Vec<_> = channels.0.iter().cloned().collect();
    listed.sort();
    let mut state = json!({
        "routerIdentity": router,
        "routerInRoom": router_in_room,
        "channels": listed,
        "channelCount": channels.0.len(),
        "roomSpawned": room.is_some(),
    });
    if let Some((_, stats)) = room {
        if let (serde_json::Value::Object(state), Ok(serde_json::Value::Object(stats))) =
            (&mut state, serde_json::to_value(stats))
        {
            state.extend(stats);
        }
    }
    input.reply_ok(state.to_string());
}

/// `/dm_room` prints the private chat room's connection state and who is in it.
#[derive(clap::Parser, ConsoleCommand)]
#[command(name = "/dm_room")]
struct DmRoomCommand;

#[allow(clippy::type_complexity)]
fn dm_room(
    mut input: ConsoleCommand<DmRoomCommand>,
    rooms: Query<
        (
            &LivekitTransport,
            Has<Connecting>,
            Has<Connected>,
            Has<Disconnected>,
            Option<&HostingParticipants>,
        ),
        With<PrivateChatRoom>,
    >,
    participants: Query<
        (&LivekitParticipant, Option<&PrivateChatPrivacy>),
        Without<LocalParticipant>,
    >,
    wallet: Res<Wallet>,
) {
    const LISTED: usize = 20;

    if input.take().is_none() {
        return;
    }
    let Ok((transport, connecting, connected, disconnected, hosting)) = rooms.single() else {
        input.reply_ok("private chat room: not spawned");
        return;
    };
    let state = match (connecting, connected, disconnected) {
        (_, true, _) => "connected",
        (true, _, _) => "connecting",
        (_, _, true) => "disconnected",
        _ => "pending",
    };
    let host = transport.address.split('?').next().unwrap_or_default();
    let identity = wallet
        .address()
        .map(|a| format!("{a:#x}"))
        .unwrap_or_else(|| "none".to_owned());
    let remote: Vec<_> = hosting
        .map(|hosting| participants.iter_many(hosting.collection()).collect())
        .unwrap_or_default();
    let mut lines = vec![
        format!("private chat room: {state} ({host})"),
        format!("identity: {identity}"),
        format!("remote participants: {}", remote.len()),
    ];
    lines.extend(remote.iter().take(LISTED).map(|(participant, privacy)| {
        format!(
            "  {} {:?}",
            participant.identity(),
            privacy.map(|p| p.0).unwrap_or_default()
        )
    }));
    if remote.len() > LISTED {
        lines.push(format!("  ... and {} more", remote.len() - LISTED));
    }
    input.reply_ok(lines.join("\n"));
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use livekit::id::ParticipantIdentity;

    use super::*;

    const ALICE: &str = "0x1111111111111111111111111111111111111111";
    const BOB: &str = "0x2222222222222222222222222222222222222222";
    const COMMUNITY: &str = "abc-1";

    /// A world with the private chat room and `participant_payload` listening on it, the local
    /// user listening to `COMMUNITY`.
    fn world() -> (World, Entity) {
        let mut world = World::new();
        let mut wallet = Wallet::default();
        wallet.finalize_as_guest();
        world.insert_resource(wallet);
        world.init_resource::<Time>();
        world.init_resource::<InboundRateLimiter>();
        world.init_resource::<Events<PrivateChatReceived>>();
        world.init_resource::<Events<CommunityChatReceived>>();
        world.init_resource::<Events<ChatReactionEvent>>();
        let mut channels = CommunityChannels::default();
        channels.0.insert(COMMUNITY.to_owned());
        world.insert_resource(channels);
        let (sender, _) = tokio::sync::mpsc::channel(1);
        let (_control, _) = tokio::sync::mpsc::channel(1);
        let room = world
            .spawn((
                PrivateChatRoom { sender, _control },
                CommunityChatStats::default(),
            ))
            .id();
        world.add_observer(participant_payload);
        (world, room)
    }

    fn payload(from: &str, message: rfc4::packet::Message, topic: &str) -> ParticipantPayload {
        let packet = rfc4::Packet {
            message: Some(message),
            protocol_version: 100,
        };
        ParticipantPayload {
            room: Entity::PLACEHOLDER,
            identity: ParticipantIdentity(from.to_owned()),
            payload: Arc::new(packet.encode_to_vec()),
            topic: Some(topic.to_owned()),
        }
    }

    fn community_chat(from: &str, forwarded_from: Option<&str>) -> ParticipantPayload {
        community_chat_at(from, forwarded_from, 1.0)
    }

    fn community_chat_at(
        from: &str,
        forwarded_from: Option<&str>,
        timestamp: f64,
    ) -> ParticipantPayload {
        payload(
            from,
            rfc4::packet::Message::Chat(rfc4::Chat {
                message: "hello".to_owned(),
                timestamp,
                forwarded_from: forwarded_from.map(str::to_owned),
            }),
            &channel_for(COMMUNITY),
        )
    }

    fn community_reaction(from: &str, address: &str) -> ParticipantPayload {
        payload(
            from,
            rfc4::packet::Message::ChatReaction(rfc4::ChatReaction {
                emoji_index: 0,
                message_id: format!("{ALICE}:1"),
                address: address.to_owned(),
                emoji: "\u{1f44d}".to_owned(),
                remove: false,
            }),
            &channel_for(COMMUNITY),
        )
    }

    fn community_received(world: &World) -> usize {
        world
            .resource::<Events<CommunityChatReceived>>()
            .iter_current_update_events()
            .count()
    }

    fn reactions_received(world: &World) -> usize {
        world
            .resource::<Events<ChatReactionEvent>>()
            .iter_current_update_events()
            .count()
    }

    #[test]
    fn a_community_message_is_taken_from_the_router_only() {
        let (mut world, room) = world();
        let router = message_router_identity();

        // relayed by the router: received, attributed to the stamped author
        world.trigger(ParticipantPayload {
            room,
            ..community_chat(&router, Some(ALICE))
        });
        let received: Vec<_> = world
            .resource::<Events<CommunityChatReceived>>()
            .iter_current_update_events()
            .map(|ev| (ev.community_id.clone(), ev.from))
            .collect();
        assert_eq!(
            received,
            vec![(COMMUNITY.to_owned(), ALICE.as_h160().unwrap())]
        );

        // sent straight by a peer, with or without a stamp: dropped, not under their own name
        world.trigger(ParticipantPayload {
            room,
            ..community_chat(BOB, Some(ALICE))
        });
        world.trigger(ParticipantPayload {
            room,
            ..community_chat(BOB, None)
        });
        assert_eq!(community_received(&world), 1);
        let stats = world.get::<CommunityChatStats>(room).unwrap();
        assert_eq!(stats.received, 1);
        assert_eq!(stats.dropped.unattributed, 2);
        assert_eq!(stats.dropped.not_member, 0);
    }

    #[test]
    fn a_community_reaction_is_taken_from_the_router_with_a_wallet_in_it() {
        let (mut world, room) = world();
        let router = message_router_identity();
        world.trigger(ParticipantPayload {
            room,
            ..community_reaction(&router, ALICE)
        });
        assert_eq!(reactions_received(&world), 1);
        let from = world
            .resource::<Events<ChatReactionEvent>>()
            .iter_current_update_events()
            .next()
            .map(|ev| (ev.channel.clone(), ev.from))
            .unwrap();
        assert_eq!(from, (channel_for(COMMUNITY), ALICE.as_h160().unwrap()));
        // a peer's, and the router's without a wallet: dropped
        world.trigger(ParticipantPayload {
            room,
            ..community_reaction(BOB, ALICE)
        });
        world.trigger(ParticipantPayload {
            room,
            ..community_reaction(&router, "not a wallet")
        });
        assert_eq!(reactions_received(&world), 1);
        let stats = world.get::<CommunityChatStats>(room).unwrap();
        assert_eq!(stats.reactions_received, 1);
        assert_eq!(stats.dropped.unattributed, 2);
    }

    #[test]
    fn a_community_the_user_does_not_listen_to_is_dropped() {
        let (mut world, room) = world();
        let router = message_router_identity();
        let other = ParticipantPayload {
            room,
            topic: Some(channel_for("other-1")),
            ..community_chat(&router, Some(ALICE))
        };
        world.trigger(other);
        world.trigger(ParticipantPayload {
            room,
            topic: Some(channel_for("other-1")),
            ..community_reaction(&router, ALICE)
        });
        assert_eq!(community_received(&world), 0);
        assert_eq!(reactions_received(&world), 0);
        assert_eq!(
            world
                .get::<CommunityChatStats>(room)
                .unwrap()
                .dropped
                .not_member,
            2
        );
    }

    #[test]
    fn the_router_is_limited_per_author_not_as_one_sender() {
        let (mut world, room) = world();
        let router = message_router_identity();
        let limit = crate::livekit::participant::plugin::MAX_MESSAGES_PER_WINDOW;
        // one author over the window: their extra line goes, nobody else's does
        for i in 0..=limit {
            world.trigger(ParticipantPayload {
                room,
                ..community_chat_at(&router, Some(ALICE), i as f64)
            });
        }
        world.trigger(ParticipantPayload {
            room,
            ..community_chat(&router, Some(BOB))
        });
        assert_eq!(community_received(&world), limit + 1);
        let stats = world.get::<CommunityChatStats>(room).unwrap();
        assert_eq!(stats.dropped.rate_limited, 1);
    }

    #[test]
    fn an_echo_of_the_local_users_own_packet_is_dropped() {
        let (mut world, room) = world();
        let router = message_router_identity();
        let me = format!("{:#x}", world.resource::<Wallet>().address().unwrap());
        world.trigger(ParticipantPayload {
            room,
            ..community_chat(&router, Some(&me))
        });
        world.trigger(ParticipantPayload {
            room,
            ..community_reaction(&router, &me)
        });
        assert_eq!(community_received(&world), 0);
        assert_eq!(reactions_received(&world), 0);
        let stats = world.get::<CommunityChatStats>(room).unwrap();
        assert_eq!((stats.received, stats.reactions_received), (0, 0));
    }
}
