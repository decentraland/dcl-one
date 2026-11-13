//! The baked tree layout: every tree, bush and stone of a realm, grouped by parcel.
//!
//! The file is little-endian: `i32 min_x, min_z, max_x, max_z` (max exclusive), one `i32` start
//! index per parcel in z-major order, `i32 count`, then `count` 6-byte instances
//! `{prototype, pos_x, pos_z, rot_y, scale_xz, scale_y}`. A parcel's instances run from its start
//! to the next parcel's start, or to `count` for the last parcel. Coordinates in the file are
//! parcel coordinates, z growing north, which is bevy -z.

use std::fmt;

use bevy::{
    asset::{io::Reader, AssetLoader, LoadContext},
    prelude::*,
};
use common::partition::PARCEL_SIZE;

use super::prototypes::PROTOTYPES;

/// Largest width or depth, in parcels, the reference loader accepts.
const SIZE_LIMIT: i32 = 512;
/// Largest instance count the reference loader accepts.
const INSTANCE_LIMIT: i32 = 1 << 19;

#[derive(Asset, TypePath)]
pub(crate) struct TreeLayout {
    pub(super) min: IVec2,
    pub(super) max_excl: IVec2,
    /// Validated by `parse`: never negative, never decreasing, never past `instances.len()`.
    starts: Vec<i32>,
    instances: Vec<TreeInstance>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct TreeInstance {
    /// Index into `PROTOTYPES`; `parse` rejects any other value, and only `parse` builds
    /// instances, so `transform` can index the table.
    pub(super) prototype: u8,
    /// Position inside the parcel, x then z (north), in 1/255ths of the parcel's side.
    pub(super) pos: [u8; 2],
    /// Yaw in 1/255ths of a turn, clockwise seen from above (east turns towards south).
    pub(super) rot_y: u8,
    /// Horizontal and vertical scale, in 1/255ths of the prototype's range.
    pub(super) scale_xz: u8,
    pub(super) scale_y: u8,
}

#[derive(Debug)]
pub(crate) enum LayoutError {
    /// The file ends at `len` bytes but `needed` bytes are required.
    Truncated {
        needed: usize,
        len: usize,
    },
    /// The bounds are inverted or wider than `SIZE_LIMIT`.
    Bounds {
        min: IVec2,
        max_excl: IVec2,
    },
    /// The instance count is negative or above `INSTANCE_LIMIT`.
    Count(i32),
    /// A parcel's start index is negative, below the previous parcel's, or past the count.
    Start {
        parcel: usize,
        start: i32,
    },
    /// An instance names a prototype the table does not have.
    Prototype {
        instance: usize,
        prototype: u8,
    },
    /// Bytes follow the last instance.
    TrailingBytes(usize),
    Io(std::io::Error),
}

impl fmt::Display for LayoutError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Truncated { needed, len } => {
                write!(f, "tree layout truncated: needs {needed} bytes, has {len}")
            }
            Self::Bounds { min, max_excl } => {
                write!(f, "tree layout bounds {min}..{max_excl} are invalid")
            }
            Self::Count(count) => write!(f, "tree layout instance count {count} is invalid"),
            Self::Start { parcel, start } => {
                write!(
                    f,
                    "tree layout start {start} of parcel index {parcel} is invalid"
                )
            }
            Self::Prototype {
                instance,
                prototype,
            } => write!(
                f,
                "tree layout instance {instance} has unknown prototype {prototype}"
            ),
            Self::TrailingBytes(bytes) => {
                write!(f, "tree layout has {bytes} bytes after its last instance")
            }
            Self::Io(error) => write!(f, "tree layout could not be read: {error}"),
        }
    }
}

impl std::error::Error for LayoutError {}

/// Reads little-endian values off the front of the file, failing instead of running past its end.
struct Cursor<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl<'a> Cursor<'a> {
    fn take(&mut self, len: usize) -> Result<&'a [u8], LayoutError> {
        let end = self.at.checked_add(len);
        let slice = end.and_then(|end| self.bytes.get(self.at..end));
        let slice = slice.ok_or(LayoutError::Truncated {
            needed: end.unwrap_or(usize::MAX),
            len: self.bytes.len(),
        })?;
        self.at += len;
        Ok(slice)
    }

    fn i32(&mut self) -> Result<i32, LayoutError> {
        let bytes = self.take(4)?;
        Ok(i32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }
}

impl TreeLayout {
    /// Decodes a layout file. Anything truncated, out of range or inconsistent is an error, so
    /// `instances` can index without checks.
    pub fn parse(bytes: &[u8]) -> Result<Self, LayoutError> {
        let mut cursor = Cursor { bytes, at: 0 };
        let min_x = cursor.i32()?;
        let min_z = cursor.i32()?;
        let max_x = cursor.i32()?;
        let max_z = cursor.i32()?;
        let min = IVec2::new(min_x, min_z);
        let max_excl = IVec2::new(max_x, max_z);

        let in_limit = |size: Option<i32>| size.filter(|size| (0..=SIZE_LIMIT).contains(size));
        let (Some(width), Some(depth)) = (
            in_limit(max_x.checked_sub(min_x)),
            in_limit(max_z.checked_sub(min_z)),
        ) else {
            return Err(LayoutError::Bounds { min, max_excl });
        };

        let cells = width as usize * depth as usize;
        let starts: Vec<i32> = cursor
            .take(cells * 4)?
            .chunks_exact(4)
            .map(|bytes| i32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
            .collect();

        let count = cursor.i32()?;
        if !(0..=INSTANCE_LIMIT).contains(&count) {
            return Err(LayoutError::Count(count));
        }

        let mut previous = 0;
        for (parcel, &start) in starts.iter().enumerate() {
            if start < previous || start > count {
                return Err(LayoutError::Start { parcel, start });
            }
            previous = start;
        }

        let instances = cursor
            .take(count as usize * 6)?
            .chunks_exact(6)
            .enumerate()
            .map(|(instance, bytes)| {
                if usize::from(bytes[0]) >= PROTOTYPES.len() {
                    return Err(LayoutError::Prototype {
                        instance,
                        prototype: bytes[0],
                    });
                }
                Ok(TreeInstance {
                    prototype: bytes[0],
                    pos: [bytes[1], bytes[2]],
                    rot_y: bytes[3],
                    scale_xz: bytes[4],
                    scale_y: bytes[5],
                })
            })
            .collect::<Result<Vec<_>, _>>()?;

        let trailing = bytes.len() - cursor.at;
        if trailing != 0 {
            return Err(LayoutError::TrailingBytes(trailing));
        }

        Ok(Self {
            min,
            max_excl,
            starts,
            instances,
        })
    }

    /// The instances of a parcel (Decentraland parcel coordinates); empty outside the bounds.
    pub fn instances(&self, parcel: IVec2) -> &[TreeInstance] {
        if parcel.cmplt(self.min).any() || parcel.cmpge(self.max_excl).any() {
            return &[];
        }
        let offset = parcel - self.min;
        let index = offset.y as usize * (self.max_excl.x - self.min.x) as usize + offset.x as usize;
        let start = self.starts[index] as usize;
        let end = self
            .starts
            .get(index + 1)
            .map_or(self.instances.len(), |&end| end as usize);
        &self.instances[start..end]
    }

    /// The parcels the layout covers, minimum inclusive and maximum exclusive.
    pub fn bounds(&self) -> (IVec2, IVec2) {
        (self.min, self.max_excl)
    }

    /// Instances in the whole layout.
    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.instances.len()
    }
}

impl TreeInstance {
    /// World transform of the instance in `parcel`. `height(x, z)` is the ground height at bevy
    /// world x and z (z is bevy's, so south of the origin is positive).
    ///
    /// The file's frame has z north and yaws that turn +x towards -z. Bevy z is the negated file
    /// z; mirroring that axis reverses the sense of a yaw, so the rotation is about +y by the
    /// negated yaw. Scales are symmetric in x and z and need no change.
    pub fn transform(&self, parcel: IVec2, height: impl Fn(f32, f32) -> f32) -> Transform {
        let fraction = |byte: u8| byte as f32 * (1.0 / 255.0);
        let x = (parcel.x as f32 + fraction(self.pos[0])) * PARCEL_SIZE;
        let z = -((parcel.y as f32 + fraction(self.pos[1])) * PARCEL_SIZE);
        let yaw = (self.rot_y as f32 * (360.0 / 255.0)).to_radians();
        let prototype = &PROTOTYPES[usize::from(self.prototype)];
        let lerp = |(min, max): (f32, f32), byte: u8| min + fraction(byte) * (max - min);
        let scale_xz = lerp(prototype.scale_xz, self.scale_xz);
        let scale_y = lerp(prototype.scale_y, self.scale_y);
        Transform {
            translation: Vec3::new(x, height(x, z), z),
            rotation: Quat::from_rotation_y(-yaw),
            scale: Vec3::new(scale_xz, scale_y, scale_xz),
        }
    }
}

/// Whether an instance at bevy world `world_xz` (x and bevy z) is hidden: its own parcel is
/// occupied, or `radius` (the prototype's radius times the instance's horizontal scale) reaches
/// past an edge or, on both axes at once, a corner of its parcel into an occupied parcel.
/// `occupied` takes Decentraland parcel coordinates (y north). The caller counts parcels outside
/// the terrain bounds as occupied, as it does parcels whose occupancy is not yet known.
pub(crate) fn hidden_by_occupancy(
    world_xz: Vec2,
    radius: f32,
    occupied: impl Fn(IVec2) -> bool,
) -> bool {
    // parcel coordinates grow north, which is bevy -z
    let position = Vec2::new(world_xz.x, -world_xz.y);
    let parcel = (position * (1.0 / PARCEL_SIZE)).floor().as_ivec2();
    if occupied(parcel) {
        return true;
    }
    let local = position - parcel.as_vec2() * PARCEL_SIZE;
    let west = local.x < radius;
    let east = PARCEL_SIZE - local.x < radius;
    let south = local.y < radius;
    let north = PARCEL_SIZE - local.y < radius;
    let neighbour = |x: i32, y: i32| occupied(parcel + IVec2::new(x, y));
    (west && neighbour(-1, 0))
        || (east && neighbour(1, 0))
        || (south && neighbour(0, -1))
        || (north && neighbour(0, 1))
        || (west && south && neighbour(-1, -1))
        || (east && north && neighbour(1, 1))
        || (east && south && neighbour(1, -1))
        || (west && north && neighbour(-1, 1))
}

/// Loads `.trees` layout files as `TreeLayout` assets.
#[derive(Default)]
pub(crate) struct TreeLayoutLoader;

impl AssetLoader for TreeLayoutLoader {
    type Asset = TreeLayout;
    type Settings = ();
    type Error = LayoutError;

    async fn load(
        &self,
        reader: &mut dyn Reader,
        _settings: &Self::Settings,
        _load_context: &mut LoadContext<'_>,
    ) -> Result<Self::Asset, Self::Error> {
        let mut bytes = Vec::new();
        reader
            .read_to_end(&mut bytes)
            .await
            .map_err(LayoutError::Io)?;
        TreeLayout::parse(&bytes)
    }

    fn extensions(&self) -> &[&str] {
        &["trees"]
    }
}
