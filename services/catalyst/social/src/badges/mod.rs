//! Badges: the read surface of badges.decentraland.org (`/badges`,
//! `/badges/{id}`, `/badges/{id}/tiers`, `/categories`, `/status`,
//! `/users/{address}/badges`, `/users/{address}/preview`), the bearer-gated
//! admin read, grant, revoke and [`backfill`] (grant and revoke with an
//! audit row), and badge art mirrored into `BADGES_ASSETS_DIR` at
//! `/assets/{path}`. Revoke answers 200 with a body in every environment;
//! upstream registers it in dev and test only, with a bare 204. A missing,
//! wrong or unconfigured bearer is 403 on every admin route (the shared
//! `RequireAdmin`); upstream answers backfill 401 and does not register it
//! without a token. Tables:
//! `social.badge_definitions`, `badge_tiers` (upstream's catalog, seeded by
//! the schema), `user_badge_progress`, `user_achieved_tiers`,
//! `badge_admin_audit`, `presence_badges`. Every progress write goes through
//! [`progress::apply_progress`]: the console grant and the backfill here.
//! `badge_admin_audit`, `presence_badges`, `badge_cursors`. Every progress
//! write goes through [`progress::apply_progress`]: the console grant and the
//! processor's [`jobs`], which feed upstream's observers ([`evaluators`]).
//! [`progress::apply_progress`]: the console grant and the presence flow
//! ([`presence`], with its runtime tables `presence_events`, `presence_visits`
//! and the applied, pending-leave and absence facts).
//! admin read, grant and revoke (the last two with an audit row), the
//! bearer-gated catalog writes ([`catalog`]: `PUT` and `PATCH
//! /badges/{id}`), and badge art mirrored into `BADGES_ASSETS_DIR` at
//! `/assets/{path}`. Tables: `social.badge_definitions`, `badge_tiers`
//! (upstream's catalog, seeded by the schema), `user_badge_progress`,
//! `user_achieved_tiers`, `badge_admin_audit`, `badge_catalog_audit`,
//! `presence_badges`. Every progress write goes through
//! [`progress::apply_progress`]; the console grant is its only caller here.
//!
//! Mounted by `serve` as `.nest("/badges", router(state))` (the old gateway
//! stripped that prefix), so `<node>/badges` is the base URL of upstream's
//! paths, and so is the `badges.<domain>` host. At the node root the gateway
//! aliases only `/users/0x…/{badges,preview}` (what the web profile reads);
//! upstream's `/badges`, `/badges/{id}[/tiers]`, `/categories` and `/status`
//! there would collide with this prefix and with places' `/categories`.

pub mod assets;
pub mod backfill;
pub mod evaluators;
pub mod catalog;
pub mod handlers;
pub mod jobs;
pub mod presence;
pub mod progress;
pub mod store;
pub mod types;

use std::path::PathBuf;
use std::sync::Arc;

use axum::Router;
use axum::routing::{get, post};
use axum::extract::DefaultBodyLimit;
use axum::routing::get;
use sqlx::PgPool;

use crate::notifications::admin::AdminSecretSource;

pub use progress::{Applied, ApplyOptions, Progress, ProgressChange, apply_progress};
pub use store::{BadgesStore, DEFAULT_ASSET_BASE_URL};

/// Request bodies of every route; the catalog's are far below it.
pub const MAX_BODY_BYTES: usize = 1024 * 1024;

pub const ADMIN_TOKEN_ENV: &str = "CATALYRST_BADGES_ADMIN_TOKEN";

#[derive(Clone, Debug)]
pub struct Settings {
    /// `CATALYRST_BADGES_ADMIN_TOKEN`: bearer of the admin routes; unset refuses them.
    pub admin_token: Option<String>,
    /// `BADGES_PUBLIC_ASSET_BASE_URL`: the base every catalog asset path
    /// resolves against (default: upstream's CDN, so the URLs are
    /// upstream's). `<node>/badges/assets` serves `assets_dir` instead.
    pub public_asset_base_url: String,
    /// `BADGES_ASSETS_DIR`: the mirrored art `/assets/{path}` serves.
    pub assets_dir: Option<PathBuf>,
    /// The commit `/status` reports.
    pub commit_hash: Option<String>,
    /// The badge processor ([`jobs`]).
    pub processor: jobs::ProcessorSettings,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            admin_token: None,
            public_asset_base_url: DEFAULT_ASSET_BASE_URL.to_string(),
            assets_dir: None,
            commit_hash: None,
            processor: jobs::ProcessorSettings::default(),
        }
    }
}

pub struct State {
    pub badges: BadgesStore,
    pub settings: Settings,
}

pub type SharedState = Arc<State>;

impl State {
    pub fn new(pool: PgPool, settings: Settings) -> Self {
        let base = settings
            .public_asset_base_url
            .trim_end_matches('/')
            .to_string();
        Self {
            badges: BadgesStore::new(pool, base),
            settings,
        }
    }
}

impl AdminSecretSource for SharedState {
    const ENVIRONMENT_VARIABLE: &'static str = ADMIN_TOKEN_ENV;
    const UNSET_MESSAGE: &'static str = "admin token not configured";
    const MISMATCH_MESSAGE: &'static str = "missing or invalid bearer token";
    fn admin_token(&self) -> Option<&str> {
        self.settings.admin_token.as_deref()
    }
}

/// The routes, unprefixed: nest under `/badges`. CORS is permissive as the
/// old service's was, and a preflight answers 204 as upstream's does.
pub fn router(state: SharedState) -> Router {
    Router::new()
        .route("/badges", get(handlers::get_badges))
        .route(
            "/badges/{badge_id}",
            get(handlers::get_badge)
                .put(handlers::put_badge)
                .patch(handlers::patch_badge),
        )
        .route("/badges/{badge_id}/tiers", get(handlers::get_badge_tiers))
        .route(
            "/badges/{badge_id}/backfill",
            post(backfill::backfill_badge),
        )
        .route("/categories", get(handlers::get_categories))
        .route("/status", get(handlers::get_status))
        .route("/users/{address}/preview", get(handlers::get_user_preview))
        .route("/users/{address}/badges", get(handlers::get_user_badges))
        .route(
            "/users/{address}/badges/{badge_id}",
            get(handlers::get_user_badge)
                .post(handlers::grant_user_badge)
                .delete(handlers::revoke_user_badge),
        )
        .route("/assets/{*path}", get(handlers::get_asset))
        .layer(DefaultBodyLimit::max(MAX_BODY_BYTES))
        .layer(tower_http::cors::CorsLayer::permissive())
        .layer(axum::middleware::map_response(preflight_no_content))
        .with_state(state)
}

/// tower-http answers a preflight 200; upstream's 204.
async fn preflight_no_content(mut response: axum::response::Response) -> axum::response::Response {
    let preflight = response
        .headers()
        .contains_key(axum::http::header::ACCESS_CONTROL_ALLOW_METHODS);
    if preflight && response.status() == axum::http::StatusCode::OK {
        *response.status_mut() = axum::http::StatusCode::NO_CONTENT;
    }
    response
}

#[cfg(test)]
mod backfill_tests;
#[cfg(test)]
mod tests;
