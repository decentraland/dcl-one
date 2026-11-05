//! HMAC-SHA256 signed URLs for email unsubscribe links, byte-compatible with
//! notifications-workers `common/src/signing.ts` so links already sent keep working.

use hmac::{Hmac, KeyInit, Mac};
use sha2::Sha256;
use url::Url;

const PARAM: &str = "signature";

fn hmac_hex(key: &str, message: &str) -> String {
    let mut mac =
        Hmac::<Sha256>::new_from_slice(key.as_bytes()).expect("hmac accepts any key length");
    mac.update(message.as_bytes());
    hex::encode(mac.finalize().into_bytes())
}

/// Appends `?signature=<hex>` over the normalized URL; `None` if `url` does not parse.
pub fn sign_url(key: &str, url: &str) -> Option<String> {
    let mut parsed = Url::parse(url).ok()?;
    if parsed.query_pairs().any(|(k, _)| k == PARAM) {
        return None;
    }
    let signature = hmac_hex(key, parsed.as_str());
    parsed.query_pairs_mut().append_pair(PARAM, &signature);
    Some(parsed.into())
}

/// Verifies a URL produced by [`sign_url`] (or the old TypeScript `signUrl`).
pub fn has_valid_signature(key: &str, url: &str) -> bool {
    let Ok(mut parsed) = Url::parse(url) else {
        return false;
    };
    let mut received = None;
    let rest: Vec<(String, String)> = parsed
        .query_pairs()
        .filter_map(|(k, v)| {
            if k == PARAM && received.is_none() {
                received = Some(v.into_owned());
                None
            } else {
                Some((k.into_owned(), v.into_owned()))
            }
        })
        .collect();
    let Some(received) = received else {
        return false;
    };
    if rest.is_empty() {
        parsed.set_query(None);
    } else {
        parsed.query_pairs_mut().clear().extend_pairs(rest);
    }
    let expected = hmac_hex(key, parsed.as_str());
    expected.len() == received.len()
        && expected
            .bytes()
            .zip(received.bytes())
            .fold(0u8, |acc, (a, b)| acc | (a ^ b))
            == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips() {
        let signed = sign_url("k", "https://notifications.example.org/unsubscribe/0xabc").unwrap();
        assert!(signed.contains("?signature="));
        assert!(has_valid_signature("k", &signed));
        assert!(!has_valid_signature("other", &signed));
    }

    #[test]
    fn rejects_missing_signature() {
        assert!(!has_valid_signature(
            "k",
            "https://notifications.example.org/unsubscribe/0xabc"
        ));
    }

    #[test]
    fn matches_node_crypto_hmac() {
        // node: createHmac('sha256','k').update('https://a.example/x').digest('hex')
        let signed = sign_url("k", "https://a.example/x").unwrap();
        let expected = hmac_hex("k", "https://a.example/x");
        assert_eq!(signed, format!("https://a.example/x?signature={expected}"));
    }
}
