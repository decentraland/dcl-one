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
    core_pipeline::{
        bloom::{Bloom, BloomCompositeMode, BloomPrefilter},
        dof::DepthOfField,
        tonemapping::Tonemapping,
    },
    ecs::component::Component,
    render::view::{ColorGrading, ColorGradingGlobal, ColorGradingSection},
};
use std::ops::Range;

/// A camera's own look, as the explorer sets it: the camera's spawn, the player's settings, the
/// console and the auto-focus. They edit this, not the camera's `Tonemapping`, `ColorGrading`,
/// `Bloom` and `DepthOfField`: `visuals::post_processing` writes those from it, with a scene's
/// settings over it.
#[derive(Component, Clone)]
pub struct CameraLook {
    pub tonemapping: Tonemapping,
    pub grading: ColorGrading,
    pub bloom: Option<Bloom>,
    pub dof: Option<DepthOfField>,
}

impl Default for CameraLook {
    fn default() -> Self {
        Self {
            tonemapping: Tonemapping::AcesFitted,
            grading: default_color_grading(),
            bloom: Some(scatter_bloom(BLOOM_SCATTER)),
            dof: None,
        }
    }
}

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

/// The Explorer's bloom scatter setting, the one the Bloom setting turns on.
pub const BLOOM_SCATTER: f32 = 0.5;

/// A scatter setting as the lerp factor towards the coarser mip on each upsample:
/// `lerp(0.05, 0.95, scatter)`.
pub fn scatter_lerp(scatter: f32) -> f32 {
    0.05 + 0.9 * scatter
}

/// A bloom with threshold 1, intensity 1 and the given scatter setting.
///
/// A scatter bloom upsamples with `lerp(fine, coarse, s)` folded over the mip chain, where `s`
/// is `scatter_lerp(scatter)`, and adds the result once, so mip `i` weighs `(1 - s) s^i`. bevy's
/// `Additive` mode adds each mip with its own blend constant (`compute_blend_factor`):
/// `intensity` for mip 0 and `intensity + low_frequency_boost` for the rest (the boost curve is
/// a near-step at mip 1 with curvature 0.99), and the weights multiply down the chain. So
/// intensity is `1 - s` and the boost brings the other mips to `s`; bevy's intensity 1 would
/// instead sum eight mips at weights above 1 and wash the frame out. The prefilter knee is
/// `threshold * 0.5`.
pub fn scatter_bloom(scatter: f32) -> Bloom {
    let s = scatter_lerp(scatter);
    let intensity = 1.0 - s;
    Bloom {
        intensity,
        low_frequency_boost: s - intensity,
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

/// `bloom` with its whole glow multiplied by `scale`, capped where the composite reaches full
/// weight (or at `bloom` itself, if it is past that). bevy multiplies the blend factors down the
/// mip chain, so the scale goes on the composite (mip 0, which the low-frequency boost never
/// reaches) and the boost keeps every other mip's factor. For an additive bloom.
pub fn scaled_bloom(bloom: &Bloom, scale: f32) -> Bloom {
    let cap = bloom.intensity.recip().max(1.0);
    let intensity = bloom.intensity * scale.min(cap).max(0.0);
    Bloom {
        intensity,
        low_frequency_boost: bloom.intensity + bloom.low_frequency_boost - intensity,
        ..bloom.clone()
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
