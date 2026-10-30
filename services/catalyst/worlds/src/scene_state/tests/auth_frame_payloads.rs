use crate::scene_state::auth::{AuthError, FIVE_MINUTES, verify_auth_frame};
use crate::scene_state::delegation::{Wallet, create_simple_auth_chain};
use crate::shared::signed_fetch::{
    AUTH_CHAIN_HEADER_PREFIX, AUTH_METADATA_HEADER, AUTH_TIMESTAMP_HEADER, build_legacy_payload,
    build_payload_v6,
};

const TEST_KEY: &str = "0x59c6995e998f97a5a0044966f0945389dc9e86dae88c7a8412f4603b6b78690d";
const METHOD: &str = "GET";
const PATH: &str = "/ws/MyWorld.dcl.eth";
const METADATA: &str =
    r#"{"realmName":"MyWorld.dcl.eth","sceneId":"bafkreiAbC123","parcel":"1,-2","isGuest":false}"#;

#[derive(Clone, Copy, Debug)]
enum Shape {
    Legacy,
    V6,
}

const SHAPES: [Shape; 2] = [Shape::Legacy, Shape::V6];

fn wallet() -> Wallet {
    Wallet::from_hex(TEST_KEY).unwrap()
}

fn expected_signer() -> String {
    wallet().address().to_lowercase()
}

fn payload(shape: Shape, method: &str, path: &str, ts: &str, metadata: &str) -> String {
    match shape {
        Shape::Legacy => build_legacy_payload(method, path, ts, metadata),
        Shape::V6 => build_payload_v6(method, path, ts, metadata),
    }
}

fn frame_for(
    shape: Shape,
    signed_path: &str,
    signed_metadata: &str,
    delivered_metadata: &str,
    ts_ms: i64,
) -> serde_json::Map<String, serde_json::Value> {
    let ts = ts_ms.to_string();
    let signed = payload(shape, METHOD, signed_path, &ts, signed_metadata);
    let chain = create_simple_auth_chain(&wallet(), &signed).unwrap();

    let mut obj = serde_json::Map::new();
    obj.insert(AUTH_TIMESTAMP_HEADER.to_string(), ts.into());
    obj.insert(
        AUTH_METADATA_HEADER.to_string(),
        delivered_metadata.to_string().into(),
    );
    for (i, link) in chain.as_array().into_iter().flatten().enumerate() {
        obj.insert(
            format!("{AUTH_CHAIN_HEADER_PREFIX}{i}"),
            link.to_string().into(),
        );
    }
    obj
}

pub(crate) fn frame(
    shape_v6: bool,
    signed_path: &str,
    signed_metadata: &str,
    delivered_metadata: &str,
    ts_ms: i64,
) -> Vec<u8> {
    let shape = if shape_v6 { Shape::V6 } else { Shape::Legacy };
    serde_json::Value::Object(frame_for(
        shape,
        signed_path,
        signed_metadata,
        delivered_metadata,
        ts_ms,
    ))
    .to_string()
    .into_bytes()
}

fn now_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

async fn verify(
    body: &[u8],
    path: &str,
    now: i64,
) -> Result<crate::scene_state::auth::Authenticated, AuthError> {
    verify_auth_frame(body, METHOD, &[path.to_string()], now, None).await
}

#[test]
fn the_fixture_metadata_makes_the_two_shapes_differ() {
    assert_ne!(
        build_legacy_payload(METHOD, PATH, "1", METADATA),
        build_payload_v6(METHOD, PATH, "1", METADATA)
    );
}

#[tokio::test]
async fn either_shape_verifies_through_the_ws_auth_frame() {
    for shape in SHAPES {
        let ts_ms = now_ms();
        let body = frame(matches!(shape, Shape::V6), PATH, METADATA, METADATA, ts_ms);
        let authed = verify(&body, PATH, ts_ms / 1000)
            .await
            .unwrap_or_else(|err| panic!("{shape:?}: {err:?}"));
        assert_eq!(authed.signer.as_str(), expected_signer());
        assert_eq!(authed.metadata["sceneId"], "bafkreiAbC123");
        assert_eq!(authed.presented_secret(), None);
    }
}

#[tokio::test]
async fn a_bad_signature_still_fails_as_invalid_signature_under_either_shape() {
    for shape in SHAPES {
        let ts_ms = now_ms();
        let now = ts_ms / 1000;

        let wrong_path = frame(
            matches!(shape, Shape::V6),
            "/ws/Other.dcl.eth",
            METADATA,
            METADATA,
            ts_ms,
        );
        let err = verify(&wrong_path, PATH, now).await.unwrap_err();
        assert!(
            matches!(err, AuthError::InvalidSignature(_)),
            "{shape:?}: {err:?}"
        );

        let tampered = METADATA.replace("bafkreiAbC123", "bafkreiXyZ999");
        assert_ne!(tampered, METADATA);
        let tampered_metadata = frame(matches!(shape, Shape::V6), PATH, METADATA, &tampered, ts_ms);
        let err = verify(&tampered_metadata, PATH, now).await.unwrap_err();
        assert!(
            matches!(err, AuthError::InvalidSignature(_)),
            "{shape:?}: {err:?}"
        );
    }
}

#[tokio::test]
async fn a_deterministic_failure_is_not_retried_against_the_other_shape() {
    for shape in SHAPES {
        let ts_ms = now_ms();
        let body = frame(matches!(shape, Shape::V6), PATH, METADATA, METADATA, ts_ms);
        let err = verify(&body, PATH, ts_ms / 1000 + FIVE_MINUTES + 60)
            .await
            .unwrap_err();
        assert!(
            matches!(err, AuthError::Expired { .. }),
            "{shape:?}: {err:?}"
        );
    }
}

#[tokio::test]
async fn a_scene_signed_frame_and_a_non_numeric_timestamp_are_refused() {
    let ts_ms = now_ms();
    let scene_meta = r#"{"signer":"decentraland-kernel-scene"}"#;
    let body = frame(true, PATH, scene_meta, scene_meta, ts_ms);
    assert!(matches!(
        verify(&body, PATH, ts_ms / 1000).await.unwrap_err(),
        AuthError::RefusedSigner
    ));

    let mut obj = frame_for(Shape::V6, PATH, METADATA, METADATA, ts_ms);
    obj.insert(AUTH_TIMESTAMP_HEADER.to_string(), "yesterday".into());
    let body = serde_json::Value::Object(obj).to_string().into_bytes();
    assert!(matches!(
        verify(&body, PATH, ts_ms / 1000).await.unwrap_err(),
        AuthError::MalformedChain(_)
    ));
}

#[tokio::test]
async fn the_secret_in_the_signed_metadata_is_handed_back_verified() {
    let ts_ms = now_ms();
    let meta = r#"{"secret":"open sesame"}"#;
    let body = frame(true, PATH, meta, meta, ts_ms);
    let authed = verify(&body, PATH, ts_ms / 1000).await.unwrap();
    assert_eq!(authed.presented_secret(), Some("open sesame"));
    // The first verifying candidate path settles it.
    let paths = vec![
        "/scene-state/ws/MyWorld.dcl.eth".to_string(),
        PATH.to_string(),
    ];
    assert!(
        verify_auth_frame(&body, METHOD, &paths, ts_ms / 1000, None)
            .await
            .is_ok()
    );
}
