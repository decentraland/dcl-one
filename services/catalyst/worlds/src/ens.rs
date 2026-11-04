//! The ENS owner of a `*.eth` world (`ALLOW_ENS_DOMAINS`), read over
//! `ETH_RPC_URL` as the worlds content server reads it: a second-level name
//! is the BaseRegistrar's `ownerOf(labelhash)`, unless that owner is the
//! NameWrapper, whose `ownerOf(namehash)` then answers, as it does for every
//! deeper name. A reverted call or the zero address is "nobody holds it"; an
//! RPC fault is an error, never an owner. Answers are memoized per name for
//! [`ENS_OWNER_TTL`].

use std::sync::Arc;
use std::time::Duration;

use alloy_primitives::{Address as EvmAddress, B256, U256, address, keccak256};
use alloy_sol_types::{SolCall, sol};
use serde_json::json;

use crate::comms::world_identity::WorldName;
use crate::http::Outbound;
use crate::http::jsonrpc::{Rpc, RpcError};
use crate::shared::cache::TtlMap;
use crate::shared::identity::Address;

sol! {
    interface OwnerOf {
        function ownerOf(uint256 id) external view returns (address);
    }
}

/// The upstream's one-minute name-ownership memo.
pub const ENS_OWNER_TTL: Duration = Duration::from_secs(60);
const CACHE_MAX_ENTRIES: usize = 20_000;
const RPC_TIMEOUT: Duration = Duration::from_secs(10);

/// The ENS contracts of one network (the upstream's `ensContracts`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EnsContracts {
    pub base_registrar: EvmAddress,
    pub name_wrapper: EvmAddress,
}

impl EnsContracts {
    /// `ETH_NETWORK`'s contracts; `None` for a network ENS is not read on.
    pub fn for_network(network: &str) -> Option<Self> {
        let base_registrar = address!("57f1887a8BF19b14fC0dF6Fd9B2acc9Af147eA85");
        match network {
            "mainnet" => Some(Self {
                base_registrar,
                name_wrapper: address!("D4416b13d2b3a9aBae7AcD5D6C2BbDBE25686401"),
            }),
            "sepolia" => Some(Self {
                base_registrar,
                name_wrapper: address!("0635513f179D50A207757E05759CbD106d7dFcE8"),
            }),
            _ => None,
        }
    }
}

/// ENSIP-1 namehash of an already normalized name.
pub fn namehash(name: &str) -> B256 {
    if name.is_empty() {
        return B256::ZERO;
    }
    name.rsplit('.').fold(B256::ZERO, |node, label| {
        let mut buf = [0u8; 64];
        buf[..32].copy_from_slice(node.as_slice());
        buf[32..].copy_from_slice(keccak256(label.as_bytes()).as_slice());
        keccak256(buf)
    })
}

/// Names this reader resolves: ASCII labels a normalizer leaves unchanged.
/// Anything else has no owner here (no ENSIP-15 normalization).
fn resolvable(name: &str) -> bool {
    name.split('.').all(|label| {
        !label.is_empty()
            && label
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-' || b == b'_')
    })
}

/// Whether a JSON-RPC error is the call's own revert.
fn is_revert(e: &RpcError) -> bool {
    matches!(e, RpcError::Response { code, message }
    if *code == 3 || {
        let m = message.to_ascii_lowercase();
        m.contains("revert") || m.contains("vm execution error")
    })
}

pub struct EnsResolver {
    http: Outbound,
    rpc_url: String,
    contracts: EnsContracts,
    cache: TtlMap<String, Arc<Option<Address>>>,
}

impl EnsResolver {
    pub fn new(http: Outbound, rpc_url: &str, contracts: EnsContracts) -> Self {
        Self {
            http,
            rpc_url: rpc_url.to_string(),
            contracts,
            cache: TtlMap::bounded(ENS_OWNER_TTL, CACHE_MAX_ENTRIES),
        }
    }

    /// Drops every memoized answer (tests).
    pub fn clear_cache(&self) {
        self.cache.clear();
    }

    /// The holder of the ENS world; `Ok(None)` when nobody holds it.
    pub async fn owner(&self, world: &WorldName) -> Result<Option<Address>, String> {
        let name = world.key().to_string();
        if !world.is_ens_world() || !resolvable(&name) {
            return Ok(None);
        }
        self.cache
            .get_or_fetch(name.clone(), || async move {
                self.resolve(&name).await.map(Arc::new)
            })
            .await
            .map(|owner| (*owner).clone())
    }

    async fn resolve(&self, name: &str) -> Result<Option<Address>, String> {
        let labels: Vec<&str> = name.split('.').collect();
        if let [label, _tld] = labels[..] {
            let id = U256::from_be_bytes(keccak256(label.as_bytes()).0);
            match self.owner_of(self.contracts.base_registrar, id).await? {
                Some(owner) if owner == self.contracts.name_wrapper => {}
                owner => return Ok(owner.map(to_address)),
            }
        }
        let id = U256::from_be_bytes(namehash(name).0);
        Ok(self
            .owner_of(self.contracts.name_wrapper, id)
            .await?
            .map(to_address))
    }

    /// `ownerOf(id)` on `contract` at the head; `None` when reverted or zero.
    async fn owner_of(&self, contract: EvmAddress, id: U256) -> Result<Option<EvmAddress>, String> {
        let data = OwnerOf::ownerOfCall { id }.abi_encode();
        let params = json!([
            { "to": format!("{contract:#x}"), "data": format!("0x{}", hex::encode(data)) },
            "latest"
        ]);
        let answer = Rpc::new(&self.http, &self.rpc_url)
            .with_timeout(RPC_TIMEOUT)
            .call_hex("eth_call", params)
            .await;
        let hex = match answer {
            Ok(hex) => hex,
            Err(e) if is_revert(&e) => return Ok(None),
            Err(e) => return Err(format!("ENS ownerOf: {e}")),
        };
        let bytes = hex::decode(hex.trim_start_matches("0x"))
            .map_err(|e| format!("ENS ownerOf: bad hex: {e}"))?;
        if bytes.is_empty() {
            return Ok(None);
        }
        let owner = OwnerOf::ownerOfCall::abi_decode_returns(&bytes)
            .map_err(|e| format!("ENS ownerOf: bad answer: {e}"))?;
        Ok(Some(owner).filter(|o| !o.is_zero()))
    }
}

fn to_address(owner: EvmAddress) -> Address {
    Address::parse(&format!("{owner:#x}")).expect("a 20-byte address is a valid address")
}

#[cfg(test)]
pub(crate) mod test_support {
    use std::collections::HashMap;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{Arc, Mutex};

    use alloy_primitives::{Address as EvmAddress, U256, keccak256};
    use alloy_sol_types::SolCall;
    use axum::extract::State;
    use axum::routing::post;
    use axum::{Json, Router};
    use serde_json::{Value, json};

    use super::{EnsContracts, EnsResolver, OwnerOf, namehash};
    use crate::http::Outbound;
    use crate::shared::identity::Address;

    /// The ENS registry the fake chain answers from.
    #[derive(Clone, Default)]
    pub struct FakeEns {
        /// `(contract, id)` -> owner; a missing id reverts.
        owners: Arc<Mutex<HashMap<(EvmAddress, U256), EvmAddress>>>,
        pub down: Arc<AtomicBool>,
        pub calls: Arc<Mutex<Vec<(EvmAddress, U256)>>>,
    }

    pub fn contracts() -> EnsContracts {
        EnsContracts::for_network("mainnet").unwrap()
    }

    fn evm(a: &Address) -> EvmAddress {
        a.as_str().parse().unwrap()
    }

    impl FakeEns {
        /// A second-level name registered directly to `owner`.
        pub fn register(&self, name: &str, owner: &Address) {
            let label = name.split('.').next().unwrap();
            let id = U256::from_be_bytes(keccak256(label.as_bytes()).0);
            self.owners
                .lock()
                .unwrap()
                .insert((contracts().base_registrar, id), evm(owner));
        }

        /// A name held through the NameWrapper (the registrar names the wrapper).
        pub fn wrap(&self, name: &str, owner: &Address) {
            let c = contracts();
            let labels: Vec<&str> = name.split('.').collect();
            if labels.len() == 2 {
                let id = U256::from_be_bytes(keccak256(labels[0].as_bytes()).0);
                self.owners
                    .lock()
                    .unwrap()
                    .insert((c.base_registrar, id), c.name_wrapper);
            }
            let id = U256::from_be_bytes(namehash(name).0);
            self.owners
                .lock()
                .unwrap()
                .insert((c.name_wrapper, id), evm(owner));
        }

        pub fn set_down(&self, down: bool) {
            self.down.store(down, Ordering::Relaxed);
        }
    }

    async fn answer(
        State(ens): State<FakeEns>,
        Json(body): Json<Value>,
    ) -> axum::response::Response {
        use axum::response::IntoResponse;
        if ens.down.load(Ordering::Relaxed) {
            return (axum::http::StatusCode::BAD_GATEWAY, "down").into_response();
        }
        let id = body["id"].clone();
        let call = &body["params"][0];
        let to: EvmAddress = call["to"].as_str().unwrap().parse().unwrap();
        let data = hex::decode(call["data"].as_str().unwrap().trim_start_matches("0x")).unwrap();
        let token = OwnerOf::ownerOfCall::abi_decode(&data).unwrap().id;
        ens.calls.lock().unwrap().push((to, token));
        let owner = ens.owners.lock().unwrap().get(&(to, token)).copied();
        Json(match owner {
            Some(owner) => json!({
                "jsonrpc": "2.0", "id": id,
                "result": format!("0x{}", hex::encode(OwnerOf::ownerOfCall::abi_encode_returns(&owner)))
            }),
            None => json!({
                "jsonrpc": "2.0", "id": id,
                "error": { "code": 3, "message": "execution reverted: ERC721: invalid token ID" }
            }),
        })
        .into_response()
    }

    /// A resolver over a loopback fake chain, plus the fake's registry.
    pub async fn resolver() -> (Arc<EnsResolver>, FakeEns) {
        let ens = FakeEns::default();
        let app = Router::new()
            .route("/", post(answer))
            .with_state(ens.clone());
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        let url = format!("http://{addr}/");
        (
            Arc::new(EnsResolver::new(Outbound::for_tests(), &url, contracts())),
            ens,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::test_support::*;
    use super::*;

    fn wallet() -> Address {
        Address::parse(&format!("0x00000000{}", uuid::Uuid::new_v4().simple())).unwrap()
    }

    fn world(name: &str) -> WorldName {
        WorldName::parse(name).unwrap()
    }

    #[test]
    fn namehash_matches_the_ensip1_vectors() {
        assert_eq!(namehash(""), B256::ZERO);
        assert_eq!(
            format!("{:#x}", namehash("eth")),
            "0x93cdeb708b7545dc668eb9280176169d1c33cfd8ed6f04690a0bcc88a93fc4ae"
        );
        assert_eq!(
            format!("{:#x}", namehash("foo.eth")),
            "0xde9b09fd7c5f901e23a3f19fecc54828e9c848539801e86591bd9801b019f84f"
        );
    }

    #[tokio::test]
    async fn a_second_level_name_is_the_registrars_and_a_wrapped_one_the_wrappers() {
        let (r, ens) = resolver().await;
        let (direct, wrapped, sub) = (wallet(), wallet(), wallet());
        ens.register("direct.eth", &direct);
        ens.wrap("wrapped.eth", &wrapped);
        ens.wrap("a.wrapped.eth", &sub);
        assert_eq!(r.owner(&world("Direct.eth")).await.unwrap(), Some(direct));
        assert_eq!(r.owner(&world("wrapped.eth")).await.unwrap(), Some(wrapped));
        assert_eq!(r.owner(&world("a.wrapped.eth")).await.unwrap(), Some(sub));
        assert_eq!(
            r.owner(&world("nobody.eth")).await.unwrap(),
            None,
            "reverted"
        );
        assert_eq!(r.owner(&world("x.nobody.eth")).await.unwrap(), None);
        for unresolved in ["foo.dcl.eth", "foo", "café.eth", "a!b.eth"] {
            assert_eq!(
                r.owner(&world(unresolved)).await.unwrap(),
                None,
                "{unresolved}"
            );
        }
    }

    #[tokio::test]
    async fn an_rpc_fault_is_an_error_and_answers_are_memoized() {
        let (r, ens) = resolver().await;
        let owner = wallet();
        ens.register("memo.eth", &owner);
        assert_eq!(
            r.owner(&world("memo.eth")).await.unwrap(),
            Some(owner.clone())
        );
        ens.set_down(true);
        assert_eq!(
            r.owner(&world("memo.eth")).await.unwrap(),
            Some(owner),
            "within the TTL"
        );
        assert!(r.owner(&world("other.eth")).await.is_err());
        r.clear_cache();
        assert!(r.owner(&world("memo.eth")).await.is_err());
    }
}
