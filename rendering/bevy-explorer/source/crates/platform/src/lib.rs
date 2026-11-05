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
