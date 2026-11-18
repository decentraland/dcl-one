use bevy::{
    core_pipeline::{bloom::Bloom, tonemapping::Tonemapping},
    prelude::*,
};
use bevy_console::ConsoleCommand;
use common::structs::StageCamera;
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
                (
                    "Camera unprocessed (no tonemapping or bloom)",
                    "unprocessed",
                ),
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
    requests: Query<&StageRequest>,
    stages: Query<&Stage>,
    cameras: Query<(Has<StageCamera>, Has<Bloom>, Option<&Tonemapping>)>,
) {
    let Some(Ok(_)) = cmd.take() else {
        return;
    };
    let stage = stages.iter().next();
    cmd.reply_ok(
        serde_json::json!({
            "requested": requests.iter().next().map(|r| r.0.preset.clone()),
            "shown": stage.map(|s| s.preset.id),
            "layers": stage.map(|s| [s.layer as usize, s.render_layer]),
            "camera": stage.map(|s| s.camera.is_some()),
            "unprocessed": stage
                .and_then(|s| s.camera)
                .and_then(|c| cameras.get(c).ok())
                .map(|(marked, bloom, tonemapping)| {
                    marked && !bloom && tonemapping == Some(&Tonemapping::None)
                }),
            "unavailable": {
                "shown": "no stage: the lobby is closed or the system scene's camera is not up yet",
                "layers": "no stage",
                "camera": "no stage",
                "unprocessed": "no stage, or its camera is not up yet",
            },
        })
        .to_string(),
    );
}
