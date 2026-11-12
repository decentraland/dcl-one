//! Where a scene, or an avatar, is relative to the player and the camera: the input to
//! nearest-first ordering of scene ticks and download slots.

use bevy::math::{FloatOrd, IVec2, Vec2, Vec3, Vec3Swizzles};

pub const PARCEL_SIZE: f32 = 16.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PartitionData {
    /// squared distance from the player to the nearest point of the scene's nearest parcel, or
    /// to the point, on the ground plane
    pub sqr_distance: f32,
    /// Chebyshev distance in parcels from the player's parcel to the scene's nearest parcel, or
    /// to the point's parcel
    pub parcel_distance: u16,
    /// every corner of every parcel, or the point, is behind the camera
    pub is_behind: bool,
}

impl PartitionData {
    /// a scene with no parcels is everywhere, so nearest and in front
    pub const NEAREST: Self = Self {
        sqr_distance: 0.0,
        parcel_distance: 0,
        is_behind: false,
    };

    pub fn load_priority(&self, tier: Tier) -> LoadPriority {
        LoadPriority {
            tier,
            parcel_distance: self.parcel_distance,
            is_behind: self.is_behind,
            sqr_distance: FloatOrd(self.sqr_distance),
        }
    }
}

/// The band a download belongs to; the first thing the request-slot queue orders by. Within a
/// band the partition decides.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Tier {
    /// the scenes the player stands in, portables and global scenes, the player's own body,
    /// wearables and emotes, the avatar previews the player opens (the backpack, a passport), and
    /// files every scene runtime needs (the SDK6 adaption layer)
    Active,
    /// other avatars' bodies and wearables: all before any other scene, nearest avatar first
    Avatar,
    /// the other scenes in range, nearest first
    Scene,
    /// no scene's and no avatar's: NFT images, URL-form fonts, imposter archives, other
    /// avatars' emotes
    Other,
    /// a scene that has left while its downloads wait
    Departed,
}

/// The order downloads are admitted in: lower first. The tier first; then parcel distance
/// before facing, so that "in front before behind" decides between scenes at the same parcel
/// distance, where raw distance alone would never tie.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct LoadPriority {
    pub tier: Tier,
    pub parcel_distance: u16,
    pub is_behind: bool,
    pub sqr_distance: FloatOrd,
}

impl LoadPriority {
    /// The player's scenes.
    pub const TOP: Self = Self {
        tier: Tier::Active,
        parcel_distance: 0,
        is_behind: false,
        sqr_distance: FloatOrd(0.0),
    };

    /// After every live scene: a request that is no scene's and no avatar's.
    pub const OTHER: Self = Self {
        tier: Tier::Other,
        parcel_distance: 0,
        is_behind: false,
        sqr_distance: FloatOrd(0.0),
    };

    /// Last: a scene that has left while its downloads wait.
    pub const BOTTOM: Self = Self {
        tier: Tier::Departed,
        parcel_distance: 0,
        is_behind: false,
        sqr_distance: FloatOrd(0.0),
    };

    /// The bottom of the scene tier: a scene that has no partition yet (spawned, no context).
    pub const SCENE_PENDING: Self = Self {
        tier: Tier::Scene,
        parcel_distance: u16::MAX,
        is_behind: true,
        sqr_distance: FloatOrd(f32::MAX),
    };

    /// The bottom of the avatar tier: an avatar that has no partition yet.
    pub const AVATAR_PENDING: Self = Self {
        tier: Tier::Avatar,
        parcel_distance: u16::MAX,
        is_behind: true,
        sqr_distance: FloatOrd(f32::MAX),
    };
}

/// `origin` is where distance is measured from (the player); `camera` is the position and
/// forward the behind test uses (the rendering camera), or `None` when nothing renders, and
/// then nothing is behind. World space; parcel y grows toward -z.
pub fn scene_partition(
    origin: Vec3,
    camera: Option<(Vec3, Vec3)>,
    parcels: impl IntoIterator<Item = IVec2>,
) -> PartitionData {
    // parcel space: x east, y north, parcel `p` spans [p * 16, (p + 1) * 16)
    let flip = Vec2::new(1.0, -1.0);
    let origin = origin.xz() * flip;
    let origin_parcel = (origin / PARCEL_SIZE).floor().as_ivec2();
    let camera = camera.map(|(position, forward)| (position.xz() * flip, forward.xz() * flip));

    let mut partition = PartitionData {
        sqr_distance: f32::MAX,
        parcel_distance: u16::MAX,
        is_behind: camera.is_some(),
    };
    let mut any = false;

    for parcel in parcels {
        any = true;
        let min = parcel.as_vec2() * PARCEL_SIZE;
        let max = (parcel + 1).as_vec2() * PARCEL_SIZE;

        let nearest = origin.clamp(min, max);
        partition.sqr_distance = partition
            .sqr_distance
            .min((nearest - origin).length_squared());

        let parcel_distance = (parcel - origin_parcel).abs().max_element();
        partition.parcel_distance = partition
            .parcel_distance
            .min(parcel_distance.min(u16::MAX as i32) as u16);

        if let Some((camera, forward)) = camera {
            if partition.is_behind {
                let corners = [min, Vec2::new(min.x, max.y), Vec2::new(max.x, min.y), max];
                partition.is_behind = corners
                    .iter()
                    .all(|corner| forward.dot(*corner - camera) < 0.0);
            }
        }
    }

    if any {
        partition
    } else {
        PartitionData::NEAREST
    }
}

/// The partition of a point, an avatar's position: distance from `origin` (the player), the
/// ring of its parcel, and whether it is behind `camera` (the rendering camera); with no camera
/// nothing is behind.
pub fn point_partition(origin: Vec3, camera: Option<(Vec3, Vec3)>, point: Vec3) -> PartitionData {
    let flip = Vec2::new(1.0, -1.0);
    let origin = origin.xz() * flip;
    let point = point.xz() * flip;
    let origin_parcel = (origin / PARCEL_SIZE).floor().as_ivec2();
    let point_parcel = (point / PARCEL_SIZE).floor().as_ivec2();
    PartitionData {
        sqr_distance: (point - origin).length_squared(),
        parcel_distance: (point_parcel - origin_parcel)
            .abs()
            .max_element()
            .min(u16::MAX as i32) as u16,
        is_behind: camera.is_some_and(|(position, forward)| {
            let forward = forward.xz() * flip;
            forward.dot(point - position.xz() * flip) < 0.0
        }),
    }
}

/// The content hashes behind a set of collectible urns, each once, sorted: `resolve` gives a
/// urn's hash once its pointer is known. What an avatar's downloads are filed under.
pub fn resolved_hashes<'a>(
    urns: impl IntoIterator<Item = &'a str>,
    resolve: impl FnMut(&'a str) -> Option<String>,
) -> Vec<String> {
    let mut hashes: Vec<String> = urns.into_iter().filter_map(resolve).collect();
    hashes.sort();
    hashes.dedup();
    hashes
}

#[cfg(test)]
mod tests;
