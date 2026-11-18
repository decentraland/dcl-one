//! Periodic ripple slopes and foam noise for the sea, generated once and filtered by the GPU.

use bevy::{
    asset::RenderAssetUsages,
    image::{ImageAddressMode, ImageFilterMode, ImageSampler, ImageSamplerDescriptor},
    prelude::*,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat},
};

use super::random;

pub(super) const SIZE: usize = 256;
/// Foam noise cells across the texture.
const FOAM_CELLS: i32 = 8;

/// Tiling value noise with `cells` cells across the unit square.
pub(super) fn noise(p: Vec2, cells: i32, seed: u32) -> f32 {
    let p = p * cells as f32;
    let cell = p.floor().as_ivec2();
    // warped coordinates can cross zero, where `fract` keeps the sign
    let f = p - p.floor();
    let f = f * f * f * (f * (f * 6.0 - Vec2::splat(15.0)) + Vec2::splat(10.0));
    let value = |x: i32, y: i32| {
        random(seed ^ (x.rem_euclid(cells) as u32 * 733) ^ (y.rem_euclid(cells) as u32 * 179))
    };
    let a = value(cell.x, cell.y).lerp(value(cell.x + 1, cell.y), f.x);
    let b = value(cell.x, cell.y + 1).lerp(value(cell.x + 1, cell.y + 1), f.x);
    a.lerp(b, f.y)
}

pub(super) fn height_field(p: Vec2) -> f32 {
    // gentle domain warp, baked once, so the ripples don't sit on the octave grids
    let warp = Vec2::new(noise(p, 4, 1709), noise(p, 5, 2371)) - Vec2::splat(0.5);
    let p = p + warp * 0.11;
    noise(p, 7, 37) + noise(p, 13, 113) * 0.45 + noise(p, 29, 691) * 0.2 + noise(p, 61, 947) * 0.08
}

/// Tiling foam noise in [0, 1]: value noise `FOAM_CELLS` cells across with a finer octave,
/// stretched to the full range so the foam threshold reaches its peaks.
pub(super) fn foam_field() -> Vec<f32> {
    let foam: Vec<f32> = (0..SIZE * SIZE)
        .map(|i| {
            let p = Vec2::new((i % SIZE) as f32, (i / SIZE) as f32) / SIZE as f32;
            noise(p, FOAM_CELLS, 4441) + noise(p, FOAM_CELLS * 2, 8761) * 0.35
        })
        .collect();
    let (low, high) = foam.iter().fold((f32::MAX, f32::MIN), |(low, high), v| {
        (low.min(*v), high.max(*v))
    });
    foam.into_iter()
        .map(|v| (v - low) / (high - low).max(f32::EPSILON))
        .collect()
}

/// RGBA8: the slopes of `height_field` in rg and `foam_field` in b, with a full mip chain.
pub(super) fn image() -> Image {
    let heights: Vec<f32> = (0..SIZE * SIZE)
        .map(|i| height_field(Vec2::new((i % SIZE) as f32, (i / SIZE) as f32) / SIZE as f32))
        .collect();
    let foam = foam_field();
    let height = |x: usize, y: usize| heights[(y % SIZE) * SIZE + x % SIZE];
    let unorm = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    let mut level: Vec<[u8; 4]> = (0..SIZE * SIZE)
        .map(|i| {
            let (x, y) = (i % SIZE, i / SIZE);
            let slope = (Vec2::new(
                height(x + 1, y) - height(x + SIZE - 1, y),
                height(x, y + 1) - height(x, y + SIZE - 1),
            ) * 4.0)
                .clamp(Vec2::NEG_ONE, Vec2::ONE)
                * 0.5
                + 0.5;
            [unorm(slope.x), unorm(slope.y), unorm(foam[i]), 255]
        })
        .collect();
    let mut bytes = Vec::new();
    let mut size = SIZE;
    loop {
        bytes.extend(level.iter().flatten().copied());
        if size == 1 {
            break;
        }
        let half = size / 2;
        level = (0..half * half)
            .map(|i| {
                let origin = i / half * 2 * size + i % half * 2;
                std::array::from_fn(|channel| {
                    ((([origin, origin + 1, origin + size, origin + size + 1]
                        .into_iter()
                        .map(|p| u32::from(level[p][channel]))
                        .sum::<u32>())
                        + 2)
                        / 4) as u8
                })
            })
            .collect();
        size = half;
    }
    let mut image = Image::new_fill(
        Extent3d {
            width: SIZE as u32,
            height: SIZE as u32,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        &[128, 128, 0, 255],
        TextureFormat::Rgba8Unorm,
        RenderAssetUsages::RENDER_WORLD,
    );
    // `Image::new` only takes the first level
    image.texture_descriptor.mip_level_count = SIZE.ilog2() + 1;
    image.data = Some(bytes);
    image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
        address_mode_u: ImageAddressMode::Repeat,
        address_mode_v: ImageAddressMode::Repeat,
        mag_filter: ImageFilterMode::Linear,
        min_filter: ImageFilterMode::Linear,
        mipmap_filter: ImageFilterMode::Linear,
        anisotropy_clamp: 8,
        ..default()
    });
    image
}
