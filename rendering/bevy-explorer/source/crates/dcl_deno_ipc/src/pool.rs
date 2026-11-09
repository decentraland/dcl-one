use super::{
    ipc_socket_name, run_host_connection, HostScenes, NewSceneCommand, EXIT_ON_SIDECAR_LOSS,
};
use anyhow::{anyhow, Context};
use bevy::log::error;
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
    sync::{atomic::Ordering, Arc, Mutex, RwLock},
};
use tokio::{
    io::{AsyncRead, AsyncWrite},
    sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender},
};

/// Which scene host a new scene runs in.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Placement {
    /// One scene host for every scene.
    #[default]
    Shared,
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

/// What the engine knows about one scene host. Its IPC thread owns the process.
pub(crate) struct Host {
    pub(crate) scenes: Arc<Mutex<HostScenes>>,
    state: Mutex<HostState>,
}

impl Host {
    fn new() -> Self {
        Self {
            scenes: Default::default(),
            state: Mutex::new(HostState::Starting),
        }
    }

    pub(crate) fn state(&self) -> HostState {
        *self.state.lock().unwrap()
    }

    fn set_state(&self, state: HostState) {
        *self.state.lock().unwrap() = state;
    }
}

/// How to start the scene host binary.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct HostCommand {
    pub program: PathBuf,
    pub args: Vec<OsString>,
}

impl HostCommand {
    fn scene_host(endpoint: &str) -> Self {
        let mut program = std::env::current_exe().unwrap();
        program.pop();

        // pop an extra folder when running tests
        if program.file_name().and_then(|s| s.to_str()) == Some("deps") {
            program.pop();
        }

        let mut program = program.join("dcl_deno_ipc");
        if cfg!(windows) {
            program.set_extension("exe");
        }
        Self {
            program,
            args: vec![endpoint.into()],
        }
    }
}

/// A running scene host and the engine's end of its connection.
pub(crate) struct HostLink {
    pub inbound: Pin<Box<dyn AsyncRead + Send>>,
    pub outbound: Pin<Box<dyn AsyncWrite + Send>>,
    pub process: Box<dyn HostProcess>,
}

pub(crate) trait HostProcess: Send {
    fn start_kill(&mut self);
    fn wait(&mut self) -> BoxFuture<'_, std::io::Result<()>>;
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
struct ProcessLauncher;

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
            process
                .args(&command.args)
                .stdout(Stdio::inherit())
                .stderr(Stdio::inherit())
                .kill_on_drop(true);
            // A console-subsystem child spawned by the GUI-subsystem app has no console to
            // inherit, so windows pops a visible one; suppress it. The inherited handles
            // still carry output to the parent's console when it has one (console builds).
            #[cfg(windows)]
            {
                const CREATE_NO_WINDOW: u32 = 0x0800_0000;
                process.creation_flags(CREATE_NO_WINDOW);
            }
            let mut child = process
                .spawn()
                .with_context(|| format!("failed to spawn deno binary at {:?}", command.program))?;

            let stream = tokio::select! {
                stream = listener.accept() => stream?,
                status = child.wait() => {
                    return Err(anyhow!("scene host exited before connecting: {}", status?));
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

impl HostProcess for tokio::process::Child {
    fn start_kill(&mut self) {
        let _ = tokio::process::Child::start_kill(self);
    }

    fn wait(&mut self) -> BoxFuture<'_, std::io::Result<()>> {
        Box::pin(async move { tokio::process::Child::wait(self).await.map(|_| ()) })
    }
}

struct HostEntry {
    host: Arc<Host>,
    // dropping it ends the host: its connection closes once no scene can be queued on it
    new_scene: UnboundedSender<NewSceneCommand>,
}

/// The engine's scene hosts.
pub struct HostPool {
    options: HostOptions,
    launcher: Arc<dyn Launch>,
    hosts: Mutex<Vec<HostEntry>>,
}

impl HostPool {
    pub(crate) fn new(options: HostOptions, launcher: Arc<dyn Launch>) -> Self {
        Self {
            options,
            launcher,
            hosts: Default::default(),
        }
    }

    /// Starts the hosts the placement needs before any scene arrives. With `Shared` this waits
    /// until the one host has connected.
    pub(crate) fn start(&self) -> anyhow::Result<()> {
        match self.options.placement {
            Placement::Shared => {
                let (ready_sx, ready_rx) = tokio::sync::oneshot::channel();
                let entry = self.start_host(Some(ready_sx));
                self.hosts.lock().unwrap().push(entry);
                ready_rx.blocking_recv()?
            }
        }
    }

    /// Queues a scene on the host its placement picks.
    pub(crate) fn place(&self, command: NewSceneCommand) -> anyhow::Result<()> {
        let hosts = self.hosts.lock().unwrap();
        let entry = match self.options.placement {
            Placement::Shared => hosts.first(),
        };
        let Some(entry) = entry.filter(|entry| entry.host.state() != HostState::Lost) else {
            return Err(anyhow!("scene host lost"));
        };
        entry
            .new_scene
            .send(command)
            .map_err(|e| anyhow!("ipc channel closed ({e})"))
    }

    fn start_host(
        &self,
        ready: Option<tokio::sync::oneshot::Sender<anyhow::Result<()>>>,
    ) -> HostEntry {
        let host = Arc::new(Host::new());
        let (new_scene, new_scene_rx) = unbounded_channel();
        let thread_host = host.clone();
        let launcher = self.launcher.clone();
        std::thread::spawn(move || run_host(thread_host, launcher, new_scene_rx, ready));
        HostEntry { host, new_scene }
    }

    #[cfg(test)]
    pub(crate) fn hosts(&self) -> Vec<Arc<Host>> {
        let hosts = self.hosts.lock().unwrap();
        hosts.iter().map(|entry| entry.host.clone()).collect()
    }
}

// The body of a host's IPC thread: starts the scene host, serves it until the connection ends,
// then ends the process.
fn run_host(
    host: Arc<Host>,
    launcher: Arc<dyn Launch>,
    new_scene: UnboundedReceiver<NewSceneCommand>,
    ready: Option<tokio::sync::oneshot::Sender<anyhow::Result<()>>>,
) {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();

    let endpoint = ipc_socket_name();
    let command = HostCommand::scene_host(&endpoint);
    let mut link = match rt.block_on(launcher.launch(endpoint, command)) {
        Ok(link) => link,
        Err(e) => {
            error!("runtime initialization failed: {e:#}");
            host.set_state(HostState::Lost);
            if let Some(ready) = ready {
                let _ = ready.send(Err(e));
            }
            return;
        }
    };
    host.set_state(HostState::Ready);
    if let Some(ready) = ready {
        let _ = ready.send(Ok(()));
    }

    rt.block_on(run_host_connection(
        link.inbound,
        link.outbound,
        new_scene,
        host.scenes.clone(),
    ));
    link.process.start_kill();
    let _ = rt.block_on(link.process.wait());
    host.set_state(HostState::Lost);

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

pub(crate) static POOL: Lazy<RwLock<Option<Arc<HostPool>>>> = Lazy::new(Default::default);

/// Starts the scene host pool and makes it the one `spawn_scene` uses. A later call replaces it,
/// which ends the previous pool's hosts.
pub fn init_runtime_with(options: HostOptions) -> anyhow::Result<()> {
    let pool = HostPool::new(options, Arc::new(ProcessLauncher));
    pool.start()?;
    *POOL.write().unwrap() = Some(Arc::new(pool));
    Ok(())
}
