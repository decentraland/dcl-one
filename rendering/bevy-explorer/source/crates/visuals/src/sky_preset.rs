//! Sky preset values from unity-explorer: `Assets/DCL/SkyBox/Presets/StylizedV1.asset` and
//! `Assets/DCL/SkyBox/SkyboxSettings.asset`. Unity's curve tangents are not carried: keys are
//! interpolated linearly.

use bevy::prelude::*;

/// Fog density at every phase (`fogDensityByPhase` default), `FogFalloff::Exponential`.
pub const FOG_DENSITY_PER_METRE: f32 = 0.0005;

/// `(day fraction, phase)`: phase 0 night, 0.25 sunrise, 0.5 day, 0.75 sunset, 1 night.
pub const TIME_TO_PHASE: [(f32, f32); 10] = [
    (0.0, 0.0),
    (0.17, 0.0),
    (0.225, 0.25),
    (0.29, 0.25),
    (0.4, 0.5),
    (0.62, 0.5),
    (0.745, 0.75),
    (0.8, 0.75),
    (0.855, 1.0),
    (1.0, 1.0),
];

/// sRGB, by phase.
pub const FOG_COLOR_RAMP: [(f32, Vec3); 5] = [
    (0.0, Vec3::new(0.18701142, 0.106132075, 0.4245283)),
    (0.25, Vec3::new(0.98, 0.9, 0.75)),
    (0.5, Vec3::new(0.62, 0.78, 0.98)),
    (0.75, Vec3::new(1.0, 0.72, 0.5)),
    (1.0, Vec3::new(0.18701142, 0.106132075, 0.4245283)),
];

/// sRGB, by phase.
pub const DIRECTIONAL_COLOR_RAMP: [(f32, Vec3); 5] = [
    (0.0, Vec3::new(0.47713804, 0.46542, 0.75)),
    (0.25, Vec3::new(1.0, 0.9, 0.7)),
    (0.5, Vec3::new(1.0, 0.98, 0.94)),
    (0.75, Vec3::new(1.0, 0.72, 0.5)),
    (1.0, Vec3::new(0.47713804, 0.46542, 0.75)),
];

/// By day fraction.
pub const LIGHT_INTENSITY_BY_TIME: [(f32, f32); 12] = [
    (0.0, 1.0),
    (0.14583, 0.5),
    (0.16667, 0.01),
    (0.1875, 0.01),
    (0.25, 0.6),
    (0.29167, 1.0),
    (0.5, 1.2),
    (0.75, 1.0),
    (0.85417, 0.01),
    (0.875, 0.01),
    (0.91667, 1.0),
    (1.0, 1.0),
];

/// By day fraction.
pub const LENS_FLARE_INTENSITY_BY_TIME: [(f32, f32); 10] = [
    (0.0, 0.6),
    (0.0723, 0.6),
    (0.1410, 0.0018),
    (0.2738, 0.0),
    (0.35, 1.0),
    (0.7, 1.0),
    (0.7584, 0.0),
    (0.8655, 0.0),
    (0.9319, 0.6),
    (1.0, 0.6),
];

/// Linear between keys, clamped outside the range. `keys` must be sorted ascending.
fn sample<T: Copy>(keys: &[(f32, T)], t: f32, lerp: impl Fn(T, T, f32) -> T) -> T {
    let (first, last) = (keys[0], keys[keys.len() - 1]);
    if t <= first.0 {
        return first.1;
    }
    if t >= last.0 {
        return last.1;
    }
    let (a, b) = keys
        .windows(2)
        .map(|w| (w[0], w[1]))
        .find(|(_, b)| t <= b.0)
        .unwrap_or((last, last));
    lerp(a.1, b.1, (t - a.0) / (b.0 - a.0))
}

pub fn sample_keys(keys: &[(f32, f32)], t: f32) -> f32 {
    sample(keys, t, |a, b, f| a * (1.0 - f) + b * f)
}

pub fn sample_ramp(keys: &[(f32, Vec3)], phase: f32) -> Vec3 {
    sample(keys, phase, |a, b, f| a * (1.0 - f) + b * f)
}

/// `day_fraction` in [0, 1).
pub fn time_to_phase(day_fraction: f32) -> f32 {
    sample_keys(&TIME_TO_PHASE, day_fraction)
}

pub fn fog_color(day_fraction: f32) -> Color {
    let rgb = sample_ramp(&FOG_COLOR_RAMP, time_to_phase(day_fraction));
    Color::srgb(rgb.x, rgb.y, rgb.z)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::day_night::day_fraction;

    #[test]
    fn phase_anchors_match_unity() {
        assert_eq!(time_to_phase(0.0), 0.0);
        assert_eq!(time_to_phase(0.225), 0.25);
        assert_eq!(time_to_phase(0.5), 0.5);
        assert_eq!(time_to_phase(0.7725), 0.75);
        assert!((time_to_phase(0.1975) - 0.125).abs() < 1e-6); // linear between (0.17, 0) and (0.225, 0.25)
        assert_eq!(time_to_phase(0.9), 1.0);
    }

    #[test]
    fn fog_colour_hits_the_ramp_keys() {
        assert_eq!(
            sample_ramp(&FOG_COLOR_RAMP, 0.5),
            Vec3::new(0.62, 0.78, 0.98)
        );
        assert_eq!(
            sample_ramp(&FOG_COLOR_RAMP, 1.0),
            sample_ramp(&FOG_COLOR_RAMP, 0.0)
        );
        let night = fog_color(0.0).to_linear();
        assert!(night.red < 0.04 && night.blue > night.red); // sRGB (0.187, 0.106, 0.425) linearised
    }

    #[test]
    fn day_fraction_wraps() {
        assert_eq!(day_fraction(6.0), 0.25);
        assert_eq!(day_fraction(24.0), 0.0);
        assert_eq!(day_fraction(30.0), 0.25);
    }
}
