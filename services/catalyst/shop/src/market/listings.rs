//! `POST /v1/trades`: a wallet-signed off-chain listing or bid accepted as
//! local authority (`shop.trades`, `mirrored_from` NULL) behind
//! `DATA_WRITES_ENABLED`. The old policy, except that only the current
//! marketplace accepts new trades: the signed-fetch chain must carry
//! `signer` dcl:marketplace|dcl:builder and intent `dcl:create-trade`, the
//! recovered address must be the trade's signer, the EIP-712 signature must
//! verify against the chain's current off-chain marketplace (v3; one that
//! verifies only on v2 is a 400 naming v3), an erc721 listing's token must
//! be owned by the signer per `ownerOf` on `TRADE_RPC_URLS` (never the mirror: a
//! stale index is not a reason to accept a listing), and a duplicate
//! signature is a 409. As marketplace-server ae872a4a
//! (`src/ports/trades/utils.ts`) does, each side carries exactly one asset,
//! the price is the chain's MANA or USD-pegged MANA, an Estate asset carries
//! the registry's current fingerprint (`getFingerprintV2`), and a second open
//! order for one NFT or item, or a second open bid by one bidder on one
//! asset, is a 409; every amount, token id and item id is canonical decimal
//! before it is stored. The RPC calls go through the ONE outbound client
//! and its SSRF guard. A V3 trade stores its cancellation digest, the
//! EIP-712 signing hash the signature was verified over (`trade_digest`, as
//! marketplace-server stores it); the `trade_events` fold verifies it and
//! keys the trade's V3 cancellation on it (`trade_chain_state.digest`); `GET
//! /v1/trades/{hash}/accept` is served there. `GET /v1/activity` (a
//! wallet's sales, bids and orders across the squid tables) answers the
//! envelope.

use alloy_primitives::{Address, B256, U256, keccak256};
use axum::Json;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use catalyst_shared::json_number::{integral_i64, integral_u64};
use chrono::{DateTime, TimeZone, Utc};
use serde::Deserialize;
use serde_json::json;
use sqlx::PgPool;
use std::str::FromStr;

use super::signed::{
    CREATE_TRADE_INTENT, MARKETPLACE_AUTH_SIGNERS, require_auth_metadata, require_signer,
};
use super::wire::ApiError;
use super::{MarketplaceState, SharedState};
use crate::shared::limits::{RateLimitDecision, RateLimiter};

use crate::types::{
    ASSET_TYPE_COLLECTION_ITEM, ASSET_TYPE_ERC20, ASSET_TYPE_ERC721, ASSET_TYPE_USD_PEGGED_MANA,
};

pub const TRADE_TYPE_BID: &str = "bid";
pub const TRADE_TYPE_PUBLIC_NFT_ORDER: &str = "public_nft_order";
pub const TRADE_TYPE_PUBLIC_ITEM_ORDER: &str = "public_item_order";

/// `TRADE_RPC_URLS`: `137=https://polygon.example,1=https://eth.example`.
#[derive(Debug, Clone, Default)]
pub struct RpcEndpoints {
    endpoints: Vec<(i64, String)>,
}

impl RpcEndpoints {
    pub fn from_env(raw: Option<&str>) -> Self {
        let endpoints = raw
            .unwrap_or("")
            .split(',')
            .filter_map(|entry| {
                let (chain, url) = entry.split_once('=')?;
                let chain = chain.trim().parse::<i64>().ok()?;
                let url = url.trim();
                if url.is_empty() {
                    return None;
                }
                Some((chain, url.to_string()))
            })
            .collect();
        RpcEndpoints { endpoints }
    }

    pub fn for_chain(&self, chain_id: i64) -> Option<&str> {
        self.endpoints
            .iter()
            .find(|(chain, _)| *chain == chain_id)
            .map(|(_, url)| url.as_str())
    }

    pub fn is_empty(&self) -> bool {
        self.endpoints.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = (i64, &str)> {
        self.endpoints.iter().map(|(c, u)| (*c, u.as_str()))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OffChainMarketplace {
    /// The deployment's contract name, as error messages name it.
    pub contract: &'static str,
    pub name: &'static str,
    pub version: &'static str,
    pub address: &'static str,
    /// V3 keys a cancellation on the trade's EIP-712 digest; V1/V2 on keccak256 of the signature.
    pub cancels_by_digest: bool,
}

pub const ETHEREUM_MAINNET: i64 = 1;
pub const ETHEREUM_SEPOLIA: i64 = 11155111;
pub const MATIC_MAINNET: i64 = 137;
pub const MATIC_AMOY: i64 = 80002;

pub fn offchain_marketplace_v2(chain_id: i64) -> Option<OffChainMarketplace> {
    match chain_id {
        ETHEREUM_MAINNET | ETHEREUM_SEPOLIA => Some(OffChainMarketplace {
            contract: "OffChainMarketplaceV2",
            name: "DecentralandMarketplaceEthereum",
            version: "1.0.0",
            address: "0x1b67d0e31eeb6b52d8eeed71d3616c2f5b33b8e7",
            cancels_by_digest: false,
        }),
        MATIC_MAINNET => Some(OffChainMarketplace {
            contract: "OffChainMarketplaceV2",
            name: "DecentralandMarketplacePolygon",
            version: "1.0.0",
            address: "0xa40b1d129b8906888720686f3a01921ddf37716f",
            cancels_by_digest: false,
        }),
        MATIC_AMOY => Some(OffChainMarketplace {
            contract: "OffChainMarketplaceV2",
            name: "DecentralandMarketplacePolygon",
            version: "1.0.0",
            address: "0x1b67d0e31eeb6b52d8eeed71d3616c2f5b33b8e7",
            cancels_by_digest: false,
        }),
        _ => None,
    }
}

pub fn offchain_marketplace_v3(chain_id: i64) -> Option<OffChainMarketplace> {
    match chain_id {
        ETHEREUM_MAINNET => Some(OffChainMarketplace {
            contract: "OffChainMarketplaceV3",
            name: "DecentralandMarketplaceEthereum",
            version: "1.0.0",
            address: "0x0f11d0d1671519683bd48abf3dbe779e300941cd",
            cancels_by_digest: true,
        }),
        MATIC_MAINNET => Some(OffChainMarketplace {
            contract: "OffChainMarketplaceV3",
            name: "DecentralandMarketplacePolygon",
            version: "1.0.0",
            address: "0xe38ef22abe871513555cba89adfe45ab4f548ada",
            cancels_by_digest: true,
        }),
        ETHEREUM_SEPOLIA => Some(OffChainMarketplace {
            contract: "OffChainMarketplaceV3",
            name: "DecentralandMarketplaceEthereum",
            version: "1.0.0",
            address: "0x257db44ac97789c16ab277eae87dcde0c246cc9f",
            cancels_by_digest: true,
        }),
        MATIC_AMOY => Some(OffChainMarketplace {
            contract: "OffChainMarketplaceV3",
            name: "DecentralandMarketplacePolygon",
            version: "1.0.0",
            address: "0x36fd1434a6c4b8ade80c9847c1d15033ce34488c",
            cancels_by_digest: true,
        }),
        _ => None,
    }
}

/// Newest first: an ambiguous signature settles on the newest deployment.
pub fn offchain_marketplaces(chain_id: i64) -> Vec<OffChainMarketplace> {
    [
        offchain_marketplace_v3(chain_id),
        offchain_marketplace_v2(chain_id),
    ]
    .into_iter()
    .flatten()
    .collect()
}

pub fn network_for_chain(chain_id: i64) -> Option<&'static str> {
    match chain_id {
        ETHEREUM_MAINNET | ETHEREUM_SEPOLIA => Some("ETHEREUM"),
        MATIC_MAINNET | MATIC_AMOY => Some("MATIC"),
        _ => None,
    }
}

pub fn is_estate_chain(chain_id: i64) -> bool {
    matches!(chain_id, ETHEREUM_MAINNET | ETHEREUM_SEPOLIA)
}

/// The chain's MANA token, the one ERC-20 a trade may be priced in
/// (decentraland-transactions 3.3.0 `MANAToken`, which marketplace-server
/// ae872a4a `isValidPriceAsset` reads).
pub fn mana_token(chain_id: i64) -> Option<&'static str> {
    match chain_id {
        ETHEREUM_MAINNET => Some("0x0f5d2fb29fb7d3cfee444a200298f468908cc942"),
        ETHEREUM_SEPOLIA => Some("0xfa04d2e2ba9aec166c93dfeeba7427b2303befa9"),
        MATIC_MAINNET => Some("0xa1c57f48f0deb89f569dfbe6e2b7f46d33606fd4"),
        MATIC_AMOY => Some("0x7ad72b9f944ea9793cf4055d88f81138cc2c63a0"),
        _ => None,
    }
}

/// The chain's Estate registry (marketplace-server ae872a4a
/// `getMarketplaceContracts`, the `Estates` entry).
pub fn estate_registry(chain_id: i64) -> Option<&'static str> {
    match chain_id {
        ETHEREUM_MAINNET => Some(crate::land_authz::ESTATE_REGISTRY_MAINNET),
        ETHEREUM_SEPOLIA => Some(crate::land_authz::ESTATE_REGISTRY_SEPOLIA),
        _ => None,
    }
}

/// A uint256 in canonical decimal: digits only, no sign, no prefix, no
/// separator and no leading zero. The readers compare and cast the stored
/// text, so any other spelling of a number would either break the cast
/// (`""`) or never match a filter (`"0x2a"`, `"042"`).
pub fn is_canonical_uint(value: &str) -> bool {
    crate::types::parse_decimal(value).is_ok() && (value == "0" || !value.starts_with('0'))
}

#[derive(Debug, Deserialize)]
pub struct ExternalCheckInput {
    #[serde(rename = "contractAddress")]
    pub contract_address: String,
    pub selector: String,
    #[serde(default)]
    pub value: Option<String>,
    #[serde(default)]
    pub required: bool,
}

#[derive(Debug, Deserialize)]
pub struct TradeChecksInput {
    #[serde(deserialize_with = "integral_u64")]
    pub uses: u64,
    #[serde(deserialize_with = "integral_i64")]
    pub expiration: i64,
    #[serde(deserialize_with = "integral_i64")]
    pub effective: i64,
    pub salt: String,
    #[serde(rename = "contractSignatureIndex", deserialize_with = "integral_u64")]
    pub contract_signature_index: u64,
    #[serde(rename = "signerSignatureIndex", deserialize_with = "integral_u64")]
    pub signer_signature_index: u64,
    #[serde(rename = "allowedRoot")]
    pub allowed_root: String,
    #[serde(rename = "allowedProof", default)]
    pub allowed_proof: Option<Vec<String>>,
    #[serde(rename = "externalChecks", default)]
    pub external_checks: Option<Vec<ExternalCheckInput>>,
}

#[derive(Debug, Deserialize)]
pub struct TradeAssetInput {
    #[serde(rename = "assetType")]
    pub asset_type: i32,
    #[serde(rename = "contractAddress")]
    pub contract_address: String,
    #[serde(default)]
    pub extra: Option<String>,
    #[serde(default)]
    pub beneficiary: Option<String>,
    #[serde(default)]
    pub amount: Option<String>,
    #[serde(rename = "tokenId", default)]
    pub token_id: Option<String>,
    #[serde(rename = "itemId", default)]
    pub item_id: Option<String>,
}

impl TradeAssetInput {
    pub fn signed_value(&self) -> Result<String, SignatureError> {
        let value = match self.asset_type {
            ASSET_TYPE_ERC20 | ASSET_TYPE_USD_PEGGED_MANA => self.amount.as_ref(),
            ASSET_TYPE_ERC721 => self.token_id.as_ref(),
            ASSET_TYPE_COLLECTION_ITEM => self.item_id.as_ref(),
            other => {
                return Err(SignatureError::Malformed(format!(
                    "unsupported asset type {other}"
                )));
            }
        };
        value.cloned().ok_or_else(|| {
            SignatureError::Malformed(format!(
                "asset type {} is missing its value field",
                self.asset_type
            ))
        })
    }
    fn is_erc721(&self) -> bool {
        self.asset_type == ASSET_TYPE_ERC721
    }
    fn is_item(&self) -> bool {
        self.asset_type == ASSET_TYPE_COLLECTION_ITEM
    }
    fn is_fungible(&self) -> bool {
        matches!(
            self.asset_type,
            ASSET_TYPE_ERC20 | ASSET_TYPE_USD_PEGGED_MANA
        )
    }
    /// Settles in MANA: USD-pegged MANA always does, an ERC-20 only when it
    /// is the chain's MANA token (marketplace-server ae872a4a `isValidPriceAsset`).
    fn is_mana_price(&self, chain_id: i64) -> bool {
        match self.asset_type {
            ASSET_TYPE_USD_PEGGED_MANA => true,
            ASSET_TYPE_ERC20 => mana_token(chain_id)
                .is_some_and(|mana| mana.eq_ignore_ascii_case(self.contract_address.trim())),
            _ => false,
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct TradeCreation {
    pub signer: String,
    pub signature: String,
    #[serde(rename = "type")]
    pub trade_type: String,
    pub network: String,
    #[serde(rename = "chainId", deserialize_with = "integral_i64")]
    pub chain_id: i64,
    pub checks: TradeChecksInput,
    pub sent: Vec<TradeAssetInput>,
    pub received: Vec<TradeAssetInput>,
}

const TRADE_TYPE: &str = concat!(
    "Trade(Checks checks,AssetWithoutBeneficiary[] sent,Asset[] received)",
    "Asset(uint256 assetType,address contractAddress,uint256 value,bytes extra,address beneficiary)",
    "AssetWithoutBeneficiary(uint256 assetType,address contractAddress,uint256 value,bytes extra)",
    "Checks(uint256 uses,uint256 expiration,uint256 effective,bytes32 salt,uint256 contractSignatureIndex,uint256 signerSignatureIndex,bytes32 allowedRoot,ExternalCheck[] externalChecks)",
    "ExternalCheck(address contractAddress,bytes4 selector,bytes value,bool required)",
);
const EXTERNAL_CHECK_TYPE: &str =
    "ExternalCheck(address contractAddress,bytes4 selector,bytes value,bool required)";
/// `Checks` carries the referenced `ExternalCheck` even when the array is empty.
pub(crate) const CHECKS_TYPE: &str = concat!(
    "Checks(uint256 uses,uint256 expiration,uint256 effective,bytes32 salt,uint256 contractSignatureIndex,uint256 signerSignatureIndex,bytes32 allowedRoot,ExternalCheck[] externalChecks)",
    "ExternalCheck(address contractAddress,bytes4 selector,bytes value,bool required)",
);
const ASSET_TYPE: &str = "Asset(uint256 assetType,address contractAddress,uint256 value,bytes extra,address beneficiary)";
const ASSET_WITHOUT_BENEFICIARY_TYPE: &str =
    "AssetWithoutBeneficiary(uint256 assetType,address contractAddress,uint256 value,bytes extra)";
pub(crate) const DOMAIN_TYPE_CONTRACT_SALT: &str =
    "EIP712Domain(string name,string version,address verifyingContract,bytes32 salt)";

/// The word packing and digest math (one copy, in the auth leaf crate).
pub use crate::auth::eip712;

use eip712::{
    hash_array_of_structs, hash_dynamic, struct_hash, typed_data_digest, word_address, word_u64,
    word_u256,
};

#[derive(Debug, PartialEq, Eq)]
pub enum SignatureError {
    Malformed(String),
    Mismatch { recovered: String, expected: String },
}

impl std::fmt::Display for SignatureError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SignatureError::Malformed(why) => write!(f, "invalid trade signature: {why}"),
            SignatureError::Mismatch {
                recovered,
                expected,
            } => write!(
                f,
                "trade signature recovers to {recovered}, not the signer {expected}"
            ),
        }
    }
}

pub(crate) fn hex_bytes(value: &str) -> Result<Vec<u8>, SignatureError> {
    let trimmed = value.strip_prefix("0x").unwrap_or(value);
    if trimmed.is_empty() {
        return Ok(Vec::new());
    }
    hex::decode(trimmed).map_err(|e| SignatureError::Malformed(format!("bad hex {value}: {e}")))
}

fn left_padded_32(value: &str) -> Result<[u8; 32], SignatureError> {
    let raw = hex_bytes(value)?;
    if raw.len() > 32 {
        return Err(SignatureError::Malformed(format!(
            "value wider than 32 bytes: {value}"
        )));
    }
    let mut out = [0u8; 32];
    out[32 - raw.len()..].copy_from_slice(&raw);
    Ok(out)
}

fn right_padded_32(value: &str) -> Result<[u8; 32], SignatureError> {
    let raw = hex_bytes(value)?;
    if raw.len() > 32 {
        return Err(SignatureError::Malformed(format!(
            "value wider than 32 bytes: {value}"
        )));
    }
    let mut out = [0u8; 32];
    out[..raw.len()].copy_from_slice(&raw);
    Ok(out)
}

pub(crate) fn parse_address(value: &str) -> Result<Address, SignatureError> {
    Address::from_str(value)
        .map_err(|e| SignatureError::Malformed(format!("bad address {value}: {e}")))
}

fn address_word(value: &str) -> Result<[u8; 32], SignatureError> {
    Ok(word_address(parse_address(value)?))
}

fn uint_word(value: &str) -> Result<[u8; 32], SignatureError> {
    Ok(word_u256(U256::from_str(value).map_err(|e| {
        SignatureError::Malformed(format!("bad uint {value}: {e}"))
    })?))
}

fn to_seconds(ms: i64) -> u64 {
    (ms / 1000).max(0) as u64
}

fn hash_external_checks(checks: &TradeChecksInput) -> Result<[u8; 32], SignatureError> {
    let type_hash = hash_dynamic(EXTERNAL_CHECK_TYPE.as_bytes());
    let mut members = Vec::new();
    for check in checks.external_checks.iter().flatten() {
        let value = check.value.clone().unwrap_or_else(|| "0x".to_string());
        members.push(struct_hash(
            type_hash,
            &[
                address_word(&check.contract_address)?,
                right_padded_32(&check.selector)?,
                hash_dynamic(&hex_bytes(&value)?),
                word_u64(u64::from(check.required)),
            ],
        ));
    }
    Ok(hash_array_of_structs(&members))
}

pub(crate) fn hash_checks(checks: &TradeChecksInput) -> Result<[u8; 32], SignatureError> {
    Ok(struct_hash(
        hash_dynamic(CHECKS_TYPE.as_bytes()),
        &[
            word_u64(checks.uses),
            word_u64(to_seconds(checks.expiration)),
            word_u64(to_seconds(checks.effective)),
            left_padded_32(&checks.salt)?,
            word_u64(checks.contract_signature_index),
            word_u64(checks.signer_signature_index),
            left_padded_32(&checks.allowed_root)?,
            hash_external_checks(checks)?,
        ],
    ))
}

fn hash_assets(
    assets: &[TradeAssetInput],
    with_beneficiary: bool,
) -> Result<[u8; 32], SignatureError> {
    let type_hash = hash_dynamic(if with_beneficiary {
        ASSET_TYPE.as_bytes()
    } else {
        ASSET_WITHOUT_BENEFICIARY_TYPE.as_bytes()
    });
    let mut members = Vec::with_capacity(assets.len());
    for asset in assets {
        let mut fields = vec![
            word_u64(asset.asset_type as u64),
            address_word(&asset.contract_address)?,
            uint_word(&asset.signed_value()?)?,
            hash_dynamic(&hex_bytes(asset.extra.as_deref().unwrap_or("0x"))?),
        ];
        if with_beneficiary {
            let beneficiary = asset
                .beneficiary
                .clone()
                .unwrap_or_else(|| format!("{:?}", Address::ZERO));
            fields.push(address_word(&beneficiary)?);
        }
        members.push(struct_hash(type_hash, &fields));
    }
    Ok(hash_array_of_structs(&members))
}

fn trade_domain_separator(
    marketplace: &OffChainMarketplace,
    chain_id: i64,
) -> Result<[u8; 32], SignatureError> {
    Ok(struct_hash(
        hash_dynamic(DOMAIN_TYPE_CONTRACT_SALT.as_bytes()),
        &[
            hash_dynamic(marketplace.name.as_bytes()),
            hash_dynamic(marketplace.version.as_bytes()),
            word_address(parse_address(marketplace.address)?),
            word_u64(chain_id.max(0) as u64),
        ],
    ))
}

pub fn trade_struct_hash(trade: &TradeCreation) -> Result<B256, SignatureError> {
    Ok(B256::from(struct_hash(
        hash_dynamic(TRADE_TYPE.as_bytes()),
        &[
            hash_checks(&trade.checks)?,
            hash_assets(&trade.sent, false)?,
            hash_assets(&trade.received, true)?,
        ],
    )))
}

pub fn signing_hash(
    trade: &TradeCreation,
    marketplace: &OffChainMarketplace,
) -> Result<B256, SignatureError> {
    Ok(B256::from(typed_data_digest(
        trade_domain_separator(marketplace, trade.chain_id)?,
        trade_struct_hash(trade)?.0,
    )))
}

pub fn recover_signer(
    trade: &TradeCreation,
    marketplace: &OffChainMarketplace,
) -> Result<Address, SignatureError> {
    let raw = hex_bytes(&trade.signature)?;
    if raw.len() != 65 {
        return Err(SignatureError::Malformed(format!(
            "expected a 65-byte signature, got {}",
            raw.len()
        )));
    }
    if !matches!(raw[64], 0 | 1 | 27 | 28) {
        return Err(SignatureError::Malformed(format!(
            "unsupported signature v byte {}",
            raw[64]
        )));
    }
    let hash = signing_hash(trade, marketplace)?;
    let recovered = crate::auth::recover::recover_address_from_digest(
        &hash.0,
        &format!("0x{}", hex::encode(&raw)),
    )
    .map_err(|e| SignatureError::Malformed(format!("unrecoverable signature: {e}")))?;
    Address::from_str(&recovered)
        .map_err(|e| SignatureError::Malformed(format!("unrecoverable signature: {e}")))
}

pub fn verify_signature(
    trade: &TradeCreation,
    marketplace: &OffChainMarketplace,
) -> Result<(), SignatureError> {
    let recovered = recover_signer(trade, marketplace)?;
    let expected = Address::from_str(&trade.signer)
        .map_err(|e| SignatureError::Malformed(format!("bad signer {}: {e}", trade.signer)))?;
    if recovered == expected {
        Ok(())
    } else {
        Err(SignatureError::Mismatch {
            recovered: recovered.to_checksum(None),
            expected: expected.to_checksum(None),
        })
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct SignatureMatch {
    pub marketplace: OffChainMarketplace,
    pub cancellation_digest: Option<String>,
}

/// A structurally invalid signature (length, v byte, a high s) is Malformed
/// on the first candidate; one that verifies nowhere reports the newest mismatch.
pub fn resolve_signature(
    trade: &TradeCreation,
    candidates: &[OffChainMarketplace],
) -> Result<SignatureMatch, SignatureError> {
    let mut first_mismatch: Option<SignatureError> = None;
    for marketplace in candidates {
        match verify_signature(trade, marketplace) {
            Ok(()) => {
                let cancellation_digest = if marketplace.cancels_by_digest {
                    Some(format!("0x{:x}", signing_hash(trade, marketplace)?))
                } else {
                    None
                };
                return Ok(SignatureMatch {
                    marketplace: *marketplace,
                    cancellation_digest,
                });
            }
            Err(malformed @ SignatureError::Malformed(_)) => return Err(malformed),
            Err(mismatch) => {
                first_mismatch.get_or_insert(mismatch);
            }
        }
    }
    Err(first_mismatch.unwrap_or_else(|| {
        SignatureError::Malformed("no off-chain marketplace to verify against".to_string())
    }))
}

#[derive(Debug)]
pub enum OwnershipError {
    NotConfigured(i64),
    Rpc(String),
    NoOwner(String),
}

impl std::fmt::Display for OwnershipError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OwnershipError::NotConfigured(chain) => write!(
                f,
                "ownership cannot be verified: no rpc endpoint is configured for chain {chain}"
            ),
            OwnershipError::Rpc(why) => write!(f, "ownership lookup failed: {why}"),
            OwnershipError::NoOwner(token) => write!(f, "token {token} has no owner on chain"),
        }
    }
}

const OWNER_OF_SELECTOR: &str = "6352211e";

fn owner_of_calldata(token_id: &str) -> Result<String, OwnershipError> {
    let parsed = U256::from_str(token_id)
        .map_err(|e| OwnershipError::Rpc(format!("bad token id {token_id}: {e}")))?;
    Ok(format!(
        "0x{OWNER_OF_SELECTOR}{}",
        hex::encode(parsed.to_be_bytes::<32>())
    ))
}

fn address_from_word(word: &str) -> Result<Address, OwnershipError> {
    let trimmed = word.strip_prefix("0x").unwrap_or(word);
    if trimmed.len() < 40 {
        return Err(OwnershipError::NoOwner(word.to_string()));
    }
    let tail = &trimmed[trimmed.len() - 40..];
    Address::from_str(&format!("0x{tail}"))
        .map_err(|e| OwnershipError::Rpc(format!("undecodable owner word {word}: {e}")))
}

/// `ownerOf(tokenId)` on the chain's endpoint through the guarded client.
pub async fn owner_of(
    state: &MarketplaceState,
    chain_id: i64,
    contract_address: &str,
    token_id: &str,
) -> Result<Address, OwnershipError> {
    let url = state
        .settings
        .market
        .trade_rpc_urls
        .for_chain(chain_id)
        .ok_or(OwnershipError::NotConfigured(chain_id))?;
    if !state.upstream_allowed(url) {
        return Err(OwnershipError::Rpc(
            "rpc endpoint is not a publicly routable https url".to_string(),
        ));
    }
    let body = json!({ "jsonrpc": "2.0", "id": 1, "method": "eth_call", "params": [{ "to": contract_address, "data": owner_of_calldata(token_id)? }, "latest"] });
    let resp = state
        .http
        .post(url)
        .timeout(std::time::Duration::from_secs(15))
        .json(&body)
        .send()
        .await
        .map_err(|e| OwnershipError::Rpc(e.to_string()))?;
    if !resp.status().is_success() {
        return Err(OwnershipError::Rpc(format!(
            "rpc status {}",
            resp.status().as_u16()
        )));
    }
    let bytes = crate::http::read_body_capped(resp, 64 * 1024)
        .await
        .map_err(|e| OwnershipError::Rpc(e.to_string()))?;
    let payload: serde_json::Value = serde_json::from_slice(&bytes)
        .map_err(|e| OwnershipError::Rpc(format!("unreadable rpc response: {e}")))?;
    if let Some(error) = payload.get("error") {
        return Err(OwnershipError::Rpc(error.to_string()));
    }
    let result = payload
        .get("result")
        .and_then(|r| r.as_str())
        .ok_or_else(|| OwnershipError::Rpc("rpc response carried no result".to_string()))?;
    let owner = address_from_word(result)?;
    if owner == Address::ZERO {
        return Err(OwnershipError::NoOwner(token_id.to_string()));
    }
    Ok(owner)
}

#[derive(Debug)]
pub enum TradeCreationError {
    Expired,
    EffectiveAfterExpiration,
    SignerMismatch,
    UnsupportedChain(i64),
    NetworkMismatch {
        expected: String,
        got: String,
    },
    InvalidStructure(String),
    InvalidSignature(String),
    /// Signed against an off-chain marketplace other than the chain's current one.
    RetiredMarketplace {
        signed_on: &'static str,
        current: &'static str,
    },
    OwnershipUnverifiable(i64),
    OwnershipLookupFailed(String),
    NotTheOwner {
        owner: String,
    },
    /// A chain this node does not serve for the trade's network.
    WrongChain {
        expected: i64,
        got: i64,
    },
    /// The item is not in the mirrored catalogue (yet).
    ItemUnknown,
    NotTheCreator,
    /// The price is not the chain's MANA (nor USD-pegged MANA).
    InvalidPriceAsset,
    /// An Estate asset whose `extra` is not the registry's current fingerprint.
    InvalidEstateTrade,
    /// The same signature is already stored.
    Duplicate,
    /// An open trade already covers this bid or order (marketplace-server
    /// ae872a4a `validateTradeByType`); the message is upstream's.
    DuplicateOpen(&'static str),
    Db(sqlx::Error),
}

impl std::fmt::Display for TradeCreationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Expired => write!(f, "the trade has already expired"),
            Self::EffectiveAfterExpiration => {
                write!(f, "the trade becomes effective after it expires")
            }
            Self::SignerMismatch => write!(f, "the authenticated address is not the trade signer"),
            Self::UnsupportedChain(chain) => {
                write!(f, "no off-chain marketplace is deployed on chain {chain}")
            }
            Self::NetworkMismatch { expected, got } => {
                write!(f, "chain belongs to network {expected}, not {got}")
            }
            Self::InvalidStructure(why) => write!(f, "invalid trade structure: {why}"),
            Self::InvalidSignature(why) => write!(f, "{why}"),
            Self::RetiredMarketplace { signed_on, current } => write!(
                f,
                "Trades signed on {signed_on} are no longer accepted, sign it on {current}"
            ),
            Self::OwnershipUnverifiable(chain) => write!(
                f,
                "ownership cannot be verified: no rpc endpoint is configured for chain {chain}"
            ),
            Self::OwnershipLookupFailed(why) => write!(f, "{why}"),
            Self::NotTheOwner { owner } => write!(
                f,
                "the signer does not own this token; it belongs to {owner}"
            ),
            Self::WrongChain { expected, got } => {
                write!(
                    f,
                    "this node serves chain {expected} for the network, not {got}"
                )
            }
            Self::ItemUnknown => write!(f, "the item is not in the catalogue yet; retry later"),
            Self::NotTheCreator => {
                write!(f, "only the item's creator may sell it as a primary sale")
            }
            Self::InvalidPriceAsset => write!(f, "The price of a trade must be paid in MANA"),
            Self::InvalidEstateTrade => write!(
                f,
                "The Estate trade is invalid, check for the fingerprint to see if it matches the Estate's one"
            ),
            Self::Duplicate => write!(f, "a trade with this signature already exists"),
            Self::DuplicateOpen(message) => write!(f, "{message}"),
            Self::Db(e) => write!(f, "database error: {e}"),
        }
    }
}

impl From<sqlx::Error> for TradeCreationError {
    fn from(e: sqlx::Error) -> Self {
        Self::Db(e)
    }
}

/// The chain this node serves for `network` ("ETHEREUM" or "MATIC").
pub fn served_chain(indexes: &super::chain_walk::IndexSettings, network: &str) -> i64 {
    use super::chain_walk::ChainNet;
    if network.eq_ignore_ascii_case("ETHEREUM") {
        indexes.chain_id(ChainNet::Ethereum)
    } else {
        indexes.chain_id(ChainNet::Matic)
    }
}

/// A public item order is a primary sale: only the item's creator may sign it.
async fn verify_item_creator(
    pool: &PgPool,
    network: &str,
    trade: &TradeCreation,
) -> Result<(), TradeCreationError> {
    let sent = &trade.sent[0];
    let item_id = sent.item_id.as_deref().unwrap_or_default();
    let creator: Option<String> = sqlx::query_scalar(
        "SELECT creator FROM shop.items \
         WHERE network = lower($1) AND contract_address = lower($2) AND item_id = $3::numeric",
    )
    .bind(network)
    .bind(&sent.contract_address)
    .bind(item_id)
    .fetch_optional(pool)
    .await?;
    match creator {
        None => Err(TradeCreationError::ItemUnknown),
        Some(c) if c.eq_ignore_ascii_case(&trade.signer) => Ok(()),
        Some(_) => Err(TradeCreationError::NotTheCreator),
    }
}

/// The shape marketplace-server ae872a4a `validateTradeByType` requires:
/// exactly one asset on each side, the right kinds for the type, a price
/// in the chain's MANA, and (here, before anything is stored) every
/// amount, token id and item id in canonical decimal.
fn validate_structure(trade: &TradeCreation) -> Result<(), TradeCreationError> {
    let invalid = |why: &str| Err(TradeCreationError::InvalidStructure(why.to_string()));
    let (sent, received) = match (trade.sent.as_slice(), trade.received.as_slice()) {
        ([sent], [received]) => (sent, received),
        _ => return invalid("sent and received must each carry exactly one asset"),
    };
    let price = match trade.trade_type.as_str() {
        TRADE_TYPE_BID => {
            if !(received.is_erc721() || received.is_item()) {
                return invalid("a bid must receive an nft or a collection item");
            }
            sent
        }
        TRADE_TYPE_PUBLIC_NFT_ORDER => {
            if !sent.is_erc721() {
                return invalid("a public nft order must send an erc721");
            }
            received
        }
        TRADE_TYPE_PUBLIC_ITEM_ORDER => {
            if !sent.is_item() {
                return invalid("a public item order must send a collection item");
            }
            received
        }
        other => {
            return Err(TradeCreationError::InvalidStructure(format!(
                "unknown trade type {other}"
            )));
        }
    };
    if !price.is_fungible() {
        return invalid("the price must be a fungible asset");
    }
    for asset in [sent, received] {
        let value = asset.signed_value().map_err(|e| {
            TradeCreationError::InvalidStructure(match e {
                SignatureError::Malformed(why) => why,
                other => other.to_string(),
            })
        })?;
        if !is_canonical_uint(&value) {
            return Err(TradeCreationError::InvalidStructure(format!(
                "{value:?} is not a canonical decimal uint256"
            )));
        }
    }
    if is_estate_chain(trade.chain_id) && (sent.is_item() || received.is_item()) {
        return invalid("collection items are not tradeable on an estate chain");
    }
    if !price.is_mana_price(trade.chain_id) {
        return Err(TradeCreationError::InvalidPriceAsset);
    }
    Ok(())
}

async fn verify_sent_ownership(
    state: Option<&MarketplaceState>,
    trade: &TradeCreation,
) -> Result<(), TradeCreationError> {
    let Some(asset) = trade.sent.first() else {
        return Ok(());
    };
    if !asset.is_erc721() {
        return Ok(());
    }
    let token_id = asset.token_id.as_ref().ok_or_else(|| {
        TradeCreationError::InvalidStructure("erc721 asset needs a tokenId".to_string())
    })?;
    let state = state.ok_or(TradeCreationError::OwnershipUnverifiable(trade.chain_id))?;
    let owner = owner_of(state, trade.chain_id, &asset.contract_address, token_id)
        .await
        .map_err(|e| match e {
            OwnershipError::NotConfigured(chain) => {
                TradeCreationError::OwnershipUnverifiable(chain)
            }
            other => TradeCreationError::OwnershipLookupFailed(other.to_string()),
        })?;
    if !owner.to_string().eq_ignore_ascii_case(&trade.signer) {
        return Err(TradeCreationError::NotTheOwner {
            owner: owner.to_string(),
        });
    }
    Ok(())
}

/// An Estate trade carries the registry's current fingerprint in `extra`
/// (marketplace-server ae872a4a `isValidEstateTrade`): an empty one is
/// refused outright, a stale one after `getFingerprintV2` on
/// `TRADE_RPC_URLS`. The read fails closed like the ownership check.
async fn verify_estate_fingerprint(
    state: Option<&MarketplaceState>,
    trade: &TradeCreation,
) -> Result<(), TradeCreationError> {
    let Some(estate) = estate_registry(trade.chain_id) else {
        return Ok(());
    };
    let Some(asset) = trade
        .sent
        .iter()
        .chain(trade.received.iter())
        .find(|a| a.contract_address.trim().eq_ignore_ascii_case(estate))
    else {
        return Ok(());
    };
    let extra = asset.extra.as_deref().unwrap_or_default().trim();
    if extra.is_empty() || extra == "0x" {
        return Err(TradeCreationError::InvalidEstateTrade);
    }
    let Some(token_id) = asset
        .token_id
        .as_deref()
        .and_then(|t| crate::types::parse_decimal(t).ok())
    else {
        return Err(TradeCreationError::InvalidEstateTrade);
    };
    let state = state.ok_or(TradeCreationError::OwnershipUnverifiable(trade.chain_id))?;
    let url = state
        .settings
        .market
        .trade_rpc_urls
        .for_chain(trade.chain_id)
        .ok_or(TradeCreationError::OwnershipUnverifiable(trade.chain_id))?;
    if !state.upstream_allowed(url) {
        return Err(TradeCreationError::OwnershipLookupFailed(
            "rpc endpoint is not a publicly routable https url".to_string(),
        ));
    }
    let estate: Address = estate.parse().expect("a pinned estate registry address");
    let current = crate::mana::chain::Reader::new(state.http.clone(), url)
        .estate_fingerprint(estate, token_id)
        .await
        .map_err(|e| {
            TradeCreationError::OwnershipLookupFailed(format!(
                "estate fingerprint lookup failed: {e}"
            ))
        })?;
    if !format!("{current:#x}").eq_ignore_ascii_case(extra) {
        return Err(TradeCreationError::InvalidEstateTrade);
    }
    Ok(())
}

/// marketplace-server ae872a4a `validateTradeByType`'s duplicate refusals,
/// over the trades this node counts as open (unexpired, not cancelled, not
/// closed on chain), each scoped to the signer: one open bid per bidder and
/// asset, one open order per NFT and one per collection item. Upstream
/// counts any signer's open order, so a previous owner's order that was
/// never executed would block the new owner until it expired; here the
/// signer has just been checked as the NFT's owner (`ownerOf`) or the item's
/// creator, and an order signed by anyone else cannot execute, so only the
/// signer's own open orders count. Run inside the insert's transaction under
/// a lock on the asset, so two concurrent posts cannot both pass. The same
/// signature is left to the insert's own 409.
///
/// Workload: one lookup per trade post (rate limited per signer and per
/// client). It is driven by `idx_trades_signer_created` (signer, then the
/// signer's trades) and joins each candidate's assets by
/// `idx_trade_assets_trade`; both writers store `contract_address`
/// lowercase, so it is compared as stored.
pub(crate) async fn refuse_duplicate_open(
    tx: &mut sqlx::PgConnection,
    trade: &TradeCreation,
    network: &str,
    since: Option<chrono::NaiveDateTime>,
    now: chrono::NaiveDateTime,
) -> Result<(), TradeCreationError> {
    let (asset, message) = match trade.trade_type.as_str() {
        TRADE_TYPE_BID => (
            &trade.received[0],
            "There is already a bid with the same parameters",
        ),
        TRADE_TYPE_PUBLIC_NFT_ORDER => (
            &trade.sent[0],
            "There is already an open order for this NFT",
        ),
        _ => (
            &trade.sent[0],
            "There is already an open order for this Item",
        ),
    };
    let signer = trade.signer.to_lowercase();
    let side = if trade.trade_type == TRADE_TYPE_BID {
        "received"
    } else {
        "sent"
    };
    let contract = asset.contract_address.to_lowercase();
    let token_id = asset.token_id.as_deref().filter(|_| asset.is_erc721());
    let item_id = asset.item_id.as_deref().filter(|_| asset.is_item());
    let key = format!(
        "trade-open:{}:{}:{contract}:{}:{}:{}",
        trade.trade_type,
        network.to_ascii_lowercase(),
        token_id.unwrap_or_default(),
        item_id.unwrap_or_default(),
        signer,
    );
    sqlx::query("SELECT pg_advisory_xact_lock(hashtext($1))")
        .bind(&key)
        .execute(&mut *tx)
        .await?;
    let sql = format!(
        "SELECT EXISTS (SELECT 1 FROM shop.trades t \
           JOIN shop.trade_assets a ON a.trade_id = t.id AND a.direction = $3::shop.asset_direction_type \
           LEFT JOIN shop.trade_assets_erc721 e ON e.asset_id = a.id \
           LEFT JOIN shop.trade_assets_item i ON i.asset_id = a.id \
          WHERE t.type = $1::shop.trade_type AND lower(t.network) = lower($2) \
            AND a.contract_address = $4 \
            AND e.token_id IS NOT DISTINCT FROM $5 AND i.item_id IS NOT DISTINCT FROM $6 \
            AND t.signer = $7 \
            AND t.hashed_signature <> $8 AND t.expires_at > $9 \
            AND NOT EXISTS (SELECT 1 FROM shop.trade_cancellations c WHERE c.target_hashed_signature = t.hashed_signature) \
            AND NOT {closed} AND NOT {unverifiable} AND {counted})",
        closed = super::trade_events::closed_on_chain_sql("t", "$9"),
        unverifiable = super::trade_events::unverifiable_sql("t"),
        counted = super::trades::counted_sql("t.", "$10"),
    );
    let open: bool = sqlx::query_scalar(sqlx::AssertSqlSafe(sql))
        .bind(&trade.trade_type)
        .bind(network)
        .bind(side)
        .bind(&contract)
        .bind(token_id)
        .bind(item_id)
        .bind(&signer)
        .bind(hashed_signature(&trade.signature.to_lowercase()))
        .bind(now)
        .bind(since)
        .fetch_one(&mut *tx)
        .await?;
    if open {
        return Err(TradeCreationError::DuplicateOpen(message));
    }
    Ok(())
}

fn ms_to_utc(ms: i64) -> Result<DateTime<Utc>, TradeCreationError> {
    Utc.timestamp_millis_opt(ms)
        .single()
        .ok_or_else(|| TradeCreationError::InvalidStructure(format!("unrepresentable time {ms}")))
}

/// keccak256 of the signature's bytes, as upstream stores it
/// (`keccak256(trade.signature)` over the hex, marketplace-server ae872a4)
/// and as the contract emits it in `Traded`/`SignatureCancelled`. The old
/// port hashed the hex text, a key no chain event or mirrored row carries;
/// a value that is not hex keeps that text hash (it cannot reach the chain).
pub fn hashed_signature(signature: &str) -> String {
    match hex::decode(signature.trim().trim_start_matches("0x")) {
        Ok(bytes) if !bytes.is_empty() => format!("0x{:x}", keccak256(&bytes)),
        _ => format!("0x{:x}", keccak256(signature.as_bytes())),
    }
}

/// The canonical camelCase JSON the `checks` column holds.
pub fn checks_json(checks: &TradeChecksInput) -> serde_json::Value {
    let mut v = json!({
        "uses": checks.uses,
        "expiration": checks.expiration,
        "effective": checks.effective,
        "salt": checks.salt,
        "contractSignatureIndex": checks.contract_signature_index,
        "signerSignatureIndex": checks.signer_signature_index,
        "allowedRoot": checks.allowed_root,
        "externalChecks": checks.external_checks.iter().flatten().map(|c| json!({
            "contractAddress": c.contract_address, "selector": c.selector, "value": c.value.as_deref().unwrap_or("0x"), "required": c.required
        })).collect::<Vec<_>>(),
    });
    if let Some(proof) = &checks.allowed_proof {
        v["allowedProof"] = json!(proof);
    }
    v
}

/// The whole policy, then the insert; `state` carries the RPC access (none
/// = an erc721 listing fails closed as unverifiable).
pub async fn create_trade(
    pool: &PgPool,
    state: Option<&MarketplaceState>,
    trade: &TradeCreation,
    authenticated: &str,
    now_ms: i64,
) -> Result<String, TradeCreationError> {
    if trade.checks.expiration < now_ms {
        return Err(TradeCreationError::Expired);
    }
    if trade.checks.expiration < trade.checks.effective {
        return Err(TradeCreationError::EffectiveAfterExpiration);
    }
    if !trade.signer.eq_ignore_ascii_case(authenticated) {
        return Err(TradeCreationError::SignerMismatch);
    }
    let candidates = offchain_marketplaces(trade.chain_id);
    if candidates.is_empty() {
        return Err(TradeCreationError::UnsupportedChain(trade.chain_id));
    }
    let network = network_for_chain(trade.chain_id)
        .ok_or(TradeCreationError::UnsupportedChain(trade.chain_id))?;
    if !trade.network.eq_ignore_ascii_case(network) {
        return Err(TradeCreationError::NetworkMismatch {
            expected: network.to_string(),
            got: trade.network.clone(),
        });
    }
    if let Some(s) = state {
        let expected = served_chain(&s.settings.market.indexes, network);
        if trade.chain_id != expected {
            return Err(TradeCreationError::WrongChain {
                expected,
                got: trade.chain_id,
            });
        }
    }
    validate_structure(trade)?;
    let matched = resolve_signature(trade, &candidates)
        .map_err(|e| TradeCreationError::InvalidSignature(e.to_string()))?;
    // Candidates are newest first and non-empty; older versions still verify, so a trade
    // signed on one is told where to sign instead.
    let current = candidates[0];
    if matched.marketplace != current {
        return Err(TradeCreationError::RetiredMarketplace {
            signed_on: matched.marketplace.contract,
            current: current.contract,
        });
    }
    verify_sent_ownership(state, trade).await?;
    verify_estate_fingerprint(state, trade).await?;
    if trade.trade_type == TRADE_TYPE_PUBLIC_ITEM_ORDER {
        verify_item_creator(pool, network, trade).await?;
    }
    // Stateless callers (tests) link to the default origins.
    let defaults;
    let settings = match state {
        Some(s) => &s.settings,
        None => {
            defaults = super::Settings::for_tests();
            &defaults
        }
    };
    // The bid_received notification is best effort: it never fails the bid.
    let bid_asset = if trade.trade_type == TRADE_TYPE_BID {
        super::trade_events::bid_asset(pool, &settings.land_image_base_url, network, trade)
            .await
            .unwrap_or_else(|e| {
                tracing::warn!(error = %e, "bid_received: asset lookup failed; no notification");
                None
            })
    } else {
        None
    };
    let now = ms_to_utc(now_ms)?.naive_utc();
    let since = super::trades::mirrored_since(settings, now);
    let mut tx = pool.begin().await?;
    refuse_duplicate_open(&mut tx, trade, network, since, now).await?;
    let trade_id = insert_trade(&mut *tx, trade, network, matched.marketplace.address).await?;
    if let Some(n) = bid_asset.and_then(|a| {
        super::trade_events::bid_received_notification(
            &a,
            &settings.market.marketplace_base_url,
            &trade_id,
            network,
            trade,
        )
    }) {
        let mut savepoint = sqlx::Connection::begin(&mut *tx).await?;
        match crate::shared::notifications::insert_notifications(&mut *savepoint, &[n]).await {
            Ok(_) => savepoint.commit().await?,
            Err(e) => {
                tracing::warn!(error = %e, trade_id, "bid_received: insert failed; the bid stands");
                savepoint.rollback().await?;
            }
        }
    }
    tx.commit().await?;
    Ok(trade_id)
}

const CREATE_TRADE_SQL: &str = "WITH t AS (
        INSERT INTO shop.trades (network, chain_id, signature, hashed_signature, checks, signer, type, expires_at, effective_since, contract, trade_digest)
        VALUES ($1, $2, $3, $4, $5, $6, $7::shop.trade_type, $8, $9, $10, $12)
        ON CONFLICT (hashed_signature) DO NOTHING
        RETURNING id
     ), input AS MATERIALIZED (
        SELECT gen_random_uuid() AS id, t.id AS trade_id, r.*
        FROM t, jsonb_to_recordset($11) AS r(direction text, asset_type smallint, contract_address text, beneficiary text, extra text, value text)
     ), inserted AS (
        INSERT INTO shop.trade_assets (id, trade_id, direction, asset_type, contract_address, beneficiary, extra)
        SELECT id, trade_id, direction::shop.asset_direction_type, asset_type, contract_address, beneficiary, extra FROM input
        RETURNING id
     ), erc721 AS (
        INSERT INTO shop.trade_assets_erc721 (asset_id, token_id) SELECT i.id, i.value FROM input i JOIN inserted USING (id) WHERE i.asset_type = 3
     ), erc20 AS (
        INSERT INTO shop.trade_assets_erc20 (asset_id, amount) SELECT i.id, i.value::numeric FROM input i JOIN inserted USING (id) WHERE i.asset_type IN (1, 2)
     ), item AS (
        INSERT INTO shop.trade_assets_item (asset_id, item_id) SELECT i.id, i.value FROM input i JOIN inserted USING (id) WHERE i.asset_type = 4
     )
     SELECT id::text FROM t";

/// The V3 cancellation key of `trade` signed on `contract`: its EIP-712
/// signing hash against that marketplace's domain, `None` on a marketplace
/// that cancels by signature (V1, V2) or one this node does not know.
pub(crate) fn cancellation_digest(trade: &TradeCreation, contract: &str) -> Option<String> {
    offchain_marketplaces(trade.chain_id)
        .into_iter()
        .find(|m| m.cancels_by_digest && m.address.eq_ignore_ascii_case(contract))
        .and_then(|m| signing_hash(trade, &m).ok())
        .map(|h| format!("{h:#x}"))
}

/// Head, assets and the typed sub-rows in one statement; a duplicate
/// signature inserts nothing. The signature is stored lowercase, as the
/// mirror stores upstream's, so one listing cannot land twice re-cased. A
/// trade on a V3 marketplace stores its [`cancellation_digest`].
pub(crate) async fn insert_trade<'e>(
    executor: impl sqlx::PgExecutor<'e>,
    trade: &TradeCreation,
    network: &str,
    contract: &str,
) -> Result<String, TradeCreationError> {
    let expires_at = ms_to_utc(trade.checks.expiration)?;
    let effective_since = ms_to_utc(trade.checks.effective)?;
    let mut rows = Vec::with_capacity(trade.sent.len() + trade.received.len());
    for (direction, assets) in [("sent", &trade.sent), ("received", &trade.received)] {
        for asset in assets {
            let (value, error) = match asset.asset_type {
                ASSET_TYPE_ERC721 => (&asset.token_id, "erc721 asset needs a tokenId"),
                ASSET_TYPE_ERC20 | ASSET_TYPE_USD_PEGGED_MANA => {
                    (&asset.amount, "fungible asset needs an amount")
                }
                ASSET_TYPE_COLLECTION_ITEM => (&asset.item_id, "collection item needs an itemId"),
                other => {
                    return Err(TradeCreationError::InvalidStructure(format!(
                        "unsupported asset type {other}"
                    )));
                }
            };
            let value = value
                .as_ref()
                .ok_or_else(|| TradeCreationError::InvalidStructure(error.to_owned()))?;
            rows.push(json!({
                "direction": direction,
                "asset_type": asset.asset_type,
                "contract_address": asset.contract_address.to_lowercase(),
                "beneficiary": asset.beneficiary.as_ref().map(|b| b.to_lowercase()),
                "extra": asset.extra.as_deref().filter(|e| !e.is_empty() && *e != "0x").unwrap_or(""),
                "value": value,
            }));
        }
    }
    let signature = trade.signature.to_lowercase();
    let inserted: Option<(String,)> = sqlx::query_as(CREATE_TRADE_SQL)
        .bind(network)
        .bind(trade.chain_id as i32)
        .bind(&signature)
        .bind(hashed_signature(&signature))
        .bind(checks_json(&trade.checks))
        .bind(trade.signer.to_lowercase())
        .bind(&trade.trade_type)
        .bind(expires_at.naive_utc())
        .bind(effective_since.naive_utc())
        .bind(contract)
        .bind(serde_json::Value::Array(rows))
        .bind(cancellation_digest(trade, contract))
        .fetch_optional(executor)
        .await?;
    let Some((trade_id,)) = inserted else {
        return Err(TradeCreationError::Duplicate);
    };
    Ok(trade_id)
}

fn creation_error_to_api(e: TradeCreationError) -> ApiError {
    let message = e.to_string();
    match e {
        TradeCreationError::SignerMismatch | TradeCreationError::InvalidSignature(_) => {
            ApiError::http(401, message)
        }
        TradeCreationError::NotTheOwner { .. } | TradeCreationError::NotTheCreator => {
            ApiError::http(403, message)
        }
        TradeCreationError::OwnershipUnverifiable(_)
        | TradeCreationError::OwnershipLookupFailed(_)
        | TradeCreationError::ItemUnknown => ApiError::http(503, message),
        TradeCreationError::Duplicate | TradeCreationError::DuplicateOpen(_) => {
            ApiError::http(409, message)
        }
        TradeCreationError::Db(_) => ApiError::http(500, message),
        _ => ApiError::bad_request(message),
    }
}

/// Trades one signer may post per minute; the map holds enough signers that a
/// burst of fresh wallets does not force the sweep that resets every bucket.
static TRADE_LIMITER: std::sync::LazyLock<RateLimiter> = std::sync::LazyLock::new(|| {
    RateLimiter::new(30, std::time::Duration::from_secs(60)).with_max_buckets(200_000)
});

/// Trades one client address may post per minute, across the wallets it signs
/// with (fresh wallets cost nothing); a request whose address is unknown or
/// shared (a gateway) is not counted here.
static TRADE_IP_LIMITER: std::sync::LazyLock<RateLimiter> = std::sync::LazyLock::new(|| {
    RateLimiter::new(120, std::time::Duration::from_secs(60)).with_max_buckets(200_000)
});

async fn post_trade_inner(
    state: &MarketplaceState,
    headers: &HeaderMap,
    peer: Option<std::net::SocketAddr>,
    body: &str,
) -> Result<Response, ApiError> {
    if let Some(ip) = crate::shared::client_ip::distinct_client_ip(
        headers,
        peer,
        &state.settings.market.trusted_proxy_cidrs,
    ) && TRADE_IP_LIMITER.check(&crate::shared::client_ip::source_key(&ip))
        == RateLimitDecision::Deny
    {
        return Err(ApiError::http(429, "rate limit exceeded".to_string()));
    }
    require_auth_metadata(headers, MARKETPLACE_AUTH_SIGNERS, Some(CREATE_TRADE_INTENT))?;
    let recovered = require_signer(headers, "post", "/v1/trades").await?;
    if TRADE_LIMITER.check(&recovered.to_ascii_lowercase()) == RateLimitDecision::Deny {
        return Err(ApiError::http(429, "rate limit exceeded".to_string()));
    }
    let trade: TradeCreation = serde_json::from_str(body)
        .map_err(|e| ApiError::bad_request(format!("invalid trade body: {e}")))?;
    // A creator's own item order is the Shop's primary sale; every other trade (a resale listing, a bid, and any type
    // added later) is the Marketplace's.
    if trade.trade_type != TRADE_TYPE_PUBLIC_ITEM_ORDER {
        state.require_marketplace()?;
    }
    let id = match create_trade(
        &state.pool,
        Some(state),
        &trade,
        &recovered,
        Utc::now().timestamp_millis(),
    )
    .await
    {
        Ok(id) => id,
        // The mirror catches up within a pass: tell the client when to retry.
        Err(TradeCreationError::ItemUnknown) => {
            let mut response =
                creation_error_to_api(TradeCreationError::ItemUnknown).into_response();
            response.headers_mut().insert(
                axum::http::header::RETRY_AFTER,
                axum::http::HeaderValue::from_static("60"),
            );
            return Ok(response);
        }
        Err(e) => return Err(creation_error_to_api(e)),
    };
    state.catalog_cache.invalidate();
    // The stored trade as `GET /v1/trades/{id}` answers it; its id alone if that read fails.
    let data = match super::trades::get_trade_by_id(
        &state.pool,
        &id,
        None,
        super::trades::TradeScope::Any,
    )
    .await
    {
        Ok(Some(trade)) => json!(trade),
        Ok(None) => json!({ "id": id }),
        Err(e) => {
            tracing::warn!(error = %e, trade_id = %id, "created trade could not be read back");
            json!({ "id": id })
        }
    };
    Ok((
        StatusCode::CREATED,
        Json(json!({ "ok": true, "data": data })),
    )
        .into_response())
}

pub async fn post_trade(
    State(state): State<SharedState>,
    peer: Option<axum::Extension<axum::extract::ConnectInfo<std::net::SocketAddr>>>,
    headers: HeaderMap,
    body: String,
) -> Response {
    let peer = peer.map(|axum::Extension(axum::extract::ConnectInfo(p))| p);
    post_trade_inner(&state, &headers, peer, &body)
        .await
        .unwrap_or_else(IntoResponse::into_response)
}

#[cfg(test)]
pub(crate) mod test_support {
    use super::*;
    use alloy::signers::SignerSync;
    use alloy::signers::local::PrivateKeySigner;

    pub fn signer() -> PrivateKeySigner {
        "0x4c0883a69102937d6231471b5dbb6204fe5129617082792ae468d01a3f362318"
            .parse()
            .unwrap()
    }

    pub fn trade_json_on(signer_address: &str, signature: &str, chain_id: i64) -> String {
        let mana = mana_token(chain_id).unwrap_or(NOT_MANA);
        format!(
            r#"{{
              "signer": "{signer_address}", "signature": "{signature}", "type": "public_nft_order", "network": "MATIC", "chainId": {chain_id},
              "checks": {{ "uses": 1, "expiration": 4102444800000, "effective": 0, "salt": "0x1234", "contractSignatureIndex": 0, "signerSignatureIndex": 0, "allowedRoot": "0x", "externalChecks": [] }},
              "sent": [ {{"assetType": 3, "contractAddress": "0x1111111111111111111111111111111111111111", "tokenId": "42", "extra": "0x"}} ],
              "received": [ {{"assetType": 1, "contractAddress": "{mana}", "amount": "1000", "extra": "0x", "beneficiary": "0x3333333333333333333333333333333333333333"}} ]
            }}"#
        )
    }

    /// An ERC-20 that is not MANA on any chain.
    pub const NOT_MANA: &str = "0x2222222222222222222222222222222222222222";

    pub fn parse(json: &str) -> TradeCreation {
        serde_json::from_str(json).expect("trade parses")
    }

    pub fn signed_trade_on(chain_id: i64, marketplace: &OffChainMarketplace) -> TradeCreation {
        let wallet = signer();
        let mut trade = parse(&trade_json_on(
            &wallet.address().to_checksum(None),
            "0x00",
            chain_id,
        ));
        sign(&mut trade, marketplace);
        trade
    }

    pub fn sign(trade: &mut TradeCreation, marketplace: &OffChainMarketplace) {
        let hash = signing_hash(trade, marketplace).unwrap();
        let sig = signer().sign_hash_sync(&hash).unwrap();
        trade.signature = format!("0x{}", hex::encode(sig.as_bytes()));
    }

    /// A JSON-RPC endpoint whose every `eth_call` answers `word`.
    pub async fn eth_call_stub(word: &'static str) -> String {
        use axum::routing::post;
        let app = axum::Router::new().route(
            "/",
            post(move |Json(req): Json<serde_json::Value>| async move {
                Json(json!({ "jsonrpc": "2.0", "id": req["id"], "result": word }))
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        format!("http://{addr}/")
    }

    pub fn signed_trade() -> TradeCreation {
        signed_trade_on(
            MATIC_MAINNET,
            &offchain_marketplace_v3(MATIC_MAINNET).unwrap(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::test_support::*;
    use super::*;

    #[test]
    fn endpoints_parse_per_chain_and_owner_words_decode() {
        let e = RpcEndpoints::from_env(Some("137=https://polygon.example, 1=https://eth.example"));
        assert_eq!(e.for_chain(137), Some("https://polygon.example"));
        assert_eq!(e.for_chain(42), None);
        assert!(RpcEndpoints::from_env(Some("137=")).is_empty());
        let data = owner_of_calldata("42").unwrap();
        assert!(data.starts_with("0x6352211e") && data.len() == 2 + 8 + 64 && data.ends_with("2a"));
        assert_eq!(
            address_from_word("0x000000000000000000000000abcdefabcdefabcdefabcdefabcdefabcdefabcd")
                .unwrap(),
            Address::from_str("0xabcdefabcdefabcdefabcdefabcdefabcdefabcd").unwrap()
        );
        assert!(matches!(
            address_from_word("0x00"),
            Err(OwnershipError::NoOwner(_))
        ));
    }

    #[test]
    fn a_real_mainnet_trade_signature_recovers_its_signer() {
        const SIGNER: &str = "0x02d0bb59a5f04a12d883751dc1605e15b4959b7e";
        let marketplace = OffChainMarketplace {
            contract: "OffChainMarketplaceV1",
            name: "DecentralandMarketplacePolygon",
            version: "1.0.0",
            address: "0x540fb08edb56aae562864b390542c97f562825ba",
            cancels_by_digest: false,
        };
        let mut trade = parse(&format!(
            r#"{{ "signer": "{SIGNER}", "signature": "0x2860a680deb41ba57ee26d6972c21d49d6cca25c74613ca04b9ed15d48a154f205fd3554d71836277e9d3f0143a62afad6f5c1636a7cd3d8f691dc4b0d8ccd011b",
                 "type": "public_nft_order", "network": "MATIC", "chainId": 137,
                 "checks": {{ "uses": 1, "expiration": 1798783200000, "effective": 1733927535000, "salt": "0x199a4082c5", "contractSignatureIndex": 0, "signerSignatureIndex": 0, "allowedRoot": "0x", "externalChecks": [] }},
                 "sent": [ {{"assetType": 3, "contractAddress": "0xe9e86941b23fbe9d8f4dd0c5b7e5f89722936878", "tokenId": "283", "extra": "0x"}} ],
                 "received": [ {{"assetType": 1, "contractAddress": "0xa1c57f48f0deb89f569dfbe6e2b7f46d33606fd4", "amount": "1000000000000000000", "extra": "0x", "beneficiary": "{SIGNER}"}} ] }}"#
        ));
        assert_eq!(
            hex::encode(signing_hash(&trade, &marketplace).unwrap()),
            "d4d6a86e2a1f0ab327b88353ef9cbd59ddde578a73fdcc176d8b07564c6f7718"
        );
        verify_signature(&trade, &marketplace).expect("a real signature verifies");
        trade.checks.uses = 2;
        assert!(verify_signature(&trade, &marketplace).is_err());
    }

    #[test]
    fn the_signing_hash_matches_its_captured_vectors() {
        let address = signer().address().to_checksum(None);
        let polygon = offchain_marketplace_v2(MATIC_MAINNET).unwrap();
        let ethereum = offchain_marketplace_v2(ETHEREUM_MAINNET).unwrap();
        let plain = parse(
            &trade_json_on(&address, "0x00", MATIC_MAINNET)
                .replace(mana_token(MATIC_MAINNET).unwrap(), NOT_MANA),
        );
        assert_eq!(
            hex::encode(signing_hash(&plain, &polygon).unwrap()),
            "3d05ec81149b209755d224fbdc951ac74a14e0dce7ae990544483f3d9e98b6c8"
        );
        assert_eq!(
            hex::encode(signing_hash(&plain, &ethereum).unwrap()),
            "cf23221fb47430c0c2cda5f501d69f68426ddeab266e9fb96be581c994154108"
        );
        let checked = parse(&format!(
            r#"{{ "signer": "{address}", "signature": "0x00", "type": "public_item_order", "network": "MATIC", "chainId": 137,
                 "checks": {{ "uses": 2, "expiration": 4102444800123, "effective": 1733691659485, "salt": "0x988c1638e5", "contractSignatureIndex": 3, "signerSignatureIndex": 5, "allowedRoot": "0xabcdef",
                   "externalChecks": [ {{"contractAddress": "0x4444444444444444444444444444444444444444", "selector": "0xdeadbeef", "value": "0x01f4", "required": true}}, {{"contractAddress": "0x5555555555555555555555555555555555555555", "selector": "0x12345678", "required": false}} ] }},
                 "sent": [ {{"assetType": 4, "contractAddress": "0x6666666666666666666666666666666666666666", "itemId": "7", "extra": "0xc0ffee"}} ],
                 "received": [ {{"assetType": 2, "contractAddress": "0x7777777777777777777777777777777777777777", "amount": "150000000000000000000"}} ] }}"#
        ));
        assert_eq!(
            hex::encode(signing_hash(&checked, &polygon).unwrap()),
            "082dcf8039c260f5ca1e02f811d3beb9ec6c2c4a64607086c912697b85c39a3c"
        );
    }

    #[test]
    fn signatures_resolve_newest_first_and_tampering_breaks_them() {
        let v3 = offchain_marketplace_v3(MATIC_AMOY).unwrap();
        let trade = signed_trade_on(MATIC_AMOY, &v3);
        let matched = resolve_signature(&trade, &offchain_marketplaces(MATIC_AMOY)).unwrap();
        assert_eq!(matched.marketplace, v3);
        assert_eq!(
            matched.cancellation_digest.as_deref().map(str::len),
            Some(66)
        );
        let v2 = offchain_marketplace_v2(MATIC_AMOY).unwrap();
        let matched = resolve_signature(
            &signed_trade_on(MATIC_AMOY, &v2),
            &offchain_marketplaces(MATIC_AMOY),
        )
        .unwrap();
        assert_eq!(
            (matched.marketplace, matched.cancellation_digest),
            (v2, None)
        );
        let trade = signed_trade();
        assert!(matches!(
            resolve_signature(&trade, &offchain_marketplaces(MATIC_AMOY)).unwrap_err(),
            SignatureError::Mismatch { .. }
        ));
        let mut tampered = signed_trade();
        tampered.sent[0].token_id = Some("43".into());
        assert!(matches!(
            verify_signature(&tampered, &offchain_marketplace_v2(MATIC_MAINNET).unwrap()),
            Err(SignatureError::Mismatch { .. })
        ));
        let mut short = signed_trade();
        short.signature = "0xdeadbeef".into();
        assert!(matches!(
            verify_signature(&short, &offchain_marketplace_v2(MATIC_MAINNET).unwrap()),
            Err(SignatureError::Malformed(_))
        ));
        let mut high_s = signed_trade();
        let mut raw = hex::decode(high_s.signature.trim_start_matches("0x")).unwrap();
        let n = U256::from_str_radix(
            "fffffffffffffffffffffffffffffffebaaedce6af48a03bbfd25e8cd0364141",
            16,
        )
        .unwrap();
        let s = U256::from_be_slice(&raw[32..64]);
        raw[32..64].copy_from_slice(&(n - s).to_be_bytes::<32>());
        raw[64] = match raw[64] {
            27 => 28,
            28 => 27,
            other => other ^ 1,
        };
        high_s.signature = format!("0x{}", hex::encode(&raw));
        assert!(
            matches!(
                verify_signature(&high_s, &offchain_marketplace_v2(MATIC_MAINNET).unwrap()),
                Err(SignatureError::Malformed(_))
            ),
            "malleated signatures read as invalid, not as a server fault"
        );
    }

    #[test]
    fn the_body_is_read_like_ajv_and_the_checks_are_stored_canonically() {
        let body = trade_json_on("0x1111111111111111111111111111111111111111", "0x00", 137)
            .replace(r#""chainId": 137"#, r#""chainId": 137.0"#)
            .replace(r#""uses": 1"#, r#""uses": 1.0"#);
        let t: TradeCreation = serde_json::from_str(&body).unwrap();
        assert_eq!((t.chain_id, t.checks.uses), (137, 1));
        for bad in [r#""uses": 1.5"#, r#""signerSignatureIndex": -1"#] {
            let body = trade_json_on("0x1", "0x00", 137).replace(
                if bad.starts_with("\"uses\"") {
                    r#""uses": 1"#
                } else {
                    r#""signerSignatureIndex": 0"#
                },
                bad,
            );
            assert!(
                serde_json::from_str::<TradeCreation>(&body).is_err(),
                "{bad}"
            );
        }
        let proved = parse(
            &trade_json_on("0x1111111111111111111111111111111111111111", "0x00", 137).replace(
                r#""allowedRoot": "0x","#,
                r#""allowedRoot": "0x", "allowedProof": ["0x01"],"#,
            ),
        );
        assert_eq!(checks_json(&proved.checks)["allowedProof"], json!(["0x01"]));
        let plain = parse(&trade_json_on(
            "0x1111111111111111111111111111111111111111",
            "0x00",
            137,
        ));
        assert!(checks_json(&plain.checks).get("allowedProof").is_none());
        let polygon = offchain_marketplace_v2(MATIC_MAINNET).unwrap();
        assert_eq!(
            signing_hash(&plain, &polygon).unwrap(),
            signing_hash(&proved, &polygon).unwrap(),
            "the proof is not signed over"
        );
        assert_eq!(
            hashed_signature("0xabcd"),
            format!("0x{:x}", keccak256([0xab_u8, 0xcd]))
        );
        // A mirrored upstream row (marketplace_squid lab copy, 2026-09-24):
        // the stored hash is keccak256 of the signature's bytes.
        let sig = "0x7fd2afc813e91b1b5ddabe66cd81b132a881ff9a5d73440c100afe0a338d2d6b3598857ffa3add6acc1389d4b9d067ea14b138d9ad7488865b966920ee7e41181b";
        assert_eq!(
            hashed_signature(sig),
            "0xf573a15c62efabc7ad5328d649fc89fd0bb10c66f7d524e336d4b1f5c10ad673"
        );
        assert_eq!(
            hashed_signature("0xabc"),
            format!("0x{:x}", keccak256(b"0xabc")),
            "odd hex keeps the text hash"
        );
    }

    /// The post limit is per client address: one address flooding the route
    /// is refused, and another client is not.
    #[tokio::test]
    async fn one_client_address_cannot_exhaust_trade_posts_for_others() {
        let state = crate::market::tests::support::state_without_db();
        let status = |ip: &str| {
            let (state, peer) = (state.clone(), format!("{ip}:443").parse().ok());
            async move {
                match post_trade_inner(&state, &HeaderMap::new(), peer, "{}").await {
                    Ok(r) => r.status().as_u16(),
                    Err(e) => e.into_response().status().as_u16(),
                }
            }
        };
        for _ in 0..120 {
            assert_ne!(status("203.0.113.71").await, 429);
        }
        assert_eq!(status("203.0.113.71").await, 429);
        assert_ne!(status("203.0.113.72").await, 429, "another client posts");
        // An IPv6 client is one /64: rotating addresses inside it shares the budget.
        for i in 0..120 {
            assert_ne!(status(&format!("[2001:db8:7:1::{i:x}]")).await, 429);
        }
        assert_eq!(status("[2001:db8:7:1::ffff]").await, 429);
        assert_ne!(status("[2001:db8:7:2::1]").await, 429, "another /64 posts");
    }

    /// A trade that verifies only against an older off-chain marketplace is answered 400 naming
    /// the contract to sign on, before ownership or the database is consulted.
    #[tokio::test]
    async fn a_trade_signed_on_an_older_marketplace_is_refused() {
        let state = crate::market::tests::support::state_without_db();
        for chain_id in [
            MATIC_MAINNET,
            ETHEREUM_MAINNET,
            MATIC_AMOY,
            ETHEREUM_SEPOLIA,
        ] {
            let mut trade = signed_trade_on(chain_id, &offchain_marketplace_v2(chain_id).unwrap());
            if network_for_chain(chain_id) == Some("ETHEREUM") {
                trade.network = "ETHEREUM".into();
                sign(&mut trade, &offchain_marketplace_v2(chain_id).unwrap());
            }
            let err = create_trade(&state.pool, None, &trade, &trade.signer.clone(), 1_000)
                .await
                .unwrap_err();
            assert!(
                matches!(err, TradeCreationError::RetiredMarketplace { .. }),
                "{chain_id}: {err}"
            );
            let response = creation_error_to_api(err).into_response();
            assert_eq!(response.status(), StatusCode::BAD_REQUEST);
            let body: serde_json::Value = serde_json::from_slice(
                &axum::body::to_bytes(response.into_body(), 1 << 16)
                    .await
                    .unwrap(),
            )
            .unwrap();
            assert_eq!(
                body["message"],
                "Trades signed on OffChainMarketplaceV2 are no longer accepted, sign it on OffChainMarketplaceV3"
            );
        }
    }

    /// The refusal names the marketplace the trade was signed on and the chain's current one.
    #[test]
    fn the_retired_marketplace_message_names_both_contracts() {
        let current = offchain_marketplace_v3(MATIC_MAINNET).unwrap();
        for signed_on in ["OffChainMarketplaceV1", "OffChainMarketplaceV2"] {
            let err = TradeCreationError::RetiredMarketplace {
                signed_on,
                current: current.contract,
            };
            assert_eq!(
                err.to_string(),
                format!(
                    "Trades signed on {signed_on} are no longer accepted, sign it on OffChainMarketplaceV3"
                )
            );
        }
    }

    #[tokio::test]
    async fn the_policy_refuses_before_any_signature_or_database_work() {
        let state = crate::market::tests::support::state_without_db();
        let pool = &state.pool;
        let mut expired = signed_trade();
        expired.checks.expiration = 1_000;
        assert!(matches!(
            create_trade(pool, None, &expired, &expired.signer.clone(), 2_000)
                .await
                .unwrap_err(),
            TradeCreationError::Expired
        ));
        let trade = signed_trade();
        assert!(matches!(
            create_trade(
                pool,
                None,
                &trade,
                "0x9999999999999999999999999999999999999999",
                1_000
            )
            .await
            .unwrap_err(),
            TradeCreationError::SignerMismatch
        ));
        assert!(
            matches!(
                create_trade(pool, None, &trade, &trade.signer.clone(), 1_000)
                    .await
                    .unwrap_err(),
                TradeCreationError::OwnershipUnverifiable(137)
            ),
            "an unverifiable erc721 listing fails closed"
        );
        assert!(
            matches!(
                create_trade(pool, Some(&state), &trade, &trade.signer.clone(), 1_000)
                    .await
                    .unwrap_err(),
                TradeCreationError::OwnershipUnverifiable(137)
            ),
            "no TRADE_RPC_URLS entry for the chain"
        );
        let testnet = signed_trade_on(MATIC_AMOY, &offchain_marketplace_v3(MATIC_AMOY).unwrap());
        assert!(
            matches!(
                create_trade(pool, Some(&state), &testnet, &testnet.signer.clone(), 1_000)
                    .await
                    .unwrap_err(),
                TradeCreationError::WrongChain {
                    expected: 137,
                    got: MATIC_AMOY
                }
            ),
            "a node serving Polygon mainnet refuses an Amoy-signed trade"
        );
        let mut other_chain = signed_trade();
        other_chain.chain_id = 42;
        assert!(matches!(
            create_trade(pool, None, &other_chain, &other_chain.signer.clone(), 1_000)
                .await
                .unwrap_err(),
            TradeCreationError::UnsupportedChain(42)
        ));
        let mut wrong_network = signed_trade();
        wrong_network.network = "ETHEREUM".into();
        assert!(matches!(
            create_trade(
                pool,
                None,
                &wrong_network,
                &wrong_network.signer.clone(),
                1_000
            )
            .await
            .unwrap_err(),
            TradeCreationError::NetworkMismatch { .. }
        ));
    }

    /// One case per refusal marketplace-server ae872a4a `validateTradeByType`
    /// makes, plus the canonical-number rule: each is a 400 before the
    /// signature, the chain or the database is consulted.
    #[tokio::test]
    async fn malformed_shapes_and_non_mana_prices_are_refused_before_any_insert() {
        let state = crate::market::tests::support::state_without_db();
        let pool = &state.pool;
        let address = signer().address().to_checksum(None);
        let bid_on = |token_id: &str| {
            parse(
                &trade_json_on(&address, "0x00", MATIC_MAINNET)
                    .replace(r#""type": "public_nft_order""#, r#""type": "bid""#)
                    .replace(r#""sent""#, r#""tmp""#)
                    .replace(r#""received""#, r#""sent""#)
                    .replace(r#""tmp""#, r#""received""#)
                    .replace(r#""tokenId": "42""#, &format!(r#""tokenId": "{token_id}""#)),
            )
        };
        let order = || parse(&trade_json_on(&address, "0x00", MATIC_MAINNET));
        let mut cases: Vec<(&str, TradeCreation)> = Vec::new();
        for token_id in ["", "0x2a", "1_0", "-1", "042", " 42"] {
            cases.push(("a non-canonical bid token id", bid_on(token_id)));
        }
        let mut amount = order();
        amount.received[0].amount = Some("1e18".into());
        cases.push(("a non-canonical amount", amount));
        let mut wide = order();
        wide.received[0].amount = Some("1".repeat(79));
        cases.push(("an amount past uint256", wide));
        let mut two_prices = order();
        two_prices.received.push(order().received.remove(0));
        cases.push(("two price assets", two_prices));
        let mut two_nfts = order();
        two_nfts.sent.push(order().sent.remove(0));
        cases.push(("two sent nfts", two_nfts));
        let mut no_token = bid_on("1");
        no_token.received[0].token_id = None;
        cases.push(("a bid without its token id", no_token));
        let mut nft_for_nft = order();
        nft_for_nft.received[0] = order().sent.remove(0);
        cases.push(("an nft priced in an nft", nft_for_nft));
        for (why, trade) in &cases {
            let err = create_trade(pool, None, trade, &address, 1_000)
                .await
                .unwrap_err();
            assert!(
                matches!(err, TradeCreationError::InvalidStructure(_)),
                "{why}: {err}"
            );
            assert_eq!(
                creation_error_to_api(err).into_response().status(),
                StatusCode::BAD_REQUEST,
                "{why}"
            );
        }

        let mut foreign = order();
        foreign.received[0].contract_address = NOT_MANA.into();
        let mut foreign_bid = bid_on("1");
        foreign_bid.sent[0].contract_address = NOT_MANA.into();
        let mut other_chains_mana = order();
        other_chains_mana.received[0].contract_address =
            mana_token(ETHEREUM_MAINNET).unwrap().into();
        for (why, trade) in [
            ("an order priced in another erc20", foreign),
            ("a bid paid in another erc20", foreign_bid),
            ("an order priced in another chain's MANA", other_chains_mana),
        ] {
            let err = create_trade(pool, None, &trade, &address, 1_000)
                .await
                .unwrap_err();
            assert!(
                matches!(err, TradeCreationError::InvalidPriceAsset),
                "{why}: {err}"
            );
            assert_eq!(err.to_string(), "The price of a trade must be paid in MANA");
            assert_eq!(
                creation_error_to_api(err).into_response().status(),
                StatusCode::BAD_REQUEST
            );
        }

        // The chain's MANA in any case and USD-pegged MANA from any address pass
        // the shape and reach the ownership check.
        let mut checksummed = order();
        checksummed.received[0].contract_address =
            "0xA1c57f48F0Deb89f569dFbE6E2B7f46D33606fD4".into();
        let mut pegged = order();
        pegged.received[0].asset_type = ASSET_TYPE_USD_PEGGED_MANA;
        pegged.received[0].contract_address = NOT_MANA.into();
        for mut trade in [checksummed, pegged] {
            sign(&mut trade, &offchain_marketplace_v3(MATIC_MAINNET).unwrap());
            assert!(matches!(
                create_trade(pool, None, &trade, &address, 1_000)
                    .await
                    .unwrap_err(),
                TradeCreationError::OwnershipUnverifiable(MATIC_MAINNET)
            ));
        }
    }

    /// An Estate bid on Ethereum carries the registry's current fingerprint
    /// (marketplace-server ae872a4a `isValidEstateTrade`).
    #[tokio::test]
    async fn an_estate_trade_needs_the_estates_current_fingerprint() {
        const CURRENT: &str = "0x00000000000000000000000000000000000000000000000000000000000f1a6e";
        let address = signer().address().to_checksum(None);
        let v3 = offchain_marketplace_v3(ETHEREUM_MAINNET).unwrap();
        let estate_bid = |extra: &str| {
            let mut trade = parse(
                &trade_json_on(&address, "0x00", ETHEREUM_MAINNET)
                    .replace(r#""network": "MATIC""#, r#""network": "ETHEREUM""#)
                    .replace(r#""type": "public_nft_order""#, r#""type": "bid""#)
                    .replace(r#""sent""#, r#""tmp""#)
                    .replace(r#""received""#, r#""sent""#)
                    .replace(r#""tmp""#, r#""received""#)
                    .replace(
                        "0x1111111111111111111111111111111111111111",
                        estate_registry(ETHEREUM_MAINNET).unwrap(),
                    ),
            );
            trade.received[0].extra = Some(extra.to_string());
            sign(&mut trade, &v3);
            trade
        };
        let rpc = eth_call_stub(CURRENT).await;
        let state = crate::market::tests::support::state_without_db_settings(
            crate::market::Settings::for_tests().with_market(crate::market::MarketSettings {
                trade_rpc_urls: RpcEndpoints::from_env(Some(&format!("1={rpc}"))),
                ..Default::default()
            }),
        );
        state
            .test_allow_loopback
            .store(true, std::sync::atomic::Ordering::Relaxed);
        let create = |trade: TradeCreation| {
            let state = state.clone();
            let address = address.clone();
            async move { create_trade(&state.pool, Some(&state), &trade, &address, 1_000).await }
        };
        for (why, extra) in [
            ("no fingerprint", "0x"),
            ("an empty fingerprint", ""),
            (
                "a stale fingerprint",
                "0x00000000000000000000000000000000000000000000000000000000000f1a6f",
            ),
        ] {
            let err = create(estate_bid(extra)).await.unwrap_err();
            assert!(
                matches!(err, TradeCreationError::InvalidEstateTrade),
                "{why}: {err}"
            );
            assert_eq!(
                creation_error_to_api(err).into_response().status(),
                StatusCode::BAD_REQUEST
            );
        }
        // The current fingerprint passes, in any case, on to the database.
        assert!(matches!(
            create(estate_bid(&CURRENT.to_uppercase().replace("0X", "0x")))
                .await
                .unwrap_err(),
            TradeCreationError::Db(_)
        ));
        // Without an endpoint the fingerprint cannot be read: fail closed.
        let err = create_trade(&state.pool, None, &estate_bid(CURRENT), &address, 1_000)
            .await
            .unwrap_err();
        assert!(matches!(
            err,
            TradeCreationError::OwnershipUnverifiable(ETHEREUM_MAINNET)
        ));
    }

    /// The create path and checkout read MANA from two tables; they agree on
    /// every chain a venue book settles on.
    #[test]
    fn the_mana_token_matches_every_venue_books_chain() {
        use crate::mana::venues::Book;
        use crate::types::ChainId;
        let production = Book::production(|_| None);
        let testnet = Book::testnet(|_| None);
        for (book, chain) in [
            (&production, ChainId::ETHEREUM),
            (&production, ChainId::POLYGON),
            (&testnet, ChainId::AMOY),
        ] {
            let mana = book.chain(chain).expect("a chain of the book").mana;
            assert_eq!(
                mana_token(chain.0 as i64),
                Some(format!("{mana:#x}").as_str()),
                "chain {}",
                chain.0
            );
        }
    }
}
