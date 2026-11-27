//! The photo camera: a free camera the player flies around their avatar while in photo mode.
//! Angles are degrees, as the settings are. The follow target is the point the player steers;
//! the rendered camera lags it by `follow_damping`.

use bevy::prelude::*;
use common::structs::CameraSmoothing;

use crate::smooth_damp::{follow_damp_fraction, smooth_damp};

/// Zoom input is held for a 60 fps frame per wheel line, and per second while a zoom button is
/// held; both at `fov_change_speed` times this.
const ZOOM_SCALAR: f32 = 50.0;
const WHEEL_LINE_SECS: f32 = 1.0 / 60.0;

#[derive(Resource, Clone, Copy, Debug)]
pub struct PhotoCameraSettings {
    /// Metres from the player the camera can fly to.
    pub max_distance_from_player: f32,
    /// Metres per second.
    pub translation_speed: f32,
    pub run_speed_multiplier: f32,
    pub walk_speed_multiplier: f32,
    /// Degrees of vertical field of view per zoom unit per second.
    pub fov_change_speed: f32,
    pub fov_damping: f32,
    pub min_fov: f32,
    pub max_fov: f32,
    /// The player camera's turn per pointer delta, scaled by this.
    pub look_scale: f32,
    /// Degrees a frame may turn the camera.
    pub max_rotation_per_frame: f32,
    pub min_vertical_angle: f32,
    pub max_vertical_angle: f32,
    /// Degrees per second while a tilt key is held.
    pub tilt_speed: f32,
    pub max_tilt_per_frame: f32,
    pub max_tilt_angle: f32,
    pub tilt_damping: f32,
    /// Where the camera starts when photo mode opens from first person: behind the eye and above it.
    pub behind_direction_offset: f32,
    pub behind_up_offset: f32,
    /// Seconds for the rendered camera to close 99% of the gap to the follow target.
    pub follow_damping: f32,
    pub enter_from_first_person_secs: f32,
    pub exit_secs: f32,
    /// Radius of the sphere the camera is kept out of scene geometry with.
    pub collider_radius: f32,
}

impl Default for PhotoCameraSettings {
    fn default() -> Self {
        Self {
            max_distance_from_player: 50.0,
            translation_speed: 7.0,
            run_speed_multiplier: 2.0,
            walk_speed_multiplier: 0.5,
            fov_change_speed: 2.0,
            fov_damping: 0.5,
            min_fov: 10.0,
            max_fov: 120.0,
            look_scale: 0.17,
            max_rotation_per_frame: 10.0,
            min_vertical_angle: -90.0,
            max_vertical_angle: 90.0,
            tilt_speed: 30.0,
            max_tilt_per_frame: 30.0,
            max_tilt_angle: 360.0,
            tilt_damping: 0.1,
            behind_direction_offset: 3.0,
            behind_up_offset: 0.5,
            follow_damping: 1.0,
            enter_from_first_person_secs: 1.0,
            exit_secs: 0.5,
            collider_radius: 0.1,
        }
    }
}

impl PhotoCameraSettings {
    /// `position` kept within `max_distance_from_player` of the player: itself when it is in
    /// reach, else the nearest point that is.
    pub fn within_reach(&self, position: Vec3, player_position: Vec3) -> Vec3 {
        if position.distance(player_position) > self.max_distance_from_player {
            let direction = (position - player_position).normalize_or_zero();
            player_position + direction * self.max_distance_from_player
        } else {
            position
        }
    }
}

/// A pose to put the photo camera in at once, as the console's `/photo_mode --position` asks for
/// one.
/// A part left `None` stays as it is.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PhotoPose {
    /// Where the camera goes, in bevy's world.
    pub position: Option<Vec3>,
    /// A point to aim it at, in bevy's world; aiming also levels the roll.
    pub look_at: Option<Vec3>,
    /// The vertical field of view, in degrees.
    pub fov: Option<f32>,
}

/// One frame of player input for the photo camera.
#[derive(Default, Clone, Copy, Debug, PartialEq)]
pub struct PhotoCameraInput {
    /// x right, y forward, each in -1..=1.
    pub translation: Vec2,
    /// Up positive, -1..=1.
    pub panning: f32,
    /// Tilt right positive, -1..=1.
    pub tilting: f32,
    pub running: bool,
    pub walking: bool,
    /// Pointer motion this frame, x right, y down.
    pub aim: Vec2,
    /// The pointer button is held while the cursor is free.
    pub mouse_dragging: bool,
    /// Wheel lines turned this frame, zoom in positive.
    pub zoom_lines: f32,
    /// A held zoom button, in -1..=1, zoom in positive.
    pub zoom_held: f32,
    pub cursor_locked: bool,
}

/// How the player camera turns, which the photo camera follows.
#[derive(Clone, Copy, Debug)]
pub struct Look {
    /// The player camera's sensitivity setting.
    pub sensitivity: f32,
    pub smoothing: CameraSmoothing,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Damped {
    pub current: f32,
    pub target: f32,
    pub velocity: f32,
}

impl Damped {
    pub fn at(value: f32) -> Self {
        Self {
            current: value,
            target: value,
            velocity: 0.0,
        }
    }
}

/// How photo mode was opened, for analytics.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
pub enum PhotoModeSource {
    Shortcut,
    Button,
    Console,
}

impl std::fmt::Display for PhotoModeSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            PhotoModeSource::Shortcut => "Shortcut",
            PhotoModeSource::Button => "Button",
            PhotoModeSource::Console => "Console",
        })
    }
}

/// The primary camera's state while photo mode is on.
#[derive(Component, Clone, Debug)]
pub struct PhotoCamera {
    /// The follow target: position and orientation the player steers.
    pub target: Transform,
    pub yaw: f32,
    pub pitch: f32,
    pub roll: f32,
    pub fov: Damped,
    pub tilt: Damped,
    /// The smoothed pointer delta, the player camera's filter state.
    look_smoothed: Vec2,
    cursor_was_locked: bool,
    /// False until the opening transition lands and the follow target takes over.
    pub following: bool,
    pub hud_hidden: bool,
    pub source: PhotoModeSource,
    /// Counts photo-mode openings: a photo's late outcome applies only to the session it was shot in.
    pub session: u32,
    /// The global nametag setting photo mode opened with, restored when it closes.
    pub nametags_visible_before: bool,
}

impl PhotoCamera {
    /// A camera opening at `start` with the current vertical `fov_degrees`.
    pub fn new(start: Transform, fov_degrees: f32, source: PhotoModeSource) -> Self {
        let (yaw, pitch, roll) = start.rotation.to_euler(EulerRot::YXZ);
        Self {
            target: start,
            yaw: yaw.to_degrees(),
            pitch: pitch.to_degrees(),
            roll: roll.to_degrees(),
            fov: Damped::at(fov_degrees),
            tilt: Damped::default(),
            look_smoothed: Vec2::ZERO,
            cursor_was_locked: true,
            following: false,
            hud_hidden: false,
            source,
            session: 0,
            nametags_visible_before: true,
        }
    }

    /// Where the camera opens from `camera` (the player camera's transform): behind and above the
    /// eye when it was in first person, else where the camera already is.
    pub fn start_transform(
        camera: &Transform,
        first_person: bool,
        settings: &PhotoCameraSettings,
    ) -> Transform {
        if first_person {
            Transform {
                translation: camera.translation
                    - camera.forward() * settings.behind_direction_offset
                    + Vec3::Y * settings.behind_up_offset,
                rotation: camera.rotation,
                scale: Vec3::ONE,
            }
        } else {
            Transform {
                scale: Vec3::ONE,
                ..*camera
            }
        }
    }

    /// The follow target starts from where the camera landed after the opening transition.
    pub fn attach(&mut self, camera: &Transform) {
        self.target = Transform {
            scale: Vec3::ONE,
            ..*camera
        };
        self.following = true;
    }

    /// The translation this frame's input asks for, kept above ground level and within reach of
    /// the player. The caller tests it against scene geometry before `apply_move`.
    pub fn requested_move(
        &self,
        input: &PhotoCameraInput,
        settings: &PhotoCameraSettings,
        dt: f32,
        player_position: Vec3,
    ) -> Vec3 {
        let forward = self.target.forward() * input.translation.y;
        let horizontal = self.target.right() * input.translation.x;
        let vertical = self.target.up() * input.panning;

        let mut speed = settings.translation_speed;
        if input.running {
            speed *= settings.run_speed_multiplier;
        }
        if input.walking {
            speed *= settings.walk_speed_multiplier;
        }

        let mut movement = (forward + horizontal + vertical) * (speed * dt);

        if self.target.translation.y + movement.y <= 0.0 {
            movement.y = 0.0;
        }
        let desired = self.target.translation + movement;
        let reachable = settings.within_reach(desired, player_position);
        if reachable != desired {
            movement = reachable - self.target.translation;
        }
        movement
    }

    /// Puts the camera in `pose` at once and returns the follow target, which the caller writes
    /// to the rendered camera so that nothing is left to ease. The position is kept within reach
    /// of the player, as a flown camera's is, and the field of view within the settings' bounds.
    /// A point straight above or below the camera has no heading, so aiming at one keeps the yaw.
    pub fn place(
        &mut self,
        pose: &PhotoPose,
        settings: &PhotoCameraSettings,
        player_position: Option<Vec3>,
    ) -> Transform {
        let mut position = pose.position.unwrap_or(self.target.translation);
        if let Some(player_position) = player_position {
            position = settings.within_reach(position, player_position);
        }
        if let Some(point) = pose.look_at {
            let to_point = point - position;
            let horizontal = Vec2::new(to_point.x, to_point.z).length();
            if horizontal > 1e-4 {
                self.yaw = (-to_point.x).atan2(-to_point.z).to_degrees();
            }
            self.pitch = to_point
                .y
                .atan2(horizontal)
                .to_degrees()
                .clamp(settings.min_vertical_angle, settings.max_vertical_angle);
            self.roll = 0.0;
            self.tilt = Damped::default();
        }
        if let Some(fov) = pose.fov {
            self.fov = Damped::at(fov.clamp(settings.min_fov, settings.max_fov));
        }
        // a look still running out would turn the camera off the pose
        self.look_smoothed = Vec2::ZERO;
        self.target = Transform {
            translation: position,
            rotation: Quat::from_euler(
                EulerRot::YXZ,
                self.yaw.to_radians(),
                self.pitch.to_radians(),
                self.roll.to_radians(),
            ),
            scale: Vec3::ONE,
        };
        self.following = true;
        self.target
    }

    pub fn apply_move(&mut self, movement: Vec3) {
        self.target.translation += movement;
    }

    /// Look, tilt and zoom for one frame. The pointer looks while the cursor is locked or its
    /// button is held; otherwise the smoothed look runs out.
    pub fn steer(
        &mut self,
        input: &PhotoCameraInput,
        settings: &PhotoCameraSettings,
        look: &Look,
        dt: f32,
    ) {
        let aim = if input.cursor_locked || input.mouse_dragging {
            input.aim
        } else {
            Vec2::ZERO
        };
        self.rotate(aim, settings, look, dt);
        let reset_tilt = input.cursor_locked != self.cursor_was_locked;
        self.tilt_by(input.tilting, settings, reset_tilt, dt);
        self.zoom(input.zoom_lines, input.zoom_held, settings, dt);
        self.target.rotation = Quat::from_euler(
            EulerRot::YXZ,
            self.yaw.to_radians(),
            self.pitch.to_radians(),
            self.roll.to_radians(),
        );
    }

    fn rotate(&mut self, aim: Vec2, settings: &PhotoCameraSettings, look: &Look, dt: f32) {
        let turn = look.smoothing.smooth(aim, &mut self.look_smoothed, dt) * look.sensitivity
            / 1000.0
            * settings.look_scale;
        let max = settings.max_rotation_per_frame;
        let horizontal = turn.x.to_degrees().clamp(-max, max);
        let vertical = turn.y.to_degrees().clamp(-max, max);
        self.yaw -= horizontal;
        self.pitch =
            (self.pitch - vertical).clamp(settings.min_vertical_angle, settings.max_vertical_angle);
    }

    fn tilt_by(&mut self, tilting: f32, settings: &PhotoCameraSettings, reset: bool, dt: f32) {
        if reset {
            self.tilt = Damped::default();
            self.roll = 0.0;
            self.cursor_was_locked = !self.cursor_was_locked;
        }
        self.tilt.target = if tilting.abs() > f32::EPSILON {
            -tilting * settings.tilt_speed
        } else {
            0.0
        };
        self.tilt.current = smooth_damp(
            self.tilt.current,
            self.tilt.target,
            &mut self.tilt.velocity,
            settings.tilt_damping,
            dt,
        );
        let max = settings.max_tilt_per_frame;
        let rotation = (self.tilt.current * dt).clamp(-max, max);
        self.roll = (self.roll + rotation).clamp(-settings.max_tilt_angle, settings.max_tilt_angle);
    }

    fn zoom(&mut self, lines: f32, held: f32, settings: &PhotoCameraSettings, dt: f32) {
        let rate = settings.fov_change_speed * ZOOM_SCALAR;
        self.fov.target -= lines * rate * WHEEL_LINE_SECS + held * rate * dt;
        self.fov.target = self.fov.target.clamp(settings.min_fov, settings.max_fov);
        self.fov.current = smooth_damp(
            self.fov.current,
            self.fov.target,
            &mut self.fov.velocity,
            settings.fov_damping,
            dt,
        );
    }

    /// The rendered camera's transform this frame: lagging the follow target.
    pub fn follow(&self, camera: &Transform, settings: &PhotoCameraSettings, dt: f32) -> Transform {
        let fraction = follow_damp_fraction(settings.follow_damping, dt);
        Transform {
            translation: camera.translation.lerp(self.target.translation, fraction),
            rotation: camera.rotation.slerp(self.target.rotation, fraction),
            scale: Vec3::ONE,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DT: f32 = 1.0 / 60.0;
    const LOOK: Look = Look {
        sensitivity: 5.0,
        smoothing: CameraSmoothing::Smoothed,
    };

    fn camera() -> PhotoCamera {
        PhotoCamera::new(Transform::IDENTITY, 60.0, PhotoModeSource::Console)
    }

    #[test]
    fn opening_from_first_person_starts_behind_and_above_the_eye() {
        let settings = PhotoCameraSettings::default();
        let eye = Transform::from_xyz(10.0, 1.8, 10.0);
        let start = PhotoCamera::start_transform(&eye, true, &settings);
        assert_eq!(start.translation, Vec3::new(10.0, 2.3, 13.0));
        assert_eq!(start.rotation, eye.rotation);
        let third = PhotoCamera::start_transform(&eye, false, &settings);
        assert_eq!(third.translation, eye.translation);
    }

    #[test]
    fn forward_input_moves_along_the_target_forward_at_translation_speed() {
        let settings = PhotoCameraSettings::default();
        let mut cam = camera();
        cam.attach(&Transform::from_xyz(0.0, 5.0, 0.0));
        let input = PhotoCameraInput {
            translation: Vec2::new(0.0, 1.0),
            ..Default::default()
        };
        let movement = cam.requested_move(&input, &settings, 1.0, Vec3::ZERO);
        assert!((movement - Vec3::NEG_Z * 7.0).length() < 1e-5, "{movement}");
        let run = PhotoCameraInput {
            running: true,
            ..input
        };
        assert!((cam.requested_move(&run, &settings, 1.0, Vec3::ZERO).z + 14.0).abs() < 1e-5);
        let walk = PhotoCameraInput {
            walking: true,
            ..input
        };
        assert!((cam.requested_move(&walk, &settings, 1.0, Vec3::ZERO).z + 3.5).abs() < 1e-5);
    }

    #[test]
    fn the_camera_stays_above_ground_and_within_reach_of_the_player() {
        let settings = PhotoCameraSettings::default();
        let mut cam = camera();
        cam.attach(&Transform::from_xyz(0.0, 0.1, 0.0));
        let down = PhotoCameraInput {
            panning: -1.0,
            ..Default::default()
        };
        assert_eq!(cam.requested_move(&down, &settings, 1.0, Vec3::ZERO).y, 0.0);

        cam.attach(&Transform::from_xyz(49.0, 5.0, 0.0));
        let right = PhotoCameraInput {
            translation: Vec2::new(1.0, 0.0),
            running: true,
            ..Default::default()
        };
        let movement = cam.requested_move(&right, &settings, 1.0, Vec3::ZERO);
        let landed = cam.target.translation + movement;
        assert!((landed.length() - 50.0).abs() < 1e-3, "{landed}");
    }

    /// The angle, in degrees, between where the camera looks and the way to `point`.
    fn off_aim(camera: &Transform, point: Vec3) -> f32 {
        camera
            .forward()
            .angle_between(point - camera.translation)
            .to_degrees()
    }

    #[test]
    fn a_placed_camera_lands_aimed_with_nothing_left_to_ease() {
        let settings = PhotoCameraSettings::default();
        let mut cam = camera();
        cam.roll = 15.0;
        cam.tilt = Damped {
            current: 4.0,
            target: 30.0,
            velocity: 2.0,
        };
        cam.look_smoothed = Vec2::new(40.0, -10.0);
        let position = Vec3::new(10.0, 5.0, -10.0);
        let point = Vec3::new(10.0, 1.0, -20.0);
        let pose = PhotoPose {
            position: Some(position),
            look_at: Some(point),
            fov: Some(40.0),
        };
        let placed = cam.place(&pose, &settings, Some(Vec3::ZERO));
        assert_eq!(placed.translation, position);
        assert_eq!(placed, cam.target);
        assert!(cam.following);
        assert!(off_aim(&placed, point) < 0.01);
        assert!((cam.pitch + 21.8014).abs() < 1e-3, "{}", cam.pitch);
        assert_eq!((cam.roll, cam.tilt), (0.0, Damped::default()));
        assert_eq!(cam.fov, Damped::at(40.0));
        // a frame without input leaves it there
        let still = PhotoCameraInput {
            cursor_locked: true,
            ..Default::default()
        };
        assert_eq!(
            cam.requested_move(&still, &settings, DT, Vec3::ZERO),
            Vec3::ZERO
        );
        cam.steer(&still, &settings, &LOOK, DT);
        assert!(off_aim(&cam.target, point) < 0.01);
        assert_eq!(cam.fov.current, 40.0);
    }

    #[test]
    fn a_pose_keeps_the_parts_it_does_not_give() {
        let settings = PhotoCameraSettings::default();
        let start = Transform::from_xyz(1.0, 2.0, 3.0).with_rotation(Quat::from_rotation_y(1.0));
        let mut cam = PhotoCamera::new(start, 60.0, PhotoModeSource::Console);
        cam.roll = 15.0;

        // a position alone: the orientation, the roll and the field of view stay
        let moved = PhotoPose {
            position: Some(Vec3::ONE),
            ..Default::default()
        };
        let placed = cam.place(&moved, &settings, None);
        assert_eq!(placed.translation, Vec3::ONE);
        assert!((cam.yaw - 1f32.to_degrees()).abs() < 1e-3);
        assert_eq!((cam.roll, cam.fov.current), (15.0, 60.0));

        // a field of view alone, held to the settings' bounds
        let wide = PhotoPose {
            fov: Some(500.0),
            ..Default::default()
        };
        assert_eq!(cam.place(&wide, &settings, None).translation, Vec3::ONE);
        assert_eq!(cam.fov, Damped::at(settings.max_fov));

        // a point straight below has no heading: the yaw stays, and the camera looks down
        let down = PhotoPose {
            look_at: Some(Vec3::new(1.0, -9.0, 1.0)),
            ..Default::default()
        };
        cam.place(&down, &settings, None);
        assert!((cam.yaw - 1f32.to_degrees()).abs() < 1e-3);
        assert!((cam.pitch + 90.0).abs() < 1e-3, "{}", cam.pitch);
    }

    #[test]
    fn a_pose_out_of_reach_of_the_player_lands_at_the_nearest_point_in_reach() {
        let settings = PhotoCameraSettings::default();
        let player = Vec3::new(100.0, 0.0, 0.0);
        let mut cam = camera();
        let far = PhotoPose {
            position: Some(player + Vec3::X * 80.0),
            ..Default::default()
        };
        let placed = cam.place(&far, &settings, Some(player));
        assert!(
            (placed.translation - (player + Vec3::X * 50.0)).length() < 1e-3,
            "{placed:?}"
        );
        // where a flown camera would be held too: the next frame does not move it
        let still = PhotoCameraInput::default();
        let pulled = cam.requested_move(&still, &settings, DT, player);
        assert!(pulled.length() < 1e-3, "{pulled}");
        // in reach, and with no player to measure from, the point is taken as given
        let near = PhotoPose {
            position: Some(player + Vec3::X * 49.0),
            ..Default::default()
        };
        assert_eq!(
            cam.place(&near, &settings, Some(player)).translation,
            player + Vec3::X * 49.0
        );
        assert_eq!(
            cam.place(&far, &settings, None).translation,
            player + Vec3::X * 80.0
        );
    }

    #[test]
    fn look_input_turns_right_and_down_within_the_per_frame_cap() {
        let settings = PhotoCameraSettings::default();
        let mut cam = camera();
        let input = PhotoCameraInput {
            aim: Vec2::new(400.0, 400.0),
            cursor_locked: true,
            ..Default::default()
        };
        for _ in 0..30 {
            cam.steer(&input, &settings, &LOOK, DT);
        }
        assert!(cam.yaw < 0.0, "mouse right turns right: {}", cam.yaw);
        assert!(cam.pitch < 0.0, "mouse down looks down: {}", cam.pitch);
        assert!(cam.pitch >= settings.min_vertical_angle);
        let before = cam.yaw;
        cam.steer(&input, &settings, &LOOK, DT);
        assert!(before - cam.yaw <= settings.max_rotation_per_frame + 1e-4);
    }

    #[test]
    fn a_pointer_delta_turns_as_the_player_camera_would_times_the_look_scale() {
        let settings = PhotoCameraSettings::default();
        let mut cam = camera();
        let raw = Look {
            sensitivity: 5.0,
            smoothing: CameraSmoothing::Raw,
        };
        let input = PhotoCameraInput {
            aim: Vec2::new(100.0, 0.0),
            cursor_locked: true,
            ..Default::default()
        };
        cam.steer(&input, &settings, &raw, DT);
        // the player camera turns raw * sensitivity / 1000 radians
        let player_turn = (100.0f32 * 5.0 / 1000.0).to_degrees();
        assert!(
            (-cam.yaw - player_turn * settings.look_scale).abs() < 1e-4,
            "{}",
            cam.yaw
        );
        // the same delta at half the frame rate turns the same
        let mut slow = camera();
        slow.steer(&input, &settings, &raw, DT * 2.0);
        assert!((slow.yaw - cam.yaw).abs() < 1e-5);
    }

    #[test]
    fn a_free_cursor_only_looks_while_dragging() {
        let settings = PhotoCameraSettings::default();
        let mut cam = camera();
        let free = PhotoCameraInput {
            aim: Vec2::new(100.0, 0.0),
            ..Default::default()
        };
        cam.steer(&free, &settings, &LOOK, DT);
        cam.steer(&free, &settings, &LOOK, DT);
        assert_eq!(cam.yaw, 0.0);
        let drag = PhotoCameraInput {
            mouse_dragging: true,
            ..free
        };
        cam.steer(&drag, &settings, &LOOK, DT);
        cam.steer(&drag, &settings, &LOOK, DT);
        assert!(cam.yaw < 0.0);
    }

    #[test]
    fn tilt_springs_back_and_resets_when_the_lock_changes() {
        let settings = PhotoCameraSettings::default();
        let mut cam = camera();
        let tilt = PhotoCameraInput {
            tilting: -1.0,
            cursor_locked: true,
            ..Default::default()
        };
        for _ in 0..60 {
            cam.steer(&tilt, &settings, &LOOK, DT);
        }
        assert!(cam.roll > 1.0, "tilt left rolls positive: {}", cam.roll);
        let release = PhotoCameraInput {
            tilting: 0.0,
            ..tilt
        };
        cam.steer(&release, &settings, &LOOK, DT);
        assert!(cam.tilt.target == 0.0);
        let unlocked = PhotoCameraInput {
            cursor_locked: false,
            ..release
        };
        cam.steer(&unlocked, &settings, &LOOK, DT);
        assert_eq!(cam.roll, 0.0);
    }

    #[test]
    fn a_wheel_line_zooms_a_fixed_angle_and_a_held_button_zooms_per_second() {
        let settings = PhotoCameraSettings::default();
        let mut cam = camera();
        let line = PhotoCameraInput {
            zoom_lines: 1.0,
            ..Default::default()
        };
        cam.steer(&line, &settings, &LOOK, DT);
        let per_line = 60.0 - cam.fov.target;
        assert!((per_line - 5.0 / 3.0).abs() < 1e-4, "{per_line}");
        let mut slow = camera();
        slow.steer(&line, &settings, &LOOK, DT * 4.0);
        assert!((slow.fov.target - cam.fov.target).abs() < 1e-5);

        let mut held = camera();
        let button = PhotoCameraInput {
            zoom_held: -1.0,
            ..Default::default()
        };
        held.steer(&button, &settings, &LOOK, 0.25);
        assert!((held.fov.target - 85.0).abs() < 1e-4, "{}", held.fov.target);
    }

    #[test]
    fn zoom_narrows_the_fov_within_bounds() {
        let settings = PhotoCameraSettings::default();
        let mut cam = camera();
        let zoom_in = PhotoCameraInput {
            zoom_held: 1.0,
            cursor_locked: true,
            ..Default::default()
        };
        for _ in 0..600 {
            cam.steer(&zoom_in, &settings, &LOOK, DT);
        }
        assert!((cam.fov.target - settings.min_fov).abs() < 1e-4);
        assert!(cam.fov.current >= settings.min_fov - 1e-3);
        let zoom_out = PhotoCameraInput {
            zoom_held: -1.0,
            ..zoom_in
        };
        for _ in 0..600 {
            cam.steer(&zoom_out, &settings, &LOOK, DT);
        }
        assert!((cam.fov.target - settings.max_fov).abs() < 1e-4);
    }

    #[test]
    fn the_rendered_camera_lags_then_reaches_the_target() {
        let settings = PhotoCameraSettings::default();
        let mut cam = camera();
        cam.attach(&Transform::from_xyz(0.0, 0.0, 0.0));
        cam.apply_move(Vec3::X * 10.0);
        let mut rendered = Transform::IDENTITY;
        rendered = cam.follow(&rendered, &settings, DT);
        assert!(rendered.translation.x > 0.0 && rendered.translation.x < 10.0);
        for _ in 0..120 {
            rendered = cam.follow(&rendered, &settings, DT);
        }
        assert!(
            (rendered.translation.x - 10.0).abs() < 0.05,
            "{}",
            rendered.translation.x
        );
    }
}
