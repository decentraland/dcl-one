//! Third-party wearables: the approved providers of the third-party registry
//! subgraph (`THIRD_PARTY_REGISTRY_L2_SUBGRAPH_URL`), the entities each
//! provider deployed (their `metadata.mappings` link a wearable to NFT token
//! ids), and the one ownership resolver the listings, the profiles and the
//! outfits share:
//!
//! - the owner's holdings on the providers' contracts (the listings): the NFT
//!   worker the old lambdas asked (`NFT_WORKER_BASE_URL`) when configured,
//!   else the chain itself, contract by contract (`owned_on_network`);
//! - a worn linked item's NFT (profiles, outfits): its entity's mapping must
//!   include it and `ownerOf`/`balanceOf` must confirm it
//!   (`sanitize_third_party_urns`).
//!
//! Chain reads go over each `ContractNetwork`'s JSON-RPC endpoint
//! (`rpc_url_for`) through the ONE guarded client, and never scan logs: per
//! contract a `balanceOf(owner)` and two `supportsInterface` probes, then
//! the `tokenOfOwnerByIndex` walk of an enumerable ERC-721, or the mapped
//! ids' `ownerOf`/`balanceOfBatch`. An `any` or wide `range` mapping on a
//! contract that cannot list an owner's tokens (a plain ERC-721 the owner
//! holds some of, any ERC-1155) is unresolved on this path: its mapped ids
//! are still confirmed, the rest is logged and left out. The old lambdas
//! read those holdings from the NFT worker only; without it they need an
//! indexer of the contracts' transfers.
//!
//! A contract whose read failed (transport, non-2xx, a JSON-RPC error other
//! than a revert) is a failure of that contract, never "nothing owned", and
//! is remembered `FAILED_TTL`. The dedicated routes and the by-owner list
//! answer it with the old lambdas' 502 `FetcherError`; the explorer leaves
//! that contract's items out (`OnFailure`). The NFT worker answers as the
//! old fetcher read it: a non-2xx answer reports nothing for that network,
//! a transport failure or an unreadable body is a failure.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

use alloy_primitives::{Address, Bytes, U256, address, hex};
use alloy_sol_types::{SolCall, sol};
use axum::Json;
use axum::extract::{Path, Request, State};
use axum::response::{IntoResponse, Response};
use serde_json::{Value, json};

use crate::content::queries::{
    active_entity_docs_by_pointers, active_entity_docs_by_prefix, retain_non_denylisted,
};
use crate::shared::cache::TtlMap;
use crate::shared::query_params::{
    JsPagination, parse_pagination_js, parse_query_string, qs_get_array, qs_get_string,
};

use super::definitions::{extract_wearable_definition, locale_cmp};
use super::graph::graph_query;
use super::listings::{ETHEREUM_MAINNET, ETHEREUM_SEPOLIA, MATIC_AMOY, MATIC_MAINNET};
use super::user_items::is_third_party_name_urn;
use super::{MarketplaceState, SharedState, bad_request};

const PROVIDER_TTL: Duration = Duration::from_secs(6 * 60 * 60);
const COLLECTION_ENTITIES_TTL: Duration = Duration::from_secs(300);
const OWNED_TTL: Duration = Duration::from_secs(60);
/// A holdings read that failed is answered from memory this long, so a
/// burst of requests does not ask the failing endpoint again.
const FAILED_TTL: Duration = Duration::from_secs(5);
const OWNED_MAX_ENTRIES: usize = 4096;

sol! {
    interface IERC165 {
        function supportsInterface(bytes4 interfaceId) external view returns (bool);
    }
    interface IERC721 {
        function balanceOf(address owner) external view returns (uint256);
        function ownerOf(uint256 tokenId) external view returns (address);
        function tokenOfOwnerByIndex(address owner, uint256 index) external view returns (uint256);
    }
    interface IERC1155 {
        function balanceOf(address account, uint256 id) external view returns (uint256);
        function balanceOfBatch(address[] accounts, uint256[] ids) external view returns (uint256[]);
    }
    interface IMulticall3 {
        struct Call3 { address target; bool allowFailure; bytes callData; }
        struct Result3 { bool success; bytes returnData; }
        function aggregate3(Call3[] calls) external payable returns (Result3[] returnData);
    }
}

/// Same address on every chain it is deployed to, mainnet and polygon included.
const MULTICALL3: Address = address!("0xcA11bde05977b3631167028862bE2a173976CA11");
const IFACE_ERC1155: [u8; 4] = [0xd9, 0xb6, 0x7a, 0x26];
const IFACE_ERC721_ENUMERABLE: [u8; 4] = [0x78, 0x0e, 0x9d, 0x63];
/// A `range` mapping wider than this is not expanded into ids.
const RANGE_CAP: u64 = 2048;
/// The largest enumerable balance walked; past it the contract's read fails.
const ENUMERATE_MAX: u64 = 50_000;
/// Sub-calls per `aggregate3` (ids per `balanceOfBatch`), and those calls
/// per JSON-RPC request: a walk is paged 2,000 tokens a request.
const MULTICALL_CHUNK: usize = 500;
const MULTICALLS_PER_REQUEST: usize = 4;
const RPC_BATCH: usize = 50;
const CONTRACT_LEG_CONCURRENCY: usize = 8;
const RPC_TIMEOUT: Duration = Duration::from_secs(30);
const RPC_BODY_CAP: usize = 16 * 1024 * 1024;
/// A JSON-RPC answer quoted in an error is cut to this many bytes.
const ERROR_TEXT_CAP: usize = 512;

/// `(owner, network, fingerprint of the network's whole candidate set)`.
type OwnedKey = (String, String, u64);
/// One provider contract's read: its `network:contract:tokenId` triples, or
/// why they could not be read.
type ContractRead = (String, Result<Vec<String>, String>);

/// The third-party readers' memo: the provider list (6 h), each provider's
/// mapped entities (5 min), the owner's tokens per network (60 s, a failed
/// read `FAILED_TTL`).
pub struct Caches {
    providers: TtlMap<(), Arc<Vec<ThirdPartyProvider>>>,
    collection_entities: TtlMap<String, Arc<Vec<Value>>>,
    owned: TtlMap<OwnedKey, Arc<Vec<ContractRead>>>,
    failed: TtlMap<OwnedKey, Arc<Vec<ContractRead>>>,
}

impl Default for Caches {
    fn default() -> Self {
        Self {
            providers: TtlMap::bounded(PROVIDER_TTL, 1),
            collection_entities: TtlMap::bounded(COLLECTION_ENTITIES_TTL, 256),
            owned: TtlMap::bounded(OWNED_TTL, OWNED_MAX_ENTRIES),
            failed: TtlMap::bounded(FAILED_TTL, OWNED_MAX_ENTRIES),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TpContract {
    pub network: String,
    pub address: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ThirdPartyProvider {
    pub id: String,
    pub contracts: Vec<TpContract>,
    /// `metadata.thirdParty.name` and `.description` as the registry
    /// answers them (`null` kept); `None` when absent.
    pub name: Option<Value>,
    pub description: Option<Value>,
}

const TP_QUERY: &str = r#"
{
  thirdParties(where: {isApproved: true}) {
    id
    resolver
    metadata {
      thirdParty {
        name
        description
        contracts {
          network
          address
        }
      }
    }
  }
}"#;

fn parse_providers(data: &Value) -> Vec<ThirdPartyProvider> {
    let arr = data
        .get("thirdParties")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    let mut out = Vec::with_capacity(arr.len());
    for p in arr {
        let id = p
            .get("id")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        if id.is_empty() {
            continue;
        }
        let mut contracts = Vec::new();
        let third_party = p.get("metadata").and_then(|m| m.get("thirdParty"));
        if let Some(cs) = third_party
            .and_then(|t| t.get("contracts"))
            .and_then(|c| c.as_array())
        {
            for c in cs {
                let network = c.get("network").and_then(|v| v.as_str()).unwrap_or("");
                let address = c.get("address").and_then(|v| v.as_str()).unwrap_or("");
                if !network.is_empty() && !address.is_empty() {
                    contracts.push(TpContract {
                        network: network.to_string(),
                        address: address.to_lowercase(),
                    });
                }
            }
        }
        out.push(ThirdPartyProvider {
            id,
            contracts,
            name: third_party.and_then(|t| t.get("name")).cloned(),
            description: third_party.and_then(|t| t.get("description")).cloned(),
        });
    }
    out
}

/// The approved providers; empty without a registry subgraph. A failed
/// registry read serves the last list however old, and with none is an
/// error, as lamb2's providers storage
/// (decentraland/lamb2:src/logic/third-party-providers-storage.ts:33-52).
pub async fn third_party_providers(
    state: &MarketplaceState,
) -> Result<Vec<ThirdPartyProvider>, String> {
    let Some(url) = state.settings.third_party_registry_subgraph_url.clone() else {
        tracing::debug!("THIRD_PARTY_REGISTRY_L2_SUBGRAPH_URL is unset: no third-party providers");
        return Ok(Vec::new());
    };
    let cache = &state.third_party.providers;
    match cache
        .get_or_fetch((), || async {
            let data = graph_query(&state.index, &url, TP_QUERY, json!({})).await?;
            Ok::<_, String>(Arc::new(parse_providers(&data)))
        })
        .await
    {
        Ok(p) => Ok((*p).clone()),
        Err(e) => match cache.get_stale(&()) {
            Some(p) => {
                tracing::warn!(error = %e, "third-party registry lookup failed: serving the last list");
                Ok((*p).clone())
            }
            None => Err(format!("third-party registry: {e}")),
        },
    }
}

pub fn mappings_includes_nft(
    mappings: &Value,
    network: &str,
    contract: &str,
    token_id: &str,
) -> bool {
    let Some(list) = mappings
        .get(network)
        .and_then(|n| n.get(contract))
        .and_then(|c| c.as_array())
    else {
        return false;
    };
    list.iter()
        .any(|m| match m.get("type").and_then(|t| t.as_str()) {
            Some("single") => m.get("id").and_then(|v| v.as_str()) == Some(token_id),
            Some("any") => true,
            Some("multiple") => m
                .get("ids")
                .and_then(|v| v.as_array())
                .map(|ids| ids.iter().any(|i| i.as_str() == Some(token_id)))
                .unwrap_or(false),
            Some("range") => {
                let from = m.get("from").and_then(|v| v.as_str());
                let to = m.get("to").and_then(|v| v.as_str());
                match (from, to, U256::from_str_radix(token_id, 10).ok()) {
                    (Some(f), Some(t), Some(tk)) => {
                        match (U256::from_str_radix(f, 10), U256::from_str_radix(t, 10)) {
                            (Ok(fv), Ok(tv)) => tk >= fv && tk <= tv,
                            _ => false,
                        }
                    }
                    _ => false,
                }
            }
            _ => false,
        })
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Hash)]
pub struct Candidates {
    /// Token ids exactly as the mappings spell them.
    pub ids: BTreeSet<String>,
    /// An `any` or an oversized `range` mapping: only enumerable contracts can answer.
    pub open: bool,
}

/// `(network, lowercase contract)` -> the token ids the mappings link to it.
pub type CandidateMap = BTreeMap<(String, String), Candidates>;

pub fn candidates_from_mappings<'a>(entities: impl IntoIterator<Item = &'a Value>) -> CandidateMap {
    let mut out = CandidateMap::new();
    for entity in entities {
        let Some(mappings) = entity
            .pointer("/metadata/mappings")
            .and_then(Value::as_object)
        else {
            continue;
        };
        for (network, contracts) in mappings {
            let Some(contracts) = contracts.as_object() else {
                continue;
            };
            for (contract, list) in contracts {
                let Some(list) = list.as_array() else {
                    continue;
                };
                let slot = out
                    .entry((network.clone(), contract.to_lowercase()))
                    .or_default();
                for m in list {
                    match m.get("type").and_then(Value::as_str) {
                        Some("single") => {
                            if let Some(id) = m.get("id").and_then(Value::as_str) {
                                slot.ids.insert(id.to_string());
                            }
                        }
                        Some("multiple") => {
                            for id in m.get("ids").and_then(Value::as_array).into_iter().flatten() {
                                if let Some(id) = id.as_str() {
                                    slot.ids.insert(id.to_string());
                                }
                            }
                        }
                        Some("range") => {
                            let bound = |k: &str| {
                                m.get(k)
                                    .and_then(Value::as_str)
                                    .and_then(|s| U256::from_str_radix(s, 10).ok())
                            };
                            match (bound("from"), bound("to")) {
                                (Some(from), Some(to))
                                    if to >= from && to - from < U256::from(RANGE_CAP) =>
                                {
                                    let mut id = from;
                                    loop {
                                        slot.ids.insert(id.to_string());
                                        if id == to {
                                            break;
                                        }
                                        id += U256::from(1);
                                    }
                                }
                                _ => slot.open = true,
                            }
                        }
                        Some("any") => slot.open = true,
                        _ => {}
                    }
                }
            }
        }
    }
    out
}

/// The chain id of a `ContractNetwork` (the networks a provider contract, a
/// mapping or a linked item's NFT may name).
fn chain_id_of(network: &str) -> Option<i64> {
    match network {
        "mainnet" => Some(ETHEREUM_MAINNET),
        "sepolia" => Some(ETHEREUM_SEPOLIA),
        "matic" => Some(MATIC_MAINNET),
        "amoy" => Some(MATIC_AMOY),
        _ => None,
    }
}

/// The JSON-RPC endpoint of a `ContractNetwork`. The L2 of `ETH_NETWORK`
/// (matic beside mainnet, amoy beside sepolia) reads `RPC_ENDPOINT_POLYGON`,
/// else its `TRADE_RPC_URLS` entry; any other network its `TRADE_RPC_URLS`
/// entry (`1`, `11155111`, `137`, `80002`), and the L1 of `ETH_NETWORK`
/// then `ETH_RPC_URL`, as the old lambdas paired their L1 and L2 providers.
fn rpc_url_for(state: &MarketplaceState, network: &str) -> Option<String> {
    let chain_id = chain_id_of(network)?;
    let listed = state
        .settings
        .market
        .trade_rpc_urls
        .for_chain(chain_id)
        .map(str::to_string);
    let (l1, l2) = match state.settings.eth_network.trim() {
        "sepolia" => ("sepolia", "amoy"),
        _ => ("mainnet", "matic"),
    };
    if network == l2 {
        state.settings.polygon_rpc_url.clone().or(listed)
    } else if network == l1 {
        listed.or_else(|| state.settings.eth_rpc_url.clone())
    } else {
        listed
    }
}

/// An owner's holdings: `network:contract:tokenId` triples, the NFT worker's
/// answer shape, and the contracts (or worker networks) that could not be
/// read, each with its cause.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Holdings {
    pub owned: Vec<String>,
    pub failed: Vec<String>,
}

/// The owner's holdings over each network's JSON-RPC endpoint. A network
/// with no RPC endpoint configured reports nothing (logged): the deployment
/// did not provide the reader.
pub async fn owned_via_rpc(
    state: &MarketplaceState,
    owner: &str,
    candidates: &CandidateMap,
) -> Holdings {
    let mut holdings = Holdings::default();
    let Ok(owner_addr) = Address::from_str(owner) else {
        return holdings;
    };
    let mut by_network: BTreeMap<&str, Vec<(String, Candidates)>> = BTreeMap::new();
    for ((network, contract), c) in candidates {
        by_network
            .entry(network.as_str())
            .or_default()
            .push((contract.clone(), c.clone()));
    }
    let per_network = by_network.into_iter().filter_map(|(network, contracts)| {
        let Some(rpc) = rpc_url_for(state, network) else {
            tracing::warn!(network, "third-party wearable ownership: no RPC endpoint for this network; reporting nothing owned");
            return None;
        };
        let key = (
            owner.to_lowercase(),
            network.to_string(),
            fingerprint(&contracts),
        );
        let network = network.to_string();
        Some(async move {
            if let Some(reads) = state.third_party.failed.get_fresh(&key) {
                return (network, reads);
            }
            let reads = state
                .third_party
                .owned
                .get_or_fetch(key.clone(), || async {
                    let reads = Arc::new(
                        owned_on_network(state, &rpc, owner_addr, &network, &contracts).await,
                    );
                    if reads.iter().any(|(_, read)| read.is_err()) {
                        state.third_party.failed.insert(key, reads.clone());
                        Err(reads)
                    } else {
                        Ok(reads)
                    }
                })
                .await
                .unwrap_or_else(|failed| failed);
            (network, reads)
        })
    });
    for (network, reads) in futures::future::join_all(per_network).await {
        for (contract, read) in reads.iter() {
            match read {
                Ok(owned) => holdings.owned.extend(owned.iter().cloned()),
                Err(e) => holdings.failed.push(format!("{network}:{contract}: {e}")),
            }
        }
    }
    holdings
}

/// A network's candidate set, every contract's ids and open flag included,
/// so two sets that differ in any id never share a cache entry.
fn fingerprint(contracts: &[(String, Candidates)]) -> u64 {
    use std::hash::{DefaultHasher, Hash, Hasher};
    let mut h = DefaultHasher::new();
    contracts.hash(&mut h);
    h.finish()
}

/// The owner's holdings on one network's provider contracts, contract by
/// contract, each confirmed against current ownership: the whole
/// `tokenOfOwnerByIndex` walk of an enumerable ERC-721, else the mapped ids
/// (`ownerOf` on an ERC-721 the owner holds some of, `balanceOfBatch` on an
/// ERC-1155). An open mapping (`any`, a wide `range`) on a contract that
/// cannot be walked is logged as unresolved. A failure fails its contract
/// only; a failed probe batch fails them all.
async fn owned_on_network(
    state: &MarketplaceState,
    rpc: &str,
    owner: Address,
    network: &str,
    contracts: &[(String, Candidates)],
) -> Vec<ContractRead> {
    let mut probes = Vec::with_capacity(contracts.len() * 3);
    let mut addrs = Vec::with_capacity(contracts.len());
    for (contract, _) in contracts {
        let Ok(addr) = Address::from_str(contract) else {
            addrs.push(None);
            continue;
        };
        addrs.push(Some(addr));
        probes.push(RpcCall {
            to: addr,
            data: IERC721::balanceOfCall { owner }.abi_encode().into(),
        });
        probes.push(supports_call(addr, IFACE_ERC1155));
        probes.push(supports_call(addr, IFACE_ERC721_ENUMERABLE));
    }
    let answers = match eth_call_batch(state, rpc, &probes).await {
        Ok(answers) => answers,
        Err(e) => {
            return contracts
                .iter()
                .map(|(contract, _)| (contract.clone(), Err(e.clone())))
                .collect();
        }
    };

    // One leg per contract, run concurrently; the answer keeps contract order.
    let mut legs = Vec::with_capacity(contracts.len());
    let mut i = 0;
    for ((contract, cand), addr) in contracts.iter().zip(addrs) {
        let Some(addr) = addr else {
            continue;
        };
        let balance = answers
            .get(i)
            .and_then(Option::as_ref)
            .and_then(|b| IERC721::balanceOfCall::abi_decode_returns(b).ok());
        let is_1155 = decode_bool(answers.get(i + 1));
        let enumerable = decode_bool(answers.get(i + 2));
        i += 3;
        legs.push(async move {
            let found = if is_1155 {
                if cand.open {
                    unresolved(
                        network,
                        contract,
                        owner,
                        "an ERC-1155 cannot list an owner's tokens",
                    );
                }
                owned_1155(state, rpc, addr, owner, parsed_ids(cand)).await
            } else {
                match balance {
                    Some(b) if b.is_zero() => Ok(Vec::new()),
                    Some(b) if enumerable => enumerate_721(state, rpc, addr, owner, b).await,
                    Some(_) => {
                        if cand.open {
                            unresolved(network, contract, owner, "the ERC-721 is not enumerable");
                        }
                        check_721(state, rpc, addr, owner, parsed_ids(cand)).await
                    }
                    // Neither an ERC-721 nor an ERC-1155.
                    None => Ok(Vec::new()),
                }
            };
            let found = found.map(|ids| {
                ids.into_iter()
                    .map(|id| format!("{network}:{contract}:{id}"))
                    .collect()
            });
            (contract.clone(), found)
        });
    }
    use futures::stream::StreamExt;
    futures::stream::iter(legs)
        .buffered(CONTRACT_LEG_CONCURRENCY)
        .collect()
        .await
}

/// Logs an open mapping the chain path cannot list (`owned_on_network`).
fn unresolved(network: &str, contract: &str, owner: Address, why: &'static str) {
    tracing::info!(
        network,
        contract,
        owner = %owner,
        why,
        "third-party holdings: an open mapping is unresolved without NFT_WORKER_BASE_URL; only its mapped ids are read"
    );
}

fn supports_call(to: Address, interface: [u8; 4]) -> RpcCall {
    RpcCall {
        to,
        data: IERC165::supportsInterfaceCall {
            interfaceId: interface.into(),
        }
        .abi_encode()
        .into(),
    }
}

fn decode_bool(answer: Option<&Option<Bytes>>) -> bool {
    answer
        .and_then(Option::as_ref)
        .and_then(|b| IERC165::supportsInterfaceCall::abi_decode_returns(b).ok())
        .unwrap_or(false)
}

/// Token ids to confirm: each as the mappings spell it (the spelling a
/// `single` or `multiple` mapping matches), with its value.
type IdSet = BTreeMap<String, U256>;

fn parsed_ids(cand: &Candidates) -> IdSet {
    cand.ids
        .iter()
        .filter_map(|id| U256::from_str_radix(id, 10).ok().map(|u| (id.clone(), u)))
        .collect()
}

async fn check_721(
    state: &MarketplaceState,
    rpc: &str,
    addr: Address,
    owner: Address,
    ids: IdSet,
) -> Result<Vec<String>, String> {
    let calldatas: Vec<Bytes> = ids
        .values()
        .map(|id| IERC721::ownerOfCall { tokenId: *id }.abi_encode().into())
        .collect();
    let answers = multicall(state, rpc, addr, calldatas).await?;
    Ok(ids
        .into_keys()
        .zip(answers)
        .filter(|(_, ret)| {
            ret.as_ref()
                .and_then(|b| IERC721::ownerOfCall::abi_decode_returns(b).ok())
                .is_some_and(|holder| holder == owner)
        })
        .map(|(id, _)| id)
        .collect())
}

/// Every token of an enumerable ERC-721 the owner holds, up to a balance of
/// `ENUMERATE_MAX`, paged (`multicall`); a larger one (or a contract
/// answering a nonsense balance) fails the contract's read rather than
/// truncating it.
async fn enumerate_721(
    state: &MarketplaceState,
    rpc: &str,
    addr: Address,
    owner: Address,
    balance: U256,
) -> Result<Vec<String>, String> {
    if balance > U256::from(ENUMERATE_MAX) {
        return Err(format!(
            "{addr}: a balance of {balance} is past the {ENUMERATE_MAX} tokens walked"
        ));
    }
    let n = balance.to::<u64>();
    let calldatas: Vec<Bytes> = (0..n)
        .map(|i| {
            IERC721::tokenOfOwnerByIndexCall {
                owner,
                index: U256::from(i),
            }
            .abi_encode()
            .into()
        })
        .collect();
    Ok(multicall(state, rpc, addr, calldatas)
        .await?
        .into_iter()
        .filter_map(|ret| {
            ret.and_then(|b| IERC721::tokenOfOwnerByIndexCall::abi_decode_returns(&b).ok())
        })
        .map(|id| id.to_string())
        .collect())
}

async fn owned_1155(
    state: &MarketplaceState,
    rpc: &str,
    addr: Address,
    owner: Address,
    ids: IdSet,
) -> Result<Vec<String>, String> {
    let ids: Vec<(String, U256)> = ids.into_iter().collect();
    let mut owned = Vec::new();
    for page in ids.chunks(MULTICALL_CHUNK * MULTICALLS_PER_REQUEST) {
        let calls: Vec<RpcCall> = page
            .chunks(MULTICALL_CHUNK)
            .map(|chunk| RpcCall {
                to: addr,
                data: IERC1155::balanceOfBatchCall {
                    accounts: vec![owner; chunk.len()],
                    ids: chunk.iter().map(|(_, u)| *u).collect(),
                }
                .abi_encode()
                .into(),
            })
            .collect();
        let answers = eth_call_batch(state, rpc, &calls).await?;
        for (chunk, answer) in page.chunks(MULTICALL_CHUNK).zip(answers) {
            let Some(balances) = answer
                .as_ref()
                .and_then(|b| IERC1155::balanceOfBatchCall::abi_decode_returns(b).ok())
            else {
                continue;
            };
            for ((id, _), balance) in chunk.iter().zip(balances) {
                if !balance.is_zero() {
                    owned.push(id.clone());
                }
            }
        }
    }
    Ok(owned)
}

/// One entry per calldata: the sub-call's return data when it succeeded.
/// Paged `MULTICALLS_PER_REQUEST` multicalls of `MULTICALL_CHUNK` a request.
async fn multicall(
    state: &MarketplaceState,
    rpc: &str,
    target: Address,
    calldatas: Vec<Bytes>,
) -> Result<Vec<Option<Bytes>>, String> {
    let mut out = Vec::with_capacity(calldatas.len());
    for page in calldatas.chunks(MULTICALL_CHUNK * MULTICALLS_PER_REQUEST) {
        let calls: Vec<RpcCall> = page
            .chunks(MULTICALL_CHUNK)
            .map(|chunk| RpcCall {
                to: MULTICALL3,
                data: IMulticall3::aggregate3Call {
                    calls: chunk
                        .iter()
                        .map(|d| IMulticall3::Call3 {
                            target,
                            allowFailure: true,
                            callData: d.clone(),
                        })
                        .collect(),
                }
                .abi_encode()
                .into(),
            })
            .collect();
        let answers = eth_call_batch(state, rpc, &calls).await?;
        for (chunk, answer) in page.chunks(MULTICALL_CHUNK).zip(answers) {
            out.extend(
                decode_aggregate3(answer.as_ref(), chunk.len())
                    .ok_or_else(|| format!("{target}: the multicall answered no results"))?,
            );
        }
    }
    Ok(out)
}

/// The sub-call answers of one `aggregate3`; `None` when the multicall
/// itself failed, which says nothing about the sub-calls.
fn decode_aggregate3(answer: Option<&Bytes>, expected: usize) -> Option<Vec<Option<Bytes>>> {
    match answer.and_then(|b| IMulticall3::aggregate3Call::abi_decode_returns(b).ok()) {
        Some(results) if results.len() == expected => Some(
            results
                .into_iter()
                .map(|r| r.success.then_some(r.returnData))
                .collect(),
        ),
        _ => None,
    }
}

struct RpcCall {
    to: Address,
    data: Bytes,
}

/// `eth_call` each call at `latest`. A call the node answers with a revert
/// is `None`, as the old checkers read a missing `result` as not owned; any
/// other JSON-RPC error (a rate limit such as -32005, a node timeout) fails
/// the batch.
async fn eth_call_batch(
    state: &MarketplaceState,
    rpc: &str,
    calls: &[RpcCall],
) -> Result<Vec<Option<Bytes>>, String> {
    let requests: Vec<(&str, Value)> = calls
        .iter()
        .map(|c| {
            (
                "eth_call",
                json!([{ "to": c.to.to_string(), "data": c.data.to_string() }, "latest"]),
            )
        })
        .collect();
    rpc_batch(state, rpc, &requests)
        .await?
        .into_iter()
        .map(|answer| match answer {
            Ok(result) => Ok(result
                .as_str()
                .and_then(|h| hex::decode(h.trim_start_matches("0x")).ok())
                .map(Bytes::from)),
            Err(error) if is_revert(&error) => Ok(None),
            Err(error) => Err(format!("eth_call: {}", clipped(&error.to_string()))),
        })
        .collect()
}

/// A JSON-RPC error that is the call reverting: code 3, or the `execution
/// reverted` message nodes answer with -32000.
fn is_revert(error: &Value) -> bool {
    error.get("code").and_then(Value::as_i64) == Some(3)
        || error
            .get("message")
            .and_then(Value::as_str)
            .is_some_and(|m| m.to_ascii_lowercase().contains("revert"))
}

/// `text` cut to `ERROR_TEXT_CAP` bytes, for an error or a log line.
fn clipped(text: &str) -> String {
    if text.len() <= ERROR_TEXT_CAP {
        return text.to_string();
    }
    let mut end = ERROR_TEXT_CAP;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}... ({} bytes)", &text[..end], text.len())
}

/// JSON-RPC batches of `RPC_BATCH` requests through the guarded client:
/// per request its `result`, or the `error` the node answered it with. A
/// transport failure, a non-2xx status, an unreadable body or a missing
/// answer fails the whole call.
async fn rpc_batch(
    state: &MarketplaceState,
    rpc: &str,
    requests: &[(&str, Value)],
) -> Result<Vec<Result<Value, Value>>, String> {
    if !state.upstream_allowed(rpc) {
        return Err("the RPC endpoint is not a publicly routable http(s) URL".to_string());
    }
    let mut out: Vec<Option<Result<Value, Value>>> = vec![None; requests.len()];
    for (chunk_index, chunk) in requests.chunks(RPC_BATCH).enumerate() {
        let base = chunk_index * RPC_BATCH;
        let body: Vec<Value> = chunk
            .iter()
            .enumerate()
            .map(|(i, (method, params))| {
                json!({ "jsonrpc": "2.0", "id": base + i, "method": method, "params": params })
            })
            .collect();
        let resp = state
            .http
            .post(rpc)
            .timeout(RPC_TIMEOUT)
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("JSON-RPC batch failed: {e}"))?;
        if !resp.status().is_success() {
            return Err(format!("JSON-RPC batch rejected: {}", resp.status()));
        }
        let parsed: Value = crate::http::read_body_capped(resp, RPC_BODY_CAP)
            .await
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .ok_or_else(|| "JSON-RPC batch: unreadable response".to_string())?;
        let Some(items) = parsed.as_array() else {
            return Err(format!(
                "JSON-RPC batch: answered {} instead of a batch",
                clipped(&parsed.to_string())
            ));
        };
        for item in items {
            let Some(idx) = item.get("id").and_then(Value::as_u64).map(|id| id as usize) else {
                continue;
            };
            if idx < base || idx >= base + chunk.len() {
                continue;
            }
            out[idx] = Some(match item.get("result") {
                Some(result) => Ok(result.clone()),
                None => Err(item
                    .get("error")
                    .cloned()
                    .unwrap_or_else(|| json!("no result"))),
            });
        }
    }
    out.into_iter()
        .map(|answer| answer.ok_or_else(|| "JSON-RPC batch: a request got no answer".to_string()))
        .collect()
}

/// `NFT_WORKER_BASE_URL`: `POST <base>/wallets/<owner>/networks/<network>/nfts`
/// with the network's contract addresses, answering `{data: [triple]}`, the
/// old lambdas' source (`alchemy-nft-fetcher.ts`). As there, a network
/// outside `ContractNetwork` and a non-2xx answer are logged and report
/// nothing; a transport failure or an unreadable body fails that network.
async fn owned_via_nft_worker(
    state: &MarketplaceState,
    base: &str,
    owner: &str,
    contracts_by_network: &BTreeMap<String, BTreeSet<String>>,
) -> Holdings {
    let per_network = contracts_by_network
        .iter()
        .map(|(network, contracts)| async move {
            let found = async {
                if chain_id_of(network).is_none() {
                    tracing::warn!(network, "network not supported for linked wearables");
                    return Ok(Vec::new());
                }
                let url = format!("{base}/wallets/{owner}/networks/{network}/nfts");
                if !state.upstream_allowed(&url) {
                    return Err(
                        "NFT_WORKER_BASE_URL is not a publicly routable http(s) URL".to_string()
                    );
                }
                let resp = state
                    .http
                    .post(&url)
                    .timeout(RPC_TIMEOUT)
                    .header("Accept", "application/json")
                    .json(contracts)
                    .send()
                    .await
                    .map_err(|e| format!("NFT worker: {e}"))?;
                if !resp.status().is_success() {
                    tracing::error!(status = %resp.status(), network, "NFT worker answered an error");
                    return Ok(Vec::new());
                }
                let body: Value = crate::http::read_body_capped(resp, RPC_BODY_CAP)
                    .await
                    .ok()
                    .and_then(|b| serde_json::from_slice(&b).ok())
                    .ok_or_else(|| "NFT worker: unreadable response".to_string())?;
                Ok::<_, String>(
                    body.get("data")
                        .and_then(Value::as_array)
                        .into_iter()
                        .flatten()
                        .filter_map(Value::as_str)
                        .map(str::to_string)
                        .collect::<Vec<_>>(),
                )
            };
            (network, found.await)
        });
    let mut holdings = Holdings::default();
    for (network, found) in futures::future::join_all(per_network).await {
        match found {
            Ok(owned) => holdings.owned.extend(owned),
            Err(e) => holdings.failed.push(format!("{network}: {e}")),
        }
    }
    holdings
}

#[derive(Clone)]
pub struct TpwElement {
    pub urn: String,
    pub individual_data: Vec<Value>,
    pub name: String,
    pub category: String,
    pub entity: Value,
}

impl TpwElement {
    fn to_value(&self) -> Value {
        json!({
            "urn": self.urn,
            "individualData": self.individual_data,
            "amount": self.individual_data.len(),
            "name": self.name,
            "category": self.category,
            "entity": self.entity,
        })
    }
}

/// What a holdings read does with a provider contract it could not read.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OnFailure {
    /// Fail the whole read: the dedicated routes' 502 `FetcherError`.
    Fail,
    /// Leave that contract's items out (logged): the explorer.
    Skip,
}

/// Why a third-party read answers no list, as the old lambdas' `FetcherError`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FetchFailure {
    /// The owner's holdings could not be read (the cause is logged).
    Fetch(String),
    /// The provider name a route asked for is not an approved provider.
    ThirdPartyNotFound(String),
}

impl FetchFailure {
    /// The old lambdas' answer: 502, `Cannot fetch elements for <address>`
    /// or `Third Party not found: <name>`.
    pub(crate) fn response(&self, address: &str) -> Response {
        match self {
            Self::Fetch(cause) => fetch_error(address, cause),
            Self::ThirdPartyNotFound(name) => super::lambdas_error(
                axum::http::StatusCode::BAD_GATEWAY,
                FETCHER_ERROR,
                &format!("Third Party not found: {name}"),
            ),
        }
    }
}

/// The provider name of a registry id (`urn:decentraland:<network>:collections-thirdparty:<name>`).
fn provider_name(id: &str) -> Option<&str> {
    id.split(':').nth(4)
}

/// Every third-party wearable `owner` holds through a mapped NFT, optionally
/// only the provider named `only_name` (the URN's fifth part). With a
/// registry configured, a name no approved provider has is
/// `ThirdPartyNotFound`, as the old lambdas threw. A contract that could not
/// be read fails the fetch or is left out, per `on_failure`. An `owner` that
/// is not an Ethereum address holds nothing (lamb2 takes any `:address`)
/// and no holdings reader is asked.
pub async fn fetch_all_third_party_wearables(
    state: &MarketplaceState,
    owner: &str,
    only_name: Option<&str>,
    on_failure: OnFailure,
) -> Result<Vec<TpwElement>, FetchFailure> {
    let owner = owner.to_lowercase();
    let mut providers = third_party_providers(state)
        .await
        .map_err(FetchFailure::Fetch)?;
    if let Some(name) = only_name {
        providers.retain(|p| provider_name(&p.id) == Some(name));
        if providers.is_empty() && state.settings.third_party_registry_subgraph_url.is_some() {
            return Err(FetchFailure::ThirdPartyNotFound(name.to_string()));
        }
    }
    providers.retain(|p| !p.contracts.is_empty());
    if providers.is_empty() || !super::is_valid_eth_address(&owner) {
        return Ok(Vec::new());
    }
    let mut contracts_by_network: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for p in &providers {
        for c in &p.contracts {
            contracts_by_network
                .entry(c.network.clone())
                .or_default()
                .insert(c.address.clone());
        }
    }
    let (lists, holdings) = match state.settings.nft_worker_base_url.as_deref() {
        Some(base) => {
            let holdings = owned_via_nft_worker(state, base, &owner, &contracts_by_network).await;
            // Only the providers whose contracts returned a token are read.
            let hit: Vec<&str> = providers
                .iter()
                .filter(|p| {
                    p.contracts.iter().any(|c| {
                        let prefix = format!("{}:{}:", c.network, c.address);
                        holdings.owned.iter().any(|nft| nft.starts_with(&prefix))
                    })
                })
                .map(|p| p.id.as_str())
                .collect();
            (
                collection_entity_lists(state, hit.into_iter()).await,
                holdings,
            )
        }
        None => {
            let lists =
                collection_entity_lists(state, providers.iter().map(|p| p.id.as_str())).await;
            let mut candidates = candidates_from_mappings(lists.iter().flat_map(|l| l.iter()));
            candidates.retain(|(network, contract), _| {
                contracts_by_network
                    .get(network)
                    .is_some_and(|cs| cs.contains(contract))
            });
            (lists, owned_via_rpc(state, &owner, &candidates).await)
        }
    };
    if !holdings.failed.is_empty() {
        match on_failure {
            OnFailure::Fail => return Err(FetchFailure::Fetch(holdings.failed.join("; "))),
            OnFailure::Skip => tracing::warn!(
                owner = %owner,
                failed = ?holdings.failed,
                "third-party holdings: leaving out what could not be read"
            ),
        }
    }
    if holdings.owned.is_empty() {
        return Ok(Vec::new());
    }
    Ok(elements_from(&lists, &holdings.owned))
}

fn elements_from(lists: &[Arc<Vec<Value>>], owned_nft_urns: &[String]) -> Vec<TpwElement> {
    let mut by_urn: HashMap<String, TpwElement> = HashMap::new();
    let mut order: Vec<String> = Vec::new();
    for entity in lists.iter().flat_map(|l| l.iter()) {
        let Some(meta) = entity.get("metadata") else {
            continue;
        };
        let mappings = match meta.get("mappings") {
            Some(m) if !m.is_null() => m,
            _ => continue,
        };
        let Some(entity_urn) = meta.get("id").and_then(|v| v.as_str()).map(str::to_string) else {
            continue;
        };
        for nft in owned_nft_urns {
            let parts: Vec<&str> = nft.split(':').collect();
            if parts.len() < 3 {
                continue;
            }
            let (network, contract, token_id) = (parts[0], parts[1], parts[2]);
            if !mappings_includes_nft(mappings, network, contract, token_id) {
                continue;
            }
            let elem = by_urn.entry(entity_urn.clone()).or_insert_with(|| {
                order.push(entity_urn.clone());
                TpwElement {
                    urn: entity_urn.clone(),
                    individual_data: Vec::new(),
                    name: meta
                        .get("name")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    category: meta
                        .get("data")
                        .and_then(|d| d.get("category"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    entity: entity.clone(),
                }
            });
            elem.individual_data
                .push(json!({ "id": format!("{}:{}", entity_urn, nft), "tokenId": nft }));
        }
    }
    order
        .into_iter()
        .filter_map(|u| by_urn.remove(&u))
        .collect()
}

async fn collection_entity_lists<'a>(
    state: &MarketplaceState,
    providers: impl Iterator<Item = &'a str>,
) -> Vec<Arc<Vec<Value>>> {
    futures::future::join_all(providers.map(|id| {
        let id = id.to_string();
        async move {
            state
                .third_party
                .collection_entities
                .get_or_fetch(id.clone(), || async {
                    Ok::<_, ()>(Arc::new(fetch_collection_entities(state, &id).await))
                })
                .await
                .unwrap_or_default()
        }
    }))
    .await
}

fn has_mappings(entity: &Value) -> bool {
    entity
        .get("metadata")
        .and_then(|m| m.get("mappings"))
        .map(|m| !m.is_null())
        .unwrap_or(false)
}

/// The provider's active entities that carry mappings, 1,000 per page in
/// pointer order.
async fn fetch_collection_entities(state: &MarketplaceState, collection_id: &str) -> Vec<Value> {
    const PAGE: i64 = 1000;
    const PAGE_CONCURRENCY: usize = 4;
    use futures::stream::StreamExt;
    // A page is short by its own length; its denylisted entities leave it after.
    let Ok(mut first) = active_entity_docs_by_prefix(&state.pool, collection_id, 0, PAGE).await
    else {
        return Vec::new();
    };
    let short = (first.entities.len() as i64) < PAGE;
    retain_non_denylisted(&mut first.entities, &state.denylist);
    let mut out: Vec<Value> = first
        .entities
        .iter()
        .filter_map(|d| d.to_value())
        .filter(has_mappings)
        .collect();
    if short || first.total <= PAGE {
        return out;
    }
    let rest = (PAGE..first.total)
        .step_by(PAGE as usize)
        .map(|offset| active_entity_docs_by_prefix(&state.pool, collection_id, offset, PAGE));
    let mut pages = futures::stream::iter(rest).buffered(PAGE_CONCURRENCY);
    while let Some(page) = pages.next().await {
        let Ok(mut page) = page else {
            break;
        };
        let short = (page.entities.len() as i64) < PAGE;
        retain_non_denylisted(&mut page.entities, &state.denylist);
        out.extend(
            page.entities
                .iter()
                .filter_map(|d| d.to_value())
                .filter(has_mappings),
        );
        if short {
            break;
        }
    }
    out
}

fn tpw_passes_filter(e: &TpwElement, categories: &[String], name: &Option<String>) -> bool {
    if !categories.is_empty() && !categories.contains(&e.category) {
        return false;
    }
    if let Some(n) = name
        && !e.name.to_lowercase().contains(&n.to_lowercase())
    {
        return false;
    }
    true
}

/// lamb2 pages these routes with `paginationObject(url, Number.MAX_VALUE)`
/// (decentraland/lamb2:src/controllers/handlers/third-party-wearables-handler.ts:42,
/// 100): no `pageSize` is too large.
fn pagination_object(
    query_string: &str,
) -> Result<(crate::shared::query_params::QueryParams, JsPagination), Response> {
    let params = parse_query_string(query_string);
    let pagination = parse_pagination_js(&params, i64::MAX).map_err(|msg| bad_request(&msg))?;
    Ok((params, pagination))
}

/// `GET /lambdas/users/{address}/third-party-wearables`.
pub async fn user_third_party_wearables(
    State(state): State<SharedState>,
    Path(address): Path<String>,
    request: Request,
) -> Response {
    let (params, pagination) = match pagination_object(request.uri().query().unwrap_or("")) {
        Ok(p) => p,
        Err(resp) => return resp,
    };
    let categories: Vec<String> = qs_get_array(&params, "category");
    let name = qs_get_string(&params, "name");
    let sort = qs_get_string(&params, "orderBy")
        .map(|s| s.to_lowercase())
        .unwrap_or_else(|| "name".to_string());
    let direction = qs_get_string(&params, "direction")
        .map(|s| s.to_uppercase())
        .unwrap_or_else(|| {
            if sort == "name" {
                "ASC".into()
            } else {
                "DESC".into()
            }
        });
    if !(sort == "name" && (direction == "ASC" || direction == "DESC")) {
        return bad_request(&format!("Invalid sorting requested: {sort} {direction}"));
    }
    let mut elements =
        match fetch_all_third_party_wearables(&state, &address, None, OnFailure::Fail).await {
            Ok(elements) => elements,
            Err(e) => return e.response(&address),
        };
    elements.retain(|e| tpw_passes_filter(e, &categories, &name));
    elements.sort_by(|a, b| {
        let primary = locale_cmp(&a.name, &b.name).then_with(|| locale_cmp(&a.urn, &b.urn));
        if direction == "DESC" {
            primary.reverse()
        } else {
            primary
        }
    });
    let include_definitions = params.contains_key("includeDefinitions");
    finish_tpw_response(&state, elements, &pagination, include_definitions)
}

/// `GET /lambdas/users/{address}/third-party-wearables/{collectionId}`.
pub async fn user_third_party_collection_wearables(
    State(state): State<SharedState>,
    Path((address, collection_id)): Path<(String, String)>,
    request: Request,
) -> Response {
    let cleaned: String = collection_id
        .split(':')
        .take(5)
        .collect::<Vec<_>>()
        .join(":");
    if !is_third_party_name_urn(&cleaned) {
        return bad_request(&format!(
            "Invalid collection id: {collection_id} not a valid URN"
        ));
    }
    let name = cleaned.split(':').nth(4).unwrap_or("").to_string();
    let (params, pagination) = match pagination_object(request.uri().query().unwrap_or("")) {
        Ok(p) => p,
        Err(resp) => return resp,
    };
    // The fetcher's order, unsorted, as lamb2 pages it
    // (decentraland/lamb2:src/controllers/handlers/third-party-wearables-handler.ts:102-105).
    let elements =
        match fetch_all_third_party_wearables(&state, &address, Some(&name), OnFailure::Fail).await
        {
            Ok(elements) => elements,
            Err(e) => return e.response(&address),
        };
    let include_definitions = params.contains_key("includeDefinitions");
    finish_tpw_response(&state, elements, &pagination, include_definitions)
}

fn finish_tpw_response(
    state: &MarketplaceState,
    elements: Vec<TpwElement>,
    pagination: &JsPagination,
    include_definitions: bool,
) -> Response {
    let total = elements.len();
    let (start, end) = pagination.slice_range(total);
    let page = &elements[start..end];
    let content_public_url = &state.settings.content_public_url;
    let out: Vec<Value> = page
        .iter()
        .map(|e| {
            let mut v = e.to_value();
            // A missing definition leaves the key out, as lamb2's `undefined`
            // does (decentraland/lamb2:src/controllers/handlers/third-party-wearables-handler.ts:60-65).
            if include_definitions
                && let Some(def) = extract_wearable_definition(&e.entity, content_public_url)
                && let Some(obj) = v.as_object_mut()
            {
                obj.insert("definition".to_string(), def);
            }
            v
        })
        .collect();
    Json(json!({
        "elements": out,
        "totalAmount": total,
        "pageNum": pagination.page_num,
        "pageSize": pagination.page_size,
    }))
    .into_response()
}

/// One `/third-party-integrations` entry: `{name, description, urn}`, a
/// key the registry left out omitted as lamb2's `undefined` is.
fn integration_of(p: &ThirdPartyProvider) -> Value {
    let mut m = serde_json::Map::new();
    if let Some(name) = &p.name {
        m.insert("name".into(), name.clone());
    }
    if let Some(description) = &p.description {
        m.insert("description".into(), description.clone());
    }
    m.insert("urn".into(), json!(p.id));
    Value::Object(m)
}

/// `GET /lambdas/third-party-integrations`: `{data: [...]}` over the
/// providers list the other third-party routes read (the registry's
/// approved providers, its default page of 100), empty without a registry
/// subgraph (decentraland/lamb2:src/controllers/handlers/third-party-wearables-handler.ts:143-155).
/// A failed registry read serves the last list, and with none is lamb2's
/// 502 `Cannot fetch third party providers`
/// (decentraland/lamb2:src/logic/third-party-providers-storage.ts:46-52).
pub async fn third_party_integrations(State(state): State<SharedState>) -> Response {
    match third_party_providers(&state).await {
        Ok(providers) => {
            let data: Vec<Value> = providers.iter().map(integration_of).collect();
            Json(json!({ "data": data })).into_response()
        }
        Err(e) => {
            tracing::error!("third-party-integrations failed: {e}");
            super::lambdas_error(
                axum::http::StatusCode::BAD_GATEWAY,
                FETCHER_ERROR,
                "Cannot fetch third party providers",
            )
        }
    }
}

/// The owned third-party URNs of a collection, for the by-owner lists.
pub(crate) async fn owned_third_party_urns(
    state: &MarketplaceState,
    owner: &str,
    provider_name: &str,
) -> Result<Vec<(String, usize, Value)>, FetchFailure> {
    Ok(
        fetch_all_third_party_wearables(state, owner, Some(provider_name), OnFailure::Fail)
            .await?
            .into_iter()
            .map(|e| (e.urn, e.individual_data.len(), e.entity))
            .collect(),
    )
}

/// A worn URN naming a third-party collection (it contains
/// `collections-thirdparty`, the old lambdas' test), as their checker split
/// them: only items of an L2 collection (`matic`, `amoy`) count. The one
/// classifier the profiles and the outfits share.
#[derive(Debug, PartialEq, Eq)]
enum WornThirdParty {
    /// `urn:decentraland:<l2>:collections-thirdparty:<tp>:<collection>:<item>`:
    /// kept unchecked.
    V1,
    /// `...:<item>:<nftChain>:<contract>:<tokenId>`: a linked item, kept when
    /// its NFT is owned.
    Linked(LinkedItem),
    /// Any other shape: never kept.
    Unrecognised,
}

#[derive(Debug, PartialEq, Eq)]
struct LinkedItem {
    /// The item's own URN (the first seven parts), its entity's `metadata.id`.
    asset_urn: String,
    network: String,
    contract: String,
    token_id: String,
    /// The NFT lives on mainnet or sepolia.
    on_l1: bool,
}

/// `None` when `urn` is not a third-party item.
fn worn_third_party(urn: &str) -> Option<WornThirdParty> {
    if !urn.contains("collections-thirdparty") {
        return None;
    }
    let p: Vec<&str> = urn.split(':').collect();
    let is = |i: usize, want: &str| p.get(i).is_some_and(|s| s.eq_ignore_ascii_case(want));
    if !(is(0, "urn") && is(1, "decentraland") && is(3, "collections-thirdparty"))
        || !(is(2, "matic") || is(2, "amoy"))
        || p[4..].iter().any(|s| s.is_empty())
    {
        return Some(WornThirdParty::Unrecognised);
    }
    match p.len() {
        7 => Some(WornThirdParty::V1),
        10 => {
            let network = p[7].to_lowercase();
            let on_l1 = match network.as_str() {
                "mainnet" | "sepolia" => true,
                "matic" | "amoy" => false,
                _ => return Some(WornThirdParty::Unrecognised),
            };
            Some(WornThirdParty::Linked(LinkedItem {
                asset_urn: p[..7].join(":"),
                network,
                contract: p[8].to_lowercase(),
                token_id: p[9].to_string(),
                on_l1,
            }))
        }
        _ => Some(WornThirdParty::Unrecognised),
    }
}

/// Whether a worn URN is a third-party item, to be checked by
/// `sanitize_third_party_urns`.
pub(crate) fn is_third_party_urn(urn: &str) -> bool {
    worn_third_party(urn).is_some()
}

/// The v1 items among worn URNs, which need no check: what a profile keeps
/// of its third-party items when its linked items could not be read.
pub(crate) fn v1_third_party_urns(urns: &[String]) -> Vec<String> {
    urns.iter()
        .filter(|u| worn_third_party(u) == Some(WornThirdParty::V1))
        .cloned()
        .collect()
}

/// The worn third-party URNs `owner` may keep, in the order the old lambdas
/// answered them (`tpw-ownership-checker.ts`): the v1 items unchecked, then
/// the linked items whose NFT is on L1, then those on L2. A linked item is
/// kept when its entity's mappings include the NFT and the chain confirms it
/// (`ownerOf` is the owner, or an ERC-1155 `balanceOf` above zero); any
/// other URN is not kept. A linked item on a network with no RPC endpoint
/// reads as not owned (logged). `Err` when an endpoint failed.
pub async fn sanitize_third_party_urns(
    state: &MarketplaceState,
    owner: &str,
    urns: &[String],
) -> Result<Vec<String>, String> {
    let mut v1 = Vec::new();
    let mut linked: Vec<(&String, LinkedItem)> = Vec::new();
    for urn in urns {
        match worn_third_party(urn) {
            Some(WornThirdParty::V1) => v1.push(urn.clone()),
            Some(WornThirdParty::Linked(item)) => linked.push((urn, item)),
            Some(WornThirdParty::Unrecognised) | None => {}
        }
    }
    let items: Vec<&LinkedItem> = linked.iter().map(|(_, item)| item).collect();
    let owned = linked_items_owned(state, owner, &items).await?;
    let kept = |l1: bool| {
        linked
            .iter()
            .zip(&owned)
            .filter(move |((_, item), owned)| item.on_l1 == l1 && **owned)
            .map(|((urn, _), _)| (*urn).clone())
    };
    Ok(v1
        .into_iter()
        .chain(kept(true))
        .chain(kept(false))
        .collect())
}

/// Per item, whether `owner` holds its NFT (`third-party-item-checker.ts`).
async fn linked_items_owned(
    state: &MarketplaceState,
    owner: &str,
    items: &[&LinkedItem],
) -> Result<Vec<bool>, String> {
    let Ok(owner) = Address::from_str(owner) else {
        return Ok(vec![false; items.len()]);
    };
    if items.is_empty() {
        return Ok(Vec::new());
    }
    let mut pointers: Vec<String> = items.iter().map(|i| i.asset_urn.to_lowercase()).collect();
    pointers.sort();
    pointers.dedup();
    let mut docs = active_entity_docs_by_pointers(&state.pool, &pointers)
        .await
        .map_err(|e| format!("third-party item entities: {e}"))?;
    retain_non_denylisted(&mut docs, &state.denylist);
    let mappings: HashMap<String, Value> = docs
        .iter()
        .filter_map(|d| d.to_value())
        .filter_map(|e| {
            let id = e.pointer("/metadata/id")?.as_str()?.to_string();
            Some((id, e.pointer("/metadata/mappings")?.clone()))
        })
        .collect();
    confirm_linked_items(state, owner, items, &mappings).await
}

/// Per item, whether its entity's `mappings` include its NFT and the chain
/// confirms `owner` holds it.
async fn confirm_linked_items(
    state: &MarketplaceState,
    owner: Address,
    items: &[&LinkedItem],
    mappings: &HashMap<String, Value>,
) -> Result<Vec<bool>, String> {
    let mut owned = vec![false; items.len()];
    // The mapped items, grouped by the network their NFT is on.
    let mut by_network: BTreeMap<&str, Vec<(usize, Address, U256)>> = BTreeMap::new();
    for (i, item) in items.iter().enumerate() {
        let mapped = mappings.get(&item.asset_urn).is_some_and(|m| {
            mappings_includes_nft(m, &item.network, &item.contract, &item.token_id)
        });
        let (Ok(contract), Ok(id)) = (
            Address::from_str(&item.contract),
            U256::from_str_radix(&item.token_id, 10),
        ) else {
            continue;
        };
        if mapped {
            by_network
                .entry(&item.network)
                .or_default()
                .push((i, contract, id));
        }
    }
    for (network, checks) in by_network {
        let Some(rpc) = rpc_url_for(state, network) else {
            tracing::warn!(
                network,
                items = checks.len(),
                "third-party item ownership: no RPC endpoint for this network; reading the items as not owned"
            );
            continue;
        };
        let contracts: Vec<Address> = checks
            .iter()
            .map(|(_, c, _)| *c)
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        let probes: Vec<RpcCall> = contracts
            .iter()
            .map(|c| supports_call(*c, IFACE_ERC1155))
            .collect();
        let answers = eth_call_batch(state, &rpc, &probes).await?;
        let erc1155: BTreeSet<Address> = contracts
            .iter()
            .zip(&answers)
            .filter(|(_, a)| decode_bool(Some(a)))
            .map(|(c, _)| *c)
            .collect();
        let calls: Vec<RpcCall> = checks
            .iter()
            .map(|(_, contract, id)| RpcCall {
                to: *contract,
                data: if erc1155.contains(contract) {
                    IERC1155::balanceOfCall {
                        account: owner,
                        id: *id,
                    }
                    .abi_encode()
                    .into()
                } else {
                    IERC721::ownerOfCall { tokenId: *id }.abi_encode().into()
                },
            })
            .collect();
        let answers = eth_call_batch(state, &rpc, &calls).await?;
        for ((i, contract, _), answer) in checks.iter().zip(answers) {
            owned[*i] = answer.as_ref().is_some_and(|b| {
                if erc1155.contains(contract) {
                    IERC1155::balanceOfCall::abi_decode_returns(b).is_ok_and(|n| !n.is_zero())
                } else {
                    IERC721::ownerOfCall::abi_decode_returns(b).is_ok_and(|h| h == owner)
                }
            });
        }
    }
    Ok(owned)
}

/// The old lambdas' `FetcherError` label.
const FETCHER_ERROR: &str = "The requested items cannot be fetched right now";

/// The old lambdas' `FetcherError` answer: 502 `Cannot fetch elements for
/// <address>`, the cause logged.
pub(crate) fn fetch_error(address: &str, cause: &str) -> Response {
    tracing::warn!(error = %cause, address, "ownership read failed");
    super::lambdas_error(
        axum::http::StatusCode::BAD_GATEWAY,
        FETCHER_ERROR,
        &format!("Cannot fetch elements for {address}"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy_sol_types::SolValue;

    fn entity(mappings: Value) -> Value {
        json!({ "metadata": { "mappings": mappings } })
    }

    fn mappings_with(entry: Value) -> Value {
        json!({ "amoy": { "0xcontract": [entry] } })
    }

    #[test]
    fn single_matches_exact_id_only() {
        let m = mappings_with(json!({ "type": "single", "id": "42" }));
        assert!(mappings_includes_nft(&m, "amoy", "0xcontract", "42"));
        assert!(!mappings_includes_nft(&m, "amoy", "0xcontract", "43"));
    }

    #[test]
    fn any_matches_everything() {
        let m = mappings_with(json!({ "type": "any" }));
        assert!(mappings_includes_nft(&m, "amoy", "0xcontract", "0"));
        assert!(mappings_includes_nft(&m, "amoy", "0xcontract", "999999"));
    }

    #[test]
    fn multiple_matches_membership() {
        let m = mappings_with(json!({ "type": "multiple", "ids": ["1", "5", "9"] }));
        assert!(mappings_includes_nft(&m, "amoy", "0xcontract", "5"));
        assert!(!mappings_includes_nft(&m, "amoy", "0xcontract", "6"));
    }

    #[test]
    fn range_inclusive_endpoints_and_wide_token_ids() {
        let m = mappings_with(json!({ "type": "range", "from": "10", "to": "20" }));
        assert!(mappings_includes_nft(&m, "amoy", "0xcontract", "10"));
        assert!(mappings_includes_nft(&m, "amoy", "0xcontract", "20"));
        assert!(mappings_includes_nft(&m, "amoy", "0xcontract", "15"));
        assert!(!mappings_includes_nft(&m, "amoy", "0xcontract", "9"));
        assert!(!mappings_includes_nft(&m, "amoy", "0xcontract", "21"));
        let wide = "115792089237316195423570985008687907853269984665640564039457584007913129639935";
        let m = mappings_with(json!({ "type": "range", "from": "1", "to": wide }));
        assert!(
            mappings_includes_nft(&m, "amoy", "0xcontract", wide),
            "a token id past u128 still compares"
        );
    }

    #[test]
    fn candidates_follow_every_mapping_type() {
        let entities = [
            entity(json!({ "mainnet": { "0xAbC": [
                { "type": "single", "id": "4" },
                { "type": "multiple", "ids": ["502", "0503"] },
                { "type": "range", "from": "10", "to": "12" },
            ] } })),
            entity(
                json!({ "mainnet": { "0xabc": [{ "type": "single", "id": "7" }] }, "matic": { "0xdef": [{ "type": "any" }] } }),
            ),
            entity(
                json!({ "matic": { "0x123": [{ "type": "range", "from": "1", "to": "1000000" }] } }),
            ),
            json!({ "metadata": {} }),
        ];
        let got = candidates_from_mappings(entities.iter());
        let abc = &got[&("mainnet".into(), "0xabc".into())];
        let ids: Vec<&str> = abc.ids.iter().map(String::as_str).collect();
        assert_eq!(ids, ["0503", "10", "11", "12", "4", "502", "7"]);
        assert!(!abc.open);
        assert!(got[&("matic".into(), "0xdef".into())].open);
        let wide = &got[&("matic".into(), "0x123".into())];
        assert!(wide.open && wide.ids.is_empty());
        assert_eq!(got.len(), 3);
    }

    #[test]
    fn aggregate3_answers_map_back_per_subcall() {
        let holder = address!("0x29fb0d1b0836f9963f963b0bb07c49d2d61370b4");
        let results = vec![
            IMulticall3::Result3 {
                success: true,
                returnData: holder.abi_encode().into(),
            },
            IMulticall3::Result3 {
                success: false,
                returnData: Bytes::new(),
            },
            IMulticall3::Result3 {
                success: true,
                returnData: U256::from(4576).abi_encode().into(),
            },
        ];
        let blob = Bytes::from(results.abi_encode());
        let decoded = decode_aggregate3(Some(&blob), 3).unwrap();
        assert_eq!(decoded.len(), 3);
        let owner = IERC721::ownerOfCall::abi_decode_returns(decoded[0].as_ref().unwrap()).unwrap();
        assert_eq!(owner, holder);
        assert!(decoded[1].is_none());
        let id = IERC721::tokenOfOwnerByIndexCall::abi_decode_returns(decoded[2].as_ref().unwrap())
            .unwrap();
        assert_eq!(id.to_string(), "4576");
        assert_eq!(
            decode_aggregate3(Some(&blob), 2),
            None,
            "a short answer is a failed multicall, not reverted sub-calls"
        );
        assert_eq!(decode_aggregate3(None, 1), None);
    }

    #[test]
    fn every_contract_network_has_a_chain() {
        assert_eq!(chain_id_of("mainnet"), Some(1));
        assert_eq!(chain_id_of("sepolia"), Some(11155111));
        assert_eq!(chain_id_of("matic"), Some(137));
        assert_eq!(chain_id_of("amoy"), Some(80002));
        assert!(chain_id_of("ethereum").is_none());
        assert!(chain_id_of("").is_none());
    }

    const OWNER: Address = address!("0x0000000000000000000000000000000000000001");
    const OTHER: Address = address!("0x0000000000000000000000000000000000000002");
    const PLAIN_721: Address = address!("0x00000000000000000000000000000000000000a1");
    /// A plain ERC-721 the owner holds none of; its `ownerOf` would answer
    /// the owner, so a call to it shows in the holdings.
    const EMPTY_721: Address = address!("0x00000000000000000000000000000000000000a0");
    const ENUM_721: Address = address!("0x00000000000000000000000000000000000000e1");
    const MULTI_1155: Address = address!("0x0000000000000000000000000000000000001155");
    /// A plain ERC-721 whose multicalls the endpoint throttles (-32005).
    const THROTTLED_721: Address = address!("0x00000000000000000000000000000000000000f1");
    /// The fake chain's head: a polygon-sized history no log scan could cover.
    const HEAD: u64 = 80_000_000;
    /// Past one request's 2,000 tokens, so the walk is paged.
    const ENUM_BALANCE: u64 = 4500;

    fn word(v: U256) -> Bytes {
        Bytes::from(v.abi_encode())
    }

    /// A contract call on the fake chain; `None` reverts.
    fn chain_call(to: Address, data: &[u8]) -> Option<Bytes> {
        let selector: [u8; 4] = data.get(..4)?.try_into().ok()?;
        match selector {
            IERC165::supportsInterfaceCall::SELECTOR => {
                let iface = IERC165::supportsInterfaceCall::abi_decode(data)
                    .ok()?
                    .interfaceId;
                let yes = (to == ENUM_721 && iface.0 == IFACE_ERC721_ENUMERABLE)
                    || (to == MULTI_1155 && iface.0 == IFACE_ERC1155);
                Some(Bytes::from(yes.abi_encode()))
            }
            IERC721::balanceOfCall::SELECTOR if to == PLAIN_721 || to == THROTTLED_721 => {
                Some(word(U256::from(1)))
            }
            IERC721::balanceOfCall::SELECTOR if to == EMPTY_721 => Some(word(U256::ZERO)),
            IERC721::balanceOfCall::SELECTOR if to == ENUM_721 => {
                Some(word(U256::from(ENUM_BALANCE)))
            }
            IERC721::ownerOfCall::SELECTOR if to == EMPTY_721 => {
                Some(Bytes::from(OWNER.abi_encode()))
            }
            IERC721::ownerOfCall::SELECTOR if to == PLAIN_721 => {
                let id = IERC721::ownerOfCall::abi_decode(data).ok()?.tokenId;
                let holder = if id == U256::from(7) { OWNER } else { OTHER };
                Some(Bytes::from(holder.abi_encode()))
            }
            IERC721::tokenOfOwnerByIndexCall::SELECTOR if to == ENUM_721 => {
                let i = IERC721::tokenOfOwnerByIndexCall::abi_decode(data)
                    .ok()?
                    .index;
                (i < U256::from(ENUM_BALANCE)).then(|| word(U256::from(1000) + i))
            }
            IERC1155::balanceOfBatchCall::SELECTOR if to == MULTI_1155 => {
                let call = IERC1155::balanceOfBatchCall::abi_decode(data).ok()?;
                let balances: Vec<U256> = call
                    .ids
                    .iter()
                    .map(|id| match id.to::<u64>() {
                        11 => U256::from(1),
                        13 => U256::from(2),
                        _ => U256::ZERO,
                    })
                    .collect();
                Some(Bytes::from(balances.abi_encode()))
            }
            IMulticall3::aggregate3Call::SELECTOR if to == MULTICALL3 => {
                let call = IMulticall3::aggregate3Call::abi_decode(data).ok()?;
                assert!(
                    call.calls.len() <= MULTICALL_CHUNK,
                    "a multicall holds at most {MULTICALL_CHUNK} sub-calls"
                );
                let results: Vec<IMulticall3::Result3> = call
                    .calls
                    .iter()
                    .map(|c| match chain_call(c.target, &c.callData) {
                        Some(ret) => IMulticall3::Result3 {
                            success: true,
                            returnData: ret,
                        },
                        None => IMulticall3::Result3 {
                            success: false,
                            returnData: Bytes::new(),
                        },
                    })
                    .collect();
                Some(Bytes::from(
                    IMulticall3::aggregate3Call::abi_encode_returns(&results),
                ))
            }
            _ => None,
        }
    }

    fn throttled(data: &[u8]) -> bool {
        IMulticall3::aggregate3Call::abi_decode(data)
            .is_ok_and(|call| call.calls.iter().any(|c| c.target == THROTTLED_721))
    }

    fn chain_answer(req: &Value) -> Value {
        let id = req["id"].clone();
        let error = |code: i64, msg: &str| json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": msg } });
        let result = |r: Value| json!({ "jsonrpc": "2.0", "id": id, "result": r });
        let p = &req["params"][0];
        match req["method"].as_str() {
            Some("eth_blockNumber") => result(json!(format!("0x{HEAD:x}"))),
            Some("eth_call") => {
                let to: Address = p["to"].as_str().unwrap().parse().unwrap();
                let data =
                    hex::decode(p["data"].as_str().unwrap().trim_start_matches("0x")).unwrap();
                if throttled(&data) {
                    return error(-32005, "rate limit exceeded");
                }
                match chain_call(to, &data) {
                    Some(ret) => result(json!(ret)),
                    None => error(3, "execution reverted"),
                }
            }
            // `eth_getLogs` included: no read scans the history.
            other => panic!("unexpected JSON-RPC method {other:?}"),
        }
    }

    async fn serve(router: axum::Router) -> String {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
        format!("http://{addr}/")
    }

    /// The fake chain, and the count of JSON-RPC requests (HTTP posts) it took.
    async fn serve_chain() -> (String, Arc<std::sync::atomic::AtomicUsize>) {
        use axum::routing::post;
        use std::sync::atomic::{AtomicUsize, Ordering};
        let posts = Arc::new(AtomicUsize::new(0));
        let counted = posts.clone();
        let url = serve(axum::Router::new().route(
            "/",
            post(move |Json(body): Json<Value>| {
                counted.fetch_add(1, Ordering::SeqCst);
                async move {
                    Json(match &body {
                        Value::Array(reqs) => Value::Array(reqs.iter().map(chain_answer).collect()),
                        req => chain_answer(req),
                    })
                }
            }),
        ))
        .await;
        (url, posts)
    }

    fn state_with(settings: crate::market::Settings) -> crate::market::SharedState {
        let state = crate::market::tests::support::state_without_db_settings(settings);
        state
            .test_allow_loopback
            .store(true, std::sync::atomic::Ordering::Relaxed);
        state
    }

    fn with_rpcs(rpcs: &str) -> crate::market::Settings {
        crate::market::Settings {
            market: crate::market::MarketSettings {
                trade_rpc_urls: crate::market::listings::RpcEndpoints::from_env(Some(rpcs)),
                ..crate::market::MarketSettings::default()
            },
            ..crate::market::Settings::for_tests()
        }
    }

    fn candidate(
        network: &str,
        contract: Address,
        ids: &[&str],
        open: bool,
    ) -> ((String, String), Candidates) {
        (
            (network.to_string(), contract.to_string().to_lowercase()),
            Candidates {
                ids: ids.iter().map(|s| s.to_string()).collect(),
                open,
            },
        )
    }

    fn triple(network: &str, contract: Address, id: u64) -> String {
        format!("{network}:{}:{id}", contract.to_string().to_lowercase())
    }

    #[tokio::test]
    async fn each_contract_network_reads_its_own_endpoint() {
        let mut s = with_rpcs("11155111=https://sepolia.example,80002=https://amoy.example");
        s.eth_rpc_url = Some("https://l1.example".into());
        s.polygon_rpc_url = Some("https://l2.example".into());
        let state = state_with(s.clone());
        assert_eq!(
            rpc_url_for(&state, "mainnet").as_deref(),
            Some("https://l1.example")
        );
        assert_eq!(
            rpc_url_for(&state, "matic").as_deref(),
            Some("https://l2.example")
        );
        assert_eq!(
            rpc_url_for(&state, "sepolia").as_deref(),
            Some("https://sepolia.example")
        );
        assert_eq!(
            rpc_url_for(&state, "amoy").as_deref(),
            Some("https://amoy.example")
        );
        assert_eq!(rpc_url_for(&state, "ethereum"), None);
        let testnet = state_with(crate::market::Settings {
            eth_network: "sepolia".into(),
            market: crate::market::MarketSettings::default(),
            ..s
        });
        assert_eq!(
            rpc_url_for(&testnet, "sepolia").as_deref(),
            Some("https://l1.example"),
            "ETH_RPC_URL serves ETH_NETWORK"
        );
        assert_eq!(
            rpc_url_for(&testnet, "amoy").as_deref(),
            Some("https://l2.example")
        );
        assert_eq!(rpc_url_for(&testnet, "mainnet"), None);
        assert_eq!(rpc_url_for(&testnet, "matic"), None);
    }

    #[tokio::test]
    async fn holdings_never_scan_logs_and_leave_open_mappings_unresolved() {
        let (rpc, posts) = serve_chain().await;
        let state = state_with(with_rpcs(&format!("11155111={rpc},80002={rpc}")));
        let candidates: CandidateMap = [
            // `any` on a plain ERC-721 the owner holds one of: only the
            // mapped ids are read (7 held, 3 not); the rest is unresolved.
            candidate("amoy", PLAIN_721, &["3", "7"], true),
            // A zero balance: nothing owned, no `ownerOf`.
            candidate("amoy", EMPTY_721, &["1"], true),
            // An enumerable ERC-721, walked whole, 2,000 tokens a request.
            candidate("amoy", ENUM_721, &[], true),
            // `any` on an ERC-1155: the mapped ids only (13 is held but
            // named by no mapping).
            candidate("sepolia", MULTI_1155, &["11", "12", "14"], true),
        ]
        .into_iter()
        .collect();
        let holdings = owned_via_rpc(&state, &OWNER.to_string(), &candidates).await;
        assert_eq!(holdings.failed, Vec::<String>::new());
        let owned = holdings.owned;
        assert!(owned.contains(&triple("amoy", PLAIN_721, 7)), "{owned:?}");
        assert!(
            owned.contains(&triple("sepolia", MULTI_1155, 11)),
            "{owned:?}"
        );
        let enumerated = owned
            .iter()
            .filter(|o| o.contains(&ENUM_721.to_string().to_lowercase()))
            .count();
        assert_eq!(
            enumerated as u64, ENUM_BALANCE,
            "the whole balance is walked"
        );
        assert_eq!(owned.len() as u64, 2 + ENUM_BALANCE, "{:?}", &owned[..5]);
        // amoy: the probes, PLAIN_721's `ownerOf` and the walk's three pages;
        // sepolia: the probes and one `balanceOfBatch`. No `eth_getLogs`
        // (`chain_answer` refuses it), however long the chain.
        assert_eq!(posts.load(std::sync::atomic::Ordering::SeqCst), 7);
    }

    #[tokio::test]
    async fn a_failing_contract_fails_alone_and_is_remembered_briefly() {
        let (rpc, posts) = serve_chain().await;
        let state = state_with(with_rpcs(&format!("80002={rpc}")));
        let candidates: CandidateMap = [
            candidate("amoy", PLAIN_721, &["7"], false),
            candidate("amoy", THROTTLED_721, &["7"], false),
        ]
        .into_iter()
        .collect();
        let owner = OWNER.to_string();
        let holdings = owned_via_rpc(&state, &owner, &candidates).await;
        assert_eq!(holdings.owned, vec![triple("amoy", PLAIN_721, 7)]);
        assert_eq!(holdings.failed.len(), 1, "{:?}", holdings.failed);
        assert!(
            holdings.failed[0].contains(&THROTTLED_721.to_string().to_lowercase())
                && holdings.failed[0].contains("-32005"),
            "a rate limit is an error, not a revert: {:?}",
            holdings.failed
        );
        let asked = posts.load(std::sync::atomic::Ordering::SeqCst);
        assert_eq!(
            owned_via_rpc(&state, &owner, &candidates).await,
            holdings,
            "the failed read is answered from memory"
        );
        assert_eq!(posts.load(std::sync::atomic::Ordering::SeqCst), asked);

        // A down endpoint fails every contract of its network.
        use axum::routing::post;
        let down = serve(
            axum::Router::new().route("/", post(|| async { axum::http::StatusCode::BAD_GATEWAY })),
        )
        .await;
        let state = state_with(with_rpcs(&format!("80002={down}")));
        let holdings = owned_via_rpc(&state, &owner, &candidates).await;
        assert!(holdings.owned.is_empty());
        assert_eq!(holdings.failed.len(), 2, "{:?}", holdings.failed);

        // No endpoint configured for the network: the deployment has no reader.
        let state = state_with(with_rpcs(""));
        assert_eq!(
            owned_via_rpc(&state, &owner, &candidates).await,
            Holdings::default()
        );
    }

    #[tokio::test]
    async fn candidate_sets_of_one_size_never_share_a_cache_entry() {
        let (rpc, _) = serve_chain().await;
        let state = state_with(with_rpcs(&format!("80002={rpc}")));
        let owner = OWNER.to_string();
        let held: CandidateMap = [candidate("amoy", PLAIN_721, &["7"], false)]
            .into_iter()
            .collect();
        let not_held: CandidateMap = [candidate("amoy", PLAIN_721, &["8"], false)]
            .into_iter()
            .collect();
        assert_eq!(
            owned_via_rpc(&state, &owner, &held).await.owned,
            vec![triple("amoy", PLAIN_721, 7)]
        );
        assert_eq!(
            owned_via_rpc(&state, &owner, &not_held).await.owned,
            Vec::<String>::new()
        );
    }

    #[test]
    fn long_rpc_answers_are_clipped_in_errors() {
        let long = "x".repeat(10 * ERROR_TEXT_CAP);
        let cut = clipped(&long);
        assert!(cut.len() < ERROR_TEXT_CAP + 32, "{}", cut.len());
        assert!(cut.ends_with(&format!("({} bytes)", long.len())));
        assert_eq!(clipped("short"), "short");
        assert!(is_revert(
            &json!({ "code": 3, "message": "execution reverted" })
        ));
        assert!(is_revert(
            &json!({ "code": -32000, "message": "Execution Reverted: no token" })
        ));
        assert!(!is_revert(
            &json!({ "code": -32005, "message": "rate limit exceeded" })
        ));
    }

    #[tokio::test]
    async fn a_linked_item_on_a_network_without_an_endpoint_reads_as_not_owned() {
        let item = LinkedItem {
            asset_urn: "urn:decentraland:amoy:collections-thirdparty:tp:c1:hat".into(),
            network: "amoy".into(),
            contract: PLAIN_721.to_string().to_lowercase(),
            token_id: "7".into(),
            on_l1: false,
        };
        let mappings: HashMap<String, Value> = [(
            item.asset_urn.clone(),
            json!({ "amoy": { item.contract.clone(): [{ "type": "single", "id": "7" }] } }),
        )]
        .into_iter()
        .collect();
        let state = state_with(with_rpcs(""));
        assert_eq!(
            confirm_linked_items(&state, OWNER, &[&item], &mappings).await,
            Ok(vec![false])
        );
        let (rpc, _) = serve_chain().await;
        let state = state_with(with_rpcs(&format!("80002={rpc}")));
        assert_eq!(
            confirm_linked_items(&state, OWNER, &[&item], &mappings).await,
            Ok(vec![true])
        );
    }

    #[tokio::test]
    async fn the_nft_worker_is_asked_per_network_as_the_old_lambdas_did() {
        use axum::routing::post;
        let worker = serve(axum::Router::new().route(
            "/wallets/{owner}/networks/{network}/nfts",
            post(
                |Path((owner, network)): Path<(String, String)>, Json(contracts): Json<Vec<String>>| async move {
                    match network.as_str() {
                        "matic" => Json(json!({ "data": contracts.iter().map(|c| format!("matic:{c}:{owner}")).collect::<Vec<_>>() })).into_response(),
                        _ => axum::http::StatusCode::SERVICE_UNAVAILABLE.into_response(),
                    }
                },
            ),
        ))
        .await;
        let state = state_with(crate::market::Settings::for_tests());
        let contracts: BTreeMap<String, BTreeSet<String>> = [
            ("matic".to_string(), BTreeSet::from(["0xabc".to_string()])),
            ("mainnet".to_string(), BTreeSet::from(["0xdef".to_string()])),
            (
                "ethereum".to_string(),
                BTreeSet::from(["0x123".to_string()]),
            ),
        ]
        .into_iter()
        .collect();
        let base = worker.trim_end_matches('/');
        let holdings = owned_via_nft_worker(&state, base, "0xowner", &contracts).await;
        assert_eq!(
            holdings,
            Holdings {
                owned: vec!["matic:0xabc:0xowner".to_string()],
                failed: Vec::new(),
            },
            "a non-2xx answer and a network outside ContractNetwork report nothing"
        );
        let unreachable =
            owned_via_nft_worker(&state, "http://127.0.0.1:1", "0xowner", &contracts).await;
        assert_eq!(
            unreachable.failed.len(),
            2,
            "a transport failure fails its network: {unreachable:?}"
        );
    }

    #[test]
    fn worn_third_party_urns_split_as_the_old_checker_did() {
        assert_eq!(
            worn_third_party("urn:decentraland:matic:collections-thirdparty:tp:c1:hat"),
            Some(WornThirdParty::V1)
        );
        assert_eq!(
            worn_third_party(
                "urn:decentraland:amoy:collections-thirdparty:tp:c1:hat:sepolia:0xAbC:12"
            ),
            Some(WornThirdParty::Linked(LinkedItem {
                asset_urn: "urn:decentraland:amoy:collections-thirdparty:tp:c1:hat".into(),
                network: "sepolia".into(),
                contract: "0xabc".into(),
                token_id: "12".into(),
                on_l1: true,
            }))
        );
        for unrecognised in [
            // An L1 collection: not checked, so not kept.
            "urn:decentraland:mainnet:collections-thirdparty:tp:c1:hat",
            "urn:decentraland:ethereum:collections-thirdparty:tp:c1:hat",
            // The NFT on a chain outside ContractNetwork.
            "urn:decentraland:matic:collections-thirdparty:tp:c1:hat:ethereum:0xabc:1",
            "urn:decentraland:matic:collections-thirdparty:tp:c1",
        ] {
            assert_eq!(
                worn_third_party(unrecognised),
                Some(WornThirdParty::Unrecognised),
                "{unrecognised}"
            );
            assert!(is_third_party_urn(unrecognised));
        }
        for other in [
            "urn:decentraland:matic:collections-v2:0xabc:1",
            "urn:decentraland:off-chain:base-avatars:eyes_00",
        ] {
            assert_eq!(worn_third_party(other), None, "{other}");
        }
        assert_eq!(
            v1_third_party_urns(&[
                "urn:decentraland:matic:collections-thirdparty:tp:c1:hat".to_string(),
                "urn:decentraland:matic:collections-thirdparty:tp:c1:hat:matic:0xabc:1".to_string(),
            ]),
            ["urn:decentraland:matic:collections-thirdparty:tp:c1:hat"]
        );
    }
    #[test]
    fn providers_parse_the_registry_answer() {
        let data = json!({ "thirdParties": [
            { "id": "urn:decentraland:matic:collections-thirdparty:cryptomotors", "metadata": { "thirdParty": { "contracts": [
                { "network": "matic", "address": "0xABC" }, { "network": "", "address": "0x1" } ] } } },
            { "id": "", "metadata": null },
            { "id": "urn:decentraland:matic:collections-thirdparty:empty", "metadata": null }
        ] });
        let got = parse_providers(&data);
        assert_eq!(got.len(), 2);
        assert_eq!(
            got[0].contracts,
            vec![TpContract {
                network: "matic".into(),
                address: "0xabc".into()
            }]
        );
        assert!(got[1].contracts.is_empty());
        let tpi: Vec<Value> = parse_providers(&json!({ "thirdParties": [
            { "id": "urn:x", "metadata": { "thirdParty": { "name": "N", "description": "D" } } },
            { "id": "urn:y", "metadata": { "thirdParty": { "name": null } } }
        ] }))
        .iter()
        .map(integration_of)
        .collect();
        assert_eq!(
            tpi,
            vec![
                json!({ "name": "N", "description": "D", "urn": "urn:x" }),
                json!({ "name": null, "urn": "urn:y" }),
            ],
            "a null name stays null and a missing description is left out"
        );
    }

    #[test]
    fn owned_tokens_group_by_the_wearable_their_mapping_names() {
        let hat = json!({ "metadata": { "id": "urn:decentraland:matic:collections-thirdparty:tp:c1:hat", "name": "Hat", "data": { "category": "hat" },
                           "mappings": { "matic": { "0xabc": [{ "type": "range", "from": "1", "to": "5" }] } } } });
        let cape = json!({ "metadata": { "id": "urn:decentraland:matic:collections-thirdparty:tp:c1:cape", "name": "Cape", "data": { "category": "upper_body" },
                            "mappings": { "matic": { "0xabc": [{ "type": "single", "id": "9" }] } } } });
        let lists = vec![Arc::new(vec![hat, cape])];
        let owned = vec![
            "matic:0xabc:2".to_string(),
            "matic:0xabc:9".to_string(),
            "matic:0xabc:4".to_string(),
            "mainnet:0xabc:2".to_string(),
        ];
        let got = elements_from(&lists, &owned);
        assert_eq!(got.len(), 2);
        assert_eq!(
            got[0].urn,
            "urn:decentraland:matic:collections-thirdparty:tp:c1:hat"
        );
        assert_eq!(got[0].individual_data.len(), 2);
        assert_eq!(
            got[0].individual_data[0]["id"],
            "urn:decentraland:matic:collections-thirdparty:tp:c1:hat:matic:0xabc:2"
        );
        assert_eq!(got[1].category, "upper_body");
    }
}
