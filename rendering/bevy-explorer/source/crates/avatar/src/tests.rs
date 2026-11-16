use std::sync::Arc;

use bevy::{
    asset::io::memory::{Dir, MemoryAssetReader},
    ecs::system::RunSystemOnce,
};
use collectibles::{Collectible, CollectibleData, Collectibles, Emote};
use ipfs::{IpfsIo, IpfsResource};
use scene_runner::scene_partition::PartitionedEmoteDownloads;

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
