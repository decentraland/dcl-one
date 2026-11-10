use super::{
    ipc_socket_name, run_host_connection, ConnectionEnd, HostScenes, NewSceneCommand,
    EXIT_ON_SIDECAR_LOSS, KILL_GRACE_PERIOD,
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
    ffi::OsString,
    path::PathBuf,
    pin::Pin,
    process::Stdio,
    sync::{
        atomic::{AtomicU32, Ordering},
        Arc, Mutex, RwLock,
    },
    time::{Duration, Instant},
};
use tokio::{
    io::{AsyncBufReadExt, AsyncRead, AsyncWrite},
    sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender},
};

/// Host spawns that fail in a row, with no host running, before the pool counts as unavailable.
const SPAWN_FAILURES_UNAVAILABLE: u32 = 3;
// a host that neither connects nor exits by then has failed to start
const HOST_CONNECT_TIMEOUT: Duration = Duration::from_secs(30);
// a host's output line is cut to this many bytes in the engine's log
const HOST_LOG_LINE_BYTES: usize = 4096;
// lines over this many per second of one host stream are dropped and counted
const HOST_LOG_LINES_PER_SECOND: u32 = 200;
// how often a host whose scenes were all removed checks whether its grace period is over
const RETIRE_POLL: Duration = Duration::from_millis(250);

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
    /// It could not be started.
    Spawn,
}

impl HostExitReason {
    pub fn as_str(self) -> &'static str {
        match self {
            HostExitReason::Exit => "exit",
            HostExitReason::Protocol => "protocol",
            HostExitReason::Spawn => "spawn",
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

/// A scene host as the pool sees it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SceneHostInfo {
    pub pid: Option<u32>,
    pub state: HostState,
    /// The scenes it runs, not counting removed ones.
    pub scenes: Vec<SceneId>,
}

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
}

impl Host {
    fn new() -> Self {
        Self {
            scenes: Default::default(),
            state: Mutex::new(HostState::Starting),
            pid: Default::default(),
        }
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
        }
    }
}

/// How to start the scene host binary.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct HostCommand {
    pub program: PathBuf,
    pub args: Vec<OsString>,
}

impl HostCommand {
    fn scene_host(endpoint: &str) -> anyhow::Result<Self> {
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
        Ok(Self {
            program,
            args: vec![endpoint.into()],
        })
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
}

// What the hosts' IPC threads report to the pool. They must not hold the pool itself: dropping
// the pool is what ends its hosts.
#[derive(Default)]
struct PoolEvents {
    exits: Mutex<Vec<HostExit>>,
    failed_spawns: AtomicU32,
    ready_hosts: AtomicU32,
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
}

impl HostPool {
    pub(crate) fn new(options: HostOptions, launcher: Arc<dyn Launch>) -> Self {
        Self {
            options,
            launcher,
            hosts: Default::default(),
            events: Default::default(),
            retire_after: KILL_GRACE_PERIOD,
        }
    }

    /// Starts the hosts the placement needs before any scene arrives. With `Shared` this waits
    /// until the one host has connected.
    pub(crate) fn start(&self) -> anyhow::Result<()> {
        match self.options.placement {
            Placement::Shared => {
                let (ready_sx, ready_rx) = tokio::sync::oneshot::channel();
                let entry = self.start_host(Some(ready_sx), None)?;
                self.hosts.lock().unwrap().push(entry);
                ready_rx.blocking_recv()?
            }
            Placement::PerScene => Ok(()),
        }
    }

    /// Queues a scene on the host its placement picks. Never waits for a host to start.
    pub(crate) fn place(&self, command: NewSceneCommand) -> anyhow::Result<()> {
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
                let entry = self.start_host(None, Some(command))?;
                hosts.push(entry);
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
            retire_after: self.retire_after,
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
        Ok(HostEntry { host, new_scene })
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
    retire_after: Duration,
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
        let launched = rt.and_then(|rt| {
            let command = HostCommand::scene_host(&endpoint)?;
            let link = rt.block_on(self.launcher.launch(endpoint, command))?;
            Ok((rt, link))
        });
        let (rt, mut link) = match launched {
            Ok(launched) => launched,
            Err(e) => {
                error!("runtime initialization failed: {e:#}");
                self.host.set_state(HostState::Lost);
                if let Some(ready) = ready {
                    let _ = ready.send(Err(e));
                    return;
                }
                let failed = self.events.failed_spawns.fetch_add(1, Ordering::SeqCst) + 1;
                self.report_loss(
                    HostStatus::default(),
                    HostExitReason::Spawn,
                    "scene host failed to start".to_owned(),
                );
                if self.events.unavailable() && EXIT_ON_SIDECAR_LOSS.load(Ordering::SeqCst) {
                    error!("{failed} scene hosts in a row failed to start and none runs; exiting process");
                    std::process::exit(1);
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
            }
        });
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
                if EXIT_ON_SIDECAR_LOSS.load(Ordering::SeqCst) {
                    error!("dcl_deno_ipc runtime terminated (JS sidecar lost); exiting process");
                    std::process::exit(1);
                }
            }
            Placement::PerScene => {
                // a host retired after its scenes were removed lost nothing
                if let Some(ended) = ended {
                    let reason = match ended {
                        ConnectionEnd::Closed => HostExitReason::Exit,
                        ConnectionEnd::Rejected => HostExitReason::Protocol,
                    };
                    self.report_loss(status, reason, format!("scene host exited: {status}"));
                }
            }
        }
    }

    // Reports the host's live scenes as lost: the record first, so it is there by the time a
    // scene is seen broken, then an error to each scene. Outside the runtime: it may wait.
    fn report_loss(&self, status: HostStatus, reason: HostExitReason, message: String) {
        let lost = self.host.scenes.lock().unwrap().close();
        if lost.is_empty() {
            return;
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

pub(crate) static POOL: Lazy<RwLock<Option<Arc<HostPool>>>> = Lazy::new(Default::default);

/// Starts the scene host pool and makes it the one `spawn_scene` uses. A later call replaces it,
/// which ends the previous pool's hosts.
pub fn init_runtime_with(options: HostOptions) -> anyhow::Result<()> {
    let pool = HostPool::new(options, Arc::new(ProcessLauncher));
    pool.start()?;
    *POOL.write().unwrap() = Some(Arc::new(pool));
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
