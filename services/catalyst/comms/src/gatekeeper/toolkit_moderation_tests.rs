//! Pinned Admin Toolkit JSON writes exercised through the actual HTTP router.
use super::*;
use crate::AuthorityNotEstablished;
use crate::access::{Access, AccessType, LandOperators, SceneRef};
use crate::interfaces::SceneIdentity;
use crate::interfaces::{AccessPolicy, NameOwnerReader};
use crate::rooms::{ActiveDeployment, ResolveError, ResolvedRoom, RoomRequest};
use crate::world_identity::ParcelSet;
use crate::world_identity::{Parcel, RealmId, SceneId, WorldName};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};

struct ToolkitScenes;

fn resolved(realm: RealmId, id: &str, parcel: Parcel) -> ResolvedRoom {
    let scene = SceneId::parse(id).unwrap();
    let room = match &realm {
        RealmId::World(world) => crate::rooms::worlds_service_scene_room(world, &scene),
        RealmId::Shared(name) => crate::rooms::gatekeeper_scene_room(name, id),
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

#[async_trait::async_trait]
impl SceneIdentity for ToolkitScenes {
    async fn classify_room(
        &self,
        room: &str,
    ) -> Result<crate::rooms::RoomClassification, ResolveError> {
        crate::interfaces::test_support::classify_fixture(self, room).await
    }
    async fn resolve(&self, request: RoomRequest<'_>) -> Result<ResolvedRoom, ResolveError> {
        match request {
            RoomRequest::Parcel { realm, parcel } => {
                let id = match (
                    realm.as_str().to_lowercase().as_str(),
                    parcel.canonical().as_str(),
                ) {
                    ("main", "42,42") => "GenesisA",
                    ("single.eth", "0,0") => "SingleHash",
                    ("mixed.dcl.eth", "0,0") => "SceneA",
                    ("mixed.dcl.eth", "1,1") => "SceneB",
                    ("colon.dcl.eth", "2,2") => "Cid:Case",
                    _ => {
                        return Err(ResolveError::NoSceneAtParcel {
                            realm: realm.as_str().to_string(),
                            parcel: parcel.to_string(),
                        });
                    }
                };
                Ok(resolved(realm.clone(), id, parcel))
            }
            RoomRequest::WorldScene { world, scene } => {
                let dep = self.world_scene(world, scene).await?.ok_or_else(|| {
                    ResolveError::SceneNotInWorld {
                        world: world.to_string(),
                        scene: scene.to_string(),
                    }
                })?;
                Ok(resolved(
                    RealmId::World(world.clone()),
                    scene.as_str(),
                    dep.base,
                ))
            }
            RoomRequest::Adapter {
                realm: RealmId::Shared(name),
                scene,
            } if name == "main" && scene.as_str() == "GenesisA" => Ok(resolved(
                RealmId::Shared(name.clone()),
                "GenesisA",
                Parcel::parse("42,42").unwrap(),
            )),
            RoomRequest::Adapter {
                realm: RealmId::Shared(name),
                scene,
            } if name == "main" && scene.as_str() == "HistoricalGenesis" => Ok(resolved(
                RealmId::Shared(name.clone()),
                "HistoricalGenesis",
                Parcel::parse("42,42").unwrap(),
            )),
            RoomRequest::Adapter {
                realm: RealmId::World(world),
                scene,
            } => {
                let deployment = if scene.is_world_shaped() {
                    self.world_active_scene(world).await?
                } else {
                    self.world_scene(world, scene).await?
                }
                .ok_or_else(|| ResolveError::WorldHasNoScene(world.to_string()))?;
                Ok(resolved(
                    RealmId::World(world.clone()),
                    deployment.scene_id.as_str(),
                    deployment.base,
                ))
            }
            _ => Err(ResolveError::UnresolvedRoom(
                "unsupported fixture request".into(),
            )),
        }
    }
    async fn world_scene(
        &self,
        world: &WorldName,
        scene: &SceneId,
    ) -> Result<Option<ActiveDeployment>, ResolveError> {
        let parcel = match (world.key(), scene.as_str()) {
            ("single.eth", "SingleHash") | ("mixed.dcl.eth", "SceneA") => "0,0",
            ("mixed.dcl.eth", "SceneB") => "1,1",
            ("colon.dcl.eth", "Cid:Case") => "2,2",
            _ => return Ok(None),
        };
        Ok(resolved(
            RealmId::World(world.clone()),
            scene.as_str(),
            Parcel::parse(parcel).unwrap(),
        )
        .deployment)
    }
    async fn world_active_scene(
        &self,
        world: &WorldName,
    ) -> Result<Option<ActiveDeployment>, ResolveError> {
        self.world_scene(
            world,
            &SceneId::parse(if world.key() == "single.eth" {
                "SingleHash"
            } else {
                "SceneA"
            })
            .unwrap(),
        )
        .await
    }
}

fn metadata(realm: &str, scene: &str, parcel: &str) -> Value {
    json!({"signer":"decentraland-kernel-scene", "sceneId":scene, "parcel":parcel,
        "realm":{"serverName":realm,"hostname":if realm == "main" {"https://realm.example"} else {"https://worlds-content-server.example"}}})
}

async fn call(
    app: &axum::Router,
    method: &str,
    path: &str,
    meta: &Value,
    body: Value,
    key: &str,
) -> (StatusCode, Value) {
    let mut request = Request::builder()
        .method(method)
        .uri(path)
        .body(Body::from(body.to_string()))
        .unwrap();
    request.headers_mut().extend(
        signed(
            key,
            method,
            path.split('?').next().unwrap(),
            &meta.to_string(),
        )
        .await,
    );
    body_json(app.clone().oneshot(request).await.unwrap()).await
}

#[tokio::test]
async fn toolkit_body_only_writes_reach_authorization_without_content_type_or_place_id() {
    // Regression: required place_id and Json extraction used to reject all
    // four real caller requests before evaluating their signed scene authority.
    let mut state = comms_state(lazy_pool(), Settings::for_tests());
    state.scenes = Arc::new(ToolkitScenes);
    let app = super::super::router(Arc::new(state));
    let meta = metadata("MiXeD.dcl.eth", "SceneB", "1,1");
    let mut results = Vec::new();
    for (method, path, body) in [
        (
            "POST",
            "/scene-admin",
            json!({"admin":address_of(OUTSIDER_KEY)}),
        ),
        (
            "DELETE",
            "/scene-admin",
            json!({"admin":address_of(OUTSIDER_KEY)}),
        ),
        (
            "POST",
            "/scene-bans",
            json!({"banned_address":address_of(OUTSIDER_KEY)}),
        ),
        (
            "DELETE",
            "/scene-bans",
            json!({"banned_address":address_of(OUTSIDER_KEY)}),
        ),
    ] {
        let (status, value) = call(&app, method, path, &meta, body, KEY).await;
        results.push((method, path, status, value));
    }
    assert!(
        results.iter().all(|r| r.2 == StatusCode::FORBIDDEN),
        "{results:?}"
    );
}

#[tokio::test]
async fn toolkit_signed_scope_rejects_malformed_cross_scene_and_unsigned_redirects_before_authority()
 {
    let mut state = comms_state(lazy_pool(), Settings::for_tests());
    state.scenes = Arc::new(ToolkitScenes);
    let app = super::super::router(Arc::new(state));
    let valid = metadata("MiXeD.dcl.eth", "SceneB", "1,1");
    let mut bad_metadata = Vec::new();
    for field in ["realm", "sceneId"] {
        let mut bad = valid.clone();
        bad.as_object_mut().unwrap().remove(field);
        bad_metadata.push(bad);
    }
    for (pointer, value) in [
        ("/realm", json!(false)),
        ("/realm/serverName", json!(" mixed.dcl.eth")),
        ("/realm/hostname", json!("%%%")),
        ("/sceneId", json!(" SceneB")),
        ("/sceneId", json!("SceneA")),
        ("/sceneId", json!("NoSuchScene")),
        ("/parcel", json!("0,0")),
        ("/parcel", json!("garbage")),
        ("/realm/serverName", json!("other.dcl.eth")),
    ] {
        let mut bad = valid.clone();
        *bad.pointer_mut(pointer).unwrap() = value;
        bad_metadata.push(bad);
    }
    let mut conflict = valid.clone();
    conflict["realmName"] = json!("other.eth");
    bad_metadata.push(conflict);
    for (method, path, target) in [
        ("POST", "/scene-admin", "admin"),
        ("DELETE", "/scene-admin", "admin"),
        ("POST", "/scene-bans", "banned_address"),
        ("DELETE", "/scene-bans", "banned_address"),
    ] {
        for meta in &bad_metadata {
            let (status, value) = call(
                &app,
                method,
                path,
                meta,
                json!({target:address_of(OUTSIDER_KEY)}),
                KEY,
            )
            .await;
            assert_eq!(
                status,
                StatusCode::BAD_REQUEST,
                "{method} {path} {meta}: {value}"
            );
        }
        for selector in ["place_id", "realmName", "sceneId", "parcel"] {
            let body = json!({target:address_of(OUTSIDER_KEY),selector:"another-scene"});
            let (status, value) = call(&app, method, path, &valid, body, KEY).await;
            assert_eq!(status, StatusCode::BAD_REQUEST, "body {selector}: {value}");
            let (status, value) = call(
                &app,
                method,
                &format!("{path}?{selector}=another-scene"),
                &valid,
                json!({target:address_of(OUTSIDER_KEY)}),
                KEY,
            )
            .await;
            assert_eq!(status, StatusCode::BAD_REQUEST, "query {selector}: {value}");
        }
    }
}

#[tokio::test]
async fn toolkit_baseline_optional_parcel_hostname_and_flat_realm_reach_authorization() {
    let mut state = comms_state(lazy_pool(), Settings::for_tests());
    state.scenes = Arc::new(ToolkitScenes);
    let app = super::super::router(Arc::new(state));
    for (realm, scene, parcel) in [
        ("main", "GenesisA", "42,42"),
        ("Single.eth", "SingleHash", "0,0"),
        ("MiXeD.dcl.eth", "SceneB", "1,1"),
    ] {
        let nested = metadata(realm, scene, parcel);
        let mut no_parcel = nested.clone();
        no_parcel.as_object_mut().unwrap().remove("parcel");
        let mut no_hostname = no_parcel.clone();
        no_hostname["realm"]
            .as_object_mut()
            .unwrap()
            .remove("hostname");
        // Dated baseline capture uses a bare hostname, not an absolute URL.
        let mut bare_hostname = nested.clone();
        bare_hostname["realm"]["hostname"] = json!(if realm == "main" {
            "realm-provider.example"
        } else {
            "worlds-content-server.example"
        });
        let flat = json!({"signer":"decentraland-kernel-scene","sceneId":scene,"realmName":realm});
        let mut variants = vec![nested, no_parcel, no_hostname, bare_hostname, flat];
        if realm != "main" {
            let mut legacy = metadata(realm, scene, parcel);
            legacy["sceneId"] = json!(realm);
            variants.push(legacy);
        }
        for meta in variants {
            let (status, value) = call(
                &app,
                "DELETE",
                "/scene-bans",
                &meta,
                json!({"banned_address":address_of(OUTSIDER_KEY)}),
                KEY,
            )
            .await;
            assert_eq!(status, StatusCode::FORBIDDEN, "{meta}: {value}");
        }
    }
}

#[tokio::test]
async fn toolkit_world_scene_colons_reach_write_authorization_and_matching_get_storage() {
    let pool = lazy_pool();
    pool.close().await;
    let mut state = comms_state(pool, Settings::for_tests());
    state.scenes = Arc::new(ToolkitScenes);
    let app = super::super::router(Arc::new(state));
    let exact = metadata("CoLoN.dcl.eth", "Cid:Case", "2,2");
    let mut no_parcel = exact.clone();
    no_parcel.as_object_mut().unwrap().remove("parcel");
    let flat = json!({"signer":"decentraland-kernel-scene","realmName":"CoLoN.dcl.eth","sceneId":"Cid:Case"});
    for meta in [exact, no_parcel, flat] {
        for (method, path, key) in [
            ("POST", "/scene-admin", "admin"),
            ("DELETE", "/scene-admin", "admin"),
            ("POST", "/scene-bans", "banned_address"),
            ("DELETE", "/scene-bans", "banned_address"),
        ] {
            let (status, value) = call(
                &app,
                method,
                path,
                &meta,
                json!({key:address_of(OUTSIDER_KEY)}),
                KEY,
            )
            .await;
            assert_eq!(
                status,
                StatusCode::FORBIDDEN,
                "valid world scene must reach actor authorization: {meta} {method} {path}: {value}"
            );
        }
        for path in ["/scene-admin", "/scene-bans", "/scene-bans/addresses"] {
            let (status, value) = call(&app, "GET", path, &meta, Value::Null, KEY).await;
            assert_eq!(
                status,
                StatusCode::INTERNAL_SERVER_ERROR,
                "valid signed world scene must reach the deliberately closed store: {meta} {path}: {value}"
            );
        }
        let (status, value) = call(
            &app,
            "GET",
            "/scene-bans?place_id=another-scene",
            &meta,
            Value::Null,
            KEY,
        )
        .await;
        assert_eq!(
            status,
            StatusCode::BAD_REQUEST,
            "unsigned selector remains bound: {value}"
        );
    }
    let shared = metadata("main", "Cid:Case", "42,42");
    for (method, path, body) in [
        (
            "POST",
            "/scene-admin",
            json!({"admin":address_of(OUTSIDER_KEY)}),
        ),
        ("GET", "/scene-admin", Value::Null),
    ] {
        let (status, value) = call(&app, method, path, &shared, body, KEY).await;
        assert_eq!(
            status,
            StatusCode::BAD_REQUEST,
            "shared room delimiter restriction remains: {value}"
        );
    }
}

struct ActorPolicy {
    pool: Option<PgPool>,
    unavailable: bool,
    protected: BTreeSet<Address>,
}

#[async_trait::async_trait]
impl AccessPolicy for ActorPolicy {
    async fn world_standing(
        &self,
        _: &WorldName,
    ) -> Result<crate::access::WorldStanding, AuthorityNotEstablished> {
        Ok(crate::access::WorldStanding::Open(AccessType::Unrestricted))
    }
    async fn privileged_addresses(
        &self,
        _: &WorldName,
    ) -> Result<BTreeSet<Address>, AuthorityNotEstablished> {
        Ok(BTreeSet::new())
    }
    async fn world_owner(&self, _: &WorldName) -> Result<Option<Address>, AuthorityNotEstablished> {
        Ok(Some(Address::parse(&address_of(KEY)).unwrap()))
    }
    async fn land_document(
        &self,
        _: Parcel,
    ) -> Result<Option<Arc<Option<LandOperators>>>, AuthorityNotEstablished> {
        Ok(Some(Arc::new(Some(LandOperators {
            owner: address_of(KEY),
            ..Default::default()
        }))))
    }
    async fn can_access(
        &self,
        _: &WorldName,
        _: &Address,
        _: Option<&str>,
        _: Option<&crate::access::SceneBanKey>,
        _: Option<&str>,
        _: chrono::NaiveDateTime,
    ) -> Result<Access, AuthorityNotEstablished> {
        Ok(Access::Allowed(crate::access::AccessGrant::Unrestricted))
    }
    async fn is_blocked(&self, _: &Address) -> Result<bool, AuthorityNotEstablished> {
        Ok(false)
    }
    async fn is_admin(&self, scene: &SceneRef<'_>, actor: &Address) -> PermissionDecision {
        if self.unavailable {
            return PermissionDecision::undetermined("policy fixture", &sqlx::Error::PoolClosed);
        }
        if actor.as_str() == address_of(KEY) {
            return PermissionDecision::Allowed;
        }
        match &self.pool {
            Some(pool) => {
                super::super::moderation::Moderation::new(pool.clone())
                    .admin_decision(scene.scene_id().as_str(), actor)
                    .await
            }
            None => PermissionDecision::refused("not authorized"),
        }
    }
    async fn protected_addresses(
        &self,
        _: &WorldName,
        _: &ParcelSet,
    ) -> Result<BTreeSet<Address>, AuthorityNotEstablished> {
        let mut addresses = self.protected.clone();
        addresses.insert(Address::parse(&address_of(KEY)).unwrap());
        Ok(addresses)
    }
}

struct Names(Result<Option<Address>, &'static str>);
#[async_trait::async_trait]
impl NameOwnerReader for Names {
    async fn name_owner(&self, _: &str) -> Result<Option<Address>, String> {
        self.0.clone().map_err(str::to_string)
    }
}

#[derive(Default)]
struct Media {
    metadata: BTreeMap<String, Value>,
    connected: BTreeMap<String, BTreeSet<String>>,
    requests: Vec<(String, Value)>,
}

async fn media_fixture() -> (
    String,
    Arc<std::sync::Mutex<Media>>,
    tokio::task::JoinHandle<()>,
) {
    use axum::response::IntoResponse;
    let media = Arc::new(std::sync::Mutex::new(Media::default()));
    let capture = media.clone();
    let fixture = axum::Router::new().route("/twirp/livekit.RoomService/{method}", axum::routing::post(move |Path(method):Path<String>, Json(body):Json<Value>| {
        let capture = capture.clone();
        async move {
            let mut data = capture.lock().unwrap();
            data.requests.push((method.clone(),body.clone()));
            match method.as_str() {
                "ListRooms" => {
                    // The full listing (replaced-scene rooms) holds no extra room here.
                    let Some(room) = body["names"][0].as_str() else {
                        return Json(json!({"rooms":[]})).into_response();
                    };
                    Json(json!({"rooms":[{"name":room,"sid":"RM_toolkit","metadata":data.metadata.get(room).cloned().unwrap_or(json!({"presenters":["canary"]})).to_string()}]})).into_response()
                }
                "UpdateRoomMetadata" => {
                    data.metadata.insert(body["room"].as_str().unwrap().to_owned(),serde_json::from_str(body["metadata"].as_str().unwrap()).unwrap());
                    Json(json!({})).into_response()
                }
                "RemoveParticipant" => {
                    data.connected.entry(body["room"].as_str().unwrap().to_owned()).or_default().remove(body["identity"].as_str().unwrap());
                    Json(json!({})).into_response()
                }
                _ => StatusCode::NOT_FOUND.into_response(),
            }
        }
    }));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let task = tokio::spawn(async move { axum::serve(listener, fixture).await.unwrap() });
    (url, media, task)
}

#[tokio::test]
async fn toolkit_rejections_have_no_media_effect_and_reader_faults_stay_unavailable() {
    let (url, media, server) = media_fixture().await;
    let mut state = comms_state(lazy_pool(), Settings::for_tests());
    state.scenes = Arc::new(ToolkitScenes);
    state.livekit.configured = true;
    state.livekit.api_url = url;
    state.policy = Arc::new(ActorPolicy {
        pool: None,
        unavailable: false,
        protected: BTreeSet::new(),
    });
    let app = super::super::router(Arc::new(state));
    let meta = metadata("MiXeD.dcl.eth", "SceneB", "1,1");
    for (method, path, body) in [
        ("POST", "/scene-admin", json!({"admin":address_of(KEY)})),
        ("DELETE", "/scene-admin", json!({"admin":address_of(KEY)})),
        (
            "POST",
            "/scene-bans",
            json!({"banned_address":address_of(KEY)}),
        ),
    ] {
        let (status, value) = call(&app, method, path, &meta, body, KEY).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{method} {path}: {value}");
    }
    for (method, path, body) in [
        ("POST", "/scene-admin", json!({"name":"Alice"})),
        (
            "POST",
            "/scene-bans",
            json!({"banned_name":"Alice.dcl.eth"}),
        ),
        ("DELETE", "/scene-bans", json!({"banned_name":"Alice"})),
    ] {
        let (status, value) = call(&app, method, path, &meta, body, KEY).await;
        assert_eq!(
            status,
            StatusCode::SERVICE_UNAVAILABLE,
            "{method} {path}: {value}"
        );
    }
    assert!(media.lock().unwrap().requests.is_empty());
    server.abort();

    for (result, expected) in [
        (Ok(None), StatusCode::NOT_FOUND),
        (Err("provider unavailable"), StatusCode::SERVICE_UNAVAILABLE),
    ] {
        let mut state = comms_state(lazy_pool(), Settings::for_tests());
        state.scenes = Arc::new(ToolkitScenes);
        state.policy = Arc::new(ActorPolicy {
            pool: None,
            unavailable: false,
            protected: BTreeSet::new(),
        });
        state.name_owners = Some(Arc::new(Names(result)));
        let app = super::super::router(Arc::new(state));
        let (status, value) = call(
            &app,
            "DELETE",
            "/scene-bans",
            &meta,
            json!({"banned_name":"Alice"}),
            KEY,
        )
        .await;
        assert_eq!(status, expected, "{value}");
    }
}

#[tokio::test]
async fn toolkit_signature_and_policy_fault_matrix_has_no_media_effect() {
    use super::super::auth::test_support::{Shape, signed_at};
    let (url, media, server) = media_fixture().await;
    let mut state = comms_state(lazy_pool(), Settings::for_tests());
    state.scenes = Arc::new(ToolkitScenes);
    state.livekit.configured = true;
    state.livekit.api_url = url;
    let app = super::super::router(Arc::new(state));
    let meta = metadata("MiXeD.dcl.eth", "SceneB", "1,1");
    for (method, path, key) in [
        ("POST", "/scene-admin", "admin"),
        ("DELETE", "/scene-admin", "admin"),
        ("POST", "/scene-bans", "banned_address"),
        ("DELETE", "/scene-bans", "banned_address"),
    ] {
        let body = json!({key:address_of(OUTSIDER_KEY)}).to_string();
        let stale = (chrono::Utc::now().timestamp_millis() - 120_000).to_string();
        let wrong_meta =
            json!({"signer":"dcl:explorer","realmName":"mixed.dcl.eth","sceneId":"SceneB"});
        for headers in [
            HeaderMap::new(),
            signed(KEY, method, "/wrong-path", &meta.to_string()).await,
            signed(KEY, method, path, &wrong_meta.to_string()).await,
            signed_at(
                KEY,
                Shape::V6,
                method,
                path,
                &meta.to_string(),
                &meta.to_string(),
                &stale,
            )
            .await,
        ] {
            let mut request = Request::builder()
                .method(method)
                .uri(path)
                .body(Body::from(body.clone()))
                .unwrap();
            request.headers_mut().extend(headers);
            let status = app.clone().oneshot(request).await.unwrap().status();
            assert!(
                matches!(status, StatusCode::BAD_REQUEST | StatusCode::UNAUTHORIZED),
                "{method} {path}: {status}"
            );
        }
        let (status, value) = call(
            &app,
            method,
            path,
            &meta,
            json!({key:address_of(OUTSIDER_KEY)}),
            OUTSIDER_KEY,
        )
        .await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{value}");
    }
    assert!(media.lock().unwrap().requests.is_empty());
    server.abort();
    let mut state = comms_state(lazy_pool(), Settings::for_tests());
    state.scenes = Arc::new(ToolkitScenes);
    state.policy = Arc::new(ActorPolicy {
        pool: None,
        unavailable: true,
        protected: BTreeSet::new(),
    });
    let app = super::super::router(Arc::new(state));
    let (status, value) = call(
        &app,
        "POST",
        "/scene-admin",
        &meta,
        json!({"admin":address_of(OUTSIDER_KEY)}),
        KEY,
    )
    .await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{value}");
}

#[tokio::test]
async fn toolkit_current_placement_legacy_query_delete_and_mounted_signature_are_preserved() {
    let mut state = comms_state(lazy_pool(), Settings::for_tests());
    state.scenes = Arc::new(ToolkitScenes);
    let app = super::super::router(Arc::new(state));
    let meta = metadata("MiXeD.dcl.eth", "SceneB", "1,1");
    for (path, key) in [("/scene-admin", "admin"), ("/scene-bans", "banned_address")] {
        let path = format!("{path}?place_id=SceneB&{key}={}", address_of(OUTSIDER_KEY));
        let mut request = Request::delete(&path).body(Body::empty()).unwrap();
        request.headers_mut().extend(
            signed(
                KEY,
                "delete",
                path.split('?').next().unwrap(),
                &meta.to_string(),
            )
            .await,
        );
        let (status, value) = body_json(app.clone().oneshot(request).await.unwrap()).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{value}");
    }
    let historical = metadata("main", "HistoricalGenesis", "42,42");
    let (status, value) = call(
        &app,
        "DELETE",
        "/scene-admin",
        &historical,
        json!({"admin":address_of(OUTSIDER_KEY)}),
        KEY,
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "historical: {value}");
    let mounted = axum::Router::new().nest("/comms-gatekeeper", app);
    let (status, value) = call(
        &mounted,
        "DELETE",
        "/comms-gatekeeper/scene-admin",
        &meta,
        json!({"admin":address_of(OUTSIDER_KEY)}),
        KEY,
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "mounted: {value}");
}

#[tokio::test]
async fn toolkit_store_fault_cannot_reach_media() {
    let (url, media, server) = media_fixture().await;
    let pool = lazy_pool();
    pool.close().await;
    let mut state = comms_state(pool, Settings::for_tests());
    state.scenes = Arc::new(ToolkitScenes);
    state.policy = Arc::new(ActorPolicy {
        pool: None,
        unavailable: false,
        protected: BTreeSet::new(),
    });
    state.livekit.configured = true;
    state.livekit.api_url = url;
    let app = super::super::router(Arc::new(state));
    let meta = metadata("MiXeD.dcl.eth", "SceneB", "1,1");
    for (method, path, key) in [
        ("POST", "/scene-admin", "admin"),
        ("DELETE", "/scene-admin", "admin"),
        ("POST", "/scene-bans", "banned_address"),
        ("DELETE", "/scene-bans", "banned_address"),
    ] {
        let (status, value) = call(
            &app,
            method,
            path,
            &meta,
            json!({key:address_of(OUTSIDER_KEY)}),
            KEY,
        )
        .await;
        assert!(status.is_server_error(), "{method} {path}: {value}");
    }
    assert!(media.lock().unwrap().requests.is_empty());
    server.abort();
}

#[tokio::test]
async fn toolkit_ban_listings_bind_unsigned_selectors_to_the_signed_scene() {
    let pool = lazy_pool();
    pool.close().await;
    let mut state = comms_state(pool, Settings::for_tests());
    state.scenes = Arc::new(ToolkitScenes);
    let app = super::super::router(Arc::new(state));
    let meta = metadata("MiXeD.dcl.eth", "MiXeD.dcl.eth", "1,1");
    for path in [
        "/scene-bans?place_id=SceneA",
        "/scene-bans/addresses?place_id=SceneA",
    ] {
        let (status, value) = call(&app, "GET", path, &meta, Value::Null, KEY).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{path}: {value}");
    }
}

#[tokio::test]
async fn toolkit_scene_provider_fault_and_extended_targets_fail_before_media() {
    let (url, media, server) = media_fixture().await;
    let protected = Address::parse(&address_of(OUTSIDER_KEY)).unwrap();
    let mut state = comms_state(lazy_pool(), Settings::for_tests());
    state.scenes = Arc::new(ToolkitScenes);
    state.policy = Arc::new(ActorPolicy {
        pool: None,
        unavailable: false,
        protected: [protected.clone()].into_iter().collect(),
    });
    state.livekit.configured = true;
    state.livekit.api_url = url;
    let app = super::super::router(Arc::new(state));
    let meta = metadata("MiXeD.dcl.eth", "SceneB", "1,1");
    for (method, path, key) in [
        ("POST", "/scene-admin", "admin"),
        ("DELETE", "/scene-admin", "admin"),
        ("POST", "/scene-bans", "banned_address"),
    ] {
        let (status, value) = call(
            &app,
            method,
            path,
            &meta,
            json!({key:protected.as_str()}),
            KEY,
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{value}");
    }
    assert!(media.lock().unwrap().requests.is_empty());
    server.abort();
    let mut state = comms_state(lazy_pool(), Settings::for_tests());
    state.scenes = Arc::new(StubScenes {
        room: Err(ResolveError::Db(sqlx::Error::PoolClosed)),
        ..Default::default()
    });
    let app = super::super::router(Arc::new(state));
    let (status, value) = call(
        &app,
        "POST",
        "/scene-admin",
        &meta,
        json!({"admin":protected.as_str()}),
        KEY,
    )
    .await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{value}");
}

struct ContractProof([u8; 32]);
#[async_trait::async_trait]
impl crate::auth::eip1654::Eip1654Validator for ContractProof {
    async fn validate_signature(
        &self,
        contract: &str,
        hash: &[u8],
        _: &[u8],
    ) -> Result<bool, crate::auth::AuthError> {
        Ok(contract.eq_ignore_ascii_case(&address_of(KEY)) && hash == self.0)
    }
}

#[tokio::test]
async fn toolkit_body_writes_keep_contract_wallet_signature_validation() {
    let meta = metadata("MiXeD.dcl.eth", "SceneB", "1,1");
    for (method, path, key) in [
        ("POST", "/scene-admin", "admin"),
        ("DELETE", "/scene-admin", "admin"),
        ("POST", "/scene-bans", "banned_address"),
        ("DELETE", "/scene-bans", "banned_address"),
    ] {
        let mut headers = signed(KEY, method, path, &meta.to_string()).await;
        let mut link: Value =
            serde_json::from_str(headers["x-identity-auth-chain-1"].to_str().unwrap()).unwrap();
        let hash = alloy_primitives::keccak256(link["payload"].as_str().unwrap()).0;
        link["type"] = json!("ECDSA_EIP_1654_SIGNED_ENTITY");
        headers.insert(
            "x-identity-auth-chain-1",
            HeaderValue::from_str(&link.to_string()).unwrap(),
        );
        for (validator, expected) in [
            (None, StatusCode::SERVICE_UNAVAILABLE),
            (Some(Arc::new(ContractProof(hash))), StatusCode::FORBIDDEN),
            (
                Some(Arc::new(ContractProof([0; 32]))),
                StatusCode::UNAUTHORIZED,
            ),
        ] {
            let mut state = comms_state(lazy_pool(), Settings::for_tests());
            state.scenes = Arc::new(ToolkitScenes);
            state.eip1654 = validator.map(|v| v as Arc<dyn crate::auth::eip1654::Eip1654Validator>);
            let app = super::super::router(Arc::new(state));
            let mut request = Request::builder()
                .method(method)
                .uri(path)
                .body(Body::from(
                    json!({key:address_of(OUTSIDER_KEY)}).to_string(),
                ))
                .unwrap();
            request.headers_mut().extend(headers.clone());
            let (status, value) = body_json(app.oneshot(request).await.unwrap()).await;
            assert_eq!(status, expected, "{method} {path}: {value}");
        }
    }
}

#[tokio::test]
async fn toolkit_all_writes_persist_get_state_and_target_canonical_metadata_and_connected_ban_fixture()
 {
    let Some(pool) = scratch().await else {
        eprintln!(
            "GUARDED toolkit persisted-state and Twirp closure: CATALYST_TEST_DATABASE_URL unset"
        );
        return;
    };
    let (url, media, server) = media_fixture().await;
    let target = Address::parse(&address_of(&random_key())).unwrap();
    let mut state = comms_state(pool.clone(), Settings::for_tests());
    state.scenes = Arc::new(ToolkitScenes);
    state.policy = Arc::new(ActorPolicy {
        pool: Some(pool.clone()),
        unavailable: false,
        protected: BTreeSet::new(),
    });
    state.name_owners = Some(Arc::new(Names(Ok(Some(target.clone())))));
    state.livekit.configured = true;
    state.livekit.api_url = url;
    let app = super::super::router(Arc::new(state));
    // Literals independent of the room helper pin canonical mixed-case scope.
    for (realm, scene, parcel, rooms) in [
        ("main", "GenesisA", "42,42", vec!["scene:main:GenesisA"]),
        (
            "Single.eth",
            "SingleHash",
            "0,0",
            vec!["world-single.eth", "w1:a:single.ethSingleHash"],
        ),
        (
            "MiXeD.dcl.eth",
            "SceneA",
            "0,0",
            vec!["world-mixed.dcl.eth", "w1:d:mixed.dcl.ethSceneA"],
        ),
        (
            "MIXED.DCL.ETH",
            "SceneB",
            "1,1",
            vec!["world-mixed.dcl.eth", "w1:d:mixed.dcl.ethSceneB"],
        ),
    ] {
        let meta = metadata(realm, scene, parcel);
        let checksum = target
            .as_str()
            .parse::<alloy_primitives::Address>()
            .unwrap()
            .to_checksum(None);
        let before_media = media.lock().unwrap().requests.len();
        for (method, path, key) in [
            ("POST", "/scene-admin", "admin"),
            ("DELETE", "/scene-admin", "admin"),
            ("POST", "/scene-bans", "banned_address"),
            ("DELETE", "/scene-bans", "banned_address"),
        ] {
            for (body, signer, status) in [
                (
                    json!({key:target.as_str(),"place_id":"OtherScene"}),
                    KEY,
                    StatusCode::BAD_REQUEST,
                ),
                (
                    json!({key:target.as_str()}),
                    OUTSIDER_KEY,
                    StatusCode::FORBIDDEN,
                ),
            ] {
                let (actual, value) = call(&app, method, path, &meta, body, signer).await;
                assert_eq!(actual, status, "{realm} {method} {path}: {value}");
            }
        }
        assert_eq!(media.lock().unwrap().requests.len(), before_media);
        let rows:i64=sqlx::query_scalar("SELECT (SELECT count(*) FROM social.scene_admins WHERE place_id=$1 AND admin_address=$2 AND active)+(SELECT count(*) FROM social.scene_bans WHERE place_id=$1 AND banned_address=$2)")
            .bind(scene).bind(&target).fetch_one(&pool).await.unwrap();
        assert_eq!(
            rows, 0,
            "rejections must leave no persisted target mutation"
        );
        for named in [false, true] {
            let body = if named {
                json!({"name":"Alice.dcl.eth"})
            } else {
                json!({"admin":checksum})
            };
            let (status, value) = call(&app, "POST", "/scene-admin", &meta, body, KEY).await;
            assert_eq!(status, StatusCode::NO_CONTENT, "{realm} {scene}: {value}");
            let (status, value) = call(&app, "GET", "/scene-admin", &meta, Value::Null, KEY).await;
            assert_eq!(status, StatusCode::OK, "{value}");
            assert!(
                value
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|entry| entry["admin"] == target.as_str())
            );
            for room in &rooms {
                let data = media.lock().unwrap();
                assert_eq!(
                    data.metadata[*room]["sceneAdmins"],
                    json!([target.as_str()])
                );
                assert_eq!(data.metadata[*room]["presenters"], json!(["canary"]));
            }
            let (status, value) = call(
                &app,
                "DELETE",
                "/scene-admin",
                &meta,
                json!({"admin":checksum}),
                KEY,
            )
            .await;
            assert_eq!(status, StatusCode::NO_CONTENT, "{value}");
            let (_, value) = call(&app, "GET", "/scene-admin", &meta, Value::Null, KEY).await;
            assert!(
                !value
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|entry| entry["admin"] == target.as_str())
            );
            for room in &rooms {
                media
                    .lock()
                    .unwrap()
                    .connected
                    .entry((*room).into())
                    .or_default()
                    .insert(target.as_str().into());
            }
            let body = if named {
                json!({"banned_name":"Alice"})
            } else {
                json!({"banned_address":checksum})
            };
            let (status, value) = call(&app, "POST", "/scene-bans", &meta, body, KEY).await;
            assert_eq!(status, StatusCode::NO_CONTENT, "{value}");
            let (status, value) = call(&app, "GET", "/scene-bans", &meta, Value::Null, KEY).await;
            assert_eq!(status, StatusCode::OK, "{value}");
            assert!(
                value["results"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|entry| entry["bannedAddress"] == target.as_str())
            );
            if realm != "main" {
                let mut alias = meta.clone();
                alias["sceneId"] = json!(realm);
                let (status, value) =
                    call(&app, "GET", "/scene-bans", &alias, Value::Null, KEY).await;
                assert_eq!(status, StatusCode::OK, "{value}");
                assert!(
                    value["results"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .any(|entry| entry["bannedAddress"] == target.as_str())
                );
                let (status, value) = call(
                    &app,
                    "GET",
                    "/scene-bans/addresses",
                    &alias,
                    Value::Null,
                    KEY,
                )
                .await;
                assert_eq!(status, StatusCode::OK, "{value}");
                assert!(
                    value["results"]
                        .as_array()
                        .unwrap()
                        .contains(&json!(target.as_str()))
                );
            }
            for room in &rooms {
                let data = media.lock().unwrap();
                assert!(!data.connected[*room].contains(target.as_str()));
                assert_eq!(
                    data.metadata[*room]["bannedAddresses"],
                    json!([target.as_str()])
                );
            }
            let body = if named {
                json!({"banned_name":"Alice.dcl.eth"})
            } else {
                json!({"banned_address":checksum})
            };
            let (status, value) = call(&app, "DELETE", "/scene-bans", &meta, body, KEY).await;
            assert_eq!(status, StatusCode::NO_CONTENT, "{value}");
            let (_, value) = call(&app, "GET", "/scene-bans", &meta, Value::Null, KEY).await;
            assert!(
                !value["results"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|entry| entry["bannedAddress"] == target.as_str())
            );
            for room in &rooms {
                let data = media.lock().unwrap();
                assert_eq!(data.metadata[*room]["sceneAdmins"], json!([]));
                assert_eq!(data.metadata[*room]["bannedAddresses"], json!([]));
            }
        }
    }
    server.abort();
}

#[tokio::test]
async fn notification_failure_rolls_back_scene_ban_before_kick_or_room_metadata() {
    let Some(pool) = scratch().await else {
        eprintln!(
            "GUARDED moderation notice failure/media assertions: CATALYST_TEST_DATABASE_URL unset"
        );
        return;
    };
    let (url, media, server) = media_fixture().await;
    let directory = Arc::new(crate::interfaces::test_support::StubDirectory::default());
    directory
        .fail_moderation_notifications
        .store(true, std::sync::atomic::Ordering::SeqCst);
    let target = Address::parse(&address_of(&random_key())).unwrap();
    let mut state = comms_state(pool.clone(), Settings::for_tests());
    state.scenes = Arc::new(ToolkitScenes);
    state.policy = Arc::new(ActorPolicy {
        pool: Some(pool.clone()),
        unavailable: false,
        protected: BTreeSet::new(),
    });
    state.moderation =
        super::super::moderation::Moderation::with_social(pool.clone(), directory.clone());
    state.social = directory;
    state.livekit.configured = true;
    state.livekit.api_url = url;
    let app = super::super::router(Arc::new(state));
    let (status, response) = call(
        &app,
        "POST",
        "/scene-bans",
        &metadata("MiXeD.dcl.eth", "SceneB", "1,1"),
        json!({"banned_address":target.as_str()}),
        KEY,
    )
    .await;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR, "{response}");
    assert!(
        media.lock().unwrap().requests.is_empty(),
        "failed persistence must not kick or alter room metadata"
    );
    let counts: (i64, i64, i64) = sqlx::query_as("SELECT (SELECT count(*) FROM social.scene_bans WHERE banned_address=$1),(SELECT count(*) FROM social.livekit_removal_intents WHERE wallet=$1),(SELECT count(*) FROM social.notifications WHERE address=$1)").bind(&target).fetch_one(&pool).await.unwrap();
    assert_eq!(counts, (0, 0, 0));
    server.abort();
}
