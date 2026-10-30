//! The operator plane's console API (the old `catalyrst-live` admin surface,
//! stage 5's operator features): SIWE sign-in
//! over `ADMIN_ADDRESSES` + `SESSION_SECRET` into an HMAC cookie
//! (`auth`, `session`), the sync controls the stage-2 README listed as "not
//! copied, needed later" (`/admin/api/content/sync/{pause,resume,force}` on
//! the orchestrator's [`SyncControl`]), the deployments cache flush, the
//! failed-deployments admin (re-arm one for the retry worker, purge one or
//! all), the reports (`/admin/api/{jobs,cache,sync,mirrors}`: the job
//! registry, the in-process caches, the sync state with its per-peer cursors
//! and failed-deployment counts, every mirror's pass row) and the audit log
//! (`/admin/api/audit` over `ops.admin_audit`, the ONE audit relation every
//! operator mutation in the tree writes through [`audit::record`]), and
//! `/admin/api/content/snapshots/regenerate` (`snapshots`, its own router
//! over the snapshot generator, merged beside this one).
//!
//! `/admin/api/<family>/*` ([`console`]): the old console's proxies into
//! every other service's admin routes (telemetry, credits, price, social
//! moderation, the social RPC, communities, notifications, badges, worlds,
//! the scene host, the camera reel, builder curation, the EVM relay, the
//! explorer overlays, place/world/event moderation) as calls into the owning
//! modules in this process, each behind the write switch its domain uses and
//! audited here; the asset-bundle registry's controls go to the external
//! abgen runtime over loopback HTTP, as they did. `GET /admin` and
//! `/admin/{family}` ([`pages`]) render the old console page over them.
//!
//! Authorization is the old server's, unchanged: every `/admin/api/*` route
//! takes [`auth::AdminSession`] (same-origin check, `cat_admin` cookie whose
//! HMAC, expiry and allowlist membership are re-verified per request); no
//! cookie, no route. Without `SESSION_SECRET` or an empty `ADMIN_ADDRESSES`
//! the console is disabled: sign-in answers 403 and nothing can mint a
//! cookie.
//!
//! Writes: `OPS_WRITES_ENABLED` (default false, `ops::OPS_WRITES_ENABLED`)
//! gates the mutating routes that touch durable state or the node's
//! caches (`flush-cache`, `failed-deployments/{retry,clear}`) with the 503
//! envelope, checked after the session so an unauthenticated caller still
//! sees the old 403. The sync controls stay open while writes are disabled:
//! pausing a node in catch-up is exactly what an operator of a read-only
//! node needs, and they change no durable state (a restart resumes). So
//! does `snapshots/regenerate`: it moves the next pass of a job that runs
//! regardless, and the pass writes only data derived from the deployments.
//! Every
//! mutation, open or gated, records an `ops.admin_audit` row: the SQL ones
//! in the same transaction as their write, the sync/cache ones after the
//! effect (`audit::record_after`).
//!
//! The content server's own operations (the durable denylist over
//! `content.denylist`, `challenge/refresh`, the `read-only` and
//! `accepting-users` toggles) are [`content`], its own router merged beside
//! this one (`content::ROUTES` has its own 403 gating test).

pub mod api;
pub mod audit;
pub mod auth;
pub mod console;
pub mod content;
mod create;
mod economy;
mod explore;
pub mod pages;
mod rpc;
pub mod session;
pub mod snapshots;
mod social;
mod telemetry;
mod worlds;

#[cfg(test)]
mod console_tests;
#[cfg(test)]
mod tests;

use std::collections::HashSet;
use std::sync::Arc;

use axum::Router;
use axum::routing::{get, post};
use sqlx::PgPool;

use crate::content::ContentState;
use crate::content::sync::orchestrator::SyncControl;
use crate::jobs::Jobs;

/// The environment as `config.rs` carries it; [`Settings::parse`] normalises.
#[derive(Debug, Clone, Default)]
pub struct RawSettings {
    /// `SESSION_SECRET` (or `SESSION_SECRET_FILE`): the cookie and nonce HMAC key.
    pub session_secret: Option<String>,
    /// `ADMIN_ADDRESSES`: comma-separated wallets allowed to sign in.
    pub admin_addresses: String,
    /// `ADMIN_SESSION_TTL_SECS`; non-positive falls back to 12 hours.
    pub session_ttl_secs: i64,
    /// `ADMIN_COOKIE_INSECURE`: drop the cookie's `Secure` flag (plain-http labs only).
    pub cookie_insecure: bool,
    /// `ETH_NETWORK`: the SIWE chain id.
    pub eth_network: String,
    /// `OPS_WRITES_ENABLED`.
    pub writes_enabled: bool,
}

#[derive(Debug, Clone)]
pub struct Settings {
    pub session_secret: Option<String>,
    pub admin_addresses: HashSet<String>,
    pub session_ttl_secs: i64,
    pub cookie_insecure: bool,
    pub eth_network: String,
    pub writes_enabled: bool,
}

impl Settings {
    pub fn parse(raw: RawSettings) -> Self {
        let admin_addresses = raw
            .admin_addresses
            .split(',')
            .map(|a| a.trim().to_lowercase())
            .filter(|a| !a.is_empty())
            .collect();
        Self {
            session_secret: raw
                .session_secret
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty()),
            admin_addresses,
            session_ttl_secs: if raw.session_ttl_secs > 0 {
                raw.session_ttl_secs
            } else {
                session::DEFAULT_TTL_SECS
            },
            cookie_insecure: raw.cookie_insecure,
            eth_network: raw.eth_network,
            writes_enabled: raw.writes_enabled,
        }
    }

    /// A secret and at least one allowed wallet; otherwise the console is off.
    pub fn admin_enabled(&self) -> bool {
        self.session_secret.is_some() && !self.admin_addresses.is_empty()
    }

    pub fn is_allowed(&self, lowercased: &str) -> bool {
        self.admin_addresses.contains(lowercased)
    }
}

pub struct State {
    pub pool: PgPool,
    pub settings: Settings,
    /// The sync status, the deployments cache and the denylist the reports read.
    pub content: Arc<ContentState>,
    pub jobs: Arc<Jobs>,
    /// `None` on a node without the sync worker: the controls answer 501, as
    /// the old default backend did.
    pub sync_control: Option<SyncControl>,
    /// The modules `/admin/api/<family>/*` calls into ([`console`]).
    pub domains: console::Domains,
}

pub type SharedState = Arc<State>;

impl State {
    pub fn new(
        pool: PgPool,
        settings: Settings,
        content: Arc<ContentState>,
        jobs: Arc<Jobs>,
        sync_control: Option<SyncControl>,
    ) -> Self {
        Self {
            pool,
            settings,
            content,
            jobs,
            sync_control,
            domains: console::Domains::default(),
        }
    }

    /// The module states the console calls into (`serve` clones them in
    /// before mounting their routers).
    pub fn with_domains(mut self, domains: console::Domains) -> Self {
        self.domains = domains;
        self
    }
}

/// The console API at its old paths. Mounted at the root (the old server
/// answered `/admin/*` there); no other v2 router owns `/admin/auth` or
/// `/admin/api/content`, `/admin/api/{jobs,cache,sync,mirrors,audit}`.
pub fn router(state: SharedState) -> Router {
    Router::new()
        .route("/admin/auth/nonce", get(auth::nonce))
        .route("/admin/auth/verify", post(auth::verify))
        .route("/admin/auth/logout", post(auth::logout))
        .route("/admin/auth/me", get(auth::me))
        .route("/admin/api/content/sync/pause", post(api::sync_pause))
        .route("/admin/api/content/sync/resume", post(api::sync_resume))
        .route("/admin/api/content/sync/force", post(api::sync_force))
        .route("/admin/api/content/flush-cache", post(api::flush_cache))
        .route(
            "/admin/api/content/failed-deployments/retry",
            post(api::failed_retry),
        )
        .route(
            "/admin/api/content/failed-deployments/clear",
            post(api::failed_clear),
        )
        .route("/admin/api/jobs", get(api::jobs))
        .route("/admin/api/cache", get(api::cache))
        .route("/admin/api/sync", get(api::sync))
        .route("/admin/api/mirrors", get(api::mirrors))
        .route("/admin/api/audit", get(api::audit_list))
        .merge(console::routes())
        .merge(pages::routes())
        .with_state(state)
}

/// Every route above, for the gating tests (each `/admin/api/*` one answers
/// 403 without a cookie, the old `admin_gating_tests` contract).
pub const ADMIN_API_ROUTES: &[(&str, &str)] = &[
    ("POST", "/admin/api/content/sync/pause"),
    ("POST", "/admin/api/content/sync/resume"),
    ("POST", "/admin/api/content/sync/force"),
    ("POST", "/admin/api/content/flush-cache"),
    ("POST", "/admin/api/content/failed-deployments/retry"),
    ("POST", "/admin/api/content/failed-deployments/clear"),
    ("GET", "/admin/api/jobs"),
    ("GET", "/admin/api/cache"),
    ("GET", "/admin/api/sync"),
    ("GET", "/admin/api/mirrors"),
    ("GET", "/admin/api/audit"),
];

/// The mutating routes that stay open while `OPS_WRITES_ENABLED` is false.
pub const OPEN_WHILE_WRITES_DISABLED: &[&str] = &[
    "/admin/api/content/sync/pause",
    "/admin/api/content/sync/resume",
    "/admin/api/content/sync/force",
];
