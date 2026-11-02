use bevy::math::{FloatOrd, IVec2, Vec3};

use super::*;

// the centre of parcel (0,0), at eye height
const CENTRE: Vec3 = Vec3::new(8.0, 1.5, -8.0);
const EAST: Vec3 = Vec3::X;

/// A camera at `position` looking along `forward`.
fn camera(position: Vec3, forward: Vec3) -> Option<(Vec3, Vec3)> {
    Some((position, forward))
}

#[test]
fn origin_over_the_scene_is_distance_zero() {
    assert_eq!(
        scene_partition(CENTRE, camera(CENTRE, EAST), [IVec2::ZERO]),
        PartitionData {
            sqr_distance: 0.0,
            parcel_distance: 0,
            is_behind: false
        }
    );
}

#[test]
fn distance_is_to_the_nearest_edge_not_the_corner() {
    let origin = Vec3::new(-4.0, 0.0, -8.0);
    let partition = scene_partition(origin, camera(origin, EAST), [IVec2::ZERO]);
    // the corner would be 4² + 8² = 80
    assert_eq!(partition.sqr_distance, 16.0);
    assert_eq!(partition.parcel_distance, 1);
    assert!(!partition.is_behind);
}

#[test]
fn behind_when_every_corner_is_behind_the_camera() {
    let origin = Vec3::new(40.0, 0.0, -8.0);
    let partition = scene_partition(origin, camera(origin, EAST), [IVec2::ZERO]);
    assert!(partition.is_behind);
    assert_eq!(partition.sqr_distance, 576.0);
    assert_eq!(partition.parcel_distance, 2);
}

#[test]
fn in_front_when_one_corner_is_in_front() {
    let origin = Vec3::new(12.0, 0.0, -8.0);
    let partition = scene_partition(origin, camera(origin, EAST), [IVec2::ZERO]);
    assert!(!partition.is_behind);
    assert_eq!(partition.sqr_distance, 0.0);
}

#[test]
fn distance_from_the_origin_facing_from_the_camera() {
    // the player stands on the parcel while a third-person camera 32 m east looks away from it
    let partition = scene_partition(
        CENTRE,
        camera(Vec3::new(40.0, 0.0, -8.0), EAST),
        [IVec2::ZERO],
    );
    assert_eq!(partition.sqr_distance, 0.0);
    assert_eq!(partition.parcel_distance, 0);
    assert!(partition.is_behind);
}

#[test]
fn the_nearest_parcel_of_a_scene_wins() {
    let origin = Vec3::new(90.0, 0.0, -8.0);
    let partition = scene_partition(
        origin,
        camera(origin, Vec3::NEG_X),
        [IVec2::ZERO, IVec2::new(5, 0)],
    );
    assert_eq!(partition.sqr_distance, 0.0);
    assert_eq!(partition.parcel_distance, 0);
    assert!(!partition.is_behind);
}

#[test]
fn parcel_distance_is_chebyshev() {
    let distance = |parcel| scene_partition(CENTRE, camera(CENTRE, EAST), [parcel]).parcel_distance;
    assert_eq!(distance(IVec2::new(3, 2)), 3);
    assert_eq!(distance(IVec2::new(-2, 2)), 2);
    // parcel y grows toward -z: (0, -3) is three parcels south
    assert_eq!(distance(IVec2::new(0, -3)), 3);
}

#[test]
fn no_camera_means_nothing_is_behind() {
    // the same geometry as `behind_when_every_corner_is_behind_the_camera`, with no camera
    let partition = scene_partition(Vec3::new(40.0, 0.0, -8.0), None, [IVec2::ZERO]);
    assert!(!partition.is_behind);
    assert_eq!(partition.sqr_distance, 576.0);
}

#[test]
fn no_parcels_is_nearest_and_in_front() {
    assert_eq!(
        scene_partition(CENTRE, camera(CENTRE, EAST), []),
        PartitionData::NEAREST
    );
}

#[test]
fn a_point_at_the_origin_is_nearest_and_in_front() {
    assert_eq!(
        point_partition(CENTRE, camera(CENTRE, EAST), CENTRE),
        PartitionData::NEAREST
    );
}

#[test]
fn a_point_is_measured_from_the_origin_and_ringed_by_parcel() {
    let partition = point_partition(CENTRE, camera(CENTRE, EAST), Vec3::new(40.0, 0.0, -8.0));
    assert_eq!(partition.sqr_distance, 1024.0);
    assert_eq!(partition.parcel_distance, 2);
    assert!(!partition.is_behind);
}

#[test]
fn a_point_behind_the_camera_is_behind() {
    let partition = point_partition(CENTRE, camera(CENTRE, EAST), Vec3::new(-24.0, 0.0, -8.0));
    assert!(partition.is_behind);
    assert_eq!(partition.parcel_distance, 2);
}

#[test]
fn a_point_with_no_camera_is_never_behind() {
    assert!(!point_partition(CENTRE, None, Vec3::new(-24.0, 0.0, -8.0)).is_behind);
}

#[test]
fn load_priority_orders_tier_then_parcel_then_facing_then_distance() {
    let key = |tier, parcel_distance, is_behind, sqr_distance| LoadPriority {
        tier,
        parcel_distance,
        is_behind,
        sqr_distance: FloatOrd(sqr_distance),
    };
    let ordered = [
        LoadPriority::TOP,
        // every avatar before every other scene, nearest avatar first
        key(Tier::Avatar, 0, false, 0.0),
        key(Tier::Avatar, 5, true, 6400.0),
        key(Tier::Scene, 0, true, 0.0),
        key(Tier::Scene, 1, false, 1000.0),
        key(Tier::Scene, 1, true, 0.0),
        key(Tier::Scene, 2, false, 0.0),
        key(Tier::Scene, 300, true, f32::MAX),
        LoadPriority::OTHER,
        LoadPriority::BOTTOM,
    ];
    for pair in ordered.windows(2) {
        assert!(
            pair[0] < pair[1],
            "{:?} should sort before {:?}",
            pair[0],
            pair[1]
        );
    }
    assert_eq!(
        PartitionData::NEAREST.load_priority(Tier::Active),
        LoadPriority::TOP
    );
    assert_eq!(
        PartitionData::NEAREST.load_priority(Tier::Scene),
        key(Tier::Scene, 0, false, 0.0)
    );
}

#[test]
fn resolved_hashes_are_collected_once_each_and_sorted() {
    let urns = ["urn:b", "urn:a", "urn:b", "urn:c"];
    let hashes = resolved_hashes(urns, |urn| match urn {
        "urn:a" => Some("hash-a".to_owned()),
        "urn:b" => Some("hash-b".to_owned()),
        // still resolving: no hash yet
        _ => None,
    });
    assert_eq!(hashes, vec!["hash-a".to_owned(), "hash-b".to_owned()]);
}

#[test]
fn nothing_resolved_is_an_empty_list() {
    assert!(resolved_hashes(["urn:a"], |_| None).is_empty());
}
