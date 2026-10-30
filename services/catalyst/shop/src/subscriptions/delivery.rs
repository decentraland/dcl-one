// from: shop-server@dc1a8be96:src/logic/notify/consumer.ts (notifyItemAvailable, buildItemUrl, buildUnsubscribeUrl, the page size), src/ports/email/component.ts (escapeHtml, renderItemAvailableEmail, sendItemAvailable), src/logic/notify/on-sale.ts (the on-sale signal)
//! The waitlist's trigger and sender, both local (SHOP.md 6: the SNS/SQS
//! hop replaced by a transactional outbox and `jobs`):
//!
//! - [`watch_pass`] (the `shop-notify-availability` job): for every item
//!   somebody still waits for, read `shop.items` and record the on-sale
//!   state in `shop.item_availability_watch`; a false -> true transition
//!   enqueues one outbox row per pending subscriber in the same transaction
//!   ([`item_on_sale`], also the in-process twin of upstream's
//!   `POST /notify/item-on-sale`). The first observation is the baseline:
//!   upstream keyed on the ITEM_PUBLISHED event, so an item already on sale
//!   when a buyer subscribes does not notify until it goes off and on again.
//! - [`run_pass`] (the `shop-notify-deliveries` job): claim the due rows
//!   under a lease (`FOR UPDATE SKIP LOCKED`, `attempts + 1`,
//!   `next_attempt_at = now + LEASE`), send each outside any transaction,
//!   then [`record`]: a sent row marks its request notified in the same
//!   statement (`WHERE status = 'pending'`, so a redelivery never
//!   re-notifies); a transient failure backs off (30 s doubling to an
//!   hour, five attempts then failed); an unconfigured sender ends the row
//!   `skipped`, never a silent success.

use crate::shared::now_utc;
use std::time::Duration;

use chrono::NaiveDateTime;
use serde_json::json;
use sqlx::PgPool;

use crate::social::notifications::email::{EmailSender, SendFailure};

pub const BATCH: i64 = 100;
pub const MAX_ATTEMPTS: i32 = 5;
pub const LEASE: Duration = Duration::from_secs(300);
/// Page size of the fan-out, so a viral item never loads every subscriber at once.
pub const PENDING_PAGE_SIZE: i64 = 100;
/// The watcher's bound per pass on distinct items with a pending subscriber.
pub const WATCH_ITEMS_PER_PASS: i64 = 500;
const BACKOFF_BASE: Duration = Duration::from_secs(30);
const BACKOFF_MAX: Duration = Duration::from_secs(3600);
const GENERIC_ITEM_NAME: &str = "An item you wanted";

/// An item a signal says is (again) available, normalised for the waitlist lookup.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OnSaleItem {
    /// Lowercased collection contract address.
    pub contract_address: String,
    pub item_id: String,
    /// The item name when the signal carried one (the email falls back to a generic name).
    pub item_name: Option<String>,
}

/// What the deliveries worker sends with. The production one is the
/// notifications module's SendGrid sender; tests plug a recorder in.
#[async_trait::async_trait]
pub trait AvailabilityMailer: Send + Sync {
    fn is_configured(&self) -> bool;
    /// Sends the "your item is available again" email; the provider's message id when it gives one.
    async fn send_item_available(
        &self,
        to: &str,
        item_name: Option<&str>,
        item_url: &str,
        unsubscribe_url: &str,
        outbox_id: i64,
    ) -> Result<Option<String>, SendFailure>;
}

#[async_trait::async_trait]
impl AvailabilityMailer for EmailSender {
    fn is_configured(&self) -> bool {
        self.html_configured()
    }

    async fn send_item_available(
        &self,
        to: &str,
        item_name: Option<&str>,
        item_url: &str,
        unsubscribe_url: &str,
        outbox_id: i64,
    ) -> Result<Option<String>, SendFailure> {
        let name = item_name
            .map(str::trim)
            .filter(|n| !n.is_empty())
            .unwrap_or(GENERIC_ITEM_NAME);
        let email = render_item_available_email(name, item_url, Some(unsubscribe_url));
        self.send_html(
            to,
            &email.subject,
            &email.html,
            &email.text,
            &json!({ "shop_notify_outbox_id": outbox_id.to_string() }),
        )
        .await
    }
}

/// Escapes the five HTML-significant characters, so creator-controlled
/// values (the item name) and URLs cannot inject markup into the body.
pub fn escape_html(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for c in value.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            other => out.push(other),
        }
    }
    out
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderedEmail {
    pub subject: String,
    pub html: String,
    pub text: String,
}

/// The "your item is available again" email: web2 copy only (no wallets,
/// chains or MANA), inline styles, one call to action deep-linking the item,
/// every interpolated value HTML-escaped; the subject and text are plain.
pub fn render_item_available_email(
    item_name: &str,
    item_url: &str,
    unsubscribe_url: Option<&str>,
) -> RenderedEmail {
    let subject = format!("{item_name} is available again");
    let mut text = format!(
        "Good news — {item_name} is available again in the Decentraland shop.\n\nView it here: {item_url}\n\nYou are receiving this because you asked to be notified when it came back. There is nothing else you need to do."
    );
    if let Some(u) = unsubscribe_url {
        text.push_str(&format!("\n\nStop these alerts: {u}"));
    }
    let safe_name = escape_html(item_name);
    let safe_url = escape_html(item_url);
    let unsubscribe_html = unsubscribe_url
        .map(|u| format!(r#"<p style="margin:8px 0 0 0;">Don't want these alerts? <a href="{}" style="color:#8f8b96;text-decoration:underline;">Unsubscribe</a>.</p>"#, escape_html(u)))
        .unwrap_or_default();
    let html = format!(
        r#"<!doctype html>
<html>
  <body style="margin:0;padding:0;background-color:#f4f4f7;font-family:Helvetica,Arial,sans-serif;color:#16141a;">
    <table role="presentation" width="100%" cellpadding="0" cellspacing="0" style="background-color:#f4f4f7;padding:24px 0;">
      <tr>
        <td align="center">
          <table role="presentation" width="480" cellpadding="0" cellspacing="0" style="background-color:#ffffff;border-radius:12px;overflow:hidden;">
            <tr>
              <td style="padding:32px 32px 8px 32px;">
                <h1 style="margin:0;font-size:22px;line-height:28px;color:#16141a;">Good news — it's back</h1>
              </td>
            </tr>
            <tr>
              <td style="padding:8px 32px 0 32px;font-size:15px;line-height:22px;color:#43404a;">
                <p style="margin:0 0 16px 0;"><strong>{safe_name}</strong> is available again in the Decentraland shop.</p>
                <p style="margin:0 0 24px 0;">You asked to be notified when it came back, so here it is. There's nothing else you need to do.</p>
              </td>
            </tr>
            <tr>
              <td style="padding:0 32px 32px 32px;" align="left">
                <a href="{safe_url}" style="display:inline-block;background-color:#ff2d55;color:#ffffff;text-decoration:none;font-size:15px;font-weight:bold;padding:14px 28px;border-radius:8px;">View item</a>
              </td>
            </tr>
            <tr>
              <td style="padding:0 32px 32px 32px;font-size:12px;line-height:18px;color:#8f8b96;border-top:1px solid #ece9f1;">
                <p style="margin:16px 0 0 0;">You're receiving this email because you signed up for an availability alert in the Decentraland shop.</p>
                {unsubscribe_html}
              </td>
            </tr>
          </table>
        </td>
      </tr>
    </table>
  </body>
</html>"#
    );
    RenderedEmail {
        subject,
        html,
        text,
    }
}

/// The Shop item page: `<base>/item/<contract>/<itemId>` (the canonical `/shop/item/...` page).
pub fn item_url(shop_base_url: &str, contract: &str, item_id: &str) -> String {
    format!(
        "{}/item/{contract}/{item_id}",
        shop_base_url.trim_end_matches('/')
    )
}

/// The opt-out link of the footer (the page calls `DELETE /v1/notify-requests` for the item).
pub fn unsubscribe_url(shop_base_url: &str, contract: &str, item_id: &str) -> String {
    format!(
        "{}/unsubscribe?contractAddress={contract}&itemId={item_id}",
        shop_base_url.trim_end_matches('/')
    )
}

/// The wait after the `attempts`-th failure: 30 s, 60 s, 120 s ... capped at an hour.
pub fn backoff(attempts: i32) -> Duration {
    let factor = 1u32 << attempts.clamp(1, 16).saturating_sub(1) as u32;
    BACKOFF_BASE.saturating_mul(factor).min(BACKOFF_MAX)
}

/// Enqueues one delivery per pending subscriber of the item (the
/// `pending_once` unique makes a duplicate trigger a no-op) on the given
/// executor, so a caller can make it part of the transaction that records
/// the signal. Returns the rows enqueued.
pub async fn enqueue_on<'e, E>(executor: E, item: &OnSaleItem, trigger: &str) -> sqlx::Result<u64>
where
    E: sqlx::PgExecutor<'e>,
{
    let res = sqlx::query(
        "INSERT INTO shop.item_notify_outbox (request_id, contract_address, item_id, item_name, trigger) \
         SELECT r.id, r.contract_address, r.item_id, $3, $4 \
         FROM shop.item_notify_requests r \
         WHERE r.contract_address = $1 AND r.item_id = $2 AND r.status = 'pending' \
         ON CONFLICT (request_id) WHERE status = 'pending' DO NOTHING",
    )
    .bind(&item.contract_address)
    .bind(&item.item_id)
    .bind(&item.item_name)
    .bind(trigger)
    .execute(executor)
    .await?;
    Ok(res.rows_affected())
}

/// The upstream `POST /notify/item-on-sale` as an in-process call: an
/// indexer that sees an item come on sale enqueues its waitlist here; the
/// deliveries job sends. Idempotent while the rows are pending.
pub async fn item_on_sale(pool: &PgPool, item: &OnSaleItem, trigger: &str) -> sqlx::Result<u64> {
    let enqueued = enqueue_on(pool, item, trigger).await?;
    if enqueued > 0 {
        tracing::info!(contract = %item.contract_address, item_id = %item.item_id, enqueued, trigger, "item on sale: waitlist deliveries enqueued");
    }
    Ok(enqueued)
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct WatchSummary {
    pub items: usize,
    pub observed: usize,
    pub transitions: usize,
    pub enqueued: u64,
}

/// What the items mirror says of an item: `None` when the mirror has no
/// such row (unknown contract, item not mirrored yet: no verdict).
async fn on_sale_now(
    pool: &PgPool,
    contract: &str,
    item_id: &str,
) -> sqlx::Result<Option<(bool, String)>> {
    if !item_id.bytes().all(|b| b.is_ascii_digit()) || item_id.len() > 78 {
        return Ok(None);
    }
    let row: Option<(bool, String)> = sqlx::query_as(
        "SELECT (available > 0 AND search_is_store_minter AND search_is_collection_approved), name \
         FROM shop.items WHERE contract_address = $1 AND item_id = $2::numeric LIMIT 1",
    )
    .bind(contract)
    .bind(item_id)
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

/// One watcher pass over the distinct items with a pending subscriber
/// (bounded): record the observation and, on a false -> true transition,
/// enqueue the waitlist in the same transaction.
pub async fn watch_pass(pool: &PgPool) -> anyhow::Result<WatchSummary> {
    let items: Vec<(String, String)> = sqlx::query_as(
        "SELECT DISTINCT contract_address, item_id FROM shop.item_notify_requests WHERE status = 'pending' ORDER BY contract_address, item_id LIMIT $1",
    )
    .bind(WATCH_ITEMS_PER_PASS)
    .fetch_all(pool)
    .await?;
    let mut summary = WatchSummary {
        items: items.len(),
        ..Default::default()
    };
    for (contract, item_id) in items {
        let Some((on_sale, name)) = on_sale_now(pool, &contract, &item_id).await? else {
            continue;
        };
        summary.observed += 1;
        let now = now_utc();
        let mut tx = pool.begin().await?;
        let previous: Option<Option<bool>> = sqlx::query_scalar(
            "SELECT on_sale FROM shop.item_availability_watch WHERE contract_address = $1 AND item_id = $2 FOR UPDATE",
        )
        .bind(&contract)
        .bind(&item_id)
        .fetch_optional(&mut *tx)
        .await?;
        let transition = matches!(previous, Some(Some(false))) && on_sale;
        sqlx::query(
            "INSERT INTO shop.item_availability_watch (contract_address, item_id, on_sale, observed_at, changed_at) \
             VALUES ($1, $2, $3, $4, CASE WHEN $5 THEN $4 ELSE NULL END) \
             ON CONFLICT (contract_address, item_id) DO UPDATE \
             SET on_sale = EXCLUDED.on_sale, observed_at = EXCLUDED.observed_at, \
                 changed_at = CASE WHEN shop.item_availability_watch.on_sale IS DISTINCT FROM EXCLUDED.on_sale THEN EXCLUDED.observed_at ELSE shop.item_availability_watch.changed_at END",
        )
        .bind(&contract)
        .bind(&item_id)
        .bind(on_sale)
        .bind(now)
        .bind(on_sale)
        .execute(&mut *tx)
        .await?;
        if transition {
            let item = OnSaleItem {
                contract_address: contract.clone(),
                item_id: item_id.clone(),
                item_name: Some(name),
            };
            let enqueued = enqueue_on(
                &mut *tx,
                &item,
                &format!("items-mirror:{}", now.format("%Y-%m-%dT%H:%M:%SZ")),
            )
            .await?;
            summary.transitions += 1;
            summary.enqueued += enqueued;
            tracing::info!(contract = %contract, item_id = %item_id, enqueued, "item on sale again: waitlist deliveries enqueued");
        }
        tx.commit().await?;
    }
    Ok(summary)
}

/// A claimed outbox row with what the send needs (the email read from the
/// request at claim time, never copied into the outbox).
#[derive(Debug, Clone)]
pub struct Claimed {
    pub id: i64,
    pub request_id: i64,
    pub attempts: i32,
    pub contract_address: String,
    pub item_id: String,
    pub item_name: Option<String>,
    pub email: String,
    /// Whether the request is still pending: an unsubscribed or already
    /// notified request makes the row `skipped` without a send.
    pub request_pending: bool,
}

/// Claims the due rows and, in the same statement, counts the attempt and
/// moves `next_attempt_at` to `now + LEASE`, so a crash before the outcome
/// lands makes the row due again later.
pub async fn claim(pool: &PgPool, now: NaiveDateTime, limit: i64) -> sqlx::Result<Vec<Claimed>> {
    let lease_until = now + chrono::Duration::from_std(LEASE).expect("LEASE fits");
    type Row = (
        i64,
        i64,
        i32,
        String,
        String,
        Option<String>,
        String,
        String,
    );
    let rows: Vec<Row> = sqlx::query_as(
        "WITH due AS ( \
             SELECT o.id, o.request_id FROM shop.item_notify_outbox o \
             WHERE o.status = 'pending' AND o.next_attempt_at <= $1 \
             ORDER BY o.next_attempt_at, o.id LIMIT $2 FOR UPDATE SKIP LOCKED \
         ) \
         UPDATE shop.item_notify_outbox o \
         SET attempts = o.attempts + 1, next_attempt_at = $3, updated_at = $1 \
         FROM due JOIN shop.item_notify_requests r ON r.id = due.request_id \
         WHERE o.id = due.id \
         RETURNING o.id, o.request_id, o.attempts, o.contract_address, o.item_id, o.item_name, r.email, r.status",
    )
    .bind(now)
    .bind(limit)
    .bind(lease_until)
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(
            |(id, request_id, attempts, contract_address, item_id, item_name, email, status)| {
                Claimed {
                    id,
                    request_id,
                    attempts,
                    contract_address,
                    item_id,
                    item_name,
                    email,
                    request_pending: status == "pending",
                }
            },
        )
        .collect())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    Sent(Option<String>),
    Skipped(String),
    Failed(String),
    Retry { after: Duration, error: String },
}

/// The state transition for one claimed row's send result.
pub fn outcome_of(claimed: &Claimed, result: Result<Option<String>, SendFailure>) -> Outcome {
    match result {
        Ok(message_id) => Outcome::Sent(message_id),
        Err(SendFailure::NotConfigured) => {
            Outcome::Skipped("email delivery not configured".to_string())
        }
        Err(SendFailure::Permanent(e)) => Outcome::Failed(e),
        Err(SendFailure::Transient(e)) if claimed.attempts >= MAX_ATTEMPTS => {
            Outcome::Failed(format!("{e} (after {} attempts)", claimed.attempts))
        }
        Err(SendFailure::Transient(e)) => Outcome::Retry {
            after: backoff(claimed.attempts),
            error: e,
        },
    }
}

/// Records an outcome. A sent row flips its request to `notified` in the
/// same statement, guarded on `status = 'pending'`: a concurrent or
/// duplicate delivery can never re-notify a row.
pub async fn record(
    pool: &PgPool,
    claimed: &Claimed,
    now: NaiveDateTime,
    outcome: &Outcome,
) -> sqlx::Result<()> {
    match outcome {
        Outcome::Sent(message_id) => {
            sqlx::query(
                "WITH o AS ( \
                     UPDATE shop.item_notify_outbox SET status = 'sent', sent_at = $2, provider_message_id = $3, last_error = NULL, updated_at = $2 \
                     WHERE id = $1 RETURNING request_id \
                 ) \
                 UPDATE shop.item_notify_requests r SET status = 'notified', notified_at = $2, updated_at = $2 \
                 FROM o WHERE r.id = o.request_id AND r.status = 'pending'",
            )
            .bind(claimed.id)
            .bind(now)
            .bind(message_id)
            .execute(pool)
            .await?;
        }
        Outcome::Skipped(reason) | Outcome::Failed(reason) => {
            let state = if matches!(outcome, Outcome::Skipped(_)) {
                "skipped"
            } else {
                "failed"
            };
            sqlx::query("UPDATE shop.item_notify_outbox SET status = $2, last_error = $3, updated_at = $4 WHERE id = $1")
                .bind(claimed.id)
                .bind(state)
                .bind(reason)
                .bind(now)
                .execute(pool)
                .await?;
        }
        Outcome::Retry { after, error } => {
            let next = now + chrono::Duration::from_std(*after).expect("backoff fits");
            sqlx::query("UPDATE shop.item_notify_outbox SET next_attempt_at = $2, last_error = $3, updated_at = $4 WHERE id = $1")
                .bind(claimed.id)
                .bind(next)
                .bind(error)
                .bind(now)
                .execute(pool)
                .await?;
        }
    }
    Ok(())
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct PassSummary {
    pub claimed: usize,
    pub sent: usize,
    pub skipped: usize,
    pub failed: usize,
    pub retried: usize,
}

/// One pass: claim, send each outside any transaction, record. A row whose
/// request is no longer pending (unsubscribed, or notified by another
/// delivery) is `skipped` without a send.
pub async fn run_pass(
    pool: &PgPool,
    mailer: &dyn AvailabilityMailer,
    shop_base_url: &str,
) -> anyhow::Result<PassSummary> {
    let claimed = claim(pool, now_utc(), BATCH).await?;
    let mut summary = PassSummary {
        claimed: claimed.len(),
        ..Default::default()
    };
    let configured = mailer.is_configured();
    for row in &claimed {
        let outcome = if !row.request_pending {
            Outcome::Skipped("the request is no longer pending".to_string())
        } else if !configured {
            outcome_of(row, Err(SendFailure::NotConfigured))
        } else {
            let url = item_url(shop_base_url, &row.contract_address, &row.item_id);
            let opt_out = unsubscribe_url(shop_base_url, &row.contract_address, &row.item_id);
            let result = mailer
                .send_item_available(&row.email, row.item_name.as_deref(), &url, &opt_out, row.id)
                .await;
            outcome_of(row, result)
        };
        match &outcome {
            Outcome::Sent(_) => summary.sent += 1,
            Outcome::Skipped(_) => summary.skipped += 1,
            Outcome::Failed(_) => summary.failed += 1,
            Outcome::Retry { .. } => summary.retried += 1,
        }
        if let Outcome::Failed(e) = &outcome {
            // The recipient is PII: the row id and the item identify the failure.
            tracing::warn!(outbox_id = row.id, contract = %row.contract_address, item_id = %row.item_id, error = %e, "availability alert failed");
        }
        record(pool, row, now_utc(), &outcome).await?;
    }
    Ok(summary)
}
