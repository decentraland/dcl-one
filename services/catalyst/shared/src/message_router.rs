//! The contract between the community-chat message router (`catalyst-message-router`, its own
//! process) and the gatekeeper in `catalyst serve`, one copy for both sides: the router never
//! holds the LiveKit secret, so it asks the gatekeeper for its join token on
//! `POST /internal/message-router/token/v1` ([`TOKEN_PATH`]).
//!
//! The request is authenticated with a dedicated HMAC-SHA256 key (`MESSAGE_ROUTER_KEY`, 32
//! base64url bytes, never the LiveKit secret) over the method, path, timestamp, nonce and body
//! hash ([`preimage`]); the gatekeeper refuses a timestamp more than [`MAX_SKEW_SECS`] away and a
//! nonce it has seen. The body names only the router's replica ([`TokenRequest`]); the gatekeeper
//! derives the identity from its own configured prefix ([`identity`]), so the key cannot mint a
//! wallet's identity. The answer ([`TokenResponse`]) is a join to the private-messages room that
//! may subscribe and publish data, nothing else.

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use hmac::{Hmac, KeyInit, Mac};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

type HmacSha256 = Hmac<Sha256>;

pub const TOKEN_PATH: &str = "/internal/message-router/token/v1";
pub const HEADER_TIMESTAMP: &str = "x-message-router-timestamp";
pub const HEADER_NONCE: &str = "x-message-router-nonce";
pub const HEADER_SIGNATURE: &str = "x-message-router-signature";
/// The gatekeeper reads no larger request body.
pub const MAX_BODY: usize = 256;
/// How far the request's timestamp may be from the gatekeeper's clock.
pub const MAX_SKEW_SECS: u64 = 30;
/// The identity prefix the Explorer addresses outside decentraland.org: its
/// `message_router_identity()` is `message-router-dev-0` unless the base domain is
/// `decentraland.org` (`message-router-prd-0`).
pub const DEFAULT_IDENTITY_PREFIX: &str = "message-router-dev";

/// The router's request: which replica it is. Any other field is refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TokenRequest {
    pub replica: u16,
}

/// The gatekeeper's answer: the LiveKit signaling URL, the join token and the room and identity
/// the token names.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TokenResponse {
    pub url: String,
    pub token: String,
    pub room: String,
    pub identity: String,
}

impl std::fmt::Debug for TokenResponse {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TokenResponse")
            .field("url", &self.url)
            .field("token", &"<set>")
            .field("room", &self.room)
            .field("identity", &self.identity)
            .finish()
    }
}

/// The router's LiveKit identity: `<prefix>-<replica>`, as upstream's
/// `LIVEKIT_IDENTITY_PREFIX` and replica number compose it.
pub fn identity(prefix: &str, replica: u16) -> String {
    format!("{prefix}-{replica}")
}

/// A prefix that keeps the identity apart from every wallet identity: a letter first, then
/// lowercase letters, digits and `-`, at most 64 bytes.
pub fn valid_identity_prefix(prefix: &str) -> bool {
    !prefix.is_empty()
        && prefix.len() <= 64
        && prefix.starts_with(|c: char| c.is_ascii_lowercase())
        && prefix
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum KeyError {
    #[error("MESSAGE_ROUTER_KEY must be 32 base64url bytes without padding")]
    Shape,
    #[error("MESSAGE_ROUTER_KEY must not reuse the LiveKit secret")]
    LivekitSecret,
}

/// `MESSAGE_ROUTER_KEY`: 32 base64url bytes without padding. The gatekeeper also passes its
/// LiveKit secret, which the key must not equal; the router holds none and passes `None`.
pub fn parse_key(encoded: &str, livekit_secret: Option<&[u8]>) -> Result<[u8; 32], KeyError> {
    let bytes = URL_SAFE_NO_PAD
        .decode(encoded.trim())
        .map_err(|_| KeyError::Shape)?;
    let key: [u8; 32] = bytes.try_into().map_err(|_| KeyError::Shape)?;
    if livekit_secret.is_some_and(|secret| secret == key.as_slice()) {
        return Err(KeyError::LivekitSecret);
    }
    Ok(key)
}

/// What the signature covers: `POST`, the path, the timestamp, the nonce and the body's SHA-256
/// in lowercase hex, one per line.
pub fn preimage(timestamp: &str, nonce: &str, body: &[u8]) -> String {
    let hash = hex::encode(Sha256::digest(body));
    format!("POST\n{TOKEN_PATH}\n{timestamp}\n{nonce}\n{hash}")
}

/// The request signature, base64url without padding.
pub fn sign(key: &[u8; 32], timestamp: &str, nonce: &str, body: &[u8]) -> String {
    let mut mac = HmacSha256::new_from_slice(key).expect("HMAC takes any key length");
    mac.update(preimage(timestamp, nonce, body).as_bytes());
    URL_SAFE_NO_PAD.encode(mac.finalize().into_bytes())
}

/// Checks `signature` in constant time.
pub fn verify(key: &[u8; 32], timestamp: &str, nonce: &str, body: &[u8], signature: &str) -> bool {
    let Ok(signature) = URL_SAFE_NO_PAD.decode(signature) else {
        return false;
    };
    let mut mac = HmacSha256::new_from_slice(key).expect("HMAC takes any key length");
    mac.update(preimage(timestamp, nonce, body).as_bytes());
    mac.verify_slice(&signature).is_ok()
}

/// A fresh nonce: 16 random bytes, base64url without padding.
pub fn nonce(bytes: [u8; 16]) -> String {
    URL_SAFE_NO_PAD.encode(bytes)
}

/// The nonce's 16 bytes when it is in the canonical encoding [`nonce`] writes.
pub fn decode_nonce(value: &str) -> Option<[u8; 16]> {
    if value.len() != 22 {
        return None;
    }
    let bytes = URL_SAFE_NO_PAD.decode(value).ok()?;
    if URL_SAFE_NO_PAD.encode(&bytes) != value {
        return None;
    }
    bytes.try_into().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    const KEY: [u8; 32] = [7; 32];

    #[test]
    fn signature_covers_timestamp_nonce_and_body() {
        let nonce = nonce([1; 16]);
        let body = br#"{"replica":0}"#;
        let signature = sign(&KEY, "1700000000", &nonce, body);
        assert!(verify(&KEY, "1700000000", &nonce, body, &signature));
        assert!(!verify(&KEY, "1700000001", &nonce, body, &signature));
        assert!(!verify(
            &KEY,
            "1700000000",
            &super::nonce([2; 16]),
            body,
            &signature
        ));
        assert!(!verify(
            &KEY,
            "1700000000",
            &nonce,
            br#"{"replica":1}"#,
            &signature
        ));
        assert!(!verify(&[8; 32], "1700000000", &nonce, body, &signature));
        assert!(!verify(&KEY, "1700000000", &nonce, body, "not base64!"));
    }

    #[test]
    fn preimage_pins_the_signed_lines() {
        assert_eq!(
            preimage("1", "n", b""),
            "POST\n/internal/message-router/token/v1\n1\nn\n\
             e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    #[test]
    fn key_is_32_base64url_bytes_and_not_the_livekit_secret() {
        let encoded = URL_SAFE_NO_PAD.encode(KEY);
        assert_eq!(parse_key(&encoded, None), Ok(KEY));
        assert_eq!(parse_key(&format!(" {encoded}\n"), Some(b"other")), Ok(KEY));
        assert_eq!(
            parse_key(&encoded, Some(&KEY)),
            Err(KeyError::LivekitSecret)
        );
        assert_eq!(
            parse_key(&URL_SAFE_NO_PAD.encode([7; 31]), None),
            Err(KeyError::Shape)
        );
        assert_eq!(parse_key("short", None), Err(KeyError::Shape));
    }

    #[test]
    fn nonce_round_trips_only_in_its_canonical_form() {
        let encoded = nonce([9; 16]);
        assert_eq!(decode_nonce(&encoded), Some([9; 16]));
        assert_eq!(decode_nonce(&encoded[1..]), None);
        assert_eq!(decode_nonce("!!!!!!!!!!!!!!!!!!!!!!"), None);
    }

    #[test]
    fn identity_is_prefix_and_replica() {
        assert_eq!(identity(DEFAULT_IDENTITY_PREFIX, 0), "message-router-dev-0");
        assert!(valid_identity_prefix("message-router-prd"));
        for bad in ["", "0x", "Message", "a b", "a_b", &"a".repeat(65)] {
            assert!(!valid_identity_prefix(bad), "{bad}");
        }
    }

    #[test]
    fn request_refuses_other_fields() {
        assert_eq!(
            serde_json::from_str::<TokenRequest>(r#"{"replica":3}"#).unwrap(),
            TokenRequest { replica: 3 }
        );
        for bad in [
            r#"{"replica":0,"identity":"0xabc"}"#,
            r#"{"replica":-1}"#,
            r#"{"replica":70000}"#,
            r#"{}"#,
        ] {
            assert!(serde_json::from_str::<TokenRequest>(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn response_debug_hides_the_token() {
        let response = TokenResponse {
            url: "wss://livekit.example".into(),
            token: "secret-token".into(),
            room: "private-messages".into(),
            identity: "message-router-dev-0".into(),
        };
        assert!(!format!("{response:?}").contains("secret-token"));
    }
}
