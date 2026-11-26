//! The grass, flower and reed meshes, built in code. Each is drawn many thousand times, so each is
//! a handful of double-sided triangles: a grass tuft is two crossed cards cut out by the blade
//! mask (`mask.rs`), a flower or a reed an own low-poly lookalike. The shader colours them by part
//! and bends them by height.

use std::f32::consts::TAU;

use bevy::math::{Vec2, Vec3};

use super::mask;

/// Parts, which the shader colours (`grass_wind.wgsl`'s `part_colour`).
/// A grass card, cut out by the blade mask and coloured from the ground under it.
pub(crate) const PART_BLADE: f32 = 0.0;
/// A green stem or leaf.
pub(crate) const PART_STEM: f32 = 1.0;
pub(crate) const PART_DAISY_PETAL: f32 = 2.0;
pub(crate) const PART_DAISY_HEART: f32 = 3.0;
pub(crate) const PART_BELL_PETAL: f32 = 4.0;
pub(crate) const PART_BELL_HEART: f32 = 5.0;
pub(crate) const PART_REED_HEAD: f32 = 6.0;

/// One instance's triangles at scale 1, its root at the origin and y up.
#[derive(Debug, Default)]
pub(crate) struct Shape {
    pub(crate) positions: Vec<Vec3>,
    /// Per vertex: its height as a share of the instance's (0 at the root, 1 at the top), and
    /// its part.
    pub(crate) parts: Vec<[f32; 2]>,
    /// Per vertex: where it reads the blade mask. Only a grass card reads it.
    pub(crate) mask_uvs: Vec<[f32; 2]>,
    pub(crate) indices: Vec<u16>,
}

impl Shape {
    fn vertex(&mut self, position: Vec3, height: f32, part: f32) -> u16 {
        self.masked_vertex(position, height, part, [0.0; 2])
    }

    fn masked_vertex(&mut self, position: Vec3, height: f32, part: f32, mask_uv: [f32; 2]) -> u16 {
        self.positions.push(position);
        self.mask_uvs.push(mask_uv);
        self.parts
            .push([(position.y / height).clamp(0.0, 1.0), part]);
        (self.positions.len() - 1) as u16
    }

    fn triangle(&mut self, a: u16, b: u16, c: u16) {
        self.indices.extend_from_slice(&[a, b, c]);
    }
}

/// Horizontal direction at `turns` of a full turn.
fn around(turns: f32) -> Vec3 {
    let (sin, cos) = (turns * TAU).sin_cos();
    Vec3::new(cos, 0.0, sin)
}

/// A tuft built as unity-explorer's grass card: two crossed quads 0.64 m wide and 0.27 m tall,
/// each showing one strip of the blade mask. 8 vertices and 4 triangles.
pub(crate) fn blade_mesh() -> Shape {
    const HEIGHT: f32 = 0.27;
    const HALF_WIDTH: f32 = 0.32;
    // the second quad's turn from the first (55.5 degrees)
    const QUAD_TURN: f32 = 0.154;
    let mut shape = Shape::default();
    for (strip, turns) in [0.0, QUAD_TURN].into_iter().enumerate() {
        let along = around(turns) * HALF_WIDTH;
        // bottom left, bottom right, top left, top right
        let corners = [(-1.0, 0.0), (1.0, 0.0), (-1.0, 1.0), (1.0, 1.0)].map(|(side, up)| {
            shape.masked_vertex(
                along * side + Vec3::Y * HEIGHT * up,
                HEIGHT,
                PART_BLADE,
                mask::uv(strip, side * 0.5 + 0.5, up),
            )
        });
        shape.triangle(corners[0], corners[1], corners[2]);
        shape.triangle(corners[2], corners[1], corners[3]);
    }
    shape
}

/// A thin stem, one triangle from the root to `height`.
fn stem(shape: &mut Shape, height: f32, top: f32) {
    const HALF_WIDTH: f32 = 0.008;
    let left = shape.vertex(Vec3::X * -HALF_WIDTH, height, PART_STEM);
    let right = shape.vertex(Vec3::X * HALF_WIDTH, height, PART_STEM);
    let tip = shape.vertex(Vec3::Y * top, height, PART_STEM);
    shape.triangle(left, right, tip);
}

/// A flower on a stem: kind 0 a white daisy, five petals around a yellow heart, its face tipped
/// toward the viewer's side; kind 1 a violet bell, four petals hanging from a dark heart.
/// Nine or eight vertices, six or five triangles.
pub(crate) fn flower_mesh(kind: u8) -> Shape {
    const HEIGHT: f32 = 0.22;
    let mut shape = Shape::default();
    stem(&mut shape, HEIGHT, HEIGHT);
    let heart_at = Vec3::Y * HEIGHT;
    let (petals, radius, drop, heart_part, petal_part) = match kind {
        0 => (5, 0.05, -0.01, PART_DAISY_HEART, PART_DAISY_PETAL),
        _ => (4, 0.035, 0.04, PART_BELL_HEART, PART_BELL_PETAL),
    };
    // a daisy's face tips 30 degrees about x so it shows from the side
    let tip = |p: Vec3| {
        if kind == 0 {
            let (sin, cos) = 30f32.to_radians().sin_cos();
            Vec3::new(p.x, p.y * cos - p.z * sin, p.y * sin + p.z * cos)
        } else {
            p
        }
    };
    let heart = shape.vertex(heart_at, HEIGHT, heart_part);
    let rim: Vec<u16> = (0..petals)
        .map(|i| {
            let offset = around(i as f32 / petals as f32) * radius - Vec3::Y * drop;
            shape.vertex(heart_at + tip(offset), HEIGHT, petal_part)
        })
        .collect();
    for i in 0..petals {
        shape.triangle(heart, rim[i], rim[(i + 1) % petals]);
    }
    shape
}

/// A reed: a stem, one leaf leaning away, and the brown seed head near the top, a closed
/// double cone around the stem. Twelve vertices and ten triangles.
pub(crate) fn cattail_mesh() -> Shape {
    const HEIGHT: f32 = 0.7;
    const HEAD_BOTTOM: f32 = 0.46;
    const HEAD_MIDDLE: f32 = 0.56;
    const HEAD_TOP: f32 = 0.66;
    const HEAD_RADIUS: f32 = 0.02;
    let mut shape = Shape::default();
    stem(&mut shape, HEIGHT, HEIGHT);
    let leaf_base = [Vec2::new(0.0, 0.015), Vec2::new(0.02, -0.01)]
        .map(|p| shape.vertex(Vec3::new(p.x, 0.0, p.y), HEIGHT, PART_STEM));
    let leaf_tip = shape.vertex(Vec3::new(0.12, 0.55, 0.04), HEIGHT, PART_STEM);
    shape.triangle(leaf_base[0], leaf_base[1], leaf_tip);
    let bottom = shape.vertex(Vec3::Y * HEAD_BOTTOM, HEIGHT, PART_REED_HEAD);
    let top = shape.vertex(Vec3::Y * HEAD_TOP, HEIGHT, PART_REED_HEAD);
    let ring: Vec<u16> = (0..4)
        .map(|i| {
            let p = around(i as f32 / 4.0) * HEAD_RADIUS + Vec3::Y * HEAD_MIDDLE;
            shape.vertex(p, HEIGHT, PART_REED_HEAD)
        })
        .collect();
    for i in 0..4 {
        let next = (i + 1) % 4;
        shape.triangle(bottom, ring[next], ring[i]);
        shape.triangle(top, ring[i], ring[next]);
    }
    shape
}
