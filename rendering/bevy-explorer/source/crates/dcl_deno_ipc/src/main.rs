use anyhow::Result;
use bevy::{
    log::{
        debug, error, info,
        tracing_subscriber::{self, EnvFilter},
        warn,
    },
    platform::collections::HashMap,
};
use common::rpc::{rmp_encode, IpcMessage, RequestContext, SCENE_IPC_CONTEXT};
use dcl_deno_ipc::{
    write_frame, write_msg, EngineToScene, SandboxMode, SceneChannels, SceneToEngine,
    KILL_GRACE_PERIOD, MAX_SCENE_IPC_CHANNELS,
};
use dcl_types::{
    SceneId, SceneResponse, SceneResponseReceiver, SceneResponseSender, TaggedSceneResponseSender,
};
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

thread_local! {
    // both IPC tasks run on the main thread's current-thread runtime
    static SCENE_CHANNELS: std::cell::RefCell<SceneChannels> = Default::default();
}

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or(EnvFilter::new("info")))
        .init();

    let mut args = env::args().skip(1);
    let name_str = args.next().expect("No socket name provided");
    if name_str == "--sandbox-self-test" {
        let mode = match args.next().as_deref() {
            None => SandboxMode::On,
            Some("--require") => SandboxMode::Require,
            Some(arg) => panic!("unknown argument {arg}"),
        };
        std::process::exit(sandbox_self_test(mode));
    }
    let mut v8_threads = None;
    let mut sandbox = SandboxMode::Off;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--v8-threads" => {
                let threads = args.next().and_then(|n| n.parse::<u32>().ok());
                let threads = threads.filter(|n| *n > 0);
                v8_threads = Some(threads.expect("--v8-threads takes a positive count"));
            }
            "--sandbox" => {
                let mode = args.next().map(|mode| mode.parse::<SandboxMode>());
                sandbox = mode.expect("--sandbox takes off, on or require")?;
            }
            _ => panic!("unknown argument {arg}"),
        }
    }

    let name = name_str.to_fs_name::<GenericFilePath>()?;

    info!("IPC scene host initializing");

    // spawn runtime
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();

    // connect ipc stream
    let stream = rt.block_on(async move { Stream::connect(name).await.unwrap() });

    // after the connect, which the sandbox would refuse, and before V8 starts its threads, which
    // must start under it
    let sandbox = match scene_host_sandbox::apply(sandbox) {
        Ok(status) => status,
        Err(e) => {
            error!("scene host sandbox not applied: {e:#}");
            std::process::exit(1);
        }
    };

    let (recv_half, mut send_half) = stream.split();

    // init engine
    dcl_deno::init_runtime(v8_threads);

    rt.block_on(write_msg(
        &mut send_half,
        &SceneToEngine::HostReady { sandbox },
    ))?;

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

    info!("IPC scene host running");

    // the engine closing its end ends the host, even with a scene stuck in a loop
    rt.block_on(async move {
        tokio::select! {
            _ = f_in => (),
            out = f_out => {
                if let Ok(Err(e)) = out {
                    warn!("scene host stopped writing to the engine: {e}");
                }
            }
        }
    });

    Ok(())
}

// Applies the sandbox to this process and tries what it must deny and allow. Prints the report as
// one JSON line; the exit code is 0 only when every check holds.
fn sandbox_self_test(mode: SandboxMode) -> i32 {
    let probe = scene_host_sandbox::Probe::prepare().expect("prepare the sandbox probe");
    match scene_host_sandbox::apply(mode) {
        Ok(status) => {
            let report = probe.run();
            let holds = report.all_hold();
            let output = serde_json::json!({ "status": status, "report": report, "holds": holds });
            println!("{output}");
            if holds {
                0
            } else {
                1
            }
        }
        Err(e) => {
            println!("{}", serde_json::json!({ "error": format!("{e:#}") }));
            1
        }
    }
}

async fn scene_ipc_out(
    mut stream: SendHalf,
    mut scene_rx: SceneResponseReceiver,
    mut close_rx: tokio::sync::mpsc::UnboundedReceiver<u64>,
    mut system_api_rx: tokio::sync::mpsc::UnboundedReceiver<SystemApi>,
) -> std::io::Result<()> {
    loop {
        tokio::select! {
            scene_rx = scene_rx.recv() => {
                let Some((scene_id, response)) = scene_rx else {
                    warn!("scene_ipc_out exit on scene_rx closed");
                    return Ok(());
                };
                let scene = scene_id.0.to_bits();
                if SCENE_CHANNELS.with_borrow(|channels| channels.is_ended(scene)) {
                    continue;
                }
                let last_channel = || SCENE_IPC_CONTEXT.with_borrow(|ctx| ctx.as_ref().unwrap().next_id);
                let before = last_channel();
                let frame = rmp_encode(&SceneToEngine::SceneResponse(scene_id, response))
                    .map_err(std::io::Error::other)?;
                let opened = before + 1..last_channel() + 1;
                if SCENE_CHANNELS.with_borrow_mut(|channels| channels.open(scene, opened.clone())) {
                    write_frame(&mut stream, &frame).await?;
                    continue;
                }

                warn!("[{scene_id:?}] ended: it holds more than {MAX_SCENE_IPC_CHANNELS} open reply channels");
                let held = SCENE_CHANNELS.with_borrow_mut(|channels| channels.end(scene));
                SCENE_IPC_CONTEXT.with_borrow_mut(|ctx| {
                    let registry = &mut ctx.as_mut().unwrap().registry;
                    for id in opened.chain(held.iter().copied()) {
                        registry.remove(&id);
                    }
                });
                for id in held {
                    write_msg(&mut stream, &SceneToEngine::IpcMessage(id, IpcMessage::Closed)).await?;
                }
                let error = SceneResponse::Error(format!(
                    "scene ended: more than {MAX_SCENE_IPC_CHANNELS} open reply channels"
                ));
                write_msg(&mut stream, &SceneToEngine::SceneResponse(scene_id, error)).await?;
                dcl_deno::terminate_scene(scene_id);
            }
            close_rx = close_rx.recv() => {
                let Some(close_id) = close_rx else {
                    warn!("scene_ipc_out exit on close_rx closed");
                    return Ok(());
                };

                SCENE_CHANNELS.with_borrow_mut(|channels| channels.close(close_id));
                let was_open = SCENE_IPC_CONTEXT.with(|ctx| {
                    let mut ctx = ctx.borrow_mut();
                    let ctx = ctx.as_mut().unwrap();
                    ctx.registry.remove(&close_id).is_some()
                });

                if was_open {
                    write_msg(&mut stream, &SceneToEngine::IpcMessage(close_id, IpcMessage::Closed)).await?;
                }
            },
            system_api_rx = system_api_rx.recv() => {
                let Some(system_api) = system_api_rx else {
                    warn!("scene_ipc_out exit on system_api_rx closed");
                    return Ok(());
                };
                write_msg(&mut stream, &SceneToEngine::SystemApi(system_api)).await?;
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

    // one broadcast channel per presence context; scenes subscribe to their own
    let mut global_senders: HashMap<u64, tokio::sync::broadcast::Sender<_>> = HashMap::new();

    while let Ok(len) = stream.read_u64_le().await {
        let mut buffer = vec![0u8; len as usize];
        stream.read_exact(&mut buffer).await.unwrap();
        let msg: EngineToScene = rmp_serde::from_slice(&buffer).unwrap();

        match msg {
            EngineToScene::NewScene(id, new_scene_info) => {
                let global_sx = global_senders
                    .entry(new_scene_info.presence_context)
                    .or_insert_with(|| tokio::sync::broadcast::channel(1000).0);
                let response_sx = dcl_deno::spawn_scene(
                    new_scene_info.initial_crdt_store,
                    new_scene_info.scene_context,
                    common::structs::SceneJsFile(Arc::new(new_scene_info.scene_js)),
                    new_scene_info.crdt_component_interfaces,
                    SceneResponseSender::new(
                        SceneId(bevy::ecs::entity::Entity::from_bits(id)),
                        scene_sx.clone(),
                    ),
                    global_sx.subscribe(),
                    new_scene_info.storage_root,
                    new_scene_info.inspect,
                    new_scene_info.is_super.then(|| system_api_sx.clone()),
                    new_scene_info.scene_origin,
                );

                renderer_senders.insert(id, response_sx);
            }
            EngineToScene::KillScene(id) => {
                renderer_senders.remove(&id);
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
                // send fails only with no live subscribers: every scene of the context
                // is gone, so drop the channel (the engine side stops sending too)
                if let Some(sender) = global_senders.get(&presence_context) {
                    if sender.send(data).is_err() {
                        global_senders.remove(&presence_context);
                    }
                }
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
                            SCENE_CHANNELS.with_borrow_mut(|channels| channels.close(id));
                        }
                    }
                });
            }
        }
    }
}
