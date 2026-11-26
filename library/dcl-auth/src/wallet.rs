//! A secp256k1 signing key and the two-link chain a plain key signs with.

use alloy_primitives::eip191_hash_message;
use k256::ecdsa::SigningKey;
use zeroize::Zeroizing;

use crate::eth_address::{format_address, from_verifying_key};

#[derive(Debug, thiserror::Error)]
pub enum SignError {
    #[error("invalid private key: {0}")]
    InvalidPrivateKey(String),
    #[error("signing failed: {0}")]
    SigningFailed(String),
}

/// A secp256k1 key that signs EIP-191 personal messages: the authoritative
/// signer (in-process minting) and every ephemeral delegation key.
pub struct Wallet {
    key: SigningKey,
    address: [u8; 20],
}

impl std::fmt::Debug for Wallet {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Wallet")
            .field("address", &self.address())
            .finish_non_exhaustive()
    }
}

impl Wallet {
    pub fn from_hex(raw: &str) -> Result<Self, SignError> {
        let hexs = raw.trim().trim_start_matches("0x");
        let bytes = Zeroizing::new(
            hex::decode(hexs).map_err(|e| SignError::InvalidPrivateKey(format!("not hex: {e}")))?,
        );
        if bytes.len() != 32 {
            return Err(SignError::InvalidPrivateKey(format!(
                "expected 32 bytes (64 hex chars), got {}",
                bytes.len()
            )));
        }
        let key = SigningKey::from_slice(&bytes)
            .map_err(|e| SignError::InvalidPrivateKey(e.to_string()))?;
        let address = from_verifying_key(key.verifying_key());
        Ok(Wallet { key, address })
    }

    /// A fresh key from the OS entropy source (an ephemeral delegation key).
    pub fn random() -> Result<Self, SignError> {
        let mut bytes = Zeroizing::new([0u8; 32]);
        getrandom::fill(bytes.as_mut_slice())
            .map_err(|e| SignError::InvalidPrivateKey(format!("no entropy: {e}")))?;
        let key = SigningKey::from_slice(bytes.as_slice())
            .map_err(|e| SignError::InvalidPrivateKey(e.to_string()))?;
        let address = from_verifying_key(key.verifying_key());
        Ok(Wallet { key, address })
    }

    /// Lowercase `0x…` (the form every auth chain and claim carries).
    pub fn address(&self) -> String {
        format_address(&self.address)
    }

    /// The uncompressed SEC1 public key, `0x04…` (the envelope's `publicKey`).
    pub fn public_key_hex(&self) -> String {
        format!(
            "0x{}",
            hex::encode(self.key.verifying_key().to_sec1_point(false).as_bytes())
        )
    }

    pub fn private_key_hex(&self) -> String {
        format!(
            "0x{}",
            hex::encode(Zeroizing::new(self.key.to_bytes()).as_slice())
        )
    }

    pub fn sign_message(&self, message: &[u8]) -> Result<String, SignError> {
        let hash = eip191_hash_message(message);
        let (sig, recovery_id) = self.key.sign_prehash_recoverable(hash.as_slice());
        let mut bytes = [0u8; 65];
        bytes[..64].copy_from_slice(&sig.to_bytes());
        bytes[64] = 27 + u8::from(recovery_id.is_y_odd());
        Ok(format!("0x{}", hex::encode(bytes)))
    }
}

/// A scoped login (ADR-162) from `wallet` to `ephemeral` for `grants` until
/// `expiration_ms`, with `payload` signed by the ephemeral key.
pub fn create_scoped_auth_chain(
    wallet: &Wallet,
    ephemeral: &Wallet,
    expiration_ms: i64,
    grants: &[crate::scoped_login::ScopedLoginGrant],
    payload: &str,
) -> Result<serde_json::Value, SignError> {
    let message =
        crate::scoped_login::scoped_ephemeral_message(&ephemeral.address(), expiration_ms, grants)
            .map_err(SignError::SigningFailed)?;
    Ok(serde_json::json!([
        { "type": "SIGNER", "payload": wallet.address(), "signature": "" },
        { "type": "ECDSA_SCOPED_EPHEMERAL", "payload": message, "signature": wallet.sign_message(message.as_bytes())? },
        { "type": "ECDSA_SIGNED_ENTITY", "payload": payload, "signature": ephemeral.sign_message(payload.as_bytes())? },
    ]))
}

/// A two-link chain (SIGNER, ECDSA_SIGNED_ENTITY over `payload`): what a
/// signed fetch by a plain key carries.
pub fn create_simple_auth_chain(
    wallet: &Wallet,
    payload: &str,
) -> Result<serde_json::Value, SignError> {
    let signature = wallet.sign_message(payload.as_bytes())?;
    Ok(serde_json::json!([
        { "type": "SIGNER", "payload": wallet.address(), "signature": "" },
        { "type": "ECDSA_SIGNED_ENTITY", "payload": payload, "signature": signature },
    ]))
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy::signers::{SignerSync, local::PrivateKeySigner};

    #[test]
    fn matches_alloy_byte_for_byte() {
        let keys = [
            "ac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80",
            "59c6995e998f97a5a0044966f0945389dc9e86dae88c7a8412f4603b6b78690d",
            "0000000000000000000000000000000000000000000000000000000000000001",
            "fffffffffffffffffffffffffffffffebaaedce6af48a03bbfd25e8cd0364140",
        ];
        let messages: [&[u8]; 4] = [
            b"",
            b"Decentraland Login",
            b"get:/foo:1700000000000:{}",
            &[0xff; 300],
        ];
        for hex_key in keys {
            let ours = Wallet::from_hex(hex_key).unwrap();
            let theirs: PrivateKeySigner = hex_key.parse().unwrap();
            assert_eq!(ours.address(), format!("{:#x}", theirs.address()));
            assert_eq!(
                ours.public_key_hex(),
                format!(
                    "0x{}",
                    hex::encode(
                        theirs
                            .credential()
                            .verifying_key()
                            .to_encoded_point(false)
                            .as_bytes()
                    )
                )
            );
            for message in messages {
                let sig = ours.sign_message(message).unwrap();
                assert_eq!(sig, theirs.sign_message_sync(message).unwrap().to_string());
                assert_eq!(
                    crate::recover_address(message, &sig).unwrap(),
                    ours.address()
                );
            }
        }
    }

    #[test]
    fn random_keys_sign_what_they_claim() {
        for _ in 0..32 {
            let wallet = Wallet::random().unwrap();
            let again = Wallet::from_hex(&wallet.private_key_hex()).unwrap();
            assert_eq!(again.address(), wallet.address());
            let sig = wallet.sign_message(b"hello").unwrap();
            assert_eq!(
                crate::recover_address(b"hello", &sig).unwrap(),
                wallet.address()
            );
        }
    }
}
