//! MEASUREMENT ONLY (local measurement build, not for merge): the movement recorder ported onto
//! this branch with the scene loop's stage marks (`scene_runner::tick_stages`) in each frame's
//! `extra.stage` and the CPU probe's spans in `extra.cpu`. Velocities come from what this
//! branch has: `vel` is the frame's displacement over its delta, `vel_requested` the movement
//! scene's `AvatarMovement` velocity, `vel_external` null.
//!
//! `/movement_trace`: plays a scripted keyboard scenario on the player and records its movement
//! frame by frame, to compare locomotion across clients, frame rates and builds.
//!
//! A run places the player at a spawn point in the scene it stands in, as an SDK `movePlayerTo`
//! with a facing does, and waits until the player has stood still, grounded and in place, for the
//! scenario's `settle` seconds. The next frame is t = 0. With `--start placed` it waits only the
//! `settle` seconds from the placement and then the first frame the player rests on something
//! (ground, or a slope too steep to stand on), wherever it is, so a run can start on a ledge edge
//! or a steep slope the player slides off. From t = 0 the
//! scenario's keys are pressed by frame time and each frame is recorded until `duration`.
//!
//! The keys enter as window keyboard events ahead of Bevy's input systems, the way the window
//! delivers a real key: bindings, input priorities, the scenes' input events and the movement
//! scene all see what a keyboard would give them. Only the browser's or operating system's key
//! delivery is skipped. With `--input external` nothing is pressed: t = 0 is the first frame with
//! movement input, so a driver's real keys can be recorded on the same timeline.
//!
//! The camera's yaw is held at the scenario's `camera_yaw`, or follows the scenario's camera
//! track by frame time. With `--camera free` nothing holds it: whoever plays turns it, and each
//! frame records the camera's yaw and pitch, so a free play can be replayed as a scenario with a
//! camera track.
//!
//! The trace is JSON Lines: a header, one line per frame, a footer (see [`Run::finish`]). A long
//! run can hand its lines out while it records (`drain`), so it keeps only the frames since.
//! Positions are the SDK's, relative to the scene's base parcel; yaw is degrees clockwise from
//! north, in (-180, 180].

use std::f32::consts::FRAC_1_SQRT_2;

use bevy::{
    diagnostic::FrameCount,
    input::{
        keyboard::{Key, KeyboardInput, NativeKey},
        ButtonState, InputSystem,
    },
    prelude::*,
    time::TimeUpdateStrategy,
    window::PrimaryWindow,
};
use bevy_console::ConsoleCommand;
use common::{
    inputs::{Action, CommonInputAction, InputIdentifier, InputMap},
    rpc::{RpcCall, RpcCallEvent},
    sets::{PostUpdateSets, SceneSets},
    structs::{
        AvatarDynamicState, CurrentRealm, PrimaryCamera, PrimaryUser, SessionGraphics, Version,
    },
    util::RingBufferReceiver,
};
use console::{AddDebugSection, DebugSection, DoAddConsoleCommand};
use dcl_component::proto_components::common::Vector3;
use dcl_types::SceneLogMessage;
use input_manager::{InputManager, InputPriority};
use scene_runner::{
    initialize_scene::PARCEL_SIZE,
    renderer_context::RendererSceneContext,
    tick_stages::{FrameStages, TickStages, RELAY_NAMES},
    ContainingScene, OutOfWorld,
};
use serde::Deserialize;

use crate::{
    avatar_movement::{ActivePlayerComponent, AvatarMovement},
    camera::update_camera,
};

/// The trace format this module writes, and the scenario format without a camera track.
pub const FORMAT: u32 = 1;
/// The scenario format that may carry a camera track and last up to [`MAX_TRACK_DURATION`]. A
/// client that reads only format 1 refuses it instead of holding the camera still.
pub const CAMERA_TRACK_FORMAT: u32 = 2;
/// The longest a format 1 scenario runs (s).
const MAX_DURATION: f64 = 120.0;
/// The longest a format 2 scenario runs (s): an hour of free play, handed out with `drain`.
const MAX_TRACK_DURATION: f64 = 3600.0;
/// A frame within this much before an event's time reaches it, so an event on a frame boundary
/// (2.0 s at 60 fps) does not move between frames on rounding.
const EVENT_EPSILON: f64 = 1e-4;
/// A frame counts as dropped when it took longer than this many target frame intervals.
const DROPPED_FRAME_FACTOR: f64 = 1.5;
/// Still means slower than this, horizontally (m/s).
const STILL_SPEED: f32 = 1e-3;
/// In place means within this of the spawn point, horizontally (m), ...
const SPAWN_TOLERANCE: f32 = 0.02;
/// ... and facing within this of the spawn yaw (degrees).
const SPAWN_YAW_TOLERANCE: f64 = 1.0;
/// Placed again when still but not in place for this long after a placement (s).
const RESPAWN_AFTER: f64 = 1.0;
const MAX_SPAWNS: u32 = 5;
/// Gives up settling, or waiting for external input, after this long (s).
const WAIT_LIMIT: f64 = 30.0;
/// With `--start placed`, starts anyway when the player is still in the air this long after the
/// scenario's `settle` (s).
const GROUNDED_WAIT: f64 = 5.0;
/// The start of the console line in which the scene controlling movement prints its movement
/// model's state: `LOCO ` and a JSON object, from `console.log('LOCO ' + JSON.stringify(state))`.
const LOCOMOTION_LINE: &str = "LOCO ";

pub struct MovementTracePlugin;

impl Plugin for MovementTracePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<MovementTrace>()
            .add_console_command::<MovementTraceCommand, _>(movement_trace_command)
            .add_systems(PreUpdate, press_scenario_keys.before(InputSystem))
            .add_systems(
                Update,
                (read_consumed_input, drive_camera.after(update_camera)).in_set(SceneSets::Input),
            )
            .add_systems(
                PostUpdate,
                record_frame
                    .after(PostUpdateSets::CameraUpdate)
                    .before(PostUpdateSets::InverseKinematics),
            )
            .add_debug_section(DebugSection {
                title: "Movement trace",
                query: "/movement_trace status",
                fields: &[
                    ("Phase", "phase"),
                    ("Scenario", "scenario"),
                    ("Time (s)", "t"),
                    ("Frames", "frames"),
                    ("Dropped frames", "dropped"),
                    ("Input mismatch frames", "inputMismatchFrames"),
                    ("Error", "error"),
                ],
                actions: &[
                    ("Finish", "/movement_trace finish"),
                    ("Stop", "/movement_trace stop"),
                ],
                inputs: &[],
            });
    }
}

// --- scenario ---

/// A scripted input: a step function of key states over time, from t = 0 to `duration`.
#[derive(Deserialize, Clone, Debug, PartialEq)]
pub struct Scenario {
    pub format: u32,
    pub id: String,
    #[serde(default)]
    pub seed: i64,
    /// The frame rate to run at, unless the run gives its own
    pub fps: u32,
    /// The camera's yaw for the whole run, without a camera track; movement keys are
    /// camera-relative
    #[serde(default)]
    pub camera_yaw: f64,
    /// Format 2: the camera's yaw (and pitch) by time, interpolated between samples and held
    /// before the first and after the last
    #[serde(default)]
    pub camera: Vec<CameraSample>,
    /// Seconds still before t = 0
    #[serde(default = "default_settle")]
    pub settle: f64,
    /// Seconds recorded from t = 0
    pub duration: f64,
    /// Each event's keys hold until the next event's time
    pub events: Vec<ScenarioEvent>,
}

fn default_settle() -> f64 {
    1.0
}

#[derive(Deserialize, Clone, Copy, Debug, PartialEq, Default)]
pub struct ScenarioEvent {
    pub t: f64,
    #[serde(default)]
    pub fwd: i8,
    #[serde(default)]
    pub right: i8,
    #[serde(default)]
    pub walk: bool,
    #[serde(default)]
    pub sprint: bool,
    #[serde(default)]
    pub jump: bool,
}

/// A camera track sample: `yaw` in the SDK convention (degrees clockwise from north) and `pitch`
/// in degrees, positive looking up. Pitch is on every sample of a track or on none.
#[derive(Deserialize, Clone, Copy, Debug, PartialEq, Default)]
pub struct CameraSample {
    pub t: f64,
    pub yaw: f64,
    #[serde(default)]
    pub pitch: Option<f64>,
}

/// The camera's pose at a moment: SDK yaw in (-180, 180] and, when the track has it, pitch.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct CameraPose {
    pub yaw: f64,
    pub pitch: Option<f64>,
}

/// The keys held at a moment: `fwd` W (1) or S (-1), `right` D (1) or A (-1), the walk and
/// sprint modifiers and the jump key.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct InputStep {
    pub fwd: i8,
    pub right: i8,
    pub walk: bool,
    pub sprint: bool,
    pub jump: bool,
}

impl InputStep {
    /// The gait these keys ask for: sprint wins over walk, and neither moves without a direction.
    pub fn gait(&self) -> Gait {
        Gait::of(self.fwd != 0 || self.right != 0, self.walk, self.sprint)
    }
}

impl From<&ScenarioEvent> for InputStep {
    fn from(event: &ScenarioEvent) -> Self {
        Self {
            fwd: event.fwd,
            right: event.right,
            walk: event.walk,
            sprint: event.sprint,
            jump: event.jump,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Gait {
    #[default]
    Idle,
    Walk,
    Jog,
    Run,
}

impl Gait {
    fn of(moving: bool, walk: bool, sprint: bool) -> Self {
        match (moving, sprint, walk) {
            (false, _, _) => Gait::Idle,
            (true, true, _) => Gait::Run,
            (true, false, true) => Gait::Walk,
            (true, false, false) => Gait::Jog,
        }
    }

    fn name(&self) -> &'static str {
        match self {
            Gait::Idle => "idle",
            Gait::Walk => "walk",
            Gait::Jog => "jog",
            Gait::Run => "run",
        }
    }
}

impl Scenario {
    pub fn parse(json: &str) -> Result<Self, String> {
        let scenario: Scenario =
            serde_json::from_str(json).map_err(|e| format!("not a scenario: {e}"))?;
        scenario.validate()?;
        Ok(scenario)
    }

    fn validate(&self) -> Result<(), String> {
        let max_duration = match self.format {
            FORMAT => MAX_DURATION,
            CAMERA_TRACK_FORMAT => MAX_TRACK_DURATION,
            other => {
                return Err(format!(
                    "scenario format {other} is not {FORMAT} or {CAMERA_TRACK_FORMAT}"
                ))
            }
        };
        if self.id.is_empty()
            || !self
                .id
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
        {
            return Err("the id must be letters, digits, '.', '_' or '-'".into());
        }
        if !(1..=1000).contains(&self.fps) {
            return Err(format!("fps {} is outside 1..=1000", self.fps));
        }
        if !(self.duration > 0.0 && self.duration <= max_duration) {
            return Err(format!(
                "duration {} is outside (0, {max_duration}] for format {}",
                self.duration, self.format
            ));
        }
        if !(0.0..=30.0).contains(&self.settle) {
            return Err(format!("settle {} is outside [0, 30]", self.settle));
        }
        if !self.camera_yaw.is_finite() {
            return Err("camera_yaw is not a number".into());
        }
        if self.events.is_empty() {
            return Err("no events".into());
        }
        let mut previous = 0.0;
        for event in &self.events {
            if event.t.is_nan() || event.t < previous {
                return Err(format!(
                    "event times must be non-negative and in order: {} after {previous}",
                    event.t
                ));
            }
            previous = event.t;
            if !(-1..=1).contains(&event.fwd) || !(-1..=1).contains(&event.right) {
                return Err(format!(
                    "fwd and right are -1, 0 or 1: event at {}",
                    event.t
                ));
            }
        }
        if !self.camera.is_empty() && self.format != CAMERA_TRACK_FORMAT {
            return Err(format!("a camera track needs format {CAMERA_TRACK_FORMAT}"));
        }
        let with_pitch = self.camera.first().is_some_and(|s| s.pitch.is_some());
        let mut previous = 0.0;
        for sample in &self.camera {
            if sample.t.is_nan() || sample.t < previous {
                return Err(format!(
                    "camera sample times must be non-negative and in order: {} after {previous}",
                    sample.t
                ));
            }
            previous = sample.t;
            if !sample.yaw.is_finite() {
                return Err(format!(
                    "camera yaw is not a number: sample at {}",
                    sample.t
                ));
            }
            match sample.pitch {
                Some(pitch) if with_pitch && (-90.0..=90.0).contains(&pitch) => (),
                None if !with_pitch => (),
                _ => {
                    return Err(format!(
                        "camera pitch is in [-90, 90] on every sample or on none: sample at {}",
                        sample.t
                    ))
                }
            }
        }
        Ok(())
    }

    /// The camera track's pose at `t`, interpolated the short way round between the samples
    /// around it and held outside them; `None` without a track.
    pub fn camera_at(&self, t: f64) -> Option<CameraPose> {
        let pose = |s: &CameraSample| CameraPose {
            yaw: normalize_yaw(s.yaw),
            pitch: s.pitch,
        };
        let next = self.camera.iter().position(|s| s.t > t);
        match next {
            None => self.camera.last().map(pose),
            Some(0) => self.camera.first().map(pose),
            Some(i) => {
                let (a, b) = (&self.camera[i - 1], &self.camera[i]);
                let f = (t - a.t) / (b.t - a.t);
                Some(CameraPose {
                    yaw: normalize_yaw(a.yaw + yaw_difference(b.yaw, a.yaw) * f),
                    pitch: a.pitch.zip(b.pitch).map(|(a, b)| a + (b - a) * f),
                })
            }
        }
    }

    /// The keys held at `t`: the last event reached, else none.
    pub fn input_at(&self, t: f64) -> InputStep {
        self.events
            .iter()
            .rev()
            .find(|event| t >= event.t - EVENT_EPSILON)
            .map(InputStep::from)
            .unwrap_or_default()
    }

    /// Whether any event holds the jump key. Only such a scenario presses it, compares it and
    /// writes it in a frame's input, so a trace without jumps keeps its earlier form.
    pub fn uses_jump(&self) -> bool {
        self.events.iter().any(|event| event.jump)
    }

    /// Whether the frame at `t` is the last one recorded.
    pub fn ends_at(&self, t: f64) -> bool {
        t >= self.duration - EVENT_EPSILON
    }
}

// --- input ---

/// The movement input the scenes were given this frame, as the movement code reads it: a
/// diagonal is normalised, and the walk and sprint modifiers count only with a direction held.
/// The jump key counts with or without a direction.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct ConsumedInput {
    pub fwd: f32,
    pub right: f32,
    pub gait: Gait,
    pub jump: bool,
}

impl ConsumedInput {
    pub fn from_actions(
        forward: bool,
        backward: bool,
        left: bool,
        right: bool,
        walk: bool,
        sprint: bool,
    ) -> Self {
        let fwd = forward as i8 - backward as i8;
        let side = right as i8 - left as i8;
        let scale = if fwd != 0 && side != 0 {
            FRAC_1_SQRT_2
        } else {
            1.0
        };
        Self {
            fwd: fwd as f32 * scale,
            right: side as f32 * scale,
            gait: Gait::of(fwd != 0 || side != 0, walk, sprint),
            jump: false,
        }
    }

    pub fn with_jump(self, jump: bool) -> Self {
        Self { jump, ..self }
    }

    pub fn matches(&self, expected: &InputStep) -> bool {
        let sign = |v: f32| (v > 0.5) as i8 - (v < -0.5) as i8;
        sign(self.fwd) == expected.fwd
            && sign(self.right) == expected.right
            && self.gait == expected.gait()
            && self.jump == expected.jump
    }

    /// The frame's `in`: `fwd`, `right`, `walk` and `sprint`, and `jump` when the scenario jumps.
    pub fn to_json(&self, with_jump: bool) -> String {
        let jump = match (with_jump, self.jump) {
            (false, _) => "",
            (true, false) => ",\"jump\":false",
            (true, true) => ",\"jump\":true",
        };
        format!(
            "{{\"fwd\":{},\"right\":{},\"walk\":{},\"sprint\":{}{jump}}}",
            num(self.fwd as f64),
            num(self.right as f64),
            self.gait == Gait::Walk,
            self.gait == Gait::Run,
        )
    }
}

/// The keys bound to the movement actions, first keyboard binding of each. Only a scenario that
/// jumps needs a key bound to jump.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MovementKeys {
    forward: KeyCode,
    backward: KeyCode,
    left: KeyCode,
    right: KeyCode,
    walk: KeyCode,
    sprint: KeyCode,
    jump: Option<KeyCode>,
}

impl MovementKeys {
    pub fn from_map(map: &InputMap) -> Result<Self, String> {
        let bound = |action: CommonInputAction| {
            map.inputs.get(&Action::Scene(action)).and_then(|ids| {
                ids.iter().find_map(|id| match id {
                    InputIdentifier::Key(k) => Some(*k),
                    _ => None,
                })
            })
        };
        let key = |action: CommonInputAction| {
            bound(action).ok_or_else(|| format!("no key is bound to {action:?}"))
        };
        Ok(Self {
            forward: key(CommonInputAction::IaForward)?,
            backward: key(CommonInputAction::IaBackward)?,
            left: key(CommonInputAction::IaLeft)?,
            right: key(CommonInputAction::IaRight)?,
            walk: key(CommonInputAction::IaWalk)?,
            sprint: key(CommonInputAction::IaModifier)?,
            jump: bound(CommonInputAction::IaJump),
        })
    }

    /// These keys, refused when the scenario jumps and no key is bound to jump.
    pub fn for_scenario(self, scenario: &Scenario) -> Result<Self, String> {
        if scenario.uses_jump() && self.jump.is_none() {
            return Err(format!(
                "no key is bound to {:?}",
                CommonInputAction::IaJump
            ));
        }
        Ok(self)
    }

    pub fn pressed(&self, step: &InputStep) -> Vec<KeyCode> {
        let mut keys = Vec::new();
        match step.fwd {
            1 => keys.push(self.forward),
            -1 => keys.push(self.backward),
            _ => (),
        }
        match step.right {
            1 => keys.push(self.right),
            -1 => keys.push(self.left),
            _ => (),
        }
        if step.walk {
            keys.push(self.walk);
        }
        if step.sprint {
            keys.push(self.sprint);
        }
        if let (true, Some(jump)) = (step.jump, self.jump) {
            keys.push(jump);
        }
        keys
    }
}

/// The logical key a keyboard reports with these codes; others are unidentified, which no
/// binding reads.
fn logical_key(code: KeyCode) -> Key {
    let letter = format!("{code:?}")
        .strip_prefix("Key")
        .filter(|l| l.len() == 1)
        .map(str::to_ascii_lowercase);
    if let Some(letter) = letter {
        return Key::Character(letter.as_str().into());
    }
    match code {
        KeyCode::ShiftLeft | KeyCode::ShiftRight => Key::Shift,
        KeyCode::ControlLeft | KeyCode::ControlRight => Key::Control,
        KeyCode::AltLeft | KeyCode::AltRight => Key::Alt,
        KeyCode::Space => Key::Space,
        KeyCode::ArrowUp => Key::ArrowUp,
        KeyCode::ArrowDown => Key::ArrowDown,
        KeyCode::ArrowLeft => Key::ArrowLeft,
        KeyCode::ArrowRight => Key::ArrowRight,
        _ => Key::Unidentified(NativeKey::Unidentified),
    }
}

// --- geometry ---

/// Yaw in the SDK's convention, degrees clockwise from north (+z) in (-180, 180], of something
/// that faces its local -z, as the player and the camera do.
pub fn sdk_yaw(rotation: Quat) -> f64 {
    let forward = rotation * Vec3::NEG_Z;
    normalize_yaw((forward.x as f64).atan2(-forward.z as f64).to_degrees())
}

fn normalize_yaw(yaw: f64) -> f64 {
    let yaw = yaw.rem_euclid(360.0);
    if yaw > 180.0 {
        yaw - 360.0
    } else {
        yaw
    }
}

fn yaw_difference(a: f64, b: f64) -> f64 {
    normalize_yaw(a - b)
}

/// The world direction an SDK yaw faces.
fn yaw_direction(yaw: f64) -> Vec3 {
    let yaw = yaw.to_radians();
    Vec3::new(yaw.sin() as f32, 0.0, -yaw.cos() as f32)
}

fn base_offset(base: IVec2) -> Vec3 {
    Vec3::new(base.x as f32, 0.0, base.y as f32) * PARCEL_SIZE
}

/// An engine translation in SDK axes relative to a scene's base parcel.
pub fn scene_position(translation: Vec3, base: IVec2) -> Vec3 {
    Vec3::new(translation.x, translation.y, -translation.z) - base_offset(base)
}

/// The engine translation of an SDK position relative to a scene's base parcel.
pub fn world_translation(position: Vec3, base: IVec2) -> Vec3 {
    let world = position + base_offset(base);
    Vec3::new(world.x, world.y, -world.z)
}

fn horizontal_speed(v: Option<Vector3>) -> f32 {
    v.map_or(0.0, |v| Vec2::new(v.x, v.z).length())
}

// --- formatting ---

/// The movement scene's tick timing in a frame's `extra`, each key followed by a comma:
/// `scene_sent` (ms after the frame's scene loop started that the scene was dispatched; null when
/// it was not dispatched this frame), `scene_reply_late` (for the reply taken this frame, ms
/// after its dispatch frame's deadline that the engine took it, negative before it; null without
/// one), `scene_reply_frames` (frames from that reply's dispatch to this frame), and `stage`:
///
/// - the frame, ms on the shared clock: `f0` (frame start), `S` (scene loop start), `D`
///   (its deadline), `E` (its end), `prev_f1` (the previous frame's end, for its Q); ms:
///   `B` (D - S), `I` (S - the previous S), `P` (S - f0); `end` (`deadline` or `idle`:
///   no scene eligible and none in flight), `eligible_left`, `in_flight` (at the loop's end),
///   `dispatched` (scenes dispatched in the loop), `runs` (scene schedule runs), `polls`,
///   `heap_js_mb` (`performance.memory.usedJSHeapSize` when the worker has it), `wasm_mb`,
///   `mode` (the scene loop mode), `K` (the deadline mode's reserve for the frame's tail, ms),
///   `relay_waits` and `relay_sleeps` (every relay's steps since startup that waited for the
///   engine's send and that slept; null natively),
///   and the previous frame's `prev_wait_render`, `prev_wait_handoff`, `prev_main_update`
///   (`pipelined/*`, ms) and `prev_handoff_end` (its app update's end, shared clock; its tail
///   after the loop is this minus its `E`);
/// - `sent`: the movement scene's dispatch this frame, `seq` (its number) and `at`;
/// - `reply`: the movement scene's reply taken this frame: `seq` of its dispatch, `at`,
///   `first_poll`, the dispatch's `sent_frame`, `sent_at`, `sent_S`, `sent_D`, and
///   `relay` (`dcl_wasm::tick_marks::RELAY_NAMES`; null when the relay published none).
fn tick_timing_json(stage: &FrameStages, frame: u32) -> String {
    let ms = |v: f64| num3(v);
    let sent = stage.sent.filter(|sent| sent.frame == frame);
    let reply = stage.reply.filter(|reply| reply.frame == frame);
    let relay = reply.and_then(|r| r.relay).map_or("null".into(), |fields| {
        let mut s = String::from("{");
        for (i, (name, value)) in RELAY_NAMES.iter().zip(fields).enumerate() {
            if i > 0 {
                s.push(',');
            }
            s.push_str(&format!("\"{name}\":{}", ms(value)));
        }
        s.push('}');
        s
    });
    let opt = |v: Option<f64>| v.map_or("null".into(), ms);
    format!(
        concat!(
            "\"scene_sent\":{},\"scene_reply_late\":{},\"scene_reply_frames\":{},",
            "\"stage\":{{\"f0\":{},\"S\":{},\"D\":{},\"E\":{},\"prev_f1\":{},\"B\":{},\"I\":{},",
            "\"P\":{},\"end\":\"{}\",\"eligible_left\":{},\"in_flight\":{},\"dispatched\":{},",
            "\"runs\":{},\"polls\":{},\"heap_js_mb\":{},\"wasm_mb\":{},\"mode\":\"{}\",\"K\":{},",
            "\"relay_waits\":{},\"relay_sleeps\":{},",
            "\"prev_wait_render\":{},\"prev_wait_handoff\":{},\"prev_main_update\":{},",
            "\"prev_handoff_end\":{},\"sent\":{},\"reply\":{}}},"
        ),
        sent.map_or("null".into(), |s| ms(s.at - s.loop_start)),
        reply.map_or("null".into(), |r| ms(r.at - r.sent.deadline)),
        reply.map_or("null".into(), |r| r
            .frame
            .wrapping_sub(r.sent.frame)
            .to_string()),
        ms(stage.start),
        ms(stage.loop_start),
        ms(stage.deadline),
        ms(stage.loop_end),
        ms(stage.prev_end),
        ms(stage.deadline - stage.loop_start),
        ms(stage.interval),
        ms(stage.loop_start - stage.start),
        stage.end.name(),
        stage.eligible_left,
        stage.in_flight_at_end,
        stage.dispatched,
        stage.schedule_runs,
        stage.polls,
        opt(stage.heap_js_mb),
        opt(stage.wasm_mb),
        stage.mode,
        opt(stage.tail_reserve),
        stage.relay_waits.map_or("null".into(), |v| v.to_string()),
        stage.relay_sleeps.map_or("null".into(), |v| v.to_string()),
        opt(stage.prev_wait_render),
        opt(stage.prev_wait_handoff),
        opt(stage.prev_main_update),
        opt(stage.prev_handoff_end),
        sent.map_or("null".into(), |s| format!(
            "{{\"seq\":{},\"at\":{}}}",
            s.seq,
            ms(s.at)
        )),
        reply.map_or("null".into(), |r| format!(
            concat!(
                "{{\"seq\":{},\"at\":{},\"first_poll\":{},\"sent_frame\":{},",
                "\"sent_at\":{},\"sent_S\":{},\"sent_D\":{},\"relay\":{}}}"
            ),
            r.sent.seq,
            ms(r.at),
            r.first_poll,
            r.sent.frame,
            ms(r.sent.at),
            ms(r.sent.loop_start),
            ms(r.sent.deadline),
            relay,
        )),
    )
}

/// A millisecond value to the microsecond; null when not finite.
fn num3(v: f64) -> String {
    if v.is_finite() {
        format!("{v:.3}")
    } else {
        "null".into()
    }
}

fn num(v: f64) -> String {
    if !v.is_finite() {
        return "null".into();
    }
    let s = format!("{v:.6}");
    let s = s.trim_end_matches('0');
    let s = if s.ends_with('.') {
        format!("{s}0")
    } else {
        s.to_owned()
    };
    if s == "-0.0" {
        "0.0".into()
    } else {
        s
    }
}

fn vector(v: Vec3) -> String {
    format!(
        "[{},{},{}]",
        num(v.x as f64),
        num(v.y as f64),
        num(v.z as f64)
    )
}

fn sdk_vector(v: Option<Vector3>) -> String {
    let v = v.unwrap_or_default();
    vector(Vec3::new(v.x, v.y, v.z))
}

/// The movement model's state from a scene log line (see [`LOCOMOTION_LINE`]), as compact JSON;
/// `None` for any other line. The log keeps `console.log`'s level, `LOG `; the native runtime
/// writes a string argument JSON-quoted, the web one as it is.
fn locomotion_line(message: &str) -> Option<String> {
    let text = message.strip_prefix("LOG ")?;
    let unquoted;
    let text = if text.starts_with('"') {
        unquoted = serde_json::from_str::<String>(text).ok()?;
        unquoted.as_str()
    } else {
        text
    };
    let state: serde_json::Value =
        serde_json::from_str(text.strip_prefix(LOCOMOTION_LINE)?).ok()?;
    state.is_object().then(|| state.to_string())
}

fn utc_now() -> String {
    let now = web_time::SystemTime::now()
        .duration_since(web_time::UNIX_EPOCH)
        .unwrap_or_default();
    chrono::DateTime::from_timestamp(now.as_secs() as i64, now.subsec_nanos())
        .map(|t| t.to_rfc3339_opts(chrono::SecondsFormat::Millis, true))
        .unwrap_or_default()
}

// --- the run ---

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum InputSource {
    /// The run presses the scenario's keys
    Keys(MovementKeys),
    /// The run waits for movement input from outside, and starts on it
    External,
}

/// When a run's t = 0 comes after the placement.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum StartCondition {
    /// Once the player has stood still, grounded and at the spawn for the scenario's `settle`;
    /// placed again when it does not stay there
    #[default]
    Still,
    /// The first frame resting on ground or a steep slope at least `settle` after the placement,
    /// or the first frame [`GROUNDED_WAIT`] later, wherever the player is: it may be sliding or
    /// falling
    Placed,
}

impl StartCondition {
    fn name(&self) -> &'static str {
        match self {
            StartCondition::Still => "still",
            StartCondition::Placed => "placed",
        }
    }
}

/// Who turns the camera during a run.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum CameraControl {
    /// The run: at the scenario's `camera_yaw`, or along its camera track
    #[default]
    Held,
    /// Whoever plays; each frame records the camera's yaw and pitch
    Free,
}

/// Whether a `--start placed` run starts in a frame `since_placed` seconds after the placement.
/// The placement's own frame never does: its ground state may still be the old position's.
/// `landed`: the player rests on ground or on a slope too steep to stand on.
fn placed_start(since_placed: f64, settle: f64, landed: bool) -> bool {
    since_placed > 0.0
        && since_placed >= settle
        && (landed || since_placed >= settle + GROUNDED_WAIT)
}

#[derive(Debug, PartialEq)]
enum Phase {
    /// The player is placed in the next frame's `PreUpdate`
    Spawning,
    Settling {
        placed_at: f64,
        still_since: Option<f64>,
    },
    /// Settled: the next frame is t = 0
    Starting,
    /// Settled: t = 0 is the first frame with movement input
    Armed {
        since: f64,
    },
    Running {
        t0: f64,
    },
}

impl Phase {
    fn name(&self) -> &'static str {
        match self {
            Phase::Spawning => "spawning",
            Phase::Settling { .. } => "settling",
            Phase::Starting => "starting",
            Phase::Armed { .. } => "armed",
            Phase::Running { .. } => "running",
        }
    }
}

struct Run {
    scenario: Scenario,
    fps: u32,
    repeat: u32,
    input: InputSource,
    start: StartCondition,
    camera: CameraControl,
    fixed_dt: bool,
    /// The session frame-rate setting to put back afterwards
    previous_fps: Option<usize>,
    base: IVec2,
    spawn: Vec3,
    spawn_yaw: f64,
    realm: String,
    scene: String,
    client_version: String,
    animations: Option<bool>,
    camera_mode: &'static str,
    phase: Phase,
    spawns: u32,
    settle_started: f64,
    started_utc: String,
    /// this frame's input as read in `Update`
    consumed: Option<ConsumedInput>,
    /// the frame lines not yet handed out by `drain`
    frames: Vec<String>,
    /// frames `drain` handed out
    drained: usize,
    /// whether `drain` handed out the header
    header_drained: bool,
    dropped: u32,
    mismatches: u32,
    scene_ticks: u32,
    /// the log of the scene controlling movement, read for its movement model's state
    scene_log: Option<(Entity, RingBufferReceiver<SceneLogMessage>)>,
    /// the latest movement model state that scene printed (JSON), `None` before the first
    locomotion: Option<String>,
    /// where the player stood when the run started
    start_position: Option<Vec3>,
    last_position: Vec3,
    last_t: f64,
}

impl Run {
    fn position_label(&self) -> String {
        format!(
            "{} scene \"{}\" base {},{} spawn {},{},{} yaw {}",
            self.realm,
            self.scene,
            self.base.x,
            self.base.y,
            num(self.spawn.x as f64),
            num(self.spawn.y as f64),
            num(self.spawn.z as f64),
            num(self.spawn_yaw),
        )
    }

    /// The trace: a header line, the frames, a footer line.
    ///
    /// - header: `type`, `format`, `client` (`bevy`), `client_version`, `scenario` (its id),
    ///   `seed`, `fps_target`, `animations` (whether the player's own avatar animated), `position`
    ///   (realm, scene, base parcel and spawn), `started_utc` (t = 0) and `extra`: the repeat,
    ///   the scenario's `camera_yaw`, `settle` and `duration`, `input_seam` (`keyboard`: keys
    ///   pressed by the run; `external`: keys from outside), `dt_mode` (`real`, or `fixed` at
    ///   1/fps), `camera_mode`, `scene_base_parcel`, `spawn`, `spawn_yaw`, `start_pos` (where
    ///   the player stood before t = 0) and, for a run with `--start placed`, `start`
    ///   (`placed`: the player need not have been still or at the spawn at t = 0).
    /// - frame: `i`, `t` and `dt` (s, the frame clock), `in` (the movement input given this
    ///   frame: `fwd` and `right`, normalised on a diagonal, `walk` and `sprint` from the gait,
    ///   and `jump` when the scenario has a jump),
    ///   `pos` (after this frame's movement), `vel` (this frame's displacement over its delta),
    ///   `grounded` (the feet within 0.05 m of ground the player can stand on: not a wall it
    ///   grazes or a slope steeper than 46.5 degrees), `yaw_avatar`, `yaw_camera`, and `extra`:
    ///   the `kind` (gait), `vel_requested`
    ///   (by the movement scene), `vel_external` (SDK forces and impulses), `scene_tick` (1
    ///   when the controlling scene's reply landed this frame, 0 when the frame reused its last
    ///   one), `scene_sent`, `scene_reply_late` and `scene_reply_frames` (the scene's dispatch
    ///   and reply times against the frame's deadline, [`tick_timing_json`]) and `locomotion`
    ///   (the latest movement model state the controlling scene printed in a
    ///   `LOCO {...}` console line, as printed; `null` before the first).
    /// - footer: `frames`, `dropped` (frames after the first longer than 1.5 target intervals)
    ///   and `extra.input_mismatch_frames` (frames whose input differs from the scenario's) and
    ///   `extra.scene_tick_frames`.
    ///
    /// A run whose camera is free or follows a camera track adds `extra.camera_control` (`free`
    /// or `track`) to the header and `extra.cam` (`yaw` and `pitch`, the camera's own angles this
    /// frame, set by whoever turned it or by the track) to each frame; a free run writes `in.jump`
    /// on every frame. Lines `drain` handed out are not written again.
    fn finish(&self) -> String {
        let footer = format!(
            "{{\"type\":\"footer\",\"frames\":{},\"dropped\":{},\"extra\":{{\"input_mismatch_frames\":{},\"scene_tick_frames\":{}}}}}",
            self.frame_count(),
            self.dropped,
            self.mismatches,
            self.scene_ticks,
        );
        let mut text = String::with_capacity(400 * (self.frames.len() + 2));
        if !self.header_drained {
            text.push_str(&self.header());
            text.push('\n');
        }
        for line in self.frames.iter().chain(std::iter::once(&footer)) {
            text.push_str(line);
            text.push('\n');
        }
        text
    }

    /// The lines recorded since the last drain, the header first; empty before t = 0.
    fn drain(&mut self) -> String {
        if !matches!(self.phase, Phase::Running { .. }) {
            return String::new();
        }
        let mut text = String::with_capacity(400 * (self.frames.len() + 1));
        if !self.header_drained {
            text.push_str(&self.header());
            text.push('\n');
            self.header_drained = true;
        }
        self.drained += self.frames.len();
        for line in self.frames.drain(..) {
            text.push_str(&line);
            text.push('\n');
        }
        text
    }

    /// Frames recorded so far, drained or not.
    fn frame_count(&self) -> usize {
        self.drained + self.frames.len()
    }

    /// Whether frames carry the camera's angles: when the run does not hold it at one yaw.
    fn records_camera(&self) -> bool {
        self.camera == CameraControl::Free || !self.scenario.camera.is_empty()
    }

    fn header(&self) -> String {
        let s = &self.scenario;
        let animations = match self.animations {
            Some(enabled) => enabled.to_string(),
            None => "null".into(),
        };
        format!(
            concat!(
                "{{\"type\":\"header\",\"format\":{},\"client\":\"bevy\",\"client_version\":{},",
                "\"scenario\":{},\"seed\":{},\"fps_target\":{},\"animations\":{},\"position\":{},",
                "\"started_utc\":{},\"extra\":{{\"repeat\":{},\"camera_yaw\":{},\"settle\":{},",
                "\"duration\":{},\"input_seam\":\"{}\",\"dt_mode\":\"{}\",\"camera_mode\":\"{}\",",
                "\"scene_base_parcel\":[{},{}],\"spawn\":{},\"spawn_yaw\":{},\"start_pos\":{}{}{}}}}}"
            ),
            FORMAT,
            serde_json::Value::from(self.client_version.as_str()),
            serde_json::Value::from(s.id.as_str()),
            s.seed,
            self.fps,
            animations,
            serde_json::Value::from(self.position_label()),
            serde_json::Value::from(self.started_utc.as_str()),
            self.repeat,
            num(s.camera_yaw),
            num(s.settle),
            num(s.duration),
            match self.input {
                InputSource::Keys(_) => "keyboard",
                InputSource::External => "external",
            },
            if self.fixed_dt { "fixed" } else { "real" },
            self.camera_mode,
            self.base.x,
            self.base.y,
            vector(self.spawn),
            num(self.spawn_yaw),
            self.start_position.map_or("null".into(), vector),
            // the earlier header form unless the run started on placement
            match self.start {
                StartCondition::Still => "",
                StartCondition::Placed => ",\"start\":\"placed\"",
            },
            // the earlier header form while the camera is held at one yaw
            match (self.camera, s.camera.is_empty()) {
                (CameraControl::Free, _) => ",\"camera_control\":\"free\"",
                (CameraControl::Held, false) => ",\"camera_control\":\"track\"",
                (CameraControl::Held, true) => "",
            },
        )
    }

    fn file_name(&self) -> String {
        format!(
            "{}__fps{}__r{}.jsonl",
            self.scenario.id, self.fps, self.repeat
        )
    }

    fn summary(&self) -> serde_json::Value {
        let distance = self
            .start_position
            .map(|first| (self.last_position - first) * Vec3::new(1.0, 0.0, 1.0))
            .map(|d| d.length());
        serde_json::json!({
            "scenario": self.scenario.id,
            "repeat": self.repeat,
            "fps": self.fps,
            "dtMode": if self.fixed_dt { "fixed" } else { "real" },
            "frames": self.frame_count(),
            "t": self.last_t,
            "dropped": self.dropped,
            "inputMismatchFrames": self.mismatches,
            "sceneTickFrames": self.scene_ticks,
            "distance": distance,
            "file": self.file_name(),
        })
    }
}

struct Finished {
    text: String,
    summary: serde_json::Value,
}

/// The recorder's state: the run in progress, the last run's result and the keys it holds.
#[derive(Resource, Default)]
pub struct MovementTrace {
    run: Option<Run>,
    last: Option<Result<Finished, String>>,
    /// keys pressed by the run and not yet released
    held: Vec<KeyCode>,
}

impl MovementTrace {
    fn status(&self) -> serde_json::Value {
        let (phase, error) = match (&self.run, &self.last) {
            (Some(run), _) => (run.phase.name(), None),
            (None, Some(Ok(_))) => ("done", None),
            (None, Some(Err(e))) => ("failed", Some(e.clone())),
            (None, None) => ("idle", None),
        };
        let mut status = serde_json::json!({ "phase": phase, "error": error });
        let summary = match (&self.run, &self.last) {
            (Some(run), _) => Some(run.summary()),
            (None, Some(Ok(finished))) => Some(finished.summary.clone()),
            _ => None,
        };
        if let (Some(summary), Some(status)) = (summary, status.as_object_mut()) {
            if let Some(summary) = summary.as_object() {
                status.extend(summary.clone());
            }
        }
        status["unavailable"] = serde_json::json!({
            "": "no run yet: /movement_trace play <scenario>",
            "error": "no error",
        });
        status
    }

    /// Ends the run in progress, keeping `result` as the last one, and puts the session's frame
    /// rate and clock back. The keys are released in the next frame's `PreUpdate`.
    fn end(
        &mut self,
        result: Result<(), String>,
        session: Option<&mut SessionGraphics>,
        strategy: Option<&mut TimeUpdateStrategy>,
    ) {
        let Some(run) = self.run.take() else {
            return;
        };
        if let Some(session) = session {
            session.fps_target = run.previous_fps;
        }
        if let (true, Some(strategy)) = (run.fixed_dt, strategy) {
            *strategy = TimeUpdateStrategy::Automatic;
        }
        self.last = Some(result.map(|()| Finished {
            text: run.finish(),
            summary: run.summary(),
        }));
    }
}

// --- console ---

/// Play a movement scenario on the player and record each frame (`play`), report the run
/// (`status`), return the lines recorded since the last drain while it runs (`drain`), end it
/// now and keep its trace (`finish`), return the last trace as JSON Lines, less what was
/// drained (`fetch`), or abandon the run (`stop`). `play` answers once the run has started; poll
/// `status` until `done` or `failed`
#[derive(clap::Parser, ConsoleCommand)]
#[command(name = "/movement_trace")]
struct MovementTraceCommand {
    /// `play`, `status`, `drain`, `finish`, `fetch` or `stop`
    action: String,
    /// play: the frame rate, instead of the scenario's
    #[arg(long)]
    fps: Option<u32>,
    /// play: `real` frame times, or `fixed` (each frame advances the clock by exactly 1/fps)
    #[arg(long)]
    dt: Option<String>,
    /// play: `x,y,z[,yaw]` relative to the base parcel of the scene the player is in; yaw defaults
    /// to the scenario's camera yaw. Without it the player is placed where it stands
    #[arg(long, allow_hyphen_values = true)]
    spawn: Option<String>,
    /// play: the repeat number the trace carries (1 by default)
    #[arg(long)]
    repeat: Option<u32>,
    /// play: `keys` (the run presses the scenario's keys) or `external` (the run starts on the
    /// first movement input from outside)
    #[arg(long)]
    input: Option<String>,
    /// play: `still` (t = 0 once the player has stood still, grounded and at the spawn for the
    /// scenario's `settle`) or `placed` (t = 0 on the first frame resting on ground or a steep
    /// slope `settle` seconds after the placement, wherever the player is then)
    #[arg(long)]
    start: Option<String>,
    /// play: `hold` (the run turns the camera: to the scenario's `camera_yaw`, or along its
    /// camera track) or `free` (whoever plays turns it; each frame records its yaw and pitch, and
    /// the player faces as it does now unless `--spawn` gives a yaw)
    #[arg(long)]
    camera: Option<String>,
    /// play: the scenario as JSON
    #[arg(trailing_var_arg = true, allow_hyphen_values = true, num_args = 0..)]
    json: Vec<String>,
}

fn parse_spawn(value: &str) -> Result<(Vec3, Option<f64>), String> {
    let parts = value
        .split(',')
        .map(|p| p.trim().parse::<f64>())
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| format!("spawn `{value}` is not x,y,z[,yaw]"))?;
    match parts[..] {
        [x, y, z] => Ok((Vec3::new(x as f32, y as f32, z as f32), None)),
        [x, y, z, yaw] => Ok((Vec3::new(x as f32, y as f32, z as f32), Some(yaw))),
        _ => Err(format!("spawn `{value}` is not x,y,z[,yaw]")),
    }
}

#[allow(clippy::too_many_arguments)]
fn movement_trace_command(
    mut cmd: ConsoleCommand<MovementTraceCommand>,
    mut trace: ResMut<MovementTrace>,
    map: Res<InputMap>,
    time: Res<Time>,
    player: Query<(Entity, &Transform, Has<OutOfWorld>), With<PrimaryUser>>,
    camera: Query<&PrimaryCamera>,
    containing: ContainingScene,
    scenes: Query<&RendererSceneContext>,
    realm: Res<CurrentRealm>,
    version: Option<Res<Version>>,
    mut session: Option<ResMut<SessionGraphics>>,
    mut strategy: Option<ResMut<TimeUpdateStrategy>>,
) {
    let Some(Ok(command)) = cmd.take() else {
        return;
    };
    match command.action.as_str() {
        "status" => cmd.reply_ok(trace.status().to_string()),
        "fetch" => match &trace.last {
            Some(Ok(finished)) => cmd.reply_ok(finished.text.clone()),
            Some(Err(e)) => cmd.reply_failed(format!("the last run failed: {e}")),
            None => cmd.reply_failed("no finished run"),
        },
        "drain" => match trace.run.as_mut() {
            Some(run) => cmd.reply_ok(run.drain()),
            None => cmd.reply_failed("no run in progress: fetch the rest of the last trace"),
        },
        "finish" => match trace.run.as_ref().map(|run| run.phase.name()) {
            Some("running") => {
                trace.end(Ok(()), session.as_deref_mut(), strategy.as_deref_mut());
                cmd.reply_ok("finished");
            }
            Some(phase) => cmd.reply_failed(format!(
                "the run is {phase}, not running: /movement_trace stop abandons it"
            )),
            None => cmd.reply_failed("no run in progress"),
        },
        "stop" => {
            if trace.run.is_some() {
                trace.end(
                    Err("stopped".into()),
                    session.as_deref_mut(),
                    strategy.as_deref_mut(),
                );
                cmd.reply_ok("stopped");
            } else {
                cmd.reply_ok("no run in progress");
            }
        }
        "play" => {
            if let Some(run) = &trace.run {
                cmd.reply_failed(format!(
                    "a run is {}: /movement_trace stop first",
                    run.phase.name()
                ));
                return;
            }
            let result = (|| {
                let scenario = Scenario::parse(&command.json.join(" "))?;
                let fps = command.fps.unwrap_or(scenario.fps);
                if !(1..=1000).contains(&fps) {
                    return Err(format!("fps {fps} is outside 1..=1000"));
                }
                let fixed_dt = match command.dt.as_deref() {
                    None | Some("real") => false,
                    Some("fixed") => true,
                    Some(other) => return Err(format!("dt `{other}`: use real or fixed")),
                };
                let input = match command.input.as_deref() {
                    None | Some("keys") => {
                        InputSource::Keys(MovementKeys::from_map(&map)?.for_scenario(&scenario)?)
                    }
                    Some("external") => InputSource::External,
                    Some(other) => return Err(format!("input `{other}`: use keys or external")),
                };
                let start = match command.start.as_deref() {
                    None | Some("still") => StartCondition::Still,
                    Some("placed") => StartCondition::Placed,
                    Some(other) => return Err(format!("start `{other}`: use still or placed")),
                };
                let camera_control = match command.camera.as_deref() {
                    None | Some("hold") => CameraControl::Held,
                    Some("free") if !scenario.camera.is_empty() => {
                        return Err("the scenario's camera track turns the camera: use hold".into())
                    }
                    Some("free") => CameraControl::Free,
                    Some(other) => return Err(format!("camera `{other}`: use hold or free")),
                };
                let (player, transform, out_of_world) =
                    player.single().map_err(|_| "no player".to_string())?;
                if out_of_world {
                    return Err("the player is not in the world yet".into());
                }
                // the parcel scene the player stands in; an empty parcel is its own base
                let scene = containing
                    .get(player)
                    .into_iter()
                    .filter_map(|e| scenes.get(e).ok())
                    .find(|ctx| !ctx.is_portable);
                let (base, scene_title) = match scene {
                    Some(ctx) => (ctx.base, ctx.title.clone()),
                    None => (
                        (Vec2::new(transform.translation.x, -transform.translation.z)
                            / PARCEL_SIZE)
                            .floor()
                            .as_ivec2(),
                        "empty parcel".to_string(),
                    ),
                };
                let (spawn, spawn_yaw) = match command.spawn.as_deref().map(parse_spawn) {
                    Some(parsed) => parsed?,
                    None => (scene_position(transform.translation, base), None),
                };
                let camera_mode = match camera.single() {
                    Ok(camera) if camera.distance < 0.05 => "first_person",
                    Ok(_) => "third_person",
                    Err(_) => "none",
                };
                // a free camera leaves the player facing as it does; a held one faces it the
                // camera's way at t = 0
                let default_yaw = match camera_control {
                    CameraControl::Free => sdk_yaw(transform.rotation),
                    CameraControl::Held => scenario
                        .camera_at(0.0)
                        .map_or(scenario.camera_yaw, |pose| pose.yaw),
                };
                Ok(Run {
                    spawn_yaw: normalize_yaw(spawn_yaw.unwrap_or(default_yaw)),
                    scenario,
                    fps,
                    repeat: command.repeat.unwrap_or(1),
                    input,
                    start,
                    camera: camera_control,
                    fixed_dt,
                    previous_fps: session.as_ref().and_then(|s| s.fps_target),
                    base,
                    spawn,
                    realm: realm
                        .config
                        .realm_name
                        .clone()
                        .unwrap_or_else(|| realm.address.clone()),
                    scene: scene_title,
                    client_version: version.as_ref().map(|v| v.0.clone()).unwrap_or_default(),
                    // the own-avatar animation switch is not on this branch (b8f2622a6 not reverted)
                    animations: None,
                    camera_mode,
                    phase: Phase::Spawning,
                    spawns: 0,
                    settle_started: time.elapsed_secs_f64(),
                    started_utc: String::new(),
                    consumed: None,
                    frames: Vec::new(),
                    drained: 0,
                    header_drained: false,
                    dropped: 0,
                    mismatches: 0,
                    scene_ticks: 0,
                    scene_log: None,
                    locomotion: None,
                    start_position: None,
                    last_position: Vec3::ZERO,
                    last_t: 0.0,
                })
            })();
            match result {
                Ok(run) => {
                    if let Some(session) = session.as_deref_mut() {
                        session.fps_target = Some(run.fps as usize);
                    }
                    if let (true, Some(strategy)) = (run.fixed_dt, strategy.as_deref_mut()) {
                        *strategy = TimeUpdateStrategy::ManualDuration(
                            std::time::Duration::from_secs_f64(1.0 / run.fps as f64),
                        );
                    }
                    let reply = serde_json::json!({
                        "started": run.scenario.id,
                        "fps": run.fps,
                        "base": [run.base.x, run.base.y],
                        "scene": run.scene,
                        "spawn": [run.spawn.x, run.spawn.y, run.spawn.z],
                        "spawnYaw": run.spawn_yaw,
                        "start": run.start.name(),
                    });
                    trace.last = None;
                    trace.run = Some(run);
                    cmd.reply_ok(reply.to_string());
                }
                Err(e) => cmd.reply_failed(e),
            }
        }
        other => cmd.reply_failed(format!(
            "unknown action `{other}`: use play, status, drain, finish, fetch or stop"
        )),
    }
}

// --- systems ---

/// Before Bevy's input systems: places the player when a run asks, starts the clock at t = 0
/// and presses or releases keys so the keys held are the scenario's at this frame's time. Keys
/// the run holds are released when no run is pressing them.
fn press_scenario_keys(
    mut trace: ResMut<MovementTrace>,
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    window: Query<Entity, With<PrimaryWindow>>,
    mut keyboard: EventWriter<KeyboardInput>,
    mut rpc: EventWriter<RpcCallEvent>,
) {
    let now = time.elapsed_secs_f64();
    let trace = &mut *trace;
    let mut desired = Vec::new();
    if let Some(run) = trace.run.as_mut() {
        match run.phase {
            Phase::Spawning => {
                let to = world_translation(run.spawn, run.base);
                rpc.write(RpcCallEvent::engine(RpcCall::MovePlayer {
                    to,
                    looking_at: Some(to + yaw_direction(run.spawn_yaw)),
                    duration: None,
                    camera_rotation: None,
                    response: None,
                }));
                run.spawns += 1;
                run.phase = Phase::Settling {
                    placed_at: now,
                    still_since: None,
                };
            }
            Phase::Starting => {
                run.phase = Phase::Running { t0: now };
                run.started_utc = utc_now();
            }
            _ => (),
        }
        if let (Phase::Running { t0 }, InputSource::Keys(movement_keys)) = (&run.phase, &run.input)
        {
            desired = movement_keys.pressed(&run.scenario.input_at(now - t0));
        }
    }

    let window = window.single().unwrap_or(Entity::PLACEHOLDER);
    let mut event = |key_code: KeyCode, state: ButtonState| {
        keyboard.write(KeyboardInput {
            key_code,
            logical_key: logical_key(key_code),
            state,
            text: None,
            repeat: false,
            window,
        });
    };
    trace.held.retain(|key| {
        let keep = desired.contains(key);
        if !keep {
            event(*key, ButtonState::Released);
        }
        keep
    });
    for key in desired {
        // pressed again if something released it, such as the window losing focus
        if !keys.pressed(key) {
            event(key, ButtonState::Pressed);
        }
        if !trace.held.contains(&key) {
            trace.held.push(key);
        }
    }
}

/// The movement input the scenes are given this frame.
fn read_consumed_input(mut trace: ResMut<MovementTrace>, input: InputManager) {
    let Some(run) = trace.run.as_mut() else {
        return;
    };
    let down = |action| input.is_down(action, InputPriority::Scene);
    run.consumed = Some(
        ConsumedInput::from_actions(
            down(CommonInputAction::IaForward),
            down(CommonInputAction::IaBackward),
            down(CommonInputAction::IaLeft),
            down(CommonInputAction::IaRight),
            down(CommonInputAction::IaWalk),
            down(CommonInputAction::IaModifier),
        )
        .with_jump(down(CommonInputAction::IaJump)),
    );
}

/// Turns the camera for the whole run, settling included, unless it is free: to the scenario's
/// yaw, or along its camera track by frame time (held at the track's start before t = 0).
fn drive_camera(trace: Res<MovementTrace>, time: Res<Time>, mut camera: Query<&mut PrimaryCamera>) {
    let Some(run) = trace.run.as_ref() else {
        return;
    };
    if run.camera == CameraControl::Free {
        return;
    }
    let t = match run.phase {
        Phase::Running { t0 } => time.elapsed_secs_f64() - t0,
        _ => 0.0,
    };
    let pose = run.scenario.camera_at(t).unwrap_or(CameraPose {
        yaw: run.scenario.camera_yaw,
        pitch: None,
    });
    if let Ok(mut camera) = camera.single_mut() {
        // the camera faces its local -z: an SDK yaw is the negated engine yaw
        camera.yaw = -(pose.yaw.to_radians() as f32);
        if let Some(pitch) = pose.pitch {
            camera.pitch = pitch.to_radians() as f32;
        }
    }
}

/// After the player's movement and the camera: settles, then records the frame.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn record_frame(
    mut trace: ResMut<MovementTrace>,
    time: Res<Time>,
    frame: Res<FrameCount>,
    player: Query<
        (
            &Transform,
            &AvatarDynamicState,
            &ActivePlayerComponent<AvatarMovement>,
            Has<OutOfWorld>,
        ),
        With<PrimaryUser>,
    >,
    camera: Query<(&Transform, &PrimaryCamera)>,
    scenes: Query<&RendererSceneContext>,
    stages: Res<TickStages>,
    mut session: Option<ResMut<SessionGraphics>>,
    mut strategy: Option<ResMut<TimeUpdateStrategy>>,
) {
    // MEASUREMENT ONLY: this frame's main-thread time of the movement systems, taken every frame
    let cpu = scene_runner::cpu_probe::peek_json();
    let now = time.elapsed_secs_f64();
    let Some(run) = trace.run.as_mut() else {
        return;
    };
    let Ok((transform, dynamics, movement, out_of_world)) = player.single() else {
        return;
    };
    if out_of_world {
        trace.end(
            Err("the player left the world".into()),
            session.as_deref_mut(),
            strategy.as_deref_mut(),
        );
        return;
    }
    let position = scene_position(transform.translation, run.base);
    let yaw_avatar = sdk_yaw(transform.rotation);
    let grounded = dynamics.ground_height <= common::dynamics::PLAYER_GROUND_THRESHOLD;
    // resting on something, ground or a slope too steep to stand on
    // this branch keeps no contact height apart from the ground's
    let landed = grounded;
    // this frame's displacement over its delta, and the movement scene's requested velocity
    // (engine axes to the SDK's: z flips)
    let dt = time.delta_secs_f64();
    let actual_velocity = (dt > 0.0).then(|| {
        let v = (position - run.last_position) / dt as f32;
        Vector3 {
            x: v.x,
            y: v.y,
            z: v.z,
        }
    });
    let v = movement.component.velocity;
    let requested_velocity = Some(Vector3 {
        x: v.x,
        y: v.y,
        z: -v.z,
    });
    run.last_position = position;
    // a scenario without jumps neither compares nor writes the jump key, unless the run records
    // free play
    let uses_jump = run.scenario.uses_jump() || run.camera == CameraControl::Free;
    let consumed = run.consumed.take().unwrap_or_default();
    let consumed = consumed.with_jump(uses_jump && consumed.jump);

    // the controlling scene's console: keep the latest movement model state it printed
    let scene = movement.scene();
    if run
        .scene_log
        .as_ref()
        .is_none_or(|(log_scene, _)| *log_scene != scene)
    {
        run.scene_log = scenes.get(scene).ok().map(|ctx| {
            let (_, backlog, receiver) = ctx.logs.read();
            if let Some(state) = backlog
                .iter()
                .rev()
                .find_map(|l| locomotion_line(&l.message))
            {
                run.locomotion = Some(state);
            }
            (scene, receiver)
        });
    }
    if let Some((_, log)) = run.scene_log.as_mut() {
        while let Ok(line) = log.try_recv() {
            if let Some(state) = locomotion_line(&line.message) {
                run.locomotion = Some(state);
            }
        }
    }

    let mut outcome = None;
    match run.phase {
        Phase::Spawning | Phase::Starting => (),
        Phase::Settling {
            placed_at,
            ref mut still_since,
        } => {
            let settled = match run.start {
                StartCondition::Placed => {
                    placed_start(now - placed_at, run.scenario.settle, landed)
                }
                StartCondition::Still => {
                    let still = grounded
                        && horizontal_speed(actual_velocity) < STILL_SPEED
                        && horizontal_speed(requested_velocity) < STILL_SPEED;
                    let in_place = (position.xz() - run.spawn.xz()).length() < SPAWN_TOLERANCE
                        && yaw_difference(yaw_avatar, run.spawn_yaw).abs() < SPAWN_YAW_TOLERANCE;
                    if still && in_place {
                        let since = *still_since.get_or_insert(now);
                        now - since >= run.scenario.settle
                    } else {
                        *still_since = None;
                        if still && now - placed_at > RESPAWN_AFTER {
                            if run.spawns >= MAX_SPAWNS {
                                outcome = Some(Err(format!(
                                    "the player does not stay at the spawn: at {} facing {}",
                                    vector(position),
                                    num(yaw_avatar)
                                )));
                            } else {
                                run.phase = Phase::Spawning;
                            }
                        }
                        false
                    }
                }
            };
            if settled {
                run.start_position = Some(position);
                run.phase = match run.input {
                    InputSource::Keys(_) => Phase::Starting,
                    InputSource::External => Phase::Armed { since: now },
                };
            }
            if now - run.settle_started > WAIT_LIMIT && outcome.is_none() {
                outcome = Some(Err(format!(
                    "the player did not settle in {WAIT_LIMIT} s: at {} facing {}, grounded {grounded}",
                    vector(position),
                    num(yaw_avatar)
                )));
            }
        }
        Phase::Armed { since } => {
            if consumed.gait != Gait::Idle || consumed.jump {
                run.phase = Phase::Running { t0: now };
                run.started_utc = utc_now();
            } else if now - since > WAIT_LIMIT {
                outcome = Some(Err(format!("no movement input in {WAIT_LIMIT} s")));
            }
        }
        Phase::Running { .. } => (),
    }

    if let Phase::Running { t0 } = run.phase {
        let t = now - t0;
        let dt = time.delta_secs_f64();
        let i = run.frame_count();
        let expected = run.scenario.input_at(t);
        if i > 0 && dt > DROPPED_FRAME_FACTOR / run.fps as f64 {
            run.dropped += 1;
        }
        if !consumed.matches(&expected) {
            run.mismatches += 1;
        }
        let scene_ctx = scenes.get(movement.scene()).ok();
        let scene_tick = scene_ctx.is_some_and(|ctx| ctx.last_update_frame == frame.0);
        run.scene_ticks += scene_tick as u32;
        let camera = camera.single().ok();
        let yaw_camera = camera.map_or(f64::NAN, |(t, _)| sdk_yaw(t.rotation));
        // the camera's own angles, as whoever turned it or the track left them this frame
        let cam = match camera {
            Some((_, camera)) if run.records_camera() => format!(
                ",\"cam\":{{\"yaw\":{},\"pitch\":{}}}",
                num(normalize_yaw(-(camera.yaw.to_degrees() as f64))),
                num(camera.pitch.to_degrees() as f64),
            ),
            None if run.records_camera() => ",\"cam\":null".into(),
            _ => String::new(),
        };
        run.frames.push(format!(
            concat!(
                "{{\"type\":\"frame\",\"i\":{},\"t\":{},\"dt\":{},\"in\":{},",
                "\"pos\":{},\"vel\":{},\"grounded\":{},",
                "\"yaw_avatar\":{},\"yaw_camera\":{},\"extra\":{{\"kind\":\"{}\",",
                "\"vel_requested\":{},\"vel_external\":{},\"scene_tick\":{},{}{}",
                "\"locomotion\":{}{}}}}}"
            ),
            i,
            num(t),
            num(dt),
            consumed.to_json(uses_jump),
            vector(position),
            sdk_vector(actual_velocity),
            grounded,
            num(yaw_avatar),
            num(yaw_camera),
            consumed.gait.name(),
            sdk_vector(requested_velocity),
            "null",
            scene_tick as u8,
            tick_timing_json(&stages.frame, frame.0),
            cpu,
            run.locomotion.as_deref().unwrap_or("null"),
            cam,
        ));
        run.last_t = t;
        if run.scenario.ends_at(t) {
            outcome = Some(Ok(()));
        }
    }

    if let Some(outcome) = outcome {
        trace.end(outcome, session.as_deref_mut(), strategy.as_deref_mut());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::input::InputPlugin;
    use std::time::Duration;

    const JOG: &str = r#"{
      "format": 1, "id": "jog-W-hold2.0-release", "seed": 1, "fps": 60, "camera_yaw": 0,
      "settle": 1.0, "duration": 4.0,
      "events": [
        { "t": 0.0, "fwd": 1, "right": 0, "walk": false, "sprint": false },
        { "t": 2.0, "fwd": 0, "right": 0, "walk": false, "sprint": false }
      ]
    }"#;

    #[test]
    fn a_scenario_parses_with_its_defaults() {
        let scenario = Scenario::parse(JOG).unwrap();
        assert_eq!(scenario.id, "jog-W-hold2.0-release");
        assert_eq!(scenario.fps, 60);
        assert_eq!(scenario.events.len(), 2);
        let bare = Scenario::parse(
            r#"{"format":1,"id":"x","fps":30,"duration":1,"events":[{"t":0,"right":-1}]}"#,
        )
        .unwrap();
        assert_eq!(bare.settle, 1.0);
        assert_eq!(bare.camera_yaw, 0.0);
        assert_eq!(
            bare.input_at(0.0),
            InputStep {
                right: -1,
                ..default()
            }
        );
    }

    #[test]
    fn a_malformed_scenario_is_refused() {
        let with = |field: &str, value: &str| {
            let mut json: serde_json::Value = serde_json::from_str(JOG).unwrap();
            json[field] = serde_json::from_str(value).unwrap();
            Scenario::parse(&json.to_string())
        };
        assert!(with("format", "3").is_err());
        assert!(with("duration", "120.5").is_err());
        assert!(with("id", "\"../x\"").is_err());
        assert!(with("fps", "0").is_err());
        assert!(with("duration", "0").is_err());
        assert!(with("events", "[]").is_err());
        assert!(with("events", r#"[{"t":1},{"t":0.5}]"#).is_err());
        assert!(with("events", r#"[{"t":0,"fwd":2}]"#).is_err());
        assert!(Scenario::parse("{").is_err());
    }

    #[test]
    fn an_event_is_reached_a_tenth_of_a_millisecond_early() {
        let scenario = Scenario::parse(JOG).unwrap();
        let w = InputStep {
            fwd: 1,
            ..default()
        };
        assert_eq!(scenario.input_at(1.9998), w);
        assert_eq!(scenario.input_at(1.99995), InputStep::default());
        assert_eq!(scenario.input_at(-1.0), InputStep::default());
        assert!(!scenario.ends_at(3.9998));
        assert!(scenario.ends_at(3.99995));
    }

    #[test]
    fn consumed_input_is_normalised_and_gaited_as_the_movement_code_reads_it() {
        let diagonal = ConsumedInput::from_actions(true, false, false, true, false, true);
        assert!((diagonal.fwd - FRAC_1_SQRT_2).abs() < 1e-6);
        assert!((diagonal.right - FRAC_1_SQRT_2).abs() < 1e-6);
        assert_eq!(diagonal.gait, Gait::Run);
        assert!(diagonal.matches(&InputStep {
            fwd: 1,
            right: 1,
            walk: false,
            sprint: true,
            jump: false
        }));
        // a modifier alone moves nothing
        let walk_only = ConsumedInput::from_actions(false, false, false, false, true, false);
        assert_eq!(walk_only.gait, Gait::Idle);
        assert!(walk_only.matches(&InputStep {
            walk: true,
            ..default()
        }));
        // sprint wins over walk
        let both = ConsumedInput::from_actions(false, true, false, false, true, true);
        assert_eq!((both.fwd, both.gait), (-1.0, Gait::Run));
        assert!(!both.matches(&InputStep {
            fwd: -1,
            walk: true,
            ..default()
        }));
    }

    #[test]
    fn the_default_bindings_give_wasd_ctrl_shift_and_space() {
        let keys = MovementKeys::from_map(&InputMap::default()).unwrap();
        assert_eq!(
            keys.pressed(&InputStep {
                fwd: -1,
                right: 1,
                walk: true,
                sprint: true,
                jump: true
            }),
            vec![
                KeyCode::KeyS,
                KeyCode::KeyD,
                KeyCode::ControlLeft,
                KeyCode::ShiftLeft,
                KeyCode::Space
            ]
        );
        assert_eq!(logical_key(KeyCode::KeyW), Key::Character("w".into()));
        assert_eq!(logical_key(KeyCode::Space), Key::Space);
    }

    #[test]
    fn jump_is_a_held_key_off_by_default() {
        let scenario = Scenario::parse(
            r#"{"format":1,"id":"j","fps":60,"duration":2,
                "events":[{"t":0,"fwd":1},{"t":0.5,"fwd":1,"jump":true},{"t":0.6,"fwd":1,"jump":false}]}"#,
        )
        .unwrap();
        assert!(scenario.uses_jump());
        assert!(!Scenario::parse(JOG).unwrap().uses_jump());
        assert!(!scenario.input_at(0.4).jump);
        assert!(scenario.input_at(0.55).jump);
        assert!(!scenario.input_at(0.6).jump);
        let mut json: serde_json::Value = serde_json::from_str(JOG).unwrap();
        json["events"][0]["jump"] = "yes".into();
        assert!(Scenario::parse(&json.to_string()).is_err());
    }

    #[test]
    fn a_scenario_that_jumps_needs_a_jump_key() {
        let jumping = Scenario::parse(
            r#"{"format":1,"id":"j","fps":60,"duration":1,"events":[{"t":0,"jump":true}]}"#,
        )
        .unwrap();
        let mut map = InputMap::default();
        map.inputs.remove(&Action::Scene(CommonInputAction::IaJump));
        let unbound = MovementKeys::from_map(&map).unwrap();
        assert!(unbound.for_scenario(&jumping).is_err());
        assert!(unbound.for_scenario(&Scenario::parse(JOG).unwrap()).is_ok());
        let keys = MovementKeys::from_map(&InputMap::default()).unwrap();
        assert!(keys.for_scenario(&jumping).is_ok());
    }

    #[test]
    fn the_jump_key_counts_in_the_input_only_when_the_scenario_jumps() {
        let still = ConsumedInput::default();
        let jumping = still.with_jump(true);
        assert!(jumping.matches(&InputStep {
            jump: true,
            ..default()
        }));
        assert!(!jumping.matches(&InputStep::default()));
        assert!(!still.matches(&InputStep {
            jump: true,
            ..default()
        }));
        // without jumps, `in` keeps its earlier form
        let jog = ConsumedInput::from_actions(true, false, false, false, false, false);
        assert_eq!(
            jog.to_json(false),
            r#"{"fwd":1.0,"right":0.0,"walk":false,"sprint":false}"#
        );
        assert_eq!(
            jog.with_jump(true).to_json(true),
            r#"{"fwd":1.0,"right":0.0,"walk":false,"sprint":false,"jump":true}"#
        );
        assert_eq!(
            jog.to_json(true),
            r#"{"fwd":1.0,"right":0.0,"walk":false,"sprint":false,"jump":false}"#
        );
    }

    #[test]
    fn yaw_is_degrees_clockwise_from_north() {
        let facing = |yaw: f64| {
            sdk_yaw(
                Transform::IDENTITY
                    .looking_to(yaw_direction(yaw), Vec3::Y)
                    .rotation,
            )
        };
        assert!(facing(0.0).abs() < 1e-4);
        assert!((facing(90.0) - 90.0).abs() < 1e-4);
        assert!((facing(-45.0) + 45.0).abs() < 1e-4);
        assert_eq!(normalize_yaw(-180.0), 180.0);
        assert_eq!(normalize_yaw(270.0), -90.0);
        // the camera's engine yaw the recorder holds for an SDK yaw
        let camera = Quat::from_euler(EulerRot::YXZ, -(30f32.to_radians()), -0.3, 0.0);
        assert!((sdk_yaw(camera) - 30.0).abs() < 1e-3);
        // north is +z in the SDK, -z in the engine
        let base = IVec2::new(84, -6);
        let world = world_translation(Vec3::new(32.0, 0.0, 8.0), base);
        assert_eq!(world, Vec3::new(1376.0, 0.0, 88.0));
        assert_eq!(scene_position(world, base), Vec3::new(32.0, 0.0, 8.0));
    }

    #[test]
    fn a_frame_writes_the_movement_scenes_dispatch_reply_and_stages() {
        use scene_runner::tick_stages::{LoopEnd, ReplyMark, SentMark};

        // the keys as the frame line carries them, each followed by a comma
        let fields = |keys: String| -> serde_json::Value {
            serde_json::from_str(&format!("{{{}}}", keys.trim_end_matches(','))).unwrap()
        };
        let sent = |frame, at| SentMark {
            seq: frame as u64,
            frame,
            at,
            loop_start: 1000.0,
            deadline: 1016.0,
        };
        // dispatched in frame 7, 2 ms into the loop; frame 6's reply came 3 ms after its deadline
        let stage = FrameStages {
            start: 999.0,
            loop_start: 1000.0,
            deadline: 1016.0,
            loop_end: 1016.5,
            interval: 16.5,
            end: LoopEnd::Deadline,
            mode: "both",
            tail_reserve: Some(6.5),
            sent: Some(sent(7, 1002.0)),
            reply: Some(ReplyMark {
                sent: sent(6, 1004.0),
                frame: 7,
                at: 1019.0,
                first_poll: true,
                relay: None,
            }),
            ..Default::default()
        };
        let line = fields(tick_timing_json(&stage, 7));
        assert_eq!(line["scene_sent"], 2.0);
        assert_eq!(line["scene_reply_late"], 3.0);
        assert_eq!(line["scene_reply_frames"], 1);
        assert_eq!(line["stage"]["B"], 16.0);
        assert_eq!(line["stage"]["P"], 1.0);
        assert_eq!(line["stage"]["end"], "deadline");
        assert_eq!(line["stage"]["mode"], "both");
        assert_eq!(line["stage"]["K"], 6.5);
        assert!(line["stage"]["relay_waits"].is_null());
        assert!(line["stage"]["prev_wait_render"].is_null());
        assert_eq!(line["stage"]["sent"]["seq"], 7);
        assert_eq!(line["stage"]["reply"]["seq"], 6);
        assert_eq!(line["stage"]["reply"]["first_poll"], true);
        assert!(line["stage"]["reply"]["relay"].is_null());
        // a frame with neither
        let line = fields(tick_timing_json(&stage, 8));
        assert!(line["scene_sent"].is_null());
        assert!(line["scene_reply_late"].is_null());
        assert!(line["stage"]["reply"].is_null());
    }

    #[test]
    fn numbers_are_written_short_and_finite() {
        assert_eq!(num(0.0), "0.0");
        assert_eq!(num(-0.0000001), "0.0");
        assert_eq!(num(1.0 / 60.0), "0.016667");
        assert_eq!(num(-2.5), "-2.5");
        assert_eq!(num(f64::NAN), "null");
    }

    /// Runs `frames` frames 1/60 s apart with a run already at t = 0 on the first, and returns
    /// whether W was down in each.
    fn w_by_frame(scenario: &str, frames: usize) -> Vec<bool> {
        key_by_frame(scenario, frames, KeyCode::KeyW)
    }

    /// Runs `frames` frames 1/60 s apart with a run already at t = 0 on the first, and returns
    /// whether `key` was down in each.
    fn key_by_frame(scenario: &str, frames: usize, key: KeyCode) -> Vec<bool> {
        let mut app = App::new();
        app.add_plugins((bevy::time::TimePlugin, InputPlugin))
            .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f64(
                1.0 / 60.0,
            )))
            .init_resource::<MovementTrace>()
            .add_event::<RpcCallEvent>()
            .add_systems(PreUpdate, press_scenario_keys.before(InputSystem));
        app.update();
        let scenario = Scenario::parse(scenario).unwrap();
        let keys = MovementKeys::from_map(&InputMap::default()).unwrap();
        app.world_mut().resource_mut::<MovementTrace>().run = Some(test_run(scenario, keys));
        (0..frames)
            .map(|_| {
                app.update();
                app.world().resource::<ButtonInput<KeyCode>>().pressed(key)
            })
            .collect()
    }

    fn test_run(scenario: Scenario, keys: MovementKeys) -> Run {
        Run {
            fps: scenario.fps,
            scenario,
            repeat: 1,
            input: InputSource::Keys(keys),
            start: StartCondition::Still,
            camera: CameraControl::Held,
            fixed_dt: true,
            previous_fps: None,
            base: IVec2::ZERO,
            spawn: Vec3::ZERO,
            spawn_yaw: 0.0,
            realm: String::new(),
            scene: String::new(),
            client_version: String::new(),
            animations: None,
            camera_mode: "third_person",
            phase: Phase::Starting,
            spawns: 0,
            settle_started: 0.0,
            started_utc: String::new(),
            consumed: None,
            frames: Vec::new(),
            drained: 0,
            header_drained: false,
            dropped: 0,
            mismatches: 0,
            scene_ticks: 0,
            scene_log: None,
            locomotion: None,
            start_position: None,
            last_position: Vec3::ZERO,
            last_t: 0.0,
        }
    }

    #[test]
    fn keys_follow_the_scenario_by_frame_time() {
        // a tap of 0.1 s at 60 fps: down at t = 0 .. 5/60, up from t = 6/60 = 0.1
        let tap = r#"{"format":1,"id":"tap","fps":60,"duration":0.5,
            "events":[{"t":0,"fwd":1},{"t":0.1}]}"#;
        let w = w_by_frame(tap, 9);
        assert_eq!(
            w,
            vec![true, true, true, true, true, true, false, false, false]
        );
    }

    #[test]
    fn the_jump_key_follows_the_scenario_by_frame_time() {
        // jump held from t = 0.05 (frame 3) to t = 0.1 (frame 6), while W stays down
        let tap = r#"{"format":1,"id":"jump-tap","fps":60,"duration":0.5,
            "events":[{"t":0,"fwd":1},{"t":0.05,"fwd":1,"jump":true},{"t":0.1,"fwd":1}]}"#;
        assert_eq!(
            key_by_frame(tap, 9, KeyCode::Space),
            vec![false, false, false, true, true, true, false, false, false]
        );
        assert_eq!(w_by_frame(tap, 9), vec![true; 9]);
        // a scenario without jumps never presses the jump key
        assert_eq!(
            key_by_frame(
                r#"{"format":1,"id":"w","fps":60,"duration":0.5,"events":[{"t":0,"fwd":1}]}"#,
                4,
                KeyCode::Space
            ),
            vec![false; 4]
        );
    }

    #[test]
    fn keys_the_run_holds_are_released_when_it_ends() {
        let hold = r#"{"format":1,"id":"hold","fps":60,"duration":5,"events":[{"t":0,"fwd":1}]}"#;
        let mut app = App::new();
        app.add_plugins((bevy::time::TimePlugin, InputPlugin))
            .init_resource::<MovementTrace>()
            .add_event::<RpcCallEvent>()
            .add_systems(PreUpdate, press_scenario_keys.before(InputSystem));
        let scenario = Scenario::parse(hold).unwrap();
        let keys = MovementKeys::from_map(&InputMap::default()).unwrap();
        app.world_mut().resource_mut::<MovementTrace>().run = Some(test_run(scenario, keys));
        app.update();
        app.update();
        assert!(app
            .world()
            .resource::<ButtonInput<KeyCode>>()
            .pressed(KeyCode::KeyW));
        app.world_mut()
            .resource_mut::<MovementTrace>()
            .end(Err("stopped".into()), None, None);
        app.update();
        assert!(!app
            .world()
            .resource::<ButtonInput<KeyCode>>()
            .pressed(KeyCode::KeyW));
        assert!(app.world().resource::<MovementTrace>().held.is_empty());
    }

    #[test]
    fn a_trace_is_a_header_frames_and_a_footer() {
        let scenario = Scenario::parse(JOG).unwrap();
        let mut run = test_run(
            scenario,
            MovementKeys::from_map(&InputMap::default()).unwrap(),
        );
        run.frames.push("{\"type\":\"frame\",\"i\":0}".into());
        run.dropped = 1;
        run.mismatches = 2;
        let text = run.finish();
        let lines: Vec<serde_json::Value> = text
            .lines()
            .map(|l| serde_json::from_str(l).unwrap())
            .collect();
        assert_eq!(lines.len(), 3);
        assert_eq!(lines[0]["type"], "header");
        assert_eq!(lines[0]["client"], "bevy");
        assert_eq!(lines[0]["scenario"], "jog-W-hold2.0-release");
        assert_eq!(lines[0]["fps_target"], 60);
        assert_eq!(lines[0]["extra"]["dt_mode"], "fixed");
        assert_eq!(lines[0]["extra"]["input_seam"], "keyboard");
        assert_eq!(lines[2]["type"], "footer");
        assert_eq!(lines[2]["frames"], 1);
        assert_eq!(lines[2]["dropped"], 1);
        assert_eq!(lines[2]["extra"]["input_mismatch_frames"], 2);
        assert_eq!(run.file_name(), "jog-W-hold2.0-release__fps60__r1.jsonl");
        // a run that waits for the player to stand still keeps the earlier header
        assert!(lines[0]["extra"].get("start").is_none());
        run.start = StartCondition::Placed;
        let header: serde_json::Value =
            serde_json::from_str(run.finish().lines().next().unwrap()).unwrap();
        assert_eq!(header["extra"]["start"], "placed");
        // a camera held at one yaw keeps the earlier header too
        assert!(header["extra"].get("camera_control").is_none());
        assert!(!run.records_camera());
        run.camera = CameraControl::Free;
        let header: serde_json::Value =
            serde_json::from_str(run.finish().lines().next().unwrap()).unwrap();
        assert_eq!(header["extra"]["camera_control"], "free");
        assert!(run.records_camera());
        run.camera = CameraControl::Held;
        run.scenario = Scenario::parse(TRACK).unwrap();
        let header: serde_json::Value =
            serde_json::from_str(run.finish().lines().next().unwrap()).unwrap();
        assert_eq!(header["extra"]["camera_control"], "track");
        assert_eq!(header["format"], FORMAT);
    }

    #[test]
    fn a_drained_run_hands_out_each_line_once() {
        let mut run = test_run(
            Scenario::parse(JOG).unwrap(),
            MovementKeys::from_map(&InputMap::default()).unwrap(),
        );
        let frame = |i: usize| format!("{{\"type\":\"frame\",\"i\":{i}}}");
        // nothing before t = 0
        assert_eq!(run.drain(), "");
        run.phase = Phase::Running { t0: 0.0 };
        let whole = {
            let mut whole = test_run(
                Scenario::parse(JOG).unwrap(),
                MovementKeys::from_map(&InputMap::default()).unwrap(),
            );
            whole.frames = (0..5).map(frame).collect();
            whole.finish()
        };
        run.frames = (0..2).map(frame).collect();
        let first = run.drain();
        assert_eq!(first.lines().count(), 3);
        assert_eq!(run.drain(), "");
        run.frames = (2..5).map(frame).collect();
        assert_eq!(run.frame_count(), 5);
        assert_eq!(run.summary()["frames"], 5);
        let second = run.drain();
        let rest = run.finish();
        assert_eq!(format!("{first}{second}{rest}"), whole);
    }

    const TRACK: &str = r#"{
      "format": 2, "id": "free-play-1", "fps": 60, "settle": 1.0, "duration": 300,
      "events": [{ "t": 0.0, "fwd": 1 }, { "t": 0.05 }],
      "camera": [
        { "t": 0.0, "yaw": 170, "pitch": -10 },
        { "t": 0.05, "yaw": 190, "pitch": 10 },
        { "t": 0.1, "yaw": -150, "pitch": 10 }
      ]
    }"#;

    #[test]
    fn a_camera_track_needs_format_2_which_runs_up_to_an_hour() {
        let track = Scenario::parse(TRACK).unwrap();
        assert_eq!(track.camera.len(), 3);
        assert_eq!(Scenario::parse(JOG).unwrap().camera, vec![]);
        let with = |field: &str, value: &str| {
            let mut json: serde_json::Value = serde_json::from_str(TRACK).unwrap();
            json[field] = serde_json::from_str(value).unwrap();
            Scenario::parse(&json.to_string())
        };
        // format 1 neither carries a track nor runs past 120 s
        assert!(with("format", "1").is_err());
        assert!(with("camera", "[]").is_ok());
        let mut json: serde_json::Value = serde_json::from_str(JOG).unwrap();
        json["format"] = 2.into();
        json["duration"] = 3600.into();
        assert!(Scenario::parse(&json.to_string()).is_ok());
        json["duration"] = 3600.5.into();
        assert!(Scenario::parse(&json.to_string()).is_err());
        // samples in order, a finite yaw, pitch on all or none and within a quarter turn
        assert!(with("camera", r#"[{"t":1,"yaw":0},{"t":0.5,"yaw":0}]"#).is_err());
        assert!(with("camera", r#"[{"t":0,"yaw":null}]"#).is_err());
        assert!(with("camera", r#"[{"t":0,"yaw":0,"pitch":5},{"t":1,"yaw":0}]"#).is_err());
        assert!(with("camera", r#"[{"t":0,"yaw":0},{"t":1,"yaw":0,"pitch":5}]"#).is_err());
        assert!(with("camera", r#"[{"t":0,"yaw":0,"pitch":91}]"#).is_err());
        assert!(with("camera", r#"[{"t":0,"yaw":720},{"t":1,"yaw":-30}]"#).is_ok());
    }

    #[test]
    fn the_camera_track_is_interpolated_the_short_way_round() {
        let track = Scenario::parse(TRACK).unwrap();
        let close = |a: f64, b: f64| (a - b).abs() < 1e-9;
        let at = |t: f64| track.camera_at(t).unwrap();
        // held before the first sample and after the last
        assert!(close(at(-1.0).yaw, 170.0) && at(-1.0).pitch == Some(-10.0));
        assert!(close(at(9.0).yaw, -150.0));
        // 170 to 190 crosses south, through 180, not back through north
        assert!(close(at(0.025).yaw, 180.0));
        assert!(close(at(0.0125).yaw, 175.0));
        assert!(close(at(0.0375).yaw, -175.0));
        assert!(close(at(0.025).pitch.unwrap(), 0.0));
        // 190 (-170) to -150
        assert!(close(at(0.075).yaw, -160.0));
        assert_eq!(Scenario::parse(JOG).unwrap().camera_at(0.0), None);
    }

    /// Plays `scenario` from t = 0 for `frames` frames at its frame rate with a fixed clock, and
    /// returns each frame's time, the input the keys held give and the camera's SDK yaw and
    /// pitch (degrees).
    fn replay(scenario: Scenario, frames: usize) -> Vec<(f64, ConsumedInput, f64, f64)> {
        let fps = scenario.fps as f64;
        let mut app = App::new();
        app.add_plugins((bevy::time::TimePlugin, InputPlugin))
            .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f64(
                1.0 / fps,
            )))
            .init_resource::<MovementTrace>()
            .add_event::<RpcCallEvent>()
            .add_systems(PreUpdate, press_scenario_keys.before(InputSystem))
            .add_systems(Update, drive_camera);
        app.world_mut().spawn(PrimaryCamera::default());
        app.update();
        let keys = MovementKeys::from_map(&InputMap::default()).unwrap();
        app.world_mut().resource_mut::<MovementTrace>().run = Some(test_run(scenario, keys));
        (0..frames)
            .map(|_| {
                app.update();
                let world = app.world_mut();
                let t = match world
                    .resource::<MovementTrace>()
                    .run
                    .as_ref()
                    .unwrap()
                    .phase
                {
                    Phase::Running { t0 } => world.resource::<Time>().elapsed_secs_f64() - t0,
                    _ => f64::NAN,
                };
                let down = |key| world.resource::<ButtonInput<KeyCode>>().pressed(key);
                let input = ConsumedInput::from_actions(
                    down(keys.forward),
                    down(keys.backward),
                    down(keys.left),
                    down(keys.right),
                    down(keys.walk),
                    down(keys.sprint),
                )
                .with_jump(keys.jump.is_some_and(down));
                let camera = world.query::<&PrimaryCamera>().single(world).unwrap();
                (
                    t,
                    input,
                    normalize_yaw(-(camera.yaw.to_degrees() as f64)),
                    camera.pitch.to_degrees() as f64,
                )
            })
            .collect()
    }

    #[test]
    fn the_camera_follows_the_track_by_frame_time() {
        // 60 fps: W down in frames 0 to 2; the camera turns from 170 through 180 to -170 at
        // t = 0.05 (frame 3), then on to -150
        let frames = replay(Scenario::parse(TRACK).unwrap(), 7);
        let w = |fwd| InputStep { fwd, ..default() };
        for (i, (t, input, yaw, pitch)) in frames.iter().enumerate() {
            let expected = Scenario::parse(TRACK).unwrap().camera_at(*t).unwrap();
            // the clock steps in whole nanoseconds
            assert!((t - i as f64 / 60.0).abs() < 1e-6, "frame {i} at {t}");
            assert!(input.matches(&w((i < 3) as i8)), "frame {i}: {input:?}");
            assert!(
                yaw_difference(*yaw, expected.yaw).abs() < 1e-3,
                "frame {i}: {yaw}"
            );
            assert!(
                (pitch - expected.pitch.unwrap()).abs() < 1e-3,
                "frame {i}: {pitch}"
            );
        }
        assert!((frames[0].2 - 170.0).abs() < 1e-3);
        assert!(yaw_difference(frames[3].2, -170.0).abs() < 1e-3);
        // without a track the camera holds the scenario's yaw and leaves the pitch alone
        let held = replay(Scenario::parse(JOG).unwrap(), 2);
        assert!(held.iter().all(|f| f.2.abs() < 1e-6 && f.3 == 0.0));
    }

    /// Plays a scenario against the recorded trace it was made from and checks each frame's
    /// keys and camera against the trace's: `MOVEMENT_TRACE_REPLAY_TRACE` (a `--camera free`
    /// trace, its lines joined in order) and `MOVEMENT_TRACE_REPLAY_SCENARIO` (the scenario).
    /// A frame expects the input of the last recorded frame at or before its time and the
    /// camera interpolated between the recorded frames around it, within 0.1 degrees.
    #[test]
    #[ignore = "needs a recorded trace and its scenario"]
    fn a_scenario_made_from_a_free_play_trace_replays_its_keys_and_camera() {
        let read = |var: &str| {
            std::fs::read_to_string(std::env::var(var).unwrap_or_else(|_| panic!("{var} unset")))
                .unwrap()
        };
        let scenario = Scenario::parse(&read("MOVEMENT_TRACE_REPLAY_SCENARIO")).unwrap();
        let recorded: Vec<serde_json::Value> = read("MOVEMENT_TRACE_REPLAY_TRACE")
            .lines()
            .map(|l| serde_json::from_str(l).unwrap())
            .filter(|l: &serde_json::Value| l["type"] == "frame")
            .collect();
        assert!(!recorded.is_empty(), "no frames in the trace");
        let f = |v: &serde_json::Value| v.as_f64().unwrap();
        let step = |frame: &serde_json::Value| {
            let sign = |v: f64| (v > 0.5) as i8 - (v < -0.5) as i8;
            let input = &frame["in"];
            InputStep {
                fwd: sign(f(&input["fwd"])),
                right: sign(f(&input["right"])),
                walk: input["walk"] == true,
                sprint: input["sprint"] == true,
                jump: input["jump"] == true,
            }
        };
        let count = (scenario.duration * scenario.fps as f64).round() as usize + 1;
        let mut wrong = Vec::new();
        for (t, input, yaw, pitch) in replay(scenario, count) {
            let next = recorded.partition_point(|r| f(&r["t"]) <= t + EVENT_EPSILON);
            let at = &recorded[next.max(1) - 1];
            if !input.matches(&step(at)) {
                wrong.push(format!("t {t:.4}: keys {input:?}, recorded {}", at["in"]));
            }
            let b = &recorded[next.min(recorded.len() - 1)];
            let (ta, tb) = (f(&at["t"]), f(&b["t"]));
            let k = if tb > ta {
                ((t - ta) / (tb - ta)).clamp(0.0, 1.0)
            } else {
                0.0
            };
            let (ca, cb) = (&at["extra"]["cam"], &b["extra"]["cam"]);
            let want_yaw = f(&ca["yaw"]) + yaw_difference(f(&cb["yaw"]), f(&ca["yaw"])) * k;
            let want_pitch = f(&ca["pitch"]) + (f(&cb["pitch"]) - f(&ca["pitch"])) * k;
            if yaw_difference(yaw, want_yaw).abs() > 0.1 || (pitch - want_pitch).abs() > 0.1 {
                wrong.push(format!(
                    "t {t:.4}: camera {yaw:.3}/{pitch:.3}, recorded {want_yaw:.3}/{want_pitch:.3}"
                ));
            }
        }
        assert!(
            wrong.is_empty(),
            "{} of {count} frames differ:\n{}",
            wrong.len(),
            wrong[..wrong.len().min(20)].join("\n")
        );
    }

    #[test]
    fn the_movement_model_state_is_read_from_the_scenes_console_line() {
        let state = r#"{"t":42,"gait":"jog","grounded":false,"jump_count":1,"glide":"closed","stunned":false,"sliding":false}"#;
        // the web runtime writes the string as it is, the native one JSON-quoted
        assert_eq!(
            locomotion_line(&format!("LOG LOCO {state}")).as_deref(),
            Some(state)
        );
        let quoted = serde_json::Value::from(format!("LOCO {state}")).to_string();
        assert_eq!(
            locomotion_line(&format!("LOG {quoted}")).as_deref(),
            Some(state)
        );
        for other in [
            "LOG \"hello\"",
            "LOG LOCO",
            "LOG LOCO 3",
            "LOG LOCO {broken",
            &format!("ERROR LOCO {state}"),
        ] {
            assert_eq!(locomotion_line(other), None, "{other}");
        }
    }

    #[test]
    fn a_placed_start_waits_for_the_settle_and_the_ground_but_not_for_stillness() {
        // never the placement's own frame, even with no settle
        assert!(!placed_start(0.0, 0.0, true));
        assert!(placed_start(1.0 / 60.0, 0.0, true));
        // the settle counts from the placement
        assert!(!placed_start(0.5, 1.0, true));
        assert!(placed_start(1.0, 1.0, true));
        // in the air it waits for the ground, then starts anyway
        assert!(!placed_start(1.0, 0.0, false));
        assert!(!placed_start(GROUNDED_WAIT - 0.01, 0.0, false));
        assert!(placed_start(GROUNDED_WAIT, 0.0, false));
        assert!(placed_start(1.0 + GROUNDED_WAIT, 1.0, false));
    }
}
