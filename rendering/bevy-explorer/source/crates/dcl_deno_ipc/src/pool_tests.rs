use super::pool::*;
use super::*;
use bevy::prelude::{Entity, Vec3};
use dcl_types::scene_response_channel;
use std::time::Duration;
use tokio::{
    io::{DuplexStream, ReadHalf, WriteHalf},
    sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender},
};

// a broken expectation fails the test instead of hanging it
const TIMEOUT: Duration = Duration::from_secs(10);

/// Hands the pool in-memory hosts and gives the test the other end of each.
struct FakeLauncher {
    launched: UnboundedSender<FakeHost>,
}

impl FakeLauncher {
    fn new() -> (Arc<Self>, UnboundedReceiver<FakeHost>) {
        let (launched, hosts) = unbounded_channel();
        (Arc::new(Self { launched }), hosts)
    }
}

struct FakeHost {
    command: HostCommand,
    reader: ReadHalf<DuplexStream>,
    _writer: WriteHalf<DuplexStream>,
}

impl FakeHost {
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
}

struct FakeProcess;

impl HostProcess for FakeProcess {
    fn start_kill(&mut self) {}

    fn wait(&mut self) -> futures_util::future::BoxFuture<'_, std::io::Result<()>> {
        Box::pin(async { Ok(()) })
    }
}

impl Launch for FakeLauncher {
    fn launch(
        &self,
        _endpoint: String,
        command: HostCommand,
    ) -> futures_util::future::BoxFuture<'_, anyhow::Result<HostLink>> {
        let (engine_end, host_end) = tokio::io::duplex(64 * 1024);
        let (inbound, outbound) = tokio::io::split(engine_end);
        let (reader, writer) = tokio::io::split(host_end);
        let host = FakeHost {
            command,
            reader,
            _writer: writer,
        };
        let launched = self.launched.send(host);
        Box::pin(async move {
            launched.map_err(|_| anyhow!("the test stopped taking hosts"))?;
            Ok(HostLink {
                inbound: Box::pin(inbound),
                outbound: Box::pin(outbound),
                process: Box::new(FakeProcess),
            })
        })
    }
}

/// A scene as `spawn_scene` builds it, and the renderer channel that keeps it alive.
fn new_scene(entity: u32) -> (NewSceneCommand, UnboundedSender<RendererResponse>) {
    let scene = SceneId(Entity::from_raw(entity));
    let (renderer_sx, renderer_rx) = unbounded_channel();
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
            is_super: false,
            scene_origin: Vec3::ZERO,
        },
        renderer_channel: renderer_rx,
        global_channel: tokio::sync::broadcast::channel(16).1,
        response_channel: SceneResponseSender::new(scene, scene_response_channel().0),
        system_api_sender: None,
    };
    (command, renderer_sx)
}

fn runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
}

#[test]
fn shared_placement_puts_every_scene_on_one_host() {
    let (launcher, mut launched) = FakeLauncher::new();
    let pool = HostPool::new(
        HostOptions {
            placement: Placement::Shared,
        },
        launcher,
    );
    pool.start().unwrap();

    let scenes = [1, 2, 3].map(|entity| {
        let (command, renderer) = new_scene(entity);
        pool.place(command).unwrap();
        (Entity::from_raw(entity).to_bits(), renderer)
    });

    runtime().block_on(async {
        let mut host = launched.try_recv().expect("the pool started no host");
        assert_eq!(host.command.program.file_stem().unwrap(), "dcl_deno_ipc");
        for (scene, _) in &scenes {
            let EngineToScene::NewScene(started, _) = host.read().await else {
                panic!("expected the engine to start the scene");
            };
            assert_eq!(started, *scene);
        }
    });
    assert!(launched.try_recv().is_err());
    assert_eq!(pool.hosts().len(), 1);
    assert_eq!(pool.hosts()[0].state(), HostState::Ready);
}
