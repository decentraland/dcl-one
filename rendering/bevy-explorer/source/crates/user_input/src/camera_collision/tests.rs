use bevy::prelude::*;
use dcl_component::{proto_components::sdk::components::ColliderLayer, SceneEntityId};
use rapier3d_f64::prelude::{ColliderBuilder, Group, InteractionGroups, InteractionTestMode};
use scene_runner::update_world::mesh_collider::{
    ColliderId, SceneColliderData, GROUND_COLLISION_MASK,
};

use super::*;

const PHYSICS: u32 = ColliderLayer::ClPhysics as u32;
const POINTER: u32 = ColliderLayer::ClPointer as u32;
const FRAME: f32 = 1.0 / 60.0;

/// The camera looks at the origin from 5 m along +z.
const LOOK_AT: Vec3 = Vec3::ZERO;
const RAW: Vec3 = Vec3::new(0.0, 0.0, 5.0);

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

/// A box of `size` centred on `centre` on the collider layers `layers`.
fn add_box(colliders: &mut SceneColliderData, index: u16, layers: u32, centre: Vec3, size: Vec3) {
    let id = collider_id(index);
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
    colliders.prepare_camera_queries();
}

/// A wall across the line of sight, its near face 2.9 m from the target.
fn wall(colliders: &mut SceneColliderData, layers: u32) {
    add_box(
        colliders,
        0,
        layers,
        Vec3::new(0.0, 0.0, 3.0),
        Vec3::new(4.0, 4.0, 0.2),
    );
}

fn step(colliders: &SceneColliderData, state: &mut CameraCollisionState, now: f32) -> Resolved {
    resolve(
        &CameraObstacles(vec![colliders]),
        state,
        LOOK_AT,
        RAW,
        now,
        FRAME,
    )
}

#[test]
fn a_wall_on_physics_and_pointer_pulls_the_camera_in_front_of_it() {
    let mut colliders = SceneColliderData::default();
    wall(&mut colliders, PHYSICS | POINTER);

    let resolved = step(&colliders, &mut CameraCollisionState::default(), 1.0);

    assert!(resolved.occluded);
    // the ball of radius 0.2 rests against the face at 2.9
    assert!(
        (resolved.position.z - 2.7).abs() < 0.01,
        "{:?}",
        resolved.position
    );
    assert!(resolved.position.xy().length() < 0.01);
}

#[test]
fn walls_on_one_layer_alone_are_not_obstacles() {
    for layers in [PHYSICS, POINTER, ColliderLayer::ClCustom1 as u32] {
        let mut colliders = SceneColliderData::default();
        wall(&mut colliders, layers);

        let resolved = step(&colliders, &mut CameraCollisionState::default(), 1.0);

        assert!(!resolved.occluded, "layers {layers}");
        assert_eq!(resolved.position, RAW, "layers {layers}");
    }
}

#[test]
fn the_ground_is_an_obstacle() {
    let mut colliders = SceneColliderData::default();
    wall(&mut colliders, PHYSICS | GROUND_COLLISION_MASK);

    let resolved = step(&colliders, &mut CameraCollisionState::default(), 1.0);

    assert!(resolved.occluded);
    assert!((resolved.position.z - 2.7).abs() < 0.01);
}

#[test]
fn the_camera_pulls_in_over_the_occluded_damping_time() {
    let mut colliders = SceneColliderData::default();
    let mut state = CameraCollisionState::default();
    let mut now = 1.0;
    step(&colliders, &mut state, now);

    wall(&mut colliders, PHYSICS | POINTER);
    now += FRAME;
    let first = step(&colliders, &mut state, now);
    // one frame of a 0.2 s damping covers 1 - 0.01^(FRAME / 0.2), about 32%, of the way
    let expected = 5.0 - (5.0 - 2.7) * (1.0 - 0.01f32.powf(FRAME / 0.2));
    assert!(
        (first.position.z - expected).abs() < 0.02,
        "{} vs {expected}",
        first.position.z
    );
    assert_eq!(first.damp_time, 0.2);

    for _ in 0..30 {
        now += FRAME;
        step(&colliders, &mut state, now);
    }
    now += FRAME;
    let settled = step(&colliders, &mut state, now);
    assert!(
        (settled.position.z - 2.7).abs() < 0.02,
        "{:?}",
        settled.position
    );
}

#[test]
fn the_camera_holds_then_releases_after_the_obstacle_clears() {
    let mut colliders = SceneColliderData::default();
    wall(&mut colliders, PHYSICS | POINTER);
    let mut state = CameraCollisionState::default();
    let mut now = 1.0;
    step(&colliders, &mut state, now);

    colliders.remove_collider(&collider_id(0));
    colliders.prepare_camera_queries();

    // the smoothing time holds the distance for 0.3 s
    let mut held = Vec::new();
    while now < 1.25 {
        now += FRAME;
        held.push(step(&colliders, &mut state, now).position.z);
    }
    assert!(held.iter().all(|z| (z - 2.7).abs() < 0.02), "{held:?}");

    // then the release damping of 0.5 s takes the camera back out
    let mut z = 0.0;
    while now < 3.0 {
        now += FRAME;
        z = step(&colliders, &mut state, now).position.z;
    }
    assert!((z - 5.0).abs() < 0.01, "{z}");
}

#[test]
fn the_camera_keeps_its_radius_from_the_target() {
    let mut colliders = SceneColliderData::default();
    add_box(
        &mut colliders,
        0,
        PHYSICS | POINTER,
        Vec3::new(0.0, 0.0, 0.15),
        Vec3::new(4.0, 4.0, 0.1),
    );

    let resolved = step(&colliders, &mut CameraCollisionState::default(), 1.0);

    let distance = resolved.position.distance(LOOK_AT);
    assert!(distance >= CAMERA_RADIUS, "{:?}", resolved.position);
}

/// A wall whose near face is `face` metres to the right of the origin.
fn wall_to_the_right(face: f32) -> SceneColliderData {
    let mut colliders = SceneColliderData::default();
    add_box(
        &mut colliders,
        0,
        PHYSICS | POINTER,
        Vec3::new(face + 0.1, 0.0, 0.0),
        Vec3::new(0.2, 4.0, 4.0),
    );
    colliders
}

const SHOULDER: Vec3 = Vec3::new(0.75, 0.0, 0.0);

#[test]
fn the_shoulder_offset_starts_short_of_a_wall() {
    let colliders = wall_to_the_right(0.6);
    let obstacles = CameraObstacles(vec![&colliders]);

    // with no history it takes the target at once: 0.5 m short of the wall
    let offset = shoulder_offset(
        &obstacles,
        &mut CameraCollisionState::default(),
        Vec3::ZERO,
        SHOULDER,
        FRAME,
    );
    assert!((offset.x - 0.1).abs() < 0.001, "{offset:?}");

    let free = shoulder_offset(
        &obstacles,
        &mut CameraCollisionState::default(),
        Vec3::ZERO,
        -SHOULDER,
        FRAME,
    );
    assert_eq!(free, -SHOULDER);
}

#[test]
fn the_shoulder_offset_stops_at_the_centre() {
    // a wall nearer than 0.5 m: stopping 0.5 m short of it would cross to the other side
    let colliders = wall_to_the_right(0.3);
    let obstacles = CameraObstacles(vec![&colliders]);

    let offset = shoulder_offset(
        &obstacles,
        &mut CameraCollisionState::default(),
        Vec3::ZERO,
        SHOULDER,
        FRAME,
    );
    assert_eq!(offset, Vec3::ZERO);
}

#[test]
fn the_shoulder_offset_eases_in_and_out() {
    let empty = SceneColliderData::default();
    let free = CameraObstacles(vec![&empty]);
    // the wall comes within reach of the offset, not of the camera's sphere
    let colliders = wall_to_the_right(0.74);
    let blocked = CameraObstacles(vec![&colliders]);
    let mut state = CameraCollisionState::default();

    assert_eq!(
        shoulder_offset(&free, &mut state, Vec3::ZERO, SHOULDER, FRAME),
        SHOULDER
    );

    // in: towards 0.24 m over 0.2 s
    let mut previous = SHOULDER.x;
    for frame in 0..30 {
        let x = shoulder_offset(&blocked, &mut state, Vec3::ZERO, SHOULDER, FRAME).x;
        assert!(x <= 0.74 - CAMERA_RADIUS + 0.001, "frame {frame}: {x}");
        assert!(previous - x < 0.25, "frame {frame}: {previous} -> {x}");
        previous = x;
    }
    assert!((previous - 0.24).abs() < 0.01, "{previous}");

    // out over 0.5 s
    let first = shoulder_offset(&free, &mut state, Vec3::ZERO, SHOULDER, FRAME).x;
    assert!(first < 0.4, "{first}");
    for _ in 0..60 {
        previous = shoulder_offset(&free, &mut state, Vec3::ZERO, SHOULDER, FRAME).x;
    }
    assert!((previous - 0.75).abs() < 0.01, "{previous}");
}

#[test]
fn a_triangle_mesh_wall_pulls_the_camera_in() {
    // glTF colliders are triangle meshes: a 4 m square facing the target at z = 2.9
    let mut colliders = SceneColliderData::default();
    let groups = Group::from_bits_truncate(PHYSICS | POINTER);
    let vertices = [
        [-2.0, -2.0, 2.9],
        [2.0, -2.0, 2.9],
        [2.0, 2.0, 2.9],
        [-2.0, 2.0, 2.9],
    ]
    .map(|[x, y, z]| rapier3d_f64::na::Point3::new(x, y, z));
    let collider = ColliderBuilder::trimesh(vertices.to_vec(), vec![[0, 1, 2], [0, 2, 3]])
        .unwrap()
        .collision_groups(InteractionGroups::new(
            groups,
            groups,
            InteractionTestMode::And,
        ))
        .build();
    let id = collider_id(0);
    colliders.set_collider(&id, collider, Some(Entity::from_raw(1)));
    colliders.update_collider_transform(&id, &GlobalTransform::IDENTITY);
    colliders.prepare_camera_queries();

    let resolved = step(&colliders, &mut CameraCollisionState::default(), 1.0);

    assert!(resolved.occluded);
    assert!(
        (resolved.position.z - 2.7).abs() < 0.01,
        "{:?}",
        resolved.position
    );
}

#[test]
fn the_terrain_heightfield_pulls_the_camera_above_it() {
    // flat ground 1 m under the target, the camera looking up from below it
    let mut colliders = SceneColliderData::default();
    colliders.set_ground_heightfield(
        &collider_id(0),
        3,
        3,
        &[-1.0; 9],
        Vec2::splat(32.0),
        Vec3::ZERO,
    );
    colliders.prepare_camera_queries();

    let resolved = resolve(
        &CameraObstacles(vec![&colliders]),
        &mut CameraCollisionState::default(),
        LOOK_AT,
        Vec3::new(0.0, -3.0, 4.0),
        1.0,
        FRAME,
    );

    assert!(resolved.occluded);
    assert!(resolved.position.y >= -0.81, "{:?}", resolved.position);
}

#[test]
fn orbiting_while_occluded_keeps_the_camera_pulled_in() {
    // a cylinder of wall all around the target: the damping must follow the orbit, not lag
    let mut colliders = SceneColliderData::default();
    for (index, angle) in (0..16)
        .map(|i| i as f32 * std::f32::consts::TAU / 16.0)
        .enumerate()
    {
        let centre = Quat::from_rotation_y(angle) * Vec3::new(0.0, 0.0, 3.0);
        add_box(
            &mut colliders,
            index as u16,
            PHYSICS | POINTER,
            centre,
            Vec3::new(1.4, 4.0, 0.2),
        );
        colliders.update_collider_transform(
            &collider_id(index as u16),
            &GlobalTransform::from(
                Transform::from_translation(centre)
                    .with_rotation(Quat::from_rotation_y(angle))
                    .with_scale(Vec3::new(1.4, 4.0, 0.2)),
            ),
        );
    }
    colliders.prepare_camera_queries();
    let obstacles = CameraObstacles(vec![&colliders]);
    let mut state = CameraCollisionState::default();

    let mut now = 1.0;
    for frame in 0..120 {
        let raw = Quat::from_rotation_y(frame as f32 * 0.05) * RAW;
        now += FRAME;
        let resolved = resolve(&obstacles, &mut state, LOOK_AT, raw, now, FRAME);
        let distance = resolved.position.distance(LOOK_AT);
        assert!(distance < 2.95, "frame {frame}: {distance}");
    }
}

#[test]
fn orbiting_into_a_corner_moves_the_shoulder_without_a_jump() {
    // the player stands 0.6 m from a side wall, a back wall behind; orbiting towards the side wall
    // brings it within the shoulder offset's reach, which must not move the offset in one frame
    let mut colliders = SceneColliderData::default();
    wall(&mut colliders, PHYSICS | POINTER);
    add_box(
        &mut colliders,
        1,
        PHYSICS | POINTER,
        Vec3::new(0.7, 0.0, 3.0),
        Vec3::new(0.2, 4.0, 8.0),
    );
    let obstacles = CameraObstacles(vec![&colliders]);
    let mut state = CameraCollisionState::default();

    let mut now = 1.0;
    let mut previous: Option<Vec3> = None;
    for frame in 0..120 {
        let rotation = Quat::from_rotation_y(-0.6 + frame as f32 * 0.01);
        let offset = shoulder_offset(&obstacles, &mut state, LOOK_AT, rotation * SHOULDER, FRAME);
        let pivot = LOOK_AT + offset;
        now += FRAME;
        resolve(
            &obstacles,
            &mut state,
            pivot,
            pivot + rotation * RAW,
            now,
            FRAME,
        );
        if let Some(previous) = previous {
            let step = offset.distance(previous);
            assert!(step < 0.1, "frame {frame}: the offset moved {step} m");
        }
        previous = Some(offset);
    }
}
