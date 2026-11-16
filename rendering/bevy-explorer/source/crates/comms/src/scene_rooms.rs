//! Which scene room carries a scene's messages, and whether it is connected: the one rule for
//! scene topic messages, the message bus (both directions) and the `isConnectedSceneRoom` flag
//! scenes read.

use bevy::{ecs::system::SystemParam, prelude::*};
use common::structs::{server_mode, CurrentRealm};

use crate::{SceneRoomConnection, ServerSceneRooms, Transport};

#[cfg(feature = "livekit")]
use crate::livekit::room::Connected;
/// Never inserted: without LiveKit no scene room connects.
#[cfg(not(feature = "livekit"))]
#[derive(Component)]
struct Connected;

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

/// The room carrying `scene_hash`, connected or not. A server holds one room per hosted scene;
/// a client holds the room of the scene the player is in, which carries only that scene when
/// rooms are `isolated`.
fn scene_room(
    scene_hash: &str,
    connection: &SceneRoomConnection,
    server_rooms: &ServerSceneRooms,
    isolated: bool,
) -> Option<Entity> {
    match server_rooms.0.get(scene_hash) {
        Some((_, room)) => Some(*room),
        None => connection
            .0
            .as_ref()
            .filter(|(scene, ..)| !isolated || scene.scene_id.eq_ignore_ascii_case(scene_hash))
            .map(|(.., room)| *room),
    }
}

#[derive(SystemParam)]
pub struct SceneRooms<'w, 's> {
    connection: Res<'w, SceneRoomConnection>,
    server_rooms: Res<'w, ServerSceneRooms>,
    realm: Res<'w, CurrentRealm>,
    rooms: Query<'w, 's, (&'static Transport, Has<Connected>)>,
}

impl SceneRooms<'_, '_> {
    /// The room that carries the scene's messages, connected or not: a message sent before the
    /// room connects waits in the room's channel.
    pub fn room(&self, scene_hash: &str) -> Option<Entity> {
        scene_room(
            scene_hash,
            &self.connection,
            &self.server_rooms,
            self.isolated(),
        )
    }

    pub fn transport(&self, scene_hash: &str) -> Option<&Transport> {
        let (transport, _) = self.rooms.get(self.room(scene_hash)?).ok()?;
        Some(transport)
    }

    /// Whether the room that carries the scene is connected: `isConnectedSceneRoom`.
    pub fn connected(&self, scene_hash: &str) -> bool {
        self.room(scene_hash)
            .and_then(|room| self.rooms.get(room).ok())
            .is_some_and(|(_, connected)| connected)
    }

    pub(crate) fn carries(&self, transport: Entity, scene_hash: &str) -> bool {
        self.room(scene_hash) == Some(transport)
    }

    pub(crate) fn isolated(&self) -> bool {
        scene_rooms_isolated(&self.realm)
    }
}

/// Test rooms for the systems that send and receive on scene rooms.
#[cfg(test)]
pub(crate) mod test_support {
    use bevy::prelude::*;
    use tokio::sync::mpsc;

    use crate::{
        NetworkMessage, SceneRoom, SceneRoomConnection, ServerSceneRooms, SetCurrentScene,
        Transport, TransportType,
    };

    pub fn init_scene_rooms(app: &mut App) {
        app.init_resource::<SceneRoomConnection>()
            .init_resource::<ServerSceneRooms>()
            .init_resource::<common::structs::CurrentRealm>();
    }

    /// A scene room entity. It is not connected: no LiveKit room connects in tests.
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

    #[test]
    fn an_isolated_room_carries_only_its_own_scene() {
        let none = ServerSceneRooms::default();
        assert_eq!(
            scene_room("bafya", &client("bafyA"), &none, true),
            Some(ROOM)
        );
        assert_eq!(scene_room("bafyB", &client("bafyA"), &none, true), None);
    }

    #[test]
    fn a_shared_room_carries_every_scene() {
        let none = ServerSceneRooms::default();
        assert_eq!(
            scene_room("bafyB", &client("bafyA"), &none, false),
            Some(ROOM)
        );
    }

    #[test]
    fn a_server_room_carries_only_its_own_scene() {
        let no_client = SceneRoomConnection::default();
        assert_eq!(
            scene_room("bafyA", &no_client, &server("bafyA"), true),
            Some(ROOM)
        );
        assert_eq!(
            scene_room("bafyB", &no_client, &server("bafyA"), true),
            None
        );
    }

    #[test]
    fn no_room_carries_nothing() {
        let (no_client, none) = (SceneRoomConnection::default(), ServerSceneRooms::default());
        assert_eq!(scene_room("bafyA", &no_client, &none, false), None);
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
