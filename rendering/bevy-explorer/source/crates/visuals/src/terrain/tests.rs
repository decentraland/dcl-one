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
        parcel_mesh(16),
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

/// Heights that resolve after the loading screen lifts (the rest of the realm's pointers arrive
/// in batches) take their value in the frame they arrive, with nothing left to rise.
#[test]
fn a_late_reveal_takes_its_heights_at_once() {
    let (targets, settled) = surface(12);
    // the same realm, its targets not applied yet
    let mut revealed = TerrainSurface {
        epoch: settled.epoch,
        enabled: true,
        ..Default::default()
    };
    revealed.apply(&targets, Some(TerrainChange::Full), true);
    let (x, z) = (6.0 * 16.0 + 8.0, 6.0 * 16.0 + 8.0);
    assert!(settled.height(x, z) > 5.0);
    assert_eq!(revealed.height(x, z), settled.height(x, z));
    assert_eq!(revealed.changes.targets, revealed.bounds);
}

/// A parcel that resolves as empty inside the current bounds raises its neighbourhood at once.
#[test]
fn a_late_parcel_takes_its_heights_at_once() {
    let mut targets = TerrainTargets::default();
    targets.reset(false);
    let corners = [IVec2::splat(-12), IVec2::splat(12)];
    let unknown = IVec2::new(6, 6);
    let occupancy = |resolved: bool| {
        move |parcel: IVec2| {
            if parcel == IVec2::ZERO || corners.contains(&parcel) {
                Occupancy::Occupied
            } else if parcel == unknown && !resolved {
                Occupancy::Unknown
            } else {
                Occupancy::Empty
            }
        }
    };
    let occupied = [IVec2::ZERO, corners[0], corners[1]];
    let change = targets.resolve(occupancy(false), occupied.into_iter());
    let mut terrain = TerrainSurface::default();
    terrain.apply(&targets, change, true);
    let (x, z) = (6.0 * 16.0 + 8.0, 6.0 * 16.0 + 8.0);
    let before = terrain.height(x, z);

    targets.mark(unknown, false);
    let change = targets.resolve(occupancy(true), occupied.into_iter());
    assert!(matches!(change, Some(TerrainChange::Rect(..))));
    terrain.changes = SurfaceChanges::default();
    terrain.apply(&targets, change, true);
    let (_, settled) = surface(12);
    assert!(terrain.height(x, z) > before + 1.0);
    assert_eq!(terrain.height(x, z), settled.height(x, z));
    assert!(terrain.changes.targets.is_some());
}

/// Ground that rises under the player is floored before collisions resolve. Pushing the player
/// out of it instead would reach the movement scene as a launch velocity.
#[test]
fn rising_ground_lifts_the_player_before_collisions_resolve() {
    #[derive(Resource, Default)]
    struct SeenByCollisions(Option<f32>);

    let (_, surface) = surface(12);
    let (x, z) = (6.0 * 16.0 + 8.0, -(6.0 * 16.0 + 8.0));
    let ground = surface.cell_height_at(Vec3::new(x, 0.0, z));
    assert!(ground > 1.0);
    let mut pointers = ScenePointers::default();
    // a one-parcel realm: the player's parcel lies outside it, so it is empty
    pointers.set_realm(IVec2::ZERO, IVec2::ZERO);

    let mut app = App::new();
    app.insert_resource(surface)
        .insert_resource(pointers)
        .init_resource::<EngineMovementControl>()
        .init_resource::<SeenByCollisions>()
        .configure_sets(
            PostUpdate,
            (
                PostUpdateSets::ColliderUpdate,
                PostUpdateSets::PlayerUpdate,
                PostUpdateSets::CameraUpdate,
            )
                .chain(),
        )
        .add_systems(
            PostUpdate,
            (|player: Single<&Transform, With<PrimaryUser>>,
              mut seen: ResMut<SeenByCollisions>| {
                seen.0 = Some(player.translation.y);
            })
            .in_set(PostUpdateSets::PlayerUpdate),
        );
    add_player_floor(&mut app);
    app.world_mut()
        .spawn((PrimaryUser::default(), Transform::from_xyz(x, 0.0, z)));
    app.update();
    assert_eq!(app.world().resource::<SeenByCollisions>().0, Some(ground));
}

/// The shortest Landscape Distance still displaces the hills across the `RING_CELL` rings.
#[test]
fn hill_reach_never_below_832() {
    assert_eq!(hill_reach(LandscapeDistance::MIN), 832.0);
    assert_eq!(hill_reach(832.0), 832.0);
    assert_eq!(hill_reach(1000.0), 1000.0);
    assert_eq!(hill_reach(LandscapeDistance::MAX), 7000.0);

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

/// The ground mesh reaches past where the hills flatten at the longest Landscape Distance, so
/// they never run to its edge.
#[test]
fn ground_reaches_the_longest_hill_reach() {
    assert!(ground_extent(GROUND_RINGS) >= hill_reach(LandscapeDistance::MAX) * 1.1);
}

/// Every terrain vertex shader variant (flat ground and grass shells, main pass, prepasses and
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
            mesh::MeshTag,
            pipelined_rendering::PipelinedRenderingPlugin,
            render_resource::{
                CachedPipelineState, PipelineCache, PipelineCacheError, PipelineDescriptor,
                TextureUsages,
            },
        },
        window::ExitCondition,
        winit::WinitPlugin,
    };

    use crate::shell_texturing::{self, ShellTexture};

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
    .add_plugins(assets::EmbedAssetsPlugin)
    .add_plugins((
        MaterialPlugin::<TerrainFlatMaterial>::default(),
        MaterialPlugin::<ShellTexture>::default(),
    ));
    embed_shaders(&mut app);
    shell_texturing::embed_shaders(&mut app);
    app.finish();
    app.cleanup();

    let world = app.world_mut();
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
    let ground = meshes.add(ground_mesh(|_| 4, GROUND_RINGS));
    let parcel = meshes.add(parcel_mesh(4));
    let flat = world
        .resource_mut::<Assets<TerrainFlatMaterial>>()
        .add(TerrainFlatMaterial {
            base: StandardMaterial::default(),
            extension: TerrainExtension::default(),
        });
    let shells = world
        .resource_mut::<Assets<ShellTexture>>()
        .add(shell_texturing::shell_material(
            &default(),
            4,
            TerrainParams::default(),
        ));
    world.spawn((Mesh3d(ground.clone()), MeshMaterial3d(flat)));
    for mesh in [ground, parcel] {
        world.spawn((
            Mesh3d(mesh),
            MeshMaterial3d(shells.clone()),
            MeshTag(1 << 16),
        ));
    }
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
        // each material's main pass, prepass and shadow pass
        if seen >= 4 && ready && cache.waiting_pipelines().next().is_none() {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    panic!("the terrain pipelines did not finish compiling ({seen} seen)");
}
