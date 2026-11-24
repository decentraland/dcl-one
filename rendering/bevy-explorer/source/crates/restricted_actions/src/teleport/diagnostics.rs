//! The teleport's debug section and its `/teleport_state` console command.

use bevy::prelude::*;
use bevy_console::ConsoleCommand;
use common::structs::PrimaryUser;
use console::{AddDebugSection, DebugSection, DoAddConsoleCommand};
use ipfs::RealmInitialLocation;
use scene_runner::OutOfWorld;

use super::LastNamedLanding;

pub(crate) struct TeleportDiagnosticsPlugin;

impl Plugin for TeleportDiagnosticsPlugin {
    fn build(&self, app: &mut App) {
        app.add_console_command::<TeleportState, _>(teleport_state)
            .add_debug_section(DebugSection {
                title: "Teleport",
                query: "/teleport_state",
                fields: &[
                    ("Spawn point the landing under way looks for", "pending"),
                    ("Spawn point the last landing looked for", "last.name"),
                    ("The landing scene had it", "last.found"),
                ],
                actions: &[],
                inputs: &[],
            });
    }
}

/// The spawn point a landing names, for the debug panel's Teleport section.
#[derive(clap::Parser, ConsoleCommand)]
#[command(name = "/teleport_state")]
struct TeleportState;

fn teleport_state(
    mut input: ConsoleCommand<TeleportState>,
    player: Query<(Option<&OutOfWorld>, Option<&LastNamedLanding>), With<PrimaryUser>>,
    realm_target: Res<RealmInitialLocation>,
) {
    if input.take().is_none() {
        return;
    }
    let (out_of_world, last) = player.single().unwrap_or_default();
    // of the landing under way, else of the realm change that has yet to start one
    let pending = out_of_world
        .and_then(|out_of_world| out_of_world.spawn_point.clone())
        .or_else(|| match &*realm_target {
            RealmInitialLocation::Base { spawn_point }
            | RealmInitialLocation::Parcel { spawn_point, .. } => spawn_point.clone(),
            RealmInitialLocation::None => None,
        });
    input.reply_ok(
        serde_json::json!({
            "pending": pending,
            "last": last.map(|last| {
                serde_json::json!({ "name": last.name, "found": last.found })
            }),
            "unavailable": {
                "pending": "No landing under way names a spawn point.",
                "last": "No landing has looked for a named spawn point.",
            },
        })
        .to_string(),
    );
}

#[cfg(test)]
mod tests {
    use std::sync::{mpsc, Arc};

    use bevy_console::{ConsoleCommandEntered, PrintConsoleLine};
    use serde_json::{json, Value};

    use super::*;

    /// What `/teleport_state` answers for a player with these components and this landing
    /// target of a realm change.
    fn state(player: impl Bundle, realm_target: RealmInitialLocation) -> Value {
        let mut app = App::new();
        app.add_event::<ConsoleCommandEntered>()
            .add_event::<PrintConsoleLine>()
            .insert_resource(realm_target)
            .add_systems(Update, teleport_state);
        app.world_mut().spawn((PrimaryUser::default(), player));
        let (tx, rx) = mpsc::channel();
        app.world_mut().send_event(ConsoleCommandEntered {
            command_name: "/teleport_state".into(),
            args: vec![],
            responder: Some(Arc::new(move |result| tx.send(result).unwrap())),
        });
        app.update();
        serde_json::from_str(&rx.try_recv().unwrap().unwrap()).unwrap()
    }

    #[test]
    fn a_landing_under_way_shows_the_spawn_point_it_looks_for() {
        let report = state(
            OutOfWorld {
                spawn_point: Some("roof".to_owned()),
            },
            RealmInitialLocation::None,
        );
        assert_eq!(report["pending"], json!("roof"));
        assert!(report["last"].is_null());
    }

    #[test]
    fn a_realm_change_yet_to_land_shows_the_spawn_point_it_will_look_for() {
        let report = state(
            (),
            RealmInitialLocation::Parcel {
                parcel: IVec2::new(3, 4),
                spawn_point: Some("door".to_owned()),
            },
        );
        assert_eq!(report["pending"], json!("door"));
    }

    #[test]
    fn the_last_named_landing_stays_with_whether_its_scene_had_the_spawn_point() {
        let report = state(
            LastNamedLanding {
                name: "cellar".to_owned(),
                found: false,
            },
            RealmInitialLocation::None,
        );
        assert!(report["pending"].is_null());
        assert_eq!(report["last"], json!({ "name": "cellar", "found": false }));
    }
}
