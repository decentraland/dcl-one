// from: credits-server@d9c6a261:src/controllers/handlers/get-ledger-stats.ts, src/adapters/db/db.ts (getUsdLedgerStats), src/adapters/stripe-orders-db.ts (getOrderCountsByStatus), src/adapters/analytics-db.ts
//! The ops dashboard's ledger health aggregates (`GET /admin/ledger-stats`),
//! all wallets, in the old server's shape and read from this node's tables in
//! one snapshot. Cents are integers; 1 credit = 10 cents.
//!
//! - `usd`: `credits.ledger`. Issued is every grant, pack purchase and goal
//!   claim; reversed is every removal that bought nothing (revocations,
//!   chargebacks, expiries, the reconciler's adjustments); settled is what
//!   purchases spent net of the refunds that returned it; pending is the
//!   credits still pledged (`credits.authorizations` authorized or retired).
//!   So the outstanding liability is the ledger's balance minus the pledges.
//! - `orders.byStatus`: card pack purchases (`credits.purchases`), with this
//!   node's states named as the old funnel reads them: `pending` is
//!   `processing`, `paid` is `credited`, `refunded` and `disputed` are
//!   `reversed`; `failed` and `abandoned` keep their names.
//! - `analytics.stripe`: the same purchases. Gross is what paid orders
//!   charged, net what Stripe has not refunded, clawed back the credits
//!   removed for refunds and disputes. A partly refunded order stays
//!   credited.
//! - `analytics.usdTrends`: the last 90 UTC days of issued and spent credits
//!   (the ledger) and paid card orders.
//! - `analytics.usdTopSpenders`: net spend per wallet (the ledger), with the
//!   number of debits; `usdTopCollections`: the lines of the credits the
//!   chain consumed (`credits.authorization_lines`), all primary sales.
//! - `analytics.flaggedWallets`: `credits.wallet_flags`.
//! - `analytics.legacy`: the seasonal MANA credits (`credits.goal_grants`)
//!   and what the chain consumed of them (`credits.usages` by salt).
//!
//! The old `saleProceeds` block is not served: this node issues no credits
//! from sale proceeds. Every source is a local table, so `analytics` is
//! never null and a failing query fails the request.

use serde::Serialize;
use sqlx::Row;

use super::errors::ApiError;
use super::store::Credits;

const TRENDS_WINDOW_DAYS: i32 = 90;
const EXPIRING_WINDOWS_DAYS: [i32; 3] = [7, 30, 90];
const TOP_LIMIT: i64 = 10;
const RECENT_FLAGGED_LIMIT: i64 = 5;

/// Credit kinds that add to a balance without a purchase spending them.
const ISSUED: &str = "('grant', 'purchase', 'claim')";
/// Removals that bought nothing.
const REVERSED: &str = "('consume', 'expire')";

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LedgerStats {
    pub usd: UsdBlock,
    pub orders: Orders,
    pub analytics: Analytics,
    /// Milliseconds since the epoch.
    pub generated_at: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UsdBlock {
    pub total_issued_cents: i64,
    pub total_reversed_cents: i64,
    pub net_credited_cents: i64,
    pub total_settled_cents: i64,
    pub total_pending_cents: i64,
    pub outstanding_liability_cents: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Orders {
    pub by_status: Vec<OrderStatusCount>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OrderStatusCount {
    pub status: String,
    pub count: i64,
    pub gross_cents: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Analytics {
    pub stripe: Stripe,
    pub usd_trends: UsdTrends,
    pub usd_top_spenders: Vec<UsdTopSpender>,
    pub usd_top_collections: Vec<UsdTopCollection>,
    pub flagged_wallets: FlaggedWallets,
    pub legacy: Legacy,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Stripe {
    pub gross_cents: i64,
    pub net_cents: i64,
    pub refunded_gross_cents: i64,
    pub clawed_back_cents: i64,
    pub credited_count: i64,
    pub reversed_count: i64,
    /// reversed / (credited + reversed), 0..1.
    pub refund_rate: f64,
    pub by_pack: Vec<PackRevenue>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PackRevenue {
    pub pack_id: String,
    pub paid_orders: i64,
    pub gross_cents: i64,
    pub net_cents: i64,
    pub credits: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UsdTrends {
    pub window_days: i32,
    pub daily: Vec<UsdDay>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UsdDay {
    pub date: String,
    pub issued_usd_cents: i64,
    pub spent_usd_cents: i64,
    pub stripe_gross_cents: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UsdTopSpender {
    pub address: String,
    pub spent_usd_cents: i64,
    pub purchases: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UsdTopCollection {
    pub collection_address: String,
    pub purchases: i64,
    pub usd_cents: i64,
    pub primary_sales: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FlaggedWallets {
    pub total: i64,
    pub recent: Vec<FlaggedWallet>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FlaggedWallet {
    pub address: String,
    /// ISO 8601, UTC.
    pub flagged_at: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Legacy {
    pub redemption: Redemption,
    pub top_spenders: Vec<ManaTopSpender>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Redemption {
    pub issued_mana_wei: String,
    pub consumed_mana_wei: String,
    /// consumed / issued, 0..1, at basis-point precision.
    pub redemption_rate: f64,
    pub breakage_mana_wei: String,
    pub expiring: Vec<ExpiringBucket>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExpiringBucket {
    pub label: String,
    pub within_days: i32,
    pub count: i64,
    pub mana_wei: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ManaTopSpender {
    pub address: String,
    pub consumed_mana_wei: String,
    pub consumptions: i64,
}

/// A NUMERIC sum read as text, as integer cents. Sums are rounded in SQL; a
/// value past a bigint saturates instead of failing the whole report.
pub(crate) fn cents(text: &str) -> i64 {
    match text.parse::<i64>() {
        Ok(v) => v,
        Err(_) if text.starts_with('-') => i64::MIN,
        Err(_) => i64::MAX,
    }
}

/// The old buckets' labels: the first is `≤ 7d`, each later one starts
/// where the previous ended (`7–30d`).
fn bucket_label(lower: i32, upper: i32) -> String {
    if lower == 0 {
        format!("≤ {upper}d")
    } else {
        format!("{lower}–{upper}d")
    }
}

impl Credits {
    pub async fn report_ledger_stats(&self) -> Result<LedgerStats, ApiError> {
        let mut tx = self.read_snapshot().await?;

        let usd = sqlx::query(sqlx::AssertSqlSafe(format!(
            "SELECT round(COALESCE(SUM(amount) FILTER (WHERE kind IN {ISSUED}), 0) * 10)::text AS issued, \
                    round(COALESCE(SUM(amount) FILTER (WHERE kind IN {REVERSED}), 0) * 10)::text AS reversed, \
                    round((COALESCE(SUM(amount) FILTER (WHERE kind = 'spend'), 0) \
                         - COALESCE(SUM(amount) FILTER (WHERE kind = 'refund'), 0)) * 10)::text AS settled, \
                    (SELECT COALESCE(SUM(usd_cents), 0)::text FROM credits.authorizations \
                      WHERE status IN ('authorized', 'retired')) AS pending \
             FROM credits.ledger"
        )))
        .fetch_one(&mut *tx)
        .await?;
        let issued = cents(usd.get::<String, _>("issued").as_str());
        let reversed = cents(usd.get::<String, _>("reversed").as_str());
        let settled = cents(usd.get::<String, _>("settled").as_str());
        let pending = cents(usd.get::<String, _>("pending").as_str());
        let net = issued.saturating_sub(reversed);
        let usd = UsdBlock {
            total_issued_cents: issued,
            total_reversed_cents: reversed,
            net_credited_cents: net,
            total_settled_cents: settled,
            total_pending_cents: pending,
            outstanding_liability_cents: net.saturating_sub(settled).saturating_sub(pending),
        };

        let by_status = sqlx::query(
            "SELECT CASE status WHEN 'pending' THEN 'processing' WHEN 'paid' THEN 'credited' \
                                WHEN 'refunded' THEN 'reversed' WHEN 'disputed' THEN 'reversed' \
                                ELSE status END AS status, \
                    count(*) AS count, COALESCE(SUM(amount_cents), 0)::text AS gross \
             FROM credits.purchases WHERE method = 'card' GROUP BY 1 ORDER BY 1",
        )
        .fetch_all(&mut *tx)
        .await?
        .into_iter()
        .map(|r| OrderStatusCount {
            status: r.get("status"),
            count: r.get("count"),
            gross_cents: cents(r.get::<String, _>("gross").as_str()),
        })
        .collect();

        const PAID: &str = "status IN ('paid', 'refunded', 'disputed')";
        const REVERSED_ORDER: &str = "status IN ('refunded', 'disputed')";
        let s = sqlx::query(sqlx::AssertSqlSafe(format!(
            "SELECT COALESCE(SUM(amount_cents) FILTER (WHERE {PAID}), 0)::text AS gross, \
                    COALESCE(SUM(refunded_cents) FILTER (WHERE {PAID}), 0)::text AS refunded, \
                    round(COALESCE(SUM(revoked_credits), 0) * 10)::text AS clawed_back, \
                    count(*) FILTER (WHERE status = 'paid') AS credited, \
                    count(*) FILTER (WHERE {REVERSED_ORDER}) AS reversed \
             FROM credits.purchases WHERE method = 'card'"
        )))
        .fetch_one(&mut *tx)
        .await?;
        let by_pack = sqlx::query(sqlx::AssertSqlSafe(format!(
            "SELECT sku, count(*) FILTER (WHERE {PAID}) AS paid_orders, \
                    COALESCE(SUM(amount_cents) FILTER (WHERE {PAID}), 0)::text AS gross, \
                    COALESCE(SUM(amount_cents - refunded_cents) FILTER (WHERE {PAID}), 0)::text AS net, \
                    trunc(COALESCE(SUM(credits) FILTER (WHERE status = 'paid'), 0))::text AS credits \
             FROM credits.purchases WHERE method = 'card' \
             GROUP BY sku HAVING count(*) FILTER (WHERE {PAID}) > 0 \
             ORDER BY COALESCE(SUM(amount_cents) FILTER (WHERE {PAID}), 0) DESC, sku"
        )))
        .fetch_all(&mut *tx)
        .await?
        .into_iter()
        .map(|r| PackRevenue {
            pack_id: r.get("sku"),
            paid_orders: r.get("paid_orders"),
            gross_cents: cents(r.get::<String, _>("gross").as_str()),
            net_cents: cents(r.get::<String, _>("net").as_str()),
            credits: cents(r.get::<String, _>("credits").as_str()),
        })
        .collect();
        let gross = cents(s.get::<String, _>("gross").as_str());
        let refunded = cents(s.get::<String, _>("refunded").as_str());
        let credited: i64 = s.get("credited");
        let reversed_count: i64 = s.get("reversed");
        let stripe = Stripe {
            gross_cents: gross,
            net_cents: gross.saturating_sub(refunded),
            refunded_gross_cents: refunded,
            clawed_back_cents: cents(s.get::<String, _>("clawed_back").as_str()),
            credited_count: credited,
            reversed_count,
            refund_rate: if credited + reversed_count > 0 {
                reversed_count as f64 / (credited + reversed_count) as f64
            } else {
                0.0
            },
            by_pack,
        };

        let daily = sqlx::query(sqlx::AssertSqlSafe(format!(
            "WITH since AS (SELECT date_trunc('day', timezone('UTC', now())) - make_interval(days => $1 - 1) AS at), \
             d AS ( \
                 SELECT created_at::date AS day, \
                        COALESCE(SUM(amount) FILTER (WHERE kind IN {ISSUED}), 0) * 10 AS issued, \
                        (COALESCE(SUM(amount) FILTER (WHERE kind = 'spend'), 0) \
                         - COALESCE(SUM(amount) FILTER (WHERE kind = 'refund'), 0)) * 10 AS spent, \
                        0::numeric AS stripe \
                 FROM credits.ledger, since WHERE created_at >= since.at GROUP BY 1 \
                 UNION ALL \
                 SELECT created_at::date, 0, 0, SUM(amount_cents) \
                 FROM credits.purchases, since \
                 WHERE method = 'card' AND {PAID} AND created_at >= since.at GROUP BY 1 \
             ) \
             SELECT to_char(day, 'YYYY-MM-DD') AS date, round(SUM(issued))::text AS issued, \
                    round(SUM(spent))::text AS spent, SUM(stripe)::text AS stripe \
             FROM d GROUP BY day ORDER BY day"
        )))
        .bind(TRENDS_WINDOW_DAYS)
        .fetch_all(&mut *tx)
        .await?
        .into_iter()
        .map(|r| UsdDay {
            date: r.get("date"),
            issued_usd_cents: cents(r.get::<String, _>("issued").as_str()),
            spent_usd_cents: cents(r.get::<String, _>("spent").as_str()),
            stripe_gross_cents: cents(r.get::<String, _>("stripe").as_str()),
        })
        .collect();

        // Net spend per wallet; a debit split across the earned and paid buckets is one
        // purchase, so purchases count distinct causes.
        let usd_top_spenders = sqlx::query(
            "SELECT address, spent::text AS spent, purchases FROM ( \
                 SELECT address, \
                        round((COALESCE(SUM(amount) FILTER (WHERE kind = 'spend'), 0) \
                             - COALESCE(SUM(amount) FILTER (WHERE kind = 'refund'), 0)) * 10) AS spent, \
                        count(DISTINCT COALESCE(tx_ref, id::text)) FILTER (WHERE kind = 'spend') AS purchases \
                 FROM credits.ledger WHERE kind IN ('spend', 'refund') GROUP BY address \
             ) s WHERE spent > 0 ORDER BY spent DESC, address LIMIT $1",
        )
        .bind(TOP_LIMIT)
        .fetch_all(&mut *tx)
        .await?
        .into_iter()
        .map(|r| UsdTopSpender {
            address: r.get("address"),
            spent_usd_cents: cents(r.get::<String, _>("spent").as_str()),
            purchases: r.get("purchases"),
        })
        .collect();

        let usd_top_collections = sqlx::query(
            "SELECT l.asset_contract AS collection, count(*) AS purchases, \
                    COALESCE(SUM(l.charged_cents), 0)::text AS cents, \
                    count(*) FILTER (WHERE l.sale_kind LIKE 'primary%') AS primary_sales \
             FROM credits.authorization_lines l \
             JOIN credits.authorizations a ON a.id = l.authorization_id \
             WHERE a.status = 'consumed' \
             GROUP BY l.asset_contract \
             ORDER BY SUM(l.charged_cents) DESC, count(*) DESC, l.asset_contract LIMIT $1",
        )
        .bind(TOP_LIMIT)
        .fetch_all(&mut *tx)
        .await?
        .into_iter()
        .map(|r| UsdTopCollection {
            collection_address: r.get("collection"),
            purchases: r.get("purchases"),
            usd_cents: cents(r.get::<String, _>("cents").as_str()),
            primary_sales: r.get("primary_sales"),
        })
        .collect();

        let total: i64 = sqlx::query_scalar("SELECT count(*) FROM credits.wallet_flags")
            .fetch_one(&mut *tx)
            .await?;
        let recent = sqlx::query(
            "SELECT address, to_char(flagged_at, 'YYYY-MM-DD\"T\"HH24:MI:SS.MS\"Z\"') AS flagged_at \
             FROM credits.wallet_flags ORDER BY flagged_at DESC, address LIMIT $1",
        )
        .bind(RECENT_FLAGGED_LIMIT)
        .fetch_all(&mut *tx)
        .await?
        .into_iter()
        .map(|r| FlaggedWallet {
            address: r.get("address"),
            flagged_at: r.get("flagged_at"),
        })
        .collect();

        let legacy = self.legacy_mana(&mut tx).await?;
        tx.commit().await?;

        Ok(LedgerStats {
            usd,
            orders: Orders { by_status },
            analytics: Analytics {
                stripe,
                usd_trends: UsdTrends {
                    window_days: TRENDS_WINDOW_DAYS,
                    daily,
                },
                usd_top_spenders,
                usd_top_collections,
                flagged_wallets: FlaggedWallets { total, recent },
                legacy,
            },
            generated_at: chrono::Utc::now().timestamp_millis(),
        })
    }

    /// The seasonal MANA credits: issued, consumed on chain, left on expired
    /// credits (breakage), left on credits expiring in each window, and the
    /// wallets that spent the most of them.
    async fn legacy_mana(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    ) -> Result<Legacy, ApiError> {
        const PER_CREDIT: &str = "WITH c AS ( \
                 SELECT g.amount_wei, g.expires_at, \
                        COALESCE((SELECT SUM(u.value_wei) FROM credits.usages u WHERE u.salt = g.salt), 0) AS used \
                 FROM credits.goal_grants g )";
        let r = sqlx::query(sqlx::AssertSqlSafe(format!(
            "{PER_CREDIT} \
             SELECT COALESCE(SUM(amount_wei), 0)::text AS issued, \
                    COALESCE(SUM(used), 0)::text AS consumed, \
                    CASE WHEN COALESCE(SUM(amount_wei), 0) > 0 \
                         THEN LEAST(1, trunc(COALESCE(SUM(used), 0) * 10000 / SUM(amount_wei)) / 10000) \
                         ELSE 0 END::float8 AS rate, \
                    COALESCE(SUM(GREATEST(amount_wei - used, 0)) \
                        FILTER (WHERE expires_at < timezone('UTC', now())), 0)::text AS breakage \
             FROM c"
        )))
        .fetch_one(&mut **tx)
        .await?;

        let mut expiring = Vec::with_capacity(EXPIRING_WINDOWS_DAYS.len());
        let mut lower = 0;
        for upper in EXPIRING_WINDOWS_DAYS {
            let b = sqlx::query(sqlx::AssertSqlSafe(format!(
                "{PER_CREDIT} \
                 SELECT count(*) AS count, COALESCE(SUM(GREATEST(amount_wei - used, 0)), 0)::text AS wei \
                 FROM c WHERE expires_at > timezone('UTC', now()) + make_interval(days => $1) \
                          AND expires_at <= timezone('UTC', now()) + make_interval(days => $2)"
            )))
            .bind(lower)
            .bind(upper)
            .fetch_one(&mut **tx)
            .await?;
            expiring.push(ExpiringBucket {
                label: bucket_label(lower, upper),
                within_days: upper,
                count: b.get("count"),
                mana_wei: b.get("wei"),
            });
            lower = upper;
        }

        let top_spenders = sqlx::query(
            "SELECT u.sender AS address, SUM(u.value_wei)::text AS wei, count(*) AS consumptions \
             FROM credits.usages u JOIN credits.goal_grants g ON g.salt = u.salt \
             GROUP BY u.sender ORDER BY SUM(u.value_wei) DESC, u.sender LIMIT $1",
        )
        .bind(TOP_LIMIT)
        .fetch_all(&mut **tx)
        .await?
        .into_iter()
        .map(|r| ManaTopSpender {
            address: r.get("address"),
            consumed_mana_wei: r.get("wei"),
            consumptions: r.get("consumptions"),
        })
        .collect();

        Ok(Legacy {
            redemption: Redemption {
                issued_mana_wei: r.get("issued"),
                consumed_mana_wei: r.get("consumed"),
                redemption_rate: r.get("rate"),
                breakage_mana_wei: r.get("breakage"),
                expiring,
            },
            top_spenders,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cents_saturate_instead_of_failing() {
        assert_eq!(cents("1234"), 1234);
        assert_eq!(cents("-5"), -5);
        assert_eq!(cents("99999999999999999999999"), i64::MAX);
        assert_eq!(cents("-99999999999999999999999"), i64::MIN);
    }

    #[test]
    fn buckets_are_labelled_like_the_old_server() {
        assert_eq!(bucket_label(0, 7), "≤ 7d");
        assert_eq!(bucket_label(7, 30), "7–30d");
    }
}
