//! The operator bearer gate every admin surface shares: `Authorization:
//! Bearer <token>` parsed once, compared in constant time, and failing closed
//! when the configured token is unset or empty. Each surface keeps only its
//! own wire mapping of a [`Refusal`] (401 or 403, its old message).

use axum::http::HeaderMap;
use axum::http::header::AUTHORIZATION;
use axum::response::{IntoResponse, Response};
use sha2::{Digest, Sha256};

/// Why the gate refused, so a surface can keep its old status per case.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refusal {
    /// No token configured (unset or empty): nobody is admitted.
    NotConfigured,
    /// The request carried no `Bearer` credential.
    NoBearer,
    /// The presented token is not the configured one.
    Mismatch,
}

impl Refusal {
    pub fn http_status(self) -> u16 {
        match self {
            Refusal::NotConfigured | Refusal::Mismatch => 403,
            Refusal::NoBearer => 401,
        }
    }
}

impl IntoResponse for Refusal {
    fn into_response(self) -> Response {
        (axum::http::StatusCode::FORBIDDEN, "Not authorized").into_response()
    }
}

/// The token after the case-sensitive `Bearer ` scheme, as every old service parsed it.
pub fn bearer_token(headers: &HeaderMap) -> Option<&str> {
    headers
        .get(AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.strip_prefix("Bearer "))
}

/// Equality without a length or content timing channel: both sides are
/// hashed to a fixed width and the digests folded with XOR.
pub fn timing_safe_eq(a: &[u8], b: &[u8]) -> bool {
    let da = Sha256::digest(a);
    let db = Sha256::digest(b);
    let mut diff = 0u8;
    for (x, y) in da.iter().zip(db.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

/// The gate over an already-extracted credential (a cookie, a query, a
/// second token): unset or empty `expected` refuses everything.
pub fn check_token(expected: Option<&str>, presented: Option<&str>) -> Result<(), Refusal> {
    let Some(expected) = expected.filter(|s| !s.is_empty()) else {
        return Err(Refusal::NotConfigured);
    };
    let Some(presented) = presented else {
        return Err(Refusal::NoBearer);
    };
    if timing_safe_eq(presented.as_bytes(), expected.as_bytes()) {
        Ok(())
    } else {
        Err(Refusal::Mismatch)
    }
}

/// The gate over the request's `Authorization` header.
pub fn check(expected: Option<&str>, headers: &HeaderMap) -> Result<(), Refusal> {
    check_token(expected, bearer_token(headers))
}

/// [`check`] as a predicate.
pub fn matches(expected: Option<&str>, headers: &HeaderMap) -> bool {
    check(expected, headers).is_ok()
}

/// [`check`] with the argument order the review asked for; a refusal renders
/// as a plain 403 unless the surface maps it.
pub fn require(headers: &HeaderMap, expected: Option<&str>) -> Result<(), Refusal> {
    check(expected, headers)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderValue;

    fn with_auth(value: Option<&str>) -> HeaderMap {
        let mut h = HeaderMap::new();
        if let Some(v) = value {
            h.insert(AUTHORIZATION, HeaderValue::from_str(v).unwrap());
        }
        h
    }

    #[test]
    fn bearer_token_parses_the_case_sensitive_scheme_only() {
        assert_eq!(
            bearer_token(&with_auth(Some("Bearer abc123"))),
            Some("abc123")
        );
        assert_eq!(bearer_token(&with_auth(Some("Bearer "))), Some(""));
        assert_eq!(bearer_token(&with_auth(Some("Basic abc123"))), None);
        assert_eq!(bearer_token(&with_auth(Some("bearer abc123"))), None);
        assert_eq!(bearer_token(&with_auth(Some("abc123"))), None);
        assert_eq!(bearer_token(&with_auth(None)), None);
    }

    #[test]
    fn timing_safe_eq_matches_and_mismatches() {
        assert!(timing_safe_eq(b"s3cr3t-token", b"s3cr3t-token"));
        assert!(!timing_safe_eq(b"s3cr3t-token", b"s3cr3t-toker"));
        assert!(!timing_safe_eq(b"secret", b"Secret"));
        assert!(!timing_safe_eq(b"short", b"longer-token"));
        assert!(!timing_safe_eq(b"", b"x"));
        assert!(timing_safe_eq(b"", b""));
        assert!(!timing_safe_eq(b"aaaaaa", b"baaaaa"));
        assert!(!timing_safe_eq(b"aaaaaa", b"aaaaab"));
    }

    #[test]
    fn an_unset_or_empty_token_admits_nobody() {
        assert_eq!(
            check(None, &with_auth(Some("Bearer anything"))),
            Err(Refusal::NotConfigured)
        );
        assert_eq!(check(None, &with_auth(None)), Err(Refusal::NotConfigured));
        assert_eq!(
            check(Some(""), &with_auth(Some("Bearer "))),
            Err(Refusal::NotConfigured),
            "empty token accepted an empty bearer: fail-open"
        );
        assert_eq!(
            check(Some(""), &with_auth(Some("Bearer anything"))),
            Err(Refusal::NotConfigured)
        );
        assert_eq!(
            check(Some(""), &with_auth(None)),
            Err(Refusal::NotConfigured)
        );
        assert_eq!(check_token(Some(""), Some("")), Err(Refusal::NotConfigured));
    }

    #[test]
    fn a_configured_token_needs_the_exact_bearer() {
        let expected = Some("s3cret");
        assert_eq!(check(expected, &with_auth(Some("Bearer s3cret"))), Ok(()));
        assert_eq!(
            check(expected, &with_auth(Some("Bearer wrong"))),
            Err(Refusal::Mismatch)
        );
        assert_eq!(
            check(expected, &with_auth(Some("Bearer "))),
            Err(Refusal::Mismatch)
        );
        assert_eq!(
            check(expected, &with_auth(Some("Bearer s3cret-longer"))),
            Err(Refusal::Mismatch)
        );
        assert_eq!(
            check(expected, &with_auth(Some("s3cret"))),
            Err(Refusal::NoBearer)
        );
        assert_eq!(
            check(expected, &with_auth(Some("Basic s3cret"))),
            Err(Refusal::NoBearer)
        );
        assert_eq!(check(expected, &with_auth(None)), Err(Refusal::NoBearer));
        assert!(matches(expected, &with_auth(Some("Bearer s3cret"))));
        assert!(!matches(expected, &with_auth(Some("Bearer wrong"))));
        assert_eq!(require(&with_auth(Some("Bearer s3cret")), expected), Ok(()));
    }

    #[test]
    fn check_token_serves_credentials_that_did_not_arrive_as_a_bearer() {
        assert_eq!(check_token(Some("tok"), Some("tok")), Ok(()));
        assert_eq!(
            check_token(Some("tok"), Some("nope")),
            Err(Refusal::Mismatch)
        );
        assert_eq!(check_token(Some("tok"), None), Err(Refusal::NoBearer));
        assert_eq!(
            check_token(None, Some("anything")),
            Err(Refusal::NotConfigured)
        );
    }

    #[tokio::test]
    async fn the_default_rendering_is_a_plain_403() {
        for refusal in [Refusal::NotConfigured, Refusal::NoBearer, Refusal::Mismatch] {
            let resp = refusal.into_response();
            assert_eq!(resp.status(), axum::http::StatusCode::FORBIDDEN);
            let bytes = axum::body::to_bytes(resp.into_body(), 64).await.unwrap();
            assert_eq!(&bytes[..], b"Not authorized");
        }
        assert_eq!(Refusal::NoBearer.http_status(), 401);
        assert_eq!(Refusal::Mismatch.http_status(), 403);
    }
}
