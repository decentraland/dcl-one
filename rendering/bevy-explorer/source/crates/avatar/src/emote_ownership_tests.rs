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
    AssetLoader, AsyncReadExt, LoadContext, StrongHandle,
};
use bevy::ecs::system::RunSystemOnce;
use collectibles::{
    base_bodyshapes,
    wearables::{Wearable, WearableCategory},
    Collectibles, CollectiblesTypePlugin,
};
use common::memory_reclaim::{
    MemoryReclaimPlugin, MemoryReclaimSettings, ReclaimBudget, ReclaimSimulation,
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
fn pending_request_reuses_its_handle_and_releases_on_replacement() {
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
    let node = AnimationNodeIndex::new(1);
    owner.bind(graph.id(), node, handle);
    let mut player = AnimationPlayer::default();
    player.start(node);
    let gltfs = Assets::<Gltf>::default();
    let mut pending = PendingEmoteGltf::default();
    pending.load(&EmoteUrn::new("wave").unwrap(), || gltfs.reserve_handle());
    let entity = app
        .world_mut()
        .spawn((
            owner,
            AnimationGraphHandle(graph),
            player,
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
type GltfLoads = Arc<Mutex<HashMap<String, usize>>>;

struct ControlledGltfLoader {
    gates: GltfGates,
    loads: GltfLoads,
}

impl AssetLoader for ControlledGltfLoader {
    type Asset = Gltf;
    type Settings = ();
    type Error = anyhow::Error;

    async fn load(
        &self,
        reader: &mut dyn Reader,
        _: &(),
        context: &mut LoadContext<'_>,
    ) -> Result<Gltf, Self::Error> {
        let content = context.path().file_stem().unwrap().to_str().unwrap();
        *self
            .loads
            .lock()
            .unwrap()
            .entry(content.to_owned())
            .or_default() += 1;
        let (started, result) = self
            .gates
            .lock()
            .unwrap()
            .remove(content)
            .ok_or_else(|| anyhow::anyhow!("unexpected repeat load for {content}"))?;
        let _ = started.send(());
        anyhow::ensure!(result.await?, "fixture load failed");
        let mut contents = Vec::new();
        reader.read_to_end(&mut contents).await?;
        if contents == b"no-animations" {
            return Ok(Gltf {
                animations: Vec::new(),
                ..gltf(Handle::default())
            });
        }
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
    loads: GltfLoads,
    avatar: Entity,
}

#[derive(Resource, Default)]
struct PausePlaybackResolution(bool);

fn resolve_this_frame(paused: Res<PausePlaybackResolution>) -> bool {
    !paused.0
}

impl PlaybackFixture {
    fn new() -> Self {
        Self::build(false)
    }

    fn with_pressure() -> Self {
        Self::build(true)
    }

    fn build(pressure: bool) -> Self {
        let mut app = App::new();
        let dir = Dir::default();
        let gates = GltfGates::default();
        let loads = GltfLoads::default();
        let source = dir.clone();
        app.register_asset_source(
            AssetSourceId::Default,
            AssetSource::build().with_reader(move || {
                Box::new(MemoryAssetReader {
                    root: source.clone(),
                })
            }),
        )
        .add_plugins((MinimalPlugins, AssetPlugin::default()));
        if pressure {
            app.add_plugins((
                MemoryReclaimPlugin,
                CollectiblesTypePlugin::<Emote>::default(),
            ))
            .insert_resource(MemoryReclaimSettings {
                frame_budget: Duration::from_secs(1),
                simulation: ReclaimSimulation::Ram,
            });
        } else {
            app.init_asset::<Collectible<Emote>>()
                .init_asset::<CollectibleData<Emote>>()
                .init_resource::<Collectibles<Emote>>();
        }
        app.init_asset::<Gltf>()
            .init_asset::<AnimationGraph>()
            .init_asset::<AnimationClip>()
            .init_asset::<bevy_kira_audio::AudioSource>()
            .init_asset::<Mesh>()
            .register_asset_loader(ControlledGltfLoader {
                gates: gates.clone(),
                loads: loads.clone(),
            })
            .init_resource::<PausePlaybackResolution>()
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
                    animate.run_if(resolve_this_frame),
                    synchronize_emote_ownership,
                    play_current_emote.run_if(resolve_this_frame),
                    play_masked_emote.run_if(resolve_this_frame),
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
                _collectibles: Vec::new(),
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
        let mut fixture = Self {
            app,
            dir,
            gates,
            loads,
            avatar,
        };
        if pressure {
            fixture.install_idle();
        }
        fixture
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

    fn request_movement(&mut self, scene: Option<Entity>, content: &str) {
        self.app
            .world_mut()
            .entity_mut(self.avatar)
            .insert(SceneDrivenAnim {
                active: Some(SceneDrivenAnimationRequest {
                    scene,
                    src: format!("{content}.glb"),
                    urn: scene_emote(content).to_string(),
                    scene_hash: SCENE.to_owned(),
                    content_hash: content.to_owned(),
                    speed: 1.0,
                    r#loop: true,
                    ..Default::default()
                }),
            });
    }

    fn stop_nodes(&mut self) {
        self.app
            .world_mut()
            .get_mut::<AnimationPlayer>(self.avatar)
            .unwrap()
            .stop_all();
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

    fn install_idle(&mut self) {
        let world = self.app.world_mut();
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
    }

    fn wait_for_playback(&mut self, content: &str, mask: EmoteMask) {
        let avatar = self.avatar;
        let urn = scene_emote(content);
        self.update_until("requested scene emote never played", |world| {
            let Some(player) = world.get::<AnimationPlayer>(avatar) else {
                return false;
            };
            match mask {
                EmoteMask::FullBody => {
                    world
                        .get::<ActiveEmote>(avatar)
                        .is_some_and(|active| active.urn == urn && active.duration_ms == Some(1000))
                        && world
                            .get::<Clips>(avatar)
                            .and_then(|clips| clips.named.get(urn.as_str()))
                            .is_some_and(|(node, _)| {
                                player.is_playing_animation(*node)
                                    && world.get::<AnimationTransitions>(avatar).is_some_and(
                                        |transitions| {
                                            transitions.get_main_animation() == Some(*node)
                                        },
                                    )
                            })
                }
                EmoteMask::UpperBody => world.get::<MaskedEmote>(avatar).is_some_and(|masked| {
                    masked
                        .active_request()
                        .is_some_and(|active| active.urn == urn && active.duration_ms == Some(1000))
                        && masked.playing.is_some_and(|(node, _)| {
                            masked
                                .clips
                                .get(urn.as_str())
                                .is_some_and(|(requested, _)| *requested == node)
                                && player.is_playing_animation(node)
                        })
                }),
            }
        });
    }

    fn aggregate_id(&self, content: &str) -> Option<AssetId<Collectible<Emote>>> {
        let urn = scene_emote(content);
        self.app
            .world()
            .resource::<Assets<Collectible<Emote>>>()
            .iter()
            .find_map(|(id, asset)| (asset.data.urn == urn.as_str()).then_some(id))
    }

    fn gltf_id(&self, content: &str) -> Option<AssetId<Gltf>> {
        self.app
            .world()
            .resource::<AssetServer>()
            .get_handle::<Gltf>(path(content))
            .map(|handle| handle.id())
    }

    fn expire_without_resolution(&mut self) {
        // Cache age and asset tracking advance while the installed animation keeps its state.
        // Ownership synchronization still runs; only repeated resolver touches are withheld.
        self.app
            .world_mut()
            .resource_mut::<PausePlaybackResolution>()
            .0 = true;
        for _ in 0..8 {
            self.app.update();
        }
        let cache = self
            .app
            .world()
            .resource::<ReclaimBudget>()
            .caches()
            .iter()
            .find(|cache| cache.spec.name == "emote")
            .unwrap();
        assert!(
            cache.released > 0,
            "real pressure cleanup must release the cache pin"
        );
        assert_eq!(
            cache.entries, 0,
            "the cache must not mask missing playback ownership"
        );
    }
}

#[test]
fn actual_playback_owns_full_and_masked_aggregates_after_pressure_releases_cache_pins() {
    for mask in [EmoteMask::FullBody, EmoteMask::UpperBody] {
        let mut fixture = PlaybackFixture::with_pressure();
        let mut gate = fixture.gate("wave");
        fixture.request("wave", 1, mask);
        fixture.wait_for_load(&mut gate);
        gate.result.send(true).unwrap();
        fixture.wait_for_playback("wave", mask);
        let aggregate = fixture.aggregate_id("wave").unwrap();
        let gltf = fixture.gltf_id("wave").unwrap();
        fixture.expire_without_resolution();
        assert_eq!(
            fixture.aggregate_id("wave"),
            Some(aggregate),
            "play_slot must retain its active aggregate"
        );
        assert_eq!(
            fixture.gltf_id("wave"),
            Some(gltf),
            "the managed GLTF belongs to the active aggregate"
        );

        fixture
            .app
            .world_mut()
            .resource_mut::<PausePlaybackResolution>()
            .0 = false;
        fixture.request("wave", 2, mask);
        fixture.app.update();
        fixture.wait_for_playback("wave", mask);
        assert_eq!(fixture.aggregate_id("wave"), Some(aggregate));
        assert_eq!(
            fixture.loads.lock().unwrap()["wave"],
            1,
            "active replay must not load again"
        );
        fixture.app.world_mut().despawn(fixture.avatar);
        fixture.expire_without_resolution();
        assert_eq!(fixture.aggregate_id("wave"), None);
        assert_eq!(fixture.gltf_id("wave"), None);
    }
}

#[test]
fn stopped_graph_node_keeps_its_aggregate_and_replays_without_loading() {
    let mut fixture = PlaybackFixture::with_pressure();
    let mut gate = fixture.gate("wave");
    fixture.request("wave", 1, EmoteMask::FullBody);
    fixture.wait_for_load(&mut gate);
    gate.result.send(true).unwrap();
    fixture.wait_for_playback("wave", EmoteMask::FullBody);
    let aggregate = fixture.aggregate_id("wave").unwrap();
    let gltf = fixture.gltf_id("wave").unwrap();
    let node = fixture
        .app
        .world()
        .get::<Clips>(fixture.avatar)
        .unwrap()
        .named[scene_emote("wave").as_str()]
    .0;
    let mut next = fixture.gate("dance");
    fixture.request("dance", 2, EmoteMask::FullBody);
    fixture.wait_for_load(&mut next);
    next.result.send(true).unwrap();
    fixture.wait_for_playback("dance", EmoteMask::FullBody);
    {
        let mut player = fixture
            .app
            .world_mut()
            .get_mut::<AnimationPlayer>(fixture.avatar)
            .unwrap();
        let outgoing = player
            .animation_mut(node)
            .expect("crossfade must keep the outgoing node");
        outgoing.pause().set_repeat(RepeatAnimation::Count(0));
        assert!(outgoing.is_paused() && outgoing.is_finished());
    }
    fixture.expire_without_resolution();
    assert_eq!(
        fixture.aggregate_id("wave"),
        Some(aggregate),
        "fading, paused and finished nodes remain owned while registered on the player"
    );
    assert_eq!(fixture.gltf_id("wave"), Some(gltf));
    fixture
        .app
        .world_mut()
        .get_mut::<AnimationPlayer>(fixture.avatar)
        .unwrap()
        .stop_all();
    fixture.expire_without_resolution();
    assert_eq!(
        fixture.aggregate_id("wave"),
        Some(aggregate),
        "a stopped graph node owns its aggregate until its node is reused"
    );
    assert_eq!(
        fixture.gltf_id("wave"),
        Some(gltf),
        "graph membership must keep the managed GLTF loaded"
    );
    assert_eq!(
        fixture
            .app
            .world()
            .get::<Clips>(fixture.avatar)
            .unwrap()
            .named[scene_emote("wave").as_str()]
        .0,
        node
    );

    fixture
        .app
        .world_mut()
        .resource_mut::<PausePlaybackResolution>()
        .0 = false;
    fixture.request("wave", 3, EmoteMask::FullBody);
    fixture.wait_for_playback("wave", EmoteMask::FullBody);
    assert_eq!(fixture.aggregate_id("wave"), Some(aggregate));
    assert_eq!(fixture.gltf_id("wave"), Some(gltf));
    assert_eq!(fixture.loads.lock().unwrap()["wave"], 1);
    assert!(fixture
        .app
        .world()
        .get::<AnimationPlayer>(fixture.avatar)
        .unwrap()
        .is_playing_animation(node));
    fixture.expire_without_resolution();
    assert!(
        fixture.aggregate_id("wave").is_some(),
        "replayed animation keeps the original aggregate"
    );
}

#[test]
fn suspended_loop_keeps_its_graph_aggregate_and_resumes_without_loading() {
    let mut fixture = PlaybackFixture::with_pressure();
    fixture
        .app
        .insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
            Duration::from_millis(100),
        ));
    let mut overlay = fixture.gate("wave");
    fixture.request("wave", 1, EmoteMask::UpperBody);
    fixture.wait_for_load(&mut overlay);
    overlay.result.send(true).unwrap();
    fixture.wait_for_playback("wave", EmoteMask::UpperBody);
    fixture.app.update();
    let aggregate = fixture.aggregate_id("wave").unwrap();
    let gltf = fixture.gltf_id("wave").unwrap();
    let mut full = fixture.gate("dance");
    fixture.request("dance", 2, EmoteMask::FullBody);
    fixture.wait_for_load(&mut full);
    full.result.send(true).unwrap();
    fixture.wait_for_playback("dance", EmoteMask::FullBody);
    for _ in 0..8 {
        fixture.app.update();
    }
    let masked = fixture
        .app
        .world()
        .get::<MaskedEmote>(fixture.avatar)
        .unwrap();
    assert!(!masked.is_playing());
    assert_eq!(masked.request.as_ref().unwrap().urn, scene_emote("wave"));
    assert_eq!(
        fixture.aggregate_id("wave"),
        Some(aggregate),
        "suspended loop remains a graph member"
    );
    assert_eq!(fixture.gltf_id("wave"), Some(gltf));

    // Removing the full-body command leaves the suspended looping request intact.
    fixture
        .app
        .world_mut()
        .entity_mut(fixture.avatar)
        .remove::<EmoteCommand>();
    fixture.wait_for_playback("wave", EmoteMask::UpperBody);
    assert_eq!(fixture.aggregate_id("wave"), Some(aggregate));
    assert_eq!(fixture.loads.lock().unwrap()["wave"], 1);
    fixture.expire_without_resolution();
    assert!(
        fixture.aggregate_id("wave").is_some(),
        "resumed slot keeps the original aggregate"
    );
}

#[test]
fn movement_scene_keeps_rare_clips_after_graph_reuse_and_avatar_despawn() {
    let mut fixture = PlaybackFixture::with_pressure();
    fixture
        .app
        .insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
            Duration::from_millis(100),
        ));
    let scene = fixture.app.world_mut().spawn_empty().id();
    let mut loaded = Vec::new();
    for index in 0..MAX_CLIP_NODES + 2 {
        let content = format!("step{index}");
        let mut gate = fixture.gate(&content);
        fixture.stop_nodes();
        fixture.request_movement(Some(scene), &content);
        fixture.wait_for_load(&mut gate);
        gate.result.send(true).unwrap();
        fixture.wait_for_playback(&content, EmoteMask::FullBody);
        loaded.push((
            content.clone(),
            fixture.aggregate_id(&content).unwrap(),
            fixture.gltf_id(&content).unwrap(),
        ));
    }
    let (rare, rare_aggregate, rare_gltf) = &loaded[0];
    assert!(
        !fixture
            .app
            .world()
            .get::<Clips>(fixture.avatar)
            .unwrap()
            .named
            .contains_key(scene_emote(rare).as_str()),
        "the rare clip must actually leave the graph before checking scene retention"
    );
    fixture.expire_without_resolution();
    assert_eq!(fixture.aggregate_id(rare), Some(*rare_aggregate));
    assert_eq!(fixture.gltf_id(rare), Some(*rare_gltf));

    fixture
        .app
        .world_mut()
        .resource_mut::<PausePlaybackResolution>()
        .0 = false;
    fixture.stop_nodes();
    fixture.request_movement(Some(scene), rare);
    fixture.wait_for_playback(rare, EmoteMask::FullBody);
    assert_eq!(fixture.aggregate_id(rare), Some(*rare_aggregate));
    assert_eq!(fixture.loads.lock().unwrap()[rare], 1);

    let other_scene = fixture.app.world_mut().spawn_empty().id();
    let (shared, shared_aggregate, shared_gltf) = loaded.last().unwrap();
    fixture.request_movement(Some(scene), shared);
    fixture.wait_for_playback(shared, EmoteMask::FullBody);
    // A controller switch must acquire ownership even when the clip does not change.
    fixture.request_movement(Some(other_scene), shared);
    fixture.app.update();
    fixture.wait_for_playback(shared, EmoteMask::FullBody);
    fixture.app.world_mut().despawn(fixture.avatar);
    fixture.expire_without_resolution();
    for (content, aggregate, gltf) in &loaded {
        assert_eq!(fixture.aggregate_id(content), Some(*aggregate));
        assert_eq!(fixture.gltf_id(content), Some(*gltf));
    }

    fixture.app.world_mut().despawn(scene);
    fixture.expire_without_resolution();
    assert_eq!(fixture.aggregate_id(rare), None);
    assert_eq!(fixture.gltf_id(rare), None);
    assert_eq!(fixture.aggregate_id(shared), Some(*shared_aggregate));
    assert_eq!(fixture.gltf_id(shared), Some(*shared_gltf));

    fixture.app.world_mut().despawn(other_scene);
    fixture.expire_without_resolution();
    for (content, _, _) in &loaded {
        assert_eq!(fixture.aggregate_id(content), None);
        assert_eq!(fixture.gltf_id(content), None);
    }
}

#[test]
fn remote_movement_emotes_release_when_their_graph_nodes_are_reused() {
    let mut fixture = PlaybackFixture::with_pressure();
    fixture
        .app
        .insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
            Duration::from_millis(100),
        ));
    for index in 0..MAX_CLIP_NODES + 2 {
        let content = format!("step{index}");
        let mut gate = fixture.gate(&content);
        fixture.stop_nodes();
        fixture.request_movement(None, &content);
        fixture.wait_for_load(&mut gate);
        gate.result.send(true).unwrap();
        fixture.wait_for_playback(&content, EmoteMask::FullBody);
    }
    fixture.expire_without_resolution();
    assert_eq!(fixture.aggregate_id("step0"), None);
    assert_eq!(fixture.gltf_id("step0"), None);
    let recent = format!("step{}", MAX_CLIP_NODES + 1);
    let aggregate = fixture.aggregate_id(&recent).unwrap();
    fixture.stop_nodes();
    fixture.expire_without_resolution();
    assert_eq!(fixture.aggregate_id(&recent), Some(aggregate));
    fixture.app.world_mut().despawn(fixture.avatar);
    fixture.expire_without_resolution();
    assert_eq!(fixture.aggregate_id(&recent), None);
    assert_eq!(fixture.gltf_id(&recent), None);
}

#[test]
fn delayed_movement_resolution_belongs_to_the_current_scene() {
    let mut fixture = PlaybackFixture::with_pressure();
    let old_scene = fixture.app.world_mut().spawn_empty().id();
    let current_scene = fixture.app.world_mut().spawn_empty().id();
    let mut gate = fixture.gate("stride");
    fixture.request_movement(Some(old_scene), "stride");
    fixture.wait_for_load(&mut gate);
    // The content is shared, but only the scene controlling playback when it resolves owns it.
    fixture.request_movement(Some(current_scene), "stride");
    gate.result.send(true).unwrap();
    fixture.wait_for_playback("stride", EmoteMask::FullBody);
    let aggregate = fixture.aggregate_id("stride").unwrap();
    fixture.app.world_mut().despawn(fixture.avatar);
    fixture.expire_without_resolution();
    assert_eq!(fixture.aggregate_id("stride"), Some(aggregate));
    assert_eq!(fixture.loads.lock().unwrap()["stride"], 1);

    fixture.app.world_mut().despawn(current_scene);
    fixture.expire_without_resolution();
    assert_eq!(fixture.aggregate_id("stride"), None);
    assert_eq!(fixture.gltf_id("stride"), None);
    assert!(fixture.app.world().get_entity(old_scene).is_ok());
}

#[test]
fn despawned_movement_scene_cannot_acquire_a_late_resolved_emote() {
    let mut fixture = PlaybackFixture::with_pressure();
    let scene = fixture.app.world_mut().spawn_empty().id();
    let mut gate = fixture.gate("stride");
    fixture.request_movement(Some(scene), "stride");
    fixture.wait_for_load(&mut gate);
    fixture.app.world_mut().despawn(scene);
    let replacement = fixture.app.world_mut().spawn_empty().id();
    gate.result.send(true).unwrap();
    fixture.wait_for_playback("stride", EmoteMask::FullBody);
    fixture.expire_without_resolution();
    assert!(fixture.aggregate_id("stride").is_some());

    fixture.app.world_mut().despawn(fixture.avatar);
    fixture.expire_without_resolution();
    assert_eq!(fixture.aggregate_id("stride"), None);
    assert_eq!(fixture.gltf_id("stride"), None);
    assert!(fixture.app.world().get_entity(replacement).is_ok());
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
    fixture.app.update();
    assert!(
        pending.upgrade().is_none(),
        "failed staging handle is still owned"
    );
    assert_eq!(
        fixture.app.world().get::<ActiveEmote>(avatar).unwrap().urn,
        *URN_IDLE
    );
}

#[test]
fn recurring_failed_scene_movement_uses_one_load_and_stable_fallback() {
    let mut fixture = PlaybackFixture::with_pressure();
    let scene = fixture.app.world_mut().spawn_empty().id();
    let mut failed = fixture.gate("stride");
    fixture.request_movement(Some(scene), "stride");
    fixture.wait_for_load(&mut failed);
    let pending = fixture.pending("stride");
    failed.result.send(false).unwrap();
    let avatar = fixture.avatar;
    fixture.update_until("failed movement never selected locomotion", |world| {
        world.get::<ActiveEmote>(avatar).is_some_and(|active| {
            active.urn == *URN_IDLE
                && active.source == ActiveEmoteSource::VelocitySelected
                && active.duration_ms == Some(2000)
        })
    });
    let idle = fixture
        .app
        .world()
        .get::<AnimationTransitions>(avatar)
        .unwrap()
        .get_main_animation()
        .unwrap();
    let mut fallback_stayed_selected = true;
    for _ in 0..16 {
        fixture.request_movement(Some(scene), "stride");
        fixture.app.update();
        let world = fixture.app.world();
        let active = world.get::<ActiveEmote>(avatar).unwrap();
        fallback_stayed_selected &= active.urn == *URN_IDLE
            && active.source == ActiveEmoteSource::VelocitySelected
            && active.duration_ms == Some(2000);
        assert_eq!(
            world
                .get::<AnimationTransitions>(avatar)
                .unwrap()
                .get_main_animation(),
            Some(idle)
        );
        assert!(world
            .get::<AnimationPlayer>(avatar)
            .unwrap()
            .is_playing_animation(idle));
        std::thread::sleep(Duration::from_millis(1));
    }
    assert_eq!(
        fixture.loads.lock().unwrap()["stride"],
        1,
        "an unchanged movement request must not reload its failed asset"
    );
    assert!(
        fallback_stayed_selected,
        "a terminal movement failure must keep selecting the locomotion fallback"
    );

    let mut replacement = fixture.gate("replacement");
    fixture.request_movement(Some(scene), "replacement");
    fixture.wait_for_load(&mut replacement);
    assert!(
        pending.upgrade().is_none(),
        "replaced failure is still owned"
    );
    replacement.result.send(true).unwrap();
    fixture.wait_for_playback("replacement", EmoteMask::FullBody);

    let mut retried = fixture.gate("stride");
    fixture.request_movement(Some(scene), "stride");
    fixture.wait_for_load(&mut retried);
    retried.result.send(true).unwrap();
    fixture.wait_for_playback("stride", EmoteMask::FullBody);
    assert_eq!(fixture.loads.lock().unwrap()["stride"], 2);
}

#[test]
fn empty_scene_movement_gltf_loads_once_until_request_replacement_or_avatar_despawn() {
    for despawn in [false, true] {
        let mut fixture = PlaybackFixture::with_pressure();
        let scene = fixture.app.world_mut().spawn_empty().id();
        let mut empty = fixture.gate("empty");
        fixture
            .dir
            .insert_asset_text(path("empty").path(), "no-animations");
        fixture.request_movement(Some(scene), "empty");
        fixture.wait_for_load(&mut empty);
        let pending = fixture.pending("empty");
        let gltf = fixture.gltf_id("empty").unwrap();
        empty.result.send(true).unwrap();
        let avatar = fixture.avatar;
        fixture.update_until("empty movement never selected locomotion", |world| {
            world.get::<ActiveEmote>(avatar).is_some_and(|active| {
                active.urn == *URN_IDLE
                    && active.source == ActiveEmoteSource::VelocitySelected
                    && active.duration_ms == Some(2000)
            })
        });
        let idle = fixture
            .app
            .world()
            .get::<AnimationTransitions>(avatar)
            .unwrap()
            .get_main_animation()
            .unwrap();
        let mut fallback_stayed_selected = true;
        for _ in 0..16 {
            fixture.request_movement(Some(scene), "empty");
            fixture.app.update();
            let world = fixture.app.world();
            let active = world.get::<ActiveEmote>(avatar).unwrap();
            fallback_stayed_selected &= active.urn == *URN_IDLE
                && active.source == ActiveEmoteSource::VelocitySelected
                && active.duration_ms == Some(2000);
            assert_eq!(
                world
                    .get::<AnimationTransitions>(avatar)
                    .unwrap()
                    .get_main_animation(),
                Some(idle)
            );
            assert!(world
                .get::<AnimationPlayer>(avatar)
                .unwrap()
                .is_playing_animation(idle));
            std::thread::sleep(Duration::from_millis(1));
        }
        assert_eq!(
            fixture.loads.lock().unwrap()["empty"],
            1,
            "an unchanged movement request must not reload a GLTF without animations"
        );
        assert!(fallback_stayed_selected);
        assert!(pending.upgrade().is_some());
        assert!(fixture
            .app
            .world()
            .resource::<Assets<Gltf>>()
            .get(gltf)
            .unwrap()
            .animations
            .is_empty());

        if despawn {
            fixture.app.world_mut().despawn(avatar);
        } else {
            let mut replacement = fixture.gate("replacement");
            fixture.request_movement(Some(scene), "replacement");
            fixture.wait_for_load(&mut replacement);
            replacement.result.send(true).unwrap();
            fixture.wait_for_playback("replacement", EmoteMask::FullBody);
        }
        assert!(
            pending.upgrade().is_none(),
            "an abandoned invalid movement GLTF is still owned"
        );
        fixture.update_until("the invalid GLTF was not released", |world| {
            world.resource::<Assets<Gltf>>().get(gltf).is_none()
        });
    }
}

#[test]
fn terminal_scene_movement_failure_releases_when_request_ends_or_avatar_despawns() {
    for despawn in [false, true] {
        let mut fixture = PlaybackFixture::with_pressure();
        let mut failed = fixture.gate("stride");
        fixture.request_movement(None, "stride");
        fixture.wait_for_load(&mut failed);
        let pending = fixture.pending("stride");
        failed.result.send(false).unwrap();
        let avatar = fixture.avatar;
        fixture.update_until("failed movement never selected locomotion", |world| {
            world.get::<ActiveEmote>(avatar).is_some_and(|active| {
                active.urn == *URN_IDLE && active.source == ActiveEmoteSource::VelocitySelected
            })
        });
        if despawn {
            fixture.app.world_mut().despawn(avatar);
        } else {
            fixture
                .app
                .world_mut()
                .entity_mut(avatar)
                .remove::<SceneDrivenAnim>();
            fixture.app.update();
        }
        assert!(
            pending.upgrade().is_none(),
            "an abandoned movement failure is still owned"
        );
    }
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
    fixture.install_idle();
    let world = fixture.app.world_mut();
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
