//! Byte packing and digest math shared by the hand-rolled EIP-712 encoders of
//! this module (the meta-transaction digest, the off-chain marketplace trade).
//! Type strings, field order and input parsing stay at the call site: they
//! are per-contract wire facts.

use alloy_primitives::{Address, U256, keccak256};

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
}
