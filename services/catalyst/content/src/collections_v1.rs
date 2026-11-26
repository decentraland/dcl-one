//! The collections-v1 catalogue on Ethereum, vendored beside the content
//! validator's contracts (`validator/onchain/contracts/`, provenance in its
//! `upstream.lock.json`):
//!
//! - `collections-v1.json`: the 42 v1 collections, each contract and its
//!   `name` (`dcl://<collection>`), from `@dcl/urn-resolver`;
//! - `collections-v1-symbols.json`: each one's `symbol()`, read from the
//!   chain (set by its constructor, never changed);
//! - `wearables-v1.json`: the static representations the collections
//!   subgraph gives a v1 item (`decentraland/collections-graph`
//!   `src/data/wearablesV1`), as arrays in the order it searches them.
//!
//! No v1 item's name, description, category, rarity or body shapes is on
//! chain: an `AddWearable` names only the wearable id and its maximum
//! issuance. The subgraph takes the rest from the first representation with
//! that id, whichever collection's array holds it ([`representation`]).

use std::collections::HashMap;
use std::sync::LazyLock;

use serde::Deserialize;

const COLLECTIONS_JSON: &str = include_str!("validator/onchain/contracts/collections-v1.json");
const SYMBOLS_JSON: &str = include_str!("validator/onchain/contracts/collections-v1-symbols.json");
const WEARABLES_JSON: &str = include_str!("validator/onchain/contracts/wearables-v1.json");

/// One v1 collection.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
pub struct Collection {
    /// The contract, lowercase hex.
    #[serde(rename = "id")]
    pub address: String,
    /// `name()` as the contract answers it: `dcl://<collection>`.
    pub name: String,
    /// `symbol()` as the contract answers it.
    #[serde(skip)]
    pub symbol: String,
}

impl Collection {
    /// The collection segment of its URNs: the name without `dcl://`.
    pub fn urn_name(&self) -> &str {
        self.name.strip_prefix("dcl://").unwrap_or(&self.name)
    }
}

/// One static representation of a v1 wearable.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Wearable {
    pub id: String,
    pub name: String,
    pub description: String,
    pub category: String,
    pub rarity: String,
    pub body_shapes: Vec<String>,
}

/// One array of the file; its `collection` name is for review only.
#[derive(Deserialize)]
struct Array {
    wearables: Vec<Wearable>,
}

static COLLECTIONS: LazyLock<Vec<Collection>> = LazyLock::new(|| {
    let symbols: HashMap<String, String> =
        serde_json::from_str(SYMBOLS_JSON).expect("vendored collections-v1-symbols.json");
    serde_json::from_str::<Vec<Collection>>(COLLECTIONS_JSON)
        .expect("vendored collections-v1.json")
        .into_iter()
        .map(|c| {
            let address = c.address.to_ascii_lowercase();
            Collection {
                symbol: symbols.get(&address).cloned().unwrap_or_default(),
                address,
                ..c
            }
        })
        .collect()
});

/// Each wearable id's first representation, in the subgraph's search order.
static REPRESENTATIONS: LazyLock<HashMap<String, Wearable>> = LazyLock::new(|| {
    let arrays: Vec<Array> =
        serde_json::from_str(WEARABLES_JSON).expect("vendored wearables-v1.json");
    let mut first = HashMap::new();
    for wearable in arrays.into_iter().flat_map(|a| a.wearables) {
        first.entry(wearable.id.clone()).or_insert(wearable);
    }
    first
});

/// The 42 v1 collections.
pub fn collections() -> &'static [Collection] {
    &COLLECTIONS
}

/// The v1 collection at `address` (any case).
pub fn collection(address: &str) -> Option<&'static Collection> {
    let address = address.to_ascii_lowercase();
    COLLECTIONS.iter().find(|c| c.address == address)
}

/// The representation the subgraph gives wearable `id`: the first with that
/// id in its search order, whichever collection's array holds it; `None`
/// when no array holds it.
pub fn representation(id: &str) -> Option<&'static Wearable> {
    REPRESENTATIONS.get(id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_vendored_catalogue_reads_whole() {
        let collections = collections();
        assert_eq!(collections.len(), 42);
        assert!(collections.iter().all(|c| c.address.len() == 42
            && c.address.starts_with("0x")
            && c.name.starts_with("dcl://")
            && c.symbol.starts_with("DCL")));
        assert_eq!(
            collection("0xC1F4B0EEA2BD6690930E6C66EFD3E197D620B9C2").map(Collection::urn_name),
            Some("halloween_2019")
        );
        assert_eq!(REPRESENTATIONS.len(), 445);
        let bee = representation("bee_suit_earring").unwrap();
        assert_eq!(
            (
                bee.name.as_str(),
                bee.category.as_str(),
                bee.rarity.as_str()
            ),
            ("Bee Earrings", "earring", "epic")
        );
        assert_eq!(bee.body_shapes, ["BaseFemale"]);
        assert_eq!(representation("no_such_wearable"), None);
    }
}
