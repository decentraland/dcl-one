use bevy::math::{DVec2, IVec2};
use common::partition::PARCEL_SIZE;
use rapier3d_f64::prelude::Collider;
use scene_material::BoundRegion;

/// How far a collider's box may reach past the edge of its scene, in metres.
const EDGE_ALLOWANCE: f64 = 0.05;
/// The box is first drawn in on each side by a quarter of its smaller horizontal size, and by
/// this at most, in metres.
const MAX_INSET: f64 = 0.15;

/// The space a scene's colliders may occupy: the rectangle around its parcels, up to one height
/// limit. Without limits the scene has no bounds.
///
/// The rectangle, not the parcels themselves: a collider is on or off as a whole, and one large
/// collider often covers an irregular scene's footprint, the notches included.
#[derive(Default)]
pub(super) struct ColliderBounds(Option<Limits>);

struct Limits {
    /// world xz, the edge allowance included
    min: DVec2,
    max: DVec2,
    height: f64,
}

impl ColliderBounds {
    /// The limits of a scene of `parcels` parcels in `regions`.
    pub(super) fn new(regions: &[BoundRegion], parcels: usize) -> Self {
        if regions.is_empty() {
            return Self(None);
        }

        let mut min = IVec2::MAX;
        let mut max = IVec2::MIN;
        let mut height = 0.0f32;
        for region in regions {
            let (region_min, region_max) = region.cells();
            min = min.min(region_min);
            max = max.max(region_max);
            height = height.max(region.height);
        }
        // one limit for the whole scene: that of its parcel count, or a higher one its regions
        // carry
        let height = height.max(BoundRegion::height_for_parcels(parcels as u32));

        let parcel_size = f64::from(PARCEL_SIZE);
        Self(Some(Limits {
            min: min.as_dvec2() * parcel_size - EDGE_ALLOWANCE,
            max: (max + IVec2::ONE).as_dvec2() * parcel_size + EDGE_ALLOWANCE,
            height: f64::from(height),
        }))
    }

    pub(super) fn height(&self) -> Option<f64> {
        self.0.as_ref().map(|limits| limits.height)
    }

    /// Whether a collider lies inside the scene: its world-space box within the rectangle and
    /// no higher than the limit. A box that leaves is outside as a whole.
    pub(super) fn contains(&self, collider: &Collider) -> bool {
        let Some(limits) = &self.0 else {
            return true;
        };

        let aabb = collider.compute_aabb();
        let min = DVec2::new(aabb.mins.x, aabb.mins.z);
        let max = DVec2::new(aabb.maxs.x, aabb.maxs.z);
        let inset = ((max - min).min_element() * 0.25).min(MAX_INSET);

        (min + inset).cmpge(limits.min).all()
            && (max - inset).cmple(limits.max).all()
            && aabb.maxs.y <= limits.height
    }
}
