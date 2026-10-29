//! Comms and multiplayer, on layer 8 under worlds and social: `pulse` (the
//! real-time layer), the archipelago connector (the `/ws` sockets that hand
//! each wallet its island), the comms gatekeeper (admission, moderation,
//! voice, streaming, the LiveKit webhook and room events) and live presence.
//! Worlds, social and shop call it; it imports nothing from worlds, social or
//! above. It also owns the vocabulary comms shares with the crates above it: the
//! world identity types (`world_identity`), the access vocabulary of a world
//! join (`access`), the per-key rate limiter (`limits`), the worlds error
//! envelope (`worlds_envelope`), the switch a side write asks (`writes`) and
//! the worlds signed-fetch verifier (`signed_fetch`).

// The crates this one builds on, under their module names (AGENTS.md).
pub(crate) use catalyst_chain_index as chain_index;
pub(crate) use catalyst_content as content;
#[cfg(test)]
pub(crate) use catalyst_db as db;
pub(crate) use catalyst_explore as explore;
pub(crate) use catalyst_jobs as jobs;
pub(crate) use catalyst_shared as shared;
pub(crate) use dcl_auth as auth;
pub(crate) use dcl_http as http;

pub mod access;
pub mod archipelago;
pub mod deny_list;
pub mod gatekeeper;
pub mod interfaces;
pub mod limits;
pub mod metrics;
pub mod presence;
pub mod protocol;
pub mod pulse;
pub mod rooms;
pub mod signed_fetch;
pub mod topology;
pub mod world_identity;
pub mod worlds_envelope;
pub mod writes;

pub use shared::identity::{
    Address, AuthorityNotEstablished, CommunityId, PermissionDecision, ZERO_ADDRESS,
};
