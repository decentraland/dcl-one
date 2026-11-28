use std::{
    f32::consts::PI,
    marker::PhantomData,
    num::ParseIntError,
    ops::{Deref, Range, RangeInclusive},
    str::FromStr,
    sync::{atomic::AtomicU32, Arc},
};

use alloy_core::primitives::Address;
use bevy::{
    color::palettes,
    math::DVec3,
    pbr::CascadeShadowConfig,
    platform::collections::{HashMap, HashSet},
    prelude::*,
    render::{primitives::Aabb, view::RenderLayers},
};
use dcl_component::proto_components::{
    sdk::{
        components::common::CameraTransition,
        development::{ws_scene_message, UpdateModelType},
    },
    social_service::v2::PrivateMessagePrivacySetting,
};
pub use platform::{
    default_color_grading, scatter_bloom, scatter_lerp, BLOOM_SCATTER, GRADING_MIDTONES_RANGE,
    GRADING_SHADOWS_WEIGHT,
};
use serde::{Deserialize, Serialize};
use system_api_types::SatelliteView;
pub use system_api_types::{PermissionLevel, PermissionType, PermissionValue, PointerTargetType};
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

use crate::inputs::{Action, InputIdentifier, InputMapSerialized, SystemAction};

#[derive(Resource)]
pub struct Version(pub String);

// main user entity
#[derive(Component, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct PrimaryUser {
    pub walk_speed: f32,
    pub jog_speed: f32,
    pub run_speed: f32,
    pub jump_height: f32,
    pub run_jump_height: f32,
    pub block_all: bool,
    pub block_run: bool,
    pub block_walk: bool,
    pub block_jump: bool,
    pub block_emote: bool,
}

impl Default for PrimaryUser {
    fn default() -> Self {
        Self {
            walk_speed: 1.5,
            jog_speed: 8.0,
            run_speed: 10.0,
            jump_height: 1.0,
            run_jump_height: 1.5,
            block_all: false,
            block_run: false,
            block_walk: false,
            block_jump: false,
            block_emote: false,
        }
    }
}

/// A player's wallet address: on every peer from its spawn, and on the local player while a
/// profile is set (not between login and the profile being set, nor in server mode).
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct PlayerAddress(pub Address);

/// The walk, jog and run speeds (m/s) the primary user's locomotion currently allows, after area
/// modifiers and the active scene locomotion settings. Written by the movement plugin; read to
/// express the player's speed as a gait.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct LocomotionSpeeds {
    pub walk: f32,
    pub jog: f32,
    pub run: f32,
}

impl Default for LocomotionSpeeds {
    fn default() -> Self {
        let user = PrimaryUser::default();
        Self {
            walk: user.walk_speed,
            jog: user.jog_speed,
            run: user.run_speed,
        }
    }
}

#[derive(Component, Default, Debug)]
pub struct PlayerModifiers {
    pub hide: bool,
    pub hide_profile: bool,
    pub walk_speed: Option<f32>,
    pub run_speed: Option<f32>,
    pub friction: Option<f32>,
    pub gravity: Option<f32>,
    pub jump_height: Option<f32>,
    pub fall_speed: Option<f32>,
    pub control_type: Option<AvatarControl>,
    pub turn_speed: Option<f32>,
    pub block_all: bool,
    pub block_run: bool,
    pub block_walk: bool,
    pub block_jump: bool,
    pub block_emote: bool,
    pub areas: Vec<ActiveAvatarArea>,
    /// The scenes whose areas hide the primary player's avatar, passport or nametag now.
    pub hide_scenes: Vec<Entity>,
    /// The scenes whose areas or input modifiers change or block the primary player's movement now.
    pub locomotion_scenes: Vec<Entity>,
}

#[derive(Clone, Debug)]
pub struct ActiveAvatarArea {
    pub entity: Entity,
    pub allow_locomotion: PermissionState,
}

impl PlayerModifiers {
    pub fn combine(&self, user: &PrimaryUser) -> PrimaryUser {
        PrimaryUser {
            walk_speed: self.walk_speed.unwrap_or(user.walk_speed),
            jog_speed: self.run_speed.unwrap_or(user.jog_speed),
            run_speed: self.run_speed.unwrap_or(user.run_speed),
            jump_height: self.jump_height.unwrap_or(user.jump_height),
            run_jump_height: self.jump_height.unwrap_or(user.run_jump_height),
            block_all: self.block_all || user.block_all,
            block_run: self.block_run || user.block_run,
            block_walk: self.block_walk || user.block_walk,
            block_jump: self.block_jump || user.block_jump,
            block_emote: self.block_emote || user.block_emote,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PermissionState {
    Resolved(bool),
    NotRequested,
    Pending,
}

// attachment points for local or foreign players
#[derive(Component)]
pub struct AttachPoints {
    pub position: Entity,
    pub nametag: Entity,
    pub head: Entity,
    pub neck: Entity,
    pub spine: Entity,
    pub spine_1: Entity,
    pub spine_2: Entity,
    pub hip: Entity,
    pub left_shoulder: Entity,
    pub left_arm: Entity,
    pub left_forearm: Entity,
    pub left_hand: Entity,
    pub left_hand_index: Entity,
    pub right_shoulder: Entity,
    pub righ_arm: Entity,
    pub right_forearm: Entity,
    pub right_hand: Entity,
    pub right_hand_index: Entity,
    /// AAPT_LEFT_UP_LEG
    pub left_thigh: Entity,
    /// AAPT_LEFT_LEG
    pub left_shin: Entity,
    pub left_foot: Entity,
    pub left_toe_base: Entity,
    /// AAPT_RIGHT_UP_LEG
    pub right_thigh: Entity,
    /// AAPT_RIGHT_LEG
    pub right_shin: Entity,
    pub right_foot: Entity,
    pub right_toe_base: Entity,
}

impl AttachPoints {
    pub fn new(commands: &mut Commands) -> Self {
        let inverted_transform = Transform::from_rotation(Quat::from_rotation_y(PI));
        let default_visibility = Visibility::default();
        let default_bundle = (inverted_transform, default_visibility);
        Self {
            position: commands
                .spawn((Transform::default(), Visibility::default()))
                .id(),
            nametag: commands
                .spawn((
                    Transform::from_translation(Vec3::Y * 2.2),
                    Visibility::default(),
                ))
                .id(),
            head: commands
                .spawn((
                    default_bundle,
                    // This Aabb roughly encloses the head
                    Aabb::from_min_max(Vec3::new(-16., -21., -22.), Vec3::new(16., 21., 22.)),
                ))
                .id(),
            neck: commands.spawn(default_bundle).id(),
            spine: commands.spawn(default_bundle).id(),
            spine_1: commands.spawn(default_bundle).id(),
            spine_2: commands.spawn(default_bundle).id(),
            hip: commands.spawn(default_bundle).id(),
            left_shoulder: commands.spawn(default_bundle).id(),
            left_arm: commands.spawn(default_bundle).id(),
            left_forearm: commands.spawn(default_bundle).id(),
            left_hand: commands.spawn(default_bundle).id(),
            left_hand_index: commands.spawn(default_bundle).id(),
            right_shoulder: commands.spawn(default_bundle).id(),
            righ_arm: commands.spawn(default_bundle).id(),
            right_forearm: commands.spawn(default_bundle).id(),
            right_hand: commands.spawn(default_bundle).id(),
            right_hand_index: commands.spawn(default_bundle).id(),
            left_thigh: commands.spawn(default_bundle).id(),
            left_shin: commands.spawn(default_bundle).id(),
            left_foot: commands.spawn(default_bundle).id(),
            left_toe_base: commands.spawn(default_bundle).id(),
            right_thigh: commands.spawn(default_bundle).id(),
            right_shin: commands.spawn(default_bundle).id(),
            right_foot: commands.spawn(default_bundle).id(),
            right_toe_base: commands.spawn(default_bundle).id(),
        }
    }

    /// AttachPoints entities ordered by their protocol id
    pub fn entities(&self) -> [Entity; 26] {
        [
            self.position,
            self.nametag,
            self.left_hand,
            self.right_hand,
            self.head,
            self.neck,
            self.spine,
            self.spine_1,
            self.spine_2,
            self.hip,
            self.left_shoulder,
            self.left_arm,
            self.left_forearm,
            self.left_hand_index,
            self.right_shoulder,
            self.righ_arm,
            self.right_forearm,
            self.right_hand_index,
            self.left_thigh,
            self.left_shin,
            self.left_foot,
            self.left_toe_base,
            self.right_thigh,
            self.right_shin,
            self.right_foot,
            self.right_toe_base,
        ]
    }
}

/// Which bones an emote drives. Upper body = the `Avatar_Spine` subtree; hips and legs stay with
/// locomotion so the player keeps walking. Scene-only (`triggerEmote` / `triggerSceneEmote` with
/// `AvatarMask.AM_UPPER_BODY`); the wheel is always full body.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum EmoteMask {
    #[default]
    FullBody,
    UpperBody,
}

impl EmoteMask {
    /// The wire enum (rfc4 / Pulse / `AvatarEmoteCommand`): `0`/absent full body, `1` upper body.
    /// Not the sdk's `AvatarMask`, whose only value `AM_UPPER_BODY` is `0`.
    pub fn to_wire(self) -> Option<i32> {
        match self {
            EmoteMask::FullBody => None,
            EmoteMask::UpperBody => Some(1),
        }
    }

    pub fn from_wire(mask: Option<i32>) -> Self {
        match mask {
            Some(1) => EmoteMask::UpperBody,
            _ => EmoteMask::FullBody,
        }
    }
}

#[derive(Component, Clone, Debug, PartialEq, Default)]
pub struct EmoteCommand {
    pub urn: String,
    pub timestamp: i64,
    pub r#loop: bool,
    pub mask: EmoteMask,
}

/// The local player triggered an emote: `source` is `self` (the HUD, the console) or `scene`,
/// the `used_emote` source values.
#[derive(Event, Clone, Debug)]
pub struct PlayerEmoteTriggered {
    pub urn: String,
    pub source: &'static str,
}

/// The local player reacted with an emoji over their avatar: `emoji_index` is the emoji's index
/// on the wire, the `react_situation` reaction id.
#[derive(Event, Clone, Copy, Debug, PartialEq, Eq)]
pub struct PlayerReacted {
    pub emoji_index: i32,
}

/// A transition in an avatar's triggered-emote playback, reported to scenes as an
/// `AvatarEmoteCommand` entry by `avatar::emote_report`. Raised by `comms` from the wire (in wire
/// order, so client and server report the same sequence) and by the avatar animator from
/// playback; the reporter keeps the wire's word for foreign players and playback's for the rest.
#[derive(Event, Clone, Debug)]
pub struct EmoteLifecycleEvent {
    pub avatar: Entity,
    pub event: EmoteLifecycle,
    pub source: EmoteLifecycleSource,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EmoteLifecycleSource {
    Playback,
    Wire,
}

/// One emote plays at a time, whatever its slot, so an end names no emote: it ends the last start.
#[derive(Clone, Debug)]
pub enum EmoteLifecycle {
    /// `r#loop` is the flag known at trigger time; the emote's own metadata may still make it loop.
    /// `mask` is the slot (full body / upper body) it plays in.
    Started {
        urn: String,
        r#loop: bool,
        mask: EmoteMask,
    },
    /// A one-shot ran to its end.
    Finished,
    /// Playback was cut short: movement, a scene animation, a stop from the wire.
    Interrupted,
}

// Current scene-driven movement animation request for a player avatar. For the
// primary player, written by the bridge system in `user_input` (after resolving
// the scene-relative path against the active scene's content map). For foreign
// players, written by the comms crate when a Movement packet with anim fields
// arrives. Read by `animate` in the avatar crate uniformly for both.
#[derive(Component, Default, Clone, Debug, PartialEq)]
pub struct SceneDrivenAnim {
    pub active: Option<SceneDrivenAnimationRequest>,
}

/// Build the render-only avatar tilt (lean) quaternion from pitch/roll in degrees.
/// Composed as `Y * X * Z`, so `Quat::from_rotation_y(yaw) * avatar_tilt_quat(p, r)`
/// equals `Quat::from_euler(EulerRot::YXZ, yaw, p_rad, r_rad)` — letting consumers recover
/// the authoritative yaw with `to_euler(EulerRot::YXZ).0` even when tilt is present.
pub fn avatar_tilt_quat(pitch_deg: f32, roll_deg: f32) -> bevy::math::Quat {
    bevy::math::Quat::from_euler(
        bevy::math::EulerRot::YXZ,
        0.0,
        pitch_deg.to_radians(),
        roll_deg.to_radians(),
    )
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct SceneDrivenAnimationRequest {
    // scene-relative path (e.g. "assets/walk.glb") — used for feedback to the controlling
    // scene. Empty for remote requests received over the network.
    pub src: String,
    // Pre-built scene-emote URN encoding scene_hash + content_hash. For local requests
    // this is constructed by user_input; for remote requests it's reassembled from the
    // hash pair received on the wire.
    pub urn: String,
    // The two hashes that compose the URN. Kept alongside `urn` so the broadcaster can
    // ship them to remotes without repeating the fixed URN preamble on every packet.
    pub scene_hash: String,
    pub content_hash: String,
    pub r#loop: bool,
    pub speed: f32,
    pub idle: bool,
    pub transition_seconds: f32,
    pub seek: Option<f32>,
    // Scene-requested avatar-bus sound clips to play this update. Each entry is the
    // content_hash of a file hosted in the same scene as the animation. The consumer
    // dedups per-avatar so leaving identical entries across consecutive updates doesn't
    // re-fire; the scene clears the list on frames it doesn't want sound.
    pub sounds: Vec<String>,
    // The scene is a startup portable, as the movement scene is, so the clip's file name says the
    // avatar's motion (`ClipMotion::of_clip`). False for remote requests.
    pub movement_scene: bool,
}

// Current scene-driven animation playback state, written by `play_current_emote` in
// the avatar crate and mirrored into `AvatarMovementInfo.active_animation_state` by
// `broadcast_movement_info` in `user_input`.
// `playback_time` freezes while a triggerSceneEmote overrides the animation.
#[derive(Resource, Default)]
pub struct SceneDrivenAnimationFeedback {
    pub state: Option<SceneDrivenAnimationFeedbackState>,
}

/// Marker: present when the app has no render app. Render-only scene plugins are then
/// skipped — nothing displays their output and none of them feed results back to the
/// scene, so their components stay in the scene-side filtered store and never cross the
/// IPC boundary. Insert it before `SceneRunnerPlugin`, which reads it at build time.
///
/// Absence is the normal case, so nothing needs to opt in and no default can be wrong.
///
/// Narrower than "headless": `impost.rs` disables winit and runs with no primary window,
/// but keeps `DefaultPlugins` and a camera to bake imposters, so it must NOT insert this.
/// Doing so would strip every imposter texture with no panic and no test failure.
///
/// Also distinct from [`server_mode`]: the headless binary never has a render app, but
/// is only in server mode when run with --server-mode.
#[derive(Resource)]
pub struct NoRenderApp;

static SERVER_MODE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Latch this process as an authoritative scene server: scenes run in server role and
/// `isServer()` returns true to scene JS. Set by the headless server binary before the
/// app is built; irreversible by design.
pub fn set_server_mode() {
    SERVER_MODE.store(true, std::sync::atomic::Ordering::Relaxed);
}

/// True once [`set_server_mode`] has been called.
pub fn server_mode() -> bool {
    SERVER_MODE.load(std::sync::atomic::Ordering::Relaxed)
}

static MULTI_TENANT: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Latch this process as a multi-tenant (orchestrated) scene server: every scene has its
/// own room's crdt context, registered before the scene is queued. A standalone server
/// (`--server-mode` without `--orchestrated`) is NOT multi-tenant — its single scene
/// legitimately rides the shared context. Irreversible like [`set_server_mode`].
pub fn set_multi_tenant() {
    MULTI_TENANT.store(true, std::sync::atomic::Ordering::Relaxed);
}

/// True once [`set_multi_tenant`] has been called.
pub fn multi_tenant() -> bool {
    MULTI_TENANT.load(std::sync::atomic::Ordering::Relaxed)
}

#[derive(Debug, Clone)]
pub struct SceneDrivenAnimationFeedbackState {
    pub src: String,
    pub r#loop: bool,
    pub speed: f32,
    pub idle: bool,
    pub playback_time: f32,
    pub duration: f32,
    pub loop_count: u32,
}

/// vertical fov of the player camera, in radians (60 degrees). also the
/// `PBVirtualCamera.fov` default, per the proto definition.
pub const PLAYER_CAMERA_FOV: f32 = std::f32::consts::PI / 3.0;

/// The player camera's zoom level. The wheel steps between the three; there is no zoom within one.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CameraZoomLevel {
    FirstPerson,
    #[default]
    ThirdPerson,
    Drone,
}

/// Which shoulder the third-person and drone cameras look over while the cursor is locked.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CameraShoulder {
    #[default]
    Right,
    Left,
}

// main camera entity
#[derive(Component)]
pub struct PrimaryCamera {
    // settings
    /// The player's own zoom level; a scene's override wins while it is set.
    pub level: CameraZoomLevel,
    pub shoulder: CameraShoulder,
    pub sensitivity: f32,
    // impl details (todo: move to separate private struct)
    pub initialized: bool,
    pub yaw: f32,
    pub pitch: f32,
    pub roll: f32,
    // override
    pub scene_override: Option<CameraOverride>,
    /// The scene whose camera mode area or virtual camera set `scene_override`.
    pub scene_override_owner: Option<Entity>,
}

/// Where the camera sits relative to the player, with a scene's override applied.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CameraView {
    FirstPerson,
    ThirdPerson,
    Cinematic,
}

impl PrimaryCamera {
    /// The zoom level in effect: a scene's override wins over the player's own. None while a
    /// scene's cinematic camera has the view.
    pub fn level_in_effect(&self) -> Option<CameraZoomLevel> {
        match self.scene_override {
            Some(CameraOverride::Cinematic(_)) => None,
            Some(CameraOverride::Level(level)) => Some(level),
            None => Some(self.level),
        }
    }

    /// The view in effect, as scenes are told it: the drone level is third person to them.
    pub fn view(&self) -> CameraView {
        match self.level_in_effect() {
            None => CameraView::Cinematic,
            Some(CameraZoomLevel::FirstPerson) => CameraView::FirstPerson,
            Some(CameraZoomLevel::ThirdPerson | CameraZoomLevel::Drone) => CameraView::ThirdPerson,
        }
    }
}

#[derive(Clone, Debug)]
pub struct CinematicSettings {
    pub origin: Entity,
    pub allow_manual_rotation: bool,
    pub yaw_range: Option<f32>,
    pub pitch_range: Option<f32>,
    pub roll_range: Option<f32>,
    pub look_at_entity: Option<Entity>,
    pub transition: Option<CameraTransition>,
    /// vertical fov, in radians
    pub fov: f32,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum AvatarControl {
    None,
    Relative,
    Tank,
}

#[derive(Clone, Debug)]
pub enum CameraOverride {
    Level(CameraZoomLevel),
    Cinematic(CinematicSettings),
}

impl CameraOverride {
    pub fn effectively_equals(&self, other: &CameraOverride) -> bool {
        match (self, other) {
            (CameraOverride::Level(x), CameraOverride::Level(y)) => x == y,
            (CameraOverride::Cinematic(c0), CameraOverride::Cinematic(c1)) => {
                c0.origin == c1.origin
            }
            _ => false,
        }
    }
}

impl Default for PrimaryCamera {
    fn default() -> Self {
        Self {
            sensitivity: 5.0,
            initialized: Default::default(),
            yaw: Default::default(),
            pitch: Default::default(),
            roll: Default::default(),
            level: Default::default(),
            shoulder: Default::default(),
            scene_override: None,
            scene_override_owner: None,
        }
    }
}

#[derive(Resource)]
pub struct PrimaryPlayerRes(pub Entity);

#[derive(Resource)]
pub struct PrimaryCameraRes(pub Entity);

/// A camera that draws the lens flare of the directional light (`visuals::lens_flare`).
#[derive(Component, Clone, Copy, Default)]
pub struct LensFlare;

/// World-space head gaze angles, in degrees. On the local player, written each frame
/// from the real camera (and zeroed/disabled when a scene-driven camera is active). On
/// remote players, written from the incoming rfc4::Movement head-sync fields. The
/// `*_enabled` flags drive the receive-side IK weight crossfade and gate the broadcast
/// of valid angles for the local player.
#[derive(Component, Default, Clone, Copy)]
pub struct HeadSync {
    pub yaw_deg: f32,
    pub pitch_deg: f32,
    pub yaw_enabled: bool,
    pub pitch_enabled: bool,
}

/// World-space point-at state. On the local player, populated when the PointAt
/// action fires and `WorldPointerTarget` has a hit; cleared when the latch
/// expires. On remote players, populated from the incoming rfc4::Movement
/// `point_at_*` / `is_pointing_at` fields. Coordinates are stored in DCL
/// convention (Z mirrored from bevy world) so the same value flows over the
/// wire and into the IK apply step without a per-site flip.
#[derive(Component, Default, Clone, Copy)]
pub struct PointAtSync {
    pub target_world: Vec3,
    pub is_pointing: bool,
}

// marker for the root ui component (full screen, used for checking pointer/mouse button events are not intercepted by any other ui component)
#[derive(Component)]
pub struct UiRoot;

#[derive(PartialEq, Eq, Hash, PartialOrd, Ord, Clone, Copy)]
pub enum TooltipSource {
    Label(&'static str),
    Entity(Entity),
}

#[derive(Resource, Default)]
pub struct ToolTips(pub HashMap<TooltipSource, Vec<(String, bool)>>);

// web3 authorization chain link
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ChainLink {
    #[serde(rename = "type")]
    pub ty: String,
    pub payload: String,
    pub signature: String,
}

// ephemeral identity info
#[derive(Serialize, Deserialize, Clone)]
pub struct PreviousLogin {
    pub root_address: Address,
    pub ephemeral_key: Vec<u8>,
    pub auth: Vec<ChainLink>,
    #[serde(default)]
    pub guest_account: bool,
}

pub fn default_home_realm() -> String {
    crate::base_domain::url(crate::base_domain::Service::RealmProvider, "/main")
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct PinnedHome {
    pub realm: String,
    pub location: IVec2,
}

/// The key of an account's entry in [`AppConfig::homes`].
pub fn home_account(address: &Address) -> String {
    format!("{address:#x}")
}

/// The values the "Live Scene Ring" setting takes, in parcel rings.
pub const LIVE_SCENE_RINGS: RangeInclusive<u16> = 0..=3;
/// The values the "Near LOD Ring" setting takes, in parcel rings.
pub const NEAR_LOD_RINGS: RangeInclusive<u16> = 1..=8;
/// The values the "Far LOD Ring" setting takes, in parcel rings.
pub const FAR_LOD_RINGS: RangeInclusive<u16> = 5..=40;

// app configuration
#[derive(Serialize, Deserialize, Resource, Clone)]
#[serde(default)]
pub struct AppConfig {
    /// A home pinned for no account: the first account to sign in adopts it
    /// ([`Self::adopt_home`]), and the headless server boots from it. --realm / --position are
    /// startup params (like the web's ?realm= / ?position=) and are deliberately never merged
    /// in here — the config file is rewritten wholesale on any settings change, which would
    /// silently persist a one-off CLI target as home.
    pub home_realm: Option<String>,
    pub home_location: Option<IVec2>,
    /// The home each account pinned, by [`home_account`]. An account with no entry has no home,
    /// and starts at the base-domain-derived default realm, on 0,0. A pinned home persists (and
    /// survives switching base domains) even when it happens to equal some domain's default.
    pub homes: HashMap<String, PinnedHome>,
    /// The account a home was last read for: the one whose home applies at startup when no
    /// sign-in is stored.
    pub home_last_account: Option<String>,
    pub previous_login: Option<PreviousLogin>,
    /// The root key of the benchmark autopilot's local guest (`wallet::local_guest`): it signs each
    /// ephemeral identity, so an expired one is signed again for the same account. Nothing but the
    /// autopilot reads it, and a person signing in leaves it: the guest keeps its address.
    pub guest_root_key: Option<Vec<u8>>,
    /// The [`TERMS_VERSION`] last accepted on this install, on the welcome page.
    pub accepted_terms: Option<u32>,
    pub graphics: GraphicsSettings,
    pub audio: AudioSettings,
    pub cache_bytes: u64,
    pub scene_threads: usize,
    pub scene_load_distance: f32,
    pub scene_unload_extra_distance: f32,
    /// How many loading scenes may draw a near LOD (their own models) at once, in view and
    /// behind the camera together: the nearest in view start first, and with none free one
    /// behind the camera makes way for one in view. A near LOD's colliders take no place.
    pub max_near_lods: usize,
    /// How many scenes may load or show a far LOD (the asset pipeline's simplified mesh) at once,
    /// in view and behind the camera together, as `max_near_lods` counts near LODs.
    pub max_far_lods: usize,
    /// Scenes within this Chebyshev parcel ring of the player's parcel go live whatever the load
    /// distance, and are kept one ring further. Within [`LIVE_SCENE_RINGS`].
    pub live_scene_ring: u16,
    /// A scene that is not live starts a near LOD within this ring, kept one ring further; far
    /// LODs start past that. Within [`NEAR_LOD_RINGS`].
    pub near_lod_ring: u16,
    /// The outer bound of far LODs; the imposter tiles may end them sooner. Within
    /// [`FAR_LOD_RINGS`].
    pub far_lod_ring: u16,
    pub scene_imposter_distances: Vec<f32>,
    pub scene_imposter_multisample: bool,
    pub scene_imposter_multisample_amount: f32,
    pub scene_imposter_bake: SceneImposterBake,
    pub parcel_grass_setting: ParcelGrassSetting,
    /// The landscape distance in metres ([`LandscapeDistance`]), read at startup.
    pub landscape_distance: f32,
    pub sysinfo_visible: bool,
    pub scene_log_to_console: bool,
    pub max_avatars: usize,
    /// Shows every nearby avatar, whatever `max_avatars` says; that value is kept for when this
    /// goes off again.
    pub max_avatars_unlimited: bool,
    pub constrain_scene_ui: bool,
    /// Holding the pointer button and dragging turns the camera.
    pub left_drag_camera: bool,
    /// "Allow overriding scene controls": the player may change what the scene they stand in
    /// controls, today the time of day.
    pub allow_scene_control_override: bool,
    pub player_settings: PrimaryUser,
    pub max_videos: usize,
    pub max_concurrent_remotes: usize,
    pub user_id: String,
    pub default_permissions: HashMap<PermissionType, PermissionValue>,
    pub realm_permissions: HashMap<String, HashMap<PermissionType, PermissionValue>>,
    pub scene_permissions: HashMap<String, HashMap<PermissionType, PermissionValue>>,
    pub inputs: InputMapSerialized,
    pub point_at_marker_visibility: PointAtMarkerVisibility,
    pub chat_bubble_visibility: ChatBubbleVisibility,
    /// Whether situational reactions are also shown on screen, by the HUD.
    pub chat_reactions: bool,
    /// Who may DM the local user. Owned by the social service, so never persisted here: the
    /// slot mirrors the server value and the social crate upserts user changes to it. `None`
    /// until either the user or the server has set it.
    #[serde(skip)]
    pub dm_privacy: Option<DmPrivacy>,
    pub camera_smoothing: CameraSmoothing,
    /// The language the player chose (BCP-47); `None` follows the system language.
    pub language: Option<String>,
    /// Base url of an abgen Bevy variants service (prototype), when `--bevy-variants-url` /
    /// `?bevyVariants=` and an `abgen_cdn` override are absent and the endpoint feature flag
    /// names none; `None` = the asset-bundle generator's CDN for the base domain, `off` = off.
    pub bevy_variants_url: Option<String>,
    // field-level default (0) so configs saved before this field existed read as outdated,
    // rather than taking the current generation from the container-level default
    #[serde(default)]
    pub settings_generation: u32,
    #[serde(default)]
    pub inputs_generation: u32,
    /// set when the user_id was minted here rather than loaded, so startup saves the config to
    /// keep it. never serialized; the field-level default keeps loaded configs false
    #[serde(skip, default)]
    pub is_fresh_user_id: bool,
}

/// bump to run one-time migrations of the preset-managed settings in existing configs
/// (see [`AppConfig::reset_outdated_settings`])
pub const SETTINGS_GENERATION: u32 = 6;

/// bump to run one-time migrations of saved input binding tables
/// (see [`AppConfig::migrate_inputs`])
pub const INPUTS_GENERATION: u32 = 2;

/// bump when the terms of use change, so every install accepts them again
pub const TERMS_VERSION: u32 = 1;

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            home_realm: None,
            home_location: None,
            homes: HashMap::new(),
            home_last_account: None,
            previous_login: None,
            guest_root_key: None,
            accepted_terms: None,
            graphics: Default::default(),
            audio: Default::default(),
            cache_bytes: 1024 * 1024 * 1024 * 10, // 10gb
            scene_threads: 4,
            scene_load_distance: 10.0,
            scene_unload_extra_distance: 10.0,
            max_near_lods: 12,
            max_far_lods: 200,
            live_scene_ring: 1,
            near_lod_ring: 3,
            far_lod_ring: 20,
            scene_imposter_distances: vec![100.0, 200.0, 400.0, 800.0, 1600.0, 99999.0],
            scene_imposter_multisample: false,
            scene_imposter_multisample_amount: 0.0,
            scene_imposter_bake: SceneImposterBake::Off,
            parcel_grass_setting: Default::default(),
            landscape_distance: LandscapeDistance::default().0,
            sysinfo_visible: false,
            scene_log_to_console: false,
            max_avatars: 20,
            max_avatars_unlimited: false,
            constrain_scene_ui: false,
            left_drag_camera: false,
            allow_scene_control_override: false,
            player_settings: Default::default(),
            max_videos: 1,
            max_concurrent_remotes: 32,
            user_id: uuid::Uuid::new_v4().to_string(),
            default_permissions: Default::default(),
            realm_permissions: Default::default(),
            scene_permissions: Default::default(),
            inputs: Default::default(),
            point_at_marker_visibility: Default::default(),
            chat_bubble_visibility: Default::default(),
            chat_reactions: false,
            dm_privacy: None,
            camera_smoothing: Default::default(),
            language: None,
            bevy_variants_url: None,
            settings_generation: SETTINGS_GENERATION,
            inputs_generation: INPUTS_GENERATION,
            is_fresh_user_id: true,
        }
    }
}

impl AppConfig {
    pub fn terms_accepted(&self) -> bool {
        self.accepted_terms.is_some_and(|v| v >= TERMS_VERSION)
    }

    /// Parse a saved config. One that fails to parse (e.g. written by a newer build) falls back
    /// to the default, keeping its user_id when that can still be read, in which case the file
    /// is left alone rather than overwritten at startup.
    pub fn from_json(json: &[u8]) -> Self {
        #[derive(Deserialize)]
        struct UserIdOnly {
            user_id: String,
        }

        match serde_json::from_slice(json) {
            Ok(config) => config,
            Err(e) => {
                warn!("failed to parse config.json: {e}");
                let mut config = Self::default();
                if let Ok(UserIdOnly { user_id }) = serde_json::from_slice(json) {
                    config.user_id = user_id;
                    config.is_fresh_user_id = false;
                }
                config
            }
        }
    }

    fn startup_home_account(&self) -> Option<String> {
        self.previous_login
            .as_ref()
            .map(|login| home_account(&login.root_address))
            .or_else(|| self.home_last_account.clone())
    }

    fn accountless_home(&self) -> Option<PinnedHome> {
        (self.home_realm.is_some() || self.home_location.is_some()).then(|| PinnedHome {
            realm: self.home_realm.clone().unwrap_or_else(default_home_realm),
            location: self.home_location.unwrap_or(IVec2::ZERO),
        })
    }

    /// The account-less home stands in until an account adopts it.
    pub fn pinned_home(&self, account: Option<&str>) -> Option<PinnedHome> {
        account
            .and_then(|account| self.homes.get(account).cloned())
            .or_else(|| self.accountless_home())
    }

    pub fn startup_home(&self) -> Option<PinnedHome> {
        self.pinned_home(self.startup_home_account().as_deref())
    }

    /// Record `account` as the one signed in: it takes the account-less home unless it has its
    /// own, and no later account inherits it. True when the config changed.
    pub fn adopt_home(&mut self, account: &str) -> bool {
        let mut changed = false;
        if let Some(home) = self.accountless_home() {
            self.homes.entry(account.to_owned()).or_insert(home);
            self.home_realm = None;
            self.home_location = None;
            changed = true;
        }
        if self.home_last_account.as_deref() != Some(account) {
            self.home_last_account = Some(account.to_owned());
            changed = true;
        }
        changed
    }

    /// The realm the engine starts in: the startup home's, else the base-domain-derived default.
    pub fn home_realm(&self) -> String {
        self.startup_home()
            .map_or_else(default_home_realm, |home| home.realm)
    }

    /// The parcel the engine starts on: the startup home's, else 0,0.
    pub fn home_location(&self) -> IVec2 {
        self.startup_home()
            .map_or(IVec2::ZERO, |home| home.location)
    }

    /// one-time migrations for configs saved with an older generation, keeping everything
    /// else. gen 1: reinitialize the preset-managed settings to the current defaults.
    /// gen 2: moved configs off an old bloom level; bloom is only on or off now (`BloomSetting`),
    /// so there is nothing left to move.
    /// gen 3: native gpu uploads became capped by default; move configs still uncapped (0).
    /// gen 4: the defaults became the Medium graphics preset; reset the settings the presets set,
    /// so every saved config lands on Medium.
    /// gen 5: Render Resolution moves in 10% steps; move a value saved between steps to its
    /// nearest step.
    /// gen 6: new defaults for the player speeds and jump heights, which now measure a jump
    /// released at once. A value still on its old default moves to the new default.
    pub fn reset_outdated_settings(&mut self) {
        if self.settings_generation >= SETTINGS_GENERATION {
            return;
        }
        let default = Self::default();
        if self.settings_generation < 1 {
            self.reset_preset_settings(&default);
        }
        if self.settings_generation < 3 && self.graphics.gpu_bytes_per_frame == 0 {
            self.graphics.gpu_bytes_per_frame = default.graphics.gpu_bytes_per_frame;
        }
        if self.settings_generation < 4 {
            self.graphics.reset_quality_preset(&default.graphics);
        }
        if self.settings_generation < 5 {
            self.graphics
                .set_render_scale_steps(self.graphics.render_scale_steps());
        }
        if self.settings_generation < 6 {
            let (saved, new) = (&mut self.player_settings, &default.player_settings);
            for (value, old, new) in [
                (&mut saved.walk_speed, 2.5, new.walk_speed),
                (&mut saved.jog_speed, 8.18, new.jog_speed),
                (&mut saved.run_speed, 11.0, new.run_speed),
                (&mut saved.jump_height, 1.9, new.jump_height),
                (&mut saved.run_jump_height, 2.95, new.run_jump_height),
            ] {
                if *value == old {
                    *value = new;
                }
            }
        }
        self.settings_generation = SETTINGS_GENERATION;
    }

    /// gen 1's list, frozen: the settings the presets managed then, load distances and avatars
    /// included. Today's preset settings are [`GraphicsSettings::reset_quality_preset`].
    fn reset_preset_settings(&mut self, default: &Self) {
        self.graphics.msaa = default.graphics.msaa;
        self.graphics.shadow_distance = default.graphics.shadow_distance;
        self.graphics.shadow_settings = default.graphics.shadow_settings;
        self.graphics.light_count = default.graphics.light_count;
        self.graphics.shadow_caster_count = default.graphics.shadow_caster_count;
        self.graphics.fog = default.graphics.fog;
        self.graphics.bloom = default.graphics.bloom;
        self.graphics.dof = default.graphics.dof;
        self.graphics.ssao = default.graphics.ssao;
        self.graphics.sky_reflections = default.graphics.sky_reflections;
        self.graphics.oob = default.graphics.oob;
        self.graphics.spring_bones = default.graphics.spring_bones;
        self.scene_load_distance = default.scene_load_distance;
        self.scene_unload_extra_distance = default.scene_unload_extra_distance;
        self.max_near_lods = default.max_near_lods;
        self.max_far_lods = default.max_far_lods;
        self.scene_imposter_distances = default.scene_imposter_distances.clone();
        self.scene_imposter_multisample = default.scene_imposter_multisample;
        self.scene_imposter_multisample_amount = default.scene_imposter_multisample_amount;
        self.parcel_grass_setting = default.parcel_grass_setting;
        self.max_avatars = default.max_avatars;
        self.max_videos = default.max_videos;
    }

    /// migrate saved input tables: replace bindings still on changed old defaults
    /// (generation 1: RollLeft/RollRight KeyT/KeyG freed for ChatPanel/Gallery, quick emotes
    /// moved off the Action 3-6 digits onto the numpad; generation 2: the microphone moved
    /// from KeyV to KeyT and ChatPanel lost KeyT), and add
    /// defaults for any actions the saved table doesn't mention
    pub fn migrate_inputs(&mut self) {
        if self.inputs_generation < 1 {
            for (action, bindings) in self.inputs.0.iter_mut() {
                let (old_default, new_default) = match action {
                    Action::System(SystemAction::RollLeft) => (KeyCode::KeyT, None),
                    Action::System(SystemAction::RollRight) => (KeyCode::KeyG, None),
                    Action::System(SystemAction::QuickEmote0) => {
                        (KeyCode::Digit0, Some(KeyCode::Numpad0))
                    }
                    Action::System(SystemAction::QuickEmote1) => {
                        (KeyCode::Digit1, Some(KeyCode::Numpad1))
                    }
                    Action::System(SystemAction::QuickEmote2) => {
                        (KeyCode::Digit2, Some(KeyCode::Numpad2))
                    }
                    Action::System(SystemAction::QuickEmote3) => {
                        (KeyCode::Digit3, Some(KeyCode::Numpad3))
                    }
                    Action::System(SystemAction::QuickEmote4) => {
                        (KeyCode::Digit4, Some(KeyCode::Numpad4))
                    }
                    Action::System(SystemAction::QuickEmote5) => {
                        (KeyCode::Digit5, Some(KeyCode::Numpad5))
                    }
                    Action::System(SystemAction::QuickEmote6) => {
                        (KeyCode::Digit6, Some(KeyCode::Numpad6))
                    }
                    Action::System(SystemAction::QuickEmote7) => {
                        (KeyCode::Digit7, Some(KeyCode::Numpad7))
                    }
                    Action::System(SystemAction::QuickEmote8) => {
                        (KeyCode::Digit8, Some(KeyCode::Numpad8))
                    }
                    Action::System(SystemAction::QuickEmote9) => {
                        (KeyCode::Digit9, Some(KeyCode::Numpad9))
                    }
                    _ => continue,
                };
                if *bindings == [InputIdentifier::Key(old_default)] {
                    *bindings = new_default.map(InputIdentifier::Key).into_iter().collect();
                }
            }
        }
        if self.inputs_generation < 2 {
            let t = InputIdentifier::Key(KeyCode::KeyT);
            let v = InputIdentifier::Key(KeyCode::KeyV);
            let is = |action: &Action, system: SystemAction| *action == Action::System(system);
            let bindings_of = |system: SystemAction| {
                self.inputs
                    .0
                    .iter()
                    .find(|(action, _)| is(action, system))
                    .map(|(_, bindings)| bindings.as_slice())
            };
            // the microphone takes T when it is still on its old default (or has no row yet, so
            // the default merges in below) and no action other than the chat panel's old default
            // holds T; otherwise one press would fire both
            let chat_panel_on_t = bindings_of(SystemAction::ChatPanel) == Some(&[t][..]);
            let mic_on_old_default = bindings_of(SystemAction::Microphone).is_none_or(|b| b == [v]);
            // the rows that may hold T after the move: the microphone's, and the chat panel's old
            // default, which gives it up
            let gives_way = |action: &Action| {
                is(action, SystemAction::Microphone)
                    || (is(action, SystemAction::ChatPanel) && chat_panel_on_t)
            };
            let t_taken = self
                .inputs
                .0
                .iter()
                .any(|(action, bindings)| !gives_way(action) && bindings.contains(&t));
            // the chat panel gives up T only for the microphone: a player who moved the
            // microphone elsewhere keeps T on the chat panel
            if mic_on_old_default && !t_taken {
                for (action, bindings) in self.inputs.0.iter_mut() {
                    if is(action, SystemAction::ChatPanel) && *bindings == [t] {
                        bindings.clear();
                    } else if is(action, SystemAction::Microphone) {
                        *bindings = vec![t];
                    }
                }
            }
        }
        self.inputs_generation = self.inputs_generation.max(INPUTS_GENERATION);
        // ShowProfile is legacy (see the SystemAction variant): drop saved rows so the dead
        // action neither lingers in tables nor resurfaces anywhere.
        self.inputs
            .0
            .retain(|(action, _)| *action != Action::System(SystemAction::ShowProfile));
        // A merged default leaves out inputs the user gave another action, or one press would
        // fire both. Inputs the defaults share on purpose still merge.
        let defaults = InputMapSerialized::default().0;
        let chosen: HashSet<InputIdentifier> = self
            .inputs
            .0
            .iter()
            .flat_map(|(action, bindings)| {
                let own = defaults
                    .iter()
                    .find(|(default_action, _)| default_action == action)
                    .map(|(_, default_bindings)| default_bindings.as_slice())
                    .unwrap_or_default();
                bindings.iter().filter(move |id| !own.contains(id)).copied()
            })
            .collect();
        for (action, bindings) in defaults {
            if !self
                .inputs
                .0
                .iter()
                .any(|(existing, _)| *existing == action)
            {
                let bindings = bindings
                    .into_iter()
                    .filter(|id| !chosen.contains(id))
                    .collect();
                self.inputs.0.push((action, bindings));
            }
        }
        // fixed bindings can never be removed — re-add any a saved table has lost.
        for (action, id) in crate::inputs::FIXED_BINDINGS {
            match self.inputs.0.iter_mut().find(|(a, _)| *a == action) {
                Some((_, bindings)) => {
                    if !bindings.contains(&id) {
                        bindings.push(id);
                    }
                }
                None => self.inputs.0.push((action, vec![id])),
            }
        }
    }

    pub fn get_permission(
        &self,
        ty: PermissionType,
        realm: impl AsRef<str>,
        scene: impl AsRef<str>,
        is_portable: bool,
    ) -> PermissionValue {
        self.scene_permissions
            .get(scene.as_ref())
            .and_then(|map| map.get(&ty))
            .or_else(|| {
                if !is_portable {
                    self.realm_permissions
                        .get(realm.as_ref())
                        .and_then(|map| map.get(&ty))
                } else {
                    None
                }
            })
            .or_else(|| self.default_permissions.get(&ty))
            .copied()
            .unwrap_or_else(|| Self::default_permission(ty))
    }

    pub const fn default_permission(ty: PermissionType) -> PermissionValue {
        match ty {
            PermissionType::MovePlayer
            | PermissionType::ForceCamera
            | PermissionType::PlayEmote
            | PermissionType::SetLocomotion
            | PermissionType::HideAvatarsNametags
            | PermissionType::DisableVoice
            | PermissionType::OpenExplorerUi => PermissionValue::Allow,
            _ => PermissionValue::Ask,
        }
    }
}

/// The `fps_target` meaning no cap (Max).
pub const UNCAPPED_FPS: usize = 999;

#[derive(Serialize, Deserialize, Clone)]
#[serde(default)]
pub struct GraphicsSettings {
    pub vsync: bool,
    pub log_fps: bool,
    pub msaa: AaSetting,
    pub fps_target: usize,
    pub shadow_distance: f32,
    pub shadow_settings: ShadowSetting,
    pub light_count: usize,
    pub shadow_caster_count: usize,
    pub window: WindowSetting,
    // removed until bevy window resizing bugs are fixed
    // pub fullscreen_res: FullscreenResSetting,
    pub fog: FogSetting,
    pub bloom: BloomSetting,
    pub dof: DofSetting,
    pub ssao: SsaoSetting,
    pub sky_reflections: SkyReflectionSetting,
    pub oob: f32,
    pub ambient_brightness: i32,
    /// draw the dark edge outline around avatars
    pub avatar_outline: bool,
    /// show a hologram ghost where an avatar is still loading
    pub avatar_ghosts: bool,
    /// swing the extra bones of wearables (hair, tails, capes) on the nearest avatars
    pub spring_bones: bool,
    /// bytes of throttled asset uploads to the gpu per frame; 0 = unlimited
    pub gpu_bytes_per_frame: usize,
    /// share of the window's device pixels the world renders at, upscaled to the window with FSR
    /// (`visuals::upscale`); the UI stays at full resolution
    pub render_scale: f32,
    /// draw the lens flare of the sun by day and of the moon at night (`visuals::lens_flare`)
    pub lens_flare: bool,
}

/// The ambient brightness setting at which the Trilight ambient is the sky preset's
/// (visuals::sky_envmap).
pub const AMBIENT_BRIGHTNESS_DEFAULT: i32 = 50;

impl GraphicsSettings {
    /// Render Resolution steps in a render scale of 1: one step is 10%.
    pub const RENDER_SCALE_STEPS: i32 = 10;
    /// The lowest Render Resolution a player can choose, in steps (50%). Hosts may go lower for
    /// the session ([`SessionGraphics::MIN_RENDER_SCALE`]).
    pub const MIN_PLAYER_RENDER_SCALE_STEPS: i32 = 5;

    /// The saved render scale at its nearest Render Resolution step within the player's range.
    pub fn render_scale_steps(&self) -> i32 {
        ((self.render_scale * Self::RENDER_SCALE_STEPS as f32).round() as i32).clamp(
            Self::MIN_PLAYER_RENDER_SCALE_STEPS,
            Self::RENDER_SCALE_STEPS,
        )
    }

    pub fn set_render_scale_steps(&mut self, steps: i32) {
        self.render_scale = steps as f32 / Self::RENDER_SCALE_STEPS as f32;
    }
}

/// The defaults are the Medium graphics preset, the one a new install gets: the HUD's Graphics
/// Preset (`react-web/bridge-scene/src/domains/graphicsPresets.ts`) reads Medium off each
/// setting's published default.
impl Default for GraphicsSettings {
    fn default() -> Self {
        Self {
            // the browser paces the web build to the display, whatever this says
            vsync: !cfg!(target_arch = "wasm32"),
            log_fps: !cfg!(target_arch = "wasm32"),
            // FXAA, not MSAA, so depth of field stays on (see `DofSetting::apply_to_camera`)
            msaa: AaSetting::FxaaHigh,
            fps_target: 60,
            // one cascade on the 2048 map (`visuals`), so a short distance keeps its texels small
            shadow_distance: 20.0,
            // one cascade
            shadow_settings: ShadowSetting::Low,
            light_count: 6,
            // spot light shadows within 20 m
            shadow_caster_count: 3,
            window: WindowSetting::Windowed,
            // fullscreen_res: FullscreenResSetting(UVec2::new(1280,720)),
            fog: FogSetting::On,
            bloom: BloomSetting::On,
            dof: DofSetting::High,
            ssao: SsaoSetting::Off,
            sky_reflections: SkyReflectionSetting::High,
            oob: 2.0,
            ambient_brightness: AMBIENT_BRIGHTNESS_DEFAULT,
            avatar_outline: true,
            avatar_ghosts: true,
            spring_bones: false,
            gpu_bytes_per_frame: 10_000_000,
            render_scale: 1.0,
            lens_flare: true,
        }
    }
}

impl GraphicsSettings {
    /// Set every setting the HUD's Graphics Preset sets to `preset`'s value, keeping the rest.
    pub fn reset_quality_preset(&mut self, preset: &Self) {
        self.vsync = preset.vsync;
        self.fps_target = preset.fps_target;
        self.render_scale = preset.render_scale;
        self.msaa = preset.msaa;
        self.bloom = preset.bloom;
        self.avatar_outline = preset.avatar_outline;
        self.shadow_settings = preset.shadow_settings;
        self.shadow_distance = preset.shadow_distance;
        self.light_count = preset.light_count;
        self.shadow_caster_count = preset.shadow_caster_count;
        self.lens_flare = preset.lens_flare;
        self.dof = preset.dof;
        self.ssao = preset.ssao;
        self.fog = preset.fog;
        self.sky_reflections = preset.sky_reflections;
    }
}

/// Graphics a host page sets for this session only, such as the scene editor's viewport: never
/// saved, and each one that is set wins over the player's own setting.
#[derive(Resource, Default, Debug, Clone, PartialEq)]
pub struct SessionGraphics {
    pub fps_target: Option<usize>,
    pub parcel_grass: Option<ParcelGrassSetting>,
    pub render_scale: Option<f32>,
    /// Set by the preview's `/avatar_demo cap`, which takes values past the setting's range and
    /// wins over the player's setting, Unlimited Avatars included.
    pub max_avatars: Option<usize>,
}

impl SessionGraphics {
    /// The lowest render scale a host can set. It is below the player's Render Resolution floor
    /// (50%) on purpose: a player needs the world sharp enough to play in, while a host such as
    /// the editor on a weak machine may trade more sharpness for frame rate.
    pub const MIN_RENDER_SCALE: f32 = 0.25;

    pub fn fps_target(&self, config: &AppConfig) -> usize {
        self.fps_target.unwrap_or(config.graphics.fps_target)
    }

    /// The value of "no cap" in a Max Avatars count.
    pub const NO_AVATAR_CAP: usize = usize::MAX;

    /// The session's cap if it set one, else no cap when the player turned Max Avatars off, else
    /// the player's count.
    pub fn max_avatars(&self, config: &AppConfig) -> usize {
        self.max_avatars.unwrap_or(if config.max_avatars_unlimited {
            Self::NO_AVATAR_CAP
        } else {
            config.max_avatars
        })
    }

    /// The frame rate the explorer paces to: the session's target, else the player's, and with
    /// VSync on no faster than the monitor. VSync only removes tearing, so a lower target still
    /// applies, and Max ([`UNCAPPED_FPS`]) means the monitor's rate. `refresh_hz`, when known, is
    /// the monitor's rate rounded up past it (60 Hz gives 61), so at the monitor's rate VSync, not
    /// the pacer's sleep, sets the frame time.
    pub fn paced_fps(
        session: Option<&Self>,
        config: &AppConfig,
        refresh_hz: Option<usize>,
    ) -> usize {
        let target = session.map_or(config.graphics.fps_target, |session| {
            session.fps_target(config)
        });
        match refresh_hz {
            Some(refresh) if config.graphics.vsync && (target == 0 || refresh <= target + 1) => {
                refresh
            }
            _ => target,
        }
    }

    /// [`Self::paced_fps`] as the debug panel shows it: when VSync sets the rate, the monitor's
    /// rate (60 for a 60 Hz monitor, not the pacer's 61).
    pub fn shown_fps(
        session: Option<&Self>,
        config: &AppConfig,
        refresh_hz: Option<usize>,
    ) -> usize {
        let fps = Self::paced_fps(session, config, refresh_hz);
        if config.graphics.vsync && refresh_hz == Some(fps) {
            fps.saturating_sub(1)
        } else {
            fps
        }
    }

    pub fn parcel_grass(&self, config: &AppConfig) -> ParcelGrassSetting {
        self.parcel_grass.unwrap_or(config.parcel_grass_setting)
    }

    pub fn render_scale(&self, config: &AppConfig) -> f32 {
        self.render_scale.unwrap_or(config.graphics.render_scale)
    }
}

#[derive(Debug)]
pub enum AudioType {
    Voice,
    Scene,
    System,
    Avatar,
}

// a field missing from a saved config takes its default (a failed parse resets the whole config)
#[derive(Resource, Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(default)]
pub struct AudioSettings {
    pub master: i32, // 0-100
    /// nearby players' voices
    pub voice: i32,
    /// private calls and community voice streams
    pub voice_chat: i32,
    pub scene: i32,
    pub system: i32,
    pub avatar: i32,
    /// Whether [`Self::microphone_focus`] passes the client window's focus through.
    pub mute_mic_in_background: bool,
    /// The chosen audio input's device id; `None` uses the system default.
    pub microphone: Option<String>,
}

impl Default for AudioSettings {
    fn default() -> Self {
        Self {
            master: 100,
            voice: 100,
            voice_chat: 100,
            scene: 100,
            system: 100,
            avatar: 100,
            mute_mic_in_background: true,
            microphone: None,
        }
    }
}

impl AudioSettings {
    /// `focus` while `mute_mic_in_background` is on, `Focused` while it is off.
    pub const fn microphone_focus(&self, focus: ClientWindowFocus) -> ClientWindowFocus {
        if self.mute_mic_in_background {
            focus
        } else {
            ClientWindowFocus::Focused
        }
    }
    pub fn voice(&self) -> f32 {
        (self.voice * self.master) as f32 / 10_000.0
    }
    pub fn voice_chat(&self) -> f32 {
        (self.voice_chat * self.master) as f32 / 10_000.0
    }
    pub fn scene(&self) -> f32 {
        (self.scene * self.master) as f32 / 10_000.0
    }
    pub fn system(&self) -> f32 {
        (self.system * self.master) as f32 / 10_000.0
    }
    pub fn avatar(&self) -> f32 {
        (self.avatar * self.master) as f32 / 10_000.0
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
pub enum ShadowSetting {
    Off,
    /// one cascade, hardware 2x2 filtering
    Low,
    /// four soft cascades
    High,
}

/// The far bounds of High's first three cascades, as fractions of the shadow distance.
pub const SUN_CASCADE_4_SPLITS: [f32; 3] = [0.102, 0.282, 0.474];
/// How far from the player a spot light can cast shadows, in metres.
pub const SPOT_SHADOW_MAX_DISTANCE: f32 = 20.0;
pub const MIN_CASCADE: f32 = 0.1;
/// How far Low's one cascade reaches, in metres, whatever the shadow distance: past it the
/// 2048 map's texels are too large to hold an avatar's shadow (see decisions.md).
pub const SUN_LOW_MAX_DISTANCE: f32 = 40.0;

/// A light with its own cascades (the lobby stage's key light): the shadow settings only turn its
/// shadows on or off.
#[derive(Component)]
pub struct StageLight;

/// A light that never casts shadows (a photo booth's key light): the shadow settings leave it
/// alone.
#[derive(Component)]
pub struct ShadowlessLight;

/// The camera that shows the lobby stage, drawn with no post-processing: the bloom setting
/// leaves it alone.
#[derive(Component)]
pub struct StageCamera;

/// The sun's cascades for a quality and a shadow distance, `None` when it casts no shadows. Low's
/// one cascade stops at `SUN_LOW_MAX_DISTANCE`.
pub fn sun_cascades(setting: ShadowSetting, distance: f32) -> Option<CascadeShadowConfig> {
    if distance <= 0.0 {
        return None;
    }
    let far = distance.max(MIN_CASCADE + f32::EPSILON);
    let bounds = match setting {
        ShadowSetting::Off => return None,
        ShadowSetting::Low => vec![far.min(SUN_LOW_MAX_DISTANCE)],
        ShadowSetting::High => {
            let mut previous = MIN_CASCADE;
            SUN_CASCADE_4_SPLITS
                .iter()
                .map(|split| (f64::from(*split) * f64::from(far)) as f32)
                .chain([far])
                .map(|bound| {
                    // bounds must increase strictly, even for a distance under the minimum
                    previous = bound.max(previous + f32::EPSILON);
                    previous
                })
                .collect()
        }
    };
    Some(CascadeShadowConfig {
        bounds,
        overlap_proportion: 0.2,
        minimum_distance: MIN_CASCADE,
    })
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
pub enum AaSetting {
    Off,
    FxaaLow,
    FxaaHigh,
    Msaa2x,
    Msaa4x,
    Msaa8x,
}

impl AaSetting {
    pub fn is_msaa(self) -> bool {
        matches!(self, Self::Msaa2x | Self::Msaa4x | Self::Msaa8x)
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
pub enum WindowSetting {
    #[cfg(not(target_arch = "wasm32"))]
    Fullscreen,
    Windowed,
    Borderless,
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
pub struct FullscreenResSetting(pub UVec2);

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
pub enum FogSetting {
    Off,
    #[serde(alias = "Basic", alias = "Atmospheric")]
    On,
}

/// Bloom is on or off. A saved config may name one of the two levels there used to be: both
/// read as on. On is saved as `High`, the old level it draws, so an older client reads it.
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
pub enum BloomSetting {
    Off,
    #[serde(rename = "High", alias = "Low", alias = "On")]
    On,
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
pub enum DofSetting {
    Off,
    Low,
    High,
}

#[derive(Component)]
// (sensor height, extra focal distance)
pub struct DofConfig {
    pub default_sensor_height: f32,
    pub extra_focal_distance: f32,
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
pub enum SsaoSetting {
    Off,
    Low,
    High,
}

/// The sky environment map: the sky's reflections and ambient light.
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
pub enum SkyReflectionSetting {
    /// one colour per direction: sky, horizon and ground
    Low,
    /// filtered reflections of the sky
    High,
}

#[derive(Debug)]
pub enum AudioDecoderError {
    StreamClosed,
    Other(String),
}

#[derive(Resource)]
pub struct SceneLoadDistance {
    pub load: f32,
    pub unload: f32, // additional
    pub load_imposter: f32,
}

/// The landscape distance in metres: how far the hills are displaced and the cliffs are drawn.
/// Set from the config's `landscape_distance` at startup ([`LandscapeDistance::from_config`]).
#[derive(Resource, Clone, Copy, Debug, PartialEq)]
pub struct LandscapeDistance(pub f32);

impl LandscapeDistance {
    pub const MIN: f32 = 500.0;
    pub const MAX: f32 = 7000.0;

    /// The config's value held to [`Self::MIN`]..=[`Self::MAX`]; a value that is not a number
    /// gives the default.
    pub fn from_config(metres: f32) -> Self {
        if metres.is_nan() {
            Self::default()
        } else {
            Self(metres.clamp(Self::MIN, Self::MAX))
        }
    }
}

impl Default for LandscapeDistance {
    fn default() -> Self {
        Self(1000.0)
    }
}

#[derive(Debug)]
pub struct IVec2Arg(pub IVec2);

impl FromStr for IVec2Arg {
    type Err = ParseIntError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut chars = s.chars().peekable();

        let skip = |chars: &mut std::iter::Peekable<std::str::Chars>, numeric: bool| {
            while numeric
                == chars
                    .peek()
                    .map_or(!numeric, |c| c.is_numeric() || *c == '-')
            {
                chars.next();
            }
        };

        let parse = |chars: &std::iter::Peekable<std::str::Chars>| {
            chars
                .clone()
                .take_while(|c| c.is_numeric() || *c == '-')
                .collect::<String>()
                .parse::<i32>()
        };

        skip(&mut chars, false);
        let x = parse(&chars)?;
        skip(&mut chars, true);
        skip(&mut chars, false);
        let y = parse(&chars)?;

        Ok(IVec2Arg(IVec2::new(x, y)))
    }
}

// scene metadata
#[derive(Deserialize, Debug, Clone)]
pub struct SpawnPosition {
    x: serde_json::Value,
    y: serde_json::Value,
    z: serde_json::Value,
}

impl SpawnPosition {
    pub fn bounding_box(&self) -> (Vec3, Vec3) {
        let parse_val = |v: &serde_json::Value| -> Option<Range<f32>> {
            if let Some(val) = v.as_f64() {
                Some(val as f32..val as f32)
            } else if let Some(array) = v.as_array() {
                if let Some(mut start) = array.first().and_then(|s| s.as_f64()) {
                    let mut end = array.get(1).and_then(|e| e.as_f64()).unwrap_or(start);
                    if end < start {
                        (start, end) = (end, start);
                    }
                    Some(start as f32..end as f32)
                } else {
                    None
                }
            } else {
                None
            }
        };

        let x = parse_val(&self.x).unwrap_or(0.0..16.0);
        let y = parse_val(&self.y).unwrap_or(0.0..0.0);
        let z = parse_val(&self.z).unwrap_or(0.0..16.0);

        (
            Vec3::new(x.start, y.start, z.start),
            Vec3::new(x.end, y.end, z.end),
        )
    }
}

#[derive(Deserialize, Debug, Clone)]
pub struct SpawnPoint {
    pub name: Option<String>,
    #[serde(default)]
    pub default: bool,
    pub position: SpawnPosition,
}

#[derive(Deserialize, Debug)]
pub struct SceneMetaScene {
    pub base: String,
    pub parcels: Vec<String>,
}

#[derive(Deserialize, Debug)]
pub struct SceneDisplay {
    pub title: Option<String>,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct SkyboxConfig {
    pub fixed_time: Option<f32>,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct SceneMeta {
    pub owner: Option<String>,
    pub display: Option<SceneDisplay>,
    pub main: String,
    pub scene: SceneMetaScene,
    pub runtime_version: Option<String>,
    pub spawn_points: Option<Vec<SpawnPoint>>,
    pub skybox_config: Option<SkyboxConfig>,
    pub authoritative_multiplayer: Option<bool>,
    /// `featureToggles`: a host scene's switches, of which `portableExperiences`
    /// ([`PortableExperiencesToggle`]) and `nearbyVoiceChat` are read. Kept as raw json so a value of another shape never
    /// fails the scene.json.
    pub feature_toggles: Option<serde_json::Value>,
    /// `isPortableExperience`. Kept as raw json so a value of another shape never fails the
    /// scene.json.
    pub is_portable_experience: Option<serde_json::Value>,
}

impl SceneMeta {
    /// `isPortableExperience: true`, however the scene was loaded.
    pub fn declares_portable_experience(&self) -> bool {
        self.is_portable_experience
            .as_ref()
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false)
    }

    /// What this scene says about portable experiences while the player stands in it.
    pub fn portable_experiences_toggle(&self) -> PortableExperiencesToggle {
        self.feature_toggles
            .as_ref()
            .and_then(|toggles| toggles.get("portableExperiences"))
            .and_then(serde_json::Value::as_str)
            .map(|value| value.parse().unwrap_or_default())
            .unwrap_or_default()
    }

    /// Whether nearby voice stays on while the player stands in this scene: `nearbyVoiceChat`
    /// absent, not a string, empty or `enabled` (any case); any other string turns it off.
    pub fn nearby_voice_chat_enabled(&self) -> bool {
        self.feature_toggles
            .as_ref()
            .and_then(|toggles| toggles.get("nearbyVoiceChat"))
            .and_then(serde_json::Value::as_str)
            .is_none_or(|value| value.is_empty() || value.eq_ignore_ascii_case("enabled"))
    }
}

/// A host scene's `featureToggles.portableExperiences`: whether portable experiences run (and
/// show their UI) while the player is inside the scene. Anything but `disabled` lets them run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum PortableExperiencesToggle {
    #[default]
    Enabled,
    Disabled,
    HideUi,
}

impl std::str::FromStr for PortableExperiencesToggle {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_ascii_lowercase().as_str() {
            "enabled" => Ok(Self::Enabled),
            "disabled" => Ok(Self::Disabled),
            "hideui" => Ok(Self::HideUi),
            _ => Err(()),
        }
    }
}

#[derive(Component)]
pub struct NearbyVoiceDisabled;

/// On the scene whose request to turn nearby voice off the player is visiting: the answer of
/// the `DisableVoice` permission, `None` until there is one. Removed when the player leaves.
#[derive(Component)]
pub struct VoiceOffAnswer(pub Option<bool>);

/// On the root of a scene loaded as a portable experience whose scene.json also declares
/// `isPortableExperience: true`: the only scenes whose `MapPin`s are read.
#[derive(Component)]
pub struct ReadsMapPins;

pub trait PermissionStrings {
    fn active(&self) -> &str;
    fn passive(&self) -> &str;
    fn title(&self) -> &str;
    fn request(&self) -> String;
    fn on_success(&self, portable: Option<&str>, additional: Option<&str>) -> String;

    fn on_fail(&self, portable: Option<&str>) -> String;
    fn description(&self) -> String;
}

impl PermissionStrings for PermissionType {
    fn title(&self) -> &str {
        match self {
            PermissionType::MovePlayer => "Move Avatar",
            PermissionType::ForceCamera => "Force Camera",
            PermissionType::PlayEmote => "Play Emote",
            PermissionType::SetLocomotion => "Set Locomotion",
            PermissionType::HideAvatarsNametags => {
                "Hide Avatars and/or Nametags, and/or disables Passports"
            }
            PermissionType::DisableVoice => "Disable Voice",
            PermissionType::Teleport => "Teleport",
            PermissionType::ChangeRealm => "Change Realm",
            PermissionType::SpawnPortable => "Spawn Portable Experience",
            PermissionType::KillPortables => "Manage Portable Experiences",
            PermissionType::Web3 => "Web3 Transaction",
            PermissionType::Fetch => "Fetch Data",
            PermissionType::Websocket => "Open Websocket",
            PermissionType::OpenUrl => "Open Url",
            PermissionType::CopyToClipboard => "Copy to Clipboard",
            PermissionType::OpenExplorerUi => "Open Explorer Menu or Item Purchase",
        }
    }

    fn request(&self) -> String {
        format!("The scene wants permission to {}", self.passive())
    }

    fn description(&self) -> String {
        format!(
            "This permission is requested when scene attempts to {}",
            self.passive()
        )
    }

    fn on_success(&self, portable: Option<&str>, additional: Option<&str>) -> String {
        format!(
            "{} is {}{}(click to manage)",
            match portable {
                Some(portable) => format!("The portable scene {portable}"),
                None => "The scene".to_owned(),
            },
            self.active(),
            additional
                .map(|add| format!(": {add} "))
                .unwrap_or_default(),
        )
    }
    fn on_fail(&self, portable: Option<&str>) -> String {
        format!(
            "{} was blocked from {} (click to manage)",
            match portable {
                Some(portable) => format!("The portable scene {portable}"),
                None => "The scene".to_owned(),
            },
            self.active()
        )
    }

    fn passive(&self) -> &str {
        match self {
            PermissionType::MovePlayer => "move your avatar within the scene bounds",
            PermissionType::ForceCamera => "temporarily change the camera view",
            PermissionType::PlayEmote => "make your avatar perform an emote",
            PermissionType::SetLocomotion => "temporarily modify your avatar's locomotion settings",
            PermissionType::HideAvatarsNametags => {
                "temporarily hide player avatars and/or nametags, and/or disables passports"
            }
            PermissionType::DisableVoice => "temporarily disable voice chat",
            PermissionType::Teleport => "teleport you to a new location",
            PermissionType::ChangeRealm => "move you to a new realm",
            PermissionType::SpawnPortable => "spawn a portable experience",
            PermissionType::KillPortables => "manage your active portable experiences",
            PermissionType::Web3 => "initiate a web3 transaction with your wallet",
            PermissionType::Fetch => "fetch data from a remote server",
            PermissionType::Websocket => "open a web socket to communicate with a remote server",
            PermissionType::OpenUrl => "open a url in your browser",
            PermissionType::CopyToClipboard => "copy text into the clipboard",
            PermissionType::OpenExplorerUi => {
                "open an explorer menu panel (map, backpack, settings, ...) or the purchase confirmation for an item"
            }
        }
    }

    fn active(&self) -> &str {
        match self {
            PermissionType::MovePlayer => "moving your avatar",
            PermissionType::ForceCamera => "enforcing the camera view",
            PermissionType::PlayEmote => "making your avatar perform an emote",
            PermissionType::SetLocomotion => "enforcing your locomotion settings",
            PermissionType::HideAvatarsNametags => {
                "hiding some avatars and/or nametags, and/or disables passports"
            }
            PermissionType::DisableVoice => "disabling voice communications",
            PermissionType::Teleport => "teleporting you to a new location",
            PermissionType::ChangeRealm => "teleporting you to a new realm",
            PermissionType::SpawnPortable => "spawning a portable experience",
            PermissionType::KillPortables => "managing your active portables",
            PermissionType::Web3 => "initiating a web3 transaction",
            PermissionType::Fetch => "fetching remote data",
            PermissionType::Websocket => "opening a websocket",
            PermissionType::OpenUrl => "opening a url in your browser",
            PermissionType::CopyToClipboard => "copying text into the clipboard",
            PermissionType::OpenExplorerUi => {
                "opening an explorer menu panel or an item's purchase confirmation"
            }
        }
    }
}

#[derive(Clone, Serialize, Deserialize, Event)]
#[serde(rename_all = "camelCase")]
pub struct PermissionUsed {
    pub ty: PermissionType,
    pub additional: Option<String>,
    pub scene: String,
    pub was_allowed: bool,
    /// `None` for a use: the scene asked and was allowed or refused. `Some` reports instead that
    /// what the permission allows started (`true`) or stopped (`false`) acting on the player,
    /// such as a camera the scene holds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub in_force: Option<bool>,
}

#[derive(Resource)]
pub struct ActiveDialog(Arc<Semaphore>);

impl Default for ActiveDialog {
    fn default() -> Self {
        Self(Arc::new(Semaphore::new(1)))
    }
}

impl ActiveDialog {
    pub fn try_acquire(&self) -> Option<DialogPermit> {
        self.0
            .clone()
            .try_acquire_owned()
            .ok()
            .map(|p| DialogPermit { _p: Some(p) })
    }

    pub fn in_use(&self) -> bool {
        self.0.available_permits() == 0
    }
}

#[derive(Component)]
pub struct DialogPermit {
    _p: Option<OwnedSemaphorePermit>,
}

impl DialogPermit {
    pub fn take(&mut self) -> Self {
        Self {
            _p: Some(self._p.take().unwrap()),
        }
    }
}

#[derive(Event, Clone)]
pub struct SystemAudio(pub String);

impl From<&str> for SystemAudio {
    fn from(value: &str) -> Self {
        Self(value.to_owned())
    }
}

impl From<String> for SystemAudio {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl From<&String> for SystemAudio {
    fn from(value: &String) -> Self {
        Self(value.to_owned())
    }
}

// render layers
// 0 is default
// - normally 0 and 1 is used for the player, when in first person only 1 is used for the player
// - world lights target both 0 and 1, the main camera uses 0
// - this allows shadows to be cast by the player without the player being visible
pub const PRIMARY_AVATAR_LIGHT_LAYER_INDEX: usize = 1;
pub const PRIMARY_AVATAR_LIGHT_LAYER: RenderLayers =
    RenderLayers::layer(PRIMARY_AVATAR_LIGHT_LAYER_INDEX);
// layer for profile content
pub const PROFILE_UI_RENDERLAYER: RenderLayers = RenderLayers::layer(3);
// layer for ground
pub const GROUND_RENDERLAYER: RenderLayers = RenderLayers::layer(4);

#[derive(PartialEq, Eq, Clone, Copy, Serialize, Deserialize)]
pub enum SceneImposterBake {
    Off,
    FullSpeed,
    HalfSpeed,
    QuarterSpeed,
}

impl SceneImposterBake {
    pub fn as_mult(&self) -> f32 {
        match self {
            SceneImposterBake::Off => panic!(),
            SceneImposterBake::FullSpeed => 1.0,
            SceneImposterBake::HalfSpeed => 0.5,
            SceneImposterBake::QuarterSpeed => 0.25,
        }
    }
}

#[derive(Resource, Default)]
pub struct CursorLocks(pub HashSet<&'static str>);

/// The `CursorLocks` entry held while the pointer turns the camera.
pub const CAMERA_CURSOR_LOCK: &str = "camera";

/// Controls engine-level overrides to avatar movement, independent of scene-driven movement.
#[derive(Resource, Default)]
pub struct EngineMovementControl {
    /// Non-empty means collision/clipping is disabled (e.g. "noclip" from /idnoclip)
    pub suppress_clipping: HashSet<&'static str>,
    /// Non-empty means avatar physics movement systems are suppressed (e.g. "move_player_to" during interpolation)
    pub suppress_avatar_physics: HashSet<&'static str>,
    /// Scene-driven `AvatarMovement` is ignored (physics suppressed) until a tick
    /// initiated strictly after this time (engine dispatch clock, `last_sent` secs)
    /// is received. Set by `movePlayerTo` when it imposes a facing, so the imposed
    /// orientation survives until the controller scene reads the new transform and
    /// echoes it back, rather than being clobbered by an in-flight stale tick.
    pub accept_movement_after: f64,
    /// Set by `movePlayerTo` (teleport or interpolation end): the scene deliberately
    /// placed the player here, possibly inside a collider (sit-on-chair emotes).
    /// While set, depenetration is not applied in x/z and the sweep ignores colliders
    /// the player starts inside; movement is only clamped to not go deeper. Cleared
    /// by `resolve_collisions` once no correction is required.
    pub deliberate_penetration: bool,
    /// While `deliberate_penetration` is set: the per-axis depenetration bounds
    /// (min, max) relative to the current transform, refreshed each frame by
    /// `resolve_collisions`. `min.axis > 0` means the eject direction is +axis.
    pub penetration_bounds: (DVec3, DVec3),
}

#[derive(Default, Clone, Copy, Debug, PartialEq, Eq)]
pub enum MoveKind {
    #[default]
    Idle,
    Walk,
    Jog,
    Run,
    Jump,
    /// In-air second jump. Set on foreign avatars when the incoming rfc4::Movement
    /// reports jump_count >= 2 and there's no scene-driven animation to take
    /// precedence; the velocity picker emits the `double_jump` emote for it.
    DoubleJump,
    Falling,
    LongFalling,
    /// Gliding state. Set on foreign avatars when rfc4::Movement.glide_state is
    /// OPENING_PROP or GLIDING and there's no scene-driven animation; the
    /// velocity picker emits the `glide` emote.
    Glide,
    Emote,
}

#[derive(Component, Default)]
pub struct AvatarDynamicState {
    pub velocity: Vec3,
    /// Primary player only: how far a startup portable's controller (launch-time, console or
    /// server-hosted, such as the default controller) moved the player this frame, after collisions
    /// and without lifts or platforms, with movement input held; zero while a parcel scene, a
    /// smart wearable or a scene-spawned portable drives, a suppression holds or a force pushes the
    /// player.
    pub own_displacement: Vec3,
    /// Height of the feet above the ground the avatar can stand on.
    pub ground_height: f32,
    pub jump_time: f64,
    pub move_kind: MoveKind,
}

#[derive(Event)]
pub enum PreviewCommand {
    ReloadScene {
        hash: String,
    },
    ReloadModel {
        hash: String,
        src: String,
        scene_id: String,
    },
    RemoveModel {
        hash: String,
        src: String,
        scene_id: String,
    },
}

impl From<ws_scene_message::Message> for PreviewCommand {
    fn from(value: ws_scene_message::Message) -> Self {
        match value {
            ws_scene_message::Message::UpdateScene(update_scene) => Self::ReloadScene {
                hash: update_scene.scene_id.to_owned(),
            },
            ws_scene_message::Message::UpdateModel(update_model) => match update_model.r#type() {
                UpdateModelType::UmtChange => Self::ReloadModel {
                    hash: update_model.hash.to_owned(),
                    src: update_model.src.to_owned(),
                    scene_id: update_model.scene_id.to_owned(),
                },
                UpdateModelType::UmtRemove => Self::RemoveModel {
                    hash: update_model.hash.to_owned(),
                    src: update_model.src.to_owned(),
                    scene_id: update_model.scene_id.to_owned(),
                },
            },
        }
    }
}

/// The local player was instantly repositioned — a durationless `move_player_to`, a `teleport_player`,
/// or a spawn snap — rather than walking there. Comms turns this into a Pulse `TeleportRequest` so peers
/// snap to the new position instead of interpolating across the gap. `position` is Bevy world space.
#[derive(Event)]
pub struct PlayerTeleported {
    pub position: Vec3,
}

/// The local player's parcel changed, the scene the realm lists at it did, or the player entered
/// or left the world. `old` is the parcel before (`None` for the first; `parcel` itself when only
/// the scene or `in_world` changed). `scene`: `None` unresolved, `Some(None)` an empty parcel,
/// `Some(Some(hash))` a scene. `in_world` is read with the parcel: false behind the loading
/// screen ([`OutOfWorld`]).
///
/// A change of the scene alone is reported too: analytics cannot read the scene pointers, and a
/// realm change at the same parcel lists another scene there.
#[derive(Event, Clone, Debug, PartialEq)]
pub struct PlayerParcelChanged {
    pub old: Option<IVec2>,
    pub parcel: IVec2,
    pub scene: Option<Option<String>>,
    pub in_world: bool,
}

/// Marks the local player as behind the loading screen — teleported or spawning, with a provisional
/// position — until the destination scene resolves and they're placed in-world. Lives here (rather
/// than `scene_runner`) so lower-level crates like `comms` can read it; `scene_runner` re-exports it.
#[derive(Component, Default, Debug)]
pub struct OutOfWorld {
    /// The name of the spawn point of the destination scene to be placed on, compared in
    /// lowercase. Without one, or when the scene has none of that name, the scene's default spawn
    /// points are used.
    pub spawn_point: Option<String>,
}

/// The primary camera is the camera reel's free photo camera: the player camera systems leave its
/// transform and projection alone until it is removed.
#[derive(Component)]
pub struct PhotoMode;

/// The player is let back into the world (`OutOfWorld` removed), and why: the parcel scene is
/// ready (or there is none), or it never will be and they go in regardless.
#[derive(Event, Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReturnedToWorld {
    SceneReady,
    /// the parcel scene hung past the not-responding timeout, errored or was unreachable
    SceneBroken,
    /// the parcel scene failed to load
    SceneLoadFailed,
}

/// Holds the world back for a lobby shown before it: the realm is resolved (so sign-in, profiles
/// and wearables work) and system scenes run, but no realm scenes or realm comms load. Released
/// by the first successful realm change request.
#[derive(Resource)]
pub struct WorldHold;

pub struct StartupScene {
    pub source: String,
    pub super_user: bool,
    pub preview: bool,
    pub hot_reload: Option<tokio::sync::mpsc::UnboundedSender<PreviewCommand>>,
    pub hash: Option<String>,
    /// the source didn't resolve: the scene is never coming
    pub failed: bool,
}

#[derive(Resource)]
pub struct StartupScenes {
    pub scenes: Vec<StartupScene>,
}

#[derive(Resource, Default, Clone, Debug, PartialEq)]
pub struct SceneGlobalLight {
    pub source: Option<Entity>,
    pub dir_color: Color,
    pub dir_illuminance: f32,
    pub dir_direction: Vec3,
    /// the sun as the sky draws it. the same as the directional light by day, but it keeps
    /// setting below the horizon at night while the light follows the moon
    pub sun_illuminance: f32,
    pub sun_direction: Vec3,
    /// a tint and a multiplier on the Trilight ambient and on the floor the sky reflections are
    /// levelled towards (visuals::sky_envmap): white at 1 unless a scene overrides it
    pub ambient_color: Color,
    pub ambient_brightness: f32,
    /// a scene hid the sun and moon discs (not their light)
    pub sun_hidden: bool,
    pub layers: RenderLayers,
}

/// The time the sky shows and who sets it. Sent whole to every scene, which reads it with
/// `~system/Runtime.getWorldTime`.
#[derive(Resource, Clone, Copy, Default, PartialEq, Debug, Serialize, Deserialize)]
pub struct TimeOfDay {
    /// secs since midnight
    pub time: f32,
    /// The scene the player stands in holds the time of day and the player may not change it.
    pub locked: bool,
    /// The speed of the player's clock; 0 when it is stopped.
    pub speed: f32,
    /// The player's clock sets the sky: no scene's time does.
    pub follows_clock: bool,
}

impl TimeOfDay {
    pub fn elapsed_secs(&self) -> f32 {
        self.time
    }
}

/// Vertical FOV of the active player camera, in radians. Pushed to scene
/// workers via the GlobalCrdtState channel — defaulted on the worker side and
/// updated when the renderer's camera projection changes.
#[derive(Resource, Default, Clone, Copy, Debug)]
pub struct CameraFov(pub f32);

/// Fixed time defined on `scene.json` `skyboxConfig`
#[derive(Component)]
#[component(immutable)]
pub struct SceneTime {
    /// secs since midnight
    pub time: f32,
}

// porting aid, used to be one component
pub type TextStyle = (TextFont, TextColor);

// non-spatial audio
#[derive(Component, Debug)]
pub struct AudioEmitter {
    pub handle: Handle<bevy_kira_audio::AudioSource>,
    pub playing: bool,
    pub playback_speed: f32,
    pub r#loop: bool,
    pub volume: f32,
    pub global: bool,
    pub seek_time: Option<f32>,
    pub ty: AudioType,
}

impl Default for AudioEmitter {
    fn default() -> Self {
        Self {
            handle: default(),
            playing: true,
            playback_speed: 1.0,
            r#loop: false,
            volume: 1.0,
            global: false,
            seek_time: None,
            ty: AudioType::System,
        }
    }
}

// despawns the `AudioEmitter` entity once its (non-looping) sound has finished playing
#[derive(Component, Debug, Default)]
pub struct OneShotAudio;

#[derive(Clone, Copy)]
#[repr(i32)]
pub enum ZOrder {
    Crosshair = -65536,
    // PortableScene -> -65535 <= value <= -1
    // default 0 => appear in world, under scene ui
    PointAtMarker = 1,
    OutOfWorldBackdrop,
    SceneUi,
    SceneUiOverlay,
    SystemSceneUi,
    SystemSceneUiOverlay,
    MouseInteractionComponent,
    SceneLoadingDialog,
    ChatBubble,
    NftDialog,
    EmoteSelect,
    ProfileView,
    Login,
    Minimap,
    SystemUi,
    ToolTip,
    Backpack,
    BackpackPopup,
    Toast,
    Permission,
    DefaultComboPopup,
}

impl ZOrder {
    pub fn default(self) -> GlobalZIndex {
        GlobalZIndex(self as i32)
    }
}

#[derive(Event, Debug)]
pub enum AppError {
    NetworkFailure(anyhow::Error),
}

/// The client's focus: one representation for the native window's focus and the browser page's
/// focus.
#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClientWindowFocus {
    Focused,
    NoFocus,
}

impl ClientWindowFocus {
    /// From the OS's focus report for the window, not `Window::focused`, which also holds
    /// unanswered focus requests.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn from_window_focused(window_focused: &bevy::window::WindowFocused) -> Self {
        Self::from_has_focus(window_focused.focused)
    }

    /// `page_has_focus` is `document.hasFocus()`, not `Window::focused`.
    #[cfg(target_arch = "wasm32")]
    pub fn from_page_focus(page_has_focus: bool) -> Self {
        Self::from_has_focus(page_has_focus)
    }

    fn from_has_focus(has_focus: bool) -> Self {
        if has_focus {
            Self::Focused
        } else {
            Self::NoFocus
        }
    }
}

/// Asks the OS to focus the primary window. [`ClientWindowFocus`] changes only once the OS
/// reports the focus.
#[derive(Event, Debug, Clone, Copy)]
pub struct RequestClientWindowFocus;

/// An audio input the host offers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MicrophoneInput {
    pub id: String,
    pub label: String,
}

/// An input reported during this session, kept after it is unplugged.
#[derive(Debug, Clone, PartialEq, Eq)]
struct KnownMicrophone {
    input: MicrophoneInput,
    present: bool,
}

/// Every audio input the host reported this session, in first-report order. An input keeps its
/// place once seen, so a choice index read from an older list still names the same input.
#[derive(Resource, Default, Debug, Clone, PartialEq, Eq)]
pub struct MicrophoneDevices {
    known: Vec<KnownMicrophone>,
}

impl MicrophoneDevices {
    /// Whether the host's latest report listed `id`.
    pub fn offers(&self, id: &str) -> bool {
        self.known
            .iter()
            .any(|known| known.present && known.input.id == id)
    }

    /// How many inputs the host's latest report listed.
    pub fn offered(&self) -> usize {
        self.known.iter().filter(|known| known.present).count()
    }

    /// Every known input's id, in choice order.
    pub fn ids(&self) -> Vec<String> {
        self.known
            .iter()
            .map(|known| known.input.id.clone())
            .collect()
    }

    /// Every known input's name, in choice order.
    pub fn labels(&self) -> Vec<String> {
        self.known.iter().map(Self::shown).collect()
    }

    fn shown(known: &KnownMicrophone) -> String {
        if known.present {
            known.input.label.clone()
        } else {
            format!("{} (unplugged)", known.input.label)
        }
    }

    /// Takes the host's current inputs.
    pub fn replace(&mut self, reported: Vec<MicrophoneInput>) {
        for known in self.known.iter_mut() {
            known.present = reported.iter().any(|input| input.id == known.input.id);
        }
        for input in reported {
            match self
                .known
                .iter_mut()
                .find(|known| known.input.id == input.id)
            {
                Some(known) => known.input.label = input.label,
                None => self.known.push(KnownMicrophone {
                    input,
                    present: true,
                }),
            }
        }
    }
}

#[derive(Resource, Default, Serialize, Deserialize, Clone)]
pub struct MicState {
    pub available: bool,
    pub enabled: bool,
    /// Nearby voice is off where the player stands: the scene turns it off, or the scene room
    /// refused the player as banned. Nobody nearby is heard and the mic stays closed. Written
    /// through [`MicState::set_scene_disabled`] and [`MicState::set_scene_banned`].
    #[serde(default)]
    scene_blocked: bool,
    /// The scene the player stands in turns nearby voice off, and the player's `DisableVoice`
    /// permission allows it.
    #[serde(skip)]
    scene_disabled: bool,
    /// The current scene's room refused the player as banned.
    #[serde(skip)]
    scene_banned: bool,
    /// Counts the mic opens the scene refused.
    #[serde(default)]
    pub refusals: u32,
}

impl MicState {
    /// This state with `enabled` false while the client window lacks focus, since the mic is
    /// paused then.
    pub fn gated_by_client_window_focus(&self, focus: ClientWindowFocus) -> Self {
        Self {
            enabled: self.enabled && focus == ClientWindowFocus::Focused,
            ..self.clone()
        }
    }

    pub const fn scene_blocked(&self) -> bool {
        self.scene_blocked
    }

    pub fn scene_disabled(&self) -> bool {
        self.scene_disabled
    }

    pub fn scene_banned(&self) -> bool {
        self.scene_banned
    }

    /// Leaving the scene does not reopen the mic.
    pub fn set_scene_disabled(&mut self, disabled: bool) {
        self.scene_disabled = disabled;
        self.update_scene_blocked();
    }

    /// Being admitted again does not reopen the mic.
    pub fn set_scene_banned(&mut self, banned: bool) {
        self.scene_banned = banned;
        self.update_scene_blocked();
    }

    fn update_scene_blocked(&mut self) {
        self.scene_blocked = self.scene_disabled || self.scene_banned;
        if self.scene_blocked {
            self.enabled = false;
        }
    }

    pub fn refuse(&mut self) {
        self.refusals = self.refusals.wrapping_add(1);
    }

    /// Closing the mic is never refused.
    pub fn request_enabled(&mut self, enabled: bool) {
        if enabled && self.available && self.scene_blocked {
            self.refuse();
        }
        self.enabled = self.available && !self.scene_blocked && enabled;
    }
}

#[derive(Debug, Resource, Default)]
pub struct PreviewMode {
    pub server: Option<String>,
    pub is_preview: bool,
    pub preview_parcel: Option<IVec2>,
}

/// Render out-of-bounds geometry (dithered) instead of culling it, and keep its colliders on.
/// Set at startup (preview, a loopback realm, or editor mode — never a public realm); read by
/// scene_material's show-outside-bounds observer and by the scene collider data.
#[derive(Debug, Resource, Default)]
pub struct ShowOutOfBounds(pub bool);

impl ShowOutOfBounds {
    /// The switch for a run on `realm`: on in the editor, in preview and on a loopback realm
    /// (local development). A run with no realm of its own passes none.
    pub fn for_run(editor: bool, preview: bool, realm: Option<&str>) -> Self {
        Self(editor || preview || realm.is_some_and(crate::util::is_loopback_url))
    }
}

/// True when the explorer is embedded in a scene editor (the explicit `--editor` arg /
/// `editor` web boot param — the editor host is expected to unfreeze scenes when the
/// user hits play). Set once at startup; freezes a scene after main() has run once so
/// the initial state is deterministic. Default false; NOT implied by preview or a
/// loopback realm — plain previews must free-run.
#[derive(Debug, Resource, Default)]
pub struct EditorMode(pub bool);

// resource into which systems can add debug info
#[derive(Resource, Default, Debug)]
pub struct DebugInfo {
    pub info: HashMap<&'static str, String>,
}

#[derive(Clone, Serialize, Deserialize)]
pub enum GlobalCrdtStateUpdate {
    Crdt(Vec<u8>, dcl_component::Localizer),
    Time(TimeOfDay),
    /// Vertical FOV of the active camera, in radians. Pushed when the value
    /// changes (camera zoom etc.).
    CameraFov(f32),
}

/// The animation graph nodes an avatar or a glTF container exposes: the default clip, and the
/// named clips with the time each was last played. Scenes drive them through their animator,
/// avatars through emote playback.
#[derive(Component, Default)]
pub struct Clips {
    pub default: Option<AnimationNodeIndex>,
    pub named: HashMap<String, (AnimationNodeIndex, f32)>,
}

/// A remote player's avatar; comms' `ForeignPlayer` requires it. Avatar systems that
/// treat remote and local emotes differently read this instead of the comms type.
#[derive(Component, Default, Debug, Clone, Copy)]
pub struct ForeignAvatar;

// used for responses to scenes which require strict monotonic timestamps
// by convention we use T = the protobuf struct containing the timestamp (e.g. PbPointerEventsResult, PbTriggerAreaResult)
#[derive(Resource)]
pub struct MonotonicTimestamp<T>(AtomicU32, PhantomData<fn() -> T>);

impl<T> Default for MonotonicTimestamp<T> {
    fn default() -> Self {
        Self(Default::default(), Default::default())
    }
}

impl<T> MonotonicTimestamp<T> {
    pub fn next_timestamp(&self) -> u32 {
        self.0.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ParcelGrassSetting {
    Off,
    #[default]
    Low,
    Mid,
    High,
}

impl Deref for ParcelGrassSetting {
    type Target = ParcelGrassConfig;

    fn deref(&self) -> &Self::Target {
        match self {
            Self::Off => &ParcelGrassConfig {
                layers: 0,
                subdivisions: 32,
                y_displacement: 0.04,
                root_color: Color::Srgba(ParcelGrassConfig::ROOT_COLOR),
                tip_color: Color::Srgba(ParcelGrassConfig::TIP_COLOR),
            },
            Self::Low => &ParcelGrassConfig {
                layers: 8,
                subdivisions: 32,
                y_displacement: 0.02,
                root_color: Color::Srgba(ParcelGrassConfig::ROOT_COLOR),
                tip_color: Color::Srgba(ParcelGrassConfig::TIP_COLOR),
            },
            Self::Mid => &ParcelGrassConfig {
                layers: 16,
                subdivisions: 32,
                y_displacement: 0.02,
                root_color: Color::Srgba(ParcelGrassConfig::ROOT_COLOR),
                tip_color: Color::Srgba(ParcelGrassConfig::TIP_COLOR),
            },
            Self::High => &ParcelGrassConfig {
                layers: 32,
                subdivisions: 32,
                y_displacement: 0.01,
                root_color: Color::Srgba(ParcelGrassConfig::ROOT_COLOR),
                tip_color: Color::Srgba(ParcelGrassConfig::TIP_COLOR),
            },
        }
    }
}

#[derive(Clone, Copy, Resource, Serialize, Deserialize)]
pub struct ParcelGrassConfig {
    pub layers: u32,
    pub subdivisions: u32,
    pub y_displacement: f32,
    pub root_color: Color,
    pub tip_color: Color,
}

impl ParcelGrassConfig {
    pub const ROOT_COLOR: Srgba = palettes::tailwind::LIME_800;
    pub const TIP_COLOR: Srgba = palettes::tailwind::LIME_600;
}

impl Default for ParcelGrassConfig {
    fn default() -> Self {
        Self {
            layers: 32,
            subdivisions: 32,
            y_displacement: 0.01,
            root_color: Self::ROOT_COLOR.into(),
            tip_color: Self::TIP_COLOR.into(),
        }
    }
}

#[derive(Resource, Default, Debug)]
pub struct CurrentRealm {
    pub about_url: String,
    pub address: String,
    pub connected: bool,
    pub config: ServerConfiguration,
    pub comms: Option<CommsConfig>,
    pub public_url: String,
    /// The parcel the realm (a world) spawns players on, from its manifest; none for a city
    pub spawn_parcel: Option<IVec2>,
}

impl CurrentRealm {
    /// Serves a project being developed (a `dcl start` server, or the web editor's preview), which
    /// it signals by listing the project's parcels in its `about`.
    pub fn is_local(&self) -> bool {
        self.config
            .local_scene_parcels
            .as_ref()
            .is_some_and(|parcels| !parcels.is_empty())
    }
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct CommsConfig {
    pub healthy: bool,
    pub protocol: String,
    pub fixed_adapter: Option<String>,
    pub adapter: Option<String>,
}

#[derive(Deserialize, Debug, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct ServerConfiguration {
    pub scenes_urn: Option<Vec<String>>,
    pub realm_name: Option<String>,
    pub network_id: Option<u32>,
    pub city_loader_content_server: Option<String>,
    pub map: Option<MapData>,
    pub local_scene_parcels: Option<Vec<String>>,
}

impl ServerConfiguration {
    /// Whether the realm lists its scenes up front: a World, and also some previews and the
    /// editor's realm. A city lists none and resolves its scenes around the player.
    pub fn lists_scenes(&self) -> bool {
        self.scenes_urn
            .as_ref()
            .is_some_and(|urns| !urns.is_empty())
    }
}

#[derive(Deserialize, Debug, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct MapData {
    pub minimap_enabled: Option<bool>,
    #[serde(default, deserialize_with = "lenient_satellite_view")]
    pub satellite_view: Option<SatelliteView>,
    pub sizes: Vec<Region>,
}

// A malformed satellite view is treated as absent rather than failing the whole `/about`.
fn lenient_satellite_view<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<SatelliteView>, D::Error> {
    let value = serde_json::Value::deserialize(deserializer)?;
    Ok(serde_json::from_value(value).ok())
}

#[derive(Deserialize, Debug, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct Region {
    pub left: i32,
    pub right: i32,
    pub top: i32,
    pub bottom: i32,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PointAtMarkerVisibility {
    #[default]
    All,
    Friends,
    None,
}

/// Which chat messages open a bubble over their sender's nametag.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChatBubbleVisibility {
    #[default]
    All,
    NearbyOnly,
    None,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CameraSmoothing {
    Raw,
    #[default]
    Smoothed,
    Drunk,
}

impl CameraSmoothing {
    /// exponential smoothing rate; `None` means no smoothing
    pub fn rate(&self) -> Option<f32> {
        match self {
            CameraSmoothing::Raw => None,
            CameraSmoothing::Smoothed => Some(15.0),
            CameraSmoothing::Drunk => Some(5.0),
        }
    }

    /// This frame's look delta: `raw` filtered against `smoothed`, the previous frame's result,
    /// which is updated in place.
    pub fn smooth(&self, raw: Vec2, smoothed: &mut Vec2, dt: f32) -> Vec2 {
        match self.rate() {
            Some(rate) => {
                let factor = 1.0 - (-dt * rate).exp();
                *smoothed = factor * raw + (1.0 - factor) * *smoothed;
            }
            None => *smoothed = raw,
        }
        *smoothed
    }
}

#[derive(Event)]
pub struct PointAtMarkerVisibilityChanged;

/// The local player's language as a BCP-47 tag: the chosen one, else the system's. `None` where
/// no player is present (headless).
#[derive(Resource, Clone, Debug, Default, PartialEq, Eq)]
pub struct PlayerLanguage(pub Option<String>);

/// Who may DM a user.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum DmPrivacy {
    #[default]
    All,
    OnlyFriends,
}

impl From<PrivateMessagePrivacySetting> for DmPrivacy {
    fn from(value: PrivateMessagePrivacySetting) -> Self {
        match value {
            PrivateMessagePrivacySetting::All => DmPrivacy::All,
            PrivateMessagePrivacySetting::OnlyFriends => DmPrivacy::OnlyFriends,
        }
    }
}

impl From<DmPrivacy> for PrivateMessagePrivacySetting {
    fn from(value: DmPrivacy) -> Self {
        match value {
            DmPrivacy::All => PrivateMessagePrivacySetting::All,
            DmPrivacy::OnlyFriends => PrivateMessagePrivacySetting::OnlyFriends,
        }
    }
}

/// Another user's DM privacy, as returned by `GetPrivateMessagesSettings`.
#[derive(Clone, Debug)]
pub struct DmPrivacyOf {
    pub address: String,
    pub privacy: DmPrivacy,
}

/// A scene's javascript, as loaded from its content
#[derive(Asset, Debug, Clone, TypePath)]
pub struct SceneJsFile(pub Arc<String>);

/// A world password the HUD must ask for (see `ipfs::world_access`); answered with the password,
/// or `None` when the player gives up. Dropped unanswered when the realm change no longer needs it.
pub struct WorldPasswordPrompt {
    pub world: String,
    /// Why the previous answer was refused; `None` on the first ask
    pub error: Option<String>,
    pub answer: tokio::sync::oneshot::Sender<Option<String>>,
}

/// Where world password prompts go, installed by whatever shows them (the system bridge). Without
/// it a password-protected world can't be entered.
#[derive(Resource, Clone)]
pub struct WorldPasswordPrompter(pub tokio::sync::mpsc::UnboundedSender<WorldPasswordPrompt>);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inputs::CommonInputAction;

    /// the removed `cel_shading` field in a saved config must not stop it loading
    #[test]
    fn old_config_with_cel_shading_still_loads() {
        let mut graphics = serde_json::to_value(GraphicsSettings::default()).unwrap();
        graphics["cel_shading"] = serde_json::Value::Bool(false);
        let config: AppConfig =
            serde_json::from_value(serde_json::json!({ "graphics": graphics })).unwrap();
        assert_eq!(
            serde_json::to_value(&config.graphics).unwrap(),
            serde_json::to_value(GraphicsSettings::default()).unwrap()
        );
    }

    #[test]
    fn out_of_bounds_geometry_shows_in_the_editor_in_preview_and_on_a_loopback_realm() {
        let public = Some("https://realm.example.test/main");
        assert!(!ShowOutOfBounds::for_run(false, false, public).0);
        assert!(ShowOutOfBounds::for_run(true, false, public).0);
        assert!(ShowOutOfBounds::for_run(false, true, public).0);
        assert!(ShowOutOfBounds::for_run(false, false, Some("http://127.0.0.1:8000")).0);
        assert!(ShowOutOfBounds::for_run(false, false, Some("http://localhost:8000")).0);
        // a preview server reached by its LAN address
        assert!(!ShowOutOfBounds::for_run(false, false, Some("http://192.168.1.20:8000")).0);
        // no realm of its own
        assert!(!ShowOutOfBounds::for_run(false, false, None).0);
        assert!(ShowOutOfBounds::for_run(false, true, None).0);
    }

    #[test]
    fn fog_reads_the_old_variants_as_on() {
        for old in ["\"Basic\"", "\"Atmospheric\"", "\"On\""] {
            assert_eq!(
                serde_json::from_str::<FogSetting>(old).unwrap(),
                FogSetting::On
            );
        }
    }

    #[test]
    fn grading_brightens_shadows_and_keeps_gamma_neutral() {
        let g = default_color_grading();
        // the shadows weight w adds 4w to the (1, 1, 1) multiplier
        assert_eq!(g.shadows.gain, 1.0 + 4.0 * GRADING_SHADOWS_WEIGHT);
        assert_eq!(g.shadows.lift, 0.0);
        assert_eq!(
            (g.shadows.gamma, g.midtones.gamma, g.highlights.gamma),
            (1.0, 1.0, 1.0)
        );
        assert_eq!(g.global.midtones_range, GRADING_MIDTONES_RANGE);
    }

    const TONEMAPPING_SHADER: &str = include_str!(concat!(
        env!("DCL_ONE_ROOT"),
        "/rendering/bevy-explorer/bevy/crates/bevy_core_pipeline/src/tonemapping/tonemapping_shared.wgsl"
    ));

    /// bevy_core_pipeline's `sectional_color_grading` (tonemapping_shared.wgsl): the weight of
    /// the shadows section for a colour whose channels average `level`.
    fn bevy_shadows_weight(level: f32) -> f32 {
        const LEVEL_MARGIN: f32 = 0.1;
        let midtones = (level - GRADING_MIDTONES_RANGE.start) / (2.0 * LEVEL_MARGIN) + 0.5;
        1.0 - midtones.clamp(0.0, 1.0)
    }

    /// The shader the function above stands for: the shadows section fades into the midtones.
    /// With the fade given to the highlights instead, the shadow gain ends at a hard step, and
    /// a pixel just above the step comes out darker than one just below it: a dark rim along
    /// every shadow edge, and blotches wherever a shaded surface straddles the step.
    #[test]
    fn bevy_fades_the_shadows_section_into_the_midtones() {
        assert!(TONEMAPPING_SHADER.contains("const LEVEL_MARGIN: f32 = 0.1;"));
        let fade = TONEMAPPING_SHADER
            .split("level < midtone_range.x + LEVEL_MARGIN")
            .nth(1)
            .and_then(|rest| rest.split("} else if").next())
            .expect("the shadows to midtones fade");
        assert!(fade.contains("levels.x = 1.0 - levels.y;"), "{fade}");
    }

    #[test]
    fn grading_shadow_band_follows_the_smoothstep() {
        // the target band: `1 - smoothstep(start, end, luma)` with the
        // limits 0 and 0.3
        let target = |level: f32| {
            let t = (level / 0.3).clamp(0.0, 1.0);
            1.0 - t * t * (3.0 - 2.0 * t)
        };
        for step in 0..=100 {
            let level = step as f32 / 100.0;
            let (ours, theirs) = (bevy_shadows_weight(level), target(level));
            assert!((ours - theirs).abs() < 0.08, "{level}: {ours} vs {theirs}");
        }
    }

    #[test]
    fn grading_never_darkens_a_brighter_pixel() {
        let gain = default_color_grading().shadows.gain;
        let graded = |level: f32| level * (1.0 + (gain - 1.0) * bevy_shadows_weight(level));
        for step in 0..1000 {
            let (darker, brighter) = (step as f32 / 1000.0, (step + 1) as f32 / 1000.0);
            assert!(graded(darker) < graded(brighter), "{darker}");
        }
    }

    /// bevy_core_pipeline's `compute_blend_factor` (bloom/mod.rs): the blend constant of the
    /// upsample pass that adds mip `mip` of `max_mip` into the finer one; mip 0 is the composite.
    fn bevy_bloom_blend(b: &bevy::core_pipeline::bloom::Bloom, mip: f32, max_mip: f32) -> f32 {
        let lf_boost = (1.0
            - (1.0 - mip / max_mip).powf(1.0 / (1.0 - b.low_frequency_boost_curvature)))
            * b.low_frequency_boost;
        b.intensity + lf_boost
    }

    #[test]
    fn bloom_weighs_mips_by_scatter() {
        // the Explorer's scatter and the avatar preview's
        for setting in [BLOOM_SCATTER, 0.6] {
            let b = scatter_bloom(setting);
            assert_eq!(b.prefilter.threshold, 1.0);
            // the knee is threshold * 0.5; bevy: knee = threshold * softness
            assert_eq!(b.prefilter.threshold_softness, 0.5);
            assert_eq!(b.high_pass_frequency, 1.0);
            // a scatter bloom folds lerp(fine, coarse, s) over the chain and adds the result once:
            // mip i weighs (1 - s) s^i. bevy adds mip i with weight blend(0) * blend(1) * .. * blend(i).
            let s = scatter_lerp(setting);
            let max_mip = 7.0;
            let mut weight = 1.0;
            for mip in 0..=7 {
                weight *= bevy_bloom_blend(&b, mip as f32, max_mip);
                let scatter = (1.0 - s) * s.powi(mip);
                assert!(
                    (weight - scatter).abs() < 0.002,
                    "scatter {setting}, mip {mip}: bevy {weight} vs scatter {scatter}"
                );
            }
        }
    }

    #[test]
    fn landscape_distance_from_config_is_clamped() {
        let from = |metres| LandscapeDistance::from_config(metres).0;
        assert_eq!(from(1000.0), 1000.0);
        assert_eq!(from(0.0), LandscapeDistance::MIN);
        assert_eq!(from(-50.0), LandscapeDistance::MIN);
        assert_eq!(from(20_000.0), LandscapeDistance::MAX);
        assert_eq!(from(f32::INFINITY), LandscapeDistance::MAX);
        assert_eq!(from(f32::NAN), LandscapeDistance::default().0);
    }

    #[test]
    fn sun_cascades_split_the_shadow_distance() {
        let high = sun_cascades(ShadowSetting::High, 100.0).unwrap();
        assert_eq!(high.bounds, vec![10.2, 28.2, 47.4, 100.0]);
        assert_eq!(high.minimum_distance, MIN_CASCADE);
        let medium = sun_cascades(ShadowSetting::Low, 20.0).unwrap();
        assert_eq!(medium.bounds, vec![20.0]);
        // Low's one cascade stops at 40 m; High's four reach the whole distance
        for distance in [40.0, 60.0, 300.0] {
            let low = sun_cascades(ShadowSetting::Low, distance).unwrap();
            assert_eq!(low.bounds, vec![SUN_LOW_MAX_DISTANCE]);
        }
        let far = sun_cascades(ShadowSetting::High, 300.0).unwrap();
        assert_eq!(far.bounds.last(), Some(&300.0));
        assert!(sun_cascades(ShadowSetting::Off, 60.0).is_none());
        assert!(sun_cascades(ShadowSetting::High, 0.0).is_none());
        // a distance under the minimum still yields a strictly increasing config
        let tiny = sun_cascades(ShadowSetting::High, 0.05).unwrap();
        assert!(tiny.bounds.windows(2).all(|w| w[0] < w[1]) && tiny.bounds[0] > MIN_CASCADE);
    }

    #[test]
    fn a_config_saved_before_the_voice_chat_volume_keeps_its_audio() {
        let config = AppConfig::from_json(
            br#"{"user_id":"u","audio":{"master":80,"voice":40,"scene":30,"system":20,"avatar":10}}"#,
        );
        assert_eq!(config.user_id, "u");
        assert_eq!(
            config.audio,
            AudioSettings {
                master: 80,
                voice: 40,
                voice_chat: 100,
                scene: 30,
                system: 20,
                avatar: 10,
                mute_mic_in_background: true,
                microphone: None,
            }
        );
    }

    #[test]
    fn the_mic_follows_the_window_focus_only_while_it_mutes_in_the_background() {
        let mut audio = AudioSettings::default();
        assert_eq!(
            audio.microphone_focus(ClientWindowFocus::NoFocus),
            ClientWindowFocus::NoFocus
        );
        audio.mute_mic_in_background = false;
        assert_eq!(
            audio.microphone_focus(ClientWindowFocus::NoFocus),
            ClientWindowFocus::Focused
        );
    }

    #[test]
    fn the_mic_stays_closed_after_leaving_a_scene_that_turns_nearby_voice_off() {
        let mut mic = MicState {
            available: true,
            enabled: true,
            ..Default::default()
        };
        mic.set_scene_disabled(true);
        assert!(mic.scene_blocked && !mic.enabled);
        mic.set_scene_disabled(false);
        assert!(!mic.scene_blocked && !mic.enabled);
    }

    #[test]
    fn a_scene_ban_and_a_scene_turning_voice_off_block_nearby_voice_until_both_clear() {
        let mut mic = MicState {
            available: true,
            enabled: true,
            ..Default::default()
        };
        mic.set_scene_banned(true);
        assert!(mic.scene_blocked && !mic.enabled, "a ban closes the mic");
        mic.set_scene_disabled(true);
        mic.set_scene_banned(false);
        assert!(mic.scene_blocked, "the scene still turns voice off");
        mic.request_enabled(true);
        assert!(!mic.enabled);
        mic.set_scene_disabled(false);
        assert!(!mic.scene_blocked && !mic.enabled);
        mic.request_enabled(true);
        assert!(mic.enabled);
    }

    #[test]
    fn a_scene_that_turns_nearby_voice_off_counts_each_refused_open() {
        let mut mic = MicState {
            available: true,
            enabled: false,
            ..Default::default()
        };
        mic.set_scene_disabled(true);
        mic.request_enabled(true);
        assert!(!mic.enabled);
        assert_eq!(mic.refusals, 1);
        mic.request_enabled(false);
        assert_eq!(mic.refusals, 1, "closing the mic is never refused");

        mic.available = false;
        mic.request_enabled(true);
        assert_eq!(mic.refusals, 1, "no mic to refuse");

        mic.available = true;
        mic.set_scene_disabled(false);
        mic.request_enabled(true);
        assert!(mic.enabled);
        assert_eq!(mic.refusals, 1);
    }

    #[test]
    fn a_scene_turns_nearby_voice_off_with_any_value_but_enabled() {
        let enabled = |toggles: &str| {
            let meta: SceneMeta = serde_json::from_str(&format!(
                r#"{{"main":"index.js","scene":{{"base":"0,0","parcels":["0,0"]}}{toggles}}}"#
            ))
            .unwrap();
            meta.nearby_voice_chat_enabled()
        };
        assert!(enabled(""));
        assert!(enabled(r#","featureToggles":{}"#));
        assert!(enabled(r#","featureToggles":{"voiceChat":"disabled"}"#));
        assert!(enabled(r#","featureToggles":{"nearbyVoiceChat":""}"#));
        assert!(enabled(
            r#","featureToggles":{"nearbyVoiceChat":"enabled"}"#
        ));
        assert!(enabled(
            r#","featureToggles":{"nearbyVoiceChat":"ENABLED"}"#
        ));
        assert!(!enabled(
            r#","featureToggles":{"nearbyVoiceChat":"disabled"}"#
        ));
        assert!(!enabled(r#","featureToggles":{"nearbyVoiceChat":"off"}"#));
        // values of another shape are ignored rather than failing the scene.json
        assert!(enabled(r#","featureToggles":{"nearbyVoiceChat":false}"#));
        assert!(enabled(r#","featureToggles":{"nearbyVoiceChat":1}"#));
        assert!(enabled(r#","featureToggles":["disabled"]"#));
    }

    #[test]
    fn reset_outdated_settings_caps_uncapped_uploads() {
        let saved = |gpu_bytes_per_frame| {
            let mut config = AppConfig {
                settings_generation: 2,
                ..Default::default()
            };
            config.graphics.gpu_bytes_per_frame = gpu_bytes_per_frame;
            config.reset_outdated_settings();
            config
        };
        let uncapped = saved(0);
        assert_eq!(uncapped.graphics.gpu_bytes_per_frame, 10_000_000);
        assert_eq!(uncapped.settings_generation, SETTINGS_GENERATION);
        assert_eq!(saved(500_000).graphics.gpu_bytes_per_frame, 500_000);
    }

    #[test]
    fn reset_outdated_settings_moves_render_scale_to_a_ten_percent_step() {
        for (saved, migrated) in [
            (0.97, 1.0),
            (0.84, 0.8),
            (0.53, 0.5),
            (0.3, 0.5),
            (0.7, 0.7),
        ] {
            let mut config = AppConfig {
                settings_generation: 4,
                ..Default::default()
            };
            config.graphics.render_scale = saved;
            config.reset_outdated_settings();
            assert_eq!(config.graphics.render_scale, migrated);
        }
    }

    #[test]
    fn reset_outdated_settings_moves_movement_still_on_the_old_defaults() {
        // written by the old build and read back through JSON, as at startup
        let saved = |settings_generation, walk_speed| {
            let old = AppConfig {
                settings_generation,
                player_settings: PrimaryUser {
                    walk_speed,
                    jog_speed: 8.18,
                    run_speed: 11.0,
                    jump_height: 1.9,
                    run_jump_height: 2.95,
                    ..Default::default()
                },
                ..Default::default()
            };
            let mut config = AppConfig::from_json(&serde_json::to_vec(&old).unwrap());
            config.reset_outdated_settings();
            config
        };
        let moved = |user: &PrimaryUser| {
            (
                user.jog_speed,
                user.run_speed,
                user.jump_height,
                user.run_jump_height,
            )
        };

        let config = saved(5, 2.5);
        assert_eq!(config.player_settings.walk_speed, 1.5);
        assert_eq!(moved(&config.player_settings), (8.0, 10.0, 1.0, 1.5));
        assert_eq!(config.settings_generation, SETTINGS_GENERATION);

        // a value the player set stays
        let config = saved(5, 3.2);
        assert_eq!(config.player_settings.walk_speed, 3.2);
        assert_eq!(moved(&config.player_settings), (8.0, 10.0, 1.0, 1.5));

        // a config already on the current generation chose these values
        let config = saved(SETTINGS_GENERATION, 2.5);
        assert_eq!(config.player_settings.walk_speed, 2.5);
        assert_eq!(moved(&config.player_settings), (8.18, 11.0, 1.9, 2.95));
    }

    #[test]
    fn default_graphics_are_the_medium_preset() {
        let g = GraphicsSettings::default();
        assert_eq!((g.fps_target, g.render_scale), (60, 1.0));
        assert_eq!((g.bloom, g.avatar_outline), (BloomSetting::On, true));
        assert_eq!(
            (
                g.shadow_settings,
                g.shadow_distance,
                g.light_count,
                g.shadow_caster_count
            ),
            (ShadowSetting::Low, 20.0, 6, 3)
        );
        assert_eq!(
            (g.dof, g.ssao, g.fog, g.sky_reflections),
            (
                DofSetting::High,
                SsaoSetting::Off,
                FogSetting::On,
                SkyReflectionSetting::High
            )
        );
        assert_eq!(g.msaa, AaSetting::FxaaHigh);
        assert_eq!(g.vsync, !cfg!(target_arch = "wasm32"));
    }

    #[test]
    fn vsync_caps_the_target_at_the_refresh_rate() {
        let mut config = AppConfig::default();
        let paced = |config: &AppConfig, session: Option<&SessionGraphics>, refresh| {
            SessionGraphics::paced_fps(session, config, refresh)
        };
        let thirty = SessionGraphics {
            fps_target: Some(30),
            ..Default::default()
        };
        config.graphics.vsync = true;
        config.graphics.fps_target = 60;
        assert_eq!(paced(&config, None, Some(144)), 60);
        assert_eq!(paced(&config, None, Some(50)), 50);
        // a 60 Hz monitor reads as 61: VSync sets the rate
        assert_eq!(paced(&config, None, Some(61)), 61);
        assert_eq!(paced(&config, Some(&thirty), Some(144)), 30);
        assert_eq!(paced(&config, None, None), 60);
        config.graphics.fps_target = UNCAPPED_FPS;
        assert_eq!(paced(&config, None, Some(144)), 144);
        config.graphics.vsync = false;
        assert_eq!(paced(&config, None, Some(144)), UNCAPPED_FPS);
        assert_eq!(paced(&config, Some(&thirty), Some(144)), 30);
    }

    #[test]
    fn the_shown_rate_is_the_monitors_when_vsync_sets_it() {
        let mut config = AppConfig::default();
        let shown = |config: &AppConfig, refresh| SessionGraphics::shown_fps(None, config, refresh);
        config.graphics.vsync = true;
        config.graphics.fps_target = UNCAPPED_FPS;
        assert_eq!(shown(&config, Some(61)), 60);
        assert_eq!(shown(&config, None), UNCAPPED_FPS);
        config.graphics.fps_target = 60;
        assert_eq!(shown(&config, Some(61)), 60);
        assert_eq!(shown(&config, Some(145)), 60);
        config.graphics.vsync = false;
        config.graphics.fps_target = UNCAPPED_FPS;
        assert_eq!(shown(&config, Some(61)), UNCAPPED_FPS);
    }

    #[test]
    fn unlimited_avatars_lifts_the_cap_but_keeps_the_count_and_yields_to_the_session() {
        let mut config = AppConfig::default();
        let mut session = SessionGraphics::default();
        assert_eq!(session.max_avatars(&config), 20);
        config.max_avatars_unlimited = true;
        assert_eq!(session.max_avatars(&config), SessionGraphics::NO_AVATAR_CAP);
        assert_eq!(config.max_avatars, 20);
        session.max_avatars = Some(5);
        assert_eq!(session.max_avatars(&config), 5);
    }

    #[test]
    fn a_saved_bloom_level_reads_as_on() {
        let read = |saved: &str| serde_json::from_str::<BloomSetting>(saved).unwrap();
        assert_eq!(read(r#""Low""#), BloomSetting::On);
        assert_eq!(read(r#""High""#), BloomSetting::On);
        assert_eq!(read(r#""On""#), BloomSetting::On);
        assert_eq!(read(r#""Off""#), BloomSetting::Off);
        // saved under the old name an older client reads
        let write = |setting| serde_json::to_string(&setting).unwrap();
        assert_eq!(write(BloomSetting::On), r#""High""#);
        assert_eq!(write(BloomSetting::Off), r#""Off""#);
    }

    #[test]
    fn generation_4_lands_on_medium_and_keeps_the_rest() {
        let mut config = AppConfig {
            settings_generation: 3,
            scene_load_distance: 100.0,
            max_avatars: 50,
            ..Default::default()
        };
        let g = &mut config.graphics;
        (g.vsync, g.fps_target, g.render_scale) = (false, 144, 0.5);
        (g.msaa, g.bloom, g.dof) = (AaSetting::FxaaLow, BloomSetting::Off, DofSetting::Off);
        (g.shadow_distance, g.light_count, g.shadow_caster_count) = (200.0, 32, 8);
        g.lens_flare = false;
        (g.ambient_brightness, g.oob) = (10, 0.0);
        config.reset_outdated_settings();

        let mut medium = GraphicsSettings::default();
        (medium.ambient_brightness, medium.oob) = (10, 0.0);
        assert_eq!(
            serde_json::to_value(&config.graphics).unwrap(),
            serde_json::to_value(&medium).unwrap()
        );
        assert_eq!(
            (config.scene_load_distance, config.max_avatars),
            (100.0, 50)
        );
        assert_eq!(config.settings_generation, SETTINGS_GENERATION);
    }

    #[test]
    fn migrate_inputs_updates_old_tables() {
        // a table saved before Places/Gallery/etc existed, with old roll defaults and
        // one deliberate user customization
        let mut config = AppConfig {
            inputs: InputMapSerialized(
                vec![
                    (
                        Action::System(SystemAction::RollLeft),
                        vec![InputIdentifier::Key(KeyCode::KeyT)],
                    ),
                    (
                        Action::System(SystemAction::RollRight),
                        vec![InputIdentifier::Key(KeyCode::KeyR)],
                    ),
                    // a fixed binding the user managed to strip (older build)
                    (Action::System(SystemAction::ScrollUp), vec![]),
                    // legacy action from an old default table
                    (
                        Action::System(SystemAction::ShowProfile),
                        vec![InputIdentifier::Gamepad(
                            bevy::input::gamepad::GamepadButton::North,
                        )],
                    ),
                    // quick emote still on its old digit default, and one rebound
                    (
                        Action::System(SystemAction::QuickEmote3),
                        vec![InputIdentifier::Key(KeyCode::Digit3)],
                    ),
                    (
                        Action::System(SystemAction::QuickEmote4),
                        vec![InputIdentifier::Key(KeyCode::KeyX)],
                    ),
                    // a default binding that a photo-mode default shares
                    (
                        Action::Scene(CommonInputAction::IaPrimary),
                        vec![InputIdentifier::Key(KeyCode::KeyE)],
                    ),
                ],
                Default::default(),
            ),
            inputs_generation: 0,
            ..Default::default()
        };

        config.migrate_inputs();

        fn get(config: &AppConfig, action: SystemAction) -> Option<Vec<InputIdentifier>> {
            config
                .inputs
                .0
                .iter()
                .find(|(a, _)| *a == Action::System(action))
                .map(|(_, bindings)| bindings.clone())
        }

        // old default cleared, user customization preserved
        assert_eq!(get(&config, SystemAction::RollLeft), Some(vec![]));
        assert_eq!(
            get(&config, SystemAction::RollRight),
            Some(vec![InputIdentifier::Key(KeyCode::KeyR)])
        );
        // missing actions merged in with their defaults
        assert_eq!(
            get(&config, SystemAction::Places),
            Some(vec![InputIdentifier::Key(KeyCode::KeyZ)])
        );
        // ...but not onto a key the user gave another action: X stays QuickEmote4's alone
        assert_eq!(get(&config, SystemAction::Events), Some(vec![]));
        // a key the defaults share on purpose still merges
        let photo_tilt_right = InputMapSerialized::default()
            .0
            .into_iter()
            .find(|(a, _)| *a == Action::System(SystemAction::PhotoTiltRight))
            .map(|(_, bindings)| bindings);
        assert!(photo_tilt_right
            .as_ref()
            .is_some_and(|b| b.contains(&InputIdentifier::Key(KeyCode::KeyE))));
        assert_eq!(get(&config, SystemAction::PhotoTiltRight), photo_tilt_right);
        assert_eq!(config.inputs_generation, INPUTS_GENERATION);
        // fixed bindings restored: the wheel always scrolls
        assert_eq!(
            get(&config, SystemAction::ScrollUp),
            Some(vec![InputIdentifier::Analog(
                crate::inputs::AxisIdentifier::MouseWheel,
                crate::inputs::InputDirection::Up
            )])
        );
        // the legacy ShowProfile row is stripped, not merged back
        assert_eq!(get(&config, SystemAction::ShowProfile), None);
        // quick emotes: old digit default remapped to the numpad, a rebind preserved
        assert_eq!(
            get(&config, SystemAction::QuickEmote3),
            Some(vec![InputIdentifier::Key(KeyCode::Numpad3)])
        );
        assert_eq!(
            get(&config, SystemAction::QuickEmote4),
            Some(vec![InputIdentifier::Key(KeyCode::KeyX)])
        );

        // re-running after a deliberate rebind back to KeyT doesn't clear it again
        let roll_left = config
            .inputs
            .0
            .iter_mut()
            .find(|(a, _)| *a == Action::System(SystemAction::RollLeft))
            .unwrap();
        roll_left.1 = vec![InputIdentifier::Key(KeyCode::KeyT)];
        config.migrate_inputs();
        assert_eq!(
            get(&config, SystemAction::RollLeft),
            Some(vec![InputIdentifier::Key(KeyCode::KeyT)])
        );
    }

    #[test]
    fn migrate_inputs_moves_the_microphone_to_t() {
        fn table(rows: &[(SystemAction, Option<KeyCode>)]) -> AppConfig {
            AppConfig {
                inputs: InputMapSerialized(
                    rows.iter()
                        .map(|(action, key)| {
                            (
                                Action::System(*action),
                                key.map(InputIdentifier::Key).into_iter().collect(),
                            )
                        })
                        .collect(),
                    Default::default(),
                ),
                inputs_generation: 1,
                ..Default::default()
            }
        }
        fn get(config: &AppConfig, action: SystemAction) -> Vec<InputIdentifier> {
            config
                .inputs
                .0
                .iter()
                .find(|(a, _)| *a == Action::System(action))
                .map(|(_, bindings)| bindings.clone())
                .unwrap()
        }
        let t = InputIdentifier::Key(KeyCode::KeyT);
        let v = InputIdentifier::Key(KeyCode::KeyV);

        // both on the old defaults: the microphone takes T, ChatPanel is left unbound
        let mut config = table(&[
            (SystemAction::Microphone, Some(KeyCode::KeyV)),
            (SystemAction::ChatPanel, Some(KeyCode::KeyT)),
        ]);
        config.migrate_inputs();
        assert_eq!(get(&config, SystemAction::Microphone), vec![t]);
        assert_eq!(get(&config, SystemAction::ChatPanel), vec![]);
        assert_eq!(config.inputs_generation, INPUTS_GENERATION);

        // rebinds are kept
        let mut config = table(&[
            (SystemAction::Microphone, Some(KeyCode::KeyB)),
            (SystemAction::ChatPanel, Some(KeyCode::KeyY)),
        ]);
        config.migrate_inputs();
        assert_eq!(
            get(&config, SystemAction::Microphone),
            vec![InputIdentifier::Key(KeyCode::KeyB)]
        );
        assert_eq!(
            get(&config, SystemAction::ChatPanel),
            vec![InputIdentifier::Key(KeyCode::KeyY)]
        );

        // the microphone moved elsewhere by the player: the chat panel keeps T
        let mut config = table(&[
            (SystemAction::Microphone, Some(KeyCode::KeyB)),
            (SystemAction::ChatPanel, Some(KeyCode::KeyT)),
        ]);
        config.migrate_inputs();
        assert_eq!(
            get(&config, SystemAction::Microphone),
            vec![InputIdentifier::Key(KeyCode::KeyB)]
        );
        assert_eq!(get(&config, SystemAction::ChatPanel), vec![t]);

        // no microphone row yet: the chat panel gives up T and the default merges in on T
        let mut config = table(&[(SystemAction::ChatPanel, Some(KeyCode::KeyT))]);
        config.migrate_inputs();
        assert_eq!(get(&config, SystemAction::Microphone), vec![t]);
        assert_eq!(get(&config, SystemAction::ChatPanel), vec![]);

        // T given to another action: the microphone stays on V, the chat panel keeps T
        let mut config = table(&[
            (SystemAction::Microphone, Some(KeyCode::KeyV)),
            (SystemAction::ChatPanel, Some(KeyCode::KeyY)),
            (SystemAction::RollLeft, Some(KeyCode::KeyT)),
        ]);
        config.migrate_inputs();
        assert_eq!(get(&config, SystemAction::Microphone), vec![v]);
        assert_eq!(get(&config, SystemAction::RollLeft), vec![t]);

        // a deliberate rebind of the microphone back to V after the migration stays
        let mut config = table(&[(SystemAction::Microphone, Some(KeyCode::KeyV))]);
        config.inputs_generation = INPUTS_GENERATION;
        config.migrate_inputs();
        assert_eq!(get(&config, SystemAction::Microphone), vec![v]);
    }

    #[test]
    fn malformed_satellite_view_is_absent() {
        let parse = |map: &str| serde_json::from_str::<MapData>(map).unwrap().satellite_view;

        let genesis = parse(
            r#"{"minimapEnabled":true,"sizes":[],"satelliteView":{"version":"v1","baseUrl":"https://genesis.city/map/latest","suffixUrl":".jpg","topLeftOffset":{"x":-2,"y":-6}}}"#,
        )
        .unwrap();
        assert_eq!(genesis.base_url, "https://genesis.city/map/latest");
        assert_eq!(genesis.top_left_offset.y, -6.0);

        assert!(parse(r#"{"minimapEnabled":false,"sizes":[]}"#).is_none());
        assert!(parse(r#"{"sizes":[],"satelliteView":{"topLeftOffset":null}}"#).is_none());
        assert!(parse(r#"{"sizes":[],"satelliteView":{"topLeftOffset":{"x":1}}}"#).is_none());
        assert!(parse(r#"{"sizes":[],"satelliteView":{"version":1}}"#).is_none());
    }

    #[test]
    fn feature_toggles_parse_leniently() {
        let meta = |toggles: &str| -> SceneMeta {
            serde_json::from_str(&format!(
                r#"{{"main":"bin/index.js","scene":{{"base":"0,0","parcels":["0,0"]}}{toggles}}}"#
            ))
            .unwrap()
        };
        assert_eq!(
            meta("").portable_experiences_toggle(),
            PortableExperiencesToggle::Enabled
        );
        assert_eq!(
            meta(r#","featureToggles":{"portableExperiences":"disabled","voiceChat":false}"#)
                .portable_experiences_toggle(),
            PortableExperiencesToggle::Disabled
        );
        assert_eq!(
            meta(r#","featureToggles":{"portableExperiences":"hideUi"}"#)
                .portable_experiences_toggle(),
            PortableExperiencesToggle::HideUi
        );
        // values of another shape are ignored rather than failing the scene.json
        for toggles in [
            r#","featureToggles":{"portableExperiences":1}"#,
            r#","featureToggles":["disabled"]"#,
            r#","featureToggles":{"portableExperiences":"unknown"}"#,
        ] {
            assert_eq!(
                meta(toggles).portable_experiences_toggle(),
                PortableExperiencesToggle::Enabled,
                "{toggles}"
            );
        }
    }

    #[test]
    fn a_realm_lists_scenes_only_with_a_scene_in_its_list() {
        let config = |json: &str| serde_json::from_str::<ServerConfiguration>(json).unwrap();
        assert!(config(r#"{"scenesUrn":["urn:decentraland:entity:a"]}"#).lists_scenes());
        assert!(!config(r#"{"scenesUrn":[]}"#).lists_scenes());
        assert!(!config(r#"{"realmName":"main"}"#).lists_scenes());
    }

    #[test]
    fn is_portable_experience_parses_leniently() {
        let meta = |field: &str| -> SceneMeta {
            serde_json::from_str(&format!(
                r#"{{"main":"bin/index.js","scene":{{"base":"0,0","parcels":["0,0"]}}{field}}}"#
            ))
            .unwrap()
        };
        assert!(meta(r#","isPortableExperience":true"#).declares_portable_experience());
        for field in [
            "",
            r#","isPortableExperience":false"#,
            r#","isPortableExperience":"true""#,
            r#","isPortableExperience":1"#,
        ] {
            assert!(!meta(field).declares_portable_experience(), "{field}");
        }
    }

    fn home(realm: &str, x: i32, y: i32) -> PinnedHome {
        PinnedHome {
            realm: realm.to_owned(),
            location: IVec2::new(x, y),
        }
    }

    #[test]
    fn an_account_with_no_home_starts_at_the_default() {
        let mut config = AppConfig::default();
        assert_eq!(config.pinned_home(Some("0xa")), None);
        assert_eq!(config.startup_home(), None);
        assert_eq!(config.home_location(), IVec2::ZERO);
        assert_eq!(config.home_realm(), default_home_realm());

        config.homes.insert("0xa".into(), home("w.dcl.eth", 0, 0));
        assert_eq!(config.pinned_home(Some("0xb")), None);
        assert_eq!(config.pinned_home(None), None);
    }

    #[test]
    fn each_account_keeps_its_own_home_and_clearing_one_leaves_the_other() {
        let mut config = AppConfig::default();
        config.homes.insert("0xa".into(), home("realm-a", 3, 4));
        config.homes.insert("0xb".into(), home("realm-b", -5, 6));
        assert_eq!(config.pinned_home(Some("0xa")), Some(home("realm-a", 3, 4)));
        assert_eq!(
            config.pinned_home(Some("0xb")),
            Some(home("realm-b", -5, 6))
        );

        config.homes.remove("0xa");
        assert_eq!(config.pinned_home(Some("0xa")), None);
        assert_eq!(
            config.pinned_home(Some("0xb")),
            Some(home("realm-b", -5, 6))
        );
    }

    #[test]
    fn the_first_account_adopts_the_accountless_home_and_no_later_one_inherits_it() {
        let mut config = AppConfig {
            home_realm: Some("realm-old".into()),
            home_location: Some(IVec2::new(7, 8)),
            ..Default::default()
        };
        assert_eq!(config.startup_home(), Some(home("realm-old", 7, 8)));

        assert!(config.adopt_home("0xa"));
        assert_eq!(
            config.pinned_home(Some("0xa")),
            Some(home("realm-old", 7, 8))
        );
        assert_eq!(
            (config.home_realm.clone(), config.home_location),
            (None, None)
        );
        assert!(!config.adopt_home("0xa"));

        assert!(config.adopt_home("0xb"));
        assert_eq!(config.pinned_home(Some("0xb")), None);
    }

    #[test]
    fn an_account_with_its_own_home_keeps_it_over_the_accountless_one() {
        let mut config = AppConfig {
            home_location: Some(IVec2::new(7, 8)),
            ..Default::default()
        };
        config.homes.insert("0xa".into(), home("realm-a", 1, 2));
        assert!(config.adopt_home("0xa"));
        assert_eq!(config.pinned_home(Some("0xa")), Some(home("realm-a", 1, 2)));
        assert_eq!(config.home_location, None);
    }

    #[test]
    fn startup_uses_the_stored_sign_in_then_the_last_account() {
        let mut config = AppConfig::default();
        let address = Address::repeat_byte(0x11);
        let account = home_account(&address);
        assert_eq!(account, format!("0x{}", "11".repeat(20)));
        config.homes.insert(account.clone(), home("realm-a", 9, 9));
        config.homes.insert("0xb".into(), home("realm-b", 2, 2));

        config.home_last_account = Some("0xb".into());
        assert_eq!(config.home_location(), IVec2::new(2, 2));
        assert_eq!(config.home_realm(), "realm-b");

        config.previous_login = Some(PreviousLogin {
            root_address: address,
            ephemeral_key: Vec::new(),
            auth: Vec::new(),
            guest_account: false,
        });
        assert_eq!(config.home_location(), IVec2::new(9, 9));

        let old: AppConfig =
            serde_json::from_str(r#"{"home_realm":"r","home_location":[1,2]}"#).unwrap();
        assert_eq!(old.startup_home(), Some(home("r", 1, 2)));
    }
}
