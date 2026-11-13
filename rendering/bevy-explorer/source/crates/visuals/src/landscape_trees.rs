//! Trees, bushes and stones from unity-explorer's baked layout, so they stand where Unity's do.
//!
//! Only parcels confirmed empty carry them: a parcel that is unknown, occupied or outside the
//! terrain counts as occupied, and an instance whose footprint reaches such a parcel is hidden.
//! They stand within the Landscape Distance of the player, on the eased terrain.

use bevy::{
    asset::{embedded_asset, embedded_path, RenderAssetUsages},
    ecs::system::SystemParam,
    gltf::GltfAssetLabel,
    platform::collections::HashMap,
    prelude::*,
    render::{mesh::MeshAabb, render_resource::ShaderRef, view::VisibilitySystems},
};
use bevy_console::ConsoleCommand;
use common::{
    partition::PARCEL_SIZE,
    sets::RealmLifecycle,
    structs::{LandscapeDistance, PrimaryUser},
};
use console::DoAddConsoleCommand;
use scene_runner::{
    initialize_scene::{PointerResult, ScenePointers},
    vec3_to_parcel,
};
use serde_json::json;

use crate::{
    solids::{prototype_solids, Solid},
    terrain::{TerrainSet, TerrainSurface},
};
use layout::{hidden_by_occupancy, TreeLayout, TreeLayoutLoader};
use material::{
    add_mips, stone_material, surface_material, StoneMaterial, Surface, SurfaceMaterial,
    TreeMaterial, TreeSway, ALL_SURFACES,
};
use prototypes::PROTOTYPES;

pub(crate) mod layout;
pub(crate) mod material;
pub(crate) mod prototypes;

const ASSET_DIR: &str = "embedded://landscape/trees";

pub(crate) struct LandscapeTreesPlugin;

impl Plugin for LandscapeTreesPlugin {
    fn build(&self, app: &mut App) {
        embed_shaders(app);
        app.init_asset::<TreeLayout>()
            .init_asset_loader::<TreeLayoutLoader>()
            .add_plugins((
                MaterialPlugin::<TreeMaterial>::default(),
                MaterialPlugin::<StoneMaterial>::default(),
            ))
            .init_resource::<TreeWind>()
            .add_console_command::<TreeWindCommand, _>(tree_wind_command)
            .add_systems(Startup, setup)
            .add_systems(Update, (add_texture_mips, apply_tree_wind))
            .add_systems(
                PostUpdate,
                dress_trees.before(VisibilitySystems::CalculateBounds),
            );
        add_spawn_systems(app);
    }
}

/// Spawning and dropping the trees, after the realm has resolved this frame's pointers, so a
/// parcel resolved empty has its trees in that frame. Ordering the terrain itself after the realm
/// would close a cycle through the collider update, so a tree spawned here gets its colliders in
/// the next frame's collider update.
pub(crate) fn add_spawn_systems(app: &mut App) {
    app.add_systems(
        PostUpdate,
        update_trees.after(TerrainSet).after(RealmLifecycle),
    );
}

/// The farthest the wind moves a vertex, in metres: the sway at full force (a crest of the wave
/// plus the noise's largest turbulence) along the wind with its fifth upwards, and the flutter
/// along a direction of up to unit size on each axis.
fn wind_reach(wind: &material::Wind) -> f32 {
    // three octaves of noise in 0..1 at amplitudes 0.5, 0.25 and 0.125
    const MAX_FBM: f32 = 0.875;
    let force = (1.0 + MAX_FBM * wind.turbulence) * wind.strength;
    force * (1.0f32 + 0.2 * 0.2).sqrt() + 3.0f32.sqrt() * wind.flutter_strength
}

/// The bounds padding, in mesh units, that keeps a swaying mesh inside its bounds at the smallest
/// scale any prototype is drawn at; 0 for a still surface.
pub(crate) fn wind_padding(wind: Option<&material::Wind>) -> f32 {
    let smallest = PROTOTYPES
        .iter()
        .map(|prototype| prototype.scale_xz.0.min(prototype.scale_y.0))
        .fold(f32::INFINITY, f32::min);
    wind.map_or(0.0, |wind| wind_reach(wind) / smallest)
}

fn embed_shaders(app: &mut App) {
    embedded_asset!(app, "landscape_tree.wgsl");
    embedded_asset!(app, "soft_shadows.wgsl");
}

pub(crate) fn tree_vertex_shader() -> ShaderRef {
    ShaderRef::Path(
        format!(
            "embedded://{}",
            embedded_path!("landscape_tree.wgsl").display()
        )
        .into(),
    )
}

pub(crate) fn soft_shadows_fragment() -> ShaderRef {
    ShaderRef::Path(
        format!(
            "embedded://{}",
            embedded_path!("soft_shadows.wgsl").display()
        )
        .into(),
    )
}

/// A spawned instance of the layout: its parcel and its index among the parcel's instances.
#[derive(Component)]
pub(crate) struct Tree {
    pub parcel: IVec2,
    pub index: u32,
}

#[derive(Resource)]
pub(crate) struct LandscapeTreeAssets {
    pub genesis: Handle<TreeLayout>,
    pub worlds: Handle<TreeLayout>,
    /// By prototype.
    pub scenes: [Handle<Scene>; 8],
}

/// The surfaces' materials, the models' own materials they replace, and the textures they use.
#[derive(Resource)]
struct TreeMaterials {
    /// By `Surface`.
    surfaces: Vec<SurfaceMaterial>,
    by_model_material: HashMap<AssetId<StandardMaterial>, (Surface, SurfaceMaterial)>,
    /// Held so the models' materials keep the ids `by_model_material` knows them by.
    _model_materials: Vec<Handle<StandardMaterial>>,
    textures: Vec<Handle<Image>>,
}

/// Parcels to scan either side of the player's parcel for instances within `distance_m`.
pub(crate) fn tree_radius_parcels(distance_m: f32) -> i32 {
    (distance_m / PARCEL_SIZE).floor() as i32 + 1
}

fn setup(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut materials: ResMut<Assets<TreeMaterial>>,
    mut stone_materials: ResMut<Assets<StoneMaterial>>,
    wind: Res<TreeWind>,
) {
    let mut textures = Vec::new();
    let surfaces: Vec<_> = ALL_SURFACES
        .iter()
        .map(|surface| {
            let spec = surface.spec();
            if let Some(stone) = &spec.stone {
                let (material, loading) = stone_material(spec, stone, &asset_server);
                textures.extend(loading);
                SurfaceMaterial::Stone(stone_materials.add(material))
            } else {
                let (material, loading) = surface_material(spec, &asset_server, wind.0);
                textures.extend(loading);
                SurfaceMaterial::Tree(materials.add(material))
            }
        })
        .collect();
    let mut by_model_material = HashMap::new();
    let mut model_materials = Vec::new();
    let scenes = std::array::from_fn(|index| {
        let prototype = &PROTOTYPES[index];
        let path = format!("{ASSET_DIR}/{}.glb", prototype.asset);
        for (slot, surface) in prototype.surfaces.iter().enumerate() {
            let model_material: Handle<StandardMaterial> = asset_server.load(
                GltfAssetLabel::Material {
                    index: slot,
                    is_scale_inverted: false,
                }
                .from_asset(path.clone()),
            );
            by_model_material.insert(
                model_material.id(),
                (*surface, surfaces[*surface as usize].clone()),
            );
            model_materials.push(model_material);
        }
        asset_server.load(GltfAssetLabel::Scene(0).from_asset(path))
    });
    commands.insert_resource(LandscapeTreeAssets {
        genesis: asset_server.load("embedded://landscape/layout/genesis.trees"),
        worlds: asset_server.load("embedded://landscape/layout/worlds.trees"),
        scenes,
    });
    commands.insert_resource(TreeMaterials {
        surfaces,
        by_model_material,
        _model_materials: model_materials,
        textures,
    });
}

/// The textures load without mips: give them theirs, then let the main world drop its copy.
fn add_texture_mips(
    mut events: EventReader<AssetEvent<Image>>,
    mut images: ResMut<Assets<Image>>,
    materials: Option<Res<TreeMaterials>>,
) {
    let Some(materials) = materials else {
        events.clear();
        return;
    };
    for event in events.read() {
        let AssetEvent::LoadedWithDependencies { id } = event else {
            continue;
        };
        if !materials.textures.iter().any(|texture| texture.id() == *id) {
            continue;
        }
        if let Some(image) = images.get_mut(*id) {
            if !add_mips(image) {
                warn!("landscape tree texture {id} is not square RGBA8; drawn without mips");
            }
            image.asset_usage = RenderAssetUsages::RENDER_WORLD;
        }
    }
}

/// A model mesh as the scene spawns it.
type ModelMesh = (
    Entity,
    &'static MeshMaterial3d<StandardMaterial>,
    &'static Mesh3d,
);

/// The models carry placeholder materials; swap in the surfaces' as their meshes spawn, and pad a
/// swaying mesh's bounds by the wind's reach so it is not culled while it leans out of them.
fn dress_trees(
    mut commands: Commands,
    materials: Option<Res<TreeMaterials>>,
    meshes: Res<Assets<Mesh>>,
    added: Query<ModelMesh, Added<MeshMaterial3d<StandardMaterial>>>,
) {
    let Some(materials) = materials else { return };
    for (entity, material, mesh) in &added {
        let Some((surface, surface_material)) = materials.by_model_material.get(&material.id())
        else {
            continue;
        };
        let mut entity = commands.entity(entity);
        entity.remove::<MeshMaterial3d<StandardMaterial>>();
        match surface_material {
            SurfaceMaterial::Tree(handle) => entity.insert(MeshMaterial3d(handle.clone())),
            SurfaceMaterial::Stone(handle) => entity.insert(MeshMaterial3d(handle.clone())),
        };
        let padding = wind_padding(surface.spec().wind.as_ref());
        if padding > 0.0 {
            if let Some(mut aabb) = meshes.get(mesh).and_then(|mesh| mesh.compute_aabb()) {
                aabb.half_extents += Vec3A::splat(padding);
                entity.insert(aabb);
            }
        }
    }
}

/// Whether the trees and bushes sway. Still, they stand in their rest pose, so two frames of them
/// agree whenever they were taken (the end-to-end suite's visual references need that).
#[derive(Resource)]
pub(crate) struct TreeWind(pub(crate) bool);

impl Default for TreeWind {
    fn default() -> Self {
        Self(true)
    }
}

/// Read or set whether the landscape's trees and bushes sway in the wind.
#[derive(clap::Parser, ConsoleCommand)]
#[command(name = "/tree_wind")]
struct TreeWindCommand {
    enabled: Option<bool>,
}

fn tree_wind_command(mut cmd: ConsoleCommand<TreeWindCommand>, mut wind: ResMut<TreeWind>) {
    let Some(Ok(args)) = cmd.take() else { return };
    if let Some(enabled) = args.enabled {
        wind.0 = enabled;
    }
    cmd.reply_ok(json!({ "enabled": wind.0 }).to_string());
}

/// The wind in the surfaces' materials: their own, or none.
fn apply_tree_wind(
    wind: Res<TreeWind>,
    materials: Option<Res<TreeMaterials>>,
    mut assets: ResMut<Assets<TreeMaterial>>,
) {
    let Some(materials) = materials else { return };
    if !wind.is_changed() {
        return;
    }
    for (surface, material) in ALL_SURFACES.iter().zip(&materials.surfaces) {
        let SurfaceMaterial::Tree(handle) = material else {
            continue;
        };
        if let Some(material) = assets.get_mut(handle) {
            material.extension = TreeSway::new(surface.spec().wind.as_ref(), wind.0);
        }
    }
}

#[derive(SystemParam)]
struct Landscape<'w> {
    surface: Res<'w, TerrainSurface>,
    pointers: Res<'w, ScenePointers>,
    distance: Res<'w, LandscapeDistance>,
    assets: Res<'w, LandscapeTreeAssets>,
    layouts: Res<'w, Assets<TreeLayout>>,
}

#[derive(Default)]
struct TreeState {
    centre: Option<IVec2>,
    world: Option<bool>,
    /// Something changed that the trees have not followed yet.
    pending: bool,
}

/// An instance: its parcel and its index among the parcel's instances.
type Key = (IVec2, u32);

/// The layout's instances within `distance` metres of `player` (horizontally), with their
/// prototype and their transform at height 0. `empty` says which parcels are confirmed empty
/// landscape; any other parcel counts as occupied and hides an instance that reaches it.
fn desired_trees(
    layout: &TreeLayout,
    player: Vec3,
    distance: f32,
    empty: impl Fn(IVec2) -> bool,
) -> HashMap<Key, (usize, Transform)> {
    let centre = vec3_to_parcel(player);
    let radius = tree_radius_parcels(distance);
    let (min, max_excl) = layout.bounds();
    let from = (centre - radius).max(min);
    let to = (centre + radius + 1).min(max_excl);
    let mut desired = HashMap::new();
    for y in from.y..to.y {
        for x in from.x..to.x {
            let parcel = IVec2::new(x, y);
            if !empty(parcel) {
                continue;
            }
            for (index, instance) in layout.instances(parcel).iter().enumerate() {
                let transform = instance.transform(parcel, |_, _| 0.0);
                let position = transform.translation.xz();
                if position.distance(player.xz()) > distance {
                    continue;
                }
                let prototype = usize::from(instance.prototype);
                let reach = PROTOTYPES[prototype].radius * transform.scale.x;
                if hidden_by_occupancy(position, reach, |parcel| !empty(parcel)) {
                    continue;
                }
                desired.insert((parcel, index as u32), (prototype, transform));
            }
        }
    }
    desired
}

/// Spawn and drop the layout's instances around the player, and keep them on the eased terrain.
fn update_trees(
    mut commands: Commands,
    landscape: Landscape,
    player: Query<&GlobalTransform, With<PrimaryUser>>,
    mut trees: Query<(Entity, &Tree, &mut Transform)>,
    mut state: Local<TreeState>,
) {
    let Ok(player) = player.single() else {
        return;
    };
    let position = player.translation();
    if !position.is_finite() {
        return;
    }
    let Landscape {
        surface,
        pointers,
        distance,
        assets,
        layouts,
    } = landscape;
    let centre = vec3_to_parcel(position);
    let world = pointers.terrain().is_world();
    let changes = surface.changes();

    state.pending |= state.centre != Some(centre)
        || state.world != Some(world)
        || pointers.is_changed()
        || distance.is_changed()
        || changes.relayout
        || changes.targets.is_some();
    let layout = layouts.get(if world {
        &assets.worlds
    } else {
        &assets.genesis
    });
    // a realm of the other kind: the old layout's trees go at once, whether or not the new
    // layout has loaded
    let switched = state.world != Some(world);
    if switched {
        for (entity, _, _) in &trees {
            commands.entity(entity).despawn();
        }
        state.world = Some(world);
    }
    if let (true, Some(layout)) = (state.pending, layout) {
        state.centre = Some(centre);
        state.pending = false;
        // unknown parcels stay clear until the realm confirms they are empty
        let mut desired = desired_trees(layout, position, distance.0, |parcel| {
            surface.contains(parcel) && matches!(pointers.get(parcel), Some(PointerResult::Nothing))
        });
        // after a switch the old trees are already going
        if !switched {
            for (entity, tree, _) in &trees {
                if desired.remove(&(tree.parcel, tree.index)).is_none() {
                    commands.entity(entity).despawn();
                }
            }
        }
        for ((parcel, index), (prototype, mut transform)) in desired {
            let translation = transform.translation;
            transform.translation.y = surface.height(translation.x, -translation.z);
            let mut tree = commands.spawn((
                Name::new(PROTOTYPES[prototype].name),
                Tree { parcel, index },
                SceneRoot(assets.scenes[prototype].clone()),
                transform,
            ));
            let solids = prototype_solids(prototype);
            if !solids.is_empty() {
                tree.insert(Solid(solids));
            }
        }
    }

    // parcels whose values reach a tree's parcel through bilinear sampling
    let resnap = |parcel: IVec2| {
        changes.relayout
            || changes.params
            || changes.eased.is_some_and(|(min, max)| {
                (min - 1).cmple(parcel).all() && (max + 1).cmpge(parcel).all()
            })
    };
    for (_, tree, mut transform) in &mut trees {
        if !resnap(tree.parcel) {
            continue;
        }
        let y = surface.height(transform.translation.x, -transform.translation.z);
        if transform.translation.y != y {
            transform.translation.y = y;
        }
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod spawn_tests;
