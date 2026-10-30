//! The wire envelopes of the community REST surface: `limit/offset/page`
//! pagination with the 1-based `page` the social service publishes, the
//! `{ data }` and `{ results, total, page, pages, limit }` bodies, and the
//! error envelope `{ error, message }` of upstream's `@dcl/http-commons`
//! (never an `ok` key: the explorer boot corpus row 42).
//!
//! The signed-fetch gate renders as upstream social-service-ea does: a
//! request without a usable auth chain is a **400** `{ error: "Invalid Auth
//! Chain", message: <ADR-44> }` (the explorer boot corpus rows 43 and 58,
//! `tests/explorer-boot.md`); a chain that is present but does not verify is
//! a 401 with the verifier's reason under `error` and the same message. The
//! old Rust port answered 401 for a missing chain too; the corpus is the
//! acceptance here.

use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::Serialize;
use serde_json::json;

use crate::AuthorityNotEstablished;
use crate::shared::signed_fetch::SignedFetchError;

pub const MAX_LIMIT: i64 = 100;
pub const MAX_OFFSET: i64 = 100_000;

pub const ADR44_MESSAGE: &str = "This endpoint requires a signed fetch request. See ADR-44.";

/// Upstream's answer to a malformed id reaching a query (`src/adapters/pg.ts`
/// at `fb3dcb8`, Postgres' 22P02 as `InvalidRequestError`), rendered
/// `{ error: "Bad request", message }` (capture `social/community-by-id-bad-id`).
pub const INVALID_IDENTIFIER: &str = "Invalid identifier";

/// Matches upstream's own cut.
const METADATA_ECHO_MAX: usize = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Pagination {
    pub limit: i64,
    pub offset: i64,
}

fn limit_or_max(requested: Option<i64>, max: i64) -> i64 {
    match requested {
        Some(n) if n > 0 && n <= max => n,
        _ => max,
    }
}

/// `limit` (1..=100, else 100), `offset` (>= 0, else `page * limit`, else 0),
/// the offset capped at 100,000. A negative offset such as the explorer's
/// `offset=-100` reads as 0.
pub fn get_pagination_params(pairs: &[(String, String)]) -> Pagination {
    let mut limit_raw: Option<&str> = None;
    let mut offset_raw: Option<&str> = None;
    let mut page_raw: Option<&str> = None;
    for (k, v) in pairs {
        match k.as_str() {
            "limit" if limit_raw.is_none() => limit_raw = Some(v),
            "offset" if offset_raw.is_none() => offset_raw = Some(v),
            "page" if page_raw.is_none() => page_raw = Some(v),
            _ => {}
        }
    }
    let limit = limit_or_max(limit_raw.and_then(|s| s.parse().ok()), MAX_LIMIT);
    let offset = match offset_raw.and_then(|s| s.parse::<i64>().ok()) {
        Some(n) if n >= 0 => n,
        _ => match page_raw.and_then(|s| s.parse::<i64>().ok()) {
            Some(p) if p >= 0 => p.saturating_mul(limit),
            _ => 0,
        },
    };
    Pagination {
        limit,
        offset: offset.clamp(0, MAX_OFFSET),
    }
}

pub fn get_first(pairs: &[(String, String)], key: &str) -> Option<String> {
    pairs.iter().find(|(k, _)| k == key).map(|(_, v)| v.clone())
}

pub fn get_all(pairs: &[(String, String)], key: &str) -> Vec<String> {
    pairs
        .iter()
        .filter(|(k, _)| k == key)
        .map(|(_, v)| v.clone())
        .collect()
}

pub fn get_bool(pairs: &[(String, String)], key: &str) -> bool {
    get_first(pairs, key)
        .map(|v| v.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
}

#[derive(Debug, Serialize)]
pub struct EnvelopeData<T> {
    pub data: T,
}

#[derive(Debug, Serialize)]
pub struct Paginated<T> {
    pub results: Vec<T>,
    pub total: i64,
    pub page: i64,
    pub pages: i64,
    pub limit: i64,
}

impl<T> Paginated<T> {
    /// The 1-based `page` the social service publishes; the first page is 1.
    pub fn new(results: Vec<T>, total: i64, pagination: &Pagination) -> Self {
        let (limit, offset) = (pagination.limit, pagination.offset);
        let page = if limit > 0 { offset / limit } else { 0 } + 1;
        let pages = if limit > 0 {
            (total + limit - 1) / limit
        } else {
            0
        };
        Self {
            results,
            total,
            page,
            pages,
            limit,
        }
    }
}

/// `{ error, message }`.
pub fn error_body(error: impl Into<String>, message: impl Into<String>) -> serde_json::Value {
    json!({ "error": error.into(), "message": message.into() })
}

/// The one-message form: `error` and `message` are the same text.
pub fn err_json(status: StatusCode, message: impl Into<String>) -> Response {
    let message = message.into();
    (status, Json(error_body(message.clone(), message))).into_response()
}

#[derive(Debug)]
pub enum CommError {
    BadRequest(String),
    NotAuthorized(String),
    NotFound(String),
    Status(StatusCode, String),
    SignedFetchGate {
        status: StatusCode,
        error: String,
    },
    /// A handler's own `{ message }` body, without `error` (upstream's
    /// moderation refusal, capture `social/moderation-communities-signed-not-moderator`).
    MessageOnly(StatusCode, String),
    Internal,
}

impl CommError {
    pub fn bad_request(msg: impl Into<String>) -> Self {
        CommError::BadRequest(msg.into())
    }
    pub fn not_authorized(msg: impl Into<String>) -> Self {
        CommError::NotAuthorized(msg.into())
    }
    pub fn not_found(msg: impl Into<String>) -> Self {
        CommError::NotFound(msg.into())
    }
    pub fn status(code: StatusCode, msg: impl Into<String>) -> Self {
        CommError::Status(code, msg.into())
    }
    pub fn forbidden(msg: impl Into<String>) -> Self {
        CommError::Status(StatusCode::FORBIDDEN, msg.into())
    }
}

impl IntoResponse for CommError {
    fn into_response(self) -> Response {
        let (status, body) = match self {
            CommError::BadRequest(message) => {
                (StatusCode::BAD_REQUEST, error_body("Bad request", message))
            }
            CommError::NotAuthorized(message) => (
                StatusCode::UNAUTHORIZED,
                error_body("Not Authorized", message),
            ),
            CommError::NotFound(message) => {
                (StatusCode::NOT_FOUND, error_body("Not Found", message))
            }
            CommError::Status(code, message) => (code, error_body(message.clone(), message)),
            CommError::SignedFetchGate { status, error } => {
                (status, error_body(error, ADR44_MESSAGE))
            }
            CommError::MessageOnly(code, message) => (code, json!({ "message": message })),
            // upstream's `errorHandler` (`@dcl/http-commons` 2.0.1, the version
            // fb3dcb8's yarn.lock pins) answers every other error so, without a message
            CommError::Internal => (
                StatusCode::INTERNAL_SERVER_ERROR,
                json!({ "error": "Internal Server Error" }),
            ),
        };
        (status, Json(body)).into_response()
    }
}

fn echo_metadata(metadata: &str) -> String {
    if metadata.chars().count() <= METADATA_ECHO_MAX {
        return metadata.to_string();
    }
    let head: String = metadata.chars().take(METADATA_ECHO_MAX).collect();
    format!("{head}\u{2026}")
}

/// Why a signed fetch was refused, before or during verification.
#[derive(Debug)]
pub enum SignedFetchRejection {
    /// The metadata named the scene signer (ADR-44): answered before any
    /// crypto, echoing the metadata as upstream does.
    RefusedMetadata(String),
    Chain(SignedFetchError),
}

impl From<SignedFetchError> for SignedFetchRejection {
    fn from(e: SignedFetchError) -> Self {
        SignedFetchRejection::Chain(e)
    }
}

pub fn signed_fetch_gate_parts(e: SignedFetchRejection) -> (StatusCode, String) {
    use SignedFetchError as E;
    let e = match e {
        SignedFetchRejection::RefusedMetadata(metadata) => {
            return (
                StatusCode::BAD_REQUEST,
                format!("Invalid metadata content: {}", echo_metadata(&metadata)),
            );
        }
        SignedFetchRejection::Chain(e) => e,
    };
    match e {
        E::InsufficientLinks | E::MalformedChain { .. } => {
            (StatusCode::BAD_REQUEST, "Invalid Auth Chain".to_string())
        }
        E::InvalidTimestamp(v) => (
            StatusCode::BAD_REQUEST,
            format!("Invalid chain timestamp: {v}"),
        ),
        E::InvalidSignature(d) => (StatusCode::UNAUTHORIZED, format!("Invalid signature: {d}")),
        E::Expired { .. } => (StatusCode::UNAUTHORIZED, "Expired signature".to_string()),
        E::EipNotImplemented | E::CatalystUnavailable(_) => {
            (StatusCode::UNAUTHORIZED, "Invalid signature".to_string())
        }
    }
}

pub fn signed_fetch_gate(e: SignedFetchRejection) -> CommError {
    let (status, error) = signed_fetch_gate_parts(e);
    CommError::SignedFetchGate { status, error }
}

impl From<SignedFetchRejection> for CommError {
    fn from(e: SignedFetchRejection) -> Self {
        signed_fetch_gate(e)
    }
}

/// For the read-path handlers: a refusal renders 401, a backing-store
/// failure 500. The two cannot be confused for one another on the way here.
impl From<AuthorityNotEstablished> for CommError {
    fn from(e: AuthorityNotEstablished) -> Self {
        match e {
            AuthorityNotEstablished::RefusedLacksAuthority { detail } => {
                CommError::NotAuthorized(detail)
            }
            AuthorityNotEstablished::AuthenticationMissingOrInvalid { detail } => {
                CommError::NotAuthorized(detail.to_string())
            }
            AuthorityNotEstablished::PresentedSharedSecretDidNotMatch => {
                CommError::NotAuthorized("presented shared secret did not match".to_string())
            }
            AuthorityNotEstablished::UndeterminedStoreUnavailable {
                store,
                reason_for_operators,
            } => {
                tracing::error!(%store, reason = %reason_for_operators, "community membership authority could not be determined");
                CommError::Internal
            }
            AuthorityNotEstablished::CredentialNotConfigured {
                environment_variable,
            } => {
                tracing::error!(%environment_variable, "credential not configured");
                CommError::Internal
            }
        }
    }
}

impl From<sqlx::Error> for CommError {
    fn from(e: sqlx::Error) -> Self {
        tracing::error!(error = %e, "sqlx error on a communities route");
        CommError::Internal
    }
}

/// The client write paths render a refusal with their own status and
/// sentence (401 and a UI-facing message); an undetermined authority is a
/// 500 "database error", never that sentence, so an outage cannot read as a
/// decision.
pub fn status_and_message_for_refusal(
    refusal: &AuthorityNotEstablished,
    status_when_the_principal_lacks_the_authority: StatusCode,
    message_when_the_principal_lacks_the_authority: impl FnOnce() -> String,
) -> (StatusCode, String) {
    match refusal {
        AuthorityNotEstablished::RefusedLacksAuthority { detail } => {
            tracing::debug!(%detail, "community membership authority refused");
            (
                status_when_the_principal_lacks_the_authority,
                message_when_the_principal_lacks_the_authority(),
            )
        }
        AuthorityNotEstablished::AuthenticationMissingOrInvalid { detail } => {
            (StatusCode::UNAUTHORIZED, format!("auth chain: {detail}"))
        }
        AuthorityNotEstablished::PresentedSharedSecretDidNotMatch => (
            StatusCode::UNAUTHORIZED,
            "presented shared secret did not match".to_string(),
        ),
        AuthorityNotEstablished::UndeterminedStoreUnavailable {
            store,
            reason_for_operators,
        } => {
            tracing::error!(%store, reason = %reason_for_operators, "community membership authority could not be determined; refusing without deciding");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "database error".to_string(),
            )
        }
        AuthorityNotEstablished::CredentialNotConfigured {
            environment_variable,
        } => {
            tracing::error!(%environment_variable, "community membership authority reached a credential gate it does not use");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "credential not configured".to_string(),
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pairs(s: &[(&str, &str)]) -> Vec<(String, String)> {
        s.iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    #[test]
    fn offset_is_capped_and_ordinary_values_pass_through() {
        assert_eq!(
            get_pagination_params(&pairs(&[("offset", "250")])).offset,
            250
        );
        assert_eq!(
            get_pagination_params(&pairs(&[("offset", "1000000")])).offset,
            MAX_OFFSET
        );
        assert_eq!(
            get_pagination_params(&pairs(&[("page", "999999999"), ("limit", "100")])).offset,
            MAX_OFFSET
        );
        let p = get_pagination_params(&pairs(&[("limit", "24"), ("page", "2")]));
        assert_eq!((p.limit, p.offset), (24, 48));
        let p = get_pagination_params(&pairs(&[("offset", "-100"), ("limit", "100")]));
        assert_eq!(
            (p.limit, p.offset),
            (100, 0),
            "the explorer's offset=-100 reads as the first page"
        );
        assert_eq!(
            get_pagination_params(&pairs(&[("limit", "500")])).limit,
            MAX_LIMIT
        );
        assert_eq!(
            get_pagination_params(&pairs(&[("limit", "0")])).limit,
            MAX_LIMIT
        );
    }

    #[test]
    fn paginated_page_is_one_based() {
        let p = Paginated::new(
            vec![1, 2],
            5,
            &Pagination {
                limit: 2,
                offset: 0,
            },
        );
        assert_eq!((p.page, p.pages, p.limit, p.total), (1, 3, 2, 5));
        let p = Paginated::new(
            Vec::<i32>::new(),
            5,
            &Pagination {
                limit: 2,
                offset: 4,
            },
        );
        assert_eq!(p.page, 3);
    }

    #[tokio::test]
    async fn error_envelope_wire_shape() {
        let resp = CommError::not_found("community not found").into_response();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
        let bytes = axum::body::to_bytes(resp.into_body(), 1024).await.unwrap();
        let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(
            v,
            json!({ "error": "Not Found", "message": "community not found" })
        );
    }

    #[tokio::test]
    async fn internal_envelope_is_upstreams_error_handler_body() {
        let resp = CommError::Internal.into_response();
        assert_eq!(resp.status(), StatusCode::INTERNAL_SERVER_ERROR);
        let bytes = axum::body::to_bytes(resp.into_body(), 1024).await.unwrap();
        assert_eq!(&bytes[..], br#"{"error":"Internal Server Error"}"#);
    }

    #[tokio::test]
    async fn status_envelope_wire_shape() {
        let resp = CommError::status(StatusCode::SERVICE_UNAVAILABLE, "friends unavailable")
            .into_response();
        assert_eq!(resp.status(), StatusCode::SERVICE_UNAVAILABLE);
        let bytes = axum::body::to_bytes(resp.into_body(), 1024).await.unwrap();
        let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(
            v,
            json!({ "error": "friends unavailable", "message": "friends unavailable" })
        );
    }

    #[tokio::test]
    async fn refused_metadata_wire_shape() {
        let raw = r#"{"signer":"decentraland-kernel-scene"}"#;
        let resp = signed_fetch_gate(SignedFetchRejection::RefusedMetadata(raw.to_string()))
            .into_response();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
        let bytes = axum::body::to_bytes(resp.into_body(), 1024).await.unwrap();
        let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(
            v,
            json!({ "error": format!("Invalid metadata content: {raw}"), "message": ADR44_MESSAGE })
        );
    }

    #[tokio::test]
    async fn a_missing_chain_is_the_upstream_400_and_a_bad_signature_a_401() {
        let resp = signed_fetch_gate(SignedFetchError::InsufficientLinks.into()).into_response();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
        let bytes = axum::body::to_bytes(resp.into_body(), 1024).await.unwrap();
        let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(
            v,
            json!({ "error": "Invalid Auth Chain", "message": ADR44_MESSAGE })
        );

        let resp = signed_fetch_gate(SignedFetchError::InvalidSignature("x".into()).into())
            .into_response();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
        let bytes = axum::body::to_bytes(resp.into_body(), 1024).await.unwrap();
        let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(
            v,
            json!({ "error": "Invalid signature: x", "message": ADR44_MESSAGE })
        );
    }

    #[test]
    fn a_long_metadata_echo_is_cut_where_upstream_cuts_it() {
        let raw = format!(r#"{{"signer":"{}"}}"#, "a".repeat(120));
        let (status, error) =
            signed_fetch_gate_parts(SignedFetchRejection::RefusedMetadata(raw.clone()));
        assert_eq!(status, StatusCode::BAD_REQUEST);
        let echoed = error.strip_prefix("Invalid metadata content: ").unwrap();
        assert_eq!(echoed.chars().count(), METADATA_ECHO_MAX + 1);
        assert!(echoed.starts_with(&raw[..METADATA_ECHO_MAX]));
        assert!(echoed.ends_with('\u{2026}'));
    }

    #[test]
    fn an_undetermined_authority_never_renders_as_the_call_sites_refusal() {
        let undetermined = AuthorityNotEstablished::UndeterminedStoreUnavailable {
            store: "community_members",
            reason_for_operators: "pool timed out".to_string(),
        };
        for call_site_refusal_status in [StatusCode::UNAUTHORIZED, StatusCode::FORBIDDEN] {
            let (status, message) =
                status_and_message_for_refusal(&undetermined, call_site_refusal_status, || {
                    unreachable!("the refusal message must not be built for an outage")
                });
            assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
            assert_eq!(message, "database error");
        }
        let refusal = AuthorityNotEstablished::RefusedLacksAuthority {
            detail: "0xactor holds the member tier".to_string(),
        };
        let (status, message) =
            status_and_message_for_refusal(&refusal, StatusCode::UNAUTHORIZED, || {
                "The user 0xactor doesn't have permission to ban 0xtarget".to_string()
            });
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(
            message,
            "The user 0xactor doesn't have permission to ban 0xtarget"
        );
    }
}
