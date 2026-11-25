mod av;
#[cfg(all(target_arch = "wasm32", feature = "html"))]
pub mod html;
pub mod plugin;
#[cfg(any(test, all(target_arch = "wasm32", feature = "html")))]
mod retry;
#[cfg(any(test, all(target_arch = "wasm32", feature = "html")))]
mod source_state;
#[cfg(all(not(target_arch = "wasm32"), feature = "uuav"))]
mod uuav;

pub use av::*;
#[cfg(all(target_arch = "wasm32", feature = "html"))]
pub use html::*;
#[cfg(any(test, all(target_arch = "wasm32", feature = "html")))]
pub use retry::{MAX_REOPENS, MediaRetryStatus, first_reopen_delay, set_first_reopen_delay};
#[cfg(all(not(target_arch = "wasm32"), feature = "uuav"))]
pub use uuav::*;

/// Page-wide media element id.
pub type MediaId = u32;
