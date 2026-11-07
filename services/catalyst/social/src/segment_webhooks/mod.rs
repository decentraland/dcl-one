//! Segment destination webhooks: the events-notifier's `POST /forward`
//! (explorer events), `POST /forward-web` (web identify events) and
//! `POST /forward-onboarding-checkpoint`, pinned at
//! `github.com/decentraland/events-notifier@1550a6e8`
//! (`src/controllers/handlers/forward-*.ts`, `src/logic/event-parser.ts`).
//! Mounted by `serve` as `.nest("/events-notifier", router(state))` behind
//! the social writes switch. Every route takes the body Segment signs with
//! the shared `SEGMENT_SIGNING_KEY` (`x-signature`,
//! `shared::segment_signature`), answers `{ok, error?}` as the notifier did
//! (401 `Invalid signature`, then the route's own 400s) and writes nothing
//! on refusal. The notifier published each accepted event to SNS; here the
//! event is one row of `social.client_events` (`store`), and a forwarded
//! checkpoint is recorded in the explore onboarding state directly where the
//! notifier posted it to the auth server. `/forward-blockchain` (tip
//! received) is not served yet; it belongs in this router when it is.
//!
//! Recorded differences from the notifier:
//! - a client event's auth chain is verified (signatures and the ephemeral
//!   expiry, `dcl_auth::verify_auth_chain`) besides the owner check; the
//!   notifier checked the chain's shape and owner only;
//! - 400 where the notifier published or crashed: a body that is not JSON,
//!   an event without a string `messageId` or without a parsable RFC 3339
//!   `receivedAt`/`timestamp` (the notifier's `NaN` timestamps), a web
//!   identify without `context` or `context.page` (the notifier's 500);
//! - a web identify without `traits` is answered like one without
//!   `traits.ethAddress`, 200 `validationPassed: 0` (its unit spec's intent;
//!   the source threw), and an identify whose wallet is missing or not an
//!   address stores no row;
//! - the explorer's `sentAt` is not stored; `reported_at` is the event's
//!   `timestamp`;
//! - the IP and anonymous-id multi-accounting clusters and the credits-server
//!   flags are not kept (the detector's step); the denylist refusal is;
//! - the two delay metrics are histograms named without `_total` (their
//!   `_sum` is the notifier's counter), the Segment-to-webhook delay measured
//!   from the node's clock;
//! - an unset `SEGMENT_SIGNING_KEY` answers 503 (the notifier refused to start).

mod checkpoint;
mod explorer;
pub mod store;
mod web;

use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;

use axum::Router;
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use serde_json::{Value, json};
use sqlx::PgPool;

use crate::explore::api::onboarding::OnboardingState;
use crate::jobs::{Jobs, PeriodicCfg};

/// The settings `serve` resolves from `Config` (`SocialConfig`).
#[derive(Clone, Debug, Default)]
pub struct Settings {
    /// `SEGMENT_SIGNING_KEY`; unset, every route answers 503.
    pub signing_key: Option<String>,
    /// `FORCE_EXPLORER_CONTEXT_VALIDATION`: a soft context check refuses the event.
    pub force_context_validation: bool,
    /// `SEGMENT_EXPECTED_REALM` and `SEGMENT_EXPECTED_REALM_URL`: the realm
    /// check of the notifier's production environment; unset, skipped.
    pub expected_realm: Option<String>,
    pub expected_realm_url: Option<String>,
    /// `SEGMENT_ANON_ID_DENYLIST`, lower-cased.
    pub anon_id_denylist: HashSet<String>,
    /// `SEGMENT_WEB_PROJECT_ID`: the project whose identify events are stored.
    pub web_project_id: String,
    /// `CLIENT_EVENTS_RETENTION_DAYS`.
    pub retention: Duration,
}

impl Settings {
    /// A comma-separated list as the lower-cased, trimmed set of its non-empty items.
    pub fn parse_list(list: &str) -> HashSet<String> {
        list.split(',')
            .map(|s| s.trim().to_lowercase())
            .filter(|s| !s.is_empty())
            .collect()
    }
}

pub struct State {
    pub pool: PgPool,
    pub settings: Settings,
    /// The explore onboarding checkpoints a forwarded event records into.
    pub onboarding: Arc<OnboardingState>,
}

pub type SharedState = Arc<State>;

impl State {
    pub fn new(pool: PgPool, settings: Settings, onboarding: Arc<OnboardingState>) -> Self {
        Self {
            pool,
            settings,
            onboarding,
        }
    }

    /// The daily purge of rows older than the retention.
    pub fn spawn_jobs(self: &Arc<Self>, jobs: &Jobs) {
        let state = self.clone();
        jobs.spawn_periodic_leased(
            "client-events-purge",
            PeriodicCfg::every(Duration::from_secs(24 * 3600))
                .with_jitter(Duration::from_secs(60))
                .with_error_backoff(Duration::from_secs(3600)),
            move |ctx| {
                let state = state.clone();
                async move {
                    let retention = chrono::Duration::from_std(state.settings.retention)?;
                    let deleted = store::purge(&state.pool, chrono::Utc::now() - retention).await?;
                    if deleted > 0 {
                        tracing::info!(deleted, "client events purge pass");
                        ctx.progress(format!("deleted {deleted}"));
                    }
                    Ok(())
                }
            },
        );
    }
}

/// The routes, unprefixed: nest under `/events-notifier`. Segment events are
/// small; bodies are capped at 256 KiB.
pub fn router(state: SharedState) -> Router {
    Router::new()
        .route("/forward", post(explorer::forward))
        .route("/forward-web", post(web::forward))
        .route("/forward-onboarding-checkpoint", post(checkpoint::forward))
        .layer(axum::extract::DefaultBodyLimit::max(256 * 1024))
        .with_state(state)
}

/// The notifier's envelope: `{ok: false, error}` under `status`.
fn refused(status: StatusCode, error: &str) -> Response {
    reply(status, json!({ "ok": false, "error": error }))
}

fn reply(status: StatusCode, body: Value) -> Response {
    (
        status,
        [(header::CONTENT_TYPE, "application/json")],
        body.to_string(),
    )
        .into_response()
}

/// The signature check every route runs first: 503 without a key, 401 when
/// `x-signature` is missing or is not the body's HMAC.
fn gate(state: &State, headers: &HeaderMap, body: &[u8]) -> Result<(), Response> {
    let Some(key) = state.settings.signing_key.as_deref() else {
        return Err((
            StatusCode::SERVICE_UNAVAILABLE,
            [
                (header::CONTENT_TYPE, "application/json"),
                (header::RETRY_AFTER, "3600"),
            ],
            json!({
                "ok": false,
                "error": "Service Unavailable",
                "message": "SEGMENT_SIGNING_KEY is not set: this node is not a Segment destination",
            })
            .to_string(),
        )
            .into_response());
    };
    let signature = headers.get("x-signature").and_then(|v| v.to_str().ok());
    if !crate::shared::segment_signature::verify(key.as_bytes(), body, signature) {
        tracing::warn!(
            signature = signature.unwrap_or("empty-signature"),
            "Segment webhook: invalid signature"
        );
        return Err(refused(StatusCode::UNAUTHORIZED, "Invalid signature"));
    }
    Ok(())
}

/// `new Date(value).getTime()` for the RFC 3339 strings Segment sends.
fn parse_time(value: Option<&Value>) -> Option<chrono::DateTime<chrono::Utc>> {
    let text = value?.as_str()?;
    chrono::DateTime::parse_from_rfc3339(text)
        .ok()
        .map(|t| t.with_timezone(&chrono::Utc))
}

/// An object of the pairs whose value is present: what `JSON.stringify`
/// keeps of an object literal with `undefined` members.
fn object(pairs: &[(&str, Option<&Value>)]) -> Value {
    Value::Object(
        pairs
            .iter()
            .filter_map(|(key, value)| value.map(|v| (key.to_string(), v.clone())))
            .collect(),
    )
}

/// A non-empty string member of `value`.
fn text<'a>(value: &'a Value, key: &str) -> Option<&'a str> {
    value
        .get(key)
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
}
