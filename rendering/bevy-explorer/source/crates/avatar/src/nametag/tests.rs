use bevy::ecs::system::RunSystemOnce;
use common::profile::SerializedProfile;

use super::*;

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
fn a_name_shows_in_range_in_front_of_the_camera() {
    let view = |first_person| TagView {
        position: Vec3::ZERO,
        forward: Vec3::NEG_Z,
        first_person,
    };
    let third = view(false);
    assert!(third.shows(Vec3::new(0.0, 0.0, -2.0), false));
    assert!(third.shows(Vec3::new(0.0, 0.0, -40.0), false));
    assert!(!third.shows(Vec3::new(0.0, 0.0, -1.9), false));
    assert!(!third.shows(Vec3::new(0.0, 0.0, -40.1), false));
    // behind the camera
    assert!(!third.shows(Vec3::new(0.0, 0.0, 5.0), false));
    // off to the side still counts as in front
    assert!(third.shows(Vec3::new(30.0, 0.0, -1.0), false));
    // the local player's own, in third person only
    assert!(third.shows(Vec3::new(0.0, 0.0, -3.0), true));
    assert!(!view(true).shows(Vec3::new(0.0, 0.0, -3.0), true));
    assert!(view(true).shows(Vec3::new(0.0, 0.0, -3.0), false));
}

#[test]
fn the_voice_badge_follows_speech_mutes_and_the_microphone() {
    let nearby = Some(Speaking { in_call: false });
    let call = Some(Speaking { in_call: true });
    assert_eq!(voice_badge(None, false, false), Voice::Off);
    assert_eq!(voice_badge(nearby, false, false), Voice::Speaking);
    assert_eq!(voice_badge(nearby, true, false), Voice::Hushed);
    // a nearby mute leaves a call audible
    assert_eq!(voice_badge(call, true, false), Voice::Speaking);
    // muted and silent: nothing to mark
    assert_eq!(voice_badge(None, true, false), Voice::Off);
    // the local user's open microphone shows before they speak
    assert_eq!(voice_badge(None, false, true), Voice::Idle);
    assert_eq!(voice_badge(nearby, false, true), Voice::Speaking);
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

    // no name given reads NPC; an empty one asks for no tag
    let unnamed = NametagContent::from_shape(&shape("guide-0001", None)).unwrap();
    assert_eq!(unnamed.name, "NPC");
    assert!(NametagContent::from_shape(&shape("guide-0001", Some(""))).is_none());

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

    // an empty name takes the tag away, and a name brings it back
    for (name, shown) in [("", None), ("Guide", Some("Guide"))] {
        app.world_mut()
            .entity_mut(guide)
            .insert(AvatarShape(shape("guide-0001", Some(name))));
        app.update();
        assert_eq!(name_of(&app, guide).as_deref(), shown, "{name:?}");
    }

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

/// A world with the camera at the origin looking down -z, and the systems that hand tags out.
fn pool_app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .insert_resource(NametagsVisible(true))
        .insert_resource(NametagAssets {
            material: Handle::default(),
            font: Handle::default(),
            bound: [None; 2],
        })
        .add_systems(Update, acquire_and_release_tags)
        .add_observer(release_tag);
    app.world_mut()
        .spawn((Transform::default(), PrimaryCamera::default()));
    app
}

fn spawn_player(app: &mut App, name: &str, distance: f32) -> Entity {
    let world = app.world_mut();
    let attach_points = AttachPoints::new(&mut world.commands());
    world.flush();
    world
        .spawn((
            GlobalTransform::from_translation(Vec3::new(0.0, 0.0, -distance)),
            content(name),
            attach_points,
        ))
        .id()
}

fn move_player(app: &mut App, player: Entity, distance: f32) {
    *app.world_mut().get_mut::<GlobalTransform>(player).unwrap() =
        GlobalTransform::from_translation(Vec3::new(0.0, 0.0, -distance));
}

fn tag_of(app: &App, player: Entity) -> Option<Entity> {
    app.world()
        .get::<ShownNametag>(player)
        .map(ShownNametag::tag)
}

fn free_tags(app: &mut App) -> Vec<Entity> {
    app.world_mut()
        .query_filtered::<Entity, (With<Nametag>, Without<NametagOf>)>()
        .iter(app.world())
        .collect()
}

#[test]
fn a_player_leaving_the_range_gives_their_tag_to_the_next() {
    let mut app = pool_app();
    let ada = spawn_player(&mut app, "Ada", 10.0);
    app.update();
    let tag = tag_of(&app, ada).expect("a player in range has a tag");
    assert!(free_tags(&mut app).is_empty());

    // a tag in use carries the last player's state: a bubble, and what was built
    app.world_mut().entity_mut(tag).insert((
        ChatBubble {
            kind: BubbleKind::Nearby,
            message: "hello".to_owned(),
            message_id: "1".to_owned(),
            mention: false,
            recipient: None,
            stale: Stale::Message,
            texts: None,
            animation: None,
        },
        AwaitingGlyphs,
        Visibility::Inherited,
    ));
    app.world_mut().get_mut::<Nametag>(tag).unwrap().built = Some(Built {
        content: content("Ada"),
        voice: Voice::Off,
        generation: 0,
    });

    move_player(&mut app, ada, 41.0);
    app.update();
    assert_eq!(tag_of(&app, ada), None);
    assert_eq!(free_tags(&mut app), [tag]);
    let released = app.world().entity(tag);
    assert!(!released.contains::<ChatBubble>() && !released.contains::<AwaitingGlyphs>());
    assert!(released.get::<Nametag>().unwrap().built.is_none());
    assert_eq!(released.get::<Visibility>(), Some(&Visibility::Hidden));

    // the pooled tag serves whoever comes next, without a new entity
    let grace = spawn_player(&mut app, "Grace", 5.0);
    app.update();
    assert_eq!(tag_of(&app, grace), Some(tag));
    assert!(free_tags(&mut app).is_empty());
}

#[test]
fn a_player_who_disconnects_frees_their_tag() {
    let mut app = pool_app();
    let ada = spawn_player(&mut app, "Ada", 10.0);
    app.update();
    let tag = tag_of(&app, ada).unwrap();
    app.world_mut()
        .entity_mut(tag)
        .insert(Visibility::Inherited);

    app.world_mut().despawn(ada);
    app.update();
    assert_eq!(free_tags(&mut app), [tag]);
    assert_eq!(
        app.world().get::<Visibility>(tag),
        Some(&Visibility::Hidden)
    );
}

#[test]
fn hiding_takes_the_tag_and_showing_gives_one_back() {
    let mut app = pool_app();
    let ada = spawn_player(&mut app, "Ada", 10.0);
    app.update();
    assert!(tag_of(&app, ada).is_some());

    // the nametags toggle
    app.world_mut().resource_mut::<NametagsVisible>().0 = false;
    app.update();
    assert_eq!(tag_of(&app, ada), None);
    app.world_mut().resource_mut::<NametagsVisible>().0 = true;
    app.update();
    assert!(tag_of(&app, ada).is_some());

    // a scene area hiding nametags, the avatar itself hidden, and the nearest-avatars cap
    app.world_mut().entity_mut(ada).insert(HiddenNametag);
    app.update();
    assert_eq!(tag_of(&app, ada), None);
    app.world_mut().entity_mut(ada).remove::<HiddenNametag>();
    app.world_mut().entity_mut(ada).insert(PlayerModifiers {
        hide: true,
        ..Default::default()
    });
    app.update();
    assert_eq!(tag_of(&app, ada), None);
    app.world_mut().entity_mut(ada).remove::<PlayerModifiers>();
    app.world_mut().entity_mut(ada).insert(BeyondMaxAvatars);
    app.update();
    assert_eq!(tag_of(&app, ada), None);
    app.world_mut().entity_mut(ada).remove::<BeyondMaxAvatars>();
    app.update();
    assert!(tag_of(&app, ada).is_some());

    // a player with nothing to say (their profile gone) keeps no tag
    app.world_mut().entity_mut(ada).remove::<NametagContent>();
    app.update();
    assert_eq!(tag_of(&app, ada), None);
    assert_eq!(free_tags(&mut app).len(), 1);
}

#[test]
fn first_person_hides_only_the_local_players_own_name() {
    let mut app = pool_app();
    let me = spawn_player(&mut app, "Me", 3.0);
    app.world_mut()
        .entity_mut(me)
        .insert(PrimaryUser::default());
    let other = spawn_player(&mut app, "Other", 3.0);
    app.update();
    assert!(tag_of(&app, me).is_some() && tag_of(&app, other).is_some());

    let mut cameras = app.world_mut().query::<&mut PrimaryCamera>();
    cameras.single_mut(app.world_mut()).unwrap().distance = 0.0;
    app.update();
    assert_eq!(tag_of(&app, me), None);
    assert!(tag_of(&app, other).is_some());
}

#[test]
fn the_pool_keeps_a_bounded_number_of_free_tags() {
    let mut app = pool_app();
    let players = (0..MAX_FREE_TAGS + 8)
        .map(|index| spawn_player(&mut app, &format!("P{index}"), 10.0))
        .collect::<Vec<_>>();
    app.update();
    assert!(players.iter().all(|player| tag_of(&app, *player).is_some()));

    app.world_mut().resource_mut::<NametagsVisible>().0 = false;
    app.update();
    // the release lands this frame; the surplus goes the next
    app.update();
    assert_eq!(free_tags(&mut app).len(), MAX_FREE_TAGS);
}

#[test]
fn a_tag_stands_on_its_anchor_facing_the_camera() {
    let mut app = pool_app();
    let ada = spawn_player(&mut app, "Ada", 10.0);
    app.update();
    let tag = tag_of(&app, ada).unwrap();

    let anchor = app.world().get::<AttachPoints>(ada).unwrap().nametag;
    app.world_mut()
        .get_mut::<Transform>(anchor)
        .unwrap()
        .translation = Vec3::new(0.1, 1.9, 0.0);
    let camera_rotation = Quat::from_rotation_y(0.3);
    let fov = 60f32.to_radians();
    let mut cameras = app
        .world_mut()
        .query_filtered::<Entity, With<PrimaryCamera>>();
    let camera = cameras.single(app.world()).unwrap();
    app.world_mut().entity_mut(camera).insert((
        Transform::from_rotation(camera_rotation),
        Projection::Perspective(PerspectiveProjection {
            fov,
            ..Default::default()
        }),
    ));

    app.world_mut().run_system_once(place_tags).unwrap();
    let transform = *app.world().get::<Transform>(tag).unwrap();
    assert!(transform.translation.distance(Vec3::new(0.1, 1.9, -10.0)) < 1e-5);
    assert_eq!(transform.rotation, camera_rotation);
    assert!((transform.scale.x - tag_scale(10.0, fov)).abs() < 1e-7);
    assert_eq!(
        app.world().get::<MeshTag>(tag).unwrap().0,
        opacity_mesh_tag(1.0)
    );

    // farther off: larger in the world, and fading
    move_player(&mut app, ada, 30.0);
    app.world_mut().run_system_once(place_tags).unwrap();
    let transform = *app.world().get::<Transform>(tag).unwrap();
    assert!((transform.scale.x - tag_scale(30.0, fov)).abs() < 1e-7);
    assert_eq!(
        app.world().get::<MeshTag>(tag).unwrap().0,
        opacity_mesh_tag(0.5)
    );
}

/// `PlayerUpdate` has moved the player by the time tags are placed, and nothing has propagated
/// the move to its `GlobalTransform` yet.
#[test]
fn a_tag_follows_a_player_that_moved_this_frame() {
    let mut app = pool_app();
    let ada = spawn_player(&mut app, "Ada", 10.0);
    app.update();
    let tag = tag_of(&app, ada).unwrap();
    // the anchor at the player's feet: the tag stands where the player is
    let anchor = app.world().get::<AttachPoints>(ada).unwrap().nametag;
    app.world_mut()
        .get_mut::<Transform>(anchor)
        .unwrap()
        .translation = Vec3::ZERO;
    let fov = 60f32.to_radians();
    let mut cameras = app
        .world_mut()
        .query_filtered::<Entity, With<PrimaryCamera>>();
    let camera = cameras.single(app.world()).unwrap();
    app.world_mut()
        .entity_mut(camera)
        .insert(Projection::Perspective(PerspectiveProjection {
            fov,
            ..Default::default()
        }));

    // the global transform still says 10 m
    app.world_mut()
        .entity_mut(ada)
        .insert(Transform::from_xyz(0.0, 0.0, -30.0));
    app.world_mut().run_system_once(place_tags).unwrap();
    let transform = *app.world().get::<Transform>(tag).unwrap();
    assert!(transform.translation.distance(Vec3::new(0.0, 0.0, -30.0)) < 1e-5);
    assert!((transform.scale.x - tag_scale(30.0, fov)).abs() < 1e-7);

    // under a parent the player's own transform is not its place in the world
    let parent = app.world_mut().spawn_empty().id();
    app.world_mut().entity_mut(ada).insert(ChildOf(parent));
    app.world_mut().run_system_once(place_tags).unwrap();
    let transform = *app.world().get::<Transform>(tag).unwrap();
    assert!(transform.translation.distance(Vec3::new(0.0, 0.0, -10.0)) < 1e-5);
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
    app.add_plugins(MaterialPlugin::<NametagMaterial>::default());
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
    let (mesh, aabb) = quad_mesh(&[
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
        MeshTag(opacity_mesh_tag(0.5)),
        aabb,
        NotShadowCaster,
        Transform::from_scale(Vec3::splat(0.01)),
    ));

    compile_pipelines(&mut app);
    assert!(
        fragment_pipelines(&app, "embedded://shaders/nametag.wgsl") >= 1,
        "the nametag was not drawn"
    );
}
