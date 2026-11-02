use super::*;
use common::partition::ScenePartitionData;

const BEHIND_FAR: ScenePartitionData = ScenePartitionData {
    sqr_distance: 6400.0,
    parcel_distance: 5,
    is_behind: true,
};
const FRONT_FAR: ScenePartitionData = ScenePartitionData {
    sqr_distance: 6400.0,
    parcel_distance: 5,
    is_behind: false,
};

#[test]
fn a_behind_scene_is_not_due_within_the_interval() {
    assert_eq!(scene_tick_key(BEHIND_FAR, false, false, 0.5), None);
}

#[test]
fn a_behind_scene_is_due_after_the_interval() {
    assert_eq!(
        scene_tick_key(BEHIND_FAR, false, false, 1.0),
        Some(FloatOrd(6400.0))
    );
}

#[test]
fn the_players_scene_is_never_capped() {
    assert_eq!(
        scene_tick_key(BEHIND_FAR, true, false, 0.016),
        Some(FloatOrd(0.0))
    );
}

#[test]
fn a_super_user_scene_is_never_capped() {
    assert_eq!(
        scene_tick_key(BEHIND_FAR, false, true, 0.016),
        Some(FloatOrd(6400.0 / 0.016f64 as f32))
    );
}

#[test]
fn a_front_scene_ages_as_before() {
    assert_eq!(
        scene_tick_key(FRONT_FAR, false, false, 0.5),
        Some(FloatOrd(12800.0))
    );
}
