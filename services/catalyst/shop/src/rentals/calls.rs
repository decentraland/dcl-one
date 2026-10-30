// from: rentals-contract@b51989f0:contracts/Rentals.sol (Listing, acceptListing, claim, bumpAssetIndex, the index getters, _rent, _handleTokenTransfers), marketplace@f3956826:webapp/src/modules/rental/{sagas,utils}.ts (listingTuple, generateECDSASignatureWithValidV, the days per period)
//! The Rentals contract calls the Shop builds, pure and offline: the terms
//! of one rent (a period of a stored listing, the days chosen inside it,
//! the MANA it costs), `acceptListing` for the tenant, `claim` and
//! `bumpAssetIndex` for the lessor, the view calls the verifier reads the
//! current indexes with, and the fee split the contract applies. Every
//! selector is checked against the pinned ABI
//! (`tests/fixtures/shop/rentals-graph@ddb426725/source/abis/Rentals.json`).

use alloy_primitives::{Address, B256, Bytes, U256};
use alloy_sol_types::{SolCall, sol};

use super::types::RentalListing;
use crate::types::ManaWei;

sol! {
    #[derive(Debug)]
    struct Listing {
        address signer;
        address contractAddress;
        uint256 tokenId;
        uint256 expiration;
        uint256[3] indexes;
        uint256[] pricePerDay;
        uint256[] maxDays;
        uint256[] minDays;
        address target;
        bytes signature;
    }

    function acceptListing(Listing _listing, address _operator, uint256 _conditionIndex, uint256 _rentalDays, bytes32 _fingerprint) external;
    function claim(address[] _contractAddresses, uint256[] _tokenIds) external;
    function bumpAssetIndex(address _contractAddress, uint256 _tokenId) external;
    function getAssetIndex(address _contractAddress, uint256 _tokenId, address _signer) external view returns (uint256);
    function getSignerIndex(address _signer) external view returns (uint256);
    function getContractIndex() external view returns (uint256);
    function getIsRented(address _contractAddress, uint256 _tokenId) external view returns (bool);
    function getRental(address _contractAddress, uint256 _tokenId) external view returns (address lessor, address tenant, uint256 endDate);
}

/// The contract's `MAX_RENTAL_DAYS` (100 years).
pub const MAX_RENTAL_DAYS: u64 = 36_525;
/// The contract's `MAX_FEE`: the fee is parts per million of the rent.
pub const FEE_DENOMINATOR: u64 = 1_000_000;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RentError {
    #[error("the listing has no lessor")]
    NoLessor,
    #[error("the listing does not carry three nonces")]
    Nonces,
    #[error("the listing has no period {0}")]
    NoPeriod(usize),
    #[error("{days} days is outside the period's {min}..={max}")]
    DaysOutOfRange { days: u64, min: u64, max: u64 },
    #[error("a rent is at most {MAX_RENTAL_DAYS} days")]
    TooManyDays,
    #[error("the listing's {0} is not a decimal integer")]
    Number(&'static str),
    #[error("the rent overflows")]
    Overflow,
    #[error("the listing's signature is not 65 bytes")]
    Signature,
    #[error("{0} is not an address")]
    Address(&'static str),
}

/// What one rent settles: the period chosen, the days inside it and the
/// MANA the tenant pays (`pricePerDay * days`, the contract's
/// `_handleTokenTransfers`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RentTerms {
    pub period_index: usize,
    pub rental_days: u64,
    pub price_per_day: U256,
    pub total: ManaWei,
}

fn decimal(text: &str, what: &'static str) -> Result<U256, RentError> {
    crate::types::parse_decimal(text).map_err(|_| RentError::Number(what))
}

fn address(text: &str, what: &'static str) -> Result<Address, RentError> {
    text.parse().map_err(|_| RentError::Address(what))
}

/// The terms of renting `listing` for its period `period_index`, `days` long
/// (the period's `maxDays` when `None`, as the UI's `pricePerRent`), with the
/// contract's own checks: the period exists, `minDays > 0`, the days lie in
/// `minDays..=maxDays` and under `MAX_RENTAL_DAYS`.
pub fn rent_terms(
    listing: &RentalListing,
    period_index: usize,
    days: Option<u64>,
) -> Result<RentTerms, RentError> {
    let period = listing
        .periods
        .get(period_index)
        .ok_or(RentError::NoPeriod(period_index))?;
    let min = u64::try_from(period.min_days).map_err(|_| RentError::Number("minDays"))?;
    let max = u64::try_from(period.max_days).map_err(|_| RentError::Number("maxDays"))?;
    let rental_days = days.unwrap_or(max);
    if min == 0 || min > max || rental_days < min || rental_days > max {
        return Err(RentError::DaysOutOfRange {
            days: rental_days,
            min,
            max,
        });
    }
    if rental_days > MAX_RENTAL_DAYS {
        return Err(RentError::TooManyDays);
    }
    let price_per_day = decimal(&period.price_per_day, "pricePerDay")?;
    let total = price_per_day
        .checked_mul(U256::from(rental_days))
        .ok_or(RentError::Overflow)?;
    Ok(RentTerms {
        period_index,
        rental_days,
        price_per_day,
        total: ManaWei::new(total),
    })
}

/// Some wallets sign with `v` 0/1; the contract recovers with 27/28 (the
/// UI's `generateECDSASignatureWithValidV`, applied again here so a stored
/// listing is sent as the contract accepts it).
pub fn canonical_signature(signature: &str) -> Result<Bytes, RentError> {
    let mut raw =
        hex::decode(signature.trim_start_matches("0x")).map_err(|_| RentError::Signature)?;
    if raw.len() != 65 {
        return Err(RentError::Signature);
    }
    if raw[64] < 27 {
        raw[64] += 27;
    }
    Ok(raw.into())
}

/// The `Listing` tuple `acceptListing` takes, from a listing as the store
/// serves it (the wire's millisecond expiration back in seconds, the nonces
/// in the signed order `[contract, signer, asset]`).
pub fn listing_tuple(listing: &RentalListing) -> Result<Listing, RentError> {
    let lessor = listing.lessor.as_deref().ok_or(RentError::NoLessor)?;
    if listing.nonces.len() != 3 {
        return Err(RentError::Nonces);
    }
    let mut indexes = [U256::ZERO; 3];
    for (slot, nonce) in indexes.iter_mut().zip(&listing.nonces) {
        *slot = decimal(nonce, "nonces")?;
    }
    let mut price_per_day = Vec::with_capacity(listing.periods.len());
    let mut max_days = Vec::with_capacity(listing.periods.len());
    let mut min_days = Vec::with_capacity(listing.periods.len());
    for period in &listing.periods {
        price_per_day.push(decimal(&period.price_per_day, "pricePerDay")?);
        max_days.push(U256::from(
            u64::try_from(period.max_days).map_err(|_| RentError::Number("maxDays"))?,
        ));
        min_days.push(U256::from(
            u64::try_from(period.min_days).map_err(|_| RentError::Number("minDays"))?,
        ));
    }
    Ok(Listing {
        signer: address(lessor, "lessor")?,
        contractAddress: address(&listing.contract_address, "contractAddress")?,
        tokenId: decimal(&listing.token_id, "tokenId")?,
        expiration: U256::from((listing.expiration / 1000).max(0) as u64),
        indexes,
        pricePerDay: price_per_day,
        maxDays: max_days,
        minDays: min_days,
        target: address(&listing.target, "target")?,
        signature: canonical_signature(&listing.signature)?,
    })
}

/// `acceptListing(listing, operator, conditionIndex, rentalDays, fingerprint)`:
/// the tenant's call. `fingerprint` is the Estate registry's current
/// `getFingerprintV2` for an estate and the zero word for a parcel.
pub fn accept_listing_calldata(
    listing: &RentalListing,
    operator: Address,
    terms: &RentTerms,
    fingerprint: B256,
) -> Result<Bytes, RentError> {
    let call = acceptListingCall {
        _listing: listing_tuple(listing)?,
        _operator: operator,
        _conditionIndex: U256::from(terms.period_index),
        _rentalDays: U256::from(terms.rental_days),
        _fingerprint: fingerprint,
    };
    Ok(call.abi_encode().into())
}

/// `claim(contracts, tokenIds)`: the lessor takes the assets back once
/// their rent is over.
pub fn claim_calldata(assets: &[(Address, U256)]) -> Bytes {
    claimCall {
        _contractAddresses: assets.iter().map(|(c, _)| *c).collect(),
        _tokenIds: assets.iter().map(|(_, t)| *t).collect(),
    }
    .abi_encode()
    .into()
}

/// `bumpAssetIndex(contract, tokenId)`: the lessor invalidates every
/// listing signed over the asset's current index (the Shop's "remove rental").
pub fn bump_asset_index_calldata(contract: Address, token_id: U256) -> Bytes {
    bumpAssetIndexCall {
        _contractAddress: contract,
        _tokenId: token_id,
    }
    .abi_encode()
    .into()
}

/// The three view calls a listing's nonces are checked against, in the
/// signed order: `[getContractIndex(), getSignerIndex(lessor), getAssetIndex(contract, token, lessor)]`.
pub fn index_calldata(contract: Address, token_id: U256, lessor: Address) -> [Bytes; 3] {
    [
        getContractIndexCall {}.abi_encode().into(),
        getSignerIndexCall { _signer: lessor }.abi_encode().into(),
        getAssetIndexCall {
            _contractAddress: contract,
            _tokenId: token_id,
            _signer: lessor,
        }
        .abi_encode()
        .into(),
    ]
}

pub fn is_rented_calldata(contract: Address, token_id: U256) -> Bytes {
    getIsRentedCall {
        _contractAddress: contract,
        _tokenId: token_id,
    }
    .abi_encode()
    .into()
}

pub fn rental_calldata(contract: Address, token_id: U256) -> Bytes {
    getRentalCall {
        _contractAddress: contract,
        _tokenId: token_id,
    }
    .abi_encode()
    .into()
}

/// A `uint256` view answer.
pub fn decode_uint(data: &[u8]) -> Option<U256> {
    (data.len() >= 32).then(|| U256::from_be_slice(&data[..32]))
}

/// `getRental`'s `(lessor, tenant, endDate)`.
pub fn decode_rental(data: &[u8]) -> Option<(Address, Address, U256)> {
    getRentalCall::abi_decode_returns(data)
        .ok()
        .map(|r| (r.lessor, r.tenant, r.endDate))
}

/// The contract's split of a rent between the lessor and the fee collector
/// (`_handleTokenTransfers`: the collector's share is floored, the lessor
/// gets the rest).
pub fn fee_split(total: U256, fee_ppm: U256) -> (U256, U256) {
    let collector = total
        .checked_mul(fee_ppm)
        .map_or(U256::ZERO, |v| v / U256::from(FEE_DENOMINATOR));
    (total - collector, collector)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rentals::types::RentalListingPeriod;
    use alloy_sol_types::SolValue;

    fn listing() -> RentalListing {
        RentalListing {
            id: "9d8ee0c3-8f50-4b56-9e8a-1e1d4f2c0a11".into(),
            nft_id: "0xf87e31492faf9a91b02ee0deaad50d51d56d5d4d-42".into(),
            category: "parcel".into(),
            search_text: "10,20".into(),
            network: "ETHEREUM".into(),
            chain_id: 1,
            expiration: 1_893_456_000_999,
            signature: format!("0x{}00", "ab".repeat(64)),
            nonces: vec!["0".into(), "1".into(), "2".into()],
            token_id: "42".into(),
            contract_address: "0xf87e31492faf9a91b02ee0deaad50d51d56d5d4d".into(),
            rental_contract_address: super::super::index::RENTALS_MAINNET.into(),
            lessor: Some("0x19e7e376e7c213b7e7e7e46cc70a5dd086daff2a".into()),
            tenant: None,
            status: "open".into(),
            created_at: 0,
            updated_at: 0,
            started_at: None,
            periods: vec![
                RentalListingPeriod {
                    min_days: 1,
                    max_days: 1,
                    price_per_day: "1000000000000000000".into(),
                },
                RentalListingPeriod {
                    min_days: 7,
                    max_days: 30,
                    price_per_day: "2500000000000000000".into(),
                },
            ],
            target: "0x0000000000000000000000000000000000000000".into(),
            rented_days: None,
        }
    }

    fn pinned_abi() -> serde_json::Value {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../tests/fixtures/shop/rentals-graph@ddb426725/source/abis/Rentals.json"
        );
        serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
    }

    fn abi_type(input: &serde_json::Value) -> String {
        let t = input["type"].as_str().unwrap();
        if let Some(rest) = t.strip_prefix("tuple") {
            let inner: Vec<String> = input["components"]
                .as_array()
                .unwrap()
                .iter()
                .map(abi_type)
                .collect();
            format!("({}){rest}", inner.join(","))
        } else {
            t.to_string()
        }
    }

    fn pinned_signatures(kind: &str) -> std::collections::HashMap<String, String> {
        pinned_abi()
            .as_array()
            .unwrap()
            .iter()
            .filter(|e| e["type"] == kind)
            .map(|e| {
                let name = e["name"].as_str().unwrap().to_string();
                let args: Vec<String> = e["inputs"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(abi_type)
                    .collect();
                (name.clone(), format!("{name}({})", args.join(",")))
            })
            .collect()
    }

    #[test]
    fn every_selector_is_the_pinned_abis() {
        let functions = pinned_signatures("function");
        let selector = |sig: &str| -> [u8; 4] {
            alloy_primitives::keccak256(sig.as_bytes())[..4]
                .try_into()
                .unwrap()
        };
        assert_eq!(
            acceptListingCall::SELECTOR,
            selector(&functions["acceptListing"])
        );
        assert_eq!(claimCall::SELECTOR, selector(&functions["claim"]));
        assert_eq!(
            bumpAssetIndexCall::SELECTOR,
            selector(&functions["bumpAssetIndex"])
        );
        assert_eq!(
            getAssetIndexCall::SELECTOR,
            selector(&functions["getAssetIndex"])
        );
        assert_eq!(
            getSignerIndexCall::SELECTOR,
            selector(&functions["getSignerIndex"])
        );
        assert_eq!(
            getContractIndexCall::SELECTOR,
            selector(&functions["getContractIndex"])
        );
        assert_eq!(
            getIsRentedCall::SELECTOR,
            selector(&functions["getIsRented"])
        );
        assert_eq!(getRentalCall::SELECTOR, selector(&functions["getRental"]));
        assert_eq!(
            functions["acceptListing"],
            "acceptListing((address,address,uint256,uint256,uint256[3],uint256[],uint256[],uint256[],address,bytes),address,uint256,uint256,bytes32)"
        );
    }

    #[test]
    fn the_terms_follow_the_contracts_checks() {
        let l = listing();
        let t = rent_terms(&l, 1, None).unwrap();
        assert_eq!((t.period_index, t.rental_days), (1, 30));
        assert_eq!(
            t.total.get(),
            U256::from(2_500_000_000_000_000_000u128) * U256::from(30u8)
        );
        assert_eq!(
            rent_terms(&l, 1, Some(7)).unwrap().total.get(),
            U256::from(2_500_000_000_000_000_000u128) * U256::from(7u8)
        );
        assert_eq!(
            rent_terms(&l, 1, Some(6)),
            Err(RentError::DaysOutOfRange {
                days: 6,
                min: 7,
                max: 30
            })
        );
        assert_eq!(
            rent_terms(&l, 1, Some(31)),
            Err(RentError::DaysOutOfRange {
                days: 31,
                min: 7,
                max: 30
            })
        );
        assert_eq!(rent_terms(&l, 2, None), Err(RentError::NoPeriod(2)));
        let mut zero = listing();
        zero.periods[0].min_days = 0;
        assert!(
            matches!(
                rent_terms(&zero, 0, None),
                Err(RentError::DaysOutOfRange { .. })
            ),
            "minDays 0 reverts on chain"
        );
        let mut long = listing();
        long.periods[0].max_days = 40_000;
        assert_eq!(
            rent_terms(&long, 0, Some(40_000)),
            Err(RentError::TooManyDays)
        );
    }

    #[test]
    fn accept_listing_calldata_carries_the_signed_listing_and_a_canonical_v() {
        let l = listing();
        let terms = rent_terms(&l, 1, Some(10)).unwrap();
        let operator = Address::repeat_byte(0x77);
        let data = accept_listing_calldata(&l, operator, &terms, B256::repeat_byte(0x11)).unwrap();
        assert_eq!(&data[..4], acceptListingCall::SELECTOR);
        let call = acceptListingCall::abi_decode(&data).unwrap();
        assert_eq!(
            call._listing.signer,
            l.lessor.as_deref().unwrap().parse::<Address>().unwrap()
        );
        assert_eq!(
            call._listing.expiration,
            U256::from(1_893_456_000u64),
            "milliseconds back to seconds, floored"
        );
        assert_eq!(
            call._listing.indexes,
            [U256::ZERO, U256::from(1u8), U256::from(2u8)]
        );
        assert_eq!(call._listing.pricePerDay.len(), 2);
        assert_eq!(
            call._listing.maxDays,
            vec![U256::from(1u8), U256::from(30u8)]
        );
        assert_eq!(call._listing.signature[64], 27, "v 0 becomes 27");
        assert_eq!(
            (call._operator, call._conditionIndex, call._rentalDays),
            (operator, U256::from(1u8), U256::from(10u8))
        );
        assert_eq!(call._fingerprint, B256::repeat_byte(0x11));
        let mut no_lessor = listing();
        no_lessor.lessor = None;
        assert_eq!(
            accept_listing_calldata(&no_lessor, operator, &terms, B256::ZERO),
            Err(RentError::NoLessor)
        );
        let mut two = listing();
        two.nonces.pop();
        assert_eq!(listing_tuple(&two).unwrap_err(), RentError::Nonces);
    }

    #[test]
    fn claim_and_bump_encode_their_arguments() {
        let land = Address::repeat_byte(0xaa);
        let data = claim_calldata(&[(land, U256::from(7u8)), (land, U256::from(9u8))]);
        let call = claimCall::abi_decode(&data).unwrap();
        assert_eq!(
            (call._contractAddresses, call._tokenIds),
            (vec![land, land], vec![U256::from(7u8), U256::from(9u8)])
        );
        let data = bump_asset_index_calldata(land, U256::from(7u8));
        let call = bumpAssetIndexCall::abi_decode(&data).unwrap();
        assert_eq!(
            (call._contractAddress, call._tokenId),
            (land, U256::from(7u8))
        );
        let [contract, signer, asset] =
            index_calldata(land, U256::from(7u8), Address::repeat_byte(0x01));
        assert_eq!(&contract[..4], getContractIndexCall::SELECTOR);
        assert_eq!(&signer[..4], getSignerIndexCall::SELECTOR);
        assert_eq!(&asset[..4], getAssetIndexCall::SELECTOR);
        assert_eq!(
            decode_uint(&U256::from(5u8).abi_encode()),
            Some(U256::from(5u8))
        );
        let encoded = (
            Address::repeat_byte(0x01),
            Address::repeat_byte(0x02),
            U256::from(99u8),
        )
            .abi_encode_params();
        assert_eq!(
            decode_rental(&encoded),
            Some((
                Address::repeat_byte(0x01),
                Address::repeat_byte(0x02),
                U256::from(99u8)
            ))
        );
    }

    #[test]
    fn the_fee_split_floors_the_collectors_share() {
        let total = U256::from(1_000_000_000_000_000_003u128);
        let (lessor, collector) = fee_split(total, U256::from(25_000u32));
        assert_eq!(collector, U256::from(25_000_000_000_000_000u128));
        assert_eq!(lessor + collector, total);
        assert_eq!(fee_split(total, U256::ZERO), (total, U256::ZERO));
    }
}
