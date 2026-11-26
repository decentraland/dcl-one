//! Situational reactions: an emoji a player picks to show how they feel about what is going
//! on, played once for each time it was picked.
//!
//! The local user's pick plays at once and is sent to the players around them, gathered with
//! their other picks into one message per emoji. A peer's message is held to what its sender
//! may send, then its reactions play one at a time. Each one played is passed to the HUD on a
//! stream.
//!
//! Over an avatar a reaction is a particle: an entity of its own in the world, drawn by one
//! shared material (reaction.wgsl) with the emoji's mesh, whose glyphs are in a glyph atlas of
//! the reactions' own. It rises from the player's nametag anchor and goes when its life is
//! over, or when its player does.

mod diagnostics;
mod material;
mod particle;
mod traffic;

#[cfg(test)]
mod tests;

use alloy_core::primitives::Address;
use ban_block::{BanBlock, PlatformBanStatus};
use bevy::{
    ecs::system::SystemParam,
    pbr::NotShadowCaster,
    platform::collections::HashMap,
    prelude::*,
    render::{mesh::MeshTag, primitives::Aabb},
    text::{CosmicFontSystem, LineBreak, TextPipeline},
};
use common::{
    rpc::RpcStreamSender,
    sets::{PostUpdateSets, SceneSets},
    structs::{
        AppConfig, AttachPoints, ChatBubbleVisibility, PlayerModifiers, PlayerReacted,
        PrimaryCamera, PrimaryUser,
    },
    util::DespawnWith,
};
use comms::{
    broadcast_to,
    chat_reaction::{atlas_index_to_emoji, picked_emoji_tile},
    global_crdt::{process_transport_updates, ForeignPlayer, ReactionEvent},
    BlockedOrBanned, BroadcastTarget, Transport,
};
use console::{AddDebugSection, DoAddConsoleCommand};
use dcl_component::proto_components::kernel::comms::rfc4;
use scene_runner::update_world::text_shape::{
    glyph_quads, layout_text, AtlasGlyphUse, AtlasKey, GlyphAtlas, GlyphAtlasMaintenance,
    GlyphAtlases, LaidOutText, Page, TextSpanData,
};
use system_bridge::{ReactionData, SystemApi};
use ui_core::{FontName, WeightName, FONTS};
use wallet::Wallet;

use self::{
    material::{emoji_mesh, opacity_tag, ReactionMaterial},
    particle::Particle,
    traffic::{ReactionGuard, ReactionOutbox, ReactionQueue, Refused},
};
use crate::{
    dynamic_nametag::{avatar_units, dynamic_nametag_position, NametagsVisible},
    BeyondMaxAvatars,
};

/// The glyph atlas the emoji are drawn from: what peers pick fills it, and its rebuild moves
/// no other feature's glyphs.
const ATLAS: AtlasKey = AtlasKey::Engine("reaction");
/// The size an emoji is laid out and rastered at, whatever size it is drawn.
const RASTER_FONT_SIZE: f32 = 64.0;
/// Glyphs a frame may send to be rastered.
const RASTERS_PER_FRAME: usize = 2;
/// The most particles alive at once, and over one player.
const MAX_PARTICLES: usize = 1023;
const MAX_PARTICLES_PER_PLAYER: usize = 15;
/// How far over the nametag anchor a reaction starts, in the avatar's units.
const ANCHOR_RISE: f32 = 0.15;
/// A player's reactions are drawn while their anchor is this near the camera, in metres.
const DRAW_DISTANCE: f32 = 25.0;

pub struct ReactionPlugin;

impl Plugin for ReactionPlugin {
    fn build(&self, app: &mut App) {
        // blended and never a shadow caster: the prepass and the shadow passes draw none of it
        app.add_plugins(MaterialPlugin::<ReactionMaterial> {
            prepass_enabled: false,
            shadows_enabled: false,
            ..default()
        })
        .add_event::<ReactionPlayed>()
        // read by analytics, which registers it too
        .add_event::<PlayerReacted>()
        .init_resource::<ReactionStats>()
        .init_resource::<ReactionAssets>()
        .init_resource::<ReactionGlyphs>()
        .register_required_components::<ForeignPlayer, ReactionGuard>()
        .register_required_components::<PrimaryUser, ReactionOutbox>()
        .add_systems(
            Update,
            (
                accept_local_reactions,
                // a peer's reactions are taken only where the block and ban lists are
                // kept: an app without them (the offline avatar renderer) hears no one
                receive_reactions
                    .run_if(on_event::<ReactionEvent>.and(resource_exists::<PlatformBanStatus>)),
                // asks who is blocked, which only an app that keeps the lists can answer: it runs
                // while a sender has a queue
                play_queued_reactions.run_if(any_with_component::<ReactionQueue>),
                flush_reactions,
                spawn_reaction_particles.run_if(on_event::<ReactionPlayed>),
                pipe_reactions_to_scene,
                build_reaction_glyphs,
            )
                .chain()
                .in_set(SceneSets::PostLoop)
                // a sender first heard from this frame has its entity by now
                .after(process_transport_updates)
                // a rebuilt atlas has moved every glyph: build from where they are now
                .after(GlyphAtlasMaintenance),
        )
        .add_systems(
            PostUpdate,
            simulate_reaction_particles
                .in_set(PostUpdateSets::Nametag)
                .after(dynamic_nametag_position),
        )
        .add_console_command::<diagnostics::ReactionStateCommand, _>(diagnostics::reaction_state)
        .add_console_command::<diagnostics::ReactionSendCommand, _>(diagnostics::reaction_send)
        .add_console_command::<diagnostics::ReactionSimulateCommand, _>(
            diagnostics::reaction_simulate,
        )
        .add_debug_section(diagnostics::SECTION);
    }
}

/// One reaction as it plays: `emoji` for `player`, the local user or a peer.
#[derive(Event, Debug, Clone, Copy, PartialEq, Eq)]
struct ReactionPlayed {
    player: Entity,
    emoji: &'static str,
}

/// The address a player reacts under. A peer is who comms heard them as; the one other player
/// who reacts is the local user.
fn address_of(peer: Option<&ForeignPlayer>, wallet: &Wallet) -> Address {
    peer.map(|peer| peer.address)
        .or_else(|| wallet.address())
        .unwrap_or_default()
}

/// What has happened to reactions this session, for the debug panel.
#[derive(Resource, Default)]
struct ReactionStats {
    /// The local user's picks that played: no peer's reaction moves this.
    picked: u32,
    /// The local user's picks that were no emoji the wire can carry, or had no one to send them.
    refused_picks: u32,
    /// Messages put on a room's transport, one per emoji per send.
    sent: u32,
    /// Peers' messages, before any is dropped.
    received: u32,
    played: u32,
    /// Reactions passed to the HUD, and how many streams they go to.
    streamed: u32,
    subscribers: usize,
    dropped: Dropped,
}

/// Why peers' reactions did not play.
#[derive(Default)]
struct Dropped {
    /// Messages from a sender the local user must not see, and reactions such a sender had
    /// waiting.
    blocked: u32,
    duplicate: u32,
    unknown_emoji: u32,
    rate_limited: u32,
    /// Reactions that gave way to newer ones in a full queue.
    queue_full: u32,
    /// Reactions that played with no particle: as many as may be were alive already.
    particle_cap: u32,
}

/// On a reaction's particle. Its player is the one it is [`DespawnWith`].
#[derive(Component)]
struct ReactionParticle {
    emoji: &'static str,
    motion: Particle,
    /// The generation of the glyph atlas its mesh samples; it is drawn while that is the
    /// atlas' own.
    built: Option<u32>,
}

#[derive(Resource)]
struct ReactionAssets {
    material: Handle<ReactionMaterial>,
    /// The atlas page the material binds, and its size.
    bound: Option<(AssetId<Image>, u32)>,
    /// The atlas' generation as of the last build.
    generation: u32,
}

impl FromWorld for ReactionAssets {
    fn from_world(world: &mut World) -> Self {
        Self {
            material: world
                .resource_mut::<Assets<ReactionMaterial>>()
                .add(ReactionMaterial { bitmap: None }),
            bound: None,
            generation: 0,
        }
    }
}

/// What drawing each emoji played so far takes.
#[derive(Resource, Default)]
struct ReactionGlyphs {
    /// The font the emoji are laid out in: any does, as each is drawn from the colour font
    /// every text falls back to. The built-in fonts are made at startup, after this is.
    font: Option<Handle<Font>>,
    emoji: HashMap<&'static str, EmojiGlyphs>,
}

struct EmojiGlyphs {
    /// `None` for an emoji that can't be laid out.
    text: Option<LaidOutText>,
    /// Its mesh, and the generation of the atlas the mesh samples.
    mesh: Option<(Handle<Mesh>, u32)>,
}

/// Lays emoji out and makes their meshes from the reactions' atlas.
#[derive(SystemParam)]
struct EmojiShaper<'w> {
    pipeline: ResMut<'w, TextPipeline>,
    fonts: Res<'w, Assets<Font>>,
    font_system: ResMut<'w, CosmicFontSystem>,
    atlases: ResMut<'w, GlyphAtlases>,
    images: ResMut<'w, Assets<Image>>,
    meshes: ResMut<'w, Assets<Mesh>>,
    glyphs: ResMut<'w, ReactionGlyphs>,
}

impl EmojiShaper<'_> {
    /// What keeps the emoji's glyphs in the atlas while a particle shows it. `None` for an
    /// emoji that can't be laid out, or before there is a font to lay it out in.
    fn glyph_use(&mut self, emoji: &'static str) -> Option<AtlasGlyphUse> {
        let Self {
            pipeline,
            fonts,
            font_system,
            glyphs,
            ..
        } = self;
        let ReactionGlyphs { font, emoji: laid } = &mut **glyphs;
        if font.is_none() {
            *font = FONTS
                .get()
                .and_then(|fonts| fonts.get(&(FontName::Sans, WeightName::Regular)))
                .cloned();
        }
        // nothing is laid out before the built-in fonts are made
        let font = font.clone()?;
        let glyphs = laid.entry(emoji).or_insert_with(|| {
            let font = TextFont {
                font,
                font_size: RASTER_FONT_SIZE,
                ..default()
            };
            let spans: [TextSpanData; 1] =
                [(TextSpan::new(emoji), font, TextColor(Color::WHITE), None)];
            let text = layout_text(
                Entity::PLACEHOLDER,
                &spans,
                JustifyText::Left,
                LineBreak::NoWrap,
                None,
                None,
                RASTER_FONT_SIZE,
                false,
                pipeline,
                fonts,
                font_system,
            )
            .inspect_err(|err| warn!("can't lay out the reaction {emoji:?}: {err}"))
            .ok();
            EmojiGlyphs { text, mesh: None }
        });
        glyphs
            .text
            .as_ref()
            .map(|text| AtlasGlyphUse::new(ATLAS, [text]))
    }

    fn atlas(&mut self) -> &GlyphAtlas {
        let images = &mut *self.images;
        self.atlases.get_or_create(ATLAS, images)
    }

    /// The emoji's mesh for the atlas as it is now. `None` while its glyphs are being
    /// rastered (as `budget` allows them to be sent), and for an emoji with nothing to draw.
    fn mesh(&mut self, emoji: &'static str, budget: &mut usize) -> Option<Handle<Mesh>> {
        let images = &mut *self.images;
        let atlas = self.atlases.get_or_create(ATLAS, images);
        let generation = atlas.generation();
        let glyphs = self.glyphs.emoji.get_mut(emoji)?;
        if let Some((mesh, built)) = &glyphs.mesh {
            if *built == generation {
                return Some(mesh.clone());
            }
        }
        let (_, colour) = glyph_quads(
            glyphs.text.as_ref()?,
            atlas,
            images,
            &mut self.font_system,
            budget,
            false,
        )
        .ok()?;
        let mesh = self.meshes.add(emoji_mesh(&colour)?);
        glyphs.mesh = Some((mesh.clone(), generation));
        Some(mesh)
    }
}

/// The local user's picks: each plays at once and waits in the outbox to be sent.
fn accept_local_reactions(
    mut requests: EventReader<SystemApi>,
    wallet: Res<Wallet>,
    mut local: Query<(Entity, &mut ReactionOutbox), With<PrimaryUser>>,
    mut played: EventWriter<ReactionPlayed>,
    mut reacted: EventWriter<PlayerReacted>,
    mut stats: ResMut<ReactionStats>,
) {
    for request in requests.read() {
        let SystemApi::SendReaction(picked) = request else {
            continue;
        };
        // what plays is the emoji the wire carries for the pick, so the local user sees what
        // the others will
        let reaction =
            picked_emoji_tile(picked).and_then(|tile| Some((tile, atlas_index_to_emoji(tile)?)));
        // no one signed in reacts as no one
        let (Some((tile, emoji)), Some(_), Ok((player, mut outbox))) =
            (reaction, wallet.address(), local.single_mut())
        else {
            debug!("no reaction for {picked:?}");
            stats.refused_picks += 1;
            continue;
        };
        outbox.push(tile);
        played.write(ReactionPlayed { player, emoji });
        stats.picked += 1;
        stats.played += 1;
        reacted.write(PlayerReacted { emoji_index: tile });
    }
}

/// Peers' messages: what their sender may send is queued on the sender, to play one at a time.
/// Nothing is taken from a sender the local user must not see.
fn receive_reactions(
    mut commands: Commands,
    mut events: EventReader<ReactionEvent>,
    ban_block: BanBlock,
    wallet: Res<Wallet>,
    mut senders: Query<(
        &mut ReactionGuard,
        Option<&mut ReactionQueue>,
        Option<&ForeignPlayer>,
        Has<BlockedOrBanned>,
    )>,
    time: Res<Time<Real>>,
    mut stats: ResMut<ReactionStats>,
) {
    let now = time.elapsed_secs_f64();
    // the queues of senders who had none: a queue inserted here is not there for the sender's
    // next message of the frame
    let mut started = HashMap::<Entity, ReactionQueue>::default();
    for event in events.read() {
        stats.received += 1;
        // a sender who has already gone has no avatar to react
        let Ok((mut guard, queue, peer, blocked)) = senders.get_mut(event.sender) else {
            continue;
        };
        let from = address_of(peer, &wallet);
        // the marker follows a block or ban by a frame or two, and a player who has just
        // appeared has none yet: their first message is asked about by address
        if blocked
            || ban_block.is_blocked_either_way(from)
            || ban_block.is_addr_banned_in_scene(from)
        {
            stats.dropped.blocked += 1;
            continue;
        }
        let (emoji, count) = match guard.admit(&event.reaction, now) {
            Ok(admitted) => admitted,
            Err(refused) => {
                debug!("reaction from {from:#x} refused: {refused:?}");
                match refused {
                    Refused::Duplicate => stats.dropped.duplicate += 1,
                    Refused::UnknownEmoji => stats.dropped.unknown_emoji += 1,
                    Refused::RateLimited => stats.dropped.rate_limited += 1,
                }
                continue;
            }
        };
        stats.dropped.queue_full += match queue {
            Some(mut queue) => queue.push(emoji, count),
            None => started.entry(event.sender).or_default().push(emoji, count),
        };
    }
    for (sender, queue) in started {
        commands.entity(sender).try_insert(queue);
    }
}

/// Each sender's queue plays what is due. It goes once nothing waits and the gap after the last
/// has run out: gone sooner, the sender's next message would play at once.
fn play_queued_reactions(
    mut commands: Commands,
    time: Res<Time>,
    wallet: Res<Wallet>,
    ban_block: BanBlock,
    mut queues: Query<(
        Entity,
        &mut ReactionQueue,
        Option<&ForeignPlayer>,
        Has<BlockedOrBanned>,
    )>,
    mut played: EventWriter<ReactionPlayed>,
    mut stats: ResMut<ReactionStats>,
) {
    for (player, mut queue, peer, blocked) in &mut queues {
        let from = address_of(peer, &wallet);
        // blocked or banned since they were queued: asked by address as at intake, as the
        // marker follows by a frame or two
        if blocked
            || ban_block.is_blocked_either_way(from)
            || ban_block.is_addr_banned_in_scene(from)
        {
            stats.dropped.blocked += queue.len() as u32;
            commands.entity(player).try_remove::<ReactionQueue>();
            continue;
        }
        for emoji in queue.tick(time.delta_secs()) {
            played.write(ReactionPlayed { player, emoji });
            stats.played += 1;
        }
        if queue.done() {
            commands.entity(player).try_remove::<ReactionQueue>();
        }
    }
}

/// The local user's picks go where nearby chat goes, once they have waited.
fn flush_reactions(
    time: Res<Time>,
    uptime: Res<Time<Real>>,
    mut outboxes: Query<&mut ReactionOutbox, With<PrimaryUser>>,
    transports: Query<&Transport>,
    mut stats: ResMut<ReactionStats>,
) {
    for mut outbox in &mut outboxes {
        if outbox.waiting() == 0 {
            continue;
        }
        for outgoing in outbox.tick(time.delta_secs(), uptime.elapsed_secs()) {
            // a message no room took went nowhere, and is not counted as sent
            let carried = broadcast_to(
                transports.iter(),
                BroadcastTarget::WEBSOCKET | BroadcastTarget::LIVEKIT,
                false,
                &rfc4::Packet {
                    message: Some(rfc4::packet::Message::Reaction(outgoing.into())),
                    protocol_version: 100,
                },
            );
            stats.sent += u32::from(carried > 0);
        }
    }
}

/// Each reaction played gets a particle, unless the user has chat bubbles, and reactions with
/// them, off, or as many particles as may be are alive already.
fn spawn_reaction_particles(
    mut commands: Commands,
    mut played: EventReader<ReactionPlayed>,
    config: Res<AppConfig>,
    assets: Res<ReactionAssets>,
    players: Query<(), With<AttachPoints>>,
    particles: Query<&DespawnWith, With<ReactionParticle>>,
    mut shaper: EmojiShaper,
    mut stats: ResMut<ReactionStats>,
) {
    if config.chat_bubble_visibility == ChatBubbleVisibility::None {
        played.clear();
        return;
    }
    let mut alive = particles.iter().count();
    let mut alive_over = HashMap::<Entity, usize>::default();
    let mut rng = rand::thread_rng();
    for reaction in played.read() {
        // a player with no avatar has nowhere for it to rise from
        if !players.contains(reaction.player) {
            continue;
        }
        let over_player = alive_over.entry(reaction.player).or_insert_with(|| {
            particles
                .iter()
                .filter(|owner| owner.0 == reaction.player)
                .count()
        });
        if alive >= MAX_PARTICLES || *over_player >= MAX_PARTICLES_PER_PLAYER {
            stats.dropped.particle_cap += 1;
            continue;
        }
        let Some(glyph_use) = shaper.glyph_use(reaction.emoji) else {
            continue;
        };
        commands.spawn((
            ReactionParticle {
                emoji: reaction.emoji,
                motion: Particle::new(&mut rng),
                built: None,
            },
            DespawnWith(reaction.player),
            glyph_use,
            Mesh3d::default(),
            MeshMaterial3d(assets.material.clone()),
            MeshTag(0),
            Transform::default(),
            Visibility::Hidden,
            // a cube: the quad faces the camera wherever that is
            Aabb::from_min_max(
                Vec3::splat(-std::f32::consts::FRAC_1_SQRT_2),
                Vec3::splat(std::f32::consts::FRAC_1_SQRT_2),
            ),
            NotShadowCaster,
        ));
        alive += 1;
        *over_player += 1;
    }
}

/// Every reaction played, the local user's included, goes to the HUD on each stream it opened.
fn pipe_reactions_to_scene(
    mut played: EventReader<ReactionPlayed>,
    mut requests: EventReader<SystemApi>,
    mut senders: Local<Vec<RpcStreamSender<ReactionData>>>,
    players: Query<Option<&ForeignPlayer>>,
    wallet: Res<Wallet>,
    mut stats: ResMut<ReactionStats>,
) {
    senders.extend(requests.read().filter_map(|ev| {
        if let SystemApi::GetReactionStream(sender) = ev {
            Some(sender.clone())
        } else {
            None
        }
    }));
    senders.retain(|s| !s.is_closed());
    stats.subscribers = senders.len();

    if senders.is_empty() {
        played.clear();
        return;
    }
    for reaction in played.read() {
        // a player who went this frame is not streamed
        let Ok(peer) = players.get(reaction.player) else {
            continue;
        };
        let data = ReactionData {
            from: format!("{:#x}", address_of(peer, &wallet)),
            emoji: reaction.emoji.to_owned(),
        };
        for sender in senders.iter() {
            let _ = sender.send(data.clone());
        }
        stats.streamed += 1;
    }
}

/// Give each particle the mesh of its emoji for the glyph atlas as it is now, and the material
/// the atlas' page. A particle whose glyphs are still being rastered waits, undrawn.
fn build_reaction_glyphs(
    mut shaper: EmojiShaper,
    mut assets: ResMut<ReactionAssets>,
    mut materials: ResMut<Assets<ReactionMaterial>>,
    mut particles: Query<(&mut ReactionParticle, &mut Mesh3d)>,
) {
    if particles.is_empty() {
        return;
    }
    let atlas = shaper.atlas();
    let generation = atlas.generation();
    let page = atlas
        .page_image(Page::Color)
        .map(|(image, size)| (image.clone(), size));
    let bound = page.as_ref().map(|(image, size)| (image.id(), *size));
    if assets.bound != bound {
        assets.bound = bound;
        if let Some(material) = materials.get_mut(&assets.material) {
            material.bitmap = page.map(|(image, _)| image);
        }
    }
    if assets.generation != generation {
        assets.generation = generation;
    }

    let mut budget = RASTERS_PER_FRAME;
    for (mut particle, mut mesh) in &mut particles {
        if particle.built == Some(generation) {
            continue;
        }
        if let Some(built) = shaper.mesh(particle.emoji, &mut budget) {
            mesh.0 = built;
            particle.built = Some(generation);
        }
    }
}

/// Move each particle on from its player's anchor as the avatar stands this frame, and show
/// it where its player's nametag would show: not over an avatar that is hidden, blocked or one
/// too many, nor with the nametags off, nor far from the camera unless it is the local user's.
fn simulate_reaction_particles(
    mut commands: Commands,
    time: Res<Time>,
    nametags: Res<NametagsVisible>,
    assets: Res<ReactionAssets>,
    camera: Query<&GlobalTransform, With<PrimaryCamera>>,
    players: Query<(
        &AttachPoints,
        Has<PrimaryUser>,
        Has<BeyondMaxAvatars>,
        Has<BlockedOrBanned>,
        Option<&PlayerModifiers>,
        Option<&InheritedVisibility>,
    )>,
    globals: Query<&GlobalTransform>,
    anchors: Query<&Transform, Without<ReactionParticle>>,
    mut particles: Query<(
        Entity,
        &DespawnWith,
        &mut ReactionParticle,
        &mut Transform,
        &mut MeshTag,
        &mut Mesh3d,
        &mut Visibility,
    )>,
) {
    let camera = camera.single().ok().map(GlobalTransform::translation);
    for (entity, owner, mut particle, mut transform, mut tag, mut mesh, mut visibility) in
        &mut particles
    {
        // where its player's reactions rise from, and whether they show there
        let place = players.get(owner.0).ok().and_then(
            |(attach_points, is_local, beyond_max, blocked, modifiers, drawn)| {
                let avatar = globals.get(attach_points.position).ok()?;
                // an avatar scaled to nothing has no place for one
                let (scale, _) = avatar_units(avatar)?;
                let nametag = anchors.get(attach_points.nametag).ok()?;
                let anchor =
                    avatar.transform_point(nametag.translation) + Vec3::Y * ANCHOR_RISE * scale;
                let shown = nametags.0
                    && !beyond_max
                    && !blocked
                    && !modifiers.is_some_and(|modifiers| modifiers.hide)
                    && drawn.is_none_or(|drawn| drawn.get())
                    && (is_local
                        || camera.is_some_and(|camera| camera.distance(anchor) <= DRAW_DISTANCE));
                Some((anchor, shown))
            },
        );

        if !particle
            .motion
            .tick(time.delta_secs(), place.map(|(anchor, _)| anchor))
        {
            commands.entity(entity).try_despawn();
            continue;
        }

        let pose = particle.motion.pose().filter(|_| {
            place.is_some_and(|(_, shown)| shown) && particle.built == Some(assets.generation)
        });
        let Some(pose) = pose else {
            if *visibility != Visibility::Hidden {
                *visibility = Visibility::Hidden;
            }
            continue;
        };
        *transform = Transform::from_translation(pose.position).with_scale(Vec3::splat(pose.size));
        let opacity = opacity_tag(pose.opacity);
        if tag.0 != opacity {
            tag.0 = opacity;
            // bevy's mesh extraction doesn't watch MeshTag (0.16); Mesh3d is on its list
            mesh.set_changed();
        }
        if *visibility != Visibility::Inherited {
            *visibility = Visibility::Inherited;
        }
    }
}
