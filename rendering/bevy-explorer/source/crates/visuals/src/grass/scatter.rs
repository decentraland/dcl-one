//! Where a parcel's grass blades, flowers and reeds stand: unity-explorer's GPU scatter, run on
//! the CPU once per parcel. Every value comes from an integer hash of the parcel corner and a grid
//! cell, so a parcel always scatters the same way.

use bevy::math::{IVec2, Vec2};

use crate::landscape_textures::GroundGreen;

/// Metres along a parcel side.
const PARCEL_METRES: i32 = 16;
/// Instances per parcel at full density: the blade grid has one cell per metre.
pub(crate) const FULL_DENSITY: u32 = 256;
/// Flower and reed slots per parcel, on a 4 by 4 grid of 4 m cells.
const SPARSE_SLOTS: u32 = 16;
const SPARSE_AXIS: i32 = 4;
const SPARSE_CELL: i32 = PARCEL_METRES / SPARSE_AXIS;
/// Flowers grow where the ground's scatter green is below this.
const FLOWER_GREEN_BELOW: f32 = 0.08;
/// Reeds grow in the cells whose scatter green is above this, when at least two cells are.
const REED_GREEN_ABOVE: f32 = 0.1;

/// One instance: its root within the parcel, its turn and its size.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Placement {
    /// Unity x/z in metres from the parcel's corner, 0 to 16.
    pub(crate) offset: Vec2,
    /// Turn about the vertical in radians, clockwise seen from above (Unity's sense).
    pub(crate) yaw: f32,
    /// Uniform scale of the instance's mesh.
    pub(crate) scale: f32,
}

/// A parcel's instances by kind.
#[derive(Debug, Default, PartialEq)]
pub(crate) struct ScatterSet {
    pub(crate) blades: Vec<Placement>,
    pub(crate) flowers: [Vec<Placement>; 2],
    pub(crate) reeds: Vec<Placement>,
}

/// The 32-bit integer finaliser the scatter hashes with.
pub(crate) fn hash_int(mut x: u32) -> u32 {
    x ^= x >> 16;
    x = x.wrapping_mul(0x7feb_352d);
    x ^= x >> 15;
    x = x.wrapping_mul(0x846c_a68b);
    x ^= x >> 16;
    x
}

/// A point's hash as a value from 0 to 1, from two large primes and the finaliser.
pub(crate) fn hash(p: IVec2) -> f32 {
    let h = (p.x as u32)
        .wrapping_mul(73_856_093)
        .wrapping_add((p.y as u32).wrapping_mul(19_349_663));
    hash_int(h) as f32 * (1.0 / 4_294_967_296.0)
}

/// The rank of a cell in an ordered dither over a `size` square grid (a power of two): the
/// cells ranked below any count are spread evenly over the grid.
fn dither_rank(cell: IVec2, size: i32) -> u32 {
    if size == 1 {
        return 0;
    }
    let half = size / 2;
    let quadrant = [[0, 2], [3, 1]][(cell.y / half) as usize][(cell.x / half) as usize];
    4 * dither_rank(cell % half, half) + quadrant
}

/// The parcel's instances at `per_parcel` blades (0 to 256). Blades stand one per metre cell,
/// jittered within it; flowers and reeds are one per 4 m cell at full density and scale down
/// with the blades, read against `ground` (none without it).
pub(crate) fn scatter(parcel: IVec2, per_parcel: u32, ground: Option<&GroundGreen>) -> ScatterSet {
    let per_parcel = per_parcel.min(FULL_DENSITY);
    let corner = parcel * PARCEL_METRES;
    let mut set = ScatterSet::default();

    for index in 0..FULL_DENSITY as i32 {
        let cell = IVec2::new(index % PARCEL_METRES, index / PARCEL_METRES);
        if dither_rank(cell, PARCEL_METRES) >= per_parcel {
            continue;
        }
        let jitter = hash(corner + cell);
        set.blades.push(Placement {
            offset: Vec2::new(jitter, hash(-corner + cell)) + cell.as_vec2(),
            yaw: jitter * 2.0,
            scale: 1.0 + jitter,
        });
    }

    let Some(ground) = ground else {
        return set;
    };
    let sparse = SPARSE_SLOTS * per_parcel / FULL_DENSITY;
    let slots = || {
        (0..SPARSE_SLOTS as i32)
            .map(|slot| (slot, IVec2::new(slot % SPARSE_AXIS, slot / SPARSE_AXIS)))
            .filter(move |(_, cell)| dither_rank(*cell, SPARSE_AXIS) < sparse)
    };
    let green = |cell: IVec2| ground.at(corner + cell * SPARSE_CELL);

    // two flowers per non-sandy cell, each anywhere in the parcel, the second with the first's x
    // and z swapped
    for (slot, cell) in slots() {
        let grid = cell * SPARSE_CELL;
        if green(cell) >= FLOWER_GREEN_BELOW {
            continue;
        }
        let x = hash(corner + grid) * (SPARSE_CELL * SPARSE_AXIS) as f32;
        let z = hash(-corner + grid) * (SPARSE_CELL * SPARSE_AXIS) as f32;
        let yaw = hash(corner + grid) * 2.0;
        for (kind, offset) in [Vec2::new(x, z), Vec2::new(z, x)].into_iter().enumerate() {
            // the corner plus the offset plus the slot, truncated to whole metres
            let seed = (corner.as_vec2() + offset + slot as f32).as_ivec2();
            set.flowers[kind].push(Placement {
                offset,
                yaw,
                scale: 1.0 + 2.0 * hash(seed),
            });
        }
    }

    // reeds share the sandy cells out between the slots, in cell order
    let sandy: Vec<IVec2> = (0..SPARSE_SLOTS as i32)
        .map(|slot| IVec2::new(slot % SPARSE_AXIS, slot / SPARSE_AXIS))
        .filter(|cell| green(*cell) > REED_GREEN_ABOVE)
        .map(|cell| cell * SPARSE_CELL)
        .collect();
    if sandy.len() > 1 {
        let per_cell = SPARSE_SLOTS.div_ceil(sandy.len() as u32) as i32;
        for (slot, _) in slots() {
            let grid = sandy[(slot / per_cell) as usize];
            let seed = grid + slot;
            let jitter = Vec2::new(hash(seed), hash(-grid + slot)) * SPARSE_CELL as f32;
            set.reeds.push(Placement {
                offset: grid.as_vec2() + jitter,
                yaw: hash(seed),
                scale: 1.0 + 2.0 * hash(seed),
            });
        }
    }
    set
}
