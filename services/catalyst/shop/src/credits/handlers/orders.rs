// from: catalyrst/crates/catalyrst-credits/src/handlers/orders.rs, credits-server src/controllers/handlers/create-checkout-session.ts, credits-server src/logic/checkout-return-url.ts
//! The explorer's order flow: `POST /credits/checkout` opens a Stripe
//! Checkout Session for `quantity` (1..=5, default 1) of one pack (the
//! idempotency key derived from the signer, the pack, the quantity and the
//! signed-fetch timestamp) and records the pending order with its totals and
//! the surface that opened it (`source`), which also picks where Stripe
//! returns the buyer (the builder's pages for a builder checkout);
//! `GET /credits/orders/{id}` answers the client's status vocabulary.

use axum::Json;
use axum::extract::{OriginalUri, Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};

use super::{random_32_hex, signer_from};
use crate::credits::SharedState;
use crate::credits::errors::ApiError;
use crate::credits::packs::{MAX_PACK_QUANTITY, PackRow, parse_marketplace_source};
use crate::shared::signed_fetch::AUTH_TIMESTAMP_HEADER;

#[derive(Debug, Deserialize)]
pub struct CheckoutRequestBody {
    #[serde(rename = "packId")]
    pack_id: String,
    #[serde(default)]
    quantity: Option<serde_json::Value>,
    #[serde(default)]
    source: Option<serde_json::Value>,
}

/// The number of packs. Unlike `source` this moves money (it multiplies the
/// charge and the grant), so a malformed value is refused, never read as one:
/// absent or null is 1, anything but a whole number in `1..=max` is a 400.
fn parse_pack_quantity(value: Option<&serde_json::Value>, max: u32) -> Result<u32, ApiError> {
    let refuse = || {
        ApiError::bad_request(format!(
            "quantity must be a whole number between 1 and {max}"
        ))
    };
    let Some(value) = value.filter(|v| !v.is_null()) else {
        return Ok(1);
    };
    let whole = match value.as_u64() {
        Some(n) => n,
        None => match value.as_f64() {
            Some(f) if f.fract() == 0.0 && f >= 1.0 && f <= f64::from(max) => f as u64,
            _ => return Err(refuse()),
        },
    };
    if (1..=u64::from(max)).contains(&whole) {
        Ok(whole as u32)
    } else {
        Err(refuse())
    }
}

#[derive(Debug, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "credits/"))]
pub struct CheckoutSessionOut {
    #[serde(rename = "orderId")]
    pub order_id: String,
    pub url: String,
}

#[derive(Debug, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "credits/"))]
pub struct CreditsOrderStatusOut {
    pub status: String,
    #[serde(rename = "creditsGranted")]
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub credits_granted: i64,
    #[serde(rename = "newBalance")]
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub new_balance: i64,
    pub error: String,
}

fn checkout_idempotency_key(
    signer: &str,
    pack: &PackRow,
    quantity: u32,
    return_to: Option<&str>,
    auth_ts: &str,
) -> String {
    let mut h = Sha256::new();
    let quantity = quantity.to_string();
    let quantity = if quantity == "1" {
        None
    } else {
        Some(quantity.as_str())
    };
    for field in [
        signer,
        pack.sku.as_str(),
        pack.credits.as_str(),
        &pack.price_cents.to_string(),
        pack.currency.as_str(),
        auth_ts,
    ]
    .into_iter()
    .chain(quantity)
    .chain(return_to)
    {
        h.update(field.as_bytes());
        h.update([0u8]);
    }
    format!("landiler-checkout-{}", hex::encode(h.finalize()))
}

pub async fn create_checkout(
    State(state): State<SharedState>,
    original: OriginalUri,
    headers: HeaderMap,
    body: axum::body::Bytes,
) -> Result<Json<CheckoutSessionOut>, ApiError> {
    let signer = signer_from(&state, &headers, "post", "/credits/checkout", &original).await?;
    let body: CheckoutRequestBody =
        serde_json::from_slice(&body).map_err(|_| ApiError::bad_request("invalid request body"))?;
    let auth_ts = headers
        .get(AUTH_TIMESTAMP_HEADER)
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default();

    let Some(stripe) = state.stripe.as_ref() else {
        return Err(ApiError::service_unavailable(
            "card purchases are unavailable (payments provider not configured)",
        ));
    };
    let source = parse_marketplace_source(body.source.as_ref());
    let (success_url, cancel_url) = state.settings.checkout_return_urls(source);
    if success_url.is_empty() || cancel_url.is_empty() {
        return Err(ApiError::service_unavailable(
            "card purchases are unavailable (checkout redirect URLs not configured)",
        ));
    }
    let returns_to_builder = success_url != state.settings.checkout_success_url;

    let pack = state
        .credits
        .get_pack(&body.pack_id)
        .await?
        .ok_or_else(|| ApiError::bad_request("unknown or inactive packId"))?;
    let quantity = parse_pack_quantity(body.quantity.as_ref(), MAX_PACK_QUANTITY)?;
    let order = pack
        .order(quantity)
        .ok_or_else(|| ApiError::bad_request("the order total is out of range"))?
        .from_source(source);

    tracing::debug!(source = source.unwrap_or("unknown"), sku = %pack.sku, quantity, "credits checkout requested");

    let order_id = format!("ord_{}", random_32_hex());
    let idempotency_key = checkout_idempotency_key(
        signer.as_str(),
        &pack,
        quantity,
        returns_to_builder.then_some("builder"),
        auth_ts,
    );

    let session = stripe
        .create_checkout_session(
            &order,
            signer.as_str(),
            &order_id,
            success_url,
            cancel_url,
            &idempotency_key,
        )
        .await?;

    state
        .credits
        .insert_pending_order(&order_id, signer.as_str(), &order, &session.id)
        .await?;

    Ok(Json(CheckoutSessionOut {
        order_id,
        url: session.url,
    }))
}

fn wire_status(db_status: &str) -> &'static str {
    match db_status {
        "pending" => "processing",
        "paid" | "refunded" | "disputed" => "credited",
        "failed" => "failed",
        "abandoned" => "abandoned",
        _ => "processing",
    }
}

pub async fn order_status(
    State(state): State<SharedState>,
    Path(order_id): Path<String>,
    original: OriginalUri,
    headers: HeaderMap,
) -> Result<Json<CreditsOrderStatusOut>, ApiError> {
    let path = format!("/credits/orders/{}", order_id);
    let signer = signer_from(&state, &headers, "get", &path, &original).await?;

    let (order, available) = state
        .credits
        .get_order_with_balance(&order_id, signer.as_str())
        .await?
        .ok_or_else(|| ApiError::not_found("order not found"))?;

    let status = wire_status(&order.status);
    let credits_granted = if status == "credited" {
        crate::credits::money::whole_credits(&order.credits)
    } else {
        0
    };
    let new_balance = available;
    let error = if status == "failed" {
        order.failure_reason.clone().unwrap_or_default()
    } else {
        String::new()
    };

    Ok(Json(CreditsOrderStatusOut {
        status: status.to_string(),
        credits_granted,
        new_balance,
        error,
    }))
}

async fn order_status_out(
    state: &SharedState,
    order_id: &str,
    address: &str,
) -> Result<CreditsOrderStatusOut, ApiError> {
    let (order, available) = state
        .credits
        .get_order_with_balance(order_id, address)
        .await?
        .ok_or_else(|| ApiError::not_found("order not found"))?;
    let status = wire_status(&order.status);
    Ok(CreditsOrderStatusOut {
        status: status.to_string(),
        credits_granted: if status == "credited" {
            crate::credits::money::whole_credits(&order.credits)
        } else {
            0
        },
        new_balance: available,
        error: if status == "failed" {
            order.failure_reason.clone().unwrap_or_default()
        } else {
            String::new()
        },
    })
}

/// `POST /credits/orders/{id}/cancel`: the buyer walked away from the hosted
/// page. The session is expired at Stripe first (outside any transaction),
/// then the order retired by one guarded UPDATE; a paid order is never
/// touched and answers its status. Idempotent: a second click finds nothing
/// pending and answers the same.
pub async fn cancel_order(
    State(state): State<SharedState>,
    Path(order_id): Path<String>,
    original: OriginalUri,
    headers: HeaderMap,
) -> Result<Json<CreditsOrderStatusOut>, ApiError> {
    let path = format!("/credits/orders/{}/cancel", order_id);
    let signer = signer_from(&state, &headers, "post", &path, &original).await?;
    let (order, session) = state
        .credits
        .get_order_with_session(&order_id, signer.as_str())
        .await?
        .ok_or_else(|| ApiError::not_found("order not found"))?;
    if order.status == "pending" {
        if let (Some(stripe), Some(session)) = (state.stripe.as_ref(), session.as_deref()) {
            let key = format!("expire:{session}");
            match stripe.expire_checkout_session(session, &key).await? {
                Some(s) if s.is_paid() => {
                    tracing::info!(order_id = %order_id, "cancel found a paid session; leaving the order to the webhook");
                    return Ok(Json(
                        order_status_out(&state, &order_id, signer.as_str()).await?,
                    ));
                }
                _ => {}
            }
        }
        state
            .credits
            .mark_order_status_by_session(
                session.as_deref().unwrap_or(""),
                "abandoned",
                Some("cancelled by the buyer"),
            )
            .await?;
    }
    Ok(Json(
        order_status_out(&state, &order_id, signer.as_str()).await?,
    ))
}

#[derive(Debug, Serialize)]
pub struct ResumeOut {
    #[serde(rename = "orderId")]
    pub order_id: String,
    pub url: String,
}

/// `POST /credits/orders/{id}/resume`: the same hosted page while Stripe
/// still has the session open (200 `{orderId, url}`); a paid or no longer
/// payable order answers 409 with its status, retiring an expired one so the
/// click cleans up after itself.
pub async fn resume_order(
    State(state): State<SharedState>,
    Path(order_id): Path<String>,
    original: OriginalUri,
    headers: HeaderMap,
) -> Response {
    let path = format!("/credits/orders/{}/resume", order_id);
    let signer = match signer_from(&state, &headers, "post", &path, &original).await {
        Ok(signer) => signer,
        Err(e) => return e.into_response(),
    };
    match resume(&state, &order_id, signer.as_str()).await {
        Ok(crate::credits::orders::Resume::Url(url)) => Json(ResumeOut { order_id, url }).into_response(),
        Ok(crate::credits::orders::Resume::Paid(status)) | Ok(crate::credits::orders::Resume::NoLongerPayable(status)) => {
            (StatusCode::CONFLICT, Json(json!({ "ok": false, "error": "order is no longer payable", "message": "order is no longer payable", "status": status }))).into_response()
        }
        Err(e) => e.into_response(),
    }
}

async fn resume(
    state: &SharedState,
    order_id: &str,
    address: &str,
) -> Result<crate::credits::orders::Resume, ApiError> {
    use crate::credits::orders::Resume;
    let (order, session) = state
        .credits
        .get_order_with_session(order_id, address)
        .await?
        .ok_or_else(|| ApiError::not_found("order not found"))?;
    if order.status != "pending" {
        let status = wire_status(&order.status);
        return Ok(if status == "credited" {
            Resume::Paid(status)
        } else {
            Resume::NoLongerPayable(status)
        });
    }
    let (Some(stripe), Some(session)) = (state.stripe.as_ref(), session.as_deref()) else {
        return Err(ApiError::service_unavailable(
            "card purchases are unavailable (payments provider not configured)",
        ));
    };
    match stripe.retrieve_checkout_session(session).await? {
        Some(s) if s.is_payable() => Ok(Resume::Url(s.url.unwrap_or_default())),
        Some(s) if s.is_paid() => Ok(Resume::Paid("processing")),
        _ => {
            state
                .credits
                .mark_order_status_by_session(
                    session,
                    "abandoned",
                    Some("checkout session expired"),
                )
                .await?;
            Ok(Resume::NoLongerPayable("abandoned"))
        }
    }
}

/// `GET /credits/orders/{id}/cancel-return`: Stripe's `cancel_url` landing,
/// unauthenticated (a browser redirect), so nothing is taken on the
/// client's word: the session is read from Stripe and the order retired
/// only when Stripe says it is not paid and no longer open; then the buyer
/// is sent to the Shop's cancel page.
pub async fn cancel_return(
    State(state): State<SharedState>,
    Path(order_id): Path<String>,
) -> Response {
    let cancel_url = state.settings.checkout_cancel_url.clone();
    if let Some(stripe) = state.stripe.as_ref()
        && let Ok(Some((_, session))) = state.credits.get_order_with_session_any(&order_id).await
        && let Some(session) = session.as_deref()
        && let Ok(Some(s)) = stripe.retrieve_checkout_session(session).await
        && !s.is_paid()
        && !s.is_payable()
    {
        let _ = state
            .credits
            .mark_order_status_by_session(
                session,
                "abandoned",
                Some("cancelled at the hosted page"),
            )
            .await;
    }
    if cancel_url.is_empty() {
        return (
            StatusCode::OK,
            Json(json!({ "ok": true, "orderId": order_id })),
        )
            .into_response();
    }
    axum::response::Redirect::to(&cancel_url).into_response()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn wire_identity_checkout_session() {
        let out = CheckoutSessionOut {
            order_id: "ord_abc".into(),
            url: "https://checkout.example/session/cs_test".into(),
        };
        assert_eq!(
            serde_json::to_value(&out).unwrap(),
            json!({ "orderId": "ord_abc", "url": "https://checkout.example/session/cs_test" })
        );
    }

    #[test]
    fn wire_identity_order_status() {
        let out = CreditsOrderStatusOut {
            status: "credited".into(),
            credits_granted: 100,
            new_balance: 250,
            error: String::new(),
        };
        assert_eq!(
            serde_json::to_value(&out).unwrap(),
            json!({ "status": "credited", "creditsGranted": 100, "newBalance": 250, "error": "" })
        );
    }

    #[test]
    fn db_status_maps_to_the_lowercase_unity_vocabulary() {
        assert_eq!(wire_status("pending"), "processing");
        assert_eq!(wire_status("paid"), "credited");
        assert_eq!(wire_status("refunded"), "credited");
        assert_eq!(wire_status("disputed"), "credited");
        assert_eq!(wire_status("failed"), "failed");
        assert_eq!(wire_status("abandoned"), "abandoned");
    }

    #[test]
    fn checkout_key_is_deterministic_and_distinct_from_intent() {
        let pack = PackRow {
            sku: "pack_100".into(),
            title: "100 Credits".into(),
            credits: "100".into(),
            price_cents: 999,
            currency: "usd".into(),
            sort_order: 0,
        };
        let k = checkout_idempotency_key("0xabc", &pack, 1, None, "1690000000000");
        assert_eq!(
            k,
            checkout_idempotency_key("0xabc", &pack, 1, None, "1690000000000")
        );
        assert!(k.starts_with("landiler-checkout-"));
        assert_ne!(
            k,
            checkout_idempotency_key("0xabc", &pack, 2, None, "1690000000000"),
            "another quantity is another session"
        );
        assert_ne!(
            k,
            checkout_idempotency_key("0xabc", &pack, 1, Some("builder"), "1690000000000"),
            "another return page is another session"
        );
    }

    #[test]
    fn a_builder_checkout_returns_to_the_builder_only_where_one_is_configured() {
        use crate::credits::{RawSettings, Settings};
        let shop_only = Settings::parse(RawSettings {
            checkout_success_url: "https://shop/ok".into(),
            checkout_cancel_url: "https://shop/no".into(),
            checkout_success_url_builder: Some("https://builder/ok".into()),
            ..RawSettings::default()
        })
        .unwrap();
        assert_eq!(
            shop_only.checkout_return_urls(Some("builder")),
            ("https://shop/ok", "https://shop/no"),
            "half a pair is no pair"
        );
        let both = Settings::parse(RawSettings {
            checkout_success_url: "https://shop/ok".into(),
            checkout_cancel_url: "https://shop/no".into(),
            checkout_success_url_builder: Some("https://builder/ok".into()),
            checkout_cancel_url_builder: Some(" https://builder/no ".into()),
            ..RawSettings::default()
        })
        .unwrap();
        assert_eq!(
            both.checkout_return_urls(Some("builder")),
            ("https://builder/ok", "https://builder/no")
        );
        for other in [Some("website"), Some("client"), None] {
            assert_eq!(
                both.checkout_return_urls(other),
                ("https://shop/ok", "https://shop/no"),
                "{other:?}"
            );
        }
    }

    #[test]
    fn the_pack_quantity_defaults_to_one_and_refuses_anything_malformed() {
        let q = |v: serde_json::Value| parse_pack_quantity(Some(&v), MAX_PACK_QUANTITY);
        assert_eq!(parse_pack_quantity(None, MAX_PACK_QUANTITY).unwrap(), 1);
        assert_eq!(q(json!(null)).unwrap(), 1);
        assert_eq!(q(json!(1)).unwrap(), 1);
        assert_eq!(q(json!(5)).unwrap(), 5);
        assert_eq!(
            q(json!(2.0)).unwrap(),
            2,
            "a whole number written as a float is that number"
        );
        for bad in [
            json!(0),
            json!(6),
            json!(-1),
            json!(2.5),
            json!("2"),
            json!(true),
            json!([2]),
            json!(1e300),
        ] {
            let err = q(bad.clone()).expect_err(&format!("{bad}"));
            assert!(
                format!("{err:?}").contains("between 1 and 5"),
                "{bad}: {err:?}"
            );
        }
    }
}
