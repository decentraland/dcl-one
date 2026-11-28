//! Test identities: a wallet and a device key derived from text, and the three signatures the
//! stack asks for (the Pulse handshake, the archipelago challenge, a signed fetch). The keys the
//! tool derives own nothing and their mandates last until 2099. A wallet whose key is given (one
//! the servers admit) delegates to a random device key, by a mandate that lasts
//! [`WALLET_KEY_MANDATE`].

use catalyst_comms::pulse::decentraland::pulse::{
    ClientMessage, HandshakeRequest, PlayerInitialState, SceneListenerAoi,
    SceneListenerHandshakeRequest, client_message,
};
use dcl_auth::signed_fetch::{
    AUTH_CHAIN_HEADER_PREFIX, AUTH_METADATA_HEADER, AUTH_TIMESTAMP_HEADER, build_payload_v6,
};
use dcl_auth::{AuthLink, AuthLinkType, Wallet};
use sha2::{Digest, Sha256};

/// How long the mandate of a wallet whose key is given lasts.
pub const WALLET_KEY_MANDATE: std::time::Duration = std::time::Duration::from_secs(15 * 60);

/// The metadata the explorer signs its own requests with.
pub const EXPLORER_METADATA: &str = "{\"signer\":\"dcl:explorer\"}";

/// A wallet and the device key it delegates to. Two devices of one wallet share the wallet and
/// differ in the device key, which is what the servers call a session.
pub struct Identity {
    wallet: Wallet,
    device: Wallet,
    material: String,
    index: u32,
    device_number: u32,
    mandate: Mandate,
    /// The profile version its Pulse handshakes announce: 0, none, unless a profile was deployed
    /// for the wallet ([`Identity::with_profile_version`]).
    profile_version: i32,
}

/// How a wallet delegates to its device key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mandate {
    /// A derived device key, under a mandate that expires in 2099.
    Derived,
    /// A random device key, under a mandate that expires at this Unix millisecond.
    Until(i64),
}

/// The expiry line of a derived identity's mandate.
const DERIVED_EXPIRATION: &str = "2099-01-01T00:00:00.000Z";

/// A random device key and the end of its mandate, [`WALLET_KEY_MANDATE`] from now.
fn random_device() -> anyhow::Result<(Wallet, Mandate)> {
    let device = Wallet::random().map_err(|error| anyhow::anyhow!("no device key: {error}"))?;
    let until = now_ms() + crate::casts::millis_i64(WALLET_KEY_MANDATE);
    Ok((device, Mandate::Until(until)))
}

impl std::fmt::Debug for Identity {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Identity")
            .field("address", &self.address())
            .field("session", &self.session())
            .finish()
    }
}

fn derived_key(purpose: &str, material: &str, index: u32, counter: u32) -> Wallet {
    // A digest that is not a valid key is astronomically unlikely; a salt keeps the loop honest.
    for salt in 0u32.. {
        let digest = Sha256::new()
            .chain_update(b"comms-e2e test key\0")
            .chain_update(purpose.as_bytes())
            .chain_update([0])
            .chain_update(material.as_bytes())
            .chain_update([0])
            .chain_update(index.to_be_bytes())
            .chain_update(counter.to_be_bytes())
            .chain_update(salt.to_be_bytes())
            .finalize();
        if let Ok(wallet) = Wallet::from_hex(&hex::encode(digest)) {
            return wallet;
        }
    }
    unreachable!("some salt yields a valid key")
}

impl Identity {
    /// The identity for `index` under `material` (a run, a scenario, a repeat). The same inputs
    /// always give the same wallet.
    pub fn derive(material: &str, index: u32) -> Identity {
        Identity {
            wallet: derived_key("wallet", material, index, 0),
            device: derived_key("device", material, index, 0),
            material: material.to_owned(),
            index,
            device_number: 0,
            mandate: Mandate::Derived,
            profile_version: 0,
        }
    }

    /// The identity of visiting player `index` of the crowd labelled `label`: the same wallet on
    /// every run under that label, and never one a run's own players have, whose material holds
    /// the run's id.
    pub fn visitor(label: &str, index: u32) -> Identity {
        Identity::derive(&format!("crowd-visit/{label}"), index)
    }

    /// The same identity, its handshakes announcing profile `version`: the version of the
    /// profile deployed for its wallet.
    pub fn with_profile_version(mut self, version: i32) -> Identity {
        self.profile_version = version;
        self
    }

    /// The profile version its Pulse handshakes announce.
    pub fn profile_version(&self) -> i32 {
        self.profile_version
    }

    /// The identity of a wallet whose private key is given (a scene listener's wallet, which the
    /// server is configured to admit). Its device key is random and its mandate expires
    /// [`WALLET_KEY_MANDATE`] after this call, so a handshake has to be signed within that time.
    /// The key is never printed or kept beyond the wallet it builds.
    pub fn from_wallet_key(private_key_hex: &str, material: &str) -> anyhow::Result<Identity> {
        let wallet = Wallet::from_hex(private_key_hex.trim())
            .map_err(|_| anyhow::anyhow!("the wallet key is not a valid private key"))?;
        let (device, mandate) = random_device()?;
        Ok(Identity {
            wallet,
            device,
            material: material.to_owned(),
            index: 0,
            device_number: 0,
            mandate,
            profile_version: 0,
        })
    }

    /// The wallet of a headless explorer started with `--wallet-seed <seed>`, with a device key of
    /// its own derived from `material`: the explorer's wallet on another device. A seeded guest
    /// explorer signs with its wallet key itself; that key is drawn from the standard generator of
    /// `rand` 0.8 (ChaCha with 12 rounds) seeded with the seed's eight little-endian bytes followed
    /// by zeros, as the explorer's wallet does.
    pub fn seeded_guest_wallet(seed: u64, material: &str) -> Identity {
        use rand_chacha::rand_core::SeedableRng as _;
        let mut bytes = [0u8; 32];
        bytes[..8].copy_from_slice(&seed.to_le_bytes());
        let key = k256::ecdsa::SigningKey::random(&mut rand_chacha::ChaCha12Rng::from_seed(bytes));
        Identity {
            wallet: Wallet::from_hex(&hex::encode(key.to_bytes()))
                .expect("a signing key is a valid private key"),
            device: derived_key("guest device", material, 0, 0),
            material: material.to_owned(),
            index: 0,
            device_number: 0,
            mandate: Mandate::Derived,
            profile_version: 0,
        }
    }

    /// The same wallet on another device: a new session key, random with a fresh short mandate
    /// for a wallet whose key was given.
    pub fn second_device(&self) -> Identity {
        let device_number = self.device_number + 1;
        let (device, mandate) = match self.mandate {
            Mandate::Derived => (
                derived_key("device", &self.material, self.index, device_number),
                Mandate::Derived,
            ),
            Mandate::Until(_) => random_device().expect("the system has entropy"),
        };
        Identity {
            wallet: Wallet::from_hex(&self.wallet.private_key_hex()).expect("a key that was valid"),
            device,
            material: self.material.clone(),
            index: self.index,
            device_number,
            mandate,
            profile_version: self.profile_version,
        }
    }

    /// The same wallet and the same device, as a separate value.
    pub fn same_device(&self) -> Identity {
        Identity {
            wallet: Wallet::from_hex(&self.wallet.private_key_hex()).expect("a key that was valid"),
            device: Wallet::from_hex(&self.device.private_key_hex()).expect("a key that was valid"),
            material: self.material.clone(),
            index: self.index,
            device_number: self.device_number,
            mandate: self.mandate,
            profile_version: self.profile_version,
        }
    }

    /// The wallet address, lowercase `0x…`.
    pub fn address(&self) -> String {
        self.wallet.address()
    }

    /// The device key's address: the session the servers tell two devices apart by.
    pub fn session(&self) -> String {
        self.device.address()
    }

    /// SIGNER, the wallet's mandate for the device key, and `payload` signed by the device key.
    pub fn auth_chain(&self, payload: &str) -> Vec<AuthLink> {
        let expiration = match self.mandate {
            Mandate::Derived => DERIVED_EXPIRATION.to_owned(),
            Mandate::Until(until) => chrono::DateTime::from_timestamp_millis(until)
                .expect("a time within chrono's range")
                .to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
        };
        let mandate = format!(
            "Decentraland Login\nEphemeral address: {}\nExpiration: {expiration}",
            self.device.address()
        );
        let mandate_signature = self
            .wallet
            .sign_message(mandate.as_bytes())
            .expect("a derived key signs");
        let payload_signature = self
            .device
            .sign_message(payload.as_bytes())
            .expect("a derived key signs");
        vec![
            AuthLink {
                link_type: AuthLinkType::SIGNER,
                payload: self.wallet.address(),
                signature: None,
            },
            AuthLink {
                link_type: AuthLinkType::EcdsaEphemeral,
                payload: mandate,
                signature: Some(mandate_signature),
            },
            AuthLink {
                link_type: AuthLinkType::EcdsaSignedEntity,
                payload: payload.to_owned(),
                signature: Some(payload_signature),
            },
        ]
    }

    /// The header set of a signed fetch, as `(name, value)` pairs.
    pub fn signed_fetch(
        &self,
        method: &str,
        path: &str,
        metadata: &str,
        timestamp_ms: i64,
    ) -> Vec<(String, String)> {
        let timestamp = timestamp_ms.to_string();
        let payload = build_payload_v6(method, path, &timestamp, metadata);
        let mut headers: Vec<(String, String)> = self
            .auth_chain(&payload)
            .iter()
            .enumerate()
            .map(|(position, link)| {
                (
                    format!("{AUTH_CHAIN_HEADER_PREFIX}{position}"),
                    serde_json::to_string(link).expect("a link serializes"),
                )
            })
            .collect();
        headers.push((AUTH_TIMESTAMP_HEADER.to_owned(), timestamp));
        headers.push((AUTH_METADATA_HEADER.to_owned(), metadata.to_owned()));
        headers
    }

    /// The `auth_chain` bytes of a Pulse handshake: the signed-fetch headers of `connect /` as one
    /// JSON object.
    pub fn pulse_auth_chain(&self, timestamp_ms: i64) -> Vec<u8> {
        let bag: serde_json::Map<String, serde_json::Value> = self
            .signed_fetch("connect", "/", EXPLORER_METADATA, timestamp_ms)
            .into_iter()
            .map(|(name, value)| (name, serde_json::Value::String(value)))
            .collect();
        serde_json::to_vec(&bag).expect("a map of strings serializes")
    }

    /// A player's Pulse handshake, announcing the identity's profile version.
    pub fn pulse_handshake(
        &self,
        timestamp_ms: i64,
        initial_state: Option<PlayerInitialState>,
    ) -> ClientMessage {
        ClientMessage {
            message: Some(client_message::Message::Handshake(HandshakeRequest {
                auth_chain: self.pulse_auth_chain(timestamp_ms),
                profile_version: self.profile_version,
                initial_state,
                protocol_features: 0,
            })),
        }
    }

    /// A scene listener's Pulse handshake.
    pub fn pulse_listener_handshake(
        &self,
        timestamp_ms: i64,
        area_of_interest: Vec<SceneListenerAoi>,
    ) -> ClientMessage {
        ClientMessage {
            message: Some(client_message::Message::SceneListenerHandshake(
                SceneListenerHandshakeRequest {
                    auth_chain: self.pulse_auth_chain(timestamp_ms),
                    aoi: area_of_interest,
                    protocol_features: 0,
                },
            )),
        }
    }

    /// The auth chain JSON an archipelago `SignedChallenge` carries.
    pub fn archipelago_chain(&self, challenge: &str) -> String {
        serde_json::to_string(&self.auth_chain(challenge)).expect("a chain serializes")
    }
}

/// Now, in Unix milliseconds: the timestamp a signature made at this moment carries.
pub fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(crate::casts::millis_i64)
        .expect("the clock is past 1970")
}

#[cfg(test)]
mod tests {
    use super::*;
    use catalyst_comms::pulse::handshake::MAX_TIMESTAMP_SKEW_MS;
    use catalyst_comms::pulse::verify_handshake;

    #[test]
    fn identity_is_stable_and_scoped() {
        let first = Identity::derive("run-a/pulse-smoke/0", 3);
        assert_eq!(
            first.address(),
            Identity::derive("run-a/pulse-smoke/0", 3).address()
        );
        assert_ne!(
            first.address(),
            Identity::derive("run-a/pulse-smoke/0", 4).address()
        );
        assert_ne!(
            first.address(),
            Identity::derive("run-b/pulse-smoke/0", 3).address()
        );
        assert!(first.address().starts_with("0x") && first.address().len() == 42);
        assert_eq!(first.address(), first.address().to_lowercase());

        let second = first.second_device();
        assert_eq!(second.address(), first.address());
        assert_ne!(second.session(), first.session());
        assert_ne!(second.second_device().session(), second.session());
        assert_eq!(first.same_device().session(), first.session());
    }

    /// The profile version a handshake of `identity` announces.
    fn announced_version(identity: &Identity) -> i32 {
        let ClientMessage {
            message: Some(client_message::Message::Handshake(request)),
        } = identity.pulse_handshake(now_ms(), None)
        else {
            panic!("a handshake message");
        };
        request.profile_version
    }

    #[test]
    fn a_handshake_announces_the_profile_the_identity_has() {
        let plain = Identity::derive("run/pulse-smoke/0", 0);
        assert_eq!(plain.profile_version(), 0);
        assert_eq!(announced_version(&plain), 0);
        let visitor = Identity::visitor("crowd-a", 3).with_profile_version(1);
        assert_eq!(announced_version(&visitor), 1);
        assert_eq!(announced_version(&visitor.second_device()), 1);
        assert_eq!(announced_version(&visitor.same_device()), 1);
        // the handshake still verifies
        let now = now_ms();
        let ClientMessage {
            message: Some(client_message::Message::Handshake(request)),
        } = visitor.pulse_handshake(now, None)
        else {
            panic!("a handshake message");
        };
        assert_eq!(
            verify_handshake(&request, now).unwrap().user_address,
            visitor.address()
        );
    }

    #[test]
    fn a_visitor_is_the_same_wallet_on_every_run_of_its_label() {
        let first = Identity::visitor("crowd-a", 3);
        assert_eq!(first.address(), Identity::visitor("crowd-a", 3).address());
        assert_ne!(first.address(), Identity::visitor("crowd-a", 4).address());
        assert_ne!(first.address(), Identity::visitor("crowd-b", 3).address());
        // and never a wallet a run's own players have
        assert_ne!(first.address(), Identity::derive("crowd-a", 3).address());
    }

    #[test]
    fn a_seeded_guest_wallet_is_the_explorers_on_another_device() {
        // the addresses the explorer's own tool prints for these seeds (`harness-util address
        // --seed <n>`), which a headless explorer started with `--wallet-seed <n>` signs as
        for (seed, address) in [
            (7, "0x16af891c5e59ba6e773680580ee412c0a176a48b"),
            (
                12_345_678_901_234_567,
                "0xc1f0b32491674b401d8aaa04ce8320a94c6dd839",
            ),
        ] {
            let identity = Identity::seeded_guest_wallet(seed, "another device");
            assert_eq!(identity.address(), address, "seed {seed}");
            // its own session key, not the wallet key the explorer signs with
            assert_ne!(identity.session(), identity.address());
            assert_ne!(
                identity.session(),
                Identity::seeded_guest_wallet(seed, "a third device").session()
            );
        }
        let identity = Identity::seeded_guest_wallet(7, "verify");
        let now = now_ms();
        let ClientMessage {
            message: Some(client_message::Message::Handshake(request)),
        } = identity.pulse_handshake(now, None)
        else {
            panic!("a handshake message");
        };
        let verified = verify_handshake(&request, now).expect("the server accepts the chain");
        assert_eq!(verified.user_address, identity.address());
        assert_eq!(verified.session, identity.session());
    }

    #[test]
    fn pulse_handshake_verifies() {
        let identity = Identity::derive("verify", 0);
        let now = now_ms();
        let ClientMessage {
            message: Some(client_message::Message::Handshake(request)),
        } = identity.pulse_handshake(now, None)
        else {
            panic!("a handshake message");
        };
        let verified = verify_handshake(&request, now).expect("the server accepts the chain");
        assert_eq!(verified.user_address, identity.address());
        assert_eq!(verified.session, identity.session());
    }

    #[test]
    fn a_stale_pulse_handshake_does_not_verify() {
        let identity = Identity::derive("verify", 0);
        let now = now_ms();
        let ClientMessage {
            message: Some(client_message::Message::Handshake(request)),
        } = identity.pulse_handshake(now - 10 * MAX_TIMESTAMP_SKEW_MS, None)
        else {
            panic!("a handshake message");
        };
        assert!(verify_handshake(&request, now).is_err());
    }

    /// The expiry of the mandate in `identity`'s auth chain, in Unix milliseconds.
    fn mandate_expiry(identity: &Identity) -> i64 {
        let chain = identity.auth_chain("payload");
        dcl_auth::parse_ephemeral_payload(&chain[1].payload)
            .expect("the server parses the mandate")
            .2
    }

    #[test]
    fn a_wallet_key_gets_a_random_device_and_a_short_mandate() {
        let key = hex::encode(Sha256::digest(b"comms-e2e identity test wallet"));
        let first = Identity::from_wallet_key(&key, "run/listener").unwrap();
        let again = Identity::from_wallet_key(&key, "run/listener").unwrap();
        assert_eq!(first.address(), again.address());
        // nothing anyone can recompute: the same inputs give another device key
        assert_ne!(first.session(), again.session());
        assert_ne!(first.second_device().session(), first.session());
        assert_eq!(first.same_device().session(), first.session());

        let now = now_ms();
        let expiry = mandate_expiry(&first);
        assert!(expiry > now, "the mandate has expired already");
        assert!(
            expiry <= now + crate::casts::millis_i64(WALLET_KEY_MANDATE),
            "the mandate lasts past {WALLET_KEY_MANDATE:?}: {expiry} at {now}"
        );
        assert!(
            mandate_expiry(&first.second_device())
                <= now_ms() + crate::casts::millis_i64(WALLET_KEY_MANDATE)
        );

        // the server still accepts the handshake it signs
        let ClientMessage {
            message: Some(client_message::Message::Handshake(request)),
        } = first.pulse_handshake(now, None)
        else {
            panic!("a handshake message");
        };
        let verified = verify_handshake(&request, now).expect("the server accepts the chain");
        assert_eq!(verified.user_address, first.address());
        assert_eq!(verified.session, first.session());
    }

    #[test]
    fn a_wallet_key_that_does_not_parse_is_not_echoed() {
        for malformed in ["not-a-real-key-fake-0123", "0xFAKEFAKEFAKE", "  zz  "] {
            let error = format!(
                "{:#}",
                Identity::from_wallet_key(malformed, "m").unwrap_err()
            );
            assert!(error.contains("not a valid private key"), "{error}");
            assert!(!error.contains(malformed.trim()), "{error}");
        }
    }

    #[test]
    fn a_derived_identity_keeps_its_far_mandate() {
        let derived = Identity::derive("verify", 0);
        let year_2090 = 3_786_912_000_000;
        assert!(mandate_expiry(&derived) > year_2090);
        assert!(mandate_expiry(&Identity::seeded_guest_wallet(7, "far")) > year_2090);
    }

    #[test]
    fn the_archipelago_chain_delegates_to_the_device() {
        let identity = Identity::derive("archipelago", 0);
        let chain: Vec<AuthLink> =
            serde_json::from_str(&identity.archipelago_chain("dcl-challenge")).unwrap();
        assert_eq!(chain.len(), 3);
        assert_eq!(chain[0].payload, identity.address());
        assert!(chain[1].payload.contains(&identity.session()));
        assert_eq!(chain[2].payload, "dcl-challenge");
        let verified = dcl_auth::verify_auth_chain(&chain, "dcl-challenge", None);
        assert!(verified.is_ok(), "{verified:?}");
        // signed for one challenge, it does not pass for another
        assert!(dcl_auth::verify_auth_chain(&chain, "dcl-other", None).is_err());
    }
}
