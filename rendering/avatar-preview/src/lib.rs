//! Avatar preview renderer: the Explorer's avatar look as a standalone, single-threaded wasm
//! module. Size-and-feasibility spike: boots the shared Explorer plugins (embedded shaders,
//! scene material, collectibles over ipfs) and renders one base body with the toon material.

use std::f32::consts::PI;

use assets::EmbedAssetsPlugin;
use bevy::{
    asset::UnapprovedPathMode,
    gltf::GltfLoaderSettings,
    core_pipeline::core_3d::{AlphaMask3d, Opaque3d, Transparent3d},
    prelude::*,
    render::{
        mesh::{skinning::SkinnedMesh, MeshTag},
        primitives::Aabb,
        render_phase::DrawFunctions,
        RenderApp,
    },
    window::WindowResolution,
};
use boimp::bake::ImposterPhaseItem;
use collectibles::{
    base_wearables,
    wearables::{Wearable, WearableUrn},
    CollectibleError, CollectibleManager, CollectiblesPlugin,
};
use common::structs::{AppConfig, PreviewMode, ShowOutOfBounds};
use ipfs::{EndpointConfig, IpfsIoPlugin, IpfsResource, ServerAbout};
use scene_material::{
    SceneBoundPlugin, SceneMaterial, SceneMaterialExt, SCENE_MATERIAL_NO_DITHERING_MESH_TAG,
    SCENE_MATERIAL_OUTLINE_BLACK_MESH_TAG, SCENE_MATERIAL_TOON_MESH_TAG,
};

/// What the page asked for.
#[derive(Resource, Clone, Debug)]
pub struct PreviewRequest {
    pub body_shape: WearableUrn,
    pub content_url: String,
    pub lambdas_url: String,
}

/// Diagnostic switches from the page (`?flags=plain,cube`).
#[derive(Resource, Default, Debug)]
pub struct PreviewFlags {
    /// keep the glTF's standard materials instead of the Explorer's scene material
    pub plain: bool,
    /// spawn a reference cube at the origin
    pub cube: bool,
    /// strip the skin component from the body meshes (renders the bind pose, no skinning)
    pub noskin: bool,
    /// reload the body glTF through bevy's loader with default asset usage (bypasses the
    /// wearable loader's main-world-only meshes)
    pub direct: bool,
}

/// The root of the spawned avatar, with the glTF it was spawned from (for material names).
#[derive(Component)]
struct PreviewAvatar(Handle<Gltf>);

/// Explorer default avatar colours (`avatar::AvatarShape::from(&UserProfile)`), linear.
const DEFAULT_SKIN: Color = Color::linear_rgb(0.6, 0.462, 0.356);
const DEFAULT_HAIR: Color = Color::linear_rgb(0.283, 0.142, 0.0);

/// A mesh whose material has been swapped for the Explorer's scene material.
#[derive(Component)]
struct Retagged;

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn start(
    canvas: String,
    body_shape: Option<String>,
    base_domain: Option<String>,
    flags: Option<String>,
) {
    console_error_panic_hook::set_once();
    let flags = flags.unwrap_or_default();
    let flags = PreviewFlags {
        plain: flags.split(',').any(|f| f == "plain"),
        cube: flags.split(',').any(|f| f == "cube"),
        noskin: flags.split(',').any(|f| f == "noskin"),
        direct: flags.split(',').any(|f| f == "direct"),
    };
    run(&canvas, body_shape.as_deref(), base_domain.as_deref(), flags);
}

pub fn run(canvas: &str, body_shape: Option<&str>, base_domain: Option<&str>, flags: PreviewFlags) {
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
    app.add_systems(Startup, (point_at_catalyst, setup_stage));
    app.add_systems(
        Update,
        (spawn_body, promote_meshes, retag_materials, spin, debug_meshes, strip_skins),
    );
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

/// Resolves the body shape through the Explorer's collectible manager and spawns its glTF scene.
fn spawn_body(
    mut commands: Commands,
    request: Res<PreviewRequest>,
    flags: Res<PreviewFlags>,
    asset_server: Res<AssetServer>,
    mut wearables: CollectibleManager<Wearable>,
    gltfs: Res<Assets<Gltf>>,
    mut done: Local<bool>,
    mut direct: Local<Option<Handle<Gltf>>>,
) {
    if *done {
        return;
    }
    match wearables.get_representation(&request.body_shape, request.body_shape.as_str()) {
        Ok(wearable) => {
            let Some(model) = &wearable.model else {
                warn!("body shape has no model");
                *done = true;
                return;
            };
            let handle = if flags.direct {
                let Some(path) = model.gltf.path() else {
                    warn!("body glTF has no path");
                    *done = true;
                    return;
                };
                direct
                    .get_or_insert_with(|| {
                        info!("direct load of {path}");
                        asset_server.load_with_settings::<Gltf, GltfLoaderSettings>(
                            path.clone(),
                            |s| {
                                s.load_meshes = bevy::render::render_asset::RenderAssetUsages::default();
                                s.load_materials = bevy::render::render_asset::RenderAssetUsages::default();
                            },
                        )
                    })
                    .clone()
            } else {
                model.gltf.clone()
            };
            let Some(gltf) = gltfs.get(&handle) else {
                return;
            };
            let Some(scene) = gltf
                .default_scene
                .clone()
                .or_else(|| gltf.scenes.first().cloned())
            else {
                warn!("body shape glTF has no scene");
                *done = true;
                return;
            };
            commands.spawn((
                SceneRoot(scene),
                Transform::from_rotation(Quat::from_rotation_y(PI)),
                PreviewAvatar(handle.clone()),
            ));
            info!("spawned {}", request.body_shape.as_str());
            *done = true;
        }
        Err(CollectibleError::Loading) => {}
        Err(e) => {
            warn!("body shape {} failed: {e:?}", request.body_shape.as_str());
            *done = true;
        }
    }
}

/// The wearable loader keeps meshes in the main world only (`load_meshes = MAIN_WORLD` in
/// `collectibles`); the Explorer's avatar processing normalises the joint weights and then
/// promotes each mesh to the render world. The same step, for the meshes spawned here.
fn promote_meshes(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    pending: Query<(Entity, &Mesh3d), Without<Promoted>>,
) {
    use bevy::render::render_asset::RenderAssetUsages;
    for (entity, mesh) in pending.iter() {
        let Some(data) = meshes.get(&mesh.0) else {
            continue;
        };
        info!("mesh {entity} usage before promotion: {:?}", data.asset_usage);
        if !data.asset_usage.contains(RenderAssetUsages::RENDER_WORLD) {
            if let Some(data) = meshes.get_mut(&mesh.0) {
                data.normalize_joint_weights();
                data.asset_usage |= RenderAssetUsages::RENDER_WORLD;
                info!("mesh {entity} promoted to {:?}", data.asset_usage);
            }
        }
        commands.entity(entity).insert(Promoted);
    }
}

/// A mesh whose asset usage has been checked (and promoted to the render world when needed).
#[derive(Component)]
struct Promoted;

/// Swaps every glTF material for the Explorer's scene material with the avatar mesh tags, so
/// the toon ramp and the outline apply as in the Explorer.
fn retag_materials(
    mut commands: Commands,
    meshes: Query<(Entity, &MeshMaterial3d<StandardMaterial>), Without<Retagged>>,
    standard: Res<Assets<StandardMaterial>>,
    mut scene_materials: ResMut<Assets<SceneMaterial>>,
    flags: Res<PreviewFlags>,
    avatars: Query<&PreviewAvatar>,
    gltfs: Res<Assets<Gltf>>,
) {
    if flags.plain {
        return;
    }
    let count = meshes.iter().count();
    if count > 0 {
        info!("retagging {count} meshes");
    }
    // the Explorer tints materials by name: "skin" and "hair" (case-insensitive) take the
    // avatar's colours (`avatar::spawn_scenes` + `derived_scene_material`)
    let named: Vec<(String, AssetId<StandardMaterial>)> = avatars
        .iter()
        .filter_map(|avatar| gltfs.get(&avatar.0))
        .flat_map(|gltf| {
            gltf.named_materials
                .iter()
                .map(|(name, handle)| (name.to_lowercase(), handle.id()))
        })
        .collect();
    for (entity, material) in meshes.iter() {
        let Some(base) = standard.get(&material.0) else {
            continue;
        };
        let name = named
            .iter()
            .find(|(_, id)| *id == material.0.id())
            .map(|(name, _)| name.as_str())
            .unwrap_or("");
        let mut base = base.clone();
        if name.contains("skin") {
            base.base_color = DEFAULT_SKIN;
        } else if name.contains("hair") {
            base.base_color = DEFAULT_HAIR;
        }
        let handle = scene_materials.add(SceneMaterial::new_unbounded(base));
        commands
            .entity(entity)
            .remove::<MeshMaterial3d<StandardMaterial>>()
            .insert((
                MeshMaterial3d(handle),
                MeshTag(
                    SCENE_MATERIAL_TOON_MESH_TAG
                        | SCENE_MATERIAL_OUTLINE_BLACK_MESH_TAG
                        | SCENE_MATERIAL_NO_DITHERING_MESH_TAG,
                ),
                Retagged,
            ));
    }
}

/// Diagnostic (`noskin`): remove skinning so the meshes draw in their bind pose.
fn strip_skins(
    mut commands: Commands,
    flags: Res<PreviewFlags>,
    skinned: Query<Entity, (With<SkinnedMesh>, With<Mesh3d>)>,
) {
    if !flags.noskin {
        return;
    }
    for entity in skinned.iter() {
        commands.entity(entity).remove::<SkinnedMesh>();
    }
}

/// Once, a few seconds in: what every mesh looks like to the renderer.
#[allow(clippy::type_complexity)]
fn debug_meshes(
    mut frames: Local<u32>,
    meshes: Query<(
        Entity,
        Option<&Name>,
        &GlobalTransform,
        Option<&Aabb>,
        Option<&SkinnedMesh>,
        &ViewVisibility,
        &InheritedVisibility,
        &Mesh3d,
    ), With<Mesh3d>>,
    assets: Res<Assets<Mesh>>,
) {
    *frames += 1;
    if *frames != 240 {
        return;
    }
    for (entity, name, transform, aabb, skin, view, inherited, mesh) in meshes.iter() {
        let data = assets.get(&mesh.0);
        info!(
            "mesh {entity} {:?} at {:?} aabb={:?} skinned={} inherited={} view={} usage={:?} verts={:?} attrs={:?}",
            name.map(|n| n.as_str()),
            transform.translation(),
            aabb.map(|a| (a.center, a.half_extents)),
            skin.map(|s| s.joints.len()).unwrap_or(0),
            inherited.get(),
            view.get(),
            data.map(|d| d.asset_usage),
            data.map(|d| d.count_vertices()),
            data.map(|d| d.attributes().map(|(a, _)| a.name).collect::<Vec<_>>()),
        );
    }
}

fn spin(time: Res<Time>, mut avatars: Query<&mut Transform, With<PreviewAvatar>>) {
    for mut transform in avatars.iter_mut() {
        transform.rotate_y(time.delta_secs() * 0.4);
    }
}
