//! Scene topic messages (`~system/CommsApi`): each scene's inbox, delivery of received messages
//! and the shared scene-message diagnostics. `restricted_actions` sends them, with the message bus.

use bevy::{ecs::system::SystemParam, prelude::*};
use bevy_console::ConsoleCommand;
use common::{
    rpc::{RpcCall, RpcCallEvent, RpcOrigin, RpcStreamSender},
    sets::SceneSets,
};
use console::{AddDebugSection, DebugSection, DoAddConsoleCommand};
use dcl_types::{CommsMessageType, MAX_TOPIC_MESSAGE_BYTES};

use crate::{scene_messages::SceneMessageTraffic, scene_rooms::SceneRooms, SceneRoom};

pub struct SceneTopicsPlugin;

impl Plugin for SceneTopicsPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(SceneTopicsRoutingPlugin)
            .add_console_command::<SceneTopicsCommand, _>(scene_topics_command)
            .add_debug_section(DebugSection {
                title: "Scene messages",
                query: "/scene_topics",
                fields: &[
                    ("Scene rooms", "sceneRooms"),
                    ("First scene room", "room"),
                    ("First scene room connected", "running"),
                    ("Rooms isolated per scene", "isolated"),
                    ("Subscribed scenes", "subscribedScenes"),
                    ("Published", "published"),
                    (
                        "Publishes dropped (not a topic message, or no room for the scene)",
                        "publishDropped",
                    ),
                    ("Banned sender drops", "bannedSenderDrops"),
                    ("Filtered inbound CRDT", "filteredInboundCrdt"),
                    ("Filtered outbound CRDT", "filteredOutboundCrdt"),
                    ("Invalid or oversized messages", "invalidMessages"),
                    ("Binary payload limit (bytes)", "binaryPayloadLimit"),
                    ("Received", "received"),
                    (
                        "Received but dropped (banned, invalid, no room or no subscribed scene)",
                        "receiveDropped",
                    ),
                ],
                actions: &[],
                inputs: &[],
            });
    }
}

pub(crate) struct SceneTopicsRoutingPlugin;

impl Plugin for SceneTopicsRoutingPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SceneMessageTraffic>().add_systems(
            Update,
            subscribe_scene_topics.in_set(SceneSets::RestrictedActions),
        );
    }
}

/// On a scene root: where the scene's topic messages go.
#[derive(Component)]
struct SceneTopicInbox {
    hash: String,
    sender: RpcStreamSender<(String, Vec<u8>)>,
}

/// Whether a scene's publish is a topic message within the size limit. The scene runtime is
/// untrusted (on web its memory is reachable by scene code), so the engine checks the frame
/// again before sending it.
pub fn is_topic_frame(data: &[u8]) -> bool {
    data.first() == Some(&(CommsMessageType::CommsData as u8))
        && data.len() <= MAX_TOPIC_MESSAGE_BYTES
}

fn subscribe_scene_topics(mut commands: Commands, mut events: EventReader<RpcCallEvent>) {
    for ev in events.read() {
        if let (RpcOrigin::Scene { entity, hash }, RpcCall::SubscribeSceneTopics { sender }) =
            (&ev.origin, &ev.call)
        {
            commands.entity(*entity).try_insert(SceneTopicInbox {
                hash: hash.to_string(),
                sender: sender.clone(),
            });
        }
    }
}

#[derive(SystemParam)]
pub struct SceneTopicRouter<'w, 's> {
    inboxes: Query<'w, 's, &'static SceneTopicInbox>,
    pub(crate) traffic: ResMut<'w, SceneMessageTraffic>,
}

impl SceneTopicRouter<'_, '_> {
    /// For a message that arrived on the room carrying its scene. `payload` is the scene
    /// message after its type byte. A payload larger than any conforming publisher sends is
    /// dropped. Only the first live inbox for the hash gets the message.
    pub fn deliver(&mut self, scene_hash: &str, sender: String, payload: Vec<u8>) {
        self.traffic.received += 1;
        // the type byte, already taken off, counts toward the limit
        let delivered = payload.len() < MAX_TOPIC_MESSAGE_BYTES
            && self
                .inboxes
                .iter()
                .find(|inbox| inbox.hash == scene_hash && !inbox.sender.is_closed())
                .is_some_and(|inbox| inbox.sender.send((sender, payload)).is_ok());
        if !delivered {
            self.traffic.receive_dropped += 1;
        }
    }

    pub(crate) fn drop_topic_message(&mut self) {
        self.traffic.received += 1;
        self.traffic.receive_dropped += 1;
    }
}

/// Scene-room routing, topic traffic and engine message policy counters.
#[derive(clap::Parser, ConsoleCommand)]
#[command(name = "/scene_topics")]
struct SceneTopicsCommand;

fn scene_topics_command(
    mut input: ConsoleCommand<SceneTopicsCommand>,
    rooms: Query<(Entity, &SceneRoom)>,
    scene_rooms: SceneRooms,
    inboxes: Query<&SceneTopicInbox>,
    traffic: Res<SceneMessageTraffic>,
) {
    if let Some(Ok(_)) = input.take() {
        // A client holds one scene room at a time; a headless server holds one per hosted scene.
        let room = rooms.iter().next();
        input.reply_ok(
            serde_json::json!({
                "sceneRooms": rooms.iter().count(),
                "room": room.map(|(_, room)| room.0.clone()),
                "running": room.map(|(entity, room)| {
                    scene_rooms.carries(entity, &room.0) && scene_rooms.connected(&room.0)
                }),
                "isolated": scene_rooms.isolated(),
                "subscribedScenes": inboxes.iter().count(),
                "published": traffic.published,
                "publishDropped": traffic.publish_dropped,
                "received": traffic.received,
                "receiveDropped": traffic.receive_dropped,
                "bannedSenderDrops": traffic.banned_senders,
                "filteredInboundCrdt": traffic.filtered_inbound,
                "filteredOutboundCrdt": traffic.filtered_outbound,
                "invalidMessages": traffic.invalid,
                "binaryPayloadLimit": dcl_types::MAX_BINARY_MESSAGE_BYTES,
                "unavailable": {"room": "No scene room", "running": "No scene room"},
            })
            .to_string(),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;

    fn routing_app() -> App {
        let mut app = App::new();
        app.add_event::<RpcCallEvent>();
        app.add_plugins(SceneTopicsRoutingPlugin);
        app
    }

    fn spawn_inbox(app: &mut App, hash: &str) -> common::rpc::RpcStreamReceiver<(String, Vec<u8>)> {
        let (sender, receiver) = RpcStreamSender::channel_with_capacity(8);
        app.world_mut().spawn(SceneTopicInbox {
            hash: hash.to_owned(),
            sender,
        });
        receiver
    }

    fn deliver(app: &mut App, scene_hash: &str, payload: Vec<u8>) {
        let scene_hash = scene_hash.to_owned();
        app.world_mut()
            .run_system_once(move |mut router: SceneTopicRouter| {
                router.deliver(&scene_hash, "0x1".to_owned(), payload.clone());
            })
            .unwrap();
    }

    #[test]
    fn delivery_skips_a_closed_inbox_for_the_same_scene() {
        let mut app = routing_app();
        // a scene with this hash being torn down, ahead of the live one in the query
        drop(spawn_inbox(&mut app, "bafyscene"));
        let mut live = spawn_inbox(&mut app, "bafyscene");
        deliver(&mut app, "bafyscene", vec![1, 0, b't']);
        assert_eq!(
            live.try_recv().unwrap(),
            ("0x1".to_owned(), vec![1, 0, b't'])
        );
        let traffic = app.world().resource::<SceneMessageTraffic>();
        assert_eq!((traffic.received, traffic.receive_dropped), (1, 0));
    }

    #[test]
    fn delivery_reaches_only_the_addressed_scene() {
        let mut app = routing_app();
        let mut other = spawn_inbox(&mut app, "bafyother");
        deliver(&mut app, "bafyscene", vec![1, 0, b't']);
        assert!(other.try_recv().is_err());
        let traffic = app.world().resource::<SceneMessageTraffic>();
        assert_eq!((traffic.received, traffic.receive_dropped), (1, 1));
    }
}
