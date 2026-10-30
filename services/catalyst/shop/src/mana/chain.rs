// from: marketplace@f3956826:webapp/src/modules/{nft/estate/utils.ts,rental/utils.ts,order/utils.ts} (which reads a purchase makes before it is sent), catalyrst/crates/catalyrst-economy/src/ports/chain.rs (the eth_call/estimateGas/receipt round trips)
//! The chain reads the MANA quotes and the execution tracker make, over
//! the process's one JSON-RPC round trip (`economy::rpc::Rpc`, the guarded
//! client): ownership, balances and allowances, the estate fingerprint, the
//! registrar's NAME availability and price, the Rentals indexes, a gas
//! estimate, and receipts. A transport failure is [`ReadError::Unavailable`]
//! (the quote says the fact is unknown); a revert is what the contract
//! said and is read as such (`ownerOf` of a burned or unminted token).

use std::time::Duration;

use alloy_primitives::{Address, B256, Bytes, U256};
use alloy_sol_types::{SolCall, sol};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::evm::rpc::{Rpc, RpcError, decode_hex, parse_quantity_u64, parse_quantity_u128};
use crate::evm::signer::ReceiptLog;
use crate::http::Outbound;

sol! {
    function ownerOf(uint256 tokenId) external view returns (address);
    function balanceOf(address owner) external view returns (uint256);
    function allowance(address owner, address spender) external view returns (uint256);
    function isApprovedForAll(address owner, address operator) external view returns (bool);
    function getApproved(uint256 tokenId) external view returns (address);
    function getFingerprintV2(uint256 estateId) external view returns (bytes32);
    function getFingerprint(uint256 estateId) external view returns (bytes32);
    function PRICE() external view returns (uint256);
}

pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ReadError {
    #[error("the chain is unavailable: {0}")]
    Unavailable(String),
    #[error("the contract reverted: {0}")]
    Revert(String),
    #[error("the answer does not decode: {0}")]
    Decode(String),
}

impl From<RpcError> for ReadError {
    fn from(e: RpcError) -> Self {
        if e.is_execution_revert() {
            ReadError::Revert(e.message().to_string())
        } else {
            ReadError::Unavailable(e.to_string())
        }
    }
}

/// A mined transaction as the tracker reads it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Receipt {
    pub tx_hash: String,
    pub status: Option<bool>,
    pub block_number: u64,
    pub from: Address,
    pub to: Option<Address>,
    pub gas_used: Option<u128>,
    pub logs: Vec<ReceiptLog>,
}

#[derive(Debug, Deserialize)]
struct WireLog {
    address: Address,
    #[serde(default)]
    topics: Vec<B256>,
    #[serde(default)]
    data: Bytes,
}

#[derive(Debug, Deserialize)]
struct WireReceipt {
    #[serde(rename = "transactionHash")]
    transaction_hash: String,
    #[serde(default)]
    status: Option<String>,
    #[serde(rename = "blockNumber")]
    block_number: String,
    from: String,
    #[serde(default)]
    to: Option<String>,
    #[serde(rename = "gasUsed", default)]
    gas_used: Option<String>,
    #[serde(default)]
    logs: Vec<WireLog>,
}

/// One chain's reader.
#[derive(Clone)]
pub struct Reader {
    rpc: Rpc,
}

impl std::fmt::Debug for Reader {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Reader").finish_non_exhaustive()
    }
}

fn hex_bytes(data: &[u8]) -> String {
    format!("0x{}", hex::encode(data))
}

impl Reader {
    pub fn new(http: Outbound, url: &str) -> Self {
        Self {
            rpc: Rpc::new(http, url, DEFAULT_TIMEOUT),
        }
    }

    pub fn rpc(&self) -> &Rpc {
        &self.rpc
    }

    /// `eth_call` of `data` at `to`, latest block.
    pub async fn call(&self, to: Address, data: &[u8]) -> Result<Vec<u8>, ReadError> {
        let result = self
            .rpc
            .call_hex(
                "eth_call",
                json!([{ "to": format!("{to:#x}"), "data": hex_bytes(data) }, "latest"]),
            )
            .await?;
        decode_hex(&result)
            .ok_or_else(|| ReadError::Decode(format!("eth_call answered non-hex {result}")))
    }

    async fn call_typed<C: SolCall>(&self, to: Address, call: C) -> Result<C::Return, ReadError> {
        let out = self.call(to, &call.abi_encode()).await?;
        C::abi_decode_returns(&out).map_err(|e| ReadError::Decode(format!("{}: {e}", C::SIGNATURE)))
    }

    /// `ownerOf(token)`: `None` when the contract reverts (no such token).
    pub async fn owner_of(
        &self,
        contract: Address,
        token_id: U256,
    ) -> Result<Option<Address>, ReadError> {
        match self
            .call_typed(contract, ownerOfCall { tokenId: token_id })
            .await
        {
            Ok(owner) => Ok((owner != Address::ZERO).then_some(owner)),
            Err(ReadError::Revert(_)) => Ok(None),
            Err(e) => Err(e),
        }
    }

    pub async fn erc20_balance(&self, token: Address, owner: Address) -> Result<U256, ReadError> {
        self.call_typed(token, balanceOfCall { owner }).await
    }

    pub async fn erc20_allowance(
        &self,
        token: Address,
        owner: Address,
        spender: Address,
    ) -> Result<U256, ReadError> {
        self.call_typed(token, allowanceCall { owner, spender })
            .await
    }

    pub async fn is_approved_for_all(
        &self,
        contract: Address,
        owner: Address,
        operator: Address,
    ) -> Result<bool, ReadError> {
        self.call_typed(contract, isApprovedForAllCall { owner, operator })
            .await
    }

    pub async fn get_approved(
        &self,
        contract: Address,
        token_id: U256,
    ) -> Result<Address, ReadError> {
        self.call_typed(contract, getApprovedCall { tokenId: token_id })
            .await
    }

    /// The Estate registry's `getFingerprintV2`, falling back to
    /// `getFingerprint` on a registry without the V2 read.
    pub async fn estate_fingerprint(
        &self,
        estate: Address,
        estate_id: U256,
    ) -> Result<B256, ReadError> {
        match self
            .call_typed(
                estate,
                getFingerprintV2Call {
                    estateId: estate_id,
                },
            )
            .await
        {
            Ok(fp) => Ok(fp),
            Err(ReadError::Revert(_)) => {
                self.call_typed(
                    estate,
                    getFingerprintCall {
                        estateId: estate_id,
                    },
                )
                .await
            }
            Err(e) => Err(e),
        }
    }

    /// `DCLControllerV2.PRICE()`.
    pub async fn name_price(&self, controller: Address) -> Result<U256, ReadError> {
        self.call_typed(controller, PRICECall {}).await
    }

    pub async fn native_balance(&self, owner: Address) -> Result<U256, ReadError> {
        let hex = self
            .rpc
            .call_hex("eth_getBalance", json!([format!("{owner:#x}"), "latest"]))
            .await?;
        U256::from_str_radix(hex.trim_start_matches("0x"), 16)
            .map_err(|e| ReadError::Decode(format!("eth_getBalance: {e}")))
    }

    pub async fn gas_price(&self) -> Result<u128, ReadError> {
        Ok(parse_quantity_u128(
            &self.rpc.call_hex("eth_gasPrice", json!([])).await?,
        )?)
    }

    pub async fn head_block(&self) -> Result<u64, ReadError> {
        Ok(parse_quantity_u64(
            &self.rpc.call_hex("eth_blockNumber", json!([])).await?,
        )?)
    }

    pub async fn chain_id(&self) -> Result<u64, ReadError> {
        Ok(parse_quantity_u64(
            &self.rpc.call_hex("eth_chainId", json!([])).await?,
        )?)
    }

    /// `eth_estimateGas` from `from`; a revert is the call failing as sent
    /// (an allowance or a balance short), returned as such.
    pub async fn estimate_gas(
        &self,
        from: Address,
        to: Address,
        data: &[u8],
    ) -> Result<u64, ReadError> {
        let hex = self.rpc.call_hex("eth_estimateGas", json!([{ "from": format!("{from:#x}"), "to": format!("{to:#x}"), "data": hex_bytes(data) }])).await?;
        Ok(parse_quantity_u64(&hex)?)
    }

    /// The receipt of `tx_hash`, `None` while unmined.
    pub async fn receipt(&self, tx_hash: &str) -> Result<Option<Receipt>, ReadError> {
        let value = self
            .rpc
            .call("eth_getTransactionReceipt", json!([tx_hash]))
            .await?;
        if value.is_null() {
            return Ok(None);
        }
        let r: WireReceipt = serde_json::from_value(value)
            .map_err(|e| ReadError::Decode(format!("receipt: {e}")))?;
        let status = match r.status.as_deref() {
            Some(s) => Some(parse_quantity_u64(s)? == 1),
            None => None,
        };
        Ok(Some(Receipt {
            tx_hash: r.transaction_hash.to_ascii_lowercase(),
            status,
            block_number: parse_quantity_u64(&r.block_number)?,
            from: r
                .from
                .parse()
                .map_err(|_| ReadError::Decode("receipt from".into()))?,
            to: r.to.as_deref().and_then(|t| t.parse().ok()),
            gas_used: match r.gas_used.as_deref() {
                Some(g) => Some(parse_quantity_u128(g)?),
                None => None,
            },
            logs: r
                .logs
                .into_iter()
                .map(|l| ReceiptLog {
                    address: l.address,
                    topics: l.topics,
                    data: l.data,
                })
                .collect(),
        }))
    }

    /// A raw JSON-RPC call (the tests drive anvil with it).
    pub async fn raw(&self, method: &str, params: Value) -> Result<Value, ReadError> {
        Ok(self.rpc.call(method, params).await?)
    }
}

/// Calldata as the wire carries it.
pub fn calldata_hex(data: &Bytes) -> String {
    hex_bytes(data)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_revert_is_the_contracts_answer_a_transport_failure_is_not() {
        assert!(matches!(
            ReadError::from(RpcError::Response {
                code: 3,
                message: "execution reverted".into()
            }),
            ReadError::Revert(_)
        ));
        assert!(matches!(
            ReadError::from(RpcError::Transport("boom".into())),
            ReadError::Unavailable(_)
        ));
        assert!(matches!(
            ReadError::from(RpcError::Response {
                code: 429,
                message: "rate limited".into()
            }),
            ReadError::Unavailable(_)
        ));
    }

    #[test]
    fn the_selectors_are_the_standard_ones() {
        assert_eq!(ownerOfCall::SELECTOR, [0x63, 0x52, 0x21, 0x1e]);
        assert_eq!(balanceOfCall::SELECTOR, [0x70, 0xa0, 0x82, 0x31]);
        assert_eq!(allowanceCall::SELECTOR, [0xdd, 0x62, 0xed, 0x3e]);
        assert_eq!(
            format!(
                "{:#x}",
                alloy_primitives::keccak256("getFingerprintV2(uint256)")
            )[..10],
            format!("0x{}", hex::encode(getFingerprintV2Call::SELECTOR))
        );
    }

    #[test]
    fn a_receipt_decodes_its_status_block_and_logs() {
        let value = json!({
            "transactionHash": "0xAB", "status": "0x1", "blockNumber": "0x10", "from": "0x00000000000000000000000000000000000000a1",
            "to": "0x00000000000000000000000000000000000000b2", "gasUsed": "0x5208",
            "logs": [{ "address": "0x00000000000000000000000000000000000000cc", "topics": [format!("0x{:0>64}", "01")], "data": "0x" }]
        });
        let r: WireReceipt = serde_json::from_value(value).unwrap();
        assert_eq!(
            (r.status.as_deref(), r.logs.len(), r.gas_used.as_deref()),
            (Some("0x1"), 1, Some("0x5208"))
        );
    }
}
