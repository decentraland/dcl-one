//! Unity's ocean waves: the Stylized Water 2 wave sum
//! (unity:Explorer/Library/PackageCache/decentraland.rendergraphs@*/Ocean/Shaders/Libraries/Waves.hlsl)
//! with the values of unity:Explorer/Assets/DCL/Landscape/Assets/Materials/Ocean.mat. The vertex
//! stage in coast_waves.wgsl computes the same sum; this CPU twin and `shader_defs` keep the two
//! on the same numbers.

use bevy::{
    asset::RenderAssetUsages,
    prelude::*,
    render::{
        mesh::{Indices, PrimitiveTopology},
        render_resource::ShaderDefVal,
    },
};

/// `_WaveCount`: the sum runs `WAVE_COUNT + 1` times, each with higher frequencies.
pub const WAVE_COUNT: u32 = 5;
/// `_WaveHeight` (m).
pub const WAVE_HEIGHT: f32 = 0.5;
/// `_WaveDistance`: larger is longer waves.
pub const WAVE_DISTANCE: f32 = 0.267;
/// `_WaveSpeed`.
pub const WAVE_SPEED: f32 = 4.37;
/// `_WaveSteepness`: zero keeps the waves to vertical motion.
pub const WAVE_STEEPNESS: f32 = 0.0;
/// `_WaveFadeDistance` (m): the waves fade out from the camera between these distances.
pub const WAVE_FADE: Vec2 = Vec2::new(0.0, 500.0);
/// `_Direction`, the animation direction; time runs along its negation.
pub const WAVE_DIRECTION: Vec2 = Vec2::new(0.5, 0.5);
/// The amplitudes, frequencies and two pairs of directions of the four waves Waves.hlsl sums
/// (`_WaveDirection` is one in Ocean.mat, leaving the directions as they are).
pub const WAVE_AMP: Vec4 = Vec4::new(0.3, 0.35, 0.25, 0.25);
pub const WAVE_FREQ: Vec4 = Vec4::new(1.3, 1.35, 1.25, 1.25);
pub const WAVE_DIR1: Vec4 = Vec4::new(0.3, 0.85, 0.85, 0.25);
pub const WAVE_DIR2: Vec4 = Vec4::new(0.1, 0.9, -0.5, -0.5);
/// `_RefractionStrength` and `_RefractionChromaticAberration`.
pub const REFRACTION_STRENGTH: f32 = 0.06;
pub const REFRACTION_CHROMATIC: f32 = 0.1;
/// `_DepthHorizontal`, `_DepthVertical` and `_DepthExp`, the water density's height and view
/// depth scales and its exponential blend (`GetWaterDensity` in ForwardPass.hlsl).
pub const WATER_DEPTH_HORIZONTAL: f32 = 1.5;
pub const WATER_DEPTH_VERTICAL: f32 = 0.01;
pub const WATER_DEPTH_EXP: f32 = 1.0;
/// The displaced grid around the camera; the flat ocean plane covers the rest.
pub const WAVE_GRID_SIZE_M: f32 = 1024.0;
pub const WAVE_GRID_SPACING_M: f32 = 2.0;

/// Height (m) and xz offset of the Unity wave sum at `xz`, `time` seconds, before distance fade.
/// `xz` and the offset are unity x/z metres; the result is (x offset, height, z offset). Only the
/// tests call it: it is the twin they hold the shader's sum to.
#[cfg(test)]
pub fn wave_displacement(xz: Vec2, time: f32) -> Vec3 {
    // `TIME_VERTEX * _WaveSpeed`, with TIME_VERTEX = time * -_Direction (Common.hlsl)
    let time = -WAVE_DIRECTION * time * WAVE_SPEED;
    let speed = Vec4::new(1.2 * time.x, 1.375 * time.y, 1.1 * time.x, time.y);
    // `lerp(1, MAX_WAVE_COUNT, 1 / WAVE_COUNT)` divides two uints, so it is 1
    let steepness = 12.0 * WAVE_STEEPNESS;
    let mut freq = WAVE_FREQ * (1.0 - WAVE_DISTANCE) * 3.0;
    let mut offset = Vec3::ZERO;
    for i in 0..=WAVE_COUNT {
        freq *= 1.0 + i as f32 / WAVE_COUNT as f32;
        let phase =
            freq * Vec4::new(
                WAVE_DIR1.xy().dot(xz),
                WAVE_DIR1.zw().dot(xz),
                WAVE_DIR2.xy().dot(xz),
                WAVE_DIR2.zw().dot(xz),
            ) + speed;
        let cos = Vec4::from_array(phase.to_array().map(f32::cos));
        let sin = Vec4::from_array(phase.to_array().map(f32::sin));
        // GerstnerOffset4: AB = steepness * dirAB * amp.xxyy, CD likewise
        let ab = steepness * WAVE_DIR1 * Vec4::new(WAVE_AMP.x, WAVE_AMP.x, WAVE_AMP.y, WAVE_AMP.y);
        let cd = steepness * WAVE_DIR2 * Vec4::new(WAVE_AMP.z, WAVE_AMP.z, WAVE_AMP.w, WAVE_AMP.w);
        offset.x += cos.dot(Vec4::new(ab.x, ab.z, cd.x, cd.z));
        offset.z += cos.dot(Vec4::new(ab.y, ab.w, cd.y, cd.w));
        offset.y += sin.dot(WAVE_AMP);
    }
    offset.y *= WAVE_HEIGHT / WAVE_COUNT as f32;
    offset.x *= 0.01 * WAVE_HEIGHT;
    offset.z *= 0.01 * WAVE_HEIGHT;
    offset
}

/// A flat grid `WAVE_GRID_SIZE_M` square with vertices every `WAVE_GRID_SPACING_M`, centred on
/// the origin at y 0. Positions only: coast_waves.wgsl derives the normals.
pub fn wave_grid_mesh() -> Mesh {
    let cells = (WAVE_GRID_SIZE_M / WAVE_GRID_SPACING_M) as u32;
    let side = cells + 1;
    let half = WAVE_GRID_SIZE_M * 0.5;
    let positions: Vec<[f32; 3]> = (0..side * side)
        .map(|i| {
            let (x, z) = (i % side, i / side);
            [
                x as f32 * WAVE_GRID_SPACING_M - half,
                0.0,
                z as f32 * WAVE_GRID_SPACING_M - half,
            ]
        })
        .collect();
    let mut indices = Vec::with_capacity((cells * cells * 6) as usize);
    for z in 0..cells {
        for x in 0..cells {
            let a = z * side + x;
            let [b, c, d] = [a + 1, a + side, a + side + 1];
            // counter-clockwise seen from above
            indices.extend([a, c, b, b, c, d]);
        }
    }
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::RENDER_WORLD,
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_indices(Indices::U32(indices))
}

/// The numbers coast_waves.wgsl and coast_water.wgsl share with this file; floats as their bit
/// patterns, which the shaders read back with `bitcast<f32>`, and vectors one def per component.
pub(super) fn shader_defs() -> Vec<ShaderDefVal> {
    let float = |name: String, value: f32| ShaderDefVal::UInt(name, value.to_bits());
    let mut defs = vec![
        ShaderDefVal::UInt("WAVE_COUNT".into(), WAVE_COUNT),
        float("WAVE_HEIGHT".into(), WAVE_HEIGHT),
        float("WAVE_DISTANCE".into(), WAVE_DISTANCE),
        float("WAVE_SPEED".into(), WAVE_SPEED),
        float("WAVE_STEEPNESS".into(), WAVE_STEEPNESS),
        float("REFRACTION_STRENGTH".into(), REFRACTION_STRENGTH),
        float("REFRACTION_CHROMATIC".into(), REFRACTION_CHROMATIC),
        float("WATER_DEPTH_HORIZONTAL".into(), WATER_DEPTH_HORIZONTAL),
        float("WATER_DEPTH_VERTICAL".into(), WATER_DEPTH_VERTICAL),
        float("WATER_DEPTH_EXP".into(), WATER_DEPTH_EXP),
    ];
    let vectors: [(&str, &[f32]); 5] = [
        ("WAVE_DIRECTION", &WAVE_DIRECTION.to_array()),
        ("WAVE_AMP", &WAVE_AMP.to_array()),
        ("WAVE_FREQ", &WAVE_FREQ.to_array()),
        ("WAVE_DIR1", &WAVE_DIR1.to_array()),
        ("WAVE_DIR2", &WAVE_DIR2.to_array()),
    ];
    for (name, components) in vectors {
        for (axis, value) in ["X", "Y", "Z", "W"].iter().zip(components) {
            defs.push(float(format!("{name}_{axis}"), *value));
        }
    }
    defs
}

#[cfg(test)]
pub(super) mod tests {
    use super::*;

    /// The bound Waves.hlsl implies on the height: each of the `WAVE_COUNT + 1` iterations adds
    /// at most sum(amp) before the sum is averaged over `WAVE_COUNT` and scaled by the height.
    pub(in crate::coast) fn wave_bound() -> f32 {
        WAVE_HEIGHT * (WAVE_COUNT + 1) as f32 * WAVE_AMP.element_sum() / WAVE_COUNT as f32
    }

    #[test]
    fn wave_sum_is_bounded_and_periodic_in_time() {
        let bound = wave_bound();
        let h = |t| wave_displacement(Vec2::new(10.0, 5.0), t).y;
        assert!((0..200).all(|i| h(i as f32 * 0.1).abs() <= bound * 1.001));
        assert!(h(0.0) != h(1.0));
    }

    #[test]
    fn steepness_zero_gives_no_horizontal_displacement() {
        let d = wave_displacement(Vec2::new(3.0, 7.0), 2.0);
        assert!(d.x.abs() < 1e-5 && d.z.abs() < 1e-5);
    }

    #[test]
    fn wave_grid_has_the_expected_vertices() {
        let m = wave_grid_mesh();
        let n = (WAVE_GRID_SIZE_M / WAVE_GRID_SPACING_M) as usize + 1;
        assert_eq!(m.count_vertices(), n * n);
    }
}
