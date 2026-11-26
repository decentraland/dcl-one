/// The Ethereum and Polygon pair selected by `ETH_NETWORK`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChainPair {
    Mainnet,
    Testnet,
}

impl ChainPair {
    pub fn from_eth_network(network: &str) -> Option<Self> {
        let network = network.trim();
        if ["mainnet", "ethereum", "ethereum-mainnet"]
            .iter()
            .any(|name| network.eq_ignore_ascii_case(name))
        {
            Some(Self::Mainnet)
        } else if ["sepolia", "ethereum-sepolia"]
            .iter()
            .any(|name| network.eq_ignore_ascii_case(name))
        {
            Some(Self::Testnet)
        } else {
            None
        }
    }

    pub fn names(self) -> (&'static str, &'static str) {
        match self {
            Self::Mainnet => ("mainnet", "polygon"),
            Self::Testnet => ("sepolia", "amoy"),
        }
    }

    pub fn chain_ids(self) -> (i64, i64) {
        match self {
            Self::Mainnet => (1, 137),
            Self::Testnet => (11_155_111, 80_002),
        }
    }
}
