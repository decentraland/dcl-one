pub mod client_login_store;
mod http_client;
pub use http_client::http_client_builder;

#[cfg(not(target_arch = "wasm32"))]
pub mod devtools;
#[cfg(not(target_arch = "wasm32"))]
mod native;
#[cfg(not(target_arch = "wasm32"))]
pub use native::*;

#[cfg(target_arch = "wasm32")]
mod wasm;
#[cfg(target_arch = "wasm32")]
pub use wasm::*;

use bevy::{
    core_pipeline::bloom::{Bloom, BloomCompositeMode, BloomPrefilter},
    render::view::{ColorGrading, ColorGradingGlobal, ColorGradingSection},
};
use std::ops::Range;

/// unity: Quality Settings.asset "Custom Volume Profile 0", ShadowsMidtonesHighlights: shadows
/// `(1, 1, 1, w)`. URP (`ColorUtils.PrepareShadowsMidtonesHighlights`) adds `4w` to the rgb
/// multiplier and the LUT (`LutBuilderHdr.shader`) multiplies the colour by it, weighted by
/// `1 - smoothstep(shadowsStart, shadowsEnd, luma)`; so it is a gain on dark pixels, not an
/// additive lift. bevy weights its shadow band as a near-step at `GRADING_MIDTONES_RANGE.start`
/// where URP tapers from 0, so the band just under 0.3 brightens slightly more than in Unity.
pub const GRADING_SHADOWS_WEIGHT: f32 = 0.098987624;
pub const GRADING_MIDTONES_RANGE: Range<f32> = 0.3..0.55;

pub fn unity_color_grading() -> ColorGrading {
    ColorGrading {
        global: ColorGradingGlobal {
            midtones_range: GRADING_MIDTONES_RANGE,
            ..Default::default()
        },
        shadows: ColorGradingSection {
            gain: 1.0 + 4.0 * GRADING_SHADOWS_WEIGHT,
            ..Default::default()
        },
        midtones: ColorGradingSection::default(),
        highlights: ColorGradingSection::default(),
    }
}

/// unity: same profile, Bloom scatter 0.3: `Mathf.Lerp(0.05, 0.95, scatter)` (`BloomPostProcessPass.cs`),
/// the lerp factor towards the coarser mip on each upsample.
pub const BLOOM_SCATTER: f32 = 0.05 + 0.9 * 0.3;

/// unity: same profile, Bloom threshold 1, intensity 1, scatter 0.3, Gaussian filter.
///
/// URP (`Bloom.shader`, `UberPost.shader`) upsamples with `lerp(fine, coarse, SCATTER)` folded
/// over the mip chain and adds the result once, so mip `i` weighs `(1 - s) s^i`. bevy's
/// `Additive` mode adds each mip with its own blend constant (`compute_blend_factor`):
/// `intensity` for mip 0 and `intensity + low_frequency_boost` for the rest (the boost curve is
/// a near-step at mip 1 with curvature 0.99), and the weights multiply down the chain. So
/// intensity is `1 - s` and the boost brings the other mips to `s`; bevy's intensity 1 would
/// instead sum eight mips at weights above 1 and wash the frame out. The prefilter knee is URP's
/// hardcoded `threshold * 0.5`.
pub fn unity_bloom() -> Bloom {
    let intensity = 1.0 - BLOOM_SCATTER;
    Bloom {
        intensity,
        low_frequency_boost: BLOOM_SCATTER - intensity,
        low_frequency_boost_curvature: 0.99,
        high_pass_frequency: 1.0,
        prefilter: BloomPrefilter {
            threshold: 1.0,
            threshold_softness: 0.5,
        },
        composite_mode: BloomCompositeMode::Additive,
        ..Bloom::NATURAL
    }
}

/// One stored DM, or a reaction to one. `from` is the sender's wallet (the account or the
/// partner).
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct DmHistoryEntry {
    pub from: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub message: String,
    /// Unix seconds when the local client stored it.
    pub received_at: f64,
    /// The sender's rfc4 timestamp, which names the DM for reactions. Missing on DMs stored
    /// before reactions.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sent_at: Option<f64>,
    /// Set when this record is a reaction by `from` to an earlier DM rather than a DM.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reaction: Option<DmHistoryReaction>,
}

/// A reaction added to or removed from a stored DM.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct DmHistoryReaction {
    pub message_id: String,
    pub emoji: String,
    pub remove: bool,
}

pub mod http;
pub use http::{fetch, within, FetchError, FetchedResponse, TracedJsonResponse};
