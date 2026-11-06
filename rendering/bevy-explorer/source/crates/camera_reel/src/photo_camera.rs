//! The photo camera: a free camera the player flies around their avatar while in photo mode.
//! Angles are degrees, as the settings are. The follow target is the point the player steers;
//! the rendered camera lags it by `follow_damping`.

use bevy::prelude::*;

use crate::smooth_damp::{follow_damp_fraction, smooth_damp, smooth_damp_vec2};

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
    /// Degrees per second per unit of look input.
    pub rotation_speed: f32,
    /// Degrees a frame may turn the camera.
    pub max_rotation_per_frame: f32,
    pub rotation_damping: f32,
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
            rotation_speed: 3.0,
            max_rotation_per_frame: 10.0,
            rotation_damping: 0.1,
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
    /// Zoom in positive, in wheel lines (or a held zoom button per frame).
    pub zoom: f32,
    pub cursor_locked: bool,
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

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct DampedVec2 {
    pub current: Vec2,
    pub velocity: Vec2,
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
    pub aim: DampedVec2,
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
            aim: DampedVec2 {
                current: Vec2::Y,
                velocity: Vec2::Y,
            },
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
        if desired.distance(player_position) > settings.max_distance_from_player {
            let direction = (desired - player_position).normalize_or_zero();
            let clamped = player_position + direction * settings.max_distance_from_player;
            movement = clamped - self.target.translation;
        }
        movement
    }

    pub fn apply_move(&mut self, movement: Vec3) {
        self.target.translation += movement;
    }

    /// Look, tilt and zoom for one frame; `rotate` is false while the cursor is free and the
    /// pointer button is up.
    pub fn steer(&mut self, input: &PhotoCameraInput, settings: &PhotoCameraSettings, dt: f32) {
        if input.cursor_locked || input.mouse_dragging {
            self.rotate(input.aim, settings, dt);
        }
        let reset_tilt = input.cursor_locked != self.cursor_was_locked;
        self.tilt_by(input.tilting, settings, reset_tilt, dt);
        self.zoom(input.zoom, settings, dt);
        self.target.rotation = Quat::from_euler(
            EulerRot::YXZ,
            self.yaw.to_radians(),
            self.pitch.to_radians(),
            self.roll.to_radians(),
        );
    }

    fn rotate(&mut self, aim: Vec2, settings: &PhotoCameraSettings, dt: f32) {
        let target = aim * settings.rotation_speed;
        self.aim.current = smooth_damp_vec2(
            self.aim.current,
            target,
            &mut self.aim.velocity,
            settings.rotation_damping,
            dt,
        );
        let max = settings.max_rotation_per_frame;
        let horizontal = (self.aim.current.x * dt).clamp(-max, max);
        let vertical = (self.aim.current.y * dt).clamp(-max, max);
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

    fn zoom(&mut self, zoom: f32, settings: &PhotoCameraSettings, dt: f32) {
        if zoom.abs() > f32::EPSILON {
            self.fov.target -= zoom * settings.fov_change_speed * dt * zoom_speed_scalar();
        }
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

/// Wheel lines are small on every platform but macOS, whose trackpads deliver pixel-sized deltas.
fn zoom_speed_scalar() -> f32 {
    if cfg!(target_os = "macos") {
        1.0
    } else {
        50.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DT: f32 = 1.0 / 60.0;

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
            cam.steer(&input, &settings, DT);
        }
        assert!(cam.yaw < 0.0, "mouse right turns right: {}", cam.yaw);
        assert!(cam.pitch < 0.0, "mouse down looks down: {}", cam.pitch);
        assert!(cam.pitch >= settings.min_vertical_angle);
        let before = cam.yaw;
        cam.steer(&input, &settings, DT);
        assert!(before - cam.yaw <= settings.max_rotation_per_frame + 1e-4);
    }

    #[test]
    fn a_free_cursor_only_looks_while_dragging() {
        let settings = PhotoCameraSettings::default();
        let mut cam = camera();
        let free = PhotoCameraInput {
            aim: Vec2::new(100.0, 0.0),
            ..Default::default()
        };
        cam.steer(&free, &settings, DT);
        cam.steer(&free, &settings, DT);
        assert_eq!(cam.yaw, 0.0);
        let drag = PhotoCameraInput {
            mouse_dragging: true,
            ..free
        };
        cam.steer(&drag, &settings, DT);
        cam.steer(&drag, &settings, DT);
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
            cam.steer(&tilt, &settings, DT);
        }
        assert!(cam.roll > 1.0, "tilt left rolls positive: {}", cam.roll);
        let release = PhotoCameraInput {
            tilting: 0.0,
            ..tilt
        };
        cam.steer(&release, &settings, DT);
        assert!(cam.tilt.target == 0.0);
        let unlocked = PhotoCameraInput {
            cursor_locked: false,
            ..release
        };
        cam.steer(&unlocked, &settings, DT);
        assert_eq!(cam.roll, 0.0);
    }

    #[test]
    fn zoom_narrows_the_fov_within_bounds() {
        let settings = PhotoCameraSettings::default();
        let mut cam = camera();
        let zoom_in = PhotoCameraInput {
            zoom: 1.0,
            cursor_locked: true,
            ..Default::default()
        };
        for _ in 0..10_000 {
            cam.steer(&zoom_in, &settings, DT);
        }
        assert!((cam.fov.target - settings.min_fov).abs() < 1e-4);
        assert!(cam.fov.current >= settings.min_fov - 1e-3);
        let zoom_out = PhotoCameraInput {
            zoom: -1.0,
            ..zoom_in
        };
        for _ in 0..20_000 {
            cam.steer(&zoom_out, &settings, DT);
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
