use std::{
    sync::{Arc, LazyLock, Mutex},
    time::Duration,
};

use alloy_core::primitives::Address;
use bevy::{
    text::{cosmic_text::fontdb::Source, CosmicFontSystem, TextPipeline},
    time::TimeUpdateStrategy,
};
use bevy_console::ConsoleCommandEntered;
use common::{
    rpc::RpcStreamReceiver,
    structs::{PlayerModifiers, PrimaryPlayerRes},
    sync::MutexExt,
};
use scene_runner::update_world::text_shape::{GlyphAtlases, Page};

use crate::BeyondMaxAvatars;
use comms::{chat_reaction::emoji_to_atlas_index, NetworkMessage, TransportType};
use tokio::sync::mpsc;

use super::*;

const FRAME: f32 = 0.05;
/// The address every stand-in peer reacts under.
const PEER: Address = Address::repeat_byte(9);

/// The plugin in an app of its own, with a signed-in local player and a stream to the HUD.
struct Harness {
    app: App,
    local: Entity,
    me: Address,
    hud: RpcStreamReceiver<ReactionData>,
}

/// The wire index of each reaction the local player was reported to make, in order.
#[derive(Resource, Default)]
struct Reported(Vec<i32>);

fn harness() -> Harness {
    let mut wallet = Wallet::default();
    wallet.finalize_as_guest_with_seed([3; 32]);
    let me = wallet.address().unwrap();

    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        bevy::asset::AssetPlugin::default(),
        console::ConsolePlugin {
            add_bevy_console: false,
        },
    ))
    .add_event::<SystemApi>()
    .add_event::<ReactionEvent>()
    // what a block or ban is asked of
    .init_resource::<ban_block::PlatformBanStatus>()
    .init_resource::<ban_block::SceneAdmission>()
    // what the particles are shaped and drawn with
    .init_asset::<Font>()
    .init_asset::<Image>()
    .init_asset::<Mesh>()
    .init_resource::<TextPipeline>()
    .init_resource::<CosmicFontSystem>()
    .init_resource::<AppConfig>()
    .insert_resource(NametagsVisible(true))
    .insert_resource(wallet)
    .init_resource::<Reported>()
    .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f32(
        FRAME,
    )))
    .add_plugins(ReactionPlugin);
    scene_runner::update_world::text_shape::add_glyph_atlas_maintenance(&mut app);
    // the colour emoji font, as the Explorer makes it at startup
    let font = app.world_mut().resource_mut::<Assets<Font>>().add(Font {
        data: EMOJI_BYTES.clone(),
    });
    app.insert_resource(EmojiFont(font));
    app.add_systems(
        Update,
        |mut reacted: EventReader<PlayerReacted>, mut reported: ResMut<Reported>| {
            reported
                .0
                .extend(reacted.read().map(|reaction| reaction.emoji_index));
        },
    );
    // as the engine spawns them: the outbox is the plugin's to add
    let local = app
        .world_mut()
        .spawn((PrimaryUser::default(), PlayerAddress(me)))
        .id();
    app.insert_resource(PrimaryPlayerRes(local));

    let (sender, hud) = RpcStreamSender::channel();
    app.world_mut()
        .send_event(SystemApi::GetReactionStream(sender));
    app.update();

    Harness {
        app,
        local,
        me,
        hud,
    }
}

impl Harness {
    fn pick(&mut self, emoji: &str) {
        self.app
            .world_mut()
            .send_event(SystemApi::SendReaction(emoji.to_owned()));
    }

    /// A stand-in for a peer: what the comms decode gives every player it hears from, but for
    /// the `ForeignPlayer` that only comms can make.
    fn peer(&mut self) -> Entity {
        self.app
            .world_mut()
            .spawn((ReactionGuard::default(), PlayerAddress(PEER)))
            .id()
    }

    fn hear(&mut self, sender: Entity, emoji: &str, timestamp: f32, count: i32) {
        self.app.world_mut().send_event(ReactionEvent {
            sender,
            reaction: rfc4::Reaction {
                emoji_index: emoji_to_atlas_index(emoji),
                timestamp,
                count,
            },
        });
    }

    fn transport(&mut self, transport_type: TransportType) -> mpsc::Receiver<NetworkMessage> {
        let (sender, receiver) = mpsc::channel(64);
        self.app.world_mut().spawn(Transport {
            transport_type,
            sender,
            control: None,
            context: Entity::PLACEHOLDER,
        });
        receiver
    }

    /// The reactions that played in the frame just run.
    fn played(&self) -> Vec<ReactionPlayed> {
        self.app
            .world()
            .resource::<Events<ReactionPlayed>>()
            .iter_current_update_events()
            .copied()
            .collect()
    }

    fn streamed(&mut self) -> Vec<(String, String)> {
        std::iter::from_fn(|| self.hud.try_recv().ok())
            .map(|reaction| (reaction.from, reaction.emoji))
            .collect()
    }

    /// What analytics would hear of: the reactions reported since the last call.
    fn reported(&mut self) -> Vec<i32> {
        std::mem::take(&mut self.app.world_mut().resource_mut::<Reported>().0)
    }

    fn stats(&self) -> &ReactionStats {
        self.app.world().resource::<ReactionStats>()
    }

    /// Run frames for `seconds`.
    fn run(&mut self, seconds: f32) {
        for _ in 0..(seconds / FRAME).round() as u32 {
            self.app.update();
        }
    }
}

fn sent(wire: &mut mpsc::Receiver<NetworkMessage>) -> Vec<bool> {
    std::iter::from_fn(|| wire.try_recv().ok())
        .map(|message| message.unreliable)
        .collect()
}

#[test]
fn a_pick_plays_at_once_and_is_sent_once_the_picks_stop() {
    let mut harness = harness();
    let mut wire = harness.transport(TransportType::Livekit);
    let me = format!("{:#x}", harness.me);

    harness.pick("👍");
    harness.app.update();
    assert_eq!(
        harness.played(),
        [ReactionPlayed {
            player: harness.local,
            emoji: "👍",
        }]
    );
    assert_eq!(harness.streamed(), [(me.clone(), "👍".to_owned())]);
    assert!(sent(&mut wire).is_empty());

    harness.pick("👍");
    harness.pick("🔥");
    harness.app.update();
    assert_eq!(harness.streamed().len(), 2);
    assert!(sent(&mut wire).is_empty());

    // one reliable message for each emoji
    harness.run(0.3);
    assert_eq!(sent(&mut wire), [false, false]);
    assert_eq!(harness.stats().sent, 2);
    harness.run(1.0);
    assert!(sent(&mut wire).is_empty());

    // each pick is reported once, as the index it is sent as
    let (thumb, fire) = (emoji_to_atlas_index("👍"), emoji_to_atlas_index("🔥"));
    assert_eq!(harness.reported(), [thumb, thumb, fire]);
}

#[test]
fn a_pick_with_no_room_to_carry_it_plays_and_is_not_counted_as_sent() {
    let mut harness = harness();
    // a transport reactions do not go by
    let mut island = harness.transport(TransportType::Archipelago);
    harness.pick("👍");
    harness.run(0.5);
    assert_eq!(harness.stats().picked, 1);
    assert_eq!(harness.stats().played, 1);
    assert_eq!(harness.stats().sent, 0);
    assert!(sent(&mut island).is_empty());
}

#[test]
fn picks_go_where_nearby_chat_goes() {
    let mut harness = harness();
    let mut room = harness.transport(TransportType::Livekit);
    let mut socket = harness.transport(TransportType::WebsocketRoom);
    let mut island = harness.transport(TransportType::Archipelago);
    let mut pulse = harness.transport(TransportType::Pulse);
    harness.pick("👍");
    harness.run(0.5);
    assert_eq!(sent(&mut room).len(), 1);
    assert_eq!(sent(&mut socket).len(), 1);
    assert!(sent(&mut island).is_empty());
    assert!(sent(&mut pulse).is_empty());
}

#[test]
fn an_emoji_the_wire_does_not_carry_does_nothing_anywhere() {
    let mut harness = harness();
    let mut wire = harness.transport(TransportType::Livekit);
    for picked in ["🇬🇧", "thumbs up", "", "👍👍"] {
        harness.pick(picked);
    }
    harness.run(1.0);
    assert!(harness.streamed().is_empty());
    assert!(sent(&mut wire).is_empty());
    assert!(harness.reported().is_empty());
    assert_eq!(harness.stats().refused_picks, 4);
    assert_eq!(harness.stats().picked, 0);
    assert_eq!(harness.stats().played, 0);
}

#[test]
fn what_plays_is_the_emoji_the_others_will_see() {
    let mut harness = harness();
    // the picker's heart has no variation selector; the wire's does
    harness.pick("\u{2764}");
    harness.app.update();
    assert_eq!(harness.played()[0].emoji, "\u{2764}\u{fe0f}");
}

#[test]
fn no_one_signed_in_reacts_as_no_one() {
    let mut harness = harness();
    harness
        .app
        .world_mut()
        .resource_mut::<Wallet>()
        .disconnect();
    harness.pick("👍");
    harness.app.update();
    assert!(harness.played().is_empty());
    assert_eq!(harness.stats().refused_picks, 1);
}

#[test]
fn a_peers_reactions_play_one_at_a_time() {
    let mut harness = harness();
    let peer = harness.peer();
    harness.hear(peer, "🔥", 7.0, 3);
    harness.app.update();
    // the first at once, on the peer
    assert_eq!(
        harness
            .played()
            .iter()
            .map(|played| (played.player, played.emoji))
            .collect::<Vec<_>>(),
        [(peer, "🔥")]
    );
    assert!(harness.app.world().get::<ReactionQueue>(peer).is_some());

    harness.run(0.5);
    assert_eq!(
        harness.streamed(),
        vec![(format!("{PEER:#x}"), "🔥".to_owned()); 3]
    );
    assert_eq!(harness.stats().played, 3);
    // a peer's reactions are no pick of the local user's
    assert_eq!(harness.stats().picked, 0);
    // nothing is kept on a peer once the gap after their last has run out
    harness.run(0.3);
    assert!(harness.app.world().get::<ReactionQueue>(peer).is_none());
}

#[test]
fn messages_of_one_reaction_each_play_a_gap_apart_too() {
    let mut harness = harness();
    let peer = harness.peer();
    // a message a frame, each with a stamp of its own
    for message in 0..4 {
        harness.hear(peer, "🔥", 1.0 + message as f32, 1);
        harness.app.update();
        // the first at once; the next two arrive inside its gap and wait
        if message < 3 {
            assert_eq!(harness.stats().played, 1);
        }
    }
    // then one for each gap that runs out
    harness.run(0.15);
    assert_eq!(harness.stats().played, 2);
    for played in 3..=4 {
        harness.run(0.2);
        assert_eq!(harness.stats().played, played);
    }
}

#[test]
fn several_messages_in_a_frame_share_their_senders_queue() {
    let mut harness = harness();
    let peer = harness.peer();
    harness.hear(peer, "🔥", 1.0, 1);
    harness.hear(peer, "👍", 2.0, 1);
    // the copy that came by the other room
    harness.hear(peer, "🔥", 1.0, 1);
    harness.run(1.0);
    assert_eq!(
        harness
            .streamed()
            .into_iter()
            .map(|(_, emoji)| emoji)
            .collect::<Vec<_>>(),
        ["🔥", "👍"]
    );
    assert_eq!(harness.stats().received, 3);
    assert_eq!(harness.stats().dropped.duplicate, 1);
}

#[test]
fn a_blocked_sender_plays_nothing() {
    let mut harness = harness();
    let peer = harness.peer();
    harness
        .app
        .world_mut()
        .entity_mut(peer)
        .insert(BlockedOrBanned);
    harness.hear(peer, "🔥", 1.0, 5);
    harness.run(1.0);
    assert!(harness.streamed().is_empty());
    assert_eq!(harness.stats().dropped.blocked, 1);
}

#[test]
fn an_app_that_keeps_no_block_list_takes_no_peers_reactions() {
    let mut harness = harness();
    harness
        .app
        .world_mut()
        .remove_resource::<ban_block::PlatformBanStatus>();
    let peer = harness.peer();
    harness.hear(peer, "🔥", 1.0, 2);
    // nothing is asked of a list that is not there
    harness.run(1.0);
    assert_eq!(harness.stats().received, 0);
    assert_eq!(harness.stats().played, 0);
    // the local user's own picks play all the same
    harness.pick("👍");
    harness.app.update();
    assert_eq!(harness.stats().played, 1);
}

#[test]
fn a_sender_blocked_while_their_reactions_wait_plays_no_more() {
    let mut harness = harness();
    let peer = harness.peer();
    harness.hear(peer, "🔥", 1.0, 5);
    harness.app.update();
    assert_eq!(harness.streamed().len(), 1);

    harness
        .app
        .world_mut()
        .entity_mut(peer)
        .insert(BlockedOrBanned);
    harness.run(2.0);
    assert!(harness.streamed().is_empty());
    assert!(harness.app.world().get::<ReactionQueue>(peer).is_none());
    assert_eq!(harness.stats().dropped.blocked, 4);
}

#[test]
fn a_sender_who_leaves_takes_their_waiting_reactions() {
    let mut harness = harness();
    let peer = harness.peer();
    harness.hear(peer, "🔥", 1.0, 5);
    harness.app.update();
    harness.app.world_mut().despawn(peer);
    // a message of theirs still on its way in finds no one
    harness.hear(peer, "👍", 2.0, 1);
    harness.run(2.0);
    assert_eq!(harness.streamed().len(), 1);
}

// The world lane: the particles over avatars.

/// Where the nametag anchor of a player made by [`AttachPoints::new`] stands over their feet.
const NAMETAG_HEIGHT: f32 = 2.2;

/// The colour font every text falls back to for emoji, as the Explorer loads it.
static EMOJI_BYTES: LazyLock<Arc<Vec<u8>>> = LazyLock::new(|| {
    let path = concat!(
        env!("DCL_ONE_ROOT"),
        "/rendering/bevy-explorer/source/crates/ui_core/src/NotoColorEmoji.ttf"
    );
    Arc::new(std::fs::read(path).expect("the emoji font is in the tree"))
});

impl Harness {
    /// The emoji font there for every text to fall back to, as the Explorer has it, and the
    /// camera at the origin. The local player gets an avatar where they stand.
    fn with_avatars() -> Self {
        let mut harness = harness();
        let app = &mut harness.app;
        app.world_mut()
            .resource_mut::<CosmicFontSystem>()
            .db_mut()
            .load_font_source(Source::Binary(EMOJI_BYTES.clone()));

        app.world_mut()
            .spawn((GlobalTransform::default(), PrimaryCamera::default()));
        let local = harness.local;
        harness.give_avatar(local, Vec3::ZERO);
        harness
    }

    /// Lay the emoji out in a text font, the nametags', with the emoji font to fall back to.
    fn lay_out_in_a_text_font(&mut self) {
        let font = std::fs::read(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/nametag/fonts/Inter_18pt-SemiBold.ttf"
        ))
        .expect("the nametag font is in the tree");
        let font = self
            .app
            .world_mut()
            .resource_mut::<Assets<Font>>()
            .add(Font::try_from_bytes(font).expect("the nametag font is a font"));
        self.app.insert_resource(EmojiFont(font));
    }

    fn give_avatar(&mut self, player: Entity, at: Vec3) {
        let world = self.app.world_mut();
        let attach_points = AttachPoints::new(&mut world.commands());
        world.flush();
        *world
            .get_mut::<GlobalTransform>(attach_points.position)
            .unwrap() = GlobalTransform::from_translation(at);
        world.entity_mut(player).insert(attach_points);
    }

    /// A peer with an avatar at `at`.
    fn peer_at(&mut self, at: Vec3) -> Entity {
        let peer = self.peer();
        self.give_avatar(peer, at);
        peer
    }

    fn move_avatar(&mut self, player: Entity, to: GlobalTransform) {
        let position = self
            .app
            .world()
            .get::<AttachPoints>(player)
            .unwrap()
            .position;
        *self
            .app
            .world_mut()
            .get_mut::<GlobalTransform>(position)
            .unwrap() = to;
    }

    fn particles(&mut self) -> Vec<Entity> {
        self.app
            .world_mut()
            .query_filtered::<Entity, With<ReactionParticle>>()
            .iter(self.app.world())
            .collect()
    }

    /// The one particle alive.
    fn particle(&mut self) -> Entity {
        let particles = self.particles();
        assert_eq!(particles.len(), 1, "one particle");
        particles[0]
    }

    /// Have every particle's mesh count as built, as its emoji's glyphs landing in the atlas
    /// does.
    fn glyphs_land(&mut self) {
        let (generation, _) = self.atlas();
        let mut particles = self.app.world_mut().query::<&mut ReactionParticle>();
        for mut particle in particles.iter_mut(self.app.world_mut()) {
            particle.built = Some(generation);
        }
    }

    /// A particle that is gone is not drawn.
    fn drawn(&self, particle: Entity) -> bool {
        self.app
            .world()
            .get::<Visibility>(particle)
            .is_some_and(|visibility| *visibility != Visibility::Hidden)
    }

    /// Run frames with time standing still until `particle` is drawn: its emoji's glyph is
    /// rastered on the task pool, which takes as long as the machine is busy.
    fn wait_until_drawn(&mut self, particle: Entity) {
        self.app
            .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::ZERO));
        let deadline = std::time::Instant::now() + Duration::from_secs(60);
        while !self.drawn(particle) {
            assert!(
                std::time::Instant::now() < deadline,
                "the glyph was never rastered"
            );
            std::thread::yield_now();
            self.app.update();
        }
    }

    /// Give a console command, and take its reply.
    fn console(&mut self, command: &str) -> String {
        let reply = Arc::new(Mutex::new(None));
        let output = reply.clone();
        self.app.world_mut().send_event(ConsoleCommandEntered {
            command_name: command.into(),
            args: vec![],
            responder: Some(Arc::new(move |result| {
                *output.lock_ignore_poison() = Some(result)
            })),
        });
        self.app.update();
        let reply = reply.lock_ignore_poison().take();
        reply.expect("the command replies").expect("and succeeds")
    }

    /// The colour page size `/reaction_state` reports.
    fn reported_atlas(&mut self) -> u64 {
        let state: serde_json::Value =
            serde_json::from_str(&self.console("/reaction_state")).unwrap();
        state["atlas"].as_u64().expect("a size in pixels")
    }

    /// The generation of the reactions' glyph atlas, if it has been made.
    fn atlas_made(&self) -> Option<u32> {
        let atlases = self.app.world().resource::<GlyphAtlases>();
        atlases.get(&ATLAS.key).map(|atlas| atlas.generation())
    }

    /// An emoji with a glyph the atlas had no room for.
    fn emoji_with_no_room(&self) -> Option<&'static str> {
        let world = self.app.world();
        let atlas = world.resource::<GlyphAtlases>().get(&ATLAS.key)?;
        world
            .resource::<ReactionGlyphs>()
            .emoji
            .iter()
            .find(|(_, glyphs)| atlas.has_dropped(glyphs.text.laid_out()))
            .map(|(emoji, _)| *emoji)
    }

    fn particle_of(&mut self, emoji: &str) -> Option<Entity> {
        self.app
            .world_mut()
            .query::<(Entity, &ReactionParticle)>()
            .iter(self.app.world())
            .find(|(_, particle)| particle.emoji == emoji)
            .map(|(entity, _)| entity)
    }

    /// The reactions' glyph atlas: its generation, and its colour page.
    fn atlas(&mut self) -> (u32, Option<AssetId<Image>>) {
        let world = self.app.world_mut();
        world.resource_scope(|world, mut atlases: Mut<GlyphAtlases>| {
            let atlas =
                atlases.get_or_create(ATLAS.key, &mut world.resource_mut::<Assets<Image>>());
            (
                atlas.generation(),
                atlas.page_image(Page::Color).map(|(image, _)| image.id()),
            )
        })
    }

    fn set_camera_distance(&mut self, distance: f32) {
        let mut cameras = self.app.world_mut().query::<&mut PrimaryCamera>();
        cameras.single_mut(self.app.world_mut()).unwrap().distance = distance;
    }

    /// A particle over the local player, its glyphs landed and a frame on.
    fn local_particle(&mut self) -> Entity {
        self.pick("👍");
        self.app.update();
        self.glyphs_land();
        self.app.update();
        self.particle()
    }
}

#[test]
fn a_reaction_rises_from_over_its_players_nametag_and_is_gone_within_a_second() {
    let mut harness = Harness::with_avatars();
    harness.pick("👍");
    harness.app.update();
    let particle = harness.particle();
    // made for the player, to go when they do
    let owner = harness.app.world().get::<DespawnWith>(particle).unwrap().0;
    assert_eq!(owner, harness.local);
    // not drawn before its glyphs are in the atlas
    assert!(!harness.drawn(particle));

    harness.glyphs_land();
    harness.app.update();
    assert!(harness.drawn(particle));
    let start = harness
        .app
        .world()
        .get::<Transform>(particle)
        .unwrap()
        .translation;
    assert!(
        (start.y - (NAMETAG_HEIGHT + ANCHOR_RISE)).abs() < 0.2,
        "{start}"
    );
    assert!(start.x.abs() < 0.2 && start.z.abs() < 0.2, "{start}");

    harness.run(0.3);
    let later = *harness.app.world().get::<Transform>(particle).unwrap();
    assert!(later.translation.y > start.y + 0.2);
    assert!(
        later.scale.x > 0.1 && later.scale.x < 0.25,
        "{}",
        later.scale
    );
    assert_eq!(harness.app.world().get::<MeshTag>(particle).unwrap().0, 255);

    harness.run(0.8);
    assert!(harness.particles().is_empty());
}

#[test]
fn an_emoji_is_rastered_into_the_atlas_and_its_particle_drawn_from_it() {
    let mut harness = Harness::with_avatars();
    harness.pick("🔥");
    harness.app.update();
    let particle = harness.particle();
    harness.wait_until_drawn(particle);

    let assets = harness.app.world().resource::<ReactionAssets>();
    let (page, size) = assets.bound[1].expect("the atlas has a colour page");
    assert!(size > 0);
    let material = harness
        .app
        .world()
        .resource::<Assets<ReactionMaterial>>()
        .get(&assets.material)
        .unwrap();
    assert_eq!(material.bitmap.as_ref().map(Handle::id), Some(page));
    let mesh = harness
        .app
        .world()
        .get::<Mesh3d>(particle)
        .unwrap()
        .0
        .clone();
    assert!(harness
        .app
        .world()
        .resource::<Assets<Mesh>>()
        .get(&mesh)
        .is_some());

    // the next of the same emoji draws from the same mesh, at once
    harness.pick("🔥");
    harness.app.update();
    let particles = harness.particles();
    assert_eq!(particles.len(), 2);
    for particle in particles {
        assert_eq!(harness.app.world().get::<Mesh3d>(particle).unwrap().0, mesh);
    }
}

#[test]
fn a_particle_alive_through_an_atlas_rebuild_is_drawn_from_the_new_page() {
    let mut harness = Harness::with_avatars();
    harness.pick("🔥");
    harness.app.update();
    let kept = harness.particle();
    harness.wait_until_drawn(kept);
    // a glyph no particle uses any more, for the rebuild to free: with none there is none
    harness.pick("👍");
    harness.app.update();
    let gone = harness
        .particles()
        .into_iter()
        .find(|particle| *particle != kept)
        .unwrap();
    harness.wait_until_drawn(gone);
    harness.app.world_mut().despawn(gone);

    let (generation, page) = harness.atlas();
    let mesh = harness.app.world().get::<Mesh3d>(kept).unwrap().0.clone();
    let is_mesh = |harness: &Harness, mesh: &Handle<Mesh>| {
        harness
            .app
            .world()
            .resource::<Assets<Mesh>>()
            .get(mesh)
            .is_some()
    };
    assert!(is_mesh(&harness, &mesh));

    harness
        .app
        .world_mut()
        .resource_scope(|world, mut atlases: Mut<GlyphAtlases>| {
            atlases
                .get_or_create(ATLAS.key, &mut world.resource_mut::<Assets<Image>>())
                .request_rebuild();
        });
    harness.app.update();

    // the atlas was rebuilt: a generation on, in a new texture
    let (rebuilt, new_page) = harness.atlas();
    assert_eq!(rebuilt, generation + 1);
    assert!(new_page.is_some() && new_page != page);
    // the particle's mesh was made again for it in the frame of the rebuild, so it is never
    // drawn from where its glyph was
    let particle = harness.app.world().get::<ReactionParticle>(kept).unwrap();
    assert_eq!(particle.built, Some(rebuilt));
    let remade = harness.app.world().get::<Mesh3d>(kept).unwrap().0.clone();
    assert_ne!(remade, mesh);
    assert!(is_mesh(&harness, &remade));
    assert!(harness.drawn(kept));
    // and the material draws from the new texture
    let assets = harness.app.world().resource::<ReactionAssets>();
    assert_eq!(assets.bound[1].map(|(page, _)| page), new_page);
    let material = harness
        .app
        .world()
        .resource::<Assets<ReactionMaterial>>()
        .get(&assets.material)
        .unwrap();
    assert_eq!(material.bitmap.as_ref().map(Handle::id), new_page);

    // the freed emoji is placed again when it is next picked
    harness.pick("👍");
    harness.app.update();
    let again = harness
        .particles()
        .into_iter()
        .find(|particle| *particle != kept)
        .unwrap();
    harness.wait_until_drawn(again);
    assert!(harness.drawn(kept));
}

#[test]
fn an_emoji_a_text_font_has_the_base_character_of_is_drawn_in_colour() {
    let mut harness = Harness::with_avatars();
    // each is a character a text font has, followed by the selector that asks for the emoji
    for emoji in ["©️", "®️", "™️", "‼️", "⁉️", "ℹ️"] {
        harness.pick(emoji);
        harness.app.update();
        let particle = *harness.particles().last().unwrap();
        harness.wait_until_drawn(particle);
    }
    assert_eq!(harness.particles().len(), 6);
    assert_eq!(harness.stats().dropped.no_glyphs, 0);
    let assets = harness.app.world().resource::<ReactionAssets>();
    assert!(assets.bound[1].is_some());
}

#[test]
fn an_emoji_with_no_glyph_in_colour_loses_its_particle_and_gets_no_other() {
    let mut harness = Harness::with_avatars();
    // in a text font the copyright sign is that font's, in one colour
    harness.lay_out_in_a_text_font();
    harness
        .app
        .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::ZERO));
    harness.pick("©️");
    harness.app.update();
    // its glyph is rastered, and is none of the colour page's
    let deadline = std::time::Instant::now() + Duration::from_secs(60);
    while !harness.particles().is_empty() {
        assert!(
            std::time::Instant::now() < deadline,
            "the particle was never given up"
        );
        std::thread::yield_now();
        harness.app.update();
    }
    assert_eq!(harness.stats().dropped.no_glyphs, 1);

    // the next takes no slot and asks nothing of the atlas
    harness.pick("©️");
    harness.app.update();
    assert!(harness.particles().is_empty());
    assert_eq!(harness.stats().dropped.no_glyphs, 2);
    assert_eq!(harness.streamed().len(), 2);
}

#[test]
fn an_emoji_the_full_atlas_has_no_room_for_waits_and_is_drawn_once_there_is() {
    let mut harness = Harness::with_avatars();
    harness
        .app
        .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::ZERO));
    // more emoji alive at once than the atlas' colour page holds at its largest, each player
    // with as many as one may have
    let mut tiles = 0..;
    for index in 0..MAX_PARTICLES / MAX_PARTICLES_PER_PLAYER {
        let peer = harness.peer_at(Vec3::new(index as f32 * 0.1, 0.0, -3.0));
        for emoji in tiles
            .by_ref()
            .take(MAX_PARTICLES_PER_PLAYER)
            .filter_map(atlas_index_to_emoji)
        {
            harness.app.world_mut().send_event(ReactionPlayed {
                player: peer,
                emoji,
            });
        }
    }
    harness.app.update();

    // each is rastered and placed until one finds no room, with every glyph placed in use
    let deadline = std::time::Instant::now() + Duration::from_secs(300);
    let emoji = loop {
        if let Some(emoji) = harness.emoji_with_no_room() {
            break emoji;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "the atlas never filled"
        );
        std::thread::yield_now();
        harness.app.update();
    };
    harness.app.update();
    // its particle waits undrawn, and the emoji is not taken for one with no colour glyph
    let waiting = harness.particle_of(emoji).expect("its particle is kept");
    assert!(!harness.drawn(waiting));
    let glyphs = harness.app.world().resource::<ReactionGlyphs>();
    assert!(!glyphs.emoji[emoji].colourless);

    // the others go, and the rebuild frees their glyphs and places its own
    for particle in harness.particles() {
        if particle != waiting {
            harness.app.world_mut().despawn(particle);
        }
    }
    let (generation, _) = harness.atlas();
    harness
        .app
        .world_mut()
        .resource_mut::<GlyphAtlases>()
        .get_mut(&ATLAS.key)
        .unwrap()
        .request_rebuild();
    harness.app.update();
    assert_eq!(harness.atlas().0, generation + 1);
    harness.wait_until_drawn(waiting);
    assert_eq!(harness.emoji_with_no_room(), None);
}

#[test]
fn the_console_reads_the_atlas_without_making_it_and_has_it_rebuilt() {
    let mut harness = Harness::with_avatars();
    // before any reaction there is no atlas, and neither command makes one
    assert_eq!(harness.reported_atlas(), 0);
    harness.console("/reaction_atlas_rebuild");
    harness.app.update();
    assert_eq!(harness.atlas_made(), None);

    harness.pick("🔥");
    harness.app.update();
    let kept = harness.particle();
    harness.wait_until_drawn(kept);
    // a glyph no particle uses any more, for the rebuild to free
    harness.pick("👍");
    harness.app.update();
    let gone = harness.particle_of("👍").unwrap();
    harness.wait_until_drawn(gone);
    harness.app.world_mut().despawn(gone);
    let (_, size) = harness.app.world().resource::<ReactionAssets>().bound[1].unwrap();
    assert_eq!(harness.reported_atlas(), u64::from(size));

    let generation = harness.atlas_made().unwrap();
    harness.console("/reaction_atlas_rebuild");
    // asked for in the frame of the command, served by the next upkeep
    harness.app.update();
    assert_eq!(harness.atlas_made(), Some(generation + 1));
    harness.wait_until_drawn(kept);
}

#[test]
fn only_so_many_particles_live_over_one_player() {
    let mut harness = Harness::with_avatars();
    for _ in 0..MAX_PARTICLES_PER_PLAYER + 3 {
        harness.pick("👍");
    }
    harness.app.update();
    assert_eq!(harness.particles().len(), MAX_PARTICLES_PER_PLAYER);
    assert_eq!(harness.stats().dropped.particle_cap, 3);
    // the ones with no particle played all the same
    assert_eq!(harness.streamed().len(), MAX_PARTICLES_PER_PLAYER + 3);

    // another player's are their own
    let peer = harness.peer_at(Vec3::new(3.0, 0.0, 0.0));
    harness.hear(peer, "🔥", 1.0, 1);
    harness.app.update();
    assert_eq!(harness.particles().len(), MAX_PARTICLES_PER_PLAYER + 1);
}

#[test]
fn only_so_many_particles_live_in_all() {
    let mut harness = Harness::with_avatars();
    // more players than the cap has room for, each with as many as one may have
    let players = MAX_PARTICLES / MAX_PARTICLES_PER_PLAYER + 2;
    for index in 0..players {
        let peer = harness.peer_at(Vec3::new(index as f32 * 0.1, 0.0, -3.0));
        for _ in 0..MAX_PARTICLES_PER_PLAYER {
            harness.app.world_mut().send_event(ReactionPlayed {
                player: peer,
                emoji: "🔥",
            });
        }
    }
    harness.app.update();
    assert_eq!(harness.particles().len(), MAX_PARTICLES);
    assert_eq!(
        harness.stats().dropped.particle_cap as usize,
        players * MAX_PARTICLES_PER_PLAYER - MAX_PARTICLES
    );
}

#[test]
fn an_emoji_that_cannot_be_laid_out_yet_is_counted_and_tried_again() {
    let mut harness = Harness::with_avatars();
    // a font whose asset is not there has nothing to lay an emoji out in
    let font = harness
        .app
        .world()
        .resource::<Assets<Font>>()
        .reserve_handle();
    harness.app.insert_resource(EmojiFont(font.clone()));
    harness.pick("👍");
    harness.app.update();
    assert!(harness.particles().is_empty());
    assert_eq!(harness.stats().dropped.no_glyphs, 1);
    assert_eq!(harness.streamed().len(), 1);

    harness
        .app
        .world_mut()
        .resource_mut::<Assets<Font>>()
        .insert(
            &font,
            Font {
                data: EMOJI_BYTES.clone(),
            },
        );
    harness.pick("👍");
    harness.app.update();
    assert_eq!(harness.particles().len(), 1);
    assert_eq!(harness.stats().dropped.no_glyphs, 1);
}

#[test]
fn with_chat_bubbles_off_reactions_play_with_no_particle() {
    let mut harness = Harness::with_avatars();
    let mut wire = harness.transport(TransportType::Livekit);
    harness
        .app
        .world_mut()
        .resource_mut::<AppConfig>()
        .chat_bubble_visibility = ChatBubbleVisibility::None;
    let peer = harness.peer_at(Vec3::new(3.0, 0.0, 0.0));
    harness.pick("👍");
    harness.hear(peer, "🔥", 1.0, 1);
    harness.run(0.5);
    assert!(harness.particles().is_empty());
    assert_eq!(harness.streamed().len(), 2);
    assert_eq!(sent(&mut wire).len(), 1);

    // one already alive when the setting goes off is hidden at once
    harness
        .app
        .world_mut()
        .resource_mut::<AppConfig>()
        .chat_bubble_visibility = ChatBubbleVisibility::All;
    let particle = harness.local_particle();
    harness
        .app
        .world_mut()
        .resource_mut::<AppConfig>()
        .chat_bubble_visibility = ChatBubbleVisibility::None;
    harness.app.update();
    assert!(!harness.drawn(particle));
}

#[test]
fn a_player_with_no_avatar_gets_no_particle() {
    let mut harness = Harness::with_avatars();
    let peer = harness.peer();
    harness.hear(peer, "🔥", 1.0, 2);
    harness.run(0.5);
    assert!(harness.particles().is_empty());
    assert_eq!(harness.streamed().len(), 2);
}

#[test]
fn a_particle_shows_where_its_players_nametag_would() {
    let mut harness = Harness::with_avatars();
    let peer = harness.peer_at(Vec3::new(3.0, 0.0, 0.0));
    harness.hear(peer, "🔥", 1.0, 1);
    harness.app.update();
    harness.glyphs_land();
    harness.app.update();
    let particle = harness.particle();
    assert!(harness.drawn(particle));

    let shows_again = |harness: &mut Harness| {
        harness.app.update();
        harness.drawn(particle)
    };

    // an avatar past the limit of avatars drawn
    harness
        .app
        .world_mut()
        .entity_mut(peer)
        .insert(BeyondMaxAvatars);
    assert!(!shows_again(&mut harness));
    harness
        .app
        .world_mut()
        .entity_mut(peer)
        .remove::<BeyondMaxAvatars>();
    assert!(shows_again(&mut harness));

    // a scene's area hiding avatars
    harness
        .app
        .world_mut()
        .entity_mut(peer)
        .insert(PlayerModifiers {
            hide: true,
            ..Default::default()
        });
    assert!(!shows_again(&mut harness));
    harness
        .app
        .world_mut()
        .entity_mut(peer)
        .remove::<PlayerModifiers>();
    assert!(shows_again(&mut harness));

    // a scene hiding the avatar itself
    harness
        .app
        .world_mut()
        .entity_mut(peer)
        .insert(InheritedVisibility::HIDDEN);
    assert!(!shows_again(&mut harness));
    harness
        .app
        .world_mut()
        .entity_mut(peer)
        .insert(InheritedVisibility::VISIBLE);
    assert!(shows_again(&mut harness));

    // nametags switched off, as photo mode does
    harness.app.world_mut().resource_mut::<NametagsVisible>().0 = false;
    assert!(!shows_again(&mut harness));
    harness.app.world_mut().resource_mut::<NametagsVisible>().0 = true;
    assert!(shows_again(&mut harness));

    // an avatar scaled to nothing
    harness.move_avatar(
        peer,
        GlobalTransform::from(Transform::from_xyz(3.0, 0.0, 0.0).with_scale(Vec3::ZERO)),
    );
    assert!(!shows_again(&mut harness));
    harness.move_avatar(peer, GlobalTransform::from_xyz(3.0, 0.0, 0.0));
    assert!(shows_again(&mut harness));

    // a player blocked since
    harness
        .app
        .world_mut()
        .entity_mut(peer)
        .insert(BlockedOrBanned);
    assert!(!shows_again(&mut harness));
}

#[test]
fn far_from_the_camera_only_the_local_users_show() {
    let mut harness = Harness::with_avatars();
    let peer = harness.peer_at(Vec3::new(0.0, 0.0, -(DRAW_DISTANCE - 5.0)));
    harness.hear(peer, "🔥", 1.0, 1);
    harness.app.update();
    harness.glyphs_land();
    harness.app.update();
    let theirs = harness.particle();
    assert!(harness.drawn(theirs));
    // farther, it is hidden
    harness.move_avatar(
        peer,
        GlobalTransform::from_xyz(0.0, 0.0, -(DRAW_DISTANCE + 5.0)),
    );
    harness.app.update();
    assert!(!harness.drawn(theirs));

    // the local user's show however far the camera has gone from them
    let local = harness.local;
    harness.move_avatar(
        local,
        GlobalTransform::from_xyz(0.0, 0.0, DRAW_DISTANCE * 3.0),
    );
    harness.pick("👍");
    harness.app.update();
    harness.glyphs_land();
    harness.app.update();
    let mine = harness
        .particles()
        .into_iter()
        .find(|particle| *particle != theirs)
        .unwrap();
    assert!(harness.drawn(mine));
}

#[test]
fn a_reaction_of_a_player_not_shown_plays_with_no_particle_and_no_glyphs() {
    let mut harness = Harness::with_avatars();
    let far = harness.peer_at(Vec3::new(0.0, 0.0, -(DRAW_DISTANCE + 5.0)));
    let hidden = harness.peer_at(Vec3::new(3.0, 0.0, 0.0));
    harness
        .app
        .world_mut()
        .entity_mut(hidden)
        .insert(PlayerModifiers {
            hide: true,
            ..Default::default()
        });
    harness.hear(far, "🔥", 1.0, 1);
    harness.hear(hidden, "👍", 1.0, 1);
    harness.run(0.5);
    assert!(harness.particles().is_empty());
    assert_eq!(harness.stats().dropped.not_shown, 2);
    // nothing was laid out for them, so nothing of theirs is asked of the atlas
    assert!(harness
        .app
        .world()
        .resource::<ReactionGlyphs>()
        .emoji
        .is_empty());
    // they played all the same
    assert_eq!(harness.streamed().len(), 2);

    // one who comes near gets a particle for the next
    harness.move_avatar(far, GlobalTransform::from_xyz(0.0, 0.0, -5.0));
    harness.hear(far, "🔥", 2.0, 1);
    harness.run(0.1);
    assert_eq!(harness.particles().len(), 1);
}

#[test]
fn in_first_person_the_local_users_own_are_not_drawn() {
    let mut harness = Harness::with_avatars();
    let peer = harness.peer_at(Vec3::new(3.0, 0.0, 0.0));
    let mine = harness.local_particle();
    assert!(harness.drawn(mine));

    // one alive as the camera goes into the player's head is hidden
    harness.set_camera_distance(0.0);
    harness.app.update();
    assert!(!harness.drawn(mine));
    // and the next gets no particle, while it is passed to the HUD as any is
    harness.streamed();
    harness.pick("👍");
    harness.app.update();
    assert_eq!(harness.particles().len(), 1);
    assert_eq!(harness.stats().dropped.not_shown, 1);
    assert_eq!(harness.streamed().len(), 1);

    // other players' are drawn as ever
    harness.hear(peer, "🔥", 1.0, 1);
    harness.app.update();
    harness.glyphs_land();
    harness.app.update();
    let theirs = harness
        .particles()
        .into_iter()
        .find(|particle| *particle != mine)
        .unwrap();
    assert!(harness.drawn(theirs));

    harness.set_camera_distance(1.0);
    harness.app.update();
    assert!(harness.drawn(mine));
}

#[test]
fn a_particle_keeps_over_a_player_who_moves() {
    let mut harness = Harness::with_avatars();
    let particle = harness.local_particle();
    let local = harness.local;
    harness.move_avatar(local, GlobalTransform::from_xyz(4.0, 0.0, 0.0));
    harness.run(0.4);
    let position = harness
        .app
        .world()
        .get::<Transform>(particle)
        .unwrap()
        .translation;
    assert!(position.x > 2.5 && position.x < 4.2, "{position}");
}

#[test]
fn a_player_who_leaves_takes_their_particles() {
    let mut harness = Harness::with_avatars();
    harness.app.add_plugins(common::util::UtilsPlugin);
    let peer = harness.peer_at(Vec3::new(3.0, 0.0, 0.0));
    harness.hear(peer, "🔥", 1.0, 1);
    harness.app.update();
    harness.glyphs_land();
    harness.app.update();
    let particle = harness.particle();
    assert!(harness.drawn(particle));

    harness.app.world_mut().despawn(peer);
    harness.app.update();
    assert!(harness.particles().is_empty());
}

/// The reaction shader compiles on this machine's adapter, in a renderer with no window: a
/// particle in front of a camera with the prepasses and depth of field (whose focus target a
/// transparent material must write). Needs a GPU, so `#[ignore]`d and skipped by CI:
///
/// ```text
/// cargo test -p avatar reaction_shader -- --ignored --nocapture
/// ```
#[test]
#[ignore = "needs a GPU; run with --ignored"]
fn reaction_shader_compiles() {
    use bevy::{
        core_pipeline::{
            dof::DepthOfField,
            prepass::{DepthPrepass, NormalPrepass},
        },
        render::camera::RenderTarget,
    };
    use pipeline_test::{compile_pipelines, fragment_pipelines, headless_app, render_target};
    use scene_runner::update_world::text_shape::TextQuad;

    let mut app = headless_app();
    app.add_plugins(MaterialPlugin::<ReactionMaterial> {
        prepass_enabled: false,
        shadows_enabled: false,
        ..default()
    });
    app.finish();
    app.cleanup();

    let target = render_target(&mut app);
    let harness = app.world_mut();
    let material = harness
        .resource_mut::<Assets<ReactionMaterial>>()
        .add(ReactionMaterial { bitmap: None });
    let mesh = emoji_mesh(&[TextQuad {
        min: Vec2::ZERO,
        max: Vec2::ONE,
        z: 0.0,
        uv_min: Vec2::ZERO,
        uv_max: Vec2::ONE,
        color: LinearRgba::WHITE,
    }])
    .unwrap();
    let mesh = harness.resource_mut::<Assets<Mesh>>().add(mesh);

    harness.spawn((
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
    harness.spawn((
        Mesh3d(mesh),
        MeshMaterial3d(material),
        MeshTag(opacity_tag(1.0)),
        Aabb::from_min_max(Vec3::splat(-1.0), Vec3::splat(1.0)),
        NotShadowCaster,
        Transform::default(),
    ));

    compile_pipelines(&mut app);
    assert!(
        fragment_pipelines(&app, "embedded://shaders/reaction.wgsl") >= 1,
        "the reaction was not drawn"
    );
}
