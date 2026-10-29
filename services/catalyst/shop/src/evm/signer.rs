//! The direct JSON-RPC signer: the relayer key signs a transaction locally
//! (EIP-1559 when the chain reports a base fee, EIP-155 legacy otherwise)
//! and the raw bytes go out through the shared guarded client as
//! `eth_sendRawTransaction`; receipts come back the same way. The old crate
//! let alloy's provider own a second HTTP client and its fee filler; v2 keeps
//! the same sizing rule (gas estimate under `MAX_GAS_LIMIT`, `2 * base + tip`)
//! over the ONE client. The key is parsed once and never formatted.

use std::str::FromStr;
use std::sync::Arc;
use std::time::{Duration, Instant};

use alloy_consensus::{SignableTransaction, TxEip1559, TxLegacy};
use alloy_eips::eip2718::Encodable2718;
use alloy_primitives::{Address, B256, Bytes, TxHash, TxKind, U256};
use alloy_signer::SignerSync;
use alloy_signer_local::PrivateKeySigner;
use serde::Deserialize;
use serde_json::json;

use super::chain::{ChainComponent, ChainError};
use super::errors::ApiError;
use super::rpc::{RpcError, decode_hex, parse_quantity_u64, parse_quantity_u128};
use super::transaction::TransactionData;

fn within_gas_cap(estimate: u64, cap: u64) -> bool {
    cap == 0 || estimate <= cap
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReceiptOutcome {
    Confirmed,
    Reverted,
    Pending,
}

pub fn classify_receipt(status: Option<bool>) -> ReceiptOutcome {
    match status {
        Some(true) => ReceiptOutcome::Confirmed,
        Some(false) => ReceiptOutcome::Reverted,
        None => ReceiptOutcome::Pending,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReceiptLog {
    pub address: Address,
    pub topics: Vec<B256>,
    pub data: Bytes,
}

#[derive(Deserialize)]
struct WireReceipt {
    status: Option<String>,
    #[serde(default)]
    logs: Vec<WireLog>,
}

#[derive(Deserialize)]
struct WireLog {
    address: Address,
    #[serde(default)]
    topics: Vec<B256>,
    #[serde(default)]
    data: Bytes,
}

#[derive(Deserialize)]
struct WireBlockFees {
    #[serde(rename = "baseFeePerGas")]
    base_fee_per_gas: Option<String>,
}

enum Fees {
    Eip1559 {
        max_fee_per_gas: u128,
        max_priority_fee_per_gas: u128,
    },
    Legacy {
        gas_price: u128,
    },
}

pub struct DirectSigner {
    chain: Arc<ChainComponent>,
    signer: PrivateKeySigner,
    chain_id: u64,
    relayer_address: Address,
    max_gas_limit: u64,
    receipt_poll_interval: Duration,
    receipt_timeout: Duration,
    send_lock: tokio::sync::Mutex<()>,
}

impl std::fmt::Debug for DirectSigner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DirectSigner")
            .field("chain_id", &self.chain_id)
            .field("relayer_address", &self.relayer_address)
            .finish_non_exhaustive()
    }
}

impl DirectSigner {
    /// `key` is the hex private key (with or without `0x`); the error names
    /// the variable and never repeats the value.
    pub fn new(
        chain: Arc<ChainComponent>,
        key: &str,
        chain_id: u64,
        max_gas_limit: u64,
        receipt_poll_interval: Duration,
        receipt_timeout: Duration,
    ) -> Result<Self, String> {
        let signer = PrivateKeySigner::from_str(key.trim().trim_start_matches("0x"))
            .map_err(|_| "invalid RELAYER_PRIVATE_KEY: not a 32-byte secp256k1 key".to_string())?;
        let relayer_address = signer.address();
        Ok(Self {
            chain,
            signer,
            chain_id,
            relayer_address,
            max_gas_limit,
            receipt_poll_interval: receipt_poll_interval.max(Duration::from_millis(1)),
            receipt_timeout,
            send_lock: tokio::sync::Mutex::new(()),
        })
    }

    pub fn relayer_address(&self) -> Address {
        self.relayer_address
    }

    pub fn chain_id(&self) -> u64 {
        self.chain_id
    }

    pub fn chain(&self) -> &ChainComponent {
        &self.chain
    }

    pub async fn eth_call(&self, to: Address, data: Bytes) -> Result<Bytes, ApiError> {
        self.chain
            .eth_call(to, &data)
            .await
            .map(Bytes::from)
            .map_err(|e| ApiError::RelayerFailed(format!("eth_call failed: {e}")))
    }

    pub async fn await_receipt(&self, tx_hash_hex: &str) -> Result<ReceiptOutcome, ApiError> {
        let (outcome, _logs) = self.await_receipt_detailed(tx_hash_hex).await?;
        Ok(outcome)
    }

    pub async fn await_receipt_detailed(
        &self,
        tx_hash_hex: &str,
    ) -> Result<(ReceiptOutcome, Vec<ReceiptLog>), ApiError> {
        let hash = parse_tx_hash(tx_hash_hex)?;
        let deadline = Instant::now() + self.receipt_timeout;
        loop {
            match self.receipt(hash).await {
                Ok(Some((outcome, logs))) => {
                    tracing::info!(tx_hash = %tx_hash_hex, ?outcome, "broker tx receipt observed");
                    return Ok((outcome, logs));
                }
                Ok(None) => {}
                Err(e) => {
                    tracing::warn!(tx_hash = %tx_hash_hex, error = %e, "receipt poll RPC error; retrying")
                }
            }
            if Instant::now() >= deadline {
                tracing::warn!(
                    tx_hash = %tx_hash_hex,
                    timeout_ms = self.receipt_timeout.as_millis() as u64,
                    "broker tx not mined within receipt timeout; leaving for reconciler"
                );
                return Ok((ReceiptOutcome::Pending, Vec::new()));
            }
            tokio::time::sleep(self.receipt_poll_interval).await;
        }
    }

    pub async fn fetch_receipt_once(
        &self,
        tx_hash_hex: &str,
    ) -> Result<(ReceiptOutcome, Vec<ReceiptLog>), ApiError> {
        let hash = parse_tx_hash(tx_hash_hex)?;
        match self.receipt(hash).await {
            Ok(Some(found)) => Ok(found),
            Ok(None) => Ok((ReceiptOutcome::Pending, Vec::new())),
            Err(e) => Err(ApiError::RelayerFailed(format!(
                "receipt lookup failed for {tx_hash_hex}: {e}"
            ))),
        }
    }

    async fn receipt(
        &self,
        hash: TxHash,
    ) -> Result<Option<(ReceiptOutcome, Vec<ReceiptLog>)>, RpcError> {
        let value = self
            .chain
            .rpc()
            .call("eth_getTransactionReceipt", json!([format!("{hash:#x}")]))
            .await?;
        if value.is_null() {
            return Ok(None);
        }
        let receipt: WireReceipt = serde_json::from_value(value)
            .map_err(|e| RpcError::Transport(format!("malformed receipt: {e}")))?;
        let status = match receipt.status.as_deref() {
            Some(s) => Some(parse_quantity_u64(s)? == 1),
            None => None,
        };
        let logs = receipt
            .logs
            .into_iter()
            .map(|l| ReceiptLog {
                address: l.address,
                topics: l.topics,
                data: l.data,
            })
            .collect();
        Ok(Some((classify_receipt(status), logs)))
    }

    pub async fn send_meta_transaction(&self, tx: &TransactionData) -> Result<String, ApiError> {
        let to = Address::from_str(tx.params[0].trim())
            .map_err(|e| ApiError::InvalidTransaction(format!("invalid `to` address: {e}")))?;
        let data = Bytes::from_str(tx.params[1].trim())
            .map_err(|e| ApiError::InvalidTransaction(format!("invalid call data: {e}")))?;
        self.broadcast(to, data, "broadcast meta-transaction via direct JSON-RPC")
            .await
    }

    pub async fn send_direct_call(&self, to: Address, data: Bytes) -> Result<String, ApiError> {
        self.broadcast(to, data, "broadcast direct contract call via JSON-RPC")
            .await
    }

    async fn broadcast(
        &self,
        to: Address,
        data: Bytes,
        log_msg: &'static str,
    ) -> Result<String, ApiError> {
        let _guard = self.send_lock.lock().await;

        let estimate = self
            .chain
            .estimate_gas(Some(self.relayer_address), to, &data)
            .await
            .map_err(|e| match e {
                ChainError::Revert(msg) => {
                    ApiError::RelayReverted(format!("gas estimation reverted: {msg}"))
                }
                other => ApiError::RelayerFailed(format!("eth_estimateGas failed: {other}")),
            })?;
        if !within_gas_cap(estimate, self.max_gas_limit) {
            return Err(ApiError::InvalidTransaction(format!(
                "estimated gas {estimate} exceeds the relayer cap {} (raise MAX_GAS_LIMIT if intentional)",
                self.max_gas_limit
            )));
        }

        let rpc = self.chain.rpc();
        let nonce_hex = rpc
            .call_hex(
                "eth_getTransactionCount",
                json!([format!("{:#x}", self.relayer_address), "pending"]),
            )
            .await
            .map_err(|e| ApiError::RelayerFailed(format!("eth_getTransactionCount failed: {e}")))?;
        let nonce =
            parse_quantity_u64(&nonce_hex).map_err(|e| ApiError::RelayerFailed(e.to_string()))?;
        let fees = self
            .fees()
            .await
            .map_err(|e| ApiError::RelayerFailed(format!("fee lookup failed: {e}")))?;

        let raw = self.sign(nonce, estimate, fees, to, data)?;
        let hash = rpc
            .call_hex(
                "eth_sendRawTransaction",
                json!([format!("0x{}", hex::encode(&raw))]),
            )
            .await
            .map_err(|e| {
                if e.is_execution_revert() {
                    ApiError::RelayReverted(format!("transaction reverted: {}", e.message()))
                } else {
                    ApiError::RelayerFailed(format!("eth_sendRawTransaction failed: {e}"))
                }
            })?;
        let tx_hash = parse_tx_hash(&hash)
            .map_err(|_| ApiError::RelayerFailed(format!("node answered a non-hash {hash:?}")))?;
        tracing::info!(tx_hash = %tx_hash, relayer = %self.relayer_address, "{log_msg}");
        Ok(format!("{tx_hash:#x}"))
    }

    fn sign(
        &self,
        nonce: u64,
        gas_limit: u64,
        fees: Fees,
        to: Address,
        data: Bytes,
    ) -> Result<Vec<u8>, ApiError> {
        let sign_err =
            |e: alloy_signer::Error| ApiError::RelayerFailed(format!("signing failed: {e}"));
        Ok(match fees {
            Fees::Eip1559 {
                max_fee_per_gas,
                max_priority_fee_per_gas,
            } => {
                let tx = TxEip1559 {
                    chain_id: self.chain_id,
                    nonce,
                    gas_limit,
                    max_fee_per_gas,
                    max_priority_fee_per_gas,
                    to: TxKind::Call(to),
                    value: U256::ZERO,
                    access_list: Default::default(),
                    input: data,
                };
                let sig = self
                    .signer
                    .sign_hash_sync(&tx.signature_hash())
                    .map_err(sign_err)?;
                tx.into_signed(sig).encoded_2718()
            }
            Fees::Legacy { gas_price } => {
                let tx = TxLegacy {
                    chain_id: Some(self.chain_id),
                    nonce,
                    gas_price,
                    gas_limit,
                    to: TxKind::Call(to),
                    value: U256::ZERO,
                    input: data,
                };
                let sig = self
                    .signer
                    .sign_hash_sync(&tx.signature_hash())
                    .map_err(sign_err)?;
                tx.into_signed(sig).encoded_2718()
            }
        })
    }

    /// alloy's default filler: `max_fee = 2 * base + tip`; a chain without a
    /// base fee gets a legacy transaction at `eth_gasPrice`.
    async fn fees(&self) -> Result<Fees, RpcError> {
        let rpc = self.chain.rpc();
        let block = rpc
            .call("eth_getBlockByNumber", json!(["latest", false]))
            .await?;
        let base = match serde_json::from_value::<Option<WireBlockFees>>(block) {
            Ok(Some(WireBlockFees {
                base_fee_per_gas: Some(hex_fee),
            })) => Some(parse_quantity_u128(&hex_fee)?),
            Ok(_) => None,
            Err(e) => return Err(RpcError::Transport(format!("malformed latest block: {e}"))),
        };
        let Some(base) = base else {
            let gas_price = parse_quantity_u128(&rpc.call_hex("eth_gasPrice", json!([])).await?)?;
            return Ok(Fees::Legacy { gas_price });
        };
        let tip = match rpc.call_hex("eth_maxPriorityFeePerGas", json!([])).await {
            Ok(hex_tip) => parse_quantity_u128(&hex_tip)?,
            Err(_) => parse_quantity_u128(&rpc.call_hex("eth_gasPrice", json!([])).await?)?
                .saturating_sub(base),
        };
        Ok(Fees::Eip1559 {
            max_fee_per_gas: base.saturating_mul(2).saturating_add(tip),
            max_priority_fee_per_gas: tip,
        })
    }
}

fn parse_tx_hash(tx_hash_hex: &str) -> Result<TxHash, ApiError> {
    let bytes = decode_hex(tx_hash_hex.trim())
        .filter(|b| b.len() == 32)
        .ok_or_else(|| {
            ApiError::Internal(format!("invalid tx hash {tx_hash_hex:?} for receipt poll"))
        })?;
    Ok(TxHash::from_slice(&bytes))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn receipt_status_maps_to_outcome() {
        assert_eq!(classify_receipt(Some(true)), ReceiptOutcome::Confirmed);
        assert_eq!(classify_receipt(Some(false)), ReceiptOutcome::Reverted);
        assert_eq!(classify_receipt(None), ReceiptOutcome::Pending);
    }

    #[test]
    fn gas_cap_enforced() {
        assert!(within_gas_cap(250_000, 1_500_000));
        assert!(within_gas_cap(1_500_000, 1_500_000));
        assert!(!within_gas_cap(1_500_001, 1_500_000));
        assert!(!within_gas_cap(30_000_000, 1_500_000));
        assert!(within_gas_cap(30_000_000, 0));
    }

    #[test]
    fn tx_hashes_are_32_bytes() {
        assert!(parse_tx_hash(&format!("0x{}", "ab".repeat(32))).is_ok());
        assert!(parse_tx_hash("0x1234").is_err());
        assert!(parse_tx_hash("nope").is_err());
    }

    #[test]
    fn a_bad_key_is_refused_without_echo() {
        let chain = Arc::new(ChainComponent::new(
            crate::http::Outbound::for_tests(),
            "http://127.0.0.1:1/",
            Duration::from_secs(1),
        ));
        let err = DirectSigner::new(
            chain,
            "0xnot-a-key-SECRET",
            137,
            1,
            Duration::from_millis(1),
            Duration::from_millis(1),
        )
        .unwrap_err();
        assert!(!err.contains("SECRET"));
        assert!(err.contains("RELAYER_PRIVATE_KEY"));
    }

    #[test]
    fn debug_shows_the_address_not_the_key() {
        let key = alloy::signers::local::PrivateKeySigner::random();
        let key_hex = hex::encode(key.to_bytes());
        let chain = Arc::new(ChainComponent::new(
            crate::http::Outbound::for_tests(),
            "http://127.0.0.1:1/",
            Duration::from_secs(1),
        ));
        let signer = DirectSigner::new(
            chain,
            &key_hex,
            137,
            1,
            Duration::from_millis(1),
            Duration::from_millis(1),
        )
        .unwrap();
        let shown = format!("{signer:?}");
        assert!(shown.contains(&format!("{:#x}", key.address())));
        assert!(!shown.contains(&key_hex));
        assert_eq!(signer.relayer_address(), key.address());
    }
}
