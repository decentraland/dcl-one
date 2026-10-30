//! The chain reads the relay policy needs: the meta-transaction nonce and
//! domain separator of a target, the node's chain id, code presence, the
//! proxy anchor a collection delegates to, the caller's simulation
//! (`eth_estimateGas` from the user) and the gas price.
//! A revert describes the target; a transport failure describes the node and
//! is never read as a property of the contract.

use std::time::Duration;

use alloy_primitives::{Address, B256, U256, b256, keccak256};
use alloy_sol_types::{SolCall, sol};
use serde_json::json;

use super::abi::getNonceCall;
use super::rpc::{Rpc, RpcError, decode_hex, parse_quantity_u64, parse_quantity_u128};
use crate::http::Outbound;
use crate::shared::cache::TtlMap;

sol! {
    function domainSeparator() external view returns (bytes32);
    function getDomainSeperator() external view returns (bytes32);
}

pub const DEFAULT_RPC_TIMEOUT: Duration = Duration::from_secs(10);

/// The EIP-1167 minimal proxy runtime around the 20 inlined bytes of its
/// implementation: code that does nothing but delegate to that address.
const MINIMAL_PROXY_PREFIX: [u8; 10] = [0x36, 0x3d, 0x3d, 0x37, 0x3d, 0x3d, 0x3d, 0x36, 0x3d, 0x73];
const MINIMAL_PROXY_SUFFIX: [u8; 15] = [
    0x5a, 0xf4, 0x3d, 0x82, 0x80, 0x3e, 0x90, 0x3d, 0x91, 0x60, 0x2b, 0x57, 0xfd, 0x5b, 0xf3,
];

/// keccak256 of the beacon proxy runtime the current collection factory
/// deploys. Its anchor lives in storage, and a storage word proves nothing on
/// its own (any contract can write any slot in its constructor), so the code
/// is recognised first and the slot read second. The runtime carries no
/// chain-specific bytes: one hash covers every network.
pub const BEACON_PROXY_RUNTIME_HASH: B256 =
    b256!("0xb012165a0b4b9acfff6b0a792a94024f0f9c0e245de7d585ac53d8a6ec5cc3c0");

/// `keccak256("eip1967.proxy.beacon") - 1`, the slot that runtime reads its
/// beacon from. The plain implementation slot is not consulted: no collection
/// is that shape, and every slot read outside recognised code is one more an
/// address can fill in to describe itself.
pub const BEACON_SLOT: B256 =
    b256!("0xa3f0ad74e5423aebfd80d3ef4346578335a9a72aeaee59ff6cb3582b35133d50");

/// What an address's runtime is, as far as collection provenance goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProxyRuntime {
    /// An EIP-1167 minimal proxy over the inlined implementation.
    Minimal(Address),
    /// The recorded beacon proxy runtime; its beacon is in [`BEACON_SLOT`].
    Beacon,
    /// No code, or code of any other shape, however its storage is arranged.
    Unrecognised,
}

pub fn classify_proxy_runtime(code: &[u8]) -> ProxyRuntime {
    if code.len() == MINIMAL_PROXY_PREFIX.len() + 20 + MINIMAL_PROXY_SUFFIX.len()
        && code.starts_with(&MINIMAL_PROXY_PREFIX)
        && code.ends_with(&MINIMAL_PROXY_SUFFIX)
    {
        return ProxyRuntime::Minimal(Address::from_slice(
            &code[MINIMAL_PROXY_PREFIX.len()..MINIMAL_PROXY_PREFIX.len() + 20],
        ));
    }
    if !code.is_empty() && keccak256(code) == BEACON_PROXY_RUNTIME_HASH {
        return ProxyRuntime::Beacon;
    }
    ProxyRuntime::Unrecognised
}

/// The address a proxy's slot holds: `None` when the slot is unset, or when
/// the word carries anything above its low 20 bytes (a real proxy's slot is
/// a left-padded address, so such a word never was one).
pub fn address_in_slot(word: &[u8]) -> Option<Address> {
    if word.len() != 32 || word[..12].iter().any(|b| *b != 0) {
        return None;
    }
    let address = Address::from_slice(&word[12..]);
    (address != Address::ZERO).then_some(address)
}

/// A contract's domain separator and the node's chain id never change, so
/// they are memoized for a day (bounded only so a redeploy cannot serve them
/// forever).
const IMMUTABLE_TTL: Duration = Duration::from_secs(24 * 60 * 60);
const DOMAIN_SEPARATOR_CACHE_CAP: usize = 4096;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChainError {
    Transport(String),
    /// The target reverted (an `eth_estimateGas`/`eth_call` refusal); the
    /// message is the node's, for the client.
    Revert(String),
}

impl std::fmt::Display for ChainError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ChainError::Transport(m) => write!(f, "rpc unavailable: {m}"),
            ChainError::Revert(m) => write!(f, "{m}"),
        }
    }
}

impl From<RpcError> for ChainError {
    fn from(e: RpcError) -> Self {
        if e.is_execution_revert() {
            ChainError::Revert(e.message().to_string())
        } else {
            ChainError::Transport(e.to_string())
        }
    }
}

pub struct ChainComponent {
    rpc: Rpc,
    domain_separators: TtlMap<Address, Option<B256>>,
    chain_id: TtlMap<(), u64>,
}

impl ChainComponent {
    pub fn new(http: Outbound, rpc_url: &str, timeout: Duration) -> Self {
        Self {
            rpc: Rpc::new(http, rpc_url, timeout),
            domain_separators: TtlMap::bounded(IMMUTABLE_TTL, DOMAIN_SEPARATOR_CACHE_CAP),
            chain_id: TtlMap::new(IMMUTABLE_TTL),
        }
    }

    pub fn rpc(&self) -> &Rpc {
        &self.rpc
    }

    /// An `eth_call`, distinguishing a target that reverted (`Ok(None)`)
    /// from an RPC that could not answer (`Err`).
    pub async fn call_view(
        &self,
        to: Address,
        data: Vec<u8>,
    ) -> Result<Option<Vec<u8>>, ChainError> {
        match self.eth_call(to, &data).await {
            Ok(bytes) => Ok(Some(bytes)),
            Err(ChainError::Revert(_)) => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// A raw `eth_call` at `latest`; a revert is an error of its own kind.
    pub async fn eth_call(&self, to: Address, data: &[u8]) -> Result<Vec<u8>, ChainError> {
        let params = json!([{ "to": format!("{to:#x}"), "data": format!("0x{}", hex::encode(data)) }, "latest"]);
        let hex_out = self.rpc.call_hex("eth_call", params).await?;
        decode_hex(&hex_out)
            .ok_or_else(|| ChainError::Transport(format!("eth_call returned non-hex: {hex_out}")))
    }

    pub async fn get_meta_transaction_nonce(
        &self,
        contract: Address,
        user: Address,
    ) -> Result<Option<U256>, ChainError> {
        let Some(result) = self
            .call_view(contract, getNonceCall { user }.abi_encode())
            .await?
        else {
            return Ok(None);
        };
        Ok(getNonceCall::abi_decode_returns(&result).ok())
    }

    pub async fn get_domain_separator(
        &self,
        contract: Address,
    ) -> Result<Option<B256>, ChainError> {
        self.domain_separators
            .get_or_fetch(contract, || self.fetch_domain_separator(contract))
            .await
    }

    async fn fetch_domain_separator(&self, contract: Address) -> Result<Option<B256>, ChainError> {
        if let Some(result) = self
            .call_view(contract, domainSeparatorCall {}.abi_encode())
            .await?
            && let Ok(sep) = domainSeparatorCall::abi_decode_returns(&result)
        {
            return Ok(Some(sep));
        }
        if let Some(result) = self
            .call_view(contract, getDomainSeperatorCall {}.abi_encode())
            .await?
            && let Ok(sep) = getDomainSeperatorCall::abi_decode_returns(&result)
        {
            return Ok(Some(sep));
        }
        Ok(None)
    }

    pub async fn get_chain_id(&self) -> Result<u64, ChainError> {
        self.chain_id
            .get_or_fetch((), || async {
                let hex_id = self.rpc.call_hex("eth_chainId", json!([])).await?;
                Ok(parse_quantity_u64(&hex_id)?)
            })
            .await
    }

    pub async fn get_code(&self, address: Address) -> Result<Vec<u8>, ChainError> {
        let hex_code = self
            .rpc
            .call_hex("eth_getCode", json!([format!("{address:#x}"), "latest"]))
            .await?;
        decode_hex(&hex_code)
            .ok_or_else(|| ChainError::Transport(format!("eth_getCode non-hex: {hex_code}")))
    }

    pub async fn is_contract(&self, address: Address) -> Result<bool, ChainError> {
        Ok(!self.get_code(address).await?.is_empty())
    }

    pub async fn get_storage_at(
        &self,
        address: Address,
        slot: B256,
    ) -> Result<Vec<u8>, ChainError> {
        let hex_word = self
            .rpc
            .call_hex(
                "eth_getStorageAt",
                json!([format!("{address:#x}"), format!("{slot:#x}"), "latest"]),
            )
            .await?;
        decode_hex(&hex_word)
            .ok_or_else(|| ChainError::Transport(format!("eth_getStorageAt non-hex: {hex_word}")))
    }

    /// The address that decides what code a collection proxy runs, for the
    /// two shapes collections are deployed over, both established from the
    /// bytecode: the implementation a minimal proxy inlines, or the beacon of
    /// a proxy whose runtime is the recorded one. `None` for anything else,
    /// whatever its slots say; a transport failure is an error, never a
    /// verdict about the address.
    pub async fn get_proxy_anchor(&self, address: Address) -> Result<Option<Address>, ChainError> {
        match classify_proxy_runtime(&self.get_code(address).await?) {
            ProxyRuntime::Minimal(implementation) => Ok(Some(implementation)),
            ProxyRuntime::Beacon => Ok(address_in_slot(
                &self.get_storage_at(address, BEACON_SLOT).await?,
            )),
            ProxyRuntime::Unrecognised => Ok(None),
        }
    }

    /// `eth_estimateGas` as `from` would send it: the relay policy's
    /// simulation of the user's meta-transaction and the signer's gas sizing.
    pub async fn estimate_gas(
        &self,
        from: Option<Address>,
        to: Address,
        data: &[u8],
    ) -> Result<u64, ChainError> {
        let mut call = json!({ "to": format!("{to:#x}"), "data": format!("0x{}", hex::encode(data)), "value": "0x0" });
        if let Some(from) = from {
            call["from"] = json!(format!("{from:#x}"));
        }
        let hex_gas = self.rpc.call_hex("eth_estimateGas", json!([call])).await?;
        Ok(parse_quantity_u64(&hex_gas)?)
    }

    pub async fn gas_price(&self) -> Result<u128, ChainError> {
        let hex_price = self.rpc.call_hex("eth_gasPrice", json!([])).await?;
        Ok(parse_quantity_u128(&hex_price)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    async fn spawn_counting_rpc() -> (String, Arc<AtomicUsize>) {
        let calls = Arc::new(AtomicUsize::new(0));
        let counter = calls.clone();
        let app = axum::Router::new().route(
            "/",
            axum::routing::post(move |axum::Json(req): axum::Json<Value>| {
                let counter = counter.clone();
                async move {
                    counter.fetch_add(1, Ordering::SeqCst);
                    let id = req.get("id").cloned().unwrap_or(json!(1));
                    let result = match req["method"].as_str() {
                        Some("eth_chainId") => json!("0x89"),
                        Some("eth_call") => {
                            let data = req["params"][0]["data"].as_str().unwrap_or("");
                            if data.starts_with(&format!(
                                "0x{}",
                                hex::encode(domainSeparatorCall::SELECTOR)
                            )) {
                                json!(format!("0x{}", "5a".repeat(32)))
                            } else {
                                return axum::Json(json!({
                                    "jsonrpc": "2.0", "id": id,
                                    "error": { "code": 3, "message": "execution reverted" }
                                }));
                            }
                        }
                        _ => json!(null),
                    };
                    axum::Json(json!({ "jsonrpc": "2.0", "id": id, "result": result }))
                }
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind");
        let addr = listener.local_addr().expect("addr");
        tokio::spawn(async move {
            let _ = axum::serve(listener, app).await;
        });
        (format!("http://{addr}"), calls)
    }

    /// The beacon proxy runtime captured from a live Polygon collection
    /// (`0x20ca2be53888fbdf253b5cdba13ac903433b391d`), upstream's fixture; the
    /// recorded hash is checked against it so the two cannot drift apart.
    const BEACON_PROXY_RUNTIME: &str = concat!(
        "60806040523661001357610011610017565b005b6100115b61001f61002f565b61002f61002a61013b565b6101ae565b",
        "565b3b151590565b606061004284610031565b61007d5760405162461bcd60e51b815260040180806020018281038252",
        "602681526020018061029c6026913960400191505060405180910390fd5b600080856001600160a01b03168560405180",
        "82805190602001908083835b602083106100ba5780518252601f19909201916020918201910161009b565b6001836020",
        "036101000a038019825116818451168082178552505050505050905001915050600060405180830381855af49150503d",
        "806000811461011a576040519150601f19603f3d011682016040523d82523d6000602084013e61011f565b606091505b",
        "509150915061012f8282866101d2565b925050505b9392505050565b6000610145610276565b6001600160a01b031663",
        "5c60da1b6040518163ffffffff1660e01b815260040160206040518083038186803b15801561017d57600080fd5b505a",
        "fa158015610191573d6000803e3d6000fd5b505050506040513d60208110156101a757600080fd5b5051905090565b36",
        "60008037600080366000845af43d6000803e8080156101cd573d6000f35b3d6000fd5b606083156101e1575081610134",
        "565b8251156101f15782518084602001fd5b8160405162461bcd60e51b81526004018080602001828103825283818151",
        "815260200191508051906020019080838360005b8381101561023b578181015183820152602001610223565b50505050",
        "905090810190601f1680156102685780820380516001836020036101000a031916815260200191505b50925050506040",
        "5180910390fd5b7fa3f0ad74e5423aebfd80d3ef4346578335a9a72aeaee59ff6cb3582b35133d50549056fe41646472",
        "6573733a2064656c65676174652063616c6c20746f206e6f6e2d636f6e7472616374a2646970667358221220d81e99b6",
        "e6543e743b23013becdbaf0c927e58a3e19bf7f4dcfd9c3ceb58b66464736f6c63430007060033",
    );

    const COLLECTION_IMPLEMENTATION: Address =
        alloy_primitives::address!("0x006080c6061c4af79b39da0842a3a22a7b3f185e");
    const BEACON: Address =
        alloy_primitives::address!("0xddb3781fff645325c8896aa1f067baa381607ecc");

    fn minimal_proxy(implementation: Address) -> Vec<u8> {
        [
            &MINIMAL_PROXY_PREFIX[..],
            implementation.as_slice(),
            &MINIMAL_PROXY_SUFFIX[..],
        ]
        .concat()
    }

    fn slot_word(address: Address) -> Vec<u8> {
        [vec![0u8; 12], address.to_vec()].concat()
    }

    #[test]
    fn the_recorded_beacon_hash_is_the_captured_runtime() {
        let runtime = hex::decode(BEACON_PROXY_RUNTIME).unwrap();
        assert_eq!(runtime.len(), 759);
        assert_eq!(keccak256(&runtime), BEACON_PROXY_RUNTIME_HASH);
        assert_eq!(classify_proxy_runtime(&runtime), ProxyRuntime::Beacon);
        let mut altered = runtime.clone();
        altered[100] ^= 1;
        assert_eq!(classify_proxy_runtime(&altered), ProxyRuntime::Unrecognised);
    }

    #[test]
    fn a_minimal_proxy_is_recognised_by_its_whole_runtime() {
        let code = minimal_proxy(COLLECTION_IMPLEMENTATION);
        assert_eq!(code.len(), 45);
        assert_eq!(
            classify_proxy_runtime(&code),
            ProxyRuntime::Minimal(COLLECTION_IMPLEMENTATION)
        );
        let mut trailing = code.clone();
        trailing.push(0x00);
        assert_eq!(
            classify_proxy_runtime(&trailing),
            ProxyRuntime::Unrecognised
        );
        assert_eq!(
            classify_proxy_runtime(&code[..44]),
            ProxyRuntime::Unrecognised
        );
        assert_eq!(classify_proxy_runtime(&[]), ProxyRuntime::Unrecognised);
    }

    #[test]
    fn a_slot_word_is_an_address_only_when_left_padded_and_set() {
        assert_eq!(address_in_slot(&slot_word(BEACON)), Some(BEACON));
        assert_eq!(address_in_slot(&[0u8; 32]), None);
        let mut dirty = slot_word(BEACON);
        dirty[0] = 1;
        assert_eq!(
            address_in_slot(&dirty),
            None,
            "anything above the low 20 bytes is not an address"
        );
        assert_eq!(address_in_slot(&slot_word(BEACON)[1..]), None);
    }

    /// An RPC serving `eth_getCode` / `eth_getStorageAt` per address.
    async fn spawn_code_rpc(accounts: Vec<(Address, Vec<u8>, Vec<u8>)>) -> String {
        let accounts = Arc::new(accounts);
        let app = axum::Router::new().route(
            "/",
            axum::routing::post(move |axum::Json(req): axum::Json<Value>| {
                let accounts = accounts.clone();
                async move {
                    let id = req.get("id").cloned().unwrap_or(json!(1));
                    let target: Option<Address> =
                        req["params"][0].as_str().and_then(|a| a.parse().ok());
                    let account = accounts.iter().find(|(a, _, _)| Some(*a) == target);
                    let result = match (req["method"].as_str(), account) {
                        (Some("eth_getCode"), Some((_, code, _))) => {
                            json!(format!("0x{}", hex::encode(code)))
                        }
                        (Some("eth_getCode"), None) => json!("0x"),
                        (Some("eth_getStorageAt"), Some((_, _, word))) => {
                            assert_eq!(
                                req["params"][1].as_str(),
                                Some(format!("{BEACON_SLOT:#x}").as_str())
                            );
                            json!(format!("0x{}", hex::encode(word)))
                        }
                        (Some("eth_getStorageAt"), None) => json!(format!("0x{}", "00".repeat(32))),
                        _ => json!(null),
                    };
                    axum::Json(json!({ "jsonrpc": "2.0", "id": id, "result": result }))
                }
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind");
        let addr = listener.local_addr().expect("addr");
        tokio::spawn(async move {
            let _ = axum::serve(listener, app).await;
        });
        format!("http://{addr}")
    }

    #[tokio::test]
    async fn the_proxy_anchor_is_read_from_recognised_code_only() {
        let beacon_runtime = hex::decode(BEACON_PROXY_RUNTIME).unwrap();
        let minimal = Address::repeat_byte(0x01);
        let beacon_proxy = Address::repeat_byte(0x02);
        let impostor = Address::repeat_byte(0x03);
        let dirty_slot = Address::repeat_byte(0x04);
        let mut dirty = slot_word(BEACON);
        dirty[3] = 0xff;
        let url = spawn_code_rpc(vec![
            (minimal, minimal_proxy(COLLECTION_IMPLEMENTATION), vec![]),
            (beacon_proxy, beacon_runtime.clone(), slot_word(BEACON)),
            (
                impostor,
                vec![0x60, 0x00, 0x60, 0x00, 0xf3],
                slot_word(BEACON),
            ),
            (dirty_slot, beacon_runtime, dirty),
        ])
        .await;
        let chain = ChainComponent::new(Outbound::for_tests(), &url, Duration::from_secs(5));
        assert_eq!(
            chain.get_proxy_anchor(minimal).await.unwrap(),
            Some(COLLECTION_IMPLEMENTATION)
        );
        assert_eq!(
            chain.get_proxy_anchor(beacon_proxy).await.unwrap(),
            Some(BEACON)
        );
        assert_eq!(
            chain.get_proxy_anchor(impostor).await.unwrap(),
            None,
            "a slot written by unrecognised code is a claim, not an anchor"
        );
        assert_eq!(chain.get_proxy_anchor(dirty_slot).await.unwrap(), None);
        assert_eq!(
            chain
                .get_proxy_anchor(Address::repeat_byte(0x09))
                .await
                .unwrap(),
            None,
            "no code"
        );
    }

    #[tokio::test]
    async fn domain_separator_and_chain_id_are_fetched_once_per_contract() {
        let (url, calls) = spawn_counting_rpc().await;
        let chain = ChainComponent::new(Outbound::for_tests(), &url, Duration::from_secs(5));
        let contract = Address::repeat_byte(0x11);

        let first = chain.get_domain_separator(contract).await.unwrap();
        let second = chain.get_domain_separator(contract).await.unwrap();
        assert_eq!(first, Some(B256::repeat_byte(0x5a)));
        assert_eq!(first, second);
        assert_eq!(
            calls.load(Ordering::SeqCst),
            1,
            "second lookup is served from memory"
        );

        assert_eq!(chain.get_chain_id().await.unwrap(), 137);
        assert_eq!(chain.get_chain_id().await.unwrap(), 137);
        assert_eq!(calls.load(Ordering::SeqCst), 2);

        let other = Address::repeat_byte(0x22);
        assert_eq!(chain.get_domain_separator(other).await.unwrap(), first);
        assert_eq!(
            calls.load(Ordering::SeqCst),
            3,
            "each contract is fetched once"
        );

        // A revert on a view is `None`, never a transport error.
        assert_eq!(
            chain
                .get_meta_transaction_nonce(contract, Address::ZERO)
                .await
                .unwrap(),
            None
        );
    }
}
