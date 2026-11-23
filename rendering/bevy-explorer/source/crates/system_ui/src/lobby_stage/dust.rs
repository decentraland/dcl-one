use bevy::{
    prelude::*,
    render::{
        mesh::{Indices, PrimitiveTopology},
        render_asset::RenderAssetUsages,
    },
};

use super::materials::DustUniform;

/// In the stage's frame.
pub const EMITTER_CENTRE: Vec3 = Vec3::new(0.0, 1.9, -3.0);
pub const EMITTER_SIZE: Vec3 = Vec3::new(26.0, 6.0, 16.0);
/// Seconds; each slot lives this long, then respawns somewhere else.
const LIFETIME: (f32, f32) = (6.0, 10.0);
/// Motes alive at once: about 27 spawned a second, living about 8 s each.
pub const SLOTS: u32 = 220;
/// Metres per second, along +z.
const SPEED: (f32, f32) = (0.02, 0.06);
/// Metres across.
const SIZE: (f32, f32) = (0.015, 0.045);
/// The drift: metres per second at most, the noise's cells per metre, and how fast it scrolls.
const NOISE_STRENGTH: f32 = 0.05;
const NOISE_FREQUENCY: f32 = 0.25;
const NOISE_SCROLL: f32 = 0.05;

pub fn uniform(tint: LinearRgba, color: LinearRgba, softness: f32) -> DustUniform {
    DustUniform {
        color: tint.to_vec4() * color.to_vec4(),
        emitter_centre: EMITTER_CENTRE.extend(0.0),
        emitter_size: EMITTER_SIZE.extend(0.0),
        lifetime_speed: Vec4::new(LIFETIME.0, LIFETIME.1, SPEED.0, SPEED.1),
        size_noise: Vec4::new(SIZE.0, SIZE.1, NOISE_STRENGTH, NOISE_FREQUENCY),
        noise_scroll: NOISE_SCROLL,
        softness,
    }
}

/// One quad per slot: `position.xy` is the corner (-0.5..0.5), `position.z` the slot.
pub fn mesh() -> Mesh {
    let corners = [[-0.5, -0.5], [0.5, -0.5], [0.5, 0.5], [-0.5, 0.5]];
    let mut positions = Vec::with_capacity(SLOTS as usize * 4);
    let mut indices = Vec::with_capacity(SLOTS as usize * 6);
    for slot in 0..SLOTS {
        let base = positions.len() as u32;
        positions.extend(corners.map(|[x, y]| [x, y, slot as f32]));
        indices.extend([base, base + 1, base + 2, base, base + 2, base + 3]);
    }
    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::RENDER_WORLD,
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_indices(Indices::U32(indices));
    mesh
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mesh_has_a_quad_per_slot() {
        let mesh = mesh();
        assert_eq!(mesh.count_vertices(), SLOTS as usize * 4);
        let Some(Indices::U32(indices)) = mesh.indices() else {
            panic!("u32 indices");
        };
        assert_eq!(indices.len(), SLOTS as usize * 6);
    }

    #[test]
    fn uniform_carries_the_emitter() {
        let u = uniform(LinearRgba::WHITE, LinearRgba::new(0.5, 0.5, 0.5, 0.55), 1.0);
        assert_eq!(u.emitter_centre.truncate(), EMITTER_CENTRE);
        assert_eq!(u.emitter_size.truncate(), EMITTER_SIZE);
        assert!((u.color.w - 0.55).abs() < 1e-6);
        assert!((u.color.x - 0.5).abs() < 1e-6);
    }
}
