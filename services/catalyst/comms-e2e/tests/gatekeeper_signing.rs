//! What the gatekeeper client signs, checked with the gatekeeper's own verifier.

use axum::http::{HeaderMap, HeaderName, HeaderValue};
use catalyst_comms::gatekeeper::auth::verify_wallet_signed_fetch;
use catalyst_comms_e2e::gatekeeper::{scene_adapter_metadata, signed_path};
use catalyst_comms_e2e::identity::{Identity, now_ms};
use dcl_auth::signed_fetch::{FIVE_MINUTES, SignedPath};

fn headers(pairs: Vec<(String, String)>) -> HeaderMap {
    let mut map = HeaderMap::new();
    for (name, value) in pairs {
        map.insert(
            HeaderName::from_bytes(name.as_bytes()).unwrap(),
            HeaderValue::from_str(&value).unwrap(),
        );
    }
    map
}

const ROUTE: &str = "/get-scene-adapter";

fn scene_fetch(identity: &Identity, path: &str, timestamp_ms: i64) -> HeaderMap {
    headers(identity.signed_fetch(
        "post",
        path,
        &scene_adapter_metadata("main", "scene-1"),
        timestamp_ms,
    ))
}

/// The path the gatekeeper compares: the request path and the route it serves.
fn served() -> SignedPath {
    SignedPath::new(signed_path(ROUTE), ROUTE)
}

#[tokio::test]
async fn the_gatekeeper_accepts_a_scene_adapter_fetch_and_reads_the_wallet_and_the_scene() {
    let identity = Identity::derive("gatekeeper-signing", 0);
    let verified = verify_wallet_signed_fetch(
        &scene_fetch(&identity, &signed_path(ROUTE), now_ms()),
        "post",
        &served(),
        None,
    )
    .await
    .expect("the gatekeeper's verifier accepts the fetch");
    assert_eq!(verified.signer.as_str(), identity.address());
    assert_eq!(verified.metadata["realmName"], "main");
    assert_eq!(verified.metadata["sceneId"], "scene-1");
    assert_eq!(verified.metadata["signer"], "dcl:explorer");
}

#[tokio::test]
async fn a_second_device_signs_for_the_same_wallet() {
    let identity = Identity::derive("gatekeeper-signing", 1);
    let other = identity.second_device();
    let verified = verify_wallet_signed_fetch(
        &scene_fetch(&other, &signed_path(ROUTE), now_ms()),
        "post",
        &served(),
        None,
    )
    .await
    .unwrap();
    assert_eq!(verified.signer.as_str(), identity.address());
}

#[tokio::test]
async fn a_fetch_signed_over_the_route_without_its_prefix_is_refused() {
    // the gatekeeper compares the path the request arrived on, prefix included
    let identity = Identity::derive("gatekeeper-signing", 2);
    let refused = verify_wallet_signed_fetch(
        &scene_fetch(&identity, ROUTE, now_ms()),
        "post",
        &served(),
        None,
    )
    .await;
    assert!(refused.is_err());
}

#[tokio::test]
async fn a_stale_fetch_is_refused() {
    let identity = Identity::derive("gatekeeper-signing", 3);
    let refused = verify_wallet_signed_fetch(
        // one second older than the window a signed fetch is accepted in
        &scene_fetch(
            &identity,
            &signed_path(ROUTE),
            now_ms() - (FIVE_MINUTES + 1) * 1000,
        ),
        "post",
        &served(),
        None,
    )
    .await;
    assert!(refused.is_err());
}
