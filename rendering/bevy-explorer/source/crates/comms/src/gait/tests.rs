use super::*;

const DEFAULT: LocomotionSpeeds = LocomotionSpeeds {
    walk: 2.5,
    jog: 8.18,
    run: 11.0,
};

fn near(actual: f32, expected: f32) -> bool {
    (actual - expected).abs() < 1e-4
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
fn blend_scales_within_the_walk_tier() {
    assert_eq!(movement_blend(1.25, &DEFAULT), 0.5);
}

#[test]
fn blend_is_not_speed_squared() {
    // A 2.5 m/s walk used to go out as 6.25, clamped to 3 on the wire.
    assert_eq!(movement_blend(2.5, &DEFAULT), 1.0);
}

#[test]
fn blend_clamps_above_the_run_speed() {
    assert_eq!(movement_blend(30.0, &DEFAULT), BLEND_MAX);
    assert_eq!(movement_blend(f32::INFINITY, &DEFAULT), BLEND_MAX);
}

#[test]
fn tiers_send_a_blend_inside_their_own_range() {
    // Walk tier, up to the walk limit and its tolerance.
    assert!(near(movement_blend(2.6, &DEFAULT), 1.0));
    // Jog tier: a slow jog is not raised above the walk blend.
    assert!(near(movement_blend(4.0, &DEFAULT), 1.0));
    assert!(near(movement_blend(6.135, &DEFAULT), 1.5));
    assert!(near(movement_blend(8.5, &DEFAULT), 2.0));
    // Run tier.
    assert!(near(movement_blend(9.0, &DEFAULT), 3.0 * 9.0 / 11.0));
    assert!(near(movement_blend(11.0, &DEFAULT), 3.0));
}

#[test]
fn walk_speed_override_moves_the_walk_tier() {
    let speeds = LocomotionSpeeds {
        walk: 4.0,
        ..DEFAULT
    };
    assert_eq!(movement_blend(4.0, &speeds), 1.0);
    assert_eq!(movement_blend(2.0, &speeds), 0.5);
    assert!(near(movement_blend(4.1, &speeds), 1.0));
    assert!(near(movement_blend(6.0, &speeds), 2.0 * 6.0 / 8.18));
}

#[test]
fn jog_speed_override_moves_the_jog_tier() {
    let speeds = LocomotionSpeeds {
        jog: 12.0,
        ..DEFAULT
    };
    assert!(near(movement_blend(11.0, &speeds), 2.0 * 11.0 / 12.0));
    assert_eq!(movement_blend(13.0, &speeds), BLEND_MAX);
}

#[test]
fn degenerate_speeds_stay_on_the_scale() {
    let all_equal = LocomotionSpeeds {
        walk: 5.0,
        jog: 5.0,
        run: 5.0,
    };
    assert_eq!(movement_blend(5.0, &all_equal), 1.0);
    let inverted = LocomotionSpeeds {
        walk: 5.0,
        jog: 3.0,
        run: 2.0,
    };
    assert_eq!(movement_blend(5.0, &inverted), 1.0);
    let zero = LocomotionSpeeds {
        walk: 0.0,
        jog: 0.0,
        run: 0.0,
    };
    let blend = movement_blend(4.0, &zero);
    assert!((0.0..=BLEND_MAX).contains(&blend));
}

#[test]
fn blend_never_falls_as_speed_rises() {
    let sets = [
        DEFAULT,
        LocomotionSpeeds {
            walk: 4.0,
            ..DEFAULT
        },
        LocomotionSpeeds {
            jog: 12.0,
            run: 20.0,
            ..DEFAULT
        },
    ];
    for speeds in sets {
        let mut previous = 0.0;
        for step in 0..=600 {
            let speed = step as f32 * 0.05;
            let blend = movement_blend(speed, &speeds);
            assert!(blend >= previous, "blend fell at {speed} m/s, {speeds:?}");
            assert!((0.0..=BLEND_MAX).contains(&blend));
            previous = blend;
        }
    }
}

#[test]
fn the_walk_and_jog_tiers_join_without_a_step() {
    let edge = DEFAULT.walk * TIER_TOLERANCE;
    let below = movement_blend(edge - 0.001, &DEFAULT);
    let above = movement_blend(edge + 0.001, &DEFAULT);
    assert!((above - below).abs() < 0.01);
}

#[test]
fn the_jog_and_run_tiers_join_with_a_bounded_step() {
    // The run blend starts at its own fraction of the run limit, so the edge is a step of the
    // difference between the two limits' fractions, not a jump across a gait.
    let edge = DEFAULT.jog * TIER_TOLERANCE;
    let below = movement_blend(edge - 0.001, &DEFAULT);
    let above = movement_blend(edge + 0.001, &DEFAULT);
    assert!(above >= below);
    assert!(above - below < 0.4);
}

#[test]
fn falling_needs_air_and_a_fast_enough_descent() {
    assert!(is_falling(false, -3.5));
    assert!(!is_falling(false, -3.0));
    assert!(!is_falling(false, -2.0));
    assert!(!is_falling(false, 4.0));
    assert!(!is_falling(true, -9.0));
}
