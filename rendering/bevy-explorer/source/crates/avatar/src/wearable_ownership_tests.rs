use std::{
    sync::{Arc, Mutex},
    time::Instant,
};

use bevy::asset::{
    io::{
        memory::{Dir, MemoryAssetReader},
        AssetSource, AssetSourceId, Reader,
    },
    AssetLoader, LoadContext,
};
use bevy::ecs::system::RunSystemOnce;
use collectibles::{wearables::WearableExtraData, Collectible, CollectibleData};
use common::memory_reclaim::{MemoryReclaimPlugin, MemoryReclaimSettings, ReclaimSimulation};
use ipfs::{IpfsIo, IpfsResource};

use super::*;

type LoadCounts = Arc<Mutex<HashMap<PathBuf, usize>>>;

struct WearableFixtureLoader {
    wearables: HashMap<PathBuf, (WearableUrn, Wearable)>,
    loads: LoadCounts,
}

impl AssetLoader for WearableFixtureLoader {
    type Asset = Collectible<Wearable>;
    type Settings = ();
    type Error = anyhow::Error;

    async fn load(
        &self,
        _reader: &mut dyn Reader,
        _settings: &(),
        context: &mut LoadContext<'_>,
    ) -> Result<Self::Asset, Self::Error> {
        *self
            .loads
            .lock()
            .unwrap()
            .entry(context.path().to_owned())
            .or_default() += 1;
        let (urn, wearable) = &self.wearables[context.path()];
        let body = base_wearables::default_bodyshape_urn().to_string();
        Ok(Collectible {
            representations: HashMap::from_iter([(body.clone(), wearable.clone())]),
            data: CollectibleData {
                hash: String::new(),
                urn: urn.to_string(),
                thumbnail: String::new(),
                available_representations: HashSet::from_iter([body]),
                name: String::new(),
                description: String::new(),
                extra_data: WearableExtraData {
                    category: wearable.category,
                },
            },
        })
    }

    fn extensions(&self) -> &[&str] {
        &["wearable"]
    }
}

struct Fixture {
    app: App,
    paths: HashMap<String, PathBuf>,
    loads: LoadCounts,
}

impl Fixture {
    fn new(simulation: ReclaimSimulation) -> Self {
        let body = base_wearables::default_bodyshape_urn();
        let mut entries = vec![(body.clone(), WearableCategory::BODY_SHAPE)];
        entries.extend(
            base_wearables::default_wearables(&body)
                .zip([
                    WearableCategory::EYES,
                    WearableCategory::EYEBROWS,
                    WearableCategory::MOUTH,
                ])
                .map(|(instance, category)| (instance.base().clone(), category)),
        );
        entries.extend([
            (urn("hat"), WearableCategory::HAT),
            (urn("other_hat"), WearableCategory::HAT),
            (urn("hair"), WearableCategory::HAIR),
        ]);
        let dir = Dir::default();
        let mut pointers = Vec::new();
        let mut wearables = HashMap::new();
        let mut paths = HashMap::new();
        for (index, (urn, category)) in entries.into_iter().enumerate() {
            let hash = format!("wearable-{index}");
            let path = PathBuf::from(&IpfsPath::new(IpfsType::new_content_file(
                hash.clone(),
                "collectible.wearable".to_owned(),
            )));
            dir.insert_asset_text(&path, "fixture");
            paths.insert(urn.to_string(), path.clone());
            pointers.push((urn.clone(), hash));
            wearables.insert(
                path,
                (
                    urn,
                    Wearable {
                        category,
                        hides: if category == WearableCategory::HAT {
                            HashSet::from_iter([WearableCategory::HAIR])
                        } else {
                            HashSet::default()
                        },
                        model: None,
                        texture: None,
                        mask: None,
                    },
                ),
            );
        }
        let loads = LoadCounts::default();
        let source = dir.clone();
        let mut app = App::new();
        app.register_asset_source(
            AssetSourceId::Default,
            AssetSource::build().with_reader(move || {
                Box::new(MemoryAssetReader {
                    root: source.clone(),
                })
            }),
        )
        .add_plugins((MinimalPlugins, AssetPlugin::default(), MemoryReclaimPlugin))
        .register_asset_loader(WearableFixtureLoader {
            wearables,
            loads: loads.clone(),
        })
        .init_resource::<MonotonicTimestamp<PbAssetLoadLoadingState>>()
        .init_resource::<AppConfig>()
        .insert_resource(MemoryReclaimSettings {
            frame_budget: Duration::from_secs(1),
            simulation,
        })
        .insert_resource(IpfsResource {
            inner: Arc::new(IpfsIo::new(
                false,
                Box::new(MemoryAssetReader { root: dir }),
                None,
                Default::default(),
                1,
                tokio::sync::mpsc::unbounded_channel().0,
            )),
        })
        .add_systems(Update, update_render_avatar);
        collectibles::test_support::wearable_cache(&mut app, pointers);
        app.finish();
        app.cleanup();
        Self { app, paths, loads }
    }

    fn spawn(&mut self, names: &[&str]) -> Entity {
        let avatar = self
            .app
            .world_mut()
            .spawn(AvatarSelection {
                scene: None,
                shape: AvatarShape(PbAvatarShape {
                    body_shape: Some(base_wearables::default_bodyshape_urn().to_string()),
                    wearables: names.iter().map(|name| urn(name).to_string()).collect(),
                    ..Default::default()
                }),
                automatic_delete: true,
                disable_dither: false,
            })
            .id();
        self.app
            .world_mut()
            .run_system_once(move |mut commands: Commands| {
                let points = AttachPoints::new(&mut commands);
                commands.entity(avatar).insert(points);
            })
            .unwrap();
        avatar
    }

    fn definition(&self, avatar: Entity) -> Option<Entity> {
        self.app
            .world()
            .get::<Children>(avatar)?
            .iter()
            .find(|child| self.app.world().get::<AvatarDefinition>(*child).is_some())
    }

    fn wait_for_definition(&mut self, avatar: Entity) -> Entity {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            self.app.update();
            if let Some(definition) = self.definition(avatar) {
                return definition;
            }
            assert!(Instant::now() < deadline, "avatar definition did not load");
            std::thread::sleep(Duration::from_millis(1));
        }
    }

    fn expire(&mut self) {
        // Includes the idle grace and Bevy's following asset tracking pass.
        for _ in 0..10 {
            self.app.update();
        }
    }

    fn id(&self, urn: &WearableUrn) -> Option<AssetId<Collectible<Wearable>>> {
        self.app
            .world()
            .resource::<AssetServer>()
            .get_handle::<Collectible<Wearable>>(self.paths[urn.as_str()].clone())
            .map(|handle| handle.id())
    }

    fn assert_loaded_once(&self) {
        assert!(
            self.loads.lock().unwrap().values().all(|count| *count == 1),
            "active aggregates must be reused without reparsing"
        );
    }
}

fn urn(name: &str) -> WearableUrn {
    WearableUrn::new(&format!("urn:decentraland:off-chain:base-avatars:{name}")).unwrap()
}

#[test]
fn active_avatar_aggregates_survive_idle_and_pressure_cleanup_without_reload() {
    for simulation in [
        ReclaimSimulation::Off,
        ReclaimSimulation::Ram,
        ReclaimSimulation::Gpu,
    ] {
        let mut fixture = Fixture::new(simulation);
        let avatar = fixture.spawn(&["hat"]);
        let definition = fixture.wait_for_definition(avatar);
        let expected = std::iter::once(base_wearables::default_bodyshape_urn())
            .chain(
                base_wearables::default_wearables(&base_wearables::default_bodyshape_urn())
                    .map(|instance| instance.base().clone()),
            )
            .chain([urn("hat")])
            .map(|urn| {
                let id = fixture.id(&urn).unwrap();
                (urn, id)
            })
            .collect::<Vec<_>>();
        fixture.expire();
        for (urn, id) in &expected {
            assert_eq!(
                fixture.id(urn),
                Some(*id),
                "active aggregate {urn} was unloaded under {simulation:?}"
            );
            assert!(fixture
                .app
                .world()
                .resource::<Assets<Collectible<Wearable>>>()
                .contains(*id));
        }
        fixture
            .app
            .world_mut()
            .entity_mut(avatar)
            .insert(RetryRenderAvatar);
        fixture.app.update();
        assert_eq!(
            fixture.definition(avatar),
            Some(definition),
            "unchanged definition must keep its entity"
        );
        let second = fixture.spawn(&["hat"]);
        fixture.wait_for_definition(second);
        fixture.assert_loaded_once();

        fixture
            .app
            .world_mut()
            .entity_mut(avatar)
            .remove::<AvatarSelection>();
        fixture.expire();
        for (urn, id) in &expected {
            assert_eq!(fixture.id(urn), Some(*id));
        }
        fixture
            .app
            .world_mut()
            .entity_mut(second)
            .remove::<AvatarSelection>();
        fixture.expire();
        for (urn, _) in expected {
            assert_eq!(fixture.id(&urn), None, "unowned aggregate must unload");
        }
    }
}

#[test]
fn hidden_and_superseded_wearables_expire_and_previous_definition_owns_outfit_until_despawn() {
    let mut fixture = Fixture::new(ReclaimSimulation::Ram);
    let avatar = fixture.spawn(&["hat", "other_hat", "hair"]);
    let old_definition = fixture.wait_for_definition(avatar);
    let hat_id = fixture.id(&urn("hat")).unwrap();
    fixture.expire();
    assert_eq!(
        fixture.id(&urn("other_hat")),
        None,
        "same-category loser is not worn"
    );
    assert_eq!(
        fixture.id(&urn("hair")),
        None,
        "hidden category is not rendered"
    );
    assert_eq!(fixture.id(&urn("hat")), Some(hat_id));

    fixture
        .app
        .world_mut()
        .get_mut::<AvatarSelection>(avatar)
        .unwrap()
        .shape
        .0
        .wearables
        .clear();
    fixture.app.update();
    assert_eq!(
        fixture.app.world().get::<PreviousAvatar>(avatar).unwrap().0,
        old_definition
    );
    fixture.expire();
    assert_eq!(
        fixture.id(&urn("hat")),
        Some(hat_id),
        "previous outfit stays alive during replacement"
    );
    fixture.app.world_mut().despawn(old_definition);
    fixture.expire();
    assert_eq!(
        fixture.id(&urn("hat")),
        None,
        "replaced definition releases its outfit"
    );
    assert!(fixture
        .id(&base_wearables::default_bodyshape_urn())
        .is_some());
    fixture.assert_loaded_once();
}
