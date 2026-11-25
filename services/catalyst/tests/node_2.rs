//! Node integration tests that need no socket budget, one binary for several files so a
//! build links few test executables. Each file stays a module named after it.
#![allow(clippy::unwrap_used, reason = "test support")]

#[path = "comms_v4_assignment_fence.rs"]
mod comms_v4_assignment_fence;
#[path = "create_live.rs"]
mod create_live;
#[path = "social_notifications_fixtures.rs"]
mod social_notifications_fixtures;
#[path = "social_provider_bootstrap.rs"]
mod social_provider_bootstrap;
#[path = "stats_metrics_startup.rs"]
mod stats_metrics_startup;
#[path = "worlds_fed_admission.rs"]
mod worlds_fed_admission;
#[path = "worlds_fed_mirror.rs"]
mod worlds_fed_mirror;
#[path = "worlds_live.rs"]
mod worlds_live;
#[path = "worlds_notices.rs"]
mod worlds_notices;
#[path = "worlds_read_through.rs"]
mod worlds_read_through;
