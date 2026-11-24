use super::*;
use crate::scene_partition::{BEHIND_SCENE_TICK_RATE, FAR_SCENE_TICK_RATE};
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
fn the_farthest_band_keeps_a_slow_tick() {
    assert_eq!(scene_tick_rate(&at(80.0, false)), FAR_SCENE_TICK_RATE);
    assert_eq!(scene_tick_rate(&at(5000.0, true)), FAR_SCENE_TICK_RATE);
}

#[test]
fn a_behind_scene_ticks_at_most_once_a_second() {
    assert_eq!(scene_tick_rate(&at(0.0, true)), BEHIND_SCENE_TICK_RATE);
    assert_eq!(scene_tick_rate(&at(70.0, true)), BEHIND_SCENE_TICK_RATE);
}

#[test]
fn a_capped_scene_ticks_once_its_interval_has_passed() {
    assert!(!tick_rate_allows(5.0, 0.15, 0.016));
    assert!(tick_rate_allows(5.0, 0.2, 0.016));
    assert!(tick_rate_allows(5.0, 1.0, 0.016));
}

#[test]
fn an_interval_within_half_a_frame_counts_as_passed() {
    // two 60 fps frames that came in just under 1/30 s
    assert!(tick_rate_allows(30.0, 0.0332, 1.0 / 60.0));
    assert!(!tick_rate_allows(30.0, 0.0166, 1.0 / 60.0));
}
