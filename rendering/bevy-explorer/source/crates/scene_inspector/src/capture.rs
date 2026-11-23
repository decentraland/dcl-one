//! `/screenshot` and `/capture_sequence`: frames of the primary window, read back at the end of
//! the frame they were rendered in, through one capture job at a time (the gate; a second capture
//! is refused and the refusal names the holder). A job waits two frames, then for the camera's
//! transition (at most 1.5 s), then captures; the frames are scaled and encoded on the IO task
//! pool, which is off the main thread natively and on the engine's own thread on the web.
//! `--world-only` hides the native HUD and the scenes' window UI while the job runs, the scene UI
//! through `HiddenSceneUis`, the rule `/show_ui` uses.

use std::path::PathBuf;

use base64::{prelude::BASE64_STANDARD, Engine};
use bevy::{
    diagnostic::FrameCount,
    prelude::*,
    render::view::screenshot::{Screenshot, ScreenshotCaptured},
    tasks::{IoTaskPool, Task},
    window::PrimaryWindow,
};
use bevy_console::ConsoleCommand;
use common::{
    structs::{PrimaryCamera, PrimaryUser},
    util::TaskExt,
};
use console::{
    reply::{self, Screen},
    AddDebugSection, DebugSection, DoAddConsoleCommand, PendingConsoleResponses,
};
use image::{codecs::jpeg::JpegEncoder, DynamicImage};
use scene_runner::{update_world::scene_ui::HiddenSceneUis, vec3_to_parcel};
use serde_json::{json, Value};
use tokio::sync::oneshot;
use tween::{SystemTween, SystemTweenData};

use crate::active_scene::SceneResolver;

const JPG_QUALITY: u8 = 75;
/// Frames a capture waits before it looks at the camera, so a change the command's own frame
/// started (a virtual camera handing back) has begun its transition.
const SETTLE_FRAMES: u32 = 2;
const BLEND_DEADLINE_SECONDS: f64 = 1.5;
const SEQUENCE_MAX_BURST_SECONDS: f32 = 20.0;
/// How long a job may wait for its frames after the last was asked for.
const READBACK_TIMEOUT_SECONDS: f64 = 10.0;

pub(crate) struct CapturePlugin;

impl Plugin for CapturePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CaptureGate>();
        app.init_resource::<CaptureStore>();
        app.add_console_command::<ScreenshotCommand, _>(screenshot_cmd);
        app.add_console_command::<CaptureSequenceCommand, _>(capture_sequence_cmd);
        app.add_console_command::<CaptureFrameCommand, _>(capture_frame_cmd);
        app.add_console_command::<CaptureStateCommand, _>(capture_state_cmd)
            .add_debug_section(DebugSection {
                title: "Frame capture",
                query: "/capture_state",
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
/// at full size (no `data:` prefix; the editor host's thumbnail contract). With any option it is
/// JSON: `caption` (the image size, the screen and frame it was scaled from, the parcel, the
/// camera wait), `format`, `width`, `height`, `frame`, `screen`, `parcel`, `worldOnly`,
/// `blendWaitMs`, `image` (base64). Every capture, the bare one included, waits two frames and
/// then for a camera transition (at most 1.5 s), and is refused while another capture runs.
#[derive(clap::Parser, ConsoleCommand)]
#[command(name = "/screenshot")]
struct ScreenshotCommand {
    /// scale the image down to at most this many pixels wide (64 to 1920)
    #[arg(long, value_parser = clap::value_parser!(u32).range(64..=1920))]
    max_width: Option<u32>,
    /// encode as JPEG (quality 75) instead of PNG
    #[arg(long)]
    jpg: bool,
    /// hide the HUD and the scenes' window UI for the captured frame
    #[arg(long)]
    world_only: bool,
}

fn screenshot_cmd(
    mut input: ConsoleCommand<ScreenshotCommand>,
    mut commands: Commands,
    mut gate: ResMut<CaptureGate>,
    time: Res<Time<Real>>,
    mut console_responses: ResMut<PendingConsoleResponses>,
) {
    let Some(Ok(cmd)) = input.take() else {
        return;
    };
    if let Some(refusal) = gate.refusal() {
        input.reply_failed(refusal);
        return;
    }
    let json = cmd.max_width.is_some() || cmd.jpg || cmd.world_only;
    let (tx, rx) = oneshot::channel();
    gate.job = Some(CaptureJob::new(
        Kind::Shot { json },
        Options {
            max_width: cmd.max_width,
            format: Format::from_jpg(cmd.jpg),
            world_only: cmd.world_only,
        },
        tx,
        time.elapsed_secs_f64(),
        &mut commands,
    ));
    console_responses.push_oneshot(rx, |r| r, input.take_responder());
}

// --- /capture_sequence ---

/// Capture `count` frames `interval-seconds` apart, scheduled from the first frame so encoding
/// never delays the next. Natively the frames are files under `--dir` (default: a new folder in
/// the client's cache); a `--dir` that already holds frames is written over from `frame-000`, the
/// rest left alone and counted in `staleFrames`. On the web they stay in memory, the last sequence
/// only, and `/capture_frame <path>` reads one. JSON reply: `directory` (absolute), `stored`
/// (disk or memory), `count`,
/// `requestedIntervalSeconds`, `meanIntervalSeconds`, `late` (frames taken more than half an
/// interval after their time), `screen`, `frames` (`path`, `tick` of the scene, `engineFrame`,
/// `atSeconds`, `late`, ...). No image is inline. Shares the gate with `/screenshot`.
#[derive(clap::Parser, ConsoleCommand)]
#[command(name = "/capture_sequence")]
struct CaptureSequenceCommand {
    /// frames to capture (1 to 20; the burst may span at most 20 s)
    #[arg(long, default_value_t = 5, value_parser = clap::value_parser!(u32).range(1..=20))]
    count: u32,
    /// seconds between frames (0.05 to 2)
    #[arg(long, default_value_t = 0.5)]
    interval_seconds: f32,
    /// scale each frame down to at most this many pixels wide (64 to 1920)
    #[arg(long, default_value_t = 640, value_parser = clap::value_parser!(u32).range(64..=1920))]
    max_width: u32,
    /// encode as JPEG (quality 75) instead of PNG
    #[arg(long)]
    jpg: bool,
    /// hide the HUD and the scenes' window UI while the sequence runs
    #[arg(long)]
    world_only: bool,
    /// folder to write the frames to (native only; created if missing)
    #[arg(long)]
    dir: Option<PathBuf>,
}

fn capture_sequence_cmd(
    mut input: ConsoleCommand<CaptureSequenceCommand>,
    mut commands: Commands,
    mut gate: ResMut<CaptureGate>,
    mut store: ResMut<CaptureStore>,
    time: Res<Time<Real>>,
    mut console_responses: ResMut<PendingConsoleResponses>,
) {
    let Some(Ok(cmd)) = input.take() else {
        return;
    };
    if let Some(refusal) = gate.refusal() {
        input.reply_failed(refusal);
        return;
    }
    if let Err(reason) = check_burst(cmd.count, cmd.interval_seconds) {
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
    let (tx, rx) = oneshot::channel();
    gate.job = Some(CaptureJob::new(
        Kind::Sequence {
            count: cmd.count as usize,
            interval: cmd.interval_seconds as f64,
            target,
        },
        Options {
            max_width: Some(cmd.max_width),
            format: Format::from_jpg(cmd.jpg),
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
/// web, which cannot write the host's disk.
fn capture_target(dir: Option<PathBuf>, store: &mut CaptureStore) -> Result<Target, String> {
    let folder = match dir {
        Some(dir) if cfg!(target_arch = "wasm32") => {
            return Err(format!(
                "the web client cannot write files ({}); omit --dir, the frames stay in memory for /capture_frame",
                dir.display()
            ))
        }
        Some(dir) => Some(dir),
        None => platform::project_directories().map(|dirs| {
            let stamp = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis())
                .unwrap_or_default();
            dirs.cache_dir()
                .join("captures")
                .join(format!("capture-{stamp}"))
        }),
    };
    match folder {
        Some(folder) => {
            std::fs::create_dir_all(&folder)
                .map_err(|e| format!("cannot create {}: {e}", folder.display()))?;
            // a relative `--dir` is the client's working directory's: the reply names it in full
            Ok(Target::Disk(std::path::absolute(&folder).unwrap_or(folder)))
        }
        None => {
            store.sequence += 1;
            store.frames.clear();
            Ok(Target::Memory(store.sequence))
        }
    }
}

// --- /capture_frame ---

/// One frame of the last in-memory `/capture_sequence` (the web client's), by the `path` its reply
/// gave, as raw base64.
#[derive(clap::Parser, ConsoleCommand)]
#[command(name = "/capture_frame")]
struct CaptureFrameCommand {
    /// the frame's `path`, `capture://<sequence>/frame-<index>.<ext>`
    path: String,
}

fn capture_frame_cmd(mut input: ConsoleCommand<CaptureFrameCommand>, store: Res<CaptureStore>) {
    let Some(Ok(cmd)) = input.take() else {
        return;
    };
    match store.frames.iter().find(|(path, _)| *path == cmd.path) {
        Some((_, bytes)) => input.reply_ok(BASE64_STANDARD.encode(bytes)),
        None => input.reply_failed(format!(
            "no frame {} in memory: only the last in-memory sequence ({}) is kept",
            cmd.path,
            store.directory()
        )),
    }
}

// --- /capture_state ---

/// The capture gate, for the debug panel: `holder` (`screenshot`, `capture_sequence` or `none`),
/// `framesTaken` and `count` of the running capture, `worldOnly`, and the `framesInMemory` and
/// `bytesInMemory` the last in-memory sequence keeps.
#[derive(clap::Parser, ConsoleCommand)]
#[command(name = "/capture_state")]
struct CaptureStateCommand;

fn capture_state_cmd(
    mut input: ConsoleCommand<CaptureStateCommand>,
    gate: Res<CaptureGate>,
    store: Res<CaptureStore>,
) {
    if let Some(Ok(_)) = input.take() {
        let job = gate.job.as_ref();
        input.reply_ok(
            json!({
                "holder": job.map_or("none", CaptureJob::holder),
                "framesTaken": job.map_or(0, |job| job.frames.len()),
                "count": job.map_or(0, CaptureJob::count),
                "worldOnly": job.is_some_and(|job| job.options.world_only),
                "framesInMemory": store.frames.len(),
                "bytesInMemory": store.frames.iter().map(|(_, bytes)| bytes.len()).sum::<usize>(),
            })
            .to_string(),
        );
    }
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

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Format {
    Png,
    Jpg,
}

impl Format {
    fn from_jpg(jpg: bool) -> Self {
        if jpg {
            Self::Jpg
        } else {
            Self::Png
        }
    }

    fn name(self) -> &'static str {
        match self {
            Self::Png => "png",
            Self::Jpg => "jpg",
        }
    }
}

struct Options {
    max_width: Option<u32>,
    format: Format,
    world_only: bool,
}

enum Target {
    Disk(PathBuf),
    Memory(u32),
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
            Kind::Sequence { .. } => "capture_sequence",
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

    /// When the next frame is due, if one is: the first at once, then one per interval from the
    /// first frame's time.
    fn next_due(&self) -> Option<f64> {
        if self.frames.len() >= self.count() {
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
        let name = format!("frame-{index:03}.{}", self.options.format.name());
        Some(match target {
            Target::Disk(folder) => {
                let file = folder.join(name);
                (file.display().to_string(), Some(file))
            }
            Target::Memory(sequence) => (format!("capture://{sequence}/{name}"), None),
        })
    }
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
    if job.options.world_only && !scene_uis.suspended {
        scene_uis.suspended = true;
    }
    if job.settle_frames > 0 {
        job.settle_frames -= 1;
        return;
    }
    if !job.camera_ready(camera_tweens.iter().any(is_blending), now) {
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
        let (tx, rx) = oneshot::channel();
        let mut tx = Some(tx);
        commands.spawn(Screenshot::window(window)).observe(
            move |mut trigger: Trigger<ScreenshotCaptured>| {
                if let Some(tx) = tx.take() {
                    let _ = tx.send(std::mem::take(&mut trigger.0));
                }
            },
        );
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
    if all_asked && all_done {
        let screen = windows.single().ok().map(|(_, window)| Screen::of(window));
        let parcel = player
            .single()
            .ok()
            .map(|transform| vec3_to_parcel(transform.translation()));
        let result = build_reply(job, screen, parcel, &mut store);
        finish(
            &mut commands,
            &mut gate,
            &mut store,
            &mut scene_uis.reborrow(),
            result,
        );
    } else if job
        .frames
        .last()
        .is_some_and(|last| all_asked && now - last.at > READBACK_TIMEOUT_SECONDS)
    {
        finish(
            &mut commands,
            &mut gate,
            &mut store,
            &mut scene_uis.reborrow(),
            Err(format!(
                "the frames were not read back within {READBACK_TIMEOUT_SECONDS} s of the last capture"
            )),
        );
    }
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
    if job.options.world_only {
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

/// Scale `image` to at most `max_width` wide and encode it; write it to `path` when it has a file.
fn encode(
    image: Image,
    max_width: Option<u32>,
    format: Format,
    path: Option<(String, Option<PathBuf>)>,
) -> Result<Encoded, String> {
    let dynamic = image
        .try_into_dynamic()
        .map_err(|e| format!("the frame cannot be read: {e:?}"))?;
    let frame = UVec2::new(dynamic.width(), dynamic.height());
    let scaled = match max_width.filter(|max| frame.x > *max) {
        Some(max) => dynamic.thumbnail(max, u32::MAX),
        None => dynamic,
    };
    let bytes = encode_image(&scaled, format)?;
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
        frame,
        path,
    })
}

/// The one encoder of a captured frame: `/texture_camera_screenshot` encodes through it too.
pub(crate) fn encode_image(image: &DynamicImage, format: Format) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    match format {
        Format::Png => image
            .write_to(
                &mut std::io::Cursor::new(&mut bytes),
                image::ImageFormat::Png,
            )
            .map_err(|e| format!("PNG encode failed: {e}"))?,
        Format::Jpg => JpegEncoder::new_with_quality(&mut bytes, JPG_QUALITY)
            .encode_image(&image.to_rgb8())
            .map_err(|e| format!("JPEG encode failed: {e}"))?,
    }
    Ok(bytes)
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

fn build_reply(
    job: &mut CaptureJob,
    screen: Option<Screen>,
    parcel: Option<IVec2>,
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
                "format": format.name(),
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
            let stale = match target {
                Target::Disk(folder) => stale_frames(folder, &encoded),
                Target::Memory(_) => 0,
            };
            let (directory, stored) = match target {
                Target::Disk(folder) => (folder.display().to_string(), "disk"),
                Target::Memory(_) => {
                    store.frames = encoded
                        .into_iter()
                        .filter_map(|frame| frame.path.map(|path| (path, frame.bytes)))
                        .collect();
                    (store.directory(), "memory")
                }
            };
            let read = match target {
                Target::Disk(_) => "frames are files, read only the ones you need",
                Target::Memory(_) => {
                    "frames stay in memory until the next sequence; read one with /capture_frame <path>"
                }
            };
            let mut reply = json!({
                "directory": directory,
                "stored": stored,
                "count": count,
                "format": format.name(),
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

/// The `frame-*` files in `folder` that are not among `written`: an earlier sequence's, left alone.
fn stale_frames(folder: &std::path::Path, written: &[Encoded]) -> usize {
    let Ok(entries) = std::fs::read_dir(folder) else {
        return 0;
    };
    entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|file| {
            file.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with("frame-"))
                && !written
                    .iter()
                    .any(|frame| frame.path.as_deref() == file.to_str())
        })
        .count()
}

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn a_frame_is_scaled_down_to_the_width_never_up_and_encoded() {
        let wide = encode(frame(320, 160), Some(64), Format::Jpg, None).unwrap();
        assert_eq!((wide.width, wide.height), (64, 32));
        assert_eq!(wide.frame, UVec2::new(320, 160));
        assert_eq!(&wide.bytes[..2], &[0xFF, 0xD8]);

        let narrow = encode(frame(48, 24), Some(64), Format::Png, None).unwrap();
        assert_eq!((narrow.width, narrow.height), (48, 24));
        assert_eq!(&narrow.bytes[1..4], b"PNG");

        let full = encode(frame(320, 160), None, Format::Png, None).unwrap();
        assert_eq!((full.width, full.height), (320, 160));
    }

    fn job(kind: Kind, world_only: bool, world: &mut World) -> CaptureJob {
        let (tx, _rx) = oneshot::channel();
        let mut queue = bevy::ecs::world::CommandQueue::default();
        let job = CaptureJob::new(
            kind,
            Options {
                max_width: Some(320),
                format: Format::Png,
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
            serde_json::from_str(&build_reply(&mut job, None, None, &mut store).unwrap()).unwrap();
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

    /// Runs `finish` on a world-only or full-view sequence with `result`; returns the reply,
    /// whether the scene UI is still suspended, whether `HiddenSceneUis` was written, and the HUD
    /// markers left.
    fn finished(
        world_only: bool,
        result: Result<String, String>,
    ) -> (Result<String, String>, bool, bool, usize) {
        let mut world = World::new();
        world.insert_resource(HiddenSceneUis {
            suspended: world_only,
            ..Default::default()
        });
        let (tx, mut rx) = oneshot::channel();
        let mut queue = bevy::ecs::world::CommandQueue::default();
        let job = CaptureJob::new(
            sequence(2, 0.1),
            Options {
                max_width: None,
                format: Format::Png,
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
        let (reply, suspended, written, markers) = finished(true, timeout.clone());
        assert_eq!(reply, timeout);
        assert!(!suspended && written);
        assert_eq!(markers, 0);

        let (reply, suspended, written, markers) = finished(true, Ok("{}".to_owned()));
        assert_eq!(reply, Ok("{}".to_owned()));
        assert!(!suspended && written);
        assert_eq!(markers, 0);
    }

    #[test]
    fn a_full_view_capture_does_not_write_the_scene_ui_rule() {
        let (reply, suspended, written, _) = finished(false, Err("no primary window".to_owned()));
        assert!(reply.is_err());
        assert!(!suspended && !written);
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
                format: Format::Png,
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
            "a capture is already in progress (capture_sequence); retry when it completes"
        );
        assert_eq!(refusal["cause"]["holder"], "capture_sequence");
        assert!(CaptureGate::default().refusal().is_none());
    }
}
