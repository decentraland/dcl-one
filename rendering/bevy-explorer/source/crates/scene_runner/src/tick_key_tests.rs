use super::*;
use crate::scene_partition::BEHIND_SCENE_TICK_RATE;
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

fn at(distance: f32, is_behind: bool) -> PartitionData {
    PartitionData {
        sqr_distance: distance * distance,
        parcel_distance: (distance / 16.0) as u16,
        is_behind,
    }
}

#[test]
fn each_distance_band_has_its_rate() {
    let rates: Vec<f32> = [0.0, 15.9, 16.0, 40.0, 50.0, 70.0, 79.9]
        .into_iter()
        .map(|distance| scene_tick_rate(&at(distance, false)))
        .collect();
    assert_eq!(rates, [40.0, 40.0, 30.0, 15.0, 10.0, 5.0, 5.0]);
}

#[test]
fn the_farthest_band_does_not_tick() {
    let partition = at(80.0, false);
    assert_eq!(scene_tick_rate(&partition), 0.0);
    assert!(!tick_rate_allows(scene_tick_rate(&partition), 3600.0));
}

#[test]
fn a_behind_scene_ticks_at_most_once_a_second() {
    assert_eq!(scene_tick_rate(&at(0.0, true)), BEHIND_SCENE_TICK_RATE);
    assert_eq!(scene_tick_rate(&at(70.0, true)), BEHIND_SCENE_TICK_RATE);
    assert_eq!(scene_tick_rate(&at(90.0, true)), 0.0);
}

#[test]
fn a_capped_scene_ticks_once_its_interval_has_passed() {
    assert!(!tick_rate_allows(5.0, 0.19));
    assert!(tick_rate_allows(5.0, 0.2));
    assert!(tick_rate_allows(5.0, 1.0));
}
