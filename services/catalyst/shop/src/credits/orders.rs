// from: credits-server@465e628bc:src/controllers/routes.ts:287-300 (the pack-order routes and their comments: cancel, cancel-return, resume), src/logic/usd-reconciler.ts (markAbandonedCheckouts, DEFAULT_ABANDONED_ORDER_AFTER_SECONDS)
//! Pack orders after the Checkout Session opened (`credits.purchases` rows
//! of the order flow): the buyer walks away (`cancel`: the session is
//! expired at Stripe, then the order retired), comes back (`resume`: the
//! same hosted page while Stripe still has it open, the order retired when
//! it does not), or lands on Stripe's cancel page (`cancel-return`: the
//! cancellation confirmed with Stripe before anything is recorded), and the
//! sweep that retires orders nobody paid within a session's lifetime. Every
//! Stripe call runs outside a transaction; the state change is one guarded
//! UPDATE (`status = 'pending'`), so a late webhook or a second click moves
//! nothing twice. An order whose session completed with its payment still
//! processing is never retired by these routes or the sweep, and a payment
//! that settles after its order was retired still credits it
//! (`packs::SETTLEABLE_STATUSES`).

use sqlx::Row;

use super::errors::ApiError;
use super::packs::OrderRow;
use super::store::Credits;

/// A Stripe Checkout Session lives 24 h; two more for clock skew and webhook backlogs.
pub const ABANDON_AFTER: chrono::Duration = chrono::Duration::hours(26);

/// What Stripe says about a session, as the order routes read it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionState {
    pub id: String,
    /// `open`, `complete` or `expired`.
    pub status: String,
    /// `paid`, `unpaid` or `no_payment_required`.
    pub payment_status: String,
    pub url: Option<String>,
    pub payment_intent: Option<String>,
}

impl SessionState {
    pub fn is_payable(&self) -> bool {
        self.status == "open" && self.url.is_some()
    }

    pub fn is_paid(&self) -> bool {
        self.status == "complete" || self.payment_status == "paid"
    }
}

/// How a resume answers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Resume {
    /// The hosted page, still payable.
    Url(String),
    /// The buyer paid: the order's wire status (`processing` until the webhook lands, then `credited`).
    Paid(&'static str),
    /// Retired now, or already: the order's wire status.
    NoLongerPayable(&'static str),
}

impl Credits {
    /// The order and its Checkout Session id, for the signer only.
    pub async fn get_order_with_session(
        &self,
        order_id: &str,
        address: &str,
    ) -> Result<Option<(OrderRow, Option<String>)>, ApiError> {
        let row = sqlx::query(
            "SELECT order_id, address, sku, credits::text AS credits, amount_cents, status, failure_reason, stripe_checkout_session \
             FROM credits.purchases WHERE order_id = $1 AND address = $2",
        )
        .bind(order_id)
        .bind(address)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(|r| {
            let session: Option<String> = r.get("stripe_checkout_session");
            (
                OrderRow {
                    order_id: r.get("order_id"),
                    address: r.get("address"),
                    sku: r.get("sku"),
                    credits: r.get("credits"),
                    amount_cents: r.get("amount_cents"),
                    status: r.get("status"),
                    failure_reason: r.get("failure_reason"),
                },
                session,
            )
        }))
    }

    /// The order by id alone (the unauthenticated cancel return: Stripe is
    /// then asked before anything moves).
    pub async fn get_order_with_session_any(
        &self,
        order_id: &str,
    ) -> Result<Option<(String, Option<String>)>, ApiError> {
        let row = sqlx::query(
            "SELECT address, stripe_checkout_session FROM credits.purchases WHERE order_id = $1",
        )
        .bind(order_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(|r| (r.get("address"), r.get("stripe_checkout_session"))))
    }

    /// The order behind a session id, for the unauthenticated cancel return.
    pub async fn order_status_by_session(
        &self,
        session_id: &str,
    ) -> Result<Option<(String, String)>, ApiError> {
        let row = sqlx::query(
            "SELECT order_id, status FROM credits.purchases WHERE stripe_checkout_session = $1",
        )
        .bind(session_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(|r| {
            (
                r.get::<Option<String>, _>("order_id").unwrap_or_default(),
                r.get("status"),
            )
        }))
    }

    /// Retires pending Checkout Session orders older than `after` (upstream
    /// `markAbandonedCheckouts`); a paid one is never touched, and neither is
    /// one whose payment is processing (its payment intent is recorded): a
    /// bank debit outlives the session, and its outcome settles the order.
    pub async fn abandon_stale_pack_orders(
        &self,
        after: chrono::Duration,
    ) -> Result<u64, ApiError> {
        let cutoff = super::store::now() - after;
        let res = sqlx::query(
            "UPDATE credits.purchases SET status = 'abandoned', failure_reason = COALESCE(failure_reason, 'checkout session lifetime elapsed'), updated_at = timezone('UTC', now()) \
             WHERE status = 'pending' AND stripe_checkout_session IS NOT NULL AND stripe_payment_intent IS NULL AND created_at < $1",
        )
        .bind(cutoff)
        .execute(&self.pool)
        .await?;
        Ok(res.rows_affected())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_session_is_payable_only_while_open_with_a_page() {
        let open = SessionState {
            id: "cs".into(),
            status: "open".into(),
            payment_status: "unpaid".into(),
            url: Some("https://checkout.stripe.test/cs".into()),
            payment_intent: None,
        };
        assert!(open.is_payable() && !open.is_paid());
        let complete = SessionState {
            status: "complete".into(),
            payment_status: "paid".into(),
            url: None,
            ..open.clone()
        };
        assert!(!complete.is_payable() && complete.is_paid());
        let expired = SessionState {
            status: "expired".into(),
            url: None,
            ..open.clone()
        };
        assert!(!expired.is_payable() && !expired.is_paid());
        assert_eq!(ABANDON_AFTER, chrono::Duration::seconds(26 * 3600));
    }
}
