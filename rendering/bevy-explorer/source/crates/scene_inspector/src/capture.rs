//! `/screenshot`, one frame or with `--count` a timed sequence: frames of the primary window, read
//! back at the end of the frame they were rendered in, through one capture job at a time (the
//! gate; a second capture is refused and the refusal names the holder). A job waits two frames,
//! then for the camera's transition (at most 1.5 s), then captures. The frames are scaled and
//! encoded by `common::frame`, the helper `/texture_camera_screenshot` and the camera reel's
//! photo use too. Here that work runs on the IO task pool, and so does a sequence's file work:
//! making its folder, writing its frames, counting the frames an earlier sequence left there. The
//! pool is off the main thread natively and on the engine's own thread on the web.
//! (`/texture_camera_screenshot` scales and encodes its one small image on the main thread, when
//! the console polls its reply.) `--world-only` hides the native HUD and the scenes' window UI
//! while the job runs, the scene UI through `HiddenSceneUis`, the rule `/show_ui` uses.

use std::{
    future::Future,
    path::{Path, PathBuf},
};

use base64::{prelude::BASE64_STANDARD, Engine};
use bevy::{
    diagnostic::FrameCount,
    prelude::*,
    render::view::screenshot::Screenshot,
    tasks::{IoTaskPool, Task},
    window::PrimaryWindow,
};
use bevy_console::ConsoleCommand;
use common::{
    frame::{self, Encoding, Scale},
    structs::{PrimaryCamera, PrimaryUser},
    util::TaskExt,
};
use console::{
    reply::{self, Screen},
    AddDebugSection, DebugSection, DoAddConsoleCommand, PendingConsoleResponses,
};
use scene_runner::{update_world::scene_ui::HiddenSceneUis, vec3_to_parcel};
use serde_json::{json, Value};
use tokio::sync::oneshot;
use tween::{SystemTween, SystemTweenData};

use crate::active_scene::SceneResolver;

/// Frames a capture waits before it looks at the camera, so a change the command's own frame
/// started (a virtual camera handing back) has begun its transition.
const SETTLE_FRAMES: u32 = 2;
const BLEND_DEADLINE_SECONDS: f64 = 1.5;
const SEQUENCE_MAX_BURST_SECONDS: f32 = 20.0;
/// How long a job may wait for the IO task pool and the GPU: for its folder after it started, and
/// for its frames, and the search of its folder, after the last frame was asked for.
const READBACK_TIMEOUT_SECONDS: f64 = 10.0;
/// The frames of a sequence that may wait, at the window's full size, to be read back and encoded.
/// The next frame is not asked for while this many do: an encoder that falls behind makes later
/// frames late, which the reply says, where it would otherwise keep every frame in memory.
const MAX_FRAMES_IN_FLIGHT: usize = 6;

pub(crate) struct CapturePlugin;

impl Plugin for CapturePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CaptureGate>();
        app.init_resource::<CaptureStore>();
        app.add_console_command::<ScreenshotCommand, _>(screenshot_cmd)
            .add_debug_section(DebugSection {
                title: "Frame capture",
                query: "/screenshot --state",
                fields: &[
                    ("Capture running", "holder"),
                    ("Frames taken", "framesTaken"),
                    ("Frames asked for", "count"),
                    ("Hiding the HUD and scene UI", "worldOnly"),
                    ("Frames kept in memory", "framesInMemory"),
                    ("Bytes kept in memory", "bytesInMemory"),
                ],
                actions: &[],
                inputs: &[],
            });
        app.add_systems(Update, drive_capture);
    }
}

/// While an entity holds this, a `--world-only` capture runs: the native HUD hides.
#[derive(Component)]
pub struct WorldOnlyCapture;

// --- /screenshot ---

/// Capture the rendered frame of the primary window. With no option the reply is raw base64 PNG
/// at full size (no `data:` prefix; the editor host's thumbnail contract). With `--max-width`,
/// `--jpg` or `--world-only` it is JSON: `caption` (the image size, the screen and frame it was
/// scaled from, the parcel, the camera wait), `format`, `width`, `height`, `frame`, `screen`,
/// `parcel`, `worldOnly`, `blendWaitMs`, `image` (base64). Every capture, the bare one included,
/// waits two frames and then for a camera transition (at most 1.5 s), and is refused while
/// another capture runs.
///
/// `--count` captures a sequence instead: that many frames `--interval-seconds` apart (0.5 by
/// default), scheduled from the first frame, each at most `--max-width` wide (640 by default).
/// Encoding delays the next frame only when six frames already wait for the encoder. Natively
/// the frames are files under `--dir` (default: a new folder in the client's cache); a `--dir`
/// that already holds frames is written over from `frame-000`, the rest left alone and counted
/// in `staleFrames`. On the web they stay in memory, the last sequence only, and `--frame <path>`
/// reads one as raw base64. JSON reply: `directory` (absolute), `stored` (disk or memory),
/// `count`, `requestedIntervalSeconds`, `meanIntervalSeconds`, `late` (frames taken more than
/// half an interval after their time), `screen`, `frames` (`path`, `tick` of the scene,
/// `engineFrame`, `atSeconds`, `late`, ...). No image is inline.
///
/// `--state` captures nothing and replies with the capture gate, for the debug panel: `holder`
/// (`screenshot`, `screenshot --count` or `none`), `framesTaken` and `count` of the running
/// capture, `worldOnly`, and the `framesInMemory` and `bytesInMemory` the last in-memory sequence
/// keeps.
#[derive(clap::Parser, ConsoleCommand)]
#[command(name = "/screenshot")]
struct ScreenshotCommand {
    /// scale the image down to at most this many pixels wide (64 to 1920; 640 for a sequence
    /// when not given)
    #[arg(long, value_parser = clap::value_parser!(u32).range(64..=1920))]
    max_width: Option<u32>,
    /// encode as JPEG (quality 75) instead of PNG
    #[arg(long)]
    jpg: bool,
    /// hide the HUD and the scenes' window UI while the capture runs
    #[arg(long)]
    world_only: bool,
    /// capture a sequence of this many frames (1 to 20; the burst may span at most 20 s)
    #[arg(long, value_parser = clap::value_parser!(u32).range(1..=20))]
    count: Option<u32>,
    /// seconds between a sequence's frames (0.05 to 2; 0.5 when not given)
    #[arg(long, requires = "count")]
    interval_seconds: Option<f32>,
    /// folder to write a sequence's frames to (native only; created if missing)
    #[arg(long, requires = "count")]
    dir: Option<PathBuf>,
    /// read one frame of the last in-memory sequence, by the `path` its reply gave
    /// (`capture://<sequence>/frame-<index>.<ext>`), instead of capturing
    #[arg(
        long,
        conflicts_with_all = ["max_width", "jpg", "world_only", "count", "interval_seconds", "dir", "state"]
    )]
    frame: Option<String>,
    /// read the capture gate, instead of capturing
    #[arg(
        long,
        conflicts_with_all = ["max_width", "jpg", "world_only", "count", "interval_seconds", "dir"]
    )]
    state: bool,
}

const SEQUENCE_DEFAULT_INTERVAL_SECONDS: f32 = 0.5;
const SEQUENCE_DEFAULT_MAX_WIDTH: u32 = 640;

fn screenshot_cmd(
    mut input: ConsoleCommand<ScreenshotCommand>,
    mut commands: Commands,
    mut gate: ResMut<CaptureGate>,
    mut store: ResMut<CaptureStore>,
    time: Res<Time<Real>>,
    mut console_responses: ResMut<PendingConsoleResponses>,
) {
    let Some(Ok(cmd)) = input.take() else {
        return;
    };
    if cmd.state {
        input.reply_ok(state_reply(&gate, &store));
        return;
    }
    if let Some(path) = cmd.frame {
        match stored_frame(&store, &path) {
            Ok(frame) => input.reply_ok(frame),
            Err(reason) => input.reply_failed(reason),
        }
        return;
    }
    if let Some(refusal) = gate.refusal() {
        input.reply_failed(refusal);
        return;
    }
    let format = if cmd.jpg {
        Encoding::Jpeg(frame::JPEG_QUALITY)
    } else {
        Encoding::Png
    };
    let (kind, max_width) = match cmd.count {
        None => (
            Kind::Shot {
                json: cmd.max_width.is_some() || cmd.jpg || cmd.world_only,
            },
            cmd.max_width,
        ),
        Some(count) => {
            let interval = cmd
                .interval_seconds
                .unwrap_or(SEQUENCE_DEFAULT_INTERVAL_SECONDS);
            if let Err(reason) = check_burst(count, interval) {
                input.reply_failed(reply::refusal(reason, None));
                return;
            }
            let target = match capture_target(cmd.dir, &mut store) {
                Ok(target) => target,
                Err(reason) => {
                    input.reply_failed(reply::refusal(reason, None));
                    return;
                }
            };
            (
                Kind::Sequence {
                    count: count as usize,
                    interval: interval as f64,
                    target,
                },
                Some(cmd.max_width.unwrap_or(SEQUENCE_DEFAULT_MAX_WIDTH)),
            )
        }
    };
    let (tx, rx) = oneshot::channel();
    gate.job = Some(CaptureJob::new(
        kind,
        Options {
            max_width,
            format,
            world_only: cmd.world_only,
        },
        tx,
        time.elapsed_secs_f64(),
        &mut commands,
    ));
    console_responses.push_oneshot(rx, |r| r, input.take_responder());
}

/// The interval and count a sequence may use.
fn check_burst(count: u32, interval: f32) -> Result<(), String> {
    if !(0.05..=2.0).contains(&interval) {
        return Err(format!(
            "--interval-seconds runs from 0.05 to 2; got {interval}"
        ));
    }
    let burst = (count.saturating_sub(1)) as f32 * interval;
    if burst > SEQUENCE_MAX_BURST_SECONDS + 1e-4 {
        return Err(format!(
            "{count} frames {interval} s apart span {burst:.2} s; a sequence spans at most {SEQUENCE_MAX_BURST_SECONDS} s"
        ));
    }
    Ok(())
}

/// Where a sequence's frames go: a folder (given, or new under the cache) natively; memory on the
/// web, which cannot write the host's disk. The folder is made on the IO task pool, and the job
/// waits for it before it asks for a frame.
fn capture_target(dir: Option<PathBuf>, store: &mut CaptureStore) -> Result<Target, String> {
    let folder = match dir {
        Some(dir) if cfg!(target_arch = "wasm32") => {
            return Err(format!(
                "the web client cannot write files ({}); omit --dir, the frames stay in memory for /screenshot --frame",
                dir.display()
            ))
        }
        Some(dir) => Some(dir),
        None => platform::project_directories().map(|dirs| {
            let stamp = web_time::SystemTime::now()
                .duration_since(web_time::UNIX_EPOCH)
                .map(|d| d.as_millis())
                .unwrap_or_default();
            dirs.cache_dir()
                .join("captures")
                .join(format!("capture-{stamp}"))
        }),
    };
    match folder {
        // a relative `--dir` is the client's working directory's: the reply names it in full
        Some(folder) => Ok(Target::Disk(Folder::make(
            std::path::absolute(&folder).unwrap_or(folder),
        ))),
        None => {
            store.sequence += 1;
            store.frames.clear();
            Ok(Target::Memory(store.sequence))
        }
    }
}

/// One frame of the last in-memory sequence (the web client's), by the `path` its reply gave, as
/// raw base64.
fn stored_frame(store: &CaptureStore, path: &str) -> Result<String, String> {
    match store.frames.iter().find(|(stored, _)| stored == path) {
        Some((_, bytes)) => Ok(BASE64_STANDARD.encode(bytes)),
        None => Err(format!(
            "no frame {path} in memory: only the last in-memory sequence ({}) is kept",
            store.directory()
        )),
    }
}

/// The capture gate and what the last in-memory sequence keeps, as JSON.
fn state_reply(gate: &CaptureGate, store: &CaptureStore) -> String {
    let job = gate.job.as_ref();
    json!({
        "holder": job.map_or("none", CaptureJob::holder),
        "framesTaken": job.map_or(0, |job| job.frames.len()),
        "count": job.map_or(0, CaptureJob::count),
        "worldOnly": job.is_some_and(|job| job.options.world_only),
        "framesInMemory": store.frames.len(),
        "bytesInMemory": store.frames.iter().map(|(_, bytes)| bytes.len()).sum::<usize>(),
    })
    .to_string()
}

// --- the job ---

/// The capture running now, if any.
#[derive(Resource, Default)]
struct CaptureGate {
    job: Option<CaptureJob>,
}

impl CaptureGate {
    fn refusal(&self) -> Option<String> {
        self.job.as_ref().map(|job| {
            reply::refusal(
                format!(
                    "a capture is already in progress ({}); retry when it completes",
                    job.holder()
                ),
                Some(json!({ "holder": job.holder() })),
            )
        })
    }
}

/// The frames of the last sequence kept in memory, by path.
#[derive(Resource, Default)]
struct CaptureStore {
    sequence: u32,
    frames: Vec<(String, Vec<u8>)>,
}

impl CaptureStore {
    fn directory(&self) -> String {
        format!("capture://{}", self.sequence)
    }
}

struct Options {
    max_width: Option<u32>,
    format: Encoding,
    world_only: bool,
}

enum Target {
    Disk(Folder),
    Memory(u32),
}

/// A sequence's folder on disk. The IO task pool makes it and, once the frames are written,
/// counts the frames an earlier sequence left in it. The job waits for the first before it asks
/// for a frame, and for the second before it replies.
struct Folder {
    path: PathBuf,
    made: Io<Result<(), String>>,
    /// when the count was asked for, and the count
    stale: Option<(f64, Io<usize>)>,
}

impl Folder {
    fn make(path: PathBuf) -> Self {
        let folder = path.clone();
        let made = Io::spawn(async move {
            std::fs::create_dir_all(&folder)
                .map_err(|e| format!("cannot create {}: {e}", folder.display()))
        });
        Self {
            path,
            made,
            stale: None,
        }
    }
}

/// A value the IO task pool works out, which the job polls each frame.
enum Io<T> {
    Running(Task<T>),
    Done(T),
}

impl<T: Send + 'static> Io<T> {
    fn spawn(work: impl Future<Output = T> + Send + 'static) -> Self {
        Self::Running(IoTaskPool::get().spawn(work))
    }

    /// The value, once the pool has it.
    fn poll(&mut self) -> Option<&T> {
        if let Self::Running(task) = self {
            let value = task.complete()?;
            *self = Self::Done(value);
        }
        match self {
            Self::Done(value) => Some(value),
            Self::Running(_) => None,
        }
    }
}

enum Kind {
    Shot {
        json: bool,
    },
    Sequence {
        count: usize,
        interval: f64,
        target: Target,
    },
}

struct CaptureJob {
    kind: Kind,
    options: Options,
    reply: Option<oneshot::Sender<Result<String, String>>>,
    started: f64,
    settle_frames: u32,
    /// when the camera was first seen in a transition
    blend_seen: Option<f64>,
    blend: Option<Blend>,
    hud_marker: Option<Entity>,
    frames: Vec<Slot>,
}

#[derive(Clone, Copy)]
struct Blend {
    waited_ms: u32,
    still_blending: bool,
}

struct Slot {
    scheduled: f64,
    at: f64,
    engine_frame: u32,
    tick: Option<u32>,
    state: SlotState,
}

enum SlotState {
    Reading(oneshot::Receiver<Image>),
    Encoding(Task<Result<Encoded, String>>),
    Done(Result<Encoded, String>),
}

struct Encoded {
    bytes: Vec<u8>,
    width: u32,
    height: u32,
    frame: UVec2,
    path: Option<String>,
}

impl CaptureJob {
    fn new(
        kind: Kind,
        options: Options,
        reply: oneshot::Sender<Result<String, String>>,
        now: f64,
        commands: &mut Commands,
    ) -> Self {
        let hud_marker = options
            .world_only
            .then(|| commands.spawn(WorldOnlyCapture).id());
        Self {
            kind,
            options,
            reply: Some(reply),
            started: now,
            settle_frames: SETTLE_FRAMES,
            blend_seen: None,
            blend: None,
            hud_marker,
            frames: Vec::new(),
        }
    }

    fn holder(&self) -> &'static str {
        match self.kind {
            Kind::Shot { .. } => "screenshot",
            Kind::Sequence { .. } => "screenshot --count",
        }
    }

    fn count(&self) -> usize {
        match self.kind {
            Kind::Shot { .. } => 1,
            Kind::Sequence { count, .. } => count,
        }
    }

    /// Whether the camera is where the frame should be taken from: at rest, or still in a
    /// transition `BLEND_DEADLINE_SECONDS` after the capture started. Records the wait once.
    fn camera_ready(&mut self, blending: bool, now: f64) -> bool {
        if self.blend.is_some() {
            return true;
        }
        if blending {
            let seen = *self.blend_seen.get_or_insert(now);
            if now - self.started < BLEND_DEADLINE_SECONDS {
                return false;
            }
            self.blend = Some(Blend {
                waited_ms: ((now - seen) * 1000.0) as u32,
                still_blending: true,
            });
        } else {
            self.blend = Some(Blend {
                waited_ms: self
                    .blend_seen
                    .map_or(0, |seen| ((now - seen) * 1000.0) as u32),
                still_blending: false,
            });
        }
        true
    }

    /// The folder of a sequence on disk.
    fn disk_folder(&mut self) -> Option<&mut Folder> {
        match &mut self.kind {
            Kind::Sequence {
                target: Target::Disk(folder),
                ..
            } => Some(folder),
            _ => None,
        }
    }

    /// Whether the job may ask for frames: `Ok(false)` while it waits out its settle frames, the
    /// camera's transition or the making of its folder. The folder is polled from the first
    /// frame, so a folder that cannot be made fails the capture at once.
    fn ready(&mut self, blending: bool, now: f64) -> Result<bool, String> {
        let has_folder = self.folder_made(now)?;
        if self.settle_frames > 0 {
            self.settle_frames -= 1;
            return Ok(false);
        }
        Ok(self.camera_ready(blending, now) && has_folder)
    }

    /// Whether a sequence on disk has its folder: `Ok(false)` while the IO task pool is making
    /// it, and a refusal when it could not be made or was not within
    /// `READBACK_TIMEOUT_SECONDS` of the capture's start. A capture with no folder has all it
    /// needs.
    fn folder_made(&mut self, now: f64) -> Result<bool, String> {
        let started = self.started;
        let Some(folder) = self.disk_folder() else {
            return Ok(true);
        };
        match folder.made.poll() {
            Some(Ok(())) => Ok(true),
            Some(Err(reason)) => Err(reply::refusal(reason.clone(), None)),
            None if now - started > READBACK_TIMEOUT_SECONDS => Err(reply::refusal(
                format!(
                    "{} was not made within {READBACK_TIMEOUT_SECONDS} s",
                    folder.path.display()
                ),
                None,
            )),
            None => Ok(false),
        }
    }

    /// The count of frame files in a sequence's folder that it did not write: `Ok(None)` while
    /// the IO task pool counts, 0 for a capture with no folder, and the failure when the count
    /// did not end within `READBACK_TIMEOUT_SECONDS` of being asked for. The first call, made
    /// once every frame is written, asks for it.
    fn stale_count(&mut self, now: f64) -> Result<Option<usize>, String> {
        let (count, format) = (self.count(), self.options.format);
        let Some(folder) = self.disk_folder() else {
            return Ok(Some(0));
        };
        let (asked, stale) = folder.stale.get_or_insert_with(|| {
            let path = folder.path.clone();
            let written: Vec<String> = (0..count).map(|index| frame_name(index, format)).collect();
            (now, Io::spawn(async move { stale_frames(&path, &written) }))
        });
        match stale.poll() {
            Some(&stale) => Ok(Some(stale)),
            None if now - *asked > READBACK_TIMEOUT_SECONDS => Err(format!(
                "{} was not searched within {READBACK_TIMEOUT_SECONDS} s",
                folder.path.display()
            )),
            None => Ok(None),
        }
    }

    /// When the next frame is due, if one is: the first at once, then one per interval from the
    /// first frame's time. None is due while `MAX_FRAMES_IN_FLIGHT` wait for the encoder.
    fn next_due(&self) -> Option<f64> {
        let in_flight = self
            .frames
            .iter()
            .filter(|slot| !matches!(slot.state, SlotState::Done(_)))
            .count();
        if self.frames.len() >= self.count() || in_flight >= MAX_FRAMES_IN_FLIGHT {
            return None;
        }
        match (&self.kind, self.frames.first()) {
            (Kind::Sequence { interval, .. }, Some(first)) => {
                Some(first.at + interval * self.frames.len() as f64)
            }
            _ => Some(f64::NEG_INFINITY),
        }
    }

    /// Where frame `index` is written and named.
    fn frame_path(&self, index: usize) -> Option<(String, Option<PathBuf>)> {
        let Kind::Sequence { target, .. } = &self.kind else {
            return None;
        };
        let name = frame_name(index, self.options.format);
        Some(match target {
            Target::Disk(folder) => {
                let file = folder.path.join(name);
                (file.display().to_string(), Some(file))
            }
            Target::Memory(sequence) => (format!("capture://{sequence}/{name}"), None),
        })
    }
}

/// The file name of a sequence's frame `index`.
fn frame_name(index: usize, format: Encoding) -> String {
    format!("frame-{index:03}.{}", format.extension())
}

/// What the primary camera carries of a transition.
type CameraTween = (Option<&'static SystemTween>, Has<SystemTweenData>);

/// Whether the camera is in a timed transition. A timed `SystemTween` counts from the frame it is
/// asked for, a frame before its `SystemTweenData` arrives, until both are removed at its end. An
/// instant cut leaves its `SystemTween` on the camera for good, and is no transition to wait for.
fn is_blending((tween, timed): (Option<&SystemTween>, bool)) -> bool {
    timed || tween.is_some_and(|tween| tween.time > 0.0)
}

#[allow(clippy::too_many_arguments)]
fn drive_capture(
    mut commands: Commands,
    mut gate: ResMut<CaptureGate>,
    mut store: ResMut<CaptureStore>,
    time: Res<Time<Real>>,
    frame_count: Res<FrameCount>,
    windows: Query<(Entity, &Window), With<PrimaryWindow>>,
    camera_tweens: Query<CameraTween, With<PrimaryCamera>>,
    mut scene_uis: ResMut<HiddenSceneUis>,
    player: Query<&GlobalTransform, With<PrimaryUser>>,
    resolver: SceneResolver,
) {
    let Some(job) = gate.job.as_mut() else {
        return;
    };
    let now = time.elapsed_secs_f64();
    let ready = match job.ready(camera_tweens.iter().any(is_blending), now) {
        Ok(ready) => ready,
        Err(reason) => {
            finish(
                &mut commands,
                &mut gate,
                &mut store,
                &mut scene_uis.reborrow(),
                Err(reason),
            );
            return;
        }
    };
    if job.options.world_only && !scene_uis.suspended {
        scene_uis.suspended = true;
    }
    if !ready {
        return;
    }

    let Ok((window, _)) = windows.single() else {
        finish(
            &mut commands,
            &mut gate,
            &mut store,
            &mut scene_uis.reborrow(),
            Err("no primary window to capture".to_owned()),
        );
        return;
    };
    if let Some(scheduled) = job.next_due().filter(|due| now >= *due) {
        let rx = frame::read_back(&mut commands, Screenshot::window(window));
        job.frames.push(Slot {
            // the first frame sets the schedule: it is never late
            scheduled: if scheduled.is_finite() {
                scheduled
            } else {
                now
            },
            at: now,
            engine_frame: frame_count.0,
            tick: resolver
                .resolve()
                .ok()
                .map(|(_, context)| context.tick_number),
            state: SlotState::Reading(rx),
        });
    }

    for index in 0..job.frames.len() {
        let path = job.frame_path(index);
        let (max_width, format) = (job.options.max_width, job.options.format);
        let slot = &mut job.frames[index];
        slot.state = match std::mem::replace(&mut slot.state, SlotState::Done(Err(String::new()))) {
            SlotState::Reading(mut rx) => match rx.try_recv() {
                Ok(image) => SlotState::Encoding(
                    IoTaskPool::get().spawn(async move { encode(image, max_width, format, path) }),
                ),
                Err(oneshot::error::TryRecvError::Empty) => SlotState::Reading(rx),
                Err(oneshot::error::TryRecvError::Closed) => {
                    SlotState::Done(Err("the frame was not read back".to_owned()))
                }
            },
            SlotState::Encoding(mut task) => match task.complete() {
                Some(result) => SlotState::Done(result),
                None => SlotState::Encoding(task),
            },
            done => done,
        };
    }

    let all_asked = job.frames.len() == job.count();
    let all_done = job
        .frames
        .iter()
        .all(|slot| matches!(slot.state, SlotState::Done(_)));
    let result = if all_asked && all_done {
        // a frame that failed is the reply, so the folder is not searched
        let failed = job
            .frames
            .iter()
            .any(|slot| matches!(slot.state, SlotState::Done(Err(_))));
        match if failed {
            Ok(Some(0))
        } else {
            job.stale_count(now)
        } {
            Ok(Some(stale)) => {
                let screen = windows.single().ok().map(|(_, window)| Screen::of(window));
                let parcel = player
                    .single()
                    .ok()
                    .map(|transform| vec3_to_parcel(transform.translation()));
                build_reply(job, screen, parcel, stale, &mut store)
            }
            Ok(None) => return,
            Err(reason) => Err(reason),
        }
    } else if job
        .frames
        .last()
        // a frame held back by `MAX_FRAMES_IN_FLIGHT` waits on the ones before it, so the
        // newest frame's age bounds the wait whether or not every frame was asked for
        .is_some_and(|last| now - last.at > READBACK_TIMEOUT_SECONDS)
    {
        Err(format!(
            "the frames were not read back within {READBACK_TIMEOUT_SECONDS} s of the last capture"
        ))
    } else {
        return;
    };
    finish(
        &mut commands,
        &mut gate,
        &mut store,
        &mut scene_uis.reborrow(),
        result,
    );
}

/// Reply, show what a world-only capture hid, and open the gate. `HiddenSceneUis` is written,
/// and so applied again, only when this capture suspended it.
fn finish(
    commands: &mut Commands,
    gate: &mut CaptureGate,
    store: &mut CaptureStore,
    scene_uis: &mut Mut<HiddenSceneUis>,
    result: Result<String, String>,
) {
    let Some(mut job) = gate.job.take() else {
        return;
    };
    // a capture that failed before its first frame never suspended it
    if job.options.world_only && scene_uis.suspended {
        scene_uis.suspended = false;
    }
    if let Some(marker) = job.hud_marker.take() {
        commands.entity(marker).despawn();
    }
    if result.is_err() {
        if let Kind::Sequence {
            target: Target::Memory(_),
            ..
        } = job.kind
        {
            store.frames.clear();
        }
    }
    if let Some(reply) = job.reply.take() {
        let _ = reply.send(result);
    }
}

/// Scale `image` to at most `max_width` wide and encode it, through `common::frame`; write it to
/// `path` when it has a file.
fn encode(
    image: Image,
    max_width: Option<u32>,
    format: Encoding,
    path: Option<(String, Option<PathBuf>)>,
) -> Result<Encoded, String> {
    let pixels = frame::pixels(image)?;
    let read = UVec2::new(pixels.width(), pixels.height());
    let scaled = max_width
        .map_or(Scale::Full, Scale::max_width)
        .apply(pixels);
    let bytes = frame::encode(&scaled, format)?;
    let path = match path {
        Some((name, Some(file))) => {
            std::fs::write(&file, &bytes)
                .map_err(|e| format!("cannot write {}: {e}", file.display()))?;
            Some(name)
        }
        Some((name, None)) => Some(name),
        None => None,
    };
    Ok(Encoded {
        width: scaled.width(),
        height: scaled.height(),
        bytes,
        frame: read,
        path,
    })
}

/// `640x325 (screen 1600x813, frame 3200x1626) full-view capture at parcel (149,149)`, then the
/// camera wait when there was one.
fn caption(
    size: UVec2,
    screen: Option<Screen>,
    frame: UVec2,
    world_only: bool,
    parcel: Option<IVec2>,
    blend: Option<Blend>,
) -> String {
    let screen = screen.map_or("no window".to_owned(), |s| {
        format!("screen {:.0}x{:.0}", s.width, s.height)
    });
    let mut caption = format!(
        "{}x{} ({screen}, frame {}x{}) {} capture",
        size.x,
        size.y,
        frame.x,
        frame.y,
        if world_only {
            "world-only"
        } else {
            "full-view"
        },
    );
    if let Some(parcel) = parcel {
        caption += &format!(" at parcel ({},{})", parcel.x, parcel.y);
    }
    match blend {
        Some(blend) if blend.still_blending => {
            caption += &format!(
                ", camera still blending after {} ms -- the frame may be mid-transition",
                blend.waited_ms
            )
        }
        Some(blend) if blend.waited_ms > 0 => {
            caption += &format!(", waited {} ms for the camera blend", blend.waited_ms)
        }
        _ => {}
    }
    caption
}

/// The web capture is the engine's canvas: the page's HUD is never in it.
fn web_note() -> Option<&'static str> {
    cfg!(target_arch = "wasm32")
        .then_some("the web capture is the engine's canvas: the page's HUD is never in it")
}

/// The reply to a capture whose frames are all done; `stale` is what `CaptureJob::stale_count`
/// counted.
fn build_reply(
    job: &mut CaptureJob,
    screen: Option<Screen>,
    parcel: Option<IVec2>,
    stale: usize,
    store: &mut CaptureStore,
) -> Result<String, String> {
    let mut encoded = Vec::with_capacity(job.frames.len());
    for (index, slot) in job.frames.iter_mut().enumerate() {
        match std::mem::replace(&mut slot.state, SlotState::Done(Err(String::new()))) {
            SlotState::Done(Ok(frame)) => encoded.push(frame),
            SlotState::Done(Err(e)) => return Err(format!("frame {index}: {e}")),
            _ => return Err(format!("frame {index} was not finished")),
        }
    }
    let world_only = job.options.world_only;
    let format = job.options.format;
    let screen_json = screen.map_or(Value::Null, |s| s.to_json());
    let parcel_json = parcel.map_or(Value::Null, |p| json!({ "x": p.x, "y": p.y }));
    match &job.kind {
        Kind::Shot { json } => {
            let shot = encoded.pop().ok_or("no frame was captured")?;
            if !json {
                return Ok(BASE64_STANDARD.encode(&shot.bytes));
            }
            let size = UVec2::new(shot.width, shot.height);
            let mut reply = json!({
                "caption": caption(size, screen, shot.frame, world_only, parcel, job.blend),
                "format": format.extension(),
                "width": shot.width,
                "height": shot.height,
                "frame": { "width": shot.frame.x, "height": shot.frame.y },
                "screen": screen_json,
                "parcel": parcel_json,
                "worldOnly": world_only,
                "blendWaitMs": job.blend.map_or(0, |b| b.waited_ms),
                "image": BASE64_STANDARD.encode(&shot.bytes),
            });
            if let Some(note) = web_note() {
                reply["info"] = Value::String(note.to_owned());
            }
            Ok(reply.to_string())
        }
        Kind::Sequence {
            count,
            interval,
            target,
        } => {
            let first_at = job.frames.first().map_or(0.0, |slot| slot.at);
            let mut late = 0;
            let mut frames = Vec::with_capacity(encoded.len());
            for (index, (slot, frame)) in job.frames.iter().zip(&encoded).enumerate() {
                let was_late = slot.at - slot.scheduled > interval / 2.0;
                late += was_late as u32;
                let previous = index
                    .checked_sub(1)
                    .map(|i| reply::round_fine((slot.at - job.frames[i].at) as f32));
                frames.push(json!({
                    "index": index,
                    "path": frame.path,
                    "width": frame.width,
                    "height": frame.height,
                    "bytes": frame.bytes.len(),
                    "atSeconds": reply::round_fine((slot.at - first_at) as f32),
                    "intervalFromPreviousSeconds": previous,
                    "engineFrame": slot.engine_frame,
                    "tick": slot.tick,
                    "late": was_late,
                }));
            }
            let mean = (*count > 1).then(|| {
                let last = job.frames.last().map_or(first_at, |slot| slot.at);
                reply::round_fine(((last - first_at) / (*count - 1) as f64) as f32)
            });
            let (directory, stored, read) = match target {
                Target::Disk(folder) => (
                    folder.path.display().to_string(),
                    "disk",
                    "frames are files, read only the ones you need",
                ),
                Target::Memory(_) => {
                    store.frames = encoded
                        .into_iter()
                        .filter_map(|frame| frame.path.map(|path| (path, frame.bytes)))
                        .collect();
                    (
                        store.directory(),
                        "memory",
                        "frames stay in memory until the next sequence; read one with /screenshot --frame <path>",
                    )
                }
            };
            let mut reply = json!({
                "directory": directory,
                "stored": stored,
                "count": count,
                "format": format.extension(),
                "worldOnly": world_only,
                "requestedIntervalSeconds": reply::round_fine(*interval as f32),
                "meanIntervalSeconds": mean,
                "late": late,
                "screen": screen_json,
                "parcel": parcel_json,
                "frames": frames,
                "note": format!(
                    "{count} {} frame(s); {read}",
                    if world_only { "world-only" } else { "full-view" }
                ),
            });
            if let Some(note) = web_note() {
                reply["info"] = Value::String(note.to_owned());
            }
            if stale > 0 {
                reply["staleFrames"] = json!(stale);
                reply["warning"] = Value::String(format!(
                    "{directory} also holds {stale} frame file(s) this sequence did not write; read only the paths in `frames`"
                ));
            }
            Ok(reply.to_string())
        }
    }
}

/// The `frame-*` files in `folder` whose names are not among `written`: an earlier sequence's,
/// left alone. Reads the folder, so it runs on the IO task pool.
fn stale_frames(folder: &Path, written: &[String]) -> usize {
    let Ok(entries) = std::fs::read_dir(folder) else {
        return 0;
    };
    entries
        .flatten()
        .filter_map(|entry| entry.file_name().into_string().ok())
        .filter(|name| name.starts_with("frame-") && !written.contains(name))
        .count()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> Result<ScreenshotCommand, clap::Error> {
        use clap::Parser;
        ScreenshotCommand::try_parse_from(
            std::iter::once("/screenshot").chain(args.iter().copied()),
        )
    }

    #[test]
    fn the_sequence_flags_need_a_count_and_the_reads_stand_alone() {
        assert!(parse(&[]).is_ok());
        assert!(parse(&["--max-width", "640", "--jpg", "--world-only"]).is_ok());
        let sequence = parse(&[
            "--count",
            "3",
            "--interval-seconds",
            "0.1",
            "--dir",
            "frames",
        ])
        .unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(sequence.count, Some(3));
        assert_eq!(sequence.interval_seconds, Some(0.1));
        assert!(parse(&["--interval-seconds", "0.1"]).is_err());
        assert!(parse(&["--dir", "frames"]).is_err());
        assert!(parse(&["--count", "21"]).is_err());
        assert!(parse(&["--frame", "capture://1/frame-000.png"]).is_ok());
        assert!(parse(&["--frame", "capture://1/frame-000.png", "--count", "2"]).is_err());
        assert!(parse(&["--frame", "capture://1/frame-000.png", "--state"]).is_err());
        assert!(parse(&["--state"]).is_ok());
        assert!(parse(&["--state", "--jpg"]).is_err());
    }

    #[test]
    fn a_stored_frame_is_read_by_its_path_and_the_state_counts_what_is_kept() {
        let store = CaptureStore {
            sequence: 2,
            frames: vec![("capture://2/frame-000.png".to_owned(), vec![1, 2, 3])],
        };
        assert_eq!(
            stored_frame(&store, "capture://2/frame-000.png"),
            Ok(BASE64_STANDARD.encode([1, 2, 3]))
        );
        let missing = stored_frame(&store, "capture://1/frame-000.png").unwrap_err();
        assert!(missing.contains("capture://2"), "{missing}");

        let state: Value =
            serde_json::from_str(&state_reply(&CaptureGate::default(), &store)).unwrap();
        assert_eq!(state["holder"], "none");
        assert_eq!(state["count"], 0);
        assert_eq!(state["framesInMemory"], 1);
        assert_eq!(state["bytesInMemory"], 3);
    }

    #[test]
    fn a_burst_longer_than_twenty_seconds_is_refused() {
        assert!(check_burst(20, 1.0).is_ok());
        assert!(check_burst(20, 1.1).is_err());
        assert!(check_burst(5, 0.01).is_err());
        assert!(check_burst(1, 2.0).is_ok());
    }

    #[test]
    fn the_caption_names_the_screen_the_frame_and_the_wait() {
        let screen = Screen {
            width: 1600.0,
            height: 813.0,
            scale: 2.0,
        };
        let text = caption(
            UVec2::new(640, 325),
            Some(screen),
            UVec2::new(3200, 1626),
            false,
            Some(IVec2::new(149, 149)),
            Some(Blend {
                waited_ms: 420,
                still_blending: false,
            }),
        );
        assert_eq!(
            text,
            "640x325 (screen 1600x813, frame 3200x1626) full-view capture at parcel (149,149), waited 420 ms for the camera blend"
        );
    }

    fn frame(width: u32, height: u32) -> Image {
        Image::new_fill(
            bevy::render::render_resource::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            bevy::render::render_resource::TextureDimension::D2,
            &[255, 0, 0, 255],
            bevy::render::render_resource::TextureFormat::Rgba8UnormSrgb,
            bevy::asset::RenderAssetUsages::default(),
        )
    }

    /// The scale rule itself is tested in `common::frame`: here, that a frame read back as an
    /// `Image` goes through it and the reply keeps the size it was read at.
    #[test]
    fn a_frame_read_back_is_scaled_and_encoded_and_keeps_its_read_size() {
        let jpeg = Encoding::Jpeg(frame::JPEG_QUALITY);
        let wide = encode(frame(320, 160), Some(64), jpeg, None).unwrap();
        assert_eq!((wide.width, wide.height), (64, 32));
        assert_eq!(wide.frame, UVec2::new(320, 160));
        assert_eq!(&wide.bytes[..2], &[0xFF, 0xD8]);
        assert!(wide.path.is_none());
    }

    fn job(kind: Kind, world_only: bool, world: &mut World) -> CaptureJob {
        let (tx, _rx) = oneshot::channel();
        let mut queue = bevy::ecs::world::CommandQueue::default();
        let job = CaptureJob::new(
            kind,
            Options {
                max_width: Some(320),
                format: Encoding::Png,
                world_only,
            },
            tx,
            0.0,
            &mut Commands::new(&mut queue, world),
        );
        queue.apply(world);
        job
    }

    fn sequence(count: usize, interval: f64) -> Kind {
        Kind::Sequence {
            count,
            interval,
            target: Target::Memory(1),
        }
    }

    #[test]
    fn an_instant_camera_cut_is_not_a_transition_to_wait_for() {
        let mut app = App::new();
        app.init_resource::<Time>();
        app.add_systems(Update, tween::update_system_tween);
        let tween = |time| SystemTween {
            target: Transform::from_xyz(1.0, 0.0, 0.0),
            time,
            fov: None,
        };
        let cut = app
            .world_mut()
            .spawn((Transform::default(), PrimaryCamera::default(), tween(0.0)))
            .id();
        let mut cameras = app
            .world_mut()
            .query_filtered::<CameraTween, With<PrimaryCamera>>();
        let mut blending = |app: &App| cameras.iter(app.world()).any(is_blending);
        assert!(!blending(&app));
        app.update();
        app.update();
        assert!(!blending(&app));
        assert!(app.world().get::<SystemTween>(cut).is_some());

        // a timed transition counts from the frame it is asked for, until it completes
        app.world_mut().entity_mut(cut).insert(tween(10.0));
        assert!(blending(&app));
        app.update();
        app.update();
        assert!(blending(&app));
        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(std::time::Duration::from_secs(11));
        app.update();
        assert!(!blending(&app));
        assert!(app.world().get::<SystemTween>(cut).is_none());
    }

    #[test]
    fn a_capture_waits_for_the_camera_until_the_deadline() {
        let mut world = World::new();
        let mut resting = job(Kind::Shot { json: true }, false, &mut world);
        assert!(resting.camera_ready(false, 0.1));
        assert_eq!(resting.blend.unwrap().waited_ms, 0);

        let mut blended = job(Kind::Shot { json: true }, false, &mut world);
        assert!(!blended.camera_ready(true, 0.1));
        assert!(blended.camera_ready(false, 0.5));
        let blend = blended.blend.unwrap();
        assert_eq!((blend.waited_ms, blend.still_blending), (400, false));

        let mut late = job(Kind::Shot { json: true }, false, &mut world);
        assert!(!late.camera_ready(true, 0.25));
        assert!(late.camera_ready(true, 1.5));
        let blend = late.blend.unwrap();
        assert_eq!((blend.waited_ms, blend.still_blending), (1250, true));
        // the caption and `blendWaitMs` report the same wait
        let text = caption(UVec2::ONE, None, UVec2::ONE, false, None, Some(blend));
        assert!(text
            .ends_with(", camera still blending after 1250 ms -- the frame may be mid-transition"));
    }

    #[test]
    fn a_sequence_counts_its_late_frames_and_its_mean_interval() {
        let mut world = World::new();
        let mut job = job(sequence(3, 0.1), false, &mut world);
        // the second frame is 0.02 s after its time (on time), the third 0.06 s (late)
        for (scheduled, at) in [(1.0, 1.0), (1.1, 1.12), (1.2, 1.26)] {
            let index = job.frames.len();
            let path = job.frame_path(index).map(|(name, _)| name);
            job.frames.push(Slot {
                scheduled,
                at,
                engine_frame: index as u32,
                tick: Some(7),
                state: SlotState::Done(Ok(Encoded {
                    bytes: vec![0; 4],
                    width: 320,
                    height: 160,
                    frame: UVec2::new(640, 320),
                    path,
                })),
            });
        }
        let mut store = CaptureStore {
            sequence: 1,
            frames: Vec::new(),
        };
        let reply: Value =
            serde_json::from_str(&build_reply(&mut job, None, None, 0, &mut store).unwrap())
                .unwrap();
        assert_eq!(reply["late"], 1);
        assert_eq!(reply["meanIntervalSeconds"], 0.13);
        assert_eq!(reply["stored"], "memory");
        assert_eq!(reply["directory"], "capture://1");
        let late: Vec<bool> = reply["frames"]
            .as_array()
            .unwrap()
            .iter()
            .map(|frame| frame["late"].as_bool().unwrap())
            .collect();
        assert_eq!(late, [false, false, true]);
        assert_eq!(
            reply["frames"][0]["intervalFromPreviousSeconds"],
            Value::Null
        );
        assert_eq!(reply["frames"][2]["path"], "capture://1/frame-002.png");
        assert_eq!(store.frames.len(), 3);
    }

    fn done(path: Option<String>) -> SlotState {
        SlotState::Done(Ok(Encoded {
            bytes: vec![0; 4],
            width: 320,
            height: 160,
            frame: UVec2::new(640, 320),
            path,
        }))
    }

    fn slot(at: f64, state: SlotState) -> Slot {
        Slot {
            scheduled: at,
            at,
            engine_frame: 0,
            tick: None,
            state,
        }
    }

    #[test]
    fn a_sequence_asks_for_no_frame_while_six_wait_for_the_encoder() {
        let mut world = World::new();
        let mut job = job(sequence(10, 0.1), false, &mut world);
        assert_eq!(job.next_due(), Some(f64::NEG_INFINITY));
        for index in 0..MAX_FRAMES_IN_FLIGHT {
            // a frame still being read back: its sender is kept so the channel stays open
            let (tx, rx) = oneshot::channel::<Image>();
            std::mem::forget(tx);
            job.frames
                .push(slot(1.0 + 0.1 * index as f64, SlotState::Reading(rx)));
        }
        assert_eq!(job.next_due(), None);

        // one frame encoded: the next is due at its own time on the first frame's schedule
        job.frames[0].state = done(None);
        let due = job.next_due().unwrap();
        assert!((due - 1.6).abs() < 1e-9, "{due}");

        // and none once every frame was asked for
        while job.frames.len() < 10 {
            job.frames.push(slot(2.0, done(None)));
        }
        assert_eq!(job.next_due(), None);
    }

    /// The IO task pool the capture's file work runs on; the app's `TaskPoolPlugin` makes it.
    fn io_pool() {
        IoTaskPool::get_or_init(bevy::tasks::TaskPool::new);
    }

    /// Polls until `ready` answers, as the capture system does once a frame.
    fn wait_for<T>(mut ready: impl FnMut() -> Option<T>) -> T {
        let give_up = std::time::Instant::now() + std::time::Duration::from_secs(30);
        loop {
            if let Some(value) = ready() {
                return value;
            }
            assert!(
                std::time::Instant::now() < give_up,
                "the IO task did not finish"
            );
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
    }

    /// A new, empty folder for one test, under the system's temporary folder.
    fn scratch(name: &str) -> PathBuf {
        let folder =
            std::env::temp_dir().join(format!("dcl-capture-test-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&folder);
        std::fs::create_dir_all(&folder).unwrap();
        folder
    }

    fn on_disk(count: usize, folder: Folder) -> Kind {
        Kind::Sequence {
            count,
            interval: 0.1,
            target: Target::Disk(folder),
        }
    }

    #[test]
    fn a_sequence_waits_for_its_folder_and_fails_when_it_cannot_be_made() {
        io_pool();
        let scratch = scratch("folder");
        let mut world = World::new();

        // a capture with no folder has nothing to wait for
        let mut shot = job(Kind::Shot { json: false }, false, &mut world);
        assert_eq!(shot.folder_made(0.0), Ok(true));
        let mut in_memory = job(sequence(2, 0.1), false, &mut world);
        assert_eq!(in_memory.folder_made(0.0), Ok(true));

        // a folder the pool makes, parents included
        let nested = scratch.join("a").join("b");
        let mut made = job(on_disk(2, Folder::make(nested.clone())), false, &mut world);
        assert!(wait_for(|| made.folder_made(0.0).unwrap().then_some(true)));
        assert!(nested.is_dir());

        // a folder that cannot be made: its parent is a file
        let file = scratch.join("file");
        std::fs::write(&file, b"x").unwrap();
        let mut blocked = job(
            on_disk(2, Folder::make(file.join("frames"))),
            false,
            &mut world,
        );
        let refusal = wait_for(|| match blocked.folder_made(0.0) {
            Ok(false) => None,
            other => Some(other),
        })
        .unwrap_err();
        let refusal: Value = serde_json::from_str(&refusal).unwrap();
        let reason = refusal["refused"].as_str().unwrap();
        assert!(reason.starts_with("cannot create "), "{reason}");
        assert!(reason.contains("frames"), "{reason}");

        // a folder the pool never answers for: the wait ends at the limit
        let stuck = Folder {
            path: scratch.join("stuck"),
            made: Io::spawn(std::future::pending()),
            stale: None,
        };
        let mut stuck = job(on_disk(2, stuck), false, &mut world);
        assert_eq!(stuck.folder_made(READBACK_TIMEOUT_SECONDS), Ok(false));
        // the same refusal shape as a folder that cannot be made
        let late = stuck
            .folder_made(READBACK_TIMEOUT_SECONDS + 0.1)
            .unwrap_err();
        let late: Value = serde_json::from_str(&late).unwrap();
        let reason = late["refused"].as_str().unwrap();
        assert!(reason.ends_with("was not made within 10 s"), "{reason}");

        std::fs::remove_dir_all(&scratch).unwrap();
    }

    #[test]
    fn a_sequence_asks_for_no_frame_before_its_folder_is_made() {
        io_pool();
        let mut world = World::new();
        let waiting = Folder {
            path: PathBuf::from("frames"),
            made: Io::spawn(std::future::pending()),
            stale: None,
        };
        let mut job = job(on_disk(2, waiting), false, &mut world);
        // past the settle frames, with the camera at rest, the folder still holds it back
        for _ in 0..SETTLE_FRAMES {
            assert_eq!(job.ready(false, 0.1), Ok(false));
        }
        assert_eq!(job.ready(false, 0.1), Ok(false));
        assert!(job.blend.is_some());

        let Some(folder) = job.disk_folder() else {
            unreachable!()
        };
        folder.made = Io::Done(Ok(()));
        assert_eq!(job.ready(false, 0.2), Ok(true));
    }

    #[test]
    fn the_frames_an_earlier_sequence_left_are_counted_before_the_reply() {
        io_pool();
        let scratch = scratch("stale");
        for name in [
            "frame-000.png",
            "frame-001.png",
            "frame-002.png",
            "frame-000.jpg",
            "notes.txt",
        ] {
            std::fs::write(scratch.join(name), b"x").unwrap();
        }
        let folder = Folder {
            path: scratch.clone(),
            made: Io::Done(Ok(())),
            stale: None,
        };
        let mut world = World::new();
        let mut job = job(on_disk(2, folder), false, &mut world);
        for index in 0..2 {
            let path = job.frame_path(index).map(|(name, _)| name);
            job.frames.push(slot(1.0 + 0.1 * index as f64, done(path)));
        }

        // the first call asks the pool; the reply is built only once the count is in
        let stale = wait_for(|| job.stale_count(0.0).unwrap());
        let mut store = CaptureStore::default();
        let reply: Value =
            serde_json::from_str(&build_reply(&mut job, None, None, stale, &mut store).unwrap())
                .unwrap();
        // the third png and the jpg are an earlier sequence's; the text file is no frame
        assert_eq!(reply["staleFrames"], 2);
        assert_eq!(reply["stored"], "disk");
        assert_eq!(reply["directory"], scratch.display().to_string());
        assert_eq!(
            reply["frames"][1]["path"],
            scratch.join("frame-001.png").display().to_string()
        );
        let warning = reply["warning"].as_str().unwrap();
        assert!(warning.contains("also holds 2 frame file(s)"), "{warning}");
        assert!(store.frames.is_empty());

        // a folder that holds only this sequence's frames carries neither field
        std::fs::remove_file(scratch.join("frame-002.png")).unwrap();
        std::fs::remove_file(scratch.join("frame-000.jpg")).unwrap();
        let clean = Folder {
            path: scratch.clone(),
            made: Io::Done(Ok(())),
            stale: None,
        };
        let mut job = self::job(on_disk(2, clean), false, &mut world);
        for index in 0..2 {
            let path = job.frame_path(index).map(|(name, _)| name);
            job.frames.push(slot(1.0 + 0.1 * index as f64, done(path)));
        }
        let stale = wait_for(|| job.stale_count(0.0).unwrap());
        let reply: Value =
            serde_json::from_str(&build_reply(&mut job, None, None, stale, &mut store).unwrap())
                .unwrap();
        assert!(reply.get("staleFrames").is_none() && reply.get("warning").is_none());

        // a capture with no folder has nothing to count
        let mut in_memory = self::job(sequence(2, 0.1), false, &mut world);
        assert_eq!(in_memory.stale_count(0.0), Ok(Some(0)));

        std::fs::remove_dir_all(&scratch).unwrap();
    }

    #[test]
    fn the_count_of_earlier_frames_has_its_own_time_limit() {
        io_pool();
        let mut world = World::new();
        let folder = Folder {
            path: PathBuf::from("frames"),
            made: Io::Done(Ok(())),
            stale: Some((5.0, Io::spawn(std::future::pending()))),
        };
        let mut job = job(on_disk(2, folder), false, &mut world);
        // the limit runs from when the count was asked for, not from the last frame
        assert_eq!(job.stale_count(5.0 + READBACK_TIMEOUT_SECONDS), Ok(None));
        let late = job.stale_count(5.1 + READBACK_TIMEOUT_SECONDS).unwrap_err();
        assert!(late.ends_with("was not searched within 10 s"), "{late}");
    }

    /// Runs `finish` on a world-only or full-view sequence with `result`, the scene UI suspended
    /// when `suspended`; returns the reply, whether the scene UI is still suspended, whether
    /// `HiddenSceneUis` was written, and the HUD markers left.
    fn finished(
        world_only: bool,
        suspended: bool,
        result: Result<String, String>,
    ) -> (Result<String, String>, bool, bool, usize) {
        let mut world = World::new();
        world.insert_resource(HiddenSceneUis {
            suspended,
            ..Default::default()
        });
        let (tx, mut rx) = oneshot::channel();
        let mut queue = bevy::ecs::world::CommandQueue::default();
        let job = CaptureJob::new(
            sequence(2, 0.1),
            Options {
                max_width: None,
                format: Encoding::Png,
                world_only,
            },
            tx,
            0.0,
            &mut Commands::new(&mut queue, &world),
        );
        queue.apply(&mut world);
        let mut gate = CaptureGate { job: Some(job) };
        let mut store = CaptureStore {
            sequence: 1,
            frames: vec![("capture://1/frame-000.png".to_owned(), vec![1])],
        };
        let failed = result.is_err();
        // from here on, a change to the rule is `finish` writing it
        world.clear_trackers();
        let (suspended, written) =
            world.resource_scope(|world, mut scene_uis: Mut<HiddenSceneUis>| {
                finish(
                    &mut Commands::new(&mut queue, world),
                    &mut gate,
                    &mut store,
                    &mut scene_uis,
                    result,
                );
                (scene_uis.suspended, scene_uis.is_changed())
            });
        queue.apply(&mut world);
        assert!(gate.job.is_none());
        assert_eq!(store.frames.is_empty(), failed);
        let markers = world.query::<&WorldOnlyCapture>().iter(&world).count();
        (rx.try_recv().unwrap(), suspended, written, markers)
    }

    #[test]
    fn a_capture_that_fails_or_times_out_still_opens_the_gate_and_shows_the_ui() {
        let timeout =
            Err("the frames were not read back within 10 s of the last capture".to_owned());
        let (reply, suspended, written, markers) = finished(true, true, timeout.clone());
        assert_eq!(reply, timeout);
        assert!(!suspended && written);
        assert_eq!(markers, 0);

        let (reply, suspended, written, markers) = finished(true, true, Ok("{}".to_owned()));
        assert_eq!(reply, Ok("{}".to_owned()));
        assert!(!suspended && written);
        assert_eq!(markers, 0);
    }

    #[test]
    fn a_capture_that_never_suspended_the_scene_ui_does_not_write_its_rule() {
        let (reply, suspended, written, _) =
            finished(false, false, Err("no primary window".to_owned()));
        assert!(reply.is_err());
        assert!(!suspended && !written);

        // a world-only capture whose folder failed before its first frame
        let (reply, suspended, written, markers) = finished(
            true,
            false,
            Err(r#"{"refused":"cannot create frames"}"#.to_owned()),
        );
        assert!(reply.is_err());
        assert!(!suspended && !written);
        assert_eq!(markers, 0);
    }

    #[test]
    fn the_gate_names_its_holder() {
        let world = World::new();
        let (tx, _rx) = oneshot::channel();
        let mut queue = bevy::ecs::world::CommandQueue::default();
        let mut commands = Commands::new(&mut queue, &world);
        let job = CaptureJob::new(
            Kind::Sequence {
                count: 3,
                interval: 0.1,
                target: Target::Memory(1),
            },
            Options {
                max_width: Some(320),
                format: Encoding::Png,
                world_only: false,
            },
            tx,
            0.0,
            &mut commands,
        );
        let gate = CaptureGate { job: Some(job) };
        let refusal = gate.refusal().unwrap();
        let refusal: Value = serde_json::from_str(&refusal).unwrap();
        assert_eq!(
            refusal["refused"],
            "a capture is already in progress (screenshot --count); retry when it completes"
        );
        assert_eq!(refusal["cause"]["holder"], "screenshot --count");
        assert!(CaptureGate::default().refusal().is_none());
    }
}
