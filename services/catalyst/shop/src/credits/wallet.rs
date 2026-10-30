//! The balance moves: spend (earned first, reject-if-short under the wallet
//! lock, the outstanding authorizations counted against the budget), refund
//! (bounded by what its `tx_ref` debited, earned restored first) and revoke
//! (paid first, floored at zero, the shortfall reported). Every move writes
//! its `credits.ledger` rows in the same transaction as the balance update,
//! from the same NUMERIC values, filtered by a NUMERIC `> 0` predicate.
//!
//! Every earlier version of this decided in Rust with
//! `portion.parse::<f64>().unwrap_or(0.0) > 0.0` while the effect was applied
//! in NUMERIC. Those two disagree: PostgreSQL says `1e-400::numeric > 0` is
//! TRUE, `f64::from_str("1e-400")` is `0.0`. The balance moved, the row was
//! omitted, and reconcile diverged forever. Keeping the predicate inside the
//! INSERT makes that divergence unrepresentable.

use serde_json::json;
use sqlx::Row;

use super::errors::ApiError;
use super::money::CreditAmount;
use super::store::{AuditEntry, Credits, SYSTEM_ACTOR};
use crate::types::UsdCents;

#[derive(Debug, Clone)]
pub struct GrantOutcome {
    pub available: String,
    pub applied: String,
    pub replayed: bool,
}

macro_rules! ledger_split_insert_sql {
    () => {
        "INSERT INTO credits.ledger \
             (address, kind, amount, tx_ref, bucket, captcha_ok) \
         SELECT $1, $5, v.amount, $4, v.bucket, FALSE \
         FROM (VALUES ('earned'::text, $2::numeric), ('paid'::text, $3::numeric)) \
              AS v(bucket, amount) \
         WHERE v.amount > 0 \
         ORDER BY v.bucket"
    };
}
const LEDGER_SPLIT_INSERT_RETURNING_ID: &str = concat!(ledger_split_insert_sql!(), " RETURNING id");

/// The split plus its `ops.admin_audit` row in one statement, the ledger
/// rows naming the audit row: `$1` address, `$2` earned, `$3` paid, `$4`
/// tx_ref, `$5` kind, then `$6` action, `$7` actor, `$8` detail.
macro_rules! ledger_split_insert_with_audit_sql {
    () => {
        "WITH a AS ( \
             INSERT INTO ops.admin_audit (actor, domain, action, target, detail, result) \
             VALUES ($7, 'credits', $6, $1, $8, 'ok') RETURNING id \
         ) \
         INSERT INTO credits.ledger \
             (address, kind, amount, tx_ref, bucket, captcha_ok, admin_audit_id) \
         SELECT $1, $5, v.amount, $4, v.bucket, FALSE, a.id \
         FROM (VALUES ('earned'::text, $2::numeric), ('paid'::text, $3::numeric)) \
              AS v(bucket, amount), a \
         WHERE v.amount > 0"
    };
}
pub(crate) const LEDGER_SPLIT_INSERT_WITH_AUDIT: &str = ledger_split_insert_with_audit_sql!();
/// [`LEDGER_SPLIT_INSERT_WITH_AUDIT`] that also stamps the refund idempotency
/// snapshot: `$9` available, `$10` applied, `$11` key (NULL matches no row).
const LEDGER_SPLIT_INSERT_WITH_AUDIT_AND_REFUND_IDEM: &str = "WITH a AS ( INSERT INTO ops.admin_audit (actor, domain, action, target, detail, result) VALUES ($7, 'credits', $6, $1, $8, 'ok') RETURNING id ), l AS ( INSERT INTO credits.ledger (address, kind, amount, tx_ref, bucket, captcha_ok, admin_audit_id) SELECT $1, $5, v.amount, $4, v.bucket, FALSE, a.id FROM (VALUES ('earned'::text, $2::numeric), ('paid'::text, $3::numeric)) AS v(bucket, amount), a WHERE v.amount > 0 ) UPDATE credits.idempotency SET available = $9::numeric, applied = $10::numeric WHERE scope = 'refund' AND idempotency_key = $11";

impl Credits {
    pub async fn balance(&self, address: &str) -> Result<String, ApiError> {
        let row = sqlx::query(
            "SELECT available::text AS available FROM credits.wallets WHERE address = $1",
        )
        .bind(address)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row
            .map(|r| r.get::<String, _>("available"))
            .unwrap_or_else(|| "0".to_string()))
    }

    pub async fn spend(
        &self,
        address: &str,
        amount: &str,
        tx_ref: &str,
        idempotency_key: Option<&str>,
    ) -> Result<GrantOutcome, ApiError> {
        let mut tx = self.pool.begin().await?;
        let outcome = self
            .spend_in_tx(&mut tx, address, amount, tx_ref, idempotency_key)
            .await?;
        tx.commit().await?;
        Ok(outcome)
    }

    pub(crate) async fn spend_in_tx(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        address: &str,
        amount: &str,
        tx_ref: &str,
        idempotency_key: Option<&str>,
    ) -> Result<GrantOutcome, ApiError> {
        self.spend_in_tx_with_ledger(tx, address, amount, tx_ref, idempotency_key)
            .await
            .map(|(outcome, _)| outcome)
    }

    /// [`Self::spend_in_tx`] that also returns the id of the last ledger row it
    /// wrote (`None` for a zero or replayed spend), straight from the insert's RETURNING.
    pub(crate) async fn spend_in_tx_with_ledger(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        address: &str,
        amount: &str,
        tx_ref: &str,
        idempotency_key: Option<&str>,
    ) -> Result<(GrantOutcome, Option<i64>), ApiError> {
        let amount = CreditAmount::parse_non_negative(amount)?;
        if amount.is_zero() {
            let available: String = sqlx::query(
                "SELECT COALESCE((SELECT available::text FROM credits.wallets WHERE address = $1), '0') AS available",
            )
            .bind(address)
            .fetch_one(&mut **tx)
            .await?
            .get("available");
            return Ok((
                GrantOutcome {
                    available,
                    applied: amount.to_string(),
                    replayed: false,
                },
                None,
            ));
        }
        let amount = amount.as_str();
        if let Some(key) = idempotency_key {
            let claimed = sqlx::query(
                "INSERT INTO credits.idempotency \
                     (scope, idempotency_key, address, amount, available, tx_ref) \
                 VALUES ('spend', $1, $2, $3::numeric, 0, $4) \
                 ON CONFLICT (scope, idempotency_key) DO NOTHING \
                 RETURNING idempotency_key",
            )
            .bind(key)
            .bind(address)
            .bind(amount)
            .bind(tx_ref)
            .fetch_optional(&mut **tx)
            .await?
            .is_some();

            if !claimed {
                let prior = sqlx::query(
                    "SELECT available::text AS available, amount::text AS amount, \
                            (lower(address) = lower($2)) AS addr_match, \
                            (amount = $3::numeric) AS amount_match \
                     FROM credits.idempotency WHERE scope = 'spend' AND idempotency_key = $1",
                )
                .bind(key)
                .bind(address)
                .bind(amount)
                .fetch_one(&mut **tx)
                .await?;
                let addr_match: bool = prior.get("addr_match");
                let amount_match: bool = prior.get("amount_match");
                if !addr_match || !amount_match {
                    return Err(ApiError::conflict(
                        "idempotency key already used for a different spend (address/amount mismatch)",
                    ));
                }
                return Ok((
                    GrantOutcome {
                        available: prior.get("available"),
                        applied: prior.get("amount"),
                        replayed: true,
                    },
                    None,
                ));
            }
        }

        let current = sqlx::query(
            "SELECT available::text AS available, \
                    (available - COALESCE(( \
                         SELECT SUM(usd_cents) FROM credits.authorizations \
                         WHERE address = $1 AND status IN ('authorized', 'retired') \
                     ), 0)::numeric / $3::numeric >= $2::numeric) AS sufficient, \
                    LEAST(earned_available, $2::numeric)::text AS earned_spent, \
                    ($2::numeric - LEAST(earned_available, $2::numeric))::text AS paid_spent \
             FROM credits.wallets WHERE address = $1 FOR UPDATE",
        )
        .bind(address)
        .bind(amount)
        .bind(UsdCents::CENTS_PER_CREDIT)
        .fetch_optional(&mut **tx)
        .await?;

        let Some(current) = current else {
            return Err(ApiError::payment_required("insufficient credits balance"));
        };
        let sufficient: bool = current.get("sufficient");
        if !sufficient {
            return Err(ApiError::payment_required("insufficient credits balance"));
        }
        let earned_spent: String = current.get("earned_spent");
        let paid_spent: String = current.get("paid_spent");

        let row = sqlx::query(
            "UPDATE credits.wallets \
             SET available = available - $2::numeric, \
                 earned_available = earned_available - LEAST(earned_available, $2::numeric), \
                 updated_at = timezone('UTC', now()) \
             WHERE address = $1 \
             RETURNING available::text AS available",
        )
        .bind(address)
        .bind(amount)
        .fetch_one(&mut **tx)
        .await?;
        let available: String = row.get("available");

        let ledger_id = sqlx::query(LEDGER_SPLIT_INSERT_RETURNING_ID)
            .bind(address)
            .bind(&earned_spent)
            .bind(&paid_spent)
            .bind(tx_ref)
            .bind("spend")
            .fetch_all(&mut **tx)
            .await?
            .into_iter()
            .map(|r| r.get::<i64, _>("id"))
            .max();

        if let Some(key) = idempotency_key {
            sqlx::query(
                "UPDATE credits.idempotency \
                 SET available = $2::numeric, applied = $3::numeric \
                 WHERE scope = 'spend' AND idempotency_key = $1",
            )
            .bind(key)
            .bind(&available)
            .bind(amount)
            .execute(&mut **tx)
            .await?;
        }

        Ok((
            GrantOutcome {
                available,
                applied: amount.to_string(),
                replayed: false,
            },
            ledger_id,
        ))
    }

    pub async fn refund(
        &self,
        address: &str,
        amount: &str,
        tx_ref: &str,
        idempotency_key: Option<&str>,
    ) -> Result<GrantOutcome, ApiError> {
        let mut tx = self.pool.begin().await?;
        let outcome = self
            .refund_in_tx(&mut tx, address, amount, tx_ref, idempotency_key)
            .await?;
        tx.commit().await?;
        Ok(outcome)
    }

    /// Credits back at most what `tx_ref` debited (cumulative spends minus
    /// cumulative refunds under it, in any order), earned first up to the
    /// earned share it debited; the split is read AFTER the wallet lock so two
    /// racing refunds under distinct keys see each other's rows.
    pub(crate) async fn refund_in_tx(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        address: &str,
        amount: &str,
        tx_ref: &str,
        idempotency_key: Option<&str>,
    ) -> Result<GrantOutcome, ApiError> {
        let amount = CreditAmount::parse_positive(amount)?;
        let amount = amount.as_str();
        if let Some(key) = idempotency_key {
            let claimed = sqlx::query(
                "INSERT INTO credits.idempotency \
                     (scope, idempotency_key, address, amount, available, tx_ref) \
                 VALUES ('refund', $1, $2, $3::numeric, 0, $4) \
                 ON CONFLICT (scope, idempotency_key) DO NOTHING \
                 RETURNING idempotency_key",
            )
            .bind(key)
            .bind(address)
            .bind(amount)
            .bind(tx_ref)
            .fetch_optional(&mut **tx)
            .await?
            .is_some();

            if !claimed {
                let prior = sqlx::query(
                    "SELECT available::text AS available, \
                            COALESCE(applied, amount)::text AS applied, \
                            (lower(address) = lower($2)) AS addr_match, \
                            (amount = $3::numeric) AS amount_match \
                     FROM credits.idempotency WHERE scope = 'refund' AND idempotency_key = $1",
                )
                .bind(key)
                .bind(address)
                .bind(amount)
                .fetch_one(&mut **tx)
                .await?;
                let addr_match: bool = prior.get("addr_match");
                let amount_match: bool = prior.get("amount_match");
                if !addr_match || !amount_match {
                    return Err(ApiError::conflict(
                        "idempotency key already used for a different refund (address/amount mismatch)",
                    ));
                }
                return Ok(GrantOutcome {
                    available: prior.get("available"),
                    applied: prior.get("applied"),
                    replayed: true,
                });
            }
        }

        sqlx::query("SELECT 1 FROM credits.wallets WHERE address = $1 FOR UPDATE")
            .bind(address)
            .fetch_optional(&mut **tx)
            .await?;

        let split = sqlx::query(
            "WITH w AS ( \
                 SELECT COALESCE(SUM(amount) FILTER (WHERE kind = 'spend' AND bucket = 'earned'), 0) AS spent_earned, \
                        COALESCE(SUM(amount) FILTER (WHERE kind = 'refund' AND bucket = 'earned'), 0) AS refunded_earned, \
                        COALESCE(SUM(amount) FILTER (WHERE kind = 'spend'), 0) AS spent_total, \
                        COALESCE(SUM(amount) FILTER (WHERE kind = 'refund'), 0) AS refunded_total \
                 FROM credits.ledger \
                 WHERE address = $3 AND tx_ref = $1 AND kind IN ('spend', 'refund') \
             ), a AS ( \
                 SELECT LEAST($2::numeric, GREATEST(0, w.spent_total - w.refunded_total)) AS applied, \
                        GREATEST(0, w.spent_earned - w.refunded_earned) AS earned_window \
                 FROM w \
             ) \
             SELECT a.applied::text AS applied, \
                    LEAST(a.applied, a.earned_window)::text AS earned_back \
             FROM a",
        )
        .bind(tx_ref)
        .bind(amount)
        .bind(address)
        .fetch_one(&mut **tx)
        .await?;
        let applied: String = split.get("applied");
        let earned_back: String = split.get("earned_back");

        let row = sqlx::query(
            "INSERT INTO credits.wallets (address, available, earned_available, has_started_program, updated_at) \
             VALUES ($1, $2::numeric, $3::numeric, FALSE, timezone('UTC', now())) \
             ON CONFLICT (address) DO UPDATE \
                 SET available = credits.wallets.available + $2::numeric, \
                     earned_available = credits.wallets.earned_available + $3::numeric, \
                     updated_at = timezone('UTC', now()) \
             RETURNING available::text AS available, \
                       ($2::numeric - $3::numeric)::text AS paid_back",
        )
        .bind(address)
        .bind(&applied)
        .bind(&earned_back)
        .fetch_one(&mut **tx)
        .await?;
        let available: String = row.get("available");
        let paid_back: String = row.get("paid_back");

        let detail = json!({ "source": "refund", "txRef": tx_ref, "requested": amount, "applied": applied, "reason": "credits refund" });
        sqlx::query(LEDGER_SPLIT_INSERT_WITH_AUDIT_AND_REFUND_IDEM)
            .bind(address)
            .bind(&earned_back)
            .bind(&paid_back)
            .bind(tx_ref)
            .bind("refund")
            .bind("refund")
            .bind(SYSTEM_ACTOR)
            .bind(&detail)
            .bind(&available)
            .bind(&applied)
            .bind(idempotency_key)
            .execute(&mut **tx)
            .await?;

        Ok(GrantOutcome {
            available,
            applied,
            replayed: false,
        })
    }

    /// Remove credits from a wallet as the compensation for a REVERSED fiat
    /// payment (Stripe refund / dispute / chargeback). The opposite of
    /// [`Self::refund_in_tx`]: the balance floors at zero (the CHECKs make a
    /// negative one unrepresentable), what could not be clawed back is
    /// reported as `shortfall`, and the debit is split PAID-FIRST (a purchase
    /// grants paid credits, so a charged-back pack does not eat credits earned
    /// by playing) as `consume` rows so the signed ledger sum keeps matching.
    pub(crate) async fn revoke_in_tx(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        address: &str,
        amount: &str,
        tx_ref: &str,
        reason: &str,
        detail: &serde_json::Value,
    ) -> Result<RevokeOutcome, ApiError> {
        let amount = CreditAmount::parse_positive(amount)?;
        let amount = amount.as_str();

        let before = sqlx::query(
            "SELECT available::text AS available, earned_available::text AS earned \
             FROM credits.wallets WHERE address = $1 FOR UPDATE",
        )
        .bind(address)
        .fetch_optional(&mut **tx)
        .await?;

        let Some(before) = before else {
            let mut detail = detail.clone();
            if let serde_json::Value::Object(map) = &mut detail {
                map.insert("txRef".into(), json!(tx_ref));
                map.insert("chargedBack".into(), json!(amount));
                map.insert("removed".into(), json!("0"));
                map.insert("shortfall".into(), json!(amount));
                map.insert("walletRow".into(), json!("missing"));
                map.insert("reason".into(), json!(reason));
                map.insert("amount".into(), json!("0"));
            }
            Self::audit(
                &mut **tx,
                AuditEntry {
                    action: "revoke",
                    actor: SYSTEM_ACTOR,
                    target: Some(address),
                    detail: &detail,
                },
            )
            .await?;
            tracing::error!(
                address = %address,
                tx_ref = %tx_ref,
                charged_back = %amount,
                "credits revoke found NO wallet row: the entire charge-back is an unrecovered loss"
            );
            return Ok(RevokeOutcome {
                available: "0".to_string(),
                removed: "0".to_string(),
                shortfall: amount.to_string(),
                has_shortfall: true,
            });
        };
        let available_before: String = before.get("available");
        let earned_before: String = before.get("earned");

        let row = sqlx::query(
            "UPDATE credits.wallets \
             SET available = GREATEST(available - $2::numeric, 0), \
                 earned_available = LEAST(earned_available, GREATEST(available - $2::numeric, 0)), \
                 updated_at = timezone('UTC', now()) \
             WHERE address = $1 \
             RETURNING available::text AS available, \
                       ($3::numeric - available)::text AS removed, \
                       ($4::numeric - earned_available)::text AS earned_removed, \
                       (($3::numeric - available) - ($4::numeric - earned_available))::text AS paid_removed, \
                       GREATEST($2::numeric - ($3::numeric - available), 0)::text AS shortfall, \
                       ($2::numeric > $3::numeric - available) AS has_shortfall",
        )
        .bind(address)
        .bind(amount)
        .bind(&available_before)
        .bind(&earned_before)
        .fetch_one(&mut **tx)
        .await?;
        let available: String = row.get("available");
        let removed: String = row.get("removed");
        let earned_removed: String = row.get("earned_removed");
        let paid_removed: String = row.get("paid_removed");
        let shortfall: String = row.get("shortfall");
        let has_shortfall: bool = row.get("has_shortfall");

        let mut detail = detail.clone();
        if let serde_json::Value::Object(map) = &mut detail {
            map.insert("txRef".into(), json!(tx_ref));
            map.insert("chargedBack".into(), json!(amount));
            map.insert("removed".into(), json!(removed));
            map.insert("shortfall".into(), json!(shortfall));
            map.insert("reason".into(), json!(reason));
            map.insert("amount".into(), json!(removed));
        }
        sqlx::query(LEDGER_SPLIT_INSERT_WITH_AUDIT)
            .bind(address)
            .bind(&earned_removed)
            .bind(&paid_removed)
            .bind(tx_ref)
            .bind("consume")
            .bind("revoke")
            .bind(SYSTEM_ACTOR)
            .bind(&detail)
            .execute(&mut **tx)
            .await?;

        Ok(RevokeOutcome {
            available,
            removed,
            shortfall,
            has_shortfall,
        })
    }
}

#[derive(Debug, Clone)]
pub struct RevokeOutcome {
    pub available: String,
    pub removed: String,
    /// Credits that could NOT be removed because the buyer had already spent
    /// them. Non-zero means an unrecovered loss; callers must log it loudly.
    pub shortfall: String,
    /// Whether `shortfall` is strictly positive, decided by PostgreSQL in
    /// NUMERIC (the text form may render as "0.00").
    pub has_shortfall: bool,
}
