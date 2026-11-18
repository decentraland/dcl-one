mod av;
#[cfg(all(not(target_arch = "wasm32"), feature = "ffmpeg"))]
mod ffmpeg;
#[cfg(all(target_arch = "wasm32", feature = "html"))]
pub mod html;
pub mod plugin;
mod retry;
#[cfg(any(test, all(target_arch = "wasm32", feature = "html")))]
mod source_state;

pub use av::*;
#[cfg(all(not(target_arch = "wasm32"), feature = "ffmpeg"))]
pub use ffmpeg::*;
#[cfg(all(target_arch = "wasm32", feature = "html"))]
pub use html::*;
pub use retry::{MAX_REOPENS, MediaRetryStatus, first_reopen_delay, set_first_reopen_delay};

/// Page-wide media element id.
pub type MediaId = u32;
