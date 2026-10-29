// from: shop@94584a5c:app/src/lib/{payment-options,mana,buy}.ts (what a purchase presentation reads: the quote per method, the wallet's MANA and allowance, the gas), SHOP.md "For a web2 buyer on a MANA-only feature"
//! `POST /v1/purchase-offers`: the quote of every cart line, authoritative
//! end to end. Each line is resolved to its record ([`facts`]), judged by
//! THE rule ([`policy::eligibility`]), priced per offered method
//! ([`policy::quote`], credits and MANA, at the node's MANA/USD spot), and,
//! where MANA is offered, given its execution block: the chain and venue,
//! the exact MANA required, the allowance and the calldata, a gas estimate
//! and, for a known wallet, its balances and shortfalls; a web2 buyer's
//! guidance comes from the rule (`WalletGuidance`) and this block together.
//! A method the rule offers but the node cannot price is reported in
//! `unavailable` with the missing fact, never dropped (the UI keeps the
//! option on screen, unavailable, with a retry).

use std::sync::Arc;

use alloy_primitives::{Address, U256};
use axum::Json;
use axum::extract::{OriginalUri, State};
use axum::http::HeaderMap;
use axum::response::{IntoResponse, Response};
use chrono::Utc;
use serde_json::{Value, json};

use super::chain::{ReadError, Reader, calldata_hex};
use super::facts::{self, Listing, Resolved};
use super::venues::Chain;
use super::wire::{
    AllowanceNeed, ApprovalNeed, Balances, CallDto, Check, Freshness, GasEstimate, LineError,
    MAX_LINES, ManaExecution, OfferAnswer, OffersRequest, OffersResponse, PAYMENT_RULE_REFUSED,
    PurchaseOffer, SettlementFor, Unavailable, missing_source, refusal_json,
};
use super::{SharedState, calls, verify};
use crate::policy::{Authoritative, CartLine, Quote, Refusal, eligibility, partition, quote};
use crate::types::{EvmAddress, ManaUsdRate, ManaWei, PaymentMethod, Session, SettlementGroupKey};

pub const ROUTE: &str = "/v1/purchase-offers";

/// The static gas budgets of the calls, used when the RPC estimate cannot
/// run (no wallet known, or the call would revert as it stands: an
/// allowance not yet given): generous upper bounds of the observed costs.
pub fn static_gas(listing: &Listing) -> u64 {
    match listing {
        Listing::Store { .. } => 300_000,
        Listing::Trade { .. } => 350_000,
        Listing::LegacyOrder { .. } => 250_000,
        Listing::Rental { .. } => 400_000,
        Listing::Name { .. } => 300_000,
    }
}

/// The node's MANA/USD spot as the rule's rate: the newest price snapshot
/// within its freshness budget, exact decimal text, never a float.
pub async fn mana_usd_rate(pool: &sqlx::PgPool) -> Result<Option<ManaUsdRate>, sqlx::Error> {
    let row: Option<(String, chrono::NaiveDateTime)> = sqlx::query_as(
        "SELECT mana_usd::text, taken_at FROM economy.price_snapshots ORDER BY taken_at DESC LIMIT 1",
    )
    .fetch_optional(pool)
    .await?;
    let budget = crate::price::store::SNAPSHOT_MAX_AGE.as_secs() as i64;
    Ok(row.and_then(|(rate, taken_at)| {
        let fresh = Utc::now().timestamp() - taken_at.and_utc().timestamp() <= budget;
        fresh
            .then(|| ManaUsdRate::parse_usd_per_mana(rate.trim()))
            .flatten()
    }))
}

/// The session the rule reads: a signed-fetch identity is an account; it
/// is a wallet only when the signed metadata says so (`walletKind:
/// "web3"`, the UI's own session fact, signed by the identity that made
/// it). Nothing else in a request can widen the choices.
pub fn session_of(signer: Option<&str>, metadata: Option<&str>) -> Session {
    let Some(signer) = signer.and_then(|s| EvmAddress::parse(&s.to_ascii_lowercase()).ok()) else {
        return Session::SignedOut;
    };
    let web3 = metadata
        .and_then(|m| serde_json::from_str::<Value>(m).ok())
        .and_then(|m| {
            m.get("walletKind")
                .and_then(|k| k.as_str())
                .map(|k| k == "web3")
        })
        .unwrap_or(false);
    if web3 {
        Session::Web3 { wallet: signer }
    } else {
        Session::Web2 { account: signer }
    }
}

fn line_error(index: usize, e: facts::ResolveError) -> LineError {
    match e {
        facts::ResolveError::IndexNotReady { index: unpublished } => LineError {
            index,
            error: "Service Unavailable",
            message: e.to_string(),
            refusal: None,
            source: Some(unpublished),
        },
        facts::ResolveError::Db(_) => LineError {
            index,
            error: "Internal Server Error",
            message: "the listing could not be read".into(),
            refusal: None,
            source: None,
        },
        other => LineError {
            index,
            error: "Bad Request",
            message: other.to_string(),
            refusal: None,
            source: None,
        },
    }
}

fn refused(index: usize, refusal: Refusal) -> LineError {
    let source = match &refusal {
        Refusal::Incomplete { missing } => Some(missing_source(*missing)),
        _ => None,
    };
    LineError {
        index,
        error: if source.is_some() {
            "Service Unavailable"
        } else {
            PAYMENT_RULE_REFUSED
        },
        message: refusal.to_string(),
        refusal: Some(refusal_json(&refusal)),
        source,
    }
}

/// The settlement key a line joins when paid with `method`.
fn settlement_key(
    line: &Authoritative,
    method: PaymentMethod,
    rate: Option<&ManaUsdRate>,
) -> Option<SettlementGroupKey> {
    partition(
        &[CartLine {
            line: line.clone(),
            method,
        }],
        rate,
    )
    .ok()
    .and_then(|groups| groups.first().map(|g| g.key))
}

/// The MANA execution block of a line the rule offers MANA for.
async fn mana_execution(
    resolved: &Resolved,
    amount: ManaWei,
    wallet: Option<Address>,
    reader: Option<&Reader>,
    mut checks: Vec<Check>,
    fingerprint: Option<alloy_primitives::B256>,
) -> ManaExecution {
    let chain: &Chain = &resolved.chain;
    let caller = wallet.unwrap_or(Address::ZERO);
    let call = match &resolved.listing {
        Listing::Store { venue, price } => match (resolved.asset.token, wallet) {
            (crate::types::TokenRef::Item { item_id }, Some(buyer)) => {
                let call = crate::checkout::calls::build_store_buy(*venue, resolved.asset.contract.address(), item_id, price.get(), buyer);
                Ok(calls::Call { to: call.to, data: call.data, allowance: Some((chain.mana, *venue, price.get())), approval: None })
            }
            _ => Err("a mint names its beneficiary: sign in for the unit calldata".to_string()),
        },
        Listing::Trade { trade, venue, .. } => calls::accept(chain, *venue, trade, caller, Some(amount.get())).map_err(|e| e.to_string()),
        Listing::LegacyOrder { order, .. } => match resolved.asset.token {
            crate::types::TokenRef::Token { token_id } => calls::execute_order(chain, resolved.asset.contract.address(), token_id, order.price, fingerprint).map_err(|e| e.to_string()),
            _ => Err("a legacy order sells a token".to_string()),
        },
        Listing::Rental { listing, terms, rentals, .. } => match wallet {
            Some(tenant) => crate::rentals::calls::accept_listing_calldata(listing, tenant, terms, fingerprint.unwrap_or_default())
                .map(|data| calls::Call { to: *rentals, data, allowance: Some((chain.mana, *rentals, amount.get())), approval: None })
                .map_err(|e| e.to_string()),
            None => Err("a rent names its tenant as the operator: sign in with the renting wallet for the calldata".to_string()),
        },
        Listing::Name { label, .. } => match wallet {
            Some(beneficiary) => calls::register_name(chain, label, beneficiary, amount.get()).map_err(|e| e.to_string()),
            None => Err("a registration names its beneficiary: sign in with the registering wallet for the calldata".to_string()),
        },
    };
    let (call, venue) = match call {
        Ok(call) => {
            let venue = call.to;
            (Some(call), venue)
        }
        Err(e) => {
            checks.push(Check::fail("calldata", e));
            (None, resolved.offer_venue().unwrap_or(Address::ZERO))
        }
    };
    let allowance = call
        .as_ref()
        .and_then(|c| c.allowance)
        .map(|(token, spender, amount)| AllowanceNeed {
            token: EvmAddress::new(token),
            spender: EvmAddress::new(spender),
            amount: ManaWei::new(amount),
        });
    let approval = call
        .as_ref()
        .and_then(|c| c.approval)
        .map(|(contract, token_id)| ApprovalNeed {
            contract: EvmAddress::new(contract),
            token_id: token_id.to_string(),
            operator: EvmAddress::new(venue),
        });
    let mut gas = GasEstimate {
        estimate: Some(static_gas(&resolved.listing)),
        source: "static",
        gas_price_wei: None,
        fee_wei: None,
        detail: None,
    };
    let mut balances = None;
    if let Some(reader) = reader {
        match reader.gas_price().await {
            Ok(price) => {
                gas.gas_price_wei = Some(price.to_string());
            }
            Err(e) => gas.detail = Some(format!("gas price: {e}")),
        }
        if let (Some(wallet), Some(call)) = (wallet, call.as_ref()) {
            match reader.estimate_gas(wallet, call.to, &call.data).await {
                Ok(estimate) => {
                    gas.estimate = Some(estimate);
                    gas.source = "rpc";
                }
                Err(ReadError::Revert(why)) => {
                    gas.detail = Some(format!(
                        "the call would revert as it stands (an allowance or balance short?): {why}"
                    ))
                }
                Err(e) => gas.detail = Some(e.to_string()),
            }
            let mana = reader.erc20_balance(chain.mana, wallet).await;
            let native = reader.native_balance(wallet).await;
            let allowed = reader.erc20_allowance(chain.mana, wallet, venue).await;
            if let (Ok(mana), Ok(native), Ok(allowed)) = (mana, native, allowed) {
                let fee = gas
                    .gas_price_wei
                    .as_deref()
                    .and_then(|p| U256::from_str_radix(p, 10).ok())
                    .and_then(|p| p.checked_mul(U256::from(gas.estimate.unwrap_or(0))))
                    .unwrap_or(U256::ZERO);
                balances = Some(Balances {
                    wallet: EvmAddress::new(wallet),
                    mana: ManaWei::new(mana),
                    native: native.to_string(),
                    allowance: ManaWei::new(allowed),
                    mana_shortfall: ManaWei::new(amount.get().saturating_sub(mana)),
                    native_shortfall: if chain.chain_id.native_gas_required() {
                        fee.saturating_sub(native).to_string()
                    } else {
                        "0".to_string()
                    },
                });
            } else {
                gas.detail
                    .get_or_insert_with(|| "the wallet's balances could not be read".to_string());
            }
        }
        if let Some(price) = gas
            .gas_price_wei
            .as_deref()
            .and_then(|p| U256::from_str_radix(p, 10).ok())
        {
            gas.fee_wei = gas.estimate.map(|g| (price * U256::from(g)).to_string());
        }
    } else {
        gas.detail = Some("no JSON-RPC endpoint is configured for the chain".to_string());
    }
    let executable = verify::executable(&checks);
    ManaExecution {
        chain_id: chain.chain_id,
        venue: EvmAddress::new(venue),
        token: EvmAddress::new(chain.mana),
        amount,
        native_gas_required: chain.chain_id.native_gas_required(),
        allowance,
        approval,
        call: call.map(|c| CallDto {
            to: EvmAddress::new(c.to),
            data: calldata_hex(&c.data),
        }),
        gas,
        balances,
        checks,
        executable,
        freshness: Freshness {
            source: resolved.listing.source(),
            ready: resolved.listing.index_ready(),
            verdict: resolved.listing.verdict(),
        },
        fingerprint: fingerprint.map(|f| format!("{f:#x}")),
    }
}

impl Resolved {
    fn offer_venue(&self) -> Option<Address> {
        match &self.listing {
            Listing::Store { venue, .. } => Some(*venue),
            Listing::Trade { venue, .. } => Some(*venue),
            Listing::LegacyOrder { order, .. } => Some(order.marketplace),
            Listing::Rental { rentals, .. } => Some(*rentals),
            Listing::Name { controller, .. } => Some(*controller),
        }
    }
}

/// One line's offer, or why there is none.
pub async fn quote_line(
    state: &SharedState,
    session: Session,
    index: usize,
    line: &super::wire::LineRequest,
    rate: Option<&ManaUsdRate>,
    now_ms: i64,
) -> OfferAnswer {
    let resolved =
        match facts::resolve(&state.pool, &state.rentals.db, &state.venues, session, line).await {
            Ok(r) => r,
            Err(e) => return OfferAnswer::Error(line_error(index, e)),
        };
    let reader = state.reader(resolved.chain.chain_id);
    let (checks, fingerprint, name_reads) = verify::checks(&resolved, reader.as_ref()).await;
    let resolved = match name_reads {
        Some((price, owner)) => facts::with_name_reads(resolved, price, owner),
        None => resolved,
    };
    let authoritative = match Authoritative::derive(resolved.facts.clone()) {
        Ok(a) => a,
        Err(refusal) => return OfferAnswer::Error(refused(index, refusal)),
    };
    let elig = eligibility(&authoritative);
    let mut quotes = Vec::new();
    let mut unavailable = Vec::new();
    let mut settlement = Vec::new();
    let mut mana_amount = None;
    for method in &elig.allowed {
        match quote(&authoritative, *method, rate) {
            Ok(q) => {
                if let Quote::Mana { amount } = q {
                    mana_amount = Some(amount);
                }
                quotes.push(q);
                if let Some(key) = settlement_key(&authoritative, *method, rate) {
                    settlement.push(SettlementFor {
                        method: *method,
                        key,
                    });
                }
            }
            Err(Refusal::Incomplete { missing }) => unavailable.push(Unavailable {
                method: *method,
                missing,
            }),
            Err(refusal) => return OfferAnswer::Error(refused(index, refusal)),
        }
    }
    let wallet = match session {
        Session::Web3 { wallet } => Some(wallet.address()),
        Session::Web2 { account } => Some(account.address()),
        Session::SignedOut => None,
    };
    let mana = match mana_amount {
        Some(amount) if elig.allows(PaymentMethod::Mana) => Some(
            mana_execution(
                &resolved,
                amount,
                wallet,
                reader.as_ref(),
                checks,
                fingerprint,
            )
            .await,
        ),
        _ => None,
    };
    OfferAnswer::Offer(Box::new(PurchaseOffer {
        asset: authoritative.asset(),
        kind: authoritative.kind(),
        sale: authoritative.sale(),
        bundle: authoritative.is_bundle(),
        eligibility: elig,
        quotes,
        unavailable,
        settlement,
        available: resolved.available,
        quoted_at: now_ms,
        mana,
    }))
}

pub async fn post_purchase_offers(
    State(state): State<SharedState>,
    OriginalUri(original): OriginalUri,
    headers: HeaderMap,
    body: axum::body::Bytes,
) -> Response {
    let request: OffersRequest = match serde_json::from_slice(&body) {
        Ok(r) => r,
        Err(e) => {
            return super::error(
                axum::http::StatusCode::BAD_REQUEST,
                "Bad Request",
                &format!("body: {e}"),
            );
        }
    };
    if request.lines.is_empty() || request.lines.len() > MAX_LINES {
        return super::error(
            axum::http::StatusCode::BAD_REQUEST,
            "Bad Request",
            &format!("lines: between 1 and {MAX_LINES}"),
        );
    }
    let signer = match state
        .optional_signer(&headers, "post", &original, ROUTE)
        .await
    {
        Ok(s) => s,
        Err(r) => return r,
    };
    let session = session_of(
        signer.as_deref(),
        crate::shared::signed_fetch::header_str(
            &headers,
            crate::shared::signed_fetch::AUTH_METADATA_HEADER,
        ),
    );
    let rate = match mana_usd_rate(&state.pool).await {
        Ok(rate) => rate,
        Err(e) => {
            tracing::error!(error = %e, "price_snapshots read failed");
            return super::error(
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                "Internal Server Error",
                "the MANA spot could not be read",
            );
        }
    };
    let now_ms = Utc::now().timestamp_millis();
    let mut offers = Vec::with_capacity(request.lines.len());
    for (index, line) in request.lines.iter().enumerate() {
        offers.push(quote_line(&state, session, index, line, rate.as_ref(), now_ms).await);
    }
    Json(json!(OffersResponse {
        quoted_at: now_ms,
        offers
    }))
    .into_response()
}

/// The states a route needs, shared with the actions and executions.
pub type Shared = Arc<super::State>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_signed_identity_is_an_account_unless_its_metadata_names_a_wallet() {
        let a = "0x00000000000000000000000000000000000000a1";
        assert_eq!(session_of(None, None), Session::SignedOut);
        assert_eq!(
            session_of(Some(a), None),
            Session::Web2 {
                account: EvmAddress::parse(a).unwrap()
            }
        );
        assert_eq!(
            session_of(Some(a), Some(r#"{"walletKind":"web2"}"#)),
            Session::Web2 {
                account: EvmAddress::parse(a).unwrap()
            }
        );
        assert_eq!(
            session_of(Some(a), Some(r#"{"walletKind":"web3"}"#)),
            Session::Web3 {
                wallet: EvmAddress::parse(a).unwrap()
            }
        );
        assert_eq!(
            session_of(Some("not an address"), Some(r#"{"walletKind":"web3"}"#)),
            Session::SignedOut
        );
    }

    #[test]
    fn the_static_budgets_cover_the_call_shapes() {
        assert!(
            static_gas(&Listing::Name {
                label: crate::types::NameLabel::parse("ab").unwrap(),
                registrar: Address::ZERO,
                controller: Address::ZERO
            }) >= 200_000
        );
    }
}
