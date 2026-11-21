//! Whether a standard collection's contract is on Polygon, asked of
//! `BUILDER_POLYGON_RPC_URL` at the address the collection's row holds:
//! `eth_getCode` there and the contract's `itemsCount()`, both at the
//! `finalized` block. The address is the CREATE2 address of the collection
//! factory for the collection's salt and its `initialize` calldata
//! (`contract::address`), so code at it is a proxy the factory deployed
//! with exactly that calldata: this collection, with its name, creator and
//! items.
//!
//! One read per contract runs at a time and its answer is kept
//! [`DEPLOYMENT_TTL`] ([`crate::kept`]); a read that fails can't tell for
//! [`crate::kept::FAILURE_TTL`]. A read never holds a database connection.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use alloy_primitives::{U256, hex};
use alloy_sol_types::SolCall;
use dcl_http::Outbound;
use dcl_http::jsonrpc::Rpc;
use serde_json::json;

use crate::State;
use crate::kept::{FETCH_TIMEOUT, Kept};
use crate::publication_chain::itemsCountCall;

/// How long an answer is kept.
pub const DEPLOYMENT_TTL: Duration = Duration::from_secs(5);
/// The most contracts kept at once; past it the cache starts over.
pub const KEPT_CONTRACTS: usize = 4096;

/// What the chain says of a contract address.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Onchain {
    /// No Polygon RPC is configured.
    Unconfigured,
    /// The RPC failed or answered something else than Polygon's facts.
    Unknown,
    /// No code at the address.
    Absent,
    /// A collection holding this many items.
    Deployed(u64),
}

/// The kept reads, one per contract address.
#[derive(Default)]
pub struct Deployments {
    kept: Mutex<HashMap<String, Arc<Kept<Option<u64>>>>>,
}

impl Deployments {
    /// What the chain says of `contract`.
    pub async fn of(&self, state: &State, contract: &str) -> Onchain {
        let Some(url) = state.settings.polygon_rpc_url.clone() else {
            return Onchain::Unconfigured;
        };
        let contract = contract.to_ascii_lowercase();
        let kept = {
            let mut kept = self.kept.lock().unwrap_or_else(|p| p.into_inner());
            if kept.len() >= KEPT_CONTRACTS && !kept.contains_key(&contract) {
                kept.clear();
            }
            kept.entry(contract.clone())
                .or_insert_with(|| Arc::new(Kept::new(DEPLOYMENT_TTL)))
                .clone()
        };
        let http = state.http.clone();
        match kept
            .get(|| fetch(http.clone(), url.clone(), contract.clone()))
            .await
        {
            None => Onchain::Unknown,
            Some(None) => Onchain::Absent,
            Some(Some(items)) => Onchain::Deployed(items),
        }
    }
}

async fn fetch(http: Outbound, url: String, contract: String) -> Option<Option<u64>> {
    let error = match tokio::time::timeout(FETCH_TIMEOUT, read(&http, &url, &contract)).await {
        Ok(Ok(answer)) => return Some(answer),
        Ok(Err(error)) => error,
        Err(_) => "timed out".to_string(),
    };
    tracing::warn!(error = %error, contract = %contract, "builder deployment read failed");
    None
}

async fn read(http: &Outbound, url: &str, contract: &str) -> Result<Option<u64>, String> {
    let rpc = Rpc::new(http, url).with_timeout(FETCH_TIMEOUT);
    let chain = rpc
        .call_hex("eth_chainId", json!([]))
        .await
        .map_err(|e| format!("chain id: {e}"))?;
    if U256::from_str_radix(chain.trim_start_matches("0x"), 16).ok() != Some(U256::from(137)) {
        return Err(format!("the RPC answers chain {chain}, not Polygon"));
    }
    let code = rpc
        .call_hex("eth_getCode", json!([contract, "finalized"]))
        .await
        .map_err(|e| format!("code: {e}"))?;
    if code.trim_start_matches("0x").is_empty() {
        return Ok(None);
    }
    let count = rpc
        .call_hex(
            "eth_call",
            json!([{ "to": contract, "data": hex::encode_prefixed(itemsCountCall {}.abi_encode()) }, "finalized"]),
        )
        .await
        .map_err(|e| format!("items count: {e}"))?;
    let bytes = hex::decode(&count).map_err(|_| "items count is not hex".to_string())?;
    let items = itemsCountCall::abi_decode_returns(&bytes)
        .map_err(|_| "items count is not a number".to_string())?;
    u64::try_from(items)
        .map(Some)
        .map_err(|_| "items count out of range".to_string())
}
