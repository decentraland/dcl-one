//! The spring bones' debug section and `/spring_bones` console command.

use bevy::prelude::*;
use bevy_console::ConsoleCommand;
use common::structs::{AppConfig, PrimaryUser};
use console::{AddDebugSection, DebugInput, DebugInputKind, DebugSection, DoAddConsoleCommand};

use super::{SpringChains, SpringSimulation, MAX_SIMULATED_AVATARS};
use crate::AvatarProcessed;

pub(super) struct SpringBonesDiagnosticsPlugin;

impl Plugin for SpringBonesDiagnosticsPlugin {
    fn build(&self, app: &mut App) {
        app.add_console_command::<SpringBonesCommand, _>(spring_bones_command)
            .add_debug_section(DebugSection {
                title: "Spring bones",
                query: "/spring_bones",
                fields: &[
                    ("Simulated avatars", "simulated_avatars"),
                    ("Most simulated at once", "max_simulated_avatars"),
                    ("Simulated chains", "simulated_chains"),
                    ("Player simulated", "player_simulated"),
                    ("Player chains", "chains"),
                    ("Player spring bones", "joints"),
                    ("Longest chain (bones)", "longest_chain"),
                ],
                actions: &[],
                inputs: &[DebugInput {
                    label: "Jiggle Bones setting",
                    field: "enabled",
                    command: "/spring_bones {value}",
                    kind: DebugInputKind::Toggle,
                }],
            });
    }
}

/// Read the spring bones' state, or turn the Jiggle Bones setting on or off.
#[derive(clap::Parser, ConsoleCommand)]
#[command(name = "/spring_bones")]
struct SpringBonesCommand {
    enabled: Option<bool>,
}

fn spring_bones_command(
    mut cmd: ConsoleCommand<SpringBonesCommand>,
    player: Query<Entity, With<PrimaryUser>>,
    avatars: Query<(&SpringChains, &ChildOf, Has<SpringSimulation>), With<AvatarProcessed>>,
    simulations: Query<&SpringSimulation>,
    mut config: ResMut<AppConfig>,
) {
    let Some(Ok(request)) = cmd.take() else {
        return;
    };
    if let Some(value) = request.enabled {
        // the user setting itself, so it is saved and the settings panel shows it
        if config.graphics.spring_bones != value {
            config.graphics.spring_bones = value;
        }
    }
    let player = player.single().ok();
    let avatar = avatars
        .iter()
        .find(|(_, parent, _)| Some(parent.parent()) == player);
    let chains = avatar.map(|(chains, ..)| &chains.0);
    let unavailable = if player.is_none() {
        "no primary player"
    } else {
        "the player's avatar is not built yet"
    };
    cmd.reply_ok(
        serde_json::json!({
            "enabled": config.graphics.spring_bones,
            "simulated_avatars": simulations.iter().count(),
            "max_simulated_avatars": MAX_SIMULATED_AVATARS,
            "simulated_chains": simulations
                .iter()
                .map(SpringSimulation::simulated_chains)
                .sum::<usize>(),
            "player_simulated": avatar.map(|(.., simulated)| simulated),
            "chains": chains.map(|chains| chains.len()),
            "joints": chains.map(|chains| {
                chains.iter().map(|chain| chain.joints.len()).sum::<usize>()
            }),
            "longest_chain": chains.map(|chains| {
                chains.iter().map(|chain| chain.joints.len()).max().unwrap_or(0)
            }),
            "unavailable": { "": unavailable },
        })
        .to_string(),
    );
}
