use super::*;

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
