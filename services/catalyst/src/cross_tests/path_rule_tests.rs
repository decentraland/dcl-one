//! The ONE path rule, exercised through every signed route family that
//! verifies over a [`SignedPath`]: a chain over the path the request arrived
//! on (its gateway prefix included) verifies; a chain over the bare
//! route-local path does not, unless the proxy's `x-original-path` says that
//! is what the client asked for. Before 2026-09-26 five modules accepted
//! both unconditionally and comms accepted the route-local one only, so a
//! client that worked against `/social-api` failed against
//! `/comms-gatekeeper` with the same code (`tests/conformance/REPORT.md`).

use axum::http::{HeaderMap, HeaderValue};

use crate::shared::signed_fetch::test_support::signed_headers;
use crate::shared::signed_fetch::{
    ORIGINAL_PATH_HEADER, PayloadPolicy, SignedPath, candidate_paths, require_signer_at,
};

const PREFIXED: &str = "/gateway-prefix/v1/route";
const LOCAL: &str = "/v1/route";

fn path() -> SignedPath {
    SignedPath::new(PREFIXED, LOCAL)
}

/// Headers signed over `signed_over`, optionally with the proxy header.
async fn headers(method: &str, signed_over: &str, original_path: Option<&str>) -> HeaderMap {
    let mut h = signed_headers(method, signed_over, "{}", true).await;
    if let Some(original) = original_path {
        h.insert(
            ORIGINAL_PATH_HEADER,
            HeaderValue::from_str(original).unwrap(),
        );
    }
    h
}

#[test]
fn the_candidates_are_the_request_path_and_a_matching_proxy_header() {
    let plain = HeaderMap::new();
    assert_eq!(candidate_paths(&plain, &path()), vec![PREFIXED]);
    let mut with_local = HeaderMap::new();
    with_local.insert(
        ORIGINAL_PATH_HEADER,
        HeaderValue::from_static("/v1/route?page=2"),
    );
    assert_eq!(
        candidate_paths(&with_local, &path()),
        vec![PREFIXED, "/v1/route"]
    );
    let mut with_other_prefix = HeaderMap::new();
    with_other_prefix.insert(
        ORIGINAL_PATH_HEADER,
        HeaderValue::from_static("/vhost/v1/route"),
    );
    assert_eq!(
        candidate_paths(&with_other_prefix, &path()),
        vec![PREFIXED, "/vhost/v1/route"]
    );
    let mut forged = HeaderMap::new();
    forged.insert(ORIGINAL_PATH_HEADER, HeaderValue::from_static("/v1/other"));
    assert_eq!(candidate_paths(&forged, &path()), vec![PREFIXED]);
    let mut same = HeaderMap::new();
    same.insert(ORIGINAL_PATH_HEADER, HeaderValue::from_static(PREFIXED));
    assert_eq!(
        candidate_paths(&same, &path()),
        vec![PREFIXED],
        "the request path is not listed twice"
    );
    assert_eq!(
        candidate_paths(&plain, &SignedPath::local(LOCAL)),
        vec![LOCAL]
    );
    assert_eq!(
        SignedPath::from_uris(&PREFIXED.parse().unwrap(), &LOCAL.parse().unwrap()),
        path()
    );
    assert_eq!(
        SignedPath::at(&format!("{PREFIXED}?x=1").parse().unwrap(), LOCAL),
        path()
    );
}

/// Every family below is one call into the shared rule, so the three
/// verdicts are the same for all of them: prefixed verifies, route-local
/// alone does not, route-local behind the proxy header does.
#[tokio::test]
async fn every_signed_route_family_signs_the_same_path() {
    let prefixed = headers("post", PREFIXED, None).await;
    let local_alone = headers("post", LOCAL, None).await;
    let local_behind_proxy = headers("post", LOCAL, Some(LOCAL)).await;
    let local_behind_vhost = headers("post", "/vhost/v1/route", Some("/vhost/v1/route?x=1")).await;
    let p = path();

    // explore, notifications, the content unpublish, the auth-api: the shared verifier itself
    for policy in [PayloadPolicy::V6ThenLegacy, PayloadPolicy::V6Only] {
        assert!(
            require_signer_at(&prefixed, "post", &p, policy, None)
                .await
                .is_ok(),
            "{policy:?}"
        );
        assert_eq!(
            require_signer_at(&local_alone, "post", &p, policy, None)
                .await
                .unwrap_err()
                .http_status_and_message()
                .0,
            401,
            "{policy:?}"
        );
        assert!(
            require_signer_at(&local_behind_proxy, "post", &p, policy, None)
                .await
                .is_ok(),
            "{policy:?}"
        );
        assert!(
            require_signer_at(&local_behind_vhost, "post", &p, policy, None)
                .await
                .is_ok(),
            "{policy:?}"
        );
    }

    assert!(
        crate::builder::auth::require_signer(&prefixed, "post", &p, None)
            .await
            .is_ok()
    );
    assert_eq!(
        crate::builder::auth::require_signer(&local_alone, "post", &p, None)
            .await
            .unwrap_err()
            .status(),
        401
    );
    assert!(
        crate::builder::auth::require_signer(&local_behind_proxy, "post", &p, None)
            .await
            .is_ok()
    );
    assert!(
        crate::builder::auth::try_extract_signer(&local_alone, "post", &p, None)
            .await
            .is_none()
    );

    // communities (6.x only)
    assert!(
        crate::social::communities::auth::require_signer(&prefixed, "post", &p, None)
            .await
            .is_ok()
    );
    assert!(
        crate::social::communities::auth::require_signer(&local_alone, "post", &p, None)
            .await
            .is_err()
    );
    assert!(
        crate::social::communities::auth::require_signer(&local_behind_proxy, "post", &p, None)
            .await
            .is_ok()
    );
    assert!(
        crate::social::communities::auth::try_extract_signer(&local_alone, "post", &p, None)
            .await
            .is_none()
    );

    // comms: the gatekeeper's 60 s window, its metadata gates, the same paths
    use crate::comms::gatekeeper::auth as comms;
    assert!(
        comms::verify_signed_fetch(&prefixed, "post", &p, &[], None)
            .await
            .is_ok()
    );
    assert_eq!(
        comms::verify_signed_fetch(&local_alone, "post", &p, &[], None)
            .await
            .map(|_| ())
            .unwrap_err()
            .status,
        401
    );
    assert!(
        comms::verify_signed_fetch(&local_behind_proxy, "post", &p, &[], None)
            .await
            .is_ok()
    );
    assert!(
        comms::verify_wallet_signed_fetch(&prefixed, "post", &p, None)
            .await
            .is_ok()
    );
    assert!(
        comms::verify_wallet_signed_fetch(&local_alone, "post", &p, None)
            .await
            .is_err()
    );
    assert!(
        comms::require_signer(&prefixed, "post", &p, None)
            .await
            .is_ok()
    );
    assert!(
        comms::require_signer(&local_alone, "post", &p, None)
            .await
            .is_err()
    );
    assert!(
        comms::require_signer(&local_behind_proxy, "post", &p, None)
            .await
            .is_ok()
    );

    // worlds: root-mounted `path` is both, the proxy header still counts
    let worlds_prefixed = headers(
        "put",
        "/worlds-content-server/world/w.dcl.eth/settings",
        Some("/worlds-content-server/world/w.dcl.eth/settings"),
    )
    .await;
    assert!(
        crate::worlds::core::auth::require_verified(
            &worlds_prefixed,
            "put",
            &SignedPath::local("/world/w.dcl.eth/settings"),
            &[],
            None
        )
        .await
        .is_ok()
    );
    let worlds_local = headers("put", "/world/w.dcl.eth/settings", None).await;
    assert!(
        crate::worlds::core::auth::require_verified(
            &worlds_local,
            "put",
            &SignedPath::local("/world/w.dcl.eth/settings"),
            &[],
            None
        )
        .await
        .is_ok()
    );
}

/// The comms gatekeeper before this rule: a chain over the route-local path
/// opened `/comms-gatekeeper/x` with no proxy involved. Not any more.
#[tokio::test]
async fn comms_no_longer_accepts_the_bare_route_local_path_behind_the_gateway() {
    let p = SignedPath::new("/comms-gatekeeper/get-scene-adapter", "/get-scene-adapter");
    let recorded = headers("post", "/comms-gatekeeper/get-scene-adapter", None).await;
    assert!(
        crate::comms::gatekeeper::auth::verify_signed_fetch(&recorded, "post", &p, &[], None)
            .await
            .is_ok(),
        "what the explorer signs"
    );
    let route_local = headers("post", "/get-scene-adapter", None).await;
    assert!(
        crate::comms::gatekeeper::auth::verify_signed_fetch(&route_local, "post", &p, &[], None)
            .await
            .is_err()
    );
    let root_alias = headers("post", "/get-scene-adapter", Some("/get-scene-adapter")).await;
    assert!(
        crate::comms::gatekeeper::auth::verify_signed_fetch(&root_alias, "post", &p, &[], None)
            .await
            .is_ok(),
        "nginx's root alias forwards the client's URI"
    );
}
