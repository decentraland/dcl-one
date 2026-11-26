//! The mask the grass cards are cut out by: two strips of grass blades, drawn here. No reference
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

/// The blades of each strip: one mass across the card and up to its top, solid at the root and
/// breaking into spikes above, a little lower toward its sides, where the blades fan outward.
const STRIPS: [&[Blade]; 2] = [
    &[
        (0.096, 0.021, -0.076, 0.47),
        (0.130, 0.021, -0.086, 0.72),
        (0.154, 0.027, -0.103, 0.60),
        (0.181, 0.023, -0.066, 0.77),
        (0.214, 0.026, -0.105, 0.85),
        (0.240, 0.026, -0.085, 0.86),
        (0.267, 0.022, -0.038, 0.70),
        (0.296, 0.027, -0.074, 0.81),
        (0.322, 0.024, -0.030, 0.84),
        (0.350, 0.021, -0.060, 0.90),
        (0.376, 0.026, -0.063, 0.59),
        (0.407, 0.021, -0.023, 0.92),
        (0.431, 0.026, -0.057, 0.92),
        (0.456, 0.021, -0.032, 0.95),
        (0.488, 0.027, -0.032, 0.70),
        (0.510, 0.021, -0.025, 0.71),
        (0.544, 0.022, 0.042, 0.93),
        (0.568, 0.024, -0.018, 0.93),
        (0.594, 0.025, -0.009, 0.92),
        (0.627, 0.021, 0.021, 0.71),
        (0.653, 0.024, -0.008, 0.91),
        (0.680, 0.026, 0.031, 0.70),
        (0.709, 0.023, 0.034, 0.85),
        (0.737, 0.020, 0.091, 0.84),
        (0.760, 0.021, 0.013, 0.81),
        (0.786, 0.023, 0.054, 0.85),
        (0.819, 0.022, 0.034, 0.73),
        (0.845, 0.021, 0.078, 0.81),
        (0.875, 0.022, 0.075, 0.67),
        (0.903, 0.022, 0.073, 0.76),
    ],
    &[
        (0.099, 0.023, -0.079, 0.60),
        (0.127, 0.020, -0.078, 0.79),
        (0.159, 0.026, -0.034, 0.63),
        (0.179, 0.023, -0.094, 0.80),
        (0.213, 0.026, -0.060, 0.68),
        (0.236, 0.025, -0.036, 0.77),
        (0.268, 0.024, -0.046, 0.70),
        (0.291, 0.024, -0.025, 0.71),
        (0.323, 0.024, 0.005, 0.87),
        (0.349, 0.023, -0.054, 0.68),
        (0.378, 0.024, -0.048, 0.65),
        (0.405, 0.024, -0.038, 0.92),
        (0.427, 0.022, -0.045, 0.95),
        (0.462, 0.021, -0.029, 0.82),
        (0.482, 0.021, -0.003, 0.70),
        (0.514, 0.025, -0.035, 0.67),
        (0.544, 0.026, -0.030, 0.84),
        (0.566, 0.021, 0.018, 0.88),
        (0.597, 0.025, 0.026, 0.86),
        (0.628, 0.023, -0.004, 0.93),
        (0.655, 0.021, 0.040, 0.80),
        (0.681, 0.020, 0.017, 0.63),
        (0.709, 0.021, 0.026, 0.86),
        (0.735, 0.025, 0.010, 0.82),
        (0.760, 0.023, 0.066, 0.87),
        (0.790, 0.026, 0.085, 0.83),
        (0.814, 0.027, 0.056, 0.84),
        (0.842, 0.027, 0.108, 0.70),
        (0.876, 0.024, 0.104, 0.73),
        (0.904, 0.023, 0.076, 0.60),
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

    /// Each strip is one mass at the root, most of the card wide, breaking into many spikes
    /// above, and about half of it is blade.
    #[test]
    fn strips_are_a_wide_mass_of_spikes() {
        let runs = |row: usize| {
            let blade: Vec<bool> = (0..SIZE).map(|x| is_blade(x, row)).collect();
            blade.windows(2).filter(|pair| !pair[0] && pair[1]).count()
        };
        for (root_row, towards_tips) in [(0, 1), (SIZE - 1, -1)] {
            assert_eq!(runs(root_row), 1);
            let width = (0..SIZE).filter(|x| is_blade(*x, root_row)).count();
            assert!(width > SIZE * 4 / 5, "{width} texels wide");
            // 0.7 of the way up the strip
            let high = root_row as i32 + towards_tips * (SIZE as i32 * 7 / 20);
            assert!(runs(high as usize) >= 5, "{} spikes", runs(high as usize));
        }
        let level: Vec<u8> = (0..SIZE * SIZE)
            .map(|i| texel(i % SIZE, i / SIZE))
            .collect();
        let coverage = kept(&level, 1.0);
        assert!((0.4..0.6).contains(&coverage), "coverage {coverage}");
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
