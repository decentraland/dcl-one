use super::*;
use bevy::prelude::{Entity, Vec3};
use dcl_types::{scene_response_channel, SceneResourceCounters, SceneResponseReceiver};
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
    ended: tokio::sync::oneshot::Receiver<()>,
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
            rt.block_on(run_host_connection(engine_in, engine_out, new_scene_rx));
            let _ = ended_sx.send(());
        });
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

    /// The scene and marker of the next response the engine delivered.
    async fn delivered(&mut self) -> (SceneId, u64) {
        let (scene, response) = tokio::time::timeout(TIMEOUT, self.responses.recv())
            .await
            .expect("no response delivered")
            .expect("response channel closed");
        let SceneResponse::Stats(counters) = response else {
            panic!("unexpected response {response:?}");
        };
        (scene, counters.sample_seq)
    }

    async fn connection_ends(&mut self) -> bool {
        tokio::time::timeout(TIMEOUT, &mut self.ended).await.is_ok()
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
