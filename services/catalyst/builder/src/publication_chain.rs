//! The receipt half of a standard publication: the client signs and submits
//! the collection-manager transaction from its own wallet and reports the
//! hash; this verifies over the Polygon JSON-RPC (`BUILDER_POLYGON_RPC_URL`,
//! through the ONE guarded client), once it answers the node's chain
//! ([`crate::network`]), that the transaction is the reviewed
//! calldata from the owner's wallet to that chain's manager, records it, and once
//! the receipt is in a finalized canonical block reads the factory event,
//! the deployed collection's creator and item count, and finishes the
//! attempt as `published` or `reverted`. A transaction the wallet sped up or
//! replaced carries the same reviewed call under a new hash: it replaces the
//! recorded one once the RPC knows neither the recorded hash nor its
//! receipt, asked twice a few seconds apart. v2 never holds a signing key.

use alloy_primitives::{Address, B256, U256, hex, keccak256};
use alloy_sol_types::{SolCall, sol};
use serde_json::{Value, json};

use crate::access::Publisher;

use dcl_http::Outbound;
use dcl_http::jsonrpc::{Rpc, RpcError};

use super::http::ApiError;
use super::items::ItemsComponent;
use super::publication::PublicationPreparation;
use super::publication_store::PublicationState;
use crate::network::Network;

/// How long the node waits before it asks the RPC a second time whether a
/// recorded transaction is gone.
pub const RECHECK: std::time::Duration = std::time::Duration::from_secs(3);

sol! {
    struct ContractItem { string rarity; uint256 price; address beneficiary; string metadata; }
    function createCollection(address forwarder, address factory, bytes32 salt, string name, string symbol, string baseURI, address creator, ContractItem[] items);
    function creator() external view returns (address);
    function itemsCount() external view returns (uint256);
}

fn invalid(message: &str) -> ApiError {
    ApiError::bad_request(message)
}
fn parse<T: std::str::FromStr>(value: &str) -> Result<T, ApiError> {
    value
        .parse()
        .map_err(|_| invalid("Invalid publication contract parameter"))
}

pub fn publication_calldata(p: &PublicationPreparation) -> Result<String, ApiError> {
    let items = p
        .items
        .iter()
        .map(|i| {
            Ok(ContractItem {
                rarity: i.rarity.clone(),
                price: parse(&i.price)?,
                beneficiary: parse(&i.beneficiary)?,
                metadata: i.metadata.clone(),
            })
        })
        .collect::<Result<_, ApiError>>()?;
    let call = createCollectionCall {
        forwarder: parse(&p.forwarder)?,
        factory: parse(&p.factory)?,
        salt: parse(&p.salt)?,
        name: p.name.clone(),
        symbol: p.symbol.clone(),
        baseURI: p.base_uri.clone(),
        creator: parse(&p.creator)?,
        items,
    };
    Ok(hex::encode_prefixed(call.abi_encode()))
}

pub struct PublicationChain<'a> {
    pub http: &'a Outbound,
    pub url: &'a str,
}
impl PublicationChain<'_> {
    async fn rpc(&self, method: &str, params: Value) -> Result<Value, ApiError> {
        Rpc::new(self.http, self.url).call(method, params).await.map_err(|e| match e {
            RpcError::Refused(_) | RpcError::Transport(_) => {
                ApiError::service_unavailable("Polygon RPC is unavailable. Retry to check the same transaction.")
            }
            RpcError::Status(_) => ApiError::service_unavailable("Polygon RPC returned an error. Retry to check the same transaction."),
            RpcError::Malformed(_) => ApiError::service_unavailable("Polygon RPC returned an invalid response"),
            RpcError::Response { .. } => {
                ApiError::service_unavailable("Polygon RPC could not verify this publication. Retry to check the same transaction.")
            }
        })
    }

    /// The RPC knows neither `hash` nor its receipt, asked twice
    /// [`RECHECK`] apart, so one empty answer from a lagging or
    /// load-balanced node never counts as a transaction gone.
    async fn forgot(&self, hash: &str) -> Result<bool, ApiError> {
        for check in 0..2 {
            if check > 0 {
                tokio::time::sleep(RECHECK).await;
            }
            if !self
                .rpc("eth_getTransactionByHash", json!([hash]))
                .await?
                .is_null()
                || !self
                    .rpc("eth_getTransactionReceipt", json!([hash]))
                    .await?
                    .is_null()
            {
                return Ok(false);
            }
        }
        Ok(true)
    }

    pub async fn verify(
        &self,
        store: &ItemsComponent,
        publisher: &Publisher,
        hash: &str,
    ) -> Result<PublicationState, ApiError> {
        let owner = publisher.signer();
        let hash = format!("{:#x}", parse::<B256>(hash)?);
        let state = store.publication_state(publisher).await?.ok_or_else(|| {
            ApiError::not_found("Prepare this collection before submitting its transaction")
        })?;
        let replaced = state.tx_hash.as_deref().filter(|saved| *saved != hash);
        if replaced.is_some() && state.status != "submitted" {
            return Err(another_recorded());
        }
        if state.status == "published" || state.status == "reverted" {
            return Ok(state);
        }
        let network = &store.network;
        if quantity(&self.rpc("eth_chainId", json!([])).await?)? != U256::from(network.chain_id) {
            return Err(ApiError::service_unavailable(format!(
                "Publication verification requires the Polygon chain {}",
                network.chain_id
            )));
        }
        if let Some(saved) = replaced
            && !self.forgot(saved).await?
        {
            return Err(another_recorded());
        }
        let transaction = self.rpc("eth_getTransactionByHash", json!([hash])).await?;
        if transaction.is_null() {
            return Err(ApiError::service_unavailable(
                "The transaction is not visible on Polygon yet. Retry with the same hash.",
            ));
        }
        verify_transaction(network, &transaction, &state.preparation, &hash)?;
        store
            .record_publication_tx(publisher, &hash, replaced, &state.preparation.revision)
            .await?;
        let receipt = self.rpc("eth_getTransactionReceipt", json!([hash])).await?;
        if receipt.is_null() {
            return pending(store, publisher).await;
        }
        if !same(&receipt["transactionHash"], &hash)
            || !same(&receipt["from"], owner)
            || !same(&receipt["to"], network.collection_manager)
        {
            return Err(invalid(
                "Publication receipt does not match its transaction",
            ));
        }
        let block = quantity(&receipt["blockNumber"])?;
        let finalized = self
            .rpc("eth_getBlockByNumber", json!(["finalized", false]))
            .await?;
        if finalized.is_null() || quantity(&finalized["number"])? < block {
            return pending(store, publisher).await;
        }
        let canonical = self
            .rpc(
                "eth_getBlockByNumber",
                json!([receipt["blockNumber"], false]),
            )
            .await?;
        let block_hash = receipt["blockHash"]
            .as_str()
            .ok_or_else(|| invalid("Publication receipt has no block hash"))?;
        parse::<B256>(block_hash)?;
        if !same(&canonical["hash"], block_hash)
            || !same(&transaction["blockHash"], block_hash)
            || quantity(&transaction["blockNumber"])? != block
        {
            return Err(ApiError::service_unavailable(
                "Publication block changed. Retry to verify the canonical receipt.",
            ));
        }
        match quantity(&receipt["status"])? {
            status if status == U256::ZERO => {
                store.finish_publication(publisher, &hash, None).await
            }
            status if status == U256::from(1) => {
                let contract = deployed_collection(network, &receipt, &state.preparation.salt)?;
                let block_ref = receipt["blockNumber"].clone();
                let actual_creator = self.rpc("eth_call", json!([{"to":contract, "data":hex::encode_prefixed(creatorCall {}.abi_encode())}, block_ref])).await?;
                let bytes = hex::decode(actual_creator.as_str().unwrap_or_default())
                    .map_err(|_| invalid("Invalid collection creator response"))?;
                let actual_creator = creatorCall::abi_decode_returns(&bytes)
                    .map_err(|_| invalid("Invalid collection creator response"))?;
                if actual_creator != parse::<Address>(owner)? {
                    return Err(invalid("The deployed collection has another creator"));
                }
                let count = self.rpc("eth_call", json!([{"to":contract, "data":hex::encode_prefixed(itemsCountCall {}.abi_encode())}, block_ref])).await?;
                if quantity(&count)? != U256::from(state.preparation.items.len()) {
                    return Err(invalid(
                        "The deployed collection has a different item count",
                    ));
                }
                store
                    .finish_publication(publisher, &hash, Some(&contract))
                    .await
            }
            _ => Err(invalid("Publication receipt has an invalid status")),
        }
    }
}

fn another_recorded() -> ApiError {
    ApiError::http(
        409,
        "Another transaction is already recorded for this publication",
    )
}

async fn pending(
    store: &ItemsComponent,
    publisher: &Publisher,
) -> Result<PublicationState, ApiError> {
    store
        .publication_state(publisher)
        .await?
        .ok_or_else(|| ApiError::not_found("Publication not found"))
}

fn same(value: &Value, expected: &str) -> bool {
    value
        .as_str()
        .is_some_and(|v| v.eq_ignore_ascii_case(expected))
}
fn quantity(value: &Value) -> Result<U256, ApiError> {
    parse(
        value
            .as_str()
            .ok_or_else(|| invalid("Invalid Polygon RPC quantity"))?,
    )
}

fn verify_transaction(
    network: &Network,
    tx: &Value,
    p: &PublicationPreparation,
    hash: &str,
) -> Result<(), ApiError> {
    if !same(&tx["hash"], hash)
        || !same(&tx["from"], &p.creator)
        || !same(&tx["to"], network.collection_manager)
        || !same(&tx["input"], &publication_calldata(p)?)
        || quantity(&tx["value"])? != U256::ZERO
    {
        return Err(invalid(
            "Transaction does not publish the reviewed collection from its owner's wallet",
        ));
    }
    Ok(())
}

fn deployed_collection(network: &Network, receipt: &Value, salt: &str) -> Result<String, ApiError> {
    let signature = format!("{:#x}", keccak256("ProxyCreated(address,bytes32)"));
    let logs = receipt["logs"]
        .as_array()
        .ok_or_else(|| invalid("Publication receipt has no factory logs"))?;
    let matches: Vec<_> = logs
        .iter()
        .filter(|log| {
            same(&log["address"], network.collection_factory)
                && same(&log["topics"][0], &signature)
                && same(&log["data"], salt)
                && log["removed"] != true
        })
        .collect();
    if matches.len() != 1 {
        return Err(invalid(
            "Publication receipt must contain one matching collection factory event",
        ));
    }
    let topics = matches[0]["topics"]
        .as_array()
        .ok_or_else(|| invalid("Invalid factory event"))?;
    if topics.len() != 2 {
        return Err(invalid("Invalid factory event"));
    }
    let encoded = parse::<B256>(topics[1].as_str().unwrap_or_default())?;
    if encoded[..12].iter().any(|b| *b != 0) {
        return Err(invalid("Invalid collection address in factory event"));
    }
    let address = Address::from_slice(&encoded[12..]);
    if address == Address::ZERO {
        return Err(invalid("Collection factory returned the zero address"));
    }
    Ok(format!("{address:#x}"))
}
