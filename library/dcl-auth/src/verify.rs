use tracing::debug;

use super::auth_chain::{
    AuthChain, AuthLink, AuthLinkType, MAX_AUTH_CHAIN_LINKS, is_valid_auth_chain,
    split_ephemeral_payload,
};
use super::eip1654::{Eip1654Validator, PeerValidated, verify_eip1654};
use super::error::AuthError;
use super::hex0x::{HexDecodeError, decode_hex_0x};
use super::recover::recover_address;

/// Walks the chain with plain ECDSA links only; an EIP-1654 link fails with
/// [`AuthError::Eip1654NotImplemented`], exactly as [`verify_auth_chain_async`]
/// does without a validator.
pub fn verify_auth_chain(
    chain: &AuthChain,
    expected_address: &str,
    now_ms: Option<i64>,
) -> Result<(), AuthError> {
    check_structure(chain)?;

    let now_ms = now_ms.unwrap_or_else(|| chrono::Utc::now().timestamp_millis());
    let mut current_authority = String::new();

    for (index, link) in chain.iter().enumerate() {
        current_authority = match link.link_type {
            AuthLinkType::SIGNER => signer_link(link),
            AuthLinkType::EcdsaSignedEntity => signed_entity_link(link, index, &current_authority)?,
            AuthLinkType::EcdsaEphemeral => {
                ephemeral_link(
                    link,
                    index,
                    &current_authority,
                    now_ms,
                    ExpirationPolicy::Strict,
                )?
                .0
            }
            AuthLinkType::EcdsaEip1654Ephemeral | AuthLinkType::EcdsaEip1654SignedEntity => {
                return Err(AuthError::Eip1654NotImplemented);
            }
        };
    }

    check_final_authority(&current_authority, expected_address)
}

/// What an ephemeral link whose expiration parses as no known date shape means.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExpirationPolicy {
    /// A local deployment: the payload is invalid and the chain is rejected.
    Strict,
    /// An entity mirrored from a peer: the peer accepted the payload when the entity was
    /// deployed (the old node read it with `new Date`), so the link is verified without the
    /// expiry check and the raw text is reported in the [`ChainVerdict`].
    TrustPeer,
}

/// A verified chain, with the expirations that were taken on the peer's word.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ChainVerdict {
    pub trusted_expirations: Vec<String>,
}

pub async fn verify_auth_chain_async(
    chain: &AuthChain,
    expected_address: &str,
    now_ms: Option<i64>,
    eip1654_validator: Option<&dyn Eip1654Validator>,
) -> Result<(), AuthError> {
    walk_chain(
        chain,
        expected_address,
        now_ms,
        eip1654_validator,
        ExpirationPolicy::Strict,
    )
    .await
    .map(|_| ())
}

/// The sync path's verification of an entity a peer serves: ECDSA links recovered locally,
/// ephemeral expiry checked against the entity timestamp, contract-wallet links and
/// expirations that parse as no known date taken as the peer's verdict ([`PeerValidated`],
/// [`ExpirationPolicy::TrustPeer`]).
pub async fn verify_peer_auth_chain(
    chain: &AuthChain,
    expected_address: &str,
    entity_timestamp_ms: i64,
) -> Result<ChainVerdict, AuthError> {
    walk_chain(
        chain,
        expected_address,
        Some(entity_timestamp_ms),
        Some(&PeerValidated),
        ExpirationPolicy::TrustPeer,
    )
    .await
}

async fn walk_chain(
    chain: &AuthChain,
    expected_address: &str,
    now_ms: Option<i64>,
    eip1654_validator: Option<&dyn Eip1654Validator>,
    policy: ExpirationPolicy,
) -> Result<ChainVerdict, AuthError> {
    check_structure(chain)?;

    let now_ms = now_ms.unwrap_or_else(|| chrono::Utc::now().timestamp_millis());
    let mut current_authority = String::new();
    let mut verdict = ChainVerdict::default();

    for (index, link) in chain.iter().enumerate() {
        current_authority = match link.link_type {
            AuthLinkType::SIGNER => signer_link(link),
            AuthLinkType::EcdsaSignedEntity => signed_entity_link(link, index, &current_authority)?,
            AuthLinkType::EcdsaEphemeral => {
                let (authority, trusted) =
                    ephemeral_link(link, index, &current_authority, now_ms, policy)?;
                verdict.trusted_expirations.extend(trusted);
                authority
            }

            AuthLinkType::EcdsaEip1654Ephemeral => {
                let validator = eip1654_validator.ok_or(AuthError::Eip1654NotImplemented)?;
                let signature = require_signature(link, index)?;
                let (message, ephemeral_address, expiration) =
                    ephemeral_payload_within(link, now_ms, policy)?;

                let sig_bytes = decode_hex_signature(&signature)?;

                let valid = verify_eip1654(
                    validator,
                    &current_authority,
                    message.as_bytes(),
                    &sig_bytes,
                )
                .await?;

                if !valid {
                    return Err(AuthError::Eip1654Rejected {
                        contract: current_authority.clone(),
                    });
                }

                debug!(
                    ephemeral = %ephemeral_address,
                    contract = %current_authority,
                    expiration_ms = expiration.as_ref().ok().copied(),
                    trusted_expiration = expiration.as_ref().err().map(String::as_str),
                    "ECDSA_EIP_1654_EPHEMERAL: contract validated, advancing authority"
                );
                verdict.trusted_expirations.extend(expiration.err());
                ephemeral_address
            }

            AuthLinkType::EcdsaEip1654SignedEntity => {
                let validator = eip1654_validator.ok_or(AuthError::Eip1654NotImplemented)?;
                let signature = require_signature(link, index)?;

                let sig_bytes = decode_hex_signature(&signature)?;

                let valid = verify_eip1654(
                    validator,
                    &current_authority,
                    link.payload.as_bytes(),
                    &sig_bytes,
                )
                .await?;

                if !valid {
                    return Err(AuthError::Eip1654Rejected {
                        contract: current_authority.clone(),
                    });
                }

                debug!(
                    link_index = index,
                    contract = %current_authority,
                    "ECDSA_EIP_1654_SIGNED_ENTITY: contract validated"
                );
                link.payload.clone()
            }
        };
    }

    check_final_authority(&current_authority, expected_address)?;
    Ok(verdict)
}

fn check_structure(chain: &AuthChain) -> Result<(), AuthError> {
    if chain.len() > MAX_AUTH_CHAIN_LINKS {
        return Err(AuthError::MalformedChain(format!(
            "auth chain too long: {} links (max {})",
            chain.len(),
            MAX_AUTH_CHAIN_LINKS
        )));
    }
    if !is_valid_auth_chain(chain) {
        return Err(AuthError::MalformedChain("invalid chain structure".into()));
    }
    // SIGNER, then ephemeral delegations, then one last signed link: an entity, or the
    // ephemeral link an identity chain ends on. A SIGNER alone is unsigned, and a signed
    // entity mid-chain would hand its payload a delegation with no expiration.
    if chain.len() < 2 {
        return Err(AuthError::MalformedChain(
            "auth chain has no signed link after the SIGNER".into(),
        ));
    }
    if let Some((index, link)) =
        chain[..chain.len() - 1]
            .iter()
            .enumerate()
            .skip(1)
            .find(|(_, link)| {
                !matches!(
                    link.link_type,
                    AuthLinkType::EcdsaEphemeral | AuthLinkType::EcdsaEip1654Ephemeral
                )
            })
    {
        return Err(AuthError::MalformedChain(format!(
            "{} at index {index} must be the last link",
            link.link_type
        )));
    }
    Ok(())
}

fn check_final_authority(current_authority: &str, expected_address: &str) -> Result<(), AuthError> {
    if current_authority != expected_address {
        return Err(AuthError::FinalAuthorityMismatch {
            expected: expected_address.to_string(),
            actual: current_authority.to_string(),
        });
    }
    Ok(())
}

fn signer_link(link: &AuthLink) -> String {
    debug!(address = %link.payload, "SIGNER link: set initial authority");
    link.payload.clone()
}

fn signed_entity_link(
    link: &AuthLink,
    index: usize,
    current_authority: &str,
) -> Result<String, AuthError> {
    let signature = require_signature(link, index)?;
    let recovered = recover_address(link.payload.as_bytes(), &signature)?;

    check_address_match(current_authority, &recovered, index)?;

    debug!(
        link_index = index,
        signer = %recovered,
        "ECDSA_SIGNED_ENTITY: verified"
    );
    Ok(link.payload.clone())
}

/// The ephemeral address the link delegates to, and the raw expiration when the policy took
/// it on the peer's word.
fn ephemeral_link(
    link: &AuthLink,
    index: usize,
    current_authority: &str,
    now_ms: i64,
    policy: ExpirationPolicy,
) -> Result<(String, Option<String>), AuthError> {
    let signature = require_signature(link, index)?;
    let (message, ephemeral_address, expiration) = ephemeral_payload_within(link, now_ms, policy)?;

    let recovered = recover_address(message.as_bytes(), &signature)?;

    check_address_match(current_authority, &recovered, index)?;

    debug!(
        ephemeral = %ephemeral_address,
        signer = %recovered,
        expiration_ms = expiration.as_ref().ok().copied(),
        trusted_expiration = expiration.as_ref().err().map(String::as_str),
        "ECDSA_EPHEMERAL: verified, advancing authority to ephemeral address"
    );
    Ok((ephemeral_address, expiration.err()))
}

/// The payload's message and ephemeral address once its expiration passed the check
/// against `now_ms`; the expiration comes back as its raw text (`Err`) when it parses as
/// no known date and the policy trusts the peer for it.
fn ephemeral_payload_within(
    link: &AuthLink,
    now_ms: i64,
    policy: ExpirationPolicy,
) -> Result<(String, String, Result<i64, String>), AuthError> {
    let (message, ephemeral_address, expiration) = split_ephemeral_payload(&link.payload)?;
    match expiration {
        Ok(expiration_ms) if expiration_ms <= now_ms => Err(AuthError::EphemeralExpired {
            expiration_ms,
            now_ms,
        }),
        Ok(expiration_ms) => Ok((message, ephemeral_address, Ok(expiration_ms))),
        Err(raw) => match policy {
            ExpirationPolicy::Strict => Err(AuthError::InvalidEphemeralPayload(format!(
                "Invalid expiration date '{}'",
                raw
            ))),
            ExpirationPolicy::TrustPeer => Ok((message, ephemeral_address, Err(raw))),
        },
    }
}

fn decode_hex_signature(hex_str: &str) -> Result<Vec<u8>, AuthError> {
    decode_hex_0x(hex_str).map_err(|e| match e {
        HexDecodeError::OddLength => AuthError::RecoveryFailed("Odd-length signature hex".into()),
        HexDecodeError::InvalidChar { c, .. } if !c.is_ascii() => {
            AuthError::RecoveryFailed("Non-ASCII signature hex".into())
        }
        HexDecodeError::InvalidChar { .. } => {
            AuthError::RecoveryFailed("Hex decode: invalid digit found in string".into())
        }
    })
}

fn require_signature(link: &AuthLink, index: usize) -> Result<String, AuthError> {
    match &link.signature {
        Some(sig) if !sig.is_empty() => Ok(sig.clone()),
        _ => Err(AuthError::MissingSignature {
            link_type: link.link_type.to_string(),
            index,
        }),
    }
}

fn check_address_match(expected: &str, actual: &str, index: usize) -> Result<(), AuthError> {
    if expected.to_lowercase() != actual.to_lowercase() {
        return Err(AuthError::SignerMismatch {
            index,
            expected: expected.to_lowercase(),
            actual: actual.to_lowercase(),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::super::auth_chain::{AuthLink, AuthLinkType};
    use super::*;
    use std::io::Write;
    use std::sync::{Arc, Mutex};

    #[derive(Clone, Default)]
    struct LogBuffer(Arc<Mutex<Vec<u8>>>);

    impl Write for LogBuffer {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(bytes);
            Ok(bytes.len())
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    /// SIGNER plus one ECDSA_SIGNED_ENTITY over `payload`, signed by a fixed local key.
    fn signed_entity_chain(payload: &str) -> AuthChain {
        use alloy::signers::{SignerSync, local::PrivateKeySigner};

        let key: PrivateKeySigner =
            "ac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80"
                .parse()
                .unwrap();
        vec![
            AuthLink {
                link_type: AuthLinkType::SIGNER,
                payload: format!("{:#x}", key.address()),
                signature: None,
            },
            AuthLink {
                link_type: AuthLinkType::EcdsaSignedEntity,
                payload: payload.into(),
                signature: Some(
                    key.sign_message_sync(payload.as_bytes())
                        .unwrap()
                        .to_string(),
                ),
            },
        ]
    }

    #[test]
    fn test_empty_chain_rejected() {
        let result = verify_auth_chain(&vec![], "0x123", Some(0));
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), AuthError::MalformedChain(_)));
    }

    #[test]
    fn test_missing_signature_rejected() {
        let chain = vec![
            AuthLink {
                link_type: AuthLinkType::SIGNER,
                payload: "0xabc".into(),
                signature: None,
            },
            AuthLink {
                link_type: AuthLinkType::EcdsaSignedEntity,
                payload: "hash".into(),
                signature: None,
            },
        ];
        let result = verify_auth_chain(&chain, "hash", Some(0));
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            AuthError::MissingSignature { .. }
        ));
    }

    #[test]
    fn test_eip1654_returns_not_implemented() {
        let chain = vec![
            AuthLink {
                link_type: AuthLinkType::SIGNER,
                payload: "0xabc".into(),
                signature: None,
            },
            AuthLink {
                link_type: AuthLinkType::EcdsaEip1654Ephemeral,
                payload: "Decentraland Login\nEphemeral address: 0x123\nExpiration: 2099-01-01T00:00:00.000Z".into(),
                signature: Some("0xdeadbeef".into()),
            },
        ];
        let result = verify_auth_chain(&chain, "0x123", Some(0));
        assert!(matches!(
            result.unwrap_err(),
            AuthError::Eip1654NotImplemented
        ));
    }

    #[test]
    fn test_final_authority_mismatch() {
        let chain = signed_entity_chain("0xabc");
        let result = verify_auth_chain(&chain, "0xdifferent", Some(0));
        assert!(matches!(
            result.unwrap_err(),
            AuthError::FinalAuthorityMismatch { .. }
        ));
    }

    #[test]
    fn test_final_authority_case_sensitive() {
        let chain = signed_entity_chain("0xABC");
        let result = verify_auth_chain(&chain, "0xabc", Some(0));
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            AuthError::FinalAuthorityMismatch { .. }
        ));

        let result = verify_auth_chain(&chain, "0xABC", Some(0));
        assert!(result.is_ok());
    }

    async fn ecdsa_chain_expiring(expiration: &str) -> (AuthChain, &'static str) {
        use alloy::signers::{Signer, local::PrivateKeySigner};

        let root_key: PrivateKeySigner =
            "ac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80"
                .parse()
                .unwrap();
        let ephemeral_key: PrivateKeySigner =
            "59c6995e998f97a5a0044966f0945389dc9e86dae88c7a8412f4603b6b78690d"
                .parse()
                .unwrap();
        let ephemeral_payload = format!(
            "Decentraland Login\nEphemeral address: {:#x}\nExpiration: {expiration}",
            ephemeral_key.address()
        );
        let entity_payload = "bafybeigdyrzt5sfp7udm7hu76uh7y26nf3efuylqabf3oclgtqy55fbzdi";
        let chain = vec![
            AuthLink {
                link_type: AuthLinkType::SIGNER,
                payload: format!("{:#x}", root_key.address()),
                signature: None,
            },
            AuthLink {
                link_type: AuthLinkType::EcdsaEphemeral,
                payload: ephemeral_payload.clone(),
                signature: Some(
                    root_key
                        .sign_message(ephemeral_payload.as_bytes())
                        .await
                        .unwrap()
                        .to_string(),
                ),
            },
            AuthLink {
                link_type: AuthLinkType::EcdsaSignedEntity,
                payload: entity_payload.to_string(),
                signature: Some(
                    ephemeral_key
                        .sign_message(entity_payload.as_bytes())
                        .await
                        .unwrap()
                        .to_string(),
                ),
            },
        ];
        (chain, entity_payload)
    }

    /// A SIGNER link carries no signature, so a chain of it alone proves nothing.
    #[test]
    fn a_signer_alone_proves_nothing() {
        let address = "0xf39fd6e51aad88f6f4ce6ab8827279cfffb92266";
        let chain = vec![AuthLink {
            link_type: AuthLinkType::SIGNER,
            payload: address.into(),
            signature: None,
        }];
        let err = verify_auth_chain(&chain, address, Some(0)).unwrap_err();
        assert!(matches!(err, AuthError::MalformedChain(_)), "{err}");
    }

    /// A signed entity ends the chain: its payload is data, not an address that may sign on
    /// (without the expiration an ephemeral link would carry).
    #[tokio::test]
    async fn a_signed_entity_cannot_delegate_mid_chain() {
        use alloy::signers::{Signer, local::PrivateKeySigner};

        let root: PrivateKeySigner =
            "ac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80"
                .parse()
                .unwrap();
        let delegate: PrivateKeySigner =
            "59c6995e998f97a5a0044966f0945389dc9e86dae88c7a8412f4603b6b78690d"
                .parse()
                .unwrap();
        let delegate_address = format!("{:#x}", delegate.address());
        let entity = "bafybeigdyrzt5sfp7udm7hu76uh7y26nf3efuylqabf3oclgtqy55fbzdi";
        let sign = async |key: &PrivateKeySigner, text: &str| {
            Some(key.sign_message(text.as_bytes()).await.unwrap().to_string())
        };
        let chain = vec![
            AuthLink {
                link_type: AuthLinkType::SIGNER,
                payload: format!("{:#x}", root.address()),
                signature: None,
            },
            AuthLink {
                link_type: AuthLinkType::EcdsaSignedEntity,
                payload: delegate_address.clone(),
                signature: sign(&root, &delegate_address).await,
            },
            AuthLink {
                link_type: AuthLinkType::EcdsaSignedEntity,
                payload: entity.into(),
                signature: sign(&delegate, entity).await,
            },
        ];
        let err = verify_auth_chain(&chain, entity, Some(0)).unwrap_err();
        assert!(matches!(err, AuthError::MalformedChain(_)), "{err}");
        let err = verify_auth_chain_async(&chain, entity, Some(0), None)
            .await
            .unwrap_err();
        assert!(matches!(err, AuthError::MalformedChain(_)), "{err}");
    }

    /// An identity chain ends on its ephemeral link and names the ephemeral address.
    #[tokio::test]
    async fn an_identity_chain_ends_on_its_ephemeral_link() {
        let (mut chain, _) = ecdsa_chain_expiring("2099-01-01T00:00:00.000Z").await;
        chain.pop();
        let (_, ephemeral, _) =
            super::super::auth_chain::parse_ephemeral_payload(&chain[1].payload).unwrap();
        verify_auth_chain(&chain, &ephemeral, Some(0)).unwrap();
    }

    /// An ephemeral link is valid while its expiration is after `now`, as upstream's
    /// `expiration > dateToValidateExpirationInMillis`.
    #[tokio::test]
    async fn an_ephemeral_link_expires_at_its_expiration() {
        let (chain, entity) = ecdsa_chain_expiring("2020-02-13T18:31:10.000Z").await;
        let expiration = 1_581_618_670_000;
        verify_auth_chain(&chain, entity, Some(expiration - 1)).unwrap();
        let err = verify_auth_chain(&chain, entity, Some(expiration)).unwrap_err();
        assert!(matches!(err, AuthError::EphemeralExpired { .. }), "{err}");
    }

    /// A `Date.toString()` expiration is a date on both paths; one that parses as nothing is
    /// invalid locally and the peer's verdict when the entity is mirrored, with the ECDSA
    /// recovery still enforced around it.
    #[tokio::test]
    async fn an_unparseable_expiration_is_rejected_locally_and_trusted_from_a_peer() {
        let feb_2020_ms = 1_581_000_000_000;
        let (js, entity) =
            ecdsa_chain_expiring("Thu Feb 13 2020 15:31:10 GMT-0300 (Argentina Standard Time)")
                .await;
        verify_auth_chain_async(&js, entity, Some(feb_2020_ms), None)
            .await
            .expect("the JS date parses");
        verify_auth_chain(&js, entity, Some(feb_2020_ms)).expect("the JS date parses");
        let expired = verify_auth_chain_async(&js, entity, Some(1_581_618_670_000), None)
            .await
            .unwrap_err();
        assert!(
            matches!(expired, AuthError::EphemeralExpired { .. }),
            "{expired}"
        );
        assert_eq!(
            verify_peer_auth_chain(&js, entity, feb_2020_ms)
                .await
                .unwrap(),
            ChainVerdict::default()
        );

        let (unknown, entity) = ecdsa_chain_expiring("next Tuesday").await;
        let err = verify_auth_chain_async(&unknown, entity, Some(feb_2020_ms), None)
            .await
            .unwrap_err();
        assert_eq!(
            err.to_string(),
            "Invalid ephemeral payload: Invalid expiration date 'next Tuesday'"
        );
        assert!(verify_auth_chain(&unknown, entity, Some(feb_2020_ms)).is_err());
        let verdict = verify_peer_auth_chain(&unknown, entity, feb_2020_ms)
            .await
            .unwrap();
        assert_eq!(verdict.trusted_expirations, ["next Tuesday"]);

        let mut forged = unknown.clone();
        forged[0].payload = "0x0000000000000000000000000000000000000001".into();
        let err = verify_peer_auth_chain(&forged, entity, feb_2020_ms)
            .await
            .unwrap_err();
        assert!(matches!(err, AuthError::SignerMismatch { .. }), "{err}");
    }

    #[tokio::test]
    async fn test_real_ecdsa_auth_chain() {
        use alloy::signers::{Signer, local::PrivateKeySigner};

        let root_key: PrivateKeySigner =
            "ac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80"
                .parse()
                .unwrap();
        let root_address = format!("{:#x}", root_key.address());

        let ephemeral_key: PrivateKeySigner =
            "59c6995e998f97a5a0044966f0945389dc9e86dae88c7a8412f4603b6b78690d"
                .parse()
                .unwrap();
        let ephemeral_address = format!("{:#x}", ephemeral_key.address());

        let ephemeral_payload = format!(
            "Decentraland Login\nEphemeral address: {}\nExpiration: 2099-01-01T00:00:00.000Z",
            ephemeral_address
        );

        let ephemeral_sig = root_key
            .sign_message(ephemeral_payload.as_bytes())
            .await
            .unwrap();
        let ephemeral_sig_hex = ephemeral_sig.to_string();

        let entity_payload = "bafybeigdyrzt5sfp7udm7hu76uh7y26nf3efuylqabf3oclgtqy55fbzdi";

        let entity_sig = ephemeral_key
            .sign_message(entity_payload.as_bytes())
            .await
            .unwrap();
        let entity_sig_hex = entity_sig.to_string();

        let chain = vec![
            AuthLink {
                link_type: AuthLinkType::SIGNER,
                payload: root_address.clone(),
                signature: None,
            },
            AuthLink {
                link_type: AuthLinkType::EcdsaEphemeral,
                payload: ephemeral_payload,
                signature: Some(ephemeral_sig_hex),
            },
            AuthLink {
                link_type: AuthLinkType::EcdsaSignedEntity,
                payload: entity_payload.to_string(),
                signature: Some(entity_sig_hex),
            },
        ];

        let result = verify_auth_chain(&chain, entity_payload, Some(0));
        assert!(
            result.is_ok(),
            "Real ECDSA auth chain should verify: {:?}",
            result
        );

        let bad_result = verify_auth_chain(&chain, "wrong-payload", Some(0));
        assert!(bad_result.is_err());
        assert!(matches!(
            bad_result.unwrap_err(),
            AuthError::FinalAuthorityMismatch { .. }
        ));
    }

    /// The old test built the chain with the crypto crate's `Wallet` test signer
    /// (left out of v2 with the rest of the signing code); an alloy local signer
    /// produces the same two-link chain shape as `create_simple_auth_chain`.
    #[tokio::test(flavor = "current_thread")]
    async fn signed_entity_verification_logs_never_retain_the_payload() {
        use alloy::signers::{SignerSync, local::PrivateKeySigner};

        const KEY: &str = "0x4c0883a69102937d6231471b5dbb6204fe5129617082792ae468d01a3f362318";
        const CANARY: &str = "crypto-signed-entity-private-material-canary";
        let wallet: PrivateKeySigner = KEY.parse().unwrap();
        let signature = wallet
            .sign_message_sync(CANARY.as_bytes())
            .unwrap()
            .to_string();
        let chain: AuthChain = serde_json::from_value(serde_json::json!([
            { "type": "SIGNER", "payload": format!("{:#x}", wallet.address()), "signature": "" },
            { "type": "ECDSA_SIGNED_ENTITY", "payload": CANARY, "signature": signature },
        ]))
        .unwrap();
        let buffer = LogBuffer::default();
        let writer = buffer.clone();
        let _forces_per_thread_interest =
            tracing::Dispatch::new(tracing::subscriber::NoSubscriber::default());
        let subscriber = tracing_subscriber::fmt()
            .with_max_level(tracing::Level::TRACE)
            .without_time()
            .with_ansi(false)
            .with_writer(move || writer.clone())
            .finish();
        let _guard = tracing::subscriber::set_default(subscriber);

        verify_auth_chain(&chain, CANARY, Some(0)).unwrap();
        verify_auth_chain_async(&chain, CANARY, Some(0), None)
            .await
            .unwrap();

        let logs = String::from_utf8(buffer.0.lock().unwrap().clone()).unwrap();
        assert_eq!(logs.matches("ECDSA_SIGNED_ENTITY: verified").count(), 2);
        assert!(!logs.contains(CANARY), "signed payload survived in {logs}");
    }
}
