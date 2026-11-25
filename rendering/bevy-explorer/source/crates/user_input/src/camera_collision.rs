//! Third-person camera collisions against
//! [`scene_runner::update_world::mesh_collider::is_camera_obstacle`].

use bevy::{prelude::*, render::mesh::MeshTag};
use rapier3d_f64::prelude::ColliderHandle;
use scene_material::{SceneMaterial, SCENE_MATERIAL_CAMERA_OBSTACLE_MESH_TAG};
use scene_runner::{
    update_world::mesh_collider::{CameraHit, CtCollider, HasCollider, SceneColliderData},
    ContainerEntity,
};

const MINIMUM_DISTANCE_FROM_TARGET: f32 = 0.1;
pub const CAMERA_RADIUS: f32 = 0.2;
/// holds the camera in after an obstacle clears, so bumpy geometry doesn't make it pop
const SMOOTHING_TIME: f32 = 0.3;
const DAMPING: f32 = 0.5;
/// short: the view is blocked until the camera is in front of the obstacle
const DAMPING_WHEN_OCCLUDED: f32 = 0.2;
const SHOULDER_DISTANCE_FROM_OBSTACLE: f32 = 0.5;
const EPSILON: f32 = 0.0001;
/// keeps a camera placed on an obstacle's surface from counting as inside it
const PRECISION_SLUSH: f32 = 0.001;
/// ln(0.01): a damping time is how long a change takes to settle to 1%
const LOG_NEGLIGIBLE_RESIDUAL: f32 = -4.605_170_2;

#[derive(Resource)]
pub struct CameraCollision {
    pub enabled: bool,
}

impl Default for CameraCollision {
    fn default() -> Self {
        Self { enabled: true }
    }
}

/// With collisions off no mesh is tagged, so every scene mesh fades. The tag bit is the only
/// record: other systems rewrite or drop `MeshTag`.
pub fn tag_camera_obstacle_meshes(
    collision: Res<CameraCollision>,
    scene_colliders: Query<&SceneColliderData>,
    mut meshes: Query<
        (
            Option<(&HasCollider<CtCollider>, &ContainerEntity)>,
            &mut Mesh3d,
            &mut MeshTag,
        ),
        With<MeshMaterial3d<SceneMaterial>>,
    >,
) {
    for (collider, mut mesh, mut tag) in &mut meshes {
        let stops_camera = collision.enabled
            && collider.is_some_and(|(collider, container)| {
                scene_colliders
                    .get(container.root)
                    .is_ok_and(|colliders| colliders.stops_camera(&collider.0))
            });
        let tagged = tag.0 & SCENE_MATERIAL_CAMERA_OBSTACLE_MESH_TAG != 0;
        if stops_camera != tagged {
            tag.0 ^= SCENE_MATERIAL_CAMERA_OBSTACLE_MESH_TAG;
            // bevy's mesh extraction doesn't watch MeshTag (0.16); Mesh3d is on its list
            mesh.set_changed();
        }
    }
}

/// The collider sets around the camera, queried as one.
pub struct CameraObstacles<'a>(pub Vec<&'a SceneColliderData>);

#[derive(Clone, Copy, Debug, PartialEq)]
struct ObstacleId(usize, ColliderHandle);

impl CameraObstacles<'_> {
    /// When the ball starts inside more than one obstacle, their hits are weighted by depth so it
    /// leaves them together.
    fn sphere_cast(
        &self,
        start: Vec3,
        radius: f32,
        direction: Vec3,
        length: f32,
    ) -> Option<CameraHit> {
        let mut hits = Vec::new();
        let mut penetrations = Vec::new();
        for set in &self.0 {
            let sweep = set.camera_sphere_sweep(start, radius, direction, length);
            hits.extend(sweep.nearest);
            for hit in sweep.starting {
                if hit.distance < -0.0001 {
                    penetrations.push(hit);
                }
                hits.push(hit);
            }
        }

        if penetrations.len() > 1 {
            let sum: f32 = penetrations.iter().map(|h| h.distance).sum();
            let mut combined = CameraHit {
                distance: 0.0,
                point: Vec3::ZERO,
                normal: Vec3::ZERO,
            };
            for hit in &penetrations {
                let t = hit.distance / sum;
                combined.point += hit.point * t;
                combined.distance += hit.distance * t;
                combined.normal += hit.normal * t;
            }
            combined.normal = combined.normal.normalize_or_zero();
            return Some(combined);
        }

        hits.into_iter()
            .min_by(|a, b| a.distance.total_cmp(&b.distance))
    }

    fn ray(&self, origin: Vec3, direction: Vec3, length: f32) -> Option<(f32, ObstacleId)> {
        self.0
            .iter()
            .enumerate()
            .filter_map(|(index, set)| {
                set.camera_ray(origin, direction, length)
                    .map(|(toi, handle)| (toi, ObstacleId(index, handle)))
            })
            .min_by(|a, b| a.0.total_cmp(&b.0))
    }

    fn overlaps(&self, centre: Vec3, radius: f32) -> Vec<ObstacleId> {
        self.0
            .iter()
            .enumerate()
            .flat_map(|(index, set)| {
                set.camera_overlaps(centre, radius)
                    .into_iter()
                    .map(move |handle| ObstacleId(index, handle))
            })
            .collect()
    }

    fn ray_collider(
        &self,
        id: ObstacleId,
        origin: Vec3,
        direction: Vec3,
        length: f32,
    ) -> Option<f32> {
        self.0[id.0].camera_ray_collider(id.1, origin, direction, length)
    }

    fn penetration(&self, id: ObstacleId, centre: Vec3, radius: f32) -> Option<(Vec3, f32)> {
        self.0[id.0].camera_penetration(id.1, centre, radius)
    }
}

#[derive(Component, Default)]
pub struct CameraCollisionState {
    pub desired_distance: f32,
    pub collided_distance: f32,
    pub occluded: bool,
    pub damp_time: f32,
    valid: bool,
    previous_raw_position: Vec3,
    previous_displacement: Vec3,
    previous_camera_offset: Vec3,
    smoothed_distance: f32,
    smoothed_time: f32,
    /// None takes the next target at once
    shoulder_length: Option<f32>,
}

impl CameraCollisionState {
    /// The next pass starts undamped.
    pub fn invalidate(&mut self) {
        self.valid = false;
        self.previous_displacement = Vec3::ZERO;
        self.smoothed_distance = 0.0;
        self.smoothed_time = 0.0;
        self.shoulder_length = None;
    }

    fn update_distance_smoothing(&mut self, distance: f32, now: f32) {
        if self.smoothed_distance == 0.0 || distance < self.smoothed_distance {
            self.smoothed_distance = distance;
            self.smoothed_time = now;
        }
    }

    fn apply_distance_smoothing(&self, distance: f32, now: f32) -> f32 {
        if self.smoothed_time != 0.0
            && SMOOTHING_TIME > EPSILON
            && now - self.smoothed_time < SMOOTHING_TIME
        {
            return distance.min(self.smoothed_distance);
        }
        distance
    }

    fn reset_distance_smoothing(&mut self, now: f32) {
        if now - self.smoothed_time >= SMOOTHING_TIME {
            self.smoothed_distance = 0.0;
            self.smoothed_time = 0.0;
        }
    }
}

pub struct Resolved {
    pub position: Vec3,
    pub occluded: bool,
    pub damp_time: f32,
}

/// `now` is the elapsed time and `dt` the frame time, in seconds.
pub fn resolve(
    obstacles: &CameraObstacles,
    state: &mut CameraCollisionState,
    look_at: Vec3,
    raw: Vec3,
    now: f32,
    dt: f32,
) -> Resolved {
    // the eased displacement turns with the orbit, so orbiting while occluded doesn't drag the
    // camera behind
    let mut bypass = Quat::IDENTITY;
    if state.valid {
        let dir0 = state.previous_raw_position - look_at;
        let dir1 = raw - look_at;
        if dir0.length_squared() > 0.01 && dir1.length_squared() > 0.01 {
            bypass = safe_from_to_rotation(dir0, dir1, Vec3::Y);
        }
    }
    state.previous_displacement = bypass * state.previous_displacement;

    let mut displacement = pull_camera_in_front_of_nearest_obstacle(obstacles, raw, look_at);
    let occluded = !almost_zero(displacement);

    if SMOOTHING_TIME > EPSILON {
        let pos = raw + displacement;
        let mut dir = pos - look_at;
        let mut distance = dir.length();
        if distance > EPSILON {
            dir /= distance;
            if !almost_zero(displacement) {
                state.update_distance_smoothing(distance, now);
            }
            distance = state.apply_distance_smoothing(distance, now);
            displacement += (look_at + dir * distance) - pos;
        }
    }
    if almost_zero(displacement) {
        state.reset_distance_smoothing(now);
    }

    let camera_pos = raw + displacement;
    displacement += respect_camera_radius(obstacles, camera_pos, look_at);

    let mut damp_time = DAMPING_WHEN_OCCLUDED;
    if dt >= 0.0 && state.valid && DAMPING_WHEN_OCCLUDED + DAMPING > EPSILON {
        // damping the damp time eases between damped and undamped regions
        let disp_sqr = displacement.length_squared();
        damp_time = if disp_sqr > state.previous_displacement.length_squared() {
            DAMPING_WHEN_OCCLUDED
        } else {
            DAMPING
        };
        if disp_sqr < EPSILON {
            damp_time = state.damp_time - damp(state.damp_time, damp_time, dt);
        }
        if damp_time > 0.0 {
            let previous = look_at + bypass * state.previous_camera_offset - raw;
            displacement = previous + damp_vec(displacement - previous, damp_time, dt);
        }
    }

    let position = raw + displacement;
    state.previous_raw_position = raw;
    state.previous_displacement = displacement;
    state.previous_camera_offset = position - look_at;
    state.damp_time = damp_time;
    state.valid = true;

    Resolved {
        position,
        occluded,
        damp_time,
    }
}

/// The shoulder `offset` from `pivot`, kept clear of obstacles beside it. It must move the pivot
/// before [`resolve`]: applied after, it shifts the camera sideways at once as a wall comes in
/// range, most of all in a corner. It eases rather than jumps, and is cut at once only where the
/// camera's ball would enter the obstacle.
pub fn shoulder_offset(
    obstacles: &CameraObstacles,
    state: &mut CameraCollisionState,
    pivot: Vec3,
    offset: Vec3,
    dt: f32,
) -> Vec3 {
    let full = offset.length();
    if full <= 0.0 {
        state.shoulder_length = Some(0.0);
        return offset;
    }
    let direction = offset / full;
    // past the offset by the camera's radius, so the cap below comes on continuously
    let hit = obstacles
        .ray(pivot, direction, full + CAMERA_RADIUS)
        .map(|(toi, _)| toi);

    let target = match hit {
        Some(toi) if toi <= full => (toi - SHOULDER_DISTANCE_FROM_OBSTACLE).clamp(0.0, full),
        _ => full,
    };
    let mut length = match state.shoulder_length {
        Some(current) => {
            let damp_time = if target < current {
                DAMPING_WHEN_OCCLUDED
            } else {
                DAMPING
            };
            current + damp(target - current, damp_time, dt)
        }
        None => target,
    };
    if let Some(toi) = hit {
        length = length.min((toi - CAMERA_RADIUS).max(0.0));
    }
    let length = length.clamp(0.0, full);
    state.shoulder_length = Some(length);
    direction * length
}

fn pull_camera_in_front_of_nearest_obstacle(
    obstacles: &CameraObstacles,
    camera_pos: Vec3,
    look_at: Vec3,
) -> Vec3 {
    let mut displacement = Vec3::ZERO;
    let mut dir = camera_pos - look_at;
    let target_distance = dir.length();
    if target_distance > EPSILON {
        dir /= target_distance;
        let min_distance_from_target = MINIMUM_DISTANCE_FROM_TARGET.max(EPSILON);
        if target_distance < min_distance_from_target + EPSILON {
            displacement = dir * (min_distance_from_target - target_distance);
        } else {
            let ray_length = target_distance - min_distance_from_target + PRECISION_SLUSH;
            if ray_length > EPSILON && CAMERA_RADIUS >= EPSILON {
                if let Some(hit) = obstacles.sphere_cast(
                    look_at + dir * CAMERA_RADIUS,
                    CAMERA_RADIUS,
                    dir,
                    ray_length - CAMERA_RADIUS,
                ) {
                    let desired = hit.point + hit.normal * CAMERA_RADIUS;
                    displacement = desired - camera_pos;
                }
            }
        }
    }
    displacement
}

fn respect_camera_radius(obstacles: &CameraObstacles, camera_pos: Vec3, look_at: Vec3) -> Vec3 {
    let mut result = Vec3::ZERO;
    if CAMERA_RADIUS < EPSILON {
        return result;
    }

    let mut dir = camera_pos - look_at;
    let distance = dir.length();
    if distance > EPSILON {
        dir /= distance;
    }

    let mut found = obstacles.overlaps(camera_pos, CAMERA_RADIUS);
    if found.is_empty() && distance > MINIMUM_DISTANCE_FROM_TARGET + EPSILON {
        // an overlap test misses a camera wholly inside an obstacle
        let d = distance - MINIMUM_DISTANCE_FROM_TARGET;
        let target_pos = look_at + dir * MINIMUM_DISTANCE_FROM_TARGET;
        if let Some((_, id)) = obstacles.ray(target_pos, dir, d) {
            if obstacles.ray_collider(id, camera_pos, -dir, d).is_none() {
                found.push(id);
            }
        }
    }

    if (!found.is_empty() && distance == 0.0) || distance > MINIMUM_DISTANCE_FROM_TARGET {
        let mut new_cam_pos = camera_pos;
        for id in found {
            if distance > MINIMUM_DISTANCE_FROM_TARGET {
                let mut dir = new_cam_pos - look_at;
                let d = dir.length();
                if d > EPSILON {
                    dir /= d;
                    if let Some(toi) = obstacles.ray_collider(id, look_at, dir, d + CAMERA_RADIUS) {
                        new_cam_pos = look_at + dir * toi - dir * PRECISION_SLUSH;
                    }
                }
            }
            if let Some((push, depth)) = obstacles.penetration(id, new_cam_pos, CAMERA_RADIUS) {
                new_cam_pos += push * depth;
            }
        }
        result = new_cam_pos - camera_pos;
    }

    if distance > EPSILON && MINIMUM_DISTANCE_FROM_TARGET > EPSILON {
        let min_distance = MINIMUM_DISTANCE_FROM_TARGET.max(CAMERA_RADIUS) + PRECISION_SLUSH;
        let new_offset = camera_pos + result - look_at;
        if new_offset.length() < min_distance {
            result = look_at - camera_pos + dir * min_distance;
        }
    }

    result
}

fn almost_zero(v: Vec3) -> bool {
    v.length_squared() < EPSILON * EPSILON
}

/// The part of `initial` that takes effect over `dt`.
fn damp(initial: f32, damp_time: f32, dt: f32) -> f32 {
    if damp_time < EPSILON || initial.abs() < EPSILON {
        return initial;
    }
    if dt < EPSILON {
        return 0.0;
    }
    let k = -LOG_NEGLIGIBLE_RESIDUAL / damp_time;
    initial * (1.0 - (-k * dt).exp())
}

fn damp_vec(initial: Vec3, damp_time: f32, dt: f32) -> Vec3 {
    Vec3::new(
        damp(initial.x, damp_time, dt),
        damp(initial.y, damp_time, dt),
        damp(initial.z, damp_time, dt),
    )
}

fn safe_from_to_rotation(v1: Vec3, v2: Vec3, up: Vec3) -> Quat {
    let mut axis = v1.cross(v2);
    if almost_zero(axis) {
        axis = up;
    }
    let angle = v1.angle_between(v2);
    Quat::from_axis_angle(axis.normalize(), angle)
}

#[cfg(test)]
mod tests;
