//! The lobby stage's debug section and `/lobby_stage` console command.

use bevy::prelude::*;
use bevy_console::ConsoleCommand;
use console::{AddDebugSection, DebugSection, DoAddConsoleCommand};

use super::{Stage, StageRequest};

pub(super) fn register(app: &mut App) {
    app.add_console_command::<LobbyStageCommand, _>(lobby_stage_command)
        .add_debug_section(DebugSection {
            title: "Lobby stage",
            query: "/lobby_stage",
            fields: &[
                ("Requested look", "requested"),
                ("Shown look", "shown"),
                ("Camera layer (scene / render)", "layers"),
                ("Camera found", "camera"),
            ],
            actions: &[],
            inputs: &[],
        });
}

/// Read what the lobby stage was asked for and what it shows.
#[derive(clap::Parser, ConsoleCommand)]
#[command(name = "/lobby_stage")]
struct LobbyStageCommand;

fn lobby_stage_command(
    mut cmd: ConsoleCommand<LobbyStageCommand>,
    request: Res<StageRequest>,
    stages: Query<&Stage>,
) {
    let Some(Ok(_)) = cmd.take() else {
        return;
    };
    let stage = stages.iter().next();
    cmd.reply_ok(
        serde_json::json!({
            "requested": request.0.as_ref().map(|r| r.preset.clone()),
            "shown": stage.map(|s| s.preset.id),
            "layers": stage.map(|s| [s.layer as usize, s.render_layer]),
            "camera": stage.map(|s| s.camera.is_some()),
            "unavailable": {
                "shown": "no stage: the lobby is closed or the system scene's camera is not up yet",
                "layers": "no stage",
                "camera": "no stage",
            },
        })
        .to_string(),
    );
}
