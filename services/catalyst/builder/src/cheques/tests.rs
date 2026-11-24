use alloy::signers::SignerSync;
use alloy::signers::local::PrivateKeySigner;
use alloy_primitives::{Address, B256, U256};
use serde_json::json;

use super::*;
use crate::network::{AMOY, POLYGON};

const KEY: &str = "0x1111111111111111111111111111111111111111111111111111111111111111";
const SIGNER: &str = "0x19e7e376e7c213b7e7e7e46cc70a5dd086daff2a";
const PROVIDER: &str = "urn:decentraland:matic:collections-thirdparty:provider";
const N: &str = "fffffffffffffffffffffffffffffffebaaedce6af48a03bbfd25e8cd0364141";

fn salt_of(byte: &str) -> String {
    format!("0x{}", byte.repeat(32))
}

fn signed(digest: [u8; 32]) -> [u8; 65] {
    let wallet: PrivateKeySigner = KEY.parse().unwrap();
    wallet
        .sign_hash_sync(&B256::from(digest))
        .unwrap()
        .as_bytes()
}

fn offer(qty: Value, salt: &str, signature: &str) -> Offer {
    Offer::parse(&json!({ "qty": qty, "salt": salt, "signature": signature }))
}

fn cheque_for(provider: &str, qty: i32, salt: &str) -> String {
    let digest = digest(&POLYGON, provider, qty, salt.parse().unwrap());
    format!("0x{}", hex::encode(signed(digest)))
}

#[test]
fn the_digest_is_the_registry_typed_data_the_clients_sign() {
    assert_eq!(
        hex::encode(domain(&POLYGON)),
        "7c22cdd2fa6286b864b9d5b8b0b0ee6eaee6235d17b48eb1231839228755b0ab"
    );
    assert_eq!(
        hex::encode(digest(
            &POLYGON,
            PROVIDER,
            2,
            salt_of("ab").parse().unwrap()
        )),
        "0ee8c55105ec0418bb2f56be192e7b3c8d814026705d6699076714a43a8a3932"
    );
    let known = "0x32141b34fc2592055513ef900955b86fbe4b0a708d230f7ba988948ea67d5ecf4452643c3eb73d2ee5718be1e014a95531de3a06b7bab8112816bf598af41bd31b";
    assert_eq!(
        offer(json!(2), &salt_of("ab"), known).signer(&POLYGON, PROVIDER),
        Ok(SIGNER.to_string())
    );
    let stored = offer(json!(2), &salt_of("ab"), known).stored().unwrap();
    assert_eq!(
        receipt_id(&POLYGON, PROVIDER, &stored).unwrap(),
        "0x0ee8c55105ec0418bb2f56be192e7b3c8d814026705d6699076714a43a8a3932"
    );
}

#[test]
fn an_amoy_cheque_hashes_as_builder_servers_on_sepolia_and_recovers_its_signer_only_there() {
    let provider = "urn:decentraland:amoy:collections-thirdparty:jean-pier";
    let salt = "0x79ab6dbeeebdd32191ad0b9774e07349b7883359f07237a6cb2179d7bf462a2f";
    let cheque = Cheque {
        qty: 1,
        salt: salt.into(),
        signature: String::new(),
    };
    assert_eq!(
        receipt_id(&AMOY, provider, &cheque).as_deref(),
        Some("0x96f6844148bdeac5fa9c947bc253e634545e97400780485ab9c8ef409414e562")
    );
    let signature = format!(
        "0x{}",
        hex::encode(signed(digest(&AMOY, provider, 1, salt.parse().unwrap())))
    );
    assert_eq!(
        offer(json!(1), salt, &signature).signer(&AMOY, provider),
        Ok(SIGNER.to_string())
    );
    assert_ne!(
        offer(json!(1), salt, &signature).signer(&POLYGON, provider),
        Ok(SIGNER.to_string()),
        "a mainnet node recovers another wallet from an Amoy cheque"
    );
}

#[test]
fn a_cheque_recovers_its_signer_only_for_the_provider_quantity_and_salt_it_signed() {
    let signature = cheque_for(PROVIDER, 3, &salt_of("01"));
    assert_eq!(
        offer(json!(3), &salt_of("01"), &signature).signer(&POLYGON, PROVIDER),
        Ok(SIGNER.to_string())
    );
    for (qty, salt, provider) in [
        (4, salt_of("01"), PROVIDER),
        (3, salt_of("02"), PROVIDER),
        (
            3,
            salt_of("01"),
            "urn:decentraland:matic:collections-thirdparty:other",
        ),
        (
            3,
            salt_of("01"),
            "urn:decentraland:amoy:collections-thirdparty:provider",
        ),
    ] {
        assert_ne!(
            offer(json!(qty), &salt, &signature).signer(&POLYGON, provider),
            Ok(SIGNER.to_string()),
            "{qty} {salt} {provider}"
        );
    }
}

#[test]
fn a_cheque_signed_under_another_chain_or_contract_recovers_another_wallet() {
    let salt: B256 = salt_of("05").parse().unwrap();
    let message = struct_hash(
        hash_dynamic(CONSUME_SLOTS.as_bytes()),
        &[
            hash_dynamic(PROVIDER.as_bytes()),
            word_u256(U256::from(1u64)),
            salt.0,
        ],
    );
    let (registry, amoy_registry) = (
        POLYGON.third_party_registry.parse::<Address>().unwrap(),
        AMOY.third_party_registry.parse::<Address>().unwrap(),
    );
    for (contract, chain) in [
        (amoy_registry, AMOY.chain_id),
        (registry, AMOY.chain_id),
        (amoy_registry, POLYGON.chain_id),
    ] {
        let other = domain_separator_salted(
            NAME,
            VERSION,
            contract,
            U256::from(chain).to_be_bytes::<32>(),
        );
        let signature = format!(
            "0x{}",
            hex::encode(signed(typed_data_digest(other, message)))
        );
        assert_ne!(
            offer(json!(1), &salt_of("05"), &signature).signer(&POLYGON, PROVIDER),
            Ok(SIGNER.to_string()),
            "{contract} {chain}"
        );
    }
}

#[test]
fn both_v_encodings_and_the_compact_form_store_one_spelling() {
    let salts: Vec<String> = (0..16).map(|n| salt_of(&format!("{n:02x}"))).collect();
    let parities: std::collections::BTreeSet<u8> = salts
        .iter()
        .map(|salt| signed(digest(&POLYGON, PROVIDER, 2, salt.parse().unwrap()))[64])
        .collect();
    assert_eq!(parities.len(), 2, "the salts give both parities");
    for salt in &salts {
        spellings(salt);
    }
}

fn spellings(salt: &str) {
    let salt = salt.to_string();
    let full = signed(digest(&POLYGON, PROVIDER, 2, salt.parse().unwrap()));
    let canonical = format!("0x{}", hex::encode(full));
    let mut zero_one = full;
    zero_one[64] -= 27;
    let mut compact = [0u8; 64];
    compact.copy_from_slice(&full[..64]);
    compact[32] |= (full[64] - 27) << 7;
    for spelling in [
        canonical.clone(),
        format!("0x{}", hex::encode(zero_one)),
        format!("0x{}", hex::encode(compact)),
        canonical.to_uppercase().replacen("0X", "0x", 1),
    ] {
        let offer = offer(
            json!(2),
            &salt.to_uppercase().replacen("0X", "0x", 1),
            &spelling,
        );
        assert_eq!(
            offer.signer(&POLYGON, PROVIDER),
            Ok(SIGNER.to_string()),
            "{spelling}"
        );
        assert_eq!(
            offer.stored(),
            Some(Cheque {
                qty: 2,
                salt: salt.clone(),
                signature: canonical.clone()
            }),
            "{spelling}"
        );
    }
}

#[test]
fn a_high_s_signature_recovers_nobody() {
    let salt = salt_of("0d");
    let full = signed(digest(&POLYGON, PROVIDER, 2, salt.parse().unwrap()));
    let n = U256::from_str_radix(N, 16).unwrap();
    let mut high = full;
    high[32..64].copy_from_slice(&(n - U256::from_be_slice(&full[32..64])).to_be_bytes::<32>());
    high[64] = if full[64] == 27 { 28 } else { 27 };
    assert_eq!(
        offer(json!(2), &salt, &format!("0x{}", hex::encode(high))).signer(&POLYGON, PROVIDER),
        Err("invalid signature")
    );
}

#[test]
fn fields_of_another_shape_read_as_absent() {
    let salt = salt_of("0e");
    let signature = cheque_for(PROVIDER, 2, &salt);
    for qty in [
        json!("2"),
        json!(0),
        json!(-1),
        json!(2.5),
        json!(2_147_483_648u64),
        json!(null),
    ] {
        assert_eq!(offer(qty.clone(), &salt, &signature).qty, None, "{qty}");
    }
    for bad in [
        salt[2..].to_string(),
        salt_of("0").to_string(),
        format!("{salt}00"),
        salt.replace('e', "g"),
    ] {
        let offer = offer(json!(2), &bad, &signature);
        assert_eq!(
            offer.signer(&POLYGON, PROVIDER),
            Err("invalid salt"),
            "{bad}"
        );
        assert_eq!(offer.stored(), None);
    }
    let mut v29 = signature.clone();
    v29.replace_range(130.., "1d");
    for bad in [
        signature[2..].to_string(),
        format!("{signature}00"),
        signature[..128].to_string(),
        v29,
        signature.replace('a', "z"),
        String::new(),
    ] {
        let offer = offer(json!(2), &salt, &bad);
        assert_eq!(
            offer.signer(&POLYGON, PROVIDER),
            Err("invalid signature"),
            "{bad}"
        );
        assert_eq!(offer.stored(), None);
    }
    assert!(Offer::parse(&json!("cheque")).stored().is_none());
}
