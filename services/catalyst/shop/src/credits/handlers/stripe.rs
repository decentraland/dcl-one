//! `POST /stripe/webhook`: the signature is verified over the raw body
//! before any row is touched; then ONE transaction lands the event claim,
//! the purchase flip and the wallet move, granting `purchases.credits`
//! (captured at intent/session creation), never the event's metadata, and
//! only for a settled payment in the purchase's currency and amount. A
//! reversal REVOKES the granted Credits, bounded by the purchase.
//!
//! The Stripe endpoint must deliver every event handled below. A delayed
//! payment method completes its session unpaid and is credited only on
//! `checkout.session.async_payment_succeeded`, so an endpoint without that
//! event never credits those orders.

use axum::Json;
use axum::body::Bytes;
use axum::extract::State;
use axum::extract::rejection::BytesRejection;
use axum::http::HeaderMap;
use serde_json::{Value as JsonValue, json};

use crate::credits::SharedState;
use crate::credits::admin_store::GrantRequest;
use crate::credits::errors::ApiError;
use crate::credits::packs::{Charge, MarkPaidOutcome, ReversalOutcome};
use crate::credits::store::Credits;
use crate::credits::stripe::{SIGNATURE_TOLERANCE_SECS, verify_stripe_signature};

type Tx<'a> = sqlx::Transaction<'a, sqlx::Postgres>;

pub const STRIPE_ACTOR: &str = "stripe";

fn ok(detail: &str) -> Json<JsonValue> {
    Json(json!({ "ok": true, "detail": detail }))
}

pub async fn webhook(
    State(state): State<SharedState>,
    headers: HeaderMap,
    body: Result<Bytes, BytesRejection>,
) -> Result<Json<JsonValue>, ApiError> {
    let body = body.map_err(|e| ApiError::bad_request(e.body_text()))?;

    let Some(secret) = state.settings.stripe_webhook_secret.as_ref() else {
        return Err(ApiError::not_implemented(
            "stripe webhook disabled (STRIPE_WEBHOOK_SECRET unset)",
        ));
    };

    let sig_header = headers
        .get("stripe-signature")
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| ApiError::bad_request("missing Stripe-Signature header"))?;

    let now = chrono::Utc::now().timestamp();
    if !verify_stripe_signature(secret, sig_header, &body, SIGNATURE_TOLERANCE_SECS, now) {
        return Err(ApiError::bad_request("invalid Stripe signature"));
    }

    let event: JsonValue =
        serde_json::from_slice(&body).map_err(|_| ApiError::bad_request("invalid webhook body"))?;

    let event_id = event
        .get("id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| ApiError::bad_request("event missing id"))?
        .to_string();
    let event_type = event
        .get("type")
        .and_then(|v| v.as_str())
        .ok_or_else(|| ApiError::bad_request("event missing type"))?
        .to_string();

    let mut tx = state.credits.pool.begin().await?;
    let needs_processing =
        Credits::record_stripe_event_in_tx(&mut tx, &event_id, &event_type, &event).await?;
    if !needs_processing {
        tx.commit().await?;
        return Ok(ok("already processed"));
    }

    let object = event
        .get("data")
        .and_then(|d| d.get("object"))
        .cloned()
        .unwrap_or(JsonValue::Null);

    match event_type.as_str() {
        "payment_intent.succeeded" => {
            let pi_id = object
                .get("id")
                .and_then(|v| v.as_str())
                .ok_or_else(|| ApiError::bad_request("payment_intent missing id"))?;
            let charged = charge(&object, "amount", "payment_intent")?;

            match state
                .credits
                .mark_purchase_paid_in_tx(&mut tx, pi_id, &event_id, charged)
                .await?
            {
                MarkPaidOutcome::Granted { address, credits } => {
                    let detail = json!({
                        "source": "stripe",
                        "eventId": event_id,
                        "paymentIntent": pi_id,
                        "sku": object.get("metadata").and_then(|m| m.get("sku")),
                    });
                    state
                        .credits
                        .admin_grant_credits_in_tx(
                            &mut tx,
                            GrantRequest {
                                address: &address,
                                amount: &credits,
                                kind: "purchase",
                                reason: Some("stripe purchase"),
                                actor: Some(STRIPE_ACTOR),
                                idempotency_key: Some(&event_id),
                                detail: &detail,
                            },
                        )
                        .await?;
                }
                MarkPaidOutcome::AmountMismatch {
                    expected_cents,
                    charged_cents,
                } => {
                    tracing::warn!(event_id = %event_id, payment_intent = %pi_id, expected_cents, charged_cents, "payment_intent.succeeded amount mismatch; NOT granting credits");
                }
                MarkPaidOutcome::CurrencyMismatch { expected, charged } => {
                    tracing::warn!(event_id = %event_id, payment_intent = %pi_id, expected = %expected, charged = %charged, "payment_intent.succeeded currency mismatch; NOT granting credits");
                }
                MarkPaidOutcome::NoPendingPurchase => {
                    tracing::warn!(event_id = %event_id, payment_intent = %pi_id, "payment_intent.succeeded matched no pending purchase; no grant");
                }
            }
        }
        "checkout.session.completed" | "checkout.session.async_payment_succeeded" => {
            settle_session(&state, &mut tx, &object, &event_id, &event_type).await?;
        }
        "checkout.session.expired" => {
            mark_session_terminal(&mut tx, &object, &event_id, "abandoned", None).await?;
        }
        "checkout.session.async_payment_failed" => {
            let reason = object
                .get("last_payment_error")
                .and_then(|e| e.get("message"))
                .and_then(|v| v.as_str());
            mark_session_terminal(&mut tx, &object, &event_id, "failed", reason).await?;
        }
        "charge.refunded" => {
            apply_partial_refund(&state, &mut tx, &object, &event_id).await?;
        }
        "charge.dispute.created" | "charge.dispute.funds_withdrawn" => {
            apply_full_reversal(&state, &mut tx, &object, &event_id, "disputed").await?;
        }
        _ => {}
    }

    tx.commit().await?;
    Ok(ok("processed"))
}

/// The amount and currency an event says were charged; both are required,
/// since the amount is only meaningful in its currency's minor unit.
fn charge<'a>(
    object: &'a JsonValue,
    amount_field: &str,
    what: &str,
) -> Result<Charge<'a>, ApiError> {
    let cents = object
        .get(amount_field)
        .and_then(|v| v.as_i64())
        .ok_or_else(|| ApiError::bad_request(format!("{what} missing {amount_field}")))?;
    let currency = object
        .get("currency")
        .and_then(|v| v.as_str())
        .ok_or_else(|| ApiError::bad_request(format!("{what} missing currency")))?;
    Ok(Charge { cents, currency })
}

/// Whether a Checkout Session's money has arrived: Stripe says `paid`.
/// `checkout.session.completed` also fires for a delayed method (a bank
/// debit) while it is still `unpaid`; that one settles later as
/// `checkout.session.async_payment_succeeded` or `_failed`. An absent
/// status is not settled, and neither is `no_payment_required`: every
/// pack has a price, so a session that needs no payment grants nothing.
pub(crate) fn session_payment_settled(session: &JsonValue) -> bool {
    session.get("payment_status").and_then(|v| v.as_str()) == Some("paid")
}

/// `checkout.session.completed` / `async_payment_succeeded`: the order is
/// marked paid and credited only once the session is settled. An unpaid
/// completion marks it processing instead (pending, with the session's
/// payment intent), so nothing but the async outcome retires it. Whichever
/// settled event lands first flips it; any later one finds it paid.
async fn settle_session(
    state: &SharedState,
    tx: &mut Tx<'_>,
    object: &JsonValue,
    event_id: &str,
    event_type: &str,
) -> Result<(), ApiError> {
    let session_id = object
        .get("id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| ApiError::bad_request("checkout.session missing id"))?;
    let payment_intent = object.get("payment_intent").and_then(|v| v.as_str());
    if !session_payment_settled(object) {
        let payment_status = object.get("payment_status").and_then(|v| v.as_str());
        match (payment_status, payment_intent) {
            (Some("unpaid"), Some(payment_intent)) => {
                let marked =
                    Credits::mark_order_processing_by_session_in_tx(tx, session_id, payment_intent)
                        .await?;
                tracing::info!(event_id = %event_id, event_type = %event_type, checkout_session = %session_id, marked, "checkout.session payment still processing; no grant until it settles");
            }
            _ => {
                tracing::warn!(event_id = %event_id, event_type = %event_type, checkout_session = %session_id, payment_status = payment_status.unwrap_or("absent"), "checkout.session not paid; NOT granting credits");
            }
        }
        return Ok(());
    }
    let charged = charge(object, "amount_total", "checkout.session")?;

    match state
        .credits
        .mark_order_paid_by_session_in_tx(tx, session_id, event_id, charged, payment_intent)
        .await?
    {
        MarkPaidOutcome::Granted { address, credits } => {
            let detail = json!({
                "source": "stripe",
                "eventId": event_id,
                "checkoutSession": session_id,
                "sku": object.get("metadata").and_then(|m| m.get("sku")),
                "orderId": object.get("metadata").and_then(|m| m.get("orderId")),
            });
            state
                .credits
                .admin_grant_credits_in_tx(
                    tx,
                    GrantRequest {
                        address: &address,
                        amount: &credits,
                        kind: "purchase",
                        reason: Some("stripe checkout purchase"),
                        actor: Some(STRIPE_ACTOR),
                        idempotency_key: Some(event_id),
                        detail: &detail,
                    },
                )
                .await?;
        }
        MarkPaidOutcome::AmountMismatch {
            expected_cents,
            charged_cents,
        } => {
            tracing::warn!(event_id = %event_id, event_type = %event_type, checkout_session = %session_id, expected_cents, charged_cents, "checkout.session amount mismatch; NOT granting credits");
        }
        MarkPaidOutcome::CurrencyMismatch { expected, charged } => {
            tracing::warn!(event_id = %event_id, event_type = %event_type, checkout_session = %session_id, expected = %expected, charged = %charged, "checkout.session currency mismatch; NOT granting credits");
        }
        MarkPaidOutcome::NoPendingPurchase => {
            tracing::warn!(event_id = %event_id, event_type = %event_type, checkout_session = %session_id, "checkout.session matched no unpaid order; no grant");
        }
    }
    Ok(())
}

/// Retires a pending order only. Stripe does not order deliveries, so a
/// failure reaching an order already credited is a conflict to verify
/// against the session at Stripe, never a reason to revoke on its own.
async fn mark_session_terminal(
    tx: &mut Tx<'_>,
    object: &JsonValue,
    event_id: &str,
    status: &str,
    failure_reason: Option<&str>,
) -> Result<(), ApiError> {
    let Some(session_id) = object.get("id").and_then(|v| v.as_str()) else {
        tracing::warn!(event_id = %event_id, status = %status, "checkout.session event missing id");
        return Ok(());
    };
    let changed =
        Credits::mark_order_status_by_session_on(&mut **tx, session_id, status, failure_reason)
            .await?;
    if !changed {
        let current = Credits::order_status_by_session_on(&mut **tx, session_id).await?;
        let credited = matches!(current.as_deref(), Some("paid" | "refunded" | "disputed"));
        if status == "failed" && credited {
            tracing::error!(event_id = %event_id, checkout_session = %session_id, order_status = current.as_deref().unwrap_or("none"), "checkout.session payment failure for an order already credited; credits NOT revoked: verify the session at Stripe");
        } else {
            tracing::info!(event_id = %event_id, checkout_session = %session_id, status = %status, order_status = current.as_deref().unwrap_or("none"), "checkout.session terminal event matched no pending order");
        }
    }
    Ok(())
}

async fn apply_partial_refund(
    state: &SharedState,
    tx: &mut Tx<'_>,
    object: &JsonValue,
    event_id: &str,
) -> Result<(), ApiError> {
    let Some(pi_id) = object.get("payment_intent").and_then(|v| v.as_str()) else {
        tracing::warn!(event_id = %event_id, "charge.refunded has no payment_intent; nothing to reverse");
        return Ok(());
    };
    let cumulative_refunded_cents = object
        .get("amount_refunded")
        .and_then(|v| v.as_i64())
        .ok_or_else(|| ApiError::bad_request("charge.refunded missing amount_refunded"))?;

    let outcome = state
        .credits
        .record_charge_refund_in_tx(tx, pi_id, cumulative_refunded_cents, event_id)
        .await?;
    log_reversal(&outcome, event_id, pi_id, "charge.refunded");
    Ok(())
}

async fn apply_full_reversal(
    state: &SharedState,
    tx: &mut Tx<'_>,
    object: &JsonValue,
    event_id: &str,
    status: &str,
) -> Result<(), ApiError> {
    let Some(pi_id) = object.get("payment_intent").and_then(|v| v.as_str()) else {
        tracing::warn!(event_id = %event_id, status = %status, "reversal event has no payment_intent; nothing to reverse");
        return Ok(());
    };

    let outcome = state
        .credits
        .record_full_reversal_in_tx(tx, pi_id, status, event_id)
        .await?;
    log_reversal(&outcome, event_id, pi_id, status);
    Ok(())
}

fn log_reversal(outcome: &ReversalOutcome, event_id: &str, pi_id: &str, kind: &str) {
    match outcome {
        ReversalOutcome::Reversed {
            address,
            charged_back,
            removed,
            shortfall,
            has_shortfall,
        } => {
            tracing::info!(event_id = %event_id, payment_intent = %pi_id, kind = %kind, address = %address, charged_back = %charged_back, removed = %removed, "fiat reversed: revoked the credits this purchase granted (atomic)");
            if *has_shortfall {
                tracing::error!(
                    event_id = %event_id,
                    payment_intent = %pi_id,
                    address = %address,
                    charged_back = %charged_back,
                    removed = %removed,
                    shortfall = %shortfall,
                    "REVERSAL SHORTFALL: buyer had already spent part of the reversed purchase's credits; that amount is an unrecovered loss"
                );
            }
        }
        ReversalOutcome::NothingToReverse => {
            tracing::info!(event_id = %event_id, payment_intent = %pi_id, kind = %kind, "reversal added no new reversed amount; no credits revoked");
        }
        ReversalOutcome::NoPaidPurchase => {
            tracing::warn!(event_id = %event_id, payment_intent = %pi_id, kind = %kind, "reversal matched no paid purchase; nothing revoked");
        }
    }
}
