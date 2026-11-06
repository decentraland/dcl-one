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
//! an hour. The notification id is a uuid v5 of the transaction hash.

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
const POLL_SECS: u64 = 15;

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
    match claim(&state.pool, &tx, &signer).await {
        Ok(Outcome::Notified) => StatusCode::NO_CONTENT.into_response(),
        Ok(Outcome::Pending) => (StatusCode::ACCEPTED, Json(json!({}))).into_response(),
        Ok(Outcome::TooMany) => error(
            StatusCode::TOO_MANY_REQUESTS,
            "Too many tips waiting for confirmation",
        ),
        Ok(Outcome::Rejected) => error(
            StatusCode::BAD_REQUEST,
            format!("Transaction is not a single MANA transfer from the signer: {tx}"),
        ),
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
    Rejected,
    TooMany,
}

/// Records the claim and resolves it at once if the index already has it.
pub async fn claim(pool: &PgPool, tx: &str, sender: &str) -> anyhow::Result<Outcome> {
    let inserted = sqlx::query(
        "INSERT INTO shop.tip_claim (transaction_hash, sender) \
         SELECT $1, $2 WHERE (SELECT count(*) FROM shop.tip_claim WHERE sender = $2) < $3 \
         ON CONFLICT DO NOTHING",
    )
    .bind(tx)
    .bind(sender)
    .bind(MAX_PENDING_PER_SENDER)
    .execute(pool)
    .await?
    .rows_affected();
    if inserted == 0 {
        let exists: bool = sqlx::query_scalar(
            "SELECT EXISTS (SELECT 1 FROM shop.tip_claim WHERE transaction_hash = $1 AND sender = $2)",
        )
        .bind(tx)
        .bind(sender)
        .fetch_one(pool)
        .await?;
        if !exists {
            return Ok(Outcome::TooMany);
        }
    }
    let resolved = resolve(pool, Some((tx, sender))).await?;
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

/// Settles the claims (one, or all with `None`) the published index can
/// answer for: a single transfer from the signer notifies its recipient, any
/// other shape is rejected, none yet leaves the claim pending.
pub async fn resolve(
    pool: &PgPool,
    only: Option<(&str, &str)>,
) -> anyhow::Result<Vec<(String, Outcome)>> {
    let now = chrono::Utc::now();
    let Some(cursor) = chain_walk::ready_cursor(pool, INDEXER_MANA, NET, now).await else {
        return Ok(Vec::new());
    };
    let mut txn = pool.begin().await?;
    let rows: Vec<ClaimRow> = sqlx::query_as(
        "SELECT c.transaction_hash, c.sender, c.created_at, t.recipient, t.amount::text AS amount \
         FROM (SELECT * FROM shop.tip_claim \
               WHERE $3::text IS NULL OR (transaction_hash = $3 AND sender = $4) \
               ORDER BY transaction_hash, sender FOR UPDATE SKIP LOCKED) c \
         LEFT JOIN shop.mana_transfer t ON t.network = $1 AND t.sender = c.sender \
              AND t.transaction_hash = c.transaction_hash AND t.block_number <= $2 \
         ORDER BY c.transaction_hash, c.sender, t.log_index",
    )
    .bind(NET.db_name())
    .bind(cursor.last_block)
    .bind(only.map(|(tx, _)| tx))
    .bind(only.map(|(_, s)| s))
    .fetch_all(&mut *txn)
    .await?;
    let mut outcomes = Vec::new();
    let mut notifications = Vec::new();
    let mut settled: Vec<(String, String)> = Vec::new();
    for group in
        rows.chunk_by(|a, b| (&a.transaction_hash, &a.sender) == (&b.transaction_hash, &b.sender))
    {
        let first = &group[0];
        let transfers: Vec<(&str, &str)> = group
            .iter()
            .filter_map(|r| Some((r.recipient.as_deref()?, r.amount.as_deref()?)))
            .collect();
        let outcome = match transfers.as_slice() {
            [] => Outcome::Pending,
            [(recipient, amount)] if *recipient != first.sender => {
                let (Ok(address), Some(mana)) = (Address::parse(recipient), format_mana(amount))
                else {
                    anyhow::bail!("malformed indexed transfer for {}", first.transaction_hash);
                };
                notifications.push(NewNotification {
                    id: notification_id(&first.transaction_hash),
                    address,
                    kind: KIND.into(),
                    metadata: metadata(&first.sender, recipient, &mana),
                    occurred_at: first.created_at,
                });
                Outcome::Notified
            }
            _ => Outcome::Rejected,
        };
        if outcome != Outcome::Pending {
            settled.push((first.transaction_hash.clone(), first.sender.clone()));
        }
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
            let pool = pool.clone();
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
                let outcomes = resolve(&pool, None).await?;
                let notified = outcomes
                    .iter()
                    .filter(|(_, o)| *o == Outcome::Notified)
                    .count();
                if notified > 0 || expired > 0 {
                    tracing::info!(notified, expired, "tip notifications");
                }
                ctx.progress(format!(
                    "{notified} notified, {} waiting, {expired} expired",
                    outcomes.iter().filter(|(_, o)| *o == Outcome::Pending).count()
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

    /// Pending until the published index holds the transfer, then the
    /// recipient is told once; unpublished blocks, a stranger's claim, a
    /// multi-transfer transaction and the per-sender cap are refused.
    #[tokio::test]
    async fn a_tip_notifies_its_recipient_once_the_index_publishes_it() {
        let Some(db) = crate::market::tests::support::OwnDb::create("tips").await else {
            return;
        };
        let pool = &db.pool;
        let tipper = "0x00000000000000000000000000000000000000a1";
        let receiver = "0x00000000000000000000000000000000000000b2";
        let hash = |n: u8| format!("0x{:064x}", n);
        let transfer = |block: i64, log: i32, tx: String, from: &'static str| async move {
            sqlx::query(
                "INSERT INTO shop.mana_transfer (network, block_number, log_index, transaction_hash, sender, recipient, amount, timestamp) \
                 VALUES ('matic', $1, $2, $3, $4, $5, 2500000000000000000, 1700000000)",
            )
            .bind(block)
            .bind(log)
            .bind(tx)
            .bind(from)
            .bind(receiver)
            .execute(pool)
            .await
            .unwrap();
        };
        let count = || async {
            sqlx::query_scalar::<_, i64>(
                "SELECT count(*) FROM social.notifications WHERE type = 'tip_received'",
            )
            .fetch_one(pool)
            .await
            .unwrap()
        };
        transfer(100, 0, hash(1), tipper).await;
        assert_eq!(
            claim(pool, &hash(1), tipper).await.unwrap(),
            Outcome::Pending,
            "an index that is not ready answers nothing"
        );
        sqlx::query(
            "INSERT INTO shop.chain_cursor (indexer, network, start_block, last_block, folded_block, folded_at, full_history) \
             VALUES ('mana_transfers', 'matic', 0, 150, 150, timezone('UTC', now()), true)",
        )
        .execute(pool)
        .await
        .unwrap();
        let outcomes = resolve(pool, None).await.unwrap();
        assert_eq!(outcomes, vec![(hash(1), Outcome::Notified)]);
        assert_eq!(
            claim(pool, &hash(1), tipper).await.unwrap(),
            Outcome::Notified,
            "a repeated forward is idempotent"
        );
        assert_eq!(count().await, 1);
        let (address, metadata, id): (String, Value, Uuid) = sqlx::query_as(
            "SELECT address, metadata, id FROM social.notifications WHERE type = 'tip_received'",
        )
        .fetch_one(pool)
        .await
        .unwrap();
        assert_eq!(
            (address.as_str(), id),
            (receiver, notification_id(&hash(1)))
        );
        assert_eq!(
            metadata,
            json!({"amount": "2.5", "manaAmount": "2500000000000000000", "senderAddress": tipper, "receiverAddress": receiver})
        );

        transfer(200, 0, hash(2), tipper).await;
        assert_eq!(
            claim(pool, &hash(2), tipper).await.unwrap(),
            Outcome::Pending,
            "past the published block"
        );
        transfer(120, 0, hash(3), tipper).await;
        assert_eq!(
            claim(pool, &hash(3), "0x00000000000000000000000000000000000000c3")
                .await
                .unwrap(),
            Outcome::Pending,
            "someone else's transfer never matches"
        );
        transfer(130, 0, hash(4), tipper).await;
        transfer(130, 1, hash(4), tipper).await;
        assert_eq!(
            claim(pool, &hash(4), tipper).await.unwrap(),
            Outcome::Rejected
        );
        assert_eq!(count().await, 1);

        sqlx::query("UPDATE shop.tip_claim SET created_at = created_at - interval '2 hours'")
            .execute(pool)
            .await
            .unwrap();
        assert_eq!(expire(pool).await.unwrap(), 2);
        for n in 10..10 + MAX_PENDING_PER_SENDER as u8 {
            assert_eq!(
                claim(pool, &hash(n), tipper).await.unwrap(),
                Outcome::Pending
            );
        }
        assert_eq!(
            claim(pool, &hash(99), tipper).await.unwrap(),
            Outcome::TooMany
        );
        db.drop().await;
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
