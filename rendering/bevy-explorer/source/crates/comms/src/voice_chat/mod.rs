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
use bevy::{ecs::relationship::Relationship, prelude::*};
use bevy_console::ConsoleCommand;
#[cfg(target_arch = "wasm32")]
use common::structs::AudioSettings;
use common::{
    rpc::RpcStreamSender,
    sets::SceneSets,
    structs::SystemAudio,
    util::{redact_access_token, AsH160},
};
use console::DoAddConsoleCommand;
use social::{
    voice::{VoiceEvent, VoiceQuery, VoiceRefusal},
    SocialClient, VoiceEventReceived,
};
use system_bridge::{
    CommunityVoiceUpdate as CommunityVoiceUpdateData, SystemApi, VoiceChatAction,
    VoiceChatParticipant as VoiceChatParticipantData, VoiceChatState as VoiceChatStateData,
};
use tokio::sync::{mpsc::Sender, oneshot};
use wallet::Wallet;

use crate::{
    global_crdt::ChannelControl,
    livekit::{
        participant::{
            ActiveSpeaker, HostedBy, LivekitParticipant, ParticipantConnected,
            ParticipantDisconnected, ParticipantMetadataChanged,
        },
        room::{ConnectFailed, Connected, Connecting, Reconnecting},
        standalone_room_components,
        track::{Audio, Microphone, PublishedBy, SubscribeToTrack},
    },
    DisableRealmComms, DisableSceneRoomGatekeeper, NetworkMessage,
};

use self::state::{CallState, Effect, Input, Kind, Metadata, Reply, Rpc, Sound};

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
            .init_resource::<VoiceInputs>()
            .init_resource::<VoiceCommands>();
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
        app.add_observer(participant_connected);
        app.add_observer(participant_disconnected);
        app.add_observer(participant_metadata_changed);
        app.add_observer(speaker_added);
        app.add_observer(speaker_removed);

        app.add_console_command::<VoiceCallCommand, _>(voice_call);
        app.add_console_command::<VoiceAcceptCommand, _>(voice_accept);
        app.add_console_command::<VoiceRejectCommand, _>(voice_reject);
        app.add_console_command::<VoiceEndCommand, _>(voice_end);
        app.add_console_command::<VoiceCommunityCommand, _>(voice_community);
        app.add_console_command::<VoiceHandCommand, _>(voice_hand);
        app.add_console_command::<VoiceMicCommand, _>(voice_mic);
        app.add_console_command::<VoiceModerateCommand, _>(voice_moderate);
    }
}

/// How often, and how far apart, a dropped call room is re-dialled.
const MAX_RECONNECTION_ATTEMPTS: u32 = 3;
const RECONNECTION_DELAY_SECS: f64 = 2.0;
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

/// Console commands, refused like the HUD's actions.
#[derive(Resource, Default)]
struct VoiceCommands(VecDeque<Input>);

/// The voice room entity: the LiveKit room of the call, with its outgoing channels (unused, the
/// room carries no data of ours, but the control channel must stay open) and its re-dial budget.
#[derive(Component)]
pub struct VoiceRoom {
    pub kind: Kind,
    reconnects: u32,
    redial_at: Option<f64>,
    _sender: Sender<NetworkMessage>,
    _control: Sender<ChannelControl>,
}

/// Set by the room plugin when the voice room's connection dropped, with LiveKit's reason:
/// `terminal` when the room will not come back (kicked, deleted, closed, duplicate identity…).
#[derive(Component, Debug)]
pub struct VoiceRoomDisconnected {
    pub terminal: bool,
    pub reason: String,
}

/// A microphone track of the voice room, subscribed by this module and played flat.
#[derive(Component)]
pub struct VoiceRoomTrack;

enum Pending {
    StartPrivate(u64, oneshot::Receiver<Result<String, VoiceRefusal>>),
    AcceptPrivate(String, oneshot::Receiver<Result<String, VoiceRefusal>>),
    RejectPrivate(oneshot::Receiver<Result<(), VoiceRefusal>>),
    EndPrivate(String, oneshot::Receiver<Result<(), VoiceRefusal>>),
    StartCommunity(String, u64, oneshot::Receiver<Result<String, VoiceRefusal>>),
    JoinCommunity(String, u64, oneshot::Receiver<Result<String, VoiceRefusal>>),
    EndCommunity(String, oneshot::Receiver<Result<(), VoiceRefusal>>),
    Moderation(oneshot::Receiver<Result<(), VoiceRefusal>>),
    Mute(oneshot::Receiver<Result<bool, VoiceRefusal>>),
}

impl Pending {
    /// The reply, or `None` while it is in flight. A dropped channel is a transport failure.
    fn poll(&mut self) -> Option<Reply> {
        fn take<T>(
            rx: &mut oneshot::Receiver<Result<T, VoiceRefusal>>,
        ) -> Option<Result<T, VoiceRefusal>> {
            match rx.try_recv() {
                Ok(result) => Some(result),
                Err(oneshot::error::TryRecvError::Empty) => None,
                Err(oneshot::error::TryRecvError::Closed) => Some(Err(VoiceRefusal::Transport(
                    "the social service dropped the request".to_owned(),
                ))),
            }
        }
        Some(match self {
            Pending::StartPrivate(attempt, rx) => Reply::StartPrivate(*attempt, take(rx)?),
            Pending::AcceptPrivate(id, rx) => Reply::AcceptPrivate(id.clone(), take(rx)?),
            Pending::RejectPrivate(rx) => Reply::RejectPrivate(take(rx)?),
            Pending::EndPrivate(id, rx) => Reply::EndPrivate(id.clone(), take(rx)?),
            Pending::StartCommunity(id, attempt, rx) => {
                Reply::StartCommunity(id.clone(), *attempt, take(rx)?)
            }
            Pending::JoinCommunity(id, attempt, rx) => {
                Reply::JoinCommunity(id.clone(), *attempt, take(rx)?)
            }
            Pending::EndCommunity(id, rx) => Reply::EndCommunity(id.clone(), take(rx)?),
            Pending::Moderation(rx) => Reply::Moderation(take(rx)?),
            Pending::Mute(rx) => Reply::Moderation(take(rx)?.map(|_| ())),
        })
    }
}

#[derive(Resource, Default)]
pub struct VoiceChat {
    pub state: CallState,
    pending: Vec<Pending>,
    state_streams: Vec<RpcStreamSender<VoiceChatStateData>>,
    update_streams: Vec<RpcStreamSender<CommunityVoiceUpdateData>>,
    ring: Option<Timer>,
    last_snapshot: Option<VoiceChatStateData>,
}

impl VoiceChat {
    /// The state as the HUD sees it.
    pub fn snapshot(&self, mic: &CallMic, connection: &str) -> VoiceChatStateData {
        let state = &self.state;
        VoiceChatStateData {
            status: state.status.name().to_owned(),
            kind: state.kind.name().to_owned(),
            call_id: state.call_id.clone(),
            community_id: state.community_id.clone(),
            peer: state.peer.map(|a| format!("{a:#x}")),
            connection: connection.to_owned(),
            participants: state
                .participants
                .iter()
                .map(|(address, p)| VoiceChatParticipantData {
                    address: format!("{address:#x}"),
                    name: p.name.clone(),
                    has_claimed_name: p.has_claimed_name,
                    profile_picture_url: p.profile_picture_url.clone(),
                    role: p.role.name().to_owned(),
                    is_speaker: p.is_speaker,
                    is_requesting_to_speak: p.is_requesting_to_speak,
                    is_muted: p.is_muted,
                    is_speaking: p.is_speaking,
                })
                .collect(),
            mic_enabled: mic.enabled,
            local_speaker: state.local_speaker,
            local_role: state.local_role.name().to_owned(),
            local_requesting_to_speak: state.local_requesting_to_speak,
            error: state.error.clone(),
            error_seq: state.error_seq(),
        }
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

/// Every reason the SDK names is terminal; what is left
/// (a media failure, an agent error, a refused dial) gets the re-dial budget.
#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn is_terminal_disconnect(reason: &livekit::DisconnectReason) -> bool {
    !matches!(
        reason,
        livekit::DisconnectReason::MediaFailure | livekit::DisconnectReason::AgentError
    )
}

#[cfg(target_arch = "wasm32")]
pub(crate) fn is_terminal_disconnect(_reason: &crate::livekit::web::DisconnectReason) -> bool {
    true
}

type VoiceRoomQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static mut VoiceRoom,
        Has<Connected>,
        Has<Connecting>,
        Has<Reconnecting>,
        Has<ConnectFailed>,
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
    mut console: ResMut<VoiceCommands>,
    mut system_api: EventReader<SystemApi>,
    mut social_events: EventReader<VoiceEventReceived>,
    social: Res<SocialClient>,
    wallet: Res<Wallet>,
    time: Res<Time>,
    mut rooms: VoiceRoomQuery,
    disable_realm_comms: Option<Res<DisableRealmComms>>,
    disable_gatekeeper: Res<DisableSceneRoomGatekeeper>,
) {
    let now = time.elapsed_secs_f64();
    let me = wallet.address();
    let mut force_snapshot = false;
    if voice.state.me() != me {
        if voice.state.is_active() {
            info!(target: "comms::voice_chat", "the account changed: ending the call");
        }
        inputs.0.push_front(Input::SignedOut);
        // the HUD clears its call state on logout
        force_snapshot = true;
    }
    let refusal = user_refusal(disable_realm_comms.as_deref(), &disable_gatekeeper, &wallet);
    for input in console.0.drain(..) {
        match refusal {
            Some(reason) => {
                voice.state.set_error(reason.to_owned());
                force_snapshot = true;
            }
            None => inputs.0.push_back(input),
        }
    }

    // the HUD
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
    let mut pending = std::mem::take(&mut voice.pending);
    pending.retain_mut(|p| match p.poll() {
        Some(reply) => {
            debug!(target: "comms::voice_chat", "reply {}", redact_access_token(&format!("{reply:?}")));
            inputs.0.push_back(Input::Reply(reply));
            false
        }
        None => true,
    });
    voice.pending = pending;

    // the room
    let mut connection = "none";
    for (entity, mut room, connected, connecting, reconnecting, failed, disconnected) in
        rooms.iter_mut()
    {
        connection = if connected {
            "connected"
        } else if reconnecting || room.redial_at.is_some() {
            "reconnecting"
        } else {
            "connecting"
        };
        if connected && !voice.state.room_connected {
            room.reconnects = 0;
            inputs.0.push_back(Input::RoomConnected);
        }
        // a refused dial keeps its `Connecting` marker beside `ConnectFailed`
        let dropped = match disconnected {
            Some(d) => Some((d.terminal, d.reason.clone())),
            None if failed && room.redial_at.is_none() => {
                Some((false, "the connection was refused".to_owned()))
            }
            None => None,
        };
        if let Some((terminal, reason)) = dropped {
            commands
                .entity(entity)
                .remove::<(VoiceRoomDisconnected, ConnectFailed)>();
            if terminal || room.reconnects >= MAX_RECONNECTION_ATTEMPTS {
                let reason = if terminal {
                    reason
                } else {
                    format!("{reason} ({MAX_RECONNECTION_ATTEMPTS} reconnection attempts)")
                };
                warn!(target: "comms::voice_chat", "voice room dropped: {reason}");
                inputs.0.push_back(Input::RoomDisconnected {
                    terminal: true,
                    reason,
                });
            } else {
                room.reconnects += 1;
                room.redial_at = Some(now + RECONNECTION_DELAY_SECS);
                info!(
                    target: "comms::voice_chat",
                    "voice room dropped ({reason}); re-dialling in {RECONNECTION_DELAY_SECS}s ({}/{MAX_RECONNECTION_ATTEMPTS})",
                    room.reconnects
                );
                inputs.0.push_back(Input::RoomDisconnected {
                    terminal: false,
                    reason,
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
    if let Some(ring) = voice.ring.as_mut() {
        ring.tick(time.delta());
        if ring.just_finished() {
            commands.send_event(SystemAudio(
                sound_clip(Sound::RingStart).unwrap_or_default().to_owned(),
            ));
        }
    }

    // drive
    let mut effects = voice.state.tick(now);
    let mut queued = std::mem::take(&mut inputs.0);
    while let Some(input) = queued.pop_front() {
        effects.extend(voice.state.apply(input, now));
    }
    let changed = !effects.is_empty();
    for effect in effects {
        debug!(target: "comms::voice_chat", "effect {}", redact_access_token(&format!("{effect:?}")));
        match effect {
            Effect::Rpc(rpc) => {
                let client = social.0.as_ref();
                let outcome = match rpc {
                    Rpc::StartPrivate { callee, attempt } => {
                        let (tx, rx) = oneshot::channel();
                        voice.pending.push(Pending::StartPrivate(attempt, rx));
                        client.map(|c| {
                            c.voice(VoiceQuery::StartPrivate {
                                callee: format!("{callee:#x}"),
                                response: tx,
                            })
                        })
                    }
                    Rpc::AcceptPrivate { call_id } => {
                        let (tx, rx) = oneshot::channel();
                        voice
                            .pending
                            .push(Pending::AcceptPrivate(call_id.clone(), rx));
                        client.map(|c| {
                            c.voice(VoiceQuery::AcceptPrivate {
                                call_id,
                                response: tx,
                            })
                        })
                    }
                    Rpc::RejectPrivate { call_id } => {
                        let (tx, rx) = oneshot::channel();
                        voice.pending.push(Pending::RejectPrivate(rx));
                        client.map(|c| {
                            c.voice(VoiceQuery::RejectPrivate {
                                call_id,
                                response: tx,
                            })
                        })
                    }
                    Rpc::EndPrivate { call_id } => {
                        let (tx, rx) = oneshot::channel();
                        voice.pending.push(Pending::EndPrivate(call_id.clone(), rx));
                        client.map(|c| {
                            c.voice(VoiceQuery::EndPrivate {
                                call_id,
                                response: tx,
                            })
                        })
                    }
                    Rpc::StartCommunity {
                        community_id,
                        attempt,
                    } => {
                        let (tx, rx) = oneshot::channel();
                        voice.pending.push(Pending::StartCommunity(
                            community_id.clone(),
                            attempt,
                            rx,
                        ));
                        client.map(|c| {
                            c.voice(VoiceQuery::StartCommunity {
                                community_id,
                                response: tx,
                            })
                        })
                    }
                    Rpc::JoinCommunity {
                        community_id,
                        attempt,
                    } => {
                        let (tx, rx) = oneshot::channel();
                        voice.pending.push(Pending::JoinCommunity(
                            community_id.clone(),
                            attempt,
                            rx,
                        ));
                        client.map(|c| {
                            c.voice(VoiceQuery::JoinCommunity {
                                community_id,
                                response: tx,
                            })
                        })
                    }
                    Rpc::EndCommunity { community_id } => {
                        let (tx, rx) = oneshot::channel();
                        voice
                            .pending
                            .push(Pending::EndCommunity(community_id.clone(), rx));
                        client.map(|c| {
                            c.voice(VoiceQuery::EndCommunity {
                                community_id,
                                response: tx,
                            })
                        })
                    }
                    Rpc::RequestToSpeak {
                        community_id,
                        raise,
                    } => {
                        let (tx, rx) = oneshot::channel();
                        voice.pending.push(Pending::Moderation(rx));
                        client.map(|c| {
                            c.voice(VoiceQuery::RequestToSpeak {
                                community_id,
                                raising_hand: raise,
                                response: tx,
                            })
                        })
                    }
                    Rpc::PromoteSpeaker {
                        community_id,
                        address,
                    } => {
                        let (tx, rx) = oneshot::channel();
                        voice.pending.push(Pending::Moderation(rx));
                        client.map(|c| {
                            c.voice(VoiceQuery::PromoteSpeaker {
                                community_id,
                                address: format!("{address:#x}"),
                                response: tx,
                            })
                        })
                    }
                    Rpc::DemoteSpeaker {
                        community_id,
                        address,
                    } => {
                        let (tx, rx) = oneshot::channel();
                        voice.pending.push(Pending::Moderation(rx));
                        client.map(|c| {
                            c.voice(VoiceQuery::DemoteSpeaker {
                                community_id,
                                address: format!("{address:#x}"),
                                response: tx,
                            })
                        })
                    }
                    Rpc::KickPlayer {
                        community_id,
                        address,
                    } => {
                        let (tx, rx) = oneshot::channel();
                        voice.pending.push(Pending::Moderation(rx));
                        client.map(|c| {
                            c.voice(VoiceQuery::KickPlayer {
                                community_id,
                                address: format!("{address:#x}"),
                                response: tx,
                            })
                        })
                    }
                    Rpc::RejectSpeakRequest {
                        community_id,
                        address,
                    } => {
                        let (tx, rx) = oneshot::channel();
                        voice.pending.push(Pending::Moderation(rx));
                        client.map(|c| {
                            c.voice(VoiceQuery::RejectSpeakRequest {
                                community_id,
                                address: format!("{address:#x}"),
                                response: tx,
                            })
                        })
                    }
                    Rpc::MuteSpeaker {
                        community_id,
                        address,
                        muted,
                    } => {
                        let (tx, rx) = oneshot::channel();
                        voice.pending.push(Pending::Mute(rx));
                        client.map(|c| {
                            c.voice(VoiceQuery::MuteSpeaker {
                                community_id,
                                address: format!("{address:#x}"),
                                muted,
                                response: tx,
                            })
                        })
                    }
                };
                match outcome {
                    Some(Ok(())) => (),
                    // no client, or its channel is gone: the reply channel was dropped with the
                    // query and the next drive reads that as a transport failure
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
                        reason: "unsupported voice adapter".to_owned(),
                    });
                    continue;
                };
                let (components, sender, control) = standalone_room_components(address.to_owned());
                commands.spawn((
                    components,
                    VoiceRoom {
                        kind: voice.state.kind,
                        reconnects: 0,
                        redial_at: None,
                        _sender: sender,
                        _control: control,
                    },
                ));
                connection = "connecting";
                info!(target: "comms::voice_chat", "joining the voice room");
            }
            Effect::DespawnRoom => {
                for (entity, ..) in rooms.iter() {
                    commands.entity(entity).despawn();
                }
                connection = "none";
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
    voice.state.set_me(me);
    call_mic.in_call = voice.state.is_in_call() && voice.state.room_connected;
    if voice.state.is_in_call() {
        call_mic.suppress_nearby = true;
    } else if voice.state.status.is_not_connected() {
        call_mic.suppress_nearby = false;
    }
    if !call_mic.in_call && call_mic.enabled {
        call_mic.enabled = false;
        voice.state.mic_enabled = false;
    }

    // the HUD's snapshot
    voice.state_streams.retain(|s| !s.is_closed());
    if voice.state_streams.is_empty() {
        return;
    }
    let snapshot = voice.snapshot(&call_mic, connection);
    if force_snapshot || changed || voice.last_snapshot.as_ref() != Some(&snapshot) {
        for sender in &voice.state_streams {
            let _ = sender.send(snapshot.clone());
        }
        voice.last_snapshot = Some(snapshot);
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
/// centred and follows the voice volume setting (nearby voice is placed by `mic.rs`).
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
            remote.pan_and_volume(CENTRE, settings.voice());
        }
    }
}

fn participant_address(participant: &LivekitParticipant) -> Option<Address> {
    participant.identity().as_str().as_h160()
}

fn participant_connected(
    trigger: Trigger<ParticipantConnected>,
    rooms: Query<(), With<VoiceRoom>>,
    mut inputs: ResMut<VoiceInputs>,
) {
    let ParticipantConnected { participant, room } = trigger.event();
    if !rooms.contains(*room) {
        return;
    }
    let Some(address) = participant_address(participant) else {
        return;
    };
    debug!(target: "comms::voice_chat", "{address:#x} joined the voice room");
    inputs.0.push_back(Input::ParticipantJoined {
        address,
        metadata: Metadata::parse(&participant.metadata()),
    });
}

fn participant_disconnected(
    trigger: Trigger<ParticipantDisconnected>,
    rooms: Query<(), With<VoiceRoom>>,
    mut inputs: ResMut<VoiceInputs>,
) {
    let ParticipantDisconnected { participant, room } = trigger.event();
    if !rooms.contains(*room) {
        return;
    }
    let Some(address) = participant_address(participant) else {
        return;
    };
    debug!(target: "comms::voice_chat", "{address:#x} left the voice room");
    inputs.0.push_back(Input::ParticipantLeft(address));
}

fn participant_metadata_changed(
    trigger: Trigger<ParticipantMetadataChanged>,
    rooms: Query<(), With<VoiceRoom>>,
    mut inputs: ResMut<VoiceInputs>,
) {
    let ParticipantMetadataChanged { participant, room } = trigger.event();
    if !rooms.contains(*room) {
        return;
    }
    let Some(address) = participant_address(participant) else {
        return;
    };
    inputs.0.push_back(Input::ParticipantMetadata {
        address,
        metadata: Metadata::parse(&participant.metadata()),
    });
}

fn speaker_changed(
    entity: Entity,
    active: bool,
    participants: &Query<(&LivekitParticipant, &HostedBy)>,
    rooms: &Query<(), With<VoiceRoom>>,
    inputs: &mut VoiceInputs,
) {
    let Ok((participant, hosted_by)) = participants.get(entity) else {
        return;
    };
    if !rooms.contains(hosted_by.get()) {
        return;
    }
    if let Some(address) = participant_address(participant) {
        inputs.0.push_back(Input::Speaking { address, active });
    }
}

fn speaker_added(
    trigger: Trigger<OnInsert, ActiveSpeaker>,
    participants: Query<(&LivekitParticipant, &HostedBy)>,
    rooms: Query<(), With<VoiceRoom>>,
    mut inputs: ResMut<VoiceInputs>,
) {
    speaker_changed(trigger.target(), true, &participants, &rooms, &mut inputs);
}

fn speaker_removed(
    trigger: Trigger<OnReplace, ActiveSpeaker>,
    participants: Query<(&LivekitParticipant, &HostedBy)>,
    rooms: Query<(), With<VoiceRoom>>,
    mut inputs: ResMut<VoiceInputs>,
) {
    speaker_changed(trigger.target(), false, &participants, &rooms, &mut inputs);
}

// --- console ---

/// `/voice_call <address>` rings a user (a private call).
#[derive(clap::Parser, ConsoleCommand)]
#[command(name = "/voice_call")]
struct VoiceCallCommand {
    address: String,
}

fn voice_call(mut input: ConsoleCommand<VoiceCallCommand>, mut inputs: ResMut<VoiceCommands>) {
    let Some(Ok(command)) = input.take() else {
        return;
    };
    let Some(address) = command.address.as_h160() else {
        input.reply_failed(format!("`{}` is not a wallet address", command.address));
        return;
    };
    inputs.0.push_back(Input::StartPrivate(address));
    input.reply_ok(format!("calling {address:#x}"));
}

/// `/voice_accept` answers the ringing private call.
#[derive(clap::Parser, ConsoleCommand)]
#[command(name = "/voice_accept")]
struct VoiceAcceptCommand;

fn voice_accept(mut input: ConsoleCommand<VoiceAcceptCommand>, mut inputs: ResMut<VoiceCommands>) {
    if input.take().is_some() {
        inputs.0.push_back(Input::Accept);
        input.reply_ok("accepting");
    }
}

/// `/voice_reject` declines the ringing private call.
#[derive(clap::Parser, ConsoleCommand)]
#[command(name = "/voice_reject")]
struct VoiceRejectCommand;

fn voice_reject(mut input: ConsoleCommand<VoiceRejectCommand>, mut inputs: ResMut<VoiceCommands>) {
    if input.take().is_some() {
        inputs.0.push_back(Input::Reject);
        input.reply_ok("rejecting");
    }
}

/// `/voice_end` hangs up the private call or leaves the community stream.
#[derive(clap::Parser, ConsoleCommand)]
#[command(name = "/voice_end")]
struct VoiceEndCommand;

fn voice_end(mut input: ConsoleCommand<VoiceEndCommand>, mut inputs: ResMut<VoiceCommands>) {
    if input.take().is_some() {
        inputs.0.push_back(Input::EndCall);
        input.reply_ok("ending");
    }
}

/// `/voice_community <start|join|end> <community_id>` opens, joins or closes a community
/// stream (`leave` is `/voice_end`).
#[derive(clap::Parser, ConsoleCommand)]
#[command(name = "/voice_community")]
struct VoiceCommunityCommand {
    /// `start`, `join` or `end`
    action: String,
    community_id: Option<String>,
}

fn voice_community(
    mut input: ConsoleCommand<VoiceCommunityCommand>,
    mut inputs: ResMut<VoiceCommands>,
) {
    let Some(Ok(command)) = input.take() else {
        return;
    };
    let id = command.community_id.clone().unwrap_or_default();
    let action = match command.action.as_str() {
        "start" if !id.is_empty() => Input::StartCommunity(id),
        "join" if !id.is_empty() => Input::JoinCommunity(id),
        "end" => Input::EndCommunity,
        "leave" => Input::EndCall,
        other => {
            input.reply_failed(format!(
                "unknown action `{other}`: use `start <id>`, `join <id>`, `end` or `leave`"
            ));
            return;
        }
    };
    input.reply_ok(format!(
        "{} {}",
        command.action,
        command.community_id.unwrap_or_default()
    ));
    inputs.0.push_back(action);
}

/// `/voice_hand <up|down>` raises or lowers the hand in a community stream.
#[derive(clap::Parser, ConsoleCommand)]
#[command(name = "/voice_hand")]
struct VoiceHandCommand {
    /// `up` or `down`
    position: String,
}

fn voice_hand(mut input: ConsoleCommand<VoiceHandCommand>, mut inputs: ResMut<VoiceCommands>) {
    let Some(Ok(command)) = input.take() else {
        return;
    };
    let raise = match command.position.as_str() {
        "up" => true,
        "down" => false,
        other => {
            input.reply_failed(format!("`{other}`: use `up` or `down`"));
            return;
        }
    };
    inputs.0.push_back(Input::RequestToSpeak(raise));
    input.reply_ok(format!("hand {}", command.position));
}

/// `/voice_mic <on|off>` publishes or stops the call microphone.
#[derive(clap::Parser, ConsoleCommand)]
#[command(name = "/voice_mic")]
struct VoiceMicCommand {
    /// `on` or `off`
    state: String,
}

fn voice_mic(mut input: ConsoleCommand<VoiceMicCommand>, mut inputs: ResMut<VoiceCommands>) {
    let Some(Ok(command)) = input.take() else {
        return;
    };
    let enabled = match command.state.as_str() {
        "on" => true,
        "off" => false,
        other => {
            input.reply_failed(format!("`{other}`: use `on` or `off`"));
            return;
        }
    };
    inputs.0.push_back(Input::SetMic(enabled));
    input.reply_ok(format!("call mic {}", command.state));
}

/// `/voice_moderate <promote|demote|kick|reject> <address>` moderates a community stream.
#[derive(clap::Parser, ConsoleCommand)]
#[command(name = "/voice_moderate")]
struct VoiceModerateCommand {
    /// `promote`, `demote`, `kick` or `reject`
    action: String,
    address: String,
}

fn voice_moderate(
    mut input: ConsoleCommand<VoiceModerateCommand>,
    mut inputs: ResMut<VoiceCommands>,
) {
    let Some(Ok(command)) = input.take() else {
        return;
    };
    let Some(address) = command.address.as_h160() else {
        input.reply_failed(format!("`{}` is not a wallet address", command.address));
        return;
    };
    let action = match command.action.as_str() {
        "promote" => Input::PromoteSpeaker(address),
        "demote" => Input::DemoteSpeaker(address),
        "kick" => Input::KickPlayer(address),
        "reject" => Input::RejectSpeakRequest(address),
        other => {
            input.reply_failed(format!(
                "unknown action `{other}`: use `promote`, `demote`, `kick` or `reject`"
            ));
            return;
        }
    };
    inputs.0.push_back(action);
    input.reply_ok(format!("{} {address:#x}", command.action));
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
            .init_resource::<VoiceCommands>()
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
                VoiceRoom {
                    kind: Kind::Community,
                    reconnects: 0,
                    redial_at: None,
                    _sender: sender,
                    _control: control,
                },
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
            .is_some_and(|e| e.contains("refused")));
        assert!(app.world().get_entity(room).is_err());
    }
}
