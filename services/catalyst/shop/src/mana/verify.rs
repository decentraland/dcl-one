// from: shop@94584a5c:app/src/lib/{dead-listings,trades}.ts (resolveLiveTrade: a listing is dead when cancelled, sold out, expired or its signer no longer holds the token), marketplace@f3956826:webapp/src/modules/{order,rental}/utils.ts (isOrderExpired, the rental nonce reads), offchain-marketplace-contract@87350c63:src/common/Verifications.sol (what the contract itself will check), rentals-contract@b51989f0:contracts/Rentals.sol (acceptListing's checks)
//! The checks a MANA purchase passes before it is quoted as executable:
//! the signature recovers to the signer over the venue's own EIP-712
//! domain, the listing is unexpired and effective, the chain has not closed
//! it (the trade-events verdict, the order fold, the rental status), the
//! seller still holds what it sells (`ownerOf` on chain), the rental nonces
//! are the contract's current ones, and a NAME is not registered. A check
//! whose source cannot answer is `unknown`, shown as freshness, never a
//! pass: the contract runs the same checks at settlement and the quote
//! says which of them the Shop could not run.

use alloy_primitives::{Address, B256, U256};
use chrono::Utc;

use super::chain::{ReadError, Reader};
use super::facts::{Listing, Resolved};
use super::venues::Chain;
use super::wire::Check;
use crate::auth::eip712::{hash_dynamic, struct_hash, typed_data_digest, word_address, word_u64};
use crate::checkout::trade::TradeIn;
use crate::market::listings;
use crate::rentals::calls as rentals_calls;
use crate::types::{ManaWei, TokenRef, TradeAssetType};

/// The EIP-712 domain of an off-chain marketplace: name, version, the
/// verifying contract and the chain id as the `salt` (the contracts'
/// modified `EIP712`, `DOMAIN_TYPE_CONTRACT_SALT`).
pub fn marketplace_domain(name: &str, version: &str, venue: Address, chain_id: u64) -> [u8; 32] {
    struct_hash(
        hash_dynamic(
            "EIP712Domain(string name,string version,address verifyingContract,bytes32 salt)"
                .as_bytes(),
        ),
        &[
            hash_dynamic(name.as_bytes()),
            hash_dynamic(version.as_bytes()),
            word_address(venue),
            word_u64(chain_id),
        ],
    )
}

/// The signing hash of a stored trade on `venue`.
pub fn trade_signing_hash(trade: &TradeIn, chain: &Chain, venue: Address) -> Result<B256, String> {
    let marketplace = chain.marketplace(venue).ok_or_else(|| {
        format!(
            "{venue:#x} is not a marketplace of chain {}",
            chain.chain_id
        )
    })?;
    let creation: listings::TradeCreation =
        serde_json::from_value(serde_json::to_value(trade).map_err(|e| e.to_string())?)
            .map_err(|e| format!("the stored trade does not re-read as a creation: {e}"))?;
    let struct_hash = listings::trade_struct_hash(&creation).map_err(|e| e.to_string())?;
    Ok(B256::from(typed_data_digest(
        marketplace_domain(
            &marketplace.name,
            &marketplace.version,
            venue,
            chain.chain_id.0,
        ),
        struct_hash.0,
    )))
}

/// Whether the stored signature recovers to the stored signer on `venue`.
pub fn trade_signature_check(trade: &TradeIn, chain: &Chain, venue: Address) -> Check {
    let digest = match trade_signing_hash(trade, chain, venue) {
        Ok(d) => d,
        Err(e) => return Check::fail("signature", e),
    };
    match crate::auth::recover::recover_address_from_digest(&digest.0, trade.signature.trim()) {
        Ok(recovered) if recovered.eq_ignore_ascii_case(trade.signer.trim()) => Check::pass(
            "signature",
            format!("recovers to {recovered} on {venue:#x}"),
        ),
        Ok(recovered) => Check::fail(
            "signature",
            format!("recovers to {recovered}, not the signer {}", trade.signer),
        ),
        Err(e) => Check::fail("signature", format!("does not recover: {e}")),
    }
}

fn now_ms() -> i64 {
    Utc::now().timestamp_millis()
}

/// The time checks a trade's `checks` carry.
pub fn trade_time_checks(trade: &TradeIn, now: i64) -> Vec<Check> {
    let mut out = Vec::new();
    out.push(if trade.checks.expiration > now {
        Check::pass(
            "notExpired",
            format!("expires at {}", trade.checks.expiration),
        )
    } else {
        Check::fail(
            "notExpired",
            format!("expired at {}", trade.checks.expiration),
        )
    });
    out.push(if trade.checks.effective <= now {
        Check::pass("effective", "")
    } else {
        Check::fail(
            "effective",
            format!("effective from {}", trade.checks.effective),
        )
    });
    out
}

/// The chain verdict of a trade as the trade-events index folded it.
pub fn trade_verdict_check(verdict: Option<&(String, i32)>, uses: u64, index_ready: bool) -> Check {
    match verdict {
        Some((status, executions)) if index_ready => match status.as_str() {
            "open" if (*executions as u64) < uses => {
                Check::pass("chainVerdict", format!("open, {executions} of {uses} uses"))
            }
            "open" => Check::fail("chainVerdict", format!("its {uses} uses are spent")),
            other => Check::fail("chainVerdict", other.to_string()),
        },
        _ => Check::unknown("chainVerdict", "the trade-events index is not published"),
    }
}

async fn owner_check(
    reader: Option<&Reader>,
    name: &'static str,
    contract: Address,
    token_id: U256,
    expected: Address,
    or_held_by: Option<Address>,
) -> Check {
    let Some(reader) = reader else {
        return Check::unknown(name, "no JSON-RPC endpoint for the chain");
    };
    match reader.owner_of(contract, token_id).await {
        Ok(Some(owner)) if owner == expected => {
            Check::pass(name, format!("{owner:#x} holds the token"))
        }
        Ok(Some(owner)) if Some(owner) == or_held_by => Check::pass(
            name,
            format!("held by the contract {owner:#x} for its lessor"),
        ),
        Ok(Some(owner)) => Check::fail(
            name,
            format!("{owner:#x} holds the token, not {expected:#x}"),
        ),
        Ok(None) => Check::fail(name, "the token does not exist"),
        Err(ReadError::Unavailable(e)) => Check::unknown(name, e),
        Err(e) => Check::fail(name, e.to_string()),
    }
}

/// The checks of one resolved line. `reader` is the line's chain, when
/// configured. Returns the checks and, for an estate, the fingerprint an
/// execution binds to; a NAME line also gets its price and registration
/// state read here (`facts::with_name_reads`).
pub async fn checks(
    resolved: &Resolved,
    reader: Option<&Reader>,
) -> (Vec<Check>, Option<B256>, Option<(ManaWei, Option<Address>)>) {
    let chain = &resolved.chain;
    let mut out = Vec::new();
    let mut fingerprint = None;
    let mut name_reads = None;
    match &resolved.listing {
        Listing::Store { .. } => out.push(match resolved.available {
            Some(n) if n > 0 => Check::pass("mintSupply", format!("{n} units in the items mirror")),
            Some(_) => Check::fail("mintSupply", "the item is sold out"),
            None => Check::unknown("mintSupply", "the items mirror has no supply value"),
        }),
        Listing::Trade {
            trade,
            venue,
            verdict,
            index_ready,
            ..
        } => {
            out.push(trade_signature_check(trade, chain, *venue));
            out.extend(trade_time_checks(trade, now_ms()));
            out.push(trade_verdict_check(
                verdict.as_ref(),
                trade.checks.uses,
                *index_ready,
            ));
            let receives_mana = trade.received.iter().any(|a| {
                (a.asset_type == 1
                    && a.contract_address.trim().parse::<Address>().ok() == Some(chain.mana))
                    || a.asset_type == 2
            });
            out.push(if trade.trade_type == "bid" || receives_mana {
                Check::pass("receivesMana", "")
            } else {
                Check::fail("receivesMana", "the trade is not paid in the chain's MANA")
            });
            if let Some(sent) = trade.sent.first() {
                let signer: Option<Address> = trade.signer.trim().parse().ok();
                match (
                    sent.asset_type,
                    sent.token_id
                        .as_deref()
                        .and_then(|t| U256::from_str_radix(t.trim(), 10).ok()),
                    sent.contract_address.trim().parse::<Address>().ok(),
                    signer,
                ) {
                    (3, Some(token_id), Some(contract), Some(signer)) => out.push(
                        owner_check(reader, "sellerOwnsToken", contract, token_id, signer, None)
                            .await,
                    ),
                    (4, ..) => match &resolved.facts.item {
                        crate::types::Lookup::Found(record) => {
                            let signer_is_creator =
                                signer.is_some_and(|s| s == record.creator.address());
                            out.push(if signer_is_creator {
                                Check::pass("signerIsCreator", "")
                            } else {
                                Check::fail(
                                    "signerIsCreator",
                                    "an item order is signed by its collection's creator",
                                )
                            });
                        }
                        _ => out.push(Check::unknown(
                            "signerIsCreator",
                            "the items mirror has no complete pass",
                        )),
                    },
                    _ => {}
                }
            }
        }
        Listing::LegacyOrder { order, .. } => {
            out.push(if order.status == "open" {
                Check::pass("open", "")
            } else {
                Check::fail("open", order.status.clone())
            });
            out.push(if order.expires_at > now_ms() {
                Check::pass("notExpired", format!("expires at {}", order.expires_at))
            } else {
                Check::fail("notExpired", format!("expired at {}", order.expires_at))
            });
            out.push(if chain.legacy_marketplace == Some(order.marketplace) {
                Check::pass("venue", format!("{:#x}", order.marketplace))
            } else {
                Check::fail(
                    "venue",
                    format!(
                        "{:#x} is not the chain's legacy marketplace",
                        order.marketplace
                    ),
                )
            });
            if let TokenRef::Token { token_id } = resolved.asset.token {
                out.push(
                    owner_check(
                        reader,
                        "sellerOwnsToken",
                        resolved.asset.contract.address(),
                        token_id,
                        order.seller,
                        None,
                    )
                    .await,
                );
                if chain.is_estate_registry(resolved.asset.contract.address()) {
                    fingerprint = estate_fingerprint(
                        reader,
                        &mut out,
                        resolved.asset.contract.address(),
                        token_id,
                    )
                    .await;
                }
            }
        }
        Listing::Rental {
            listing,
            terms,
            rentals,
            status_index_ready,
        } => {
            out.push(match listing.status.as_str() {
                "open" => Check::pass(
                    "open",
                    if *status_index_ready {
                        "open, the rentals index is published"
                    } else {
                        "open as posted; the rentals index is not published"
                    },
                ),
                other => Check::fail("open", other.to_string()),
            });
            out.push(if listing.expiration > now_ms() {
                Check::pass("notExpired", format!("expires at {}", listing.expiration))
            } else {
                Check::fail("notExpired", format!("expired at {}", listing.expiration))
            });
            out.push(Check::pass(
                "terms",
                format!(
                    "period {} for {} days",
                    terms.period_index, terms.rental_days
                ),
            ));
            let lessor: Option<Address> = listing.lessor.as_deref().and_then(|l| l.parse().ok());
            let TokenRef::Token { token_id } = resolved.asset.token else {
                unreachable!("a rental line is a token")
            };
            let contract = resolved.asset.contract.address();
            match (reader, lessor) {
                (Some(reader), Some(lessor)) => {
                    let mut current = Vec::with_capacity(3);
                    let mut failed = None;
                    for data in rentals_calls::index_calldata(contract, token_id, lessor) {
                        match reader
                            .call(*rentals, &data)
                            .await
                            .map(|out| rentals_calls::decode_uint(&out))
                        {
                            Ok(Some(v)) => current.push(v.to_string()),
                            Ok(None) => {
                                failed = Some(Check::fail(
                                    "noncesCurrent",
                                    "the Rentals contract answered no index",
                                ))
                            }
                            Err(ReadError::Unavailable(e)) => {
                                failed = Some(Check::unknown("noncesCurrent", e))
                            }
                            Err(e) => failed = Some(Check::fail("noncesCurrent", e.to_string())),
                        }
                    }
                    out.push(failed.unwrap_or_else(|| {
                        if current == listing.nonces {
                            Check::pass("noncesCurrent", format!("[{}]", current.join(", ")))
                        } else {
                            Check::fail(
                                "noncesCurrent",
                                format!(
                                    "signed [{}], the contract has [{}]",
                                    listing.nonces.join(", "),
                                    current.join(", ")
                                ),
                            )
                        }
                    }));
                    out.push(
                        owner_check(
                            Some(reader),
                            "lessorHoldsAsset",
                            contract,
                            token_id,
                            lessor,
                            Some(*rentals),
                        )
                        .await,
                    );
                    out.push(
                        match reader
                            .call(
                                *rentals,
                                &rentals_calls::is_rented_calldata(contract, token_id),
                            )
                            .await
                        {
                            Ok(data) => match rentals_calls::decode_uint(&data) {
                                Some(v) if v.is_zero() => Check::pass("notRented", ""),
                                Some(_) => Check::fail("notRented", "the asset is rented now"),
                                None => Check::unknown("notRented", "no answer"),
                            },
                            Err(ReadError::Unavailable(e)) => Check::unknown("notRented", e),
                            Err(e) => Check::fail("notRented", e.to_string()),
                        },
                    );
                    if chain.is_estate_registry(contract) {
                        fingerprint =
                            estate_fingerprint(Some(reader), &mut out, contract, token_id).await;
                    }
                }
                (None, _) => out.push(Check::unknown(
                    "noncesCurrent",
                    "no JSON-RPC endpoint for the chain",
                )),
                (_, None) => out.push(Check::fail("lessorHoldsAsset", "the listing has no lessor")),
            }
        }
        Listing::Name {
            label,
            registrar,
            controller,
        } => {
            let Some(reader) = reader else {
                out.push(Check::unknown(
                    "available",
                    "no JSON-RPC endpoint for the NAMEs chain",
                ));
                return (out, None, None);
            };
            let token_id = label.token_id();
            let owner = match reader.owner_of(*registrar, token_id).await {
                Ok(owner) => owner,
                Err(e) => {
                    out.push(Check::unknown("available", e.to_string()));
                    return (out, None, None);
                }
            };
            out.push(match owner {
                None => Check::pass("available", format!("{} is not registered", label.as_str())),
                Some(owner) => Check::fail(
                    "available",
                    format!("{} is registered to {owner:#x}", label.as_str()),
                ),
            });
            match reader.name_price(*controller).await {
                Ok(price) => {
                    out.push(Check::pass(
                        "price",
                        format!("{price} wei from the controller"),
                    ));
                    name_reads = Some((ManaWei::new(price), owner));
                }
                Err(e) => out.push(Check::unknown("price", e.to_string())),
            }
        }
    }
    (out, fingerprint, name_reads)
}

async fn estate_fingerprint(
    reader: Option<&Reader>,
    out: &mut Vec<Check>,
    estate: Address,
    estate_id: U256,
) -> Option<B256> {
    let Some(reader) = reader else {
        out.push(Check::unknown(
            "fingerprint",
            "no JSON-RPC endpoint for the chain",
        ));
        return None;
    };
    match reader.estate_fingerprint(estate, estate_id).await {
        Ok(fp) => {
            out.push(Check::pass("fingerprint", format!("{fp:#x}")));
            Some(fp)
        }
        Err(e) => {
            out.push(Check::unknown("fingerprint", e.to_string()));
            None
        }
    }
}

/// Every check that could run passed.
pub fn executable(checks: &[Check]) -> bool {
    checks.iter().all(|c| c.ok != Some(false))
}

/// The asset type of a trade's first sent asset.
pub fn sent_kind(trade: &TradeIn) -> Option<TradeAssetType> {
    trade
        .sent
        .first()
        .map(|a| TradeAssetType::from_code(u64::try_from(a.asset_type).unwrap_or(u64::MAX)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::market::listings::test_support as fixtures;

    #[test]
    fn the_domain_and_signing_hash_match_the_listings_module() {
        let chain = super::super::venues::Book::production(|_| None)
            .chain(crate::types::ChainId::POLYGON)
            .unwrap()
            .clone();
        let marketplace = listings::offchain_marketplace_v3(137).unwrap();
        let creation = fixtures::signed_trade_on(137, &marketplace);
        let expected = listings::signing_hash(&creation, &marketplace).unwrap();
        let mut json: serde_json::Value = serde_json::from_str(&fixtures::trade_json_on(
            &creation.signer,
            &creation.signature,
            137,
        ))
        .unwrap();
        json["id"] = serde_json::json!("t");
        json["contract"] = serde_json::json!(marketplace.address);
        let trade: TradeIn = serde_json::from_value(json).unwrap();
        let venue: Address = marketplace.address.parse().unwrap();
        assert_eq!(trade_signing_hash(&trade, &chain, venue).unwrap(), expected);
        assert_eq!(trade_signature_check(&trade, &chain, venue).ok, Some(true));
        let other: Address = listings::offchain_marketplace_v2(137)
            .unwrap()
            .address
            .parse()
            .unwrap();
        assert_eq!(
            trade_signature_check(&trade, &chain, other).ok,
            Some(false),
            "another venue is another domain"
        );
    }

    #[test]
    fn verdicts_and_times_read_as_the_contract_would() {
        assert_eq!(
            trade_verdict_check(Some(&("open".into(), 0)), 1, true).ok,
            Some(true)
        );
        assert_eq!(
            trade_verdict_check(Some(&("open".into(), 1)), 1, true).ok,
            Some(false)
        );
        assert_eq!(
            trade_verdict_check(Some(&("sold".into(), 1)), 1, true).ok,
            Some(false)
        );
        assert_eq!(
            trade_verdict_check(Some(&("open".into(), 0)), 1, false).ok,
            None
        );
        assert_eq!(trade_verdict_check(None, 1, true).ok, None);
        assert!(executable(&[Check::pass("a", ""), Check::unknown("b", "")]));
        assert!(!executable(&[Check::pass("a", ""), Check::fail("b", "")]));
    }
}
