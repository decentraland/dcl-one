use super::{
    bounds_tests::{regions, scene_in_a_run},
    *,
};

const MASK: u32 = ColliderLayer::ClPointer as u32 | ColliderLayer::ClPhysics as u32;

/// A scene around a notch: parcels (0, 0) and (2, 0), joined by the row (0, 1), (1, 1), (2, 1).
/// Parcel (x, y) spans world x from 16x to 16x + 16 and world z from -16y - 16 to -16y, so the
/// notch at (1, 0), which is not the scene's, spans x 16 to 32 and z -16 to 0. The rectangle
/// around the scene covers the notch, so a collider over it stays on.
const AROUND_A_NOTCH: [(i32, i32); 5] = [(0, 0), (2, 0), (0, 1), (1, 1), (2, 1)];

/// Two parcels of one scene with another's between them: each is a region of its own, limited
/// to 20 m, while a collider is on up to the 31.7 m of a two-parcel scene.
const IN_TWO_PIECES: [(i32, i32); 2] = [(0, 0), (2, 0)];

fn scene(parcels: &[(i32, i32)]) -> SceneColliderData {
    SceneColliderData::for_regions(&regions(parcels), parcels.len())
}

/// A box of `size` centred on `centre`, as a scene entity's `MeshCollider` registers it.
fn add_box(colliders: &mut SceneColliderData, index: u16, centre: Vec3, size: Vec3) -> ColliderId {
    let id = ColliderId::new(
        SceneEntityId {
            id: 512 + index,
            generation: 0,
        },
        None,
        0,
    );
    let collider = ColliderBuilder::cuboid(0.5, 0.5, 0.5)
        .collision_groups(InteractionGroups::new(
            Group::from_bits_truncate(MASK),
            Group::from_bits_truncate(MASK),
            InteractionTestMode::And,
        ))
        .build();
    colliders.set_collider(&id, collider, Some(Entity::from_raw(u32::from(index) + 1)));
    colliders.update_collider_transform(
        &id,
        &GlobalTransform::from(Transform::from_translation(centre).with_scale(size)),
    );
    id
}

/// A slab a metre thick under the whole rectangle around [`AROUND_A_NOTCH`], notch included.
fn add_slab(colliders: &mut SceneColliderData, index: u16) -> ColliderId {
    add_box(
        colliders,
        index,
        Vec3::new(24.0, 0.5, -16.0),
        Vec3::new(48.0, 1.0, 32.0),
    )
}

fn ray(colliders: &mut SceneColliderData, origin: Vec3, direction: Vec3) -> Option<RaycastResult> {
    colliders.update_bvh();
    colliders.cast_ray_nearest_in_scene_flushed(origin, direction, 100.0, MASK, true, false, None)
}

fn whole_ray(
    colliders: &mut SceneColliderData,
    origin: Vec3,
    direction: Vec3,
) -> Option<RaycastResult> {
    colliders.cast_ray_nearest(origin, direction, 100.0, MASK, true, false, None)
}

/// The avatar's capsule swept from `feet` along `direction` for up to `distance`.
fn sweep(
    colliders: &mut SceneColliderData,
    feet: Vec3,
    direction: Vec3,
    distance: f64,
) -> Option<RaycastResult> {
    colliders.cast_avatar_nearest_in_scene(
        feet.as_dvec3(),
        direction.as_dvec3(),
        distance,
        MASK,
        false,
        false,
        HashSet::new(),
        -PLAYER_COLLIDER_OVERLAP,
    )
}

fn whole_sweep(
    colliders: &mut SceneColliderData,
    feet: Vec3,
    direction: Vec3,
    distance: f64,
) -> Option<RaycastResult> {
    colliders.cast_avatar_nearest(
        feet.as_dvec3(),
        direction.as_dvec3(),
        distance,
        MASK,
        false,
        false,
        HashSet::new(),
        -PLAYER_COLLIDER_OVERLAP,
    )
}

fn assert_near(actual: f32, expected: f32) {
    assert!(
        (actual - expected).abs() < 2e-3,
        "{actual} is not near {expected}"
    );
}

#[test]
fn a_ray_misses_the_part_of_a_collider_over_another_scenes_parcel() {
    let mut colliders = scene(&AROUND_A_NOTCH);
    add_slab(&mut colliders, 0);

    let over_the_notch = Vec3::new(24.0, 10.0, -8.0);
    assert!(whole_ray(&mut colliders, over_the_notch, Vec3::NEG_Y).is_some());
    assert!(ray(&mut colliders, over_the_notch, Vec3::NEG_Y).is_none());

    // the same slab over the scene's own parcels, on each side of the notch and behind it
    for own in [
        Vec3::new(8.0, 10.0, -8.0),
        Vec3::new(40.0, 10.0, -8.0),
        Vec3::new(24.0, 10.0, -24.0),
    ] {
        let hit = ray(&mut colliders, own, Vec3::NEG_Y).unwrap();
        assert_near(hit.toi, 9.0);
        assert_near(hit.position.y, 1.0);
    }
}

#[test]
fn a_ray_that_leaves_and_re_enters_the_scene_hits_what_stands_past_the_notch() {
    let mut colliders = scene(&AROUND_A_NOTCH);
    let post = Vec3::new(1.0, 4.0, 1.0);
    let in_the_notch = add_box(&mut colliders, 0, Vec3::new(24.0, 2.0, -8.0), post);
    let past_the_notch = add_box(&mut colliders, 1, Vec3::new(40.0, 2.0, -8.0), post);

    let origin = Vec3::new(2.0, 2.0, -8.0);
    assert_eq!(
        whole_ray(&mut colliders, origin, Vec3::X).unwrap().id,
        in_the_notch
    );

    let hit = ray(&mut colliders, origin, Vec3::X).unwrap();
    assert_eq!(hit.id, past_the_notch);
    assert_near(hit.toi, 37.5);
    assert_near(hit.position.x, 39.5);
}

#[test]
fn every_hit_of_a_ray_is_inside_the_scene() {
    let mut colliders = scene(&AROUND_A_NOTCH);
    let post = Vec3::new(1.0, 4.0, 1.0);
    let before = add_box(&mut colliders, 0, Vec3::new(8.0, 2.0, -8.0), post);
    add_box(&mut colliders, 1, Vec3::new(24.0, 2.0, -8.0), post);
    let past = add_box(&mut colliders, 2, Vec3::new(40.0, 2.0, -8.0), post);

    colliders.update_bvh();
    let origin = Vec3::new(2.0, 2.0, -8.0);
    assert_eq!(
        colliders
            .cast_ray_all_flushed(origin, Vec3::X, 100.0, MASK, true)
            .len(),
        3
    );

    let mut hits = colliders.cast_ray_all_in_scene_flushed(origin, Vec3::X, 100.0, MASK, true);
    hits.sort_by(|a, b| a.toi.total_cmp(&b.toi));
    assert_eq!(
        hits.iter().map(|hit| hit.id.clone()).collect::<Vec<_>>(),
        vec![before, past]
    );
    assert_near(hits[0].toi, 5.5);
    assert_near(hits[1].toi, 37.5);
}

#[test]
fn a_ray_is_cut_at_the_height_limit_of_the_region_under_it() {
    let mut colliders = scene(&IN_TWO_PIECES);
    let size = Vec3::splat(2.0);
    // on, under the scene's 31.7 m, and above the 20 m its region is drawn to
    add_box(&mut colliders, 0, Vec3::new(8.0, 25.0, -8.0), size);
    let below = add_box(&mut colliders, 1, Vec3::new(8.0, 10.0, -8.0), size);

    let above = Vec3::new(8.0, 40.0, -8.0);
    assert_near(
        whole_ray(&mut colliders, above, Vec3::NEG_Y).unwrap().toi,
        14.0,
    );

    let hit = ray(&mut colliders, above, Vec3::NEG_Y).unwrap();
    assert_eq!(hit.id, below);
    assert_near(hit.toi, 29.0);

    // and from below the limit upwards, the ray ends at it
    assert!(ray(&mut colliders, Vec3::new(8.0, 15.0, -8.0), Vec3::Y).is_none());
}

#[test]
fn a_ray_that_enters_the_scene_inside_a_solid_box_passes_through_it() {
    let mut colliders = scene(&AROUND_A_NOTCH);
    // x from 14 to 18, across the edge at 16 between parcel (0, 0) and the notch
    let across_the_edge = add_box(
        &mut colliders,
        0,
        Vec3::new(16.0, 2.0, -8.0),
        Vec3::splat(4.0),
    );
    let behind = add_box(
        &mut colliders,
        1,
        Vec3::new(8.0, 2.0, -8.0),
        Vec3::new(1.0, 4.0, 1.0),
    );

    let from_the_notch = Vec3::new(24.0, 2.0, -8.0);
    assert_eq!(
        whole_ray(&mut colliders, from_the_notch, Vec3::NEG_X)
            .unwrap()
            .id,
        across_the_edge
    );

    let hit = ray(&mut colliders, from_the_notch, Vec3::NEG_X).unwrap();
    assert_eq!(hit.id, behind);
    assert_near(hit.toi, 15.5);

    // from inside the scene the box is hit on the face it shows there
    let hit = ray(&mut colliders, Vec3::new(10.0, 2.0, -8.0), Vec3::X).unwrap();
    assert_eq!(hit.id, across_the_edge);
    assert_near(hit.toi, 4.0);
}

#[test]
fn a_scene_in_pieces_at_negative_parcels_answers_over_each_piece_only() {
    // (-2, -1) spans x -32 to -16 and z 0 to 16; (-1, -2) spans x -16 to 0 and z 16 to 32
    let mut colliders = scene(&[(-2, -1), (-1, -2)]);
    // a slab under the whole rectangle around the two, x -32 to 0 and z 0 to 32
    add_box(
        &mut colliders,
        0,
        Vec3::new(-16.0, 0.5, 16.0),
        Vec3::new(32.0, 1.0, 32.0),
    );

    for own in [Vec3::new(-24.0, 10.0, 8.0), Vec3::new(-8.0, 10.0, 24.0)] {
        assert_near(ray(&mut colliders, own, Vec3::NEG_Y).unwrap().toi, 9.0);
    }
    for other in [Vec3::new(-8.0, 10.0, 8.0), Vec3::new(-24.0, 10.0, 24.0)] {
        assert!(whole_ray(&mut colliders, other, Vec3::NEG_Y).is_some());
        assert!(ray(&mut colliders, other, Vec3::NEG_Y).is_none());
    }
}

#[test]
fn a_scene_answers_five_centimetres_past_its_edge_and_no_further() {
    let mut colliders = scene(&AROUND_A_NOTCH);
    add_slab(&mut colliders, 0);

    // the notch starts at x = 16
    assert!(ray(&mut colliders, Vec3::new(16.04, 10.0, -8.0), Vec3::NEG_Y).is_some());
    assert!(ray(&mut colliders, Vec3::new(16.06, 10.0, -8.0), Vec3::NEG_Y).is_none());
    // and ends at z = -16
    assert!(ray(&mut colliders, Vec3::new(24.0, 10.0, -15.96), Vec3::NEG_Y).is_some());
    assert!(ray(&mut colliders, Vec3::new(24.0, 10.0, -15.94), Vec3::NEG_Y).is_none());
}

#[test]
fn collider_data_without_regions_answers_everywhere() {
    // a portable's, the terrain's, or a scene's in a run that shows what is out of bounds
    let mut colliders = SceneColliderData::default();
    add_slab(&mut colliders, 0);
    add_box(
        &mut colliders,
        1,
        Vec3::new(24.0, 2.0, -8.0),
        Vec3::new(8.0, 2.0, 0.5),
    );

    assert!(ray(&mut colliders, Vec3::new(24.0, 10.0, -8.0), Vec3::NEG_Y).is_some());
    assert!(ray(&mut colliders, Vec3::new(240.0, 500.0, -8.0), Vec3::NEG_Y).is_none());
    assert!(colliders.reaches_avatar(DVec3::new(2400.0, 300.0, 2400.0)));
    assert!(sweep(&mut colliders, Vec3::new(24.0, 1.5, -4.0), Vec3::NEG_Z, 8.0).is_some());
}

#[test]
fn a_scene_is_cut_to_where_it_is_drawn_unless_the_run_shows_what_is_out_of_bounds() {
    for (show_out_of_bounds, reached) in [(false, false), (true, true)] {
        let (app, scene) = scene_in_a_run(show_out_of_bounds);
        let colliders = app.world().get::<SceneColliderData>(scene).unwrap();
        // over the next parcel, and above the 20 m a scene of one parcel is drawn to
        assert_eq!(
            colliders.reaches_avatar(DVec3::new(24.0, 0.0, -8.0)),
            reached
        );
        assert_eq!(
            colliders.reaches_avatar(DVec3::new(8.0, 40.0, -8.0)),
            reached
        );
    }
}

#[test]
fn a_scene_reaches_an_avatar_whose_capsule_touches_it() {
    let colliders = scene(&AROUND_A_NOTCH);

    assert!(colliders.reaches_avatar(DVec3::new(8.0, 0.0, -8.0)));
    // in the notch: the capsule's 0.3 m and the 0.05 m a scene is drawn past its edge
    assert!(colliders.reaches_avatar(DVec3::new(16.34, 0.0, -8.0)));
    assert!(!colliders.reaches_avatar(DVec3::new(16.36, 0.0, -8.0)));
    assert!(colliders.reaches_avatar(DVec3::new(24.0, 0.0, -15.66)));
    assert!(!colliders.reaches_avatar(DVec3::new(24.0, 0.0, -15.64)));
    assert!(!colliders.reaches_avatar(DVec3::new(24.0, 0.0, -8.0)));

    // five parcels in one piece are drawn up to log2(6) * 20 = 51.7 m
    assert!(colliders.reaches_avatar(DVec3::new(8.0, 51.7, -8.0)));
    assert!(!colliders.reaches_avatar(DVec3::new(8.0, 51.8, -8.0)));
}

#[test]
fn a_sweep_passes_the_part_of_a_collider_over_another_scenes_parcel() {
    let mut colliders = scene(&AROUND_A_NOTCH);
    // a wall along x through parcel (0, 0), the notch and parcel (2, 0)
    add_box(
        &mut colliders,
        0,
        Vec3::new(24.0, 2.0, -8.0),
        Vec3::new(48.0, 4.0, 0.5),
    );

    let in_the_notch = Vec3::new(24.0, 0.0, -4.0);
    assert_near(
        whole_sweep(&mut colliders, in_the_notch, Vec3::NEG_Z, 8.0)
            .unwrap()
            .toi,
        3.45,
    );
    assert!(sweep(&mut colliders, in_the_notch, Vec3::NEG_Z, 8.0).is_none());

    // the same wall inside the scene
    let in_the_scene = Vec3::new(8.0, 0.0, -4.0);
    assert_near(
        sweep(&mut colliders, in_the_scene, Vec3::NEG_Z, 8.0)
            .unwrap()
            .toi,
        3.45,
    );
}

#[test]
fn a_sweep_that_crosses_into_the_scene_in_one_step_stops_at_a_wall_inside_it() {
    let mut colliders = scene(&AROUND_A_NOTCH);
    // x from 9.75 to 10.25, six metres inside parcel (0, 0)
    add_box(
        &mut colliders,
        0,
        Vec3::new(10.0, 2.0, -8.0),
        Vec3::new(0.5, 4.0, 8.0),
    );

    let hit = sweep(
        &mut colliders,
        Vec3::new(24.0, 0.0, -8.0),
        Vec3::NEG_X,
        20.0,
    )
    .unwrap();
    assert_near(hit.toi, 13.45);
    assert_near(hit.normal.x, 1.0);
}

#[test]
fn a_wall_on_the_edge_line_stops_an_avatar_that_comes_from_outside() {
    let mut colliders = scene(&AROUND_A_NOTCH);
    // a fence a centimetre thick on the edge at x = 16
    add_box(
        &mut colliders,
        0,
        Vec3::new(16.0, 2.0, -8.0),
        Vec3::new(0.01, 4.0, 8.0),
    );

    let hit = sweep(&mut colliders, Vec3::new(20.0, 0.0, -8.0), Vec3::NEG_X, 6.0).unwrap();
    assert_near(hit.toi, 3.695);
    assert_near(hit.normal.x, 1.0);
}

#[test]
fn an_avatar_that_enters_the_scene_inside_a_collider_is_stopped_at_the_edge() {
    let mut colliders = scene(&AROUND_A_NOTCH);
    // x from 13 to 21: five metres of it over the notch, where the avatar walks through it.
    // From the scene's edge the nearest way out of the box leads on into the scene.
    add_box(
        &mut colliders,
        0,
        Vec3::new(17.0, 2.0, -8.0),
        Vec3::new(8.0, 4.0, 8.0),
    );

    // the capsule touches the scene at x = 16.35, and stops a millimetre short of that
    let hit = sweep(&mut colliders, Vec3::new(19.0, 0.0, -8.0), Vec3::NEG_X, 5.0).unwrap();
    assert_near(hit.toi, 2.649);
    assert_eq!(hit.normal, Vec3::X);
    assert!(!colliders.reaches_avatar(DVec3::new(19.0 - f64::from(hit.toi), 0.0, -8.0)));
}

#[test]
fn an_avatar_that_enters_the_scene_against_a_wall_just_past_the_edge_is_stopped_at_the_edge() {
    let mut colliders = scene(&AROUND_A_NOTCH);
    // x from 13 to 16.18: on, and 0.13 m past what the scene is drawn to
    add_box(
        &mut colliders,
        0,
        Vec3::new(14.59, 2.0, -8.0),
        Vec3::new(3.18, 4.0, 8.0),
    );

    let hit = sweep(&mut colliders, Vec3::new(20.0, 0.0, -8.0), Vec3::NEG_X, 6.0).unwrap();
    assert_near(hit.toi, 3.649);
    assert_eq!(hit.normal, Vec3::X);
    assert!(!colliders.reaches_avatar(DVec3::new(20.0 - f64::from(hit.toi), 0.0, -8.0)));
}

#[test]
fn an_avatar_stopped_at_the_edge_on_a_shallow_path_stays_a_millimetre_clear_of_the_scene() {
    let mut colliders = scene(&AROUND_A_NOTCH);
    // x from 13 to 21 and z from -15 to -1: the avatar walks inside it, along the scene's edge
    add_box(
        &mut colliders,
        0,
        Vec3::new(17.0, 2.0, -8.0),
        Vec3::new(8.0, 4.0, 14.0),
    );

    // 0.1 m towards the scene for each metre along its edge
    let from = Vec3::new(16.55, 0.0, -4.0);
    let path = Vec3::new(-0.1, 0.0, -1.0);
    let hit = sweep(&mut colliders, from, path, 5.0).unwrap();
    assert_eq!(hit.normal, Vec3::X);

    // the capsule touches the scene at x = 16.35
    let stopped = from.as_dvec3() + path.as_dvec3() * f64::from(hit.toi);
    let clear = stopped.x - 16.35;
    assert!(
        (0.0009..0.0011).contains(&clear),
        "{clear} m clear of the scene"
    );
}

#[test]
fn an_avatar_that_leaves_what_it_starts_inside_is_still_stopped_at_the_edge_of_a_scene() {
    let mut colliders = scene(&AROUND_A_NOTCH);
    // x from 13 to 21, across the edge at 16 between parcel (0, 0) and the notch
    add_box(
        &mut colliders,
        0,
        Vec3::new(17.0, 2.0, -8.0),
        Vec3::new(8.0, 4.0, 8.0),
    );
    // the sweep of a player the scene has placed inside a collider
    let mut leaving = |feet: Vec3| {
        colliders.cast_avatar_nearest_in_scene(
            feet.as_dvec3(),
            DVec3::NEG_X,
            5.0,
            MASK,
            true,
            false,
            HashSet::new(),
            -PLAYER_COLLIDER_OVERLAP,
        )
    };

    // from the notch: what it overlaps at the scene's edge is not what it started inside
    let hit = leaving(Vec3::new(19.0, 0.0, -8.0)).expect("stopped at the edge");
    assert_near(hit.toi, 2.649);
    assert_eq!(hit.normal, Vec3::X);

    // inside the scene it leaves the box it starts in
    assert!(leaving(Vec3::new(15.0, 0.0, -8.0)).is_none());
}

#[test]
fn a_sweep_that_does_not_touch_a_scene_leaves_its_bvh_unflushed() {
    let mut colliders = scene(&AROUND_A_NOTCH);
    add_slab(&mut colliders, 0);
    let waiting = colliders.changed_colliders.len();
    assert!(waiting > 0);

    // in the notch, above the slab
    assert!(sweep(&mut colliders, Vec3::new(24.0, 1.5, -4.0), Vec3::NEG_Z, 4.0).is_none());
    assert_eq!(colliders.changed_colliders.len(), waiting);

    // in the scene
    assert!(sweep(&mut colliders, Vec3::new(8.0, 1.5, -4.0), Vec3::NEG_Z, 4.0).is_none());
    assert!(colliders.changed_colliders.is_empty());
}

#[test]
fn an_avatar_that_enters_the_scene_onto_higher_ground_is_let_in_and_lifted() {
    let mut colliders = scene(&AROUND_A_NOTCH);
    // ground 0.1 m above the level the avatar walks at in the notch
    add_box(
        &mut colliders,
        0,
        Vec3::new(24.0, 0.05, -16.0),
        Vec3::new(48.0, 0.1, 32.0),
    );

    // a millimetre into the scene, for the push-out to lift it from there
    let hit = sweep(&mut colliders, Vec3::new(20.0, 0.0, -8.0), Vec3::NEG_X, 6.0).unwrap();
    assert_near(hit.toi, 3.651);
    assert_near(hit.normal.y, 1.0);

    let stopped = DVec3::new(20.0 - f64::from(hit.toi), 0.0, -8.0);
    assert!(colliders.reaches_avatar(stopped));
    let (push_min, _) = colliders.avatar_constraints(stopped);
    assert!((push_min.y - 0.1).abs() < 1e-3, "{push_min}");
}

#[test]
fn an_avatar_falling_onto_a_collider_that_tops_its_regions_height_lands_at_the_limit() {
    let mut colliders = scene(&IN_TWO_PIECES);
    // y from 18 to 26: on, under the scene's 31.7 m, and across the 20 m its region is drawn to
    add_box(
        &mut colliders,
        0,
        Vec3::new(8.0, 22.0, -8.0),
        Vec3::new(4.0, 8.0, 4.0),
    );

    let above = Vec3::new(8.0, 30.0, -8.0);
    assert_near(
        whole_sweep(&mut colliders, above, Vec3::NEG_Y, 20.0)
            .unwrap()
            .toi,
        4.0,
    );

    // the feet reach what is drawn at 20.05 m
    let hit = sweep(&mut colliders, above, Vec3::NEG_Y, 20.0).unwrap();
    assert_near(hit.toi, 9.949);
    assert_eq!(hit.normal, Vec3::Y);

    // a millimetre above what is drawn: out of the scene's reach, and on its ground
    let stopped = above.as_dvec3() - DVec3::Y * f64::from(hit.toi);
    assert!(!colliders.reaches_avatar(stopped));
    assert!(colliders.grounds_avatar(stopped));
    assert!(!colliders.grounds_avatar(stopped + DVec3::Y * 0.01));
}

#[test]
fn every_hit_of_a_sweep_is_inside_the_scene() {
    let mut colliders = scene(&AROUND_A_NOTCH);
    let post = Vec3::new(1.0, 4.0, 1.0);
    add_box(&mut colliders, 0, Vec3::new(24.0, 2.0, -8.0), post);
    let past = add_box(&mut colliders, 1, Vec3::new(40.0, 2.0, -8.0), post);

    colliders.update_bvh();
    let hits = colliders.cast_avatar_all_in_scene_flushed(
        DVec3::new(18.0, 0.0, -8.0),
        DVec3::X,
        40.0,
        MASK,
        false,
        false,
        -PLAYER_COLLIDER_OVERLAP,
    );
    assert_eq!(
        hits.iter().map(|hit| hit.id.clone()).collect::<Vec<_>>(),
        vec![past]
    );
    assert_near(hits[0].toi, 21.2);
}
