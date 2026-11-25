// from: credits-server@d9c6a261f:src/logic/iap-mint.ts (mintIapCreditWithCapCheck, mintIapUsdTopUp), src/logic/iap-refund.ts (processRefund, refundUsd), src/logic/iap-quote.ts (evaluateIapPurchase), src/logic/iap-product-catalog.ts (IAP_PRODUCT_CATALOG_USD)
//! App Store purchases of Credits (`credits.iap_transactions`,
//! `credits.iap_account_tokens`, `credits.iap_notifications`,
//! `credits.iap_refund_decisions`).
//!
//! A purchase is credited from Apple's signed transaction only: the product
//! decides the Credits (`IAP_PRODUCT_CATALOG_USD`, product id to whole
//! Credits per unit, times the signed quantity), never the client. As
//! upstream, the variable names the USD rail; its values are Credits. One
//! transaction id credits once, whichever path delivers it first (the
//! server notification, the device's `/iap/verify`, an operator), and a
//! transaction Apple revoked or refunded is never credited: a revoked one
//! seen first is recorded `refunded`. The per-wallet caps
//! (`IAP_WALLET_TOTAL_LIMIT_CENTS`, `IAP_WALLET_DAILY_LIMIT_CENTS`, 0
//! uncapped) and the node's daily ceiling (`IAP_DAILY_LIMIT_CENTS`, 0
//! closed) are checked under one advisory lock with the insert, over the
//! last 24 hours, counting purchases credited here or imported; an
//! operator credit is held to the node's ceiling only.
//!
//! A paid purchase that is not credited (a cap, the ceiling, an unknown
//! product or quantity, an account token naming no wallet) is kept as a
//! `held` row an operator lists (`held_iap`); the next delivery or an
//! operator credit re-checks it and credits it, with the product and
//! quantity Apple signed (an operator credit naming others is refused).
//!
//! A refund charges the purchase's Credits back, or Apple's
//! `revocationPercentage` share of them, from the wallet in the refund's
//! transaction (`Credits::revoke_in_tx`, floored at zero, the wallet
//! blocked when the balance net of live pledges cannot cover it). When it
//! cannot, the wallet's newest unexpired signed credits not queued yet,
//! covering what they pledge beyond the balance, are queued for
//! `revokeCredits` on chain (`credits.revocations`). A purchase imported
//! from credits-server is charged back the same way; it keeps its
//! `imported` status and `refunded_at` marks the refund. A refund reference
//! (Apple's notification uuid, an operator's refund id) applies once, even
//! after a reversal. A transaction is charged back once, as credits-server
//! does: a refund with another reference of a transaction already refunded
//! is recorded and logged, not charged back again. A refund of a
//! transaction never credited, or held, leaves a `refunded` row that keeps
//! it from being credited later; an operator refund of an unknown id
//! records nothing. A refund Apple reverses (`REFUND_REVERSED`) grants back
//! what was charged back, withdraws its revocations not sent yet while the
//! balance covers the credits left unqueued, lifts a block it raised once
//! the balance covers the live pledges, or turns the `refunded` row of a
//! purchase never credited back into a `held` one, in its own transaction,
//! before crediting it, so a failed credit leaves the purchase held.
//!
//! The paths that write both lock the wallet row before the transaction's
//! row, as an operator block and a card reversal do.
//!
//! A server notification's `notificationUUID` is recorded in
//! `credits.iap_notifications` in the transaction of its effects, so a
//! redelivery applies nothing. A notification Apple should retry (a held
//! purchase of an unknown product or over the node's ceiling) and a reversal
//! whose purchase is credited next, in its own transaction, are recorded by
//! that credit instead.
//!
//! Apple's latest refund decision per transaction (a `REFUND` or a
//! `REFUND_REVERSED`, with its `signedDate`) is kept in
//! `credits.iap_refund_decisions`, written under the transaction lock;
//! Apple's is written even when the purchase is not refunded or has no row
//! yet. Decisions are ordered by `signedDate`, then by notification uuid
//! (bytewise), so two signed in the same millisecond end the same in either
//! delivery order. A refund or reversal ordered before the recorded decision
//! is stale: it is recorded under its uuid and applies nothing, so a reversal
//! delivered before its refund keeps that refund from charging the wallet.
//! An operator refund is not signed by Apple and records nothing; it is
//! refused while the latest decision is a reversal unless the operator
//! overrides it. A revoked transaction signed before a recorded reversal
//! applies nothing.
//! An operator reversal (`/admin/iap/reversals`) undoes a refund as Apple's
//! does, for a `REFUND_REVERSED` too old to deliver: it names that
//! reversal's `signedDate`, is ordered against the recorded decision like
//! Apple's and is recorded as the latest decision; one with no refund to
//! undo is refused and records nothing. A refund is granted back once,
//! keyed on the refund it undoes, whichever reversal applies it.
//!
//! Every operator credit, refund and reversal it applies is audited with the
//! admin's `audit_actor_description()` as actor and `"source": "operator"`:
//! a refund of a held purchase as `iap.refund`, a charge-back of nothing as a
//! zero `revoke`, a reversal that holds the purchase again as `iap.hold`.
//! Apple's credit, refund charge-back and reversal are audited as actor
//! `apple` with `"source": "apple"`. A refused action records nothing.
//!
//! The account token is derived from the wallet address, so a purchase is
//! credited to the wallet its token names whichever device bought it, and
//! its refund charges that wallet back, as credits-server does.

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use serde_json::json;
use sqlx::Row;

use super::admin_store::GrantRequest;
use super::apple::{Environment, Verifier};
use super::errors::ApiError;
use super::settlement::enqueue_revocation;
use super::store::Credits;
use crate::types::UsdCents;

pub const IAP_ACTOR: &str = "apple";
/// `refund_ref` of an upstream purchase the import kept without a wallet.
pub const UPSTREAM_REFUND_REF: &str = "credits-server";
/// The most units one signed transaction is credited for.
pub const MAX_QUANTITY: i64 = 10;
const DAILY_LOCK: &str = "apple-iap-daily";

/// The raw variables (`APPLE_*`, `IAP_*`).
#[derive(Clone, Default)]
pub struct RawSettings {
    /// `APPLE_BUNDLE_ID`: unset, the App Store routes answer 503.
    pub bundle_id: Option<String>,
    /// `APPLE_APP_APPLE_ID`: required in production.
    pub app_apple_id: Option<i64>,
    /// `APPLE_ENVIRONMENT`: production or sandbox, required with `APPLE_BUNDLE_ID`.
    pub environment: Option<String>,
    /// `APPLE_ROOT_CERTS_DIR`: Apple's root certificates, DER `.cer`/`.der` files.
    pub root_certs_dir: Option<String>,
    /// `IAP_PRODUCT_CATALOG_USD`: `{"<productId>": <whole Credits>}`.
    pub catalog: Option<String>,
    /// `IAP_WALLET_TOTAL_LIMIT_CENTS` (0: uncapped).
    pub wallet_total_limit_cents: i64,
    /// `IAP_WALLET_DAILY_LIMIT_CENTS` (0: uncapped).
    pub wallet_daily_limit_cents: i64,
    /// `IAP_DAILY_LIMIT_CENTS`: the node's ceiling (0: closed); required with `APPLE_BUNDLE_ID`.
    pub daily_limit_cents: Option<i64>,
}

#[derive(Clone)]
pub struct Iap {
    pub verifier: Verifier,
    /// Product id to whole Credits per unit.
    pub catalog: HashMap<String, i64>,
    pub limits: Limits,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct Limits {
    pub wallet_total_cents: i64,
    pub wallet_daily_cents: i64,
    pub daily_cents: i64,
}

impl Iap {
    /// `None` without `APPLE_BUNDLE_ID`; a partial configuration refuses to start.
    pub fn parse(raw: RawSettings) -> anyhow::Result<Option<Arc<Self>>> {
        let Some(bundle_id) = raw.bundle_id.filter(|s| !s.trim().is_empty()) else {
            return Ok(None);
        };
        let environment = Environment::parse(raw.environment.as_deref())?;
        let dir = raw
            .root_certs_dir
            .filter(|s| !s.trim().is_empty())
            .ok_or_else(|| {
                anyhow::anyhow!("APPLE_ROOT_CERTS_DIR is required with APPLE_BUNDLE_ID")
            })?;
        let roots = load_roots(Path::new(dir.trim()))?;
        let verifier = Verifier::new(roots, bundle_id, raw.app_apple_id, environment)?;
        let catalog = parse_catalog(raw.catalog.as_deref().unwrap_or("{}"))?;
        if catalog.is_empty() {
            tracing::warn!("IAP_PRODUCT_CATALOG_USD is empty: every App Store purchase is held");
        }
        let daily_cents = raw.daily_limit_cents.ok_or_else(|| {
            anyhow::anyhow!("IAP_DAILY_LIMIT_CENTS is required with APPLE_BUNDLE_ID")
        })?;
        if daily_cents < 0 || raw.wallet_total_limit_cents < 0 || raw.wallet_daily_limit_cents < 0 {
            anyhow::bail!("the IAP_*_LIMIT_CENTS values must not be negative");
        }
        Ok(Some(Arc::new(Self {
            verifier,
            catalog,
            limits: Limits {
                wallet_total_cents: raw.wallet_total_limit_cents,
                wallet_daily_cents: raw.wallet_daily_limit_cents,
                daily_cents,
            },
        })))
    }

    /// Whole Credits per unit of `product_id`.
    pub fn credits_per_unit(&self, product_id: &str) -> Option<i64> {
        self.catalog.get(product_id).copied()
    }
}

fn load_roots(dir: &Path) -> anyhow::Result<Vec<Vec<u8>>> {
    let mut roots = Vec::new();
    for entry in std::fs::read_dir(dir)
        .map_err(|e| anyhow::anyhow!("APPLE_ROOT_CERTS_DIR cannot be read: {e}"))?
    {
        let path = entry?.path();
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .map(str::to_ascii_lowercase);
        if matches!(ext.as_deref(), Some("cer" | "der")) {
            roots.push(std::fs::read(&path)?);
        }
    }
    Ok(roots)
}

pub fn parse_catalog(raw: &str) -> anyhow::Result<HashMap<String, i64>> {
    let parsed: HashMap<String, serde_json::Value> = serde_json::from_str(raw)
        .map_err(|_| anyhow::anyhow!("IAP_PRODUCT_CATALOG_USD must be a JSON object"))?;
    parsed
        .into_iter()
        .map(|(product, credits)| match credits.as_i64() {
            // Bounded so the most units one transaction carries still fit in cents.
            Some(c)
                if c > 0
                    && !product.is_empty()
                    && c.checked_mul(MAX_QUANTITY)
                        .and_then(UsdCents::from_credits)
                        .is_some() =>
            {
                Ok((product, c))
            }
            _ => anyhow::bail!(
                "IAP_PRODUCT_CATALOG_USD maps each product id to a positive whole number of Credits"
            ),
        })
        .collect()
}

/// Who an app store write is audited as.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Actor<'a> {
    Apple,
    /// An admin, by its `audit_actor_description()`.
    Operator(&'a str),
}

impl<'a> Actor<'a> {
    /// The audit and ledger actor.
    fn name(self) -> &'a str {
        match self {
            Self::Apple => IAP_ACTOR,
            Self::Operator(actor) => actor,
        }
    }

    /// The audit detail's `source`.
    fn source(self) -> &'static str {
        match self {
            Self::Apple => "apple",
            Self::Operator(_) => "operator",
        }
    }
}

/// Which path credited a transaction (`credits.iap_transactions.credited_by`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Channel<'a> {
    Notification,
    Device,
    /// An admin, audited as its `audit_actor_description()`.
    Operator(&'a str),
}

impl<'a> Channel<'a> {
    fn as_str(self) -> &'static str {
        match self {
            Self::Notification => "notification",
            Self::Device => "device",
            Self::Operator(_) => "operator",
        }
    }

    fn actor(self) -> Actor<'a> {
        match self {
            Self::Operator(actor) => Actor::Operator(actor),
            Self::Notification | Self::Device => Actor::Apple,
        }
    }
}

/// Who refunds a transaction.
#[derive(Debug, Clone, Copy)]
pub enum Refunder<'a> {
    /// Apple's `REFUND`, recorded with its effects and ordered as a decision.
    Apple(Notice<'a>),
    /// An admin, audited as `actor`. It records no decision and is refused
    /// while Apple's latest decision is a reversal, unless `override_reversal`.
    Operator {
        actor: &'a str,
        override_reversal: bool,
    },
}

impl<'a> Refunder<'a> {
    fn notification(self) -> Option<Notice<'a>> {
        match self {
            Self::Apple(n) => Some(n),
            Self::Operator { .. } => None,
        }
    }

    fn actor(self) -> Actor<'a> {
        match self {
            Self::Apple(_) => Actor::Apple,
            Self::Operator { actor, .. } => Actor::Operator(actor),
        }
    }
}

/// The server notification behind a write, recorded with its effects.
#[derive(Debug, Clone, Copy)]
pub struct Notice<'a> {
    pub uuid: &'a str,
    /// `signedDate`, in milliseconds since the epoch.
    pub signed_date_ms: i64,
}

/// Who reverses a refund. Both record a reversal they apply as the
/// transaction's latest decision; an operator's with no refund to undo
/// records nothing. Only Apple's notification is recorded with its effects.
#[derive(Debug, Clone, Copy)]
pub enum Reverser<'a> {
    Apple(Notice<'a>),
    /// An operator applying a `REFUND_REVERSED` too old to deliver, under
    /// its own id and Apple's `signedDate`, audited as `actor`.
    Operator {
        notice: Notice<'a>,
        actor: &'a str,
    },
}

impl<'a> Reverser<'a> {
    fn notice(self) -> Notice<'a> {
        match self {
            Self::Apple(n) | Self::Operator { notice: n, .. } => n,
        }
    }

    /// The notification this reversal records with its effects (Apple's only).
    fn notification(self) -> Option<Notice<'a>> {
        match self {
            Self::Apple(n) => Some(n),
            Self::Operator { .. } => None,
        }
    }

    fn actor(self) -> Actor<'a> {
        match self {
            Self::Apple(_) => Actor::Apple,
            Self::Operator { actor, .. } => Actor::Operator(actor),
        }
    }
}

pub struct Purchase<'a> {
    /// `None`: the transaction's account token names no wallet.
    pub address: Option<&'a str>,
    pub app_account_token: Option<&'a str>,
    pub transaction_id: &'a str,
    pub original_transaction_id: Option<&'a str>,
    pub product_id: &'a str,
    /// `None` (an operator credit only): the held row's quantity, else 1.
    pub quantity: Option<i64>,
    pub revoked: bool,
    pub channel: Channel<'a>,
    /// The notification that delivered it, if any.
    pub notice: Option<Notice<'a>>,
    /// The transaction's own `signedDate`, in milliseconds since the epoch.
    pub signed_date_ms: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CapReason {
    TotalLimit,
    DailyLimit,
    ServiceDailyLimit,
}

impl CapReason {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::TotalLimit => "total_limit",
            Self::DailyLimit => "daily_limit",
            Self::ServiceDailyLimit => "service_daily_limit",
        }
    }
}

/// Why a paid purchase was not credited (`credits.iap_transactions.held_reason`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HoldReason {
    UnknownWallet,
    UnknownProduct,
    Quantity,
    Capped(CapReason),
    /// Apple reversed the refund of a purchase never credited.
    RefundReversed,
}

impl HoldReason {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::UnknownWallet => "unresolvable_app_account_token",
            Self::UnknownProduct => "unknown_product",
            Self::Quantity => "quantity_out_of_range",
            Self::Capped(reason) => reason.as_str(),
            Self::RefundReversed => "refund_reversed",
        }
    }

    /// A notification held for this reason is answered so Apple retries it.
    pub fn retried(&self) -> bool {
        matches!(
            self,
            Self::UnknownProduct | Self::Capped(CapReason::ServiceDailyLimit)
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CreditOutcome {
    /// Credited now, or earlier by another path (`already`), to `address`.
    Credited {
        address: String,
        credits: i64,
        already: bool,
    },
    /// Apple's signed transaction is revoked.
    Revoked,
    /// The transaction was refunded; it is never credited.
    Refunded,
    /// Not credited; a `held` row records it unless an operator asked.
    Held(HoldReason),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RefundOutcome {
    Reversed {
        address: String,
        removed: String,
        shortfall: String,
        /// Live signed credits queued for on-chain revocation.
        revocations: usize,
    },
    AlreadyRefunded,
    /// Signed before the transaction's latest refund decision (a refund or
    /// reversal), or an operator refund after Apple's reversal: applies nothing.
    Superseded,
    /// Not credited on this node: a `refunded` row now keeps it from being credited.
    NotCredited,
    /// A `held` purchase, now `refunded`, so it is never credited.
    HeldRefunded,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReinstateOutcome {
    /// The charged-back Credits were granted again.
    Reinstated {
        address: String,
        credits: String,
        /// Revocations this refund queued that were withdrawn before being sent.
        withdrawn: u64,
    },
    /// The purchase stands but is not credited here: no row, a held one, or
    /// a refund of a purchase never credited, now held. Credit it.
    Uncredited,
    /// No refund of this transaction to reverse.
    NotRefunded,
    /// An operator reversal of a transaction this node has no row for.
    Unknown,
    /// Signed before the transaction's latest refund decision: applies nothing.
    Superseded,
}

/// A paid purchase not credited yet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeldPurchase {
    pub transaction_id: String,
    pub product_id: String,
    pub quantity: Option<i64>,
    pub address: Option<String>,
    pub app_account_token: Option<String>,
    pub reason: String,
    pub created_at: chrono::NaiveDateTime,
}

/// What a wallet bought and may still buy (cents; `None` uncapped).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Totals {
    pub total_cents: i64,
    pub daily_cents: i64,
    pub service_daily_cents: i64,
}

impl Totals {
    pub fn decide(&self, limits: &Limits, requested_cents: i64) -> Option<CapReason> {
        if limits.wallet_total_cents > 0
            && self.total_cents.saturating_add(requested_cents) > limits.wallet_total_cents
        {
            return Some(CapReason::TotalLimit);
        }
        if limits.wallet_daily_cents > 0
            && self.daily_cents.saturating_add(requested_cents) > limits.wallet_daily_cents
        {
            return Some(CapReason::DailyLimit);
        }
        if self.service_daily_cents.saturating_add(requested_cents) > limits.daily_cents {
            return Some(CapReason::ServiceDailyLimit);
        }
        None
    }
}

async fn lock(tx: &mut sqlx::Transaction<'_, sqlx::Postgres>, key: &str) -> Result<(), ApiError> {
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1, 0))")
        .bind(key)
        .execute(&mut **tx)
        .await?;
    Ok(())
}

/// Locks the wallet behind a transaction's row first, so the App Store paths
/// take row locks in the same order as an operator block and a card reversal.
async fn lock_wallet_of(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    transaction_id: &str,
) -> Result<(), ApiError> {
    sqlx::query(
        "SELECT 1 FROM credits.wallets WHERE address = \
             (SELECT address FROM credits.iap_transactions WHERE transaction_id = $1) FOR UPDATE",
    )
    .bind(transaction_id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

/// Records `notice` in this transaction; a uuid another transaction recorded
/// (a concurrent delivery) answers 409, so Apple retries and then sees it applied.
async fn claim(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    notice: Option<Notice<'_>>,
) -> Result<(), ApiError> {
    let Some(n) = notice else { return Ok(()) };
    let signed_at = signed_at(n)?;
    let inserted = sqlx::query(
        "INSERT INTO credits.iap_notifications (notification_uuid, signed_at) VALUES ($1, $2) \
         ON CONFLICT (notification_uuid) DO NOTHING",
    )
    .bind(n.uuid)
    .bind(signed_at)
    .execute(&mut **tx)
    .await?
    .rows_affected();
    if inserted == 0 {
        return Err(ApiError::conflict("this notification is already applied"));
    }
    Ok(())
}

fn signed_at(n: Notice<'_>) -> Result<chrono::NaiveDateTime, ApiError> {
    Ok(chrono::DateTime::from_timestamp_millis(n.signed_date_ms)
        .ok_or_else(|| ApiError::bad_request("signedDate out of range"))?
        .naive_utc())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Decision {
    Refund,
    Reversal,
}

impl Decision {
    fn as_str(self) -> &'static str {
        match self {
            Self::Refund => "refund",
            Self::Reversal => "reversal",
        }
    }
}

/// The transaction's latest recorded refund decision and its `signedDate`.
async fn latest_decision(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    transaction_id: &str,
) -> Result<Option<(Decision, chrono::NaiveDateTime)>, ApiError> {
    let row: Option<(String, chrono::NaiveDateTime)> = sqlx::query_as(
        "SELECT decision, signed_at FROM credits.iap_refund_decisions WHERE transaction_id = $1",
    )
    .bind(transaction_id)
    .fetch_optional(&mut **tx)
    .await?;
    Ok(row.map(|(d, at)| {
        let d = if d == Decision::Reversal.as_str() {
            Decision::Reversal
        } else {
            Decision::Refund
        };
        (d, at)
    }))
}

/// Records `notice` as the transaction's latest refund decision; false when
/// one ordered after it is recorded, so this one is stale. Decisions order by
/// `signedDate`, then uuid bytewise; a retry of the recorded one is not stale.
/// An operator reversal passes Apple's `signedDate` under its own id; without
/// a notice (an operator refund) nothing is compared.
async fn record_decision(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    transaction_id: &str,
    decision: Decision,
    notice: Option<Notice<'_>>,
) -> Result<bool, ApiError> {
    let Some(n) = notice else { return Ok(true) };
    let written = sqlx::query(
        "INSERT INTO credits.iap_refund_decisions AS d \
             (transaction_id, decision, signed_at, notification_uuid) VALUES ($1, $2, $3, $4) \
         ON CONFLICT (transaction_id) DO UPDATE SET decision = EXCLUDED.decision, \
             signed_at = EXCLUDED.signed_at, notification_uuid = EXCLUDED.notification_uuid, \
             updated_at = timezone('UTC', now()) \
         WHERE (d.signed_at, d.notification_uuid COLLATE \"C\") \
               <= (EXCLUDED.signed_at, EXCLUDED.notification_uuid COLLATE \"C\")",
    )
    .bind(transaction_id)
    .bind(decision.as_str())
    .bind(signed_at(n)?)
    .bind(n.uuid)
    .execute(&mut **tx)
    .await?
    .rows_affected();
    Ok(written == 1)
}

/// Drops the claim of a notification whose effects commit without it.
async fn release(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    notice: Option<Notice<'_>>,
) -> Result<(), ApiError> {
    if let Some(n) = notice {
        sqlx::query("DELETE FROM credits.iap_notifications WHERE notification_uuid = $1")
            .bind(n.uuid)
            .execute(&mut **tx)
            .await?;
    }
    Ok(())
}

fn grant_key(transaction_id: &str) -> String {
    format!("apple-iap:{transaction_id}")
}

/// `credits.revocations.reason` of the revocations one refund queued.
fn revocation_reason(transaction_id: &str) -> String {
    format!("reversed:{}", grant_key(transaction_id))
}

impl Credits {
    /// Purchases credited by this node or imported, refunded or not; held ones are not counted.
    pub async fn iap_totals<'e, E: sqlx::PgExecutor<'e>>(
        executor: E,
        address: &str,
    ) -> Result<Totals, ApiError> {
        let row = sqlx::query(
            "SELECT COALESCE(SUM(amount_cents) FILTER (WHERE address = $1), 0)::bigint AS total, \
                    COALESCE(SUM(amount_cents) FILTER (WHERE address = $1 AND created_at > timezone('UTC', now()) - interval '24 hours'), 0)::bigint AS daily, \
                    COALESCE(SUM(amount_cents) FILTER (WHERE created_at > timezone('UTC', now()) - interval '24 hours'), 0)::bigint AS service \
             FROM credits.iap_transactions \
             WHERE address IS NOT NULL AND (credited_by IS NOT NULL OR status = 'imported') \
               AND (address = $1 OR created_at > timezone('UTC', now()) - interval '24 hours')",
        )
        .bind(address)
        .fetch_one(executor)
        .await?;
        Ok(Totals {
            total_cents: row.get("total"),
            daily_cents: row.get("daily"),
            service_daily_cents: row.get("service"),
        })
    }

    /// Whether a notification's effects were recorded.
    pub async fn iap_notification_applied(&self, uuid: &str) -> Result<bool, ApiError> {
        Ok(sqlx::query_scalar(
            "SELECT EXISTS (SELECT 1 FROM credits.iap_notifications WHERE notification_uuid = $1)",
        )
        .bind(uuid)
        .fetch_one(&self.pool)
        .await?)
    }

    /// Credits one App Store transaction once; see the module docs.
    pub async fn credit_iap(&self, iap: &Iap, p: Purchase<'_>) -> Result<CreditOutcome, ApiError> {
        let mut tx = self.pool.begin().await?;
        lock(&mut tx, &grant_key(p.transaction_id)).await?;
        claim(&mut tx, p.notice).await?;
        let existing = sqlx::query(
            "SELECT status, address, floor(credits)::bigint AS credits, product_id, quantity, \
                    refunded_at IS NOT NULL AS refunded \
             FROM credits.iap_transactions WHERE transaction_id = $1",
        )
        .bind(p.transaction_id)
        .fetch_optional(&mut *tx)
        .await?;
        let mut held_for = None;
        let mut held_as: Option<(String, Option<i64>)> = None;
        if let Some(row) = existing {
            let status: String = row.get("status");
            let address: Option<String> = row.get("address");
            let settled = match (status.as_str(), address) {
                ("paid" | "imported", Some(_)) if p.revoked => Some(CreditOutcome::Revoked),
                ("imported", Some(_)) if row.get::<bool, _>("refunded") => {
                    Some(CreditOutcome::Refunded)
                }
                ("paid" | "imported", Some(address)) => Some(CreditOutcome::Credited {
                    address,
                    credits: row.get::<Option<i64>, _>("credits").unwrap_or(0),
                    already: true,
                }),
                ("held", address) => {
                    held_for = address;
                    held_as = Some((row.get("product_id"), row.get("quantity")));
                    None
                }
                _ => Some(CreditOutcome::Refunded),
            };
            if let Some(outcome) = settled {
                // Commits the notification's record only.
                tx.commit().await?;
                return Ok(outcome);
            }
        }
        if p.revoked {
            // Apple reversed the refund after signing this: it changes nothing.
            if let Some((Decision::Reversal, at)) =
                latest_decision(&mut tx, p.transaction_id).await?
                && p.signed_date_ms.is_none_or(|ms| {
                    chrono::DateTime::from_timestamp_millis(ms).is_none_or(|t| t.naive_utc() <= at)
                })
            {
                tx.commit().await?;
                return Ok(CreditOutcome::Revoked);
            }
            // Recorded, so a later delivery of the same purchase is not credited either.
            sqlx::query(
                "INSERT INTO credits.iap_transactions AS t \
                     (transaction_id, original_transaction_id, product_id, status, refund_ref, refund_reason, refunded_at) \
                 VALUES ($1, $2, $3, 'refunded', 'apple', 'revoked when received', timezone('UTC', now())) \
                 ON CONFLICT (transaction_id) DO UPDATE SET status = 'refunded', held_reason = NULL, \
                     refund_ref = EXCLUDED.refund_ref, refund_reason = EXCLUDED.refund_reason, \
                     refunded_at = EXCLUDED.refunded_at, updated_at = timezone('UTC', now())",
            )
            .bind(p.transaction_id)
            .bind(p.original_transaction_id)
            .bind(p.product_id)
            .execute(&mut *tx)
            .await?;
            tx.commit().await?;
            return Ok(CreditOutcome::Revoked);
        }
        let address = match (p.address, held_for) {
            (Some(a), Some(h)) if a != h => {
                return Err(ApiError::conflict(
                    "this transaction is held for another wallet",
                ));
            }
            (Some(a), _) => a.to_string(),
            (None, Some(h)) => h,
            (None, None) => return hold(tx, &p, None, None, HoldReason::UnknownWallet).await,
        };
        // An operator credits a held purchase as Apple signed it, never as the request names it.
        if let (Channel::Operator(_), Some((product, quantity))) = (p.channel, &held_as)
            && (product != p.product_id || p.quantity.is_some_and(|q| Some(q) != *quantity))
        {
            return Err(ApiError::conflict(format!(
                "this transaction is held for {product} with quantity {}",
                quantity.map_or("unknown".into(), |q| q.to_string())
            )));
        }
        let quantity = p.quantity.or(held_as.and_then(|(_, q)| q)).unwrap_or(1);
        let Some(per_unit) = iap.credits_per_unit(p.product_id) else {
            return hold(tx, &p, Some(&address), None, HoldReason::UnknownProduct).await;
        };
        if !(1..=MAX_QUANTITY).contains(&quantity) {
            return hold(tx, &p, Some(&address), None, HoldReason::Quantity).await;
        }
        let (credits, cents) = per_unit
            .checked_mul(quantity)
            .and_then(|c| Some((c, UsdCents::from_credits(c)?.get())))
            .ok_or_else(|| ApiError::bad_request("credits out of range"))?;

        // An operator credit is how a purchase refused by a wallet cap is reconciled.
        let limits = match p.channel {
            Channel::Operator(_) => Limits {
                wallet_total_cents: 0,
                wallet_daily_cents: 0,
                ..iap.limits
            },
            _ => iap.limits,
        };
        lock(&mut tx, DAILY_LOCK).await?;
        let totals = Self::iap_totals(&mut *tx, &address).await?;
        if let Some(reason) = totals.decide(&limits, cents) {
            let value = Some((credits, cents));
            return hold(tx, &p, Some(&address), value, HoldReason::Capped(reason)).await;
        }

        // A held row turns `paid` in place: its account token, original id and
        // refund references (a reversal's redelivery applies once) stay. Its
        // `created_at` becomes the credit's time, which the caps count.
        let written = sqlx::query(
            "INSERT INTO credits.iap_transactions AS t \
                 (transaction_id, original_transaction_id, product_id, address, credits, amount_cents, quantity, status, credited_by) \
             VALUES ($1, $2, $3, $4, $5::numeric, $6, $7, 'paid', $8) \
             ON CONFLICT (transaction_id) DO UPDATE SET \
                 original_transaction_id = COALESCE(EXCLUDED.original_transaction_id, t.original_transaction_id), \
                 product_id = EXCLUDED.product_id, address = EXCLUDED.address, credits = EXCLUDED.credits, \
                 amount_cents = EXCLUDED.amount_cents, quantity = EXCLUDED.quantity, status = 'paid', \
                 credited_by = EXCLUDED.credited_by, held_reason = NULL, \
                 created_at = timezone('UTC', now()), updated_at = timezone('UTC', now()) \
             WHERE t.status = 'held'",
        )
        .bind(p.transaction_id)
        .bind(p.original_transaction_id)
        .bind(p.product_id)
        .bind(&address)
        .bind(credits.to_string())
        .bind(cents)
        .bind(quantity)
        .bind(p.channel.as_str())
        .execute(&mut *tx)
        .await?
        .rows_affected();
        if written != 1 {
            return Err(ApiError::conflict("this transaction is already settled"));
        }
        let detail = json!({
            "source": p.channel.actor().source(),
            "transactionId": p.transaction_id,
            "productId": p.product_id,
            "quantity": quantity,
            "creditedBy": p.channel.as_str(),
        });
        let key = grant_key(p.transaction_id);
        self.admin_grant_credits_in_tx(
            &mut tx,
            GrantRequest {
                address: &address,
                amount: &credits.to_string(),
                kind: "purchase",
                reason: Some("app store purchase"),
                actor: Some(p.channel.actor().name()),
                idempotency_key: Some(&key),
                detail: &detail,
                studio_name: None,
            },
        )
        .await?;
        tx.commit().await?;
        Ok(CreditOutcome::Credited {
            address,
            credits,
            already: false,
        })
    }

    /// Reverses one App Store transaction once; see the module docs.
    /// `product_id` is `None` for an operator refund, which records nothing
    /// for a transaction never seen. `percentage` is the refunded share in
    /// thousandths of a percent; `None` refunds all of it. An operator refund
    /// while Apple's latest decision is a reversal answers `Superseded` unless
    /// it overrides the reversal.
    pub async fn refund_iap(
        &self,
        transaction_id: &str,
        product_id: Option<&str>,
        refund_ref: &str,
        reason: &str,
        percentage: Option<i64>,
        by: Refunder<'_>,
    ) -> Result<RefundOutcome, ApiError> {
        let notice = by.notification();
        let mut tx = self.pool.begin().await?;
        lock(&mut tx, &grant_key(transaction_id)).await?;
        lock_wallet_of(&mut tx, transaction_id).await?;
        claim(&mut tx, notice).await?;
        let row = sqlx::query(
            "SELECT status, address, refunded_at IS NOT NULL AS refunded, $2 = ANY(refund_refs) AS applied, \
                    credited_by IS NOT NULL OR status = 'imported' AS credited \
             FROM credits.iap_transactions WHERE transaction_id = $1 FOR UPDATE",
        )
        .bind(transaction_id)
        .bind(refund_ref)
        .fetch_optional(&mut *tx)
        .await?;
        // A redelivered refund, even after its reversal, applies once.
        if row.as_ref().is_some_and(|r| r.get::<bool, _>("applied")) {
            tx.commit().await?;
            return Ok(RefundOutcome::AlreadyRefunded);
        }
        if !record_decision(&mut tx, transaction_id, Decision::Refund, notice).await? {
            tracing::warn!(transaction_id = %transaction_id, refund_ref = %refund_ref, "app store refund signed before the transaction's latest refund decision: not applied");
            tx.commit().await?;
            return Ok(RefundOutcome::Superseded);
        }
        if let Refunder::Operator {
            override_reversal: false,
            ..
        } = by
            && matches!(
                latest_decision(&mut tx, transaction_id).await?,
                Some((Decision::Reversal, _))
            )
        {
            tx.commit().await?;
            return Ok(RefundOutcome::Superseded);
        }
        let Some(row) = row else {
            if let Some(product_id) = product_id {
                sqlx::query(
                    "INSERT INTO credits.iap_transactions \
                         (transaction_id, product_id, status, refund_ref, refund_reason, refunded_at, refund_refs) \
                     VALUES ($1, $2, 'refunded', $3, $4, timezone('UTC', now()), ARRAY[$3])",
                )
                .bind(transaction_id)
                .bind(product_id)
                .bind(refund_ref)
                .bind(reason)
                .execute(&mut *tx)
                .await?;
            }
            tx.commit().await?;
            return Ok(RefundOutcome::NotCredited);
        };
        let status: String = row.get("status");
        let address: Option<String> = row.get("address");
        let refunded: bool = row.get("refunded");
        let credited: bool = row.get("credited");
        match (status.as_str(), address) {
            ("held", address) => {
                sqlx::query(
                    "UPDATE credits.iap_transactions \
                     SET status = 'refunded', held_reason = NULL, refund_ref = $2, refund_reason = $3, \
                         refund_refs = array_append(refund_refs, $2), \
                         refunded_at = timezone('UTC', now()), updated_at = timezone('UTC', now()) \
                     WHERE transaction_id = $1",
                )
                .bind(transaction_id)
                .bind(refund_ref)
                .bind(reason)
                .execute(&mut *tx)
                .await?;
                // Nothing was credited, so no revoke records the operator's refund.
                if let Refunder::Operator { actor, .. } = by {
                    let detail = json!({ "source": by.actor().source(), "transactionId": transaction_id, "refundRef": refund_ref, "reason": reason, "held": true });
                    Self::audit(
                        &mut *tx,
                        super::store::AuditEntry {
                            action: "iap.refund",
                            actor,
                            target: address.as_deref(),
                            detail: &detail,
                        },
                    )
                    .await?;
                }
                tx.commit().await?;
                Ok(RefundOutcome::HeldRefunded)
            }
            // Credited upstream: charged back like a purchase credited here; it stays `imported`.
            ("paid", Some(address)) | ("imported", Some(address)) if !refunded => {
                let charged: String = sqlx::query_scalar(
                    "UPDATE credits.iap_transactions \
                     SET status = CASE WHEN status = 'paid' THEN 'refunded' ELSE status END, \
                         revoked_credits = CASE WHEN $4::bigint IS NULL THEN credits \
                                                ELSE trim_scale(round(credits * $4::bigint / 100000, 2)) END, \
                         refunded_cents = CASE WHEN $4::bigint IS NULL THEN amount_cents \
                                               ELSE round(amount_cents * $4::bigint / 100000.0)::bigint END, \
                         refund_ref = $2, refund_reason = $3, refund_refs = array_append(refund_refs, $2), \
                         refunded_at = timezone('UTC', now()), updated_at = timezone('UTC', now()) \
                     WHERE transaction_id = $1 RETURNING revoked_credits::text",
                )
                .bind(transaction_id)
                .bind(refund_ref)
                .bind(reason)
                .bind(percentage)
                .fetch_one(&mut *tx)
                .await?;
                self.charge_back_iap(
                    tx,
                    transaction_id,
                    address,
                    &charged,
                    refund_ref,
                    reason,
                    by,
                )
                .await
            }
            // One refund per transaction, as credits-server: another reference
            // of a refunded credit is recorded, not charged back again.
            _ => {
                if credited {
                    tracing::warn!(transaction_id = %transaction_id, refund_ref = %refund_ref, "app store refund of a transaction already refunded under another reference: recorded, nothing charged back");
                }
                tx.commit().await?;
                Ok(RefundOutcome::AlreadyRefunded)
            }
        }
    }

    #[allow(
        clippy::too_many_arguments,
        reason = "the refunded row's fields, each used once"
    )]
    async fn charge_back_iap(
        &self,
        mut tx: sqlx::Transaction<'_, sqlx::Postgres>,
        transaction_id: &str,
        address: String,
        charged: &str,
        refund_ref: &str,
        reason: &str,
        by: Refunder<'_>,
    ) -> Result<RefundOutcome, ApiError> {
        if super::money::CreditAmount::parse_positive(charged).is_err() {
            // Nothing to revoke: the operator's refund is still recorded.
            if let Refunder::Operator { actor, .. } = by {
                let base = json!({
                    "source": by.actor().source(),
                    "transactionId": transaction_id,
                    "refundRef": refund_ref,
                });
                let tx_ref = grant_key(transaction_id);
                let detail = super::wallet::revoke_detail(&base, &tx_ref, "0", "0", "0", reason);
                Self::audit(
                    &mut *tx,
                    super::store::AuditEntry {
                        action: "revoke",
                        actor,
                        target: Some(&address),
                        detail: &detail,
                    },
                )
                .await?;
            }
            tx.commit().await?;
            return Ok(RefundOutcome::Reversed {
                address,
                removed: "0".into(),
                shortfall: "0".into(),
                revocations: 0,
            });
        }
        let detail = json!({ "source": by.actor().source(), "transactionId": transaction_id, "refundRef": refund_ref });
        let was_blocked: Option<bool> = sqlx::query_scalar(
            "SELECT is_blocked_for_claiming FROM credits.wallets WHERE address = $1 FOR UPDATE",
        )
        .bind(&address)
        .fetch_optional(&mut *tx)
        .await?;
        let revoked = self
            .revoke_in_tx(
                &mut tx,
                &address,
                charged,
                &grant_key(transaction_id),
                reason,
                &detail,
                by.actor().name(),
            )
            .await?;
        // What was actually removed: a reversed refund grants back this, not the shortfall.
        sqlx::query(
            "UPDATE credits.iap_transactions SET revoked_credits = $2::numeric, raised_block = $3 \
             WHERE transaction_id = $1",
        )
        .bind(transaction_id)
        .bind(&revoked.removed)
        .bind(revoked.uncovered && was_blocked != Some(true))
        .execute(&mut *tx)
        .await?;
        let revocations = if revoked.uncovered {
            revoke_live_credits_in_tx(
                &mut tx,
                &address,
                &revoked.uncovered_amount,
                &revocation_reason(transaction_id),
            )
            .await?
        } else {
            0
        };
        tx.commit().await?;
        if revoked.has_shortfall {
            tracing::error!(address = %address, transaction_id = %transaction_id, shortfall = %revoked.shortfall, "app store refund could not charge back every credit: the rest was already spent");
        }
        Ok(RefundOutcome::Reversed {
            address,
            removed: revoked.removed,
            shortfall: revoked.shortfall,
            revocations,
        })
    }

    /// Undoes a refund Apple reversed (`REFUND_REVERSED`): a credited (or
    /// imported) purchase gets back what its refund charged back, its
    /// revocations the job has not claimed yet are withdrawn while the
    /// balance covers the unexpired credits left unqueued (a claimed one may
    /// be on chain), and a block it raised, that no operator or later
    /// reversal took over, is lifted (and audited) once the balance covers
    /// the live pledges. A refund of a purchase never credited becomes a
    /// `held` row, committed before the caller credits it. Apple's reversal
    /// and an operator's are both ordered against, and recorded as, the
    /// transaction's latest decision; an operator reversal of an unknown or
    /// unrefunded transaction is refused and records nothing.
    pub async fn reinstate_iap(
        &self,
        transaction_id: &str,
        by: Reverser<'_>,
    ) -> Result<ReinstateOutcome, ApiError> {
        let notification = by.notification();
        let reversal_ref = by.notice().uuid;
        let mut tx = self.pool.begin().await?;
        lock(&mut tx, &grant_key(transaction_id)).await?;
        lock_wallet_of(&mut tx, transaction_id).await?;
        claim(&mut tx, notification).await?;
        let row = sqlx::query(
            "SELECT status, address, credited_by IS NOT NULL AS credited, revoked_credits::text AS revoked, \
                    raised_block, refunded_at IS NOT NULL AS refunded, refund_ref AS undone_refund_ref, \
                    refund_ref IS NOT DISTINCT FROM $2 AS upstream \
             FROM credits.iap_transactions WHERE transaction_id = $1 FOR UPDATE",
        )
        .bind(transaction_id)
        .bind(UPSTREAM_REFUND_REF)
        .fetch_optional(&mut *tx)
        .await?;
        let undo = Undo::of(row.as_ref());
        // An operator reversal that would change nothing records no decision,
        // so it never makes a later Apple refund stale. Apple's is recorded
        // either way: a reversal delivered before its refund must still order it.
        if let Reverser::Operator { .. } = by
            && !matches!(undo, Undo::Rehold { .. } | Undo::Grant { .. })
        {
            tx.rollback().await?;
            return Ok(match undo {
                Undo::Unknown => ReinstateOutcome::Unknown,
                _ => ReinstateOutcome::NotRefunded,
            });
        }
        // Signed before the latest decision (a refund or reversal): it applies nothing.
        if !record_decision(
            &mut tx,
            transaction_id,
            Decision::Reversal,
            Some(by.notice()),
        )
        .await?
        {
            tracing::warn!(transaction_id = %transaction_id, reversal_ref = %reversal_ref, "app store refund reversal signed before the transaction's latest refund decision: not applied");
            tx.commit().await?;
            return Ok(ReinstateOutcome::Superseded);
        }
        // The decision commits; the credit that follows records the notification.
        let (address, credits, raised_block, undone_refund_ref) = match undo {
            Undo::Unknown | Undo::Held => {
                release(&mut tx, notification).await?;
                tx.commit().await?;
                return Ok(ReinstateOutcome::Uncredited);
            }
            Undo::NotRefunded => {
                tx.commit().await?;
                return Ok(ReinstateOutcome::NotRefunded);
            }
            Undo::Rehold {
                address,
                undone_refund_ref,
            } => {
                // Held, not deleted: a failed credit after this commit leaves the purchase held.
                sqlx::query(
                    "UPDATE credits.iap_transactions \
                     SET status = 'held', held_reason = $2, refund_ref = NULL, refund_reason = NULL, \
                         refunded_at = NULL, updated_at = timezone('UTC', now()) \
                     WHERE transaction_id = $1",
                )
                .bind(transaction_id)
                .bind(HoldReason::RefundReversed.as_str())
                .execute(&mut *tx)
                .await?;
                if let Reverser::Operator { actor, .. } = by {
                    let detail = json!({
                        "source": by.actor().source(),
                        "transactionId": transaction_id,
                        "refundRef": undone_refund_ref,
                        "reversalRef": reversal_ref,
                        "held": true,
                    });
                    Self::audit(
                        &mut *tx,
                        super::store::AuditEntry {
                            action: "iap.hold",
                            actor,
                            target: address.as_deref(),
                            detail: &detail,
                        },
                    )
                    .await?;
                }
                release(&mut tx, notification).await?;
                tx.commit().await?;
                return Ok(ReinstateOutcome::Uncredited);
            }
            Undo::Grant {
                address,
                credits,
                raised_block,
                undone_refund_ref,
            } => (address, credits, raised_block, undone_refund_ref),
        };
        // One grant per refund undone, whichever reversal names it. Every
        // refund that sets `refunded_at` also sets `refund_ref`.
        let undone_refund_ref = undone_refund_ref.ok_or_else(|| {
            ApiError::Internal(format!(
                "refunded app store transaction {transaction_id} has no refund reference"
            ))
        })?;
        sqlx::query(
            "UPDATE credits.iap_transactions \
             SET status = CASE WHEN status = 'refunded' THEN 'paid' ELSE status END, revoked_credits = 0, refunded_cents = 0, refund_ref = NULL, refund_reason = NULL, \
                 refunded_at = NULL, raised_block = false, updated_at = timezone('UTC', now()) \
             WHERE transaction_id = $1",
        )
        .bind(transaction_id)
        .execute(&mut *tx)
        .await?;
        if super::money::CreditAmount::parse_positive(&credits).is_ok() {
            let key = format!(
                "{}:reinstated:{undone_refund_ref}",
                grant_key(transaction_id)
            );
            let detail = json!({ "source": by.actor().source(), "transactionId": transaction_id, "refundRef": undone_refund_ref, "reversalRef": reversal_ref });
            let granted = self
                .admin_grant_credits_in_tx(
                    &mut tx,
                    GrantRequest {
                        address: &address,
                        amount: &credits,
                        kind: "refund",
                        reason: Some("app store refund reversed"),
                        actor: Some(by.actor().name()),
                        idempotency_key: Some(&key),
                        detail: &detail,
                        studio_name: None,
                    },
                )
                .await?;
            // Unreachable unless the grant key was already used: `refund_iap` applies a
            // `refund_ref` once. Dropping `tx` on this error rolls back the UPDATE above.
            if granted.replayed {
                return Err(ApiError::Internal(format!(
                    "app store refund {undone_refund_ref} of {transaction_id} was already granted back"
                )));
            }
        }
        // Only unclaimed rows (a claimed one may be on chain), and only while
        // the balance covers every unexpired credit the withdrawal leaves unqueued.
        let withdrawn = sqlx::query(
            "DELETE FROM credits.revocations WHERE reason = $1 AND status = 'pending' AND attempts = 0 \
               AND COALESCE((SELECT available FROM credits.wallets WHERE address = $2), 0) * $3::numeric >= ( \
                   SELECT COALESCE(SUM(usd_cents), 0) FROM credits.authorizations a \
                   WHERE address = $2 AND status IN ('authorized', 'retired') \
                     AND expires_at > timezone('UTC', now()) \
                     AND NOT EXISTS (SELECT 1 FROM credits.revocations r WHERE r.credit_hash = a.credit_hash \
                                     AND NOT (r.reason = $1 AND r.status = 'pending' AND r.attempts = 0)))",
        )
        .bind(revocation_reason(transaction_id))
        .bind(&address)
        .bind(UsdCents::CENTS_PER_CREDIT)
        .execute(&mut *tx)
        .await?
        .rows_affected();
        if raised_block {
            let lifted = sqlx::query(
                "UPDATE credits.wallets SET is_blocked_for_claiming = false, updated_at = timezone('UTC', now()) \
                 WHERE address = $1 AND is_blocked_for_claiming AND available * $2::numeric >= ( \
                     SELECT COALESCE(SUM(usd_cents), 0) FROM credits.authorizations \
                     WHERE address = $1 AND status IN ('authorized', 'retired'))",
            )
            .bind(&address)
            .bind(UsdCents::CENTS_PER_CREDIT)
            .execute(&mut *tx)
            .await?
            .rows_affected();
            if lifted == 1 {
                let detail = json!({
                    "source": by.actor().source(),
                    "transactionId": transaction_id,
                    "refundRef": undone_refund_ref,
                    "reversalRef": reversal_ref,
                    "reason": "app store refund reversed",
                });
                Self::audit(
                    &mut *tx,
                    super::store::AuditEntry {
                        action: "user.unblock",
                        actor: by.actor().name(),
                        target: Some(&address),
                        detail: &detail,
                    },
                )
                .await?;
            }
        }
        tx.commit().await?;
        Ok(ReinstateOutcome::Reinstated {
            address,
            credits,
            withdrawn,
        })
    }

    /// Paid purchases not credited yet, oldest first.
    pub async fn held_iap(&self, limit: i64, offset: i64) -> Result<Vec<HeldPurchase>, ApiError> {
        let rows = sqlx::query(
            "SELECT transaction_id, product_id, quantity, address, app_account_token, held_reason, created_at \
             FROM credits.iap_transactions WHERE status = 'held' \
             ORDER BY created_at, transaction_id LIMIT $1 OFFSET $2",
        )
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .into_iter()
            .map(|r| HeldPurchase {
                transaction_id: r.get("transaction_id"),
                product_id: r.get("product_id"),
                quantity: r.get("quantity"),
                address: r.get("address"),
                app_account_token: r.get("app_account_token"),
                reason: r.get("held_reason"),
                created_at: r.get("created_at"),
            })
            .collect())
    }

    /// Records `address` as the wallet behind its account token.
    pub async fn link_iap_token(&self, address: &str) -> Result<String, ApiError> {
        let token = super::apple::account_token(address);
        // A no-op update returns the existing row, waiting for a concurrent insert of it.
        let owner: String = sqlx::query_scalar(
            "INSERT INTO credits.iap_account_tokens AS t (token, address) VALUES ($1, $2) \
             ON CONFLICT (token) DO UPDATE SET address = t.address RETURNING t.address",
        )
        .bind(&token)
        .bind(address)
        .fetch_one(&self.pool)
        .await?;
        if owner != address {
            return Err(ApiError::conflict(
                "the account token belongs to another wallet",
            ));
        }
        Ok(token)
    }

    /// The wallet a transaction's `appAccountToken` names: a registered token, or a wallet address itself.
    pub async fn wallet_for_iap_token(
        &self,
        token: Option<&str>,
    ) -> Result<Option<String>, ApiError> {
        let Some(token) = token.map(str::to_ascii_lowercase) else {
            return Ok(None);
        };
        if token.len() == 42
            && token.starts_with("0x")
            && token[2..].bytes().all(|b| b.is_ascii_hexdigit())
        {
            return Ok(Some(token));
        }
        Ok(
            sqlx::query_scalar("SELECT address FROM credits.iap_account_tokens WHERE token = $1")
                .bind(&token)
                .fetch_optional(&self.pool)
                .await?,
        )
    }
}

/// What a reversal does to a transaction's row, decided once before its
/// decision is recorded.
enum Undo {
    /// No row: Apple's reversal arrived first; an operator's is refused.
    Unknown,
    /// Paid but not credited yet: the credit that follows decides.
    Held,
    /// A refund of a purchase never credited: the row is held again.
    Rehold {
        address: Option<String>,
        undone_refund_ref: Option<String>,
    },
    /// A refund of a credited (or imported) purchase: granted back.
    Grant {
        address: String,
        credits: String,
        raised_block: bool,
        undone_refund_ref: Option<String>,
    },
    /// Nothing to undo: not refunded, or an upstream marker never cleared.
    NotRefunded,
}

impl Undo {
    fn of(row: Option<&sqlx::postgres::PgRow>) -> Self {
        let Some(row) = row else {
            return Self::Unknown;
        };
        let status: String = row.get("status");
        let refunded: bool = row.get("refunded");
        match status.as_str() {
            "held" => Self::Held,
            "refunded" if row.get::<bool, _>("upstream") => Self::NotRefunded,
            "refunded" if !row.get::<bool, _>("credited") => Self::Rehold {
                address: row.get("address"),
                undone_refund_ref: row.get("undone_refund_ref"),
            },
            "refunded" | "imported" if refunded => match row.get::<Option<String>, _>("address") {
                Some(address) => Self::Grant {
                    address,
                    credits: row.get("revoked"),
                    raised_block: row.get("raised_block"),
                    undone_refund_ref: row.get("undone_refund_ref"),
                },
                None => Self::NotRefunded,
            },
            _ => Self::NotRefunded,
        }
    }
}

/// Records a paid purchase that was not credited as `held` (not for an
/// operator credit, which is not Apple-signed) and answers why.
async fn hold(
    mut tx: sqlx::Transaction<'_, sqlx::Postgres>,
    p: &Purchase<'_>,
    address: Option<&str>,
    value: Option<(i64, i64)>,
    reason: HoldReason,
) -> Result<CreditOutcome, ApiError> {
    if matches!(p.channel, Channel::Operator(_)) {
        return Ok(CreditOutcome::Held(reason));
    }
    if reason.retried() {
        release(&mut tx, p.notice).await?;
    }
    sqlx::query(
        "INSERT INTO credits.iap_transactions AS t \
             (transaction_id, original_transaction_id, product_id, address, credits, amount_cents, quantity, \
              app_account_token, status, held_reason) \
         VALUES ($1, $2, $3, $4, $5::numeric, $6, $7, $8, 'held', $9) \
         ON CONFLICT (transaction_id) DO UPDATE SET \
             address = COALESCE(EXCLUDED.address, t.address), credits = EXCLUDED.credits, \
             amount_cents = EXCLUDED.amount_cents, quantity = EXCLUDED.quantity, \
             app_account_token = COALESCE(EXCLUDED.app_account_token, t.app_account_token), \
             held_reason = EXCLUDED.held_reason, updated_at = timezone('UTC', now()) \
         WHERE t.status = 'held'",
    )
    .bind(p.transaction_id)
    .bind(p.original_transaction_id)
    .bind(p.product_id)
    .bind(address)
    .bind(value.map(|(credits, _)| credits.to_string()))
    .bind(value.map(|(_, cents)| cents))
    .bind(p.quantity)
    .bind(p.app_account_token.map(str::to_ascii_lowercase))
    .bind(reason.as_str())
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    tracing::warn!(transaction_id = %p.transaction_id, reason = reason.as_str(), "app store purchase held: paid but not credited");
    Ok(CreditOutcome::Held(reason))
}

/// Queues the newest unexpired signed credits of `address` not queued yet
/// for `revokeCredits` until they cover what those credits pledge beyond the
/// balance, at most `uncovered` Credits. An expired credit cannot be spent
/// on chain, so it is neither revoked nor counted.
async fn revoke_live_credits_in_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    address: &str,
    uncovered: &str,
    reason: &str,
) -> Result<usize, ApiError> {
    let rows = sqlx::query(
        "WITH live AS ( \
             SELECT id, credit_hash, chain_id, contract, usd_cents, created_at \
             FROM credits.authorizations a \
             WHERE address = $1 AND status IN ('authorized', 'retired') \
               AND expires_at > timezone('UTC', now()) \
               AND NOT EXISTS (SELECT 1 FROM credits.revocations r WHERE r.credit_hash = a.credit_hash) \
         ), need AS ( \
             SELECT LEAST($2::numeric * $3::numeric, \
                          (SELECT COALESCE(SUM(usd_cents), 0) FROM live) \
                          - COALESCE((SELECT available FROM credits.wallets WHERE address = $1), 0) * $3::numeric) AS cents \
         ) \
         SELECT id, credit_hash, chain_id, contract FROM ( \
             SELECT id, credit_hash, chain_id, contract, \
                    SUM(usd_cents) OVER (ORDER BY created_at DESC, id) - usd_cents AS before \
             FROM live \
             WHERE credit_hash IS NOT NULL AND chain_id IS NOT NULL AND contract IS NOT NULL \
         ) a WHERE before < (SELECT cents FROM need)",
    )
    .bind(address)
    .bind(uncovered)
    .bind(UsdCents::CENTS_PER_CREDIT)
    .fetch_all(&mut **tx)
    .await?;
    let mut queued = 0;
    for r in rows {
        let (id, hash, chain_id, contract): (String, String, i64, String) = (
            r.get("id"),
            r.get("credit_hash"),
            r.get("chain_id"),
            r.get("contract"),
        );
        if enqueue_revocation(tx, &hash, &id, chain_id, &contract, reason).await? {
            queued += 1;
        }
    }
    if queued > 0 {
        tracing::warn!(address = %address, queued, uncovered, "app store refund exceeds the unpledged balance: live signed credits queued for revocation");
    }
    Ok(queued)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_bundle_id_without_an_environment_refuses_to_start() {
        let err = Iap::parse(RawSettings {
            bundle_id: Some("org.example.app".into()),
            daily_limit_cents: Some(0),
            ..RawSettings::default()
        })
        .err()
        .expect("APPLE_ENVIRONMENT is required");
        assert!(err.to_string().contains("APPLE_ENVIRONMENT"), "{err}");
    }

    #[test]
    fn the_catalog_takes_positive_whole_credits_only() {
        assert_eq!(parse_catalog(r#"{"p1": 100}"#).unwrap()["p1"], 100);
        assert!(parse_catalog(r#"{"p1": 0}"#).is_err());
        assert!(parse_catalog(r#"{"p1": 1.5}"#).is_err());
        assert!(parse_catalog("[]").is_err());
        assert!(
            parse_catalog(&format!(r#"{{"p1": {}}}"#, i64::MAX / MAX_QUANTITY)).is_err(),
            "a value whose most units overflow the cents is refused"
        );
    }

    #[test]
    fn caps_apply_in_order_and_zero_closes_the_node() {
        let limits = Limits {
            wallet_total_cents: 1000,
            wallet_daily_cents: 500,
            daily_cents: 10_000,
        };
        let t = Totals {
            total_cents: 900,
            daily_cents: 0,
            service_daily_cents: 0,
        };
        assert_eq!(t.decide(&limits, 200), Some(CapReason::TotalLimit));
        let t = Totals {
            total_cents: 0,
            daily_cents: 400,
            service_daily_cents: 0,
        };
        assert_eq!(t.decide(&limits, 200), Some(CapReason::DailyLimit));
        let t = Totals {
            total_cents: 0,
            daily_cents: 0,
            service_daily_cents: 9_900,
        };
        assert_eq!(t.decide(&limits, 200), Some(CapReason::ServiceDailyLimit));
        assert_eq!(t.decide(&limits, 100), None);
        let closed = Limits {
            daily_cents: 0,
            ..Limits::default()
        };
        assert_eq!(
            Totals {
                total_cents: 0,
                daily_cents: 0,
                service_daily_cents: 0
            }
            .decide(&closed, 10),
            Some(CapReason::ServiceDailyLimit)
        );
    }
}
