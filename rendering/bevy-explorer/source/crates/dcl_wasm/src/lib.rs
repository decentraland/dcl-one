#![forbid(unsafe_code)]

#[cfg(target_arch = "wasm32")]
pub mod inner;
pub mod scene_loop_mode;
pub mod tick_limits;

#[cfg(target_arch = "wasm32")]
pub use inner::*;
