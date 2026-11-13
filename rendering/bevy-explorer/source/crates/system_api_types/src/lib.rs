//! Plain serde data structs that cross the `~system/BevyExplorerApi` boundary.
//!
//! Split out of `system_bridge` so they can be compiled without bevy and fed to
//! `ts-rs` (scripts/gen-ts-bindings.sh) to generate the TypeScript the react-web
//! page and bridge scene consume. `system_bridge` re-exports everything here.

pub mod languages;
pub mod launch_options;
pub mod services;
pub mod web_params;

use dcl_component::proto_components::{
    common::{Color3, Vector2, Vector3},
    sdk::components::{pb_pointer_events, PbAvatarBase, PbAvatarEquippedData},
};
use serde::{Deserialize, Serialize};
use strum_macros::EnumIter;

/// The safe connectivity snapshot shared by the HUD and diagnostic console.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, ts_rs::TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ConnectivitySnapshot {
    pub connection_lost: bool,
    pub probing: bool,
    pub ever_connected: bool,
    pub suppressed: bool,
    pub rooms: Vec<ConnectivityRoomStatus>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ts_rs::TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ConnectivityRoomStatus {
    pub scope: String,
    pub state: String,
    pub retry_at: Option<f64>,
}

/// A Color3 with a `clear` flag, distinguishing "set this color" from "clear the
/// stored color" while remaining representable in JSON (where an absent field
/// already means "leave unchanged").
#[derive(Debug, Clone, Copy, Serialize, Deserialize, ts_rs::TS)]
pub struct ClearableColor3 {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub clear: bool,
}

impl ClearableColor3 {
    pub fn to_color3(self) -> Option<Color3> {
        if self.clear {
            None
        } else {
            Some(Color3 {
                r: self.r,
                g: self.g,
                b: self.b,
            })
        }
    }
}

/// A partial update to the local player's profile: every field is optional, and an omitted one is
/// left as it is. Applying it bumps the profile version and redeploys, so callers should send one
/// update per user-visible save rather than one per edited field.
#[derive(Serialize, Deserialize, Clone, Debug, ts_rs::TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct SetAvatarData {
    /// Body shape, colors and name. An empty `body_shape_urn` and absent colors are "unchanged",
    /// so a name-only edit needn't restate the avatar.
    #[ts(optional)]
    pub base: Option<PbAvatarBase>,
    #[ts(optional)]
    pub equip: Option<PbAvatarEquippedData>,
    #[ts(optional)]
    pub has_claimed_name: Option<bool>,
    /// Profile keys the renderer doesn't model (description, links, country, …), MERGED over the
    /// current set: omit a key to leave it alone, send `null` to remove it.
    #[ts(optional)]
    pub profile_extras: Option<std::collections::HashMap<String, serde_json::Value>>,
    #[ts(optional)]
    pub name_color: Option<ClearableColor3>,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
#[derive(ts_rs::TS)]
#[ts(export)]
pub struct LiveSceneInfo {
    pub hash: String,
    pub base_url: Option<String>,
    pub title: String,
    pub parcels: Vec<Vector2>,
    pub is_portable: bool,
    /// A portable backing an equipped or previewed smart wearable.
    pub is_smart_wearable: bool,
    pub is_broken: bool,
    pub is_blocked: bool,
    pub is_super: bool,
    pub sdk_version: String,
}

/// A portable the player can see and stop: every portable but startup ones and the HUD's own
/// scene, and the smart wearables stopped while still equipped.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, ts_rs::TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PortableInfo {
    /// The live scene hash, which `/stop_portable` takes. `None` for a smart wearable that is not
    /// loaded (stopped, or waiting for its wearable data).
    pub hash: Option<String>,
    /// A smart wearable's urn; a scene portable's urn or ens.
    pub pid: String,
    /// The wearable's name; a scene portable's title, else its ens or pid.
    pub name: String,
    pub kind: PortableInfoKind,
    pub state: PortableState,
    /// Why a portable failed, when known.
    pub reason: Option<String>,
    /// Smart wearables only: why the engine holds it.
    pub source: Option<SmartWearableSource>,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, ts_rs::TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum PortableInfoKind {
    Scene,
    SmartWearable,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, ts_rs::TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum PortableState {
    Loading,
    Running,
    Failed,
    /// Paused while the player stands in a scene with `portableExperiences: disabled`.
    Blocked,
    /// A smart wearable stopped while still equipped; it can be restarted.
    Stopped,
}

/// Why the engine holds a smart wearable: the profile equips it, a console `run`, or the realm's
/// `/preview-wearables`.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash, ts_rs::TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum SmartWearableSource {
    Equipped,
    Console,
    Preview,
}

#[derive(Serialize, Deserialize, Clone, Debug, ts_rs::TS)]
#[ts(export)]
pub struct HomeScene {
    pub realm: String,
    pub parcel: Vector2,
}

/// What the welcome page must show for the signed-in account.
#[derive(Serialize, Deserialize, Clone, Debug, ts_rs::TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct WelcomeState {
    /// The terms aren't accepted on this install.
    pub terms: bool,
    /// The account had no profile and signed in while the world was held: its new one deploys
    /// with the page's save (a setAvatar once the terms are accepted).
    pub new_profile: bool,
}

/// The satellite map tiles a realm advertises in its `/about` (`configurations.map.satelliteView`).
/// Only Genesis City realms carry one.
#[derive(Serialize, Deserialize, Clone, Debug, Default, ts_rs::TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export)]
pub struct SatelliteView {
    pub version: String,
    pub base_url: String,
    pub suffix_url: String,
    pub top_left_offset: Vector2,
}

#[derive(Clone, Serialize, Deserialize, ts_rs::TS)]
#[ts(export)]
pub struct ChatMessage {
    pub sender_address: String,
    pub message: String,
    pub channel: String,
    /// What reactions name this message by; empty for system messages.
    pub message_id: String,
}

/// A reaction added to or removed from a chat message, by the local user or a peer. `channel` is
/// "Nearby" or the DM partner's wallet.
#[derive(Clone, Serialize, Deserialize, ts_rs::TS)]
#[ts(export)]
pub struct ChatReactionData {
    pub channel: String,
    pub message_id: String,
    pub emoji: String,
    pub from: String,
    pub remove: bool,
}

#[derive(Clone, Serialize, Deserialize, ts_rs::TS)]
#[ts(export)]
pub struct VoiceMessage {
    pub sender_address: String,
    pub channel: String,
    pub active: bool,
}

/// Where the call stands.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[derive(ts_rs::TS)]
#[ts(export)]
pub enum VoiceChatStatus {
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

impl VoiceChatStatus {
    /// A new call may start.
    pub fn is_not_connected(self) -> bool {
        matches!(self, Self::Disconnected | Self::Busy | Self::GenericError)
    }
}

/// The kind of call (nearby voice is the existing, separate feature).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[derive(ts_rs::TS)]
#[ts(export)]
pub enum VoiceChatKind {
    #[default]
    None,
    Private,
    Community,
}

/// A participant's community role, from the participant metadata `role`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[derive(ts_rs::TS)]
#[ts(export)]
pub enum VoiceChatRole {
    #[default]
    None,
    User,
    Moderator,
    Owner,
}

impl VoiceChatRole {
    pub fn parse(role: &str) -> Self {
        match role.to_ascii_lowercase().as_str() {
            "user" | "member" => Self::User,
            "moderator" => Self::Moderator,
            "owner" => Self::Owner,
            _ => Self::None,
        }
    }

    /// A moderator or the owner.
    pub fn is_moderator(self) -> bool {
        matches!(self, Self::Moderator | Self::Owner)
    }
}

/// The call's LiveKit room.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[derive(ts_rs::TS)]
#[ts(export)]
pub enum VoiceChatConnection {
    #[default]
    None,
    Connecting,
    Connected,
    /// Dropped, and being re-dialled.
    Reconnecting,
}

/// A participant of the voice room the local user is in (a private call's peer, or a community
/// stream's speakers and listeners), as the gatekeeper's participant metadata describes them.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[derive(ts_rs::TS)]
#[ts(export)]
pub struct VoiceChatParticipant {
    pub address: String,
    pub name: String,
    pub has_claimed_name: bool,
    pub profile_picture_url: String,
    pub role: VoiceChatRole,
    pub is_speaker: bool,
    pub is_requesting_to_speak: bool,
    pub is_muted: bool,
    pub is_speaking: bool,
}

/// The voice chat the local user is in or being offered: one snapshot per change
/// (`GetVoiceChatStateStream`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[derive(ts_rs::TS)]
#[ts(export)]
pub struct VoiceChatState {
    pub status: VoiceChatStatus,
    pub kind: VoiceChatKind,
    /// The private call's id.
    pub call_id: Option<String>,
    /// The community whose stream this is.
    pub community_id: Option<String>,
    /// The other party of a private call.
    pub peer: Option<String>,
    pub connection: VoiceChatConnection,
    pub participants: Vec<VoiceChatParticipant>,
    /// The call microphone is publishing.
    pub mic_enabled: bool,
    /// The local user may publish (always in a private call; a community speaker).
    pub local_speaker: bool,
    /// The local user's role in the community.
    pub local_role: VoiceChatRole,
    pub local_requesting_to_speak: bool,
    /// Why the last action failed, why the call ended in busy / error, or how the call room
    /// went away.
    pub error: Option<String>,
    /// `error` tells how the call ended (hung up, removed, joined elsewhere), not a failure.
    pub error_is_notice: bool,
    /// Counts the errors set, so the same message twice in a row is two failures.
    pub error_seq: u32,
}

/// Whether a community voice chat opened or closed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[derive(ts_rs::TS)]
#[ts(export)]
pub enum CommunityVoiceStatus {
    Started,
    Ended,
}

/// A community voice chat started or ended (`GetCommunityVoiceUpdateStream`), as the social
/// service broadcasts it: to everyone for a public listed community, to members otherwise.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[derive(ts_rs::TS)]
#[ts(export)]
pub struct CommunityVoiceUpdate {
    pub community_id: String,
    pub status: CommunityVoiceStatus,
    pub created_at: i64,
    pub ended_at: Option<i64>,
    /// Parcels ("x,y") of the community's places, sent to non-members.
    pub positions: Vec<String>,
    /// World names of the community's places, sent to non-members.
    pub worlds: Vec<String>,
    pub is_member: bool,
    pub community_name: String,
    pub community_image: Option<String>,
}

/// What the HUD asks of the voice chat (`SystemApi::VoiceChatAction`); outcomes arrive on the
/// state stream.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "op")]
#[derive(ts_rs::TS)]
#[ts(export)]
pub enum VoiceChatAction {
    /// Ring a user (opens a private call).
    #[serde(rename_all = "camelCase")]
    StartPrivateCall {
        address: String,
    },
    AcceptCall,
    RejectCall,
    /// Hang up a private call, at any stage.
    EndCall,
    /// Open a community's stream (owner or moderator).
    #[serde(rename_all = "camelCase")]
    StartCommunityCall {
        community_id: String,
    },
    #[serde(rename_all = "camelCase")]
    JoinCommunityCall {
        community_id: String,
    },
    /// Leave the community stream (no server call).
    LeaveCall,
    /// Close the community's stream for everyone (owner or moderator).
    EndCommunityCall,
    /// Raise (`true`) or lower the hand in a community stream.
    #[serde(rename_all = "camelCase")]
    RequestToSpeak {
        raise: bool,
    },
    #[serde(rename_all = "camelCase")]
    PromoteSpeaker {
        address: String,
    },
    /// Demote a speaker; the local user's own address leaves the stage.
    #[serde(rename_all = "camelCase")]
    DemoteSpeaker {
        address: String,
    },
    #[serde(rename_all = "camelCase")]
    KickPlayer {
        address: String,
    },
    #[serde(rename_all = "camelCase")]
    RejectSpeakRequest {
        address: String,
    },
    /// Publish or stop publishing the call microphone.
    #[serde(rename_all = "camelCase")]
    SetMic {
        enabled: bool,
    },
    /// Clear a busy / error status.
    DismissError,
}

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
#[derive(ts_rs::TS)]
#[ts(export)]
pub struct HoverAction {
    #[serde(flatten)]
    pub event: pb_pointer_events::Entry,
    pub enabled: bool,
}

/// Classifier for pointer-target hits — distinguishes scene world geometry,
/// scene UI overlays, and avatars. The engine's pointer pipeline produces it
/// before any scene API surface is involved; `common::structs` re-exports it
/// for engine use, and `HoverEvent` carries the same value across the
/// system-api boundary.
#[derive(
    Hash,
    Clone,
    Copy,
    serde_repr::Serialize_repr,
    serde_repr::Deserialize_repr,
    Debug,
    PartialEq,
    Eq,
    Default,
)]
#[repr(u32)]
pub enum PointerTargetType {
    #[default]
    World = 0,
    Ui = 1,
    Avatar = 2,
}

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
#[derive(ts_rs::TS)]
#[ts(export)]
pub struct HoverEvent {
    pub entered: bool,
    #[ts(type = "number")]
    pub target_type: PointerTargetType,
    pub actions: Vec<HoverAction>,
}

/// Streamed to the system scene whenever an entity carrying PROXIMITY pointer
/// entries enters or leaves the avatar's interaction range, or when one of its
/// per-entry distance gates flips (so the `enabled` flag on an action changes).
/// `entity` is an opaque session-stable identifier so the scene can match
/// enter/leave pairs. `entity_position` is the world-space AABB centre of the
/// specific collider on the entity that produced the closest-point hit — for
/// multi-collider entities (e.g. GltfContainers) this anchors UI on the part
/// of the entity the player is actually nearest. The scene is responsible for
/// projecting it to screen space per frame; the active camera's vertical FOV
/// is available via `Runtime.getCameraFov`.
#[derive(Clone, Serialize, Deserialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
#[derive(ts_rs::TS)]
#[ts(export)]
pub struct ProximityEvent {
    pub entered: bool,
    pub entity: u32,
    pub entity_position: Vector3,
    pub actions: Vec<HoverAction>,
}

/// Photo mode as the HUD shows it: whether the in-world camera is open, what it is doing with
/// the last photo, and how much room the player has. `event_seq` increments on each `event`,
/// which carries the capture's preview (base64 JPEG) or the upload's failure message.
#[derive(Clone, Serialize, Deserialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
#[derive(ts_rs::TS)]
#[ts(export)]
pub struct PhotoModeState {
    pub active: bool,
    pub hud_hidden: bool,
    pub cursor_locked: bool,
    /// `idle`, `capturing`, `encoding` or `uploading`.
    pub phase: String,
    pub storage_current: u32,
    pub storage_max: u32,
    pub storage_loaded: bool,
    /// `none`, `captured`, `uploaded` or `uploadFailed`.
    pub event: String,
    pub event_seq: u32,
    pub preview_jpeg_base64: String,
    pub message: String,
}

/// A profile the engine holds was inserted or replaced: a foreign player's, or the local
/// player's own. Carries only the address and the new version; a consumer holding an older
/// version re-reads the profile itself.
#[derive(Clone, Serialize, Deserialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
#[derive(ts_rs::TS)]
#[ts(export)]
pub struct ProfileChangedEvent {
    pub address: String,
    pub version: u32,
}

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
#[derive(ts_rs::TS)]
#[ts(export)]
pub struct SceneLoadingUi {
    pub visible: bool,
    pub realm_connected: bool,
    pub title: String,
    pub pending_assets: Option<u32>,
}

/// One of the engine's own analytics events as it is queued for Segment (`crates/analytics`): its
/// Segment name and properties, without the common fields. The page's events, reported through
/// `/track`, are not echoed back. Events arrive as they happen, before the terms are accepted too
/// (Segment only holds them until then), so a consumer that forwards one off the page checks the
/// terms itself.
#[derive(Clone, Serialize, Deserialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
#[derive(ts_rs::TS)]
#[ts(export)]
pub struct AnalyticsEvent {
    pub event: String,
    pub properties: serde_json::Value,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
#[derive(ts_rs::TS)]
#[ts(export)]
pub struct AvatarModifierState {
    pub user_id: String,
    pub hide_avatar: bool,
    pub hide_profile: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[derive(ts_rs::TS)]
#[ts(export)]
pub struct NameColor {
    pub r: f32,
    pub g: f32,
    pub b: f32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[derive(ts_rs::TS)]
#[ts(export)]
pub struct FriendData {
    pub address: String,
    pub name: String,
    pub has_claimed_name: bool,
    pub profile_picture_url: String,
    pub name_color: Option<NameColor>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[derive(ts_rs::TS)]
#[ts(export)]
pub struct FriendRequestData {
    pub address: String,
    pub name: String,
    pub has_claimed_name: bool,
    pub profile_picture_url: String,
    pub name_color: Option<NameColor>,
    #[ts(type = "number")]
    pub created_at: i64,
    pub message: Option<String>,
    pub id: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all_fields = "camelCase")]
#[serde(tag = "type")]
#[derive(ts_rs::TS)]
#[ts(export)]
pub enum FriendshipEventUpdate {
    #[serde(rename = "request")]
    Request {
        address: String,
        name: String,
        has_claimed_name: bool,
        profile_picture_url: String,
        name_color: Option<NameColor>,
        #[ts(type = "number")]
        created_at: i64,
        message: Option<String>,
        id: String,
    },
    #[serde(rename = "accept")]
    Accept { address: String },
    #[serde(rename = "reject")]
    Reject { address: String },
    #[serde(rename = "cancel")]
    Cancel { address: String },
    #[serde(rename = "delete")]
    Delete { address: String },
    #[serde(rename = "block")]
    Block { address: String },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[derive(ts_rs::TS)]
#[ts(export)]
pub struct FriendStatusData {
    pub address: String,
    pub name: String,
    pub has_claimed_name: bool,
    pub profile_picture_url: String,
    pub name_color: Option<NameColor>,
    /// "online", "offline", or "away"
    pub status: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[derive(ts_rs::TS)]
#[ts(export)]
pub struct BlockedUserData {
    pub address: String,
    pub name: String,
    pub has_claimed_name: bool,
    pub profile_picture_url: String,
    pub name_color: Option<NameColor>,
}

/// Both directions of the blocking relationship for the local user,
/// addresses only (no profiles).
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[derive(ts_rs::TS)]
#[ts(export)]
pub struct BlockingStatusData {
    pub blocked_users: Vec<String>,
    pub blocked_by_users: Vec<String>,
}

/// Emitted when another user blocks / unblocks the local user.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[derive(ts_rs::TS)]
#[ts(export)]
pub struct BlockUpdateData {
    pub address: String,
    pub is_blocked: bool,
}

/// One stored DM; `from` is the local user's wallet or the partner's.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[derive(ts_rs::TS)]
#[ts(export)]
pub struct DmHistoryEntryData {
    pub from: String,
    pub message: String,
    /// Unix seconds when the local client stored it.
    pub received_at: f64,
    /// What reactions name this DM by; empty for DMs stored before reactions.
    pub message_id: String,
    /// Its reactions, in the order each emoji was first used.
    pub reactions: Vec<DmReactionData>,
}

/// One emoji's reactions to a stored DM.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[derive(ts_rs::TS)]
#[ts(export)]
pub struct DmReactionData {
    pub emoji: String,
    /// The wallets that reacted with it.
    pub from: Vec<String>,
}

/// A community member came online or went offline (the social service's
/// `SubscribeToCommunityMemberConnectivityUpdates`; `away` is never relayed). The local user's own
/// updates tell which communities they are in.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[derive(ts_rs::TS)]
#[ts(export)]
pub struct CommunityMemberConnectivityData {
    pub community_id: String,
    pub address: String,
    /// "online" or "offline"
    pub status: String,
}

/// Whether the local user can DM `address`, and why not; emitted for the watched recipient.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[derive(ts_rs::TS)]
#[ts(export)]
pub struct DmUserStateData {
    pub address: String,
    /// "notConnected" (the local user is not in the private chat room), "connected",
    /// "blockedByOwnUser", "privateMessagesBlockedByOwnUser", "privateMessagesBlocked",
    /// "disconnected" or "otherClient"; for a community channel (`community:<id>`),
    /// "notConnected", "noRouter" (no message router in the room) or "connected"
    pub state: String,
    /// In the private chat room and not blocked by the local user.
    pub online: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[derive(ts_rs::TS)]
#[ts(export)]
pub struct FriendConnectivityEvent {
    pub address: String,
    pub name: String,
    pub has_claimed_name: bool,
    pub profile_picture_url: String,
    pub name_color: Option<NameColor>,
    /// "online", "offline", or "away"
    pub status: String,
}

/// The engine needs a password to enter a world (see `ipfs::world_access`); answered with
/// `SetWorldPassword`.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[derive(ts_rs::TS)]
#[ts(export)]
pub struct WorldPasswordRequest {
    /// The world's name, e.g. `my-world.dcl.eth`
    pub world: String,
    /// Why the previous answer was refused (a wrong password, too many attempts), to show with
    /// the prompt; absent on the first ask
    pub error: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
#[derive(ts_rs::TS)]
#[ts(export)]
pub enum WorldAccessEvent {
    #[serde(rename = "passwordRequest")]
    PasswordRequest(WorldPasswordRequest),
    /// The prompt no longer needs an answer: the realm change it was for is over
    #[serde(rename = "cancelled")]
    Cancelled,
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Hash, Debug, ts_rs::TS)]
pub enum PermissionValue {
    Allow,
    Deny,
    Ask,
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Hash, Debug, EnumIter, ts_rs::TS)]
pub enum PermissionType {
    MovePlayer,
    ForceCamera,
    PlayEmote,
    SetLocomotion,
    HideAvatarsNametags,
    DisableVoice,
    Teleport,
    ChangeRealm,
    SpawnPortable,
    KillPortables,
    Web3,
    CopyToClipboard,
    Fetch,
    Websocket,
    OpenUrl,
    OpenExplorerUi,
}

#[derive(Clone, Debug, Serialize, Deserialize, ts_rs::TS)]
pub enum PermissionLevel {
    Scene(String),
    Realm(String),
    Global,
}

#[derive(Serialize, Deserialize, Clone, ts_rs::TS)]
#[ts(export)]
pub struct PermanentPermissionItem {
    pub ty: PermissionType,
    pub allow: PermissionValue,
}

#[derive(Clone, Serialize, Deserialize, ts_rs::TS)]
#[ts(export)]
pub struct PermissionRequest {
    pub ty: PermissionType,
    pub additional: Option<String>,
    pub scene: String,
    pub id: usize,
}

#[derive(Clone, Serialize, Deserialize, ts_rs::TS)]
#[serde(tag = "type")]
#[ts(export)]
pub enum PermissionRequestEvent {
    #[serde(rename = "request")]
    Request(PermissionRequest),
    // a streamed request no longer needs a decision: the engine resolved it itself (e.g. a
    // permanent rule set for an earlier request now covers it, or the scene went away)
    #[serde(rename = "cancelled")]
    Cancelled { id: usize },
}

#[derive(Clone, Debug, Serialize, Deserialize, ts_rs::TS)]
#[ts(export)]
pub struct SetSinglePermission {
    pub id: usize,
    pub allow: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, ts_rs::TS)]
#[ts(export)]
pub struct SetPermanentPermission {
    pub ty: PermissionType,
    pub level: PermissionLevel,
    pub allow: Option<PermissionValue>,
}

#[derive(Serialize, Deserialize, Clone, ts_rs::TS)]
#[ts(export)]
pub struct NamedVariant {
    pub name: String,
    pub description: String,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
#[derive(ts_rs::TS)]
#[ts(export)]
pub struct SettingInfo {
    pub name: String,
    pub category: String,
    pub description: String,
    pub min_value: f32,
    pub max_value: f32,
    pub named_variants: Vec<NamedVariant>,
    pub step_size: f32,
    pub value: f32,
    pub default: f32,
}

/// A color as the catalyst `outfits` entity stores it. Unity's outfits reader takes a missing `a` as 0.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, ts_rs::TS)]
#[ts(export)]
pub struct OutfitColor {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    #[serde(default = "opaque")]
    pub a: f32,
}

fn opaque() -> f32 {
    1.0
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, ts_rs::TS)]
#[ts(export)]
pub struct OutfitColored {
    pub color: OutfitColor,
}

/// One saved look, as the catalyst `outfits` entity stores it.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, ts_rs::TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct OutfitData {
    pub body_shape: String,
    pub eyes: OutfitColored,
    pub hair: OutfitColored,
    pub skin: OutfitColored,
    pub wearables: Vec<String>,
    #[serde(default)]
    pub force_render: Vec<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, ts_rs::TS)]
#[ts(export)]
pub struct OutfitSlotData {
    pub slot: u32,
    pub outfit: OutfitData,
}

/// The local player's saved outfits: the metadata of their catalyst `outfits` entity. Fields
/// other explorers add are dropped on the next deploy.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, ts_rs::TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct OutfitsData {
    pub outfits: Vec<OutfitSlotData>,
    pub names_for_extra_slots: Vec<String>,
}

/// A saved outfit's picture (PNG data URL), kept by the engine on this device.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, ts_rs::TS)]
#[ts(export)]
pub struct OutfitThumbnailData {
    pub slot: u32,
    pub thumbnail: String,
}
