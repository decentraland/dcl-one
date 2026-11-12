//! The satellite map laid flat under Genesis City: 8x8 tiles of the minimap's images, 40 parcels
//! each. It lies under the ground and the scenes and shows where nothing else is drawn.
//!
//! Coordinates in the layout are unity x/z metres (bevy z = -unity z).

use bevy::{
    asset::RenderAssetUsages,
    image::{ImageLoaderSettings, ImageSampler},
    pbr::NotShadowCaster,
    prelude::*,
    render::mesh::{Indices, PrimitiveTopology},
};
use common::{partition::PARCEL_SIZE, structs::CurrentRealm, terrain::TerrainTargets};
use scene_runner::initialize_scene::ScenePointers;

use crate::debug_panel::LandscapeLayers;

/// Tiles per side.
const RESOLUTION: u32 = 8;
/// Parcels per tile side.
const CHUNK_PARCELS: f32 = 40.0;
const CHUNK_SIZE: f32 = CHUNK_PARCELS * PARCEL_SIZE;
/// The layout's origin, in parcels from the city's centre.
const GENESIS_HALF_PARCEL_WIDTH: f32 = 150.0;
/// Depth of the floor under y 0, before the z-fight threshold.
const FLOOR_DEPTH: f32 = 0.2;
const Z_FIGHT_THRESHOLD: f32 = 0.005;
/// The images carry black margins; this shift, fitted by hand, lines the map up with the parcels.
const MAP_MARGINS: Vec2 = Vec2::new(-2.0 * PARCEL_SIZE, -(20.0 * PARCEL_SIZE) + 50.0 - 1.7);
/// The floor is drawn only within this distance of the origin on each axis, which cuts off the
/// images' margins at the city's edges.
const SIZE_LIMIT: f32 = 2400.0;

pub(crate) struct SatelliteFloorPlugin;

impl Plugin for SatelliteFloorPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, update_floor);
    }
}

/// The floor's root; its 64 tiles are children.
#[derive(Component)]
pub(crate) struct SatelliteFloor;

#[derive(Component)]
pub(crate) struct SatelliteTile;

/// Shown only in a city (not a World) once a realm is known, while its toggle is on.
pub(crate) fn shown(enabled: bool, realm: &CurrentRealm, terrain: &TerrainTargets) -> bool {
    enabled && !realm.about_url.is_empty() && !terrain.is_world()
}

/// Centre of tile (x, y) in unity x/z; y grows northwards.
fn chunk_centre(x: u32, y: u32) -> Vec2 {
    Vec2::new(x as f32, y as f32) * CHUNK_SIZE
        - Vec2::splat(GENESIS_HALF_PARCEL_WIDTH * PARCEL_SIZE)
        + Vec2::splat(CHUNK_SIZE * 0.5)
        + MAP_MARGINS
}

/// Places the unit quad of `chunk_mesh` for tile (x, y).
pub(crate) fn chunk_transform(x: u32, y: u32) -> Transform {
    let centre = chunk_centre(x, y);
    Transform::from_xyz(centre.x, -FLOOR_DEPTH - Z_FIGHT_THRESHOLD, -centre.y)
        .with_scale(Vec3::new(CHUNK_SIZE, 1.0, CHUNK_SIZE))
}

/// The image for tile (x, y): image rows run north to south, tiles south to north.
fn chunk_texture(x: u32, y: u32) -> String {
    format!(
        "embedded://landscape/satellite/{x},{}.jpg",
        RESOLUTION - 1 - y
    )
}

/// The part of an upward-facing unit quad that lies within `SIZE_LIMIT` once placed by
/// `chunk_transform`. The uv keeps each texel where the whole quad would have put it: u runs
/// west to east, v north to south.
fn chunk_mesh(x: u32, y: u32) -> Mesh {
    let centre = chunk_centre(x, y);
    let range = |centre: f32| {
        (
            ((-SIZE_LIMIT - centre) / CHUNK_SIZE).max(-0.5),
            ((SIZE_LIMIT - centre) / CHUNK_SIZE).min(0.5),
        )
    };
    let (x0, x1) = range(centre.x);
    // bevy z runs opposite to unity z
    let (z0, z1) = range(-centre.y);
    let corners = [[x0, z0], [x0, z1], [x1, z1], [x1, z0]];
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    )
    .with_inserted_attribute(
        Mesh::ATTRIBUTE_POSITION,
        corners.map(|[x, z]| [x, 0.0, z]).to_vec(),
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0.0, 1.0, 0.0]; 4])
    .with_inserted_attribute(
        Mesh::ATTRIBUTE_UV_0,
        corners.map(|[x, z]| [x + 0.5, z + 0.5]).to_vec(),
    )
    .with_inserted_indices(Indices::U16(vec![0, 1, 2, 0, 2, 3]))
}

/// Spawns the floor the first time it is wanted and keeps it, so a session that never enters a
/// city never decodes the images; after that only its visibility follows the realm and toggle.
#[allow(clippy::too_many_arguments)]
fn update_floor(
    mut commands: Commands,
    layers: Res<LandscapeLayers>,
    realm: Res<CurrentRealm>,
    pointers: Res<ScenePointers>,
    mut floor: Query<&mut Visibility, With<SatelliteFloor>>,
    asset_server: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let shown = shown(layers.satellite, &realm, pointers.terrain());
    if let Ok(mut visibility) = floor.single_mut() {
        visibility.set_if_neq(if shown {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        });
        return;
    }
    if !shown {
        return;
    }
    commands
        .spawn((
            Name::new("Satellite floor"),
            SatelliteFloor,
            Transform::default(),
            Visibility::Inherited,
        ))
        .with_children(|floor| {
            for x in 0..RESOLUTION {
                for y in 0..RESOLUTION {
                    let texture = asset_server.load_with_settings(
                        chunk_texture(x, y),
                        |settings: &mut ImageLoaderSettings| {
                            // drawn only, never read back: no main-world copy of the pixels
                            settings.asset_usage = RenderAssetUsages::RENDER_WORLD;
                            settings.sampler = ImageSampler::linear();
                        },
                    );
                    floor.spawn((
                        Name::new(format!("Satellite tile {x},{y}")),
                        SatelliteTile,
                        Mesh3d(meshes.add(chunk_mesh(x, y))),
                        MeshMaterial3d(materials.add(StandardMaterial {
                            base_color_texture: Some(texture),
                            unlit: true,
                            ..default()
                        })),
                        chunk_transform(x, y),
                        NotShadowCaster,
                    ));
                }
            }
        });
}

#[cfg(test)]
mod tests;
