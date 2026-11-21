//! The gestures the automation commands run over several frames (`/walk` holds keys until the
//! player settles). A command claims its gesture before it acts and releases it with its reply, so
//! a second call is refused naming the holder instead of queueing behind it. `/gesture_state` and
//! the debug panel's section read what is running and what it is doing.

use std::collections::BTreeMap;

use bevy::prelude::*;
use bevy_console::ConsoleCommand;
use console::{reply::refusal, AddDebugSection, DebugSection, DoAddConsoleCommand};
use serde_json::json;
use user_input::avatar_movement::InputModifierBypass;

pub(crate) struct AutomationGesturePlugin;

impl Plugin for AutomationGesturePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<GestureArbiter>()
            .add_console_command::<GestureStateCommand, _>(gesture_state_cmd)
            .add_debug_section(DebugSection {
                title: "Automation gestures",
                query: "/gesture_state",
                fields: &[
                    ("Walk", "running.walk"),
                    (
                        "InputModifier withheld from the movement scene",
                        "inputModifierWithheld",
                    ),
                ],
                actions: &[],
                inputs: &[],
            });
    }
}

/// The running gestures by command name, each with what it is doing now.
#[derive(Resource, Default)]
pub(crate) struct GestureArbiter {
    running: BTreeMap<&'static str, String>,
}

impl GestureArbiter {
    /// The refusal for a `gesture` that is still running, naming it as the holder.
    pub(crate) fn refuse_running(&self, gesture: &'static str) -> Result<(), String> {
        if self.running.contains_key(gesture) {
            return Err(refusal(
                format!("a {gesture} is still running; wait for its reply"),
                Some(json!({ "holder": gesture })),
            ));
        }
        Ok(())
    }

    /// Marks `gesture` running, doing `status`, or says what a running one does now.
    pub(crate) fn report(&mut self, gesture: &'static str, status: String) {
        self.running.insert(gesture, status);
    }

    pub(crate) fn release(&mut self, gesture: &'static str) {
        self.running.remove(gesture);
    }
}

/// Read the automation gestures that are running, as JSON: running (each gesture's command name
/// with what it is doing now) and inputModifierWithheld
#[derive(clap::Parser, ConsoleCommand)]
#[command(name = "/gesture_state")]
struct GestureStateCommand;

fn gesture_state_cmd(
    mut input: ConsoleCommand<GestureStateCommand>,
    arbiter: Res<GestureArbiter>,
    bypass: Res<InputModifierBypass>,
) {
    let Some(Ok(_)) = input.take() else {
        return;
    };
    input.reply_ok(
        json!({
            "running": arbiter.running,
            "inputModifierWithheld": bypass.0,
            "unavailable": { "running.walk": "No walk is running." },
        })
        .to_string(),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_running_gesture_refuses_a_second_call_naming_the_holder() {
        let mut arbiter = GestureArbiter::default();
        assert_eq!(arbiter.refuse_running("walk"), Ok(()));
        arbiter.report("walk", "pressing KeyW".to_owned());
        arbiter.report("walk", "holding KeyW".to_owned());
        assert_eq!(arbiter.running["walk"], "holding KeyW");
        let refused: serde_json::Value =
            serde_json::from_str(&arbiter.refuse_running("walk").unwrap_err()).unwrap();
        assert_eq!(refused["cause"]["holder"], "walk");
        arbiter.release("walk");
        assert_eq!(arbiter.refuse_running("walk"), Ok(()));
    }
}
