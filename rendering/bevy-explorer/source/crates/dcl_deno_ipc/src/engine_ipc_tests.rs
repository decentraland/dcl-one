use super::*;
use bevy::prelude::{Entity, Vec3};
use common::rpc::{RequestContext, RpcCall, RpcStreamSender, SCENE_IPC_CONTEXT};
use dcl_types::{
    scene_response_channel, RpcCalls, SceneCensus, SceneElapsedTime, SceneResourceCounters,
    SceneResponseReceiver,
};
use std::{collections::HashMap, time::Duration};
use tokio::{
    io::{DuplexStream, ReadHalf, WriteHalf},
    sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender},
};

// a broken expectation fails the test instead of hanging it
const TIMEOUT: Duration = Duration::from_secs(10);

/// Plays the scene host against `run_host_connection`, which runs on its own thread with its
/// own runtime, as `init_runtime` runs it.
struct Host {
    reader: ReadHalf<DuplexStream>,
    writer: WriteHalf<DuplexStream>,
    new_scene: Option<UnboundedSender<NewSceneCommand>>,
    responses_sx: TaggedSceneResponseSender,
    responses: SceneResponseReceiver,
    global: tokio::sync::broadcast::Sender<GlobalCrdtStateUpdate>,
    // dropping a scene's renderer channel kills the scene
    renderer_channels: HashMap<u64, UnboundedSender<RendererResponse>>,
    // the connection's count of rejected messages, once it has ended
    ended: tokio::sync::oneshot::Receiver<u64>,
    next_entity: u32,
}

impl Host {
    fn connect() -> Self {
        let (engine_end, host_end) = tokio::io::duplex(64 * 1024);
        let (engine_in, engine_out) = tokio::io::split(engine_end);
        let (reader, writer) = tokio::io::split(host_end);
        let (new_scene_sx, new_scene_rx) = unbounded_channel();
        let (ended_sx, ended) = tokio::sync::oneshot::channel();
        std::thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();
            let rejected = rt.block_on(run_host_connection(
                engine_in,
                engine_out,
                new_scene_rx,
                Default::default(),
            ));
            let _ = ended_sx.send(rejected);
        });
        // the context the host's reply channels are serialized in
        SCENE_IPC_CONTEXT.set(Some(RequestContext {
            registry: Default::default(),
            close_sender: unbounded_channel().0,
            next_id: 1,
        }));
        let (responses_sx, responses) = scene_response_channel();
        Self {
            reader,
            writer,
            new_scene: Some(new_scene_sx),
            responses_sx,
            responses,
            global: tokio::sync::broadcast::channel(16).0,
            renderer_channels: Default::default(),
            ended,
            next_entity: 0,
        }
    }

    /// Queues a scene as `spawn_scene` does and reads the `NewScene` the engine writes.
    async fn start_scene(
        &mut self,
        is_super: bool,
    ) -> (SceneId, Option<UnboundedReceiver<SystemApi>>) {
        self.next_entity += 1;
        let scene = SceneId(Entity::from_raw(self.next_entity));
        let (renderer_sx, renderer_rx) = unbounded_channel();
        let (system_api_sx, system_api_rx) = match is_super {
            true => {
                let (sx, rx) = unbounded_channel();
                (Some(sx), Some(rx))
            }
            false => (None, None),
        };
        let command = NewSceneCommand {
            id: scene.0.to_bits(),
            info: NewSceneInfo {
                initial_crdt_store: Default::default(),
                scene_context: CrdtContext::new(
                    scene,
                    "hash".to_owned(),
                    "title".to_owned(),
                    false,
                    false,
                    false,
                ),
                presence_context: 0,
                scene_js: String::new(),
                crdt_component_interfaces: Default::default(),
                storage_root: String::new(),
                inspect: false,
                is_super,
                scene_origin: Vec3::ZERO,
            },
            renderer_channel: renderer_rx,
            global_channel: self.global.subscribe(),
            response_channel: SceneResponseSender::new(scene, self.responses_sx.clone()),
            system_api_sender: system_api_sx,
        };
        let Ok(()) = self.new_scene.as_ref().unwrap().send(command) else {
            panic!("the engine stopped taking scenes");
        };
        let EngineToScene::NewScene(started, _) = self.read().await else {
            panic!("expected the engine to start the scene");
        };
        assert_eq!(started, scene.0.to_bits());
        self.renderer_channels
            .insert(scene.0.to_bits(), renderer_sx);
        (scene, system_api_rx)
    }

    /// Drops the scene's renderer channel and reads the `KillScene` the engine writes.
    async fn kill_scene(&mut self, scene: SceneId) {
        self.renderer_channels.remove(&scene.0.to_bits());
        let EngineToScene::KillScene(killed) = self.read().await else {
            panic!("expected the engine to kill the scene");
        };
        assert_eq!(killed, scene.0.to_bits());
    }

    async fn read(&mut self) -> EngineToScene {
        tokio::time::timeout(TIMEOUT, async {
            let len = self.reader.read_u64_le().await.unwrap();
            let mut buffer = vec![0; len as usize];
            self.reader.read_exact(&mut buffer).await.unwrap();
            rmp_serde::from_slice(&buffer).unwrap()
        })
        .await
        .expect("no frame from the engine")
    }

    async fn send(&mut self, message: &SceneToEngine) {
        write_msg(&mut self.writer, message).await.unwrap();
    }

    async fn send_raw(&mut self, bytes: &[u8]) {
        self.writer.write_all(bytes).await.unwrap();
    }

    /// A response the engine can tell apart from others by `marker`.
    async fn respond(&mut self, scene: SceneId, marker: u64) {
        let counters = SceneResourceCounters {
            sample_seq: marker,
            ..Default::default()
        };
        self.send(&SceneToEngine::SceneResponse(
            scene,
            SceneResponse::Stats(counters),
        ))
        .await;
    }

    async fn delivered_response(&mut self) -> (SceneId, SceneResponse) {
        tokio::time::timeout(TIMEOUT, self.responses.recv())
            .await
            .expect("no response delivered")
            .expect("response channel closed")
    }

    /// The scene and marker of the next response the engine delivered.
    async fn delivered(&mut self) -> (SceneId, u64) {
        let (scene, response) = self.delivered_response().await;
        let SceneResponse::Stats(counters) = response else {
            panic!("unexpected response {response:?}");
        };
        (scene, counters.sample_seq)
    }

    async fn connection_ends(&mut self) -> bool {
        tokio::time::timeout(TIMEOUT, &mut self.ended).await.is_ok()
    }

    /// Closes the connection from the host's side and returns how many messages it rejected.
    async fn close_and_count_rejected(mut self) -> u64 {
        self.writer.shutdown().await.unwrap();
        tokio::time::timeout(TIMEOUT, &mut self.ended)
            .await
            .expect("the connection did not end")
            .unwrap()
    }

    /// Whether the engine released its side: reading then finds the end of the stream.
    async fn engine_side_closed(&mut self) -> bool {
        matches!(
            tokio::time::timeout(TIMEOUT, self.reader.read_u8()).await,
            Ok(Err(e)) if e.kind() == std::io::ErrorKind::UnexpectedEof
        )
    }
}

#[tokio::test]
async fn oversized_frame_ends_the_host_connection() {
    let mut host = Host::connect();
    let (scene, _) = host.start_scene(false).await;
    host.respond(scene, 1).await;
    assert_eq!(host.delivered().await, (scene, 1));

    host.send_raw(&(MAX_FRAME_BYTES + 1).to_le_bytes()).await;

    assert!(host.connection_ends().await);
}

#[tokio::test]
async fn truncated_frame_ends_the_host_connection() {
    let mut host = Host::connect();
    let (scene, _) = host.start_scene(false).await;
    host.respond(scene, 1).await;
    assert_eq!(host.delivered().await, (scene, 1));

    host.send_raw(&100u64.to_le_bytes()).await;
    host.send_raw(&[0; 10]).await;
    host.writer.shutdown().await.unwrap();

    assert!(host.connection_ends().await);
}

#[tokio::test]
async fn frame_length_cut_short_ends_the_host_connection() {
    let mut host = Host::connect();
    let (scene, _) = host.start_scene(false).await;
    host.respond(scene, 1).await;
    assert_eq!(host.delivered().await, (scene, 1));

    host.send_raw(&[1, 0, 0]).await;
    host.writer.shutdown().await.unwrap();

    assert!(host.connection_ends().await);
}

fn send_async_nested(
    depth: usize,
) -> (
    SceneResponse,
    common::rpc::RpcResultReceiver<Result<serde_json::Value, String>>,
) {
    let mut params = serde_json::Value::Null;
    for _ in 0..depth {
        params = serde_json::Value::Array(vec![params]);
    }
    let (response, receiver) = common::rpc::RpcResultSender::channel();
    let call = RpcCall::SendAsync {
        body: common::rpc::RPCSendableMessage {
            method: "eth_call".to_owned(),
            params: vec![params],
        },
        response,
    };
    (SceneResponse::ImmediateRpcCall(call), receiver)
}

#[tokio::test]
async fn deeply_nested_frame_ends_the_host_connection() {
    let mut host = Host::connect();
    let (scene, _) = host.start_scene(false).await;
    let (shallow, _shallow_reply) = send_async_nested(8);
    host.send(&SceneToEngine::SceneResponse(scene, shallow))
        .await;
    let (_, delivered) = host.delivered_response().await;
    assert!(matches!(delivered, SceneResponse::ImmediateRpcCall(_)));

    let (deep, _deep_reply) = send_async_nested(MAX_DECODE_DEPTH * 2);
    host.send(&SceneToEngine::SceneResponse(scene, deep)).await;

    assert!(host.connection_ends().await);
}

#[tokio::test]
async fn undecodable_frame_ends_the_host_connection() {
    let mut host = Host::connect();
    let (scene, _) = host.start_scene(false).await;
    host.respond(scene, 1).await;
    assert_eq!(host.delivered().await, (scene, 1));

    // 0xc1 is reserved in msgpack and never valid
    host.send_raw(&1u64.to_le_bytes()).await;
    host.send_raw(&[0xc1]).await;

    assert!(host.connection_ends().await);
}

#[tokio::test]
async fn engine_side_loops_end_together() {
    // the host stops sending
    let mut host = Host::connect();
    host.start_scene(false).await;
    host.writer.shutdown().await.unwrap();
    assert!(host.connection_ends().await);
    assert!(host.engine_side_closed().await);

    // the engine stops sending
    let mut host = Host::connect();
    host.start_scene(false).await;
    host.new_scene = None;
    assert!(host.connection_ends().await);
    assert!(host.engine_side_closed().await);
}

#[tokio::test]
async fn response_for_a_scene_of_this_host_is_delivered() {
    let mut host = Host::connect();
    let (first, _) = host.start_scene(false).await;
    let (second, _) = host.start_scene(false).await;

    host.respond(second, 1).await;
    host.respond(first, 2).await;

    assert_eq!(host.delivered().await, (second, 1));
    assert_eq!(host.delivered().await, (first, 2));
}

#[tokio::test]
async fn response_for_an_unknown_scene_is_dropped_and_counted() {
    let mut host = Host::connect();
    let (scene, _) = host.start_scene(false).await;
    let rejected = rejected_host_messages();

    host.respond(SceneId(Entity::from_raw(1000)), 1).await;
    host.respond(scene, 2).await;

    assert_eq!(host.delivered().await, (scene, 2));
    assert!(rejected_host_messages() > rejected);
    assert_eq!(host.close_and_count_rejected().await, 1);
}

#[tokio::test]
async fn response_after_kill_is_delivered_within_the_grace_period() {
    let mut host = Host::connect();
    let (scene, _) = host.start_scene(false).await;
    host.kill_scene(scene).await;

    host.respond(scene, 1).await;

    assert_eq!(host.delivered().await, (scene, 1));
}

#[test]
fn a_killed_scene_is_forgotten_after_the_grace_period() {
    let mut scenes = HostScenes::default();
    let killed_at = Instant::now();
    scenes.start(1, scene_response_channel().0, None, killed_at);
    scenes.kill(1, killed_at);

    let before = killed_at + KILLED_SCENE_RETENTION - Duration::from_millis(1);
    assert!(scenes.responses(1, before).is_some());
    assert!(scenes
        .responses(1, killed_at + KILLED_SCENE_RETENTION)
        .is_none());
}

#[test]
fn a_killed_scene_is_pruned_when_another_starts_after_the_grace_period() {
    let mut scenes = HostScenes::default();
    let killed_at = Instant::now();
    scenes.start(1, scene_response_channel().0, None, killed_at);
    scenes.kill(1, killed_at);

    scenes.start(
        2,
        scene_response_channel().0,
        None,
        killed_at + KILLED_SCENE_RETENTION,
    );

    assert_eq!(scenes.0.len(), 1);
    assert!(scenes.0.contains_key(&2));
}

#[test]
fn a_scene_over_its_reply_channel_limit_is_refused_alone() {
    let limit = MAX_SCENE_IPC_CHANNELS as u64;
    let mut channels = SceneChannels::default();
    assert!(channels.open(1, 0..limit));
    assert!(channels.open(2, limit..limit + 10));

    assert!(!channels.open(1, limit + 10..limit + 11));
    assert!(channels.open(2, limit + 11..limit + 12));

    channels.close(0);
    assert!(channels.open(1, limit + 12..limit + 13));

    let mut held = channels.end(1);
    held.sort_unstable();
    assert_eq!(held.len(), MAX_SCENE_IPC_CHANNELS);
    assert_eq!((held[0], held[held.len() - 1]), (1, limit + 12));
    assert!(channels.is_ended(1));
    assert!(!channels.is_ended(2));
}

fn subscription() -> (RpcCall, common::rpc::RpcStreamReceiver<String>) {
    let (sender, receiver) = RpcStreamSender::channel();
    (RpcCall::SubscribePlayerConnected { sender }, receiver)
}

#[tokio::test]
async fn reply_channels_close_when_the_host_connection_ends() {
    let mut host = Host::connect();
    let (scene, _) = host.start_scene(false).await;
    let (call, _receiver) = subscription();
    host.send(&SceneToEngine::SceneResponse(
        scene,
        SceneResponse::ImmediateRpcCall(call),
    ))
    .await;
    let (_, SceneResponse::ImmediateRpcCall(RpcCall::SubscribePlayerConnected { sender })) =
        host.delivered_response().await
    else {
        panic!("expected the subscription");
    };
    assert!(!sender.is_closed());

    host.writer.shutdown().await.unwrap();

    assert!(host.connection_ends().await);
    assert!(sender.is_closed());
}

#[tokio::test]
async fn reply_channels_over_the_limit_end_the_host_connection() {
    let mut host = Host::connect();
    let (scene, _) = host.start_scene(false).await;
    let tick = |count| {
        let (calls, receivers): (Vec<_>, Vec<_>) = (0..count).map(|_| subscription()).unzip();
        let mut rpc_calls = RpcCalls::default();
        rpc_calls.extend(calls);
        let response = SceneResponse::Ok(
            SceneCensus {
                born: Default::default(),
                died: Default::default(),
            },
            Default::default(),
            SceneElapsedTime(0.0),
            Vec::new(),
            rpc_calls,
        );
        (SceneToEngine::SceneResponse(scene, response), receivers)
    };

    let (at_the_limit, _receivers) = tick(MAX_HOST_IPC_CHANNELS);
    host.send(&at_the_limit).await;
    let (_, delivered) = host.delivered_response().await;
    assert!(matches!(delivered, SceneResponse::Ok(..)));

    let (one_more, _receivers) = tick(1);
    host.send(&one_more).await;

    assert!(host.connection_ends().await);
}

#[tokio::test]
async fn system_api_from_a_host_with_a_system_scene_is_forwarded() {
    let mut host = Host::connect();
    host.start_scene(false).await;
    let (_, system_api) = host.start_scene(true).await;
    let mut system_api = system_api.unwrap();

    host.send(&SceneToEngine::SystemApi(SystemApi::LoginCancel))
        .await;

    let forwarded = tokio::time::timeout(TIMEOUT, system_api.recv()).await;
    assert!(matches!(forwarded, Ok(Some(SystemApi::LoginCancel))));
}

#[tokio::test]
async fn system_api_from_a_host_without_a_system_scene_is_dropped() {
    let mut host = Host::connect();
    let (scene, _) = host.start_scene(false).await;
    let rejected = rejected_host_messages();

    host.send(&SceneToEngine::SystemApi(SystemApi::LoginCancel))
        .await;
    host.respond(scene, 1).await;

    assert_eq!(host.delivered().await, (scene, 1));
    assert!(rejected_host_messages() > rejected);
    assert_eq!(host.close_and_count_rejected().await, 1);
}

#[tokio::test]
async fn system_api_before_any_scene_does_not_end_the_engine_loop() {
    let mut host = Host::connect();

    host.send(&SceneToEngine::SystemApi(SystemApi::LoginCancel))
        .await;
    let (scene, _) = host.start_scene(false).await;
    host.respond(scene, 1).await;

    assert_eq!(host.delivered().await, (scene, 1));
}
