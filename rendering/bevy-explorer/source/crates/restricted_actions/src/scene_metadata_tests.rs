use std::time::{Duration, Instant};

use bevy::asset::io::{
    memory::{Dir, MemoryAssetReader},
    AssetSource, AssetSourceId,
};
use common::{rpc::RpcResultReceiver, structs::ChainLink};
use ipfs::{ContentMap, IpfsModifier, IpfsResource};
use scene_runner::initialize_scene::SceneEntityDefinitionHandle;

use super::*;

const SCENE: &str = "eyJwcmV2aWV3IjoxfQ==";
const CONTENTS: &str = "https://content.example.test/contents/";

fn metadata() -> Value {
    json!({
        "main": "scene.js",
        "scene": {"base": "-7,13", "parcels": ["-6,13", "-7,13"]},
        "display": {"title": "Retained scene"}
    })
}

fn fixture() -> (App, Arc<IpfsIo>, Entity) {
    let ipfs = Arc::new(IpfsIo::new(
        false,
        Box::new(MemoryAssetReader {
            root: Dir::default(),
        }),
        None,
        Default::default(),
        1,
        tokio::sync::mpsc::unbounded_channel().0,
    ));
    let mut wallet = Wallet::default();
    wallet.finalize_as_guest_with_seed([17; 32]);
    let mut app = App::new();
    app.register_asset_source(
        AssetSourceId::Default,
        AssetSource::build().with_reader(|| {
            Box::new(MemoryAssetReader {
                root: Dir::default(),
            })
        }),
    )
    .add_plugins((MinimalPlugins, AssetPlugin::default()))
    .init_asset::<EntityDefinition>()
    .add_event::<RpcCallEvent>()
    .add_event::<SystemApi>()
    .insert_resource(wallet)
    .insert_resource(IpfsResource {
        inner: ipfs.clone(),
    })
    .insert_resource(CurrentRealm {
        about_url: "https://realm.example.test/about".to_owned(),
        ..Default::default()
    })
    .init_resource::<comms::global_crdt::SceneRealms>()
    .init_resource::<comms::ServerSceneRooms>()
    .init_resource::<PreviewMode>()
    .init_resource::<LiveScenes>()
    .insert_resource(CurrentUserProfile {
        profile: Some(UserProfile {
            content: SerializedProfile {
                has_connected_web3: Some(true),
                ..Default::default()
            },
            ..Default::default()
        }),
        ..Default::default()
    })
    .add_systems(Update, (handle_sign_request, handle_entity_definition));

    let definition = app
        .world_mut()
        .resource_mut::<Assets<EntityDefinition>>()
        .add(EntityDefinition {
            id: SCENE.to_owned(),
            content: ContentMap::new_single("scene.js".to_owned(), "owned-script".to_owned()),
            metadata: Some(metadata()),
            ..Default::default()
        });
    let scene = app
        .world_mut()
        .spawn(SceneEntityDefinitionHandle(definition))
        .id();
    app.world_mut()
        .resource_mut::<LiveScenes>()
        .scenes
        .insert(SCENE.to_owned(), scene);
    app.finish();
    app.cleanup();
    (app, ipfs, scene)
}

fn reply<T>(app: &mut App, mut receiver: RpcResultReceiver<T>) -> T {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        app.update();
        match receiver.try_recv() {
            Ok(value) => return value,
            Err(tokio::sync::oneshot::error::TryRecvError::Empty) => {}
            Err(error) => panic!("RPC sender dropped before replying: {error}"),
        }
        assert!(Instant::now() < deadline, "RPC did not complete");
        std::thread::yield_now();
    }
}

fn entity_request(app: &mut App, hash: &str) -> Option<EntityDefinitionResponse> {
    let (response, receiver) = RpcResultSender::channel();
    app.world_mut()
        .send_event(RpcCallEvent::engine(RpcCall::EntityDefinition {
            urn: hash.to_owned(),
            response,
        }));
    reply(app, receiver)
}

#[test]
fn live_scene_signing_keeps_its_base_parcel_without_registry_metadata() {
    let (mut app, _ipfs, scene) = fixture();
    let (response, receiver) = RpcResultSender::channel();
    app.world_mut().send_event(RpcCallEvent {
        origin: RpcOrigin::Scene {
            entity: scene,
            hash: SCENE.into(),
        },
        call: RpcCall::SignRequest(common::rpc::SignRequest {
            method: "GET".to_owned(),
            uri: "https://service.example.test/API/Mixed".to_owned(),
            body_hash: None,
            response,
        }),
    });

    let headers: HashMap<_, _> = reply(&mut app, receiver).unwrap().into_iter().collect();
    let signed_metadata: Value = serde_json::from_str(&headers["x-identity-metadata"]).unwrap();
    assert_eq!(signed_metadata["sceneId"], SCENE);
    assert_eq!(signed_metadata["parcel"], "-7,13");
    let signed_payload: ChainLink =
        serde_json::from_str(&headers["x-identity-auth-chain-1"]).unwrap();
    assert_eq!(
        signed_payload.payload,
        format!(
            "get:/api/mixed:{}:{}",
            headers["x-identity-timestamp"], headers["x-identity-metadata"]
        )
    );
}

#[test]
fn live_scene_info_uses_its_owner_when_registry_metadata_is_unavailable() {
    let (mut app, ipfs, _) = fixture();
    ipfs.add_collection(
        SCENE.to_owned(),
        ContentMap::from_content([
            ("scene.js".to_owned(), "owned-script".to_owned()),
            ("imported/model.glb".to_owned(), "imported-model".to_owned()),
        ]),
        Some(IpfsModifier {
            base_url: Some(CONTENTS.to_owned()),
        }),
        None,
    );

    let response = entity_request(&mut app, SCENE).unwrap();
    assert_eq!(response.collection.len(), 2);
    assert_eq!(response.collection["scene.js"], "owned-script");
    assert_eq!(response.collection["imported/model.glb"], "imported-model");
    assert_eq!(
        serde_json::from_str::<Value>(&response.metadata.unwrap()).unwrap(),
        metadata()
    );
    assert_eq!(response.base_url, CONTENTS);
}

#[test]
fn non_live_scene_info_uses_the_registry_and_unknown_hashes_stay_absent() {
    let (mut app, ipfs, _) = fixture();
    app.world_mut().resource_mut::<LiveScenes>().scenes.clear();
    ipfs.add_collection(
        SCENE.to_owned(),
        ContentMap::new_single("registry.js".to_owned(), "registry-script".to_owned()),
        Some(IpfsModifier {
            base_url: Some(CONTENTS.to_owned()),
        }),
        Some(json!({"registry": true}).to_string()),
    );

    let response = entity_request(&mut app, SCENE).unwrap();
    assert_eq!(response.collection.len(), 1);
    assert_eq!(response.collection["registry.js"], "registry-script");
    assert_eq!(
        serde_json::from_str::<Value>(&response.metadata.unwrap()).unwrap(),
        json!({"registry": true})
    );
    assert_eq!(response.base_url, CONTENTS);
    assert!(entity_request(&mut app, "unknown-scene").is_none());
}
