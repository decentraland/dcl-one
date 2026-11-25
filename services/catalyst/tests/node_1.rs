//! Node integration tests that need no socket budget, one binary for several files so a
//! build links few test executables. Each file stays a module named after it.
// explore_parity needs it.
#![recursion_limit = "256"]

#[path = "content_live.rs"]
mod content_live;
#[path = "explore_mirrors.rs"]
mod explore_mirrors;
#[path = "explore_node_origin.rs"]
mod explore_node_origin;
#[path = "explore_parity.rs"]
mod explore_parity;
#[path = "gatekeeper_policy.rs"]
mod gatekeeper_policy;
#[path = "index_process.rs"]
mod index_process;
#[path = "notifier_parity.rs"]
mod notifier_parity;
#[path = "social_live.rs"]
mod social_live;
#[path = "worlds_mirror.rs"]
mod worlds_mirror;
