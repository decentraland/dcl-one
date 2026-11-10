//! The instantiation slice in the runtime debug panel: the slice in effect, what bevy's scene
//! spawner has queued and is writing, and how many glTF models are being processed.

use bevy::{prelude::*, scene::SceneSpawner};
use bevy_console::ConsoleCommand;
use console::{AddDebugSection, DebugSection, DoAddConsoleCommand};
use serde_json::json;

use super::{ProcessingDeadline, PROCESSING_SHARE, SCENE_SPAWN_SLICE};
use crate::update_world::gltf_container::GltfProcessing;

pub(crate) fn register(app: &mut App) {
    app.add_console_command::<InstantiationState, _>(instantiation_state)
        .add_debug_section(DebugSection {
            title: "Instantiation",
            query: "/instantiation_state",
            fields: &[
                ("Slice per frame (ms)", "slice"),
                ("Kept for glTF processing (ms)", "processingShare"),
                ("Scene instances queued", "queued"),
                (
                    "Instance being written (entities written / in all)",
                    "writing",
                ),
                ("glTF models being processed", "processing"),
            ],
            actions: &[],
            inputs: &[],
        });
}

#[derive(clap::Parser, ConsoleCommand)]
#[command(name = "/instantiation_state")]
struct InstantiationState;

fn instantiation_state(
    mut input: ConsoleCommand<InstantiationState>,
    spawner: Res<SceneSpawner>,
    deadline: Res<ProcessingDeadline>,
    processing: Query<(), With<GltfProcessing>>,
) {
    if input.take().is_none() {
        return;
    }
    let progress = spawner.spawn_progress();
    let sliced = deadline.0.is_some();
    let whole = "None: behind the loading backdrop and on headless, instantiation runs whole.";
    input.reply_ok(
        json!({
            "slice": sliced.then(|| SCENE_SPAWN_SLICE.as_secs_f64() * 1000.0),
            "processingShare": sliced.then(|| PROCESSING_SHARE.as_secs_f64() * 1000.0),
            "queued": progress.queued,
            "writing": progress.writing.map(|(written, total)| [written, total]),
            "processing": processing.iter().count(),
            "unavailable": {
                "slice": whole,
                "processingShare": whole,
                "writing": "No scene instance is part-written.",
            },
        })
        .to_string(),
    );
}
