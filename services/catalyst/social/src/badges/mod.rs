//! Badges: the read surface of badges.decentraland.org (`/badges`,
//! `/badges/{id}`, `/badges/{id}/tiers`, `/categories`, `/status`,
//! `/users/{address}/badges`, `/users/{address}/preview`), the bearer-gated
//! admin read, grant and revoke (the last two with an audit row), and badge
//! art mirrored into `BADGES_ASSETS_DIR` at `/assets/{path}`. Tables:
//! `social.badge_definitions`, `badge_tiers` (upstream's catalog, seeded by
//! the schema), `user_badge_progress`, `user_achieved_tiers`,
//! `badge_admin_audit`, `presence_badges`. Every progress
//! write goes through [`progress::apply_progress`]: the console grant, and
//! upstream's observers ([`evaluators`]) as closures of it.
//!
//! Mounted by `serve` as `.nest("/badges", router(state))` (the old gateway
//! stripped that prefix).

pub mod assets;
pub mod evaluators;
pub mod handlers;
pub mod progress;
pub mod store;
pub mod types;

use std::path::PathBuf;
use std::sync::Arc;

use axum::Router;
use axum::routing::get;
use sqlx::PgPool;

use crate::notifications::admin::AdminSecretSource;

pub use progress::{Applied, ApplyOptions, Progress, ProgressChange, apply_progress};
pub use store::{BadgesStore, DEFAULT_ASSET_BASE_URL};

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
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            admin_token: None,
            public_asset_base_url: DEFAULT_ASSET_BASE_URL.to_string(),
            assets_dir: None,
            commit_hash: None,
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
/// old service's was.
pub fn router(state: SharedState) -> Router {
    Router::new()
        .route("/badges", get(handlers::get_badges))
        .route("/badges/{badge_id}", get(handlers::get_badge))
        .route("/badges/{badge_id}/tiers", get(handlers::get_badge_tiers))
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
        .layer(tower_http::cors::CorsLayer::permissive())
        .with_state(state)
}

#[cfg(test)]
mod tests;
