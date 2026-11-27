//! Mip chains the engine builds on the CPU for its own textures: each level a 2x2 box filter of
//! the one above, down to one texel.

/// Filters the mip chain below `top`, a `width` by `height` level, handing each new level to
/// `level`, which may change it before the next is filtered from it. `average` filters four
/// texels into one; at an odd edge the last row or column is read twice. Returns the number of
/// levels, `top` included.
pub(crate) fn box_mips<T: Copy>(
    mut top: Vec<T>,
    (mut width, mut height): (usize, usize),
    average: impl Fn([T; 4]) -> T,
    mut level: impl FnMut(&mut Vec<T>),
) -> u32 {
    let mut levels = 1;
    while width > 1 || height > 1 {
        let (half_width, half_height) = ((width / 2).max(1), (height / 2).max(1));
        let mut next: Vec<T> = (0..half_width * half_height)
            .map(|i| {
                let (x, y) = (i % half_width * 2, i / half_width * 2);
                let (x1, y1) = ((x + 1).min(width - 1), (y + 1).min(height - 1));
                average([
                    top[y * width + x],
                    top[y * width + x1],
                    top[y1 * width + x],
                    top[y1 * width + x1],
                ])
            })
            .collect();
        level(&mut next);
        top = next;
        (width, height) = (half_width, half_height);
        levels += 1;
    }
    levels
}

/// Appends to `bytes`, which holds the first level of a `size` image of `N`-byte texels, its mip
/// chain, each channel averaged to the nearest value. Returns the number of levels.
///
/// With `coverage`, a channel and a cut, that channel of each level is scaled so that as large a
/// share of its texels reaches the cut as of the first level's: plain averaging thins
/// alpha-tested shapes away with distance.
pub(crate) fn byte_mips<const N: usize>(
    bytes: &mut Vec<u8>,
    size: (usize, usize),
    coverage: Option<(usize, u8)>,
) -> u32 {
    let top = bytes.as_chunks::<N>().0.to_vec();
    let target = coverage.map(|(channel, cut)| (channel, cut, share_reaching(&top, channel, cut)));
    box_mips(
        top,
        size,
        |texels| {
            std::array::from_fn(|channel| {
                let sum: u32 = texels.iter().map(|texel| u32::from(texel[channel])).sum();
                ((sum + 2) / 4) as u8
            })
        },
        |level| {
            if let Some((channel, cut, share)) = target {
                keep_share(level, channel, cut, share);
            }
            bytes.extend(level.iter().flatten());
        },
    )
}

/// The share of `texels` whose `channel` reaches `cut`.
pub(crate) fn share_reaching<const N: usize>(texels: &[[u8; N]], channel: usize, cut: u8) -> f32 {
    scaled_share(texels, channel, cut, 1.0)
}

fn scaled_share<const N: usize>(texels: &[[u8; N]], channel: usize, cut: u8, scale: f32) -> f32 {
    let reaching = texels
        .iter()
        .filter(|texel| f32::from(texel[channel]) * scale >= f32::from(cut))
        .count();
    reaching as f32 / texels.len() as f32
}

/// Scales `channel` of `texels` so that `share` of them reach `cut`, found by bisection.
fn keep_share<const N: usize>(texels: &mut [[u8; N]], channel: usize, cut: u8, share: f32) {
    let (mut low, mut high) = (0.25, 4.0);
    for _ in 0..10 {
        let scale = (low + high) * 0.5;
        if scaled_share(texels, channel, cut, scale) < share {
            low = scale;
        } else {
            high = scale;
        }
    }
    for texel in texels {
        texel[channel] = (f32::from(texel[channel]) * high).min(255.0) as u8;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every level down to one texel, each the rounded mean of the four above it.
    #[test]
    fn byte_mips_average_to_one_texel() {
        let mut bytes = vec![0, 10, 1, 11, 2, 12, 3, 13];
        let levels = byte_mips::<2>(&mut bytes, (2, 2), None);
        assert_eq!(levels, 2);
        assert_eq!(bytes[8..], [2, 12]);
    }

    /// Averaging pulls noise toward its mean, so fewer texels would reach a high cut; the kept
    /// channel reaches it as often on the next level as on the first.
    #[test]
    fn coverage_is_kept() {
        const SIZE: usize = 64;
        const CUT: u8 = 200;
        let mut bytes: Vec<u8> = (0..(SIZE * SIZE) as u32)
            .map(|i| (i.wrapping_mul(2_654_435_761) >> 24) as u8)
            .collect();
        let first = share_reaching(bytes.as_chunks::<1>().0, 0, CUT);
        let levels = byte_mips::<1>(&mut bytes, (SIZE, SIZE), Some((0, CUT)));
        assert_eq!(levels, 7);
        let level = &bytes[SIZE * SIZE..SIZE * SIZE + SIZE * SIZE / 4];
        let kept = share_reaching(level.as_chunks::<1>().0, 0, CUT);
        assert!((kept - first).abs() < 0.02, "{kept} against {first}");
    }

    /// An odd edge reads its last texel twice, and a strip halves along its long side alone.
    #[test]
    fn odd_and_strip_sizes() {
        let mut sizes = Vec::new();
        let levels = box_mips(
            vec![1.0f32; 6],
            (3, 2),
            |t| t.iter().sum::<f32>() / 4.0,
            |level| {
                sizes.push(level.len());
                assert!(level.iter().all(|&v| v == 1.0));
            },
        );
        assert_eq!((levels, sizes), (2, vec![1]));
    }
}
