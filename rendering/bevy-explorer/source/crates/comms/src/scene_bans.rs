//! The gatekeeper writes a scene room's LiveKit metadata as
//! `{"sceneAdmins": [..], "bannedAddresses": [..]}` and rewrites it on every ban and unban.

use alloy_core::primitives::Address;
use bevy::{platform::collections::HashSet, prelude::*};
use bevy_console::ConsoleCommand;
use console::{AddDebugSection, DebugSection, DoAddConsoleCommand};
use serde::Deserialize;
use serde_json::json;

use crate::{livekit::participant::RoomMetadataChanged, SceneRoom};

pub struct SceneBansPlugin;

impl Plugin for SceneBansPlugin {
    fn build(&self, app: &mut App) {
        app.add_observer(scene_room_metadata);
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

/// On a [`SceneRoom`]: the addresses banned from its scene.
#[derive(Component, Default, Debug)]
pub struct SceneBans {
    banned: HashSet<Address>,
    dropped_chats: u64,
}

impl SceneBans {
    pub fn is_banned(&self, address: Address) -> bool {
        self.banned.contains(&address)
    }

    /// Counts the drop when `address` is banned.
    pub fn drops_chat_from(&mut self, address: Address) -> bool {
        let banned = self.is_banned(address);
        self.dropped_chats += u64::from(banned);
        banned
    }
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct SceneRoomMetadata {
    #[serde(default)]
    banned_addresses: Vec<String>,
}

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
    mut commands: Commands,
    mut scene_rooms: Query<Option<&mut SceneBans>, With<SceneRoom>>,
) {
    let Ok(bans) = scene_rooms.get_mut(trigger.room) else {
        return;
    };
    let banned = banned_addresses(&trigger.metadata);
    match bans {
        Some(mut bans) => bans.banned = banned,
        None => {
            commands.entity(trigger.room).try_insert(SceneBans {
                banned,
                dropped_chats: 0,
            });
        }
    }
}

/// `/scene_bans` prints who the current scene room bans, as JSON.
#[derive(clap::Parser, ConsoleCommand)]
#[command(name = "/scene_bans")]
struct SceneBansCommand;

fn scene_bans(
    mut input: ConsoleCommand<SceneBansCommand>,
    scene_rooms: Query<(&SceneRoom, &SceneBans)>,
) {
    if input.take().is_none() {
        return;
    }
    let room = scene_rooms.iter().next();
    let mut banned: Vec<_> = room
        .iter()
        .flat_map(|(_, bans)| bans.banned.iter().map(|a| format!("{a:#x}")))
        .collect();
    banned.sort();
    input.reply_ok(
        json!({
            "sceneRoom": room.map(|(scene, _)| &scene.0),
            "banned": banned,
            "droppedChats": room.map_or(0, |(_, bans)| bans.dropped_chats),
        })
        .to_string(),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    const A: &str = "0x00000000000000000000000000000000000000aa";
    const B: &str = "0x00000000000000000000000000000000000000bb";

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
        assert!(bans.contains(&address(B)));
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
        app.add_plugins(SceneBansPlugin);
        app
    }

    fn metadata(app: &mut App, room: Entity, banned: &[&str]) {
        app.world_mut().trigger(RoomMetadataChanged {
            room,
            metadata: json!({ "bannedAddresses": banned }).to_string(),
        });
        app.world_mut().flush();
    }

    fn banned(app: &App, room: Entity, who: &str) -> bool {
        app.world()
            .get::<SceneBans>(room)
            .is_some_and(|bans| bans.is_banned(address(who)))
    }

    #[test]
    fn only_a_scene_room_takes_the_list() {
        let mut app = app();
        let other_room = app.world_mut().spawn_empty().id();
        let scene_room = app.world_mut().spawn(SceneRoom("scene".into())).id();

        metadata(&mut app, other_room, &[A]);
        metadata(&mut app, scene_room, &[A]);

        assert!(app.world().get::<SceneBans>(other_room).is_none());
        assert!(banned(&app, scene_room, A));
    }

    #[test]
    fn a_rewrite_replaces_the_list_and_keeps_the_count() {
        let mut app = app();
        let scene_room = app.world_mut().spawn(SceneRoom("scene".into())).id();
        metadata(&mut app, scene_room, &[A]);
        assert!(app
            .world_mut()
            .get_mut::<SceneBans>(scene_room)
            .unwrap()
            .drops_chat_from(address(A)));

        metadata(&mut app, scene_room, &[B]);

        assert!(!banned(&app, scene_room, A));
        assert!(banned(&app, scene_room, B));
        assert_eq!(
            app.world()
                .get::<SceneBans>(scene_room)
                .unwrap()
                .dropped_chats,
            1
        );
    }
}
