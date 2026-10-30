use super::*;

#[test]
fn startup_has_unknown_source_time_and_no_admission_authority() {
    let presence = Presence::new();
    assert_eq!(presence.last_updated(), "1970-01-01T00:00:00.000Z");
    assert!(presence.world_counts().is_empty());
    assert!(
        presence
            .admission_count(&Scope::Lobby("foo.dcl.eth".into()))
            .is_none()
    );
}

#[test]
fn dirty_events_never_create_membership_or_source_time() {
    let presence = Presence::new();
    presence.invalidate_room("world-foo.dcl.eth");
    assert!(presence.world_counts().is_empty());
    assert_eq!(presence.last_updated(), "1970-01-01T00:00:00.000Z");
}

use crate::gatekeeper::{CommsState, Settings};
use crate::interfaces::test_support::{StubDirectory, StubPolicy};
use crate::rooms::{RoomClassification, RoomRefusal};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

#[derive(Default)]
struct Source {
    fail: AtomicBool,
    empty: AtomicBool,
    sid_changed: AtomicBool,
    enumerate: AtomicUsize,
    rooms: Mutex<Vec<(String, String, Vec<(String, String)>)>>,
    dirty_on_read: Mutex<Option<Arc<Presence>>>,
    not_found_remaining: AtomicUsize,
    recreate_on_not_found: AtomicBool,
    roster_reads: AtomicUsize,
}

async fn fixture() -> (Arc<CommsState>, Arc<Source>, tokio::task::JoinHandle<()>) {
    use axum::extract::OriginalUri;
    let source = Arc::new(Source::default());
    source.rooms.lock().push((
        "world-foo.dcl.eth".into(),
        "RM_1".into(),
        vec![("watch:foo".into(), "PA_1".into())],
    ));
    let app = Router::new().route("/twirp/livekit.RoomService/{method}", axum::routing::post(
        |State(source): State<Arc<Source>>, OriginalUri(uri): OriginalUri, Json(body): Json<Value>| async move {
            use axum::response::IntoResponse;
            if source.fail.load(Ordering::SeqCst) { return axum::http::StatusCode::SERVICE_UNAVAILABLE.into_response(); }
            if uri.path().ends_with("ListRooms") {
                let call = source.enumerate.fetch_add(1, Ordering::SeqCst);
                let values = if source.empty.load(Ordering::SeqCst) { vec![] } else {
                    source.rooms.lock().iter().map(|(name,sid,_)| json!({"name":name,"sid":if source.sid_changed.load(Ordering::SeqCst) && call%2 == 1 { "RM_recreated" } else { sid }})).collect::<Vec<_>>()
                };
                Json(json!({"rooms":values})).into_response()
            } else {
                source.roster_reads.fetch_add(1, Ordering::SeqCst);
                if source.not_found_remaining.try_update(Ordering::SeqCst, Ordering::SeqCst, |n| n.checked_sub(1)).is_ok() {
                    if source.recreate_on_not_found.load(Ordering::SeqCst) {
                        source.rooms.lock()[0].1 = "RM_recreated_after_missing".into();
                    }
                    return axum::http::StatusCode::NOT_FOUND.into_response();
                }
                if let Some(presence) = source.dirty_on_read.lock().as_ref() {
                    presence.invalidate_room(body["room"].as_str().unwrap());
                }
                let values = source.rooms.lock().iter().find(|(name,_,_)| Some(name.as_str()) == body["room"].as_str())
                    .map(|(_,_,people)| people.iter().map(|(identity,sid)| json!({"identity":identity,"sid":sid})).collect::<Vec<_>>()).unwrap_or_default();
                Json(json!({"participants":values})).into_response()
            }
        }
    )).with_state(source.clone());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let mut settings = Settings::for_tests();
    settings.livekit.api_url = format!("http://{}", listener.local_addr().unwrap());
    let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let state = Arc::new(CommsState::new(
        sqlx::PgPool::connect_lazy("postgres://postgres@127.0.0.1:1/postgres").unwrap(),
        crate::http::Outbound::for_tests(),
        settings,
        None,
        None,
        Arc::new(StubPolicy::default()),
        Arc::new(Catalogue::default()),
        Arc::new(Presence::new()),
        Arc::new(StubDirectory::default()),
    ));
    (state, source, server)
}

async fn get(app: &Router, path: &str) -> (axum::http::StatusCode, Value) {
    use tower::ServiceExt;
    let response = app
        .clone()
        .oneshot(
            axum::http::Request::get(path)
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let bytes = axum::body::to_bytes(response.into_body(), 1 << 20)
        .await
        .unwrap();
    (status, serde_json::from_slice(&bytes).unwrap())
}

async fn wait_pass(jobs: &crate::jobs::Jobs, pass: u64) {
    for _ in 0..500 {
        if jobs
            .report()
            .iter()
            .any(|r| r.name == "comms-occupancy-recovery" && r.passes >= pass)
        {
            return;
        }
        tokio::time::sleep(Duration::from_millis(2)).await;
    }
    panic!("recovery worker did not finish pass {pass}");
}

async fn elapse(duration: Duration) {
    tokio::time::pause();
    tokio::time::advance(duration).await;
    tokio::time::resume();
    tokio::task::yield_now().await;
}

#[tokio::test]
async fn real_worker_source_down_keeps_http_healthy_expires_then_recovers_and_clears_on_replica() {
    let (state, source, server) = fixture().await;
    let jobs = crate::jobs::Jobs::new().with_fleet_jobs(false);
    let app = router(state.presence.clone());
    let unknown = get(&app, "/live-data").await;
    assert_eq!(unknown.0, axum::http::StatusCode::OK);
    assert_eq!(unknown.1["lastUpdated"], "1970-01-01T00:00:00.000Z");
    assert!(
        state
            .presence
            .admission_count(&Scope::Lobby("foo.dcl.eth".into()))
            .is_none()
    );
    state.spawn_presence_recovery(&jobs);
    wait_pass(&jobs, 1).await;
    let initial = get(&app, "/live-data").await.1;
    assert_eq!(initial["data"]["totalUsers"], 1);
    assert_eq!(
        state
            .presence
            .admission_occupancy(&state, "foo.dcl.eth", false)
            .await
            .unwrap(),
        1
    );
    let same = get(&app, "/live-data").await.1;
    assert_eq!(initial["lastUpdated"], same["lastUpdated"]);

    source.fail.store(true, Ordering::SeqCst);
    elapse(Duration::from_secs(31)).await;
    wait_pass(&jobs, 2).await;
    assert!(
        state
            .presence
            .admission_occupancy(&state, "foo.dcl.eth", false)
            .await
            .is_err()
    );
    let stale = get(&app, "/live-data").await.1;
    assert_eq!(stale, initial);
    elapse(Duration::from_secs(60)).await;
    wait_pass(&jobs, 3).await;
    let (status, expired) = get(&app, "/live-data").await;
    assert_eq!(status, axum::http::StatusCode::OK);
    assert_eq!(expired["data"], json!({"totalUsers":0,"perWorld":[]}));
    assert_eq!(expired["lastUpdated"], initial["lastUpdated"]);
    assert_eq!(
        get(&app, "/wallet/watch:foo/connected-world").await.0,
        axum::http::StatusCode::NOT_FOUND
    );

    source.fail.store(false, Ordering::SeqCst);
    elapse(Duration::from_secs(7)).await;
    wait_pass(&jobs, 4).await;
    let restored = get(&app, "/live-data").await.1;
    assert_eq!(restored["data"]["totalUsers"], 1);
    assert_ne!(restored["lastUpdated"], initial["lastUpdated"]);
    source.empty.store(true, Ordering::SeqCst);
    elapse(Duration::from_secs(31)).await;
    wait_pass(&jobs, 5).await;
    let empty = get(&app, "/live-data").await.1;
    assert_eq!(empty["data"], json!({"totalUsers":0,"perWorld":[]}));
    assert_ne!(empty["lastUpdated"], restored["lastUpdated"]);
    assert_eq!(
        state
            .presence
            .admission_occupancy(&state, "foo.dcl.eth", false)
            .await
            .unwrap(),
        0
    );
    jobs.shutdown(Duration::from_secs(1)).await;
    server.abort();
}

#[tokio::test]
async fn replacement_dirty_generation_partial_and_recreated_room_preserve_previous_complete_time() {
    let (state, source, server) = fixture().await;
    state.presence.recover_complete(&state).await.unwrap();
    let original_time = state.presence.last_updated();
    state.presence.invalidate_room("world-foo.dcl.eth");
    assert!(
        state
            .presence
            .admission_count(&Scope::Lobby("foo.dcl.eth".into()))
            .is_none()
    );
    assert_eq!(state.presence.last_updated(), original_time);
    assert_eq!(state.presence.world_participant_count("foo.dcl.eth"), 1);
    source.sid_changed.store(true, Ordering::SeqCst);
    assert!(state.presence.recover_complete(&state).await.is_err());
    assert_eq!(state.presence.last_updated(), original_time);
    source.sid_changed.store(false, Ordering::SeqCst);
    source.rooms.lock()[0].2 = vec![
        ("streamer".into(), "PA_new".into()),
        ("authoritative-server".into(), "PA_server".into()),
    ];
    state.presence.recover_complete(&state).await.unwrap();
    assert_eq!(state.presence.comms_stats().users, 1);
    assert_eq!(state.presence.get_peer_world("watch:foo").unwrap(), None);
    source.rooms.lock().push((
        "world-bar.dcl.eth".into(),
        "RM_2".into(),
        vec![("bad".into(), "".into())],
    ));
    let time = state.presence.last_updated();
    assert!(state.presence.recover_complete(&state).await.is_err());
    assert_eq!(state.presence.last_updated(), time);
    assert_eq!(state.presence.comms_stats().users, 1);
    server.abort();
}

#[tokio::test]
async fn waiter_and_permit_saturation_are_bounded_failures() {
    let (state, _, server) = fixture().await;
    let waiters = state.presence.waiters.acquire_many(64).await.unwrap();
    assert!(matches!(
        state
            .presence
            .admission_occupancy(&state, "foo.dcl.eth", false)
            .await,
        Err(RecoveryFailure::Budget)
    ));
    drop(waiters);
    let permits = state.presence.reads.acquire_many(4).await.unwrap();
    let result = state
        .presence
        .admission_occupancy(&state, "foo.dcl.eth", false)
        .await;
    assert!(matches!(result, Err(RecoveryFailure::Deadline)));
    drop(permits);
    server.abort();
}

#[tokio::test]
async fn local_event_between_roster_and_commit_rejects_complete_candidate() {
    let (state, source, server) = fixture().await;
    state.presence.recover_complete(&state).await.unwrap();
    let time = state.presence.last_updated();
    *source.dirty_on_read.lock() = Some(state.presence.clone());
    assert!(matches!(
        state.presence.recover_complete(&state).await,
        Err(RecoveryFailure::Changed)
    ));
    assert_eq!(state.presence.last_updated(), time);
    assert!(
        state
            .presence
            .admission_count(&Scope::Lobby("foo.dcl.eth".into()))
            .is_none()
    );
    server.abort();
}

#[tokio::test]
async fn unrelated_known_room_dirty_does_not_invalidate_fresh_scope_and_reserved_slot_survives() {
    let (state, source, server) = fixture().await;
    source
        .rooms
        .lock()
        .push(("world-bar.dcl.eth".into(), "RM_2".into(), vec![]));
    state.presence.recover_complete(&state).await.unwrap();
    state.presence.invalidate_room("world-bar.dcl.eth");
    assert_eq!(
        state
            .presence
            .admission_count(&Scope::Lobby("foo.dcl.eth".into())),
        Some(1)
    );
    assert_eq!(
        state
            .presence
            .admission_count(&Scope::Lobby("bar.dcl.eth".into())),
        None
    );
    let permits = state.presence.reads.acquire_many(3).await.unwrap();
    assert_eq!(
        state
            .presence
            .admission_occupancy(&state, "bar.dcl.eth", false)
            .await
            .unwrap(),
        0
    );
    drop(permits);
    server.abort();
}

#[tokio::test]
async fn same_scope_refresh_is_single_flight_and_does_not_refresh_global_source_time() {
    let (state, source, server) = fixture().await;
    let results = futures::future::join_all((0..32).map(|_| {
        state
            .presence
            .admission_occupancy(&state, "foo.dcl.eth", false)
    }))
    .await;
    assert!(results.iter().all(|result| matches!(result, Ok(1))));
    assert_eq!(source.enumerate.load(Ordering::SeqCst), 2);
    assert_eq!(state.presence.last_updated(), "1970-01-01T00:00:00.000Z");
    assert!(state.presence.world_counts().is_empty());
    server.abort();
}

#[derive(Default)]
struct Catalogue {
    refusal: Option<(String, RoomRefusal)>,
    unavailable: Option<String>,
    extra: HashMap<String, crate::rooms::ResolvedRoom>,
}
#[async_trait::async_trait]
impl crate::interfaces::SceneIdentity for Catalogue {
    async fn classify_room(
        &self,
        room: &str,
    ) -> Result<RoomClassification, crate::rooms::ResolveError> {
        if self.unavailable.as_deref() == Some(room) {
            return Err(crate::rooms::ResolveError::Db(sqlx::Error::Protocol(
                "injected catalogue outage".into(),
            )));
        }
        if let Some((refused, reason)) = &self.refusal {
            if refused == room {
                return Ok(RoomClassification::RelevantRefused {
                    room: room.into(),
                    reason: *reason,
                });
            }
        }
        if let Some(resolved) = self.extra.get(room) {
            return Ok(RoomClassification::Canonical(resolved.clone()));
        }
        let shared_scene = match room {
            "scene:main:GenesisScene" => Some("GenesisScene"),
            "scene:main:EmptyScene" => Some("EmptyScene"),
            "scene:main:MissingScene" => Some("MissingScene"),
            _ => None,
        };
        if let Some(scene) = shared_scene {
            return Ok(RoomClassification::Canonical(crate::rooms::ResolvedRoom {
                realm: crate::world_identity::RealmId::Shared("main".into()),
                scene_id: Some(crate::world_identity::SceneId::parse(scene).unwrap()),
                deployment: None,
                room: room.into(),
            }));
        }
        // Explicit test catalogue: lobby existence is not inferred from syntax.
        let world = match room {
            "world-foo.dcl.eth" => Some("foo.dcl.eth"),
            "world-bar.dcl.eth" => Some("bar.dcl.eth"),
            _ => None,
        };
        if let Some(world) = world {
            return Ok(RoomClassification::Canonical(crate::rooms::ResolvedRoom {
                realm: crate::world_identity::RealmId::World(
                    crate::world_identity::WorldName::parse(world).unwrap(),
                ),
                scene_id: None,
                deployment: None,
                room: room.into(),
            }));
        }
        match room {
            "scene-foo.dcl.eth-scenea" | "scene-foo.dcl.eth-sceneb" => self
                .resolve(crate::rooms::RoomRequest::NamedWorldScene { room })
                .await
                .map(RoomClassification::Canonical),
            "island-fixture" | "voice-chat-private-fixture" => Ok(RoomClassification::Unrelated),
            _ => Ok(RoomClassification::RelevantRefused {
                room: room.into(),
                reason: RoomRefusal::Unresolved,
            }),
        }
    }

    async fn resolve(
        &self,
        request: crate::rooms::RoomRequest<'_>,
    ) -> Result<crate::rooms::ResolvedRoom, crate::rooms::ResolveError> {
        let crate::rooms::RoomRequest::NamedWorldScene { room } = request else {
            panic!("named fixture only")
        };
        let scene = match room {
            "scene-foo.dcl.eth-scenea" => "SceneA",
            "scene-foo.dcl.eth-sceneb" => "SceneB",
            _ => return Err(crate::rooms::ResolveError::UnresolvedRoom(room.into())),
        };
        Ok(crate::rooms::ResolvedRoom {
            realm: crate::world_identity::RealmId::World(
                crate::world_identity::WorldName::parse("foo.dcl.eth").unwrap(),
            ),
            scene_id: Some(crate::world_identity::SceneId::parse(scene).unwrap()),
            deployment: None,
            room: room.into(),
        })
    }
    async fn world_scene(
        &self,
        _: &crate::world_identity::WorldName,
        _: &crate::world_identity::SceneId,
    ) -> Result<Option<crate::rooms::ActiveDeployment>, crate::rooms::ResolveError> {
        Ok(None)
    }
    async fn world_active_scene(
        &self,
        _: &crate::world_identity::WorldName,
    ) -> Result<Option<crate::rooms::ActiveDeployment>, crate::rooms::ResolveError> {
        Ok(None)
    }
}

#[tokio::test]
async fn baseline_world_lobby_and_scene_counts_remain_scoped() {
    let (mut state, source, server) = fixture().await;
    Arc::get_mut(&mut state).unwrap().scenes = Arc::new(Catalogue::default());
    *source.rooms.lock() = vec![
        (
            "world-foo.dcl.eth".into(),
            "RM_lobby".into(),
            vec![
                ("wallet".into(), "PA_1".into()),
                ("WALLET".into(), "PA_2".into()),
                ("AUTHORITATIVE-SERVER".into(), "PA_3".into()),
                ("authoritative-server".into(), "PA_server".into()),
            ],
        ),
        (
            "scene-foo.dcl.eth-scenea".into(),
            "RM_a".into(),
            vec![
                ("wallet".into(), "PA_a".into()),
                ("stream-1-streamer".into(), "PA_stream".into()),
            ],
        ),
        (
            "scene-foo.dcl.eth-sceneb".into(),
            "RM_b".into(),
            vec![
                ("wallet".into(), "PA_b".into()),
                ("watch:room:1".into(), "PA_watch".into()),
            ],
        ),
    ];
    source.rooms.lock().push((
        "scene:main:GenesisScene".into(),
        "RM_shared".into(),
        vec![
            ("shared-wallet".into(), "PA_shared".into()),
            ("authoritative-server".into(), "PA_reserved".into()),
            ("authoritative-server-extra".into(), "PA_prefix".into()),
            ("AUTHORITATIVE-SERVER".into(), "PA_uppercase".into()),
        ],
    ));
    state.presence.recover_complete(&state).await.unwrap();
    assert_eq!(
        state.presence.world_counts(),
        vec![("foo.dcl.eth".into(), 3)]
    );
    assert_eq!(
        state
            .presence
            .admission_occupancy(&state, "foo.dcl.eth", false)
            .await
            .unwrap(),
        3
    );
    assert_eq!(
        state
            .presence
            .admission_occupancy(&state, "foo.dcl.eth", true)
            .await
            .unwrap(),
        4
    );
    assert_eq!(
        state
            .presence
            .requested_scene_occupancy(&state, "scene-foo.dcl.eth-scenea")
            .await
            .unwrap(),
        2
    );
    assert_eq!(
        state
            .presence
            .get_peer_world("authoritative-server")
            .unwrap(),
        None
    );
    assert_eq!(
        state.presence.get_peer_world("WALLET").unwrap(),
        Some("foo.dcl.eth".into())
    );
    let view = state.presence.complete();
    let oldest = view
        .rooms
        .iter()
        .map(|room| &room.observed)
        .min_by_key(|o| o.mono)
        .unwrap();
    assert_eq!(view.observed.as_ref().unwrap().mono, oldest.mono);
    assert_eq!(view.observed.as_ref().unwrap().utc, oldest.utc);
    assert_eq!(
        view.rooms
            .iter()
            .find(|r| r.name.ends_with("-scenea"))
            .unwrap()
            .scene
            .as_deref(),
        Some("SceneA")
    );
    assert_eq!(
        view.rooms
            .iter()
            .map(|r| r.participants.len())
            .sum::<usize>(),
        12
    );
    // Public lobby/discovery projections consume the same complete observation;
    // exact shared-scene admission remains separate from both world scene scopes.
    let app = router(state.presence.clone());
    let (_, live) = get(&app, "/live-data").await;
    assert_eq!(
        live["data"],
        json!({"totalUsers":3,"perWorld":[{"worldName":"foo.dcl.eth","users":3}]})
    );
    assert_eq!(live["lastUpdated"], state.presence.last_updated());
    assert_eq!(
        get(&app, "/wallet/WALLET/connected-world").await.0,
        axum::http::StatusCode::OK
    );
    assert_eq!(
        get(&app, "/wallet/shared-wallet/connected-world").await.0,
        axum::http::StatusCode::NOT_FOUND
    );
    assert_eq!(
        state
            .presence
            .requested_scene_occupancy(&state, "scene:main:GenesisScene")
            .await
            .unwrap(),
        3
    );
    assert_eq!(
        state
            .presence
            .requested_scene_occupancy(&state, "scene-foo.dcl.eth-sceneb")
            .await
            .unwrap(),
        2
    );
    assert_eq!(
        (
            state.presence.comms_stats().users,
            state.presence.comms_stats().rooms
        ),
        (5, 3)
    );
    server.abort();
}

#[tokio::test]
async fn every_typed_relevant_refusal_preserves_complete_snapshot_and_source_time() {
    let (mut state, source, server) = fixture().await;
    state.presence.recover_complete(&state).await.unwrap();
    let published = state.presence.complete();
    let observed = state.presence.last_updated();
    let counts = state.presence.world_counts();
    for (room, reason) in [
        ("scene:", RoomRefusal::Malformed),
        ("scene-foo.dcl.eth-unknown", RoomRefusal::Unresolved),
        ("world-foo.dcl.eth-scene", RoomRefusal::Ambiguous),
        ("world-Foo.dcl.eth", RoomRefusal::Noncanonical),
        ("world-foo.dcl.eth-scene", RoomRefusal::Legacy),
    ] {
        *source.rooms.lock() = vec![
            (
                "world-foo.dcl.eth".into(),
                "RM_1".into(),
                vec![("replacement-watch".into(), "PA_new".into())],
            ),
            (room.into(), "RM_refused".into(), vec![]),
        ];
        Arc::get_mut(&mut state).unwrap().scenes = Arc::new(Catalogue {
            refusal: Some((room.into(), reason)),
            ..Default::default()
        });
        // Relevant refusal is tested among otherwise valid occupied rooms.
        assert!(matches!(
            state.presence.recover_complete(&state).await,
            Err(RecoveryFailure::Unresolved)
        ));
        assert!(Arc::ptr_eq(&state.presence.complete(), &published));
        assert_eq!(state.presence.world_counts(), counts);
        assert_eq!(state.presence.last_updated(), observed);
        assert_eq!(
            state
                .presence
                .get_peer_world("watch:foo")
                .unwrap()
                .as_deref(),
            Some("foo.dcl.eth")
        );
        assert_eq!(
            state.presence.get_peer_world("replacement-watch").unwrap(),
            None
        );

        // A dirty scope must try authority; public retained display is never
        // substituted for a determined admission answer.
        state.presence.invalidate_room("world-foo.dcl.eth");
        assert!(matches!(
            state
                .presence
                .admission_occupancy(&state, "foo.dcl.eth", false)
                .await,
            Err(RecoveryFailure::Unresolved)
        ));
        assert!(Arc::ptr_eq(&state.presence.complete(), &published));
        assert_eq!(state.presence.last_updated(), observed);
    }
    server.abort();
}

#[tokio::test]
async fn classifier_store_error_among_valid_rooms_is_not_empty_or_successful_observation() {
    let (mut state, source, server) = fixture().await;
    state.presence.recover_complete(&state).await.unwrap();
    let published = state.presence.complete();
    let observed = state.presence.last_updated();
    source
        .rooms
        .lock()
        .push(("world-bar.dcl.eth".into(), "RM_unavailable".into(), vec![]));
    Arc::get_mut(&mut state).unwrap().scenes = Arc::new(Catalogue {
        unavailable: Some("world-bar.dcl.eth".into()),
        ..Default::default()
    });
    assert!(matches!(
        state.presence.recover_complete(&state).await,
        Err(RecoveryFailure::Source)
    ));
    assert!(Arc::ptr_eq(&state.presence.complete(), &published));
    assert_eq!(state.presence.last_updated(), observed);
    assert_eq!(state.presence.world_participant_count("foo.dcl.eth"), 1);
    state.presence.invalidate_room("world-foo.dcl.eth");
    assert!(matches!(
        state
            .presence
            .admission_occupancy(&state, "foo.dcl.eth", false)
            .await,
        Err(RecoveryFailure::Source)
    ));
    assert!(Arc::ptr_eq(&state.presence.complete(), &published));
    assert_eq!(state.presence.last_updated(), observed);
    server.abort();
}

#[tokio::test]
async fn unrelated_inventory_is_ignored_and_unrelated_only_or_empty_scan_replaces_with_real_zero() {
    let (state, source, server) = fixture().await;
    source.rooms.lock().push((
        "island-fixture".into(),
        "RM_island".into(),
        vec![("island-only".into(), "PA_island".into())],
    ));
    source.rooms.lock().push((
        "voice-chat-private-fixture".into(),
        "RM_voice".into(),
        vec![("voice-only".into(), "PA_voice".into())],
    ));
    state.presence.recover_complete(&state).await.unwrap();
    assert_eq!(state.presence.world_participant_count("foo.dcl.eth"), 1);
    assert_eq!(state.presence.comms_stats().users, 1);
    let occupied = state.presence.complete();

    tokio::time::sleep(Duration::from_millis(2)).await;
    source
        .rooms
        .lock()
        .retain(|(room, _, _)| room != "world-foo.dcl.eth");
    state.presence.recover_complete(&state).await.unwrap();
    let unrelated = state.presence.complete();
    assert!(unrelated.rooms.is_empty());
    assert!(unrelated.observed.as_ref().unwrap().mono > occupied.observed.as_ref().unwrap().mono);
    assert_eq!(
        state
            .presence
            .admission_occupancy(&state, "foo.dcl.eth", false)
            .await
            .unwrap(),
        0
    );

    tokio::time::sleep(Duration::from_millis(2)).await;
    source.empty.store(true, Ordering::SeqCst);
    state.presence.recover_complete(&state).await.unwrap();
    let empty = state.presence.complete();
    assert!(empty.rooms.is_empty());
    assert!(empty.observed.as_ref().unwrap().mono > unrelated.observed.as_ref().unwrap().mono);
    let app = router(state.presence.clone());
    let (status, body) = get(&app, "/live-data").await;
    assert_eq!(status, axum::http::StatusCode::OK);
    assert_eq!(body["data"], json!({"totalUsers":0,"perWorld":[]}));
    assert_ne!(body["lastUpdated"], "1970-01-01T00:00:00.000Z");
    server.abort();
}

#[tokio::test]
async fn not_found_reenumerates_and_reads_recreated_sid_without_fabricating_empty() {
    let (state, source, server) = fixture().await;
    source.not_found_remaining.store(1, Ordering::SeqCst);
    source.recreate_on_not_found.store(true, Ordering::SeqCst);
    state.presence.recover_complete(&state).await.unwrap();
    assert_eq!(source.enumerate.load(Ordering::SeqCst), 3);
    assert_eq!(source.roster_reads.load(Ordering::SeqCst), 2);
    let view = state.presence.complete();
    assert_eq!(view.rooms[0].sid, "RM_recreated_after_missing");
    assert_eq!(view.rooms[0].participants[0].identity, "watch:foo");
    assert_eq!(state.presence.world_participant_count("foo.dcl.eth"), 1);
    server.abort();
}

#[tokio::test]
async fn repeated_not_found_stops_after_one_retry_and_preserves_snapshot_and_time() {
    let (state, source, server) = fixture().await;
    state.presence.recover_complete(&state).await.unwrap();
    let published = state.presence.complete();
    let observed = state.presence.last_updated();
    source
        .not_found_remaining
        .store(usize::MAX, Ordering::SeqCst);
    source.roster_reads.store(0, Ordering::SeqCst);
    assert!(matches!(
        state.presence.recover_complete(&state).await,
        Err(RecoveryFailure::RoomDisappeared)
    ));
    assert_eq!(source.roster_reads.load(Ordering::SeqCst), 2);
    assert!(Arc::ptr_eq(&state.presence.complete(), &published));
    assert_eq!(state.presence.last_updated(), observed);
    state.presence.invalidate_room("world-foo.dcl.eth");
    assert!(matches!(
        state
            .presence
            .admission_occupancy(&state, "foo.dcl.eth", false)
            .await,
        Err(RecoveryFailure::RoomDisappeared)
    ));
    assert_eq!(source.roster_reads.load(Ordering::SeqCst), 4);
    assert_eq!(state.presence.last_updated(), observed);
    server.abort();
}

#[tokio::test]
async fn same_wallet_in_distinct_worlds_returns_retryable_503_then_single_world_recovers() {
    use tower::ServiceExt;
    let (state, source, server) = fixture().await;
    *source.rooms.lock() = vec![
        (
            "world-foo.dcl.eth".into(),
            "RM_foo".into(),
            vec![("0xAaAa".into(), "PA_foo".into())],
        ),
        (
            "world-bar.dcl.eth".into(),
            "RM_bar".into(),
            vec![("0xaaaa".into(), "PA_bar".into())],
        ),
    ];
    state.presence.recover_complete(&state).await.unwrap();
    let before = state.presence.complete();
    assert_eq!(state.presence.get_peer_world("0xAAAA"), Err(AmbiguousWorld));
    let app = router(state.presence.clone());
    let response = app
        .clone()
        .oneshot(
            axum::http::Request::get("/wallet/0xAAAA/connected-world")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        response.status(),
        axum::http::StatusCode::SERVICE_UNAVAILABLE
    );
    assert_eq!(response.headers()[axum::http::header::RETRY_AFTER], "1");
    let bytes = axum::body::to_bytes(response.into_body(), 1 << 16)
        .await
        .unwrap();
    let error: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(error["ok"], false);
    assert_eq!(
        error["error"],
        "Connected world could not be decided; retry shortly."
    );
    assert!(Arc::ptr_eq(&state.presence.complete(), &before));
    assert_eq!(
        state.presence.world_counts(),
        vec![("bar.dcl.eth".into(), 1), ("foo.dcl.eth".into(), 1)]
    );
    source
        .rooms
        .lock()
        .retain(|(room, _, _)| room == "world-foo.dcl.eth");
    state.presence.recover_complete(&state).await.unwrap();
    let (status, body) = get(&app, "/wallet/0xAAAA/connected-world").await;
    assert_eq!(status, axum::http::StatusCode::OK);
    assert_eq!(body, json!({"wallet":"0xAAAA","world":"foo.dcl.eth"}));
    server.abort();
}

#[tokio::test]
async fn aggregate_exact_100000_records_across_bounded_responses_publish_but_one_more_retains_old_view()
 {
    let (state, source, server) = fixture().await;
    let names = [
        "world-foo.dcl.eth",
        "world-bar.dcl.eth",
        "scene-foo.dcl.eth-scenea",
        "scene-foo.dcl.eth-sceneb",
    ];
    *source.rooms.lock() = names
        .into_iter()
        .enumerate()
        .map(|(room, name)| {
            let members = (room * 25_000..(room + 1) * 25_000)
                .map(|n| (format!("i{n}"), format!("p{n}")))
                .collect();
            (name.into(), format!("RM_{room}"), members)
        })
        .collect();
    state.presence.recover_complete(&state).await.unwrap();
    let view = state.presence.complete();
    assert_eq!(
        view.rooms
            .iter()
            .map(|r| r.participants.len())
            .sum::<usize>(),
        100_000
    );
    let observed = state.presence.last_updated();
    source.rooms.lock()[0]
        .2
        .push(("i100000".into(), "p100000".into()));
    assert!(matches!(
        state.presence.recover_complete(&state).await,
        Err(RecoveryFailure::Budget)
    ));
    assert!(Arc::ptr_eq(&state.presence.complete(), &view));
    assert_eq!(state.presence.last_updated(), observed);
    server.abort();
}

#[tokio::test]
async fn aggregate_exact_16_mebibytes_publish_but_one_more_decoded_byte_retains_old_view() {
    let (mut state, source, server) = fixture().await;
    let mut catalogue = Catalogue::default();
    let mut rooms = Vec::new();
    let mut header_bytes = 0;
    for index in 0..16 {
        let world = format!("budget{index}.dcl.eth");
        let room = format!("world-{world}");
        let sid = format!("RM_{index}");
        header_bytes += world.len() + room.len() + sid.len();
        catalogue.extra.insert(
            room.clone(),
            crate::rooms::ResolvedRoom {
                realm: crate::world_identity::RealmId::World(
                    crate::world_identity::WorldName::parse(&world).unwrap(),
                ),
                scene_id: None,
                deployment: None,
                room: room.clone(),
            },
        );
        let participants = (index * 1024..(index + 1) * 1024)
            .map(|n| {
                let identity = format!("i{n:05}");
                let sid = format!("p{n:05}");
                (
                    format!("{identity}{}", "i".repeat(512 - identity.len())),
                    format!("{sid}{}", "s".repeat(512 - sid.len())),
                )
            })
            .collect::<Vec<_>>();
        rooms.push((room, sid, participants));
    }
    // All identity/SID fields are <=512 and remain nonempty. Subtract the
    // actual room/world/SID headers so the production decoded-byte sum lands
    // exactly on 16MiB, independently of the per-response JSON byte ceiling.
    let mut remaining = header_bytes;
    for (_, _, members) in &mut rooms {
        for (identity, _) in members {
            let remove = remaining.min(identity.len() - 1);
            identity.truncate(identity.len() - remove);
            remaining -= remove;
            if remaining == 0 {
                break;
            }
        }
        if remaining == 0 {
            break;
        }
    }
    assert_eq!(remaining, 0);
    *source.rooms.lock() = rooms;
    Arc::get_mut(&mut state).unwrap().scenes = Arc::new(catalogue);
    state.presence.recover_complete(&state).await.unwrap();
    let view = state.presence.complete();
    let decoded: usize = view
        .rooms
        .iter()
        .map(|r| {
            r.name.len()
                + r.sid.len()
                + r.realm.len()
                + r.scene.as_ref().map_or(0, String::len)
                + r.participants
                    .iter()
                    .map(|p| p.identity.len() + p.sid.len())
                    .sum::<usize>()
        })
        .sum();
    assert_eq!(decoded, 16 * 1024 * 1024);
    let observed = state.presence.last_updated();
    source.rooms.lock()[0].2[0].0.push('x');
    assert!(matches!(
        state.presence.recover_complete(&state).await,
        Err(RecoveryFailure::Budget)
    ));
    assert!(Arc::ptr_eq(&state.presence.complete(), &view));
    assert_eq!(state.presence.last_updated(), observed);
    server.abort();
}

#[tokio::test]
async fn shared_exact_scene_preserves_membership_without_widening_world_or_public_stats() {
    let (state, source, server) = fixture().await;
    *source.rooms.lock() = vec![
        (
            "world-foo.dcl.eth".into(),
            "RM_world".into(),
            vec![("0xAAA".into(), "PA_world".into())],
        ),
        (
            "scene:main:GenesisScene".into(),
            "RM_shared".into(),
            vec![
                ("0xaaa".into(), "PA_shared".into()),
                ("watch:shared".into(), "PA_watch".into()),
                ("AUTHORITATIVE-SERVER".into(), "PA_case".into()),
                ("authoritative-server".into(), "PA_server".into()),
            ],
        ),
        ("scene:main:EmptyScene".into(), "RM_empty".into(), vec![]),
    ];
    state.presence.recover_complete(&state).await.unwrap();
    assert_eq!(
        state
            .presence
            .requested_scene_occupancy(&state, "scene:main:GenesisScene")
            .await
            .unwrap(),
        3
    );
    assert_eq!(
        state
            .presence
            .requested_scene_occupancy(&state, "scene:main:EmptyScene")
            .await
            .unwrap(),
        0
    );
    assert_eq!(
        state
            .presence
            .requested_scene_occupancy(&state, "scene:main:MissingScene")
            .await
            .unwrap(),
        0
    );
    assert_eq!(
        state.presence.world_counts(),
        vec![("foo.dcl.eth".into(), 1)]
    );
    assert_eq!(
        (
            state.presence.comms_stats().users,
            state.presence.comms_stats().rooms
        ),
        (1, 1)
    );
    assert_eq!(
        state.presence.get_peer_world("0xaaa").unwrap(),
        Some("foo.dcl.eth".into())
    );
    assert_eq!(state.presence.get_peer_world("watch:shared").unwrap(), None);
    assert_eq!(
        state
            .presence
            .admission_occupancy(&state, "foo.dcl.eth", false)
            .await
            .unwrap(),
        1
    );
    assert_eq!(
        state
            .presence
            .admission_occupancy(&state, "foo.dcl.eth", true)
            .await
            .unwrap(),
        0
    );
    let app = router(state.presence.clone());
    assert_eq!(
        get(&app, "/live-data").await.1["data"],
        json!({"totalUsers":1,"perWorld":[{"worldName":"foo.dcl.eth","users":1}]})
    );
    assert_eq!(
        get(&app, "/wallet/0xaaa/connected-world").await.0,
        axum::http::StatusCode::OK
    );
    let shared = state.presence.complete();
    let room = shared
        .rooms
        .iter()
        .find(|room| room.name == "scene:main:GenesisScene")
        .unwrap();
    assert!(!room.is_world);
    assert_eq!(room.realm, "main");
    assert_eq!(room.scene.as_deref(), Some("GenesisScene"));
    assert_eq!(room.participants.len(), 4);
    for (members, expected) in [
        (vec![("authoritative-server".into(), "PA_server".into())], 0),
        (vec![], 0),
        (vec![("watch:shared".into(), "PA_watch".into())], 1),
    ] {
        source.rooms.lock()[1].2 = members;
        state.presence.recover_complete(&state).await.unwrap();
        assert_eq!(
            state
                .presence
                .requested_scene_occupancy(&state, "scene:main:GenesisScene")
                .await
                .unwrap(),
            expected
        );
        assert_eq!(
            state.presence.world_counts(),
            vec![("foo.dcl.eth".into(), 1)]
        );
        assert_eq!(
            (
                state.presence.comms_stats().users,
                state.presence.comms_stats().rooms
            ),
            (1, 1)
        );
    }
    server.abort();
}

#[tokio::test]
async fn exact_scene_rejects_unsupported_or_unavailable_scope_even_with_fresh_complete_view() {
    let (mut state, source, server) = fixture().await;
    source.rooms.lock().push((
        "scene:main:GenesisScene".into(),
        "RM_shared".into(),
        vec![("watch:shared".into(), "PA_shared".into())],
    ));
    state.presence.recover_complete(&state).await.unwrap();
    let published = state.presence.complete();
    let observed = state.presence.last_updated();
    for name in [
        "world-foo.dcl.eth",
        "island-fixture",
        "voice-chat-private-fixture",
        "scene:main:UnknownScene",
        "scene:preview:PreviewScene",
        "scene:",
    ] {
        assert!(
            matches!(
                state.presence.requested_scene_occupancy(&state, name).await,
                Err(RecoveryFailure::Unresolved)
            ),
            "unsupported scope {name} must not become cached zero"
        );
    }
    Arc::get_mut(&mut state).unwrap().scenes = Arc::new(Catalogue {
        unavailable: Some("scene:main:GenesisScene".into()),
        ..Default::default()
    });
    assert!(matches!(
        state
            .presence
            .requested_scene_occupancy(&state, "scene:main:GenesisScene")
            .await,
        Err(RecoveryFailure::Source)
    ));
    assert!(Arc::ptr_eq(&state.presence.complete(), &published));
    assert_eq!(state.presence.last_updated(), observed);
    assert_eq!(state.presence.comms_stats().users, 1);
    server.abort();
}
