use async_trait::async_trait;

use super::error::AuthError;

#[async_trait]
pub trait Eip1654Validator: Send + Sync {
    /// A validator that asks the current authority on every call, without reusing a
    /// previous signature verdict. Fresh authentication fails closed when unavailable.
    /// Wrappers must forward this capability explicitly; historical trust is not fresh.
    fn fresh_validator(&self) -> Option<&dyn Eip1654Validator> {
        None
    }

    async fn validate_signature(
        &self,
        contract_address: &str,
        hash: &[u8],
        signature: &[u8],
    ) -> Result<bool, AuthError>;

    /// The same question at `block`. A validator that cannot answer for a past block fails
    /// the call (undecidable).
    async fn validate_signature_at_block(
        &self,
        _contract_address: &str,
        _hash: &[u8],
        _signature: &[u8],
        _block: u64,
    ) -> Result<bool, AuthError> {
        Err(AuthError::Eip1654ValidationFailed(
            "this validator cannot answer for a past block".to_string(),
        ))
    }

    /// Selects the contract-check block for a captured verification-start timestamp.
    /// This selector includes first/head shortcuts and strict predecessor selection.
    async fn verification_time_block(&self, _started_ms: i64) -> Result<u64, AuthError> {
        Err(AuthError::Eip1654ValidationFailed(
            "this validator cannot select a verification-time block".into(),
        ))
    }
}

/// Accepts every contract-wallet link. For entities a peer already holds: the
/// peer asked the contract when the entity was deployed, and `isValidSignature`
/// at `latest` is a different question (owners rotate, wallets upgrade), so a
/// replica re-asking it would refuse history the peer serves. The structural
/// checks around the link (signature present and hex, ephemeral payload and
/// expiry against the entity timestamp, final authority) still run.
pub struct PeerValidated;

#[async_trait]
impl Eip1654Validator for PeerValidated {
    async fn validate_signature(
        &self,
        _contract_address: &str,
        _hash: &[u8],
        _signature: &[u8],
    ) -> Result<bool, AuthError> {
        Ok(true)
    }

    async fn validate_signature_at_block(
        &self,
        _contract_address: &str,
        _hash: &[u8],
        _signature: &[u8],
        _block: u64,
    ) -> Result<bool, AuthError> {
        Ok(true)
    }
}

/// Asks the contract about keccak256(message), then about its EIP-191 hash, each on its own
/// as `@dcl/crypto` does: a magic answer to either accepts; otherwise a failed call is the
/// caller's error to report, and two clean refusals are `Ok(false)`.
pub async fn verify_eip1654(
    validator: &dyn Eip1654Validator,
    contract_address: &str,
    message: &[u8],
    signature: &[u8],
) -> Result<bool, AuthError> {
    verify_hash_pair(validator, contract_address, message, signature, None).await
}

pub(crate) async fn verify_eip1654_at_verification_time(
    validator: &dyn Eip1654Validator,
    contract_address: &str,
    message: &[u8],
    signature: &[u8],
    started_ms: i64,
) -> Result<bool, AuthError> {
    if matches!(
        verify_eip1654(validator, contract_address, message, signature).await,
        Ok(true)
    ) {
        return Ok(true);
    }
    let block = validator.verification_time_block(started_ms).await?;
    verify_hash_pair(validator, contract_address, message, signature, Some(block)).await
}

async fn verify_hash_pair(
    validator: &dyn Eip1654Validator,
    contract_address: &str,
    message: &[u8],
    signature: &[u8],
    block: Option<u64>,
) -> Result<bool, AuthError> {
    let raw_hash = alloy_primitives::keccak256(message);
    let raw = match block {
        Some(block) => {
            validator
                .validate_signature_at_block(
                    contract_address,
                    raw_hash.as_slice(),
                    signature,
                    block,
                )
                .await
        }
        None => {
            validator
                .validate_signature(contract_address, raw_hash.as_slice(), signature)
                .await
        }
    };
    if let Ok(true) = raw {
        return Ok(true);
    }

    let prefixed_hash = alloy_primitives::eip191_hash_message(message);
    let prefixed = match block {
        Some(block) => {
            validator
                .validate_signature_at_block(
                    contract_address,
                    prefixed_hash.as_slice(),
                    signature,
                    block,
                )
                .await
        }
        None => {
            validator
                .validate_signature(contract_address, prefixed_hash.as_slice(), signature)
                .await
        }
    };
    match (raw, prefixed) {
        (_, Ok(true)) => Ok(true),
        (Err(e), _) | (_, Err(e)) => Err(e),
        _ => Ok(false),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct HashMatchValidator {
        expected_hash: [u8; 32],
        calls: AtomicUsize,
    }

    #[async_trait]
    impl Eip1654Validator for HashMatchValidator {
        async fn validate_signature(
            &self,
            _contract_address: &str,
            hash: &[u8],
            _signature: &[u8],
        ) -> Result<bool, AuthError> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            Ok(hash == self.expected_hash.as_slice())
        }
    }

    #[tokio::test]
    async fn accepts_raw_keccak_hash_without_trying_prefixed() {
        let message = b"hello world";
        let raw = alloy_primitives::keccak256(message).0;
        let v = HashMatchValidator {
            expected_hash: raw,
            calls: AtomicUsize::new(0),
        };
        let ok = verify_eip1654(&v, "0xcontract", message, &[0u8; 65])
            .await
            .unwrap();
        assert!(ok);
        assert_eq!(v.calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn falls_back_to_eip191_prefixed_hash() {
        let message = b"hello world";
        let prefixed: [u8; 32] = alloy_primitives::eip191_hash_message(message).0;
        let v = HashMatchValidator {
            expected_hash: prefixed,
            calls: AtomicUsize::new(0),
        };
        let ok = verify_eip1654(&v, "0xcontract", message, &[0u8; 65])
            .await
            .unwrap();
        assert!(ok);
        assert_eq!(v.calls.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn rejects_when_neither_hash_matches() {
        let v = HashMatchValidator {
            expected_hash: [0xEE; 32],
            calls: AtomicUsize::new(0),
        };
        let ok = verify_eip1654(&v, "0xcontract", b"msg", &[0u8; 65])
            .await
            .unwrap();
        assert!(!ok);
        assert_eq!(v.calls.load(Ordering::SeqCst), 2);
    }

    /// Each hash is its own question, as upstream asks them: a failed call for the raw hash
    /// does not stop the EIP-191 hash from being asked.
    #[tokio::test]
    async fn a_failed_call_for_one_hash_still_asks_the_other() {
        struct FailsRaw([u8; 32]);
        #[async_trait]
        impl Eip1654Validator for FailsRaw {
            async fn validate_signature(
                &self,
                _c: &str,
                hash: &[u8],
                _s: &[u8],
            ) -> Result<bool, AuthError> {
                if hash == self.0.as_slice() {
                    Ok(true)
                } else {
                    Err(AuthError::Eip1654ValidationFailed("rpc unavailable".into()))
                }
            }
        }
        let message = b"Decentraland Login";
        let v = FailsRaw(alloy_primitives::eip191_hash_message(message).0);
        assert!(
            verify_eip1654(&v, "0xcontract", message, &[0u8; 65])
                .await
                .unwrap()
        );
    }

    #[tokio::test]
    async fn propagates_rpc_error() {
        struct Failing;
        #[async_trait]
        impl Eip1654Validator for Failing {
            async fn validate_signature(
                &self,
                _c: &str,
                _h: &[u8],
                _s: &[u8],
            ) -> Result<bool, AuthError> {
                Err(AuthError::Eip1654ValidationFailed("boom".into()))
            }
        }
        let err = verify_eip1654(&Failing, "0xcontract", b"msg", &[0u8; 65])
            .await
            .unwrap_err();
        assert!(matches!(err, AuthError::Eip1654ValidationFailed(_)));
    }
}
