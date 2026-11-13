//! The instantiation slice in the runtime debug panel: the slice in effect, how long the scene
//! spawner and glTF processing take, what bevy's scene spawner has queued and is writing, and how
//! many glTF models are spawning or being processed.

use std::time::Duration;

use bevy::{
    diagnostic::{DiagnosticPath, DiagnosticsStore},
    prelude::*,
    scene::SceneSpawner,
};
use bevy_console::ConsoleCommand;
use console::{AddDebugSection, DebugSection, DoAddConsoleCommand};
use serde_json::json;

use super::{
    processing_share, slice,
    spawn_timing::{GLTF_PROCESSING, SCENE_SPAWNER},
    ProcessingDeadline,
};
use crate::{update_world::gltf_container::GltfProcessing, SceneUpdates};

pub(super) fn add_debug_section(app: &mut App) {
    app.add_console_command::<InstantiationState, _>(instantiation_state)
        .add_debug_section(DebugSection {
            title: "Instantiation",
            query: "/instantiation_state",
            fields: &[
                ("Slice per frame (ms)", "slice"),
                ("Kept for glTF processing (ms)", "processingShare"),
                ("Scene spawner per frame, smoothed (ms)", "spawner"),
                ("glTF processing per frame, smoothed (ms)", "processing"),
                ("Scene instances queued", "queued"),
                (
                    "Instance being written (entities written / in all)",
                    "writing",
                ),
                ("glTF models spawning or being processed", "models"),
            ],
            actions: &[],
            inputs: &[],
        });
}

#[derive(clap::Parser, ConsoleCommand)]
#[command(name = "/instantiation_state")]
struct InstantiationState;

fn milliseconds(duration: Duration) -> f64 {
    duration.as_secs_f64() * 1000.0
}

fn instantiation_state(
    mut input: ConsoleCommand<InstantiationState>,
    spawner: Res<SceneSpawner>,
    deadline: Res<ProcessingDeadline>,
    updates: Res<SceneUpdates>,
    store: Res<DiagnosticsStore>,
    models: Query<(), With<GltfProcessing>>,
) {
    if input.take().is_none() {
        return;
    }
    let progress = spawner.spawn_progress();
    let slice = deadline.0.map(|_| slice(updates.frame_target));
    let smoothed =
        |path: &DiagnosticPath| store.get(path).and_then(|diagnostic| diagnostic.smoothed());
    let whole = "None: behind the loading backdrop and on headless, instantiation runs whole.";
    input.reply_ok(
        json!({
            "slice": slice.map(milliseconds),
            "processingShare": slice.map(|slice| milliseconds(processing_share(slice))),
            "spawner": smoothed(&SCENE_SPAWNER),
            "processing": smoothed(&GLTF_PROCESSING),
            "queued": progress.queued,
            "writing": progress.writing.map(|(written, total)| [written, total]),
            "models": models.iter().count(),
            "unavailable": {
                "slice": whole,
                "processingShare": whole,
                "spawner": "Nothing measured yet.",
                "processing": "Nothing measured yet.",
                "writing": "No scene instance is part-written.",
            },
        })
        .to_string(),
    );
}
