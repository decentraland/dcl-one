use std::time::Duration;

use bevy::time::{TimePlugin, TimeUpdateStrategy};

use super::*;

const NORTH: Vec2 = Vec2::new(0.0, -1.0);

#[test]
fn walks_at_the_speed_and_stops_at_the_target() {
    let from = Vec2::ZERO;
    let to = Vec2::new(0.0, -100.0);

    assert_eq!(walk_position(from, to, 6.0, 0.0), from);
    let partway = walk_position(from, to, 6.0, 10.0);
    assert!(
        partway.abs_diff_eq(Vec2::new(0.0, -60.0), 1e-4),
        "{partway}"
    );
    assert_eq!(walk_position(from, to, 6.0, 20.0), to);
    assert_eq!(walk_position(from, to, 6.0, 100.0), to);
}

#[test]
fn a_walk_to_where_it_stands_is_there_at_once() {
    let here = Vec2::new(8.0, -8.0);
    assert_eq!(walk_position(here, here, 6.0, 0.0), here);
}

#[test]
fn the_camera_faces_where_it_walks() {
    for direction in [NORTH, Vec2::X, Vec2::new(-1.0, 1.0).normalize()] {
        let forward = Quat::from_rotation_y(facing_yaw(direction)) * Vec3::NEG_Z;
        assert!(
            forward.xz().abs_diff_eq(direction, 1e-5),
            "{direction} faced {forward}"
        );
    }
}

#[test]
fn a_parcels_centre_is_where_a_teleport_lands() {
    assert_eq!(parcel_centre(IVec2::new(0, 0)), Vec2::new(8.0, -8.0));
    assert_eq!(parcel_centre(IVec2::new(0, -14)), Vec2::new(8.0, 216.0));
}

/// A run measuring from its first update, on a clock that advances a second per update, with the
/// player standing at the centre of parcel 0,-2 and the camera facing east.
fn app(phase: AutopilotPhase) -> (App, Entity, Entity) {
    let mut app = App::new();
    app.add_plugins(TimePlugin)
        .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs(1)))
        .insert_resource(phase)
        .insert_resource(AutopilotWalk {
            to: IVec2::new(0, 0),
        });
    register(&mut app);
    let start = parcel_centre(IVec2::new(0, -2));
    let player = app
        .world_mut()
        .spawn((
            PrimaryUser::default(),
            Transform::from_xyz(start.x, 3.0, start.y),
        ))
        .id();
    let camera = app
        .world_mut()
        .spawn(PrimaryCamera {
            yaw: -std::f32::consts::FRAC_PI_2,
            ..Default::default()
        })
        .id();
    (app, player, camera)
}

fn position(app: &App, player: Entity) -> Vec3 {
    app.world().get::<Transform>(player).unwrap().translation
}

#[test]
fn walks_north_into_the_target_parcel_while_measuring() {
    let (mut app, player, camera) = app(AutopilotPhase::Measuring);

    // the first update starts the walk; the clock moves a second per update after it
    app.update();
    assert_eq!(app.world().get::<PrimaryCamera>(camera).unwrap().yaw, 0.0);
    app.update();
    app.update();
    let partway = position(&app, player);
    assert!(partway.z < 24.0 && partway.z > -8.0, "{partway}");
    assert_eq!(partway.x, 8.0);
    assert_eq!(partway.y, 3.0, "the walk leaves height to the physics");

    for _ in 0..10 {
        app.update();
    }
    assert_eq!(position(&app, player), Vec3::new(8.0, 3.0, -8.0));
}

#[test]
fn stands_still_until_measuring() {
    let (mut app, player, camera) = app(AutopilotPhase::Arriving);

    for _ in 0..5 {
        app.update();
    }

    assert_eq!(position(&app, player), Vec3::new(8.0, 3.0, 24.0));
    assert_eq!(
        app.world().get::<PrimaryCamera>(camera).unwrap().yaw,
        -std::f32::consts::FRAC_PI_2
    );
}
