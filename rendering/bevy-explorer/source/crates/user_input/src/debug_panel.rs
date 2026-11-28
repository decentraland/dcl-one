use bevy::prelude::*;
use bevy_console::ConsoleCommand;
use common::{
    dynamics::landed,
    structs::{
        AvatarDynamicState, CameraOverride, CameraZoomLevel, PhotoMode, PrimaryCamera, PrimaryUser,
    },
};
use console::{
    reply::refusal, AddDebugSection, DebugInput, DebugInputKind, DebugSection, DoAddConsoleCommand,
};
use dcl_component::proto_components::common::Vector3;
use scene_runner::renderer_context::RendererSceneContext;
use serde_json::json;

use crate::{
    avatar_movement::{
        ActivePlayerComponent, AvatarLocomotionSettings, AvatarMovement, LastMovementStep,
    },
    camera_collision::{CameraCollision, CameraCollisionState},
    camera_zoom::CameraZoom,
};

pub(super) struct InputDebugPanelPlugin;

impl Plugin for InputDebugPanelPlugin {
    fn build(&self, app: &mut App) {
        app.add_console_command::<CameraState, _>(camera_state)
            .add_console_command::<CameraMode, _>(camera_mode)
            .add_console_command::<MovementState, _>(movement_state)
            .add_preview_console_command::<CameraSensitivity, _>(camera_sensitivity)
            .add_preview_console_command::<CameraCollisions, _>(camera_collisions)
            .add_preview_debug_inputs(DebugSection {
                title: "Camera",
                query: "/camera_state",
                fields: &[
                    ("Zoom level", "level"),
                    ("Level in effect", "levelInEffect"),
                    ("Shoulder", "shoulder"),
                    ("Shoulder offset right / up (m)", "shoulderOffset"),
                    ("Blending between levels", "blending"),
                    ("Far clip (m)", "far"),
                    ("Vertical FOV (degrees)", "fov"),
                    ("Scene override", "override"),
                    ("Collision: desired distance (m)", "desiredDistance"),
                    ("Collision: distance (m)", "collidedDistance"),
                    ("Collision: occluded", "occluded"),
                    ("Collision: damping (s)", "dampTime"),
                ],
                actions: &[],
                inputs: &[
                    DebugInput {
                        label: "Look sensitivity",
                        field: "sensitivity",
                        command: "/camera_sensitivity {value}",
                        kind: DebugInputKind::Number {
                            min: 0.01,
                            max: 100.0,
                            step: 0.01,
                        },
                    },
                    DebugInput {
                        label: "Camera collisions",
                        field: "collisions",
                        command: "/camera_collisions {value}",
                        kind: DebugInputKind::Toggle,
                    },
                ],
            })
            .add_debug_section(DebugSection {
                title: "Locomotion",
                query: "/movement_state",
                fields: &[
                    ("Walk speed", "walk"),
                    ("Jog speed", "jog"),
                    ("Run speed", "run"),
                    ("Jump height", "jump"),
                    ("Run jump height", "runJump"),
                    ("Double jump height", "doubleJump"),
                    ("Hard landing cooldown", "landing"),
                    ("Requested velocity (X / Y / Z)", "requested"),
                    ("Actual velocity (X / Y / Z)", "actual"),
                    ("External velocity (X / Y / Z)", "external"),
                    ("On the ground", "grounded"),
                    ("Height over the ground (m)", "groundHeight"),
                ],
                inputs: &[],
                actions: &[],
            });
    }
}

/// Read the player's camera.
#[derive(clap::Parser, ConsoleCommand)]
#[command(name = "/camera_state")]
struct CameraState;

/// Switch the player's camera to first_person, third_person or drone, the levels zooming steps
/// between. Replies JSON: before, after, changed. Refused while photo mode or a scene holds the
/// camera, naming which.
#[derive(clap::Parser, ConsoleCommand)]
#[command(name = "/camera_mode")]
struct CameraMode {
    level: String,
}

fn level_name(level: CameraZoomLevel) -> &'static str {
    match level {
        CameraZoomLevel::FirstPerson => "first_person",
        CameraZoomLevel::ThirdPerson => "third_person",
        CameraZoomLevel::Drone => "drone",
    }
}

fn camera_mode(
    mut cmd: ConsoleCommand<CameraMode>,
    mut camera: Query<(&mut PrimaryCamera, Has<PhotoMode>)>,
    scenes: Query<&RendererSceneContext>,
) {
    let Some(Ok(args)) = cmd.take() else { return };
    let level = match args.level.as_str() {
        "first_person" => CameraZoomLevel::FirstPerson,
        "third_person" => CameraZoomLevel::ThirdPerson,
        "drone" => CameraZoomLevel::Drone,
        other => {
            cmd.reply_failed(format!(
                "expected first_person, third_person or drone, got {other}"
            ));
            return;
        }
    };
    let Ok((mut camera, photo)) = camera.single_mut() else {
        cmd.reply_failed(refusal("no player camera", None));
        return;
    };
    if photo {
        cmd.reply_failed(refusal(
            "photo mode holds the camera; /photo_mode off closes it",
            Some(json!({ "holder": "photo_mode" })),
        ));
        return;
    }
    if let Some(held) = &camera.scene_override {
        // the scene that set the override, as `scene_override_owner` keeps it
        let scene = camera
            .scene_override_owner
            .and_then(|owner| scenes.get(owner).ok());
        let mode = match held {
            CameraOverride::Level(level) => level_name(*level),
            CameraOverride::Cinematic(_) => "cinematic",
        };
        cmd.reply_failed(refusal(
            "a scene holds the camera",
            Some(json!({
                "holder": "scene",
                "scene": scene.map(|scene| scene.title.as_str()),
                "sceneHash": scene.map(|scene| scene.hash.as_ref()),
                "mode": mode,
            })),
        ));
        return;
    }
    let before = camera.level;
    if before != level {
        camera.level = level;
    }
    cmd.reply_ok(
        json!({
            "before": level_name(before),
            "after": level_name(level),
            "changed": before != level,
        })
        .to_string(),
    );
}

/// Set look sensitivity for the player's camera in preview.
#[derive(clap::Parser, ConsoleCommand)]
#[command(name = "/camera_sensitivity")]
struct CameraSensitivity {
    value: f32,
}

fn camera_sensitivity(
    mut cmd: ConsoleCommand<CameraSensitivity>,
    mut camera: Query<&mut PrimaryCamera>,
) {
    let Some(Ok(args)) = cmd.take() else { return };
    if !(0.01..=100.0).contains(&args.value) {
        cmd.reply_failed("sensitivity must be in 0.01..=100");
        return;
    }
    if let Ok(mut camera) = camera.single_mut() {
        camera.sensitivity = args.value;
        cmd.reply_ok(format!("camera sensitivity {}", args.value));
    } else {
        cmd.reply_failed("player camera unavailable");
    }
}

/// Turn the third-person camera's collisions on or off in preview.
#[derive(clap::Parser, ConsoleCommand)]
#[command(name = "/camera_collisions")]
struct CameraCollisions {
    #[arg(action = clap::ArgAction::Set)]
    enabled: bool,
}

fn camera_collisions(
    mut cmd: ConsoleCommand<CameraCollisions>,
    mut collision: ResMut<CameraCollision>,
) {
    let Some(Ok(args)) = cmd.take() else { return };
    collision.enabled = args.enabled;
    cmd.reply_ok(if args.enabled {
        "camera collisions on"
    } else {
        "camera collisions off"
    });
}

fn camera_state(
    mut cmd: ConsoleCommand<CameraState>,
    camera: Query<(
        &PrimaryCamera,
        &CameraZoom,
        &Projection,
        &CameraCollisionState,
    )>,
    collision: Res<CameraCollision>,
) {
    let Some(Ok(_)) = cmd.take() else { return };
    let Ok((camera, zoom, projection, state)) = camera.single() else {
        cmd.reply_failed("player camera unavailable");
        return;
    };
    let (far, fov) = match projection {
        Projection::Perspective(p) => (Some(p.far), Some(p.fov.to_degrees())),
        _ => (None, None),
    };
    let shoulder_offset = zoom
        .level()
        .map(|level| zoom.shoulder_offset(level).to_array());
    cmd.reply_ok(
        json!({
            "sensitivity": camera.sensitivity,
            "level": level_name(camera.level),
            "levelInEffect": zoom.level().map(level_name),
            "shoulder": format!("{:?}", camera.shoulder),
            "shoulderOffset": shoulder_offset,
            "blending": zoom.blending(),
            "far": far,
            "fov": fov,
            "override": camera.scene_override.is_some(),
            "collisions": collision.enabled,
            "desiredDistance": state.desired_distance,
            "collidedDistance": state.collided_distance,
            "occluded": state.occluded,
            "dampTime": state.damp_time,
        })
        .to_string(),
    );
}

/// Read the active SDK movement settings, the last movement step and the player's height over the
/// ground.
#[derive(clap::Parser, ConsoleCommand)]
#[command(name = "/movement_state")]
struct MovementState;

fn movement_state(
    mut cmd: ConsoleCommand<MovementState>,
    active: Query<&ActivePlayerComponent<AvatarLocomotionSettings>, With<PrimaryUser>>,
    player: Query<(&ActivePlayerComponent<AvatarMovement>, &AvatarDynamicState), With<PrimaryUser>>,
    step: Res<LastMovementStep>,
) {
    let Some(Ok(_)) = cmd.take() else { return };
    let ground_height = player
        .single()
        .ok()
        .map(|(_, dynamics)| dynamics.ground_height);
    let loc = active.single().ok().map(|active| &active.component.0);
    let player = player.single().ok();
    let vector = |v: Vector3| [v.x, v.y, v.z];
    let requested = step.requested_velocity.or_else(|| {
        player.map(|(movement, _)| Vector3::world_vec_from_vec3(&movement.component.velocity))
    });
    // before the first step, a stationary controller reads as zero
    let actual = step.actual_velocity.or_else(|| {
        player
            .filter(|(_, dynamics)| dynamics.velocity == Vec3::ZERO)
            .map(|(_, dynamics)| Vector3::world_vec_from_vec3(&dynamics.velocity))
    });
    let setting_reason = if loc.is_some() {
        "The active locomotion settings do not specify this value."
    } else {
        "No active locomotion settings for the player."
    };
    cmd.reply_ok(json!({
        "walk":loc.and_then(|s| s.walk_speed), "jog":loc.and_then(|s| s.jog_speed), "run":loc.and_then(|s| s.run_speed),
        "jump":loc.and_then(|s| s.jump_height), "runJump":loc.and_then(|s| s.run_jump_height),
        "doubleJump":loc.and_then(|s| s.double_jump_height), "landing":loc.and_then(|s| s.hard_landing_cooldown),
        "requested":requested.map(vector), "actual":actual.map(vector),
        "external":step.external_velocity.map(vector),
        "grounded":ground_height.map(landed),
        "groundHeight":ground_height,
        "unavailable":{
            "grounded":"No player movement state.",
            "groundHeight":"No player movement state.",
            "walk":setting_reason, "jog":setting_reason, "run":setting_reason, "jump":setting_reason,
            "runJump":setting_reason, "doubleJump":setting_reason, "landing":setting_reason,
            "requested":"No active player movement controller.",
            "actual":"No completed player movement step.",
            "external":"No SDK force or impulse in the last movement step.",
        },
    }).to_string());
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::avatar_movement::{AvatarMovement, AvatarMovementInfo, FromConfig};
    use bevy_console::{ConsoleCommandEntered, PrintConsoleLine};
    use common::{dynamics::PLAYER_LANDED_THRESHOLD, structs::AvatarDynamicState};
    use std::sync::{mpsc, Arc};

    #[test]
    fn stationary_controller_reports_its_velocities_when_sdk_feedback_is_omitted() {
        let mut app = App::new();
        app.add_event::<ConsoleCommandEntered>()
            .add_event::<PrintConsoleLine>()
            .init_resource::<LastMovementStep>()
            .add_systems(Update, movement_state);
        app.world_mut().spawn((
            PrimaryUser::default(),
            ActivePlayerComponent::<AvatarMovement>::from_config(&default()),
            AvatarDynamicState::default(),
        ));
        let (tx, rx) = mpsc::channel();
        app.world_mut().send_event(ConsoleCommandEntered {
            command_name: "/movement_state".into(),
            args: vec![],
            responder: Some(Arc::new(move |result| tx.send(result).unwrap())),
        });
        app.update();
        let reply = rx.try_recv().unwrap().unwrap();
        let report: serde_json::Value = serde_json::from_str(&reply).unwrap();
        assert_eq!(report["requested"], json!([0.0, 0.0, 0.0]));
        assert_eq!(report["actual"], json!([0.0, 0.0, 0.0]));
        assert!(report["external"].is_null());
    }

    /// Sends `/camera_mode <level>` and returns its reply.
    fn set_mode(app: &mut App, level: &str) -> Result<String, String> {
        let (tx, rx) = mpsc::channel();
        app.world_mut().send_event(ConsoleCommandEntered {
            command_name: "/camera_mode".into(),
            args: vec![level.into()],
            responder: Some(Arc::new(move |result| tx.send(result).unwrap())),
        });
        app.update();
        rx.try_recv().unwrap()
    }

    fn json_of(reply: &str) -> serde_json::Value {
        serde_json::from_str(reply).unwrap()
    }

    #[test]
    fn the_camera_mode_sets_the_zoom_level_unless_photo_mode_or_a_scene_holds_the_camera() {
        let mut app = App::new();
        app.add_event::<ConsoleCommandEntered>()
            .add_event::<PrintConsoleLine>()
            .add_systems(Update, camera_mode);
        let camera = app.world_mut().spawn(PrimaryCamera::default()).id();
        let level = |app: &App| app.world().get::<PrimaryCamera>(camera).unwrap().level;

        let reply = json_of(&set_mode(&mut app, "first_person").unwrap());
        assert_eq!(
            reply,
            json!({"before": "third_person", "after": "first_person", "changed": true})
        );
        assert_eq!(level(&app), CameraZoomLevel::FirstPerson);
        let reply = json_of(&set_mode(&mut app, "first_person").unwrap());
        assert_eq!(reply["changed"], false);
        set_mode(&mut app, "drone").unwrap();
        assert_eq!(level(&app), CameraZoomLevel::Drone);
        assert!(set_mode(&mut app, "free").is_err());

        // photo mode holds it
        app.world_mut().entity_mut(camera).insert(PhotoMode);
        let refused = json_of(&set_mode(&mut app, "third_person").unwrap_err());
        assert_eq!(refused["cause"]["holder"], "photo_mode");
        app.world_mut().entity_mut(camera).remove::<PhotoMode>();

        // a scene's camera mode area holds it: the refusal names the scene
        let scene = app
            .world_mut()
            .spawn(RendererSceneContext::new(
                dcl_types::SceneId::DUMMY,
                "bafkscene".to_owned(),
                "storage_root".to_owned(),
                false,
                0,
                "Plaza".to_owned(),
                IVec2::ZERO,
                [IVec2::ZERO].into_iter().collect(),
                vec![],
                Default::default(),
                vec![],
                Entity::PLACEHOLDER,
                0.,
                false,
                "sdk_version",
                false,
                false,
            ))
            .id();
        {
            let mut primary = app.world_mut().get_mut::<PrimaryCamera>(camera).unwrap();
            primary.scene_override = Some(CameraOverride::Level(CameraZoomLevel::FirstPerson));
            primary.scene_override_owner = Some(scene);
        }
        let refused = json_of(&set_mode(&mut app, "third_person").unwrap_err());
        assert_eq!(refused["refused"], "a scene holds the camera");
        assert_eq!(
            refused["cause"],
            json!({"holder": "scene", "scene": "Plaza", "sceneHash": "bafkscene", "mode": "first_person"})
        );
        assert_eq!(level(&app), CameraZoomLevel::Drone, "unchanged");
    }

    #[test]
    fn a_primary_camera_reports_its_collision_state() {
        let mut app = App::new();
        app.add_event::<ConsoleCommandEntered>()
            .add_event::<PrintConsoleLine>()
            .init_resource::<CameraCollision>()
            .add_systems(Update, camera_state);
        crate::require_camera_state(&mut app);
        let camera = app
            .world_mut()
            .spawn((PrimaryCamera::default(), Projection::default()))
            .id();
        app.world_mut()
            .get_mut::<CameraCollisionState>(camera)
            .expect("update_camera_position requires the state on the camera")
            .collided_distance = 2.5;
        let (tx, rx) = mpsc::channel();
        app.world_mut().send_event(ConsoleCommandEntered {
            command_name: "/camera_state".into(),
            args: vec![],
            responder: Some(Arc::new(move |result| tx.send(result).unwrap())),
        });
        app.update();
        let reply = rx.try_recv().unwrap().unwrap();
        let report: serde_json::Value = serde_json::from_str(&reply).unwrap();
        assert_eq!(report["collisions"], json!(true));
        assert_eq!(report["desiredDistance"], json!(0.0));
        assert_eq!(report["collidedDistance"], json!(2.5));
        assert_eq!(report["occluded"], json!(false));
        assert_eq!(report["dampTime"], json!(0.0));
    }

    #[test]
    fn the_player_is_on_the_ground_within_the_landed_height() {
        for (ground_height, grounded) in [(0.1, true), (PLAYER_LANDED_THRESHOLD, false)] {
            let mut app = App::new();
            app.add_event::<ConsoleCommandEntered>()
                .add_event::<PrintConsoleLine>()
                .init_resource::<LastMovementStep>()
                .add_systems(Update, movement_state);
            app.world_mut().spawn((
                PrimaryUser::default(),
                ActivePlayerComponent::<AvatarMovement>::from_config(&default()),
                AvatarDynamicState {
                    ground_height,
                    ..default()
                },
            ));
            let (tx, rx) = mpsc::channel();
            app.world_mut().send_event(ConsoleCommandEntered {
                command_name: "/movement_state".into(),
                args: vec![],
                responder: Some(Arc::new(move |result| tx.send(result).unwrap())),
            });
            app.update();
            let reply = rx.try_recv().unwrap().unwrap();
            let report: serde_json::Value = serde_json::from_str(&reply).unwrap();
            assert_eq!(report["grounded"], grounded, "{ground_height}");
        }
    }

    #[test]
    fn a_moving_step_reports_its_actual_velocity_after_the_next_broadcast() {
        use crate::avatar_movement::{apply_movement, CentralCollisions, GroundCollider};
        use bevy::time::{TimePlugin, TimeUpdateStrategy};
        use common::structs::EngineMovementControl;
        use scene_runner::initialize_scene::{LiveScenes, PortableScenes, ScenePointers};

        let mut app = App::new();
        app.add_plugins(TimePlugin)
            .insert_resource(TimeUpdateStrategy::ManualDuration(
                std::time::Duration::from_millis(20),
            ))
            .add_event::<ConsoleCommandEntered>()
            .add_event::<PrintConsoleLine>()
            .init_resource::<AvatarMovementInfo>()
            .init_resource::<LastMovementStep>()
            .init_resource::<CentralCollisions>()
            .init_resource::<ScenePointers>()
            .init_resource::<LiveScenes>()
            .init_resource::<PortableScenes>()
            .insert_resource(EngineMovementControl {
                // accept the test's movement, initiated at time zero
                accept_movement_after: -1.0,
                ..default()
            })
            // the broadcast to the scenes clears the movement info
            .add_systems(
                Update,
                (
                    |mut info: ResMut<AvatarMovementInfo>| info.0 = default(),
                    movement_state,
                )
                    .chain(),
            )
            .add_systems(PostUpdate, apply_movement);
        let mut movement = ActivePlayerComponent::<AvatarMovement>::from_config(&default());
        // 2 m/s east, 1 m/s north: bevy's z points south
        movement.component.velocity = Vec3::new(2.0, 0.0, -1.0);
        app.world_mut().spawn((
            PrimaryUser::default(),
            movement,
            AvatarDynamicState::default(),
            GroundCollider::default(),
            Transform::from_xyz(0.0, 1.0, 0.0),
        ));
        app.update();
        app.update();

        let (tx, rx) = mpsc::channel();
        app.world_mut().send_event(ConsoleCommandEntered {
            command_name: "/movement_state".into(),
            args: vec![],
            responder: Some(Arc::new(move |result| tx.send(result).unwrap())),
        });
        app.update();
        let reply = rx.try_recv().unwrap().unwrap();
        let report: serde_json::Value = serde_json::from_str(&reply).unwrap();
        let actual: Vec<f64> = serde_json::from_value(report["actual"].clone()).unwrap();
        assert!((actual[0] - 2.0).abs() < 1e-3, "{report}");
        assert!(actual[1].abs() < 1e-3, "{report}");
        assert!((actual[2] - 1.0).abs() < 1e-3, "{report}");
        assert_eq!(report["requested"], json!([2.0, 0.0, 1.0]));
        // over no collider: in the air
        assert_eq!(report["grounded"], false);
    }
}
