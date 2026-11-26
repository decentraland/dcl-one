use bevy::ecs::system::RunSystemOnce;

use super::{bounds_tests::regions, *};

const PHYSICS: u32 = ColliderLayer::ClPhysics as u32;
const POINTER: u32 = ColliderLayer::ClPointer as u32;
const RADIUS: f32 = 0.2;

fn id(index: u16) -> ColliderId {
    ColliderId::new(
        SceneEntityId {
            id: 512 + index,
            generation: 0,
        },
        None,
        0,
    )
}

fn add_box(
    colliders: &mut SceneColliderData,
    index: u16,
    layers: u32,
    centre: Vec3,
    size: Vec3,
) -> ColliderId {
    let id = id(index);
    let groups = Group::from_bits_truncate(layers);
    let collider = ColliderBuilder::cuboid(0.5, 0.5, 0.5)
        .collision_groups(InteractionGroups::new(
            groups,
            groups,
            InteractionTestMode::And,
        ))
        .build();
    colliders.set_collider(&id, collider, Some(Entity::from_raw(u32::from(index) + 1)));
    colliders.update_collider_transform(
        &id,
        &GlobalTransform::from(Transform::from_translation(centre).with_scale(size)),
    );
    colliders.update_bvh();
    id
}

/// A wall across -z whose near face is 2 m from the origin.
fn wall(colliders: &mut SceneColliderData, index: u16, layers: u32) -> ColliderId {
    add_box(
        colliders,
        index,
        layers,
        Vec3::new(0.0, 0.0, -2.5),
        Vec3::new(4.0, 4.0, 1.0),
    )
}

fn ray(colliders: &SceneColliderData, layers: impl Into<ColliderLayers>) -> Option<ColliderId> {
    colliders
        .cast_ray_nearest_in_scene_flushed(Vec3::ZERO, Vec3::NEG_Z, 10.0, layers, true, false, None)
        .map(|hit| hit.id)
}

#[test]
fn the_camera_layers_meet_a_collider_on_both_layers_or_the_ground() {
    for (layers, met) in [
        (PHYSICS | POINTER, true),
        (PHYSICS | POINTER | ColliderLayer::ClCustom1 as u32, true),
        (PHYSICS | GROUND_COLLISION_MASK, true),
        (PHYSICS, false),
        (POINTER, false),
        (ColliderLayer::ClCustom1 as u32, false),
    ] {
        let mut colliders = SceneColliderData::default();
        let wall = wall(&mut colliders, 0, layers);

        assert_eq!(
            ray(&colliders, ColliderLayers::CAMERA),
            met.then_some(wall.clone()),
            "ray, layers {layers}"
        );
        let sweep = colliders.cast_ball_in_scene_flushed(
            Vec3::ZERO,
            Vec3::NEG_Z,
            10.0,
            RADIUS,
            ColliderLayers::CAMERA,
        );
        assert_eq!(sweep.nearest.is_some(), met, "sweep, layers {layers}");
        assert_eq!(colliders.stops_camera(&wall), met, "layers {layers}");
    }
}

#[test]
fn a_bare_mask_still_meets_a_collider_on_any_of_its_layers() {
    let mut colliders = SceneColliderData::default();
    let wall = wall(&mut colliders, 0, PHYSICS);

    assert_eq!(ray(&colliders, PHYSICS | POINTER), Some(wall));
    assert_eq!(ray(&colliders, POINTER), None);
}

#[test]
fn a_collider_that_is_off_for_the_player_stops_no_camera_but_is_still_hit_by_a_mask() {
    let mut colliders = SceneColliderData::default();
    let wall = wall(&mut colliders, 0, PHYSICS | POINTER);
    let handle = colliders.get_collider_handle(&wall).unwrap();
    colliders.disabled.insert(handle);

    assert_eq!(ray(&colliders, ColliderLayers::CAMERA), None);
    assert!(!colliders.stops_camera(&wall));
    assert!(colliders
        .ball_overlaps_flushed(Vec3::new(0.0, 0.0, -2.0), RADIUS, ColliderLayers::CAMERA)
        .is_empty());
    assert_eq!(ray(&colliders, PHYSICS | POINTER), Some(wall));
}

#[test]
fn a_ball_sweep_reports_the_contact_on_the_obstacle() {
    let mut colliders = SceneColliderData::default();
    wall(&mut colliders, 0, PHYSICS | POINTER);

    let sweep = colliders.cast_ball_in_scene_flushed(
        Vec3::ZERO,
        Vec3::NEG_Z,
        10.0,
        RADIUS,
        ColliderLayers::CAMERA,
    );

    let hit = sweep.nearest.unwrap();
    assert!((hit.distance - 1.8).abs() < 1e-3, "{hit:?}");
    assert!(
        hit.point.distance(Vec3::new(0.0, 0.0, -2.0)) < 1e-3,
        "{hit:?}"
    );
    assert!(hit.normal.distance(Vec3::Z) < 1e-3, "{hit:?}");
    assert!(sweep.starting.is_empty());
}

#[test]
fn a_ball_that_starts_in_an_obstacle_is_told_the_way_out_and_still_hits_the_next() {
    let mut colliders = SceneColliderData::default();
    wall(&mut colliders, 0, PHYSICS | POINTER);
    add_box(
        &mut colliders,
        1,
        PHYSICS | POINTER,
        Vec3::new(0.0, 0.0, -6.5),
        Vec3::new(4.0, 4.0, 1.0),
    );

    // 0.1 m inside the near face
    let origin = Vec3::new(0.0, 0.0, -2.1);
    let sweep = colliders.cast_ball_in_scene_flushed(
        origin,
        Vec3::NEG_Z,
        10.0,
        RADIUS,
        ColliderLayers::CAMERA,
    );

    let [out] = sweep.starting[..] else {
        panic!("{:?}", sweep.starting);
    };
    let freed = out.point + out.normal * RADIUS;
    assert!(freed.distance(Vec3::new(0.0, 0.0, -1.8)) < 1e-3, "{out:?}");
    let next = sweep.nearest.unwrap();
    assert!((origin.z - next.distance - -5.8).abs() < 1e-3, "{next:?}");
}

/// Parcel (0, 0) spans world x 0 to 16 and z -16 to 0; a one-parcel scene is drawn up to 20 m.
fn one_parcel() -> SceneColliderData {
    SceneColliderData::for_regions(&regions(&[(0, 0)]), 1)
}

#[test]
fn a_ball_sweep_ends_where_the_scene_does() {
    let mut colliders = one_parcel();
    let post = add_box(
        &mut colliders,
        0,
        PHYSICS | POINTER,
        Vec3::new(8.0, 2.0, -8.0),
        Vec3::new(1.0, 4.0, 1.0),
    );
    let layers = ColliderLayers::CAMERA;

    let towards = colliders.cast_ball_in_scene_flushed(
        Vec3::new(2.0, 2.0, -8.0),
        Vec3::X,
        10.0,
        RADIUS,
        layers,
    );
    assert!((towards.nearest.unwrap().distance - 5.3).abs() < 1e-3);

    let from_outside = colliders.cast_ball_in_scene_flushed(
        Vec3::new(-4.0, 2.0, -8.0),
        Vec3::X,
        20.0,
        RADIUS,
        layers,
    );
    assert!((from_outside.nearest.unwrap().distance - 11.3).abs() < 1e-3);

    let above = colliders.cast_ball_in_scene_flushed(
        Vec3::new(2.0, 30.0, -8.0),
        Vec3::X,
        10.0,
        RADIUS,
        layers,
    );
    assert!(above.nearest.is_none());
    assert!(colliders
        .ball_overlaps_flushed(Vec3::new(8.0, 30.0, -8.0), RADIUS, layers)
        .is_empty());
    assert_eq!(
        colliders.ball_overlaps_flushed(Vec3::new(8.0, 2.0, -8.0), RADIUS, layers),
        vec![post]
    );
}

#[test]
fn a_ball_that_meets_an_obstacle_at_the_scene_edge_stops_at_the_edge() {
    let mut colliders = one_parcel();
    // the wall reaches 2 m past the scene's -x edge
    colliders.set_collider(
        &id(0),
        ColliderBuilder::cuboid(3.0, 2.0, 2.0)
            .translation(DVec3::new(1.0, 2.0, -8.0).into())
            .collision_groups(ColliderLayers::from(PHYSICS | POINTER).groups())
            .build(),
        None,
    );
    colliders.update_bvh();

    // the ball starts outside the scene, inside the part of the wall that is not drawn
    let origin = Vec3::new(-1.0, 2.0, -8.0);
    let sweep =
        colliders.cast_ball_in_scene_flushed(origin, Vec3::X, 10.0, RADIUS, ColliderLayers::CAMERA);

    assert!(sweep.starting.is_empty());
    let hit = sweep.nearest.unwrap();
    assert!(hit.normal.distance(Vec3::NEG_X) < 1e-3, "{hit:?}");
    let stop = hit.point + hit.normal * RADIUS;
    assert!(
        stop.distance(origin + Vec3::X * hit.distance) < 1e-3,
        "{hit:?}"
    );
    // the scene is drawn 0.05 m past its parcel
    assert!((stop.x - -0.25).abs() < 1e-3, "{hit:?}");
}

#[test]
fn only_the_colliders_whose_player_collisions_flip_are_reported() {
    let mut world = World::new();
    world.init_resource::<Events<PlayerCollisionToggled>>();
    let root = world.spawn(SceneColliderData::default()).id();
    let container = |index: u16| ContainerEntity {
        container: root,
        root,
        container_id: id(index).entity,
    };
    let has_collider = |index: u16| HasCollider::<CtCollider>(id(index), Default::default());
    let kept = world.spawn((container(0), has_collider(0))).id();
    let moved = world.spawn((container(1), has_collider(1))).id();
    let holder = world.spawn((container(2), DisableCollisions)).id();
    let mut colliders = world.get_mut::<SceneColliderData>(root).unwrap();
    for (index, entity) in [(0, kept), (1, moved)] {
        colliders.set_collider(&id(index), ColliderBuilder::ball(0.5).build(), Some(entity));
    }
    let toggled = |world: &mut World| {
        world
            .run_system_once(propagate_disabled::<CtCollider>)
            .unwrap();
        world
            .resource_mut::<Events<PlayerCollisionToggled>>()
            .drain()
            .map(|toggled| toggled.0)
            .collect::<Vec<_>>()
    };

    assert_eq!(toggled(&mut world), []);

    world.entity_mut(moved).insert(ChildOf(holder));
    assert_eq!(toggled(&mut world), [moved]);
    assert_eq!(toggled(&mut world), []);

    world.entity_mut(moved).remove::<ChildOf>();
    assert_eq!(toggled(&mut world), [moved]);
}
