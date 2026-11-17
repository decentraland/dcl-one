use bevy::ecs::system::RunSystemOnce;
use common::{
    profile::SerializedProfile,
    structs::{CameraOverride, CinematicSettings},
};

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
fn a_name_shows_in_range_of_the_camera() {
    let view = |first_person| TagView {
        position: Vec3::ZERO,
        first_person,
    };
    let third = view(false);
    assert!(third.shows(Vec3::new(0.0, 0.0, -2.0), false));
    assert!(third.shows(Vec3::new(0.0, 0.0, -40.0), false));
    assert!(!third.shows(Vec3::new(0.0, 0.0, -1.9), false));
    assert!(!third.shows(Vec3::new(0.0, 0.0, -40.1), false));
    // behind the camera or off to the side: the frustum's to cull
    assert!(third.shows(Vec3::new(0.0, 0.0, 5.0), false));
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
        .add_observer(despawn_tag);
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
        message: message.to_owned(),
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
    assert_eq!(app.world().get::<ChatBubble>(ada).unwrap().message, "hello");
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
    assert_eq!(nametag.built.as_ref().unwrap().content.name, "Ada Lovelace");
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
