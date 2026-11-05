//! Avatar preview renderer: the Explorer's avatar look as a standalone, single-threaded wasm
//! module. It boots the shared Explorer plugins (embedded shaders, scene material, collectibles
//! over ipfs, the avatar renderer) and renders one avatar from a body shape and wearable urns.

use assets::EmbedAssetsPlugin;
use avatar::{AvatarRenderPlugin, AvatarSelection, AvatarShape};
use bevy::{
    asset::UnapprovedPathMode,
    core_pipeline::core_3d::{AlphaMask3d, Opaque3d, Transparent3d},
    prelude::*,
    render::{render_phase::DrawFunctions, RenderApp},
    window::WindowResolution,
};
use boimp::bake::ImposterPhaseItem;
use collectibles::{base_wearables, wearables::WearableUrn, CollectiblesPlugin};
use common::structs::{AppConfig, AvatarDynamicState, PreviewMode, ShowOutOfBounds};
use dcl_component::proto_components::sdk::components::PbAvatarShape;
use ipfs::{EndpointConfig, IpfsIoPlugin, IpfsResource, ServerAbout};
use scene_material::SceneBoundPlugin;

/// What the page asked for.
#[derive(Resource, Clone, Debug)]
pub struct PreviewRequest {
    pub body_shape: WearableUrn,
    pub wearables: Vec<String>,
    pub content_url: String,
    pub lambdas_url: String,
}

/// Diagnostic switches from the page (`?flags=cube`).
#[derive(Resource, Default, Debug)]
pub struct PreviewFlags {
    /// spawn a reference cube at the origin
    pub cube: bool,
}

/// The root of the previewed avatar.
#[derive(Component)]
struct PreviewAvatar;

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn start(
    canvas: String,
    body_shape: Option<String>,
    wearables: Option<String>,
    base_domain: Option<String>,
    flags: Option<String>,
) {
    console_error_panic_hook::set_once();
    let flags = flags.unwrap_or_default();
    let flags = PreviewFlags {
        cube: flags.split(',').any(|f| f == "cube"),
    };
    let wearables = wearables
        .unwrap_or_default()
        .split(',')
        .map(str::trim)
        .filter(|urn| !urn.is_empty())
        .map(str::to_owned)
        .collect();
    run(
        &canvas,
        body_shape.as_deref(),
        wearables,
        base_domain.as_deref(),
        flags,
    );
}

pub fn run(
    canvas: &str,
    body_shape: Option<&str>,
    wearables: Vec<String>,
    base_domain: Option<&str>,
    flags: PreviewFlags,
) {
    info!("{flags:?}");
    if let Some(domain) = base_domain {
        if let Err(e) = common::base_domain::set(domain) {
            error!("{e}");
        }
    }
    let catalyst = common::base_domain::url(common::base_domain::Service::Catalyst, "");
    let body_shape = body_shape
        .and_then(|urn| WearableUrn::new(urn).ok())
        .unwrap_or_else(base_wearables::default_bodyshape_urn);
    let request = PreviewRequest {
        body_shape,
        wearables,
        content_url: format!("{catalyst}/content"),
        lambdas_url: format!("{catalyst}/lambdas"),
    };

    let mut app = App::new();
    app.add_plugins(
        DefaultPlugins
            .set(WindowPlugin {
                primary_window: Some(Window {
                    canvas: Some(canvas.to_owned()),
                    fit_canvas_to_parent: true,
                    resolution: WindowResolution::new(512.0, 768.0),
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
        ImposterDrawFunctionsPlugin,
        boimp::bake::ImposterBakePlugin,
        SceneBoundPlugin,
        bevy_kira_audio::AudioPlugin,
        CollectiblesPlugin,
        AvatarRenderPlugin,
    ));
    app.insert_resource(AppConfig::default())
        .insert_resource(PreviewMode::default())
        .insert_resource(ShowOutOfBounds(false))
        .insert_resource(AmbientLight {
            color: Color::srgb(0.85, 0.85, 1.0),
            brightness: 575.0,
            ..default()
        })
        .insert_resource(request)
        .insert_resource(flags);
    app.add_systems(Startup, (point_at_catalyst, setup_stage, spawn_avatar));
    app.add_systems(Update, (spin, debug_meshes));
    app.run();
}

/// `boimp::bake::ImposterBakePlugin` creates its render-side resources in `build`, where the
/// render sub-app does not exist yet when the renderer is created asynchronously (single-threaded
/// wasm), so the scene material's imposter commands panic in `finish`. This creates the draw
/// function registries in `finish`, ahead of the scene material plugin. The proper fix is in
/// boimp: move that initialisation to `finish`.
struct ImposterDrawFunctionsPlugin;

impl Plugin for ImposterDrawFunctionsPlugin {
    fn build(&self, _: &mut App) {}

    fn finish(&self, app: &mut App) {
        let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
            return;
        };
        render_app
            .init_resource::<DrawFunctions<ImposterPhaseItem<Opaque3d>>>()
            .init_resource::<DrawFunctions<ImposterPhaseItem<AlphaMask3d>>>()
            .init_resource::<DrawFunctions<ImposterPhaseItem<Transparent3d>>>();
    }
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

fn setup_stage(
    mut commands: Commands,
    flags: Res<PreviewFlags>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    if flags.cube {
        commands.spawn((
            Mesh3d(meshes.add(Cuboid::new(0.5, 0.5, 0.5))),
            MeshMaterial3d(materials.add(Color::srgb(0.2, 0.9, 0.3))),
            Transform::from_xyz(0.0, 1.0, 0.0),
        ));
    }
    commands.spawn((
        Camera3d::default(),
        Camera {
            hdr: true,
            clear_color: ClearColorConfig::Custom(Color::srgb(0.16, 0.14, 0.2)),
            ..default()
        },
        Projection::from(PerspectiveProjection {
            fov: 26.2f32.to_radians(),
            near: 0.05,
            ..default()
        }),
        Transform::from_xyz(0.0, 1.1, 6.0).looking_at(Vec3::new(0.0, 1.0, 0.0), Vec3::Y),
        platform::default_camera_components(),
    ));
    commands.spawn((
        DirectionalLight {
            illuminance: 8000.0,
            shadows_enabled: true,
            ..default()
        },
        Transform::from_rotation(Quat::from_euler(EulerRot::YXZ, -0.6, -1.0, 0.0)),
    ));
}

/// The avatar: a selection the Explorer's renderer dresses, animates and emotes, exactly as a
/// scene-placed avatar in the world, minus the scene.
fn spawn_avatar(mut commands: Commands, request: Res<PreviewRequest>) {
    let shape = PbAvatarShape {
        id: "preview".to_owned(),
        body_shape: Some(request.body_shape.as_str().to_owned()),
        wearables: request.wearables.clone(),
        ..Default::default()
    };
    commands.spawn((
        // the shape component itself, as on the Explorer's player and NPC entities: the
        // attach-point observer keys on it
        AvatarShape(shape.clone()),
        AvatarSelection {
            scene: None,
            shape: AvatarShape(shape),
            automatic_delete: false,
            disable_dither: true,
        },
        AvatarDynamicState::default(),
        Transform::default(),
        Visibility::default(),
        PreviewAvatar,
    ));
    info!(
        "avatar {} with {} wearables",
        request.body_shape.as_str(),
        request.wearables.len()
    );
}

/// Once, a few seconds in: what every mesh looks like to the renderer.
#[allow(clippy::type_complexity)]
fn debug_meshes(
    mut frames: Local<u32>,
    meshes: Query<(Entity, Option<&Name>, &ViewVisibility, &Mesh3d)>,
    assets: Res<Assets<Mesh>>,
) {
    *frames += 1;
    if *frames != 300 {
        return;
    }
    for (entity, name, view, mesh) in meshes.iter() {
        let data = assets.get(&mesh.0);
        info!(
            "mesh {entity} {:?} view={} usage={:?} verts={:?}",
            name.map(|n| n.as_str()),
            view.get(),
            data.map(|d| d.asset_usage),
            data.map(|d| d.count_vertices()),
        );
    }
}

fn spin(time: Res<Time>, mut avatars: Query<&mut Transform, With<PreviewAvatar>>) {
    for mut transform in avatars.iter_mut() {
        transform.rotate_y(time.delta_secs() * 0.4);
    }
}
