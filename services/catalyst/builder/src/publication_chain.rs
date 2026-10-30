//! The receipt half of a standard publication: the client signs and submits
//! the collection-manager transaction from its own wallet and reports the
//! hash; this verifies over the Polygon JSON-RPC (`BUILDER_POLYGON_RPC_URL`,
//! through the ONE guarded client) that the transaction is the reviewed
//! calldata from the owner's wallet to the manager, records it, and once
//! the receipt is in a finalized canonical block reads the factory event,
//! the deployed collection's creator and item count, and finishes the
//! attempt as `published` or `reverted`. v2 never holds a signing key.

use alloy_primitives::{Address, B256, U256, hex, keccak256};
use alloy_sol_types::{SolCall, sol};
use serde_json::{Value, json};
use uuid::Uuid;

use dcl_http::Outbound;
use dcl_http::jsonrpc::{Rpc, RpcError};

use super::http::ApiError;
use super::items::ItemsComponent;
use super::publication::{COLLECTION_FACTORY, COLLECTION_MANAGER, PublicationPreparation};
use super::publication_store::PublicationState;

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

    pub async fn verify(
        &self,
        store: &ItemsComponent,
        owner: &str,
        id: Uuid,
        hash: &str,
    ) -> Result<PublicationState, ApiError> {
        let hash = format!("{:#x}", parse::<B256>(hash)?);
        let state = store.publication_state(owner, id).await?.ok_or_else(|| {
            ApiError::not_found("Prepare this collection before submitting its transaction")
        })?;
        if state.tx_hash.as_deref().is_some_and(|saved| saved != hash) {
            return Err(ApiError::http(
                409,
                "Another transaction is already recorded for this publication",
            ));
        }
        if state.status == "published" || state.status == "reverted" {
            return Ok(state);
        }
        if quantity(&self.rpc("eth_chainId", json!([])).await?)? != U256::from(137) {
            return Err(ApiError::service_unavailable(
                "Publication verification requires Polygon Mainnet",
            ));
        }
        let transaction = self.rpc("eth_getTransactionByHash", json!([hash])).await?;
        if transaction.is_null() {
            return Err(ApiError::service_unavailable(
                "The transaction is not visible on Polygon yet. Retry with the same hash.",
            ));
        }
        verify_transaction(&transaction, &state.preparation, &hash)?;
        store
            .record_publication_tx(owner, id, &hash, &state.preparation.revision)
            .await?;
        let receipt = self.rpc("eth_getTransactionReceipt", json!([hash])).await?;
        if receipt.is_null() {
            return pending(store, owner, id).await;
        }
        if !same(&receipt["transactionHash"], &hash)
            || !same(&receipt["from"], owner)
            || !same(&receipt["to"], COLLECTION_MANAGER)
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
            return pending(store, owner, id).await;
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
                store.finish_publication(owner, id, &hash, None).await
            }
            status if status == U256::from(1) => {
                let contract = deployed_collection(&receipt, &state.preparation.salt)?;
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
                    .finish_publication(owner, id, &hash, Some(&contract))
                    .await
            }
            _ => Err(invalid("Publication receipt has an invalid status")),
        }
    }
}

async fn pending(
    store: &ItemsComponent,
    owner: &str,
    id: Uuid,
) -> Result<PublicationState, ApiError> {
    store
        .publication_state(owner, id)
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

fn verify_transaction(tx: &Value, p: &PublicationPreparation, hash: &str) -> Result<(), ApiError> {
    if !same(&tx["hash"], hash)
        || !same(&tx["from"], &p.creator)
        || !same(&tx["to"], COLLECTION_MANAGER)
        || !same(&tx["input"], &publication_calldata(p)?)
        || quantity(&tx["value"])? != U256::ZERO
    {
        return Err(invalid(
            "Transaction does not publish the reviewed collection from its owner's wallet",
        ));
    }
    Ok(())
}

fn deployed_collection(receipt: &Value, salt: &str) -> Result<String, ApiError> {
    let signature = format!("{:#x}", keccak256("ProxyCreated(address,bytes32)"));
    let logs = receipt["logs"]
        .as_array()
        .ok_or_else(|| invalid("Publication receipt has no factory logs"))?;
    let matches: Vec<_> = logs
        .iter()
        .filter(|log| {
            same(&log["address"], COLLECTION_FACTORY)
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
