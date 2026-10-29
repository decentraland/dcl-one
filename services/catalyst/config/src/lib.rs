//! Process configuration: the environment the node reads at startup and its typed settings. The
//! module keeps its application path (`crate::config`) and the crates it builds on are aliased
//! under theirs, so its code reads as it did in the application crate (AGENTS.md, docs/module-
//! layers.toml).

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
