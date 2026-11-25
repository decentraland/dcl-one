// from: credits-server@d9c6a261f:src/controllers/handlers/apple-iap-webhook.ts, iap-verify.ts, iap-register.ts, iap-quote.ts, mint-iap-credit.ts, refund-iap-credit.ts
//! The App Store routes (`credits::iap`):
//!
//! - `POST /apple/webhook` (`/v1/apple/webhook`): App Store Server
//!   Notifications V2. The signed payload is the authentication: a payload
//!   that does not verify answers 400; once verified, `ONE_TIME_CHARGE`
//!   credits the wallet registered for the transaction's account token and
//!   `REFUND` reverses it (`REFUND_REVERSED` undoes that; either one signed
//!   before the transaction's latest refund decision answers `superseded` and
//!   applies nothing). A purchase not
//!   credited is held (`credits::iap`). A notification whose `signedDate` is
//!   older than the accepted window (`credits::apple`) is refused with 200, so
//!   Apple stops sending it; one already applied (its `notificationUUID`
//!   recorded) answers 200 and applies nothing. Every other outcome answers
//!   200 except the ones Apple should retry: a `signedDate` ahead of the
//!   node's clock (503), an unknown product, a purchase over the node's daily
//!   ceiling, a concurrent delivery of the same notification (409) and a node
//!   without App Store configuration (503).
//! - `POST /credits/iap/verify` (`/v1/iap/verify`, signed fetch): the device
//!   posts StoreKit's `jwsRepresentation`; it credits the signer only when the
//!   transaction's account token is the signer's. Business outcomes answer
//!   200 with `ok: false` and a `code`.
//! - `POST /credits/iap/register` (`/v1/iap/register`, signed fetch): records
//!   the signer's account token.
//! - `POST /credits/iap/quote` (`/v1/iap/quote`, signed fetch): whether the
//!   signer may buy `productId` under the caps; an allowed quote records the
//!   account token.
//! - operator `POST /credits/iap` (`/admin/iap/credits`) and
//!   `POST /credits/iap/refund` (`/admin/iap/refunds`): the same credit and
//!   refund for support, behind the operator bearer; the credit is held
//!   to the node's daily ceiling, not the per-wallet caps, and the refund
//!   is refused (409) while Apple's latest decision is a reversal unless the
//!   body sets `overrideReversal`. Operator `POST /credits/iap/reversal`
//!   (`/admin/iap/reversals`) undoes a refund as `REFUND_REVERSED` does,
//!   given that notification's `signedDate`: it is refused (409) when Apple's
//!   latest decision is signed after it, and recorded as the latest decision
//!   otherwise; a purchase never credited becomes held, for `/admin/iap/credits`.
//!   An unknown transaction (404) or one with no refund to undo (409) is
//!   refused and records nothing.
//!   Operator `GET /credits/iap/held` (`/admin/iap/held`) lists the held purchases.

use axum::Json;
use axum::extract::rejection::JsonRejection;
use axum::extract::{OriginalUri, Query, State};
use axum::http::{HeaderMap, Uri};
use serde::Deserialize;
use serde_json::{Value as JsonValue, json};

use super::signer_from;
use crate::credits::SharedState;
use crate::credits::admin::{RequireAdmin, normalize_address, paginate};
use crate::credits::apple::{NOTIFICATION_MAX_FUTURE_SECS, Transaction, VerifyError, signed_ahead};
use crate::credits::errors::ApiError;
use crate::credits::iap::{
    CapReason, Channel, CreditOutcome, HoldReason, Iap, Notice, Purchase, RefundOutcome, Refunder,
    ReinstateOutcome, Reverser, UPSTREAM_REFUND_REF,
};
use crate::types::UsdCents;

fn configured(state: &SharedState) -> Result<&Iap, ApiError> {
    state.settings.iap.as_deref().ok_or_else(|| {
        ApiError::service_unavailable("App Store purchases are not configured on this node")
    })
}

fn now() -> i64 {
    chrono::Utc::now().timestamp()
}

#[derive(Debug, Deserialize)]
pub struct WebhookBody {
    #[serde(rename = "signedPayload")]
    signed_payload: Option<String>,
}

pub async fn webhook(
    State(state): State<SharedState>,
    body: Result<Json<WebhookBody>, JsonRejection>,
) -> Result<Json<JsonValue>, ApiError> {
    let iap = configured(&state)?;
    let Json(body) = body.map_err(|e| ApiError::bad_request(e.body_text()))?;
    let signed = body
        .signed_payload
        .filter(|s| !s.is_empty())
        .ok_or_else(|| ApiError::bad_request("missing signedPayload"))?;
    let notification = match iap.verifier.notification(&signed, now()) {
        Ok(n) => n,
        Err(e) => {
            let Some((notification_uuid, signed_date_ms)) = e.out_of_window() else {
                tracing::warn!(error = %e, "app store notification refused");
                return Err(ApiError::bad_request("invalid signedPayload"));
            };
            // Too old: refused for good, since a retry carries the same signedDate.
            if matches!(e, VerifyError::Expired { .. }) {
                tracing::warn!(
                    notification_uuid = %notification_uuid,
                    signed_date_ms,
                    "app store notification refused: signedDate older than the accepted window; apply it through /admin/iap/credits, /admin/iap/refunds or /admin/iap/reversals"
                );
                return Ok(Json(
                    json!({ "ok": true, "error": "signed_date_out_of_window" }),
                ));
            }
            // Ahead of this node's clock: answered for a retry. Apple's last retry
            // comes 157 hours after the first, so a clock that stays behind loses it.
            tracing::error!(
                notification_uuid = %notification_uuid,
                signed_date_ms,
                "app store notification refused: signedDate ahead of this node's clock; answered for a retry, check the node's clock"
            );
            return Err(ApiError::service_unavailable("signed_date_ahead"));
        }
    };
    let uuid = notification.notification_uuid.as_str();
    let notice = Some(Notice {
        uuid,
        signed_date_ms: notification.signed_date_ms,
    });
    if state.credits.iap_notification_applied(uuid).await? {
        tracing::info!(notification_uuid = %uuid, "app store notification already applied");
        return Ok(Json(json!({ "ok": true, "duplicate": true })));
    }
    let Some(t) = notification.transaction else {
        tracing::info!(notification_uuid = %uuid, kind = %notification.notification_type, "app store notification without a transaction");
        return Ok(Json(json!({ "ok": true, "skipped": true })));
    };
    match notification.notification_type.as_str() {
        "ONE_TIME_CHARGE" => charge(&state, iap, notice, &t).await,
        "REFUND" => {
            let reason = format!(
                "apple_refund:{}",
                notification.subtype.as_deref().unwrap_or("unspecified")
            );
            match state
                .credits
                .refund_iap(
                    &t.transaction_id,
                    Some(&t.product_id),
                    uuid,
                    &reason,
                    t.revocation_percentage,
                    Refunder::Apple(Notice {
                        uuid,
                        signed_date_ms: notification.signed_date_ms,
                    }),
                )
                .await?
            {
                RefundOutcome::Reversed {
                    removed,
                    revocations,
                    ..
                } => Ok(Json(json!({
                    "ok": true, "action": "refunded", "creditId": t.transaction_id,
                    "removed": removed, "revocations": revocations,
                }))),
                RefundOutcome::AlreadyRefunded => {
                    Ok(Json(json!({ "ok": true, "action": "already_refunded" })))
                }
                // Signed before the transaction's latest refund or reversal.
                RefundOutcome::Superseded => {
                    Ok(Json(json!({ "ok": true, "action": "superseded" })))
                }
                RefundOutcome::NotCredited | RefundOutcome::HeldRefunded => {
                    tracing::warn!(notification_uuid = %uuid, transaction_id = %t.transaction_id, "app store refund of a transaction never credited: recorded so it is never credited");
                    Ok(Json(json!({ "ok": true, "action": "unknown_credit" })))
                }
            }
        }
        "REFUND_REVERSED" => match state
            .credits
            .reinstate_iap(
                &t.transaction_id,
                Reverser::Apple(Notice {
                    uuid,
                    signed_date_ms: notification.signed_date_ms,
                }),
            )
            .await?
        {
            ReinstateOutcome::Reinstated {
                credits, withdrawn, ..
            } => Ok(Json(json!({
                "ok": true, "action": "reinstated", "creditId": t.transaction_id, "credits": credits,
                "revocationsWithdrawn": withdrawn,
            }))),
            // Idempotent per transaction: a retry after a failed credit credits it.
            ReinstateOutcome::Uncredited => charge(&state, iap, notice, &t).await,
            // `Unknown` answers only an operator reversal.
            ReinstateOutcome::NotRefunded | ReinstateOutcome::Unknown => {
                Ok(Json(json!({ "ok": true, "action": "not_refunded" })))
            }
            // Signed before the transaction's latest refund or reversal.
            ReinstateOutcome::Superseded => Ok(Json(json!({ "ok": true, "action": "superseded" }))),
        },
        "TEST" => Ok(Json(json!({ "ok": true, "test": true }))),
        other => {
            tracing::info!(notification_uuid = %uuid, kind = %other, "app store notification type not acted on");
            Ok(Json(json!({ "ok": true, "ignored": true })))
        }
    }
}

/// Credits a notification's purchase to the wallet its account token names.
async fn charge(
    state: &SharedState,
    iap: &Iap,
    notice: Option<Notice<'_>>,
    t: &Transaction,
) -> Result<Json<JsonValue>, ApiError> {
    let uuid = notice.map_or("", |n| n.uuid);
    let address = state
        .credits
        .wallet_for_iap_token(t.app_account_token.as_deref())
        .await?;
    let outcome = state
        .credits
        .credit_iap(
            iap,
            Purchase {
                address: address.as_deref(),
                app_account_token: t.app_account_token.as_deref(),
                transaction_id: &t.transaction_id,
                original_transaction_id: t.original_transaction_id.as_deref(),
                product_id: &t.product_id,
                quantity: Some(t.quantity),
                revoked: t.revoked,
                channel: Channel::Notification,
                notice,
                signed_date_ms: t.signed_date_ms,
            },
        )
        .await?;
    match outcome {
        CreditOutcome::Credited { already, .. } => Ok(Json(
            json!({ "ok": true, "action": "minted", "creditId": t.transaction_id, "alreadyExisted": already }),
        )),
        CreditOutcome::Revoked => Ok(Json(json!({ "ok": true, "error": "revoked_transaction" }))),
        CreditOutcome::Refunded => Ok(Json(json!({ "ok": true, "error": "refunded_transaction" }))),
        CreditOutcome::Held(HoldReason::UnknownProduct) => {
            tracing::error!(notification_uuid = %uuid, transaction_id = %t.transaction_id, product_id = %t.product_id, "app store purchase of a product outside IAP_PRODUCT_CATALOG_USD: held, answered for a later retry");
            Err(ApiError::service_unavailable("unknown_product"))
        }
        CreditOutcome::Held(HoldReason::Capped(CapReason::ServiceDailyLimit)) => {
            tracing::error!(notification_uuid = %uuid, transaction_id = %t.transaction_id, "app store purchase over IAP_DAILY_LIMIT_CENTS: held, answered for a later retry");
            Err(ApiError::service_unavailable("service_daily_limit"))
        }
        CreditOutcome::Held(HoldReason::Capped(reason)) => {
            tracing::error!(notification_uuid = %uuid, transaction_id = %t.transaction_id, reason = reason.as_str(), "app store purchase over a wallet cap: held for an operator");
            Ok(Json(
                json!({ "ok": true, "error": "wallet_cap_exceeded", "reason": reason.as_str(), "held": true }),
            ))
        }
        CreditOutcome::Held(reason) => {
            tracing::error!(notification_uuid = %uuid, transaction_id = %t.transaction_id, reason = reason.as_str(), "app store purchase not credited: held for an operator");
            Ok(Json(
                json!({ "ok": true, "error": reason.as_str(), "held": true }),
            ))
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct VerifyBody {
    #[serde(rename = "jwsRepresentation")]
    jws_representation: Option<String>,
}

fn refused(code: &str) -> Json<JsonValue> {
    Json(json!({ "ok": false, "code": code }))
}

pub async fn verify(
    State(state): State<SharedState>,
    original: OriginalUri,
    uri: Uri,
    headers: HeaderMap,
    body: Result<Json<VerifyBody>, JsonRejection>,
) -> Result<Json<JsonValue>, ApiError> {
    let iap = configured(&state)?;
    let signer = signer_from(&state, &headers, "post", uri.path(), &original).await?;
    let wallet = signer.as_str().to_string();
    let Json(body) = body.map_err(|e| ApiError::bad_request(e.body_text()))?;
    let jws = body
        .jws_representation
        .filter(|s| !s.is_empty())
        .ok_or_else(|| ApiError::bad_request("jwsRepresentation is required"))?;
    let t = match iap.verifier.transaction(&jws, now()) {
        Ok(t) => t,
        Err(e) => {
            tracing::warn!(wallet = %wallet, error = %e, "app store transaction refused");
            return Ok(refused("invalid_jws"));
        }
    };
    // The transaction must have been bought for the signer's account.
    let token = t
        .app_account_token
        .as_deref()
        .unwrap_or_default()
        .to_ascii_lowercase();
    if token != crate::credits::apple::account_token(&wallet) && token != wallet {
        tracing::warn!(wallet = %wallet, transaction_id = %t.transaction_id, "app store transaction bought for another account");
        return Ok(refused("token_mismatch"));
    }
    let outcome = state
        .credits
        .credit_iap(
            iap,
            Purchase {
                address: Some(&wallet),
                app_account_token: t.app_account_token.as_deref(),
                transaction_id: &t.transaction_id,
                original_transaction_id: t.original_transaction_id.as_deref(),
                product_id: &t.product_id,
                quantity: Some(t.quantity),
                revoked: t.revoked,
                channel: Channel::Device,
                notice: None,
                signed_date_ms: t.signed_date_ms,
            },
        )
        .await?;
    Ok(match outcome {
        CreditOutcome::Credited { already, .. } => Json(json!({
            "ok": true,
            "data": { "credited": true, "creditId": t.transaction_id, "alreadyExisted": already },
        })),
        CreditOutcome::Revoked => refused("revoked_transaction"),
        CreditOutcome::Refunded => refused("refunded_transaction"),
        CreditOutcome::Held(HoldReason::Capped(CapReason::ServiceDailyLimit)) => {
            refused("service_daily_limit")
        }
        CreditOutcome::Held(HoldReason::Capped(reason)) => {
            Json(json!({ "ok": false, "code": "cap_exceeded", "reason": reason.as_str() }))
        }
        CreditOutcome::Held(reason) => {
            tracing::error!(wallet = %wallet, transaction_id = %t.transaction_id, product_id = %t.product_id, reason = reason.as_str(), "app store purchase not credited: held for an operator");
            refused(reason.as_str())
        }
    })
}

pub async fn register(
    State(state): State<SharedState>,
    original: OriginalUri,
    uri: Uri,
    headers: HeaderMap,
) -> Result<Json<JsonValue>, ApiError> {
    configured(&state)?;
    let signer = signer_from(&state, &headers, "post", uri.path(), &original).await?;
    let token = state.credits.link_iap_token(signer.as_str()).await?;
    Ok(Json(
        json!({ "ok": true, "data": { "appAccountToken": token } }),
    ))
}

#[derive(Debug, Deserialize)]
pub struct QuoteBody {
    #[serde(rename = "productId")]
    product_id: Option<String>,
}

pub async fn quote(
    State(state): State<SharedState>,
    original: OriginalUri,
    uri: Uri,
    headers: HeaderMap,
    body: Result<Json<QuoteBody>, JsonRejection>,
) -> Result<Json<JsonValue>, ApiError> {
    let iap = configured(&state)?;
    let signer = signer_from(&state, &headers, "post", uri.path(), &original).await?;
    let wallet = signer.as_str();
    let Json(body) = body.map_err(|e| ApiError::bad_request(e.body_text()))?;
    let product_id = body
        .product_id
        .filter(|s| !s.is_empty())
        .ok_or_else(|| ApiError::bad_request("productId is required"))?;
    let Some(credits) = iap.credits_per_unit(&product_id) else {
        return Ok(Json(
            json!({ "ok": false, "code": "unknown_product", "data": { "productId": product_id } }),
        ));
    };
    let cents = UsdCents::from_credits(credits).map_or(i64::MAX, UsdCents::get);
    let totals = crate::credits::store::Credits::iap_totals(&state.credits.pool, wallet).await?;
    let refusal = totals.decide(&iap.limits, cents);
    let token = match refusal {
        None => Some(state.credits.link_iap_token(wallet).await?),
        Some(_) => None,
    };
    let remaining = |cap: i64, used: i64| (cap > 0).then(|| (cap - used).max(0) as f64 / 100.0);
    // Upstream's USD-rail shape: `denomination` names the rail, `amount` is whole Credits, `balance` dollars.
    let data = json!({
        "productId": product_id,
        "denomination": "USD",
        "amount": credits,
        "credits": credits,
        "appAccountToken": token,
        "balance": totals.total_cents as f64 / 100.0,
        "totalRemaining": remaining(iap.limits.wallet_total_cents, totals.total_cents),
        "dailyRemaining": remaining(iap.limits.wallet_daily_cents, totals.daily_cents),
    });
    Ok(Json(match refusal {
        None => json!({ "ok": true, "data": data }),
        Some(r) if r.as_str() == "service_daily_limit" => {
            json!({ "ok": false, "code": "service_daily_limit", "data": data })
        }
        Some(r) => {
            json!({ "ok": false, "code": "cap_exceeded", "reason": r.as_str(), "data": data })
        }
    }))
}

#[derive(Debug, Deserialize)]
pub struct AdminCreditBody {
    #[serde(rename = "userAddress")]
    user_address: String,
    #[serde(rename = "appleTransactionId")]
    apple_transaction_id: String,
    #[serde(rename = "appleOriginalTransactionId")]
    apple_original_transaction_id: Option<String>,
    #[serde(rename = "appleProductId")]
    apple_product_id: String,
    /// Omitted: a held purchase's own quantity, else 1.
    quantity: Option<i64>,
}

fn validate_id(name: &str, v: &str) -> Result<(), ApiError> {
    if v.is_empty() || v.len() > 128 {
        return Err(ApiError::bad_request(format!(
            "{name} must be 1 to 128 characters"
        )));
    }
    Ok(())
}

/// Operator credit of one App Store transaction, by its product (never an amount).
pub async fn admin_credit(
    admin: RequireAdmin,
    State(state): State<SharedState>,
    body: Result<Json<AdminCreditBody>, JsonRejection>,
) -> Result<Json<JsonValue>, ApiError> {
    let iap = configured(&state)?;
    let Json(b) = body.map_err(|e| ApiError::bad_request(e.body_text()))?;
    let address = normalize_address(&b.user_address)?;
    validate_id("appleTransactionId", &b.apple_transaction_id)?;
    validate_id("appleProductId", &b.apple_product_id)?;
    let actor = admin.audit_actor_description();
    let outcome = state
        .credits
        .credit_iap(
            iap,
            Purchase {
                address: Some(&address),
                app_account_token: None,
                transaction_id: &b.apple_transaction_id,
                original_transaction_id: b.apple_original_transaction_id.as_deref(),
                product_id: &b.apple_product_id,
                quantity: b.quantity,
                revoked: false,
                channel: Channel::Operator(&actor),
                notice: None,
                signed_date_ms: None,
            },
        )
        .await?;
    match outcome {
        CreditOutcome::Credited {
            address: credited,
            credits,
            already,
        } => {
            if credited != address {
                return Err(ApiError::conflict(
                    "this transaction already credited another wallet",
                ));
            }
            Ok(Json(json!({
                "creditId": b.apple_transaction_id, "appleTransactionId": b.apple_transaction_id,
                "credits": credits, "alreadyExisted": already,
            })))
        }
        CreditOutcome::Refunded => Err(ApiError::conflict("this transaction was refunded")),
        CreditOutcome::Revoked => Err(ApiError::conflict("this transaction was revoked")),
        CreditOutcome::Held(HoldReason::Capped(r)) => Err(ApiError::unprocessable(format!(
            "over a cap: {}",
            r.as_str()
        ))),
        CreditOutcome::Held(_) => Err(ApiError::bad_request("unknown appleProductId or quantity")),
    }
}

#[derive(Debug, Deserialize)]
pub struct AdminRefundBody {
    #[serde(rename = "appleTransactionId")]
    apple_transaction_id: String,
    #[serde(rename = "refundId")]
    refund_id: String,
    reason: String,
    /// Refund even though Apple's latest decision reversed a refund.
    #[serde(rename = "overrideReversal", default)]
    override_reversal: bool,
}

pub async fn admin_refund(
    admin: RequireAdmin,
    State(state): State<SharedState>,
    body: Result<Json<AdminRefundBody>, JsonRejection>,
) -> Result<Json<JsonValue>, ApiError> {
    configured(&state)?;
    let Json(b) = body.map_err(|e| ApiError::bad_request(e.body_text()))?;
    validate_id("appleTransactionId", &b.apple_transaction_id)?;
    validate_id("refundId", &b.refund_id)?;
    // The import's marker of an upstream purchase; as a refund id no reversal could undo it.
    if b.refund_id == UPSTREAM_REFUND_REF {
        return Err(ApiError::bad_request(format!(
            "refundId {UPSTREAM_REFUND_REF} is reserved"
        )));
    }
    if b.reason.is_empty() || b.reason.len() > 200 {
        return Err(ApiError::bad_request("reason must be 1 to 200 characters"));
    }
    let actor = admin.audit_actor_description();
    match state
        .credits
        .refund_iap(
            &b.apple_transaction_id,
            None,
            &b.refund_id,
            &b.reason,
            None,
            Refunder::Operator {
                actor: &actor,
                override_reversal: b.override_reversal,
            },
        )
        .await?
    {
        RefundOutcome::Reversed {
            removed,
            shortfall,
            revocations,
            ..
        } => Ok(Json(json!({
            "creditId": b.apple_transaction_id, "appleTransactionId": b.apple_transaction_id,
            "refundId": b.refund_id, "removed": removed, "shortfall": shortfall, "revocations": revocations,
        }))),
        RefundOutcome::AlreadyRefunded => {
            Err(ApiError::conflict("this transaction was already refunded"))
        }
        RefundOutcome::Superseded => Err(ApiError::conflict(
            "Apple reversed this transaction's refund; send overrideReversal: true to refund it anyway",
        )),
        RefundOutcome::HeldRefunded => Ok(Json(json!({
            "creditId": b.apple_transaction_id, "appleTransactionId": b.apple_transaction_id,
            "refundId": b.refund_id, "removed": "0", "shortfall": "0", "revocations": 0, "held": true,
        }))),
        RefundOutcome::NotCredited => Err(ApiError::not_found(
            "no credited App Store transaction with this id",
        )),
    }
}

#[derive(Debug, Deserialize)]
pub struct AdminReversalBody {
    #[serde(rename = "appleTransactionId")]
    apple_transaction_id: String,
    /// Names this reversal.
    #[serde(rename = "reversalId")]
    reversal_id: String,
    /// The `REFUND_REVERSED` notification's `signedDate`, in milliseconds.
    #[serde(rename = "signedDate")]
    signed_date: i64,
}

/// Operator reversal of a refund (a `REFUND_REVERSED` too old to deliver),
/// ordered by Apple's `signedDate` against the transaction's latest decision.
pub async fn admin_reversal(
    admin: RequireAdmin,
    State(state): State<SharedState>,
    body: Result<Json<AdminReversalBody>, JsonRejection>,
) -> Result<Json<JsonValue>, ApiError> {
    configured(&state)?;
    let Json(b) = body.map_err(|e| ApiError::bad_request(e.body_text()))?;
    validate_id("appleTransactionId", &b.apple_transaction_id)?;
    validate_id("reversalId", &b.reversal_id)?;
    // A future date would make every later Apple decision stale.
    if signed_ahead(b.signed_date, now().saturating_mul(1000)) {
        return Err(ApiError::bad_request(format!(
            "signedDate is more than {} minutes ahead of this node's clock",
            NOTIFICATION_MAX_FUTURE_SECS / 60
        )));
    }
    let uuid = format!("operator:{}", b.reversal_id);
    let actor = admin.audit_actor_description();
    let by = Reverser::Operator {
        notice: Notice {
            uuid: &uuid,
            signed_date_ms: b.signed_date,
        },
        actor: &actor,
    };
    let ids = json!({ "creditId": b.apple_transaction_id, "appleTransactionId": b.apple_transaction_id, "reversalId": b.reversal_id });
    match state
        .credits
        .reinstate_iap(&b.apple_transaction_id, by)
        .await?
    {
        ReinstateOutcome::Reinstated {
            credits, withdrawn, ..
        } => Ok(Json(json!({
            "creditId": ids["creditId"], "appleTransactionId": ids["appleTransactionId"],
            "reversalId": ids["reversalId"], "credits": credits, "revocationsWithdrawn": withdrawn,
        }))),
        // Held now: credit it through /admin/iap/credits.
        ReinstateOutcome::Uncredited => Ok(Json(json!({
            "creditId": ids["creditId"], "appleTransactionId": ids["appleTransactionId"],
            "reversalId": ids["reversalId"], "held": true,
        }))),
        ReinstateOutcome::NotRefunded => Err(ApiError::conflict(
            "no refund of this transaction to reverse",
        )),
        ReinstateOutcome::Unknown => {
            Err(ApiError::not_found("no App Store transaction with this id"))
        }
        ReinstateOutcome::Superseded => Err(ApiError::conflict(
            "Apple's latest refund decision for this transaction is signed after this reversal",
        )),
    }
}

#[derive(Debug, Deserialize)]
pub struct HeldQuery {
    limit: Option<i64>,
    offset: Option<i64>,
}

/// Operator list of the paid purchases not credited yet, oldest first.
pub async fn admin_held(
    _admin: RequireAdmin,
    State(state): State<SharedState>,
    Query(q): Query<HeldQuery>,
) -> Result<Json<JsonValue>, ApiError> {
    configured(&state)?;
    let (limit, offset) = paginate(q.limit, q.offset);
    let held = state.credits.held_iap(limit, offset).await?;
    let held: Vec<JsonValue> = held
        .into_iter()
        .map(|h| {
            json!({
                "appleTransactionId": h.transaction_id,
                "appleProductId": h.product_id,
                "quantity": h.quantity,
                "userAddress": h.address,
                "appAccountToken": h.app_account_token,
                "reason": h.reason,
                "createdAt": h.created_at.and_utc().to_rfc3339(),
            })
        })
        .collect();
    Ok(Json(json!({ "held": held })))
}
