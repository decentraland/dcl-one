//! One verifier for signed requests (`x-identity-*`), shared by every surface
//! that takes one: HTTP routes, WebSocket auth frames and the Pulse handshake.
//! It verifies and answers as `@dcl/crypto-middleware` 6.5.0 does, in its
//! stage order:
//!
//! 1. the chain headers `x-identity-auth-chain-0..max` (an empty value ends
//!    the chain; one at index `max` is refused);
//! 2. the timestamp, digits only, at most the expiration window old and at
//!    most that window, capped at 60 s unless the options lift the cap, ahead;
//! 3. the metadata object (absent: `{}` and an empty payload segment);
//! 4. the service's `metadata_validator`;
//! 5. the signature over `method:path:timestamp:metadata` (method and path
//!    lowercased, metadata as received); on any 401, with
//!    `canonical_metadata_keys`, the key guard, then the pre-6.0.0 payload
//!    folded whole;
//! 6. a scoped login (ADR-162) against the route's operation and resource.
//!
//! The path may also be the gateway's `x-original-path` when it ends with the
//! route ([`crate::signed_fetch::candidate_paths`]). The chain rules are
//! stricter than the reference's: only delegations between the SIGNER and the
//! last link, high-s signatures refused, no contract call for an address in
//! the precompile range. Contract-wallet links get at most
//! [`MAX_RPC_CALLS`] JSON-RPC calls of [`RPC_CALL_TIMEOUT`] each, within
//! [`SIGNATURE_VALIDATION_TIMEOUT`].

mod error;
mod metadata;
mod options;
mod parts;

#[cfg(test)]
mod tests;

use std::time::Duration;

use serde_json::Value;

pub use error::{RequestAuthError, ScopedErrorCode};
pub use metadata::{
    Metadata, MetadataPredicate, canonical_field, reject_if_signer, require_canonical_field,
    require_signer,
};
pub use options::{
    DEFAULT_EXPIRATION, DEFAULT_MAX_CHAIN_LENGTH, OptionsError, Resolve, ResolveFailed, Resolver,
    VerifyOptions, VerifyOptionsBuilder,
};
pub use parts::{RequestParts, header_value};

use crate::auth_chain::{AuthChain, AuthLink, AuthLinkType};
use crate::error::AuthError;
use crate::rpc_validator::{RPC_BUDGET, RpcBudget};
use crate::scoped_login::{
    ScopeRefusal, ScopedLoginScope, authorize_scope, is_resource, parse_scoped_ephemeral_payload,
};
use crate::verify::{WalkRules, Walked, walk_chain};
use error::{ScopedErrorCode as Code, forbidden, malformed, safe, unauthorized};

/// The deadline of one signature validation, its RPC calls included.
pub const SIGNATURE_VALIDATION_TIMEOUT: Duration = Duration::from_secs(15);
/// The JSON-RPC calls one signature validation may make.
pub const MAX_RPC_CALLS: usize = 16;
/// The deadline of one JSON-RPC call.
pub const RPC_CALL_TIMEOUT: Duration = Duration::from_secs(5);

/// A verified, authorized signed request.
#[derive(Debug, Clone, PartialEq)]
pub struct VerifiedRequest {
    /// The SIGNER's address, lowercased.
    pub signer: String,
    pub metadata: Metadata,
    /// What a scoped login may do on this service; `None` for a full login.
    pub scope: Option<ScopedLoginScope>,
    /// The resource the route resolved and the verifier authorized.
    pub resource: Option<String>,
}

/// Verifies and authorizes a signed request now. `Ok(None)` only with
/// `optional`, for a request that carries no verifiable signature.
pub async fn verify_request(
    parts: &RequestParts,
    options: &VerifyOptions,
) -> Result<Option<VerifiedRequest>, RequestAuthError> {
    verify_request_at(parts, options, chrono::Utc::now().timestamp_millis()).await
}

/// [`verify_request`] at `now_ms`.
pub async fn verify_request_at(
    parts: &RequestParts,
    options: &VerifyOptions,
    now_ms: i64,
) -> Result<Option<VerifiedRequest>, RequestAuthError> {
    let verified = match verify_identity(parts, options, now_ms).await {
        Ok(verified) => verified,
        Err(_) if options.optional => return Ok(None),
        Err(e) => return Err(e),
    };
    authorize(verified, parts, options).map(Some)
}

/// A link as the chain headers carry it: three strings.
struct RawLink {
    link_type: String,
    payload: String,
    signature: String,
}

fn extract_chain(parts: &RequestParts, max: usize) -> Result<Vec<RawLink>, RequestAuthError> {
    let mut chain = Vec::new();
    for index in 0..max {
        let Some(raw) = parts.link(index) else {
            break;
        };
        let value: Value = serde_json::from_str(raw)
            .map_err(|e| malformed(format!("Invalid chain format: {}", safe(&e.to_string()))))?;
        let field = |name: &str| value.get(name).and_then(Value::as_str).map(str::to_string);
        let (Some(link_type), Some(payload), Some(signature)) =
            (field("type"), field("payload"), field("signature"))
        else {
            return Err(malformed(format!(
                "Invalid chain format: malformed auth link at position {index}"
            )));
        };
        chain.push(RawLink {
            link_type,
            payload,
            signature,
        });
    }
    if parts.link(max).is_some() {
        return Err(malformed(format!(
            "Auth chain exceeds maximum length of {max}"
        )));
    }
    if chain.len() <= 1 {
        return Err(malformed("Invalid Auth Chain"));
    }
    Ok(chain)
}

/// An absent or empty timestamp reads as 0, which is expired. The window bounds the
/// past, `future_ms` (`None`: nothing) the future.
fn check_timestamp(
    raw: Option<&str>,
    window_ms: i64,
    future_ms: Option<i64>,
    now_ms: i64,
) -> Result<(), RequestAuthError> {
    let raw = raw.filter(|raw| !raw.is_empty()).unwrap_or("0");
    if !raw.bytes().all(|b| b.is_ascii_digit()) {
        return Err(malformed(format!("Invalid chain timestamp: {}", safe(raw))));
    }
    let timestamp = raw
        .parse::<u128>()
        .ok()
        .and_then(|t| i128::try_from(t).ok())
        .unwrap_or(i128::MAX);
    let (window, now) = (i128::from(window_ms), i128::from(now_ms));
    if timestamp.saturating_add(window) < now {
        return Err(unauthorized(format!(
            "Expired signature: signature timestamp: {timestamp}, timestamp expiration: {}, local timestamp: {now}",
            timestamp.saturating_add(window)
        )));
    }
    if future_ms.is_some_and(|ahead| timestamp > now + i128::from(ahead)) {
        return Err(unauthorized(format!(
            "Signature timestamp is too far in the future: signature timestamp: {timestamp}, local timestamp: {now}"
        )));
    }
    Ok(())
}

/// The crypto-middleware >= 6.0.0 payload.
pub fn create_payload(method: &str, path: &str, timestamp: &str, metadata: &str) -> String {
    format!(
        "{}:{}:{}:{metadata}",
        method.to_lowercase(),
        path.to_lowercase(),
        timestamp.to_lowercase()
    )
}

/// The crypto-middleware < 6.0.0 payload: everything folded, metadata included.
pub fn create_legacy_payload(method: &str, path: &str, timestamp: &str, metadata: &str) -> String {
    format!("{method}:{path}:{timestamp}:{metadata}").to_lowercase()
}

enum Refusal {
    Invalid(String),
    Unavailable,
}

impl From<Refusal> for RequestAuthError {
    fn from(refusal: Refusal) -> Self {
        match refusal {
            Refusal::Invalid(message) => unauthorized(format!("Invalid signature: {message}")),
            Refusal::Unavailable => RequestAuthError::Unavailable,
        }
    }
}

fn to_auth_chain(raw: &[RawLink]) -> Result<AuthChain, Refusal> {
    raw.iter()
        .map(|link| {
            let link_type: AuthLinkType =
                serde_json::from_value(Value::String(link.link_type.clone())).map_err(|_| {
                    Refusal::Invalid(format!(
                        "ERROR. Link type: {}. Unknown link type.",
                        safe(&link.link_type)
                    ))
                })?;
            Ok(AuthLink {
                link_type,
                payload: link.payload.clone(),
                signature: Some(link.signature.clone()),
            })
        })
        .collect()
}

/// The chain's walk under the RPC budget and the validation deadline.
async fn walk(chain: &AuthChain, now_ms: i64, options: &VerifyOptions) -> Result<Walked, Refusal> {
    let budget = RpcBudget::new(MAX_RPC_CALLS, RPC_CALL_TIMEOUT);
    let rules = WalkRules::request(options.service.as_deref());
    let run = RPC_BUDGET.scope(
        budget.clone(),
        walk_chain(chain, Some(now_ms), options.eip1654.as_deref(), rules),
    );
    let result = tokio::time::timeout(SIGNATURE_VALIDATION_TIMEOUT, run).await;
    if budget.exceeded() {
        return Err(Refusal::Invalid(format!(
            "Auth chain validation exceeded {MAX_RPC_CALLS} RPC calls"
        )));
    }
    if budget.failed() {
        return Err(Refusal::Unavailable);
    }
    match result {
        Err(_) | Ok(Err(AuthError::Eip1654ValidationFailed(_))) => Err(Refusal::Unavailable),
        Ok(Err(e)) => Err(Refusal::Invalid(e.to_string())),
        Ok(Ok(walked)) => Ok(walked),
    }
}

/// The scope of a scoped login, once its chain verified: the grants for this
/// service, within the lifetime cap.
fn scope_of(
    chain: &AuthChain,
    walked: &Walked,
    now_ms: i64,
    options: &VerifyOptions,
) -> Result<Option<ScopedLoginScope>, Refusal> {
    let (Some((index, expiration)), Some(service)) = (walked.scoped, options.service.as_deref())
    else {
        return Ok(None);
    };
    let parsed = parse_scoped_ephemeral_payload(&chain[index].payload).map_err(Refusal::Invalid)?;
    let grants: Vec<_> = parsed
        .grants
        .into_iter()
        .filter(|grant| grant.service == service)
        .collect();
    if grants.is_empty() {
        return Err(Refusal::Invalid(format!(
            "ERROR: The scoped login grants nothing to \"{service}\""
        )));
    }
    if expiration.saturating_sub(now_ms) > options.max_scoped_login_lifetime_ms {
        return Err(Refusal::Invalid(
            "ERROR: The scoped login lasts longer than this verifier accepts".into(),
        ));
    }
    Ok(Some(ScopedLoginScope {
        service: service.to_string(),
        expiration,
        grants,
    }))
}

/// The walked chain's verdict for one payload shape over every candidate path.
fn settle(
    chain: &AuthChain,
    walked: &Result<Walked, Refusal>,
    payloads: &[String],
    now_ms: i64,
    options: &VerifyOptions,
) -> Result<Option<ScopedLoginScope>, Refusal> {
    let walked = match walked {
        Ok(walked) => walked,
        Err(Refusal::Invalid(message)) => return Err(Refusal::Invalid(message.clone())),
        Err(Refusal::Unavailable) => return Err(Refusal::Unavailable),
    };
    if !payloads.contains(&walked.authority) {
        return Err(Refusal::Invalid(format!(
            "ERROR: Invalid final authority. Expected: {}. Current {}.",
            safe(payloads.first().map_or("", String::as_str)),
            safe(&walked.authority)
        )));
    }
    scope_of(chain, walked, now_ms, options)
}

async fn verify_identity(
    parts: &RequestParts,
    options: &VerifyOptions,
    now_ms: i64,
) -> Result<VerifiedRequest, RequestAuthError> {
    let raw_chain = extract_chain(parts, options.max_chain_length)?;
    check_timestamp(
        parts.timestamp.as_deref(),
        options.expiration_ms,
        options.future_ms,
        now_ms,
    )?;
    let metadata = metadata::parse_metadata(parts.metadata.as_deref())?;
    if let Some(validator) = &options.metadata_validator
        && !validator(&metadata)
    {
        return Err(malformed(format!(
            "Invalid metadata content: {}",
            safe(&Value::Object(metadata).to_string())
        )));
    }

    let paths = parts.candidate_paths();
    let timestamp = parts.timestamp.as_deref().unwrap_or("");
    let raw_metadata = parts.metadata.as_deref().unwrap_or("");
    let payloads = |create: fn(&str, &str, &str, &str) -> String| -> Vec<String> {
        paths
            .iter()
            .map(|path| create(&parts.method, path, timestamp, raw_metadata))
            .collect()
    };

    let (chain, walked) = match to_auth_chain(&raw_chain) {
        Ok(chain) => {
            let walked = walk(&chain, now_ms, options).await;
            (chain, walked)
        }
        Err(refusal) => (Vec::new(), Err(refusal)),
    };
    let scope = match settle(&chain, &walked, &payloads(create_payload), now_ms, options) {
        Ok(scope) => scope,
        Err(Refusal::Unavailable) => return Err(RequestAuthError::Unavailable),
        Err(refusal) => {
            let Some(keys) = &options.canonical_metadata_keys else {
                return Err(refusal.into());
            };
            metadata::assert_legacy_metadata_keys(&metadata, keys)?;
            settle(
                &chain,
                &walked,
                &payloads(create_legacy_payload),
                now_ms,
                options,
            )?
        }
    };

    Ok(VerifiedRequest {
        signer: raw_chain[0].payload.to_lowercase(),
        metadata,
        scope,
        resource: None,
    })
}

fn authorize(
    mut verified: VerifiedRequest,
    parts: &RequestParts,
    options: &VerifyOptions,
) -> Result<VerifiedRequest, RequestAuthError> {
    let resolve = |option: &Option<Resolve>| match option {
        Some(resolve) => resolve.resolve(parts),
        None => Ok(None),
    };
    let (operation, resource) = match (resolve(&options.operation), resolve(&options.resource)) {
        (Ok(operation), Ok(resource)) => (operation, resource),
        _ => {
            return Err(forbidden(
                Code::ResolutionFailed,
                "Cannot resolve the request operation or resource",
            ));
        }
    };
    let resource_required = || {
        forbidden(
            Code::ResourceRequired,
            "Cannot resolve the request resource",
        )
    };
    if options.resource.is_some() && resource.is_none() {
        return Err(resource_required());
    }
    if let Some(scope) = &verified.scope {
        authorize_scope(
            Some(scope),
            options.service.as_deref(),
            operation.as_deref(),
            resource.as_deref(),
        )
        .map_err(|refusal| match refusal {
            ScopeRefusal::OperationRequired => forbidden(
                Code::OperationRequired,
                "This route does not declare a scoped operation",
            ),
            ScopeRefusal::ResourceInvalid => resource_required(),
            ScopeRefusal::OperationDenied => forbidden(
                Code::OperationDenied,
                format!(
                    "The scoped login does not grant \"{}\"",
                    operation.as_deref().unwrap_or_default()
                ),
            ),
        })?;
    }
    if let Some(resource) = resource {
        if !is_resource(&resource) {
            return Err(resource_required());
        }
        verified.resource = Some(resource);
    }
    Ok(verified)
}
