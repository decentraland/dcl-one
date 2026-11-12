//! Grass blades, flowers and reeds on the empty parcels around the player, scattered as
//! unity-explorer's GPU scatter places them and bent by the wind. Each parcel merges its instances
//! into one mesh for the blades and one for the flowers and reeds, built once when the parcel
//! appears and again only when the density or the ground they read changes. The vertex shader
//! stands each instance on the eased terrain, so heights never rebuild a mesh.

use std::f32::consts::SQRT_2;

use bevy::{
    asset::{embedded_asset, embedded_path, RenderAssetUsages},
    pbr::{
        ExtendedMaterial, MaterialExtension, MaterialExtensionKey, MaterialExtensionPipeline,
        NotShadowCaster,
    },
    platform::collections::HashSet,
    prelude::*,
    render::{
        mesh::{Indices, MeshVertexBufferLayoutRef, PrimitiveTopology},
        primitives::Aabb,
        render_resource::{
            AsBindGroup, RenderPipelineDescriptor, ShaderRef, SpecializedMeshPipelineError,
        },
        view::VisibilityRange,
    },
};
use common::structs::{LandscapeDistance, ParcelGrassConfig, PrimaryUser};
use scene_runner::{
    initialize_scene::{PointerResult, ScenePointers},
    vec3_to_parcel,
};

use crate::{
    landscape_textures::{ground_shader_defs, rebind_on_replace, LandscapeTextures, ScatterGround},
    terrain::{
        TerrainParams, TerrainSet, TerrainSurface, GROUND_MAX_Y, GROUND_MIN_Y, GROUND_Y,
        TERRAIN_TEXTURE,
    },
    trees::TreeWind,
};
use geometry::{blade_mesh, cattail_mesh, flower_mesh, Shape};
use scatter::{scatter, Placement};

pub(crate) mod geometry;
pub(crate) mod scatter;
#[cfg(test)]
mod tests;

/// Grass reaches no farther than this from the player, in metres, whatever the Landscape
/// Distance.
const MAX_REACH: f32 = 180.0;
/// At most this many parcels carry grass: the nearest, when more are in reach.
const MAX_PARCELS: usize = 256;
/// A parcel within this many metres of the player gets its meshes in the frame it appears, so
/// it is there when the loading screen lifts. Farther parcels are built `BUILDS_PER_FRAME` a
/// frame, nearest first, so a teleport spreads its work.
const BUILD_AT_ONCE_WITHIN: f32 = 48.0;
const BUILDS_PER_FRAME: usize = 16;
/// Instances shrink away by this many metres from the camera (`grass_wind.wgsl`'s `FADE`); a
/// parcel whose centre is farther than this plus its half-diagonal draws nothing.
const FADE_END: f32 = 100.0;
const PARCEL_HALF_DIAGONAL: f32 = 8.0 * SQRT_2;
/// Widest instance reach from its root at the largest scale, with the wind's lean, in metres.
const SWAY_PADDING: f32 = 1.0;

pub(crate) struct GrassPlugin;

impl Plugin for GrassPlugin {
    fn build(&self, app: &mut App) {
        embed_shaders(app);
        app.add_plugins(MaterialPlugin::<GrassMaterial>::default())
            .init_resource::<ParcelGrassConfig>()
            .add_systems(Startup, setup)
            .add_systems(PostUpdate, update_material.after(TerrainSet));
        add_parcel_systems(app);
        rebind_materials(app);
    }
}

/// The grass material binds the landscape textures: prepare it again when one is replaced.
pub(crate) fn rebind_materials(app: &mut App) {
    rebind_on_replace::<GrassMaterial>(app);
}

pub(crate) fn embed_shaders(app: &mut App) {
    embedded_asset!(app, "grass_wind.wgsl");
}

/// Spawning, dropping and building the parcels' grass, after the terrain has taken this frame's
/// pointers.
pub(crate) fn add_parcel_systems(app: &mut App) {
    app.add_systems(
        PostUpdate,
        (update_parcels, build_parcels).chain().after(TerrainSet),
    );
}

pub(crate) type GrassMaterial = ExtendedMaterial<GrassExtension>;

/// The terrain the instances stand on, the ground the blades take their colour from, and the
/// wind.
#[derive(Asset, AsBindGroup, TypePath, Clone)]
pub(crate) struct GrassExtension {
    #[texture(100, sample_type = "float", filterable = false, visibility(vertex))]
    terrain_steps: Handle<Image>,
    #[uniform(101)]
    terrain: TerrainParams,
    #[texture(102)]
    #[sampler(103)]
    ground_splat: Handle<Image>,
    #[texture(104)]
    #[sampler(105)]
    ground_colour: Handle<Image>,
    #[texture(106)]
    #[sampler(107)]
    sand_colour: Handle<Image>,
    /// x: 1 while the landscape sways, 0 while it is held still.
    #[uniform(108)]
    wind: Vec4,
}

impl GrassExtension {
    pub(crate) fn new(textures: &LandscapeTextures, terrain: TerrainParams, wind: bool) -> Self {
        Self {
            terrain_steps: TERRAIN_TEXTURE,
            terrain,
            ground_splat: textures.splat.clone(),
            ground_colour: textures.ground.clone(),
            sand_colour: textures.sand.clone(),
            wind: wind_settings(wind),
        }
    }
}

fn wind_settings(wind: bool) -> Vec4 {
    Vec4::new(if wind { 1.0 } else { 0.0 }, 0.0, 0.0, 0.0)
}

fn grass_shader() -> ShaderRef {
    ShaderRef::Path(format!("embedded://{}", embedded_path!("grass_wind.wgsl").display()).into())
}

impl MaterialExtension for GrassExtension {
    type Base = StandardMaterial;

    fn vertex_shader() -> ShaderRef {
        grass_shader()
    }

    fn prepass_vertex_shader() -> ShaderRef {
        grass_shader()
    }

    fn deferred_vertex_shader() -> ShaderRef {
        grass_shader()
    }

    fn fragment_shader() -> ShaderRef {
        grass_shader()
    }

    // the one file holds both stages, and the fragment stage imports the ground's colour
    fn specialize(
        _pipeline: &MaterialExtensionPipeline,
        descriptor: &mut RenderPipelineDescriptor,
        _layout: &MeshVertexBufferLayoutRef,
        _key: MaterialExtensionKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        descriptor.vertex.shader_defs.extend(ground_shader_defs());
        if let Some(fragment) = descriptor.fragment.as_mut() {
            fragment.shader_defs.extend(ground_shader_defs());
        }
        Ok(())
    }
}

/// The material every parcel's grass draws with, and the instance shapes.
#[derive(Resource)]
pub(crate) struct GrassAssets {
    material: Handle<GrassMaterial>,
    blade: Shape,
    flowers: [Shape; 2],
    cattail: Shape,
}

impl GrassAssets {
    pub(crate) fn new(material: Handle<GrassMaterial>) -> Self {
        Self {
            material,
            blade: blade_mesh(),
            flowers: [flower_mesh(0), flower_mesh(1)],
            cattail: cattail_mesh(),
        }
    }
}

/// The base of the grass material: matte, both faces drawn, unfogged like the ground.
pub(crate) fn grass_base() -> StandardMaterial {
    StandardMaterial {
        // 1 - unity-explorer's grass smoothness
        perceptual_roughness: 0.979,
        metallic: 0.0,
        cull_mode: None,
        fog_enabled: false,
        ..default()
    }
}

fn setup(
    mut commands: Commands,
    mut materials: ResMut<Assets<GrassMaterial>>,
    textures: Res<LandscapeTextures>,
    surface: Res<TerrainSurface>,
    wind: Res<TreeWind>,
) {
    let material = materials.add(GrassMaterial {
        base: grass_base(),
        extension: GrassExtension::new(&textures, surface.params(), wind.0),
    });
    commands.insert_resource(GrassAssets::new(material));
}

/// Terrain parameters (touching the material also rebinds the step texture after a relayout)
/// and whether the grass sways, which follows the trees' wind.
fn update_material(
    surface: Res<TerrainSurface>,
    wind: Res<TreeWind>,
    assets: Option<Res<GrassAssets>>,
    mut materials: ResMut<Assets<GrassMaterial>>,
) {
    let Some(assets) = assets else { return };
    let changes = surface.changes();
    if !changes.params && !changes.relayout && !wind.is_changed() && !assets.is_added() {
        return;
    }
    if let Some(material) = materials.get_mut(&assets.material) {
        material.extension.terrain = surface.params();
        material.extension.wind = wind_settings(wind.0);
    }
}

/// The grass of one empty parcel. Its meshes are its children.
#[derive(Component)]
#[require(Transform, Visibility)]
pub(crate) struct ParcelGrass {
    pub(crate) parcel: IVec2,
    /// Grass tufts in its blade mesh, for the debug panel.
    pub(crate) tufts: u32,
}

/// The parcel's blades are to be built.
#[derive(Component)]
struct BladesPending;

/// The parcel's flowers and reeds are to be built.
#[derive(Component)]
struct FlowersPending;

/// Which of a parcel's meshes an entity draws.
#[derive(Component, Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum GrassMesh {
    Blades,
    Flowers,
}

/// How far grass reaches for a Landscape Distance, in metres.
pub(crate) fn grass_reach(landscape_distance: f32) -> f32 {
    landscape_distance.min(MAX_REACH)
}

/// The parcels that carry grass for a player at `player` (Unity x/z): those `empty` whose square
/// comes within `reach`, at most `MAX_PARCELS`, nearest first.
pub(crate) fn grass_parcels(player: Vec2, reach: f32, empty: impl Fn(IVec2) -> bool) -> Vec<IVec2> {
    let span = (reach / 16.0).ceil() as i32 + 1;
    let centre = (player / 16.0).floor().as_ivec2();
    let mut parcels = Vec::new();
    for y in -span..=span {
        for x in -span..=span {
            let parcel = centre + IVec2::new(x, y);
            let corner = parcel.as_vec2() * 16.0;
            let nearest = player.clamp(corner, corner + 16.0);
            if nearest.distance_squared(player) > reach * reach || !empty(parcel) {
                continue;
            }
            parcels.push(((corner + 8.0).distance_squared(player), parcel));
        }
    }
    parcels.sort_by(|(a, p), (b, q)| a.total_cmp(b).then(p.y.cmp(&q.y)).then(p.x.cmp(&q.x)));
    parcels.truncate(MAX_PARCELS);
    parcels.into_iter().map(|(_, parcel)| parcel).collect()
}

/// True if the parcel lies wholly inside the coast (Unity x/z bounds), when there is one.
fn inside_coast(parcel: IVec2, coast: Option<Vec4>) -> bool {
    coast.is_none_or(|coast| {
        let (min, max) = (parcel.as_vec2() * 16.0, (parcel + 1).as_vec2() * 16.0);
        min.cmpge(coast.xy()).all() && max.cmple(coast.zw()).all()
    })
}

/// The parcel's centre in bevy space, at the ground's level.
fn parcel_centre(parcel: IVec2) -> Vec3 {
    Vec3::new(
        parcel.x as f32 * 16.0 + 8.0,
        GROUND_Y,
        -(parcel.y as f32 * 16.0) - 8.0,
    )
}

/// What the parcel layout was last worked out for.
#[derive(Default)]
struct Layout {
    player_parcel: Option<IVec2>,
    reach: f32,
    per_parcel: u32,
}

/// Spawn grass on the empty parcels in reach and drop it from the rest; mark meshes for building
/// when the density or the ground the flowers read has changed. Only confirmed empty parcels
/// carry grass: one whose pointer is unknown counts as occupied.
#[allow(clippy::too_many_arguments)]
fn update_parcels(
    mut commands: Commands,
    config: Res<ParcelGrassConfig>,
    distance: Res<LandscapeDistance>,
    surface: Res<TerrainSurface>,
    pointers: Res<ScenePointers>,
    ground: Res<ScatterGround>,
    player: Query<&GlobalTransform, With<PrimaryUser>>,
    grass: Query<(Entity, &ParcelGrass)>,
    mut last: Local<Layout>,
) {
    let Ok(player) = player.single() else {
        return;
    };
    let position = player.translation();
    if !position.is_finite() {
        return;
    }
    let player_parcel = vec3_to_parcel(position);
    let reach = grass_reach(distance.0);
    let per_parcel = config.per_parcel;
    let density_changed = last.per_parcel != per_parcel;
    let changes = surface.changes();
    let relayout = last.player_parcel != Some(player_parcel)
        || last.reach != reach
        || density_changed
        || pointers.is_changed()
        || changes.relayout
        || changes.params
        || changes.targets.is_some();
    let ground_changed = ground.is_changed();
    if !relayout && !ground_changed {
        return;
    }

    let mut wanted: HashSet<IVec2> = HashSet::new();
    if relayout {
        *last = Layout {
            player_parcel: Some(player_parcel),
            reach,
            per_parcel,
        };
        if per_parcel > 0 {
            let coast = surface.coast();
            let empty = |parcel: IVec2| {
                surface.contains(parcel)
                    && matches!(pointers.get(parcel), Some(PointerResult::Nothing))
                    && inside_coast(parcel, coast)
            };
            wanted.extend(grass_parcels(
                Vec2::new(position.x, -position.z),
                reach,
                empty,
            ));
        }
    }
    for (entity, grass) in &grass {
        if relayout && !wanted.remove(&grass.parcel) {
            commands.entity(entity).despawn();
        } else if density_changed {
            commands
                .entity(entity)
                .insert((BladesPending, FlowersPending));
        } else if ground_changed {
            commands.entity(entity).insert(FlowersPending);
        }
    }
    for parcel in wanted {
        commands.spawn((
            Name::new("Parcel grass"),
            ParcelGrass { parcel, tufts: 0 },
            Transform::from_translation(parcel_centre(parcel)),
            BladesPending,
            FlowersPending,
        ));
    }
}

/// Build the marked meshes, nearest parcel first: all those near the player, and a few farther
/// ones a frame.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
fn build_parcels(
    mut commands: Commands,
    config: Res<ParcelGrassConfig>,
    ground: Res<ScatterGround>,
    assets: Res<GrassAssets>,
    mut meshes: ResMut<Assets<Mesh>>,
    player: Query<&GlobalTransform, With<PrimaryUser>>,
    mut pending: Query<
        (
            Entity,
            &mut ParcelGrass,
            Has<BladesPending>,
            Has<FlowersPending>,
            Option<&Children>,
        ),
        Or<(With<BladesPending>, With<FlowersPending>)>,
    >,
    built: Query<&GrassMesh>,
) {
    if pending.is_empty() {
        return;
    }
    let Ok(player) = player.single() else {
        return;
    };
    let position = player.translation();
    let player = Vec2::new(position.x, -position.z);
    let mut order: Vec<(f32, Entity)> = pending
        .iter()
        .map(|(entity, grass, ..)| {
            let corner = grass.parcel.as_vec2() * 16.0;
            (player.clamp(corner, corner + 16.0).distance(player), entity)
        })
        .collect();
    order.sort_by(|(a, _), (b, _)| a.total_cmp(b));

    let mut budget = BUILDS_PER_FRAME;
    for (distance, entity) in order {
        if distance > BUILD_AT_ONCE_WITHIN {
            if budget == 0 {
                break;
            }
            budget -= 1;
        }
        let Ok((entity, mut grass, blades, flowers, children)) = pending.get_mut(entity) else {
            continue;
        };
        let set = scatter(grass.parcel, config.per_parcel, ground.0.as_deref());
        let mut rebuild = Vec::new();
        if blades {
            grass.tufts = set.blades.len() as u32;
            rebuild.push((GrassMesh::Blades, vec![(&assets.blade, &set.blades)]));
        }
        if flowers {
            rebuild.push((
                GrassMesh::Flowers,
                vec![
                    (&assets.flowers[0], &set.flowers[0]),
                    (&assets.flowers[1], &set.flowers[1]),
                    (&assets.cattail, &set.reeds),
                ],
            ));
        }
        for (kind, parts) in rebuild {
            for child in children.into_iter().flatten() {
                if matches!(built.get(*child), Ok(built) if *built == kind) {
                    commands.entity(*child).despawn();
                }
            }
            let Some((mesh, aabb)) = merged_mesh(&parts) else {
                continue;
            };
            commands.spawn((
                kind,
                Mesh3d(meshes.add(mesh)),
                MeshMaterial3d(assets.material.clone()),
                aabb,
                NotShadowCaster,
                VisibilityRange::abrupt(0.0, FADE_END + PARCEL_HALF_DIAGONAL),
                ChildOf(entity),
            ));
        }
        commands
            .entity(entity)
            .remove::<(BladesPending, FlowersPending)>();
    }
}

/// One mesh of every placed shape, relative to the parcel's centre, and its bounds; none if
/// nothing is placed. Each vertex carries its instance's root (uv 0), and its height share and
/// part (uv 1), which the shader stands, bends and colours it by.
fn merged_mesh(parts: &[(&Shape, &Vec<Placement>)]) -> Option<(Mesh, Aabb)> {
    let vertices: usize = parts
        .iter()
        .map(|(shape, placements)| shape.positions.len() * placements.len())
        .sum();
    if vertices == 0 {
        return None;
    }
    let mut positions = Vec::with_capacity(vertices);
    let mut roots = Vec::with_capacity(vertices);
    let mut shares = Vec::with_capacity(vertices);
    let mut indices = Vec::new();
    let mut top: f32 = 0.0;
    for (shape, placements) in parts {
        for placement in placements.iter() {
            // Unity x/z from the parcel corner to bevy x/z from its centre
            let root = Vec2::new(placement.offset.x - 8.0, 8.0 - placement.offset.y);
            // Unity turns clockwise seen from above, bevy anticlockwise
            let turn = Quat::from_rotation_y(-placement.yaw);
            let base = positions.len() as u16;
            for (position, share) in shape.positions.iter().zip(&shape.parts) {
                let p = turn * (*position * placement.scale) + Vec3::new(root.x, 0.0, root.y);
                top = top.max(p.y);
                positions.push(p.to_array());
                roots.push(root.to_array());
                shares.push(*share);
            }
            indices.extend(shape.indices.iter().map(|index| base + index));
        }
    }
    let mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::RENDER_WORLD,
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, roots)
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_1, shares)
    .with_inserted_indices(Indices::U16(indices));
    // the shader raises every vertex onto the terrain, which lies within the ground's bounds
    let side = 8.0 + SWAY_PADDING;
    let aabb = Aabb::from_min_max(
        Vec3::new(-side, GROUND_MIN_Y, -side),
        Vec3::new(side, GROUND_MAX_Y + top + SWAY_PADDING, side),
    );
    Some((mesh, aabb))
}
