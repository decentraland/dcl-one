use anyhow::Result;
use bevy::{
    log::{
        debug, info,
        tracing_subscriber::{self, EnvFilter},
        warn,
    },
    platform::collections::HashMap,
};
use common::rpc::{IpcMessage, RequestContext, SCENE_IPC_CONTEXT};
use dcl_deno_ipc::{write_msg, EngineToScene, SceneToEngine};
use dcl_types::{SceneId, SceneResponseReceiver, SceneResponseSender, TaggedSceneResponseSender};
use interprocess::local_socket::{
    tokio::{RecvHalf, SendHalf, Stream},
    traits::tokio::Stream as _,
    GenericFilePath, ToFsName,
};
use std::{env, sync::Arc};
use system_bridge::SystemApi;
use tokio::io::AsyncReadExt;

#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

// how long a killed scene gets to exit cleanly (finish its in-flight tick after its
// renderer channel closes) before its isolate is forcibly terminated
const KILL_GRACE_PERIOD: std::time::Duration = std::time::Duration::from_secs(5);

// a force-terminated scene thread still alive after this long is reported as a zombie
const ZOMBIE_MIN_AGE: std::time::Duration = std::time::Duration::from_secs(30);
const ZOMBIE_CHECK_INTERVAL: std::time::Duration = std::time::Duration::from_secs(60);

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or(EnvFilter::new("info")))
        .init();

    info!("IPC scene host initializing");

    // spawn runtime
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();

    // get the socket name from args
    let name_str = env::args().nth(1).expect("No socket name provided");
    let name = name_str.to_fs_name::<GenericFilePath>()?;

    // connect ipc stream
    let stream = rt.block_on(async move { Stream::connect(name).await.unwrap() });

    let (recv_half, send_half) = stream.split();

    // init engine
    dcl_deno::init_runtime();

    // init context
    let (close_sx, close_rx) = tokio::sync::mpsc::unbounded_channel();
    let (scene_sx, scene_rx) = tokio::sync::mpsc::channel(1000);
    let (system_api_sx, system_api_rx) = tokio::sync::mpsc::unbounded_channel();
    SCENE_IPC_CONTEXT.set(Some(RequestContext {
        registry: Default::default(),
        close_sender: close_sx,
        next_id: 1,
    }));

    let f_in = rt.spawn(scene_ipc_in(recv_half, scene_sx, system_api_sx));
    let f_out = rt.spawn(scene_ipc_out(send_half, scene_rx, close_rx, system_api_rx));

    rt.spawn(async {
        let mut interval = tokio::time::interval(ZOMBIE_CHECK_INTERVAL);
        loop {
            interval.tick().await;
            let zombies = dcl_deno::zombie_scene_count(ZOMBIE_MIN_AGE);
            if zombies > 0 {
                warn!(
                    "{zombies} killed scene thread(s) still alive {}s+ after force-termination",
                    ZOMBIE_MIN_AGE.as_secs()
                );
            }
        }
    });

    info!("IPC scene host running");

    let _ = rt.block_on(async move { tokio::join!(f_in, f_out) });

    Ok(())
}

async fn scene_ipc_out(
    mut stream: SendHalf,
    mut scene_rx: SceneResponseReceiver,
    mut close_rx: tokio::sync::mpsc::UnboundedReceiver<u64>,
    mut system_api_rx: tokio::sync::mpsc::UnboundedReceiver<SystemApi>,
) {
    loop {
        tokio::select! {
            scene_rx = scene_rx.recv() => {
                let Some((scene_id, response)) = scene_rx else {
                    warn!("scene_ipc_out exit on scene_rx closed");
                    return;
                };
                write_msg(&mut stream, &SceneToEngine::SceneResponse(scene_id, response)).await;
            }
            close_rx = close_rx.recv() => {
                let Some(close_id) = close_rx else {
                    warn!("scene_ipc_out exit on close_rx closed");
                    return;
                };

                let was_open = SCENE_IPC_CONTEXT.with(|ctx| {
                    let mut ctx = ctx.borrow_mut();
                    let ctx = ctx.as_mut().unwrap();
                    ctx.registry.remove(&close_id).is_some()
                });

                if was_open {
                    write_msg(&mut stream, &SceneToEngine::IpcMessage(close_id, IpcMessage::Closed)).await;
                }
            },
            system_api_rx = system_api_rx.recv() => {
                let Some(system_api) = system_api_rx else {
                    warn!("scene_ipc_out exit on system_api_rx closed");
                    return;
                };
                write_msg(&mut stream, &SceneToEngine::SystemApi(system_api)).await;
            }
        }
    }
}

async fn scene_ipc_in(
    mut stream: RecvHalf,
    scene_sx: TaggedSceneResponseSender,
    system_api_sx: tokio::sync::mpsc::UnboundedSender<SystemApi>,
) {
    let mut renderer_senders = HashMap::new();

    let mut global_senders = ContextSenders::default();

    while let Ok(len) = stream.read_u64_le().await {
        let mut buffer = vec![0u8; len as usize];
        stream.read_exact(&mut buffer).await.unwrap();
        let msg: EngineToScene = rmp_serde::from_slice(&buffer).unwrap();

        match msg {
            EngineToScene::NewScene(id, new_scene_info) => {
                global_senders.sweep();
                let global_rx = global_senders.subscribe(new_scene_info.presence_context);
                let response_sx = dcl_deno::spawn_scene(
                    new_scene_info.initial_crdt_store,
                    new_scene_info.scene_context,
                    common::structs::SceneJsFile(Arc::new(new_scene_info.scene_js)),
                    new_scene_info.crdt_component_interfaces,
                    SceneResponseSender::new(
                        SceneId(bevy::ecs::entity::Entity::from_bits(id)),
                        scene_sx.clone(),
                    ),
                    global_rx,
                    new_scene_info.storage_root,
                    new_scene_info.inspect,
                    new_scene_info.is_super.then(|| system_api_sx.clone()),
                    new_scene_info.scene_origin,
                );

                renderer_senders.insert(id, response_sx);
            }
            EngineToScene::KillScene(id) => {
                renderer_senders.remove(&id);
                // a dying scene holds its receiver until its thread exits, so this collects
                // contexts whose scenes have already gone
                global_senders.sweep();
                // dropping the sender lets a healthy scene finish its current tick and
                // exit cleanly; force-terminate only if the thread is still around after
                // a grace period (wedged in js, or stuck awaiting something external)
                tokio::spawn(async move {
                    tokio::time::sleep(KILL_GRACE_PERIOD).await;
                    dcl_deno::terminate_scene(dcl_types::SceneId(
                        bevy::ecs::entity::Entity::from_bits(id),
                    ));
                });
            }
            EngineToScene::SceneUpdate(id, renderer_response) => {
                let Some(sender) = renderer_senders.get(&id) else {
                    warn!("no sender with id {id}");
                    continue;
                };

                let _ = sender.send(renderer_response);
            }
            EngineToScene::GlobalUpdate(presence_context, data) => {
                global_senders.send(presence_context, data);
            }
            EngineToScene::IpcMessage(id, ipc_message) => {
                SCENE_IPC_CONTEXT.with(|ctx| {
                    let mut ctx = ctx.borrow_mut();
                    let ctx = ctx.as_mut().unwrap();

                    debug!(
                        "ipc response {} -> {}",
                        id,
                        !matches!(ipc_message, IpcMessage::Closed)
                    );

                    match ipc_message {
                        common::rpc::IpcMessage::Data(data) => {
                            if let Some(endpoint) = ctx.registry.get_mut(&id) {
                                endpoint.send(data);
                            }
                        }
                        common::rpc::IpcMessage::Closed => {
                            let _ = ctx.registry.remove(&id);
                        }
                    }
                });
            }
        }
    }
}

/// One broadcast channel per presence context; scenes subscribe to their own.
struct ContextSenders<T> {
    senders: HashMap<u64, tokio::sync::broadcast::Sender<T>>,
}

impl<T> Default for ContextSenders<T> {
    fn default() -> Self {
        Self {
            senders: HashMap::new(),
        }
    }
}

impl<T: Clone> ContextSenders<T> {
    fn subscribe(&mut self, context: u64) -> tokio::sync::broadcast::Receiver<T> {
        self.senders
            .entry(context)
            .or_insert_with(|| tokio::sync::broadcast::channel(1000).0)
            .subscribe()
    }

    /// Fails only with no live subscribers: every scene of the context is gone, so the
    /// channel is dropped (the engine side stops sending too).
    fn send(&mut self, context: u64, data: T) {
        if let Some(sender) = self.senders.get(&context) {
            if sender.send(data).is_err() {
                self.senders.remove(&context);
            }
        }
    }

    /// Drops channels with no subscriber left, which a quiet context never reaches through `send`.
    fn sweep(&mut self) {
        self.senders.retain(|_, sender| sender.receiver_count() > 0);
    }

    #[cfg(test)]
    fn contains(&self, context: u64) -> bool {
        self.senders.contains_key(&context)
    }
}

#[cfg(test)]
mod tests {
    use super::ContextSenders;

    #[test]
    fn a_context_is_dropped_once_its_last_scene_receiver_is_gone() {
        let mut senders = ContextSenders::<u32>::default();
        let first = senders.subscribe(1);
        let second = senders.subscribe(1);
        let other = senders.subscribe(2);

        drop(first);
        senders.sweep();
        assert!(senders.contains(1));

        drop(second);
        senders.sweep();
        assert!(!senders.contains(1));
        assert!(senders.contains(2));
        drop(other);
    }

    #[test]
    fn a_scene_joining_a_swept_context_receives_its_updates() {
        let mut senders = ContextSenders::<u32>::default();
        drop(senders.subscribe(1));
        senders.sweep();

        let mut receiver = senders.subscribe(1);
        senders.send(1, 7);
        assert_eq!(receiver.try_recv().unwrap(), 7);
    }

    #[test]
    fn sending_to_a_context_without_receivers_drops_it() {
        let mut senders = ContextSenders::<u32>::default();
        drop(senders.subscribe(1));
        senders.send(1, 7);
        assert!(!senders.contains(1));
    }
}
