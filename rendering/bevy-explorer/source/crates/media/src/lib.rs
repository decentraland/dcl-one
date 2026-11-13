mod av;
#[cfg(not(target_arch = "wasm32"))]
mod capture;
#[cfg(all(not(target_arch = "wasm32"), feature = "ffmpeg"))]
mod ffmpeg;
#[cfg(all(target_arch = "wasm32", feature = "html"))]
pub mod html;
pub mod plugin;

pub use av::*;
#[cfg(not(target_arch = "wasm32"))]
pub use capture::{AudioCapture, CAPTURE_CHANNELS, CAPTURE_FRAMES};
#[cfg(all(not(target_arch = "wasm32"), feature = "ffmpeg"))]
pub use ffmpeg::*;
#[cfg(all(target_arch = "wasm32", feature = "html"))]
pub use html::*;
