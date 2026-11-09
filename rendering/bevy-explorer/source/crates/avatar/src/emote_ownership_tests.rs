use super::*;
use std::{
    sync::{Arc, Mutex, Weak},
    time::Instant,
};

use bevy::asset::{
    io::{
        memory::{Dir, MemoryAssetReader},
        AssetSource, AssetSourceId, Reader,
    },
    AssetLoader, LoadContext, StrongHandle,
};
use bevy::ecs::system::RunSystemOnce;
use collectibles::{
    base_bodyshapes,
    wearables::{Wearable, WearableCategory},
    Collectibles,
};
use common::structs::SceneDrivenAnimationRequest;
use ipfs::{IpfsIo, IpfsResource};
use tokio::sync::oneshot;

fn emote(assets: &mut Assets<Collectible<Emote>>) -> Handle<Collectible<Emote>> {
    assets.add(Collectible {
        representations: Default::default(),
        data: CollectibleData {
            hash: String::new(),
            urn: String::new(),
            thumbnail: String::new(),
            available_representations: Default::default(),
            name: String::new(),
            description: String::new(),
            extra_data: EmoteExtraData::default(),
        },
    })
}

#[test]
fn graph_nodes_own_full_and_masked_emotes_independently_until_replaced() {
    let mut assets = Assets::default();
    let mut graphs = Assets::<AnimationGraph>::default();
    let graph = graphs.add(AnimationGraph::new());
    let other_graph = graphs.add(AnimationGraph::new());
    let handle = emote(&mut assets);
    let Handle::Strong(strong) = &handle else {
        panic!()
    };
    let weak = std::sync::Arc::downgrade(strong);
    let mut owner = EmoteGraphAssets::default();
    let full = AnimationNodeIndex::new(1);
    let masked = AnimationNodeIndex::new(2);
    owner.bind(graph.id(), full, handle.clone());
    owner.bind(graph.id(), masked, handle.clone());
    drop(handle);
    owner.bind(graph.id(), full, emote(&mut assets));
    assert!(
        weak.upgrade().is_some(),
        "masked node still owns the same emote"
    );
    // A visual rebuild that preserves the graph does not discard either node's lease.
    owner.bind(graph.id(), full, emote(&mut assets));
    assert!(weak.upgrade().is_some());
    owner.bind(other_graph.id(), full, emote(&mut assets));
    assert!(
        weak.upgrade().is_none(),
        "replacing the graph drops the old graph's leases"
    );
}

#[test]
fn pending_request_reuses_its_handle_and_releases_on_replacement_and_despawn() {
    let assets = Assets::<Gltf>::default();
    let first = assets.reserve_handle();
    let Handle::Strong(strong) = &first else {
        panic!()
    };
    let weak = std::sync::Arc::downgrade(strong);
    let urn = EmoteUrn::new("wave").unwrap();
    let mut pending = PendingEmoteGltf::default();
    pending.load(&urn, || first.clone());
    drop(first);
    pending.load(&urn, || panic!("same request must not restart the load"));
    assert!(weak.upgrade().is_some());
    pending.load(&EmoteUrn::new("dance").unwrap(), || assets.reserve_handle());
    assert!(weak.upgrade().is_none());

    let handle = pending.0.as_ref().unwrap().1.clone();
    let Handle::Strong(strong) = &handle else {
        panic!()
    };
    let weak = std::sync::Arc::downgrade(strong);
    drop(handle);
    let mut world = World::new();
    let entity = world
        .spawn(PendingEmoteLoads {
            full: pending,
            masked: Default::default(),
        })
        .id();
    world.despawn(entity);
    assert!(weak.upgrade().is_none());
}

#[test]
fn despawning_an_avatar_releases_its_graph_node_leases() {
    let mut assets = Assets::default();
    let handle = emote(&mut assets);
    let Handle::Strong(strong) = &handle else {
        panic!()
    };
    let weak = std::sync::Arc::downgrade(strong);
    let mut owner = EmoteGraphAssets::default();
    owner.bind(
        Handle::<AnimationGraph>::default().id(),
        AnimationNodeIndex::new(1),
        handle,
    );
    let mut world = World::new();
    let entity = world.spawn(owner).id();
    assert!(weak.upgrade().is_some());
    world.despawn(entity);
    assert!(weak.upgrade().is_none());
}

#[test]
fn ownership_system_preserves_the_same_graph_and_clears_removed_graphs_and_abandoned_requests() {
    let mut app = App::new();
    app.add_systems(Update, synchronize_emote_ownership);
    let mut assets = Assets::default();
    let handle = emote(&mut assets);
    let Handle::Strong(strong) = &handle else {
        panic!()
    };
    let weak = std::sync::Arc::downgrade(strong);
    let graph = Handle::<AnimationGraph>::default();
    let mut owner = EmoteGraphAssets::default();
    owner.bind(graph.id(), AnimationNodeIndex::new(1), handle);
    let gltfs = Assets::<Gltf>::default();
    let mut pending = PendingEmoteGltf::default();
    pending.load(&EmoteUrn::new("wave").unwrap(), || gltfs.reserve_handle());
    let entity = app
        .world_mut()
        .spawn((
            owner,
            AnimationGraphHandle(graph),
            PendingEmoteLoads {
                full: pending,
                masked: Default::default(),
            },
        ))
        .id();
    app.update();
    assert!(weak.upgrade().is_some());
    assert!(app
        .world()
        .get::<PendingEmoteLoads>(entity)
        .unwrap()
        .full
        .0
        .is_none());
    app.world_mut()
        .entity_mut(entity)
        .remove::<AnimationGraphHandle>();
    app.update();
    assert!(weak.upgrade().is_none());
}

type GltfGates = Arc<Mutex<HashMap<String, (oneshot::Sender<()>, oneshot::Receiver<bool>)>>>;

struct ControlledGltfLoader(GltfGates);

impl AssetLoader for ControlledGltfLoader {
    type Asset = Gltf;
    type Settings = ();
    type Error = anyhow::Error;

    async fn load(
        &self,
        _: &mut dyn Reader,
        _: &(),
        context: &mut LoadContext<'_>,
    ) -> Result<Gltf, Self::Error> {
        let content = context.path().file_stem().unwrap().to_str().unwrap();
        let (started, result) = self
            .0
            .lock()
            .unwrap()
            .remove(content)
            .ok_or_else(|| anyhow::anyhow!("unexpected repeat load for {content}"))?;
        let _ = started.send(());
        anyhow::ensure!(result.await?, "fixture load failed");
        let clip = context.add_labeled_asset("clip".into(), clip(1.0));
        Ok(gltf(clip))
    }

    fn extensions(&self) -> &[&str] {
        &["gltf"]
    }
}

fn clip(duration: f32) -> AnimationClip {
    let mut clip = AnimationClip::default();
    clip.set_duration(duration);
    clip
}

fn gltf(clip: Handle<AnimationClip>) -> Gltf {
    Gltf {
        animations: vec![clip],
        named_animations: Default::default(),
        scenes: Default::default(),
        named_scenes: Default::default(),
        meshes: Default::default(),
        named_meshes: Default::default(),
        materials: Default::default(),
        named_materials: Default::default(),
        nodes: Default::default(),
        named_nodes: Default::default(),
        skins: Default::default(),
        named_skins: Default::default(),
        default_scene: None,
        source: None,
    }
}

struct GltfGate {
    started: oneshot::Receiver<()>,
    result: oneshot::Sender<bool>,
}

const SCENE: &str = "fixture";

fn scene_emote(content: &str) -> EmoteUrn {
    EmoteUrn::new(&format!(
        "urn:decentraland:off-chain:scene-emote:{SCENE}-{content}-false"
    ))
    .unwrap()
}

fn path(content: &str) -> bevy::asset::AssetPath<'static> {
    IpfsAssetServer::scene_content_hash_path::<Gltf>(SCENE, content)
}

struct PlaybackFixture {
    app: App,
    dir: Dir,
    gates: GltfGates,
    avatar: Entity,
}

impl PlaybackFixture {
    fn new() -> Self {
        let mut app = App::new();
        let dir = Dir::default();
        let gates = GltfGates::default();
        let source = dir.clone();
        app.register_asset_source(
            AssetSourceId::Default,
            AssetSource::build().with_reader(move || {
                Box::new(MemoryAssetReader {
                    root: source.clone(),
                })
            }),
        )
        .add_plugins((MinimalPlugins, AssetPlugin::default()))
        .init_asset::<Collectible<Emote>>()
        .init_asset::<CollectibleData<Emote>>()
        .init_asset::<Gltf>()
        .init_asset::<AnimationGraph>()
        .init_asset::<AnimationClip>()
        .init_asset::<bevy_kira_audio::AudioSource>()
        .init_asset::<Mesh>()
        .register_asset_loader(ControlledGltfLoader(gates.clone()))
        .init_resource::<Collectibles<Emote>>()
        .init_resource::<SceneSpawner>()
        .init_resource::<SceneDrivenAnimationFeedback>()
        .insert_resource(IpfsResource {
            inner: Arc::new(IpfsIo::new(
                false,
                Box::new(MemoryAssetReader {
                    root: Dir::default(),
                }),
                None,
                Default::default(),
                1,
                tokio::sync::mpsc::unbounded_channel().0,
            )),
        })
        .add_event::<EmoteLifecycleEvent>()
        .add_systems(
            Update,
            (
                animate,
                synchronize_emote_ownership,
                play_current_emote,
                play_masked_emote,
            )
                .chain(),
        );
        app.finish();
        app.cleanup();

        let world = app.world_mut();
        let graph = world
            .resource_mut::<Assets<AnimationGraph>>()
            .add(AnimationGraph::new());
        let avatar = world
            .spawn((AvatarDynamicState::default(), GlobalTransform::IDENTITY))
            .id();
        world.entity_mut(avatar).insert((
            AvatarAnimPlayer(avatar),
            AnimationPlayer::default(),
            AnimationTransitions::default(),
            AnimationGraphHandle(graph),
            EmoteGraphAssets::default(),
            Clips::default(),
        ));
        world.spawn((
            AvatarDefinition {
                render: crate::AvatarRenderDefinition {
                    body: Wearable {
                        category: WearableCategory::BODY_SHAPE,
                        hides: Default::default(),
                        model: None,
                        texture: None,
                        mask: None,
                    },
                    body_shape: base_bodyshapes().remove(0),
                    skin_color: Color::WHITE,
                    hair_color: Color::WHITE,
                    eyes_color: Color::WHITE,
                    wearables: Vec::new(),
                    hides: Default::default(),
                    bounds: Default::default(),
                    disable_dither: false,
                    cel_shading: false,
                    avatar_outline: false,
                },
                emote: None,
            },
            ChildOf(avatar),
        ));
        Self {
            app,
            dir,
            gates,
            avatar,
        }
    }

    fn gate(&self, content: &str) -> GltfGate {
        let (started_tx, started) = oneshot::channel();
        let (result, result_rx) = oneshot::channel();
        self.dir.insert_asset_text(path(content).path(), "fixture");
        self.gates
            .lock()
            .unwrap()
            .insert(content.to_owned(), (started_tx, result_rx));
        GltfGate { started, result }
    }

    fn request(&mut self, content: &str, timestamp: i64, mask: EmoteMask) {
        self.app
            .world_mut()
            .entity_mut(self.avatar)
            .insert(EmoteCommand {
                urn: scene_emote(content).to_string(),
                timestamp,
                r#loop: true,
                mask,
            });
    }

    fn pending(&self, content: &str) -> Weak<StrongHandle> {
        let server = self.app.world().resource::<AssetServer>();
        let Handle::Strong(handle) = server.get_handle::<Gltf>(path(content)).unwrap() else {
            panic!("by-hash request should be strongly held");
        };
        Arc::downgrade(&handle)
    }

    fn update_until(&mut self, reason: &str, mut check: impl FnMut(&World) -> bool) {
        let deadline = Instant::now() + Duration::from_secs(5);
        while !check(self.app.world()) {
            assert!(Instant::now() < deadline, "{reason}");
            self.app.update();
            std::thread::sleep(Duration::from_millis(1));
        }
    }

    fn wait_for_load(&mut self, gate: &mut GltfGate) {
        self.update_until("the by-hash loader did not start", |_| {
            match gate.started.try_recv() {
                Ok(()) => true,
                Err(oneshot::error::TryRecvError::Empty) => false,
                Err(err) => panic!("loader did not signal start: {err}"),
            }
        });
    }
}

#[test]
fn delayed_full_body_request_keeps_one_load_until_its_clip_plays() {
    let mut fixture = PlaybackFixture::new();
    let mut gate = fixture.gate("wave");
    fixture.request("wave", 1, EmoteMask::FullBody);
    fixture.wait_for_load(&mut gate);
    let original = fixture.pending("wave");
    for _ in 0..4 {
        fixture.app.update();
        assert!(Weak::ptr_eq(&original, &fixture.pending("wave")));
    }
    gate.result.send(true).unwrap();
    let avatar = fixture.avatar;
    fixture.update_until("the original request never played", |world| {
        world.get::<ActiveEmote>(avatar).is_some_and(|active| {
            active.urn == scene_emote("wave") && active.duration_ms == Some(1000)
        })
    });
    assert!(fixture
        .app
        .world()
        .get::<AnimationPlayer>(avatar)
        .unwrap()
        .playing_animations()
        .next()
        .is_some());
}

#[test]
fn replacement_and_avatar_despawn_drop_their_pending_loads() {
    let mut fixture = PlaybackFixture::new();
    let mut first = fixture.gate("wave");
    let mut replacement = fixture.gate("dance");
    fixture.request("wave", 1, EmoteMask::FullBody);
    fixture.wait_for_load(&mut first);
    let original = fixture.pending("wave");
    fixture.request("dance", 2, EmoteMask::FullBody);
    fixture.wait_for_load(&mut replacement);
    assert!(original.upgrade().is_none(), "replaced load is still owned");
    let pending = fixture.pending("dance");
    fixture.app.world_mut().despawn(fixture.avatar);
    assert!(
        pending.upgrade().is_none(),
        "despawned avatar still owns its pending load"
    );
}

#[test]
fn masked_suspension_releases_its_pending_load_without_ending_the_request() {
    let mut fixture = PlaybackFixture::new();
    let mut overlay = fixture.gate("wave");
    let mut full = fixture.gate("dance");
    fixture.request("wave", 1, EmoteMask::UpperBody);
    fixture.wait_for_load(&mut overlay);
    let masked = fixture.pending("wave");
    fixture.request("dance", 2, EmoteMask::FullBody);
    fixture.wait_for_load(&mut full);
    let request = fixture
        .app
        .world()
        .get::<MaskedEmote>(fixture.avatar)
        .unwrap();
    assert!(!request.is_playing());
    assert_eq!(request.request.as_ref().unwrap().urn, scene_emote("wave"));
    assert!(
        masked.upgrade().is_none(),
        "suspended overlay still owns its pending load"
    );
    assert!(
        fixture.pending("dance").upgrade().is_some(),
        "full-body replacement is not retained"
    );

    let mut resumed = fixture.gate("wave");
    full.result.send(false).unwrap();
    fixture.wait_for_load(&mut resumed);
    assert!(!Weak::ptr_eq(&masked, &fixture.pending("wave")));
    resumed.result.send(true).unwrap();
    let avatar = fixture.avatar;
    fixture.update_until(
        "the looping overlay did not resume after suspension",
        |world| {
            world
                .get::<MaskedEmote>(avatar)
                .and_then(MaskedEmote::active_request)
                .is_some_and(|request| {
                    request.urn == scene_emote("wave") && request.duration_ms == Some(1000)
                })
        },
    );
}

#[test]
fn terminal_gltf_failure_stops_playback_and_releases_its_staging_handle() {
    let mut fixture = PlaybackFixture::new();
    let mut gate = fixture.gate("wave");
    fixture.request("wave", 1, EmoteMask::FullBody);
    fixture.wait_for_load(&mut gate);
    let pending = fixture.pending("wave");
    gate.result.send(false).unwrap();
    let avatar = fixture.avatar;
    fixture.update_until("the load error did not end the requested emote", |world| {
        world
            .get::<ActiveEmote>(avatar)
            .is_some_and(|active| active.urn == scene_emote("wave") && active.finished)
    });
    assert!(
        pending.upgrade().is_none(),
        "failed staging handle is still owned"
    );
    fixture.app.update();
    assert_eq!(
        fixture.app.world().get::<ActiveEmote>(avatar).unwrap().urn,
        *URN_IDLE
    );
}

#[test]
fn matching_preload_takes_over_a_pending_by_hash_request() {
    let mut fixture = PlaybackFixture::new();
    let mut gate = fixture.gate("wave");
    fixture.request("wave", 1, EmoteMask::FullBody);
    fixture.wait_for_load(&mut gate);
    let pending = fixture.pending("wave");
    let world = fixture.app.world_mut();
    let preloaded_clip = world.resource_mut::<Assets<AnimationClip>>().add(clip(2.0));
    let preloaded = world
        .resource_mut::<Assets<Gltf>>()
        .add(gltf(preloaded_clip));
    world.spawn(PreloadedGltf {
        scene_hash: SCENE.to_owned(),
        content_hash: "wave".to_owned(),
        handle: preloaded,
    });

    fixture.app.update();
    assert_eq!(
        fixture
            .app
            .world()
            .get::<ActiveEmote>(fixture.avatar)
            .unwrap()
            .duration_ms,
        Some(2000),
        "the clip supplied by the preload must play even while the by-hash loader is blocked",
    );
    assert!(
        pending.upgrade().is_none(),
        "preload did not release the by-hash handle"
    );
}

#[test]
fn locomotion_fallback_does_not_discard_the_scene_movement_load() {
    let mut fixture = PlaybackFixture::new();
    let mut gate = fixture.gate("stride");
    // Install playback components before making either clip available.
    fixture.app.update();
    let world = fixture.app.world_mut();
    let idle_clip = world.resource_mut::<Assets<AnimationClip>>().add(clip(2.0));
    let mut idle_gltf = gltf(idle_clip.clone());
    idle_gltf
        .named_animations
        .insert("_Avatar".into(), idle_clip);
    let idle_gltf = world.resource_mut::<Assets<Gltf>>().add(idle_gltf);
    world
        .run_system_once(move |mut emotes: CollectibleManager<Emote>| {
            emotes.add_builtin(
                URN_IDLE.clone(),
                Collectible {
                    representations: HashMap::from_iter([(
                        base_bodyshapes().remove(0),
                        Emote {
                            gltf: idle_gltf.clone(),
                            default_repeat: true,
                            sound: Vec::new(),
                        },
                    )]),
                    data: CollectibleData {
                        hash: String::new(),
                        urn: URN_IDLE.to_string(),
                        thumbnail: String::new(),
                        available_representations: Default::default(),
                        name: String::new(),
                        description: String::new(),
                        extra_data: EmoteExtraData::default(),
                    },
                },
            );
        })
        .unwrap();
    world.entity_mut(fixture.avatar).insert(SceneDrivenAnim {
        active: Some(SceneDrivenAnimationRequest {
            src: "stride.glb".to_owned(),
            urn: scene_emote("stride").to_string(),
            scene_hash: SCENE.to_owned(),
            content_hash: "stride".to_owned(),
            speed: 1.0,
            ..Default::default()
        }),
    });
    fixture.app.update();
    let world = fixture.app.world();
    assert!(
        world
            .get::<Clips>(fixture.avatar)
            .unwrap()
            .named
            .contains_key(URN_IDLE.as_str()),
        "the avatar must be able to play locomotion while waiting for its scene clip"
    );
    assert!(
        world
            .get::<PendingEmoteLoads>(fixture.avatar)
            .unwrap()
            .full
            .0
            .is_some(),
        "playing the locomotion fallback must preserve the scene's pending GLTF"
    );
    fixture.wait_for_load(&mut gate);
    let pending = fixture.pending("stride");
    fixture.app.update();
    assert!(Weak::ptr_eq(&pending, &fixture.pending("stride")));
    gate.result.send(true).unwrap();
    let avatar = fixture.avatar;
    fixture.update_until(
        "the scene movement clip never replaced locomotion",
        |world| {
            world.get::<ActiveEmote>(avatar).is_some_and(|active| {
                active.urn == scene_emote("stride") && active.duration_ms == Some(1000)
            })
        },
    );
}
