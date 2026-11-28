use super::connection_tests::session_with_link;
use super::*;

const REALM: &str = "realm";
const SCENE: &str = "scene";
const OTHER: &str = "other";
/// Stands in for the routing a rebuild replaces wholesale.
const MARK: &str = "mark";

/// `update_listener_aoi` on an orchestrated server: the process is on no realm of its own and each
/// scene states one, so the default realm never changes and only the inputs themselves can trigger
/// a rebuild.
struct Aoi {
    app: App,
}

impl Aoi {
    fn new() -> Self {
        let mut app = App::new();
        app.init_resource::<Assets<EntityDefinition>>()
            .add_event::<AssetEvent<EntityDefinition>>()
            .init_resource::<CurrentRealm>()
            .insert_resource(SceneRealms(HashMap::from_iter([
                (SCENE.to_owned(), REALM.to_owned()),
                (OTHER.to_owned(), REALM.to_owned()),
            ])))
            .add_systems(Update, update_listener_aoi);
        let shared = app.world_mut().spawn(GlobalCrdtState::new(None)).id();
        app.insert_resource(CrdtContexts(HashMap::from_iter([(String::new(), shared)])));
        Self { app }
    }

    fn listening() -> Self {
        let mut aoi = Self::new();
        aoi.listen();
        aoi
    }

    fn listen(&mut self) {
        let (session, _) = session_with_link(PulseRole::Listener(ListenerRole::default()));
        self.app.insert_resource(session);
    }

    /// Registers a room context for `hash`, with its state or (as a deferred spawn leaves it) not
    /// yet.
    fn context(&mut self, hash: &str, with_state: bool) -> Entity {
        let world = self.app.world_mut();
        let context = if with_state {
            world
                .spawn(GlobalCrdtState::new(Some(hash.to_owned())))
                .id()
        } else {
            world.spawn_empty().id()
        };
        world
            .resource_mut::<CrdtContexts>()
            .0
            .insert(hash.to_owned(), context);
        context
    }

    fn definition(&mut self, id: &str, pointers: &[&str]) -> AssetId<EntityDefinition> {
        let world = self.app.world_mut();
        let id = world
            .resource_mut::<Assets<EntityDefinition>>()
            .add(EntityDefinition {
                id: id.to_owned(),
                pointers: pointers.iter().map(|p| (*p).to_owned()).collect(),
                ..default()
            })
            .id();
        world.send_event(AssetEvent::Added { id });
        id
    }

    fn remove(&mut self, id: AssetId<EntityDefinition>) {
        let world = self.app.world_mut();
        world.resource_mut::<Assets<EntityDefinition>>().remove(id);
        world.send_event(AssetEvent::Removed { id });
    }

    fn listener(&mut self) -> &mut ListenerRole {
        match &mut self
            .app
            .world_mut()
            .resource_mut::<PulseSession>()
            .into_inner()
            .role
        {
            PulseRole::Listener(listener) => listener,
            PulseRole::Player(_) => unreachable!(),
        }
    }

    fn mark(&mut self) {
        self.listener().transport_by_parcel = HashMap::from_iter([(MARK.to_owned(), default())]);
    }

    fn rebuilt(&mut self) -> bool {
        !self.listener().transport_by_parcel.contains_key(MARK)
    }

    /// The parcels routed in `REALM`, by the context behind each one's transport.
    fn routed(&mut self) -> HashMap<IVec2, Entity> {
        let world = self.app.world_mut();
        let transports: HashMap<Entity, Entity> = world
            .query::<(Entity, &Transport)>()
            .iter(world)
            .map(|(entity, transport)| (entity, transport.context))
            .collect();
        self.listener()
            .transport_by_parcel
            .get(REALM)
            .into_iter()
            .flatten()
            .map(|(parcel, transport)| (*parcel, transports[transport]))
            .collect()
    }

    /// The contexts that have a transport, one entry per transport.
    fn transported(&mut self) -> Vec<Entity> {
        let world = self.app.world_mut();
        let mut contexts: Vec<Entity> = world
            .query::<&Transport>()
            .iter(world)
            .map(|transport| transport.context)
            .collect();
        contexts.sort();
        contexts
    }

    fn update(&mut self) {
        self.app.update();
    }
}

fn routes(entries: &[((i32, i32), Entity)]) -> HashMap<IVec2, Entity> {
    entries
        .iter()
        .map(|((x, z), context)| (IVec2::new(*x, *z), *context))
        .collect()
}

#[test]
fn nothing_is_rebuilt_while_nothing_changes() {
    let mut aoi = Aoi::listening();
    let scene = aoi.context(SCENE, true);
    aoi.definition(SCENE, &["0,0", "1,0"]);
    aoi.update();
    assert_eq!(aoi.routed(), routes(&[((0, 0), scene), ((1, 0), scene)]));

    aoi.mark();
    for _ in 0..3 {
        aoi.update();
        assert!(!aoi.rebuilt());
    }
    assert_eq!(aoi.transported(), [scene]);

    let other = aoi.context(OTHER, true);
    aoi.definition(OTHER, &["5,5"]);
    aoi.update();
    assert!(aoi.rebuilt());
    assert_eq!(
        aoi.routed(),
        routes(&[((0, 0), scene), ((1, 0), scene), ((5, 5), other)])
    );
}

/// A room's state is spawned by deferred commands, so it can land a frame after its context is
/// registered. It gets its transport then, and waiting for it costs no rebuild per frame.
#[test]
fn a_context_gets_its_transport_once_its_state_lands() {
    let mut aoi = Aoi::listening();
    let scene = aoi.context(SCENE, false);
    aoi.definition(SCENE, &["0,0"]);
    aoi.update();
    assert!(aoi.transported().is_empty());
    assert!(aoi.routed().is_empty());

    aoi.mark();
    aoi.update();
    aoi.update();
    assert!(!aoi.rebuilt());

    aoi.app
        .world_mut()
        .entity_mut(scene)
        .insert(GlobalCrdtState::new(Some(SCENE.to_owned())));
    aoi.update();
    assert_eq!(aoi.transported(), [scene]);
    assert_eq!(aoi.routed(), routes(&[((0, 0), scene)]));
}

#[test]
fn a_change_before_the_listener_exists_is_applied_once_it_does() {
    let mut aoi = Aoi::new();
    let scene = aoi.context(SCENE, true);
    aoi.definition(SCENE, &["0,0"]);
    aoi.update();
    aoi.update();

    aoi.listen();
    aoi.update();
    assert_eq!(aoi.transported(), [scene]);
    assert_eq!(aoi.routed(), routes(&[((0, 0), scene)]));
}

/// Profiles and wearables load as players come and go; they point at no parcel.
#[test]
fn a_definition_without_parcels_rebuilds_nothing() {
    let mut aoi = Aoi::listening();
    let scene = aoi.context(SCENE, true);
    aoi.definition(SCENE, &["0,0"]);
    aoi.update();

    aoi.mark();
    let profile = aoi.definition("profile", &["0x0000000000000000000000000000000000000001"]);
    aoi.update();
    assert!(!aoi.rebuilt());
    aoi.remove(profile);
    aoi.update();
    assert!(!aoi.rebuilt());
    assert_eq!(aoi.transported(), [scene]);
}

#[test]
fn removing_a_scene_drops_its_routing_and_its_transport() {
    let mut aoi = Aoi::listening();
    aoi.context(SCENE, true);
    let other = aoi.context(OTHER, true);
    let definition = aoi.definition(SCENE, &["0,0"]);
    aoi.definition(OTHER, &["5,5"]);
    aoi.update();
    assert_eq!(aoi.transported().len(), 2);

    aoi.remove(definition);
    aoi.update();
    assert_eq!(aoi.routed(), routes(&[((5, 5), other)]));
    assert_eq!(aoi.transported(), [other]);
}

/// The server's receive buffer: a larger message is dropped and counted as corrupt.
const SERVER_BUFFER_BYTES: usize = 4096;

/// A handshake at the AoI budget, auth chain included, leaves room in the server's buffer.
#[test]
fn a_handshake_at_the_aoi_budget_fits_the_server_buffer() {
    let scenes = [listener_aoi::HostedScene {
        realm: "track.dcl.eth".to_owned(),
        scene: SCENE.to_owned(),
        parcels: (0..2000).map(|i| IVec2::new(i, i)).collect(),
    }];
    let (aoi, compaction) = listener_aoi::listener_aoi(&scenes, usize::MAX);
    assert!(compaction.is_empty());
    let mut fill = aoi[0].clone();
    while listener_aoi::encoded_len(std::slice::from_ref(&fill)) > listener_aoi::MAX_AOI_BYTES {
        fill.parcel_rects.pop();
    }

    let guest = {
        let mut wallet = Wallet::default();
        wallet.finalize_as_guest();
        wallet
    };
    let delegated = {
        let root = wallet::local_guest::new_root_key();
        let (ephemeral, delegates) =
            wallet::local_guest::sign_identity(&root, 1_800_000_000_000).unwrap();
        let mut wallet = Wallet::default();
        wallet.finalize(root.address(), ephemeral, delegates, false);
        wallet
    };
    for (name, wallet) in [("guest", guest), ("delegated", delegated)] {
        let request = futures_lite::future::block_on(build_listener_handshake_request(
            &wallet,
            "",
            vec![fill.clone()],
        ))
        .unwrap();
        let bytes = pulse::ClientMessage {
            message: Some(pulse::client_message::Message::SceneListenerHandshake(
                request.clone(),
            )),
        }
        .encoded_len();
        println!(
            "{name}: auth chain {} bytes, handshake {bytes} bytes at an AoI of {} bytes",
            request.auth_chain.len(),
            listener_aoi::encoded_len(std::slice::from_ref(&fill))
        );
        assert!(bytes + 256 <= SERVER_BUFFER_BYTES, "{name}: {bytes}");
    }
}

/// Compaction changes what is announced, never what is routed.
#[test]
fn a_compacted_scene_is_still_routed_by_its_exact_parcels() {
    let mut aoi = Aoi::listening();
    let scene = aoi.context(SCENE, true);
    let other = aoi.context(OTHER, true);
    let track = listener_aoi::tests::track();
    let pointers: Vec<String> = track.iter().map(|p| format!("{},{}", p.x, p.y)).collect();
    let pointers: Vec<&str> = pointers.iter().map(String::as_str).collect();
    aoi.definition(SCENE, &pointers);
    aoi.definition(OTHER, &["1000,1000"]);
    aoi.update();

    let expected: HashMap<IVec2, Entity> = track
        .iter()
        .map(|p| (*p, scene))
        .chain([(IVec2::new(1000, 1000), other)])
        .collect();
    assert_eq!(aoi.routed(), expected);
    let listener = aoi.listener();
    assert!(listener_aoi::encoded_len(&listener.aoi) <= listener_aoi::MAX_AOI_BYTES);
    assert_eq!(listener.compaction.collapsed.len(), 1);
    assert_eq!(listener.aoi[0].parcel_rects.len(), 2);
}

/// A realm left out after a rejection stays out until the hosted scenes change.
#[test]
fn a_left_out_realm_stays_out_until_the_hosted_scenes_change() {
    let mut aoi = Aoi::listening();
    aoi.app
        .world_mut()
        .resource_mut::<SceneRealms>()
        .0
        .insert(OTHER.to_owned(), "other-realm".to_owned());
    aoi.context(SCENE, true);
    aoi.context(OTHER, true);
    aoi.definition(SCENE, &["0,0"]);
    aoi.definition(OTHER, &["5,5"]);
    aoi.update();
    let realms = |aoi: &mut Aoi| -> Vec<String> {
        aoi.listener().aoi.iter().map(|e| e.realm.clone()).collect()
    };
    assert_eq!(realms(&mut aoi), ["other-realm", REALM]);

    aoi.listener().left_out.insert("other-realm".to_owned());
    aoi.listener().aoi.retain(|e| e.realm != "other-realm");
    // an unrelated rebuild with the same scenes keeps it out
    aoi.app
        .world_mut()
        .resource_mut::<SceneRealms>()
        .set_changed();
    aoi.update();
    assert_eq!(realms(&mut aoi), [REALM]);

    aoi.definition("third", &["9,9"]);
    aoi.context("third", true);
    aoi.app
        .world_mut()
        .resource_mut::<SceneRealms>()
        .0
        .insert("third".to_owned(), REALM.to_owned());
    aoi.update();
    assert_eq!(realms(&mut aoi), ["other-realm", REALM]);
    assert!(aoi.listener().left_out.is_empty());
}
