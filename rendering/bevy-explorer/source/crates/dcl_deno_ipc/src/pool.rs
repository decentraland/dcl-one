use super::{
    ipc_socket_name, read_frame, run_host_connection, ConnectionEnd, HostScenes, NewSceneCommand,
    Rejected, SandboxMode, SandboxStatus, SceneToEngine, EXIT_ON_SIDECAR_LOSS, HOST_EXIT_SANDBOX,
    KILL_GRACE_PERIOD,
};
use anyhow::{anyhow, Context};
use bevy::{
    ecs::entity::Entity,
    log::{error, info, warn},
};
use dcl_types::{SceneId, SceneResponse};
use futures_util::future::BoxFuture;
use interprocess::local_socket::{
    traits::tokio::{Listener, Stream as _},
    GenericFilePath, ListenerOptions, ToFsName,
};
use once_cell::sync::Lazy;
use std::{
    collections::HashMap,
    ffi::OsString,
    path::PathBuf,
    pin::Pin,
    process::Stdio,
    sync::{
        atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering},
        Arc, Mutex, RwLock, Weak,
    },
    time::{Duration, Instant},
};
use tokio::{
    io::{AsyncBufReadExt, AsyncRead, AsyncWrite},
    sync::{
        mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender},
        Notify,
    },
};

/// Host spawns that fail in a row, with no host running, before the pool counts as unavailable.
const SPAWN_FAILURES_UNAVAILABLE: u32 = 3;
// a host that neither connects nor exits by then has failed to start
const HOST_CONNECT_TIMEOUT: Duration = Duration::from_secs(30);
// a host's output line is cut to this many bytes in the engine's log
const HOST_LOG_LINE_BYTES: usize = 4096;
// lines over this many per second of one host stream are dropped and counted
const HOST_LOG_LINES_PER_SECOND: u32 = 200;
// a connected host that has not reported ready by then (V8 is starting) has failed to start
const HOST_READY_TIMEOUT: Duration = Duration::from_secs(30);
// how long a host that closed its connection before reporting gets to exit with its own status
const HOST_EXIT_WAIT: Duration = Duration::from_secs(2);
// The headless launcher's "permanently unavailable here" status (EX_CONFIG), for hosts that keep
// failing the required sandbox.
const EXIT_SANDBOX_UNAVAILABLE: i32 = 78;
// how often a host whose scenes were all removed checks whether its grace period is over
const RETIRE_POLL: Duration = Duration::from_millis(250);
// how often the resident memory of per-scene hosts is sampled
const MEMORY_SAMPLE_PERIOD: Duration = Duration::from_secs(2);
// V8's worker pool in a per-scene host, which runs one isolate
const PER_SCENE_V8_THREADS: u32 = 2;
/// The environment a per-scene host is started with: what the scene host and the libraries it
/// links read (logging, the scene log budget, temporary files, TLS roots, HTTP proxies, the time
/// zone and locale).
pub(crate) const PER_SCENE_HOST_ENV: &[&str] = &[
    "RUST_LOG",
    "RUST_BACKTRACE",
    "RUST_LIB_BACKTRACE",
    "NO_COLOR",
    "DCL_SCENE_LOG_MAX_LINES",
    "DCL_SCENE_LOG_MAX_LINE_BYTES",
    "TMPDIR",
    "TEMP",
    "TMP",
    "SSL_CERT_FILE",
    "SSL_CERT_DIR",
    "HTTP_PROXY",
    "HTTPS_PROXY",
    "ALL_PROXY",
    "NO_PROXY",
    "http_proxy",
    "https_proxy",
    "all_proxy",
    "no_proxy",
    "TZ",
    "LANG",
    "LC_ALL",
    "SystemRoot",
];

/// Which scene host a new scene runs in.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Placement {
    /// One scene host for every scene.
    #[default]
    Shared,
    /// A scene host for each scene, never given another scene.
    PerScene,
}

#[derive(Clone, Debug, Default)]
pub struct HostOptions {
    pub placement: Placement,
    /// `PerScene` only: hosts kept started with no scene, so a new scene need not wait for one.
    pub spares: usize,
    /// `PerScene` only: a host whose resident memory goes over this many bytes is ended.
    pub memory_cap: Option<u64>,
    /// What each host applies to itself before it takes a scene. A host whose report does not
    /// meet it is ended without a scene.
    pub sandbox: SandboxMode,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HostState {
    Starting,
    Ready,
    Lost,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HostExitReason {
    /// The process exited or its connection ended.
    Exit,
    /// The engine ended it over a frame it sent.
    Protocol,
    /// The engine ended it for going over its memory cap.
    Memory,
    /// It could not be started.
    Spawn,
    /// It did not apply the sandbox the engine requires.
    Sandbox,
}

impl HostExitReason {
    pub fn as_str(self) -> &'static str {
        match self {
            HostExitReason::Exit => "exit",
            HostExitReason::Protocol => "protocol",
            HostExitReason::Memory => "memory",
            HostExitReason::Spawn => "spawn",
            HostExitReason::Sandbox => "sandbox",
        }
    }
}

/// A scene host lost while it still ran scenes. Each of them has been sent a
/// `SceneResponse::Error`. Only [`Placement::PerScene`] reports these.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HostExit {
    pub scenes: Vec<SceneId>,
    pub code: Option<i32>,
    pub signal: Option<i32>,
    pub reason: HostExitReason,
}

/// The pool as a tester sees it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PoolInfo {
    pub placement: Placement,
    pub memory_cap: Option<u64>,
    pub sandbox: SandboxMode,
    pub hosts: Vec<SceneHostInfo>,
    /// Hosts that ended while the engine still served them, since the pool started.
    pub lost_hosts: u64,
}

/// A scene host as the pool sees it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SceneHostInfo {
    pub pid: Option<u32>,
    pub state: HostState,
    /// The scenes it runs, not counting removed ones.
    pub scenes: Vec<SceneId>,
    /// Resident memory at the last sample; sampled only with [`Placement::PerScene`].
    pub rss_bytes: Option<u64>,
    /// The sandbox it reported, once it has.
    pub sandbox: Option<SandboxStatus>,
}

/// The scene host could not apply the sandbox the engine requires.
#[derive(Debug)]
pub struct SandboxRefused(pub String);

impl std::fmt::Display for SandboxRefused {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for SandboxRefused {}

/// How a scene host process ended.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct HostStatus {
    pub code: Option<i32>,
    pub signal: Option<i32>,
}

impl std::fmt::Display for HostStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match (self.code, self.signal) {
            (Some(code), _) => write!(f, "exit code {code}"),
            (None, Some(signal)) => write!(f, "signal {signal}"),
            (None, None) => f.write_str("unknown status"),
        }
    }
}

/// What the engine knows about one scene host. Its IPC thread owns the process.
pub(crate) struct Host {
    pub(crate) scenes: Arc<Mutex<HostScenes>>,
    state: Mutex<HostState>,
    pid: Mutex<Option<u32>>,
    // 0 until sampled
    rss_bytes: AtomicU64,
    sandbox: Mutex<Option<SandboxStatus>>,
    // set, then notified, to make the IPC thread end the host
    end_reason: Mutex<Option<HostExitReason>>,
    end: Notify,
}

impl Host {
    fn new() -> Self {
        Self {
            scenes: Default::default(),
            state: Mutex::new(HostState::Starting),
            pid: Default::default(),
            rss_bytes: Default::default(),
            sandbox: Default::default(),
            end_reason: Default::default(),
            end: Notify::new(),
        }
    }

    fn end(&self, reason: HostExitReason) {
        self.end_reason.lock().unwrap().get_or_insert(reason);
        self.stop_taking_scenes();
        self.end.notify_one();
    }

    // An ending host is given no new scene; the ones it has are reported when it is gone.
    fn stop_taking_scenes(&self) {
        self.scenes.lock().unwrap().closed = true;
    }

    fn state(&self) -> HostState {
        *self.state.lock().unwrap()
    }

    fn set_state(&self, state: HostState) {
        *self.state.lock().unwrap() = state;
    }

    fn info(&self) -> SceneHostInfo {
        let scenes = self.scenes.lock().unwrap();
        SceneHostInfo {
            pid: *self.pid.lock().unwrap(),
            state: self.state(),
            scenes: scenes
                .live()
                .map(|(id, _)| SceneId(Entity::from_bits(id)))
                .collect(),
            rss_bytes: Some(self.rss_bytes.load(Ordering::Relaxed)).filter(|rss| *rss > 0),
            sandbox: *self.sandbox.lock().unwrap(),
        }
    }
}

/// How to start the scene host binary.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct HostCommand {
    pub program: PathBuf,
    pub args: Vec<OsString>,
    /// The whole environment; `None` inherits the engine's.
    pub env: Option<Vec<(OsString, OsString)>>,
}

impl HostCommand {
    fn scene_host(
        endpoint: &str,
        placement: Placement,
        sandbox: SandboxMode,
    ) -> anyhow::Result<Self> {
        let mut program = std::env::current_exe().context("locate the scene host binary")?;
        program.pop();

        // pop an extra folder when running tests
        if program.file_name().and_then(|s| s.to_str()) == Some("deps") {
            program.pop();
        }

        let mut program = program.join("dcl_deno_ipc");
        if cfg!(windows) {
            program.set_extension("exe");
        }
        let mut args = vec![endpoint.into()];
        if sandbox != SandboxMode::Off {
            args.extend(["--sandbox".into(), sandbox.as_str().into()]);
        }
        let mut env = None;
        if placement == Placement::PerScene {
            args.extend([
                "--v8-threads".into(),
                PER_SCENE_V8_THREADS.to_string().into(),
            ]);
            let inherited = std::env::vars_os().filter(|(name, _)| {
                PER_SCENE_HOST_ENV
                    .iter()
                    .any(|allowed| name.as_os_str() == *allowed)
            });
            env = Some(inherited.collect());
        }
        Ok(Self { program, args, env })
    }
}

/// A running scene host and the engine's end of its connection.
pub(crate) struct HostLink {
    pub inbound: Pin<Box<dyn AsyncRead + Send>>,
    pub outbound: Pin<Box<dyn AsyncWrite + Send>>,
    pub process: Box<dyn HostProcess>,
}

pub(crate) trait HostProcess: Send {
    fn id(&self) -> Option<u32>;
    fn start_kill(&mut self);
    fn wait(&mut self) -> BoxFuture<'_, std::io::Result<HostStatus>>;
}

/// Starts a scene host and connects to it. Called on the host's IPC thread, inside its runtime.
pub(crate) trait Launch: Send + Sync {
    fn launch(
        &self,
        endpoint: String,
        command: HostCommand,
    ) -> BoxFuture<'_, anyhow::Result<HostLink>>;
}

/// Starts the `dcl_deno_ipc` binary and accepts its connection on a local socket.
pub(crate) struct ProcessLauncher;

impl Launch for ProcessLauncher {
    fn launch(
        &self,
        endpoint: String,
        command: HostCommand,
    ) -> BoxFuture<'_, anyhow::Result<HostLink>> {
        Box::pin(async move {
            let name = endpoint.to_fs_name::<GenericFilePath>()?;
            let listener = ListenerOptions::new().name(name).create_tokio()?;

            let mut process = tokio::process::Command::new(&command.program);
            // the headless server's stdin and stdout are its control channel: a host reads
            // neither, and its output goes to the engine's log
            process
                .args(&command.args)
                .stdin(Stdio::null())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .kill_on_drop(true);
            if let Some(env) = &command.env {
                process.env_clear().envs(env.iter().cloned());
            }
            // the IPC thread spawns the host and lives as long as it does, so the signal fires
            // when the engine is gone
            #[cfg(target_os = "linux")]
            {
                let engine = std::process::id();
                // SAFETY: only async-signal-safe calls between fork and exec
                unsafe {
                    process.pre_exec(move || {
                        if libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGKILL) != 0 {
                            return Err(std::io::Error::last_os_error());
                        }
                        if libc::getppid() as u32 != engine {
                            libc::_exit(1);
                        }
                        Ok(())
                    });
                }
            }
            // A console-subsystem child spawned by the GUI-subsystem app has no console to
            // inherit, so windows pops a visible one; suppress it.
            #[cfg(windows)]
            {
                const CREATE_NO_WINDOW: u32 = 0x0800_0000;
                process.creation_flags(CREATE_NO_WINDOW);
            }
            let mut child = process
                .spawn()
                .with_context(|| format!("failed to spawn deno binary at {:?}", command.program))?;
            let pid = child.id().unwrap_or_default();
            if let Some(stdout) = child.stdout.take() {
                tokio::spawn(log_host_output(stdout, pid));
            }
            if let Some(stderr) = child.stderr.take() {
                tokio::spawn(log_host_output(stderr, pid));
            }

            let stream = tokio::select! {
                stream = listener.accept() => stream?,
                status = child.wait() => {
                    return Err(anyhow!("scene host exited before connecting: {}", status?));
                }
                () = tokio::time::sleep(HOST_CONNECT_TIMEOUT) => {
                    return Err(anyhow!("scene host did not connect within {HOST_CONNECT_TIMEOUT:?}"));
                }
            };
            let (inbound, outbound) = stream.split();
            Ok(HostLink {
                inbound: Box::pin(inbound),
                outbound: Box::pin(outbound),
                process: Box::new(child),
            })
        })
    }
}

/// Forwards a host's output to the engine's log, a line per event, with control characters
/// replaced: never to the engine's stdout. Lines are cut at [`HOST_LOG_LINE_BYTES`], and lines over
/// [`HOST_LOG_LINES_PER_SECOND`] are dropped and counted.
pub(crate) async fn log_host_output(output: impl AsyncRead + Unpin, pid: u32) {
    let mut output = tokio::io::BufReader::new(output);
    let mut line = Vec::with_capacity(256);
    let mut window = (tokio::time::Instant::now(), 0u32, 0u64);
    loop {
        let (consumed, complete) = match output.fill_buf().await {
            Ok([]) | Err(_) => (0, false),
            Ok(buffer) => {
                let end = buffer.iter().position(|b| *b == b'\n');
                let chunk = &buffer[..end.unwrap_or(buffer.len())];
                let room = HOST_LOG_LINE_BYTES.saturating_sub(line.len());
                line.extend_from_slice(&chunk[..chunk.len().min(room)]);
                (end.map_or(buffer.len(), |end| end + 1), end.is_some())
            }
        };
        if consumed == 0 {
            if !line.is_empty() {
                emit_host_line(pid, &line, &mut window);
            }
            if window.2 > 0 {
                warn!(target: "scene_host", "[scene host {pid}] {} output lines dropped", window.2);
            }
            return;
        }
        output.consume(consumed);
        if complete {
            emit_host_line(pid, &line, &mut window);
            line.clear();
        }
    }
}

// `window`: when the current second started, the lines logged in it, the lines dropped in it
fn emit_host_line(pid: u32, line: &[u8], window: &mut (tokio::time::Instant, u32, u64)) {
    let now = tokio::time::Instant::now();
    if now.duration_since(window.0) >= Duration::from_secs(1) {
        if window.2 > 0 {
            warn!(target: "scene_host", "[scene host {pid}] {} output lines dropped", window.2);
        }
        *window = (now, 0, 0);
    }
    if window.1 >= HOST_LOG_LINES_PER_SECOND {
        window.2 += 1;
        return;
    }
    window.1 += 1;
    let text: String = String::from_utf8_lossy(line)
        .chars()
        .map(|c| {
            if c.is_control() && c != '\t' {
                '\u{fffd}'
            } else {
                c
            }
        })
        .collect();
    info!(target: "scene_host", "[scene host {pid}] {text}");
}

impl HostProcess for tokio::process::Child {
    fn id(&self) -> Option<u32> {
        tokio::process::Child::id(self)
    }

    fn start_kill(&mut self) {
        let _ = tokio::process::Child::start_kill(self);
    }

    fn wait(&mut self) -> BoxFuture<'_, std::io::Result<HostStatus>> {
        Box::pin(async move {
            let status = tokio::process::Child::wait(self).await?;
            #[cfg(unix)]
            let signal = std::os::unix::process::ExitStatusExt::signal(&status);
            #[cfg(not(unix))]
            let signal = None;
            Ok(HostStatus {
                code: status.code(),
                signal,
            })
        })
    }
}

struct HostEntry {
    host: Arc<Host>,
    // dropping it ends the host: its connection closes once no scene can be queued on it
    new_scene: UnboundedSender<NewSceneCommand>,
    // started with no scene, for the next one
    spare: bool,
}

/// Reads the resident memory of processes.
pub(crate) trait SampleMemory: Send {
    /// Resident bytes of each process of `pids` that is still running.
    fn sample(&mut self, pids: &[u32]) -> HashMap<u32, u64>;
}

struct SystemMemory(sysinfo::System);

impl SampleMemory for SystemMemory {
    fn sample(&mut self, pids: &[u32]) -> HashMap<u32, u64> {
        let pids = pids
            .iter()
            .map(|pid| sysinfo::Pid::from_u32(*pid))
            .collect::<Vec<_>>();
        self.0.refresh_processes_specifics(
            sysinfo::ProcessesToUpdate::Some(&pids),
            true,
            sysinfo::ProcessRefreshKind::nothing().with_memory(),
        );
        pids.iter()
            .filter_map(|pid| Some((pid.as_u32(), self.0.process(*pid)?.memory())))
            .collect()
    }
}

// What the hosts' IPC threads report to the pool. They must not hold the pool itself: dropping
// the pool is what ends its hosts.
#[derive(Default)]
struct PoolEvents {
    exits: Mutex<Vec<HostExit>>,
    failed_spawns: AtomicU32,
    ready_hosts: AtomicU32,
    // hosts that ended other than by retiring
    lost: AtomicU64,
    // the sandbox the first host to report one reported
    first_sandbox: Mutex<Option<SandboxStatus>>,
}

impl PoolEvents {
    // Starts keep failing and no host runs, so no scene can run. Starts run concurrently, so a
    // run of failures while another host serves scenes is not enough.
    fn unavailable(&self) -> bool {
        self.failed_spawns.load(Ordering::SeqCst) >= SPAWN_FAILURES_UNAVAILABLE
            && self.ready_hosts.load(Ordering::SeqCst) == 0
    }
}

/// The engine's scene hosts.
pub struct HostPool {
    options: HostOptions,
    launcher: Arc<dyn Launch>,
    hosts: Mutex<Vec<HostEntry>>,
    events: Arc<PoolEvents>,
    // how long a per-scene host outlives the removal of its scene
    pub(crate) retire_after: Duration,
    // how long a connected host has to report ready
    pub(crate) ready_timeout: Duration,
    memory: Mutex<Option<Box<dyn SampleMemory>>>,
    pub(crate) sample_every: Duration,
    inspect_refused: AtomicBool,
}

impl HostPool {
    pub(crate) fn new(
        options: HostOptions,
        launcher: Arc<dyn Launch>,
        memory: Box<dyn SampleMemory>,
    ) -> Self {
        Self {
            options,
            launcher,
            hosts: Default::default(),
            events: Default::default(),
            retire_after: KILL_GRACE_PERIOD,
            ready_timeout: HOST_READY_TIMEOUT,
            memory: Mutex::new(Some(memory)),
            sample_every: MEMORY_SAMPLE_PERIOD,
            inspect_refused: AtomicBool::new(false),
        }
    }

    /// Starts the hosts the placement needs before any scene arrives. With `Shared` this waits
    /// until the one host has connected.
    pub(crate) fn start(self: &Arc<Self>) -> anyhow::Result<()> {
        match self.options.placement {
            Placement::Shared => {
                let (ready_sx, ready_rx) = tokio::sync::oneshot::channel();
                let entry = self.start_host(Some(ready_sx), None)?;
                self.hosts.lock().unwrap().push(entry);
                ready_rx.blocking_recv()?
            }
            Placement::PerScene => {
                self.start_spares(&mut self.hosts.lock().unwrap());
                if let Some(memory) = self.memory.lock().unwrap().take() {
                    let pool = Arc::downgrade(self);
                    let every = self.sample_every;
                    std::thread::spawn(move || sample_memory(pool, memory, every));
                }
                Ok(())
            }
        }
    }

    fn start_spares(&self, hosts: &mut Vec<HostEntry>) {
        let spares = hosts.iter().filter(|entry| entry.spare).count();
        for _ in spares..self.options.spares {
            match self.start_host(None, None) {
                Ok(mut entry) => {
                    entry.spare = true;
                    hosts.push(entry);
                }
                Err(e) => {
                    error!("no spare scene host: {e:#}");
                    break;
                }
            }
        }
    }

    // Records each host's resident memory and ends the ones over the cap.
    fn check_memory(&self, memory: &mut dyn SampleMemory) {
        let hosts = self
            .hosts
            .lock()
            .unwrap()
            .iter()
            .map(|entry| entry.host.clone())
            .collect::<Vec<_>>();
        let pids = hosts
            .iter()
            .filter_map(|host| *host.pid.lock().unwrap())
            .collect::<Vec<_>>();
        let resident = memory.sample(&pids);
        for host in hosts {
            let Some(rss) = host.pid.lock().unwrap().and_then(|pid| resident.get(&pid)) else {
                continue;
            };
            host.rss_bytes.store(*rss, Ordering::Relaxed);
            // an idle spare runs no scene code, so only its fixed footprint counts
            let idle = host.scenes.lock().unwrap().live().next().is_none();
            if !idle && self.options.memory_cap.is_some_and(|cap| *rss > cap) {
                error!("scene host at {rss} resident bytes is over its memory cap; ending it");
                host.end(HostExitReason::Memory);
            }
        }
    }

    /// Queues a scene on the host its placement picks. Never waits for a host to start.
    pub(crate) fn place(&self, mut command: NewSceneCommand) -> anyhow::Result<()> {
        if command.info.inspect && self.options.sandbox != SandboxMode::Off {
            command.info.inspect = false;
            if !self.inspect_refused.swap(true, Ordering::Relaxed) {
                warn!("scenes run without the inspector: their scene hosts are sandboxed");
            }
        }
        let mut hosts = self.hosts.lock().unwrap();
        match self.options.placement {
            Placement::Shared => {
                let entry = hosts
                    .first()
                    .filter(|entry| command.assign_to(&entry.host.scenes))
                    .ok_or_else(|| anyhow!("scene host lost"))?;
                entry
                    .new_scene
                    .send(command)
                    .map_err(|e| anyhow!("ipc channel closed ({e})"))
            }
            Placement::PerScene => {
                hosts.retain(|entry| entry.host.state() != HostState::Lost);
                // a spare is ready once its sandbox report was accepted
                let spare = hosts.iter_mut().find(|entry| {
                    entry.spare
                        && entry.host.state() == HostState::Ready
                        && command.assign_to(&entry.host.scenes)
                });
                match spare {
                    Some(entry) => {
                        entry.spare = false;
                        // a host lost meanwhile reports the scene, which it was assigned
                        let _ = entry.new_scene.send(command);
                    }
                    None => hosts.push(self.start_host(None, Some(command))?),
                }
                self.start_spares(&mut hosts);
                Ok(())
            }
        }
    }

    fn start_host(
        &self,
        ready: Option<tokio::sync::oneshot::Sender<anyhow::Result<()>>>,
        scene: Option<NewSceneCommand>,
    ) -> anyhow::Result<HostEntry> {
        let host = Arc::new(Host::new());
        let (new_scene, new_scene_rx) = unbounded_channel();
        if let Some(scene) = scene {
            scene.assign_to(&host.scenes);
            let _ = new_scene.send(scene);
        }
        let thread = HostThread {
            host: host.clone(),
            launcher: self.launcher.clone(),
            events: self.events.clone(),
            placement: self.options.placement,
            sandbox: self.options.sandbox,
            retire_after: self.retire_after,
            ready_timeout: self.ready_timeout,
        };
        let spawned = std::thread::Builder::new()
            .name("scene-host-ipc".to_owned())
            .spawn(move || thread.run(new_scene_rx, ready));
        if let Err(e) = spawned {
            // on the caller's thread, which drains the scenes' responses: never wait on them
            host.set_state(HostState::Lost);
            self.events.failed_spawns.fetch_add(1, Ordering::SeqCst);
            for (id, responses) in host.scenes.lock().unwrap().close() {
                let error = SceneResponse::Error(format!("scene host failed to start: {e}"));
                let _ = responses.try_send((SceneId(Entity::from_bits(id)), error));
            }
            return Err(e).context("start a scene host thread");
        }
        Ok(HostEntry {
            host,
            new_scene,
            spare: false,
        })
    }

    /// Lost hosts not yet drained, oldest first.
    pub(crate) fn drain_exits(&self) -> Vec<HostExit> {
        std::mem::take(&mut *self.events.exits.lock().unwrap())
    }

    /// Whether the last host spawns all failed and no host runs, so no scene can run.
    #[cfg(test)]
    pub(crate) fn unavailable(&self) -> bool {
        self.events.unavailable()
    }

    pub(crate) fn info(&self) -> PoolInfo {
        PoolInfo {
            placement: self.options.placement,
            memory_cap: self.options.memory_cap,
            sandbox: self.options.sandbox,
            hosts: self.hosts(),
            lost_hosts: self.events.lost.load(Ordering::Relaxed),
        }
    }

    /// The sandbox the first host to report one reported.
    pub(crate) fn first_sandbox(&self) -> Option<SandboxStatus> {
        *self.events.first_sandbox.lock().unwrap()
    }

    /// Ends the running host with process id `pid`, as if it had exited. False if there is none.
    pub(crate) fn end_host(&self, pid: u32) -> bool {
        let hosts = self.hosts.lock().unwrap();
        let host = hosts
            .iter()
            .map(|entry| &entry.host)
            .find(|host| host.state() != HostState::Lost && *host.pid.lock().unwrap() == Some(pid));
        host.inspect(|host| host.end(HostExitReason::Exit))
            .is_some()
    }

    pub(crate) fn hosts(&self) -> Vec<SceneHostInfo> {
        let hosts = self.hosts.lock().unwrap();
        hosts
            .iter()
            .map(|entry| entry.host.info())
            .filter(|info| info.state != HostState::Lost)
            .collect()
    }
}

// A host's IPC thread: starts the scene host, serves it until it ends, then ends the process
// and reports the scenes it took with it.
struct HostThread {
    host: Arc<Host>,
    launcher: Arc<dyn Launch>,
    events: Arc<PoolEvents>,
    placement: Placement,
    sandbox: SandboxMode,
    retire_after: Duration,
    ready_timeout: Duration,
}

/// Why a scene host did not become ready.
struct StartFailure {
    reason: HostExitReason,
    status: HostStatus,
    error: anyhow::Error,
}

impl StartFailure {
    fn spawn(error: anyhow::Error) -> Self {
        Self {
            reason: HostExitReason::Spawn,
            status: HostStatus::default(),
            error,
        }
    }
}

impl HostThread {
    fn run(
        self,
        new_scene: UnboundedReceiver<NewSceneCommand>,
        ready: Option<tokio::sync::oneshot::Sender<anyhow::Result<()>>>,
    ) {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .context("build the scene host's runtime");
        let endpoint = ipc_socket_name();
        let launched = rt.map_err(StartFailure::spawn).and_then(|rt| {
            let command = HostCommand::scene_host(&endpoint, self.placement, self.sandbox)
                .map_err(StartFailure::spawn)?;
            let link = rt.block_on(self.start(endpoint, command))?;
            Ok((rt, link))
        });
        let (rt, mut link) = match launched {
            Ok(launched) => launched,
            Err(StartFailure {
                reason,
                status,
                error,
            }) => {
                error!("runtime initialization failed: {error:#}");
                self.host.set_state(HostState::Lost);
                if let Some(ready) = ready {
                    let _ = ready.send(Err(error));
                    return;
                }
                let failed = self.events.failed_spawns.fetch_add(1, Ordering::SeqCst) + 1;
                self.report_loss(
                    status,
                    reason,
                    format!("scene host failed to start: {error}"),
                );
                if self.events.unavailable() && EXIT_ON_SIDECAR_LOSS.load(Ordering::SeqCst) {
                    error!("{failed} scene hosts in a row failed to start and none runs; exiting process");
                    std::process::exit(match reason {
                        HostExitReason::Sandbox => EXIT_SANDBOX_UNAVAILABLE,
                        _ => 1,
                    });
                }
                return;
            }
        };
        self.events.failed_spawns.store(0, Ordering::SeqCst);
        self.events.ready_hosts.fetch_add(1, Ordering::SeqCst);
        *self.host.pid.lock().unwrap() = link.process.id();
        self.host.set_state(HostState::Ready);
        if let Some(ready) = ready {
            let _ = ready.send(Ok(()));
        }

        let connection = run_host_connection(
            link.inbound,
            link.outbound,
            new_scene,
            self.host.scenes.clone(),
        );
        let retired = retired(&self.host.scenes, self.retire_after);
        let ended = rt.block_on(async {
            tokio::select! {
                ended = connection => Some(ended),
                () = retired, if self.placement == Placement::PerScene => None,
                () = self.host.end.notified() => Some(ConnectionEnd::Closed),
            }
        });
        self.host.stop_taking_scenes();
        link.process.start_kill();
        let status = rt.block_on(link.process.wait()).unwrap_or_default();
        self.host.set_state(HostState::Lost);
        self.events.ready_hosts.fetch_sub(1, Ordering::SeqCst);

        match self.placement {
            Placement::Shared => {
                // Orchestrated headless server only (EXIT_ON_SIDECAR_LOSS): the IPC loops ended,
                // so the JS sidecar is gone and the engine can run no scene code. Nothing here
                // can respawn it, and the parent's engine-down recovery never fires for a still-
                // running process — so exit hard and let the supervisor restart the whole engine.
                // Desktop/tests leave the flag false and end quietly (tests build a fresh runtime
                // per app, where this is a normal shutdown, not a crash).
                if ended.is_some() {
                    self.events.lost.fetch_add(1, Ordering::Relaxed);
                }
                if EXIT_ON_SIDECAR_LOSS.load(Ordering::SeqCst) {
                    error!("dcl_deno_ipc runtime terminated (JS sidecar lost); exiting process");
                    std::process::exit(1);
                }
            }
            Placement::PerScene => {
                // a host retired after its scenes were removed lost nothing
                if let Some(ended) = ended {
                    let ended_by_engine = *self.host.end_reason.lock().unwrap();
                    let reason = ended_by_engine.unwrap_or(match ended {
                        ConnectionEnd::Closed => HostExitReason::Exit,
                        ConnectionEnd::Rejected => HostExitReason::Protocol,
                    });
                    // an idle spare that ended lost no scene
                    if self.report_loss(status, reason, format!("scene host exited: {status}")) {
                        self.events.lost.fetch_add(1, Ordering::Relaxed);
                    }
                }
            }
        }
    }

    // Starts the host and reads its sandbox report. The host gets no scene before it is accepted.
    async fn start(
        &self,
        endpoint: String,
        command: HostCommand,
    ) -> Result<HostLink, StartFailure> {
        let mut link = self
            .launcher
            .launch(endpoint, command)
            .await
            .map_err(|error| StartFailure {
                reason: HostExitReason::Spawn,
                status: HostStatus::default(),
                error,
            })?;
        let ready = tokio::time::timeout(self.ready_timeout, host_ready(&mut link.inbound)).await;
        let (reason, error) = match ready {
            Ok(Ok(Some(sandbox))) => {
                self.events
                    .first_sandbox
                    .lock()
                    .unwrap()
                    .get_or_insert(sandbox);
                *self.host.sandbox.lock().unwrap() = Some(sandbox);
                if sandbox.satisfies(self.sandbox) {
                    return Ok(link);
                }
                let refused = SandboxRefused(format!(
                    "its sandbox {sandbox:?} does not meet {}",
                    self.sandbox.as_str()
                ));
                (HostExitReason::Sandbox, refused.into())
            }
            // a host that cannot apply its sandbox says so with its exit status; any other early
            // end (V8 failing to start, a kill) is a failed start
            Ok(Ok(None)) => {
                let status = match tokio::time::timeout(HOST_EXIT_WAIT, link.process.wait()).await {
                    Ok(status) => status.unwrap_or_default(),
                    Err(_) => {
                        link.process.start_kill();
                        link.process.wait().await.unwrap_or_default()
                    }
                };
                let failure = if status.code == Some(HOST_EXIT_SANDBOX) {
                    StartFailure {
                        reason: HostExitReason::Sandbox,
                        status,
                        error: SandboxRefused(format!(
                            "it could not apply the {} sandbox",
                            self.sandbox.as_str()
                        ))
                        .into(),
                    }
                } else {
                    StartFailure {
                        reason: HostExitReason::Spawn,
                        status,
                        error: anyhow!("it ended before reporting ready ({status})"),
                    }
                };
                return Err(failure);
            }
            Ok(Err(e)) => (HostExitReason::Protocol, e),
            Err(_) => (
                HostExitReason::Spawn,
                anyhow!("it did not report ready within {:?}", self.ready_timeout),
            ),
        };
        link.process.start_kill();
        let status = link.process.wait().await.unwrap_or_default();
        Err(StartFailure {
            reason,
            status,
            error,
        })
    }

    // Reports the host's live scenes as lost: the record first, so it is there by the time a
    // scene is seen broken, then an error to each scene. Outside the runtime: it may wait. False
    // when the host ran no scene.
    fn report_loss(&self, status: HostStatus, reason: HostExitReason, message: String) -> bool {
        let lost = self.host.scenes.lock().unwrap().close();
        if lost.is_empty() {
            return false;
        }
        let scenes = lost
            .iter()
            .map(|(id, _)| SceneId(Entity::from_bits(*id)))
            .collect::<Vec<_>>();
        self.events.exits.lock().unwrap().push(HostExit {
            scenes: scenes.clone(),
            code: status.code,
            signal: status.signal,
            reason,
        });
        for (scene, (_, responses)) in scenes.into_iter().zip(lost) {
            // the channel is bounded; this message must not be shed
            let _ = responses.blocking_send((scene, SceneResponse::Error(message.clone())));
        }
        true
    }
}

// The sandbox a host reports in its first frame; `None` if it ends first.
async fn host_ready(
    inbound: &mut (impl AsyncRead + Unpin),
) -> anyhow::Result<Option<SandboxStatus>> {
    let buffer = match read_frame(inbound).await {
        Ok(Some(buffer)) => buffer,
        Ok(None) => return Ok(None),
        Err(e) if e.is::<Rejected>() => return Err(e),
        // a truncated frame or a read error: the host closed the connection
        Err(_) => return Ok(None),
    };
    match rmp_serde::from_slice(&buffer) {
        Ok(SceneToEngine::HostReady { sandbox }) => Ok(Some(sandbox)),
        Ok(_) => Err(Rejected("the first frame is not host-ready".to_owned()).into()),
        Err(e) => Err(Rejected(format!("undecodable frame of {} bytes: {e}", buffer.len())).into()),
    }
}

// Completes once every scene the host was given has been removed for `grace`.
async fn retired(scenes: &Mutex<HostScenes>, grace: Duration) {
    loop {
        tokio::time::sleep(RETIRE_POLL).await;
        let retired_since = scenes.lock().unwrap().retired_since();
        if retired_since.is_some_and(|since| Instant::now().duration_since(since) >= grace) {
            return;
        }
    }
}

// The pool's memory sampler, until the pool is dropped.
fn sample_memory(pool: Weak<HostPool>, mut memory: Box<dyn SampleMemory>, every: Duration) {
    loop {
        std::thread::sleep(every);
        let Some(pool) = pool.upgrade() else {
            return;
        };
        pool.check_memory(&mut *memory);
    }
}

pub(crate) static POOL: Lazy<RwLock<Option<Arc<HostPool>>>> = Lazy::new(Default::default);

/// Starts the scene host pool and makes it the one `spawn_scene` uses. A later call replaces it,
/// which ends the previous pool's hosts.
pub fn init_runtime_with(options: HostOptions) -> anyhow::Result<()> {
    let memory = Box::new(SystemMemory(sysinfo::System::new()));
    let pool = Arc::new(HostPool::new(options, Arc::new(ProcessLauncher), memory));
    pool.start()?;
    *POOL.write().unwrap() = Some(pool);
    Ok(())
}

/// Scene hosts lost since the last call. Empty with [`Placement::Shared`].
pub fn drain_host_exits() -> Vec<HostExit> {
    let pool = POOL.read().unwrap();
    pool.as_ref()
        .map(|pool| pool.drain_exits())
        .unwrap_or_default()
}

/// The pool's current scene hosts.
pub fn scene_hosts() -> Vec<SceneHostInfo> {
    let pool = POOL.read().unwrap();
    pool.as_ref().map(|pool| pool.hosts()).unwrap_or_default()
}

/// The pool `spawn_scene` uses, if the runtime is initialized.
pub fn scene_host_pool() -> Option<PoolInfo> {
    let pool = POOL.read().unwrap();
    pool.as_ref().map(|pool| pool.info())
}

/// The sandbox the pool's first scene host reported, once one has.
pub fn first_host_sandbox() -> Option<SandboxStatus> {
    let pool = POOL.read().unwrap();
    pool.as_ref().and_then(|pool| pool.first_sandbox())
}

/// Ends the scene host with process id `pid` as if it had exited: its scenes are lost. False if
/// the pool has no such running host.
pub fn end_scene_host(pid: u32) -> bool {
    let pool = POOL.read().unwrap();
    pool.as_ref().is_some_and(|pool| pool.end_host(pid))
}
