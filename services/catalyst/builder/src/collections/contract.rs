//! The address a standard collection will have on Polygon, computed the way
//! builder-server's `FactoryCollection` computes it: the salt is the
//! keccak-256 of the collection id, and the address is the collection
//! factory's CREATE2 of the keccak-256 of the salt, the forwarder and the
//! collection's `initialize` calldata, over the factory's proxy code hash.
//! The calldata is the one the client will deploy with; it must name the
//! signer as the collection's creator.

use alloy_primitives::{Address, B256, keccak256};
use alloy_sol_types::{SolCall, sol};

use crate::publication::{COLLECTION_FACTORY, COLLECTION_FORWARDER};

/// The keccak-256 of the collection factory's proxy init code on Polygon.
pub const FACTORY_CODE_HASH: &str =
    "0x5a1d707e8f0be7be88213a8216231468689b96dcd4abed0931276f4886a87beb";

sol! {
    struct InitializeItem { string rarity; uint256 price; address beneficiary; string metadata; }
    function initialize(string name, string symbol, string baseURI, address creator, bool shouldComplete, bool isApproved, address rarities, InitializeItem[] items);
}

/// `FactoryCollection.getSalt(id)`.
pub fn salt(id: &str) -> B256 {
    keccak256(id.as_bytes())
}

/// `FactoryCollection.getContractAddress(salt, data)`, lowercase.
pub fn address(salt: B256, data: &[u8]) -> String {
    let factory: Address = COLLECTION_FACTORY.parse().unwrap_or_default();
    let forwarder: Address = COLLECTION_FORWARDER.parse().unwrap_or_default();
    let code_hash: B256 = FACTORY_CODE_HASH.parse().unwrap_or_default();
    let inner = keccak256([salt.as_slice(), forwarder.as_slice(), data].concat());
    let outer = keccak256(
        [
            &[0xff][..],
            factory.as_slice(),
            inner.as_slice(),
            code_hash.as_slice(),
        ]
        .concat(),
    );
    format!("{:#x}", Address::from_slice(&outer[12..]))
}

/// The bytes of `data` when it is `initialize` calldata naming `creator`, in
/// its canonical ABI encoding.
pub fn initialize_for(data: &str, creator: &str) -> Option<Vec<u8>> {
    let bytes = alloy_primitives::hex::decode(data.strip_prefix("0x")?).ok()?;
    let call = initializeCall::abi_decode(&bytes).ok()?;
    let creator: Address = creator.parse().ok()?;
    (call.creator == creator && call.abi_encode() == bytes).then_some(bytes)
}

/// `initialize` calldata naming `creator`, with no items.
#[cfg(test)]
pub(crate) fn initialize_calldata(creator: &str) -> String {
    initialize_calldata_named(creator, "Test")
}

/// [`initialize_calldata`] for a collection called `name`.
#[cfg(test)]
pub(crate) fn initialize_calldata_named(creator: &str, name: &str) -> String {
    let call = initializeCall {
        name: name.into(),
        symbol: "DCL-TST".into(),
        baseURI: String::new(),
        creator: creator.parse().unwrap(),
        shouldComplete: true,
        isApproved: false,
        rarities: Address::ZERO,
        items: vec![],
    };
    format!("0x{}", alloy_primitives::hex::encode(call.abi_encode()))
}

#[cfg(test)]
mod tests {
    use super::*;

    const OWNER: &str = "0xc1aba94a79ca07d9d15a62ff5c9b382777e0c70e";

    #[test]
    fn the_salt_and_address_are_builder_servers() {
        let salt = salt("c011ec71-0000-4000-8000-000000000002");
        assert_eq!(
            format!("{salt:#x}"),
            "0xda8033e38e37a4cf3bd9f0a4d697db91d8f1c927fadb3220a8cc71bc2a539eb8"
        );
        assert_eq!(
            address(salt, &[0]),
            "0x969194d18107025936324ddc6b1019a6755c94d5"
        );
    }

    #[test]
    fn calldata_no_deployment_could_carry_is_refused() {
        let canonical = initialize_calldata(OWNER);
        let bytes = alloy_primitives::hex::decode(&canonical[2..]).unwrap();
        assert!(initialize_for(&canonical, OWNER).is_some());
        let word = |i: usize| 4 + 32 * i;
        let name_at =
            4 + usize::try_from(alloy_primitives::U256::from_be_slice(&bytes[4..36])).unwrap();
        let mut cases: Vec<(&str, Vec<u8>)> = Vec::new();
        let mut trailing = bytes.clone();
        trailing.extend([0u8; 32]);
        cases.push(("trailing bytes", trailing));
        let mut creator = bytes.clone();
        creator[word(3)] = 1;
        cases.push(("dirty high bits on the creator", creator));
        let mut rarities = bytes.clone();
        rarities[word(6) + 5] = 0xff;
        cases.push(("dirty high bits on the rarities address", rarities));
        let mut flag = bytes.clone();
        flag[word(4) + 31] = 2;
        cases.push(("a bool that is neither 0 nor 1", flag));
        let mut into_head = bytes.clone();
        into_head[4..36].copy_from_slice(&alloy_primitives::U256::from(32).to_be_bytes::<32>());
        cases.push(("a name offset into the head", into_head));
        let mut padding = bytes.clone();
        padding[name_at + 32 + 31] = 1;
        cases.push(("non-zero padding after the name", padding));
        let accepted: Vec<&str> = cases
            .iter()
            .filter(|(_, malformed)| {
                let hex = format!("0x{}", alloy_primitives::hex::encode(malformed));
                initialize_for(&hex, OWNER).is_some()
            })
            .map(|(case, _)| *case)
            .collect();
        assert!(accepted.is_empty(), "accepted: {accepted:?}");
    }

    #[test]
    fn initialize_calldata_is_accepted_only_for_its_creator() {
        let data = "0xd14fee77000000000000000000000000000000000000000000000000000000000000010000000000000000000000000000000000000000000000000000000000000001400000000000000000000000000000000000000000000000000000000000000180000000000000000000000000c1aba94a79ca07d9d15a62ff5c9b382777e0c70e0000000000000000000000000000000000000000000000000000000000000001000000000000000000000000000000000000000000000000000000000000000000000000000000000000000017113b44fdd661a156cc01b5031e3acf72c32eb30000000000000000000000000000000000000000000000000000000000000200000000000000000000000000000000000000000000000000000000000000000b566563746f722068617473000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000b44434c2d56435452485453000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000004668747470733a2f2f706565722e646563656e7472616c616e642e6f72672f6c616d626461732f636f6c6c656374696f6e732f7374616e646172642f6572633732312f3133372f00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000001000000000000000000000000000000000000000000000000000000000000002000000000000000000000000000000000000000000000000000000000000000800000000000000000000000000000000000000000000000000de0b6b3a7640000000000000000000000000000c1aba94a79ca07d9d15a62ff5c9b382777e0c70e00000000000000000000000000000000000000000000000000000000000000c00000000000000000000000000000000000000000000000000000000000000006636f6d6d6f6e0000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000001c313a773a566563746f72206861743a3a6861743a426173654d616c6500000000";
        let bytes = initialize_for(data, OWNER).expect("the owner's calldata");
        assert_eq!(
            address(salt("c011ec71-0000-4000-8000-0000000000f1"), &bytes),
            "0xe237c5ffa36d5432a8cfdf53960bcf61ce4cdfc5"
        );
        assert!(initialize_for(data, "0xcd6884d4258edc833f31ac773e24affe6a7d3154").is_none());
        for refused in ["0x00", "", "d14fee77", "0xzz", &data[..data.len() - 64]] {
            assert!(initialize_for(refused, OWNER).is_none(), "{refused}");
        }
    }
}
