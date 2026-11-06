//! Avatar preview renderer: the Explorer's avatar look as a standalone, single-threaded wasm
//! module. It boots the shared Explorer plugins (embedded shaders, scene material, collectibles
//! over ipfs, the avatar renderer) and renders one avatar that the page drives through the host
//! surface in `host` (commands in, events out).

mod audio;
mod host;
mod stage;

use assets::EmbedAssetsPlugin;
use avatar::{AvatarRenderPlugin, AvatarSelection, AvatarShape};
use bevy::{asset::UnapprovedPathMode, prelude::*, window::WindowResolution};
use collectibles::CollectiblesPlugin;
use common::structs::{AppConfig, AvatarDynamicState, PreviewMode, ShowOutOfBounds};
use ipfs::{EndpointConfig, IpfsIoPlugin, IpfsResource, ServerAbout};
use scene_material::SceneBoundPlugin;

pub use host::{Command, Event, OrbitCamera};
use host::{HostPlugin, PreviewCamera};

/// Where content comes from.
#[derive(Resource, Clone, Debug)]
pub struct PreviewRequest {
    pub content_url: String,
    pub lambdas_url: String,
}

/// The root of the previewed avatar.
#[derive(Component)]
pub struct PreviewAvatar;

/// Starts the renderer on `canvas` (a CSS selector). `base_domain` picks the catalyst
/// (`peer.<domain>`); `on_event` receives every event as a JSON string. The avatar appears with
/// the first `outfit` command (`preview_command`).
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn start(
    canvas: String,
    base_domain: Option<String>,
    on_event: js_sys::Function,
    flags: Option<String>,
) {
    console_error_panic_hook::set_once();
    host::set_event_callback(on_event);
    run(&canvas, base_domain.as_deref(), &flags.unwrap_or_default());
}

/// Diagnostic switches (`flags`, comma-separated) for the camera pipeline.
#[derive(Resource, Default, Debug, Clone)]
pub struct StageFlags {
    pub no_bloom: bool,
    pub no_msaa: bool,
    pub sdr: bool,
    pub clear_surface: bool,
}

pub fn run(canvas: &str, base_domain: Option<&str>, flags: &str) {
    let stage = StageFlags {
        no_bloom: flags.split(',').any(|f| f == "nobloom"),
        no_msaa: flags.split(',').any(|f| f == "nomsaa"),
        sdr: flags.split(',').any(|f| f == "sdr"),
        clear_surface: flags.split(',').any(|f| f == "clearsurface"),
    };
    info!("{stage:?}");
    if let Some(domain) = base_domain {
        if let Err(e) = common::base_domain::set(domain) {
            error!("{e}");
        }
    }
    let catalyst = common::base_domain::url(common::base_domain::Service::Catalyst, "");
    let request = PreviewRequest {
        content_url: format!("{catalyst}/content"),
        lambdas_url: format!("{catalyst}/lambdas"),
    };

    let mut app = App::new();
    // the Explorer's preview mode, set before its plugins build: the scene material then leaves
    // out imposter baking, which the preview never does and which needs fragment-writable
    // storage buffers WebGL2 does not have
    app.insert_resource(PreviewMode {
        is_preview: true,
        ..default()
    });
    app.add_plugins(
        DefaultPlugins
            .set(WindowPlugin {
                primary_window: Some(Window {
                    canvas: Some(canvas.to_owned()),
                    fit_canvas_to_parent: true,
                    resolution: WindowResolution::new(512.0, 768.0),
                    // the page decides the background: a colour, or none through the canvas.
                    // The surface must present alpha too: the WebGPU default is opaque, which
                    // discards it at presentation and shows black where nothing was drawn.
                    // WebGL2 offers only `Opaque`, but its canvas keeps the browser's default
                    // premultiplied alpha, so the transparent clear still shows through.
                    transparent: true,
                    composite_alpha_mode: if cfg!(feature = "webgl2") {
                        bevy::window::CompositeAlphaMode::Opaque
                    } else {
                        bevy::window::CompositeAlphaMode::PreMultiplied
                    },
                    ..default()
                }),
                ..default()
            })
            .set(AssetPlugin {
                unapproved_path_mode: UnapprovedPathMode::Allow,
                ..default()
            })
            .set(
                bevy::gltf::GltfPlugin::default()
                    .with_uri_resolver(std::sync::Arc::new(ipfs::ipfs_path::resolve_content_uri)),
            )
            .add_before::<AssetPlugin>(IpfsIoPlugin {
                preview: false,
                assets_root: None,
                starting_realm: None,
                content_server_override: None,
                num_slots: 8,
            }),
    );
    app.add_plugins((
        console::ConsolePlugin {
            add_bevy_console: false,
        },
        EmbedAssetsPlugin,
        SceneBoundPlugin,
        bevy_kira_audio::AudioPlugin,
        CollectiblesPlugin,
        AvatarRenderPlugin,
        HostPlugin,
        stage::StagePlugin,
        audio::PreviewAudioPlugin,
    ));
    app.insert_resource(AppConfig::default())
        .insert_resource(ShowOutOfBounds(false))
        .insert_resource(AmbientLight {
            color: Color::srgb(0.85, 0.85, 1.0),
            brightness: 575.0,
            ..default()
        })
        .insert_resource(request)
        .insert_resource(stage);
    app.add_systems(Startup, (point_at_catalyst, setup_stage));
    app.add_systems(Update, spawn_avatar_on_first_outfit);
    app.run();
}

/// The collectible resolver asks the realm's content server for pointers; there is no realm
/// here, so the catalyst is the realm.
fn point_at_catalyst(ipfs: Res<IpfsResource>, request: Res<PreviewRequest>) {
    ipfs.set_realm_about(ServerAbout {
        content: Some(EndpointConfig {
            healthy: true,
            public_url: request.content_url.clone(),
        }),
        comms: None,
        configurations: None,
        lambdas: Some(EndpointConfig {
            healthy: true,
            public_url: request.lambdas_url.clone(),
        }),
    });
    info!("content server: {}", request.content_url);
}

fn setup_stage(mut commands: Commands, orbit: Res<OrbitCamera>, flags: Res<StageFlags>) {
    let mut camera = commands.spawn((
        Camera3d::default(),
        Camera {
            hdr: !flags.sdr,
            clear_color: ClearColorConfig::Custom(Color::srgb(0.16, 0.14, 0.2)),
            output_mode: if flags.clear_surface {
                bevy::render::camera::CameraOutputMode::Write {
                    blend_state: None,
                    clear_color: ClearColorConfig::Custom(Color::NONE),
                }
            } else {
                bevy::render::camera::CameraOutputMode::default()
            },
            ..default()
        },
        host::projection_for(&orbit),
        Transform::from_xyz(0.0, 1.0, 6.0).looking_at(Vec3::new(0.0, 1.0, 0.0), Vec3::Y),
        platform::default_camera_components(),
        bevy::render::camera::Exposure::BLENDER,
        PreviewCamera,
    ));
    if flags.no_bloom {
        camera.remove::<bevy::core_pipeline::bloom::Bloom>();
    } else {
        // only what is brighter than white blooms (emissive parts), as in the Unity preview: the
        // Explorer's thresholdless bloom haloes the whole avatar on a transparent page
        camera.insert(bevy::core_pipeline::bloom::Bloom {
            intensity: 0.15,
            prefilter: bevy::core_pipeline::bloom::BloomPrefilter {
                threshold: 1.0,
                threshold_softness: 0.5,
            },
            ..bevy::core_pipeline::bloom::Bloom::OLD_SCHOOL
        });
    }
    if flags.no_msaa {
        camera.insert(Msaa::Off);
    }
    // WebGL2 can neither read a depth prepass back in a shader nor make multisampled textures:
    // no prepasses (so no avatar outline, which edges on them) and FXAA in place of MSAA
    if cfg!(feature = "webgl2") {
        camera
            .remove::<(
                bevy::core_pipeline::prepass::DepthPrepass,
                bevy::core_pipeline::prepass::NormalPrepass,
            )>()
            .insert((Msaa::Off, bevy::core_pipeline::fxaa::Fxaa::default()));
    }
}

/// The avatar entity exists from the first `outfit` command on: a selection the Explorer's
/// renderer dresses, animates and emotes, exactly as a scene-placed avatar in the world, minus
/// the scene. `host::apply_commands` updates its shape afterwards.
fn spawn_avatar_on_first_outfit(
    mut commands: Commands,
    avatars: Query<(), With<PreviewAvatar>>,
    mut pending: ResMut<host::PendingOutfit>,
) {
    if !avatars.is_empty() {
        pending.0 = None;
        return;
    }
    let Some(shape) = pending.0.take() else {
        return;
    };
    info!(
        "avatar {:?} with {} wearables",
        shape.body_shape,
        shape.wearables.len()
    );
    commands.spawn((
        AvatarShape(shape.clone()),
        AvatarSelection {
            scene: None,
            shape: AvatarShape(shape),
            automatic_delete: false,
            disable_dither: true,
        },
        AvatarDynamicState::default(),
        avatar::animate::EmoteControl::default(),
        // the Explorer builds the body facing away from its player's camera; the preview's camera
        // stands in front
        Transform::from_rotation(Quat::from_rotation_y(std::f32::consts::PI)),
        Visibility::default(),
        PreviewAvatar,
    ));
}
