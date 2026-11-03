//! Item pointers that name one collections-v1 asset by contract and by
//! collection name ([`super::onchain::v1_collection_contract`]) count as one
//! pointer for a synced item, as the reference access check resolves them.

use alloy_primitives::Address;

/// `(contract, item)` for a mainnet collections-v1 asset URN, by address or
/// by a listed name; `None` for anything else.
fn v1_asset_key(pointer: &str) -> Option<(Address, &str)> {
    let parts: Vec<&str> = pointer.split(':').collect();
    let [
        "urn",
        "decentraland",
        "ethereum",
        "collections-v1",
        collection,
        item,
    ] = parts[..]
    else {
        return None;
    };
    let contract = if collection.starts_with("0x") {
        collection.parse().ok()?
    } else {
        super::onchain::v1_collection_contract(collection)?
    };
    Some((contract, item))
}

/// The first of `pointers` when every one names the same collections-v1
/// asset; `None` otherwise (including a single pointer).
pub(super) fn single_item_pointer(pointers: &[String]) -> Option<String> {
    let (first, rest) = pointers.split_first()?;
    if rest.is_empty() {
        return None;
    }
    let key = v1_asset_key(first)?;
    rest.iter()
        .all(|p| v1_asset_key(p) == Some(key))
        .then(|| first.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pointers(list: &[&str]) -> Vec<String> {
        list.iter().map(|p| p.to_string()).collect()
    }

    #[test]
    fn a_v1_asset_named_by_address_and_by_name_is_one_pointer() {
        let both = pointers(&[
            "urn:decentraland:ethereum:collections-v1:halloween_2019:hat",
            "urn:decentraland:ethereum:collections-v1:0xc1f4b0eea2bd6690930e6c66efd3e197d620b9c2:hat",
        ]);
        assert_eq!(
            single_item_pointer(&both).as_deref(),
            Some(both[0].as_str())
        );
    }

    #[test]
    fn different_assets_or_unlisted_collections_stay_separate() {
        for list in [
            &[
                "urn:decentraland:ethereum:collections-v1:halloween_2019:hat",
                "urn:decentraland:ethereum:collections-v1:halloween_2019:mask",
            ][..],
            &[
                "urn:decentraland:ethereum:collections-v1:not_listed:hat",
                "urn:decentraland:ethereum:collections-v1:0xc1f4b0eea2bd6690930e6c66efd3e197d620b9c2:hat",
            ],
            &[
                "urn:decentraland:matic:collections-v2:0xabc:0",
                "urn:decentraland:matic:collections-v2:0xabc:1",
            ],
            &["urn:decentraland:ethereum:collections-v1:halloween_2019:hat"],
        ] {
            assert_eq!(single_item_pointer(&pointers(list)), None, "{list:?}");
        }
    }
}
