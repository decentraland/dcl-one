//! Which item URNs an ownership check asks the chain about, as
//! `@dcl/urn-resolver` resolves them and the reference client splits them:
//! a collections-v1 item on L1 and a collections-v2 item on L2 by their
//! token, a third-party item by its linked NFT on the chain it names.
//! Anything else (assets without a token, off-chain and unparsable URNs) is
//! not asked about.

use std::sync::LazyLock;

use alloy_primitives::{Address, U256};
use serde::Deserialize;

use super::super::types::{L1_NETWORKS, L2_NETWORKS};

const COLLECTIONS_V1_JSON: &str = include_str!("contracts/collections-v1.json");

#[derive(Deserialize)]
struct V1Collection {
    id: String,
    name: String,
}

static COLLECTIONS_V1: LazyLock<Vec<(String, Address)>> = LazyLock::new(|| {
    serde_json::from_str::<Vec<V1Collection>>(COLLECTIONS_V1_JSON)
        .expect("vendored collections-v1.json")
        .into_iter()
        .filter_map(|c| {
            Some((
                c.name.strip_prefix("dcl://")?.to_string(),
                c.id.parse().ok()?,
            ))
        })
        .collect()
});

/// The mainnet contract of a collections-v1 collection named in place of its address.
pub fn v1_collection_contract(name: &str) -> Option<Address> {
    COLLECTIONS_V1
        .iter()
        .find(|(n, _)| n == name)
        .map(|(_, a)| *a)
}

/// What an ownership check asks about one URN.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ItemRef {
    /// Not an item with a token: counts as owned.
    NotAsked,
    /// A collection item: `ownerOf(token)` on `contract` (`None`: a v1
    /// collection name with no known contract, never owned).
    Token {
        l1: bool,
        contract: Option<Address>,
        token: U256,
    },
    /// A third-party item's linked NFT.
    ThirdParty {
        l1: bool,
        contract: Address,
        token: U256,
    },
}

impl ItemRef {
    /// The layer the check reads, `None` when nothing is read.
    pub fn on_l1(&self) -> Option<bool> {
        match self {
            ItemRef::NotAsked => None,
            ItemRef::Token { l1, .. } | ItemRef::ThirdParty { l1, .. } => Some(*l1),
        }
    }
}

/// A decimal or `0x` hex unsigned integer.
pub fn parse_uint(s: &str) -> Option<U256> {
    match s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
        Some(hex) => U256::from_str_radix(hex, 16).ok(),
        None if !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit()) => {
            U256::from_str_radix(s, 10).ok()
        }
        None => None,
    }
}

fn is_hex_address(s: &str) -> bool {
    s.len() > 2 && s.starts_with("0x") && s[2..].bytes().all(|b| b.is_ascii_hexdigit())
}

fn is_digits(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit())
}

/// A URN network the resolver knows (`mainnet` is spelled `ethereum`):
/// `Some(true)` on L1, `Some(false)` on L2.
fn layer(network: &str) -> Option<bool> {
    let network = network.to_lowercase();
    if network == "ethereum" || (network != "mainnet" && L1_NETWORKS.contains(&network.as_str())) {
        Some(true)
    } else if L2_NETWORKS.contains(&network.as_str()) {
        Some(false)
    } else {
        None
    }
}

/// An address the call can be sent to; an unparsable one is never owned.
fn contract(s: &str) -> Option<Address> {
    s.parse().ok()
}

pub fn item_ref(urn: &str) -> ItemRef {
    let parts: Vec<&str> = urn.split(':').collect();
    let ["urn", "decentraland", network, kind, rest @ ..] = &parts[..] else {
        return ItemRef::NotAsked;
    };
    let (network, kind) = (*network, *kind);
    let Some(l1) = layer(network) else {
        return ItemRef::NotAsked;
    };
    let token = |s: &str| U256::from_str_radix(s, 10).ok();
    match (kind, rest) {
        ("collections-v1", [collection, _name, id]) if l1 && is_digits(id) => {
            let address = if is_hex_address(collection) {
                contract(collection)
            } else if network == "ethereum" {
                v1_collection_contract(collection)
            } else {
                return ItemRef::NotAsked;
            };
            match token(id) {
                Some(token) => ItemRef::Token {
                    l1,
                    contract: address,
                    token,
                },
                None => ItemRef::Token {
                    l1,
                    contract: None,
                    token: U256::ZERO,
                },
            }
        }
        ("collections-v2", [collection, item, id])
            if !l1
                && is_hex_address(collection)
                && (is_hex_address(item) || is_digits(item))
                && is_digits(id) =>
        {
            match token(id) {
                Some(token) => ItemRef::Token {
                    l1,
                    contract: contract(collection),
                    token,
                },
                None => ItemRef::Token {
                    l1,
                    contract: None,
                    token: U256::ZERO,
                },
            }
        }
        ("collections-thirdparty", [_tp, _collection, _item, nft_chain, nft_contract, id])
            if !l1 && is_hex_address(nft_contract) && is_digits(id) =>
        {
            let nft_l1 = if L1_NETWORKS.contains(nft_chain) {
                true
            } else if L2_NETWORKS.contains(nft_chain) {
                false
            } else {
                return ItemRef::NotAsked;
            };
            match (contract(nft_contract), token(id)) {
                (Some(contract), Some(token)) => ItemRef::ThirdParty {
                    l1: nft_l1,
                    contract,
                    token,
                },
                _ => ItemRef::Token {
                    l1: nft_l1,
                    contract: None,
                    token: U256::ZERO,
                },
            }
        }
        _ => ItemRef::NotAsked,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const A: &str = "0xaaa0000000000000000000000000000000000001";

    #[test]
    fn items_with_a_token_are_asked_and_assets_are_not() {
        let addr: Address = A.parse().unwrap();
        assert_eq!(
            item_ref(&format!("urn:decentraland:matic:collections-v2:{A}:1:42")),
            ItemRef::Token {
                l1: false,
                contract: Some(addr),
                token: U256::from(42)
            }
        );
        assert_eq!(
            item_ref(&format!("urn:decentraland:AMOY:collections-v2:{A}:0x1:7")),
            ItemRef::Token {
                l1: false,
                contract: Some(addr),
                token: U256::from(7)
            }
        );
        assert_eq!(
            item_ref("urn:decentraland:ethereum:collections-v1:halloween_2019:hat:12"),
            ItemRef::Token {
                l1: true,
                contract: Some(
                    "0xc1f4b0eea2bd6690930e6c66efd3e197d620b9c2"
                        .parse()
                        .unwrap()
                ),
                token: U256::from(12)
            }
        );
        assert_eq!(
            item_ref("urn:decentraland:ethereum:collections-v1:not_listed:hat:12"),
            ItemRef::Token {
                l1: true,
                contract: None,
                token: U256::from(12)
            },
            "an unknown v1 name is never owned"
        );
        assert_eq!(
            item_ref(&format!(
                "urn:decentraland:matic:collections-thirdparty:tp:c:i:mainnet:{A}:5"
            )),
            ItemRef::ThirdParty {
                l1: true,
                contract: addr,
                token: U256::from(5)
            }
        );
        for not_asked in [
            format!("urn:decentraland:matic:collections-v2:{A}:1"),
            "urn:decentraland:ethereum:collections-v1:halloween_2019:hat".to_string(),
            "urn:decentraland:off-chain:base-avatars:eyes_00".to_string(),
            "urn:decentraland:matic:collections-thirdparty:tp:c:i".to_string(),
            format!("urn:decentraland:mainnet:collections-v2:{A}:1:3"),
            format!("urn:decentraland:ethereum:collections-v2:{A}:1:3"),
            format!("urn:decentraland:matic:collections-v1:{A}:hat:3"),
            "urn:decentraland:sepolia:collections-v1:halloween_2019:hat:12".to_string(),
            "dcl://halloween_2019/hat".to_string(),
            "invalid-urn".to_string(),
        ] {
            assert_eq!(item_ref(&not_asked), ItemRef::NotAsked, "{not_asked}");
        }
    }

    #[test]
    fn integers_parse_in_either_base() {
        assert_eq!(parse_uint("12"), Some(U256::from(12)));
        assert_eq!(parse_uint("0x10"), Some(U256::from(16)));
        assert_eq!(parse_uint("x"), None);
        assert_eq!(parse_uint(""), None);
    }
}
