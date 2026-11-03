//! Which item URNs an ownership check asks the chain about, as
//! `@dcl/urn-resolver` resolves them and the reference client splits them:
//! a collections-v1 or v2 item by its token on the chain its network names,
//! a third-party item by its linked NFT on the chain it names. Assets
//! without a token and URNs that are not collection items are not asked
//! about; a collection URN that names no item it can ask about is never owned.

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
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ItemRef {
    /// Not an item with a token: counts as owned.
    NotAsked,
    /// A collection URN naming no item that can be asked about: never owned.
    Malformed,
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
            ItemRef::NotAsked | ItemRef::Malformed => None,
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

/// Whether a lowercase network is Ethereum mainnet (`ethereum`, or the
/// `mainnet` alias the stored URNs normalize).
fn is_ethereum(network: &str) -> bool {
    network == "ethereum" || network == "mainnet"
}

/// The chain a lowercase URN network names: `Some(true)` on L1, `Some(false)` on L2.
fn layer(network: &str) -> Option<bool> {
    if is_ethereum(network) || L1_NETWORKS.contains(&network) {
        Some(true)
    } else if L2_NETWORKS.contains(&network) {
        Some(false)
    } else {
        None
    }
}

/// `ownerOf(id)` on `contract`; a token or address that does not parse is never owned.
fn token_ref(l1: bool, contract: Option<Address>, id: &str) -> ItemRef {
    match U256::from_str_radix(id, 10).ok() {
        Some(token) => ItemRef::Token {
            l1,
            contract,
            token,
        },
        None => ItemRef::Malformed,
    }
}

pub fn item_ref(urn: &str) -> ItemRef {
    let urn = urn.to_lowercase();
    let parts: Vec<&str> = urn.split(':').collect();
    let ["urn", "decentraland", network, kind, rest @ ..] = &parts[..] else {
        return ItemRef::NotAsked;
    };
    let (network, kind) = (*network, *kind);
    if !matches!(
        kind,
        "collections-v1" | "collections-v2" | "collections-thirdparty"
    ) {
        return ItemRef::NotAsked;
    }
    let Some(l1) = layer(network) else {
        return ItemRef::Malformed;
    };
    match (kind, rest) {
        // A collection or an asset without a token: the reference counts it as owned.
        ("collections-v1", [_] | [_, _]) => ItemRef::NotAsked,
        ("collections-v1", [collection, _name, id]) if is_digits(id) => {
            if is_hex_address(collection) {
                token_ref(l1, collection.parse().ok(), id)
            } else if is_ethereum(network) {
                token_ref(l1, v1_collection_contract(collection), id)
            } else {
                ItemRef::Malformed
            }
        }
        ("collections-v2", [collection]) if is_hex_address(collection) => ItemRef::NotAsked,
        ("collections-v2", [collection, item])
            if is_hex_address(collection) && (is_hex_address(item) || is_digits(item)) =>
        {
            ItemRef::NotAsked
        }
        ("collections-v2", [collection, item, id])
            if is_hex_address(collection)
                && (is_hex_address(item) || is_digits(item))
                && is_digits(id) =>
        {
            token_ref(l1, collection.parse().ok(), id)
        }
        ("collections-thirdparty", [_] | [_, _] | [_, _, _]) if !l1 => ItemRef::NotAsked,
        ("collections-thirdparty", [_tp, _collection, _item, nft_chain, nft_contract, id])
            if !l1 && is_hex_address(nft_contract) && is_digits(id) =>
        {
            match (
                layer(nft_chain),
                nft_contract.parse::<Address>().ok(),
                U256::from_str_radix(id, 10).ok(),
            ) {
                (Some(l1), Some(contract), Some(token)) => ItemRef::ThirdParty {
                    l1,
                    contract,
                    token,
                },
                _ => ItemRef::Malformed,
            }
        }
        _ => ItemRef::Malformed,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const A: &str = "0xaaa0000000000000000000000000000000000001";

    #[test]
    fn items_with_a_token_are_asked_and_assets_are_not() {
        let addr: Address = A.parse().unwrap();
        let token = |l1, contract, n: u64| ItemRef::Token {
            l1,
            contract,
            token: U256::from(n),
        };
        assert_eq!(
            item_ref(&format!("urn:decentraland:matic:collections-v2:{A}:1:42")),
            token(false, Some(addr), 42)
        );
        assert_eq!(
            item_ref(&format!("urn:decentraland:AMOY:collections-v2:{A}:0x1:7")),
            token(false, Some(addr), 7)
        );
        assert_eq!(
            item_ref(&format!("urn:decentraland:sepolia:collections-v2:{A}:0:5")),
            token(true, Some(addr), 5),
            "a v2 item on an L1 network is asked on L1, as the reference splits it"
        );
        assert_eq!(
            item_ref(&format!("urn:decentraland:matic:collections-v1:{A}:hat:3")),
            token(false, Some(addr), 3)
        );
        let halloween = "0xc1f4b0eea2bd6690930e6c66efd3e197d620b9c2".parse().ok();
        assert_eq!(
            item_ref("urn:decentraland:ethereum:collections-v1:halloween_2019:hat:12"),
            token(true, halloween, 12)
        );
        assert_eq!(
            item_ref("urn:decentraland:ethereum:collections-v1:not_listed:hat:12"),
            token(true, None, 12),
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
            format!("urn:decentraland:matic:collections-v2:{A}"),
            "urn:decentraland:ethereum:collections-v1:halloween_2019:hat".to_string(),
            "urn:decentraland:ethereum:collections-v1:halloween_2019".to_string(),
            "urn:decentraland:off-chain:base-avatars:eyes_00".to_string(),
            "urn:decentraland:matic:collections-thirdparty:tp:c:i".to_string(),
            "urn:decentraland:matic:collections-thirdparty:tp".to_string(),
            "dcl://halloween_2019/hat".to_string(),
            "invalid-urn".to_string(),
        ] {
            assert_eq!(item_ref(&not_asked), ItemRef::NotAsked, "{not_asked}");
        }
    }

    #[test]
    fn case_and_spelling_variants_are_asked_like_the_canonical_urn() {
        let addr: Address = A.parse().unwrap();
        let expected = ItemRef::Token {
            l1: false,
            contract: Some(addr),
            token: U256::from(42),
        };
        let hex = &A[2..];
        for variant in [
            format!("urn:decentraland:matic:Collections-V2:{A}:0:42"),
            format!("URN:DECENTRALAND:MATIC:COLLECTIONS-V2:{A}:0:42"),
            format!("urn:decentraland:matic:collections-v2:0X{hex}:0:42"),
            format!(
                "urn:decentraland:matic:collections-v2:0x{}:0:42",
                hex.to_uppercase()
            ),
        ] {
            assert_eq!(item_ref(&variant), expected, "{variant}");
        }
        for chain in ["ethereum", "Mainnet", "SEPOLIA"] {
            assert_eq!(
                item_ref(&format!(
                    "urn:decentraland:matic:collections-thirdparty:tp:c:i:{chain}:0X{hex}:5"
                )),
                ItemRef::ThirdParty {
                    l1: true,
                    contract: addr,
                    token: U256::from(5)
                },
                "{chain}"
            );
        }
        assert_eq!(
            item_ref("urn:decentraland:mainnet:collections-v1:halloween_2019:hat:12"),
            item_ref("urn:decentraland:ethereum:collections-v1:halloween_2019:hat:12"),
            "mainnet is the ethereum alias"
        );
        assert_eq!(
            item_ref("urn:decentraland:mainnet:collections-v1:halloween_2019:hat"),
            ItemRef::NotAsked
        );
        assert_eq!(
            item_ref("urn:decentraland:ethereum:Collections-V1:Halloween_2019:hat:12"),
            item_ref("urn:decentraland:ethereum:collections-v1:halloween_2019:hat:12")
        );
    }

    #[test]
    fn collection_urns_that_name_no_askable_item_are_malformed() {
        for malformed in [
            format!("urn:decentraland:matic:collections-v2:{A}:0:42:extra"),
            format!("urn:decentraland:matic:collections-v2:{A}:0:0x2a"),
            format!("urn:decentraland:matic:collections-v2:{A}:abc:5"),
            format!("urn:decentraland:matic:collections-v2:{A}:0:5abc"),
            "urn:decentraland:matic:collections-v2:not-an-address:0:5".to_string(),
            format!("urn:decentraland:foo:collections-v2:{A}:0:42"),
            "urn:decentraland:sepolia:collections-v1:halloween_2019:hat:12".to_string(),
            "urn:decentraland:ethereum:collections-v1:halloween_2019:hat:12:x".to_string(),
            "urn:decentraland:ethereum:collections-v1:halloween_2019:hat:0x1f".to_string(),
            format!("urn:decentraland:matic:collections-thirdparty:tp:c:i:foo:{A}:5"),
            format!("urn:decentraland:matic:collections-thirdparty:tp:c:i:mainnet:{A}:5:x"),
            format!("urn:decentraland:matic:collections-thirdparty:tp:c:i:mainnet:{A}:abc"),
            "urn:decentraland:matic:collections-thirdparty:tp:c:i:mainnet".to_string(),
            format!("urn:decentraland:ethereum:collections-thirdparty:tp:c:i:mainnet:{A}:5"),
        ] {
            assert_eq!(item_ref(&malformed), ItemRef::Malformed, "{malformed}");
        }
        assert_eq!(
            item_ref("urn:decentraland:matic:collections-v2:0x123:0:5"),
            ItemRef::Token {
                l1: false,
                contract: None,
                token: U256::from(5)
            },
            "a contract that is not an address is never owned"
        );
    }

    #[test]
    fn integers_parse_in_either_base() {
        assert_eq!(parse_uint("12"), Some(U256::from(12)));
        assert_eq!(parse_uint("0x10"), Some(U256::from(16)));
        assert_eq!(parse_uint("x"), None);
        assert_eq!(parse_uint(""), None);
    }
}
