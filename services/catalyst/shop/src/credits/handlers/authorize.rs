// from: credits-server@465e628bc:src/controllers/handlers/{authorize-usd-credit,authorize-usd-credit-batch,cancel-usd-intents,record-intent-submission,get-credits-name-route}.ts, catalyrst/crates/catalyrst-credits/src/handlers/authorize.rs (the signed-fetch binding, the idempotency key)
//! The credit-authorization routes, at `/credits/credits/authorize*` and
//! `/shop-api/v1/credit-authorizations*`:
//!
//! - `POST /credits/authorize`: one line, one credit (`batch::authorize_group`
//!   with one line, the single-route messages);
//! - `POST /credits/authorize/batch`: one credit for a whole settlement
//!   group, its lines echoed in request order;
//! - `POST /credits/authorize/cancel`: retire the signer's pledges by salt;
//!   the answer is upstream's `{released, retired, spendableAtSeconds}`, the
//!   money rejoins the balance only when the CreditUsed index has read the
//!   chain past the credit's expiry;
//! - `POST /credits/authorize/submitted`: stamp the transaction the credit
//!   was submitted in (`{recorded}`);
//! - `GET /credits/usage-index/status`: the index's cursor and readiness;
//! - `GET /credits/credits-name-route`: NAME registration with credits stays
//!   unavailable (503 naming the missing executor; docs/shop/CONTRACTS.md).
//!
//! The idempotency key of an authorization is derived from the signer, the
//! lines and the signed-fetch timestamp, so a retried request replays the
//! same credit (and the same signature) instead of pledging twice.

use axum::Json;
use axum::extract::{OriginalUri, State};
use axum::http::HeaderMap;
use axum::response::{IntoResponse, Response};
use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};

use super::signer_from;
use crate::credits::SharedState;
use crate::credits::batch::{
    self, AuthorizeError, NAME_EXECUTOR_CONFIGURE, NAME_EXECUTOR_SOURCE, ValidatedLine,
};
use crate::credits::digest::{bytes32_hex, parse_bytes32};
use crate::credits::errors::ApiError;
use crate::credits::history::releases_at_seconds;
use crate::credits::usage;
use crate::shared::signed_fetch::AUTH_TIMESTAMP_HEADER;

pub use crate::credits::batch::{AuthorizeCreditOut, AuthorizedCreditOut, LineOut};
pub use crate::credits::digest::TreasuryWallet;

const MAX_SALTS_PER_REQUEST: usize = 100;

pub(crate) fn derive_idempotency_key(
    signer: &str,
    lines: &[ValidatedLine],
    auth_ts: &str,
) -> String {
    let mut h = Sha256::new();
    h.update(signer.to_ascii_lowercase().as_bytes());
    for line in lines {
        h.update([0x1f]);
        for field in [
            Some(line.quoted_cents.to_string()),
            line.trade_id.clone(),
            line.contract_address.clone(),
            line.item_id.clone(),
            line.registered_name.clone(),
        ] {
            h.update(field.unwrap_or_default().as_bytes());
            h.update([0x1e]);
        }
    }
    h.update([0x1f]);
    h.update(auth_ts.as_bytes());
    format!("authz:{}", hex::encode(h.finalize()))
}

fn body_json(body: &[u8]) -> Value {
    serde_json::from_slice(body).unwrap_or(Value::Object(Default::default()))
}

fn refuse(e: AuthorizeError, single: bool) -> Response {
    let (status, body) = e.wire(single);
    let mut response = (
        axum::http::StatusCode::from_u16(status)
            .unwrap_or(axum::http::StatusCode::INTERNAL_SERVER_ERROR),
        Json(body),
    )
        .into_response();
    if status == 503 {
        response.headers_mut().insert(
            axum::http::header::RETRY_AFTER,
            axum::http::HeaderValue::from_static("60"),
        );
    }
    response
}

async fn run(
    state: &SharedState,
    headers: &HeaderMap,
    signer: &str,
    body: Value,
    single: bool,
) -> Response {
    let source = match batch::parse_source(body.get("source")) {
        Ok(source) => source,
        Err(message) => return refuse(AuthorizeError::Line { index: 0, message }, true),
    };
    let raw_lines: Vec<Value> = if single {
        vec![body.clone()]
    } else {
        match body.get("items").and_then(Value::as_array) {
            Some(items) if !items.is_empty() => items.clone(),
            _ => {
                return refuse(
                    AuthorizeError::Line {
                        index: 0,
                        message: "A non-empty \"items\" array is required".to_string(),
                    },
                    true,
                );
            }
        }
    };
    if raw_lines.len() > batch::MAX_LINES_PER_GROUP {
        return refuse(
            AuthorizeError::Line {
                index: 0,
                message: format!(
                    "A checkout may hold at most {} items",
                    batch::MAX_LINES_PER_GROUP
                ),
            },
            true,
        );
    }
    let mut lines = Vec::with_capacity(raw_lines.len());
    for (index, raw) in raw_lines.iter().enumerate() {
        match batch::validate_line(raw) {
            Ok(line) => lines.push(line),
            Err(message) => return refuse(AuthorizeError::Line { index, message }, single),
        }
    }
    let auth_ts = headers
        .get(AUTH_TIMESTAMP_HEADER)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    let key = derive_idempotency_key(signer, &lines, auth_ts);
    match batch::authorize_group(state, signer, &lines, source, &key, single).await {
        Ok(out) => Json(out).into_response(),
        Err(e) => refuse(e, single),
    }
}

pub async fn authorize(
    State(state): State<SharedState>,
    original: OriginalUri,
    headers: HeaderMap,
    body: axum::body::Bytes,
) -> Response {
    let signer = match signer_from(&state, &headers, "post", "/credits/authorize", &original).await
    {
        Ok(signer) => signer,
        Err(e) => return e.into_response(),
    };
    run(&state, &headers, signer.as_str(), body_json(&body), true).await
}

pub async fn authorize_batch(
    State(state): State<SharedState>,
    original: OriginalUri,
    headers: HeaderMap,
    body: axum::body::Bytes,
) -> Response {
    let signer = match signer_from(
        &state,
        &headers,
        "post",
        "/credits/authorize/batch",
        &original,
    )
    .await
    {
        Ok(signer) => signer,
        Err(e) => return e.into_response(),
    };
    run(&state, &headers, signer.as_str(), body_json(&body), false).await
}

/// Upstream's salt filter: a bounded array, the bytes32 entries kept, the
/// rest dropped; nothing valid is a 400.
fn salts_of(body: &Value) -> Result<Vec<String>, ApiError> {
    let raw = body
        .get("salts")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    if raw.is_empty() || raw.len() > MAX_SALTS_PER_REQUEST {
        return Err(ApiError::bad_request(format!(
            "A \"salts\" array with 1 to {MAX_SALTS_PER_REQUEST} entries is required"
        )));
    }
    let salts: Vec<String> = raw
        .iter()
        .filter_map(Value::as_str)
        .filter_map(parse_bytes32)
        .map(bytes32_hex)
        .collect();
    if salts.is_empty() {
        return Err(ApiError::bad_request("No valid credit ids in \"salts\""));
    }
    Ok(salts)
}

#[derive(Debug, Serialize)]
pub struct CancelOut {
    pub released: i64,
    pub retired: i64,
    #[serde(rename = "spendableAtSeconds")]
    pub spendable_at_seconds: Option<i64>,
}

pub async fn cancel(
    State(state): State<SharedState>,
    original: OriginalUri,
    headers: HeaderMap,
    body: axum::body::Bytes,
) -> Result<Json<CancelOut>, ApiError> {
    let signer = signer_from(
        &state,
        &headers,
        "post",
        "/credits/authorize/cancel",
        &original,
    )
    .await?;
    let salts = salts_of(&body_json(&body))?;
    let retired = state
        .credits
        .retire_intents(&salts, signer.as_str())
        .await?;
    tracing::info!(address = %signer.as_str(), submitted = salts.len(), retired = retired.retired, "retired reserved credits");
    Ok(Json(CancelOut {
        released: retired.retired,
        retired: retired.retired,
        spendable_at_seconds: retired.latest_expiry.map(releases_at_seconds),
    }))
}

#[derive(Debug, Serialize)]
pub struct SubmittedOut {
    pub recorded: i64,
}

pub async fn submitted(
    State(state): State<SharedState>,
    original: OriginalUri,
    headers: HeaderMap,
    body: axum::body::Bytes,
) -> Result<Json<SubmittedOut>, ApiError> {
    let signer = signer_from(
        &state,
        &headers,
        "post",
        "/credits/authorize/submitted",
        &original,
    )
    .await?;
    let body = body_json(&body);
    let tx_hash = body
        .get("txHash")
        .and_then(Value::as_str)
        .and_then(parse_bytes32)
        .map(bytes32_hex)
        .ok_or_else(|| ApiError::bad_request("A \"txHash\" bytes32 hex string is required"))?;
    let salts = salts_of(&body)?;
    let recorded = state
        .credits
        .record_submission(&salts, signer.as_str(), &tx_hash)
        .await?;
    tracing::info!(address = %signer.as_str(), tx_hash = %tx_hash, submitted = salts.len(), recorded, "recorded a credit submission");
    Ok(Json(SubmittedOut { recorded }))
}

/// `GET /credits/credits-name-route`: no executor builds a NAME registration
/// on chain yet, so the credits route is unavailable and says so.
pub async fn name_credit_route(
    State(state): State<SharedState>,
    original: OriginalUri,
    headers: HeaderMap,
) -> Result<Json<Value>, ApiError> {
    signer_from(&state, &headers, "get", "/credits-name-route", &original).await?;
    Err(ApiError::source_unavailable(
        NAME_EXECUTOR_SOURCE,
        NAME_EXECUTOR_CONFIGURE,
        "NAME registration with credits is unavailable: no executor builds the registration on chain (docs/shop/CONTRACTS.md, \"NAME credits\"); register the NAME with MANA",
    ))
}

/// `GET /credits/usage-index/status`: the cursor row, readiness and what the
/// lane needs; public like the land-authz status (no wallet data).
pub async fn usage_index_status(State(state): State<SharedState>) -> Result<Json<Value>, ApiError> {
    let settings = &state.usage.settings;
    let now = chrono::Utc::now();
    let row = usage::cursor(&state.credits.pool, settings.chain_id).await?;
    let ready = state.usage.ready_at(now).await.is_some();
    let ts = |t: Option<chrono::NaiveDateTime>| {
        t.map(|t| {
            t.and_utc()
                .to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
        })
    };
    Ok(Json(serde_json::json!({
        "source": usage::SOURCE,
        "configured": settings.is_configured(),
        "enabled": settings.enabled,
        "authorizeEnabled": state.settings.credits_onchain_authorize_enabled,
        "chainId": settings.chain_id,
        "contract": settings.contract,
        "startBlock": settings.start_block,
        "ready": ready,
        "ttlSeconds": state.settings.credit_ttl_seconds,
        "capHeadroomPct": state.settings.credit_cap_headroom_pct,
        "cursor": row.map(|c| serde_json::json!({
            "contract": c.contract,
            "lastBlock": c.last_block,
            "foldedBlock": c.folded_block,
            "foldedBlockTime": ts(c.folded_block_time),
            "foldedAt": ts(c.folded_at),
            "fullReplayStarted": c.full_replay_started,
            "updatedAt": ts(Some(c.updated_at)),
        })),
    })))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn line(cents: i64, trade: Option<&str>) -> ValidatedLine {
        ValidatedLine {
            quoted_cents: cents,
            charged_cents: cents,
            trade_id: trade.map(str::to_string),
            contract_address: None,
            item_id: None,
            registered_name: None,
        }
    }

    #[test]
    fn wire_identity_authorize_credit() {
        let out = AuthorizeCreditOut {
            credit: AuthorizedCreditOut {
                id: "0xabc".into(),
                amount: "1000000000000000000".into(),
                available_amount: "1000000000000000000".into(),
                expires_at: 1_690_000_600,
                signature: "0xsig".into(),
                contract: "0xcontract".into(),
            },
            max_credited_value: "1000000000000000000".into(),
            usd_cents: 500,
            oracle_rate: "420000000000000000".into(),
            lines: None,
        };
        let v = serde_json::to_value(&out).unwrap();
        assert_eq!(
            v,
            json!({
                "credit": {
                    "id": "0xabc",
                    "amount": "1000000000000000000",
                    "availableAmount": "1000000000000000000",
                    "expiresAt": 1_690_000_600i64,
                    "signature": "0xsig",
                    "contract": "0xcontract",
                },
                "maxCreditedValue": "1000000000000000000",
                "usdCents": 500,
                "oracleRate": "420000000000000000",
            })
        );
        let mut keys: Vec<&String> = v.as_object().unwrap().keys().collect();
        keys.sort();
        assert_eq!(
            keys,
            ["credit", "maxCreditedValue", "oracleRate", "usdCents"]
        );
        let batch = AuthorizeCreditOut {
            lines: Some(vec![LineOut {
                usd_cents: 110,
                trade_id: None,
                contract_address: Some("0xc".into()),
                item_id: Some("2".into()),
                name: None,
            }]),
            ..out
        };
        let v = serde_json::to_value(&batch).unwrap();
        assert_eq!(
            v["lines"],
            json!([{ "usdCents": 110, "tradeId": null, "contractAddress": "0xc", "itemId": "2", "name": null }])
        );
    }

    #[test]
    fn wire_identity_cancel_and_submitted() {
        assert_eq!(
            serde_json::to_value(CancelOut {
                released: 2,
                retired: 2,
                spendable_at_seconds: Some(1_800_000_000)
            })
            .unwrap(),
            json!({ "released": 2, "retired": 2, "spendableAtSeconds": 1_800_000_000i64 })
        );
        assert_eq!(
            serde_json::to_value(CancelOut {
                released: 0,
                retired: 0,
                spendable_at_seconds: None
            })
            .unwrap(),
            json!({ "released": 0, "retired": 0, "spendableAtSeconds": null })
        );
        assert_eq!(
            serde_json::to_value(SubmittedOut { recorded: 2 }).unwrap(),
            json!({ "recorded": 2 })
        );
    }

    #[test]
    fn salts_are_filtered_and_bounded_like_upstream() {
        let ok = salts_of(&json!({ "salts": ["0xA1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1", 42, "0xb2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2", null, "0xnope"] })).unwrap();
        assert_eq!(
            ok,
            [
                "0xa1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1",
                "0xb2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2"
            ]
        );
        let err = |v: Value| salts_of(&v).unwrap_err().to_string();
        assert_eq!(
            err(json!({})),
            "A \"salts\" array with 1 to 100 entries is required"
        );
        assert_eq!(
            err(json!({ "salts": [] })),
            "A \"salts\" array with 1 to 100 entries is required"
        );
        assert_eq!(
            err(json!({ "salts": vec!["0x<64 hex>"; 101] })),
            "A \"salts\" array with 1 to 100 entries is required"
        );
        assert_eq!(
            err(json!({ "salts": ["0xsalt1", "nope", "0x1234"] })),
            "No valid credit ids in \"salts\""
        );
        assert_eq!(
            err(json!({ "salts": [1, 2, null] })),
            "No valid credit ids in \"salts\""
        );
    }

    #[test]
    fn idempotency_key_is_deterministic_and_input_sensitive() {
        let base = derive_idempotency_key("0xABC", &[line(500, Some("t1"))], "1690000000000");
        assert_eq!(
            base,
            derive_idempotency_key("0xabc", &[line(500, Some("t1"))], "1690000000000"),
            "signer case is normalized"
        );
        assert!(base.starts_with("authz:"));
        assert_ne!(
            base,
            derive_idempotency_key("0xABC", &[line(501, Some("t1"))], "1690000000000")
        );
        assert_ne!(
            base,
            derive_idempotency_key("0xABC", &[line(500, Some("t2"))], "1690000000000")
        );
        assert_ne!(
            base,
            derive_idempotency_key(
                "0xABC",
                &[line(500, Some("t1")), line(1, None)],
                "1690000000000"
            )
        );
        assert_ne!(
            base,
            derive_idempotency_key("0xABC", &[line(500, Some("t1"))], "1690000000001")
        );
    }
}
