//! Where a scene is relative to the camera: the input to nearest-first ordering of scene
//! spawning, scene ticks and download slots.

use bevy::math::{FloatOrd, IVec2, Vec2, Vec3, Vec3Swizzles};

pub const PARCEL_SIZE: f32 = 16.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScenePartitionData {
    /// squared distance from the camera to the nearest point of the scene's nearest parcel,
    /// on the ground plane
    pub sqr_distance: f32,
    /// Chebyshev distance in parcels from the camera's parcel to the scene's nearest parcel
    pub parcel_distance: u16,
    /// every corner of every parcel is behind the camera
    pub is_behind: bool,
}

impl ScenePartitionData {
    /// a scene with no parcels is everywhere, so nearest and in front
    pub const NEAREST: Self = Self {
        sqr_distance: 0.0,
        parcel_distance: 0,
        is_behind: false,
    };

    pub fn load_priority(&self) -> LoadPriority {
        LoadPriority {
            parcel_distance: self.parcel_distance,
            is_behind: self.is_behind,
            sqr_distance: FloatOrd(self.sqr_distance),
        }
    }
}

/// The order downloads are admitted in: lower first. Parcel distance before facing, so that
/// "in front before behind" decides between scenes at the same parcel distance; raw distance
/// alone would never tie.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct LoadPriority {
    pub parcel_distance: u16,
    pub is_behind: bool,
    pub sqr_distance: FloatOrd,
}

impl LoadPriority {
    pub const TOP: Self = Self {
        parcel_distance: 0,
        is_behind: false,
        sqr_distance: FloatOrd(0.0),
    };
}

/// `camera_pos` and `camera_forward` in world space; parcel y grows toward -z.
pub fn scene_partition(
    camera_pos: Vec3,
    camera_forward: Vec3,
    parcels: impl IntoIterator<Item = IVec2>,
) -> ScenePartitionData {
    // parcel space: x east, y north, parcel `p` spans [p * 16, (p + 1) * 16)
    let flip = Vec2::new(1.0, -1.0);
    let camera = camera_pos.xz() * flip;
    let forward = camera_forward.xz() * flip;
    let camera_parcel = (camera / PARCEL_SIZE).floor().as_ivec2();

    let mut partition = ScenePartitionData {
        sqr_distance: f32::MAX,
        parcel_distance: u16::MAX,
        is_behind: true,
    };
    let mut any = false;

    for parcel in parcels {
        any = true;
        let min = parcel.as_vec2() * PARCEL_SIZE;
        let max = (parcel + 1).as_vec2() * PARCEL_SIZE;

        let nearest = camera.clamp(min, max);
        partition.sqr_distance = partition
            .sqr_distance
            .min((nearest - camera).length_squared());

        let parcel_distance = (parcel - camera_parcel).abs().max_element();
        partition.parcel_distance = partition
            .parcel_distance
            .min(parcel_distance.min(u16::MAX as i32) as u16);

        if partition.is_behind {
            let corners = [min, Vec2::new(min.x, max.y), Vec2::new(max.x, min.y), max];
            partition.is_behind = corners
                .iter()
                .all(|corner| forward.dot(*corner - camera) < 0.0);
        }
    }

    if any {
        partition
    } else {
        ScenePartitionData::NEAREST
    }
}

#[cfg(test)]
mod tests {
    use bevy::math::{FloatOrd, IVec2, Vec3};

    use super::*;

    // the centre of parcel (0,0), at eye height
    const CENTRE: Vec3 = Vec3::new(8.0, 1.5, -8.0);
    const EAST: Vec3 = Vec3::X;

    #[test]
    fn camera_over_the_scene_is_distance_zero() {
        assert_eq!(
            scene_partition(CENTRE, EAST, [IVec2::ZERO]),
            ScenePartitionData {
                sqr_distance: 0.0,
                parcel_distance: 0,
                is_behind: false
            }
        );
    }

    #[test]
    fn distance_is_to_the_nearest_edge_not_the_corner() {
        let partition = scene_partition(Vec3::new(-4.0, 0.0, -8.0), EAST, [IVec2::ZERO]);
        // the corner would be 4² + 8² = 80
        assert_eq!(partition.sqr_distance, 16.0);
        assert_eq!(partition.parcel_distance, 1);
        assert!(!partition.is_behind);
    }

    #[test]
    fn behind_when_every_corner_is_behind() {
        let partition = scene_partition(Vec3::new(40.0, 0.0, -8.0), EAST, [IVec2::ZERO]);
        assert!(partition.is_behind);
        assert_eq!(partition.sqr_distance, 576.0);
        assert_eq!(partition.parcel_distance, 2);
    }

    #[test]
    fn in_front_when_one_corner_is_in_front() {
        let partition = scene_partition(Vec3::new(12.0, 0.0, -8.0), EAST, [IVec2::ZERO]);
        assert!(!partition.is_behind);
        assert_eq!(partition.sqr_distance, 0.0);
    }

    #[test]
    fn the_nearest_parcel_of_a_scene_wins() {
        let partition = scene_partition(
            Vec3::new(90.0, 0.0, -8.0),
            Vec3::NEG_X,
            [IVec2::ZERO, IVec2::new(5, 0)],
        );
        assert_eq!(partition.sqr_distance, 0.0);
        assert_eq!(partition.parcel_distance, 0);
        assert!(!partition.is_behind);
    }

    #[test]
    fn parcel_distance_is_chebyshev() {
        let distance = |parcel| scene_partition(CENTRE, EAST, [parcel]).parcel_distance;
        assert_eq!(distance(IVec2::new(3, 2)), 3);
        assert_eq!(distance(IVec2::new(-2, 2)), 2);
        // parcel y grows toward -z: (0, -3) is three parcels south
        assert_eq!(distance(IVec2::new(0, -3)), 3);
    }

    #[test]
    fn no_parcels_is_nearest_and_in_front() {
        assert_eq!(
            scene_partition(CENTRE, EAST, []),
            ScenePartitionData::NEAREST
        );
    }

    #[test]
    fn load_priority_orders_parcel_then_facing_then_distance() {
        let key = |parcel_distance, is_behind, sqr_distance| LoadPriority {
            parcel_distance,
            is_behind,
            sqr_distance: FloatOrd(sqr_distance),
        };
        let ordered = [
            LoadPriority::TOP,
            key(0, true, 0.0),
            key(1, false, 1000.0),
            key(1, true, 0.0),
            key(2, false, 0.0),
        ];
        for pair in ordered.windows(2) {
            assert!(
                pair[0] < pair[1],
                "{:?} should sort before {:?}",
                pair[0],
                pair[1]
            );
        }
        assert_eq!(
            ScenePartitionData::NEAREST.load_priority(),
            LoadPriority::TOP
        );
    }
}
