//! A secp256k1 signing key and the two-link chain a plain key signs with.

use alloy_primitives::{Address as EthAddress, Signature, U256, eip191_hash_message};
use k256::ecdsa::SigningKey;

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
    address: EthAddress,
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
        let bytes =
            hex::decode(hexs).map_err(|e| SignError::InvalidPrivateKey(format!("not hex: {e}")))?;
        if bytes.len() != 32 {
            return Err(SignError::InvalidPrivateKey(format!(
                "expected 32 bytes (64 hex chars), got {}",
                bytes.len()
            )));
        }
        let key = SigningKey::from_slice(&bytes)
            .map_err(|e| SignError::InvalidPrivateKey(e.to_string()))?;
        let address = EthAddress::from_public_key(key.verifying_key());
        Ok(Wallet { key, address })
    }

    /// A fresh key from the OS entropy source (an ephemeral delegation key).
    pub fn random() -> Result<Self, SignError> {
        let mut bytes = [0u8; 32];
        getrandom::fill(&mut bytes)
            .map_err(|e| SignError::InvalidPrivateKey(format!("no entropy: {e}")))?;
        let key = SigningKey::from_slice(&bytes)
            .map_err(|e| SignError::InvalidPrivateKey(e.to_string()))?;
        let address = EthAddress::from_public_key(key.verifying_key());
        Ok(Wallet { key, address })
    }

    /// Lowercase `0x…` (the form every auth chain and claim carries).
    pub fn address(&self) -> String {
        format!("{:#x}", self.address)
    }

    /// The uncompressed SEC1 public key, `0x04…` (the envelope's `publicKey`).
    pub fn public_key_hex(&self) -> String {
        format!(
            "0x{}",
            hex::encode(self.key.verifying_key().to_encoded_point(false).as_bytes())
        )
    }

    pub fn private_key_hex(&self) -> String {
        format!("0x{}", hex::encode(self.key.to_bytes()))
    }

    pub fn sign_message(&self, message: &[u8]) -> Result<String, SignError> {
        let hash = eip191_hash_message(message);
        let (sig, recovery_id) = self
            .key
            .sign_prehash_recoverable(hash.as_slice())
            .map_err(|e| SignError::SigningFailed(e.to_string()))?;
        let signature = Signature::new(
            U256::from_be_slice(&sig.r().to_bytes()),
            U256::from_be_slice(&sig.s().to_bytes()),
            recovery_id.is_y_odd(),
        );
        Ok(signature.to_string())
    }
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
