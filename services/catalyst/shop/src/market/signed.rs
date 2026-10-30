//! The marketplace-server signed-fetch policy over the shared verifier
//! (`shared::signed_fetch`): the scene-signer gate every favorites, picks,
//! lists and catalog route applies (upstream `validateNotKernelSceneSigner`),
//! the signer/intent allow-list of `POST /v1/trades`
//! (`validateAuthMetadata`), and the two verification modes, required (401
//! on any refusal) and optional (a refused chain reads as a signed-out
//! visitor, never as the visiting user). Comparisons are exact against
//! canonical spellings: the legacy payload lowercased the metadata before
//! signing, so a re-cased `Signer` carries a byte-identical signature and
//! folding before comparing would authorize a request as something it is not.

use axum::http::HeaderMap;
use serde_json::Value;

use crate::shared::signed_fetch::{
    self, AUTH_CHAIN_HEADER_PREFIX, AUTH_METADATA_HEADER, PayloadPolicy, SignedFetchError,
};

use super::wire::ApiError;

/// Axum strips the mount from a nested request. Bind signatures to the actual
/// incoming path, replacing any caller-supplied forwarding header at that mount.
/// Standalone routes retain their existing reverse-proxy compatibility.
pub(super) async fn mounted_path(
    mut request: axum::extract::Request,
    next: axum::middleware::Next,
) -> axum::response::Response {
    if let Some(axum::extract::OriginalUri(uri)) =
        request.extensions().get::<axum::extract::OriginalUri>()
        && uri.path() != request.uri().path()
        && let Ok(path) = axum::http::HeaderValue::from_str(uri.path())
    {
        request.headers_mut().insert("x-original-path", path);
    }
    next.run(request).await
}

pub const SCENE_SIGNER: &str = "decentraland-kernel-scene";
pub const MARKETPLACE_AUTH_SIGNERS: &[&str] = &["dcl:marketplace", "dcl:builder"];
pub const CREATE_TRADE_INTENT: &str = "dcl:create-trade";

/// Upstream `verifyMetadata`: an unparseable or non-object header is refused
/// before any validator runs; an explicit JSON `null` is an empty object.
fn metadata_object(headers: &HeaderMap) -> Result<serde_json::Map<String, Value>, ApiError> {
    let raw = signed_fetch::header_str(headers, AUTH_METADATA_HEADER).unwrap_or("{}");
    match serde_json::from_str::<Value>(raw) {
        Ok(Value::Null) => Ok(serde_json::Map::new()),
        Ok(Value::Object(m)) => Ok(m),
        _ => {
            let echo: String = raw.chars().take(64).collect();
            Err(ApiError::bad_request(format!(
                "Invalid chain metadata: {echo}"
            )))
        }
    }
}

/// A `signer` key in any spelling whose value folds to the scene signer, or
/// a canonical `signer` whose value is not already canonical, is refused.
fn scene_signer_permits(metadata: &serde_json::Map<String, Value>) -> bool {
    for (key, value) in metadata {
        if !key.eq_ignore_ascii_case("signer") {
            continue;
        }
        if key != "signer" {
            return false;
        }
        match value.as_str() {
            Some(s) if s.eq_ignore_ascii_case(SCENE_SIGNER) => return false,
            Some(s) if s != s.trim() || s != s.to_lowercase() => return false,
            _ => {}
        }
    }
    true
}

/// Upstream answers 400 "Invalid signer" to a chain the explorer signed on a scene's behalf.
pub fn require_not_scene_signer(headers: &HeaderMap) -> Result<(), ApiError> {
    let metadata = metadata_object(headers)?;
    if !scene_signer_permits(&metadata) {
        return Err(ApiError::bad_request("Invalid signer"));
    }
    Ok(())
}

fn exact_field(metadata: &serde_json::Map<String, Value>, field: &str) -> Option<String> {
    let mut found = None;
    for (key, value) in metadata {
        if key.eq_ignore_ascii_case(field) {
            if key != field {
                return None;
            }
            found = value.as_str().map(str::to_string);
        }
    }
    found
}

/// Upstream `validateAuthMetadata(signers, intent)`: the route-level policy
/// behind `POST /v1/trades`.
pub fn require_auth_metadata(
    headers: &HeaderMap,
    allowed_signers: &[&str],
    intent: Option<&str>,
) -> Result<(), ApiError> {
    let metadata = metadata_object(headers)?;
    match exact_field(&metadata, "signer") {
        Some(s) if allowed_signers.contains(&s.as_str()) => {}
        _ => return Err(ApiError::bad_request("Invalid auth signer")),
    }
    if let Some(expected) = intent {
        match exact_field(&metadata, "intent") {
            Some(i) if i == expected => {}
            _ => {
                return Err(ApiError::bad_request(
                    "Invalid auth intent to perform this operation",
                ));
            }
        }
    }
    Ok(())
}

/// Marketplace-server wording: everything not special-cased is "Invalid Auth Chain".
fn message(e: &SignedFetchError) -> &'static str {
    match e {
        SignedFetchError::Expired { .. } => "Expired signature",
        SignedFetchError::EipNotImplemented => "EIP-1654 not supported on this route",
        _ => "Invalid Auth Chain",
    }
}

fn to_api(e: SignedFetchError) -> ApiError {
    match e {
        SignedFetchError::EipNotImplemented => ApiError::http(501, message(&e)),
        other => ApiError::unauthorized(message(&other)),
    }
}

/// The signer of a request that must be signed: the 6.x payload first, the
/// legacy one behind the canonical-`signer` guard, `x-original-path` honoured
/// when it ends with the route.
pub async fn require_signer(
    headers: &HeaderMap,
    method: &str,
    path: &str,
) -> Result<String, ApiError> {
    signed_fetch::require_signer(headers, method, path, PayloadPolicy::V6ThenLegacy, None)
        .await
        .map(|s| s.into_string())
        .map_err(to_api)
}

/// Upstream's `wellKnownComponents({ optional: true })`: no chain is anonymous,
/// a scene-signed or otherwise refused-by-metadata chain is anonymous too, a
/// chain that does not verify is a 401.
pub async fn optional_signer(
    headers: &HeaderMap,
    method: &str,
    path: &str,
) -> Result<Option<String>, ApiError> {
    if !headers.contains_key(format!("{AUTH_CHAIN_HEADER_PREFIX}0").as_str()) {
        return Ok(None);
    }
    if require_not_scene_signer(headers).is_err() {
        return Ok(None);
    }
    require_signer(headers, method, path).await.map(Some)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderValue;

    fn headers(metadata: &str) -> HeaderMap {
        let mut h = HeaderMap::new();
        h.insert(
            AUTH_METADATA_HEADER,
            HeaderValue::from_str(metadata).unwrap(),
        );
        h
    }

    #[test]
    fn the_canonical_and_recased_scene_signers_are_refused() {
        for metadata in [
            r#"{"signer":"decentraland-kernel-scene"}"#,
            r#"{"origin":"https://play.decentraland.org","signer":"decentraland-kernel-scene"}"#,
            r#"{"Signer":"decentraland-kernel-scene"}"#,
            r#"{"signer":"Decentraland-Kernel-Scene"}"#,
            r#"{"signer":"decentraland-kernel-scene","Signer":"x"}"#,
            r#"{"signer":" dcl:marketplace"}"#,
            r#"{"signer":"Dcl:Marketplace"}"#,
        ] {
            let err = require_not_scene_signer(&headers(metadata)).expect_err(metadata);
            assert_eq!(err.message(), "Invalid signer", "{metadata}");
        }
        for metadata in [
            r#"{"signer":"dcl:marketplace","intent":"dcl:marketplace:add-pick"}"#,
            r#"{"intent":"x"}"#,
            "{}",
            "null",
        ] {
            assert!(
                require_not_scene_signer(&headers(metadata)).is_ok(),
                "{metadata}"
            );
        }
        assert!(require_not_scene_signer(&HeaderMap::new()).is_ok());
        for metadata in ["not json", "[]", r#""a string""#, "7"] {
            assert!(
                require_not_scene_signer(&headers(metadata))
                    .unwrap_err()
                    .message()
                    .starts_with("Invalid chain metadata: "),
                "{metadata}"
            );
        }
    }

    #[test]
    fn the_trade_route_policy_matches_upstream_exactly() {
        for signer in MARKETPLACE_AUTH_SIGNERS {
            let m = format!(r#"{{"signer":"{signer}","intent":"{CREATE_TRADE_INTENT}"}}"#);
            assert!(
                require_auth_metadata(
                    &headers(&m),
                    MARKETPLACE_AUTH_SIGNERS,
                    Some(CREATE_TRADE_INTENT)
                )
                .is_ok()
            );
        }
        for m in [
            r#"{"signer":"decentraland-kernel-scene","intent":"dcl:create-trade"}"#,
            r#"{"signer":"Dcl:Marketplace","intent":"dcl:create-trade"}"#,
            r#"{"signer":" dcl:marketplace","intent":"dcl:create-trade"}"#,
            r#"{"signer":42,"intent":"dcl:create-trade"}"#,
            r#"{"Signer":"dcl:marketplace","intent":"dcl:create-trade"}"#,
            r#"{"signer":"dcl:marketplace","Signer":"x","intent":"dcl:create-trade"}"#,
            r#"{"intent":"dcl:create-trade"}"#,
            "{}",
        ] {
            let err = require_auth_metadata(
                &headers(m),
                MARKETPLACE_AUTH_SIGNERS,
                Some(CREATE_TRADE_INTENT),
            )
            .expect_err(m);
            assert_eq!(err.message(), "Invalid auth signer", "{m}");
        }
        for m in [
            r#"{"signer":"dcl:marketplace"}"#,
            r#"{"signer":"dcl:marketplace","intent":"dcl:marketplace:add-pick"}"#,
            r#"{"signer":"dcl:marketplace","intent":"Dcl:Create-Trade"}"#,
            r#"{"signer":"dcl:marketplace","intent":"dcl:create-trade "}"#,
            r#"{"signer":"dcl:marketplace","intent":null}"#,
        ] {
            let err = require_auth_metadata(
                &headers(m),
                MARKETPLACE_AUTH_SIGNERS,
                Some(CREATE_TRADE_INTENT),
            )
            .expect_err(m);
            assert_eq!(
                err.message(),
                "Invalid auth intent to perform this operation",
                "{m}"
            );
        }
        assert!(
            require_auth_metadata(
                &headers(r#"{"signer":"dcl:builder","intent":"anything"}"#),
                MARKETPLACE_AUTH_SIGNERS,
                None
            )
            .is_ok()
        );
        assert_eq!(
            require_auth_metadata(&HeaderMap::new(), MARKETPLACE_AUTH_SIGNERS, None)
                .unwrap_err()
                .message(),
            "Invalid auth signer"
        );
    }

    #[tokio::test]
    async fn optional_reads_a_refused_chain_as_anonymous_and_verifies_a_good_one() {
        for metadata in [
            r#"{"signer":"decentraland-kernel-scene"}"#,
            r#"{"Signer":"decentraland-kernel-scene"}"#,
            "not json",
        ] {
            let mut h = headers(metadata);
            h.insert(
                axum::http::HeaderName::from_static("x-identity-auth-chain-0"),
                HeaderValue::from_static("{}"),
            );
            assert_eq!(
                optional_signer(&h, "get", "/v1/catalog").await.ok(),
                Some(None),
                "{metadata}"
            );
        }
        assert_eq!(
            optional_signer(&HeaderMap::new(), "get", "/v1/catalog")
                .await
                .unwrap(),
            None
        );

        use crate::shared::signed_fetch::test_support::{address, signed_headers};
        let metadata = r#"{"signer":"dcl:marketplace","intent":"dcl:marketplace:add-pick"}"#;
        for v6 in [true, false] {
            let h = signed_headers("get", "/v1/lists", metadata, v6).await;
            assert_eq!(
                require_signer(&h, "get", "/v1/lists").await.unwrap(),
                address(),
                "v6={v6}"
            );
            assert_eq!(
                optional_signer(&h, "get", "/v1/lists").await.unwrap(),
                Some(address())
            );
            let err = require_signer(&h, "get", "/v1/other").await.unwrap_err();
            assert_eq!(err.message(), "Invalid Auth Chain");
            let err = optional_signer(&h, "get", "/v1/other")
                .await
                .expect_err("a chain that does not verify is not anonymous");
            assert_eq!(err.message(), "Invalid Auth Chain");
        }
        let mut h = signed_headers("get", "/market/v1/lists", metadata, true).await;
        h.insert(
            "x-original-path",
            HeaderValue::from_static("/market/v1/lists?page=1"),
        );
        assert_eq!(
            require_signer(&h, "get", "/v1/lists").await.unwrap(),
            address()
        );
    }
}
