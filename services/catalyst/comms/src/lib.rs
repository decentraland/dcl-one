//! Comms and multiplayer, on layer 8 under worlds and social: `pulse` (the
//! real-time layer), the archipelago connector (the `/ws` sockets that hand
//! each wallet its island), the comms gatekeeper (admission, moderation,
//! voice, streaming, the LiveKit webhook and room events) and live presence.
//! Worlds and social call it; it imports nothing from worlds, social or
//! above. It also owns the vocabulary comms shares with the crates above it: the
//! world identity types (`world_identity`), the access vocabulary of a world
//! join (`access`), the per-key rate limiter (`limits`, from `shared`), the worlds error
//! envelope (`worlds_envelope`), the switch a side write asks (`writes`) and
//! the worlds signed-fetch verifier (`signed_fetch`).

// Production routes go through `shared::routing` (comms/clippy.toml); test routers may use axum's.
#![cfg_attr(not(test), deny(clippy::disallowed_methods))]
#![cfg_attr(
    test,
    allow(
        clippy::disallowed_methods,
        reason = "test routers may use axum's routing"
    )
)]

// The crates this one builds on, under their module names (AGENTS.md).
pub(crate) use catalyst_chain_index as chain_index;
pub(crate) use catalyst_content as content;
pub(crate) use catalyst_db as db;
pub(crate) use catalyst_jobs as jobs;
pub(crate) use catalyst_shared as shared;
pub(crate) use dcl_auth as auth;
pub(crate) use dcl_http as http;

pub mod access;
pub mod archipelago;
pub mod cast_presenter;
pub mod casts;
pub mod deny_list;
pub mod gatekeeper;
pub mod interfaces;
pub use crate::shared::limits;
pub mod metrics;
pub mod presence;
pub mod protocol;
pub mod pulse;
pub mod rooms;
pub mod signed_fetch;
pub mod stats;
#[cfg(test)]
mod test_logs;
pub mod topology;
pub mod world_identity;
pub mod worlds_envelope;
pub mod writes;

pub use shared::identity::{
    Address, AuthorityNotEstablished, CommunityId, PermissionDecision, ZERO_ADDRESS,
};

/// Production routes stay on `shared::routing` (see `shared::routing::guard`).
#[cfg(test)]
#[test]
fn production_routes_use_shared_routing() {
    let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let found = crate::shared::routing::guard::direct_axum_routes(&src);
    assert!(
        found.is_empty(),
        "use crate::shared::routing instead of axum's method routers at {found:?}"
    );
}

/// Production spawns stay on `shared::task` (see `shared::routing::guard`).
#[cfg(test)]
#[test]
fn production_spawns_use_shared_task() {
    let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let found = crate::shared::routing::guard::direct_tokio_spawns(&src);
    assert!(
        found.is_empty(),
        "use crate::shared::task instead of tokio::spawn at {found:?}"
    );
}
