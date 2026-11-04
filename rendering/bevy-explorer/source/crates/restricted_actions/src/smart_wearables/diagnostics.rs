//! Console readouts and the runtime debug panel section for smart wearables.

use bevy::prelude::*;
use bevy_console::ConsoleCommand;
use common::structs::PrimaryUser;
use console::{AddDebugSection, DebugInput, DebugInputKind, DebugSection, DoAddConsoleCommand};
use scene_runner::{renderer_context::RendererSceneContext, ContainingScene};
use serde_json::json;
use system_bridge::SmartWearableStateInfo;

use super::{current_toggle, SmartWearables};

pub(super) fn register(app: &mut App) {
    app.add_console_command::<SmartWearablesCommand, _>(smart_wearables)
        .add_console_command::<SmartWearablesStateCommand, _>(smart_wearables_state)
        .add_debug_section(DebugSection {
            title: "Smart wearables",
            query: "/smart_wearables_state",
            fields: &[
                (
                    "Current scene allows portable experiences",
                    "currentSceneAllows",
                ),
                ("Current scene hides their UI", "currentSceneHidesUi"),
                ("Body shape folder", "bodyShapePrefix"),
                ("Login wearables still loading", "loginPending"),
                ("Realm preview wearables", "previewCount"),
                ("HUD listening for authorizations", "authorizationListener"),
                ("Open authorization request", "pendingAuthorization"),
                ("Entries (urn / state / reason)", "entries"),
            ],
            actions: &[],
            inputs: &[
                DebugInput {
                    label: "Run without equipping (urn)",
                    field: "",
                    command: "/smart_wearable run {value}",
                    kind: DebugInputKind::Text,
                },
                DebugInput {
                    label: "Kill (urn)",
                    field: "",
                    command: "/smart_wearable kill {value}",
                    kind: DebugInputKind::Text,
                },
                DebugInput {
                    label: "Restart (urn)",
                    field: "",
                    command: "/smart_wearable restart {value}",
                    kind: DebugInputKind::Text,
                },
            ],
        });
}

/// List the smart wearables and their states.
#[derive(clap::Parser, ConsoleCommand)]
#[command(name = "/smart_wearables")]
struct SmartWearablesCommand;

/// The smart wearables state as JSON, for the debug panel.
#[derive(clap::Parser, ConsoleCommand)]
#[command(name = "/smart_wearables_state")]
struct SmartWearablesStateCommand;

/// `current scene allows: <bool>` then `<urn> <state> [<reason>]` per entry.
pub fn format_listing(info: &SmartWearableStateInfo) -> String {
    if info.entries.is_empty() {
        return "no smart wearables".to_owned();
    }
    let mut lines = vec![format!(
        "current scene allows: {}",
        info.current_scene_allows
    )];
    for entry in info.entries.iter() {
        let state = serde_json::to_value(entry.state)
            .ok()
            .and_then(|v| v.as_str().map(ToOwned::to_owned))
            .unwrap_or_default();
        lines.push(match &entry.reason {
            Some(reason) => format!("{} {state} {reason}", entry.urn),
            None => format!("{} {state}", entry.urn),
        });
    }
    lines.join("\n")
}

fn smart_wearables(
    mut input: ConsoleCommand<SmartWearablesCommand>,
    sw: Res<SmartWearables>,
    player: Query<Entity, With<PrimaryUser>>,
    containing_scene: ContainingScene,
    contexts: Query<&RendererSceneContext>,
) {
    let Some(Ok(_)) = input.take() else {
        return;
    };
    let info = sw
        .model
        .info(current_toggle(&player, &containing_scene, &contexts));
    input.reply_ok(format_listing(&info));
}

fn smart_wearables_state(
    mut input: ConsoleCommand<SmartWearablesStateCommand>,
    sw: Res<SmartWearables>,
    player: Query<Entity, With<PrimaryUser>>,
    containing_scene: ContainingScene,
    contexts: Query<&RendererSceneContext>,
) {
    let Some(Ok(_)) = input.take() else {
        return;
    };
    let info = sw
        .model
        .info(current_toggle(&player, &containing_scene, &contexts));
    let entries: Vec<String> = info
        .entries
        .iter()
        .map(|entry| {
            let state = serde_json::to_value(entry.state).unwrap_or_default();
            let state = state.as_str().unwrap_or_default();
            match &entry.reason {
                Some(reason) => format!("{} / {state} / {reason}", entry.urn),
                None => format!("{} / {state}", entry.urn),
            }
        })
        .collect();
    let pending = sw
        .model
        .pending_request()
        .map(|request| format!("{} {:?}", request.urn, request.permissions));
    input.reply_ok(
        json!({
            "currentSceneAllows": info.current_scene_allows,
            "currentSceneHidesUi": info.current_scene_hides_ui,
            "bodyShapePrefix": sw.model.equipped.as_ref().map(|e| e.body_shape_prefix.clone()),
            "loginPending": sw.model.login_pending(),
            "previewCount": sw.model.preview_count(),
            "authorizationListener": sw.has_auth_listener(),
            "pendingAuthorization": pending,
            "entries": if entries.is_empty() { None } else { Some(entries) },
            "unavailable": {
                "bodyShapePrefix": "No profile: the player is logged out.",
                "pendingAuthorization": "No smart wearable is waiting for the player's answer.",
                "entries": "No smart wearable is equipped, previewed by the realm or run from the console.",
            },
        })
        .to_string(),
    );
}
