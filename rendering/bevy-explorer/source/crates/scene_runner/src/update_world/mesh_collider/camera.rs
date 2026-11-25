//! Collider queries for the third-person camera. Triggers and disabled colliders never count.

use bevy::math::{DVec3, Vec3};
use rapier3d_f64::{
    parry::{
        self,
        query::{Ray, ShapeCastOptions},
        shape::Ball,
    },
    prelude::*,
};

use dcl_component::proto_components::sdk::components::ColliderLayer;

use super::{ColliderId, SceneColliderData, GROUND_COLLISION_MASK};

const INSIDE_PROBE: f64 = 0.001;

/// SDK colliders on both `CL_PHYSICS` and `CL_POINTER` (the `MeshCollider` and invisible-mesh
/// default) and the terrain ground. Physics-only, pointer-only and custom-layer colliders, so a
/// scene can keep one out of the camera's way, border walls, landscape solids and avatars don't.
pub fn is_camera_obstacle(collider: &Collider) -> bool {
    let memberships = collider.collision_groups().memberships.bits();
    let physics_and_pointer = ColliderLayer::ClPhysics as u32 | ColliderLayer::ClPointer as u32;
    memberships & physics_and_pointer == physics_and_pointer
        || memberships & GROUND_COLLISION_MASK != 0
}

/// The point and surface normal are the obstacle's.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CameraHit {
    pub distance: f32,
    pub point: Vec3,
    pub normal: Vec3,
}

#[derive(Default, Debug)]
pub struct CameraSweep {
    /// The nearest obstacle the ball reaches that it did not start in.
    pub nearest: Option<CameraHit>,
    /// The obstacles the ball starts in, each as the hit whose `point + normal * radius` is
    /// `start` moved out of it; `distance` is the penetration depth minus the radius.
    pub starting: Vec<CameraHit>,
}

impl SceneColliderData {
    /// Must run after collider changes and before the shared-borrow `camera_*` queries.
    pub fn prepare_camera_queries(&mut self) {
        self.update_bvh();
    }

    fn camera_filter<'a>(
        &'a self,
        predicate: &'a dyn Fn(ColliderHandle, &Collider) -> bool,
    ) -> QueryFilter<'a> {
        QueryFilter {
            predicate: Some(predicate),
            ..QueryFilter::default().exclude_sensors()
        }
    }

    pub fn stops_camera(&self, id: &ColliderId) -> bool {
        self.get_collider_handle(id).is_some_and(|handle| {
            self.collider_set.get(handle).is_some_and(|collider| {
                collider.is_enabled()
                    && !collider.is_sensor()
                    && self.is_camera_candidate(handle, collider)
            })
        })
    }

    fn is_camera_candidate(&self, handle: ColliderHandle, collider: &Collider) -> bool {
        !self.disabled.contains(&handle) && is_camera_obstacle(collider)
    }

    pub fn camera_sphere_sweep(
        &self,
        start: Vec3,
        radius: f32,
        direction: Vec3,
        length: f32,
    ) -> CameraSweep {
        let mut sweep = CameraSweep::default();
        if !direction.is_finite() || !start.is_finite() || radius <= 0.0 {
            return sweep;
        }
        let ball = Ball::new(f64::from(radius));
        let start_pos =
            Isometry::translation(f64::from(start.x), f64::from(start.y), f64::from(start.z));

        let candidate = |h, c: &Collider| self.is_camera_candidate(h, c);
        let mut started_in = Vec::new();
        for (handle, collider) in self
            .query_pipeline(self.camera_filter(&candidate))
            .intersect_shape(start_pos, &ball)
        {
            started_in.push(handle);
            // no hit when the ball only touches the obstacle
            if let Some((push, depth)) = ball_penetration(&ball, &start_pos, collider) {
                sweep.starting.push(CameraHit {
                    distance: depth - radius,
                    point: start + push * (depth - radius),
                    normal: push,
                });
            }
        }

        if length <= 0.0 {
            return sweep;
        }
        let not_started_in = |h, c: &Collider| candidate(h, c) && !started_in.contains(&h);
        let Some((handle, hit)) = self
            .query_pipeline(self.camera_filter(&not_started_in))
            .cast_shape(
                &start_pos,
                &direction.as_dvec3().into(),
                &ball,
                ShapeCastOptions {
                    max_time_of_impact: f64::from(length),
                    target_distance: 0.0,
                    stop_at_penetration: true,
                    compute_impact_geometry_on_penetration: false,
                },
            )
        else {
            return sweep;
        };
        let Some(collider) = self.collider_set.get(handle) else {
            return sweep;
        };

        // the sweep's witnesses are in local frames; the contact at the stop is in world space
        let centre = start.as_dvec3() + direction.as_dvec3() * hit.time_of_impact;
        let centre_pos = Isometry::translation(centre.x, centre.y, centre.z);
        let contact = parry::query::contact(
            &centre_pos,
            &ball,
            collider.position(),
            collider.shape(),
            f64::from(radius),
        )
        .ok()
        .flatten();
        sweep.nearest = Some(match contact {
            Some(contact) => CameraHit {
                distance: hit.time_of_impact as f32,
                point: DVec3::from(contact.point2).as_vec3(),
                normal: DVec3::from(contact.normal2.into_inner()).as_vec3(),
            },
            None => CameraHit {
                distance: hit.time_of_impact as f32,
                point: centre.as_vec3() + direction * radius,
                normal: -direction,
            },
        });
        sweep
    }

    /// A collider the ray starts inside is not hit.
    pub fn camera_ray(
        &self,
        origin: Vec3,
        direction: Vec3,
        length: f32,
    ) -> Option<(f32, ColliderHandle)> {
        if !direction.is_finite() || !origin.is_finite() || length <= 0.0 {
            return None;
        }
        let candidate = |h, c: &Collider| self.is_camera_candidate(h, c);
        let probe = Ball::new(INSIDE_PROBE);
        let origin_pos = Isometry::translation(
            f64::from(origin.x),
            f64::from(origin.y),
            f64::from(origin.z),
        );
        let inside: Vec<ColliderHandle> = self
            .query_pipeline(self.camera_filter(&candidate))
            .intersect_shape(origin_pos, &probe)
            .map(|(h, _)| h)
            .collect();
        let outside = |h, c: &Collider| candidate(h, c) && !inside.contains(&h);
        let ray = Ray::new(origin.as_dvec3().into(), direction.as_dvec3().into());
        self.query_pipeline(self.camera_filter(&outside))
            .cast_ray(&ray, f64::from(length), true)
            .map(|(handle, toi)| (toi as f32, handle))
    }

    pub fn camera_overlaps(&self, centre: Vec3, radius: f32) -> Vec<ColliderHandle> {
        if !centre.is_finite() || radius <= 0.0 {
            return Vec::new();
        }
        let candidate = |h, c: &Collider| self.is_camera_candidate(h, c);
        let ball = Ball::new(f64::from(radius));
        self.query_pipeline(self.camera_filter(&candidate))
            .intersect_shape(
                Isometry::translation(
                    f64::from(centre.x),
                    f64::from(centre.y),
                    f64::from(centre.z),
                ),
                &ball,
            )
            .map(|(h, _)| h)
            .collect()
    }

    /// No hit when the ray starts inside the collider.
    pub fn camera_ray_collider(
        &self,
        handle: ColliderHandle,
        origin: Vec3,
        direction: Vec3,
        length: f32,
    ) -> Option<f32> {
        if !direction.is_finite() || !origin.is_finite() || length <= 0.0 {
            return None;
        }
        let collider = self.collider_set.get(handle)?;
        let origin_pos = Isometry::translation(
            f64::from(origin.x),
            f64::from(origin.y),
            f64::from(origin.z),
        );
        if parry::query::intersection_test(
            &origin_pos,
            &Ball::new(INSIDE_PROBE),
            collider.position(),
            collider.shape(),
        )
        .unwrap_or(false)
        {
            return None;
        }
        let ray = Ray::new(origin.as_dvec3().into(), direction.as_dvec3().into());
        collider
            .shape()
            .cast_ray(collider.position(), &ray, f64::from(length), true)
            .map(|toi| toi as f32)
    }

    /// The unit direction and distance that separate the ball from the collider.
    pub fn camera_penetration(
        &self,
        handle: ColliderHandle,
        centre: Vec3,
        radius: f32,
    ) -> Option<(Vec3, f32)> {
        let collider = self.collider_set.get(handle)?;
        let centre_pos = Isometry::translation(
            f64::from(centre.x),
            f64::from(centre.y),
            f64::from(centre.z),
        );
        ball_penetration(&Ball::new(f64::from(radius)), &centre_pos, collider)
    }
}

fn ball_penetration(
    ball: &Ball,
    ball_pos: &Isometry<f64>,
    collider: &Collider,
) -> Option<(Vec3, f32)> {
    let contact = parry::query::contact(ball_pos, ball, collider.position(), collider.shape(), 0.0)
        .ok()
        .flatten()?;
    if contact.dist >= 0.0 {
        return None;
    }
    // normal1 points from the ball towards the obstacle
    let push = -DVec3::from(contact.normal1.into_inner()).as_vec3();
    Some((push, -contact.dist as f32))
}
