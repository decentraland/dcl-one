//! The benchmark autopilot's guest on desktop: an account of its own, whose root key the engine
//! keeps in `AppConfig::guest_root_key`. The root key signs each ephemeral delegation the way a
//! sign-in's wallet does, and a delegation lasts [`IDENTITY_LIFETIME`], so an expired identity is
//! signed again with the same root key and the account (its address, its profile) never changes.
//! The web shell does the same with its own key (react-web `shell/localGuest.ts`).

use std::time::Duration;

use alloy_core::primitives::hex;
use alloy_signer::SignerSync;
use alloy_signer_local::PrivateKeySigner;
use common::structs::ChainLink;

/// How long an identity the guest's root key signed stays valid (the web guest's, too).
pub const IDENTITY_LIFETIME: Duration = Duration::from_secs(30 * 24 * 3600);

/// A new root key for a guest account.
pub fn new_root_key() -> PrivateKeySigner {
    PrivateKeySigner::random_with(&mut rand::thread_rng())
}

/// The guest's identity as the wallet takes it: the ephemeral key and the delegate chain (the
/// `ECDSA_EPHEMERAL` link, signed by the root key), valid from `now_millis` for
/// [`IDENTITY_LIFETIME`]. Every call is a new ephemeral key for the same account.
pub fn sign_identity(
    root: &PrivateKeySigner,
    now_millis: i64,
) -> Result<(PrivateKeySigner, Vec<ChainLink>), anyhow::Error> {
    let ephemeral = PrivateKeySigner::random_with(&mut rand::thread_rng());
    let expiration =
        chrono::DateTime::from_timestamp_millis(now_millis + IDENTITY_LIFETIME.as_millis() as i64)
            .ok_or_else(|| anyhow::anyhow!("expiration out of range"))?
            .to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
    let payload = format!(
        "Decentraland Login\nEphemeral address: {}\nExpiration: {expiration}",
        ephemeral.address().to_checksum(None)
    );
    let signature = root.sign_message_sync(payload.as_bytes())?;
    let delegates = vec![ChainLink {
        ty: "ECDSA_EPHEMERAL".to_owned(),
        payload,
        signature: hex::encode_prefixed(signature.as_bytes()),
    }];
    Ok((ephemeral, delegates))
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use alloy_core::primitives::Address;
    use alloy_signer::Signature;

    use super::*;

    const NOW: i64 = 1_800_000_000_000;

    /// the root address the delegate link's signature recovers to
    fn signer_of(link: &ChainLink) -> Address {
        Signature::from_str(&link.signature)
            .unwrap()
            .recover_address_from_msg(link.payload.as_bytes())
            .unwrap()
    }

    #[test]
    fn the_root_key_delegates_a_fresh_ephemeral_key_for_thirty_days() {
        let root = new_root_key();
        let (ephemeral, delegates) = sign_identity(&root, NOW).unwrap();

        let [link] = delegates.as_slice() else {
            panic!("one delegate link")
        };
        assert_eq!(link.ty, "ECDSA_EPHEMERAL");
        assert_eq!(signer_of(link), root.address());
        assert_ne!(ephemeral.address(), root.address());
        let expiration = (NOW as u64 + IDENTITY_LIFETIME.as_millis() as u64) as i64;
        let expiration = chrono::DateTime::from_timestamp_millis(expiration)
            .unwrap()
            .to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
        assert_eq!(
            link.payload,
            format!(
                "Decentraland Login\nEphemeral address: {}\nExpiration: {expiration}",
                ephemeral.address().to_checksum(None)
            )
        );
    }

    #[test]
    fn signing_again_keeps_the_account_and_changes_the_ephemeral_key() {
        let root = new_root_key();
        let (first, first_delegates) = sign_identity(&root, NOW).unwrap();
        let (second, second_delegates) = sign_identity(&root, NOW + 31 * 24 * 3600 * 1000).unwrap();
        assert_ne!(first.address(), second.address());
        assert_eq!(
            signer_of(&first_delegates[0]),
            signer_of(&second_delegates[0])
        );
        assert_ne!(first_delegates[0].payload, second_delegates[0].payload);
    }
}
