//! Node integration tests that need no socket budget, one binary for several files so a
//! build links few test executables. Each file stays a module named after it.

// Support shared by several modules, loaded once; the modules `use crate::<name>`.
#[path = "support/auth.rs"]
mod auth;
#[path = "support/db.rs"]
mod db;

#[path = "failed_deployments_retry_backoff.rs"]
mod failed_deployments_retry_backoff;
#[path = "ops_live.rs"]
mod ops_live;
#[path = "schema_upgrade_from_closure_base.rs"]
mod schema_upgrade_from_closure_base;
#[path = "snapshot_generation.rs"]
mod snapshot_generation;
#[path = "sync_flush_overwrites.rs"]
mod sync_flush_overwrites;
#[path = "sync_frontier_monotonic.rs"]
mod sync_frontier_monotonic;
#[path = "sync_pause_control.rs"]
mod sync_pause_control;
#[path = "sync_peer_changes.rs"]
mod sync_peer_changes;
#[path = "worlds_acceptance.rs"]
mod worlds_acceptance;
