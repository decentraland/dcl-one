// from: shop@94584a5c:app/src/lib/trade-encoding.ts (getOnChainTrade, buildAcceptCalldata), marketplace@f3956826:webapp/src/modules/order/utils.ts and webapp/src/modules/vendor/decentraland/{OrderService,BidService,NFTService}.ts (executeOrder, safeExecuteOrder, cancelOrder, cancelSignature, transfer), offchain-marketplace-contract@87350c63:src/marketplace/Marketplace.sol (accept, cancelSignature), catalyrst/crates/catalyrst-economy/src/ports/abi.rs (the shared ABI)
//! The calldata a MANA operation sends, built from authoritative records
//! only (a stored trade, a legacy order row, a rentals listing, a NAME
//! label): the off-chain marketplace's `accept` and `cancelSignature`, the
//! legacy order book's `executeOrder`, `safeExecuteOrder` and `cancelOrder`,
//! the ERC-721 `safeTransferFrom`, MANA `approve`, and the registrar
//! controller's `register`. Every call names its target and the allowance
//! it needs; nothing here reads a price from a request.

use alloy_primitives::{Address, B256, Bytes, U256, keccak256};
use alloy_sol_types::{SolCall, SolValue, sol};

use super::venues::Chain;
use crate::checkout::trade::{TradeAssetIn, TradeIn};
use crate::evm::abi::{
    approveCall, estate, executeOrderCall, offchain, registerCall, safeTransferFromCall,
};
use crate::types::NameLabel;

sol! {
    function cancelOrder(address nftAddress, uint256 assetId) external;
}

/// One transaction the buyer's (or seller's) wallet sends: the target, the
/// calldata and the MANA allowance the target needs first.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Call {
    pub to: Address,
    pub data: Bytes,
    /// `(token, spender, amount)` the wallet must have allowed before.
    pub allowance: Option<(Address, Address, U256)>,
    /// The ERC-721 the caller must have approved the target for (a transfer
    /// out of the caller's wallet through the marketplace).
    pub approval: Option<(Address, U256)>,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CallError {
    #[error("trade {0}")]
    Trade(String),
    #[error("the trade names an allowed-callers root; the Shop cannot supply its proof")]
    AllowedRoot,
    #[error("the venue {0:#x} is not an off-chain marketplace of its chain")]
    UnknownVenue(Address),
    #[error("the chain has no legacy marketplace")]
    NoLegacyMarketplace,
    #[error("the chain has no NAME controller")]
    NoNameController,
    #[error("coupon {0}")]
    Coupon(String),
}

fn bytes_of(label: &str, raw: Option<&str>) -> Result<Vec<u8>, CallError> {
    match raw.map(str::trim) {
        None | Some("") | Some("0x") => Ok(Vec::new()),
        Some(s) => hex::decode(s.strip_prefix("0x").unwrap_or(s))
            .map_err(|e| CallError::Trade(format!("{label}: bad hex: {e}"))),
    }
}

fn word(label: &str, raw: Option<&str>) -> Result<B256, CallError> {
    let bytes = bytes_of(label, raw)?;
    if bytes.len() > 32 {
        return Err(CallError::Trade(format!("{label}: longer than 32 bytes")));
    }
    let mut out = [0u8; 32];
    out[32 - bytes.len()..].copy_from_slice(&bytes);
    Ok(B256::from(out))
}

fn decimal(label: &str, raw: &str) -> Result<U256, CallError> {
    crate::types::parse_decimal(raw.trim())
        .map_err(|_| CallError::Trade(format!("{label}: not a decimal integer")))
}

fn address(label: &str, raw: &str) -> Result<Address, CallError> {
    raw.trim()
        .parse()
        .map_err(|_| CallError::Trade(format!("{label}: not an address")))
}

fn secs(ms: i64) -> U256 {
    U256::from((ms / 1000).max(0) as u64)
}

fn asset(
    label: &str,
    a: &TradeAssetIn,
    beneficiary: Address,
) -> Result<offchain::Asset, CallError> {
    let value = match a.asset_type {
        1 | 2 => a.amount.as_deref(),
        3 => a.token_id.as_deref(),
        4 => a.item_id.as_deref(),
        other => {
            return Err(CallError::Trade(format!(
                "{label}: unsupported asset type {other}"
            )));
        }
    }
    .ok_or_else(|| CallError::Trade(format!("{label}: missing its value")))?;
    Ok(offchain::Asset {
        assetType: U256::from(a.asset_type.max(0) as u64),
        contractAddress: address(label, &a.contract_address)?,
        value: decimal(label, value)?,
        beneficiary,
        extra: bytes_of(label, a.extra.as_deref())?.into(),
    })
}

fn external_checks(
    checks: &[serde_json::Value],
) -> Result<Vec<offchain::ExternalCheck>, CallError> {
    checks
        .iter()
        .map(|c| {
            let contract = c
                .get("contractAddress")
                .and_then(|v| v.as_str())
                .ok_or_else(|| CallError::Trade("externalCheck.contractAddress".into()))?;
            let selector = bytes_of(
                "externalCheck.selector",
                c.get("selector").and_then(|v| v.as_str()),
            )?;
            let selector: [u8; 4] = selector
                .get(..4)
                .and_then(|s| s.try_into().ok())
                .ok_or_else(|| CallError::Trade("externalCheck.selector".into()))?;
            Ok(offchain::ExternalCheck {
                contractAddress: address("externalCheck.contractAddress", contract)?,
                selector: selector.into(),
                value: bytes_of(
                    "externalCheck.value",
                    c.get("value").and_then(|v| v.as_str()),
                )?
                .into(),
                required: c.get("required").and_then(|v| v.as_bool()).unwrap_or(true),
            })
        })
        .collect()
}

/// The stored trade as the tuple the marketplace takes: the signed
/// `checks`, the `sent` assets delivered to `caller` (the signature does not
/// cover a sent beneficiary; the caller names itself), the `received` ones
/// as signed (the signer, or the beneficiary it named).
/// The on-chain `Checks` of a signed trade or coupon, under its own allowed root.
fn on_chain_checks(
    c: &crate::checkout::trade::TradeChecksIn,
    allowed_root: B256,
) -> Result<offchain::Checks, CallError> {
    Ok(offchain::Checks {
        uses: U256::from(c.uses),
        expiration: secs(c.expiration),
        effective: secs(c.effective),
        salt: word("checks.salt", Some(&c.salt))?,
        contractSignatureIndex: U256::from(c.contract_signature_index),
        signerSignatureIndex: U256::from(c.signer_signature_index),
        allowedRoot: allowed_root,
        allowedProof: Vec::new(),
        externalChecks: external_checks(&c.external_checks)?,
    })
}

pub fn on_chain_trade(trade: &TradeIn, caller: Address) -> Result<offchain::Trade, CallError> {
    let c = &trade.checks;
    let allowed_root = word("checks.allowedRoot", c.allowed_root.as_deref())?;
    if allowed_root != B256::ZERO {
        return Err(CallError::AllowedRoot);
    }
    let signer = address("signer", &trade.signer)?;
    let signature = bytes_of("signature", Some(&trade.signature))?;
    if signature.len() != 65 {
        return Err(CallError::Trade("the signature is not 65 bytes".into()));
    }
    let received = trade
        .received
        .iter()
        .map(|a| {
            let beneficiary = match a
                .beneficiary
                .as_deref()
                .map(str::trim)
                .filter(|b| !b.is_empty())
            {
                Some(b) => address("received.beneficiary", b)?,
                None => Address::ZERO,
            };
            asset("received", a, beneficiary)
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(offchain::Trade {
        signer,
        signature: signature.into(),
        checks: on_chain_checks(c, allowed_root)?,
        sent: trade
            .sent
            .iter()
            .map(|a| asset("sent", a, caller))
            .collect::<Result<Vec<_>, _>>()?,
        received,
    })
}

/// The MANA leg a caller pays when accepting `trade` (a listing: its
/// `received` MANA; a bid the seller accepts: none, the bidder pays), as
/// `(token, amount)`; `None` when the caller pays nothing.
pub fn mana_leg(trade: &TradeIn) -> Option<(Address, U256)> {
    trade
        .received
        .iter()
        .find(|a| a.asset_type == 1 || a.asset_type == 2)
        .and_then(|a| {
            Some((
                a.contract_address.trim().parse().ok()?,
                a.amount
                    .as_deref()
                    .and_then(|v| decimal("amount", v).ok())?,
            ))
        })
}

/// The MANA leg a bid's signer sends (the bidder pays the seller who accepts).
pub fn mana_leg_sent(trade: &TradeIn) -> Option<(Address, U256)> {
    trade
        .sent
        .iter()
        .find(|a| a.asset_type == 1 || a.asset_type == 2)
        .and_then(|a| {
            Some((
                a.contract_address.trim().parse().ok()?,
                a.amount
                    .as_deref()
                    .and_then(|v| decimal("amount", v).ok())?,
            ))
        })
}

/// `accept([trade])` on the venue the trade was signed for.
pub fn accept(
    chain: &Chain,
    venue: Address,
    trade: &TradeIn,
    caller: Address,
    allowance: Option<U256>,
) -> Result<Call, CallError> {
    if chain.marketplace(venue).is_none() {
        return Err(CallError::UnknownVenue(venue));
    }
    let on_chain = on_chain_trade(trade, caller)?;
    // A bid the seller accepts moves the seller's token through the
    // marketplace: the token is what the bidder `received`, and the caller
    // approves it; a listing moves the signer's, who approved at listing time.
    let approval = (trade.trade_type == "bid")
        .then(|| trade.received.iter().find(|a| a.asset_type == 3))
        .flatten()
        .and_then(|a| {
            Some((
                a.contract_address.trim().parse().ok()?,
                a.token_id
                    .as_deref()
                    .and_then(|t| decimal("tokenId", t).ok())?,
            ))
        });
    Ok(Call {
        to: venue,
        data: offchain::acceptCall {
            _trades: vec![on_chain],
        }
        .abi_encode()
        .into(),
        allowance: allowance.map(|amount| (chain.mana, venue, amount)),
        approval,
    })
}

/// A creator's stored coupon (`shop.coupons`), as `acceptWithCoupon` takes it.
#[derive(Debug, Clone)]
pub struct CouponIn {
    pub id: String,
    pub coupon_address: Address,
    pub discount_type: u64,
    pub discount_ppm: u32,
    pub root: String,
    pub collections: Vec<String>,
    pub signature: String,
    pub checks: crate::checkout::trade::TradeChecksIn,
}

/// `acceptWithCoupon([trade], [coupon])` on the venue the trade was signed
/// for, encoded as the Shop's UI encodes it (upstream `getOnChainCoupon`):
/// `data` is `(discountType, discount, root)`, and `callerData` wraps one
/// collection proof per sent asset, in the order of `sent`: the coupon checks
/// `proofs[i]` against `sent[i].contractAddress`. An asset whose collection the
/// coupon does not cover is refused here, with no call to fall back on.
pub fn accept_with_coupon(
    chain: &Chain,
    venue: Address,
    trade: &TradeIn,
    coupon: &CouponIn,
    caller: Address,
    allowance: Option<U256>,
) -> Result<Call, CallError> {
    let accept = accept(chain, venue, trade, caller, allowance)?;
    let on_chain = on_chain_trade(trade, caller)?;
    if on_chain.sent.is_empty() {
        return Err(CallError::Trade("no sent asset".into()));
    }
    let proofs = on_chain
        .sent
        .iter()
        .map(|asset| {
            let collection = format!("{:#x}", asset.contractAddress);
            crate::coupons::merkle::collection_proof(&coupon.collections, &collection)
                .map(|proof| proof.into_iter().map(B256::from).collect::<Vec<B256>>())
                .map_err(|e| CallError::Coupon(e.to_string()))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let signature = bytes_of("coupon.signature", Some(&coupon.signature))?;
    let coupon = offchain::Coupon {
        signature: signature.into(),
        checks: on_chain_checks(&coupon.checks, B256::ZERO)?,
        couponAddress: coupon.coupon_address,
        data: (
            U256::from(coupon.discount_type),
            U256::from(coupon.discount_ppm),
            word("coupon.root", Some(&coupon.root))?,
        )
            .abi_encode()
            .into(),
        callerData: (proofs,).abi_encode().into(),
    };
    Ok(Call {
        data: offchain::acceptWithCouponCall {
            _trades: vec![on_chain],
            _coupons: vec![coupon],
        }
        .abi_encode()
        .into(),
        ..accept
    })
}

/// `cancelSignature([trade])`: the signer revokes a listing or a bid.
pub fn cancel_signature(chain: &Chain, venue: Address, trade: &TradeIn) -> Result<Call, CallError> {
    if chain.marketplace(venue).is_none() {
        return Err(CallError::UnknownVenue(venue));
    }
    let on_chain = on_chain_trade(trade, Address::ZERO)?;
    Ok(Call {
        to: venue,
        data: offchain::cancelSignatureCall {
            _trades: vec![on_chain],
        }
        .abi_encode()
        .into(),
        allowance: None,
        approval: None,
    })
}

/// `executeOrder(nft, tokenId, price)` on the legacy order book, or
/// `safeExecuteOrder` bound to an estate's fingerprint.
pub fn execute_order(
    chain: &Chain,
    nft: Address,
    token_id: U256,
    price: U256,
    fingerprint: Option<B256>,
) -> Result<Call, CallError> {
    let venue = chain
        .legacy_marketplace
        .ok_or(CallError::NoLegacyMarketplace)?;
    let data: Vec<u8> = match fingerprint {
        Some(fp) => estate::safeExecuteOrderCall {
            nftAddress: nft,
            assetId: token_id,
            price,
            fingerprint: fp.abi_encode().into(),
        }
        .abi_encode(),
        None => executeOrderCall {
            nftAddress: nft,
            assetId: token_id,
            price,
        }
        .abi_encode(),
    };
    Ok(Call {
        to: venue,
        data: data.into(),
        allowance: Some((chain.mana, venue, price)),
        approval: None,
    })
}

/// `cancelOrder(nft, tokenId)`: the seller of a legacy order.
pub fn cancel_order(chain: &Chain, nft: Address, token_id: U256) -> Result<Call, CallError> {
    let venue = chain
        .legacy_marketplace
        .ok_or(CallError::NoLegacyMarketplace)?;
    Ok(Call {
        to: venue,
        data: cancelOrderCall {
            nftAddress: nft,
            assetId: token_id,
        }
        .abi_encode()
        .into(),
        allowance: None,
        approval: None,
    })
}

/// `safeTransferFrom(from, to, tokenId, "")` on the asset contract.
pub fn transfer(contract: Address, from: Address, to: Address, token_id: U256) -> Call {
    Call {
        to: contract,
        data: safeTransferFromCall {
            from,
            to,
            tokenId: token_id,
            data: Bytes::new(),
        }
        .abi_encode()
        .into(),
        allowance: None,
        approval: None,
    }
}

/// `approve(spender, amount)` on the chain's MANA.
pub fn approve_mana(chain: &Chain, spender: Address, amount: U256) -> Call {
    Call {
        to: chain.mana,
        data: approveCall { spender, amount }.abi_encode().into(),
        allowance: None,
        approval: None,
    }
}

/// `DCLControllerV2.register(name, beneficiary)`: a primary NAME paid in MANA.
pub fn register_name(
    chain: &Chain,
    label: &NameLabel,
    beneficiary: Address,
    price: U256,
) -> Result<Call, CallError> {
    let controller = chain.name_controller.ok_or(CallError::NoNameController)?;
    Ok(Call {
        to: controller,
        data: registerCall {
            _name: label.as_str().to_string(),
            _beneficiary: beneficiary,
        }
        .abi_encode()
        .into(),
        allowance: Some((chain.mana, controller, price)),
        approval: None,
    })
}

/// keccak256 of a trade's signature bytes: the marketplace's `Traded` topic
/// and `shop.trades.hashed_signature`.
pub fn hashed_signature(signature: &str) -> Option<B256> {
    let raw = hex::decode(signature.trim().trim_start_matches("0x")).ok()?;
    Some(keccak256(raw))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::checkout::trade::TradeChecksIn;

    fn chain() -> Chain {
        super::super::venues::Book::production(|_| None)
            .chain(crate::types::ChainId::POLYGON)
            .unwrap()
            .clone()
    }

    fn trade(kind: &str) -> TradeIn {
        let mana = TradeAssetIn {
            asset_type: 1,
            contract_address: format!("{:#x}", chain().mana),
            amount: Some("2000000000000000000".into()),
            token_id: None,
            item_id: None,
            extra: None,
            beneficiary: Some("0x00000000000000000000000000000000000000a1".into()),
        };
        let nft = TradeAssetIn {
            asset_type: 3,
            contract_address: "0x00000000000000000000000000000000000000cc".into(),
            amount: None,
            token_id: Some("7".into()),
            item_id: None,
            extra: Some("0x".into()),
            beneficiary: None,
        };
        let (sent, received) = if kind == "bid" {
            (vec![mana], vec![nft])
        } else {
            (vec![nft], vec![mana])
        };
        TradeIn {
            id: "t".into(),
            signer: "0x00000000000000000000000000000000000000a1".into(),
            signature: format!("0x{}1b", "ab".repeat(64)),
            trade_type: kind.into(),
            network: Some("MATIC".into()),
            chain_id: Some(137),
            checks: TradeChecksIn {
                uses: 1,
                expiration: 1_900_000_000_000,
                effective: 1_700_000_000_000,
                salt: "0x01".into(),
                contract_signature_index: 0,
                signer_signature_index: 0,
                allowed_root: None,
                allowed_proof: None,
                external_checks: vec![],
            },
            sent,
            received,
            contract: Some("0xe38ef22abe871513555cba89adfe45ab4f548ada".into()),
            status: None,
        }
    }

    #[test]
    fn the_cancel_selector_is_keccak_of_the_pinned_signature() {
        let tuple = "(address,bytes,(uint256,uint256,uint256,bytes32,uint256,uint256,bytes32,bytes32[],(address,bytes4,bytes,bool)[]),(uint256,address,uint256,address,bytes)[],(uint256,address,uint256,address,bytes)[])";
        assert_eq!(
            &keccak256(format!("cancelSignature({tuple}[])"))[..4],
            offchain::cancelSignatureCall::SELECTOR
        );
        assert_eq!(
            &keccak256(format!("accept({tuple}[])"))[..4],
            offchain::acceptCall::SELECTOR
        );
        assert_eq!(
            &keccak256("cancelOrder(address,uint256)")[..4],
            cancelOrderCall::SELECTOR
        );
    }

    #[test]
    fn accept_delivers_the_sent_assets_to_the_caller_and_keeps_the_signed_received_beneficiary() {
        let chain = chain();
        let venue: Address = "0xe38ef22abe871513555cba89adfe45ab4f548ada"
            .parse()
            .unwrap();
        let buyer = Address::repeat_byte(0xb0);
        let call = accept(
            &chain,
            venue,
            &trade("public_nft_order"),
            buyer,
            Some(U256::from(2u8)),
        )
        .unwrap();
        assert_eq!(call.to, venue);
        assert_eq!(call.allowance, Some((chain.mana, venue, U256::from(2u8))));
        assert_eq!(call.approval, None);
        let decoded = offchain::acceptCall::abi_decode(&call.data).unwrap();
        let t = &decoded._trades[0];
        assert_eq!(t.sent[0].beneficiary, buyer);
        assert_eq!(
            t.received[0].beneficiary,
            "0x00000000000000000000000000000000000000a1"
                .parse::<Address>()
                .unwrap()
        );
        assert_eq!(t.checks.expiration, U256::from(1_900_000_000u64));
        assert_eq!(t.checks.salt, B256::left_padding_from(&[1]));
        assert!(matches!(
            accept(
                &chain,
                Address::repeat_byte(0x99),
                &trade("public_nft_order"),
                buyer,
                None
            ),
            Err(CallError::UnknownVenue(_))
        ));
        let bid = accept(&chain, venue, &trade("bid"), buyer, None).unwrap();
        assert_eq!(
            bid.approval,
            Some((
                "0x00000000000000000000000000000000000000cc"
                    .parse()
                    .unwrap(),
                U256::from(7u8)
            )),
            "accepting a bid moves the seller's own token"
        );
        assert_eq!(mana_leg(&trade("bid")), None);
        assert_eq!(
            mana_leg(&trade("public_nft_order")).map(|(_, a)| a),
            Some(U256::from(2_000_000_000_000_000_000u128))
        );
        let mut rooted = trade("public_nft_order");
        rooted.checks.allowed_root = Some(format!("0x{}", "11".repeat(32)));
        assert_eq!(
            accept(&chain, venue, &rooted, buyer, None).unwrap_err(),
            CallError::AllowedRoot
        );
    }

    #[test]
    fn cancel_and_legacy_calls_encode_their_arguments() {
        let chain = chain();
        let venue: Address = "0xe38ef22abe871513555cba89adfe45ab4f548ada"
            .parse()
            .unwrap();
        let cancel = cancel_signature(&chain, venue, &trade("public_nft_order")).unwrap();
        assert_eq!(&cancel.data[..4], offchain::cancelSignatureCall::SELECTOR);
        let (trades,): (Vec<offchain::Trade>,) =
            <(Vec<offchain::Trade>,)>::abi_decode_params(&cancel.data[4..]).unwrap();
        assert_eq!(trades[0].sent[0].beneficiary, Address::ZERO);
        let nft = Address::repeat_byte(0xcc);
        let exec = execute_order(&chain, nft, U256::from(7u8), U256::from(5u8), None).unwrap();
        assert_eq!(exec.to, chain.legacy_marketplace.unwrap());
        assert_eq!(
            exec.allowance,
            Some((
                chain.mana,
                chain.legacy_marketplace.unwrap(),
                U256::from(5u8)
            ))
        );
        let decoded = executeOrderCall::abi_decode(&exec.data).unwrap();
        assert_eq!(
            (decoded.nftAddress, decoded.assetId, decoded.price),
            (nft, U256::from(7u8), U256::from(5u8))
        );
        let safe = execute_order(
            &chain,
            nft,
            U256::from(7u8),
            U256::from(5u8),
            Some(B256::repeat_byte(0x22)),
        )
        .unwrap();
        let decoded = estate::safeExecuteOrderCall::abi_decode(&safe.data).unwrap();
        assert_eq!(
            decoded.fingerprint.as_ref(),
            B256::repeat_byte(0x22).abi_encode().as_slice(),
            "the fingerprint travels abi-encoded, as the registry's verifyFingerprint decodes it"
        );
        let cancel = cancel_order(&chain, nft, U256::from(7u8)).unwrap();
        assert_eq!(
            cancelOrderCall::abi_decode(&cancel.data).unwrap().assetId,
            U256::from(7u8)
        );
        let t = transfer(
            nft,
            Address::repeat_byte(0x01),
            Address::repeat_byte(0x02),
            U256::from(7u8),
        );
        let decoded = safeTransferFromCall::abi_decode(&t.data).unwrap();
        assert_eq!(
            (decoded.from, decoded.to, decoded.tokenId),
            (
                Address::repeat_byte(0x01),
                Address::repeat_byte(0x02),
                U256::from(7u8)
            )
        );
        let approve = approve_mana(&chain, venue, U256::from(9u8));
        assert_eq!(
            (
                approve.to,
                approveCall::abi_decode(&approve.data).unwrap().amount
            ),
            (chain.mana, U256::from(9u8))
        );
        let eth = super::super::venues::Book::production(|_| None)
            .chain(crate::types::ChainId::ETHEREUM)
            .unwrap()
            .clone();
        let reg = register_name(
            &eth,
            &NameLabel::parse("pixelgolf").unwrap(),
            Address::repeat_byte(0x03),
            U256::from(100u8),
        )
        .unwrap();
        assert_eq!(reg.to, eth.name_controller.unwrap());
        assert_eq!(
            registerCall::abi_decode(&reg.data).unwrap()._name,
            "pixelgolf"
        );
        assert_eq!(
            hashed_signature(&format!("0x{}1b", "ab".repeat(64))),
            Some(keccak256(
                hex::decode(format!("{}1b", "ab".repeat(64))).unwrap()
            ))
        );
    }

    #[test]
    fn a_coupon_carries_each_sent_assets_own_proof_and_refuses_an_uncovered_one() {
        use crate::coupons::merkle;
        let chain = chain();
        let venue: Address = "0xe38ef22abe871513555cba89adfe45ab4f548ada"
            .parse()
            .unwrap();
        let (cc, dd, ee, ff) = (
            "0x00000000000000000000000000000000000000cc",
            "0x00000000000000000000000000000000000000dd",
            "0x00000000000000000000000000000000000000ee",
            "0x00000000000000000000000000000000000000ff",
        );
        let covered: Vec<String> = [cc, dd, ee].iter().map(|c| c.to_string()).collect();
        let root = merkle::collections_root(&covered).unwrap();
        let coupon = CouponIn {
            id: "c".into(),
            coupon_address: Address::repeat_byte(0xc0),
            discount_type: 1,
            discount_ppm: 100_000,
            root: merkle::to_hex32(root),
            collections: covered,
            signature: format!("0x{}1c", "cd".repeat(64)),
            checks: trade("public_item_order").checks,
        };
        let with_second = |collection: &str| {
            let mut t = trade("public_item_order");
            let mut second = t.sent[0].clone();
            second.contract_address = collection.into();
            t.sent.push(second);
            t
        };

        let call = accept_with_coupon(
            &chain,
            venue,
            &with_second(dd),
            &coupon,
            Address::repeat_byte(0xb0),
            None,
        )
        .unwrap();
        let decoded = offchain::acceptWithCouponCall::abi_decode(&call.data).unwrap();
        let (proofs,): (Vec<Vec<B256>>,) =
            <(Vec<Vec<B256>>,)>::abi_decode(&decoded._coupons[0].callerData).unwrap();
        assert_eq!(proofs.len(), 2, "one proof per sent asset");
        assert_ne!(proofs[0], proofs[1], "each asset proves its own collection");
        for (asset, proof) in decoded._trades[0].sent.iter().zip(&proofs) {
            let proof: Vec<[u8; 32]> = proof.iter().map(|n| n.0).collect();
            assert!(
                merkle::verify_collection_proof(
                    root,
                    &format!("{:#x}", asset.contractAddress),
                    &proof
                ),
                "the proof for {:#x} verifies against the coupon's root",
                asset.contractAddress
            );
        }

        assert!(
            matches!(
                accept_with_coupon(
                    &chain,
                    venue,
                    &with_second(ff),
                    &coupon,
                    Address::repeat_byte(0xb0),
                    None
                ),
                Err(CallError::Coupon(_))
            ),
            "an asset of a collection the coupon does not cover gets no calldata"
        );
    }

    #[test]
    fn the_tracker_prices_a_trade_by_the_coupon_its_own_calldata_carried() {
        use super::super::tracker::applied_coupon;
        use crate::evm::abi::{combined_sig, discounted_amount};
        let chain = chain();
        let venue: Address = "0xe38ef22abe871513555cba89adfe45ab4f548ada"
            .parse()
            .unwrap();
        let buyer = Address::repeat_byte(0xb0);
        let sold = trade("public_item_order");
        let reference = format!("{:#x}", hashed_signature(&sold.signature).unwrap());
        let covered = vec!["0x00000000000000000000000000000000000000cc".to_string()];
        let coupon = |address: &str, ppm: u32| CouponIn {
            id: "c".into(),
            coupon_address: address.parse().unwrap(),
            discount_type: 1,
            discount_ppm: ppm,
            root: crate::coupons::merkle::to_hex32(
                crate::coupons::merkle::collections_root(&covered).unwrap(),
            ),
            collections: covered.clone(),
            signature: format!("0x{}1c", "cd".repeat(64)),
            checks: sold.checks.clone(),
        };
        let signed = U256::from(2_000_000_000_000_000_000u128);
        let price = |data: &[u8]| -> Result<Option<U256>, String> {
            applied_coupon(137, venue, &reference, Some(venue), data)
                .map(|c| c.map(|(_, d)| discounted_amount(&d, signed).unwrap()))
        };
        let modelled = "0xc914507fe297b2dddd1232ac3a8903f1c125e794";

        // The coupon sent is the one priced, whatever the coupons look like
        // by settlement: the tracker reads no coupon state at all.
        let ten = accept_with_coupon(
            &chain,
            venue,
            &sold,
            &coupon(modelled, 100_000),
            buyer,
            None,
        )
        .unwrap();
        assert_eq!(
            price(&ten.data),
            Ok(Some(U256::from(1_800_000_000_000_000_000u128)))
        );
        let relayed = combined_sig::executeMetaTransactionCall {
            userAddress: buyer,
            functionData: ten.data.clone(),
            signature: vec![0u8; 65].into(),
        }
        .abi_encode();
        assert_eq!(
            price(&relayed),
            Ok(Some(U256::from(1_800_000_000_000_000_000u128))),
            "a relayed call is unwrapped"
        );

        // A plain accept pays the signed price, even if a coupon is live now.
        let plain = accept(&chain, venue, &sold, buyer, None).unwrap();
        assert_eq!(price(&plain.data), Ok(None));

        // What does not settle this row as the Shop prices it is refused.
        assert!(
            applied_coupon(
                137,
                venue,
                &reference,
                Some(Address::repeat_byte(0x99)),
                &ten.data
            )
            .is_err(),
            "another target"
        );
        assert!(
            applied_coupon(
                137,
                venue,
                &format!("0x{}", "00".repeat(32)),
                Some(venue),
                &ten.data
            )
            .is_err(),
            "another trade"
        );
        let unknown = accept_with_coupon(
            &chain,
            venue,
            &sold,
            &coupon("0x00000000000000000000000000000000000000c0", 100_000),
            buyer,
            None,
        )
        .unwrap();
        assert!(
            price(&unknown.data).is_err(),
            "an implementation whose data the Shop does not model"
        );
        assert!(
            price(&cancel_signature(&chain, venue, &sold).unwrap().data).is_err(),
            "not an accept"
        );
    }
}
