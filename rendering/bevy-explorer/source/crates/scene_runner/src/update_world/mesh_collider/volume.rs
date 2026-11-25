use bevy::math::{DVec2, DVec3, IVec2, Vec3Swizzles};
use itertools::Either;
use scene_material::BoundRegion;

use super::SCENE_EDGE_MARGIN;

/// How far past its parcels and above its height limit a scene counts as drawn, in metres: the
/// hard edge of the scene materials (`bound_material.wgsl`). With the out-of-bounds effect on
/// they dissolve what lies past it over a wider band; colliders keep to this edge.
pub(super) const DRAWN_PAST_BOUNDS: f64 = 0.05;

/// The space a scene is drawn in: over its parcels, up to the height limit of each connected
/// group of them. Without regions the scene is drawn everywhere.
#[derive(Default)]
pub(super) struct SceneVolume(Vec<Block>);

/// One rectangle of a scene's parcels, [`DRAWN_PAST_BOUNDS`] included. It has no floor.
struct Block {
    /// world xz
    min: DVec2,
    max: DVec2,
    top: f64,
}

/// A part of a sweep that lies inside a [`SceneVolume`], in units of the sweep's direction.
pub(super) struct Stretch {
    pub(super) start: f64,
    pub(super) end: f64,
    /// The outward normal of the face of the volume the sweep comes in through. None when the
    /// sweep starts inside.
    pub(super) entered: Option<DVec3>,
}

impl SceneVolume {
    pub(super) fn new(regions: &[BoundRegion]) -> Self {
        Self(
            regions
                .iter()
                .map(|region| {
                    let height = f64::from(region.world_max().y);
                    Block::over(region, height + DRAWN_PAST_BOUNDS)
                })
                .collect(),
        )
    }

    /// Whether a shape touches the volume: one whose lowest point is `bottom` and that reaches
    /// `radius` to each side of it.
    pub(super) fn reaches(&self, bottom: DVec3, radius: f64) -> bool {
        self.touches(bottom, DVec3::ZERO, 0.0, radius)
    }

    /// Whether a sweep touches the volume at all: whether it has any of [`Self::stretches`].
    pub(super) fn touches(
        &self,
        bottom: DVec3,
        direction: DVec3,
        distance: f64,
        radius: f64,
    ) -> bool {
        self.0.is_empty()
            || self
                .0
                .iter()
                .any(|block| block.span(bottom, direction, distance, radius).is_some())
    }

    /// The stretches of a sweep during which the swept shape touches the volume, in order. The
    /// shape's lowest point moves from `bottom` along `direction` for up to `distance`, and the
    /// shape reaches `radius` to each side of it: none for a ray.
    pub(super) fn stretches(
        &self,
        bottom: DVec3,
        direction: DVec3,
        distance: f64,
        radius: f64,
    ) -> impl Iterator<Item = Stretch> {
        if self.0.is_empty() {
            return Either::Left(std::iter::once(Stretch {
                start: 0.0,
                end: distance,
                entered: None,
            }));
        }

        let mut spans = self
            .0
            .iter()
            .filter_map(|block| block.span(bottom, direction, distance, radius))
            .collect::<Vec<_>>();
        spans.sort_by(|a, b| a.start.total_cmp(&b.start));

        // neighbouring blocks overlap by what is drawn past each, so a sweep from one into the
        // next is one stretch
        let mut stretches: Vec<Stretch> = Vec::with_capacity(spans.len());
        for span in spans {
            match stretches.last_mut() {
                Some(last) if span.start <= last.end => last.end = last.end.max(span.end),
                _ => stretches.push(span),
            }
        }
        Either::Right(stretches.into_iter())
    }
}

/// The distance along a sweep from which a scene on `parcel` can answer it, if it can. That is
/// where the swept shape first touches the space the parcel is drawn in, whatever the height its
/// scene is drawn to ([`SceneVolume::stretches`] for that parcel), less the margin by which a
/// stop at the scene's edge is reported before it.
pub(crate) fn parcel_answer_distance(
    parcel: IVec2,
    bottom: DVec3,
    direction: DVec3,
    distance: f64,
    radius: f64,
) -> Option<f64> {
    Block::over(&BoundRegion::new(parcel, parcel, 1), f64::INFINITY)
        .span(bottom, direction, distance, radius)
        .map(|stretch| match stretch.entered {
            Some(edge) => (stretch.start - SCENE_EDGE_MARGIN / direction.dot(edge).abs()).max(0.0),
            None => stretch.start,
        })
}

impl Block {
    /// The block over `region`, up to `top`.
    fn over(region: &BoundRegion, top: f64) -> Self {
        Self {
            min: region.world_min().as_dvec3().xz() - DRAWN_PAST_BOUNDS,
            max: region.world_max().as_dvec3().xz() + DRAWN_PAST_BOUNDS,
            top,
        }
    }

    /// The part of a sweep during which the swept shape touches the block. The shape counts as
    /// a square of `radius` to each side, which is at most 0.4 `radius` generous at a corner.
    fn span(&self, bottom: DVec3, direction: DVec3, distance: f64, radius: f64) -> Option<Stretch> {
        let mut stretch = Stretch {
            start: 0.0,
            end: distance,
            entered: None,
        };

        // per axis: where the point is, how fast it moves, and the range it has to be in
        let sides = [
            (
                bottom.x,
                direction.x,
                self.min.x - radius,
                self.max.x + radius,
                DVec3::X,
            ),
            (
                bottom.z,
                direction.z,
                self.min.y - radius,
                self.max.y + radius,
                DVec3::Z,
            ),
            (bottom.y, direction.y, f64::NEG_INFINITY, self.top, DVec3::Y),
        ];
        for (position, speed, low, high, axis) in sides {
            if speed == 0.0 {
                if position < low || position > high {
                    return None;
                }
                continue;
            }
            let (near, far, face) = if speed > 0.0 {
                ((low - position) / speed, (high - position) / speed, -axis)
            } else {
                ((high - position) / speed, (low - position) / speed, axis)
            };
            if near > stretch.start {
                stretch.start = near;
                stretch.entered = Some(face);
            }
            stretch.end = stretch.end.min(far);
        }

        (stretch.start <= stretch.end).then_some(stretch)
    }
}
