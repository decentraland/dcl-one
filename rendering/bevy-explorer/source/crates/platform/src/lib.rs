#![forbid(unsafe_code)]

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

/// The shadows weight `w` of the grading: dark pixels are multiplied by `1 + 4w`, weighted by
/// `1 - smoothstep(0, 0.3, luma)`. It is a gain on dark pixels, not an additive lift.
pub const GRADING_SHADOWS_WEIGHT: f32 = 0.098987624;
/// The grading's shadows fade out from 0 to 0.3 and its highlights start at 0.55. bevy fades its
/// shadows section out in a straight line from 0.1 under the range's start to 0.1 over it
/// (`sectional_color_grading`), so the start is the middle of that fade: the two weights then
/// differ by under 0.08 at any brightness.
pub const GRADING_MIDTONES_RANGE: Range<f32> = 0.15..0.55;

pub fn default_color_grading() -> ColorGrading {
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

/// The bloom's scatter setting, 0.5, as the lerp factor towards the coarser mip on each
/// upsample: `lerp(0.05, 0.95, scatter)`.
pub const BLOOM_SCATTER: f32 = 0.05 + 0.9 * 0.5;

/// The bloom of the High setting: threshold 1, intensity 1, scatter 0.5.
///
/// A scatter bloom upsamples with `lerp(fine, coarse, SCATTER)` folded over the mip chain and
/// adds the result once, so mip `i` weighs `(1 - s) s^i`. bevy's `Additive` mode adds each mip
/// with its own blend constant (`compute_blend_factor`): `intensity` for mip 0 and
/// `intensity + low_frequency_boost` for the rest (the boost curve is a near-step at mip 1 with
/// curvature 0.99), and the weights multiply down the chain. So intensity is `1 - s` and the
/// boost brings the other mips to `s`; bevy's intensity 1 would instead sum eight mips at
/// weights above 1 and wash the frame out. The prefilter knee is `threshold * 0.5`.
pub fn scatter_bloom() -> Bloom {
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
