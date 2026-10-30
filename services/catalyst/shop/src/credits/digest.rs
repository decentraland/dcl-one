// from: credits-server@465e628bc:src/logic/credit-hash.ts (computeCreditHash), src/logic/signer.ts (sign), offchain-marketplace-contract@87350c63e:src/credits/CreditsManagerPolygon.sol (_validateAndApplyCredits, revokeCredits)
//! THE identity of an ephemeral credit, as `CreditsManagerPolygon` computes
//! it (docs/shop/CREDIT-HASH.md):
//!
//! ```text
//! creditHash = keccak256(abi.encode(sender, block.chainid, address(this), Credit{value, expiresAt, salt}))
//! ```
//!
//! Six static 32-byte words, `expiresAt` in unix SECONDS, signed as the
//! EIP-191 personal message of the 32 hash bytes
//! (`creditHash.toEthSignedMessageHash().recover(signature)` must hold the
//! `CREDITS_SIGNER_ROLE`). The same hash keys `isRevoked` and `spentValue`
//! on chain and is the indexed `_creditId` of `CreditUsed`; the salt alone is
//! not the identity (the squid keys on it, the contract does not). Every
//! signer and every revoker in this module derives the hash through
//! [`CreditIdentity::credit_hash`], so the two cannot disagree.

use std::str::FromStr;

use alloy_primitives::{Address, B256, Signature, U256, eip191_hash_message, keccak256};
use alloy_sol_types::{SolValue, sol};
use k256::ecdsa::SigningKey;

sol! {
    /// `CreditsManagerPolygon.Credit`.
    struct Credit {
        uint256 value;
        uint256 expiresAt;
        bytes32 salt;
    }
}

/// What a credit binds: the buyer (`_msgSender()` of `useCredits`, through a
/// meta-transaction too), the chain, the manager and the struct.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CreditIdentity {
    pub sender: Address,
    pub chain_id: u64,
    pub manager: Address,
    /// MANA wei.
    pub value: U256,
    /// Unix seconds.
    pub expires_at: u64,
    pub salt: B256,
}

impl CreditIdentity {
    /// `abi.encode(sender, chainId, manager, Credit)`: 192 bytes, no offsets.
    pub fn abi_encoded(&self) -> Vec<u8> {
        let credit = Credit {
            value: self.value,
            expiresAt: U256::from(self.expires_at),
            salt: self.salt,
        };
        (self.sender, U256::from(self.chain_id), self.manager, credit).abi_encode()
    }

    pub fn credit_hash(&self) -> B256 {
        keccak256(self.abi_encoded())
    }

    /// The prehash the contract recovers the signer from.
    pub fn eth_signed_message_hash(&self) -> B256 {
        eip191_hash_message(self.credit_hash())
    }
}

/// A bytes32 hex string (`0x` + 64 hex digits, either case), as the salts
/// and transaction hashes arrive on the wire.
pub fn parse_bytes32(text: &str) -> Option<B256> {
    let digits = text.strip_prefix("0x")?;
    if digits.len() != 64 || !digits.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    B256::from_str(text).ok()
}

/// A salt as the database stores it and the wire carries it: lowercase.
pub fn bytes32_hex(value: B256) -> String {
    format!("{value:#x}")
}

/// The address `toEthSignedMessageHash().recover` yields for `signature`
/// over `credit_hash`, or `None` for a malformed signature.
pub fn recover_credit_signer(credit_hash: B256, signature: &str) -> Option<Address> {
    let signature = Signature::from_str(signature).ok()?;
    signature
        .recover_address_from_prehash(&eip191_hash_message(credit_hash))
        .ok()
}

/// The treasury signer (`CREDITS_SIGNER_KEY_FILE`): an EIP-191 personal
/// message signature over the 32 hash bytes, RFC 6979 deterministic, so a
/// replayed authorization re-signs the same bytes.
pub struct TreasuryWallet {
    key: SigningKey,
    address: Address,
}

impl std::fmt::Debug for TreasuryWallet {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TreasuryWallet")
            .field("address", &self.address)
            .finish_non_exhaustive()
    }
}

impl TreasuryWallet {
    pub fn from_hex(raw: &str) -> Result<Self, String> {
        let hexs = raw.trim().trim_start_matches("0x");
        let bytes = hex::decode(hexs).map_err(|e| format!("not hex: {e}"))?;
        if bytes.len() != 32 {
            return Err(format!(
                "expected 32 bytes (64 hex chars), got {}",
                bytes.len()
            ));
        }
        let key = SigningKey::from_slice(&bytes).map_err(|e| e.to_string())?;
        let address = Address::from_public_key(key.verifying_key());
        Ok(Self { key, address })
    }

    /// Lowercase, as the role holder is compared on chain.
    pub fn address(&self) -> String {
        format!("{:#x}", self.address)
    }

    pub fn signer(&self) -> Address {
        self.address
    }

    /// EIP-191 over `message` (the 32 credit-hash bytes for a credit).
    pub fn sign_message(&self, message: &[u8]) -> Result<String, String> {
        let hash = eip191_hash_message(message);
        let (sig, recovery_id) = self
            .key
            .sign_prehash_recoverable(hash.as_slice())
            .map_err(|e| e.to_string())?;
        let signature = Signature::new(
            U256::from_be_slice(&sig.r().to_bytes()),
            U256::from_be_slice(&sig.s().to_bytes()),
            recovery_id.is_y_odd(),
        );
        Ok(signature.to_string())
    }

    /// The signature `useCredits` verifies for `credit`.
    pub fn sign_credit(&self, credit: &CreditIdentity) -> Result<String, String> {
        self.sign_message(credit.credit_hash().as_slice())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    const VECTORS: &str = include_str!(concat!(
        env!("DCL_ONE_ROOT"),
        "/services/catalyst/tests/fixtures/shop/credit-hash/vectors.json"
    ));

    fn doc() -> Value {
        serde_json::from_str(VECTORS).unwrap()
    }

    fn s<'a>(v: &'a Value, path: &[&str]) -> &'a str {
        path.iter()
            .fold(v, |v, k| &v[*k])
            .as_str()
            .unwrap_or_else(|| panic!("{path:?}"))
    }

    fn identity(v: &Value) -> CreditIdentity {
        let input = &v["input"];
        CreditIdentity {
            sender: Address::from_str(s(input, &["userAddress"])).unwrap(),
            chain_id: input["chainId"].as_u64().unwrap(),
            manager: Address::from_str(s(input, &["creditsManager"])).unwrap(),
            value: U256::from_str(s(input, &["credit", "value"])).unwrap(),
            expires_at: input["credit"]["expiresAt"].as_u64().unwrap(),
            salt: B256::from_str(s(input, &["credit", "salt"])).unwrap(),
        }
    }

    #[test]
    fn the_five_vectors_hash_encode_and_sign_as_upstream_and_the_contract() {
        let doc = doc();
        let wallet = TreasuryWallet::from_hex(s(&doc, &["signer", "privateKey"])).unwrap();
        assert_eq!(
            wallet.address(),
            s(&doc, &["signer", "address"]).to_lowercase()
        );
        let vectors = doc["vectors"].as_array().unwrap();
        assert_eq!(vectors.len(), 5);
        for v in vectors {
            let id = identity(v);
            let name = v["name"].as_str().unwrap();
            assert_eq!(id.abi_encoded().len(), 192, "{name}");
            assert_eq!(
                format!("0x{}", hex::encode(id.abi_encoded())),
                s(v, &["upstream", "abiEncoded"]),
                "{name}"
            );
            assert_eq!(
                bytes32_hex(id.credit_hash()),
                s(v, &["upstream", "creditHash"]),
                "{name}"
            );
            assert_eq!(
                bytes32_hex(id.eth_signed_message_hash()),
                s(v, &["upstream", "ethSignedMessageHash"]),
                "{name}"
            );
            let signature = wallet.sign_credit(&id).unwrap();
            assert_eq!(signature, s(v, &["upstream", "signature"]), "{name}");
            assert_eq!(
                recover_credit_signer(id.credit_hash(), &signature),
                Some(wallet.signer()),
                "{name}"
            );
            let millis = CreditIdentity {
                expires_at: id.expires_at * 1000,
                ..id
            };
            assert_eq!(
                bytes32_hex(millis.credit_hash()),
                s(v, &["v2", "creditHashIfTheClientSubmitsV2ExpiresAt"]),
                "{name}"
            );
            assert_ne!(
                millis.credit_hash(),
                id.credit_hash(),
                "{name}: milliseconds are another credit"
            );
            let v2_signature = wallet
                .sign_message(
                    &hex::decode(s(v, &["v2", "digest"]).trim_start_matches("0x")).unwrap(),
                )
                .unwrap();
            assert_eq!(v2_signature, s(v, &["v2", "signature"]), "{name}");
            let recovered = recover_credit_signer(id.credit_hash(), &v2_signature).unwrap();
            assert_eq!(
                format!("{recovered:#x}"),
                s(v, &["v2", "contractRecoversFromV2Signature"]).to_lowercase(),
                "{name}"
            );
            assert_ne!(
                recovered,
                wallet.signer(),
                "{name}: the old text digest never verifies"
            );
        }
    }

    #[test]
    fn signing_is_deterministic_and_bytes32_parsing_is_exact() {
        let doc = doc();
        let wallet = TreasuryWallet::from_hex(s(&doc, &["signer", "privateKey"])).unwrap();
        let id = identity(&doc["vectors"][0]);
        assert_eq!(
            wallet.sign_credit(&id).unwrap(),
            wallet.sign_credit(&id).unwrap(),
            "RFC 6979: a replay re-signs the same bytes"
        );
        assert!(TreasuryWallet::from_hex("0x1234").is_err());
        let salt = "0xA1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1";
        assert_eq!(
            bytes32_hex(parse_bytes32(salt).unwrap()),
            salt.to_lowercase()
        );
        for bad in [
            "0xsalt1",
            "nope",
            "0x1234",
            &salt[2..],
            &format!("{salt}00"),
        ] {
            assert!(parse_bytes32(bad).is_none(), "{bad}");
        }
        assert!(recover_credit_signer(id.credit_hash(), "0xnope").is_none());
    }
}
