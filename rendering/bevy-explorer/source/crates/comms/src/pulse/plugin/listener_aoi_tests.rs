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

const WORLD: &str = "track.dcl.eth";

/// A ring of parcels around `0,0`-`size,size`: every row but the first and last is two runs.
fn ring(size: i32) -> Vec<String> {
    (0..=size)
        .flat_map(|x| (0..=size).map(move |z| (x, z)))
        .filter(|&(x, z)| x == 0 || z == 0 || x == size || z == size)
        .map(|(x, z)| format!("{x},{z}"))
        .collect()
}

impl Aoi {
    fn in_realm(&mut self, hash: &str, realm: &str) {
        self.app
            .world_mut()
            .resource_mut::<SceneRealms>()
            .0
            .insert(hash.to_owned(), realm.to_owned());
    }

    fn rects(&mut self, realm: &str) -> Vec<pulse::ParcelRect> {
        self.listener()
            .aoi
            .iter()
            .find(|entry| entry.realm == realm)
            .map(|entry| entry.parcel_rects.clone())
            .unwrap_or_default()
    }
}

/// A world is announced as one rect however irregular its scenes are, while routing stays exact.
#[test]
fn a_world_is_announced_as_its_bounding_box() {
    let mut aoi = Aoi::listening();
    aoi.in_realm(SCENE, WORLD);
    aoi.in_realm(OTHER, WORLD);
    let scene = aoi.context(SCENE, true);
    aoi.context(OTHER, true);
    let pointers = ring(20);
    aoi.definition(
        SCENE,
        &pointers.iter().map(String::as_str).collect::<Vec<_>>(),
    );
    aoi.definition(OTHER, &["30,-5"]);
    aoi.update();

    assert_eq!(
        aoi.rects(WORLD),
        [parcel_rect(IVec2::new(0, -5), IVec2::new(30, 20))]
    );
    let routed = aoi.listener().transport_by_parcel[WORLD].clone();
    assert_eq!(routed.len(), pointers.len() + 1);
    // inside the ring, and between the two scenes: in the box, routed to no scene
    assert!(!routed.contains_key(&IVec2::new(10, 10)));
    assert!(!routed.contains_key(&IVec2::new(25, 0)));
    let transport = routed[&IVec2::new(0, 0)];
    assert_eq!(
        aoi.app.world().get::<Transport>(transport).unwrap().context,
        scene
    );
}

/// Anything but a world keeps its exact rects: a Genesis City realm's box would take in other
/// people's scenes.
#[test]
fn a_realm_that_is_not_a_world_is_announced_exactly() {
    let mut aoi = Aoi::listening();
    aoi.context(SCENE, true);
    let pointers = ring(4);
    aoi.definition(
        SCENE,
        &pointers.iter().map(String::as_str).collect::<Vec<_>>(),
    );
    aoi.update();

    let rects = aoi.rects(REALM);
    assert!(rects.len() > 1);
    let area: i64 = rects.iter().map(rect_area).sum();
    assert_eq!(area as usize, pointers.len());
}

/// A realm whose only scene has no transport yet has nothing routable, and the server rejects a
/// realm with no rects, so it waits out of the announcement.
#[test]
fn a_realm_without_a_transport_yet_is_not_announced() {
    let mut aoi = Aoi::listening();
    aoi.in_realm(OTHER, WORLD);
    aoi.context(SCENE, true);
    aoi.context(OTHER, false);
    aoi.definition(SCENE, &["0,0"]);
    aoi.definition(OTHER, &["5,5"]);
    aoi.update();

    let realms: Vec<String> = aoi.listener().aoi.iter().map(|e| e.realm.clone()).collect();
    assert_eq!(realms, [REALM]);
}
