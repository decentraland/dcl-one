//! The EIP-712 byte packing and digest math every hand-rolled encoder in
//! this crate shares (the rentals listing, the meta-transaction digest, the
//! off-chain trade, the Snapshot proposal). Type strings, field order and
//! input parsing stay at each call site: they are per-contract wire facts.

use alloy_primitives::{Address, U256, keccak256};

const DOMAIN_TYPE_CHAIN_CONTRACT: &str =
    "EIP712Domain(string name,string version,uint256 chainId,address verifyingContract)";
const DOMAIN_TYPE_CONTRACT_SALT: &str =
    "EIP712Domain(string name,string version,address verifyingContract,bytes32 salt)";

pub fn word_address(address: Address) -> [u8; 32] {
    let mut word = [0u8; 32];
    word[12..].copy_from_slice(address.as_slice());
    word
}

pub fn word_u256(value: U256) -> [u8; 32] {
    value.to_be_bytes::<32>()
}

pub fn word_u64(value: u64) -> [u8; 32] {
    U256::from(value).to_be_bytes::<32>()
}

pub fn hash_dynamic(bytes: &[u8]) -> [u8; 32] {
    keccak256(bytes).0
}

pub fn hash_array_of_structs(members: &[[u8; 32]]) -> [u8; 32] {
    let mut cat = Vec::with_capacity(members.len() * 32);
    for member in members {
        cat.extend_from_slice(member);
    }
    keccak256(&cat).0
}

pub fn struct_hash(type_hash: [u8; 32], fields: &[[u8; 32]]) -> [u8; 32] {
    let mut enc = Vec::with_capacity((fields.len() + 1) * 32);
    enc.extend_from_slice(&type_hash);
    for field in fields {
        enc.extend_from_slice(field);
    }
    keccak256(&enc).0
}

/// The standard domain: `name`, `version`, `chainId`, `verifyingContract`.
pub fn domain_separator(
    name: &str,
    version: &str,
    chain_id: u64,
    verifying_contract: Address,
) -> [u8; 32] {
    struct_hash(
        hash_dynamic(DOMAIN_TYPE_CHAIN_CONTRACT.as_bytes()),
        &[
            hash_dynamic(name.as_bytes()),
            hash_dynamic(version.as_bytes()),
            word_u64(chain_id),
            word_address(verifying_contract),
        ],
    )
}

/// The Decentraland Polygon contracts' domain: `salt` carries the chain id.
pub fn domain_separator_salted(
    name: &str,
    version: &str,
    verifying_contract: Address,
    salt: [u8; 32],
) -> [u8; 32] {
    struct_hash(
        hash_dynamic(DOMAIN_TYPE_CONTRACT_SALT.as_bytes()),
        &[
            hash_dynamic(name.as_bytes()),
            hash_dynamic(version.as_bytes()),
            word_address(verifying_contract),
            salt,
        ],
    )
}

pub fn typed_data_digest(domain_separator: [u8; 32], struct_hash: [u8; 32]) -> [u8; 32] {
    let mut msg = Vec::with_capacity(2 + 64);
    msg.extend_from_slice(&[0x19, 0x01]);
    msg.extend_from_slice(&domain_separator);
    msg.extend_from_slice(&struct_hash);
    keccak256(&msg).0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn words_are_left_padded_big_endian() {
        let a: Address = "0x1111111111111111111111111111111111111111"
            .parse()
            .unwrap();
        let w = word_address(a);
        assert_eq!(&w[..12], &[0u8; 12]);
        assert_eq!(&w[12..], a.as_slice());
        assert_eq!(word_u64(1)[31], 1);
        assert_eq!(word_u256(U256::from(256u64))[30], 1);
    }

    #[test]
    fn digest_is_1901_domain_struct() {
        let d = [0x11u8; 32];
        let s = [0x22u8; 32];
        let mut msg = vec![0x19, 0x01];
        msg.extend_from_slice(&d);
        msg.extend_from_slice(&s);
        assert_eq!(typed_data_digest(d, s), keccak256(&msg).0);
        assert_eq!(hash_array_of_structs(&[]), keccak256([]).0);
    }

    /// The EIP-712 example domain: hashStruct(EIP712Domain) for name "Ether
    /// Mail", version "1", chain 1, verifyingContract 0xCcCC...cccC.
    #[test]
    fn domain_separator_is_the_standard_chain_contract_form() {
        let sep = domain_separator(
            "Ether Mail",
            "1",
            1,
            "0xCcCCccccCCCCcCCCCCCcCcCccCcCCCcCcccccccC"
                .parse()
                .unwrap(),
        );
        assert_eq!(
            hex::encode(sep),
            "f2cee375fa42b42143804025fc449deafd50cc031ca257e0b194a650a912090f"
        );
    }

    #[test]
    fn the_salted_domain_binds_the_salt_instead_of_the_chain_id() {
        let contract: Address = "0xCcCCccccCCCCcCCCCCCcCcCccCcCCCcCcccccccC"
            .parse()
            .unwrap();
        let salted = domain_separator_salted("Decentraland", "1", contract, word_u64(137));
        assert_ne!(salted, domain_separator("Decentraland", "1", 137, contract));
        assert_ne!(
            salted,
            domain_separator_salted("Decentraland", "1", contract, word_u64(80002))
        );
    }
}
