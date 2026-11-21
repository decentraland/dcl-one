//! `/walk`: hold the movement keys for a time, camera-relative, as a person would, and answer with
//! where the player went. The keys are the ones bound to the movement actions, pressed through the
//! input script, so the movement scene reads them as it reads the keyboard: the gait is the walk
//! (`IaWalk`) or run (`IaModifier`) binding held with them, jog neither. `--ignore-input-modifiers`
//! keeps the active `InputModifier` from the movement scene for the walk (`InputModifierBypass`).
//! `drive_walk` follows the walk frame by frame and answers once the player has settled:
//!
//! `{"startPosition", "endPosition": {"x", "y", "z"}, "distance", "parcel": [x, y], "seconds",
//! "kind", "jump", "keys", "modifierActive", "modifier", "ignoreInputModifiers", "settled"}`
//!
//! A walk that cannot start (one is running, the HUD holds a key, a gamepad binding, no player)
//! is refused before a key is pressed: `{"refused", "cause"}`.

use std::collections::BTreeSet;

use bevy::{
    diagnostic::{DiagnosticsStore, FrameCount},
    input::ButtonState,
    prelude::*,
};
use bevy_console::ConsoleCommand;
use common::{
    inputs::{CommonInputAction, InputIdentifier, InputMap},
    rpc::{RpcResultReceiver, RpcResultSender},
    sets::SceneSets,
    structs::PrimaryUser,
};
use console::{
    reply::{self, refusal, FrameRate},
    DoAddConsoleCommand, PendingConsoleResponses,
};
use dcl_component::{
    proto_components::sdk::components::{pb_input_modifier::Mode, PbInputModifier},
    transform_and_parent::DclTranslation,
};
use input_manager::{InputManager, InputPriority};
use scene_runner::{update_world::avatar_modifier_area::InputModifier, vec3_to_parcel};
use serde::Serialize;
use serde_json::{json, Value};
use user_input::avatar_movement::{ActivePlayerComponent, InputModifierBypass};

use super::{
    gesture::GestureArbiter,
    input::InputScript,
    pointer::{binding_error, binding_step},
};

/// The gesture's name with the arbiter.
const WALK: &str = "walk";

/// How long the jump key stays down at most: long enough for a full jump, short of a second one.
const JUMP_HOLD_SECONDS: f32 = 0.5;
/// How far the player may move in a frame and count as still.
const STILL_METRES: f32 = 0.005;
/// Frames in a row the player must be still to have settled.
const STILL_FRAMES: u32 = 3;
/// How long the player may take to settle after the keys are up.
const SETTLE_SECONDS: f32 = 3.0;
/// The longest walk.
const MAX_SECONDS: f32 = 30.0;
/// How far from zero a direction component must be to press its key.
const AXIS_THRESHOLD: f32 = 0.3;

pub(crate) struct AutomationWalkPlugin;

impl Plugin for AutomationWalkPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<WalkJob>()
            .add_console_command::<WalkCommand, _>(walk_cmd)
            .add_systems(Update, drive_walk.in_set(SceneSets::PostLoop));
    }
}

/// Walk the player camera-relative for --seconds (1 by default) by holding the movement keys, as
/// the keyboard does: dx is right (+) or left (-), dy forward (+) or backward (-), each from -1 to
/// 1, a component past 0.3 pressing its key. --kind walk holds the walk key with them, run the run
/// key, jog neither; --jump presses jump as the walk starts (held at most 0.5 s);
/// --ignore-input-modifiers keeps the scene's InputModifier from the movement scene for the walk.
/// Answers, as JSON, once the player has settled after the keys are up: startPosition,
/// endPosition, distance, parcel, the seconds the keys were held, the keys, and whether an
/// InputModifier was active (modifierActive, modifier: the disabled gaits)
#[derive(clap::Parser, ConsoleCommand)]
#[command(name = "/walk")]
pub struct WalkCommand {
    /// Right (+) or left (-), -1 to 1
    #[arg(allow_negative_numbers = true)]
    dx: f32,
    /// Forward (+) or backward (-), -1 to 1
    #[arg(allow_negative_numbers = true)]
    dy: f32,
    /// How long to hold the keys, at most 30
    #[arg(long, default_value_t = 1.0)]
    seconds: f32,
    /// walk, jog or run
    #[arg(long, default_value = "jog")]
    kind: String,
    /// Press jump as the walk starts
    #[arg(long)]
    jump: bool,
    /// Keep the active InputModifier from the movement scene for the walk
    #[arg(long)]
    ignore_input_modifiers: bool,
}

/// The movement actions `dx`, `dy` press: right or left, then forward or backward.
pub fn direction_actions(dx: f32, dy: f32) -> Result<Vec<CommonInputAction>, String> {
    let in_range = |v: f32| v.is_finite() && (-1.0..=1.0).contains(&v);
    if !in_range(dx) || !in_range(dy) {
        return Err(format!(
            "dx and dy take numbers from -1 to 1; got {dx} {dy}"
        ));
    }
    let mut actions = Vec::new();
    if dx >= AXIS_THRESHOLD {
        actions.push(CommonInputAction::IaRight);
    } else if dx <= -AXIS_THRESHOLD {
        actions.push(CommonInputAction::IaLeft);
    }
    if dy >= AXIS_THRESHOLD {
        actions.push(CommonInputAction::IaForward);
    } else if dy <= -AXIS_THRESHOLD {
        actions.push(CommonInputAction::IaBackward);
    }
    if actions.is_empty() {
        return Err(format!(
            "no direction: dx or dy must be at least {AXIS_THRESHOLD} from 0"
        ));
    }
    Ok(actions)
}

/// The action held for a gait with the direction keys, `None` for jog.
pub fn gait_action(kind: &str) -> Result<Option<CommonInputAction>, String> {
    match kind {
        "walk" => Ok(Some(CommonInputAction::IaWalk)),
        "jog" => Ok(None),
        "run" => Ok(Some(CommonInputAction::IaModifier)),
        other => Err(format!("unknown kind `{other}`: use walk, jog or run")),
    }
}

/// What an InputModifier disables, by name; empty when it disables nothing.
pub fn disabled(modifier: &PbInputModifier) -> Vec<&'static str> {
    let Some(Mode::Standard(standard)) = &modifier.mode else {
        return Vec::new();
    };
    [
        ("all", standard.disable_all),
        ("walk", standard.disable_walk),
        ("jog", standard.disable_jog),
        ("run", standard.disable_run),
        ("jump", standard.disable_jump),
        ("emote", standard.disable_emote),
        ("doubleJump", standard.disable_double_jump),
        ("gliding", standard.disable_gliding),
    ]
    .into_iter()
    .filter(|(_, flag)| flag.unwrap_or(false))
    .map(|(name, _)| name)
    .collect()
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Stage {
    /// the presses are queued: waiting for the last to be played
    Pressing { step: u64 },
    /// the keys are down since this time
    Holding { since: f64 },
    /// the releases are queued; the keys went down at `since`
    Releasing { step: u64, since: f64 },
    /// the keys are up since this frame; the player has been still for `still` frames
    Settling { frame: u32, still: u32, held: f64 },
}

impl Stage {
    fn name(&self) -> &'static str {
        match self {
            Stage::Pressing { .. } => "pressing",
            Stage::Holding { .. } => "holding",
            Stage::Releasing { .. } => "releasing",
            Stage::Settling { .. } => "settling",
        }
    }
}

struct Walk {
    reply: RpcResultSender<Result<String, String>>,
    stage: Stage,
    /// the gait and direction bindings, in the order they were pressed
    held: Vec<InputIdentifier>,
    /// the jump binding while it is down
    jump: Option<InputIdentifier>,
    /// seconds the keys stay down
    hold: f64,
    /// seconds the jump key stays down
    jump_hold: f64,
    settle_frames: u32,
    start: Vec3,
    last: Vec3,
    kind: String,
    jumped: bool,
    keys: Vec<String>,
    ignore: bool,
    /// what the active InputModifiers disabled while the walk ran
    modifier: BTreeSet<&'static str>,
}

#[derive(Resource, Default)]
pub struct WalkJob(Option<Walk>);

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct WalkReply<'a> {
    start_position: Value,
    end_position: Value,
    /// from the start to the end, straight
    distance: f64,
    parcel: [i32; 2],
    /// how long the keys were down
    seconds: f64,
    kind: &'a str,
    jump: bool,
    keys: &'a [String],
    /// an InputModifier disabled something while the walk ran
    modifier_active: bool,
    modifier: Option<Vec<&'static str>>,
    ignore_input_modifiers: bool,
    /// the player stopped within 3 s of the keys going up
    settled: bool,
}

#[allow(clippy::too_many_arguments)]
fn walk_cmd(
    mut input: ConsoleCommand<WalkCommand>,
    mut job: ResMut<WalkJob>,
    mut arbiter: ResMut<GestureArbiter>,
    mut script: ResMut<InputScript>,
    mut bypass: ResMut<InputModifierBypass>,
    mut pending: ResMut<PendingConsoleResponses>,
    diagnostics: Res<DiagnosticsStore>,
    input_map: Res<InputMap>,
    input_manager: InputManager,
    player: Query<&GlobalTransform, With<PrimaryUser>>,
) {
    let Some(Ok(command)) = input.take() else {
        return;
    };
    let result = start_walk(
        &command,
        &mut job,
        &mut arbiter,
        &mut script,
        &FrameRate::measured(&diagnostics),
        &input_map,
        &input_manager,
        &player,
    );
    match result {
        Ok(rx) => {
            bypass.0 = command.ignore_input_modifiers;
            pending.push_receiver(rx, |r| r, input.take_responder());
        }
        Err(e) => input.reply_failed(e),
    }
}

/// Checks everything that can refuse the walk, then presses its keys.
#[allow(clippy::too_many_arguments)]
fn start_walk(
    command: &WalkCommand,
    job: &mut WalkJob,
    arbiter: &mut GestureArbiter,
    script: &mut InputScript,
    rate: &FrameRate,
    input_map: &InputMap,
    input_manager: &InputManager,
    player: &Query<&GlobalTransform, With<PrimaryUser>>,
) -> Result<RpcResultReceiver<Result<String, String>>, String> {
    arbiter.refuse_running(WALK)?;
    if !command.seconds.is_finite() || command.seconds <= 0.0 || command.seconds > MAX_SECONDS {
        return Err(format!(
            "--seconds takes more than 0 and at most {MAX_SECONDS}; got {}",
            command.seconds
        ));
    }
    let gait = gait_action(&command.kind)?;
    let directions = direction_actions(command.dx, command.dy)?;
    let Ok(start) = player.single().map(GlobalTransform::translation) else {
        return Err(refusal("no player", None));
    };

    let binding = |action: CommonInputAction| -> Result<InputIdentifier, String> {
        let name = format!("{action:?}");
        let cause = || Some(json!({ "input": name }));
        let binding = input_map
            .get_input(action)
            .ok_or_else(|| refusal(format!("{name} has no binding"), cause()))?;
        if let Some(reason) = binding_error(&name, &binding) {
            return Err(refusal(reason, cause()));
        }
        if !input_manager.check_priority(&binding, InputPriority::Scene) {
            return Err(refusal(
                "the HUD holds the input",
                Some(json!({ "input": name, "holder": "hud" })),
            ));
        }
        Ok(binding)
    };
    let held = gait
        .into_iter()
        .chain(directions)
        .map(binding)
        .collect::<Result<Vec<_>, _>>()?;
    let jump = command
        .jump
        .then(|| binding(CommonInputAction::IaJump))
        .transpose()?;

    let mut step = 0;
    for binding in held.iter().chain(&jump) {
        if let Some(press) = binding_step(binding, ButtonState::Pressed) {
            step = script.push(press);
        }
    }
    let key_name = |binding: &InputIdentifier| match binding {
        InputIdentifier::Key(key) => format!("{key:?}"),
        InputIdentifier::Mouse(button) => format!("Mouse{button:?}"),
        other => format!("{other:?}"),
    };
    let keys: Vec<String> = held.iter().chain(&jump).map(key_name).collect();
    arbiter.report(WALK, status(Stage::Pressing { step }, &keys));
    let (reply, rx) = RpcResultSender::channel();
    job.0 = Some(Walk {
        reply,
        stage: Stage::Pressing { step },
        held,
        jump,
        hold: command.seconds as f64,
        jump_hold: JUMP_HOLD_SECONDS.min(command.seconds) as f64,
        settle_frames: rate.frames_in(SETTLE_SECONDS),
        start,
        last: start,
        kind: command.kind.clone(),
        jumped: command.jump,
        keys,
        ignore: command.ignore_input_modifiers,
        modifier: BTreeSet::new(),
    });
    Ok(rx)
}

/// What the walk is doing, for `/gesture_state`: its stage and the keys it holds.
fn status(stage: Stage, keys: &[String]) -> String {
    format!("{} {}", stage.name(), keys.join(" "))
}

/// Queues the release of `binding`; the step number, or 0 when it has none.
fn release(script: &mut InputScript, binding: &InputIdentifier) -> u64 {
    binding_step(binding, ButtonState::Released)
        .map(|step| script.push(step))
        .unwrap_or_default()
}

/// Follows the walk each frame: starts the hold once the presses are played, releases the jump and
/// then the keys when their time is up, and answers once the player has settled.
#[allow(clippy::too_many_arguments)]
fn drive_walk(
    mut job: ResMut<WalkJob>,
    mut arbiter: ResMut<GestureArbiter>,
    mut script: ResMut<InputScript>,
    mut bypass: ResMut<InputModifierBypass>,
    frame: Res<FrameCount>,
    time: Res<Time>,
    diagnostics: Res<DiagnosticsStore>,
    player: Query<
        (
            &GlobalTransform,
            Option<&ActivePlayerComponent<InputModifier>>,
        ),
        With<PrimaryUser>,
    >,
) {
    if job.0.is_none() {
        return;
    }
    let Ok((transform, modifier)) = player.single() else {
        // nobody is left to walk: let the keys go and answer, or they stay down and the gesture
        // stays claimed for good
        if let Some(mut walk) = job.0.take() {
            if matches!(walk.stage, Stage::Pressing { .. } | Stage::Holding { .. }) {
                for binding in walk.jump.take().iter().chain(walk.held.iter().rev()) {
                    release(&mut script, binding);
                }
            }
            bypass.0 = false;
            arbiter.release(WALK);
            walk.reply.send(Err(refusal(
                "the player was removed during the walk",
                Some(json!({ "stage": walk.stage.name() })),
            )));
        }
        return;
    };
    let Some(walk) = job.0.as_mut() else {
        return;
    };
    let frame = frame.0;
    let position = transform.translation();
    let now = time.elapsed_secs_f64();
    let frame_seconds = FrameRate::measured(&diagnostics).frame_seconds as f64;
    let before = walk.stage.name();
    let mut done = None;
    if let Some(modifier) = modifier.and_then(|m| m.component.0.as_ref()) {
        walk.modifier.extend(disabled(modifier));
    }

    walk.stage = match walk.stage {
        // the steps of a frame are played in its `PreUpdate`, before this `Update`
        Stage::Pressing { step } if script.played() >= step => Stage::Holding { since: now },
        Stage::Holding { since } => {
            if now - since >= walk.jump_hold {
                if let Some(jump) = walk.jump.take() {
                    release(&mut script, &jump);
                }
            }
            // the release is played next frame: queue it a frame early
            if now + frame_seconds - since >= walk.hold {
                let mut step = 0;
                for binding in walk.jump.take().iter().chain(walk.held.iter().rev()) {
                    step = step.max(release(&mut script, binding));
                }
                Stage::Releasing { step, since }
            } else {
                walk.stage
            }
        }
        Stage::Releasing { step, since } if script.played() >= step => Stage::Settling {
            frame,
            still: 0,
            held: now - since,
        },
        Stage::Settling {
            frame: from,
            still,
            held,
        } => {
            let still = if position.distance(walk.last) < STILL_METRES {
                still + 1
            } else {
                0
            };
            let settled = still >= STILL_FRAMES;
            if settled || frame >= from.saturating_add(walk.settle_frames) {
                done = Some((held, settled));
            }
            Stage::Settling {
                frame: from,
                still,
                held,
            }
        }
        stage => stage,
    };
    walk.last = position;
    if walk.stage.name() != before {
        arbiter.report(WALK, status(walk.stage, &walk.keys));
    }

    if let Some((held, settled)) = done {
        bypass.0 = false;
        arbiter.release(WALK);
        if let Some(walk) = job.0.take() {
            let reply = walk_reply(&walk, position, held, settled);
            walk.reply.send(Ok(reply));
        }
    }
}

fn walk_reply(walk: &Walk, end: Vec3, held: f64, settled: bool) -> String {
    let parcel = vec3_to_parcel(end);
    let point = |v: Vec3| reply::xyz(DclTranslation::from_bevy_translation(v).0);
    let modifier: Vec<_> = walk.modifier.iter().copied().collect();
    serde_json::to_string(&WalkReply {
        start_position: point(walk.start),
        end_position: point(end),
        distance: reply::round(walk.start.distance(end)),
        parcel: [parcel.x, parcel.y],
        seconds: reply::round(held as f32),
        kind: &walk.kind,
        jump: walk.jumped,
        keys: &walk.keys,
        modifier_active: !modifier.is_empty(),
        modifier: (!modifier.is_empty()).then_some(modifier),
        ignore_input_modifiers: walk.ignore,
        settled,
    })
    .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use dcl_component::proto_components::sdk::components::pb_input_modifier::StandardInput;

    #[test]
    fn directions_press_one_key_per_axis_past_the_threshold() {
        assert_eq!(
            direction_actions(0.0, 1.0).unwrap(),
            vec![CommonInputAction::IaForward]
        );
        assert_eq!(
            direction_actions(-1.0, -0.5).unwrap(),
            vec![CommonInputAction::IaLeft, CommonInputAction::IaBackward]
        );
        assert_eq!(
            direction_actions(0.2, 1.0).unwrap(),
            vec![CommonInputAction::IaForward]
        );
        assert!(direction_actions(0.1, -0.1)
            .unwrap_err()
            .starts_with("no direction"));
        assert!(direction_actions(2.0, 0.0).is_err());
        assert!(direction_actions(f32::NAN, 1.0).is_err());
    }

    #[test]
    fn gaits_hold_the_walk_or_run_binding_and_jog_neither() {
        assert_eq!(gait_action("walk"), Ok(Some(CommonInputAction::IaWalk)));
        assert_eq!(gait_action("jog"), Ok(None));
        assert_eq!(gait_action("run"), Ok(Some(CommonInputAction::IaModifier)));
        assert!(gait_action("sprint").is_err());
    }

    #[test]
    fn a_modifier_names_what_it_disables() {
        let modifier = PbInputModifier {
            mode: Some(Mode::Standard(StandardInput {
                disable_all: Some(true),
                disable_jump: Some(true),
                disable_run: Some(false),
                ..Default::default()
            })),
        };
        assert_eq!(disabled(&modifier), vec!["all", "jump"]);
        assert!(disabled(&PbInputModifier { mode: None }).is_empty());
    }
}
