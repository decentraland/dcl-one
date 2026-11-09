use std::{sync::Arc, time::Duration};

use bevy::{
    asset::io::memory::{Dir, MemoryAssetReader},
    ecs::system::RunSystemOnce,
    gltf::Gltf,
};
use common::memory_reclaim::{MemoryReclaimPlugin, MemoryReclaimSettings, ReclaimSimulation};
use ipfs::{IpfsIo, IpfsResource};

use super::*;

const BODY_SHAPE: &str = "urn:decentraland:off-chain:base-avatars:basefemale";
const SCENE_EMOTE: &str = "urn:decentraland:off-chain:scene-emote:scene-clip-false";

/// Models the lease an avatar's graph node takes on the whole emote. Its clip
/// alone would not keep the collectible, GLTF, props or sounds alive.
#[derive(Component)]
struct GraphEmoteLease(Handle<Collectible<Emote>>);

fn synthesize(world: &mut World) -> AssetId<Collectible<Emote>> {
    let clip = world
        .resource_mut::<Assets<AnimationClip>>()
        .add(AnimationClip::default());
    let gltf = world.resource_mut::<Assets<Gltf>>().add(Gltf {
        animations: vec![clip.clone()],
        named_animations: HashMap::from_iter([("_Avatar".into(), clip)]),
        scenes: Vec::new(),
        named_scenes: Default::default(),
        meshes: Vec::new(),
        named_meshes: Default::default(),
        materials: Vec::new(),
        named_materials: Default::default(),
        nodes: Vec::new(),
        named_nodes: Default::default(),
        skins: Vec::new(),
        named_skins: Default::default(),
        default_scene: None,
        source: None,
    });
    world
        .run_system_once(move |mut manager: CollectibleManager<Emote>| {
            let urn = EmoteUrn::new(SCENE_EMOTE).unwrap();
            manager.add_dynamic(
                urn.clone(),
                Collectible {
                    representations: HashMap::from_iter([(
                        BODY_SHAPE.to_owned(),
                        Emote {
                            gltf: gltf.clone(),
                            default_repeat: false,
                            sound: Vec::new(),
                        },
                    )]),
                    data: CollectibleData {
                        hash: "clip".to_owned(),
                        urn: urn.to_string(),
                        thumbnail: String::new(),
                        available_representations: HashSet::from_iter([BODY_SHAPE.to_owned()]),
                        name: "Scene emote".to_owned(),
                        description: String::new(),
                        extra_data: EmoteExtraData::default(),
                    },
                },
            );
            manager
                .get_representation_with_handle(&urn, BODY_SHAPE)
                .unwrap()
                .1
                .id()
        })
        .unwrap()
}

fn lookup(
    mut manager: CollectibleManager<Emote>,
) -> Result<(Handle<Gltf>, Handle<Collectible<Emote>>), CollectibleError> {
    let (representation, handle) =
        manager.get_representation_with_handle(EmoteUrn::new(SCENE_EMOTE).unwrap(), BODY_SHAPE)?;
    Ok((representation.gltf.clone(), handle))
}

fn advance_to_idle(app: &mut App) {
    // The first update after a manager call still uses the same frame counter;
    // reclamation is eligible after four frames have elapsed.
    for _ in 0..5 {
        app.update();
    }
}

fn released(world: &World) -> u64 {
    world
        .resource::<ReclaimBudget>()
        .caches()
        .iter()
        .find(|stats| stats.spec.name == "emote")
        .unwrap()
        .released
}

fn app_with_pressure() -> App {
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        AssetPlugin::default(),
        MemoryReclaimPlugin,
        CollectiblesTypePlugin::<Emote>::default(),
    ))
    .init_asset::<AnimationGraph>()
    .init_asset::<AnimationClip>()
    .init_asset::<Gltf>()
    .insert_resource(MemoryReclaimSettings {
        frame_budget: Duration::from_secs(1),
        simulation: ReclaimSimulation::Ram,
    })
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
    });
    app.finish();
    app.cleanup();
    app
}

#[test]
fn scene_emote_is_recovered_while_owned_and_reconstructed_after_final_drop() {
    let mut app = app_with_pressure();
    let world = app.world_mut();
    let id = synthesize(world);
    let (gltf, lease) = world.run_system_once(lookup).unwrap().unwrap();
    let gltf_id = gltf.id();
    let clip = world
        .resource::<Assets<Gltf>>()
        .get(&gltf)
        .unwrap()
        .animations[0]
        .clone();
    let clip_id = clip.id();
    drop(gltf);
    let (graph, _) = AnimationGraph::from_clip(clip);
    let graph = world.resource_mut::<Assets<AnimationGraph>>().add(graph);
    let owner = world
        .spawn((AnimationGraphHandle(graph), GraphEmoteLease(lease)))
        .id();

    advance_to_idle(&mut app);
    assert_eq!(
        released(app.world()),
        1,
        "pressure must release the cache pin"
    );
    // Let asset tracking process the pin's release before lookup can acquire
    // another handle. Only the graph lease should be keeping the asset alive.
    app.update();
    let world = app.world_mut();
    let urn = EmoteUrn::new(SCENE_EMOTE).unwrap();
    // Complete a remote request that was already in flight when the scene emote
    // was synthesized. Use the same result admission path as request_collectibles.
    let mut collectibles = world.resource_mut::<Collectibles<Emote>>();
    collectibles.record_remote(urn.clone(), PointerResult::Missing);
    assert!(!collectibles.pointers.contains_key(&urn));
    let (gltf, recovered) = world.run_system_once(lookup).unwrap().unwrap();
    assert_eq!(
        recovered.id(),
        world.get::<GraphEmoteLease>(owner).unwrap().0.id()
    );
    assert_eq!(recovered.id(), id);
    assert_eq!(gltf.id(), gltf_id);
    assert_eq!(
        world
            .resource::<Assets<Gltf>>()
            .get(&gltf)
            .unwrap()
            .animations[0]
            .id(),
        clip_id
    );
    drop((gltf, recovered));

    advance_to_idle(&mut app);
    assert_eq!(
        released(app.world()),
        2,
        "recovered cache pin must also expire"
    );
    app.update();
    assert!(app
        .world()
        .resource::<Assets<Collectible<Emote>>>()
        .contains(id));
    let mut removals = app
        .world()
        .resource::<Events<AssetEvent<Collectible<Emote>>>>()
        .get_cursor_current();
    app.world_mut().despawn(owner);
    app.update();
    assert!(removals
        .read(
            app.world()
                .resource::<Events<AssetEvent<Collectible<Emote>>>>()
        )
        .any(|event| matches!(event, AssetEvent::Removed { id: removed } if *removed == id)));
    app.update();
    let world = app.world_mut();
    assert!(!world.resource::<Assets<Collectible<Emote>>>().contains(id));
    assert!(!world.resource::<Assets<Gltf>>().contains(gltf_id));
    let collectibles = world.resource::<Collectibles<Emote>>();
    assert!(!collectibles.dynamic.contains_key(&urn));
    assert!(!collectibles.dynamic_urns.contains_key(&id));
    assert!(matches!(
        world.run_system_once(lookup).unwrap(),
        Err(CollectibleError::Missing)
    ));

    let replay = synthesize(world);
    let (replayed_gltf, replayed) = world.run_system_once(lookup).unwrap().unwrap();
    assert_ne!(replay, id);
    assert_ne!(replayed_gltf.id(), gltf_id);
    assert_eq!(replayed.id(), replay);
}

#[test]
fn scene_emote_recovery_survives_the_queued_final_handle_drop() {
    let mut app = app_with_pressure();
    let world = app.world_mut();
    let id = synthesize(world);
    let (_, lease) = world.run_system_once(lookup).unwrap().unwrap();
    let Handle::Strong(original) = &lease else {
        panic!("a graph lease must be strong");
    };
    let original = Arc::downgrade(original);
    let owner = world.spawn(GraphEmoteLease(lease)).id();
    advance_to_idle(&mut app);
    assert_eq!(released(app.world()), 1);

    let world = app.world_mut();
    world.despawn(owner);
    // The old handle family's drop is queued. A lookup in the same Update
    // can recover the emote before Bevy processes that drop in PreUpdate.
    assert!(original.upgrade().is_none());
    assert!(world.resource::<Assets<Collectible<Emote>>>().contains(id));
    let (_, recovered) = world.run_system_once(lookup).unwrap().unwrap();
    assert_eq!(recovered.id(), id);
    drop(recovered);
    app.update();
    app.update();
    let (_, after_tracking) = app
        .world_mut()
        .run_system_once(lookup)
        .unwrap()
        .expect("the recovered pin must survive Bevy tracking the old family's drop");
    assert_eq!(after_tracking.id(), id);
    drop(after_tracking);

    advance_to_idle(&mut app);
    assert_eq!(released(app.world()), 2);
    app.update();
    app.update();
    let world = app.world_mut();
    assert!(!world.resource::<Assets<Collectible<Emote>>>().contains(id));
    assert!(matches!(
        world.run_system_once(lookup).unwrap(),
        Err(CollectibleError::Missing)
    ));
    assert_ne!(synthesize(world), id);
}
