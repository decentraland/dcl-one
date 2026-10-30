//! The console's session cookie: `cat_admin=<payload>.<mac>`, the payload a
//! base64url JSON `{addr, exp}` and the mac an HMAC-SHA256 over the encoded
//! payload under `SESSION_SECRET`. `verify` re-checks the mac, the expiry
//! and the live `ADMIN_ADDRESSES` allowlist on every request, so removing an
//! address from the allowlist logs it out at once. The same mac signs the
//! SIWE nonce (`auth::nonce`), which is what binds a sign-in message to this
//! host, address and expiry without a nonce table.

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use hmac::{Hmac, KeyInit, Mac};
use serde::{Deserialize, Serialize};
use sha2::Sha256;

use super::Settings;

type HmacSha256 = Hmac<Sha256>;

pub const COOKIE_NAME: &str = "cat_admin";

/// `ADMIN_SESSION_TTL_SECS` unset: 12 hours.
pub const DEFAULT_TTL_SECS: i64 = 43200;

#[derive(Serialize, Deserialize)]
pub struct SessionPayload {
    pub addr: String,
    pub exp: i64,
}

pub(crate) fn now_unix() -> i64 {
    chrono::Utc::now().timestamp()
}

fn sign(secret: &str, payload_b64: &str) -> Vec<u8> {
    let mut mac =
        HmacSha256::new_from_slice(secret.as_bytes()).expect("HMAC accepts any key length");
    mac.update(payload_b64.as_bytes());
    mac.finalize().into_bytes().to_vec()
}

/// A cookie for `addr` (lowercased), or `None` without a session secret.
pub fn mint(settings: &Settings, addr: &str) -> Option<String> {
    let secret = settings.session_secret.as_deref()?;
    let payload = SessionPayload {
        addr: addr.to_lowercase(),
        exp: now_unix() + settings.session_ttl_secs,
    };
    let json = serde_json::to_vec(&payload).ok()?;
    let p_b64 = URL_SAFE_NO_PAD.encode(json);
    let sig_b64 = URL_SAFE_NO_PAD.encode(sign(secret, &p_b64));
    Some(format!("{p_b64}.{sig_b64}"))
}

/// The address a valid, unexpired, still-allowlisted cookie names.
pub fn verify(settings: &Settings, cookie_val: &str) -> Option<String> {
    let secret = settings.session_secret.as_deref()?;
    let (p_b64, sig_b64) = cookie_val.split_once('.')?;

    let sig = URL_SAFE_NO_PAD.decode(sig_b64).ok()?;
    let mut mac = HmacSha256::new_from_slice(secret.as_bytes()).ok()?;
    mac.update(p_b64.as_bytes());
    mac.verify_slice(&sig).ok()?;

    let json = URL_SAFE_NO_PAD.decode(p_b64).ok()?;
    let payload: SessionPayload = serde_json::from_slice(&json).ok()?;
    if payload.exp <= now_unix() {
        return None;
    }
    let addr = payload.addr.to_lowercase();
    if !settings.is_allowed(&addr) {
        return None;
    }
    Some(addr)
}

pub(crate) fn mac_b64(settings: &Settings, msg: &str) -> Option<String> {
    let secret = settings.session_secret.as_deref()?;
    Some(URL_SAFE_NO_PAD.encode(sign(secret, msg)))
}

pub(crate) fn mac_verify(settings: &Settings, msg: &str, mac_b64_str: &str) -> bool {
    let Some(secret) = settings.session_secret.as_deref() else {
        return false;
    };
    let Ok(provided) = URL_SAFE_NO_PAD.decode(mac_b64_str) else {
        return false;
    };
    let Ok(mut mac) = HmacSha256::new_from_slice(secret.as_bytes()) else {
        return false;
    };
    mac.update(msg.as_bytes());
    mac.verify_slice(&provided).is_ok()
}

pub fn set_cookie_header(settings: &Settings, value: &str, secure: bool) -> String {
    let mut s = format!(
        "{COOKIE_NAME}={value}; HttpOnly; SameSite=Strict; Path=/; Max-Age={}",
        settings.session_ttl_secs
    );
    if secure {
        s.push_str("; Secure");
    }
    s
}

pub fn clear_cookie_header() -> String {
    format!("{COOKIE_NAME}=; HttpOnly; SameSite=Strict; Path=/; Max-Age=0")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn settings() -> Settings {
        Settings::parse(super::super::RawSettings {
            session_secret: Some("test-secret-key-0123456789".to_string()),
            admin_addresses: "0xAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA,0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".to_string(),
            session_ttl_secs: 0,
            cookie_insecure: false,
            eth_network: "mainnet".to_string(),
            writes_enabled: false,
        })
    }

    fn forged(settings: &Settings, addr: &str, exp: i64) -> String {
        let secret = settings.session_secret.as_deref().unwrap();
        let payload = SessionPayload {
            addr: addr.into(),
            exp,
        };
        let p_b64 = URL_SAFE_NO_PAD.encode(serde_json::to_vec(&payload).unwrap());
        let sig_b64 = URL_SAFE_NO_PAD.encode(sign(secret, &p_b64));
        format!("{p_b64}.{sig_b64}")
    }

    #[test]
    fn round_trip_mint_verify() {
        let s = settings();
        assert_eq!(
            s.session_ttl_secs, DEFAULT_TTL_SECS,
            "a non-positive TTL falls back to the default"
        );
        let addr = "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
        let cookie = mint(&s, addr).expect("secret configured");
        assert_eq!(verify(&s, &cookie).as_deref(), Some(addr));
    }

    #[test]
    fn mint_lowercases_address() {
        let s = settings();
        let cookie = mint(&s, "0xAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA").unwrap();
        assert_eq!(
            verify(&s, &cookie).as_deref(),
            Some("0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")
        );
    }

    #[test]
    fn tampered_signature_fails() {
        let s = settings();
        let cookie = mint(&s, "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa").unwrap();
        let (p, _sig) = cookie.split_once('.').unwrap();
        let forged = format!("{p}.{}", URL_SAFE_NO_PAD.encode([0u8; 32]));
        assert!(verify(&s, &forged).is_none());
    }

    #[test]
    fn tampered_payload_fails() {
        let s = settings();
        let cookie = mint(&s, "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa").unwrap();
        let (_p, sig) = cookie.split_once('.').unwrap();
        let other = SessionPayload {
            addr: "0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".into(),
            exp: now_unix() + 100,
        };
        let p2 = URL_SAFE_NO_PAD.encode(serde_json::to_vec(&other).unwrap());
        assert!(verify(&s, &format!("{p2}.{sig}")).is_none());
    }

    #[test]
    fn expired_fails() {
        let s = settings();
        assert!(
            verify(
                &s,
                &forged(
                    &s,
                    "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                    now_unix() - 1
                )
            )
            .is_none()
        );
    }

    #[test]
    fn addr_not_in_allowlist_fails() {
        let s = settings();
        assert!(
            verify(
                &s,
                &forged(
                    &s,
                    "0xcccccccccccccccccccccccccccccccccccccccc",
                    now_unix() + 100
                )
            )
            .is_none()
        );
    }

    #[test]
    fn malformed_cookie_fails() {
        let s = settings();
        assert!(verify(&s, "not-a-cookie").is_none());
        assert!(verify(&s, "").is_none());
        assert!(verify(&s, "a.b.c").is_none());
    }

    #[test]
    fn no_secret_mints_and_verifies_nothing() {
        let mut s = settings();
        let cookie = mint(&s, "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa").unwrap();
        s.session_secret = None;
        assert!(mint(&s, "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa").is_none());
        assert!(verify(&s, &cookie).is_none());
        assert!(!s.admin_enabled());
    }
}
