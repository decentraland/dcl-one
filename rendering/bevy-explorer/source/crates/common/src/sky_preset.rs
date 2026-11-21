//! The sky preset: the fog, the directional light and the Trilight ambient by time of day. Keys
//! are interpolated linearly (see decisions.md).

use std::f32::consts::PI;

use bevy::prelude::*;

/// One light unit of the sky preset and of scene light intensities: the directional light that
/// lights a facing white surface to 1. bevy divides illuminance by pi and multiplies by the view
/// exposure, so the lux that land at 1 are `pi / exposure`; the explorer leaves every camera on
/// bevy's default exposure (`Exposure::BLENDER`, ev100 9.7: `1 / exposure = 1.2 * 2^9.7`).
pub const LIGHT_UNIT_LUX: f32 = PI * 1.2 * 831.7465;

/// Fog density at every phase, in the preset's units (see `fog_falloff_density`).
pub const FOG_DENSITY_PER_METRE: f32 = 0.0005;

/// The preset's fog is a base 2 exponential of density / sqrt(ln 2), so it falls off as
/// `exp(-sqrt(ln 2) * density * depth)`, not `exp(-density * depth)`.
pub const EXP2_FOG_SCALE: f32 = 0.832_554_6;

/// A density in the preset's units, which a scene's fog component shares, as the density of
/// bevy's `FogFalloff::Exponential`.
pub fn fog_falloff_density(preset_density: f32) -> f32 {
    preset_density * EXP2_FOG_SCALE
}

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

/// The Trilight ambient's sky colour, sRGB, by phase.
pub const AMBIENT_SKY_RAMP: [(f32, Vec3); 5] = [
    (0.0, Vec3::new(0.31163162, 0.1308295, 0.5660378)),
    (0.25, Vec3::new(1.0, 0.5967654, 0.5264151)),
    (0.5, Vec3::new(0.51868874, 0.6788208, 0.73800004)),
    (0.7, Vec3::new(1.0, 0.49981537, 0.4584905)),
    (1.0, Vec3::new(0.31163162, 0.1308295, 0.5660378)),
];

/// The Trilight ambient's equator colour, sRGB, by phase.
pub const AMBIENT_EQUATOR_RAMP: [(f32, Vec3); 5] = [
    (0.0, Vec3::new(0.6171662, 0.6205496, 0.7660377)),
    (0.3, Vec3::new(0.90188676, 0.52418566, 0.4747668)),
    (0.5, Vec3::new(0.7317719, 0.6489298, 0.78699994)),
    (0.7, Vec3::new(0.7372549, 0.5176471, 0.6171487)),
    (1.0, Vec3::new(0.6171662, 0.6205496, 0.7660377)),
];

/// The Trilight ambient's ground colour, sRGB, by phase.
pub const AMBIENT_GROUND_RAMP: [(f32, Vec3); 6] = [
    (0.0, Vec3::new(0.42499506, 0.18426357, 0.20304249)),
    (0.135, Vec3::new(0.5471698, 0.18325028, 0.18325028)),
    (0.3, Vec3::new(0.5660378, 0.10190902, 0.08276967)),
    (0.5, Vec3::new(0.5849056, 0.1017827, 0.09104664)),
    (0.7, Vec3::new(0.5283019, 0.09220363, 0.10541872)),
    (1.0, Vec3::new(0.56078434, 0.24313724, 0.26083013)),
];

/// The lens flare's intensity, by day fraction.
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

/// `hours` of the day as a fraction of it, wrapped into [0, 1).
pub fn day_fraction(hours: f32) -> f32 {
    (hours / 24.0).rem_euclid(1.0)
}

/// `day_fraction` in [0, 1).
pub fn time_to_phase(day_fraction: f32) -> f32 {
    sample_keys(&TIME_TO_PHASE, day_fraction)
}

pub fn fog_color(day_fraction: f32) -> Color {
    let rgb = sample_ramp(&FOG_COLOR_RAMP, time_to_phase(day_fraction));
    Color::srgb(rgb.x, rgb.y, rgb.z)
}

/// The Trilight ambient colours at `day_fraction`, linear: sky, equator, ground.
pub fn ambient_trilight(day_fraction: f32) -> [Vec3; 3] {
    let phase = time_to_phase(day_fraction);
    [
        &AMBIENT_SKY_RAMP[..],
        &AMBIENT_EQUATOR_RAMP[..],
        &AMBIENT_GROUND_RAMP[..],
    ]
    .map(|ramp| {
        let rgb = sample_ramp(ramp, phase);
        Color::srgb(rgb.x, rgb.y, rgb.z).to_linear().to_vec3()
    })
}

/// Sky colour over elevation (0 horizon, 1 zenith) at night, HDR linear values as authored.
pub const SKY_NIGHT: [(f32, Vec3); 4] = [
    (0.0, Vec3::new(0.030885903, 0.015796326, 0.17688681)),
    (0.11999695, Vec3::new(0.25916684, 0.0768957, 0.6037736)),
    (0.6500038, Vec3::new(0.20923078, 0.12326525, 0.3042453)),
    (1.0, Vec3::new(0.28286394, 0.20586, 0.39)),
];

/// Sky colour over elevation at sunrise.
pub const SKY_SUNRISE: [(f32, Vec3); 6] = [
    (0.0, Vec3::new(2.2, 1.7, 1.0)),
    (0.029999238, Vec3::new(2.5, 1.8, 0.9)),
    (0.08999771, Vec3::new(1.4, 0.95, 0.55)),
    (0.22000457, Vec3::new(0.82, 0.68, 0.5)),
    (0.4500038, Vec3::new(0.45, 0.5, 0.75)),
    (1.0, Vec3::new(0.15, 0.4, 0.85)),
];

/// Sky colour over elevation by day.
pub const SKY_DAY: [(f32, Vec3); 5] = [
    (0.0, Vec3::new(1.0, 1.0, 1.02)),
    (0.020004578, Vec3::new(1.08, 1.1, 1.15)),
    (0.17941558, Vec3::new(0.06724853, 0.78176415, 1.0717734)),
    (0.5264668, Vec3::new(0.0, 0.54509807, 1.0)),
    (1.0, Vec3::new(0.02, 0.42, 0.88)),
];

/// Sky colour over elevation at sunset.
pub const SKY_SUNSET: [(f32, Vec3); 7] = [
    (0.0, Vec3::new(2.0, 0.9, 0.5)),
    (0.029999238, Vec3::new(3.2, 1.1, 0.35)),
    (0.08999771, Vec3::new(1.3, 0.75, 0.35)),
    (0.17999542, Vec3::new(1.0, 0.55, 0.55)),
    (0.34999618, Vec3::new(0.8, 0.42, 0.72)),
    (0.5499962, Vec3::new(0.45, 0.3, 0.72)),
    (1.0, Vec3::new(0.22, 0.24, 0.72)),
];

/// The sun disc colour (HDR), by phase.
pub const SUN_COLOR_RAMP: [(f32, Vec3); 5] = [
    (0.0, Vec3::new(1.7477998, 1.4129999, 2.25)),
    (0.2500038, Vec3::new(6.0, 3.2, 1.6)),
    (0.5000076, Vec3::new(10.0, 10.0, 9.5)),
    (0.7499962, Vec3::new(4.8671436, 1.7698704, 0.6194546)),
    (1.0, Vec3::new(1.7477998, 1.4129999, 2.25)),
];

/// The moon disc colour (HDR), by moon progress.
pub const MOON_COLOR_RAMP: [(f32, Vec3); 4] = [
    (0.0, Vec3::new(2.6, 2.0, 1.4)),
    (0.10000763, Vec3::new(2.0, 2.2, 3.0)),
    (0.9000076, Vec3::new(2.0, 2.2, 3.0)),
    (1.0, Vec3::new(2.6, 2.0, 1.4)),
];

/// The lit cloud colour, by phase.
pub const CLOUD_LIT_RAMP: [(f32, Vec3); 5] = [
    (0.0, Vec3::new(0.2600742, 0.2021627, 0.7264151)),
    (0.2500038, Vec3::new(2.0628636, 0.9062017, 0.16541831)),
    (0.5000076, Vec3::new(1.5157166, 1.3498304, 0.908)),
    (0.7499962, Vec3::new(1.35, 0.9, 0.8)),
    (1.0, Vec3::new(0.2600742, 0.2021627, 0.7264151)),
];

/// The shadowed cloud colour, by phase.
pub const CLOUD_SHADOW_RAMP: [(f32, Vec3); 5] = [
    (0.0, Vec3::new(0.22761655, 0.11480955, 0.5660378)),
    (0.2500038, Vec3::new(0.5707547, 0.8513301, 1.0)),
    (0.5000076, Vec3::new(0.3537736, 0.6480374, 1.0)),
    (0.7499962, Vec3::new(1.0, 0.33490568, 0.6250066)),
    (1.0, Vec3::new(0.22761655, 0.11480955, 0.5660378)),
];

/// The sun disc size, by day fraction.
pub const SUN_SIZE_CURVE: [(f32, f32); 11] = [
    (0.0, 0.12),
    (0.14583333, 0.12),
    (0.16666667, 0.09),
    (0.1875, 0.0),
    (0.25, 0.3),
    (0.5, 0.1),
    (0.8125, 0.3),
    (0.8541667, 0.0),
    (0.875, 0.09),
    (0.9166667, 0.113015875),
    (1.0, 0.12),
];

/// The sun radiance exponent, by day fraction.
pub const SUN_RADIANCE_CURVE: [(f32, f32); 8] = [
    (0.0, -0.38),
    (0.16327958, -0.7),
    (0.29731447, -0.12),
    (0.5, 0.11993486),
    (0.71102285, -0.002634123),
    (0.79309154, -0.34879404),
    (0.9, -0.7),
    (1.0, -0.38),
];

/// The sun radiance intensity, by day fraction.
pub const SUN_RADIANCE_INTENSITY_CURVE: [(f32, f32); 7] = [
    (0.0, 1.0),
    (0.17, 0.9021137),
    (0.24828002, 1.0),
    (0.328332, 0.97904044),
    (0.50000167, 0.7838129),
    (0.75, 0.87),
    (1.0, 1.0),
];

pub const SKY_LUT_WIDTH: usize = 256;
pub const SKY_LUT_ROWS: usize = 5;

/// The sky lookup: rows night, sunrise, day, sunset, night; texel `x` is the row's gradient at
/// `x / 255`. Unity's `SkyboxLookPresetEditor.BakeSkyLut`, row-major, alpha 1.
pub fn bake_sky_lut() -> Vec<[f32; 4]> {
    let rows: [&[(f32, Vec3)]; SKY_LUT_ROWS] =
        [&SKY_NIGHT, &SKY_SUNRISE, &SKY_DAY, &SKY_SUNSET, &SKY_NIGHT];
    rows.iter()
        .flat_map(|row| {
            (0..SKY_LUT_WIDTH).map(move |x| {
                let c = sample_ramp(row, x as f32 / (SKY_LUT_WIDTH - 1) as f32);
                [c.x, c.y, c.z, 1.0]
            })
        })
        .collect()
}

pub const MOON_DISC_SIZE: f32 = 0.12;
pub const MOON_MASK_SIZE: f32 = 0.16;
pub const MOON_MASK_OFFSET: Vec2 = Vec2::new(-0.009, -0.007);

/// The horizon noise: strength, tiling U, tiling V, speed.
pub const HORIZON_NOISE: Vec4 = Vec4::new(0.1, 5.0, 10.0, 0.002);

/// One cloud strip layer.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CloudLayer {
    pub stretch_v: f32,
    pub offset_v: f32,
    pub tiling_u: f32,
    pub offset_u: f32,
    pub speed: f32,
    pub strength: f32,
    pub opacity: f32,
    /// Whether the layer flows at each phase anchor (night, sunrise, day, sunset).
    pub flow_by_phase: Vec4,
}

/// Overhead, low and cumulus layers, in that order.
pub const CLOUD_LAYERS: [CloudLayer; 3] = [
    CloudLayer {
        stretch_v: 0.4,
        offset_v: 0.2,
        tiling_u: 1.0,
        offset_u: 0.0,
        speed: 2.0,
        strength: 1.2,
        opacity: 1.0,
        flow_by_phase: Vec4::new(1.0, 1.0, 1.0, 1.0),
    },
    CloudLayer {
        stretch_v: 0.28,
        offset_v: -0.17,
        tiling_u: 1.0,
        offset_u: 0.5,
        speed: 1.4,
        strength: 1.2,
        opacity: 1.0,
        flow_by_phase: Vec4::new(1.0, 1.0, 1.0, 1.0),
    },
    CloudLayer {
        stretch_v: 0.4,
        offset_v: 0.05,
        tiling_u: 1.0,
        offset_u: 0.25,
        speed: 1.4,
        strength: 1.2,
        opacity: 1.0,
        flow_by_phase: Vec4::new(0.0, 0.0, 1.0, 0.0),
    },
];

/// Cloud ramp knee, highlight strength, highlight threshold, highlight falloff.
pub const CLOUD_PARAMS: Vec4 = Vec4::new(0.15, 2.0, 0.6, 3.0);
/// Cloud zenith fade start and end, occlusion start and end.
pub const CLOUD_FADES: Vec4 = Vec4::new(0.97, 1.0, 0.35, 0.9);

/// The V2 stars.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StarsV2 {
    pub brightness: f32,
    pub density: f32,
    /// Degrees.
    pub size: f32,
    /// Brightness at each phase anchor, see `evaluate_by_phase`.
    pub brightness_by_phase: Vec4,
    pub rotation_speed: f32,
    pub twinkle: f32,
    pub twinkle_speed: f32,
    pub patch_strength: f32,
    pub horizon_fade: Vec2,
    pub shooting_rate: f32,
}

pub const STARS_V2: StarsV2 = StarsV2 {
    brightness: 3.0,
    density: 24.1,
    size: 0.102,
    brightness_by_phase: Vec4::new(3.0, 0.1, 0.0, 0.5),
    rotation_speed: 0.001,
    twinkle: 1.0,
    twinkle_speed: 0.5,
    patch_strength: 0.7,
    horizon_fade: Vec2::new(0.02, 0.15),
    shooting_rate: 0.4,
};

/// Linear between the four phase anchors (night, sunrise, day, sunset), wrapping from the last
/// back to the first. Unity's `SkyboxLookPreset.EvaluateByPhase`.
pub fn evaluate_by_phase(by_phase: Vec4, phase: f32) -> f32 {
    let scaled = phase.clamp(0.0, 1.0) * 4.0;
    let anchor = (scaled as usize).min(3);
    let from = by_phase[anchor];
    let to = by_phase[(anchor + 1) % 4];
    from + (to - from) * (scaled - anchor as f32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn phase_anchors_match_the_preset() {
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
    fn ambient_ramps_hit_the_preset_keys() {
        assert_eq!(
            sample_ramp(&AMBIENT_SKY_RAMP, 0.7),
            Vec3::new(1.0, 0.49981537, 0.4584905)
        );
        assert_eq!(
            sample_ramp(&AMBIENT_EQUATOR_RAMP, 0.3),
            Vec3::new(0.90188676, 0.52418566, 0.4747668)
        );
        assert_eq!(
            sample_ramp(&AMBIENT_GROUND_RAMP, 0.135),
            Vec3::new(0.5471698, 0.18325028, 0.18325028)
        );
        // midnight and midday-plus-one wrap to the same colour
        assert_eq!(
            sample_ramp(&AMBIENT_SKY_RAMP, 1.0),
            sample_ramp(&AMBIENT_SKY_RAMP, 0.0)
        );
    }

    #[test]
    fn ambient_trilight_is_linear() {
        // noon is phase 0.5: sky (0.5187, 0.6788, 0.7380) sRGB
        let [sky, equator, ground] = ambient_trilight(0.5);
        let expected = Color::srgb(0.51868874, 0.6788208, 0.73800004)
            .to_linear()
            .to_vec3();
        assert!((sky - expected).length() < 1e-6);
        assert!(sky.z < 0.738 && sky.z > 0.5); // linearised, not the sRGB value
        assert!(equator.x > ground.x && ground.y < 0.02); // a dark red ground
    }

    #[test]
    fn fog_falls_off_as_a_base_two_exponential() {
        // exp2(-(density / sqrt(ln 2)) * z)
        let (density, z) = (FOG_DENSITY_PER_METRE, 200.0);
        let preset = (-(density / std::f32::consts::LN_2.sqrt()) * z).exp2();
        let bevy = (-fog_falloff_density(density) * z).exp();
        assert!((preset - bevy).abs() < 1e-6, "{preset} vs {bevy}");
    }

    #[test]
    fn day_fraction_wraps() {
        assert_eq!(day_fraction(6.0), 0.25);
        assert_eq!(day_fraction(24.0), 0.0);
        assert_eq!(day_fraction(30.0), 0.25);
    }

    #[test]
    fn sky_lut_rows_hit_the_gradient_ends() {
        let lut = bake_sky_lut();
        assert_eq!(lut.len(), SKY_LUT_WIDTH * SKY_LUT_ROWS);
        let day = &lut[2 * SKY_LUT_WIDTH..3 * SKY_LUT_WIDTH];
        let (first, last) = (SKY_DAY[0].1, SKY_DAY[SKY_DAY.len() - 1].1);
        assert_eq!(day[0], [first.x, first.y, first.z, 1.0]);
        assert_eq!(day[SKY_LUT_WIDTH - 1], [last.x, last.y, last.z, 1.0]);
        assert_eq!(lut[..SKY_LUT_WIDTH], lut[4 * SKY_LUT_WIDTH..]);
    }

    #[test]
    fn sky_lut_matches_unity() {
        // Texels of Presets/StylizedV1_SkyLut.asset (RGBAHalf): (row, column, rgb).
        let unity: [(usize, usize, [f32; 3]); 8] = [
            (0, 0, [0.030883789, 0.015792847, 0.17687988]),
            (0, 128, [0.22314453, 0.11029053, 0.38793945]),
            (1, 0, [2.1992188, 1.7001953, 1.0]),
            (1, 128, [0.42163086, 0.49047852, 0.75927734]),
            (2, 0, [1.0, 1.0, 1.0195313]),
            (2, 128, [0.004749298, 0.5620117, 1.0048828]),
            (3, 0, [2.0, 0.8999023, 0.5]),
            (3, 128, [0.5341797, 0.32885742, 0.7202148]),
        ];
        let lut = bake_sky_lut();
        for (row, x, rgb) in unity {
            let texel = lut[row * SKY_LUT_WIDTH + x];
            for c in 0..3 {
                assert!(
                    (texel[c] - rgb[c]).abs() < 2e-3,
                    "row {row} x {x} channel {c}: {} vs {}",
                    texel[c],
                    rgb[c]
                );
            }
            assert_eq!(texel[3], 1.0);
        }
    }

    #[test]
    fn evaluate_by_phase_wraps() {
        let by_phase = Vec4::new(3.0, 0.1, 0.0, 0.5);
        assert_eq!(evaluate_by_phase(by_phase, 0.0), 3.0);
        assert!((evaluate_by_phase(by_phase, 0.25) - 0.1).abs() < 1e-6);
        assert!((evaluate_by_phase(by_phase, 0.875) - 1.75).abs() < 1e-6);
        assert!((evaluate_by_phase(by_phase, 1.0) - 3.0).abs() < 1e-6);
    }

    #[test]
    fn cloud_layers_match_the_preset() {
        assert_eq!(CLOUD_LAYERS.map(|l| l.speed), [2.0, 1.4, 1.4]);
        assert_eq!(CLOUD_LAYERS.map(|l| l.offset_u), [0.0, 0.5, 0.25]);
        assert_eq!(CLOUD_LAYERS[2].flow_by_phase, Vec4::new(0.0, 0.0, 1.0, 0.0));
    }
}
