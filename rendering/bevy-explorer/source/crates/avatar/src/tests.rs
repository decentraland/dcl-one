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
