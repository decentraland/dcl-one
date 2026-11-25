//! The player camera's three zoom levels: where the camera sits at each, how the wheel and the
//! switch key step between them, and the blend from one to the next.
//!
//! First person sits at the camera focus and looks where the player aims. Third person and drone
//! orbit the focus: looking up or down slides the camera along a curve through three rings (top,
//! middle, bottom), each a height above the focus and a radius around it. The orbit value runs
//! from 0 (bottom ring) to 1 (top ring) and is the camera pitch mapped linearly.

use std::f32::consts::{FRAC_PI_2, PI};

use bevy::prelude::*;
use common::structs::{CameraShoulder, CameraZoomLevel};

/// The point the camera follows and aims at, above the player's feet.
pub const FOCUS_HEIGHT: f32 = 1.75;
/// Seconds a change of level takes.
const BLEND_SECS: f32 = 0.5;
/// Metres per second the shoulder offset moves towards its target.
const SHOULDER_CHANGE_SPEED: f32 = 4.0;
/// Degrees the first-person camera may pitch.
const FIRST_PERSON_PITCH_LIMIT: f32 = 89.0;
const EPSILON: f32 = 0.0001;

/// Height above the focus and radius around it, in metres.
#[derive(Clone, Copy)]
struct Ring {
    height: f32,
    radius: f32,
}

/// An orbiting level. Arrays are bottom, middle, top ring.
struct Orbit {
    rings: [Ring; 3],
    /// Where the focus sits on screen from the top (0) to the bottom (1) at each ring.
    screen_y: [f32; 3],
    /// The camera-space shoulder offset (right, up) at each ring.
    shoulder: [Vec2; 3],
    fov_degrees: f32,
}

const FIRST_PERSON_FOV_DEGREES: f32 = 60.0;

const THIRD_PERSON: Orbit = Orbit {
    rings: [
        Ring {
            height: -0.75,
            radius: 0.7,
        },
        Ring {
            height: 1.0,
            radius: 4.5,
        },
        Ring {
            height: 4.5,
            radius: 1.2,
        },
    ],
    screen_y: [0.6, 0.55, 0.5],
    shoulder: [
        Vec2::new(0.3, 0.2),
        Vec2::new(0.6, 0.0),
        Vec2::new(0.3, 0.7),
    ],
    fov_degrees: 60.0,
};

const DRONE: Orbit = Orbit {
    rings: [
        Ring {
            height: 0.5,
            radius: 7.0,
        },
        Ring {
            height: 4.0,
            radius: 12.0,
        },
        Ring {
            height: 12.0,
            radius: 0.8,
        },
    ],
    screen_y: [0.6, 0.55, 0.5],
    shoulder: [Vec2::ZERO, Vec2::ZERO, Vec2::new(0.0, 2.0)],
    fov_degrees: 70.0,
};

fn orbit(level: CameraZoomLevel) -> Option<&'static Orbit> {
    match level {
        CameraZoomLevel::FirstPerson => None,
        CameraZoomLevel::ThirdPerson => Some(&THIRD_PERSON),
        CameraZoomLevel::Drone => Some(&DRONE),
    }
}

/// Vertical, in radians.
fn level_fov(level: CameraZoomLevel) -> f32 {
    orbit(level)
        .map_or(FIRST_PERSON_FOV_DEGREES, |orbit| orbit.fov_degrees)
        .to_radians()
}

/// Radians a level's camera may pitch either side of level.
pub fn pitch_limit(level: CameraZoomLevel) -> f32 {
    match level {
        CameraZoomLevel::FirstPerson => FIRST_PERSON_PITCH_LIMIT.to_radians(),
        CameraZoomLevel::ThirdPerson | CameraZoomLevel::Drone => FRAC_PI_2,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ZoomDirection {
    In,
    Out,
}

impl ZoomDirection {
    pub fn reversed(self) -> Self {
        match self {
            ZoomDirection::In => ZoomDirection::Out,
            ZoomDirection::Out => ZoomDirection::In,
        }
    }
}

/// The level one step from `level`, and whether the step went in `direction`. The wheel stops at
/// the ends; the switch key (`ping_pong`) bounces back from them, and its caller then reverses.
pub fn step_level(
    level: CameraZoomLevel,
    direction: ZoomDirection,
    ping_pong: bool,
) -> (CameraZoomLevel, bool) {
    use CameraZoomLevel::*;
    match (direction, level) {
        (ZoomDirection::Out, FirstPerson) => (ThirdPerson, true),
        (ZoomDirection::Out, ThirdPerson) => (Drone, true),
        (ZoomDirection::Out, Drone) => (if ping_pong { ThirdPerson } else { Drone }, false),
        (ZoomDirection::In, Drone) => (ThirdPerson, true),
        (ZoomDirection::In, ThirdPerson) => (FirstPerson, true),
        (ZoomDirection::In, FirstPerson) => {
            (if ping_pong { ThirdPerson } else { FirstPerson }, false)
        }
    }
}

/// The orbit value of a camera pitch (radians, up positive): 0.5 level, 1 looking straight down.
fn orbit_value(pitch: f32) -> f32 {
    (0.5 - pitch / PI).clamp(0.0, 1.0)
}

fn pitch_of_orbit_value(value: f32) -> f32 {
    (0.5 - value) * PI
}

/// Control points that make the cubic segments through `knots` join smoothly: each coordinate
/// solved as a tridiagonal system, straight into the first segment and out of the last.
fn smooth_control_points(knots: &[Vec2; 5]) -> ([Vec2; 4], [Vec2; 4]) {
    const SEGMENTS: usize = 4;
    let mut ctrl1 = [Vec2::ZERO; SEGMENTS];
    let mut ctrl2 = [Vec2::ZERO; SEGMENTS];
    for axis in 0..2 {
        let knot = |i: usize| knots[i][axis];
        let mut a = [1.0f32; SEGMENTS];
        let mut b = [4.0f32; SEGMENTS];
        let mut c = [1.0f32; SEGMENTS];
        let mut r = [0.0f32; SEGMENTS];
        (a[0], b[0], c[0]) = (0.0, 2.0, 1.0);
        r[0] = knot(0) + 2.0 * knot(1);
        for (i, r) in r.iter_mut().enumerate().take(SEGMENTS - 1).skip(1) {
            *r = 4.0 * knot(i) + 2.0 * knot(i + 1);
        }
        let last = SEGMENTS - 1;
        (a[last], b[last], c[last]) = (2.0, 7.0, 0.0);
        r[last] = 8.0 * knot(last) + knot(SEGMENTS);

        for i in 1..SEGMENTS {
            let m = a[i] / b[i - 1];
            b[i] -= m * c[i - 1];
            r[i] -= m * r[i - 1];
        }

        let mut first = [0.0f32; SEGMENTS];
        first[last] = r[last] / b[last];
        for i in (0..last).rev() {
            first[i] = (r[i] - c[i] * first[i + 1]) / b[i];
        }
        for i in 0..last {
            ctrl1[i][axis] = first[i];
            ctrl2[i][axis] = 2.0 * knot(i + 1) - first[i + 1];
        }
        ctrl1[last][axis] = first[last];
        ctrl2[last][axis] = 0.5 * (knot(SEGMENTS) + first[last]);
    }
    (ctrl1, ctrl2)
}

/// Where the camera sits on the orbit at `value`: x is metres behind the focus, y metres above.
fn orbit_point(orbit: &Orbit, value: f32) -> Vec2 {
    let ring = |i: usize| Vec2::new(orbit.rings[i].radius, orbit.rings[i].height);
    // the curve runs on from the bottom and top rings towards the focus, which shapes its ends
    let knots = [Vec2::ZERO, ring(0), ring(1), ring(2), Vec2::ZERO];
    let (ctrl1, ctrl2) = smooth_control_points(&knots);
    let (segment, t) = if value > 0.5 {
        (2, (value - 0.5) * 2.0)
    } else {
        (1, value * 2.0)
    };
    let t = t.clamp(0.0, 1.0);
    let d = 1.0 - t;
    d * d * d * knots[segment]
        + 3.0 * d * d * t * ctrl1[segment]
        + 3.0 * d * t * t * ctrl2[segment]
        + t * t * t * knots[segment + 1]
}

fn angle_degrees(a: Vec2, b: Vec2) -> f32 {
    let (a, b) = (a.normalize_or_zero(), b.normalize_or_zero());
    ((a - b).length().atan2((a + b).length()) * 2.0).to_degrees()
}

/// The orbit value whose point lies in the direction nearest `offset` (metres behind and above
/// the focus): the best of eleven samples, refined by Newton steps on the angle between them.
fn closest_orbit_value(orbit: &Orbit, offset: Vec2) -> f32 {
    const MAX_ITERATIONS: usize = 10;
    const TOLERANCE: f32 = 0.005;
    let angle = |value: f32| angle_degrees(offset, orbit_point(orbit, value));

    let mut best = 0.5;
    let mut best_angle = angle(best);
    for step in 0..=5 {
        let t = step as f32 * 0.1;
        for candidate in [0.5 + t, 0.5 - t] {
            let candidate_angle = angle(candidate);
            if candidate_angle < best_angle {
                best_angle = candidate_angle;
                best = candidate;
            }
        }
    }

    let mut value = best;
    for _ in 0..MAX_ITERATIONS {
        let at = angle(value);
        let slope = (angle(value + TOLERANCE) - angle(value - TOLERANCE)) / (2.0 * TOLERANCE);
        if slope.abs() < TOLERANCE || at.abs() < TOLERANCE {
            break;
        }
        value = (value - at / slope).clamp(0.0, 1.0);
    }
    value
}

fn across_rings<T: Copy + std::ops::Add<Output = T> + std::ops::Mul<f32, Output = T>>(
    rings: [T; 3],
    value: f32,
) -> T {
    let (from, to, t) = if value < 0.5 {
        (rings[0], rings[1], value * 2.0)
    } else {
        (rings[1], rings[2], (value - 0.5) * 2.0)
    };
    from * (1.0 - t) + to * t
}

/// Radians the camera tilts up from the focus so the focus shows at `screen_y`.
fn screen_tilt(screen_y: f32, fov_degrees: f32) -> f32 {
    ((2.0 * screen_y - 1.0) * (fov_degrees.to_radians() / 2.0).tan()).atan()
}

/// The yaw and pitch the camera continues from when the level changes from `from` to `to`.
fn carry_view(from: CameraZoomLevel, to: CameraZoomLevel, yaw: f32, pitch: f32) -> (f32, f32) {
    let pitch = match (orbit(from), orbit(to)) {
        (Some(from), Some(to)) => pitch_of_orbit_value(closest_orbit_value(
            to,
            orbit_point(from, orbit_value(pitch)),
        )),
        // into first person from the third-person camera: keep looking where it looked
        (Some(third), None) if from == CameraZoomLevel::ThirdPerson => {
            let value = orbit_value(pitch);
            let point = orbit_point(third, value);
            let tilt = across_rings(
                third.screen_y.map(|y| screen_tilt(y, third.fov_degrees)),
                value,
            );
            tilt - point.y.atan2(point.x)
        }
        _ => pitch,
    };
    let limit = pitch_limit(to);
    (yaw, pitch.clamp(-limit, limit))
}

/// A camera placement. `look_at` is the point an orbiting camera aims around; first person has
/// none.
#[derive(Clone, Copy, Debug)]
pub struct CameraPose {
    pub position: Vec3,
    pub rotation: Quat,
    /// Vertical, in radians.
    pub fov: f32,
    look_at: Option<Vec3>,
    /// Blend this pose's position around its `look_at` instead of in a straight line.
    spherical: bool,
}

impl CameraPose {
    pub fn transform(&self) -> Transform {
        Transform::from_translation(self.position).with_rotation(self.rotation)
    }
}

fn level_pose(
    level: CameraZoomLevel,
    focus: Vec3,
    yaw: f32,
    pitch: f32,
    roll: f32,
    shoulder_offset: Vec2,
) -> CameraPose {
    let Some(orbit) = orbit(level) else {
        let limit = pitch_limit(level);
        return CameraPose {
            position: focus,
            rotation: Quat::from_euler(EulerRot::YXZ, yaw, pitch.clamp(-limit, limit), roll),
            fov: level_fov(level),
            look_at: None,
            spherical: false,
        };
    };

    let value = orbit_value(pitch);
    let point = orbit_point(orbit, value);
    let heading = Quat::from_rotation_y(yaw);
    let position = focus + heading * Vec3::new(0.0, point.y, point.x);
    let tilt = across_rings(
        orbit.screen_y.map(|y| screen_tilt(y, orbit.fov_degrees)),
        value,
    );
    let aim = heading * Quat::from_rotation_x(-point.y.atan2(point.x));
    let rotation = aim * Quat::from_rotation_x(tilt) * Quat::from_rotation_z(roll);
    let shift = aim * Quat::from_rotation_x(tilt) * shoulder_offset.extend(0.0);
    CameraPose {
        position: position + shift,
        rotation,
        fov: level_fov(level),
        look_at: Some(focus + shift),
        spherical: true,
    }
}

/// The camera-space shoulder offset a level's camera moves towards. `side` is 1 over the right
/// shoulder, -1 over the left and 0 centred.
fn shoulder_target(level: CameraZoomLevel, pitch: f32, side: f32) -> Vec2 {
    let Some(orbit) = orbit(level) else {
        return Vec2::ZERO;
    };
    let offset = across_rings(orbit.shoulder, orbit_value(pitch));
    Vec2::new(offset.x * side, offset.y)
}

fn move_towards(current: Vec2, target: Vec2, max_delta: f32) -> Vec2 {
    let to = target - current;
    let distance = to.length();
    if distance <= max_delta || distance < EPSILON {
        target
    } else {
        current + to / distance * max_delta
    }
}

fn slerp_vec3(a: Vec3, b: Vec3, t: f32) -> Vec3 {
    let (length_a, length_b) = (a.length(), b.length());
    if length_a < EPSILON || length_b < EPSILON {
        return a.lerp(b, t);
    }
    let (dir_a, dir_b) = (a / length_a, b / length_b);
    let angle = dir_a.angle_between(dir_b);
    let axis = dir_a.cross(dir_b);
    if angle < EPSILON || axis.length_squared() < EPSILON * EPSILON {
        return a.lerp(b, t);
    }
    let dir = Quat::from_axis_angle(axis.normalize(), angle * t) * dir_a;
    dir * (length_a + (length_b - length_a) * t)
}

fn signed_angle(from: Vec3, to: Vec3, axis: Vec3) -> f32 {
    let angle = from.angle_between(to);
    if axis.dot(from.cross(to)) < 0.0 {
        -angle
    } else {
        angle
    }
}

/// The turn about `up`, then the tilt about the camera's right, that take a camera at `rotation`
/// to face `direction`.
fn rotation_to_target(rotation: Quat, direction: Vec3, up: Vec3) -> Vec2 {
    if direction.length_squared() < EPSILON * EPSILON {
        return Vec2::ZERO;
    }
    let to_local = rotation.inverse();
    let up = to_local * up;
    let direction = to_local * direction;
    let mut turn = 0.0;
    let flat_direction = direction - up * direction.dot(up);
    if flat_direction.length_squared() > EPSILON * EPSILON {
        let mut flat_forward = Vec3::NEG_Z - up * Vec3::NEG_Z.dot(up);
        if flat_forward.length_squared() < EPSILON * EPSILON {
            flat_forward = Vec3::Y - up * Vec3::Y.dot(up);
        }
        turn = signed_angle(flat_forward, flat_direction, up);
    }
    let turned = Quat::from_axis_angle(up.normalize_or(Vec3::Y), turn);
    let tilt = signed_angle(turned * Vec3::NEG_Z, direction, turned * Vec3::X);
    Vec2::new(tilt, turn)
}

fn apply_rotation(rotation: Quat, tilt_and_turn: Vec2, up: Vec3) -> Quat {
    if tilt_and_turn.length_squared() < EPSILON {
        return rotation;
    }
    Quat::from_axis_angle(up, tilt_and_turn.y) * rotation * Quat::from_rotation_x(tilt_and_turn.x)
}

/// The field of view between two shots of the same subject, keeping the subject's size on screen
/// moving evenly. Radians.
fn interpolate_fov(fov_a: f32, fov_b: f32, distance_a: f32, distance_b: f32, t: f32) -> f32 {
    let height_a = distance_a * 2.0 * (fov_a / 2.0).tan();
    let height_b = distance_b * 2.0 * (fov_b / 2.0).tan();
    let height = height_a + (height_b - height_a) * t;
    let distance = distance_a + (distance_b - distance_a) * t;
    let fov = if distance > EPSILON {
        2.0 * (height / (2.0 * distance)).atan()
    } else {
        179f32.to_radians()
    };
    fov.clamp(fov_a.min(fov_b), fov_a.max(fov_b))
}

/// The pose `t` of the way from `a` to `b`. Two orbiting cameras swing around what they look at
/// and keep it framed; otherwise position and rotation go straight.
fn blend_poses(a: &CameraPose, b: &CameraPose, t: f32) -> CameraPose {
    const NEAR: f32 = 0.1;
    let t = t.clamp(0.0, 1.0);
    let mut fov = a.fov + (b.fov - a.fov) * t;
    let mut aim_t = t;
    let look_at = match (a.look_at, b.look_at) {
        (Some(look_a), Some(look_b)) => {
            if (a.fov - b.fov).abs() > EPSILON {
                fov = interpolate_fov(
                    a.fov,
                    b.fov,
                    (look_a - a.position).length().max(NEAR),
                    (look_b - b.position).length().max(NEAR),
                    t,
                );
                aim_t = ((fov - a.fov) / (b.fov - a.fov)).abs();
            }
            Some((look_a.lerp(look_b, aim_t), look_a, look_b))
        }
        _ => None,
    };

    let spherical = a.spherical || b.spherical;
    let position = match look_at {
        Some((_, look_a, look_b)) if spherical => {
            let arm = slerp_vec3(a.position - look_a, b.position - look_b, t);
            (look_a + arm).lerp(look_b + arm, t)
        }
        _ => a.position.lerp(b.position, t),
    };

    let straight = a.rotation.slerp(b.rotation, t);
    let rotation = match look_at {
        Some((look, look_a, look_b)) if a.rotation.angle_between(b.rotation) > EPSILON => {
            let direction = (look - position).normalize_or_zero();
            if direction.cross(Vec3::Y).length_squared() < EPSILON * EPSILON {
                straight
            } else {
                let facing = Transform::IDENTITY.looking_to(direction, Vec3::Y).rotation;
                let from = -rotation_to_target(a.rotation, look_a - a.position, Vec3::Y);
                let to = -rotation_to_target(b.rotation, look_b - b.position, Vec3::Y);
                apply_rotation(facing, from.lerp(to, aim_t), Vec3::Y)
            }
        }
        _ => straight,
    };

    CameraPose {
        position,
        rotation,
        fov,
        look_at: look_at.map(|(look, ..)| look),
        spherical,
    }
}

/// The yaw and pitch each level was left with, by `CameraZoomLevel as usize`.
type LevelAims = [Option<(f32, f32)>; 3];

/// What a blend starts from: a level's camera as it was aimed when the player left it, or a
/// blend that was under way, held at the pose it had reached.
#[derive(Clone, Debug)]
enum BlendSource {
    Level {
        level: CameraZoomLevel,
        yaw: f32,
        pitch: f32,
    },
    Held {
        /// Relative to the focus, so the held camera keeps following the player.
        pose: CameraPose,
        /// The level the held blend was heading to.
        left: CameraZoomLevel,
        aims: LevelAims,
    },
}

impl BlendSource {
    fn aims(&self) -> LevelAims {
        match self {
            BlendSource::Level { level, yaw, pitch } => {
                let mut aims = LevelAims::default();
                aims[*level as usize] = Some((*yaw, *pitch));
                aims
            }
            BlendSource::Held { aims, .. } => *aims,
        }
    }

    /// The level the player most recently left.
    fn left(&self) -> CameraZoomLevel {
        match self {
            BlendSource::Level { level, .. } => *level,
            BlendSource::Held { left, .. } => *left,
        }
    }
}

#[derive(Clone, Debug)]
struct Blend {
    from: BlendSource,
    seconds: f32,
    elapsed: f32,
    /// How far along a blend this one backed out of had come, counted from its far end.
    start: f32,
}

impl Blend {
    fn weight(&self) -> f32 {
        if self.elapsed >= self.seconds {
            return 1.0;
        }
        let t = (self.elapsed / self.seconds).clamp(0.0, 1.0);
        t * t * (3.0 - 2.0 * t)
    }
}

/// The player camera's zoom state beside `PrimaryCamera`: the level it shows or is blending to,
/// the blend, and each orbiting level's shoulder offset.
#[derive(Component, Default, Debug)]
pub struct CameraZoom {
    /// None while a scene's cinematic camera has the view.
    level: Option<CameraZoomLevel>,
    blend: Option<Blend>,
    /// Camera-space (right, up) offsets of the third-person and the drone camera.
    shoulder_offsets: [Vec2; 2],
    /// The way the switch key steps next.
    pub switch_direction: Option<ZoomDirection>,
}

fn shoulder_slot(level: CameraZoomLevel) -> Option<usize> {
    match level {
        CameraZoomLevel::FirstPerson => None,
        CameraZoomLevel::ThirdPerson => Some(0),
        CameraZoomLevel::Drone => Some(1),
    }
}

impl CameraZoom {
    pub fn level(&self) -> Option<CameraZoomLevel> {
        self.level
    }

    pub fn blending(&self) -> bool {
        self.blend.is_some()
    }

    pub fn shoulder_offset(&self, level: CameraZoomLevel) -> Vec2 {
        shoulder_slot(level).map_or(Vec2::ZERO, |slot| self.shoulder_offsets[slot])
    }

    /// Follows the level in effect. A change between two levels starts a blend and returns the
    /// aim the camera continues from; `yaw` and `pitch` are the aim it had.
    pub fn set_level(
        &mut self,
        level: Option<CameraZoomLevel>,
        yaw: f32,
        pitch: f32,
    ) -> Option<(f32, f32)> {
        if level == self.level {
            return None;
        }
        let (Some(from), Some(to)) = (self.level, level) else {
            // a scene's camera took or returned the view: its own transition covers the change
            self.level = level;
            self.blend = None;
            return None;
        };
        self.level = level;

        let mut seconds = BLEND_SECS;
        let mut start = 0.0;
        let mut kept_aim = None;
        let source = match self.blend.take() {
            Some(running) => {
                if running.from.left() == to {
                    let progress =
                        running.start + (1.0 - running.start) * (running.elapsed / running.seconds);
                    seconds *= progress;
                    start = 1.0 - progress;
                }
                let mut aims = running.from.aims();
                // a level still in the running blend keeps the aim it was left with
                kept_aim = aims[to as usize];
                aims[from as usize] = Some((yaw, pitch));
                // the pose shown now, around a focus at the origin
                let leaving = level_pose(
                    from,
                    Vec3::ZERO,
                    yaw,
                    pitch,
                    0.0,
                    self.shoulder_offset(from),
                );
                let pose = blend_poses(
                    &self.source_pose(&running.from, Vec3::ZERO),
                    &leaving,
                    running.weight(),
                );
                BlendSource::Held {
                    pose,
                    left: from,
                    aims,
                }
            }
            None => BlendSource::Level {
                level: from,
                yaw,
                pitch,
            },
        };
        self.blend = Some(Blend {
            from: source,
            seconds,
            elapsed: 0.0,
            start,
        });
        Some(kept_aim.unwrap_or_else(|| carry_view(from, to, yaw, pitch)))
    }

    /// Advances the blend and moves the shown level's shoulder offset. `locked` is whether the
    /// cursor is locked to the camera: the camera centres while it is free.
    pub fn tick(&mut self, dt: f32, pitch: f32, shoulder: CameraShoulder, locked: bool) {
        if let Some(blend) = &mut self.blend {
            blend.elapsed += dt;
            if blend.elapsed >= blend.seconds {
                self.blend = None;
            }
        }
        let Some(level) = self.level else { return };
        let Some(slot) = shoulder_slot(level) else {
            return;
        };
        let side = match (locked, shoulder) {
            (false, _) => 0.0,
            (true, CameraShoulder::Right) => 1.0,
            (true, CameraShoulder::Left) => -1.0,
        };
        self.shoulder_offsets[slot] = move_towards(
            self.shoulder_offsets[slot],
            shoulder_target(level, pitch, side),
            SHOULDER_CHANGE_SPEED * dt,
        );
    }

    fn source_pose(&self, source: &BlendSource, focus: Vec3) -> CameraPose {
        match source {
            BlendSource::Level { level, yaw, pitch } => level_pose(
                *level,
                focus,
                *yaw,
                *pitch,
                0.0,
                self.shoulder_offset(*level),
            ),
            BlendSource::Held { pose, .. } => CameraPose {
                position: pose.position + focus,
                look_at: pose.look_at.map(|look_at| look_at + focus),
                ..*pose
            },
        }
    }

    /// None while a scene's cinematic camera has the view.
    pub fn pose(&self, focus: Vec3, yaw: f32, pitch: f32, roll: f32) -> Option<CameraPose> {
        let level = self.level?;
        let target = level_pose(level, focus, yaw, pitch, roll, self.shoulder_offset(level));
        Some(match &self.blend {
            Some(blend) => blend_poses(
                &self.source_pose(&blend.from, focus),
                &target,
                blend.weight(),
            ),
            None => target,
        })
    }
}

#[cfg(test)]
mod tests;
