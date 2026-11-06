//! The events-notifier tip forward, `POST /events-notifier/forward-blockchain`
//! (`{type: "tip-received", transactionHash}`, ADR-44 signed by the tipper),
//! as the auth dApp sends it after a MANA meta-transaction. Where the old
//! service read the receipt over RPC, the claim is checked against this node's
//! own Polygon MANA transfer index (`shop.mana_transfer`, the `mana_transfers`
//! chain walk): the transaction must hold exactly one published Transfer whose
//! `from` is the signer, and its `to` is told `tip_received`. The forward
//! arrives before the index reaches the transaction, so it is kept as a
//! claim (`shop.tip_claim`) and resolved then, by the request itself or the
//! `tip-notifications` job; a claim the index never confirms expires after
//! an hour. A transfer below the minimum amount, or mined longer than the
//! maximum age before its claim, is rejected (`TipSettings`). The
//! notification id is a uuid v5 of the transaction hash.

use std::time::Duration;

use alloy_primitives::U256;
use axum::Json;
use axum::Router;
use axum::body::Bytes;
use axum::extract::{OriginalUri, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use chrono::NaiveDateTime;
use serde_json::{Value, json};
use sqlx::PgPool;
use uuid::Uuid;

use super::SharedState;
use super::chain_walk::{self, ChainNet, INDEXER_MANA};
use crate::jobs::{Jobs, PeriodicCfg};
use crate::shared::identity::Address;
use crate::shared::signed_fetch::{self, PayloadPolicy, SignedPath};
use crate::social::notifications::store::{NewNotification, insert_notifications};

pub const KIND: &str = "tip_received";
const SUBTYPE: &str = "tip-received";
const NET: ChainNet = ChainNet::Matic;
const CLAIM_TTL_SECS: i64 = 3600;
const MAX_PENDING_PER_SENDER: i64 = 20;
/// Claims one resolve pass settles at most.
const RESOLVE_LIMIT: i64 = 500;
const POLL_SECS: u64 = 15;

pub const DEFAULT_MIN_AMOUNT_WEI: &str = "1000000000000000000";
pub const DEFAULT_MAX_AGE_SECS: u64 = 86_400;
pub const DEFAULT_MAX_PENDING: u64 = 2_000;

/// What a tip must be to notify, and how many claims may wait at once.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TipSettings {
    /// `MARKETPLACE_TIP_MIN_AMOUNT_WEI`: the smallest transfer that notifies.
    pub min_amount_wei: U256,
    /// `MARKETPLACE_TIP_MAX_AGE_SECS`: the oldest transfer (block time before the claim) that notifies.
    pub max_age_secs: i64,
    /// `MARKETPLACE_TIP_MAX_PENDING`: pending claims across every sender.
    pub max_pending: i64,
}

impl Default for TipSettings {
    fn default() -> Self {
        Self::parse(
            DEFAULT_MIN_AMOUNT_WEI,
            DEFAULT_MAX_AGE_SECS,
            DEFAULT_MAX_PENDING,
        )
        .expect("the defaults parse")
    }
}

impl TipSettings {
    pub fn parse(
        min_amount_wei: &str,
        max_age_secs: u64,
        max_pending: u64,
    ) -> anyhow::Result<Self> {
        let min_amount_wei = U256::from_str_radix(min_amount_wei.trim(), 10).map_err(|e| {
            anyhow::anyhow!("MARKETPLACE_TIP_MIN_AMOUNT_WEI is not a wei amount: {e}")
        })?;
        Ok(Self {
            min_amount_wei,
            max_age_secs: i64::try_from(max_age_secs).unwrap_or(i64::MAX),
            max_pending: i64::try_from(max_pending).unwrap_or(i64::MAX),
        })
    }
}

pub fn notification_id(transaction_hash: &str) -> Uuid {
    Uuid::new_v5(
        &Uuid::NAMESPACE_OID,
        format!("tip:{transaction_hash}").as_bytes(),
    )
}

pub fn router(state: SharedState) -> Router {
    Router::new()
        .route("/forward-blockchain", post(forward))
        .with_state(state)
}

fn error(status: StatusCode, message: impl Into<String>) -> Response {
    (status, Json(json!({ "error": message.into() }))).into_response()
}

/// The old route's schema: exactly `{type, transactionHash}`, the type `tip-received`.
fn parse_body(body: &[u8]) -> Result<String, String> {
    let Ok(Value::Object(obj)) = serde_json::from_slice::<Value>(body) else {
        return Err("Invalid JSON body".into());
    };
    if obj.keys().any(|k| k != "type" && k != "transactionHash") {
        return Err("must NOT have additional properties".into());
    }
    match obj.get("type").and_then(Value::as_str) {
        Some(SUBTYPE) => {}
        Some(other) => return Err(format!("Unsupported event type: {other}")),
        None => return Err("must have required property 'type'".into()),
    }
    match obj.get("transactionHash").and_then(Value::as_str) {
        Some(h)
            if h.len() == 66
                && h.starts_with("0x")
                && h[2..].bytes().all(|b| b.is_ascii_hexdigit()) =>
        {
            Ok(h.to_ascii_lowercase())
        }
        Some(_) => Err("transactionHash must match pattern \"^0x[a-fA-F0-9]{64}$\"".into()),
        None => Err("must have required property 'transactionHash'".into()),
    }
}

async fn forward(
    State(state): State<SharedState>,
    OriginalUri(original): OriginalUri,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let signer = match signed_fetch::require_signer_at(
        &headers,
        "post",
        &SignedPath::at(&original, "/forward-blockchain"),
        PayloadPolicy::V6ThenLegacy,
        state.eip1654.as_deref(),
    )
    .await
    {
        Ok(s) => s.into_string().to_ascii_lowercase(),
        Err(e) => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(json!({
                    "error": e.to_string(),
                    "message": "This endpoint requires a signed fetch request. See ADR-44.",
                })),
            )
                .into_response();
        }
    };
    let tx = match parse_body(&body) {
        Ok(tx) => tx,
        Err(message) => return error(StatusCode::BAD_REQUEST, message),
    };
    match claim(&state.pool, &state.settings.market.tips, &tx, &signer).await {
        Ok(Outcome::Notified) => StatusCode::NO_CONTENT.into_response(),
        Ok(Outcome::Pending) => (StatusCode::ACCEPTED, Json(json!({}))).into_response(),
        Ok(Outcome::TooMany) => error(
            StatusCode::TOO_MANY_REQUESTS,
            "Too many tips waiting for confirmation",
        ),
        Ok(Outcome::Rejected(why)) => error(StatusCode::BAD_REQUEST, why.message(&tx)),
        Err(e) => {
            tracing::error!(error = %e, tx, "tip forward failed");
            error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Tip could not be processed",
            )
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum Outcome {
    Notified,
    Pending,
    Rejected(Rejection),
    TooMany,
}

/// Why a claim was refused; each is the old route's 400.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rejection {
    NotSingleTransfer,
    TooSmall,
    TooOld,
    Malformed,
}

impl Rejection {
    fn message(self, tx: &str) -> String {
        match self {
            Rejection::NotSingleTransfer => {
                format!("Transaction is not a single MANA transfer from the signer: {tx}")
            }
            Rejection::TooSmall => format!("Transaction {tx} transfers less than the minimum tip"),
            Rejection::TooOld => format!("Transaction {tx} is too old"),
            Rejection::Malformed => format!("Transaction {tx} could not be read"),
        }
    }
}

/// Records the claim (under the sender's lock, within the per-sender and
/// global caps) and resolves it at once if the index already has it.
pub async fn claim(
    pool: &PgPool,
    settings: &TipSettings,
    tx: &str,
    sender: &str,
) -> anyhow::Result<Outcome> {
    let mut txn = pool.begin().await?;
    sqlx::query("SELECT pg_advisory_xact_lock(hashtext('tip_claim:' || $1))")
        .bind(sender)
        .execute(&mut *txn)
        .await?;
    let (exists, mine, all): (bool, i64, i64) = sqlx::query_as(
        "SELECT EXISTS (SELECT 1 FROM shop.tip_claim WHERE transaction_hash = $1 AND sender = $2), \
                (SELECT count(*) FROM shop.tip_claim WHERE sender = $2), \
                (SELECT count(*) FROM shop.tip_claim)",
    )
    .bind(tx)
    .bind(sender)
    .fetch_one(&mut *txn)
    .await?;
    if !exists {
        // The global count is advisory: concurrent senders may pass it together.
        if mine >= MAX_PENDING_PER_SENDER || all >= settings.max_pending {
            return Ok(Outcome::TooMany);
        }
        sqlx::query(
            "INSERT INTO shop.tip_claim (transaction_hash, sender) VALUES ($1, $2) ON CONFLICT DO NOTHING",
        )
        .bind(tx)
        .bind(sender)
        .execute(&mut *txn)
        .await?;
    }
    txn.commit().await?;
    let resolved = resolve(pool, settings, Some((tx, sender))).await?;
    Ok(resolved
        .into_iter()
        .next()
        .map(|(_, o)| o)
        .unwrap_or(Outcome::Pending))
}

#[derive(sqlx::FromRow)]
struct ClaimRow {
    transaction_hash: String,
    sender: String,
    created_at: NaiveDateTime,
    recipient: Option<String>,
    amount: Option<String>,
    timestamp: Option<i64>,
}

/// `ethers.formatUnits(wei, 18)`: at least one fraction digit, no trailing zeros.
pub fn format_mana(wei: &str) -> Option<String> {
    let v = U256::from_str_radix(wei, 10).ok()?;
    let unit = U256::from(10u64).pow(U256::from(18u64));
    let frac = format!("{:0>18}", (v % unit).to_string());
    let frac = frac.trim_end_matches('0');
    Some(format!(
        "{}.{}",
        v / unit,
        if frac.is_empty() { "0" } else { frac }
    ))
}

/// JavaScript `String(n)` for a non-negative finite number.
fn js_number(n: f64) -> String {
    if n < 1e21 {
        format!("{n}")
    } else {
        format!("{n:e}").replace('e', "e+")
    }
}

/// The old processor's metadata: `amount` in MANA, `manaAmount` its wei as a JS number.
pub fn metadata(sender: &str, recipient: &str, amount: &str) -> Value {
    let mana_amount = js_number(amount.parse::<f64>().unwrap_or(0.0) * 1e18);
    json!({
        "amount": amount,
        "manaAmount": mana_amount,
        "senderAddress": sender,
        "receiverAddress": recipient,
    })
}

/// The verdict on a claim's published transfers.
fn judge(
    settings: &TipSettings,
    claim: &ClaimRow,
    transfers: &[(&str, &str, i64)],
) -> Result<NewNotification, Rejection> {
    let [(recipient, amount, timestamp)] = transfers else {
        return Err(Rejection::NotSingleTransfer);
    };
    if *recipient == claim.sender {
        return Err(Rejection::NotSingleTransfer);
    }
    let (Ok(address), Ok(wei), Some(mana)) = (
        Address::parse(recipient),
        U256::from_str_radix(amount, 10),
        format_mana(amount),
    ) else {
        return Err(Rejection::Malformed);
    };
    if wei < settings.min_amount_wei {
        return Err(Rejection::TooSmall);
    }
    if claim.created_at.and_utc().timestamp() - timestamp > settings.max_age_secs {
        return Err(Rejection::TooOld);
    }
    Ok(NewNotification {
        id: notification_id(&claim.transaction_hash),
        address,
        kind: KIND.into(),
        metadata: metadata(&claim.sender, recipient, &mana),
        occurred_at: claim.created_at,
    })
}

/// Settles the claims (one, or up to [`RESOLVE_LIMIT`] with `None`) the
/// published index holds a transfer for: a single transfer from the signer
/// notifies its recipient, any other shape is rejected; a claim without one
/// stays pending and is not read.
pub async fn resolve(
    pool: &PgPool,
    settings: &TipSettings,
    only: Option<(&str, &str)>,
) -> anyhow::Result<Vec<(String, Outcome)>> {
    let now = chrono::Utc::now();
    let Some(cursor) = chain_walk::ready_cursor(pool, INDEXER_MANA, NET, now).await else {
        return Ok(Vec::new());
    };
    let mut txn = pool.begin().await?;
    let rows: Vec<ClaimRow> = sqlx::query_as(
        "SELECT c.transaction_hash, c.sender, c.created_at, t.recipient, t.amount::text AS amount, t.timestamp \
         FROM (SELECT * FROM shop.tip_claim tc \
               WHERE ($3::text IS NULL OR (tc.transaction_hash = $3 AND tc.sender = $4)) \
                 AND EXISTS (SELECT 1 FROM shop.mana_transfer m WHERE m.network = $1 AND m.sender = tc.sender \
                             AND m.transaction_hash = tc.transaction_hash AND m.block_number <= $2) \
               ORDER BY tc.created_at, tc.transaction_hash, tc.sender LIMIT $5 FOR UPDATE SKIP LOCKED) c \
         JOIN shop.mana_transfer t ON t.network = $1 AND t.sender = c.sender \
              AND t.transaction_hash = c.transaction_hash AND t.block_number <= $2 \
         ORDER BY c.transaction_hash, c.sender, t.log_index",
    )
    .bind(NET.db_name())
    .bind(cursor.last_block)
    .bind(only.map(|(tx, _)| tx))
    .bind(only.map(|(_, s)| s))
    .bind(RESOLVE_LIMIT)
    .fetch_all(&mut *txn)
    .await?;
    let mut outcomes = Vec::new();
    let mut notifications = Vec::new();
    let mut settled: Vec<(String, String)> = Vec::new();
    for group in
        rows.chunk_by(|a, b| (&a.transaction_hash, &a.sender) == (&b.transaction_hash, &b.sender))
    {
        let first = &group[0];
        let transfers: Vec<(&str, &str, i64)> = group
            .iter()
            .filter_map(|r| Some((r.recipient.as_deref()?, r.amount.as_deref()?, r.timestamp?)))
            .collect();
        let outcome = match judge(settings, first, &transfers) {
            Ok(n) => {
                notifications.push(n);
                Outcome::Notified
            }
            Err(why) => {
                if why == Rejection::Malformed {
                    tracing::warn!(
                        tx = first.transaction_hash,
                        "tip claim: malformed indexed transfer; rejected"
                    );
                }
                Outcome::Rejected(why)
            }
        };
        settled.push((first.transaction_hash.clone(), first.sender.clone()));
        outcomes.push((first.transaction_hash.clone(), outcome));
    }
    insert_notifications(&mut *txn, &notifications).await?;
    let (txs, senders): (Vec<String>, Vec<String>) = settled.into_iter().unzip();
    sqlx::query(
        "DELETE FROM shop.tip_claim c USING UNNEST($1::text[], $2::text[]) AS s(tx, sender) \
         WHERE c.transaction_hash = s.tx AND c.sender = s.sender",
    )
    .bind(&txs)
    .bind(&senders)
    .execute(&mut *txn)
    .await?;
    txn.commit().await?;
    Ok(outcomes)
}

/// Claims past their hour; returns how many were dropped.
async fn expire(pool: &PgPool) -> anyhow::Result<u64> {
    Ok(sqlx::query(
        "DELETE FROM shop.tip_claim WHERE created_at < timezone('UTC', now()) - make_interval(secs => $1)",
    )
    .bind(CLAIM_TTL_SECS as f64)
    .execute(pool)
    .await?
    .rows_affected())
}

pub fn spawn_job(state: &SharedState, jobs: &Jobs) {
    let settings = state.settings.market.tips.clone();
    if !state.settings.market.indexes.mana_transfers_enabled {
        tracing::warn!(
            "MARKETPLACE_MANA_TRANSFERS_INDEX_ENABLED is off here: tip forwards wait for an index another process must walk, and expire unconfirmed without one"
        );
    }
    let pool = state.pool.clone();
    jobs.spawn_periodic_leased(
        "tip-notifications",
        PeriodicCfg::every(Duration::from_secs(POLL_SECS))
            .with_jitter(Duration::from_secs(5))
            .with_error_backoff(Duration::from_secs(300)),
        move |ctx| {
            let (pool, settings) = (pool.clone(), settings.clone());
            async move {
                let expired = expire(&pool).await?;
                if chain_walk::ready_cursor(&pool, INDEXER_MANA, NET, chrono::Utc::now())
                    .await
                    .is_none()
                {
                    let waiting: i64 = sqlx::query_scalar("SELECT count(*) FROM shop.tip_claim")
                        .fetch_one(&pool)
                        .await?;
                    ctx.progress(format!(
                        "MANA transfer index ({}) not ready: {waiting} tip(s) waiting, {expired} expired",
                        NET.db_name()
                    ));
                    return Ok(());
                }
                let outcomes = resolve(&pool, &settings, None).await?;
                let notified = outcomes
                    .iter()
                    .filter(|(_, o)| *o == Outcome::Notified)
                    .count();
                let rejected = outcomes.len() - notified;
                if notified > 0 || rejected > 0 || expired > 0 {
                    tracing::info!(notified, rejected, expired, "tip notifications");
                }
                let waiting: i64 = sqlx::query_scalar("SELECT count(*) FROM shop.tip_claim")
                    .fetch_one(&pool)
                    .await?;
                ctx.progress(format!(
                    "{notified} notified, {rejected} rejected, {waiting} waiting, {expired} expired"
                ));
                Ok(())
            }
        },
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn amounts_format_as_the_old_processor() {
        assert_eq!(format_mana("10000000000000000000").as_deref(), Some("10.0"));
        assert_eq!(format_mana("1500000000000000000").as_deref(), Some("1.5"));
        assert_eq!(format_mana("1").as_deref(), Some("0.000000000000000001"));
        assert_eq!(format_mana("0").as_deref(), Some("0.0"));
        let m = metadata("0xa", "0xb", "10.0");
        assert_eq!(m["manaAmount"], "10000000000000000000");
        assert_eq!(metadata("0xa", "0xb", "1500.0")["manaAmount"], "1.5e+21");
        assert_eq!(
            metadata("0xa", "0xb", "0.1")["manaAmount"],
            "100000000000000000"
        );
    }

    const TIPPER: &str = "0x00000000000000000000000000000000000000a1";
    const RECEIVER: &str = "0x00000000000000000000000000000000000000b2";

    fn hash(n: u8) -> String {
        format!("0x{:064x}", n)
    }

    fn now_secs() -> i64 {
        chrono::Utc::now().timestamp()
    }

    #[allow(clippy::too_many_arguments)]
    async fn transfer(
        pool: &PgPool,
        block: i64,
        log: i32,
        tx: &str,
        from: &str,
        to: &str,
        amount: &str,
        timestamp: i64,
    ) {
        sqlx::query(
            "INSERT INTO shop.mana_transfer (network, block_number, log_index, transaction_hash, sender, recipient, amount, timestamp) \
             VALUES ('matic', $1, $2, $3, $4, $5, $6::numeric, $7)",
        )
        .bind(block)
        .bind(log)
        .bind(tx)
        .bind(from)
        .bind(to)
        .bind(amount)
        .bind(timestamp)
        .execute(pool)
        .await
        .unwrap();
    }

    async fn publish(pool: &PgPool, block: i64) {
        sqlx::query(
            "INSERT INTO shop.chain_cursor (indexer, network, start_block, last_block, folded_block, folded_at, full_history) \
             VALUES ('mana_transfers', 'matic', 0, $1, $1, timezone('UTC', now()), true)",
        )
        .bind(block)
        .execute(pool)
        .await
        .unwrap();
    }

    async fn tip_count(pool: &PgPool) -> i64 {
        sqlx::query_scalar("SELECT count(*) FROM social.notifications WHERE type = 'tip_received'")
            .fetch_one(pool)
            .await
            .unwrap()
    }

    /// Pending until the published index holds the transfer, then the
    /// recipient is told once; unpublished blocks, a stranger's claim, a
    /// multi-transfer transaction and the per-sender cap are refused.
    #[tokio::test]
    async fn a_tip_notifies_its_recipient_once_the_index_publishes_it() {
        let Some(db) = crate::market::tests::support::OwnDb::create("tips").await else {
            return;
        };
        let pool = &db.pool;
        let settings = TipSettings::default();
        let tip = "2500000000000000000";
        let t = now_secs();
        transfer(pool, 100, 0, &hash(1), TIPPER, RECEIVER, tip, t).await;
        assert_eq!(
            claim(pool, &settings, &hash(1), TIPPER).await.unwrap(),
            Outcome::Pending,
            "an index that is not ready answers nothing"
        );
        publish(pool, 150).await;
        let outcomes = resolve(pool, &settings, None).await.unwrap();
        assert_eq!(outcomes, vec![(hash(1), Outcome::Notified)]);
        assert_eq!(
            claim(pool, &settings, &hash(1), TIPPER).await.unwrap(),
            Outcome::Notified,
            "a repeated forward is idempotent"
        );
        assert_eq!(tip_count(pool).await, 1);
        let (address, metadata, id): (String, Value, Uuid) = sqlx::query_as(
            "SELECT address, metadata, id FROM social.notifications WHERE type = 'tip_received'",
        )
        .fetch_one(pool)
        .await
        .unwrap();
        assert_eq!(
            (address.as_str(), id),
            (RECEIVER, notification_id(&hash(1)))
        );
        assert_eq!(
            metadata,
            json!({"amount": "2.5", "manaAmount": "2500000000000000000", "senderAddress": TIPPER, "receiverAddress": RECEIVER})
        );

        transfer(pool, 200, 0, &hash(2), TIPPER, RECEIVER, tip, t).await;
        assert_eq!(
            claim(pool, &settings, &hash(2), TIPPER).await.unwrap(),
            Outcome::Pending,
            "past the published block"
        );
        transfer(pool, 120, 0, &hash(3), TIPPER, RECEIVER, tip, t).await;
        assert_eq!(
            claim(
                pool,
                &settings,
                &hash(3),
                "0x00000000000000000000000000000000000000c3"
            )
            .await
            .unwrap(),
            Outcome::Pending,
            "someone else's transfer never matches"
        );
        transfer(pool, 130, 0, &hash(4), TIPPER, RECEIVER, tip, t).await;
        transfer(pool, 130, 1, &hash(4), TIPPER, RECEIVER, tip, t).await;
        assert_eq!(
            claim(pool, &settings, &hash(4), TIPPER).await.unwrap(),
            Outcome::Rejected(Rejection::NotSingleTransfer)
        );
        assert_eq!(tip_count(pool).await, 1);

        sqlx::query("UPDATE shop.tip_claim SET created_at = created_at - interval '2 hours'")
            .execute(pool)
            .await
            .unwrap();
        assert_eq!(expire(pool).await.unwrap(), 2);
        for n in 10..10 + MAX_PENDING_PER_SENDER as u8 {
            assert_eq!(
                claim(pool, &settings, &hash(n), TIPPER).await.unwrap(),
                Outcome::Pending
            );
        }
        assert_eq!(
            claim(pool, &settings, &hash(99), TIPPER).await.unwrap(),
            Outcome::TooMany
        );
        assert_eq!(
            claim(pool, &settings, &hash(10), TIPPER).await.unwrap(),
            Outcome::Pending,
            "a repeated claim is not a new one"
        );
        db.drop().await;
    }

    /// Below the minimum or mined more than the maximum age before the
    /// claim is refused with the old route's 400; the boundaries notify.
    #[tokio::test]
    async fn small_or_old_tips_are_rejected() {
        let Some(db) = crate::market::tests::support::OwnDb::create("tips_thresholds").await else {
            return;
        };
        let pool = &db.pool;
        let settings = TipSettings::default();
        let t = now_secs();
        let one = "1000000000000000000";
        transfer(pool, 10, 0, &hash(1), TIPPER, RECEIVER, "1", t).await;
        transfer(
            pool,
            11,
            0,
            &hash(2),
            TIPPER,
            RECEIVER,
            "999999999999999999",
            t,
        )
        .await;
        transfer(pool, 12, 0, &hash(3), TIPPER, RECEIVER, one, t).await;
        transfer(
            pool,
            13,
            0,
            &hash(4),
            TIPPER,
            RECEIVER,
            one,
            t - 86_400 - 60,
        )
        .await;
        transfer(
            pool,
            14,
            0,
            &hash(5),
            TIPPER,
            RECEIVER,
            one,
            t - 86_400 + 60,
        )
        .await;
        transfer(pool, 15, 0, &hash(6), TIPPER, TIPPER, one, t).await;
        publish(pool, 100).await;
        let expect = [
            (1, Outcome::Rejected(Rejection::TooSmall)),
            (2, Outcome::Rejected(Rejection::TooSmall)),
            (3, Outcome::Notified),
            (4, Outcome::Rejected(Rejection::TooOld)),
            (5, Outcome::Notified),
            (6, Outcome::Rejected(Rejection::NotSingleTransfer)),
        ];
        for (n, outcome) in expect {
            assert_eq!(
                claim(pool, &settings, &hash(n), TIPPER).await.unwrap(),
                outcome,
                "tx {n}"
            );
        }
        assert_eq!(tip_count(pool).await, 2);
        let pending: i64 = sqlx::query_scalar("SELECT count(*) FROM shop.tip_claim")
            .fetch_one(pool)
            .await
            .unwrap();
        assert_eq!(pending, 0, "every verdict settles its claim");
        let lenient = TipSettings::parse("1", 10 * 86_400, 10).unwrap();
        transfer(pool, 16, 0, &hash(7), TIPPER, RECEIVER, "1", t - 2 * 86_400).await;
        assert_eq!(
            claim(pool, &lenient, &hash(7), TIPPER).await.unwrap(),
            Outcome::Notified,
            "the thresholds are settings"
        );
        db.drop().await;
    }

    /// Past the global cap every sender is refused until claims settle.
    #[tokio::test]
    async fn the_pending_claims_have_a_global_cap() {
        let Some(db) = crate::market::tests::support::OwnDb::create("tips_cap").await else {
            return;
        };
        let pool = &db.pool;
        let settings = TipSettings::parse(DEFAULT_MIN_AMOUNT_WEI, DEFAULT_MAX_AGE_SECS, 3).unwrap();
        for n in 1..=3u8 {
            let sender = format!("0x{:040x}", n);
            assert_eq!(
                claim(pool, &settings, &hash(n), &sender).await.unwrap(),
                Outcome::Pending
            );
        }
        assert_eq!(
            claim(
                pool,
                &settings,
                &hash(9),
                "0x00000000000000000000000000000000000000f9"
            )
            .await
            .unwrap(),
            Outcome::TooMany
        );
        sqlx::query("DELETE FROM shop.tip_claim WHERE transaction_hash = $1")
            .bind(hash(1))
            .execute(pool)
            .await
            .unwrap();
        assert_eq!(
            claim(
                pool,
                &settings,
                &hash(9),
                "0x00000000000000000000000000000000000000f9"
            )
            .await
            .unwrap(),
            Outcome::Pending
        );
        db.drop().await;
    }

    /// A transfer the index holds in a shape the producer cannot read rejects
    /// its own claim; the rest of the batch is still settled.
    #[tokio::test]
    async fn a_malformed_transfer_rejects_only_its_claim() {
        let Some(db) = crate::market::tests::support::OwnDb::create("tips_malformed").await else {
            return;
        };
        let pool = &db.pool;
        let settings = TipSettings::default();
        let t = now_secs();
        let one = "1000000000000000000";
        claim(pool, &settings, &hash(1), TIPPER).await.unwrap();
        claim(pool, &settings, &hash(2), TIPPER).await.unwrap();
        sqlx::query("ALTER TABLE shop.mana_transfer DROP CONSTRAINT mana_transfer_recipient_check")
            .execute(pool)
            .await
            .unwrap();
        transfer(pool, 10, 0, &hash(1), TIPPER, "0xnot-an-address", one, t).await;
        transfer(pool, 11, 0, &hash(2), TIPPER, RECEIVER, one, t).await;
        publish(pool, 100).await;
        let mut outcomes = resolve(pool, &settings, None).await.unwrap();
        outcomes.sort_by(|a, b| a.0.cmp(&b.0));
        assert_eq!(
            outcomes,
            vec![
                (hash(1), Outcome::Rejected(Rejection::Malformed)),
                (hash(2), Outcome::Notified)
            ]
        );
        assert_eq!(tip_count(pool).await, 1);
        db.drop().await;
    }

    #[test]
    fn the_tip_settings_parse() {
        let d = TipSettings::default();
        assert_eq!(d.min_amount_wei, U256::from(10u64).pow(U256::from(18u64)));
        assert_eq!((d.max_age_secs, d.max_pending), (86_400, 2_000));
        assert!(TipSettings::parse("1e18", 1, 1).is_err());
    }

    #[tokio::test]
    async fn an_unsigned_forward_is_refused() {
        use tower::ServiceExt;
        let app = Router::new().nest(
            "/events-notifier",
            router(crate::market::tests::support::state_without_db()),
        );
        let res = app
            .oneshot(
                axum::http::Request::post("/events-notifier/forward-blockchain")
                    .body(axum::body::Body::from("{}"))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
    }

    #[test]
    fn the_body_keeps_the_old_schema() {
        let tx = format!("0x{}", "Ab".repeat(32));
        assert_eq!(
            parse_body(format!(r#"{{"type":"tip-received","transactionHash":"{tx}"}}"#).as_bytes()),
            Ok(tx.to_ascii_lowercase())
        );
        assert!(parse_body(br#"{"type":"tip-received","transactionHash":"0x12"}"#).is_err());
        assert!(
            parse_body(format!(r#"{{"type":"other","transactionHash":"{tx}"}}"#).as_bytes())
                .is_err()
        );
        assert!(
            parse_body(
                format!(r#"{{"type":"tip-received","transactionHash":"{tx}","x":1}}"#).as_bytes()
            )
            .is_err()
        );
    }
}
