use super::*;
use dcl_component::proto_components::pulse;

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
fn jog_speed_override_moves_the_jog_tier() {
    let speeds = LocomotionSpeeds {
        jog: 12.0,
        ..DEFAULT
    };
    assert_eq!(gait_of(11.0, &speeds), Some(Gait::Jog));
    assert_eq!(gait_of(13.0, &speeds), Some(Gait::Run));
}

#[test]
fn tiers_follow_the_limits_with_a_small_tolerance() {
    assert_eq!(gait_of(2.5, &DEFAULT), Some(Gait::Walk));
    assert_eq!(gait_of(2.6, &DEFAULT), Some(Gait::Walk));
    assert_eq!(gait_of(2.7, &DEFAULT), Some(Gait::Jog));
    assert_eq!(gait_of(8.18, &DEFAULT), Some(Gait::Jog));
    assert_eq!(gait_of(8.5, &DEFAULT), Some(Gait::Jog));
    assert_eq!(gait_of(8.7, &DEFAULT), Some(Gait::Run));
    let walk_override = LocomotionSpeeds {
        walk: 4.0,
        ..DEFAULT
    };
    assert_eq!(gait_of(4.0, &walk_override), Some(Gait::Walk));
    assert_eq!(gait_of(4.1, &walk_override), Some(Gait::Walk));
    assert_eq!(gait_of(4.5, &walk_override), Some(Gait::Jog));
}

#[test]
fn mid_speed_at_default_speeds_is_a_jog() {
    assert_eq!(gait_of(5.0, &DEFAULT), Some(Gait::Jog));
}

#[test]
fn raised_blends_stay_inside_their_gait() {
    // A jog just above the walk tier and a run just above the jog tier.
    assert!((movement_blend(2.7, &DEFAULT) - 1.51).abs() < 1e-5);
    assert!((movement_blend(8.7, &DEFAULT) - 2.51).abs() < 1e-5);
    assert_eq!(movement_blend(100.0, &DEFAULT), BLEND_MAX);
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
    assert_eq!(gait_of(4.0, &DEFAULT), Some(Gait::Jog));
    assert_eq!(gait_of(7.0, &DEFAULT), Some(Gait::Jog));
    assert_eq!(gait_of(10.0, &DEFAULT), Some(Gait::Run));
}

#[test]
fn every_sent_gait_round_trips_for_every_speed() {
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
        for step in 1..=600 {
            let speed = step as f32 * 0.05;
            let expected = if speed <= speeds.walk * 1.05 {
                Gait::Walk
            } else if speed <= speeds.jog * 1.05 {
                Gait::Jog
            } else {
                Gait::Run
            };
            let blend = movement_blend(speed, &speeds);
            if blend == 0.0 {
                assert!(speed <= speeds.walk * WALK_EPSILON * 1.01);
                continue;
            }
            assert_eq!(
                gait_from_blend(blend),
                Some(expected),
                "{speed} m/s under {speeds:?}"
            );
        }
    }
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

/// The blend as a receiver sees it after the Pulse 5-bit quantizer.
fn over_the_wire(blend: f32) -> f32 {
    let state = pulse::PlayerState {
        movement_blend: pulse::PlayerState::movement_blend_quantized(blend),
        ..Default::default()
    };
    state.movement_blend_dequantized()
}

#[test]
fn every_sent_gait_survives_the_wire_quantizer() {
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
        for step in 1..=600 {
            let speed = step as f32 * 0.05;
            let sent = movement_blend(speed, &speeds);
            if sent == 0.0 {
                continue;
            }
            assert_eq!(
                gait_from_blend(over_the_wire(sent)),
                gait_from_blend(sent),
                "{speed} m/s under {speeds:?}"
            );
        }
    }
}
