//! The Polygon chain the builder works on: the one `ETH_NETWORK` pairs
//! (mainnet with Polygon, sepolia with Amoy), as builder-server's
//! `CHAIN_NAME` picks it (`getMappedChainIdForCurrentChainName`). Its
//! contracts are `decentraland-transactions`' for that chain, the factory's
//! proxy code hash builder-server's own (`getFactoryCollectionCodeHash`),
//! and the base URI the creators app deploys with
//! (`ERC721_COLLECTION_BASE_URI`).

/// One Polygon chain's collection contracts and names.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Network {
    /// The `eth_chainId` its RPC answers.
    pub chain_id: u64,
    /// The network segment of its collection and item URNs.
    pub urn_protocol: &'static str,
    pub collection_manager: &'static str,
    pub collection_factory: &'static str,
    pub forwarder: &'static str,
    /// The keccak-256 of the factory's proxy init code.
    pub factory_code_hash: &'static str,
    pub rarities_with_oracle: &'static str,
    /// The slot cheques' verifying contract.
    pub third_party_registry: &'static str,
    pub base_uri: &'static str,
    /// Its collections subgraph on Decentraland's subgraph worker.
    pub collections_subgraph: &'static str,
    /// Its third-party registry subgraph on Decentraland's subgraph worker.
    pub registry_subgraph: &'static str,
}

/// Polygon mainnet.
pub const POLYGON: Network = Network {
    chain_id: 137,
    urn_protocol: "matic",
    collection_manager: "0x9d32aac179153a991e832550d9f96441ea27763a",
    collection_factory: "0x3195e88ae10704b359764cb38e429d24f1c2f781",
    forwarder: "0xbf6755a83c0dcdbb2933a96ea778e00b717d7004",
    factory_code_hash: "0x5a1d707e8f0be7be88213a8216231468689b96dcd4abed0931276f4886a87beb",
    rarities_with_oracle: "0xa9158e22f89bb3f69c5600338895cb5fb81e5090",
    third_party_registry: "0x1c436c1efb4608dffdc8bace99d2b03c314f3348",
    base_uri: "https://peer.decentraland.org/lambdas/collections/standard/erc721/",
    collections_subgraph: "https://subgraph.decentraland.org/collections-matic-mainnet",
    registry_subgraph: "https://subgraph.decentraland.org/tpr-matic-mainnet",
};

/// Amoy, Polygon's test network.
pub const AMOY: Network = Network {
    chain_id: 80_002,
    urn_protocol: "amoy",
    collection_manager: "0x5309ae874fc4eb21adcd63f8b6c3f766cc3b1849",
    collection_factory: "0x802de0c509add2ee29de24de7225daaff4741c43",
    forwarder: "0x7b1fe9de545b22cb553766817b84d655ce8121c9",
    factory_code_hash: "0xe8aa6287567a0945907cc65108e8b18ba8cd8bff5675eb3b3d405125525ed1cf",
    rarities_with_oracle: "0x25b6b4bac4adb582a0abd475439da6730777fbf7",
    third_party_registry: "0x7d7c0b9d97385bada5fec6861e97d0df414af3c3",
    base_uri: "https://peer.decentraland.zone/lambdas/collections/standard/erc721/",
    collections_subgraph: "https://subgraph.decentraland.org/collections-matic-amoy",
    registry_subgraph: "https://subgraph.decentraland.org/tpr-matic-amoy",
};

impl Default for Network {
    fn default() -> Self {
        POLYGON
    }
}

/// The chain `eth_network` (`mainnet` | `sepolia`, the `ETH_NETWORK`
/// spellings) pairs; `None` for another network.
pub fn for_eth_network(eth_network: &str) -> Option<Network> {
    match eth_network.trim().to_ascii_lowercase().as_str() {
        "mainnet" | "ethereum" | "ethereum-mainnet" => Some(POLYGON),
        "sepolia" | "ethereum-sepolia" => Some(AMOY),
        _ => None,
    }
}

impl Network {
    /// `urn:decentraland:<protocol>:collections-v2:<contract>`.
    pub fn collection_urn(&self, contract: &str) -> String {
        format!(
            "urn:decentraland:{}:collections-v2:{contract}",
            self.urn_protocol
        )
    }

    /// `urn:decentraland:<protocol>:collections-v2:<contract>:<item>`.
    pub fn item_urn(&self, contract: &str, item: &str) -> String {
        format!("{}:{item}", self.collection_urn(contract))
    }

    /// Whether an `eth_chainId` answer (hex quantity) names this chain.
    pub fn answers(&self, chain_id: &str) -> bool {
        u64::from_str_radix(chain_id.trim_start_matches("0x"), 16).ok() == Some(self.chain_id)
    }

    /// What an RPC answering another chain is told.
    pub fn other_chain(&self, chain_id: &str) -> String {
        format!(
            "the RPC answers chain {chain_id}, not {} ({})",
            self.chain_id, self.urn_protocol
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn eth_network_pairs_its_polygon_chain() {
        for (name, net) in [
            ("mainnet", Some(POLYGON)),
            (" Mainnet ", Some(POLYGON)),
            ("sepolia", Some(AMOY)),
            ("goerli", None),
            ("polygon", None),
            ("", None),
        ] {
            assert_eq!(for_eth_network(name), net, "{name}");
        }
    }

    #[test]
    fn contracts_match_the_pinned_address_book() {
        let pinned: serde_json::Value = serde_json::from_str(include_str!(concat!(
            env!("DCL_ONE_ROOT"),
            "/services/catalyst/shop/src/evm/addresses.json"
        )))
        .unwrap();
        for (key, net) in [("matic", POLYGON), ("amoy", AMOY)] {
            let book = |name: &str| pinned[key][name].as_str().unwrap().to_ascii_lowercase();
            assert_eq!(net.collection_manager, book("CollectionManager "), "{key}");
            assert_eq!(net.collection_factory, book("CollectionFactoryV3"), "{key}");
            assert_eq!(net.forwarder, book("Forwarder"), "{key}");
            assert_eq!(
                net.rarities_with_oracle,
                book("RaritiesWithOracle"),
                "{key}"
            );
        }
    }

    #[test]
    fn a_chain_id_answer_names_one_network() {
        assert!(POLYGON.answers("0x89"));
        assert!(!POLYGON.answers("0x13882"));
        assert!(AMOY.answers("0x13882"));
        assert!(!AMOY.answers("0x89"));
        assert!(!AMOY.answers("amoy"));
    }

    #[test]
    fn urns_carry_the_network_segment() {
        assert_eq!(
            AMOY.item_urn("0xc0", "3"),
            "urn:decentraland:amoy:collections-v2:0xc0:3"
        );
        assert_eq!(
            POLYGON.collection_urn("0xc0"),
            "urn:decentraland:matic:collections-v2:0xc0"
        );
    }
}
