//! The kinds of object the baked layout places, indexed by an instance's prototype byte.

use bevy::math::Vec3;

use super::material::Surface::{self, *};

pub(crate) struct Prototype {
    pub name: &'static str,
    /// File stem of the model under `landscape/trees/`, and of its impostor under
    /// `landscape/impostors/`.
    pub asset: &'static str,
    /// The model's material slots, in order.
    pub surfaces: &'static [Surface],
    /// Footprint in metres at scale 1. Scaled by the instance's horizontal scale, it decides
    /// whether the instance reaches into an occupied parcel and is hidden.
    pub radius: f32,
    /// Horizontal scale at an instance byte of 0 and of 255.
    pub scale_xz: (f32, f32),
    /// Vertical scale at an instance byte of 0 and of 255.
    pub scale_y: (f32, f32),
    /// The model's bounds at scale 1, in its own space: the lowest and highest corner.
    pub bounds: (Vec3, Vec3),
    pub lod: LodGroup,
}

/// The reference's level-of-detail group of the prototype: the screen heights (the share of the
/// view's height the group's size covers) at which it changes what it draws.
pub(crate) struct LodGroup {
    /// The point distances are measured to, in the model's space.
    pub reference: Vec3,
    /// The group's size in metres at scale 1.
    pub size: f32,
    /// The full mesh draws at and above this screen height.
    pub mesh: f32,
    /// The impostor draws at and above this screen height, below the mesh's. Without one the
    /// prototype is culled below the mesh's.
    pub impostor: Option<f32>,
}

impl Prototype {
    pub(crate) fn centre(&self) -> Vec3 {
        (self.bounds.0 + self.bounds.1) * 0.5
    }

    pub(crate) fn size(&self) -> Vec3 {
        self.bounds.1 - self.bounds.0
    }
}

/// Every prototype scales uniformly, so its horizontal and vertical ranges are the same range.
const fn uniform(
    name: &'static str,
    asset: &'static str,
    surfaces: &'static [Surface],
    radius: f32,
    scale: (f32, f32),
    bounds: (Vec3, Vec3),
    lod: LodGroup,
) -> Prototype {
    Prototype {
        name,
        asset,
        surfaces,
        radius,
        scale_xz: scale,
        scale_y: scale,
        bounds,
        lod,
    }
}

/// A level-of-detail group as the reference stores it: the reference point in its frame (x east,
/// z north, so z is negated here), the size, and the lowest screen height of the mesh and of
/// the impostor.
const fn lod(reference: [f32; 3], size: f32, mesh: f32, impostor: Option<f32>) -> LodGroup {
    LodGroup {
        reference: Vec3::new(reference[0], reference[1], -reference[2]),
        size,
        mesh,
        impostor,
    }
}

const fn bounds(min: [f32; 3], max: [f32; 3]) -> (Vec3, Vec3) {
    (
        Vec3::new(min[0], min[1], min[2]),
        Vec3::new(max[0], max[1], max[2]),
    )
}

/// In the layout's prototype order. A stone's group has three meshes, which all draw the one
/// model here, so its mesh threshold is the last of the three.
pub(crate) const PROTOTYPES: [Prototype; 8] = [
    uniform(
        "Bush01",
        "bush01",
        &[LeafBushGreenDark],
        2.0,
        (2.0, 3.0),
        bounds([-0.73237, -0.39732, -0.57792], [0.66514, 0.64056, 0.68817]),
        lod(
            [-0.033614576, 0.12162286, -0.05512464],
            1.3975023,
            0.077743225,
            Some(0.0012675737),
        ),
    ),
    uniform(
        "Bush02",
        "bush02",
        &[LeafBushGreen, Trunk],
        2.0,
        (2.0, 3.0),
        bounds([-0.64222, -0.07378, -0.57108], [0.74117, 1.45790, 0.93536]),
        lod(
            [0.04947543, 0.69205713, -0.18213958],
            1.5316815,
            0.086567685,
            Some(0.0025430839),
        ),
    ),
    uniform(
        "Stone01",
        "stone01",
        &[Rock01],
        1.0,
        (1.0, 4.0),
        bounds([-0.91136, -0.99551, -0.91199], [0.91136, 0.99551, 0.91199]),
        lod(
            [-0.0029865205, 0.0056033134, 0.001739651],
            2.0203369,
            0.021002172,
            None,
        ),
    ),
    uniform(
        "Stone02",
        "stone02",
        &[Rock02],
        1.0,
        (1.0, 4.0),
        bounds([-0.63233, -0.94767, -0.65405], [0.63233, 0.94767, 0.65405]),
        lod(
            [-0.0047525465, -0.005103737, 0.01731342],
            1.9055371,
            0.021002172,
            None,
        ),
    ),
    uniform(
        "Stone03",
        "stone03",
        &[Rock03],
        1.0,
        (1.0, 4.0),
        bounds([-0.78184, -0.86630, -0.68400], [0.78184, 0.86630, 0.68400]),
        lod(
            [0.012681067, 0.0, 0.0061148107],
            1.7325934,
            0.021002172,
            None,
        ),
    ),
    uniform(
        "Tree01",
        "tree01",
        &[LeafYellow, Trunk],
        10.0,
        (0.75, 1.25),
        bounds(
            [-9.71206, -0.54140, -10.12533],
            [12.00655, 16.61907, 7.13523],
        ),
        lod(
            [1.1472416, 8.0388365, 1.4950495],
            21.718613,
            0.2036845,
            Some(0.019613095),
        ),
    ),
    uniform(
        "Tree02",
        "tree02",
        &[LeafPink, Trunk],
        10.0,
        (0.75, 1.0),
        bounds(
            [-12.38087, -0.59927, -8.01219],
            [9.32198, 19.49918, 10.95658],
        ),
        lod(
            [-1.5294409, 9.449959, -1.4721947],
            21.70285,
            0.203304,
            Some(0.020316),
        ),
    ),
    uniform(
        "Tree03",
        "tree03",
        &[LeafGreen, Trunk],
        13.0,
        (0.75, 1.0),
        bounds(
            [-13.39215, -3.71510, -10.77876],
            [11.39806, 37.4105, 14.44978],
        ),
        lod(
            [-0.9970484, 16.8477, -1.8355103],
            41.125607,
            0.24983539,
            Some(0.036636002),
        ),
    ),
];
