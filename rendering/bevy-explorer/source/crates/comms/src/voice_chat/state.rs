//! The call state machine, off the ECS: every input (a HUD command, an RPC reply, a social update,
//! a room or participant change) is applied to [`CallState`] and answered with the effects the
//! engine must perform. Statuses, their transitions and the guards follow unity-explorer's
//! `Assets/DCL/VoiceChat/Core/VoiceChatStatus.cs`, `VoiceChatOrchestrator.cs`,
//! `PrivateVoiceChat/PrivateVoiceChatCallStatusService.cs` and
//! `CommunityVoiceChat/CommunityVoiceChatCallStatusService.cs`.

use std::collections::BTreeMap;

use alloy_core::primitives::Address;
use serde::Deserialize;
use social::voice::{
    CommunityVoiceStatus, CommunityVoiceUpdate, PrivateVoiceStatus, PrivateVoiceUpdate,
    VoiceRefusal,
};

/// Unity `VoiceChatStatus`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Status {
    #[default]
    Disconnected,
    /// A private call is ringing for the local user.
    ReceivedCall,
    /// The start / join / accept procedure is in flight.
    StartingCall,
    /// A private call is ringing on the other side.
    StartedCall,
    InCall,
    EndingCall,
    RejectingCall,
    /// The other side (or the local user) is already in a call.
    Busy,
    GenericError,
}

impl Status {
    /// Unity `IsNotConnected`: a new call may start.
    pub fn is_not_connected(self) -> bool {
        matches!(
            self,
            Status::Disconnected | Status::Busy | Status::GenericError
        )
    }

    pub fn name(self) -> &'static str {
        match self {
            Status::Disconnected => "disconnected",
            Status::ReceivedCall => "receivedCall",
            Status::StartingCall => "startingCall",
            Status::StartedCall => "startedCall",
            Status::InCall => "inCall",
            Status::EndingCall => "endingCall",
            Status::RejectingCall => "rejectingCall",
            Status::Busy => "busy",
            Status::GenericError => "genericError",
        }
    }
}

/// Unity `VoiceChatType` (nearby voice is the existing, separate feature).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Kind {
    #[default]
    None,
    Private,
    Community,
}

impl Kind {
    pub fn name(self) -> &'static str {
        match self {
            Kind::None => "none",
            Kind::Private => "private",
            Kind::Community => "community",
        }
    }
}

/// Unity `VoiceChatParticipantCommunityRole`, from the participant metadata `role`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Role {
    #[default]
    None,
    User,
    Moderator,
    Owner,
}

impl Role {
    pub fn parse(role: &str) -> Self {
        match role.to_ascii_lowercase().as_str() {
            "user" | "member" => Role::User,
            "moderator" => Role::Moderator,
            "owner" => Role::Owner,
            _ => Role::None,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Role::None => "none",
            Role::User => "user",
            Role::Moderator => "moderator",
            Role::Owner => "owner",
        }
    }

    /// Unity `VoiceChatRoleHelper.IsModeratorOrOwner`.
    pub fn is_moderator(self) -> bool {
        matches!(self, Role::Moderator | Role::Owner)
    }
}

/// The participant metadata the gatekeeper stamps on a community room token and patches as
/// moderation happens (`services/catalyst/comms/src/gatekeeper/voice/mod.rs`); a private room
/// carries none. Unity `ParticipantCallMetadata`.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", default)]
pub struct Metadata {
    pub name: String,
    pub profile_picture_url: String,
    pub has_claimed_name: bool,
    pub is_requesting_to_speak: bool,
    pub is_speaker: bool,
    pub muted: bool,
    pub role: String,
}

impl Metadata {
    /// Missing or malformed metadata is the default (a listener with no role).
    pub fn parse(json: &str) -> Self {
        serde_json::from_str(json).unwrap_or_default()
    }
}

/// What the state knows of a room participant.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Participant {
    pub name: String,
    pub profile_picture_url: String,
    pub has_claimed_name: bool,
    pub role: Role,
    pub is_speaker: bool,
    pub is_requesting_to_speak: bool,
    pub is_muted: bool,
    pub is_speaking: bool,
}

impl Participant {
    fn apply(&mut self, metadata: &Metadata) {
        self.name = metadata.name.clone();
        self.profile_picture_url = metadata.profile_picture_url.clone();
        self.has_claimed_name = metadata.has_claimed_name;
        self.role = Role::parse(&metadata.role);
        self.is_speaker = metadata.is_speaker;
        self.is_requesting_to_speak = metadata.is_requesting_to_speak;
        self.is_muted = metadata.muted;
    }
}

/// A procedure the engine must run against the social service.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Rpc {
    StartPrivate {
        callee: Address,
    },
    AcceptPrivate {
        call_id: String,
    },
    RejectPrivate {
        call_id: String,
    },
    EndPrivate {
        call_id: String,
    },
    StartCommunity {
        community_id: String,
    },
    JoinCommunity {
        community_id: String,
    },
    EndCommunity {
        community_id: String,
    },
    RequestToSpeak {
        community_id: String,
        raise: bool,
    },
    PromoteSpeaker {
        community_id: String,
        address: Address,
    },
    DemoteSpeaker {
        community_id: String,
        address: Address,
    },
    KickPlayer {
        community_id: String,
        address: Address,
    },
    RejectSpeakRequest {
        community_id: String,
        address: Address,
    },
    MuteSpeaker {
        community_id: String,
        address: Address,
        muted: bool,
    },
}

/// Unity's call sounds (`PrivateVoiceChatView`, `CommunityVoiceChatInCallView`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sound {
    /// Start the ring tune loop.
    RingStart,
    RingStop,
    Join,
    Leave,
    RaiseHand,
    StartStream,
    EndStream,
}

/// What the engine does after an input.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Effect {
    Rpc(Rpc),
    /// Join the LiveKit room at this connection url.
    SpawnRoom(String),
    DespawnRoom,
    Sound(Sound),
    /// Publish (or stop publishing) the call microphone.
    SetCallMic(bool),
}

/// The reply of a procedure the state waits for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Reply {
    StartPrivate(Result<String, VoiceRefusal>),
    AcceptPrivate(Result<String, VoiceRefusal>),
    RejectPrivate(Result<(), VoiceRefusal>),
    EndPrivate(Result<(), VoiceRefusal>),
    StartCommunity(Result<String, VoiceRefusal>),
    JoinCommunity(Result<String, VoiceRefusal>),
    EndCommunity(Result<(), VoiceRefusal>),
    /// Raise hand, promote, demote, kick, reject, mute: fire and forget, the result only logged
    /// (Unity) or shown as the last error.
    Moderation(Result<(), VoiceRefusal>),
}

/// Everything that moves the state.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Input {
    // the HUD or the console
    StartPrivate(Address),
    Accept,
    Reject,
    /// Hang up a private call, or leave a community stream.
    EndCall,
    StartCommunity(String),
    JoinCommunity(String),
    EndCommunity,
    RequestToSpeak(bool),
    PromoteSpeaker(Address),
    DemoteSpeaker(Address),
    KickPlayer(Address),
    RejectSpeakRequest(Address),
    SetMic(bool),
    /// Clear a busy / error status.
    DismissError,
    // the social service
    Reply(Reply),
    PrivateUpdate(PrivateVoiceUpdate),
    CommunityUpdate(CommunityVoiceUpdate),
    SocialConnected,
    SocialDisconnected,
    // the LiveKit room
    RoomConnected,
    /// The room dropped; `terminal` when it will not come back (kicked, deleted, duplicate
    /// identity, …) or the reconnection budget is spent.
    RoomDisconnected {
        terminal: bool,
        reason: String,
    },
    ParticipantJoined {
        address: Address,
        metadata: Metadata,
    },
    ParticipantLeft(Address),
    ParticipantMetadata {
        address: Address,
        metadata: Metadata,
    },
    Speaking {
        address: Address,
        active: bool,
    },
    /// The call microphone publishes (or stopped); mirrors the mic plugin.
    CallMicChanged(bool),
}

/// Unity `HANG_UP_BEFORE_NEW_CALL_DELAY_MS`: a community start / join while in another call
/// hangs up first and starts this long after.
pub const HANG_UP_BEFORE_NEW_CALL_SECS: f64 = 1.0;

/// Unity `VoiceChatMicrophoneHandler`: who is not allowed in.
pub const GUEST_REFUSAL: &str = "Voice chat is not available for guest accounts.";

#[derive(Clone, Debug, PartialEq, Eq)]
enum Deferred {
    StartCommunity(String),
    JoinCommunity(String),
}

#[derive(Clone, Debug, Default)]
pub struct CallState {
    pub status: Status,
    pub kind: Kind,
    pub call_id: Option<String>,
    pub community_id: Option<String>,
    /// The other party of a private call.
    pub peer: Option<Address>,
    pub connection_url: Option<String>,
    /// The LiveKit room is connected.
    pub room_connected: bool,
    pub mic_enabled: bool,
    /// The local user may publish: always in a private call, as a community speaker.
    pub local_speaker: bool,
    pub local_role: Role,
    pub local_requesting_to_speak: bool,
    /// Why the last action failed or the call could not happen; cleared by the next action.
    pub error: Option<String>,
    pub participants: BTreeMap<Address, Participant>,
    deferred: Option<(Deferred, f64)>,
    me: Option<Address>,
}

impl CallState {
    pub fn new(me: Option<Address>) -> Self {
        Self {
            me,
            ..Default::default()
        }
    }

    pub fn set_me(&mut self, me: Option<Address>) {
        self.me = me;
    }

    pub fn is_in_call(&self) -> bool {
        self.status == Status::InCall
    }

    /// A call is being set up, rings, or runs: the HUD shows a panel.
    pub fn is_active(&self) -> bool {
        !self.status.is_not_connected()
    }

    /// Back to no call; the last error and a deferred start survive it.
    fn reset(&mut self) {
        let me = self.me;
        let error = self.error.take();
        let deferred = self.deferred.take();
        *self = Self::new(me);
        self.error = error;
        self.deferred = deferred;
    }

    fn fail(&mut self, status: Status, message: impl Into<String>) {
        self.reset();
        self.status = status;
        self.error = Some(message.into());
    }

    /// Ends whatever is active without a server call: the room goes, the ring stops.
    fn drop_call(&mut self, effects: &mut Vec<Effect>) {
        let ringing = matches!(self.status, Status::StartedCall | Status::ReceivedCall);
        let in_room = self.connection_url.is_some();
        if ringing {
            effects.push(Effect::Sound(Sound::RingStop));
        }
        if in_room {
            effects.push(Effect::DespawnRoom);
            effects.push(Effect::Sound(Sound::Leave));
        }
        if self.mic_enabled {
            effects.push(Effect::SetCallMic(false));
        }
        self.reset();
    }

    /// Applies one input; the effects are the engine's to run, in order.
    pub fn apply(&mut self, input: Input, now: f64) -> Vec<Effect> {
        let mut effects = Vec::new();
        match input {
            Input::StartPrivate(callee) => {
                if !self.status.is_not_connected() {
                    self.error = Some("already in a call".to_owned());
                    return effects;
                }
                self.reset();
                self.error = None;
                self.status = Status::StartingCall;
                self.kind = Kind::Private;
                self.peer = Some(callee);
                effects.push(Effect::Rpc(Rpc::StartPrivate { callee }));
            }
            Input::Accept => {
                if self.status != Status::ReceivedCall {
                    return effects;
                }
                let Some(call_id) = self.call_id.clone() else {
                    return effects;
                };
                // the tune stops the moment the callee answers (Unity: the InCall view replaces
                // the ringing one); a refused accept then fails without a second stop
                self.status = Status::StartingCall;
                effects.push(Effect::Sound(Sound::RingStop));
                effects.push(Effect::Rpc(Rpc::AcceptPrivate { call_id }));
            }
            Input::Reject => {
                if self.status != Status::ReceivedCall {
                    return effects;
                }
                let Some(call_id) = self.call_id.clone() else {
                    return effects;
                };
                self.status = Status::RejectingCall;
                effects.push(Effect::Sound(Sound::RingStop));
                effects.push(Effect::Rpc(Rpc::RejectPrivate { call_id }));
            }
            Input::EndCall => match self.kind {
                Kind::Private => {
                    if !matches!(
                        self.status,
                        Status::StartingCall | Status::StartedCall | Status::InCall
                    ) {
                        return effects;
                    }
                    let call_id = self.call_id.clone();
                    self.drop_call(&mut effects);
                    self.status = Status::EndingCall;
                    self.kind = Kind::Private;
                    // hung up before the start reply named the call: nothing to end yet; the
                    // reply ends it (see `Reply::StartPrivate`)
                    if let Some(call_id) = call_id {
                        effects.push(Effect::Rpc(Rpc::EndPrivate { call_id }));
                    }
                }
                Kind::Community => {
                    if self.status.is_not_connected() {
                        return effects;
                    }
                    // Unity `HangUp`: no server call, the room is just left
                    self.drop_call(&mut effects);
                }
                Kind::None => {}
            },
            Input::StartCommunity(community_id) => {
                self.error = None;
                if !self.status.is_not_connected() {
                    self.defer(Deferred::StartCommunity(community_id), now, &mut effects);
                    return effects;
                }
                self.start_community(community_id, &mut effects);
            }
            Input::JoinCommunity(community_id) => {
                self.error = None;
                if !self.status.is_not_connected() {
                    self.defer(Deferred::JoinCommunity(community_id), now, &mut effects);
                    return effects;
                }
                self.join_community(community_id, &mut effects);
            }
            Input::EndCommunity => {
                if let (Kind::Community, Some(community_id)) =
                    (self.kind, self.community_id.clone())
                {
                    if self.status == Status::InCall {
                        effects.push(Effect::Rpc(Rpc::EndCommunity { community_id }));
                    }
                }
            }
            Input::RequestToSpeak(raise) => {
                if let Some(community_id) = self.in_community() {
                    if raise {
                        effects.push(Effect::Sound(Sound::RaiseHand));
                    }
                    effects.push(Effect::Rpc(Rpc::RequestToSpeak {
                        community_id,
                        raise,
                    }));
                }
            }
            Input::PromoteSpeaker(address) => {
                if let Some(community_id) = self.in_community() {
                    effects.push(Effect::Rpc(Rpc::PromoteSpeaker {
                        community_id,
                        address,
                    }));
                }
            }
            Input::DemoteSpeaker(address) => {
                if let Some(community_id) = self.in_community() {
                    effects.push(Effect::Rpc(Rpc::DemoteSpeaker {
                        community_id,
                        address,
                    }));
                }
            }
            Input::KickPlayer(address) => {
                if let Some(community_id) = self.in_community() {
                    effects.push(Effect::Rpc(Rpc::KickPlayer {
                        community_id,
                        address,
                    }));
                }
            }
            Input::RejectSpeakRequest(address) => {
                if let Some(community_id) = self.in_community() {
                    effects.push(Effect::Rpc(Rpc::RejectSpeakRequest {
                        community_id,
                        address,
                    }));
                }
            }
            Input::SetMic(enabled) => {
                if self.status != Status::InCall || !self.room_connected {
                    return effects;
                }
                match self.kind {
                    Kind::Private => effects.push(Effect::SetCallMic(enabled)),
                    Kind::Community => {
                        if !self.local_speaker {
                            return effects;
                        }
                        effects.push(Effect::SetCallMic(enabled));
                        // Unity `VoiceChatMicrophoneHandler.NotifyMicrophoneStateChange`
                        if let (Some(community_id), Some(me)) = (self.community_id.clone(), self.me)
                        {
                            effects.push(Effect::Rpc(Rpc::MuteSpeaker {
                                community_id,
                                address: me,
                                muted: !enabled,
                            }));
                        }
                    }
                    Kind::None => {}
                }
            }
            Input::DismissError => {
                if matches!(self.status, Status::Busy | Status::GenericError) {
                    self.reset();
                    self.error = None;
                }
            }
            Input::Reply(reply) => self.reply(reply, &mut effects),
            Input::PrivateUpdate(update) => self.private_update(update, &mut effects),
            Input::CommunityUpdate(update) => {
                if update.status == CommunityVoiceStatus::Ended
                    && self.kind == Kind::Community
                    && self.community_id.as_deref() == Some(update.community_id.as_str())
                    && !self.status.is_not_connected()
                {
                    self.drop_call(&mut effects);
                    effects.push(Effect::Sound(Sound::EndStream));
                }
            }
            Input::SocialConnected => {
                if self.status == Status::GenericError {
                    self.reset();
                }
            }
            Input::SocialDisconnected => {
                // Unity: the transport closed while not in a call is an error
                if !self.status.is_not_connected() && self.status != Status::InCall {
                    self.drop_call(&mut effects);
                    self.status = Status::GenericError;
                    self.error = Some("the social service disconnected".to_owned());
                }
            }
            Input::RoomConnected => {
                if self.status != Status::InCall {
                    return effects;
                }
                self.room_connected = true;
                effects.push(Effect::Sound(Sound::Join));
                // Unity `VoiceChatRoomManager.OnConnectionEstablished`: the mic goes up in a
                // private call and for a speaker
                if self.kind == Kind::Private || self.local_speaker {
                    effects.push(Effect::SetCallMic(true));
                }
            }
            Input::RoomDisconnected { terminal, reason } => {
                self.room_connected = false;
                if terminal && self.status == Status::InCall {
                    self.drop_call(&mut effects);
                    self.error = Some(reason);
                }
            }
            Input::ParticipantJoined { address, metadata } => {
                let participant = self.participants.entry(address).or_default();
                participant.apply(&metadata);
                self.sync_local(address, &metadata, &mut effects);
            }
            Input::ParticipantLeft(address) => {
                self.participants.remove(&address);
            }
            Input::ParticipantMetadata { address, metadata } => {
                let participant = self.participants.entry(address).or_default();
                let was_requesting = participant.is_requesting_to_speak;
                participant.apply(&metadata);
                if !was_requesting
                    && metadata.is_requesting_to_speak
                    && self.local_role.is_moderator()
                {
                    effects.push(Effect::Sound(Sound::RaiseHand));
                }
                self.sync_local(address, &metadata, &mut effects);
            }
            Input::Speaking { address, active } => {
                if let Some(participant) = self.participants.get_mut(&address) {
                    participant.is_speaking = active;
                }
            }
            Input::CallMicChanged(enabled) => {
                self.mic_enabled = enabled;
            }
        }
        effects
    }

    /// Time-driven work: the deferred community start after a hang-up.
    pub fn tick(&mut self, now: f64) -> Vec<Effect> {
        let mut effects = Vec::new();
        if let Some((deferred, due)) = self.deferred.clone() {
            if now >= due && self.status.is_not_connected() {
                self.deferred = None;
                match deferred {
                    Deferred::StartCommunity(id) => self.start_community(id, &mut effects),
                    Deferred::JoinCommunity(id) => self.join_community(id, &mut effects),
                }
            }
        }
        effects
    }

    fn defer(&mut self, deferred: Deferred, now: f64, effects: &mut Vec<Effect>) {
        // Unity `VoiceChatOrchestrator`: hang up, wait, then start the new call
        self.deferred = Some((deferred, now + HANG_UP_BEFORE_NEW_CALL_SECS));
        effects.extend(self.apply(Input::EndCall, now));
    }

    fn in_community(&self) -> Option<String> {
        (self.kind == Kind::Community && self.status == Status::InCall)
            .then(|| self.community_id.clone())
            .flatten()
    }

    fn start_community(&mut self, community_id: String, effects: &mut Vec<Effect>) {
        self.reset();
        self.error = None;
        self.status = Status::StartingCall;
        self.kind = Kind::Community;
        self.community_id = Some(community_id.clone());
        effects.push(Effect::Sound(Sound::StartStream));
        effects.push(Effect::Rpc(Rpc::StartCommunity { community_id }));
    }

    fn join_community(&mut self, community_id: String, effects: &mut Vec<Effect>) {
        self.reset();
        self.error = None;
        self.status = Status::StartingCall;
        self.kind = Kind::Community;
        self.community_id = Some(community_id.clone());
        effects.push(Effect::Rpc(Rpc::JoinCommunity { community_id }));
    }

    /// The local participant's own metadata names the role and whether the mic may go up.
    fn sync_local(&mut self, address: Address, metadata: &Metadata, effects: &mut Vec<Effect>) {
        if self.me != Some(address) {
            return;
        }
        let was_speaker = self.local_speaker;
        self.local_role = Role::parse(&metadata.role);
        self.local_requesting_to_speak = metadata.is_requesting_to_speak;
        self.local_speaker = self.kind == Kind::Private || metadata.is_speaker;
        // Unity `VoiceChatRoomManager`: a demoted speaker's mic goes down; a promoted one's
        // comes up muted (here: stays down until toggled)
        if was_speaker && !self.local_speaker && self.mic_enabled {
            effects.push(Effect::SetCallMic(false));
        }
    }

    fn reply(&mut self, reply: Reply, effects: &mut Vec<Effect>) {
        match reply {
            Reply::StartPrivate(result) => {
                if self.kind != Kind::Private {
                    return;
                }
                match (self.status, result) {
                    (Status::StartingCall, Ok(call_id)) => {
                        self.call_id = Some(call_id);
                        self.status = Status::StartedCall;
                        effects.push(Effect::Sound(Sound::RingStart));
                    }
                    // hung up while the start was in flight: end the call the server just made
                    (Status::EndingCall, Ok(call_id)) => {
                        effects.push(Effect::Rpc(Rpc::EndPrivate { call_id }));
                    }
                    (Status::EndingCall, Err(_)) => {
                        self.reset();
                    }
                    (Status::StartingCall, Err(e)) => self.refused(e),
                    _ => {}
                }
            }
            Reply::AcceptPrivate(result) => {
                if self.kind != Kind::Private || self.status != Status::StartingCall {
                    return;
                }
                match result {
                    Ok(url) => self.enter_room(url, effects),
                    Err(e) => self.fail(Status::GenericError, e.message().to_owned()),
                }
            }
            Reply::RejectPrivate(result) => {
                if self.status == Status::RejectingCall {
                    if let Err(e) = result {
                        self.error = Some(e.message().to_owned());
                    }
                    self.reset();
                }
            }
            Reply::EndPrivate(result) => {
                if self.status == Status::EndingCall {
                    if let Err(e) = result {
                        self.error = Some(e.message().to_owned());
                    }
                    self.reset();
                }
            }
            Reply::StartCommunity(result) => {
                if self.kind != Kind::Community || self.status != Status::StartingCall {
                    return;
                }
                match result {
                    Ok(url) => {
                        // the creator is a speaker from the start (the token grants publish)
                        self.local_speaker = true;
                        self.enter_room(url, effects);
                    }
                    Err(e) => self.refused(e),
                }
            }
            Reply::JoinCommunity(result) => {
                if self.kind != Kind::Community || self.status != Status::StartingCall {
                    return;
                }
                match result {
                    Ok(url) => self.enter_room(url, effects),
                    Err(e) => self.fail(Status::GenericError, e.message().to_owned()),
                }
            }
            Reply::EndCommunity(result) => match result {
                Ok(()) => {
                    if self.kind == Kind::Community && !self.status.is_not_connected() {
                        self.drop_call(effects);
                        effects.push(Effect::Sound(Sound::EndStream));
                    }
                }
                Err(e) => self.error = Some(e.message().to_owned()),
            },
            Reply::Moderation(result) => {
                if let Err(e) = result {
                    self.error = Some(e.message().to_owned());
                }
            }
        }
    }

    /// Unity: `InvalidRequest` and `ConflictingError` are "busy", anything else an error.
    fn refused(&mut self, refusal: VoiceRefusal) {
        let status = match refusal {
            VoiceRefusal::Invalid(_) | VoiceRefusal::Conflict(_) => Status::Busy,
            _ => Status::GenericError,
        };
        self.fail(status, refusal.message().to_owned());
    }

    fn enter_room(&mut self, url: String, effects: &mut Vec<Effect>) {
        if matches!(self.status, Status::StartedCall | Status::ReceivedCall) {
            effects.push(Effect::Sound(Sound::RingStop));
        }
        self.status = Status::InCall;
        self.connection_url = Some(url.clone());
        effects.push(Effect::SpawnRoom(url));
    }

    fn private_update(&mut self, update: PrivateVoiceUpdate, effects: &mut Vec<Effect>) {
        // Unity `VoiceChatOrchestrator`: private updates are ignored during a community call
        if self.kind == Kind::Community && !self.status.is_not_connected() {
            return;
        }
        match update.status {
            PrivateVoiceStatus::Requested => {
                if !self.status.is_not_connected() {
                    return;
                }
                let Some(caller) = update.caller.as_deref().and_then(parse_address) else {
                    return;
                };
                self.reset();
                self.error = None;
                self.status = Status::ReceivedCall;
                self.kind = Kind::Private;
                self.call_id = Some(update.call_id);
                self.peer = Some(caller);
                effects.push(Effect::Sound(Sound::RingStart));
            }
            PrivateVoiceStatus::Accepted => {
                if self.kind != Kind::Private
                    || self.status != Status::StartedCall
                    || self.call_id.as_deref() != Some(update.call_id.as_str())
                {
                    return;
                }
                match update.connection_url {
                    Some(url) => self.enter_room(url, effects),
                    None => {
                        effects.push(Effect::Sound(Sound::RingStop));
                        self.fail(Status::GenericError, "accepted without credentials");
                    }
                }
            }
            PrivateVoiceStatus::Rejected
            | PrivateVoiceStatus::Ended
            | PrivateVoiceStatus::Expired => {
                if self.kind != Kind::Private
                    || self.call_id.as_deref() != Some(update.call_id.as_str())
                {
                    return;
                }
                let message = match update.status {
                    PrivateVoiceStatus::Rejected => Some("the call was declined"),
                    PrivateVoiceStatus::Expired => Some("no answer"),
                    _ => None,
                };
                self.drop_call(effects);
                self.error = message.map(str::to_owned);
            }
        }
    }
}

pub fn parse_address(address: &str) -> Option<Address> {
    address.parse::<Address>().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn addr(last: u8) -> Address {
        Address::from([last; 20])
    }

    fn me() -> Address {
        addr(1)
    }

    fn state() -> CallState {
        CallState::new(Some(me()))
    }

    fn update(call_id: &str, status: PrivateVoiceStatus) -> PrivateVoiceUpdate {
        PrivateVoiceUpdate {
            call_id: call_id.to_owned(),
            status,
            caller: None,
            callee: None,
            connection_url: None,
        }
    }

    fn ring(state: &mut CallState) -> Vec<Effect> {
        let mut effects = state.apply(Input::StartPrivate(addr(2)), 0.0);
        effects.extend(state.apply(
            Input::Reply(Reply::StartPrivate(Ok("call-1".to_owned()))),
            0.0,
        ));
        effects
    }

    #[test]
    fn a_private_call_rings_connects_and_hangs_up() {
        let mut s = state();
        let effects = s.apply(Input::StartPrivate(addr(2)), 0.0);
        assert_eq!(s.status, Status::StartingCall);
        assert_eq!(s.kind, Kind::Private);
        assert_eq!(
            effects,
            vec![Effect::Rpc(Rpc::StartPrivate { callee: addr(2) })]
        );

        let effects = s.apply(
            Input::Reply(Reply::StartPrivate(Ok("call-1".to_owned()))),
            0.0,
        );
        assert_eq!(s.status, Status::StartedCall);
        assert_eq!(s.call_id.as_deref(), Some("call-1"));
        assert_eq!(effects, vec![Effect::Sound(Sound::RingStart)]);

        let mut accepted = update("call-1", PrivateVoiceStatus::Accepted);
        accepted.connection_url = Some("livekit:wss://x?access_token=t".to_owned());
        let effects = s.apply(Input::PrivateUpdate(accepted), 0.0);
        assert_eq!(s.status, Status::InCall);
        assert_eq!(
            effects,
            vec![
                Effect::Sound(Sound::RingStop),
                Effect::SpawnRoom("livekit:wss://x?access_token=t".to_owned())
            ]
        );

        let effects = s.apply(Input::RoomConnected, 0.0);
        assert!(s.room_connected);
        assert_eq!(
            effects,
            vec![Effect::Sound(Sound::Join), Effect::SetCallMic(true)]
        );
        s.apply(Input::CallMicChanged(true), 0.0);

        let effects = s.apply(Input::EndCall, 0.0);
        assert_eq!(s.status, Status::EndingCall);
        assert_eq!(
            effects,
            vec![
                Effect::DespawnRoom,
                Effect::Sound(Sound::Leave),
                Effect::SetCallMic(false),
                Effect::Rpc(Rpc::EndPrivate {
                    call_id: "call-1".to_owned()
                })
            ]
        );
        s.apply(Input::Reply(Reply::EndPrivate(Ok(()))), 0.0);
        assert_eq!(s.status, Status::Disconnected);
        assert_eq!(s.kind, Kind::None);
        assert!(s.participants.is_empty());
    }

    #[test]
    fn a_busy_callee_is_busy_and_an_internal_error_is_an_error() {
        let mut s = state();
        s.apply(Input::StartPrivate(addr(2)), 0.0);
        s.apply(
            Input::Reply(Reply::StartPrivate(Err(VoiceRefusal::Conflict(
                "One of the users is busy calling someone else".to_owned(),
            )))),
            0.0,
        );
        assert_eq!(s.status, Status::Busy);
        assert!(s.error.as_deref().unwrap().contains("busy"));
        assert!(s.status.is_not_connected());

        s.apply(Input::DismissError, 0.0);
        assert_eq!(s.status, Status::Disconnected);
        assert_eq!(s.error, None);

        s.apply(Input::StartPrivate(addr(2)), 0.0);
        s.apply(
            Input::Reply(Reply::StartPrivate(Err(VoiceRefusal::Internal(
                "boom".to_owned(),
            )))),
            0.0,
        );
        assert_eq!(s.status, Status::GenericError);
    }

    #[test]
    fn an_incoming_call_is_accepted_with_the_reply_credentials() {
        let mut s = state();
        let mut requested = update("call-2", PrivateVoiceStatus::Requested);
        requested.caller = Some(format!("{:#x}", addr(3)));
        let effects = s.apply(Input::PrivateUpdate(requested), 0.0);
        assert_eq!(s.status, Status::ReceivedCall);
        assert_eq!(s.peer, Some(addr(3)));
        assert_eq!(effects, vec![Effect::Sound(Sound::RingStart)]);

        let effects = s.apply(Input::Accept, 0.0);
        assert_eq!(s.status, Status::StartingCall);
        assert_eq!(
            effects,
            vec![
                Effect::Sound(Sound::RingStop),
                Effect::Rpc(Rpc::AcceptPrivate {
                    call_id: "call-2".to_owned()
                })
            ]
        );
        let effects = s.apply(
            Input::Reply(Reply::AcceptPrivate(Ok("livekit:u".to_owned()))),
            0.0,
        );
        assert_eq!(s.status, Status::InCall);
        assert_eq!(effects, vec![Effect::SpawnRoom("livekit:u".to_owned())]);
    }

    #[test]
    fn an_incoming_call_is_rejected() {
        let mut s = state();
        let mut requested = update("call-2", PrivateVoiceStatus::Requested);
        requested.caller = Some(format!("{:#x}", addr(3)));
        s.apply(Input::PrivateUpdate(requested), 0.0);
        let effects = s.apply(Input::Reject, 0.0);
        assert_eq!(s.status, Status::RejectingCall);
        assert_eq!(
            effects,
            vec![
                Effect::Sound(Sound::RingStop),
                Effect::Rpc(Rpc::RejectPrivate {
                    call_id: "call-2".to_owned()
                })
            ]
        );
        s.apply(Input::Reply(Reply::RejectPrivate(Ok(()))), 0.0);
        assert_eq!(s.status, Status::Disconnected);
    }

    #[test]
    fn a_ringing_call_expires_is_declined_or_ended_by_the_other_side() {
        for (status, message) in [
            (PrivateVoiceStatus::Expired, Some("no answer")),
            (PrivateVoiceStatus::Rejected, Some("the call was declined")),
            (PrivateVoiceStatus::Ended, None),
        ] {
            let mut s = state();
            ring(&mut s);
            let effects = s.apply(Input::PrivateUpdate(update("call-1", status)), 0.0);
            assert_eq!(s.status, Status::Disconnected, "{status:?}");
            assert_eq!(effects, vec![Effect::Sound(Sound::RingStop)]);
            assert_eq!(s.error.as_deref(), message);
        }
    }

    #[test]
    fn an_update_for_another_call_is_ignored() {
        let mut s = state();
        ring(&mut s);
        let effects = s.apply(
            Input::PrivateUpdate(update("call-9", PrivateVoiceStatus::Ended)),
            0.0,
        );
        assert_eq!(s.status, Status::StartedCall);
        assert!(effects.is_empty());
    }

    #[test]
    fn hanging_up_during_the_start_ends_the_call_the_reply_names() {
        let mut s = state();
        s.apply(Input::StartPrivate(addr(2)), 0.0);
        let effects = s.apply(Input::EndCall, 0.0);
        assert_eq!(s.status, Status::EndingCall);
        assert!(effects.is_empty());
        let effects = s.apply(
            Input::Reply(Reply::StartPrivate(Ok("late".to_owned()))),
            0.0,
        );
        assert_eq!(
            effects,
            vec![Effect::Rpc(Rpc::EndPrivate {
                call_id: "late".to_owned()
            })]
        );
        s.apply(Input::Reply(Reply::EndPrivate(Ok(()))), 0.0);
        assert_eq!(s.status, Status::Disconnected);
    }

    #[test]
    fn a_second_ring_while_in_a_community_call_is_ignored() {
        let mut s = state();
        s.apply(Input::JoinCommunity("c1".to_owned()), 0.0);
        s.apply(
            Input::Reply(Reply::JoinCommunity(Ok("livekit:c".to_owned()))),
            0.0,
        );
        assert_eq!(s.status, Status::InCall);
        let mut requested = update("call-2", PrivateVoiceStatus::Requested);
        requested.caller = Some(format!("{:#x}", addr(3)));
        let effects = s.apply(Input::PrivateUpdate(requested), 0.0);
        assert!(effects.is_empty());
        assert_eq!(s.kind, Kind::Community);
        assert_eq!(s.status, Status::InCall);
    }

    #[test]
    fn a_community_stream_is_started_by_a_moderator_and_left_without_a_call() {
        let mut s = state();
        let effects = s.apply(Input::StartCommunity("c1".to_owned()), 0.0);
        assert_eq!(s.status, Status::StartingCall);
        assert_eq!(
            effects,
            vec![
                Effect::Sound(Sound::StartStream),
                Effect::Rpc(Rpc::StartCommunity {
                    community_id: "c1".to_owned()
                })
            ]
        );
        s.apply(
            Input::Reply(Reply::StartCommunity(Ok("livekit:c".to_owned()))),
            0.0,
        );
        assert_eq!(s.status, Status::InCall);
        assert!(s.local_speaker);
        let effects = s.apply(Input::RoomConnected, 0.0);
        assert_eq!(
            effects,
            vec![Effect::Sound(Sound::Join), Effect::SetCallMic(true)]
        );
        s.apply(Input::CallMicChanged(true), 0.0);
        let effects = s.apply(Input::EndCall, 0.0);
        assert_eq!(
            effects,
            vec![
                Effect::DespawnRoom,
                Effect::Sound(Sound::Leave),
                Effect::SetCallMic(false)
            ]
        );
        assert_eq!(s.status, Status::Disconnected);
    }

    #[test]
    fn a_listener_is_promoted_then_demoted_and_the_mic_follows() {
        let mut s = state();
        s.apply(Input::JoinCommunity("c1".to_owned()), 0.0);
        s.apply(
            Input::Reply(Reply::JoinCommunity(Ok("livekit:c".to_owned()))),
            0.0,
        );
        let listener = Metadata {
            role: "member".to_owned(),
            ..Default::default()
        };
        s.apply(
            Input::ParticipantJoined {
                address: me(),
                metadata: listener,
            },
            0.0,
        );
        let effects = s.apply(Input::RoomConnected, 0.0);
        assert_eq!(effects, vec![Effect::Sound(Sound::Join)]);
        assert!(!s.local_speaker);
        assert_eq!(s.local_role, Role::User);
        // a listener's mic never goes up
        assert!(s.apply(Input::SetMic(true), 0.0).is_empty());

        let speaker = Metadata {
            role: "member".to_owned(),
            is_speaker: true,
            ..Default::default()
        };
        let effects = s.apply(
            Input::ParticipantMetadata {
                address: me(),
                metadata: speaker,
            },
            0.0,
        );
        assert!(s.local_speaker);
        // promoted: the mic stays down until toggled
        assert!(effects.is_empty());
        let effects = s.apply(Input::SetMic(true), 0.0);
        assert_eq!(
            effects,
            vec![
                Effect::SetCallMic(true),
                Effect::Rpc(Rpc::MuteSpeaker {
                    community_id: "c1".to_owned(),
                    address: me(),
                    muted: false
                })
            ]
        );
        s.apply(Input::CallMicChanged(true), 0.0);

        let demoted = Metadata {
            role: "member".to_owned(),
            ..Default::default()
        };
        let effects = s.apply(
            Input::ParticipantMetadata {
                address: me(),
                metadata: demoted,
            },
            0.0,
        );
        assert!(!s.local_speaker);
        assert_eq!(effects, vec![Effect::SetCallMic(false)]);
    }

    #[test]
    fn a_raised_hand_is_heard_by_a_moderator_only() {
        let mut s = state();
        s.apply(Input::StartCommunity("c1".to_owned()), 0.0);
        s.apply(
            Input::Reply(Reply::StartCommunity(Ok("livekit:c".to_owned()))),
            0.0,
        );
        s.apply(
            Input::ParticipantJoined {
                address: me(),
                metadata: Metadata {
                    role: "owner".to_owned(),
                    is_speaker: true,
                    ..Default::default()
                },
            },
            0.0,
        );
        assert_eq!(s.local_role, Role::Owner);
        s.apply(
            Input::ParticipantJoined {
                address: addr(5),
                metadata: Metadata::default(),
            },
            0.0,
        );
        let effects = s.apply(
            Input::ParticipantMetadata {
                address: addr(5),
                metadata: Metadata {
                    is_requesting_to_speak: true,
                    ..Default::default()
                },
            },
            0.0,
        );
        assert_eq!(effects, vec![Effect::Sound(Sound::RaiseHand)]);
        assert!(s.participants[&addr(5)].is_requesting_to_speak);
        let effects = s.apply(Input::PromoteSpeaker(addr(5)), 0.0);
        assert_eq!(
            effects,
            vec![Effect::Rpc(Rpc::PromoteSpeaker {
                community_id: "c1".to_owned(),
                address: addr(5)
            })]
        );
    }

    #[test]
    fn a_kick_ends_the_call_and_a_blip_does_not() {
        let mut s = state();
        s.apply(Input::JoinCommunity("c1".to_owned()), 0.0);
        s.apply(
            Input::Reply(Reply::JoinCommunity(Ok("livekit:c".to_owned()))),
            0.0,
        );
        s.apply(Input::RoomConnected, 0.0);
        let effects = s.apply(
            Input::RoomDisconnected {
                terminal: false,
                reason: "SignalClose".to_owned(),
            },
            0.0,
        );
        assert!(effects.is_empty());
        assert_eq!(s.status, Status::InCall);
        assert!(!s.room_connected);
        let effects = s.apply(
            Input::RoomDisconnected {
                terminal: true,
                reason: "ParticipantRemoved".to_owned(),
            },
            0.0,
        );
        assert_eq!(
            effects,
            vec![Effect::DespawnRoom, Effect::Sound(Sound::Leave)]
        );
        assert_eq!(s.status, Status::Disconnected);
        assert_eq!(s.error.as_deref(), Some("ParticipantRemoved"));
    }

    #[test]
    fn the_stream_ending_drops_everyone() {
        let mut s = state();
        s.apply(Input::JoinCommunity("c1".to_owned()), 0.0);
        s.apply(
            Input::Reply(Reply::JoinCommunity(Ok("livekit:c".to_owned()))),
            0.0,
        );
        let effects = s.apply(
            Input::CommunityUpdate(CommunityVoiceUpdate {
                community_id: "c1".to_owned(),
                status: CommunityVoiceStatus::Ended,
                created_at: 0,
                ended_at: Some(1),
                positions: vec![],
                worlds: vec![],
                is_member: true,
                community_name: "c".to_owned(),
                community_image: None,
            }),
            0.0,
        );
        assert_eq!(
            effects,
            vec![
                Effect::DespawnRoom,
                Effect::Sound(Sound::Leave),
                Effect::Sound(Sound::EndStream)
            ]
        );
        assert_eq!(s.status, Status::Disconnected);
    }

    #[test]
    fn joining_a_stream_from_a_call_hangs_up_and_joins_a_second_later() {
        let mut s = state();
        ring(&mut s);
        let effects = s.apply(Input::JoinCommunity("c1".to_owned()), 10.0);
        assert_eq!(s.status, Status::EndingCall);
        assert!(effects.contains(&Effect::Rpc(Rpc::EndPrivate {
            call_id: "call-1".to_owned()
        })));
        s.apply(Input::Reply(Reply::EndPrivate(Ok(()))), 10.1);
        assert!(s.tick(10.5).is_empty());
        let effects = s.tick(11.0);
        assert_eq!(s.status, Status::StartingCall);
        assert_eq!(
            effects,
            vec![Effect::Rpc(Rpc::JoinCommunity {
                community_id: "c1".to_owned()
            })]
        );
    }

    #[test]
    fn the_social_link_dropping_while_ringing_is_an_error_and_reconnecting_clears_it() {
        let mut s = state();
        ring(&mut s);
        let effects = s.apply(Input::SocialDisconnected, 0.0);
        assert_eq!(s.status, Status::GenericError);
        assert_eq!(effects, vec![Effect::Sound(Sound::RingStop)]);
        s.apply(Input::SocialConnected, 0.0);
        assert_eq!(s.status, Status::Disconnected);

        let mut s = state();
        s.apply(Input::JoinCommunity("c1".to_owned()), 0.0);
        s.apply(
            Input::Reply(Reply::JoinCommunity(Ok("livekit:c".to_owned()))),
            0.0,
        );
        assert!(s.apply(Input::SocialDisconnected, 0.0).is_empty());
        assert_eq!(s.status, Status::InCall);
    }

    #[test]
    fn metadata_parses_the_gatekeeper_shape_and_tolerates_gaps() {
        let md = Metadata::parse(
            r#"{"role":"Moderator","isSpeaker":true,"muted":false,"name":"Foo","hasClaimedName":true,"profilePictureUrl":"p","isRequestingToSpeak":true}"#,
        );
        assert_eq!(Role::parse(&md.role), Role::Moderator);
        assert!(md.is_speaker && md.is_requesting_to_speak && md.has_claimed_name);
        assert_eq!(md.name, "Foo");
        let md = Metadata::parse(r#"{"role":"none"}"#);
        assert_eq!(
            md,
            Metadata {
                role: "none".to_owned(),
                ..Default::default()
            }
        );
        assert_eq!(Metadata::parse("not json"), Metadata::default());
        assert_eq!(Metadata::parse(""), Metadata::default());
    }
}
