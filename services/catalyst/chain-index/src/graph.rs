//! The subgraph readers: owned wearables/emotes and per-owner ownership checks
//! from the L1/L2 collections subgraphs, ENS names from the marketplace
//! subgraph. The old handlers read these from the squid mirror
//! (`squid_marketplace.nft`, `mv.owned_nft`) and had no subgraph path for
//! them; these queries are the ones the reference lambdas ran against the
//! same subgraphs. Every call goes through the shared client with the SSRF
//! guard, the retry ladder for 429/5xx and an 8 MiB body cap; answers are
//! memoized per owner for [`OWNED_TTL`] so a profile burst costs one round
//! trip per network.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Duration;

use serde_json::{Value, json};

use crate::http::{
    RetryDecision, is_transient_status, parse_retry_after, read_body_capped, retry_with_backoff,
};
use crate::shared::cache::TtlMap;
use crate::shared::identity::is_valid_eth_address;

use super::ChainIndex;

pub const THE_GRAPH_PAGE_SIZE: i64 = 1000;
/// Pages walked per owner and network before the answer is cut off: 10,000
/// tokens, past which the explorer paginates a truncated set rather than the
/// node walking a whale's inventory on every request.
const MAX_PAGES: usize = 10;
const BODY_CAP: usize = 8 * 1024 * 1024;
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
const RETRY_ATTEMPTS: u32 = 3;
const RETRY_BASE_MS: u64 = 500;
pub const OWNED_TTL: Duration = Duration::from_secs(30);
const OWNED_CACHE_MAX_ENTRIES: usize = 20_000;
/// Owners folded into one aliased ownership document.
const OWNERS_PER_QUERY: usize = 20;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Network {
    Ethereum,
    Matic,
}

impl Network {
    pub fn url(self, state: &ChainIndex) -> Option<&str> {
        match self {
            Network::Ethereum => state.settings.eth_collections_subgraph_url.as_deref(),
            Network::Matic => state.settings.matic_collections_subgraph_url.as_deref(),
        }
    }

    /// The mirrors' `network` column and the subgraphs' spelling.
    pub fn db_name(self) -> &'static str {
        match self {
            Network::Ethereum => "ethereum",
            Network::Matic => "matic",
        }
    }

    pub fn parse_db(s: &str) -> Option<Self> {
        match s {
            "ethereum" | "mainnet" | "ETHEREUM" => Some(Network::Ethereum),
            "matic" | "polygon" | "MATIC" | "POLYGON" => Some(Network::Matic),
            _ => None,
        }
    }

    /// The collections subgraph a URN's third segment names; `None` for
    /// off-chain and third-party URNs, which no subgraph indexes.
    pub fn of_urn(urn: &str) -> Option<Self> {
        match urn.split(':').nth(2) {
            Some("ethereum") | Some("mainnet") => Some(Network::Ethereum),
            Some("matic") | Some("polygon") => Some(Network::Matic),
            _ => None,
        }
    }
}

/// One owned copy of a collection item, the columns the old owner queries projected.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OwnedCopy {
    pub urn: String,
    pub token_id: String,
    pub transferred_at: i64,
    pub price: String,
    pub rarity: String,
    pub name: String,
    pub category: String,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct OwnedName {
    pub name: String,
    #[serde(rename = "contractAddress")]
    pub contract_address: String,
    #[serde(rename = "tokenId")]
    pub token_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub price: Option<String>,
}

/// A Genesis parcel as the marketplace subgraph indexes it: the parcel NFT's
/// owner (the estate contract when the parcel sits in an estate) and, then,
/// the estate's owner.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LandParcel {
    pub x: i32,
    pub y: i32,
    pub owner: String,
    pub estate_owner: Option<String>,
}

/// One parcel or estate NFT the owner holds, as the marketplace subgraph
/// indexes it: the columns the old `/lambdas/users/{a}/lands` row projected.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OwnedLand {
    pub contract_address: String,
    pub token_id: String,
    pub category: String,
    pub name: Option<String>,
    pub x: Option<i32>,
    pub y: Option<i32>,
    /// `Some` when the LAND carries a data record (its description, possibly empty).
    pub description: Option<String>,
    pub price: Option<String>,
    pub transferred_at: i64,
}

pub struct Caches {
    /// `(owner, category)` -> every copy the owner holds across both networks.
    owned: TtlMap<(String, String), Arc<Vec<OwnedCopy>>>,
    /// `owner` -> the ENS names the owner holds.
    pub names: TtlMap<String, Arc<Vec<OwnedName>>>,
    /// The sorted coordinate list of a place -> its parcels and their owners.
    land: TtlMap<Vec<(i32, i32)>, Arc<Vec<LandParcel>>>,
    /// lowercased DCL name -> its holder.
    pub name_owner: TtlMap<String, Arc<Option<String>>>,
    /// `owner` -> the parcels and estates the owner holds, newest transfer first.
    lands: TtlMap<String, Arc<Vec<OwnedLand>>>,
}

impl Default for Caches {
    fn default() -> Self {
        Self {
            owned: TtlMap::bounded(OWNED_TTL, OWNED_CACHE_MAX_ENTRIES),
            names: TtlMap::bounded(OWNED_TTL, OWNED_CACHE_MAX_ENTRIES),
            land: TtlMap::bounded(OWNED_TTL, OWNED_CACHE_MAX_ENTRIES),
            name_owner: TtlMap::bounded(OWNED_TTL, OWNED_CACHE_MAX_ENTRIES),
            lands: TtlMap::bounded(OWNED_TTL, OWNED_CACHE_MAX_ENTRIES),
        }
    }
}

/// `urn:decentraland:mainnet:...` and `urn:decentraland:ethereum:...` name the
/// same L1 item; the subgraphs spell it `ethereum`, so that is the canonical
/// form here (the squid spelled it `mainnet`, and the old code normalized the
/// other way).
pub fn normalize_urn(urn: &str) -> String {
    urn.replacen(":mainnet:", ":ethereum:", 1)
}

/// POST one GraphQL document; transient answers (429, 5xx, a dropped
/// connection) retry on the shared ladder, anything else is the error text.
pub async fn graph_query(
    state: &ChainIndex,
    url: &str,
    query: &str,
    variables: Value,
) -> Result<Value, String> {
    if !state.upstream_allowed(url) {
        return Err("subgraph URL is not a publicly routable http(s) endpoint".to_string());
    }
    let client = &state.http;
    let body = json!({ "query": query, "variables": variables });
    let outcome = retry_with_backoff("subgraph", RETRY_ATTEMPTS, RETRY_BASE_MS, || {
        let body = body.clone();
        async move {
            let resp = match client
                .post(url)
                .timeout(REQUEST_TIMEOUT)
                .json(&body)
                .send()
                .await
            {
                Ok(resp) => resp,
                Err(e) if e.is_connect() || e.is_timeout() || e.is_request() => {
                    tracing::debug!(error = %e, "subgraph request failed; retrying");
                    return Ok(RetryDecision::Retry {
                        after: None,
                        rate_limited: false,
                    });
                }
                Err(e) => return Err(anyhow::anyhow!("subgraph request failed: {e}")),
            };
            let status = resp.status();
            if is_transient_status(status) {
                let after = resp
                    .headers()
                    .get(reqwest::header::RETRY_AFTER)
                    .and_then(|v| v.to_str().ok())
                    .and_then(parse_retry_after);
                return Ok(RetryDecision::Retry {
                    after,
                    rate_limited: status == reqwest::StatusCode::TOO_MANY_REQUESTS,
                });
            }
            if !status.is_success() {
                return Err(anyhow::anyhow!("subgraph returned HTTP {status}"));
            }
            let bytes = read_body_capped(resp, BODY_CAP).await?;
            let parsed: Value = serde_json::from_slice(&bytes)
                .map_err(|e| anyhow::anyhow!("failed to parse subgraph response: {e}"))?;
            Ok(RetryDecision::Done(parsed))
        }
    })
    .await
    .map_err(|e| e.to_string())?;
    let body = outcome.ok_or_else(|| "subgraph unavailable after retries".to_string())?;
    if let Some(errors) = body
        .get("errors")
        .and_then(Value::as_array)
        .filter(|e| !e.is_empty())
    {
        return Err(format!("subgraph errors: {}", Value::Array(errors.clone())));
    }
    body.get("data")
        .cloned()
        .filter(|d| !d.is_null())
        .ok_or_else(|| format!("subgraph response without data: {body}"))
}

const QUERY_OWNED: &str = r#"
query owned($owner: String!, $idFrom: ID!, $first: Int!) {
  nfts(where: { owner: $owner, category: "CATEGORY", id_gt: $idFrom }, orderBy: id, orderDirection: asc, first: $first) {
    id
    urn
    tokenId
    transferredAt
    item { rarity price }
    metadata { wearable { name category } emote { name category } }
  }
}"#;

fn str_of(v: &Value, key: &str) -> String {
    v.get(key).and_then(Value::as_str).unwrap_or("").to_string()
}

fn int_of(v: &Value, key: &str) -> i64 {
    match v.get(key) {
        Some(Value::String(s)) => s.parse().unwrap_or(0),
        Some(Value::Number(n)) => n.as_i64().unwrap_or(0),
        _ => 0,
    }
}

pub(crate) fn parse_owned_copies(data: &Value, category: &str) -> Vec<OwnedCopy> {
    let meta_key = if category == "emote" {
        "emote"
    } else {
        "wearable"
    };
    data.get("nfts")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|nft| {
            let urn = nft.get("urn").and_then(Value::as_str)?;
            let item = nft.get("item").filter(|i| !i.is_null());
            let meta = nft
                .get("metadata")
                .and_then(|m| m.get(meta_key))
                .filter(|m| !m.is_null());
            Some(OwnedCopy {
                urn: normalize_urn(urn),
                token_id: str_of(nft, "tokenId"),
                transferred_at: int_of(nft, "transferredAt"),
                price: item
                    .map(|i| str_of(i, "price"))
                    .filter(|p| !p.is_empty())
                    .unwrap_or_else(|| "0".into()),
                rarity: item.map(|i| str_of(i, "rarity")).unwrap_or_default(),
                name: meta.map(|m| str_of(m, "name")).unwrap_or_default(),
                category: meta.map(|m| str_of(m, "category")).unwrap_or_default(),
            })
        })
        .collect()
}

fn last_id(data: &Value) -> Option<String> {
    data.get("nfts")?
        .as_array()?
        .last()?
        .get("id")?
        .as_str()
        .map(str::to_string)
}

fn page_len(data: &Value) -> usize {
    data.get("nfts")
        .and_then(Value::as_array)
        .map(Vec::len)
        .unwrap_or(0)
}

async fn owned_on_network(
    state: &ChainIndex,
    network: Network,
    owner: &str,
    category: &str,
) -> Result<Vec<OwnedCopy>, String> {
    let Some(url) = network.url(state) else {
        return Ok(Vec::new());
    };
    let query = QUERY_OWNED.replace("CATEGORY", category);
    let mut out = Vec::new();
    let mut id_from = String::new();
    for _ in 0..MAX_PAGES {
        let data = graph_query(
            state,
            url,
            &query,
            json!({ "owner": owner, "idFrom": id_from, "first": THE_GRAPH_PAGE_SIZE }),
        )
        .await?;
        let n = page_len(&data);
        out.extend(parse_owned_copies(&data, category));
        if (n as i64) < THE_GRAPH_PAGE_SIZE {
            return Ok(out);
        }
        match last_id(&data) {
            Some(id) => id_from = id,
            None => return Ok(out),
        }
    }
    tracing::warn!(
        owner,
        category,
        ?network,
        "owned items cut off at {MAX_PAGES} pages"
    );
    Ok(out)
}

/// Every copy of `category` (`wearable` | `emote`) the owner holds on both
/// networks, newest transfer first. `Ok(empty)` when no subgraph is configured.
pub async fn owned_copies(
    state: &ChainIndex,
    owner: &str,
    category: &str,
) -> Result<Arc<Vec<OwnedCopy>>, String> {
    let owner = owner.to_lowercase();
    let key = (owner.clone(), category.to_string());
    state
        .graph
        .owned
        .get_or_fetch(key, || async move {
            let (eth, matic) = tokio::join!(
                owned_on_network(state, Network::Ethereum, &owner, category),
                owned_on_network(state, Network::Matic, &owner, category),
            );
            let mut all = eth?;
            all.extend(matic?);
            all.sort_by(|a, b| {
                b.transferred_at
                    .cmp(&a.transferred_at)
                    .then_with(|| a.token_id.cmp(&b.token_id))
                    .then_with(|| a.price.cmp(&b.price))
            });
            Ok::<_, String>(Arc::new(all))
        })
        .await
}

pub fn any_collections_reader(state: &ChainIndex) -> bool {
    state.settings.eth_collections_subgraph_url.is_some()
        || state.settings.matic_collections_subgraph_url.is_some()
}

fn alias(i: usize) -> String {
    format!("o{i}")
}

/// One aliased document per network and chunk of owners: `oN: nfts(where:
/// {owner, urn_in}) { urn }`. Owners are validated hex addresses and the
/// URNs are JSON-encoded, so the inlined literals cannot break the document.
fn ownership_document(chunk: &[(&str, &[String])]) -> String {
    let mut doc = String::from("query {");
    for (i, (owner, urns)) in chunk.iter().enumerate() {
        let urns = serde_json::to_string(urns).unwrap_or_else(|_| "[]".to_string());
        doc.push_str(&format!(
            " {}: nfts(first: 1000, where: {{ owner: \"{}\", urn_in: {} }}) {{ urn }}",
            alias(i),
            owner,
            urns
        ));
    }
    doc.push_str(" }");
    doc
}

/// Which of the requested `(owner, urn)` pairs the subgraphs confirm: the
/// answer maps each owner to the normalized URNs it holds. Third-party and
/// off-chain URNs have no reader and never come back owned. An owner whose
/// address is not hex is skipped (nothing owned).
pub async fn owned_urns(
    state: &ChainIndex,
    requested: &HashSet<(String, String)>,
) -> Result<HashMap<String, HashSet<String>>, String> {
    let mut by_network: HashMap<Network, HashMap<String, Vec<String>>> = HashMap::new();
    for (owner, urn) in requested {
        if !is_valid_eth_address(owner) {
            continue;
        }
        let Some(network) = Network::of_urn(urn) else {
            continue;
        };
        by_network
            .entry(network)
            .or_default()
            .entry(owner.to_lowercase())
            .or_default()
            .push(normalize_urn(urn));
    }
    let mut owned: HashMap<String, HashSet<String>> = HashMap::new();
    for (network, owners) in by_network {
        let Some(url) = network.url(state) else {
            continue;
        };
        let mut owners: Vec<(&str, &[String])> = owners
            .iter()
            .map(|(o, u)| (o.as_str(), u.as_slice()))
            .collect();
        owners.sort();
        for chunk in owners.chunks(OWNERS_PER_QUERY) {
            let data = graph_query(state, url, &ownership_document(chunk), json!({})).await?;
            for (i, (owner, _)) in chunk.iter().enumerate() {
                let urns = data
                    .get(alias(i))
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                    .filter_map(|n| n.get("urn").and_then(Value::as_str))
                    .map(normalize_urn);
                owned.entry((*owner).to_string()).or_default().extend(urns);
            }
        }
    }
    Ok(owned)
}

const QUERY_NAMES: &str = r#"
query names($owner: String!, $idFrom: ID!, $first: Int!) {
  nfts(where: { owner: $owner, category: ens, id_gt: $idFrom }, orderBy: id, orderDirection: asc, first: $first) {
    id
    name
    contractAddress
    tokenId
    activeOrder { price }
  }
}"#;

pub(crate) fn parse_names(data: &Value) -> Vec<OwnedName> {
    data.get("nfts")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|nft| {
            let name = nft.get("name").and_then(Value::as_str)?;
            Some(OwnedName {
                name: name.to_string(),
                contract_address: str_of(nft, "contractAddress"),
                token_id: str_of(nft, "tokenId"),
                price: nft
                    .get("activeOrder")
                    .filter(|o| !o.is_null())
                    .map(|o| str_of(o, "price"))
                    .filter(|p| !p.is_empty()),
            })
        })
        .collect()
}

/// The ENS names the owner holds, in subgraph id order. `Ok(empty)` when no
/// marketplace subgraph is configured.
pub async fn owned_names(state: &ChainIndex, owner: &str) -> Result<Arc<Vec<OwnedName>>, String> {
    let owner = owner.to_lowercase();
    state
        .graph
        .names
        .get_or_fetch(owner.clone(), || async move {
            let Some(url) = state.settings.marketplace_subgraph_url.as_deref() else {
                return Ok(Arc::new(Vec::new()));
            };
            if !is_valid_eth_address(&owner) {
                return Ok(Arc::new(Vec::new()));
            }
            let mut out = Vec::new();
            let mut id_from = String::new();
            for _ in 0..MAX_PAGES {
                let data = graph_query(
                    state,
                    url,
                    QUERY_NAMES,
                    json!({ "owner": owner, "idFrom": id_from, "first": THE_GRAPH_PAGE_SIZE }),
                )
                .await?;
                let n = page_len(&data);
                out.extend(parse_names(&data));
                if (n as i64) < THE_GRAPH_PAGE_SIZE {
                    break;
                }
                match last_id(&data) {
                    Some(id) => id_from = id,
                    None => break,
                }
            }
            Ok::<_, String>(Arc::new(out))
        })
        .await
}

/// The parcel NFTs standing on the requested coordinates. `searchParcelX_in`
/// x `searchParcelY_in` is a superset (the cross product of the distinct
/// columns and rows), filtered back to the requested pairs here; the old
/// comms crate ran the same shape over the squid `nft` table
/// (`search_parcel_x/y`, `search_parcel_estate_id`).
const QUERY_PARCELS_AT: &str = r#"
query parcelsAt($xs: [BigInt!]!, $ys: [BigInt!]!, $idFrom: ID!, $first: Int!) {
  nfts(where: { category: parcel, searchParcelX_in: $xs, searchParcelY_in: $ys, id_gt: $idFrom }, orderBy: id, orderDirection: asc, first: $first) {
    id
    searchParcelX
    searchParcelY
    owner { id }
    parcel { estate { owner { id } } }
  }
}"#;

pub(crate) fn parse_parcels(data: &Value) -> Vec<LandParcel> {
    data.get("nfts")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|nft| {
            let coord = |key: &str| -> Option<i32> {
                match nft.get(key)? {
                    Value::String(s) => s.parse().ok(),
                    Value::Number(n) => i32::try_from(n.as_i64()?).ok(),
                    _ => None,
                }
            };
            let (x, y) = (coord("searchParcelX")?, coord("searchParcelY")?);
            let owner = nft.get("owner")?.get("id")?.as_str()?.to_lowercase();
            let estate_owner = nft
                .pointer("/parcel/estate/owner/id")
                .and_then(Value::as_str)
                .map(str::to_lowercase);
            Some(LandParcel {
                x,
                y,
                owner,
                estate_owner,
            })
        })
        .collect()
}

/// The parcels at `coords` (deduplicated, sorted), from the marketplace
/// subgraph. `Ok(empty)` when no marketplace subgraph is configured, so a
/// caller deciding ownership refuses rather than guesses.
pub async fn parcels_at(
    state: &ChainIndex,
    coords: &[(i32, i32)],
) -> Result<Arc<Vec<LandParcel>>, String> {
    let mut key: Vec<(i32, i32)> = coords.to_vec();
    key.sort_unstable();
    key.dedup();
    if key.is_empty() {
        return Ok(Arc::new(Vec::new()));
    }
    state
        .graph
        .land
        .get_or_fetch(key.clone(), || async move {
            let Some(url) = state.settings.marketplace_subgraph_url.as_deref() else {
                return Ok(Arc::new(Vec::new()));
            };
            let mut xs: Vec<String> = key.iter().map(|(x, _)| x.to_string()).collect();
            let mut ys: Vec<String> = key.iter().map(|(_, y)| y.to_string()).collect();
            xs.sort_unstable();
            xs.dedup();
            ys.sort_unstable();
            ys.dedup();
            let mut out = Vec::new();
            let mut id_from = String::new();
            for _ in 0..MAX_PAGES {
                let data = graph_query(
                    state,
                    url,
                    QUERY_PARCELS_AT,
                    json!({ "xs": xs, "ys": ys, "idFrom": id_from, "first": THE_GRAPH_PAGE_SIZE }),
                )
                .await?;
                let n = page_len(&data);
                out.extend(
                    parse_parcels(&data)
                        .into_iter()
                        .filter(|p| key.binary_search(&(p.x, p.y)).is_ok()),
                );
                if (n as i64) < THE_GRAPH_PAGE_SIZE {
                    break;
                }
                match last_id(&data) {
                    Some(id) => id_from = id,
                    None => break,
                }
            }
            Ok::<_, String>(Arc::new(out))
        })
        .await
}

/// The ENS name's holder: `searchText` is the subgraph's lowercased name, so
/// the match is case-insensitive like the old `lower(name) = lower($1)`.
const QUERY_NAME_OWNER: &str = r#"
query nameOwner($name: String!) {
  nfts(where: { category: ens, searchText: $name }, orderBy: id, orderDirection: asc, first: 1) {
    id
    owner { id }
  }
}"#;

pub(crate) fn parse_name_owner(data: &Value) -> Option<String> {
    data.pointer("/nfts/0/owner/id")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .map(str::to_lowercase)
}

/// Who holds the DCL name (`.dcl.eth` already stripped); `Ok(None)` when
/// nobody does or no marketplace subgraph is configured.
pub async fn name_owner(state: &ChainIndex, name: &str) -> Result<Option<String>, String> {
    let name = name.to_lowercase();
    let Some(url) = state.settings.marketplace_subgraph_url.as_deref() else {
        return Ok(None);
    };
    state
        .graph
        .name_owner
        .get_or_fetch(name.clone(), || async move {
            let data = graph_query(state, url, QUERY_NAME_OWNER, json!({ "name": name })).await?;
            Ok::<_, String>(Arc::new(parse_name_owner(&data)))
        })
        .await
        .map(|a| (*a).clone())
}

/// The holders of up to [`NAMES_PER_QUERY`] names in one round trip.
const QUERY_NAME_OWNERS: &str = r#"
query nameOwners($names: [String!]!, $first: Int!) {
  nfts(where: { category: ens, searchText_in: $names }, orderBy: id, orderDirection: asc, first: $first) {
    id
    name
    owner { id }
  }
}"#;

/// Names [`name_owners`] asks per query.
pub const NAMES_PER_QUERY: usize = 100;

/// The first token per name (lowest id, as [`name_owner`] picks), so both
/// lookups name the same holder of a name with several tokens.
pub(crate) fn parse_name_owners(data: &Value) -> HashMap<String, String> {
    let mut first: HashMap<String, Option<String>> = HashMap::new();
    for nft in data
        .get("nfts")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let Some(name) = nft.get("name").and_then(Value::as_str) else {
            continue;
        };
        let owner = nft
            .pointer("/owner/id")
            .and_then(Value::as_str)
            .filter(|o| !o.is_empty())
            .map(str::to_lowercase);
        first.entry(name.to_lowercase()).or_insert(owner);
    }
    first
        .into_iter()
        .filter_map(|(name, owner)| Some((name, owner?)))
        .collect()
}

/// Lowercased DCL name -> holder for the `names` (`.dcl.eth` stripped) the
/// subgraph knows, [`NAMES_PER_QUERY`] per query, not memoized; a name it
/// does not list (or a full page cut off) is absent. `Ok(empty)` when no
/// marketplace subgraph is configured.
pub async fn name_owners(
    state: &ChainIndex,
    names: &[String],
) -> Result<HashMap<String, String>, String> {
    let Some(url) = state.settings.marketplace_subgraph_url.as_deref() else {
        return Ok(HashMap::new());
    };
    let mut out = HashMap::new();
    for chunk in names.chunks(NAMES_PER_QUERY) {
        let chunk: Vec<String> = chunk.iter().map(|n| n.to_lowercase()).collect();
        let data = graph_query(
            state,
            url,
            QUERY_NAME_OWNERS,
            json!({ "names": chunk, "first": THE_GRAPH_PAGE_SIZE }),
        )
        .await?;
        if page_len(&data) as i64 >= THE_GRAPH_PAGE_SIZE {
            tracing::warn!(
                names = chunk.len(),
                "name owners page full; later names unresolved"
            );
        }
        out.extend(parse_name_owners(&data));
    }
    Ok(out)
}

const QUERY_OWNED_LANDS: &str = r#"
query ownedLands($owner: String!, $idFrom: ID!, $first: Int!) {
  nfts(where: { owner: $owner, category_in: [parcel, estate], id_gt: $idFrom }, orderBy: id, orderDirection: asc, first: $first) {
    id
    name
    contractAddress
    tokenId
    category
    searchParcelX
    searchParcelY
    transferredAt
    parcel { data { description } }
    estate { data { description } }
    activeOrder { price }
  }
}"#;

pub(crate) fn parse_owned_lands(data: &Value) -> Vec<OwnedLand> {
    let coord = |nft: &Value, key: &str| {
        nft.get(key)
            .and_then(Value::as_str)
            .and_then(|s| s.parse::<i32>().ok())
    };
    let data_of = |nft: &Value, leg: &str| {
        nft.get(leg)
            .filter(|l| !l.is_null())
            .and_then(|l| l.get("data"))
            .filter(|d| !d.is_null())
            .map(|d| {
                d.get("description")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string()
            })
    };
    data.get("nfts")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .map(|nft| {
            let category = str_of(nft, "category");
            OwnedLand {
                contract_address: str_of(nft, "contractAddress").to_lowercase(),
                token_id: str_of(nft, "tokenId"),
                description: data_of(nft, &category),
                name: nft.get("name").and_then(Value::as_str).map(str::to_string),
                x: coord(nft, "searchParcelX"),
                y: coord(nft, "searchParcelY"),
                price: nft
                    .get("activeOrder")
                    .filter(|o| !o.is_null())
                    .map(|o| str_of(o, "price"))
                    .filter(|p| !p.is_empty()),
                transferred_at: nft
                    .get("transferredAt")
                    .and_then(Value::as_str)
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(0),
                category,
            }
        })
        .collect()
}

/// The parcels and estates `owner` holds, newest transfer first (the old
/// `transferred_at DESC`), walked in id pages up to [`MAX_PAGES`].
/// `Ok(empty)` when no marketplace subgraph is configured.
pub async fn owned_lands(state: &ChainIndex, owner: &str) -> Result<Arc<Vec<OwnedLand>>, String> {
    let owner = owner.to_lowercase();
    state
        .graph
        .lands
        .get_or_fetch(owner.clone(), || async move {
            let Some(url) = state.settings.marketplace_subgraph_url.as_deref() else {
                return Ok(Arc::new(Vec::new()));
            };
            if !is_valid_eth_address(&owner) {
                return Ok(Arc::new(Vec::new()));
            }
            let mut out = Vec::new();
            let mut id_from = String::new();
            for _ in 0..MAX_PAGES {
                let data = graph_query(
                    state,
                    url,
                    QUERY_OWNED_LANDS,
                    json!({ "owner": owner, "idFrom": id_from, "first": THE_GRAPH_PAGE_SIZE }),
                )
                .await?;
                let n = page_len(&data);
                out.extend(parse_owned_lands(&data));
                if (n as i64) < THE_GRAPH_PAGE_SIZE {
                    break;
                }
                match last_id(&data) {
                    Some(id) => id_from = id,
                    None => break,
                }
            }
            out.sort_by(|a, b| {
                b.transferred_at
                    .cmp(&a.transferred_at)
                    .then_with(|| a.contract_address.cmp(&b.contract_address))
                    .then_with(|| a.token_id.cmp(&b.token_id))
            });
            Ok::<_, String>(Arc::new(out))
        })
        .await
}

/// Whether `owner` holds any of the parcels at `coords`, directly or through
/// the estate the parcel sits in: the LAND half of "scene owner".
pub async fn owns_land_at(
    state: &ChainIndex,
    owner: &str,
    coords: &[(i32, i32)],
) -> Result<bool, String> {
    if !is_valid_eth_address(owner) {
        return Ok(false);
    }
    let owner = owner.to_lowercase();
    let parcels = parcels_at(state, coords).await?;
    Ok(parcels
        .iter()
        .any(|p| p.owner == owner || p.estate_owner.as_deref() == Some(owner.as_str())))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn name_owners_map_each_lowercased_name_to_its_holder() {
        let owners = parse_name_owners(&json!({ "nfts": [
            { "id": "ens-1", "name": "Boedo", "owner": { "id": "0xABCD" } },
            { "id": "ens-2", "name": "palermo", "owner": { "id": "" } },
            { "id": "ens-3", "owner": { "id": "0x01" } },
            { "id": "ens-4", "name": "boedo", "owner": { "id": "0x02" } },
            { "id": "ens-5", "name": "palermo", "owner": { "id": "0x03" } }
        ] }));
        assert_eq!(owners.len(), 1);
        assert_eq!(owners.get("boedo").map(String::as_str), Some("0xabcd"));
    }

    fn holder(i: usize) -> crate::shared::identity::Address {
        crate::shared::identity::Address::parse(&format!("0x{i:040x}")).unwrap()
    }

    #[tokio::test]
    async fn name_owners_asks_100_names_per_query() {
        let (index, names) = test_support::index_with_names().await;
        let labels: Vec<String> = (0..150).map(|i| format!("name{i}")).collect();
        for (i, l) in labels.iter().enumerate() {
            names.set(&holder(i + 1), l);
        }
        let owners = name_owners(&index, &labels).await.unwrap();
        assert_eq!(owners.len(), 150);
        assert_eq!(owners["name149"], holder(150).as_str());
        assert_eq!(names.batch_queries(), 2);
    }

    #[tokio::test]
    async fn a_name_with_several_tokens_has_one_holder_and_crowds_out_no_other() {
        let (index, names) = test_support::index_with_names().await;
        for i in [3, 1, 2] {
            names.set(&holder(i), "aaa");
        }
        names.set(&holder(9), "zzz");
        let labels = vec!["aaa".to_string(), "zzz".to_string()];
        let owners = name_owners(&index, &labels).await.unwrap();
        assert_eq!(owners["aaa"], holder(1).as_str());
        assert_eq!(owners["zzz"], holder(9).as_str());
        assert_eq!(
            name_owner(&index, "AAA").await.unwrap().as_deref(),
            Some(holder(1).as_str())
        );
    }

    #[test]
    fn name_owner_is_the_first_ens_holder_lowercased() {
        assert_eq!(
            parse_name_owner(&json!({ "nfts": [{ "id": "ens-1", "owner": { "id": "0xABCD" } }] }))
                .as_deref(),
            Some("0xabcd")
        );
        assert_eq!(parse_name_owner(&json!({ "nfts": [] })), None);
        assert_eq!(
            parse_name_owner(&json!({ "nfts": [{ "id": "ens-1", "owner": { "id": "" } }] })),
            None
        );
    }

    #[test]
    fn owned_lands_carry_the_old_row_columns() {
        let data = json!({ "nfts": [
            { "id": "parcel-0xF87E-5", "name": null, "contractAddress": "0xF87E", "tokenId": "5", "category": "parcel", "searchParcelX": "15", "searchParcelY": "-73",
              "transferredAt": "1693411667", "parcel": { "data": null }, "estate": null, "activeOrder": null },
            { "id": "estate-0x959e-9", "name": "Home", "contractAddress": "0x959e", "tokenId": "9", "category": "estate", "searchParcelX": null, "searchParcelY": null,
              "transferredAt": "1700000000", "parcel": null, "estate": { "data": { "description": "" } }, "activeOrder": { "price": "1000" } }
        ] });
        let lands = parse_owned_lands(&data);
        assert_eq!(
            lands[0],
            OwnedLand {
                contract_address: "0xf87e".into(),
                token_id: "5".into(),
                category: "parcel".into(),
                name: None,
                x: Some(15),
                y: Some(-73),
                description: None,
                price: None,
                transferred_at: 1693411667
            }
        );
        assert_eq!(
            lands[1],
            OwnedLand {
                contract_address: "0x959e".into(),
                token_id: "9".into(),
                category: "estate".into(),
                name: Some("Home".into()),
                x: None,
                y: None,
                description: Some(String::new()),
                price: Some("1000".into()),
                transferred_at: 1700000000
            }
        );
    }

    #[test]
    fn parcels_parse_the_owner_and_the_estate_owner() {
        let data = json!({ "nfts": [
            { "id": "0xland-1", "searchParcelX": "10", "searchParcelY": "20", "owner": { "id": "0xAAAA" }, "parcel": { "estate": null } },
            { "id": "0xland-2", "searchParcelX": 10, "searchParcelY": 21, "owner": { "id": "0xestatecontract" },
              "parcel": { "estate": { "owner": { "id": "0xBBBB" } } } },
            { "id": "no-coords", "owner": { "id": "0xcccc" } },
            { "id": "no-owner", "searchParcelX": "1", "searchParcelY": "1" }
        ]});
        let parcels = parse_parcels(&data);
        assert_eq!(
            parcels,
            vec![
                LandParcel {
                    x: 10,
                    y: 20,
                    owner: "0xaaaa".into(),
                    estate_owner: None
                },
                LandParcel {
                    x: 10,
                    y: 21,
                    owner: "0xestatecontract".into(),
                    estate_owner: Some("0xbbbb".into())
                },
            ]
        );
    }

    #[test]
    fn network_of_urn_follows_the_third_segment() {
        assert_eq!(
            Network::of_urn("urn:decentraland:ethereum:collections-v1:x:y"),
            Some(Network::Ethereum)
        );
        assert_eq!(
            Network::of_urn("urn:decentraland:mainnet:collections-v1:x:y"),
            Some(Network::Ethereum)
        );
        assert_eq!(
            Network::of_urn("urn:decentraland:matic:collections-v2:0xabc:1"),
            Some(Network::Matic)
        );
        assert_eq!(
            Network::of_urn("urn:decentraland:off-chain:base-avatars:eyes_00"),
            None
        );
        assert_eq!(
            Network::of_urn("urn:decentraland:matic:collections-thirdparty:tp:c:i"),
            Some(Network::Matic)
        );
        assert_eq!(Network::of_urn("nope"), None);
    }

    #[test]
    fn normalize_urn_maps_mainnet_to_ethereum() {
        assert_eq!(
            normalize_urn("urn:decentraland:mainnet:collections-v1:0xabc:hat"),
            "urn:decentraland:ethereum:collections-v1:0xabc:hat"
        );
        assert!(normalize_urn("a:ethereum:b").contains(":ethereum:"));
    }

    #[test]
    fn owned_copies_parse_the_item_and_metadata_legs() {
        let data = json!({ "nfts": [
            { "id": "0xc0ffee-1", "urn": "urn:decentraland:matic:collections-v2:0xc0ffee:1", "tokenId": "7",
              "transferredAt": "100", "item": { "rarity": "epic", "price": "10" },
              "metadata": { "wearable": { "name": "Hat", "category": "hat" }, "emote": null } },
            { "id": "0xold-1", "urn": "urn:decentraland:mainnet:collections-v1:oldies:hat", "tokenId": "1",
              "transferredAt": 50, "item": null, "metadata": null },
            { "id": "no-urn", "tokenId": "2" }
        ]});
        let copies = parse_owned_copies(&data, "wearable");
        assert_eq!(copies.len(), 2, "a copy without a urn is dropped");
        assert_eq!(copies[0].name, "Hat");
        assert_eq!(copies[0].category, "hat");
        assert_eq!(copies[0].rarity, "epic");
        assert_eq!(copies[0].price, "10");
        assert_eq!(copies[0].transferred_at, 100);
        assert_eq!(
            copies[1].urn,
            "urn:decentraland:ethereum:collections-v1:oldies:hat"
        );
        assert_eq!(copies[1].price, "0", "no item row means price 0");
        assert_eq!(
            (copies[1].name.as_str(), copies[1].rarity.as_str()),
            ("", "")
        );
        assert_eq!(last_id(&data).as_deref(), Some("no-urn"));
        assert_eq!(page_len(&data), 3);

        let data = json!({ "nfts": [ { "id": "e", "urn": "urn:decentraland:matic:collections-v2:0xc0ffee:3",
            "tokenId": "1", "transferredAt": "400", "item": { "rarity": "rare", "price": "5" },
            "metadata": { "wearable": null, "emote": { "name": "Dance", "category": "dance" } } } ]});
        let emotes = parse_owned_copies(&data, "emote");
        assert_eq!(emotes[0].name, "Dance");
        assert_eq!(emotes[0].category, "dance");
    }

    #[test]
    fn names_parse_the_active_order_price_when_present() {
        let data = json!({ "nfts": [
            { "id": "1", "name": "alice", "contractAddress": "0xens", "tokenId": "11", "activeOrder": { "price": "1000" } },
            { "id": "2", "name": "bob", "contractAddress": "0xens", "tokenId": "22", "activeOrder": null },
            { "id": "3", "contractAddress": "0xens", "tokenId": "33" }
        ]});
        let names = parse_names(&data);
        assert_eq!(names.len(), 2, "a row without a name is dropped");
        assert_eq!(names[0].price.as_deref(), Some("1000"));
        assert_eq!(names[1].price, None);
        assert_eq!(
            serde_json::to_value(&names[1]).unwrap(),
            json!({ "name": "bob", "contractAddress": "0xens", "tokenId": "22" })
        );
    }

    #[test]
    fn ownership_document_aliases_each_owner_with_json_encoded_urns() {
        let a = vec!["urn:decentraland:matic:collections-v2:0xa:1".to_string()];
        let b = vec!["urn:x\"y".to_string(), "urn:z".to_string()];
        let doc = ownership_document(&[("0xaaaa", &a), ("0xbbbb", &b)]);
        assert!(doc.starts_with("query { o0: nfts(first: 1000, where: { owner: \"0xaaaa\", urn_in: [\"urn:decentraland:matic:collections-v2:0xa:1\"] }) { urn }"), "{doc}");
        assert!(doc.contains(" o1: nfts(first: 1000, where: { owner: \"0xbbbb\", urn_in: [\"urn:x\\\"y\",\"urn:z\"] }) { urn }"), "{doc}");
        assert!(doc.ends_with(" }"));
    }

    #[tokio::test]
    async fn unsafe_subgraph_urls_are_refused_before_any_request() {
        let state = ChainIndex::new(
            crate::http::Outbound::for_tests(),
            crate::IndexSettings::default(),
        );
        let err = graph_query(&state, "http://127.0.0.1:1/graphql", "{ x }", json!({}))
            .await
            .unwrap_err();
        assert!(err.contains("not a publicly routable"), "{err}");
    }
}

/// A loopback marketplace subgraph answering the `names` and name-owner queries from a
/// mutable registry: the name authority of the worlds tests (the old tests
/// seeded squid's `ens`/`nft` tables).
#[cfg(any(test, feature = "test-support"))]
pub mod test_support {
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex};

    use crate::http::Outbound;
    use crate::shared::identity::Address;
    use crate::{ChainIndex, IndexSettings};
    use axum::Router;
    use axum::extract::State;
    use axum::response::IntoResponse;
    use axum::routing::post;
    use serde_json::{Value, json};

    /// Owner address -> labels, plus the batch name-owner queries answered.
    #[derive(Clone, Default)]
    pub struct Names(
        Arc<Mutex<HashMap<String, Vec<String>>>>,
        Arc<std::sync::atomic::AtomicUsize>,
    );

    impl Names {
        pub fn batch_queries(&self) -> usize {
            self.1.load(std::sync::atomic::Ordering::SeqCst)
        }
        pub fn set(&self, owner: &Address, label: &str) {
            self.0
                .lock()
                .unwrap()
                .entry(owner.as_str().to_string())
                .or_default()
                .push(label.to_string());
        }
        pub fn forget(&self, owner: &Address) {
            self.0.lock().unwrap().remove(owner.as_str());
        }
    }

    async fn answer(
        State(names): State<Names>,
        axum::Json(body): axum::Json<Value>,
    ) -> axum::response::Response {
        let query = body["query"].as_str().unwrap_or("");
        // Every holder of `wanted` is one token, id-ordered and cut at `first`.
        let tokens = |wanted: &[&str], first: usize| -> Vec<Value> {
            let names = names.0.lock().unwrap();
            let mut out: Vec<(String, Value)> = names
                .iter()
                .flat_map(|(owner, labels)| labels.iter().map(move |l| (owner, l)))
                .filter(|(_, l)| wanted.iter().any(|w| l.eq_ignore_ascii_case(w)))
                .map(|(owner, label)| {
                    let id = format!("0xname-{}-{owner}", label.to_lowercase());
                    let nft = json!({ "id": id, "name": label, "owner": { "id": owner } });
                    (id, nft)
                })
                .collect();
            out.sort_by(|a, b| a.0.cmp(&b.0));
            out.into_iter().take(first).map(|(_, nft)| nft).collect()
        };
        let nfts: Vec<Value> = if query.contains("nameOwners(") {
            names.1.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            let wanted: Vec<&str> = body["variables"]["names"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
                .collect();
            let first = body["variables"]["first"].as_u64().unwrap_or(100) as usize;
            tokens(&wanted, first)
        } else if query.contains("nameOwner(") {
            tokens(&[body["variables"]["name"].as_str().unwrap_or("")], 1)
        } else if query.contains("names(") {
            let owner = body["variables"]["owner"]
                .as_str()
                .unwrap_or("")
                .to_lowercase();
            names
                .0
                .lock()
                .unwrap()
                .get(&owner)
                .map(|labels| {
                    labels
                        .iter()
                        .enumerate()
                        .map(|(i, l)| json!({ "id": format!("0xname-{owner}-{i}"), "name": l, "contractAddress": "0xregistrar", "tokenId": i.to_string(), "activeOrder": null }))
                        .collect()
                })
                .unwrap_or_default()
        } else {
            Vec::new()
        };
        axum::Json(json!({ "data": { "nfts": nfts } })).into_response()
    }

    pub async fn fake_names_subgraph(names: Names) -> String {
        let app = Router::new()
            .route("/subgraph", post(answer))
            .with_state(names);
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        format!("http://{addr}/subgraph")
    }

    /// A chain index whose only reader is the fake, plus its registry.
    pub async fn index_with_names() -> (Arc<ChainIndex>, Names) {
        let names = Names::default();
        let subgraph = fake_names_subgraph(names.clone()).await;
        let settings = IndexSettings {
            marketplace_subgraph_url: Some(subgraph),
            eth_network: "mainnet".to_string(),
            ..IndexSettings::default()
        };
        let index = ChainIndex::new(Outbound::for_tests(), settings);
        index.allow_loopback_for_tests();
        (Arc::new(index), names)
    }
}
