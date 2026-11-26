//! Legacy contracts and first blocks from decentraland/marketplace-squid-core
//! at 85c6832e96e5459b37f0f315b8e384693684e12d (src/eth/{addresses,data/contracts}
//! and src/polygon/addresses). The Amoy legacy Marketplace filter follows
//! decentraland/collections-graph at ab6c58670d94bfbe774a0f56dadef1b541c53e8b,
//! config/amoy.json, including the older address from its manifest start.

use super::{ChainNet, DCL_REGISTRAR};
use crate::land_authz::{LAND_MAINNET, LAND_SEPOLIA, LandNetwork};

pub(super) const REGISTRAR_SEPOLIA: &str = "0x7518456ae93eb98f3e64571b689c626616bb7f30";

#[derive(Clone, Copy)]
pub(crate) struct Deployments {
    pub chain_id: i64,
    pub start: u64,
    pub land: Option<LandNetwork>,
    pub markets: &'static [(&'static str, u64)],
    registrar: Option<(&'static str, u64)>,
}

impl Deployments {
    pub fn mainnet(net: ChainNet) -> Self {
        match net {
            ChainNet::Ethereum => Self {
                chain_id: net.chain_id(),
                start: LAND_MAINNET.start_block,
                land: Some(LAND_MAINNET),
                markets: &[("0x8e5660b4ab70168b5a6feea0e0315cb49c8cd539", 6_496_012)],
                registrar: Some((DCL_REGISTRAR, 9_412_979)),
            },
            ChainNet::Matic => Self {
                chain_id: net.chain_id(),
                start: 15_202_000,
                land: None,
                markets: &[
                    ("0x02080031b45a3c67d338dd4a2cc309d28756a160", 15_202_565),
                    ("0x480a0f4e360e8964e68858dd231c2922f1df45ef", 22_513_570),
                ],
                registrar: None,
            },
        }
    }

    pub fn for_chain(net: ChainNet, chain_id: i64) -> Option<Self> {
        Some(match (net, chain_id) {
            (_, id) if id == net.chain_id() => Self::mainnet(net),
            (ChainNet::Ethereum, id) if id == net.testnet_chain_id() => Self {
                chain_id,
                start: LAND_SEPOLIA.start_block,
                land: Some(LAND_SEPOLIA),
                markets: &[("0xccf0c17da6cd68041b1bf0f7e015767242077d8c", 3_831_225)],
                registrar: Some((REGISTRAR_SEPOLIA, 3_831_239)),
            },
            (ChainNet::Matic, id) if id == net.testnet_chain_id() => Self {
                chain_id,
                start: 5_706_656,
                land: None,
                markets: &[
                    ("0x0c8ad1f6aadf89d2eb19f01a100a6143108fe2b0", 5_706_656),
                    ("0x2a09a000224f5dbe0e17214ca95cde506da7cb74", 14_517_370),
                ],
                registrar: None,
            },
            _ => return None,
        })
    }

    pub fn fixed_contracts(self) -> Vec<(&'static str, u64)> {
        self.markets.iter().copied().chain(self.registrar).collect()
    }

    /// The names registrar the chain's walk takes in, if any.
    pub fn registrar(self) -> Option<&'static str> {
        self.registrar.map(|(address, _)| address)
    }
}
