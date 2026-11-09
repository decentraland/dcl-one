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
//! event is one row of `social.client_events` (`store`), and the explorer's
//! CP7 checkpoint is recorded in the explore onboarding state directly where
//! the notifier posted it to the auth server. The Segment HMAC proves only
//! that Segment delivered the event, and a write key is public: every write
//! keyed on a wallet (a checkpoint) waits for the event's auth chain and is
//! keyed on the chain owner, so `/forward-web` and
//! `/forward-onboarding-checkpoint`, whose bodies merely name a wallet,
//! write no such row. The notifier's `POST /forward-blockchain` (a tip
//! reported by its sender) is not this router's: the shop serves it under
//! the same prefix.
//!
//! The chain proves that its owner once delegated to a key, not that the
//! owner sent this event (nothing in the event is signed by the chain, so a
//! chain replayed in a forged event writes under its owner), and nothing
//! else in the event: `context.ip`,
//! the anonymous id and `properties.email` are the client's own words, so
//! the OTP checkpoint keeps no email of the event's, the onboarding store
//! resolving the wallet to the email the auth server bound to it, and the
//! node runs no multi-accounting detector over them (`event-parser.ts:
//! 110-179`, `client-denylister.ts`): it announces every stored client
//! event on the `social.client_events` channel (the trigger in
//! `schemas/40_social.sql`) and applies the wallet flags notified on the
//! `credits.wallet_flags` channel (`shop/src/credits/wallet_flags.rs`).
//!
//! | Route | Auth | Writes |
//! |---|---|---|
//! | `POST /forward` | Segment HMAC, then the auth chain | `social.client_events`, the CP7 checkpoint |
//! | `POST /forward-web` | Segment HMAC | `social.client_events` |
//! | `POST /forward-onboarding-checkpoint` | Segment HMAC | nothing |
//!
//! Recorded differences from the notifier:
//! - a client event's auth chain is verified (signatures and the ephemeral
//!   expiry, `dcl_auth::verify_auth_chain`) besides the owner check, and only
//!   the explorer's login chain `[SIGNER, ECDSA_EPHEMERAL]` is one: a chain
//!   ending in a signed entity, which content servers publish with every
//!   deployment, is 401; the notifier checked the chain's shape and owner only;
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
//! - the explorer's CP7 checkpoints (`otp_verification_success` reached,
//!   `logged_in`/`logged_in_cached` completed) are recorded after the auth
//!   chain verified, by the chain owner (the wallet is the user identifier)
//!   and without the OTP event's email or metadata (the store resolves the
//!   wallet to the email the auth server bound to it, so a client-chosen
//!   address reaches no nudge, and keeps no row for a wallet without one, so
//!   throwaway wallets evict no user's); the notifier recorded the OTP
//!   checkpoint by the email it carried, before any check, and
//!   `/forward-onboarding-checkpoint` lifted a client-chosen body, which
//!   here answers as the notifier did and records nothing;
//! - the multi-accounting detector (`event-parser.ts:110-179`), the
//!   anonymous-id denylist (`client-denylister.ts`) and the `/admin/anonids`
//!   routes are not served: no event is refused for its anonymous id,
//!   `/forward-web` answers `validationPassed: 1` for every stored identify,
//!   and no cluster is kept: the node announces the stored events and
//!   applies the flags notified to it;
//! - the two delay metrics are histograms named without `_total` (their
//!   `_sum` is the notifier's counter), the Segment-to-webhook delay measured
//!   from the node's clock;
//! - an unset `SEGMENT_SIGNING_KEY` answers 503 (the notifier refused to start).
//!
//! Routes, all `POST`:
//!
//! | Route | Gate | Answers |
//! |---|---|---|
//! | `/forward` | `x-signature` | 200 `{ok}`; 400 `Invalid event`; 401 `Invalid AuthChain` |
//! | `/forward-web` | `x-signature` | 200 `{ok, validationPassed}`; 400 `Invalid web event structure`, `Invalid web event` |
//! | `/forward-onboarding-checkpoint` | `x-signature` | 200 `{ok}`; 400 `Invalid checkpointId` |

mod checkpoint;
mod explorer;
pub mod store;
mod web;

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
    /// `SEGMENT_WEB_PROJECT_ID`: the project whose identify events are stored.
    pub web_project_id: String,
    /// `CLIENT_EVENTS_RETENTION_DAYS`.
    pub retention: Duration,
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

    /// The hourly purge of rows older than the retention: at most a million
    /// rows a pass, so the table keeps up with some 280 inserts a second.
    pub fn spawn_jobs(self: &Arc<Self>, jobs: &Jobs) {
        let state = self.clone();
        jobs.spawn_periodic_leased(
            "client-events-purge",
            PeriodicCfg::every(Duration::from_secs(3600))
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

/// The bound on the identity columns a client string reaches (`key`, the
/// session, anonymous and realm ids, the IP): the row's key and the notify
/// payload, which Postgres caps at 8000 bytes once escaped (a quote or a
/// backslash doubles, a control character is six bytes, so [`identity`]
/// refuses those), so a longer optional value is absent and a longer
/// `messageId` refuses the event.
pub(crate) const MAX_IDENTITY_BYTES: usize = 512;

/// [`text`] within [`MAX_IDENTITY_BYTES`] and free of control characters.
fn identity<'a>(value: &'a Value, key: &str) -> Option<&'a str> {
    text(value, key).filter(|s| s.len() <= MAX_IDENTITY_BYTES && !s.chars().any(char::is_control))
}
