//! Test identities: a wallet and a device key derived from text, and the three signatures the
//! stack asks for (the Pulse handshake, the archipelago challenge, a signed fetch). The keys are
//! derived, never real: nothing here may hold a key that owns anything.

use catalyst_comms::pulse::decentraland::pulse::{
    ClientMessage, HandshakeRequest, PlayerInitialState, SceneListenerAoi,
    SceneListenerHandshakeRequest, client_message,
};
use dcl_auth::signed_fetch::{
    AUTH_CHAIN_HEADER_PREFIX, AUTH_METADATA_HEADER, AUTH_TIMESTAMP_HEADER, build_payload_v6,
};
use dcl_auth::{AuthLink, AuthLinkType, Wallet};
use sha2::{Digest, Sha256};

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
        }
    }

    /// The identity of a wallet whose private key is given (a scene listener's wallet, which the
    /// server is configured to admit), with a device key derived from `material`. The key is
    /// never printed or kept beyond the wallet it builds.
    pub fn from_wallet_key(private_key_hex: &str, material: &str) -> anyhow::Result<Identity> {
        let wallet = Wallet::from_hex(private_key_hex.trim())
            .map_err(|_| anyhow::anyhow!("the wallet key is not a valid private key"))?;
        Ok(Identity {
            wallet,
            device: derived_key("device", material, 0, 0),
            material: material.to_owned(),
            index: 0,
            device_number: 0,
        })
    }

    /// The same wallet on another device: a new session key.
    pub fn second_device(&self) -> Identity {
        let device_number = self.device_number + 1;
        Identity {
            wallet: Wallet::from_hex(&self.wallet.private_key_hex()).expect("a key that was valid"),
            device: derived_key("device", &self.material, self.index, device_number),
            material: self.material.clone(),
            index: self.index,
            device_number,
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
        let mandate = format!(
            "Decentraland Login\nEphemeral address: {}\nExpiration: 2099-01-01T00:00:00.000Z",
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

    /// A player's Pulse handshake.
    pub fn pulse_handshake(
        &self,
        timestamp_ms: i64,
        initial_state: Option<PlayerInitialState>,
    ) -> ClientMessage {
        ClientMessage {
            message: Some(client_message::Message::Handshake(HandshakeRequest {
                auth_chain: self.pulse_auth_chain(timestamp_ms),
                profile_version: 0,
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
        .expect("the clock is past 1970")
        .as_millis() as i64
}

#[cfg(test)]
mod tests {
    use super::*;
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
        } = identity.pulse_handshake(now - 10 * 60_000, None)
        else {
            panic!("a handshake message");
        };
        assert!(verify_handshake(&request, now).is_err());
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
