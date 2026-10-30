//! The live presence's two routes as `serve` mounts them
//! (`serve::live_presence_routes`): comms' router at the root and under
//! `/worlds-content-server`, each behind the worlds writes gate, as the
//! worlds core routers that served them before were.

use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use serde_json::json;
use tower::ServiceExt;

use crate::comms::presence::Presence;
use crate::serve::live_presence_routes;

/// With `WORLDS_WRITES_ENABLED` off, a GET at either mount reads the map and
/// a POST answers the gate's 503 envelope with its `Retry-After`, not
/// axum's 405.
#[tokio::test]
async fn the_presence_routes_answer_at_both_mounts_behind_the_worlds_writes_gate() {
    let presence = Arc::new(Presence::new());
    presence.peer_joined("0xaaa", "foo.dcl.eth", "world-foo.dcl.eth", false);
    let app = live_presence_routes(presence, false);
    let call = |method: &str, uri: String| {
        let req = Request::builder()
            .method(method)
            .uri(uri)
            .body(Body::empty())
            .unwrap();
        app.clone().oneshot(req)
    };

    for prefix in ["", "/worlds-content-server"] {
        let resp = call("GET", format!("{prefix}/wallet/0xaaa/connected-world"))
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK, "{prefix}");
        let resp = call("GET", format!("{prefix}/live-data")).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK, "{prefix}");

        let resp = call("POST", format!("{prefix}/live-data")).await.unwrap();
        assert_eq!(
            (
                resp.status(),
                resp.headers()
                    .get(header::RETRY_AFTER)
                    .and_then(|v| v.to_str().ok())
            ),
            (StatusCode::SERVICE_UNAVAILABLE, Some("3600")),
            "{prefix}"
        );
        let bytes = axum::body::to_bytes(resp.into_body(), 1 << 16)
            .await
            .unwrap();
        let body: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(
            (&body["ok"], &body["error"]),
            (&json!(false), &json!("Service Unavailable")),
            "{prefix}: {body}"
        );
    }
}

use crate::comms::gatekeeper::{CommsState, Settings};
use crate::comms::interfaces::{
    SceneIdentity,
    test_support::{StubDirectory, StubPolicy},
};
use crate::comms::rooms::{
    ActiveDeployment, ResolveError, ResolvedRoom, RoomClassification, RoomRefusal, RoomRequest,
};
use crate::comms::world_identity::{RealmId, SceneId, WorldName};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

struct MountedCatalogue;
#[async_trait::async_trait]
impl SceneIdentity for MountedCatalogue {
    async fn classify_room(&self, room: &str) -> Result<RoomClassification, ResolveError> {
        let world = match room {
            "world-foo.dcl.eth" => "foo.dcl.eth",
            "world-bar.dcl.eth" => "bar.dcl.eth",
            _ => {
                return Ok(RoomClassification::RelevantRefused {
                    room: room.into(),
                    reason: RoomRefusal::Unresolved,
                });
            }
        };
        Ok(RoomClassification::Canonical(ResolvedRoom {
            realm: RealmId::World(WorldName::parse(world).unwrap()),
            scene_id: None,
            deployment: None,
            room: room.into(),
        }))
    }
    async fn resolve(&self, _: RoomRequest<'_>) -> Result<ResolvedRoom, ResolveError> {
        Err(ResolveError::UnresolvedRoom(
            "unused fixture resolution".into(),
        ))
    }
    async fn world_scene(
        &self,
        _: &WorldName,
        _: &SceneId,
    ) -> Result<Option<ActiveDeployment>, ResolveError> {
        Ok(None)
    }
    async fn world_active_scene(
        &self,
        _: &WorldName,
    ) -> Result<Option<ActiveDeployment>, ResolveError> {
        Ok(None)
    }
}

async fn mounted_worker_fixture() -> (
    Arc<CommsState>,
    Arc<AtomicUsize>,
    tokio::task::JoinHandle<()>,
) {
    use axum::{
        Json, Router,
        extract::{OriginalUri, State},
        response::IntoResponse,
        routing::post,
    };
    let mode = Arc::new(AtomicUsize::new(0)); // occupied, source down, empty, ambiguous
    let app = Router::new()
        .route(
            "/twirp/livekit.RoomService/{method}",
            post(
                |State(mode): State<Arc<AtomicUsize>>,
                 OriginalUri(uri): OriginalUri,
                 Json(body): Json<serde_json::Value>| async move {
                    match mode.load(Ordering::SeqCst) {
                        1 => StatusCode::SERVICE_UNAVAILABLE.into_response(),
                        current => {
                            let value = if uri.path().ends_with("ListRooms") {
                                let rooms = match current {
                                    2 => vec![],
                                    3 => vec![
                                        json!({"name":"world-foo.dcl.eth","sid":"RM_foo"}),
                                        json!({"name":"world-bar.dcl.eth","sid":"RM_bar"}),
                                    ],
                                    _ => vec![json!({"name":"world-foo.dcl.eth","sid":"RM_foo"})],
                                };
                                json!({"rooms":rooms})
                            } else if body["room"] == "world-bar.dcl.eth" {
                                json!({"participants":[{"identity":"0xABC","sid":"PA_bar"}]})
                            } else {
                                json!({"participants":[{"identity":"0xabc","sid":"PA_foo"}]})
                            };
                            Json(value).into_response()
                        }
                    }
                },
            ),
        )
        .with_state(mode.clone());
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
        Arc::new(MountedCatalogue),
        Arc::new(Presence::new()),
        Arc::new(StubDirectory::default()),
    ));
    (state, mode, server)
}

async fn mounted_read(
    app: &axum::Router,
    path: &str,
) -> (StatusCode, axum::http::HeaderMap, serde_json::Value) {
    let response = app
        .clone()
        .oneshot(Request::get(path).body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let headers = response.headers().clone();
    let bytes = axum::body::to_bytes(response.into_body(), 1 << 20)
        .await
        .unwrap();
    (status, headers, serde_json::from_slice(&bytes).unwrap())
}

async fn wait_recovery_pass(jobs: &crate::jobs::Jobs, pass: u64) {
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
    panic!("mounted recovery worker did not complete pass {pass}");
}

async fn advance_worker_clock(duration: Duration) {
    tokio::time::pause();
    tokio::time::advance(duration).await;
    tokio::time::resume();
    tokio::task::yield_now().await;
}

#[tokio::test]
async fn both_public_mounts_keep_real_worker_source_time_through_outage_expiry_recovery_and_empty()
{
    let (state, mode, server) = mounted_worker_fixture().await;
    let jobs = crate::jobs::Jobs::new().with_fleet_jobs(false);
    let app = live_presence_routes(state.presence.clone(), false);
    for prefix in ["", "/worlds-content-server"] {
        let (status, _, body) = mounted_read(&app, &format!("{prefix}/live-data")).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            body,
            json!({"data":{"totalUsers":0,"perWorld":[]},"lastUpdated":"1970-01-01T00:00:00.000Z"})
        );
        assert_eq!(
            mounted_read(&app, &format!("{prefix}/wallet/0xabc/connected-world"))
                .await
                .0,
            StatusCode::NOT_FOUND
        );
    }
    state.spawn_presence_recovery(&jobs);
    wait_recovery_pass(&jobs, 1).await;
    let initial = mounted_read(&app, "/live-data").await.2;
    assert_eq!(
        initial["data"],
        json!({"totalUsers":1,"perWorld":[{"worldName":"foo.dcl.eth","users":1}]})
    );
    assert_ne!(initial["lastUpdated"], "1970-01-01T00:00:00.000Z");
    for prefix in ["", "/worlds-content-server"] {
        assert_eq!(
            mounted_read(&app, &format!("{prefix}/live-data")).await.2,
            initial
        );
        let (status, _, connected) =
            mounted_read(&app, &format!("{prefix}/wallet/0xABC/connected-world")).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(connected, json!({"wallet":"0xABC","world":"foo.dcl.eth"}));
    }
    mode.store(1, Ordering::SeqCst);
    advance_worker_clock(Duration::from_secs(31)).await;
    wait_recovery_pass(&jobs, 2).await;
    assert!(
        state
            .presence
            .admission_occupancy(&state, "foo.dcl.eth", false)
            .await
            .is_err()
    );
    for prefix in ["", "/worlds-content-server"] {
        let (status, _, body) = mounted_read(&app, &format!("{prefix}/live-data")).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body, initial);
    }
    advance_worker_clock(Duration::from_secs(60)).await;
    wait_recovery_pass(&jobs, 3).await;
    for prefix in ["", "/worlds-content-server"] {
        let (status, _, body) = mounted_read(&app, &format!("{prefix}/live-data")).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["data"], json!({"totalUsers":0,"perWorld":[]}));
        assert_eq!(body["lastUpdated"], initial["lastUpdated"]);
        let (status, _, absent) =
            mounted_read(&app, &format!("{prefix}/wallet/0xabc/connected-world")).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(
            absent,
            json!({"ok":false,"error":"Wallet 0xabc is not connected to any world","message":"Wallet 0xabc is not connected to any world"})
        );
    }
    mode.store(0, Ordering::SeqCst);
    advance_worker_clock(Duration::from_secs(7)).await;
    wait_recovery_pass(&jobs, 4).await;
    let restored = mounted_read(&app, "/live-data").await.2;
    assert_eq!(restored["data"], initial["data"]);
    assert_ne!(restored["lastUpdated"], initial["lastUpdated"]);
    mode.store(2, Ordering::SeqCst);
    advance_worker_clock(Duration::from_secs(31)).await;
    wait_recovery_pass(&jobs, 5).await;
    for prefix in ["", "/worlds-content-server"] {
        let (status, _, body) = mounted_read(&app, &format!("{prefix}/live-data")).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["data"], json!({"totalUsers":0,"perWorld":[]}));
        assert_ne!(body["lastUpdated"], restored["lastUpdated"]);
    }
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
async fn both_mounts_return_retryable_503_for_recovered_multiworld_wallet_without_changing_membership()
 {
    let (state, mode, server) = mounted_worker_fixture().await;
    mode.store(3, Ordering::SeqCst);
    let jobs = crate::jobs::Jobs::new().with_fleet_jobs(false);
    let app = live_presence_routes(state.presence.clone(), false);
    state.spawn_presence_recovery(&jobs);
    wait_recovery_pass(&jobs, 1).await;
    for prefix in ["", "/worlds-content-server"] {
        let (status, headers, body) =
            mounted_read(&app, &format!("{prefix}/wallet/0xabc/connected-world")).await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(headers[header::RETRY_AFTER], "1");
        assert_eq!(
            body,
            json!({"ok":false,"error":"Connected world could not be decided; retry shortly.","message":"Connected world could not be decided; retry shortly."})
        );
        assert_eq!(
            mounted_read(&app, &format!("{prefix}/live-data")).await.2["data"],
            json!({"totalUsers":2,"perWorld":[{"worldName":"bar.dcl.eth","users":1},{"worldName":"foo.dcl.eth","users":1}]})
        );
    }
    jobs.shutdown(Duration::from_secs(1)).await;
    server.abort();
}
