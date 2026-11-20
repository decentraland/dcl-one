use std::sync::Arc;

use bevy::{
    asset::io::memory::{Dir, MemoryAssetReader},
    ecs::system::RunSystemOnce,
    scene::ScenePlugin,
};
use collectibles::{Collectible, CollectibleData, Collectibles, Emote};
use ipfs::partition::PartitionedEmoteDownloads;
use ipfs::{IpfsIo, IpfsResource};

use super::*;

/// The resources `update_render_avatar` reads, with nothing resolved and one request slot.
fn app_with_render_avatar() -> App {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, AssetPlugin::default()))
        .init_asset::<Collectible<Wearable>>()
        .init_asset::<CollectibleData<Wearable>>()
        .init_resource::<Collectibles<Wearable>>()
        .add_event::<AvatarLoadReport>()
        .init_resource::<AppConfig>()
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
    app
}

#[test]
fn a_removed_selection_releases_the_avatars_downloads() {
    let mut app = app_with_render_avatar();
    let world = app.world_mut();
    // a scene avatar, its outfit registered under its wearables' hashes
    let avatar = world
        .spawn((
            AvatarSelection {
                scene: None,
                shape: AvatarShape(PbAvatarShape::default()),
                automatic_delete: true,
                disable_dither: false,
            },
            PartitionedDownloads(vec!["hat".to_owned()]),
        ))
        .id();
    assert!(world
        .resource::<IpfsResource>()
        .scene_priority("hat")
        .is_some());

    // the scene removed the entity's AvatarShape, and select_avatar dropped the orphaned
    // selection; the entity itself stays
    world.entity_mut(avatar).remove::<AvatarSelection>();
    world.run_system_once(update_render_avatar).unwrap();

    assert!(
        world.get::<PartitionedDownloads>(avatar).is_none(),
        "the entity keeps nothing of the outfit"
    );
    assert_eq!(
        world.resource::<IpfsResource>().scene_priority("hat"),
        None,
        "the outfit's hashes are no longer registered"
    );
}

/// The resources `file_emote_downloads` reads, with no emote pointer resolved.
fn app_with_emote_downloads() -> App {
    let mut app = app_with_render_avatar();
    app.init_asset::<Collectible<Emote>>()
        .init_asset::<CollectibleData<Emote>>()
        .init_resource::<Collectibles<Emote>>();
    app
}

#[test]
fn emote_downloads_are_filed_under_the_collectible_emotes_an_avatar_plays() {
    use crate::animate::emote_download_hashes;

    let dance = EmoteUrn::new("urn:decentraland:matic:collections-v2:0xabc:0").unwrap();
    let clap = EmoteUrn::new("urn:decentraland:matic:collections-v2:0xabc:1").unwrap();
    let unresolved = EmoteUrn::new("urn:decentraland:matic:collections-v2:0xabc:2").unwrap();
    let scene_emote =
        EmoteUrn::new("urn:decentraland:off-chain:scene-emote:bafyscene-bafyfile-false").unwrap();
    let mut asked = Vec::new();

    // the same emote in both slots is listed once; one whose pointer has not resolved yet is
    // listed when it has
    let hashes = emote_download_hashes([&dance, &clap, &dance, &unresolved, &scene_emote], |urn| {
        asked.push(urn.as_str().to_owned());
        match urn.as_str() {
            urn if urn == dance.as_str() => Some("bafydance".to_owned()),
            urn if urn == clap.as_str() => Some("bafyclap".to_owned()),
            _ => None,
        }
    });

    assert_eq!(hashes, vec!["bafyclap".to_owned(), "bafydance".to_owned()]);
    assert!(
        !asked.iter().any(|urn| urn == scene_emote.as_str()),
        "a scene emote is no collectible: its pointer is never asked for"
    );
}

#[test]
fn an_avatar_that_stops_its_emote_releases_the_emotes_downloads() {
    use crate::animate::{file_emote_downloads, ActiveEmote, AvatarAnimPlayer};

    let mut app = app_with_emote_downloads();
    let world = app.world_mut();
    // an avatar back on its idle pose, with the emote it just played still filed
    let avatar = world
        .spawn((
            ActiveEmote::default(),
            PartitionedEmoteDownloads(vec!["bafydance".to_owned()]),
        ))
        .id();
    world.entity_mut(avatar).insert(AvatarAnimPlayer(avatar));
    assert!(world
        .resource::<IpfsResource>()
        .scene_priority("bafydance")
        .is_some());

    world.run_system_once(file_emote_downloads).unwrap();

    assert!(world.get::<PartitionedEmoteDownloads>(avatar).is_none());
    assert_eq!(
        world.resource::<IpfsResource>().scene_priority("bafydance"),
        None,
        "the emote's hash has no owner once nothing plays it"
    );
}

fn urn(urn: &str) -> WearableUrn {
    WearableUrn::new(urn).unwrap()
}

/// A body that is not the default one.
fn other_body() -> WearableUrn {
    let male = urn("urn:decentraland:off-chain:base-avatars:BaseMale");
    let female = urn("urn:decentraland:off-chain:base-avatars:BaseFemale");
    if male == base_wearables::default_bodyshape_urn() {
        female
    } else {
        male
    }
}

#[test]
fn an_avatar_keeps_its_body_until_the_body_fails() {
    let body = other_body();
    assert_eq!(
        AvatarLoadFailures::body(None, body.clone()),
        Some(body.clone())
    );
    // another body failing does not move this one
    let default_failed = AvatarLoadFailures {
        bodies: HashSet::from([base_wearables::default_bodyshape_urn()]),
        ..Default::default()
    };
    assert_eq!(
        AvatarLoadFailures::body(Some(&default_failed), body.clone()),
        Some(body)
    );
}

#[test]
fn a_failed_body_falls_back_to_the_default_body() {
    let body = other_body();
    let failures = AvatarLoadFailures {
        bodies: HashSet::from([body.clone()]),
        ..Default::default()
    };
    assert_eq!(
        AvatarLoadFailures::body(Some(&failures), body),
        Some(base_wearables::default_bodyshape_urn())
    );
}

#[test]
fn an_avatar_whose_body_and_default_body_failed_has_no_body_to_build() {
    let default = base_wearables::default_bodyshape_urn();
    let body = other_body();
    let failures = AvatarLoadFailures {
        bodies: HashSet::from([body.clone(), default.clone()]),
        ..Default::default()
    };
    assert_eq!(AvatarLoadFailures::body(Some(&failures), body), None);
    // the default body failing alone ends the fallback too
    let failures = AvatarLoadFailures {
        bodies: HashSet::from([default.clone()]),
        ..Default::default()
    };
    assert_eq!(AvatarLoadFailures::body(Some(&failures), default), None);
}

#[test]
fn only_the_wearables_that_failed_are_left_out() {
    let hat = urn("urn:decentraland:off-chain:base-avatars:cap");
    let shirt = urn("urn:decentraland:off-chain:base-avatars:f_sweater");
    let failures = AvatarLoadFailures {
        wearables: HashSet::from([hat.clone()]),
        ..Default::default()
    };
    assert!(AvatarLoadFailures::wearable_failed(Some(&failures), &hat));
    assert!(!AvatarLoadFailures::wearable_failed(
        Some(&failures),
        &shirt
    ));
    assert!(!AvatarLoadFailures::wearable_failed(None, &hat));
}

#[test]
fn a_bone_is_scaled_below_its_avatar_whatever_scales_the_avatar() {
    let mut world = World::new();
    // a scene holds the avatar at a scale of nothing while its body is built
    let scene_entity = world.spawn(Transform::from_scale(Vec3::ZERO)).id();
    let avatar = world
        .spawn((Transform::default(), ChildOf(scene_entity)))
        .id();
    let armature = world
        .spawn((Transform::from_scale(Vec3::splat(0.01)), ChildOf(avatar)))
        .id();
    let head = world
        .spawn((Transform::from_scale(Vec3::splat(150.0)), ChildOf(armature)))
        .id();
    let stray = world.spawn(Transform::default()).id();

    let (head_scale, stray_scale) = world
        .run_system_once(move |transforms: Query<(&Transform, &ChildOf)>| {
            (
                scale_below(head, avatar, &transforms),
                scale_below(stray, avatar, &transforms),
            )
        })
        .unwrap();
    assert!((head_scale.unwrap() - 1.5).abs() < 1e-6, "{head_scale:?}");
    assert_eq!(stray_scale, None);
}

#[test]
fn a_wearables_own_branch_moves_under_the_body_bone_it_hangs_off() {
    let mut world = World::new();
    let body_hips = world.spawn(Name::new("Avatar_Hips")).id();
    let scene = world.spawn(Name::new("Scene")).id();
    let armature = world.spawn((Name::new("Armature"), ChildOf(scene))).id();
    let mut node =
        |name: &'static str, parent: Entity| world.spawn((Name::new(name), ChildOf(parent))).id();
    let hips = node("Avatar_Hips", armature);
    // a bone straight under a body bone, and one below it
    let tail_01 = node("Tail_01", hips);
    let tail_02 = node("Tail_02", tail_01);
    // bones under a node that is no joint
    let holder = node("Holder", hips);
    let cape_01 = node("Cape_01", holder);
    // bones under an `avatar_*` bone the body lacks, below a body bone and below the armature
    let wing = node("Avatar_Wing", hips);
    let wing_01 = node("Wing_01", wing);
    let prop = node("Avatar_Prop", armature);
    let prop_01 = node("Prop_01", prop);
    // a node of the wearable's own under the armature, which moves to the body's by itself
    let loose = node("Loose", armature);
    let loose_01 = node("Loose_01", loose);
    let stray = world.spawn(Name::new("Stray")).id();

    let body_bones = HashMap::from_iter([("avatar_hips".to_owned(), body_hips)]);
    let joints = [tail_02, cape_01, wing_01, prop_01, loose_01, stray];
    let branches = world
        .run_system_once(
            move |nodes: Query<(&Name, &ChildOf)>, names: Query<&Name>| {
                joints.map(|joint| {
                    wearable_branch(joint, &nodes, &names, &body_bones)
                        .map(|branch| (branch.top, branch.body_bone, branch.body_lacks_top))
                })
            },
        )
        .unwrap();
    assert_eq!(
        branches,
        [
            Some((tail_01, Some(body_hips), false)),
            Some((holder, Some(body_hips), false)),
            Some((wing, Some(body_hips), true)),
            Some((prop, None, true)),
            None,
            None,
        ]
    );
}

#[test]
fn a_body_that_exhausted_its_fallback_reports_failure_to_the_renderer_host() {
    let mut app = app_with_render_avatar();
    let world = app.world_mut();
    let body = other_body();
    let owner = world
        .spawn((
            AvatarSelection {
                scene: None,
                shape: AvatarShape(PbAvatarShape {
                    body_shape: Some(body.as_str().to_owned()),
                    ..Default::default()
                }),
                automatic_delete: true,
                disable_dither: false,
            },
            AvatarLoadFailures {
                bodies: HashSet::from_iter([body.clone(), base_wearables::default_bodyshape_urn()]),
                ..Default::default()
            },
        ))
        .id();

    world.run_system_once(update_render_avatar).unwrap();

    let reports: Vec<_> = world
        .resource_mut::<Events<AvatarLoadReport>>()
        .drain()
        .collect();
    assert_eq!(reports.len(), 1);
    assert_eq!(reports[0].owner, owner);
    assert_eq!(reports[0].body_shape, body.as_str());
    assert_eq!(reports[0].state, LoadingState::FinishedWithError);
    assert!(world.get::<RetryRenderAvatar>(owner).is_none());
}

#[test]
fn a_build_waits_for_the_face_textures_and_masks() {
    use bevy::asset::{uuid::Uuid, LoadState};
    let image = |id: u128| {
        Handle::<Image>::Weak(AssetId::Uuid {
            uuid: Uuid::from_u128(id),
        })
    };
    let face = |texture: u128, mask: Option<u128>| Wearable {
        category: WearableCategory::MOUTH,
        hides: HashSet::default(),
        model: None,
        texture: Some(image(texture)),
        mask: mask.map(image),
    };
    let loading = |ids: &'static [u128]| {
        move |handle: &Handle<Image>| {
            if ids.iter().any(|id| handle.id() == image(*id).id()) {
                Some(LoadState::Loading)
            } else {
                Some(LoadState::Loaded)
            }
        }
    };
    let worn = [face(1, Some(2)), face(3, None)];
    assert!(!face_images_loading(&worn, loading(&[])));
    // a texture or a mask still on its way holds the build
    assert!(face_images_loading(&worn, loading(&[3])));
    assert!(face_images_loading(&worn, loading(&[2])));
    // a failed image does not: the face is built without it
    assert!(!face_images_loading(&worn, |_: &Handle<Image>| Some(
        LoadState::Failed(Arc::new(
            bevy::asset::AssetLoadError::RequestedHandleTypeMismatch {
                path: "face.png".into(),
                requested: std::any::TypeId::of::<Image>(),
                actual_asset_name: "",
                loader_name: "",
            }
        ))
    )));
}

#[test]
fn a_body_failure_waits_for_its_fallback_before_reporting_a_terminal_error() {
    let mut app = app_with_render_avatar();
    let world = app.world_mut();
    let body = other_body();
    let failed_body = body.clone();
    world
        .run_system_once(move |mut wearables: CollectibleManager<Wearable>| {
            wearables.add_builtin(
                failed_body.clone(),
                Collectible {
                    representations: HashMap::default(),
                    data: CollectibleData {
                        hash: "fake-body-hash".to_owned(),
                        urn: failed_body.as_str().to_owned(),
                        thumbnail: String::new(),
                        available_representations: HashSet::default(),
                        name: String::new(),
                        description: String::new(),
                        extra_data: collectibles::wearables::WearableExtraData {
                            category: WearableCategory::BODY_SHAPE,
                            scene_json_files: Vec::new(),
                        },
                    },
                },
            );
        })
        .unwrap();
    let owner = world
        .spawn(AvatarSelection {
            scene: None,
            shape: AvatarShape(PbAvatarShape {
                body_shape: Some(body.as_str().to_owned()),
                ..Default::default()
            }),
            automatic_delete: true,
            disable_dither: false,
        })
        .id();

    world.run_system_once(update_render_avatar).unwrap();

    assert!(world.resource::<Events<AvatarLoadReport>>().is_empty());
    assert!(world.get::<RetryRenderAvatar>(owner).is_some());
    assert_eq!(
        AvatarLoadFailures::body(world.get::<AvatarLoadFailures>(owner), body),
        Some(base_wearables::default_bodyshape_urn()),
    );
}

#[derive(Clone, Copy, Debug)]
enum BodyGltfFailure {
    FailedLoad,
    MissingAsset,
    NoDefaultScene,
}

#[derive(Default)]
struct TestGltfLoader;

impl bevy::asset::AssetLoader for TestGltfLoader {
    type Asset = Gltf;
    type Settings = ();
    type Error = std::io::Error;

    async fn load(
        &self,
        _reader: &mut dyn bevy::asset::io::Reader,
        _settings: &(),
        _context: &mut bevy::asset::LoadContext<'_>,
    ) -> Result<Gltf, Self::Error> {
        Err(std::io::Error::other("fake invalid GLTF"))
    }

    fn extensions(&self) -> &[&str] {
        &["gltf"]
    }
}

fn gltf_with_scene(default_scene: Option<Handle<Scene>>) -> Gltf {
    Gltf {
        scenes: default_scene.iter().cloned().collect(),
        named_scenes: default(),
        meshes: default(),
        named_meshes: default(),
        materials: default(),
        named_materials: default(),
        nodes: default(),
        named_nodes: default(),
        skins: default(),
        named_skins: default(),
        default_scene,
        animations: default(),
        named_animations: default(),
        source: None,
    }
}

fn app_with_avatar_scenes() -> App {
    let mut app = app_with_emote_downloads();
    app.add_plugins((TransformPlugin, ScenePlugin))
        .register_type::<Name>()
        .register_type::<ChildOf>()
        .register_type::<Children>()
        .register_type::<Visibility>()
        .register_type::<InheritedVisibility>()
        .register_type::<ViewVisibility>()
        .init_asset::<Gltf>()
        .register_asset_loader(TestGltfLoader)
        .init_asset::<StandardMaterial>()
        .init_asset::<SceneMaterial>()
        .init_asset::<MaskMaterial>()
        .init_asset::<Mesh>()
        .init_asset::<SkinnedMeshInverseBindposes>()
        .init_asset::<AnimationGraph>()
        .init_resource::<AssetCache<AvatarMatKey, SceneMaterial>>()
        .init_resource::<AssetCache<AvatarMaskKey, MaskMaterial>>();
    app.finish();
    app.cleanup();
    app
}

fn broken_body_gltf(app: &mut App, failure: BodyGltfFailure) -> Handle<Gltf> {
    match failure {
        BodyGltfFailure::FailedLoad => {
            let handle = app
                .world()
                .resource::<AssetServer>()
                .load::<Gltf>("fake-missing-body.gltf");
            let deadline = std::time::Instant::now() + Duration::from_secs(5);
            loop {
                app.update();
                if matches!(
                    app.world()
                        .resource::<AssetServer>()
                        .get_load_state(&handle),
                    Some(bevy::asset::LoadState::Failed(_))
                ) {
                    return handle;
                }
                assert!(
                    std::time::Instant::now() < deadline,
                    "the missing GLTF never failed"
                );
                std::thread::yield_now();
            }
        }
        BodyGltfFailure::MissingAsset => app.world().resource::<Assets<Gltf>>().reserve_handle(),
        BodyGltfFailure::NoDefaultScene => app
            .world_mut()
            .resource_mut::<Assets<Gltf>>()
            .add(gltf_with_scene(None)),
    }
}

fn usable_body_gltf(world: &mut World) -> Handle<Gltf> {
    let mut scene = World::new();
    let armature = scene
        .spawn((
            Name::new("Armature"),
            Transform::default(),
            Visibility::default(),
        ))
        .id();
    scene.spawn((
        Name::new("Avatar_Hips"),
        Transform::default(),
        Visibility::default(),
        ChildOf(armature),
    ));
    let scene = world.resource_mut::<Assets<Scene>>().add(Scene::new(scene));
    world
        .resource_mut::<Assets<Gltf>>()
        .add(gltf_with_scene(Some(scene)))
}

fn register_test_body(world: &mut World, urn: WearableUrn, gltf: Handle<Gltf>) {
    world
        .run_system_once(move |mut wearables: CollectibleManager<Wearable>| {
            // The base body supplies all geometry; no default garment needs to resolve in this fixture.
            for default in base_wearables::default_wearables(&urn) {
                let name = default.base().clone();
                wearables.add_builtin(
                    name.clone(),
                    Collectible {
                        representations: HashMap::default(),
                        data: test_wearable_data(&name),
                    },
                );
            }
            wearables.add_builtin(
                urn.clone(),
                Collectible {
                    representations: HashMap::from_iter([(
                        urn.as_str().to_owned(),
                        Wearable {
                            category: WearableCategory::BODY_SHAPE,
                            hides: HashSet::default(),
                            model: Some(WearableModel {
                                gltf: gltf.clone(),
                                outline_compatible: true,
                                spring_bones: None,
                            }),
                            texture: None,
                            mask: None,
                        },
                    )]),
                    data: test_wearable_data(&urn),
                },
            );
        })
        .unwrap();
}

fn test_wearable_data(urn: &WearableUrn) -> CollectibleData<Wearable> {
    CollectibleData {
        hash: "fake-body-hash".to_owned(),
        urn: urn.as_str().to_owned(),
        thumbnail: String::new(),
        available_representations: HashSet::default(),
        name: String::new(),
        description: String::new(),
        extra_data: collectibles::wearables::WearableExtraData {
            category: WearableCategory::BODY_SHAPE,
            scene_json_files: Vec::new(),
        },
    }
}

fn avatar_build(world: &mut World, owner: Entity) -> Entity {
    world
        .query::<(Entity, &AvatarDefinition, &ChildOf)>()
        .iter(world)
        .find(|(_, _, parent)| parent.parent() == owner)
        .map(|(entity, _, _)| entity)
        .expect("the avatar must have a pending build")
}

#[cfg(feature = "explorer")]
#[test]
fn removing_a_scene_avatar_shape_despawns_the_render_child() {
    use scene_runner::initialize_scene::{LiveScenes, PortableScenes, ScenePointers};

    let mut app = app_with_avatar_scenes();
    app.add_observer(add_attach_points_to_avatar_shape)
        .add_observer(remove_attach_points_from_avatar_shape)
        .init_resource::<LiveScenes>()
        .init_resource::<PortableScenes>()
        .init_resource::<ScenePointers>();
    let world = app.world_mut();
    let body = usable_body_gltf(world);
    register_test_body(world, base_wearables::default_bodyshape_urn(), body);

    let scene = world.spawn_empty().id();
    let owner = world
        .spawn((
            AvatarShape(PbAvatarShape::default()),
            SceneEntity {
                root: scene,
                scene_id: dcl_types::SceneId(scene),
                id: SceneEntityId {
                    id: 42,
                    generation: 0,
                },
            },
        ))
        .id();
    let other_child = world.spawn(ChildOf(owner)).id();
    world.run_system_once(select_avatar).unwrap();
    world.run_system_once(update_render_avatar).unwrap();
    let render = avatar_build(world, owner);

    world.entity_mut(owner).remove::<AvatarShape>();
    world.run_system_once(select_avatar).unwrap();
    assert!(world.get::<AttachPoints>(owner).is_none());
    assert!(world.get::<AvatarSelection>(owner).is_none());

    world.run_system_once(update_render_avatar).unwrap();

    assert!(world.get_entity(render).is_err());
    assert!(world.get_entity(other_child).is_ok());
}

fn take_load_reports(
    world: &World,
    cursor: &mut bevy::ecs::event::EventCursor<AvatarLoadReport>,
) -> Vec<AvatarLoadReport> {
    cursor
        .read(world.resource::<Events<AvatarLoadReport>>())
        .cloned()
        .collect()
}

fn exercise_gltf_fallback(
    failure: BodyGltfFailure,
    exhaust_fallback: bool,
    scene_owned: bool,
) -> (App, Entity, Vec<AvatarLoadReport>) {
    let mut app = app_with_avatar_scenes();
    let failed = broken_body_gltf(&mut app, failure);
    let requested = other_body();
    let default_body = base_wearables::default_bodyshape_urn();
    register_test_body(app.world_mut(), requested.clone(), failed.clone());
    let fallback = if exhaust_fallback {
        failed
    } else {
        usable_body_gltf(app.world_mut())
    };
    register_test_body(app.world_mut(), default_body.clone(), fallback);
    let owner = app
        .world_mut()
        .spawn((
            Transform::default(),
            AvatarSelection {
                scene: None,
                shape: AvatarShape(PbAvatarShape {
                    body_shape: Some(requested.as_str().to_owned()),
                    ..default()
                }),
                automatic_delete: true,
                disable_dither: false,
            },
        ))
        .id();
    #[cfg(feature = "explorer")]
    if scene_owned {
        use dcl_types::SceneId;
        let scene = app.world_mut().spawn_empty().id();
        let context = RendererSceneContext::new(
            SceneId(scene),
            "fake-scene".to_owned(),
            String::new(),
            false,
            0,
            String::new(),
            IVec2::ZERO,
            default(),
            Vec::new(),
            default(),
            Vec::new(),
            scene,
            0.0,
            false,
            "",
            false,
            false,
        );
        app.world_mut().entity_mut(scene).insert(context);
        app.world_mut().entity_mut(owner).insert(SceneEntity {
            root: scene,
            scene_id: SceneId(scene),
            id: SceneEntityId {
                id: 42,
                generation: 0,
            },
        });
        app.world_mut()
            .get_mut::<AvatarSelection>(owner)
            .unwrap()
            .scene = Some(scene);
        app.init_resource::<MonotonicTimestamp<PbAssetLoadLoadingState>>();
    }
    #[cfg(not(feature = "explorer"))]
    assert!(!scene_owned);

    let mut cursor = bevy::ecs::event::EventCursor::default();
    app.world_mut()
        .run_system_once(update_render_avatar)
        .unwrap();
    let mut reports = take_load_reports(app.world(), &mut cursor);
    let first_build = avatar_build(app.world_mut(), owner);
    app.world_mut().run_system_once(spawn_scenes).unwrap();
    assert!(app.world().get_entity(first_build).is_err(), "{failure:?}");
    assert!(
        app.world().get::<RetryRenderAvatar>(owner).is_some(),
        "{failure:?}"
    );
    reports.extend(take_load_reports(app.world(), &mut cursor));
    assert!(
        reports
            .iter()
            .all(|report| report.state != LoadingState::FinishedWithError),
        "{failure:?}"
    );

    app.world_mut()
        .run_system_once(update_render_avatar)
        .unwrap();
    let fallback_build = avatar_build(app.world_mut(), owner);
    assert_eq!(
        app.world()
            .get::<AvatarDefinition>(fallback_build)
            .unwrap()
            .render
            .body_shape,
        default_body.as_str()
    );
    app.world_mut().run_system_once(spawn_scenes).unwrap();
    reports.extend(take_load_reports(app.world(), &mut cursor));
    assert!(
        reports
            .iter()
            .all(|report| report.state != LoadingState::FinishedWithError),
        "{failure:?}"
    );

    if exhaust_fallback {
        assert!(app.world().get_entity(fallback_build).is_err());
        assert!(app.world().get::<RetryRenderAvatar>(owner).is_some());
        app.world_mut()
            .run_system_once(update_render_avatar)
            .unwrap();
        reports.extend(take_load_reports(app.world(), &mut cursor));
        let errors: Vec<_> = reports
            .iter()
            .filter(|report| report.state == LoadingState::FinishedWithError)
            .collect();
        assert_eq!(errors.len(), 1, "{failure:?}");
        assert_eq!(errors[0].owner, owner);
        assert_eq!(errors[0].body_shape, requested.as_str());
        assert!(app.world().get::<RetryRenderAvatar>(owner).is_none());
    } else {
        assert!(app.world().get::<AvatarLoaded>(fallback_build).is_some());
        app.update();
        app.world_mut().run_system_once(process_avatar).unwrap();
        reports.extend(take_load_reports(app.world(), &mut cursor));
        assert!(
            app.world().get::<AvatarProcessed>(fallback_build).is_some(),
            "{failure:?}"
        );
        assert!(app
            .world()
            .get::<animate::AvatarAnimPlayer>(owner)
            .is_some());
        assert!(matches!(
            app.world().get::<Visibility>(fallback_build),
            Some(Visibility::Inherited)
        ));
        assert!(reports
            .iter()
            .all(|report| report.state != LoadingState::FinishedWithError));
        assert!(reports.iter().any(|report| report.owner == owner
            && report.body_shape == default_body.as_str()
            && report.state == LoadingState::Finished));
    }
    (app, owner, reports)
}

#[test]
fn a_failed_gltf_can_fall_back_without_terminal_error() {
    exercise_gltf_fallback(BodyGltfFailure::FailedLoad, false, false);
}

#[test]
fn a_missing_gltf_can_fall_back_without_terminal_error() {
    exercise_gltf_fallback(BodyGltfFailure::MissingAsset, false, false);
}

#[test]
fn a_gltf_without_a_default_scene_can_fall_back_without_terminal_error() {
    exercise_gltf_fallback(BodyGltfFailure::NoDefaultScene, false, false);
}

#[test]
fn failed_gltfs_report_terminal_error_only_after_both_bodies_fail() {
    exercise_gltf_fallback(BodyGltfFailure::FailedLoad, true, false);
}

#[test]
fn missing_gltfs_report_terminal_error_only_after_both_bodies_fail() {
    exercise_gltf_fallback(BodyGltfFailure::MissingAsset, true, false);
}

#[test]
fn gltfs_without_default_scenes_report_terminal_error_only_after_both_bodies_fail() {
    exercise_gltf_fallback(BodyGltfFailure::NoDefaultScene, true, false);
}

#[cfg(feature = "explorer")]
#[test]
fn an_exhausted_gltf_fallback_reports_terminal_error_to_the_scene_crdt() {
    use dcl_component::DclReader;
    let (mut app, owner, _) = exercise_gltf_fallback(BodyGltfFailure::NoDefaultScene, true, true);
    app.world_mut()
        .run_system_once(apply_avatar_load_reports)
        .unwrap();
    let scene_ent = app.world().get::<SceneEntity>(owner).unwrap();
    let context = app
        .world()
        .get::<RendererSceneContext>(scene_ent.root)
        .unwrap();
    let entries = &context.crdt_store.go[&SceneComponentId::ASSET_LOAD_LOADING_STATE].0;
    assert_eq!(entries.len(), 1);
    let reports: Vec<PbAssetLoadLoadingState> = entries[&scene_ent.id]
        .iter()
        .map(|entry| DclReader::new(&entry.data).read().unwrap())
        .collect();
    let errors: Vec<_> = reports
        .iter()
        .filter(|report| report.current_state == LoadingState::FinishedWithError as i32)
        .collect();
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].asset, other_body().as_str());
    assert!(errors[0].timestamp > 0);
}

fn render_definition(avatar_outline: bool, disable_dither: bool) -> AvatarRenderDefinition {
    AvatarRenderDefinition {
        body: Wearable {
            category: WearableCategory::BODY_SHAPE,
            hides: default(),
            model: Some(WearableModel {
                gltf: default(),
                outline_compatible: true,
                spring_bones: None,
            }),
            texture: None,
            mask: None,
        },
        body_shape: String::new(),
        skin_color: Color::WHITE,
        hair_color: Color::WHITE,
        eyes_color: Color::WHITE,
        wearables: Vec::new(),
        hides: default(),
        bounds: SceneBounds::unbounded(),
        disable_dither,
        avatar_outline,
    }
}

/// the face masks carry the body's tag, so they are outlined and faded by the camera dither
/// exactly when the body is, plus the face-feature tag that lights them by the ambient alone
#[test]
fn mask_tag_matches_the_body() {
    let dither = SCENE_MATERIAL_CONE_ONLY_DITHER_MESH_TAG;
    let face = SCENE_MATERIAL_FACIAL_FEATURE_MESH_TAG;
    let outlined = render_definition(true, false);
    assert_eq!(
        outlined.mask_tag(),
        SCENE_MATERIAL_TOON_MESH_TAG | SCENE_MATERIAL_OUTLINE_MESH_TAG | dither | face
    );
    assert_eq!(outlined.mask_tag(), outlined.body_tag() | face);

    let plain = render_definition(false, false);
    assert_eq!(
        plain.mask_tag(),
        SCENE_MATERIAL_TOON_MESH_TAG | dither | face
    );
    assert_eq!(plain.mask_tag(), plain.body_tag() | face);

    let undithered = render_definition(true, true);
    assert_eq!(
        undithered.mask_tag(),
        SCENE_MATERIAL_TOON_MESH_TAG
            | SCENE_MATERIAL_OUTLINE_MESH_TAG
            | SCENE_MATERIAL_NO_DITHERING_MESH_TAG
            | dither
            | face
    );
    assert_eq!(undithered.mask_tag(), undithered.body_tag() | face);

    // the body itself is not a face feature
    assert_eq!(outlined.body_tag() & face, 0);
}

/// a wearable opts out of the hover glow with `outlineCompatible: false`, with the outline
/// setting on or off, and takes no outline tag either
#[test]
fn a_wearable_opts_out_of_the_hover_glow() {
    let model = |outline_compatible| WearableModel {
        gltf: default(),
        outline_compatible,
        spring_bones: None,
    };
    for avatar_outline in [true, false] {
        let def = render_definition(avatar_outline, false);
        assert!(hover_glows(def.body.model.as_ref()));
        assert!(hover_glows(Some(&model(true))));
        assert!(!hover_glows(Some(&model(false))));
        assert_eq!(
            def.model_tag(Some(&model(false))) & SCENE_MATERIAL_OUTLINE_MESH_TAG,
            0
        );
    }
}
