use bevy::{ecs::system::SystemParam, prelude::*};
use bevy_console::ConsoleCommand;
use common::structs::{
    AvatarDynamicState, CameraView, CursorLocks, OutOfWorld, PhotoMode, PrimaryCamera, PrimaryUser,
};
use console::{
    reply, AddDebugSection, DebugInput, DebugInputKind, DebugSection, DoAddConsoleCommand,
};
use dcl_component::{proto_components::common::Vector3, transform_and_parent::DclTranslation};
use scene_runner::vec3_to_parcel;
use serde_json::{json, Value};

use crate::avatar_movement::{
    ActivePlayerComponent, AvatarLocomotionSettings, AvatarMovement, AvatarMovementInfo,
    GROUNDED_METRES,
};

pub(super) struct InputDebugPanelPlugin;

impl Plugin for InputDebugPanelPlugin {
    fn build(&self, app: &mut App) {
        app.add_console_command::<CameraState, _>(camera_state)
            .add_console_command::<MovementState, _>(movement_state)
            .add_preview_console_command::<CameraSensitivity, _>(camera_sensitivity)
            .add_preview_debug_inputs(DebugSection {
                title: "Camera",
                query: "/camera_state",
                fields: &[
                    ("Follow distance (m)", "distance"),
                    ("Far clip (m)", "far"),
                    ("Vertical FOV (degrees)", "fov"),
                    ("Scene override", "override"),
                ],
                actions: &[],
                inputs: &[DebugInput {
                    label: "Look sensitivity",
                    field: "sensitivity",
                    command: "/camera_sensitivity {value}",
                    kind: DebugInputKind::Number {
                        min: 0.01,
                        max: 100.0,
                        step: 0.01,
                    },
                }],
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

/// A perspective camera's far clip, in metres, and vertical field of view, in degrees.
fn lens(projection: &Projection) -> (Option<f32>, Option<f32>) {
    match projection {
        Projection::Perspective(p) => (Some(p.far), Some(p.fov.to_degrees())),
        _ => (None, None),
    }
}

fn camera_state(
    mut cmd: ConsoleCommand<CameraState>,
    camera: Query<(&PrimaryCamera, &Projection)>,
) {
    let Some(Ok(_)) = cmd.take() else { return };
    let Ok((camera, projection)) = camera.single() else {
        cmd.reply_failed("player camera unavailable");
        return;
    };
    let (far, fov) = lens(projection);
    cmd.reply_ok(json!({"sensitivity":camera.sensitivity, "distance":camera.distance, "far":far, "fov":fov, "override":camera.scene_override.is_some()}).to_string());
}

/// Read the active SDK movement settings and last frame of controller feedback.
#[derive(clap::Parser, ConsoleCommand)]
#[command(name = "/movement_state")]
struct MovementState;

fn movement_state(
    mut cmd: ConsoleCommand<MovementState>,
    active: Query<&ActivePlayerComponent<AvatarLocomotionSettings>, With<PrimaryUser>>,
    player: Query<(&ActivePlayerComponent<AvatarMovement>, &AvatarDynamicState), With<PrimaryUser>>,
    info: Res<AvatarMovementInfo>,
) {
    let Some(Ok(_)) = cmd.take() else { return };
    let loc = active.single().ok().map(|active| &active.component.0);
    let player = player.single().ok();
    let vector = |v: Vector3| [v.x, v.y, v.z];
    let requested = info.0.requested_velocity.or_else(|| {
        player.map(|(movement, _)| Vector3::world_vec_from_vec3(&movement.component.velocity))
    });
    // The controller skips the displacement solve when idle or movement is suppressed.
    // It explicitly sets its velocity to zero in that case, leaving SDK feedback absent.
    let actual = info.0.actual_velocity.or_else(|| {
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
        "external":info.0.external_velocity.map(vector),
        "unavailable":{
            "walk":setting_reason, "jog":setting_reason, "run":setting_reason, "jump":setting_reason,
            "runJump":setting_reason, "doubleJump":setting_reason, "landing":setting_reason,
            "requested":"No active player movement controller.",
            "actual":"No completed player movement step.",
            "external":"No SDK force or impulse in the last frame.",
        },
    }).to_string());
}

/// A bevy rotation as Decentraland euler angles, `{"x", "y", "z"}` in degrees from 0 to 360.
/// Decentraland mirrors bevy's z axis, which turns a yaw or a pitch the other way and keeps a roll.
fn dcl_euler(rotation: Quat) -> Value {
    let (yaw, pitch, roll) = rotation.to_euler(EulerRot::YXZ);
    let degrees = |radians: f32| {
        let degrees = reply::round(radians.to_degrees().rem_euclid(360.0));
        if degrees >= 360.0 {
            0.0
        } else {
            degrees
        }
    };
    json!({ "x": degrees(-pitch), "y": degrees(-yaw), "z": degrees(roll) })
}

fn mode_name(camera: &PrimaryCamera, photo: bool) -> &'static str {
    if photo {
        return "free";
    }
    match camera.view() {
        CameraView::FirstPerson => "first_person",
        CameraView::ThirdPerson => "third_person",
        CameraView::Cinematic => "cinematic",
    }
}

/// The player's and the camera's pose, read for `/player_state`.
#[derive(SystemParam)]
#[allow(clippy::type_complexity)]
pub struct PlayerPose<'w, 's> {
    player: Query<
        'w,
        's,
        (
            &'static GlobalTransform,
            Option<&'static AvatarDynamicState>,
            Has<OutOfWorld>,
        ),
        With<PrimaryUser>,
    >,
    camera: Query<
        'w,
        's,
        (
            &'static GlobalTransform,
            &'static PrimaryCamera,
            &'static Projection,
            Has<PhotoMode>,
        ),
    >,
    locks: Res<'w, CursorLocks>,
}

impl PlayerPose<'_, '_> {
    /// The pose as JSON, or `None` when there is no player: position, rotationEuler, parcel,
    /// velocity, grounded (null while out of the world), outOfWorld, pointerLocked and camera
    /// {mode, position, rotationEuler, fov, modeChangeAllowed, pointerLocked}. Positions and the
    /// velocity are in Decentraland world coordinates; euler angles are Decentraland's, in degrees
    /// from 0 to 360 (x the pitch, looking down positive; y the yaw clockwise from north; z the
    /// roll).
    pub fn read(&self) -> Option<Value> {
        let (transform, dynamics, out_of_world) = self.player.single().ok()?;
        let (_, rotation, position) = transform.to_scale_rotation_translation();
        let parcel = vec3_to_parcel(position);
        let velocity = Vector3::world_vec_from_vec3(&dynamics.map_or(Vec3::ZERO, |d| d.velocity));
        let pointer_locked = self.locks.0.contains("camera");
        let camera = self
            .camera
            .single()
            .ok()
            .map(|(transform, camera, projection, photo)| {
                let (_, rotation, position) = transform.to_scale_rotation_translation();
                json!({
                    "mode": mode_name(camera, photo),
                    "position": reply::xyz(DclTranslation::from_bevy_translation(position).0),
                    "rotationEuler": dcl_euler(rotation),
                    "fov": lens(projection).1.map(reply::round),
                    "modeChangeAllowed": camera.scene_override.is_none(),
                    "pointerLocked": pointer_locked,
                })
            });
        Some(json!({
            "position": reply::xyz(DclTranslation::from_bevy_translation(position).0),
            "rotationEuler": dcl_euler(rotation),
            "parcel": [parcel.x, parcel.y],
            "velocity": reply::xyz([velocity.x, velocity.y, velocity.z]),
            // the player is spawned with a ground height of 0, which says nothing until the
            // movement controller has placed it in the world
            "grounded": dynamics
                .filter(|_| !out_of_world)
                .map(|d| d.ground_height < GROUNDED_METRES),
            "outOfWorld": out_of_world,
            "camera": camera,
            "pointerLocked": pointer_locked,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::avatar_movement::{AvatarMovement, FromConfig};
    use bevy::ecs::system::RunSystemOnce;
    use bevy_console::{ConsoleCommandEntered, PrintConsoleLine};
    use common::structs::AvatarDynamicState;
    use std::sync::{mpsc, Arc};

    fn read(app: &mut App) -> Option<Value> {
        app.world_mut()
            .run_system_once(|pose: PlayerPose| pose.read())
            .unwrap()
    }

    #[test]
    fn stationary_controller_reports_its_velocities_when_sdk_feedback_is_omitted() {
        let mut app = App::new();
        app.add_event::<ConsoleCommandEntered>()
            .add_event::<PrintConsoleLine>()
            .init_resource::<AvatarMovementInfo>()
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

    #[test]
    fn euler_is_decentralands_yaw_from_north_and_pitch_down_positive() {
        assert_eq!(
            dcl_euler(Quat::IDENTITY),
            json!({"x": 0.0, "y": 0.0, "z": 0.0})
        );
        // a bevy yaw of +90 degrees looks west: 270 clockwise from north
        let west = Quat::from_rotation_y(90f32.to_radians());
        assert_eq!(dcl_euler(west)["y"], 270.0);
        // a bevy pitch of +30 degrees looks up: -30, so 330
        let up = Quat::from_rotation_x(30f32.to_radians());
        assert_eq!(dcl_euler(up)["x"], 330.0);
    }

    #[test]
    fn the_player_pose_reads_the_pose_the_parcel_and_the_camera() {
        let mut app = App::new();
        app.init_resource::<CursorLocks>();
        // Decentraland (24.5, 1, 40): bevy's z is mirrored
        let at = Transform::from_xyz(24.5, 1.0, -40.0);
        app.world_mut().spawn((
            PrimaryUser::default(),
            at,
            GlobalTransform::from(at),
            AvatarDynamicState {
                velocity: Vec3::new(1.0, 0.0, -2.0),
                ..default()
            },
        ));
        let report = read(&mut app).unwrap();
        assert_eq!(report["position"], json!({"x": 24.5, "y": 1.0, "z": 40.0}));
        assert_eq!(report["parcel"], json!([1, 2]));
        assert_eq!(report["velocity"], json!({"x": 1.0, "y": 0.0, "z": 2.0}));
        assert_eq!(report["grounded"], true);
        assert_eq!(report["outOfWorld"], false);
        assert!(report["camera"].is_null());
        assert_eq!(report["pointerLocked"], false);
    }

    #[test]
    fn grounded_is_unknown_while_the_player_is_out_of_the_world() {
        let mut app = App::new();
        app.init_resource::<CursorLocks>();
        assert!(read(&mut app).is_none(), "no player, no pose");
        app.world_mut().spawn((
            PrimaryUser::default(),
            GlobalTransform::IDENTITY,
            AvatarDynamicState::default(),
            OutOfWorld::default(),
        ));
        let report = read(&mut app).unwrap();
        assert!(report["grounded"].is_null());
        assert_eq!(report["outOfWorld"], true);
    }
}
