use bevy::math::{FloatOrd, IVec2, Vec3};

use super::*;

// the centre of parcel (0,0), at eye height
const CENTRE: Vec3 = Vec3::new(8.0, 1.5, -8.0);
const EAST: Vec3 = Vec3::X;

#[test]
fn camera_over_the_scene_is_distance_zero() {
    assert_eq!(
        scene_partition(CENTRE, EAST, [IVec2::ZERO]),
        ScenePartitionData {
            sqr_distance: 0.0,
            parcel_distance: 0,
            is_behind: false
        }
    );
}

#[test]
fn distance_is_to_the_nearest_edge_not_the_corner() {
    let partition = scene_partition(Vec3::new(-4.0, 0.0, -8.0), EAST, [IVec2::ZERO]);
    // the corner would be 4² + 8² = 80
    assert_eq!(partition.sqr_distance, 16.0);
    assert_eq!(partition.parcel_distance, 1);
    assert!(!partition.is_behind);
}

#[test]
fn behind_when_every_corner_is_behind() {
    let partition = scene_partition(Vec3::new(40.0, 0.0, -8.0), EAST, [IVec2::ZERO]);
    assert!(partition.is_behind);
    assert_eq!(partition.sqr_distance, 576.0);
    assert_eq!(partition.parcel_distance, 2);
}

#[test]
fn in_front_when_one_corner_is_in_front() {
    let partition = scene_partition(Vec3::new(12.0, 0.0, -8.0), EAST, [IVec2::ZERO]);
    assert!(!partition.is_behind);
    assert_eq!(partition.sqr_distance, 0.0);
}

#[test]
fn the_nearest_parcel_of_a_scene_wins() {
    let partition = scene_partition(
        Vec3::new(90.0, 0.0, -8.0),
        Vec3::NEG_X,
        [IVec2::ZERO, IVec2::new(5, 0)],
    );
    assert_eq!(partition.sqr_distance, 0.0);
    assert_eq!(partition.parcel_distance, 0);
    assert!(!partition.is_behind);
}

#[test]
fn parcel_distance_is_chebyshev() {
    let distance = |parcel| scene_partition(CENTRE, EAST, [parcel]).parcel_distance;
    assert_eq!(distance(IVec2::new(3, 2)), 3);
    assert_eq!(distance(IVec2::new(-2, 2)), 2);
    // parcel y grows toward -z: (0, -3) is three parcels south
    assert_eq!(distance(IVec2::new(0, -3)), 3);
}

#[test]
fn no_forward_means_nothing_is_behind() {
    // the same camera as `behind_when_every_corner_is_behind`, but no facing is known
    let partition = scene_partition(Vec3::new(40.0, 0.0, -8.0), Vec3::ZERO, [IVec2::ZERO]);
    assert!(!partition.is_behind);
    assert_eq!(partition.sqr_distance, 576.0);
}

#[test]
fn no_parcels_is_nearest_and_in_front() {
    assert_eq!(
        scene_partition(CENTRE, EAST, []),
        ScenePartitionData::NEAREST
    );
}

#[test]
fn load_priority_orders_parcel_then_facing_then_distance() {
    let key = |parcel_distance, is_behind, sqr_distance| LoadPriority {
        parcel_distance,
        is_behind,
        sqr_distance: FloatOrd(sqr_distance),
    };
    let ordered = [
        LoadPriority::TOP,
        key(0, true, 0.0),
        key(1, false, 1000.0),
        key(1, true, 0.0),
        key(2, false, 0.0),
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
        ScenePartitionData::NEAREST.load_priority(),
        LoadPriority::TOP
    );
}
