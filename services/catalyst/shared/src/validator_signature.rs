//! The wearable validator's request signature: `x-wearable-validator-signature`
//! is `sha256=` and the lowercase hex HMAC-SHA256, under the shared secret, of
//! `<timestamp>.<METHOD>.<path>.<body>`, where the timestamp is
//! `x-wearable-validator-timestamp` (Unix milliseconds), the method is
//! uppercase, the path is the request path without its query and the body is
//! the raw bytes sent. The timestamp is at most [`WINDOW_MS`] away from the
//! receiver's clock either way.
//!
//! The headers, the `sha256=` hex encoding, the millisecond timestamp, the
//! window and the secret are those of builder-server's validator callback
//! (`withValidatorSignature`), which the validator already signs; its
//! `<timestamp>.<body>` preimage gains the method and the path because the
//! validator calls several routes. This module keeps no record of what it
//! verified: a signature is valid for its whole window, so the caller
//! refuses a MAC it has seen within the window, in storage every replica
//! shares. [`crate::signed_request`]'s nonce cache lives in one process, so
//! it can't refuse a replay sent to another replica.

use hmac::{Hmac, KeyInit, Mac};
use sha2::Sha256;

pub const SIGNATURE_HEADER: &str = "x-wearable-validator-signature";
pub const TIMESTAMP_HEADER: &str = "x-wearable-validator-timestamp";
/// How far a signature's timestamp may be from the receiver's clock.
pub const WINDOW_MS: i64 = 5 * 60 * 1000;
const PREFIX: &str = "sha256=";

fn mac(secret: &[u8], timestamp: &str, method: &str, path: &str, body: &[u8]) -> Hmac<Sha256> {
    let mut mac = Hmac::<Sha256>::new_from_slice(secret).expect("HMAC accepts any key length");
    for part in [timestamp.as_bytes(), method.as_bytes(), path.as_bytes()] {
        mac.update(part);
        mac.update(b".");
    }
    mac.update(body);
    mac
}

/// The signature header's value for a request.
pub fn sign(secret: &[u8], timestamp: &str, method: &str, path: &str, body: &[u8]) -> String {
    let digest = mac(secret, timestamp, method, path, body).finalize();
    format!("{PREFIX}{}", hex::encode(digest.into_bytes()))
}

/// The timestamp header as Unix milliseconds: digits only.
pub fn timestamp_ms(timestamp: &str) -> Option<i64> {
    if timestamp.is_empty()
        || timestamp.len() > 15
        || !timestamp.bytes().all(|b| b.is_ascii_digit())
    {
        return None;
    }
    timestamp.parse().ok()
}

/// The verified MAC as lowercase hex when `signature` signs the request over
/// one of `paths` and `timestamp` is within [`WINDOW_MS`] of `now_ms`;
/// `None` otherwise. The comparison runs in constant time.
pub fn verify(
    secret: &[u8],
    timestamp: &str,
    signature: &str,
    method: &str,
    paths: &[&str],
    body: &[u8],
    now_ms: i64,
) -> Option<String> {
    let sent = timestamp_ms(timestamp)?;
    if (now_ms - sent).abs() > WINDOW_MS {
        return None;
    }
    let expected = hex::decode(signature.strip_prefix(PREFIX)?).ok()?;
    if expected.len() != 32 {
        return None;
    }
    let method = method.to_ascii_uppercase();
    paths
        .iter()
        .any(|path| {
            mac(secret, timestamp, &method, path, body)
                .verify_slice(&expected)
                .is_ok()
        })
        .then(|| hex::encode(&expected))
}

#[cfg(test)]
mod tests {
    use super::*;

    const SECRET: &[u8] = b"example-validator-secret-0123456789abcdef";
    const NOW: i64 = 1_767_225_600_000;
    const PATH: &str = "/v1/validator/claims";
    /// `printf '%s' '1767225600000.POST./v1/validator/claims.{}' | openssl dgst -sha256 -hmac <SECRET>`.
    const VECTOR: &str = "sha256=4d1121401188cbc0efa54170f2349db90874f36968b364e7ee293c631f55c42e";

    fn check(
        timestamp: &str,
        signature: &str,
        method: &str,
        path: &str,
        body: &[u8],
        now: i64,
    ) -> Option<String> {
        verify(SECRET, timestamp, signature, method, &[path], body, now)
    }

    #[test]
    fn a_signature_is_the_hmac_of_timestamp_method_path_and_body() {
        assert_eq!(sign(SECRET, "1767225600000", "POST", PATH, b"{}"), VECTOR);
        assert_eq!(
            check("1767225600000", VECTOR, "post", PATH, b"{}", NOW).as_deref(),
            Some(&VECTOR[PREFIX.len()..])
        );
    }

    #[test]
    fn a_signature_binds_its_body_route_and_secret() {
        let ts = "1767225600000";
        assert_eq!(check(ts, VECTOR, "POST", PATH, b"{ }", NOW), None);
        assert_eq!(check(ts, VECTOR, "PUT", PATH, b"{}", NOW), None);
        assert_eq!(
            check(
                ts,
                VECTOR,
                "POST",
                "/v1/validator/claims/x/lease",
                b"{}",
                NOW
            ),
            None
        );
        assert_eq!(
            verify(
                b"another-secret-0123456789abcdefghij",
                ts,
                VECTOR,
                "POST",
                &[PATH],
                b"{}",
                NOW
            ),
            None
        );
        assert!(
            verify(
                SECRET,
                ts,
                VECTOR,
                "POST",
                &["/builder-api/v1/validator/claims", PATH],
                b"{}",
                NOW
            )
            .is_some()
        );
    }

    #[test]
    fn the_timestamp_is_digits_within_five_minutes_either_way() {
        for (offset, accepted) in [
            (-300_000, true),
            (300_000, true),
            (-300_001, false),
            (300_001, false),
        ] {
            let ts = (NOW + offset).to_string();
            let signature = sign(SECRET, &ts, "POST", PATH, b"{}");
            assert_eq!(
                check(&ts, &signature, "POST", PATH, b"{}", NOW).is_some(),
                accepted,
                "{offset}"
            );
        }
        for ts in [
            "",
            " 1767225600000",
            "1767225600000.0",
            "1.7e12",
            "-1",
            "+1767225600000",
            "1234567890123456",
        ] {
            let signature = sign(SECRET, ts, "POST", PATH, b"{}");
            assert_eq!(
                check(ts, &signature, "POST", PATH, b"{}", NOW),
                None,
                "{ts:?}"
            );
        }
    }

    #[test]
    fn the_verified_mac_is_canonical_whatever_case_it_was_sent_in() {
        let upper = format!("{PREFIX}{}", VECTOR[PREFIX.len()..].to_ascii_uppercase());
        assert_eq!(
            check("1767225600000", &upper, "POST", PATH, b"{}", NOW).as_deref(),
            Some(&VECTOR[PREFIX.len()..])
        );
        for refused in [&VECTOR[PREFIX.len()..], "sha256=", "sha256=00", "sha1=4d11"] {
            assert_eq!(
                check("1767225600000", refused, "POST", PATH, b"{}", NOW),
                None,
                "{refused}"
            );
        }
    }
}
