//! Headless authoritative-server scene runner.
//!
//! Runs Decentraland SDK7 scenes with no window / render / GPU — the render-free
//! plugin set proven by `scene_runner`'s integration tests (`TestPlugins` +
//! `init_test_app`), plus the realm/preview loading and comms the real explorer
//! uses. `--server-mode` makes `isServer()` return true to scene JS, so a scene's
//! authoritative-server branch runs. Intended to replace hammurabi-headless.

use std::{str::FromStr, sync::OnceLock, time::Duration};
use webgpu_build::headless::{scene_stats_sample, EmittedSceneStats};

use bevy::tasks::IoTaskPool;
use bevy::{app::ScheduleRunnerPlugin, log::LogPlugin, prelude::*};
use clap::Parser;
use common::{
    structs::{AppConfig, AppError, IVec2Arg, PreviewMode, SceneLoadDistance},
    util::{TaskCompat, TaskExt},
};
use comms::{
    livekit::{
        room::{ConnectFailed, Connecting, Disconnected, LivekitRoom, ServerRoomTerminal},
        LivekitTransport,
    },
    AdapterManager, ServerSceneRooms,
};
use dcl_deno_ipc::init_runtime;
use dcl_types::SceneLogMessage;
use ipfs::{map_realm_name, IpfsAssetServer};
use scene_runner::{
    initialize_scene::{PortableScenes, PortableSource, SceneHash, SceneLoading},
    renderer_context::RendererSceneContext,
};
use system_api_types::launch_options::{help_heading::HEADLESS, LaunchOptions};
use wallet::{
    delegation::{StorageDelegation, StorageDelegations},
    sign_request, Wallet,
};

#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;
static SESSION_LOG: OnceLock<String> = OnceLock::new();

// The shared launch params (realm, position, preview, base domain, pulse server, content
// server, and whatever gets added later — accepted here automatically) plus the server-role
// flags. The rendering clients' options (`ClientOptions`) are not flags here at all.
#[derive(clap::Parser)]
#[command(
    name = "headless",
    about = "Headless scene runner: an authoritative scene server, or a render-free test client",
    mut_arg("realm", |a| a.help("Realm to boot into; absent = http://localhost:8000")),
    mut_arg("position", |a| {
        a.visible_alias("location")
            .help("Parcel to load as `x,y`; absent = 0,0")
    }),
)]
struct Args {
    #[command(flatten)]
    launch: LaunchOptions,

    /// Serve the scene: `isServer()` is true to scene code and realm-wide comms are never
    /// joined. Standalone (without --orchestrated) is a local-dev flow and requires --preview
    #[arg(long, help_heading = HEADLESS)]
    server_mode: bool,

    /// Multi-scene worker driven over stdin by an orchestrator; implies --server-mode
    #[arg(long, help_heading = HEADLESS)]
    orchestrated: bool,

    /// Exit cleanly after this many seconds
    #[arg(long, value_name = "secs", help_heading = HEADLESS)]
    timeout: Option<f32>,

    /// Scene javascript threads; absent = 16 orchestrated, 4 otherwise
    #[arg(
        long,
        value_name = "n",
        value_parser = clap::builder::RangedU64ValueParser::<usize>::new().range(1..), help_heading = HEADLESS
    )]
    scene_threads: Option<usize>,

    /// Scene tick rate
    #[arg(
        long,
        value_name = "hz",
        default_value_t = 30,
        value_parser = clap::value_parser!(u32).range(1..), help_heading = HEADLESS
    )]
    tick_hz: u32,

    /// base64 world-storage delegation (hammurabi envelope), or the PROCESS_STORAGE_DELEGATION
    /// env var; single-scene runs only — orchestrated scenes receive theirs via the control
    /// channel
    #[arg(long, value_name = "base64", help_heading = HEADLESS)]
    storage_delegation: Option<String>,

    /// Deterministic guest wallet (test harness): the address is derivable offline from the seed
    #[arg(long, value_name = "seed", help_heading = HEADLESS)]
    wallet_seed: Option<u64>,

    /// Pulse realm to announce verbatim; `--server-mode` only, and optional there: absent, the
    /// engine derives the same key from a locally hosted scene's entity id once it loads, this
    /// just announces it sooner. Orchestrated servers host several realms at once and take one
    /// per scene on `add-scene` instead
    #[arg(long, value_name = "key", help_heading = HEADLESS)]
    pulse_realm: Option<String>,

    // the loopback console server: a client run only (see `parse_args`)
    #[command(flatten)]
    automation: webgpu_build::automation::AutomationOptions,

    // resolved after parsing: realm defaults to a local preview server, position to 0,0
    #[arg(skip)]
    realm: String,
    #[arg(skip)]
    location: IVec2,
}

/// The launcher's "permanently unavailable here" status (EX_CONFIG). A refused argument is not
/// worth retrying, and callers fall back to another server implementation on it — the npm
/// launcher already answers its own argument checks with this
/// (deploy/headless/launcher/bin/cli.js) and forwards whatever the engine returns.
const EXIT_UNAVAILABLE: i32 = 78;

fn usage_error(message: impl std::fmt::Display) -> ! {
    eprintln!("{message}");
    std::process::exit(EXIT_UNAVAILABLE);
}

fn parse_args() -> Args {
    let mut args = match Args::try_parse() {
        Ok(args) => args,
        Err(e) => {
            let _ = e.print();
            std::process::exit(if e.use_stderr() { EXIT_UNAVAILABLE } else { 0 });
        }
    };
    // latch first: everything below that composes a backend host reads it
    if let Err(e) = webgpu_build::launch::latch(&args.launch) {
        usage_error(e);
    }
    args.realm = args
        .launch
        .realm
        .clone()
        .unwrap_or_else(|| "http://localhost:8000".to_owned());
    args.location = match &args.launch.position {
        Some(position) => {
            IVec2Arg::from_str(position)
                .unwrap_or_else(|e| usage_error(format!("--location {position}: {e}")))
                .0
        }
        None => IVec2::ZERO,
    };
    // orchestrated mode is always a server
    args.server_mode |= args.orchestrated;
    // standalone server mode is a local-dev flow (single scene on the shared context,
    // self-minted scene room); production servers are always orchestrated
    if args.server_mode && !args.orchestrated && !args.launch.preview {
        usage_error(
            "--server-mode without --orchestrated is a local-dev mode and requires --preview",
        );
    }
    // latch the process-global server-role flags before the app is built
    if args.server_mode {
        common::structs::set_server_mode();
    }
    if args.orchestrated {
        common::structs::set_multi_tenant();
    }
    // mirror hammurabi's worker env contract (PROCESS_STORAGE_DELEGATION)
    if args.storage_delegation.is_none() {
        args.storage_delegation = std::env::var("PROCESS_STORAGE_DELEGATION").ok();
    }
    if args.pulse_realm.is_some() && args.orchestrated {
        usage_error(
            "--pulse-realm is a --server-mode flag; orchestrated scenes carry their own realm",
        );
    }
    // a server serves scenes and takes no commands from outside; the environment variables are
    // ignored there, so one set for a client run elsewhere does not stop it
    if args.server_mode && args.automation != Default::default() {
        usage_error(
            "--automation-token and --automation-port are for a client run: --server-mode takes no console server",
        );
    }
    args
}

/// harness support: a seeded wallet gives a deterministic address, so livekit tokens
/// (identity == address) can be minted before the process starts
fn finalize_guest_wallet(wallet: &mut Wallet, seed: Option<u64>) {
    match seed {
        Some(seed) => {
            let mut bytes = [0u8; 32];
            bytes[..8].copy_from_slice(&seed.to_le_bytes());
            wallet.finalize_as_guest_with_seed(bytes);
        }
        None => wallet.finalize_as_guest(),
    }
}

// ---------------- orchestrator control protocol ----------------
// stdin: one JSON command per line. stdout: control events as single lines with a
// reserved `@bevy-ctl ` prefix; scene logs as `@scene-log ` lines keyed by scene hash.
// The parent (multiplayer-server) never shares its authoritative key: adapters and
// delegations are minted parent-side and passed per scene, mirroring hammurabi workers.

#[derive(serde::Deserialize, Debug)]
#[serde(tag = "type", rename_all = "kebab-case")]
enum ControlCommand {
    /// Pin a scene and (optionally) join its pre-minted room. Re-sent for a running scene
    /// with a fresh adapter, it re-dials the scene's room in place without reloading the scene:
    /// the answer to `scene-room-disconnected`.
    /// `urn` must be a full entity urn incl. baseUrl: urn:decentraland:entity:<hash>?=&baseUrl=<content>/contents/
    AddScene {
        #[serde(rename = "sceneId")]
        scene_id: String,
        urn: String,
        /// pre-minted comms adapter (livekit:wss://host?access_token=T). If absent in
        /// --preview mode, the engine mints one from the local gatekeeper (smoke tests only).
        adapter: Option<String>,
        /// base64 world-storage delegation minted by the orchestrator for this scene
        #[serde(rename = "storageDelegation")]
        storage_delegation: Option<String>,
        /// The realm this scene belongs to — the same name a client reads from that realm's
        /// `about`. Like `adapter`, it states how the scene is being served, which only the
        /// orchestrator knows. Pulse partitions peer visibility by realm and every world numbers
        /// its parcels from 0,0, so this is what tells two cohosted worlds' `0,0` apart — and
        /// getting it wrong is silent, not loud, which is why it is not optional.
        realm: String,
    },
    RemoveScene {
        #[serde(rename = "sceneId")]
        scene_id: String,
    },
    Status,
    /// Storage-delegation renewal (mirrors hammurabi's storage-delegation:response IPC):
    /// the engine emits `storage-delegation-request` when a delegation nears expiry and
    /// the parent replies with a fresh one for that scene.
    #[serde(rename = "storage-delegation-response")]
    StorageDelegationResponse {
        #[serde(rename = "sceneId")]
        scene_id: Option<String>,
        delegation: Option<String>,
    },
}

#[derive(Resource)]
struct ControlChannel(std::sync::Mutex<std::sync::mpsc::Receiver<ControlCommand>>);

/// What the orchestrator asked for: the scenes to host and each one's room credential.
/// `drain_control_commands` only writes this; `reconcile_scene_rooms` brings the world to
/// match it, one action per scene per frame, so commands may arrive in any order and batch.
#[derive(Resource, Default)]
struct OrchestratedScenes {
    wanted: std::collections::HashMap<String, WantedScene>,
    /// removed scenes whose entity or room is still being torn down: a wanted entry for one
    /// of these waits, so an `add-scene` after a `remove-scene` restarts the scene however
    /// close together the two land
    removed: std::collections::HashSet<String>,
}

struct WantedScene {
    urn: String,
    credential: RoomCredential,
}

enum RoomCredential {
    /// a livekit adapter: dialled when the scene has no room, re-dialled on the scene's room
    /// when it differs from the one that room was dialled with
    Adapter(String),
    /// refused or kicked: never dialled again, a new adapter replaces it
    Spent,
    /// preview only: minted from the local preview gatekeeper (smoke tests)
    Unminted,
    Minting(bevy::tasks::Task<Result<String, anyhow::Error>>),
}

fn ctl_emit(event: &serde_json::Value) {
    println!("@bevy-ctl {event}");
}

fn spawn_stdin_reader() -> std::sync::mpsc::Receiver<ControlCommand> {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        use std::io::BufRead;
        let stdin = std::io::stdin();
        for line in stdin.lock().lines() {
            let Ok(line) = line else { break };
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            match serde_json::from_str::<ControlCommand>(line) {
                Ok(cmd) => {
                    if tx.send(cmd).is_err() {
                        break;
                    }
                }
                Err(e) => ctl_emit(&serde_json::json!({
                    "type": "error", "error": format!("bad command: {e}")
                })),
            }
        }
        // stdin closed: parent died or finished — exit like hammurabi's disconnect handler
        ctl_emit(&serde_json::json!({"type": "stdin-closed"}));
        std::process::exit(0);
    });
    rx
}

fn main() -> AppExit {
    let session_time: chrono::DateTime<chrono::Utc> = chrono::DateTime::from_timestamp_millis(
        web_time::SystemTime::now()
            .duration_since(web_time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64,
    )
    .unwrap();
    let dirs = platform::project_directories().unwrap();
    let log_dir = dirs.data_local_dir();
    std::fs::create_dir_all(log_dir).unwrap();
    let session_log = log_dir.join(format!(
        "headless-{}.log",
        session_time.format("%Y%m%d-%H%M%S")
    ));
    SESSION_LOG
        .set(session_log.to_string_lossy().into_owned())
        .unwrap();

    // args first, so --help and a bad flag answer without spawning the scene runtime
    let args = parse_args();
    TIMEOUT.set(args.timeout).ok();

    // v8 runtime must init on the main thread before the App is built (matches the tests).
    // Headless is always a server: a lost JS sidecar must restart the whole engine (the
    // desktop client and the scene_runner tests leave this false).
    dcl_deno_ipc::EXIT_ON_SIDECAR_LOSS.store(true, std::sync::atomic::Ordering::SeqCst);
    init_runtime().unwrap();

    println!(
        "[headless] realm={} location={} preview={} server_mode={} tick_hz={}",
        args.realm, args.location, args.launch.preview, args.server_mode, args.tick_hz
    );

    let config = webgpu_build::headless::config(
        &args.realm,
        args.location,
        args.tick_hz,
        args.scene_threads
            .unwrap_or(if args.orchestrated { 16 } else { 4 }),
        // orchestrated: scene console output is creator-only, served via @scene-log frames
        !args.orchestrated,
    );

    let mut app = App::new();

    app.add_plugins(LogPlugin {
        filter: "wgpu=error,naga=error,bevy_animation=error,matrix=error,symphonia=warn"
            .to_string(),
        ..default()
    });

    app.add_plugins(TaskPoolPlugin::default())
        .add_plugins(ScheduleRunnerPlugin::run_loop(Duration::ZERO));

    // world-storage delegations (per scene; CLI/env value is the single-scene fallback)
    let mut delegations = StorageDelegations::default();
    if args.orchestrated {
        // KEY-3 style structural gate: a process-level credential in multi-scene mode
        // would be a wildcard — delegations must arrive per scene over the control channel
        if args.storage_delegation.is_some() {
            println!("[headless] refusing --storage-delegation/PROCESS_STORAGE_DELEGATION in orchestrated mode; pass storageDelegation per add-scene");
        }
    } else if let Some(encoded) = &args.storage_delegation {
        match StorageDelegation::parse(encoded, &map_realm_name(&args.realm)) {
            Ok(delegation) => {
                println!(
                    "[headless] storage delegation loaded: world={} scene={} parcel={}",
                    delegation.world, delegation.scene_id, delegation.parcel
                );
                delegations.fallback = Some(delegation);
            }
            Err(e) => println!("[headless] ignoring storage delegation: {e}"),
        }
    }

    // ---- the render-free app (src/headless/app.rs, shared with the web entry) ----
    let mut wallet = Wallet::default();
    finalize_guest_wallet(&mut wallet, args.wallet_seed);
    // KEY-3: the orchestrated engine must only ever hold a throwaway identity — the
    // authoritative key lives in the orchestrator and never reaches this process.
    assert!(
        wallet.is_guest(),
        "orchestrated engine must run with a guest wallet"
    );

    webgpu_build::headless::assemble(
        &mut app,
        config,
        &args.launch,
        &args.realm,
        args.server_mode,
        wallet,
    );
    app.insert_resource(delegations);
    if !args.server_mode {
        // a test client answers `/world_state` (and, from the comms plugin, `/chat_history` and
        // `/voice_state`) without a window; with a token, the loopback server runs them
        app.add_plugins(webgpu_build::world_state::WorldStatePlugin);
        if let Some(server) = args.automation.server() {
            app.add_plugins(server);
        }
    }
    if args.orchestrated {
        // never load by player position — scenes come ONLY from the control channel
        // (portables are exempt from distance loading).
        app.insert_resource(SceneLoadDistance {
            load: -1.0,
            unload: 0.0,
            load_imposter: 0.0,
        });
    }

    // the orchestrator minted the realm key for this preview, so we announce it rather than
    // waiting for a scene to load and deriving the same string from its entity id
    if let Some(realm) = args.pulse_realm.clone() {
        app.insert_resource(comms::pulse::plugin::PulseRealmOverride(realm));
    }

    app.add_systems(PreUpdate, supervisor);
    #[cfg(unix)]
    {
        shutdown_signal::install();
        app.add_systems(Update, exit_on_shutdown_signal);
    }

    if args.orchestrated {
        app.insert_resource(ControlChannel(std::sync::Mutex::new(spawn_stdin_reader())))
            .init_resource::<OrchestratedScenes>()
            // structural gate: the engine must never sign gatekeeper handshakes in
            // orchestrated mode — adapters are always minted by the trusted parent
            .insert_resource(comms::DisableSceneRoomGatekeeper(true))
            .add_systems(
                Update,
                (
                    (drain_control_commands, reconcile_scene_rooms).chain(),
                    reap_scene_contexts,
                    demux_scene_logs,
                    request_delegation_renewals,
                    reap_terminal_scene_rooms,
                    report_disconnected_scene_rooms,
                    project_server_scene_rooms,
                ),
            )
            .add_systems(
                PostUpdate,
                (
                    emit_scene_status,
                    emit_scene_stats,
                    emit_failed_scene_status,
                ),
            );
        ctl_emit(&serde_json::json!({"type": "starting", "realm": args.realm}));
    } else if args.server_mode {
        // before the scene loads: the lifecycle spawns the SceneHash, loading starts next frame
        app.add_systems(
            PostUpdate,
            claim_scene_locks.after(common::sets::RealmLifecycle),
        );
    }

    log_panics::init();

    // Warn-and-continue on fallible-system / command / observer errors instead of the
    // Bevy default (panic). A raw panic in a scene-data system still aborts the shared
    // engine and takes every co-tenant down, so this is a safety net, not a full fix —
    // see #1063 for the scene-reachable panic sites this does not cover.
    bevy::ecs::error::GLOBAL_ERROR_HANDLER
        .set(bevy::ecs::error::warn)
        .ok();

    // returned, not dropped: the supervisor's AppExit is the process status the launcher
    // forwards (deploy/headless/launcher/bin/cli.js)
    app.run()
}

/// SIGINT/SIGTERM/SIGHUP wind the engine down through `AppExit` instead of the default
/// immediate termination, so the `World` drops and the comms drivers get to say goodbye
/// (the Pulse driver sends its ENet DISCONNECT on drop; without it the server only notices
/// us via its peer timeout). A repeated signal bails out hard.
#[cfg(unix)]
fn exit_on_shutdown_signal(mut sent: Local<bool>, mut exit: EventWriter<AppExit>) {
    if !*sent && shutdown_signal::received() {
        *sent = true;
        info!("[headless] shutdown signal received, exiting");
        exit.write_default();
    }
}

#[cfg(unix)]
mod shutdown_signal {
    use std::sync::atomic::{AtomicBool, Ordering};

    static RECEIVED: AtomicBool = AtomicBool::new(false);

    pub fn received() -> bool {
        RECEIVED.load(Ordering::Relaxed)
    }

    extern "C" fn flag(signal: libc::c_int) {
        if RECEIVED.swap(true, Ordering::Relaxed) {
            // a repeated signal means the graceful exit isn't getting there; bail out hard
            unsafe { libc::_exit(128 + signal) };
        }
    }

    pub fn install() {
        unsafe {
            for sig in [libc::SIGINT, libc::SIGTERM, libc::SIGHUP] {
                libc::signal(sig, flag as *const () as libc::sighandler_t);
            }
        }
    }
}

/// Liveness supervisor. Logs when the scene first ticks; exits non-zero on a
/// broken scene or the optional wall-clock timeout, zero on graceful conditions.
#[allow(clippy::too_many_arguments)]
fn supervisor(
    time: Res<Time>,
    scenes: Query<&RendererSceneContext>,
    mut errors: EventReader<AppError>,
    mut exit: EventWriter<AppExit>,
    mut announced: Local<bool>,
    mut last_report: Local<f64>,
    orchestrated: Option<Res<OrchestratedScenes>>,
    updates: Res<scene_runner::SceneUpdates>,
) {
    let elapsed = time.elapsed_secs_f64();

    for e in errors.read() {
        error!("[headless] scene error: {e:?}");
    }

    let mut any_broken = false;
    let mut max_tick = 0u32;
    let mut count = 0usize;
    for ctx in scenes.iter() {
        count += 1;
        max_tick = max_tick.max(ctx.tick_number);
        if ctx.broken() {
            any_broken = true;
            error!("[headless] scene {} is broken", ctx.hash);
        }
    }

    if count > 0 && max_tick >= 1 && !*announced {
        *announced = true;
        println!("[headless] {count} scene(s) live, first tick reached");
    }

    if *announced && elapsed - *last_report > 5.0 {
        *last_report = elapsed;
        println!(
            "[headless] alive: {count} scene(s), max_tick={max_tick}, t={elapsed:.0}s, \
             frames={}, skip_inflight={}, skip_sent={}",
            updates.frames, updates.skipped_in_flight, updates.skipped_already_sent
        );
    }

    // Orchestrated: a broken scene is reported (scene-broken event) and removed by the
    // orchestrator — it must NOT take the other scenes down with an engine exit.
    if any_broken && orchestrated.is_none() {
        exit.write(AppExit::from_code(1));
    }

    // wall-clock timeout: graceful success exit for smoke tests
    // (checked in main via arg-injected resource below)
    if let Some(limit) = TIMEOUT.get().copied().flatten() {
        if elapsed > limit as f64 {
            println!("[headless] timeout {limit}s reached, exiting");
            exit.write_default();
        }
    }
}

// timeout is read in the supervisor; stored globally to avoid threading it as a resource
static TIMEOUT: OnceLock<Option<f32>> = OnceLock::new();

/// Standalone servers hold an OS file lock per scene hash (the `b64-` project path in
/// preview) for the life of the process, so a second local server for the same scene
/// declines to run instead of fighting the first over the scene room. A crashed holder's
/// lock is released by the OS. A preview reload respawns the scene entity under the same
/// hash; the lock this process already holds covers it, so it is not claimed again (a
/// second handle on the same file would conflict with our own lock). A lock whose scene
/// is gone (removed, or never loaded) is released.
fn claim_scene_locks(
    added: Query<&SceneHash, Added<SceneHash>>,
    scenes: Query<&SceneHash>,
    mut held: Local<std::collections::HashMap<String, std::fs::File>>,
    mut exit: EventWriter<AppExit>,
) {
    held.retain(|hash, _| scenes.iter().any(|SceneHash(h)| h == hash));

    for SceneHash(hash) in added.iter() {
        if held.contains_key(hash) {
            continue;
        }
        let dir = platform::project_directories()
            .unwrap()
            .data_local_dir()
            .join("scene-locks");
        // base64 may contain '/'; '_' is outside its alphabet so names stay distinct.
        // long hashes keep their tail to fit windows MAX_PATH under the data dir
        let name = hash.replace('/', "_");
        let name = &name[name.char_indices().rev().nth(127).map_or(0, |(i, _)| i)..];
        let path = dir.join(format!("{name}.lock"));
        let file = std::fs::create_dir_all(&dir).and_then(|_| {
            std::fs::OpenOptions::new()
                .create(true)
                .truncate(false)
                .write(true)
                .open(&path)
        });
        let file = match file {
            Ok(file) => file,
            Err(e) => {
                warn!("[headless] can't open scene lock {}: {e}", path.display());
                continue;
            }
        };
        match file.try_lock() {
            Ok(()) => {
                held.insert(hash.clone(), file);
            }
            Err(std::fs::TryLockError::WouldBlock) => {
                println!(
                    "[headless] scene {} is already served by another local server (lock {}); exiting",
                    hash,
                    path.display()
                );
                exit.write(AppExit::from_code(1));
            }
            Err(std::fs::TryLockError::Error(e)) => {
                warn!("[headless] can't lock {}: {e}", path.display());
            }
        }
    }
}

/// A room's crdt context must outlive its scene: scenes resolve their context strictly
/// on a multi-tenant server (a miss panics), and a removed scene tears down over several
/// frames during which it still resolves. Reap a context only once nothing references
/// its hash — not queued (portables), not loading, not live.
fn reap_scene_contexts(
    mut commands: Commands,
    mut crdt_contexts: ResMut<comms::global_crdt::CrdtContexts>,
    portables: Res<PortableScenes>,
    orch: Res<OrchestratedScenes>,
    live: Query<&RendererSceneContext>,
    loading: Query<&SceneHash, With<SceneLoading>>,
) {
    crdt_contexts.0.retain(|hash, context| {
        if hash.is_empty() {
            // the shared context
            return true;
        }
        // a wanted scene keeps its context even while unqueued (a mint in flight, a room still
        // being re-dialled): its room points at it and the scene reloads into it
        if portables.contains_key(hash)
            || orch.wanted.contains_key(hash)
            || live.iter().any(|ctx| *ctx.hash == **hash)
            || loading.iter().any(|loading_hash| &loading_hash.0 == hash)
        {
            return true;
        }
        if let Ok(mut c) = commands.get_entity(*context) {
            c.despawn();
        }
        false
    });
}

/// The adapter a scene room was dialled with.
#[derive(Component)]
struct SceneRoomAdapter(String);

/// A disconnected room the orchestrator has been told about.
#[derive(Component)]
struct DisconnectReported;

/// ONLY livekit adapters: any other protocol (signed-login, ws-room, fixed-adapter
/// recursion) could make the engine sign a remote-chosen payload with its identity. Applies
/// to orchestrator input AND gatekeeper mint responses alike.
fn livekit_address(adapter: &str) -> Result<&str, String> {
    adapter.strip_prefix("livekit:").ok_or_else(|| {
        format!(
            "refusing non-livekit adapter: {}",
            adapter.chars().take(24).collect::<String>()
        )
    })
}

/// Dial a scene room.
fn dial_scene_room(
    scene_id: &str,
    adapter: &str,
    context: Entity,
    manager: &mut AdapterManager,
    commands: &mut Commands,
) -> Result<Entity, String> {
    livekit_address(adapter)?;
    // a scene room, never the realm island: it must not bring up Pulse
    let entity = manager
        .connect_scene(adapter, context)
        .ok_or_else(|| "adapter connect failed".to_owned())?;
    commands.entity(entity).try_insert((
        comms::SceneRoom(scene_id.to_owned()),
        SceneRoomAdapter(adapter.to_owned()),
    ));
    Ok(entity)
}

/// Tear down scene rooms whose LiveKit room hit a terminal disconnect (DuplicateIdentity /
/// kicked) or whose connect was refused: the token is spent, the parent re-adds the scene
/// with a freshly minted adapter.
#[allow(clippy::type_complexity)]
fn reap_terminal_scene_rooms(
    mut commands: Commands,
    mut orch: ResMut<OrchestratedScenes>,
    terminal: Query<
        (
            Entity,
            &comms::SceneRoom,
            &SceneRoomAdapter,
            Has<DisconnectReported>,
        ),
        Or<(With<ServerRoomTerminal>, With<ConnectFailed>)>,
    >,
) {
    for (entity, room, adapter, reported) in terminal.iter() {
        info!(
            "[headless] scene room for {} terminally disconnected (duplicate identity / kicked / connect refused); tearing down",
            room.0
        );
        if let Ok(mut c) = commands.get_entity(entity) {
            c.despawn();
        }
        // unless a newer adapter is already wanted
        if let Some(wanted) = orch.wanted.get_mut(&room.0) {
            if matches!(&wanted.credential, RoomCredential::Adapter(current) if *current == adapter.0)
            {
                wanted.credential = RoomCredential::Spent;
            }
        }
        // a kick lands as two SDK events; if the first already got the room reported, this is
        // the same disconnect
        if !reported {
            ctl_emit(&serde_json::json!({
                "type": "scene-room-disconnected", "scene": room.0
            }));
        }
    }
}

/// Report a scene room LiveKit gave up on: the SDK's own reconnects are exhausted and the
/// adapter's token has long expired, so the engine cannot re-dial it. The room is kept
/// until the orchestrator either removes the scene or re-sends `add-scene` with a fresh
/// adapter.
#[allow(clippy::type_complexity)]
fn report_disconnected_scene_rooms(
    mut commands: Commands,
    rooms: Query<
        (Entity, &comms::SceneRoom),
        (
            With<Disconnected>,
            Without<ServerRoomTerminal>,
            Without<DisconnectReported>,
        ),
    >,
) {
    for (entity, room) in rooms.iter() {
        info!("[headless] scene room for {} disconnected", room.0);
        commands.entity(entity).try_insert(DisconnectReported);
        ctl_emit(&serde_json::json!({
            "type": "scene-room-disconnected", "scene": room.0
        }));
    }
}

/// `ServerSceneRooms` is read in other crates (realm info, signed fetch): a projection of
/// the scene rooms, rewritten when they change.
#[allow(clippy::type_complexity)]
fn project_server_scene_rooms(
    rooms: Query<(Entity, &comms::SceneRoom, &SceneRoomAdapter)>,
    changed: Query<(), Or<(Changed<comms::SceneRoom>, Changed<SceneRoomAdapter>)>>,
    mut removed: RemovedComponents<comms::SceneRoom>,
    mut server_rooms: ResMut<ServerSceneRooms>,
) {
    let any_removed = !removed.is_empty();
    removed.clear();
    if changed.is_empty() && !any_removed {
        return;
    }
    let current: bevy::platform::collections::HashMap<_, _> = rooms
        .iter()
        .map(|(entity, room, adapter)| (room.0.clone(), (adapter.0.clone(), entity)))
        .collect();
    if server_rooms.0 != current {
        server_rooms.0 = current;
    }
}

/// Ask the orchestrator for fresh delegations nearing expiry (hammurabi semantics:
/// 5-minute refresh buffer, at most one request per scene per 30 s).
fn request_delegation_renewals(
    delegations: Res<StorageDelegations>,
    time: Res<Time>,
    mut last_request: Local<std::collections::HashMap<String, f64>>,
) {
    const REFRESH_BUFFER_MS: i64 = 5 * 60 * 1000;
    const REQUEST_THROTTLE_SECS: f64 = 30.0;

    let now_ms = web_time::SystemTime::now()
        .duration_since(web_time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64;
    let elapsed = time.elapsed_secs_f64();

    // drop throttle state for scenes no longer holding a delegation (removed scenes),
    // so this map doesn't grow unbounded over the engine's lifetime
    last_request.retain(|scene, _| delegations.by_scene.contains_key(scene));

    for (scene, delegation) in &delegations.by_scene {
        if now_ms < delegation.expiration - REFRESH_BUFFER_MS {
            continue;
        }
        let due = last_request
            .get(scene)
            .is_none_or(|last| elapsed - last > REQUEST_THROTTLE_SECS);
        if due {
            last_request.insert(scene.clone(), elapsed);
            ctl_emit(&serde_json::json!({
                "type": "storage-delegation-request", "scene": scene
            }));
        }
    }
}

// ---------------- orchestrated-mode systems ----------------

/// Gather: apply the orchestrator's commands to `OrchestratedScenes` and the per-scene
/// resources. Rooms are not touched here; `reconcile_scene_rooms` does that.
#[allow(clippy::too_many_arguments)]
fn drain_control_commands(
    control: Res<ControlChannel>,
    mut orch: ResMut<OrchestratedScenes>,
    mut portables: ResMut<PortableScenes>,
    mut scene_realms: ResMut<comms::global_crdt::SceneRealms>,
    preview: Res<PreviewMode>,
    scenes: Query<&RendererSceneContext>,
    mut delegations: ResMut<StorageDelegations>,
    config: Res<AppConfig>,
) {
    let store_delegation = |scene_id: &str, encoded: &str, delegations: &mut StorageDelegations| {
        match StorageDelegation::parse(encoded, &map_realm_name(&config.home_realm())) {
            Ok(delegation) => {
                // reject a renewal that rebinds to another scene's credential
                // (hammurabi's same-scene guard)
                let same_scene = delegations.by_scene.get(scene_id).is_none_or(|current| {
                    current.world == delegation.world
                        && current.scene_id == delegation.scene_id
                        && current.parcel == delegation.parcel
                });
                if !same_scene {
                    ctl_emit(&serde_json::json!({
                        "type": "error", "scene": scene_id,
                        "error": "storage delegation is bound to a different scene; ignored"
                    }));
                    return;
                }
                delegations.by_scene.insert(scene_id.to_owned(), delegation);
                ctl_emit(&serde_json::json!({
                    "type": "storage-delegation-set", "scene": scene_id
                }));
            }
            Err(e) => ctl_emit(&serde_json::json!({
                "type": "error", "scene": scene_id,
                "error": format!("bad storage delegation: {e}")
            })),
        }
    };

    let Ok(rx) = control.0.lock() else { return };
    while let Ok(cmd) = rx.try_recv() {
        match cmd {
            ControlCommand::AddScene {
                scene_id,
                urn,
                adapter,
                storage_delegation,
                realm,
            } => {
                if let Some(encoded) = &storage_delegation {
                    store_delegation(&scene_id, encoded, &mut delegations);
                }
                scene_realms.0.insert(scene_id.clone(), realm);
                ctl_emit(&serde_json::json!({"type": "scene-added", "scene": scene_id}));
                let credential = match adapter {
                    // pre-minted by the trusted orchestrator: never sign anything ourselves
                    Some(adapter) => {
                        if let Err(error) = livekit_address(&adapter) {
                            // a scene already running keeps its room; a new one never starts
                            let event = if orch.wanted.contains_key(&scene_id) {
                                "error"
                            } else {
                                "scene-failed"
                            };
                            ctl_emit(
                                &serde_json::json!({"type": event, "scene": scene_id, "error": error}),
                            );
                            continue;
                        }
                        RoomCredential::Adapter(adapter)
                    }
                    // production: the adapter MUST be minted by the trusted orchestrator.
                    // The engine never signs gatekeeper handshakes outside local preview.
                    None if !preview.is_preview => {
                        ctl_emit(&serde_json::json!({
                            "type": "scene-failed", "scene": scene_id,
                            "error": "adapter required outside preview mode"
                        }));
                        continue;
                    }
                    // smoke-test fallback: mint from the local preview gatekeeper with the
                    // guest identity (mirrors hammurabi's LocalPreview flow)
                    None => RoomCredential::Unminted,
                };
                orch.wanted
                    .insert(scene_id, WantedScene { urn, credential });
            }
            ControlCommand::RemoveScene { scene_id } => {
                portables.remove(&scene_id);
                // takes a mint in flight with it; the room goes in reconcile_scene_rooms
                orch.wanted.remove(&scene_id);
                orch.removed.insert(scene_id.clone());
                delegations.by_scene.remove(&scene_id);
                scene_realms.0.remove(&scene_id);
                // the crdt context is NOT despawned here: the scene tears down over the
                // following frames and must still resolve it; reap_scene_contexts
                // collects it once nothing references the hash
                ctl_emit(&serde_json::json!({"type": "scene-removed", "scene": scene_id}));
            }
            ControlCommand::Status => {
                for ctx in scenes.iter() {
                    ctl_emit(&serde_json::json!({
                        "type": "scene-status", "scene": ctx.hash,
                        "tick": ctx.tick_number, "broken": ctx.broken(),
                    }));
                }
            }
            ControlCommand::StorageDelegationResponse {
                scene_id,
                delegation,
            } => {
                if let (Some(scene_id), Some(encoded)) = (&scene_id, &delegation) {
                    store_delegation(scene_id, encoded, &mut delegations);
                }
            }
        }
    }
}

/// Diff: bring the scene rooms in line with `OrchestratedScenes`, one action per scene per
/// frame. A wanted scene with no room is dialled and only then queued: a scene must never
/// load without its room context (scenes resolve their context strictly on a multi-tenant
/// server; a fallback to the shared context would see cross-room presence). A room whose
/// scene is no longer wanted is despawned. A room dialled with another adapter than the
/// wanted one is re-dialled on the same transport, so the messages it holds carry over
/// and its players rejoin through it. A removed scene is torn down even if it is wanted
/// again: its entry waits until the old room and scene entity are gone, then dials afresh.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn reconcile_scene_rooms(
    mut commands: Commands,
    mut orch: ResMut<OrchestratedScenes>,
    mut manager: AdapterManager,
    mut crdt_contexts: ResMut<comms::global_crdt::CrdtContexts>,
    mut portables: ResMut<PortableScenes>,
    mut rooms: Query<(
        Entity,
        &comms::SceneRoom,
        &SceneRoomAdapter,
        Option<&mut LivekitTransport>,
        Has<Disconnected>,
        Has<Connecting>,
    )>,
    wallet: Res<Wallet>,
    ipfs: IpfsAssetServer,
    live: Query<&RendererSceneContext>,
    loading: Query<&SceneHash, With<SceneLoading>>,
) {
    for (entity, room, ..) in rooms.iter() {
        if !orch.wanted.contains_key(&room.0) || orch.removed.contains(&room.0) {
            if let Ok(mut c) = commands.get_entity(entity) {
                c.despawn();
            }
        }
    }
    orch.removed.retain(|scene_id| {
        rooms.iter().any(|(_, room, ..)| room.0 == *scene_id)
            || live.iter().any(|ctx| &*ctx.hash == scene_id.as_str())
            || loading.iter().any(|SceneHash(hash)| hash == scene_id)
    });
    let OrchestratedScenes { wanted, removed } = &mut *orch;
    for (scene_id, wanted) in wanted.iter_mut() {
        if removed.contains(scene_id) {
            continue;
        }
        if let RoomCredential::Minting(task) = &mut wanted.credential {
            let Some(result) = task.complete() else {
                continue;
            };
            wanted.credential = match result {
                Ok(adapter) => RoomCredential::Adapter(adapter),
                Err(e) => {
                    ctl_emit(&serde_json::json!({
                        "type": "error", "scene": scene_id, "error": format!("gatekeeper mint failed: {e}")
                    }));
                    RoomCredential::Spent
                }
            };
        }
        // a wanted scene with a room runs, whatever its credential is doing: a `remove-scene`
        // whose room is still around when the `add-scene` lands must not leave it unqueued
        if rooms.iter().any(|(_, room, ..)| room.0 == *scene_id)
            && !portables.contains_key(scene_id)
        {
            portables.insert(
                scene_id.clone(),
                PortableSource {
                    pid: wanted.urn.clone(),
                    parent_scene: None,
                    ens: None,
                    super_user: false,
                },
            );
        }
        if matches!(wanted.credential, RoomCredential::Unminted) {
            wanted.credential =
                RoomCredential::Minting(start_preview_mint(scene_id, &wallet, &ipfs));
            continue;
        }
        let RoomCredential::Adapter(adapter) = &wanted.credential else {
            continue;
        };
        let adapter = adapter.clone();
        let room = rooms.iter_mut().find(|(_, room, ..)| room.0 == *scene_id);
        let result = match room {
            None => Some({
                // each scene room gets its own crdt context (player-presence view), reused
                // across reconnects so scene subscriptions and player state survive re-mints
                let context = crdt_contexts.0.get(scene_id).copied().unwrap_or_else(|| {
                    let context = commands
                        .spawn(comms::global_crdt::GlobalCrdtState::new(Some(
                            scene_id.to_owned(),
                        )))
                        .id();
                    crdt_contexts.0.insert(scene_id.to_owned(), context);
                    context
                });
                dial_scene_room(scene_id, &adapter, context, &mut manager, &mut commands)
                    .map(|_| ())
            }),
            Some((_, _, current, ..)) if current.0 == adapter => None,
            // dialled this frame: its transport arrives next frame
            Some((_, _, _, None, ..)) => None,
            // a dial is in flight: the new adapter waits for it to settle
            Some((_, _, _, Some(_), _, true)) => None,
            // a live room asked to re-dial goes down first and is re-dialled next frame: its
            // participants leave through the Disconnected hook, and dropping LivekitRoom drops
            // the old room's event stream, so the new connection evicting it (DuplicateIdentity,
            // which would read as a kick and reap the entity) is never seen. The orchestrator
            // asked, so no report.
            Some((entity, _, _, Some(_), false, false)) => {
                commands
                    .entity(entity)
                    .try_insert((Disconnected, DisconnectReported))
                    .remove::<LivekitRoom>();
                None
            }
            Some((entity, _, _, Some(mut transport), true, false)) => {
                Some(livekit_address(&adapter).map(|address| {
                    transport.address = address.to_owned();
                    commands
                        .entity(entity)
                        .try_insert((SceneRoomAdapter(adapter.clone()), Connecting))
                        .remove::<DisconnectReported>();
                }))
            }
        };
        match result {
            Some(Ok(())) => {
                ctl_emit(&serde_json::json!({"type": "scene-room-connected", "scene": scene_id}));
            }
            Some(Err(error)) => {
                // a refused dial for a running scene is a dead room; for a scene that never
                // started it is a dead scene
                let event = if portables.contains_key(scene_id) {
                    "scene-room-disconnected"
                } else {
                    "scene-failed"
                };
                ctl_emit(&serde_json::json!({"type": event, "scene": scene_id, "error": error}));
                wanted.credential = RoomCredential::Spent;
                continue;
            }
            None => {}
        }
        if !portables.contains_key(scene_id) {
            portables.insert(
                scene_id.clone(),
                PortableSource {
                    pid: wanted.urn.clone(),
                    parent_scene: None,
                    ens: None,
                    super_user: false,
                },
            );
        }
    }
}

/// Smoke tests only: mint a scene room adapter from the local preview gatekeeper with the
/// guest identity (mirrors hammurabi's LocalPreview flow).
fn start_preview_mint(
    scene_id: &str,
    wallet: &Wallet,
    ipfs: &IpfsAssetServer,
) -> bevy::tasks::Task<Result<String, anyhow::Error>> {
    let wallet = wallet.clone();
    let client = ipfs.ipfs().client();
    let sid = scene_id.to_owned();
    IoTaskPool::get().spawn_compat(async move {
        let url = common::base_domain::url(
            common::base_domain::Service::PreviewGatekeeper,
            "/get-server-scene-adapter",
        );
        let uri = http::Uri::try_from(url.as_str())?;
        let meta = serde_json::json!({
            "intent": "dcl:explorer:comms-handshake",
            "signer": "dcl:explorer",
            "isGuest": true,
            "realm": {"serverName": "LocalPreview"},
            "realmName": "LocalPreview",
            "sceneId": sid,
        })
        .to_string();
        let headers = sign_request("POST", &uri, &wallet, meta).await?;
        let mut request = client
            .post(url)
            .timeout(std::time::Duration::from_secs(10))
            .header("Content-Type", "application/json");
        for (k, v) in headers {
            request = request.header(k, v);
        }
        let response = request.send().await?;
        if !response.status().is_success() {
            anyhow::bail!("gatekeeper status {}", response.status());
        }
        let body: serde_json::Value = response.json().await?;
        body["adapter"]
            .as_str()
            .map(ToOwned::to_owned)
            .ok_or_else(|| anyhow::anyhow!("no adapter in response"))
    })
}

/// Demux per-scene logs onto stdout as machine-parsable lines keyed by scene hash,
/// so the orchestrator can route them to its per-scene SSE log buffers.
fn demux_scene_logs(
    scenes: Query<(Entity, &RendererSceneContext)>,
    mut receivers: Local<webgpu_build::headless::SceneLogReceivers>,
) {
    webgpu_build::headless::drain_scene_logs(&scenes, &mut receivers, emit_scene_log);
}

fn emit_scene_log(hash: &str, log: &SceneLogMessage) {
    println!(
        "@scene-log {}",
        serde_json::json!({"scene": hash, "level": webgpu_build::headless::scene_log_level(&log.level), "ts": log.timestamp, "msg": log.message})
    );
}

/// Periodic per-scene status + first-tick / broken events for the orchestrator.
fn emit_scene_status(
    time: Res<Time>,
    scenes: Query<&RendererSceneContext>,
    mut last: Local<f64>,
    mut live: Local<std::collections::HashSet<String>>,
    mut broken: Local<std::collections::HashSet<String>>,
) {
    // Prune dedup entries for scenes that no longer exist, so a scene that is removed
    // and later re-added (same hash, fresh entity) re-emits scene-live / scene-broken.
    // Without this the orchestrator's remove+re-add recovery works only once per scene
    // per engine lifetime — a re-broken scene would silently stay broken while the
    // orchestrator still believes it is live.
    let current: std::collections::HashSet<String> =
        scenes.iter().map(|ctx| ctx.hash.to_string()).collect();
    live.retain(|hash| current.contains(hash));
    broken.retain(|hash| current.contains(hash));

    for ctx in scenes.iter() {
        if ctx.tick_number >= 1 && !live.contains(&*ctx.hash) {
            live.insert(ctx.hash.to_string());
            ctl_emit(&serde_json::json!({"type": "scene-live", "scene": ctx.hash}));
        }
        if ctx.broken() && !broken.contains(&*ctx.hash) {
            broken.insert(ctx.hash.to_string());
            ctl_emit(&serde_json::json!({"type": "scene-broken", "scene": ctx.hash}));
        }
    }

    let elapsed = time.elapsed_secs_f64();
    if elapsed - *last > 5.0 {
        *last = elapsed;
        for ctx in scenes.iter() {
            ctl_emit(&serde_json::json!({
                "type": "scene-status", "scene": ctx.hash,
                "tick": ctx.tick_number, "broken": ctx.broken(),
            }));
        }
    }
}

/// Periodic per-scene resource stats for the orchestrator. Every field except the
/// instantaneous gauges is a per-window delta against the previous emission's cumulative
/// snapshot, so the orchestrator can rate scenes without tracking totals itself.
fn emit_scene_stats(
    time: Res<Time<Real>>,
    mut commands: Commands,
    mut scenes: Query<(
        Entity,
        &RendererSceneContext,
        Option<&mut EmittedSceneStats>,
    )>,
    mut last: Local<f64>,
) {
    let elapsed = time.elapsed_secs_f64();
    if elapsed - *last <= 10.0 {
        return;
    }
    *last = elapsed;

    for (entity, ctx, previous) in scenes.iter_mut() {
        // Native scene threads publish counters after completed updates.
        let Some(cur) = &ctx.resource_counters else {
            continue;
        };
        let base = previous
            .as_ref()
            .map(|previous| previous.0.clone())
            .unwrap_or_default();
        let Some(sample) = scene_stats_sample(&ctx.hash, entity, cur, &base, cfg!(unix)) else {
            continue;
        };
        println!("@scene-stats {sample}");
        if let Some(mut previous) = previous {
            previous.0 = cur.clone();
        } else {
            commands
                .entity(entity)
                .try_insert(EmittedSceneStats(cur.clone()));
        }
    }
}

/// Report scenes that FAILED to load (bad/missing entity definition, JS init error)
/// to the orchestrator. A failed scene never gains a RendererSceneContext, so neither
/// emit_scene_status nor the supervisor observes it — without this it sits invisibly
/// wedged in live_scenes, and the orchestrator keeps it "active" so join events never
/// respawn it. Emit scene-broken (deduped per live entity) so the orchestrator runs
/// its bounded remove + re-add recovery, which re-fetches the entity and clears a
/// transient content-server flake. Keyed by Entity, so a re-added scene (fresh entity,
/// same hash) reports again and the per-scene restart budget applies.
fn emit_failed_scene_status(
    loading: Query<(Entity, &SceneHash, &SceneLoading)>,
    mut reported: Local<std::collections::HashSet<Entity>>,
    orchestrated: Option<Res<OrchestratedScenes>>,
) {
    if orchestrated.is_none() {
        return;
    }
    let failed: Vec<(Entity, &str)> = loading
        .iter()
        .filter(|(_, _, state)| matches!(state, SceneLoading::Failed))
        .map(|(entity, hash, _)| (entity, hash.0.as_str()))
        .collect();
    let current: std::collections::HashSet<Entity> = failed.iter().map(|(e, _)| *e).collect();
    reported.retain(|e| current.contains(e));
    for (entity, hash) in failed {
        if reported.insert(entity) {
            error!("[headless] scene {hash} failed to load");
            ctl_emit(&serde_json::json!({"type": "scene-broken", "scene": hash}));
        }
    }
}
