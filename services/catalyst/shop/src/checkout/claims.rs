//! One round trip that claims an idempotency key before a broadcast:
//! `claimed` when this call inserted the row, `rearmed` when a prior 'error'
//! row (or, for a broker purchase, a stale 'pending' one; see
//! [`claim_broker_purchase`]) was reset to 'pending', otherwise the pre-existing row's
//! `status`/`tx_hash`. Both `None` with nothing claimed means another claim
//! landed concurrently, read as in flight. `$now` is the request clock.

use chrono::NaiveDateTime;
use sqlx::PgPool;

#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct Claim {
    pub claimed: bool,
    pub rearmed: bool,
    pub status: Option<String>,
    pub tx_hash: Option<String>,
}

impl Claim {
    pub fn status(&self) -> &str {
        self.status.as_deref().unwrap_or("pending")
    }
}

pub struct EscrowActionClaim<'a> {
    pub key: &'a str,
    pub action: &'a str,
    pub collection: &'a str,
    pub token_id: &'a str,
    pub buyer: Option<&'a str>,
    pub escrow_address: &'a str,
    pub chain_id: i64,
}

pub async fn claim_escrow_action(
    pool: &PgPool,
    claim: &EscrowActionClaim<'_>,
    now: NaiveDateTime,
) -> Result<Claim, sqlx::Error> {
    sqlx::query_as(
        "WITH claim AS (\
           INSERT INTO shop.escrow_actions \
           (idempotency_key, action, collection, token_id, buyer, escrow_address, chain_id, status, created_at, updated_at) \
           VALUES ($1, $2, $3, $4, $5, $6, $7, 'pending', $8, $8) \
           ON CONFLICT (idempotency_key) WHERE idempotency_key IS NOT NULL DO NOTHING \
           RETURNING id\
         ), rearm AS (\
           UPDATE shop.escrow_actions SET status = 'pending', updated_at = $8 \
           WHERE idempotency_key = $1 AND status = 'error' AND NOT EXISTS (SELECT 1 FROM claim) \
           RETURNING id\
         ) \
         SELECT EXISTS (SELECT 1 FROM claim) AS claimed, EXISTS (SELECT 1 FROM rearm) AS rearmed, \
                prior.status, prior.tx_hash \
         FROM (SELECT 1) AS one \
         LEFT JOIN shop.escrow_actions AS prior ON prior.idempotency_key = $1",
    )
    .bind(claim.key)
    .bind(claim.action)
    .bind(claim.collection)
    .bind(claim.token_id)
    .bind(claim.buyer)
    .bind(claim.escrow_address)
    .bind(claim.chain_id)
    .bind(now)
    .fetch_one(pool)
    .await
}

/// A `shop.broker_purchases` claim (a wearable buy or a NAME buy; a NAME
/// buy keeps the registrar in `collection` and the label in `item_id`).
pub struct BrokerPurchaseClaim<'a> {
    pub key: &'a str,
    pub collection: &'a str,
    pub item_id: Option<&'a str>,
    pub token_id: Option<&'a str>,
    pub buyer_address: &'a str,
    pub escrow_address: &'a str,
    pub price_wei: &'a str,
    pub chain_id: i64,
    pub mode: &'a str,
    pub trade_hashed_signature: Option<&'a str>,
    pub usd_amount_wei: Option<&'a str>,
    pub mana_usd_rate_wei: Option<&'a str>,
}

/// How long a broker claim may stay `pending` before a new caller may take
/// it over: longer than one signing (estimate, nonce, fees) under the send
/// lock. Taking over is safe at any age, because a transaction is sent only
/// after its record, which is fenced on the claim's `status = 'pending'` and
/// `updated_at`; the delay only spares an in-flight caller.
pub const BROKER_PENDING_TAKEOVER_SECS: i64 = 120;

/// Claims a broker purchase. Every claim this binary makes is `write_ahead`:
/// its transaction is recorded before it is broadcast, so a `write_ahead`
/// claim with no recorded `tx_hash` provably sent nothing. Only such a claim
/// is re-armed: one marked `error` (its signing failed), or one left
/// `pending` for [`BROKER_PENDING_TAKEOVER_SECS`] (its caller stopped before
/// recording). A re-armed row takes this request's columns, so the row
/// describes the transaction it will record. The claimant fences its writes
/// on `updated_at = $now`. A key naming a credits checkout line
/// (`checkout:{checkout_id}:{outbox_id}`) is neither claimed nor re-armed once
/// that checkout is frozen (`reversing`, `reversed`, `failed`); the checkout
/// row is read under `FOR SHARE`, so the check orders against the freeze.
pub async fn claim_broker_purchase(
    pool: &PgPool,
    claim: &BrokerPurchaseClaim<'_>,
    now: NaiveDateTime,
) -> Result<Claim, sqlx::Error> {
    sqlx::query_as(
        "WITH frozen AS (\
           SELECT status FROM credits.checkouts \
           WHERE id = substring($1 from '^checkout:([0-9]{1,18}):[0-9]+$')::bigint \
           FOR SHARE\
         ), claim AS (\
           INSERT INTO shop.broker_purchases \
           (idempotency_key, collection, item_id, token_id, buyer_address, escrow_address, price_wei, chain_id, mode, \
            trade_hashed_signature, usd_amount_wei, mana_usd_rate_wei, status, write_ahead, created_at, updated_at) \
           SELECT $1, $2, $3, $4, $5, $6, $7::numeric, $8, $9, $10, $11::numeric, $12::numeric, 'pending', true, $13, $13 \
           WHERE NOT EXISTS (SELECT 1 FROM frozen WHERE status IN ('reversing', 'reversed', 'failed')) \
           ON CONFLICT (idempotency_key) WHERE idempotency_key IS NOT NULL DO NOTHING \
           RETURNING id\
         ), rearm AS (\
           UPDATE shop.broker_purchases SET status = 'pending', updated_at = $13, \
             collection = $2, item_id = $3, token_id = $4, buyer_address = $5, escrow_address = $6, \
             price_wei = $7::numeric, chain_id = $8, mode = $9, trade_hashed_signature = $10, \
             usd_amount_wei = $11::numeric, mana_usd_rate_wei = $12::numeric \
           WHERE idempotency_key = $1 AND write_ahead AND tx_hash IS NULL \
             AND (status = 'error' OR (status = 'pending' AND updated_at < $13 - make_interval(secs => $14))) \
             AND NOT EXISTS (SELECT 1 FROM claim) \
             AND NOT EXISTS (SELECT 1 FROM frozen WHERE status IN ('reversing', 'reversed', 'failed')) \
           RETURNING id\
         ) \
         SELECT EXISTS (SELECT 1 FROM claim) AS claimed, EXISTS (SELECT 1 FROM rearm) AS rearmed, \
                prior.status, prior.tx_hash \
         FROM (SELECT 1) AS one \
         LEFT JOIN shop.broker_purchases AS prior ON prior.idempotency_key = $1",
    )
    .bind(claim.key)
    .bind(claim.collection)
    .bind(claim.item_id)
    .bind(claim.token_id)
    .bind(claim.buyer_address)
    .bind(claim.escrow_address)
    .bind(claim.price_wei)
    .bind(claim.chain_id)
    .bind(claim.mode)
    .bind(claim.trade_hashed_signature)
    .bind(claim.usd_amount_wei)
    .bind(claim.mana_usd_rate_wei)
    .bind(now)
    .bind(BROKER_PENDING_TAKEOVER_SECS as f64)
    .fetch_one(pool)
    .await
}

pub struct NameTransferClaim<'a> {
    pub key: &'a str,
    pub registrar: &'a str,
    pub token_id: &'a str,
    pub from_address: &'a str,
    pub to_address: &'a str,
    pub chain_id: i64,
}

pub async fn claim_name_transfer(
    pool: &PgPool,
    claim: &NameTransferClaim<'_>,
    now: NaiveDateTime,
) -> Result<Claim, sqlx::Error> {
    sqlx::query_as(
        "WITH claim AS (\
           INSERT INTO shop.name_transfers \
           (idempotency_key, registrar, token_id, from_address, to_address, chain_id, status, created_at, updated_at) \
           VALUES ($1, $2, $3, $4, $5, $6, 'pending', $7, $7) \
           ON CONFLICT (idempotency_key) DO NOTHING \
           RETURNING id\
         ), rearm AS (\
           UPDATE shop.name_transfers SET status = 'pending', updated_at = $7 \
           WHERE idempotency_key = $1 AND status = 'error' AND NOT EXISTS (SELECT 1 FROM claim) \
           RETURNING id\
         ) \
         SELECT EXISTS (SELECT 1 FROM claim) AS claimed, EXISTS (SELECT 1 FROM rearm) AS rearmed, \
                prior.status, prior.tx_hash \
         FROM (SELECT 1) AS one \
         LEFT JOIN shop.name_transfers AS prior ON prior.idempotency_key = $1",
    )
    .bind(claim.key)
    .bind(claim.registrar)
    .bind(claim.token_id)
    .bind(claim.from_address)
    .bind(claim.to_address)
    .bind(claim.chain_id)
    .bind(now)
    .fetch_one(pool)
    .await
}
