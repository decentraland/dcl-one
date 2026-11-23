//! `GET /v1/rarities` and `GET /v1/rarities/:name`: the rarities the Polygon
//! collections subgraph lists (`id`, `name`, the USD `price`, `maxSupply`),
//! each with its MANA price from the node's Polygon chain's
//! rarities-with-oracle contract ([`crate::network`], `getRarityByName` over
//! `BUILDER_POLYGON_RPC_URL`), as builder-server answers them. An RPC that
//! answers another chain can't tell.
//!
//! The subgraph goes through the shared client
//! (`chain_index::graph::graph_query`); the oracle is asked once per listed
//! name, every name at once (at most [`MAX_RARITIES`]), never for a name a
//! request brings. The whole read is kept
//! [`RARITIES_TTL`] ([`crate::kept`]). A read that fails, a list that is
//! empty, longer than [`MAX_RARITIES`] or malformed, or an oracle answer for
//! another name can't tell: 503, never an empty list.

use std::sync::Arc;
use std::time::Duration;

use alloy_primitives::{U256, hex};
use alloy_sol_types::{SolCall, sol};
use dcl_http::Outbound;
use dcl_http::jsonrpc::Rpc;
use serde::Serialize;
use serde_json::{Value, json};

use crate::State;
use crate::chain_index::ChainIndex;
use crate::chain_index::graph::graph_query;
use crate::http::ApiError;
use crate::kept::{FETCH_TIMEOUT, Kept};
use crate::network::Network;

/// How long a good read answers.
pub const RARITIES_TTL: Duration = Duration::from_secs(60);
/// The most rarities a read accepts: The Graph's default page.
pub const MAX_RARITIES: usize = 100;
const NAME_MAX: usize = 64;
const AMOUNT_MAX: usize = 78;

const QUERY: &str = "query getRaritiesQuery { rarities { id name price maxSupply } }";

sol! {
    struct OracleRarity { string name; uint256 maxSupply; uint256 price; }
    function getRarityByName(string rarity) external view returns (OracleRarity);
}

/// A rarity's price in each currency.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Prices {
    #[serde(rename = "MANA")]
    pub mana: String,
    #[serde(rename = "USD")]
    pub usd: String,
}

/// One rarity as the routes answer it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Rarity {
    pub id: String,
    pub name: String,
    pub price: String,
    #[serde(rename = "maxSupply")]
    pub max_supply: String,
    pub prices: Prices,
}

pub type RarityList = Arc<Vec<Rarity>>;

/// The rarities read and its cache.
pub struct Rarities {
    kept: Kept<RarityList>,
}

impl Default for Rarities {
    fn default() -> Self {
        Self {
            kept: Kept::new(RARITIES_TTL),
        }
    }
}

impl Rarities {
    /// Every rarity, waiting for a read when none is kept; 503 when the
    /// sources can't tell.
    pub async fn all(&self, state: &State) -> Result<RarityList, ApiError> {
        self.kept
            .get(|| {
                fetch(
                    state.market.clone(),
                    state.collections_subgraph(),
                    state.http.clone(),
                    state.settings.network,
                    state.settings.polygon_rpc_url.clone(),
                )
            })
            .await
            .ok_or_else(|| {
                ApiError::service_unavailable("The rarities are not available. Try again later.")
            })
    }

    /// The rarity named `name`; 404 when the subgraph lists none.
    pub async fn named(&self, state: &State, name: &str) -> Result<Rarity, ApiError> {
        self.all(state)
            .await?
            .iter()
            .find(|r| r.name == name)
            .cloned()
            .ok_or_else(|| {
                ApiError::not_found("Rarity not found").with_data(json!({ "name": name }))
            })
    }
}

async fn fetch(
    market: Option<Arc<ChainIndex>>,
    url: Option<String>,
    http: Outbound,
    network: Network,
    rpc: Option<String>,
) -> Option<RarityList> {
    let error =
        match tokio::time::timeout(FETCH_TIMEOUT, read(market, url, &http, &network, rpc)).await {
            Ok(Ok(rarities)) => return Some(Arc::new(rarities)),
            Ok(Err(error)) => error,
            Err(_) => "timed out".to_string(),
        };
    tracing::warn!(error = %error, "builder rarities read failed; the rarities routes answer 503");
    None
}

fn amount(value: &Value) -> Option<String> {
    value
        .as_str()
        .filter(|s| !s.is_empty() && s.len() <= AMOUNT_MAX && s.bytes().all(|b| b.is_ascii_digit()))
        .map(str::to_string)
}

async fn read(
    market: Option<Arc<ChainIndex>>,
    url: Option<String>,
    http: &Outbound,
    network: &Network,
    rpc: Option<String>,
) -> Result<Vec<Rarity>, String> {
    let market = market.ok_or("no chain index")?;
    let url = url.ok_or("no Polygon collections subgraph")?;
    let rpc = rpc.ok_or("no Polygon RPC")?;
    let data = graph_query(&market, &url, QUERY, json!({})).await?;
    let listed = data
        .get("rarities")
        .and_then(Value::as_array)
        .ok_or("subgraph answered no rarities")?;
    if listed.is_empty() || listed.len() > MAX_RARITIES {
        return Err(format!("subgraph answered {} rarities", listed.len()));
    }
    let rpc = Rpc::new(http, &rpc).with_timeout(FETCH_TIMEOUT);
    let chain = rpc
        .call_hex("eth_chainId", json!([]))
        .await
        .map_err(|e| format!("chain id: {e}"))?;
    if !network.answers(&chain) {
        return Err(network.other_chain(&chain));
    }
    let mut rows = Vec::with_capacity(listed.len());
    for row in listed {
        let text = |key: &str| {
            row.get(key)
                .and_then(Value::as_str)
                .filter(|s| !s.is_empty() && s.chars().count() <= NAME_MAX)
                .map(str::to_string)
        };
        let (Some(id), Some(name), Some(usd), Some(max_supply)) = (
            text("id"),
            text("name"),
            amount(&row["price"]),
            amount(&row["maxSupply"]),
        ) else {
            return Err("subgraph answered a malformed rarity".into());
        };
        rows.push((id, name, usd, max_supply));
    }
    futures::future::try_join_all(rows.into_iter().map(|(id, name, usd, max_supply)| {
        let rpc = &rpc;
        async move {
            let call = getRarityByNameCall {
                rarity: name.clone(),
            };
            let answer = rpc
                .call_hex(
                    "eth_call",
                    json!([{ "to": network.rarities_with_oracle, "data": hex::encode_prefixed(call.abi_encode()) }, "latest"]),
                )
                .await
                .map_err(|e| format!("oracle read of {name}: {e}"))?;
            let bytes =
                hex::decode(&answer).map_err(|_| format!("oracle answered no hex for {name}"))?;
            let oracle = getRarityByNameCall::abi_decode_returns(&bytes)
                .map_err(|_| format!("oracle answered no rarity for {name}"))?;
            if oracle.name != name {
                return Err(format!("oracle answered another rarity for {name}"));
            }
            let mana: U256 = oracle.price;
            Ok(Rarity {
                id,
                name,
                price: usd.clone(),
                max_supply,
                prices: Prices {
                    mana: mana.to_string(),
                    usd,
                },
            })
        }
    }))
    .await
}
