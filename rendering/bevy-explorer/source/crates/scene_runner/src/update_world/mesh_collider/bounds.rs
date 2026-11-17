use bevy::math::{DVec2, I64Vec2};
use common::partition::PARCEL_SIZE;
use rapier3d_f64::parry::bounding_volume::Aabb;
use scene_material::BoundRegion;

/// How far a collider's box may reach past the edge of its scene, in metres.
const EDGE_ALLOWANCE: f64 = 0.05;
/// The box is first drawn in on each side by a quarter of its smaller horizontal size, and by
/// this at most, in metres.
const MAX_INSET: f64 = 0.15;

/// The space a scene's colliders may occupy: its parcels as disjoint rectangles of parcel cells
/// (world xz / 16, inclusive), each with the height limit of its region. Without rectangles
/// there is no limit.
#[derive(Default)]
pub(super) struct ColliderBounds {
    rects: Vec<(I64Vec2, I64Vec2, f64)>,
}

impl ColliderBounds {
    pub(super) fn new(regions: &[BoundRegion]) -> Self {
        Self {
            rects: regions
                .iter()
                .map(|region| {
                    let (min, max) = region.cells();
                    (min.as_i64vec2(), max.as_i64vec2(), f64::from(region.height))
                })
                .collect(),
        }
    }

    /// Whether a collider with this world-space box lies inside the scene: over its parcels
    /// only and no higher than their limit. A box that leaves is outside as a whole.
    pub(super) fn contains(&self, aabb: &Aabb) -> bool {
        if self.rects.is_empty() {
            return true;
        }

        let min = DVec2::new(aabb.mins.x, aabb.mins.z);
        let max = DVec2::new(aabb.maxs.x, aabb.maxs.z);
        let centre = (min + max) * 0.5;
        let inset = ((max - min).min_element() * 0.25).min(MAX_INSET) + EDGE_ALLOWANCE;
        // a box thinner than twice the inset counts as its centre line
        let low = (min + inset).min(centre);
        let high = (max - inset).max(centre);

        // through i32, so a box of any size leaves cell numbers that subtract without overflow
        let parcel_size = f64::from(PARCEL_SIZE);
        let first = (low / parcel_size).floor().as_ivec2().as_i64vec2();
        let last = ((high / parcel_size).ceil().as_ivec2().as_i64vec2() - I64Vec2::ONE).max(first);
        let span = last - first + I64Vec2::ONE;
        let Some(cells) = span.x.checked_mul(span.y) else {
            return false;
        };

        let mut covered = 0;
        let mut height = f64::INFINITY;
        for (rect_min, rect_max, rect_height) in &self.rects {
            let overlap = last.min(*rect_max) - first.max(*rect_min) + I64Vec2::ONE;
            if overlap.cmpgt(I64Vec2::ZERO).all() {
                covered += overlap.x * overlap.y;
                height = height.min(*rect_height);
            }
        }

        covered == cells && aabb.maxs.y <= height
    }
}
