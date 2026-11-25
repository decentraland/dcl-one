use std::time::Duration;

use bevy::time::TimeUpdateStrategy;
use common::{rpc::RpcStreamReceiver, structs::PrimaryPlayerRes};
use comms::{chat_reaction::emoji_to_atlas_index, NetworkMessage, TransportType};
use tokio::sync::mpsc;

use super::*;

const PEER: Address = Address::repeat_byte(0xab);
const FRAME: f32 = 0.05;

/// The plugin in an app of its own, with a signed-in local player and a stream to the HUD.
struct World {
    app: App,
    local: Entity,
    me: Address,
    hud: RpcStreamReceiver<ReactionData>,
}

/// The wire index of each reaction the local player was reported to make, in order.
#[derive(Resource, Default)]
struct Reported(Vec<i32>);

fn world() -> World {
    let mut wallet = Wallet::default();
    wallet.finalize_as_guest_with_seed([3; 32]);
    let me = wallet.address().unwrap();

    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        console::ConsolePlugin {
            add_bevy_console: false,
        },
    ))
    .add_event::<SystemApi>()
    .add_event::<ReactionEvent>()
    // what a block or ban is asked of
    .init_resource::<ban_block::PlatformBanStatus>()
    .init_resource::<ban_block::SceneAdmission>()
    .insert_resource(wallet)
    .init_resource::<Reported>()
    .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f32(
        FRAME,
    )))
    .add_plugins(ReactionPlugin)
    .add_systems(
        Update,
        |mut reacted: EventReader<PlayerReacted>, mut reported: ResMut<Reported>| {
            reported
                .0
                .extend(reacted.read().map(|reaction| reaction.emoji_index));
        },
    );
    // as the engine spawns them: the outbox is the plugin's to add
    let local = app.world_mut().spawn(PrimaryUser::default()).id();
    app.insert_resource(PrimaryPlayerRes(local));

    let (sender, hud) = RpcStreamSender::channel();
    app.world_mut()
        .send_event(SystemApi::GetReactionStream(sender));
    app.update();

    World {
        app,
        local,
        me,
        hud,
    }
}

impl World {
    fn pick(&mut self, emoji: &str) {
        self.app
            .world_mut()
            .send_event(SystemApi::SendReaction(emoji.to_owned()));
    }

    /// A stand-in for a peer: what the comms decode gives every player it hears from.
    fn peer(&mut self) -> Entity {
        self.app.world_mut().spawn(ReactionGuard::default()).id()
    }

    fn hear(&mut self, sender: Entity, emoji: &str, timestamp: f32, count: i32) {
        self.app.world_mut().send_event(ReactionEvent {
            sender,
            from: PEER,
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
    let mut world = world();
    let mut wire = world.transport(TransportType::Livekit);
    let me = format!("{:#x}", world.me);

    world.pick("👍");
    world.app.update();
    assert_eq!(
        world.played(),
        [ReactionPlayed {
            player: world.local,
            from: world.me,
            emoji: "👍",
        }]
    );
    assert_eq!(world.streamed(), [(me.clone(), "👍".to_owned())]);
    assert!(sent(&mut wire).is_empty());

    world.pick("👍");
    world.pick("🔥");
    world.app.update();
    assert_eq!(world.streamed().len(), 2);
    assert!(sent(&mut wire).is_empty());

    // one reliable message for each emoji
    world.run(0.3);
    assert_eq!(sent(&mut wire), [false, false]);
    assert_eq!(world.stats().sent, 2);
    world.run(1.0);
    assert!(sent(&mut wire).is_empty());

    // each pick is reported once, as the index it is sent as
    let (thumb, fire) = (emoji_to_atlas_index("👍"), emoji_to_atlas_index("🔥"));
    assert_eq!(world.reported(), [thumb, thumb, fire]);
}

#[test]
fn picks_go_where_nearby_chat_goes() {
    let mut world = world();
    let mut room = world.transport(TransportType::Livekit);
    let mut socket = world.transport(TransportType::WebsocketRoom);
    let mut island = world.transport(TransportType::Archipelago);
    let mut pulse = world.transport(TransportType::Pulse);
    world.pick("👍");
    world.run(0.5);
    assert_eq!(sent(&mut room).len(), 1);
    assert_eq!(sent(&mut socket).len(), 1);
    assert!(sent(&mut island).is_empty());
    assert!(sent(&mut pulse).is_empty());
}

#[test]
fn an_emoji_the_wire_does_not_carry_does_nothing_anywhere() {
    let mut world = world();
    let mut wire = world.transport(TransportType::Livekit);
    for picked in ["🇬🇧", "thumbs up", "", "👍👍"] {
        world.pick(picked);
    }
    world.run(1.0);
    assert!(world.streamed().is_empty());
    assert!(sent(&mut wire).is_empty());
    assert!(world.reported().is_empty());
    assert_eq!(world.stats().refused_picks, 4);
    assert_eq!(world.stats().played, 0);
}

#[test]
fn what_plays_is_the_emoji_the_others_will_see() {
    let mut world = world();
    // the picker's heart has no variation selector; the wire's does
    world.pick("\u{2764}");
    world.app.update();
    assert_eq!(world.played()[0].emoji, "\u{2764}\u{fe0f}");
}

#[test]
fn no_one_signed_in_reacts_as_no_one() {
    let mut world = world();
    world.app.world_mut().resource_mut::<Wallet>().disconnect();
    world.pick("👍");
    world.app.update();
    assert!(world.played().is_empty());
    assert_eq!(world.stats().refused_picks, 1);
}

#[test]
fn a_peers_reactions_play_one_at_a_time() {
    let mut world = world();
    let peer = world.peer();
    world.hear(peer, "🔥", 7.0, 3);
    world.app.update();
    // the first at once, on the peer
    assert_eq!(
        world.played(),
        [ReactionPlayed {
            player: peer,
            from: PEER,
            emoji: "🔥",
        }]
    );
    assert!(world.app.world().get::<ReactionQueue>(peer).is_some());

    world.run(0.5);
    let peer_address = format!("{PEER:#x}");
    assert_eq!(world.streamed(), vec![(peer_address, "🔥".to_owned()); 3]);
    assert_eq!(world.stats().played, 3);
    // nothing is kept on a peer once the gap after their last has run out
    world.run(0.3);
    assert!(world.app.world().get::<ReactionQueue>(peer).is_none());
}

#[test]
fn messages_of_one_reaction_each_play_a_gap_apart_too() {
    let mut world = world();
    let peer = world.peer();
    // a message a frame, each with a stamp of its own
    for message in 0..4 {
        world.hear(peer, "🔥", 1.0 + message as f32, 1);
        world.app.update();
        // the first at once; the next two arrive inside its gap and wait
        if message < 3 {
            assert_eq!(world.stats().played, 1);
        }
    }
    // then one for each gap that runs out
    world.run(0.15);
    assert_eq!(world.stats().played, 2);
    for played in 3..=4 {
        world.run(0.2);
        assert_eq!(world.stats().played, played);
    }
}

#[test]
fn several_messages_in_a_frame_share_their_senders_queue() {
    let mut world = world();
    let peer = world.peer();
    world.hear(peer, "🔥", 1.0, 1);
    world.hear(peer, "👍", 2.0, 1);
    // the copy that came by the other room
    world.hear(peer, "🔥", 1.0, 1);
    world.run(1.0);
    assert_eq!(
        world
            .streamed()
            .into_iter()
            .map(|(_, emoji)| emoji)
            .collect::<Vec<_>>(),
        ["🔥", "👍"]
    );
    assert_eq!(world.stats().received, 3);
    assert_eq!(world.stats().dropped.duplicate, 1);
}

#[test]
fn a_blocked_sender_plays_nothing() {
    let mut world = world();
    let peer = world.peer();
    world
        .app
        .world_mut()
        .entity_mut(peer)
        .insert(BlockedOrBanned);
    world.hear(peer, "🔥", 1.0, 5);
    world.run(1.0);
    assert!(world.streamed().is_empty());
    assert_eq!(world.stats().dropped.blocked, 1);
}

#[test]
fn a_sender_blocked_while_their_reactions_wait_plays_no_more() {
    let mut world = world();
    let peer = world.peer();
    world.hear(peer, "🔥", 1.0, 5);
    world.app.update();
    assert_eq!(world.streamed().len(), 1);

    world
        .app
        .world_mut()
        .entity_mut(peer)
        .insert(BlockedOrBanned);
    world.run(2.0);
    assert!(world.streamed().is_empty());
    assert!(world.app.world().get::<ReactionQueue>(peer).is_none());
    assert_eq!(world.stats().dropped.blocked, 4);
}

#[test]
fn a_sender_who_leaves_takes_their_waiting_reactions() {
    let mut world = world();
    let peer = world.peer();
    world.hear(peer, "🔥", 1.0, 5);
    world.app.update();
    world.app.world_mut().despawn(peer);
    // a message of theirs still on its way in finds no one
    world.hear(peer, "👍", 2.0, 1);
    world.run(2.0);
    assert_eq!(world.streamed().len(), 1);
}
