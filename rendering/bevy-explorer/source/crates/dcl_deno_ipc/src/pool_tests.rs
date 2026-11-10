use super::pool::*;
use super::*;
use bevy::prelude::{Entity, Vec3};
use dcl_types::{scene_response_channel, SceneResourceCounters, SceneResponseReceiver};
use std::{collections::HashSet, sync::atomic::AtomicU32, time::Duration};
use tokio::{
    io::{DuplexStream, ReadHalf, WriteHalf},
    sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender},
};

// a broken expectation fails the test instead of hanging it
const TIMEOUT: Duration = Duration::from_secs(10);
// how long to wait for something that must not happen
const QUIET: Duration = Duration::from_millis(500);

/// Hands the pool in-memory hosts and gives the test the other end of each.
struct FakeLauncher {
    launched: UnboundedSender<FakeHost>,
    // launches that fail before any succeeds again
    failures: AtomicU32,
    next_pid: AtomicU32,
}

impl FakeLauncher {
    fn new() -> (Arc<Self>, UnboundedReceiver<FakeHost>) {
        let (launched, hosts) = unbounded_channel();
        let launcher = Self {
            launched,
            failures: AtomicU32::new(0),
            next_pid: AtomicU32::new(1),
        };
        (Arc::new(launcher), hosts)
    }
}

impl Launch for FakeLauncher {
    fn launch(
        &self,
        _endpoint: String,
        command: HostCommand,
    ) -> futures_util::future::BoxFuture<'_, anyhow::Result<HostLink>> {
        if self
            .failures
            .try_update(Ordering::SeqCst, Ordering::SeqCst, |n| n.checked_sub(1))
            .is_ok()
        {
            return Box::pin(async { Err(anyhow::anyhow!("no such binary")) });
        }
        let (engine_end, host_end) = tokio::io::duplex(64 * 1024);
        let (inbound, outbound) = tokio::io::split(engine_end);
        let (reader, writer) = tokio::io::split(host_end);
        let killed = Arc::new(AtomicBool::new(false));
        let host = FakeHost {
            command,
            reader,
            writer,
            killed: killed.clone(),
        };
        let pid = self.next_pid.fetch_add(1, Ordering::SeqCst);
        let launched = self.launched.send(host);
        Box::pin(async move {
            launched.map_err(|_| anyhow::anyhow!("the test stopped taking hosts"))?;
            Ok(HostLink {
                inbound: Box::pin(inbound),
                outbound: Box::pin(outbound),
                process: Box::new(FakeProcess { pid, killed }),
            })
        })
    }
}

struct FakeProcess {
    pid: u32,
    killed: Arc<AtomicBool>,
}

impl HostProcess for FakeProcess {
    fn id(&self) -> Option<u32> {
        Some(self.pid)
    }

    fn start_kill(&mut self) {
        self.killed.store(true, Ordering::SeqCst);
    }

    fn wait(&mut self) -> futures_util::future::BoxFuture<'_, std::io::Result<HostStatus>> {
        let status = match self.killed.load(Ordering::SeqCst) {
            true => HostStatus {
                code: None,
                signal: Some(9),
            },
            false => HostStatus {
                code: Some(0),
                signal: None,
            },
        };
        Box::pin(async move { Ok(status) })
    }
}

/// The scene host's end of a fake host.
struct FakeHost {
    command: HostCommand,
    reader: ReadHalf<DuplexStream>,
    writer: WriteHalf<DuplexStream>,
    killed: Arc<AtomicBool>,
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

    /// The scene id of the `NewScene` the engine writes next.
    async fn started(&mut self) -> u64 {
        let EngineToScene::NewScene(started, _) = self.read().await else {
            panic!("expected the engine to start a scene");
        };
        started
    }

    async fn respond(&mut self, scene: u64, marker: u64) {
        let counters = SceneResourceCounters {
            sample_seq: marker,
            ..Default::default()
        };
        let response = SceneResponse::Stats(counters);
        let message = SceneToEngine::SceneResponse(SceneId(Entity::from_bits(scene)), response);
        write_msg(&mut self.writer, &message).await.unwrap();
    }

    fn killed(&self) -> bool {
        self.killed.load(Ordering::SeqCst)
    }
}

/// A scene as `spawn_scene` builds it, and the renderer channel that keeps it alive.
fn new_scene(
    entity: u32,
    responses: &TaggedSceneResponseSender,
) -> (NewSceneCommand, UnboundedSender<RendererResponse>) {
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
        response_channel: SceneResponseSender::new(scene, responses.clone()),
        system_api_sender: None,
    };
    (command, renderer_sx)
}

fn scene_id(entity: u32) -> u64 {
    Entity::from_raw(entity).to_bits()
}

fn pool(placement: Placement) -> (HostPool, UnboundedReceiver<FakeHost>, Arc<FakeLauncher>) {
    let (launcher, launched) = FakeLauncher::new();
    let pool = HostPool::new(HostOptions { placement }, launcher.clone());
    (pool, launched, launcher)
}

/// Places scenes `entities` and returns the renderer channels that keep them alive.
fn place(
    pool: &HostPool,
    responses: &TaggedSceneResponseSender,
    entities: impl IntoIterator<Item = u32>,
) -> Vec<UnboundedSender<RendererResponse>> {
    entities
        .into_iter()
        .map(|entity| {
            let (command, renderer) = new_scene(entity, responses);
            pool.place(command).unwrap();
            renderer
        })
        .collect()
}

/// The next host the pool launched, with the scene it was given.
async fn next_host(launched: &mut UnboundedReceiver<FakeHost>) -> (u64, FakeHost) {
    let mut host = tokio::time::timeout(TIMEOUT, launched.recv())
        .await
        .expect("no host launched")
        .unwrap();
    (host.started().await, host)
}

async fn delivered(responses: &mut SceneResponseReceiver) -> (u64, SceneResponse) {
    let (scene, response) = tokio::time::timeout(TIMEOUT, responses.recv())
        .await
        .expect("no response delivered")
        .expect("response channel closed");
    (scene.0.to_bits(), response)
}

async fn nothing_delivered(responses: &mut SceneResponseReceiver) -> bool {
    tokio::time::timeout(QUIET, responses.recv()).await.is_err()
}

async fn eventually(mut condition: impl FnMut() -> bool) {
    tokio::time::timeout(TIMEOUT, async {
        while !condition() {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("condition never held");
}

fn is_error_starting_with(response: &SceneResponse, prefix: &str) -> bool {
    matches!(response, SceneResponse::Error(message) if message.starts_with(prefix))
}

#[tokio::test]
async fn shared_placement_puts_every_scene_on_one_host() {
    let (pool, mut launched, _) = pool(Placement::Shared);
    std::thread::scope(|scope| scope.spawn(|| pool.start()).join().unwrap()).unwrap();
    let (responses, _receiver) = scene_response_channel();

    let _scenes = place(&pool, &responses, [1, 2, 3]);

    let mut host = launched.try_recv().expect("the pool started no host");
    assert_eq!(host.command.program.file_stem().unwrap(), "dcl_deno_ipc");
    for entity in [1, 2, 3] {
        assert_eq!(host.started().await, scene_id(entity));
    }
    assert!(launched.try_recv().is_err());
    let hosts = pool.hosts();
    assert_eq!(hosts.len(), 1);
    assert_eq!(hosts[0].state, HostState::Ready);
    assert_eq!(hosts[0].scenes.len(), 3);
}

#[tokio::test]
async fn per_scene_placement_gives_each_scene_its_own_host() {
    let (pool, mut launched, _) = pool(Placement::PerScene);
    let (responses, _receiver) = scene_response_channel();

    let _scenes = place(&pool, &responses, [1, 2, 3]);

    let mut started = HashSet::new();
    let mut fake_hosts = Vec::new();
    for _ in 0..3 {
        let (scene, host) = next_host(&mut launched).await;
        started.insert(scene);
        fake_hosts.push(host);
    }
    assert_eq!(started, [1, 2, 3].map(scene_id).into());
    eventually(|| {
        pool.hosts()
            .iter()
            .all(|host| host.state == HostState::Ready)
    })
    .await;
    let hosts = pool.hosts();
    assert_eq!(hosts.len(), 3);
    assert!(hosts.iter().all(|host| host.scenes.len() == 1));
}

#[tokio::test]
async fn losing_a_host_breaks_only_its_scene() {
    let (pool, mut launched, _) = pool(Placement::PerScene);
    let (responses, mut receiver) = scene_response_channel();
    let _scenes = place(&pool, &responses, [1, 2]);
    let (first, first_host) = next_host(&mut launched).await;
    let (second, mut second_host) = next_host(&mut launched).await;

    drop(first_host);
    second_host.respond(second, 7).await;

    let mut received = [
        delivered(&mut receiver).await,
        delivered(&mut receiver).await,
    ];
    received.sort_by_key(|(scene, _)| *scene != first);
    assert_eq!(received[0].0, first);
    assert!(is_error_starting_with(&received[0].1, "scene host exited"));
    assert_eq!(received[1].0, second);
    assert!(matches!(&received[1].1, SceneResponse::Stats(c) if c.sample_seq == 7));
    assert!(nothing_delivered(&mut receiver).await);
    let hosts = pool.hosts();
    assert_eq!(hosts.len(), 1);
    assert_eq!(hosts[0].scenes, [SceneId(Entity::from_bits(second))]);
    assert!(!second_host.killed());
}

#[tokio::test]
async fn a_host_exit_is_reported_once_with_its_scenes() {
    let (pool, mut launched, _) = pool(Placement::PerScene);
    let (responses, mut receiver) = scene_response_channel();
    let _scenes = place(&pool, &responses, [1]);
    let (scene, host) = next_host(&mut launched).await;

    drop(host);

    let (errored, _) = delivered(&mut receiver).await;
    assert_eq!(errored, scene);
    let exits = pool.drain_exits();
    assert_eq!(
        exits,
        [HostExit {
            scenes: vec![SceneId(Entity::from_bits(scene))],
            code: None,
            signal: Some(9),
            reason: HostExitReason::Exit,
        }]
    );
    assert!(nothing_delivered(&mut receiver).await);
    assert!(pool.drain_exits().is_empty());
}

#[tokio::test]
async fn a_host_ended_over_a_frame_is_reported_as_a_protocol_exit() {
    let (pool, mut launched, _) = pool(Placement::PerScene);
    let (responses, mut receiver) = scene_response_channel();
    let _scenes = place(&pool, &responses, [1]);
    let (scene, mut host) = next_host(&mut launched).await;

    // 0xc1 is reserved in msgpack and never valid
    host.writer.write_all(&1u64.to_le_bytes()).await.unwrap();
    host.writer.write_all(&[0xc1]).await.unwrap();

    let (errored, _) = delivered(&mut receiver).await;
    assert_eq!(errored, scene);
    let exits = pool.drain_exits();
    assert_eq!(exits.len(), 1);
    assert_eq!(exits[0].reason, HostExitReason::Protocol);
    assert!(host.killed());
}

#[tokio::test]
async fn a_removed_scene_leaves_no_host_and_no_loss_report() {
    let (mut pool, mut launched, _) = pool(Placement::PerScene);
    pool.retire_after = Duration::from_millis(100);
    let (responses, mut receiver) = scene_response_channel();
    let removed_scene = place(&pool, &responses, [1]);
    let (removed, mut removed_host) = next_host(&mut launched).await;
    let _kept_scene = place(&pool, &responses, [2]);
    let (_, kept_host) = next_host(&mut launched).await;

    drop(removed_scene);

    let EngineToScene::KillScene(killed) = removed_host.read().await else {
        panic!("expected the engine to kill the scene");
    };
    assert_eq!(killed, removed);
    eventually(|| removed_host.killed()).await;
    eventually(|| pool.hosts().len() == 1).await;
    assert!(nothing_delivered(&mut receiver).await);
    assert!(pool.drain_exits().is_empty());
    assert!(!kept_host.killed());
    assert_eq!(pool.hosts()[0].scenes, [SceneId(Entity::from_raw(2))]);
}

/// The scenes of the next `count` responses, each a failed start.
async fn spawn_failures(receiver: &mut SceneResponseReceiver, count: usize) -> HashSet<u64> {
    let mut failed = HashSet::new();
    for _ in 0..count {
        let (scene, response) = delivered(receiver).await;
        assert!(is_error_starting_with(
            &response,
            "scene host failed to start"
        ));
        failed.insert(scene);
    }
    failed
}

#[tokio::test]
async fn three_failed_spawns_with_no_host_running_report_the_pool_as_unavailable() {
    let (pool, mut launched, launcher) = pool(Placement::PerScene);
    let (responses, mut receiver) = scene_response_channel();
    // two failures, then a host that starts
    launcher.failures.store(2, Ordering::SeqCst);
    let _failed = place(&pool, &responses, [1, 2]);
    assert_eq!(spawn_failures(&mut receiver, 2).await.len(), 2);
    let _started = place(&pool, &responses, [3]);
    let host = next_host(&mut launched).await;
    eventually(|| {
        pool.hosts()
            .iter()
            .any(|host| host.state == HostState::Ready)
    })
    .await;
    assert!(!pool.unavailable());

    // three in a row while a host runs
    launcher.failures.store(3, Ordering::SeqCst);
    let _failed = place(&pool, &responses, [4, 5, 6]);
    assert_eq!(
        spawn_failures(&mut receiver, 3).await,
        [4, 5, 6].map(scene_id).into()
    );
    assert!(!pool.unavailable());

    // the running host is lost, and the next start fails too
    drop(host);
    let (lost, _) = delivered(&mut receiver).await;
    assert_eq!(lost, scene_id(3));
    launcher.failures.store(1, Ordering::SeqCst);
    let _failed = place(&pool, &responses, [7]);
    assert_eq!(
        spawn_failures(&mut receiver, 1).await,
        [7].map(scene_id).into()
    );
    eventually(|| pool.unavailable()).await;
    let exits = pool.drain_exits();
    assert_eq!(exits.len(), 7);
    assert_eq!(
        exits
            .iter()
            .filter(|exit| exit.reason == HostExitReason::Spawn && exit.scenes.len() == 1)
            .count(),
        6
    );
}
