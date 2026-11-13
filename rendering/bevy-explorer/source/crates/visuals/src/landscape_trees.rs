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
    render::render_resource::ShaderRef,
};
use bevy_console::ConsoleCommand;
use common::{
    partition::PARCEL_SIZE,
    sets::PostUpdateSets,
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
use material::{add_mips, surface_material, TreeMaterial, TreeSway, ALL_SURFACES};
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
            .add_plugins(MaterialPlugin::<TreeMaterial>::default())
            .init_resource::<TreeWind>()
            .add_console_command::<TreeWindCommand, _>(tree_wind_command)
            .add_systems(Startup, setup)
            .add_systems(Update, (add_texture_mips, apply_tree_wind))
            .add_systems(
                PostUpdate,
                (
                    update_trees
                        .after(TerrainSet)
                        .before(PostUpdateSets::ColliderUpdate),
                    dress_trees,
                ),
            );
    }
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
    surfaces: Vec<Handle<TreeMaterial>>,
    by_model_material: HashMap<AssetId<StandardMaterial>, Handle<TreeMaterial>>,
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
    wind: Res<TreeWind>,
) {
    let mut textures = Vec::new();
    let surfaces: Vec<_> = ALL_SURFACES
        .iter()
        .map(|surface| {
            let (material, loading) = surface_material(surface.spec(), &asset_server, wind.0);
            textures.extend(loading);
            materials.add(material)
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
            by_model_material.insert(model_material.id(), surfaces[*surface as usize].clone());
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

/// The models carry placeholder materials; swap in the surfaces' as their meshes spawn.
fn dress_trees(
    mut commands: Commands,
    materials: Option<Res<TreeMaterials>>,
    added: Query<
        (Entity, &MeshMaterial3d<StandardMaterial>),
        Added<MeshMaterial3d<StandardMaterial>>,
    >,
) {
    let Some(materials) = materials else { return };
    for (entity, material) in &added {
        if let Some(surface) = materials.by_model_material.get(&material.id()) {
            commands
                .entity(entity)
                .remove::<MeshMaterial3d<StandardMaterial>>()
                .insert(MeshMaterial3d(surface.clone()));
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
    for (surface, handle) in ALL_SURFACES.iter().zip(&materials.surfaces) {
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
    if let (true, Some(layout)) = (state.pending, layout) {
        let switched = state.world != Some(world);
        *state = TreeState {
            centre: Some(centre),
            world: Some(world),
            pending: false,
        };
        // unknown parcels stay clear until the realm confirms they are empty
        let mut desired = desired_trees(layout, position, distance.0, |parcel| {
            surface.contains(parcel) && matches!(pointers.get(parcel), Some(PointerResult::Nothing))
        });
        for (entity, tree, _) in &trees {
            if switched || desired.remove(&(tree.parcel, tree.index)).is_none() {
                commands.entity(entity).despawn();
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
