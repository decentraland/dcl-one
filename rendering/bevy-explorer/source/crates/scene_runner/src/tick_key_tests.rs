use super::*;
use common::partition::PartitionData;

const BEHIND_FAR: PartitionData = PartitionData {
    sqr_distance: 6400.0,
    parcel_distance: 5,
    is_behind: true,
};
const FRONT_FAR: PartitionData = PartitionData {
    sqr_distance: 6400.0,
    parcel_distance: 5,
    is_behind: false,
};

#[test]
fn a_front_scene_ages_as_before() {
    assert_eq!(scene_tick_key(FRONT_FAR, false, 0.5), FloatOrd(12800.0));
}

#[test]
fn a_behind_scene_counts_as_four_times_farther() {
    assert_eq!(
        scene_tick_key(BEHIND_FAR, false, 0.5),
        FloatOrd(12800.0 * BEHIND_SCENE_TICK_PENALTY)
    );
}

#[test]
fn a_behind_scene_still_rises_with_age() {
    assert!(scene_tick_key(BEHIND_FAR, false, 2.0) < scene_tick_key(BEHIND_FAR, false, 0.5));
}

#[test]
fn the_players_scene_is_always_first() {
    assert_eq!(scene_tick_key(BEHIND_FAR, true, 0.016), FloatOrd(0.0));
}
