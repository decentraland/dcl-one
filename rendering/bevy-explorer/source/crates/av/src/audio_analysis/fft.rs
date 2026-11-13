//! The real FFT the analysis needs: an iterative radix-2 transform of a power-of-two window,
//! returning the magnitudes of the `n / 2 + 1` non-negative frequency bins, unnormalised, as
//! fundsp's `real_fft` returns them to unity-explorer's `native/audio-analysis`.

/// Buffers the transform works in; reused across calls so the steady path allocates nothing.
#[derive(Default)]
pub struct Scratch {
    re: Vec<f32>,
    im: Vec<f32>,
}

/// Writes the magnitude of bins `0..=n/2` of `samples` (a power-of-two length) into `out`.
pub fn real_fft_magnitudes(samples: &[f32], out: &mut Vec<f32>, scratch: &mut Scratch) {
    let n = samples.len();
    debug_assert!(n.is_power_of_two(), "the window is a power of two, got {n}");
    let Scratch { re, im } = scratch;
    re.clear();
    re.extend_from_slice(samples);
    im.clear();
    im.resize(n, 0.0);

    let bits = n.trailing_zeros();
    for i in 0..n {
        let j = if bits == 0 {
            0
        } else {
            i.reverse_bits() >> (usize::BITS - bits)
        };
        if j > i {
            re.swap(i, j);
        }
    }

    let mut len = 2;
    while len <= n {
        let half = len / 2;
        let angle = -2.0 * std::f64::consts::PI / len as f64;
        let (step_im, step_re) = angle.sin_cos();
        for start in (0..n).step_by(len) {
            let (mut twiddle_re, mut twiddle_im) = (1.0f64, 0.0f64);
            for k in 0..half {
                let a = start + k;
                let b = a + half;
                let (br, bi) = (re[b] as f64, im[b] as f64);
                let tr = br * twiddle_re - bi * twiddle_im;
                let ti = br * twiddle_im + bi * twiddle_re;
                let (ar, ai) = (re[a] as f64, im[a] as f64);
                re[b] = (ar - tr) as f32;
                im[b] = (ai - ti) as f32;
                re[a] = (ar + tr) as f32;
                im[a] = (ai + ti) as f32;
                let next_re = twiddle_re * step_re - twiddle_im * step_im;
                twiddle_im = twiddle_re * step_im + twiddle_im * step_re;
                twiddle_re = next_re;
            }
        }
        len *= 2;
    }

    out.clear();
    out.extend((0..=n / 2).map(|i| (re[i] * re[i] + im[i] * im[i]).sqrt()));
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The definition, for the bins the transform returns.
    fn naive_magnitudes(samples: &[f32]) -> Vec<f32> {
        let n = samples.len();
        (0..=n / 2)
            .map(|k| {
                let (mut re, mut im) = (0.0f64, 0.0f64);
                for (t, x) in samples.iter().enumerate() {
                    let angle = -2.0 * std::f64::consts::PI * (k * t) as f64 / n as f64;
                    re += *x as f64 * angle.cos();
                    im += *x as f64 * angle.sin();
                }
                (re * re + im * im).sqrt() as f32
            })
            .collect()
    }

    fn noise(n: usize, mut seed: u32) -> Vec<f32> {
        (0..n)
            .map(|_| {
                seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                (seed >> 8) as f32 / (1u32 << 24) as f32 * 2.0 - 1.0
            })
            .collect()
    }

    #[test]
    fn magnitudes_match_the_definition() {
        let mut scratch = Scratch::default();
        let mut out = Vec::new();
        for (n, seed) in [(2, 1), (8, 2), (1024, 3)] {
            let samples = noise(n, seed);
            real_fft_magnitudes(&samples, &mut out, &mut scratch);
            let expected = naive_magnitudes(&samples);
            assert_eq!(out.len(), n / 2 + 1);
            for (bin, (got, want)) in out.iter().zip(&expected).enumerate() {
                assert!(
                    (got - want).abs() <= 1e-3 * want.abs().max(1.0),
                    "n {n} bin {bin}: {got} vs {want}"
                );
            }
        }
    }

    #[test]
    fn a_pure_tone_is_one_bin_at_its_frequency() {
        let n = 1024;
        let samples: Vec<f32> = (0..n)
            .map(|t| (2.0 * std::f32::consts::PI * 16.0 * t as f32 / n as f32).sin())
            .collect();
        let mut out = Vec::new();
        real_fft_magnitudes(&samples, &mut out, &mut Scratch::default());
        let peak = out
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.total_cmp(b.1))
            .map(|(bin, _)| bin)
            .unwrap();
        assert_eq!(peak, 16);
        assert!(
            (out[16] - n as f32 / 2.0).abs() < 1e-2,
            "unnormalised: {}",
            out[16]
        );
    }
}
