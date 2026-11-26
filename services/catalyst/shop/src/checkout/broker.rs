use alloy_primitives::{Address, U256};
use axum::Json;
use axum::extract::State;
use axum::http::HeaderMap;
use chrono::NaiveDateTime;
use serde::Deserialize;
use serde_json::{Value, json};

use crate::checkout::calls::{
    BrokerCall, PurchaseMode, build_forward_to_escrow, build_mana_approve, build_primary,
    build_secondary, minted_token_id_from_logs, parse_address, parse_item_id, parse_token_id,
    parse_wei, parse_wei_allow_zero,
};
use crate::checkout::claims::{
    BROKER_PENDING_TAKEOVER_SECS, BrokerPurchaseClaim, checkout_frozen, claim_broker_purchase,
};
use crate::checkout::trade::{
    TradeDelivery, TradeExpectations, TradeIn, ValidatedTrade, build_trade_accept,
    transferred_token_id_from_logs, validate_trade,
};
use crate::evm::SharedState;
use crate::evm::contracts_addrs::DclContracts;
use crate::evm::errors::ApiError;
use crate::evm::handlers::admin::require_admin;
use crate::evm::handlers::{idempotency_key, require_broadcast_enabled};
use crate::evm::oracle::{UsdPeggedExpectation, fetch_mana_usd_rate};
use crate::evm::signer::{DirectSigner, Rebroadcast, ReceiptOutcome, SignedTx};
use crate::types::ASSET_TYPE_USD_PEGGED_MANA;
use crate::types::chain_id_column;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BuyBody {
    pub collection: String,

    #[serde(default)]
    pub item_id: Option<String>,

    #[serde(default)]
    pub token_id: Option<String>,

    pub price_wei: String,

    pub escrow_address: String,

    pub mode: String,

    #[serde(default)]
    pub buyer_address: Option<String>,

    #[serde(default)]
    pub trade: Option<serde_json::Value>,

    #[serde(default)]
    pub quote_mana_wei: Option<String>,

    #[serde(default)]
    pub idempotency_key: Option<String>,

    /// The credits debit this buy settles, in US cents (the fulfilment
    /// outbox sends it; absent, the buy is a credit-funded one all the same).
    #[serde(default)]
    pub charge_cents: Option<i64>,
}

struct BuyOutcome {
    forward_tx_hash: String,
}

pub async fn buy(
    State(state): State<SharedState>,
    headers: HeaderMap,
    body: Result<Json<BuyBody>, axum::extract::rejection::JsonRejection>,
) -> Result<Json<Value>, ApiError> {
    require_admin(&state, &headers)?;
    let Json(body) = body.map_err(|e| ApiError::MalformedBody(e.body_text()))?;

    let mode = PurchaseMode::parse_wearable(body.mode.trim())?;
    let collection = parse_address("collection", &body.collection)?;
    let escrow = parse_address("escrowAddress", &body.escrow_address)?;
    let price_wei = match mode {
        PurchaseMode::Primary => parse_wei_allow_zero("priceWei", &body.price_wei)?,
        _ => parse_wei("priceWei", &body.price_wei)?,
    };
    if mode != PurchaseMode::Trade && body.trade.is_some() {
        return Err(ApiError::InvalidTransaction(
            "a `trade` payload was sent but mode is not \"trade\"".into(),
        ));
    }
    if mode != PurchaseMode::Trade && body.quote_mana_wei.is_some() {
        return Err(ApiError::InvalidTransaction(
            "`quoteManaWei` was sent but mode is not \"trade\" (it is the listing-time MANA \
             quote for a USD-pegged trade)"
                .into(),
        ));
    }

    let buyer = match body.buyer_address.as_deref() {
        Some(b) => parse_address("buyerAddress", b)?,
        None => {
            return Err(ApiError::InvalidTransaction(
                "buyerAddress is required: the buyer is ABI-encoded into the escrow's onERC721Received _data so the 15-day lease can be recorded".into(),
            ))
        }
    };

    if let Some(pinned_raw) = state.settings.landiler_escrow_address.as_deref() {
        let pinned = parse_address("LANDILER_ESCROW_ADDRESS", pinned_raw).map_err(|_| {
            ApiError::Internal("LANDILER_ESCROW_ADDRESS is set but is not a valid address".into())
        })?;
        if escrow != pinned {
            return Err(ApiError::Forbidden(format!(
                "escrowAddress {escrow:#x} is not the configured LANDILER_ESCROW_ADDRESS; broker buys may only deliver to the pinned escrow"
            )));
        }
    }

    let chain_id = state.settings.collections_chain_id;
    let contracts = DclContracts::for_chain(chain_id).ok_or_else(|| {
        ApiError::InvalidTransaction(format!(
            "no Decentraland contracts known for chain {chain_id}"
        ))
    })?;

    require_broadcast_enabled(&state, "broker buys")?;

    let Some(signer) = state.transaction.direct_signer() else {
        return Err(ApiError::RelayerUnavailable(
            "Broker buy requires the direct JSON-RPC signer (set META_TX_BROADCAST_ENABLED=true with RELAYER_PRIVATE_KEY_FILE + RPC_ENDPOINT_POLYGON). Validation passed; broadcast is unavailable.".into(),
        ));
    };
    let signer: &DirectSigner = signer.as_ref();
    let relayer = signer.relayer_address();

    let validated_trade: Option<ValidatedTrade> = match mode {
        PurchaseMode::Trade => {
            let raw = body.trade.as_ref().ok_or_else(|| {
                ApiError::InvalidTransaction(
                    "mode \"trade\" requires the full `trade` payload (id, signer, signature, \
                     checks, sent, received)"
                        .into(),
                )
            })?;
            let parsed: TradeIn = serde_json::from_value(raw.clone()).map_err(|e| {
                ApiError::InvalidTransaction(format!("malformed trade payload: {e}"))
            })?;
            let is_usd_pegged = parsed.received.len() == 1
                && parsed.received[0].asset_type == i64::from(ASSET_TYPE_USD_PEGGED_MANA);
            if !is_usd_pegged && body.quote_mana_wei.is_some() {
                return Err(ApiError::InvalidTransaction(
                    "`quoteManaWei` was sent but the trade is not USD-pegged (assetType 2); \
                     for MANA trades `priceWei` already pins the exact charge"
                        .into(),
                ));
            }
            let usd_pegged = if is_usd_pegged {
                let rate = fetch_mana_usd_rate(signer, state.settings.mana_usd_aggregator).await?;
                let quote_mana_wei = match body.quote_mana_wei.as_deref() {
                    Some(raw) => Some(parse_wei("quoteManaWei", raw)?),
                    None => None,
                };
                Some(UsdPeggedExpectation {
                    rate,
                    max_age_secs: state.settings.usd_pegged_oracle_max_age_secs,
                    slippage_bps: state.settings.usd_pegged_slippage_bps,
                    quote_mana_wei,
                })
            } else {
                None
            };
            let now_ms = crate::evm::now().and_utc().timestamp_millis();
            Some(validate_trade(
                &parsed,
                &TradeExpectations {
                    collection,
                    price_wei,
                    chain_id,
                    relayer,
                    now_ms,
                    usd_pegged,
                },
            )?)
        }
        _ => None,
    };

    let token_id_for_secondary: Option<U256> = match mode {
        PurchaseMode::Primary => {
            if body.item_id.as_deref().is_none() {
                return Err(ApiError::InvalidTransaction(
                    "primary buy requires `itemId`".into(),
                ));
            }
            None
        }
        PurchaseMode::Trade => match validated_trade.as_ref().map(|v| v.delivery) {
            Some(TradeDelivery::Nft { token_id }) => Some(token_id),
            _ => None,
        },
        _ => {
            let token_id_raw = body.token_id.as_deref().ok_or_else(|| {
                ApiError::InvalidTransaction("secondary buy requires `tokenId`".into())
            })?;
            Some(parse_token_id(token_id_raw)?)
        }
    };

    let item_id_for_primary = match (mode, body.item_id.as_deref()) {
        (PurchaseMode::Primary, Some(raw)) => Some(parse_item_id(raw)?),
        _ => None,
    };
    crate::guards::broker::guard_buy(
        &state.pool,
        crate::guards::broker::BuyFacts {
            chain_id,
            buyer,
            collection,
            mode,
            item_id: item_id_for_primary,
            token_id: token_id_for_secondary,
            price_wei,
            trade: validated_trade.as_ref(),
            charge_cents: body.charge_cents,
        },
    )
    .await?;

    let buy_call: BrokerCall = match mode {
        PurchaseMode::Primary => {
            let item_id = item_id_for_primary.ok_or_else(|| {
                ApiError::InvalidTransaction("primary buy requires `itemId`".into())
            })?;
            build_primary(&contracts, collection, item_id, price_wei, relayer)
        }
        PurchaseMode::Trade => build_trade_accept(
            validated_trade
                .as_ref()
                .expect("trade mode always sets validated_trade or returns early"),
            relayer,
        ),
        _ => build_secondary(
            &contracts,
            collection,
            token_id_for_secondary.expect("secondary mode always parses tokenId or returns early"),
            price_wei,
        ),
    };

    let Some(key) = idempotency_key(&headers, body.idempotency_key.as_deref()) else {
        return Err(ApiError::InvalidTransaction(
            "Idempotency-Key is required for broker buys (funds safety: it prevents a retry from re-buying)".into(),
        ));
    };

    let collection_hex = format!("{collection:#x}");
    let escrow_hex = format!("{escrow:#x}");
    let buyer_hex = format!("{buyer:#x}");
    let charge_basis_wei = validated_trade
        .as_ref()
        .map(|v| v.price_wei)
        .unwrap_or(price_wei);
    let price_text = charge_basis_wei.to_string();
    let usd_charge = validated_trade.as_ref().and_then(|v| v.usd_pegged);
    let usd_amount_text = usd_charge.map(|u| u.usd_wei.to_string());
    let usd_rate_text = usd_charge.map(|u| u.rate_18.to_string());
    if let Some(u) = &usd_charge {
        tracing::info!(
            usd_amount_wei = %u.usd_wei,
            mana_usd_rate_wei = %u.rate_18,
            rate_updated_at_s = u.rate_updated_at_s,
            charge_basis_mana_wei = %u.mana_wei,
            "USD-pegged trade: charge basis is the signed USD amount converted at the \
             execution-time oracle rate"
        );
    }
    let token_id_text = token_id_for_secondary.map(|t| t.to_string());
    let item_id_text: Option<String> = match validated_trade.as_ref().map(|v| v.delivery) {
        Some(TradeDelivery::Item { item_id }) => Some(item_id.to_string()),
        _ => body.item_id.clone(),
    };
    let trade_sig_hash = validated_trade
        .as_ref()
        .map(|v| format!("{:#x}", v.hashed_signature));

    let claimed_at = crate::evm::now();
    let claim = claim_broker_purchase(
        &state.pool,
        &BrokerPurchaseClaim {
            key: &key,
            collection: &collection_hex,
            item_id: item_id_text.as_deref(),
            token_id: token_id_text.as_deref(),
            buyer_address: &buyer_hex,
            escrow_address: &escrow_hex,
            price_wei: &price_text,
            chain_id: chain_id_column(chain_id)?,
            mode: mode.as_str(),
            trade_hashed_signature: trade_sig_hash.as_deref(),
            usd_amount_wei: usd_amount_text.as_deref(),
            mana_usd_rate_wei: usd_rate_text.as_deref(),
        },
        claimed_at,
    )
    .await
    .map_err(|e| match &e {
        sqlx::Error::Database(db) if db.constraint() == Some("broker_purchases_trade_sig_uidx") => {
            ApiError::InvalidTransaction(
                "this signed trade is already being (or has been) purchased under another \
                 request; a 1-use trade listing cannot be sold twice"
                    .into(),
            )
        }
        _ => ApiError::from(e),
    })?;

    let ctx = BuyContext {
        state: &state,
        signer,
        key: &key,
        mode,
        collection,
        escrow,
        buyer,
        relayer,
        trade: validated_trade.as_ref(),
    };
    let outcome = if claim.claimed || claim.rearmed {
        drive_buy_then_forward(ctx, buy_call, token_id_for_secondary, claimed_at).await?
    } else {
        resume_existing(ctx).await?
    };

    tracing::info!(
        idempotency_key = %key,
        forward_tx_hash = %outcome.forward_tx_hash,
        mode = mode.as_str(),
        collection = %collection_hex,
        escrow = %escrow_hex,
        "broker buy confirmed in escrow custody"
    );

    let mut resp = json!({
        "ok": true,
        "txHash": outcome.forward_tx_hash,
        "mode": mode.as_str(),
        "chargeBasisWei": price_text,
    });
    if let Some(u) = &usd_charge {
        resp["usdPegged"] = json!({
            "usdAmountWei": u.usd_wei.to_string(),
            "manaUsdRateWei": u.rate_18.to_string(),
            "rateUpdatedAt": u.rate_updated_at_s,
        });
    }
    Ok(Json(resp))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApproveCollectionsManaBody {
    pub spender: String,
    #[serde(default)]
    pub amount_wei: Option<String>,
}

pub(crate) fn collections_spender(
    contracts: &DclContracts,
    raw: &str,
) -> Result<Address, ApiError> {
    match raw.trim() {
        "collection-store" => Ok(contracts.collection_store),
        "marketplace-v2" => Ok(contracts.marketplace_v2),
        "offchain-marketplace" => Ok(contracts.offchain_marketplace),
        other => Err(ApiError::InvalidTransaction(format!(
            "invalid spender {other:?}: expected \"collection-store\", \"marketplace-v2\", or \
             \"offchain-marketplace\""
        ))),
    }
}

pub async fn approve_mana_collections(
    State(state): State<SharedState>,
    headers: HeaderMap,
    body: Result<Json<ApproveCollectionsManaBody>, axum::extract::rejection::JsonRejection>,
) -> Result<Json<Value>, ApiError> {
    require_admin(&state, &headers)?;
    let Json(body) = body.map_err(|e| ApiError::MalformedBody(e.body_text()))?;

    let chain_id = state.settings.collections_chain_id;
    let contracts = DclContracts::for_chain(chain_id).ok_or_else(|| {
        ApiError::InvalidTransaction(format!(
            "no Decentraland contracts known for chain {chain_id}"
        ))
    })?;
    let spender = collections_spender(&contracts, &body.spender)?;
    let amount = match body.amount_wei.as_deref() {
        Some(raw) => parse_wei("amountWei", raw)?,
        None => U256::MAX,
    };

    require_broadcast_enabled(&state, "MANA approvals")?;
    let Some(signer) = state.transaction.direct_signer() else {
        return Err(ApiError::RelayerUnavailable(
            "MANA approve requires the direct JSON-RPC signer (set META_TX_BROADCAST_ENABLED=true with RELAYER_PRIVATE_KEY_FILE + RPC_ENDPOINT_POLYGON); nothing was broadcast.".into(),
        ));
    };

    let call = build_mana_approve(contracts.mana_token, spender, amount);
    let tx = signer.send_direct_call(call.to, call.data).await?;

    match signer.await_receipt(&tx).await? {
        ReceiptOutcome::Confirmed => {
            tracing::info!(
                tx_hash = %tx,
                spender = %spender,
                chain_id,
                "MANA approve confirmed on collections chain"
            );
            Ok(Json(json!({ "ok": true, "txHash": tx })))
        }
        ReceiptOutcome::Reverted => Err(ApiError::RelayReverted(format!(
            "MANA approve tx {tx} reverted on-chain"
        ))),
        ReceiptOutcome::Pending => Err(ApiError::RelayerTimeout(format!(
            "MANA approve tx {tx} not yet mined; check before retrying"
        ))),
    }
}

#[derive(Clone, Copy)]
struct BuyContext<'a> {
    state: &'a SharedState,
    signer: &'a DirectSigner,
    key: &'a str,
    mode: PurchaseMode,
    collection: Address,
    escrow: Address,
    buyer: Address,
    relayer: Address,
    trade: Option<&'a ValidatedTrade>,
}

/// Buys under the claim made at `claimed_at`: the buy is recorded ('sent',
/// its hash and signed bytes) before it is broadcast, then confirmed and
/// forwarded to escrow.
async fn drive_buy_then_forward(
    ctx: BuyContext<'_>,
    buy_call: BrokerCall,
    token_id_secondary: Option<U256>,
    claimed_at: NaiveDateTime,
) -> Result<BuyOutcome, ApiError> {
    let BuyContext {
        state, signer, key, ..
    } = ctx;
    let buy_tx = send_recorded_buy(state, signer, key, claimed_at, buy_call).await?;

    let token_id = confirm_buy(ctx, &buy_tx, token_id_secondary).await?;
    drive_forward(ctx, token_id).await
}

async fn confirm_buy(
    ctx: BuyContext<'_>,
    buy_tx: &str,
    token_id_secondary: Option<U256>,
) -> Result<U256, ApiError> {
    let BuyContext {
        state,
        signer,
        key,
        mode,
        collection,
        relayer,
        trade,
        ..
    } = ctx;
    let (outcome, logs) = signer.await_receipt_detailed(buy_tx).await?;
    match outcome {
        ReceiptOutcome::Confirmed => {
            let token_id = match mode {
                PurchaseMode::Primary => minted_token_id_from_logs(&logs, collection, relayer)
                    .ok_or_else(|| {
                        ApiError::RelayerFailed(
                            "primary buy confirmed but no ERC-721 Transfer-to-relayer log found; cannot determine the minted tokenId to forward".into(),
                        )
                    })?,
                PurchaseMode::Trade => {
                    trade_delivered_token_id(&logs, collection, relayer, trade, token_id_secondary)?
                }
                _ => token_id_secondary.ok_or_else(|| {
                    ApiError::Internal("secondary buy confirmed without a tokenId".into())
                })?,
            };
            Ok(token_id)
        }
        ReceiptOutcome::Reverted | ReceiptOutcome::Pending => Err(reverted_or_pending_error(
            state,
            key,
            outcome,
            format!("broker buy tx {buy_tx} reverted on-chain; no MANA spent, no item minted"),
            format!(
                "broker buy tx {buy_tx} not yet mined; not confirming \u{2014} retry/reconcile to settle"
            ),
        )
        .await),
    }
}

async fn drive_forward(ctx: BuyContext<'_>, token_id: U256) -> Result<BuyOutcome, ApiError> {
    let BuyContext {
        state,
        signer,
        key,
        collection,
        escrow,
        buyer,
        relayer,
        ..
    } = ctx;
    if let Some(mined) = settle_by_released_forward(state, signer, key).await? {
        return Ok(BuyOutcome {
            forward_tx_hash: mined,
        });
    }
    let fwd = build_forward_to_escrow(collection, relayer, escrow, token_id, buyer);
    let token_id_text = token_id.to_string();
    let fwd_tx = signer
        .send_direct_call_recorded(fwd.to, fwd.data, |signed| {
            record_forward(state, key, signed, token_id_text)
        })
        .await?;

    let outcome = signer.await_receipt(&fwd_tx).await?;
    // A forward that reverts or has not mined may have lost the token to a
    // released forward that mined after all: that one settles the claim.
    if outcome != ReceiptOutcome::Confirmed
        && let Some(mined) = settle_by_released_forward(state, signer, key).await?
    {
        return Ok(BuyOutcome {
            forward_tx_hash: mined,
        });
    }
    match outcome {
        ReceiptOutcome::Confirmed => {
            set_confirmed(state, key).await?;
            Ok(BuyOutcome {
                forward_tx_hash: fwd_tx,
            })
        }
        ReceiptOutcome::Reverted | ReceiptOutcome::Pending => Err(reverted_or_pending_error(
            state,
            key,
            outcome,
            format!(
                "escrow forward tx {fwd_tx} reverted on-chain (token at relayer, not in custody)"
            ),
            format!(
                "escrow forward tx {fwd_tx} not yet mined; not confirming \u{2014} retry/reconcile to settle"
            ),
        )
        .await),
    }
}

fn trade_delivered_token_id(
    logs: &[crate::evm::signer::ReceiptLog],
    collection: Address,
    relayer: Address,
    trade: Option<&ValidatedTrade>,
    expected_token: Option<U256>,
) -> Result<U256, ApiError> {
    // Resumed from the recorded row alone, the trade is known by what the
    // row kept: an NFT trade recorded the token it delivers, taken on the
    // confirmed receipt as a secondary buy's is; an item trade recorded none
    // and mints.
    let from = match (trade, expected_token) {
        (Some(trade), _) => match trade.delivery {
            TradeDelivery::Nft { .. } => trade.signer,
            TradeDelivery::Item { .. } => Address::ZERO,
        },
        (None, Some(expected)) => return Ok(expected),
        (None, None) => Address::ZERO,
    };
    let delivered =
        transferred_token_id_from_logs(logs, collection, from, relayer).ok_or_else(|| {
            ApiError::RelayerFailed(format!(
                "trade accept confirmed but no ERC-721 Transfer({from:#x} -> relayer) log \
                 found on {collection:#x}; cannot determine the delivered tokenId to forward"
            ))
        })?;
    if let Some(expected) = expected_token
        && delivered != expected
    {
        return Err(ApiError::RelayerFailed(format!(
            "trade accept delivered tokenId {delivered}, expected {expected}; refusing to \
                 forward the wrong token"
        )));
    }
    Ok(delivered)
}

/// Drives the buy already claimed under `key` from its recorded row alone,
/// with no request body and no catalogue, trade or price re-check: the buy
/// may already have mined, closing the trade or using up the item's supply,
/// so a fresh validation would refuse a purchase that has spent MANA. `Ok` is
/// the forward transaction hash once the token is in escrow custody; the
/// row's status decides the rest exactly as a repeated [`buy`] does.
pub async fn resume(state: &SharedState, key: &str) -> Result<String, ApiError> {
    require_broadcast_enabled(state, "broker buys")?;
    let Some(signer) = state.transaction.direct_signer() else {
        return Err(ApiError::RelayerUnavailable(
            "resuming a broker buy requires the direct JSON-RPC signer".into(),
        ));
    };
    let signer: &DirectSigner = signer.as_ref();
    let row: Option<(String, String, String, Option<String>)> = sqlx::query_as(
        "SELECT mode, collection, escrow_address, buyer_address \
         FROM shop.broker_purchases WHERE idempotency_key = $1",
    )
    .bind(key)
    .fetch_optional(&state.pool)
    .await?;
    let Some((mode, collection, escrow, buyer)) = row else {
        return Err(ApiError::NotFound(format!(
            "no broker buy was claimed under {key:?}"
        )));
    };
    let buyer = buyer
        .ok_or_else(|| ApiError::Internal(format!("broker buy {key:?} has no buyer_address")))?;
    let ctx = BuyContext {
        state,
        signer,
        key,
        mode: PurchaseMode::parse_wearable(&mode)?,
        collection: parse_address("collection", &collection)?,
        escrow: parse_address("escrowAddress", &escrow)?,
        buyer: parse_address("buyerAddress", &buyer)?,
        relayer: signer.relayer_address(),
        trade: None,
    };
    Ok(resume_existing(ctx).await?.forward_tx_hash)
}

fn frozen_checkout(key: &str) -> ApiError {
    ApiError::Conflict(format!(
        "broker buy {key:?} refused: its credits checkout is frozen (reversing, reversed or \
         failed), so it is not bought"
    ))
}

async fn resume_existing(ctx: BuyContext<'_>) -> Result<BuyOutcome, ApiError> {
    let BuyContext {
        state,
        signer,
        key,
        mode,
        ..
    } = ctx;
    let row: ExistingRow = sqlx::query_as(
        "SELECT status, tx_hash, forward_tx_hash, token_id, minted_token_id, \
                forward_signed_tx, write_ahead \
         FROM shop.broker_purchases WHERE idempotency_key = $1",
    )
    .bind(key)
    .fetch_optional(&state.pool)
    .await?
    // only a frozen credits checkout's line is refused a claim row
    .ok_or_else(|| frozen_checkout(key))?;

    match row.status.as_str() {
        "confirmed" => {
            let fwd = row.forward_tx_hash.ok_or_else(|| {
                ApiError::Internal(format!(
                    "broker buy {key:?} is 'confirmed' but has no forward_tx_hash"
                ))
            })?;
            tracing::info!(idempotency_key = %key, forward_tx_hash = %fwd, "broker buy idempotent replay -> recorded forward txHash");
            Ok(BuyOutcome {
                forward_tx_hash: fwd,
            })
        }
        "reverted" => Err(ApiError::RelayReverted(format!(
            "broker buy {key:?} reverted on a prior attempt; not re-broadcasting"
        ))),
        // A `write_ahead` claim is re-armed by the claim in [`buy`], which
        // has the request to build the transaction from; reaching here, it
        // was taken by a concurrent caller, or it is a claim kept from before
        // transactions were recorded ahead of their broadcast, whose failure
        // cannot prove that nothing was sent.
        "error" if row.write_ahead => {
            if checkout_frozen(&state.pool, key).await? {
                return Err(frozen_checkout(key));
            }
            Err(ApiError::Conflict(format!(
                "broker buy {key:?} failed before broadcast; retry the buy request to buy it again"
            )))
        }
        "error" => Err(ApiError::Conflict(format!(
            "broker buy {key:?} failed under a claim that did not record its transaction \
             before sending; it may have been broadcast, so it is not bought again \u{2014} \
             reconcile manually"
        ))),
        "sent" => {
            let buy_tx = row.tx_hash.clone().ok_or_else(|| {
                ApiError::Internal(format!("broker buy {key:?} is 'sent' without a tx_hash"))
            })?;
            rebroadcast_recorded_buy(state, signer, key, &buy_tx).await?;
            let token_id_secondary = match (mode, row.token_id.as_deref()) {
                (PurchaseMode::Secondary, Some(t)) | (PurchaseMode::Trade, Some(t)) => {
                    Some(parse_token_id(t)?)
                }
                _ => None,
            };
            let token_id = confirm_buy(ctx, &buy_tx, token_id_secondary).await?;
            drive_forward(ctx, token_id).await
        }
        "bought" => {
            let token_id = resume_token_id(ctx, &row).await?;
            drive_forward(ctx, token_id).await
        }
        "forwarding" => {
            let fwd_tx = row.forward_tx_hash.clone().ok_or_else(|| {
                ApiError::Internal(format!(
                    "broker buy {key:?} is 'forwarding' without a forward_tx_hash"
                ))
            })?;
            // A released forward that mined already holds the token, so the
            // recorded one would only revert: settle before paying gas again.
            if let Some(mined) = settle_by_released_forward(state, signer, key).await? {
                return Ok(BuyOutcome {
                    forward_tx_hash: mined,
                });
            }
            if let Some(raw) = row.forward_signed_tx.as_deref() {
                if signer.rebroadcast(&fwd_tx, raw).await? == Rebroadcast::Dead {
                    // The buy mined and the token is with the relayer; once the
                    // recorded forward is confirmed dead, one new forward is
                    // signed in its place.
                    dead_look(state, key, &fwd_tx).await?;
                    if !release_dead_forward(state, key, &fwd_tx).await? {
                        return Err(ApiError::Conflict(format!(
                            "broker buy {key:?} changed state concurrently; retry"
                        )));
                    }
                    let token_id = resume_token_id(ctx, &row).await?;
                    return drive_forward(ctx, token_id).await;
                }
                clear_dead_look(state, key).await?;
            }
            let outcome = signer.await_receipt(&fwd_tx).await?;
            if outcome != ReceiptOutcome::Confirmed
                && let Some(mined) = settle_by_released_forward(state, signer, key).await?
            {
                return Ok(BuyOutcome {
                    forward_tx_hash: mined,
                });
            }
            match outcome {
                ReceiptOutcome::Confirmed => {
                    set_confirmed(state, key).await?;
                    Ok(BuyOutcome {
                        forward_tx_hash: fwd_tx,
                    })
                }
                ReceiptOutcome::Reverted | ReceiptOutcome::Pending => {
                    Err(reverted_or_pending_error(
                        state,
                        key,
                        outcome,
                        format!("escrow forward tx {fwd_tx} reverted on-chain"),
                        format!(
                            "escrow forward tx {fwd_tx} not yet mined; retry/reconcile to settle"
                        ),
                    )
                    .await)
                }
            }
        }
        "pending" => {
            if release_stale_pending(state, key).await? {
                return Err(ApiError::Conflict(format!(
                    "broker buy {key:?} stopped before its transaction was recorded, so nothing \
                     was sent; it is marked failed and the next buy request buys it"
                )));
            }
            Err(ApiError::Conflict(format!(
                "broker buy {key:?} is in flight (status 'pending'); not re-broadcasting \u{2014} retry once it settles"
            )))
        }
        other => Err(ApiError::Conflict(format!(
            "broker buy {key:?} has unexpected status {other:?}; reconcile manually"
        ))),
    }
}

async fn resume_token_id(ctx: BuyContext<'_>, row: &ExistingRow) -> Result<U256, ApiError> {
    let BuyContext {
        signer,
        key,
        mode,
        collection,
        relayer,
        trade,
        ..
    } = ctx;
    match mode {
        PurchaseMode::Trade => {
            if let Some(t) = row.minted_token_id.as_deref() {
                return parse_token_id(t);
            }
            if let Some(t) = row.token_id.as_deref() {
                return parse_token_id(t);
            }
            let buy_tx = row.tx_hash.as_deref().ok_or_else(|| {
                ApiError::Internal(format!("trade broker buy {key:?} 'bought' without tx_hash"))
            })?;
            let (outcome, logs) = signer.await_receipt_detailed(buy_tx).await?;
            if outcome != ReceiptOutcome::Confirmed {
                return Err(ApiError::RelayerTimeout(format!(
                    "trade broker buy {key:?} 'bought' but the accept receipt is not \
                     confirmed on re-poll"
                )));
            }
            trade_delivered_token_id(&logs, collection, relayer, trade, None)
        }
        PurchaseMode::Primary => {
            if let Some(t) = row.minted_token_id.as_deref() {
                return parse_token_id(t);
            }
            let buy_tx = row.tx_hash.as_deref().ok_or_else(|| {
                ApiError::Internal(format!(
                    "primary broker buy {key:?} 'bought' without tx_hash"
                ))
            })?;
            let (outcome, logs) = signer.await_receipt_detailed(buy_tx).await?;
            if outcome != ReceiptOutcome::Confirmed {
                return Err(ApiError::RelayerTimeout(format!(
                    "primary broker buy {key:?} 'bought' but buy receipt not confirmed on re-poll"
                )));
            }
            minted_token_id_from_logs(&logs, collection, relayer).ok_or_else(|| {
                ApiError::RelayerFailed(
                    "primary buy confirmed but no Transfer-to-relayer log found to recover tokenId"
                        .into(),
                )
            })
        }
        _ => {
            let t = row.token_id.as_deref().ok_or_else(|| {
                ApiError::Internal(format!(
                    "secondary broker buy {key:?} 'bought' without token_id"
                ))
            })?;
            parse_token_id(t)
        }
    }
}

#[derive(sqlx::FromRow)]
struct ExistingRow {
    status: String,
    tx_hash: Option<String>,
    forward_tx_hash: Option<String>,
    token_id: Option<String>,
    minted_token_id: Option<String>,
    forward_signed_tx: Option<String>,
    write_ahead: bool,
}

/// Signs the claim's buy, records it (`sent`, its hash and signed bytes)
/// and only then broadcasts it. The record is fenced on the claim made at
/// `claimed_at` still being `pending`, so a caller whose claim was taken over
/// sends nothing. A failure before the record leaves nothing sent and marks
/// the claim `error`, which the next claim re-arms; after the record, the
/// claim stays `sent` and a resume re-sends the recorded bytes.
///
/// A claim re-armed after a dead buy was released first checks the
/// released hashes: if one mined successfully after all, that buy is
/// recorded again (`sent`) and returned for the caller to settle, and
/// nothing new is signed. One that reverted spent nothing, so it is bought
/// again.
pub(crate) async fn send_recorded_buy(
    state: &SharedState,
    signer: &DirectSigner,
    key: &str,
    claimed_at: NaiveDateTime,
    call: BrokerCall,
) -> Result<String, ApiError> {
    let sent = match mined_released_tx(state, signer, key).await {
        Ok(Some((mined, ReceiptOutcome::Confirmed))) => {
            restore_mined_buy(state, key, claimed_at, &mined)
                .await
                .map(|()| mined)
        }
        // a released buy that reverted spent nothing: buy again
        Ok(_) => {
            signer
                .send_direct_call_recorded(call.to, call.data, |signed| {
                    record_buy(state, key, claimed_at, signed)
                })
                .await
        }
        Err(e) => Err(e),
    };
    if sent.is_err() {
        // Fenced on `pending` and the claim's clock: a no-op once recorded.
        mark_error(state, key, claimed_at).await;
    }
    sent
}

/// A buy released as dead under `key` that has a receipt after all (a later
/// look than the two that released it), with its outcome: a confirmed one if
/// any, else the first that reverted. An RPC error is returned, never read
/// as "none mined".
pub(crate) async fn mined_released_tx(
    state: &SharedState,
    signer: &DirectSigner,
    key: &str,
) -> Result<Option<(String, ReceiptOutcome)>, ApiError> {
    mined_released(state, signer, key, "released_tx_hashes").await
}

async fn mined_released(
    state: &SharedState,
    signer: &DirectSigner,
    key: &str,
    column: &'static str,
) -> Result<Option<(String, ReceiptOutcome)>, ApiError> {
    let released: Option<Vec<String>> = sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
        "SELECT {column} FROM shop.broker_purchases WHERE idempotency_key = $1"
    )))
    .bind(key)
    .fetch_optional(&state.pool)
    .await?
    .flatten();
    // a confirmed receipt anywhere in the list wins over an earlier revert
    let mut reverted = None;
    for hash in released.unwrap_or_default() {
        let (outcome, _) = signer.fetch_receipt_once(&hash).await?;
        if outcome != ReceiptOutcome::Pending {
            tracing::error!(idempotency_key = %key, tx_hash = %hash, ?outcome, "a broker transaction released as dead has a receipt after all");
        }
        match outcome {
            ReceiptOutcome::Confirmed => return Ok(Some((hash, outcome))),
            ReceiptOutcome::Reverted if reverted.is_none() => reverted = Some((hash, outcome)),
            _ => {}
        }
    }
    Ok(reverted)
}

/// A forward released as dead that mined successfully after all delivered
/// the token to escrow: the claim is confirmed with that forward, and no
/// other forward is signed. Returns its hash when it settled the claim.
async fn settle_by_released_forward(
    state: &SharedState,
    signer: &DirectSigner,
    key: &str,
) -> Result<Option<String>, ApiError> {
    let Some((mined, ReceiptOutcome::Confirmed)) =
        mined_released(state, signer, key, "released_forward_tx_hashes").await?
    else {
        return Ok(None);
    };
    let settled = sqlx::query(
        "UPDATE shop.broker_purchases SET status = 'confirmed', forward_tx_hash = $2, \
         forward_signed_tx = NULL, \
         released_forward_tx_hashes = array_remove(released_forward_tx_hashes, $2), \
         updated_at = timezone('UTC', now()) \
         WHERE idempotency_key = $1 AND status IN ('sent', 'bought', 'forwarding')",
    )
    .bind(key)
    .bind(&mined)
    .execute(&state.pool)
    .await?;
    if settled.rows_affected() == 0 {
        return Err(ApiError::Conflict(format!(
            "broker buy {key:?} changed state concurrently; retry"
        )));
    }
    Ok(Some(mined))
}

/// Records a released buy that mined after all back on its re-armed claim,
/// fenced like [`record_buy`].
async fn restore_mined_buy(
    state: &SharedState,
    key: &str,
    claimed_at: NaiveDateTime,
    mined: &str,
) -> Result<(), ApiError> {
    let restored = sqlx::query(
        "UPDATE shop.broker_purchases SET status = 'sent', tx_hash = $2, signed_tx = NULL, \
         released_tx_hashes = array_remove(released_tx_hashes, $2), \
         updated_at = timezone('UTC', now()) \
         WHERE idempotency_key = $1 AND status = 'pending' AND tx_hash IS NULL AND updated_at = $3",
    )
    .bind(key)
    .bind(mined)
    .bind(claimed_at)
    .execute(&state.pool)
    .await?;
    if restored.rows_affected() == 0 {
        return Err(ApiError::Conflict(format!(
            "broker buy {key:?} changed state concurrently; retry"
        )));
    }
    Ok(())
}

async fn record_buy(
    state: &SharedState,
    key: &str,
    claimed_at: NaiveDateTime,
    signed: SignedTx,
) -> Result<(), ApiError> {
    let recorded = sqlx::query(
        "UPDATE shop.broker_purchases SET status = 'sent', tx_hash = $2, signed_tx = $3, \
         updated_at = timezone('UTC', now()) \
         WHERE idempotency_key = $1 AND status = 'pending' AND tx_hash IS NULL AND updated_at = $4",
    )
    .bind(key)
    .bind(&signed.hash)
    .bind(&signed.raw)
    .bind(claimed_at)
    .execute(&state.pool)
    .await?;
    if recorded.rows_affected() == 0 {
        return Err(ApiError::Conflict(format!(
            "broker buy {key:?} was taken over before its transaction was recorded; nothing was \
             sent by this request"
        )));
    }
    Ok(())
}

/// Records the forward leg (its hash, signed bytes and the token it moves)
/// before it is broadcast; only one forward is ever recorded per claim.
async fn record_forward(
    state: &SharedState,
    key: &str,
    signed: SignedTx,
    token_id: String,
) -> Result<(), ApiError> {
    let recorded = sqlx::query(
        "UPDATE shop.broker_purchases SET status = 'forwarding', forward_tx_hash = $2, \
         forward_signed_tx = $3, minted_token_id = $4, updated_at = timezone('UTC', now()) \
         WHERE idempotency_key = $1 AND status IN ('sent', 'bought') AND forward_tx_hash IS NULL",
    )
    .bind(key)
    .bind(&signed.hash)
    .bind(&signed.raw)
    .bind(&token_id)
    .execute(&state.pool)
    .await?;
    if recorded.rows_affected() == 0 {
        return Err(ApiError::Conflict(format!(
            "broker buy {key:?} already has a forward recorded or changed state; nothing was \
             sent by this request"
        )));
    }
    Ok(())
}

/// Re-sends the buy leg recorded under `key`, unless it already has a
/// receipt; a claim recorded without its bytes is left to its receipt. A
/// recorded buy that can never mine (a finalized transaction holds its
/// nonce, confirmed by two looks; see [`dead_look`]) spent nothing: the
/// claim is released to `error` with the hash kept in `released_tx_hashes`,
/// the next buy request re-arms it, and this call answers 409.
pub(crate) async fn rebroadcast_recorded_buy(
    state: &SharedState,
    signer: &DirectSigner,
    key: &str,
    tx_hash: &str,
) -> Result<(), ApiError> {
    let raw: Option<String> = sqlx::query_scalar(
        "SELECT signed_tx FROM shop.broker_purchases WHERE idempotency_key = $1",
    )
    .bind(key)
    .fetch_optional(&state.pool)
    .await?
    .flatten();
    let Some(raw) = raw else { return Ok(()) };
    if signer.rebroadcast(tx_hash, &raw).await? != Rebroadcast::Dead {
        return clear_dead_look(state, key).await;
    }
    dead_look(state, key, tx_hash).await?;
    if !release_dead_buy(state, key, tx_hash).await? {
        return Err(ApiError::Conflict(format!(
            "broker buy {key:?} changed state concurrently; retry"
        )));
    }
    Err(ApiError::Conflict(format!(
        "broker buy {key:?}'s recorded transaction {tx_hash} can never mine (a finalized \
         transaction holds its nonce); nothing was spent, and the next buy request buys again"
    )))
}

/// Releases a dead recorded buy to `error`, keeping its hash in
/// `released_tx_hashes`. Fenced on the dead look still being at least
/// [`DEAD_CONFIRM_SECS`] old, so a look that found the transaction alive
/// (and a fresh dead look after it) in between releases nothing.
pub(crate) async fn release_dead_buy(
    state: &SharedState,
    key: &str,
    tx_hash: &str,
) -> Result<bool, ApiError> {
    let released = sqlx::query(
        "UPDATE shop.broker_purchases SET status = 'error', tx_hash = NULL, signed_tx = NULL, \
         released_tx_hashes = array_append(released_tx_hashes, tx_hash), dead_seen_at = NULL, \
         updated_at = timezone('UTC', now()) \
         WHERE idempotency_key = $1 AND status = 'sent' AND tx_hash = $2 AND write_ahead \
           AND dead_seen_at <= $3 - make_interval(secs => $4)",
    )
    .bind(key)
    .bind(tx_hash)
    .bind(crate::evm::now())
    .bind(DEAD_CONFIRM_SECS as f64)
    .execute(&state.pool)
    .await?;
    if released.rows_affected() > 0 {
        tracing::error!(idempotency_key = %key, tx_hash = %tx_hash, "broker buy's recorded transaction can never mine; claim released for a new buy");
    }
    Ok(released.rows_affected() > 0)
}

/// How long apart two dead looks at a recorded leg must be before it is
/// released: one lookup can be wrong (a lagging load-balanced backend), two
/// minutes apart agreeing are taken as final.
const DEAD_CONFIRM_SECS: i64 = 600;

/// The oldest claim a dead look may release. Nodes may prune the lookup
/// index of old transactions, so for an older claim a missing receipt is
/// not evidence: it is logged and left for manual reconciliation.
const DEAD_RELEASE_MAX_AGE_DAYS: i32 = 7;

/// Records a dead look at the recorded leg `tx_hash` and decides: `Ok` when
/// an earlier look at least [`DEAD_CONFIRM_SECS`] ago agrees (release it);
/// a retryable `RelayerTimeout` after a first look; `Conflict` for a claim
/// older than [`DEAD_RELEASE_MAX_AGE_DAYS`], which needs reconciliation.
async fn dead_look(state: &SharedState, key: &str, tx_hash: &str) -> Result<(), ApiError> {
    let now = crate::evm::now();
    let looked: Option<(bool, bool)> = sqlx::query_as(
        "UPDATE shop.broker_purchases SET dead_seen_at = COALESCE(dead_seen_at, $3) \
         WHERE idempotency_key = $1 \
           AND ((status = 'sent' AND tx_hash = $2) OR (status = 'forwarding' AND forward_tx_hash = $2)) \
         RETURNING dead_seen_at <= $3 - make_interval(secs => $4) AS confirmed, \
                   created_at >= $3 - make_interval(days => $5) AS recent",
    )
    .bind(key)
    .bind(tx_hash)
    .bind(now)
    .bind(DEAD_CONFIRM_SECS as f64)
    .bind(DEAD_RELEASE_MAX_AGE_DAYS)
    .fetch_optional(&state.pool)
    .await?;
    match looked {
        None => Err(ApiError::Conflict(format!(
            "broker buy {key:?} changed state concurrently; retry"
        ))),
        Some((_, false)) => {
            tracing::error!(idempotency_key = %key, tx_hash = %tx_hash, "recorded broker transaction looks dead but its claim is too old to trust a missing receipt; reconcile manually");
            Err(ApiError::Conflict(format!(
                "broker buy {key:?}'s recorded transaction {tx_hash} looks dead, but the claim \
                 is older than {DEAD_RELEASE_MAX_AGE_DAYS} days \u{2014} reconcile manually"
            )))
        }
        Some((false, true)) => Err(ApiError::RelayerTimeout(format!(
            "broker buy {key:?}'s recorded transaction {tx_hash} looks dead; a retry at least \
             {DEAD_CONFIRM_SECS} s later confirms it before anything is released"
        ))),
        Some((true, true)) => Ok(()),
    }
}

/// Forgets an earlier dead look once a look finds the recorded leg alive.
async fn clear_dead_look(state: &SharedState, key: &str) -> Result<(), ApiError> {
    sqlx::query(
        "UPDATE shop.broker_purchases SET dead_seen_at = NULL \
         WHERE idempotency_key = $1 AND dead_seen_at IS NOT NULL",
    )
    .bind(key)
    .execute(&state.pool)
    .await?;
    Ok(())
}

/// Clears a recorded forward that can never mine so a new one can be
/// recorded: the row goes back to `bought` (the buy mined; the token is with
/// the relayer). Fenced on the dead forward still being the recorded one and
/// its dead look still being at least [`DEAD_CONFIRM_SECS`] old.
pub(crate) async fn release_dead_forward(
    state: &SharedState,
    key: &str,
    fwd_tx: &str,
) -> Result<bool, ApiError> {
    let released = sqlx::query(
        "UPDATE shop.broker_purchases SET status = 'bought', forward_tx_hash = NULL, \
         forward_signed_tx = NULL, \
         released_forward_tx_hashes = array_append(released_forward_tx_hashes, forward_tx_hash), \
         dead_seen_at = NULL, updated_at = timezone('UTC', now()) \
         WHERE idempotency_key = $1 AND status = 'forwarding' AND forward_tx_hash = $2 \
           AND dead_seen_at <= $3 - make_interval(secs => $4)",
    )
    .bind(key)
    .bind(fwd_tx)
    .bind(crate::evm::now())
    .bind(DEAD_CONFIRM_SECS as f64)
    .execute(&state.pool)
    .await?;
    if released.rows_affected() > 0 {
        tracing::error!(idempotency_key = %key, forward_tx_hash = %fwd_tx, "broker forward's recorded transaction can never mine; signing a new forward");
    }
    Ok(released.rows_affected() > 0)
}

/// Marks a `write_ahead` claim `error` once it has stayed `pending` with no
/// recorded transaction past the takeover delay: its caller stopped before
/// recording, so nothing was sent. Fenced like the record, so an in-flight
/// caller's record then fails and it sends nothing.
async fn release_stale_pending(state: &SharedState, key: &str) -> Result<bool, ApiError> {
    let released = sqlx::query(
        "UPDATE shop.broker_purchases SET status = 'error', updated_at = $2 \
         WHERE idempotency_key = $1 AND status = 'pending' AND write_ahead AND tx_hash IS NULL \
           AND updated_at < $2 - make_interval(secs => $3)",
    )
    .bind(key)
    .bind(crate::evm::now())
    .bind(BROKER_PENDING_TAKEOVER_SECS as f64)
    .execute(&state.pool)
    .await?;
    Ok(released.rows_affected() > 0)
}

async fn set_confirmed(state: &SharedState, key: &str) -> Result<(), ApiError> {
    sqlx::query(
        "UPDATE shop.broker_purchases SET status = 'confirmed', updated_at = timezone('UTC', now()) \
         WHERE idempotency_key = $1",
    )
    .bind(key)
    .execute(&state.pool)
    .await?;
    Ok(())
}

pub(crate) async fn set_reverted(state: &SharedState, key: &str) {
    if let Err(e) = sqlx::query(
        "UPDATE shop.broker_purchases SET status = 'reverted', updated_at = timezone('UTC', now()) \
         WHERE idempotency_key = $1 AND status IN ('sent', 'bought', 'forwarding')",
    )
    .bind(key)
    .execute(&state.pool)
    .await
    {
        tracing::error!(idempotency_key = %key, error = %e, "failed to mark broker buy 'reverted'");
    }
}

/// The `Reverted`/`Pending` tail shared by `confirm_buy`, `drive_forward`, and the
/// "forwarding" branch of `resume_existing`: on `Reverted` the tracked purchase row is marked
/// reverted and the call returns `RelayReverted`; on `Pending` it returns `RelayerTimeout`
/// without touching the row. Each call site's message text differs by more than an operation
/// label, so the finished strings are passed in rather than assembled here. `Confirmed` is
/// never passed in: it differs meaningfully at each site and moves money, so it is handled
/// inline by the caller.
async fn reverted_or_pending_error(
    state: &SharedState,
    key: &str,
    outcome: ReceiptOutcome,
    reverted_message: String,
    pending_message: String,
) -> ApiError {
    match outcome {
        ReceiptOutcome::Reverted => {
            set_reverted(state, key).await;
            ApiError::RelayReverted(reverted_message)
        }
        ReceiptOutcome::Pending => ApiError::RelayerTimeout(pending_message),
        ReceiptOutcome::Confirmed => {
            unreachable!("reverted_or_pending_error is only called for Reverted/Pending")
        }
    }
}

async fn mark_error(state: &SharedState, key: &str, claimed_at: NaiveDateTime) {
    if let Err(e) = sqlx::query(
        "UPDATE shop.broker_purchases SET status = 'error', updated_at = timezone('UTC', now()) \
         WHERE idempotency_key = $1 AND status = 'pending' AND tx_hash IS NULL AND updated_at = $2",
    )
    .bind(key)
    .bind(claimed_at)
    .execute(&state.pool)
    .await
    {
        tracing::error!(idempotency_key = %key, error = %e, "failed to mark broker buy 'error' after broadcast failure");
    }
}

#[cfg(test)]
mod tests {
    use super::{collections_spender, trade_delivered_token_id};
    use crate::evm::contracts_addrs::DclContracts;
    use crate::evm::signer::ReceiptLog;
    use alloy_primitives::{Address, B256, Bytes, U256};

    fn transfer(collection: Address, from: Address, to: Address, token: u64) -> ReceiptLog {
        let word = |a: Address| B256::left_padding_from(a.as_slice());
        ReceiptLog {
            address: collection,
            topics: vec![
                B256::from(crate::evm::abi::ERC721_TRANSFER_TOPIC0),
                word(from),
                word(to),
                B256::from(U256::from(token)),
            ],
            data: Bytes::new(),
        }
    }

    #[test]
    fn a_trade_resumed_without_its_payload_takes_the_token_the_row_recorded() {
        let collection = Address::repeat_byte(0xc0);
        let relayer = Address::repeat_byte(0x11);
        let seller = Address::repeat_byte(0x22);
        let nft = [transfer(collection, seller, relayer, 7)];
        assert_eq!(
            trade_delivered_token_id(&nft, collection, relayer, None, Some(U256::from(7))).unwrap(),
            U256::from(7),
            "an NFT trade forwards the token its row recorded"
        );
        let minted = [transfer(collection, Address::ZERO, relayer, 9)];
        assert_eq!(
            trade_delivered_token_id(&minted, collection, relayer, None, None).unwrap(),
            U256::from(9),
            "an item trade with no recorded token forwards the one it minted"
        );
        assert!(
            trade_delivered_token_id(&nft, collection, relayer, None, None).is_err(),
            "an item trade whose receipt shows no mint to the relayer forwards nothing"
        );
    }

    #[test]
    fn collections_spender_resolves_store_and_marketplace_only() {
        let c = DclContracts::for_chain(137).expect("polygon contracts");
        assert_eq!(
            collections_spender(&c, "collection-store").unwrap(),
            c.collection_store
        );
        assert_eq!(
            collections_spender(&c, " marketplace-v2 ").unwrap(),
            c.marketplace_v2
        );
        assert_eq!(
            collections_spender(&c, "offchain-marketplace").unwrap(),
            c.offchain_marketplace
        );
        assert_eq!(
            format!(
                "{:#x}",
                collections_spender(&c, "offchain-marketplace").unwrap()
            ),
            "0x540fb08edb56aae562864b390542c97f562825ba"
        );
        assert!(collections_spender(&c, "controller").is_err());
        assert!(
            collections_spender(&c, "0x214ffc0f0103735728dc66b61a22e4f163e275ae").is_err(),
            "raw addresses are refused: the allowance may only be armed for known contracts"
        );
        assert!(collections_spender(&c, "").is_err());
    }
}
