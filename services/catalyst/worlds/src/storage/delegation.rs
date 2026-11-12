//! The scoped storage capability a guest presents: the `x-authoritative-scope`
//! header, a base64 JSON `{payload, signature}` whose payload is the claim
//!
//! ```text
//! Decentraland Authoritative Storage Delegation
//! Ephemeral: <the request signer>
//! World: <world name or "main">
//! SceneId: <the request metadata's sceneId>
//! Parcel: <parcel as the request metadata spells it>
//! Expiration: <a date `Date.parse` reads, ISO 8601 from the minter>
//! ```
//!
//! signed (EIP-191) by the authoritative server. The header is read as
//! upstream's Node service reads it (`Buffer.from(_, 'base64')`, `JSON.parse`,
//! `Date.parse`, ethers-style recovery that also takes a high-s signature).
//! A verified claim admits its ephemeral signer to that scene's storage on
//! the routes whose `AuthPolicy` allows a scoped delegation (`mod.rs`): the
//! per-key and list `/values*` and `/players*` routes (not the clears),
//! `GET /env/{key}`, `GET /usage/world` and `GET /usage/players/{address}`.
//! The guest never holds SQL, a key, or another scene's namespace.

use alloy_primitives::Signature;
use chrono::{DateTime, Utc};

use super::js::{date_parse, js_trim, node_base64_decode};

pub const AUTHORITATIVE_SCOPE_HEADER: &str = "x-authoritative-scope";

pub const STORAGE_DELEGATION_PREFIX: &str = "Decentraland Authoritative Storage Delegation";

pub const MAX_SCOPE_HEADER_LENGTH: usize = 4096;

const CLAIM_FIELDS: [&str; 5] = ["Ephemeral:", "World:", "SceneId:", "Parcel:", "Expiration:"];

#[derive(Debug, PartialEq)]
struct ParsedClaim {
    ephemeral: String,
    world: String,
    scene_id: String,
    parcel: String,
    /// `Date.parse` milliseconds.
    expiration: f64,
}

pub struct StorageDelegationTarget<'a> {
    pub signer: &'a str,
    pub world: &'a str,
    pub scene_id: &'a str,
    pub parcel: &'a str,
    pub trusted_signers: &'a [String],
}

fn parse_claim(payload: &str) -> Option<ParsedClaim> {
    let mut lines = payload.split('\n');
    if lines.next() != Some(STORAGE_DELEGATION_PREFIX) {
        return None;
    }

    let mut values: [Option<&str>; CLAIM_FIELDS.len()] = [None; CLAIM_FIELDS.len()];
    for line in lines {
        let idx = CLAIM_FIELDS.iter().position(|p| line.starts_with(p))?;
        if values[idx].is_some() {
            return None;
        }
        values[idx] = Some(js_trim(&line[CLAIM_FIELDS[idx].len()..]));
    }

    let [ephemeral, world, scene_id, parcel, expiration_raw] =
        values.map(|v| v.filter(|s| !s.is_empty()));
    let (ephemeral, world, scene_id, parcel) = (ephemeral?, world?, scene_id?, parcel?);
    let expiration = date_parse(expiration_raw?)?;

    Some(ParsedClaim {
        ephemeral: ephemeral.to_lowercase(),
        world: world.to_lowercase(),
        scene_id: scene_id.to_string(),
        parcel: parcel.to_string(),
        expiration,
    })
}

pub fn verify_storage_delegation(
    scope_header: &str,
    target: &StorageDelegationTarget<'_>,
) -> Result<(), &'static str> {
    verify_storage_delegation_at(scope_header, target, Utc::now())
}

pub fn verify_storage_delegation_at(
    scope_header: &str,
    target: &StorageDelegationTarget<'_>,
    now: DateTime<Utc>,
) -> Result<(), &'static str> {
    if target.trusted_signers.is_empty() {
        return Err("no trusted authoritative signers configured");
    }
    if scope_header.len() > MAX_SCOPE_HEADER_LENGTH {
        return Err("scope header too large");
    }

    let decoded = node_base64_decode(scope_header);
    let parsed: serde_json::Value = serde_json::from_str(&String::from_utf8_lossy(&decoded))
        .map_err(|_| "malformed scope header")?;
    // `typeof parsed === 'object'`: an array passes and lacks both fields.
    if !(parsed.is_object() || parsed.is_array()) {
        return Err("malformed scope header");
    }

    let (Some(payload), Some(signature)) = (
        parsed.get("payload").and_then(|v| v.as_str()),
        parsed.get("signature").and_then(|v| v.as_str()),
    ) else {
        return Err("scope missing payload or signature");
    };

    let claim = parse_claim(payload).ok_or("unparseable claim")?;

    if claim.ephemeral != target.signer.to_lowercase() {
        return Err("claim ephemeral does not match request signer");
    }
    if claim.world != target.world.to_lowercase() {
        return Err("claim world does not match target world");
    }
    if claim.scene_id != target.scene_id {
        return Err("claim sceneId does not match target scene");
    }
    if claim.parcel != target.parcel {
        return Err("claim parcel does not match target parcel");
    }
    // `Date.parse` never yields NaN here (`date_parse` returns None instead).
    if claim.expiration <= now.timestamp_millis() as f64 {
        return Err("delegation expired");
    }

    let recovered = recover_personal_signer(payload.as_bytes(), signature)
        .ok_or("claim not signed by a trusted authoritative address")?;
    if target
        .trusted_signers
        .iter()
        .any(|s| s.eq_ignore_ascii_case(&recovered))
    {
        return Ok(());
    }
    Err("claim not signed by a trusted authoritative address")
}

/// `@dcl/crypto`'s personal-signature recovery: 65 hex bytes, `v` in
/// {0, 1, 27, 28}, and, unlike the shared chain verifier, a high-s
/// signature recovers too (noble's `recoverPublicKey` does not require low s).
fn recover_personal_signer(message: &[u8], signature: &str) -> Option<String> {
    let hex = signature.strip_prefix("0x").unwrap_or(signature);
    let bytes = hex::decode(hex).ok()?;
    if bytes.len() != 65 {
        return None;
    }
    let parity = match bytes[64] {
        0 | 27 => false,
        1 | 28 => true,
        _ => return None,
    };
    let sig = Signature::from_bytes_and_parity(&bytes[..64], parity);
    sig.recover_address_from_msg(message)
        .ok()
        .map(|a| format!("{a:#x}"))
}

/// The base64 the tests and the host encode the header with (standard
/// alphabet, padded).
pub fn base64_encode(data: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for chunk in data.chunks(3) {
        let mut acc: u32 = 0;
        for (i, &b) in chunk.iter().enumerate() {
            acc |= (b as u32) << (16 - 8 * i);
        }
        for i in 0..4 {
            if i <= chunk.len() {
                out.push(ALPHABET[((acc >> (18 - 6 * i)) & 0x3f) as usize] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
}

#[cfg(test)]
pub(crate) mod test_support {
    use alloy::signers::{Signer as _, local::PrivateKeySigner};
    use chrono::{DateTime, Utc};

    use super::{STORAGE_DELEGATION_PREFIX, base64_encode};

    pub(crate) fn claim_payload(
        ephemeral: &str,
        world: &str,
        scene_id: &str,
        parcel: &str,
        expiration: DateTime<Utc>,
    ) -> String {
        format!(
            "{STORAGE_DELEGATION_PREFIX}\nEphemeral: {ephemeral}\nWorld: {world}\nSceneId: {scene_id}\nParcel: {parcel}\nExpiration: {}",
            expiration.to_rfc3339()
        )
    }

    /// The header the authoritative server hands its worker: the claim signed
    /// EIP-191 by `authoritative`.
    pub(crate) async fn scope_header(authoritative: &PrivateKeySigner, payload: &str) -> String {
        let signature = authoritative
            .sign_message(payload.as_bytes())
            .await
            .unwrap()
            .to_string();
        let json = serde_json::json!({ "payload": payload, "signature": signature });
        base64_encode(json.to_string().as_bytes())
    }
}

#[cfg(test)]
mod tests {
    use super::test_support::{claim_payload, scope_header};
    use super::*;
    use alloy::signers::local::PrivateKeySigner;
    use chrono::Duration;

    const AUTHORITATIVE_KEY: &str =
        "0x4c0883a69102937d6231471b5dbb6204fe5129617082792ae468d01a3f362318";
    const OTHER_KEY: &str = "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80";

    const EPHEMERAL: &str = "0x1111111111111111111111111111111111111111";
    const WORLD: &str = "myworld.dcl.eth";
    const SCENE_ID: &str = "bafkreigcene";
    const PARCEL: &str = "10,-25";

    fn now() -> DateTime<Utc> {
        Utc::now()
    }

    fn wallet(key: &str) -> PrivateKeySigner {
        key.parse().unwrap()
    }

    fn payload(expiration: DateTime<Utc>) -> String {
        claim_payload(EPHEMERAL, WORLD, SCENE_ID, PARCEL, expiration)
    }

    fn target<'a>(trusted: &'a [String]) -> StorageDelegationTarget<'a> {
        StorageDelegationTarget {
            signer: EPHEMERAL,
            world: WORLD,
            scene_id: SCENE_ID,
            parcel: PARCEL,
            trusted_signers: trusted,
        }
    }

    fn trusted(wallet: &PrivateKeySigner) -> Vec<String> {
        vec![format!("{:#x}", wallet.address())]
    }

    #[tokio::test]
    async fn accepts_a_valid_delegation() {
        let wallet = wallet(AUTHORITATIVE_KEY);
        let header = scope_header(&wallet, &payload(now() + Duration::hours(1))).await;
        let t = trusted(&wallet);
        assert_eq!(
            verify_storage_delegation_at(&header, &target(&t), now()),
            Ok(())
        );
    }

    #[tokio::test]
    async fn rejects_untrusted_signer() {
        let other = wallet(OTHER_KEY);
        let authoritative = wallet(AUTHORITATIVE_KEY);
        let header = scope_header(&other, &payload(now() + Duration::hours(1))).await;
        let t = trusted(&authoritative);
        assert_eq!(
            verify_storage_delegation_at(&header, &target(&t), now()),
            Err("claim not signed by a trusted authoritative address")
        );
    }

    #[tokio::test]
    async fn rejects_when_no_trusted_signers_configured() {
        let wallet = wallet(AUTHORITATIVE_KEY);
        let header = scope_header(&wallet, &payload(now() + Duration::hours(1))).await;
        assert_eq!(
            verify_storage_delegation_at(&header, &target(&[]), now()),
            Err("no trusted authoritative signers configured")
        );
    }

    #[tokio::test]
    async fn rejects_wrong_ephemeral_world_scene_and_parcel() {
        let wallet = wallet(AUTHORITATIVE_KEY);
        let header = scope_header(&wallet, &payload(now() + Duration::hours(1))).await;
        let t = trusted(&wallet);

        let mut wrong = target(&t);
        wrong.signer = "0x2222222222222222222222222222222222222222";
        assert_eq!(
            verify_storage_delegation_at(&header, &wrong, now()),
            Err("claim ephemeral does not match request signer")
        );

        let mut wrong = target(&t);
        wrong.world = "other.dcl.eth";
        assert_eq!(
            verify_storage_delegation_at(&header, &wrong, now()),
            Err("claim world does not match target world")
        );

        let mut wrong = target(&t);
        wrong.scene_id = "bafkreiother";
        assert_eq!(
            verify_storage_delegation_at(&header, &wrong, now()),
            Err("claim sceneId does not match target scene")
        );

        let mut wrong = target(&t);
        wrong.parcel = "0,0";
        assert_eq!(
            verify_storage_delegation_at(&header, &wrong, now()),
            Err("claim parcel does not match target parcel")
        );
    }

    #[tokio::test]
    async fn world_match_is_case_insensitive() {
        let wallet = wallet(AUTHORITATIVE_KEY);
        let header = scope_header(&wallet, &payload(now() + Duration::hours(1))).await;
        let t = trusted(&wallet);
        let mut upper = target(&t);
        upper.world = "MyWorld.dcl.eth";
        assert_eq!(verify_storage_delegation_at(&header, &upper, now()), Ok(()));
    }

    #[tokio::test]
    async fn rejects_expired_delegation() {
        let wallet = wallet(AUTHORITATIVE_KEY);
        let header = scope_header(&wallet, &payload(now() - Duration::minutes(1))).await;
        let t = trusted(&wallet);
        assert_eq!(
            verify_storage_delegation_at(&header, &target(&t), now()),
            Err("delegation expired")
        );
    }

    #[test]
    fn rejects_oversized_header_before_decoding() {
        let wallet = wallet(AUTHORITATIVE_KEY);
        let header = "A".repeat(MAX_SCOPE_HEADER_LENGTH + 1);
        let t = trusted(&wallet);
        assert_eq!(
            verify_storage_delegation_at(&header, &target(&t), now()),
            Err("scope header too large")
        );
    }

    #[test]
    fn rejects_null_and_primitive_json_payloads() {
        let wallet = wallet(AUTHORITATIVE_KEY);
        let t = trusted(&wallet);
        for raw in ["null", "42", "\"str\""] {
            let header = base64_encode(raw.as_bytes());
            assert_eq!(
                verify_storage_delegation_at(&header, &target(&t), now()),
                Err("malformed scope header"),
                "payload {raw:?} must be rejected"
            );
        }
        for raw in [&br#"{"payload": 1, "signature": "0xff"}"#[..], b"[1,2]"] {
            assert_eq!(
                verify_storage_delegation_at(&base64_encode(raw), &target(&t), now()),
                Err("scope missing payload or signature")
            );
        }
        assert_eq!(
            verify_storage_delegation_at("!!!not-base64!!!", &target(&t), now()),
            Err("malformed scope header")
        );
    }

    #[test]
    fn claim_parser_requires_exactly_the_known_fields() {
        let exp = (now() + Duration::hours(1)).to_rfc3339();
        let valid = payload(now() + Duration::hours(1));
        assert!(parse_claim(&valid).is_some());

        assert!(parse_claim("wrong prefix\nEphemeral: 0x1").is_none());

        let missing = format!(
            "{STORAGE_DELEGATION_PREFIX}\nEphemeral: {EPHEMERAL}\nWorld: {WORLD}\nSceneId: {SCENE_ID}\nParcel: {PARCEL}"
        );
        assert!(parse_claim(&missing).is_none(), "missing Expiration");

        let duplicate = format!("{valid}\nWorld: {WORLD}");
        assert!(parse_claim(&duplicate).is_none(), "duplicate field");

        let unknown = format!("{valid}\nExtra: x");
        assert!(parse_claim(&unknown).is_none(), "unknown field");

        let bad_exp = format!(
            "{STORAGE_DELEGATION_PREFIX}\nEphemeral: {EPHEMERAL}\nWorld: {WORLD}\nSceneId: {SCENE_ID}\nParcel: {PARCEL}\nExpiration: tomorrow"
        );
        assert!(
            parse_claim(&bad_exp).is_none(),
            "an expiration Date.parse refuses"
        );

        let reordered = format!(
            "{STORAGE_DELEGATION_PREFIX}\nExpiration: {exp}\nParcel: {PARCEL}\nSceneId: {SCENE_ID}\nWorld: {WORLD}\nEphemeral: {EPHEMERAL}"
        );
        assert!(parse_claim(&reordered).is_some(), "field order is free");
    }

    /// What the upstream Node service accepts: `Date.parse` expirations, the
    /// URL-safe alphabet, missing padding, whitespace, and a high-s signature.
    #[tokio::test]
    async fn accepts_what_node_and_ethers_accept() {
        use alloy::signers::Signer as _;
        use alloy_primitives::U256;

        let wallet = wallet(AUTHORITATIVE_KEY);
        let t = trusted(&wallet);
        let base = format!(
            "{STORAGE_DELEGATION_PREFIX}\nEphemeral: {EPHEMERAL}\nWorld: {WORLD}\nSceneId: {SCENE_ID}\nParcel: {PARCEL}\nExpiration: "
        );
        for expiration in [
            "2999-01-01T00:00:00.000Z",
            "2999-01-01",
            "Tue, 01 Jan 2999 00:00:00 GMT",
            "January 1, 2999 10:00 PM",
        ] {
            let header = scope_header(&wallet, &format!("{base}{expiration}")).await;
            assert_eq!(
                verify_storage_delegation_at(&header, &target(&t), now()),
                Ok(()),
                "{expiration}"
            );
        }

        let payload = payload(now() + Duration::hours(1));
        let header = scope_header(&wallet, &payload).await;
        let url_safe: String = header
            .trim_end_matches('=')
            .chars()
            .map(|c| match c {
                '+' => '-',
                '/' => '_',
                c => c,
            })
            .collect();
        let spaced = format!(" {}\n{} ", &header[..10], &header[10..]);
        for variant in [url_safe, spaced] {
            assert_eq!(
                verify_storage_delegation_at(&variant, &target(&t), now()),
                Ok(()),
                "{variant}"
            );
        }

        let mut raw = wallet
            .sign_message(payload.as_bytes())
            .await
            .unwrap()
            .as_bytes();
        let n = U256::from_str_radix(
            "fffffffffffffffffffffffffffffffebaaedce6af48a03bbfd25e8cd0364141",
            16,
        )
        .unwrap();
        let high_s = n - U256::from_be_slice(&raw[32..64]);
        raw[32..64].copy_from_slice(&high_s.to_be_bytes::<32>());
        raw[64] = if raw[64] == 27 { 28 } else { 27 };
        let json = serde_json::json!({ "payload": payload, "signature": format!("0x{}", hex::encode(raw)) });
        let header = base64_encode(json.to_string().as_bytes());
        assert_eq!(
            verify_storage_delegation_at(&header, &target(&t), now()),
            Ok(()),
            "a high-s twin recovers the same signer"
        );
        raw[64] = 29;
        let json = serde_json::json!({ "payload": payload, "signature": format!("0x{}", hex::encode(raw)) });
        assert_eq!(
            verify_storage_delegation_at(
                &base64_encode(json.to_string().as_bytes()),
                &target(&t),
                now()
            ),
            Err("claim not signed by a trusted authoritative address")
        );
    }
}
