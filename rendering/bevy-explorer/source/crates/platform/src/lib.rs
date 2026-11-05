mod http_client;
pub use http_client::http_client_builder;

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

/// unity: Quality Settings.asset "Custom Volume Profile 0", ShadowsMidtonesHighlights
pub const GRADING_SHADOWS_LIFT: f32 = 0.098987624;
pub const GRADING_MIDTONES_RANGE: Range<f32> = 0.3..0.55;

pub fn unity_color_grading() -> ColorGrading {
    ColorGrading {
        global: ColorGradingGlobal {
            midtones_range: GRADING_MIDTONES_RANGE,
            ..Default::default()
        },
        shadows: ColorGradingSection {
            lift: GRADING_SHADOWS_LIFT,
            ..Default::default()
        },
        midtones: ColorGradingSection::default(),
        highlights: ColorGradingSection::default(),
    }
}

/// unity: same profile, Bloom
pub fn unity_bloom() -> Bloom {
    Bloom {
        intensity: 1.0,
        low_frequency_boost: 0.3,
        prefilter: BloomPrefilter {
            threshold: 1.0,
            threshold_softness: 0.0,
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
pub use http::{fetch, FetchError, FetchedResponse};
