//! The broker reconciler: a `jobs::spawn_periodic` pass over the unsettled
//! rows of the three ledgers (bounded, durable sweeps in `id` order,
//! `LIMIT 100`, older than two minutes as of the pass clock) that reads
//! their receipts and moves them to the status the receipt implies.

use std::sync::Arc;

use chrono::NaiveDateTime;
use sqlx::Row;

use super::calls::PurchaseMode;
use crate::evm::signer::{DirectSigner, ReceiptOutcome};
use crate::evm::{State, now};
use crate::jobs::JobContext;

const DUE_BATCH: i64 = 100;
const SETTLE_GRACE_SECS: i64 = 120;

pub fn next_broker_status(
    current: &str,
    mode: PurchaseMode,
    outcome: ReceiptOutcome,
) -> Option<&'static str> {
    match (current, outcome) {
        ("sent", ReceiptOutcome::Confirmed) if mode.has_forward_leg() => Some("bought"),
        ("sent", ReceiptOutcome::Confirmed) => Some("confirmed"),
        ("sent", ReceiptOutcome::Reverted) => Some("reverted"),
        ("forwarding", ReceiptOutcome::Confirmed) => Some("confirmed"),
        ("forwarding", ReceiptOutcome::Reverted) => Some("reverted"),
        _ => None,
    }
}

fn pollable_hash(
    status: &str,
    tx_hash: Option<&str>,
    forward_tx_hash: Option<&str>,
) -> Option<String> {
    match status {
        "sent" => tx_hash.map(str::to_string),
        "forwarding" => forward_tx_hash.map(str::to_string),
        _ => None,
    }
}

fn due_before(pass_clock: NaiveDateTime) -> NaiveDateTime {
    pass_clock - chrono::Duration::seconds(SETTLE_GRACE_SECS)
}

// Only these local ledgers can reach the SQL identifiers below.
async fn receipt_batch(
    pool: &sqlx::PgPool,
    ledger: &'static str,
    chain: i64,
    pass_clock: NaiveDateTime,
) -> sqlx::Result<Vec<sqlx::postgres::PgRow>> {
    let (columns, predicate) = match ledger {
        "broker_purchases" => (
            "id, idempotency_key, status, mode, tx_hash, forward_tx_hash",
            "status IN ('sent', 'forwarding')",
        ),
        "name_transfers" => ("id, idempotency_key, tx_hash", "status = 'sent'"),
        "escrow_actions" => ("id, tx_hash", "status = 'sent' AND tx_hash IS NOT NULL"),
        _ => unreachable!("receipt ledger is a local constant"),
    };
    let mut sweep = crate::worker_progress::Sweep::begin(pool, ledger, chain).await?;
    let mut rows = Vec::new();
    for _ in 0..2 {
        if sweep.position >= sweep.ceiling {
            sweep.position = 0;
            sweep.ceiling = sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
                "SELECT COALESCE(max(id), 0) FROM shop.{ledger} WHERE {predicate} AND chain_id = $1 AND updated_at < $2"
            ))).bind(chain).bind(due_before(pass_clock)).fetch_one(&mut *sweep.tx).await?;
        }
        rows = sqlx::query(sqlx::AssertSqlSafe(format!(
            "SELECT {columns} FROM shop.{ledger} WHERE {predicate} AND chain_id = $1 AND updated_at < $2 AND id > $3 AND id <= $4 ORDER BY id LIMIT $5"
        ))).bind(chain).bind(due_before(pass_clock)).bind(sweep.position).bind(sweep.ceiling).bind(DUE_BATCH)
            .fetch_all(&mut *sweep.tx).await?;
        if let Some(last) = rows.last() {
            sweep.position = last.get("id");
            break;
        }
        // Completed/deleted rows can exhaust a cohort before its ceiling.
        sweep.position = sweep.ceiling;
    }
    sweep.commit().await?;
    Ok(rows)
}

pub async fn reconcile_once(
    state: &State,
    signer: &DirectSigner,
    pass_clock: NaiveDateTime,
) -> Result<u64, sqlx::Error> {
    let rows = receipt_batch(
        &state.pool,
        "broker_purchases",
        crate::types::chain_id_column(signer.chain_id())?,
        pass_clock,
    )
    .await?;

    let mut advanced = 0u64;
    for r in rows {
        let key: Option<String> = r.get("idempotency_key");
        let status: String = r.get("status");
        let mode_raw: String = r.get("mode");
        let tx_hash: Option<String> = r.get("tx_hash");
        let forward_tx_hash: Option<String> = r.get("forward_tx_hash");
        let Some(key) = key else { continue };
        let Some(mode) = PurchaseMode::from_db_str(&mode_raw) else {
            tracing::warn!(idempotency_key = %key, mode = %mode_raw, "reconcile: unknown purchase mode; skipping row");
            continue;
        };
        let Some(hash) = pollable_hash(&status, tx_hash.as_deref(), forward_tx_hash.as_deref())
        else {
            continue;
        };
        let outcome = match signer.await_receipt(&hash).await {
            Ok(o) => o,
            Err(e) => {
                tracing::warn!(idempotency_key = %key, error = %e, "reconcile: receipt poll failed; will retry next tick");
                continue;
            }
        };
        if let Some(new_status) = next_broker_status(&status, mode, outcome) {
            sqlx::query(
                "UPDATE shop.broker_purchases SET status = $2, updated_at = timezone('UTC', now()) \
                 WHERE idempotency_key = $1 AND status = $3",
            )
            .bind(&key)
            .bind(new_status)
            .bind(&status)
            .execute(&state.pool)
            .await?;
            advanced += 1;
            tracing::info!(idempotency_key = %key, from = %status, to = %new_status, tx_hash = %hash, "reconcile: broker buy advanced from receipt");
        }
    }
    Ok(advanced)
}

pub async fn reconcile_name_transfers_once(
    state: &State,
    signer: &DirectSigner,
    pass_clock: NaiveDateTime,
) -> Result<u64, sqlx::Error> {
    let rows = receipt_batch(
        &state.pool,
        "name_transfers",
        crate::types::chain_id_column(signer.chain_id())?,
        pass_clock,
    )
    .await?;

    let mut advanced = 0u64;
    for r in rows {
        let key: String = r.get("idempotency_key");
        let hash: Option<String> = r.get("tx_hash");
        let Some(hash) = hash else { continue };
        let outcome = match signer.await_receipt(&hash).await {
            Ok(o) => o,
            Err(e) => {
                tracing::warn!(idempotency_key = %key, error = %e, "reconcile: name transfer receipt poll failed; will retry next tick");
                continue;
            }
        };
        let new_status = match outcome {
            ReceiptOutcome::Confirmed => "confirmed",
            ReceiptOutcome::Reverted => "reverted",
            ReceiptOutcome::Pending => continue,
        };
        sqlx::query(
            "UPDATE shop.name_transfers SET status = $2, updated_at = timezone('UTC', now()) \
             WHERE idempotency_key = $1 AND status = 'sent'",
        )
        .bind(&key)
        .bind(new_status)
        .execute(&state.pool)
        .await?;
        advanced += 1;
        tracing::info!(idempotency_key = %key, to = %new_status, tx_hash = %hash, "reconcile: name transfer settled from receipt");
    }
    Ok(advanced)
}

pub async fn reconcile_escrow_actions_once(
    state: &State,
    signer: &DirectSigner,
    pass_clock: NaiveDateTime,
) -> Result<u64, sqlx::Error> {
    let rows = receipt_batch(
        &state.pool,
        "escrow_actions",
        crate::types::chain_id_column(signer.chain_id())?,
        pass_clock,
    )
    .await?;

    let mut advanced = 0u64;
    for r in rows {
        let id: i64 = r.get("id");
        let hash: Option<String> = r.get("tx_hash");
        let Some(hash) = hash else { continue };
        let outcome = match signer.await_receipt(&hash).await {
            Ok(o) => o,
            Err(e) => {
                tracing::warn!(escrow_action_id = id, error = %e, "reconcile: escrow action receipt poll failed; will retry next tick");
                continue;
            }
        };
        let new_status = match outcome {
            ReceiptOutcome::Confirmed => "confirmed",
            ReceiptOutcome::Reverted => "reverted",
            ReceiptOutcome::Pending => continue,
        };
        sqlx::query(
            "UPDATE shop.escrow_actions SET status = $2, updated_at = timezone('UTC', now()) \
             WHERE id = $1 AND status = 'sent'",
        )
        .bind(id)
        .bind(new_status)
        .execute(&state.pool)
        .await?;
        advanced += 1;
        tracing::info!(escrow_action_id = id, to = %new_status, tx_hash = %hash, "reconcile: escrow action settled from receipt");
    }
    Ok(advanced)
}

/// One pass of the `broker-reconcile` job: the Polygon signer settles buys
/// and escrow actions, the Ethereum signer settles NAME buys and transfers.
pub async fn run_pass(state: &Arc<State>, ctx: &JobContext) -> anyhow::Result<()> {
    let pass_clock = now();
    let mut advanced = 0u64;
    if let Some(signer) = state.transaction.direct_signer() {
        advanced += reconcile_once(state, signer, pass_clock).await?;
        advanced += reconcile_escrow_actions_once(state, signer, pass_clock).await?;
    }
    if let Some(signer) = state.eth_signer.as_ref() {
        advanced += reconcile_once(state, signer, pass_clock).await?;
        advanced += reconcile_name_transfers_once(state, signer, pass_clock).await?;
    }
    if advanced > 0 {
        tracing::info!(advanced, "broker reconcile pass");
    }
    ctx.progress(format!("advanced {advanced} rows"));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn receipt_sweeps_resume_and_retry_a_blocked_prefix() {
        let Some(db) = crate::test_db::OwnDatabase::create("receipt_sweeps").await else {
            return;
        };
        let clock = chrono::Utc::now().naive_utc();
        for (ledger, insert) in [
            (
                "broker_purchases",
                "INSERT INTO shop.broker_purchases (collection, escrow_address, price_wei, chain_id, mode, updated_at) SELECT '0x00', '0x00', 0, 137, 'primary', timezone('UTC', now()) - interval '1 hour' FROM generate_series(1, 101)",
            ),
            (
                "name_transfers",
                "INSERT INTO shop.name_transfers (idempotency_key, registrar, token_id, from_address, to_address, chain_id, status, updated_at) SELECT 'transfer-' || n, '0x00', n::text, '0x00', '0x00', 137, 'sent', timezone('UTC', now()) - interval '1 hour' FROM generate_series(1, 101) n",
            ),
            (
                "escrow_actions",
                "INSERT INTO shop.escrow_actions (action, chain_id, tx_hash, updated_at) SELECT 'release', 137, 'pending-' || n, timezone('UTC', now()) - interval '1 hour' FROM generate_series(1, 101) n",
            ),
        ] {
            sqlx::query(sqlx::AssertSqlSafe(insert))
                .execute(&db.pool)
                .await
                .unwrap();
            sqlx::query(sqlx::AssertSqlSafe(format!(
                "UPDATE shop.{ledger} SET tx_hash = 'ready' WHERE id = 101"
            )))
            .execute(&db.pool)
            .await
            .unwrap();
            let first = receipt_batch(&db.pool, ledger, 137, clock).await.unwrap();
            assert_eq!(first.len(), 100);
            // Simulate a crash after selection: no outcome is written. A fresh
            // call reads persisted progress and reaches the ready tail.
            let tail = receipt_batch(&db.pool, ledger, 137, clock).await.unwrap();
            assert_eq!(tail.len(), 1);
            assert_eq!(tail[0].get::<String, _>("tx_hash"), "ready");
            assert!(
                receipt_batch(&db.pool, ledger, 1, clock)
                    .await
                    .unwrap()
                    .is_empty()
            );
            let retry = receipt_batch(&db.pool, ledger, 137, clock).await.unwrap();
            assert_eq!(retry[0].get::<i64, _>("id"), 1);
            // Temporarily make a prefix row ineligible; it is rediscovered
            // after becoming due again, without changing its durable identity.
            sqlx::query(sqlx::AssertSqlSafe(format!(
                "UPDATE shop.{ledger} SET updated_at = $1 WHERE id = 1"
            )))
            .bind(clock)
            .execute(&db.pool)
            .await
            .unwrap();
            assert_eq!(
                receipt_batch(&db.pool, ledger, 137, clock).await.unwrap()[0].get::<i64, _>("id"),
                101
            );
            sqlx::query(sqlx::AssertSqlSafe(format!(
                "UPDATE shop.{ledger} SET updated_at = $1 WHERE id = 1"
            )))
            .bind(clock - chrono::Duration::hours(1))
            .execute(&db.pool)
            .await
            .unwrap();
            assert_eq!(
                receipt_batch(&db.pool, ledger, 137, clock).await.unwrap()[0].get::<i64, _>("id"),
                1
            );
        }
        db.drop().await;
    }

    #[test]
    fn status_transitions_are_correct() {
        use PurchaseMode::{Primary, Secondary};
        assert_eq!(
            next_broker_status("sent", Secondary, ReceiptOutcome::Confirmed),
            Some("bought")
        );
        assert_eq!(
            next_broker_status("sent", Primary, ReceiptOutcome::Reverted),
            Some("reverted")
        );
        assert_eq!(
            next_broker_status("forwarding", Primary, ReceiptOutcome::Confirmed),
            Some("confirmed")
        );
        assert_eq!(
            next_broker_status("forwarding", Secondary, ReceiptOutcome::Reverted),
            Some("reverted")
        );
        assert_eq!(
            next_broker_status("sent", Primary, ReceiptOutcome::Pending),
            None
        );
        assert_eq!(
            next_broker_status("forwarding", Secondary, ReceiptOutcome::Pending),
            None
        );
        assert_eq!(
            next_broker_status("confirmed", Secondary, ReceiptOutcome::Confirmed),
            None
        );
        assert_eq!(
            next_broker_status("bought", Primary, ReceiptOutcome::Confirmed),
            None
        );
    }

    #[test]
    fn name_modes_confirm_straight_from_sent() {
        use PurchaseMode::{NameMint, NameSecondary};
        assert_eq!(
            next_broker_status("sent", NameMint, ReceiptOutcome::Confirmed),
            Some("confirmed")
        );
        assert_eq!(
            next_broker_status("sent", NameSecondary, ReceiptOutcome::Confirmed),
            Some("confirmed")
        );
        assert_eq!(
            next_broker_status("sent", NameMint, ReceiptOutcome::Reverted),
            Some("reverted")
        );
        assert_eq!(
            next_broker_status("sent", NameSecondary, ReceiptOutcome::Pending),
            None
        );
    }

    #[test]
    fn pollable_hash_picks_the_right_tx() {
        assert_eq!(
            pollable_hash("sent", Some("0xbuy"), Some("0xfwd")),
            Some("0xbuy".to_string())
        );
        assert_eq!(
            pollable_hash("forwarding", Some("0xbuy"), Some("0xfwd")),
            Some("0xfwd".to_string())
        );
        assert_eq!(pollable_hash("sent", None, Some("0xfwd")), None);
        assert_eq!(pollable_hash("bought", Some("0xbuy"), None), None);
    }

    #[test]
    fn the_due_set_is_two_minutes_behind_the_pass_clock() {
        let clock = chrono::DateTime::from_timestamp(1_700_000_000, 0)
            .unwrap()
            .naive_utc();
        assert_eq!((clock - due_before(clock)).num_seconds(), 120);
    }
}
