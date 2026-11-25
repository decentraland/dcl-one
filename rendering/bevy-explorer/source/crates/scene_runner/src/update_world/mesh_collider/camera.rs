//! Ball queries that report where the ball touches, for the third-person camera. Triggers and
//! colliders that are off for the player never count.

use bevy::math::{DVec3, Vec3};
use rapier3d_f64::{
    parry::{self, query::Ray, shape::Ball},
    prelude::*,
};

use super::{ColliderId, ColliderLayers, SceneColliderData};

const INSIDE_PROBE: f64 = 0.001;

/// The point and surface normal are the obstacle's.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BallHit {
    pub distance: f32,
    pub point: Vec3,
    pub normal: Vec3,
}

#[derive(Default, Debug)]
pub struct BallSweep {
    /// The nearest obstacle the ball reaches that it did not start in.
    pub nearest: Option<BallHit>,
    /// The obstacles the ball starts in, each as the hit whose `point + normal * radius` is
    /// `origin` moved out of it; `distance` is the penetration depth minus the radius.
    pub starting: Vec<BallHit>,
}

impl SceneColliderData {
    pub fn stops_camera(&self, id: &ColliderId) -> bool {
        self.get_collider_handle(id).is_some_and(|handle| {
            self.collider_set.get(handle).is_some_and(|collider| {
                !collider.is_sensor() && self.meets(ColliderLayers::CAMERA, handle, collider)
            })
        })
    }

    /// [`Self::cast_ball_nearest`] over the stretches of the sweep during which the ball touches
    /// the space the scene is drawn in, with the contact. A ball that comes into the scene
    /// already overlapping a collider is stopped at the scene's edge, with the edge as the
    /// contact. Panics unless [`Self::update_bvh`] ran since the last collider change.
    pub fn cast_ball_in_scene_flushed(
        &self,
        origin: Vec3,
        direction: Vec3,
        distance: f32,
        radius: f32,
        collision_mask: impl Into<ColliderLayers>,
    ) -> BallSweep {
        let mut sweep = BallSweep::default();
        if !direction.is_finite() || !origin.is_finite() || radius <= 0.0 {
            return sweep;
        }
        let layers = collision_mask.into();
        let ball = Ball::new(f64::from(radius));
        let radius_f64 = f64::from(radius);
        let origin_f64 = origin.as_dvec3();
        let direction_f64 = direction.as_dvec3();

        for stretch in self.volume.stretches(
            origin_f64 - DVec3::Y * radius_f64,
            direction_f64,
            f64::from(distance.max(0.0)),
            radius_f64,
        ) {
            let start = origin_f64 + direction_f64 * stretch.start;
            let start_pos = Isometry::translation(start.x, start.y, start.z);
            let (started_in, nearest) = self.sweep_ball(
                start,
                direction_f64,
                stretch.end - stretch.start,
                &ball,
                layers,
            );

            match stretch.entered {
                None => {
                    for collider in started_in.iter().filter_map(|h| self.collider_set.get(*h)) {
                        // no hit when the ball only touches the obstacle
                        if let Some((push, depth)) = ball_penetration(&ball, &start_pos, collider) {
                            sweep.starting.push(BallHit {
                                distance: depth - radius,
                                point: origin + push * (depth - radius),
                                normal: push,
                            });
                        }
                    }
                }
                Some(edge) if !started_in.is_empty() => {
                    sweep.nearest = Some(BallHit {
                        distance: stretch.start as f32,
                        point: (start - edge * radius_f64).as_vec3(),
                        normal: edge.as_vec3(),
                    });
                    return sweep;
                }
                Some(_) => (),
            }

            let Some((handle, toi)) = nearest else {
                continue;
            };
            let Some(collider) = self.collider_set.get(handle) else {
                continue;
            };
            // the sweep's witnesses are in local frames; the contact at the stop is in world space
            let centre = start + direction_f64 * toi;
            let contact = parry::query::contact(
                &Isometry::translation(centre.x, centre.y, centre.z),
                &ball,
                collider.position(),
                collider.shape(),
                radius_f64,
            )
            .ok()
            .flatten();
            let distance = (stretch.start + toi) as f32;
            sweep.nearest = Some(match contact {
                Some(contact) => BallHit {
                    distance,
                    point: DVec3::from(contact.point2).as_vec3(),
                    normal: DVec3::from(contact.normal2.into_inner()).as_vec3(),
                },
                None => BallHit {
                    distance,
                    point: centre.as_vec3() + direction * radius,
                    normal: -direction,
                },
            });
            return sweep;
        }
        sweep
    }

    /// The colliders a ball overlaps: none where it does not touch the space the scene is drawn
    /// in. Panics unless [`Self::update_bvh`] ran since the last collider change.
    pub fn ball_overlaps_flushed(
        &self,
        centre: Vec3,
        radius: f32,
        collision_mask: impl Into<ColliderLayers>,
    ) -> Vec<ColliderId> {
        let centre_f64 = centre.as_dvec3();
        let radius_f64 = f64::from(radius);
        if !centre.is_finite()
            || radius <= 0.0
            || !self
                .volume
                .reaches(centre_f64 - DVec3::Y * radius_f64, radius_f64)
        {
            return Vec::new();
        }
        let layers = collision_mask.into();
        let predicate = |h, c: &Collider| self.meets(layers, h, c);
        self.query_pipeline(
            QueryFilter::default()
                .groups(layers.groups())
                .exclude_sensors()
                .predicate(&predicate),
        )
        .intersect_shape(
            Isometry::translation(centre_f64.x, centre_f64.y, centre_f64.z),
            &Ball::new(radius_f64),
        )
        .filter_map(|(h, _)| self.get_id(h).cloned())
        .collect()
    }

    /// No hit when the ray starts inside the collider.
    pub fn cast_ray_at(
        &self,
        id: &ColliderId,
        origin: Vec3,
        direction: Vec3,
        distance: f32,
    ) -> Option<f32> {
        if !direction.is_finite() || !origin.is_finite() || distance <= 0.0 {
            return None;
        }
        let collider = self.get_collider(id)?;
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
            .cast_ray(collider.position(), &ray, f64::from(distance), true)
            .map(|toi| toi as f32)
    }

    /// The unit direction and distance that separate a ball from the collider.
    pub fn ball_penetration(
        &self,
        id: &ColliderId,
        centre: Vec3,
        radius: f32,
    ) -> Option<(Vec3, f32)> {
        let collider = self.get_collider(id)?;
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
