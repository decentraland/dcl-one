//! The moderator's bearer (`CATALYRST_CAMERA_REEL_ADMIN_TOKEN`): unset
//! refuses the admin routes outright (403), a wrong or missing bearer is 401.
//! The parse and compare are the shared [`crate::auth::admin_bearer`] gate.

use axum::http::HeaderMap;

use crate::auth::admin_bearer::{self, Refusal};

use super::http::ApiError;

pub(crate) fn check_admin(expected: Option<&str>, headers: &HeaderMap) -> Result<(), ApiError> {
    match admin_bearer::check(expected, headers) {
        Ok(()) => Ok(()),
        Err(Refusal::NotConfigured) => Err(ApiError::Forbidden(
            "admin token not configured".to_string(),
        )),
        Err(Refusal::NoBearer | Refusal::Mismatch) => Err(ApiError::Unauthorized),
    }
}

#[cfg(test)]
mod tests {
    use super::check_admin;
    use crate::http::ApiError;
    use axum::http::{HeaderMap, HeaderValue, header::AUTHORIZATION};

    fn headers_with_bearer(token: &str) -> HeaderMap {
        let mut h = HeaderMap::new();
        h.insert(
            AUTHORIZATION,
            HeaderValue::from_str(&format!("Bearer {token}")).unwrap(),
        );
        h
    }

    #[test]
    fn check_admin_fails_closed_when_token_unset() {
        let err = check_admin(None, &headers_with_bearer("anything")).unwrap_err();
        assert!(matches!(err, ApiError::Forbidden(_)));
        assert!(matches!(
            check_admin(Some(""), &headers_with_bearer("")).unwrap_err(),
            ApiError::Forbidden(_)
        ));
        assert!(matches!(
            check_admin(Some(""), &HeaderMap::new()).unwrap_err(),
            ApiError::Forbidden(_)
        ));
    }

    #[test]
    fn check_admin_rejects_missing_wrong_and_empty_bearers_with_401() {
        assert!(matches!(
            check_admin(Some("expected"), &HeaderMap::new()).unwrap_err(),
            ApiError::Unauthorized
        ));
        assert!(matches!(
            check_admin(Some("expected"), &headers_with_bearer("wrong")).unwrap_err(),
            ApiError::Unauthorized
        ));
        assert!(matches!(
            check_admin(Some("expected"), &headers_with_bearer("")).unwrap_err(),
            ApiError::Unauthorized
        ));
    }

    #[test]
    fn check_admin_accepts_matching_token() {
        assert!(check_admin(Some("expected"), &headers_with_bearer("expected")).is_ok());
    }
}
