use std::{
    f32::consts::PI,
    path::PathBuf,
    time::{Duration, Instant},
};

use bevy::{
    asset::LoadState,
    gltf::{Gltf, GltfPlugin},
    log::LogPlugin,
    prelude::*,
    render::{
        mesh::{skinning::SkinnedMeshInverseBindposes, MeshAabb},
        pipelined_rendering::PipelinedRenderingPlugin,
    },
    scene::ScenePlugin,
    window::ExitCondition,
    winit::WinitPlugin,
};
use boimp::{
    asset_loader::ImposterLoader, bake::ImposterBakeMaterialPlugin, Imposter, ImposterBakePlugin,
};

use super::{
    bake::{bake_radius, baked_prototypes, ImpostorBake, Stage, TILE_SIZE},
    layout::TreeLayout,
    lod::{
        impostor_step_scale, impostor_transform, lod_for, screen_height, ImpostorDraw,
        ImpostorsEnabled, Lod, IMPOSTOR_SCALES,
    },
    material::TreeMaterial,
    prototypes::PROTOTYPES,
    spawn_tests::{open_land, GENESIS},
    LandscapeTreeAssets, Tree, ASSET_DIR, IMPOSTOR_DIR,
};
use crate::solids::Solid;

const TREES: [usize; 3] = [5, 6, 7];

#[test]
fn lod_thresholds_match_unity() {
    for tree in TREES {
        assert_eq!(lod_for(tree, 0.1), Lod::Impostor, "{tree}");
        assert_eq!(lod_for(tree, 0.01), Lod::Culled, "{tree}");
    }
    // Tree01 and Tree02 keep their model to about 0.204 of the view, Tree03 to about 0.25
    assert_eq!(lod_for(5, 0.21), Lod::Mesh);
    assert_eq!(lod_for(5, 0.2), Lod::Impostor);
    assert_eq!(lod_for(6, 0.21), Lod::Mesh);
    assert_eq!(lod_for(6, 0.2), Lod::Impostor);
    assert_eq!(lod_for(7, 0.25), Lod::Mesh);
    assert_eq!(lod_for(7, 0.24), Lod::Impostor);
    // and their impostor to about 0.0196, 0.0203 and 0.0366
    assert_eq!(lod_for(5, 0.0197), Lod::Impostor);
    assert_eq!(lod_for(5, 0.0195), Lod::Culled);
    assert_eq!(lod_for(6, 0.0204), Lod::Impostor);
    assert_eq!(lod_for(6, 0.0202), Lod::Culled);
    assert_eq!(lod_for(7, 0.037), Lod::Impostor);
    assert_eq!(lod_for(7, 0.036), Lod::Culled);
    // the bushes' own groups
    assert_eq!(lod_for(0, 0.078), Lod::Mesh);
    assert_eq!(lod_for(0, 0.077), Lod::Impostor);
    assert_eq!(lod_for(0, 0.0013), Lod::Impostor);
    assert_eq!(lod_for(0, 0.0012), Lod::Culled);
    assert_eq!(lod_for(1, 0.087), Lod::Mesh);
    assert_eq!(lod_for(1, 0.086), Lod::Impostor);
    assert_eq!(lod_for(1, 0.0026), Lod::Impostor);
    assert_eq!(lod_for(1, 0.0025), Lod::Culled);
}

#[test]
fn rocks_have_no_impostor() {
    for (stone, prototype) in PROTOTYPES.iter().enumerate().take(5).skip(2) {
        assert!(prototype.lod.impostor.is_none());
        assert_eq!(lod_for(stone, 0.0211), Lod::Mesh, "{stone}");
        assert_eq!(lod_for(stone, 0.0209), Lod::Culled, "{stone}");
        assert_eq!(lod_for(stone, 0.001), Lod::Culled, "{stone}");
    }
}

/// The screen height is the group's size over the view's height at its distance: Tree01 at
/// scale 1 leaves its model about 92 m out under the player camera's 60 degrees.
#[test]
fn screen_height_follows_the_view() {
    let fov = PI / 3.0;
    let switch = 21.718613 * 0.5 / (0.2036845 * (fov * 0.5).tan());
    assert!((switch - 92.35).abs() < 0.05, "{switch}");
    assert_eq!(
        lod_for(5, screen_height(5, 1.0, switch - 0.01, fov)),
        Lod::Mesh
    );
    assert_eq!(
        lod_for(5, screen_height(5, 1.0, switch + 0.01, fov)),
        Lod::Impostor
    );
    // twice as big, twice as far
    assert!((screen_height(5, 2.0, 200.0, fov) - screen_height(5, 1.0, 100.0, fov)).abs() < 1e-6);
}

/// An impostor scale step is never more than half a step from the instance's scale.
#[test]
fn impostor_scale_steps_cover_the_range() {
    for prototype in baked_prototypes() {
        let (low, high) = PROTOTYPES[prototype].scale_y;
        let half_step = (high - low) / (IMPOSTOR_SCALES - 1) as f32 * 0.5;
        assert_eq!(impostor_step_scale(prototype, 0), low);
        assert_eq!(impostor_step_scale(prototype, IMPOSTOR_SCALES - 1), high);
        for byte in 0..=255u8 {
            let scale = low + (high - low) * f32::from(byte) / 255.0;
            let (step, transform) = impostor_transform(prototype, scale);
            let stepped = impostor_step_scale(prototype, step);
            assert!((stepped - scale).abs() <= half_step + 1e-6);
            // the box is the model's bounds at the step's scale
            let size = PROTOTYPES[prototype].size();
            assert!((transform.scale * scale).abs_diff_eq(size * stepped, 1e-4));
        }
    }
}

fn wait_loaded(app: &mut App, handles: &[UntypedHandle]) {
    let server = app.world().resource::<AssetServer>().clone();
    // The files decode on the IO task pool, which every test in the process shares: wait on the
    // clock, not on a count of frames.
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        app.update();
        let pending: Vec<_> = handles
            .iter()
            .filter(|handle| {
                !matches!(
                    server.load_state(handle.id()),
                    LoadState::Loaded | LoadState::Failed(_)
                )
            })
            .map(|handle| handle.path().cloned())
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
    for handle in handles {
        assert!(
            matches!(server.load_state(handle.id()), LoadState::Loaded),
            "{:?}: {:?}",
            handle.path(),
            server.load_state(handle.id())
        );
    }
}

fn impostor_app() -> App {
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        AssetPlugin::default(),
        ImagePlugin::default(),
        assets::EmbedAssetsPlugin,
    ))
    .init_asset::<Imposter>()
    .register_asset_loader(ImposterLoader);
    app.finish();
    app.cleanup();
    app
}

fn load_impostors(app: &mut App) -> Vec<(usize, Handle<Imposter>)> {
    let server = app.world().resource::<AssetServer>().clone();
    let impostors: Vec<_> = baked_prototypes()
        .map(|prototype| {
            let handle: Handle<Imposter> = server.load(format!(
                "{IMPOSTOR_DIR}/{}.boimp",
                PROTOTYPES[prototype].asset
            ));
            (prototype, handle)
        })
        .collect();
    let handles: Vec<_> = impostors
        .iter()
        .map(|(_, handle)| handle.clone().untyped())
        .collect();
    wait_loaded(app, &handles);
    impostors
}

#[test]
fn impostor_assets_load_embedded() {
    let mut app = impostor_app();
    let impostors = load_impostors(&mut app);
    // the three trees and the two bushes
    assert_eq!(
        impostors
            .iter()
            .map(|(prototype, _)| *prototype)
            .collect::<Vec<_>>(),
        [0, 1, 5, 6, 7]
    );
}

/// The impostor of a scale 1 instance (a bush's smallest, 2) stands as tall as its model: the box
/// it draws on has the model's height, and the bake framed the model's bounds.
#[test]
fn impostor_matches_mesh_size() {
    let mut app = impostor_app();
    let impostors = load_impostors(&mut app);
    let assets = app.world().resource::<Assets<Imposter>>();
    for (prototype, handle) in impostors {
        let scale = PROTOTYPES[prototype].scale_y.0.max(1.0);
        let model = PROTOTYPES[prototype].size().y * scale;
        let (_, transform) = impostor_transform(prototype, scale);
        let height = transform.scale.y * scale;
        assert!(
            (height / model - 1.0).abs() < 0.05,
            "{}: box {height} m, model {model} m",
            PROTOTYPES[prototype].name
        );
        let radius = assets.get(&handle).unwrap().data.center_and_scale.w;
        assert!(
            (radius / bake_radius(prototype) - 1.0).abs() < 0.05,
            "{}: baked radius {radius} m, bounds {} m",
            PROTOTYPES[prototype].name,
            bake_radius(prototype)
        );
    }
}

/// The models' bounds as the prototypes list them are those of the embedded models.
#[test]
fn bounds_match_the_models() {
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
    let handles: Vec<_> = models
        .iter()
        .map(|handle| handle.clone().untyped())
        .collect();
    wait_loaded(&mut app, &handles);
    let gltfs = app.world().resource::<Assets<Gltf>>();
    let meshes = app.world().resource::<Assets<bevy::gltf::GltfMesh>>();
    let mesh_assets = app.world().resource::<Assets<Mesh>>();
    for (prototype, handle) in PROTOTYPES.iter().zip(&models) {
        let gltf = gltfs.get(handle).unwrap();
        let (mut min, mut max) = (Vec3::MAX, Vec3::MIN);
        for mesh in &gltf.meshes {
            for primitive in &meshes.get(mesh).unwrap().primitives {
                let aabb = mesh_assets
                    .get(&primitive.mesh)
                    .unwrap()
                    .compute_aabb()
                    .unwrap();
                min = min.min(aabb.min().into());
                max = max.max(aabb.max().into());
            }
        }
        assert!(
            min.abs_diff_eq(prototype.bounds.0, 1e-4) && max.abs_diff_eq(prototype.bounds.1, 1e-4),
            "{}: {min} {max}",
            prototype.name
        );
    }
}

/// Tree01's first instance of the open land around the origin, its entity, and where it stands.
fn a_tree01(app: &mut App) -> ((IVec2, u32), Vec3) {
    let layout = TreeLayout::parse(GENESIS).unwrap();
    super::spawn_tests::spawned(app)
        .into_iter()
        .find(|tree| layout.instances(tree.parcel)[tree.index as usize].prototype == 5)
        .map(|tree| ((tree.parcel, tree.index), tree.transform.translation))
        .expect("a Tree01 near the origin")
}

fn look_from(app: &mut App, position: Vec3) {
    let mut camera = app
        .world_mut()
        .query_filtered::<&mut GlobalTransform, With<common::structs::PrimaryCamera>>();
    *camera.single_mut(app.world_mut()).unwrap() = GlobalTransform::from_translation(position);
    app.update();
}

#[derive(Debug, PartialEq)]
enum Drawn {
    Model,
    Impostor,
}

/// The instance's entity, whether it is solid, and what it draws.
fn drawn(app: &mut App, key: (IVec2, u32)) -> Option<(Entity, bool, Vec<Drawn>)> {
    let mut trees = app
        .world_mut()
        .query::<(Entity, &Tree, Has<Solid>, Option<&Children>)>();
    let (entity, solid, children) = trees
        .iter(app.world())
        .find(|(_, tree, _, _)| (tree.parcel, tree.index) == key)
        .map(|(entity, _, solid, children)| {
            (
                entity,
                solid,
                children
                    .map(|children| children.to_vec())
                    .unwrap_or_default(),
            )
        })?;
    let world = app.world();
    let drawn = children
        .into_iter()
        .map(|child| {
            if world.get::<SceneRoot>(child).is_some() {
                Drawn::Model
            } else {
                assert!(world.get::<MeshMaterial3d<Imposter>>(child).is_some());
                Drawn::Impostor
            }
        })
        .collect();
    Some((entity, solid, drawn))
}

/// Near, a tree draws its model; further, its impostor, on the same solid entity; far, nothing.
#[test]
fn lod_follows_the_camera() {
    let mut app = open_land(6, Vec3::ZERO);
    app.update();
    let (key, position) = a_tree01(&mut app);

    look_from(&mut app, position + Vec3::new(10.0, 2.0, 0.0));
    let (entity, solid, near) = drawn(&mut app, key).expect("drawn near");
    assert!(solid);
    assert_eq!(near, [Drawn::Model]);

    look_from(&mut app, position + Vec3::new(300.0, 2.0, 0.0));
    let (same, solid, far) = drawn(&mut app, key).expect("drawn further out");
    assert_eq!(same, entity, "the instance keeps its entity");
    assert!(solid, "and its colliders");
    assert_eq!(far, [Drawn::Impostor]);
    let mut added = app.world_mut().query_filtered::<(), Added<Tree>>();
    app.update();
    assert_eq!(added.iter(app.world()).count(), 0, "nothing respawns");

    look_from(&mut app, position + Vec3::new(5000.0, 2.0, 0.0));
    assert!(drawn(&mut app, key).is_none(), "culled");

    look_from(&mut app, position + Vec3::new(10.0, 2.0, 0.0));
    assert_eq!(drawn(&mut app, key).expect("back").2, [Drawn::Model]);
}

/// Without impostors, a tree past its model's range is culled.
#[test]
fn impostors_off_culls_past_the_mesh() {
    let mut app = open_land(6, Vec3::ZERO);
    app.update();
    let (key, position) = a_tree01(&mut app);
    app.world_mut().resource_mut::<ImpostorsEnabled>().0 = false;

    look_from(&mut app, position + Vec3::new(300.0, 2.0, 0.0));
    assert!(drawn(&mut app, key).is_none());
    look_from(&mut app, position + Vec3::new(10.0, 2.0, 0.0));
    assert_eq!(drawn(&mut app, key).expect("near").2, [Drawn::Model]);

    app.world_mut().resource_mut::<ImpostorsEnabled>().0 = true;
    look_from(&mut app, position + Vec3::new(300.0, 2.0, 0.0));
    assert_eq!(drawn(&mut app, key).expect("impostor").2, [Drawn::Impostor]);
}

/// The plugin gives each loaded impostor a material per scale step, sized to the step.
#[test]
fn impostor_materials_follow_the_scale() {
    use super::{dress_trees, scale_impostors, setup, TreeWind};

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
    .init_asset::<AnimationClip>()
    .init_asset::<TreeLayout>()
    .init_asset::<TreeMaterial>()
    .init_asset::<Imposter>()
    .register_asset_loader(ImposterLoader)
    .init_resource::<TreeWind>()
    .add_systems(Startup, setup)
    .add_systems(Update, scale_impostors)
    .add_systems(PostUpdate, dress_trees);
    app.finish();
    app.cleanup();
    app.update();
    let bases: Vec<_> = app
        .world()
        .resource::<LandscapeTreeAssets>()
        .impostors
        .iter()
        .flatten()
        .map(|handle| handle.clone().untyped())
        .collect();
    assert_eq!(bases.len(), 5);
    wait_loaded(&mut app, &bases);
    app.update();

    let assets = app.world().resource::<LandscapeTreeAssets>();
    let draw = app.world().resource::<ImpostorDraw>();
    let imposters = app.world().resource::<Assets<Imposter>>();
    for (prototype, base) in assets.impostors.iter().enumerate() {
        let materials = &draw.materials[prototype];
        let Some(base) = base else {
            assert!(materials.is_empty());
            continue;
        };
        assert_eq!(materials.len(), IMPOSTOR_SCALES);
        let radius = imposters.get(base).unwrap().data.center_and_scale.w;
        for (step, material) in materials.iter().enumerate() {
            let scaled = imposters.get(material).expect("a scaled material");
            let expected = radius * impostor_step_scale(prototype, step);
            assert!((scaled.data.center_and_scale.w - expected).abs() < 1e-4);
        }
    }
}

/// A renderer with no window: the bake's plugins and the tree material.
fn gpu_app() -> App {
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
        ImposterBakePlugin,
        MaterialPlugin::<TreeMaterial>::default(),
        ImposterBakeMaterialPlugin::<TreeMaterial>::default(),
    ));
    super::embed_shaders(&mut app);
    app
}

/// The tree material's bake shader compiles on this machine's adapter: a leaf surface and the
/// trunk baked by an impostor camera, beside a camera with the prepasses and a shadowing light
/// that also draws two embedded impostors, one of each stored format (a small palette, Bush01,
/// and a large one, Tree01). Needs a GPU, so `#[ignore]`d and skipped by CI:
///
/// ```text
/// cargo test -p visuals tree_bake_shader_compiles -- --ignored --nocapture
/// ```
#[test]
#[ignore = "needs a GPU; run with --ignored"]
fn tree_bake_shader_compiles() {
    use bevy::{
        asset::RenderAssetUsages,
        core_pipeline::prepass::{DepthPrepass, NormalPrepass},
        render::{
            camera::RenderTarget,
            render_resource::{
                CachedPipelineState, Extent3d, PipelineCache, PipelineDescriptor, TextureDimension,
                TextureFormat, TextureUsages,
            },
            RenderApp,
        },
    };
    use boimp::bake::{BakeState, ImposterBakeBundle, ImposterBakeCamera};

    use super::material::{surface_material, Surface};

    let mut app = gpu_app();
    app.finish();
    app.cleanup();

    let server = app.world().resource::<AssetServer>().clone();
    let world = app.world_mut();
    let mut textures: Vec<UntypedHandle> = Vec::new();
    let mut materials = Vec::new();
    for surface in [Surface::LeafYellow, Surface::Trunk] {
        let (material, loading) = surface_material(surface.spec(), &server, false);
        textures.extend(loading.into_iter().map(|texture| texture.untyped()));
        materials.push(world.resource_mut::<Assets<TreeMaterial>>().add(material));
    }
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
    let target = world.resource_mut::<Assets<Image>>().add(target);
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
    let unit = world
        .resource_mut::<Assets<Mesh>>()
        .add(Cuboid::from_length(1.0));
    for (x, prototype) in [(-0.5, 0), (0.5, 5)] {
        let impostor = server.load_with_settings::<Imposter, boimp::ImposterLoaderSettings>(
            format!("{IMPOSTOR_DIR}/{}.boimp", PROTOTYPES[prototype].asset),
            |settings| *settings = super::impostor_loader_settings(),
        );
        textures.push(impostor.clone().untyped());
        // as the trees draw them: boimp's shader has no shadow pass
        world.spawn((
            Mesh3d(unit.clone()),
            MeshMaterial3d(impostor),
            Transform::from_xyz(x, 1.5, 0.0),
            bevy::pbr::NotShadowCaster,
            bevy::pbr::NotShadowReceiver,
        ));
    }

    let started = Instant::now();
    while textures
        .iter()
        .any(|texture| !matches!(server.load_state(texture.id()), LoadState::Loaded))
    {
        assert!(
            started.elapsed() < Duration::from_secs(60),
            "the textures never loaded"
        );
        app.update();
        std::thread::sleep(Duration::from_millis(1));
    }
    let mut camera = ImposterBakeCamera {
        radius: 2.0,
        grid_size: 2,
        tile_size: 16,
        multisample: 1,
        grid_mode: boimp::GridMode::Hemispherical,
        ..default()
    };
    camera.set_callback(|_| {});
    let baker = app
        .world_mut()
        .spawn(ImposterBakeBundle {
            camera,
            transform: Transform::from_xyz(0.0, 0.5, 0.0),
            ..default()
        })
        .id();

    let started = Instant::now();
    let (ok, baking, errors) = loop {
        app.update();
        let cache = app.sub_app(RenderApp).world().resource::<PipelineCache>();
        let (mut ok, mut baking, mut pending) = (0, 0, 0);
        let mut errors = Vec::new();
        for pipeline in cache.pipelines() {
            let bake = match &pipeline.descriptor {
                PipelineDescriptor::RenderPipelineDescriptor(descriptor) => descriptor
                    .label
                    .as_ref()
                    .is_some_and(|label| label.contains("imposter_bake_pipeline")),
                _ => false,
            };
            match &pipeline.state {
                CachedPipelineState::Ok(_) => {
                    ok += 1;
                    baking += usize::from(bake);
                }
                CachedPipelineState::Err(error) => errors.push(error.to_string()),
                _ => pending += 1,
            }
        }
        let finished = app
            .world()
            .get::<ImposterBakeCamera>(baker)
            .is_some_and(|camera| camera.state == BakeState::Finished);
        if pending == 0 && (finished || !errors.is_empty()) {
            break (ok, baking, errors);
        }
        assert!(
            started.elapsed() < Duration::from_secs(120),
            "{pending} pipelines still compiling, bake finished: {finished}"
        );
        std::thread::sleep(Duration::from_millis(1));
    };
    println!("{ok} pipelines compiled, {baking} of them baking");
    assert!(
        errors.is_empty(),
        "pipeline errors:\n{}",
        errors.join("\n\n")
    );
    // the leaf (cut out) and the trunk (opaque) bake
    assert!(baking >= 2, "only {baking} bake pipelines compiled");
}

/// Bakes the impostors the client embeds, into `LANDSCAPE_IMPOSTORS_OUT` (the system's temporary
/// folder without it), and checks each frames its model. Needs a GPU, so `#[ignore]`d. From the
/// workspace folder (the test runs in the crate's, so the path is absolute):
///
/// ```text
/// LANDSCAPE_IMPOSTORS_OUT="$PWD/crates/assets/src/assets/landscape/impostors" \
///   cargo test -p visuals bake_landscape_impostors -- --ignored --nocapture
/// ```
#[test]
#[ignore = "needs a GPU; run with --ignored"]
fn bake_landscape_impostors() {
    use bevy::render::view::VisibilitySystems;

    use super::{
        add_texture_mips, apply_tree_wind, bake::add_bake_systems, dress_trees,
        layout::TreeLayoutLoader, setup, TreeWind,
    };

    let out_dir = std::env::var_os("LANDSCAPE_IMPOSTORS_OUT")
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::temp_dir().join("landscape_impostors"));
    std::fs::create_dir_all(&out_dir).unwrap();

    let mut app = gpu_app();
    app.init_asset::<TreeLayout>()
        .init_asset_loader::<TreeLayoutLoader>()
        .init_resource::<TreeWind>()
        .add_systems(Startup, setup)
        .add_systems(Update, (add_texture_mips, apply_tree_wind))
        .add_systems(
            PostUpdate,
            dress_trees.before(VisibilitySystems::CalculateBounds),
        );
    add_bake_systems(&mut app);
    app.finish();
    app.cleanup();
    app.insert_resource(ImpostorBake::new(out_dir.clone()));

    let started = Instant::now();
    loop {
        app.update();
        let stage = app.world().resource::<ImpostorBake>().stage();
        if stage == Stage::Finished {
            break;
        }
        assert!(
            started.elapsed() < Duration::from_secs(600),
            "the bake stalled at {stage:?}"
        );
        std::thread::sleep(Duration::from_millis(1));
    }
    assert!(app.world().resource::<TreeWind>().0, "the wind is back");

    let heights = app
        .world()
        .resource::<ImpostorBake>()
        .heights
        .lock()
        .unwrap()
        .clone();
    let mut misfits = Vec::new();
    for prototype in baked_prototypes() {
        let prototype_ = &PROTOTYPES[prototype];
        let path = out_dir.join(format!("{}.boimp", prototype_.asset));
        let bytes = std::fs::metadata(&path)
            .unwrap_or_else(|error| panic!("{}: {error}", path.display()))
            .len();
        let height = heights
            .iter()
            .find(|(baked, _)| *baked == prototype)
            .map(|(_, height)| *height)
            .expect("baked");
        let model = prototype_.size().y;
        println!(
            "{}: {bytes} bytes, radius {:.3} m, silhouette {height:.3} m of {model:.3} m",
            path.display(),
            bake_radius(prototype)
        );
        // A tree's trunk and crown reach its bounds' bottom and top, so its silhouette spans
        // them; a bush's leaf cards are cut out at their edges, so its silhouette lies inside
        // them. Either way nothing is cropped, and the frame's scale is the radius it was given.
        let texel = bake_radius(prototype) * 2.0 / TILE_SIZE as f32;
        let fits = if TREES.contains(&prototype) {
            (height / model - 1.0).abs() < 0.05
        } else {
            height <= model + texel && height > model * 0.5
        };
        if !fits {
            misfits.push(format!(
                "{}: the side views show {height} m of a {model} m model",
                prototype_.name
            ));
        }
    }
    assert!(misfits.is_empty(), "{misfits:#?}");
}

/// What the classes cost at the farthest landscape distance, from the city's east edge with every
/// parcel of the layout counted as empty land (more than the client ever has in range): the
/// first pass, which spawns what draws, a pass after each one-step move, and a frame with the
/// camera still. Timings in this build's profile; run in release for the client's figures:
///
/// ```text
/// cargo test -p visuals lod_pass_cost -- --ignored --nocapture
/// ```
#[test]
#[ignore = "a measurement; run with --ignored"]
fn lod_pass_cost() {
    use common::{
        structs::{PrimaryCamera, PLAYER_CAMERA_FOV},
        terrain::TerrainTargets,
    };

    use super::{desired_trees, lod::update_lods, lod::TreeInstances};
    use crate::terrain::TerrainSurface;

    let layout = TreeLayout::parse(GENESIS).unwrap();
    let edge = Vec3::new(150.0 * 16.0 + 8.0, 0.0, -8.0);
    let desired = desired_trees(&layout, edge, 7000.0, |_| true);
    let total = desired.len();

    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .insert_resource(TerrainSurface::settled(&TerrainTargets::default()))
        .init_resource::<TreeInstances>()
        .init_resource::<ImpostorsEnabled>()
        .init_resource::<ImpostorDraw>()
        .insert_resource(LandscapeTreeAssets {
            genesis: default(),
            worlds: default(),
            scenes: default(),
            impostors: default(),
        })
        .add_systems(Update, update_lods);
    let camera = app
        .world_mut()
        .spawn((
            PrimaryCamera::default(),
            GlobalTransform::from_translation(edge + Vec3::Y * 2.0),
            Projection::Perspective(PerspectiveProjection {
                fov: PLAYER_CAMERA_FOV,
                ..default()
            }),
        ))
        .id();
    app.update();
    app.world_mut()
        .resource_scope(|world, mut instances: Mut<TreeInstances>| {
            instances.replace(&mut world.commands(), desired);
        });
    app.world_mut().flush();

    let started = Instant::now();
    app.update();
    let first = started.elapsed();
    let counts = app.world().resource::<TreeInstances>().counts();

    let steps = 100;
    let started = Instant::now();
    for step in 1..=steps {
        *app.world_mut().get_mut::<GlobalTransform>(camera).unwrap() =
            GlobalTransform::from_translation(edge + Vec3::new(-(step as f32), 2.0, 0.0));
        app.update();
    }
    let moving = started.elapsed() / steps;
    let started = Instant::now();
    for _ in 0..steps {
        app.update();
    }
    let still = started.elapsed() / steps;

    // the player crossing into the next parcel: the instances in range again, and the change
    let started = Instant::now();
    let next = desired_trees(&layout, edge - Vec3::X * 16.0, 7000.0, |_| true);
    let scan = started.elapsed();
    let started = Instant::now();
    app.world_mut()
        .resource_scope(|world, mut instances: Mut<TreeInstances>| {
            instances.replace(&mut world.commands(), next);
        });
    app.world_mut().flush();
    let replace = started.elapsed();
    println!(
        "{total} instances in range; mesh / impostor / culled {counts:?}; first pass {first:?}, \
         a frame after a step {moving:?}, a still frame {still:?}; on a parcel change, the scan \
         {scan:?} and the update of the list {replace:?}"
    );
}
