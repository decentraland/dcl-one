//! The reactions' console commands and their section of the debug panel.

use bevy::prelude::*;
use bevy_console::ConsoleCommand;
use common::{
    structs::{AppConfig, PrimaryPlayerRes},
    util::AsH160,
};
use comms::{
    chat_reaction::{canonical_emoji, emoji_atlas_tile},
    global_crdt::{ForeignPlayer, ReactionEvent},
};
use console::DebugSection;
use dcl_component::proto_components::kernel::comms::rfc4;
use serde_json::json;
use system_bridge::SystemApi;
use wallet::Wallet;

use super::{
    traffic::{ReactionGuard, ReactionOutbox, ReactionQueue},
    ReactionStats,
};

pub const SECTION: DebugSection = DebugSection {
    title: "Situational reactions",
    query: "/reaction_state",
    fields: &[
        ("Shown on screen (Chat Reactions setting)", "screenLane"),
        ("Picks waiting to be sent", "outbox"),
        ("Picks refused", "refusedPicks"),
        ("Messages sent", "sent"),
        ("Messages received", "received"),
        ("Waiting to play (reactions / senders)", "queued"),
        ("Played", "played"),
        ("Passed to the HUD", "streamed"),
        ("HUD streams", "subscribers"),
        ("Dropped: sender blocked or banned", "dropped.blocked"),
        ("Dropped: copy of a message", "dropped.duplicate"),
        ("Dropped: unknown emoji", "dropped.unknownEmoji"),
        ("Dropped: sender over their rate", "dropped.rateLimited"),
        ("Dropped: sender's queue full", "dropped.queueFull"),
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
    ],
    inputs: &[],
};

/// Report the reactions as JSON: `screenLane` (the "Chat Reactions" setting passes them to the
/// HUD), `outbox` (the local user's picks not yet sent), `refusedPicks` (their picks that were
/// no emoji the wire carries, or had no wallet to send them), `sent` and `received` (messages,
/// one per emoji per send), `queued` (the reactions waiting to play and the senders they wait
/// on), `played`, `streamed` (the reactions passed to the HUD) to `subscribers` streams, and
/// `dropped`, by cause: `blocked` (messages and waiting reactions of a sender blocked or
/// banned), `duplicate` (a message's copy by another room), `unknownEmoji`, `rateLimited`
/// (messages) and `queueFull` (reactions that gave way to newer ones). The counts are of this
/// session.
#[derive(clap::Parser, ConsoleCommand)]
#[command(name = "/reaction_state")]
pub struct ReactionStateCommand;

pub fn reaction_state(
    mut cmd: ConsoleCommand<ReactionStateCommand>,
    stats: Res<ReactionStats>,
    config: Res<AppConfig>,
    outboxes: Query<&ReactionOutbox>,
    queues: Query<&ReactionQueue>,
) {
    let Some(Ok(_)) = cmd.take() else {
        return;
    };
    cmd.reply_ok(
        json!({
            "screenLane": config.chat_reactions,
            "outbox": outboxes.iter().map(ReactionOutbox::waiting).sum::<u32>(),
            "refusedPicks": stats.refused_picks,
            "sent": stats.sent,
            "received": stats.received,
            "queued": [
                queues.iter().map(ReactionQueue::len).sum::<usize>(),
                queues.iter().count(),
            ],
            "played": stats.played,
            "streamed": stats.streamed,
            "subscribers": stats.subscribers,
            "dropped": {
                "blocked": stats.dropped.blocked,
                "duplicate": stats.dropped.duplicate,
                "unknownEmoji": stats.dropped.unknown_emoji,
                "rateLimited": stats.dropped.rate_limited,
                "queueFull": stats.dropped.queue_full,
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
    let Some(index) = canonical_emoji(&command.emoji).and_then(|emoji| emoji_atlas_tile(&emoji))
    else {
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
        .insert_if_new(ReactionGuard::default());
    events.write(ReactionEvent {
        sender,
        from,
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
