use alloy_primitives::Address;
use axum::Json;
use axum::extract::State;
use axum::http::HeaderMap;
use serde::Deserialize;
use serde_json::{Value, json};

use crate::checkout::calls::{
    BrokerCall, build_reclaim, build_release, parse_address, parse_token_id,
};
use crate::checkout::claims::{EscrowActionClaim, claim_escrow_action};
use crate::evm::SharedState;
use crate::evm::errors::ApiError;
use crate::evm::handlers::admin::require_admin;
use crate::evm::handlers::{idempotency_key, require_broadcast_enabled};

#[derive(Debug, Clone, Copy)]
enum EscrowAction {
    Reclaim,
    Release,
}

impl EscrowAction {
    fn as_str(self) -> &'static str {
        match self {
            EscrowAction::Reclaim => "reclaim",
            EscrowAction::Release => "release",
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReclaimBody {
    pub collection: String,

    pub token_id: String,

    #[serde(default)]
    pub idempotency_key: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReleaseBody {
    pub collection: String,

    pub token_id: String,

    pub buyer: String,

    #[serde(default)]
    pub idempotency_key: Option<String>,
}

pub async fn reclaim(
    State(state): State<SharedState>,
    headers: HeaderMap,
    body: Result<Json<ReclaimBody>, axum::extract::rejection::JsonRejection>,
) -> Result<Json<Value>, ApiError> {
    require_admin(&state, &headers)?;
    let Json(body) = body.map_err(|e| ApiError::MalformedBody(e.body_text()))?;

    let collection = parse_address("collection", &body.collection)?;
    let token_id = parse_token_id(&body.token_id)?;
    let escrow = resolve_escrow(&state)?;
    let signer = resolve_signer(&state)?;
    let idem = idempotency_key(&headers, body.idempotency_key.as_deref());

    let call = build_reclaim(escrow, collection, token_id);
    let tx_hash = broadcast_action(
        &state,
        signer,
        EscrowAction::Reclaim,
        call,
        ActionTarget {
            escrow,
            collection_hex: &collection_hex(collection),
            token_id_text: &body.token_id,
            buyer_hex: None,
        },
        idem.as_deref(),
    )
    .await?;

    Ok(Json(
        json!({ "ok": true, "status": "sent", "txHash": tx_hash }),
    ))
}

pub async fn release(
    State(state): State<SharedState>,
    headers: HeaderMap,
    body: Result<Json<ReleaseBody>, axum::extract::rejection::JsonRejection>,
) -> Result<Json<Value>, ApiError> {
    require_admin(&state, &headers)?;
    let Json(body) = body.map_err(|e| ApiError::MalformedBody(e.body_text()))?;

    let collection = parse_address("collection", &body.collection)?;
    let token_id = parse_token_id(&body.token_id)?;
    let buyer = parse_address("buyer", &body.buyer)?;
    let escrow = resolve_escrow(&state)?;
    let signer = resolve_signer(&state)?;
    let idem = idempotency_key(&headers, body.idempotency_key.as_deref());

    let call = build_release(escrow, collection, token_id, buyer);
    let tx_hash = broadcast_action(
        &state,
        signer,
        EscrowAction::Release,
        call,
        ActionTarget {
            escrow,
            collection_hex: &collection_hex(collection),
            token_id_text: &body.token_id,
            buyer_hex: Some(&format!("{buyer:#x}")),
        },
        idem.as_deref(),
    )
    .await?;

    Ok(Json(
        json!({ "ok": true, "status": "sent", "txHash": tx_hash }),
    ))
}

fn collection_hex(collection: Address) -> String {
    format!("{collection:#x}")
}

fn resolve_escrow(state: &SharedState) -> Result<Address, ApiError> {
    let raw = state
        .settings
        .landiler_escrow_address
        .as_deref()
        .ok_or_else(|| {
            ApiError::RelayerUnavailable(
                "LANDILER_ESCROW_ADDRESS is not configured; escrow reclaim/release is unavailable."
                    .into(),
            )
        })?;
    parse_address("LANDILER_ESCROW_ADDRESS", raw).map_err(|_| {
        ApiError::Internal("LANDILER_ESCROW_ADDRESS is set but is not a valid address".into())
    })
}

fn resolve_signer(state: &SharedState) -> Result<&crate::evm::signer::DirectSigner, ApiError> {
    require_broadcast_enabled(state, "escrow actions")?;
    state.transaction.direct_signer().map(|s| s.as_ref()).ok_or_else(|| {
        ApiError::RelayerUnavailable(
            "Escrow actions require the direct JSON-RPC signer (set META_TX_BROADCAST_ENABLED=true with RELAYER_PRIVATE_KEY_FILE + RPC_ENDPOINT_POLYGON).".into(),
        )
    })
}

struct ActionTarget<'a> {
    escrow: Address,
    collection_hex: &'a str,
    token_id_text: &'a str,
    buyer_hex: Option<&'a str>,
}

async fn broadcast_action(
    state: &SharedState,
    signer: &crate::evm::signer::DirectSigner,
    action: EscrowAction,
    call: BrokerCall,
    target: ActionTarget<'_>,
    idem: Option<&str>,
) -> Result<String, ApiError> {
    let ActionTarget {
        escrow,
        collection_hex,
        token_id_text,
        buyer_hex,
    } = target;
    let escrow_hex = format!("{escrow:#x}");
    let chain_id = state.settings.collections_chain_id as i64;

    let tx_hash = match idem {
        Some(key) => {
            let claim = claim_escrow_action(
                &state.pool,
                &EscrowActionClaim {
                    key,
                    action: action.as_str(),
                    collection: collection_hex,
                    token_id: token_id_text,
                    buyer: buyer_hex,
                    escrow_address: &escrow_hex,
                    chain_id,
                },
                crate::evm::now(),
            )
            .await?;

            if !claim.claimed {
                match claim.status() {
                    "sent" => {
                        let tx_hash = claim.tx_hash.ok_or_else(|| {
                            ApiError::Internal(format!(
                                "escrow action for idempotencyKey {key:?} is 'sent' but has no recorded txHash"
                            ))
                        })?;
                        tracing::info!(idempotency_key = %key, tx_hash = %tx_hash, action = action.as_str(), "escrow action idempotent replay \u{2192} returning recorded txHash");
                        return Ok(tx_hash);
                    }

                    "error" => {
                        if !claim.rearmed {
                            return Err(ApiError::Conflict(format!(
                                "an escrow {} for idempotencyKey {key:?} is already in flight; not re-broadcasting \u{2014} retry after it settles",
                                action.as_str()
                            )));
                        }
                        tracing::info!(idempotency_key = %key, action = action.as_str(), "re-arming errored escrow action for re-broadcast (prior attempt failed pre-broadcast; no NFT moved)");
                    }

                    status => {
                        return Err(ApiError::Conflict(format!(
                            "an escrow {} for idempotencyKey {key:?} is in flight (status {status:?}); not re-broadcasting \u{2014} poll the recorded action or reconcile rather than retrying",
                            action.as_str()
                        )));
                    }
                }
            }

            match signer.send_direct_call(call.to, call.data).await {
                Ok(tx_hash) => {
                    sqlx::query(
                        "UPDATE shop.escrow_actions SET tx_hash = $2, status = 'sent', updated_at = timezone('UTC', now()) WHERE idempotency_key = $1",
                    )
                    .bind(key)
                    .bind(&tx_hash)
                    .execute(&state.pool)
                    .await?;
                    tx_hash
                }
                Err(e) => {
                    if let Err(db) = sqlx::query(
                        "UPDATE shop.escrow_actions SET status = 'error', updated_at = timezone('UTC', now()) WHERE idempotency_key = $1 AND status = 'pending'",
                    )
                    .bind(key)
                    .execute(&state.pool)
                    .await
                    {
                        tracing::error!(idempotency_key = %key, error = %db, "failed to mark escrow action 'error' after broadcast failure");
                    }
                    return Err(e);
                }
            }
        }
        None => {
            let tx_hash = signer.send_direct_call(call.to, call.data).await?;
            sqlx::query(
                "INSERT INTO shop.escrow_actions \
                 (tx_hash, action, collection, token_id, buyer, escrow_address, chain_id, status, created_at, updated_at) \
                 VALUES ($1, $2, $3, $4, $5, $6, $7, 'sent', $8, $8)",
            )
            .bind(&tx_hash)
            .bind(action.as_str())
            .bind(collection_hex)
            .bind(token_id_text)
            .bind(buyer_hex)
            .bind(&escrow_hex)
            .bind(chain_id)
            .bind(crate::evm::now())
            .execute(&state.pool)
            .await?;
            tx_hash
        }
    };

    tracing::info!(
        action = action.as_str(),
        tx_hash = %tx_hash,
        collection = %collection_hex,
        escrow = %escrow_hex,
        "escrow action broadcast + recorded"
    );
    Ok(tx_hash)
}
