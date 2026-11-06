use super::*;
use crate::interfaces::{
    SceneIdentity,
    test_support::{StubDirectory, StubPolicy},
};
use crate::rooms::{ActiveDeployment, ReplacedScene, ResolveError, ResolvedRoom, RoomRequest};
use crate::world_identity::SceneId;
use std::sync::{Arc, Mutex};

struct RetainedScenes {
    unavailable: bool,
}

#[async_trait::async_trait]
impl SceneIdentity for RetainedScenes {
    async fn classify_room(
        &self,
        room: &str,
    ) -> Result<crate::rooms::RoomClassification, ResolveError> {
        crate::interfaces::test_support::classify_fixture(self, room).await
    }
    async fn resolve(&self, request: RoomRequest<'_>) -> Result<ResolvedRoom, ResolveError> {
        let RoomRequest::NamedWorldScene { room } = request else {
            panic!("unexpected request")
        };
        Err(ResolveError::UnresolvedRoom(room.into()))
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
    async fn replaced_world_scene(
        &self,
        world: &WorldName,
        scene: &SceneId,
    ) -> Result<Option<ReplacedScene>, ResolveError> {
        if self.unavailable {
            return Err(ResolveError::Db(sqlx::Error::PoolClosed));
        }
        Ok(
            (world.key() == "retained.dcl.eth" && scene.as_str() == "OldHash").then(|| {
                ReplacedScene {
                    base: Parcel::parse("1,2").unwrap(),
                    successors: vec![Parcel::parse("3,4").unwrap()],
                }
            }),
        )
    }
}

#[tokio::test]
async fn retired_room_repair_reads_current_bans_and_preserves_roles_and_unrelated_rooms() {
    let Some(pool) = crate::db::scratch::pool(4).await else {
        return;
    };
    let mut state = CommsState::new(
        pool.clone(),
        crate::http::Outbound::for_tests(),
        crate::gatekeeper::Settings::for_tests(),
        None,
        None,
        Arc::new(StubPolicy::default()),
        Arc::new(RetainedScenes { unavailable: false }),
        Arc::new(crate::presence::Presence::new()),
        Arc::new(StubDirectory::default()),
    );
    let room = crate::rooms::worlds_service_scene_room(
        &WorldName::parse("retained.dcl.eth").unwrap(),
        &SceneId::parse("OldHash").unwrap(),
    );
    let other_world = crate::rooms::worlds_service_scene_room(
        &WorldName::parse("other.dcl.eth").unwrap(),
        &SceneId::parse("OldHash").unwrap(),
    );
    let other_scene = crate::rooms::worlds_service_scene_room(
        &WorldName::parse("retained.dcl.eth").unwrap(),
        &SceneId::parse("OtherHash").unwrap(),
    );
    let metadata = Arc::new(Mutex::new(
        json!({"bannedAddresses":["unbanned"],"sceneAdmins":["existing"],"presenters":["presenter"]}),
    ));
    let writes = Arc::new(Mutex::new(0));
    let captured = metadata.clone();
    let counted = writes.clone();
    let listed_room = room.clone();
    let api = axum::Router::new().route("/twirp/livekit.RoomService/{method}", axum::routing::post(
        move |axum::extract::Path(method): axum::extract::Path<String>, axum::Json(body): axum::Json<Value>| {
            let metadata = captured.clone(); let writes = counted.clone(); let room = listed_room.clone();
            async move {
                if method == "ListRooms" {
                    axum::Json(json!({"rooms":[{"name":room,"sid":"RM_retired","metadata":metadata.lock().unwrap().to_string()}]}))
                } else {
                    assert_eq!(method, "UpdateRoomMetadata");
                    *metadata.lock().unwrap() = serde_json::from_str(body["metadata"].as_str().unwrap()).unwrap();
                    *writes.lock().unwrap() += 1;
                    axum::Json(json!({}))
                }
            }
        }
    ));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    state.livekit.api_url = format!("http://{}", listener.local_addr().unwrap());
    state.livekit.configured = true;
    let server = tokio::spawn(async move { axum::serve(listener, api).await.unwrap() });
    // The unban already committed while its immediate room inventory failed.
    refresh(&state).await.unwrap();
    assert_eq!(metadata.lock().unwrap()["bannedAddresses"], json!([]));
    for (key, address) in [
        ("retained.dcl.eth:1,2", "rebanned"),
        ("retained.dcl.eth:3,4", "successor"),
        ("OldHash", "legacy"),
        ("other.dcl.eth:1,2", "wrong-world"),
        ("retained.dcl.eth:8,9", "wrong-scene"),
    ] {
        sqlx::query("INSERT INTO social.scene_bans (place_id, banned_address, banned_by) VALUES ($1,$2,'moderator')").bind(key).bind(address).execute(&pool).await.unwrap();
    }
    refresh(&state).await.unwrap();
    assert_eq!(
        *metadata.lock().unwrap(),
        json!({"bannedAddresses":["legacy","rebanned","successor"],"sceneAdmins":["existing"],"presenters":["presenter"]})
    );
    for unrelated in [
        &other_world,
        &other_scene,
        "world-retained.dcl.eth",
        "scene:main:OldHash",
    ] {
        assert!(
            retired_room_ban_fields(&state, unrelated)
                .await
                .unwrap()
                .is_none()
        );
    }
    state.scenes = Arc::new(RetainedScenes { unavailable: true });
    refresh(&state).await.unwrap();
    assert_eq!(
        *writes.lock().unwrap(),
        2,
        "unavailable authority retains metadata"
    );
    server.abort();
}
