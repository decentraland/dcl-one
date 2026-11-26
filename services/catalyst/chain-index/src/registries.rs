//! The LAND and estate registries per Ethereum network: what an
//! authorization's `tokenAddress` is, where the land-authz index starts and
//! the `index_cursor.network` row it keeps. `land_authz` re-exports these.

use crate::shared::network::ChainPair;

pub const LAND_REGISTRY_MAINNET: &str = "0xf87e31492faf9a91b02ee0deaad50d51d56d5d4d";
pub const ESTATE_REGISTRY_MAINNET: &str = "0x959e104e1a4db6317fa58f8295f586e1a978c297";
pub const LAND_REGISTRY_SEPOLIA: &str = "0x42f4ba48791e2de32f5fbf553441c2672864bb33";
pub const ESTATE_REGISTRY_SEPOLIA: &str = "0x369a7fbe718c870c79f99fb423882e8dd8b20486";

/// The block the LAND registry was deployed in: where a full replay starts.
pub const LAND_START_BLOCK_MAINNET: u64 = 4_944_642;
/// First Sepolia LAND proxy block, from marketplace-squid-core
/// 85c6832e96e5459b37f0f315b8e384693684e12d, src/eth/data/contracts/start-blocks.ts.
pub const LAND_START_BLOCK_SEPOLIA: u64 = 3_831_219;

/// `index_cursor.network` values.
pub const NETWORK_MAINNET: &str = "ethereum-mainnet";
pub const NETWORK_SEPOLIA: &str = "ethereum-sepolia";

/// One network's registries and the block its index starts from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LandNetwork {
    pub network: &'static str,
    pub land_registry: &'static str,
    pub estate_registry: &'static str,
    pub start_block: u64,
}

/// The mainnet registries.
pub const LAND_MAINNET: LandNetwork = LandNetwork {
    network: NETWORK_MAINNET,
    land_registry: LAND_REGISTRY_MAINNET,
    estate_registry: ESTATE_REGISTRY_MAINNET,
    start_block: LAND_START_BLOCK_MAINNET,
};

/// The Sepolia registries.
pub const LAND_SEPOLIA: LandNetwork = LandNetwork {
    network: NETWORK_SEPOLIA,
    land_registry: LAND_REGISTRY_SEPOLIA,
    estate_registry: ESTATE_REGISTRY_SEPOLIA,
    start_block: LAND_START_BLOCK_SEPOLIA,
};

/// The registries of `eth_network` (`mainnet` | `sepolia`, the
/// `ETH_NETWORK` spellings); `None` for another network.
pub fn land_network(eth_network: &str) -> Option<LandNetwork> {
    ChainPair::from_eth_network(eth_network).map(|pair| match pair {
        ChainPair::Mainnet => LAND_MAINNET,
        ChainPair::Testnet => LAND_SEPOLIA,
    })
}

/// The network whose LAND or estate registry is `address` (any case);
/// `None` for another contract.
pub fn land_network_of_registry(address: &str) -> Option<LandNetwork> {
    [LAND_MAINNET, LAND_SEPOLIA].into_iter().find(|net| {
        address.eq_ignore_ascii_case(net.land_registry)
            || address.eq_ignore_ascii_case(net.estate_registry)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_registry_names_its_network_in_any_case() {
        for (address, net) in [
            (LAND_REGISTRY_MAINNET, LAND_MAINNET),
            (ESTATE_REGISTRY_MAINNET, LAND_MAINNET),
            (LAND_REGISTRY_SEPOLIA, LAND_SEPOLIA),
            (ESTATE_REGISTRY_SEPOLIA, LAND_SEPOLIA),
        ] {
            assert_eq!(land_network_of_registry(address), Some(net));
            assert_eq!(
                land_network_of_registry(&address.to_ascii_uppercase().replace("0X", "0x")),
                Some(net)
            );
        }
        assert_eq!(
            land_network_of_registry("0x2a187453064356c898cae034eaed119e1663acb8"),
            None
        );
    }
}
