//! Deploy access read from the chain, as the reference catalyst's on-chain
//! validator reads it: `eth_call`s to the Checker contract of
//! `@dcl/catalyst-contracts` (its bytecode injected as a state override at
//! the checker address, calls batched through its `multicall`), item
//! ownership through `ownerOf`/`balanceOf`, every question asked at the block
//! of the entity timestamp and again at the block five minutes before it.
//! Blocks come from a binary search over the RPC ([`dcl_http::jsonrpc::BlockSearch`]).
//!
//! The vendored contract data is in `contracts/` with its provenance
//! (`upstream.lock.json`). A call that cannot be decided (no RPC, a failing
//! RPC, a node that cannot answer for that block) is an `Err`, never a grant;
//! a call the chain reverts is a "no".

mod items;

use std::collections::HashMap;
use std::future::Future;
use std::sync::LazyLock;

use alloy_primitives::{Address, B256, Bytes, I256, U256};
use alloy_sol_types::{SolCall, sol};
use serde::Deserialize;
use serde_json::{Value, json};

use super::error::ValidatorError;
use crate::http::Outbound;
use crate::http::jsonrpc::{BlockInfo, BlockSearch, Rpc, RpcError};

pub use items::v1_collection_contract;

sol! {
    interface Checker {
        function checkLAND(address _sender, address _land, address _estate, int256 _x, int256 _y) external view returns (bool);
        function checkName(address _sender, address _registrar, string _name) external view returns (bool);
        function multicall(bytes[] data) external view returns (bool[]);
        function validateThirdParty(address _tpRegistry, string _tpId, bytes32 _root) external view returns (bool);
        function validateWearables(address _sender, address[] _factories, address _collection, uint256 _itemId, string _contentHash, address[] _committees) external view returns (bool);
    }
    interface Erc721 {
        function ownerOf(uint256 tokenId) external view returns (address);
    }
    interface Erc1155 {
        function balanceOf(address account, uint256 id) external view returns (uint256);
    }
}

const CHECKER_BYTECODE: &str = include_str!("contracts/checker.bytecode");
const CONTRACTS_JSON: &str = include_str!("contracts/contracts.json");

/// The Checker's runtime bytecode, injected at the checker address.
pub fn checker_bytecode() -> &'static str {
    CHECKER_BYTECODE.trim()
}

/// Item ownership calls in flight at once per check.
const ITEM_CALLS_IN_FLIGHT: usize = 8;

/// An address as the vendored JSON spells it.
#[derive(Debug, Clone, Copy)]
struct Addr(Address);

impl<'de> Deserialize<'de> for Addr {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        s.parse().map(Addr).map_err(serde::de::Error::custom)
    }
}

#[derive(Deserialize)]
struct VendoredContracts {
    #[serde(rename = "l1Contracts")]
    l1: HashMap<String, L1Contracts>,
    #[serde(rename = "l2Contracts")]
    l2: HashMap<String, L2Contracts>,
}

#[derive(Debug, Clone, Deserialize)]
struct L1Contracts {
    registrar: Addr,
    land: Addr,
    #[serde(rename = "state")]
    estate: Addr,
    checker: Addr,
}

#[derive(Debug, Clone, Deserialize)]
struct SinceBlock {
    address: Addr,
    #[serde(rename = "sinceBlock")]
    since_block: u64,
}

#[derive(Debug, Clone, Deserialize)]
struct L2Contracts {
    checker: Addr,
    #[serde(rename = "thirdParty")]
    third_party: Addr,
    factories: Vec<SinceBlock>,
    #[serde(rename = "commitees")]
    committees: Vec<SinceBlock>,
}

static CONTRACTS: LazyLock<VendoredContracts> =
    LazyLock::new(|| serde_json::from_str(CONTRACTS_JSON).expect("vendored contracts.json"));

/// The L2 network paired with an `ETH_NETWORK`.
fn l2_network(eth_network: &str) -> Option<&'static str> {
    match eth_network {
        "mainnet" => Some("polygon"),
        "sepolia" => Some("amoy"),
        _ => None,
    }
}

/// The blocks a check is asked at: the entity timestamp's and five minutes
/// before it (`timestampBounds` of the reference client).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Blocks {
    pub at_deployment: Option<u64>,
    pub five_min_before: Option<u64>,
}

impl Blocks {
    fn each(self) -> impl Iterator<Item = u64> {
        [self.at_deployment, self.five_min_before]
            .into_iter()
            .flatten()
            .filter(|b| *b > 0)
    }
}

/// `(upper, lower)` seconds for an entity timestamp in ms: up to 8 s after it
/// and 5 min 7 s before that.
pub fn timestamp_bounds(timestamp_ms: i64) -> (i64, i64) {
    let upper = timestamp_ms.div_euclid(1000) + i64::from(timestamp_ms.rem_euclid(1000) > 0) + 8;
    (upper, (upper - 60 * 5 - 7).max(0))
}

/// Whether a JSON-RPC error is the call's own revert.
fn is_revert(e: &RpcError) -> bool {
    matches!(e, RpcError::Response { code, message }
        if *code == 3 || message.to_ascii_lowercase().contains("revert"))
}

fn query_failed(what: &str, e: impl std::fmt::Display) -> ValidatorError {
    ValidatorError::BlockchainQuery(format!("{what}: {e}"))
}

/// One chain's RPC and the blocks read from it.
struct Chain {
    http: Outbound,
    url: String,
    search: BlockSearch,
    name: &'static str,
}

impl Chain {
    fn new(http: Outbound, url: String, name: &'static str) -> Self {
        Self {
            http,
            url,
            search: BlockSearch::new(),
            name,
        }
    }

    fn rpc(&self) -> Rpc<'_> {
        Rpc::new(&self.http, &self.url)
    }

    async fn block_at(&self, timestamp: i64) -> Result<Option<BlockInfo>, ValidatorError> {
        let rpc = self.rpc();
        self.search
            .block_at(timestamp, |tag| rpc.block(tag))
            .await
            .map_err(|e| query_failed(&format!("{} block lookup", self.name), e))
    }

    async fn blocks_for(&self, timestamp_ms: i64) -> Result<Blocks, ValidatorError> {
        let (upper, lower) = timestamp_bounds(timestamp_ms);
        let at = self.block_at(upper).await?;
        let before = self.block_at(lower).await?;
        Ok(Blocks {
            at_deployment: at.map(|b| b.number),
            // The first block at or after `lower`, as the blocks subgraph answered.
            five_min_before: before.map(|b| b.number + u64::from(b.timestamp < lower)),
        })
    }

    /// `eth_call` at `block`, with `overrides` as the state override; `None`
    /// when the chain reverts the call.
    async fn call(
        &self,
        to: Address,
        data: Vec<u8>,
        block: u64,
        overrides: Option<&Value>,
    ) -> Result<Option<Vec<u8>>, ValidatorError> {
        let mut params = vec![
            json!({ "to": to.to_string(), "data": format!("0x{}", hex::encode(&data)) }),
            json!(format!("0x{block:x}")),
        ];
        if let Some(o) = overrides {
            params.push(o.clone());
        }
        match self.rpc().call_hex("eth_call", Value::Array(params)).await {
            Ok(hex) => hex::decode(hex.trim_start_matches("0x"))
                .map(Some)
                .map_err(|e| query_failed(&format!("{} eth_call", self.name), e)),
            Err(e) if is_revert(&e) => Ok(None),
            Err(e) => Err(query_failed(&format!("{} eth_call", self.name), e)),
        }
    }

    /// The Checker's `multicall` over `calls` at `block`; all false when it reverts.
    async fn multicall(
        &self,
        checker: Address,
        calls: Vec<Vec<u8>>,
        block: u64,
    ) -> Result<Vec<bool>, ValidatorError> {
        if calls.is_empty() {
            return Ok(Vec::new());
        }
        let n = calls.len();
        let data = Checker::multicallCall {
            data: calls.into_iter().map(Bytes::from).collect(),
        }
        .abi_encode();
        let overrides = json!({ checker.to_string(): { "code": checker_bytecode() } });
        let Some(out) = self.call(checker, data, block, Some(&overrides)).await? else {
            return Ok(vec![false; n]);
        };
        let results = Checker::multicallCall::abi_decode_returns(&out)
            .map_err(|e| query_failed(&format!("{} checker answer", self.name), e))?;
        if results.len() != n {
            return Err(query_failed(
                &format!("{} checker answer", self.name),
                format!("{} results for {n} calls", results.len()),
            ));
        }
        Ok(results)
    }
}

/// Asks `check` at the deploy block and, for what it did not grant, at the
/// block five minutes before. An entry neither block granted is an `Err` when
/// either ask failed; no block at all grants nothing.
async fn at_deploy_or_before<F, Fut>(
    n: usize,
    blocks: Blocks,
    check: F,
) -> Result<Vec<bool>, ValidatorError>
where
    F: Fn(u64) -> Fut,
    Fut: Future<Output = Result<Vec<bool>, ValidatorError>>,
{
    let mut granted = vec![false; n];
    let mut failure = None;
    for block in blocks.each() {
        match check(block).await {
            Ok(answer) => {
                for (g, a) in granted.iter_mut().zip(answer) {
                    *g |= a;
                }
            }
            Err(e) => failure = Some(e),
        }
        if granted.iter().all(|g| *g) {
            return Ok(granted);
        }
    }
    match failure {
        Some(e) => Err(e),
        None => Ok(granted),
    }
}

struct L1 {
    chain: Chain,
    contracts: L1Contracts,
}

struct L2 {
    chain: Chain,
    contracts: L2Contracts,
}

/// The deploy access checks of every entity type, at an entity timestamp:
/// local deploys and entities received from peers ask the same questions.
pub struct OnChainAccess {
    l1: Result<L1, String>,
    l2: Result<L2, String>,
    /// ERC-721 or ERC-1155 per third-party NFT contract, once known.
    nft_kinds: parking_lot::Mutex<HashMap<(bool, Address), NftKind>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NftKind {
    Erc721,
    Erc1155,
    Unknown,
}

impl OnChainAccess {
    /// `eth_network` picks the vendored contracts (mainnet with polygon,
    /// sepolia with amoy); `l1_url`/`l2_url` are `ETH_RPC_URL` and
    /// `RPC_ENDPOINT_POLYGON`. A missing one leaves its checks undecidable.
    pub fn new(
        http: Outbound,
        eth_network: &str,
        l1_url: Option<String>,
        l2_url: Option<String>,
    ) -> Self {
        let l1 = match (CONTRACTS.l1.get(eth_network), l1_url) {
            (None, _) => Err(format!(
                "ETH_NETWORK {eth_network} has no checker contracts"
            )),
            (_, None) => Err("no L1 RPC is configured (ETH_RPC_URL)".to_string()),
            (Some(c), Some(url)) => Ok(L1 {
                chain: Chain::new(http.clone(), url, "L1"),
                contracts: c.clone(),
            }),
        };
        let l2 = match (
            l2_network(eth_network).and_then(|n| CONTRACTS.l2.get(n)),
            l2_url,
        ) {
            (None, _) => Err(format!(
                "ETH_NETWORK {eth_network} has no L2 checker contracts"
            )),
            (_, None) => Err("no L2 RPC is configured (RPC_ENDPOINT_POLYGON)".to_string()),
            (Some(c), Some(url)) => Ok(L2 {
                chain: Chain::new(http, url, "L2"),
                contracts: c.clone(),
            }),
        };
        Self {
            l1,
            l2,
            nft_kinds: parking_lot::Mutex::new(HashMap::new()),
        }
    }

    fn l1(&self) -> Result<&L1, ValidatorError> {
        self.l1
            .as_ref()
            .map_err(|m| ValidatorError::Unconfigured(m.clone()))
    }

    fn l2(&self) -> Result<&L2, ValidatorError> {
        self.l2
            .as_ref()
            .map_err(|m| ValidatorError::Unconfigured(m.clone()))
    }

    /// Per parcel, whether `address` may deploy on it (owner, operator,
    /// update operator, update manager or approved, of the LAND or its estate).
    pub async fn land(
        &self,
        address: &str,
        parcels: &[(i32, i32)],
        timestamp_ms: i64,
    ) -> Result<Vec<bool>, ValidatorError> {
        let l1 = self.l1()?;
        let Ok(sender) = address.parse::<Address>() else {
            return Ok(vec![false; parcels.len()]);
        };
        let calls: Vec<Vec<u8>> = parcels
            .iter()
            .map(|(x, y)| {
                Checker::checkLANDCall {
                    _sender: sender,
                    _land: l1.contracts.land.0,
                    _estate: l1.contracts.estate.0,
                    _x: I256::try_from(*x).unwrap_or_default(),
                    _y: I256::try_from(*y).unwrap_or_default(),
                }
                .abi_encode()
            })
            .collect();
        let blocks = l1.chain.blocks_for(timestamp_ms).await?;
        at_deploy_or_before(parcels.len(), blocks, |block| {
            l1.chain
                .multicall(l1.contracts.checker.0, calls.clone(), block)
        })
        .await
    }

    /// Per name, whether `address` holds it.
    pub async fn names(
        &self,
        address: &str,
        names: &[String],
        timestamp_ms: i64,
    ) -> Result<Vec<bool>, ValidatorError> {
        if names.is_empty() {
            return Ok(Vec::new());
        }
        let l1 = self.l1()?;
        let Ok(sender) = address.parse::<Address>() else {
            return Ok(vec![false; names.len()]);
        };
        let calls: Vec<Vec<u8>> = names
            .iter()
            .map(|name| {
                Checker::checkNameCall {
                    _sender: sender,
                    _registrar: l1.contracts.registrar.0,
                    _name: name.clone(),
                }
                .abi_encode()
            })
            .collect();
        let blocks = l1.chain.blocks_for(timestamp_ms).await?;
        at_deploy_or_before(names.len(), blocks, |block| {
            l1.chain
                .multicall(l1.contracts.checker.0, calls.clone(), block)
        })
        .await
    }

    /// Whether `address` may deploy item `item_id` of the L2 collection
    /// `contract` with content hashing to one of `hashes` (the creator or a
    /// manager of an editable collection, or a committee member for an item
    /// whose content hash is set).
    pub async fn collection_item(
        &self,
        address: &str,
        contract: &str,
        item_id: &str,
        hashes: &[String],
        timestamp_ms: i64,
    ) -> Result<bool, ValidatorError> {
        let l2 = self.l2()?;
        let (Ok(sender), Ok(collection), Some(item)) = (
            address.parse::<Address>(),
            contract.parse::<Address>(),
            items::parse_uint(item_id),
        ) else {
            return Ok(false);
        };
        let blocks = l2.chain.blocks_for(timestamp_ms).await?;
        let active = |list: &[SinceBlock], block: u64| -> Vec<Address> {
            list.iter()
                .filter(|s| block >= s.since_block)
                .map(|s| s.address.0)
                .collect()
        };
        let granted = at_deploy_or_before(1, blocks, |block| {
            let calls: Vec<Vec<u8>> = hashes
                .iter()
                .map(|hash| {
                    Checker::validateWearablesCall {
                        _sender: sender,
                        _factories: active(&l2.contracts.factories, block),
                        _collection: collection,
                        _itemId: item,
                        _contentHash: hash.clone(),
                        _committees: active(&l2.contracts.committees, block),
                    }
                    .abi_encode()
                })
                .collect();
            async move {
                let answers = l2
                    .chain
                    .multicall(l2.contracts.checker.0, calls, block)
                    .await?;
                Ok(vec![answers.into_iter().any(|a| a)])
            }
        })
        .await?;
        Ok(granted[0])
    }

    /// Whether the third party `tp_id` is approved with merkle root `root`.
    pub async fn third_party_root(
        &self,
        tp_id: &str,
        root: [u8; 32],
        timestamp_ms: i64,
    ) -> Result<bool, ValidatorError> {
        let l2 = self.l2()?;
        let call = Checker::validateThirdPartyCall {
            _tpRegistry: l2.contracts.third_party.0,
            _tpId: tp_id.to_string(),
            _root: B256::from(root),
        }
        .abi_encode();
        let blocks = l2.chain.blocks_for(timestamp_ms).await?;
        let granted = at_deploy_or_before(1, blocks, |block| {
            l2.chain
                .multicall(l2.contracts.checker.0, vec![call.clone()], block)
        })
        .await?;
        Ok(granted[0])
    }

    /// Per URN, whether `address` owns it: a collection item's token
    /// (`ownerOf`), a third-party item's linked NFT (`ownerOf` or
    /// `balanceOf`); assets without a token and URNs that name no item are
    /// not asked about and count as owned.
    pub async fn items(
        &self,
        address: &str,
        urns: &[String],
        timestamp_ms: i64,
    ) -> Result<Vec<bool>, ValidatorError> {
        let refs: Vec<items::ItemRef> = urns.iter().map(|u| items::item_ref(u)).collect();
        let mut owned: Vec<bool> = refs
            .iter()
            .map(|r| matches!(r, items::ItemRef::NotAsked))
            .collect();
        let owner = address.parse::<Address>().ok();
        for l1_layer in [true, false] {
            let pending: Vec<usize> = (0..refs.len())
                .filter(|i| refs[*i].on_l1() == Some(l1_layer))
                .collect();
            if pending.is_empty() {
                continue;
            }
            let chain = if l1_layer {
                &self.l1()?.chain
            } else {
                &self.l2()?.chain
            };
            let Some(owner) = owner else {
                continue;
            };
            // Each third-party contract's standard once, before the calls that need it.
            for i in &pending {
                if let items::ItemRef::ThirdParty { contract, .. } = &refs[*i] {
                    self.nft_kind(chain, l1_layer, *contract).await?;
                }
            }
            let blocks = chain.blocks_for(timestamp_ms).await?;
            let answer = at_deploy_or_before(pending.len(), blocks, |block| {
                self.owns_each(chain, l1_layer, owner, &refs, &pending, block)
            })
            .await?;
            for (i, a) in pending.iter().zip(answer) {
                owned[*i] = a;
            }
        }
        Ok(owned)
    }

    /// `owns` for the `pending` entries of `refs`, a few calls at a time.
    async fn owns_each(
        &self,
        chain: &Chain,
        l1_layer: bool,
        owner: Address,
        refs: &[items::ItemRef],
        pending: &[usize],
        block: u64,
    ) -> Result<Vec<bool>, ValidatorError> {
        let mut out = Vec::with_capacity(pending.len());
        for batch in pending.chunks(ITEM_CALLS_IN_FLIGHT) {
            let calls: Vec<_> = batch
                .iter()
                .map(|i| self.owns(chain, l1_layer, owner, &refs[*i], block))
                .collect();
            for answer in futures::future::join_all(calls).await {
                out.push(answer?);
            }
        }
        Ok(out)
    }

    async fn owns(
        &self,
        chain: &Chain,
        l1_layer: bool,
        owner: Address,
        item: &items::ItemRef,
        block: u64,
    ) -> Result<bool, ValidatorError> {
        match item {
            items::ItemRef::Token {
                contract: Some(contract),
                token,
                ..
            } => self.owner_of(chain, *contract, *token, owner, block).await,
            items::ItemRef::ThirdParty {
                contract, token, ..
            } => match self.nft_kind(chain, l1_layer, *contract).await? {
                NftKind::Erc721 => self.owner_of(chain, *contract, *token, owner, block).await,
                NftKind::Erc1155 => {
                    let data = Erc1155::balanceOfCall {
                        account: owner,
                        id: *token,
                    }
                    .abi_encode();
                    Ok(match chain.call(*contract, data, block, None).await? {
                        Some(out) => Erc1155::balanceOfCall::abi_decode_returns(&out)
                            .is_ok_and(|b| b > U256::ZERO),
                        None => false,
                    })
                }
                NftKind::Unknown => Ok(false),
            },
            _ => Ok(false),
        }
    }

    async fn owner_of(
        &self,
        chain: &Chain,
        contract: Address,
        token: U256,
        owner: Address,
        block: u64,
    ) -> Result<bool, ValidatorError> {
        let data = Erc721::ownerOfCall { tokenId: token }.abi_encode();
        Ok(match chain.call(contract, data, block, None).await? {
            Some(out) => Erc721::ownerOfCall::abi_decode_returns(&out).is_ok_and(|a| a == owner),
            None => false,
        })
    }

    /// Whether a third-party NFT contract answers as ERC-1155, else ERC-721,
    /// probed at the head as the reference checker does, kept once known.
    async fn nft_kind(
        &self,
        chain: &Chain,
        l1_layer: bool,
        contract: Address,
    ) -> Result<NftKind, ValidatorError> {
        if let Some(kind) = self.nft_kinds.lock().get(&(l1_layer, contract)) {
            return Ok(*kind);
        }
        let head = chain.block_at(i64::MAX).await?.map_or(0, |b| b.number);
        let erc1155 = Erc1155::balanceOfCall {
            account: contract,
            id: U256::ZERO,
        }
        .abi_encode();
        let kind = match chain.call(contract, erc1155, head, None).await? {
            Some(out) if out.len() >= 32 => NftKind::Erc1155,
            _ => {
                let erc721 = Erc721::ownerOfCall {
                    tokenId: U256::ZERO,
                }
                .abi_encode();
                match chain.call(contract, erc721, head, None).await? {
                    None => NftKind::Erc721,
                    Some(out) if out.len() >= 32 => NftKind::Erc721,
                    Some(_) => NftKind::Unknown,
                }
            }
        };
        self.nft_kinds.lock().insert((l1_layer, contract), kind);
        Ok(kind)
    }
}

#[cfg(test)]
mod tests;
