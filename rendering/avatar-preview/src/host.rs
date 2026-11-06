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
        /// drag to orbit, wheel to zoom
        interactive: Option<bool>,
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
    },
    /// Capture the frame as PNG, `width` x `height` (the frame is scaled and cropped to fit).
    Screenshot {
        id: String,
        width: u32,
        height: u32,
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

/// The orbit camera's state, applied to the camera transform every frame.
#[derive(Resource, Debug, Clone)]
pub struct OrbitCamera {
    pub alpha: f32,
    pub beta: f32,
    pub radius: f32,
    pub fov_degrees: f32,
    pub orthographic: bool,
    pub target: Vec3,
    pub auto_rotate: bool,
    pub auto_rotate_speed: f32,
    pub interactive: bool,
    dragging: bool,
}

impl Default for OrbitCamera {
    fn default() -> Self {
        Self {
            alpha: 0.0,
            beta: 0.05,
            radius: 6.0,
            fov_degrees: 26.2,
            orthographic: false,
            target: Vec3::new(0.0, 1.0, 0.0),
            auto_rotate: false,
            auto_rotate_speed: 0.25,
            interactive: true,
            dragging: false,
        }
    }
}

/// The preview's camera.
#[derive(Component)]
pub struct PreviewCamera;

/// An outfit received before the avatar entity exists; `spawn_avatar_on_first_outfit` takes it.
#[derive(Resource, Default)]
pub struct PendingOutfit(pub Option<PbAvatarShape>);

pub struct HostPlugin;

impl Plugin for HostPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<OrbitCamera>();
        app.init_resource::<PendingOutfit>();
        app.add_systems(
            Update,
            (
                apply_commands,
                orbit_input,
                place_camera,
                report_load,
                report_emotes,
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
            } => {
                let next = |current: Option<&PbAvatarShape>| PbAvatarShape {
                    id: "preview".to_owned(),
                    body_shape: body_shape
                        .clone()
                        .or_else(|| current.and_then(|c| c.body_shape.clone())),
                    wearables: wearables.clone(),
                    skin_color: skin.as_deref().and_then(parse_hex),
                    hair_color: hair.as_deref().and_then(parse_hex),
                    eye_color: eyes.as_deref().and_then(parse_hex),
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
            } => {
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
                let Ok(window) = windows.single() else {
                    emit(&Event::Error {
                        message: "screenshot: no window".to_owned(),
                    });
                    continue;
                };
                let mut id = Some(id);
                commands.spawn(Screenshot::window(window)).observe(
                    move |mut trigger: Trigger<ScreenshotCaptured>| {
                        let Some(id) = id.take() else {
                            return;
                        };
                        match encode_png(std::mem::take(&mut trigger.0), width, height) {
                            Ok(png) => emit(&Event::Screenshot { id, png }),
                            Err(message) => emit(&Event::Error { message }),
                        }
                    },
                );
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
        let height = 2.0 * orbit.radius * (orbit.fov_degrees.to_radians() / 2.0).tan();
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

/// Drag to orbit, wheel to zoom, when interactive.
fn orbit_input(
    mut orbit: ResMut<OrbitCamera>,
    buttons: Res<ButtonInput<MouseButton>>,
    mut motion: EventReader<MouseMotion>,
    mut wheel: EventReader<MouseWheel>,
) {
    if !orbit.interactive {
        motion.clear();
        wheel.clear();
        return;
    }
    orbit.dragging = buttons.pressed(MouseButton::Left);
    let delta: Vec2 = motion.read().map(|m| m.delta).sum();
    if orbit.dragging && delta != Vec2::ZERO {
        orbit.alpha -= delta.x * 0.01;
        orbit.beta = (orbit.beta + delta.y * 0.01).clamp(-1.4, 1.4);
    }
    let scroll: f32 = wheel
        .read()
        .map(|w| match w.unit {
            MouseScrollUnit::Line => w.y * 40.0,
            MouseScrollUnit::Pixel => w.y,
        })
        .sum();
    if scroll != 0.0 {
        orbit.radius = (orbit.radius * (-scroll * 0.002).exp()).clamp(0.5, 30.0);
    }
}

fn place_camera(
    time: Res<Time>,
    mut orbit: ResMut<OrbitCamera>,
    mut cameras: Query<(&mut Transform, &mut Projection), With<PreviewCamera>>,
) {
    if orbit.auto_rotate && !orbit.dragging {
        orbit.alpha += orbit.auto_rotate_speed * time.delta_secs();
    }
    let rotation = Quat::from_rotation_y(orbit.alpha) * Quat::from_rotation_x(-orbit.beta);
    let position = orbit.target + rotation * Vec3::new(0.0, 0.0, orbit.radius);
    for (mut transform, mut projection) in cameras.iter_mut() {
        *transform = Transform::from_translation(position).looking_at(orbit.target, Vec3::Y);
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
        match report.state {
            LoadingState::Finished => emit(&Event::Load),
            LoadingState::FinishedWithError => emit(&Event::Error {
                message: format!("could not load {}", report.body_shape),
            }),
            _ => {}
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
    let looped = (control.elapsed / control.duration).floor()
        > (progress.last_elapsed / control.duration).floor();
    progress.last_elapsed = control.elapsed;
    let time_in_clip = control.elapsed % control.duration;
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

/// The captured frame as base64 PNG, scaled and cropped to `width` x `height`.
fn encode_png(image: Image, width: u32, height: u32) -> Result<String, String> {
    let mut dynamic = image
        .try_into_dynamic()
        .map_err(|e| format!("screenshot: could not read frame: {e:?}"))?;
    if width > 0 && height > 0 && (dynamic.width() != width || dynamic.height() != height) {
        dynamic = dynamic.resize_to_fill(width, height, image::imageops::FilterType::Triangle);
    }
    let mut png = Vec::new();
    dynamic
        .write_to(&mut Cursor::new(&mut png), image::ImageFormat::Png)
        .map_err(|e| format!("screenshot: PNG encode failed: {e}"))?;
    Ok(BASE64_STANDARD.encode(&png))
}
