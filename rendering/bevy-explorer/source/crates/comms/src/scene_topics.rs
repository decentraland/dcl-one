//! Scene topic messages (`~system/CommsApi`) on the scene room, in both directions.

use bevy::{ecs::system::SystemParam, prelude::*};
use bevy_console::ConsoleCommand;
use common::{
    rpc::{RpcCall, RpcCallEvent, RpcOrigin, RpcStreamSender},
    structs::{server_mode, CurrentRealm},
};
use console::{AddDebugSection, DebugSection, DoAddConsoleCommand};
use dcl_component::proto_components::kernel::comms::rfc4;

use crate::{NetworkMessage, NetworkMessageRecipient, SceneRoom, Transport};

pub struct SceneTopicsPlugin;

impl Plugin for SceneTopicsPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(SceneTopicsRoutingPlugin)
            .add_console_command::<SceneTopicsCommand, _>(scene_topics_command)
            .add_debug_section(DebugSection {
                title: "Scene topic messages",
                query: "/scene_topics",
                fields: &[
                    ("Scene room", "room"),
                    ("Scene room running", "running"),
                    ("Rooms isolated per scene", "isolated"),
                    ("Subscribed scenes", "subscribedScenes"),
                    ("Published", "published"),
                    (
                        "Publishes dropped (no room for the scene)",
                        "publishDropped",
                    ),
                    ("Received", "received"),
                    (
                        "Received but dropped (room or subscriber)",
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
        app.init_resource::<SceneTopicTraffic>()
            .add_systems(Update, (subscribe_scene_topics, publish_scene_topics));
    }
}

/// On a scene root: where the scene's topic messages go.
#[derive(Component)]
struct SceneTopicInbox {
    hash: String,
    sender: RpcStreamSender<(String, Vec<u8>)>,
}

#[derive(Resource, Default)]
struct SceneTopicTraffic {
    published: u64,
    publish_dropped: u64,
    received: u64,
    receive_dropped: u64,
}

/// In a local preview and a single-scene world, any of the realm's scenes may use the room.
fn scene_rooms_isolated(realm: &CurrentRealm) -> bool {
    if server_mode() {
        return true;
    }
    let single_scene_world = realm
        .config
        .scenes_urn
        .as_ref()
        .is_some_and(|urns| urns.len() == 1);
    !(realm.is_local() || single_scene_world)
}

fn room_carries_scene_topics(
    room: &SceneRoom,
    running: bool,
    scene_hash: &str,
    isolated: bool,
) -> bool {
    running && (!isolated || room.0.eq_ignore_ascii_case(scene_hash))
}

#[cfg(feature = "livekit")]
type RoomRunning = Has<crate::livekit::room::Connected>;
#[cfg(not(feature = "livekit"))]
type RoomRunning = ();

#[cfg(feature = "livekit")]
fn is_running(running: bool) -> bool {
    running
}
#[cfg(not(feature = "livekit"))]
fn is_running(_: ()) -> bool {
    false
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

fn publish_scene_topics(
    mut events: EventReader<RpcCallEvent>,
    rooms: Query<(&Transport, &SceneRoom, RoomRunning)>,
    realm: Res<CurrentRealm>,
    mut traffic: ResMut<SceneTopicTraffic>,
) {
    let isolated = scene_rooms_isolated(&realm);
    for (hash, data) in events.read().filter_map(|ev| match (&ev.origin, &ev.call) {
        (RpcOrigin::Scene { hash, .. }, RpcCall::PublishSceneTopic { data }) => Some((hash, data)),
        _ => None,
    }) {
        let room = rooms.iter().find(|(_, room, running)| {
            room_carries_scene_topics(room, is_running(*running), hash, isolated)
        });
        let Some((transport, ..)) = room else {
            traffic.publish_dropped += 1;
            continue;
        };
        let packet = rfc4::Packet {
            message: Some(rfc4::packet::Message::Scene(rfc4::Scene {
                scene_id: hash.to_string(),
                data: data.clone(),
            })),
            protocol_version: 100,
        };
        let message = NetworkMessage::targetted_reliable(&packet, NetworkMessageRecipient::All);
        if transport.sender.try_send(message).is_ok() {
            traffic.published += 1;
        } else {
            traffic.publish_dropped += 1;
        }
    }
}

#[derive(SystemParam)]
pub struct SceneTopicRouter<'w, 's> {
    inboxes: Query<'w, 's, &'static SceneTopicInbox>,
    rooms: Query<'w, 's, (&'static SceneRoom, RoomRunning)>,
    realm: Res<'w, CurrentRealm>,
    traffic: ResMut<'w, SceneTopicTraffic>,
}

impl SceneTopicRouter<'_, '_> {
    /// `payload` is the scene message after its type byte.
    pub fn deliver(
        &mut self,
        transport: Entity,
        scene_hash: &str,
        sender: String,
        payload: Vec<u8>,
    ) {
        self.traffic.received += 1;
        let isolated = scene_rooms_isolated(&self.realm);
        let carried = self.rooms.get(transport).is_ok_and(|(room, running)| {
            room_carries_scene_topics(room, is_running(running), scene_hash, isolated)
        });
        let delivered = carried
            && self
                .inboxes
                .iter()
                .find(|inbox| inbox.hash == scene_hash)
                .is_some_and(|inbox| inbox.sender.send((sender, payload)).is_ok());
        if !delivered {
            self.traffic.receive_dropped += 1;
        }
    }
}

/// Read the scene room scene topic messages use and how many went out and came in.
#[derive(clap::Parser, ConsoleCommand)]
#[command(name = "/scene_topics")]
struct SceneTopicsCommand;

fn scene_topics_command(
    mut input: ConsoleCommand<SceneTopicsCommand>,
    rooms: Query<(&SceneRoom, RoomRunning)>,
    realm: Res<CurrentRealm>,
    inboxes: Query<&SceneTopicInbox>,
    traffic: Res<SceneTopicTraffic>,
) {
    if let Some(Ok(_)) = input.take() {
        let room = rooms.iter().next();
        input.reply_ok(
            serde_json::json!({
                "room": room.map(|(room, _)| room.0.clone()),
                "running": room.map(|(_, running)| is_running(running)),
                "isolated": scene_rooms_isolated(&realm),
                "subscribedScenes": inboxes.iter().count(),
                "published": traffic.published,
                "publishDropped": traffic.publish_dropped,
                "received": traffic.received,
                "receiveDropped": traffic.receive_dropped,
                "unavailable": {"room": "No scene room", "running": "No scene room"},
            })
            .to_string(),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn room(hash: &str) -> SceneRoom {
        SceneRoom(hash.to_owned())
    }

    #[test]
    fn an_isolated_room_carries_only_its_own_scene() {
        assert!(room_carries_scene_topics(
            &room("bafyA"),
            true,
            "bafya",
            true
        ));
        assert!(!room_carries_scene_topics(
            &room("bafyA"),
            true,
            "bafyB",
            true
        ));
    }

    #[test]
    fn a_shared_room_carries_every_scene() {
        assert!(room_carries_scene_topics(
            &room("bafyA"),
            true,
            "bafyB",
            false
        ));
    }

    #[test]
    fn a_room_that_is_not_running_carries_nothing() {
        assert!(!room_carries_scene_topics(
            &room("bafyA"),
            false,
            "bafyA",
            true
        ));
        assert!(!room_carries_scene_topics(
            &room("bafyA"),
            false,
            "bafyA",
            false
        ));
    }

    #[test]
    fn rooms_are_shared_in_a_preview_and_a_single_scene_world() {
        let mut realm = CurrentRealm::default();
        assert!(scene_rooms_isolated(&realm), "Genesis City");
        realm.config.scenes_urn = Some(vec!["urn:a".to_owned()]);
        assert!(!scene_rooms_isolated(&realm), "single-scene world");
        realm.config.scenes_urn = Some(vec!["urn:a".to_owned(), "urn:b".to_owned()]);
        assert!(scene_rooms_isolated(&realm), "multi-scene world");
        realm.config.local_scene_parcels = Some(vec!["0,0".to_owned()]);
        assert!(!scene_rooms_isolated(&realm), "local preview");
    }
}
