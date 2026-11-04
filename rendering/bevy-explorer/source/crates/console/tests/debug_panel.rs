use std::sync::{Arc, Mutex};

use bevy::prelude::*;
use bevy_console::ConsoleCommandEntered;
use common::structs::PreviewMode;
use console::{AddDebugSection, ConsolePlugin, DebugInput, DebugInputKind, DebugSection};
use serde_json::{json, Value};

#[test]
fn command_lists_plugins_registered_before_and_after_console_setup() {
    let mut app = App::new();
    app.add_debug_section(DebugSection {
        title: "A",
        inputs: &[],
        query: "/state_a",
        fields: &[("Live", "mic.enabled")],
        actions: &[],
    })
    .add_plugins(ConsolePlugin {
        add_bevy_console: false,
    })
    .add_debug_section(DebugSection {
        title: "B",
        inputs: &[],
        query: "/state_b",
        fields: &[],
        actions: &[("Act", "/act b")],
    });
    assert_eq!(
        manifest(&mut app),
        json!([
            { "title": "A", "query": "/state_a", "fields": [["Live", "mic.enabled"]], "actions": [], "inputs": [] },
            { "title": "B", "query": "/state_b", "fields": [], "actions": [["Act", "/act b"]], "inputs": [] }
        ])
    );
}

#[test]
fn preview_controls_become_readouts_in_retail() {
    for preview in [true, false] {
        let mut app = App::new();
        app.insert_resource(PreviewMode {
            is_preview: preview,
            ..default()
        })
        .add_plugins(ConsolePlugin {
            add_bevy_console: false,
        })
        .add_preview_debug_inputs(DebugSection {
            title: "Physics",
            query: "/physics",
            fields: &[],
            actions: &[("Reset", "/reset")],
            inputs: &[DebugInput {
                label: "Visible",
                field: "visible",
                command: "/show {value}",
                kind: DebugInputKind::Toggle,
            }],
        });
        let section = &manifest(&mut app)[0];
        if preview {
            assert_eq!(section["inputs"][0]["kind"], json!({ "type": "toggle" }));
            assert_eq!(section["actions"].as_array().unwrap().len(), 1);
        } else {
            assert_eq!(section["fields"], json!([["Visible", "visible"]]));
            assert_eq!(section["actions"], json!([]));
            assert_eq!(section["inputs"], json!([]));
        }
    }
}

fn manifest(app: &mut App) -> Value {
    app.update();
    let reply = Arc::new(Mutex::new(None));
    let output = reply.clone();
    app.world_mut().send_event(ConsoleCommandEntered {
        command_name: "/debug_panel".into(),
        args: vec![],
        responder: Some(Arc::new(move |result| {
            *output.lock().unwrap() = Some(result)
        })),
    });
    app.update();
    let text = reply.lock().unwrap().take().unwrap().unwrap();
    serde_json::from_str(&text).unwrap()
}
