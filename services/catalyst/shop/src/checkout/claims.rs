//! One round trip that claims an idempotency key before a broadcast:
//! `claimed` when this call inserted the row, `rearmed` when a prior 'error'
//! row was reset to 'pending', otherwise the pre-existing row's
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

pub struct NamePurchaseClaim<'a> {
    pub key: &'a str,
    pub registrar: &'a str,
    pub name: Option<&'a str>,
    pub token_id: Option<&'a str>,
    pub buyer_address: &'a str,
    pub custody_address: &'a str,
    pub price_wei: &'a str,
    pub chain_id: i64,
    pub mode: &'a str,
}

pub async fn claim_name_purchase(
    pool: &PgPool,
    claim: &NamePurchaseClaim<'_>,
    now: NaiveDateTime,
) -> Result<Claim, sqlx::Error> {
    sqlx::query_as(
        "WITH claim AS (\
           INSERT INTO shop.broker_purchases \
           (idempotency_key, collection, item_id, token_id, buyer_address, escrow_address, price_wei, chain_id, mode, status, created_at, updated_at) \
           VALUES ($1, $2, $3, $4, $5, $6, $7::numeric, $8, $9, 'pending', $10, $10) \
           ON CONFLICT (idempotency_key) WHERE idempotency_key IS NOT NULL DO NOTHING \
           RETURNING id\
         ), rearm AS (\
           UPDATE shop.broker_purchases SET status = 'pending', updated_at = $10 \
           WHERE idempotency_key = $1 AND status = 'error' AND NOT EXISTS (SELECT 1 FROM claim) \
           RETURNING id\
         ) \
         SELECT EXISTS (SELECT 1 FROM claim) AS claimed, EXISTS (SELECT 1 FROM rearm) AS rearmed, \
                prior.status, prior.tx_hash \
         FROM (SELECT 1) AS one \
         LEFT JOIN shop.broker_purchases AS prior ON prior.idempotency_key = $1",
    )
    .bind(claim.key)
    .bind(claim.registrar)
    .bind(claim.name)
    .bind(claim.token_id)
    .bind(claim.buyer_address)
    .bind(claim.custody_address)
    .bind(claim.price_wei)
    .bind(claim.chain_id)
    .bind(claim.mode)
    .bind(now)
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
