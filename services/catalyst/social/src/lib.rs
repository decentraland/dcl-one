//! Social: friendships, blocks, mutes, private calls, communities, scene and
//! platform moderation, notifications, badges, referral (schema `social`,
//! `schemas/40_social.sql`).
//!
//! `shared::identity` is the vocabulary every social module reuses for its keys and
//! its permission answers. The route modules each own one old service:
//! `communities` (social-api REST), `rpc` (the friends/presence/voice
//! WebSocket), `messaging` (the old gatekeeper's MLS delivery service, on
//! its own state), `notifications`, `badges`, `media`
//! (autotranslate), `profile_images` and `segment_webhooks` (the
//! events-notifier's Segment routes); domain readers share the default outbound
//! client, SQL pool and blob store. Configured AI compliance, analytics and
//! referral Slack each lazily retain one Outbound provider profile with its
//! own HTTP transport pools; no profile is built per request. The
//! LiveKit gatekeeper itself is `comms::gatekeeper`; its state is the RPC's
//! voice runtime and the communities' voice eviction. [`router`] composes
//! them the way the old gateway's location table did, behind the [`writes`]
//! switch.

// Production routes go through `shared::routing` (social/clippy.toml); test routers may use axum's.
#![cfg_attr(not(test), deny(clippy::disallowed_methods))]
// Counts, pages and chain values: a silently truncated, wrapped or rounded cast is a bug.
#![deny(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss
)]
#![cfg_attr(
    test,
    allow(
        clippy::disallowed_methods,
        reason = "test routers may use axum's routing"
    )
)]

// The crates this one builds on, under their module names (AGENTS.md).
pub(crate) use catalyst_chain_index as chain_index;
use catalyst_comms as comms;
pub(crate) use catalyst_config::config;
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

pub mod analytics;
pub mod badges;
mod casts;
pub mod chat;
pub mod comms_directory;
pub mod communities;

pub mod media;
pub mod messaging;
mod metrics;
mod mls;
pub mod notifications;
pub mod profile_images;
pub mod request_auth;
pub use metrics::FAMILIES as SERVICE_FAMILIES;
pub mod rpc;
pub mod segment_webhooks;
#[cfg(any(test, feature = "test-support"))]
#[allow(clippy::unwrap_used, reason = "test support")]
pub mod test_fixtures;
pub mod writes;

pub use crate::shared::identity::{
    self, Address, AddressError, AuthorityNotEstablished, BanScope, CommunityId,
    CommunityPermission, CommunityRole, CommunityStanding, PermissionDecision,
};

/// The nine social states `serve` builds; [`router`] owns their routes
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
    pub segment_webhooks: segment_webhooks::SharedState,
}

/// The social API host's unmatched paths and preflights apply to its entire
/// gateway prefix, including paths no domain router registered.
pub fn social_api_gateway(router: Router) -> Router {
    social_api_gateway_with_limit(router, 1024 * 1024)
}

pub fn social_api_gateway_with_limit(router: Router, max_body_bytes: usize) -> Router {
    // Wrap the complete method router: Axum adds Allow after each individual
    // route's layers, whereas source allowedMethods and CORS surround routing.
    let router = router.fallback(shared::routing::erase(|| async {
        use axum::response::IntoResponse;
        let mut response = (axum::http::StatusCode::NOT_FOUND, "Not found").into_response();
        response
            .extensions_mut()
            .insert(communities::http::SocialApiNotFound);
        response
    }));
    Router::new()
        .fallback_service(dcl_metrics::http::component(router, "social"))
        .layer(axum::middleware::from_fn(
            communities::http::social_api_methods,
        ))
        .layer(axum::middleware::from_fn(rpc::http::instrument_metrics))
        .layer(axum::middleware::from_fn(
            communities::cors::social_api_cors,
        ))
        .layer(axum::middleware::from_fn_with_state(
            max_body_bytes,
            communities::http::social_api_body_limit,
        ))
        .layer(axum::middleware::map_response(
            communities::cors::content_type_options,
        ))
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
/// vhost root; its handshake signature binds `/`) and its admin
/// routes `/admin/social/*`. Every router is wrapped in the
/// [`writes::guard`] with the process's `SOCIAL_WRITES_ENABLED`.
pub fn router(states: &States, writes_enabled: bool) -> Router {
    let guard = |r: Router| writes::guard(r, writes_enabled);
    // The social-api host: the communities routes and the HTTP twins of the
    // social RPC procedures (`rpc::http`).
    let social_api = || {
        guard(communities::router(states.communities.clone()))
            .merge(guard(rpc::http::router(states.rpc.clone())))
    };
    Router::new()
        .merge(social_api().layer(axum::middleware::from_fn(rpc::http::instrument_metrics)))
        .nest(
            "/social-api",
            social_api_gateway_with_limit(
                social_api()
                    .route(
                        "/openapi.json",
                        shared::routing::get(|| async {
                            crate::shared::openapi::document(crate::shared::openapi::COMMUNITIES)
                        }),
                    )
                    .merge(communities::health_router()),
                states.communities.settings.http_max_request_body_bytes,
            ),
        )
        .nest(
            "/notifications",
            guard(notifications::router(states.notifications.clone())),
        )
        .nest("/badges", guard(badges::router(states.badges.clone())))
        .nest(
            "/events-notifier",
            guard(segment_webhooks::router(states.segment_webhooks.clone())),
        )
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
