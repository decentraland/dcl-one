//! Who is banned from the scene the player stands in. The gatekeeper writes the scene room's
//! LiveKit metadata as `{"sceneAdmins": [..], "bannedAddresses": [..]}` and rewrites it on every
//! ban and unban; the list holds only while that room is the current scene room, so leaving the
//! scene or losing the room clears it.

use alloy_core::primitives::Address;
use bevy::{platform::collections::HashSet, prelude::*};
use bevy_console::ConsoleCommand;
use console::{AddDebugSection, DebugSection, DoAddConsoleCommand};
use serde::Deserialize;
use serde_json::json;

use crate::{livekit::participant::RoomMetadataChanged, SceneRoom, SceneRoomConnection};
#[cfg(test)]
use crate::SetCurrentScene;

pub struct SceneBansPlugin;

impl Plugin for SceneBansPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SceneBans>()
            .add_observer(scene_room_metadata)
            .add_observer(scene_room_removed);
        app.add_console_command::<SceneBansCommand, _>(scene_bans);
        app.add_debug_section(DebugSection {
            title: "Scene bans",
            query: "/scene_bans",
            fields: &[
                ("Scene room", "sceneRoom"),
                ("Banned addresses", "banned"),
                ("Nearby chats dropped", "droppedChats"),
            ],
            actions: &[],
            inputs: &[],
        });
    }
}

#[derive(Resource, Default, Debug)]
pub struct SceneBans {
    room: Option<Entity>,
    banned: HashSet<Address>,
    /// Nearby chat lines dropped because their sender is banned; survives scene changes.
    dropped_chats: u64,
}

impl SceneBans {
    pub fn is_banned(&self, address: Address) -> bool {
        self.banned.contains(&address)
    }

    /// Whether a nearby chat line from `address` is dropped, counting the drop.
    pub fn drops_chat_from(&mut self, address: Address) -> bool {
        let banned = self.is_banned(address);
        self.dropped_chats += u64::from(banned);
        banned
    }

    fn set(&mut self, room: Entity, metadata: &str) {
        self.room = Some(room);
        self.banned = banned_addresses(metadata);
    }

    fn clear(&mut self, room: Entity) {
        if self.room == Some(room) {
            self.room = None;
            self.banned.clear();
        }
    }
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct SceneRoomMetadata {
    #[serde(default)]
    banned_addresses: Vec<String>,
}

/// Unreadable metadata bans nobody; an entry that is not an address is skipped.
fn banned_addresses(metadata: &str) -> HashSet<Address> {
    serde_json::from_str::<SceneRoomMetadata>(metadata)
        .unwrap_or_default()
        .banned_addresses
        .iter()
        .filter_map(|address| address.trim().parse().ok())
        .collect()
}

fn scene_room_metadata(
    trigger: Trigger<RoomMetadataChanged>,
    scene_rooms: Query<(), With<SceneRoom>>,
    current: Res<SceneRoomConnection>,
    mut bans: ResMut<SceneBans>,
) {
    let room = trigger.room;
    // only the client's current scene room speaks for the scene the player is in
    let is_current = current.0.as_ref().is_some_and(|(_, _, entity)| *entity == room);
    if is_current && scene_rooms.contains(room) {
        bans.set(room, &trigger.metadata);
    }
}

fn scene_room_removed(trigger: Trigger<OnRemove, SceneRoom>, mut bans: ResMut<SceneBans>) {
    bans.clear(trigger.target());
}

/// `/scene_bans` prints who the current scene room bans, as JSON.
#[derive(clap::Parser, ConsoleCommand)]
#[command(name = "/scene_bans")]
struct SceneBansCommand;

fn scene_bans(
    mut input: ConsoleCommand<SceneBansCommand>,
    bans: Res<SceneBans>,
    current: Res<SceneRoomConnection>,
) {
    if input.take().is_none() {
        return;
    }
    let mut banned: Vec<_> = bans.banned.iter().map(|a| format!("{a:#x}")).collect();
    banned.sort();
    let scene = current
        .0
        .as_ref()
        .filter(|(_, _, room)| bans.room == Some(*room))
        .map(|(scene, _, _)| scene.scene_id.clone());
    input.reply_ok(
        json!({
            "sceneRoom": scene,
            "banned": banned,
            "droppedChats": bans.dropped_chats,
        })
        .to_string(),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    const A: &str = "0x00000000000000000000000000000000000000aa";
    const B: &str = "0x00000000000000000000000000000000000000Bb";

    fn address(s: &str) -> Address {
        s.parse().unwrap()
    }

    #[test]
    fn reads_banned_addresses_in_any_case_and_skips_the_rest() {
        let bans = banned_addresses(&format!(
            r#"{{"sceneAdmins":["{A}"],"bannedAddresses":["{A}","{}","not-an-address",""]}}"#,
            B.to_uppercase().replace("0X", "0x")
        ));
        assert_eq!(bans.len(), 2);
        assert!(bans.contains(&address(A)));
        assert!(bans.contains(&address(&B.to_lowercase())));
    }

    #[test]
    fn unreadable_or_missing_lists_ban_nobody() {
        assert!(banned_addresses("").is_empty());
        assert!(banned_addresses("not json").is_empty());
        assert!(banned_addresses(r#"{"sceneAdmins":[]}"#).is_empty());
        assert!(banned_addresses(r#"{"bannedAddresses":null}"#).is_empty());
    }

    fn app() -> App {
        let mut app = App::new();
        app.init_resource::<SceneRoomConnection>()
            .add_plugins(SceneBansPlugin);
        app
    }

    fn metadata(app: &mut App, room: Entity) {
        app.world_mut().trigger(RoomMetadataChanged {
            room,
            metadata: format!(r#"{{"bannedAddresses":["{A}"]}}"#),
        });
        app.world_mut().flush();
    }

    fn make_current(app: &mut App, room: Entity) {
        app.world_mut().resource_mut::<SceneRoomConnection>().0 =
            Some((
                SetCurrentScene {
                    realm_name: String::new(),
                    scene_id: "scene".into(),
                },
                String::new(),
                room,
            ));
    }

    #[test]
    fn only_the_current_scene_room_sets_the_list_and_removing_it_clears_it() {
        let mut app = app();
        let other_room = app.world_mut().spawn_empty().id();
        let scene_room = app.world_mut().spawn(SceneRoom("scene".into())).id();
        let stale_scene_room = app.world_mut().spawn(SceneRoom("old".into())).id();
        make_current(&mut app, scene_room);

        metadata(&mut app, other_room);
        metadata(&mut app, stale_scene_room);
        assert!(!app.world().resource::<SceneBans>().is_banned(address(A)));

        metadata(&mut app, scene_room);
        assert!(app.world().resource::<SceneBans>().is_banned(address(A)));

        // a stale room going away leaves the current list alone
        app.world_mut().despawn(stale_scene_room);
        assert!(app.world().resource::<SceneBans>().is_banned(address(A)));

        app.world_mut().despawn(scene_room);
        assert!(!app.world().resource::<SceneBans>().is_banned(address(A)));
    }

    #[test]
    fn an_unban_rewrite_lifts_the_ban() {
        let mut app = app();
        let scene_room = app.world_mut().spawn(SceneRoom("scene".into())).id();
        make_current(&mut app, scene_room);
        metadata(&mut app, scene_room);
        app.world_mut().trigger(RoomMetadataChanged {
            room: scene_room,
            metadata: r#"{"sceneAdmins":[],"bannedAddresses":[]}"#.into(),
        });
        app.world_mut().flush();
        assert!(!app.world().resource::<SceneBans>().is_banned(address(A)));
    }

    #[test]
    fn dropped_chats_are_counted() {
        let mut bans = SceneBans::default();
        bans.set(Entity::PLACEHOLDER, &format!(r#"{{"bannedAddresses":["{A}"]}}"#));
        assert!(bans.drops_chat_from(address(A)));
        assert!(!bans.drops_chat_from(address(&B.to_lowercase())));
        assert_eq!(bans.dropped_chats, 1);
    }
}
