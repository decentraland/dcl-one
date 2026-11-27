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
//!   no use indexed, unless another credit of the same wallet went through in
//!   that transaction. Only where the CreditUsed index bounds the sweep (it
//!   expires a pledge once the index read past it): without one the sweep runs
//!   on the clock alone and no use is ruled out, so none is queued. And a
//!   wallet with [`REPEATED_EXPIRY_ATTEMPTS`] expired purchases of one item
//!   created within [`REPEATED_EXPIRY_WINDOW_SECS`] of the newest one this
//!   sweep expired.
//!
//! - **The purchase feed**, credits-server's Shop channel, each line at the
//!   moment the money moved: a pack paid and credited and a refund or
//!   chargeback clawing it back, or matching no paid purchase yet (the Stripe
//!   webhook's transaction); an item purchase settled on chain (the
//!   settlement's transaction).
//! - **The daily summary** (`daily_summary.rs`): one row per closed UTC day,
//!   queued by its own job the morning after.
//!
//! A row is unique per (kind, key), so detecting the same case again adds
//! nothing. The job claims due rows for a lease, posts each (no redirect
//! followed), and ends it `sent` once Slack accepted it; a refused or failed
//! post is retried with backoff, Slack's `Retry-After` on a 429 holding the
//! rest of the pass too. Held rows keep their previous attempt count. A row
//! still undelivered after [`MAX_AGE`] and one attempted post ends `failed`.
//! After [`MAX_HOLD`] since the first unposted Slack hold, another hold ends
//! the row `failed` with `last_error` set to "Slack hold age limit reached".
//! That row can have zero attempts. Time in the queue before the first hold
//! does not count, so a worker outage followed by one 429 preserves the backlog.
//! With no webhook each row is logged at error and ends `skipped`. Delivery
//! is at least once: a post Slack accepted whose row was not ended (a crash,
//! a timeout after acceptance) is posted again after the lease.

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
/// Feed: a credit pack paid and credited.
pub const PACK_CREDITED: &str = "pack_credited";
/// Feed: a refund or chargeback clawed a pack's credits back.
pub const PACK_REVERSED: &str = "pack_reversed";
/// Feed: a refund or chargeback that matched no paid purchase yet.
pub const REVERSAL_UNMATCHED: &str = "reversal_unmatched";
/// Feed: an item purchase settled on chain.
pub const PURCHASE_SETTLED: &str = "purchase_settled";

/// Stripe's published card rate (2.9% plus 30 cents), the estimate of what a
/// pack payment cost: the real fee is on Stripe's balance transaction, which
/// the webhook does not read.
const STRIPE_FEE_PER_MILLE: i64 = 29;
const STRIPE_FEE_FIXED_CENTS: i64 = 30;
/// Settled purchases one message lists: the purchases of a pass share
/// messages, as credits-server posts one per tick.
const SETTLED_PER_MESSAGE: usize = 10;
/// Items a settled line names before counting the rest.
const ITEMS_PER_LINE: usize = 3;

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
/// The maximum elapsed time between the first and a subsequent Slack hold.
const MAX_HOLD: chrono::Duration = chrono::Duration::hours(48);
/// A held retry stores its start in the reason carried forward by the next claim.
const HOLD_SINCE_PREFIX: &str = "held while Slack rate limits since ";
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

/// Queues [`PURCHASE_SETTLED`] for `salt` in the settlement's transaction:
/// what the credit bought (its lines), what it charged, and the MANA its
/// first use settled on `chain_id` in `tx_hash`.
pub(crate) async fn record_purchase_settled(
    conn: &mut PgConnection,
    salt: &str,
    chain_id: i64,
    tx_hash: &str,
    mana_wei: &str,
    recovered: bool,
) -> sqlx::Result<()> {
    sqlx::query(
        "INSERT INTO credits.alerts (kind, dedupe_key, payload, created_at, next_attempt_at) \
         SELECT $1, a.id, jsonb_build_object('salt', a.id, 'address', a.address, 'usdCents', a.usd_cents, \
                    'manaWei', $4::text, 'txHash', $3::text, 'chainId', $5::bigint, 'recovered', $6::boolean, \
                    'source', a.source, 'lines', GREATEST(l.lines, 1), 'registeredName', l.registered_name, \
                    'items', CASE WHEN l.lines = 0 AND a.contract_address IS NOT NULL AND a.item_id IS NOT NULL \
                                  THEN jsonb_build_array(jsonb_build_object('contractAddress', a.contract_address, 'itemId', a.item_id)) \
                                  ELSE l.items END), $7, $7 \
         FROM credits.authorizations a, LATERAL ( \
             SELECT count(*) AS lines, max(registered_name) AS registered_name, \
                    COALESCE(jsonb_agg(jsonb_build_object('contractAddress', asset_contract, 'itemId', asset_item_id::text) \
                                       ORDER BY position) FILTER (WHERE sale_kind = 'primary_mint' AND asset_item_id IS NOT NULL), \
                             '[]'::jsonb) AS items \
             FROM credits.authorization_lines WHERE authorization_id = a.id) l \
         WHERE a.id = $2 \
         ON CONFLICT (kind, dedupe_key) DO NOTHING",
    )
    .bind(PURCHASE_SETTLED)
    .bind(salt)
    .bind(tx_hash)
    .bind(mana_wei)
    .bind(chain_id)
    .bind(recovered)
    .bind(now())
    .execute(conn)
    .await?;
    Ok(())
}

/// Queues [`PACK_CREDITED`] in the Stripe webhook's transaction, once per
/// purchase, with the surface that opened its checkout.
pub(crate) async fn record_pack_credited(
    conn: &mut PgConnection,
    purchase: &super::packs::SettledPurchase,
    event_id: &str,
) -> sqlx::Result<()> {
    let key = purchase
        .order_id
        .as_deref()
        .or(purchase.checkout_session.as_deref())
        .or(purchase.payment_intent.as_deref())
        .unwrap_or(event_id);
    let source: Option<String> = sqlx::query_scalar(
        "SELECT source FROM credits.purchases \
         WHERE order_id = $1 OR stripe_checkout_session = $2 OR stripe_payment_intent = $3 LIMIT 1",
    )
    .bind(purchase.order_id.as_deref())
    .bind(purchase.checkout_session.as_deref())
    .bind(purchase.payment_intent.as_deref())
    .fetch_optional(&mut *conn)
    .await?
    .flatten();
    record(
        conn,
        PACK_CREDITED,
        &format!("pack:{key}"),
        json!({
            "address": purchase.address, "credits": purchase.credits, "chargedCents": purchase.amount_cents,
            "packId": purchase.sku, "quantity": purchase.quantity, "source": source,
        }),
    )
    .await
}

/// Queues the feed line of a reversal in the Stripe webhook's transaction,
/// once per event: [`PACK_REVERSED`] when it clawed credits back,
/// [`REVERSAL_UNMATCHED`] when it matched no paid purchase.
pub(crate) async fn record_reversal(
    conn: &mut PgConnection,
    outcome: &super::packs::ReversalOutcome,
    event_id: &str,
    payment_intent: &str,
    reason: &str,
) -> sqlx::Result<()> {
    use super::packs::ReversalOutcome;
    let key = format!("stripe:{event_id}:{payment_intent}");
    match outcome {
        ReversalOutcome::Reversed {
            address,
            charged_back,
            shortfall,
            has_shortfall,
            ..
        } => {
            let pack: Option<String> = sqlx::query_scalar(
                "SELECT sku FROM credits.purchases WHERE stripe_payment_intent = $1 LIMIT 1",
            )
            .bind(payment_intent)
            .fetch_optional(&mut *conn)
            .await?;
            record(
                conn,
                PACK_REVERSED,
                &key,
                json!({
                    "address": address, "chargedBack": charged_back, "shortfall": shortfall,
                    "hasShortfall": has_shortfall, "packId": pack, "paymentIntent": payment_intent,
                    "reason": reason,
                }),
            )
            .await
        }
        ReversalOutcome::NoPaidPurchase => {
            record(
                conn,
                REVERSAL_UNMATCHED,
                &key,
                json!({ "paymentIntent": payment_intent, "reason": reason }),
            )
            .await
        }
        ReversalOutcome::NothingToReverse => Ok(()),
    }
}

/// The expiry sweep and the alerts it detects, as the CTEs of one
/// statement: `expired` flips the past-expiry pledges (`authorized` to
/// `expired`, `retired` to `released`) whose expiry the CreditUsed index of
/// chain `chain` read past (`usage::expiry_bound_sql`) and returns them;
/// `submitted_unconsumed` and `repeated_expiry` queue their alerts from those
/// rows. `now` is the clock's bind. Both the fulfilment tick and
/// [`Credits::expire_stale_authorizations`] run it, so a sweep never expires a
/// pledge without its alerts.
///
/// The other CTEs read the table as it was before the flip, so the rows this
/// sweep expires come from `expired` (`fresh`), the earlier ones from the
/// table. A repeated expiry counts a wallet's attempts at one item created
/// within the window before the newest fresh one (a pledge reaches the sweep
/// only after its TTL and the index's lag, so the window is anchored on the
/// attempts, not on the sweep), and is keyed by that newest attempt: each
/// sweep that adds one past the threshold alerts once.
pub(crate) fn expiry_ctes(now: &str, chain: &str) -> String {
    let bound = super::usage::expiry_bound_sql(now, chain);
    let window = format!("interval '{REPEATED_EXPIRY_WINDOW_SECS} seconds'");
    format!(
        "expired AS ( \
             UPDATE credits.authorizations SET status = CASE status WHEN 'retired' THEN 'released' ELSE 'expired' END \
             WHERE status IN ('authorized', 'retired') AND expires_at <= {bound} \
             RETURNING id, status, address, usd_cents, amount_wei, submitted_tx_hash, contract_address, item_id, created_at \
         ), submitted_unconsumed AS ( \
             INSERT INTO credits.alerts (kind, dedupe_key, payload, created_at, next_attempt_at) \
             SELECT '{SUBMITTED_UNCONSUMED}', e.id, \
                    jsonb_build_object('salt', e.id, 'address', e.address, 'usdCents', e.usd_cents, \
                                       'capWei', e.amount_wei::text, 'txHash', e.submitted_tx_hash), {now}, {now} \
             FROM expired e \
             WHERE {chain}::bigint IS NOT NULL AND e.submitted_tx_hash IS NOT NULL \
               AND NOT EXISTS (SELECT 1 FROM credits.usages u WHERE u.salt = e.id) \
               AND NOT EXISTS (SELECT 1 FROM credits.authorizations s \
                               WHERE s.submitted_tx_hash = e.submitted_tx_hash AND s.address = e.address AND s.id <> e.id \
                                 AND (s.status = 'consumed' OR EXISTS (SELECT 1 FROM credits.usages su WHERE su.salt = s.id))) \
             ON CONFLICT (kind, dedupe_key) DO NOTHING RETURNING 1 \
         ), expired_attempts AS ( \
             SELECT a.id, a.address, a.contract_address, a.item_id, a.created_at, false AS fresh \
             FROM credits.authorizations a \
             WHERE a.status IN ('expired', 'released') AND a.address IN (SELECT address FROM expired) \
               AND a.created_at > (SELECT min(e.created_at) FROM expired e WHERE e.address = a.address) - {window} \
             UNION ALL \
             SELECT e.id, e.address, e.contract_address, e.item_id, e.created_at, true FROM expired e \
         ), expired_items AS ( \
             SELECT DISTINCT t.id, t.address, t.created_at, t.fresh, \
                    COALESCE(l.asset_contract || ':' || COALESCE(l.asset_item_id::text, l.asset_token_id::text, lower(l.registered_name)), \
                             t.contract_address || ':' || t.item_id) AS item \
             FROM expired_attempts t LEFT JOIN credits.authorization_lines l ON l.authorization_id = t.id \
         ), anchored AS ( \
             SELECT i.*, max(i.created_at) FILTER (WHERE i.fresh) OVER (PARTITION BY i.address, i.item) AS anchor \
             FROM expired_items i WHERE i.item IS NOT NULL \
         ), repeated_expiry AS ( \
             INSERT INTO credits.alerts (kind, dedupe_key, payload, created_at, next_attempt_at) \
             SELECT '{REPEATED_EXPIRY}', g.address || ':' || g.item || ':' || g.newest, \
                    jsonb_build_object('address', g.address, 'item', g.item, 'attempts', g.attempts, \
                                       'windowSecs', {REPEATED_EXPIRY_WINDOW_SECS}), {now}, {now} \
             FROM (SELECT address, item, count(DISTINCT id) AS attempts, \
                          (array_agg(id ORDER BY created_at DESC, id DESC) FILTER (WHERE fresh))[1] AS newest \
                   FROM anchored WHERE anchor IS NOT NULL AND created_at > anchor - {window} \
                   GROUP BY address, item) g \
             WHERE g.attempts >= {REPEATED_EXPIRY_ATTEMPTS} \
             ON CONFLICT (kind, dedupe_key) DO NOTHING RETURNING 1 \
         )"
    )
}

/// Where the alerts go: the Slack incoming webhook through the node's
/// outbound client, or, with none, the error log.
#[derive(Clone)]
pub struct Delivery {
    webhook: Option<Arc<str>>,
    /// `SHOP_BASE_URL`: where a settled item links to.
    shop: Arc<str>,
    http: Outbound,
}

/// Why a post was not accepted.
enum Refusal {
    /// Retried with backoff; the reason never names the webhook.
    Failed(String),
    /// Slack's 429, with its `Retry-After` when it sent one.
    RateLimited(Option<chrono::Duration>),
}

/// A claimed alert.
struct Claimed {
    id: i64,
    kind: String,
    payload: Value,
    attempts: i32,
    created_at: chrono::NaiveDateTime,
    held_since: Option<chrono::NaiveDateTime>,
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
    pub fn new(webhook: Option<String>, shop_base_url: &str, http: Outbound) -> Self {
        Self {
            webhook: webhook.map(Arc::from),
            shop: Arc::from(shop_base_url.trim_end_matches('/')),
            http,
        }
    }

    /// One pass at `now`: claims up to [`BATCH`] due alerts for [`LEASE`],
    /// then ends each `sent`, `skipped` or `failed`, or schedules its retry.
    /// Each alert is a message of its own, except the settled purchases, which
    /// share messages of up to [`SETTLED_PER_MESSAGE`] lines; a message's
    /// alerts share its outcome.
    pub async fn run_pass(
        &self,
        credits: &Credits,
        now: chrono::NaiveDateTime,
    ) -> Result<PassOutcome, ApiError> {
        let lease_until = now + LEASE;
        let mut claimed: Vec<Claimed> = sqlx::query(
            "UPDATE credits.alerts a SET attempts = a.attempts + 1, next_attempt_at = $2 \
             WHERE a.id IN (SELECT id FROM credits.alerts WHERE status = 'pending' AND next_attempt_at <= $1 \
                            ORDER BY next_attempt_at, id LIMIT $3 FOR UPDATE SKIP LOCKED) \
             RETURNING a.id, a.kind, a.payload, a.attempts, a.created_at, a.last_error",
        )
        .bind(now)
        .bind(lease_until)
        .bind(BATCH)
        .fetch_all(&credits.pool)
        .await?
        .into_iter()
        .map(|row| Claimed {
            id: row.get("id"),
            kind: row.get("kind"),
            payload: row.get::<sqlx::types::Json<Value>, _>("payload").0,
            attempts: row.get("attempts"),
            created_at: row.get("created_at"),
            held_since: row
                .get::<Option<&str>, _>("last_error")
                .and_then(|reason| reason.strip_prefix(HOLD_SINCE_PREFIX))
                .and_then(|since| chrono::DateTime::parse_from_rfc3339(since).ok())
                .map(|since| since.naive_utc()),
        })
        .collect();
        claimed.sort_by_key(|alert| alert.id);
        let mut outcome = PassOutcome::default();
        let mut due = Vec::new();
        for alert in claimed {
            if self.webhook.is_none() {
                let text = render(&alert.kind, &alert.payload, &self.shop);
                tracing::error!(alert = %text, kind = alert.kind, "CREDITS_ALERTS_SLACK_WEBHOOK is unset: a credits alert is logged here and not sent");
                finish(credits, alert.id, "skipped", now).await?;
                outcome.skipped += 1;
            } else if alert.attempts > 1 && alert.created_at <= now - MAX_AGE {
                // Given up only after one attempted post: an alert queued while
                // no writer delivered is posted late rather than never.
                let text = render(&alert.kind, &alert.payload, &self.shop);
                tracing::error!(alert = %text, kind = alert.kind, attempts = alert.attempts, "a credits alert went undelivered for a day and is given up");
                finish(credits, alert.id, "failed", now).await?;
                outcome.failed += 1;
            } else {
                due.push(alert);
            }
        }
        let Some(webhook) = self.webhook.as_deref() else {
            return Ok(outcome);
        };
        let (settled, single): (Vec<Claimed>, Vec<Claimed>) = due
            .into_iter()
            .partition(|alert| alert.kind == PURCHASE_SETTLED);
        let mut messages: Vec<(Vec<&Claimed>, String)> = single
            .iter()
            .map(|alert| (vec![alert], render(&alert.kind, &alert.payload, &self.shop)))
            .collect();
        for chunk in settled.chunks(SETTLED_PER_MESSAGE) {
            let payloads: Vec<&Value> = chunk.iter().map(|alert| &alert.payload).collect();
            messages.push((
                chunk.iter().collect(),
                settled_message(&payloads, &self.shop),
            ));
        }
        messages.sort_by_key(|(alerts, _)| alerts[0].id);
        // Once Slack rate limits, the rest of the pass waits as it asked.
        let mut hold = None;
        for (alerts, text) in messages {
            if let Some((limited_at, until)) = hold {
                for alert in &alerts {
                    let held_since = alert.held_since.unwrap_or(limited_at);
                    if limited_at - held_since >= MAX_HOLD {
                        if expire_held(credits, alert.id, lease_until, limited_at).await? {
                            let text = render(&alert.kind, &alert.payload, &self.shop);
                            tracing::error!(alert = %text, kind = alert.kind, "a credits alert reached the Slack hold age limit and is given up");
                            outcome.failed += 1;
                        }
                        continue;
                    }
                    let rescheduled = retry(
                        credits,
                        alert.id,
                        lease_until,
                        until,
                        &format!("{HOLD_SINCE_PREFIX}{}", held_since.and_utc().to_rfc3339()),
                        true,
                    )
                    .await?;
                    outcome.retried += rescheduled;
                }
                continue;
            }
            match self.post(webhook, &text).await {
                Ok(()) => {
                    for alert in &alerts {
                        finish(credits, alert.id, "sent", now).await?;
                        outcome.sent += 1;
                    }
                }
                Err(Refusal::RateLimited(wait)) => {
                    let attempts = alerts.iter().map(|alert| alert.attempts).max().unwrap_or(1);
                    let limited_at = clock(now);
                    let until = limited_at + wait.unwrap_or_else(|| backoff(attempts));
                    tracing::warn!(attempts, %until, "Slack rate limited a credits alert; the pass waits as it asked");
                    for alert in &alerts {
                        let rescheduled = retry(
                            credits,
                            alert.id,
                            lease_until,
                            until,
                            "Slack answered 429 Too Many Requests",
                            false,
                        )
                        .await?;
                        outcome.retried += rescheduled;
                    }
                    hold = Some((limited_at, until));
                }
                Err(Refusal::Failed(error)) => {
                    let failed_at = clock(now);
                    for alert in &alerts {
                        tracing::warn!(kind = alert.kind, attempts = alert.attempts, %error, "a credits alert was not delivered; retried with backoff");
                        let rescheduled = retry(
                            credits,
                            alert.id,
                            lease_until,
                            failed_at + backoff(alert.attempts),
                            &error,
                            false,
                        )
                        .await?;
                        outcome.retried += rescheduled;
                    }
                }
            }
        }
        Ok(outcome)
    }

    /// Posts `text`, following no redirect: a 3xx would log the webhook (its
    /// path is the credential) and turn the post into a body-less GET. The
    /// refusal's reason never names the webhook.
    async fn post(&self, webhook: &str, text: &str) -> Result<(), Refusal> {
        let response = self
            .http
            .post(webhook)
            .max_redirects(0)
            .json(&json!({ "text": text }))
            .timeout(POST_TIMEOUT)
            .send()
            .await
            .map_err(|error| {
                Refusal::Failed(if error.is_timeout() {
                    "the post timed out".to_string()
                } else if error.is_connect() {
                    "the webhook did not accept a connection".to_string()
                } else {
                    "the post failed".to_string()
                })
            })?;
        let status = response.status();
        if status.is_success() {
            Ok(())
        } else if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
            let wait = response
                .headers()
                .get(reqwest::header::RETRY_AFTER)
                .and_then(|value| value.to_str().ok())
                .and_then(|value| value.trim().parse::<i64>().ok())
                .map(|seconds| {
                    chrono::Duration::seconds(seconds.clamp(1, MAX_BACKOFF.num_seconds()))
                });
            Err(Refusal::RateLimited(wait))
        } else {
            Err(Refusal::Failed(format!("Slack answered {status}")))
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

/// The time a retry is scheduled from: the pass's clock, or later once its
/// slow posts took time.
fn clock(now: chrono::NaiveDateTime) -> chrono::NaiveDateTime {
    now.max(super::store::now())
}

/// Reschedules this pass's claim, undoing the attempt for an unposted alert.
async fn retry(
    credits: &Credits,
    id: i64,
    lease_until: chrono::NaiveDateTime,
    at: chrono::NaiveDateTime,
    error: &str,
    unposted: bool,
) -> Result<usize, ApiError> {
    let result = sqlx::query(
        "UPDATE credits.alerts SET next_attempt_at = $2, last_error = $3, attempts = attempts - $4 \
         WHERE id = $1 AND status = 'pending' AND next_attempt_at = $5",
    )
    .bind(id)
    .bind(at)
    .bind(error)
    .bind(i32::from(unposted))
    .bind(lease_until)
    .execute(&credits.pool)
    .await?;
    Ok(result.rows_affected() as usize)
}

/// Ends this pass's expired hold and refunds the unposted claim.
async fn expire_held(
    credits: &Credits,
    id: i64,
    lease_until: chrono::NaiveDateTime,
    at: chrono::NaiveDateTime,
) -> Result<bool, ApiError> {
    let result = sqlx::query(
        "UPDATE credits.alerts SET status = 'failed', finished_at = $2, \
         last_error = 'Slack hold age limit reached', attempts = attempts - 1 \
         WHERE id = $1 AND status = 'pending' AND next_attempt_at = $3",
    )
    .bind(id)
    .bind(at)
    .bind(lease_until)
    .execute(&credits.pool)
    .await?;
    Ok(result.rows_affected() == 1)
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

/// The Slack message of one alert; `shop` is where a settled item links to.
pub(crate) fn render(kind: &str, payload: &Value, shop: &str) -> String {
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
            .map_or("an unknown amount".into(), |wei| {
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
        PACK_CREDITED => pack_credited(payload),
        PACK_REVERSED => {
            let loss = if payload["hasShortfall"].as_bool() == Some(true) {
                format!(
                    " · :rotating_light: {} credits had already been spent: an unrecovered loss",
                    credits_text(&text("shortfall"))
                )
            } else {
                String::new()
            };
            format!(
                ":warning: *Refund / chargeback* — {} credits clawed back · pack `{}` · {} · buyer `{address}`{loss}",
                credits_text(&text("chargedBack")),
                text("packId").replace('`', "'"),
                text("reason"),
            )
        }
        REVERSAL_UNMATCHED => format!(
            ":warning: *Unmatched {}* — PaymentIntent `{}` matched no paid purchase; recorded, and applied if that purchase is credited later",
            text("reason"),
            text("paymentIntent").replace('`', "'"),
        ),
        PURCHASE_SETTLED => settled_message(&[payload], shop),
        super::daily_summary::DAILY_SUMMARY => super::daily_summary::message(payload),
        other => format!("Credits alert {other}: {payload}"),
    }
}

/// A pack's line: what it credited and charged, and the spread the sale left
/// after the card fee (estimated) and the credits it issued at 10 cents each.
/// It is no profit: the creator's payout when the credits are spent is not
/// netted off.
fn pack_credited(payload: &Value) -> String {
    let raw = payload["credits"].as_str().unwrap_or("0");
    let credits = whole_credits(raw);
    let cents = payload["chargedCents"].as_i64().unwrap_or(0);
    let quantity = payload["quantity"].as_i64().unwrap_or(1);
    let fee = stripe_fee_cents(cents, 1);
    let spread = match credits {
        Some(credits) if cents > 0 => {
            let spread = cents - credits * 10 - fee;
            format!(
                " · spread *{}* ({}%) after {} Stripe (est.)",
                usd(spread),
                tenths(round_div(spread * 1000, cents)),
                usd(fee)
            )
        }
        _ => String::new(),
    };
    let packs = if quantity > 1 {
        format!("{quantity} × pack")
    } else {
        "pack".to_string()
    };
    format!(
        "{} *Credits purchased* — {} credits ({}) · {packs} `{}`{spread} · buyer `{}`{}",
        pack_emoji(credits.unwrap_or(0)),
        credits.map_or_else(|| credits_text(raw), grouped),
        usd(cents),
        payload["packId"]
            .as_str()
            .unwrap_or("unknown")
            .replace('`', "'"),
        payload["address"].as_str().unwrap_or("unknown"),
        via(payload),
    )
}

/// One message for settled purchases: a line of its own for one, a header
/// counting them over a line each for several.
fn settled_message(payloads: &[&Value], shop: &str) -> String {
    if let [payload] = payloads {
        return format!(
            ":shopping_bags: *Purchased* — {}",
            settled_line(payload, shop)
        );
    }
    let items: i64 = payloads
        .iter()
        .map(|p| p["lines"].as_i64().unwrap_or(1).max(1))
        .sum();
    let cents: i64 = payloads
        .iter()
        .map(|p| p["usdCents"].as_i64().unwrap_or(0))
        .sum();
    let buyers = payloads
        .iter()
        .filter_map(|p| p["address"].as_str())
        .collect::<std::collections::BTreeSet<_>>()
        .len();
    let plural =
        |n: i64, word: &str| format!("{} {word}{}", grouped(n), if n == 1 { "" } else { "s" });
    let mut text = format!(
        ":shopping_bags: *{} purchases* — {} · {} credits ({}) · {}",
        payloads.len(),
        plural(items, "item"),
        grouped(credits_of(cents)),
        usd(cents),
        plural(buyers as i64, "buyer"),
    );
    for payload in payloads {
        text.push_str("\n      • ");
        text.push_str(&settled_line(payload, shop));
    }
    text
}

/// One settled purchase: what it bought, its credits and cents, the MANA it
/// settled, its transaction and buyer.
fn settled_line(payload: &Value, shop: &str) -> String {
    let lines = payload["lines"].as_i64().unwrap_or(1).max(1);
    let items: Vec<(&str, &str)> = payload["items"]
        .as_array()
        .map(|items| {
            items
                .iter()
                .filter_map(|i| Some((i["contractAddress"].as_str()?, i["itemId"].as_str()?)))
                .collect()
        })
        .unwrap_or_default();
    let count = if lines == 1 {
        "1 item".to_string()
    } else {
        format!("{lines} items")
    };
    // Never naming more than settled: a NAME only when it is the one line,
    // the items only when every line carries one.
    let bought = match payload["registeredName"].as_str() {
        Some(name) if lines == 1 => format!("NAME @{}", slack_label(name)),
        _ if items.len() as i64 == lines && lines == 1 => items_list(&items, shop),
        _ if items.len() as i64 == lines => format!("{count}: {}", items_list(&items, shop)),
        _ => count,
    };
    let cents = payload["usdCents"].as_i64().unwrap_or(0);
    let credits = credits_of(cents);
    let credits = if credits == 1 {
        "1 credit".to_string()
    } else {
        format!("{} credits", grouped(credits))
    };
    let tx = tx_ref(payload["chainId"].as_i64(), payload["txHash"].as_str())
        .map(|tx| format!(" · {tx}"))
        .unwrap_or_default();
    let alarm = if payload["recovered"].as_bool() == Some(true) {
        " · :rotating_light: recovered after expiry"
    } else {
        ""
    };
    format!(
        "{bought} · {credits} ({}) · {} MANA{tx} · buyer `{}`{alarm}{}",
        usd(cents),
        mana(payload["manaWei"].as_str().unwrap_or("0")),
        payload["address"].as_str().unwrap_or("unknown"),
        via(payload),
    )
}

/// The distinct items, each linked to its Shop page with its count, the rest
/// past [`ITEMS_PER_LINE`] counted.
fn items_list(items: &[(&str, &str)], shop: &str) -> String {
    let mut distinct: Vec<((&str, &str), usize)> = Vec::new();
    for item in items {
        match distinct.iter_mut().find(|(seen, _)| seen == item) {
            Some((_, quantity)) => *quantity += 1,
            None => distinct.push((*item, 1)),
        }
    }
    let shown: Vec<String> = distinct
        .iter()
        .take(ITEMS_PER_LINE)
        .map(|((contract, item), quantity)| {
            let times = if *quantity > 1 {
                format!(" ×{quantity}")
            } else {
                String::new()
            };
            format!(
                "<{shop}/item/{}/{}|item {}>{times}",
                contract.to_ascii_lowercase(),
                urlencoding::encode(item),
                slack_label(item)
            )
        })
        .collect();
    let more = distinct.len().saturating_sub(ITEMS_PER_LINE);
    let more = if more > 0 {
        format!(" +{more} more")
    } else {
        String::new()
    };
    format!("{}{more}", shown.join(", "))
}

/// ` · via <surface>` when the checkout named one.
fn via(payload: &Value) -> String {
    payload["source"]
        .as_str()
        .map(slack_label)
        .filter(|source| !source.is_empty())
        .map(|source| format!(" · via {source}"))
        .unwrap_or_default()
}

/// The transaction linked to its chain's explorer, or the bare hash on a
/// chain with none mapped (a guessed host would read as authoritative).
fn tx_ref(chain_id: Option<i64>, hash: Option<&str>) -> Option<String> {
    let hash = hash?;
    if !hash.starts_with("0x") || !hash[2..].chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let explorer = match chain_id {
        Some(1) => Some("https://etherscan.io"),
        Some(11_155_111) => Some("https://sepolia.etherscan.io"),
        Some(137) => Some("https://polygonscan.com"),
        Some(80_002) => Some("https://amoy.polygonscan.com"),
        _ => None,
    };
    Some(match explorer {
        Some(base) => format!("<{base}/tx/{hash}|tx>"),
        None => format!("tx `{hash}`"),
    })
}

/// Emoji by pack size, so a day's sales read at a glance.
fn pack_emoji(credits: i64) -> &'static str {
    match credits {
        500.. => ":money_mouth_face:",
        250.. => ":money_with_wings:",
        100.. => ":dollar:",
        _ => ":coin:",
    }
}

/// Credits a charge buys at 10 cents each, rounded up.
fn credits_of(cents: i64) -> i64 {
    (cents.max(0) + 9) / 10
}

/// A credits amount whose fraction is zero, as a whole number.
fn whole_credits(text: &str) -> Option<i64> {
    let (whole, fraction) = text.split_once('.').unwrap_or((text, ""));
    fraction
        .chars()
        .all(|c| c == '0')
        .then(|| whole.parse().ok())
        .flatten()
}

/// A numeric credits amount without trailing fractional zeros.
fn credits_text(text: &str) -> String {
    match text.split_once('.') {
        Some((whole, fraction)) => {
            let fraction = fraction.trim_end_matches('0');
            if fraction.is_empty() {
                whole.to_string()
            } else {
                format!("{whole}.{fraction}")
            }
        }
        None => text.to_string(),
    }
}

/// Stripe's estimated fee on `payments` card payments charging `cents` in
/// all.
pub(super) fn stripe_fee_cents(cents: i64, payments: i64) -> i64 {
    round_div(cents * STRIPE_FEE_PER_MILLE, 1000) + STRIPE_FEE_FIXED_CENTS * payments
}

/// `n / d` rounded half away from zero; 0 for a zero `d`.
pub(super) fn round_div(n: i64, d: i64) -> i64 {
    if d == 0 {
        return 0;
    }
    let q = (2 * n.unsigned_abs() + d.unsigned_abs()) / (2 * d.unsigned_abs());
    let q = q as i64;
    if (n < 0) != (d < 0) { -q } else { q }
}

/// Tenths as a one-decimal number.
pub(super) fn tenths(tenths: i64) -> String {
    let sign = if tenths < 0 { "-" } else { "" };
    let abs = tenths.unsigned_abs();
    format!("{sign}{}.{}", abs / 10, abs % 10)
}

/// A whole number with thousands separated.
pub(super) fn grouped(n: i64) -> String {
    let digits = n.unsigned_abs().to_string();
    let mut out = String::new();
    for (i, digit) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(digit);
    }
    if n < 0 { format!("-{out}") } else { out }
}

/// Cents as dollars, thousands separated.
pub(super) fn usd(cents: i64) -> String {
    let sign = if cents < 0 { "-" } else { "" };
    let abs = cents.unsigned_abs();
    format!("{sign}${}.{:02}", grouped((abs / 100) as i64), abs % 100)
}

/// 18-decimal MANA to at most four decimals, truncated: a settlement figure
/// is never rounded up.
pub(super) fn mana(wei: &str) -> String {
    let Ok(wei) = U256::from_str(wei) else {
        return "an unknown amount of".to_string();
    };
    let unit = U256::from(10u64).pow(U256::from(18u64));
    let fraction = (wei % unit) / U256::from(10u64).pow(U256::from(14u64));
    let fraction = format!("{fraction:0>4}");
    let fraction = fraction.trim_end_matches('0');
    if fraction.is_empty() {
        (wei / unit).to_string()
    } else {
        format!("{}.{fraction}", wei / unit)
    }
}

/// Text a user typed (a NAME, a surface) made inert for Slack: one line,
/// without the characters that format or end a link label, capped at 40
/// characters and escaped.
fn slack_label(text: &str) -> String {
    const MAX: usize = 40;
    let format_char = |c: char| {
        matches!(c, '\u{00AD}' | '\u{061C}' | '\u{180E}' | '\u{200B}'..='\u{200F}'
            | '\u{202A}'..='\u{202E}' | '\u{2060}'..='\u{2064}' | '\u{2066}'..='\u{206F}' | '\u{FEFF}')
    };
    let flat: String = text
        .chars()
        .map(|c| {
            if c.is_whitespace() || c.is_control() || format_char(c) {
                ' '
            } else {
                c
            }
        })
        .filter(|c| !matches!(c, '*' | '~' | '`' | '|'))
        .collect();
    let words: Vec<&str> = flat
        .split(' ')
        .map(|word| word.trim_matches('_'))
        .filter(|word| !word.is_empty())
        .collect();
    let joined = words.join(" ");
    let capped: String = if joined.chars().count() > MAX {
        let head: String = joined.chars().take(MAX - 1).collect();
        format!("{}…", head.trim_end())
    } else {
        joined
    };
    capped
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
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

    const SHOP: &str = "https://shop.example";

    #[test]
    fn the_retry_wait_doubles_up_to_half_an_hour() {
        assert_eq!(backoff(1), chrono::Duration::seconds(30));
        assert_eq!(backoff(2), chrono::Duration::seconds(60));
        assert_eq!(backoff(6), chrono::Duration::seconds(960));
        assert_eq!(backoff(7), MAX_BACKOFF);
        assert_eq!(backoff(i32::MAX), MAX_BACKOFF);
    }

    #[test]
    fn the_feed_reads_like_the_shop_channel() {
        let pack = render(
            PACK_CREDITED,
            &json!({ "address": "0x02", "credits": "500", "chargedCents": 5500, "packId": "pack_500", "quantity": 1, "source": "web" }),
            SHOP,
        );
        assert_eq!(
            pack,
            ":money_mouth_face: *Credits purchased* — 500 credits ($55.00) · pack `pack_500` · spread *$3.10* (5.6%) after $1.90 Stripe (est.) · buyer `0x02` · via web"
        );
        let packs = render(
            PACK_CREDITED,
            &json!({ "address": "0x02", "credits": "150.000", "chargedCents": 1500, "packId": "pack`50", "quantity": 3, "source": null }),
            SHOP,
        );
        assert_eq!(
            packs,
            ":dollar: *Credits purchased* — 150 credits ($15.00) · 3 × pack `pack'50` · spread *-$0.74* (-4.9%) after $0.74 Stripe (est.) · buyer `0x02`"
        );
        let reversed = render(
            PACK_REVERSED,
            &json!({ "address": "0x02", "chargedBack": "25.0000", "shortfall": "5.000", "hasShortfall": true, "packId": "pack_50", "reason": "refund" }),
            SHOP,
        );
        assert_eq!(
            reversed,
            ":warning: *Refund / chargeback* — 25 credits clawed back · pack `pack_50` · refund · buyer `0x02` · :rotating_light: 5 credits had already been spent: an unrecovered loss"
        );
        let unmatched = render(
            REVERSAL_UNMATCHED,
            &json!({ "paymentIntent": "pi_1", "reason": "chargeback" }),
            SHOP,
        );
        assert_eq!(
            unmatched,
            ":warning: *Unmatched chargeback* — PaymentIntent `pi_1` matched no paid purchase; recorded, and applied if that purchase is credited later"
        );
    }

    #[test]
    fn settled_purchases_share_a_message_and_name_only_what_settled() {
        let hash = format!("0x{}", "ab".repeat(32));
        let cart = json!({ "address": "0x02", "usdCents": 1230, "manaWei": "12345678900000000000", "txHash": hash, "chainId": 137,
            "recovered": false, "source": null, "lines": 2, "registeredName": null,
            "items": [{ "contractAddress": "0xabc", "itemId": "7" }, { "contractAddress": "0xabc", "itemId": "7" }] });
        assert_eq!(
            render(PURCHASE_SETTLED, &cart, SHOP),
            format!(
                ":shopping_bags: *Purchased* — 2 items: <https://shop.example/item/0xabc/7|item 7> ×2 · 123 credits ($12.30) · 12.3456 MANA · <https://polygonscan.com/tx/{hash}|tx> · buyer `0x02`"
            )
        );
        let name = json!({ "address": "0x03", "usdCents": 10, "manaWei": "0", "txHash": hash, "chainId": 999,
            "recovered": true, "source": "unity<script>", "lines": 1, "registeredName": "co*ol_name_", "items": [] });
        assert_eq!(
            render(PURCHASE_SETTLED, &name, SHOP),
            format!(
                ":shopping_bags: *Purchased* — NAME @cool_name · 1 credit ($0.10) · 0 MANA · tx `{hash}` · buyer `0x03` · :rotating_light: recovered after expiry · via unity&lt;script&gt;"
            )
        );
        let unlisted = json!({ "address": "0x04", "usdCents": 500, "manaWei": "1", "lines": 3,
            "items": [{ "contractAddress": "0xabc", "itemId": "7" }] });
        assert!(
            render(PURCHASE_SETTLED, &unlisted, SHOP)
                .starts_with(":shopping_bags: *Purchased* — 3 items · 50 credits"),
            "three lines, one item known: the count alone"
        );
        let both = settled_message(&[&cart, &name], SHOP);
        let mut lines = both.lines();
        assert_eq!(
            lines.next(),
            Some(":shopping_bags: *2 purchases* — 3 items · 124 credits ($12.40) · 2 buyers")
        );
        assert!(lines.all(|line| line.starts_with("      • ")));
        assert_eq!(both.lines().count(), 3);
    }

    #[test]
    fn user_text_is_inert_and_amounts_read_plainly() {
        assert_eq!(
            slack_label("a\u{200B}b  <x|y>\n_c_ `d`"),
            "a b &lt;xy&gt; c d"
        );
        assert_eq!(slack_label(&"a".repeat(50)), format!("{}…", "a".repeat(39)));
        assert_eq!(usd(203_637), "$2,036.37");
        assert_eq!(usd(-5), "-$0.05");
        assert_eq!(grouped(1_234_567), "1,234,567");
        assert_eq!(mana("1999999999999999999"), "1.9999");
        assert_eq!(mana("100000000000000"), "0.0001");
        assert_eq!(tx_ref(Some(137), Some("0xnothex")), None);
        assert_eq!(whole_credits("12.5"), None);
        assert_eq!(credits_text("12.500"), "12.5");
    }

    #[test]
    fn a_batch_of_posts_fits_in_the_lease() {
        let posts = POST_TIMEOUT * BATCH as u32;
        assert!(posts < LEASE.to_std().unwrap(), "{posts:?}");
    }

    #[tokio::test]
    async fn stale_retry_and_hold_expiry_leave_the_new_claim_intact() {
        let Some(db) = crate::test_db::OwnDatabase::create("alerts_stale_retry").await else {
            return;
        };
        let credits = Credits::new(db.pool.clone());
        let old_lease = now();
        let new_lease = old_lease + LEASE;
        let id: i64 = sqlx::query_scalar(
            "INSERT INTO credits.alerts (kind, dedupe_key, payload, attempts, next_attempt_at) \
             VALUES ($1, 'stale', '{}'::jsonb, 2, $2) RETURNING id",
        )
        .bind(SUBMITTED_UNCONSUMED)
        .bind(new_lease)
        .fetch_one(&db.pool)
        .await
        .unwrap();

        let rescheduled = retry(&credits, id, old_lease, old_lease + BACKOFF, "held", true)
            .await
            .unwrap();
        assert_eq!(rescheduled, 0);
        assert!(
            !expire_held(&credits, id, old_lease, new_lease)
                .await
                .unwrap()
        );

        let still_claimed: bool = sqlx::query_scalar(
            "SELECT status = 'pending' AND finished_at IS NULL AND attempts = 2 \
             AND last_error IS NULL AND next_attempt_at = $2 FROM credits.alerts WHERE id = $1",
        )
        .bind(id)
        .bind(new_lease)
        .fetch_one(&db.pool)
        .await
        .unwrap();
        assert!(still_claimed);
        db.drop().await;
    }

    #[test]
    fn each_alert_reads_as_one_line() {
        let cap = render(
            CAP_NEAR_LIMIT,
            &json!({ "salt": "0x01", "address": "0x02", "usdCents": 1234, "capWei": "10000000000000000000", "consumedWei": "9960000000000000000", "txHash": "0x03" }),
            SHOP,
        );
        assert_eq!(
            cap,
            ":warning: Credits: credit 0x01 consumed 9.96 of the 10 MANA it was signed for (at least 99.5%). Wallet 0x02, $12.34; tx 0x03."
        );
        let repeated = render(
            REPEATED_EXPIRY,
            &json!({ "address": "0x02", "item": "0xc:7", "attempts": 3, "windowSecs": 1800 }),
            SHOP,
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
            let line = render(kind, &json!({}), SHOP);
            assert!(!line.contains('\n') && line.starts_with(':'), "{line}");
        }
    }
}
