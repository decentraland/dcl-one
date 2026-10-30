// from: credits-server@465e628bc:src/controllers/handlers/{get-user-purchases,get-user-credit-orders,get-user-credits}.ts (the shapes), shop@94584a5c:app/src/lib/credits.ts (decodePurchaseRecord, decodeCreditOrder, decodeHeld: what the UI reads)
//! The buyer's histories, read from the credit store: the purchases paid
//! with credits (one row per line of a group, in upstream's intent
//! vocabulary `PENDING`/`SETTLED`/`EXPIRED`, with the settlement hash from
//! the indexed consumption and the client-reported submission hash), the
//! pack orders (`stripe_orders` upstream: `credits.purchases` here), and the
//! held block of the balance (credits pledged to purchases not yet proven,
//! with the earliest moment they can come back).

use chrono::NaiveDateTime;
use serde::Serialize;
use sqlx::Row;

use super::errors::ApiError;
use super::store::Credits;
use crate::types::UsdCents;

/// Upstream's page bounds: `limit` default 200 (purchases cap 200, credit
/// orders cap 500), `offset` default 0.
pub fn clamp_page(limit: Option<&str>, offset: Option<&str>, max_limit: i64) -> (i64, i64) {
    let parse = |raw: Option<&str>| {
        raw.and_then(|s| s.trim().parse::<f64>().ok())
            .filter(|n| n.is_finite())
            .map(|n| n.trunc() as i64)
    };
    let limit = parse(limit)
        .map_or(200, |n| n.clamp(1, max_limit))
        .min(max_limit);
    let offset = parse(offset).map_or(0, |n| n.max(0));
    (limit, offset)
}

/// The wire status of a group (upstream `IntentStatus`).
pub fn intent_status(status: &str) -> &'static str {
    match status {
        "authorized" | "retired" => "PENDING",
        "consumed" => "SETTLED",
        _ => "EXPIRED",
    }
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct PurchaseOut {
    pub id: String,
    #[serde(rename = "tradeId")]
    pub trade_id: Option<String>,
    #[serde(rename = "contractAddress")]
    pub contract_address: Option<String>,
    #[serde(rename = "itemId")]
    pub item_id: Option<String>,
    #[serde(rename = "registeredName")]
    pub registered_name: Option<String>,
    #[serde(rename = "usdCents")]
    pub usd_cents: i64,
    pub credits: i64,
    pub status: &'static str,
    /// Milliseconds.
    #[serde(rename = "createdAt")]
    pub created_at: i64,
    #[serde(rename = "manaSettledWei")]
    pub mana_settled_wei: Option<String>,
    #[serde(rename = "txHash")]
    pub tx_hash: Option<String>,
    #[serde(rename = "submittedTxHash")]
    pub submitted_tx_hash: Option<String>,
    /// The group's salt: every line of one credit shares it.
    pub salt: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct PurchasesOut {
    pub total: i64,
    pub purchases: Vec<PurchaseOut>,
}

fn ms(t: NaiveDateTime) -> i64 {
    t.and_utc().timestamp_millis()
}

impl Credits {
    /// The lines of the wallet's groups in `statuses`, newest group first,
    /// paged over lines; `total` counts every matching line.
    pub async fn purchases_history(
        &self,
        address: &str,
        all: bool,
        limit: i64,
        offset: i64,
    ) -> Result<PurchasesOut, ApiError> {
        let statuses: &[&str] = if all {
            &["authorized", "retired", "consumed", "released", "expired"]
        } else {
            &["consumed"]
        };
        let statuses: Vec<String> = statuses.iter().map(|s| s.to_string()).collect();
        let total: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM credits.authorization_lines l JOIN credits.authorizations a ON a.id = l.authorization_id \
             WHERE a.address = $1 AND a.status = ANY($2)",
        )
        .bind(address)
        .bind(&statuses)
        .fetch_one(&self.pool)
        .await?;
        let rows = sqlx::query(
            "SELECT a.id, a.status, a.created_at, a.submitted_tx_hash, l.position, l.charged_cents, l.trade_id, l.contract_address, l.item_id, l.registered_name, \
                    (SELECT SUM(s.value_wei)::text FROM credits.usage_settlements s WHERE s.authorization_id = a.id) AS settled_wei, \
                    (SELECT u.tx_hash FROM credits.usage_settlements s JOIN credits.usages u \
                        ON u.chain_id = s.chain_id AND u.block_number = s.block_number AND u.log_index = s.log_index \
                     WHERE s.authorization_id = a.id ORDER BY s.block_number, s.log_index LIMIT 1) AS tx_hash \
             FROM credits.authorization_lines l JOIN credits.authorizations a ON a.id = l.authorization_id \
             WHERE a.address = $1 AND a.status = ANY($2) \
             ORDER BY a.created_at DESC, a.id, l.position \
             LIMIT $3 OFFSET $4",
        )
        .bind(address)
        .bind(&statuses)
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await?;
        let purchases = rows
            .into_iter()
            .map(|r| {
                let salt: String = r.get("id");
                let position: i32 = r.get("position");
                let charged: i64 = r.get("charged_cents");
                let status: String = r.get("status");
                PurchaseOut {
                    id: format!("{salt}:{position}"),
                    trade_id: r.get("trade_id"),
                    contract_address: r.get("contract_address"),
                    item_id: r.get("item_id"),
                    registered_name: r.get("registered_name"),
                    usd_cents: charged,
                    credits: UsdCents::new(charged).credits_ceil(),
                    status: intent_status(&status),
                    created_at: ms(r.get("created_at")),
                    mana_settled_wei: r.get("settled_wei"),
                    tx_hash: r.get("tx_hash"),
                    submitted_tx_hash: r.get("submitted_tx_hash"),
                    salt,
                }
            })
            .collect();
        Ok(PurchasesOut { total, purchases })
    }

    /// The wallet's pack orders newest first: one row per `credits.purchases`
    /// row of the card flows (a Checkout Session order or a PaymentIntent).
    pub async fn credit_orders_history(
        &self,
        address: &str,
        limit: i64,
        offset: i64,
    ) -> Result<CreditOrdersOut, ApiError> {
        let total: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM credits.purchases WHERE address = $1 AND method = 'card'",
        )
        .bind(address)
        .fetch_one(&self.pool)
        .await?;
        let rows = sqlx::query(
            "SELECT id, order_id, stripe_payment_intent, sku, floor(credits)::bigint AS credits, amount_cents, refunded_cents, status, created_at \
             FROM credits.purchases WHERE address = $1 AND method = 'card' ORDER BY id DESC LIMIT $2 OFFSET $3",
        )
        .bind(address)
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await?;
        let orders = rows
            .into_iter()
            .map(|r| {
                let order_id: Option<String> = r.get("order_id");
                let pi: Option<String> = r.get("stripe_payment_intent");
                let row_id: i64 = r.get("id");
                let status: String = r.get("status");
                let amount_cents: i64 = r.get("amount_cents");
                let refunded: i64 = r.get("refunded_cents");
                CreditOrderOut {
                    id: order_id
                        .or(pi.map(|p| format!("pi:{p}")))
                        .unwrap_or_else(|| format!("purchase:{row_id}")),
                    provider: "stripe",
                    status: order_status(&status, amount_cents, refunded),
                    credits: r.get("credits"),
                    usd_cents: amount_cents,
                    created_at: ms(r.get("created_at")),
                    pack_id: r.get("sku"),
                }
            })
            .collect();
        Ok(CreditOrdersOut {
            total,
            orders,
            earnings: Earnings::none(),
        })
    }

    /// The credits held by the wallet's live pledges, for the balance's
    /// `usd.held` block: `None` when nothing is held.
    pub async fn held(&self, address: &str) -> Result<Option<HeldOut>, ApiError> {
        let rows = sqlx::query(
            "SELECT a.id, a.usd_cents, a.expires_at, a.status, \
                    (SELECT contract_address FROM credits.authorization_lines l WHERE l.authorization_id = a.id ORDER BY position LIMIT 1) AS contract_address, \
                    (SELECT item_id FROM credits.authorization_lines l WHERE l.authorization_id = a.id ORDER BY position LIMIT 1) AS item_id, \
                    (SELECT folded_block_time FROM credits.usage_index_cursor c WHERE c.chain_id = a.chain_id) AS folded_time \
             FROM credits.authorizations a WHERE a.address = $1 AND a.status IN ('authorized', 'retired') ORDER BY a.created_at DESC",
        )
        .bind(address)
        .fetch_all(&self.pool)
        .await?;
        if rows.is_empty() {
            return Ok(None);
        }
        let mut cents = 0i64;
        let mut purchases = Vec::new();
        let mut latest: Option<i64> = None;
        for r in rows {
            let usd_cents: i64 = r.get("usd_cents");
            let expires_at: NaiveDateTime = r.get("expires_at");
            let releases = releases_at_seconds(expires_at);
            cents += usd_cents;
            latest = Some(latest.map_or(releases, |l: i64| l.max(releases)));
            purchases.push(HeldPurchaseOut {
                salt: r.get("id"),
                credits: UsdCents::new(usd_cents).credits_ceil(),
                releases_at_seconds: Some(releases),
                contract_address: r.get("contract_address"),
                item_id: r.get("item_id"),
                status: intent_status(&r.get::<String, _>("status")),
            });
        }
        Ok(Some(HeldOut {
            cents,
            credits: UsdCents::new(cents).credits_ceil(),
            releases_at_seconds: latest,
            purchases,
        }))
    }
}

/// Upstream's grace on top of a credit's expiry before its pledge rejoins the
/// balance (`USD_INTENT_EXPIRY_GRACE_SECONDS`): here the estimate of how far
/// the CreditUsed index lags the chain. The sweep itself waits for the index.
pub const RELEASE_GRACE_SECONDS: i64 = 60;

pub fn releases_at_seconds(expires_at: NaiveDateTime) -> i64 {
    expires_at.and_utc().timestamp() + RELEASE_GRACE_SECONDS
}

/// Upstream's pack-order vocabulary over a `credits.purchases` row.
pub fn order_status(status: &str, amount_cents: i64, refunded_cents: i64) -> &'static str {
    match status {
        "pending" => "processing",
        "paid" => "credited",
        "refunded" | "disputed" => {
            if refunded_cents >= amount_cents {
                "reversed"
            } else {
                "partially_reversed"
            }
        }
        "failed" => "failed",
        "abandoned" => "abandoned",
        _ => "processing",
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct CreditOrderOut {
    pub id: String,
    pub provider: &'static str,
    pub status: &'static str,
    pub credits: i64,
    #[serde(rename = "usdCents")]
    pub usd_cents: i64,
    #[serde(rename = "createdAt")]
    pub created_at: i64,
    #[serde(rename = "packId")]
    pub pack_id: String,
}

/// Seller proceeds routed to the treasury: not carried (a resale pays its
/// seller in MANA under the payment rule), so always empty; kept for the
/// clients that read the block.
#[derive(Debug, Clone, Serialize)]
pub struct Earnings {
    pub total: i64,
    #[serde(rename = "availableCents")]
    pub available_cents: i64,
    pub items: Vec<serde_json::Value>,
}

impl Earnings {
    pub fn none() -> Self {
        Self {
            total: 0,
            available_cents: 0,
            items: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct CreditOrdersOut {
    pub total: i64,
    pub orders: Vec<CreditOrderOut>,
    pub earnings: Earnings,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "credits/"))]
pub struct HeldPurchaseOut {
    pub salt: String,
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub credits: i64,
    #[serde(rename = "releasesAtSeconds")]
    #[cfg_attr(feature = "ts", ts(type = "number | null"))]
    pub releases_at_seconds: Option<i64>,
    #[serde(rename = "contractAddress")]
    pub contract_address: Option<String>,
    #[serde(rename = "itemId")]
    pub item_id: Option<String>,
    pub status: &'static str,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "credits/"))]
pub struct HeldOut {
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub cents: i64,
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub credits: i64,
    #[serde(rename = "releasesAtSeconds")]
    #[cfg_attr(feature = "ts", ts(type = "number | null"))]
    pub releases_at_seconds: Option<i64>,
    pub purchases: Vec<HeldPurchaseOut>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pages_clamp_like_upstream() {
        assert_eq!(clamp_page(None, None, 200), (200, 0));
        assert_eq!(clamp_page(Some("500"), Some("10"), 200), (200, 10));
        assert_eq!(clamp_page(Some("500"), None, 500), (500, 0));
        assert_eq!(clamp_page(Some("0"), Some("-3"), 200), (1, 0));
        assert_eq!(clamp_page(Some("abc"), Some("2.9"), 200), (200, 2));
    }

    #[test]
    fn statuses_map_to_the_intent_and_order_vocabularies() {
        assert_eq!(intent_status("authorized"), "PENDING");
        assert_eq!(intent_status("retired"), "PENDING");
        assert_eq!(intent_status("consumed"), "SETTLED");
        assert_eq!(intent_status("released"), "EXPIRED");
        assert_eq!(intent_status("expired"), "EXPIRED");
        assert_eq!(order_status("pending", 500, 0), "processing");
        assert_eq!(order_status("paid", 500, 0), "credited");
        assert_eq!(order_status("refunded", 500, 500), "reversed");
        assert_eq!(order_status("refunded", 500, 250), "partially_reversed");
        assert_eq!(order_status("disputed", 500, 500), "reversed");
        assert_eq!(order_status("abandoned", 500, 0), "abandoned");
    }
}
