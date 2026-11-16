//! Cast 2.0 (`/cast/*`) through the gatekeeper router: the behaviours of
//! upstream comms-gatekeeper b42d45c7b `test/integration/cast/*` and
//! `test/unit/cast/*`, against a local LiveKit Twirp fixture (rooms and
//! ingresses in memory) and the scratch database.
use std::collections::HashMap;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use serde_json::{Value, json};

use super::*;
use crate::access::Ban;
use crate::gatekeeper::moderation::test_support::StubBanStatus;
use crate::gatekeeper::streaming::cast::{build_stream_links, is_valid_presenter_identity};
use crate::rooms::{ActiveDeployment, ResolvedRoom};
use crate::world_identity::{Parcel, RealmId, SceneId, WorldName};

const SCENE_SIGNER: &str = crate::gatekeeper::auth::SCENE_SIGNER;

#[derive(Default)]
struct LiveKitFixture {
    rooms: Mutex<HashMap<String, String>>,
    ingresses: AtomicUsize,
    deleted_ingresses: AtomicUsize,
}

impl LiveKitFixture {
    fn metadata(&self, room: &str) -> Value {
        let rooms = self.rooms.lock().unwrap();
        rooms
            .get(room)
            .map(|m| serde_json::from_str(m).unwrap_or(Value::Null))
            .unwrap_or(Value::Null)
    }

    fn set_metadata(&self, room: &str, metadata: Value) {
        self.rooms
            .lock()
            .unwrap()
            .insert(room.to_owned(), metadata.to_string());
    }

    async fn serve(self: &Arc<Self>) -> (String, tokio::task::JoinHandle<()>) {
        let (a, b, c, d, e) = (
            self.clone(),
            self.clone(),
            self.clone(),
            self.clone(),
            self.clone(),
        );
        let app = axum::Router::new()
            .route(
                "/twirp/livekit.Ingress/CreateIngress",
                axum::routing::post(move || {
                    let n = a.ingresses.fetch_add(1, Ordering::SeqCst);
                    async move {
                        Json(json!({
                            "ingressId": format!("IN_cast_{n}_{}", tag()),
                            "url": "rtmp://ingress.example/live",
                            "streamKey": format!("cast-key-{}", Uuid::new_v4()),
                        }))
                    }
                }),
            )
            .route(
                "/twirp/livekit.Ingress/DeleteIngress",
                axum::routing::post(move || {
                    b.deleted_ingresses.fetch_add(1, Ordering::SeqCst);
                    async { Json(json!({})) }
                }),
            )
            .route(
                "/twirp/livekit.RoomService/ListRooms",
                axum::routing::post(move |Json(body): Json<Value>| {
                    let rooms = c.rooms.lock().unwrap().clone();
                    async move {
                        let names: Vec<String> = body["names"]
                            .as_array()
                            .map(|n| n.iter().filter_map(|v| v.as_str().map(str::to_owned)).collect())
                            .unwrap_or_default();
                        let listed: Vec<Value> = rooms
                            .iter()
                            .filter(|(name, _)| names.is_empty() || names.contains(name))
                            .map(|(name, metadata)| json!({"name": name, "sid": "RM_cast", "metadata": metadata}))
                            .collect();
                        Json(json!({ "rooms": listed }))
                    }
                }),
            )
            .route(
                "/twirp/livekit.RoomService/CreateRoom",
                axum::routing::post(move |Json(body): Json<Value>| {
                    let name = body["name"].as_str().unwrap().to_owned();
                    d.rooms.lock().unwrap().entry(name.clone()).or_default();
                    async move { Json(json!({"name": name, "sid": "RM_cast"})) }
                }),
            )
            .route(
                "/twirp/livekit.RoomService/UpdateRoomMetadata",
                axum::routing::post(move |Json(body): Json<Value>| {
                    let name = body["room"].as_str().unwrap().to_owned();
                    let metadata = body["metadata"].as_str().unwrap().to_owned();
                    e.rooms
                        .lock()
                        .unwrap()
                        .insert(name.clone(), metadata.clone());
                    async move { Json(json!({"name": name, "sid": "RM_cast", "metadata": metadata})) }
                }),
            );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        (url, server)
    }
}

/// A Genesis scene on a parcel no other test of this process uses.
struct Scene {
    id: String,
    parcel: String,
    room: String,
    place_id: String,
}

fn unique_parcel() -> String {
    let n = Uuid::new_v4().as_u128();
    format!(
        "{},{}",
        10_000 + (n % 50_000),
        10_000 + ((n >> 32) % 50_000)
    )
}

fn genesis_scene() -> Scene {
    let id = format!("bafkreicast{}", tag());
    let parcel = unique_parcel();
    Scene {
        room: format!("scene:main:{id}"),
        place_id: format!("cast-place-{}", tag()),
        id,
        parcel,
    }
}

fn resolved_room(realm: RealmId, scene: &str, parcel: &str) -> ResolvedRoom {
    let scene = SceneId::parse(scene).unwrap();
    let parcel = Parcel::parse(parcel).unwrap();
    let room = match &realm {
        RealmId::World(world) => crate::rooms::worlds_service_scene_room(world, &scene),
        RealmId::Shared(name) => crate::rooms::gatekeeper_scene_room(name, scene.as_str()),
    };
    ResolvedRoom {
        room,
        scene_id: Some(scene.clone()),
        deployment: Some(ActiveDeployment {
            title: None,
            scene_id: scene,
            base: parcel,
            parcels: [parcel].into_iter().collect(),
            namespace: crate::world_identity::SceneNamespace::new(&realm, parcel, None),
            local: true,
        }),
        realm,
    }
}

struct Harness {
    state: SharedState,
    app: axum::Router,
    livekit: Arc<LiveKitFixture>,
    server: tokio::task::JoinHandle<()>,
}

impl Drop for Harness {
    fn drop(&mut self) {
        self.server.abort();
    }
}

async fn harness(
    pool: PgPool,
    room: Option<ResolvedRoom>,
    admin: PermissionDecision,
    bans: Option<StubBanStatus>,
) -> Harness {
    let livekit = Arc::new(LiveKitFixture::default());
    let (url, server) = livekit.serve().await;
    let mut settings = Settings::for_tests();
    settings.cast2_url = "https://cast.example".into();
    let mut state = comms_state(pool.clone(), settings);
    if let Some(bans) = bans {
        state = state.with_ban_status(Arc::new(bans));
    }
    if let Some(room) = room {
        state.scenes = Arc::new(StubScenes {
            room: Ok(room),
            scene: Ok(None),
        });
    }
    state.policy = Arc::new(StubPolicy {
        admin,
        ..Default::default()
    });
    state.livekit.api_url = url;
    let state = Arc::new(state);
    Harness {
        app: crate::gatekeeper::router(state.clone()),
        state,
        livekit,
        server,
    }
}

impl Harness {
    async fn send(
        &self,
        method: &str,
        path: &str,
        headers: HeaderMap,
        body: Option<Value>,
    ) -> (StatusCode, Value) {
        let mut request = Request::builder().method(method).uri(path);
        *request.headers_mut().unwrap() = headers;
        let request = match body {
            Some(body) => request
                .header("content-type", "application/json")
                .body(Body::from(body.to_string())),
            None => request.body(Body::empty()),
        }
        .unwrap();
        body_json(self.app.clone().oneshot(request).await.unwrap()).await
    }

    async fn post(&self, path: &str, body: Value) -> (StatusCode, Value) {
        self.send("POST", path, HeaderMap::new(), Some(body)).await
    }

    async fn signed(
        &self,
        key: &str,
        method: &str,
        path: &str,
        metadata: &Value,
        body: Option<Value>,
    ) -> (StatusCode, Value) {
        let headers = signed(key, &method.to_lowercase(), path, &metadata.to_string()).await;
        self.send(method, path, headers, body).await
    }
}

async fn seed_genesis_place(pool: &PgPool, scene: &Scene, title: Option<&str>) {
    seed_place(
        pool,
        &scene.place_id,
        json!({ "positions": [scene.parcel] }),
        &scene.parcel,
    )
    .await;
    sqlx::query("UPDATE explore.place SET title = $2 WHERE id = $1")
        .bind(&scene.place_id)
        .bind(title)
        .execute(pool)
        .await
        .unwrap();
}

/// A stream credential row as a stream link stores it.
async fn seed_access(
    pool: &PgPool,
    place_id: &str,
    room: Option<&str>,
    expires_in: chrono::Duration,
) -> String {
    let key = format!("cast-seeded-{}", Uuid::new_v4());
    let now = chrono::Utc::now().naive_utc();
    sqlx::query(
        "INSERT INTO social.scene_stream_access \
         (place_id, streaming_url, streaming_key, ingress_id, room_id, generated_by, created_at, expiration_time, active) \
         VALUES ($1, 'rtmp://ingress.example/live', $2, $3, $4, $5, $6, $7, true)",
    )
    .bind(place_id)
    .bind(&key)
    .bind(format!("IN_seeded_{}", tag()))
    .bind(room)
    .bind(address_of(KEY))
    .bind(now - chrono::Duration::hours(1))
    .bind(now + expires_in)
    .execute(pool)
    .await
    .unwrap();
    key
}

fn scene_metadata(scene: &Scene) -> Value {
    json!({
        "signer": SCENE_SIGNER,
        "sceneId": scene.id,
        "parcel": scene.parcel,
        "realm": { "serverName": "main", "hostname": "peer.example", "protocol": "v3" },
    })
}

fn viewer_metadata() -> Value {
    json!({ "signer": "dcl:explorer" })
}

fn claims(token: &Value) -> Value {
    crate::gatekeeper::livekit::decode_jwt_payload(token.as_str().unwrap())
}

// ── pure behaviours ──

#[test]
fn stream_links_name_the_cast_app_with_upstreams_default() {
    assert_eq!(
        build_stream_links("", "k1", "20,-4"),
        (
            "https://cast2.decentraland.org/s/k1".to_owned(),
            "https://cast2.decentraland.org/w/20,-4".to_owned()
        )
    );
    assert_eq!(
        build_stream_links("https://cast.example", "k2", "foo.dcl.eth"),
        (
            "https://cast.example/s/k2".to_owned(),
            "https://cast.example/w/foo.dcl.eth".to_owned()
        )
    );
}

#[test]
fn presenter_identities_are_wallets_or_minted_streamer_identities() {
    let uuid = "123e4567-e89b-42d3-a456-426614174000";
    for ok in [
        "0x1234567890abcdef1234567890ABCDEF12345678".to_owned(),
        format!("stream:place-1:{uuid}"),
        // A preview place id is the room id, colons included.
        format!("stream:scene:LocalPreview:bafkrei/+=._-:{uuid}"),
    ] {
        assert!(is_valid_presenter_identity(&ok), "{ok}");
    }
    for refused in [
        format!("watch:scene:main:x:{uuid}"),
        format!("presentation-bot:scene:main:x:{uuid}"),
        format!("stream::{uuid}"),
        format!("stream:place:{}", uuid.to_uppercase()),
        "stream:place:not-a-uuid".to_owned(),
        "0x123".to_owned(),
        "not-an-identity".to_owned(),
        format!("stream:{}:{uuid}", "a".repeat(500)),
    ] {
        assert!(!is_valid_presenter_identity(&refused), "{refused}");
    }
}

#[tokio::test]
async fn bodies_are_refused_as_upstreams_schema_validator_refuses_them() {
    let h = harness(lazy_pool(), None, PermissionDecision::Allowed, None).await;
    let (status, body) = h.post("/cast/streamer-token", json!({})).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(
        body,
        json!({"ok": false, "message": "Invalid JSON body", "data": [{
            "instancePath": "", "schemaPath": "#/required", "keyword": "required",
            "params": {"missingProperty": "token"}, "message": "must have required property 'token'"
        }]})
    );
    let (status, body) = h.post("/cast/streamer-token", json!({"token": "k"})).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["data"][0]["params"]["missingProperty"], "identity");
    for identity in ["", "   ", "\u{3000}x", "a\nb"] {
        let (status, body) = h
            .post(
                "/cast/streamer-token",
                json!({"token": "k", "identity": identity}),
            )
            .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{identity:?}");
        assert_eq!(
            body["data"][0],
            json!({
                "instancePath": "/identity", "schemaPath": "#/properties/identity/pattern",
                "keyword": "pattern", "params": {"pattern": r"^\S+.*$"},
                "message": r#"must match pattern "^\S+.*$""#
            })
        );
    }
    let (status, body) = h
        .post(
            "/cast/streamer-token",
            json!({"token": "k".repeat(257), "identity": "fox"}),
        )
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["data"][0]["keyword"], "maxLength");
    assert_eq!(
        body["data"][0]["message"],
        "must NOT have more than 256 characters"
    );
    let (status, body) = h
        .post("/cast/presentation-bot-token", json!({"streamingKey": 7}))
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["data"][0]["message"], "must be string");
    let (status, _) = h
        .post("/cast/presentation-bot-token", json!({"streamingKey": ""}))
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (status, body) = h.post("/cast/presentation-bot-token", json!([])).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["data"][0]["message"], "must be object");
    let text = Request::post("/cast/streamer-token")
        .header("content-type", "text/plain")
        .body(Body::from("{}"))
        .unwrap();
    let (status, body) = body_json(h.app.clone().oneshot(text).await.unwrap()).await;
    assert_eq!(status, StatusCode::UNSUPPORTED_MEDIA_TYPE);
    assert_eq!(
        body,
        json!({"ok": false, "message": "Content-Type must be application/json"})
    );
}

#[tokio::test]
async fn watcher_tokens_need_a_viewers_own_strict_signature() {
    let h = harness(lazy_pool(), None, PermissionDecision::Allowed, None).await;
    let body = json!({"location": "0,0", "identity": "anon"});
    // Unsigned: the crypto middleware's refusal, before any credential work.
    let (status, value) = h.post("/cast/watcher-token", body.clone()).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(value["ok"], false);
    // A scene-signed request is refused: watcher tokens are for viewers.
    let (status, value) = h
        .signed(
            KEY,
            "POST",
            "/cast/watcher-token",
            &json!({"signer": SCENE_SIGNER}),
            Some(body.clone()),
        )
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(
        value,
        json!({"ok": false, "message": "Invalid metadata content"})
    );
    // Signed canonically, delivered re-cased.
    let headers = crate::gatekeeper::auth::test_support::signed_as(
        KEY,
        crate::gatekeeper::auth::test_support::Shape::V6,
        "post",
        "/cast/watcher-token",
        &json!({"signer": SCENE_SIGNER}).to_string(),
        &json!({"signer": "Decentraland-Kernel-Scene"}).to_string(),
    )
    .await;
    let (status, value) = h
        .send("POST", "/cast/watcher-token", headers, Some(body.clone()))
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(value["message"], "Invalid metadata content");
    // The re-spelled key signed as such.
    let (status, value) = h
        .signed(
            KEY,
            "POST",
            "/cast/watcher-token",
            &json!({"Signer": SCENE_SIGNER}),
            Some(body.clone()),
        )
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(value["message"], "Invalid metadata content");
    // No legacy (pre-6.0.0) payload on this route.
    let metadata = json!({"signer": "dcl:explorer", "deviceIdentifier": "Dev-1"}).to_string();
    let headers = crate::gatekeeper::auth::test_support::signed_as(
        KEY,
        crate::gatekeeper::auth::test_support::Shape::Legacy,
        "post",
        "/cast/watcher-token",
        &metadata,
        &metadata,
    )
    .await;
    let (status, value) = h
        .send("POST", "/cast/watcher-token", headers, Some(body))
        .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(value, json!({"ok": false, "message": "Invalid signature"}));
}

/// `cast-presenters-unauth` (production capture, 2026-09-25).
#[tokio::test]
async fn unsigned_presenters_answer_the_captured_middleware_refusal() {
    let h = harness(lazy_pool(), None, PermissionDecision::Allowed, None).await;
    let (status, body) = h
        .send("GET", "/cast/presenters", HeaderMap::new(), None)
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body, json!({"ok": false, "message": "Invalid Auth Chain"}));
}

#[tokio::test]
async fn stream_links_need_a_realm_and_a_scene_in_the_signed_metadata() {
    let h = harness(lazy_pool(), None, PermissionDecision::Allowed, None).await;
    let path = "/cast/generate-stream-link";
    let (status, body) = h
        .signed(
            KEY,
            "GET",
            path,
            &json!({"signer": SCENE_SIGNER, "sceneId": "bafkreix"}),
            None,
        )
        .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(
        body,
        json!({"error": "Access denied, invalid signed-fetch request, no realm"})
    );
    let (status, body) = h
        .signed(
            KEY,
            "GET",
            path,
            &json!({"signer": SCENE_SIGNER, "realm": {"serverName": "main"}}),
            None,
        )
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(
        body,
        json!({"error": "sceneId is required in authMetadata for Cast2 chat functionality"})
    );
    // Only a scene signs for a stream link.
    let (status, body) = h.signed(KEY, "GET", path, &viewer_metadata(), None).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["message"], "Invalid metadata content");
}

// ── against the scratch database ──

/// `cast-stream-info-invalid-key` (production capture, 2026-09-25), and the
/// place behind a live key.
#[tokio::test]
async fn stream_info_answers_the_place_behind_a_live_key() {
    let Some(pool) = scratch().await else { return };
    let h = harness(pool.clone(), None, PermissionDecision::Allowed, None).await;
    let (status, body) = h
        .send(
            "GET",
            "/cast/stream-info/not-a-real-streaming-key",
            HeaderMap::new(),
            None,
        )
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body, json!({"error": "Invalid streaming key"}));

    let scene = genesis_scene();
    seed_genesis_place(&pool, &scene, Some("Cast Plaza")).await;
    let key = seed_access(
        &pool,
        &scene.place_id,
        Some(&scene.room),
        chrono::Duration::days(1),
    )
    .await;
    let (status, body) = h
        .send(
            "GET",
            &format!("/cast/stream-info/{key}"),
            HeaderMap::new(),
            None,
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        body,
        json!({"placeName": scene.parcel, "placeId": scene.place_id, "location": scene.parcel, "isWorld": false})
    );

    let world = format!("cast{}.dcl.eth", tag());
    let world_place = format!("cast-world-{}", tag());
    seed_place(
        &pool,
        &world_place,
        json!({"world": true, "world_name": world, "positions": ["0,0"]}),
        "0,0",
    )
    .await;
    let key = seed_access(
        &pool,
        &world_place,
        Some("w1:room"),
        chrono::Duration::days(1),
    )
    .await;
    let (_, body) = h
        .send(
            "GET",
            &format!("/cast/stream-info/{key}"),
            HeaderMap::new(),
            None,
        )
        .await;
    assert_eq!(
        body,
        json!({"placeName": world, "placeId": world_place, "location": world, "isWorld": true})
    );

    let expired = seed_access(
        &pool,
        &format!("cast-expired-{}", tag()),
        Some(&scene.room),
        -chrono::Duration::minutes(1),
    )
    .await;
    let (status, body) = h
        .send(
            "GET",
            &format!("/cast/stream-info/{expired}"),
            HeaderMap::new(),
            None,
        )
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body, json!({"error": "Stream access has expired"}));
    let roomless = seed_access(
        &pool,
        &format!("cast-roomless-{}", tag()),
        None,
        chrono::Duration::days(1),
    )
    .await;
    let (_, body) = h
        .send(
            "GET",
            &format!("/cast/stream-info/{roomless}"),
            HeaderMap::new(),
            None,
        )
        .await;
    assert_eq!(
        body,
        json!({"error": "Stream access is missing room information"})
    );
}

#[tokio::test]
async fn a_scene_admin_gets_a_stream_link_that_is_reused_until_it_expires() {
    let Some(pool) = scratch().await else { return };
    let scene = genesis_scene();
    seed_genesis_place(&pool, &scene, Some("Cast Plaza")).await;
    let room = resolved_room(RealmId::Shared("main".into()), &scene.id, &scene.parcel);
    let h = harness(pool.clone(), Some(room), PermissionDecision::Allowed, None).await;
    let path = "/cast/generate-stream-link";
    let (status, first) = h
        .signed(KEY, "GET", path, &scene_metadata(&scene), None)
        .await;
    assert_eq!(status, StatusCode::OK, "{first}");
    let key = first["streamingKey"].as_str().unwrap().to_owned();
    assert_eq!(
        first,
        json!({
            "streamLink": format!("https://cast.example/s/{key}"),
            "watcherLink": format!("https://cast.example/w/{}", scene.parcel),
            "streamingKey": key,
            "placeId": scene.place_id,
            "placeName": "Cast Plaza",
            "expiresAt": first["expiresAt"],
            "expiresInDays": 4,
        })
    );
    let expires = chrono::DateTime::parse_from_rfc3339(first["expiresAt"].as_str().unwrap())
        .unwrap()
        .timestamp_millis();
    let left = expires - chrono::Utc::now().timestamp_millis();
    assert!((4 * 86_400_000 - 60_000..=4 * 86_400_000).contains(&left));
    let stored: (String, String) = sqlx::query_as(
        "SELECT room_id, generated_by FROM social.scene_stream_access WHERE place_id = $1 AND active",
    )
    .bind(&scene.place_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(stored, (scene.room.clone(), address_of(KEY)));

    // The same place again: the same key, no new ingress.
    let (status, second) = h
        .signed(KEY, "GET", path, &scene_metadata(&scene), None)
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(second["streamingKey"], key);
    assert_eq!(h.livekit.ingresses.load(Ordering::SeqCst), 1);

    // An expired, idle key is replaced by a new four-day one.
    sqlx::query("UPDATE social.scene_stream_access SET expiration_time = timezone('UTC', now()) - interval '1 minute' WHERE place_id = $1 AND active")
        .bind(&scene.place_id)
        .execute(&pool)
        .await
        .unwrap();
    let (status, renewed) = h
        .signed(KEY, "GET", path, &scene_metadata(&scene), None)
        .await;
    assert_eq!(status, StatusCode::OK, "{renewed}");
    assert_ne!(renewed["streamingKey"], key);
    assert_eq!(renewed["expiresInDays"], 4);
    assert_eq!(h.livekit.ingresses.load(Ordering::SeqCst), 2);

    // A live broadcast is not interrupted by a renewal.
    sqlx::query("UPDATE social.scene_stream_access SET streaming = true, expiration_time = timezone('UTC', now()) - interval '1 minute' WHERE place_id = $1 AND active")
        .bind(&scene.place_id)
        .execute(&pool)
        .await
        .unwrap();
    let (status, body) = h
        .signed(KEY, "GET", path, &scene_metadata(&scene), None)
        .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(
        body,
        json!({"error": crate::gatekeeper::streaming::RENEWAL_CONFLICT_MSG})
    );
}

#[tokio::test]
async fn a_world_scene_admin_gets_a_link_watchers_open_by_world_name() {
    let Some(pool) = scratch().await else { return };
    let world = WorldName::parse(&format!("castworld{}.dcl.eth", tag())).unwrap();
    let scene_id = format!("bafkreiworld{}", tag());
    let room = resolved_room(RealmId::World(world.clone()), &scene_id, "3,4");
    let place_id = format!("cast-world-{}", tag());
    seed_place(
        &pool,
        &place_id,
        json!({"world": true, "world_name": world.display(), "positions": ["3,4"]}),
        "3,4",
    )
    .await;
    let h = harness(
        pool.clone(),
        Some(room.clone()),
        PermissionDecision::Allowed,
        None,
    )
    .await;
    let metadata = json!({
        "signer": SCENE_SIGNER,
        "sceneId": scene_id,
        "parcel": "3,4",
        "realm": {"serverName": world.display(), "hostname": "worlds-content-server.example", "protocol": "v3"},
    });
    let (status, body) = h
        .signed(KEY, "GET", "/cast/generate-stream-link", &metadata, None)
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        body["watcherLink"],
        format!("https://cast.example/w/{}", world.display())
    );
    // Untitled: the place id names it.
    assert_eq!(body["placeName"], place_id.as_str());
    let stored: String = sqlx::query_scalar(
        "SELECT room_id FROM social.scene_stream_access WHERE place_id = $1 AND active",
    )
    .bind(&place_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(stored, room.room);
    let (status, body) = h
        .signed(
            &random_key(),
            "POST",
            "/cast/watcher-token",
            &viewer_metadata(),
            Some(json!({"location": world.display(), "identity": "viewer", "parcel": "3,4"})),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["roomId"], room.room);
}

#[tokio::test]
async fn stream_links_are_refused_to_non_admins_and_platform_banned_callers() {
    let Some(pool) = scratch().await else { return };
    let scene = genesis_scene();
    seed_genesis_place(&pool, &scene, None).await;
    let room = resolved_room(RealmId::Shared("main".into()), &scene.id, &scene.parcel);
    let path = "/cast/generate-stream-link";
    let h = harness(
        pool.clone(),
        Some(room.clone()),
        PermissionDecision::refused("not an admin"),
        None,
    )
    .await;
    let (status, body) = h
        .signed(OUTSIDER_KEY, "GET", path, &scene_metadata(&scene), None)
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(
        body,
        json!({"error": "Only scene administrators can generate stream links"})
    );
    assert_eq!(h.livekit.ingresses.load(Ordering::SeqCst), 0);

    // The ban is checked first, so the refusal says nothing about admin status.
    let banned = harness(
        pool.clone(),
        Some(room),
        PermissionDecision::Allowed,
        Some(StubBanStatus(Ok(Some(Ban::Platform)))),
    )
    .await;
    let (status, body) = banned
        .signed(KEY, "GET", path, &scene_metadata(&scene), None)
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(
        body,
        json!({"error": "Access denied, platform-banned user"})
    );
    assert_eq!(banned.livekit.ingresses.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn a_streaming_key_admits_a_streamer_who_becomes_a_presenter() {
    let Some(pool) = scratch().await else { return };
    let h = harness(pool.clone(), None, PermissionDecision::Allowed, None).await;
    let scene = genesis_scene();
    let key = seed_access(
        &pool,
        &scene.place_id,
        Some(&scene.room),
        chrono::Duration::days(1),
    )
    .await;
    let (status, body) = h
        .post(
            "/cast/streamer-token",
            json!({"token": key, "identity": "happy-fox"}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["url"], "wss://livekit.local");
    assert_eq!(body["roomId"], scene.room);
    let identity = body["identity"].as_str().unwrap();
    assert!(
        identity.starts_with(&format!("stream:{}:", scene.place_id)),
        "{identity}"
    );
    assert!(is_valid_presenter_identity(identity));
    let claims = claims(&body["token"]);
    assert_eq!(claims["sub"], identity);
    assert_eq!(claims["name"], "happy-fox");
    assert_eq!(
        claims["metadata"],
        r#"{"role":"streamer","displayName":"happy-fox"}"#
    );
    assert_eq!(claims["video"]["room"], scene.room);
    assert_eq!(claims["video"]["canPublish"], true);
    assert_eq!(claims["video"]["canSubscribe"], true);
    assert!(
        claims["video"].get("canPublishSources").is_none(),
        "a streamer publishes every source"
    );
    // The room is created and the streamer listed as a presenter.
    assert_eq!(
        h.livekit.metadata(&scene.room)["presenters"],
        json!([identity])
    );

    let (status, body) = h
        .post(
            "/cast/streamer-token",
            json!({"token": "invalid-token", "identity": "happy-fox"}),
        )
        .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(body, json!({"error": "Invalid or expired streaming token"}));
    let expired = seed_access(
        &pool,
        &format!("cast-expired-{}", tag()),
        Some(&scene.room),
        -chrono::Duration::seconds(1),
    )
    .await;
    let (status, body) = h
        .post(
            "/cast/streamer-token",
            json!({"token": expired, "identity": "test-user"}),
        )
        .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(body, json!({"error": "Streaming token has expired"}));
}

#[tokio::test]
async fn a_streaming_key_admits_the_presentation_bot() {
    let Some(pool) = scratch().await else { return };
    let h = harness(pool.clone(), None, PermissionDecision::Allowed, None).await;
    let scene = genesis_scene();
    let key = seed_access(
        &pool,
        &scene.place_id,
        Some(&scene.room),
        chrono::Duration::days(1),
    )
    .await;
    let (status, body) = h
        .post("/cast/presentation-bot-token", json!({"streamingKey": key}))
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        body.as_object().unwrap().keys().collect::<Vec<_>>(),
        ["roomId", "token", "url"]
    );
    assert_eq!(body["roomId"], scene.room);
    let claims = claims(&body["token"]);
    assert!(
        claims["sub"]
            .as_str()
            .unwrap()
            .starts_with(&format!("presentation-bot:{}:", scene.room))
    );
    assert_eq!(claims["metadata"], r#"{"role":"presentation"}"#);
    assert!(claims.get("name").is_none());
    assert_eq!(claims["video"]["canUpdateOwnMetadata"], true);
    assert_eq!(claims["video"]["canPublish"], true);
    assert!(claims["video"].get("canPublishSources").is_none());

    let (status, _) = h
        .post(
            "/cast/presentation-bot-token",
            json!({"streamingKey": "nobody-generated-this"}),
        )
        .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let expired = seed_access(
        &pool,
        &format!("cast-expired-{}", tag()),
        Some(&scene.room),
        -chrono::Duration::seconds(1),
    )
    .await;
    let (status, body) = h
        .post(
            "/cast/presentation-bot-token",
            json!({"streamingKey": expired}),
        )
        .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(body, json!({"error": "Streaming token has expired"}));
}

#[tokio::test]
async fn a_viewer_watches_a_parcel_or_world_stream_unless_banned() {
    let Some(pool) = scratch().await else { return };
    let scene = genesis_scene();
    seed_genesis_place(&pool, &scene, Some("Cast Plaza")).await;
    let world = WorldName::parse(&format!("cast{}.dcl.eth", tag())).unwrap();
    let world_scene = format!("bafkreiworld{}", tag());
    let world_room = resolved_room(RealmId::World(world.clone()), &world_scene, "0,0");
    let world_place = format!("cast-world-{}", tag());
    seed_place(
        &pool,
        &world_place,
        json!({"world": true, "world_name": world.display(), "positions": ["0,0"]}),
        "0,0",
    )
    .await;
    let h = harness(
        pool.clone(),
        Some(world_room.clone()),
        PermissionDecision::Allowed,
        None,
    )
    .await;
    let watch = |location: String| json!({"location": location, "identity": "Viewer #1"});
    let key = random_key();
    let viewer = Address::parse(&address_of(&key)).unwrap();

    // No stream yet.
    let (status, body) = h
        .signed(
            &key,
            "POST",
            "/cast/watcher-token",
            &viewer_metadata(),
            Some(watch(scene.parcel.clone())),
        )
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(
        body,
        json!({"error": format!("No active stream found for {}", scene.parcel)})
    );

    seed_access(
        &pool,
        &scene.place_id,
        Some(&scene.room),
        chrono::Duration::days(1),
    )
    .await;
    let (status, body) = h
        .signed(
            &key,
            "POST",
            "/cast/watcher-token",
            &viewer_metadata(),
            Some(watch(scene.parcel.clone())),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["roomId"], scene.room);
    assert_eq!(body["placeName"], "Cast Plaza");
    let identity = body["identity"].as_str().unwrap();
    assert!(identity.starts_with(&format!("watch:{}:", scene.room)));
    let claims = claims(&body["token"]);
    assert_eq!(claims["sub"], identity);
    assert_eq!(claims["name"], "Viewer #1");
    assert_eq!(
        claims["metadata"],
        // JSON.stringify's insertion order.
        format!(
            r#"{{"role":"watcher","displayName":"Viewer #1","roomId":"{}"}}"#,
            scene.room
        )
    );
    assert_eq!(claims["video"]["canPublish"], false);
    assert_eq!(claims["video"]["canSubscribe"], true);
    assert_eq!(claims["video"]["canUpdateOwnMetadata"], false);
    assert_eq!(claims["video"]["canPublishSources"], json!(["microphone"]));

    // A world by name; a place without a title answers the world name.
    seed_access(
        &pool,
        &world_place,
        Some(&world_room.room),
        chrono::Duration::days(1),
    )
    .await;
    let (status, body) = h
        .signed(
            &key,
            "POST",
            "/cast/watcher-token",
            &viewer_metadata(),
            Some(watch(world.display().to_uppercase())),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["roomId"], world_room.room);
    assert_eq!(body["placeName"], world.display());

    // Banned from the scene the stream runs in.
    h.state
        .moderation
        .scene_ban(&scene.id, &viewer, &wallet(), &[], "Scene")
        .await
        .unwrap();
    let (status, body) = h
        .signed(
            &key,
            "POST",
            "/cast/watcher-token",
            &viewer_metadata(),
            Some(watch(scene.parcel.clone())),
        )
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(body, json!({"error": "You are banned from this scene"}));

    // An unknown parcel and an unknown world.
    for location in [unique_parcel(), format!("nobody{}.dcl.eth", tag())] {
        let (status, _) = h
            .signed(
                &key,
                "POST",
                "/cast/watcher-token",
                &viewer_metadata(),
                Some(watch(location.clone())),
            )
            .await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{location}");
    }
}

#[tokio::test]
async fn an_expired_stream_asks_the_viewer_for_a_new_link() {
    let Some(pool) = scratch().await else { return };
    let scene = genesis_scene();
    seed_genesis_place(&pool, &scene, None).await;
    seed_access(
        &pool,
        &scene.place_id,
        Some(&scene.room),
        -chrono::Duration::seconds(1),
    )
    .await;
    let h = harness(pool.clone(), None, PermissionDecision::Allowed, None).await;
    let (status, body) = h
        .signed(
            &random_key(),
            "POST",
            "/cast/watcher-token",
            &viewer_metadata(),
            Some(json!({"location": scene.parcel, "identity": "late"})),
        )
        .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(
        body,
        json!({"error": "Stream access has expired. Please generate a new stream link."})
    );
}

#[tokio::test]
async fn platform_bans_refuse_watchers_by_wallet_or_recorded_device() {
    let Some(pool) = scratch().await else { return };
    let scene = genesis_scene();
    seed_genesis_place(&pool, &scene, None).await;
    seed_access(
        &pool,
        &scene.place_id,
        Some(&scene.room),
        chrono::Duration::days(1),
    )
    .await;
    let body = json!({"location": scene.parcel, "identity": "viewer"});
    let refused = json!({"error": "Access denied, platform-banned user"});

    // The wallet's own ban, before the location is even resolved.
    let banned = harness(
        pool.clone(),
        None,
        PermissionDecision::Allowed,
        Some(StubBanStatus(Ok(Some(Ban::Platform)))),
    )
    .await;
    for location in [scene.parcel.clone(), format!("nowhere{}.dcl.eth", tag())] {
        let (status, value) = banned
            .signed(
                &random_key(),
                "POST",
                "/cast/watcher-token",
                &viewer_metadata(),
                Some(json!({"location": location, "identity": "viewer"})),
            )
            .await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(value, refused);
    }

    // Another wallet's ban on the device this wallet last connected from.
    let h = harness(pool.clone(), None, PermissionDecision::Allowed, None).await;
    let key = random_key();
    let viewer = Address::parse(&address_of(&key)).unwrap();
    let device = format!("cast-device-{}", tag());
    h.state
        .moderation
        .create_ban(CreateBan {
            banned_address: wallet(),
            banned_by: wallet().as_str().into(),
            reason: "abuse".into(),
            custom_message: None,
            banned_device_id: Some(device.clone()),
            duration_ms: None,
        })
        .await
        .unwrap();
    h.state
        .moderation
        .upsert_connection(UpsertPlayerConnection {
            address: viewer.clone(),
            ip_address: None,
            device_id: Some(device),
        })
        .await
        .unwrap();
    let (status, value) = h
        .signed(
            &key,
            "POST",
            "/cast/watcher-token",
            &viewer_metadata(),
            Some(body.clone()),
        )
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(value, refused);
    // A clean wallet is served.
    let (status, value) = h
        .signed(
            &random_key(),
            "POST",
            "/cast/watcher-token",
            &viewer_metadata(),
            Some(body),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{value}");
}

#[tokio::test]
async fn scene_admins_list_promote_and_demote_presenters() {
    let Some(pool) = scratch().await else { return };
    let scene = genesis_scene();
    seed_genesis_place(&pool, &scene, None).await;
    let room = resolved_room(RealmId::Shared("main".into()), &scene.id, &scene.parcel);
    let h = harness(
        pool.clone(),
        Some(room.clone()),
        PermissionDecision::Allowed,
        None,
    )
    .await;
    let metadata = scene_metadata(&scene);

    // No stream in the scene's room yet.
    let (status, body) = h
        .signed(KEY, "GET", "/cast/presenters", &metadata, None)
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(
        body,
        json!({"error": format!("No active stream found for {}", scene.room)})
    );

    seed_access(
        &pool,
        &scene.place_id,
        Some(&scene.room),
        chrono::Duration::days(1),
    )
    .await;
    let (status, body) = h
        .signed(KEY, "GET", "/cast/presenters", &metadata, None)
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, json!({"presenters": []}));

    let wallet_identity = "0x1234567890abcdef1234567890abcdef12345678";
    let streamer = format!("stream:{}:{}", scene.place_id, Uuid::new_v4());
    h.livekit
        .set_metadata(&scene.room, json!({"sceneAdmins": ["0xadmin"]}));
    for identity in [wallet_identity, streamer.as_str()] {
        let path = format!("/cast/presenters/{identity}");
        let (status, body) = h.signed(KEY, "PUT", &path, &metadata, None).await;
        assert_eq!(status, StatusCode::OK, "{identity}: {body}");
        assert_eq!(
            body,
            json!({"message": "Participant promoted to presenter"})
        );
    }
    // Idempotent, and the other metadata survives.
    let (status, _) = h
        .signed(
            KEY,
            "PUT",
            &format!("/cast/presenters/{wallet_identity}"),
            &metadata,
            None,
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        h.livekit.metadata(&scene.room),
        json!({"sceneAdmins": ["0xadmin"], "presenters": [wallet_identity, streamer]})
    );
    let (status, body) = h
        .signed(KEY, "GET", "/cast/presenters", &metadata, None)
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, json!({"presenters": [wallet_identity, streamer]}));

    let (status, body) = h
        .signed(
            KEY,
            "DELETE",
            &format!("/cast/presenters/{wallet_identity}"),
            &metadata,
            None,
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        body,
        json!({"message": "Participant demoted from presenter"})
    );
    assert_eq!(
        h.livekit.metadata(&scene.room)["presenters"],
        json!([streamer])
    );

    // Watcher and bot identities, and anything else, are not presenters.
    for identity in [
        format!("watch:{}:{}", scene.room, Uuid::new_v4()),
        format!("presentation-bot:{}:{}", scene.room, Uuid::new_v4()),
        "invalid-identity".to_owned(),
    ] {
        for method in ["PUT", "DELETE"] {
            let (status, body) = h
                .signed(
                    KEY,
                    method,
                    &format!("/cast/presenters/{identity}"),
                    &metadata,
                    None,
                )
                .await;
            assert_eq!(status, StatusCode::BAD_REQUEST, "{method} {identity}");
            assert_eq!(
                body,
                json!({"error": "participantIdentity must be a valid Ethereum address or streamer identity"})
            );
        }
    }

    // A caller who is not an owner or admin of the stored place.
    let outsider = harness(
        pool.clone(),
        Some(room),
        PermissionDecision::refused("not an admin"),
        None,
    )
    .await;
    for (method, path) in [
        ("GET", "/cast/presenters".to_owned()),
        ("PUT", format!("/cast/presenters/{wallet_identity}")),
        ("DELETE", format!("/cast/presenters/{wallet_identity}")),
    ] {
        let (status, body) = outsider
            .signed(OUTSIDER_KEY, method, &path, &metadata, None)
            .await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{method} {path}");
        assert_eq!(
            body,
            json!({"error": "Only scene administrators can manage presenters"})
        );
    }

    // The stored place vanished from the catalogue.
    sqlx::query("DELETE FROM explore.place WHERE id = $1")
        .bind(&scene.place_id)
        .execute(&pool)
        .await
        .unwrap();
    let (status, body) = h
        .signed(KEY, "GET", "/cast/presenters", &metadata, None)
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(
        body,
        json!({"error": format!("No place found with id {}", scene.place_id)})
    );
}
