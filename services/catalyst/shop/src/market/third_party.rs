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
//! (`rpc_url_for`) through the ONE guarded client. An endpoint that fails
//! fails the read (`Err`): it never reads as "nothing owned".

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

use alloy_primitives::{Address, B256, Bytes, U256, address, hex};
use alloy_sol_types::{SolCall, SolEvent, SolType, sol, sol_data};
use axum::Json;
use axum::extract::{Path, Request, State};
use axum::response::{IntoResponse, Response};
use serde_json::{Value, json};

use crate::content::queries::active_entity_docs_by_prefix;
use crate::shared::cache::TtlMap;
use crate::shared::query_params::{
    MAX_PAGE_SIZE, NonPositivePolicy, OversizePolicy, Pagination, parse_pagination_with,
    parse_query_string, qs_get_array, qs_get_string,
};

use super::definitions::{extract_wearable_definition, locale_cmp};
use super::graph::graph_query;
use super::listings::{ETHEREUM_MAINNET, ETHEREUM_SEPOLIA, MATIC_AMOY, MATIC_MAINNET};
use super::user_items::is_third_party_name_urn;
use super::{MarketplaceState, SharedState, bad_request};

const PROVIDER_TTL: Duration = Duration::from_secs(6 * 60 * 60);
const COLLECTION_ENTITIES_TTL: Duration = Duration::from_secs(300);
const OWNED_TTL: Duration = Duration::from_secs(60);
const OWNED_MAX_ENTRIES: usize = 4096;

sol! {
    interface IERC165 {
        function supportsInterface(bytes4 interfaceId) external view returns (bool);
    }
    interface IERC721 {
        event Transfer(address indexed from, address indexed to, uint256 indexed tokenId);
        function balanceOf(address owner) external view returns (uint256);
        function ownerOf(uint256 tokenId) external view returns (address);
        function tokenOfOwnerByIndex(address owner, uint256 index) external view returns (uint256);
    }
    interface IERC1155 {
        event TransferSingle(address indexed operator, address indexed from, address indexed to, uint256 id, uint256 value);
        event TransferBatch(address indexed operator, address indexed from, address indexed to, uint256[] ids, uint256[] values);
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
/// The largest enumerable balance walked; past it the read fails.
const ENUMERATE_MAX: u64 = 50_000;
/// `eth_getLogs` requests one open-mapping scan may take when the endpoint
/// refuses the whole history and it is split in halves.
const LOG_SCAN_CALLS: usize = 64;
const MULTICALL_CHUNK: usize = 1000;
const RPC_BATCH: usize = 50;
const CONTRACT_LEG_CONCURRENCY: usize = 8;
const RPC_TIMEOUT: Duration = Duration::from_secs(30);
const RPC_BODY_CAP: usize = 16 * 1024 * 1024;

/// The third-party readers' memo: the provider list (6 h), each provider's
/// mapped entities (5 min), the owner's tokens per network (60 s) and the
/// integrations list (6 h).
pub struct Caches {
    providers: TtlMap<(), Arc<Vec<ThirdPartyProvider>>>,
    integrations: TtlMap<(), Value>,
    collection_entities: TtlMap<String, Arc<Vec<Value>>>,
    owned: TtlMap<(String, String, String), Arc<Vec<String>>>,
}

impl Default for Caches {
    fn default() -> Self {
        Self {
            providers: TtlMap::bounded(PROVIDER_TTL, 1),
            integrations: TtlMap::bounded(PROVIDER_TTL, 1),
            collection_entities: TtlMap::bounded(COLLECTION_ENTITIES_TTL, 256),
            owned: TtlMap::bounded(OWNED_TTL, OWNED_MAX_ENTRIES),
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
        if let Some(cs) = p
            .get("metadata")
            .and_then(|m| m.get("thirdParty"))
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
        out.push(ThirdPartyProvider { id, contracts });
    }
    out
}

/// The approved providers; empty (logged) without a registry subgraph or
/// when it fails, as the old reader answered.
pub async fn third_party_providers(state: &MarketplaceState) -> Vec<ThirdPartyProvider> {
    let Some(url) = state.settings.third_party_registry_subgraph_url.clone() else {
        tracing::debug!("THIRD_PARTY_REGISTRY_L2_SUBGRAPH_URL is unset: no third-party providers");
        return Vec::new();
    };
    state
        .third_party
        .providers
        .get_or_fetch((), || async {
            let data = graph_query(&state.index, &url, TP_QUERY, json!({})).await?;
            Ok::<_, String>(Arc::new(parse_providers(&data)))
        })
        .await
        .map(|p| (*p).clone())
        .unwrap_or_else(|e| {
            tracing::warn!(error = %e, "third-party registry lookup failed");
            Vec::new()
        })
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

#[derive(Debug, Default, Clone, PartialEq, Eq)]
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

/// Owned `network:contract:tokenId` triples, the NFT worker's answer shape.
/// `Err` when an endpoint failed: the caller answers the fetch error, never
/// "nothing owned". A network with no RPC endpoint configured reports
/// nothing (logged): the deployment did not provide the reader.
pub async fn owned_via_rpc(
    state: &MarketplaceState,
    owner: &str,
    candidates: &CandidateMap,
) -> Result<Vec<String>, String> {
    let Ok(owner_addr) = Address::from_str(owner) else {
        return Ok(Vec::new());
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
        let fingerprint = contracts
            .iter()
            .map(|(c, k)| format!("{c}/{}{}", k.ids.len(), if k.open { "+" } else { "" }))
            .collect::<Vec<_>>()
            .join(",");
        let key = (owner.to_lowercase(), network.to_string(), fingerprint);
        let network = network.to_string();
        Some(async move {
            state
                .third_party
                .owned
                .get_or_fetch(key, || async {
                    owned_on_network(state, &rpc, owner_addr, &network, &contracts)
                        .await
                        .map(Arc::new)
                })
                .await
        })
    });
    let mut out = Vec::new();
    for found in futures::future::join_all(per_network).await {
        out.extend(found?.iter().cloned());
    }
    Ok(out)
}

/// The owner's holdings on one network's provider contracts, contract by
/// contract: every ERC-1155 id the mappings name or the owner ever received,
/// the whole `tokenOfOwnerByIndex` walk of an enumerable ERC-721, and on a
/// plain ERC-721 the mapped ids plus, for an `any` or wide `range` mapping,
/// every id the owner ever received; each confirmed against current
/// ownership. That is the set the NFT worker answers from its indexer.
async fn owned_on_network(
    state: &MarketplaceState,
    rpc: &str,
    owner: Address,
    network: &str,
    contracts: &[(String, Candidates)],
) -> Result<Vec<String>, String> {
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
    let answers = eth_call_batch(state, rpc, &probes).await?;

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
        let holds_721 = balance.is_some_and(|b| !b.is_zero());
        legs.push(async move {
            let ids = if is_1155 {
                let mut ids = parsed_ids(cand);
                if cand.open {
                    merge_received(
                        &mut ids,
                        received_ids(state, rpc, addr, owner, ReceivedVia::Erc1155).await?,
                    );
                }
                owned_1155(state, rpc, addr, owner, ids).await?
            } else if holds_721 && enumerable {
                enumerate_721(state, rpc, addr, owner, balance.unwrap_or_default()).await?
            } else if holds_721 {
                let mut ids = parsed_ids(cand);
                if cand.open {
                    merge_received(
                        &mut ids,
                        received_ids(state, rpc, addr, owner, ReceivedVia::Erc721).await?,
                    );
                }
                check_721(state, rpc, addr, owner, ids).await?
            } else {
                Vec::new()
            };
            Ok::<_, String>(
                ids.into_iter()
                    .map(|id| format!("{network}:{contract}:{id}"))
                    .collect::<Vec<_>>(),
            )
        });
    }
    use futures::stream::{StreamExt, TryStreamExt};
    futures::stream::iter(legs)
        .buffered(CONTRACT_LEG_CONCURRENCY)
        .try_concat()
        .await
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

/// Adds the received ids no mapping spells already.
fn merge_received(ids: &mut IdSet, received: Vec<U256>) {
    let known: BTreeSet<U256> = ids.values().copied().collect();
    for id in received {
        if !known.contains(&id) {
            ids.insert(id.to_string(), id);
        }
    }
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
/// `ENUMERATE_MAX`; a larger one (or a contract answering a nonsense
/// balance) fails the fetch rather than truncating it.
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
    let calls: Vec<RpcCall> = ids
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
    let mut owned = Vec::new();
    for (chunk, answer) in ids.chunks(MULTICALL_CHUNK).zip(answers) {
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
    Ok(owned)
}

#[derive(Clone, Copy)]
enum ReceivedVia {
    /// `Transfer(from, to indexed, tokenId indexed)`.
    Erc721,
    /// `TransferSingle`/`TransferBatch(operator, from, to indexed, ...)`.
    Erc1155,
}

/// The ids of every transfer `contract` logged to `owner`, from genesis: the
/// candidates of an open mapping, which the caller confirms against current
/// ownership. One `eth_getLogs` over the whole history; a range the endpoint
/// refuses is asked again in halves, at most `LOG_SCAN_CALLS` requests.
async fn received_ids(
    state: &MarketplaceState,
    rpc: &str,
    contract: Address,
    owner: Address,
    via: ReceivedVia,
) -> Result<Vec<U256>, String> {
    let to = B256::left_padding_from(owner.as_slice());
    let topics = match via {
        ReceivedVia::Erc721 => json!([IERC721::Transfer::SIGNATURE_HASH, null, to]),
        ReceivedVia::Erc1155 => json!([
            [
                IERC1155::TransferSingle::SIGNATURE_HASH,
                IERC1155::TransferBatch::SIGNATURE_HASH
            ],
            null,
            null,
            to
        ]),
    };
    let filter = |from: Value, to: Value| json!([{ "address": contract, "topics": topics, "fromBlock": from, "toBlock": to }]);
    let mut logs: Vec<Value> = Vec::new();
    let mut calls = 1;
    match rpc_call(
        state,
        rpc,
        "eth_getLogs",
        filter(json!("0x0"), json!("latest")),
    )
    .await?
    {
        Ok(found) => logs.extend(found.as_array().cloned().unwrap_or_default()),
        Err(refused) => {
            tracing::debug!(%contract, refused, "eth_getLogs refused the whole history; splitting it");
            let head = rpc_call(state, rpc, "eth_blockNumber", json!([]))
                .await?
                .ok()
                .as_ref()
                .and_then(Value::as_str)
                .and_then(|h| u64::from_str_radix(h.trim_start_matches("0x"), 16).ok())
                .ok_or_else(|| format!("{contract}: eth_blockNumber gave no block"))?;
            let mut ranges = vec![(0u64, head / 2), (head / 2 + 1, head)];
            while let Some((from, upto)) = ranges.pop() {
                calls += 1;
                if calls > LOG_SCAN_CALLS {
                    return Err(format!(
                        "{contract}: the endpoint refused eth_getLogs over ranges of {} blocks and more",
                        upto - from + 1
                    ));
                }
                match rpc_call(
                    state,
                    rpc,
                    "eth_getLogs",
                    filter(json!(format!("0x{from:x}")), json!(format!("0x{upto:x}"))),
                )
                .await?
                {
                    Ok(found) => logs.extend(found.as_array().cloned().unwrap_or_default()),
                    Err(refused) if upto > from => {
                        tracing::debug!(%contract, from, upto, refused, "eth_getLogs refused the range; splitting it");
                        let mid = from + (upto - from) / 2;
                        ranges.push((mid + 1, upto));
                        ranges.push((from, mid));
                    }
                    Err(refused) => return Err(format!("{contract}: eth_getLogs: {refused}")),
                }
            }
        }
    }
    Ok(logs
        .iter()
        .flat_map(|log| log_token_ids(log, via))
        .collect())
}

fn log_token_ids(log: &Value, via: ReceivedVia) -> Vec<U256> {
    let word = |s: &str| U256::from_str_radix(s.trim_start_matches("0x"), 16).ok();
    let topics: Vec<&str> = log
        .get("topics")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .collect();
    let data = log
        .get("data")
        .and_then(Value::as_str)
        .and_then(|d| hex::decode(d.trim_start_matches("0x")).ok())
        .unwrap_or_default();
    match via {
        ReceivedVia::Erc721 => topics.get(3).and_then(|t| word(t)).into_iter().collect(),
        ReceivedVia::Erc1155 => {
            let topic0 = topics.first().and_then(|t| B256::from_str(t).ok());
            if topic0 == Some(IERC1155::TransferSingle::SIGNATURE_HASH) {
                data.get(..32)
                    .map(|w| U256::from_be_slice(w))
                    .into_iter()
                    .collect()
            } else if topic0 == Some(IERC1155::TransferBatch::SIGNATURE_HASH) {
                <(
                    sol_data::Array<sol_data::Uint<256>>,
                    sol_data::Array<sol_data::Uint<256>>,
                )>::abi_decode_params(&data)
                .map(|(ids, _)| ids)
                .unwrap_or_default()
            } else {
                Vec::new()
            }
        }
    }
}

/// One entry per calldata: the sub-call's return data when it succeeded.
async fn multicall(
    state: &MarketplaceState,
    rpc: &str,
    target: Address,
    calldatas: Vec<Bytes>,
) -> Result<Vec<Option<Bytes>>, String> {
    let calls: Vec<RpcCall> = calldatas
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
    let mut out = Vec::with_capacity(calldatas.len());
    for (chunk, answer) in calldatas.chunks(MULTICALL_CHUNK).zip(answers) {
        out.extend(
            decode_aggregate3(answer.as_ref(), chunk.len())
                .ok_or_else(|| format!("{target}: the multicall answered no results"))?,
        );
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

/// `eth_call` each call at `latest`; a call the node refused (a revert) is
/// `None`, as the old checkers read a missing `result` as not owned.
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
    Ok(rpc_batch(state, rpc, &requests)
        .await?
        .into_iter()
        .map(|answer| {
            answer
                .ok()
                .as_ref()
                .and_then(Value::as_str)
                .and_then(|h| hex::decode(h.trim_start_matches("0x")).ok())
                .map(Bytes::from)
        })
        .collect())
}

async fn rpc_call(
    state: &MarketplaceState,
    rpc: &str,
    method: &str,
    params: Value,
) -> Result<Result<Value, String>, String> {
    rpc_batch(state, rpc, &[(method, params)])
        .await?
        .pop()
        .ok_or_else(|| "JSON-RPC batch: no answer".to_string())
}

/// JSON-RPC batches of `RPC_BATCH` requests through the guarded client:
/// per request its `result`, or the `error` the node answered it with. A
/// transport failure, a non-2xx status, an unreadable body or a missing
/// answer fails the whole call.
async fn rpc_batch(
    state: &MarketplaceState,
    rpc: &str,
    requests: &[(&str, Value)],
) -> Result<Vec<Result<Value, String>>, String> {
    if !state.upstream_allowed(rpc) {
        return Err("the RPC endpoint is not a publicly routable http(s) URL".to_string());
    }
    let mut out: Vec<Option<Result<Value, String>>> = vec![None; requests.len()];
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
                "JSON-RPC batch: answered {parsed} instead of a batch"
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
                    .map(Value::to_string)
                    .unwrap_or_else(|| "no result".to_string())),
            });
        }
    }
    out.into_iter()
        .map(|answer| answer.ok_or_else(|| "JSON-RPC batch: a request got no answer".to_string()))
        .collect()
}

/// `NFT_WORKER_BASE_URL`: `POST <base>/wallets/<owner>/networks/<network>/nfts`
/// with the network's contract addresses, answering `{data: [triple]}`, the
/// old lambdas' source. As there, a network outside `ContractNetwork` and a
/// non-2xx answer are logged and report nothing; a transport failure or an
/// unreadable body fails the fetch.
async fn owned_via_nft_worker(
    state: &MarketplaceState,
    base: &str,
    owner: &str,
    contracts_by_network: &BTreeMap<String, BTreeSet<String>>,
) -> Result<Vec<String>, String> {
    let per_network = contracts_by_network
        .iter()
        .map(|(network, contracts)| async move {
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
        });
    let mut out = Vec::new();
    for found in futures::future::join_all(per_network).await {
        out.extend(found?);
    }
    Ok(out)
}

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

/// Every third-party wearable `owner` holds through a mapped NFT, optionally
/// only the provider named `only_name` (the URN's fifth part). `Err` when the
/// owner's holdings could not be read.
pub async fn fetch_all_third_party_wearables(
    state: &MarketplaceState,
    owner: &str,
    only_name: Option<&str>,
) -> Result<Vec<TpwElement>, String> {
    let owner = owner.to_lowercase();
    let mut providers = third_party_providers(state).await;
    providers.retain(|p| !p.contracts.is_empty());
    if let Some(name) = only_name {
        providers.retain(|p| p.id.split(':').nth(4) == Some(name));
    }
    if providers.is_empty() {
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
    let (lists, owned) = match state.settings.nft_worker_base_url.as_deref() {
        Some(base) => {
            let owned = owned_via_nft_worker(state, base, &owner, &contracts_by_network).await?;
            // Only the providers whose contracts returned a token are read.
            let hit: Vec<&str> = providers
                .iter()
                .filter(|p| {
                    p.contracts.iter().any(|c| {
                        let prefix = format!("{}:{}:", c.network, c.address);
                        owned.iter().any(|nft| nft.starts_with(&prefix))
                    })
                })
                .map(|p| p.id.as_str())
                .collect();
            (collection_entity_lists(state, hit.into_iter()).await, owned)
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
            let owned = owned_via_rpc(state, &owner, &candidates).await?;
            (lists, owned)
        }
    };
    if owned.is_empty() {
        return Ok(Vec::new());
    }
    Ok(elements_from(&lists, &owned))
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
    let Ok(first) = active_entity_docs_by_prefix(&state.pool, collection_id, 0, PAGE).await else {
        return Vec::new();
    };
    let short = (first.entities.len() as i64) < PAGE;
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
        let Ok(page) = page else {
            break;
        };
        let short = (page.entities.len() as i64) < PAGE;
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

fn pagination_object(
    query_string: &str,
) -> Result<(crate::shared::query_params::QueryParams, Pagination), Response> {
    let params = parse_query_string(query_string);
    let pagination = parse_pagination_with(
        &params,
        MAX_PAGE_SIZE as i64,
        OversizePolicy::Reject,
        NonPositivePolicy::PassThrough,
    )
    .map_err(|msg| bad_request(&msg))?;
    Ok((params, pagination))
}

/// `GET /lambdas/users/{address}/third-party-wearables`.
pub async fn user_third_party_wearables(
    State(state): State<SharedState>,
    Path(address): Path<String>,
    request: Request,
) -> Response {
    if !super::is_valid_eth_address(&address.to_lowercase()) {
        return bad_request("Address must be a valid Ethereum address");
    }
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
    let mut elements = match fetch_all_third_party_wearables(&state, &address, None).await {
        Ok(elements) => elements,
        Err(e) => return fetch_error(&address, &e),
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
    if !super::is_valid_eth_address(&address.to_lowercase()) {
        return bad_request("Address must be a valid Ethereum address");
    }
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
    let mut elements = match fetch_all_third_party_wearables(&state, &address, Some(&name)).await {
        Ok(elements) => elements,
        Err(e) => return fetch_error(&address, &e),
    };
    elements.sort_by(|a, b| locale_cmp(&a.name, &b.name).then_with(|| locale_cmp(&a.urn, &b.urn)));
    let include_definitions = params.contains_key("includeDefinitions");
    finish_tpw_response(&state, elements, &pagination, include_definitions)
}

fn finish_tpw_response(
    state: &MarketplaceState,
    elements: Vec<TpwElement>,
    pagination: &Pagination,
    include_definitions: bool,
) -> Response {
    let total = elements.len() as i64;
    let js_index = |i: i64| -> i64 {
        if i < 0 {
            (total + i).max(0)
        } else {
            i.min(total)
        }
    };
    let start = js_index(pagination.offset);
    let end = js_index(pagination.offset + pagination.limit);
    let page: &[TpwElement] = if start < end {
        &elements[start as usize..end as usize]
    } else {
        &[]
    };
    let content_public_url = &state.settings.content_public_url;
    let out: Vec<Value> = page
        .iter()
        .map(|e| {
            let mut v = e.to_value();
            if include_definitions {
                let def = extract_wearable_definition(&e.entity, content_public_url)
                    .unwrap_or(Value::Null);
                if let Some(obj) = v.as_object_mut() {
                    obj.insert("definition".to_string(), def);
                }
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

const TPI_QUERY: &str = r#"{ thirdParties(where: {isApproved: true}, first: 1000) { id metadata { thirdParty { name description } } } }"#;

fn integrations_from(data: &Value) -> Result<Vec<Value>, String> {
    let providers = data
        .get("thirdParties")
        .and_then(|t| t.as_array())
        .ok_or_else(|| format!("unexpected subgraph response: {data}"))?;
    Ok(providers
        .iter()
        .map(|p| {
            let id = p.get("id").and_then(|v| v.as_str()).unwrap_or("");
            let tp = p.get("metadata").and_then(|m| m.get("thirdParty"));
            let name = tp
                .and_then(|t| t.get("name"))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let description = tp
                .and_then(|t| t.get("description"))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            json!({ "name": name, "description": description, "urn": id })
        })
        .collect())
}

/// `GET /lambdas/third-party-integrations`: `{data: [...]}`, empty (logged)
/// without a registry subgraph or when it fails, as the old handler answered.
pub async fn third_party_integrations(State(state): State<SharedState>) -> impl IntoResponse {
    let Some(url) = state.settings.third_party_registry_subgraph_url.clone() else {
        tracing::debug!(
            "THIRD_PARTY_REGISTRY_L2_SUBGRAPH_URL is unset; /lambdas/third-party-integrations answers empty"
        );
        return Json(json!({ "data": [] }));
    };
    let st = state.clone();
    match state
        .third_party
        .integrations
        .get_or_fetch((), || async move {
            let data = graph_query(&st.index, &url, TPI_QUERY, json!({})).await?;
            integrations_from(&data).map(Value::Array)
        })
        .await
    {
        Ok(v) => Json(json!({ "data": v })),
        Err(e) => {
            tracing::error!("third-party-integrations failed: {e}");
            Json(json!({ "data": [] }))
        }
    }
}

/// The owned third-party URNs of a collection, for the by-owner lists.
pub(crate) async fn owned_third_party_urns(
    state: &MarketplaceState,
    owner: &str,
    provider_name: &str,
) -> Result<Vec<(String, usize, Value)>, String> {
    Ok(
        fetch_all_third_party_wearables(state, owner, Some(provider_name))
            .await?
            .into_iter()
            .map(|e| (e.urn, e.individual_data.len(), e.entity))
            .collect(),
    )
}

/// The old lambdas' `FetcherError` answer: 502 `Cannot fetch elements for
/// <address>`, the cause logged.
pub(crate) fn fetch_error(address: &str, cause: &str) -> Response {
    tracing::warn!(error = %cause, address, "third-party ownership read failed");
    super::lambdas_error(
        axum::http::StatusCode::BAD_GATEWAY,
        "The requested items cannot be fetched right now",
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
    const ENUM_721: Address = address!("0x00000000000000000000000000000000000000e1");
    const MULTI_1155: Address = address!("0x0000000000000000000000000000000000001155");
    /// The fake chain's head; `eth_getLogs` refuses any range wider than
    /// `LOG_RANGE_LIMIT` blocks, the whole history included.
    const HEAD: u64 = 4095;
    const LOG_RANGE_LIMIT: u64 = 1000;
    const ENUM_BALANCE: u64 = 1500;

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
            IERC721::balanceOfCall::SELECTOR if to == PLAIN_721 => Some(word(U256::from(1))),
            IERC721::balanceOfCall::SELECTOR if to == ENUM_721 => {
                Some(word(U256::from(ENUM_BALANCE)))
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

    fn topic(a: Address) -> String {
        B256::left_padding_from(a.as_slice()).to_string()
    }

    /// The transfers the owner received: plain ERC-721 tokens 5 (sold since)
    /// and 7, ERC-1155 ids 11 and 12 in one batch and 13 alone.
    fn chain_logs(address: Address, from: u64, to: u64) -> Vec<Value> {
        let at = |block: u64, log: Value| (from <= block && block <= to).then_some(log);
        let id_topic = |id: u64| B256::from(U256::from(id)).to_string();
        let logs = if address == PLAIN_721 {
            vec![
                at(
                    100,
                    json!({ "topics": [IERC721::Transfer::SIGNATURE_HASH, topic(OTHER), topic(OWNER), id_topic(5)], "data": "0x" }),
                ),
                at(
                    3000,
                    json!({ "topics": [IERC721::Transfer::SIGNATURE_HASH, topic(OTHER), topic(OWNER), id_topic(7)], "data": "0x" }),
                ),
            ]
        } else if address == MULTI_1155 {
            let batch = (
                vec![U256::from(11), U256::from(12)],
                vec![U256::from(1), U256::from(1)],
            )
                .abi_encode_params();
            let single = (U256::from(13), U256::from(2)).abi_encode_params();
            vec![
                at(
                    200,
                    json!({ "topics": [IERC1155::TransferBatch::SIGNATURE_HASH, topic(OTHER), topic(OTHER), topic(OWNER)], "data": Bytes::from(batch) }),
                ),
                at(
                    2500,
                    json!({ "topics": [IERC1155::TransferSingle::SIGNATURE_HASH, topic(OTHER), topic(OTHER), topic(OWNER)], "data": Bytes::from(single) }),
                ),
            ]
        } else {
            Vec::new()
        };
        logs.into_iter().flatten().collect()
    }

    fn block_of(v: &Value) -> Option<u64> {
        match v.as_str()? {
            "latest" => Some(HEAD),
            h => u64::from_str_radix(h.trim_start_matches("0x"), 16).ok(),
        }
    }

    fn chain_answer(req: &Value) -> Value {
        let id = req["id"].clone();
        let refuse = |msg: &str| json!({ "jsonrpc": "2.0", "id": id, "error": { "code": -32005, "message": msg } });
        let result = |r: Value| json!({ "jsonrpc": "2.0", "id": id, "result": r });
        let p = &req["params"][0];
        match req["method"].as_str() {
            Some("eth_blockNumber") => result(json!(format!("0x{HEAD:x}"))),
            Some("eth_getLogs") => {
                let (Some(from), Some(to)) = (block_of(&p["fromBlock"]), block_of(&p["toBlock"]))
                else {
                    return refuse("bad range");
                };
                if to - from + 1 > LOG_RANGE_LIMIT {
                    return refuse("block range too large");
                }
                let address: Address = p["address"].as_str().unwrap().parse().unwrap();
                result(json!(chain_logs(address, from, to)))
            }
            Some("eth_call") => {
                let to: Address = p["to"].as_str().unwrap().parse().unwrap();
                let data =
                    hex::decode(p["data"].as_str().unwrap().trim_start_matches("0x")).unwrap();
                match chain_call(to, &data) {
                    Some(ret) => result(json!(ret)),
                    None => {
                        json!({ "jsonrpc": "2.0", "id": id, "error": { "code": 3, "message": "execution reverted" } })
                    }
                }
            }
            _ => refuse("unknown method"),
        }
    }

    async fn serve(router: axum::Router) -> String {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
        format!("http://{addr}/")
    }

    async fn serve_chain() -> String {
        use axum::routing::post;
        serve(axum::Router::new().route(
            "/",
            post(|Json(body): Json<Value>| async move {
                Json(match &body {
                    Value::Array(reqs) => Value::Array(reqs.iter().map(chain_answer).collect()),
                    req => chain_answer(req),
                })
            }),
        ))
        .await
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
    async fn holdings_cover_open_mappings_past_the_old_caps_and_testnets() {
        let rpc = serve_chain().await;
        let state = state_with(with_rpcs(&format!("11155111={rpc},80002={rpc}")));
        let candidates: CandidateMap = [
            // `any` on a plain ERC-721: the received tokens, of which 7 is still held.
            candidate("amoy", PLAIN_721, &["3"], true),
            // An enumerable ERC-721 past the old 1,000-token walk.
            candidate("amoy", ENUM_721, &[], true),
            // `any` on an ERC-1155: the batch and the single transfer, 12 sold since.
            candidate("sepolia", MULTI_1155, &["14"], true),
        ]
        .into_iter()
        .collect();
        let owner = OWNER.to_string();
        let mut owned = owned_via_rpc(&state, &owner, &candidates).await.unwrap();
        owned.sort();
        let plain = format!("amoy:{}:7", PLAIN_721.to_string().to_lowercase());
        let multi = MULTI_1155.to_string().to_lowercase();
        assert!(owned.contains(&plain), "{owned:?}");
        assert!(owned.contains(&format!("sepolia:{multi}:11")), "{owned:?}");
        assert!(owned.contains(&format!("sepolia:{multi}:13")), "{owned:?}");
        let enumerated = owned
            .iter()
            .filter(|o| o.contains(&ENUM_721.to_string().to_lowercase()))
            .count();
        assert_eq!(
            enumerated as u64, ENUM_BALANCE,
            "the whole balance is walked"
        );
        assert_eq!(owned.len() as u64, 3 + ENUM_BALANCE, "{:?}", &owned[..5]);
    }

    #[tokio::test]
    async fn a_failing_endpoint_is_an_error_not_an_empty_wallet() {
        use axum::routing::post;
        let down = serve(
            axum::Router::new().route("/", post(|| async { axum::http::StatusCode::BAD_GATEWAY })),
        )
        .await;
        let state = state_with(with_rpcs(&format!("80002={down}")));
        let candidates: CandidateMap = [candidate("amoy", PLAIN_721, &["7"], false)]
            .into_iter()
            .collect();
        assert!(
            owned_via_rpc(&state, &OWNER.to_string(), &candidates)
                .await
                .is_err()
        );

        // No endpoint configured for the network: the deployment has no reader.
        let state = state_with(with_rpcs(""));
        assert_eq!(
            owned_via_rpc(&state, &OWNER.to_string(), &candidates).await,
            Ok(Vec::new())
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
        let owned = owned_via_nft_worker(&state, base, "0xowner", &contracts).await;
        assert_eq!(
            owned,
            Ok(vec!["matic:0xabc:0xowner".to_string()]),
            "a non-2xx answer and a network outside ContractNetwork report nothing"
        );
        let unreachable =
            owned_via_nft_worker(&state, "http://127.0.0.1:1", "0xowner", &contracts).await;
        assert!(unreachable.is_err(), "a transport failure fails the fetch");
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
        let tpi = integrations_from(&json!({ "thirdParties": [{ "id": "urn:x", "metadata": { "thirdParty": { "name": "N", "description": "D" } } }] })).unwrap();
        assert_eq!(
            tpi,
            vec![json!({ "name": "N", "description": "D", "urn": "urn:x" })]
        );
        assert!(integrations_from(&json!({})).is_err());
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
