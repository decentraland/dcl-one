//! Console commands end-to-end tests read the client's chat and voice with (docs/guidelines/running.md,
//! "Test commands"), registered in every build:
//!
//! - `/chat_history [count]`: the chat messages the client has received, kept in a bounded buffer
//!   that reads the same [`ChatEvent`]s the HUD does.
//! - `/voice_state`: the microphone, and in a `livekit` build the voice rooms and who is speaking.

use std::collections::VecDeque;

use alloy_core::primitives::Address;
use bevy::prelude::*;
use bevy_console::ConsoleCommand;
use common::{
    structs::{MicState, PrimaryUser},
    util::AsH160,
};
use console::DoAddConsoleCommand;
use serde_json::{json, Value};
use wallet::Wallet;

use crate::{
    chat_marker_things,
    chat_reaction::chat_message_id,
    global_crdt::{process_transport_updates, ChatEvent, ForeignPlayer},
};
#[cfg(feature = "livekit")]
use crate::{
    livekit::{
        participant::{ActiveSpeaker, LivekitParticipant},
        room::{ConnectFailed, Connected, Disconnected, Reconnecting},
        LivekitTransport,
    },
    SceneRoom, Transport,
};

/// Chat messages kept for `/chat_history`.
const CHAT_CAPACITY: usize = 200;
/// Messages `/chat_history` returns when given no count.
const CHAT_DEFAULT_COUNT: usize = 50;

pub struct CommsAutomationPlugin;

impl Plugin for CommsAutomationPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ChatLog>()
            // after the system that spawns a foreign player, so a first message is not recorded
            // before its sender exists
            .add_systems(Update, record_chat.after(process_transport_updates))
            .add_console_command::<ChatHistoryCommand, _>(chat_history_cmd)
            .add_console_command::<VoiceStateCommand, _>(voice_state_cmd);
    }
}

// --- /chat_history ---

struct ChatEntry {
    timestamp: f64,
    channel: String,
    message: String,
    /// `None` when the sender is unknown, such as a player gone before the message was recorded
    address: Option<Address>,
}

#[derive(Resource, Default)]
struct ChatLog {
    entries: VecDeque<ChatEntry>,
    /// Messages recorded since the client started, those dropped from `entries` included
    total: u64,
}

impl ChatLog {
    fn push(&mut self, entry: ChatEntry) {
        if self.entries.len() == CHAT_CAPACITY {
            self.entries.pop_front();
        }
        self.entries.push_back(entry);
        self.total += 1;
    }

    /// The most recent `count` messages, oldest first.
    fn latest(&self, count: usize) -> impl Iterator<Item = &ChatEntry> {
        self.entries
            .iter()
            .skip(self.entries.len().saturating_sub(count))
    }
}

/// Whether a chat event is a message from a player, as the HUD's chat stream takes it: not a
/// console or system line (no sender), the local player's own command, an emote marker, or a DM
/// (its channel is the partner's wallet; `/dm_history` has those, and a received DM is not a chat
/// event at all). Another player's text that starts with `/` is chat: the HUD shows it.
fn is_chat_message(event: &ChatEvent, me: Option<Entity>) -> bool {
    event.sender != Entity::PLACEHOLDER
        && !(Some(event.sender) == me && event.message.starts_with('/'))
        && !chat_marker_things::ALL
            .iter()
            .any(|marker| event.message.starts_with(*marker))
        && event.channel.as_h160().is_none()
}

/// The sender's address: the local player's own, or a foreign player's.
fn sender_address(
    sender: Entity,
    own: Option<(Entity, Address)>,
    foreign: impl Fn(Entity) -> Option<Address>,
) -> Option<Address> {
    match own {
        Some((entity, address)) if entity == sender => Some(address),
        _ => foreign(sender),
    }
}

fn record_chat(
    mut events: EventReader<ChatEvent>,
    mut log: ResMut<ChatLog>,
    primary: Query<Entity, With<PrimaryUser>>,
    foreign: Query<&ForeignPlayer>,
    wallet: Res<Wallet>,
) {
    if events.is_empty() {
        return;
    }
    let me = primary.single().ok();
    let own = me.zip(wallet.address());
    for event in events.read().filter(|event| is_chat_message(event, me)) {
        let address = sender_address(event.sender, own, |entity| {
            foreign.get(entity).ok().map(|player| player.address)
        });
        log.push(ChatEntry {
            timestamp: event.timestamp,
            channel: event.channel.clone(),
            message: event.message.clone(),
            address,
        });
    }
}

fn chat_history_json(log: &ChatLog, count: usize) -> Value {
    json!({
        "total": log.total,
        "messages": log.latest(count).map(|entry| json!({
            "timestamp": entry.timestamp,
            "channel": entry.channel,
            "message": entry.message,
            "address": entry.address.map(|address| format!("{address:#x}")),
            // what `/react` names the message by
            "messageId": entry.address.map(|address| chat_message_id(address, entry.timestamp)),
        })).collect::<Vec<_>>(),
    })
}

/// Report, as JSON, the chat messages the client received, oldest first: `total` counts every
/// message recorded since start, `messages` holds the most recent of the last 200 as `timestamp`
/// (as the chat event carries it: days since 1899-12-30), `channel`, `message`, the sender's
/// `address` and the `messageId` `/react` takes. Console lines, commands and DMs are not recorded
#[derive(clap::Parser, ConsoleCommand)]
#[command(name = "/chat_history")]
struct ChatHistoryCommand {
    /// how many of the most recent messages to return (default 50)
    count: Option<usize>,
}

fn chat_history_cmd(mut input: ConsoleCommand<ChatHistoryCommand>, log: Res<ChatLog>) {
    if let Some(Ok(command)) = input.take() {
        let count = command.count.unwrap_or(CHAT_DEFAULT_COUNT);
        input.reply_ok(chat_history_json(&log, count).to_string());
    }
}

// --- /voice_state ---

/// A voice room's connection, as the room's marker components say it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(not(feature = "livekit"), allow(dead_code))]
enum RoomState {
    /// not connected yet, including a room that has not started
    Connecting,
    Connected,
    Reconnecting,
    Disconnected,
    /// the connection was refused
    Failed,
}

#[cfg_attr(not(feature = "livekit"), allow(dead_code))]
impl RoomState {
    fn from_markers(connected: bool, reconnecting: bool, disconnected: bool, failed: bool) -> Self {
        if connected {
            Self::Connected
        } else if reconnecting {
            Self::Reconnecting
        } else if disconnected {
            Self::Disconnected
        } else if failed {
            Self::Failed
        } else {
            Self::Connecting
        }
    }

    fn name(self) -> &'static str {
        match self {
            Self::Connecting => "connecting",
            Self::Connected => "connected",
            Self::Reconnecting => "reconnecting",
            Self::Disconnected => "disconnected",
            Self::Failed => "failed",
        }
    }
}

#[cfg_attr(not(feature = "livekit"), allow(dead_code))]
struct VoiceRoom {
    /// a scene's room rather than the realm's
    scene: bool,
    state: RoomState,
}

/// What the LiveKit rooms say about voice; known only in a `livekit` build.
#[cfg_attr(not(feature = "livekit"), allow(dead_code))]
struct LivekitVoice {
    rooms: Vec<VoiceRoom>,
    /// sorted, once each
    speakers: Vec<Address>,
}

#[cfg_attr(not(feature = "livekit"), allow(dead_code))]
impl LivekitVoice {
    fn new(
        rooms: impl IntoIterator<Item = VoiceRoom>,
        speakers: impl IntoIterator<Item = Address>,
    ) -> Self {
        let mut speakers = speakers.into_iter().collect::<Vec<_>>();
        speakers.sort_unstable();
        speakers.dedup();
        Self {
            rooms: rooms.into_iter().collect(),
            speakers,
        }
    }

    fn connected(&self) -> bool {
        self.rooms
            .iter()
            .any(|room| room.state == RoomState::Connected)
    }
}

struct VoiceState {
    mic: MicState,
    livekit: Option<LivekitVoice>,
}

impl VoiceState {
    fn to_json(&self) -> Value {
        let livekit = self.livekit.as_ref();
        json!({
            "mic": { "available": self.mic.available, "enabled": self.mic.enabled },
            "livekit": livekit.is_some(),
            "connected": livekit.map(|livekit| livekit.connected()),
            "rooms": livekit.map(|livekit| livekit.rooms.iter().map(|room| json!({
                "scene": room.scene,
                "state": room.state.name(),
            })).collect::<Vec<_>>()),
            "speakers": livekit.map(|livekit| livekit.speakers.iter()
                .map(|address| format!("{address:#x}"))
                .collect::<Vec<_>>()),
        })
    }
}

/// The realm's and the scene's LiveKit rooms; the private chat room carries no voice and is
/// not among them (it is no [`Transport`]).
#[cfg(feature = "livekit")]
type VoiceRooms<'w, 's> = Query<
    'w,
    's,
    (
        Has<Connected>,
        Has<Reconnecting>,
        Has<Disconnected>,
        Has<ConnectFailed>,
        Has<SceneRoom>,
    ),
    (With<Transport>, With<LivekitTransport>),
>;

#[cfg(feature = "livekit")]
type ActiveSpeakers<'w, 's> = Query<'w, 's, &'static LivekitParticipant, With<ActiveSpeaker>>;

/// Report, as JSON, the microphone as `mic` (`available`, `enabled`) and, in a build with the
/// `livekit` feature, `connected` (a realm or scene voice room is connected), `rooms` (each
/// `scene` and `state`: connecting, connected, reconnecting, disconnected or failed) and
/// `speakers` (addresses speaking now, the local player's included). `livekit` says whether the
/// build has them; without it they are null
#[derive(clap::Parser, ConsoleCommand)]
#[command(name = "/voice_state")]
struct VoiceStateCommand;

fn voice_state_cmd(
    mut input: ConsoleCommand<VoiceStateCommand>,
    mic: Res<MicState>,
    #[cfg(feature = "livekit")] rooms: VoiceRooms,
    #[cfg(feature = "livekit")] speakers: ActiveSpeakers,
) {
    if let Some(Ok(_)) = input.take() {
        #[cfg(feature = "livekit")]
        let livekit = Some(LivekitVoice::new(
            rooms.iter().map(
                |(connected, reconnecting, disconnected, failed, scene)| VoiceRoom {
                    scene,
                    state: RoomState::from_markers(connected, reconnecting, disconnected, failed),
                },
            ),
            speakers
                .iter()
                .filter_map(|participant| participant.identity().as_str().as_h160()),
        ));
        #[cfg(not(feature = "livekit"))]
        let livekit = None;

        let state = VoiceState {
            mic: mic.clone(),
            livekit,
        };
        input.reply_ok(state.to_json().to_string());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn address(last: u8) -> Address {
        Address::from([last; 20])
    }

    fn entry(message: &str, address: Option<Address>) -> ChatEntry {
        ChatEntry {
            timestamp: 45_000.5,
            channel: "Nearby".to_owned(),
            message: message.to_owned(),
            address,
        }
    }

    fn event(sender: Entity, channel: &str, message: &str) -> ChatEvent {
        ChatEvent {
            timestamp: 45_000.5,
            sender,
            channel: channel.to_owned(),
            message: message.to_owned(),
        }
    }

    fn messages(log: &ChatLog, count: usize) -> Vec<&str> {
        log.latest(count).map(|e| e.message.as_str()).collect()
    }

    #[test]
    fn the_log_keeps_the_most_recent_messages_and_counts_them_all() {
        let mut log = ChatLog::default();
        for i in 0..CHAT_CAPACITY + 5 {
            log.push(entry(&i.to_string(), None));
        }
        assert_eq!(log.entries.len(), CHAT_CAPACITY);
        assert_eq!(log.total, CHAT_CAPACITY as u64 + 5);
        assert_eq!(log.entries.front().unwrap().message, "5");
        assert_eq!(
            log.entries.back().unwrap().message,
            (CHAT_CAPACITY + 4).to_string()
        );
    }

    #[test]
    fn the_latest_messages_come_oldest_first() {
        let mut log = ChatLog::default();
        for message in ["a", "b", "c"] {
            log.push(entry(message, None));
        }
        assert_eq!(messages(&log, 2), ["b", "c"]);
        assert_eq!(messages(&log, 3), ["a", "b", "c"]);
        assert_eq!(messages(&log, 50), ["a", "b", "c"]);
        assert!(messages(&log, 0).is_empty());
    }

    #[test]
    fn only_player_chat_is_a_chat_message() {
        let me = Some(Entity::from_raw(5));
        let player = Entity::from_raw(7);
        assert!(is_chat_message(&event(player, "Nearby", "hello"), me));
        assert!(is_chat_message(&event(player, "some scene", "hello"), me));
        // no sender: a console or system line
        assert!(!is_chat_message(
            &event(Entity::PLACEHOLDER, "Nearby", "[ok]"),
            me
        ));
        // the local player's command, but another player's text that starts with `/`
        assert!(!is_chat_message(
            &event(Entity::from_raw(5), "System", "/teleport 1 2"),
            me
        ));
        assert!(is_chat_message(&event(player, "Nearby", "/shrug"), me));
        for marker in chat_marker_things::ALL {
            assert!(!is_chat_message(
                &event(player, "Nearby", &format!("{marker}dance 1")),
                me
            ));
        }
        let partner = format!("{:#x}", address(3));
        assert!(!is_chat_message(&event(player, &partner, "hi"), me));
        assert!(!is_chat_message(&event(player, &partner[2..], "hi"), me));
    }

    #[test]
    fn a_sender_is_the_local_player_a_foreign_player_or_unknown() {
        let me = Entity::from_raw(1);
        let other = Entity::from_raw(2);
        let foreign = |entity| (entity == other).then(|| address(9));
        let own = Some((me, address(5)));
        assert_eq!(sender_address(me, own, foreign), Some(address(5)));
        assert_eq!(sender_address(other, own, foreign), Some(address(9)));
        assert_eq!(sender_address(Entity::from_raw(3), own, foreign), None);
        // not signed in: the local player has no address
        assert_eq!(sender_address(me, None, foreign), None);
    }

    #[test]
    fn chat_events_are_recorded_with_their_senders() {
        let mut wallet = Wallet::default();
        wallet.finalize_as_guest();
        let own = wallet.address().unwrap();

        let mut app = App::new();
        app.add_event::<ChatEvent>()
            .init_resource::<ChatLog>()
            .insert_resource(wallet)
            .add_systems(Update, record_chat);
        let me = app.world_mut().spawn(PrimaryUser::default()).id();
        let gone = app.world_mut().spawn_empty().id();
        app.world_mut().despawn(gone);
        for ev in [
            event(me, "Nearby", "mine"),
            event(Entity::PLACEHOLDER, "Nearby", "a console line"),
            event(gone, "Nearby", "from someone who left"),
            event(me, &format!("{:#x}", address(3)), "a dm"),
        ] {
            app.world_mut().send_event(ev);
        }
        app.update();

        let log = app.world().resource::<ChatLog>();
        assert_eq!(log.total, 2);
        assert_eq!(messages(log, 10), ["mine", "from someone who left"]);
        assert_eq!(log.entries[0].address, Some(own));
        assert_eq!(log.entries[1].address, None);
    }

    #[test]
    fn the_chat_history_is_json_with_camel_case_keys() {
        let mut log = ChatLog::default();
        log.push(entry("older", None));
        log.push(entry("hello", Some(address(0xab))));
        let json = chat_history_json(&log, 1);
        assert_eq!(
            json,
            json!({
                "total": 2,
                "messages": [{
                    "timestamp": 45_000.5,
                    "channel": "Nearby",
                    "message": "hello",
                    "address": format!("{:#x}", address(0xab)),
                    "messageId": chat_message_id(address(0xab), 45_000.5),
                }],
            })
        );
        assert_eq!(json["messages"][0]["address"].as_str().unwrap().len(), 42);

        let json = chat_history_json(&log, 50);
        assert_eq!(json["messages"].as_array().unwrap().len(), 2);
        assert_eq!(json["messages"][0]["address"], Value::Null);
        assert_eq!(json["messages"][0]["messageId"], Value::Null);
        assert_eq!(chat_history_json(&log, 0)["messages"], json!([]));
    }

    #[test]
    fn a_room_is_in_the_state_its_markers_say() {
        use RoomState::*;
        let state = RoomState::from_markers;
        assert_eq!(state(true, false, false, false), Connected);
        assert_eq!(state(false, true, false, false), Reconnecting);
        assert_eq!(state(false, false, true, false), Disconnected);
        // a refused room keeps its `Connecting` marker beside `ConnectFailed`
        assert_eq!(state(false, false, false, true), Failed);
        assert_eq!(state(false, false, false, false), Connecting);
    }

    #[test]
    fn voice_is_connected_when_a_voice_room_is() {
        let room = |scene, state| VoiceRoom { scene, state };
        let connected = |rooms| LivekitVoice::new(rooms, []).connected();
        assert!(!connected(vec![]));
        assert!(!connected(vec![
            room(false, RoomState::Connecting),
            room(true, RoomState::Reconnecting),
        ]));
        assert!(connected(vec![
            room(false, RoomState::Disconnected),
            room(true, RoomState::Connected),
        ]));
    }

    #[test]
    fn speakers_are_listed_once_in_order() {
        let voice = LivekitVoice::new([], [address(9), address(2), address(9)]);
        assert_eq!(voice.speakers, [address(2), address(9)]);
    }

    #[test]
    fn voice_state_without_livekit_has_only_the_mic() {
        let state = VoiceState {
            mic: MicState {
                available: true,
                enabled: false,
            },
            livekit: None,
        };
        assert_eq!(
            state.to_json(),
            json!({
                "mic": { "available": true, "enabled": false },
                "livekit": false,
                "connected": null,
                "rooms": null,
                "speakers": null,
            })
        );
    }

    #[test]
    fn voice_state_with_livekit_reports_rooms_and_speakers() {
        let state = VoiceState {
            mic: MicState {
                available: true,
                enabled: true,
            },
            livekit: Some(LivekitVoice::new(
                [
                    VoiceRoom {
                        scene: false,
                        state: RoomState::Connected,
                    },
                    VoiceRoom {
                        scene: true,
                        state: RoomState::Connecting,
                    },
                ],
                [address(0xab)],
            )),
        };
        assert_eq!(
            state.to_json(),
            json!({
                "mic": { "available": true, "enabled": true },
                "livekit": true,
                "connected": true,
                "rooms": [
                    { "scene": false, "state": "connected" },
                    { "scene": true, "state": "connecting" },
                ],
                "speakers": [format!("{:#x}", address(0xab))],
            })
        );
    }
}
