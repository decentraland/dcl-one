// from: catalyrst/crates/catalyrst-economy/src/ports/contracts.rs, catalyrst/nixos/web.nix (`= /contracts/addresses.json`), transactions-server src/logic/collectionProvenance.ts
//! The relay's contract allowlist: the `CONTRACT_ADDRESSES_URL` JSON (the
//! dApps' published contract map, one chain key, cached for the fetch
//! interval; unset, the copy this node serves at `/contracts/addresses.json`,
//! which the old deployment pointed the variable at) ∪ the collections mirror (`shop.collections`, the row
//! the `collections-mirror` job keeps per network). The mirror replaces the
//! crate's one `squid.collection` probe; while it has no complete pass on
//! the relay's network the answer is [`Membership::MirrorUnavailable`], never
//! "not a collection" out of an empty table.
//!
//! Collections are the one source that enrols a target without review, and a
//! relayable target reports the domain separator and nonce its own signature
//! is checked against. So with a chain, a mirrored collection is admitted only
//! when its code is a proxy over a recorded Decentraland anchor
//! ([`trusted_collection_anchor_label`]): the mirror can widen the allowlist
//! only to addresses running that code. Without an RPC this node verifies no
//! signature itself (the relay routes that would fail closed) and the mirror
//! decides as before.

use std::sync::Arc;
use std::time::Duration;

use serde_json::Value;
use sqlx::PgPool;

use alloy_primitives::Address;

use super::chain::ChainComponent;
use super::contracts_addrs::mirror_network;
use super::errors::ApiError;
use crate::http::{Outbound, read_body_capped};
use crate::shared::cache::TtlMap;

const WHITELIST_BODY_CAP: usize = 4 * 1024 * 1024;

/// The contract map the old main vhost served at `= /contracts/addresses.json`
/// (`catalyrst/nixos/contracts/addresses.json`, a copy).
pub const ADDRESSES_JSON: &[u8] = include_bytes!("addresses.json");
const WHITELIST_TIMEOUT: Duration = Duration::from_secs(20);
/// Anchors are stable by construction (a beacon upgrade repoints the beacon,
/// not the proxies), so a verified collection is remembered; a refusal never
/// is, and is re-checked.
const VERIFIED_COLLECTION_TTL: Duration = Duration::from_secs(24 * 60 * 60);
const VERIFIED_COLLECTION_CAP: usize = 8192;

/// The addresses that decide what code a collection runs, per chain: the
/// implementation the EIP-1167 minimal proxies inline and the beacon the
/// EIP-1967 beacon proxies read. Every live collection is one of the two
/// shapes over one of these; recorded here rather than read from the address
/// book so neither allowlist's outage decides the other's answers.
pub fn trusted_collection_anchor_label(chain_id: u64, anchor: Address) -> Option<&'static str> {
    let table: &[(&str, &str)] = match chain_id {
        137 => &[
            (
                "0x006080c6061c4af79b39da0842a3a22a7b3f185e",
                "CollectionImplementation",
            ),
            (
                "0xddb3781fff645325c8896aa1f067baa381607ecc",
                "UpgradeableBeacon",
            ),
        ],
        80002 => &[
            (
                "0x3195e88ae10704b359764cb38e429d24f1c2f781",
                "CollectionImplementation",
            ),
            (
                "0x1a91dd8d4eeddc2fac31f36818604b7093dc95e0",
                "UpgradeableBeacon",
            ),
        ],
        _ => &[],
    };
    let key = format!("{anchor:#x}");
    table
        .iter()
        .find(|(addr, _)| *addr == key)
        .map(|(_, label)| *label)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Membership {
    /// Whitelisted or a mirrored collection.
    Allowed,
    /// Neither, with a complete mirror pass behind the answer.
    Unknown,
    /// Not whitelisted and the collections mirror has no complete pass yet
    /// (or the relay's chain is not one the mirror covers).
    MirrorUnavailable,
}

pub struct ContractsComponent {
    pool: PgPool,
    http: Outbound,
    addresses_url: Option<String>,
    chain_key: String,
    network: Option<&'static str>,
    ttl: Duration,
    cache: TtlMap<(), Arc<Vec<String>>>,
    chain: Option<Arc<ChainComponent>>,
    verified_collections: TtlMap<String, ()>,
}

fn whitelist_accept_header() -> (&'static str, &'static str) {
    ("accept", "application/json")
}

impl ContractsComponent {
    pub fn new(
        pool: PgPool,
        http: Outbound,
        addresses_url: Option<String>,
        chain_key: String,
        chain_id: u64,
        ttl: Duration,
    ) -> Self {
        Self {
            pool,
            http,
            addresses_url: addresses_url
                .map(|u| u.trim().to_string())
                .filter(|u| !u.is_empty()),
            chain_key,
            network: mirror_network(chain_id),
            ttl,
            cache: TtlMap::new(ttl),
            chain: None,
            verified_collections: TtlMap::bounded(VERIFIED_COLLECTION_TTL, VERIFIED_COLLECTION_CAP),
        }
    }

    /// The RPC the collection provenance check reads code through.
    pub fn with_chain(mut self, chain: Option<Arc<ChainComponent>>) -> Self {
        self.chain = chain;
        self
    }

    /// The network the mirror is consulted on (`None`: an uncovered chain).
    pub fn network(&self) -> Option<&'static str> {
        self.network
    }

    pub async fn membership(&self, address: &str) -> Result<Membership, ApiError> {
        let addr = address.to_lowercase();
        // Whitelist first (a cache hit costs nothing); the mirror lookup still
        // admits on its own, so an unreachable whitelist never refuses a collection.
        let whitelisted = self.is_whitelisted(&addr).await;
        if matches!(whitelisted, Ok(true)) {
            return Ok(Membership::Allowed);
        }
        match self.mirror_membership(&addr).await? {
            Some(true) if self.runs_collection_code(&addr).await? => Ok(Membership::Allowed),
            Some(true) => whitelisted.map(|_| Membership::Unknown),
            Some(false) => whitelisted.map(|_| Membership::Unknown),
            None => whitelisted.map(|_| Membership::MirrorUnavailable),
        }
    }

    /// The old `is_valid_address`: an unavailable mirror is a 503, never a
    /// refusal.
    pub async fn is_valid_address(&self, address: &str) -> Result<bool, ApiError> {
        match self.membership(address).await? {
            Membership::Allowed => Ok(true),
            Membership::Unknown => Ok(false),
            Membership::MirrorUnavailable => Err(ApiError::RelayerUnavailable(
                "The collections mirror has no complete pass yet, so contract membership cannot be decided; retry \
                 later (MARKETPLACE_MIRRORS_ENABLED)"
                    .into(),
            )),
        }
    }

    /// `Some(found)` after a complete pass of the collections mirror on the
    /// relay's network, `None` before one.
    pub async fn mirror_membership(&self, address: &str) -> Result<Option<bool>, ApiError> {
        let Some(network) = self.network else {
            return Ok(None);
        };
        let ready: Option<bool> = sqlx::query_scalar(
            "SELECT last_complete_pass_at IS NOT NULL FROM shop.mirror_state \
             WHERE mirror = 'collections' AND network = $1",
        )
        .bind(network)
        .fetch_optional(&self.pool)
        .await?;
        if ready != Some(true) {
            return Ok(None);
        }
        let found: Option<i32> = sqlx::query_scalar(
            "SELECT 1 FROM shop.collections WHERE network = $1 AND contract_address = $2 LIMIT 1",
        )
        .bind(network)
        .bind(address.to_lowercase())
        .fetch_optional(&self.pool)
        .await?;
        Ok(Some(found.is_some()))
    }

    /// Whether a mirrored collection is a factory proxy over recorded
    /// Decentraland code (always, without a chain: see the module doc). An
    /// RPC failure is a 503, never a verdict about the address.
    async fn runs_collection_code(&self, address: &str) -> Result<bool, ApiError> {
        let Some(chain) = self.chain.as_ref() else {
            return Ok(true);
        };
        if self
            .verified_collections
            .get_fresh(&address.to_string())
            .is_some()
        {
            return Ok(true);
        }
        let Ok(target) = address.parse::<Address>() else {
            return Ok(false);
        };
        let unavailable = |e: super::chain::ChainError| {
            ApiError::RelayerUnavailable(format!("Could not verify the collection's code: {e}"))
        };
        let (chain_id, anchor) =
            tokio::try_join!(chain.get_chain_id(), chain.get_proxy_anchor(target))
                .map_err(unavailable)?;
        if anchor.is_some_and(|a| trusted_collection_anchor_label(chain_id, a).is_some()) {
            self.verified_collections.insert(address.to_string(), ());
            return Ok(true);
        }
        tracing::warn!(
            address,
            chain_id,
            anchor = %anchor.map(|a| format!("{a:#x}")).unwrap_or_else(|| "not a proxy".into()),
            "economy: the collections mirror lists an address that does not run Decentraland collection code"
        );
        Ok(false)
    }

    pub async fn is_whitelisted(&self, address: &str) -> Result<bool, ApiError> {
        let addr = address.to_lowercase();
        let addresses = self
            .cache
            .get_or_fetch((), || self.fetch_whitelist())
            .await?;
        if addresses.is_empty() {
            // An empty list is never trusted past this call.
            self.cache.clear();
        }
        Ok(addresses.contains(&addr))
    }

    async fn fetch_whitelist(&self) -> Result<Arc<Vec<String>>, ApiError> {
        let Some(url) = self.addresses_url.as_deref() else {
            let json: Value = serde_json::from_slice(ADDRESSES_JSON)
                .map_err(|e| ApiError::Internal(format!("addresses.json: {e}")))?;
            return Ok(Arc::new(chain_addresses(&json, &self.chain_key)));
        };
        let (accept_name, accept_value) = whitelist_accept_header();
        let resp = self
            .http
            .get(url)
            .header(accept_name, accept_value)
            .timeout(WHITELIST_TIMEOUT)
            .send()
            .await
            .map_err(|e| ApiError::Internal(format!("whitelist fetch failed: {e}")))?;
        if !resp.status().is_success() {
            return Err(ApiError::Internal(format!(
                "Could not get the whitelisted addresses from CONTRACT_ADDRESSES_URL (status {})",
                resp.status().as_u16()
            )));
        }
        let bytes = read_body_capped(resp, WHITELIST_BODY_CAP)
            .await
            .map_err(|e| ApiError::Internal(format!("whitelist read failed: {e}")))?;
        let json: Value = serde_json::from_slice(&bytes)
            .map_err(|e| ApiError::Internal(format!("whitelist decode failed: {e}")))?;
        Ok(Arc::new(chain_addresses(&json, &self.chain_key)))
    }

    pub fn ttl(&self) -> Duration {
        self.ttl
    }
}

/// The lowercased addresses under one chain key of a contract map.
fn chain_addresses(json: &Value, chain_key: &str) -> Vec<String> {
    json.get(chain_key.to_lowercase())
        .and_then(|v| v.as_object())
        .map(|obj| {
            obj.values()
                .filter_map(|v| v.as_str())
                .map(|s| s.to_lowercase())
                .collect()
        })
        .unwrap_or_default()
}

/// `GET /contracts/addresses.json`: the served copy of the contract map.
pub async fn addresses_document() -> impl axum::response::IntoResponse {
    (
        [(axum::http::header::CONTENT_TYPE, "application/json")],
        ADDRESSES_JSON,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_served_contract_map_is_the_old_one_and_feeds_the_default_whitelist() {
        let json: Value = serde_json::from_slice(ADDRESSES_JSON).unwrap();
        let keys: Vec<&str> = json
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect();
        for key in [
            "mainnet", "sepolia", "matic", "amoy", "arbitrum", "goerli", "mumbai",
        ] {
            assert!(keys.contains(&key), "{key}");
        }
        let matic = chain_addresses(&json, "MATIC");
        assert_eq!(matic.len(), 34);
        assert!(
            matic
                .iter()
                .all(|a| a.starts_with("0x") && *a == a.to_lowercase())
        );
        assert!(chain_addresses(&json, "nowhere").is_empty());
    }

    #[test]
    fn whitelist_get_uses_accept_not_content_type() {
        let (name, value) = whitelist_accept_header();
        assert_eq!(name, "accept");
        assert_ne!(name, "content-type");
        assert_eq!(value, "application/json");
    }

    #[test]
    fn the_relay_chain_maps_to_the_mirror_network() {
        assert_eq!(mirror_network(137), Some("matic"));
        assert_eq!(mirror_network(80002), Some("matic"));
        assert_eq!(mirror_network(1), Some("ethereum"));
        assert_eq!(mirror_network(4242), None);
    }
}
