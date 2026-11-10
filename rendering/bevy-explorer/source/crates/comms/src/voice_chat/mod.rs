//! Private calls and community voice chat ("streams"): the state machine in [`state`], and here
//! its engine: the HUD's commands and state stream, the social service's procedures and update
//! streams, the one LiveKit voice room (joined with the url the service mints, left by despawning
//! it), its participants, the call microphone and the flat playback of the room's audio.
//!
//! The room is deliberately not a [`Transport`](crate::Transport): nothing broadcast to the realm's
//! transports reaches it, its participants never touch the avatar store, and its audio tracks are
//! subscribed here, not by the spatial path.

pub mod state;

use std::collections::VecDeque;

use alloy_core::primitives::Address;
use bevy::{
    ecs::{relationship::Relationship, system::SystemParam},
    prelude::*,
};
use bevy_console::ConsoleCommand;
#[cfg(target_arch = "wasm32")]
use common::structs::AudioSettings;
use common::{
    rpc::RpcStreamSender,
    sets::SceneSets,
    structs::SystemAudio,
    util::{redact_access_token, AsH160},
};
use console::{AddDebugSection, DebugInput, DebugInputKind, DebugSection, DoAddConsoleCommand};
#[cfg(not(target_arch = "wasm32"))]
use livekit::DisconnectReason;
use social::{
    voice::{VoiceEvent, VoiceQuery, VoiceRefusal},
    SocialClient, VoiceEventReceived,
};
use system_bridge::{
    CommunityVoiceUpdate as CommunityVoiceUpdateData, SystemApi, VoiceChatAction,
    VoiceChatConnection, VoiceChatParticipant as VoiceChatParticipantData,
    VoiceChatState as VoiceChatStateData,
};
use tokio::sync::{mpsc::Sender, oneshot};
use wallet::Wallet;

#[cfg(target_arch = "wasm32")]
use crate::livekit::web::DisconnectReason;
use crate::{
    global_crdt::ChannelControl,
    livekit::{
        participant::{
            ActiveSpeaker, HostedBy, LivekitParticipant, ParticipantIndex,
            ParticipantMetadataChanged,
        },
        room::{ConnectFailed, Connected, Connecting, Disconnected, Reconnecting},
        standalone_room_components,
        track::{Audio, Microphone, PublishedBy, SubscribeToTrack},
    },
    DisableRealmComms, DisableSceneRoomGatekeeper, NetworkMessage,
};

use self::state::{CallEnding, CallState, Effect, Input, Kind, Metadata, Reply, Role, Rpc, Sound};

pub struct VoiceChatPlugin;

impl Plugin for VoiceChatPlugin {
    fn build(&self, app: &mut App) {
        // a headless app has no microphone and plays nothing: no calls
        if app
            .world()
            .get_resource::<common::structs::NoRenderApp>()
            .is_some()
        {
            return;
        }
        app.init_resource::<VoiceChat>()
            .init_resource::<CallMic>()
            .init_resource::<VoiceInputs>();
        app.add_systems(
            Update,
            (
                // sees an account change while `social.0` is still the old account's client,
                // which ends the call it leaves
                drive_voice_chat
                    .run_if(resource_exists::<SocialClient>)
                    .in_set(SceneSets::RestrictedActions),
                subscribe_voice_room_tracks,
                #[cfg(target_arch = "wasm32")]
                apply_voice_room_volume,
            )
                .chain(),
        );
        app.add_observer(participant_metadata_changed);

        app.add_console_command::<VoiceCommand, _>(voice_command);
        app.add_debug_section(DebugSection {
            title: "Voice call",
            query: "/voice_state",
            fields: &[
                ("Status", "call.status"),
                ("Kind", "call.kind"),
                ("Room", "call.connection"),
                ("Call id", "call.callId"),
                ("Community id", "call.communityId"),
                ("Peer", "call.peer"),
                ("Local role", "call.localRole"),
                ("Local speaker", "call.localSpeaker"),
                ("Hand raised", "call.localRequestingToSpeak"),
                ("Participants", "call.participants"),
                ("Last error", "call.error"),
            ],
            actions: &[
                ("Accept", "/voice accept"),
                ("Reject", "/voice reject"),
                ("Hang up / leave", "/voice end"),
                ("Close the stream", "/voice close"),
            ],
            inputs: &[
                DebugInput {
                    label: "Call mic",
                    field: "call.micEnabled",
                    command: "/voice mic {value}",
                    kind: DebugInputKind::Toggle,
                },
                DebugInput {
                    label: "Hand",
                    field: "call.localRequestingToSpeak",
                    command: "/voice hand {value}",
                    kind: DebugInputKind::ActionButton {
                        enable: ("Raise hand", "up"),
                        disable: ("Lower hand", "down"),
                    },
                },
                DebugInput {
                    label: "Call (wallet address)",
                    field: "",
                    command: "/voice call {value}",
                    kind: DebugInputKind::Text,
                },
                DebugInput {
                    label: "Join stream (community id)",
                    field: "",
                    command: "/voice join {value}",
                    kind: DebugInputKind::Text,
                },
                DebugInput {
                    label: "Start stream (community id)",
                    field: "",
                    command: "/voice start {value}",
                    kind: DebugInputKind::Text,
                },
            ],
        });
    }
}

/// How often, and how far apart, a dropped call room is re-dialled.
const MAX_RECONNECTION_ATTEMPTS: u32 = 3;
const RECONNECTION_DELAY_SECS: f64 = 2.0;
/// How long a dropped room waits for LiveKit's reason before it counts as a lost connection.
const UNEXPLAINED_DROP_SECS: f64 = 1.0;
/// `voice_chat_call_tune.mp3` plays once; the ring loops
/// it by re-triggering it this often (its length).
const CALL_TUNE_SECS: f32 = 3.056;
/// What the active-speaker stream names the voice room's channel.
pub const VOICE_CHAT_CHANNEL: &str = "voice-chat";

/// The call microphone: whether it publishes into the voice room (the mic plugin reads it), and
/// whether a call is connected (the mic key toggles it instead of the nearby mic then).
#[derive(Resource, Default)]
pub struct CallMic {
    pub enabled: bool,
    pub in_call: bool,
    /// From the call reaching `InCall` until it is over, the nearby mic does not publish.
    pub suppress_nearby: bool,
}

/// Inputs queued by observers and the mic key for the next drive.
#[derive(Resource, Default)]
pub struct VoiceInputs(pub VecDeque<Input>);

/// The voice room entity: the LiveKit room of the call, with its outgoing channels (unused, the
/// room carries no data of ours, but the control channel must stay open) and its re-dial budget.
#[derive(Component)]
pub struct VoiceRoom {
    pub kind: Kind,
    reconnects: u32,
    redial_at: Option<f64>,
    /// When the room was seen dropped with no reason yet.
    unexplained_drop_at: Option<f64>,
    _sender: Sender<NetworkMessage>,
    _control: Sender<ChannelControl>,
}

impl VoiceRoom {
    fn new(kind: Kind, sender: Sender<NetworkMessage>, control: Sender<ChannelControl>) -> Self {
        Self {
            kind,
            reconnects: 0,
            redial_at: None,
            unexplained_drop_at: None,
            _sender: sender,
            _control: control,
        }
    }
}

/// Set by the room plugin when the voice room's connection dropped, from LiveKit's reason.
#[derive(Component, Debug)]
pub struct VoiceRoomDisconnected {
    /// The room will not come back (kicked, deleted, closed, duplicate identity…).
    pub terminal: bool,
    pub ending: CallEnding,
    /// LiveKit's reason, for the log.
    pub reason: String,
}

impl VoiceRoomDisconnected {
    /// A timeout, a media failure or an agent error gets the re-dial budget; every other
    /// reason ends the call.
    pub(crate) fn new(reason: &DisconnectReason) -> Self {
        let terminal = !matches!(
            reason,
            DisconnectReason::ConnectionTimeout
                | DisconnectReason::MediaFailure
                | DisconnectReason::AgentError
        );
        let ending = match reason {
            DisconnectReason::ClientInitiated
            | DisconnectReason::RoomDeleted
            | DisconnectReason::RoomClosed => CallEnding::Ended,
            DisconnectReason::ParticipantRemoved => CallEnding::Removed,
            DisconnectReason::DuplicateIdentity => CallEnding::OtherDevice,
            _ => CallEnding::ConnectionLost,
        };
        Self {
            terminal,
            ending,
            reason: format!("{reason:?}"),
        }
    }
}

/// A voice room participant: its address and the gatekeeper's metadata, refreshed as it changes.
/// Speaking is the participant's [`ActiveSpeaker`].
#[derive(Component, Clone, Debug, PartialEq, Eq)]
pub struct CallParticipant {
    pub address: Address,
    pub metadata: Metadata,
}

/// A microphone track of the voice room, subscribed by this module and played flat.
#[derive(Component)]
pub struct VoiceRoomTrack;

/// A procedure in flight, `None` until its reply. A dropped channel is a transport failure.
struct Pending(Box<dyn FnMut() -> Option<Reply> + Send + Sync>);

fn awaiting<T: Send + Sync + 'static>(
    reply: impl FnOnce(Result<T, VoiceRefusal>) -> Reply + Send + Sync + 'static,
) -> (oneshot::Sender<Result<T, VoiceRefusal>>, Pending) {
    let (tx, mut rx) = oneshot::channel();
    let mut reply = Some(reply);
    let poll = move || {
        let result = match rx.try_recv() {
            Ok(result) => result,
            Err(oneshot::error::TryRecvError::Empty) => return None,
            Err(oneshot::error::TryRecvError::Closed) => Err(VoiceRefusal::Transport(
                "the social service dropped the request".to_owned(),
            )),
        };
        reply.take().map(|reply| reply(result))
    };
    (tx, Pending(Box::new(poll)))
}

fn query(rpc: Rpc) -> (VoiceQuery, Pending) {
    let hex = |address: Address| format!("{address:#x}");
    match rpc {
        Rpc::StartPrivate { callee, attempt } => {
            let (response, pending) = awaiting(move |r| Reply::StartPrivate(attempt, r));
            let callee = hex(callee);
            (VoiceQuery::StartPrivate { callee, response }, pending)
        }
        Rpc::AcceptPrivate { call_id } => {
            let id = call_id.clone();
            let (response, pending) = awaiting(move |r| Reply::AcceptPrivate(id, r));
            (VoiceQuery::AcceptPrivate { call_id, response }, pending)
        }
        Rpc::RejectPrivate { call_id } => {
            let (response, pending) = awaiting(Reply::RejectPrivate);
            (VoiceQuery::RejectPrivate { call_id, response }, pending)
        }
        Rpc::EndPrivate { call_id } => {
            let id = call_id.clone();
            let (response, pending) = awaiting(move |r| Reply::EndPrivate(id, r));
            (VoiceQuery::EndPrivate { call_id, response }, pending)
        }
        Rpc::StartCommunity {
            community_id,
            attempt,
        } => {
            let id = community_id.clone();
            let (response, pending) = awaiting(move |r| Reply::StartCommunity(id, attempt, r));
            let query = VoiceQuery::StartCommunity {
                community_id,
                response,
            };
            (query, pending)
        }
        Rpc::JoinCommunity {
            community_id,
            attempt,
        } => {
            let id = community_id.clone();
            let (response, pending) = awaiting(move |r| Reply::JoinCommunity(id, attempt, r));
            let query = VoiceQuery::JoinCommunity {
                community_id,
                response,
            };
            (query, pending)
        }
        Rpc::EndCommunity { community_id } => {
            let id = community_id.clone();
            let (response, pending) = awaiting(move |r| Reply::EndCommunity(id, r));
            let query = VoiceQuery::EndCommunity {
                community_id,
                response,
            };
            (query, pending)
        }
        Rpc::RequestToSpeak {
            community_id,
            raise,
        } => {
            let (response, pending) = awaiting(Reply::Moderation);
            let query = VoiceQuery::RequestToSpeak {
                community_id,
                raising_hand: raise,
                response,
            };
            (query, pending)
        }
        Rpc::PromoteSpeaker {
            community_id,
            address,
        } => {
            let (response, pending) = awaiting(Reply::Moderation);
            let address = hex(address);
            let query = VoiceQuery::PromoteSpeaker {
                community_id,
                address,
                response,
            };
            (query, pending)
        }
        Rpc::DemoteSpeaker {
            community_id,
            address,
        } => {
            let (response, pending) = awaiting(Reply::Moderation);
            let address = hex(address);
            let query = VoiceQuery::DemoteSpeaker {
                community_id,
                address,
                response,
            };
            (query, pending)
        }
        Rpc::KickPlayer {
            community_id,
            address,
        } => {
            let (response, pending) = awaiting(Reply::Moderation);
            let address = hex(address);
            let query = VoiceQuery::KickPlayer {
                community_id,
                address,
                response,
            };
            (query, pending)
        }
        Rpc::RejectSpeakRequest {
            community_id,
            address,
        } => {
            let (response, pending) = awaiting(Reply::Moderation);
            let address = hex(address);
            let query = VoiceQuery::RejectSpeakRequest {
                community_id,
                address,
                response,
            };
            (query, pending)
        }
        Rpc::MuteSpeaker {
            community_id,
            address,
            muted,
        } => {
            let (response, pending) =
                awaiting(|r: Result<bool, _>| Reply::Moderation(r.map(|_| ())));
            let address = hex(address);
            let query = VoiceQuery::MuteSpeaker {
                community_id,
                address,
                muted,
                response,
            };
            (query, pending)
        }
    }
}

#[derive(Resource, Default)]
pub struct VoiceChat {
    pub state: CallState,
    /// The call room, as the HUD and `/voice_state` show it.
    pub connection: VoiceChatConnection,
    pending: Vec<Pending>,
    state_streams: Vec<RpcStreamSender<VoiceChatStateData>>,
    update_streams: Vec<RpcStreamSender<CommunityVoiceUpdateData>>,
    ring: Option<Timer>,
}

impl VoiceChat {
    /// The state as the HUD sees it.
    pub fn snapshot(&self, mic: &CallMic, participants: &CallParticipants) -> VoiceChatStateData {
        let state = &self.state;
        VoiceChatStateData {
            status: state.status,
            kind: state.kind,
            call_id: state.call_id.clone(),
            community_id: state.community_id.clone(),
            peer: state.peer.map(|a| format!("{a:#x}")),
            connection: self.connection,
            participants: participants.list(),
            mic_enabled: mic.enabled,
            local_speaker: state.local_speaker,
            local_role: state.local_role,
            local_requesting_to_speak: state.local_requesting_to_speak,
            error: state.error.clone(),
            error_is_notice: state.error_is_notice,
            error_seq: state.error_seq(),
        }
    }
}

#[derive(SystemParam)]
pub struct CallParticipants<'w, 's> {
    participants: Query<'w, 's, (&'static CallParticipant, Has<ActiveSpeaker>)>,
}

impl CallParticipants<'_, '_> {
    fn list(&self) -> Vec<VoiceChatParticipantData> {
        let mut list = self
            .participants
            .iter()
            .map(|(participant, speaking)| {
                let metadata = &participant.metadata;
                VoiceChatParticipantData {
                    address: format!("{:#x}", participant.address),
                    name: metadata.name.clone(),
                    has_claimed_name: metadata.has_claimed_name,
                    profile_picture_url: metadata.profile_picture_url.clone(),
                    role: Role::parse(&metadata.role),
                    is_speaker: metadata.is_speaker,
                    is_requesting_to_speak: metadata.is_requesting_to_speak,
                    is_muted: metadata.muted,
                    is_speaking: speaking,
                }
            })
            .collect::<Vec<_>>();
        list.sort_by(|a, b| a.address.cmp(&b.address));
        list
    }
}

type ChangedCallParticipant = (
    With<CallParticipant>,
    Or<(Changed<CallParticipant>, Added<ActiveSpeaker>)>,
);

#[derive(SystemParam)]
struct CallParticipantChanges<'w, 's> {
    changed: Query<'w, 's, (), ChangedCallParticipant>,
    participants: Query<'w, 's, (), With<CallParticipant>>,
    left: RemovedComponents<'w, 's, CallParticipant>,
    silent: RemovedComponents<'w, 's, ActiveSpeaker>,
}

impl CallParticipantChanges<'_, '_> {
    fn any(&mut self) -> bool {
        // every removal is read, so none is left over for the next drive
        let left = self.left.read().count() > 0;
        let participants = &self.participants;
        let silent = self
            .silent
            .read()
            .filter(|entity| participants.contains(*entity))
            .count()
            > 0;
        left || silent || !self.changed.is_empty()
    }
}

fn action_input(action: &VoiceChatAction) -> Result<Input, String> {
    let address = |a: &str| {
        a.as_h160()
            .ok_or_else(|| format!("`{a}` is not a wallet address"))
    };
    Ok(match action {
        VoiceChatAction::StartPrivateCall { address: a } => Input::StartPrivate(address(a)?),
        VoiceChatAction::AcceptCall => Input::Accept,
        VoiceChatAction::RejectCall => Input::Reject,
        VoiceChatAction::EndCall | VoiceChatAction::LeaveCall => Input::EndCall,
        VoiceChatAction::StartCommunityCall { community_id } => {
            Input::StartCommunity(community_id.clone())
        }
        VoiceChatAction::JoinCommunityCall { community_id } => {
            Input::JoinCommunity(community_id.clone())
        }
        VoiceChatAction::EndCommunityCall => Input::EndCommunity,
        VoiceChatAction::RequestToSpeak { raise } => Input::RequestToSpeak(*raise),
        VoiceChatAction::PromoteSpeaker { address: a } => Input::PromoteSpeaker(address(a)?),
        VoiceChatAction::DemoteSpeaker { address: a } => Input::DemoteSpeaker(address(a)?),
        VoiceChatAction::KickPlayer { address: a } => Input::KickPlayer(address(a)?),
        VoiceChatAction::RejectSpeakRequest { address: a } => {
            Input::RejectSpeakRequest(address(a)?)
        }
        VoiceChatAction::SetMic { enabled } => Input::SetMic(*enabled),
        VoiceChatAction::DismissError => Input::DismissError,
    })
}

fn sound_clip(sound: Sound) -> Option<&'static str> {
    Some(match sound {
        Sound::RingStart => "embedded://sounds/ui/voice_chat_call_tune.mp3",
        Sound::Join => "embedded://sounds/ui/voice_chat_join.wav",
        Sound::Leave | Sound::EndStream => "embedded://sounds/ui/voice_chat_leave.wav",
        Sound::RaiseHand | Sound::StartStream => "embedded://sounds/ui/generic_button_press.wav",
        Sound::RingStop => return None,
    })
}

type VoiceRoomQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static mut VoiceRoom,
        (
            Has<Connected>,
            Has<Connecting>,
            Has<Reconnecting>,
            Has<ConnectFailed>,
            Has<Disconnected>,
        ),
        Option<&'static VoiceRoomDisconnected>,
    ),
>;

fn user_refusal(
    disable_realm_comms: Option<&DisableRealmComms>,
    disable_gatekeeper: &DisableSceneRoomGatekeeper,
    wallet: &Wallet,
) -> Option<&'static str> {
    if disable_realm_comms.is_some_and(|d| d.0) || disable_gatekeeper.0 {
        Some("voice chat is disabled")
    } else if wallet.address().is_none() || wallet.is_guest() {
        Some(state::GUEST_REFUSAL)
    } else {
        None
    }
}

#[allow(clippy::too_many_arguments)]
fn drive_voice_chat(
    mut commands: Commands,
    mut voice: ResMut<VoiceChat>,
    mut call_mic: ResMut<CallMic>,
    mut inputs: ResMut<VoiceInputs>,
    mut system_api: EventReader<SystemApi>,
    mut social_events: EventReader<VoiceEventReceived>,
    social: Res<SocialClient>,
    wallet: Res<Wallet>,
    time: Res<Time>,
    mut rooms: VoiceRoomQuery,
    participants: CallParticipants,
    mut participant_changes: CallParticipantChanges,
    disable_realm_comms: Option<Res<DisableRealmComms>>,
    disable_gatekeeper: Res<DisableSceneRoomGatekeeper>,
) {
    let now = time.elapsed_secs_f64();
    let me = wallet.address();
    let mut force_snapshot = participant_changes.any();
    if voice.state.me() != me {
        if voice.state.is_active() {
            info!(target: "comms::voice_chat", "the account changed: ending the call");
        }
        inputs.0.push_front(Input::SignedOut);
        // the HUD clears its call state on logout
        force_snapshot = true;
    }

    // the HUD and the console
    let refusal = user_refusal(disable_realm_comms.as_deref(), &disable_gatekeeper, &wallet);
    for event in system_api.read() {
        match event {
            SystemApi::GetVoiceChatStateStream(sender) => {
                voice.state_streams.push(sender.clone());
                force_snapshot = true;
            }
            SystemApi::GetCommunityVoiceUpdateStream(sender) => {
                voice.update_streams.push(sender.clone());
            }
            SystemApi::VoiceChatAction(action) => {
                debug!(target: "comms::voice_chat", "action {action:?}");
                if let Some(reason) = refusal {
                    voice.state.set_error(reason.to_owned());
                    force_snapshot = true;
                } else {
                    match action_input(action) {
                        Ok(input) => inputs.0.push_back(input),
                        Err(e) => {
                            voice.state.set_error(e);
                            force_snapshot = true;
                        }
                    }
                }
            }
            _ => {}
        }
    }

    // the social service
    for VoiceEventReceived(event) in social_events.read() {
        debug!(target: "comms::voice_chat", "social event {}", redact_access_token(&format!("{event:?}")));
        match event {
            VoiceEvent::Connected => inputs.0.push_back(Input::SocialConnected),
            VoiceEvent::Disconnected => inputs.0.push_back(Input::SocialDisconnected),
            VoiceEvent::Private(update) => inputs.0.push_back(Input::PrivateUpdate(update.clone())),
            VoiceEvent::Community(update) => {
                let data = CommunityVoiceUpdateData {
                    community_id: update.community_id.clone(),
                    status: match update.status {
                        social::voice::CommunityVoiceStatus::Started => "started".to_owned(),
                        social::voice::CommunityVoiceStatus::Ended => "ended".to_owned(),
                    },
                    created_at: update.created_at,
                    ended_at: update.ended_at,
                    positions: update.positions.clone(),
                    worlds: update.worlds.clone(),
                    is_member: update.is_member,
                    community_name: update.community_name.clone(),
                    community_image: update.community_image.clone(),
                };
                voice.update_streams.retain(|s| !s.is_closed());
                for sender in &voice.update_streams {
                    let _ = sender.send(data.clone());
                }
                inputs.0.push_back(Input::CommunityUpdate(update.clone()));
            }
        }
    }

    // the procedures' replies
    if !voice.pending.is_empty() {
        let mut pending = std::mem::take(&mut voice.pending);
        pending.retain_mut(|p| match (p.0)() {
            Some(reply) => {
                debug!(target: "comms::voice_chat", "reply {}", redact_access_token(&format!("{reply:?}")));
                inputs.0.push_back(Input::Reply(reply));
                false
            }
            None => true,
        });
        voice.pending = pending;
    }

    // the room
    let mut connection = VoiceChatConnection::None;
    for (
        entity,
        mut room,
        (connected, connecting, reconnecting, failed, disconnected),
        disconnected_reason,
    ) in rooms.iter_mut()
    {
        connection = if connected {
            VoiceChatConnection::Connected
        } else if reconnecting || room.redial_at.is_some() {
            VoiceChatConnection::Reconnecting
        } else {
            VoiceChatConnection::Connecting
        };
        if connected && !voice.state.room_connected {
            room.reconnects = 0;
            inputs.0.push_back(Input::RoomConnected);
        }
        if !disconnected && room.unexplained_drop_at.is_some() {
            room.unexplained_drop_at = None;
        }
        // a refused dial keeps its `Connecting` marker beside `ConnectFailed`
        let dropped = match disconnected_reason {
            Some(d) => Some((d.terminal, d.ending, d.reason.clone())),
            None if failed && room.redial_at.is_none() => Some((
                false,
                CallEnding::Refused,
                "the connection was refused".to_owned(),
            )),
            // the reason may trail the drop, or never come (one the web mirror cannot parse)
            None if disconnected && room.redial_at.is_none() => {
                let since = *room.unexplained_drop_at.get_or_insert(now);
                (now - since >= UNEXPLAINED_DROP_SECS).then(|| {
                    (
                        false,
                        CallEnding::ConnectionLost,
                        "dropped without a reason".to_owned(),
                    )
                })
            }
            None => None,
        };
        if let Some((terminal, ending, reason)) = dropped {
            room.unexplained_drop_at = None;
            commands
                .entity(entity)
                .remove::<(VoiceRoomDisconnected, ConnectFailed)>();
            if terminal || room.reconnects >= MAX_RECONNECTION_ATTEMPTS {
                warn!(
                    target: "comms::voice_chat",
                    "voice room dropped: {reason} (after {} re-dials)",
                    room.reconnects
                );
                inputs.0.push_back(Input::RoomDisconnected {
                    terminal: true,
                    ending,
                });
            } else {
                room.reconnects += 1;
                room.redial_at = Some(now + RECONNECTION_DELAY_SECS);
                connection = VoiceChatConnection::Reconnecting;
                info!(
                    target: "comms::voice_chat",
                    "voice room dropped ({reason}); re-dialling in {RECONNECTION_DELAY_SECS}s ({}/{MAX_RECONNECTION_ATTEMPTS})",
                    room.reconnects
                );
                inputs.0.push_back(Input::RoomDisconnected {
                    terminal: false,
                    ending,
                });
            }
        } else if room.redial_at.is_some_and(|due| now >= due) {
            room.redial_at = None;
            // only an added marker dials: a refused room still carries the last one
            if connecting {
                commands.entity(entity).remove::<Connecting>();
            }
            commands.entity(entity).try_insert(Connecting);
        }
    }

    // the ring
    if voice.ring.is_some() {
        if let Some(ring) = voice.ring.as_mut() {
            ring.tick(time.delta());
            if ring.just_finished() {
                commands.send_event(SystemAudio(
                    sound_clip(Sound::RingStart).unwrap_or_default().to_owned(),
                ));
            }
        }
    }

    // drive; an idle tick must not mark the resource changed
    let mut effects = voice.bypass_change_detection().state.tick(now);
    let had_inputs = !inputs.0.is_empty();
    if had_inputs {
        let mut queued = std::mem::take(&mut inputs.0);
        while let Some(input) = queued.pop_front() {
            effects.extend(voice.state.apply(input, now));
        }
    }
    let changed = had_inputs || !effects.is_empty();
    for effect in effects {
        debug!(target: "comms::voice_chat", "effect {}", redact_access_token(&format!("{effect:?}")));
        match effect {
            Effect::Rpc(rpc) => {
                let (query, pending) = query(rpc);
                voice.pending.push(pending);
                // no client, or its channel is gone: the reply channel was dropped with the
                // query and the next drive reads that as a transport failure
                match social.0.as_ref().map(|c| c.voice(query)) {
                    Some(Ok(())) => (),
                    Some(Err(e)) => warn!(target: "comms::voice_chat", "voice query failed: {e}"),
                    None => warn!(target: "comms::voice_chat", "voice query with no social client"),
                }
            }
            Effect::SpawnRoom(url) => {
                for (entity, ..) in rooms.iter() {
                    commands.entity(entity).despawn();
                }
                let Some(("livekit", address)) = url.split_once(':') else {
                    warn!(target: "comms::voice_chat", "unsupported voice adapter `{}`", redact_access_token(&url));
                    inputs.0.push_back(Input::RoomDisconnected {
                        terminal: true,
                        ending: CallEnding::Refused,
                    });
                    continue;
                };
                let (components, sender, control) = standalone_room_components(address.to_owned());
                commands.spawn((
                    components,
                    VoiceRoom::new(voice.state.kind, sender, control),
                ));
                connection = VoiceChatConnection::Connecting;
                info!(target: "comms::voice_chat", "joining the voice room");
            }
            Effect::DespawnRoom => {
                for (entity, ..) in rooms.iter() {
                    commands.entity(entity).despawn();
                }
                connection = VoiceChatConnection::None;
            }
            Effect::Sound(sound) => {
                match sound {
                    Sound::RingStart => {
                        voice.ring = Some(Timer::from_seconds(CALL_TUNE_SECS, TimerMode::Repeating))
                    }
                    Sound::RingStop => voice.ring = None,
                    _ => {}
                }
                if let Some(clip) = sound_clip(sound) {
                    commands.send_event(SystemAudio(clip.to_owned()));
                }
            }
            Effect::SetCallMic(enabled) => {
                call_mic.enabled = enabled;
                voice.state.mic_enabled = enabled;
            }
        }
    }
    let connection_changed = voice.connection != connection;
    if connection_changed {
        voice.connection = connection;
    }
    if voice.state.me() != me {
        voice.state.set_me(me);
    }
    let in_call = voice.state.is_in_call() && voice.state.room_connected;
    if call_mic.in_call != in_call {
        call_mic.in_call = in_call;
    }
    let suppress_nearby = if voice.state.is_in_call() {
        true
    } else if voice.state.status.is_not_connected() {
        false
    } else {
        call_mic.suppress_nearby
    };
    if call_mic.suppress_nearby != suppress_nearby {
        call_mic.suppress_nearby = suppress_nearby;
    }
    if !call_mic.in_call && call_mic.enabled {
        call_mic.enabled = false;
        voice.state.mic_enabled = false;
    }

    // the HUD's snapshot
    if voice.state_streams.iter().any(|s| s.is_closed()) {
        voice.state_streams.retain(|s| !s.is_closed());
    }
    let dirty = force_snapshot || changed || connection_changed || call_mic.is_changed();
    if voice.state_streams.is_empty() || !dirty {
        return;
    }
    let snapshot = voice.snapshot(&call_mic, &participants);
    for sender in &voice.state_streams {
        let _ = sender.send(snapshot.clone());
    }
}

/// Every microphone track published in the voice room is subscribed (the room is no transport,
/// so nothing else would) and played flat.
#[expect(clippy::type_complexity, reason = "Queries are complex")]
fn subscribe_voice_room_tracks(
    mut commands: Commands,
    tracks: Query<(Entity, &PublishedBy), (With<Microphone>, With<Audio>, Without<VoiceRoomTrack>)>,
    participants: Query<&HostedBy>,
    rooms: Query<(), With<VoiceRoom>>,
) {
    for (track, published_by) in tracks.iter() {
        let Ok(hosted_by) = participants.get(published_by.get()) else {
            continue;
        };
        if !rooms.contains(hosted_by.get()) {
            continue;
        }
        debug!(target: "comms::voice_chat", "subscribing to voice room track {track}");
        commands
            .entity(track)
            .try_insert(VoiceRoomTrack)
            .trigger(SubscribeToTrack);
    }
}

/// On the web the page plays the room's audio through a pan/volume rig: a voice room track is
/// centred and follows the voice chat volume setting (nearby voice is placed by `mic.rs`).
#[cfg(target_arch = "wasm32")]
fn apply_voice_room_volume(
    newly_subscribed: Query<
        &crate::livekit::track::LivekitTrack,
        (
            With<VoiceRoomTrack>,
            Added<crate::livekit::track::Subscribed>,
        ),
    >,
    all: Query<
        &crate::livekit::track::LivekitTrack,
        (
            With<VoiceRoomTrack>,
            With<crate::livekit::track::Subscribed>,
        ),
    >,
    settings: Res<AudioSettings>,
) {
    // the JS maps 0..1 to a stereo pan of -1..1: 0.5 is the centre
    const CENTRE: f32 = 0.5;
    let tracks: Vec<&crate::livekit::track::LivekitTrack> = if settings.is_changed() {
        all.iter().collect()
    } else {
        newly_subscribed.iter().collect()
    };
    for track in tracks {
        if let Some(remote) = track.track() {
            remote.pan_and_volume(CENTRE, settings.voice_chat());
        }
    }
}

fn participant_address(participant: &LivekitParticipant) -> Option<Address> {
    participant.identity().as_str().as_h160()
}

/// A voice room participant's metadata arrives on join and on every change.
fn participant_metadata_changed(
    trigger: Trigger<ParticipantMetadataChanged>,
    mut commands: Commands,
    rooms: Query<&ParticipantIndex, With<VoiceRoom>>,
    participants: Query<&CallParticipant>,
    mut inputs: ResMut<VoiceInputs>,
) {
    let ParticipantMetadataChanged { participant, room } = trigger.event();
    let Ok(index) = rooms.get(*room) else {
        return;
    };
    let Some(address) = participant_address(participant) else {
        return;
    };
    let Some(&entity) = index.get(&ParticipantIndex::key(participant.identity().as_str())) else {
        return;
    };
    let metadata = Metadata::parse(&participant.metadata());
    let previous = participants.get(entity).ok();
    if previous.is_some_and(|p| p.metadata == metadata) {
        return;
    }
    if previous.is_none() {
        debug!(target: "comms::voice_chat", "{address:#x} joined the voice room");
    }
    // a participant who joins with the hand already up raised it before the local user came
    let raised_hand = previous.is_some_and(|p| !p.metadata.is_requesting_to_speak)
        && metadata.is_requesting_to_speak;
    commands.entity(entity).try_insert(CallParticipant {
        address,
        metadata: metadata.clone(),
    });
    inputs.0.push_back(Input::ParticipantMetadata {
        address,
        metadata,
        raised_hand,
    });
}

// --- console ---

/// `/voice <action>` does what the HUD's voice chat buttons do, through the same path and
/// refusals (`SystemApi::VoiceChatAction`). It only queues the request: read `/voice_state`
/// `call` for the outcome.
#[derive(clap::Parser, ConsoleCommand)]
#[command(name = "/voice")]
struct VoiceCommand {
    #[command(subcommand)]
    action: VoiceSubcommand,
}

#[derive(clap::Subcommand)]
enum VoiceSubcommand {
    /// Ring a user (a private call).
    Call { address: String },
    /// Answer the ringing private call.
    Accept,
    /// Decline the ringing private call.
    Reject,
    /// Hang up the private call, or leave the community stream.
    End,
    /// Open a community's stream (owner or moderator).
    Start { community_id: String },
    /// Join a community's stream.
    Join { community_id: String },
    /// Close the community's stream for everyone (owner or moderator).
    Close,
    /// Raise (`up`) or lower (`down`) the hand in a community stream.
    Hand { position: String },
    /// Publish (`on`) or stop (`off`) the call microphone.
    Mic { state: String },
    /// Make a listener a speaker.
    Promote { address: String },
    /// Make a speaker a listener (the local user's own address leaves the stage).
    Demote { address: String },
    /// Remove a participant from the stream.
    Kick { address: String },
    /// Turn down a request to speak.
    Deny { address: String },
}

fn voice_command(mut input: ConsoleCommand<VoiceCommand>, mut events: EventWriter<SystemApi>) {
    let Some(Ok(command)) = input.take() else {
        return;
    };
    let address = |address: String| match address.as_h160() {
        Some(_) => Ok(address),
        None => Err(format!("`{address}` is not a wallet address")),
    };
    let switch = |value: &str, on: &str, off: &str| match value {
        v if v == on || v == "true" => Ok(true),
        v if v == off || v == "false" => Ok(false),
        other => Err(format!("`{other}`: use `{on}` or `{off}`")),
    };
    let action = match command.action {
        VoiceSubcommand::Call { address: a } => {
            address(a).map(|address| VoiceChatAction::StartPrivateCall { address })
        }
        VoiceSubcommand::Accept => Ok(VoiceChatAction::AcceptCall),
        VoiceSubcommand::Reject => Ok(VoiceChatAction::RejectCall),
        VoiceSubcommand::End => Ok(VoiceChatAction::EndCall),
        VoiceSubcommand::Start { community_id } => {
            Ok(VoiceChatAction::StartCommunityCall { community_id })
        }
        VoiceSubcommand::Join { community_id } => {
            Ok(VoiceChatAction::JoinCommunityCall { community_id })
        }
        VoiceSubcommand::Close => Ok(VoiceChatAction::EndCommunityCall),
        VoiceSubcommand::Hand { position } => {
            switch(&position, "up", "down").map(|raise| VoiceChatAction::RequestToSpeak { raise })
        }
        VoiceSubcommand::Mic { state } => {
            switch(&state, "on", "off").map(|enabled| VoiceChatAction::SetMic { enabled })
        }
        VoiceSubcommand::Promote { address: a } => {
            address(a).map(|address| VoiceChatAction::PromoteSpeaker { address })
        }
        VoiceSubcommand::Demote { address: a } => {
            address(a).map(|address| VoiceChatAction::DemoteSpeaker { address })
        }
        VoiceSubcommand::Kick { address: a } => {
            address(a).map(|address| VoiceChatAction::KickPlayer { address })
        }
        VoiceSubcommand::Deny { address: a } => {
            address(a).map(|address| VoiceChatAction::RejectSpeakRequest { address })
        }
    };
    match action {
        Ok(action) => {
            input.reply_ok(format!("{action:?}"));
            events.write(SystemApi::VoiceChatAction(action));
        }
        Err(e) => input.reply_failed(e),
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    fn in_call() -> (App, Entity) {
        let mut app = App::new();
        app.add_event::<SystemApi>()
            .add_event::<VoiceEventReceived>()
            .add_event::<SystemAudio>()
            .init_resource::<VoiceChat>()
            .init_resource::<CallMic>()
            .init_resource::<VoiceInputs>()
            .init_resource::<SocialClient>()
            .init_resource::<Wallet>()
            .init_resource::<DisableSceneRoomGatekeeper>()
            .insert_resource(Time::<()>::default())
            .init_resource::<Dials>()
            .add_observer(|_: Trigger<OnAdd, Connecting>, mut dials: ResMut<Dials>| {
                dials.0 += 1;
            })
            .add_systems(Update, drive_voice_chat);
        {
            let mut voice = app.world_mut().resource_mut::<VoiceChat>();
            voice
                .state
                .apply(Input::JoinCommunity("c1".to_owned()), 0.0);
            let attempt = 1;
            voice.state.apply(
                Input::Reply(Reply::JoinCommunity(
                    "c1".to_owned(),
                    attempt,
                    Ok("livekit:ws://x".to_owned()),
                )),
                0.0,
            );
            assert!(voice.state.is_in_call());
        }
        let (components, sender, control) = standalone_room_components("ws://x".to_owned());
        let room = app
            .world_mut()
            .spawn((
                components,
                VoiceRoom::new(Kind::Community, sender, control),
                Connecting,
            ))
            .id();
        app.update();
        (app, room)
    }

    #[derive(Resource, Default)]
    struct Dials(u32);

    fn refuse(app: &mut App, room: Entity) {
        app.world_mut().entity_mut(room).insert(ConnectFailed);
        app.update();
    }

    fn wait(app: &mut App, secs: f64) {
        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(Duration::from_secs_f64(secs));
        app.update();
    }

    #[test]
    fn a_refused_dial_is_redialled_until_the_budget_ends_the_call() {
        let (mut app, room) = in_call();
        assert_eq!(app.world().resource::<Dials>().0, 1);
        for attempt in 1..=MAX_RECONNECTION_ATTEMPTS {
            refuse(&mut app, room);
            let entity = app.world().entity(room);
            assert!(!entity.contains::<ConnectFailed>());
            assert_eq!(entity.get::<VoiceRoom>().unwrap().reconnects, attempt);
            assert_eq!(
                app.world().resource::<VoiceChat>().connection,
                VoiceChatConnection::Reconnecting
            );
            wait(&mut app, RECONNECTION_DELAY_SECS / 2.0);
            assert_eq!(app.world().resource::<Dials>().0, attempt);
            wait(&mut app, RECONNECTION_DELAY_SECS);
            assert_eq!(app.world().resource::<Dials>().0, attempt + 1);
            assert!(app.world().resource::<VoiceChat>().state.is_in_call());
        }
        refuse(&mut app, room);
        app.update();
        let voice = app.world().resource::<VoiceChat>();
        assert_eq!(voice.state.status, state::Status::Disconnected);
        assert!(voice
            .state
            .error
            .as_deref()
            .is_some_and(|e| e.contains("Couldn't connect")));
        assert!(app.world().get_entity(room).is_err());
    }

    #[test]
    fn a_media_failure_is_redialled_and_a_deleted_room_ends_the_call_quietly() {
        let (mut app, room) = in_call();
        app.world_mut()
            .entity_mut(room)
            .insert(VoiceRoomDisconnected::new(&DisconnectReason::MediaFailure));
        app.update();
        assert_eq!(
            app.world()
                .entity(room)
                .get::<VoiceRoom>()
                .unwrap()
                .reconnects,
            1
        );
        assert!(app.world().resource::<VoiceChat>().state.is_in_call());
        wait(&mut app, RECONNECTION_DELAY_SECS);
        assert_eq!(app.world().resource::<Dials>().0, 2);

        app.world_mut()
            .entity_mut(room)
            .insert(VoiceRoomDisconnected::new(&DisconnectReason::RoomDeleted));
        app.update();
        let voice = app.world().resource::<VoiceChat>();
        assert_eq!(voice.state.status, state::Status::Disconnected);
        assert_eq!(voice.state.error.as_deref(), Some("The stream ended."));
        assert!(voice.state.error_is_notice);
        assert_eq!(voice.connection, VoiceChatConnection::None);
    }
}
