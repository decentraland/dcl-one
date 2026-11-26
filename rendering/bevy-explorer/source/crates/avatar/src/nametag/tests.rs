use bevy::ecs::system::RunSystemOnce;
use common::{
    profile::SerializedProfile,
    structs::{CameraOverride, CinematicSettings, PlayerModifiers},
};
use comms::BlockedOrBanned;

use dcl_component::{proto_components::common::Color3, SceneEntityId};
use scene_runner::{
    initialize_scene::{LiveScenes, PointerResult, PortableScenes, PortableSource, ScenePointers},
    parcel_to_vec3, ContainingScene, OutOfWorld,
};

use super::*;
use crate::BeyondMaxAvatars;

const ADDRESS: &str = "0x00000000000000000000000000000000000012ab";

fn profile(name: &str, claimed: bool) -> UserProfile {
    UserProfile {
        content: SerializedProfile {
            name: name.to_owned(),
            eth_address: ADDRESS.to_owned(),
            has_claimed_name: claimed,
            has_connected_web3: Some(true),
            ..Default::default()
        },
        ..Default::default()
    }
}

fn content(name: &str) -> NametagContent {
    NametagContent::from_profile(&profile(name, true), &OfficialWallets::default())
}

#[test]
fn official_wallets_parse_from_a_csv_payload() {
    let wallets = parse_wallets(
        " 0x00000000000000000000000000000000000012AB,not an address\r\n\
         00000000000000000000000000000000000034cd ,, 0x12\n",
    );
    assert_eq!(wallets.len(), 2);
    assert!(wallets.contains(&ADDRESS.as_h160().unwrap()));
    assert!(wallets.contains(
        &"0x00000000000000000000000000000000000034cd"
            .as_h160()
            .unwrap()
    ));
    assert!(parse_wallets("").is_empty());
}

#[test]
fn an_unclaimed_name_carries_the_end_of_its_address() {
    let official = OfficialWallets::default();
    let claimed = NametagContent::from_profile(&profile("Ada", true), &official);
    assert_eq!(claimed.name, "Ada");
    assert_eq!(claimed.suffix, "");
    assert!(claimed.verified && !claimed.official);

    let unclaimed = NametagContent::from_profile(&profile("Ada", false), &official);
    assert_eq!(unclaimed.suffix, "#12ab");
    assert!(!unclaimed.verified);
    assert_ne!(unclaimed.color, claimed.color);

    // a tag in the name is dropped for the address' own, whatever it says
    for stored in ["Ada#12ab", "Ada#ffff"] {
        let tagged = NametagContent::from_profile(&profile(stored, false), &official);
        assert_eq!(tagged.name, "Ada", "{stored}");
        assert_eq!(tagged.suffix, "#12ab", "{stored}");
    }
}

#[test]
fn a_guest_claiming_a_name_gets_no_seal() {
    let mut guest = profile("Ada", true);
    guest.content.has_connected_web3 = None;
    let content = NametagContent::from_profile(&guest, &OfficialWallets::default());
    assert!(!content.verified);
    assert_eq!(content.suffix, "#12ab");
}

#[test]
fn an_address_colours_a_name_the_same_in_any_case() {
    let mut checksummed = profile("Ada", false);
    checksummed.content.eth_address = ADDRESS.to_uppercase().replacen("0X", "0x", 1);
    assert_eq!(checksummed.name_color(), profile("Ada", false).name_color());
}

#[test]
fn a_long_name_is_cut() {
    let content = content(&"a".repeat(500));
    assert_eq!(content.name.chars().count(), MAX_NAME_CHARS + 1);
    assert!(content.name.ends_with('…'));
}

#[test]
fn the_official_badge_follows_the_wallet_set() {
    let mut official = OfficialWallets::default();
    official.0.insert(ADDRESS.as_h160().unwrap());
    assert!(NametagContent::from_profile(&profile("Ada", true), &official).official);
}

#[test]
fn a_tag_fades_out_over_the_second_half_of_its_range() {
    assert_eq!(tag_opacity(2.0), 1.0);
    assert_eq!(tag_opacity(20.0), 1.0);
    assert_eq!(tag_opacity(30.0), 0.5);
    assert_eq!(tag_opacity(40.0), 0.0);
    assert_eq!(tag_opacity(55.0), 0.0);
}

#[test]
fn a_tag_keeps_its_size_on_screen() {
    let fov = 60f32.to_radians();
    // a unit's share of the view's height: its world size over the view's height there
    let share =
        |distance: f32, fov: f32| tag_scale(distance, fov) / (2.0 * distance * (fov * 0.5).tan());
    let near = share(3.0, fov);
    assert!((near - 0.00075).abs() < 1e-7);
    assert!((share(39.0, fov) - near).abs() < 1e-7);
    assert!((share(10.0, 100f32.to_radians()) - near).abs() < 1e-7);
}

#[test]
fn a_name_shows_in_range_of_the_camera() {
    let shows = |player| in_range(player, Vec3::ZERO);
    assert!(shows(Vec3::new(0.0, 0.0, -2.0)));
    assert!(shows(Vec3::new(0.0, 0.0, -40.0)));
    assert!(!shows(Vec3::new(0.0, 0.0, -1.9)));
    assert!(!shows(Vec3::new(0.0, 0.0, -40.1)));
    // behind the camera or off to the side: the frustum's to cull
    assert!(shows(Vec3::new(0.0, 0.0, 5.0)));
    assert!(shows(Vec3::new(30.0, 0.0, -1.0)));
}

#[test]
fn the_voice_badge_follows_speech_mutes_and_the_microphone() {
    let nearby = Some(Speaking { in_call: false });
    let call = Some(Speaking { in_call: true });
    assert_eq!(voice_badge(None, false, false, false), Voice::Off);
    assert_eq!(voice_badge(nearby, false, false, false), Voice::Speaking);
    assert_eq!(voice_badge(nearby, true, false, false), Voice::Hushed);
    // a nearby mute leaves a call audible
    assert_eq!(voice_badge(call, true, false, false), Voice::Speaking);
    // muted and silent: nothing to mark
    assert_eq!(voice_badge(None, true, false, false), Voice::Off);
    // the local user's open microphone shows before they speak
    assert_eq!(voice_badge(None, false, true, false), Voice::Idle);
    assert_eq!(voice_badge(nearby, false, true, false), Voice::Speaking);
    // a scene that turns nearby voice off: nearby speakers show nothing, a call's still do
    assert_eq!(voice_badge(nearby, false, false, true), Voice::Off);
    assert_eq!(voice_badge(nearby, true, false, true), Voice::Off);
    assert_eq!(voice_badge(call, false, false, true), Voice::Speaking);
}

fn shape(id: &str, name: Option<&str>) -> PbAvatarShape {
    PbAvatarShape {
        id: id.to_owned(),
        name: name.map(str::to_owned),
        ..Default::default()
    }
}

#[test]
fn a_scene_avatar_shows_its_name_and_the_end_of_its_id() {
    let named = NametagContent::from_shape(&shape("guide-0001", Some("Guide"))).unwrap();
    assert_eq!(named.name, "Guide");
    assert_eq!(named.suffix, "#0001");
    assert!(!named.verified && !named.official);

    // no name given, or a blank one, asks for no tag
    for unnamed in [None, Some(""), Some("  ")] {
        assert!(NametagContent::from_shape(&shape("guide-0001", unnamed)).is_none());
    }

    // a control character in the id stays off the tag
    let broken = NametagContent::from_shape(&shape("guide-0\n01", Some("Guide"))).unwrap();
    assert_eq!(broken.suffix, "#0 01");

    // an id too short to end a name shows zeros, and one of four characters shows itself
    for (id, suffix) in [
        ("", "#0000"),
        ("abc", "#0000"),
        ("abcd", "#abcd"),
        ("né-ñandú", "#andú"),
    ] {
        let content = NametagContent::from_shape(&shape(id, Some("Guide"))).unwrap();
        assert_eq!(content.suffix, suffix, "{id}");
    }

    // the suffix is the id's own, and the name is bounded as a player's is
    let tagged = NametagContent::from_shape(&shape("guide-0001", Some("Guide#ffff"))).unwrap();
    assert_eq!(
        (tagged.name.as_str(), tagged.suffix.as_str()),
        ("Guide", "#0001")
    );
    // what follows a `#` is dropped with the space before it, and a name that starts with
    // one still has a tag
    for (given, name) in [("Room #3", "Room"), ("#1 Fan", "NPC"), (" #", "NPC")] {
        let cut = NametagContent::from_shape(&shape("guide-0001", Some(given))).unwrap();
        assert_eq!(cut.name, name, "{given:?}");
    }
    let long = NametagContent::from_shape(&shape("guide-0001", Some(&"a".repeat(500)))).unwrap();
    assert_eq!(long.name.chars().count(), MAX_NAME_CHARS + 1);
}

fn scene_avatar_app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .init_resource::<MicState>()
        .init_resource::<NearbyMutes>()
        .add_systems(
            Update,
            (resolve_scene_avatar_content, resolve_voice).chain(),
        );
    app
}

fn selection(shape: &PbAvatarShape) -> AvatarSelection {
    AvatarSelection {
        scene: None,
        shape: AvatarShape(shape.clone()),
        automatic_delete: true,
        disable_dither: false,
    }
}

fn spawn_scene_avatar(app: &mut App, root: Entity, shape: &PbAvatarShape) -> Entity {
    app.world_mut()
        .spawn((
            SceneEntity {
                root,
                scene_id: dcl_types::SceneId::DUMMY,
                id: dcl_component::SceneEntityId::ROOT,
            },
            AvatarShape(shape.clone()),
            selection(shape),
        ))
        .id()
}

fn name_of(app: &App, avatar: Entity) -> Option<String> {
    app.world()
        .get::<NametagContent>(avatar)
        .map(|content| content.name.clone())
}

#[test]
fn a_scene_avatar_is_named_once_built_and_follows_its_shape() {
    let mut app = scene_avatar_app();
    let scene = app.world_mut().spawn_empty().id();
    let guide = spawn_scene_avatar(&mut app, scene, &shape("guide-0001", Some("Guide")));

    // not built yet: the tag would sit at its feet
    app.update();
    assert_eq!(name_of(&app, guide), None);

    app.world_mut()
        .entity_mut(guide)
        .insert(NametagHeadOffset(0.4));
    app.update();
    app.update();
    assert_eq!(name_of(&app, guide).as_deref(), Some("Guide"));
    assert_eq!(
        app.world().get::<NametagVoice>(guide).unwrap().0,
        Voice::Off
    );

    // renamed and talking
    let mut talking = shape("guide-0001", Some("Host"));
    talking.talking = Some(true);
    app.world_mut()
        .entity_mut(guide)
        .insert(AvatarShape(talking));
    app.update();
    app.update();
    assert_eq!(name_of(&app, guide).as_deref(), Some("Host"));
    assert_eq!(
        app.world().get::<NametagVoice>(guide).unwrap().0,
        Voice::Speaking
    );
    // as a call's speaker: a scene that turns nearby voice off leaves the bars up
    assert_eq!(
        app.world().get::<Speaking>(guide),
        Some(&Speaking { in_call: true })
    );
    app.world_mut()
        .resource_mut::<MicState>()
        .set_scene_disabled(true);
    app.update();
    assert_eq!(
        app.world().get::<NametagVoice>(guide).unwrap().0,
        Voice::Speaking
    );
    app.world_mut()
        .resource_mut::<MicState>()
        .set_scene_disabled(false);

    // an empty name takes the tag away, and a name brings it back
    for (name, shown) in [("", None), ("Guide", Some("Guide"))] {
        app.world_mut()
            .entity_mut(guide)
            .insert(AvatarShape(shape("guide-0001", Some(name))));
        app.update();
        assert_eq!(name_of(&app, guide).as_deref(), shown, "{name:?}");
        // the shape no longer says talking
        assert!(app.world().get::<Speaking>(guide).is_none());
    }

    // an avatar with no tag is remembered as such: its shape is read again only once changed
    app.world_mut()
        .entity_mut(guide)
        .insert(AvatarShape(shape("guide-0001", Some(""))));
    app.update();
    app.update();
    let unnamed = SceneAvatarNametag { named: false };
    assert_eq!(app.world().get::<SceneAvatarNametag>(guide), Some(&unnamed));
    let read = app
        .world()
        .entity(guide)
        .get_change_ticks::<SceneAvatarNametag>()
        .unwrap();
    app.update();
    let kept = app
        .world()
        .entity(guide)
        .get_change_ticks::<SceneAvatarNametag>()
        .unwrap();
    assert_eq!(read.changed.get(), kept.changed.get());
    assert_eq!(name_of(&app, guide), None);

    // the scene drops the avatar
    app.world_mut().entity_mut(guide).remove::<AvatarShape>();
    app.update();
    assert_eq!(name_of(&app, guide), None);
    assert!(app.world().get::<SceneAvatarNametag>(guide).is_none());
}

#[test]
fn a_scene_avatar_made_again_waits_for_its_new_body() {
    let mut app = scene_avatar_app();
    app.add_observer(crate::add_attach_points_to_avatar_shape)
        .add_observer(crate::remove_attach_points_from_avatar_shape);
    let scene = app.world_mut().spawn_empty().id();
    let guide_shape = shape("guide-0001", Some("Guide"));
    let guide = spawn_scene_avatar(&mut app, scene, &guide_shape);
    app.world_mut()
        .entity_mut(guide)
        .insert(NametagHeadOffset(0.4));
    app.update();
    assert_eq!(name_of(&app, guide).as_deref(), Some("Guide"));

    // the scene removes the shape and gives it back: the body the offset was measured on is
    // gone, and the tag would sit at the feet of the next until it is built
    app.world_mut().entity_mut(guide).remove::<AvatarShape>();
    app.world_mut()
        .entity_mut(guide)
        .insert(AvatarShape(guide_shape));
    app.update();
    assert!(app.world().get::<NametagHeadOffset>(guide).is_none());
    assert_eq!(name_of(&app, guide), None);

    app.world_mut()
        .entity_mut(guide)
        .insert(NametagHeadOffset(0.4));
    app.update();
    assert_eq!(name_of(&app, guide).as_deref(), Some("Guide"));
}

#[test]
fn previews_and_restyled_players_get_no_scene_avatar_tag() {
    let mut app = scene_avatar_app();
    let npc_shape = shape("guide-0001", None);

    // an avatar the HUD previews, made by a system scene
    let system_scene = app.world_mut().spawn(SuperUserScene).id();
    let preview = spawn_scene_avatar(&mut app, system_scene, &npc_shape);
    // a scene entity that restyles a player: it is never drawn, so it has no selection
    let scene = app.world_mut().spawn_empty().id();
    let restyle = spawn_scene_avatar(&mut app, scene, &npc_shape);
    app.world_mut()
        .entity_mut(restyle)
        .remove::<AvatarSelection>();
    // a player keeps the name of their profile
    let player = spawn_scene_avatar(&mut app, scene, &npc_shape);
    app.world_mut()
        .entity_mut(player)
        .insert(PrimaryUser::default());

    for avatar in [preview, restyle, player] {
        app.world_mut()
            .entity_mut(avatar)
            .insert(NametagHeadOffset(0.4));
    }
    app.update();
    app.update();
    for avatar in [preview, restyle, player] {
        assert_eq!(name_of(&app, avatar), None);
    }
}

/// A world with the camera at the origin looking down -z, and the systems that give each
/// player their tag and decide which names show.
fn tag_app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .insert_resource(NametagsVisible(true))
        .insert_resource(NametagAssets {
            material: Handle::default(),
            font: Handle::default(),
            bound: [None; 2],
        })
        .add_systems(Update, show_tags)
        .add_observer(spawn_tag)
        .add_observer(despawn_unnamed_tag)
        .add_observer(despawn_unplated_tag);
    app.world_mut().spawn((
        Transform::default(),
        PrimaryCamera::default(),
        Projection::Perspective(PerspectiveProjection {
            fov: FOV,
            ..Default::default()
        }),
    ));
    app
}

const FOV: f32 = std::f32::consts::FRAC_PI_3;

fn spawn_player(app: &mut App, name: &str, distance: f32) -> Entity {
    let world = app.world_mut();
    let attach_points = AttachPoints::new(&mut world.commands());
    world.flush();
    let player = world
        .spawn((
            GlobalTransform::from_translation(Vec3::new(0.0, 0.0, -distance)),
            content(name),
            attach_points,
        ))
        .id();
    world.flush();
    player
}

fn move_player(app: &mut App, player: Entity, distance: f32) {
    *app.world_mut().get_mut::<GlobalTransform>(player).unwrap() =
        GlobalTransform::from_translation(Vec3::new(0.0, 0.0, -distance));
}

fn tags(app: &mut App) -> Vec<Entity> {
    app.world_mut()
        .query_filtered::<Entity, With<Nametag>>()
        .iter(app.world())
        .collect()
}

fn tag_of(app: &App, player: Entity) -> Option<Entity> {
    app.world()
        .get::<Children>(player)?
        .iter()
        .find(|child| app.world().get::<Nametag>(*child).is_some())
}

/// Whether the player's name should show.
fn shown(app: &App, player: Entity) -> bool {
    let tag = tag_of(app, player).expect("a player with a name has a tag");
    app.world().get::<Nametag>(tag).unwrap().shown
}

fn bubble(message: &str) -> ChatBubble {
    ChatBubble {
        kind: BubbleKind::Nearby,
        message: bubble_text(message, MAX_MESSAGE_CHARS),
        message_id: "1".to_owned(),
        arrived: 0.0,
        mention: false,
        recipient: None,
        stale: Stale::Message,
        texts: None,
        animation: None,
    }
}

#[test]
fn a_player_has_their_tag_for_as_long_as_they_have_a_name() {
    let mut app = tag_app();
    let ada = spawn_player(&mut app, "Ada", 10.0);
    let grace = spawn_player(&mut app, "Grace", 5.0);
    let tag = tag_of(&app, ada).expect("a player with a name has a tag");
    assert_eq!(tags(&mut app).len(), 2);

    // what they show changing is not another tag
    app.world_mut().entity_mut(ada).insert(content("Ada L"));
    app.update();
    assert_eq!(tag_of(&app, ada), Some(tag));
    assert_eq!(tags(&mut app).len(), 2);

    // a player who disconnects takes their tag along
    app.world_mut().despawn(ada);
    app.update();
    assert!(app.world().get_entity(tag).is_err());

    // a player with nothing to say (their profile gone) keeps neither tag nor message
    app.world_mut().entity_mut(grace).insert(bubble("hello"));
    app.world_mut().entity_mut(grace).remove::<NametagContent>();
    app.update();
    assert!(tags(&mut app).is_empty());
    assert!(!app.world().entity(grace).contains::<ChatBubble>());
}

#[test]
fn a_name_out_of_range_is_hidden_and_keeps_its_tag() {
    let mut app = tag_app();
    let ada = spawn_player(&mut app, "Ada", 10.0);
    app.update();
    let tag = tag_of(&app, ada).unwrap();
    assert!(shown(&app, ada));

    for (distance, shows) in [(41.0, false), (40.0, true), (1.9, false), (2.0, true)] {
        move_player(&mut app, ada, distance);
        app.update();
        assert_eq!(shown(&app, ada), shows, "{distance}");
        assert_eq!(tag_of(&app, ada), Some(tag));
    }

    // behind the camera: still a name that shows, which the frustum culls
    move_player(&mut app, ada, -10.0);
    app.update();
    assert!(shown(&app, ada));
}

#[test]
fn a_hidden_name_keeps_its_tag_and_its_message() {
    let mut app = tag_app();
    let ada = spawn_player(&mut app, "Ada", 10.0);
    app.world_mut().entity_mut(ada).insert(bubble("hello"));
    app.update();
    let tag = tag_of(&app, ada).unwrap();
    assert!(shown(&app, ada));

    // the nametags toggle
    app.world_mut().resource_mut::<NametagsVisible>().0 = false;
    app.update();
    assert!(!shown(&app, ada));
    app.world_mut().resource_mut::<NametagsVisible>().0 = true;
    app.update();
    assert!(shown(&app, ada));

    // a scene area hiding nametags, a block or a ban, and the nearest-avatars cap
    app.world_mut().entity_mut(ada).insert(HiddenNametag);
    app.update();
    assert!(!shown(&app, ada));
    app.world_mut().entity_mut(ada).remove::<HiddenNametag>();
    app.world_mut().entity_mut(ada).insert(BlockedOrBanned);
    app.update();
    assert!(!shown(&app, ada));
    app.world_mut().entity_mut(ada).remove::<BlockedOrBanned>();
    app.world_mut().entity_mut(ada).insert(BeyondMaxAvatars);
    app.update();
    assert!(!shown(&app, ada));
    app.world_mut().entity_mut(ada).remove::<BeyondMaxAvatars>();
    // the avatar itself hidden
    app.world_mut().entity_mut(ada).insert(PlayerModifiers {
        hide: true,
        ..Default::default()
    });
    app.update();
    assert!(!shown(&app, ada));
    app.world_mut().entity_mut(ada).remove::<PlayerModifiers>();
    app.update();
    assert!(shown(&app, ada));

    assert_eq!(tag_of(&app, ada), Some(tag));
    assert!(app.world().entity(ada).contains::<ChatBubble>());
}

#[test]
fn an_avatar_its_scene_hides_shows_no_name() {
    let mut app = tag_app();
    let guide = spawn_player(&mut app, "Guide", 10.0);
    app.world_mut()
        .entity_mut(guide)
        .insert(InheritedVisibility::VISIBLE);
    app.update();
    assert!(shown(&app, guide));

    // hidden by its own visibility or a parent's
    app.world_mut()
        .entity_mut(guide)
        .insert(InheritedVisibility::HIDDEN);
    app.update();
    assert!(!shown(&app, guide));
    app.world_mut()
        .entity_mut(guide)
        .insert(InheritedVisibility::VISIBLE);
    app.update();
    assert!(shown(&app, guide));
}

#[test]
fn an_avatar_scaled_to_nothing_shows_no_name() {
    let mut app = tag_app();
    let guide = spawn_player(&mut app, "Guide", 10.0);
    app.update();
    assert!(shown(&app, guide));

    let at = |scale: Vec3| {
        GlobalTransform::from(bevy::math::Affine3A::from_scale_rotation_translation(
            scale,
            Quat::IDENTITY,
            Vec3::new(0.0, 0.0, -10.0),
        ))
    };
    // nothing at all, the sliver a scale of nothing becomes, flattened along one axis, and
    // not a number
    for scale in [
        Vec3::ZERO,
        Vec3::splat(f32::EPSILON),
        Vec3::new(1.0, 1.0, 0.0),
        Vec3::splat(f32::NAN),
    ] {
        *app.world_mut().get_mut::<GlobalTransform>(guide).unwrap() = at(scale);
        app.update();
        assert!(!shown(&app, guide), "{scale}");
        *app.world_mut().get_mut::<GlobalTransform>(guide).unwrap() = at(Vec3::splat(0.5));
        app.update();
        assert!(shown(&app, guide), "{scale}");
    }
}

#[test]
fn first_person_hides_only_the_local_players_own_name() {
    let mut app = tag_app();
    let me = spawn_player(&mut app, "Me", 3.0);
    app.world_mut()
        .entity_mut(me)
        .insert(PrimaryUser::default());
    let other = spawn_player(&mut app, "Other", 3.0);
    app.update();
    assert!(shown(&app, me) && shown(&app, other));

    let set_camera = |app: &mut App, distance: f32, scene_override| {
        let mut cameras = app.world_mut().query::<&mut PrimaryCamera>();
        let mut camera = cameras.single_mut(app.world_mut()).unwrap();
        camera.distance = distance;
        camera.scene_override = scene_override;
        app.update();
    };
    set_camera(&mut app, 0.0, None);
    assert!(!shown(&app, me));
    assert!(shown(&app, other));

    // a scene's camera wins over the player's own zoom, either way
    set_camera(&mut app, 0.0, Some(CameraOverride::Distance(1.0)));
    assert!(shown(&app, me));
    set_camera(&mut app, 3.0, Some(CameraOverride::Distance(0.0)));
    assert!(!shown(&app, me));
    let cinematic = CameraOverride::Cinematic(CinematicSettings {
        origin: Entity::PLACEHOLDER,
        allow_manual_rotation: false,
        yaw_range: None,
        pitch_range: None,
        roll_range: None,
        look_at_entity: None,
        transition: None,
        fov: FOV,
    });
    set_camera(&mut app, 0.0, Some(cinematic));
    assert!(shown(&app, me));
}

#[test]
fn a_tag_stands_on_its_anchor_within_bounds_for_its_largest() {
    let mut app = tag_app();
    let ada = spawn_player(&mut app, "Ada", 10.0);
    let tag = tag_of(&app, ada).unwrap();
    let anchor = app.world().get::<AttachPoints>(ada).unwrap().nametag;
    app.world_mut()
        .get_mut::<Transform>(anchor)
        .unwrap()
        .translation = Vec3::new(0.1, 1.9, 0.0);
    app.world_mut().get_mut::<Nametag>(tag).unwrap().reach = 200.0;

    app.world_mut().run_system_once(place_tags).unwrap();
    // relative to the player: wherever they are this frame, the tag is over them
    let transform = *app.world().get::<Transform>(tag).unwrap();
    assert_eq!(transform.translation, Vec3::new(0.1, 1.9, 0.0));
    assert_eq!(transform.rotation, Quat::IDENTITY);
    assert_eq!(transform.scale, Vec3::ONE);
    assert_eq!(app.world().get::<ChildOf>(tag).unwrap().parent(), ada);
    // the farthest corner at the far end of the range, whichever way the player faces
    let aabb = *app.world().get::<Aabb>(tag).unwrap();
    assert_eq!(aabb.center, Vec3A::ZERO);
    let largest = Vec3A::splat(200.0 * tag_scale(MAX_DISTANCE, FOV));
    assert!(aabb.half_extents.distance(largest) < 1e-4);

    // a scene's scale of its avatar shrinks the bounds with it: they are sized against it
    *app.world_mut().get_mut::<GlobalTransform>(ada).unwrap() =
        GlobalTransform::from_scale(Vec3::new(1.0, 0.5, 2.0));
    app.world_mut().run_system_once(place_tags).unwrap();
    let aabb = *app.world().get::<Aabb>(tag).unwrap();
    assert!(aabb.half_extents.distance(largest * 2.0) < 1e-4);

    // the anchor moves with the animation, and the tag with it
    app.world_mut()
        .get_mut::<Transform>(anchor)
        .unwrap()
        .translation = Vec3::new(0.0, 2.4, 0.0);
    app.world_mut().run_system_once(place_tags).unwrap();
    assert_eq!(
        app.world().get::<Transform>(tag).unwrap().translation,
        Vec3::new(0.0, 2.4, 0.0)
    );
}

#[test]
fn a_message_unseen_past_its_time_is_over() {
    let mut unseen = bubble("hello");
    unseen.arrived = 10.0;
    assert!(!unseen.over(10.0 + BUBBLE_LIFETIME - 0.01));
    assert!(unseen.over(10.0 + BUBBLE_LIFETIME));
}

/// The tags built for real: text laid out with the embedded font, glyphs rastered into the
/// nametags' atlas, and time moved by hand.
fn build_app() -> App {
    use bevy::text::{CosmicFontSystem, TextPipeline};

    let mut app = tag_app();
    app.add_plugins(bevy::asset::AssetPlugin::default())
        .init_asset::<Font>()
        .init_asset::<Image>()
        .init_asset::<Mesh>()
        .init_resource::<TextPipeline>()
        .init_resource::<CosmicFontSystem>()
        .init_resource::<AppConfig>()
        .init_resource::<CurrentUserProfile>()
        .insert_resource(PrimaryPlayerRes(Entity::PLACEHOLDER))
        .insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
            std::time::Duration::ZERO,
        ))
        .add_event::<ChatBubbleRequest>()
        .add_event::<ChatBubbleRetext>()
        .add_systems(
            Update,
            (show_chat_bubbles, build_tags)
                .chain()
                .after(show_tags)
                .after(GlyphAtlasMaintenance),
        );
    scene_runner::update_world::text_shape::add_glyph_atlas_maintenance(&mut app);
    let font = Font::try_from_bytes(include_bytes!("fonts/Inter_18pt-SemiBold.ttf").to_vec())
        .expect("the embedded nametag font is a font");
    let font = app.world_mut().resource_mut::<Assets<Font>>().add(font);
    app.world_mut().resource_mut::<NametagAssets>().font = font;
    app
}

/// One frame, `seconds` after the last, and as many more at that same time as the glyphs it
/// asked for take to be rastered: a bubble's are first drawn once it has opened a little.
fn advance(app: &mut App, seconds: f64) {
    use bevy::time::TimeUpdateStrategy;
    use std::time::Duration;

    app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f64(
        seconds,
    )));
    app.update();
    app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::ZERO));
    for _ in 0..2000 {
        let mut tags = app.world_mut().query::<&Nametag>();
        if !tags
            .iter(app.world())
            .any(|nametag| nametag.shown && nametag.awaiting)
        {
            return;
        }
        std::thread::sleep(Duration::from_millis(1));
        app.update();
    }
    panic!("the glyphs were never rastered");
}

fn draws(app: &App, player: Entity) -> bool {
    let tag = tag_of(app, player).unwrap();
    app.world().get::<Visibility>(tag) == Some(&Visibility::Inherited)
}

/// Frames without time passing until the player's tag has all its glyphs.
fn settle(app: &mut App, player: Entity) {
    for _ in 0..2000 {
        app.update();
        let tag = tag_of(app, player).unwrap();
        if draws(app, player) && !app.world().get::<Nametag>(tag).unwrap().awaiting {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    panic!("the tag's glyphs were never rastered");
}

fn say(app: &mut App, sender: Entity, message: &str) {
    app.world_mut().send_event(ChatBubbleRequest {
        sender,
        kind: BubbleKind::Nearby,
        message: message.to_owned(),
        message_id: "m1".to_owned(),
    });
}

fn retext(app: &mut App, message: &str) {
    app.world_mut().send_event(ChatBubbleRetext {
        message_id: "m1".to_owned(),
        message: message.to_owned(),
    });
}

/// How far the player's bubble has got, as their tag's mesh was last built.
fn built_progress(app: &App, player: Entity) -> BubbleProgress {
    let tag = tag_of(app, player).unwrap();
    let nametag = app.world().get::<Nametag>(tag).unwrap();
    nametag.built.as_ref().expect("a built tag").progress
}

fn bubble_phase(app: &mut App, player: Entity) -> Option<BubblePhase> {
    let now = app.world().resource::<Time<Real>>().elapsed_secs_f64();
    let mut bubble = app.world_mut().get_mut::<ChatBubble>(player)?;
    bubble
        .animation
        .as_mut()
        .map(|animation| animation.tick(now))
}

const SHOWN: f64 = animation::CONTAINER_DELAY + animation::TRANSITION;

#[test]
fn a_message_opens_over_its_sender_and_comes_down_when_its_time_is_up() {
    let mut app = build_app();
    let ada = spawn_player(&mut app, "Ada", 10.0);
    settle(&mut app, ada);
    assert_eq!(built_progress(&app, ada), BubbleProgress::default());

    say(&mut app, ada, "hello there");
    settle(&mut app, ada);
    advance(&mut app, 0.2);
    let opening = built_progress(&app, ada);
    assert!(opening.padding > 0.0 && opening.container.x > 0.0);
    advance(&mut app, SHOWN);
    let open = built_progress(&app, ada);
    assert_eq!(open.padding, 1.0);
    // built once more at its final size, then left alone
    advance(&mut app, 1.0);
    assert_eq!(built_progress(&app, ada), open);

    advance(&mut app, BUBBLE_LIFETIME - 1.2 - SHOWN + 0.1);
    assert_eq!(bubble_phase(&mut app, ada), Some(BubblePhase::Hiding));
    advance(&mut app, SHOWN);
    assert!(!app.world().entity(ada).contains::<ChatBubble>());
    assert!(draws(&app, ada));
    assert_eq!(built_progress(&app, ada), BubbleProgress::default());
}

#[test]
fn nothing_to_read_opens_no_bubble() {
    let mut app = build_app();
    let ada = spawn_player(&mut app, "Ada", 10.0);
    settle(&mut app, ada);
    for blank in ["", "   ", "\n\t"] {
        say(&mut app, ada, blank);
        app.update();
        assert!(
            !app.world().entity(ada).contains::<ChatBubble>(),
            "{blank:?}"
        );
    }

    // nor do blank words replace a message
    say(&mut app, ada, "hello");
    app.update();
    retext(&mut app, "  ");
    app.update();
    assert_eq!(
        app.world().get::<ChatBubble>(ada).unwrap().message.text,
        "hello"
    );
}

#[test]
fn a_tag_hidden_while_its_bubble_is_up_shows_it_when_it_is_back() {
    let mut app = build_app();
    let ada = spawn_player(&mut app, "Ada", 10.0);
    settle(&mut app, ada);
    say(&mut app, ada, "hello there");
    settle(&mut app, ada);
    advance(&mut app, 0.2);

    // out of range mid-way: hidden, and the message stays its sender's
    move_player(&mut app, ada, 60.0);
    advance(&mut app, 1.0);
    assert!(!draws(&app, ada));
    assert!(app.world().entity(ada).contains::<ChatBubble>());

    // back inside its time: up, as far along as its time says
    move_player(&mut app, ada, 10.0);
    advance(&mut app, 0.0);
    assert!(draws(&app, ada));
    assert_eq!(bubble_phase(&mut app, ada), Some(BubblePhase::Showing));
    assert_eq!(built_progress(&app, ada).padding, 1.0);
}

#[test]
fn nametags_turned_off_while_a_bubble_is_up_let_its_time_run_out() {
    let mut app = build_app();
    let ada = spawn_player(&mut app, "Ada", 10.0);
    settle(&mut app, ada);
    say(&mut app, ada, "hello there");
    settle(&mut app, ada);
    advance(&mut app, 0.2);

    app.world_mut().resource_mut::<NametagsVisible>().0 = false;
    advance(&mut app, 1.0);
    assert!(!draws(&app, ada));
    assert!(app.world().entity(ada).contains::<ChatBubble>());
    // a message sent meanwhile is not kept for later
    say(&mut app, ada, "anyone?");
    advance(&mut app, BUBBLE_LIFETIME);
    assert!(!app.world().entity(ada).contains::<ChatBubble>());

    // back on: the idle tag, not a message from before
    app.world_mut().resource_mut::<NametagsVisible>().0 = true;
    settle(&mut app, ada);
    assert_eq!(built_progress(&app, ada), BubbleProgress::default());
}

#[test]
fn a_message_that_came_to_a_hidden_tag_keeps_only_the_rest_of_its_time() {
    let mut app = build_app();
    let ada = spawn_player(&mut app, "Ada", 60.0);
    app.update();
    say(&mut app, ada, "hello there");
    app.update();
    advance(&mut app, 3.0);
    assert!(app.world().entity(ada).contains::<ChatBubble>());

    move_player(&mut app, ada, 10.0);
    settle(&mut app, ada);
    assert_eq!(bubble_phase(&mut app, ada), Some(BubblePhase::Showing));
    advance(&mut app, 2.1);
    assert_eq!(bubble_phase(&mut app, ada), Some(BubblePhase::Hiding));
}

#[test]
fn a_sender_who_leaves_while_their_bubble_is_up_takes_it_along() {
    let mut app = build_app();
    let ada = spawn_player(&mut app, "Ada", 10.0);
    settle(&mut app, ada);
    say(&mut app, ada, "hello there");
    settle(&mut app, ada);
    advance(&mut app, 0.2);

    app.world_mut().despawn(ada);
    advance(&mut app, 0.1);
    advance(&mut app, BUBBLE_LIFETIME);
    assert!(tags(&mut app).is_empty());
}

#[test]
fn other_words_reach_a_bubble_that_is_up_and_not_one_coming_down() {
    let mut app = build_app();
    let ada = spawn_player(&mut app, "Ada", 10.0);
    settle(&mut app, ada);
    say(&mut app, ada, "hello there");
    settle(&mut app, ada);
    advance(&mut app, 0.2);

    // while it opens: resized for the new words, on the time it had
    retext(
        &mut app,
        "hola, y unas cuantas palabras más para que sea más ancho",
    );
    settle(&mut app, ada);
    advance(&mut app, SHOWN);
    let wide = built_progress(&app, ada);
    assert!(wide.container.x > 100.0);
    assert_eq!(app.world().get::<ChatBubble>(ada).unwrap().arrived, 0.0);

    // while it comes down: it keeps coming down
    advance(&mut app, BUBBLE_LIFETIME - 0.2 - SHOWN + 0.1);
    assert_eq!(bubble_phase(&mut app, ada), Some(BubblePhase::Hiding));
    retext(&mut app, "hello there");
    settle(&mut app, ada);
    assert_eq!(bubble_phase(&mut app, ada), Some(BubblePhase::Hiding));
    advance(&mut app, 0.15);
    let closing = built_progress(&app, ada);
    assert!(closing.container.x < wide.container.x);
    advance(&mut app, SHOWN);
    assert!(!app.world().entity(ada).contains::<ChatBubble>());
}

#[test]
fn a_sender_renamed_while_their_bubble_comes_down_gets_the_new_name_on_the_idle_tag() {
    let mut app = build_app();
    let ada = spawn_player(&mut app, "Ada", 10.0);
    settle(&mut app, ada);
    say(&mut app, ada, "hello there");
    settle(&mut app, ada);
    advance(&mut app, BUBBLE_LIFETIME + 0.1);
    assert_eq!(bubble_phase(&mut app, ada), Some(BubblePhase::Hiding));

    app.world_mut()
        .entity_mut(ada)
        .insert(content("Ada Lovelace"));
    settle(&mut app, ada);
    assert_eq!(bubble_phase(&mut app, ada), Some(BubblePhase::Hiding));
    advance(&mut app, SHOWN);
    assert!(!app.world().entity(ada).contains::<ChatBubble>());
    let tag = tag_of(&app, ada).unwrap();
    let nametag = app.world().get::<Nametag>(tag).unwrap();
    assert_eq!(
        nametag
            .built
            .as_ref()
            .unwrap()
            .content
            .as_ref()
            .unwrap()
            .name,
        "Ada Lovelace"
    );
    assert_eq!(built_progress(&app, ada), BubbleProgress::default());
}

#[test]
fn a_mesh_from_before_an_atlas_rebuild_is_built_again() {
    let mut app = build_app();
    let ada = spawn_player(&mut app, "Ada", 10.0);
    settle(&mut app, ada);
    say(&mut app, ada, "hello there");
    settle(&mut app, ada);
    advance(&mut app, 0.2);

    // as a rebuild leaves it mid-way: built against a generation that is no longer the atlas'
    let tag = tag_of(&app, ada).unwrap();
    let mesh = app.world().get::<Mesh3d>(tag).unwrap().0.clone();
    let generation = {
        let mut nametag = app.world_mut().get_mut::<Nametag>(tag).unwrap();
        let built = nametag.built.as_mut().unwrap();
        built.generation += 1;
        built.generation - 1
    };
    advance(&mut app, 0.05);
    let nametag = app.world().get::<Nametag>(tag).unwrap();
    assert_eq!(nametag.built.as_ref().unwrap().generation, generation);
    assert!(draws(&app, ada));
    // the same mesh, written again
    assert_eq!(app.world().get::<Mesh3d>(tag).unwrap().0, mesh);
    assert_eq!(bubble_phase(&mut app, ada), Some(BubblePhase::Showing));
}

/// The nametag shader compiles on this machine's adapter, in a renderer with no window: a tag
/// of every kind of quad in front of a camera with the prepasses and depth of field (whose
/// focus target a transparent material must write). Needs a GPU, so `#[ignore]`d and skipped
/// by CI:
///
/// ```text
/// cargo test -p avatar nametag_shader -- --ignored --nocapture
/// ```
#[test]
#[ignore = "needs a GPU; run with --ignored"]
fn nametag_shader_compiles() {
    use bevy::{
        asset::RenderAssetUsages,
        core_pipeline::{
            dof::DepthOfField,
            prepass::{DepthPrepass, NormalPrepass},
        },
        render::{
            camera::RenderTarget,
            render_resource::{Extent3d, TextureDimension, TextureFormat},
        },
    };
    use pipeline_test::{compile_pipelines, fragment_pipelines, headless_app, render_target};

    use super::layout::Tail;

    let mut app = headless_app();
    app.add_plugins(MaterialPlugin::<NametagMaterial> {
        prepass_enabled: false,
        shadows_enabled: false,
        ..default()
    });
    app.finish();
    app.cleanup();

    let target = render_target(&mut app);
    let world = app.world_mut();
    let mut texture = |format: TextureFormat, pixel: &[u8]| {
        world.resource_mut::<Assets<Image>>().add(Image::new_fill(
            Extent3d::default(),
            TextureDimension::D2,
            pixel,
            format,
            RenderAssetUsages::all(),
        ))
    };
    let sdf = texture(TextureFormat::R8Unorm, &[255]);
    let icon = texture(TextureFormat::Rgba8UnormSrgb, &[255; 4]);
    let material = world
        .resource_mut::<Assets<NametagMaterial>>()
        .add(NametagMaterial {
            sdf,
            bitmap: None,
            verified: icon.clone(),
            official: icon.clone(),
            hushed: icon,
        });

    let rect = Rect::new(-40.0, 0.0, 40.0, 32.0);
    let badge = Rect::new(0.0, 8.0, 14.0, 22.0);
    let glyph = |bitmap| Quad::glyph(badge, Vec2::ZERO, Vec2::ONE, bitmap, LinearRgba::WHITE);
    let (mesh, _) = quad_mesh(&[
        Quad::tail(
            Tail {
                origin: Vec2::new(0.0, 1.0),
                scale: 1.0,
            },
            BORDER_COLOR,
        ),
        Quad::gradient(rect, CORNER_RADIUS, MENTION_RING_BOTTOM, MENTION_RING_TOP),
        Quad::rect(rect, CORNER_RADIUS, BACKGROUND),
        glyph(false),
        glyph(true),
        Quad::icon(badge, Icon::Verified, Color::WHITE),
        Quad::icon(badge, Icon::Official, Color::WHITE),
        Quad::icon(badge, Icon::Hushed, HUSHED_COLOR),
        Quad::voice_bar(badge, VOICE_BAR_LOW, VOICE_BAR_HIGH, VOICE_COLOR),
    ]);
    let mesh = world.resource_mut::<Assets<Mesh>>().add(mesh);

    world.spawn((
        Camera3d::default(),
        Camera {
            target: RenderTarget::Image(target.into()),
            hdr: true,
            ..default()
        },
        DepthPrepass,
        NormalPrepass,
        DepthOfField::default(),
        // the focus target is single-sampled
        Msaa::Off,
        Transform::from_xyz(0.0, 0.0, 4.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
    world.spawn((
        Mesh3d(mesh),
        MeshMaterial3d(material),
        Aabb::from_min_max(Vec3::splat(-1.0), Vec3::splat(1.0)),
        NotShadowCaster,
        Transform::default(),
    ));

    compile_pipelines(&mut app);
    assert!(
        fragment_pipelines(&app, "embedded://shaders/nametag.wgsl") >= 1,
        "the nametag was not drawn"
    );
}

fn scene_plate(label: &str) -> AvatarNametag {
    AvatarNametag::from(PbAvatarNametag {
        label: label.to_owned(),
        ..Default::default()
    })
}

/// Another player, by the id a scene knows them by.
#[derive(Component)]
struct Visitor(SceneEntityId);

/// The plates, worked out from where the scenes and the players are.
fn plate_app() -> App {
    let mut pointers = ScenePointers::default();
    pointers.set_realm(IVec2::splat(-8), IVec2::splat(8));
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .insert_resource(pointers)
        .init_resource::<LiveScenes>()
        .init_resource::<PortableScenes>()
        .add_systems(
            Update,
            |mut commands: Commands,
             containing_scene: ContainingScene,
             local: Query<Entity, With<PrimaryUser>>,
             others: Query<(Entity, &Visitor)>,
             plated: plate::Plated,
             holders: Query<(Entity, &NametagPlate)>,
             mut players: Local<bevy::platform::collections::HashMap<SceneEntityId, Entity>>,
             mut owners: Local<HashSet<Entity>>| {
                plate::plate_avatars(
                    &mut commands,
                    &containing_scene,
                    local.single().ok(),
                    others.iter().map(|(player, visitor)| (player, visitor.0)),
                    &plated,
                    &holders,
                    &mut players,
                    &mut owners,
                );
            },
        );
    app
}

const HOME: IVec2 = IVec2::ZERO;
const NEXT_DOOR: IVec2 = IVec2::new(1, 0);
/// A parcel no scene is on.
const EMPTY: IVec2 = IVec2::new(4, 4);

const FIRST_VISITOR: SceneEntityId = SceneEntityId {
    id: 32,
    generation: 0,
};

/// A scene on `parcel`.
fn scene_on(app: &mut App, parcel: IVec2) -> Entity {
    let scene = app.world_mut().spawn_empty().id();
    let hash = format!("scene {parcel}");
    app.world_mut().resource_mut::<ScenePointers>().insert(
        parcel,
        PointerResult::Exists {
            hash: hash.clone(),
            urn: None,
        },
    );
    app.world_mut()
        .resource_mut::<LiveScenes>()
        .scenes
        .insert(hash, scene);
    scene
}

fn stand_on(app: &mut App, who: Entity, parcel: IVec2) {
    app.world_mut()
        .entity_mut(who)
        .insert(GlobalTransform::from_translation(parcel_to_vec3(parcel)));
}

fn local_on(app: &mut App, parcel: IVec2) -> Entity {
    let local = app.world_mut().spawn(PrimaryUser::default()).id();
    stand_on(app, local, parcel);
    local
}

fn visitor_on(app: &mut App, id: SceneEntityId, parcel: IVec2) -> Entity {
    let visitor = app.world_mut().spawn(Visitor(id)).id();
    stand_on(app, visitor, parcel);
    visitor
}

/// A scene's entity `id` asking for a plate that reads `label`.
fn ask_for_plate(app: &mut App, root: Entity, id: SceneEntityId, label: &str) -> Entity {
    app.world_mut()
        .spawn((
            SceneEntity {
                root,
                scene_id: dcl_types::SceneId::DUMMY,
                id,
            },
            scene_plate(label),
        ))
        .id()
}

fn plate_of(app: &App, avatar: Entity) -> Option<String> {
    app.world()
        .get::<NametagPlate>(avatar)
        .map(|plate| plate.label.clone())
}

#[test]
fn a_plate_takes_the_clients_colours_where_the_scene_gives_none() {
    let plain = scene_plate("Club Owner").0;
    assert_eq!(plain.label, "Club Owner");
    assert_eq!(plain.label_color, Color::srgb_u8(0xFC, 0xFC, 0xFC));
    assert_eq!(plain.background, BACKGROUND);
    assert_eq!(plain.border, BACKGROUND);

    let red = Color3 {
        r: 1.0,
        g: 0.0,
        b: 0.0,
    };
    let filled = NametagPlate::from(&PbAvatarNametag {
        label: "VIP".to_owned(),
        background_color: Some(red),
        ..Default::default()
    });
    assert_eq!(filled.background, Color::srgb(1.0, 0.0, 0.0));
    // no border of its own
    assert_eq!(filled.border, filled.background);

    let all = NametagPlate::from(&PbAvatarNametag {
        label: "VIP".to_owned(),
        label_color: Some(Color3 {
            r: 0.0,
            g: 1.0,
            b: 0.0,
        }),
        background_color: Some(red),
        border_color: Some(Color3 {
            r: 0.0,
            g: 0.0,
            b: 1.0,
        }),
    });
    assert_eq!(all.label_color, Color::srgb(0.0, 1.0, 0.0));
    assert_eq!(all.border, Color::srgb(0.0, 0.0, 1.0));
}

#[test]
fn a_scenes_label_and_colours_are_bounded() {
    let wild = NametagPlate::from(&PbAvatarNametag {
        label: "a".repeat(5000),
        label_color: Some(Color3 {
            r: 7.0,
            g: -3.0,
            b: f32::NAN,
        }),
        ..Default::default()
    });
    assert_eq!(wild.label.chars().count(), plate::MAX_PLATE_CHARS + 1);
    assert_eq!(wild.label_color, Color::srgb(1.0, 0.0, 0.0));

    // one line, and the scene's spaces as they came
    assert_eq!(scene_plate("two\nlines").0.label, "two lines");
    assert_eq!(scene_plate("").0.label, "");
    assert_eq!(scene_plate("    ").0.label, "    ");
}

#[test]
fn a_scene_plates_the_local_player_and_the_players_standing_in_it() {
    let mut app = plate_app();
    let scene = scene_on(&mut app, HOME);
    let local = local_on(&mut app, HOME);
    let visitor = visitor_on(&mut app, FIRST_VISITOR, EMPTY);
    ask_for_plate(&mut app, scene, SceneEntityId::PLAYER, "Host");
    let asked = ask_for_plate(&mut app, scene, FIRST_VISITOR, "Guest");
    app.update();
    assert_eq!(plate_of(&app, local).as_deref(), Some("Host"));
    // written while the visitor stands outside
    assert_eq!(plate_of(&app, visitor), None);

    stand_on(&mut app, visitor, HOME);
    app.update();
    assert_eq!(plate_of(&app, visitor).as_deref(), Some("Guest"));

    // out of the scene the plate goes, and what the scene wrote stays for their return
    stand_on(&mut app, visitor, NEXT_DOOR);
    app.update();
    assert_eq!(plate_of(&app, visitor), None);
    assert!(app.world().entity(asked).contains::<AvatarNametag>());
    stand_on(&mut app, visitor, HOME);
    app.update();
    assert_eq!(plate_of(&app, visitor).as_deref(), Some("Guest"));
}

#[test]
fn a_plate_stays_with_the_player_it_was_written_for() {
    let mut app = plate_app();
    let scene = scene_on(&mut app, HOME);
    local_on(&mut app, HOME);
    let first = visitor_on(&mut app, FIRST_VISITOR, HOME);
    ask_for_plate(&mut app, scene, FIRST_VISITOR, "Guest");
    app.update();
    assert_eq!(plate_of(&app, first).as_deref(), Some("Guest"));

    // a freed id comes back as a new generation: the next player is not the one plated
    app.world_mut().entity_mut(first).despawn();
    let next = visitor_on(
        &mut app,
        SceneEntityId {
            id: FIRST_VISITOR.id,
            generation: 1,
        },
        HOME,
    );
    app.update();
    assert_eq!(plate_of(&app, next), None);
}

#[test]
fn a_scene_plates_its_own_avatars_once_they_are_built() {
    let mut app = plate_app();
    let scene = scene_on(&mut app, HOME);
    local_on(&mut app, HOME);
    let id = SceneEntityId {
        id: 600,
        generation: 0,
    };
    let guide = ask_for_plate(&mut app, scene, id, "Guide");
    // no avatar: nothing to plate
    let prop = ask_for_plate(
        &mut app,
        scene,
        SceneEntityId {
            id: 601,
            generation: 0,
        },
        "Prop",
    );
    let guide_shape = shape("guide-0001", None);
    app.world_mut()
        .entity_mut(guide)
        .insert((AvatarShape(guide_shape.clone()), selection(&guide_shape)));
    app.update();
    assert_eq!(plate_of(&app, guide), None);

    app.world_mut()
        .entity_mut(guide)
        .insert(NametagHeadOffset(0.3));
    app.update();
    assert_eq!(plate_of(&app, guide).as_deref(), Some("Guide"));
    assert_eq!(plate_of(&app, prop), None);

    // rebuilt for a new outfit, and back
    app.world_mut()
        .entity_mut(guide)
        .remove::<NametagHeadOffset>();
    app.update();
    assert_eq!(plate_of(&app, guide), None);
}

#[test]
fn an_entity_that_stands_for_a_player_plates_the_player_alone() {
    let mut app = plate_app();
    let scene = scene_on(&mut app, HOME);
    local_on(&mut app, HOME);
    // a restyled player: the entity carries a shape and its body
    let asked = ask_for_plate(&mut app, scene, FIRST_VISITOR, "Guest");
    let restyle = shape("visitor", Some("Visitor"));
    app.world_mut().entity_mut(asked).insert((
        AvatarShape(restyle.clone()),
        selection(&restyle),
        NametagHeadOffset(0.3),
    ));
    app.update();
    assert_eq!(plate_of(&app, asked), None);

    let visitor = visitor_on(&mut app, FIRST_VISITOR, HOME);
    app.update();
    assert_eq!(plate_of(&app, asked), None);
    assert_eq!(plate_of(&app, visitor).as_deref(), Some("Guest"));
}

#[test]
fn only_the_scene_the_local_player_stands_in_plates_anyone() {
    let mut app = plate_app();
    let scene = scene_on(&mut app, HOME);
    let next_door = scene_on(&mut app, NEXT_DOOR);
    let local = local_on(&mut app, EMPTY);
    let visitor = visitor_on(&mut app, FIRST_VISITOR, HOME);
    ask_for_plate(&mut app, scene, SceneEntityId::PLAYER, "Host");
    ask_for_plate(&mut app, scene, FIRST_VISITOR, "Guest");
    app.update();
    // the local player stands in no scene: no one is plated, not even who stands in one
    assert_eq!(plate_of(&app, local), None);
    assert_eq!(plate_of(&app, visitor), None);

    stand_on(&mut app, local, HOME);
    app.update();
    assert_eq!(plate_of(&app, local).as_deref(), Some("Host"));
    assert_eq!(plate_of(&app, visitor).as_deref(), Some("Guest"));

    // walked into another scene, which writes its own
    stand_on(&mut app, local, NEXT_DOOR);
    app.update();
    assert_eq!(plate_of(&app, local), None);
    assert_eq!(plate_of(&app, visitor), None);
    ask_for_plate(&mut app, next_door, SceneEntityId::PLAYER, "Stranger");
    app.update();
    assert_eq!(plate_of(&app, local).as_deref(), Some("Stranger"));

    stand_on(&mut app, local, HOME);
    app.update();
    assert_eq!(plate_of(&app, local).as_deref(), Some("Host"));

    // held out of the world while a scene loads
    app.world_mut()
        .entity_mut(local)
        .insert(OutOfWorld::default());
    app.update();
    assert_eq!(plate_of(&app, local), None);
    assert_eq!(plate_of(&app, visitor), None);
    app.world_mut().entity_mut(local).remove::<OutOfWorld>();
    app.update();
    assert_eq!(plate_of(&app, local).as_deref(), Some("Host"));
}

#[test]
fn a_portable_experience_plates_no_one() {
    let mut app = plate_app();
    scene_on(&mut app, HOME);
    let local = local_on(&mut app, HOME);
    let visitor = visitor_on(&mut app, FIRST_VISITOR, HOME);
    // live everywhere, on no parcel
    let portable = app.world_mut().spawn_empty().id();
    app.world_mut()
        .resource_mut::<LiveScenes>()
        .scenes
        .insert("portable".to_owned(), portable);
    app.world_mut().resource_mut::<PortableScenes>().insert(
        "portable".to_owned(),
        PortableSource::new("portable".to_owned(), None, None, false),
    );
    ask_for_plate(&mut app, portable, SceneEntityId::PLAYER, "Host");
    ask_for_plate(&mut app, portable, FIRST_VISITOR, "Guest");
    app.update();
    assert_eq!(plate_of(&app, local), None);
    assert_eq!(plate_of(&app, visitor), None);
}

#[test]
fn a_plate_follows_what_the_scene_writes_and_goes_with_it() {
    let mut app = plate_app();
    let scene = scene_on(&mut app, HOME);
    let local = local_on(&mut app, HOME);
    let asked = ask_for_plate(&mut app, scene, SceneEntityId::PLAYER, "Student");
    app.update();
    assert_eq!(plate_of(&app, local).as_deref(), Some("Student"));

    app.world_mut()
        .entity_mut(asked)
        .insert(scene_plate("Teacher"));
    app.update();
    assert_eq!(plate_of(&app, local).as_deref(), Some("Teacher"));

    // the component deleted
    app.world_mut().entity_mut(asked).remove::<AvatarNametag>();
    app.update();
    assert_eq!(plate_of(&app, local), None);

    // the entity deleted, or its scene unloaded
    app.world_mut()
        .entity_mut(asked)
        .insert(scene_plate("Teacher"));
    app.update();
    assert_eq!(plate_of(&app, local).as_deref(), Some("Teacher"));
    app.world_mut().entity_mut(asked).despawn();
    app.update();
    assert_eq!(plate_of(&app, local), None);
}

/// An avatar with no name, `distance` in front of the camera.
fn spawn_nameless(app: &mut App, distance: f32) -> Entity {
    let world = app.world_mut();
    let attach_points = AttachPoints::new(&mut world.commands());
    world.flush();
    world
        .spawn((
            GlobalTransform::from_translation(Vec3::new(0.0, 0.0, -distance)),
            attach_points,
        ))
        .id()
}

fn name_shown(app: &App, player: Entity) -> bool {
    let tag = tag_of(app, player).unwrap();
    app.world().get::<Nametag>(tag).unwrap().name_shown
}

#[test]
fn an_avatar_has_one_tag_for_its_name_and_its_plate() {
    let mut app = tag_app();
    let guide = spawn_nameless(&mut app, 10.0);
    assert_eq!(tag_of(&app, guide), None);

    // a plate alone
    app.world_mut()
        .entity_mut(guide)
        .insert(scene_plate("Guide").0);
    app.world_mut().flush();
    let tag = tag_of(&app, guide).expect("a plated avatar has a tag");
    app.update();
    assert!(shown(&app, guide) && !name_shown(&app, guide));

    app.world_mut().entity_mut(guide).insert(content("Ada"));
    app.update();
    assert_eq!(tags(&mut app), vec![tag]);
    assert!(shown(&app, guide) && name_shown(&app, guide));

    app.world_mut().entity_mut(guide).remove::<NametagContent>();
    app.update();
    assert_eq!(tags(&mut app), vec![tag]);
    assert!(shown(&app, guide) && !name_shown(&app, guide));

    app.world_mut().entity_mut(guide).remove::<NametagPlate>();
    app.update();
    assert!(tags(&mut app).is_empty());

    // both at once
    app.world_mut()
        .entity_mut(guide)
        .insert((content("Ada"), scene_plate("Guide").0));
    app.update();
    assert_eq!(tags(&mut app).len(), 1);
    // the plate taken away leaves the name its tag
    app.world_mut().entity_mut(guide).remove::<NametagPlate>();
    app.update();
    assert_eq!(tags(&mut app).len(), 1);
    assert!(shown(&app, guide) && name_shown(&app, guide));
}

#[test]
fn an_area_that_hides_names_leaves_the_plate() {
    let mut app = tag_app();
    let ada = spawn_player(&mut app, "Ada", 10.0);
    app.world_mut()
        .entity_mut(ada)
        .insert((scene_plate("VIP").0, HiddenNametag));
    app.update();
    assert!(shown(&app, ada) && !name_shown(&app, ada));
    app.world_mut().entity_mut(ada).remove::<HiddenNametag>();
    app.update();
    assert!(shown(&app, ada) && name_shown(&app, ada));

    // what hides the avatar, the toggle and the range hide the plate with the name
    app.world_mut().entity_mut(ada).insert(PlayerModifiers {
        hide: true,
        ..Default::default()
    });
    app.update();
    assert!(!shown(&app, ada));
    app.world_mut().entity_mut(ada).remove::<PlayerModifiers>();
    app.world_mut().resource_mut::<NametagsVisible>().0 = false;
    app.update();
    assert!(!shown(&app, ada));
    app.world_mut().resource_mut::<NametagsVisible>().0 = true;
    move_player(&mut app, ada, MAX_DISTANCE + 1.0);
    app.update();
    assert!(!shown(&app, ada));
    move_player(&mut app, ada, 10.0);
    app.update();
    assert!(shown(&app, ada) && name_shown(&app, ada));
}

fn built(app: &App, player: Entity) -> &Built {
    let tag = tag_of(app, player).unwrap();
    let nametag = app.world().get::<Nametag>(tag).unwrap();
    nametag.built.as_ref().expect("a built tag")
}

fn reach(app: &App, player: Entity) -> f32 {
    let tag = tag_of(app, player).unwrap();
    app.world().get::<Nametag>(tag).unwrap().reach
}

fn label_width(app: &App, player: Entity) -> Option<f32> {
    let tag = tag_of(app, player).unwrap();
    let nametag = app.world().get::<Nametag>(tag).unwrap();
    nametag.plate.as_ref().map(|label| label.size.x)
}

#[test]
fn a_plate_is_drawn_over_the_name_and_alone_when_the_name_is_hidden() {
    let mut app = build_app();
    let ada = spawn_player(&mut app, "Ada", 10.0);
    settle(&mut app, ada);
    let named = reach(&app, ada);
    assert!(built(&app, ada).plate.is_none());

    app.world_mut().entity_mut(ada).insert(scene_plate("VIP").0);
    settle(&mut app, ada);
    assert_eq!(
        built(&app, ada).plate.as_ref().map(|plate| &*plate.label),
        Some("VIP")
    );
    assert!(built(&app, ada).content.is_some());
    // it stands over the name's box
    let plated = reach(&app, ada);
    assert!(plated > named);

    // a message opens under it
    say(&mut app, ada, "hello there");
    settle(&mut app, ada);
    advance(&mut app, SHOWN + 0.2);
    assert!(reach(&app, ada) > plated);
    assert!(built(&app, ada).plate.is_some());

    // the name hidden: the plate alone, in its place, and no message
    app.world_mut().entity_mut(ada).insert(HiddenNametag);
    settle(&mut app, ada);
    assert!(draws(&app, ada));
    assert!(built(&app, ada).content.is_none() && built(&app, ada).plate.is_some());
    assert_eq!(built(&app, ada).progress, BubbleProgress::default());
    assert!(reach(&app, ada) < named);

    app.world_mut().entity_mut(ada).remove::<NametagPlate>();
    app.update();
    assert!(!draws(&app, ada));
    // the name back, with the rest of its message
    app.world_mut().entity_mut(ada).remove::<HiddenNametag>();
    settle(&mut app, ada);
    assert!(built(&app, ada).plate.is_none());
    assert!(reach(&app, ada) > named);
    advance(&mut app, BUBBLE_LIFETIME);
    advance(&mut app, SHOWN);
    assert!(!app.world().entity(ada).contains::<ChatBubble>());
    assert_eq!(reach(&app, ada), named);
}

#[test]
fn a_name_that_went_leaves_the_plate_none_of_its_glyphs() {
    let mut app = build_app();
    let ada = spawn_player(&mut app, "Ada", 10.0);
    app.world_mut().entity_mut(ada).insert(scene_plate("VIP").0);
    settle(&mut app, ada);
    let tag = tag_of(&app, ada).unwrap();
    assert!(app.world().get::<Nametag>(tag).unwrap().name.is_some());

    // only hidden: it may come back
    app.world_mut().entity_mut(ada).insert(HiddenNametag);
    settle(&mut app, ada);
    assert!(app.world().get::<Nametag>(tag).unwrap().name.is_some());

    app.world_mut().entity_mut(ada).remove::<NametagContent>();
    settle(&mut app, ada);
    let nametag = app.world().get::<Nametag>(tag).unwrap();
    assert!(nametag.name.is_none() && nametag.plate.is_some());
    assert!(built(&app, ada).content.is_none() && built(&app, ada).plate.is_some());
}

#[test]
fn a_plate_is_drawn_for_any_label_a_scene_writes() {
    let mut app = build_app();
    let guide = spawn_nameless(&mut app, 10.0);

    // nothing written: a plate all the same
    app.world_mut().entity_mut(guide).insert(scene_plate("").0);
    settle(&mut app, guide);
    assert!(built(&app, guide).plate.is_some());
    assert_eq!(label_width(&app, guide), None);
    let empty = reach(&app, guide);

    // spaces widen it
    app.world_mut()
        .entity_mut(guide)
        .insert(scene_plate("            ").0);
    settle(&mut app, guide);
    assert!(reach(&app, guide) > empty);

    // a long label is cut to the plate's widest
    app.world_mut()
        .entity_mut(guide)
        .insert(scene_plate(&"Club Owner ".repeat(11)).0);
    settle(&mut app, guide);
    let width = label_width(&app, guide).expect("a label");
    assert!(width <= layout::PLATE_MAX_LABEL_WIDTH && width > 200.0);
}
