#![forbid(unsafe_code)]

#[cfg(target_arch = "wasm32")]
pub mod inner;
pub mod relay_wake;
pub mod scene_loop_mode;
pub mod scene_replies;
pub mod tick_limits;

#[cfg(target_arch = "wasm32")]
pub use inner::*;
