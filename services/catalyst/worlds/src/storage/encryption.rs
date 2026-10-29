//! AES-256-GCM for the env namespace: `worlds.env_variables.value_enc` is
//! `0x01 || 12-byte IV || ciphertext || 16-byte tag`; plaintext never reaches
//! the database and a value written under one `ENCRYPTION_KEY` cannot be read
//! under another.

use aes_gcm::aead::{Aead, KeyInit, Payload};
use aes_gcm::{Aes256Gcm, Nonce};
use rand::Rng;

use super::errors::ApiError;

const FORMAT_VERSION: u8 = 0x01;
const VERSION_LENGTH: usize = 1;
const IV_LENGTH: usize = 12;
const AUTH_TAG_LENGTH: usize = 16;

#[derive(Clone)]
pub struct Encryptor {
    cipher: Aes256Gcm,
}

impl Encryptor {
    pub fn new(key: &[u8; 32]) -> Self {
        let cipher = Aes256Gcm::new(key.into());
        Self { cipher }
    }

    pub fn encrypt(&self, plaintext: &str) -> Result<Vec<u8>, ApiError> {
        let mut iv = [0u8; IV_LENGTH];
        rand::rng().fill_bytes(&mut iv);
        let nonce =
            Nonce::try_from(iv.as_slice()).map_err(|_| ApiError::internal("encryption failed"))?;

        let ciphertext = self
            .cipher
            .encrypt(
                &nonce,
                Payload {
                    msg: plaintext.as_bytes(),
                    aad: &[],
                },
            )
            .map_err(|_| ApiError::internal("encryption failed"))?;

        let mut out = Vec::with_capacity(VERSION_LENGTH + IV_LENGTH + ciphertext.len());
        out.push(FORMAT_VERSION);
        out.extend_from_slice(&iv);
        out.extend_from_slice(&ciphertext);
        Ok(out)
    }

    pub fn decrypt(&self, encrypted: &[u8]) -> Result<String, ApiError> {
        let min_len = VERSION_LENGTH + IV_LENGTH + AUTH_TAG_LENGTH;
        if encrypted.len() < min_len {
            return Err(ApiError::internal(
                "Invalid encrypted data: buffer too short",
            ));
        }
        if encrypted[0] != FORMAT_VERSION {
            return Err(ApiError::internal(format!(
                "Unsupported encryption format version: {}",
                encrypted[0]
            )));
        }

        let iv = &encrypted[VERSION_LENGTH..VERSION_LENGTH + IV_LENGTH];
        let ct_and_tag = &encrypted[VERSION_LENGTH + IV_LENGTH..];
        let nonce = Nonce::try_from(iv)
            .map_err(|_| ApiError::internal("Decryption failed: invalid IV length"))?;

        let plaintext = self
            .cipher
            .decrypt(
                &nonce,
                Payload {
                    msg: ct_and_tag,
                    aad: &[],
                },
            )
            .map_err(|_| {
                ApiError::internal(
                    "Decryption failed: data may be corrupted or encrypted with a different key",
                )
            })?;

        String::from_utf8(plaintext)
            .map_err(|_| ApiError::internal("Decryption failed: invalid UTF-8"))
    }
}

/// `ENCRYPTION_KEY`: exactly 64 hex characters (32 bytes).
pub fn parse_encryption_key(key_hex: &str) -> anyhow::Result<[u8; 32]> {
    let key_hex = key_hex.trim();
    if key_hex.len() != 64 {
        anyhow::bail!(
            "invalid ENCRYPTION_KEY length: expected 64 hex characters, got {}",
            key_hex.len()
        );
    }
    let bytes = hex::decode(key_hex).map_err(|_| {
        anyhow::anyhow!("invalid ENCRYPTION_KEY: contains non-hexadecimal characters")
    })?;
    let mut key = [0u8; 32];
    key.copy_from_slice(&bytes);
    Ok(key)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip() {
        let key = [7u8; 32];
        let enc = Encryptor::new(&key);
        let blob = enc.encrypt("super-secret-value").unwrap();
        assert_eq!(blob[0], FORMAT_VERSION);

        assert_eq!(blob.len(), 1 + 12 + "super-secret-value".len() + 16);
        let back = enc.decrypt(&blob).unwrap();
        assert_eq!(back, "super-secret-value");
    }

    #[test]
    fn wrong_key_fails() {
        let blob = Encryptor::new(&[1u8; 32]).encrypt("x").unwrap();
        assert!(Encryptor::new(&[2u8; 32]).decrypt(&blob).is_err());
    }

    #[test]
    fn the_key_is_64_hex_characters() {
        assert_eq!(parse_encryption_key(&"0".repeat(64)).unwrap(), [0u8; 32]);
        assert_eq!(
            parse_encryption_key(&format!(" {} \n", "ff".repeat(32))).unwrap(),
            [0xffu8; 32]
        );
        let short = parse_encryption_key("abc").unwrap_err().to_string();
        assert!(
            short.contains("expected 64 hex characters, got 3"),
            "{short}"
        );
        let bad = parse_encryption_key(&"zz".repeat(32))
            .unwrap_err()
            .to_string();
        assert!(bad.contains("non-hexadecimal"), "{bad}");
    }
}
