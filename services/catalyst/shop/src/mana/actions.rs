// from: marketplace@f3956826:webapp/src/modules/{bid,order,nft,rental}/sagas.ts (ACCEPT_BID, CANCEL_BID, CANCEL_ORDER, TRANSFER_NFT, CLAIM_ASSET, REMOVE_RENTAL: what each sends and checks first), shop@94584a5c:app/src/lib/trades.ts (cancelSignature for a Shop listing)
//! `POST /v1/mana/actions`: the calldata of a seller's or an owner's MANA
//! action, built from the record it names and checked against the chain
//! before it is handed out: accept a bid (the bidder pays, the seller
//! sends its token), cancel a listing or a bid, cancel a legacy order,
//! transfer a token, claim a LAND back after its rent, remove a rental
//! listing, and the MANA allowance a venue needs. Every action is signed
//! by the party it is for (the signer of the trade, the seller of the
//! order, the owner of the token, the lessor), which the route verifies
//! against the record and the chain, not against the body.

use alloy_primitives::{Address, U256};
use axum::Json;
use axum::extract::{OriginalUri, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use serde_json::json;

use super::chain::{ReadError, Reader, calldata_hex};
use super::facts;
use super::venues::Chain;
use super::wire::{
    ActionKind, ActionRequest, ActionResponse, AllowanceNeed, ApprovalNeed, CallDto, Check,
    GasEstimate,
};
use super::{SharedState, calls, error, verify};
use crate::rentals::calls as rentals_calls;
use crate::types::{AssetId, EvmAddress, TokenRef};

pub const ROUTE: &str = "/v1/mana/actions";

struct Built {
    call: calls::Call,
    checks: Vec<Check>,
    execution: serde_json::Value,
    static_gas: u64,
}

async fn owner_check(
    reader: Option<&Reader>,
    contract: Address,
    token_id: U256,
    expected: Address,
) -> Check {
    let Some(reader) = reader else {
        return Check::unknown("ownsToken", "no JSON-RPC endpoint for the chain");
    };
    match reader.owner_of(contract, token_id).await {
        Ok(Some(owner)) if owner == expected => Check::pass("ownsToken", format!("{owner:#x}")),
        Ok(Some(owner)) => Check::fail(
            "ownsToken",
            format!("{owner:#x} holds the token, not {expected:#x}"),
        ),
        Ok(None) => Check::fail("ownsToken", "the token does not exist"),
        Err(ReadError::Unavailable(e)) => Check::unknown("ownsToken", e),
        Err(e) => Check::fail("ownsToken", e.to_string()),
    }
}

fn token_of(asset: Option<AssetId>) -> Result<(AssetId, U256), Response> {
    let asset =
        asset.ok_or_else(|| error(StatusCode::BAD_REQUEST, "Bad Request", "asset is required"))?;
    match asset.token {
        TokenRef::Token { token_id } => Ok((asset, token_id)),
        TokenRef::Item { .. } => Err(error(
            StatusCode::BAD_REQUEST,
            "Bad Request",
            "the action names a minted token, not an item",
        )),
    }
}

async fn build(
    state: &SharedState,
    chain: &Chain,
    reader: Option<&Reader>,
    actor: Address,
    req: &ActionRequest,
) -> Result<Built, Response> {
    let bad = |m: String| error(StatusCode::BAD_REQUEST, "Bad Request", &m);
    match req.kind {
        ActionKind::AcceptBid | ActionKind::CancelTrade => {
            let id = req
                .trade_id
                .as_deref()
                .ok_or_else(|| bad("tradeId is required".into()))?;
            let trade = facts::load_trade(&state.pool, id)
                .await
                .map_err(|e| bad(e.to_string()))?
                .ok_or_else(|| {
                    error(
                        StatusCode::NOT_FOUND,
                        "Not Found",
                        &format!("trade {id} is not known to this node"),
                    )
                })?;
            if trade.chain_id.is_some_and(|c| c != chain.chain_id.0) {
                return Err(bad(format!(
                    "trade {id} is not on chain {}",
                    chain.chain_id
                )));
            }
            let venue: Address = trade.contract.as_deref().and_then(|c| c.trim().parse().ok()).filter(|v| chain.marketplace(*v).is_some()).ok_or_else(|| bad(format!("trade {id} was signed for a contract that is not an off-chain marketplace of its chain")))?;
            let mut checks = vec![verify::trade_signature_check(&trade, chain, venue)];
            checks.extend(verify::trade_time_checks(
                &trade,
                chrono::Utc::now().timestamp_millis(),
            ));
            let signer: Address = trade
                .signer
                .trim()
                .parse()
                .map_err(|_| bad("the trade's signer is not an address".into()))?;
            let call = if req.kind == ActionKind::CancelTrade {
                checks.push(if signer == actor {
                    Check::pass("isSigner", "")
                } else {
                    Check::fail("isSigner", "only the signer cancels a trade")
                });
                calls::cancel_signature(chain, venue, &trade).map_err(|e| bad(e.to_string()))?
            } else {
                if trade.trade_type != "bid" {
                    return Err(bad(format!(
                        "trade {id} is a {}, not a bid",
                        trade.trade_type
                    )));
                }
                let wanted = trade
                    .received
                    .iter()
                    .find(|a| a.asset_type == 3)
                    .ok_or_else(|| bad("the bid asks for no token".into()))?;
                let contract: Address = wanted
                    .contract_address
                    .trim()
                    .parse()
                    .map_err(|_| bad("the bid's token contract is not an address".into()))?;
                let token_id = wanted
                    .token_id
                    .as_deref()
                    .and_then(|t| U256::from_str_radix(t.trim(), 10).ok())
                    .ok_or_else(|| bad("the bid names no token id".into()))?;
                checks.push(owner_check(reader, contract, token_id, actor).await);
                if let Some(net) = chain.net {
                    let (verdict, ready) = facts::trade_verdict(&state.pool, net, &trade.id)
                        .await
                        .map_err(|e| bad(e.to_string()))?;
                    checks.push(verify::trade_verdict_check(
                        verdict.as_ref(),
                        trade.checks.uses,
                        ready,
                    ));
                }
                if let (Some(reader), Some((token, amount))) =
                    (reader, calls::mana_leg_sent(&trade))
                {
                    checks.push(match reader.erc20_balance(token, signer).await {
                        Ok(balance) if balance >= amount => {
                            Check::pass("bidderFunded", format!("{balance} wei"))
                        }
                        Ok(balance) => Check::fail(
                            "bidderFunded",
                            format!("the bidder holds {balance} wei of the {amount} bid"),
                        ),
                        Err(e) => Check::unknown("bidderFunded", e.to_string()),
                    });
                }
                calls::accept(chain, venue, &trade, actor, None).map_err(|e| bad(e.to_string()))?
            };
            Ok(Built {
                call,
                checks,
                execution: json!({ "kind": req.kind.execution_kind(), "chainId": chain.chain_id, "listing": { "tradeId": id } }),
                static_gas: 300_000,
            })
        }
        ActionKind::CancelOrder => {
            let (asset, token_id) = token_of(req.asset)?;
            let (order, ready) =
                facts::legacy_order(&state.pool, chain, asset.contract.address(), token_id)
                    .await
                    .map_err(|e| bad(e.to_string()))?;
            if !ready {
                return Err(crate::market::unavailable(
                    crate::market::Source::NftIndex,
                    "the legacy orders are read from the nft index, which is not published yet",
                ));
            }
            let order = order.ok_or_else(|| {
                error(
                    StatusCode::NOT_FOUND,
                    "Not Found",
                    "the token has no legacy order",
                )
            })?;
            let checks = vec![
                if order.status == "open" {
                    Check::pass("open", "")
                } else {
                    Check::fail("open", order.status.clone())
                },
                if order.seller == actor {
                    Check::pass("isSeller", "")
                } else {
                    Check::fail("isSeller", "only the seller cancels an order")
                },
            ];
            Ok(Built {
                call: calls::cancel_order(chain, asset.contract.address(), token_id)
                    .map_err(|e| bad(e.to_string()))?,
                checks,
                execution: json!({ "kind": "cancel_order", "chainId": chain.chain_id, "asset": asset, "listing": { "orderId": order.order_id } }),
                static_gas: 120_000,
            })
        }
        ActionKind::Transfer => {
            let (asset, token_id) = token_of(req.asset)?;
            let to = req.to.ok_or_else(|| bad("to is required".into()))?;
            if to.address() == actor || to.address() == Address::ZERO {
                return Err(bad("a transfer goes to another address".into()));
            }
            let checks = vec![owner_check(reader, asset.contract.address(), token_id, actor).await];
            Ok(Built {
                call: calls::transfer(asset.contract.address(), actor, to.address(), token_id),
                checks,
                execution: json!({ "kind": "transfer", "chainId": chain.chain_id, "asset": asset, "to": to }),
                static_gas: 150_000,
            })
        }
        ActionKind::ClaimRental | ActionKind::CancelRental => {
            let (asset, token_id) = token_of(req.asset)?;
            let rentals = chain
                .rentals
                .ok_or_else(|| bad("the chain has no Rentals contract".into()))?;
            let mut checks = Vec::new();
            let mut listing_id = None;
            if let Some(id) = req.rental_id.as_deref() {
                let listing = state
                    .rentals
                    .db
                    .get_listing_by_id(id)
                    .await
                    .map_err(|e| bad(e.to_string()))?
                    .ok_or_else(|| {
                        error(
                            StatusCode::NOT_FOUND,
                            "Not Found",
                            "the rental listing is not known to this node",
                        )
                    })?;
                let same = listing
                    .contract_address
                    .eq_ignore_ascii_case(&asset.contract.to_string())
                    && listing.token_id == token_id.to_string();
                checks.push(if same {
                    Check::pass("listingAsset", "")
                } else {
                    Check::fail("listingAsset", "the listing is for another asset")
                });
                checks.push(
                    if listing
                        .lessor
                        .as_deref()
                        .is_some_and(|l| l.eq_ignore_ascii_case(&format!("{actor:#x}")))
                    {
                        Check::pass("isLessor", "")
                    } else {
                        Check::fail("isLessor", "only the lessor acts on a listing")
                    },
                );
                listing_id = Some(listing.id);
            }
            let call = if req.kind == ActionKind::ClaimRental {
                if let Some(reader) = reader {
                    match reader
                        .call(
                            rentals,
                            &rentals_calls::rental_calldata(asset.contract.address(), token_id),
                        )
                        .await
                        .map(|d| rentals_calls::decode_rental(&d))
                    {
                        Ok(Some((lessor, _tenant, end))) => {
                            checks.push(if lessor == actor {
                                Check::pass("isLessor", format!("{lessor:#x}"))
                            } else {
                                Check::fail(
                                    "isLessor",
                                    format!("the contract records {lessor:#x} as the lessor"),
                                )
                            });
                            let now = U256::from(chrono::Utc::now().timestamp().max(0) as u64);
                            checks.push(if end <= now {
                                Check::pass("rentOver", format!("ended at {end}"))
                            } else {
                                Check::fail("rentOver", format!("rented until {end}"))
                            });
                        }
                        Ok(None) => checks.push(Check::unknown(
                            "isLessor",
                            "the Rentals contract answered no rental",
                        )),
                        Err(ReadError::Unavailable(e)) => {
                            checks.push(Check::unknown("isLessor", e))
                        }
                        Err(e) => checks.push(Check::fail("isLessor", e.to_string())),
                    }
                } else {
                    checks.push(Check::unknown(
                        "isLessor",
                        "no JSON-RPC endpoint for the chain",
                    ));
                }
                calls::Call {
                    to: rentals,
                    data: rentals_calls::claim_calldata(&[(asset.contract.address(), token_id)]),
                    allowance: None,
                    approval: None,
                }
            } else {
                checks.push(owner_check(reader, asset.contract.address(), token_id, actor).await);
                calls::Call {
                    to: rentals,
                    data: rentals_calls::bump_asset_index_calldata(
                        asset.contract.address(),
                        token_id,
                    ),
                    allowance: None,
                    approval: None,
                }
            };
            Ok(Built {
                call,
                checks,
                execution: json!({ "kind": req.kind.execution_kind(), "chainId": chain.chain_id, "asset": asset, "listing": { "rentalId": listing_id } }),
                static_gas: 200_000,
            })
        }
        ActionKind::ApproveMana => {
            let spender = req
                .spender
                .ok_or_else(|| bad("spender is required".into()))?;
            let amount = req.amount.ok_or_else(|| bad("amount is required".into()))?;
            let known = chain.marketplace(spender.address()).is_some()
                || chain.legacy_marketplace == Some(spender.address())
                || chain.rentals == Some(spender.address())
                || chain.name_controller == Some(spender.address());
            if !known {
                return Err(bad(format!(
                    "{spender} is not a settlement venue of chain {}",
                    chain.chain_id
                )));
            }
            Ok(Built {
                call: calls::approve_mana(chain, spender.address(), amount.get()),
                checks: vec![Check::pass("venue", format!("{spender}"))],
                execution: json!({ "kind": "approve_mana", "chainId": chain.chain_id, "to": spender }),
                static_gas: 60_000,
            })
        }
    }
}

pub async fn post_action(
    State(state): State<SharedState>,
    OriginalUri(original): OriginalUri,
    headers: HeaderMap,
    body: axum::body::Bytes,
) -> Response {
    let req: ActionRequest = match serde_json::from_slice(&body) {
        Ok(r) => r,
        Err(e) => {
            return error(
                StatusCode::BAD_REQUEST,
                "Bad Request",
                &format!("body: {e}"),
            );
        }
    };
    let actor = match state
        .require_signer(&headers, "post", &original, ROUTE)
        .await
    {
        Ok(a) => a,
        Err(r) => return r,
    };
    let chain_id = match (req.asset, req.trade_id.as_deref()) {
        (Some(asset), _) => asset.chain_id,
        (None, Some(id)) => match facts::load_trade(&state.pool, id).await {
            Ok(Some(t)) => crate::types::ChainId(t.chain_id.unwrap_or(0)),
            Ok(None) => {
                return error(
                    StatusCode::NOT_FOUND,
                    "Not Found",
                    &format!("trade {id} is not known to this node"),
                );
            }
            Err(e) => return error(StatusCode::BAD_REQUEST, "Bad Request", &e.to_string()),
        },
        (None, None) => match req.kind {
            ActionKind::ApproveMana => crate::types::ChainId(137),
            _ => {
                return error(
                    StatusCode::BAD_REQUEST,
                    "Bad Request",
                    "asset or tradeId is required",
                );
            }
        },
    };
    let Some(chain) = state.venues.chain(chain_id).cloned() else {
        return error(
            StatusCode::BAD_REQUEST,
            "Bad Request",
            &format!("chain {chain_id} is not one the Shop settles on"),
        );
    };
    let reader = state.reader(chain.chain_id);
    let built = match build(&state, &chain, reader.as_ref(), actor, &req).await {
        Ok(b) => b,
        Err(r) => return r,
    };
    let mut gas = GasEstimate {
        estimate: Some(built.static_gas),
        source: "static",
        gas_price_wei: None,
        fee_wei: None,
        detail: None,
    };
    if let Some(reader) = reader.as_ref() {
        match reader
            .estimate_gas(actor, built.call.to, &built.call.data)
            .await
        {
            Ok(estimate) => {
                gas.estimate = Some(estimate);
                gas.source = "rpc";
            }
            Err(ReadError::Revert(why)) => {
                gas.detail = Some(format!("the call would revert as it stands: {why}"))
            }
            Err(e) => gas.detail = Some(e.to_string()),
        }
        if let Ok(price) = reader.gas_price().await {
            gas.gas_price_wei = Some(price.to_string());
            gas.fee_wei = gas
                .estimate
                .map(|g| (U256::from(price) * U256::from(g)).to_string());
        }
    }
    let executable = verify::executable(&built.checks);
    let call = built.call;
    Json(ActionResponse {
        kind: req.kind,
        execution_kind: req.kind.execution_kind(),
        chain_id: chain.chain_id,
        call: CallDto {
            to: EvmAddress::new(call.to),
            data: calldata_hex(&call.data),
        },
        allowance: call
            .allowance
            .map(|(token, spender, amount)| AllowanceNeed {
                token: EvmAddress::new(token),
                spender: EvmAddress::new(spender),
                amount: crate::types::ManaWei::new(amount),
            }),
        approval: call.approval.map(|(contract, token_id)| ApprovalNeed {
            contract: EvmAddress::new(contract),
            token_id: token_id.to_string(),
            operator: EvmAddress::new(call.to),
        }),
        gas,
        checks: built.checks,
        executable,
        execution: built.execution,
    })
    .into_response()
}
