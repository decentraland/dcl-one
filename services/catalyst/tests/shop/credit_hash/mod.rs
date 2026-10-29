// from: tests/shop/credit_hash/vectors.mjs, src/credits/handlers/authorize.rs (authorize_digest, TreasuryWallet)
//! The credit-hash vectors, independently of ethers: the contract digest is
//! alloy's ABI encoding of `(address sender, uint256 chainid, address
//! manager, Credit)`, the signature is v2's own `TreasuryWallet` (EIP-191
//! over the 32 hash bytes, as `toEthSignedMessageHash().recover` expects),
//! and the v2 digest is the text layout `authorize.rs` pins in
//! `digest_layout_is_pinned` (id, contract, signer, amount wei, expiry ms).
//! The last two tests are the reason `docs/shop/CREDIT-HASH.md` exists: the
//! v2 digest does not verify on the contract, and v2's millisecond expiry is
//! a different credit.

use std::str::FromStr;

use alloy_primitives::{Address, B256, Signature, U256, eip191_hash_message, keccak256};
use alloy_sol_types::{SolValue, sol};
use catalyst::shop::credits::digest::TreasuryWallet;
use serde_json::Value;

sol! {
    struct Credit {
        uint256 value;
        uint256 expiresAt;
        bytes32 salt;
    }
}

const VECTORS: &str = include_str!(concat!(
    env!("DCL_ONE_ROOT"),
    "/services/catalyst/tests/fixtures/shop/credit-hash/vectors.json"
));

fn doc() -> Value {
    serde_json::from_str(VECTORS).expect("vectors.json parses")
}

fn s<'a>(v: &'a Value, path: &[&str]) -> &'a str {
    path.iter()
        .fold(v, |v, k| &v[*k])
        .as_str()
        .unwrap_or_else(|| panic!("{path:?} is a string"))
}

fn contract_hash(v: &Value, expires_at: U256) -> (Vec<u8>, B256) {
    let input = &v["input"];
    let user = Address::from_str(s(input, &["userAddress"])).unwrap();
    let manager = Address::from_str(s(input, &["creditsManager"])).unwrap();
    let chain = U256::from(input["chainId"].as_u64().unwrap());
    let credit = Credit {
        value: U256::from_str(s(input, &["credit", "value"])).unwrap(),
        expiresAt: expires_at,
        salt: B256::from_str(s(input, &["credit", "salt"])).unwrap(),
    };
    let encoded = (user, chain, manager, credit).abi_encode();
    let hash = keccak256(&encoded);
    (encoded, hash)
}

fn expires_at(v: &Value) -> U256 {
    U256::from(v["input"]["credit"]["expiresAt"].as_u64().unwrap())
}

fn wallet(doc: &Value) -> TreasuryWallet {
    TreasuryWallet::from_hex(s(doc, &["signer", "privateKey"])).unwrap()
}

fn recover(prehash: B256, signature: &str) -> String {
    format!(
        "{:#x}",
        Signature::from_str(signature)
            .unwrap()
            .recover_address_from_prehash(&prehash)
            .unwrap()
    )
}

#[test]
fn five_vectors_are_pinned() {
    assert_eq!(doc()["vectors"].as_array().unwrap().len(), 5);
}

#[test]
fn the_contract_digest_is_six_abi_words_over_sender_chain_manager_credit() {
    for v in doc()["vectors"].as_array().unwrap() {
        let (encoded, hash) = contract_hash(v, expires_at(v));
        assert_eq!(encoded.len(), 6 * 32, "{}", v["name"]);
        assert_eq!(
            format!("0x{}", hex::encode(&encoded)),
            s(v, &["upstream", "abiEncoded"]),
            "{}",
            v["name"]
        );
        assert_eq!(
            format!("{hash:#x}"),
            s(v, &["upstream", "creditHash"]),
            "{}",
            v["name"]
        );
    }
}

#[test]
fn the_credit_signature_is_eip191_over_the_hash_bytes_and_v2s_wallet_already_makes_it() {
    let doc = doc();
    let signer = s(&doc, &["signer", "address"]).to_lowercase();
    let wallet = wallet(&doc);
    assert_eq!(wallet.address(), signer);
    for v in doc["vectors"].as_array().unwrap() {
        let (_, hash) = contract_hash(v, expires_at(v));
        let prefixed = eip191_hash_message(hash);
        assert_eq!(
            format!("{prefixed:#x}"),
            s(v, &["upstream", "ethSignedMessageHash"]),
            "{}",
            v["name"]
        );
        let signature = wallet.sign_message(hash.as_slice()).unwrap();
        assert_eq!(signature, s(v, &["upstream", "signature"]), "{}", v["name"]);
        assert_eq!(recover(prefixed, &signature), signer, "{}", v["name"]);
    }
}

#[test]
fn the_v2_text_digest_signs_something_the_contract_does_not_recover() {
    let doc = doc();
    let signer = s(&doc, &["signer", "address"]).to_lowercase();
    let wallet = wallet(&doc);
    for v in doc["vectors"].as_array().unwrap() {
        let a = &v["v2"]["authorizeDigestArgs"];
        let text = format!(
            "{}{}{}{}{}",
            s(a, &["id"]),
            s(a, &["contract"]),
            s(a, &["signerAddress"]),
            s(a, &["amountWei"]),
            a["expiresAtMs"].as_u64().unwrap()
        );
        assert_eq!(text, s(v, &["v2", "preimageText"]), "{}", v["name"]);
        let digest = keccak256(text.as_bytes());
        assert_eq!(
            format!("{digest:#x}"),
            s(v, &["v2", "digest"]),
            "{}",
            v["name"]
        );
        let v2_signature = wallet.sign_message(digest.as_slice()).unwrap();
        assert_eq!(v2_signature, s(v, &["v2", "signature"]), "{}", v["name"]);

        let (_, hash) = contract_hash(v, expires_at(v));
        let on_chain = recover(eip191_hash_message(hash), &v2_signature);
        assert_eq!(
            on_chain,
            s(v, &["v2", "contractRecoversFromV2Signature"]).to_lowercase(),
            "{}",
            v["name"]
        );
        assert_ne!(
            on_chain, signer,
            "{}: the contract would accept a v2 signature",
            v["name"]
        );
    }
}

#[test]
fn a_millisecond_expiry_is_a_different_credit() {
    for v in doc()["vectors"].as_array().unwrap() {
        let (_, seconds) = contract_hash(v, expires_at(v));
        let (_, millis) = contract_hash(v, expires_at(v) * U256::from(1000));
        assert_ne!(seconds, millis, "{}", v["name"]);
        assert_eq!(
            format!("{millis:#x}"),
            s(v, &["v2", "creditHashIfTheClientSubmitsV2ExpiresAt"]),
            "{}",
            v["name"]
        );
    }
}
