use super::*;
use common::{bounds_calc::scene_regions, structs::ShowOutOfBounds};
use dcl_types::SceneId;
use scene_material::{BoundRegion, SceneBounds};

const MASK: u32 = ColliderLayer::ClPointer as u32 | ColliderLayer::ClPhysics as u32;

pub(crate) fn regions(parcels: &[(i32, i32)]) -> Vec<BoundRegion> {
    scene_regions(parcels.iter().map(|(x, y)| IVec2::new(*x, *y)))
        .into_iter()
        .map(|region| BoundRegion::new(region.min, region.max, region.count))
        .collect()
}

/// Collider data of a scene on `parcels`. Parcel (x, y) spans world x from 16x to 16x + 16 and
/// world z from -16y - 16 to -16y.
fn scene(parcels: &[(i32, i32)]) -> SceneColliderData {
    SceneColliderData::for_regions(&regions(parcels), parcels.len())
}

fn entity_id(index: u32) -> SceneEntityId {
    SceneEntityId {
        id: 512 + index as u16,
        generation: 0,
    }
}

fn add(colliders: &mut SceneColliderData, index: u32, sensor: bool) -> ColliderId {
    let id = ColliderId::new(entity_id(index), None, 0);
    let collider = ColliderBuilder::cuboid(0.5, 0.5, 0.5)
        .collision_groups(InteractionGroups::new(
            Group::from_bits_truncate(MASK),
            Group::from_bits_truncate(MASK),
            InteractionTestMode::And,
        ))
        .sensor(sensor)
        .build();
    colliders.set_collider(&id, collider, Some(Entity::from_raw(index + 1)));
    id
}

/// A unit box, as a scene entity's `MeshCollider` registers it.
fn add_box(colliders: &mut SceneColliderData, index: u32) -> ColliderId {
    add(colliders, index, false)
}

fn place(colliders: &mut SceneColliderData, id: &ColliderId, centre: Vec3, size: Vec3) {
    colliders.update_collider_transform(
        id,
        &GlobalTransform::from(Transform::from_translation(centre).with_scale(size)),
    );
}

fn ray_down(colliders: &mut SceneColliderData, x: f32, z: f32, sensors: bool) -> bool {
    colliders
        .cast_ray_nearest(
            Vec3::new(x, 500.0, z),
            Vec3::NEG_Y,
            1000.0,
            MASK,
            false,
            sensors,
            None,
        )
        .is_some()
}

/// Whether a ray straight down at (x, z) hits a collider.
fn hit_at(colliders: &mut SceneColliderData, x: f32, z: f32) -> bool {
    ray_down(colliders, x, z, false)
}

#[test]
fn a_collider_inside_its_scene_is_hit() {
    let mut colliders = scene(&[(0, 0)]);
    let id = add_box(&mut colliders, 0);
    place(
        &mut colliders,
        &id,
        Vec3::new(8.0, 1.0, -8.0),
        Vec3::splat(2.0),
    );
    assert!(hit_at(&mut colliders, 8.0, -8.0));
}

#[test]
fn a_collider_reaching_into_the_next_parcel_is_off_as_a_whole() {
    let mut colliders = scene(&[(0, 0)]);
    let id = add_box(&mut colliders, 0);
    // x from 13 to 17, a metre past the scene's edge at 16
    place(
        &mut colliders,
        &id,
        Vec3::new(15.0, 1.0, -8.0),
        Vec3::new(4.0, 2.0, 2.0),
    );
    assert!(!hit_at(&mut colliders, 16.5, -8.0));
    assert!(!hit_at(&mut colliders, 14.0, -8.0));
}

#[test]
fn a_collider_may_overhang_the_edge_by_a_fifth_of_a_metre() {
    let mut colliders = scene(&[(0, 0)]);
    let id = add_box(&mut colliders, 0);
    let size = Vec3::splat(2.0);

    // 0.19 m past the edge at x = 16
    place(&mut colliders, &id, Vec3::new(15.19, 1.0, -8.0), size);
    assert!(hit_at(&mut colliders, 15.19, -8.0));
    // 0.21 m past it
    place(&mut colliders, &id, Vec3::new(15.21, 1.0, -8.0), size);
    assert!(!hit_at(&mut colliders, 15.21, -8.0));

    // the same on the low side, past the edge at z = -16
    place(&mut colliders, &id, Vec3::new(8.0, 1.0, -15.19), size);
    assert!(hit_at(&mut colliders, 8.0, -15.19));
    place(&mut colliders, &id, Vec3::new(8.0, 1.0, -15.21), size);
    assert!(!hit_at(&mut colliders, 8.0, -15.21));
}

#[test]
fn a_thin_collider_may_overhang_by_less() {
    let mut colliders = scene(&[(0, 0)]);
    let id = add_box(&mut colliders, 0);
    // 0.1 m thick: the allowance is a quarter of that plus 0.05 m, on both axes
    let size = Vec3::new(0.1, 2.0, 4.0);

    // 0.07 m past the edge at z = 0
    place(&mut colliders, &id, Vec3::new(8.0, 1.0, -1.93), size);
    assert!(hit_at(&mut colliders, 8.0, -1.93));
    // 0.08 m past it
    place(&mut colliders, &id, Vec3::new(8.0, 1.0, -1.92), size);
    assert!(!hit_at(&mut colliders, 8.0, -1.92));
}

#[test]
fn a_thin_collider_on_the_edge_line_stays() {
    let mut colliders = scene(&[(0, 0)]);
    let id = add_box(&mut colliders, 0);
    // a fence a centimetre thick, along the scene's edge at x = 16 and at x = 0
    let size = Vec3::new(0.01, 2.0, 16.0);

    place(&mut colliders, &id, Vec3::new(16.0, 1.0, -8.0), size);
    assert!(hit_at(&mut colliders, 16.0, -8.0));
    place(&mut colliders, &id, Vec3::new(0.0, 1.0, -8.0), size);
    assert!(hit_at(&mut colliders, 0.0, -8.0));
}

#[test]
fn a_thin_collider_keeps_the_allowance_across_its_thin_side() {
    let mut colliders = scene(&[(0, 0)]);
    let id = add_box(&mut colliders, 0);
    // 0.1 m thick: it may reach a quarter of that plus 0.05 m past the edge
    let size = Vec3::new(0.1, 2.0, 4.0);

    // centred a centimetre outside either edge, so 0.06 m past it
    place(&mut colliders, &id, Vec3::new(16.01, 1.0, -8.0), size);
    assert!(hit_at(&mut colliders, 16.01, -8.0));
    place(&mut colliders, &id, Vec3::new(-0.01, 1.0, -8.0), size);
    assert!(hit_at(&mut colliders, -0.01, -8.0));
    // centred 0.1 m outside, so 0.15 m past it
    place(&mut colliders, &id, Vec3::new(16.1, 1.0, -8.0), size);
    assert!(!hit_at(&mut colliders, 16.1, -8.0));
}

#[test]
fn a_collider_above_the_height_limit_is_off() {
    // one parcel: 20 m
    let mut colliders = scene(&[(0, 0)]);
    let id = add_box(&mut colliders, 0);
    let size = Vec3::splat(2.0);

    place(&mut colliders, &id, Vec3::new(8.0, 18.9, -8.0), size);
    assert!(hit_at(&mut colliders, 8.0, -8.0));
    place(&mut colliders, &id, Vec3::new(8.0, 19.1, -8.0), size);
    assert!(!hit_at(&mut colliders, 8.0, -8.0));
}

#[test]
fn the_height_limit_grows_with_the_parcel_count() {
    // four parcels: log2(5) * 20 = 46.4 m
    let mut colliders = scene(&[(0, 0), (1, 0), (0, 1), (1, 1)]);
    let id = add_box(&mut colliders, 0);
    let size = Vec3::splat(2.0);

    place(&mut colliders, &id, Vec3::new(8.0, 45.0, -8.0), size);
    assert!(hit_at(&mut colliders, 8.0, -8.0));
    place(&mut colliders, &id, Vec3::new(8.0, 46.0, -8.0), size);
    assert!(!hit_at(&mut colliders, 8.0, -8.0));
}

#[test]
fn a_collider_below_the_ground_stays() {
    let mut colliders = scene(&[(0, 0)]);
    let id = add_box(&mut colliders, 0);
    place(
        &mut colliders,
        &id,
        Vec3::new(8.0, -50.0, -8.0),
        Vec3::splat(2.0),
    );
    assert!(hit_at(&mut colliders, 8.0, -8.0));
}

#[test]
fn a_collider_that_moves_back_inside_is_hit_again() {
    let mut colliders = scene(&[(0, 0)]);
    let id = add_box(&mut colliders, 0);
    let size = Vec3::splat(2.0);

    place(&mut colliders, &id, Vec3::new(24.0, 1.0, -8.0), size);
    assert!(!hit_at(&mut colliders, 24.0, -8.0));

    place(&mut colliders, &id, Vec3::new(8.0, 1.0, -8.0), size);
    assert!(hit_at(&mut colliders, 8.0, -8.0));
    assert!(!hit_at(&mut colliders, 24.0, -8.0));
}

#[test]
fn a_collider_over_the_notch_of_an_irregular_scene_stays() {
    // an L: the notch is parcel (1, 1), x from 16 to 32 and z from -32 to -16. the scene's
    // rectangle spans x from 0 to 32 and z from -32 to 0
    let mut colliders = scene(&[(0, 0), (1, 0), (0, 1)]);
    let id = add_box(&mut colliders, 0);

    // across the whole rectangle, the notch included
    place(
        &mut colliders,
        &id,
        Vec3::new(16.0, 1.0, -16.0),
        Vec3::new(31.0, 2.0, 31.0),
    );
    assert!(hit_at(&mut colliders, 8.0, -8.0));
    // in the notch alone
    place(
        &mut colliders,
        &id,
        Vec3::new(24.0, 1.0, -24.0),
        Vec3::splat(2.0),
    );
    assert!(hit_at(&mut colliders, 24.0, -24.0));
    // past the rectangle
    place(
        &mut colliders,
        &id,
        Vec3::new(34.0, 1.0, -24.0),
        Vec3::splat(2.0),
    );
    assert!(!hit_at(&mut colliders, 34.0, -24.0));
}

#[test]
fn parcels_apart_share_one_height_limit_from_their_total() {
    // two parcels apart: log2(3) * 20 = 31.7 m over each, where one parcel alone has 20 m
    let mut colliders = scene(&[(0, 0), (5, 5)]);
    let id = add_box(&mut colliders, 0);
    let size = Vec3::splat(2.0);

    place(&mut colliders, &id, Vec3::new(8.0, 30.0, -8.0), size);
    assert!(hit_at(&mut colliders, 8.0, -8.0));
    place(&mut colliders, &id, Vec3::new(8.0, 31.0, -8.0), size);
    assert!(!hit_at(&mut colliders, 8.0, -8.0));
}

#[test]
fn a_height_limit_set_on_the_regions_replaces_the_parcel_count_limit() {
    let regions = regions(&[(0, 0)])
        .into_iter()
        .map(|region| BoundRegion {
            height: 330.0,
            ..region
        })
        .collect::<Vec<_>>();
    let mut colliders = SceneColliderData::for_regions(&regions, 1);
    let id = add_box(&mut colliders, 0);
    let size = Vec3::splat(2.0);

    place(&mut colliders, &id, Vec3::new(8.0, 328.0, -8.0), size);
    assert!(hit_at(&mut colliders, 8.0, -8.0));
    place(&mut colliders, &id, Vec3::new(8.0, 330.0, -8.0), size);
    assert!(!hit_at(&mut colliders, 8.0, -8.0));
}

#[test]
fn a_collider_far_larger_than_its_scene_is_off() {
    let mut colliders = scene(&[(0, 0)]);
    let id = add_box(&mut colliders, 0);
    place(
        &mut colliders,
        &id,
        Vec3::new(8.0, 1.0, -8.0),
        Vec3::new(1.0e6, 2.0, 1.0e6),
    );
    assert!(!hit_at(&mut colliders, 8.0, -8.0));
}

#[test]
fn a_scene_without_regions_keeps_every_collider() {
    // a portable experience
    let mut colliders = scene(&[]);
    let id = add_box(&mut colliders, 0);
    place(
        &mut colliders,
        &id,
        Vec3::new(1000.0, 300.0, 1000.0),
        Vec3::splat(2.0),
    );
    assert!(hit_at(&mut colliders, 1000.0, 1000.0));
}

#[test]
fn a_trigger_outside_its_scene_stays() {
    let mut colliders = scene(&[(0, 0)]);
    let id = add(&mut colliders, 0, true);
    place(
        &mut colliders,
        &id,
        Vec3::new(24.0, 1.0, -8.0),
        Vec3::splat(2.0),
    );
    assert!(ray_down(&mut colliders, 24.0, -8.0, true));
}

#[test]
fn a_collider_outside_its_scene_is_no_proximity_target() {
    let mut colliders = scene(&[(0, 0)]);
    let id = add_box(&mut colliders, 0);
    let size = Vec3::splat(2.0);
    let player = Vec3::new(20.0, 1.0, -8.0);

    place(&mut colliders, &id, Vec3::new(8.0, 1.0, -8.0), size);
    assert!(colliders
        .closest_point_to_entity(player, id.entity, MASK)
        .is_some());

    place(&mut colliders, &id, Vec3::new(24.0, 1.0, -8.0), size);
    assert!(colliders
        .closest_point_to_entity(player, id.entity, MASK)
        .is_none());
}

#[test]
fn a_collider_scaled_to_nothing_is_not_hit_until_it_grows_back() {
    let mut colliders = scene(&[(0, 0)]);
    let id = add_box(&mut colliders, 0);
    let centre = Vec3::new(8.0, 1.0, -8.0);

    place(&mut colliders, &id, centre, Vec3::splat(2.0));
    assert!(hit_at(&mut colliders, 8.0, -8.0));

    place(&mut colliders, &id, centre, Vec3::splat(0.001));
    assert!(!hit_at(&mut colliders, 8.0, -8.0));
    // a move inside the scene does not bring it back
    place(
        &mut colliders,
        &id,
        Vec3::new(4.0, 1.0, -4.0),
        Vec3::splat(0.001),
    );
    assert!(!hit_at(&mut colliders, 4.0, -4.0));

    place(
        &mut colliders,
        &id,
        Vec3::new(4.0, 1.0, -4.0),
        Vec3::splat(2.0),
    );
    assert!(hit_at(&mut colliders, 4.0, -4.0));
}

#[test]
fn a_rescale_outside_the_scene_keeps_the_collider_off() {
    let mut colliders = scene(&[(0, 0)]);
    let id = add_box(&mut colliders, 0);
    let centre = Vec3::new(24.0, 1.0, -8.0);

    place(&mut colliders, &id, centre, Vec3::splat(2.0));
    assert!(!hit_at(&mut colliders, 24.0, -8.0));
    place(&mut colliders, &id, centre, Vec3::splat(3.0));
    assert!(!hit_at(&mut colliders, 24.0, -8.0));
}

#[test]
fn removing_a_collider_that_is_off_leaves_the_others_in_place() {
    let mut colliders = scene(&[(0, 0)]);
    let size = Vec3::splat(2.0);
    let outside = add_box(&mut colliders, 0);
    let inside = add_box(&mut colliders, 1);
    place(&mut colliders, &outside, Vec3::new(24.0, 1.0, -8.0), size);
    place(&mut colliders, &inside, Vec3::new(8.0, 1.0, -8.0), size);
    assert!(!hit_at(&mut colliders, 24.0, -8.0));

    colliders.remove_collider(&outside);
    assert!(hit_at(&mut colliders, 8.0, -8.0));

    // a new collider can take over the removed one's slot
    let replacement = add_box(&mut colliders, 2);
    place(
        &mut colliders,
        &replacement,
        Vec3::new(4.0, 1.0, -4.0),
        size,
    );
    assert!(hit_at(&mut colliders, 4.0, -4.0));
    assert!(hit_at(&mut colliders, 8.0, -8.0));
}

#[test]
fn a_collider_turned_off_removed_and_replaced_before_the_next_query_leaves_no_trace() {
    let mut colliders = scene(&[(0, 0)]);
    let size = Vec3::splat(2.0);
    let first = add_box(&mut colliders, 0);
    place(&mut colliders, &first, Vec3::new(8.0, 1.0, -8.0), size);
    assert!(hit_at(&mut colliders, 8.0, -8.0));
    let slot = |colliders: &SceneColliderData, id: &ColliderId| {
        colliders
            .get_collider_handle(id)
            .unwrap()
            .into_raw_parts()
            .0
    };
    let first_slot = slot(&colliders, &first);

    // it leaves the scene, is removed, and its slot is taken over, all before the next query
    place(&mut colliders, &first, Vec3::new(24.0, 1.0, -8.0), size);
    colliders.remove_collider(&first);
    let replacement = add_box(&mut colliders, 1);
    assert_eq!(
        slot(&colliders, &replacement),
        first_slot,
        "the replacement is meant to take over the removed collider's slot"
    );
    place(
        &mut colliders,
        &replacement,
        Vec3::new(4.0, 1.0, -4.0),
        size,
    );

    assert!(hit_at(&mut colliders, 4.0, -4.0));
    assert!(!hit_at(&mut colliders, 8.0, -8.0));
    assert!(!hit_at(&mut colliders, 24.0, -8.0));
}

#[test]
fn a_collider_that_leaves_and_returns_between_two_queries_is_hit() {
    let mut colliders = scene(&[(0, 0)]);
    let id = add_box(&mut colliders, 0);
    let size = Vec3::splat(2.0);
    place(&mut colliders, &id, Vec3::new(8.0, 1.0, -8.0), size);
    assert!(hit_at(&mut colliders, 8.0, -8.0));

    place(&mut colliders, &id, Vec3::new(24.0, 1.0, -8.0), size);
    place(&mut colliders, &id, Vec3::new(4.0, 1.0, -4.0), size);
    assert!(hit_at(&mut colliders, 4.0, -4.0));
    assert!(!hit_at(&mut colliders, 8.0, -8.0));
}

#[test]
fn a_scene_on_negative_parcels_is_bounded_where_it_lies() {
    // parcel (-2, -3): x from -32 to -16, z from 32 to 48
    let mut colliders = scene(&[(-2, -3)]);
    let id = add_box(&mut colliders, 0);
    let size = Vec3::splat(2.0);

    place(&mut colliders, &id, Vec3::new(-24.0, 1.0, 40.0), size);
    assert!(hit_at(&mut colliders, -24.0, 40.0));
    // past each edge in turn
    for (x, z) in [(-34.0, 40.0), (-14.0, 40.0), (-24.0, 30.0), (-24.0, 50.0)] {
        place(&mut colliders, &id, Vec3::new(x, 1.0, z), size);
        assert!(!hit_at(&mut colliders, x, z), "{x}, {z}");
    }
}

#[test]
fn a_turned_mesh_is_tested_by_its_local_box_turned_with_it() {
    let mut colliders = scene(&[(0, 0)]);
    // a strip along the diagonal of its own 10 m square
    let strip = SharedShape::trimesh(
        vec![
            Point::new(-5.0, 0.0, -4.9),
            Point::new(-4.9, 0.0, -5.0),
            Point::new(5.0, 0.0, 4.9),
            Point::new(4.9, 0.0, 5.0),
        ],
        vec![[0, 1, 2], [0, 2, 3]],
    )
    .unwrap();
    let id = ColliderId::new(entity_id(0), None, 0);
    let collider = ColliderBuilder::new(strip)
        .collision_groups(InteractionGroups::new(
            Group::from_bits_truncate(MASK),
            Group::from_bits_truncate(MASK),
            InteractionTestMode::And,
        ))
        .build();
    colliders.set_collider(&id, collider, Some(Entity::from_raw(1)));

    // as authored, its square is inside the scene
    colliders.update_collider_transform(
        &id,
        &GlobalTransform::from_translation(Vec3::new(8.0, 1.0, -8.0)),
    );
    assert!(hit_at(&mut colliders, 8.0, -8.0));

    // turned an eighth of a circle the strip runs along x, a metre inside the edge at z = 0,
    // but its square, turned with it, reaches six metres past that edge
    colliders.update_collider_transform(
        &id,
        &GlobalTransform::from(
            Transform::from_translation(Vec3::new(8.0, 1.0, -1.0))
                .with_rotation(Quat::from_rotation_y(std::f32::consts::FRAC_PI_4)),
        ),
    );
    assert!(!hit_at(&mut colliders, 8.0, -1.0));
}

#[test]
fn the_avatar_and_triggers_do_not_meet_a_collider_outside_its_scene() {
    let mut colliders = scene(&[(0, 0)]);
    let id = add_box(&mut colliders, 0);
    let size = Vec3::splat(2.0);
    let trigger = |centre: Vec3| {
        ColliderBuilder::ball(0.5)
            .translation(centre.as_dvec3().into())
            .build()
    };

    place(&mut colliders, &id, Vec3::new(8.0, 1.0, -8.0), size);
    assert!(colliders.get_ground(Vec3::new(8.0, 3.0, -8.0)).is_some());
    assert_eq!(
        colliders.intersect_collider(&trigger(Vec3::new(8.0, 1.0, -8.0)), MASK),
        std::slice::from_ref(&id)
    );

    place(&mut colliders, &id, Vec3::new(24.0, 1.0, -8.0), size);
    assert!(colliders.get_ground(Vec3::new(24.0, 3.0, -8.0)).is_none());
    assert!(colliders
        .intersect_collider(&trigger(Vec3::new(24.0, 1.0, -8.0)), MASK)
        .is_empty());
}

#[test]
fn the_colliders_off_for_bounds_are_counted() {
    let mut colliders = scene(&[(0, 0)]);
    let size = Vec3::splat(2.0);
    let outside = add_box(&mut colliders, 0);
    let inside = add_box(&mut colliders, 1);
    place(&mut colliders, &inside, Vec3::new(8.0, 1.0, -8.0), size);
    assert_eq!(colliders.out_of_bounds_count(), 0);

    place(&mut colliders, &outside, Vec3::new(24.0, 1.0, -8.0), size);
    assert_eq!(colliders.out_of_bounds_count(), 1);

    colliders.remove_collider(&outside);
    assert_eq!(colliders.out_of_bounds_count(), 0);
}

/// A scene on parcel (0, 0) that `update_scene_collider_data` gave its collider data, in a run
/// that shows what is out of bounds or in one that does not.
pub(super) fn scene_in_a_run(show_out_of_bounds: bool) -> (App, Entity) {
    let mut app = App::new();
    app.insert_resource(ShowOutOfBounds(show_out_of_bounds))
        .add_systems(Update, update_scene_collider_data);
    let scene = app
        .world_mut()
        .spawn(RendererSceneContext::new(
            SceneId::DUMMY,
            "hash".to_owned(),
            "storage_root".to_owned(),
            false,
            0,
            "title".to_owned(),
            IVec2::ZERO,
            HashSet::from_iter([IVec2::ZERO]),
            regions(&[(0, 0)]),
            SceneBounds::default(),
            vec![],
            Entity::PLACEHOLDER,
            0.,
            false,
            "sdk7",
            false,
            false,
        ))
        .id();
    app.update();
    (app, scene)
}

#[test]
fn a_scenes_collider_data_carries_the_scenes_bounds() {
    let (mut app, scene) = scene_in_a_run(false);
    let mut colliders = app.world_mut().get_mut::<SceneColliderData>(scene).unwrap();
    let id = add_box(&mut colliders, 0);
    place(
        &mut colliders,
        &id,
        Vec3::new(24.0, 1.0, -8.0),
        Vec3::splat(2.0),
    );
    assert!(!hit_at(&mut colliders, 24.0, -8.0));
}

#[test]
fn a_run_that_shows_what_is_out_of_bounds_keeps_its_colliders_on() {
    let (mut app, scene) = scene_in_a_run(true);
    let mut colliders = app.world_mut().get_mut::<SceneColliderData>(scene).unwrap();
    let id = add_box(&mut colliders, 0);
    let size = Vec3::splat(2.0);

    // past the scene's edge
    place(&mut colliders, &id, Vec3::new(24.0, 1.0, -8.0), size);
    assert!(hit_at(&mut colliders, 24.0, -8.0));
    // above its height limit
    place(&mut colliders, &id, Vec3::new(8.0, 40.0, -8.0), size);
    assert!(hit_at(&mut colliders, 8.0, -8.0));
    assert_eq!(colliders.out_of_bounds_count(), 0);
}
