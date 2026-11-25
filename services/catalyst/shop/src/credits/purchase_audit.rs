// from: credits-server@d9c6a261:src/controllers/handlers/get-purchase-audit.ts, src/adapters/analytics-db.ts (getPurchaseAudit, countPurchaseAudit, PURCHASE_AUDIT_SORTS)
//! `GET /admin/purchase-audit`: what each purchase took from the buyer's
//! balance against what the chain spent, one row per transaction, in the old
//! server's shape.
//!
//! A row is a transaction that consumed credits this node signed
//! (`credits.usages` whose salt is a `credits.authorizations` id). The
//! contract drains a cart's credits in order, each up to its own cap, so only
//! the transaction total reconciles; each credit is listed under it.
//!
//! - `reservedUsdCents` is what this transaction's logs debited
//!   (`credits.usage_settlements`, credits × 10): the credit's charged price
//!   on its first use, the remainder charge on a later one. A log whose debit
//!   has not landed yet (the reconciler retries it) adds nothing.
//! - `consumedUsdCents` values each credit's wei at that credit's own oracle
//!   rate, summed exactly and rounded once. A credit with no recorded rate
//!   values its consumption at 0.
//! - `retainedUsdCents` is reserved − consumed, never below 0.
//! - `oracleRate` is USD per MANA at 8 decimals, the old server's scale.
//! - `creditId` and `intentId` are both the credit's salt: this node keeps
//!   one row per credit.

use serde::Serialize;
use serde_json::Value;
use sqlx::Row;

use super::errors::ApiError;
use super::money::saturating_cents as cents;
use super::store::Credits;

/// The columns the audit sorts by, as the dashboard names them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuditSort {
    SettledAt,
    Buyer,
    Items,
    Charged,
    Consumed,
    Retained,
}

/// The old server's keys, in its order (the 400 lists them).
pub const AUDIT_SORT_KEYS: &[&str] = &[
    "settledAt",
    "buyer",
    "items",
    "charged",
    "consumed",
    "retained",
];

/// One credit's consumption in wei × its rate, in cents (exact, unrounded).
const CONSUMED: &str =
    "c.wei * COALESCE(a.oracle_rate, 0) / power(10::numeric, 16 + COALESCE(a.oracle_decimals, 8))";

impl AuditSort {
    pub fn parse(raw: &str) -> Option<Self> {
        Some(match raw {
            "settledAt" => Self::SettledAt,
            "buyer" => Self::Buyer,
            "items" => Self::Items,
            "charged" => Self::Charged,
            "consumed" => Self::Consumed,
            "retained" => Self::Retained,
            _ => return None,
        })
    }

    pub fn key(self) -> &'static str {
        AUDIT_SORT_KEYS[self as usize]
    }

    fn expression(self) -> String {
        match self {
            Self::SettledAt => "MIN(c.at)".into(),
            Self::Buyer => "MIN(a.address)".into(),
            Self::Items => "SUM(c.lines)".into(),
            Self::Charged => "SUM(c.reserved)".into(),
            Self::Consumed => format!("SUM({CONSUMED})"),
            // As the response shows it: consumed rounded once, the difference never below 0.
            Self::Retained => {
                format!("GREATEST(round(SUM(c.reserved)) - round(SUM({CONSUMED})), 0)")
            }
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuditTransaction {
    pub tx_hash: String,
    pub buyer_address: String,
    pub block: i64,
    /// Milliseconds since the epoch: the first log's block time.
    pub settled_at: i64,
    pub reserved_usd_cents: i64,
    pub consumed_usd_cents: i64,
    pub retained_usd_cents: i64,
    pub intents: Value,
}

/// Each credit's logs in one transaction, then the transaction. Bound: $1 the
/// buyer (or NULL).
fn per_credit() -> &'static str {
    "WITH c AS ( \
         SELECT u.salt, u.tx_hash, MIN(u.block_number) AS block, \
                MIN(COALESCE(u.block_time, u.indexed_at)) AS at, SUM(u.value_wei) AS wei, \
                COALESCE(SUM(s.debited_credits), 0) * 10 AS reserved, \
                (SELECT count(*) FROM credits.authorization_lines l WHERE l.authorization_id = u.salt) AS lines \
         FROM credits.usages u \
         JOIN credits.authorizations a ON a.id = u.salt \
         LEFT JOIN credits.usage_settlements s \
           ON s.chain_id = u.chain_id AND s.block_number = u.block_number AND s.log_index = u.log_index \
         WHERE ($1::text IS NULL OR a.address = $1) \
         GROUP BY u.salt, u.tx_hash )"
}

impl Credits {
    /// One page of the audit and the number of transactions the filter matches,
    /// read in one snapshot.
    pub async fn report_purchase_audit(
        &self,
        buyer: Option<&str>,
        sort: AuditSort,
        descending: bool,
        limit: i64,
        offset: i64,
    ) -> Result<(Vec<AuditTransaction>, i64), ApiError> {
        // Every interpolated piece is a constant or comes from a closed enum.
        let direction = if descending { "DESC" } else { "ASC" };
        let page_sql = format!(
            "{cte} \
             SELECT c.tx_hash, MIN(a.address) AS buyer, MIN(c.block) AS block, \
                    (extract(epoch FROM MIN(c.at)) * 1000)::bigint AS settled_ms, \
                    round(SUM(c.reserved))::text AS reserved, round(SUM({CONSUMED}))::text AS consumed, \
                    json_agg(json_build_object( \
                        'creditId', a.id, \
                        'intentId', a.id, \
                        'tradeId', a.trade_id, \
                        'reservedUsdCents', round(c.reserved)::bigint, \
                        'manaCapWei', a.amount_wei::text, \
                        'settledManaWei', c.wei::text, \
                        'oracleRate', trunc(COALESCE(a.oracle_rate, 0) \
                            * power(10::numeric, 8 - COALESCE(a.oracle_decimals, 8)))::text, \
                        'createdAt', (extract(epoch FROM a.created_at) * 1000)::bigint \
                    ) ORDER BY a.created_at, a.id) AS intents \
             FROM c JOIN credits.authorizations a ON a.id = c.salt \
             GROUP BY c.tx_hash \
             ORDER BY {sort} {direction}, c.tx_hash {direction} \
             LIMIT $2 OFFSET $3",
            cte = per_credit(),
            sort = sort.expression(),
        );
        let count_sql = "SELECT count(DISTINCT u.tx_hash) FROM credits.usages u \
                         JOIN credits.authorizations a ON a.id = u.salt \
                         WHERE ($1::text IS NULL OR a.address = $1)";

        let mut tx = self.read_snapshot().await?;
        let rows = sqlx::query(sqlx::AssertSqlSafe(page_sql))
            .bind(buyer)
            .bind(limit)
            .bind(offset)
            .fetch_all(&mut *tx)
            .await?;
        let total: i64 = sqlx::query_scalar(count_sql)
            .bind(buyer)
            .fetch_one(&mut *tx)
            .await?;
        tx.commit().await?;

        let page = rows
            .into_iter()
            .map(|r| {
                let reserved = cents(r.get::<String, _>("reserved").as_str());
                let consumed = cents(r.get::<String, _>("consumed").as_str());
                AuditTransaction {
                    tx_hash: r.get("tx_hash"),
                    buyer_address: r.get("buyer"),
                    block: r.get("block"),
                    settled_at: r.get("settled_ms"),
                    reserved_usd_cents: reserved,
                    consumed_usd_cents: consumed,
                    retained_usd_cents: reserved.saturating_sub(consumed).max(0),
                    intents: r.get("intents"),
                }
            })
            .collect();
        Ok((page, total))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sort_keys_round_trip() {
        for key in AUDIT_SORT_KEYS {
            assert_eq!(AuditSort::parse(key).unwrap().key(), *key);
        }
        assert_eq!(AuditSort::parse("constructor"), None);
    }
}
