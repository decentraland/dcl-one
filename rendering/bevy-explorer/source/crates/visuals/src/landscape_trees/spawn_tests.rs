use std::time::{Duration, Instant};

use bevy::{
    asset::LoadState,
    gltf::{Gltf, GltfPlugin},
    platform::collections::HashMap,
    prelude::*,
    render::mesh::skinning::SkinnedMeshInverseBindposes,
    scene::ScenePlugin,
};
use common::{
    sets::RealmLifecycle,
    structs::{LandscapeDistance, PrimaryUser},
    terrain::{Occupancy, TerrainTargets},
};
use scene_runner::initialize_scene::{PointerResult, ScenePointers};

use super::{
    add_spawn_systems,
    layout::TreeLayout,
    material::{Surface, SURFACES, TERRAIN_TEXTURE},
    prototypes::PROTOTYPES,
    tree_radius_parcels, LandscapeTreeAssets, Tree, ASSET_DIR,
};
use crate::{
    solids::{prototype_solids, Solid},
    terrain::TerrainSurface,
};

const GENESIS: &[u8] = include_bytes!(concat!(
    env!("DCL_ONE_ROOT"),
    "/rendering/bevy-explorer/source/crates/assets/src/assets/landscape/layout/genesis.trees"
));
const WORLDS: &[u8] = include_bytes!(concat!(
    env!("DCL_ONE_ROOT"),
    "/rendering/bevy-explorer/source/crates/assets/src/assets/landscape/layout/worlds.trees"
));

/// A headless app running `update_trees` over a settled terrain from parcel -`extent` to
/// `extent` (plus the terrain's padding), in a realm spanning `realm`: parcels outside it read as
/// empty, parcels inside it as unknown until a test resolves them. The player stands at `player`.
fn app(extent: i32, realm: (IVec2, IVec2), player: Vec3) -> App {
    let corners = [IVec2::splat(-extent), IVec2::splat(extent)];
    let mut targets = TerrainTargets::default();
    targets.reset(false);
    for corner in corners {
        targets.mark(corner, true);
    }
    targets.resolve(
        |parcel| {
            if corners.contains(&parcel) {
                Occupancy::Occupied
            } else {
                Occupancy::Empty
            }
        },
        corners.into_iter(),
    );
    let mut pointers = ScenePointers::default();
    pointers.reset_terrain(false);
    pointers.set_realm(realm.0, realm.1);

    let mut app = App::new();
    app.add_plugins((MinimalPlugins, AssetPlugin::default()))
        .init_asset::<TreeLayout>();
    let mut layouts = app.world_mut().resource_mut::<Assets<TreeLayout>>();
    let genesis = layouts.add(TreeLayout::parse(GENESIS).unwrap());
    let worlds = layouts.add(TreeLayout::parse(WORLDS).unwrap());
    app.insert_resource(LandscapeTreeAssets {
        genesis,
        worlds,
        scenes: default(),
    })
    .insert_resource(TerrainSurface::settled(&targets))
    .insert_resource(pointers)
    .insert_resource(LandscapeDistance::default());
    add_spawn_systems(&mut app);
    app.world_mut().spawn((
        PrimaryUser::default(),
        GlobalTransform::from_translation(player),
    ));
    app
}

/// The centre of a parcel, at height 0.
fn parcel_centre(parcel: IVec2) -> Vec3 {
    Vec3::new(
        parcel.x as f32 * 16.0 + 8.0,
        0.0,
        -(parcel.y as f32 * 16.0 + 8.0),
    )
}

/// A city whose realm is one parcel far from the origin, so the parcels around the origin are
/// outside it and read as empty.
fn open_land(extent: i32, player: Vec3) -> App {
    app(extent, (IVec2::splat(1000), IVec2::splat(1000)), player)
}

fn resolve(app: &mut App, parcels: impl IntoIterator<Item = IVec2>, result: PointerResult) {
    let mut pointers = app.world_mut().resource_mut::<ScenePointers>();
    for parcel in parcels {
        pointers.insert(parcel, result.clone());
    }
}

fn block(centre: IVec2, half: i32) -> impl Iterator<Item = IVec2> {
    (-half..=half).flat_map(move |y| (-half..=half).map(move |x| centre + IVec2::new(x, y)))
}

fn occupied() -> PointerResult {
    PointerResult::Exists {
        hash: "bafkreiexample".to_owned(),
        urn: None,
    }
}

struct Spawned {
    parcel: IVec2,
    index: u32,
    transform: Transform,
    solid: bool,
}

fn spawned(app: &mut App) -> Vec<Spawned> {
    let mut trees = app
        .world_mut()
        .query::<(&Tree, &Transform, Option<&Solid>)>();
    trees
        .iter(app.world())
        .map(|(tree, transform, solid)| Spawned {
            parcel: tree.parcel,
            index: tree.index,
            transform: *transform,
            solid: solid.is_some(),
        })
        .collect()
}

fn layout(bytes: &[u8]) -> TreeLayout {
    TreeLayout::parse(bytes).unwrap()
}

/// Parcel (-1,-1) holds three Genesis instances, a bush of each kind and a stone.
const PARCEL: IVec2 = IVec2::new(-1, -1);

#[test]
fn tree_assets_load_embedded() {
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        AssetPlugin::default(),
        ImagePlugin::default(),
        assets::EmbedAssetsPlugin,
        ScenePlugin,
        GltfPlugin::default(),
    ))
    .init_asset::<Mesh>()
    .init_asset::<StandardMaterial>()
    .init_asset::<SkinnedMeshInverseBindposes>()
    .init_asset::<AnimationClip>();
    app.finish();
    app.cleanup();
    let server = app.world().resource::<AssetServer>().clone();
    let models: Vec<Handle<Gltf>> = PROTOTYPES
        .iter()
        .map(|prototype| server.load(format!("{ASSET_DIR}/{}.glb", prototype.asset)))
        .collect();
    let textures: Vec<Handle<Image>> = SURFACES
        .iter()
        .flat_map(|surface| {
            [
                Some(surface.texture),
                surface.normal_map,
                surface.stone.as_ref().map(|_| TERRAIN_TEXTURE),
            ]
        })
        .flatten()
        .map(|texture| server.load(format!("{ASSET_DIR}/{texture}")))
        .collect();
    let paths = || {
        models
            .iter()
            .map(|handle| (handle.id().untyped(), handle.path()))
            .chain(
                textures
                    .iter()
                    .map(|handle| (handle.id().untyped(), handle.path())),
            )
    };
    // The files decode on the IO task pool, which every test in the process shares: wait on the
    // clock, not on a count of frames, and let the pool run between frames.
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        app.update();
        let pending: Vec<_> = paths()
            .filter(|(id, _)| {
                !matches!(
                    server.load_state(*id),
                    LoadState::Loaded | LoadState::Failed(_)
                )
            })
            .map(|(_, path)| path)
            .collect();
        if pending.is_empty() {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "still loading after 30 s: {pending:?}"
        );
        std::thread::sleep(Duration::from_millis(1));
    }
    for (id, path) in paths() {
        assert!(
            matches!(server.load_state(id), LoadState::Loaded),
            "{path:?}: {:?}",
            server.load_state(id)
        );
    }

    // each model's material slots carry the surface names, in the order the prototypes list them
    let gltfs = app.world().resource::<Assets<Gltf>>();
    for (prototype, handle) in PROTOTYPES.iter().zip(&models) {
        let gltf = gltfs.get(handle).unwrap();
        assert_eq!(gltf.scenes.len(), 1, "{}", prototype.asset);
        let names: Vec<_> = prototype
            .surfaces
            .iter()
            .map(|surface| format!("{surface:?}"))
            .collect();
        assert_eq!(gltf.materials.len(), names.len(), "{}", prototype.asset);
        for (index, name) in names.iter().enumerate() {
            assert_eq!(
                gltf.named_materials.get(name.as_str()),
                Some(&gltf.materials[index]),
                "{} material {index}",
                prototype.asset
            );
        }
    }
}

#[test]
fn spawns_layout_instances_on_empty_parcel() {
    let mut app = app(
        20,
        (IVec2::splat(-150), IVec2::splat(150)),
        parcel_centre(PARCEL),
    );
    resolve(&mut app, block(PARCEL, 1), PointerResult::Nothing);
    app.update();

    let genesis = layout(GENESIS);
    let expected = genesis.instances(PARCEL);
    assert_eq!(expected.len(), 3);
    let surface = app.world().resource::<TerrainSurface>();
    let heights: Vec<_> = expected
        .iter()
        .map(|instance| instance.transform(PARCEL, |x, z| surface.height(x, -z)))
        .collect();
    let trees: Vec<_> = spawned(&mut app)
        .into_iter()
        .filter(|tree| tree.parcel == PARCEL)
        .collect();
    assert_eq!(trees.len(), 3);
    for tree in trees {
        let expected = heights[tree.index as usize];
        assert!(
            tree.transform
                .translation
                .abs_diff_eq(expected.translation, 1e-4),
            "instance {}: {} != {}",
            tree.index,
            tree.transform.translation,
            expected.translation
        );
        assert_eq!(tree.transform.rotation, expected.rotation);
        assert_eq!(tree.transform.scale, expected.scale);
    }
}

#[test]
fn occupied_parcel_drops_its_trees() {
    let mut app = app(
        20,
        (IVec2::splat(-150), IVec2::splat(150)),
        parcel_centre(PARCEL),
    );
    resolve(&mut app, block(PARCEL, 1), PointerResult::Nothing);
    app.update();
    assert!(spawned(&mut app).iter().any(|tree| tree.parcel == PARCEL));

    resolve(&mut app, [PARCEL], occupied());
    app.update();
    assert!(spawned(&mut app).iter().all(|tree| tree.parcel != PARCEL));
}

#[test]
fn unknown_parcel_has_no_trees() {
    let mut app = app(
        20,
        (IVec2::splat(-150), IVec2::splat(150)),
        parcel_centre(PARCEL),
    );
    resolve(
        &mut app,
        block(PARCEL, 1).filter(|parcel| *parcel != PARCEL),
        PointerResult::Nothing,
    );
    app.update();
    assert!(spawned(&mut app).iter().all(|tree| tree.parcel != PARCEL));
}

#[test]
fn realm_switch_swaps_layout() {
    let mut app = open_land(6, Vec3::ZERO);
    app.update();
    let genesis = layout(GENESIS);
    let worlds = layout(WORLDS);
    let check = |trees: &[Spawned], layout: &TreeLayout| {
        assert!(!trees.is_empty());
        for tree in trees {
            let instance = layout.instances(tree.parcel)[tree.index as usize];
            let expected = instance.transform(tree.parcel, |_, _| 0.0);
            assert!(
                tree.transform
                    .translation
                    .xz()
                    .abs_diff_eq(expected.translation.xz(), 1e-4),
                "parcel {} instance {}",
                tree.parcel,
                tree.index
            );
            assert_eq!(tree.transform.scale, expected.scale);
        }
    };
    let before = spawned(&mut app);
    check(&before, &genesis);

    app.world_mut()
        .resource_mut::<ScenePointers>()
        .reset_terrain(true);
    app.update();
    let after = spawned(&mut app);
    check(&after, &worlds);
    let positions = |trees: &[Spawned]| {
        trees
            .iter()
            .map(|tree| tree.transform.translation.xz().to_array().map(f32::to_bits))
            .collect::<Vec<_>>()
    };
    assert_ne!(positions(&before), positions(&after));
}

#[test]
fn bush01_has_no_solid() {
    assert!(prototype_solids(0).is_empty());
    for prototype in 1..PROTOTYPES.len() {
        assert!(!prototype_solids(prototype).is_empty(), "{prototype}");
    }

    let mut app = open_land(6, Vec3::ZERO);
    app.update();
    let genesis = layout(GENESIS);
    let trees = spawned(&mut app);
    let prototype = |tree: &Spawned| genesis.instances(tree.parcel)[tree.index as usize].prototype;
    assert!(trees.iter().any(|tree| prototype(tree) == 0));
    assert!(trees.iter().any(|tree| prototype(tree) != 0));
    for tree in &trees {
        assert_eq!(
            tree.solid,
            prototype(tree) != 0,
            "prototype {}",
            prototype(tree)
        );
    }
}

/// Unity's collider prefabs, carried into bevy's axes (z negated): Tree01's crown sphere,
/// Rock01's stretched capsule and Bush02's trunk.
#[test]
fn colliders_match_unity_prefabs() {
    let tree01 = prototype_solids(5);
    assert_eq!(tree01.len(), 4);
    let crown = tree01
        .iter()
        .find(|capsule| capsule.radius > 5.0)
        .expect("a crown sphere");
    assert!((crown.radius - 5.3309493).abs() < 1e-5);
    assert!(crown
        .base
        .abs_diff_eq(Vec3::new(1.5728973, 9.667379, -1.2116734), 1e-4));
    assert!(crown.top.abs_diff_eq(crown.base, 1e-4));

    let rock01 = prototype_solids(2);
    assert_eq!(rock01.len(), 1);
    // radius 0.5 scaled by the larger horizontal scale, height 2 by the vertical one: the
    // segment is what the height leaves past the two caps
    let radius = 0.5000001 * 1.7924199;
    let half = 2.0 * 1.01659 * 0.5 - radius;
    assert!((rock01[0].radius - radius).abs() < 1e-5);
    assert!(rock01[0].base.abs_diff_eq(Vec3::new(0.0, -half, 0.0), 1e-4));
    assert!(rock01[0].top.abs_diff_eq(Vec3::new(0.0, half, 0.0), 1e-4));

    // Bush02's trunk: a capsule along x turned upright, 1.149 m long with 0.055 m caps
    let bush02 = prototype_solids(1);
    assert_eq!(bush02.len(), 1);
    let trunk = &bush02[0];
    assert!((trunk.base.distance(trunk.top) - (1.1490948 - 2.0 * 0.054864246)).abs() < 1e-4);
    assert!((trunk.base.y - trunk.top.y).abs() > 1.0);
}

#[test]
fn lowering_distance_despawns_far_trees() {
    let player = Vec3::ZERO;
    let mut app = open_land(40, player);
    app.world_mut().resource_mut::<LandscapeDistance>().0 = 7000.0;
    app.update();
    let far = |trees: &[Spawned]| {
        trees
            .iter()
            .filter(|tree| tree.transform.translation.xz().distance(player.xz()) > 500.0)
            .count()
    };
    let all = spawned(&mut app);
    assert!(far(&all) > 0, "the open land reaches past 500 m");

    app.world_mut().resource_mut::<LandscapeDistance>().0 = 500.0;
    app.update();
    let near = spawned(&mut app);
    assert_eq!(far(&near), 0);
    assert!(!near.is_empty());

    app.update();
    let mut added = app.world_mut().query_filtered::<(), Added<Tree>>();
    assert_eq!(added.iter(app.world()).count(), 0, "nothing respawns");
    assert_eq!(spawned(&mut app).len(), near.len());
}

#[test]
fn trees_spawn_in_the_frame_the_pointer_resolves() {
    let mut app = app(
        20,
        (IVec2::splat(-150), IVec2::splat(150)),
        parcel_centre(PARCEL),
    );
    app.init_resource::<Resolve>()
        .add_systems(PostUpdate, resolve_in_realm.in_set(RealmLifecycle));
    resolve(
        &mut app,
        block(PARCEL, 1).filter(|parcel| *parcel != PARCEL),
        PointerResult::Nothing,
    );
    app.update();
    assert!(spawned(&mut app).iter().all(|tree| tree.parcel != PARCEL));

    // the realm writes the pointer in its PostUpdate set, as the client does
    app.insert_resource(Resolve(Some(PARCEL)));
    app.update();
    assert!(app.world().resource::<Resolve>().0.is_none(), "resolved");
    assert_eq!(
        spawned(&mut app)
            .iter()
            .filter(|tree| tree.parcel == PARCEL)
            .count(),
        3
    );
}

/// A parcel the realm resolves empty in its next PostUpdate.
#[derive(Resource, Default)]
struct Resolve(Option<IVec2>);

fn resolve_in_realm(mut resolve: ResMut<Resolve>, mut pointers: ResMut<ScenePointers>) {
    if let Some(parcel) = resolve.0.take() {
        pointers.insert(parcel, PointerResult::Nothing);
    }
}

/// A realm switch while the new realm's layout is still loading clears the old layout's trees
/// at once; the new ones come when the layout arrives.
#[test]
fn realm_switch_clears_trees_while_the_layout_loads() {
    let mut app = open_land(6, Vec3::ZERO);
    let worlds_id = app.world().resource::<LandscapeTreeAssets>().worlds.id();
    let worlds = app
        .world_mut()
        .resource_mut::<Assets<TreeLayout>>()
        .remove(worlds_id)
        .unwrap();
    app.update();
    assert!(!spawned(&mut app).is_empty());

    app.world_mut()
        .resource_mut::<ScenePointers>()
        .reset_terrain(true);
    app.update();
    assert!(spawned(&mut app).is_empty(), "the city's trees are gone");

    app.world_mut()
        .resource_mut::<Assets<TreeLayout>>()
        .insert(worlds_id, worlds);
    app.update();
    let trees = spawned(&mut app);
    assert!(!trees.is_empty());
    let worlds = layout(WORLDS);
    for tree in &trees {
        let expected =
            worlds.instances(tree.parcel)[tree.index as usize].transform(tree.parcel, |_, _| 0.0);
        assert!(tree
            .transform
            .translation
            .xz()
            .abs_diff_eq(expected.translation.xz(), 1e-4));
    }
}

/// The trees' systems fit the explorer's PostUpdate ordering: the transform and collider sets,
/// which run before the realm's scene lifecycle, and the realm lifecycle itself, which the trees
/// follow. A cycle among them would panic the client at start-up.
#[test]
fn landscape_trees_schedule_is_acyclic() {
    use bevy::ecs::schedule::ScheduleLabel;
    use scene_runner::{
        initialize_scene::process_scene_lifecycle,
        update_world::transform_and_parent::configure_post_update_sets,
    };

    let mut app = App::new();
    app.add_plugins((MinimalPlugins, AssetPlugin::default()))
        .init_asset::<Shader>()
        .init_asset::<Mesh>()
        .init_asset::<Image>();
    // the explorer's own order of the transform, collider and camera sets, before the realm's
    // scene lifecycle
    configure_post_update_sets(&mut app);
    app.add_systems(PostUpdate, process_scene_lifecycle.in_set(RealmLifecycle));
    crate::terrain::TerrainPlugin.build(&mut app);
    super::LandscapeTreesPlugin.build(&mut app);
    crate::solids::SolidsPlugin.build(&mut app);
    let label = PostUpdate.intern();
    app.world_mut()
        .schedule_scope(label, |world, schedule| schedule.initialize(world))
        .unwrap_or_else(|error| panic!("PostUpdate does not build: {error}"));
}

/// A spawned instance's meshes draw with the surfaces' materials, the swaying ones inside bounds
/// padded for the wind: a tree's leaves and trunk, a stone's rock with its terrain colour and moss.
#[test]
fn spawned_meshes_wear_the_surface_materials() {
    use bevy::render::{mesh::MeshAabb, primitives::Aabb};

    use super::{
        dress_trees,
        material::{StoneMaterial, SurfaceMaterial, TreeMaterial, ALL_SURFACES},
        setup, wind_padding, TreeMaterials, TreeWind,
    };

    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        AssetPlugin::default(),
        ImagePlugin::default(),
        assets::EmbedAssetsPlugin,
        ScenePlugin,
        GltfPlugin::default(),
    ))
    .add_plugins(TransformPlugin)
    .register_type::<Visibility>()
    .register_type::<InheritedVisibility>()
    .register_type::<ViewVisibility>()
    .register_type::<bevy::render::view::VisibilityClass>()
    .register_type::<bevy::render::primitives::Aabb>()
    .register_type::<Mesh3d>()
    .register_type::<MeshMaterial3d<StandardMaterial>>()
    .init_asset::<Mesh>()
    .init_asset::<StandardMaterial>()
    .init_asset::<SkinnedMeshInverseBindposes>()
    .init_asset::<AnimationClip>()
    .init_asset::<TreeLayout>()
    .init_asset::<TreeMaterial>()
    .init_asset::<StoneMaterial>()
    .init_resource::<TreeWind>()
    .add_systems(Startup, setup)
    .add_systems(PostUpdate, dress_trees);
    app.finish();
    app.cleanup();
    app.update();
    let scenes = app.world().resource::<LandscapeTreeAssets>().scenes.clone();
    // Tree01 and Stone01
    let roots = [5, 2].map(|prototype| {
        app.world_mut()
            .spawn(SceneRoot(scenes[prototype].clone()))
            .id()
    });

    let meshes = |app: &mut App| {
        let mut query = app.world_mut().query::<(
            Entity,
            &Mesh3d,
            Option<&MeshMaterial3d<TreeMaterial>>,
            Option<&MeshMaterial3d<StoneMaterial>>,
            Has<MeshMaterial3d<StandardMaterial>>,
            Option<&Aabb>,
        )>();
        query
            .iter(app.world())
            .map(|(entity, mesh, tree, stone, standard, aabb)| {
                let material = match (tree, stone) {
                    (Some(tree), None) => Some(SurfaceMaterial::Tree(tree.0.clone())),
                    (None, Some(stone)) => Some(SurfaceMaterial::Stone(stone.0.clone())),
                    (None, None) => None,
                    (Some(_), Some(_)) => panic!("a mesh with two surface materials"),
                };
                (entity, mesh.0.clone(), material, standard, aabb.copied())
            })
            .collect::<Vec<_>>()
    };
    // a tree has its leaves and trunk, a stone its rock; the files load on the shared IO pool
    let deadline = Instant::now() + Duration::from_secs(30);
    while meshes(&mut app).len() < 3 {
        assert!(Instant::now() < deadline, "the scenes never spawned");
        app.update();
        std::thread::sleep(Duration::from_millis(1));
    }
    app.update();

    let surfaces = app.world().resource::<TreeMaterials>().surfaces.clone();
    let mut tree = Vec::new();
    let mut stone = Vec::new();
    for (entity, mesh, material, standard, aabb) in meshes(&mut app) {
        assert!(!standard, "the model's own material is gone");
        let material = material.expect("a surface material");
        let surface = ALL_SURFACES[surfaces
            .iter()
            .position(|handle| *handle == material)
            .expect("one of the surfaces")];
        let mut root = entity;
        while let Some(parent) = app.world().get::<ChildOf>(root) {
            root = parent.parent();
        }
        if root == roots[0] {
            assert!(matches!(material, SurfaceMaterial::Tree(_)), "{surface:?}");
            tree.push(surface);
        } else {
            assert_eq!(root, roots[1]);
            // the rock blends the terrain colour and its moss with the reference's values
            let SurfaceMaterial::Stone(handle) = &material else {
                panic!("{surface:?} draws without the stone blend");
            };
            let blend = &app
                .world()
                .resource::<Assets<StoneMaterial>>()
                .get(handle)
                .unwrap()
                .extension;
            let moss = Color::srgb(0.043137256, 0.42745098, 0.3019608).to_linear();
            assert_eq!(
                blend.moss,
                Vec4::new(moss.red, moss.green, moss.blue, 0.0),
                "{surface:?}"
            );
            assert_eq!(blend.terrain, Vec4::new(0.9, 0.6, 0.5, 0.0), "{surface:?}");
            assert_eq!(
                blend.terrain_texture.path().map(ToString::to_string),
                Some(format!("{ASSET_DIR}/{TERRAIN_TEXTURE}")),
            );
            stone.push(surface);
        }

        // the swaying surfaces' bounds reach past the mesh by the wind's padding
        let tight = app
            .world()
            .resource::<Assets<Mesh>>()
            .get(&mesh)
            .unwrap()
            .compute_aabb()
            .unwrap();
        let padding = wind_padding(surface.spec().wind.as_ref());
        if surface.spec().wind.is_some() {
            assert!(padding > 0.0, "{surface:?}: {padding}");
            let aabb = aabb.expect("padded bounds");
            assert_eq!(aabb.center, tight.center);
            assert!(
                (Vec3::from(aabb.half_extents) - Vec3::from(tight.half_extents))
                    .abs_diff_eq(Vec3::splat(padding), 1e-4),
                "{surface:?}"
            );
        } else {
            assert_eq!(padding, 0.0);
        }
    }
    tree.sort_by_key(|surface| *surface as usize);
    assert_eq!(tree, [Surface::LeafYellow, Surface::Trunk]);
    assert_eq!(stone, [Surface::Rock01]);
}

#[test]
fn radius_covers_the_distance() {
    assert_eq!(tree_radius_parcels(500.0), 32);
    assert_eq!(tree_radius_parcels(1000.0), 63);
    assert_eq!(tree_radius_parcels(7000.0), 438);
}

/// The surfaces the prototypes name exist, and every surface is used.
#[test]
fn every_surface_is_used() {
    let used: HashMap<Surface, usize> = PROTOTYPES
        .iter()
        .flat_map(|prototype| prototype.surfaces.iter().copied())
        .fold(HashMap::new(), |mut used, surface| {
            *used.entry(surface).or_default() += 1;
            used
        });
    assert_eq!(used.len(), SURFACES.len());
}

/// The tree and stone shaders compile on this machine's adapter, in a renderer with no window: a
/// swaying leaf surface (cut out, both faces), the trunk (opaque, normal-mapped) and a stone (its
/// terrain colour and moss) in front of a camera with the prepasses and a shadowing light. Needs a
/// GPU, so `#[ignore]`d and skipped by CI:
///
/// ```text
/// cargo test -p visuals landscape_tree_shader_compiles -- --ignored --nocapture
/// ```
#[test]
#[ignore = "needs a GPU; run with --ignored"]
fn landscape_tree_shader_compiles() {
    use bevy::{
        asset::RenderAssetUsages,
        core_pipeline::prepass::{DepthPrepass, NormalPrepass},
        log::LogPlugin,
        render::{
            camera::RenderTarget,
            pipelined_rendering::PipelinedRenderingPlugin,
            render_resource::{
                CachedPipelineState, Extent3d, PipelineCache, TextureDimension, TextureFormat,
                TextureUsages,
            },
            RenderApp,
        },
        window::ExitCondition,
        winit::WinitPlugin,
    };

    use super::{
        embed_shaders,
        material::{stone_material, surface_material, StoneMaterial},
        TreeMaterial,
    };

    let mut app = App::new();
    app.add_plugins(
        DefaultPlugins
            .build()
            .disable::<LogPlugin>()
            .disable::<WinitPlugin>()
            // keeps the render app in this app, so the pipeline cache is readable
            .disable::<PipelinedRenderingPlugin>()
            .set(WindowPlugin {
                primary_window: None,
                exit_condition: ExitCondition::DontExit,
                close_when_requested: false,
            }),
    );
    app.add_plugins((
        assets::EmbedAssetsPlugin,
        MaterialPlugin::<TreeMaterial>::default(),
        MaterialPlugin::<StoneMaterial>::default(),
    ));
    embed_shaders(&mut app);
    app.finish();
    app.cleanup();

    let server = app.world().resource::<AssetServer>().clone();
    let mut textures = Vec::new();
    let world = app.world_mut();
    let mut materials = Vec::new();
    for surface in [Surface::LeafYellow, Surface::Trunk] {
        let (material, loading) = surface_material(surface.spec(), &server, true);
        textures.extend(loading);
        materials.push(world.resource_mut::<Assets<TreeMaterial>>().add(material));
    }
    let rock = Surface::Rock03.spec();
    let (material, loading) = stone_material(rock, rock.stone.as_ref().unwrap(), &server);
    textures.extend(loading);
    let stone = world.resource_mut::<Assets<StoneMaterial>>().add(material);
    let mut images = world.resource_mut::<Assets<Image>>();
    let mut target = Image::new_fill(
        Extent3d {
            width: 64,
            height: 64,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        &[0, 0, 0, 0],
        TextureFormat::Bgra8UnormSrgb,
        RenderAssetUsages::all(),
    );
    target.texture_descriptor.usage |= TextureUsages::RENDER_ATTACHMENT;
    let target = images.add(target);
    let cube = world.resource_mut::<Assets<Mesh>>().add(
        Mesh::from(Cuboid::new(0.5, 1.0, 0.5))
            .with_generated_tangents()
            .unwrap(),
    );
    world.spawn((
        Camera3d::default(),
        Camera {
            target: RenderTarget::Image(target.into()),
            ..default()
        },
        DepthPrepass,
        NormalPrepass,
        Transform::from_xyz(0.0, 1.0, 4.0).looking_at(Vec3::Y, Vec3::Y),
    ));
    world.spawn((
        DirectionalLight {
            shadows_enabled: true,
            ..default()
        },
        Transform::from_xyz(1.0, 3.0, 1.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
    for (x, material) in [-1.0, 1.0].into_iter().zip(materials) {
        world.spawn((
            Mesh3d(cube.clone()),
            MeshMaterial3d(material),
            Transform::from_xyz(x, 0.5, 0.0),
        ));
    }
    world.spawn((
        Mesh3d(cube.clone()),
        MeshMaterial3d(stone),
        Transform::from_xyz(0.0, 0.5, -1.0),
    ));

    // the materials are prepared once their textures are in
    let started = Instant::now();
    while textures
        .iter()
        .any(|texture| !matches!(server.load_state(texture), LoadState::Loaded))
    {
        assert!(
            started.elapsed() < Duration::from_secs(60),
            "the textures never loaded"
        );
        app.update();
        std::thread::sleep(Duration::from_millis(1));
    }
    // a few frames queue every pipeline; then they compile in the background
    for _ in 0..10 {
        app.update();
    }
    let started = Instant::now();
    let mut ok;
    let mut errors = Vec::new();
    loop {
        app.update();
        let cache = app.sub_app(RenderApp).world().resource::<PipelineCache>();
        ok = 0;
        errors.clear();
        let mut pending = 0;
        for pipeline in cache.pipelines() {
            match &pipeline.state {
                CachedPipelineState::Ok(_) => ok += 1,
                CachedPipelineState::Err(error) => errors.push(error.to_string()),
                _ => pending += 1,
            }
        }
        if pending == 0 {
            break;
        }
        assert!(
            started.elapsed() < Duration::from_secs(60),
            "{pending} pipelines still compiling"
        );
    }
    println!("{ok} pipelines compiled");
    assert!(
        errors.is_empty(),
        "pipeline errors:\n{}",
        errors.join("\n\n")
    );
    // the three surfaces' main, depth and normal prepass, and shadow passes
    assert!(ok >= 9, "only {ok} pipelines compiled");
}
