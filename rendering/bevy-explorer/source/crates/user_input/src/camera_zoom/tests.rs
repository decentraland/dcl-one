use super::*;
use CameraZoomLevel::{Drone, FirstPerson, ThirdPerson};

const FOCUS: Vec3 = Vec3::new(10.0, FOCUS_HEIGHT, -4.0);
const FRAME: f32 = 1.0 / 60.0;

fn close(a: f32, b: f32, tolerance: f32) -> bool {
    (a - b).abs() <= tolerance
}

/// Compared by the dot product: the angle between two near-equal rotations is all rounding.
fn same_rotation(a: Quat, b: Quat) -> bool {
    a.dot(b).abs() > 1.0 - 1e-6
}

fn settled(level: CameraZoomLevel) -> CameraZoom {
    let mut zoom = CameraZoom::default();
    zoom.set_level(Some(level), 0.0, 0.0);
    zoom
}

/// Ticks `seconds` of frames with a free cursor.
fn run(zoom: &mut CameraZoom, seconds: f32) {
    for _ in 0..(seconds / FRAME).round() as usize {
        zoom.tick(FRAME, 0.0, CameraShoulder::Right, false);
    }
}

#[test]
fn the_wheel_steps_one_level_and_stops_at_the_ends() {
    let out = |level| step_level(level, ZoomDirection::Out, false);
    let zoom_in = |level| step_level(level, ZoomDirection::In, false);
    assert_eq!(out(FirstPerson), (ThirdPerson, true));
    assert_eq!(out(ThirdPerson), (Drone, true));
    assert_eq!(out(Drone), (Drone, false));
    assert_eq!(zoom_in(Drone), (ThirdPerson, true));
    assert_eq!(zoom_in(ThirdPerson), (FirstPerson, true));
    assert_eq!(zoom_in(FirstPerson), (FirstPerson, false));
}

#[test]
fn the_switch_key_bounces_between_first_person_and_drone() {
    let mut level = ThirdPerson;
    let mut direction = ZoomDirection::Out;
    let mut visited = vec![];
    for _ in 0..8 {
        let (next, stepped) = step_level(level, direction, true);
        level = next;
        if !stepped {
            direction = direction.reversed();
        }
        visited.push(level);
    }
    assert_eq!(
        visited,
        [
            Drone,
            ThirdPerson,
            FirstPerson,
            ThirdPerson,
            Drone,
            ThirdPerson,
            FirstPerson,
            ThirdPerson
        ]
    );
}

#[test]
fn the_orbit_passes_through_its_rings_and_follows_the_reference_curve() {
    // (orbit value, metres behind the focus, metres above it), from the reference formula
    let third_person = [
        (0.0, 0.7, -0.75),
        (0.1, 1.5248, -0.68714),
        (0.25, 2.98438, -0.36049),
        (0.5, 4.5, 1.0),
        (0.75, 3.32812, 3.24888),
        (0.9, 1.9968, 4.26886),
        (1.0, 1.2, 4.5),
    ];
    let drone = [
        (0.0, 7.0, 0.5),
        (0.1, 8.77143, 0.684),
        (0.25, 11.15804, 1.26563),
        (0.5, 12.0, 4.0),
        (0.75, 6.89554, 9.17188),
        (0.9, 2.91863, 11.54),
        (1.0, 0.8, 12.0),
    ];
    for (orbit, expected) in [(&THIRD_PERSON, third_person), (&DRONE, drone)] {
        for (value, behind, above) in expected {
            let point = orbit_point(orbit, value);
            assert!(
                close(point.x, behind, 1e-4) && close(point.y, above, 1e-4),
                "{value}: {point}"
            );
        }
    }
}

#[test]
fn pitch_maps_linearly_onto_the_orbit() {
    assert!(close(orbit_value(0.0), 0.5, 1e-6));
    assert!(close(orbit_value(-FRAC_PI_2), 1.0, 1e-6));
    assert!(close(orbit_value(FRAC_PI_2), 0.0, 1e-6));
    assert!(close(orbit_value(-45f32.to_radians()), 0.75, 1e-6));
    assert!(close(pitch_of_orbit_value(0.75), -45f32.to_radians(), 1e-6));
}

#[test]
fn each_level_has_its_field_of_view_and_pitch_range() {
    assert!(close(level_fov(FirstPerson).to_degrees(), 60.0, 1e-4));
    assert!(close(level_fov(ThirdPerson).to_degrees(), 60.0, 1e-4));
    assert!(close(level_fov(Drone).to_degrees(), 70.0, 1e-4));
    assert!(close(pitch_limit(FirstPerson).to_degrees(), 89.0, 1e-4));
    assert!(close(pitch_limit(ThirdPerson).to_degrees(), 90.0, 1e-4));
}

#[test]
fn first_person_sits_at_the_focus_and_looks_where_the_player_aims() {
    let pose = level_pose(FirstPerson, FOCUS, 0.3, -0.2, 0.0, Vec2::ZERO);
    assert_eq!(pose.position, FOCUS);
    let expected = Quat::from_euler(EulerRot::YXZ, 0.3, -0.2, 0.0);
    assert!(same_rotation(pose.rotation, expected));
}

#[test]
fn an_orbiting_camera_sits_on_its_ring_and_frames_the_focus_at_its_screen_height() {
    for (level, orbit) in [(ThirdPerson, &THIRD_PERSON), (Drone, &DRONE)] {
        for (ring, value) in [(0, 0.0), (1, 0.5), (2, 1.0)] {
            let pitch = pitch_of_orbit_value(value);
            let pose = level_pose(level, FOCUS, 0.0, pitch, 0.0, Vec2::ZERO);
            let from_focus = pose.position - FOCUS;
            assert!(close(from_focus.y, orbit.rings[ring].height, 1e-4));
            assert!(close(from_focus.z, orbit.rings[ring].radius, 1e-4));
            assert!(close(from_focus.x, 0.0, 1e-4));

            // where the focus projects: 0 at the top of the screen, 1 at the bottom
            let in_view = pose.rotation.inverse() * (FOCUS - pose.position);
            let half_height = -in_view.z * (pose.fov / 2.0).tan();
            let screen_y = 0.5 - in_view.y / half_height / 2.0;
            assert!(
                close(screen_y, orbit.screen_y[ring], 1e-4),
                "{level:?} {ring}: {screen_y}"
            );
            assert!(close(in_view.x, 0.0, 1e-4));
        }
    }
}

#[test]
fn the_heading_turns_the_orbit_around_the_focus() {
    let pose = level_pose(ThirdPerson, FOCUS, FRAC_PI_2, 0.0, 0.0, Vec2::ZERO);
    let from_focus = pose.position - FOCUS;
    // a quarter turn left puts the camera, which was behind on +z, on +x
    assert!(close(from_focus.x, 4.5, 1e-4) && close(from_focus.z, 0.0, 1e-4));
}

#[test]
fn the_shoulder_offset_shifts_the_camera_and_its_aim_together() {
    let centred = level_pose(ThirdPerson, FOCUS, 0.0, 0.0, 0.0, Vec2::ZERO);
    let shifted = level_pose(ThirdPerson, FOCUS, 0.0, 0.0, 0.0, Vec2::new(0.6, 0.0));
    assert!(close((shifted.position - centred.position).x, 0.6, 1e-4));
    assert!(same_rotation(shifted.rotation, centred.rotation));
    assert!(close((shifted.look_at.unwrap() - FOCUS).x, 0.6, 1e-4));
}

#[test]
fn the_shoulder_offset_follows_the_rings_and_the_shoulder() {
    let at = |level, value, side| shoulder_target(level, pitch_of_orbit_value(value), side);
    assert!(at(ThirdPerson, 0.0, 1.0).abs_diff_eq(Vec2::new(0.3, 0.2), 1e-5));
    assert!(at(ThirdPerson, 0.5, 1.0).abs_diff_eq(Vec2::new(0.6, 0.0), 1e-5));
    assert!(at(ThirdPerson, 1.0, 1.0).abs_diff_eq(Vec2::new(0.3, 0.7), 1e-5));
    assert!(at(ThirdPerson, 0.25, 1.0).abs_diff_eq(Vec2::new(0.45, 0.1), 1e-5));
    assert!(at(ThirdPerson, 0.5, -1.0).abs_diff_eq(Vec2::new(-0.6, 0.0), 1e-5));
    // centred keeps the height
    assert!(at(ThirdPerson, 1.0, 0.0).abs_diff_eq(Vec2::new(0.0, 0.7), 1e-5));
    assert!(at(Drone, 0.5, 1.0).abs_diff_eq(Vec2::ZERO, 1e-5));
    assert!(at(Drone, 1.0, 1.0).abs_diff_eq(Vec2::new(0.0, 2.0), 1e-5));
    assert_eq!(at(FirstPerson, 0.5, 1.0), Vec2::ZERO);
}

#[test]
fn the_shoulder_offset_moves_at_four_metres_a_second_while_the_cursor_is_locked() {
    let mut zoom = settled(ThirdPerson);
    zoom.tick(0.1, 0.0, CameraShoulder::Right, true);
    assert!(zoom
        .shoulder_offset(ThirdPerson)
        .abs_diff_eq(Vec2::new(0.4, 0.0), 1e-5));
    zoom.tick(0.1, 0.0, CameraShoulder::Right, true);
    assert!(zoom
        .shoulder_offset(ThirdPerson)
        .abs_diff_eq(Vec2::new(0.6, 0.0), 1e-5));

    // the other shoulder is 1.2 m away: 0.3 s
    for _ in 0..3 {
        zoom.tick(0.1, 0.0, CameraShoulder::Left, true);
    }
    assert!(zoom
        .shoulder_offset(ThirdPerson)
        .abs_diff_eq(Vec2::new(-0.6, 0.0), 1e-5));

    // a free cursor centres the camera
    zoom.tick(1.0, 0.0, CameraShoulder::Left, false);
    assert!(zoom
        .shoulder_offset(ThirdPerson)
        .abs_diff_eq(Vec2::ZERO, 1e-5));
    // and the drone's offset was never touched
    assert_eq!(zoom.shoulder_offset(Drone), Vec2::ZERO);
}

#[test]
fn leaving_first_person_keeps_the_aim() {
    let pitch = -30f32.to_radians();
    let (yaw, carried) = carry_view(FirstPerson, ThirdPerson, 1.2, pitch);
    assert_eq!((yaw, carried), (1.2, pitch));
    // 30 degrees down is a third of the way from the middle ring to the top
    assert!(close(orbit_value(carried), 0.5 + 30.0 / 180.0, 1e-6));
}

#[test]
fn entering_first_person_keeps_looking_where_the_third_person_camera_looked() {
    for pitch in [-1.0, -0.3, 0.0, 0.4, 1.2] {
        let third = level_pose(ThirdPerson, FOCUS, 0.7, pitch, 0.0, Vec2::ZERO);
        let (yaw, carried) = carry_view(ThirdPerson, FirstPerson, 0.7, pitch);
        let first = level_pose(FirstPerson, FOCUS, yaw, carried, 0.0, Vec2::ZERO);
        assert!(same_rotation(first.rotation, third.rotation), "{pitch}");
    }
}

#[test]
fn changing_orbit_keeps_the_cameras_direction_from_the_focus() {
    for (from, to) in [(ThirdPerson, Drone), (Drone, ThirdPerson)] {
        for pitch in [-1.2f32, -0.5, 0.0, 0.5, 1.2] {
            let (yaw, carried) = carry_view(from, to, 0.4, pitch);
            assert_eq!(yaw, 0.4);
            let before = orbit_point(orbit(from).unwrap(), orbit_value(pitch));
            let after = orbit_point(orbit(to).unwrap(), orbit_value(carried));
            // the nearest direction the other orbit has; its ends may not reach as far round
            let best = (0..=1000)
                .map(|i| angle_degrees(before, orbit_point(orbit(to).unwrap(), i as f32 / 1000.0)))
                .fold(f32::MAX, f32::min);
            assert!(
                angle_degrees(before, after) <= best + 0.5,
                "{from:?} {pitch}: {} vs {best}",
                angle_degrees(before, after)
            );
        }
    }
}

#[test]
fn a_level_change_blends_for_half_a_second_and_lands_on_the_new_level() {
    let mut zoom = settled(ThirdPerson);
    assert!(!zoom.blending());
    let start = zoom.pose(FOCUS, 0.0, 0.0, 0.0).unwrap();

    let (yaw, pitch) = zoom.set_level(Some(Drone), 0.0, 0.0).unwrap();
    assert!(zoom.blending());
    let first = zoom.pose(FOCUS, yaw, pitch, 0.0).unwrap();
    assert!(first.position.distance(start.position) < 1e-3);
    assert!(close(first.fov, start.fov, 1e-5));

    run(&mut zoom, BLEND_SECS / 2.0);
    let midway = zoom.pose(FOCUS, yaw, pitch, 0.0).unwrap();
    assert!(zoom.blending());
    assert!(midway.fov > level_fov(ThirdPerson) && midway.fov < level_fov(Drone));

    run(&mut zoom, BLEND_SECS / 2.0 + FRAME);
    assert!(!zoom.blending());
    let end = zoom.pose(FOCUS, yaw, pitch, 0.0).unwrap();
    let drone = level_pose(Drone, FOCUS, yaw, pitch, 0.0, Vec2::ZERO);
    assert!(end.position.distance(drone.position) < 1e-5);
    assert!(close(end.fov.to_degrees(), 70.0, 1e-4));
}

#[test]
fn the_blend_eases_in_and_out() {
    let blend = |elapsed| Blend {
        from: BlendSource::Level {
            level: ThirdPerson,
            yaw: 0.0,
            pitch: 0.0,
        },
        seconds: BLEND_SECS,
        elapsed,
        start: 0.0,
    };
    assert!(close(blend(0.0).weight(), 0.0, 1e-6));
    assert!(close(blend(0.125).weight(), 0.15625, 1e-6));
    assert!(close(blend(0.25).weight(), 0.5, 1e-6));
    assert!(close(blend(0.375).weight(), 0.84375, 1e-6));
    assert!(close(blend(0.5).weight(), 1.0, 1e-6));
}

#[test]
fn blending_between_orbits_swings_around_the_focus_and_keeps_it_framed() {
    let a = level_pose(ThirdPerson, FOCUS, 0.0, 0.0, 0.0, Vec2::ZERO);
    let b = level_pose(ThirdPerson, FOCUS, PI, 0.0, 0.0, Vec2::ZERO);
    let half = blend_poses(&a, &b, 0.5);
    // halfway round, not through the player
    assert!(close(
        half.position.distance(FOCUS),
        a.position.distance(FOCUS),
        1e-3
    ));
    let in_view = half.rotation.inverse() * (FOCUS - half.position);
    assert!(in_view.z < 0.0 && close(in_view.x, 0.0, 1e-3));

    assert!(same_rotation(blend_poses(&a, &b, 0.0).rotation, a.rotation));
    assert!(same_rotation(blend_poses(&a, &b, 1.0).rotation, b.rotation));
}

#[test]
fn blending_with_first_person_goes_straight() {
    let first = level_pose(FirstPerson, FOCUS, 0.0, 0.0, 0.0, Vec2::ZERO);
    let third = level_pose(ThirdPerson, FOCUS, 0.0, 0.0, 0.0, Vec2::ZERO);
    let half = blend_poses(&first, &third, 0.5);
    assert!(half
        .position
        .abs_diff_eq(first.position.lerp(third.position, 0.5), 1e-5));
    assert!(half.look_at.is_none());
}

#[test]
fn a_second_step_during_a_blend_starts_from_the_pose_shown() {
    let mut zoom = settled(FirstPerson);
    let (yaw, pitch) = zoom.set_level(Some(ThirdPerson), 0.0, 0.0).unwrap();
    run(&mut zoom, 0.1);
    let shown = zoom.pose(FOCUS, yaw, pitch, 0.0).unwrap();

    let (yaw, pitch) = zoom.set_level(Some(Drone), yaw, pitch).unwrap();
    let next = zoom.pose(FOCUS, yaw, pitch, 0.0).unwrap();
    assert!(
        next.position.distance(shown.position) < 1e-4,
        "{next:?} {shown:?}"
    );
    assert!(
        same_rotation(next.rotation, shown.rotation),
        "{next:?} {shown:?}"
    );

    run(&mut zoom, BLEND_SECS + FRAME);
    assert!(!zoom.blending());
    assert_eq!(zoom.level(), Some(Drone));
}

#[test]
fn backing_out_of_a_blend_takes_only_the_time_already_spent() {
    let mut zoom = settled(ThirdPerson);
    let (yaw, pitch) = zoom.set_level(Some(Drone), 0.3, -0.4).unwrap();
    run(&mut zoom, 0.2);
    // third person is still in the blend: it keeps the aim it was left with
    assert_eq!(
        zoom.set_level(Some(ThirdPerson), yaw, pitch),
        Some((0.3, -0.4))
    );
    run(&mut zoom, 0.2 - FRAME);
    assert!(zoom.blending());
    run(&mut zoom, 2.0 * FRAME);
    assert!(!zoom.blending());
}

#[test]
fn a_scene_camera_taking_or_returning_the_view_does_not_blend() {
    let mut zoom = settled(ThirdPerson);
    assert_eq!(zoom.set_level(None, 0.0, 0.0), None);
    assert!(!zoom.blending());
    assert!(zoom.pose(FOCUS, 0.0, 0.0, 0.0).is_none());
    assert_eq!(zoom.set_level(Some(FirstPerson), 0.0, 0.0), None);
    assert!(!zoom.blending());
    assert_eq!(zoom.level(), Some(FirstPerson));
}

#[test]
fn rotation_to_target_and_back_recovers_the_camera() {
    let pose = level_pose(ThirdPerson, FOCUS, 0.9, -0.6, 0.0, Vec2::new(0.6, 0.1));
    let look = pose.look_at.unwrap();
    let facing = Transform::IDENTITY
        .looking_to(look - pose.position, Vec3::Y)
        .rotation;
    let offset = -rotation_to_target(pose.rotation, look - pose.position, Vec3::Y);
    assert!(same_rotation(
        apply_rotation(facing, offset, Vec3::Y),
        pose.rotation
    ));
}
