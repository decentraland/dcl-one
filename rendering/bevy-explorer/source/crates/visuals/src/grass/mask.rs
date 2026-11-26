//! The mask the grass cards are cut out by: two strips of blade clumps, drawn here. No reference
//! artwork is sampled. A strip spans the mask's width and half its height, its roots along the
//! mask's top or bottom edge and its tips toward the middle, so the edges a card's sample can
//! reach past repeat what the card already shows.

use bevy::{
    asset::{weak_handle, RenderAssetUsages},
    image::{ImageFilterMode, ImageSampler, ImageSamplerDescriptor},
    prelude::*,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat},
};

pub(crate) const BLADE_MASK: Handle<Image> = weak_handle!("fa3afe51-f8f1-4c36-b54d-736618ef8821");

/// Texels along each side of the mask.
const SIZE: usize = 512;
/// A texel at or above this is blade (`grass_wind.wgsl`'s `MASK_CUTOFF`).
const CUT: u8 = 128;
/// The mask rises from 0 to 1 over this many texels across a blade's edge, so the cut stays a
/// clean line when a card fills the screen.
const EDGE: f32 = 4.0;

/// One blade: its root along the card, its half width at the root and the lean of its tip, in
/// card widths, and its height as a share of the card's.
type Blade = (f32, f32, f32, f32);

/// The blades of each strip, in clumps with gaps between them.
const STRIPS: [&[Blade]; 2] = [
    &[
        (0.060, 0.020, -0.045, 0.50),
        (0.092, 0.024, -0.060, 0.78),
        (0.128, 0.026, -0.015, 0.95),
        (0.150, 0.020, 0.070, 0.60),
        (0.186, 0.024, 0.045, 0.84),
        (0.222, 0.020, 0.100, 0.52),
        (0.352, 0.020, -0.085, 0.58),
        (0.384, 0.024, -0.040, 0.86),
        (0.420, 0.026, 0.012, 0.96),
        (0.452, 0.022, 0.075, 0.70),
        (0.486, 0.024, 0.050, 0.90),
        (0.520, 0.018, 0.110, 0.48),
        (0.670, 0.020, -0.070, 0.66),
        (0.702, 0.024, -0.025, 0.92),
        (0.738, 0.022, 0.030, 0.62),
        (0.770, 0.026, 0.070, 0.82),
        (0.806, 0.020, 0.115, 0.56),
    ],
    &[
        (0.100, 0.020, -0.075, 0.54),
        (0.134, 0.026, -0.030, 0.90),
        (0.166, 0.022, 0.050, 0.68),
        (0.200, 0.024, 0.085, 0.80),
        (0.300, 0.018, -0.060, 0.46),
        (0.330, 0.022, -0.020, 0.72),
        (0.364, 0.020, 0.065, 0.58),
        (0.500, 0.020, -0.090, 0.62),
        (0.532, 0.024, -0.050, 0.88),
        (0.566, 0.026, 0.005, 0.96),
        (0.598, 0.022, 0.060, 0.76),
        (0.632, 0.024, 0.095, 0.92),
        (0.664, 0.018, 0.120, 0.50),
        (0.800, 0.020, -0.055, 0.60),
        (0.832, 0.024, 0.010, 0.84),
        (0.864, 0.020, 0.075, 0.66),
    ],
];

/// Where a card's vertex reads the mask: `along` its width and `height` up it, both 0 to 1.
pub(crate) fn uv(strip: usize, along: f32, height: f32) -> [f32; 2] {
    [
        along,
        if strip == 0 {
            height * 0.5
        } else {
            1.0 - height * 0.5
        },
    ]
}

/// How much of a strip's point is blade, 0 to 1: each blade curves toward its lean as it rises
/// and narrows to a point at its tip.
fn blade_cover(strip: usize, along: f32, height: f32) -> f32 {
    STRIPS[strip]
        .iter()
        .filter(|(.., top)| height < *top)
        .map(|(root, half_width, lean, top)| {
            let rise = height / top;
            let centre = root + lean * rise * rise;
            let half = half_width * (1.0 - rise).powf(0.85) * (1.0 + 0.5 * rise);
            ((half - (along - centre).abs()) * SIZE as f32 / EDGE + 0.5).clamp(0.0, 1.0)
        })
        .fold(0.0, f32::max)
}

fn texel(x: usize, y: usize) -> u8 {
    let [along, across] = [x, y].map(|at| (at as f32 + 0.5) / SIZE as f32);
    let cover = if across < 0.5 {
        blade_cover(0, along, across * 2.0)
    } else {
        blade_cover(1, along, (1.0 - across) * 2.0)
    };
    (cover * 255.0) as u8
}

/// The share of `level` that passes the cut once scaled by `scale`.
fn kept(level: &[u8], scale: f32) -> f32 {
    let passing = level
        .iter()
        .filter(|value| f32::from(**value) * scale >= f32::from(CUT))
        .count();
    passing as f32 / level.len() as f32
}

pub(crate) fn image() -> Image {
    let mut level: Vec<u8> = (0..SIZE * SIZE)
        .map(|i| texel(i % SIZE, i / SIZE))
        .collect();
    let coverage = kept(&level, 1.0);
    let mut bytes = Vec::new();
    let mut size = SIZE;
    loop {
        bytes.extend_from_slice(&level);
        if size == 1 {
            break;
        }
        let half = size / 2;
        let mut next: Vec<u8> = (0..half * half)
            .map(|i| {
                let origin = i / half * 2 * size + i % half * 2;
                let sum: u32 = [origin, origin + 1, origin + size, origin + size + 1]
                    .into_iter()
                    .map(|texel| u32::from(level[texel]))
                    .sum();
                (sum / 4) as u8
            })
            .collect();
        // plain averaging thins the blades away with distance: each level is scaled so as much
        // of it passes the cut as of the first
        let (mut low, mut high) = (0.25, 4.0);
        for _ in 0..10 {
            let scale = (low + high) * 0.5;
            if kept(&next, scale) < coverage {
                low = scale;
            } else {
                high = scale;
            }
        }
        for value in &mut next {
            *value = (f32::from(*value) * high).min(255.0) as u8;
        }
        level = next;
        size = half;
    }
    let mut image = Image::new_fill(
        Extent3d {
            width: SIZE as u32,
            height: SIZE as u32,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        &[0],
        TextureFormat::R8Unorm,
        RenderAssetUsages::RENDER_WORLD,
    );
    image.texture_descriptor.mip_level_count = SIZE.ilog2() + 1;
    image.data = Some(bytes);
    image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
        mag_filter: ImageFilterMode::Linear,
        min_filter: ImageFilterMode::Linear,
        mipmap_filter: ImageFilterMode::Linear,
        ..default()
    });
    image
}

#[cfg(test)]
mod tests {
    use super::*;

    fn is_blade(x: usize, y: usize) -> bool {
        texel(x, y) >= CUT
    }

    /// Each strip is clumps of blades with gaps down to the root, and about half of it is blade.
    #[test]
    fn strips_are_clumps_with_gaps() {
        for root_row in [0, SIZE - 1] {
            let roots: Vec<bool> = (0..SIZE).map(|x| is_blade(x, root_row)).collect();
            let clumps = roots.windows(2).filter(|pair| !pair[0] && pair[1]).count();
            assert!((3..=5).contains(&clumps), "{clumps} clumps");
        }
        let level: Vec<u8> = (0..SIZE * SIZE)
            .map(|i| texel(i % SIZE, i / SIZE))
            .collect();
        let coverage = kept(&level, 1.0);
        assert!((0.2..0.5).contains(&coverage), "coverage {coverage}");
    }

    /// Nothing reaches the mask's sides or the middle rows where the strips meet, so a card's
    /// edges never show a cut blade and one strip never shows in the other's card.
    #[test]
    fn blades_stay_inside_their_strip() {
        for at in 0..SIZE {
            for side in [0, SIZE - 1] {
                assert_eq!(texel(side, at), 0, "side {side}, row {at}");
            }
            for middle in [SIZE / 2 - 2, SIZE / 2 - 1, SIZE / 2, SIZE / 2 + 1] {
                assert_eq!(texel(at, middle), 0, "column {at}, row {middle}");
            }
        }
    }

    /// A card reads its own strip: roots at its foot, tips at its top.
    #[test]
    fn card_uvs_run_from_root_to_tip() {
        assert_eq!(uv(0, 0.25, 0.0), [0.25, 0.0]);
        assert_eq!(uv(0, 0.25, 1.0), [0.25, 0.5]);
        assert_eq!(uv(1, 0.25, 0.0), [0.25, 1.0]);
        assert_eq!(uv(1, 0.25, 1.0), [0.25, 0.5]);
    }

    /// The mips keep the first level's cover, so far tufts do not thin away.
    #[test]
    fn mips_keep_the_cover() {
        let image = image();
        assert_eq!(image.texture_descriptor.mip_level_count, 10);
        let bytes = image.data.unwrap();
        let first = kept(&bytes[..SIZE * SIZE], 1.0);
        let mut start = SIZE * SIZE;
        // down to 32 texels a side, below which one texel is too large a share
        for size in [256, 128, 64, 32] {
            let level = &bytes[start..start + size * size];
            let cover = kept(level, 1.0);
            assert!(
                (cover - first).abs() < 0.03,
                "{size}: {cover} against {first}"
            );
            start += size * size;
        }
    }
}
