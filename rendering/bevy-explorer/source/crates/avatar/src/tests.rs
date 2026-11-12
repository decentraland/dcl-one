use std::sync::Arc;

use bevy::{
    asset::io::memory::{Dir, MemoryAssetReader},
    ecs::system::RunSystemOnce,
};
use collectibles::{Collectible, CollectibleData, Collectibles};
use ipfs::{IpfsIo, IpfsResource};

use super::*;

/// The resources `update_render_avatar` reads, with nothing resolved and one request slot.
fn app_with_render_avatar() -> App {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, AssetPlugin::default()))
        .init_asset::<Collectible<Wearable>>()
        .init_asset::<CollectibleData<Wearable>>()
        .init_resource::<Collectibles<Wearable>>()
        .init_resource::<MonotonicTimestamp<PbAssetLoadLoadingState>>()
        .init_resource::<AppConfig>()
        .init_resource::<RefusedAvatars>()
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

fn selection() -> AvatarSelection {
    AvatarSelection {
        scene: None,
        shape: AvatarShape(PbAvatarShape::default()),
        automatic_delete: true,
        disable_dither: false,
    }
}

fn foreign_player(world: &mut World) -> ForeignPlayer {
    let context = world.spawn_empty().id();
    ForeignPlayer::for_test(Default::default(), context, SceneEntityId::PLAYER)
}

/// The scene entity of an avatar a scene places.
fn scene_entity(world: &mut World) -> SceneEntity {
    let root = world.spawn_empty().id();
    SceneEntity {
        root,
        scene_id: dcl_types::SceneId(root),
        id: SceneEntityId {
            id: 512,
            generation: 0,
        },
    }
}

#[test]
fn full_memory_refuses_new_player_and_scene_avatars_until_it_is_lifted() {
    let mut app = app_with_render_avatar();
    let world = app.world_mut();
    world.insert_resource(LoadRefusal { refusing: true });
    let player = foreign_player(world);
    let player_avatar = world.spawn((selection(), player)).id();
    let scene_entity = scene_entity(world);
    let scene_avatar = world.spawn((selection(), scene_entity)).id();

    world.run_system_once(update_render_avatar).unwrap();

    for avatar in [player_avatar, scene_avatar] {
        assert!(
            world.get::<PartitionedDownloads>(avatar).is_none(),
            "nothing of the avatar is filed, so nothing of it is resolved or downloaded"
        );
        assert!(world.get::<RetryRenderAvatar>(avatar).is_some());
    }
    assert_eq!(world.resource::<RefusedAvatars>().0, 2);

    world.insert_resource(LoadRefusal { refusing: false });
    world.run_system_once(update_render_avatar).unwrap();

    for avatar in [player_avatar, scene_avatar] {
        assert!(
            world.get::<PartitionedDownloads>(avatar).is_some(),
            "the avatar starts once the refusal ends, with no change to its selection"
        );
    }
    assert_eq!(world.resource::<RefusedAvatars>().0, 0);
}

#[test]
fn full_memory_keeps_an_avatar_that_had_started_and_admits_the_players_own_and_previews() {
    let mut app = app_with_render_avatar();
    let world = app.world_mut();
    // another player's avatar that started before memory filled, and its outfit then changed
    let player = foreign_player(world);
    let started = world.spawn((selection(), player)).id();
    world.run_system_once(update_render_avatar).unwrap();
    assert!(world.get::<PartitionedDownloads>(started).is_some());
    world.insert_resource(LoadRefusal { refusing: true });
    world
        .get_mut::<AvatarSelection>(started)
        .unwrap()
        .set_changed();
    // the player's own avatar or a preview: neither another player's nor a scene's
    let own = world.spawn(selection()).id();

    world.run_system_once(update_render_avatar).unwrap();

    assert!(world.get::<PartitionedDownloads>(own).is_some());
    assert!(world.get::<PartitionedDownloads>(started).is_some());
    assert_eq!(world.resource::<RefusedAvatars>().0, 0);
}
