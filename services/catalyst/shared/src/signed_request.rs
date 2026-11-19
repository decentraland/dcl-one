//! The HMAC-SHA256 signature of an internal service request, one copy for every route that
//! authenticates a peer process with a dedicated 32-byte service key: the Pulse room authority
//! (`comms::gatekeeper::admission::relay_authority`, `PULSE_ROOM_AUTHORITY_KEY`) and the message
//! router's token route (`comms::gatekeeper::message_router`, contract in
//! [`crate::message_router`], `MESSAGE_ROUTER_KEY`).
//!
//! The signature covers `POST`, the path, the timestamp (decimal Unix seconds), the nonce (16
//! random bytes, base64url without padding), an optional route-specific line and the body's
//! SHA-256 in lowercase hex, one per line ([`preimage`]). The key, the nonce and the signature
//! are base64url without padding, each in its one canonical form. A route refuses a timestamp
//! outside its skew window ([`fresh_timestamp`]) and a nonce it has seen ([`ReplayCache`]).

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Duration;

use axum::http::HeaderMap;
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use hmac::{Hmac, KeyInit, Mac};
use sha2::{Digest, Sha256};
use tokio::time::Instant;

type HmacSha256 = Hmac<Sha256>;

/// What the signature covers: `POST`, `path`, `timestamp`, `nonce`, `extra` when the route has
/// one, and the body's SHA-256 in lowercase hex, one per line.
pub fn preimage(
    path: &str,
    timestamp: &str,
    nonce: &str,
    extra: Option<&str>,
    body: &[u8],
) -> String {
    let hash = hex::encode(Sha256::digest(body));
    match extra {
        Some(extra) => format!("POST\n{path}\n{timestamp}\n{nonce}\n{extra}\n{hash}"),
        None => format!("POST\n{path}\n{timestamp}\n{nonce}\n{hash}"),
    }
}

/// The signature of `preimage`, base64url without padding.
pub fn sign(key: &[u8; 32], preimage: &str) -> String {
    let mut mac = HmacSha256::new_from_slice(key).expect("HMAC takes any key length");
    mac.update(preimage.as_bytes());
    URL_SAFE_NO_PAD.encode(mac.finalize().into_bytes())
}

/// Checks `signature` (its canonical base64url form) over `preimage`, in constant time.
pub fn verify(key: &[u8; 32], preimage: &str, signature: &str) -> bool {
    let Some(signature) = decode_fixed::<32>(signature) else {
        return false;
    };
    let mut mac = HmacSha256::new_from_slice(key).expect("HMAC takes any key length");
    mac.update(preimage.as_bytes());
    mac.verify_slice(&signature).is_ok()
}

/// A fresh nonce: 16 random bytes, base64url without padding.
pub fn nonce(bytes: [u8; 16]) -> String {
    URL_SAFE_NO_PAD.encode(bytes)
}

/// The `N` bytes of `value` when it is their canonical base64url form without padding.
pub fn decode_fixed<const N: usize>(value: &str) -> Option<[u8; N]> {
    if value.len() != (N * 8).div_ceil(6) {
        return None;
    }
    let bytes = URL_SAFE_NO_PAD.decode(value).ok()?;
    if URL_SAFE_NO_PAD.encode(&bytes) != value {
        return None;
    }
    bytes.try_into().ok()
}

/// The value of the header `name` when the request carries it exactly once, as visible ASCII.
pub fn one_header<'a>(headers: &'a HeaderMap, name: &str) -> Option<&'a str> {
    let mut values = headers.get_all(name).iter();
    let value = values.next()?.to_str().ok()?;
    values.next().is_none().then_some(value)
}

/// Whether `timestamp` is canonical decimal seconds at most `max_skew_secs` from `now_secs`.
pub fn fresh_timestamp(timestamp: &str, now_secs: u64, max_skew_secs: u64) -> bool {
    timestamp.parse::<u64>().is_ok_and(|secs| {
        secs.to_string() == timestamp && secs.abs_diff(now_secs) <= max_skew_secs
    })
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum KeyError {
    #[error("{0} must be 32 base64url bytes without padding")]
    Shape(&'static str),
    #[error("{0} must not reuse the LiveKit secret")]
    LivekitSecret(&'static str),
}

/// A service key, the variable `name`: 32 base64url bytes without padding (surrounding
/// whitespace ignored). A route beside the LiveKit secret passes it, and the key must differ.
pub fn parse_key(
    name: &'static str,
    encoded: &str,
    livekit_secret: Option<&[u8]>,
) -> Result<[u8; 32], KeyError> {
    let bytes = URL_SAFE_NO_PAD
        .decode(encoded.trim())
        .map_err(|_| KeyError::Shape(name))?;
    let key: [u8; 32] = bytes.try_into().map_err(|_| KeyError::Shape(name))?;
    if livekit_secret.is_some_and(|secret| secret == key.as_slice()) {
        return Err(KeyError::LivekitSecret(name));
    }
    Ok(key)
}

/// Why [`ReplayCache::admit`] refused a nonce.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Replay {
    /// The nonce was admitted within the last `ttl`.
    Seen,
    /// `max` unexpired nonces are remembered already.
    Full,
}

/// The nonces a route admitted, each remembered for `ttl` (longer than both ends of the route's
/// skew window), at most `max` at once. Call [`ReplayCache::admit`] only after the signature
/// verified, so an unauthenticated caller cannot fill it.
pub struct ReplayCache {
    ttl: Duration,
    max: usize,
    seen: Mutex<HashMap<[u8; 16], Instant>>,
}

impl ReplayCache {
    pub fn new(ttl: Duration, max: usize) -> Self {
        Self {
            ttl,
            max,
            seen: Mutex::new(HashMap::new()),
        }
    }

    /// Remembers `nonce` until `now + ttl`; refused when it is still remembered or the cache
    /// holds `max` unexpired nonces.
    pub fn admit(&self, nonce: [u8; 16], now: Instant) -> Result<(), Replay> {
        let mut seen = self.seen.lock().unwrap_or_else(|e| e.into_inner());
        if seen.get(&nonce).is_some_and(|expires| *expires > now) {
            return Err(Replay::Seen);
        }
        if seen.len() >= self.max {
            seen.retain(|_, expires| *expires > now);
            if seen.len() >= self.max {
                return Err(Replay::Full);
            }
        }
        seen.insert(nonce, now + self.ttl);
        Ok(())
    }

    /// The nonces remembered, expired ones included until the next sweep.
    pub fn len(&self) -> usize {
        self.seen.lock().unwrap_or_else(|e| e.into_inner()).len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const KEY: [u8; 32] = [7; 32];

    #[test]
    fn preimage_pins_the_signed_lines() {
        let empty = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";
        assert_eq!(
            preimage("/p", "1", "n", None, b""),
            format!("POST\n/p\n1\nn\n{empty}")
        );
        assert_eq!(
            preimage("/p", "1", "n", Some("s"), b""),
            format!("POST\n/p\n1\nn\ns\n{empty}")
        );
    }

    #[test]
    fn a_signature_verifies_only_over_its_preimage_and_key_in_canonical_form() {
        let signed = preimage("/p", "1", "n", None, b"body");
        let signature = sign(&KEY, &signed);
        assert!(verify(&KEY, &signed, &signature));
        assert!(!verify(&KEY, &preimage("/p", "2", "n", None, b"body"), &signature));
        assert!(!verify(&[8; 32], &signed, &signature));
        assert!(!verify(&KEY, &signed, "not base64!"));
        assert!(!verify(&KEY, &signed, &format!("{signature}=")));
    }

    #[test]
    fn fixed_values_decode_only_in_their_canonical_form() {
        let encoded = nonce([9; 16]);
        assert_eq!(decode_fixed::<16>(&encoded), Some([9; 16]));
        assert_eq!(decode_fixed::<16>(&encoded[1..]), None);
        assert_eq!(decode_fixed::<16>("!!!!!!!!!!!!!!!!!!!!!!"), None);
        assert_eq!(decode_fixed::<32>(&encoded), None);
    }

    #[test]
    fn a_header_counts_only_once() {
        let mut headers = HeaderMap::new();
        assert_eq!(one_header(&headers, "x-a"), None);
        headers.append("x-a", "1".parse().unwrap());
        assert_eq!(one_header(&headers, "x-a"), Some("1"));
        headers.append("x-a", "2".parse().unwrap());
        assert_eq!(one_header(&headers, "x-a"), None);
    }

    #[test]
    fn timestamps_are_canonical_and_within_the_skew() {
        assert!(fresh_timestamp("100", 105, 5));
        assert!(fresh_timestamp("110", 105, 5));
        assert!(!fresh_timestamp("99", 105, 5));
        assert!(!fresh_timestamp("0100", 100, 5));
        assert!(!fresh_timestamp("+100", 100, 5));
        assert!(!fresh_timestamp("x", 100, 5));
    }

    #[test]
    fn keys_are_32_base64url_bytes_and_not_the_livekit_secret() {
        let encoded = URL_SAFE_NO_PAD.encode(KEY);
        assert_eq!(parse_key("K", &encoded, None), Ok(KEY));
        assert_eq!(parse_key("K", &format!(" {encoded}\n"), Some(b"other")), Ok(KEY));
        assert_eq!(
            parse_key("K", &encoded, Some(&KEY)),
            Err(KeyError::LivekitSecret("K"))
        );
        assert_eq!(
            parse_key("K", &URL_SAFE_NO_PAD.encode([7; 31]), None),
            Err(KeyError::Shape("K"))
        );
        assert_eq!(
            KeyError::Shape("MESSAGE_ROUTER_KEY").to_string(),
            "MESSAGE_ROUTER_KEY must be 32 base64url bytes without padding"
        );
    }

    #[test]
    fn a_nonce_is_refused_until_it_expires_and_the_cache_is_bounded() {
        let cache = ReplayCache::new(Duration::from_secs(10), 2);
        let start = Instant::now();
        assert_eq!(cache.admit([1; 16], start), Ok(()));
        assert_eq!(cache.admit([1; 16], start), Err(Replay::Seen));
        assert_eq!(cache.admit([2; 16], start), Ok(()));
        assert_eq!(cache.admit([3; 16], start), Err(Replay::Full));
        let later = start + Duration::from_secs(10);
        assert_eq!(cache.admit([1; 16], later), Ok(()));
        assert_eq!(cache.admit([3; 16], later), Ok(()));
    }
}
