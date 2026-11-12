//! The kinds of object the baked layout places, indexed by an instance's prototype byte.

pub(crate) struct Prototype {
    pub name: &'static str,
    /// Footprint in metres at scale 1. Scaled by the instance's horizontal scale, it decides
    /// whether the instance reaches into an occupied parcel and is hidden.
    pub radius: f32,
    /// Horizontal scale at an instance byte of 0 and of 255.
    pub scale_xz: (f32, f32),
    /// Vertical scale at an instance byte of 0 and of 255.
    pub scale_y: (f32, f32),
}

/// Every prototype scales uniformly, so its horizontal and vertical ranges are the same range.
const fn uniform(name: &'static str, radius: f32, scale: (f32, f32)) -> Prototype {
    Prototype {
        name,
        radius,
        scale_xz: scale,
        scale_y: scale,
    }
}

/// In the layout's prototype order.
pub(crate) const PROTOTYPES: [Prototype; 8] = [
    uniform("Bush01", 2.0, (2.0, 3.0)),
    uniform("Bush02", 2.0, (2.0, 3.0)),
    uniform("Stone01", 1.0, (1.0, 4.0)),
    uniform("Stone02", 1.0, (1.0, 4.0)),
    uniform("Stone03", 1.0, (1.0, 4.0)),
    uniform("Tree01", 10.0, (0.75, 1.25)),
    uniform("Tree02", 10.0, (0.75, 1.0)),
    uniform("Tree03", 13.0, (0.75, 1.0)),
];
