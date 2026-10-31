//! Room identity conformance at signed HTTP and LiveKit Twirp boundaries.
//! The catalogue is a deterministic SceneIdentity fixture; no database guard.
use super::*;
use crate::interfaces::SceneIdentity;
use crate::rooms::{ActiveDeployment, ResolveError, ResolvedRoom, RoomRequest};
use crate::world_identity::{Parcel, RealmId, SceneId, WorldName};

struct Catalogue;

fn deployment(world: &WorldName, id: &str) -> ActiveDeployment {
    let base = Parcel::parse(if id == "SceneA" { "0,0" } else { "1,1" }).unwrap();
    ActiveDeployment {
        title: None,
        scene_id: SceneId::parse(id).unwrap(),
        parcels: [base].into_iter().collect(),
        base,
        namespace: crate::world_identity::SceneNamespace::new(
            &RealmId::World(world.clone()),
            base,
            None,
        ),
        local: true,
    }
}

#[async_trait::async_trait]
impl SceneIdentity for Catalogue {
    async fn classify_room(
        &self,
        room: &str,
    ) -> Result<crate::rooms::RoomClassification, crate::rooms::ResolveError> {
        use crate::rooms::{RoomRefusal as R, refused_room};
        let reason = match room {
            "world-foo.dcl.eth-scene" => Some(R::Ambiguous),
            "world-mixed.dcl.eth-scenea" => Some(R::Legacy),
            "world-deleted.dcl.eth-scene" => Some(R::Unresolved),
            _ => None,
        };
        if let Some(reason) = reason {
            return Ok(refused_room(room, reason));
        }
        crate::interfaces::test_support::classify_fixture(self, room).await
    }

    async fn resolve(&self, request: RoomRequest<'_>) -> Result<ResolvedRoom, ResolveError> {
        let (world, dep) = match request {
            RoomRequest::NamedWorldScene { room } => {
                if room == "w1:j:mixed.dcl.eth-otherSceneA" {
                    let world = WorldName::parse("mixed.dcl.eth-other").unwrap();
                    let dep = deployment(&world, "SceneA");
                    return Ok(ResolvedRoom {
                        realm: RealmId::World(world),
                        scene_id: Some(dep.scene_id.clone()),
                        deployment: Some(dep),
                        room: room.to_owned(),
                    });
                }
                let id = match room {
                    "w1:d:mixed.dcl.ethSceneA" => "SceneA",
                    "w1:d:mixed.dcl.ethSceneB" => "SceneB",
                    _ => return Err(ResolveError::UnresolvedRoom(room.to_owned())),
                };
                let world = WorldName::parse("mixed.dcl.eth").unwrap();
                let dep = deployment(&world, id);
                return Ok(ResolvedRoom {
                    realm: RealmId::World(world),
                    scene_id: Some(dep.scene_id.clone()),
                    deployment: Some(dep),
                    room: room.to_owned(),
                });
            }
            RoomRequest::WorldLobby { world } => (world, None),
            RoomRequest::WorldScene { world, scene } => {
                let dep = self.world_scene(world, scene).await?.ok_or_else(|| {
                    ResolveError::SceneNotInWorld {
                        world: world.to_string(),
                        scene: scene.to_string(),
                    }
                })?;
                (world, Some(dep))
            }
            RoomRequest::Adapter {
                realm: RealmId::World(world),
                scene,
            } => {
                let dep = if scene.is_world_shaped() {
                    self.world_active_scene(world).await?
                } else {
                    self.world_scene(world, scene).await?
                }
                .ok_or_else(|| ResolveError::WorldHasNoScene(world.to_string()))?;
                (world, Some(dep))
            }
            RoomRequest::Parcel {
                realm: RealmId::World(world),
                parcel,
            } => {
                let id = match parcel.canonical().as_str() {
                    "0,0" => "SceneA",
                    "1,1" => "SceneB",
                    _ => {
                        return Err(ResolveError::NoSceneAtParcel {
                            realm: world.to_string(),
                            parcel: parcel.to_string(),
                        });
                    }
                };
                (
                    world,
                    self.world_scene(world, &SceneId::parse(id).unwrap())
                        .await?,
                )
            }
            _ => panic!("this fixture resolves world rooms only"),
        };
        let room = match &dep {
            Some(dep) => crate::rooms::worlds_service_scene_room(world, &dep.scene_id),
            None => crate::rooms::worlds_service_world_room(world),
        };
        Ok(ResolvedRoom {
            realm: RealmId::World(world.clone()),
            scene_id: dep.as_ref().map(|d| d.scene_id.clone()),
            deployment: dep,
            room,
        })
    }

    async fn world_scene(
        &self,
        world: &WorldName,
        scene: &SceneId,
    ) -> Result<Option<ActiveDeployment>, ResolveError> {
        Ok(
            ((world.key() == "mixed.dcl.eth" && matches!(scene.as_str(), "SceneA" | "SceneB"))
                || (world.key() == "single.eth" && scene.as_str() == "SingleHash"))
                .then(|| deployment(world, scene.as_str())),
        )
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

fn canonical_state() -> CommsState {
    let mut state = comms_state(lazy_pool(), with_authoritative(AUTHORITATIVE_KEY))
        .with_ban_status(Arc::new(
            super::super::moderation::test_support::StubBanStatus(Ok(None)),
        ));
    state.scenes = Arc::new(Catalogue);
    state.presence.complete_empty_fixture();
    state
}

fn jwt_claims(adapter: &str) -> serde_json::Value {
    super::super::livekit::decode_jwt_payload(adapter.split("access_token=").nth(1).unwrap())
}

struct BoundaryCatalogue {
    world: WorldName,
    scene: SceneId,
}

#[async_trait::async_trait]
impl SceneIdentity for BoundaryCatalogue {
    async fn classify_room(
        &self,
        room: &str,
    ) -> Result<crate::rooms::RoomClassification, ResolveError> {
        crate::interfaces::test_support::classify_fixture(self, room).await
    }

    async fn resolve(&self, request: RoomRequest<'_>) -> Result<ResolvedRoom, ResolveError> {
        let (world, scene) = match request {
            RoomRequest::WorldScene { world, scene } => (world.clone(), scene.clone()),
            RoomRequest::NamedWorldScene { room } => crate::rooms::decode_world_scene_room(room)
                .ok_or_else(|| ResolveError::UnresolvedRoom(room.into()))?,
            _ => panic!("boundary fixture expects an exact world scene"),
        };
        let dep = self.world_scene(&world, &scene).await?.ok_or_else(|| {
            ResolveError::SceneNotInWorld {
                world: world.to_string(),
                scene: scene.to_string(),
            }
        })?;
        Ok(ResolvedRoom {
            realm: RealmId::World(world.clone()),
            scene_id: Some(scene.clone()),
            deployment: Some(dep),
            room: crate::rooms::worlds_service_scene_room(&world, &scene),
        })
    }
    async fn world_scene(
        &self,
        world: &WorldName,
        scene: &SceneId,
    ) -> Result<Option<ActiveDeployment>, ResolveError> {
        Ok((world == &self.world && scene == &self.scene)
            .then(|| deployment(world, scene.as_str())))
    }
    async fn world_active_scene(
        &self,
        world: &WorldName,
    ) -> Result<Option<ActiveDeployment>, ResolveError> {
        self.world_scene(world, &self.scene).await
    }
}

async fn mint_boundary_token(world: &str, scene: &str) -> (Arc<CommsState>, String) {
    let mut state = canonical_state();
    state.livekit.configured = true;
    state.livekit.api_secret = "s".repeat(32);
    state.scenes = Arc::new(BoundaryCatalogue {
        world: WorldName::parse(world).unwrap(),
        scene: SceneId::parse(scene).unwrap(),
    });
    let state = Arc::new(state);
    let world_path: String = url::form_urlencoded::byte_serialize(world.as_bytes()).collect();
    let scene_path: String = url::form_urlencoded::byte_serialize(scene.as_bytes()).collect();
    let path = format!("/worlds/{world_path}/scenes/{scene_path}/comms");
    let mut request = Request::post(&path).body(Body::empty()).unwrap();
    request
        .headers_mut()
        .extend(signed(KEY, "post", &path, "{}").await);
    let (status, value) = body_json(
        super::super::admission::world_mint::router(state.clone())
            .oneshot(request)
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::OK,
        "boundary keys remain valid mint inputs: {value}"
    );
    (
        state,
        value["fixedAdapter"]
            .as_str()
            .unwrap()
            .split("access_token=")
            .nth(1)
            .unwrap()
            .to_owned(),
    )
}

fn verify_boundary_proof(
    state: &CommsState,
    jwt: &str,
) -> Result<crate::gatekeeper::admission::relay_authority::proof::VerifiedRoom, &'static str> {
    use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
    use hmac::{Hmac, KeyInit, Mac};
    use sha2::{Digest, Sha256};
    let (hp, signature) = jwt.rsplit_once('.').unwrap();
    let signature = URL_SAFE_NO_PAD.decode(signature).unwrap();
    let wallet = address_of(KEY);
    let wallet_bytes = crate::protocol::room_authority::address_bytes(&wallet).unwrap();
    let nonce = [7; 32];
    let mut proof = Hmac::<Sha256>::new_from_slice(&signature).unwrap();
    proof.update(crate::protocol::room_authority::PROOF_DOMAIN);
    proof.update(&nonce);
    proof.update(&wallet_bytes);
    proof.update(&wallet_bytes);
    proof.update(&Sha256::digest(hp.as_bytes()));
    crate::gatekeeper::admission::relay_authority::proof::ProofVerifier::new(
        state.livekit.api_key.clone(),
        state.livekit.api_secret.as_bytes().to_vec(),
    )
    .unwrap()
    .verify(
        hp,
        &proof.finalize().into_bytes(),
        &nonce,
        &wallet,
        &wallet,
        chrono::Utc::now().timestamp() as u64,
    )
}

#[tokio::test]
async fn canonical_mint_preserves_previously_valid_relay_room_byte_budgets() {
    for (world, scene) in [
        (format!("{}.dcl.eth", "a".repeat(177)), "b".repeat(59)),
        (format!("{}.dcl.eth", "a".repeat(182)), "b".repeat(59)),
        ("w".to_owned(), "K".repeat(248)),
    ] {
        let legacy = format!("scene-{world}-{}", scene.to_lowercase());
        assert!(legacy.len() <= 256);
        let (state, jwt) = mint_boundary_token(&world, &scene).await;
        let mut legacy_claims = super::super::livekit::decode_jwt_payload(&jwt);
        legacy_claims["video"]["room"] = serde_json::json!(legacy);
        let legacy_jwt = crate::shared::livekit::sign_hs256(
            &state.livekit.api_secret,
            br#"{"alg":"HS256","typ":"JWT"}"#,
            &serde_json::to_vec(&legacy_claims).unwrap(),
        )
        .unwrap();
        assert!(
            verify_boundary_proof(&state, &legacy_jwt).is_ok(),
            "same minted claims with the old room fit the old relay contract"
        );
        assert!(
            verify_boundary_proof(&state, &jwt).is_ok(),
            "valid minted tuple world{} / scene{} must remain relay-admissible",
            world.len(),
            scene.len()
        );
    }
}

#[tokio::test]
async fn canonical_mint_preserves_previously_valid_webhook_room_byte_budgets() {
    for (world, scene) in [
        (format!("{}.dcl.eth", "a".repeat(177)), "b".repeat(320)),
        ("w".to_owned(), "K".repeat(504)),
    ] {
        assert_eq!(format!("scene-{world}-{}", scene.to_lowercase()).len(), 512);
        let (state, jwt) = mint_boundary_token(&world, &scene).await;
        let room = super::super::livekit::decode_jwt_payload(&jwt)["video"]["room"]
            .as_str()
            .unwrap()
            .to_owned();
        let raw = serde_json::to_vec(&serde_json::json!({"id":"EV_boundary","createdAt":1718900000,"event":"participant_joined","room":{"name":room},"participant":{"identity":address_of(KEY),"sid":"PA_boundary"}})).unwrap();
        let bearer = super::super::webhook::test_support::webhook_jwt(
            &raw,
            &state.livekit.api_key,
            &state.livekit.api_secret,
        );
        let app = super::super::router(state);
        assert_eq!(
            post_webhook(&app, Some(&bearer), &raw).await,
            StatusCode::SERVICE_UNAVAILABLE,
            "a valid512-byte scene room reaches unavailable metadata authority; it must not fail name/signature validation"
        );
    }
}

#[tokio::test]
async fn canonical_escape_rooms_reach_twirp_metadata_kicks_and_ingress_verbatim() {
    let (state, jwt) = mint_boundary_token("w", "\u{212a}\u{1e9e}").await;
    let room = super::super::livekit::decode_jwt_payload(&jwt)["video"]["room"]
        .as_str()
        .unwrap()
        .to_owned();
    assert_eq!(room, "w1:1:w/ A");
    let observed = Arc::new(std::sync::Mutex::new(
        Vec::<(String, serde_json::Value)>::new(),
    ));
    let capture = observed.clone();
    let fixture = axum::Router::new().route("/twirp/{service}/{method}",axum::routing::post(move |Path((service,method)):Path<(String,String)>,Json(body):Json<serde_json::Value>| {
        let capture=capture.clone(); async move {
            capture.lock().unwrap().push((format!("{service}/{method}"),body.clone()));
            match method.as_str() {
                "ListRooms" => Json(serde_json::json!({"rooms":[{"name":body["names"][0],"metadata":"{\"presenters\":[\"kept\"]}"}]})),
                "ListParticipants" => Json(serde_json::json!({"participants":[]})),
                "CreateIngress" => Json(serde_json::json!({"ingressId":"IN_escape"})),
                _ => Json(serde_json::json!({})),
            }
        }
    }));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let mut state = Arc::try_unwrap(state).ok().unwrap();
    state.livekit.api_url = format!("http://{}", listener.local_addr().unwrap());
    let server = tokio::spawn(async move { axum::serve(listener, fixture).await.unwrap() });
    state.room_service().list_participants(&room).await.unwrap();
    state
        .room_service()
        .merge_room_metadata(
            &room,
            serde_json::json!({"sceneAdmins":[address_of(KEY)]})
                .as_object()
                .unwrap()
                .clone(),
        )
        .await
        .unwrap();
    state
        .room_service()
        .remove_participant(&room, &address_of(KEY))
        .await
        .unwrap();
    state
        .ingress()
        .create_ingress(&room, "streamer")
        .await
        .unwrap();
    let requests = observed.lock().unwrap();
    assert_eq!(requests.len(), 5);
    for (method, body) in requests.iter() {
        let sent = body
            .get("room")
            .or_else(|| body.get("roomName"))
            .or_else(|| body["names"].get(0))
            .unwrap();
        assert_eq!(sent, &serde_json::json!(room), "{method}");
    }
    let metadata: serde_json::Value = serde_json::from_str(
        requests
            .iter()
            .find(|(method, _)| method.ends_with("UpdateRoomMetadata"))
            .unwrap()
            .1["metadata"]
            .as_str()
            .unwrap(),
    )
    .unwrap();
    assert_eq!(metadata["presenters"], serde_json::json!(["kept"]));
    assert_eq!(
        metadata["sceneAdmins"],
        serde_json::json!([address_of(KEY)])
    );
    server.abort();
}

#[tokio::test]
async fn canonical_world_client_and_authoritative_tokens_are_byte_identical() {
    let state = Arc::new(canonical_state());
    for (realm, scene, expected) in [
        ("MiXeD.dcl.eth", "SceneA", "w1:d:mixed.dcl.ethSceneA"),
        ("MIXED.DCL.ETH", "SceneB", "w1:d:mixed.dcl.ethSceneB"),
        ("Single.eth", "SingleHash", "w1:a:single.ethSingleHash"),
        ("MiXeD.dcl.eth", "MiXeD.dcl.eth", "w1:d:mixed.dcl.ethSceneA"),
    ] {
        let metadata = serde_json::json!({"realmName":realm,"sceneId":scene});
        let Json(host) = handlers::get_server_scene_adapter(
            State(state.clone()),
            at("/get-server-scene-adapter"),
            signed(
                AUTHORITATIVE_KEY,
                "post",
                "/get-server-scene-adapter",
                &metadata.to_string(),
            )
            .await,
        )
        .await
        .unwrap();
        let claims = jwt_claims(&host.adapter);
        assert_eq!(
            claims["video"]["room"], expected,
            "host for {realm}/{scene}"
        );
        assert_eq!(claims["sub"], "authoritative-server");
        let path = format!(
            "/worlds/{realm}/scenes/{}/comms",
            if scene.ends_with(".eth") {
                "SceneA"
            } else {
                scene
            }
        );
        let mut request = Request::post(&path).body(Body::empty()).unwrap();
        request
            .headers_mut()
            .extend(signed(KEY, "post", &path, "{}").await);
        let (status, value) = body_json(
            super::super::admission::world_mint::router(state.clone())
                .oneshot(request)
                .await
                .unwrap(),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{value}");
        let client = jwt_claims(value["fixedAdapter"].as_str().unwrap());
        assert_eq!(client["video"]["room"], expected);
        assert_eq!(client["video"]["room"], claims["video"]["room"]);
    }
}

#[derive(Default)]
struct ProjectedSink(std::sync::Mutex<Vec<crate::protocol::bus::room_events::RoomEvent>>);

impl super::super::events::RoomEventSink for ProjectedSink {
    fn observe_room_event(&self, event: &crate::protocol::bus::room_events::RoomEvent) {
        self.0.lock().unwrap().push(event.clone());
    }
}

#[tokio::test]
async fn canonical_webhook_projects_world_scene_lobby_and_shared_identity_once() {
    use crate::protocol::bus::room_events::{RoomEventKind, RoomType};
    let sink = Arc::new(ProjectedSink::default());
    let mut state = canonical_state().with_room_event_sink(sink.clone());
    state.livekit.configured = true;
    // This fixture projects identity without a current moderation footprint.
    // Initial metadata writes have their own SQL/SFU-backed conformance tests.
    state.scenes = Arc::new(RetryCatalogue(std::sync::atomic::AtomicBool::new(false)));
    let (key, secret) = (
        state.livekit.api_key.clone(),
        state.livekit.api_secret.clone(),
    );
    let app = super::super::router(Arc::new(state));
    for (n, (room, kind, realm, scene, room_type)) in [
        (
            "w1:d:mixed.dcl.ethSceneA",
            "participant_joined",
            "mixed.dcl.eth",
            "SceneA",
            RoomType::World,
        ),
        (
            "w1:d:mixed.dcl.ethSceneB",
            "participant_left",
            "mixed.dcl.eth",
            "SceneB",
            RoomType::World,
        ),
        (
            "world-mixed.dcl.eth",
            "participant_joined",
            "mixed.dcl.eth",
            "",
            RoomType::World,
        ),
        (
            "scene:main:GenesisA",
            "participant_joined",
            "main",
            "GenesisA",
            RoomType::Scene,
        ),
        (
            "scene:LocalPreview:GenesisA",
            "participant_joined",
            "LocalPreview",
            "GenesisA",
            RoomType::Scene,
        ),
    ]
    .into_iter()
    .enumerate()
    {
        let raw = serde_json::to_vec(&serde_json::json!({"id":format!("EV_{n}"),"createdAt":1718900000+n,"event":kind,"room":{"name":room},"participant":{"identity":"0xAaAa","sid":format!("PA_{n}")}})).unwrap();
        let jwt = super::super::webhook::test_support::webhook_jwt(&raw, &key, &secret);
        assert_eq!(post_webhook(&app, Some(&jwt), &raw).await, StatusCode::OK);
        assert_eq!(post_webhook(&app, Some(&jwt), &raw).await, StatusCode::OK);
        let events = sink.0.lock().unwrap();
        assert_eq!(events.len(), n + 1, "deduplication holds");
        let projected = &events[n];
        assert_eq!(
            (projected.realm_name.as_str(), projected.scene_id.as_str()),
            (realm, scene)
        );
        assert_eq!(projected.room_type(), room_type);
        assert_eq!(
            projected.kind(),
            if kind == "participant_joined" {
                RoomEventKind::Joined
            } else {
                RoomEventKind::Left
            }
        );
        assert_eq!(projected.identity, "0xAaAa");
        assert_eq!(projected.participant_sid, format!("PA_{n}"));
        assert_eq!(projected.event_id, format!("EV_{n}"));
        assert_eq!(projected.event_time_ms, (1718900000 + n as u64) * 1000);
        assert_eq!(
            crate::protocol::bus::room_events::RoomEvent::decode(&projected.encode().unwrap())
                .unwrap(),
            *projected
        );
        if room.starts_with("world-") {
            assert!(
                projected.scene_id.is_empty(),
                "a lobby supplies no scene to start"
            );
        }
    }
}

struct RetryCatalogue(std::sync::atomic::AtomicBool);
#[async_trait::async_trait]
impl SceneIdentity for RetryCatalogue {
    async fn classify_room(
        &self,
        room: &str,
    ) -> Result<crate::rooms::RoomClassification, crate::rooms::ResolveError> {
        crate::interfaces::test_support::classify_fixture(self, room).await
    }

    async fn resolve(&self, request: RoomRequest<'_>) -> Result<ResolvedRoom, ResolveError> {
        if self.0.swap(false, std::sync::atomic::Ordering::SeqCst) {
            return Err(ResolveError::Db(sqlx::Error::PoolClosed));
        }
        let mut resolved = match request {
            RoomRequest::Adapter {
                realm: RealmId::Shared(realm),
                scene,
            } => ResolvedRoom {
                room: crate::rooms::gatekeeper_scene_room(realm, scene.as_str()),
                realm: RealmId::Shared(realm.clone()),
                scene_id: Some(scene.clone()),
                deployment: None,
            },
            _ => Catalogue.resolve(request).await?,
        };
        resolved.deployment = None;
        Ok(resolved)
    }
    async fn world_scene(
        &self,
        world: &WorldName,
        scene: &SceneId,
    ) -> Result<Option<ActiveDeployment>, ResolveError> {
        Catalogue.world_scene(world, scene).await
    }
    async fn world_active_scene(
        &self,
        world: &WorldName,
    ) -> Result<Option<ActiveDeployment>, ResolveError> {
        Catalogue.world_active_scene(world).await
    }
}

#[tokio::test]
async fn canonical_webhook_catalogue_fault_retries_before_marking_applied() {
    let sink = Arc::new(ProjectedSink::default());
    let mut state = canonical_state().with_room_event_sink(sink.clone());
    state.livekit.configured = true;
    state.scenes = Arc::new(RetryCatalogue(std::sync::atomic::AtomicBool::new(true)));
    let raw=serde_json::to_vec(&serde_json::json!({"id":"EV_retry","createdAt":1718900000,"event":"participant_joined","room":{"name":"w1:d:mixed.dcl.ethSceneA"},"participant":{"identity":"0xAaAa","sid":"PA_retry"}})).unwrap();
    let jwt = super::super::webhook::test_support::webhook_jwt(
        &raw,
        &state.livekit.api_key,
        &state.livekit.api_secret,
    );
    let app = super::super::router(Arc::new(state));
    assert_eq!(
        post_webhook(&app, Some(&jwt), &raw).await,
        StatusCode::SERVICE_UNAVAILABLE
    );
    assert!(sink.0.lock().unwrap().is_empty());
    assert_eq!(post_webhook(&app, Some(&jwt), &raw).await, StatusCode::OK);
    assert_eq!(post_webhook(&app, Some(&jwt), &raw).await, StatusCode::OK);
    assert_eq!(sink.0.lock().unwrap().len(), 1);
}

#[tokio::test]
async fn canonical_webhook_removed_deployment_leave_invalidates_without_inventing_observation() {
    let sink = Arc::new(ProjectedSink::default());
    let mut state = canonical_state().with_room_event_sink(sink.clone());
    state.livekit.configured = true;
    state.presence.peer_joined(
        "0xAaAa",
        "mixed.dcl.eth",
        "scene-mixed.dcl.eth-replaced",
        true,
    );
    let presence = state.presence.clone();
    let raw=serde_json::to_vec(&serde_json::json!({"id":"EV_removed_leave","createdAt":1718900000,"event":"participant_left","room":{"name":"scene-mixed.dcl.eth-replaced"},"participant":{"identity":"0xAaAa","sid":"PA_removed"}})).unwrap();
    let jwt = super::super::webhook::test_support::webhook_jwt(
        &raw,
        &state.livekit.api_key,
        &state.livekit.api_secret,
    );
    let app = super::super::router(Arc::new(state));
    assert_eq!(post_webhook(&app, Some(&jwt), &raw).await, StatusCode::OK);
    assert_eq!(
        presence.get_peer_world("0xAaAa").unwrap().as_deref(),
        Some("mixed.dcl.eth"),
        "a delayed leave cannot overwrite complete membership; recovery owns replacement"
    );
    assert!(
        sink.0.lock().unwrap().is_empty(),
        "unknown deployment cannot invent a hosting event"
    );
}

fn verified_room(room: &str) -> crate::gatekeeper::admission::relay_authority::proof::VerifiedRoom {
    let wallet = address_of(KEY);
    crate::gatekeeper::admission::relay_authority::proof::VerifiedRoom {
        claims: serde_json::from_value(serde_json::json!({"iss":"key","sub":wallet,"exp":u64::MAX,"video":{"room":room,"roomJoin":true,"canPublishData":true}})).unwrap(),
        wallet, session:address_of(OUTSIDER_KEY),is_guest:false,header_payload:String::new(),nonce:[0;32],proof:vec![],
    }
}

#[tokio::test]
async fn canonical_relay_world_rooms_keep_world_policy_deployment_bans_and_host_identity() {
    use super::super::admission::relay_authority::{RelayPolicy, policy::CurrentRoomPolicy};
    use super::super::moderation::test_support::StubBanStatus;
    use crate::protocol::room_authority::denied::Reason as DenialReason;
    let actor = Address::parse(&address_of(AUTHORITATIVE_KEY)).unwrap();
    let policy = CurrentRoomPolicy::new(
        Arc::new(Catalogue),
        Arc::new(StubPolicy::default()),
        Arc::new(StubBanStatus(Ok(None))),
        Some(actor.clone()),
    );
    let allowed = verified_room("w1:d:mixed.dcl.ethSceneA");
    assert_eq!(policy.authorize(&allowed).await, Ok(()));
    for room in [
        "world-foo.dcl.eth-scene",
        "world-mixed.dcl.eth-scenea",
        "world-deleted.dcl.eth-scene",
        "scene-other.dcl.eth-scenea",
        "scene-mixed.dcl.eth-replaced",
        "scene-MIXED.dcl.eth-scenea",
        "scene-mixed.dcl.eth- SceneA",
        "scene-mixed.dcl.eth-a/b",
    ] {
        assert_eq!(
            policy.authorize(&verified_room(room)).await,
            Err(DenialReason::NotAuthorized),
            "{room}"
        );
    }
    let deny_world = CurrentRoomPolicy::new(
        Arc::new(Catalogue),
        Arc::new(StubPolicy {
            access: Ok(crate::access::Access::NotAllowed),
            ..Default::default()
        }),
        Arc::new(StubBanStatus(Ok(None))),
        None,
    );
    assert_eq!(
        deny_world.authorize(&allowed).await,
        Err(DenialReason::NotAuthorized)
    );
    assert_eq!(
        deny_world
            .authorize(&verified_room("scene:main:GenesisA"))
            .await,
        Ok(()),
        "shared scenes preserve their own authority path"
    );
    for standing in [
        crate::access::WorldStanding::Unknown,
        crate::access::WorldStanding::Disabled,
        crate::access::WorldStanding::DenyListed,
    ] {
        let invalid_world = CurrentRoomPolicy::new(
            Arc::new(Catalogue),
            Arc::new(StubPolicy {
                standing: Ok(standing),
                ..Default::default()
            }),
            Arc::new(StubBanStatus(Ok(None))),
            None,
        );
        assert_eq!(
            invalid_world.authorize(&allowed).await,
            Err(DenialReason::NotAuthorized),
            "{standing:?}"
        );
    }
    for ban in [crate::access::Ban::Platform, crate::access::Ban::Scene] {
        let banned = CurrentRoomPolicy::new(
            Arc::new(Catalogue),
            Arc::new(StubPolicy::default()),
            Arc::new(StubBanStatus(Ok(Some(ban)))),
            None,
        );
        assert_eq!(
            banned.authorize(&allowed).await,
            Err(DenialReason::NotAuthorized)
        );
    }
    let mut host = allowed;
    host.claims.sub = "authoritative-server".into();
    assert_eq!(
        policy.authorize(&host).await,
        Err(DenialReason::NotAuthorized)
    );
    host.claims.metadata =
        serde_json::json!({"catalyrstAuthoritativeServer":{"wallet":actor.as_str()}}).to_string();
    assert_eq!(policy.authorize(&host).await, Ok(()));
    host.claims.metadata =
        serde_json::json!({"catalyrstAuthoritativeServer":{"wallet":address_of(OUTSIDER_KEY)}})
            .to_string();
    assert_eq!(
        policy.authorize(&host).await,
        Err(DenialReason::NotAuthorized)
    );
}

#[tokio::test]
async fn canonical_adapter_rejects_malformed_signed_keys_and_cross_world_deployments() {
    let state = Arc::new(canonical_state());
    for (realm, scene, status) in [
        (" mixed.dcl.eth", "SceneA", 400),
        ("mixed.dcl.eth", " SceneA", 400),
        ("mixed.dcl.eth", "SceneA ", 400),
        ("mixed.dcl.eth", "bad/scene", 400),
        ("main", "bad scene", 400),
        ("main:other", "SceneA", 400),
        ("main", "SceneA:other", 400),
        ("bad/realm", "SceneA", 400),
        ("other.dcl.eth", "SceneA", 404),
        ("mixed.dcl.eth", "ReplacedScene", 404),
    ] {
        let metadata = serde_json::json!({"realmName":realm,"sceneId":scene});
        let err = handlers::get_server_scene_adapter(
            State(state.clone()),
            at("/get-server-scene-adapter"),
            signed(
                AUTHORITATIVE_KEY,
                "post",
                "/get-server-scene-adapter",
                &metadata.to_string(),
            )
            .await,
        )
        .await
        .unwrap_err();
        assert_eq!(err.code, status, "{realm:?}/{scene:?}: {}", err.message);
    }
}

#[tokio::test]
async fn canonical_participant_lookup_targets_the_deployment_and_distinct_lobby() {
    let observed = Arc::new(std::sync::Mutex::new(Vec::<String>::new()));
    let capture = observed.clone();
    let fixture = axum::Router::new().route("/twirp/livekit.RoomService/ListParticipants", axum::routing::post(move |Json(body): Json<serde_json::Value>| {
        let capture = capture.clone();
        async move {
            capture.lock().unwrap().push(body["room"].as_str().unwrap().to_string());
            Json(serde_json::json!({"participants":[{"identity":"authoritative-server"},{"identity":"0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"}]}))
        }
    }));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let mut state = canonical_state();
    state.livekit.configured = true;
    state.livekit.api_url = format!("http://{}", listener.local_addr().unwrap());
    let server = tokio::spawn(async move { axum::serve(listener, fixture).await.unwrap() });
    for pointer in [Some("0,0"), Some("1,1"), None] {
        let addresses = handlers::participants(
            &state,
            handlers::ParticipantsQuery {
                realm_name: Some("MiXeD.dcl.eth".into()),
                room: None,
                pointer: pointer.map(str::to_string),
            },
        )
        .await
        .unwrap();
        assert_eq!(
            addresses,
            vec!["0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"]
        );
    }
    assert_eq!(
        *observed.lock().unwrap(),
        [
            "w1:d:mixed.dcl.ethSceneA",
            "w1:d:mixed.dcl.ethSceneB",
            "world-mixed.dcl.eth"
        ]
    );
    server.abort();
}

#[tokio::test]
async fn canonical_world_scene_name_resolves_original_case_before_relay_authorization() {
    let resolved = crate::rooms::resolve_room_name(&Catalogue, "w1:d:mixed.dcl.ethSceneA")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(resolved.realm.world().unwrap().key(), "mixed.dcl.eth");
    assert_eq!(resolved.scene_id.unwrap().as_str(), "SceneA");
    assert!(
        crate::rooms::resolve_room_name(&Catalogue, "scene-other.dcl.eth-scenea")
            .await
            .unwrap()
            .is_none()
    );
    assert!(
        crate::rooms::resolve_room_name(&Catalogue, "scene-mixed.dcl.eth-replaced")
            .await
            .unwrap()
            .is_none()
    );
}

#[tokio::test]
async fn canonical_moderation_metadata_and_kick_target_the_joined_scene_and_lobby() {
    let observed = Arc::new(std::sync::Mutex::new(
        Vec::<(String, serde_json::Value)>::new(),
    ));
    let capture = observed.clone();
    let api=axum::Router::new().route("/twirp/livekit.RoomService/{method}",axum::routing::post(move |Path(method):Path<String>,Json(body):Json<serde_json::Value>|{
        let capture=capture.clone(); async move {
            capture.lock().unwrap().push((method.clone(),body.clone()));
            if method=="ListRooms" { Json(serde_json::json!({"rooms":[{"name":body["names"][0],"metadata":"{\"presenters\":[\"retained\"]}"}]})) }
            else { Json(serde_json::json!({})) }
        }
    }));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let mut state = canonical_state();
    state.livekit.configured = true;
    state.livekit.api_url = format!("http://{}", listener.local_addr().unwrap());
    let server = tokio::spawn(async move { axum::serve(listener, api).await.unwrap() });
    let ctx = RoomContext {
        place_id: "fixture-place".into(),
        realm_name: Some("MiXeD.DCL.ETH".into()),
        scene_id: Some("SceneB".into()),
        parcel: Some("1,1".into()),
    };
    let place = scene::test_support::cached_place(
        &state,
        "fixture-place",
        scene::PlaceInfo {
            world: true,
            world_name: Some("mixed.dcl.eth".into()),
            positions: vec!["0,0".into(), "1,1".into()],
            base_position: Some("0,0".into()),
        },
    );
    let rooms = scene::resolve_rooms(&state, &ctx, &place).await.unwrap();
    assert_eq!(rooms, ["world-mixed.dcl.eth", "w1:d:mixed.dcl.ethSceneB"]);
    let actor = Address::parse(&address_of(KEY)).unwrap();
    scene::add_ban(&state, &rooms, &actor).await;
    scene::kick(&state, &rooms, &actor).await;
    let requests = observed.lock().unwrap();
    for room in ["world-mixed.dcl.eth", "w1:d:mixed.dcl.ethSceneB"] {
        let metadata = &requests
            .iter()
            .find(|(method, body)| method == "UpdateRoomMetadata" && body["room"] == room)
            .expect("metadata update")
            .1;
        let value: serde_json::Value =
            serde_json::from_str(metadata["metadata"].as_str().unwrap()).unwrap();
        assert_eq!(
            value["bannedAddresses"],
            serde_json::json!([actor.as_str()])
        );
        assert_eq!(value["presenters"], serde_json::json!(["retained"]));
        assert!(
            requests
                .iter()
                .any(|(method, body)| method == "RemoveParticipant"
                    && body["room"] == room
                    && body["identity"] == actor.as_str()),
            "kick must reach {room}"
        );
    }
    assert!(
        !requests
            .iter()
            .any(|(_, body)| body["room"] == "w1:d:mixed.dcl.ethSceneA"),
        "other deployment is isolated"
    );
    server.abort();
}

#[tokio::test]
async fn canonical_stream_route_creates_and_persists_ingress_for_the_joined_scene() {
    let Some(pool) = scratch().await else { return };
    let place_id = format!("canonical-stream-{}", tag());
    seed_place(&pool,&place_id,serde_json::json!({"world":true,"world_name":"mixed.dcl.eth","positions":["1,1"],"owner":address_of(KEY)}),"1,1").await;
    let observed = Arc::new(std::sync::Mutex::new(Vec::<serde_json::Value>::new()));
    let capture = observed.clone();
    let fixture=axum::Router::new().route("/twirp/livekit.Ingress/CreateIngress",axum::routing::post(move|Json(body):Json<serde_json::Value>|{
        let capture=capture.clone();async move{capture.lock().unwrap().push(body);Json(serde_json::json!({"ingressId":"IN_canonical","url":"rtmp://ingress.example/live","streamKey":"fixture-key"}))}
    }));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let mut state = comms_state(pool.clone(), with_authoritative(AUTHORITATIVE_KEY))
        .with_ban_status(Arc::new(
            super::super::moderation::test_support::StubBanStatus(Ok(None)),
        ));
    state.scenes = Arc::new(Catalogue);
    state.policy = Arc::new(StubPolicy {
        admin: PermissionDecision::Allowed,
        ..Default::default()
    });
    state.livekit.configured = true;
    state.livekit.api_url = format!("http://{}", listener.local_addr().unwrap());
    let server = tokio::spawn(async move { axum::serve(listener, fixture).await.unwrap() });
    let metadata = serde_json::json!({"signer":super::super::auth::SCENE_SIGNER,"realmName":"MiXeD.DCL.ETH","sceneId":"SceneB","parcel":"1,1"});
    let response = super::super::streaming::scene_stream_access(
        State(Arc::new(state)),
        at("/scene-stream-access"),
        axum::http::Method::POST,
        signed(KEY, "post", "/scene-stream-access", &metadata.to_string()).await,
    )
    .await
    .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        observed.lock().unwrap()[0]["roomName"],
        "w1:d:mixed.dcl.ethSceneB"
    );
    let stored: String = sqlx::query_scalar(
        "SELECT room_id FROM social.scene_stream_access WHERE place_id = $1 AND active",
    )
    .bind(&place_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(stored, "w1:d:mixed.dcl.ethSceneB");
    sqlx::query("DELETE FROM social.scene_stream_access WHERE place_id = $1")
        .bind(&place_id)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("DELETE FROM explore.place WHERE id = $1")
        .bind(&place_id)
        .execute(&pool)
        .await
        .unwrap();
    server.abort();
}

#[tokio::test]
async fn canonical_world_revocation_inventory_rejects_another_worlds_shared_prefix() {
    let fixture=axum::Router::new().route("/twirp/livekit.RoomService/ListRooms",axum::routing::post(||async{Json(serde_json::json!({"rooms":[{"name":"world-mixed.dcl.eth"},{"name":"w1:d:mixed.dcl.ethSceneA"},{"name":"w1:d:mixed.dcl.ethSceneB"},{"name":"w1:j:mixed.dcl.eth-otherSceneA"},{"name":"world-other.eth"}]}))}));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let mut state = canonical_state();
    state.livekit.configured = true;
    state.livekit.api_url = format!("http://{}", listener.local_addr().unwrap());
    let server = tokio::spawn(async move { axum::serve(listener, fixture).await.unwrap() });
    let mut rooms = scene::live_world_rooms(&state, &WorldName::parse("MiXeD.dcl.eth").unwrap())
        .await
        .unwrap();
    rooms.sort();
    assert_eq!(
        rooms,
        [
            "w1:d:mixed.dcl.ethSceneA",
            "w1:d:mixed.dcl.ethSceneB",
            "world-mixed.dcl.eth"
        ]
    );
    server.abort();
}

#[tokio::test]
async fn refused_complete_rooms_never_project_webhook_identity_or_host_events() {
    let sink = Arc::new(ProjectedSink::default());
    let mut state = canonical_state().with_room_event_sink(sink.clone());
    state.livekit.configured = true;
    let presence = state.presence.clone();
    let key = state.livekit.api_key.clone();
    let secret = state.livekit.api_secret.clone();
    let app = super::super::router(Arc::new(state));
    for (index, room) in [
        "world-foo.dcl.eth-scene",
        "world-mixed.dcl.eth-scenea",
        "world-deleted.dcl.eth-scene",
        "world-MiXeD.dcl.eth",
    ]
    .into_iter()
    .enumerate()
    {
        let raw = serde_json::to_vec(&serde_json::json!({"id":format!("EV_refused_{index}"), "createdAt":1718900000,"event":"participant_joined", "room":{"name":room}, "participant":{"identity":"0xAaAa","sid":"PA_refused"}})).unwrap();
        let jwt = super::super::webhook::test_support::webhook_jwt(&raw, &key, &secret);
        assert_eq!(post_webhook(&app, Some(&jwt), &raw).await, StatusCode::OK);
    }
    assert!(sink.0.lock().unwrap().is_empty());
    assert_eq!(presence.get_peer_world("0xAaAa").unwrap(), None);
}

#[tokio::test]
async fn refused_room_among_canonical_rooms_makes_revocation_inventory_incomplete() {
    let fixture = axum::Router::new().route("/twirp/livekit.RoomService/ListRooms", axum::routing::post(|| async { Json(serde_json::json!({"rooms":[{"name":"w1:d:mixed.dcl.ethSceneA"},{"name":"world-foo.dcl.eth-scene"}]})) }));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let mut state = canonical_state();
    state.livekit.configured = true;
    state.livekit.api_url = format!("http://{}", listener.local_addr().unwrap());
    let server = tokio::spawn(async move { axum::serve(listener, fixture).await.unwrap() });
    let error = scene::live_world_rooms(&state, &WorldName::parse("mixed.dcl.eth").unwrap())
        .await
        .unwrap_err();
    assert!(error.contains("world-foo.dcl.eth-scene") && error.contains("Ambiguous"));
    server.abort();
}

#[tokio::test]
async fn relay_classifier_store_fault_is_unavailable() {
    use super::super::admission::relay_authority::{RelayPolicy, policy::CurrentRoomPolicy};
    use super::super::moderation::test_support::StubBanStatus;
    use crate::protocol::room_authority::denied::Reason as DenialReason;
    let policy = CurrentRoomPolicy::new(
        Arc::new(RetryCatalogue(std::sync::atomic::AtomicBool::new(true))),
        Arc::new(StubPolicy::default()),
        Arc::new(StubBanStatus(Ok(None))),
        None,
    );
    assert_eq!(
        policy
            .authorize(&verified_room("w1:d:mixed.dcl.ethSceneA"))
            .await,
        Err(DenialReason::Unavailable)
    );
}

#[tokio::test]
async fn inventory_distinguishes_empty_unrelated_transport_and_catalogue_failure() {
    let responses = Arc::new(std::sync::Mutex::new(std::collections::VecDeque::from([
        (StatusCode::OK, serde_json::json!({"rooms":[]})),
        (
            StatusCode::OK,
            serde_json::json!({"rooms":[{"name":"island-example"},{"name":"voice-chat-private-example"}]}),
        ),
        (
            StatusCode::SERVICE_UNAVAILABLE,
            serde_json::json!({"error":"unavailable"}),
        ),
        (
            StatusCode::OK,
            serde_json::json!({"rooms":[{"name":"world-mixed.dcl.eth"}]}),
        ),
    ])));
    let fixture = axum::Router::new().route(
        "/twirp/livekit.RoomService/ListRooms",
        axum::routing::post(move || {
            let responses = responses.clone();
            async move {
                let (status, value) = responses.lock().unwrap().pop_front().unwrap();
                (status, Json(value))
            }
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let mut state = canonical_state();
    state.livekit.configured = true;
    state.livekit.api_url = format!("http://{}", listener.local_addr().unwrap());
    let server = tokio::spawn(async move { axum::serve(listener, fixture).await.unwrap() });
    let world = WorldName::parse("mixed.dcl.eth").unwrap();
    assert!(
        scene::live_world_rooms(&state, &world)
            .await
            .unwrap()
            .is_empty()
    );
    assert!(
        scene::live_world_rooms(&state, &world)
            .await
            .unwrap()
            .is_empty()
    );
    assert!(scene::live_world_rooms(&state, &world).await.is_err());
    state.scenes = Arc::new(RetryCatalogue(std::sync::atomic::AtomicBool::new(true)));
    assert!(scene::live_world_rooms(&state, &world).await.is_err());
    server.abort();
}

struct RedeployCatalogue(Arc<std::sync::atomic::AtomicBool>);

#[async_trait::async_trait]
impl SceneIdentity for RedeployCatalogue {
    async fn classify_room(
        &self,
        room: &str,
    ) -> Result<crate::rooms::RoomClassification, ResolveError> {
        crate::interfaces::test_support::classify_fixture(self, room).await
    }

    async fn resolve(&self, request: RoomRequest<'_>) -> Result<ResolvedRoom, ResolveError> {
        if self.0.load(std::sync::atomic::Ordering::SeqCst)
            && matches!(&request, RoomRequest::NamedWorldScene { room } if *room == "w1:d:mixed.dcl.ethSceneA")
        {
            return Err(ResolveError::UnresolvedRoom(
                "w1:d:mixed.dcl.ethSceneA".into(),
            ));
        }
        Catalogue.resolve(request).await
    }
    async fn world_scene(
        &self,
        world: &WorldName,
        scene: &SceneId,
    ) -> Result<Option<ActiveDeployment>, ResolveError> {
        if self.0.load(std::sync::atomic::Ordering::SeqCst)
            && world.key() == "mixed.dcl.eth"
            && scene.as_str() == "SceneA"
        {
            return Ok(None);
        }
        Catalogue.world_scene(world, scene).await
    }
    async fn world_active_scene(
        &self,
        world: &WorldName,
    ) -> Result<Option<ActiveDeployment>, ResolveError> {
        if self.0.load(std::sync::atomic::Ordering::SeqCst) && world.key() == "mixed.dcl.eth" {
            return Catalogue
                .world_scene(world, &SceneId::parse("SceneB").unwrap())
                .await;
        }
        Catalogue.world_active_scene(world).await
    }
}

#[tokio::test]
async fn canonical_world_revocation_reaches_a_connected_room_after_redeployment() {
    use super::super::RoomRevocation;
    for access_changed in [false, true] {
        let requests = Arc::new(std::sync::Mutex::new(
            Vec::<(String, serde_json::Value)>::new(),
        ));
        let capture = requests.clone();
        let fixture = axum::Router::new().route("/twirp/livekit.RoomService/{method}",axum::routing::post(move |Path(method):Path<String>,Json(body):Json<serde_json::Value>| {
            let capture=capture.clone(); async move {
                capture.lock().unwrap().push((method.clone(),body.clone()));
                match method.as_str() {
                    "ListRooms" => Json(serde_json::json!({"rooms":[{"name":"w1:d:mixed.dcl.ethSceneA","sid":"RM_deployed_a","metadata":"{}"},{"name":"w1:j:mixed.dcl.eth-otherSceneA","sid":"RM_other","metadata":"{}"}]})),
                    "ListParticipants" => Json(serde_json::json!({"participants":[{"identity":address_of(KEY),"sid":"PA_connected","metadata":"{}"}]})),
                    _ => Json(serde_json::json!({})),
                }
            }
        }));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let removed = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let mut state = canonical_state();
        state.scenes = Arc::new(RedeployCatalogue(removed.clone()));
        state.livekit.configured = true;
        state.livekit.api_url = format!("http://{}", listener.local_addr().unwrap());
        state.policy = Arc::new(StubPolicy {
            access: Ok(crate::access::Access::NotAllowed),
            ..Default::default()
        });
        let server = tokio::spawn(async move { axum::serve(listener, fixture).await.unwrap() });
        let state = Arc::new(state);
        // The live scene room was admitted and had a uniquely resolved owner
        // before the normal replacement removed its catalogue row.
        assert_eq!(
            scene::live_world_rooms(&state, &WorldName::parse("mixed.dcl.eth").unwrap())
                .await
                .unwrap(),
            vec!["w1:d:mixed.dcl.ethSceneA"]
        );
        removed.store(true, std::sync::atomic::Ordering::SeqCst);
        assert!(
            crate::rooms::resolve_room_name(state.scenes.as_ref(), "w1:d:mixed.dcl.ethSceneA")
                .await
                .unwrap()
                .is_none(),
            "old room ownership is independent of active-deployment authority"
        );
        assert!(
            state
                .scenes
                .world_scene(
                    &WorldName::parse("mixed.dcl.eth").unwrap(),
                    &SceneId::parse("SceneA").unwrap()
                )
                .await
                .unwrap()
                .is_none()
        );
        let world = WorldName::parse("mixed.dcl.eth").unwrap();
        state.revoke(if access_changed {
            RoomRevocation::WorldAccess { world }
        } else {
            RoomRevocation::WorldAddress {
                world,
                address: Address::parse(&address_of(KEY)).unwrap(),
            }
        });
        let observed = tokio::time::timeout(std::time::Duration::from_millis(200), async {
            loop {
                if requests.lock().unwrap().iter().any(|(method, body)| {
                    method == "RemoveParticipant" && body["room"] == "w1:d:mixed.dcl.ethSceneA"
                }) {
                    break;
                }
                tokio::time::sleep(std::time::Duration::from_millis(2)).await;
            }
        })
        .await;
        assert!(
            observed.is_ok(),
            "redeployment must not hide an already connected room from revocation (access_changed={access_changed})"
        );
        assert!(
            !requests
                .lock()
                .unwrap()
                .iter()
                .any(|(method, body)| method == "RemoveParticipant"
                    && body["room"] == "w1:j:mixed.dcl.eth-otherSceneA"),
            "another world's shared prefix must remain isolated"
        );
        server.abort();
    }
}

/// The old access-change handler: the owner and deployers stay, an
/// admitted or undecided participant stays, on both revocation arms.
#[tokio::test]
async fn world_revocation_spares_privileged_admitted_and_undecided_participants() {
    use super::super::RoomRevocation;
    let connected = Address::parse(&address_of(KEY)).unwrap();
    let unavailable = || {
        crate::AuthorityNotEstablished::store_unavailable(
            "worlds.worlds",
            &sqlx::Error::PoolTimedOut,
        )
    };
    let policy = |case: usize| match case {
        0 => StubPolicy {
            access: Ok(crate::access::Access::NotAllowed),
            privileged: Ok([connected.clone()].into_iter().collect()),
            ..Default::default()
        },
        1 => StubPolicy::default(),
        2 => StubPolicy {
            access: Err(unavailable()),
            ..Default::default()
        },
        _ => StubPolicy {
            access: Ok(crate::access::Access::NotAllowed),
            privileged: Err(unavailable()),
            ..Default::default()
        },
    };
    for case in 0..4 {
        for access_changed in [false, true] {
            let requests = Arc::new(std::sync::Mutex::new(Vec::<String>::new()));
            let capture = requests.clone();
            let fixture = axum::Router::new().route("/twirp/livekit.RoomService/{method}",axum::routing::post(move |Path(method):Path<String>| {
                let capture=capture.clone(); async move {
                    capture.lock().unwrap().push(method.clone());
                    match method.as_str() {
                        "ListRooms" => Json(serde_json::json!({"rooms":[{"name":"w1:d:mixed.dcl.ethSceneA","sid":"RM_a","metadata":"{}"}]})),
                        "ListParticipants" => Json(serde_json::json!({"participants":[{"identity":address_of(KEY),"sid":"PA_connected","metadata":"{}"}]})),
                        _ => Json(serde_json::json!({})),
                    }
                }
            }));
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let mut state = canonical_state();
            state.livekit.configured = true;
            state.livekit.api_url = format!("http://{}", listener.local_addr().unwrap());
            state.policy = Arc::new(policy(case));
            let server = tokio::spawn(async move { axum::serve(listener, fixture).await.unwrap() });
            let world = WorldName::parse("mixed.dcl.eth").unwrap();
            Arc::new(state).revoke(if access_changed {
                RoomRevocation::WorldAccess { world }
            } else {
                RoomRevocation::WorldAddress {
                    world,
                    address: connected.clone(),
                }
            });
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            if access_changed && case < 3 {
                assert!(
                    requests
                        .lock()
                        .unwrap()
                        .iter()
                        .any(|m| m == "ListParticipants"),
                    "case {case}: the roster was read"
                );
            }
            assert!(
                !requests
                    .lock()
                    .unwrap()
                    .iter()
                    .any(|m| m == "RemoveParticipant"),
                "case {case} access_changed={access_changed}: {:?}",
                requests.lock().unwrap()
            );
            server.abort();
        }
    }
}
