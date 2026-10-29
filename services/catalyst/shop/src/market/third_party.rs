//! Third-party wearables: the approved providers of the third-party registry
//! subgraph (`THIRD_PARTY_REGISTRY_L2_SUBGRAPH_URL`), the entities each
//! provider deployed (their `metadata.mappings` link a wearable to NFT token
//! ids), and the owner's tokens asked of the chain itself over the JSON-RPC
//! endpoint of each network (`TRADE_RPC_URLS` / `ETH_RPC_URL` for mainnet,
//! `RPC_ENDPOINT_POLYGON` or `TRADE_RPC_URLS` 137 for matic), through the ONE
//! guarded client. The old node could ask an NFT ownership indexer
//! (`NFT_WORKER_BASE_URL`) first; no deployment configured one, so the chain
//! path is the one carried.

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

use crate::content::queries::active_entity_docs_by_prefix;
use crate::shared::cache::TtlMap;
use crate::shared::query_params::{
    MAX_PAGE_SIZE, NonPositivePolicy, OversizePolicy, Pagination, parse_pagination_with,
    parse_query_string, qs_get_array, qs_get_string,
};

use super::chain_walk::ChainNet;
use super::definitions::{extract_wearable_definition, locale_cmp};
use super::graph::graph_query;
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
        function balanceOf(address owner) external view returns (uint256);
        function ownerOf(uint256 tokenId) external view returns (address);
        function tokenOfOwnerByIndex(address owner, uint256 index) external view returns (uint256);
    }
    interface IERC1155 {
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
/// Enumerable contracts are walked at most this far.
const ENUMERATE_CAP: u64 = 1000;
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

fn chain_of(network: &str) -> Option<ChainNet> {
    match network {
        "mainnet" => Some(ChainNet::Ethereum),
        "matic" => Some(ChainNet::Matic),
        _ => None,
    }
}

/// The JSON-RPC endpoint of a mappings network.
fn rpc_url_for(state: &MarketplaceState, network: &str) -> Option<String> {
    match chain_of(network)? {
        ChainNet::Matic => state
            .settings
            .polygon_rpc_url
            .clone()
            .or_else(|| state.chain_rpc_url(ChainNet::Matic)),
        net => state.chain_rpc_url(net),
    }
}

/// Owned `network:contract:tokenId` triples.
pub async fn owned_via_rpc(
    state: &MarketplaceState,
    owner: &str,
    candidates: &CandidateMap,
) -> Vec<String> {
    let Ok(owner_addr) = Address::from_str(owner) else {
        return Vec::new();
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
                    Ok::<_, String>(Arc::new(owned_on_network(state, &rpc, owner_addr, &network, &contracts).await))
                })
                .await
                .unwrap_or_default()
        })
    });
    futures::future::join_all(per_network)
        .await
        .into_iter()
        .flat_map(|found| found.iter().cloned().collect::<Vec<_>>())
        .collect()
}

async fn owned_on_network(
    state: &MarketplaceState,
    rpc: &str,
    owner: Address,
    network: &str,
    contracts: &[(String, Candidates)],
) -> Vec<String> {
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
        probes.push(RpcCall {
            to: addr,
            data: IERC165::supportsInterfaceCall {
                interfaceId: IFACE_ERC1155.into(),
            }
            .abi_encode()
            .into(),
        });
        probes.push(RpcCall {
            to: addr,
            data: IERC165::supportsInterfaceCall {
                interfaceId: IFACE_ERC721_ENUMERABLE.into(),
            }
            .abi_encode()
            .into(),
        });
    }
    let answers = eth_call_batch(state, rpc, &probes).await;

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
                owned_1155(state, rpc, addr, owner, cand).await
            } else if holds_721 && enumerable {
                enumerate_721(state, rpc, addr, owner, balance.unwrap_or_default()).await
            } else if holds_721 {
                if cand.open {
                    tracing::debug!(contract, "third-party mapping is open-ended on a non-enumerable ERC721; only the explicitly mapped ids are checked");
                }
                check_721(state, rpc, addr, owner, cand).await
            } else {
                Vec::new()
            };
            ids.into_iter().map(|id| format!("{network}:{contract}:{id}")).collect::<Vec<_>>()
        });
    }
    use futures::stream::StreamExt;
    futures::stream::iter(legs)
        .buffered(CONTRACT_LEG_CONCURRENCY)
        .concat()
        .await
}

fn decode_bool(answer: Option<&Option<Bytes>>) -> bool {
    answer
        .and_then(Option::as_ref)
        .and_then(|b| IERC165::supportsInterfaceCall::abi_decode_returns(b).ok())
        .unwrap_or(false)
}

fn parsed_ids(cand: &Candidates) -> Vec<(&str, U256)> {
    cand.ids
        .iter()
        .filter_map(|id| U256::from_str_radix(id, 10).ok().map(|u| (id.as_str(), u)))
        .collect()
}

async fn check_721(
    state: &MarketplaceState,
    rpc: &str,
    addr: Address,
    owner: Address,
    cand: &Candidates,
) -> Vec<String> {
    let ids = parsed_ids(cand);
    let calldatas: Vec<Bytes> = ids
        .iter()
        .map(|(_, id)| IERC721::ownerOfCall { tokenId: *id }.abi_encode().into())
        .collect();
    let answers = multicall(state, rpc, addr, calldatas).await;
    ids.iter()
        .zip(answers)
        .filter(|(_, ret)| {
            ret.as_ref()
                .and_then(|b| IERC721::ownerOfCall::abi_decode_returns(b).ok())
                .is_some_and(|holder| holder == owner)
        })
        .map(|((id, _), _)| (*id).to_string())
        .collect()
}

async fn enumerate_721(
    state: &MarketplaceState,
    rpc: &str,
    addr: Address,
    owner: Address,
    balance: U256,
) -> Vec<String> {
    let n = balance.min(U256::from(ENUMERATE_CAP)).to::<u64>();
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
    multicall(state, rpc, addr, calldatas)
        .await
        .into_iter()
        .filter_map(|ret| {
            ret.and_then(|b| IERC721::tokenOfOwnerByIndexCall::abi_decode_returns(&b).ok())
        })
        .map(|id| id.to_string())
        .collect()
}

async fn owned_1155(
    state: &MarketplaceState,
    rpc: &str,
    addr: Address,
    owner: Address,
    cand: &Candidates,
) -> Vec<String> {
    let ids = parsed_ids(cand);
    let calls: Vec<RpcCall> = ids
        .chunks(MULTICALL_CHUNK)
        .map(|chunk| {
            let ids: Vec<U256> = chunk.iter().map(|(_, u)| *u).collect();
            RpcCall {
                to: addr,
                data: IERC1155::balanceOfBatchCall {
                    accounts: vec![owner; ids.len()],
                    ids,
                }
                .abi_encode()
                .into(),
            }
        })
        .collect();
    let answers = eth_call_batch(state, rpc, &calls).await;
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
                owned.push((*id).to_string());
            }
        }
    }
    owned
}

/// One entry per calldata: the sub-call's return data when it succeeded.
async fn multicall(
    state: &MarketplaceState,
    rpc: &str,
    target: Address,
    calldatas: Vec<Bytes>,
) -> Vec<Option<Bytes>> {
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
    let answers = eth_call_batch(state, rpc, &calls).await;
    let mut out = Vec::with_capacity(calldatas.len());
    for (chunk, answer) in calldatas.chunks(MULTICALL_CHUNK).zip(answers) {
        out.extend(decode_aggregate3(answer.as_ref(), chunk.len()));
    }
    out
}

fn decode_aggregate3(answer: Option<&Bytes>, expected: usize) -> Vec<Option<Bytes>> {
    match answer.and_then(|b| IMulticall3::aggregate3Call::abi_decode_returns(b).ok()) {
        Some(results) if results.len() == expected => results
            .into_iter()
            .map(|r| r.success.then_some(r.returnData))
            .collect(),
        _ => (0..expected).map(|_| None).collect(),
    }
}

struct RpcCall {
    to: Address,
    data: Bytes,
}

/// JSON-RPC batches of `eth_call` through the guarded client; a failed or
/// reverted call is `None`.
async fn eth_call_batch(
    state: &MarketplaceState,
    rpc: &str,
    calls: &[RpcCall],
) -> Vec<Option<Bytes>> {
    let mut out: Vec<Option<Bytes>> = vec![None; calls.len()];
    if !state.upstream_allowed(rpc) {
        tracing::warn!(
            "third-party ownership: the RPC endpoint is not a publicly routable http(s) URL"
        );
        return out;
    }
    for (chunk_index, chunk) in calls.chunks(RPC_BATCH).enumerate() {
        let base = chunk_index * RPC_BATCH;
        let body: Vec<Value> = chunk
            .iter()
            .enumerate()
            .map(|(i, c)| {
                json!({
                    "jsonrpc": "2.0",
                    "id": base + i,
                    "method": "eth_call",
                    "params": [{ "to": c.to.to_string(), "data": c.data.to_string() }, "latest"],
                })
            })
            .collect();
        let resp = match state
            .http
            .post(rpc)
            .timeout(RPC_TIMEOUT)
            .json(&body)
            .send()
            .await
        {
            Ok(r) => r,
            Err(e) => {
                tracing::warn!(error = %e, "eth_call batch failed");
                continue;
            }
        };
        if !resp.status().is_success() {
            tracing::warn!(status = %resp.status(), "eth_call batch rejected");
            continue;
        }
        let parsed: Value = match crate::http::read_body_capped(resp, RPC_BODY_CAP)
            .await
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
        {
            Some(v) => v,
            None => {
                tracing::warn!("eth_call batch: unreadable response");
                continue;
            }
        };
        let Some(items) = parsed.as_array() else {
            tracing::warn!("eth_call batch: endpoint answered a single object, not a batch");
            continue;
        };
        for item in items {
            let Some(idx) = item.get("id").and_then(Value::as_u64).map(|id| id as usize) else {
                continue;
            };
            if idx < base || idx >= base + chunk.len() {
                continue;
            }
            if let Some(hex_str) = item.get("result").and_then(Value::as_str)
                && let Ok(bytes) = hex::decode(hex_str.trim_start_matches("0x"))
            {
                out[idx] = Some(Bytes::from(bytes));
            }
        }
    }
    out
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
/// only the provider named `only_name` (the URN's fifth part).
pub async fn fetch_all_third_party_wearables(
    state: &MarketplaceState,
    owner: &str,
    only_name: Option<&str>,
) -> Vec<TpwElement> {
    let owner = owner.to_lowercase();
    let mut providers = third_party_providers(state).await;
    providers.retain(|p| !p.contracts.is_empty());
    if let Some(name) = only_name {
        providers.retain(|p| p.id.split(':').nth(4) == Some(name));
    }
    if providers.is_empty() {
        return Vec::new();
    }
    let mut contracts_by_network: HashMap<String, Vec<String>> = HashMap::new();
    for p in &providers {
        for c in &p.contracts {
            let set = contracts_by_network.entry(c.network.clone()).or_default();
            if !set.contains(&c.address) {
                set.push(c.address.clone());
            }
        }
    }
    let lists = collection_entity_lists(state, providers.iter().map(|p| p.id.as_str())).await;
    let mut candidates = candidates_from_mappings(lists.iter().flat_map(|l| l.iter()));
    candidates.retain(|(network, contract), _| {
        contracts_by_network
            .get(network)
            .is_some_and(|cs| cs.contains(contract))
    });
    let owned = owned_via_rpc(state, &owner, &candidates).await;
    if owned.is_empty() {
        return Vec::new();
    }
    elements_from(&lists, &owned)
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
    let mut elements = fetch_all_third_party_wearables(&state, &address, None).await;
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
    let mut elements = fetch_all_third_party_wearables(&state, &address, Some(&name)).await;
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
) -> Vec<(String, usize, Value)> {
    fetch_all_third_party_wearables(state, owner, Some(provider_name))
        .await
        .into_iter()
        .map(|e| (e.urn, e.individual_data.len(), e.entity))
        .collect()
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
        let decoded = decode_aggregate3(Some(&blob), 3);
        assert_eq!(decoded.len(), 3);
        let owner = IERC721::ownerOfCall::abi_decode_returns(decoded[0].as_ref().unwrap()).unwrap();
        assert_eq!(owner, holder);
        assert!(decoded[1].is_none());
        let id = IERC721::tokenOfOwnerByIndexCall::abi_decode_returns(decoded[2].as_ref().unwrap())
            .unwrap();
        assert_eq!(id.to_string(), "4576");
        assert_eq!(decode_aggregate3(Some(&blob), 2), vec![None, None]);
        assert_eq!(decode_aggregate3(None, 1), vec![None]);
    }

    #[test]
    fn unknown_networks_have_no_chain() {
        assert!(chain_of("sepolia").is_none());
        assert!(chain_of("").is_none());
        assert_eq!(chain_of("matic"), Some(ChainNet::Matic));
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
