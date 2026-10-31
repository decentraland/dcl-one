use super::*;
use crate::interfaces::test_support::{StubDirectory, StubPolicy, StubScenes};
use crate::rooms::{ActiveDeployment, ResolvedRoom};
use crate::world_identity::{Parcel, RealmId, SceneId, SceneNamespace};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};

fn resolved(room: &str) -> ResolvedRoom {
    let realm = RealmId::Shared("main".into());
    let scene = SceneId::parse("OriginalSceneHash").unwrap();
    let base = Parcel::parse("5,6").unwrap();
    ResolvedRoom {
        room: room.into(),
        realm: realm.clone(),
        scene_id: Some(scene.clone()),
        deployment: Some(ActiveDeployment {
            scene_id: scene,
            title: None,
            base,
            parcels: [base].into_iter().collect(),
            namespace: SceneNamespace::new(&realm, base, None),
            local: true,
        }),
    }
}

async fn closed_store_state() -> SharedState {
    let pool = sqlx::PgPool::connect_lazy("postgres://postgres@127.0.0.1:1/postgres").unwrap();
    pool.close().await;
    let mut state = CommsState::new(
        pool,
        crate::http::Outbound::for_tests(),
        super::super::Settings::for_tests(),
        None,
        None,
        Arc::new(StubPolicy::default()),
        Arc::new(StubScenes {
            room: Ok(resolved("scene:main:OriginalSceneHash")),
            ..Default::default()
        }),
        Arc::new(crate::presence::Presence::new()),
        Arc::new(StubDirectory::default()),
    );
    state.livekit.configured = true;
    state.livekit.api_key = "key".into();
    state.livekit.api_secret = "secret".into();
    Arc::new(state)
}

fn event(id: &str, kind: &str, room: &str, sid: &str) -> Value {
    serde_json::json!({
        "id": id, "event": kind, "createdAt": 1_800_000_000,
        "room": {"name": room, "sid": sid},
        "participant": {"identity": "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", "sid": "PA_1"}
    })
}

async fn deliver(state: SharedState, event: &Value, secret: &str) -> Result<Response, ApiError> {
    let body = serde_json::to_vec(event).unwrap();
    let mut headers = HeaderMap::new();
    headers.insert(
        "authorization",
        test_support::webhook_jwt(&body, "key", secret)
            .parse()
            .unwrap(),
    );
    livekit_webhook(State(state), headers, Bytes::from(body)).await
}

#[tokio::test]
async fn initial_metadata_room_start_retries_when_current_authority_is_unavailable() {
    let state = closed_store_state().await;
    let started = event(
        "start",
        "room_started",
        "scene:main:OriginalSceneHash",
        "RM_1",
    );
    for _ in 0..2 {
        let result = deliver(state.clone(), &started, "secret").await;
        assert_eq!(
            result.err().map(|e| e.code),
            Some(503),
            "a room start must await current metadata and retain the same event for retry"
        );
    }
    assert!(
        state
            .webhooks
            .state
            .lock()
            .unwrap()
            .applied
            .get("start")
            .is_none()
    );
}

#[tokio::test]
async fn initial_metadata_first_join_retries_without_waiting_for_periodic_scan() {
    let state = closed_store_state().await;
    let joined = event(
        "join",
        "participant_joined",
        "scene:main:OriginalSceneHash",
        "RM_1",
    );
    for _ in 0..2 {
        let result = deliver(state.clone(), &joined, "secret").await;
        assert_eq!(
            result.err().map(|e| e.code),
            Some(503),
            "first join must reach exact-scene metadata even before any periodic pass"
        );
    }
    assert!(
        state
            .webhooks
            .state
            .lock()
            .unwrap()
            .applied
            .get("join")
            .is_none()
    );
}

#[tokio::test]
async fn initial_metadata_signature_and_non_scene_exclusions_precede_authority_reads() {
    let state = closed_store_state().await;
    assert_eq!(
        deliver(
            state.clone(),
            &event(
                "bad",
                "room_started",
                "scene:main:OriginalSceneHash",
                "RM_1"
            ),
            "wrong"
        )
        .await
        .err()
        .map(|e| e.code),
        Some(401)
    );
    for (n, room) in [
        "world-mixed.dcl.eth",
        "scene:pReViEw:OriginalSceneHash",
        "scene:localpreview:OriginalSceneHash",
        "island-C1",
    ]
    .into_iter()
    .enumerate()
    {
        assert_eq!(
            deliver(
                state.clone(),
                &event(&format!("skip{n}"), "room_started", room, "RM_skip"),
                "secret"
            )
            .await
            .unwrap()
            .status(),
            axum::http::StatusCode::OK
        );
    }
    for invalid in [
        serde_json::json!({}),
        serde_json::json!({"name":""}),
        serde_json::json!({"name":"x".repeat(513)}),
        serde_json::json!({"name":"scene:main:OriginalSceneHash","sid":"x".repeat(129)}),
    ] {
        let mut started = event(
            "invalid-room",
            "room_started",
            "scene:main:OriginalSceneHash",
            "RM_1",
        );
        started["room"] = invalid;
        assert_eq!(
            deliver(state.clone(), &started, "secret")
                .await
                .err()
                .map(|e| e.code),
            Some(400)
        );
    }
}

struct ResolvingScenes {
    calls: Mutex<Vec<String>>,
    fail: AtomicBool,
}

#[async_trait::async_trait]
impl crate::interfaces::SceneIdentity for ResolvingScenes {
    async fn classify_room(
        &self,
        room: &str,
    ) -> Result<crate::rooms::RoomClassification, crate::rooms::ResolveError> {
        crate::interfaces::test_support::classify_fixture(self, room).await
    }
    async fn resolve(
        &self,
        request: crate::rooms::RoomRequest<'_>,
    ) -> Result<ResolvedRoom, crate::rooms::ResolveError> {
        let crate::rooms::RoomRequest::Adapter { realm, scene } = request else {
            panic!("test scene must be resolved by its complete shared identity");
        };
        let room = crate::rooms::gatekeeper_scene_room(realm.as_str(), scene.as_str());
        self.calls.lock().unwrap().push(room.clone());
        if self.fail.load(Ordering::SeqCst) {
            return Err(crate::rooms::ResolveError::Db(sqlx::Error::PoolClosed));
        }
        // Unresolved deployment is deliberately safe to acknowledge without SQL.
        // This observes event dispatch/coalescing independently of the DB guard.
        let mut room = resolved(&room);
        room.deployment = None;
        Ok(room)
    }

    async fn world_scene(
        &self,
        _: &crate::world_identity::WorldName,
        _: &SceneId,
    ) -> Result<Option<ActiveDeployment>, crate::rooms::ResolveError> {
        Ok(None)
    }

    async fn world_active_scene(
        &self,
        _: &crate::world_identity::WorldName,
    ) -> Result<Option<ActiveDeployment>, crate::rooms::ResolveError> {
        Ok(None)
    }
}

async fn resolving_state() -> (SharedState, Arc<ResolvingScenes>) {
    let mut state = closed_store_state().await;
    let scenes = Arc::new(ResolvingScenes {
        calls: Mutex::new(vec![]),
        fail: AtomicBool::new(false),
    });
    Arc::get_mut(&mut state).unwrap().scenes = scenes.clone();
    (state, scenes)
}

#[tokio::test(start_paused = true)]
async fn initial_metadata_unresolved_deployment_never_installs_success_cooldown() {
    let (state, scenes) = resolving_state().await;
    let room = "scene:main:OriginalSceneHash";
    let join = event("join-first", "participant_joined", room, "RM_first");
    deliver(state.clone(), &join, "secret").await.unwrap();
    let started = event("start-after", "room_started", room, "RM_first");
    deliver(state.clone(), &started, "secret").await.unwrap();
    deliver(state.clone(), &started, "secret").await.unwrap();
    assert_eq!(
        *scenes.calls.lock().unwrap(),
        [room, room],
        "an unresolved deployment is not a successful initialization"
    );
    deliver(
        state.clone(),
        &event("recreated", "room_started", room, "RM_second"),
        "secret",
    )
    .await
    .unwrap();
    assert_eq!(
        scenes.calls.lock().unwrap().len(),
        3,
        "recreation cannot inherit the old room's cooldown"
    );
    tokio::time::advance(std::time::Duration::from_secs(60)).await;
    deliver(
        state.clone(),
        &event("later", "participant_joined", room, "RM_second"),
        "secret",
    )
    .await
    .unwrap();
    assert_eq!(scenes.calls.lock().unwrap().len(), 4);
}

#[tokio::test]
async fn initial_metadata_concurrent_unresolved_room_events_keep_exact_identity() {
    let (state, scenes) = resolving_state().await;
    let room = "scene:main:OriginalSceneHash";
    let burst = (0..128).map(|n| {
        let state = state.clone();
        async move {
            deliver(
                state,
                &event(&format!("burst{n}"), "room_started", room, "RM_burst"),
                "secret",
            )
            .await
            .unwrap()
        }
    });
    futures::future::join_all(burst).await;
    assert_eq!(
        *scenes.calls.lock().unwrap(),
        vec![room; 128],
        "unresolved rooms cannot claim successful refreshes"
    );
}

#[tokio::test]
async fn initial_metadata_failed_refresh_does_not_cache_or_consume_signed_retry() {
    let (state, scenes) = resolving_state().await;
    let room = "scene:main:OriginalSceneHash";
    let started = event("retry", "room_started", room, "RM_retry");
    scenes.fail.store(true, Ordering::SeqCst);
    assert_eq!(
        deliver(state.clone(), &started, "secret")
            .await
            .err()
            .map(|e| e.code),
        Some(503)
    );
    scenes.fail.store(false, Ordering::SeqCst);
    deliver(state.clone(), &started, "secret").await.unwrap();
    deliver(state.clone(), &started, "secret").await.unwrap();
    assert_eq!(*scenes.calls.lock().unwrap(), [room, room]);
}

#[tokio::test]
async fn initial_metadata_rejected_session_order_cannot_trigger_a_refresh() {
    let (state, scenes) = resolving_state().await;
    let room = "scene:main:OriginalSceneHash";
    let left = event("left-before-join", "participant_left", room, "RM_order");
    deliver(state.clone(), &left, "secret").await.unwrap();
    let join = event("stale-join", "participant_joined", room, "RM_order");
    deliver(state.clone(), &join, "secret").await.unwrap();
    assert!(
        scenes.calls.lock().unwrap().is_empty(),
        "equal-second leave precedence must hold before metadata effects"
    );
}

#[tokio::test(start_paused = true)]
async fn initial_metadata_success_cache_is_incarnation_scoped_expiring_and_bounded() {
    let mut state = WebhookState::default();
    state.metadata.capacity = 8;
    state.metadata.insert(
        "scene:main:original".into(),
        ("RM_original".into(), tokio::time::Instant::now()),
    );
    assert!(!state.metadata_due("scene:main:original", Some("RM_original")));
    assert!(state.metadata_due("scene:main:original", Some("RM_recreated")));
    assert!(state.metadata_due("scene:main:sibling", Some("RM_original")));
    assert!(state.metadata_due("scene:main:original", None));
    assert!(
        !state.metadata_due("scene:main:original", Some("RM_original")),
        "a missing SID cannot remove a confirmed incarnation"
    );
    tokio::time::advance(std::time::Duration::from_secs(60)).await;
    assert!(state.metadata_due("scene:main:original", Some("RM_original")));
    for n in 0..1000 {
        state.metadata.insert(
            format!("scene:main:{n}"),
            (format!("RM_{n}"), tokio::time::Instant::now()),
        );
    }
    assert!(
        state.metadata_due("scene:main:0", Some("RM_0")),
        "evicted rooms refresh normally"
    );
    assert!(!state.metadata_due("scene:main:999", Some("RM_999")));
    assert_eq!(state.metadata.values.len(), 8);
}

#[derive(Default)]
struct RoomApi {
    rooms: Mutex<std::collections::HashMap<String, Value>>,
    requests: Mutex<Vec<(String, Value)>>,
    failure: std::sync::atomic::AtomicUsize,
    lease_failure: std::sync::atomic::AtomicUsize,
}

impl RoomApi {
    fn put(&self, room: &str, sid: &str, metadata: &str) {
        self.rooms.lock().unwrap().insert(
            room.into(),
            serde_json::json!({"name":room,"sid":sid,"metadata":metadata}),
        );
    }

    fn metadata(&self, room: &str) -> Value {
        serde_json::from_str(
            self.rooms.lock().unwrap()[room]["metadata"]
                .as_str()
                .unwrap(),
        )
        .unwrap()
    }
}

async fn room_api() -> (Arc<RoomApi>, String, tokio::task::JoinHandle<()>) {
    use axum::{Json, Router, extract::Path, http::StatusCode, routing::post};
    let api = Arc::new(RoomApi::default());
    let captured = api.clone();
    let app = Router::new().route(
        "/twirp/livekit.RoomService/{method}",
        post(move |Path(method): Path<String>, Json(body): Json<Value>| {
            let api = captured.clone();
            async move {
                api.requests
                    .lock()
                    .unwrap()
                    .push((method.clone(), body.clone()));
                if method == "ListRooms" {
                    let rooms = api.rooms.lock().unwrap();
                    let selected: Vec<Value> = body["names"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .filter_map(|name| rooms.get(name.as_str().unwrap()).cloned())
                        .collect();
                    return (StatusCode::OK, Json(serde_json::json!({"rooms":selected})));
                }
                assert_eq!(method, "UpdateRoomMetadata");
                match api.failure.load(Ordering::SeqCst) {
                    1 => {
                        return (
                            StatusCode::SERVICE_UNAVAILABLE,
                            Json(serde_json::json!({"code":"unavailable"})),
                        );
                    }
                    2 => {
                        return (
                            StatusCode::NOT_FOUND,
                            Json(serde_json::json!({"code":"not_found"})),
                        );
                    }
                    3 => return (StatusCode::OK, Json(serde_json::json!({}))),
                    4 => {
                        return (
                            StatusCode::OK,
                            Json(serde_json::json!({"name":body["room"], "sid":"RM_raced"})),
                        );
                    }
                    _ => {}
                }
                let mut rooms = api.rooms.lock().unwrap();
                let room = rooms.get_mut(body["room"].as_str().unwrap()).unwrap();
                room["metadata"] = body["metadata"].clone();
                (StatusCode::OK, Json(room.clone()))
            }
        }),
    );
    let leases = api.clone();
    let app = app.route("/leases", axum::routing::get(move || {
        let api = leases.clone();
        async move {
            match api.lease_failure.load(Ordering::SeqCst) {
                1 => (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({"error":"unavailable"}))),
                2 => (StatusCode::OK, Json(serde_json::json!({"wrong":"shape"}))),
                _ => (StatusCode::OK, Json(serde_json::json!([
                    {"addresses":["0xeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee"],"plots":["5,6"]},
                    {"addresses":["0x9999999999999999999999999999999999999999"],"plots":["9,9"]}
                ]))),
            }
        }
    }));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let task = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    (api, url, task)
}

#[tokio::test]
async fn initial_metadata_incarnation_merge_requires_a_confirmed_write_and_preserves_fields() {
    let (api, url, server) = room_api().await;
    let http = reqwest::Client::new();
    let service = super::super::livekit::RoomServiceClient::new(&http, &url, "key", "secret");
    let room = "scene:main:OriginalSceneHash";
    let fields = serde_json::json!({"sceneAdmins":["owner"],"bannedAddresses":["banned"]})
        .as_object()
        .unwrap()
        .clone();
    assert!(
        service
            .merge_room_metadata_for_incarnation(room, Some("RM_1"), fields.clone())
            .await
            .is_err()
    );
    api.put(room, "RM_2", r#"{"presenters":["retained"],"other":42}"#);
    assert!(
        service
            .merge_room_metadata_for_incarnation(room, Some("RM_1"), fields.clone())
            .await
            .unwrap()
            .is_none()
    );
    assert!(
        !api.requests
            .lock()
            .unwrap()
            .iter()
            .any(|(method, _)| method == "UpdateRoomMetadata")
    );
    for invalid in ["not-json", "[]", "null", "42"] {
        api.put(room, "RM_2", invalid);
        assert!(
            service
                .merge_room_metadata_for_incarnation(room, Some("RM_2"), fields.clone())
                .await
                .is_err()
        );
        assert_eq!(api.rooms.lock().unwrap()[room]["metadata"], invalid);
    }
    for failure in 1..=4 {
        api.put(room, "RM_2", r#"{"presenters":["retained"],"other":42}"#);
        api.failure.store(failure, Ordering::SeqCst);
        assert!(
            service
                .merge_room_metadata_for_incarnation(room, Some("RM_2"), fields.clone())
                .await
                .is_err()
        );
        assert_eq!(
            api.metadata(room),
            serde_json::json!({"presenters":["retained"],"other":42})
        );
    }
    api.failure.store(0, Ordering::SeqCst);
    assert_eq!(
        service
            .merge_room_metadata_for_incarnation(room, Some("RM_2"), fields.clone())
            .await
            .unwrap()
            .as_deref(),
        Some("RM_2")
    );
    assert_eq!(
        api.metadata(room),
        serde_json::json!({"presenters":["retained"],"other":42,"sceneAdmins":["owner"],"bannedAddresses":["banned"]})
    );
    for initial in ["", "{}"] {
        api.put(room, "RM_3", initial);
        assert_eq!(
            service
                .merge_room_metadata_for_incarnation(room, None, fields.clone())
                .await
                .unwrap()
                .as_deref(),
            Some("RM_3")
        );
        assert_eq!(
            api.metadata(room),
            serde_json::json!({"sceneAdmins":["owner"],"bannedAddresses":["banned"]})
        );
    }
    server.abort();
}

#[tokio::test]
async fn initial_metadata_configured_lease_failure_retries_without_changing_best_effort_listings() {
    use super::super::admission::scene::{
        get_lease_holders_for_parcels, try_get_lease_holders_for_parcels,
    };
    let (api, url, server) = room_api().await;
    for failure in [1, 2] {
        let mut state = closed_store_state().await;
        Arc::get_mut(&mut state)
            .unwrap()
            .settings
            .lease_authorizations_url = Some(format!("{url}/leases"));
        let parcels = vec!["5,6".into()];
        api.lease_failure.store(failure, Ordering::SeqCst);
        assert!(
            try_get_lease_holders_for_parcels(&state, &[])
                .await
                .unwrap()
                .is_empty()
        );
        assert_eq!(
            try_get_lease_holders_for_parcels(&state, &parcels)
                .await
                .err()
                .map(|e| e.code),
            Some(503)
        );
        assert!(
            get_lease_holders_for_parcels(&state, &parcels)
                .await
                .is_empty(),
            "unrelated listings retain best-effort semantics"
        );
        api.lease_failure.store(0, Ordering::SeqCst);
        assert_eq!(
            try_get_lease_holders_for_parcels(&state, &parcels)
                .await
                .unwrap(),
            ["0xeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee".to_owned()]
                .into_iter()
                .collect()
        );
    }
    let state = closed_store_state().await;
    assert!(
        try_get_lease_holders_for_parcels(&state, &["5,6".into()])
            .await
            .unwrap()
            .is_empty()
    );
    server.abort();
}

#[tokio::test]
async fn initial_metadata_scratch_first_join_recreation_burst_and_scene_authority() {
    let Some(pool) = crate::db::scratch::pool(4).await else {
        eprintln!("DB-GUARD: initial metadata scene authority requires CATALYST_TEST_DATABASE_URL");
        return;
    };
    let (api, url, server) = room_api().await;
    let owner = "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    let admin = "0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
    let protected = "0xcccccccccccccccccccccccccccccccccccccccc";
    let operator = "0xdddddddddddddddddddddddddddddddddddddddd";
    let lease = "0xeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee";
    let banned = "0xffffffffffffffffffffffffffffffffffffffff";
    let unrelated = "0x9999999999999999999999999999999999999999";
    for case in ["genesis", "single", "multi"] {
        let key = uuid::Uuid::new_v4().simple().to_string();
        let scene = SceneId::parse(&format!("OriginalCase{key}")).unwrap();
        let realm = if case == "genesis" {
            RealmId::Shared("main".into())
        } else {
            RealmId::World(
                crate::world_identity::WorldName::parse(&format!("{case}-{key}.dcl.eth")).unwrap(),
            )
        };
        let room = if let Some(world) = realm.world() {
            crate::rooms::worlds_service_scene_room(world, &scene)
        } else {
            crate::rooms::gatekeeper_scene_room("main", scene.as_str())
        };
        let base = Parcel::parse("5,6").unwrap();
        // A world scene's bans live under `<world>:<base>`, a Genesis one's under its id.
        let ban_place = match realm.world() {
            Some(world) => crate::access::SceneBanKey::world(world, base),
            None => crate::access::SceneBanKey::scene(&scene),
        };
        let policy = StubPolicy {
            world_owner: Ok(Some(crate::Address::parse(owner).unwrap())),
            protected: Ok([crate::Address::parse(protected).unwrap()]
                .into_iter()
                .collect()),
            land: Ok(Some(Arc::new(Some(crate::access::LandOperators {
                owner: owner.into(),
                operator: Some(operator.into()),
                ..Default::default()
            })))),
            ..Default::default()
        };
        let mut state = CommsState::new(
            pool.clone(),
            crate::http::Outbound::for_tests(),
            super::super::Settings::for_tests(),
            None,
            None,
            Arc::new(policy),
            Arc::new(StubScenes {
                room: Ok(ResolvedRoom {
                    room: room.clone(),
                    realm: realm.clone(),
                    scene_id: Some(scene.clone()),
                    deployment: Some(ActiveDeployment {
                        scene_id: scene.clone(),
                        title: None,
                        base,
                        parcels: [base].into_iter().collect(),
                        namespace: SceneNamespace::new(&realm, base, None),
                        local: true,
                    }),
                }),
                ..Default::default()
            }),
            Arc::new(crate::presence::Presence::new()),
            Arc::new(StubDirectory::default()),
        );
        state.livekit.configured = true;
        state.livekit.api_url = url.clone();
        state.livekit.api_key = "key".into();
        state.livekit.api_secret = "secret".into();
        state.settings.lease_authorizations_url = Some(format!("{url}/leases"));
        for (place, address) in [(scene.as_str(), admin), (realm.as_str(), unrelated)] {
            state
                .moderation
                .add_admin(
                    place,
                    &crate::Address::parse(address).unwrap(),
                    &crate::Address::parse(owner).unwrap(),
                )
                .await
                .unwrap();
            sqlx::query("INSERT INTO social.scene_bans (place_id,banned_address,banned_by) VALUES ($1,$2,$3)")
                .bind(if place == scene.as_str() {ban_place.as_str()} else {place}).bind(if place == scene.as_str() {banned} else {unrelated}).bind(owner).execute(&pool).await.unwrap();
        }
        for n in 0..205 {
            api.put(
                &format!("scene:main:other{n}"),
                &format!("RM_other{n}"),
                "{}",
            );
        }
        api.put(&room, "RM_initial", r#"{"presenters":["kept"],"other":42}"#);
        let mut state = Arc::new(state);
        let kind = if case == "genesis" {
            "room_started"
        } else {
            "participant_joined"
        };
        deliver(
            state.clone(),
            &event(&format!("first{key}"), kind, &room, "RM_initial"),
            "secret",
        )
        .await
        .unwrap();
        let mut admins = vec![
            owner,
            admin,
            if case == "genesis" {
                operator
            } else {
                protected
            },
        ];
        if case == "genesis" {
            admins.push(lease);
        }
        admins.sort();
        assert_eq!(
            api.metadata(&room),
            serde_json::json!({"sceneAdmins":admins,"bannedAddresses":[banned],"presenters":["kept"],"other":42})
        );
        let before = api.requests.lock().unwrap().len();
        let burst = (0..128).map(|n| {
            let state = state.clone();
            let room = room.clone();
            let id = format!("{key}burst{n}");
            async move {
                deliver(
                    state,
                    &event(&id, "room_started", &room, "RM_initial"),
                    "secret",
                )
                .await
                .unwrap()
            }
        });
        futures::future::join_all(burst).await;
        assert_eq!(
            api.requests.lock().unwrap().len(),
            before,
            "same-incarnation burst must share its confirmed initialization"
        );

        let current_owner = "0x1111111111111111111111111111111111111111";
        let current_protected = "0x2222222222222222222222222222222222222222";
        let current_ban = "0x3333333333333333333333333333333333333333";
        Arc::get_mut(&mut state).unwrap().policy = Arc::new(StubPolicy {
            world_owner: Ok(Some(crate::Address::parse(current_owner).unwrap())),
            protected: Ok([crate::Address::parse(current_protected).unwrap()]
                .into_iter()
                .collect()),
            land: Ok(Some(Arc::new(Some(crate::access::LandOperators {
                owner: current_owner.into(),
                operator: Some(current_protected.into()),
                ..Default::default()
            })))),
            ..Default::default()
        });
        assert_eq!(
            state
                .moderation
                .remove_admin(scene.as_str(), &crate::Address::parse(admin).unwrap())
                .await
                .unwrap(),
            1
        );
        sqlx::query("DELETE FROM social.scene_bans WHERE place_id = $1")
            .bind(ban_place.as_str())
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query(
            "INSERT INTO social.scene_bans (place_id,banned_address,banned_by) VALUES ($1,$2,$3)",
        )
        .bind(ban_place.as_str())
        .bind(current_ban)
        .bind(current_owner)
        .execute(&pool)
        .await
        .unwrap();
        api.put(
            &room,
            "RM_recreated",
            r#"{"presenters":["new-kept"],"other":43}"#,
        );
        deliver(
            state.clone(),
            &event(
                &format!("recreated{key}"),
                "participant_joined",
                &room,
                "RM_recreated",
            ),
            "secret",
        )
        .await
        .unwrap();
        let mut admins = vec![current_owner, current_protected];
        if case == "genesis" {
            admins.push(lease);
        }
        assert_eq!(
            api.metadata(&room),
            serde_json::json!({"sceneAdmins":admins,"bannedAddresses":[current_ban],"presenters":["new-kept"],"other":43})
        );
        let last_good = api.metadata(&room);
        api.put(&room, "RM_retry", &last_good.to_string());
        api.failure.store(1, Ordering::SeqCst);
        let retry = event(&format!("retry{key}"), "room_started", &room, "RM_retry");
        assert_eq!(
            deliver(state.clone(), &retry, "secret")
                .await
                .err()
                .map(|e| e.code),
            Some(503)
        );
        assert_eq!(api.metadata(&room), last_good);
        api.failure.store(0, Ordering::SeqCst);
        deliver(state.clone(), &retry, "secret").await.unwrap();
        assert_eq!(api.metadata(&room), last_good);
        if case == "genesis" {
            // Model expiry of the previous successful document cache.
            Arc::get_mut(&mut state).unwrap().caches =
                super::super::admission::scene::SceneCaches::default();
            api.put(&room, "RM_lease_retry", &last_good.to_string());
            api.lease_failure.store(1, Ordering::SeqCst);
            let lease_retry = event(
                &format!("lease-retry{key}"),
                "room_started",
                &room,
                "RM_lease_retry",
            );
            let before = api.requests.lock().unwrap().len();
            assert_eq!(
                deliver(state.clone(), &lease_retry, "secret")
                    .await
                    .err()
                    .map(|e| e.code),
                Some(503)
            );
            assert_eq!(
                api.metadata(&room),
                last_good,
                "configured lease outage must retain the last known lease-derived admin"
            );
            assert_eq!(
                api.requests.lock().unwrap().len(),
                before,
                "lease failure must precede SFU writes"
            );
            assert_ne!(
                state
                    .webhooks
                    .state
                    .lock()
                    .unwrap()
                    .metadata
                    .get(&room)
                    .unwrap()
                    .0,
                "RM_lease_retry"
            );
            api.lease_failure.store(0, Ordering::SeqCst);
            deliver(state.clone(), &lease_retry, "secret")
                .await
                .unwrap();
            assert_eq!(api.metadata(&room), last_good);
            assert_eq!(
                state
                    .webhooks
                    .state
                    .lock()
                    .unwrap()
                    .metadata
                    .get(&room)
                    .unwrap()
                    .0,
                "RM_lease_retry"
            );
        }
        assert!(
            api.requests
                .lock()
                .unwrap()
                .iter()
                .all(|(method, body)| method != "ListRooms"
                    || body["names"]
                        .as_array()
                        .is_some_and(|names| names.len() == 1)),
            "205 other rooms cannot introduce a periodic/global scan into event initialization"
        );
    }
    server.abort();
}
