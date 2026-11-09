//! The host surface: commands the page sends (`preview_command`, JSON) and events the module
//! reports back (the `onEvent` callback given to `start`). Single-threaded, so a thread-local
//! queue carries commands from the JS call into the next engine frame.

use std::{cell::RefCell, collections::VecDeque, io::Cursor};

use avatar::{AvatarLoadReport, AvatarSelection, AvatarShape};
use base64::{prelude::BASE64_STANDARD, Engine};
use bevy::{
    input::mouse::{MouseMotion, MouseScrollUnit, MouseWheel},
    prelude::*,
    render::view::screenshot::{Screenshot, ScreenshotCaptured},
    window::PrimaryWindow,
};
use collectibles::{
    urn::CollectibleUrn, wearables::Wearable, CollectibleType, Collectibles, Emote,
};
use common::structs::{EmoteCommand, EmoteLifecycle, EmoteLifecycleEvent, EmoteMask};
use dcl_component::proto_components::{
    common::Color3,
    sdk::components::{common::LoadingState, PbAvatarShape},
};
use scene_material::SceneMaterial;
use serde::{Deserialize, Serialize};

use crate::PreviewAvatar;

thread_local! {
    static COMMANDS: RefCell<VecDeque<Command>> = const { RefCell::new(VecDeque::new()) };
    #[cfg(target_arch = "wasm32")]
    static ON_EVENT: RefCell<Option<js_sys::Function>> = const { RefCell::new(None) };
}

/// Queues a command from the page; the next frame applies it.
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn preview_command(json: String) -> Result<(), wasm_bindgen::JsValue> {
    let command: Command = serde_json::from_str(&json)
        .map_err(|e| wasm_bindgen::JsValue::from_str(&format!("preview_command: {e}")))?;
    COMMANDS.with(|queue| queue.borrow_mut().push_back(command));
    Ok(())
}

#[cfg(target_arch = "wasm32")]
pub fn set_event_callback(callback: js_sys::Function) {
    ON_EVENT.with(|slot| *slot.borrow_mut() = Some(callback));
}

/// Hands an event to the page.
pub fn emit(event: &Event) {
    let json = match serde_json::to_string(event) {
        Ok(json) => json,
        Err(e) => {
            error!("event: {e}");
            return;
        }
    };
    #[cfg(target_arch = "wasm32")]
    ON_EVENT.with(|slot| {
        if let Some(callback) = slot.borrow().as_ref() {
            if let Err(e) = callback.call1(
                &wasm_bindgen::JsValue::NULL,
                &wasm_bindgen::JsValue::from_str(&json),
            ) {
                error!("onEvent threw: {e:?}");
            }
        }
    });
    #[cfg(not(target_arch = "wasm32"))]
    info!("event {json}");
}

/// What the page can ask for.
#[derive(Deserialize, Debug, Clone)]
#[serde(tag = "op", rename_all = "camelCase")]
pub enum Command {
    /// The avatar: body shape and wearables by urn, colours as `#rrggbb`.
    #[serde(rename_all = "camelCase")]
    Outfit {
        body_shape: Option<String>,
        #[serde(default)]
        wearables: Vec<String>,
        skin: Option<String>,
        hair: Option<String>,
        eyes: Option<String>,
        /// categories shown even where another wearable hides them (the items being previewed)
        #[serde(default)]
        force_render: Vec<String>,
        /// no eyes, eyebrows or mouth on the face (`disableFace`)
        #[serde(default)]
        hide_face: bool,
    },
    /// The wearable shown on its own in the item view, or none: a second avatar that wears only
    /// it, its body hidden (but the head, with `keepHead`, for a facial feature).
    #[serde(rename_all = "camelCase")]
    Item {
        urn: Option<String>,
        body_shape: Option<String>,
        skin: Option<String>,
        hair: Option<String>,
        eyes: Option<String>,
        #[serde(default)]
        keep_head: bool,
    },
    /// The scene boundaries (the builder's animation reference): a cylinder 1 m round and 3 m
    /// tall on a grid floor, around the avatar; the avatar's framing grows to show it.
    Boundaries {
        shown: bool,
    },
    /// The authentication platform: the disc the avatar stands on, turning with it.
    Platform {
        shown: bool,
    },
    /// Which subject is shown: the item alone, or the avatar wearing it.
    View {
        item: bool,
    },
    /// The orbit camera; every field optional, unset fields keep their value.
    #[serde(rename_all = "camelCase")]
    Camera {
        /// yaw around the avatar, radians; 0 faces the avatar
        alpha: Option<f32>,
        /// pitch, radians; positive looks down
        beta: Option<f32>,
        /// distance to the target, metres
        radius: Option<f32>,
        /// vertical field of view, degrees (perspective)
        fov: Option<f32>,
        orthographic: Option<bool>,
        /// the orbit target, metres from the avatar's feet
        target: Option<[f32; 3]>,
        auto_rotate: Option<bool>,
        /// radians per second
        auto_rotate_speed: Option<f32>,
        /// drag to turn, wheel to zoom, right button to pan
        interactive: Option<bool>,
        /// radians per pixel of drag
        drag_speed: Option<f32>,
        lock_alpha: Option<bool>,
        /// the item view's vertical drag
        lock_beta: Option<bool>,
        lock_radius: Option<bool>,
        panning: Option<bool>,
        wheel_precision: Option<f32>,
        /// the zoom range, as factors of the framed distance (closer is larger)
        zoom_min: Option<f32>,
        zoom_max: Option<f32>,
        /// drop the zoom, pan and spin the controls left
        reset: Option<bool>,
        /// frame the avatar's measured bounds whatever their size, as the renderer's builder view
        /// does (otherwise the default view stands, and only an avatar too big for it is fitted)
        fit_subject: Option<bool>,
        /// stand where the command puts it and never fit the avatar, as the Unity renderer's
        /// profile, authentication and jesus cameras do
        fixed: Option<bool>,
    },
    /// Background colour `#rrggbb`, or none for a transparent canvas.
    Background {
        color: Option<String>,
    },
    /// Play an emote: a urn, or a base emote name such as `wave`.
    #[serde(rename_all = "camelCase")]
    Emote {
        urn: String,
        #[serde(default)]
        r#loop: bool,
    },
    StopEmote,
    /// Hold the emote where it is, or let it run on.
    PauseEmote,
    ResumeEmote,
    /// Move the emote to `seconds` into its clip (it keeps its paused or running state).
    #[serde(rename_all = "camelCase")]
    SeekEmote {
        seconds: f32,
    },
    /// Turn emote sound on or off.
    #[serde(rename_all = "camelCase")]
    Sound {
        enabled: bool,
    },
    /// An item that is not on the catalyst (an unpublished wearable or emote, or a pose the page
    /// ships): `urn` resolves to `definition` (an entity definition json, by url) whose files are
    /// fetched from `files` (name to url: http, `data:` or `blob:`). Send it before the outfit or
    /// emote that names the urn.
    #[serde(rename_all = "camelCase")]
    Collectible {
        /// `wearable` or `emote`
        kind: String,
        urn: String,
        definition: String,
        #[serde(default)]
        files: std::collections::HashMap<String, String>,
    },
    /// The light rig (see `stage::StageLight`); unset fields keep their value.
    #[serde(rename_all = "camelCase")]
    Light {
        illuminance: Option<f32>,
        ambient: Option<f32>,
        yaw: Option<f32>,
        pitch: Option<f32>,
        shadow: Option<f32>,
        shadow_reach: Option<f32>,
        exposure: Option<f32>,
        glow: Option<f32>,
        platform_base: Option<f32>,
        platform_emission: Option<f32>,
    },
    /// A PNG of the view. With a size, rendered off screen at exactly `width` x `height` pixels
    /// (up to 4096 a side), as the live camera frames it vertically, the requested aspect setting
    /// the horizontal extent; without, the canvas as it is. The page's controls are HTML over the
    /// canvas, so neither includes them.
    Screenshot {
        id: String,
        width: Option<u32>,
        height: Option<u32>,
    },
    Metrics {
        id: String,
    },
}

/// What the module reports.
#[derive(Serialize, Debug, Clone)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum Event {
    /// The avatar is dressed and visible.
    Load,
    Error {
        message: String,
    },
    /// Emote playback: `play`, `end` (ran out) or `interrupted` (stopped, replaced, movement),
    /// `pause`, `resume`, `loop` (a looping emote started over) and `playing` (about ten times a
    /// second while it runs). `time` and `length` are seconds into the clip and its length, once
    /// known; `sound` whether the emote has any.
    #[serde(rename_all = "camelCase")]
    Emote {
        event: &'static str,
        urn: Option<String>,
        time: Option<f32>,
        length: Option<f32>,
        sound: Option<bool>,
    },
    #[serde(rename_all = "camelCase")]
    Screenshot {
        id: String,
        /// base64 PNG, no data-url prefix
        png: String,
    },
    /// A request (`id`) that could not be answered, and why.
    #[serde(rename_all = "camelCase")]
    RequestFailed {
        id: String,
        message: String,
    },
    #[serde(rename_all = "camelCase")]
    Metrics {
        id: String,
        triangles: usize,
        materials: usize,
        textures: usize,
        meshes: usize,
        bodies: usize,
        entities: usize,
    },
}

/// The view's state, applied every frame. As in the Unity preview renderer the camera stays put,
/// looking at its subject, and the subject turns in front of it under a light fixed in the world:
/// dragging turns the avatar on yaw only, with inertia (`alpha` is that turn); the item turns on
/// both axes, the tilt easing back upright; auto-rotation resumes a second after the last drag;
/// the right button slides the camera within 2 m; the wheel zooms between 1/1.5 and 2 times the
/// framed distance.
#[derive(Resource, Debug, Clone)]
pub struct OrbitCamera {
    pub alpha: f32,
    /// the pitch the camera rests at (auto-rotation eases back to it)
    pub beta: f32,
    /// the framed distance; the camera stands at `radius / zoom`
    pub radius: f32,
    pub fov_degrees: f32,
    pub orthographic: bool,
    /// the point the view orbits; `pan` slides the camera from it on the screen's axes
    pub target: Vec3,
    pub auto_rotate: bool,
    /// radians per second
    pub auto_rotate_speed: f32,
    /// drag, wheel and pan
    pub interactive: bool,
    /// radians per pixel of horizontal drag
    pub drag_speed: f32,
    pub lock_alpha: bool,
    pub lock_radius: bool,
    pub panning: bool,
    /// higher is slower, as in Babylon (100 is the Unity renderer's unit step)
    pub wheel_precision: f32,
    pub zoom: f32,
    pub zoom_min: f32,
    pub zoom_max: f32,
    pub pan: Vec2,
    /// how much farther than `radius` the avatar's bounds need the camera, and where they move
    /// the target: set by `fit_to_bounds`, 1 and zero for an avatar of ordinary size
    pub fit: f32,
    pub fit_target: Vec3,
    /// the item view: the item alone, at its own centre and distance (`fit_item`), and tilted by a
    /// vertical drag that eases back, as the Unity renderer's item turns on both axes
    pub item_view: bool,
    pub item_target: Vec3,
    pub item_distance: f32,
    /// where the item's glow lies: the bottom of the item
    pub item_floor: f32,
    /// the item has been measured: from then on it turns about `item_target`
    pub item_ready: bool,
    /// fit the avatar's measured bounds whatever their size (the builder view)
    pub fit_subject: bool,
    /// never fit the avatar: the view stands as set (the profile modes)
    pub fixed: bool,
    /// the scene boundaries are shown, and framed with the avatar
    pub boundaries: bool,
    /// the authentication platform is shown under the avatar
    pub platform: bool,
    pub lock_beta: bool,
    tilt: f32,
    tilt_velocity: f32,
    /// a view switch glides from where the camera was: its pivot, distance and pitch then, and
    /// how far through the glide it is (0..1)
    blend: Option<(Vec3, f32, f32, f32)>,
    dragging: bool,
    velocity: f32,
    since_drag: f32,
}

impl Default for OrbitCamera {
    fn default() -> Self {
        Self {
            alpha: 0.0,
            beta: 0.12,
            radius: 4.6,
            fov_degrees: 26.2,
            orthographic: false,
            target: Vec3::new(0.0, 0.95, 0.0),
            auto_rotate: false,
            auto_rotate_speed: 0.2,
            interactive: true,
            drag_speed: 0.5f32.to_radians(),
            lock_alpha: false,
            lock_radius: false,
            panning: true,
            wheel_precision: 100.0,
            zoom: 1.0,
            zoom_min: 1.0 / 1.5,
            zoom_max: 2.0,
            pan: Vec2::ZERO,
            fit: 1.0,
            fit_target: Vec3::ZERO,
            item_view: false,
            item_target: Vec3::new(0.0, 0.95, 0.0),
            item_distance: 4.6,
            item_floor: 0.0,
            item_ready: false,
            fit_subject: false,
            fixed: false,
            boundaries: false,
            platform: false,
            lock_beta: false,
            tilt: 0.0,
            tilt_velocity: 0.0,
            blend: None,
            dragging: false,
            velocity: 0.0,
            since_drag: f32::MAX,
        }
    }
}

impl OrbitCamera {
    /// Where the camera stands from the target, at the current zoom.
    pub fn distance(&self) -> f32 {
        if self.item_view {
            self.item_distance / self.zoom
        } else {
            self.radius * self.fit / self.zoom
        }
    }

    /// The pitch the camera looks down at its subject from.
    pub fn pitch(&self) -> f32 {
        self.beta
    }

    /// The point the view orbits.
    pub fn pivot(&self) -> Vec3 {
        if self.item_view {
            self.item_target
        } else {
            self.target + self.fit_target
        }
    }

    /// The view as the camera command last set it, and the controls' drift dropped.
    pub fn reset_view(&mut self) {
        self.zoom = 1.0;
        self.pan = Vec2::ZERO;
        self.velocity = 0.0;
        self.tilt = 0.0;
        self.tilt_velocity = 0.0;
    }
}

/// The preview's camera.
#[derive(Component)]
pub struct PreviewCamera;

/// Where the item view's subject stands, from the avatar: the Unity renderer's wearable offset.
const ITEM_OFFSET: Vec3 = Vec3::new(-5.0, 0.0, 0.0);

/// The item view's subject: a second avatar wearing only the previewed wearable, its body hidden
/// (but the head when `keep_head`), casting no shadow, held in its first pose.
#[derive(Component)]
pub struct PreviewItem {
    keep_head: bool,
}

/// `load` waits until every subject the page asked for is framed: the avatar after an outfit
/// (`fit_to_bounds`), and the item when there is one (`fit_item`).
#[derive(Resource, Default)]
pub struct LoadGate {
    armed: bool,
    avatar_framed: bool,
    avatar_failed: bool,
    item_wanted: bool,
    item_framed: bool,
}

/// The avatar's face without its features (`disableFace`).
#[derive(Resource, Default)]
pub struct HideFace(pub bool);

/// A facial feature mesh `hide_face` hid, so it can show it again.
#[derive(Component)]
struct FaceHidden;

/// Hides the avatar's eyes, eyebrows and mouth (its body's mask meshes) while `HideFace` asks,
/// and shows again only what it hid.
#[allow(clippy::type_complexity)]
fn hide_face(
    mut commands: Commands,
    face: Res<HideFace>,
    avatars: Query<Entity, With<PreviewAvatar>>,
    children: Query<&Children>,
    mut masks: Query<(&mut Visibility, &Name, Has<FaceHidden>), With<avatar::AvatarBodyMesh>>,
) {
    for avatar in avatars.iter() {
        for entity in children.iter_descendants(avatar) {
            let Ok((mut visibility, name, hidden)) = masks.get_mut(entity) else {
                continue;
            };
            if !name.as_str().to_lowercase().contains("mask_") {
                continue;
            }
            if face.0 && !hidden {
                *visibility = Visibility::Hidden;
                commands.entity(entity).insert(FaceHidden);
            } else if !face.0 && hidden {
                *visibility = Visibility::Inherited;
                commands.entity(entity).remove::<FaceHidden>();
            }
        }
    }
}

/// An outfit received before the avatar entity exists; `spawn_avatar_on_first_outfit` takes it.
#[derive(Resource, Default)]
pub struct PendingOutfit(pub Option<PbAvatarShape>);

pub struct HostPlugin;

impl Plugin for HostPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<OrbitCamera>();
        app.init_resource::<PendingOutfit>();
        app.init_resource::<LoadGate>();
        app.init_resource::<HideFace>();
        app.add_systems(
            Update,
            (
                apply_commands,
                orbit_input,
                fit_to_bounds,
                fit_item,
                prepare_item,
                hide_face,
                place_camera,
                show_subjects,
                report_load,
                send_load,
                report_emotes,
                run_captures,
            )
                .chain(),
        );
    }
}

fn parse_hex(color: &str) -> Option<Color3> {
    let hex = color.trim().trim_start_matches('#');
    if hex.len() != 6 {
        return None;
    }
    let channel = |i: usize| u8::from_str_radix(&hex[i..i + 2], 16).ok();
    Some(Color3 {
        r: channel(0)? as f32 / 255.0,
        g: channel(2)? as f32 / 255.0,
        b: channel(4)? as f32 / 255.0,
    })
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn apply_commands(
    mut commands: Commands,
    mut avatars: Query<(Entity, &mut AvatarSelection, &mut AvatarShape), With<PreviewAvatar>>,
    mut pending: ResMut<PendingOutfit>,
    mut inline: InlineCollectibles,
    mut light: ResMut<crate::stage::StageLight>,
    mut controls: Query<&mut avatar::animate::EmoteControl, With<PreviewAvatar>>,
    mut sound: ResMut<crate::audio::PreviewSound>,
    mut orbit: ResMut<OrbitCamera>,
    mut cameras: Query<(Entity, &mut Camera, &mut Projection), With<PreviewCamera>>,
    windows: Query<Entity, With<PrimaryWindow>>,
    time: Res<Time>,
    metrics: (
        Query<(&Mesh3d, &MeshMaterial3d<SceneMaterial>)>,
        Res<Assets<Mesh>>,
        Res<Assets<SceneMaterial>>,
    ),
    (items, mut gate, mut face): (
        Query<Entity, With<PreviewItem>>,
        ResMut<LoadGate>,
        ResMut<HideFace>,
    ),
    (flags, mut images, views): (
        Res<crate::StageFlags>,
        ResMut<Assets<Image>>,
        Query<(&Transform, &bevy::render::camera::Exposure), With<PreviewCamera>>,
    ),
) {
    let queued: Vec<Command> = COMMANDS.with(|queue| queue.borrow_mut().drain(..).collect());
    let mut deferred = Vec::new();
    for command in queued {
        debug!("{command:?}");
        // an emote before the avatar entity exists (the outfit arrived this frame) waits a frame
        if matches!(command, Command::Emote { .. } | Command::StopEmote) && avatars.is_empty() {
            deferred.push(command);
            continue;
        }
        match command {
            Command::Outfit {
                body_shape,
                wearables,
                skin,
                hair,
                eyes,
                force_render,
                hide_face,
            } => {
                face.0 = hide_face;
                // `load` follows once this outfit (and the item, if one comes) is framed
                gate.armed = true;
                gate.avatar_framed = false;
                gate.avatar_failed = false;
                let next = |current: Option<&PbAvatarShape>| PbAvatarShape {
                    id: "preview".to_owned(),
                    body_shape: body_shape
                        .clone()
                        .or_else(|| current.and_then(|c| c.body_shape.clone())),
                    wearables: wearables.clone(),
                    skin_color: skin.as_deref().and_then(parse_hex),
                    hair_color: hair.as_deref().and_then(parse_hex),
                    eye_color: eyes.as_deref().and_then(parse_hex),
                    force_render: force_render.clone(),
                    ..Default::default()
                };
                if avatars.is_empty() {
                    pending.0 = Some(next(None));
                }
                for (_, mut selection, mut shape) in avatars.iter_mut() {
                    let shape_next = next(Some(&selection.shape.0));
                    shape.0 = shape_next.clone();
                    selection.shape = AvatarShape(shape_next);
                }
            }
            Command::Item {
                urn,
                body_shape,
                skin,
                hair,
                eyes,
                keep_head,
            } => {
                for item in items.iter() {
                    commands.entity(item).despawn();
                }
                gate.item_wanted = urn.is_some();
                gate.item_framed = false;
                orbit.item_ready = false;
                if let Some(urn) = urn {
                    let shape = PbAvatarShape {
                        id: "preview-item".to_owned(),
                        body_shape,
                        wearables: vec![urn],
                        skin_color: skin.as_deref().and_then(parse_hex),
                        hair_color: hair.as_deref().and_then(parse_hex),
                        eye_color: eyes.as_deref().and_then(parse_hex),
                        ..Default::default()
                    };
                    commands.spawn((
                        AvatarShape(shape.clone()),
                        AvatarSelection {
                            scene: None,
                            shape: AvatarShape(shape),
                            automatic_delete: false,
                            disable_dither: true,
                        },
                        common::structs::AvatarDynamicState::default(),
                        // held in the idle's first pose: nothing animates the item on its own
                        avatar::animate::EmoteControl {
                            paused: true,
                            seek: Some(0.0),
                            ..default()
                        },
                        // beside the avatar, where the Unity renderer floats it; the camera
                        // glides between the two
                        Transform::from_translation(ITEM_OFFSET)
                            .with_rotation(Quat::from_rotation_y(std::f32::consts::PI)),
                        Visibility::default(),
                        PreviewItem { keep_head },
                    ));
                }
            }
            Command::Boundaries { shown } => {
                orbit.boundaries = shown;
            }
            Command::Platform { shown } => {
                orbit.platform = shown;
            }
            Command::View { item } => {
                if orbit.item_view != item {
                    // the camera glides from one subject to the other, as the Unity renderer
                    // blends its cameras, and the view starts over, as it resets its rotator
                    let from = (orbit.pivot(), orbit.distance(), orbit.pitch(), 0.0);
                    orbit.item_view = item;
                    orbit.reset_view();
                    orbit.alpha = 0.0;
                    orbit.blend = Some(from);
                }
            }
            Command::Camera {
                alpha,
                beta,
                radius,
                fov,
                orthographic,
                target,
                auto_rotate,
                auto_rotate_speed,
                interactive,
                drag_speed,
                lock_alpha,
                lock_beta,
                lock_radius,
                panning,
                wheel_precision,
                zoom_min,
                zoom_max,
                reset,
                fit_subject,
                fixed,
            } => {
                if reset == Some(true) {
                    orbit.reset_view();
                }
                if let Some(v) = fit_subject {
                    orbit.fit_subject = v;
                }
                if let Some(v) = fixed {
                    orbit.fixed = v;
                }
                if let Some(v) = drag_speed {
                    orbit.drag_speed = v;
                }
                if let Some(v) = lock_alpha {
                    orbit.lock_alpha = v;
                }
                if let Some(v) = lock_beta {
                    orbit.lock_beta = v;
                }
                if let Some(v) = lock_radius {
                    orbit.lock_radius = v;
                }
                if let Some(v) = panning {
                    orbit.panning = v;
                }
                if let Some(v) = wheel_precision {
                    orbit.wheel_precision = v.max(1.0);
                }
                if let Some(v) = zoom_min {
                    orbit.zoom_min = v.max(0.05);
                }
                if let Some(v) = zoom_max {
                    orbit.zoom_max = v.max(orbit.zoom_min);
                }
                orbit.zoom = orbit.zoom.clamp(orbit.zoom_min, orbit.zoom_max);
                if let Some(v) = alpha {
                    orbit.alpha = v;
                }
                if let Some(v) = beta {
                    orbit.beta = v.clamp(-1.4, 1.4);
                }
                if let Some(v) = radius {
                    orbit.radius = v.max(0.2);
                }
                if let Some(v) = fov {
                    orbit.fov_degrees = v.clamp(5.0, 120.0);
                }
                if let Some(v) = orthographic {
                    orbit.orthographic = v;
                }
                if let Some(v) = target {
                    orbit.target = Vec3::from(v);
                }
                if let Some(v) = auto_rotate {
                    orbit.auto_rotate = v;
                }
                if let Some(v) = auto_rotate_speed {
                    orbit.auto_rotate_speed = v;
                }
                if let Some(v) = interactive {
                    orbit.interactive = v;
                }
                for (_, _, mut projection) in cameras.iter_mut() {
                    *projection = projection_for(&orbit);
                }
            }
            Command::Background { color } => {
                let clear = match color.as_deref().and_then(parse_hex) {
                    Some(c) => Color::srgb(c.r, c.g, c.b),
                    None => Color::NONE,
                };
                for (_, mut camera, _) in cameras.iter_mut() {
                    camera.clear_color = ClearColorConfig::Custom(clear);
                }
            }
            Command::Emote { urn, r#loop } => {
                for mut control in controls.iter_mut() {
                    control.paused = false;
                }
                sound.held = false;
                for (entity, _, _) in avatars.iter() {
                    commands.entity(entity).insert(EmoteCommand {
                        urn: urn.clone(),
                        timestamp: (time.elapsed_secs_f64() * 1000.0) as i64,
                        r#loop,
                        mask: EmoteMask::FullBody,
                    });
                }
            }
            Command::Collectible {
                kind,
                urn,
                definition,
                files,
            } => {
                if let Err(message) = inline.register(&kind, &urn, definition, files) {
                    emit(&Event::Error { message });
                }
            }
            Command::Light {
                illuminance,
                ambient,
                yaw,
                pitch,
                shadow,
                shadow_reach,
                exposure,
                glow,
                platform_base,
                platform_emission,
            } => {
                let light = light.as_mut();
                for (slot, value) in [
                    (&mut light.illuminance, illuminance),
                    (&mut light.ambient, ambient),
                    (&mut light.yaw, yaw),
                    (&mut light.pitch, pitch),
                    (&mut light.shadow, shadow),
                    (&mut light.shadow_reach, shadow_reach),
                    (&mut light.exposure, exposure),
                    (&mut light.glow, glow),
                    (&mut light.platform_base, platform_base),
                    (&mut light.platform_emission, platform_emission),
                ] {
                    if let Some(value) = value {
                        *slot = value;
                    }
                }
            }
            Command::PauseEmote | Command::ResumeEmote => {
                let paused = matches!(command, Command::PauseEmote);
                for mut control in controls.iter_mut() {
                    if control.paused != paused {
                        control.paused = paused;
                        emit(&Event::Emote {
                            event: if paused { "pause" } else { "resume" },
                            urn: None,
                            time: Some(control.elapsed),
                            length: Some(control.duration),
                            sound: None,
                        });
                    }
                }
                sound.held = paused;
            }
            Command::SeekEmote { seconds } => {
                for mut control in controls.iter_mut() {
                    control.seek = Some(seconds.max(0.0));
                }
            }
            Command::Sound { enabled } => sound.enabled = enabled,
            Command::StopEmote => {
                for (entity, _, _) in avatars.iter() {
                    commands.entity(entity).insert(EmoteCommand {
                        urn: String::new(),
                        timestamp: (time.elapsed_secs_f64() * 1000.0) as i64,
                        r#loop: false,
                        mask: EmoteMask::FullBody,
                    });
                }
            }
            Command::Screenshot { id, width, height } => {
                let size = match (width, height) {
                    (Some(w), Some(h)) if w > 0 && h > 0 => Some((w, h)),
                    _ => None,
                };
                let Some((width, height)) = size else {
                    // no size: the canvas as it is
                    let Ok(window) = windows.single() else {
                        emit(&Event::RequestFailed {
                            id,
                            message: "screenshot: no window".to_owned(),
                        });
                        continue;
                    };
                    let mut id = Some(id);
                    commands.spawn(Screenshot::window(window)).observe(
                        move |mut trigger: Trigger<ScreenshotCaptured>| {
                            if let Some(id) = id.take() {
                                reply_png(id, std::mem::take(&mut trigger.0));
                            }
                        },
                    );
                    continue;
                };
                const MAX_SIDE: u32 = 4096;
                if width > MAX_SIDE || height > MAX_SIDE {
                    emit(&Event::RequestFailed {
                        id,
                        message: format!(
                            "Invalid screenshot size [{width},{height}], expected width,height up to {MAX_SIDE}"
                        ),
                    });
                    continue;
                }
                // the Unity renderer's answer while a load is under way: the view is not settled
                if gate.armed {
                    emit(&Event::RequestFailed {
                        id,
                        message: "The preview is reloading".to_owned(),
                    });
                    continue;
                }
                let (Ok((_, live, projection)), Ok((transform, exposure))) =
                    (cameras.single(), views.single())
                else {
                    emit(&Event::RequestFailed {
                        id,
                        message: "screenshot: no camera".to_owned(),
                    });
                    continue;
                };
                // a second camera where the live one stands, rendering the view off screen at the
                // requested size, with the same look
                let mut image = Image::new_fill(
                    bevy::render::render_resource::Extent3d {
                        width,
                        height,
                        depth_or_array_layers: 1,
                    },
                    bevy::render::render_resource::TextureDimension::D2,
                    &[0, 0, 0, 0],
                    bevy::render::render_resource::TextureFormat::Rgba8UnormSrgb,
                    bevy::asset::RenderAssetUsages::default(),
                );
                image.texture_descriptor.usage =
                    bevy::render::render_resource::TextureUsages::TEXTURE_BINDING
                        | bevy::render::render_resource::TextureUsages::COPY_DST
                        | bevy::render::render_resource::TextureUsages::COPY_SRC
                        | bevy::render::render_resource::TextureUsages::RENDER_ATTACHMENT;
                let image = images.add(image);
                let mut capture = commands.spawn((
                    Camera3d::default(),
                    Camera {
                        hdr: live.hdr,
                        clear_color: live.clear_color,
                        target: bevy::render::camera::RenderTarget::Image(image.clone().into()),
                        order: 1,
                        ..default()
                    },
                    projection.clone(),
                    *transform,
                ));
                crate::dress_camera(&mut capture, &flags);
                capture.insert((
                    *exposure,
                    Capture {
                        id: Some(id),
                        image,
                        frames: 3,
                    },
                ));
            }
            Command::Metrics { id } => {
                let (meshes, mesh_assets, materials) = &metrics;
                let mut triangles = 0;
                let mut material_ids = std::collections::HashSet::new();
                let mut texture_ids = std::collections::HashSet::new();
                let mut count = 0;
                for (mesh, material) in meshes.iter() {
                    count += 1;
                    if let Some(data) = mesh_assets.get(&mesh.0) {
                        triangles += data
                            .indices()
                            .map(|i| i.len() / 3)
                            .unwrap_or(data.count_vertices() / 3);
                    }
                    material_ids.insert(material.0.id());
                    if let Some(material) = materials.get(&material.0) {
                        for texture in [
                            &material.base.base_color_texture,
                            &material.base.emissive_texture,
                            &material.base.normal_map_texture,
                            &material.base.metallic_roughness_texture,
                        ]
                        .into_iter()
                        .flatten()
                        {
                            texture_ids.insert(texture.id());
                        }
                    }
                }
                emit(&Event::Metrics {
                    id,
                    triangles,
                    materials: material_ids.len(),
                    textures: texture_ids.len(),
                    meshes: count,
                    bodies: 0,
                    entities: count,
                });
            }
        }
    }
    if !deferred.is_empty() {
        COMMANDS.with(|queue| {
            let mut queue = queue.borrow_mut();
            for command in deferred.into_iter().rev() {
                queue.push_front(command);
            }
        });
    }
}

/// Registers host-supplied items with the ipfs layer and points their urns at them.
#[derive(bevy::ecs::system::SystemParam)]
pub struct InlineCollectibles<'w, 's> {
    ipfs: Res<'w, ipfs::IpfsResource>,
    wearables: ResMut<'w, Collectibles<Wearable>>,
    emotes: ResMut<'w, Collectibles<Emote>>,
    next: Local<'s, u32>,
}

impl InlineCollectibles<'_, '_> {
    fn register(
        &mut self,
        kind: &str,
        urn: &str,
        definition: String,
        files: std::collections::HashMap<String, String>,
    ) -> Result<(), String> {
        *self.next += 1;
        let id = format!("inline-{}", *self.next);
        let entry = |ext: &str| format!("collectible.{ext}");
        let definition_files = match kind {
            "wearable" => [
                entry(Wearable::extension()),
                entry(Wearable::data_extension()),
            ],
            "emote" => [entry(Emote::extension()), entry(Emote::data_extension())],
            other => return Err(format!("collectible kind `{other}`: wearable or emote")),
        };
        let mut collection = ipfs::ContentMap::from_content(files);
        for file in definition_files {
            collection = collection.with(file, definition.clone());
        }
        // files map to whole urls: an empty base keeps them as they are
        self.ipfs.add_collection(
            id.clone(),
            collection,
            Some(ipfs::IpfsModifier {
                base_url: Some(String::new()),
            }),
            None,
        );
        match kind {
            "wearable" => {
                let urn =
                    CollectibleUrn::<Wearable>::new(urn).map_err(|e| format!("{urn}: {e:?}"))?;
                self.wearables.insert_inline(urn, id);
            }
            _ => {
                let urn = CollectibleUrn::<Emote>::new(urn).map_err(|e| format!("{urn}: {e:?}"))?;
                self.emotes.insert_inline(urn, id);
            }
        }
        Ok(())
    }
}

pub fn projection_for(orbit: &OrbitCamera) -> Projection {
    if orbit.orthographic {
        // the perspective frustum's height at the target, so a projection switch keeps the framing
        let height = 2.0 * orbit.distance() * (orbit.fov_degrees.to_radians() / 2.0).tan();
        Projection::Orthographic(OrthographicProjection {
            scaling_mode: bevy::render::camera::ScalingMode::FixedVertical {
                viewport_height: height,
            },
            near: 0.05,
            far: 100.0,
            ..OrthographicProjection::default_3d()
        })
    } else {
        Projection::Perspective(PerspectiveProjection {
            fov: orbit.fov_degrees.to_radians(),
            near: 0.05,
            ..default()
        })
    }
}

/// Drag turns the view on yaw with inertia, the right button pans, the wheel zooms: the Unity
/// preview renderer's controls (its `DragRotator`, pan and `ZoomRange`).
fn orbit_input(
    time: Res<Time>,
    mut orbit: ResMut<OrbitCamera>,
    buttons: Res<ButtonInput<MouseButton>>,
    mut motion: EventReader<MouseMotion>,
    mut wheel: EventReader<MouseWheel>,
    windows: Query<&Window, With<PrimaryWindow>>,
) {
    let dt = time.delta_secs();
    let delta: Vec2 = motion.read().map(|m| m.delta).sum();
    let scroll: f32 = wheel
        .read()
        .map(|w| match w.unit {
            MouseScrollUnit::Line => w.y,
            MouseScrollUnit::Pixel => w.y / 100.0,
        })
        .sum();
    if !orbit.interactive {
        orbit.dragging = false;
        orbit.velocity = 0.0;
        return;
    }
    let turning = buttons.pressed(MouseButton::Left) && !orbit.lock_alpha;
    if turning {
        let step = -delta.x * orbit.drag_speed;
        orbit.alpha += step;
        if dt > 0.0 && delta.x != 0.0 {
            orbit.velocity = step / dt;
        } else if delta.x == 0.0 {
            orbit.velocity = 0.0;
        }
        orbit.since_drag = 0.0;
    } else {
        // inertia, decaying to a quarter each second
        orbit.alpha += orbit.velocity * dt;
        orbit.velocity *= 0.25f32.powf(dt);
        if orbit.velocity.abs() < 1e-3 {
            orbit.velocity = 0.0;
        }
        orbit.since_drag += dt;
    }
    orbit.dragging = turning;

    // the item view also tilts on a vertical drag, with inertia, and eases back upright a second
    // after the last drag, as the Unity renderer's item rotator does
    if orbit.item_view && !orbit.lock_beta {
        if buttons.pressed(MouseButton::Left) {
            let step = -delta.y * orbit.drag_speed;
            orbit.tilt = (orbit.tilt + step).clamp(-1.4, 1.4);
            orbit.tilt_velocity = if dt > 0.0 { step / dt } else { 0.0 };
            orbit.since_drag = 0.0;
        } else {
            orbit.tilt = (orbit.tilt + orbit.tilt_velocity * dt).clamp(-1.4, 1.4);
            orbit.tilt_velocity *= 0.25f32.powf(dt);
            if orbit.since_drag > 1.0 {
                orbit.tilt *= 1.0 - (2.0 * dt).min(1.0);
            }
        }
    }

    // right drag pans 1:1 with the cursor at the framed distance, within 2 m of the target
    if buttons.pressed(MouseButton::Right) && orbit.panning && delta != Vec2::ZERO {
        let height = windows
            .single()
            .map(|w| w.height())
            .unwrap_or(768.0)
            .max(1.0);
        let per_pixel =
            2.0 * orbit.distance() * (orbit.fov_degrees.to_radians() / 2.0).tan() / height;
        // on the screen's axes, as the Unity renderer slides its camera: the avatar still turns
        // about its own axis, wherever the view has been moved to
        let pan = orbit.pan + Vec2::new(-delta.x, delta.y) * per_pixel;
        orbit.pan = pan.clamp_length_max(2.0);
        orbit.since_drag = 0.0;
    }

    if scroll != 0.0 && !orbit.lock_radius {
        let factor = 1.03f32.powf(scroll * 3.0 * 100.0 / orbit.wheel_precision);
        orbit.zoom = (orbit.zoom * factor).clamp(orbit.zoom_min, orbit.zoom_max);
    }
}

/// What the default view is set for, measured as `fit_to_bounds` measures: an ordinary avatar
/// through the walk a wearable is shown in, from the floor to the top of its head, reaching this
/// far from its turn axis. An avatar inside it keeps the default framing.
const ORDINARY_TOP: f32 = 2.05;
const ORDINARY_REACH: f32 = 1.0;

/// Frames what the avatar shows, emote props included, as the Unity renderer does for avatars too
/// big for its default view, once and before it is seen: after an outfit change it waits for the
/// outfit and the emote the host asked for, steps the paused clip through its poses, measures the
/// skinned surface at each, rewinds the clip and sets the camera. Only then does it send `load`,
/// which shows the canvas, so the avatar appears framed and the camera never moves on its own.
/// Bounds are against the ordinary avatar's box, at the panel's aspect.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn fit_to_bounds(
    time: Res<Time>,
    mut orbit: ResMut<OrbitCamera>,
    mut avatars: Query<
        (
            Entity,
            Ref<AvatarSelection>,
            Option<Ref<EmoteCommand>>,
            Option<&mut avatar::animate::EmoteControl>,
        ),
        With<PreviewAvatar>,
    >,
    surfaces: Surfaces,
    windows: Query<&Window, With<PrimaryWindow>>,
    (mut reports, mut emote_events): (
        EventReader<AvatarLoadReport>,
        EventReader<EmoteLifecycleEvent>,
    ),
    (mut sound, mut gate): (ResMut<crate::audio::PreviewSound>, ResMut<LoadGate>),
    mut state: Local<FitState>,
) {
    let Ok((root, selection, emote, mut control)) = avatars.single_mut() else {
        return;
    };
    let now = time.elapsed_secs();
    if selection.is_changed() {
        *state = FitState {
            changed_at: now,
            ..default()
        };
    }
    for report in reports.read() {
        if report.owner == root {
            match report.state {
                LoadingState::Finished => state.loaded = Some(now),
                // the error is the host's answer; the fit still frames what did load
                LoadingState::FinishedWithError => {
                    state.loaded = Some(now);
                    state.failed = true;
                }
                _ => {}
            }
        }
    }
    if state.done {
        return;
    }
    // the emote the host asked for is part of the outfit's framing: wait for it to start
    if emote
        .as_ref()
        .is_some_and(|e| e.is_changed() && !e.urn.is_empty())
    {
        state.emote_wanted = true;
        state.emote_started = false;
    }
    for event in emote_events.read() {
        if matches!(event.event, EmoteLifecycle::Started { .. }) {
            state.emote_started = true;
        }
    }
    let Some(control) = control.as_deref_mut() else {
        return;
    };

    if state.samples.is_none() {
        // measure the finished outfit once it plays what the host asked for; the page shows the
        // canvas only on `load`, so the wait is behind the loading spinner (a lost report or an
        // emote that never starts gives up after 10 s)
        let settled = state.loaded.is_some_and(|at| now - at > 0.3)
            && (!state.emote_wanted || state.emote_started)
            && control.duration > 0.0;
        if !settled && now - state.changed_at < 10.0 {
            return;
        }
        // hold the clip and its sound while the pose is stepped through it
        state.restore = Some((control.paused, sound.held));
        sound.held = true;
        state.samples = Some(0);
        state.wait = 2;
        control.paused = true;
        control.seek = Some(0.0);
        return;
    }

    // a seek reaches the skeleton a frame later, and the world transforms the frame after
    if state.wait > 0 {
        state.wait -= 1;
        return;
    }
    // the avatar turns about the vertical axis through its root, as the Unity renderer's avatar
    // does: how far anything reaches from that axis matters whichever way it faces the camera
    let axis = surfaces.origin(root);
    let fields = &mut *state;
    let (seen, reach) = (&mut fields.seen, &mut fields.reach);
    let mut grow = |point: Vec3| {
        let (min, max) = seen.get_or_insert((point, point));
        *min = min.min(point);
        *max = max.max(point);
        *reach = reach.max((point - axis).xz().length());
    };
    surfaces.measure(root, true, &mut grow);

    // the whole clip, a pose at a time, short of its end (a one-shot at its end has finished)
    const SAMPLES: u32 = 20;
    let sample = state.samples.unwrap_or(0) + 1;
    if sample < SAMPLES && control.duration > 0.0 {
        state.samples = Some(sample);
        state.wait = 2;
        control.seek = Some(control.duration * sample as f32 / SAMPLES as f32);
        return;
    }

    // measured: the clip starts over as it would have, and the camera is set once
    state.done = true;
    let (paused, held) = state.restore.take().unwrap_or((false, false));
    control.paused = paused;
    control.seek = Some(0.0);
    sound.held = held;
    gate.avatar_framed = true;
    gate.avatar_failed = state.failed;
    let Some((min, max)) = state.seen else {
        return;
    };
    // the avatar's footprint as the renderer measures it (the circumradius about its middle)
    let mut radius = 0.5 * (max - min).xz().length();
    let mut reach = state.reach;
    // the scene boundaries are framed with the avatar, as the renderer frames its cylinder: the
    // height from the floor to its top, and across, the larger of the two
    let (min, max) = if orbit.boundaries {
        radius = radius.max(1.0);
        reach = reach.max(1.0);
        (
            min.min(Vec3::new(-1.0, 0.0, -1.0)),
            max.max(Vec3::new(1.0, 3.0, 1.0)),
        )
    } else {
        (min, max)
    };
    let aspect = windows
        .single()
        .map(|w| w.width() / w.height().max(1.0))
        .unwrap_or(1.0);
    let tan_v = (orbit.fov_degrees.to_radians() / 2.0).tan();
    let tan_h = tan_v * aspect;
    // the distance that keeps a body in view looking at a height: up and down from it, nearer by
    // what reaches towards the camera; across, anything at its reach turned side-on
    let needs = |bottom: f32, top: f32, reach: f32, at: f32| {
        let tall = (top - at).max(at - bottom) / tan_v + reach;
        let wide = reach * (1.0 + 1.0 / (tan_h * tan_h)).sqrt();
        tall.max(wide)
    };
    let ordinary = needs(0.0, ORDINARY_TOP, ORDINARY_REACH, orbit.target.y);
    if orbit.fixed {
        orbit.fit = 1.0;
        orbit.fit_target = Vec3::ZERO;
    } else if orbit.fit_subject {
        // the builder view, as the renderer fits it: centred on the bounds' middle, its height and
        // its widest turn across (the circumradius of its footprint) in view, with a margin. The
        // renderer's is 5% over its renderers' bounds, which run looser than the skinned surface
        // measured here: 13% matches its builder captures
        const MARGIN: f32 = 1.13;
        let middle = (min.y + max.y) / 2.0;
        let distance = ((max.y - min.y) / 2.0 / tan_v).max(radius / tan_h) * MARGIN;
        orbit.fit = distance / orbit.radius.max(0.01);
        orbit.fit_target = Vec3::Y * (middle - orbit.target.y);
    } else if needs(min.y, max.y, reach, orbit.target.y) > ordinary {
        // too big for the default view: look at its middle, from as far as that needs, with a
        // little room around it as the Unity renderer's fit keeps
        const MARGIN: f32 = 1.1;
        let middle = (min.y + max.y) / 2.0;
        orbit.fit = (needs(min.y, max.y, reach, middle) * MARGIN / ordinary).max(1.0);
        orbit.fit_target = Vec3::Y * (middle - orbit.target.y);
    } else {
        orbit.fit = 1.0;
        orbit.fit_target = Vec3::ZERO;
    }
}

/// The meshes under an avatar, measured where they are drawn this frame.
#[allow(clippy::type_complexity)]
#[derive(bevy::ecs::system::SystemParam)]
pub struct Surfaces<'w, 's> {
    meshes: Query<
        'w,
        's,
        (
            &'static GlobalTransform,
            &'static bevy::render::primitives::Aabb,
            &'static InheritedVisibility,
            Option<&'static bevy::render::mesh::skinning::SkinnedMesh>,
            &'static Mesh3d,
            Has<avatar::AvatarBodyMesh>,
        ),
    >,
    joints: Query<'w, 's, &'static GlobalTransform>,
    children: Query<'w, 's, &'static Children>,
    mesh_assets: Res<'w, Assets<Mesh>>,
    bindposes: Res<'w, Assets<bevy::render::mesh::skinning::SkinnedMeshInverseBindposes>>,
}

impl Surfaces<'_, '_> {
    /// Where an avatar's root is.
    fn origin(&self, root: Entity) -> Vec3 {
        self.joints
            .get(root)
            .map(|t| t.translation())
            .unwrap_or(Vec3::ZERO)
    }

    /// Grows a box by every visible mesh under `root` (its base body too, with `with_body`):
    /// skinned meshes by their vertices in this frame's pose, the rest by their bounds' corners.
    fn measure(&self, root: Entity, with_body: bool, grow: &mut impl FnMut(Vec3)) {
        for entity in self.children.iter_descendants(root) {
            let Ok((transform, aabb, visible, skin, mesh, body)) = self.meshes.get(entity) else {
                continue;
            };
            if !visible.get() || (body && !with_body) {
                continue;
            }
            if let Some(skin) = skin {
                // a skinned mesh's box is its rest pose in the space it was bound in: skin the
                // vertices on the CPU instead, as the GPU will draw them in this pose
                let skinned = self
                    .mesh_assets
                    .get(&mesh.0)
                    .zip(self.bindposes.get(&skin.inverse_bindposes))
                    .map(|(data, inverse)| {
                        let matrices: Vec<Mat4> = skin
                            .joints
                            .iter()
                            .zip(inverse.iter())
                            .map(|(joint, inverse)| {
                                self.joints
                                    .get(*joint)
                                    .map(|joint| joint.compute_matrix() * *inverse)
                                    .unwrap_or(Mat4::ZERO)
                            })
                            .collect();
                        skin_points(data, &matrices, grow)
                    })
                    .unwrap_or(false);
                if !skinned {
                    // no vertex data to skin: the skeleton, with a margin for the surface around it
                    const MARGIN: f32 = 0.35;
                    for joint in &skin.joints {
                        if let Ok(joint) = self.joints.get(*joint) {
                            let at = joint.translation();
                            grow(at - Vec3::splat(MARGIN));
                            grow(at + Vec3::splat(MARGIN));
                        }
                    }
                }
                continue;
            }
            let (centre, half) = (Vec3::from(aabb.center), Vec3::from(aabb.half_extents));
            for corner in 0..8 {
                let sign = Vec3::new(
                    if corner & 1 == 0 { -1.0 } else { 1.0 },
                    if corner & 2 == 0 { -1.0 } else { 1.0 },
                    if corner & 4 == 0 { -1.0 } else { 1.0 },
                );
                // in the world, where the camera's target is
                grow(transform.transform_point(centre + half * sign));
            }
        }
    }
}

/// Frames the item view once, before it is seen, as the Unity renderer centres and fits a
/// wearable shown on its own: when the item has loaded, its surface (in the idle's first pose) is
/// measured, the view centres on it, and the camera stands where its height, and its widest turn
/// across, fit the panel.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
fn fit_item(
    time: Res<Time>,
    mut orbit: ResMut<OrbitCamera>,
    items: Query<(Entity, Ref<AvatarSelection>), With<PreviewItem>>,
    surfaces: Surfaces,
    windows: Query<&Window, With<PrimaryWindow>>,
    mut reports: EventReader<AvatarLoadReport>,
    mut gate: ResMut<LoadGate>,
    mut state: Local<ItemFitState>,
) {
    let Ok((item, selection)) = items.single() else {
        return;
    };
    let now = time.elapsed_secs();
    if selection.is_added() || state.item != Some(item) {
        *state = ItemFitState {
            item: Some(item),
            changed_at: now,
            ..default()
        };
    }
    for report in reports.read() {
        if report.owner == item
            && matches!(
                report.state,
                LoadingState::Finished | LoadingState::FinishedWithError
            )
        {
            state.loaded = Some(now);
        }
    }
    // the body is hidden and the pose held a moment after loading; a lost report gives up later
    let settled = state.loaded.is_some_and(|at| now - at > 0.3);
    if state.done || (!settled && now - state.changed_at < 10.0) {
        return;
    }
    state.done = true;
    gate.item_framed = true;
    let mut seen: Option<(Vec3, Vec3)> = None;
    surfaces.measure(item, false, &mut |point: Vec3| {
        let (min, max) = seen.get_or_insert((point, point));
        *min = min.min(point);
        *max = max.max(point);
    });
    let Some((min, max)) = seen else {
        return;
    };
    let aspect = windows
        .single()
        .map(|w| w.width() / w.height().max(1.0))
        .unwrap_or(1.0);
    let tan_v = (orbit.fov_degrees.to_radians() / 2.0).tan();
    let tan_h = tan_v * aspect;
    // its height, and across it the widest it can turn to (the circumradius of its footprint),
    // with the Unity renderer's 5% margin
    const MARGIN: f32 = 1.05;
    let size = max - min;
    let radius = 0.5 * size.xz().length();
    orbit.item_target = (min + max) / 2.0;
    orbit.item_floor = min.y;
    orbit.item_ready = true;
    orbit.item_distance = ((size.y / 2.0) / tan_v).max(radius / tan_h).max(0.1) * MARGIN;
}

/// What `fit_item` knows about the item it frames.
#[derive(Default)]
struct ItemFitState {
    item: Option<Entity>,
    changed_at: f32,
    loaded: Option<f32>,
    done: bool,
}

/// Hides the item's base body (but the head for a facial feature) and keeps it from casting a
/// shadow: the item floats on its own, with nothing under it to cast onto.
#[allow(clippy::type_complexity)]
fn prepare_item(
    mut commands: Commands,
    items: Query<(Entity, &PreviewItem)>,
    children: Query<&Children>,
    mut bodies: Query<(&mut Visibility, Option<&Name>), With<avatar::AvatarBodyMesh>>,
    casters: Query<(), (With<Mesh3d>, Without<bevy::pbr::NotShadowCaster>)>,
) {
    for (item, preview) in items.iter() {
        for entity in children.iter_descendants(item) {
            if let Ok((mut visibility, name)) = bodies.get_mut(entity) {
                let head = name.is_some_and(|n| n.as_str().to_lowercase().contains("head"));
                if !(preview.keep_head && head) {
                    visibility.set_if_neq(Visibility::Hidden);
                }
            }
            if casters.contains(entity) {
                commands
                    .entity(entity)
                    .try_insert(bevy::pbr::NotShadowCaster);
            }
        }
    }
}

/// Shows the subject of the view and hides the other, 5 m away: out of frame in an ordinary view,
/// but a big avatar's framing zoomed out can reach it, and the Unity renderer never shows it. Both
/// show during a switch's glide, and while a subject is measured (behind the loading spinner, as
/// measuring needs it visible).
#[allow(clippy::type_complexity)]
fn show_subjects(
    orbit: Res<OrbitCamera>,
    gate: Res<LoadGate>,
    mut avatars: Query<&mut Visibility, (With<PreviewAvatar>, Without<PreviewItem>)>,
    mut items: Query<&mut Visibility, (With<PreviewItem>, Without<PreviewAvatar>)>,
) {
    let gliding = orbit.blend.is_some();
    let measuring_avatar = gate.armed && !gate.avatar_framed;
    let measuring_item = gate.item_wanted && !gate.item_framed;
    let shown = |yes: bool| {
        if yes {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        }
    };
    for mut visibility in avatars.iter_mut() {
        visibility.set_if_neq(shown(!orbit.item_view || gliding || measuring_avatar));
    }
    for mut visibility in items.iter_mut() {
        visibility.set_if_neq(shown(orbit.item_view || gliding || measuring_item));
    }
}

/// Sends `load` once every subject the page asked for is framed.
fn send_load(mut gate: ResMut<LoadGate>) {
    if !gate.armed || !gate.avatar_framed || (gate.item_wanted && !gate.item_framed) {
        return;
    }
    gate.armed = false;
    if !gate.avatar_failed {
        emit(&Event::Load);
    }
}

/// Grows a box by where a skinned mesh's vertices are in this frame's pose: each vertex blended
/// by its joints' weights, as the GPU skins it. False when the mesh carries no skinning data.
fn skin_points(mesh: &Mesh, matrices: &[Mat4], grow: &mut impl FnMut(Vec3)) -> bool {
    use bevy::render::mesh::VertexAttributeValues as Values;
    let (
        Some(Values::Float32x3(positions)),
        Some(Values::Uint16x4(indices)),
        Some(Values::Float32x4(weights)),
    ) = (
        mesh.attribute(Mesh::ATTRIBUTE_POSITION),
        mesh.attribute(Mesh::ATTRIBUTE_JOINT_INDEX),
        mesh.attribute(Mesh::ATTRIBUTE_JOINT_WEIGHT),
    )
    else {
        return false;
    };
    for ((position, index), weight) in positions.iter().zip(indices).zip(weights) {
        let mut blended = Mat4::ZERO;
        for k in 0..4 {
            if weight[k] > 0.0 {
                if let Some(matrix) = matrices.get(index[k] as usize) {
                    blended += *matrix * weight[k];
                }
            }
        }
        grow(blended.transform_point3(Vec3::from(*position)));
    }
    true
}

/// What `fit_to_bounds` has measured since the last outfit change: it waits for the outfit (and
/// the emote the host asked for), steps the paused clip through `SAMPLES` poses, then frames once.
#[derive(Default)]
struct FitState {
    changed_at: f32,
    loaded: Option<f32>,
    failed: bool,
    emote_wanted: bool,
    emote_started: bool,
    /// the pose being measured; `None` before measuring starts
    samples: Option<u32>,
    /// frames left before the pose a seek asked for is in the world transforms
    wait: u32,
    /// the host's pause and sound hold, given back when measuring ends
    restore: Option<(bool, bool)>,
    seen: Option<(Vec3, Vec3)>,
    /// the farthest anything reaches from the turn axis
    reach: f32,
    done: bool,
}

#[allow(clippy::type_complexity)]
fn place_camera(
    time: Res<Time>,
    mut orbit: ResMut<OrbitCamera>,
    gate: Res<LoadGate>,
    mut cameras: Query<
        (&mut Transform, &mut Projection),
        (
            With<PreviewCamera>,
            Without<PreviewAvatar>,
            Without<PreviewItem>,
        ),
    >,
    mut avatars: Query<&mut Transform, (With<PreviewAvatar>, Without<PreviewItem>)>,
    mut items: Query<&mut Transform, (With<PreviewItem>, Without<PreviewAvatar>)>,
) {
    let dt = time.delta_secs();
    // auto-rotation resumes a second after the last drag, once the spin has settled, and waits
    // for `load`: the subjects are measured standing as they were built
    if orbit.auto_rotate
        && !orbit.lock_alpha
        && !orbit.dragging
        && orbit.since_drag > 1.0
        && orbit.velocity == 0.0
        && !gate.armed
    {
        orbit.alpha += orbit.auto_rotate_speed * dt;
    }

    // the subject in view turns in front of the camera: the avatar about its own vertical axis,
    // the item about its centre on both axes (the one out of view keeps its last turn, and starts
    // over when it comes back, as the Unity renderer resets its rotator)
    let facing = Quat::from_rotation_y(std::f32::consts::PI);
    if orbit.item_view {
        if orbit.item_ready {
            let turn = Quat::from_rotation_x(orbit.tilt) * Quat::from_rotation_y(-orbit.alpha);
            let centre = orbit.item_target;
            for mut transform in items.iter_mut() {
                transform.translation = centre + turn * (ITEM_OFFSET - centre);
                transform.rotation = turn * facing;
            }
        }
    } else {
        for mut transform in avatars.iter_mut() {
            transform.rotation = Quat::from_rotation_y(-orbit.alpha) * facing;
        }
    }

    // the camera looks at its subject from straight in front, at its pitch and distance; the pan
    // slides it across the screen
    let (mut pivot, mut distance, mut pitch) = (orbit.pivot(), orbit.distance(), orbit.pitch());
    // a view switch eases from the old view to the new in 0.3 s, the Unity renderer's blend
    if let Some((from_pivot, from_distance, from_pitch, t)) = orbit.blend {
        let t = (t + dt / 0.3).min(1.0);
        let s = t * t * (3.0 - 2.0 * t);
        pivot = from_pivot.lerp(pivot, s);
        distance = from_distance + (distance - from_distance) * s;
        pitch = from_pitch + (pitch - from_pitch) * s;
        orbit.blend = (t < 1.0).then_some((from_pivot, from_distance, from_pitch, t));
    }
    let rotation = Quat::from_rotation_x(-pitch);
    let placed = Transform::from_translation(pivot + rotation * Vec3::new(0.0, 0.0, distance))
        .looking_at(pivot, Vec3::Y);
    let slide = placed.rotation * orbit.pan.extend(0.0);
    for (mut transform, mut projection) in cameras.iter_mut() {
        *transform = placed.with_translation(placed.translation + slide);
        if orbit.is_changed() {
            *projection = projection_for(&orbit);
        }
    }
}

fn report_load(
    mut reports: EventReader<AvatarLoadReport>,
    avatars: Query<Entity, With<PreviewAvatar>>,
) {
    for report in reports.read() {
        if !avatars.contains(report.owner) {
            continue;
        }
        // `load` waits for the framing: `fit_to_bounds` sends it
        if report.state == LoadingState::FinishedWithError {
            emit(&Event::Error {
                message: format!("could not load {}", report.body_shape),
            });
        }
    }
}

/// Lifecycle events from the Explorer's emote code, and the playback progress the host's emote
/// controls follow: `playing` about ten times a second, `loop` when a looping clip starts over.
/// Whether the emote has sound is known once its definition has loaded, which can be after it
/// starts, so it rides on every event until then.
#[allow(clippy::type_complexity)]
fn report_emotes(
    mut events: EventReader<EmoteLifecycleEvent>,
    controls: Query<(&avatar::animate::EmoteControl, &AvatarShape), With<PreviewAvatar>>,
    mut emotes: collectibles::CollectibleManager<Emote>,
    time: Res<Time>,
    mut progress: Local<EmoteProgress>,
) {
    let control = controls.iter().next();
    let mut has_sound = |urn: &str| -> Option<bool> {
        let (_, shape) = control?;
        let body = shape.0.body_shape.clone().unwrap_or_default();
        let urn = CollectibleUrn::<Emote>::new(urn).ok()?;
        let emote = emotes.get_representation(&urn, &body).ok()?;
        Some(emote.sound.iter().any(|(_, clips)| !clips.is_empty()))
    };
    for event in events.read() {
        let (name, urn) = match &event.event {
            EmoteLifecycle::Started { urn, .. } => ("play", Some(urn.clone())),
            EmoteLifecycle::Finished => ("end", None),
            EmoteLifecycle::Interrupted => ("interrupted", None),
        };
        progress.sound = urn.as_deref().and_then(&mut has_sound);
        progress.urn = urn.clone();
        progress.last_elapsed = 0.0;
        emit(&Event::Emote {
            event: name,
            urn,
            time: None,
            length: None,
            sound: progress.sound,
        });
    }
    let Some(urn) = progress.urn.clone() else {
        return;
    };
    if progress.sound.is_none() {
        progress.sound = has_sound(&urn);
    }
    let Some((control, _)) = control else {
        return;
    };
    if control.duration <= 0.0 || control.paused {
        return;
    }
    // a one-shot's time runs on past its end until the end is reported: only a repeating clip loops
    let looped = control.repeat
        && (control.elapsed / control.duration).floor()
            > (progress.last_elapsed / control.duration).floor();
    progress.last_elapsed = control.elapsed;
    let time_in_clip = if control.repeat {
        control.elapsed % control.duration
    } else {
        control.elapsed.min(control.duration)
    };
    if looped {
        emit(&Event::Emote {
            event: "loop",
            urn: None,
            time: Some(time_in_clip),
            length: Some(control.duration),
            sound: progress.sound,
        });
    }
    let now = time.elapsed_secs();
    if now - progress.last_report >= 0.1 {
        progress.last_report = now;
        emit(&Event::Emote {
            event: "playing",
            urn: None,
            time: Some(time_in_clip),
            length: Some(control.duration),
            sound: progress.sound,
        });
    }
}

/// What `report_emotes` remembers about the emote playing.
#[derive(Default)]
struct EmoteProgress {
    urn: Option<String>,
    sound: Option<bool>,
    last_report: f32,
    last_elapsed: f32,
}

/// A sized screenshot's camera, rendering off screen into `image`: read back once it has had
/// `frames` frames to render (a new target's pipelines compile in that time), then removed.
#[derive(Component)]
pub struct Capture {
    id: Option<String>,
    image: Handle<Image>,
    frames: u32,
}

fn run_captures(mut commands: Commands, mut captures: Query<(Entity, &mut Capture)>) {
    for (camera, mut capture) in captures.iter_mut() {
        if capture.frames > 0 {
            capture.frames -= 1;
            continue;
        }
        let Some(id) = capture.id.take() else {
            continue;
        };
        let mut id = Some(id);
        commands
            .spawn(Screenshot::image(capture.image.clone()))
            .observe(
                move |mut trigger: Trigger<ScreenshotCaptured>, mut commands: Commands| {
                    if let Some(id) = id.take() {
                        reply_png(id, std::mem::take(&mut trigger.0));
                        commands.entity(camera).despawn();
                    }
                },
            );
    }
}

/// Answers a screenshot request with the image as base64 PNG, or its failure.
fn reply_png(id: String, image: Image) {
    match encode_png(image) {
        Ok(png) => emit(&Event::Screenshot { id, png }),
        Err(message) => emit(&Event::RequestFailed { id, message }),
    }
}

/// The captured frame as base64 PNG.
fn encode_png(image: Image) -> Result<String, String> {
    let dynamic = image
        .try_into_dynamic()
        .map_err(|e| format!("screenshot: could not read frame: {e:?}"))?;
    let mut png = Vec::new();
    dynamic
        .write_to(&mut Cursor::new(&mut png), image::ImageFormat::Png)
        .map_err(|e| format!("screenshot: PNG encode failed: {e}"))?;
    Ok(BASE64_STANDARD.encode(&png))
}
