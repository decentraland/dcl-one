//! The camera-reel signed-fetch profile at the routes: a signature at most
//! 60 s old and never too new, `x-identity-metadata` required but unread,
//! and the scene signer served on the owner reads and refused on every write
//! with the same 401.

use axum::http::StatusCode;
use serde_json::{Value, json};

use super::{
    API_URL, PREFIX, Wallet, image, json_of, metadata_json, multipart, png, scratch, send, tag,
    wallet,
};
use crate::Settings;
use crate::auth::{AuthLink, AuthLinkType};
use crate::shared::signed_fetch::{
    AUTH_CHAIN_HEADER_PREFIX, AUTH_METADATA_HEADER, AUTH_TIMESTAMP_HEADER, build_payload_v6,
};
use alloy::signers::Signer as _;

const SCENE: &str = r#"{"signer":"decentraland-kernel-scene","sceneId":"bafkscene"}"#;

/// Headers `w` signs (6.x payload) for `method path` `offset_ms` from now,
/// carrying `metadata` (`None`: no metadata header, signed as `{}`).
async fn signed(
    w: &Wallet,
    method: &str,
    path: &str,
    metadata: Option<&str>,
    offset_ms: i64,
) -> Vec<(String, String)> {
    let timestamp = (chrono::Utc::now().timestamp_millis() + offset_ms).to_string();
    let payload = build_payload_v6(method, path, &timestamp, metadata.unwrap_or("{}"));
    let signature = w
        .signer
        .sign_message(payload.as_bytes())
        .await
        .unwrap()
        .to_string();
    let chain = [
        AuthLink {
            link_type: AuthLinkType::SIGNER,
            payload: w.address.clone(),
            signature: None,
        },
        AuthLink {
            link_type: AuthLinkType::EcdsaSignedEntity,
            payload,
            signature: Some(signature),
        },
    ];
    let mut out = vec![(AUTH_TIMESTAMP_HEADER.to_string(), timestamp)];
    if let Some(metadata) = metadata {
        out.push((AUTH_METADATA_HEADER.to_string(), metadata.to_string()));
    }
    for (i, link) in chain.iter().enumerate() {
        out.push((
            format!("{AUTH_CHAIN_HEADER_PREFIX}{i}"),
            serde_json::to_string(link).unwrap(),
        ));
    }
    out
}

fn unauthorized() -> (StatusCode, Value) {
    (
        StatusCode::UNAUTHORIZED,
        json!({ "message": "Unauthorized" }),
    )
}

#[tokio::test]
async fn a_scene_signed_read_lists_the_owners_private_photos() {
    let Some(mut s) = scratch(Settings::new(API_URL)).await else {
        return;
    };
    let w = wallet();
    let owner = s.owner(&w.address);
    let t = tag();
    let private = image(&owner, &format!("h-{t}"), &format!("t-{t}"), false, "");
    s.db().insert_image(&private).await.unwrap();

    let images = format!("{PREFIX}/api/users/{owner}/images");
    let user = format!("{PREFIX}/api/users/{owner}");
    let (_, _, resp) = send(s.app(), "GET", &images, vec![], None).await;
    assert_eq!(json_of(&resp)["currentImages"], 0, "anonymous");
    for metadata in [SCENE, "not json"] {
        let headers = signed(&w, "get", &images, Some(metadata), 0).await;
        let (status, _, resp) = send(s.app(), "GET", &images, headers, None).await;
        assert_eq!(status, StatusCode::OK);
        let page = json_of(&resp);
        assert_eq!(page["currentImages"], 1, "{metadata}");
        assert_eq!(
            page["images"][0]["id"],
            private.id.to_string(),
            "{metadata}"
        );
        let headers = signed(&w, "get", &user, Some(metadata), 0).await;
        let (_, _, resp) = send(s.app(), "GET", &user, headers, None).await;
        assert_eq!(json_of(&resp)["currentImages"], 1, "{metadata}");
    }
    s.done().await;
}

#[tokio::test]
async fn a_scene_signed_write_is_refused_on_every_write_route() {
    let mut settings = Settings::new(API_URL);
    settings.writes_enabled = true;
    let Some(mut s) = scratch(settings).await else {
        return;
    };
    let w = wallet();
    let owner = s.owner(&w.address);
    let t = tag();
    let private = image(&owner, &format!("h-{t}"), &format!("t-{t}"), false, "");
    s.db().insert_image(&private).await.unwrap();

    let upload = format!("{PREFIX}/api/images");
    let (body, ct) = multipart(
        &png(16, 9, 1),
        "image/png",
        &metadata_json(&owner, ""),
        None,
    );
    let headers = signed(&w, "post", &upload, Some(SCENE), 0).await;
    let (status, _, resp) = send(s.app(), "POST", &upload, headers, Some((body, &ct))).await;
    assert_eq!((status, json_of(&resp)), unauthorized(), "multipart upload");

    let upload_json = format!("{PREFIX}/api/images-json");
    let body = {
        use base64::Engine as _;
        json!({
            "image": base64::engine::general_purpose::STANDARD.encode(png(16, 9, 2)),
            "content_type": "image/png",
            "metadata": metadata_json(&owner, ""),
        })
        .to_string()
        .into_bytes()
    };
    let headers = signed(&w, "post", &upload_json, Some(SCENE), 0).await;
    let (status, _, resp) = send(
        s.app(),
        "POST",
        &upload_json,
        headers,
        Some((body, "application/json")),
    )
    .await;
    assert_eq!((status, json_of(&resp)), unauthorized(), "json upload");
    assert_eq!(s.count(&owner).await, 1, "no upload landed");

    let vis = format!("{PREFIX}/api/images/{}/visibility", private.id);
    let flip = json!({ "is_public": true }).to_string().into_bytes();
    for metadata in [SCENE, r#"{"Signer":" Decentraland-Kernel-Scene"}"#] {
        let headers = signed(&w, "patch", &vis, Some(metadata), 0).await;
        let (status, _, resp) = send(
            s.app(),
            "PATCH",
            &vis,
            headers,
            Some((flip.clone(), "application/json")),
        )
        .await;
        assert_eq!((status, json_of(&resp)), unauthorized(), "{metadata}");
    }
    assert_eq!(s.is_public(private.id).await, Some(false));

    let del = format!("{PREFIX}/api/images/{}", private.id);
    let headers = signed(&w, "delete", &del, Some(SCENE), 0).await;
    let (status, _, resp) = send(s.app(), "DELETE", &del, headers, None).await;
    assert_eq!((status, json_of(&resp)), unauthorized(), "delete");
    assert_eq!(s.count(&owner).await, 1, "the row stays");

    // Metadata that does not parse is not a scene's: the write goes through.
    let headers = signed(&w, "patch", &vis, Some("not json"), 0).await;
    let (status, _, _) = send(
        s.app(),
        "PATCH",
        &vis,
        headers,
        Some((flip, "application/json")),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(s.is_public(private.id).await, Some(true));
    s.done().await;
}

#[tokio::test]
async fn the_timestamp_window_and_the_metadata_header_are_upstreams() {
    let mut settings = Settings::new(API_URL);
    settings.writes_enabled = true;
    let Some(mut s) = scratch(settings).await else {
        return;
    };
    let w = wallet();
    let owner = s.owner(&w.address);
    let t = tag();
    let private = image(&owner, &format!("h-{t}"), &format!("t-{t}"), false, "");
    s.db().insert_image(&private).await.unwrap();
    let vis = format!("{PREFIX}/api/images/{}/visibility", private.id);
    let images = format!("{PREFIX}/api/users/{owner}/images");
    let patch = |headers, public: bool| {
        let app = s.app();
        let vis = vis.clone();
        async move {
            let body = json!({ "is_public": public }).to_string().into_bytes();
            let (status, _, resp) = send(
                app,
                "PATCH",
                &vis,
                headers,
                Some((body, "application/json")),
            )
            .await;
            (status, json_of(&resp))
        }
    };
    let listed = |headers| {
        let app = s.app();
        let images = images.clone();
        async move {
            let (_, _, resp) = send(app, "GET", &images, headers, None).await;
            json_of(&resp)["currentImages"].as_u64()
        }
    };

    let two_minutes_ago = -2 * 60 * 1000;
    assert_eq!(
        patch(
            signed(&w, "patch", &vis, Some("{}"), two_minutes_ago).await,
            true
        )
        .await,
        unauthorized(),
        "a 2-minute-old signature"
    );
    assert_eq!(
        listed(signed(&w, "get", &images, Some("{}"), two_minutes_ago).await).await,
        Some(0),
        "a stale read is anonymous"
    );

    assert_eq!(
        patch(signed(&w, "patch", &vis, None, 0).await, true).await,
        unauthorized(),
        "x-identity-metadata missing"
    );
    assert_eq!(
        listed(signed(&w, "get", &images, None, 0).await).await,
        Some(0),
        "a read without metadata is anonymous"
    );
    assert_eq!(s.is_public(private.id).await, Some(false));

    let an_hour_ahead = 60 * 60 * 1000;
    assert_eq!(
        listed(signed(&w, "get", &images, Some("{}"), an_hour_ahead).await).await,
        Some(1),
        "a future signature reads as the owner"
    );
    let (status, _) = patch(
        signed(&w, "patch", &vis, Some("{}"), an_hour_ahead).await,
        true,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "a future signature writes");
    assert_eq!(s.is_public(private.id).await, Some(true));
    s.done().await;
}
