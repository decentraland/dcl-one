//! Who is calling a builder route: the ADR-44 signed fetch verified through
//! [`crate::shared::signed_fetch`] (the shared verifier the explore and
//! social routes use; EIP-1654 links go through the configured RPC
//! validator), and the admin gate of the curation routes (the operator
//! bearer, compared in constant time and failing closed when unset, or a
//! signed-fetch request from a `BUILDER_ADMIN_ADDRESSES` wallet).
//!
//! The old crate accepted the pre-6.0.0 folded payload behind the
//! canonical-`signer` guard (`CANONICAL_METADATA_KEYS = &["signer"]`) and
//! refused a scene-signed chain before any signature check; that is
//! [`PayloadPolicy::V6ThenLegacy`] here, scene gate included. A request
//! reaches these routes either at the paths the old service served
//! (`/v1/...`) or behind the gateway prefix (`/builder-api/v1/...`); a
//! client signs the path it requested, which is what the shared verifier
//! checks (`signed_fetch::candidate_paths`, the ONE path rule). Deviation: the
//! old crate tolerated a 30-minute clock window on the required path; the
//! shared verifier's window is 5 minutes on every surface.

use axum::http::{HeaderMap, Uri};

use crate::shared::signed_fetch::{self, PayloadPolicy, SignedFetchError, Signer};
use dcl_auth::admin_bearer::matches as admin_bearer_matches;
use dcl_auth::eip1654::Eip1654Validator;

use super::http::ApiError;

pub use crate::shared::signed_fetch::SignedPath;

async fn verify(
    headers: &HeaderMap,
    method: &str,
    path: &SignedPath,
    validator: Option<&dyn Eip1654Validator>,
) -> Result<Signer, SignedFetchError> {
    signed_fetch::require_signer_at(
        headers,
        method,
        path,
        PayloadPolicy::V6ThenLegacy,
        validator,
    )
    .await
}

/// The verified, lowercased signer of a route that requires one; the
/// refusal carries the verifier's status (400 malformed, 401 unverified,
/// 503 validator unavailable) in the builder envelope.
pub async fn require_signer(
    headers: &HeaderMap,
    method: &str,
    path: &SignedPath,
    validator: Option<&dyn Eip1654Validator>,
) -> Result<Signer, ApiError> {
    verify(headers, method, path, validator)
        .await
        .map_err(|error| {
            let (status, message) = error.http_status_and_message();
            ApiError::http(status, message)
        })
}

/// A refused or absent chain reads as anonymous.
pub async fn try_extract_signer(
    headers: &HeaderMap,
    method: &str,
    path: &SignedPath,
    validator: Option<&dyn Eip1654Validator>,
) -> Option<Signer> {
    headers.get(format!("{}0", signed_fetch::AUTH_CHAIN_HEADER_PREFIX))?;
    verify(headers, method, path, validator).await.ok()
}

/// The request path a client signed (`OriginalUri`, the path before any
/// nest prefix was stripped) and the route-local one (`Uri`).
pub fn signed_path(original: &Uri, local: &Uri) -> SignedPath {
    SignedPath::from_uris(original, local)
}

/// The curation gate: the configured bearer (timing-safe, an unset or empty
/// token matches nothing) or a signed-fetch request from an admin address.
pub async fn authorize_admin(
    admin_token: Option<&str>,
    admin_addresses: &[String],
    validator: Option<&dyn Eip1654Validator>,
    headers: &HeaderMap,
    method: &str,
    path: &SignedPath,
) -> Result<(), ApiError> {
    if admin_bearer_matches(admin_token, headers) {
        return Ok(());
    }
    if let Ok(signer) = verify(headers, method, path, validator).await
        && admin_addresses.iter().any(|a| a == signer.as_str())
    {
        return Ok(());
    }
    Err(ApiError::forbidden(
        "Not authorized: curation requires the admin token or a committee-address signed request",
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shared::signed_fetch::test_support::{address, signed_headers};

    const METHOD: &str = "get";
    const PATH: &str = "/v1/collections/1";

    #[tokio::test]
    async fn an_ordinary_signer_still_verifies_under_either_payload_shape() {
        let metadata = r#"{"signer":"dcl:explorer"}"#;
        for v6 in [true, false] {
            let headers = signed_headers(METHOD, PATH, metadata, v6).await;
            let signer = require_signer(&headers, METHOD, &SignedPath::local(PATH), None)
                .await
                .unwrap_or_else(|e| panic!("v6={v6}: {e}"));
            assert_eq!(signer.as_str(), address().to_lowercase());
            assert_eq!(
                try_extract_signer(&headers, METHOD, &SignedPath::local(PATH), None)
                    .await
                    .map(|s| s.into_string()),
                Some(address().to_lowercase())
            );
        }
    }

    /// Upstream refuses this before any signature is checked; ours served it as the
    /// visiting user until the gate was wired in.
    #[tokio::test]
    async fn a_scene_signed_request_is_refused() {
        for metadata in [
            r#"{"signer":"decentraland-kernel-scene"}"#,
            r#"{"Signer":"decentraland-kernel-scene"}"#,
        ] {
            for v6 in [true, false] {
                let headers = signed_headers(METHOD, PATH, metadata, v6).await;
                let err = require_signer(&headers, METHOD, &SignedPath::local(PATH), None)
                    .await
                    .expect_err("a scene-signed request must not be served");
                assert_eq!(err.status(), 400, "v6={v6} {metadata}: {err}");
                assert!(
                    try_extract_signer(&headers, METHOD, &SignedPath::local(PATH), None)
                        .await
                        .is_none(),
                    "v6={v6} {metadata}: the optional path must read a refused request as anonymous"
                );
            }
        }
    }

    #[tokio::test]
    async fn a_signature_over_the_gateway_path_verifies_behind_the_prefix() {
        let path = SignedPath::new("/builder-api/v1/collections", "/v1/collections");
        let headers = signed_headers("get", "/builder-api/v1/collections", "{}", true).await;
        assert!(require_signer(&headers, "get", &path, None).await.is_ok());
        let mut headers = signed_headers("get", "/v1/collections", "{}", true).await;
        assert_eq!(
            require_signer(&headers, "get", &path, None)
                .await
                .unwrap_err()
                .status(),
            401,
            "the route-local path alone is not what the client asked for"
        );
        headers.insert(
            crate::shared::signed_fetch::ORIGINAL_PATH_HEADER,
            "/v1/collections".parse().unwrap(),
        );
        assert!(
            require_signer(&headers, "get", &path, None).await.is_ok(),
            "the proxy's x-original-path says it is"
        );
        let headers = signed_headers("get", "/v1/items", "{}", true).await;
        assert_eq!(
            require_signer(&headers, "get", &path, None)
                .await
                .unwrap_err()
                .status(),
            401
        );
        assert!(
            try_extract_signer(&HeaderMap::new(), "get", &path, None)
                .await
                .is_none()
        );
    }

    fn headers_with(auth: Option<&str>) -> HeaderMap {
        let mut h = HeaderMap::new();
        if let Some(a) = auth {
            h.insert("authorization", a.parse().unwrap());
        }
        h
    }

    const NO_ADMINS: &[String] = &[];

    fn local() -> SignedPath {
        SignedPath::local("/x")
    }

    #[tokio::test]
    async fn authorize_admin_accepts_the_configured_bearer() {
        let h = headers_with(Some("Bearer the-real-token"));
        assert!(
            authorize_admin(Some("the-real-token"), NO_ADMINS, None, &h, "get", &local())
                .await
                .is_ok()
        );
    }

    #[tokio::test]
    async fn authorize_admin_rejects_a_wrong_bearer() {
        let h = headers_with(Some("Bearer wrong"));
        assert!(
            authorize_admin(Some("the-real-token"), NO_ADMINS, None, &h, "get", &local())
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn authorize_admin_rejects_when_no_credentials_present() {
        let h = headers_with(None);
        assert!(
            authorize_admin(Some("the-real-token"), NO_ADMINS, None, &h, "get", &local())
                .await
                .is_err()
        );
        assert!(
            authorize_admin(None, NO_ADMINS, None, &h, "get", &local())
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn authorize_admin_rejects_a_bearer_scheme_that_is_not_bearer() {
        let h = headers_with(Some("Basic the-real-token"));
        assert!(
            authorize_admin(Some("the-real-token"), NO_ADMINS, None, &h, "get", &local())
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn authorize_admin_never_authorizes_on_an_empty_token() {
        let empty_bearer = headers_with(Some("Bearer "));
        assert!(
            authorize_admin(Some(""), NO_ADMINS, None, &empty_bearer, "get", &local())
                .await
                .is_err()
        );
        assert!(
            authorize_admin(
                Some(""),
                NO_ADMINS,
                None,
                &headers_with(None),
                "get",
                &local()
            )
            .await
            .is_err()
        );
        assert!(
            authorize_admin(
                None,
                NO_ADMINS,
                None,
                &headers_with(Some("Bearer anything")),
                "get",
                &local()
            )
            .await
            .is_err()
        );
        assert!(
            authorize_admin(
                Some(""),
                NO_ADMINS,
                None,
                &headers_with(Some("Bearer anything")),
                "get",
                &local()
            )
            .await
            .is_err()
        );
        assert!(
            authorize_admin(
                Some("the-real-token"),
                NO_ADMINS,
                None,
                &empty_bearer,
                "get",
                &local()
            )
            .await
            .is_err()
        );
    }

    #[tokio::test]
    async fn authorize_admin_forbidden_error_is_403() {
        use axum::response::IntoResponse;
        let err = authorize_admin(
            Some("real"),
            NO_ADMINS,
            None,
            &headers_with(None),
            "get",
            &local(),
        )
        .await
        .unwrap_err();
        assert_eq!(
            err.into_response().status(),
            axum::http::StatusCode::FORBIDDEN
        );
    }

    #[tokio::test]
    async fn authorize_admin_accepts_an_admin_address_signed_request_and_refuses_others() {
        let path = SignedPath::local("/v1/collections/curation");
        let headers = signed_headers("get", "/v1/collections/curation", "{}", true).await;
        let admins = vec![address().to_lowercase()];
        assert!(
            authorize_admin(None, &admins, None, &headers, "get", &path)
                .await
                .is_ok()
        );
        assert!(
            authorize_admin(Some(""), &admins, None, &headers, "get", &path)
                .await
                .is_ok(),
            "an empty token does not weaken the signed-fetch branch"
        );
        let others = vec!["0xdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef".to_string()];
        assert!(
            authorize_admin(None, &others, None, &headers, "get", &path)
                .await
                .is_err()
        );
        assert!(
            authorize_admin(
                None,
                &admins,
                None,
                &headers,
                "get",
                &SignedPath::local("/v1/collections/other")
            )
            .await
            .is_err(),
            "path-bound"
        );
    }
}
