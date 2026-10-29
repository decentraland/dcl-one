// from: catalyrst/crates/catalyrst-economy/src/ports/meta_tx.rs, transactions-server src/logic/auditedDomains.ts
//! The EIP-712 digest of an `executeMetaTransaction` payload, rebuilt from the
//! target's reported domain separator (or the audited table for the few
//! contracts that expose none) so the signature can be checked against the
//! `userAddress` it acts for before anything is written or broadcast.

use alloy_primitives::{Address, B256, U256, keccak256};

use super::eip712::{
    domain_separator_salted, hash_dynamic, struct_hash, typed_data_digest, word_address, word_u64,
    word_u256,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MetaTxStruct {
    FunctionSignature,
    FunctionData,
}

pub fn meta_tx_type_hash(kind: MetaTxStruct) -> [u8; 32] {
    match kind {
        MetaTxStruct::FunctionSignature => {
            keccak256("MetaTransaction(uint256 nonce,address from,bytes functionSignature)").0
        }
        MetaTxStruct::FunctionData => {
            keccak256("MetaTransaction(uint256 nonce,address from,bytes functionData)").0
        }
    }
}

pub fn build_domain_separator(
    name: &str,
    version: &str,
    verifying_contract: Address,
    chain_id: u64,
) -> B256 {
    B256::from(domain_separator_salted(
        name,
        version,
        verifying_contract,
        word_u64(chain_id),
    ))
}

pub fn meta_tx_struct_hash(
    kind: MetaTxStruct,
    nonce: U256,
    user_address: Address,
    function_signature: &[u8],
) -> [u8; 32] {
    struct_hash(
        meta_tx_type_hash(kind),
        &[
            word_u256(nonce),
            word_address(user_address),
            hash_dynamic(function_signature),
        ],
    )
}

pub fn meta_tx_digest(
    domain_separator: B256,
    nonce: U256,
    user_address: Address,
    function_signature: &[u8],
    kind: MetaTxStruct,
) -> B256 {
    B256::from(typed_data_digest(
        domain_separator.0,
        meta_tx_struct_hash(kind, nonce, user_address, function_signature),
    ))
}

pub fn recover_meta_tx_signer(digest: B256, signature: &[u8]) -> Result<Address, String> {
    let recovered = crate::auth::recover_address_from_digest(
        &digest.0,
        &format!("0x{}", hex::encode(signature)),
    )
    .map_err(|e| e.to_string())?;
    recovered
        .parse::<Address>()
        .map_err(|e| format!("recovered address does not parse: {e}"))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AuditedDomain {
    pub label: &'static str,
    pub name: &'static str,
    pub version: &'static str,
    pub struct_kind: MetaTxStruct,
}

const CHAIN_MATIC_MAINNET: u64 = 137;
const CHAIN_MATIC_AMOY: u64 = 80002;

/// The EIP-712 name every off-chain marketplace version constructs; what marks
/// an audited domain as a marketplace the sale-price floor must price.
pub const OFF_CHAIN_MARKETPLACE_DOMAIN_NAME: &str = "DecentralandMarketplacePolygon";

const MARKETPLACE_V3: AuditedDomain = AuditedDomain {
    label: "MarketplaceV3",
    name: OFF_CHAIN_MARKETPLACE_DOMAIN_NAME,
    version: "1.0.0",
    struct_kind: MetaTxStruct::FunctionData,
};
const MARKETPLACE_V4: AuditedDomain = AuditedDomain {
    label: "MarketplaceV4",
    ..MARKETPLACE_V3
};
const MARKETPLACE_V5: AuditedDomain = AuditedDomain {
    label: "MarketplaceV5",
    ..MARKETPLACE_V3
};
const CREDITS: AuditedDomain = AuditedDomain {
    label: "CreditsManager",
    name: "Decentraland Credits",
    version: "1.0.0",
    struct_kind: MetaTxStruct::FunctionData,
};
const CREDITS_DEPRECATED: AuditedDomain = AuditedDomain {
    label: "CreditsManager_DEPRECATED",
    ..CREDITS
};

/// The contracts whose domain separator is not readable on-chain, with the
/// domain their audited source declares, keyed by chain as well as address
/// (MarketplaceV4 shares one address across Amoy and Ethereum).
fn audited_table(chain_id: u64) -> &'static [(&'static str, AuditedDomain)] {
    match chain_id {
        CHAIN_MATIC_MAINNET => &[
            ("0x540fb08edb56aae562864b390542c97f562825ba", MARKETPLACE_V3),
            ("0xa40b1d129b8906888720686f3a01921ddf37716f", MARKETPLACE_V4),
            ("0xe38ef22abe871513555cba89adfe45ab4f548ada", MARKETPLACE_V5),
            ("0x8b3a40ca1b6f5cafc99d112a4d02e897d1fd8cc5", CREDITS),
            (
                "0xe9f961e6ded4e1476bbee4faab886d63a2493eb9",
                CREDITS_DEPRECATED,
            ),
        ],
        CHAIN_MATIC_AMOY => &[
            ("0x6ab20ae56673ed65f520b7be332aeb61b3ed727d", MARKETPLACE_V3),
            ("0x1b67d0e31eeb6b52d8eeed71d3616c2f5b33b8e7", MARKETPLACE_V4),
            ("0x36fd1434a6c4b8ade80c9847c1d15033ce34488c", MARKETPLACE_V5),
            ("0x8052a560e6e6ac86eeb7e711a4497f639b322fb3", CREDITS),
            (
                "0x037566bc90f85e76587e1b07f9184585f09c1420",
                CREDITS_DEPRECATED,
            ),
        ],
        _ => &[],
    }
}

pub fn audited_domain_for(chain_id: u64, contract: Address) -> Option<AuditedDomain> {
    let key = format!("{contract:#x}");
    audited_table(chain_id)
        .iter()
        .find(|(addr, _)| *addr == key)
        .map(|(_, domain)| *domain)
}

/// Every audited domain on a chain with the address it is recorded under.
pub fn audited_domains_for(chain_id: u64) -> Vec<(Address, AuditedDomain)> {
    audited_table(chain_id)
        .iter()
        .filter_map(|(addr, domain)| addr.parse::<Address>().ok().map(|a| (a, *domain)))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy::signers::SignerSync;
    use alloy::signers::local::PrivateKeySigner;
    use alloy_primitives::address;

    #[test]
    fn type_hashes_match_their_type_strings() {
        assert_eq!(
            meta_tx_type_hash(MetaTxStruct::FunctionSignature),
            keccak256("MetaTransaction(uint256 nonce,address from,bytes functionSignature)").0
        );
        assert_eq!(
            meta_tx_type_hash(MetaTxStruct::FunctionData),
            keccak256("MetaTransaction(uint256 nonce,address from,bytes functionData)").0
        );
        assert_ne!(
            meta_tx_type_hash(MetaTxStruct::FunctionSignature),
            meta_tx_type_hash(MetaTxStruct::FunctionData)
        );
    }

    #[test]
    fn salt_domain_matches_the_hand_built_separator() {
        let contract = address!("0x540fb08eDb56AaE562864B390542C97F562825BA");
        let built =
            build_domain_separator("DecentralandMarketplacePolygon", "1.0.0", contract, 137);
        let expected = B256::from(domain_separator_salted(
            "DecentralandMarketplacePolygon",
            "1.0.0",
            contract,
            word_u64(137),
        ));
        assert_eq!(built, expected);
        // Pinned by the marketplace's own tests (trade.rs): the same separator.
        assert_eq!(
            hex::encode(built),
            "a509f87b8ee4b49969808cb8527697d8da141375b18ecb1277a1515f16d0e88e"
        );
    }

    #[test]
    fn a_real_key_over_the_reported_separator_recovers_to_itself() {
        let key = PrivateKeySigner::random();
        let user = key.address();
        let separator = B256::from(keccak256("some-reported-separator"));
        let nonce = U256::from(7u64);
        let function_signature = [0xaa, 0xbb, 0xcc, 0xdd];
        let digest = meta_tx_digest(
            separator,
            nonce,
            user,
            &function_signature,
            MetaTxStruct::FunctionSignature,
        );
        let sig = key.sign_hash_sync(&digest).expect("sign");
        let recovered = recover_meta_tx_signer(digest, &sig.as_bytes()).expect("recover");
        assert_eq!(recovered, user);
    }

    #[test]
    fn a_different_key_does_not_recover_to_the_claimed_user() {
        let signer = PrivateKeySigner::random();
        let victim = PrivateKeySigner::random().address();
        let separator = B256::from(keccak256("sep"));
        let digest = meta_tx_digest(
            separator,
            U256::ZERO,
            victim,
            &[0x01, 0x02],
            MetaTxStruct::FunctionSignature,
        );
        let sig = signer.sign_hash_sync(&digest).expect("sign");
        let recovered = recover_meta_tx_signer(digest, &sig.as_bytes()).expect("recover");
        assert_ne!(recovered, victim);
        assert_eq!(recovered, signer.address());
    }

    #[test]
    fn wrong_nonce_shifts_the_digest_so_the_signature_no_longer_matches() {
        let key = PrivateKeySigner::random();
        let user = key.address();
        let separator = B256::from(keccak256("sep"));
        let signed = meta_tx_digest(
            separator,
            U256::ZERO,
            user,
            &[0x9a],
            MetaTxStruct::FunctionSignature,
        );
        let sig = key.sign_hash_sync(&signed).expect("sign");
        let replayed = meta_tx_digest(
            separator,
            U256::from(1u64),
            user,
            &[0x9a],
            MetaTxStruct::FunctionSignature,
        );
        let recovered = recover_meta_tx_signer(replayed, &sig.as_bytes()).expect("recover");
        assert_ne!(recovered, user);
    }

    #[test]
    fn garbage_signature_bytes_fail_recovery() {
        let digest = B256::from(keccak256("digest"));
        let mut sig = Vec::new();
        sig.extend_from_slice(&[0x11u8; 32]);
        sig.extend_from_slice(&[0x22u8; 32]);
        sig.push(27);
        assert!(recover_meta_tx_signer(digest, &sig).is_err());
    }

    #[test]
    fn audited_table_covers_both_chains_and_rejects_unknown_targets() {
        let v3 = address!("0x540fb08eDb56AaE562864B390542C97F562825BA");
        let entry = audited_domain_for(137, v3).expect("v3 is audited on polygon");
        assert_eq!(entry.name, "DecentralandMarketplacePolygon");
        assert_eq!(entry.struct_kind, MetaTxStruct::FunctionData);
        let amoy_v4 = address!("0x1b67d0e31eeb6b52d8eeed71d3616c2f5b33b8e7");
        assert_eq!(
            audited_domain_for(80002, amoy_v4).map(|d| d.label),
            Some("MarketplaceV4")
        );
        assert!(
            audited_domain_for(137, address!("0x0000000000000000000000000000000000000009"))
                .is_none()
        );
        assert!(audited_domain_for(1, v3).is_none());
    }

    #[test]
    fn the_v5_marketplaces_carry_the_marketplace_domain_on_both_chains() {
        let mainnet_v5 =
            audited_domain_for(137, address!("0xe38ef22abe871513555cba89adfe45ab4f548ada"))
                .expect("mainnet V5");
        assert_eq!(
            mainnet_v5,
            AuditedDomain {
                label: "MarketplaceV5",
                name: "DecentralandMarketplacePolygon",
                version: "1.0.0",
                struct_kind: MetaTxStruct::FunctionData
            }
        );
        let amoy_v5 = audited_domain_for(
            80002,
            address!("0x36fd1434a6c4b8ade80c9847c1d15033ce34488c"),
        )
        .expect("amoy V5");
        assert_eq!(amoy_v5.label, "MarketplaceV5");
        assert!(
            audited_domain_for(
                80002,
                address!("0xe38ef22abe871513555cba89adfe45ab4f548ada")
            )
            .is_none()
        );
        let marketplaces = audited_domains_for(137)
            .into_iter()
            .filter(|(_, d)| d.name == OFF_CHAIN_MARKETPLACE_DOMAIN_NAME)
            .count();
        assert_eq!(marketplaces, 3);
        assert_eq!(audited_domains_for(1).len(), 0);
    }
}
