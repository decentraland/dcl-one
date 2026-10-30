//! Notifications: the signed-fetch reader/marker surface of
//! notifications.decentraland.org (`GET /notifications`, `PUT
//! /notifications/read`, the subscription, email confirmation and opt-out
//! routes), the bearer-gated broadcast, the `email` outbox worker
//! (`delivery`), the `friend_first_wear` producer (`first_wear`) and the
//! `land_transfer_received` producer over the land_authz index
//! (`land_transfers`), all over the ONE pool and the ONE outbound client.
//! Tables: `social.notifications`, `notification_deliveries`,
//! `subscriptions`, `unconfirmed_emails`, `subscription_opt_outs`,
//! `notification_reader_seen`, `worn_history`, `first_wear_baseline`,
//! `first_wear_cursor`, `land_transfer_cursor`.
//!
//! Mounted by `serve` as `.nest("/notifications", router(state))` (the old
//! gateway stripped that prefix in front of the bundle); a per-vhost proxy
//! forwards `notifications.<domain>/<path>` to `/notifications/<path>` with
//! `x-original-path` so the signature still verifies.

pub mod admin;
pub mod delivery;
pub mod email;
pub mod errors;
pub mod events;
pub mod first_wear;
pub mod handlers;
pub mod land_transfers;
pub(crate) mod moderation;
pub mod seen;
pub mod store;
pub mod types;

use std::sync::Arc;
use std::time::Duration;

use axum::Router;
use axum::routing::{get, post, put};
use sqlx::PgPool;

use crate::auth::eip1654::Eip1654Validator;
use crate::chain_index::ChainIndex;
use crate::http::Outbound;
use crate::jobs::{Jobs, PeriodicCfg};

pub use email::EmailConfig;
pub use errors::ApiError;
pub use first_wear::FirstWearSettings;
pub use store::{NewNotification, NotificationsStore, insert_notifications};

pub const ADMIN_TOKEN_ENV: &str = "CATALYRST_NOTIFICATIONS_ADMIN_TOKEN";

/// The settings `serve` resolves from `Config`; env names are the old
/// binary's where they map 1:1 (`main.rs` of catalyrst-notifications).
#[derive(Clone, Debug)]
pub struct Settings {
    /// `CATALYRST_NOTIFICATIONS_ADMIN_TOKEN`: bearer of `POST /notifications/broadcast`; unset refuses it.
    pub admin_token: Option<String>,
    /// `SENDGRID_*`, `ACCOUNT_BASE_URL`, `MARKETPLACE_BASE_URL`, `TURNSTILE_SECRET_KEY`, `EMAIL_DOMAIN_BLACKLIST`.
    pub email: EmailConfig,
    /// `FIRST_WEAR_ENABLED`: run the friend_first_wear producer (the old
    /// binary ran it when its three extra database DSNs were set).
    pub first_wear_enabled: bool,
    /// `SHOP_ITEM_BASE_URL`, `FIRST_WEAR_IMAGE_BASE`.
    pub first_wear: FirstWearSettings,
    /// `NOTIFICATION_DELIVERY_INTERVAL_SECS` (default 15; floored to 1).
    pub delivery_interval: Duration,
    /// Deployment origin used by event notice links (`/whats-on/{id}`).
    pub events_base_url: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            admin_token: None,
            email: EmailConfig {
                account_base_url: "https://account.decentraland.org".to_string(),
                marketplace_base_url: "https://decentraland.org/marketplace".to_string(),
                ..Default::default()
            },
            first_wear_enabled: false,
            first_wear: FirstWearSettings::default(),
            delivery_interval: Duration::from_secs(15),
            events_base_url: "https://decentraland.org".into(),
        }
    }
}

pub struct State {
    pub store: NotificationsStore,
    pub email: email::EmailSender,
    pub settings: Settings,
    pub http: Outbound,
    /// Contract-wallet (EIP-1654) signature check for signed-fetch chains;
    /// `None` refuses such chains (the old binary had no validator either).
    pub eip1654: Option<Arc<dyn Eip1654Validator>>,
    /// The collections subgraph reader the first-wear producer resolves
    /// items through; `None` means no first wear is ever notifiable.
    pub market: Option<Arc<ChainIndex>>,
}

pub type SharedState = Arc<State>;

impl State {
    pub fn new(
        pool: PgPool,
        settings: Settings,
        http: Outbound,
        eip1654: Option<Arc<dyn Eip1654Validator>>,
        market: Option<Arc<ChainIndex>>,
    ) -> Self {
        Self {
            store: NotificationsStore::new(pool),
            email: email::EmailSender::new(settings.email.clone(), http.clone()),
            settings,
            http,
            eip1654,
            market,
        }
    }

    /// The outbox worker always; the first-wear producer when enabled.
    pub fn spawn_jobs(self: &Arc<Self>, jobs: &Jobs) {
        let state = self.clone();
        let configured = self.email.notifications_configured();
        if !configured {
            tracing::info!(
                "SENDGRID_NOTIFICATION_TEMPLATE_ID unset: email deliveries are recorded as skipped"
            );
        }
        jobs.spawn_periodic_leased(
            "notification-deliveries",
            PeriodicCfg::every(self.settings.delivery_interval.max(Duration::from_secs(1)))
                .with_jitter(Duration::from_secs(5))
                .with_error_backoff(Duration::from_secs(300)),
            move |ctx| {
                let state = state.clone();
                async move {
                    let summary =
                        delivery::run_pass(state.store.pool(), &state.email, configured).await?;
                    if summary.claimed > 0 {
                        tracing::info!(?summary, "notification deliveries pass");
                        ctx.progress(format!("{summary:?}"));
                    }
                    Ok(())
                }
            },
        );
        land_transfers::spawn(self.store.pool().clone(), jobs);
        events::spawn(
            self.store.pool().clone(),
            self.settings.events_base_url.clone(),
            jobs,
        );
        if self.settings.first_wear_enabled {
            first_wear::spawn(self, jobs);
            tracing::info!(
                market = self.market.is_some(),
                "friend_first_wear producer up"
            );
        } else {
            tracing::info!("FIRST_WEAR_ENABLED=false: no friend_first_wear producer");
        }
    }
}

impl admin::AdminSecretSource for SharedState {
    const ENVIRONMENT_VARIABLE: &'static str = ADMIN_TOKEN_ENV;
    const UNSET_MESSAGE: &'static str =
        "admin broadcast disabled: CATALYRST_NOTIFICATIONS_ADMIN_TOKEN is not set";
    const MISMATCH_MESSAGE: &'static str = "You are not authorized to access this resource";
    fn admin_token(&self) -> Option<&str> {
        self.settings.admin_token.as_deref()
    }
}

/// The routes, unprefixed: nest under `/notifications`. Bodies are capped
/// at 256 KiB as before; CORS is permissive as the old bundle's was.
pub fn router(state: SharedState) -> Router {
    Router::new()
        .route("/notifications", get(handlers::get_notifications))
        .route("/notifications/read", put(handlers::put_read))
        .route(
            "/subscription",
            get(handlers::get_subscription).put(handlers::put_subscription),
        )
        .route("/set-email", put(handlers::put_set_email))
        .route("/confirm-email", put(handlers::put_confirm_email))
        .route("/subscription/opt-outs", post(handlers::post_opt_out))
        .route(
            "/subscription/opt-outs/community/{communityId}",
            get(handlers::get_community_opt_out).delete(handlers::delete_community_opt_out),
        )
        .route("/notifications/broadcast", post(handlers::post_broadcast))
        .layer(axum::extract::DefaultBodyLimit::max(256 * 1024))
        .layer(tower_http::cors::CorsLayer::permissive())
        .with_state(state)
}

#[cfg(test)]
mod tests;
