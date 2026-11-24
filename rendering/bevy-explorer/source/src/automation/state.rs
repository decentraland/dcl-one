//! `/player_state`: the one read of the player and of what the automation commands are doing to
//! it. The pose comes from `user_input`'s [`PlayerPose`]; `gestures` holds the running `/walk`,
//! read from its job. The debug panel's "Automation gestures" section reads `gestures` from the
//! same reply.

use bevy::prelude::*;
use bevy_console::ConsoleCommand;
use common::structs::EngineMovementControl;
use console::{AddDebugSection, DebugSection, DoAddConsoleCommand};
use serde_json::{json, Value};
use user_input::PlayerPose;

use super::walk::{gesture_state, WalkJob};

pub(crate) struct AutomationStatePlugin;

impl Plugin for AutomationStatePlugin {
    fn build(&self, app: &mut App) {
        app.add_console_command::<PlayerStateCommand, _>(player_state_cmd)
            .add_debug_section(DebugSection {
                title: "Automation gestures",
                query: "/player_state",
                fields: &[
                    ("Walk", "gestures.running.walk"),
                    (
                        "InputModifier withheld from the movement info",
                        "gestures.inputModifierWithheld",
                    ),
                ],
                actions: &[],
                inputs: &[],
            });
    }
}

/// Read the player's and the camera's pose and the running automation gestures as JSON: position,
/// rotationEuler, parcel, velocity, grounded (null while out of the world), outOfWorld,
/// pointerLocked, camera {mode, position, rotationEuler, fov, modeChangeAllowed, pointerLocked}
/// and gestures {running: {walk: its stage and its keys, queued while pressing, down while
/// holding, going up while releasing; absent when no walk runs}, inputModifierWithheld: the walk
/// keeps the InputModifier out of the avatar movement info}. Positions and the velocity are in
/// Decentraland world coordinates; euler angles are Decentraland's, in degrees from 0 to 360 (x
/// the pitch, looking down positive; y the yaw clockwise from north; z the roll).
/// /player_position, /camera_state and /movement_state read one part each
#[derive(clap::Parser, ConsoleCommand)]
#[command(name = "/player_state")]
struct PlayerStateCommand;

fn player_state_cmd(
    mut input: ConsoleCommand<PlayerStateCommand>,
    pose: PlayerPose,
    job: Res<WalkJob>,
    movement_control: Res<EngineMovementControl>,
) {
    let Some(Ok(_)) = input.take() else {
        return;
    };
    match player_state(&pose, &job, &movement_control) {
        Some(state) => input.reply_ok(state.to_string()),
        None => input.reply_failed("no player"),
    }
}

/// The pose with the gestures beside it, or `None` when there is no player.
pub(super) fn player_state(
    pose: &PlayerPose,
    job: &WalkJob,
    movement_control: &EngineMovementControl,
) -> Option<Value> {
    let mut state = pose.read()?;
    state["gestures"] = gesture_state(job, movement_control);
    state["unavailable"] = json!({ "gestures.running.walk": "No walk is running." });
    Some(state)
}
