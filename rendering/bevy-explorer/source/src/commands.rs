use bevy::{
    diagnostic::{Diagnostic, DiagnosticsStore, FrameTimeDiagnosticsPlugin},
    prelude::*,
};
use bevy_console::ConsoleCommand;
use common::{
    gpu_time::GPU_FRAME_TIME,
    memory::{GpuMemoryBudget, MemoryStatus},
    rpc::{RpcCall, RpcCallEvent, RpcResultSender},
    structs::{
        AppConfig, ParcelGrassSetting, PreviewMode, PrimaryUser, SceneLoadDistance, SessionGraphics,
    },
};
use console::PendingConsoleResponses;
use scene_runner::{
    initialize_scene::{parcels_in_range, ScenePointers},
    OutOfWorld,
};

/// set location
#[derive(clap::Parser, ConsoleCommand)]
#[command(name = "/teleport")]
pub struct ChangeLocationCommand {
    #[arg(allow_hyphen_values(true))]
    x: i32,
    #[arg(allow_hyphen_values(true))]
    y: i32,
    /// realm the parcel is in (a world name or realm url): change realm, then land on the parcel
    realm: Option<String>,
}

pub fn change_location(
    mut commands: Commands,
    mut input: ConsoleCommand<ChangeLocationCommand>,
    mut player: Query<(Entity, &mut Transform), With<PrimaryUser>>,
    mut pending: ResMut<PendingConsoleResponses>,
) {
    if let Some(Ok(command)) = input.take() {
        if let Some(realm) = command.realm {
            let (response, rx) = RpcResultSender::channel();
            commands.send_event(RpcCallEvent::engine(RpcCall::TeleportPlayer {
                to: Some(IVec2::new(command.x, command.y)),
                realm: Some(realm.clone()),
                response,
            }));
            let location = (command.x, command.y);
            pending.push_receiver(
                rx,
                move |result| result.map(|()| format!("new location: {location:?} in {realm}")),
                input.take_responder(),
            );
            return;
        }
        if let Ok((ent, mut transform)) = player.single_mut() {
            transform.translation.x = command.x as f32 * 16.0 + 8.0;
            transform.translation.z = -command.y as f32 * 16.0 - 8.0;
            if let Ok(mut commands) = commands.get_entity(ent) {
                commands.try_insert(OutOfWorld);
            }
            input.reply_ok(format!("new location: {:?}", (command.x, command.y)));
            return;
        }

        input.reply_failed("failed to set location");
    }
}

/// set scene load distance (defaults to 75.0m) and additional unload distance (defaults to 25.0m)
#[derive(clap::Parser, ConsoleCommand)]
#[command(name = "/scene_distance")]
pub struct SceneDistanceCommand {
    distance: Option<f32>,
    unload: Option<f32>,
}

pub fn scene_distance(
    mut input: ConsoleCommand<SceneDistanceCommand>,
    mut scene_load_distance: ResMut<SceneLoadDistance>,
) {
    if let Some(Ok(command)) = input.take() {
        let distance = command.distance.unwrap_or(75.0);
        scene_load_distance.load = distance;
        if let Some(unload) = command.unload {
            scene_load_distance.unload = unload;
        }
        input.reply_ok(format!(
            "set scene load distance to +{distance} -{}",
            scene_load_distance.load + scene_load_distance.unload
        ));
    }
}

/// Locks the preview mode to the current parcel
#[derive(clap::Parser, ConsoleCommand)]
#[command(name = "/lock_preview")]
pub struct LockPreviewCommand;

pub fn lock_preview(
    mut input: ConsoleCommand<LockPreviewCommand>,
    mut preview_mode: ResMut<PreviewMode>,
    focus: Single<&GlobalTransform, With<PrimaryUser>>,
    pointers: Res<ScenePointers>,
) {
    if let Some(Ok(_command)) = input.take() {
        let Some((parcel, _)) = parcels_in_range(&focus, 0.0, pointers.min(), pointers.max()).pop()
        else {
            unreachable!("Player should never be in a invalid parcel.");
        };
        let Some(_current_scene) = pointers.get(parcel) else {
            input.reply_failed(format!("failed to locked preview to parcel {}", parcel));
            return;
        };
        preview_mode.preview_parcel = Some(parcel);

        input.reply_ok(format!("locked preview to parcel {}", parcel));
    }
}

/// Unlocks the preview mode to the current parcel
#[derive(clap::Parser, ConsoleCommand)]
#[command(name = "/unlock_preview")]
pub struct UnlockPreviewCommand;

pub fn unlock_preview(
    mut input: ConsoleCommand<UnlockPreviewCommand>,
    mut preview_mode: ResMut<PreviewMode>,
) {
    if let Some(Ok(_command)) = input.take() {
        let parcel = preview_mode.preview_parcel.take();

        if let Some(parcel) = parcel {
            input.reply_ok(format!("unlocked preview to parcel {}", parcel));
        } else {
            input.reply("Preview was not locked to a parcel.");
        }
    }
}

/// set thread count
#[derive(clap::Parser, ConsoleCommand)]
#[command(name = "/scene_threads")]
pub struct SceneThreadsCommand {
    threads: Option<usize>,
}

pub fn scene_threads(
    mut input: ConsoleCommand<SceneThreadsCommand>,
    mut config: ResMut<AppConfig>,
) {
    if let Some(Ok(command)) = input.take() {
        let threads = command.threads.unwrap_or(4);
        config.scene_threads = threads;
        input.reply_ok("scene simultaneous thread count set to {threads}");
    }
}

/// Report the live engine readings and the effective session frame-rate target as JSON.
#[derive(clap::Parser, ConsoleCommand)]
#[command(name = "/performance_state")]
pub struct PerformanceStateCommand;

pub fn performance_state(
    mut input: ConsoleCommand<PerformanceStateCommand>,
    window: Res<analytics::frame::LastFrameWindow>,
    diagnostics: Res<DiagnosticsStore>,
    time: Res<Time<Real>>,
    session: Res<SessionGraphics>,
    config: Res<AppConfig>,
    memory: Res<MemoryStatus>,
) {
    if let Some(Ok(_)) = input.take() {
        let fps = diagnostics
            .get(&FrameTimeDiagnosticsPlugin::FPS)
            .and_then(Diagnostic::smoothed)
            .or_else(|| {
                let seconds = time.delta_secs_f64();
                (seconds > 0.0).then(|| seconds.recip())
            });
        let gpu_ms = diagnostics
            .get(&GPU_FRAME_TIME)
            .and_then(Diagnostic::smoothed);
        input.reply_ok(
            serde_json::json!({
                "version": crate::version(),
                "frameMs": (time.delta_secs_f64() * 10000.0).round() / 10.0,
                "minFps": window.min_fps, "maxFps": window.max_fps, "hiccups": window.hiccups,
                "fps": fps.map(f64::round),
                "targetFps": session.fps_target(&config),
                "overrideFps": session.fps_target,
                "gpuFrameMs": gpu_ms.map(|ms| (ms * 10.0).round() / 10.0),
                "engineMemoryMiB": memory.reading().map(|reading| reading.used_bytes >> 20),
            })
            .to_string(),
        );
    }
}

/// Inspect the engine memory budget or set its session limit in MiB (zero restores platform capacity).
#[derive(clap::Parser, ConsoleCommand)]
#[command(name = "/debug_memory")]
pub struct MemoryCommand {
    limit_mib: Option<u32>,
}

pub fn debug_memory(
    mut cmd: ConsoleCommand<MemoryCommand>,
    mut memory: ResMut<MemoryStatus>,
    gpu: Option<Res<GpuMemoryBudget>>,
) {
    let Some(Ok(args)) = cmd.take() else { return };
    if let Some(limit) = args.limit_mib {
        memory.limit_mib = std::num::NonZeroU64::new(limit.into());
        cmd.reply_ok("Updated session memory limit");
        return;
    }
    let reading = memory.reading();
    let pools = match gpu.as_deref() {
        Some(GpuMemoryBudget::Available(pools)) => Some(pools.iter().map(|p| serde_json::json!({
            "pool": format!("{:?}", p.kind), "usedMiB": p.used_bytes >> 20, "budgetMiB": p.budget_bytes.get() >> 20
        })).collect::<Vec<_>>()),
        _ => None,
    };
    cmd.reply_ok(serde_json::json!({
        "used": reading.map(|r| r.used_bytes >> 20), "capacity": reading.map(|r| r.capacity_mib),
        "cap": reading.map(|r| r.cap_mib), "limit": memory.limit_mib.map_or(0, |n| n.get()),
        "pressure": reading.map(|r| format!("{:?}", r.pressure())), "gpu": pools,
        "thresholds": reading.map(|r| { let t = r.thresholds(); serde_json::json!({
            "abundance": t.abundance_mib.round(), "warning": t.warning_mib.round(), "full": t.full_mib.round()
        }) })
    }).to_string());
}

/// set fps
#[derive(clap::Parser, ConsoleCommand)]
#[command(name = "/fps")]
pub struct FpsCommand {
    fps: usize,
}

pub fn set_fps(mut input: ConsoleCommand<FpsCommand>, mut config: ResMut<AppConfig>) {
    if let Some(Ok(command)) = input.take() {
        let fps = command.fps;
        config.graphics.fps_target = fps;
        input.reply_ok("target frame rate set to {fps}");
    }
}

/// set graphics for this session only, without saving them: `fps <n>`, `grass <off|low|mid|high>`,
/// `render_scale <0.25-1>` (below the player setting's 50% floor, see
/// `SessionGraphics::MIN_RENDER_SCALE`); without a value, that one goes back to the player's setting
#[derive(clap::Parser, ConsoleCommand)]
#[command(name = "/session_graphics")]
pub struct SessionGraphicsCommand {
    setting: String,
    value: Option<String>,
}

pub fn session_graphics(
    mut input: ConsoleCommand<SessionGraphicsCommand>,
    mut session: ResMut<SessionGraphics>,
) {
    let Some(Ok(command)) = input.take() else {
        return;
    };
    let value = command.value.as_deref();
    let valid = match command.setting.as_str() {
        "fps" => match value.map(str::parse::<usize>) {
            None => {
                session.fps_target = None;
                true
            }
            Some(Ok(fps)) if fps > 0 => {
                session.fps_target = Some(fps);
                true
            }
            Some(_) => false,
        },
        "grass" => match value.map(parse_grass) {
            None => {
                session.parcel_grass = None;
                true
            }
            Some(Some(grass)) => {
                session.parcel_grass = Some(grass);
                true
            }
            Some(None) => false,
        },
        #[cfg(not(target_arch = "wasm32"))]
        "render_scale" => {
            input.reply_failed("render_scale only applies to the web canvas");
            return;
        }
        #[cfg(target_arch = "wasm32")]
        "render_scale" => match value.map(str::parse::<f32>) {
            None => {
                session.render_scale = None;
                true
            }
            Some(Ok(scale)) if (SessionGraphics::MIN_RENDER_SCALE..=1.0).contains(&scale) => {
                session.render_scale = Some(scale);
                true
            }
            Some(_) => false,
        },
        other => {
            input.reply_failed(format!(
                "unknown setting {other}: use fps, grass or render_scale"
            ));
            return;
        }
    };
    let setting = &command.setting;
    match value {
        Some(value) if !valid => input.reply_failed(format!("invalid {setting} value {value}")),
        Some(value) => input.reply_ok(format!("{setting} set to {value} for this session")),
        None => input.reply_ok(format!("{setting} back to the player's setting")),
    }
}

fn parse_grass(value: &str) -> Option<ParcelGrassSetting> {
    match value {
        "off" => Some(ParcelGrassSetting::Off),
        "low" => Some(ParcelGrassSetting::Low),
        "mid" => Some(ParcelGrassSetting::Mid),
        "high" => Some(ParcelGrassSetting::High),
        _ => None,
    }
}
