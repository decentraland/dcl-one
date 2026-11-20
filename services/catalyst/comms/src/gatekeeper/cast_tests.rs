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
use crate::gatekeeper::streaming::cast::{
    build_stream_links, is_valid_presenter_identity, shown_name, watcher_wallet,
};
use crate::rooms::{ActiveDeployment, ResolvedRoom};
use crate::world_identity::{Parcel, RealmId, SceneId, WorldName};

const SCENE_SIGNER: &str = crate::gatekeeper::auth::SCENE_SIGNER;

#[derive(Default)]
struct LiveKitFixture {
    rooms: Mutex<HashMap<String, String>>,
    /// Connected participants per room: identity and join time (unix seconds).
    participants: Mutex<HashMap<String, Vec<(String, i64)>>>,
    removed: Mutex<Vec<String>>,
    /// CreateIngress and CreateRoom answer 500 while set.
    fail_ingress: std::sync::atomic::AtomicBool,
    fail_rooms: std::sync::atomic::AtomicBool,
    /// ListParticipants answers 500 while set.
    fail_participants: std::sync::atomic::AtomicBool,
    ingresses: AtomicUsize,
    deleted_ingresses: AtomicUsize,
    /// ListIngress by id: the status reported for an ingress; an ingress not
    /// listed here is unknown to LiveKit (empty `items`).
    ingress_status: Mutex<HashMap<String, String>>,
    /// ListIngress answers 500 while set.
    fail_list_ingress: std::sync::atomic::AtomicBool,
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

    fn join(&self, room: &str, identity: &str, joined_at: i64) {
        self.participants
            .lock()
            .unwrap()
            .entry(room.to_owned())
            .or_default()
            .push((identity.to_owned(), joined_at));
    }

    fn connected(&self, room: &str) -> Vec<String> {
        self.participants
            .lock()
            .unwrap()
            .get(room)
            .map(|p| p.iter().map(|(id, _)| id.clone()).collect())
            .unwrap_or_default()
    }

    async fn serve(self: &Arc<Self>) -> (String, tokio::task::JoinHandle<()>) {
        let (a, b, c, d, e, f, g, h) = (
            self.clone(),
            self.clone(),
            self.clone(),
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
                    let failing = a.fail_ingress.load(Ordering::SeqCst);
                    let n = if failing { 0 } else { a.ingresses.fetch_add(1, Ordering::SeqCst) };
                    async move {
                        if failing {
                            return Err(StatusCode::INTERNAL_SERVER_ERROR);
                        }
                        Ok(Json(json!({
                            "ingressId": format!("IN_cast_{n}_{}", tag()),
                            "url": "rtmp://ingress.example/live",
                            "streamKey": format!("cast-key-{}", Uuid::new_v4()),
                        })))
                    }
                }),
            )
            .route(
                "/twirp/livekit.Ingress/ListIngress",
                axum::routing::post(move |Json(body): Json<Value>| {
                    let failing = h.fail_list_ingress.load(Ordering::SeqCst);
                    let id = body["ingressId"].as_str().unwrap_or_default().to_owned();
                    let items: Vec<Value> = h
                        .ingress_status
                        .lock()
                        .unwrap()
                        .get(&id)
                        .map(|status| json!({"ingressId": id, "state": {"status": status}}))
                        .into_iter()
                        .collect();
                    async move {
                        if failing {
                            return Err(StatusCode::INTERNAL_SERVER_ERROR);
                        }
                        Ok(Json(json!({ "items": items })))
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
                    let failing = d.fail_rooms.load(Ordering::SeqCst);
                    if !failing {
                        d.rooms.lock().unwrap().entry(name.clone()).or_default();
                    }
                    async move {
                        if failing {
                            return Err(StatusCode::INTERNAL_SERVER_ERROR);
                        }
                        Ok(Json(json!({"name": name, "sid": "RM_cast"})))
                    }
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
        let app = app
            .route(
                "/twirp/livekit.RoomService/ListParticipants",
                axum::routing::post(move |Json(body): Json<Value>| {
                    let room = body["room"].as_str().unwrap().to_owned();
                    let failing = f.fail_participants.load(Ordering::SeqCst);
                    let listed: Vec<Value> = f
                        .participants
                        .lock()
                        .unwrap()
                        .get(&room)
                        .map(|p| {
                            p.iter()
                                .map(|(id, at)| json!({"sid": format!("PA_{id}"), "identity": id, "metadata": "", "joinedAt": at.to_string()}))
                                .collect()
                        })
                        .unwrap_or_default();
                    async move {
                        if failing {
                            return Err(StatusCode::INTERNAL_SERVER_ERROR);
                        }
                        Ok(Json(json!({ "participants": listed })))
                    }
                }),
            )
            .route(
                "/twirp/livekit.RoomService/RemoveParticipant",
                axum::routing::post(move |Json(body): Json<Value>| {
                    let room = body["room"].as_str().unwrap().to_owned();
                    let identity = body["identity"].as_str().unwrap().to_owned();
                    if let Some(p) = g.participants.lock().unwrap().get_mut(&room) {
                        p.retain(|(id, _)| *id != identity);
                    }
                    g.removed.lock().unwrap().push(identity);
                    async { Json(json!({})) }
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
    let policy = StubPolicy {
        admin,
        ..Default::default()
    };
    harness_with(pool, room, policy, bans).await
}

async fn harness_with(
    pool: PgPool,
    room: Option<ResolvedRoom>,
    policy: StubPolicy,
    bans: Option<StubBanStatus>,
) -> Harness {
    harness_tuned(pool, room, policy, bans, |_| {}).await
}

async fn harness_tuned(
    pool: PgPool,
    room: Option<ResolvedRoom>,
    policy: StubPolicy,
    bans: Option<StubBanStatus>,
    tune: impl FnOnce(&mut CommsState),
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
    state.policy = Arc::new(policy);
    state.livekit.api_url = url;
    tune(&mut state);
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

/// A world the worlds store knows, as one a Cast runs in is: its access
/// edits stamp the authorization proofs world tokens carry.
async fn seed_world(pool: &PgPool, world: &WorldName) {
    sqlx::query("INSERT INTO worlds.worlds (name) VALUES ($1) ON CONFLICT DO NOTHING")
        .bind(world.key())
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
fn a_watchers_wallet_comes_from_its_signed_attribute() {
    let wallet = address_of(KEY);
    let attributes: std::collections::BTreeMap<String, String> =
        [("dcl.castWatcher".to_owned(), wallet.clone())].into();
    assert_eq!(
        watcher_wallet("watch:scene:main:x:1", &attributes).map(|a| a.as_str().to_owned()),
        Some(wallet)
    );
    // Only for a watcher's identity, and only a wallet.
    assert_eq!(watcher_wallet("stream:p:1", &attributes), None);
    let junk: std::collections::BTreeMap<String, String> =
        [("dcl.castWatcher".to_owned(), "Moderator".to_owned())].into();
    assert_eq!(watcher_wallet("watch:scene:main:x:1", &junk), None);
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
    let ingress_id: String = sqlx::query_scalar("UPDATE social.scene_stream_access SET streaming = true, expiration_time = timezone('UTC', now()) - interval '1 minute' WHERE place_id = $1 AND active RETURNING ingress_id")
        .bind(&scene.place_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    h.livekit
        .ingress_status
        .lock()
        .unwrap()
        .insert(ingress_id, "ENDPOINT_PUBLISHING".into());
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
async fn a_stale_streaming_flag_does_not_block_a_new_stream_link() {
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
    // The credential belongs to a room the place no longer has (a redeploy
    // changes it) and still says it streams: the ingress_ended webhook was lost.
    let ingress_id: String = sqlx::query_scalar(
        "UPDATE social.scene_stream_access SET streaming = true, streaming_start_time = timezone('UTC', now()), room_id = 'scene:main:redeployed' \
         WHERE place_id = $1 AND active RETURNING ingress_id",
    )
    .bind(&scene.place_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let conflict = json!({"error": crate::gatekeeper::streaming::RENEWAL_CONFLICT_MSG});

    // LiveKit unreachable: the flag is kept and the renewal refused.
    h.livekit.fail_list_ingress.store(true, Ordering::SeqCst);
    let (status, body) = h
        .signed(KEY, "GET", path, &scene_metadata(&scene), None)
        .await;
    assert_eq!((status, &body), (StatusCode::CONFLICT, &conflict));
    h.livekit.fail_list_ingress.store(false, Ordering::SeqCst);

    // The ingress really publishes: still refused.
    for live in ["ENDPOINT_PUBLISHING", "ENDPOINT_BUFFERING"] {
        h.livekit
            .ingress_status
            .lock()
            .unwrap()
            .insert(ingress_id.clone(), live.into());
        let (status, body) = h
            .signed(KEY, "GET", path, &scene_metadata(&scene), None)
            .await;
        assert_eq!((status, &body), (StatusCode::CONFLICT, &conflict), "{live}");
    }
    assert_eq!(h.livekit.ingresses.load(Ordering::SeqCst), 1);

    // LiveKit no longer knows the ingress: the flag is stale, the credential replaced.
    h.livekit.ingress_status.lock().unwrap().remove(&ingress_id);
    let (status, renewed) = h
        .signed(KEY, "GET", path, &scene_metadata(&scene), None)
        .await;
    assert_eq!(status, StatusCode::OK, "{renewed}");
    assert_ne!(renewed["streamingKey"], first["streamingKey"]);
    assert_eq!(h.livekit.ingresses.load(Ordering::SeqCst), 2);
    let old: (bool, bool) = sqlx::query_as(
        "SELECT active, streaming FROM social.scene_stream_access WHERE ingress_id = $1",
    )
    .bind(&ingress_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(old, (false, false));
    let current: String = sqlx::query_scalar(
        "SELECT room_id FROM social.scene_stream_access WHERE place_id = $1 AND active",
    )
    .bind(&scene.place_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(current, scene.room);
}

#[tokio::test]
async fn a_world_scene_admin_gets_a_link_watchers_open_by_world_name() {
    let Some(pool) = scratch().await else { return };
    let world = WorldName::parse(&format!("castworld{}.dcl.eth", tag())).unwrap();
    seed_world(&pool, &world).await;
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
    // The world authorization proof a scene join carries, for this watcher and
    // room: the world access sweep keeps it past any earlier access edit.
    let claims = claims(&body["token"]);
    let proof = claims["attributes"]["dcl.worldAuthorization"]
        .as_str()
        .expect("a world watcher's token carries the authorization proof");
    let authorized_after = |cutoff: chrono::NaiveDateTime| {
        crate::gatekeeper::admission::world_mint::authorized_after(
            &h.state.livekit.api_secret,
            &world,
            body["identity"].as_str().unwrap(),
            &room.room,
            Some(proof),
            cutoff,
        )
    };
    let now = chrono::Utc::now().naive_utc();
    assert!(authorized_after(now - chrono::Duration::hours(1)));
    assert!(!authorized_after(now + chrono::Duration::hours(1)));
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
    seed_world(&pool, &world).await;
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
    // The viewer's wallet rides in a signed attribute, for the ban sweeps.
    assert_eq!(claims["attributes"]["dcl.castWatcher"], address_of(&key));
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

/// A world's access policy holds for watchers as for every other join of its
/// scene room: a wallet the world does not admit gets no watcher token.
#[tokio::test]
async fn a_private_worlds_cast_refuses_watchers_the_world_does_not_admit() {
    let Some(pool) = scratch().await else { return };
    let world = WorldName::parse(&format!("castprivate{}.dcl.eth", tag())).unwrap();
    seed_world(&pool, &world).await;
    let scene_id = format!("bafkreiprivate{}", tag());
    let room = resolved_room(RealmId::World(world.clone()), &scene_id, "1,1");
    let place_id = format!("cast-private-{}", tag());
    seed_place(
        &pool,
        &place_id,
        json!({"world": true, "world_name": world.display(), "positions": ["1,1"]}),
        "1,1",
    )
    .await;
    seed_access(
        &pool,
        &place_id,
        Some(&room.room),
        chrono::Duration::days(1),
    )
    .await;
    let body = json!({"location": world.display(), "identity": "viewer"});

    let closed = harness_with(
        pool.clone(),
        Some(room.clone()),
        StubPolicy {
            access: Ok(crate::access::Access::NotAllowed),
            ..Default::default()
        },
        None,
    )
    .await;
    let (status, refused) = closed
        .signed(
            &random_key(),
            "POST",
            "/cast/watcher-token",
            &viewer_metadata(),
            Some(body.clone()),
        )
        .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "{refused}");
    assert_eq!(
        refused,
        json!({"error": crate::gatekeeper::streaming::cast::WORLD_ACCESS_DENIED_MSG})
    );

    let open = harness_with(
        pool.clone(),
        Some(room.clone()),
        StubPolicy::default(),
        None,
    )
    .await;
    let (status, admitted) = open
        .signed(
            &random_key(),
            "POST",
            "/cast/watcher-token",
            &viewer_metadata(),
            Some(body),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{admitted}");
    assert_eq!(admitted["roomId"], room.room);
}

/// A revoked or replaced streaming key stops the WebRTC publishers it admitted,
/// not just its RTMP ingress, and their ids leave `presenters`; a replacement's
/// own publishers (joined after it was created) and everyone else stay.
#[tokio::test]
async fn revoking_a_key_removes_the_cast_publishers_it_admitted() {
    let Some(pool) = scratch().await else { return };
    let scene = genesis_scene();
    seed_genesis_place(&pool, &scene, None).await;
    let room = resolved_room(RealmId::Shared("main".into()), &scene.id, &scene.parcel);
    // The LAND owner, whom the revocation notifies.
    let owner: crate::access::LandOperators =
        serde_json::from_value(json!({"owner": address_of(KEY)})).unwrap();
    let h = harness_with(
        pool.clone(),
        Some(room),
        StubPolicy {
            admin: PermissionDecision::Allowed,
            land: Ok(Some(Arc::new(Some(owner)))),
            ..Default::default()
        },
        None,
    )
    .await;
    let metadata = scene_metadata(&scene);
    let (status, body) = h
        .signed(KEY, "GET", "/cast/generate-stream-link", &metadata, None)
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let key = body["streamingKey"].as_str().unwrap().to_owned();
    let (status, streamer) = h
        .post(
            "/cast/streamer-token",
            json!({"token": key, "identity": "Ada"}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{streamer}");
    let streamer_id = streamer["identity"].as_str().unwrap().to_owned();
    // The identity names the credential that minted it.
    let current: Uuid = sqlx::query_scalar(
        "SELECT id FROM social.scene_stream_access WHERE streaming_key = $1 AND active",
    )
    .bind(&key)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(
        streamer_id.starts_with(&format!("stream:{}:{current}:", scene.place_id)),
        "{streamer_id}"
    );
    let (status, bot_body) = h
        .post("/cast/presentation-bot-token", json!({"streamingKey": key}))
        .await;
    assert_eq!(status, StatusCode::OK, "{bot_body}");
    // The bot identity is not returned; mint one as the route does.
    let bot = format!(
        "presentation-bot:{}:{current}:{}",
        scene.room,
        Uuid::new_v4()
    );
    // Minted before identities carried the credential.
    let untagged = format!("stream:{}:{}", scene.place_id, Uuid::new_v4());
    let visitor = address_of(OUTSIDER_KEY);
    // A replacement credential: its publishers stay, whenever they joined,
    // and a token of the old key that connects late still goes.
    let replacement = Uuid::new_v4();
    let fresh = format!("stream:{}:{replacement}:{}", scene.place_id, Uuid::new_v4());
    let pending = format!("stream:{}:{replacement}:{}", scene.place_id, Uuid::new_v4());
    let early = chrono::Utc::now().timestamp() - 3600;
    let late = chrono::Utc::now().timestamp() + 3600;
    for (id, at) in [
        (fresh.as_str(), early),
        (streamer_id.as_str(), late),
        (bot.as_str(), late),
        (untagged.as_str(), early),
        (visitor.as_str(), early),
    ] {
        h.livekit.join(&scene.room, id, at);
    }
    let mut meta = h.livekit.metadata(&scene.room);
    // `pending` holds a token of the replacement and has not connected yet.
    meta["presenters"] = json!([streamer_id, untagged, visitor, pending]);
    h.livekit.set_metadata(&scene.room, meta);

    crate::gatekeeper::streaming::cast::evict_revoked_publishers(
        &h.state,
        &scene.place_id,
        &scene.room,
        Some(replacement),
    )
    .await
    .unwrap();
    let mut connected = h.livekit.connected(&scene.room);
    connected.sort();
    let mut expected = vec![fresh.clone(), visitor.clone()];
    expected.sort();
    assert_eq!(connected, expected);
    assert_eq!(
        h.livekit.metadata(&scene.room)["presenters"],
        json!([visitor, pending])
    );

    // While the key is active, its publisher joins; once revoked, a late
    // join with its still-valid token is removed by the join guard.
    h.livekit.join(&scene.room, &streamer_id, late);
    assert!(
        !crate::gatekeeper::streaming::cast::refuse_revoked_publisher(
            &h.state,
            &scene.room,
            &streamer_id
        )
        .await
        .unwrap()
    );
    assert!(h.livekit.connected(&scene.room).contains(&streamer_id));
    assert!(
        crate::gatekeeper::streaming::cast::refuse_revoked_publisher(&h.state, &scene.room, &fresh)
            .await
            .unwrap(),
        "a publisher of a credential that is not the room's active one"
    );

    // Revoked through the route, with no successor: the durable cleanup
    // removes every publisher it admitted, and the key admits no one.
    let (status, body) = h
        .signed(KEY, "DELETE", "/scene-stream-access", &metadata, None)
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT, "{body}");
    assert_eq!(h.livekit.connected(&scene.room), vec![visitor]);
    assert_eq!(h.livekit.deleted_ingresses.load(Ordering::SeqCst), 1);
    let (status, _) = h
        .post(
            "/cast/streamer-token",
            json!({"token": key, "identity": "Ada"}),
        )
        .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

/// A room-service failure while evicting the WebRTC publishers never holds
/// back the RTMP revocation: the ingress is deleted first, the route answers
/// once the revocation is stored, and the eviction is retried on its own.
#[tokio::test]
async fn an_eviction_failure_still_revokes_the_ingress() {
    let Some(pool) = scratch().await else { return };
    let scene = genesis_scene();
    seed_genesis_place(&pool, &scene, None).await;
    let room = resolved_room(RealmId::Shared("main".into()), &scene.id, &scene.parcel);
    let h = harness(pool.clone(), Some(room), PermissionDecision::Allowed, None).await;
    let metadata = scene_metadata(&scene);
    let (status, link) = h
        .signed(KEY, "GET", "/cast/generate-stream-link", &metadata, None)
        .await;
    assert_eq!(status, StatusCode::OK, "{link}");
    let key = link["streamingKey"].as_str().unwrap().to_owned();
    let (status, streamer) = h
        .post(
            "/cast/streamer-token",
            json!({"token": key, "identity": "Ada"}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{streamer}");
    let streamer_id = streamer["identity"].as_str().unwrap().to_owned();
    h.livekit
        .join(&scene.room, &streamer_id, chrono::Utc::now().timestamp());

    h.livekit.fail_participants.store(true, Ordering::SeqCst);
    let (status, body) = h
        .signed(KEY, "DELETE", "/scene-stream-access", &metadata, None)
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT, "{body}");
    assert_eq!(h.livekit.deleted_ingresses.load(Ordering::SeqCst), 1);
    let ingress_deleted: bool = sqlx::query_scalar(
        "SELECT ingress_deleted FROM social.scene_stream_cleanup WHERE place_id = $1",
    )
    .bind(&scene.place_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(
        ingress_deleted,
        "the eviction is retried apart from the ingress"
    );
    assert_eq!(h.livekit.connected(&scene.room), vec![streamer_id.clone()]);

    // The room service back: the retry evicts and skips the deleted ingress.
    h.livekit.fail_participants.store(false, Ordering::SeqCst);
    sqlx::query("UPDATE social.scene_stream_cleanup SET due_at = timezone('UTC', now()) WHERE place_id = $1")
        .bind(&scene.place_id)
        .execute(&pool)
        .await
        .unwrap();
    crate::gatekeeper::streaming::drain_cleanup_for_test(&h.state)
        .await
        .unwrap();
    assert!(h.livekit.connected(&scene.room).is_empty());
    assert_eq!(h.livekit.deleted_ingresses.load(Ordering::SeqCst), 1);
}

/// An expired credential the expiry job has not deactivated yet does not
/// make a room count as casting.
#[tokio::test]
async fn an_expired_credential_does_not_promote_admins_on_join() {
    let Some(pool) = scratch().await else { return };
    let scene = genesis_scene();
    seed_genesis_place(&pool, &scene, None).await;
    let room = resolved_room(RealmId::Shared("main".into()), &scene.id, &scene.parcel);
    let h = harness(pool.clone(), Some(room), PermissionDecision::Allowed, None).await;
    seed_access(
        &pool,
        &scene.place_id,
        Some(&scene.room),
        chrono::Duration::days(1),
    )
    .await;
    sqlx::query("UPDATE social.scene_stream_access SET expiration_time = timezone('UTC', now()) - interval '1 minute' WHERE place_id = $1 AND active")
        .bind(&scene.place_id)
        .execute(&pool)
        .await
        .unwrap();
    let admin = Address::parse(&address_of(KEY)).unwrap();
    let promoted = crate::gatekeeper::streaming::cast::add_admin_presenter_on_join(
        &h.state,
        &scene.room,
        &admin,
    )
    .await
    .unwrap();
    assert!(!promoted);
}

/// A reset whose new credential cannot be stored: the old key stays in
/// force, no cleanup is recorded for it, and the new ingress is deleted.
#[tokio::test]
async fn a_reset_that_cannot_store_its_credential_keeps_the_old_key() {
    // Its trigger on a shared table would block or deadlock other tests.
    let Some(database) =
        crate::db::scratch::OwnedDatabase::new("castrefuse", crate::db::scratch::Schema::Full, 8)
            .await
    else {
        return;
    };
    let pool = database.pool.clone();
    let scene = genesis_scene();
    seed_genesis_place(&pool, &scene, None).await;
    let room = resolved_room(RealmId::Shared("main".into()), &scene.id, &scene.parcel);
    let h = harness(pool.clone(), Some(room), PermissionDecision::Allowed, None).await;
    let metadata = scene_metadata(&scene);
    let (status, first) = h
        .signed(KEY, "GET", "/cast/generate-stream-link", &metadata, None)
        .await;
    assert_eq!(status, StatusCode::OK, "{first}");
    let active = || async {
        sqlx::query_scalar::<_, Uuid>(
            "SELECT id FROM social.scene_stream_access WHERE place_id = $1 AND active",
        )
        .bind(&scene.place_id)
        .fetch_one(&pool)
        .await
        .unwrap()
    };
    let old_access = active().await;
    // Refuses this place's next insert only, inside the replacement's transaction.
    let function = format!("cast_refuse_{}", tag());
    sqlx::raw_sql(sqlx::AssertSqlSafe(format!(
        "CREATE FUNCTION social.{function}() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN \
         IF NEW.place_id = '{place}' THEN RAISE EXCEPTION 'refused by test'; END IF; RETURN NEW; END $$; \
         CREATE TRIGGER {function} BEFORE INSERT ON social.scene_stream_access \
         FOR EACH ROW EXECUTE FUNCTION social.{function}();",
        place = scene.place_id
    )))
    .execute(&pool)
    .await
    .unwrap();
    let (status, _) = h
        .signed(KEY, "PUT", "/scene-stream-access", &metadata, None)
        .await;
    sqlx::raw_sql(sqlx::AssertSqlSafe(format!(
        "DROP TRIGGER {function} ON social.scene_stream_access; DROP FUNCTION social.{function}();"
    )))
    .execute(&pool)
    .await
    .unwrap();
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(active().await, old_access);
    let pending: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM social.scene_stream_cleanup WHERE access_id = $1)",
    )
    .bind(old_access)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(!pending);
    // The new ingress was minted and then deleted; the old key still admits a streamer.
    assert_eq!(h.livekit.ingresses.load(Ordering::SeqCst), 2);
    assert_eq!(h.livekit.deleted_ingresses.load(Ordering::SeqCst), 1);
    let (status, _) = h
        .post(
            "/cast/streamer-token",
            json!({"token": first["streamingKey"], "identity": "Ada"}),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    drop(h);
    database.drop().await;
}

/// Upstream mints the replacement's ingress before revoking the old one: a
/// LiveKit failure while replacing an expired key leaves that key in place.
#[tokio::test]
async fn a_failed_replacement_keeps_the_old_credential() {
    let Some(pool) = scratch().await else { return };
    let scene = genesis_scene();
    seed_genesis_place(&pool, &scene, None).await;
    let room = resolved_room(RealmId::Shared("main".into()), &scene.id, &scene.parcel);
    let h = harness(pool.clone(), Some(room), PermissionDecision::Allowed, None).await;
    let path = "/cast/generate-stream-link";
    let (status, first) = h
        .signed(KEY, "GET", path, &scene_metadata(&scene), None)
        .await;
    assert_eq!(status, StatusCode::OK, "{first}");
    sqlx::query("UPDATE social.scene_stream_access SET expiration_time = timezone('UTC', now()) - interval '1 minute' WHERE place_id = $1 AND active")
        .bind(&scene.place_id)
        .execute(&pool)
        .await
        .unwrap();
    h.livekit.fail_ingress.store(true, Ordering::SeqCst);
    let (status, _) = h
        .signed(KEY, "GET", path, &scene_metadata(&scene), None)
        .await;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
    let kept: String = sqlx::query_scalar(
        "SELECT streaming_key FROM social.scene_stream_access WHERE place_id = $1 AND active",
    )
    .bind(&scene.place_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(kept, first["streamingKey"].as_str().unwrap());
    assert_eq!(h.livekit.deleted_ingresses.load(Ordering::SeqCst), 0);

    // LiveKit back: the replacement goes through and the old key is refused.
    h.livekit.fail_ingress.store(false, Ordering::SeqCst);
    let (status, renewed) = h
        .signed(KEY, "GET", path, &scene_metadata(&scene), None)
        .await;
    assert_eq!(status, StatusCode::OK, "{renewed}");
    assert_ne!(renewed["streamingKey"], first["streamingKey"]);
    assert_eq!(h.livekit.deleted_ingresses.load(Ordering::SeqCst), 1);
    for (path, body) in [
        (
            "/cast/streamer-token",
            json!({"token": first["streamingKey"], "identity": "Ada"}),
        ),
        (
            "/cast/presentation-bot-token",
            json!({"streamingKey": first["streamingKey"]}),
        ),
    ] {
        let (status, _) = h.post(path, body).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "{path}");
    }
    let (status, _) = h
        .send(
            "GET",
            &format!(
                "/cast/stream-info/{}",
                first["streamingKey"].as_str().unwrap()
            ),
            HeaderMap::new(),
            None,
        )
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

/// The room API failing answers upstream's bare 500; LiveKit unconfigured
/// answers 503 on the routes that need it.
#[tokio::test]
async fn livekit_failures_and_absence_on_the_cast_routes() {
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
    let key = seed_access(
        &pool,
        &scene.place_id,
        Some(&scene.room),
        chrono::Duration::days(1),
    )
    .await;
    h.livekit.fail_rooms.store(true, Ordering::SeqCst);
    let (status, body) = h
        .post(
            "/cast/streamer-token",
            json!({"token": key, "identity": "Ada"}),
        )
        .await;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR, "{body}");
    assert_eq!(body, json!({"error": "Internal Server Error"}));

    let off = harness_tuned(
        pool.clone(),
        Some(room),
        StubPolicy {
            admin: PermissionDecision::Allowed,
            ..Default::default()
        },
        None,
        |state| state.livekit.configured = false,
    )
    .await;
    let (status, body) = off
        .post(
            "/cast/streamer-token",
            json!({"token": key, "identity": "Ada"}),
        )
        .await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{body}");
    let (status, body) = off
        .signed(
            KEY,
            "GET",
            "/cast/generate-stream-link",
            &scene_metadata(&scene),
            None,
        )
        .await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{body}");
}

#[tokio::test]
async fn a_scene_admin_joining_a_casting_room_becomes_a_presenter() {
    let Some(pool) = scratch().await else { return };
    let scene = genesis_scene();
    seed_genesis_place(&pool, &scene, None).await;
    let room = resolved_room(RealmId::Shared("main".into()), &scene.id, &scene.parcel);
    let h = harness(pool.clone(), Some(room), PermissionDecision::Allowed, None).await;
    let admin = Address::parse(&address_of(KEY)).unwrap();
    let add = crate::gatekeeper::streaming::cast::add_admin_presenter_on_join;

    // No Cast credential for the room: nothing is read from LiveKit or written.
    assert!(!add(&h.state, &scene.room, &admin).await.unwrap());
    assert_eq!(h.livekit.metadata(&scene.room), Value::Null);

    seed_access(
        &pool,
        &scene.place_id,
        Some(&scene.room),
        chrono::Duration::days(1),
    )
    .await;
    h.livekit.set_metadata(
        &scene.room,
        json!({"presenters": ["stream:p:x"], "sceneAdmins": []}),
    );
    assert!(add(&h.state, &scene.room, &admin).await.unwrap());
    // Twice is once, and the rest of the metadata survives.
    assert!(add(&h.state, &scene.room, &admin).await.unwrap());
    assert_eq!(
        h.livekit.metadata(&scene.room),
        json!({"presenters": ["stream:p:x", admin.as_str()], "sceneAdmins": []})
    );
}

#[tokio::test]
async fn a_visitor_joining_a_casting_room_is_not_a_presenter() {
    let Some(pool) = scratch().await else { return };
    let scene = genesis_scene();
    seed_genesis_place(&pool, &scene, None).await;
    let room = resolved_room(RealmId::Shared("main".into()), &scene.id, &scene.parcel);
    let h = harness(
        pool.clone(),
        Some(room),
        PermissionDecision::refused("not an admin"),
        None,
    )
    .await;
    seed_access(
        &pool,
        &scene.place_id,
        Some(&scene.room),
        chrono::Duration::days(1),
    )
    .await;
    h.livekit.set_metadata(&scene.room, json!({}));
    let visitor = Address::parse(&address_of(OUTSIDER_KEY)).unwrap();
    let add = crate::gatekeeper::streaming::cast::add_admin_presenter_on_join;
    assert!(!add(&h.state, &scene.room, &visitor).await.unwrap());
    assert_eq!(h.livekit.metadata(&scene.room), json!({}));
}

/// Through `/get-scene-adapter`: the join answers at once and the admin
/// shows up in the room's presenters right after.
#[tokio::test]
async fn the_scene_adapter_adds_a_joining_admin_to_the_presenters() {
    let Some(pool) = scratch().await else { return };
    let scene = genesis_scene();
    seed_genesis_place(&pool, &scene, None).await;
    let room = resolved_room(RealmId::Shared("main".into()), &scene.id, &scene.parcel);
    let h = harness(pool.clone(), Some(room), PermissionDecision::Allowed, None).await;
    seed_access(
        &pool,
        &scene.place_id,
        Some(&scene.room),
        chrono::Duration::days(1),
    )
    .await;
    let metadata = json!({
        "signer": "dcl:explorer",
        "realmName": "main",
        "sceneId": scene.id,
        "parcel": scene.parcel,
        "realm": { "serverName": "main", "hostname": "peer.example", "protocol": "v3" },
    });
    let (status, body) = h
        .signed(
            KEY,
            "POST",
            "/get-scene-adapter",
            &metadata,
            Some(json!({})),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(body["adapter"].as_str().unwrap().starts_with("livekit:"));
    let admin = address_of(KEY);
    let mut presenters = Value::Null;
    for _ in 0..100 {
        presenters = h.livekit.metadata(&scene.room)["presenters"].clone();
        if presenters != Value::Null {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
    assert_eq!(presenters, json!([admin]));
}

#[test]
fn display_names_drop_invisible_characters_and_never_carry_another_wallet() {
    let own = Address::parse(&address_of(KEY)).unwrap();
    let other = address_of(OUTSIDER_KEY);
    let guest = format!("Guest#{}", &own.as_str()[38..]);
    assert_eq!(shown_name("  Ada \t Lovelace ", Some(&own)), "Ada Lovelace");
    // Direction overrides, zero-width marks, the grapheme joiner, variation
    // selectors and tags render nothing.
    assert_eq!(
        shown_name("Mod\u{202e}rotareD\u{200b}", Some(&own)),
        "ModrotareD"
    );
    assert_eq!(shown_name("A\u{34f}d\u{fe0f}a\u{e0041}", Some(&own)), "Ada");
    // Names that render blank (fillers, the blank braille cell) fall back.
    for blank in [
        "\u{200b}\u{feff}",
        "\u{3164}\u{3164}",
        "\u{115f}\u{1160}",
        "\u{ffa0}",
        "\u{2800}",
    ] {
        assert_eq!(shown_name(blank, Some(&own)), guest, "{blank:?}");
    }
    // Another wallet's address, bare, inside a name, split by an invisible
    // character, through lookalike letters or abbreviated, is not shown.
    let (head, tail) = other.split_at(22);
    let lookalike = other
        .replacen("0x", "0\u{445}", 1)
        .replace('a', "\u{430}")
        .replace('e', "\u{435}");
    let abbreviated = format!("{}…{}", &other[..6], &other[38..]);
    for spoof in [
        other.clone(),
        format!("admin {}", other.to_uppercase().replace("0X", "0x")),
        format!("{head}\u{34f}{tail}"),
        lookalike,
        abbreviated.clone(),
        abbreviated.replace('…', "..."),
        format!("\u{ff10}\u{ff58}{}", &other[2..]),
    ] {
        assert_eq!(shown_name(&spoof, Some(&own)), guest, "{spoof:?}");
    }
    assert_eq!(shown_name(&other, None), "Streamer");
    // Another viewer's default name is not shown; the caller's own is.
    assert_eq!(shown_name("Guest#beef", Some(&own)), guest);
    assert_eq!(shown_name(&guest, Some(&own)), guest);
    // The caller's own address, full or abbreviated, is theirs to show.
    assert_eq!(shown_name(own.as_str(), Some(&own)), own.as_str());
    let own_short = format!("{}…{}", &own.as_str()[..6], &own.as_str()[38..]);
    // NFKC spells the ellipsis as three dots.
    assert_eq!(
        shown_name(&own_short, Some(&own)),
        own_short.replace('…', "...")
    );
    assert_eq!(shown_name("Viewer #1", Some(&own)), "Viewer #1");
    assert_eq!(shown_name("Oxford Room", Some(&own)), "Oxford Room");
}

/// A streamer identity of `place` whose token was minted `age_ms` ago, as
/// `streamer_token` mints them (a version 7 UUID carries the time).
fn streamer_minted_ago(place: &str, age_ms: i64) -> String {
    let mut bytes = *Uuid::new_v4().as_bytes();
    let ms = ((chrono::Utc::now().timestamp_millis() - age_ms) as u64).to_be_bytes();
    bytes[..6].copy_from_slice(&ms[2..]);
    bytes[6] = (bytes[6] & 0x0f) | 0x70;
    format!(
        "stream:{place}:{}:{}",
        Uuid::new_v4(),
        Uuid::from_bytes(bytes)
    )
}

#[tokio::test]
async fn streamer_presenters_stay_bounded_and_follow_the_streamers_session() {
    let Some(pool) = scratch().await else { return };
    let scene = genesis_scene();
    seed_genesis_place(&pool, &scene, None).await;
    let room = resolved_room(RealmId::Shared("main".into()), &scene.id, &scene.parcel);
    let h = harness(pool.clone(), Some(room), PermissionDecision::Allowed, None).await;
    let key = seed_access(
        &pool,
        &scene.place_id,
        Some(&scene.room),
        chrono::Duration::days(1),
    )
    .await;
    // Ten ids whose token expired unused, 32 recent ones (the first connected), and a wallet.
    let expired: Vec<String> = (0..10)
        .map(|_| streamer_minted_ago(&scene.place_id, 10 * 60 * 1000))
        .collect();
    let recent: Vec<String> = (0..32)
        .map(|_| streamer_minted_ago(&scene.place_id, 60 * 1000))
        .collect();
    let admin = address_of(KEY);
    let mut listed: Vec<Value> = expired.iter().chain(&recent).map(|id| json!(id)).collect();
    listed.push(json!(admin));
    h.livekit
        .set_metadata(&scene.room, json!({ "presenters": listed }));
    h.livekit
        .join(&scene.room, &recent[0], chrono::Utc::now().timestamp());

    let (status, body) = h
        .post(
            "/cast/streamer-token",
            json!({"token": key, "identity": address_of(OUTSIDER_KEY)}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let streamer = body["identity"].as_str().unwrap().to_owned();
    // An address is not a streamer's name to show.
    assert_eq!(claims(&body["token"])["name"], "Streamer");
    let presenters: Vec<String> =
        serde_json::from_value(h.livekit.metadata(&scene.room)["presenters"].clone()).unwrap();
    // Expired ids went first; then the oldest recent one not in the room made way.
    assert!(expired.iter().all(|id| !presenters.contains(id)));
    assert_eq!(
        presenters
            .iter()
            .filter(|id| id.starts_with("stream:"))
            .count(),
        32
    );
    assert!(presenters.contains(&recent[0]));
    assert!(!presenters.contains(&recent[1]));
    assert!(presenters.contains(&streamer));
    assert!(presenters.contains(&admin));

    // Through the LiveKit webhook: the old session of a full reconnect leaves
    // with DUPLICATE_IDENTITY and keeps the streamer; a real leave removes it,
    // and a rejoin with the live credential puts it back.
    let event = |id: &str, kind: &str, at: i64, reason: Option<&str>| {
        let mut participant = json!({"sid": format!("PA_{id}"), "identity": streamer});
        if let Some(reason) = reason {
            participant["disconnectReason"] = json!(reason);
        }
        json!({"id": id, "event": kind, "createdAt": at, "room": {"name": scene.room}, "participant": participant})
    };
    let dispatch = crate::gatekeeper::webhook::dispatch_for_tests;
    let listed = |h: &Harness| {
        h.livekit.metadata(&scene.room)["presenters"]
            .as_array()
            .unwrap()
            .contains(&json!(streamer))
    };
    dispatch(
        &h.state,
        &event(
            &format!("dup-{}", tag()),
            "participant_left",
            100,
            Some("DUPLICATE_IDENTITY"),
        ),
    )
    .await
    .unwrap();
    assert!(
        listed(&h),
        "a full reconnect's old session keeps the presenter"
    );
    dispatch(
        &h.state,
        &event(
            &format!("left-{}", tag()),
            "participant_left",
            101,
            Some("CLIENT_INITIATED"),
        ),
    )
    .await
    .unwrap();
    assert!(!listed(&h), "a streamer that left is no longer a presenter");
    dispatch(
        &h.state,
        &event(&format!("join-{}", tag()), "participant_joined", 102, None),
    )
    .await
    .unwrap();
    assert!(
        listed(&h),
        "a streamer that rejoins with a live credential is a presenter again"
    );
    assert!(
        h.livekit.metadata(&scene.room)["presenters"]
            .as_array()
            .unwrap()
            .contains(&json!(admin))
    );
}

/// A concurrent change while a reset stores its credential (here the insert
/// is silently skipped, as when another credential won the place): 409, the
/// old key stays, no cleanup is recorded, and the new ingress is deleted.
#[tokio::test]
async fn a_reset_that_loses_to_a_concurrent_change_keeps_the_old_key() {
    // Its trigger on a shared table would block or deadlock other tests.
    let Some(database) =
        crate::db::scratch::OwnedDatabase::new("castskip", crate::db::scratch::Schema::Full, 8)
            .await
    else {
        return;
    };
    let pool = database.pool.clone();
    let scene = genesis_scene();
    seed_genesis_place(&pool, &scene, None).await;
    let room = resolved_room(RealmId::Shared("main".into()), &scene.id, &scene.parcel);
    let h = harness(pool.clone(), Some(room), PermissionDecision::Allowed, None).await;
    let metadata = scene_metadata(&scene);
    let (status, first) = h
        .signed(KEY, "GET", "/cast/generate-stream-link", &metadata, None)
        .await;
    assert_eq!(status, StatusCode::OK, "{first}");
    let function = format!("cast_skip_{}", tag());
    sqlx::raw_sql(sqlx::AssertSqlSafe(format!(
        "CREATE FUNCTION social.{function}() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN \
         IF NEW.place_id = '{place}' THEN RETURN NULL; END IF; RETURN NEW; END $$; \
         CREATE TRIGGER {function} BEFORE INSERT ON social.scene_stream_access \
         FOR EACH ROW EXECUTE FUNCTION social.{function}();",
        place = scene.place_id
    )))
    .execute(&pool)
    .await
    .unwrap();
    let (status, body) = h
        .signed(KEY, "PUT", "/scene-stream-access", &metadata, None)
        .await;
    sqlx::raw_sql(sqlx::AssertSqlSafe(format!(
        "DROP TRIGGER {function} ON social.scene_stream_access; DROP FUNCTION social.{function}();"
    )))
    .execute(&pool)
    .await
    .unwrap();
    // The reset route answers its own failure text for anything but success.
    assert!(
        status.is_client_error() || status.is_server_error(),
        "{status} {body}"
    );
    let (kept, pending): (String, bool) = sqlx::query_as(
        "SELECT streaming_key, EXISTS(SELECT 1 FROM social.scene_stream_cleanup c WHERE c.access_id = a.id) \
         FROM social.scene_stream_access a WHERE place_id = $1 AND active",
    )
    .bind(&scene.place_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(kept, first["streamingKey"].as_str().unwrap());
    assert!(!pending);
    assert_eq!(h.livekit.ingresses.load(Ordering::SeqCst), 2);
    assert_eq!(h.livekit.deleted_ingresses.load(Ordering::SeqCst), 1);
    drop(h);
    database.drop().await;
}

/// A reset intent whose replacement never arrived (left by a crash before
/// replacements committed with their intent, or a replacement revoked in
/// turn) is delivered as a revocation once the grace hour is over, instead of
/// retrying forever; a recent one keeps waiting.
#[tokio::test]
async fn an_orphaned_reset_becomes_a_revocation_after_an_hour() {
    let Some(pool) = scratch().await else { return };
    let scene = genesis_scene();
    seed_genesis_place(&pool, &scene, None).await;
    let room = resolved_room(RealmId::Shared("main".into()), &scene.id, &scene.parcel);
    let h = harness(pool.clone(), Some(room), PermissionDecision::Allowed, None).await;
    let mut intents = Vec::new();
    for age in [chrono::Duration::hours(2), chrono::Duration::minutes(10)] {
        seed_access(
            &pool,
            &scene.place_id,
            Some(&scene.room),
            chrono::Duration::days(1),
        )
        .await;
        let id: Uuid = sqlx::query_scalar(
            "UPDATE social.scene_stream_access SET active = false WHERE place_id = $1 AND active RETURNING id",
        )
        .bind(&scene.place_id)
        .fetch_one(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO social.scene_stream_cleanup (access_id, place_id, notification_kind, occurred_at, due_at, ingress_deleted) \
             VALUES ($1, $2, 'streaming_key_reset', timezone('UTC', now()) - $3::interval, timezone('UTC', now()), true)",
        )
        .bind(id)
        .bind(&scene.place_id)
        .bind(format!("{} seconds", age.num_seconds()))
        .execute(&pool)
        .await
        .unwrap();
        intents.push(id);
    }
    crate::gatekeeper::streaming::drain_cleanup_for_test(&h.state)
        .await
        .unwrap();
    let kind = |id: Uuid| {
        let pool = pool.clone();
        async move {
            sqlx::query_scalar::<_, Option<String>>(
                "SELECT notification_kind FROM social.scene_stream_cleanup WHERE access_id = $1",
            )
            .bind(id)
            .fetch_one(&pool)
            .await
            .unwrap()
        }
    };
    assert_eq!(
        kind(intents[0]).await.as_deref(),
        Some("streaming_key_revoke")
    );
    assert_eq!(
        kind(intents[1]).await.as_deref(),
        Some("streaming_key_reset")
    );
}

/// Every extended holder of a place (LAND-document owner, operator, update
/// operator, update manager or approved-for-all address, lease holder, a
/// world-wide grantee) manages its stream access and Cast presenters and is
/// promoted on join, as upstream's `isSceneOwnerOrAdmin`; an outsider gets the
/// manager gate's refusal on every route, as does the lease holder on a place
/// outside its lease. The stub's grantees ignore parcels, so the world case
/// shows admission, not grant scoping.
#[tokio::test]
async fn extended_holders_manage_stream_access_and_cast_presenters() {
    use crate::gatekeeper::streaming::NOT_AUTHORIZED_TO_STREAM_MSG;
    use crate::gatekeeper::streaming::cast::{NOT_PRESENTER_ADMIN_MSG, NOT_STREAM_LINK_ADMIN_MSG};
    // Own database: a revocation whose delivery fails leaves a cleanup row another test's drain would pick up.
    let Some(database) =
        crate::db::scratch::OwnedDatabase::new("castholders", crate::db::scratch::Schema::Full, 8)
            .await
    else {
        return;
    };
    let pool = database.pool.clone();
    let holder = address_of(KEY);
    let lease_scene = genesis_scene();
    let (lease_url, lease_server) =
        lease_document(&checksummed(&holder), &lease_scene.parcel).await;

    let genesis = |scene: &Scene| {
        (
            resolved_room(RealmId::Shared("main".into()), &scene.id, &scene.parcel),
            scene_metadata(scene),
        )
    };
    let world = WorldName::parse(&format!("castholders{}.dcl.eth", tag())).unwrap();
    seed_world(&pool, &world).await;
    let world_scene = format!("bafkreiholders{}", tag());
    let world_place = format!("cast-holders-{}", tag());
    seed_place(
        &pool,
        &world_place,
        json!({"world": true, "world_name": world.display(), "positions": ["5,6"]}),
        "5,6",
    )
    .await;
    let world_metadata = json!({
        "signer": SCENE_SIGNER,
        "sceneId": world_scene,
        "parcel": "5,6",
        "realm": {"serverName": world.display(), "hostname": "worlds-content-server.example", "protocol": "v3"},
    });

    let mut cases = Vec::new();
    for (role, doc) in extended_land_roles(&holder) {
        let scene = genesis_scene();
        seed_genesis_place(&pool, &scene, None).await;
        cases.push((role, land_policy(doc), genesis(&scene), None));
    }
    seed_genesis_place(&pool, &lease_scene, None).await;
    cases.push((
        "lease holder",
        StubPolicy::default(),
        genesis(&lease_scene),
        Some(lease_url.clone()),
    ));
    cases.push((
        "world grantee",
        StubPolicy {
            grantees: Ok([Address::parse(&holder).unwrap()].into_iter().collect()),
            ..StubPolicy::default()
        },
        (
            resolved_room(RealmId::World(world.clone()), &world_scene, "5,6"),
            world_metadata,
        ),
        None,
    ));

    for (role, policy, (room, metadata), lease_url) in cases {
        let room_name = room.room.clone();
        let h = harness_tuned(pool.clone(), Some(room), policy, None, |state| {
            state.settings.lease_authorizations_url = lease_url;
        })
        .await;
        let outsider = async |method: &str, path: &str, message: &str| {
            let (status, body) = h.signed(OUTSIDER_KEY, method, path, &metadata, None).await;
            assert_eq!(
                status,
                StatusCode::FORBIDDEN,
                "{role}: outsider {method} {path}"
            );
            assert_eq!(
                body,
                json!({ "error": message }),
                "{role}: outsider {method} {path}"
            );
        };

        let access = "/scene-stream-access";
        outsider("GET", access, NOT_AUTHORIZED_TO_STREAM_MSG).await;
        let (status, body) = h.signed(KEY, "GET", access, &metadata, None).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{role}: {body}");
        outsider("POST", access, NOT_AUTHORIZED_TO_STREAM_MSG).await;
        let (status, body) = h.signed(KEY, "POST", access, &metadata, None).await;
        assert_eq!(status, StatusCode::OK, "{role}: {body}");
        outsider("PUT", access, NOT_AUTHORIZED_TO_STREAM_MSG).await;
        let (status, body) = h.signed(KEY, "PUT", access, &metadata, None).await;
        assert_eq!(status, StatusCode::OK, "{role} reset: {body}");

        let link = "/cast/generate-stream-link";
        outsider("GET", link, NOT_STREAM_LINK_ADMIN_MSG).await;
        let (status, body) = h.signed(KEY, "GET", link, &metadata, None).await;
        assert_eq!(status, StatusCode::OK, "{role}: {body}");

        let presenter = format!("/cast/presenters/{}", address_of(&random_key()));
        for (method, path) in [
            ("GET", "/cast/presenters"),
            ("PUT", presenter.as_str()),
            ("DELETE", presenter.as_str()),
        ] {
            outsider(method, path, NOT_PRESENTER_ADMIN_MSG).await;
            let (status, body) = h.signed(KEY, method, path, &metadata, None).await;
            assert_eq!(status, StatusCode::OK, "{role} {method} {path}: {body}");
        }

        let add = crate::gatekeeper::streaming::cast::add_admin_presenter_on_join;
        let stranger = Address::parse(&address_of(OUTSIDER_KEY)).unwrap();
        assert!(
            !add(&h.state, &room_name, &stranger).await.unwrap(),
            "{role}"
        );
        let joined = Address::parse(&holder).unwrap();
        assert!(add(&h.state, &room_name, &joined).await.unwrap(), "{role}");

        // A revocation skips the ban checks but not the manager gate.
        outsider("DELETE", access, NOT_AUTHORIZED_TO_STREAM_MSG).await;
        let (status, body) = h.signed(KEY, "DELETE", access, &metadata, None).await;
        assert_eq!(status, StatusCode::NO_CONTENT, "{role}: {body}");
    }

    // The lease holder is refused on a place outside its lease document.
    let unleased = genesis_scene();
    seed_genesis_place(&pool, &unleased, None).await;
    let (room, metadata) = genesis(&unleased);
    // An active stream, so the presenter route reaches its gate.
    seed_access(
        &pool,
        &unleased.place_id,
        Some(&room.room),
        chrono::Duration::hours(1),
    )
    .await;
    let h = harness_tuned(
        pool.clone(),
        Some(room),
        StubPolicy::default(),
        None,
        |state| {
            state.settings.lease_authorizations_url = Some(lease_url);
        },
    )
    .await;
    // A readable document fills the harness's lease cache, so the gate refuses from it.
    let leases = crate::gatekeeper::admission::scene::try_get_lease_holders_for_parcels;
    let leased = leases(&h.state, std::slice::from_ref(&lease_scene.parcel))
        .await
        .unwrap();
    assert!(leased.contains(&holder), "{leased:?}");
    let unleased_holders = leases(&h.state, std::slice::from_ref(&unleased.parcel))
        .await
        .unwrap();
    assert!(unleased_holders.is_empty(), "{unleased_holders:?}");
    let presenter = format!("/cast/presenters/{}", address_of(&random_key()));
    for (method, path, message) in [
        ("GET", "/scene-stream-access", NOT_AUTHORIZED_TO_STREAM_MSG),
        ("POST", "/scene-stream-access", NOT_AUTHORIZED_TO_STREAM_MSG),
        ("PUT", "/scene-stream-access", NOT_AUTHORIZED_TO_STREAM_MSG),
        (
            "GET",
            "/cast/generate-stream-link",
            NOT_STREAM_LINK_ADMIN_MSG,
        ),
        ("GET", "/cast/presenters", NOT_PRESENTER_ADMIN_MSG),
        ("PUT", presenter.as_str(), NOT_PRESENTER_ADMIN_MSG),
        ("DELETE", presenter.as_str(), NOT_PRESENTER_ADMIN_MSG),
        (
            "DELETE",
            "/scene-stream-access",
            NOT_AUTHORIZED_TO_STREAM_MSG,
        ),
    ] {
        let (status, body) = h.signed(KEY, method, path, &metadata, None).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "unleased {method} {path}");
        assert_eq!(
            body,
            json!({ "error": message }),
            "unleased {method} {path}"
        );
    }
    let add = crate::gatekeeper::streaming::cast::add_admin_presenter_on_join;
    let joined = Address::parse(&holder).unwrap();
    assert!(!add(&h.state, &unleased.room, &joined).await.unwrap());
    lease_server.abort();
    database.drop().await;
}
