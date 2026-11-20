//! MEASUREMENT ONLY (local measurement build, not for merge): main-thread time of the
//! movement systems and the frame's own breakdown, one row per frame while recording. Identical
//! file in the `main` and the PR builds; the hooks are spans at the start of `apply_movement`,
//! `send_scene_updates` and `receive_scene_updates` (no logic change).
//!   /cpu_probe start    clear and record every frame (cap 20000 rows)
//!   /cpu_probe stop     stop recording
//!   /cpu_probe fetch    {"cols":[..],"rows":[[..],..]} of the recorded frames
//!   /cpu_probe status   rows recorded, recording or not

use std::sync::atomic::{AtomicU64, Ordering};

use bevy::prelude::*;
use bevy_console::ConsoleCommand;
use common::{perf::FrameTimeBreakdown, structs::PrimaryUser};
use console::DoAddConsoleCommand;
use web_time::Instant;

use crate::SceneLoopTiming;

#[derive(Clone, Copy)]
pub enum Bucket {
    /// `apply_movement`: velocity, collide-and-slide and the ground assist.
    ApplyMovement,
    /// `send_scene_updates`, every call (one scene at most per call).
    Send,
    /// `send_scene_updates` calls that dispatched the movement scene.
    SendMovement,
    /// `receive_scene_updates`, every call.
    Receive,
    /// The movement scene's `SceneResponse::Ok` handling in `receive_scene_updates`.
    ReceiveMovement,
}

const N: usize = 5;
pub const NAMES: [&str; N] = ["apply", "send", "send_move", "recv", "recv_move"];

static NANOS: [AtomicU64; N] = [const { AtomicU64::new(0) }; N];
static COUNT: [AtomicU64; N] = [const { AtomicU64::new(0) }; N];

pub fn add(bucket: Bucket, since: Instant) {
    let ns = since.elapsed().as_nanos() as u64;
    NANOS[bucket as usize].fetch_add(ns, Ordering::Relaxed);
    COUNT[bucket as usize].fetch_add(1, Ordering::Relaxed);
}

/// Adds its lifetime to the bucket when dropped.
pub struct Span(Bucket, Instant);

impl Span {
    pub fn new(bucket: Bucket) -> Self {
        Self(bucket, Instant::now())
    }
}

impl Drop for Span {
    fn drop(&mut self) {
        add(self.0, self.1);
    }
}

const COLS: &[&str] = &[
    "frame",
    "dt_ms",
    "apply_us",
    "apply_n",
    "send_us",
    "send_n",
    "send_move_us",
    "send_move_n",
    "recv_us",
    "recv_n",
    "recv_move_us",
    "recv_move_n",
    "loop_ms",
    "loop_runs",
    "mw_ms",
    "rw_ms",
    "wait_ms",
    "gpu_ms",
    "x",
    "y",
    "z",
];
const MAX_ROWS: usize = 20000;

#[derive(Resource, Default)]
struct FrameLog {
    recording: bool,
    frame: u64,
    rows: Vec<[f32; 21]>,
}

pub struct CpuProbePlugin;

impl Plugin for CpuProbePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<FrameLog>()
            .add_systems(Last, record_frame)
            .add_console_command::<CpuProbeCommand, _>(cpu_probe_command);
    }
}

fn record_frame(
    mut log: ResMut<FrameLog>,
    time: Res<Time<Real>>,
    scene_loop: Option<Res<SceneLoopTiming>>,
    breakdown: Option<Res<FrameTimeBreakdown>>,
    player: Query<&GlobalTransform, With<PrimaryUser>>,
) {
    let mut us = [0f32; N];
    let mut n = [0f32; N];
    for i in 0..N {
        us[i] = NANOS[i].swap(0, Ordering::Relaxed) as f32 / 1000.0;
        n[i] = COUNT[i].swap(0, Ordering::Relaxed) as f32;
    }
    log.frame += 1;
    if !log.recording || log.rows.len() >= MAX_ROWS {
        return;
    }
    let (loop_ms, loop_runs) = scene_loop.as_deref().map_or((-1.0, -1.0), |t| {
        (t.scene_loop_ms, t.scene_schedule_run_count as f32)
    });
    let (mw, rw, wait, gpu) = breakdown.as_deref().map_or((-1.0, -1.0, -1.0, -1.0), |b| {
        (
            b.main_world_schedules_ms,
            b.render_world_cpu_ms,
            b.swapchain_present_wait_ms.unwrap_or(-1.0),
            b.gpu_frame_time_ms.unwrap_or(-1.0),
        )
    });
    let p = player
        .single()
        .map(|t| t.translation())
        .unwrap_or(Vec3::splat(f32::NAN));
    let frame = log.frame as f32;
    log.rows.push([
        frame,
        time.delta_secs() * 1000.0,
        us[0],
        n[0],
        us[1],
        n[1],
        us[2],
        n[2],
        us[3],
        n[3],
        us[4],
        n[4],
        loop_ms,
        loop_runs,
        mw,
        rw,
        wait,
        gpu,
        p.x,
        p.y,
        p.z,
    ]);
}

/// Temporary measurement probe: start | stop | fetch | status.
#[derive(clap::Parser, ConsoleCommand)]
#[command(name = "/cpu_probe")]
struct CpuProbeCommand {
    action: String,
}

fn cpu_probe_command(mut input: ConsoleCommand<CpuProbeCommand>, mut log: ResMut<FrameLog>) {
    let Some(Ok(command)) = input.take() else {
        return;
    };
    match command.action.as_str() {
        "start" => {
            log.rows.clear();
            log.recording = true;
            input.reply_ok("recording");
        }
        "stop" => {
            log.recording = false;
            input.reply_ok(format!("stopped {}", log.rows.len()));
        }
        "status" => input.reply_ok(format!(
            "{{\"recording\":{},\"rows\":{}}}",
            log.recording,
            log.rows.len()
        )),
        "fetch" => {
            let mut s = String::with_capacity(log.rows.len() * 120 + 400);
            s.push_str("{\"cols\":[");
            for (i, c) in COLS.iter().enumerate() {
                if i > 0 {
                    s.push(',');
                }
                s.push('"');
                s.push_str(c);
                s.push('"');
            }
            s.push_str("],\"rows\":[");
            for (r, row) in log.rows.iter().enumerate() {
                if r > 0 {
                    s.push(',');
                }
                s.push('[');
                for (i, v) in row.iter().enumerate() {
                    if i > 0 {
                        s.push(',');
                    }
                    if v.is_finite() {
                        s.push_str(&format!("{}", (v * 1000.0).round() / 1000.0));
                    } else {
                        s.push_str("null");
                    }
                }
                s.push(']');
            }
            s.push_str("]}");
            input.reply_ok(s);
        }
        other => input.reply_failed(format!("unknown action {other}: start|stop|fetch|status")),
    }
}

/// `"cpu":{"apply_us":..,"apply_n":..,...},` so far this frame, for the movement trace's frame
/// line (written before `Last`, where `record_frame` takes and resets the counters).
pub fn peek_json() -> String {
    let mut s = String::from("\"cpu\":{");
    for (i, name) in NAMES.iter().enumerate() {
        let ns = NANOS[i].load(Ordering::Relaxed);
        let n = COUNT[i].load(Ordering::Relaxed);
        if i > 0 {
            s.push(',');
        }
        s.push_str(&format!(
            "\"{name}_us\":{:.1},\"{name}_n\":{n}",
            ns as f64 / 1000.0
        ));
    }
    s.push_str("},");
    s
}
