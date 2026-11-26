//! The reactions' console commands and their section of the debug panel.

use bevy::prelude::*;
use bevy_console::ConsoleCommand;
use common::{
    structs::{AppConfig, ChatBubbleVisibility, PrimaryPlayerRes},
    util::AsH160,
};
use comms::{
    chat_reaction::picked_emoji_tile,
    global_crdt::{ForeignPlayer, ReactionEvent},
};
use console::DebugSection;
use dcl_component::proto_components::kernel::comms::rfc4;
use scene_runner::update_world::text_shape::{GlyphAtlas, GlyphAtlases, Page};
use serde_json::json;
use system_bridge::SystemApi;
use wallet::Wallet;

use super::{
    traffic::{ReactionGuard, ReactionOutbox, ReactionQueue},
    ReactionParticle, ReactionStats, ATLAS,
};

pub const SECTION: DebugSection = DebugSection {
    title: "Situational reactions",
    query: "/reaction_state",
    fields: &[
        (
            "Shown over avatars (Chat bubbles and reactions setting)",
            "world",
        ),
        ("Picks played", "picked"),
        ("Picks waiting to be sent", "outbox"),
        ("Picks refused", "refusedPicks"),
        ("Messages sent", "sent"),
        ("Messages received", "received"),
        ("Waiting to play (reactions / senders)", "queued"),
        ("Played", "played"),
        ("Particles (alive / drawn / awaiting glyphs)", "particles"),
        ("Glyph atlas (colour px)", "atlas"),
        ("Passed to the HUD", "streamed"),
        ("HUD streams", "subscribers"),
        ("Dropped: sender blocked or banned", "dropped.blocked"),
        ("Dropped: copy of a message", "dropped.duplicate"),
        ("Dropped: unknown emoji", "dropped.unknownEmoji"),
        ("Dropped: sender over their rate", "dropped.rateLimited"),
        ("Dropped: sender's queue full", "dropped.queueFull"),
        (
            "Played with no particle: too many alive",
            "dropped.particleCap",
        ),
        (
            "Played with no particle: player not shown",
            "dropped.notShown",
        ),
        (
            "Played with no particle: no colour glyph",
            "dropped.noGlyphs",
        ),
    ],
    actions: &[
        ("React with a heart", "/reaction_send \u{2665}\u{fe0f}"),
        (
            "Play 10 as if sent to yourself",
            "/reaction_simulate local \u{1f525} 10",
        ),
        (
            "Play 10 from the nearest player",
            "/reaction_simulate nearest \u{1f525} 10",
        ),
        ("Rebuild the glyph atlas", "/reaction_atlas_rebuild"),
    ],
    inputs: &[],
};

/// Report the reactions as JSON: `world` (the Chat bubbles and reactions setting lets them
/// show over avatars), `picked` (the local user's picks that played, which no peer's reaction
/// moves), `outbox` (the local user's picks not yet sent), `refusedPicks` (their picks that
/// were no emoji the wire carries, or had no wallet to send them), `sent` (messages put on a
/// room's transport, one per emoji per send: none with no room to carry it),
/// `received` (peers' messages), `queued` (the reactions waiting to play, and the senders with
/// any waiting or inside the gap after their last), `played`, `particles` (those alive, those
/// of them drawn as of the last frame, and those waiting for their emoji's glyphs: to be
/// rastered, or for room in a full atlas), `atlas` (the size in pixels of the glyph atlas page
/// the emoji are drawn from, 0 before the first), `streamed` (the reactions passed to the HUD)
/// to `subscribers` streams, and `dropped`, by cause: `blocked` (messages and waiting
/// reactions of a sender blocked or banned), `duplicate` (a message's copy by another room),
/// `unknownEmoji`, `rateLimited` (messages), `queueFull` (reactions that gave way to newer
/// ones), `particleCap` (reactions that played with no particle, as many as may be alive),
/// `notShown` (reactions that played with no particle, their player having no avatar or a
/// hidden one, being far from the camera, or being the local user in first person) and
/// `noGlyphs` (reactions that played with no particle, their emoji not laid out or having no
/// glyph in colour). The counts are of this session.
#[derive(clap::Parser, ConsoleCommand)]
#[command(name = "/reaction_state")]
pub struct ReactionStateCommand;

pub fn reaction_state(
    mut cmd: ConsoleCommand<ReactionStateCommand>,
    stats: Res<ReactionStats>,
    config: Res<AppConfig>,
    outboxes: Query<&ReactionOutbox>,
    queues: Query<&ReactionQueue>,
    particles: Query<(&ReactionParticle, &Visibility)>,
    atlases: Res<GlyphAtlases>,
) {
    let Some(Ok(_)) = cmd.take() else {
        return;
    };
    // made when the first emoji is drawn from it
    let atlas = atlases.get(&ATLAS.key);
    let generation = atlas.map(GlyphAtlas::generation);
    let count = |test: fn(&ReactionParticle, &Visibility, Option<u32>) -> bool| {
        particles
            .iter()
            .filter(|(particle, visibility)| test(particle, visibility, generation))
            .count()
    };
    cmd.reply_ok(
        json!({
            "world": config.chat_bubble_visibility != ChatBubbleVisibility::None,
            "picked": stats.picked,
            "outbox": outboxes.iter().map(ReactionOutbox::waiting).sum::<u32>(),
            "refusedPicks": stats.refused_picks,
            "sent": stats.sent,
            "received": stats.received,
            "queued": [
                queues.iter().map(ReactionQueue::len).sum::<usize>(),
                queues.iter().count(),
            ],
            "played": stats.played,
            "particles": [
                count(|_, _, _| true),
                count(|_, visibility, _| *visibility != Visibility::Hidden),
                count(|particle, _, generation| {
                    particle.built.is_none() || particle.built != generation
                }),
            ],
            // read from the atlas: the assets follow it only while a particle is alive
            "atlas": atlas
                .and_then(|atlas| atlas.page_image(Page::Color))
                .map_or(0, |(_, size)| size),
            "streamed": stats.streamed,
            "subscribers": stats.subscribers,
            "dropped": {
                "blocked": stats.dropped.blocked,
                "duplicate": stats.dropped.duplicate,
                "unknownEmoji": stats.dropped.unknown_emoji,
                "rateLimited": stats.dropped.rate_limited,
                "queueFull": stats.dropped.queue_full,
                "particleCap": stats.dropped.particle_cap,
                "notShown": stats.dropped.not_shown,
                "noGlyphs": stats.dropped.no_glyphs,
            },
        })
        .to_string(),
    );
}

/// React with `emoji` as the local user, the way the system API's `sendReaction` does. An emoji
/// the wire does not carry does nothing. Replies with nothing.
#[derive(clap::Parser, ConsoleCommand)]
#[command(name = "/reaction_send")]
pub struct ReactionSendCommand {
    emoji: String,
}

pub fn reaction_send(
    mut cmd: ConsoleCommand<ReactionSendCommand>,
    mut requests: EventWriter<SystemApi>,
) {
    if let Some(Ok(command)) = cmd.take() {
        requests.write(SystemApi::SendReaction(command.emoji));
        cmd.reply_ok("");
    }
}

/// Take a message of `count` reactions (1 unless given) with `emoji` as if `target` had sent
/// it: `local`, `nearest` (the peer nearest the local player) or a peer's address. It passes
/// what a peer's message passes, with no stamp, so a repeat is not taken for a copy. Nothing
/// is sent to anyone. Replies with the `target` taken (`local` or an address) and the emoji's
/// wire `index`.
#[derive(clap::Parser, ConsoleCommand)]
#[command(name = "/reaction_simulate")]
pub struct ReactionSimulateCommand {
    target: String,
    emoji: String,
    #[arg(allow_negative_numbers = true)]
    count: Option<i32>,
}

pub fn reaction_simulate(
    mut cmd: ConsoleCommand<ReactionSimulateCommand>,
    mut commands: Commands,
    local: Res<PrimaryPlayerRes>,
    wallet: Res<Wallet>,
    peers: Query<(Entity, &ForeignPlayer, &GlobalTransform)>,
    transforms: Query<&GlobalTransform>,
    mut events: EventWriter<ReactionEvent>,
) {
    let Some(Ok(command)) = cmd.take() else {
        return;
    };
    let Some(index) = picked_emoji_tile(&command.emoji) else {
        cmd.reply_failed("not an emoji the wire carries");
        return;
    };
    let peer =
        |(entity, peer, _): (Entity, &ForeignPlayer, &GlobalTransform)| (entity, peer.address);
    let target = match command.target.as_str() {
        "local" => Some((local.0, wallet.address().unwrap_or_default())),
        "nearest" => {
            let here = transforms
                .get(local.0)
                .map(GlobalTransform::translation)
                .unwrap_or_default();
            peers
                .iter()
                .min_by(|a, b| {
                    let distance = |player: &(Entity, &ForeignPlayer, &GlobalTransform)| {
                        player.2.translation().distance_squared(here)
                    };
                    distance(a).total_cmp(&distance(b))
                })
                .map(peer)
        }
        address => address.as_h160().and_then(|address| {
            peers
                .iter()
                .find(|(_, peer, _)| peer.address == address)
                .map(peer)
        }),
    };
    let Some((sender, from)) = target else {
        cmd.reply_failed("target must be `local`, `nearest` or the address of a player here");
        return;
    };
    // the local player has no messages of their own to guard against
    commands
        .entity(sender)
        .try_insert_if_new(ReactionGuard::default());
    events.write(ReactionEvent {
        sender,
        reaction: rfc4::Reaction {
            emoji_index: index,
            timestamp: 0.0,
            count: command.count.unwrap_or(1),
        },
    });
    cmd.reply_ok(
        json!({
            "target": if sender == local.0 { "local".to_owned() } else { format!("{from:#x}") },
            "index": index,
        })
        .to_string(),
    );
}

/// Have the reactions' glyph atlas rebuilt at its next upkeep, as a glyph that finds no room
/// in it does: its textures are replaced, the glyphs no particle shows are freed and every
/// particle alive gets a mesh for the new page. An atlas with no glyph to free is left as it
/// is, and so is one no emoji has been drawn from yet. Replies with nothing; `atlas` and
/// `particles` in `/reaction_state` show what followed.
#[derive(clap::Parser, ConsoleCommand)]
#[command(name = "/reaction_atlas_rebuild")]
pub struct ReactionAtlasRebuildCommand;

pub fn reaction_atlas_rebuild(
    mut cmd: ConsoleCommand<ReactionAtlasRebuildCommand>,
    mut commands: Commands,
) {
    if let Some(Ok(_)) = cmd.take() {
        // the atlases are written only when the command is given, not asked for every frame
        commands.queue(|world: &mut World| {
            if let Some(atlas) = world.resource_mut::<GlyphAtlases>().get_mut(&ATLAS.key) {
                atlas.request_rebuild();
            }
        });
        cmd.reply_ok("");
    }
}
