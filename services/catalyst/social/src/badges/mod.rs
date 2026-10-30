//! Badges: the read surface of badges.decentraland.org (`/categories`,
//! `/users/{address}/badges`, `/users/{address}/preview`,
//! `/badges/{id}/tiers`), the bearer-gated grant/revoke with its audit row,
//! and the badge art at `/assets/{path}` (embedded, `BADGES_ASSETS_DIR`
//! overriding). Tables: `social.badge_definitions`, `badge_tiers` (reference
//! rows seeded by the schema), `user_badge_progress`, `user_achieved_tiers`,
//! `badge_admin_audit`. No jobs: no event consumer moves `steps_done` yet.
//!
//! Mounted by `serve` as `.nest("/badges", router(state))` (the old gateway
//! stripped that prefix).

pub mod assets;
pub mod handlers;
pub mod store;
pub mod types;

use std::path::PathBuf;
use std::sync::Arc;

use axum::Router;
use axum::routing::{get, post};
use sqlx::PgPool;

use crate::notifications::admin::AdminSecretSource;

pub use store::{BadgesStore, DEFAULT_ASSET_BASE_URL};

pub const ADMIN_TOKEN_ENV: &str = "CATALYRST_BADGES_ADMIN_TOKEN";

#[derive(Clone, Debug)]
pub struct Settings {
    /// `CATALYRST_BADGES_ADMIN_TOKEN`: bearer of grant/revoke; unset refuses them.
    pub admin_token: Option<String>,
    /// `BADGES_PUBLIC_ASSET_BASE_URL`: substituted for the seed rows' host
    /// in every `assets` document at serve time (default: unchanged).
    pub public_asset_base_url: String,
    /// `BADGES_ASSETS_DIR`: files here override the embedded art, one by one.
    pub assets_dir: Option<PathBuf>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            admin_token: None,
            public_asset_base_url: DEFAULT_ASSET_BASE_URL.to_string(),
            assets_dir: None,
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
        .route("/categories", get(handlers::get_categories))
        .route("/users/{address}/preview", get(handlers::get_user_preview))
        .route("/users/{address}/badges", get(handlers::get_user_badges))
        .route("/badges/{badge_id}/tiers", get(handlers::get_badge_tiers))
        .route(
            "/users/{address}/badges/{badge_id}",
            post(handlers::grant_user_badge).delete(handlers::revoke_user_badge),
        )
        .route("/assets/{*path}", get(handlers::get_asset))
        .layer(tower_http::cors::CorsLayer::permissive())
        .with_state(state)
}

#[cfg(test)]
mod tests;
