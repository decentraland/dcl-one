use std::time::Duration;

use bevy::ecs::system::RunSystemOnce;
use common::bounds_calc::scene_regions;
use rapier3d_f64::prelude::{ColliderBuilder, Group, InteractionGroups, InteractionTestMode};
use scene_material::BoundRegion;
use scene_runner::initialize_scene::{LiveScenes, PointerResult, PortableScenes, ScenePointers};

use super::*;

/// A scene around a notch: parcels (0, 0) and (2, 0), joined by the row (0, 1), (1, 1), (2, 1).
/// Parcel (x, y) spans world x from 16x to 16x + 16 and world z from -16y - 16 to -16y, so the
/// notch at (1, 0), which is not the scene's, spans x 16 to 32 and z -16 to 0. A collider of the
/// scene over the notch is on: it lies within the rectangle around the scene.
const AROUND_A_NOTCH: [(i32, i32); 5] = [(0, 0), (2, 0), (0, 1), (1, 1), (2, 1)];

const IN_THE_NOTCH: Vec3 = Vec3::new(24.0, 0.0, -8.0);
const IN_THE_SCENE: Vec3 = Vec3::new(8.0, 0.0, -8.0);

fn scene(parcels: &[(i32, i32)]) -> SceneColliderData {
    let regions = scene_regions(parcels.iter().map(|(x, y)| IVec2::new(*x, *y)))
        .into_iter()
        .map(|region| BoundRegion::new(region.min, region.max, region.count))
        .collect::<Vec<_>>();
    SceneColliderData::for_regions(&regions, parcels.len())
}

fn collider_id(index: u16) -> ColliderId {
    ColliderId::new(
        SceneEntityId {
            id: 512 + index,
            generation: 0,
        },
        None,
        0,
    )
}

/// A solid box of `size` centred on `centre`, owned by `entity`.
fn add_box(
    colliders: &mut SceneColliderData,
    index: u16,
    entity: Entity,
    centre: Vec3,
    size: Vec3,
) {
    let id = collider_id(index);
    let layers = Group::from_bits_truncate(ColliderLayer::ClPhysics as u32);
    let collider = ColliderBuilder::cuboid(0.5, 0.5, 0.5)
        .collision_groups(InteractionGroups::new(
            layers,
            layers,
            InteractionTestMode::And,
        ))
        .build();
    colliders.set_collider(&id, collider, Some(entity));
    colliders.update_collider_transform(
        &id,
        &GlobalTransform::from(Transform::from_translation(centre).with_scale(size)),
    );
}

/// A world a tenth of a second into a frame, with the player at `position` moving at `velocity`.
fn world_with_player(position: Vec3, velocity: Vec3) -> (World, Entity) {
    let mut world = World::new();
    let mut time = Time::<()>::default();
    time.advance_by(Duration::from_millis(100));
    world.insert_resource(time);
    world.init_resource::<AvatarMovementInfo>();
    world.init_resource::<LastMovementStep>();
    world.init_resource::<EngineMovementControl>();
    world.init_resource::<CentralCollisions>();
    world.init_resource::<CollisionPush>();
    world.init_resource::<ScenePointers>();
    world.init_resource::<LiveScenes>();
    world.init_resource::<PortableScenes>();

    let player = world
        .spawn((
            Transform::from_translation(position),
            GlobalTransform::from_translation(position),
            AvatarDynamicState::default(),
            PrimaryUser::default(),
            GroundCollider::default(),
            ActivePlayerComponent {
                scene: Entity::PLACEHOLDER,
                entity: Entity::PLACEHOLDER,
                scene_last_tick: 0,
                picked_frame: None,
                scene_start_tick: 0,
                scene_is_portable: true,
                initiated_at: 1.0,
                component: AvatarMovement {
                    velocity,
                    ..default()
                },
            },
        ))
        .id();
    (world, player)
}

fn position(world: &World, player: Entity) -> Vec3 {
    world.get::<Transform>(player).unwrap().translation
}

fn assert_near(actual: Vec3, expected: Vec3) {
    assert!(
        (actual - expected).length() < 2e-3,
        "{actual} is not near {expected}"
    );
}

/// A wall of `colliders` along x through parcel (0, 0), the notch and parcel (2, 0), its near
/// face at z = -7.75.
fn add_wall_through_the_notch(colliders: &mut SceneColliderData) {
    add_box(
        colliders,
        0,
        Entity::PLACEHOLDER,
        Vec3::new(24.0, 2.0, -8.0),
        Vec3::new(48.0, 4.0, 0.5),
    );
}

// five metres towards -z in the frame, across the wall
const ACROSS_THE_WALL: Vec3 = Vec3::new(0.0, 0.0, -50.0);

#[test]
fn a_step_passes_a_scenes_collider_over_another_scenes_parcel() {
    let start = IN_THE_NOTCH.with_z(-4.0);
    let (mut world, player) = world_with_player(start, ACROSS_THE_WALL);
    let mut colliders = scene(&AROUND_A_NOTCH);
    add_wall_through_the_notch(&mut colliders);
    world.spawn(colliders);

    world.run_system_once(apply_movement).unwrap();

    assert_near(position(&world, player), start.with_z(-9.0));
}

#[test]
fn a_step_stops_at_a_scenes_collider_inside_the_scene() {
    let start = IN_THE_SCENE.with_z(-4.0);
    let (mut world, player) = world_with_player(start, ACROSS_THE_WALL);
    let mut colliders = scene(&AROUND_A_NOTCH);
    add_wall_through_the_notch(&mut colliders);
    world.spawn(colliders);

    world.run_system_once(apply_movement).unwrap();

    assert_near(position(&world, player), start.with_z(-7.45));
}

#[test]
fn a_step_stops_at_a_collider_that_belongs_to_no_scene() {
    // the terrain, the coast walls and portables: collider data without regions
    let start = IN_THE_NOTCH.with_z(-4.0);
    let (mut world, player) = world_with_player(start, ACROSS_THE_WALL);
    let mut colliders = SceneColliderData::default();
    add_wall_through_the_notch(&mut colliders);
    world.spawn(colliders);

    world.run_system_once(apply_movement).unwrap();

    assert_near(position(&world, player), start.with_z(-7.45));
}

#[test]
fn a_step_stops_at_the_collider_of_a_scene_that_shares_its_parcels_with_another() {
    // co-tenant scenes of a server that hosts several rooms: the parcels name one of them
    let start = IN_THE_SCENE.with_z(-4.0);
    let (mut world, player) = world_with_player(start, ACROSS_THE_WALL);
    let named = world.spawn(scene(&AROUND_A_NOTCH)).id();
    place_scene(&mut world, named, &AROUND_A_NOTCH);
    let mut co_tenant = scene(&AROUND_A_NOTCH);
    add_wall_through_the_notch(&mut co_tenant);
    world.spawn(co_tenant);

    world.run_system_once(apply_movement).unwrap();

    assert_near(position(&world, player), start.with_z(-7.45));
}

/// A box of `colliders` whose face stands 0.2 m from `player` towards -z: inside the capsule's
/// 0.3 m, so the push-out moves the player 0.1 m away from it.
fn add_box_against(colliders: &mut SceneColliderData, player: Vec3) {
    add_box(
        colliders,
        0,
        Entity::PLACEHOLDER,
        player + Vec3::new(0.0, 1.0, -0.7),
        Vec3::new(4.0, 2.0, 1.0),
    );
}

#[test]
fn a_scenes_collider_over_another_scenes_parcel_does_not_push_the_player() {
    let (mut world, player) = world_with_player(IN_THE_NOTCH, Vec3::ZERO);
    let mut colliders = scene(&AROUND_A_NOTCH);
    add_box_against(&mut colliders, IN_THE_NOTCH);
    world.spawn(colliders);

    world.run_system_once(resolve_collisions).unwrap();

    assert_near(position(&world, player), IN_THE_NOTCH);
}

#[test]
fn a_scenes_collider_inside_the_scene_pushes_the_player_out() {
    let (mut world, player) = world_with_player(IN_THE_SCENE, Vec3::ZERO);
    let mut colliders = scene(&AROUND_A_NOTCH);
    add_box_against(&mut colliders, IN_THE_SCENE);
    world.spawn(colliders);

    world.run_system_once(resolve_collisions).unwrap();

    assert_near(
        position(&world, player),
        IN_THE_SCENE + Vec3::new(0.0, 0.0, 0.1),
    );
}

/// A box of `colliders` around the middle of a player standing at `player`.
fn add_box_around(colliders: &mut SceneColliderData, player: Vec3) {
    add_box(
        colliders,
        0,
        Entity::PLACEHOLDER,
        player + Vec3::Y,
        Vec3::splat(4.0),
    );
}

#[test]
fn a_collider_the_player_stands_inside_over_another_scenes_parcel_is_not_recorded() {
    let (mut world, _) = world_with_player(IN_THE_NOTCH, Vec3::ZERO);
    let mut colliders = scene(&AROUND_A_NOTCH);
    add_box_around(&mut colliders, IN_THE_NOTCH);
    world.spawn(colliders);

    world.run_system_once(resolve_collisions).unwrap();

    assert!(world.resource::<CentralCollisions>().0.is_empty());
}

#[test]
fn a_collider_the_player_stands_inside_in_its_scene_is_recorded() {
    let (mut world, _) = world_with_player(IN_THE_SCENE, Vec3::ZERO);
    let mut colliders = scene(&AROUND_A_NOTCH);
    add_box_around(&mut colliders, IN_THE_SCENE);
    let scene = world.spawn(colliders).id();

    world.run_system_once(resolve_collisions).unwrap();

    let central = &world.resource::<CentralCollisions>().0;
    assert_eq!(central.keys().copied().collect::<Vec<_>>(), vec![scene]);
}

/// A slab of `colliders` under the whole rectangle around [`AROUND_A_NOTCH`], its top at 1 m.
fn add_slab(colliders: &mut SceneColliderData, entity: Entity) {
    add_box(
        colliders,
        0,
        entity,
        Vec3::new(24.0, 0.5, -16.0),
        Vec3::new(48.0, 1.0, 32.0),
    );
}

fn ground_height(world: &World, player: Entity) -> f32 {
    world
        .get::<AvatarDynamicState>(player)
        .unwrap()
        .ground_height
}

#[test]
fn the_ground_under_a_player_is_not_a_scenes_collider_over_another_scenes_parcel() {
    // three metres up: two above the slab, where the slab is the ground
    for (velocity, moved) in [(Vec3::ZERO, Vec3::ZERO), (Vec3::X, Vec3::X * 0.1)] {
        let over_the_scene = IN_THE_SCENE.with_y(3.0);
        let (mut world, player) = world_with_player(over_the_scene, velocity);
        let mut colliders = scene(&AROUND_A_NOTCH);
        add_slab(&mut colliders, Entity::PLACEHOLDER);
        world.spawn(colliders);
        world.run_system_once(apply_movement).unwrap();
        assert_near(position(&world, player), over_the_scene + moved);
        assert!((ground_height(&world, player) - 2.0).abs() < 2e-3);

        let over_the_notch = IN_THE_NOTCH.with_y(3.0);
        let (mut world, player) = world_with_player(over_the_notch, velocity);
        let mut colliders = scene(&AROUND_A_NOTCH);
        add_slab(&mut colliders, Entity::PLACEHOLDER);
        world.spawn(colliders);
        world.run_system_once(apply_movement).unwrap();
        assert_near(position(&world, player), over_the_notch + moved);
        assert_eq!(ground_height(&world, player), 3.0);
    }
}

/// Registers `scene` as the live scene on `parcels`.
fn place_scene(world: &mut World, scene: Entity, parcels: &[(i32, i32)]) {
    let mut pointers = ScenePointers::default();
    for (x, y) in parcels {
        pointers.insert(
            IVec2::new(*x, *y),
            PointerResult::Exists {
                hash: "scene".to_owned(),
                urn: None,
            },
        );
    }
    let mut live = LiveScenes::default();
    live.scenes.insert("scene".to_owned(), scene);
    world.insert_resource(pointers);
    world.insert_resource(live);
}

#[test]
fn a_collider_above_what_its_region_is_drawn_to_is_not_the_players_ground() {
    // two parcels with another scene's between them: each is drawn to 20 m, while a collider
    // is on up to the 31.7 m of a two-parcel scene
    let parcels = [(0, 0), (2, 0)];
    let platform_entity = Entity::from_raw(77);
    let add_platform = |colliders: &mut SceneColliderData, top: f32| {
        add_box(
            colliders,
            0,
            platform_entity,
            Vec3::new(8.0, top - 0.5, -8.0),
            Vec3::new(4.0, 1.0, 4.0),
        );
    };

    for (top, ground) in [(15.0, Some(platform_entity)), (25.0, None)] {
        let (mut world, player) = world_with_player(IN_THE_SCENE.with_y(top), Vec3::ZERO);
        let mut colliders = scene(&parcels);
        add_platform(&mut colliders, top);
        let scene = world.spawn(colliders).id();
        place_scene(&mut world, scene, &parcels);

        world.run_system_once(apply_movement).unwrap();

        let recorded = world.get::<GroundCollider>(player).unwrap();
        assert_eq!(recorded.0.as_ref().map(|(entity, ..)| *entity), ground);
    }
}

/// Two parcels with another scene's between them: each is drawn to 20 m, and 0.05 m past it,
/// while a collider is on up to the 31.7 m of a two-parcel scene.
const IN_TWO_PIECES: [(i32, i32); 2] = [(0, 0), (2, 0)];

/// Where a player stands on a collider that is cut at the height [`IN_TWO_PIECES`] is drawn to:
/// the feet a millimetre above it.
const ON_THE_HEIGHT_LIMIT: Vec3 = Vec3::new(8.0, 20.051, -8.0);

/// A block of `colliders` under [`ON_THE_HEIGHT_LIMIT`], y from 18 to 26: across the height its
/// region is drawn to.
fn add_block_across_the_height_limit(colliders: &mut SceneColliderData, entity: Entity) {
    add_box(
        colliders,
        0,
        entity,
        Vec3::new(8.0, 22.0, -8.0),
        Vec3::new(4.0, 8.0, 4.0),
    );
}

#[test]
fn a_player_who_falls_onto_a_collider_cut_at_its_regions_height_limit_lands_on_it() {
    // from half a metre above the limit, a metre down in the frame
    let falling = Vec3::new(0.0, -10.0, 0.0);
    let block = Entity::from_raw(77);
    let (mut world, player) = world_with_player(ON_THE_HEIGHT_LIMIT.with_y(20.5), falling);
    let mut colliders = scene(&IN_TWO_PIECES);
    add_block_across_the_height_limit(&mut colliders, block);
    let scene = world.spawn(colliders).id();
    place_scene(&mut world, scene, &IN_TWO_PIECES);

    // the frame the player lands in, and the next
    for _ in 0..2 {
        world.run_system_once(resolve_collisions).unwrap();
        world.run_system_once(apply_movement).unwrap();

        assert_near(position(&world, player), ON_THE_HEIGHT_LIMIT);
        let above = ground_height(&world, player);
        assert!(
            above < PLAYER_GROUND_THRESHOLD,
            "{above} m above the ground"
        );
        let recorded = world.get::<GroundCollider>(player).unwrap();
        assert_eq!(recorded.0.as_ref().map(|(entity, ..)| *entity), Some(block));
    }
}

#[test]
fn a_collider_cut_at_its_regions_height_limit_is_the_ground_of_a_player_who_stands_on_it() {
    let block = Entity::from_raw(77);
    let (mut world, player) = world_with_player(ON_THE_HEIGHT_LIMIT, Vec3::ZERO);
    let mut colliders = scene(&IN_TWO_PIECES);
    add_block_across_the_height_limit(&mut colliders, block);
    let scene = world.spawn(colliders).id();
    place_scene(&mut world, scene, &IN_TWO_PIECES);

    world.run_system_once(apply_movement).unwrap();
    let above = ground_height(&world, player);
    assert!(
        above < PLAYER_GROUND_THRESHOLD,
        "{above} m above the ground"
    );
    let recorded = world.get::<GroundCollider>(player).unwrap();
    assert_eq!(recorded.0.as_ref().map(|(entity, ..)| *entity), Some(block));
}

/// Times a frame of the player's collider queries (push-out, sweep, ground) with 225 scenes
/// loaded, each with a floor and 40 colliders. The first figure asks every collider set, as
/// collider data without regions is asked; the second asks the scenes that reach the player.
#[test]
#[ignore = "a timing, printed; run with --release -- --ignored --nocapture"]
fn time_a_frame_of_player_collider_queries() {
    const FRAMES: u32 = 2000;
    let start = IN_THE_SCENE;

    for with_regions in [false, true] {
        let (mut world, player) = world_with_player(start, Vec3::new(2.0, -1.0, 1.0));
        let mut seed = 12345u32;
        let mut random = move || {
            seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
            (seed >> 8) as f32 / (1 << 24) as f32
        };
        for x in -7..=7 {
            for y in -7..=7 {
                let mut colliders = if with_regions {
                    scene(&[(x, y)])
                } else {
                    SceneColliderData::default()
                };
                let corner = Vec3::new(x as f32 * 16.0, 0.0, -(y + 1) as f32 * 16.0);
                add_box(
                    &mut colliders,
                    0,
                    Entity::PLACEHOLDER,
                    corner + Vec3::new(8.0, -0.5, 8.0),
                    Vec3::new(15.0, 1.0, 15.0),
                );
                for index in 1..=40 {
                    let centre = Vec3::new(
                        3.0 + random() * 10.0,
                        4.0 + random() * 10.0,
                        3.0 + random() * 10.0,
                    );
                    add_box(
                        &mut colliders,
                        index,
                        Entity::PLACEHOLDER,
                        corner + centre,
                        Vec3::splat(1.0 + random() * 2.0),
                    );
                }
                world.spawn(colliders);
            }
        }

        let mut frame = Schedule::default();
        frame.set_executor_kind(bevy::ecs::schedule::ExecutorKind::SingleThreaded);
        frame.add_systems((resolve_collisions, apply_movement).chain());
        frame.run(&mut world);

        let started = std::time::Instant::now();
        for _ in 0..FRAMES {
            world.get_mut::<Transform>(player).unwrap().translation = start;
            frame.run(&mut world);
        }
        println!(
            "{} a frame: {:.1} us",
            if with_regions {
                "scenes that reach the player"
            } else {
                "every collider set"
            },
            started.elapsed().as_secs_f64() * 1e6 / f64::from(FRAMES)
        );
    }
}
