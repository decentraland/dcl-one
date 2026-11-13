use bevy::render::mesh::VertexAttributeValues;
use common::structs::{ParcelGrassSetting, PrimaryUser};

use super::{
    scatter::{hash, ScatterSet, FULL_DENSITY},
    *,
};
use crate::{landscape_textures::GroundGreen, terrain};

/// Ground everywhere: the scatter reads the ground texture's dark green, under the flower limit.
fn grassland() -> GroundGreen {
    GroundGreen::from_fn(|_| 0.04)
}

/// Sand everywhere: the sand texture's light green, over the reed limit.
fn beach() -> GroundGreen {
    GroundGreen::from_fn(|_| 0.3)
}

fn all(set: &ScatterSet) -> impl Iterator<Item = &Placement> {
    set.blades
        .iter()
        .chain(set.flowers.iter().flatten())
        .chain(&set.reeds)
}

/// The hash is the scatter's: values computed independently from its definition.
#[test]
fn hash_matches_the_scatter_definition() {
    assert_eq!(hash(IVec2::new(0, 0)), 0.0);
    assert_eq!(hash(IVec2::new(1, 0)), 0.614_661_9);
    assert_eq!(hash(IVec2::new(0, 1)), 0.157_620_19);
    assert_eq!(hash(IVec2::new(-16, 32)), 0.792_913_7);
    assert_eq!(hash(IVec2::new(2400, -2400)), 0.007_050_492);
    assert_eq!(hash(IVec2::new(-37, 5)), 0.042_791_68);
}

/// A parcel always scatters the same way, its own way, inside itself, one blade per metre cell.
#[test]
fn scatter_is_deterministic() {
    let ground = grassland();
    for parcel in [IVec2::ZERO, IVec2::new(-150, 150), IVec2::new(37, -12)] {
        let set = scatter(parcel, FULL_DENSITY, Some(&ground));
        assert_eq!(set, scatter(parcel, FULL_DENSITY, Some(&ground)));
        for placement in all(&set) {
            assert!(
                placement.offset.cmpge(Vec2::ZERO).all()
                    && placement.offset.cmple(Vec2::splat(16.0)).all(),
                "{placement:?} outside {parcel}"
            );
            assert!((1.0..=3.0).contains(&placement.scale), "{placement:?}");
        }
        let mut cells: Vec<IVec2> = set
            .blades
            .iter()
            .map(|blade| blade.offset.floor().as_ivec2())
            .collect();
        cells.sort_by_key(|cell| (cell.y, cell.x));
        cells.dedup();
        assert_eq!(cells.len(), 256, "one blade in every metre cell");
    }
    assert_ne!(
        scatter(IVec2::ZERO, FULL_DENSITY, None).blades,
        scatter(IVec2::X, FULL_DENSITY, None).blades
    );
}

/// High is unity-explorer's 256 blades a parcel and 16 flowers of each kind, or 16 reeds on
/// sand; the lower settings scale them all down, Off has nothing. Flowers and reeds grow on
/// different ground, so one parcel holds both only where the two meet.
#[test]
fn scatter_counts_per_setting() {
    let parcel = IVec2::new(-3, 7);
    for (setting, blades, sparse) in [
        (ParcelGrassSetting::High, 256, 16),
        (ParcelGrassSetting::Mid, 128, 8),
        (ParcelGrassSetting::Low, 64, 4),
        (ParcelGrassSetting::Off, 0, 0),
    ] {
        let per_parcel = setting.per_parcel;
        let grass = scatter(parcel, per_parcel, Some(&grassland()));
        assert_eq!(grass.blades.len(), blades, "{setting:?}");
        assert_eq!(grass.flowers.each_ref().map(Vec::len), [sparse; 2]);
        assert!(grass.reeds.is_empty());
        let sand = scatter(parcel, per_parcel, Some(&beach()));
        assert_eq!(sand.blades, grass.blades, "blades grow on sand too");
        assert_eq!(sand.flowers.each_ref().map(Vec::len), [0; 2]);
        assert_eq!(sand.reeds.len(), sparse, "{setting:?}");
        let unread = scatter(parcel, per_parcel, None);
        assert_eq!(unread.blades.len(), blades);
        assert!(unread.flowers.iter().all(Vec::is_empty) && unread.reeds.is_empty());
    }
    // the western half of the parcel is sand: the flowers keep to the eastern cells, all 16
    // reeds to the western ones
    let corner = parcel * 16;
    let shore = GroundGreen::from_fn(|p| {
        if (p.x - corner.x).rem_euclid(1024) < 8 {
            0.3
        } else {
            0.04
        }
    });
    let set = scatter(parcel, FULL_DENSITY, Some(&shore));
    assert_eq!(set.flowers.each_ref().map(Vec::len), [8; 2]);
    assert_eq!(set.reeds.len(), 16);
    assert!(set.reeds.iter().all(|reed| reed.offset.x < 8.0));
}

/// Reach is the Landscape Distance up to 180 m.
#[test]
fn reach_capped_at_180() {
    assert_eq!(grass_reach(7000.0), 180.0);
    assert_eq!(grass_reach(500.0), 180.0);
    assert_eq!(grass_reach(100.0), 100.0);
    // along one row of empty parcels, a parcel carries grass when its square comes within reach
    let player = Vec2::splat(8.0);
    let mut row = grass_parcels(player, grass_reach(7000.0), |parcel| parcel.y == 0);
    row.sort_by_key(|parcel| parcel.x);
    assert_eq!(
        row,
        (-11..=11).map(|x| IVec2::new(x, 0)).collect::<Vec<_>>()
    );
}

/// When more than 256 empty parcels are in reach, the nearest 256 carry grass.
#[test]
fn at_most_256_parcels() {
    let player = Vec2::new(8.0, 8.0);
    let parcels = grass_parcels(player, 180.0, |_| true);
    assert_eq!(parcels.len(), 256);
    let centre = |parcel: &IVec2| (parcel.as_vec2() * 16.0 + 8.0).distance(player);
    let farthest = parcels.iter().map(centre).fold(0.0, f32::max);
    let nearest_dropped = (-13..=13)
        .flat_map(|y| (-13..=13).map(move |x| IVec2::new(x, y)))
        .filter(|parcel| !parcels.contains(parcel))
        .map(|parcel| centre(&parcel))
        .fold(f32::MAX, f32::min);
    assert!(farthest <= nearest_dropped, "{farthest} {nearest_dropped}");
}

/// The player's parcel and the realm around it: parcels from -40 to 40, the two corners
/// occupied so the terrain covers them all.
fn app(setting: ParcelGrassSetting) -> App {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, AssetPlugin::default()))
        .init_asset::<Mesh>()
        .init_asset::<GrassMaterial>()
        .init_resource::<ScatterGround>()
        .insert_resource(*setting)
        .insert_resource(GrassAssets::new(Handle::default()));
    let mut pointers = ScenePointers::default();
    pointers.set_realm(IVec2::splat(-40), IVec2::splat(40));
    for corner in [IVec2::splat(-40), IVec2::splat(40)] {
        pointers.insert(corner, occupied());
    }
    app.insert_resource(pointers);
    terrain::add_surface_systems(&mut app);
    add_parcel_systems(&mut app);
    app.world_mut().spawn((
        PrimaryUser::default(),
        GlobalTransform::from_translation(Vec3::new(8.0, 0.0, -8.0)),
    ));
    app
}

fn occupied() -> PointerResult {
    PointerResult::Exists {
        hash: "scene".to_owned(),
        urn: None,
    }
}

fn pointers(app: &mut App) -> Mut<'_, ScenePointers> {
    app.world_mut().resource_mut::<ScenePointers>()
}

fn grass_parcels_of(app: &mut App) -> Vec<(Entity, IVec2)> {
    let mut query = app.world_mut().query::<(Entity, &ParcelGrass)>();
    let mut parcels: Vec<_> = query
        .iter(app.world())
        .map(|(entity, grass)| (entity, grass.parcel))
        .collect();
    parcels.sort_by_key(|(_, parcel)| (parcel.y, parcel.x));
    parcels
}

/// The vertex count of each mesh the parcel's grass draws, by kind.
fn meshes_of(app: &mut App, parcel: IVec2) -> Vec<(GrassMesh, usize)> {
    let mut query = app.world_mut().query::<(&GrassMesh, &Mesh3d, &ChildOf)>();
    let world = app.world();
    let meshes = world.resource::<Assets<Mesh>>();
    query
        .iter(world)
        .filter(|(_, _, parent)| {
            world
                .get::<ParcelGrass>(parent.parent())
                .is_some_and(|grass| grass.parcel == parcel)
        })
        .map(|(kind, mesh, _)| {
            let mesh = meshes.get(&mesh.0).unwrap();
            let Some(VertexAttributeValues::Float32x2(roots)) =
                mesh.attribute(Mesh::ATTRIBUTE_UV_0)
            else {
                panic!("no roots")
            };
            (*kind, roots.len())
        })
        .collect()
}

/// Grass grows only where the realm has said a parcel is empty: never on a scene, and never on a
/// parcel not yet resolved.
#[test]
fn no_grass_on_occupied() {
    let mut app = app(ParcelGrassSetting::Low);
    let (scene, unknown) = (IVec2::new(1, 0), IVec2::new(0, 1));
    for y in -2..=2 {
        for x in -2..=2 {
            let parcel = IVec2::new(x, y);
            if parcel == scene {
                pointers(&mut app).insert(parcel, occupied());
            } else if parcel != unknown {
                pointers(&mut app).insert(parcel, PointerResult::Nothing);
            }
        }
    }
    app.update();
    let parcels: Vec<IVec2> = grass_parcels_of(&mut app)
        .into_iter()
        .map(|(_, p)| p)
        .collect();
    assert_eq!(parcels.len(), 23, "{parcels:?}");
    assert!(!parcels.contains(&scene) && !parcels.contains(&unknown));
    assert!(parcels.iter().all(|p| p.abs().max_element() <= 2));
    // filtered the same way without the app
    let empty = |p: IVec2| p.abs().max_element() <= 2 && p != scene && p != unknown;
    assert!(grass_parcels(Vec2::splat(8.0), 180.0, empty)
        .iter()
        .all(|p| *p != scene && *p != unknown));

    // a parcel that becomes a scene's after its grass grew loses it, meshes and all; the rest stay
    let before = grass_parcels_of(&mut app);
    let taken = IVec2::new(-1, 0);
    assert!(!meshes_of(&mut app, taken).is_empty());
    let mut meshes = app.world_mut().query_filtered::<Entity, With<GrassMesh>>();
    let mesh_count = meshes.iter(app.world()).count();
    pointers(&mut app).insert(taken, occupied());
    app.update();
    let after = grass_parcels_of(&mut app);
    let expected: Vec<_> = before.into_iter().filter(|(_, p)| *p != taken).collect();
    assert_eq!(
        after, expected,
        "only the taken parcel goes, nothing is respawned"
    );
    assert_eq!(meshes.iter(app.world()).count(), mesh_count - 1);
}

/// Lowering the Landscape Distance below 180 m drops the grass beyond it, and the next frame
/// spawns nothing again.
#[test]
fn reach_follows_distance() {
    let mut app = app(ParcelGrassSetting::Low);
    pointers(&mut app).set_all_scenes_known();
    app.update();
    let far = grass_parcels_of(&mut app);
    assert_eq!(far.len(), 256);

    app.insert_resource(LandscapeDistance(40.0));
    app.update();
    let near = grass_parcels_of(&mut app);
    assert!(!near.is_empty() && near.len() < far.len());
    let player = Vec2::splat(8.0);
    for (entity, parcel) in &near {
        assert!(far.contains(&(*entity, *parcel)), "kept, not spawned again");
        let corner = parcel.as_vec2() * 16.0;
        assert!(player.clamp(corner, corner + 16.0).distance(player) <= 40.0);
    }
    app.update();
    assert_eq!(grass_parcels_of(&mut app), near);
}

/// Parcels the realm resolves as empty in this frame, as its pointer requests do.
#[derive(Resource, Default)]
struct Resolving(Vec<IVec2>);

fn resolve(mut resolving: ResMut<Resolving>, mut pointers: ResMut<ScenePointers>) {
    for parcel in resolving.0.drain(..) {
        pointers.insert(parcel, PointerResult::Nothing);
    }
}

/// The parcels near the player have their grass in the frame their pointers resolve as empty, so
/// it is there when the loading screen lifts; farther ones follow, a few a frame.
#[test]
fn grass_spawns_in_the_frame_the_pointer_resolves() {
    let mut app = app(ParcelGrassSetting::High);
    app.init_resource::<Resolving>()
        .add_systems(PostUpdate, resolve.in_set(common::sets::RealmLifecycle));
    app.update();
    assert!(grass_parcels_of(&mut app).is_empty());

    // one parcel beside the player's, and 42 beyond the distance built at once
    let near = IVec2::new(1, 0);
    let far: Vec<IVec2> = (-3..=3)
        .flat_map(|y| (4..=9).map(move |x| IVec2::new(x, y)))
        .collect();
    app.world_mut().resource_mut::<Resolving>().0 = [vec![near], far.clone()].concat();
    app.update();
    assert_eq!(grass_parcels_of(&mut app).len(), 43);
    let blade_vertices = 256 * blade_mesh().positions.len();
    assert_eq!(
        meshes_of(&mut app, near),
        [(GrassMesh::Blades, blade_vertices)]
    );
    let built = |app: &mut App| {
        far.iter()
            .filter(|parcel| !meshes_of(app, **parcel).is_empty())
            .count()
    };
    assert_eq!(built(&mut app), BUILDS_PER_FRAME);
    app.update();
    app.update();
    assert_eq!(built(&mut app), far.len());

    // once the ground they read is in, the flowers follow; the blades are not rebuilt
    let blades = |app: &mut App| {
        let mut query = app.world_mut().query::<(Entity, &GrassMesh)>();
        query
            .iter(app.world())
            .filter(|(_, kind)| **kind == GrassMesh::Blades)
            .map(|(entity, _)| entity)
            .collect::<Vec<_>>()
    };
    let before = blades(&mut app);
    app.insert_resource(ScatterGround(Some(grassland().into())));
    app.update();
    assert_eq!(blades(&mut app), before);
    let mut kinds: Vec<GrassMesh> = meshes_of(&mut app, near)
        .into_iter()
        .map(|(k, _)| k)
        .collect();
    kinds.sort_by_key(|kind| *kind as u8);
    assert_eq!(kinds, [GrassMesh::Blades, GrassMesh::Flowers]);
}

/// A density change rebuilds the meshes of the parcels already shown, and spawns none anew.
#[test]
fn a_density_change_rebuilds_meshes_in_place() {
    let mut app = app(ParcelGrassSetting::Low);
    pointers(&mut app).insert(IVec2::X, PointerResult::Nothing);
    app.update();
    let parcels = grass_parcels_of(&mut app);
    assert_eq!(meshes_of(&mut app, IVec2::X), [(GrassMesh::Blades, 64 * 6)]);
    app.insert_resource(*ParcelGrassSetting::High);
    app.update();
    assert_eq!(grass_parcels_of(&mut app), parcels);
    assert_eq!(
        meshes_of(&mut app, IVec2::X),
        [(GrassMesh::Blades, 256 * 6)]
    );
    app.insert_resource(*ParcelGrassSetting::Off);
    app.update();
    assert!(grass_parcels_of(&mut app).is_empty());
}

/// The grass material compiles in every pipeline it is drawn with: the main pass and the
/// prepasses (it casts no shadows). Needs a GPU, so `#[ignore]`d and skipped by CI:
///
/// ```text
/// cargo test -p visuals grass_wind_compiles -- --ignored --nocapture
/// ```
#[test]
#[ignore = "needs a GPU; run with --ignored"]
fn grass_wind_compiles() {
    use crate::pipeline_test;

    let mut app = pipeline_test::app();
    terrain::embed_shaders(&mut app);
    embed_shaders(&mut app);
    app.add_plugins(MaterialPlugin::<GrassMaterial>::default());
    app.finish();
    app.cleanup();

    let world = app.world_mut();
    let textures = world.resource::<LandscapeTextures>().clone();
    let steps = TerrainSurface::default().image();
    world
        .resource_mut::<Assets<Image>>()
        .insert(TERRAIN_TEXTURE.id(), steps);
    let material = world
        .resource_mut::<Assets<GrassMaterial>>()
        .add(GrassMaterial {
            base: grass_base(),
            extension: GrassExtension::new(&textures, TerrainParams::default(), true),
        });
    let assets = GrassAssets::new(material.clone());
    let set = scatter(IVec2::ZERO, FULL_DENSITY, Some(&beach()));
    let flowers = scatter(IVec2::ZERO, FULL_DENSITY, Some(&grassland())).flowers;
    for parts in [
        vec![(&assets.blade, &set.blades)],
        vec![
            (&assets.flowers[0], &flowers[0]),
            (&assets.flowers[1], &flowers[1]),
            (&assets.cattail, &set.reeds),
        ],
    ] {
        let (mesh, aabb) = merged_mesh(&parts).unwrap();
        let mesh = world.resource_mut::<Assets<Mesh>>().add(mesh);
        world.spawn((
            Mesh3d(mesh),
            MeshMaterial3d(material.clone()),
            aabb,
            NotShadowCaster,
            Transform::from_xyz(0.0, GROUND_Y, 0.0),
        ));
    }

    pipeline_test::assert_pipelines_compile(
        &mut app,
        Vec3::new(0.0, 2.0, 12.0),
        Vec3::ZERO,
        &["grass_wind.wgsl"],
    );
}
