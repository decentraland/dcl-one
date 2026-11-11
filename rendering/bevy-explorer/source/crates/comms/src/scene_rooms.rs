//! Which running scene room carries a scene's messages: the one rule for scene topic messages,
//! the message bus (both directions) and the `isConnectedSceneRoom` flag scenes read.

use bevy::{ecs::system::SystemParam, prelude::*};
use common::structs::{server_mode, CurrentRealm};

use crate::{SceneRoomConnection, ServerSceneRooms, Transport};

#[cfg(feature = "livekit")]
use crate::livekit::room::Connected;
/// Never inserted: without LiveKit no scene room runs.
#[cfg(not(feature = "livekit"))]
#[derive(Component)]
struct Connected;

/// Marks a scene room as running in tests, where no LiveKit room can connect.
#[cfg(any(test, feature = "test-support"))]
#[derive(Component)]
pub struct TestRoomRunning;

#[cfg(not(any(test, feature = "test-support")))]
type Running = With<Connected>;
#[cfg(any(test, feature = "test-support"))]
type Running = Or<(With<Connected>, With<TestRoomRunning>)>;

/// In a local preview and a single-scene world, any of the realm's scenes may use the room.
pub(crate) fn scene_rooms_isolated(realm: &CurrentRealm) -> bool {
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

/// The room carrying `scene_hash`, if it is running. A server holds one room per hosted scene;
/// a client holds the room of the scene the player is in, which carries only that scene when
/// rooms are `isolated`.
fn scene_room(
    scene_hash: &str,
    connection: &SceneRoomConnection,
    server_rooms: &ServerSceneRooms,
    isolated: bool,
    running: impl Fn(Entity) -> bool,
) -> Option<Entity> {
    let room = match server_rooms.0.get(scene_hash) {
        Some((_, room)) => Some(*room),
        None => connection
            .0
            .as_ref()
            .filter(|(scene, ..)| !isolated || scene.scene_id.eq_ignore_ascii_case(scene_hash))
            .map(|(.., room)| *room),
    };
    room.filter(|room| running(*room))
}

#[derive(SystemParam)]
pub struct SceneRooms<'w, 's> {
    connection: Res<'w, SceneRoomConnection>,
    server_rooms: Res<'w, ServerSceneRooms>,
    realm: Res<'w, CurrentRealm>,
    running: Query<'w, 's, &'static Transport, Running>,
}

impl SceneRooms<'_, '_> {
    /// The running room that carries the scene's messages.
    pub fn room(&self, scene_hash: &str) -> Option<Entity> {
        scene_room(
            scene_hash,
            &self.connection,
            &self.server_rooms,
            self.isolated(),
            |room| self.running.contains(room),
        )
    }

    pub fn transport(&self, scene_hash: &str) -> Option<&Transport> {
        self.running.get(self.room(scene_hash)?).ok()
    }

    pub(crate) fn carries(&self, transport: Entity, scene_hash: &str) -> bool {
        self.room(scene_hash) == Some(transport)
    }

    pub(crate) fn isolated(&self) -> bool {
        scene_rooms_isolated(&self.realm)
    }
}

/// Test rooms for the systems that send and receive on scene rooms.
#[cfg(any(test, feature = "test-support"))]
pub mod test_support {
    use bevy::prelude::*;
    use tokio::sync::mpsc;

    use super::TestRoomRunning;
    use crate::{
        NetworkMessage, SceneRoom, SceneRoomConnection, ServerSceneRooms, SetCurrentScene,
        Transport, TransportType,
    };

    pub fn init_scene_rooms(app: &mut App) {
        app.init_resource::<SceneRoomConnection>()
            .init_resource::<ServerSceneRooms>()
            .init_resource::<common::structs::CurrentRealm>();
    }

    /// A scene room entity that is not connected yet.
    pub fn spawn_scene_room(
        app: &mut App,
        hash: &str,
        context: Entity,
    ) -> (Entity, mpsc::Receiver<NetworkMessage>) {
        let (sender, receiver) = mpsc::channel(8);
        let room = app.world_mut().spawn((
            Transport {
                transport_type: TransportType::Livekit,
                sender,
                control: None,
                context,
            },
            SceneRoom(hash.to_owned()),
        ));
        (room.id(), receiver)
    }

    pub fn set_running(app: &mut App, room: Entity) {
        app.world_mut().entity_mut(room).insert(TestRoomRunning);
    }

    /// The room a client holds for the scene the player is in.
    pub fn spawn_client_room(
        app: &mut App,
        hash: &str,
        context: Entity,
    ) -> (Entity, mpsc::Receiver<NetworkMessage>) {
        let (room, receiver) = spawn_scene_room(app, hash, context);
        app.world_mut().resource_mut::<SceneRoomConnection>().0 = Some((
            SetCurrentScene {
                realm_name: "realm".to_owned(),
                scene_id: hash.to_owned(),
            },
            "livekit:test".to_owned(),
            room,
        ));
        (room, receiver)
    }

    /// One of the rooms a server holds, one per hosted scene.
    pub fn spawn_server_room(
        app: &mut App,
        hash: &str,
        context: Entity,
    ) -> (Entity, mpsc::Receiver<NetworkMessage>) {
        let (room, receiver) = spawn_scene_room(app, hash, context);
        app.world_mut()
            .resource_mut::<ServerSceneRooms>()
            .0
            .insert(hash.to_owned(), ("livekit:test".to_owned(), room));
        (room, receiver)
    }

    pub fn local_preview(app: &mut App) {
        app.world_mut()
            .resource_mut::<common::structs::CurrentRealm>()
            .config
            .local_scene_parcels = Some(vec!["0,0".to_owned()]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SetCurrentScene;

    const ROOM: Entity = Entity::from_raw(7);

    fn client(hash: &str) -> SceneRoomConnection {
        SceneRoomConnection(Some((
            SetCurrentScene {
                realm_name: "realm".to_owned(),
                scene_id: hash.to_owned(),
            },
            "livekit:test".to_owned(),
            ROOM,
        )))
    }

    fn server(hash: &str) -> ServerSceneRooms {
        ServerSceneRooms([(hash.to_owned(), ("livekit:test".to_owned(), ROOM))].into())
    }

    fn room(
        hash: &str,
        connection: &SceneRoomConnection,
        server_rooms: &ServerSceneRooms,
        isolated: bool,
        running: bool,
    ) -> Option<Entity> {
        scene_room(hash, connection, server_rooms, isolated, |_| running)
    }

    #[test]
    fn an_isolated_room_carries_only_its_own_scene() {
        let none = ServerSceneRooms::default();
        assert_eq!(
            room("bafya", &client("bafyA"), &none, true, true),
            Some(ROOM)
        );
        assert_eq!(room("bafyB", &client("bafyA"), &none, true, true), None);
    }

    #[test]
    fn a_shared_room_carries_every_scene() {
        let none = ServerSceneRooms::default();
        assert_eq!(
            room("bafyB", &client("bafyA"), &none, false, true),
            Some(ROOM)
        );
    }

    #[test]
    fn a_room_that_is_not_running_carries_nothing() {
        let none = ServerSceneRooms::default();
        assert_eq!(room("bafyA", &client("bafyA"), &none, true, false), None);
        assert_eq!(room("bafyA", &client("bafyA"), &none, false, false), None);
        let no_client = SceneRoomConnection::default();
        assert_eq!(
            room("bafyA", &no_client, &server("bafyA"), true, false),
            None
        );
    }

    #[test]
    fn a_server_room_carries_only_its_own_scene() {
        let no_client = SceneRoomConnection::default();
        assert_eq!(
            room("bafyA", &no_client, &server("bafyA"), true, true),
            Some(ROOM)
        );
        assert_eq!(
            room("bafyB", &no_client, &server("bafyA"), true, true),
            None
        );
    }

    #[test]
    fn no_room_carries_nothing() {
        let (no_client, none) = (SceneRoomConnection::default(), ServerSceneRooms::default());
        assert_eq!(room("bafyA", &no_client, &none, false, true), None);
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
