use std::sync::Arc;

use bevy::{
    asset::io::memory::{Dir, MemoryAssetReader},
    ecs::system::RunSystemOnce,
    prelude::*,
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
