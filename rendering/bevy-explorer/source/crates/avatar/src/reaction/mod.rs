//! Situational reactions: an emoji a player picks to show how they feel about what is going
//! on, played once for each time it was picked.
//!
//! The local user's pick plays at once and is sent to the players around them, gathered with
//! their other picks into one message per emoji. A peer's message is held to what its sender
//! may send, then its reactions play one at a time. Each one played is passed to the HUD, which
//! shows it on screen while the "Chat Reactions" setting is on.

mod diagnostics;
mod traffic;

#[cfg(test)]
mod tests;

use alloy_core::primitives::Address;
use analytics::{
    data_definition::{
        SegmentEvent, SegmentEventReactSituation, SegmentEventReactionVisualization,
    },
    segment_system::{SegmentConfig, SegmentMetricsEvents},
};
use bevy::{platform::collections::HashMap, prelude::*};
use common::{
    rpc::RpcStreamSender,
    sets::SceneSets,
    structs::{AppConfig, PrimaryUser},
};
use comms::{
    broadcast_to,
    chat_reaction::{atlas_index_to_emoji, canonical_emoji, emoji_atlas_tile},
    global_crdt::{process_transport_updates, ForeignPlayer, ReactionEvent},
    BlockedOrBanned, BroadcastTarget, Transport,
};
use console::{AddDebugSection, DoAddConsoleCommand};
use dcl_component::proto_components::kernel::comms::rfc4;
use system_bridge::{ReactionData, SystemApi};
use wallet::Wallet;

use self::traffic::{ReactionGuard, ReactionOutbox, ReactionQueue, Refused};

pub struct ReactionPlugin;

impl Plugin for ReactionPlugin {
    fn build(&self, app: &mut App) {
        app.add_event::<ReactionPlayed>()
            .init_resource::<ReactionStats>()
            .register_required_components::<ForeignPlayer, ReactionGuard>()
            .register_required_components::<PrimaryUser, ReactionOutbox>()
            .add_systems(
                Update,
                (
                    accept_local_reactions,
                    receive_reactions.run_if(on_event::<ReactionEvent>),
                    play_queued_reactions,
                    flush_reactions,
                    pipe_reactions_to_scene,
                )
                    .chain()
                    .in_set(SceneSets::PostLoop)
                    // a sender first heard from this frame has its entity by now
                    .after(process_transport_updates),
            )
            .add_systems(
                Update,
                report_reaction_visualization.run_if(resource_changed::<AppConfig>),
            )
            .add_console_command::<diagnostics::ReactionStateCommand, _>(
                diagnostics::reaction_state,
            )
            .add_console_command::<diagnostics::ReactionSendCommand, _>(diagnostics::reaction_send)
            .add_console_command::<diagnostics::ReactionSimulateCommand, _>(
                diagnostics::reaction_simulate,
            )
            .add_debug_section(diagnostics::SECTION);
    }
}

/// One reaction as it plays: `emoji` for `player`, the local user or a peer, who is `from` on
/// the wire.
#[derive(Event, Debug, Clone, Copy, PartialEq, Eq)]
struct ReactionPlayed {
    player: Entity,
    from: Address,
    emoji: &'static str,
}

/// What has happened to reactions this session, for the debug panel.
#[derive(Resource, Default)]
struct ReactionStats {
    /// The local user's picks that were no emoji the wire can carry, or had no one to send them.
    refused_picks: u32,
    /// Messages sent, one per emoji per send.
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
}

/// The local user's picks: each plays at once and waits in the outbox to be sent.
fn accept_local_reactions(
    mut requests: EventReader<SystemApi>,
    wallet: Res<Wallet>,
    mut local: Query<(Entity, &mut ReactionOutbox), With<PrimaryUser>>,
    mut played: EventWriter<ReactionPlayed>,
    mut stats: ResMut<ReactionStats>,
    mut metrics: ResMut<SegmentMetricsEvents>,
    segment_config: Res<SegmentConfig>,
) {
    for request in requests.read() {
        let SystemApi::SendReaction(picked) = request else {
            continue;
        };
        // what plays is the emoji the wire carries for the pick, so the local user sees what
        // the others will
        let reaction = canonical_emoji(picked)
            .and_then(|emoji| emoji_atlas_tile(&emoji))
            .and_then(|tile| Some((tile, atlas_index_to_emoji(tile)?)));
        let (Some((tile, emoji)), Some(from), Ok((player, mut outbox))) =
            (reaction, wallet.address(), local.single_mut())
        else {
            debug!("no reaction for {picked:?}");
            stats.refused_picks += 1;
            continue;
        };
        outbox.push(tile);
        played.write(ReactionPlayed {
            player,
            from,
            emoji,
        });
        stats.played += 1;
        metrics.add_event(
            SegmentEvent::ReactSituation(SegmentEventReactSituation { reaction_id: tile }),
            segment_config.common(),
        );
    }
}

/// Peers' messages: what their sender may send is queued on the sender, to play one at a time.
fn receive_reactions(
    mut commands: Commands,
    mut events: EventReader<ReactionEvent>,
    mut senders: Query<(
        &mut ReactionGuard,
        Option<&mut ReactionQueue>,
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
        let Ok((mut guard, queue, blocked)) = senders.get_mut(event.sender) else {
            continue;
        };
        if blocked {
            stats.dropped.blocked += 1;
            continue;
        }
        let (emoji, count) = match guard.admit(&event.reaction, now) {
            Ok(admitted) => admitted,
            Err(refused) => {
                debug!("reaction from {:#x} refused: {refused:?}", event.from);
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
            None => started
                .entry(event.sender)
                .or_insert_with(|| ReactionQueue::new(event.from))
                .push(emoji, count),
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
    mut queues: Query<(Entity, &mut ReactionQueue, Has<BlockedOrBanned>)>,
    mut played: EventWriter<ReactionPlayed>,
    mut stats: ResMut<ReactionStats>,
) {
    for (player, mut queue, blocked) in &mut queues {
        // blocked since they were queued
        if blocked {
            stats.dropped.blocked += queue.len() as u32;
            commands.entity(player).try_remove::<ReactionQueue>();
            continue;
        }
        let from = queue.from;
        for emoji in queue.tick(time.delta_secs()) {
            played.write(ReactionPlayed {
                player,
                from,
                emoji,
            });
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
            broadcast_to(
                transports.iter(),
                BroadcastTarget::WEBSOCKET | BroadcastTarget::LIVEKIT,
                false,
                &rfc4::Packet {
                    message: Some(rfc4::packet::Message::Reaction(outgoing.into())),
                    protocol_version: 100,
                },
            );
            stats.sent += 1;
        }
    }
}

/// Every reaction played, the local user's included, goes to the HUD to show on screen.
/// Nothing is passed while the "Chat Reactions" setting is off: the HUD would show none of it.
fn pipe_reactions_to_scene(
    mut played: EventReader<ReactionPlayed>,
    mut requests: EventReader<SystemApi>,
    mut senders: Local<Vec<RpcStreamSender<ReactionData>>>,
    config: Res<AppConfig>,
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

    if !config.chat_reactions || senders.is_empty() {
        played.clear();
        return;
    }
    for reaction in played.read() {
        let data = ReactionData {
            from: format!("{:#x}", reaction.from),
            emoji: reaction.emoji.to_owned(),
        };
        for sender in senders.iter() {
            let _ = sender.send(data.clone());
        }
        stats.streamed += 1;
    }
}

/// Analytics hear of each change of the "Chat Reactions" setting, whoever made it.
fn report_reaction_visualization(
    config: Res<AppConfig>,
    mut last: Local<Option<bool>>,
    mut metrics: ResMut<SegmentMetricsEvents>,
    segment_config: Res<SegmentConfig>,
) {
    let state = config.chat_reactions;
    if last.replace(state).is_some_and(|last| last != state) {
        metrics.add_event(
            SegmentEvent::ReactionVisualization(SegmentEventReactionVisualization { state }),
            segment_config.common(),
        );
    }
}
