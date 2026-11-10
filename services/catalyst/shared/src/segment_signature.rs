//! The Segment destination webhook signature: `x-signature` is the hex
//! HMAC-SHA1 of the delivered body under the shared signing key
//! (events-notifier `logic/utils.ts:47-58`, which signs
//! `JSON.stringify(parsedBody)`). The raw bytes verify first; a body that
//! arrives re-spaced (pretty-printed or padded) verifies through its
//! whitespace-compacted form, the sender's key order and escaping kept as
//! received. A difference in escaping or number formatting is not covered:
//! Segment delivers the compact form it signs.

use hmac::{Hmac, KeyInit, Mac};
use sha1::Sha1;

type HmacSha1 = Hmac<Sha1>;

/// The hex HMAC-SHA1 of `body` under `key`: what a sender puts in `x-signature`.
pub fn sign(key: &[u8], body: &[u8]) -> String {
    let mut mac = HmacSha1::new_from_slice(key).expect("HMAC accepts any key length");
    mac.update(body);
    hex::encode(mac.finalize().into_bytes())
}

/// Whether `header` is the signature of `raw_body` (or of its compacted form) under `key`.
pub fn verify(key: &[u8], raw_body: &[u8], header: Option<&str>) -> bool {
    let Some(header) = header else {
        return false;
    };
    let Ok(expected) = hex::decode(header.trim()) else {
        return false;
    };
    if verify_bytes(key, raw_body, &expected) {
        return true;
    }
    compact_json(raw_body).is_some_and(|compact| verify_bytes(key, &compact, &expected))
}

fn verify_bytes(key: &[u8], body: &[u8], expected: &[u8]) -> bool {
    let Ok(mut mac) = HmacSha1::new_from_slice(key) else {
        return false;
    };
    mac.update(body);
    mac.verify_slice(expected).is_ok()
}

/// `raw` without the whitespace outside its JSON strings; `None` when there is none to drop.
fn compact_json(raw: &[u8]) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(raw.len());
    let (mut in_string, mut escaped) = (false, false);
    for &byte in raw {
        if in_string {
            out.push(byte);
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == b'"' {
                in_string = false;
            }
            continue;
        }
        match byte {
            b' ' | b'\t' | b'\n' | b'\r' => {}
            b'"' => {
                in_string = true;
                out.push(byte);
            }
            _ => out.push(byte),
        }
    }
    (out.len() != raw.len()).then_some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    const KEY: &[u8] = b"test-signing-key";
    // events-notifier test/unit/utils.spec.ts: the body `{test: 'data'}` under `test-signing-key`.
    const BODY: &[u8] = br#"{"test":"data"}"#;
    const SIGNATURE: &str = "f0dd615ed398f4a1cc5ff52efcdac6a53f768518";

    #[test]
    fn signs_and_verifies_the_notifier_vector() {
        assert_eq!(sign(KEY, BODY), SIGNATURE);
        assert!(verify(KEY, BODY, Some(SIGNATURE)));
        assert!(verify(KEY, BODY, Some(&SIGNATURE.to_uppercase())));
    }

    #[test]
    fn refuses_a_missing_malformed_or_foreign_signature() {
        assert!(!verify(KEY, BODY, None));
        assert!(!verify(KEY, BODY, Some("")));
        assert!(!verify(KEY, BODY, Some("not-hex")));
        assert!(!verify(KEY, BODY, Some(&sign(b"other-key", BODY))));
        assert!(!verify(KEY, br#"{"test":"data "}"#, Some(SIGNATURE)));
    }

    #[test]
    fn a_respaced_body_verifies_only_through_the_compact_form() {
        let padded = b"{\n  \"test\": \"data\"\n}\n";
        assert!(verify(KEY, padded, Some(SIGNATURE)));
        assert!(!verify(
            KEY,
            padded,
            Some(&sign(KEY, b"{\"test\": \"data\"}"))
        ));
        // Whitespace inside a string is part of the signed text.
        assert!(!verify(KEY, br#"{"test":"da ta"}"#, Some(SIGNATURE)));
        let escaped = br#"{"a":"q\" x","b":1}"#;
        assert!(verify(
            KEY,
            b"{ \"a\": \"q\\\" x\", \"b\": 1 }",
            Some(&sign(KEY, escaped))
        ));
    }
}
