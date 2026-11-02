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

    /// After every live scene: the key of a scene that has left while its downloads wait.
    pub const BOTTOM: Self = Self {
        parcel_distance: u16::MAX,
        is_behind: true,
        sqr_distance: FloatOrd(f32::INFINITY),
    };
}

/// `camera_pos` and `camera_forward` in world space; parcel y grows toward -z. A zero
/// `camera_forward` means no facing is known (no rendering camera), and nothing is behind.
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
mod tests;
