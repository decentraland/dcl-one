// from: shop@94584a5c:app/src/lib/marketplace.ts and marketplace@f3956826:webapp/src/modules/vendor/decentraland/contracts.ts (which contract a purchase settles on, per chain), decentraland-transactions 3.3.0 (the addresses and EIP-712 domains, as ui3/src/shop/market/lib/contracts.ts and ui3/src/shop/land/contracts.ts carry them), docs/shop/CONTRACTS.md ("Addresses")
//! The settlement venues of a chain: where a MANA purchase, listing, bid,
//! rental or NAME registration executes and what it pays with. One book
//! per process ([`Book::production`]); a controlled deployment (the anvil
//! tests) builds its own with [`Book::with_chain`], so no route ever reads
//! an address from a request.

use std::collections::HashMap;

use alloy_primitives::{Address, address};

use crate::market::chain_walk::ChainNet;
use crate::market::listings;
use crate::types::{ChainId, ContractRef, EvmAddress};

/// An off-chain marketplace deployment and the EIP-712 domain trades on it
/// are signed against.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Marketplace {
    pub address: Address,
    pub name: String,
    pub version: String,
    /// V3 keys a cancellation on the trade's EIP-712 digest; V1/V2 on
    /// keccak256 of the signature.
    pub cancels_by_digest: bool,
}

/// The contracts of one chain.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Chain {
    pub chain_id: ChainId,
    /// The index tables' spelling (`ethereum` | `matic`), when the chain is
    /// one the indexes walk.
    pub net: Option<ChainNet>,
    pub mana: Address,
    /// Newest first.
    pub marketplaces: Vec<Marketplace>,
    /// The legacy order book (`executeOrder`/`cancelOrder`).
    pub legacy_marketplace: Option<Address>,
    pub rentals: Option<Address>,
    pub land: Option<Address>,
    pub estate: Option<Address>,
    pub name_registrar: Option<Address>,
    pub name_controller: Option<Address>,
    /// The JSON-RPC endpoint the quotes and the tracker read this chain
    /// through; None makes every chain read answer "unavailable".
    pub rpc_url: Option<String>,
    /// Blocks a settlement stays behind the head before the tracker calls it
    /// final without an index (`ChainNet::confirmations` for the two
    /// production chains).
    pub confirmations: u64,
}

impl Chain {
    pub fn marketplace(&self, address: Address) -> Option<&Marketplace> {
        self.marketplaces.iter().find(|m| m.address == address)
    }

    /// Where a new listing or bid is signed (`getLatestOffChainMarketplaceContract`).
    pub fn latest_marketplace(&self) -> Option<&Marketplace> {
        self.marketplaces.first()
    }

    pub fn contract(&self, address: Address) -> ContractRef {
        ContractRef {
            chain_id: self.chain_id,
            contract: EvmAddress::new(address),
        }
    }

    pub fn is_land_registry(&self, address: Address) -> bool {
        self.land == Some(address)
    }

    pub fn is_estate_registry(&self, address: Address) -> bool {
        self.estate == Some(address)
    }
}

#[derive(Debug, Clone, Default)]
pub struct Book {
    chains: HashMap<u64, Chain>,
}

fn marketplace(m: listings::OffChainMarketplace) -> Marketplace {
    Marketplace {
        address: m.address.parse().expect("a pinned marketplace address"),
        name: m.name.to_string(),
        version: m.version.to_string(),
        cancels_by_digest: m.cancels_by_digest,
    }
}

impl Book {
    /// The production book over the pinned addresses; `rpc` gives each
    /// chain's endpoint (`MarketplaceState::chain_rpc_url`).
    pub fn production(rpc: impl Fn(u64) -> Option<String>) -> Self {
        let mut chains = HashMap::new();
        let ethereum = Chain {
            chain_id: ChainId::ETHEREUM,
            net: Some(ChainNet::Ethereum),
            mana: address!("0x0f5d2fb29fb7d3cfee444a200298f468908cc942"),
            marketplaces: [
                listings::offchain_marketplace_v3(1),
                listings::offchain_marketplace_v2(1),
                Some(listings::OffChainMarketplace {
                    name: "DecentralandMarketplaceEthereum",
                    version: "1.0.0",
                    address: "0x2d6b3508f9aca32d2550f92b2addba932e73c1ff",
                    cancels_by_digest: false,
                }),
            ]
            .into_iter()
            .flatten()
            .map(marketplace)
            .collect(),
            legacy_marketplace: Some(address!("0x8e5660b4ab70168b5a6feea0e0315cb49c8cd539")),
            rentals: Some(address!("0x3a1469499d0be105d4f77045ca403a5f6dc2f3f5")),
            land: Some(address!("0xf87e31492faf9a91b02ee0deaad50d51d56d5d4d")),
            estate: Some(address!("0x959e104e1a4db6317fa58f8295f586e1a978c297")),
            name_registrar: Some(address!("0x2a187453064356c898cae034eaed119e1663acb8")),
            name_controller: Some(address!("0xbe92b49aee993adea3a002adcda189a2b7dec56c")),
            rpc_url: rpc(1),
            confirmations: ChainNet::Ethereum.confirmations(),
        };
        let polygon = Chain {
            chain_id: ChainId::POLYGON,
            net: Some(ChainNet::Matic),
            mana: address!("0xa1c57f48f0deb89f569dfbe6e2b7f46d33606fd4"),
            marketplaces: [
                listings::offchain_marketplace_v3(137),
                listings::offchain_marketplace_v2(137),
                Some(listings::OffChainMarketplace {
                    name: "DecentralandMarketplacePolygon",
                    version: "1.0.0",
                    address: "0x540fb08edb56aae562864b390542c97f562825ba",
                    cancels_by_digest: false,
                }),
            ]
            .into_iter()
            .flatten()
            .map(marketplace)
            .collect(),
            legacy_marketplace: Some(address!("0x480a0f4e360e8964e68858dd231c2922f1df45ef")),
            rentals: None,
            land: None,
            estate: None,
            name_registrar: None,
            name_controller: None,
            rpc_url: rpc(137),
            confirmations: ChainNet::Matic.confirmations(),
        };
        chains.insert(1, ethereum);
        chains.insert(137, polygon);
        Self { chains }
    }

    /// A book of one controlled chain (the anvil tests).
    pub fn with_chain(mut self, chain: Chain) -> Self {
        self.chains.insert(chain.chain_id.0, chain);
        self
    }

    pub fn chain(&self, chain_id: ChainId) -> Option<&Chain> {
        self.chains.get(&chain_id.0)
    }

    /// The chain a network spelling names (`ethereum` | `matic`).
    pub fn chain_of_net(&self, net: ChainNet) -> Option<&Chain> {
        self.chains.values().find(|c| c.net == Some(net))
    }

    pub fn chains(&self) -> impl Iterator<Item = &Chain> {
        self.chains.values()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_production_book_names_the_pinned_venues() {
        let book = Book::production(|_| None);
        let polygon = book.chain(ChainId::POLYGON).unwrap();
        assert_eq!(
            polygon
                .marketplaces
                .iter()
                .map(|m| format!("{:#x}", m.address))
                .collect::<Vec<_>>(),
            [
                "0xe38ef22abe871513555cba89adfe45ab4f548ada",
                "0xa40b1d129b8906888720686f3a01921ddf37716f",
                "0x540fb08edb56aae562864b390542c97f562825ba"
            ]
        );
        assert!(
            polygon.marketplaces[0].cancels_by_digest && !polygon.marketplaces[2].cancels_by_digest
        );
        assert_eq!(
            polygon.latest_marketplace().unwrap().name,
            "DecentralandMarketplacePolygon"
        );
        let ethereum = book.chain(ChainId::ETHEREUM).unwrap();
        assert_eq!(
            format!("{:#x}", ethereum.rentals.unwrap()),
            crate::rentals::index::RENTALS_MAINNET
        );
        assert_eq!(
            format!("{:#x}", ethereum.land.unwrap()),
            crate::land_authz::LAND_REGISTRY_MAINNET
        );
        assert_eq!(
            format!("{:#x}", ethereum.legacy_marketplace.unwrap()),
            crate::market::nft_index::legacy_marketplaces(ChainNet::Ethereum)[0].0
        );
        assert!(
            book.chain(ChainId::AMOY).is_none(),
            "no test chain in the production book"
        );
        assert_eq!(
            book.chain_of_net(ChainNet::Matic).unwrap().chain_id,
            ChainId::POLYGON
        );
    }

    #[test]
    fn every_marketplace_the_trade_events_index_walks_is_in_the_book() {
        let book = Book::production(|_| None);
        for net in ChainNet::ALL {
            let chain = book.chain_of_net(net).unwrap();
            for (address, _, v3) in crate::market::trade_events::marketplaces(net) {
                let m = chain
                    .marketplace(address.parse().unwrap())
                    .unwrap_or_else(|| panic!("{address} on {net:?}"));
                assert_eq!(m.cancels_by_digest, *v3, "{address}");
            }
        }
    }
}
