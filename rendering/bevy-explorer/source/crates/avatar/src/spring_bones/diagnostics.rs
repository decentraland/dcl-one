//! The spring bones' debug section and `/spring_bones` console command.

use bevy::prelude::*;
use bevy_console::ConsoleCommand;
use common::structs::PrimaryUser;
use console::{AddDebugSection, DebugSection, DoAddConsoleCommand};

use super::SpringChains;
use crate::AvatarProcessed;

pub(super) struct SpringBonesDiagnosticsPlugin;

impl Plugin for SpringBonesDiagnosticsPlugin {
    fn build(&self, app: &mut App) {
        app.add_console_command::<SpringBonesCommand, _>(spring_bones_command)
            .add_debug_section(DebugSection {
                title: "Spring bones",
                query: "/spring_bones",
                fields: &[
                    ("Player chains", "chains"),
                    ("Player spring bones", "joints"),
                    ("Longest chain (bones)", "longest_chain"),
                ],
                actions: &[],
                inputs: &[],
            });
    }
}

/// Read the spring chains of the player's avatar.
#[derive(clap::Parser, ConsoleCommand)]
#[command(name = "/spring_bones")]
struct SpringBonesCommand;

fn spring_bones_command(
    mut cmd: ConsoleCommand<SpringBonesCommand>,
    player: Query<Entity, With<PrimaryUser>>,
    avatars: Query<(&SpringChains, &ChildOf), With<AvatarProcessed>>,
) {
    let Some(Ok(_)) = cmd.take() else {
        return;
    };
    let player = player.single().ok();
    let chains = avatars
        .iter()
        .find(|(_, parent)| Some(parent.parent()) == player)
        .map(|(chains, _)| &chains.0);
    let unavailable = if player.is_none() {
        "no primary player"
    } else {
        "the player's avatar is not built yet"
    };
    cmd.reply_ok(
        serde_json::json!({
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
