//! The Stripe client (PaymentIntents for the marketplace's card flow,
//! Checkout Sessions for the explorer's order flow) over the ONE outbound
//! client, and the webhook signature check: `Stripe-Signature` is
//! `t=<unix>,v1=<hex hmac-sha256(secret, "<t>.<body>")>[,v1=...]`, accepted
//! within [`SIGNATURE_TOLERANCE_SECS`] of the request clock and compared in
//! constant time. The secret key and the webhook secret arrive through
//! `STRIPE_SECRET_KEY(_FILE)` / `STRIPE_WEBHOOK_SECRET(_FILE)` and are never
//! logged; a failing Stripe call logs the status and a truncated body only.

use hmac::{Hmac, KeyInit, Mac};
use serde::Deserialize;
use sha2::Sha256;

use super::captcha::form_encode;
use super::errors::ApiError;
use super::packs::PackOrder;
use crate::http::Outbound;

type HmacSha256 = Hmac<Sha256>;

pub const DEFAULT_STRIPE_API_BASE: &str = "https://api.stripe.com";

pub const SIGNATURE_TOLERANCE_SECS: i64 = 300;

#[derive(Debug, Clone, Deserialize)]
pub struct PaymentIntent {
    pub id: String,
    pub client_secret: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CheckoutSession {
    pub id: String,
    pub url: String,
}

#[derive(Clone)]
pub struct StripeClient {
    secret: String,
    base_url: String,
    client: Outbound,
}

impl std::fmt::Debug for StripeClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StripeClient")
            .field("base_url", &self.base_url)
            .finish_non_exhaustive()
    }
}

/// The per-request budget of a Stripe call on the ONE outbound client (the
/// old crate built a second client for this 15 s; the shared one keeps its
/// pin cache and pool, the request carries the budget).
pub const REQUEST_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(15);

impl StripeClient {
    pub fn new(secret: String, base_url: String, client: Outbound) -> Self {
        Self {
            secret,
            base_url: base_url.trim_end_matches('/').to_string(),
            client,
        }
    }

    async fn post_form<T: serde::de::DeserializeOwned>(
        &self,
        path: &str,
        params: &[(&str, &str)],
        idempotency_key: &str,
        what: &str,
        gateway: bool,
    ) -> Result<T, ApiError> {
        let fail = |m: String| {
            if gateway {
                ApiError::bad_gateway(m)
            } else {
                ApiError::Internal(m)
            }
        };
        let url = format!("{}{path}", self.base_url);
        let resp = self
            .client
            .post(&url)
            .timeout(REQUEST_TIMEOUT)
            .header(
                reqwest::header::AUTHORIZATION,
                format!("Bearer {}", self.secret),
            )
            .header("Idempotency-Key", idempotency_key)
            .header(
                reqwest::header::CONTENT_TYPE,
                "application/x-www-form-urlencoded",
            )
            .body(form_encode(params))
            .send()
            .await
            .map_err(|e| fail(format!("stripe request failed: {e}")))?;

        if !resp.status().is_success() {
            let status = resp.status().as_u16();
            let body = resp.text().await.unwrap_or_default();
            tracing::error!(status, body = %truncate(&body, 500), "stripe {what} create failed");
            return Err(fail(format!("stripe returned status {status}")));
        }

        resp.json()
            .await
            .map_err(|e| fail(format!("stripe response parse failed: {e}")))
    }

    pub async fn create_payment_intent(
        &self,
        amount_cents: i64,
        currency: &str,
        address: &str,
        sku: &str,
        credits: &str,
        idempotency_key: &str,
    ) -> Result<PaymentIntent, ApiError> {
        let amount = amount_cents.to_string();
        let params = [
            ("amount", amount.as_str()),
            ("currency", currency),
            ("metadata[address]", address),
            ("metadata[sku]", sku),
            ("metadata[credits]", credits),
            ("automatic_payment_methods[enabled]", "true"),
        ];
        self.post_form(
            "/v1/payment_intents",
            &params,
            idempotency_key,
            "payment_intent",
            false,
        )
        .await
    }

    /// `GET /v1/checkout/sessions/{id}`: what Stripe holds for the order's
    /// session (`status`, `payment_status`, the hosted `url` while open, the
    /// `payment_intent` once one exists). A 404 is `Ok(None)`.
    pub async fn retrieve_checkout_session(
        &self,
        session_id: &str,
    ) -> Result<Option<super::orders::SessionState>, ApiError> {
        let url = format!(
            "{}/v1/checkout/sessions/{}",
            self.base_url,
            percent_path(session_id)
        );
        let resp = self
            .client
            .get(&url)
            .timeout(REQUEST_TIMEOUT)
            .header(
                reqwest::header::AUTHORIZATION,
                format!("Bearer {}", self.secret),
            )
            .send()
            .await
            .map_err(|e| ApiError::bad_gateway(format!("stripe request failed: {e}")))?;
        if resp.status().as_u16() == 404 {
            return Ok(None);
        }
        if !resp.status().is_success() {
            let status = resp.status().as_u16();
            let body = resp.text().await.unwrap_or_default();
            tracing::error!(status, body = %truncate(&body, 500), "stripe checkout session retrieve failed");
            return Err(ApiError::bad_gateway(format!(
                "stripe returned status {status}"
            )));
        }
        let v: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| ApiError::bad_gateway(format!("stripe response parse failed: {e}")))?;
        Ok(Some(session_state(&v)))
    }

    /// `POST /v1/checkout/sessions/{id}/expire`: the buyer walked away; an
    /// already expired or completed session answers 400 at Stripe, which is
    /// `Ok(None)` here (the caller re-reads the session).
    pub async fn expire_checkout_session(
        &self,
        session_id: &str,
        idempotency_key: &str,
    ) -> Result<Option<super::orders::SessionState>, ApiError> {
        let url = format!(
            "{}/v1/checkout/sessions/{}/expire",
            self.base_url,
            percent_path(session_id)
        );
        let resp = self
            .client
            .post(&url)
            .timeout(REQUEST_TIMEOUT)
            .header(
                reqwest::header::AUTHORIZATION,
                format!("Bearer {}", self.secret),
            )
            .header("Idempotency-Key", idempotency_key)
            .header(
                reqwest::header::CONTENT_TYPE,
                "application/x-www-form-urlencoded",
            )
            .body(String::new())
            .send()
            .await
            .map_err(|e| ApiError::bad_gateway(format!("stripe request failed: {e}")))?;
        if resp.status().as_u16() == 400 || resp.status().as_u16() == 404 {
            return Ok(None);
        }
        if !resp.status().is_success() {
            let status = resp.status().as_u16();
            let body = resp.text().await.unwrap_or_default();
            tracing::error!(status, body = %truncate(&body, 500), "stripe checkout session expire failed");
            return Err(ApiError::bad_gateway(format!(
                "stripe returned status {status}"
            )));
        }
        let v: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| ApiError::bad_gateway(format!("stripe response parse failed: {e}")))?;
        Ok(Some(session_state(&v)))
    }

    /// One line of `order.quantity` units at the pack's unit price, so
    /// Stripe's `amount_total` is the product the order row stores and the
    /// webhook compares against.
    pub async fn create_checkout_session(
        &self,
        order: &PackOrder<'_>,
        address: &str,
        order_id: &str,
        success_url: &str,
        cancel_url: &str,
        idempotency_key: &str,
    ) -> Result<CheckoutSession, ApiError> {
        let params = checkout_session_params(order, address, order_id, success_url, cancel_url);
        let params: Vec<(&str, &str)> = params.iter().map(|(k, v)| (*k, v.as_str())).collect();
        self.post_form(
            "/v1/checkout/sessions",
            &params,
            idempotency_key,
            "checkout session",
            true,
        )
        .await
    }
}

fn checkout_session_params(
    order: &PackOrder<'_>,
    address: &str,
    order_id: &str,
    success_url: &str,
    cancel_url: &str,
) -> Vec<(&'static str, String)> {
    let pack = order.pack;
    let mut params = vec![
        ("mode", "payment".into()),
        ("line_items[0][price_data][currency]", pack.currency.clone()),
        (
            "line_items[0][price_data][unit_amount]",
            pack.price_cents.to_string(),
        ),
        (
            "line_items[0][price_data][product_data][name]",
            pack.title.clone(),
        ),
        ("line_items[0][quantity]", order.quantity.to_string()),
        ("success_url", success_url.into()),
        ("cancel_url", cancel_url.into()),
        ("metadata[address]", address.into()),
        ("metadata[sku]", pack.sku.clone()),
        ("metadata[quantity]", order.quantity.to_string()),
        ("metadata[credits]", order.credits.clone()),
        ("metadata[orderId]", order_id.into()),
    ];
    if let Some(source) = order.source {
        params.push(("metadata[source]", source.into()));
    }
    params
}

/// A Stripe id inside a path: the ids are `[A-Za-z0-9_]`, anything else is escaped.
fn percent_path(id: &str) -> String {
    let mut out = String::with_capacity(id.len());
    for b in id.bytes() {
        if b.is_ascii_alphanumeric() || b == b'_' || b == b'-' {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

fn session_state(v: &serde_json::Value) -> super::orders::SessionState {
    let text = |k: &str| v.get(k).and_then(|x| x.as_str()).map(str::to_string);
    super::orders::SessionState {
        id: text("id").unwrap_or_default(),
        status: text("status").unwrap_or_default(),
        payment_status: text("payment_status").unwrap_or_default(),
        url: text("url"),
        payment_intent: v.get("payment_intent").and_then(|p| {
            p.as_str()
                .map(str::to_string)
                .or_else(|| p.get("id").and_then(|i| i.as_str()).map(str::to_string))
        }),
    }
}

fn truncate(s: &str, max: usize) -> &str {
    if s.len() <= max {
        s
    } else {
        let mut end = max;
        while !s.is_char_boundary(end) && end > 0 {
            end -= 1;
        }
        &s[..end]
    }
}

pub fn verify_stripe_signature(
    secret: &str,
    sig_header: &str,
    raw_body: &[u8],
    tolerance_secs: i64,
    now_unix: i64,
) -> bool {
    let mut timestamp: Option<i64> = None;
    let mut v1_sigs: Vec<&str> = Vec::new();
    for part in sig_header.split(',') {
        let part = part.trim();
        let Some((key, value)) = part.split_once('=') else {
            continue;
        };
        match key {
            "t" => timestamp = value.parse::<i64>().ok(),
            "v1" => v1_sigs.push(value),
            _ => {}
        }
    }

    let Some(t) = timestamp else {
        return false;
    };
    if v1_sigs.is_empty() {
        return false;
    }

    if (now_unix as i128 - t as i128).abs() > tolerance_secs as i128 {
        return false;
    }

    let mut signed_payload = Vec::with_capacity(raw_body.len() + 16);
    signed_payload.extend_from_slice(t.to_string().as_bytes());
    signed_payload.push(b'.');
    signed_payload.extend_from_slice(raw_body);

    let mut mac = match HmacSha256::new_from_slice(secret.as_bytes()) {
        Ok(m) => m,
        Err(_) => return false,
    };
    mac.update(&signed_payload);
    let expected = hex::encode(mac.finalize().into_bytes());

    v1_sigs
        .iter()
        .any(|cand| constant_time_eq(expected.as_bytes(), cand.as_bytes()))
}

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

/// A `Stripe-Signature` header over `body` at `t`, for the tests.
#[cfg(test)]
pub(crate) fn sign_for_tests(secret: &str, t: i64, body: &[u8]) -> String {
    let mut signed = Vec::new();
    signed.extend_from_slice(t.to_string().as_bytes());
    signed.push(b'.');
    signed.extend_from_slice(body);
    let mut mac = HmacSha256::new_from_slice(secret.as_bytes()).unwrap();
    mac.update(&signed);
    let sig = hex::encode(mac.finalize().into_bytes());
    format!("t={t},v1={sig}")
}

#[cfg(test)]
mod tests {
    use super::*;

    const SECRET: &str = "whsec_test_secret_abc123";

    fn sign(secret: &str, t: i64, body: &[u8]) -> String {
        sign_for_tests(secret, t, body)
    }

    #[test]
    fn a_multi_pack_checkout_is_one_line_of_unit_price_times_quantity() {
        let pack = crate::credits::packs::PackRow {
            sku: "pack_5".into(),
            title: "50 Credits".into(),
            credits: "50".into(),
            price_cents: 500,
            currency: "usd".into(),
            sort_order: 0,
        };
        let order = pack.order(3).unwrap();
        let params = checkout_session_params(
            &order,
            "0xabc",
            "ord_1",
            "https://shop/ok",
            "https://shop/no",
        );
        let get = |k: &str| {
            params
                .iter()
                .find(|(key, _)| *key == k)
                .map(|(_, v)| v.as_str())
        };
        assert_eq!(get("line_items[0][price_data][unit_amount]"), Some("500"));
        assert_eq!(get("line_items[0][quantity]"), Some("3"));
        assert_eq!(get("metadata[quantity]"), Some("3"));
        assert_eq!(
            get("metadata[credits]"),
            Some("150"),
            "the metadata carries the order's total"
        );
        assert_eq!(
            order.amount_cents, 1500,
            "Stripe's amount_total is what the order row expects"
        );
        assert_eq!(get("metadata[source]"), None);
        let builder = pack.order(1).unwrap().from_source(Some("builder"));
        let params = checkout_session_params(
            &builder,
            "0xabc",
            "ord_2",
            "https://builder/ok",
            "https://builder/no",
        );
        assert!(
            params
                .iter()
                .any(|(k, v)| *k == "metadata[source]" && v == "builder")
        );
    }

    #[test]
    fn valid_signature_accepted() {
        let body = br#"{"id":"evt_1","type":"payment_intent.succeeded"}"#;
        let now = 1_690_000_000;
        let header = sign(SECRET, now, body);
        assert!(verify_stripe_signature(SECRET, &header, body, 300, now));
    }

    #[test]
    fn tampered_body_rejected() {
        let body = br#"{"id":"evt_1","type":"payment_intent.succeeded"}"#;
        let now = 1_690_000_000;
        let header = sign(SECRET, now, body);
        let tampered = br#"{"id":"evt_1","type":"payment_intent.SUCCEEDED!"}"#;
        assert!(!verify_stripe_signature(
            SECRET, &header, tampered, 300, now
        ));
    }

    #[test]
    fn wrong_secret_rejected() {
        let body = br#"{"id":"evt_1"}"#;
        let now = 1_690_000_000;
        let header = sign(SECRET, now, body);
        assert!(!verify_stripe_signature(
            "whsec_wrong_secret",
            &header,
            body,
            300,
            now
        ));
    }

    #[test]
    fn stale_timestamp_rejected() {
        let body = br#"{"id":"evt_1"}"#;
        let signed_at = 1_690_000_000;
        let header = sign(SECRET, signed_at, body);
        assert!(!verify_stripe_signature(
            SECRET,
            &header,
            body,
            300,
            signed_at + 301
        ));
        assert!(!verify_stripe_signature(
            SECRET,
            &header,
            body,
            300,
            signed_at - 301
        ));
        assert!(verify_stripe_signature(
            SECRET,
            &header,
            body,
            300,
            signed_at + 300
        ));
    }

    #[test]
    fn multiple_v1_with_one_matching_accepted() {
        let body = br#"{"id":"evt_1"}"#;
        let now = 1_690_000_000;
        let valid = sign(SECRET, now, body);
        let valid_sig = valid.rsplit("v1=").next().unwrap();
        let header = format!(
            "t={now},v1=deadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef,v1={valid_sig},v0=ignored"
        );
        assert!(verify_stripe_signature(SECRET, &header, body, 300, now));
    }

    #[test]
    fn extreme_timestamp_does_not_panic() {
        let body = br#"{"id":"evt_1"}"#;
        let now = 1_690_000_000;
        assert!(!verify_stripe_signature(
            SECRET,
            &format!("t={},v1=abc", i64::MIN),
            body,
            300,
            now
        ));
        assert!(!verify_stripe_signature(
            SECRET,
            &format!("t={},v1=abc", i64::MAX),
            body,
            300,
            now
        ));
    }

    #[test]
    fn malformed_header_rejected() {
        let body = br#"{"id":"evt_1"}"#;
        let now = 1_690_000_000;
        assert!(!verify_stripe_signature(
            SECRET,
            "v1=abc123",
            body,
            300,
            now
        ));
        assert!(!verify_stripe_signature(
            SECRET,
            &format!("t={now}"),
            body,
            300,
            now
        ));
        assert!(!verify_stripe_signature(SECRET, "garbage", body, 300, now));
        assert!(!verify_stripe_signature(SECRET, "", body, 300, now));
        assert!(!verify_stripe_signature(
            SECRET,
            "t=notanumber,v1=abc",
            body,
            300,
            now
        ));
    }

    #[test]
    fn form_encoding_is_stripes_bracket_syntax_percent_encoded() {
        let body = form_encode(&[
            ("amount", "500"),
            ("metadata[sku]", "credits_usd_5"),
            ("success_url", "https://x/y?a=1"),
        ]);
        assert_eq!(
            body,
            "amount=500&metadata%5Bsku%5D=credits_usd_5&success_url=https%3A%2F%2Fx%2Fy%3Fa%3D1"
        );
    }
}
