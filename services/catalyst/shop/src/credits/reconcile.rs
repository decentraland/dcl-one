//! `/admin/reconcile`: the invariants the ledger design rests on, asserted
//! in SQL. `available` and `earned_available` must equal the signed ledger
//! sums per wallet (a `FULL OUTER JOIN` of the per-address sums against
//! `credits.wallets`), paid purchases must equal the granted `purchase`
//! rows, a fulfilled checkout's total must equal its confirmed lines, and
//! every confirmed line must have its escrow lease (`shop.usage_grants`,
//! a local table now). Drift is never tolerated: `realign_ledger_to_balances`
//! records it as an explicit adjustment row, walked in keyset chunks so the
//! widest lock it ever holds is one chunk.
//!
//! The CreditUsed pass ([`super::State::reconcile_report`]): the full fold
//! of the usage index (`usage::fold` over every indexed log, not just the
//! incremental range the job folds), so a debit the job could not make is
//! retried and every pledge still unpaid is listed (`consumed: false`);
//! `creditUsageIndex` says `unavailable` (not configured), `not_ready` or
//! `ready`, and `writes_disabled` when `OPS_WRITES_ENABLED` is off (the
//! fold debits the ledger, so it does not run).

use std::collections::HashSet;

use serde::Serialize;
use serde_json::json;
use sqlx::Row;

use super::errors::ApiError;
use super::store::{AuditEntry, Credits, SYSTEM_ACTOR};

#[derive(Debug, Serialize)]
pub struct LedgerBalanceDiff {
    pub address: String,
    #[serde(rename = "ledgerSum")]
    pub ledger_sum: String,
    pub available: String,
}

#[derive(Debug, Serialize)]
pub struct PurchaseGrantDiff {
    pub address: String,
    #[serde(rename = "paidCredits")]
    pub paid_credits: String,
    #[serde(rename = "grantedCredits")]
    pub granted_credits: String,
}

#[derive(Debug, Serialize)]
pub struct CheckoutFulfillmentDiff {
    #[serde(rename = "checkoutId")]
    pub checkout_id: i64,
    #[serde(rename = "totalCredits")]
    pub total_credits: String,
    #[serde(rename = "confirmedSum")]
    pub confirmed_sum: String,
}

#[derive(Debug, Serialize)]
pub struct EscrowHoldings {
    pub available: bool,
    pub active: i64,
    pub revoked: i64,
    pub released: i64,
}

#[derive(Debug, Serialize)]
pub struct EscrowGrantDiff {
    #[serde(rename = "checkoutId")]
    pub checkout_id: i64,
    #[serde(rename = "outboxId")]
    pub outbox_id: i64,
    pub address: String,
    pub urn: String,
    #[serde(rename = "tokenId")]
    pub token_id: Option<String>,
    pub collection: Option<String>,
    #[serde(rename = "escrowRef")]
    pub escrow_ref: String,
    #[serde(rename = "unitPriceCredits")]
    pub unit_price_credits: String,
}

#[derive(Debug, Serialize)]
pub struct CreditUsageDiff {
    #[serde(rename = "creditId")]
    pub credit_id: String,
    pub address: String,
    pub debited: String,
    pub consumed: bool,
    pub replayed: bool,
    #[serde(rename = "newBalance")]
    pub new_balance: String,
    /// `purchase` (the group's charged cents) or `remainder` (a later use of the same credit).
    pub kind: String,
}

#[derive(Debug, Serialize)]
pub struct ReconcileReport {
    pub ok: bool,
    #[serde(rename = "ledgerBalanceMismatches")]
    pub ledger_balance_mismatches: Vec<LedgerBalanceDiff>,
    #[serde(rename = "earnedBalanceMismatches")]
    pub earned_balance_mismatches: Vec<LedgerBalanceDiff>,
    #[serde(rename = "purchaseGrantMismatches")]
    pub purchase_grant_mismatches: Vec<PurchaseGrantDiff>,
    #[serde(rename = "checkoutFulfillmentMismatches")]
    pub checkout_fulfillment_mismatches: Vec<CheckoutFulfillmentDiff>,
    #[serde(rename = "escrowHoldings")]
    pub escrow_holdings: EscrowHoldings,
    #[serde(rename = "escrowGrantMismatches")]
    pub escrow_grant_mismatches: Vec<EscrowGrantDiff>,
    #[serde(rename = "creditUsagesReconciled")]
    pub credit_usages_reconciled: Vec<CreditUsageDiff>,
    /// `unavailable` | `not_ready` | `ready` | `writes_disabled` (the CreditUsed index).
    #[serde(rename = "creditUsageIndex")]
    pub credit_usage_index: &'static str,
}

const RECONCILE_LIMIT: i64 = 500;

/// Wallets locked per transaction by [`Credits::realign_ledger_to_balances`].
const REALIGN_BATCH: i64 = 500;

/// `tx_ref` stamped on every adjustment row the realignment writes, so the
/// adjustments are greppable and separable from real money movement.
pub const REALIGN_TX_REF: &str = "reconcile:ledger-realign";

#[derive(Debug, Serialize)]
pub struct LedgerRealignment {
    pub address: String,
    pub bucket: String,
    pub kind: String,
    pub amount: String,
}

impl Credits {
    pub async fn reconcile(&self) -> Result<ReconcileReport, ApiError> {
        let (
            ledger_balance_mismatches,
            earned_balance_mismatches,
            purchase_grant_mismatches,
            checkout_fulfillment_mismatches,
            escrow_holdings,
            escrow_grant_mismatches,
        ) = tokio::try_join!(
            self.reconcile_ledger_balance(),
            self.reconcile_earned_balance(),
            self.reconcile_purchase_grant(),
            self.reconcile_checkout_fulfillment(),
            self.reconcile_escrow_holdings(),
            self.reconcile_escrow_grants(),
        )?;

        if !ledger_balance_mismatches.is_empty() {
            tracing::error!(
                invariant = "ledger_sum==available",
                count = ledger_balance_mismatches.len(),
                addresses = ?ledger_balance_mismatches.iter().map(|d| d.address.as_str()).collect::<Vec<_>>(),
                "RECONCILE ALERT: signed credits.ledger sum does not equal credits.wallets.available"
            );
        }
        if !earned_balance_mismatches.is_empty() {
            tracing::error!(
                invariant = "earned_ledger_sum==earned_available",
                count = earned_balance_mismatches.len(),
                addresses = ?earned_balance_mismatches.iter().map(|d| d.address.as_str()).collect::<Vec<_>>(),
                "RECONCILE ALERT: signed earned-bucket ledger sum does not equal credits.wallets.earned_available"
            );
        }
        if !purchase_grant_mismatches.is_empty() {
            tracing::error!(
                invariant = "paid_purchases==purchase_ledger",
                count = purchase_grant_mismatches.len(),
                "RECONCILE ALERT: paid Stripe purchases do not equal granted 'purchase' Credits"
            );
        }
        if !checkout_fulfillment_mismatches.is_empty() {
            tracing::error!(
                invariant = "checkout_total==confirmed_sum",
                count = checkout_fulfillment_mismatches.len(),
                "RECONCILE ALERT: fulfilled checkout totals do not equal confirmed line sums"
            );
        }
        if !escrow_grant_mismatches.is_empty() {
            tracing::error!(
                invariant = "confirmed_line==usage_grant",
                count = escrow_grant_mismatches.len(),
                escrow_refs = ?escrow_grant_mismatches.iter().map(|d| d.escrow_ref.as_str()).collect::<Vec<_>>(),
                "RECONCILE ALERT: confirmed fulfilment lines (Credits spent, escrow minted) lack a usage_grant \u{2014} the lease overlay is missing and must be re-granted"
            );
        }

        let ok = ledger_balance_mismatches.is_empty()
            && earned_balance_mismatches.is_empty()
            && purchase_grant_mismatches.is_empty()
            && checkout_fulfillment_mismatches.is_empty()
            && escrow_grant_mismatches.is_empty();

        Ok(ReconcileReport {
            ok,
            ledger_balance_mismatches,
            earned_balance_mismatches,
            purchase_grant_mismatches,
            checkout_fulfillment_mismatches,
            escrow_holdings,
            escrow_grant_mismatches,
            credit_usages_reconciled: Vec::new(),
            credit_usage_index: "unavailable",
        })
    }

    /// Records every divergence between a wallet's stored balances and its
    /// ledger sums as an explicit adjustment row (`grant`/`consume`, tx_ref
    /// [`REALIGN_TX_REF`]) so replay reproduces the balance: an equality
    /// invariant with a permanent carve-out stops being an invariant.
    /// Idempotent, batched in keyset chunks, each chunk its own transaction.
    pub async fn realign_ledger_to_balances(
        &self,
        reason: &str,
    ) -> Result<Vec<LedgerRealignment>, ApiError> {
        self.realign_ledger_to_balances_in_batches(reason, REALIGN_BATCH)
            .await
    }

    /// [`Self::realign_ledger_to_balances`] with an explicit chunk size, so
    /// tests can drive the multi-chunk path without seeding thousands of wallets.
    pub async fn realign_ledger_to_balances_in_batches(
        &self,
        reason: &str,
        batch: i64,
    ) -> Result<Vec<LedgerRealignment>, ApiError> {
        let batch = batch.max(1);
        let mut cursor = String::new();
        let mut out: Vec<LedgerRealignment> = Vec::new();
        let mut wallets_scanned: u64 = 0;
        let mut chunks: u64 = 0;

        loop {
            let mut tx = self.pool.begin().await?;

            let addresses: Vec<String> = sqlx::query_scalar(
                "SELECT address FROM credits.wallets WHERE address > $1 ORDER BY address LIMIT $2 FOR UPDATE",
            )
            .bind(&cursor)
            .bind(batch)
            .fetch_all(&mut *tx)
            .await?;

            if addresses.is_empty() {
                tx.rollback().await?;
                break;
            }
            cursor = addresses[addresses.len() - 1].clone();
            wallets_scanned += addresses.len() as u64;
            chunks += 1;

            let earned = sqlx::query(
                "WITH e AS ( \
                     SELECT u.address, \
                            u.earned_available - COALESCE(( \
                                SELECT SUM(CASE \
                                    WHEN l.kind IN ('grant','refund','purchase','claim') THEN l.amount \
                                    WHEN l.kind IN ('spend','consume','expire') THEN -l.amount \
                                    ELSE 0 END) \
                                FROM credits.ledger l \
                                WHERE l.address = u.address AND l.bucket = 'earned' \
                            ), 0) AS delta \
                     FROM credits.wallets u WHERE u.address = ANY($2) \
                 ) \
                 INSERT INTO credits.ledger (address, kind, amount, tx_ref, bucket, captcha_ok) \
                 SELECT address, CASE WHEN delta > 0 THEN 'grant' ELSE 'consume' END, abs(delta), $1, 'earned', FALSE \
                 FROM e WHERE delta <> 0 \
                 RETURNING address, amount::text AS amount, kind",
            )
            .bind(REALIGN_TX_REF)
            .bind(&addresses)
            .fetch_all(&mut *tx)
            .await?;

            let total = sqlx::query(
                "WITH t AS ( \
                     SELECT u.address, \
                            u.available - COALESCE(( \
                                SELECT SUM(CASE \
                                    WHEN l.kind IN ('grant','refund','purchase','claim') THEN l.amount \
                                    WHEN l.kind IN ('spend','consume','expire') THEN -l.amount \
                                    ELSE 0 END) \
                                FROM credits.ledger l \
                                WHERE l.address = u.address \
                            ), 0) AS delta \
                     FROM credits.wallets u WHERE u.address = ANY($2) \
                 ) \
                 INSERT INTO credits.ledger (address, kind, amount, tx_ref, bucket, captcha_ok) \
                 SELECT address, CASE WHEN delta > 0 THEN 'grant' ELSE 'consume' END, abs(delta), $1, 'paid', FALSE \
                 FROM t WHERE delta <> 0 \
                 RETURNING address, amount::text AS amount, kind",
            )
            .bind(REALIGN_TX_REF)
            .bind(&addresses)
            .fetch_all(&mut *tx)
            .await?;

            let mut applied_here = 0usize;
            for (rows, bucket) in [(earned, "earned"), (total, "paid")] {
                for r in rows {
                    let kind: String = r.get("kind");
                    let amount: String = r.get("amount");
                    let address: String = r.get("address");
                    let detail = json!({ "bucket": bucket, "kind": kind, "amount": amount, "txRef": REALIGN_TX_REF, "reason": reason });
                    Self::audit(
                        &mut *tx,
                        AuditEntry {
                            action: "ledger_realign",
                            actor: SYSTEM_ACTOR,
                            target: Some(&address),
                            detail: &detail,
                        },
                    )
                    .await?;
                    applied_here += 1;
                    out.push(LedgerRealignment {
                        address,
                        bucket: bucket.to_string(),
                        kind,
                        amount,
                    });
                }
            }

            tx.commit().await?;

            tracing::info!(chunk = chunks, wallets_scanned, adjustments_this_chunk = applied_here, adjustments_total = out.len(), cursor = %cursor, "ledger realign progress");

            if (addresses.len() as i64) < batch {
                break;
            }
        }

        if out.is_empty() {
            tracing::info!(wallets_scanned, chunks, reason = %reason, "ledger realign found no divergence; nothing written");
        } else {
            tracing::warn!(count = out.len(), wallets_scanned, chunks, reason = %reason, "ledger realigned to stored balances: adjustment rows written so replay reproduces the balance");
        }
        Ok(out)
    }

    async fn reconcile_earned_balance(&self) -> Result<Vec<LedgerBalanceDiff>, ApiError> {
        let rows = sqlx::query(
            "WITH ledger AS ( \
                 SELECT address, \
                        SUM(CASE \
                                WHEN kind IN ('grant','refund','purchase','claim') THEN amount \
                                WHEN kind IN ('spend','consume','expire') THEN -amount \
                                ELSE 0 END) AS s \
                 FROM credits.ledger WHERE bucket = 'earned' GROUP BY address \
             ) \
             SELECT COALESCE(l.address, u.address) AS address, \
                    COALESCE(l.s, 0)::text AS ledger_sum, \
                    COALESCE(u.earned_available, 0)::text AS available \
             FROM ledger l \
             FULL OUTER JOIN credits.wallets u ON u.address = l.address \
             WHERE COALESCE(l.s, 0) <> COALESCE(u.earned_available, 0) \
             ORDER BY 1 LIMIT $1",
        )
        .bind(RECONCILE_LIMIT)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .into_iter()
            .map(|r| LedgerBalanceDiff {
                address: r.get("address"),
                ledger_sum: r.get("ledger_sum"),
                available: r.get("available"),
            })
            .collect())
    }

    async fn reconcile_ledger_balance(&self) -> Result<Vec<LedgerBalanceDiff>, ApiError> {
        let rows = sqlx::query(
            "WITH ledger AS ( \
                 SELECT address, \
                        SUM(CASE \
                                WHEN kind IN ('grant','refund','purchase','claim') THEN amount \
                                WHEN kind IN ('spend','consume','expire') THEN -amount \
                                ELSE 0 END) AS s \
                 FROM credits.ledger GROUP BY address \
             ) \
             SELECT COALESCE(l.address, u.address) AS address, \
                    COALESCE(l.s, 0)::text AS ledger_sum, \
                    COALESCE(u.available, 0)::text AS available \
             FROM ledger l \
             FULL OUTER JOIN credits.wallets u ON u.address = l.address \
             WHERE COALESCE(l.s, 0) <> COALESCE(u.available, 0) \
             ORDER BY 1 LIMIT $1",
        )
        .bind(RECONCILE_LIMIT)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .into_iter()
            .map(|r| LedgerBalanceDiff {
                address: r.get("address"),
                ledger_sum: r.get("ledger_sum"),
                available: r.get("available"),
            })
            .collect())
    }

    async fn reconcile_purchase_grant(&self) -> Result<Vec<PurchaseGrantDiff>, ApiError> {
        let rows = sqlx::query(
            "WITH paid AS ( \
                 SELECT address, SUM(credits) AS c \
                 FROM credits.purchases \
                 WHERE status IN ('paid','refunded','disputed') GROUP BY address \
             ), granted AS ( \
                 SELECT address, SUM(amount) AS g \
                 FROM credits.ledger WHERE kind = 'purchase' GROUP BY address \
             ) \
             SELECT COALESCE(p.address, g.address) AS address, \
                    COALESCE(p.c, 0)::text AS paid_credits, \
                    COALESCE(g.g, 0)::text AS granted_credits \
             FROM paid p \
             FULL OUTER JOIN granted g ON g.address = p.address \
             WHERE COALESCE(p.c, 0) <> COALESCE(g.g, 0) \
             ORDER BY 1 LIMIT $1",
        )
        .bind(RECONCILE_LIMIT)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .into_iter()
            .map(|r| PurchaseGrantDiff {
                address: r.get("address"),
                paid_credits: r.get("paid_credits"),
                granted_credits: r.get("granted_credits"),
            })
            .collect())
    }

    async fn reconcile_checkout_fulfillment(
        &self,
    ) -> Result<Vec<CheckoutFulfillmentDiff>, ApiError> {
        let rows = sqlx::query(
            "SELECT c.id AS id, \
                    c.total_credits::text AS total_credits, \
                    COALESCE(o.s, 0)::text AS confirmed_sum \
             FROM credits.checkouts c \
             LEFT JOIN ( \
                 SELECT checkout_id, SUM(unit_price_credits) AS s \
                 FROM credits.fulfillment_outbox WHERE status = 'confirmed' GROUP BY checkout_id \
             ) o ON o.checkout_id = c.id \
             WHERE c.status = 'fulfilled' AND c.total_credits <> COALESCE(o.s, 0) \
             ORDER BY c.id LIMIT $1",
        )
        .bind(RECONCILE_LIMIT)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .into_iter()
            .map(|r| CheckoutFulfillmentDiff {
                checkout_id: r.get("id"),
                total_credits: r.get("total_credits"),
                confirmed_sum: r.get("confirmed_sum"),
            })
            .collect())
    }

    async fn reconcile_escrow_grants(&self) -> Result<Vec<EscrowGrantDiff>, ApiError> {
        let confirmed = sqlx::query(
            "SELECT o.id AS outbox_id, o.checkout_id, c.address AS address, o.urn, \
                    o.token_id, o.collection, o.external_ref, \
                    o.unit_price_credits::text AS unit_price_credits \
             FROM credits.fulfillment_outbox o \
             JOIN credits.checkouts c ON c.id = o.checkout_id \
             WHERE o.status = 'confirmed' AND o.external_ref IS NOT NULL \
             ORDER BY o.id LIMIT $1",
        )
        .bind(RECONCILE_LIMIT)
        .fetch_all(&self.pool)
        .await?;

        if confirmed.is_empty() {
            return Ok(Vec::new());
        }

        let refs: Vec<String> = {
            let mut seen = HashSet::new();
            confirmed
                .iter()
                .filter_map(|r| {
                    let er: String = r.get("external_ref");
                    seen.insert(er.clone()).then_some(er)
                })
                .collect()
        };

        let existing: HashSet<String> = sqlx::query(
            "SELECT DISTINCT escrow_ref FROM shop.usage_grants WHERE escrow_ref = ANY($1)",
        )
        .bind(&refs)
        .fetch_all(&self.pool)
        .await?
        .into_iter()
        .map(|r| r.get::<String, _>("escrow_ref"))
        .collect();

        Ok(confirmed
            .into_iter()
            .filter_map(|r| {
                let escrow_ref: String = r.get("external_ref");
                if existing.contains(&escrow_ref) {
                    return None;
                }
                Some(EscrowGrantDiff {
                    checkout_id: r.get("checkout_id"),
                    outbox_id: r.get("outbox_id"),
                    address: r.get("address"),
                    urn: r.get("urn"),
                    token_id: r.get("token_id"),
                    collection: Some(r.get("collection")),
                    escrow_ref,
                    unit_price_credits: r.get("unit_price_credits"),
                })
            })
            .collect())
    }

    async fn reconcile_escrow_holdings(&self) -> Result<EscrowHoldings, ApiError> {
        let rows = sqlx::query(
            "SELECT status, count(*)::bigint AS n FROM shop.usage_grants GROUP BY status",
        )
        .fetch_all(&self.pool)
        .await?;
        let mut holdings = EscrowHoldings {
            available: true,
            active: 0,
            revoked: 0,
            released: 0,
        };
        for r in rows {
            let status: String = r.get("status");
            let n: i64 = r.get("n");
            match status.as_str() {
                "active" => holdings.active = n,
                "revoked" => holdings.revoked = n,
                "released" => holdings.released = n,
                _ => {}
            }
        }
        Ok(holdings)
    }
}
