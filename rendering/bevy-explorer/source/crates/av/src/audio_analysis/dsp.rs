//! The analysis maths of unity-explorer's `native/audio-analysis` (`src/cabi.rs`,
//! `audio_analysis_analyze_audio_buffer`), line for line: the RMS amplitude of a window and the
//! mean magnitude of its spectrum in eight fixed bands, raw or on a clamped log scale. The
//! window is what Unity hands its library: the interleaved stereo DSP buffer at the nominal
//! sample rate, channel count discarded (`AudioAnalysisSystem.cs`, `AudioManager.asset`).

use dcl_component::proto_components::sdk::components::PbAudioAnalysisMode;

use super::fft;

pub const BANDS: usize = 8;
/// fundsp's `real_fft` takes a power-of-two length between 2 and 32768.
pub const FFT_MAX: usize = 32768;
/// The upper edge of bands 0 to 6; band 7 ends at Nyquist.
pub const BAND_EDGES_HZ: [f32; BANDS - 1] = [60.0, 120.0, 250.0, 500.0, 1000.0, 2000.0, 4000.0];
/// `AudioAnalysisSystem.DEFAULT_AMPLITUDE_GAIN`, applied in logarithmic mode when the scene sets
/// no gain.
pub const DEFAULT_AMPLITUDE_GAIN: f32 = 5.0;
/// `AudioAnalysisSystem.DEFAULT_BANDS_GAIN`.
pub const DEFAULT_BANDS_GAIN: f32 = 0.05;
/// Unity's DSP buffer (`ProjectSettings/AudioManager.asset`, `m_DSPBufferSize: 1024`): the
/// frames one analysis covers.
pub const WINDOW_FRAMES: usize = 1024;
/// The window is interleaved stereo, as Unity's `OnAudioFilterRead` buffer is.
pub const CHANNELS: usize = 2;
/// Samples in one window.
pub const WINDOW_SAMPLES: usize = WINDOW_FRAMES * CHANNELS;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Analysis {
    pub amplitude: f32,
    pub bands: [f32; BANDS],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Raw,
    Logarithmic,
}

impl From<PbAudioAnalysisMode> for Mode {
    fn from(mode: PbAudioAnalysisMode) -> Self {
        match mode {
            PbAudioAnalysisMode::ModeRaw => Mode::Raw,
            PbAudioAnalysisMode::ModeLogarithmic => Mode::Logarithmic,
        }
    }
}

/// The buffers an analysis works in, kept between calls.
#[derive(Default)]
pub struct Scratch {
    spectrum: Vec<f32>,
    fft: fft::Scratch,
}

/// Analyses the largest power-of-two prefix of `samples` (at most [`FFT_MAX`]); zeros for fewer
/// than two samples or a non-positive rate. In logarithmic mode every value is
/// `log10(x * gain + 1)` clamped to `[0, 1]`; raw values are unbounded.
pub fn analyze(
    samples: &[f32],
    sample_rate: f32,
    mode: Mode,
    amplitude_gain: f32,
    bands_gain: f32,
    scratch: &mut Scratch,
) -> Analysis {
    if samples.len() < 2 || sample_rate <= 0.0 {
        return Analysis::default();
    }

    let n = largest_power_of_two(samples.len().min(FFT_MAX)).max(2);
    let samples = &samples[..n];

    let amplitude_raw = rms(samples);
    fft::real_fft_magnitudes(samples, &mut scratch.spectrum, &mut scratch.fft);
    let bands_raw = compute_bands(&scratch.spectrum, sample_rate);

    match mode {
        Mode::Raw => Analysis {
            amplitude: amplitude_raw,
            bands: bands_raw,
        },
        Mode::Logarithmic => Analysis {
            amplitude: normalize_log(amplitude_raw, amplitude_gain),
            bands: bands_raw.map(|band| normalize_log(band, bands_gain)),
        },
    }
}

fn largest_power_of_two(n: usize) -> usize {
    if n == 0 {
        return 0;
    }
    1 << (usize::BITS - 1 - n.leading_zeros())
}

fn rms(samples: &[f32]) -> f32 {
    let sum: f32 = samples.iter().map(|x| x * x).sum();
    (sum / samples.len() as f32).sqrt()
}

fn compute_bands(spectrum: &[f32], sample_rate: f32) -> [f32; BANDS] {
    let mut bands = [0.0f32; BANDS];
    let nyquist = sample_rate * 0.5;
    let mut edges = [0.0f32; BANDS];
    edges[..BANDS - 1].copy_from_slice(&BAND_EDGES_HZ);
    edges[BANDS - 1] = nyquist;

    for (i, edge) in edges.iter().enumerate() {
        let start_freq = if i == 0 { 0.0 } else { edges[i - 1] };
        let start_bin = freq_to_bin(start_freq, sample_rate, spectrum.len());
        let end_bin = freq_to_bin(*edge, sample_rate, spectrum.len());

        let mut sum = 0.0;
        let mut count = 0usize;
        for bin in start_bin..end_bin {
            if bin < spectrum.len() {
                sum += spectrum[bin];
                count += 1;
            }
        }
        bands[i] = if count > 0 { sum / count as f32 } else { 0.0 };
    }

    bands
}

fn freq_to_bin(freq: f32, sample_rate: f32, bins: usize) -> usize {
    let nyquist = sample_rate * 0.5;
    if nyquist <= 0.0 || bins < 2 {
        return 0;
    }
    // bins cover 0..=Nyquist
    let bin_f = freq / nyquist * (bins as f32 - 1.0);
    bin_f.round().clamp(0.0, bins as f32 - 1.0) as usize
}

#[inline]
fn normalize_log(x: f32, gain: f32) -> f32 {
    (x * gain + 1.0).log10().clamp(0.0, 1.0)
}

/// `[end - WINDOW_FRAMES, end)` of a clip's frames, interleaved left and right, padded with
/// silence at the front while fewer frames have played.
pub fn window_at(frames: &[kira::Frame], end: usize, out: &mut Vec<f32>) {
    let end = end.min(frames.len());
    let start = end.saturating_sub(WINDOW_FRAMES);
    out.clear();
    out.resize((WINDOW_FRAMES - (end - start)) * CHANNELS, 0.0);
    for frame in &frames[start..end] {
        out.push(frame.left);
        out.push(frame.right);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RATE: f32 = 48_000.0;

    fn sine(hz: f32, len: usize, amplitude: f32) -> Vec<f32> {
        (0..len)
            .map(|t| amplitude * (2.0 * std::f32::consts::PI * hz * t as f32 / RATE).sin())
            .collect()
    }

    fn run(samples: &[f32], mode: Mode) -> Analysis {
        analyze(
            samples,
            RATE,
            mode,
            DEFAULT_AMPLITUDE_GAIN,
            DEFAULT_BANDS_GAIN,
            &mut Scratch::default(),
        )
    }

    fn loudest(bands: &[f32; BANDS]) -> usize {
        bands
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.total_cmp(b.1))
            .unwrap()
            .0
    }

    #[test]
    fn too_little_input_or_no_rate_gives_zeros() {
        let mut scratch = Scratch::default();
        assert_eq!(
            analyze(&[1.0], RATE, Mode::Raw, 1.0, 1.0, &mut scratch),
            Analysis::default()
        );
        assert_eq!(
            analyze(&[1.0, -1.0], 0.0, Mode::Raw, 1.0, 1.0, &mut scratch),
            Analysis::default()
        );
    }

    #[test]
    fn the_window_is_the_largest_power_of_two_prefix_capped_at_the_fft_limit() {
        assert_eq!(largest_power_of_two(3000), 2048);
        assert_eq!(largest_power_of_two(40_000.min(FFT_MAX)), 32768);
        assert_eq!(largest_power_of_two(1), 1);
        assert_eq!(largest_power_of_two(0), 0);
        // 2048 ones followed by 952 zeros: only the first 2048 count
        let mut samples = vec![1.0; 2048];
        samples.extend(vec![0.0; 952]);
        assert!((run(&samples, Mode::Raw).amplitude - 1.0).abs() < 1e-6);
    }

    #[test]
    fn amplitude_is_the_rms() {
        let square: Vec<f32> = (0..2048)
            .map(|t| if t % 2 == 0 { 1.0 } else { -1.0 })
            .collect();
        assert!((run(&square, Mode::Raw).amplitude - 1.0).abs() < 1e-6);
        let tone = sine(1000.0, 2048, 1.0);
        assert!((run(&tone, Mode::Raw).amplitude - std::f32::consts::FRAC_1_SQRT_2).abs() < 1e-3);
    }

    #[test]
    fn a_mono_tone_lights_the_band_that_holds_it() {
        // 350 Hz sits in band 4 (250 to 500 Hz)
        let result = run(&sine(350.0, 2048, 0.5), Mode::Raw);
        assert_eq!(loudest(&result.bands), 3);
        let result = run(&sine(3000.0, 2048, 0.5), Mode::Raw);
        assert_eq!(loudest(&result.bands), 6);
    }

    #[test]
    fn unity_feeds_interleaved_stereo_so_a_stereo_tone_reads_an_octave_down() {
        // the same 440 Hz sample in both channels, interleaved: the 48 kHz rate is nominal, the
        // sequence alternates channels, so the tone lands in band 2 (120 to 250 Hz) with an image
        // near Nyquist; part of the contract, see the ADR
        let mono = sine(440.0, WINDOW_FRAMES, 0.5);
        let stereo: Vec<f32> = mono.iter().flat_map(|s| [*s, *s]).collect();
        let result = run(&stereo, Mode::Raw);
        assert_eq!(loudest(&result.bands), 2);
        assert!(result.bands[7] > 0.0);
    }

    #[test]
    fn logarithmic_mode_clamps_to_the_unit_range() {
        assert_eq!(normalize_log(0.0, 5.0), 0.0);
        assert_eq!(normalize_log(1e9, 5.0), 1.0);
        let result = run(&sine(1000.0, 2048, 1.0), Mode::Logarithmic);
        assert!(result.amplitude > 0.0 && result.amplitude <= 1.0);
        assert!(result.bands.iter().all(|b| (0.0..=1.0).contains(b)));
        let raw = run(&sine(1000.0, 2048, 1.0), Mode::Raw);
        assert!(
            raw.bands[4] > 1.0,
            "raw magnitudes are unbounded: {}",
            raw.bands[4]
        );
    }

    #[test]
    fn frequencies_map_to_bins_over_the_nyquist_range() {
        assert_eq!(freq_to_bin(0.0, RATE, 1025), 0);
        assert_eq!(freq_to_bin(24_000.0, RATE, 1025), 1024);
        assert_eq!(freq_to_bin(12_000.0, RATE, 1025), 512);
        assert_eq!(freq_to_bin(100.0, 0.0, 1025), 0);
        assert_eq!(freq_to_bin(100.0, RATE, 1), 0);
    }

    #[test]
    fn a_clip_window_ends_at_the_position_and_pads_the_start() {
        let frames: Vec<kira::Frame> = (0..2000)
            .map(|i| kira::Frame::new(i as f32, -(i as f32)))
            .collect();
        let mut out = Vec::new();
        window_at(&frames, 1500, &mut out);
        assert_eq!(out.len(), WINDOW_SAMPLES);
        assert_eq!(out[0], 476.0);
        assert_eq!(out[1], -476.0);
        assert_eq!(out[WINDOW_SAMPLES - 2], 1499.0);
        window_at(&frames, 10, &mut out);
        assert_eq!(out.len(), WINDOW_SAMPLES);
        assert!(out[..(WINDOW_FRAMES - 10) * CHANNELS]
            .iter()
            .all(|s| *s == 0.0));
        assert_eq!(out[WINDOW_SAMPLES - 2], 9.0);
        window_at(&frames, 5000, &mut out);
        assert_eq!(out[WINDOW_SAMPLES - 2], 1999.0);
    }
}
