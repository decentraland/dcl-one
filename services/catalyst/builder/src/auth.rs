//! Who is calling a builder route: builder-server's `withAuthentication`,
//! reproduced as [`VerificationProfile::builder`] over the shared verifier
//! (`shared::signed_fetch`; EIP-1654 links go through the configured RPC
//! validator).
//!
//! It accepts only the timestamped payload
//! (`method:path:timestamp:metadata`) with a timestamp at most 60 seconds
//! behind or ahead of the node's clock, the window of every signed route; a
//! request signed over its method and path alone is refused.
//!
//! Every verification failure answers upstream's 401
//! `{"ok":false,"data":{"message":...},"error":"Unauthenticated"}` with a
//! fixed message that names the failed check and nothing the server knows.
//! A request reaches these routes at the paths upstream served (`/v1/...`)
//! or behind the gateway prefix (`/builder-api/v1/...`); a client signs the
//! path it requested (`signed_fetch::candidate_paths`, the one path rule).

use axum::extract::{FromRequestParts, OriginalUri};
use axum::http::request::Parts;
use axum::http::{HeaderMap, Uri};

use crate::shared::signed_fetch::{self, SignedFetchError, Signer, VerificationProfile};
use dcl_auth::eip1654::Eip1654Validator;

use super::SharedState;
use super::http::ApiError;

pub use crate::shared::signed_fetch::SignedPath;

const PROFILE: VerificationProfile = VerificationProfile::builder();

async fn verify(
    headers: &HeaderMap,
    method: &str,
    path: &SignedPath,
    validator: Option<&dyn Eip1654Validator>,
) -> Result<Signer, SignedFetchError> {
    signed_fetch::require_signer_with(headers, method, path, &PROFILE, validator).await
}

/// upstream's refusal of a request whose signature did not verify; a client
/// source that spent its contract-wallet checks is refused with 429.
pub fn unauthenticated(error: &SignedFetchError) -> ApiError {
    let message = match error {
        SignedFetchError::RateLimited => return ApiError::http(429, error.to_string()),
        e if e.is_metadata_refusal() => "Invalid signature",
        SignedFetchError::MalformedChain { .. } | SignedFetchError::InsufficientLinks => {
            "Invalid auth chain"
        }
        SignedFetchError::InvalidTimestamp(_) => "Invalid timestamp",
        SignedFetchError::Expired { .. } => "Expired signature",
        SignedFetchError::InvalidSignature(_)
        | SignedFetchError::EipNotImplemented
        | SignedFetchError::CatalystUnavailable(_) => "Invalid signature",
    };
    ApiError::unauthenticated(message)
}

/// The verified, lowercased signer of a route that requires one; any
/// failure is [`unauthenticated`].
pub async fn require_signer(
    headers: &HeaderMap,
    method: &str,
    path: &SignedPath,
    validator: Option<&dyn Eip1654Validator>,
) -> Result<Signer, ApiError> {
    verify(headers, method, path, validator)
        .await
        .map_err(|error| unauthenticated(&error))
}

/// `withPermissiveAuthentication`: a refused or absent chain reads as
/// anonymous.
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

/// The verified signer of a route that requires one, extracted before the
/// route's path parameters and body so an unauthenticated request learns
/// nothing from their validation; any failure is [`unauthenticated`]. A
/// verified signer then spends from its budget ([`crate::limits::spend_wallet`]).
pub struct Signed(pub Signer);

impl FromRequestParts<SharedState> for Signed {
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, state: &SharedState) -> Result<Self, ApiError> {
        let signer = require_signer(
            &parts.headers,
            parts.method.as_str(),
            &request_path(parts),
            state.eip1654.as_deref(),
        )
        .await?;
        crate::limits::spend_wallet(state, signer.as_str(), &parts.method)?;
        Ok(Signed(signer))
    }
}

fn request_path(parts: &Parts) -> SignedPath {
    let original = parts
        .extensions
        .get::<OriginalUri>()
        .map_or(&parts.uri, |original| &original.0);
    signed_path(original, &parts.uri)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shared::signed_fetch::test_support::{
        address, signed_headers, signed_headers_at, untimestamped_headers,
    };
    use axum::http::HeaderValue;
    use axum::response::IntoResponse;
    use serde_json::{Value, json};

    const METHOD: &str = "get";
    const PATH: &str = "/v1/collections/1";

    async fn wire(err: ApiError) -> (u16, Value) {
        let resp = err.into_response();
        let status = resp.status().as_u16();
        let bytes = axum::body::to_bytes(resp.into_body(), 4096).await.unwrap();
        (status, serde_json::from_slice(&bytes).unwrap())
    }

    fn refusal(message: &str) -> (u16, Value) {
        (
            401,
            json!({ "ok": false, "data": { "message": message }, "error": "Unauthenticated" }),
        )
    }

    async fn refused(headers: &HeaderMap, path: &str) -> (u16, Value) {
        wire(
            require_signer(headers, METHOD, &SignedPath::local(path), None)
                .await
                .expect_err("refused"),
        )
        .await
    }

    #[tokio::test]
    async fn a_source_that_spent_its_contract_wallet_checks_is_refused_with_429() {
        assert_eq!(
            wire(unauthenticated(&SignedFetchError::RateLimited)).await,
            (
                429,
                json!({
                    "ok": false,
                    "data": {},
                    "error": "Too many contract-wallet signature checks; retry later"
                })
            )
        );
    }

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

    #[tokio::test]
    async fn every_verification_failure_is_upstreams_401_with_a_fixed_message() {
        assert_eq!(
            refused(&HeaderMap::new(), PATH).await,
            refusal("Invalid auth chain")
        );
        let mut garbage = HeaderMap::new();
        garbage.insert(
            "x-identity-auth-chain-0",
            HeaderValue::from_static("{not json"),
        );
        garbage.insert("x-identity-auth-chain-1", HeaderValue::from_static("{}"));
        assert_eq!(refused(&garbage, PATH).await, refusal("Invalid auth chain"));
        let now = chrono::Utc::now().timestamp_millis();
        for offset in [-6 * 60 * 1000, 60 * 60 * 1000] {
            let headers =
                signed_headers_at(METHOD, PATH, "{}", true, &(now + offset).to_string()).await;
            assert_eq!(refused(&headers, PATH).await, refusal("Expired signature"));
        }
        let mut headers = signed_headers(METHOD, PATH, "{}", true).await;
        headers.insert(
            signed_fetch::AUTH_TIMESTAMP_HEADER,
            HeaderValue::from_static("soon"),
        );
        assert_eq!(refused(&headers, PATH).await, refusal("Invalid timestamp"));
        let headers = signed_headers(METHOD, "/v1/collections/2", "{}", true).await;
        assert_eq!(refused(&headers, PATH).await, refusal("Invalid signature"));
        let headers = signed_headers("put", PATH, "{}", true).await;
        assert_eq!(refused(&headers, PATH).await, refusal("Invalid signature"));
        for metadata in [
            r#"{"signer":"decentraland-kernel-scene"}"#,
            r#"{"Signer":"decentraland-kernel-scene"}"#,
            r#"{"signer":"Dcl:Explorer"}"#,
            r#"[1]"#,
        ] {
            for v6 in [true, false] {
                let headers = signed_headers(METHOD, PATH, metadata, v6).await;
                assert_eq!(
                    refused(&headers, PATH).await,
                    refusal("Invalid signature"),
                    "{metadata} v6={v6}"
                );
                assert!(
                    try_extract_signer(&headers, METHOD, &SignedPath::local(PATH), None)
                        .await
                        .is_none(),
                    "{metadata} v6={v6}: the permissive path reads it as anonymous"
                );
            }
        }
    }

    #[tokio::test]
    async fn a_request_signed_over_method_and_path_alone_is_refused() {
        let headers =
            untimestamped_headers(METHOD, "/collections/1", "2099-01-01T00:00:00.000Z").await;
        assert_eq!(refused(&headers, PATH).await, refusal("Expired signature"));
        let headers = untimestamped_headers(METHOD, PATH, "2099-01-01T00:00:00.000Z").await;
        assert_eq!(refused(&headers, PATH).await, refusal("Expired signature"));
        assert!(
            try_extract_signer(&headers, METHOD, &SignedPath::local(PATH), None)
                .await
                .is_none()
        );
    }

    #[tokio::test]
    async fn a_timestamped_signature_does_not_pass_without_its_timestamp_header() {
        let mut headers = signed_headers(METHOD, PATH, "{}", true).await;
        headers.remove(signed_fetch::AUTH_TIMESTAMP_HEADER);
        assert_eq!(refused(&headers, PATH).await, refusal("Expired signature"));
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
}
