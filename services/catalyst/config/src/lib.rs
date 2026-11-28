//! Process configuration: the environment the node reads at startup and its typed settings. The
//! module keeps its application path (`crate::config`) and the crates it builds on are aliased
//! under theirs, so its code reads as it did in the application crate (AGENTS.md, docs/module-
//! layers.toml).

// A silently truncated, wrapped or rounded cast is a bug.
#![deny(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss
)]

pub mod config;
mod content;
mod create;
mod explore;
mod land_authz;
mod node;
mod ops;
pub mod posture;
mod presence;
mod price;
mod shop;
mod social;
mod worlds;
pub use worlds::{parse_ban_sweep_interval_ms, parse_island_changed_dedup_ms};
