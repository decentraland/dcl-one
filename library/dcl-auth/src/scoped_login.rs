//! Scoped logins (ADR-162): a delegation that grants an ephemeral key only
//! named operations of named services. The message, its strict parser, the
//! chain structure a scoped link may sit in, and the check of an operation
//! against the grants a verifier accepted.

use serde::{Deserialize, Serialize};

use crate::auth_chain::{AuthLink, AuthLinkType};
use crate::js_date;

/// First line of a scoped login message.
pub const SCOPED_LOGIN_PREFIX: &str = "Decentraland Scoped Login";
/// Default maximum remaining validity of a scoped delegation: 24 hours.
pub const DEFAULT_MAX_SCOPED_LOGIN_LIFETIME_MS: i64 = 24 * 60 * 60 * 1000;
pub const MAX_SCOPED_LOGIN_GRANTS: usize = 32;
pub const MAX_SCOPED_LOGIN_PAYLOAD_BYTES: usize = 4096;

const EPHEMERAL_PREFIX: &str = "Ephemeral: ";
const EXPIRATION_PREFIX: &str = "Expiration: ";
const GRANT_PREFIX: &str = "Allow: ";

/// One `Allow` line.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScopedLoginGrant {
    pub service: String,
    /// An operation of the service, or `*` for all of them.
    pub operation: String,
    /// Limits the grant to this resource; `None` is every resource.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resource: Option<String>,
}

/// What a scoped login allows for one service.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScopedLoginScope {
    pub service: String,
    /// The earliest expiration of the chain's delegations, in Unix ms.
    pub expiration: i64,
    /// The grants for `service` only.
    pub grants: Vec<ScopedLoginGrant>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedScopedPayload {
    /// Lowercase address of the key the login delegates to.
    pub ephemeral_address: String,
    /// Unix ms.
    pub expiration: i64,
    /// Every grant, for every service.
    pub grants: Vec<ScopedLoginGrant>,
}

fn is_name(value: &str) -> bool {
    (1..=64).contains(&value.len())
        && value
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

/// `^[a-z0-9-]{1,64}$`.
pub fn is_service(value: &str) -> bool {
    is_name(value)
}

/// `^(?:[a-z0-9-]{1,64}|\*)$`.
pub fn is_operation_format(value: &str) -> bool {
    value == "*" || is_name(value)
}

/// `^[\x21-\x7e]{1,256}$`: visible ASCII, no whitespace.
pub fn is_resource(value: &str) -> bool {
    (1..=256).contains(&value.len()) && value.bytes().all(|b| (0x21..=0x7e).contains(&b))
}

fn is_lower_address(value: &str) -> bool {
    value.len() == 42
        && value.starts_with("0x")
        && value[2..]
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

fn invalid(what: &str) -> String {
    format!("Invalid scoped login {what}")
}

fn grant_line(grant: &ScopedLoginGrant) -> String {
    match &grant.resource {
        Some(resource) => format!(
            "{GRANT_PREFIX}{}:{} for {resource}",
            grant.service, grant.operation
        ),
        None => format!("{GRANT_PREFIX}{}:{}", grant.service, grant.operation),
    }
}

fn parse_grant(line: &str) -> Option<ScopedLoginGrant> {
    let rest = line.strip_prefix(GRANT_PREFIX)?;
    let (action, resource) = match rest.split_once(" for ") {
        Some((action, resource)) => (action, Some(resource)),
        None => (rest, None),
    };
    let (service, operation) = action.split_once(':')?;
    if !is_name(service) || !is_operation_format(operation) || !resource.is_none_or(is_resource) {
        return None;
    }
    Some(ScopedLoginGrant {
        service: service.to_string(),
        operation: operation.to_string(),
        resource: resource.map(str::to_string),
    })
}

/// The message a key signs to delegate only `grants` to `ephemeral_address`
/// until `expiration_ms`.
pub fn scoped_ephemeral_message(
    ephemeral_address: &str,
    expiration_ms: i64,
    grants: &[ScopedLoginGrant],
) -> Result<String, String> {
    if !crate::eth_address::is_eth_address(ephemeral_address) {
        return Err(invalid("ephemeral address"));
    }
    let expiration = js_date::to_iso_string(expiration_ms).ok_or_else(|| invalid("expiration"))?;
    if grants.is_empty() || grants.len() > MAX_SCOPED_LOGIN_GRANTS {
        return Err(format!(
            "A scoped login needs between 1 and {MAX_SCOPED_LOGIN_GRANTS} grants"
        ));
    }
    let mut lines = Vec::with_capacity(grants.len());
    for grant in grants {
        if !is_service(&grant.service)
            || !is_operation_format(&grant.operation)
            || !grant.resource.as_deref().is_none_or(is_resource)
        {
            return Err(invalid("grant"));
        }
        let line = grant_line(grant);
        if lines.contains(&line) {
            return Err("Repeated scoped login grant".into());
        }
        lines.push(line);
    }
    let message = format!(
        "{SCOPED_LOGIN_PREFIX}\n{EPHEMERAL_PREFIX}{}\n{EXPIRATION_PREFIX}{expiration}\n{}",
        ephemeral_address.to_lowercase(),
        lines.join("\n")
    );
    if message.len() > MAX_SCOPED_LOGIN_PAYLOAD_BYTES {
        return Err(format!(
            "A scoped login message cannot exceed {MAX_SCOPED_LOGIN_PAYLOAD_BYTES} bytes"
        ));
    }
    Ok(message)
}

/// Parses a scoped login message strictly: the prefix, `Ephemeral` and
/// `Expiration` lines in that order, then 1 to [`MAX_SCOPED_LOGIN_GRANTS`]
/// distinct `Allow` lines, separated by single LFs. Nothing is trimmed or
/// folded, and only the exact `Date#toISOString()` form of the expiration
/// is accepted.
pub fn parse_scoped_ephemeral_payload(payload: &str) -> Result<ParsedScopedPayload, String> {
    if payload.len() > MAX_SCOPED_LOGIN_PAYLOAD_BYTES || payload.contains('\r') {
        return Err(invalid("payload"));
    }
    let mut lines = payload.split('\n');
    let prefix = lines.next();
    let ephemeral = lines.next().and_then(|l| l.strip_prefix(EPHEMERAL_PREFIX));
    let expiration = lines.next().and_then(|l| l.strip_prefix(EXPIRATION_PREFIX));
    let grant_lines: Vec<&str> = lines.collect();
    let distinct = grant_lines
        .iter()
        .enumerate()
        .all(|(i, line)| !grant_lines[..i].contains(line));
    let (Some(SCOPED_LOGIN_PREFIX), Some(ephemeral), Some(expiration_text)) =
        (prefix, ephemeral, expiration)
    else {
        return Err(invalid("payload"));
    };
    if grant_lines.is_empty() || grant_lines.len() > MAX_SCOPED_LOGIN_GRANTS || !distinct {
        return Err(invalid("payload"));
    }
    if !is_lower_address(ephemeral) {
        return Err(invalid("ephemeral address"));
    }
    let expiration = js_date::parse(expiration_text)
        .filter(|&ms| js_date::to_iso_string(ms).as_deref() == Some(expiration_text))
        .ok_or_else(|| invalid("expiration"))?;
    let grants = grant_lines
        .iter()
        .map(|line| parse_grant(line).ok_or_else(|| invalid("grant")))
        .collect::<Result<_, _>>()?;
    Ok(ParsedScopedPayload {
        ephemeral_address: ephemeral.to_string(),
        expiration,
        grants,
    })
}

/// The index of the chain's scoped link, `None` when it has none. A scoped
/// chain is a SIGNER, any number of full-login ephemeral links, exactly one
/// scoped link, then at most one `ECDSA_SIGNED_ENTITY`; a contract-wallet
/// scoped link follows the SIGNER directly.
pub fn find_scoped_link(chain: &[AuthLink]) -> Result<Option<usize>, String> {
    let scoped: Vec<usize> = chain
        .iter()
        .enumerate()
        .filter(|(_, link)| link.link_type.is_scoped())
        .map(|(index, _)| index)
        .collect();
    let Some(&index) = scoped.first() else {
        return Ok(None);
    };
    let last = chain.len() - 1;
    let delegations_ok = chain[1.min(index)..index].iter().all(|link| {
        matches!(
            link.link_type,
            AuthLinkType::EcdsaEphemeral | AuthLinkType::EcdsaEip1654Ephemeral
        )
    });
    let tail_ok = index == last
        || (index + 1 == last && chain[last].link_type == AuthLinkType::EcdsaSignedEntity);
    let contract_ok =
        chain[index].link_type != AuthLinkType::EcdsaEip1654ScopedEphemeral || index == 1;
    if scoped.len() > 1 || !tail_ok || !delegations_ok || !contract_ok {
        return Err("Invalid scoped login chain: a scoped link must be the only one, and only an ECDSA_SIGNED_ENTITY link may follow it".into());
    }
    Ok(Some(index))
}

/// Whether `scope` allows `operation`, optionally on `resource`. A full
/// login (`None`) allows everything; `*` is not an operation.
pub fn is_operation_granted(
    scope: Option<&ScopedLoginScope>,
    operation: &str,
    resource: Option<&str>,
) -> bool {
    let Some(scope) = scope else {
        return true;
    };
    if operation == "*" || !is_operation_format(operation) {
        return false;
    }
    scope.grants.iter().any(|grant| {
        (grant.operation == operation || grant.operation == "*")
            && (grant.resource.is_none() || grant.resource.as_deref() == resource)
    })
}

/// Why an action is refused against an already verified scope.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScopeRefusal {
    OperationRequired,
    ResourceInvalid,
    OperationDenied,
}

/// The action's syntax and the grants, checked the same way for every caller.
pub fn authorize_scope(
    scope: Option<&ScopedLoginScope>,
    service: Option<&str>,
    operation: Option<&str>,
    resource: Option<&str>,
) -> Result<(), ScopeRefusal> {
    let action_ok = match (service, operation) {
        (Some(service), Some(operation)) => {
            is_service(service)
                && scope.is_none_or(|scope| scope.service == service)
                && operation != "*"
                && is_operation_format(operation)
        }
        _ => false,
    };
    if !action_ok {
        return Err(ScopeRefusal::OperationRequired);
    }
    if resource.is_some_and(|r| !is_resource(r)) {
        return Err(ScopeRefusal::ResourceInvalid);
    }
    if is_operation_granted(scope, operation.unwrap_or_default(), resource) {
        Ok(())
    } else {
        Err(ScopeRefusal::OperationDenied)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const EPHEMERAL: &str = "0x00000000000000000000000000000000000000aa";

    fn grant(service: &str, operation: &str, resource: Option<&str>) -> ScopedLoginGrant {
        ScopedLoginGrant {
            service: service.into(),
            operation: operation.into(),
            resource: resource.map(Into::into),
        }
    }

    fn link(link_type: AuthLinkType) -> AuthLink {
        AuthLink {
            link_type,
            payload: String::new(),
            signature: Some("0x".into()),
        }
    }

    #[test]
    fn the_message_round_trips() {
        let grants = vec![
            grant("insights", "logs-read", None),
            grant("insights", "*", Some("scene:bafy")),
        ];
        let message = scoped_ephemeral_message(EPHEMERAL, 1_581_618_670_000, &grants).unwrap();
        assert_eq!(
            message,
            "Decentraland Scoped Login\nEphemeral: 0x00000000000000000000000000000000000000aa\nExpiration: 2020-02-13T18:31:10.000Z\nAllow: insights:logs-read\nAllow: insights:* for scene:bafy"
        );
        let parsed = parse_scoped_ephemeral_payload(&message).unwrap();
        assert_eq!(parsed.ephemeral_address, EPHEMERAL);
        assert_eq!(parsed.expiration, 1_581_618_670_000);
        assert_eq!(parsed.grants, grants);
    }

    #[test]
    fn the_parser_refuses_any_deviation() {
        let ok = "Decentraland Scoped Login\nEphemeral: 0x00000000000000000000000000000000000000aa\nExpiration: 2020-02-13T18:31:10.000Z\nAllow: insights:logs-read";
        assert!(parse_scoped_ephemeral_payload(ok).is_ok());
        for bad in [
            ok.replace('\n', "\r\n"),
            ok.replace("Scoped Login", "Login"),
            ok.replace("0x00000000000000000000000000000000000000aa", "0x00000000000000000000000000000000000000AA"),
            ok.replace(".000Z", "Z"),
            ok.replace("2020-02-13T18:31:10.000Z", "Thu, 13 Feb 2020 18:31:10 GMT"),
            ok.replace("logs-read", "Logs-Read"),
            ok.replace("logs-read", "logs-read for "),
            ok.replace("logs-read", "logs-read for a b"),
            format!("{ok}\nAllow: insights:logs-read"),
            format!("{ok}\n"),
            "Decentraland Scoped Login\nEphemeral: 0x00000000000000000000000000000000000000aa\nExpiration: 2020-02-13T18:31:10.000Z".to_string(),
            format!("{ok}{}", "\nAllow: a:b".repeat(40)),
        ] {
            assert!(parse_scoped_ephemeral_payload(&bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn a_scoped_link_sits_where_adr_162_puts_it() {
        use AuthLinkType::*;
        let chain = |types: &[AuthLinkType]| types.iter().map(|t| link(*t)).collect::<Vec<_>>();
        assert_eq!(
            find_scoped_link(&chain(&[SIGNER, EcdsaEphemeral])),
            Ok(None)
        );
        assert_eq!(
            find_scoped_link(&chain(&[SIGNER, EcdsaScopedEphemeral])),
            Ok(Some(1))
        );
        assert_eq!(
            find_scoped_link(&chain(&[SIGNER, EcdsaScopedEphemeral, EcdsaSignedEntity])),
            Ok(Some(1))
        );
        assert_eq!(
            find_scoped_link(&chain(&[
                SIGNER,
                EcdsaEphemeral,
                EcdsaEip1654Ephemeral,
                EcdsaScopedEphemeral,
                EcdsaSignedEntity
            ])),
            Ok(Some(3))
        );
        assert_eq!(
            find_scoped_link(&chain(&[
                SIGNER,
                EcdsaEip1654ScopedEphemeral,
                EcdsaSignedEntity
            ])),
            Ok(Some(1))
        );
        for bad in [
            vec![SIGNER, EcdsaEphemeral, EcdsaEip1654ScopedEphemeral],
            vec![SIGNER, EcdsaScopedEphemeral, EcdsaScopedEphemeral],
            vec![SIGNER, EcdsaScopedEphemeral, EcdsaEphemeral],
            vec![SIGNER, EcdsaScopedEphemeral, EcdsaEip1654SignedEntity],
            vec![
                SIGNER,
                EcdsaScopedEphemeral,
                EcdsaSignedEntity,
                EcdsaSignedEntity,
            ],
            vec![SIGNER, EcdsaSignedEntity, EcdsaScopedEphemeral],
        ] {
            assert!(find_scoped_link(&chain(&bad)).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn operations_are_granted_by_name_wildcard_and_resource() {
        let scope = ScopedLoginScope {
            service: "insights".into(),
            expiration: 0,
            grants: vec![
                grant("insights", "logs-read", None),
                grant("insights", "*", Some("scene:a")),
            ],
        };
        assert!(is_operation_granted(None, "anything", None));
        assert!(is_operation_granted(Some(&scope), "logs-read", None));
        assert!(is_operation_granted(
            Some(&scope),
            "logs-read",
            Some("scene:b")
        ));
        assert!(is_operation_granted(
            Some(&scope),
            "deploy",
            Some("scene:a")
        ));
        assert!(!is_operation_granted(
            Some(&scope),
            "deploy",
            Some("scene:b")
        ));
        assert!(!is_operation_granted(Some(&scope), "deploy", None));
        assert!(!is_operation_granted(Some(&scope), "*", Some("scene:a")));
        let authorize = |service, operation, resource| {
            authorize_scope(Some(&scope), service, operation, resource)
        };
        assert_eq!(authorize(Some("insights"), Some("logs-read"), None), Ok(()));
        assert_eq!(
            authorize(Some("insights"), None, None),
            Err(ScopeRefusal::OperationRequired)
        );
        assert_eq!(
            authorize(Some("other"), Some("logs-read"), None),
            Err(ScopeRefusal::OperationRequired)
        );
        assert_eq!(
            authorize(Some("insights"), Some("*"), None),
            Err(ScopeRefusal::OperationRequired)
        );
        assert_eq!(
            authorize(Some("insights"), Some("logs-read"), Some("a b")),
            Err(ScopeRefusal::ResourceInvalid)
        );
        assert_eq!(
            authorize(Some("insights"), Some("deploy"), Some("scene:b")),
            Err(ScopeRefusal::OperationDenied)
        );
    }
}
