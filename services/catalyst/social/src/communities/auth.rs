//! Who is calling: the signed request verified by the social verifier
//! ([`crate::request_auth`]: 60 s window, current payload only, scene signer
//! refused, EIP-1654 links through the configured validator), and the operator
//! bearer of the admin routes.
//!
//! A request reaches these routes either at the paths the old social service
//! served (`/v1/...`) or behind the gateway prefix (`/social-api/v1/...`,
//! `main.rs` nests the same router there). A client signs the path it
//! requested; the gateway's `x-original-path` is accepted when it ends with
//! the route (`signed_fetch::candidate_paths`).

use std::sync::Arc;

use axum::http::{HeaderMap, Method};

use crate::Address;
use crate::auth::eip1654::Eip1654Validator;
use crate::auth::request::{RequestAuthError, RequestParts, verify_request};

pub use crate::shared::signed_fetch::SignedPath;

/// A fact about one gateway request, created only after its ADR-44 check.
/// It grants no community permission and says nothing about its body.
#[derive(Clone)]
struct VerifiedRequest {
    address: Address,
    method: Method,
    path: SignedPath,
    headers: HeaderMap,
}

impl VerifiedRequest {
    fn matches(&self, headers: &HeaderMap, method: &Method, path: &SignedPath) -> bool {
        self.method == *method && self.path == *path && self.headers == *headers
    }
}

/// Extract the private, request-scoped fact, when gateway ingress supplied it.
/// Native handlers have no such fact and retain their existing verifier order.
#[derive(Default)]
pub struct Caller(Option<VerifiedRequest>);

impl<S: Send + Sync> axum::extract::FromRequestParts<S> for Caller {
    type Rejection = std::convert::Infallible;

    async fn from_request_parts(
        parts: &mut axum::http::request::Parts,
        _: &S,
    ) -> Result<Self, Self::Rejection> {
        Ok(Self::from_extensions(&parts.extensions))
    }
}

impl Caller {
    pub(super) fn from_extensions(extensions: &axum::http::Extensions) -> Self {
        Self(extensions.get::<VerifiedRequest>().cloned())
    }

    pub(super) async fn require_signer(
        &self,
        state: &super::State,
        headers: &HeaderMap,
        method: &str,
        path: &SignedPath,
    ) -> Result<Address, super::http::CommError> {
        // Handler method literals use the signed-fetch lowercase spelling;
        // compare their corresponding HTTP Method with the ingress snapshot.
        if let Ok(method) = Method::from_bytes(method.to_ascii_uppercase().as_bytes())
            && let Some(fact) = &self.0
            && fact.matches(headers, &method, path)
        {
            return Ok(fact.address.clone());
        }
        state.require_signer(headers, method, path).await
    }
}

/// The Social gateway authenticates registered community writes before mirror
/// SQL and body classification. The private body-limit extension comes from
/// the server's gateway boundary, never a client-supplied marker header.
pub async fn gateway_preauth(
    axum::extract::State(state): axum::extract::State<super::SharedState>,
    mut request: axum::extract::Request,
    next: axum::middleware::Next,
) -> axum::response::Response {
    use axum::response::IntoResponse;
    if !super::http::is_social_api(&request)
        || !super::fed::requires_gateway_signer(request.method(), request.uri().path())
    {
        return next.run(request).await;
    }
    let original = request
        .extensions()
        .get::<axum::extract::OriginalUri>()
        .map(|original| original.0.path())
        .unwrap_or_else(|| request.uri().path());
    let path = SignedPath::new(original.to_owned(), request.uri().path().to_owned());
    let address = match state
        .require_signer(request.headers(), request.method().as_str(), &path)
        .await
    {
        Ok(address) => address,
        Err(error) => return error.into_response(),
    };
    let fact = VerifiedRequest {
        address,
        method: request.method().clone(),
        path,
        headers: request.headers().clone(),
    };
    request.extensions_mut().insert(fact);
    next.run(request).await
}

/// The verified caller of a route that requires one.
pub async fn require_signer(
    headers: &HeaderMap,
    method: &str,
    path: &SignedPath,
    validator: Option<&Arc<dyn Eip1654Validator>>,
) -> Result<Address, RequestAuthError> {
    let parts = RequestParts::from_headers(method, path.clone(), headers);
    match crate::request_auth::require(&parts, &crate::request_auth::options(validator)).await {
        Ok(verified) => Ok(Address::from_verified(&verified)),
        Err(e) => {
            tracing::warn!(error = %e, %method, path = %path.request, "signed-fetch rejected");
            Err(e)
        }
    }
}

/// Optional signer for the read paths that widen visibility when
/// authenticated: any refusal, a scene-signed chain included, reads as
/// anonymous, never as an authenticated caller.
pub async fn try_extract_signer(
    headers: &HeaderMap,
    method: &str,
    path: &SignedPath,
    validator: Option<&Arc<dyn Eip1654Validator>>,
) -> Option<Address> {
    let parts = RequestParts::from_headers(method, path.clone(), headers);
    let options =
        crate::request_auth::build(crate::request_auth::builder(validator).optional(true));
    verify_request(&parts, &options)
        .await
        .ok()
        .flatten()
        .map(|verified| Address::from_verified(&verified))
}

pub use crate::auth::admin_bearer::{bearer_token, timing_safe_eq};

/// The admin bearer matches the configured token; an unset or empty token
/// matches nothing (fail closed): the shared gate.
pub fn admin_bearer_matches(expected: Option<&str>, headers: &HeaderMap) -> bool {
    crate::auth::admin_bearer::matches(expected, headers)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderValue;

    #[test]
    fn verified_request_requires_the_exact_method_both_paths_and_all_headers() {
        let mut headers = HeaderMap::new();
        for (key, value) in [
            ("x-identity-auth-chain-0", "signer"),
            ("x-identity-auth-chain-1", "signed-entity"),
            ("x-identity-timestamp", "123"),
            ("x-identity-metadata", "{}"),
            ("content-type", "application/json"),
        ] {
            headers.insert(key, HeaderValue::from_static(value));
        }
        let path = SignedPath::new(
            "/social-api/v1/communities".to_owned(),
            "/v1/communities".to_owned(),
        );
        let fact = VerifiedRequest {
            address: Address::parse("0x1111111111111111111111111111111111111111").unwrap(),
            method: Method::POST,
            path: path.clone(),
            headers: headers.clone(),
        };
        assert!(fact.matches(&headers, &Method::POST, &path));
        assert!(!fact.matches(&headers, &Method::PUT, &path));
        for changed in [
            SignedPath::new("/v1/communities".to_owned(), path.local.clone()),
            SignedPath::new(path.request.clone(), "/v2/communities".to_owned()),
        ] {
            assert!(!fact.matches(&headers, &Method::POST, &changed));
        }
        for key in [
            "x-identity-auth-chain-0",
            "x-identity-auth-chain-1",
            "x-identity-timestamp",
            "x-identity-metadata",
            "content-type",
        ] {
            let mut changed = headers.clone();
            changed.insert(key, HeaderValue::from_static("changed"));
            assert!(!fact.matches(&changed, &Method::POST, &path), "{key}");
            let mut duplicate = headers.clone();
            duplicate.append(key, headers[key].clone());
            assert!(
                !fact.matches(&duplicate, &Method::POST, &path),
                "duplicate {key}"
            );
        }
    }

    const SCENE: &str = r#"{"signer":"decentraland-kernel-scene"}"#;

    #[tokio::test]
    async fn a_scene_signed_request_is_refused_as_malformed() {
        use crate::shared::signed_fetch::test_support::signed_headers;
        let path = SignedPath::local("/v1/mutes");
        for metadata in [
            SCENE,
            r#"{"signer":"Decentraland-Kernel-Scene"}"#,
            r#"{"Signer":"decentraland-kernel-scene"}"#,
            r#"{"signer":"DCL:Explorer"}"#,
            r#"{"signer":42}"#,
            r#"{"signer":null}"#,
        ] {
            let headers = signed_headers("get", "/v1/mutes", metadata, true).await;
            let err = require_signer(&headers, "get", &path, None)
                .await
                .unwrap_err();
            assert_eq!(err.status(), 400, "{metadata}: {err:?}");
            assert!(
                err.to_string().starts_with("Invalid metadata content"),
                "{metadata}: {err}"
            );
            assert!(
                try_extract_signer(&headers, "get", &path, None)
                    .await
                    .is_none()
            );
        }
        for metadata in ["{}", r#"{"signer":"dcl:explorer"}"#, r#"{"signer":""}"#] {
            let headers = signed_headers("get", "/v1/mutes", metadata, true).await;
            require_signer(&headers, "get", &path, None)
                .await
                .unwrap_or_else(|e| panic!("{metadata}: {e:?}"));
        }
    }

    #[tokio::test]
    async fn the_optional_path_reads_a_missing_chain_as_anonymous() {
        let path = SignedPath::local("/v1/communities");
        assert!(
            try_extract_signer(&HeaderMap::new(), "get", &path, None)
                .await
                .is_none()
        );
        let err = require_signer(&HeaderMap::new(), "get", &path, None)
            .await
            .unwrap_err();
        assert_eq!(err.status(), 400);
    }

    #[tokio::test]
    async fn a_signature_over_the_gateway_path_verifies_behind_the_prefix() {
        use crate::shared::signed_fetch::test_support::{address, signed_headers};
        let headers = signed_headers("get", "/social-api/v1/mutes", "{}", true).await;
        let path = SignedPath::new("/social-api/v1/mutes", "/v1/mutes");
        let signer = require_signer(&headers, "get", &path, None)
            .await
            .expect("the request path verifies");
        assert_eq!(signer.as_str(), address().to_lowercase());
        let mut headers = signed_headers("get", "/v1/mutes", "{}", true).await;
        assert!(
            require_signer(&headers, "get", &path, None).await.is_err(),
            "the route-local path alone is not what the client asked for"
        );
        headers.insert(
            crate::shared::signed_fetch::ORIGINAL_PATH_HEADER,
            "/v1/mutes".parse().unwrap(),
        );
        let signer = require_signer(&headers, "get", &path, None)
            .await
            .expect("the local path verifies behind the proxy's x-original-path");
        assert_eq!(signer.as_str(), address().to_lowercase());
        let headers = signed_headers("get", "/v1/forged/not-a-route", "{}", true).await;
        let err = require_signer(&headers, "get", &path, None)
            .await
            .unwrap_err();
        assert_eq!(err.status(), 401, "{err:?}");
    }

    /// A same-method chain captured for one route, replayed on another with
    /// the captured path as `x-original-path`, is refused: the header only
    /// widens the check to a prefix in front of this route.
    #[tokio::test]
    async fn a_forged_x_original_path_cannot_carry_a_chain_from_another_route() {
        use crate::shared::signed_fetch::{ORIGINAL_PATH_HEADER, test_support::signed_headers};
        let mut headers = signed_headers("post", "/v1/mutes", "{}", true).await;
        headers.insert(ORIGINAL_PATH_HEADER, HeaderValue::from_static("/v1/mutes"));
        let path = SignedPath::new(
            "/social-api/v1/communities/x/posts",
            "/v1/communities/x/posts",
        );
        let err = require_signer(&headers, "post", &path, None)
            .await
            .expect_err("a chain over /v1/mutes must not create a post");
        assert_eq!(err.status(), 401, "{err:?}");
        assert!(
            try_extract_signer(&headers, "post", &path, None)
                .await
                .is_none()
        );
        let headers = signed_headers("post", "/gateway/v1/communities/x/posts", "{}", true).await;
        let mut forwarded = headers.clone();
        forwarded.insert(
            ORIGINAL_PATH_HEADER,
            HeaderValue::from_static("/gateway/v1/communities/x/posts"),
        );
        require_signer(&forwarded, "post", &path, None)
            .await
            .expect("a stripped prefix in front of the route verifies");
    }

    /// The current payload only: a chain signed over the folded pre-6.0.0
    /// payload is refused when the metadata does not fold to itself.
    #[tokio::test]
    async fn a_legacy_only_chain_is_refused_on_this_surface() {
        use crate::shared::signed_fetch::test_support::signed_headers;
        let metadata = r#"{"signer":"dcl:explorer","intent":"Probe-Legacy"}"#;
        let headers = signed_headers("get", "/v1/mutes", metadata, false).await;
        let path = SignedPath::local("/v1/mutes");
        let err = require_signer(&headers, "get", &path, None)
            .await
            .unwrap_err();
        assert_eq!(err.status(), 401, "{err:?}");
        assert!(
            try_extract_signer(&headers, "get", &path, None)
                .await
                .is_none()
        );
        let headers = signed_headers("get", "/v1/mutes", metadata, true).await;
        require_signer(&headers, "get", &path, None)
            .await
            .expect("the current payload verifies");
    }

    /// The 60 s window: a minute and a bit old is refused with 401.
    #[tokio::test]
    async fn a_signature_older_than_sixty_seconds_is_expired() {
        use crate::shared::signed_fetch::test_support::signed_headers_at;
        let path = SignedPath::local("/v1/mutes");
        let now = chrono::Utc::now().timestamp_millis();
        let fresh =
            signed_headers_at("get", "/v1/mutes", "{}", true, &(now - 50_000).to_string()).await;
        require_signer(&fresh, "get", &path, None).await.unwrap();
        let stale =
            signed_headers_at("get", "/v1/mutes", "{}", true, &(now - 61_000).to_string()).await;
        let err = require_signer(&stale, "get", &path, None)
            .await
            .unwrap_err();
        assert_eq!(err.status(), 401, "{err:?}");
        assert!(err.to_string().starts_with("Expired signature"), "{err}");
    }

    fn bearer(tok: Option<&str>) -> HeaderMap {
        let mut h = HeaderMap::new();
        if let Some(t) = tok {
            h.insert("authorization", format!("Bearer {t}").parse().unwrap());
        }
        h
    }

    #[test]
    fn admin_bearer_fails_closed() {
        assert!(!admin_bearer_matches(None, &bearer(Some("anything"))));
        assert!(!admin_bearer_matches(None, &bearer(None)));
        assert!(
            !admin_bearer_matches(Some(""), &bearer(Some(""))),
            "empty configured admin_token accepted an empty Bearer -- fail-open bypass"
        );
        assert!(admin_bearer_matches(
            Some("secret"),
            &bearer(Some("secret"))
        ));
        assert!(!admin_bearer_matches(
            Some("secret"),
            &bearer(Some("wrong"))
        ));
        assert!(!admin_bearer_matches(Some("secret"), &bearer(Some(""))));
        assert!(!admin_bearer_matches(Some("secret"), &bearer(None)));
    }
}
