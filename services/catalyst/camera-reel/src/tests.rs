//! The old crate's contract tests over the module as `serve` mounts it
//! (nested under `/camera-reel-service`), plus the route-level walks the
//! consolidation adds: the explorer boot corpus shapes (rows 2 and 34 of
//! `tests/explorer-boot.md`), the write gate, an upload over signed fetch
//! served back from the ONE blob store, the keyset pages and the moderator's
//! review. DB-backed tests use the process's scratch database
//! (`CATALYST_TEST_DATABASE_URL`), rows scoped by random wallets and
//! uuid-suffixed hashes, and delete their rows at the end; without the
//! variable they skip.

use std::sync::Arc;

use alloy::signers::{Signer as _, local::PrivateKeySigner};
use axum::Router;
use axum::body::{Body, to_bytes};
use axum::http::{HeaderMap, Request, StatusCode, header};
use axum::response::IntoResponse;
use serde_json::{Value, json};
use tower::ServiceExt;
use uuid::Uuid;

use super::db::{BlobVerdict, Database, NewImage, Page, WITHHELD_DELETED, WITHHELD_REJECTED};
use super::dto::{Image, Metadata, User};
use super::http::{ApiError, ForbiddenError, ForbiddenReason};
use super::{Settings, SharedState, State, router};
use crate::auth::{AuthLink, AuthLinkType, hash_bytes_v1};
use crate::shared::signed_fetch::{
    AUTH_CHAIN_HEADER_PREFIX, AUTH_METADATA_HEADER, AUTH_TIMESTAMP_HEADER, build_legacy_payload,
    build_payload_v6,
};
use crate::storage::Storage;

const API_URL: &str = "http://127.0.0.1:5149/camera-reel-service";
const PREFIX: &str = "/camera-reel-service";

async fn parts(err: ApiError) -> (StatusCode, String, Value) {
    let resp = err.into_response();
    let status = resp.status();
    let content_type = resp
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default()
        .to_string();
    let bytes = to_bytes(resp.into_body(), 4096).await.unwrap();
    let value: Value = serde_json::from_slice(&bytes).unwrap();
    (status, content_type, value)
}

fn assert_json_content_type(content_type: &str) {
    assert!(
        content_type.starts_with("application/json"),
        "content-type was {content_type}"
    );
}

fn key_set(value: &Value) -> Vec<String> {
    let mut keys: Vec<String> = value
        .as_object()
        .expect("body is a json object")
        .keys()
        .cloned()
        .collect();
    keys.sort();
    keys
}

#[tokio::test]
async fn max_limit_reached_matches_upstream_forbidden_error() {
    let message = "you have reached the limit of 500 max images";
    let (status, content_type, value) = parts(ApiError::MaxLimitReached(message.to_string())).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_json_content_type(&content_type);
    assert_eq!(
        value,
        json!({ "reason": "maxLimitReached", "message": message })
    );
    assert_eq!(key_set(&value), vec!["message", "reason"]);
}

#[tokio::test]
async fn max_limit_reached_roundtrips_through_typed_struct() {
    let message = "you have reached the limit of 500 max images";
    let (_status, _content_type, value) =
        parts(ApiError::MaxLimitReached(message.to_string())).await;
    let typed: ForbiddenError = serde_json::from_value(value).unwrap();
    assert_eq!(typed.reason, ForbiddenReason::MaxLimitReached);
    assert_eq!(typed.message, message);
}

#[tokio::test]
async fn bad_request_matches_upstream_response_error() {
    let (status, content_type, value) =
        parts(ApiError::BadRequest("invalid metadata".to_string())).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_json_content_type(&content_type);
    assert_eq!(value, json!({ "message": "invalid metadata" }));
    assert_eq!(key_set(&value), vec!["message"]);
}

#[tokio::test]
async fn unauthorized_matches_bare_message_envelope() {
    let (status, content_type, value) = parts(ApiError::Unauthorized).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_json_content_type(&content_type);
    assert_eq!(value, json!({ "message": "Unauthorized" }));
    assert_eq!(key_set(&value), vec!["message"]);
}

#[tokio::test]
async fn forbidden_matches_upstream_response_error() {
    let (status, content_type, value) = parts(ApiError::Forbidden("forbidden".to_string())).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_json_content_type(&content_type);
    assert_eq!(value, json!({ "message": "forbidden" }));
    assert_eq!(key_set(&value), vec!["message"]);
}

#[tokio::test]
async fn not_found_matches_upstream_response_error() {
    let (status, content_type, value) =
        parts(ApiError::NotFound("image not found".to_string())).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_json_content_type(&content_type);
    assert_eq!(value, json!({ "message": "image not found" }));
    assert_eq!(key_set(&value), vec!["message"]);
}

#[tokio::test]
async fn bad_gateway_matches_upstream_response_error() {
    let (status, content_type, value) = parts(ApiError::BadGateway(
        "failed to resolve world name".to_string(),
    ))
    .await;
    assert_eq!(status, StatusCode::BAD_GATEWAY);
    assert_json_content_type(&content_type);
    assert_eq!(value, json!({ "message": "failed to resolve world name" }));
    assert_eq!(key_set(&value), vec!["message"]);
}

#[tokio::test]
async fn internal_matches_upstream_response_error() {
    let (status, content_type, value) =
        parts(ApiError::Internal("database error".to_string())).await;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
    assert_json_content_type(&content_type);
    assert_eq!(value, json!({ "message": "database error" }));
    assert_eq!(key_set(&value), vec!["message"]);
}

struct Scratch {
    state: SharedState,
    owners: Vec<String>,
    /// Hashes a test may have withheld (`camera_reel.withheld_blobs` rows
    /// outlive their image row by design); `done` deletes them.
    hashes: Vec<String>,
    dir: std::path::PathBuf,
}

async fn scratch(settings: Settings) -> Option<Scratch> {
    let pool = catalyst_db::scratch::pool(4).await?;
    let dir =
        std::env::temp_dir().join(format!("catalyst-camera-reel-{}", Uuid::new_v4().simple()));
    let storage = Arc::new(
        Storage::from_config(&dir)
            .await
            .expect("a scratch blob store"),
    );
    let state = Arc::new(State::new(pool, storage, settings, None));
    Some(Scratch {
        state,
        owners: Vec::new(),
        hashes: Vec::new(),
        dir,
    })
}

impl Scratch {
    fn db(&self) -> &Database {
        &self.state.db
    }

    fn app(&self) -> Router {
        Router::new().nest(PREFIX, router(self.state.clone()))
    }

    fn owner(&mut self, address: &str) -> String {
        self.owners.push(address.to_lowercase());
        address.to_string()
    }

    fn hash(&mut self, hash: &str) -> String {
        self.hashes.push(hash.to_string());
        hash.to_string()
    }

    async fn withheld(&self, hash: &str) -> Option<String> {
        sqlx::query_scalar("SELECT reason FROM camera_reel.withheld_blobs WHERE hash = $1")
            .bind(hash)
            .fetch_optional(self.db().pool())
            .await
            .unwrap()
    }

    async fn is_public(&self, id: Uuid) -> Option<bool> {
        sqlx::query_scalar("SELECT is_public FROM camera_reel.images WHERE id = $1")
            .bind(id)
            .fetch_optional(self.db().pool())
            .await
            .unwrap()
    }

    async fn count(&self, owner: &str) -> i64 {
        sqlx::query_scalar("SELECT COUNT(*) FROM camera_reel.images WHERE user_address = $1")
            .bind(owner.to_lowercase())
            .fetch_one(self.db().pool())
            .await
            .unwrap()
    }

    async fn done(self) {
        sqlx::query("DELETE FROM camera_reel.images WHERE user_address = ANY($1)")
            .bind(&self.owners)
            .execute(self.db().pool())
            .await
            .unwrap();
        sqlx::query("DELETE FROM camera_reel.withheld_blobs WHERE hash = ANY($1)")
            .bind(&self.hashes)
            .execute(self.db().pool())
            .await
            .unwrap();
        let _ = tokio::fs::remove_dir_all(&self.dir).await;
    }
}

fn random_address() -> String {
    format!("{:#x}", PrivateKeySigner::random().address())
}

fn tag() -> String {
    Uuid::new_v4().simple().to_string()
}

fn image(owner: &str, hash: &str, thumb: &str, is_public: bool, place_id: &str) -> NewImage {
    NewImage {
        id: Uuid::new_v4(),
        image_hash: hash.to_string(),
        thumbnail_hash: thumb.to_string(),
        is_public,
        metadata: Metadata {
            user_name: "owner".to_string(),
            user_address: owner.to_string(),
            date_time: "1700000000".to_string(),
            place_id: place_id.to_string(),
            ..Metadata::default()
        },
    }
}

struct Wallet {
    signer: PrivateKeySigner,
    address: String,
}

fn wallet() -> Wallet {
    let signer = PrivateKeySigner::random();
    let address = format!("{:#x}", signer.address());
    Wallet { signer, address }
}

async fn signed_headers(w: &Wallet, method: &str, path: &str, v6: bool) -> Vec<(String, String)> {
    let timestamp = chrono::Utc::now().timestamp_millis().to_string();
    let payload = if v6 {
        build_payload_v6(method, path, &timestamp, "{}")
    } else {
        build_legacy_payload(method, path, &timestamp, "{}")
    };
    let signature = w
        .signer
        .sign_message(payload.as_bytes())
        .await
        .unwrap()
        .to_string();
    let chain = [
        AuthLink {
            link_type: AuthLinkType::SIGNER,
            payload: format!("{:#x}", w.signer.address()),
            signature: None,
        },
        AuthLink {
            link_type: AuthLinkType::EcdsaSignedEntity,
            payload,
            signature: Some(signature),
        },
    ];
    let mut out = vec![
        (AUTH_TIMESTAMP_HEADER.to_string(), timestamp),
        (AUTH_METADATA_HEADER.to_string(), "{}".to_string()),
    ];
    for (i, link) in chain.iter().enumerate() {
        out.push((
            format!("{AUTH_CHAIN_HEADER_PREFIX}{i}"),
            serde_json::to_string(link).unwrap(),
        ));
    }
    out
}

async fn send(
    app: Router,
    method: &str,
    path: &str,
    headers: Vec<(String, String)>,
    body: Option<(Vec<u8>, &str)>,
) -> (StatusCode, HeaderMap, Vec<u8>) {
    let mut req = Request::builder().method(method).uri(path);
    for (k, v) in &headers {
        req = req.header(k.as_str(), v.as_str());
    }
    let req = match body {
        Some((bytes, content_type)) => req
            .header(header::CONTENT_TYPE, content_type)
            .body(Body::from(bytes))
            .unwrap(),
        None => req.body(Body::empty()).unwrap(),
    };
    let resp = app.oneshot(req).await.unwrap();
    let status = resp.status();
    let headers = resp.headers().clone();
    let bytes = to_bytes(resp.into_body(), 32 << 20).await.unwrap();
    (status, headers, bytes.to_vec())
}

fn json_of(bytes: &[u8]) -> Value {
    if bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(bytes).unwrap()
    }
}

fn png(width: u32, height: u32, seed: u8) -> Vec<u8> {
    let img = image::RgbaImage::from_fn(width, height, |x, y| {
        image::Rgba([(x as u8).wrapping_mul(seed), y as u8, seed, 255])
    });
    let mut out = std::io::Cursor::new(Vec::new());
    image::DynamicImage::ImageRgba8(img)
        .write_to(&mut out, image::ImageFormat::Png)
        .unwrap();
    out.into_inner()
}

fn multipart(
    image: &[u8],
    content_type: &str,
    metadata: &Value,
    is_public: Option<bool>,
) -> (Vec<u8>, String) {
    let boundary = format!("----catalyst{}", tag());
    let mut body = Vec::new();
    let mut part = |name: &str, filename: Option<&str>, ct: Option<&str>, data: &[u8]| {
        body.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
        match filename {
            Some(f) => body.extend_from_slice(
                format!("Content-Disposition: form-data; name=\"{name}\"; filename=\"{f}\"\r\n")
                    .as_bytes(),
            ),
            None => body.extend_from_slice(
                format!("Content-Disposition: form-data; name=\"{name}\"\r\n").as_bytes(),
            ),
        }
        if let Some(ct) = ct {
            body.extend_from_slice(format!("Content-Type: {ct}\r\n").as_bytes());
        }
        body.extend_from_slice(b"\r\n");
        body.extend_from_slice(data);
        body.extend_from_slice(b"\r\n");
    };
    part("image", Some("shot.png"), Some(content_type), image);
    part(
        "metadata",
        None,
        Some("application/json"),
        metadata.to_string().as_bytes(),
    );
    if let Some(public) = is_public {
        part("is_public", None, None, public.to_string().as_bytes());
    }
    body.extend_from_slice(format!("--{boundary}--\r\n").as_bytes());
    (body, format!("multipart/form-data; boundary={boundary}"))
}

fn metadata_json(address: &str, place_id: &str) -> Value {
    json!({
        "userName": "tester",
        "userAddress": address,
        "dateTime": "1700000000",
        "realm": "main",
        "scene": { "name": "Genesis Plaza", "location": { "x": "0", "y": "0" } },
        "visiblePeople": [],
        "placeId": place_id
    })
}

#[tokio::test]
async fn private_image_metadata_is_returned_without_auth() {
    let Some(mut s) = scratch(Settings::new(API_URL)).await else {
        return;
    };
    let owner = s.owner(&random_address());
    let t = tag();
    let img = image(
        &owner,
        &format!("deadbeef-{t}"),
        &format!("cafebabe-{t}"),
        false,
        "",
    );
    s.db()
        .insert_image(&img)
        .await
        .expect("insert private image");

    let (status, _, body) = send(
        s.app(),
        "GET",
        &format!("{PREFIX}/api/images/{}/metadata", img.id),
        vec![],
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{}", String::from_utf8_lossy(&body));
    let returned: Image = serde_json::from_slice(&body).expect("parse Image json");
    assert_eq!(returned.id, img.id.to_string());
    assert!(
        !returned.is_public,
        "returned image must be the private one (is_public stays false)"
    );
    assert_eq!(returned.metadata.user_address, owner);
    assert_eq!(returned.url, format!("{API_URL}/api/images/deadbeef-{t}"));
    assert_eq!(
        returned.thumbnail_url,
        format!("{API_URL}/api/images/cafebabe-{t}")
    );
    s.done().await;
}

#[tokio::test]
async fn missing_image_metadata_is_not_found_not_unauthorized() {
    let Some(s) = scratch(Settings::new(API_URL)).await else {
        return;
    };
    let (status, _, body) = send(
        s.app(),
        "GET",
        &format!("{PREFIX}/api/images/{}/metadata", Uuid::new_v4()),
        vec![],
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(json_of(&body), json!({ "message": "image not found" }));
    let (status, _, _) = send(
        s.app(),
        "GET",
        &format!("{PREFIX}/api/images/not-a-uuid/metadata"),
        vec![],
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    s.done().await;
}

#[tokio::test]
async fn the_insert_itself_enforces_the_per_user_limit() {
    let Some(mut s) = scratch(Settings::new(API_URL)).await else {
        return;
    };
    let a = s.owner(&random_address());
    let b = s.owner(&random_address());
    let t = tag();
    assert_eq!(
        s.db()
            .insert_image_within_limit(
                &image(&a, &format!("h1-{t}"), &format!("t1-{t}"), true, ""),
                2
            )
            .await
            .unwrap(),
        Some(1)
    );
    assert_eq!(
        s.db()
            .insert_image_within_limit(
                &image(&a, &format!("h2-{t}"), &format!("t2-{t}"), true, ""),
                2
            )
            .await
            .unwrap(),
        Some(2)
    );
    assert_eq!(
        s.db()
            .insert_image_within_limit(
                &image(&a, &format!("h3-{t}"), &format!("t3-{t}"), true, ""),
                2
            )
            .await
            .unwrap(),
        None,
        "the third row must be refused by the statement"
    );
    assert_eq!(s.count(&a).await, 2);
    assert_eq!(
        s.db()
            .insert_image_within_limit(
                &image(&b, &format!("h3-{t}"), &format!("t3-{t}"), true, ""),
                2
            )
            .await
            .unwrap(),
        Some(1),
        "limits are per user"
    );
    s.done().await;
}

#[tokio::test]
async fn owner_delete_reports_ownership_and_remaining() {
    let Some(mut s) = scratch(Settings::new(API_URL)).await else {
        return;
    };
    let a = s.owner(&random_address());
    let b = s.owner(&random_address());
    let t = tag();
    let (h1, h3, t1, t2) = (
        s.hash(&format!("h1-{t}")),
        s.hash(&format!("h3-{t}")),
        s.hash(&format!("t1-{t}")),
        s.hash(&format!("t2-{t}")),
    );
    let a1 = image(&a, &h1, &t1, true, "");
    let a2 = image(&a, &h1, &t2, true, "");
    let b1 = image(&b, &h3, &t1, true, "");
    for img in [&a1, &a2, &b1] {
        s.db().insert_image(img).await.unwrap();
    }
    let now = chrono::Utc::now().naive_utc();

    assert!(
        s.db()
            .delete_image_owned(&Uuid::new_v4().to_string(), Some(&a), now)
            .await
            .unwrap()
            .is_none()
    );
    assert!(
        s.db()
            .delete_image_owned("not-a-uuid", Some(&a), now)
            .await
            .unwrap()
            .is_none()
    );

    let refused = s
        .db()
        .delete_image_owned(&a2.id.to_string(), Some(&b), now)
        .await
        .unwrap()
        .expect("row exists");
    assert!(
        !refused.deleted,
        "another user's image is reported, not deleted"
    );
    assert!(refused.withheld.is_empty());
    assert_eq!(s.count(&a).await, 2);

    let d = s
        .db()
        .delete_image_owned(&a1.id.to_string(), Some(&a.to_uppercase()), now)
        .await
        .unwrap()
        .unwrap();
    assert!(d.deleted);
    assert_eq!(d.user_address, a.to_lowercase());
    assert_eq!(
        (
            d.image_hash.as_str(),
            d.thumbnail_hash.as_str(),
            d.remaining
        ),
        (h1.as_str(), t1.as_str(), 1)
    );
    assert!(
        d.withheld.is_empty(),
        "a2 still names h1 and b1 still names t1: nothing withheld"
    );
    assert_eq!(s.withheld(&h1).await, None);

    let d = s
        .db()
        .delete_image_owned(&a2.id.to_string(), Some(&a), now)
        .await
        .unwrap()
        .unwrap();
    assert_eq!((d.deleted, d.remaining), (true, 0));
    assert_eq!(
        d.withheld,
        vec![h1.clone(), t2.clone()],
        "the last row naming h1 and t2 is gone"
    );
    assert_eq!(s.withheld(&h1).await.as_deref(), Some(WITHHELD_DELETED));
    assert_eq!(s.withheld(&t1).await, None, "b1 still names t1");

    let d = s
        .db()
        .delete_image_owned(&b1.id.to_string(), None, now)
        .await
        .unwrap()
        .unwrap();
    assert_eq!((d.deleted, d.remaining), (true, 0));
    assert_eq!(d.withheld, vec![h3.clone(), t1.clone()]);
    assert_eq!(s.count(&a).await + s.count(&b).await, 0);
    s.done().await;
}

#[tokio::test]
async fn visibility_update_keeps_404_403_and_noop_apart() {
    let Some(mut s) = scratch(Settings::new(API_URL)).await else {
        return;
    };
    let a = s.owner(&random_address());
    let b = s.owner(&random_address());
    let t = tag();
    let a1 = image(&a, &format!("h1-{t}"), &format!("t1-{t}"), false, "");
    s.db().insert_image(&a1).await.unwrap();
    let id = a1.id.to_string();

    assert_eq!(
        s.db()
            .set_image_visibility(&Uuid::new_v4().to_string(), &a, true)
            .await
            .unwrap(),
        None
    );
    assert_eq!(
        s.db().set_image_visibility("nope", &a, true).await.unwrap(),
        None
    );
    assert_eq!(
        s.db().set_image_visibility(&id, &b, true).await.unwrap(),
        Some(false)
    );
    assert_eq!(
        s.is_public(a1.id).await,
        Some(false),
        "a stranger's update changes nothing"
    );
    assert_eq!(
        s.db()
            .set_image_visibility(&id, &a.to_uppercase(), true)
            .await
            .unwrap(),
        Some(true)
    );
    assert_eq!(s.is_public(a1.id).await, Some(true));
    assert_eq!(
        s.db().set_image_visibility(&id, &a, true).await.unwrap(),
        Some(true),
        "no-op is still owned"
    );
    assert_eq!(s.is_public(a1.id).await, Some(true));
    s.done().await;
}

#[tokio::test]
async fn pages_carry_the_total_and_fall_back_past_the_end() {
    let Some(mut s) = scratch(Settings::new(API_URL)).await else {
        return;
    };
    let a = s.owner(&random_address());
    let b = s.owner(&random_address());
    let t = tag();
    let place = Uuid::new_v4().to_string();
    for (i, public) in [true, true, true, false].into_iter().enumerate() {
        let place_id = if i < 2 { place.as_str() } else { "" };
        s.db()
            .insert_image(&image(
                &a,
                &format!("h{i}-{t}"),
                &format!("t{i}-{t}"),
                public,
                place_id,
            ))
            .await
            .unwrap();
    }
    let db = s.db();

    let p = db
        .get_user_images_page(&a, &Page::Offset(0), 2, true)
        .await
        .unwrap();
    assert_eq!((p.images.len(), p.total, p.next.is_none()), (2, 3, true));
    let p = db
        .get_user_images_page(&a.to_uppercase(), &Page::Offset(0), 2, false)
        .await
        .unwrap();
    assert_eq!((p.images.len(), p.total), (2, 4));
    let p = db
        .get_user_images_page(&a, &Page::Offset(10), 2, true)
        .await
        .unwrap();
    assert_eq!(
        (p.images.len(), p.total),
        (0, 3),
        "past the end still reports the total"
    );
    let p = db
        .get_user_images_page(&a, &Page::Offset(0), 0, false)
        .await
        .unwrap();
    assert_eq!((p.images.len(), p.total), (0, 4));
    let p = db
        .get_user_images_page(&b, &Page::Offset(0), 5, false)
        .await
        .unwrap();
    assert_eq!((p.images.len(), p.total), (0, 0));

    let p = db
        .get_place_images_page(&place, &Page::Offset(0), 10)
        .await
        .unwrap();
    assert_eq!((p.images.len(), p.total), (2, 2));
    let p = db
        .get_place_images_page(&place, &Page::Offset(1), 10)
        .await
        .unwrap();
    assert_eq!((p.images.len(), p.total), (1, 2));
    let p = db
        .get_multiple_places_images_page(std::slice::from_ref(&place), &Page::Offset(0), 1)
        .await
        .unwrap();
    assert_eq!((p.images.len(), p.total), (1, 2));
    assert!(
        db.get_place_images_page("not-a-uuid", &Page::Offset(0), 10)
            .await
            .is_err()
    );
    s.done().await;
}

#[tokio::test]
async fn keyset_pages_walk_the_offset_order_and_end_with_none() {
    let Some(mut s) = scratch(Settings::new(API_URL)).await else {
        return;
    };
    let a = s.owner(&random_address());
    let t = tag();
    for i in 0..5 {
        s.db()
            .insert_image(&image(
                &a,
                &format!("h{i}-{t}"),
                &format!("t{i}-{t}"),
                true,
                "",
            ))
            .await
            .unwrap();
    }
    let db = s.db();
    let by_offset: Vec<Uuid> = db
        .get_user_images_page(&a, &Page::Offset(0), 10, true)
        .await
        .unwrap()
        .images
        .iter()
        .map(|i| i.id)
        .collect();
    assert_eq!(by_offset.len(), 5);

    let mut walked = Vec::new();
    let mut page = Page::After(None);
    let mut pages = 0;
    loop {
        let p = db.get_user_images_page(&a, &page, 2, true).await.unwrap();
        assert_eq!(
            p.total, 5,
            "every keyset page carries the whole set's total"
        );
        walked.extend(p.images.iter().map(|i| i.id));
        pages += 1;
        match p
            .next
            .expect("a keyset page always says whether there is a next")
        {
            Some(cursor) => page = Page::After(Some(cursor)),
            None => break,
        }
    }
    assert_eq!(walked, by_offset, "the keyset walk is the offset order");
    assert_eq!(pages, 3, "2 + 2 + 1: the short last page ends the walk");
    let p = db
        .get_user_images_page(&a, &Page::After(None), 0, true)
        .await
        .unwrap();
    assert_eq!(
        (p.images.len(), p.next),
        (0, Some(None)),
        "limit 0 is an empty last page"
    );
    s.done().await;
}

#[tokio::test]
async fn servable_memo_is_dropped_by_review_and_delete() {
    let Some(mut s) = scratch(Settings::new(API_URL)).await else {
        return;
    };
    let a = s.owner(&random_address());
    let t = tag();
    let (hx, tx) = (s.hash(&format!("hx-{t}")), s.hash(&format!("tx-{t}")));
    let a1 = image(&a, &hx, &tx, true, "");
    s.db().insert_image(&a1).await.unwrap();
    let db = s.db();
    let id = a1.id.to_string();
    let now = chrono::Utc::now().naive_utc();
    let both = || vec![hx.clone(), tx.clone()];

    assert!(db.hash_is_servable(&hx).await.unwrap());
    assert!(db.hash_is_servable(&tx).await.unwrap());
    let verdict = db
        .update_image_review_status(&id, "rejected", now)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        verdict,
        BlobVerdict {
            withheld: both(),
            released: Vec::new()
        }
    );
    assert!(
        !db.hash_is_servable(&hx).await.unwrap(),
        "rejection must not be masked by the memo"
    );
    assert!(!db.hash_is_servable(&tx).await.unwrap());
    assert_eq!(s.withheld(&hx).await.as_deref(), Some(WITHHELD_REJECTED));
    let verdict = db
        .update_image_review_status(&id, "ok", now)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        verdict,
        BlobVerdict {
            withheld: Vec::new(),
            released: both()
        }
    );
    assert!(db.hash_is_servable(&hx).await.unwrap());
    assert_eq!(s.withheld(&hx).await, None);
    assert!(
        db.update_image_review_status(&Uuid::new_v4().to_string(), "ok", now)
            .await
            .unwrap()
            .is_none()
    );
    let flagged = db
        .update_image_review_status(&id, "flagged", now)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        flagged,
        BlobVerdict::default(),
        "flagged is served; nothing to release"
    );

    let verdict = db
        .update_image_review_status(&id, "rejected", now)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(verdict.withheld, both());
    assert!(!db.hash_is_servable(&hx).await.unwrap());
    let d = db
        .delete_image_owned(&id, None, now)
        .await
        .unwrap()
        .unwrap();
    assert!(d.deleted);
    assert!(
        d.withheld.is_empty(),
        "already withheld by the rejection; the row stays"
    );
    assert_eq!(s.withheld(&hx).await.as_deref(), Some(WITHHELD_REJECTED));
    assert!(
        !db.hash_is_servable(&hx).await.unwrap(),
        "no referencing row: NOT servable (the old orphan rule is gone over the shared store)"
    );
    assert!(!db.hash_is_servable(&format!("never-{t}")).await.unwrap());
    s.done().await;
}

#[tokio::test]
async fn explorer_boot_corpus_shapes_answer_for_an_unknown_wallet_and_place() {
    let Some(s) = scratch(Settings::new(API_URL)).await else {
        return;
    };
    let address = random_address();
    // row 34: GET /camera-reel-service/api/users/{address}
    let (status, _, body) = send(
        s.app(),
        "GET",
        &format!("{PREFIX}/api/users/{address}"),
        vec![],
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        json_of(&body),
        json!({ "currentImages": 0, "maxImages": 500 })
    );
    // row 2: GET /camera-reel-service/api/places/{uuid}/images
    let (status, _, body) = send(
        s.app(),
        "GET",
        &format!("{PREFIX}/api/places/{}/images", Uuid::new_v4()),
        vec![],
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json_of(&body), json!({ "images": [], "maxImages": 0 }));
    let (status, _, body) = send(
        s.app(),
        "GET",
        &format!("{PREFIX}/api/places/not-a-uuid/images"),
        vec![],
        None,
    )
    .await;
    assert_eq!(
        (status, json_of(&body)),
        (
            StatusCode::NOT_FOUND,
            json!({ "message": "place not found" })
        )
    );
    let (status, _, body) = send(
        s.app(),
        "GET",
        &format!("{PREFIX}/api/users/{address}/images?compact=true"),
        vec![],
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        json_of(&body),
        json!({ "images": [], "currentImages": 0, "maxImages": 500 }),
        "no nextCursor on an offset page"
    );
    let (status, _, body) = send(s.app(), "GET", &format!("{PREFIX}/health"), vec![], None).await;
    assert_eq!((status, json_of(&body)), (StatusCode::OK, json!("alive")));
    let (status, _, body) = send(
        s.app(),
        "GET",
        &format!("{PREFIX}/api/docs/openapi.json"),
        vec![],
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json_of(&body)["info"]["title"], "Camera Reel Service");
    s.done().await;
}

#[tokio::test]
async fn writes_disabled_gates_uploads_and_moderation_but_not_the_batch_gallery() {
    let Some(s) = scratch(Settings::new(API_URL)).await else {
        return;
    };
    let w = wallet();
    let path = format!("{PREFIX}/api/images");
    let (body, ct) = multipart(
        &png(8, 8, 1),
        "image/png",
        &metadata_json(&w.address, ""),
        None,
    );
    let headers = signed_headers(&w, "post", &path, true).await;
    let (status, resp_headers, body) =
        send(s.app(), "POST", &path, headers, Some((body, &ct))).await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(resp_headers.get(header::RETRY_AFTER).unwrap(), "3600");
    assert_eq!(key_set(&json_of(&body)), vec!["message"]);
    let (status, _, _) = send(
        s.app(),
        "PATCH",
        &format!("{PREFIX}/admin/images/{}/review", Uuid::new_v4()),
        vec![],
        Some((b"{}".to_vec(), "application/json")),
    )
    .await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    let batch = json!({ "placesIds": [Uuid::new_v4().to_string()] })
        .to_string()
        .into_bytes();
    let (status, _, body) = send(
        s.app(),
        "POST",
        &format!("{PREFIX}/api/places/images"),
        vec![],
        Some((batch, "application/json")),
    )
    .await;
    assert_eq!(
        (status, json_of(&body)),
        (StatusCode::OK, json!({ "images": [], "maxImages": 0 })),
        "the batch gallery is a read"
    );
    s.done().await;
}

#[tokio::test]
async fn upload_over_signed_fetch_is_served_back_and_owned() {
    let mut settings = Settings::new(API_URL);
    settings.writes_enabled = true;
    let Some(mut s) = scratch(settings).await else {
        return;
    };
    let w = wallet();
    let owner = s.owner(&w.address);
    let stranger = wallet();
    let place = Uuid::new_v4().to_string();
    let bytes = png(1280, 720, 7);
    let upload_path = format!("{PREFIX}/api/images");

    // unsigned: 401; signed for another wallet's metadata: 400; both before any row.
    let (body, ct) = multipart(
        &bytes,
        "image/png",
        &metadata_json(&owner, &place),
        Some(false),
    );
    let (status, _, resp) = send(
        s.app(),
        "POST",
        &upload_path,
        vec![],
        Some((body.clone(), &ct)),
    )
    .await;
    assert_eq!(
        (status, json_of(&resp)),
        (
            StatusCode::UNAUTHORIZED,
            json!({ "message": "Unauthorized" })
        )
    );
    let (other, other_ct) = multipart(
        &bytes,
        "image/png",
        &metadata_json(&stranger.address, &place),
        Some(false),
    );
    let headers = signed_headers(&w, "post", &upload_path, true).await;
    let (status, _, resp) = send(
        s.app(),
        "POST",
        &upload_path,
        headers,
        Some((other, &other_ct)),
    )
    .await;
    assert_eq!(
        (status, json_of(&resp)),
        (
            StatusCode::BAD_REQUEST,
            json!({ "message": "invalid user address" })
        )
    );
    assert_eq!(s.count(&owner).await, 0);

    // the upload, signed over the request path (the gateway shape)
    let headers = signed_headers(&w, "post", &upload_path, true).await;
    let (status, _, resp) = send(s.app(), "POST", &upload_path, headers, Some((body, &ct))).await;
    assert_eq!(status, StatusCode::OK, "{}", String::from_utf8_lossy(&resp));
    let uploaded = json_of(&resp);
    assert_eq!(uploaded["currentImages"], 1);
    assert_eq!(uploaded["maxImages"], 500);
    let image_hash = hash_bytes_v1(&bytes);
    assert_eq!(
        uploaded["image"]["url"],
        format!("{API_URL}/api/images/{image_hash}")
    );
    assert_eq!(uploaded["image"]["isPublic"], false);
    assert_eq!(uploaded["image"]["metadata"]["placeId"], place);
    let image_id = uploaded["image"]["id"].as_str().unwrap().to_string();
    let thumbnail_url = uploaded["image"]["thumbnailUrl"].as_str().unwrap();
    let thumbnail_hash = thumbnail_url.rsplit('/').next().unwrap().to_string();

    // the blobs, from the ONE store, typed by their bytes
    let (status, headers, served) = send(
        s.app(),
        "GET",
        &format!("{PREFIX}/api/images/{image_hash}"),
        vec![],
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(headers.get(header::CONTENT_TYPE).unwrap(), "image/png");
    assert_eq!(served, bytes);
    let (status, _, thumb) = send(
        s.app(),
        "GET",
        &format!("{PREFIX}/api/images/{thumbnail_hash}"),
        vec![],
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let decoded = image::load_from_memory(&thumb).unwrap();
    assert_eq!((decoded.width(), decoded.height()), (640, 360));
    assert!(s.state.storage.exist(&image_hash).await.unwrap());

    // private: the owner sees it (signed over the request path, or the vhost's local path behind the proxy header), nobody else does
    let gallery = format!("{PREFIX}/api/users/{owner}/images");
    let (status, _, resp) = send(s.app(), "GET", &gallery, vec![], None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        json_of(&resp)["currentImages"],
        0,
        "anonymous reads the public set"
    );
    let headers = signed_headers(&w, "get", &gallery, true).await;
    let (_, _, resp) = send(s.app(), "GET", &gallery, headers, None).await;
    assert_eq!(json_of(&resp)["images"][0]["id"], image_id);
    let headers = signed_headers(&w, "get", &gallery, false).await;
    let (_, _, resp) = send(s.app(), "GET", &gallery, headers, None).await;
    assert_eq!(
        json_of(&resp)["currentImages"],
        1,
        "the legacy payload over the request path verifies too"
    );
    let local = format!("/api/users/{owner}/images");
    let headers = signed_headers(&w, "get", &local, true).await;
    let (_, _, resp) = send(s.app(), "GET", &gallery, headers.clone(), None).await;
    assert_eq!(
        json_of(&resp)["currentImages"],
        0,
        "the route-local path alone reads as anonymous"
    );
    let mut forwarded = headers;
    forwarded.push(("x-original-path".to_string(), local));
    let (_, _, resp) = send(s.app(), "GET", &gallery, forwarded, None).await;
    assert_eq!(
        json_of(&resp)["currentImages"],
        1,
        "the vhost proxy's x-original-path says the client asked for the local path"
    );
    let headers = signed_headers(&stranger, "get", &gallery, true).await;
    let (_, _, resp) = send(s.app(), "GET", &gallery, headers, None).await;
    assert_eq!(
        json_of(&resp)["currentImages"],
        0,
        "another wallet reads the public set"
    );
    let (status, _, resp) = send(
        s.app(),
        "GET",
        &format!("{PREFIX}/api/places/{place}/images"),
        vec![],
        None,
    )
    .await;
    assert_eq!(
        (status, json_of(&resp)["maxImages"].as_u64()),
        (StatusCode::OK, Some(0)),
        "a private photo is not in the place gallery"
    );

    // visibility: the owner's only
    let vis = format!("{PREFIX}/api/images/{image_id}/visibility");
    let flip = json!({ "is_public": true }).to_string().into_bytes();
    let headers = signed_headers(&stranger, "patch", &vis, true).await;
    let (status, _, _) = send(
        s.app(),
        "PATCH",
        &vis,
        headers,
        Some((flip.clone(), "application/json")),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let headers = signed_headers(&w, "patch", &vis, true).await;
    let (status, _, _) = send(
        s.app(),
        "PATCH",
        &vis,
        headers,
        Some((flip, "application/json")),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (_, _, resp) = send(
        s.app(),
        "GET",
        &format!("{PREFIX}/api/places/{place}/images?cursor="),
        vec![],
        None,
    )
    .await;
    let page = json_of(&resp);
    assert_eq!(page["maxImages"], 1);
    assert_eq!(page["images"][0]["id"], image_id);
    assert_eq!(
        page["nextCursor"],
        Value::Null,
        "a keyset page says it is the last one"
    );
    let batch = json!({ "placesIds": [place, "nobody.dcl.eth"] })
        .to_string()
        .into_bytes();
    let (_, _, resp) = send(
        s.app(),
        "POST",
        &format!("{PREFIX}/api/places/images"),
        vec![],
        Some((batch, "application/json")),
    )
    .await;
    assert_eq!(json_of(&resp)["images"][0]["placeId"], place);

    // delete: the owner's only; the row goes, the blob stays but is no longer servable
    let del = format!("{PREFIX}/api/images/{image_id}");
    let headers = signed_headers(&stranger, "delete", &del, true).await;
    let (status, _, _) = send(s.app(), "DELETE", &del, headers, None).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let headers = signed_headers(&w, "delete", &del, true).await;
    let (status, _, resp) = send(s.app(), "DELETE", &del, headers, None).await;
    assert_eq!(
        (status, json_of(&resp)),
        (
            StatusCode::OK,
            json!({ "currentImages": 0, "maxImages": 500 })
        )
    );
    let (status, _, _) = send(
        s.app(),
        "GET",
        &format!("{PREFIX}/api/images/{image_hash}"),
        vec![],
        None,
    )
    .await;
    assert_eq!(
        status,
        StatusCode::NOT_FOUND,
        "an unreferenced hash is not servable"
    );
    assert!(
        s.state.storage.exist(&image_hash).await.unwrap(),
        "the shared store is never unlinked"
    );
    let headers = signed_headers(&w, "delete", &del, true).await;
    let (status, _, _) = send(s.app(), "DELETE", &del, headers, None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    s.done().await;
}

#[tokio::test]
async fn json_upload_and_the_cap_answer_the_upstream_envelopes() {
    let mut settings = Settings::new(API_URL);
    settings.writes_enabled = true;
    settings.max_images_per_user = 1;
    let Some(mut s) = scratch(settings).await else {
        return;
    };
    let w = wallet();
    let owner = s.owner(&w.address);
    let path = format!("{PREFIX}/api/images-json");
    let upload = |seed: u8| {
        use base64::Engine as _;
        json!({
            "image": base64::engine::general_purpose::STANDARD.encode(png(16, 9, seed)),
            "content_type": "image/png",
            "metadata": metadata_json(&owner, ""),
            "is_public": true
        })
        .to_string()
        .into_bytes()
    };
    let headers = signed_headers(&w, "post", &path, true).await;
    let (status, _, resp) = send(
        s.app(),
        "POST",
        &path,
        headers,
        Some((upload(3), "application/json")),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{}", String::from_utf8_lossy(&resp));
    assert_eq!(json_of(&resp)["currentImages"], 1);
    let headers = signed_headers(&w, "post", &path, true).await;
    let (status, _, resp) = send(
        s.app(),
        "POST",
        &path,
        headers,
        Some((upload(4), "application/json")),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(
        json_of(&resp),
        json!({ "reason": "maxLimitReached", "message": "you have reached the limit of 1 max images" })
    );
    assert_eq!(s.count(&owner).await, 1, "a refused upload leaves no row");
    let headers = signed_headers(&w, "post", &path, true).await;
    let bad = json!({ "image": "not base64!", "content_type": "image/png", "metadata": metadata_json(&owner, "") }).to_string().into_bytes();
    let (status, _, resp) = send(
        s.app(),
        "POST",
        &path,
        headers,
        Some((bad, "application/json")),
    )
    .await;
    assert_eq!(
        (status, json_of(&resp)),
        (
            StatusCode::BAD_REQUEST,
            json!({ "message": "invalid base64 image" })
        )
    );
    s.done().await;
}

#[tokio::test]
async fn the_moderator_hides_a_rejected_row_and_its_blobs() {
    let mut settings = Settings::new(API_URL);
    settings.writes_enabled = true;
    settings.admin_token = Some("moderator-secret".to_string());
    let Some(mut s) = scratch(settings).await else {
        return;
    };
    let owner = s.owner(&random_address());
    let bytes = png(32, 32, 9);
    let hash = s.hash(&hash_bytes_v1(&bytes));
    s.state
        .storage
        .store(&hash, bytes.clone().into())
        .await
        .unwrap();
    let img = image(&owner, &hash, &hash, true, "");
    s.db().insert_image(&img).await.unwrap();
    let review = format!("{PREFIX}/admin/images/{}/review", img.id);
    let reject = json!({ "reviewStatus": "rejected" })
        .to_string()
        .into_bytes();
    let bearer = |token: &str| vec![("authorization".to_string(), format!("Bearer {token}"))];

    let (status, _, _) = send(
        s.app(),
        "PATCH",
        &review,
        vec![],
        Some((reject.clone(), "application/json")),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let (status, _, _) = send(
        s.app(),
        "PATCH",
        &review,
        bearer("wrong"),
        Some((reject.clone(), "application/json")),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let (status, _, served) = send(
        s.app(),
        "GET",
        &format!("{PREFIX}/api/images/{hash}"),
        vec![],
        None,
    )
    .await;
    assert_eq!((status, served == bytes), (StatusCode::OK, true));

    let bad = json!({ "reviewStatus": "meh" }).to_string().into_bytes();
    let (status, _, resp) = send(
        s.app(),
        "PATCH",
        &review,
        bearer("moderator-secret"),
        Some((bad, "application/json")),
    )
    .await;
    assert_eq!(
        (status, json_of(&resp)),
        (
            StatusCode::BAD_REQUEST,
            json!({ "message": "reviewStatus must be one of: ok, flagged, rejected" })
        )
    );
    let (status, _, _) = send(
        s.app(),
        "PATCH",
        &review,
        bearer("moderator-secret"),
        Some((reject.clone(), "application/json")),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (status, _, _) = send(
        s.app(),
        "GET",
        &format!("{PREFIX}/api/images/{hash}"),
        vec![],
        None,
    )
    .await;
    assert_eq!(
        status,
        StatusCode::NOT_FOUND,
        "a rejected row's blob is withheld"
    );
    let (status, _, _) = send(
        s.app(),
        "GET",
        &format!("{PREFIX}/api/images/{}/metadata", img.id),
        vec![],
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _, resp) = send(
        s.app(),
        "GET",
        &format!("{PREFIX}/api/users/{owner}"),
        vec![],
        None,
    )
    .await;
    assert_eq!(
        (status, json_of(&resp)["currentImages"].as_u64()),
        (StatusCode::OK, Some(0))
    );
    let (status, _, _) = send(
        s.app(),
        "PATCH",
        &review,
        bearer("moderator-secret"),
        Some((
            json!({ "reviewStatus": "ok" }).to_string().into_bytes(),
            "application/json",
        )),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (status, _, _) = send(
        s.app(),
        "GET",
        &format!("{PREFIX}/api/images/{hash}"),
        vec![],
        None,
    )
    .await;
    assert_eq!(
        status,
        StatusCode::OK,
        "the memo was dropped by the review write"
    );

    let (status, _, resp) = send(
        s.app(),
        "DELETE",
        &format!("{PREFIX}/admin/images/{}", img.id),
        bearer("moderator-secret"),
        None,
    )
    .await;
    assert_eq!(
        (status, json_of(&resp)),
        (
            StatusCode::OK,
            json!({ "currentImages": 0, "maxImages": 500 })
        )
    );
    let (status, _, _) = send(
        s.app(),
        "PATCH",
        &review,
        bearer("moderator-secret"),
        Some((reject, "application/json")),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    s.done().await;
}

/// The node as `serve` composes it around the ONE store: the content
/// server's `/contents/{hash}`, the builder's under `/builder-api` and the
/// reel under its prefix, the builder and reel states holding the content
/// state's denylist (`main.rs`).
async fn node_around(s: &Scratch) -> Router {
    let pool = s.db().pool().clone();
    let storage = s.state.storage.clone();
    let mut content = crate::content::ContentState::new(
        pool.clone(),
        crate::content::Settings::for_tests(),
        storage.clone(),
    );
    content.denylist = s.state.denylist.clone();
    let builder = Arc::new(
        crate::builder::State::new(
            pool,
            storage,
            s.state.denylist.clone(),
            dcl_http::Outbound::for_tests(),
            None,
            None,
            crate::builder::Settings::default(),
        )
        .unwrap(),
    );
    Router::new()
        .merge(crate::content::build_router(Arc::new(content)))
        .nest("/builder-api", crate::builder::router(builder))
        .nest(PREFIX, router(s.state.clone()))
}

/// The three routes that can hand out a photo's bytes, as `(status, body)`.
async fn bytes_everywhere(app: &Router, hash: &str) -> [(StatusCode, bool); 3] {
    let mut out = [(StatusCode::IM_A_TEAPOT, false); 3];
    for (i, path) in [
        format!("{PREFIX}/api/images/{hash}"),
        format!("/contents/{hash}"),
        format!("/builder-api/contents/{hash}"),
    ]
    .iter()
    .enumerate()
    {
        let (status, _, body) = send(app.clone(), "GET", path, vec![], None).await;
        out[i] = (status, !body.is_empty() && status == StatusCode::OK);
    }
    out
}

#[tokio::test]
async fn a_rejected_or_deleted_photo_is_withheld_on_every_contents_route() {
    let mut settings = Settings::new(API_URL);
    settings.writes_enabled = true;
    settings.admin_token = Some("moderator-secret".to_string());
    let Some(mut s) = scratch(settings.clone()).await else {
        return;
    };
    let app = node_around(&s).await;
    let w = wallet();
    let owner = s.owner(&w.address);
    let bytes = png(48, 48, 11);
    let upload_path = format!("{PREFIX}/api/images");
    let (body, ct) = multipart(&bytes, "image/png", &metadata_json(&owner, ""), Some(false));
    let headers = signed_headers(&w, "post", &upload_path, true).await;
    let (status, _, resp) = send(
        app.clone(),
        "POST",
        &upload_path,
        headers,
        Some((body, &ct)),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{}", String::from_utf8_lossy(&resp));
    let uploaded = json_of(&resp);
    let id = uploaded["image"]["id"].as_str().unwrap().to_string();
    let image_hash = s.hash(&hash_bytes_v1(&bytes));
    let thumb_hash = s.hash(
        uploaded["image"]["thumbnailUrl"]
            .as_str()
            .unwrap()
            .rsplit('/')
            .next()
            .unwrap(),
    );
    assert_ne!(image_hash, thumb_hash);
    let served = [(StatusCode::OK, true); 3];
    let withheld = [(StatusCode::NOT_FOUND, false); 3];

    // private and not rejected: hash-as-capability on every route, as the old reel route
    assert_eq!(bytes_everywhere(&app, &image_hash).await, served);
    assert_eq!(bytes_everywhere(&app, &thumb_hash).await, served);
    let (_, _, root) = send(
        app.clone(),
        "GET",
        &format!("/contents/{image_hash}"),
        vec![],
        None,
    )
    .await;
    assert_eq!(root, bytes, "the content server serves the photo's bytes");

    // rejected: 404 on all three, image and thumbnail, and the record is durable
    let review = format!("{PREFIX}/admin/images/{id}/review");
    let bearer = vec![(
        "authorization".to_string(),
        "Bearer moderator-secret".to_string(),
    )];
    let verdict = |status: &str| {
        Some((
            json!({ "reviewStatus": status }).to_string().into_bytes(),
            "application/json",
        ))
    };
    let (status, _, _) = send(
        app.clone(),
        "PATCH",
        &review,
        bearer.clone(),
        verdict("rejected"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(bytes_everywhere(&app, &image_hash).await, withheld);
    assert_eq!(bytes_everywhere(&app, &thumb_hash).await, withheld);
    assert_eq!(
        s.withheld(&image_hash).await.as_deref(),
        Some(WITHHELD_REJECTED)
    );
    assert_eq!(
        s.withheld(&thumb_hash).await.as_deref(),
        Some(WITHHELD_REJECTED)
    );
    // a restart is a new process: a fresh store over the same root, its projection empty until the load
    let restarted_store = Arc::new(
        Storage::from_config(&s.dir)
            .await
            .expect("the same blob root"),
    );
    let restarted = State::new(s.db().pool().clone(), restarted_store, settings, None);
    assert!(!restarted.denylist.contains(&image_hash));
    assert!(
        restarted.load_withheld().await.unwrap() >= 2,
        "a restart reloads the withheld set"
    );
    assert!(restarted.denylist.contains(&image_hash) && restarted.denylist.contains(&thumb_hash));

    // set back to ok: served again everywhere, the record released
    let (status, _, _) = send(app.clone(), "PATCH", &review, bearer.clone(), verdict("ok")).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(bytes_everywhere(&app, &image_hash).await, served);
    assert_eq!(bytes_everywhere(&app, &thumb_hash).await, served);
    assert_eq!(s.withheld(&image_hash).await, None);

    // the same bytes accepted as a builder upload: a rejection withholds only the thumbnail
    sqlx::query(
        "INSERT INTO builder.uploaded_contents (hash, size, uploaded_by) VALUES ($1, $2, $3)",
    )
    .bind(&image_hash)
    .bind(bytes.len() as i64)
    .bind(owner.to_lowercase())
    .execute(s.db().pool())
    .await
    .unwrap();
    let (status, _, _) = send(
        app.clone(),
        "PATCH",
        &review,
        bearer.clone(),
        verdict("rejected"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        bytes_everywhere(&app, &image_hash).await,
        [
            (StatusCode::NOT_FOUND, false),
            (StatusCode::OK, true),
            (StatusCode::OK, true)
        ],
        "a deployed file of the same bytes stays served by the content routes"
    );
    assert_eq!(bytes_everywhere(&app, &thumb_hash).await, withheld);
    sqlx::query("DELETE FROM builder.uploaded_contents WHERE hash = $1")
        .bind(&image_hash)
        .execute(s.db().pool())
        .await
        .unwrap();
    let (status, _, _) = send(app.clone(), "PATCH", &review, bearer, verdict("ok")).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(bytes_everywhere(&app, &image_hash).await, served);

    // the owner's delete: the row is gone and the bytes are withheld everywhere
    let del_path = format!("{PREFIX}/api/images/{id}");
    let headers = signed_headers(&w, "delete", &del_path, true).await;
    let (status, _, _) = send(app.clone(), "DELETE", &del_path, headers, None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(bytes_everywhere(&app, &image_hash).await, withheld);
    assert_eq!(bytes_everywhere(&app, &thumb_hash).await, withheld);
    assert_eq!(
        s.withheld(&image_hash).await.as_deref(),
        Some(WITHHELD_DELETED)
    );
    assert!(
        s.state.storage.exist(&image_hash).await.unwrap(),
        "the blob itself is never unlinked"
    );
    s.done().await;
}

#[tokio::test]
async fn admin_routes_fail_closed_without_a_configured_token() {
    let mut settings = Settings::new(API_URL);
    settings.writes_enabled = true;
    let Some(s) = scratch(settings).await else {
        return;
    };
    let (status, _, resp) = send(
        s.app(),
        "DELETE",
        &format!("{PREFIX}/admin/images/{}", Uuid::new_v4()),
        vec![("authorization".to_string(), "Bearer anything".to_string())],
        None,
    )
    .await;
    assert_eq!(
        (status, json_of(&resp)),
        (
            StatusCode::FORBIDDEN,
            json!({ "message": "admin token not configured" })
        )
    );
    s.done().await;
}

#[tokio::test]
async fn a_world_name_gallery_reads_the_places_catalogue() {
    let Some(mut s) = scratch(Settings::new(API_URL)).await else {
        return;
    };
    let owner = s.owner(&random_address());
    let t = tag();
    let world = format!("cam{}.dcl.eth", &t[..8]);
    let place = Uuid::new_v4().to_string();
    let hidden = Uuid::new_v4().to_string();
    let pool = s.db().pool().clone();
    for (id, disabled) in [(&place, false), (&hidden, true)] {
        sqlx::query("INSERT INTO explore.place (id, base_position, disabled, raw) VALUES ($1, '0,0', $2, $3)")
            .bind(id)
            .bind(disabled)
            .bind(json!({ "world": true, "world_name": world.to_uppercase() }))
            .execute(&pool)
            .await
            .unwrap();
    }
    s.db()
        .insert_image(&image(
            &owner,
            &format!("w1-{t}"),
            &format!("wt1-{t}"),
            true,
            &place,
        ))
        .await
        .unwrap();
    s.db()
        .insert_image(&image(
            &owner,
            &format!("w2-{t}"),
            &format!("wt2-{t}"),
            true,
            &hidden,
        ))
        .await
        .unwrap();

    let (status, _, resp) = send(
        s.app(),
        "GET",
        &format!("{PREFIX}/api/places/{world}/images"),
        vec![],
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let page = json_of(&resp);
    assert_eq!(
        page["maxImages"], 1,
        "the disabled place's photo is not in the world gallery"
    );
    assert_eq!(
        page["images"][0]["url"],
        format!("{API_URL}/api/images/w1-{t}")
    );
    let batch = json!({ "placesIds": [world, hidden] })
        .to_string()
        .into_bytes();
    let (_, _, resp) = send(
        s.app(),
        "POST",
        &format!("{PREFIX}/api/places/images"),
        vec![],
        Some((batch, "application/json")),
    )
    .await;
    assert_eq!(
        json_of(&resp)["maxImages"],
        2,
        "a place id in the batch is taken as given"
    );
    let (status, _, resp) = send(
        s.app(),
        "GET",
        &format!("{PREFIX}/api/places/nobody-{}.dcl.eth/images", &t[..8]),
        vec![],
        None,
    )
    .await;
    assert_eq!(
        (status, json_of(&resp)),
        (StatusCode::OK, json!({ "images": [], "maxImages": 0 }))
    );

    sqlx::query("DELETE FROM explore.place WHERE id = ANY($1)")
        .bind(vec![place, hidden])
        .execute(&pool)
        .await
        .unwrap();
    s.done().await;
}

fn worn(owner: &str, hash: &str, is_public: bool, people: &[&[String]]) -> NewImage {
    let mut new = image(owner, hash, &format!("{hash}-thumb"), is_public, "");
    new.metadata.visible_people = people
        .iter()
        .map(|wearables| User {
            wearables: wearables.to_vec(),
            ..User::default()
        })
        .collect();
    new
}

#[tokio::test]
async fn the_wearable_items_of_a_photo_are_its_collection_items_without_token_ids() {
    let Some(s) = scratch(Settings::new(API_URL)).await else {
        return;
    };
    let items = |doc: Value| {
        let pool = s.db().pool().clone();
        async move {
            let mut got: Vec<String> =
                sqlx::query_scalar("SELECT camera_reel.image_wearable_items($1)")
                    .bind(doc)
                    .fetch_one(&pool)
                    .await
                    .unwrap();
            got.sort();
            got
        }
    };
    let contract = "0x0bf152a83a6fc55066c2b664b164ca2916ad38f5";
    let doc = json!({ "visiblePeople": [
        { "wearables": [
            "urn:decentraland:off-chain:base-avatars:eyes_00",
            format!("urn:decentraland:matic:collections-v2:{}:2:7", contract.to_uppercase()),
            5
        ] },
        { "wearables": [
            format!("urn:decentraland:matic:collections-v2:{contract}:2:105312291668557186697918027683670432318895095400549111254310977559"),
            format!("urn:decentraland:ethereum:collections-v2:{contract}:3")
        ] }
    ] });
    assert_eq!(
        items(doc).await,
        vec![format!("{contract}-2"), format!("{contract}-3")],
        "one entry per item, lowercase"
    );
    for empty in [
        json!({}),
        json!({ "visiblePeople": null }),
        json!({ "visiblePeople": "someone" }),
        json!({ "visiblePeople": [1, "s", { "wearables": "none" }, { "wearables": null }, {}] }),
    ] {
        assert_eq!(items(empty.clone()).await, Vec::<String>::new(), "{empty}");
    }
    s.done().await;
}

#[tokio::test]
async fn the_wearable_gallery_is_the_public_photos_of_an_item_whoever_asks() {
    let Some(mut s) = scratch(Settings::new(API_URL)).await else {
        return;
    };
    let w = wallet();
    let owner = s.owner(&w.address);
    let t = tag();
    let contract = random_address();
    let urn = |item: &str, token: &str| {
        format!("urn:decentraland:matic:collections-v2:{contract}:{item}:{token}")
    };

    let long_token = worn(
        &owner,
        &format!("wa-{t}"),
        true,
        &[&[urn(
            "2",
            "105312291668557186697918027683670432318895095400549111254310977559",
        )]],
    );
    let upper_contract = format!("0x{}", contract[2..].to_uppercase());
    let upper = worn(
        &owner,
        &format!("wb-{t}"),
        true,
        &[&[format!(
            "urn:decentraland:matic:collections-v2:{upper_contract}:2:7"
        )]],
    );
    let two_wearers = worn(
        &owner,
        &format!("wc-{t}"),
        true,
        &[&[urn("2", "8")], &[urn("2", "9"), urn("3", "1")]],
    );
    let other_item = worn(&owner, &format!("wd-{t}"), true, &[&[urn("3", "7")]]);
    let private = worn(&owner, &format!("we-{t}"), false, &[&[urn("2", "10")]]);
    let rejected = worn(
        &owner,
        &s.hash(&format!("wf-{t}")),
        true,
        &[&[urn("2", "11")]],
    );
    s.hash(&format!("wf-{t}-thumb"));
    for new in [
        &long_token,
        &upper,
        &two_wearers,
        &other_item,
        &private,
        &rejected,
    ] {
        s.db().insert_image(new).await.unwrap();
    }
    s.db()
        .update_image_review_status(
            &rejected.id.to_string(),
            "rejected",
            chrono::Utc::now().naive_utc(),
        )
        .await
        .unwrap();

    let expected: std::collections::BTreeSet<String> = [&long_token, &upper, &two_wearers]
        .iter()
        .map(|n| n.id.to_string())
        .collect();
    let ids = |page: &Value| -> Vec<String> {
        page["images"]
            .as_array()
            .unwrap()
            .iter()
            .map(|i| i["id"].as_str().unwrap().to_string())
            .collect()
    };

    let path = format!("{PREFIX}/api/wearables/{upper_contract}-2/images");
    let (status, _, body) = send(s.app(), "GET", &path, vec![], None).await;
    assert_eq!(status, StatusCode::OK);
    let page = json_of(&body);
    assert_eq!(
        key_set(&page),
        vec!["images", "maxImages"],
        "offset-only: no nextCursor, no currentImages"
    );
    assert_eq!(
        page["maxImages"], 3,
        "every token of the item, never the private, rejected or neighbouring one"
    );
    assert_eq!(
        ids(&page)
            .into_iter()
            .collect::<std::collections::BTreeSet<_>>(),
        expected
    );
    let first = &page["images"][0];
    assert_eq!(
        key_set(first),
        vec!["id", "isPublic", "metadata", "thumbnailUrl", "url"],
        "full images, not gallery rows"
    );
    assert!(first["metadata"]["visiblePeople"].is_array());

    let (_, _, body) = send(s.app(), "GET", &format!("{path}?limit=2"), vec![], None).await;
    let head = json_of(&body);
    let (_, _, body) = send(
        s.app(),
        "GET",
        &format!("{path}?limit=2&offset=2"),
        vec![],
        None,
    )
    .await;
    let tail = json_of(&body);
    assert_eq!(
        (head["maxImages"].clone(), tail["maxImages"].clone()),
        (json!(3), json!(3))
    );
    let walked: Vec<String> = ids(&head).into_iter().chain(ids(&tail)).collect();
    assert_eq!(
        walked,
        ids(&page),
        "offset pages walk the newest-first order"
    );

    let signed = signed_headers(&w, "get", &path, true).await;
    let (status, _, body) = send(s.app(), "GET", &path, signed, None).await;
    assert_eq!(
        (status, json_of(&body)["maxImages"].clone()),
        (StatusCode::OK, json!(3)),
        "the owner's own chain does not lift the public filter"
    );

    let (status, _, body) = send(
        s.app(),
        "GET",
        &format!("{PREFIX}/api/wearables/{contract}-4/images"),
        vec![],
        None,
    )
    .await;
    assert_eq!(
        (status, json_of(&body)),
        (StatusCode::OK, json!({ "images": [], "maxImages": 0 }))
    );

    let message = json!({ "message": "the item must be an item id, as `<contract>-<itemId>`" });
    for bad in [
        "not-an-item".to_string(),
        urn("2", "7"),
        contract.clone(),
        format!("{contract}-2-7"),
    ] {
        let (status, _, body) = send(
            s.app(),
            "GET",
            &format!("{PREFIX}/api/wearables/{bad}/images"),
            vec![],
            None,
        )
        .await;
        assert_eq!(
            (status, json_of(&body)),
            (StatusCode::BAD_REQUEST, message.clone()),
            "{bad}"
        );
    }

    let (_, _, body) = send(
        s.app(),
        "GET",
        &format!("{PREFIX}/api/docs/openapi.json"),
        vec![],
        None,
    )
    .await;
    let spec = json_of(&body);
    assert_eq!(
        spec["paths"]["/api/wearables/{item}/images"]["get"]["operationId"],
        "get_wearable_images"
    );
    assert_eq!(
        spec["components"]["schemas"]["GetWearableImagesResponse"]["required"],
        json!(["images", "maxImages"])
    );
    s.done().await;
}
