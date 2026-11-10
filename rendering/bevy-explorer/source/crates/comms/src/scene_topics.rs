//! Scene topic messages (`~system/CommsApi`) on the scene room, in both directions.

use bevy::{ecs::system::SystemParam, prelude::*};
use bevy_console::ConsoleCommand;
use common::{
    rpc::{RpcCall, RpcCallEvent, RpcOrigin, RpcStreamSender},
    sets::SceneSets,
    structs::{server_mode, CurrentRealm},
};
use console::{AddDebugSection, DebugSection, DoAddConsoleCommand};
use dcl_component::proto_components::kernel::comms::rfc4;
use dcl_types::{CommsMessageType, MAX_TOPIC_MESSAGE_BYTES};

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
                    ("Scene rooms", "sceneRooms"),
                    ("First scene room", "room"),
                    ("First scene room running", "running"),
                    ("Rooms isolated per scene", "isolated"),
                    ("Subscribed scenes", "subscribedScenes"),
                    ("Published", "published"),
                    (
                        "Publishes dropped (not a topic message, or no room for the scene)",
                        "publishDropped",
                    ),
                    ("Received", "received"),
                    (
                        "Received but dropped (too large, no room or no subscribed scene)",
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
        app.init_resource::<SceneTopicTraffic>().add_systems(
            Update,
            (subscribe_scene_topics, publish_scene_topics).in_set(SceneSets::RestrictedActions),
        );
    }
}

/// On a scene root: where the scene's topic messages go.
#[derive(Component)]
struct SceneTopicInbox {
    hash: String,
    sender: RpcStreamSender<(String, Vec<u8>)>,
}

/// A message dropped because a scene's inbox is full is only logged: the engine can't see a
/// remote scene's channel fill up.
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

#[cfg(all(feature = "livekit", not(test)))]
type RoomRunning = Has<crate::livekit::room::Connected>;
// `Connected` can only be added to a real LiveKit room, so tests mark a running room with this.
#[cfg(test)]
type RoomRunning = Has<TestRoomRunning>;
#[cfg(not(any(feature = "livekit", test)))]
type RoomRunning = ();

#[cfg(test)]
#[derive(Component)]
pub(crate) struct TestRoomRunning;

#[cfg(any(feature = "livekit", test))]
fn is_running(running: bool) -> bool {
    running
}
#[cfg(not(any(feature = "livekit", test)))]
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

/// The scene runtime is untrusted (on web its memory is reachable by scene code), so the frame
/// is checked again here: only a topic message within the size limit goes out. Topic messages
/// are broadcast even for an authoritative scene, as unity-explorer's `CommsApiWrap` does; only
/// the message bus is routed to the auth server.
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
        if data.first() != Some(&(CommsMessageType::CommsData as u8))
            || data.len() > MAX_TOPIC_MESSAGE_BYTES
        {
            traffic.publish_dropped += 1;
            continue;
        }
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
    /// `payload` is the scene message after its type byte. A payload larger than any conforming
    /// publisher sends is dropped. Only the first live inbox for the hash gets the message.
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
            // the type byte, already taken off, counts toward the limit
            && payload.len() < MAX_TOPIC_MESSAGE_BYTES
            && self
                .inboxes
                .iter()
                .find(|inbox| inbox.hash == scene_hash && !inbox.sender.is_closed())
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
        // A client holds one scene room at a time; a headless server holds one per hosted scene.
        let room = rooms.iter().next();
        input.reply_ok(
            serde_json::json!({
                "sceneRooms": rooms.iter().count(),
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
    use crate::TransportType;

    fn routing_app() -> App {
        let mut app = App::new();
        app.add_event::<RpcCallEvent>();
        app.init_resource::<CurrentRealm>();
        app.add_plugins(SceneTopicsRoutingPlugin);
        app
    }

    fn spawn_running_room(
        app: &mut App,
        hash: &str,
    ) -> tokio::sync::mpsc::Receiver<NetworkMessage> {
        let (sender, receiver) = tokio::sync::mpsc::channel(8);
        let context = app.world_mut().spawn_empty().id();
        app.world_mut().spawn((
            Transport {
                transport_type: TransportType::Livekit,
                sender,
                control: None,
                context,
            },
            SceneRoom(hash.to_owned()),
            TestRoomRunning,
        ));
        receiver
    }

    #[test]
    fn publish_goes_to_the_scene_room_and_drops_what_is_not_a_topic_message() {
        let mut app = routing_app();
        let mut own = spawn_running_room(&mut app, "bafyscene");
        let mut other = spawn_running_room(&mut app, "bafyother");
        let scene = app.world_mut().spawn_empty().id();
        let at_limit = vec![CommsMessageType::CommsData as u8; MAX_TOPIC_MESSAGE_BYTES];
        let over_limit = vec![CommsMessageType::CommsData as u8; MAX_TOPIC_MESSAGE_BYTES + 1];
        for data in [
            vec![3, 1, 0, b't', b'x'],
            at_limit,
            // a message bus frame, an oversized frame and an empty one
            vec![1, b'h', b'i'],
            over_limit,
            vec![],
        ] {
            app.world_mut().send_event(RpcCallEvent {
                origin: RpcOrigin::Scene {
                    entity: scene,
                    hash: "bafyscene".into(),
                },
                call: RpcCall::PublishSceneTopic { data },
            });
        }
        app.update();
        assert!(own.try_recv().is_ok());
        assert!(own.try_recv().is_ok());
        assert!(own.try_recv().is_err());
        assert!(other.try_recv().is_err(), "another scene's room");
        let traffic = app.world().resource::<SceneTopicTraffic>();
        assert_eq!((traffic.published, traffic.publish_dropped), (2, 3));
    }

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
