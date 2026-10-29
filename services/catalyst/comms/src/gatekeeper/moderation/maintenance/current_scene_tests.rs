use super::*;
use crate::interfaces::{
    SceneIdentity,
    test_support::{StubDirectory, StubPolicy},
};
use crate::rooms::{ActiveDeployment, ResolveError, ResolvedRoom, RoomRequest};
use crate::world_identity::{RealmId, SceneId, SceneNamespace};
use std::sync::{Arc, Mutex};

#[derive(Clone, Copy)]
enum Placement {
    Unpublished,
    Replaced,
    Unavailable,
}

struct HistoricalScenes {
    placement: Placement,
    calls: Mutex<Vec<&'static str>>,
}

fn historical(scene: &str) -> ResolvedRoom {
    let realm = RealmId::Shared("main".into());
    let base = Parcel::parse("5,6").unwrap();
    let scene = SceneId::parse(scene).unwrap();
    ResolvedRoom {
        room: crate::rooms::gatekeeper_scene_room("main", scene.as_str()),
        scene_id: Some(scene.clone()),
        deployment: Some(ActiveDeployment {
            scene_id: scene,
            title: None,
            parcels: [base].into_iter().collect(),
            base,
            namespace: SceneNamespace::new(&realm, base, None),
            local: true,
        }),
        realm,
    }
}

#[async_trait::async_trait]
impl SceneIdentity for HistoricalScenes {
    async fn classify_room(
        &self,
        room: &str,
    ) -> Result<crate::rooms::RoomClassification, ResolveError> {
        crate::interfaces::test_support::classify_fixture(self, room).await
    }

    async fn resolve(&self, request: RoomRequest<'_>) -> Result<ResolvedRoom, ResolveError> {
        match request {
            RoomRequest::Adapter { .. } => {
                self.calls.lock().unwrap().push("historical");
                // Local unpublish keeps this deployment's deleter NULL. The
                // Adapter contract intentionally still returns its old footprint.
                Ok(historical("UnpublishedHash"))
            }
            RoomRequest::Parcel { realm, parcel } => {
                self.calls.lock().unwrap().push("current");
                match self.placement {
                    Placement::Unpublished => Err(ResolveError::NoSceneAtParcel {
                        realm: realm.to_string(),
                        parcel: parcel.to_string(),
                    }),
                    Placement::Replaced => Ok(historical("ReplacementHash")),
                    Placement::Unavailable => Err(ResolveError::Db(sqlx::Error::PoolClosed)),
                }
            }
            _ => {
                panic!("metadata must use the existing historical and current-placement boundaries")
            }
        }
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

async fn state(placement: Placement) -> (Arc<CommsState>, Arc<HistoricalScenes>) {
    let pool = sqlx::PgPool::connect_lazy("postgres://postgres@127.0.0.1:1/postgres").unwrap();
    pool.close().await;
    let scenes = Arc::new(HistoricalScenes {
        placement,
        calls: Mutex::new(vec![]),
    });
    let mut state = CommsState::new(
        pool,
        crate::http::Outbound::for_tests(),
        crate::gatekeeper::Settings::for_tests(),
        None,
        None,
        Arc::new(StubPolicy::default()),
        scenes.clone(),
        Arc::new(crate::presence::Presence::new()),
        Arc::new(StubDirectory::default()),
    );
    state.livekit.configured = true;
    (Arc::new(state), scenes)
}

async fn started(state: Arc<CommsState>, id: &str) -> Result<axum::response::Response, ApiError> {
    let raw = serde_json::to_vec(&json!({
        "id":id,"createdAt":1718900000,"event":"room_started",
        "room":{"name":"scene:main:UnpublishedHash","sid":"RM_surviving"}
    }))
    .unwrap();
    let jwt = crate::gatekeeper::webhook::test_support::webhook_jwt(
        &raw,
        &state.livekit.api_key,
        &state.livekit.api_secret,
    );
    let mut headers = axum::http::HeaderMap::new();
    headers.insert("authorization", jwt.parse().unwrap());
    crate::gatekeeper::webhook::livekit_webhook(
        axum::extract::State(state),
        headers,
        axum::body::Bytes::from(raw),
    )
    .await
}

#[tokio::test]
async fn unpublished_or_replaced_genesis_metadata_abstains_on_initial_and_periodic_refresh() {
    for placement in [Placement::Unpublished, Placement::Replaced] {
        let (mut state, scenes) = state(placement).await;
        let requests = Arc::new(Mutex::new(Vec::<String>::new()));
        let capture = requests.clone();
        let api = axum::Router::new().route(
            "/twirp/livekit.RoomService/{method}",
            axum::routing::post(
                move |axum::extract::Path(method): axum::extract::Path<String>| {
                    let capture = capture.clone();
                    async move {
                        capture.lock().unwrap().push(method);
                        axum::Json(json!({"rooms":[{"name":"scene:main:UnpublishedHash"}]}))
                    }
                },
            ),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        Arc::get_mut(&mut state).unwrap().livekit.api_url =
            format!("http://{}", listener.local_addr().unwrap());
        let server = tokio::spawn(async move { axum::serve(listener, api).await.unwrap() });
        for id in ["initial", "same-incarnation-again"] {
            assert_eq!(
                started(state.clone(), id).await.unwrap().status(),
                axum::http::StatusCode::OK
            );
        }
        assert_eq!(
            scenes.calls.lock().unwrap().as_slice(),
            ["historical", "current", "historical", "current"],
            "abstention must not install a successful SID cooldown"
        );
        refresh(&state).await.unwrap();
        assert_eq!(scenes.calls.lock().unwrap().len(), 6);
        assert_eq!(
            requests.lock().unwrap().as_slice(),
            ["ListRooms"],
            "historical or replacement footprints must never reach role reads or metadata writes"
        );
        server.abort();
    }
}

#[tokio::test]
async fn current_genesis_placement_failure_remains_retryable_before_metadata_acknowledgement() {
    let (state, scenes) = state(Placement::Unavailable).await;
    for _ in 0..2 {
        assert_eq!(started(state.clone(), "retry").await.unwrap_err().code, 503);
    }
    assert_eq!(
        scenes.calls.lock().unwrap().len(),
        4,
        "the same signed event must retry current authority"
    );
}
