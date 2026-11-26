// from: credits-server@3a7c8ce38:src/logic/usd-purchase-alerts.ts (the cap share, the batch, the lease, the max age), src/migrations/1787000000000_usd-purchase-alerts.ts (one row per kind and credit, sent only once Slack accepted it), src/adapters/db/db.ts (enqueueUsdPurchaseAlerts, getRepeatedlyExpiringBuyers), src/logic/usd-reconciler.ts (the stuck buyers, the recovered intents)
//! Credits alerts: the conditions an operator must hear about, each written
//! as a `credits.alerts` row in the transaction that detects it, and posted
//! to `CREDITS_ALERTS_SLACK_WEBHOOK` by the `credits-alerts` job.
//!
//! - **Settlement** (`settlement.rs`): a credit used on chain after its hold
//!   was released or expired (the double-spend backstop: the debit is
//!   re-applied); a used credit its wallet cannot cover (nothing is debited
//!   and the fold retries it); a credit whose settled consumption reached
//!   [`CAP_NEAR_LIMIT_BPS`] of its signed MANA cap.
//! - **The expiry sweep** ([`expiry_ctes`], run by the fulfilment tick): a
//!   credit the Shop reported submitting a transaction for that expired with
//!   no use indexed (the sweep only runs once the index is past its expiry),
//!   unless another credit of the same wallet went through in that
//!   transaction; a wallet with [`REPEATED_EXPIRY_ATTEMPTS`] expired
//!   purchases of one item within [`REPEATED_EXPIRY_WINDOW_SECS`].
//!
//! A row is unique per (kind, key), so detecting the same case again adds
//! nothing. The job claims due rows for a lease, posts each, and ends it
//! `sent` once Slack accepted it; a refused or failed post is retried with
//! backoff, and a row still undelivered after [`MAX_AGE`] ends `failed`.
//! With no webhook each row is logged at error and ends `skipped`: an alert
//! is never dropped silently.

use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

use alloy_primitives::U256;
use serde_json::{Value, json};
use sqlx::{PgConnection, Row};

use super::errors::ApiError;
use super::store::{Credits, now};
use crate::http::Outbound;
use crate::jobs::{JobContext, Jobs, PeriodicCfg};
use crate::shared::writes::{PassGate, WRITES_DISABLED_MESSAGE};

pub const JOB_NAME: &str = "credits-alerts";

/// A credit used on chain after its hold was released or expired.
pub const RECOVERED_CONSUMPTION: &str = "recovered_consumption";
/// A credit used on chain that its wallet cannot cover.
pub const UNPAID_CONSUMPTION: &str = "unpaid_consumption";
/// A credit whose consumption reached [`CAP_NEAR_LIMIT_BPS`] of its cap.
pub const CAP_NEAR_LIMIT: &str = "cap_near_limit";
/// A submitted credit that expired unconsumed.
pub const SUBMITTED_UNCONSUMED: &str = "submitted_unconsumed";
/// A wallet whose purchases of one item keep expiring.
pub const REPEATED_EXPIRY: &str = "repeated_expiry";

/// The share of its signed MANA cap, in basis points, at or above which a
/// credit's consumption is alerted: signed at the quote plus the oracle's
/// buffer, a credit ordinarily consumes about 98% of it.
pub const CAP_NEAR_LIMIT_BPS: i64 = 9_950;
/// Expired purchases of one item by one wallet that raise the alert...
pub const REPEATED_EXPIRY_ATTEMPTS: i64 = 3;
/// ...when created within this many seconds.
pub const REPEATED_EXPIRY_WINDOW_SECS: i64 = 1_800;

/// Alerts one pass claims at most.
const BATCH: i64 = 20;
/// How long a claim holds its alerts: longer than a batch's posts, each
/// bounded by [`POST_TIMEOUT`], so a slow pass is never overlapped by
/// another posting the same alerts.
const LEASE: chrono::Duration = chrono::Duration::minutes(5);
/// An alert still undelivered this long after it was written is given up.
pub const MAX_AGE: chrono::Duration = chrono::Duration::hours(24);
const POST_TIMEOUT: Duration = Duration::from_secs(10);
const INTERVAL: Duration = Duration::from_secs(15);
/// The first retry's wait, doubled per attempt up to [`MAX_BACKOFF`].
const BACKOFF: chrono::Duration = chrono::Duration::seconds(30);
const MAX_BACKOFF: chrono::Duration = chrono::Duration::minutes(30);

/// Queues an alert in the caller's transaction; nothing when one of its kind
/// and key exists.
pub(crate) async fn record(
    conn: &mut PgConnection,
    kind: &str,
    dedupe_key: &str,
    payload: Value,
) -> sqlx::Result<()> {
    sqlx::query(
        "INSERT INTO credits.alerts (kind, dedupe_key, payload, created_at, next_attempt_at) \
         VALUES ($1, $2, $3, $4, $4) ON CONFLICT (kind, dedupe_key) DO NOTHING",
    )
    .bind(kind)
    .bind(dedupe_key)
    .bind(sqlx::types::Json(payload))
    .bind(now())
    .execute(conn)
    .await?;
    Ok(())
}

/// Queues [`CAP_NEAR_LIMIT`] for `salt` in the caller's transaction when its
/// settled consumption (its settlements so far, this one included) reached
/// [`CAP_NEAR_LIMIT_BPS`] of the MANA it was signed for.
pub(crate) async fn record_cap_near_limit(
    conn: &mut PgConnection,
    salt: &str,
    tx_hash: &str,
) -> sqlx::Result<()> {
    sqlx::query(
        "INSERT INTO credits.alerts (kind, dedupe_key, payload, created_at, next_attempt_at) \
         SELECT $4, a.id, jsonb_build_object('salt', a.id, 'address', a.address, 'usdCents', a.usd_cents, \
                    'capWei', a.amount_wei::text, 'consumedWei', s.consumed::text, 'txHash', $2::text), $3, $3 \
         FROM credits.authorizations a, \
              (SELECT sum(value_wei) AS consumed FROM credits.usage_settlements WHERE authorization_id = $1) s \
         WHERE a.id = $1 AND a.amount_wei > 0 AND s.consumed * 10000 >= a.amount_wei * $5 \
         ON CONFLICT (kind, dedupe_key) DO NOTHING",
    )
    .bind(salt)
    .bind(tx_hash)
    .bind(now())
    .bind(CAP_NEAR_LIMIT)
    .bind(CAP_NEAR_LIMIT_BPS)
    .execute(conn)
    .await?;
    Ok(())
}

/// The expiry sweep and the alerts it detects, as the CTEs of one
/// statement: `expired` flips the past-expiry pledges (`authorized` to
/// `expired`, `retired` to `released`) whose expiry is at or before `bound`
/// and returns them; `submitted_unconsumed` and `repeated_expiry` queue their
/// alerts from those rows. `now` is the clock's bind. Both the fulfilment
/// tick and [`Credits::expire_stale_authorizations`] run it, so a sweep
/// never expires a pledge without its alerts.
///
/// The other CTEs read the table as it was before the flip, so the rows this
/// sweep expires come from `expired` (`fresh`), the earlier ones from the
/// table. A repeated expiry is keyed by its newest fresh attempt: each sweep
/// that adds one past the threshold alerts once.
pub(crate) fn expiry_ctes(now: &str, bound: &str) -> String {
    let window = format!("{now} - interval '{REPEATED_EXPIRY_WINDOW_SECS} seconds'");
    format!(
        "expired AS ( \
             UPDATE credits.authorizations SET status = CASE status WHEN 'retired' THEN 'released' ELSE 'expired' END \
             WHERE status IN ('authorized', 'retired') AND expires_at <= {bound} \
             RETURNING id, address, usd_cents, amount_wei, submitted_tx_hash, contract_address, item_id, created_at \
         ), submitted_unconsumed AS ( \
             INSERT INTO credits.alerts (kind, dedupe_key, payload, created_at, next_attempt_at) \
             SELECT '{SUBMITTED_UNCONSUMED}', e.id, \
                    jsonb_build_object('salt', e.id, 'address', e.address, 'usdCents', e.usd_cents, \
                                       'capWei', e.amount_wei::text, 'txHash', e.submitted_tx_hash), {now}, {now} \
             FROM expired e \
             WHERE e.submitted_tx_hash IS NOT NULL \
               AND NOT EXISTS (SELECT 1 FROM credits.usages u WHERE u.salt = e.id) \
               AND NOT EXISTS (SELECT 1 FROM credits.authorizations s \
                               WHERE s.submitted_tx_hash = e.submitted_tx_hash AND s.address = e.address AND s.id <> e.id \
                                 AND (s.status = 'consumed' OR EXISTS (SELECT 1 FROM credits.usages su WHERE su.salt = s.id))) \
             ON CONFLICT (kind, dedupe_key) DO NOTHING RETURNING 1 \
         ), expired_attempts AS ( \
             SELECT a.id, a.address, a.contract_address, a.item_id, false AS fresh FROM credits.authorizations a \
             WHERE a.status IN ('expired', 'released') AND a.created_at > {window} \
               AND a.address IN (SELECT address FROM expired) \
             UNION ALL \
             SELECT e.id, e.address, e.contract_address, e.item_id, true FROM expired e WHERE e.created_at > {window} \
         ), expired_items AS ( \
             SELECT DISTINCT t.id, t.address, t.fresh, \
                    COALESCE(l.asset_contract || ':' || COALESCE(l.asset_item_id::text, l.asset_token_id::text, lower(l.registered_name)), \
                             t.contract_address || ':' || t.item_id) AS item \
             FROM expired_attempts t LEFT JOIN credits.authorization_lines l ON l.authorization_id = t.id \
         ), repeated_expiry AS ( \
             INSERT INTO credits.alerts (kind, dedupe_key, payload, created_at, next_attempt_at) \
             SELECT '{REPEATED_EXPIRY}', i.address || ':' || i.item || ':' || max(i.id) FILTER (WHERE i.fresh), \
                    jsonb_build_object('address', i.address, 'item', i.item, 'attempts', count(DISTINCT i.id), \
                                       'windowSecs', {REPEATED_EXPIRY_WINDOW_SECS}), {now}, {now} \
             FROM expired_items i WHERE i.item IS NOT NULL \
             GROUP BY i.address, i.item \
             HAVING count(DISTINCT i.id) >= {REPEATED_EXPIRY_ATTEMPTS} AND bool_or(i.fresh) \
             ON CONFLICT (kind, dedupe_key) DO NOTHING RETURNING 1 \
         )"
    )
}

/// Where the alerts go: the Slack incoming webhook through the node's
/// outbound client, or, with none, the error log.
#[derive(Clone)]
pub struct Delivery {
    webhook: Option<Arc<str>>,
    http: Outbound,
}

/// What one pass did with the alerts it claimed.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct PassOutcome {
    pub sent: usize,
    pub retried: usize,
    pub skipped: usize,
    pub failed: usize,
}

impl Delivery {
    pub fn new(webhook: Option<String>, http: Outbound) -> Self {
        Self {
            webhook: webhook.map(Arc::from),
            http,
        }
    }

    /// One pass at `now`: claims up to [`BATCH`] due alerts for [`LEASE`],
    /// then ends each `sent`, `skipped` or `failed`, or schedules its retry.
    pub async fn run_pass(
        &self,
        credits: &Credits,
        now: chrono::NaiveDateTime,
    ) -> Result<PassOutcome, ApiError> {
        let mut claimed = sqlx::query(
            "UPDATE credits.alerts a SET attempts = a.attempts + 1, next_attempt_at = $2 \
             WHERE a.id IN (SELECT id FROM credits.alerts WHERE status = 'pending' AND next_attempt_at <= $1 \
                            ORDER BY next_attempt_at, id LIMIT $3 FOR UPDATE SKIP LOCKED) \
             RETURNING a.id, a.kind, a.payload, a.attempts, a.created_at",
        )
        .bind(now)
        .bind(now + LEASE)
        .bind(BATCH)
        .fetch_all(&credits.pool)
        .await?;
        claimed.sort_by_key(|row| row.get::<i64, _>("id"));
        let mut outcome = PassOutcome::default();
        for row in claimed {
            let id: i64 = row.get("id");
            let kind: String = row.get("kind");
            let attempts: i32 = row.get("attempts");
            let created_at: chrono::NaiveDateTime = row.get("created_at");
            let payload: sqlx::types::Json<Value> = row.get("payload");
            let text = message(&kind, &payload);
            let Some(webhook) = self.webhook.as_deref() else {
                tracing::error!(alert = %text, kind, "CREDITS_ALERTS_SLACK_WEBHOOK is unset: a credits alert is logged here and not sent");
                finish(credits, id, "skipped", now).await?;
                outcome.skipped += 1;
                continue;
            };
            if created_at <= now - MAX_AGE {
                tracing::error!(alert = %text, kind, attempts, "a credits alert went undelivered for a day and is given up");
                finish(credits, id, "failed", now).await?;
                outcome.failed += 1;
                continue;
            }
            match self.post(webhook, &text).await {
                Ok(()) => {
                    finish(credits, id, "sent", now).await?;
                    outcome.sent += 1;
                }
                Err(error) => {
                    tracing::warn!(kind, attempts, %error, "a credits alert was not delivered; retried with backoff");
                    sqlx::query(
                        "UPDATE credits.alerts SET next_attempt_at = $2, last_error = $3 \
                         WHERE id = $1 AND status = 'pending'",
                    )
                    .bind(id)
                    .bind(now + backoff(attempts))
                    .bind(&error)
                    .execute(&credits.pool)
                    .await?;
                    outcome.retried += 1;
                }
            }
        }
        Ok(outcome)
    }

    /// Posts `text`; `Err` with a reason that never names the webhook (its
    /// path is the credential).
    async fn post(&self, webhook: &str, text: &str) -> Result<(), String> {
        let response = self
            .http
            .post(webhook)
            .json(&json!({ "text": text }))
            .timeout(POST_TIMEOUT)
            .send()
            .await
            .map_err(|error| {
                if error.is_timeout() {
                    "the post timed out".to_string()
                } else if error.is_connect() {
                    "the webhook did not accept a connection".to_string()
                } else {
                    "the post failed".to_string()
                }
            })?;
        let status = response.status();
        if status.is_success() {
            Ok(())
        } else {
            Err(format!("Slack answered {status}"))
        }
    }
}

/// The retry wait after `attempts` failed posts.
fn backoff(attempts: i32) -> chrono::Duration {
    let doublings = attempts.saturating_sub(1).clamp(0, 16) as u32;
    BACKOFF
        .checked_mul(1 << doublings)
        .map_or(MAX_BACKOFF, |wait| wait.min(MAX_BACKOFF))
}

async fn finish(
    credits: &Credits,
    id: i64,
    status: &str,
    now: chrono::NaiveDateTime,
) -> Result<(), ApiError> {
    sqlx::query(
        "UPDATE credits.alerts SET status = $2, finished_at = $3 WHERE id = $1 AND status = 'pending'",
    )
    .bind(id)
    .bind(status)
    .bind(now)
    .execute(&credits.pool)
    .await?;
    Ok(())
}

/// The Slack line of one alert.
pub(crate) fn message(kind: &str, payload: &Value) -> String {
    let text = |key: &str| payload[key].as_str().unwrap_or("unknown").to_string();
    let usd = |key: &str| {
        payload[key]
            .as_i64()
            .map_or("an unknown amount".into(), |cents| {
                format!("${}.{:02}", cents / 100, cents % 100)
            })
    };
    let mana = |key: &str| {
        payload[key]
            .as_str()
            .and_then(|wei| U256::from_str(wei).ok())
            .map_or("an unknown amount of".into(), |wei| {
                crate::types::decimal_text(wei, 18)
            })
    };
    let (salt, address, tx) = (text("salt"), text("address"), text("txHash"));
    match kind {
        RECOVERED_CONSUMPTION => format!(
            ":rotating_light: Credits: credit {salt} was used on chain after its hold was {} (the double-spend backstop). Wallet {address} was debited {} credits for {}; tx {tx}.",
            text("status"),
            text("debitedCredits"),
            usd("usdCents"),
        ),
        UNPAID_CONSUMPTION => format!(
            ":rotating_light: Credits: credit {salt} was used on chain, but wallet {address} cannot cover the {} credits it owes; nothing was debited and each settlement pass retries it. Tx {tx}.",
            text("debitedCredits"),
        ),
        CAP_NEAR_LIMIT => format!(
            ":warning: Credits: credit {salt} consumed {} of the {} MANA it was signed for (at least {}.{}%). Wallet {address}, {}; tx {tx}.",
            mana("consumedWei"),
            mana("capWei"),
            CAP_NEAR_LIMIT_BPS / 100,
            CAP_NEAR_LIMIT_BPS % 100 / 10,
            usd("usdCents"),
        ),
        SUBMITTED_UNCONSUMED => format!(
            ":warning: Credits: credit {salt} expired with no use indexed after the Shop reported submitting tx {tx}; its hold was released. Wallet {address}, {}.",
            usd("usdCents"),
        ),
        REPEATED_EXPIRY => format!(
            ":warning: Credits: wallet {address} let {} purchases of {} expire unused within {} minutes; its checkout may be failing.",
            payload["attempts"].as_i64().unwrap_or(0),
            text("item"),
            payload["windowSecs"].as_i64().unwrap_or(0) / 60,
        ),
        other => format!("Credits alert {other}: {payload}"),
    }
}

/// The `credits-alerts` job: a pass every [`INTERVAL`]. Delivery ends rows,
/// which only the node that writes the ledger does, so it stands down while
/// `OPS_WRITES_ENABLED` is off.
pub fn spawn(jobs: &Jobs, credits: Credits, delivery: Delivery, writes_enabled: bool) {
    if !writes_enabled {
        tracing::info!("OPS_WRITES_ENABLED=false: the credits alerts job is not running");
        return;
    }
    if delivery.webhook.is_none() {
        tracing::warn!(
            "CREDITS_ALERTS_SLACK_WEBHOOK unset: each credits alert is logged at error and not sent"
        );
    }
    let gate = Arc::new(PassGate::new(JOB_NAME));
    jobs.spawn_periodic_leased(
        JOB_NAME,
        PeriodicCfg::every(INTERVAL)
            .with_jitter(Duration::from_secs(5))
            .with_error_backoff(Duration::from_secs(300)),
        move |ctx: JobContext| {
            let credits = credits.clone();
            let delivery = delivery.clone();
            let gate = gate.clone();
            async move {
                if !gate.allows(writes_enabled) {
                    ctx.progress(WRITES_DISABLED_MESSAGE);
                    return Ok(());
                }
                let outcome = delivery
                    .run_pass(&credits, now())
                    .await
                    .map_err(|e| anyhow::anyhow!("{e}"))?;
                if outcome != PassOutcome::default() {
                    ctx.progress(format!("{outcome:?}"));
                }
                Ok(())
            }
        },
    );
}

#[cfg(test)]
mod unit_tests {
    use super::*;

    #[test]
    fn the_retry_wait_doubles_up_to_half_an_hour() {
        assert_eq!(backoff(1), chrono::Duration::seconds(30));
        assert_eq!(backoff(2), chrono::Duration::seconds(60));
        assert_eq!(backoff(6), chrono::Duration::seconds(960));
        assert_eq!(backoff(7), MAX_BACKOFF);
        assert_eq!(backoff(i32::MAX), MAX_BACKOFF);
    }

    #[test]
    fn a_batch_of_posts_fits_in_the_lease() {
        let posts = POST_TIMEOUT * BATCH as u32;
        assert!(posts < LEASE.to_std().unwrap(), "{posts:?}");
    }

    #[test]
    fn each_alert_reads_as_one_line() {
        let cap = message(
            CAP_NEAR_LIMIT,
            &json!({ "salt": "0x01", "address": "0x02", "usdCents": 1234, "capWei": "10000000000000000000", "consumedWei": "9960000000000000000", "txHash": "0x03" }),
        );
        assert_eq!(
            cap,
            ":warning: Credits: credit 0x01 consumed 9.96 of the 10 MANA it was signed for (at least 99.5%). Wallet 0x02, $12.34; tx 0x03."
        );
        let repeated = message(
            REPEATED_EXPIRY,
            &json!({ "address": "0x02", "item": "0xc:7", "attempts": 3, "windowSecs": 1800 }),
        );
        assert_eq!(
            repeated,
            ":warning: Credits: wallet 0x02 let 3 purchases of 0xc:7 expire unused within 30 minutes; its checkout may be failing."
        );
        for kind in [
            RECOVERED_CONSUMPTION,
            UNPAID_CONSUMPTION,
            SUBMITTED_UNCONSUMED,
        ] {
            let line = message(kind, &json!({}));
            assert!(!line.contains('\n') && line.starts_with(':'), "{line}");
        }
    }
}
