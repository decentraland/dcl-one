use anyhow::{anyhow, bail};
use bevy::log::{debug, error, warn};
use common::structs::SceneJsFile;
use common::{
    rpc::{rmp_encode, CancellationToken, IpcMessage, ResponseContext, ENGINE_IPC_CONTEXT},
    structs::GlobalCrdtStateUpdate,
};
use dcl_types::{
    interface::{crdt_context::CrdtContext, CrdtComponentInterfaces, CrdtStore},
    RendererResponse, SceneId, SceneResponse, SceneResponseSender, TaggedSceneResponseSender,
};
use interprocess::local_socket::{
    traits::tokio::{Listener, Stream as _},
    GenericFilePath, ListenerOptions, ToFsName,
};
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::{
    cell::RefCell,
    process::{Command, Stdio},
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        RwLock,
    },
    time::{Duration, Instant},
};
use system_bridge::SystemApi;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

// A frame over this ends the scene host. It must stay above the largest frame a host builds:
// a CRDT snapshot carries a scene's whole store, which the scene runtime caps at 512 MiB.
const MAX_FRAME_BYTES: u64 = 1024 * 1024 * 1024;
// a frame's buffer starts at most this large and grows only as its bytes arrive
const FRAME_PREALLOCATION_BYTES: u64 = 1024 * 1024;
// reply channels one scene host may hold open at once, across all its scenes
const MAX_HOST_IPC_CHANNELS: usize = 65536;
/// Reply channels one scene may hold open at once. The scene host ends a scene over it, so that
/// sixteen scenes at their limit fill a host's.
pub const MAX_SCENE_IPC_CHANNELS: usize = 4096;
// decoding recurses on the engine's stack, and scene RPC parameters are arbitrary JSON
const MAX_DECODE_DEPTH: usize = 32;

/// How long a killed scene gets to exit cleanly (finish its in-flight tick after its
/// renderer channel closes) before its isolate is forcibly terminated.
pub const KILL_GRACE_PERIOD: Duration = Duration::from_secs(5);
// how long after its kill a scene's responses are still accepted: its last tick may be in
// the pipe when the grace period ends
const KILLED_SCENE_RETENTION: Duration = KILL_GRACE_PERIOD.saturating_add(Duration::from_secs(5));

static REJECTED_HOST_MESSAGES: AtomicU64 = AtomicU64::new(0);

/// The scene host's count of the reply channels each scene holds open, so that one scene over
/// [`MAX_SCENE_IPC_CHANNELS`] ends alone instead of taking its host over the engine's limit.
#[derive(Default)]
pub struct SceneChannels {
    owners: std::collections::HashMap<u64, u64>,
    open: std::collections::HashMap<u64, usize>,
    ended: std::collections::HashSet<u64>,
}

impl SceneChannels {
    /// Records the channels a response of `scene` opened. False, recording nothing, when that
    /// would take the scene over its limit.
    pub fn open(&mut self, scene: u64, ids: std::ops::Range<u64>) -> bool {
        let open = self.open.get(&scene).copied().unwrap_or(0);
        let opened = ids.end.saturating_sub(ids.start) as usize;
        if open + opened > MAX_SCENE_IPC_CHANNELS {
            return false;
        }
        if opened > 0 {
            self.open.insert(scene, open + opened);
            self.owners.extend(ids.map(|id| (id, scene)));
        }
        true
    }

    pub fn close(&mut self, id: u64) {
        let Some(scene) = self.owners.remove(&id) else {
            return;
        };
        if let Some(open) = self.open.get_mut(&scene) {
            *open -= 1;
            if *open == 0 {
                self.open.remove(&scene);
            }
        }
    }

    /// Marks the scene ended and returns the channels it still holds, which the caller closes.
    pub fn end(&mut self, scene: u64) -> Vec<u64> {
        self.ended.insert(scene);
        self.open.remove(&scene);
        let held: Vec<u64> = self
            .owners
            .iter()
            .filter(|(_, owner)| **owner == scene)
            .map(|(id, _)| *id)
            .collect();
        for id in &held {
            self.owners.remove(id);
        }
        held
    }

    pub fn is_ended(&self, scene: u64) -> bool {
        self.ended.contains(&scene)
    }
}

/// Messages dropped because the scene host that sent them does not run the scene they are for:
/// a scene response for a scene it was not given, or a `SystemApi` message from a host that
/// runs no system scene.
pub fn rejected_host_messages() -> u64 {
    REJECTED_HOST_MESSAGES.load(Ordering::Relaxed)
}

#[derive(Serialize, Deserialize)]
pub struct NewSceneInfo {
    pub initial_crdt_store: CrdtStore,
    pub scene_context: CrdtContext,
    /// id of the crdt context (player-presence view) this scene subscribes to — routes
    /// `GlobalUpdate` messages to exactly the scenes of that context in the sidecar
    pub presence_context: u64,
    pub scene_js: String,
    pub crdt_component_interfaces: CrdtComponentInterfaces,
    pub storage_root: String,
    pub inspect: bool,
    pub is_super: bool,
    pub scene_origin: bevy::prelude::Vec3,
}

#[derive(Serialize, Deserialize)]
pub enum EngineToScene {
    NewScene(u64, Box<NewSceneInfo>),
    SceneUpdate(u64, RendererResponse),
    KillScene(u64),
    GlobalUpdate(u64, GlobalCrdtStateUpdate),
    IpcMessage(u64, IpcMessage),
}

#[derive(Serialize, Deserialize)]
pub enum SceneToEngine {
    SceneResponse(SceneId, SceneResponse),
    SystemApi(SystemApi),
    IpcMessage(u64, IpcMessage),
}

// The scenes one scene host was given, and where their responses go.
#[derive(Default)]
struct HostScenes(std::collections::HashMap<u64, HostScene>);

struct HostScene {
    responses: TaggedSceneResponseSender,
    // set for the scene the engine marked `is_super`
    system_api: Option<tokio::sync::mpsc::UnboundedSender<SystemApi>>,
    killed_at: Option<Instant>,
}

impl HostScene {
    fn is_tracked(&self, now: Instant) -> bool {
        self.killed_at
            .is_none_or(|killed_at| now.duration_since(killed_at) < KILLED_SCENE_RETENTION)
    }
}

impl HostScenes {
    fn start(
        &mut self,
        id: u64,
        responses: TaggedSceneResponseSender,
        system_api: Option<tokio::sync::mpsc::UnboundedSender<SystemApi>>,
        now: Instant,
    ) {
        self.0.retain(|_, scene| scene.is_tracked(now));
        self.0.insert(
            id,
            HostScene {
                responses,
                system_api,
                killed_at: None,
            },
        );
    }

    fn kill(&mut self, id: u64, now: Instant) {
        self.0.retain(|_, scene| scene.is_tracked(now));
        if let Some(scene) = self.0.get_mut(&id) {
            scene.killed_at.get_or_insert(now);
        }
    }

    fn responses(&self, id: u64, now: Instant) -> Option<&TaggedSceneResponseSender> {
        self.0
            .get(&id)
            .filter(|scene| scene.is_tracked(now))
            .map(|scene| &scene.responses)
    }

    fn system_api(&self, now: Instant) -> Option<&tokio::sync::mpsc::UnboundedSender<SystemApi>> {
        self.0
            .values()
            .filter(|scene| scene.is_tracked(now))
            .find_map(|scene| scene.system_api.as_ref())
    }
}

// Counts a message dropped because its scene host has no standing to send it, for the process
// and for this connection. Warns once per connection, since the host chooses how many it sends.
fn reject_host_message(rejected: &AtomicU64, message: std::fmt::Arguments) {
    REJECTED_HOST_MESSAGES.fetch_add(1, Ordering::Relaxed);
    if rejected.fetch_add(1, Ordering::Relaxed) == 0 {
        warn!("dropping {message}; further drops from this scene host are only counted");
    }
}

thread_local! {
    static HOST_SCENES: RefCell<HostScenes> = RefCell::new(HostScenes::default());
}

pub struct NewSceneCommand {
    id: u64,
    info: NewSceneInfo,
    renderer_channel: tokio::sync::mpsc::UnboundedReceiver<RendererResponse>,
    global_channel: tokio::sync::broadcast::Receiver<GlobalCrdtStateUpdate>,
    response_channel: SceneResponseSender,
    system_api_sender: Option<tokio::sync::mpsc::UnboundedSender<SystemApi>>,
}

#[allow(clippy::type_complexity)]
pub static NEW_SCENE_SENDER: Lazy<
    RwLock<Option<tokio::sync::mpsc::UnboundedSender<NewSceneCommand>>>,
> = Lazy::new(Default::default);

/// Opt-in for the orchestrated headless server ONLY: when set, a runtime whose IPC
/// loops end (the JS sidecar was lost) hard-exits the process so the supervisor can
/// restart the whole engine — otherwise a half-dead engine looks healthy forever and
/// the parent's engine-down recovery never fires. Left false for the desktop client
/// and for tests, which build a fresh runtime per app and must not kill the process.
pub static EXIT_ON_SIDECAR_LOSS: AtomicBool = AtomicBool::new(false);

// A fresh runtime is created for each test app. A 16-bit random suffix can
// collide with a previous runtime that is still shutting down in this process.
// The random starting point also separates endpoints after a PID is reused.
static NEXT_IPC_ID: Lazy<AtomicU64> = Lazy::new(|| AtomicU64::new(fastrand::u64(..)));

fn ipc_socket_name() -> String {
    let process_id = std::process::id();
    let id = NEXT_IPC_ID.fetch_add(1, Ordering::Relaxed);
    if cfg!(windows) {
        format!(r"\\.\pipe\dcl_ipc_{process_id:x}_{id:x}")
    } else {
        // Keep the basename short enough for macOS's Unix socket path limit.
        std::env::temp_dir()
            .join(format!("dcl_ipc_{process_id:x}_{id:x}.sock"))
            .to_string_lossy()
            .into_owned()
    }
}

pub fn init_runtime() -> anyhow::Result<()> {
    let (init_sx, init_rx) = tokio::sync::oneshot::channel::<anyhow::Result<()>>();

    std::thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();

        let name_str = ipc_socket_name();
        let name = name_str.clone().to_fs_name::<GenericFilePath>().unwrap();

        let listener = rt.block_on(async { ListenerOptions::new().name(name).create_tokio() });
        let listener = match listener {
            Ok(l) => l,
            Err(e) => {
                error!("failed to create listener: {e}");
                let _ = init_sx.send(Err(anyhow!(e)));
                return;
            }
        };

        let mut target = std::env::current_exe().unwrap();
        target.pop();

        // pop an extra folder when running tests
        if target.file_name().and_then(|s| s.to_str()) == Some("deps") {
            target.pop();
        }

        let mut target = target.join("dcl_deno_ipc");
        if cfg!(windows) {
            target.set_extension("exe");
        }

        let mut command = Command::new(&target);
        command
            .arg(name_str)
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit());
        // A console-subsystem child spawned by the GUI-subsystem app has no console to
        // inherit, so windows pops a visible one; suppress it. The inherited handles
        // still carry output to the parent's console when it has one (console builds).
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            command.creation_flags(CREATE_NO_WINDOW);
        }
        let mut child = command
            .spawn()
            .unwrap_or_else(|_| panic!("failed to spawn deno binary at {target:?}"));

        let stream = match rt.block_on(async { listener.accept().await }) {
            Ok(stream) => stream,
            Err(e) => {
                error!("runtime initialization failed: {e}");
                let _ = init_sx.send(Err(e.into()));
                child.wait().unwrap();
                return;
            }
        };

        let (ipc_inbound, ipc_outbound) = stream.split();

        let (new_scene_sx, new_scene_rx) = tokio::sync::mpsc::unbounded_channel();

        *NEW_SCENE_SENDER.write().unwrap() = Some(new_scene_sx);

        let _ = init_sx.send(Ok(()));

        rt.block_on(run_host_connection(ipc_inbound, ipc_outbound, new_scene_rx));
        let _ = child.kill();
        let _ = child.wait();

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
    });

    init_rx.blocking_recv()?
}

// Serves one scene host until either direction ends, and returns how many of its messages were
// rejected. Runs on a current-thread runtime, since the IPC context and the scene senders are
// thread-locals.
async fn run_host_connection(
    inbound: impl AsyncRead + Unpin + Send + 'static,
    outbound: impl AsyncWrite + Unpin + Send + 'static,
    new_scene: tokio::sync::mpsc::UnboundedReceiver<NewSceneCommand>,
) -> u64 {
    let (router_sx, router_rx) = tokio::sync::mpsc::unbounded_channel();
    let channels_open = CancellationToken::new();
    ENGINE_IPC_CONTEXT.set(Some(ResponseContext {
        ipc_channel_registry: Default::default(),
        ipc_router: router_sx,
        max_channels: Some(MAX_HOST_IPC_CHANNELS),
        parent_token: Some(channels_open.clone()),
    }));
    let _close_channels = channels_open.drop_guard();

    let rejected = std::sync::Arc::new(AtomicU64::new(0));
    let mut f_out = tokio::spawn(renderer_ipc_out(outbound, new_scene, router_rx));
    let mut f_in = tokio::spawn(renderer_ipc_in(inbound, rejected.clone()));

    let ended = tokio::select! {
        ended = &mut f_out => {
            f_in.abort();
            ended
        }
        ended = &mut f_in => {
            f_out.abort();
            ended
        }
    };
    match ended {
        Ok(Ok(())) => (),
        Ok(Err(e)) => warn!("scene host connection ended: {e:#}"),
        Err(e) => error!("scene host connection task failed: {e}"),
    }
    rejected.load(Ordering::Relaxed)
}

#[cfg(test)]
mod endpoint_tests {
    use super::ipc_socket_name;
    use std::collections::HashSet;

    #[test]
    fn concurrent_runtimes_have_distinct_endpoints() {
        let names = std::thread::scope(|scope| {
            let workers: Vec<_> = (0..16)
                .map(|_| scope.spawn(|| (0..128).map(|_| ipc_socket_name()).collect::<Vec<_>>()))
                .collect();
            workers
                .into_iter()
                .flat_map(|worker| worker.join().unwrap())
                .collect::<HashSet<_>>()
        });
        assert_eq!(names.len(), 16 * 128);
    }
}

#[allow(clippy::type_complexity)]
pub async fn renderer_ipc_out(
    mut stream: impl AsyncWrite + Unpin,
    mut new_scene: tokio::sync::mpsc::UnboundedReceiver<NewSceneCommand>,
    mut ipc_router: tokio::sync::mpsc::UnboundedReceiver<(u64, IpcMessage)>,
) -> anyhow::Result<()> {
    let (renderer_sx, mut renderer_rx) = tokio::sync::mpsc::unbounded_channel();

    // one receiver per live presence context, drained into the pipe as tagged
    // `GlobalUpdate`s so the sidecar can fan each stream to exactly the scenes of that
    // context. On every new scene the context's receiver is swapped for the incoming one
    // (created at the scene's snapshot time), replaying updates since the snapshot after
    // the `NewScene` message — the new scene misses nothing, and old scenes idempotently
    // re-apply a few duplicated crdt messages. The previous receiver's backlog is
    // flushed before the swap so old scenes stay continuous. A closed receiver (context
    // despawned) is dropped; a re-added scene room is a fresh context entity / fresh id.
    let mut global_receivers: std::collections::HashMap<
        u64,
        tokio::sync::broadcast::Receiver<GlobalCrdtStateUpdate>,
    > = Default::default();

    loop {
        tokio::select! {
            new_scene = new_scene.recv() => {
                let Some(NewSceneCommand{id,info,mut renderer_channel, global_channel, response_channel, system_api_sender }) = new_scene else {
                    warn!("renderer_ipc_out exit on new scene closed");
                    return Ok(());
                };

                HOST_SCENES.with_borrow_mut(|scenes| scenes.start(id, response_channel.channel().clone(), system_api_sender, Instant::now()));

                let presence_context = info.presence_context;
                if let Some(mut old_rx) = global_receivers.remove(&presence_context) {
                    loop {
                        match old_rx.try_recv() {
                            Ok(data) => {
                                write_msg(&mut stream, &EngineToScene::GlobalUpdate(presence_context, data)).await?;
                            }
                            Err(tokio::sync::broadcast::error::TryRecvError::Lagged(count)) => {
                                error!("global crdt state lagged, dropping {count} messages");
                            }
                            Err(_) => break,
                        }
                    }
                }
                global_receivers.insert(presence_context, global_channel);

                // spawn connector
                let renderer_sender = renderer_sx.clone();
                tokio::spawn(async move {
                    while let Some(renderer_response) = renderer_channel.recv().await {
                        let _ = renderer_sender.send(EngineToScene::SceneUpdate(id, renderer_response));
                    }
                    let _ = renderer_sender.send(EngineToScene::KillScene(id));
                });

                write_msg(&mut stream, &EngineToScene::NewScene(id, Box::new(info))).await?;
            }
            renderer_update = renderer_rx.recv() => {
                let Some(engine_to_scene) = renderer_update else {
                    warn!("renderer_ipc_out exit on inbound closed");
                    return Ok(());
                };
                if let EngineToScene::KillScene(id) = &engine_to_scene {
                    HOST_SCENES.with_borrow_mut(|scenes| scenes.kill(*id, Instant::now()));
                }
                write_msg(&mut stream, &engine_to_scene).await?;
            }
            (presence_context, received) = next_global_update(&mut global_receivers) => {
                match received {
                    Ok(data) => {
                        write_msg(&mut stream, &EngineToScene::GlobalUpdate(presence_context, data)).await?;
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(count)) => {
                        error!("global crdt state lagged, dropping {count} messages");
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                        global_receivers.remove(&presence_context);
                    }
                }
            }
            ipc = ipc_router.recv() => {
                let Some(ipc) = ipc else {
                    warn!("renderer_ipc_out exit on router closed");
                    return Ok(());
                };
                debug!("ipc {} -> {}", ipc.0, !matches!(ipc.1, IpcMessage::Closed));
                // the scene host doesn't echo our close back, so release the entry here
                if matches!(ipc.1, IpcMessage::Closed) {
                    ENGINE_IPC_CONTEXT.with_borrow_mut(|ctx| {
                        ctx.as_mut().unwrap().ipc_channel_registry.remove(&ipc.0)
                    });
                }
                write_msg(&mut stream, &EngineToScene::IpcMessage(ipc.0, ipc.1)).await?;
            }
        }
    }
}

// the next update from any live context's receiver; pends forever while none exist.
// `broadcast::Receiver::recv` is cancel-safe, so re-building the futures each select
// iteration loses nothing.
async fn next_global_update(
    receivers: &mut std::collections::HashMap<
        u64,
        tokio::sync::broadcast::Receiver<GlobalCrdtStateUpdate>,
    >,
) -> (
    u64,
    Result<GlobalCrdtStateUpdate, tokio::sync::broadcast::error::RecvError>,
) {
    if receivers.is_empty() {
        return std::future::pending().await;
    }
    let recvs = receivers
        .iter_mut()
        .map(|(context, rx)| {
            let context = *context;
            Box::pin(async move { (context, rx.recv().await) })
        })
        .collect::<Vec<_>>();
    futures_util::future::select_all(recvs).await.0
}

// The next frame's body, or `None` once the host has closed the connection between frames.
async fn read_frame(stream: &mut (impl AsyncRead + Unpin)) -> anyhow::Result<Option<Vec<u8>>> {
    let mut header = [0u8; 8];
    if stream.read(&mut header[..1]).await? == 0 {
        return Ok(None);
    }
    stream
        .read_exact(&mut header[1..])
        .await
        .map_err(|e| anyhow!("frame length cut short: {e}"))?;
    let len = u64::from_le_bytes(header);
    if len > MAX_FRAME_BYTES {
        bail!("frame of {len} bytes is over the {MAX_FRAME_BYTES}-byte limit");
    }
    let mut buffer = Vec::with_capacity(len.min(FRAME_PREALLOCATION_BYTES) as usize);
    stream.take(len).read_to_end(&mut buffer).await?;
    if buffer.len() as u64 != len {
        bail!("frame truncated at {} of {len} bytes", buffer.len());
    }
    Ok(Some(buffer))
}

pub async fn renderer_ipc_in(
    mut stream: impl AsyncRead + Unpin,
    rejected: std::sync::Arc<AtomicU64>,
) -> anyhow::Result<()> {
    while let Some(buffer) = read_frame(&mut stream).await? {
        let mut decoder = rmp_serde::Deserializer::from_read_ref(&buffer);
        decoder.set_max_depth(MAX_DECODE_DEPTH);
        let msg = SceneToEngine::deserialize(&mut decoder)
            .map_err(|e| anyhow!("undecodable frame of {} bytes: {e}", buffer.len()))?;

        match msg {
            SceneToEngine::SceneResponse(id, scene_response) => {
                HOST_SCENES.with_borrow(|scenes| {
                    let Some(sender) = scenes.responses(id.0.to_bits(), Instant::now()) else {
                        reject_host_message(
                            &rejected,
                            format_args!(
                                "a response for scene {id:?}, which this scene host does not run"
                            ),
                        );
                        return Ok(());
                    };
                    // HEADLESS-ONLY: EXIT_ON_SIDECAR_LOSS marks the orchestrated headless server,
                    // where every scene shares one engine. There, ending this connection trips the
                    // process exit above — killing the whole engine and every co-tenant scene. So
                    // shed the response instead: a scene that outruns the bevy-side drain only
                    // stalls itself. Desktop and tests (flag unset) end the connection.
                    if EXIT_ON_SIDECAR_LOSS.load(Ordering::SeqCst) {
                        if let Err(e) = sender.try_send((id, scene_response)) {
                            warn!("dropping scene response: renderer channel unavailable ({e})");
                        }
                    } else if let Err(e) = sender.try_send((id, scene_response)) {
                        bail!("renderer channel unavailable ({e})");
                    }
                    Ok(())
                })?
            }
            SceneToEngine::IpcMessage(id, ipc_message) => {
                let IpcMessage::Closed = ipc_message else {
                    warn!("ignoring unexpected ipc data message for channel {id}");
                    continue;
                };

                ENGINE_IPC_CONTEXT.with(|ctx| {
                    let mut ctx = ctx.borrow_mut();
                    let ctx = ctx.as_mut().unwrap();

                    if let Some(token) = ctx.ipc_channel_registry.remove(&id) {
                        token.cancel();
                    }
                })
            }
            SceneToEngine::SystemApi(system_command) => {
                let forwarded = HOST_SCENES.with_borrow(|scenes| {
                    let sender = scenes.system_api(Instant::now())?;
                    let _ = sender.send(system_command);
                    Some(())
                });
                if forwarded.is_none() {
                    reject_host_message(
                        &rejected,
                        format_args!(
                            "a system api message from a scene host that runs no system scene"
                        ),
                    );
                }
            }
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub fn spawn_scene(
    initial_crdt_store: CrdtStore,
    scene_context: CrdtContext,
    presence_context: u64,
    scene_js: SceneJsFile,
    crdt_component_interfaces: CrdtComponentInterfaces,
    renderer_sender: SceneResponseSender,
    global_update_receiver: tokio::sync::broadcast::Receiver<GlobalCrdtStateUpdate>,
    storage_root: String,
    inspect: bool,
    super_user: Option<tokio::sync::mpsc::UnboundedSender<SystemApi>>,
    scene_origin: bevy::prelude::Vec3,
) -> (
    tokio::sync::mpsc::UnboundedSender<RendererResponse>,
    Option<tokio::sync::oneshot::Sender<()>>,
) {
    let is_super = super_user.is_some();
    let id = scene_context.scene_id;

    let (main_sx, thread_rx) = tokio::sync::mpsc::unbounded_channel::<RendererResponse>();

    let ipc_out = NEW_SCENE_SENDER.read().unwrap();
    let Some(ipc_out) = ipc_out.as_ref() else {
        error!("scene {id:?} not started: ipc runtime is not initialized");
        return (main_sx, None);
    };

    if let Err(e) = ipc_out.send(NewSceneCommand {
        id: id.0.to_bits(),
        info: NewSceneInfo {
            initial_crdt_store,
            scene_context,
            presence_context,
            scene_js: scene_js.0.to_string(),
            crdt_component_interfaces,
            storage_root,
            inspect,
            is_super,
            scene_origin,
        },
        renderer_channel: thread_rx,
        global_channel: global_update_receiver,
        response_channel: renderer_sender,
        system_api_sender: super_user,
    }) {
        error!("scene {id:?} not started: ipc channel closed ({e})");
    }

    (main_sx, None)
}

pub async fn write_msg<T: Serialize>(
    stream: &mut (impl AsyncWrite + Unpin),
    value: &T,
) -> std::io::Result<()> {
    let bytes = rmp_encode(value).map_err(std::io::Error::other)?;
    write_frame(stream, &bytes).await
}

/// Writes a message [`rmp_encode`] already encoded.
pub async fn write_frame(
    stream: &mut (impl AsyncWrite + Unpin),
    bytes: &[u8],
) -> std::io::Result<()> {
    stream
        .write_all(&(bytes.len() as u64).to_le_bytes())
        .await?;
    stream.write_all(bytes).await
}

#[cfg(test)]
mod engine_ipc_tests;
