//! The operator bearer (`CATALYRST_CREDITS_ADMIN_TOKEN`, read from `_FILE`):
//! proof, wired into a handler's *signature*, that the request carried it.
//! axum refuses a handler into `Router::route` unless every argument
//! resolves, and the only way [`RequireAdmin`] resolves is the extractor
//! below, which compares the bearer in constant time and fails closed when
//! the token is unset. Every admin-auth failure renders as a **403** carrying
//! the `{ok:false,error,message}` envelope with the two old messages. The
//! actor an audit row records is server-chosen (`token:<VARIABLE>`), never a
//! client-supplied header.

use axum::extract::FromRequestParts;
use axum::http::HeaderMap;
use axum::http::request::Parts;
use serde::Deserialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use super::SharedState;
use super::errors::ApiError;
use crate::shared::signed_fetch::{
    AUTH_METADATA_HEADER, ORIGINAL_PATH_HEADER, PayloadPolicy, SignedPath, require_signer_at,
};

pub const ADMIN_TOKEN_ENV: &str = "CATALYRST_CREDITS_ADMIN_TOKEN";

fn bearer_token(headers: &HeaderMap) -> Option<&str> {
    headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.strip_prefix("Bearer "))
}

pub(crate) fn timing_safe_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

pub struct RequireAdmin(());

impl RequireAdmin {
    /// The verified audit actor: the name of the configured secret, never anything the client sent.
    pub fn audit_actor_description(&self) -> String {
        format!("token:{ADMIN_TOKEN_ENV}")
    }
}

pub(crate) fn establish_admin(
    configured: Option<&str>,
    headers: &HeaderMap,
) -> Result<RequireAdmin, ApiError> {
    let Some(expected) = configured.map(str::trim).filter(|s| !s.is_empty()) else {
        return Err(ApiError::forbidden(format!(
            "admin controls are disabled ({ADMIN_TOKEN_ENV} unset)"
        )));
    };
    match bearer_token(headers) {
        Some(got) if timing_safe_eq(got.as_bytes(), expected.as_bytes()) => Ok(RequireAdmin(())),
        _ => Err(ApiError::forbidden("invalid admin token")),
    }
}

/// The reports gate (the cross-wallet credits list and its grant reasons) and
/// the studio reads: the operator bearer, or a signed-fetch request whose
/// signer is one of `CREDITS_READ_ADMIN_ADDRESSES`, the old server's read-only
/// and read-write admin lists, or of `CREDITS_WRITE_ADMIN_ADDRESSES` (a write
/// admin reads too), so the ops dashboard reads them with its wallet as it
/// did. Anything else answers the old server's 401.
pub(crate) async fn require_read_admin(
    state: &SharedState,
    headers: &HeaderMap,
    route_path: &str,
    original: &axum::extract::OriginalUri,
) -> Result<(), ApiError> {
    const REFUSED: &str =
        "Unauthorized: the admin token or a read-admin signed request is required";
    if establish_admin(state.settings.admin_token.as_deref(), headers).is_ok() {
        return Ok(());
    }
    let settings = &state.settings;
    let admins = [
        settings.read_admin_addresses.as_slice(),
        settings.write_admin_addresses.as_slice(),
    ];
    signed_admin(
        state, headers, "GET", route_path, original, &admins, REFUSED,
    )
    .await
    .map(drop)
}

/// The signed-metadata key a wallet's write commits to its request body with:
/// the hex SHA-256 of the body's exact bytes.
pub const BODY_SHA256_KEY: &str = "bodySha256";

/// The studio write gate: the operator bearer, or a signed-fetch request over
/// `method` and the route whose signer is one of
/// `CREDITS_WRITE_ADMIN_ADDRESSES` (the old server's read-write admins), so
/// the ops dashboard changes studios with its wallet as it did. Answers the
/// audit actor: the bearer's [`RequireAdmin::audit_actor_description`], or the
/// signer's address, so a wallet's change names who made it. Anything else
/// answers the old server's 401.
///
/// A signed request proves the method, the route and the time, not the body.
/// So a wallet's write that signs [`BODY_SHA256_KEY`] must match `body`, or it
/// is refused before anything is written. One that signs no hash is accepted
/// with a warning, as credits-server accepted it, until
/// `CREDITS_ADMIN_WRITES_REQUIRE_BOUND_BODY` refuses it.
pub(crate) async fn require_write_admin(
    state: &SharedState,
    headers: &HeaderMap,
    method: &str,
    route_path: &str,
    original: &axum::extract::OriginalUri,
    body: &[u8],
) -> Result<String, ApiError> {
    const REFUSED: &str =
        "Unauthorized: the admin token or a write-admin signed request is required";
    if let Ok(admin) = establish_admin(state.settings.admin_token.as_deref(), headers) {
        return Ok(admin.audit_actor_description());
    }
    let admins = [state.settings.write_admin_addresses.as_slice()];
    let signer = signed_admin(
        state, headers, method, route_path, original, &admins, REFUSED,
    )
    .await?;
    if !signed_body_matches(headers, body)? {
        if state.settings.admin_writes_require_bound_body {
            return Err(ApiError::unauthorized(
                "Unauthorized: a wallet-signed write must sign its body (bodySha256 in the signed metadata)",
            ));
        }
        tracing::warn!(%signer, route = route_path, method, "a wallet-signed admin write signed no bodySha256; accepted while CREDITS_ADMIN_WRITES_REQUIRE_BOUND_BODY is off");
    }
    Ok(signer)
}

/// Whether a wallet's signed write commits to `body`: `true` when its signed
/// metadata carries [`BODY_SHA256_KEY`] and that is the SHA-256 of the exact
/// bytes sent, `false` when it signs no hash (the caller's policy decides),
/// and a 401 when the signed hash is another body's or cannot be read.
pub(crate) fn signed_body_matches(headers: &HeaderMap, body: &[u8]) -> Result<bool, ApiError> {
    match signed_body_hash(headers) {
        Some(Some(signed)) if signed.eq_ignore_ascii_case(&hex::encode(Sha256::digest(body))) => {
            Ok(true)
        }
        Some(_) => Err(ApiError::unauthorized(
            "Unauthorized: the signed bodySha256 does not match the request body",
        )),
        None => Ok(false),
    }
}

/// `headers` with a proxy `x-original-path` kept only when it IS `route_path`
/// (query stripped): the credits host's own spelling of the route. The shared
/// path rule admits one that merely ends with the route, so a signature over
/// a foreign prefix would otherwise open it.
pub(crate) fn exact_original_path(headers: &HeaderMap, route_path: &str) -> HeaderMap {
    let mut headers = headers.clone();
    let foreign = headers.get(ORIGINAL_PATH_HEADER).is_some_and(|v| {
        v.to_str().map_or(true, |raw| {
            raw.split('?').next().unwrap_or(raw) != route_path
        })
    });
    if foreign {
        headers.remove(ORIGINAL_PATH_HEADER);
    }
    headers
}

/// The body hash a request's signed metadata commits to: `None` when it signs
/// none, `Some(None)` when it cannot be read as one. A legacy signature covers
/// the metadata folded to lowercase, so another casing of the key would verify
/// as the same commitment: the key is matched in any case, and more than one
/// spelling of it is refused rather than one picked.
fn signed_body_hash(headers: &HeaderMap) -> Option<Option<String>> {
    let raw = headers.get(AUTH_METADATA_HEADER)?.to_str().ok()?;
    let metadata: Value = serde_json::from_str(raw).ok()?;
    let mut spellings = metadata
        .as_object()?
        .iter()
        .filter(|(key, _)| key.eq_ignore_ascii_case(BODY_SHA256_KEY));
    let (_, value) = spellings.next()?;
    if spellings.next().is_some() {
        return Some(None);
    }
    Some(value.as_str().map(str::to_string))
}

/// The signer of a signed-fetch request over `method` and `route_path` when it
/// is one of `admins`; otherwise `refused` as a 401. A write (any method but
/// GET) checks a contract wallet's signature with the uncached validator, so
/// a revoked delegation stops authorizing at once, not after the cache's
/// acceptance TTL; a validator with no uncached path refuses it.
async fn signed_admin(
    state: &SharedState,
    headers: &HeaderMap,
    method: &str,
    route_path: &str,
    original: &axum::extract::OriginalUri,
    admins: &[&[String]],
    refused: &'static str,
) -> Result<String, ApiError> {
    if admins.iter().all(|list| list.is_empty()) {
        return Err(ApiError::unauthorized(refused));
    }
    // The shared path rule admits a proxy `x-original-path` that merely ENDS
    // with the route, and a bare `/credits` is also the tail of every wallet's
    // own balance route (`/credits/users/{address}/credits`), which an admin
    // signs routinely. A replayed signature over that path must not open an
    // admin route, so here the header counts only when it IS the route.
    let headers = exact_original_path(headers, route_path);
    let validator = state.eip1654.as_deref();
    let validator = if !method.eq_ignore_ascii_case("get") {
        validator.and_then(|v| v.fresh_validator())
    } else {
        validator
    };
    let signer = require_signer_at(
        &headers,
        method,
        &SignedPath::at(&original.0, route_path),
        PayloadPolicy::V6ThenLegacy,
        validator,
    )
    .await
    .map_err(|_| ApiError::unauthorized(refused))?;
    if admins
        .iter()
        .any(|list| list.iter().any(|a| a == signer.as_str()))
    {
        Ok(signer.as_str().to_string())
    } else {
        Err(ApiError::unauthorized(refused))
    }
}

impl FromRequestParts<SharedState> for RequireAdmin {
    type Rejection = ApiError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &SharedState,
    ) -> Result<Self, Self::Rejection> {
        establish_admin(state.settings.admin_token.as_deref(), &parts.headers)
    }
}

pub fn validate_idempotency_key(raw: &Option<String>) -> Result<Option<String>, ApiError> {
    match raw {
        None => Ok(None),
        Some(k) => {
            let t = k.trim();
            if t.is_empty() {
                Ok(None)
            } else if t.len() > 200 {
                Err(ApiError::bad_request("idempotencyKey too long (max 200)"))
            } else if !t.chars().all(|c| c.is_ascii_graphic()) {
                Err(ApiError::bad_request(
                    "idempotencyKey must be printable ASCII",
                ))
            } else {
                Ok(Some(t.to_string()))
            }
        }
    }
}

pub fn normalize_address(raw: &str) -> Result<String, ApiError> {
    crate::shared::identity::Address::parse(raw)
        .map(|a| a.into_string())
        .map_err(|_| ApiError::bad_request("invalid wallet address"))
}

pub fn validate_positive_amount(raw: &str) -> Result<String, ApiError> {
    let s = raw.trim();
    if s.is_empty() || s.len() > 78 {
        return Err(ApiError::bad_request("invalid amount"));
    }
    let mut seen_dot = false;
    let mut any_digit = false;
    let mut any_nonzero = false;
    for c in s.chars() {
        match c {
            '0'..='9' => {
                any_digit = true;
                if c != '0' {
                    any_nonzero = true;
                }
            }
            '.' if !seen_dot => seen_dot = true,
            _ => return Err(ApiError::bad_request("invalid amount")),
        }
    }
    if !any_digit || !any_nonzero {
        return Err(ApiError::bad_request("amount must be a positive number"));
    }
    Ok(s.to_string())
}

pub fn validated_reason(reason: &Option<String>) -> Result<Option<String>, ApiError> {
    match reason {
        None => Ok(None),
        Some(r) => {
            let t = r.trim();
            if t.is_empty() {
                Ok(None)
            } else if t.len() > 500 {
                Err(ApiError::bad_request("reason too long (max 500)"))
            } else {
                Ok(Some(t.to_string()))
            }
        }
    }
}

/// An admin grant is Decentraland's: a studio's gift is granted by one of its
/// operators out of its budget (`POST /operator/studios/{studioId}/grants`),
/// which names the studio from its own row. A body that still names a
/// studio is refused rather than granted without one.
pub fn refuse_studio_name(value: Option<&Value>) -> Result<(), ApiError> {
    match value {
        // Blank is none, as every other optional field of these routes reads it.
        None | Some(Value::Null) => Ok(()),
        Some(Value::String(name)) if name.trim().is_empty() => Ok(()),
        Some(_) => Err(ApiError::bad_request(
            "\"studioName\" is not accepted: a studio's gift is granted by one of its operators, \
             with POST /operator/studios/{studioId}/grants",
        )),
    }
}

pub(crate) fn validate_escrow_ref(raw: &str) -> Result<String, ApiError> {
    let s = raw.trim();
    if s.is_empty() || s.len() > 200 {
        return Err(ApiError::bad_request("invalid escrowRef"));
    }
    if !s
        .chars()
        .all(|c| c.is_ascii_graphic() && c != '/' && c != '\\')
    {
        return Err(ApiError::bad_request("invalid escrowRef"));
    }
    Ok(s.to_string())
}

pub(crate) fn validate_price_cents(v: i64) -> Result<i64, ApiError> {
    if v < 0 {
        return Err(ApiError::bad_request("priceCents must be >= 0"));
    }
    Ok(v)
}

pub(crate) fn validate_currency(raw: &str) -> Result<String, ApiError> {
    let s = raw.trim().to_lowercase();
    if s.is_empty() || s.len() > 10 || !s.chars().all(|c| c.is_ascii_alphabetic()) {
        return Err(ApiError::bad_request("invalid currency"));
    }
    Ok(s)
}

pub(crate) fn paginate(limit: Option<i64>, offset: Option<i64>) -> (i64, i64) {
    let limit = limit.unwrap_or(50).clamp(1, 200);
    let offset = offset.unwrap_or(0).max(0);
    (limit, offset)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::credits::packs::validate_sku;
    use axum::http::{HeaderValue, StatusCode, header::AUTHORIZATION};
    use axum::response::IntoResponse;

    fn headers(authorization: Option<&str>) -> HeaderMap {
        let mut h = HeaderMap::new();
        if let Some(v) = authorization {
            h.insert(AUTHORIZATION, HeaderValue::from_str(v).unwrap());
        }
        h
    }

    async fn reject_status_and_body(
        configured: Option<&str>,
        authorization: Option<&str>,
    ) -> (StatusCode, serde_json::Value) {
        let error = match establish_admin(configured, &headers(authorization)) {
            Ok(_) => panic!("expected a rejection, not an established admin identity"),
            Err(error) => error,
        };
        let response = error.into_response();
        let status = response.status();
        let bytes = axum::body::to_bytes(response.into_body(), 4096)
            .await
            .expect("body collects");
        (status, serde_json::from_slice(&bytes).expect("json body"))
    }

    #[test]
    fn a_matching_bearer_yields_the_verified_service_token_actor() {
        let admin = establish_admin(Some("s3cret"), &headers(Some("Bearer s3cret")))
            .unwrap_or_else(|_| panic!("a matching secret establishes admin"));
        assert_eq!(
            admin.audit_actor_description(),
            "token:CATALYRST_CREDITS_ADMIN_TOKEN"
        );
    }

    #[test]
    fn an_admin_grant_names_no_studio() {
        assert!(refuse_studio_name(None).is_ok());
        assert!(refuse_studio_name(Some(&json!(null))).is_ok());
        assert!(refuse_studio_name(Some(&json!(""))).is_ok());
        assert!(refuse_studio_name(Some(&json!("   "))).is_ok());
        for named in [json!("Acme"), json!(" Acme "), json!(3)] {
            let refused = refuse_studio_name(Some(&named)).unwrap_err();
            assert!(
                refused.to_string().contains("/operator/studios/"),
                "{refused}"
            );
        }
    }

    #[tokio::test]
    async fn an_unset_token_fails_closed_as_403_disabled() {
        for presented in [Some("Bearer anything"), None] {
            let (status, body) = reject_status_and_body(None, presented).await;
            assert_eq!(status, StatusCode::FORBIDDEN);
            assert_eq!(body["ok"], serde_json::json!(false));
            assert_eq!(
                body["error"],
                "admin controls are disabled (CATALYRST_CREDITS_ADMIN_TOKEN unset)"
            );
            assert_eq!(
                body["message"],
                "admin controls are disabled (CATALYRST_CREDITS_ADMIN_TOKEN unset)"
            );
        }
    }

    #[tokio::test]
    async fn an_empty_configured_token_also_reads_as_disabled() {
        let (status, body) = reject_status_and_body(Some(""), Some("Bearer ")).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(
            body["error"],
            "admin controls are disabled (CATALYRST_CREDITS_ADMIN_TOKEN unset)"
        );
    }

    #[tokio::test]
    async fn a_missing_wrong_or_unprefixed_bearer_is_403_invalid() {
        for presented in [
            None,
            Some("Bearer nope"),
            Some("Basic s3cret"),
            Some("secret"),
        ] {
            let (status, body) = reject_status_and_body(Some("secret"), presented).await;
            assert_eq!(status, StatusCode::FORBIDDEN);
            assert_eq!(body["error"], "invalid admin token");
            assert_eq!(body["message"], "invalid admin token");
        }
    }

    #[test]
    fn a_correct_bearer_authorizes() {
        assert!(establish_admin(Some("secret"), &headers(Some("Bearer secret"))).is_ok());
    }

    #[test]
    fn validates_address() {
        assert!(normalize_address("0x1234567890abcdef1234567890abcdef12345678").is_ok());
        assert_eq!(
            normalize_address("0xABCDEF1234567890ABCDEF1234567890ABCDEF12").unwrap(),
            "0xabcdef1234567890abcdef1234567890abcdef12"
        );
        assert!(normalize_address("notanaddress").is_err());
        assert!(normalize_address("0x123").is_err());
    }

    #[test]
    fn validates_positive_amount() {
        assert_eq!(validate_positive_amount("100").unwrap(), "100");
        assert_eq!(validate_positive_amount(" 12.5 ").unwrap(), "12.5");
        assert!(validate_positive_amount("0").is_err());
        assert!(validate_positive_amount("0.0").is_err());
        assert!(validate_positive_amount("-5").is_err());
        assert!(validate_positive_amount("1e9").is_err());
        assert!(validate_positive_amount("").is_err());
    }

    #[test]
    fn validates_idempotency_key() {
        assert_eq!(validate_idempotency_key(&None).unwrap(), None);
        assert_eq!(validate_idempotency_key(&Some("  ".into())).unwrap(), None);
        assert_eq!(
            validate_idempotency_key(&Some(" grant-2026-001 ".into())).unwrap(),
            Some("grant-2026-001".to_string())
        );
        assert!(validate_idempotency_key(&Some("x".repeat(201))).is_err());
        assert!(validate_idempotency_key(&Some("bad key".into())).is_err());
        assert!(validate_idempotency_key(&Some("bad\nkey".into())).is_err());
    }

    #[test]
    fn validates_sku_and_escrow_ref_and_price_and_currency() {
        assert_eq!(validate_sku(" pack_100 ").unwrap(), "pack_100");
        assert_eq!(validate_sku(&"x".repeat(100)).unwrap(), "x".repeat(100));
        assert_eq!(validate_sku("pack_!.-@").unwrap(), "pack_!.-@");
        for invalid in ["bad\tpack", "bad\npack", "é", "a/b", "a\\b"] {
            assert_eq!(
                validate_sku(invalid).unwrap_err().to_string(),
                "invalid sku"
            );
        }

        assert!(validate_sku("").is_err());
        assert!(validate_sku("a/b").is_err());
        assert!(validate_sku("a\\b").is_err());
        assert!(validate_sku(&"x".repeat(101)).is_err());
        assert_eq!(validate_escrow_ref(" 0xdeadBEEF ").unwrap(), "0xdeadBEEF");
        assert!(validate_escrow_ref("").is_err());
        assert!(validate_escrow_ref("a/b").is_err());
        assert!(validate_escrow_ref(&"x".repeat(201)).is_err());
        assert_eq!(validate_price_cents(0).unwrap(), 0);
        assert_eq!(validate_price_cents(999).unwrap(), 999);
        assert!(validate_price_cents(-1).is_err());
        assert_eq!(validate_currency(" USD ").unwrap(), "usd");
        assert_eq!(validate_currency("eur").unwrap(), "eur");
        assert!(validate_currency("").is_err());
        assert!(validate_currency("us1").is_err());
        assert!(validate_currency(&"a".repeat(11)).is_err());
    }

    #[test]
    fn paginates_with_bounds() {
        assert_eq!(paginate(None, None), (50, 0));
        assert_eq!(paginate(Some(10), Some(5)), (10, 5));
        assert_eq!(paginate(Some(0), Some(-3)), (1, 0));
        assert_eq!(paginate(Some(9999), None), (200, 0));
    }
}

/// The most wallets one flag names: a linked set, not a dump.
pub const MAX_FLAGGED_ADDRESSES: usize = 100;

/// A flag as the events-notifier posted it to the credits server's `/flag`
/// (`{addressesLinked, ip, source}`): the admin's `POST /admin/flags` body
/// and the `credits.wallet_flags` notification.
#[derive(Debug, Deserialize)]
pub struct FlagBody {
    pub addresses: Vec<String>,
    pub source: String,
    #[serde(default)]
    pub ip: Option<String>,
}

/// A flag the store takes: one to [`MAX_FLAGGED_ADDRESSES`] distinct
/// lower-cased addresses, a source and an IP of at most 100 characters.
pub struct Flag {
    pub addresses: Vec<String>,
    pub source: String,
    pub ip: Option<String>,
}

impl FlagBody {
    pub fn validate(self) -> Result<Flag, ApiError> {
        if self.addresses.is_empty() || self.addresses.len() > MAX_FLAGGED_ADDRESSES {
            return Err(ApiError::bad_request(
                "addresses must name 1 to 100 wallets",
            ));
        }
        let mut addresses = self
            .addresses
            .iter()
            .map(|a| normalize_address(a))
            .collect::<Result<Vec<_>, _>>()?;
        addresses.sort();
        addresses.dedup();
        let source = self.source.trim();
        if source.is_empty() || source.len() > 100 {
            return Err(ApiError::bad_request("source is required (max 100)"));
        }
        let ip = self
            .ip
            .as_deref()
            .map(str::trim)
            .filter(|ip| !ip.is_empty() && ip.len() <= 100)
            .map(str::to_string);
        Ok(Flag {
            addresses,
            source: source.to_string(),
            ip,
        })
    }
}

impl Flag {
    /// The `user.flag` audit detail.
    pub fn detail(&self) -> Value {
        json!({ "addresses": self.addresses, "source": self.source, "ip": self.ip })
    }
}
