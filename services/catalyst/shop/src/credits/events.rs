// from: credits-server@d9c6a261fb7acf3beb926e10f03d1aed33e9771b:src/types/entities.ts (AnalyticsEvent, AnalyticsEventPayload), src/controllers/handlers/stripe-webhook.ts (STRIPE_CREDIT_GRANTED), src/logic/usd-reconciler.ts (USD_PURCHASE_SETTLED)
//! The revenue events credits-server reported to the analytics API: the
//! only server-side records that a paid pack was granted
//! ([`STRIPE_CREDIT_GRANTED`]) and that a purchase paid with Credits settled
//! on chain ([`USD_PURCHASE_SETTLED`]). The browser's own events fire only
//! while the buyer keeps the page open, so purchases went unrecorded or were
//! counted as failures without these.
//!
//! Each is sent once, after the transaction that made it true commits, and
//! fired rather than awaited: the analytics API can neither delay nor fail a
//! payment or a settlement. `orderId` and `salt` are the dedupe keys the
//! client events share. They go under credits-server's context
//! ([`ANALYTICS_CONTEXT`]), so the warehouse reads them as it did.

use serde_json::{Value, json};

use crate::social::analytics::Analytics;

/// The context credits-server reported under (its default `ANALYTICS_CONTEXT`).
pub const ANALYTICS_CONTEXT: &str = "credits-server";
/// A paid pack was granted, reported by the payment webhook.
pub const STRIPE_CREDIT_GRANTED: &str = "stripe_credit_granted";
/// A purchase paid with Credits settled against its on-chain consumption.
pub const USD_PURCHASE_SETTLED: &str = "usd_purchase_settled";

/// A pack purchase the payment webhook just granted.
pub struct PackGranted<'a> {
    /// The order's id; a purchase opened without an order reports its
    /// Checkout Session, else its payment intent.
    pub order_id: &'a str,
    pub user_address: &'a str,
    /// The Credits granted (the order's total).
    pub credits: &'a str,
    /// What the payment charged, in cents (the order's total).
    pub charged_cents: i64,
    pub pack_id: &'a str,
    pub quantity: i32,
}

/// `stripe_credit_granted`, as credits-server sent it. `usdTopUpId` is always
/// null: the node keeps no top-up record apart from the purchase.
pub fn pack_granted(granted: &PackGranted<'_>) -> Value {
    json!({
        "orderId": granted.order_id,
        "userAddress": granted.user_address,
        "amount": number(granted.credits),
        "chargedCents": granted.charged_cents,
        "packId": granted.pack_id,
        "quantity": granted.quantity,
        "usdTopUpId": null,
    })
}

/// One line of a credit's first settlement: the purchase it paid for became
/// revenue. A credit that bought several items reports one per line, as
/// credits-server reported one per intent, all under the credit's salt and
/// transaction.
pub struct PurchaseSettled<'a> {
    pub salt: &'a str,
    pub user_address: &'a str,
    /// What the buyer was charged for the line, in USD cents.
    pub usd_cents: i64,
    /// The line's share of the MANA the log moved, in wei: pro rata by
    /// cents, the rounding remainder to the largest line.
    pub mana_settled_wei: &'a str,
    /// The line's trade (a single-line credit's own).
    pub trade_id: Option<&'a str>,
    pub tx_hash: &'a str,
    /// The credit had been released or had expired (its hold no longer
    /// counted against the balance) and was consumed anyway: the double-spend
    /// backstop settled it.
    pub recovered: bool,
}

/// `usd_purchase_settled`, as credits-server sent it. The wei is a decimal
/// string: 18 decimals do not survive a JSON number.
pub fn purchase_settled(settled: &PurchaseSettled<'_>) -> Value {
    json!({
        "salt": settled.salt,
        "userAddress": settled.user_address,
        "usdCents": settled.usd_cents,
        "manaSettledWei": settled.mana_settled_wei,
        "tradeId": settled.trade_id,
        "txHash": settled.tx_hash,
        "recovered": settled.recovered,
    })
}

/// Fires `event` when analytics is configured; never waits on it.
pub fn fire(analytics: Option<&Analytics>, event: &str, properties: Value) {
    if let Some(analytics) = analytics {
        analytics.fire_event(event, properties);
    }
}

/// A credits amount as the JSON number credits-server sent: whole Credits
/// as an integer, a fraction as a float, anything unreadable as null.
fn number(credits: &str) -> Value {
    if let Ok(whole) = credits.parse::<i64>() {
        return json!(whole);
    }
    credits
        .parse::<f64>()
        .ok()
        .filter(|f| f.is_finite())
        .map_or(Value::Null, |f| json!(f))
}

/// An analytics client posting to a local endpoint, and what it received.
#[cfg(test)]
pub(crate) mod capture {
    use std::time::Duration;

    use axum::Router;
    use axum::extract::Request;
    use axum::routing::post;
    use serde_json::Value;
    use tokio::sync::mpsc;

    use crate::social::analytics::{Analytics, Settings};

    pub(crate) struct Captured {
        pub(crate) analytics: Analytics,
        received: mpsc::UnboundedReceiver<Value>,
    }

    impl Captured {
        /// The next payload posted, or `None` after `wait`.
        pub(crate) async fn next(&mut self, wait: Duration) -> Option<Value> {
            tokio::time::timeout(wait, self.received.recv())
                .await
                .ok()
                .flatten()
        }
    }

    pub(crate) async fn analytics() -> Captured {
        let (sent, received) = mpsc::unbounded_channel();
        let app = Router::new().route(
            "/events",
            post(move |request: Request| {
                let sent = sent.clone();
                async move {
                    let bytes = axum::body::to_bytes(request.into_body(), 1 << 20)
                        .await
                        .unwrap();
                    let _ = sent.send(serde_json::from_slice::<Value>(&bytes).unwrap());
                    axum::http::StatusCode::OK
                }
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        let analytics = Analytics::new(
            &crate::http::Outbound::for_tests(),
            Settings {
                context: Some("social".into()),
                api_url: Some(format!("http://{address}/events")),
                api_token: Some("token".into()),
                environment: Some("test".into()),
                request_timeout_ms: None,
            },
        )
        .unwrap()
        .unwrap()
        .with_context(super::ANALYTICS_CONTEXT);
        Captured {
            analytics,
            received,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    #[test]
    fn the_payloads_are_credits_servers() {
        assert_eq!(
            pack_granted(&PackGranted {
                order_id: "ord_1",
                user_address: "0xab",
                credits: "150",
                charged_cents: 1599,
                pack_id: "pack_150",
                quantity: 3,
            }),
            json!({ "orderId": "ord_1", "userAddress": "0xab", "amount": 150, "chargedCents": 1599,
                    "packId": "pack_150", "quantity": 3, "usdTopUpId": null })
        );
        assert_eq!(number("12.5"), json!(12.5));
        assert_eq!(number("x"), Value::Null);
        assert_eq!(
            purchase_settled(&PurchaseSettled {
                salt: "0x01",
                user_address: "0xab",
                usd_cents: 2500,
                mana_settled_wei: "1234567890123456789012",
                trade_id: None,
                tx_hash: "0x02",
                recovered: true,
            }),
            json!({ "salt": "0x01", "userAddress": "0xab", "usdCents": 2500,
                    "manaSettledWei": "1234567890123456789012", "tradeId": null,
                    "txHash": "0x02", "recovered": true })
        );
    }

    #[tokio::test]
    async fn an_event_goes_under_credits_servers_context() {
        let mut captured = capture::analytics().await;
        fire(
            Some(&captured.analytics),
            STRIPE_CREDIT_GRANTED,
            json!({ "orderId": "ord_1" }),
        );
        let posted = captured.next(Duration::from_secs(5)).await.unwrap();
        assert_eq!(posted["event"], STRIPE_CREDIT_GRANTED);
        assert_eq!(posted["context"], ANALYTICS_CONTEXT);
        assert_eq!(posted["body"]["orderId"], "ord_1");
        fire(None, STRIPE_CREDIT_GRANTED, json!({}));
    }
}
