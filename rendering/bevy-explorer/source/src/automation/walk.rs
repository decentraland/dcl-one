//! `/walk`: hold the movement keys for a time, camera-relative, as a person would, and answer with
//! where the player went. The keys are the ones bound to the movement actions, pressed through the
//! input script, so the movement scene reads them as it reads the keyboard: the gait is the walk
//! (`IaWalk`) or run (`IaModifier`) binding held with them, jog neither. `--ignore-input-modifiers`
//! keeps the active `InputModifier` out of the avatar movement info for the walk
//! (`EngineMovementControl::suppress_input_modifier`); it overrides a scene's rule, so it is refused outside preview, as the
//! preview-only movement commands are absent there. `drive_walk` follows the walk frame by frame
//! and answers once the player has settled:
//!
//! `{"startPosition", "endPosition": {"x", "y", "z"}, "distance", "parcel": [x, y], "seconds",
//! "kind", "jump", "keys", "modifierActive", "modifier", "ignoreInputModifiers", "settled"}`
//!
//! A walk that cannot start (one is running, the HUD holds a key, a gamepad binding, no player or
//! one out of the world, `--ignore-input-modifiers` outside preview) is refused before a key is
//! pressed: `{"refused", "cause"}`. `/gesture_state` and the debug panel's "Automation gestures"
//! section read the running walk from `WalkJob`.

use std::collections::{BTreeMap, BTreeSet};

use bevy::{input::ButtonState, prelude::*};
use bevy_console::ConsoleCommand;
use common::{
    inputs::{CommonInputAction, InputIdentifier, InputMap},
    rpc::{RpcResultReceiver, RpcResultSender},
    sets::SceneSets,
    structs::{EngineMovementControl, OutOfWorld, PreviewMode, PrimaryUser},
};
use console::{
    reply::{self, refusal},
    AddDebugSection, DebugSection, DoAddConsoleCommand, PendingConsoleResponses,
};
use dcl_component::{
    proto_components::sdk::components::{pb_input_modifier::Mode, PbInputModifier},
    transform_and_parent::DclTranslation,
};
use input_manager::{InputManager, InputPriority};
use scene_runner::{update_world::avatar_modifier_area::InputModifier, vec3_to_parcel};
use serde::Serialize;
use serde_json::{json, Value};
use user_input::avatar_movement::ActivePlayerComponent;

use super::{
    input::InputScript,
    pointer::{binding_error, binding_step},
};

/// The walk's name in `/gesture_state`, as the holder a second walk is refused with, and as its
/// reason in `EngineMovementControl::suppress_input_modifier`.
const WALK: &str = "walk";

/// How long the jump key stays down: long enough for a full jump, short of a second one.
const JUMP_HOLD_SECONDS: f32 = 0.5;
/// How fast the player may move, in metres a second, and count as still.
const STILL_SPEED: f32 = 0.3;
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
            .add_console_command::<GestureStateCommand, _>(gesture_state_cmd)
            .add_debug_section(DebugSection {
                title: "Automation gestures",
                query: "/gesture_state",
                fields: &[
                    ("Walk", "running.walk"),
                    (
                        "InputModifier withheld from the movement info",
                        "inputModifierWithheld",
                    ),
                ],
                actions: &[],
                inputs: &[],
            })
            .add_systems(Update, drive_walk.in_set(SceneSets::PostLoop));
    }
}

/// Walk the player camera-relative for --seconds (1 by default) by holding the movement keys, as
/// the keyboard does: dx is right (+) or left (-), dy forward (+) or backward (-), each from -1 to
/// 1, a component at least 0.3 from 0 pressing its key. --kind walk holds the walk key with them,
/// run the run key, jog neither; --jump presses jump as the walk starts (held 0.5 s, or the walk if shorter);
/// --ignore-input-modifiers keeps the scene's InputModifier out of the avatar movement info for
/// the walk, in preview only. Answers, as JSON, once the player has settled after the keys are up
/// (settled: false when it had not within 3 s): startPosition, endPosition, distance, parcel, the
/// seconds the keys were held, the keys, and whether an InputModifier was active (modifierActive,
/// modifier: the disabled gaits)
#[derive(clap::Parser, ConsoleCommand)]
#[command(name = "/walk")]
struct WalkCommand {
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
    /// Keep the active InputModifier out of the avatar movement info for the walk (preview only)
    #[arg(long)]
    ignore_input_modifiers: bool,
}

/// The movement actions `dx`, `dy` press: right or left, then forward or backward.
fn direction_actions(dx: f32, dy: f32) -> Result<Vec<CommonInputAction>, String> {
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
fn gait_action(kind: &str) -> Result<Option<CommonInputAction>, String> {
    match kind {
        "walk" => Ok(Some(CommonInputAction::IaWalk)),
        "jog" => Ok(None),
        "run" => Ok(Some(CommonInputAction::IaModifier)),
        other => Err(format!("unknown kind `{other}`: use walk, jog or run")),
    }
}

/// What an InputModifier disables, by name; empty when it disables nothing.
fn disabled(modifier: &PbInputModifier) -> Vec<&'static str> {
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
    /// the keys are up since this time; the player has been still for `still` frames
    Settling { since: f64, still: u32, held: f64 },
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
    start: Vec3,
    last: Vec3,
    kind: String,
    jumped: bool,
    keys: Vec<String>,
    ignore: bool,
    /// what the active InputModifiers disabled while the walk ran
    modifier: BTreeSet<&'static str>,
}

impl Walk {
    /// What the walk is doing, for `/gesture_state`: its stage and its keys, which are queued while
    /// pressing, down while holding and going up while releasing.
    fn status(&self) -> String {
        let down = match self.stage {
            Stage::Settling { .. } => None,
            _ => Some(self.held.iter().chain(&self.jump)),
        };
        std::iter::once(self.stage.name().to_owned())
            .chain(down.into_iter().flatten().map(key_name))
            .collect::<Vec<_>>()
            .join(" ")
    }
}

fn key_name(binding: &InputIdentifier) -> String {
    match binding {
        InputIdentifier::Key(key) => format!("{key:?}"),
        InputIdentifier::Mouse(button) => format!("Mouse{button:?}"),
        other => format!("{other:?}"),
    }
}

/// The walk that is running, if any: a second one is refused until it has answered.
#[derive(Resource, Default)]
struct WalkJob(Option<Walk>);

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

type Player<'w, 's> = Query<'w, 's, (&'static GlobalTransform, Has<OutOfWorld>), With<PrimaryUser>>;

#[allow(clippy::too_many_arguments)]
fn walk_cmd(
    mut input: ConsoleCommand<WalkCommand>,
    mut job: ResMut<WalkJob>,
    mut script: ResMut<InputScript>,
    mut movement_control: ResMut<EngineMovementControl>,
    mut pending: ResMut<PendingConsoleResponses>,
    preview: Option<Res<PreviewMode>>,
    input_map: Res<InputMap>,
    input_manager: InputManager,
    player: Player,
) {
    let Some(Ok(command)) = input.take() else {
        return;
    };
    let result = start_walk(
        &command,
        preview.is_some_and(|preview| preview.is_preview),
        &mut job,
        &mut script,
        &mut movement_control,
        &input_map,
        &input_manager,
        &player,
    );
    match result {
        Ok(rx) => pending.push_receiver(rx, |r| r, input.take_responder()),
        Err(e) => input.reply_failed(e),
    }
}

/// Checks everything that can refuse the walk, then presses its keys and, with
/// `--ignore-input-modifiers`, withholds the `InputModifier`. A refused walk changes nothing.
#[allow(clippy::too_many_arguments)]
fn start_walk(
    command: &WalkCommand,
    preview: bool,
    job: &mut WalkJob,
    script: &mut InputScript,
    movement_control: &mut EngineMovementControl,
    input_map: &InputMap,
    input_manager: &InputManager,
    player: &Player,
) -> Result<RpcResultReceiver<Result<String, String>>, String> {
    if job.0.is_some() {
        return Err(refusal(
            "a walk is still running; wait for its reply",
            Some(json!({ "holder": WALK })),
        ));
    }
    if !command.seconds.is_finite() || command.seconds <= 0.0 || command.seconds > MAX_SECONDS {
        return Err(format!(
            "--seconds takes more than 0 and at most {MAX_SECONDS}; got {}",
            command.seconds
        ));
    }
    let gait = gait_action(&command.kind)?;
    let directions = direction_actions(command.dx, command.dy)?;
    if command.ignore_input_modifiers && !preview {
        return Err(refusal(
            "--ignore-input-modifiers overrides a scene's InputModifier, which only a preview may do",
            Some(json!({ "flag": "ignore-input-modifiers" })),
        ));
    }
    let start = match player.single() {
        Ok((_, true)) => {
            return Err(refusal(
                "the player is out of the world",
                Some(json!({ "player": "outOfWorld" })),
            ))
        }
        Ok((transform, false)) => transform.translation(),
        Err(_) => return Err(refusal("no player", None)),
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
    if command.ignore_input_modifiers {
        movement_control.suppress_input_modifier.insert(WALK);
    }
    let keys: Vec<String> = held.iter().chain(&jump).map(key_name).collect();
    let (reply, rx) = RpcResultSender::channel();
    job.0 = Some(Walk {
        reply,
        stage: Stage::Pressing { step },
        held,
        jump,
        hold: command.seconds as f64,
        jump_hold: JUMP_HOLD_SECONDS.min(command.seconds) as f64,
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

/// Queues the release of `binding`; the step number, or 0 when it has none.
fn release(script: &mut InputScript, binding: &InputIdentifier) -> u64 {
    binding_step(binding, ButtonState::Released)
        .map(|step| script.push(step))
        .unwrap_or_default()
}

/// Follows the walk each frame: starts the hold once the presses are played, releases the jump and
/// then the keys when their time is up, and answers once the player has settled.
fn drive_walk(
    mut job: ResMut<WalkJob>,
    mut script: ResMut<InputScript>,
    mut movement_control: ResMut<EngineMovementControl>,
    time: Res<Time>,
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
        // nobody is left to walk: let the keys go and answer, or they stay down and every later
        // walk is refused
        if let Some(mut walk) = job.0.take() {
            if matches!(walk.stage, Stage::Pressing { .. } | Stage::Holding { .. }) {
                for binding in walk.jump.take().iter().chain(walk.held.iter().rev()) {
                    release(&mut script, binding);
                }
            }
            withhold_no_more(&mut movement_control);
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
    let position = transform.translation();
    let now = time.elapsed_secs_f64();
    let frame_seconds = time.delta_secs_f64();
    let mut done = None;
    if let Some(modifier) = modifier.and_then(|m| m.component.0.as_ref()) {
        walk.modifier.extend(disabled(modifier));
    }

    walk.stage = match walk.stage {
        // the steps of a frame are played in its `PreUpdate`, before this `Update`
        Stage::Pressing { step } if script.played() >= step => Stage::Holding { since: now },
        Stage::Holding { since } => {
            // a release is played next frame: queue it a frame early
            if now + frame_seconds - since >= walk.jump_hold {
                if let Some(jump) = walk.jump.take() {
                    release(&mut script, &jump);
                }
            }
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
            since: now,
            still: 0,
            held: now - since,
        },
        Stage::Settling { since, still, held } => {
            // a speed, not a distance a frame, so "still" is the same at any frame rate; a frame
            // that took no time says nothing, and leaves the count as it was
            let moved = position.distance(walk.last) as f64;
            let still = if frame_seconds <= 0.0 {
                still
            } else if moved / frame_seconds < STILL_SPEED as f64 {
                still + 1
            } else {
                0
            };
            let settled = still >= STILL_FRAMES;
            if settled || now - since >= SETTLE_SECONDS as f64 {
                done = Some((held, settled));
            }
            Stage::Settling { since, still, held }
        }
        stage => stage,
    };
    walk.last = position;

    if let Some((held, settled)) = done {
        withhold_no_more(&mut movement_control);
        if let Some(walk) = job.0.take() {
            let reply = walk_reply(&walk, position, held, settled);
            walk.reply.send(Ok(reply));
        }
    }
}

/// Ends the walk's hold on the `InputModifier`, without marking the resource changed when the walk
/// held none.
fn withhold_no_more(movement_control: &mut ResMut<EngineMovementControl>) {
    if movement_control.suppress_input_modifier.contains(WALK) {
        movement_control.suppress_input_modifier.remove(WALK);
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

/// Read the running /walk, as JSON: running (walk, with its stage and its keys: the ones queued
/// while pressing, down while holding, going up while releasing; empty when no walk runs) and
/// inputModifierWithheld (the walk keeps the InputModifier out of the avatar movement info)
#[derive(clap::Parser, ConsoleCommand)]
#[command(name = "/gesture_state")]
struct GestureStateCommand;

fn gesture_state_cmd(
    mut input: ConsoleCommand<GestureStateCommand>,
    job: Res<WalkJob>,
    movement_control: Res<EngineMovementControl>,
) {
    let Some(Ok(_)) = input.take() else {
        return;
    };
    input.reply_ok(gesture_state(&job, &movement_control).to_string());
}

fn gesture_state(job: &WalkJob, movement_control: &EngineMovementControl) -> Value {
    let running: BTreeMap<&str, String> = job.0.iter().map(|walk| (WALK, walk.status())).collect();
    json!({
        "running": running,
        "inputModifierWithheld": movement_control.suppress_input_modifier.contains(WALK),
        "unavailable": { "running.walk": "No walk is running." },
    })
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use bevy::{
        ecs::system::SystemState,
        input::{keyboard::KeyboardInput, mouse::MouseButtonInput},
    };
    use dcl_component::proto_components::sdk::components::pb_input_modifier::StandardInput;
    use input_manager::{CumulativeAxisData, InputPriorities, InputType};

    use super::super::input::play_input_script;
    use super::*;

    /// A frame of the test app, in seconds: exact in binary, so the stage times compare exactly.
    const FRAME: f64 = 0.25;

    type Start<'w, 's> = (
        ResMut<'w, WalkJob>,
        ResMut<'w, InputScript>,
        ResMut<'w, EngineMovementControl>,
        Res<'w, InputMap>,
        InputManager<'w, 's>,
        Player<'w, 's>,
    );

    /// The input script, the default bindings, the walk and its driver, and a player at the
    /// origin; no window, so a played step writes nothing.
    fn app() -> (App, Entity) {
        let mut app = App::new();
        app.add_event::<KeyboardInput>()
            .add_event::<MouseButtonInput>()
            .init_resource::<InputScript>()
            .init_resource::<InputMap>()
            .init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<ButtonInput<MouseButton>>()
            .init_resource::<CumulativeAxisData>()
            .init_resource::<InputPriorities>()
            .init_resource::<EngineMovementControl>()
            .init_resource::<WalkJob>()
            .init_resource::<Time>()
            .add_systems(PreUpdate, play_input_script)
            .add_systems(Update, drive_walk);
        let player = app
            .world_mut()
            .spawn((PrimaryUser::default(), GlobalTransform::IDENTITY))
            .id();
        (app, player)
    }

    fn command(seconds: f32) -> WalkCommand {
        WalkCommand {
            dx: 0.0,
            dy: 1.0,
            seconds,
            kind: "jog".to_owned(),
            jump: false,
            ignore_input_modifiers: false,
        }
    }

    fn start(
        app: &mut App,
        command: &WalkCommand,
        preview: bool,
    ) -> Result<RpcResultReceiver<Result<String, String>>, String> {
        let mut state = SystemState::<Start>::new(app.world_mut());
        let (mut job, mut script, mut movement_control, input_map, input_manager, player) =
            state.get_mut(app.world_mut());
        start_walk(
            command,
            preview,
            &mut job,
            &mut script,
            &mut movement_control,
            &input_map,
            &input_manager,
            &player,
        )
    }

    /// One frame of `FRAME` seconds.
    fn frame(app: &mut App) {
        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(Duration::from_secs_f64(FRAME));
        app.update();
    }

    fn stage(app: &App) -> Option<&'static str> {
        app.world()
            .resource::<WalkJob>()
            .0
            .as_ref()
            .map(|walk| walk.stage.name())
    }

    /// Runs frames until the walk is at `wanted`, and fails if it is not within `limit` frames.
    fn run_to(app: &mut App, wanted: &str, limit: u32) {
        for _ in 0..limit {
            if stage(app) == Some(wanted) {
                return;
            }
            frame(app);
        }
        assert_eq!(stage(app), Some(wanted));
    }

    fn queued(app: &App) -> usize {
        app.world().resource::<InputScript>().steps()
    }

    fn bypassed(app: &App) -> bool {
        app.world()
            .resource::<EngineMovementControl>()
            .suppress_input_modifier
            .contains(WALK)
    }

    fn refused(text: &str) -> Value {
        let value: Value = serde_json::from_str(text).expect("a refusal is JSON");
        assert!(value["refused"].is_string(), "{text}");
        value
    }

    #[test]
    fn a_walk_presses_holds_releases_and_answers_once_the_player_is_still() {
        let (mut app, _) = app();
        let mut rx = start(&mut app, &command(1.0), false).unwrap();
        assert_eq!(stage(&app), Some("pressing"));
        assert_eq!(queued(&app), 1);

        // the press is played in the frame's `PreUpdate`, and the hold starts in its `Update`
        frame(&mut app);
        assert_eq!(stage(&app), Some("holding"));
        assert_eq!(queued(&app), 0);
        assert_eq!(
            gesture_state(
                app.world().resource::<WalkJob>(),
                app.world().resource::<EngineMovementControl>()
            )["running"]["walk"],
            "holding KeyW"
        );

        // the release is queued a frame before the second is up, and played in the next
        frame(&mut app);
        frame(&mut app);
        assert_eq!(stage(&app), Some("holding"));
        frame(&mut app);
        assert_eq!(stage(&app), Some("releasing"));
        assert_eq!(queued(&app), 1);
        frame(&mut app);
        assert_eq!(stage(&app), Some("settling"));
        assert_eq!(queued(&app), 0);
        assert!(rx.try_recv().is_err());

        // still for three frames
        frame(&mut app);
        frame(&mut app);
        assert!(rx.try_recv().is_err());
        frame(&mut app);
        let reply: Value = serde_json::from_str(&rx.try_recv().unwrap().unwrap()).unwrap();
        assert_eq!(reply["settled"], true);
        assert_eq!(reply["seconds"], 1.0);
        assert_eq!(reply["keys"], json!(["KeyW"]));
        assert_eq!(reply["distance"], 0.0);
        assert_eq!(reply["ignoreInputModifiers"], false);
        assert_eq!(stage(&app), None);
        assert_eq!(
            gesture_state(
                app.world().resource::<WalkJob>(),
                app.world().resource::<EngineMovementControl>()
            )["running"],
            json!({})
        );
    }

    #[test]
    fn the_jump_key_goes_up_before_the_others_and_leaves_the_status() {
        let (mut app, _) = app();
        let mut walk = command(2.0);
        walk.kind = "run".to_owned();
        walk.jump = true;
        let _rx = start(&mut app, &walk, false).unwrap();
        // the gait key first, then the direction, then jump: one press a frame
        assert_eq!(queued(&app), 3);
        let status = |app: &App| {
            app.world()
                .resource::<WalkJob>()
                .0
                .as_ref()
                .map(Walk::status)
        };
        assert_eq!(
            status(&app).as_deref(),
            Some("pressing ShiftLeft KeyW Space")
        );
        run_to(&mut app, "holding", 4);
        assert_eq!(
            status(&app).as_deref(),
            Some("holding ShiftLeft KeyW Space")
        );
        // its release is queued a frame before half a second is up; the others stay down
        frame(&mut app);
        assert_eq!(status(&app).as_deref(), Some("holding ShiftLeft KeyW"));
        assert_eq!(queued(&app), 1);
        // and played in the next, half a second after the hold began
        frame(&mut app);
        assert_eq!(queued(&app), 0);
        assert_eq!(stage(&app), Some("holding"));
    }

    #[test]
    fn a_player_that_keeps_moving_answers_unsettled_after_three_seconds() {
        let (mut app, player) = app();
        let mut rx = start(&mut app, &command(0.5), false).unwrap();
        run_to(&mut app, "settling", 8);
        let mut frames = 0;
        let reply = loop {
            // a metre a frame: 4 m/s
            frames += 1;
            app.world_mut()
                .entity_mut(player)
                .insert(GlobalTransform::from_translation(Vec3::X * frames as f32));
            frame(&mut app);
            if let Ok(reply) = rx.try_recv() {
                break reply.unwrap();
            }
            assert!(frames < 20, "no reply after {frames} frames");
        };
        // 3 s at a quarter of a second a frame
        assert_eq!(frames, 12);
        let reply: Value = serde_json::from_str(&reply).unwrap();
        assert_eq!(reply["settled"], false);
        assert_eq!(reply["distance"], 12.0);
        assert_eq!(stage(&app), None);
    }

    #[test]
    fn a_removed_player_ends_the_walk_and_releases_only_the_keys_still_down() {
        // (the stage the player is removed at, the releases queued by its removal)
        for (at, releases) in [
            ("pressing", 1),
            ("holding", 1),
            ("releasing", 0),
            ("settling", 0),
        ] {
            let (mut app, player) = app();
            let mut walk = command(1.0);
            walk.ignore_input_modifiers = true;
            let mut rx = start(&mut app, &walk, true).unwrap();
            assert!(bypassed(&app), "{at}");
            run_to(&mut app, at, 8);
            app.world_mut().entity_mut(player).despawn();
            frame(&mut app);

            let cause = refused(&rx.try_recv().unwrap().unwrap_err());
            assert_eq!(cause["cause"]["stage"], at);
            assert_eq!(stage(&app), None, "{at}");
            assert!(!bypassed(&app), "{at}");
            // whatever was queued before the removal has been played by now
            assert_eq!(queued(&app), releases, "{at}");
        }
    }

    #[test]
    fn the_bypass_lasts_for_the_walk_only() {
        let (mut app, _) = app();
        let mut walk = command(0.5);
        walk.ignore_input_modifiers = true;
        let mut rx = start(&mut app, &walk, true).unwrap();
        assert!(bypassed(&app));
        for _ in 0..12 {
            if stage(&app).is_none() {
                break;
            }
            assert!(bypassed(&app));
            frame(&mut app);
        }
        let reply: Value = serde_json::from_str(&rx.try_recv().unwrap().unwrap()).unwrap();
        assert_eq!(reply["ignoreInputModifiers"], true);
        assert!(!bypassed(&app));
    }

    #[test]
    fn a_refused_walk_presses_nothing_and_changes_nothing() {
        let untouched = |app: &App, what: &str| {
            assert_eq!(queued(app), 0, "{what}");
            assert_eq!(stage(app), None, "{what}");
            assert!(!bypassed(app), "{what}");
        };

        // the bypass outside preview
        let (mut app, player) = app();
        let mut walk = command(1.0);
        walk.ignore_input_modifiers = true;
        let cause = refused(&start(&mut app, &walk, false).err().unwrap());
        assert_eq!(cause["cause"]["flag"], "ignore-input-modifiers");
        untouched(&app, "outside preview");

        // the HUD holds the keyboard
        app.world_mut()
            .resource_mut::<InputPriorities>()
            .reserve(InputType::Keyboard, InputPriority::Ui);
        let cause = refused(&start(&mut app, &command(1.0), false).err().unwrap());
        assert_eq!(
            cause["cause"],
            json!({ "input": "IaForward", "holder": "hud" })
        );
        untouched(&app, "the HUD holds the input");
        app.world_mut()
            .resource_mut::<InputPriorities>()
            .release(InputType::Keyboard, InputPriority::Ui);

        // a player out of the world, then none
        app.world_mut().entity_mut(player).insert(OutOfWorld);
        let cause = refused(&start(&mut app, &command(1.0), false).err().unwrap());
        assert_eq!(cause["cause"]["player"], "outOfWorld");
        untouched(&app, "out of the world");
        app.world_mut().entity_mut(player).despawn();
        refused(&start(&mut app, &command(1.0), false).err().unwrap());
        untouched(&app, "no player");
    }

    #[test]
    fn a_second_walk_is_refused_while_one_runs_and_leaves_it_alone() {
        let (mut app, _) = app();
        let _rx = start(&mut app, &command(1.0), false).unwrap();
        let mut second = command(1.0);
        second.ignore_input_modifiers = true;
        let cause = refused(&start(&mut app, &second, true).err().unwrap());
        assert_eq!(cause["cause"]["holder"], "walk");
        assert_eq!(queued(&app), 1);
        assert_eq!(stage(&app), Some("pressing"));
        assert!(!bypassed(&app));
    }

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
