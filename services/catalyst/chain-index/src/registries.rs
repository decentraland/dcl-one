//! The LAND and estate registries per Ethereum network: what an
//! authorization's `tokenAddress` is, where the land-authz index starts and
//! the `index_cursor.network` row it keeps. `land_authz` re-exports these.

pub const LAND_REGISTRY_MAINNET: &str = "0xf87e31492faf9a91b02ee0deaad50d51d56d5d4d";
pub const ESTATE_REGISTRY_MAINNET: &str = "0x959e104e1a4db6317fa58f8295f586e1a978c297";
pub const LAND_REGISTRY_SEPOLIA: &str = "0x42f4ba48791e2de32f5fbf553441c2672864bb33";
pub const ESTATE_REGISTRY_SEPOLIA: &str = "0x369a7fbe718c870c79f99fb423882e8dd8b20486";

/// The block the LAND registry was deployed in: where a full replay starts.
pub const LAND_START_BLOCK_MAINNET: u64 = 4_944_642;
/// The Sepolia registries' deployment block is not recorded in the old
/// crate; a replay from genesis costs empty ranges only (the endpoint's
/// range hints bound them), and `LAND_AUTHZ_START_BLOCK` overrides it.
pub const LAND_START_BLOCK_SEPOLIA: u64 = 0;

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
    match eth_network.trim().to_ascii_lowercase().as_str() {
        "mainnet" | "ethereum" | "ethereum-mainnet" => Some(LAND_MAINNET),
        "sepolia" | "ethereum-sepolia" => Some(LAND_SEPOLIA),
        _ => None,
    }
}
