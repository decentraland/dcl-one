use super::*;

const DEFAULT: LocomotionSpeeds = LocomotionSpeeds {
    walk: 2.5,
    jog: 8.18,
    run: 11.0,
};

fn gait_of(speed: f32, speeds: &LocomotionSpeeds) -> Option<Gait> {
    gait_from_blend(movement_blend(speed, speeds))
}

#[test]
fn blend_is_zero_at_rest() {
    assert_eq!(movement_blend(0.0, &DEFAULT), 0.0);
    assert_eq!(movement_blend(0.1, &DEFAULT), 0.0);
    assert_eq!(movement_blend(f32::NAN, &DEFAULT), 0.0);
    assert_eq!(movement_blend(-3.0, &DEFAULT), 0.0);
}

#[test]
fn blend_reaches_each_gait_index_at_its_speed() {
    assert_eq!(movement_blend(2.5, &DEFAULT), 1.0);
    assert_eq!(movement_blend(8.18, &DEFAULT), 2.0);
    assert_eq!(movement_blend(11.0, &DEFAULT), 3.0);
}

#[test]
fn blend_scales_within_a_gait() {
    assert_eq!(movement_blend(1.25, &DEFAULT), 0.5);
    assert_eq!(gait_of(1.25, &DEFAULT), Some(Gait::Walk));
}

#[test]
fn blend_clamps_above_the_run_speed() {
    assert_eq!(movement_blend(30.0, &DEFAULT), BLEND_MAX);
    assert_eq!(movement_blend(f32::INFINITY, &DEFAULT), BLEND_MAX);
}

#[test]
fn blend_is_not_speed_squared() {
    // A 2.5 m/s walk used to go out as 6.25, clamped to 3 on the wire.
    assert_eq!(gait_of(2.5, &DEFAULT), Some(Gait::Walk));
}

#[test]
fn walk_speed_override_stays_a_walk() {
    let speeds = LocomotionSpeeds {
        walk: 5.0,
        ..DEFAULT
    };
    assert_eq!(movement_blend(5.0, &speeds), 1.0);
    assert_eq!(gait_of(5.0, &speeds), Some(Gait::Walk));
}

#[test]
fn run_speed_override_moves_the_run_threshold() {
    let speeds = LocomotionSpeeds {
        run: 20.0,
        ..DEFAULT
    };
    assert_eq!(gait_of(11.0, &speeds), Some(Gait::Jog));
    assert_eq!(gait_of(20.0, &speeds), Some(Gait::Run));
}

#[test]
fn degenerate_speeds_collapse_to_the_lower_gaits() {
    let all_equal = LocomotionSpeeds {
        walk: 5.0,
        jog: 5.0,
        run: 5.0,
    };
    assert_eq!(gait_of(5.0, &all_equal), Some(Gait::Walk));
    let inverted = LocomotionSpeeds {
        walk: 5.0,
        jog: 3.0,
        run: 2.0,
    };
    assert_eq!(gait_of(5.0, &inverted), Some(Gait::Walk));
    let zero = LocomotionSpeeds {
        walk: 0.0,
        jog: 0.0,
        run: 0.0,
    };
    let blend = movement_blend(4.0, &zero);
    assert!((0.0..=BLEND_MAX).contains(&blend));
}

#[test]
fn blend_rises_with_speed_and_round_trips_its_gait() {
    let mut previous = 0.0;
    for step in 0..=300 {
        let speed = step as f32 * 0.05;
        let blend = movement_blend(speed, &DEFAULT);
        assert!(blend >= previous, "blend fell at {speed} m/s");
        assert!((0.0..=BLEND_MAX).contains(&blend));
        previous = blend;
    }
    assert_eq!(gait_of(4.0, &DEFAULT), Some(Gait::Walk));
    assert_eq!(gait_of(7.0, &DEFAULT), Some(Gait::Jog));
    assert_eq!(gait_of(10.0, &DEFAULT), Some(Gait::Run));
}

#[test]
fn falling_needs_air_and_descent() {
    assert!(is_falling(false, -3.0));
    assert!(!is_falling(false, 4.0));
    assert!(!is_falling(false, 0.0));
    assert!(!is_falling(true, -3.0));
}

#[test]
fn received_blend_names_a_gait() {
    assert_eq!(gait_from_blend(0.0), Some(Gait::Idle));
    assert_eq!(gait_from_blend(0.05), Some(Gait::Idle));
    assert_eq!(gait_from_blend(0.06), Some(Gait::Walk));
    assert_eq!(gait_from_blend(0.4), Some(Gait::Walk));
    assert_eq!(gait_from_blend(1.0), Some(Gait::Walk));
    assert_eq!(gait_from_blend(1.4), Some(Gait::Walk));
    assert_eq!(gait_from_blend(1.6), Some(Gait::Jog));
    assert_eq!(gait_from_blend(2.6), Some(Gait::Run));
    assert_eq!(gait_from_blend(3.0), Some(Gait::Run));
}

#[test]
fn implausible_blend_names_no_gait() {
    assert_eq!(gait_from_blend(f32::NAN), None);
    assert_eq!(gait_from_blend(f32::INFINITY), None);
    assert_eq!(gait_from_blend(-0.5), None);
    // Speed squared from a sender that does not clamp.
    assert_eq!(gait_from_blend(6.25), None);
}

#[test]
fn fallback_follows_the_received_walk() {
    assert_eq!(fallback_gait(Some(Gait::Walk), 5.0), FallbackGait::Walk);
    assert_eq!(fallback_gait(Some(Gait::Walk), 1.0), FallbackGait::Walk);
}

#[test]
fn fallback_follows_a_received_jog_or_run_above_the_walk_clip_speed() {
    assert_eq!(fallback_gait(Some(Gait::Jog), 3.0), FallbackGait::Run);
    assert_eq!(fallback_gait(Some(Gait::Run), 8.0), FallbackGait::Run);
}

#[test]
fn fallback_distrusts_a_run_at_walking_speed() {
    // An old sender's 2.5 m/s walk arrives as blend 3.
    assert_eq!(fallback_gait(Some(Gait::Run), 2.5), FallbackGait::Walk);
    assert_eq!(fallback_gait(Some(Gait::Jog), 1.0), FallbackGait::Walk);
}

#[test]
fn fallback_uses_velocity_without_a_usable_gait() {
    assert_eq!(fallback_gait(None, 2.0), FallbackGait::Walk);
    assert_eq!(fallback_gait(None, 3.0), FallbackGait::Run);
    assert_eq!(fallback_gait(Some(Gait::Idle), 2.0), FallbackGait::Walk);
    assert_eq!(fallback_gait(Some(Gait::Idle), 3.0), FallbackGait::Run);
}
