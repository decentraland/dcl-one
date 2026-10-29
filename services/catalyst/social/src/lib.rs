//! Social: friendships, blocks, mutes, private calls, communities, scene and
//! platform moderation, notifications, badges, referral (schema `social`,
//! `schemas/40_social.sql`).
//!
//! `shared::identity` is the vocabulary every social module reuses for its keys and
//! its permission answers. The route modules each own one old service:
//! `communities` (social-api REST), `rpc` (the friends/presence/voice
//! WebSocket), `messaging` (the old gatekeeper's MLS delivery service, on
//! its own state), `notifications`, `badges`, `media`
//! (autotranslate) and `profile_images`; every one over the ONE pool, the
//! ONE outbound client, the ONE blob store and `jobs::spawn_periodic`. The
//! LiveKit gatekeeper itself is `comms::gatekeeper`; its state is the RPC's
//! voice runtime and the communities' voice eviction. [`router`] composes
//! them the way the old gateway's location table did, behind the [`writes`]
//! switch.

// The crates this one builds on, under their module names (AGENTS.md).
pub(crate) use catalyst_chain_index as chain_index;
use catalyst_comms as comms;
pub(crate) use catalyst_content as content;
#[cfg(test)]
pub(crate) use catalyst_db as db;
pub(crate) use catalyst_explore as explore;
pub(crate) use catalyst_fed as fed;
pub(crate) use catalyst_jobs as jobs;
pub(crate) use catalyst_shared as shared;
pub(crate) use catalyst_storage as storage;
pub(crate) use dcl_auth as auth;
pub(crate) use dcl_http as http;

use std::sync::Arc;

use axum::Router;

pub mod badges;
pub mod chat;
pub mod comms_directory;
pub mod communities;

pub mod media;
pub mod messaging;
mod mls;
pub mod notifications;
pub mod profile_images;
pub mod rpc;
#[cfg(test)]
mod test_fixtures;
pub mod writes;

pub use crate::shared::identity::{
    self, Address, AddressError, AuthorityNotEstablished, BanScope, CommunityId,
    CommunityPermission, CommunityRole, CommunityStanding, PermissionDecision,
};

/// The eight social states `serve` builds; [`router`] owns their routes
/// but the MLS delivery service's, which `serve` mounts under
/// `/comms-gatekeeper` with [`messaging::router`].
pub struct States {
    pub communities: communities::SharedState,
    pub comms: comms::gatekeeper::SharedState,
    pub messaging: messaging::SharedState,
    pub notifications: notifications::SharedState,
    pub badges: badges::SharedState,
    pub media: media::SharedState,
    pub profile_images: profile_images::SharedState,
    pub rpc: Arc<rpc::State>,
}

/// The old gateway's location table for the social bundle
/// (`catalyrst/nixos/web-gateway.nix`): `/social-api`, `/comms-gatekeeper`,
/// `/notifications`, `/badges`, `/autotranslate-server` and
/// `/profile-images` were stripped by nginx before reaching the bundle, and
/// the `social-api.`, `comms-gatekeeper.`, `notifications.`, `badges.`,
/// `autotranslate-server.` and `profile-images.` vhosts proxied their roots to
/// it unchanged. Here every prefix is nested, and the routers whose paths do
/// not collide with the content/lambdas root are also merged at the root
/// (communities' `/v1`, `/v2`, `/social`; media's `/translate`; the profile
/// images' `/entities/{cid}/{face,body}.png`), so a per-vhost proxy for
/// those needs no rewrite. `/comms-gatekeeper` (the gatekeeper's routes with
/// [`messaging::router`]), the world comms mints and the Pulse room
/// authority are mounted by `serve` beside this router, the first behind the
/// same switch. Notifications carries `/notifications` (its own prefix) and
/// badges `/users/{address}/...` (the gatekeeper's shape), so those answer
/// under their prefix only: a per-vhost proxy forwards
/// `notifications.<domain>/x` to `/notifications/x` and sets
/// `x-original-path` so the signed-fetch signature still verifies. The
/// social RPC socket is `/social-rpc` (the old `rpc-social-service-ea.`
/// vhost root; the handshake accepts a signature over `/` too) and its admin
/// routes `/admin/social/*`. Every router is wrapped in the
/// [`writes::guard`] with the process's `SOCIAL_WRITES_ENABLED`.
pub fn router(states: &States, writes_enabled: bool) -> Router {
    let guard = |r: Router| writes::guard(r, writes_enabled);
    Router::new()
        .merge(guard(communities::router(states.communities.clone())))
        .nest(
            "/social-api",
            guard(communities::router(states.communities.clone()))
                .route(
                    "/openapi.json",
                    axum::routing::get(|| async {
                        crate::shared::openapi::document(crate::shared::openapi::COMMUNITIES)
                    }),
                )
                .merge(communities::health_router()),
        )
        .nest(
            "/notifications",
            guard(notifications::router(states.notifications.clone())),
        )
        .nest("/badges", guard(badges::router(states.badges.clone())))
        .merge(guard(media::router(states.media.clone())))
        .nest(
            "/autotranslate-server",
            guard(media::router(states.media.clone())),
        )
        .merge(guard(profile_images::router(states.profile_images.clone())))
        .nest(
            "/profile-images",
            guard(profile_images::router(states.profile_images.clone())),
        )
        .merge(guard(rpc::router(states.rpc.clone())))
}
