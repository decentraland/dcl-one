use bevy::{
    prelude::*,
    render::view::ViewUserValue,
    window::{CursorGrabMode, PrimaryWindow},
};

use avatar::animate::{is_emoting, ActiveEmote, MaskedEmote};
use common::{
    inputs::{Action, SystemAction, CAMERA_SET, CAMERA_ZOOM, POINTER_SET},
    structs::{
        AppConfig, AvatarDynamicState, CameraOverride, CameraShoulder, CameraView, CameraZoomLevel,
        ClientWindowFocus, CursorLocks, HeadSync, MoveKind, PhotoMode, PrimaryCamera, PrimaryUser,
        RequestClientWindowFocus, PLAYER_CAMERA_FOV,
    },
    util::ModifyComponentExt,
};
use dcl_component::proto_components::sdk::components::common::camera_transition::TransitionMode;
use input_manager::{InputManager, InputPriority};
use scene_runner::OutOfWorld;
use tween::SystemTween;

use crate::{
    camera_zoom::{pitch_limit, step_level, CameraZoom, ZoomDirection, FOCUS_HEIGHT},
    TRANSITION_TIME,
};

/// Radians a scene's cinematic camera may pitch when the scene sets no range of its own.
const CINEMATIC_PITCH_LIMIT: f32 = std::f32::consts::PI / 2.1;

/// The `CursorLocks` entry held while the pointer turns the camera.
const CAMERA_CURSOR_LOCK: &str = "camera";

/// Which way the player asked to zoom this frame. The wheel asks on every frame it turns, a key or
/// button once per press; out wins over in.
fn zoom_request(input_manager: &InputManager) -> Option<ZoomDirection> {
    let zoom_in = Action::System(SystemAction::CameraZoomIn);
    let zoom_out = Action::System(SystemAction::CameraZoomOut);
    let held = input_manager.is_button_down(zoom_in, InputPriority::Scene)
        || input_manager.is_button_down(zoom_out, InputPriority::Scene);
    let wheel = if held {
        0.0
    } else {
        input_manager
            .get_analog(CAMERA_ZOOM, InputPriority::Scene)
            .y
    };
    if wheel < 0.0 || input_manager.just_down(zoom_out, InputPriority::Scene) {
        Some(ZoomDirection::Out)
    } else if wheel > 0.0 || input_manager.just_down(zoom_in, InputPriority::Scene) {
        Some(ZoomDirection::In)
    } else {
        None
    }
}

pub struct CinematicInitialData {
    base_yaw: f32,
    base_pitch: f32,
    base_roll: f32,
}

pub fn update_camera(
    time: Res<Time>,
    mut camera: Query<(&Transform, &mut PrimaryCamera, &mut CameraZoom), Without<PhotoMode>>,
    locks: Res<CursorLocks>,
    mut cinematic_data: Local<Option<CinematicInitialData>>,
    input_manager: InputManager,
    config: Res<AppConfig>,
    mut smoothed_delta: Local<Vec2>,
) {
    let dt = time.delta_secs();

    let Ok((camera_transform, mut options, mut zoom)) = camera.single_mut() else {
        return;
    };

    if !options.initialized {
        let (yaw, pitch, roll) = camera_transform.rotation.to_euler(EulerRot::YXZ);
        options.yaw = yaw;
        options.pitch = pitch;
        options.roll = roll;
        options.initialized = true;
    }

    let mut allow_cam_move = true;

    let mut yaw_range = None;
    let mut pitch_range = None;
    let mut roll_range = None;

    // record/reset cinematic start state
    if let Some(CameraOverride::Cinematic(cine)) = options.scene_override.clone() {
        if cinematic_data.is_none() {
            *cinematic_data = Some(CinematicInitialData {
                base_yaw: options.yaw,
                base_pitch: options.pitch,
                base_roll: options.roll,
            });

            options.yaw = 0.0;
            options.pitch = 0.0;
            options.roll = 0.0;
        }

        allow_cam_move = cine.allow_manual_rotation;
        yaw_range = cine.yaw_range.map(|r| -r..r);
        pitch_range = cine.pitch_range.map(|r| -r..r);
        roll_range = cine.roll_range.map(|r| -r..r);
    } else if let Some(initial) = cinematic_data.take() {
        (options.yaw, options.pitch, options.roll) =
            (initial.base_yaw, initial.base_pitch, initial.base_roll);
    }

    let mut mouse_delta = input_manager.get_analog(CAMERA_SET, InputPriority::Scene) * 10.0;
    if locks.0.contains(CAMERA_CURSOR_LOCK) {
        mouse_delta += input_manager.get_analog(POINTER_SET, InputPriority::Scroll);
    }
    let mouse_delta = config
        .camera_smoothing
        .smooth(mouse_delta, &mut smoothed_delta, dt);

    if allow_cam_move {
        if input_manager.is_down(Action::System(SystemAction::RollLeft), InputPriority::None) {
            options.roll += dt * 1.0;
        } else if input_manager
            .is_down(Action::System(SystemAction::RollRight), InputPriority::None)
        {
            options.roll -= dt * 1.0;
        } else {
            // decay roll if not in cinematic mode
            if options.roll > 0.0 {
                options.roll = (options.roll - dt * 0.25).max(0.0);
            } else {
                options.roll = (options.roll + dt * 0.25).min(0.0);
            }
        }

        options.pitch -= mouse_delta.y * options.sensitivity / 1000.0;
        options.yaw -= mouse_delta.x * options.sensitivity / 1000.0;
    }

    // the player steps the level and swaps shoulder only while no scene holds the camera
    if options.scene_override.is_none() {
        if let Some(direction) = zoom_request(&input_manager) {
            options.level = step_level(options.level, direction, false).0;
        }
        if input_manager.just_down(SystemAction::CameraSwitchLevel, InputPriority::Scene) {
            let direction = zoom.switch_direction.unwrap_or(ZoomDirection::Out);
            let (level, stepped) = step_level(options.level, direction, true);
            options.level = level;
            if !stepped {
                zoom.switch_direction = Some(direction.reversed());
            }
        }
        if options.level != CameraZoomLevel::FirstPerson
            && input_manager.just_down(SystemAction::CameraSwapShoulder, InputPriority::Scene)
        {
            options.shoulder = match options.shoulder {
                CameraShoulder::Right => CameraShoulder::Left,
                CameraShoulder::Left => CameraShoulder::Right,
            };
        }
    }

    let level = options.level_in_effect();
    if let Some((yaw, pitch)) = zoom.set_level(level, options.yaw, options.pitch) {
        options.yaw = yaw;
        options.pitch = pitch;
    }
    let limit = level.map_or(CINEMATIC_PITCH_LIMIT, pitch_limit);
    options.pitch = options.pitch.clamp(-limit, limit);

    if let Some(roll_range) = roll_range {
        options.roll = options.roll.clamp(roll_range.start, roll_range.end);
    }
    if let Some(pitch_range) = pitch_range {
        options.pitch = options.pitch.clamp(pitch_range.start, pitch_range.end);
    }
    if let Some(yaw_range) = yaw_range {
        options.yaw = options.yaw.clamp(yaw_range.start, yaw_range.end);
    }
}

pub fn update_camera_position(
    mut commands: Commands,
    mut camera: Query<
        (
            Entity,
            &Transform,
            &PrimaryCamera,
            &mut CameraZoom,
            &mut Projection,
            Option<&mut SystemTween>,
        ),
        Without<PhotoMode>,
    >,
    mut player: Query<
        (
            &Transform,
            &AvatarDynamicState,
            Has<OutOfWorld>,
            &mut HeadSync,
            Option<(&ActiveEmote, &MaskedEmote)>,
        ),
        (With<PrimaryUser>, Without<PrimaryCamera>),
    >,
    mut prev_override: Local<Option<CameraOverride>>,
    mut prev_oow: Local<bool>,
    gt_helper: TransformHelper,
    time: Res<Time>,
    locks: Res<CursorLocks>,
) {
    let (
        Ok((player_transform, dynamic_state, is_oow, mut head_sync, emotes)),
        Ok((camera_ent, camera_transform, options, mut zoom, mut projection, maybe_tween)),
    ) = (player.single_mut(), camera.single_mut())
    else {
        return;
    };

    // Capture head-sync angles only when the real camera drives rotation (not OOW, not
    // a scene-driven cinematic). We additionally gate on the avatar being idle and not
    // emoting (in either slot — an upper-body emote keeps the idle move kind) so head
    // gaze doesn't broadcast through movement/jump/emote — matches unity's HeadIK gate.
    // In first-person the head is rigidly attached to the camera, so additional yaw IK
    // would over-rotate the neck — pitch is still meaningful (head tilts up/down).
    let real_camera =
        !is_oow && !matches!(options.scene_override, Some(CameraOverride::Cinematic(_)));
    let idle = dynamic_state.move_kind == MoveKind::Idle
        && !emotes.is_some_and(|(active, masked)| is_emoting(active, masked));
    let head_active = real_camera && idle;
    let first_person = options.view() == CameraView::FirstPerson;
    head_sync.yaw_enabled = head_active && !first_person;
    head_sync.pitch_enabled = head_active;
    if head_active {
        // DCL world is left-handed (Z+ forward) while bevy is right-handed
        // (-Z forward). The Y-up rotation handedness flips going across, so
        // a yaw of θ in bevy maps to -θ on the wire. With this, looking N is
        // 0°, E is 90°, S is 180°, W is 270° — matching unity senders.
        // Pitch is also handedness-flipped on the wire (looking up positive
        // in DCL = negative bevy pitch). HeadSync holds the wire value, and
        // the IK reader applies the same negation to render back in bevy.
        head_sync.yaw_deg = -options.yaw.to_degrees();
        head_sync.pitch_deg = -options.pitch.to_degrees();
    }

    let mut target_transform = *camera_transform;
    let mut target_fov = PLAYER_CAMERA_FOV;
    let mut target_transition = TransitionMode::Time(TRANSITION_TIME);

    if is_oow {
        target_transform = Transform::from_translation(
            player_transform.translation + Vec3::new(15.0, 75.0, 100.0),
        )
        .looking_at(player_transform.translation, Vec3::Y);
    } else if let Some(CameraOverride::Cinematic(cine)) = options.scene_override.as_ref() {
        let Ok(origin) = gt_helper.compute_global_transform(cine.origin) else {
            warn!("failed to get gt");
            return;
        };

        let (_, rotation, translation) = origin.to_scale_rotation_translation();

        target_transform.translation = translation;
        target_transform.rotation = if let Some(look_at_transform) = cine
            .look_at_entity
            .and_then(|e| gt_helper.compute_global_transform(e).ok())
        {
            Transform::IDENTITY
                .looking_at(look_at_transform.translation() - translation, Vec3::Y)
                .rotation
        } else {
            let yaw = cine
                .yaw_range
                .map(|r| options.yaw.clamp(-r, r))
                .unwrap_or(options.yaw);
            let pitch = cine
                .pitch_range
                .map(|r| options.pitch.clamp(-r, r))
                .unwrap_or(options.pitch);
            let roll = cine
                .roll_range
                .map(|r| options.roll.clamp(-r, r))
                .unwrap_or(options.roll);
            rotation * Quat::from_euler(EulerRot::YXZ, yaw, pitch, roll)
        };
        target_fov = cine.fov;
        if let Some(transition) = cine
            .transition
            .as_ref()
            .and_then(|ct| ct.transition_mode.as_ref())
        {
            target_transition = transition.clone();
        }

        commands.entity(camera_ent).try_insert(ViewUserValue(0.0));
    } else {
        zoom.tick(
            time.delta_secs(),
            options.pitch,
            options.shoulder,
            locks.0.contains(CAMERA_CURSOR_LOCK),
        );
        let focus = player_transform.translation + Vec3::Y * FOCUS_HEIGHT;
        if let Some(pose) = zoom.pose(focus, options.yaw, options.pitch, options.roll) {
            target_transform = pose.transform();
            target_fov = pose.fov;
            commands
                .entity(camera_ent)
                .try_insert(ViewUserValue((pose.position - focus).length()));
        }
    }

    // the camera never carries scale; scenes may set arbitrary scale on virtual camera entities
    target_transform.scale = Vec3::ONE;

    // a change of level blends in `CameraZoom`; only a scene camera taking or returning the view,
    // or the player leaving the world, tweens here
    let scene_camera = options
        .scene_override
        .as_ref()
        .filter(|scene_override| matches!(scene_override, CameraOverride::Cinematic(_)));
    let changed = match (prev_override.as_ref(), scene_camera) {
        (Some(prev), Some(now)) => !prev.effectively_equals(now),
        (None, None) => false,
        _ => true,
    } || *prev_oow != is_oow;

    if changed {
        debug!("changed cam to {:?}", options.scene_override);
        *prev_override = scene_camera.cloned();
        *prev_oow = is_oow;
        let time = match target_transition {
            TransitionMode::Time(t) => t,
            TransitionMode::Speed(s) => {
                let distance =
                    (target_transform.translation - camera_transform.translation).length();
                distance / s.max(0.001)
            }
        };
        debug!(
            "tween {:?} to {:?} over {time} seconds",
            camera_transform, target_transform
        );
        commands.entity(camera_ent).try_insert(SystemTween {
            target: target_transform,
            time,
            fov: Some(target_fov),
        });
    } else if let Some(mut tween) = maybe_tween {
        let tween = tween.bypass_change_detection();
        if target_transform != tween.target {
            debug!(
                "tween changed to {:?} to {:?}",
                camera_transform, target_transform
            );
            tween.target = target_transform;
        }
        if tween.fov != Some(target_fov) {
            tween.fov = Some(target_fov);
        }
    } else {
        commands
            .entity(camera_ent)
            .modify_component(move |t: &mut Transform| *t = target_transform);
        // *camera_transform = target_transform;
        if let Projection::Perspective(PerspectiveProjection { ref mut fov, .. }) = &mut *projection
        {
            if *fov != target_fov {
                *fov = target_fov;
            }
        }
    }
}

pub fn update_cursor_lock(
    mut commands: Commands,
    locks: Res<CursorLocks>,
    focus: Res<ClientWindowFocus>,
    mut windows: Query<&mut Window, With<PrimaryWindow>>,
    mut prev: Local<bool>,
) {
    let lock = !locks.0.is_empty();

    if lock {
        if *focus == ClientWindowFocus::NoFocus {
            if !*prev {
                // new right-click while not focussed - try to focus
                commands.trigger(RequestClientWindowFocus);
                // and skip updating window fields until focus is processed
                return;
            }
        } else {
            for mut window in &mut windows {
                if window.cursor_options.grab_mode == CursorGrabMode::None {
                    window.cursor_options.grab_mode = CursorGrabMode::Locked;
                    return;
                }

                if window.cursor_options.visible {
                    window.cursor_options.visible = false;
                }
            }
        }
    } else {
        for mut window in &mut windows {
            if window.cursor_options.grab_mode != CursorGrabMode::None {
                window.cursor_options.grab_mode = CursorGrabMode::None;
            }

            if !window.cursor_options.visible {
                window.cursor_options.visible = true;
            }
        }
    }

    *prev = lock;
}
