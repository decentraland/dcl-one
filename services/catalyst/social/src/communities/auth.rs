//! Who is calling: the ADR-44 signed fetch verified through
//! [`crate::shared::signed_fetch`] (the shared verifier; EIP-1654 links go
//! through the configured RPC validator), the scene-signer gate answered
//! before any crypto, and the operator bearer of the admin routes.
//!
//! A request reaches these routes either at the paths the old social service
//! served (`/v1/...`) or behind the gateway prefix (`/social-api/v1/...`,
//! `main.rs` nests the same router there). A client signs the path it
//! requested, which is what the shared verifier checks
//! (`signed_fetch::candidate_paths`, the ONE path rule: the request's own
//! path, and the `x-original-path` a proxy sets when it ends with the
//! route). This surface verifies the 6.x payload only ([`PayloadPolicy::V6Only`]),
//! as the old social-service did (`CANONICAL_METADATA_KEYS = &[]`).

use axum::http::HeaderMap;

use crate::Address;
use crate::auth::eip1654::Eip1654Validator;
use crate::shared::signed_fetch::{
    self, AUTH_METADATA_HEADER, PayloadPolicy, SignedFetchError, Signer,
};

use super::http::SignedFetchRejection;

pub use crate::shared::signed_fetch::SignedPath;

pub use crate::shared::signed_fetch::is_refused_signer;

/// Read straight off the header so the "not for scenes" gate can answer
/// before any crypto: a refused request is a 400 that costs no verification.
fn refused_metadata(headers: &HeaderMap) -> Option<String> {
    let raw = headers
        .get(AUTH_METADATA_HEADER)
        .and_then(|v| v.to_str().ok())?;
    let metadata: serde_json::Value = serde_json::from_str(raw).ok()?;
    is_refused_signer(&metadata).then(|| metadata.to_string())
}

async fn verify(
    headers: &HeaderMap,
    method: &str,
    path: &SignedPath,
    validator: Option<&dyn Eip1654Validator>,
) -> Result<Signer, SignedFetchError> {
    signed_fetch::require_signer_at(headers, method, path, PayloadPolicy::V6Only, validator).await
}

/// The verified caller of a route that requires one.
pub async fn require_signer(
    headers: &HeaderMap,
    method: &str,
    path: &SignedPath,
    validator: Option<&dyn Eip1654Validator>,
) -> Result<Address, SignedFetchRejection> {
    if let Some(metadata) = refused_metadata(headers) {
        tracing::warn!(%method, path = %path.request, "signed-fetch rejected: refused signer");
        return Err(SignedFetchRejection::RefusedMetadata(metadata));
    }
    match verify(headers, method, path, validator).await {
        Ok(signer) => Ok(Address::from_signer(&signer)),
        Err(e) => {
            tracing::warn!(error = %e, %method, path = %path.request, "signed-fetch rejected");
            Err(SignedFetchRejection::Chain(e))
        }
    }
}

/// Optional signer for the read paths that widen visibility when
/// authenticated. A scene-signed chain is treated as no signer rather than a
/// valid identity, otherwise a scene could ride an anonymous caller's chain
/// into the member-only view. A request without a chain is anonymous at no
/// cost.
pub async fn try_extract_signer(
    headers: &HeaderMap,
    method: &str,
    path: &SignedPath,
    validator: Option<&dyn Eip1654Validator>,
) -> Option<Address> {
    headers.get(format!("{}0", signed_fetch::AUTH_CHAIN_HEADER_PREFIX))?;
    if refused_metadata(headers).is_some() {
        return None;
    }
    match verify(headers, method, path, validator).await {
        Ok(signer) => Some(Address::from_signer(&signer)),
        Err(e) => {
            tracing::debug!(error = %e, "optional signed-fetch verification failed");
            None
        }
    }
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
    use crate::shared::signed_fetch::SCENE_SIGNER;
    use axum::http::HeaderValue;
    use serde_json::json;

    #[test]
    fn plain_and_non_canonical_scene_signers_are_refused() {
        assert!(is_refused_signer(&json!({ "signer": SCENE_SIGNER })));
        assert!(is_refused_signer(
            &json!({ "signer": "Decentraland-Kernel-Scene" })
        ));
        assert!(is_refused_signer(
            &json!({ "signer": "  decentraland-kernel-scene\t" })
        ));
        assert!(is_refused_signer(&json!({ "signer": "DCL:Explorer" })));
        let padded = format!("\u{FEFF}{SCENE_SIGNER}");
        assert!(is_refused_signer(&json!({ "signer": padded })));
    }

    #[test]
    fn a_present_non_string_signer_is_refused() {
        assert!(is_refused_signer(&json!({ "signer": 42 })));
        assert!(is_refused_signer(&json!({ "signer": true })));
        assert!(is_refused_signer(&json!({ "signer": null })));
        assert!(is_refused_signer(&json!({ "signer": [SCENE_SIGNER] })));
        assert!(is_refused_signer(
            &json!({ "signer": { "value": SCENE_SIGNER } })
        ));
    }

    #[test]
    fn canonical_other_and_absent_signers_pass() {
        assert!(!is_refused_signer(&json!({ "signer": "dcl:explorer" })));
        assert!(!is_refused_signer(&json!({ "signer": "" })));
        assert!(!is_refused_signer(&json!({})));
        assert!(!is_refused_signer(&serde_json::Value::Null));
        assert!(!is_refused_signer(&json!({ "sceneId": "BafkreiAbcDef" })));
    }

    #[test]
    fn a_re_spelled_signer_key_is_refused_instead_of_read_as_absent() {
        assert!(is_refused_signer(&json!({ "Signer": SCENE_SIGNER })));
        assert!(is_refused_signer(&json!({ "SIGNER": SCENE_SIGNER })));
        assert!(is_refused_signer(
            &json!({ "signer": "dcl:explorer", "Signer": SCENE_SIGNER })
        ));
    }

    fn headers_with_metadata(raw: &str) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert(AUTH_METADATA_HEADER, HeaderValue::from_str(raw).unwrap());
        headers
    }

    #[tokio::test]
    async fn a_refused_signer_answers_before_verification() {
        let headers = headers_with_metadata(r#"{"signer":"decentraland-kernel-scene"}"#);
        let err = require_signer(&headers, "get", &SignedPath::local("/v1/mutes"), None)
            .await
            .expect_err("a scene-signed request must be refused");
        assert!(
            matches!(&err, SignedFetchRejection::RefusedMetadata(m) if m == r#"{"signer":"decentraland-kernel-scene"}"#),
            "{err:?}"
        );
        let headers = headers_with_metadata(r#"{"signer":"Decentraland-Kernel-Scene"}"#);
        let err = require_signer(&headers, "get", &SignedPath::local("/v1/mutes"), None)
            .await
            .unwrap_err();
        assert!(
            matches!(err, SignedFetchRejection::RefusedMetadata(_)),
            "{err:?}"
        );
    }

    #[tokio::test]
    async fn ordinary_metadata_falls_through_to_verification() {
        for raw in [r#"{}"#, r#"{"signer":"dcl:explorer"}"#, "not json"] {
            let headers = headers_with_metadata(raw);
            let err = require_signer(&headers, "get", &SignedPath::local("/v1/mutes"), None)
                .await
                .unwrap_err();
            assert!(
                matches!(err, SignedFetchRejection::Chain(_)),
                "{raw}: {err:?}"
            );
        }
    }

    #[tokio::test]
    async fn the_optional_path_drops_a_refused_signer_and_a_missing_chain() {
        let headers = headers_with_metadata(r#"{"signer":42}"#);
        assert!(
            try_extract_signer(&headers, "get", &SignedPath::local("/v1/communities"), None)
                .await
                .is_none()
        );
        assert!(
            try_extract_signer(
                &HeaderMap::new(),
                "get",
                &SignedPath::local("/v1/communities"),
                None
            )
            .await
            .is_none()
        );
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
        assert!(
            matches!(
                err,
                SignedFetchRejection::Chain(SignedFetchError::InvalidSignature(_))
            ),
            "{err:?}"
        );
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
        assert!(
            matches!(
                err,
                SignedFetchRejection::Chain(SignedFetchError::InvalidSignature(_))
            ),
            "{err:?}"
        );
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

    /// The old social-service verified the 6.x payload only; the folded
    /// legacy shape is refused here although the explore reads accept it.
    #[tokio::test]
    async fn a_legacy_only_chain_is_refused_on_this_surface() {
        use crate::shared::signed_fetch::test_support::signed_headers;
        let metadata = r#"{"signer":"dcl:explorer","intent":"Probe-Legacy"}"#;
        let headers = signed_headers("get", "/v1/mutes", metadata, false).await;
        let path = SignedPath::local("/v1/mutes");
        let err = require_signer(&headers, "get", &path, None)
            .await
            .unwrap_err();
        assert!(
            matches!(
                err,
                SignedFetchRejection::Chain(SignedFetchError::InvalidSignature(_))
            ),
            "{err:?}"
        );
        assert!(
            try_extract_signer(&headers, "get", &path, None)
                .await
                .is_none()
        );
        let headers = signed_headers("get", "/v1/mutes", metadata, true).await;
        require_signer(&headers, "get", &path, None)
            .await
            .expect("the 6.x payload verifies");
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
