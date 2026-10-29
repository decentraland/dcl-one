use serde::{Deserialize, Serialize};

use super::error::AuthError;
use super::eth_address::is_eth_address;

pub const MAX_AUTH_CHAIN_LINKS: usize = 10;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AuthLinkType {
    SIGNER,
    #[serde(rename = "ECDSA_EPHEMERAL")]
    EcdsaEphemeral,
    #[serde(rename = "ECDSA_SIGNED_ENTITY")]
    EcdsaSignedEntity,
    #[serde(rename = "ECDSA_EIP_1654_EPHEMERAL")]
    EcdsaEip1654Ephemeral,
    #[serde(rename = "ECDSA_EIP_1654_SIGNED_ENTITY")]
    EcdsaEip1654SignedEntity,
}

impl std::fmt::Display for AuthLinkType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SIGNER => write!(f, "SIGNER"),
            Self::EcdsaEphemeral => write!(f, "ECDSA_EPHEMERAL"),
            Self::EcdsaSignedEntity => write!(f, "ECDSA_SIGNED_ENTITY"),
            Self::EcdsaEip1654Ephemeral => write!(f, "ECDSA_EIP_1654_EPHEMERAL"),
            Self::EcdsaEip1654SignedEntity => write!(f, "ECDSA_EIP_1654_SIGNED_ENTITY"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuthLink {
    #[serde(rename = "type")]
    pub link_type: AuthLinkType,

    pub payload: String,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signature: Option<String>,
}

pub type AuthChain = Vec<AuthLink>;

pub const MAX_AUTH_LINK_FIELD_LEN: usize = 100_000;

pub fn is_valid_auth_chain(chain: &AuthChain) -> bool {
    if chain.is_empty() {
        return false;
    }
    if chain.len() > MAX_AUTH_CHAIN_LINKS {
        return false;
    }
    for (i, link) in chain.iter().enumerate() {
        if i == 0 && link.link_type != AuthLinkType::SIGNER {
            return false;
        }
        if link.link_type == AuthLinkType::SIGNER && i != 0 {
            return false;
        }
        if link.payload.len() > MAX_AUTH_LINK_FIELD_LEN {
            return false;
        }
        if link
            .signature
            .as_ref()
            .is_some_and(|s| s.len() > MAX_AUTH_LINK_FIELD_LEN)
        {
            return false;
        }
    }
    true
}

pub fn parse_ephemeral_payload(payload: &str) -> Result<(String, String, i64), AuthError> {
    let (message, ephemeral_address, expiration) = split_ephemeral_payload(payload)?;
    let expiration = expiration.map_err(|raw| {
        AuthError::InvalidEphemeralPayload(format!("Invalid expiration date '{}'", raw))
    })?;
    Ok((message, ephemeral_address, expiration))
}

/// [`parse_ephemeral_payload`] with the expiration left as the raw text (`Err`) when no
/// known date shape parses it, so a caller mirroring a peer can keep the entity the peer
/// already accepted instead of refusing it. The structural checks are the same.
pub fn split_ephemeral_payload(
    payload: &str,
) -> Result<(String, String, Result<i64, String>), AuthError> {
    let message = payload.replace('\r', "");
    let parts: Vec<&str> = message.split('\n').collect();

    let ephemeral_prefix = "Ephemeral address: ";
    let expiration_prefix = "Expiration: ";

    if parts.len() < 3
        || !parts[1].starts_with(ephemeral_prefix)
        || !parts[2].starts_with(expiration_prefix)
    {
        return Err(AuthError::InvalidEphemeralPayload(
            "Expected 3 lines with 'Ephemeral address: ' on line 2 and 'Expiration: ' on line 3"
                .into(),
        ));
    }

    let ephemeral_address = parts[1][ephemeral_prefix.len()..].to_string();

    if !is_eth_address(&ephemeral_address) {
        return Err(AuthError::InvalidEphemeralPayload(
            "invalid ephemeral address format".into(),
        ));
    }

    let expiration_str = &parts[2][expiration_prefix.len()..];

    let expiration = parse_expiration(expiration_str).ok_or_else(|| expiration_str.to_string());

    Ok((message, ephemeral_address, expiration))
}

/// The date shapes `new Date(payload)` accepts in the clients that produced them: ISO 8601
/// (`Date.toISOString()`, with or without an offset or fractional seconds), the RFC 2822
/// form of `Date.toUTCString()`, and `Date.toString()` -- `Thu Feb 13 2020 15:31:10 GMT-0300
/// (Argentina Standard Time)`, which the 2020 web client wrote into its ephemeral payloads.
/// Its parenthesised zone name is informational and dropped; the offset has no colon.
pub fn parse_expiration(s: &str) -> Option<i64> {
    let s = s.trim();
    if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(s) {
        return Some(dt.timestamp_millis());
    }
    for fmt in ["%Y-%m-%dT%H:%M:%S%.f", "%Y-%m-%dT%H:%M:%S"] {
        if let Ok(naive) = chrono::NaiveDateTime::parse_from_str(s, fmt) {
            return Some(naive.and_utc().timestamp_millis());
        }
    }
    if let Ok(dt) = chrono::DateTime::parse_from_rfc2822(s) {
        return Some(dt.timestamp_millis());
    }
    let without_zone_name = match (s.rfind(" ("), s.ends_with(')')) {
        (Some(open), true) => s[..open].trim_end(),
        _ => s,
    };
    let without_weekday = without_zone_name
        .split_once(' ')
        .filter(|(day, _)| day.len() == 3 && day.chars().all(|c| c.is_ascii_alphabetic()))
        .map_or(without_zone_name, |(_, rest)| rest);
    for fmt in ["%b %d %Y %H:%M:%S GMT%z", "%b %d %Y %H:%M:%S %z"] {
        if let Ok(dt) = chrono::DateTime::parse_from_str(without_weekday, fmt) {
            return Some(dt.timestamp_millis());
        }
    }
    None
}

#[cfg(test)]
mod wire_tests {
    use super::*;

    #[test]
    fn auth_link_type_roundtrip() {
        let json = serde_json::to_string(&AuthLinkType::EcdsaEphemeral).unwrap();
        assert_eq!(json, "\"ECDSA_EPHEMERAL\"");
        let back: AuthLinkType = serde_json::from_str(&json).unwrap();
        assert_eq!(back, AuthLinkType::EcdsaEphemeral);
    }

    #[test]
    fn auth_link_type_display() {
        assert_eq!(AuthLinkType::SIGNER.to_string(), "SIGNER");
        assert_eq!(AuthLinkType::EcdsaEphemeral.to_string(), "ECDSA_EPHEMERAL");
        assert_eq!(
            AuthLinkType::EcdsaSignedEntity.to_string(),
            "ECDSA_SIGNED_ENTITY"
        );
        assert_eq!(
            AuthLinkType::EcdsaEip1654Ephemeral.to_string(),
            "ECDSA_EIP_1654_EPHEMERAL"
        );
        assert_eq!(
            AuthLinkType::EcdsaEip1654SignedEntity.to_string(),
            "ECDSA_EIP_1654_SIGNED_ENTITY"
        );
    }

    #[test]
    fn auth_link_signer_serialization() {
        let link = AuthLink {
            link_type: AuthLinkType::SIGNER,
            payload: "0xabc123".to_string(),
            signature: None,
        };
        let json = serde_json::to_value(&link).unwrap();
        assert_eq!(json["type"], "SIGNER");
        assert_eq!(json["payload"], "0xabc123");
        assert!(json.get("signature").is_none());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_auth_link_type_serde_roundtrip() {
        let json = r#""ECDSA_EPHEMERAL""#;
        let parsed: AuthLinkType = serde_json::from_str(json).unwrap();
        assert_eq!(parsed, AuthLinkType::EcdsaEphemeral);
        assert_eq!(serde_json::to_string(&parsed).unwrap(), json);
    }

    #[test]
    fn test_auth_link_type_signed_entity_serde() {
        let json = r#""ECDSA_SIGNED_ENTITY""#;
        let parsed: AuthLinkType = serde_json::from_str(json).unwrap();
        assert_eq!(parsed, AuthLinkType::EcdsaSignedEntity);
    }

    #[test]
    fn test_is_valid_auth_chain_ok() {
        let chain = vec![
            AuthLink {
                link_type: AuthLinkType::SIGNER,
                payload: "0xabc".into(),
                signature: None,
            },
            AuthLink {
                link_type: AuthLinkType::EcdsaSignedEntity,
                payload: "hash".into(),
                signature: Some("0xsig".into()),
            },
        ];
        assert!(is_valid_auth_chain(&chain));
    }

    #[test]
    fn test_is_valid_auth_chain_empty() {
        assert!(!is_valid_auth_chain(&vec![]));
    }

    #[test]
    fn test_is_valid_auth_chain_no_signer_first() {
        let chain = vec![AuthLink {
            link_type: AuthLinkType::EcdsaSignedEntity,
            payload: "hash".into(),
            signature: Some("0xsig".into()),
        }];
        assert!(!is_valid_auth_chain(&chain));
    }

    #[test]
    fn test_is_valid_auth_chain_duplicate_signer() {
        let chain = vec![
            AuthLink {
                link_type: AuthLinkType::SIGNER,
                payload: "0xabc".into(),
                signature: None,
            },
            AuthLink {
                link_type: AuthLinkType::SIGNER,
                payload: "0xdef".into(),
                signature: None,
            },
        ];
        assert!(!is_valid_auth_chain(&chain));
    }

    #[test]
    fn test_parse_ephemeral_payload() {
        let payload = "Decentraland Login\nEphemeral address: 0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266\nExpiration: 2025-01-20T22:57:11.334Z";
        let (msg, addr, exp) = parse_ephemeral_payload(payload).unwrap();
        assert_eq!(msg, payload);
        assert_eq!(addr, "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266");
        assert!(exp > 0);
    }

    #[test]
    fn test_parse_ephemeral_payload_unity_editor_format() {
        let payload = "Decentraland Login\nEphemeral address: 0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266\nExpiration: 2036-06-10T16:07:42";
        let (_, _, exp) = parse_ephemeral_payload(payload).unwrap();
        assert!(exp > 2_000_000_000_000);
    }

    #[test]
    fn test_parse_ephemeral_payload_strips_cr() {
        let payload = "Decentraland Login\r\nEphemeral address: 0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266\r\nExpiration: 2025-01-20T22:57:11.334Z";
        let (msg, addr, _) = parse_ephemeral_payload(payload).unwrap();
        assert_eq!(addr, "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266");
        assert!(!msg.contains('\r'));
    }

    #[test]
    fn test_parse_ephemeral_payload_bad_format() {
        assert!(parse_ephemeral_payload("garbage").is_err());
    }

    #[test]
    fn parse_expiration_accepts_js_date_to_string() {
        let ms = 1_581_618_670_000; // 2020-02-13T18:31:10Z
        assert_eq!(
            parse_expiration("Thu Feb 13 2020 15:31:10 GMT-0300 (Argentina Standard Time)"),
            Some(ms)
        );
        assert_eq!(
            parse_expiration("Thu Feb 13 2020 15:31:10 GMT-0300"),
            Some(ms)
        );
        assert_eq!(parse_expiration("2020-02-13T18:31:10.000Z"), Some(ms));
        assert_eq!(parse_expiration("Thu, 13 Feb 2020 18:31:10 GMT"), Some(ms));
        assert_eq!(
            parse_expiration("Sat Feb 15 2020 16:53:08 GMT+0000 (Coordinated Universal Time)"),
            Some(1_581_785_588_000)
        );
        assert_eq!(
            parse_expiration("Fri Feb 14 2020 12:55:44 GMT-0500 (Eastern Standard Time)"),
            Some(1_581_702_944_000)
        );
    }

    #[test]
    fn parse_expiration_js_date_ignores_the_weekday_like_new_date() {
        assert_eq!(
            parse_expiration("Mon Feb 13 2020 15:31:10 GMT-0300 (Argentina Standard Time)"),
            Some(1_581_618_670_000)
        );
    }

    #[test]
    fn parse_expiration_js_date_far_future_parses() {
        let ms = parse_expiration("Wed Aug 07 7112 17:52:35 GMT-0300 (Argentina Standard Time)")
            .expect("year 7112 is a date, not a panic");
        assert!(ms > 160_000_000_000_000, "{ms}");
    }

    #[test]
    fn parse_expiration_rejects_unknown_shapes() {
        for s in [
            "",
            "next Tuesday",
            "Thu Feb 13 2020",
            "13/02/2020 15:31",
            "1581618670000",
        ] {
            assert_eq!(parse_expiration(s), None, "{s:?}");
        }
        let payload = "Decentraland Login\nEphemeral address: 0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266\nExpiration: next Tuesday";
        let err = parse_ephemeral_payload(payload).unwrap_err();
        assert_eq!(
            err.to_string(),
            "Invalid ephemeral payload: Invalid expiration date 'next Tuesday'"
        );
        let (_, addr, expiration) = split_ephemeral_payload(payload).unwrap();
        assert_eq!(addr, "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266");
        assert_eq!(expiration, Err("next Tuesday".to_string()));
    }

    #[test]
    fn test_parse_ephemeral_payload_js_date() {
        let payload = "Decentraland Login\nEphemeral address: 0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266\nExpiration: Thu Feb 13 2020 15:31:10 GMT-0300 (Argentina Standard Time)";
        let (_, _, exp) = parse_ephemeral_payload(payload).unwrap();
        assert_eq!(exp, 1_581_618_670_000);
    }

    #[test]
    fn test_is_eth_address() {
        assert!(is_eth_address("0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266"));
        assert!(is_eth_address("0x0000000000000000000000000000000000000000"));
        assert!(!is_eth_address("f39Fd6e51aad88F6F4ce6aB8827279cffFb92266"));
        assert!(!is_eth_address("0x1234"));
        assert!(!is_eth_address(
            "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb922660"
        ));
        assert!(!is_eth_address(
            "0xZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZ"
        ));
    }

    #[test]
    fn test_parse_ephemeral_payload_rejects_malformed_address() {
        let payload = "Decentraland Login\nEphemeral address: not-an-address\nExpiration: 2025-01-20T22:57:11.334Z";
        let result = parse_ephemeral_payload(payload);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            format!("{}", err).contains("invalid ephemeral address format"),
            "Expected 'invalid ephemeral address format', got: {}",
            err
        );
    }

    #[test]
    fn test_auth_chain_deserialization() {
        let json = r#"[
            {"type": "SIGNER", "payload": "0xabc"},
            {"type": "ECDSA_EPHEMERAL", "payload": "Decentraland Login\nEphemeral address: 0x123\nExpiration: 2025-01-01T00:00:00.000Z", "signature": "0xdeadbeef"},
            {"type": "ECDSA_SIGNED_ENTITY", "payload": "bafyhash", "signature": "0xcafebabe"}
        ]"#;
        let chain: AuthChain = serde_json::from_str(json).unwrap();
        assert_eq!(chain.len(), 3);
        assert_eq!(chain[0].link_type, AuthLinkType::SIGNER);
        assert_eq!(chain[1].link_type, AuthLinkType::EcdsaEphemeral);
        assert_eq!(chain[2].link_type, AuthLinkType::EcdsaSignedEntity);
    }

    #[test]
    fn test_is_valid_auth_chain_rejects_oversized_payload() {
        let over = AuthLink {
            link_type: AuthLinkType::SIGNER,
            payload: "a".repeat(MAX_AUTH_LINK_FIELD_LEN + 1),
            signature: None,
        };
        assert!(!is_valid_auth_chain(&vec![over]));

        let at_limit = AuthLink {
            link_type: AuthLinkType::SIGNER,
            payload: "a".repeat(MAX_AUTH_LINK_FIELD_LEN),
            signature: None,
        };
        assert!(is_valid_auth_chain(&vec![at_limit]));
    }

    #[test]
    fn test_is_valid_auth_chain_rejects_oversized_signature() {
        let make = |sig_len: usize| {
            vec![
                AuthLink {
                    link_type: AuthLinkType::SIGNER,
                    payload: "0xabc".into(),
                    signature: None,
                },
                AuthLink {
                    link_type: AuthLinkType::EcdsaSignedEntity,
                    payload: "hash".into(),
                    signature: Some("a".repeat(sig_len)),
                },
            ]
        };
        assert!(!is_valid_auth_chain(&make(MAX_AUTH_LINK_FIELD_LEN + 1)));
        assert!(is_valid_auth_chain(&make(MAX_AUTH_LINK_FIELD_LEN)));
    }

    #[test]
    fn test_is_valid_auth_chain_count_bound_still_holds() {
        let well_formed = |n: usize| {
            let mut chain = vec![AuthLink {
                link_type: AuthLinkType::SIGNER,
                payload: "0xabc".into(),
                signature: None,
            }];
            for _ in 1..n {
                chain.push(AuthLink {
                    link_type: AuthLinkType::EcdsaSignedEntity,
                    payload: "hash".into(),
                    signature: Some("0xsig".into()),
                });
            }
            chain
        };
        assert!(is_valid_auth_chain(&well_formed(MAX_AUTH_CHAIN_LINKS)));
        assert!(!is_valid_auth_chain(&well_formed(MAX_AUTH_CHAIN_LINKS + 1)));
    }
}
