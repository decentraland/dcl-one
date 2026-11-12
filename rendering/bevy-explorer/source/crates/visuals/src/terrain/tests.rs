use bevy::render::mesh::VertexAttributeValues;
use common::terrain::Occupancy;
use scene_runner::update_world::mesh_collider::GROUND_COLLISION_MASK;

use super::*;

fn ground_at(data: &mut SceneColliderData, x: f32, z: f32) -> f32 {
    let hit = data
        .cast_ray_nearest(
            Vec3::new(x, 50.0, z),
            Vec3::NEG_Y,
            100.0,
            GROUND_COLLISION_MASK,
            false,
            false,
            None,
        )
        .expect("terrain hit");
    50.0 - hit.toi
}

/// unity-explorer's `CollideTerrainSystem.CreateIndexBuffer` splits cells along
/// (x0, z0)-(x1, z1) in Unity space. Raising the (x1, z1) corner of one cell gives min(u, v)
/// with that split and max(0, u + v - 1) with the other.
#[test]
fn heightfield_split_matches_unity() {
    let mut data = SceneColliderData::default();
    // one cell over bevy x 0..1, z -1..0 (unity z 0..1); rows along +bevy z, so row 0 is unity z 1
    data.set_ground_heightfield(
        &collider_id(),
        2,
        2,
        &[0.0, 1.0, 0.0, 0.0],
        Vec2::ONE,
        Vec3::new(0.5, 0.0, -0.5),
    );
    for (u, v) in [(0.25, 0.75), (0.75, 0.25), (0.4, 0.5), (0.9, 0.8)] {
        let height = ground_at(&mut data, u, -v);
        assert!((height - f32::min(u, v)).abs() < 1e-4, "{u},{v}: {height}");
    }
}

/// The render grids use the same split, so what you see is what you stand on.
#[test]
fn mesh_split_matches_unity() {
    for mesh in [
        ground_mesh(|_| 16, 0),
        ground_mesh(|offset| 16 >> (offset.x.abs() % 3), 2),
    ] {
        let Some(VertexAttributeValues::Float32x3(positions)) =
            mesh.attribute(Mesh::ATTRIBUTE_POSITION)
        else {
            panic!("positions");
        };
        let Some(Indices::U32(indices)) = mesh.indices() else {
            panic!("indices");
        };
        for triangle in indices.chunks_exact(3) {
            let corners: [Vec3; 3] =
                std::array::from_fn(|i| Vec3::from(positions[triangle[i] as usize]));
            // front faces point up
            let normal = (corners[1] - corners[0]).cross(corners[2] - corners[0]);
            assert!(normal.y > 0.0);
            let diagonals = (0..3)
                .map(|edge| corners[(edge + 1) % 3] - corners[edge])
                .filter(|delta| delta.x != 0.0 && delta.z != 0.0)
                .collect::<Vec<_>>();
            assert_eq!(diagonals.len(), 1);
            // unity z = -bevy z
            assert!(diagonals[0].x * -diagonals[0].z > 0.0);
        }
    }
}

/// A realm with one occupied parcel at the origin and everything else empty, out to `extent`.
fn surface(extent: i32) -> (TerrainTargets, TerrainSurface) {
    let mut targets = TerrainTargets::default();
    targets.reset(false);
    targets.mark(IVec2::ZERO, true);
    // occupied bounds come from a single parcel; widen them with occupied corners
    let corners = [IVec2::splat(-extent), IVec2::splat(extent)];
    let occupancy = move |parcel: IVec2| {
        if parcel == IVec2::ZERO || corners.contains(&parcel) {
            Occupancy::Occupied
        } else {
            Occupancy::Empty
        }
    };
    let change = targets.resolve(occupancy, [IVec2::ZERO, corners[0], corners[1]].into_iter());
    let mut surface = TerrainSurface::default();
    surface.apply(&targets, change, true);
    (targets, surface)
}

#[test]
fn collider_follows_the_height_function_at_grid_points() {
    let (_, surface) = surface(20);
    let centre = IVec2::new(40, -40);
    let heights = collider_heights(&surface, centre);
    assert!(heights.iter().any(|height| *height > 3.0));
    let mut data = SceneColliderData::default();
    let side = (COLLIDER_SIZE + 1) as usize;
    data.set_ground_heightfield(
        &collider_id(),
        side,
        side,
        &heights,
        Vec2::splat(COLLIDER_SIZE as f32),
        Vec3::new(centre.x as f32, 0.0, centre.y as f32),
    );
    for (dx, dz) in [(-31, -31), (-7, 3), (0, 0), (5, 12), (30, -2), (31, 31)] {
        let (x, z) = ((centre.x + dx) as f32, (centre.y + dz) as f32);
        let expected = surface.height(x, -z);
        let actual = ground_at(&mut data, x, z);
        assert!(
            (actual - expected).abs() < 1e-3,
            "{x},{z}: {actual} vs {expected}"
        );
    }
    // between grid points the player floor matches the collider's triangles
    for (dx, dz) in [
        (-20.3, 7.8),
        (0.25, -0.75),
        (0.75, -0.25),
        (12.6, -30.1),
        (29.9, 29.2),
    ] {
        let (x, z) = (centre.x as f32 + dx, centre.y as f32 + dz);
        let expected = ground_at(&mut data, x, z);
        let actual = surface.cell_height_at(Vec3::new(x, 0.0, z));
        assert!(
            (actual - expected).abs() < 1e-3,
            "{x},{z}: {actual} vs {expected}"
        );
    }
}

#[test]
fn easing_rises_one_step_per_second_and_settles_on_the_targets() {
    let (targets, mut surface) = surface(12);
    // a new realm snaps; clear it to watch a reveal ease in
    let mut revealed = TerrainSurface {
        epoch: surface.epoch,
        enabled: true,
        ..Default::default()
    };
    revealed.apply(&targets, Some(TerrainChange::Full), true);
    let parcel = IVec2::new(6, 6);
    let x = parcel.x as f32 * 16.0 + 8.0;
    let z = parcel.y as f32 * 16.0 + 8.0;
    let final_height = surface.height(x, z);
    assert!(final_height > 5.0);
    assert_eq!(revealed.height(x, z), 0.0);

    let mut previous = 0.0;
    for _ in 0..(f32::from(MAX_STEPS) / 0.1) as usize + 2 {
        revealed.ease(0.1);
        let height = revealed.height(x, z);
        assert!(height >= previous - 1e-4);
        // one step is 1.8 m; the noise weight (up to ~4.4 m) also ramps in over the first
        // 0.4 steps
        assert!(
            height - previous <= 0.1 * 1.8 + 0.25 * 4.5,
            "{previous} -> {height}"
        );
        previous = height;
    }
    assert!((previous - final_height).abs() < 1e-4);
    assert!(revealed.active.is_empty());
    // a snapped surface has nothing to ease
    surface.changes = SurfaceChanges::default();
    surface.ease(1.0);
    assert!(surface.changes.eased.is_none());
}

#[test]
fn scene_casts_hit_the_heightfield() {
    use bevy::math::DVec3;
    use dcl_component::proto_components::sdk::components::ColliderLayer;
    let mut data = SceneColliderData::default();
    let side = (COLLIDER_SIZE + 1) as usize;
    data.set_ground_heightfield(
        &collider_id(),
        side,
        side,
        &vec![3.0; side * side],
        Vec2::splat(COLLIDER_SIZE as f32),
        Vec3::new(10.0, 0.0, -10.0),
    );
    // the movement scene's ground casts use CL_PHYSICS alone
    let mask = ColliderLayer::ClPhysics as u32;
    for skip_inside in [false, true] {
        let ray = data
            .cast_ray_nearest(
                Vec3::new(12.0, 10.0, -12.0),
                Vec3::NEG_Y,
                20.0,
                mask,
                skip_inside,
                false,
                None,
            )
            .map(|hit| hit.toi);
        assert_eq!(ray, Some(7.0), "ray, skip_inside {skip_inside}");
        let avatar = data
            .cast_avatar_nearest(
                DVec3::new(12.0, 10.0, -12.0),
                DVec3::NEG_Y,
                20.0,
                mask,
                skip_inside,
                false,
                Default::default(),
                0.0,
            )
            .map(|hit| hit.toi);
        assert!(
            avatar.is_some_and(|toi| (toi - 7.0).abs() < 1e-3),
            "avatar, skip_inside {skip_inside}: {avatar:?}"
        );
        let all = data.cast_avatar_all(
            DVec3::new(12.0, 10.0, -12.0),
            DVec3::NEG_Y,
            20.0,
            mask,
            skip_inside,
            false,
            0.0,
        );
        assert_eq!(all.len(), 1, "avatar all, skip_inside {skip_inside}");
    }
}

/// The textured ground compiles in every pipeline it is drawn with: the main pass, the prepasses
/// and shadows. Needs a GPU, so `#[ignore]`d and skipped by CI:
///
/// ```text
/// cargo test -p visuals ground_shader_compiles -- --ignored --nocapture
/// ```
#[test]
#[ignore = "needs a GPU; run with --ignored"]
fn ground_shader_compiles() {
    use crate::{landscape_textures::LandscapeTextures, pipeline_test};

    let mut app = pipeline_test::app();
    embed_shaders(&mut app);
    app.add_plugins(MaterialPlugin::<TerrainFlatMaterial>::default());
    app.finish();
    app.cleanup();

    let world = app.world_mut();
    let textures = world.resource::<LandscapeTextures>().clone();
    let steps = TerrainSurface::default().image();
    world
        .resource_mut::<Assets<Image>>()
        .insert(TERRAIN_TEXTURE.id(), steps);
    let mesh = world
        .resource_mut::<Assets<Mesh>>()
        .add(ground_mesh(ground_segments, 0));
    let flat = world
        .resource_mut::<Assets<TerrainFlatMaterial>>()
        .add(TerrainFlatMaterial {
            base: StandardMaterial::default(),
            extension: TerrainExtension::new(&textures),
        });
    world.spawn((Mesh3d(mesh), MeshMaterial3d(flat), Transform::default()));

    pipeline_test::assert_pipelines_compile(
        &mut app,
        Vec3::new(0.0, 3.0, 6.0),
        Vec3::ZERO,
        &["terrain_flat.wgsl"],
    );
}

/// The lowest Landscape Distance still displaces the hills as far as the fine rings always did.
#[test]
fn hill_reach_never_below_832() {
    assert_eq!(hill_reach(500.0), 832.0);
    assert_eq!(hill_reach(832.0), 832.0);
    assert_eq!(hill_reach(1000.0), 1000.0);
    assert_eq!(hill_reach(7000.0), 7000.0);

    let mut surface = TerrainSurface::default();
    assert_eq!(surface.params().hill_reach, 1000.0);
    surface.set_landscape_distance(500.0);
    assert!(surface.changes().params);
    assert_eq!(surface.params().hill_reach, 832.0);
    surface.changes = SurfaceChanges::default();
    surface.set_landscape_distance(500.0);
    assert!(!surface.changes().params);
    surface.set_landscape_distance(7000.0);
    assert_eq!(surface.params().hill_reach, 7000.0);
    surface.set_landscape_distance(500.0);
    assert_eq!(surface.params().hill_reach, 832.0);
}

/// The ground mesh reaches past the highest setting, so hills are displaced all the way out.
#[test]
fn ground_reaches_the_highest_hill_reach() {
    assert!(ground_extent(GROUND_RINGS) >= hill_reach(7000.0) * 1.1);
}

/// A ring's origin moves only when the player crosses a whole cell of that ring, so its vertices
/// stay on the same terrain points while the player walks.
#[test]
fn ring_origins_snap_to_cell_size() {
    for cell in [32.0, 64.0, 128.0, 256.0] {
        let origin = ring_origin(Vec2::new(1000.0, -3000.0), cell);
        assert_eq!(origin % cell, Vec2::ZERO);
        for step in [0.0, 1.0, 15.9, 16.0, 31.0] {
            let player = origin + Vec2::new(step, step * 0.5);
            assert_eq!(ring_origin(player, cell), origin, "{cell} {step}");
        }
        assert_eq!(
            ring_origin(origin + Vec2::new(cell, -0.5), cell),
            origin + Vec2::new(cell, -cell)
        );
    }
    // the whole ground snaps to its first rings' cell, the player's parcel corner
    let player = Vec3::new(37.0, 0.0, -21.0);
    let corner = ring_origin(Vec2::new(player.x, -player.z), RING_CELL);
    assert_eq!(ground_origin(player), Vec3::new(corner.x, 0.0, -corner.y));
}

/// Every terrain vertex shader variant (the ground and a parcel grid, main pass, prepasses and
/// shadows) compiles. Needs a GPU:
/// `cargo test -p visuals terrain_vertex_compiles -- --ignored --nocapture`.
#[test]
#[ignore = "needs a GPU"]
fn terrain_vertex_compiles() {
    use bevy::{
        core_pipeline::prepass::{DepthPrepass, NormalPrepass},
        log::LogPlugin,
        render::{
            camera::RenderTarget,
            pipelined_rendering::PipelinedRenderingPlugin,
            render_resource::{
                CachedPipelineState, PipelineCache, PipelineCacheError, PipelineDescriptor,
                TextureUsages,
            },
        },
        window::ExitCondition,
        winit::WinitPlugin,
    };

    use crate::landscape_textures::{LandscapeTextures, LandscapeTexturesPlugin};

    let mut app = App::new();
    app.add_plugins(
        DefaultPlugins
            .build()
            .disable::<LogPlugin>()
            .disable::<WinitPlugin>()
            .disable::<PipelinedRenderingPlugin>()
            .set(WindowPlugin {
                primary_window: None,
                exit_condition: ExitCondition::DontExit,
                close_when_requested: false,
            }),
    )
    .add_plugins((assets::EmbedAssetsPlugin, LandscapeTexturesPlugin))
    .add_plugins(MaterialPlugin::<TerrainFlatMaterial>::default());
    embed_shaders(&mut app);
    app.finish();
    app.cleanup();

    let world = app.world_mut();
    let textures = world.resource::<LandscapeTextures>().clone();
    let mut target = Image::new_fill(
        Extent3d {
            width: 64,
            height: 64,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        &[0; 4],
        TextureFormat::Bgra8UnormSrgb,
        RenderAssetUsages::default(),
    );
    target.texture_descriptor.usage |= TextureUsages::RENDER_ATTACHMENT;
    let mut images = world.resource_mut::<Assets<Image>>();
    let target = images.add(target);
    images.insert(TERRAIN_TEXTURE.id(), TerrainSurface::default().image());
    world.spawn((
        Camera3d::default(),
        Camera {
            target: RenderTarget::Image(target.into()),
            ..default()
        },
        DepthPrepass,
        NormalPrepass,
        Transform::from_xyz(0.0, 50.0, 100.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
    world.spawn((
        DirectionalLight {
            shadows_enabled: true,
            ..default()
        },
        Transform::from_xyz(1.0, 2.0, 1.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
    let mut meshes = world.resource_mut::<Assets<Mesh>>();
    let ground = meshes.add(ground_mesh(ground_segments, GROUND_RINGS));
    let flat = world
        .resource_mut::<Assets<TerrainFlatMaterial>>()
        .add(TerrainFlatMaterial {
            base: StandardMaterial::default(),
            extension: TerrainExtension::new(&textures),
        });
    world.spawn((Mesh3d(ground), MeshMaterial3d(flat)));
    let ShaderRef::Path(path) = terrain_vertex_shader() else {
        unreachable!("the terrain vertex shader is embedded");
    };
    let vertex = world.resource::<AssetServer>().load::<Shader>(path).id();

    let mut seen = 0;
    for _ in 0..2000 {
        app.update();
        let cache = app.sub_app(RenderApp).world().resource::<PipelineCache>();
        let terrain: Vec<_> = cache
            .pipelines()
            .filter(|pipeline| {
                matches!(&pipeline.descriptor,
                    PipelineDescriptor::RenderPipelineDescriptor(descriptor)
                        if descriptor.vertex.shader.id() == vertex)
            })
            .collect();
        for pipeline in &terrain {
            if let CachedPipelineState::Err(error) = &pipeline.state {
                // both clear once the shader and its imports have loaded
                if !matches!(
                    error,
                    PipelineCacheError::ShaderNotLoaded(_)
                        | PipelineCacheError::ShaderImportNotYetAvailable
                ) {
                    panic!("a terrain pipeline failed to compile: {error}");
                }
            }
        }
        seen = terrain.len();
        let ready = terrain
            .iter()
            .all(|pipeline| matches!(pipeline.state, CachedPipelineState::Ok(_)));
        // the main pass, the prepass and the shadow pass
        if seen >= 3 && ready && cache.waiting_pipelines().next().is_none() {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    panic!("the terrain pipelines did not finish compiling ({seen} seen)");
}

/// The landscape's systems (terrain, grass, trees, props, coast) fit the explorer's PostUpdate
/// ordering: the transform and collider sets, which run before the realm's scene lifecycle, and
/// the realm lifecycle itself. A cycle among them would panic the client at start-up.
#[test]
fn landscape_schedule_is_acyclic() {
    use bevy::ecs::schedule::ScheduleLabel;
    use common::sets::RealmLifecycle;
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
    TerrainPlugin.build(&mut app);
    crate::grass::GrassPlugin.build(&mut app);
    crate::trees::TreesPlugin.build(&mut app);
    crate::props::PropsPlugin.build(&mut app);
    crate::coast::CoastPlugin.build(&mut app);
    let label = PostUpdate.intern();
    app.world_mut()
        .schedule_scope(label, |world, schedule| schedule.initialize(world))
        .unwrap_or_else(|error| panic!("PostUpdate does not build: {error}"));
}
