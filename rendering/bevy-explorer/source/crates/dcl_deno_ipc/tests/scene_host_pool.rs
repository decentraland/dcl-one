use bevy::prelude::{Entity, Vec3};
use common::structs::SceneJsFile;
use dcl_deno_ipc::{
    drain_host_exits, init_runtime_with, scene_hosts, spawn_scene, HostExit, HostExitReason,
    HostOptions, HostState, Placement, SceneHostInfo,
};
use dcl_types::{
    interface::crdt_context::CrdtContext, scene_response_channel, RendererResponse, SceneId,
    SceneResponse, SceneResponseSender, TaggedSceneResponseSender,
};
use std::{
    process::Command,
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::sync::mpsc::UnboundedSender;

const TIMEOUT: Duration = Duration::from_secs(30);
const SCENE_JS: &str = "exports.onStart = async () => {}; exports.onUpdate = async () => {};";

fn start_scene(
    entity: u32,
    responses: &TaggedSceneResponseSender,
) -> (SceneId, UnboundedSender<RendererResponse>) {
    let scene = SceneId(Entity::from_raw(entity));
    let context = CrdtContext::new(
        scene,
        format!("scene-{entity}"),
        "title".to_owned(),
        false,
        false,
        false,
    );
    let (renderer, _) = spawn_scene(
        Default::default(),
        context,
        0,
        SceneJsFile(Arc::new(SCENE_JS.to_owned())),
        Default::default(),
        SceneResponseSender::new(scene, responses.clone()),
        tokio::sync::broadcast::channel(16).1,
        String::new(),
        false,
        None,
        Vec3::ZERO,
    );
    (scene, renderer)
}

fn eventually<T>(what: &str, mut found: impl FnMut() -> Option<T>) -> T {
    let deadline = Instant::now() + TIMEOUT;
    loop {
        if let Some(found) = found() {
            return found;
        }
        assert!(Instant::now() < deadline, "timed out waiting for {what}");
        std::thread::sleep(Duration::from_millis(20));
    }
}

fn ready_host_of(scene: SceneId) -> Option<SceneHostInfo> {
    scene_hosts()
        .into_iter()
        .find(|host| host.scenes == [scene] && host.state == HostState::Ready)
}

fn kill(pid: u32) {
    let status = match cfg!(windows) {
        true => Command::new("taskkill")
            .args(["/F", "/PID", &pid.to_string()])
            .status(),
        false => Command::new("kill").args(["-9", &pid.to_string()]).status(),
    };
    assert!(status.unwrap().success());
}

#[test]
fn a_killed_scene_host_ends_only_its_scene() {
    init_runtime_with(HostOptions {
        placement: Placement::PerScene,
    })
    .unwrap();
    let (responses, mut receiver) = scene_response_channel();
    let (killed, _killed_renderer) = start_scene(1, &responses);
    let (kept, _kept_renderer) = start_scene(2, &responses);
    let killed_host = eventually("the first host", || ready_host_of(killed));
    let kept_host = eventually("the second host", || ready_host_of(kept));
    assert_ne!(killed_host.pid, kept_host.pid);

    kill(killed_host.pid.unwrap());

    let exit = eventually("the exit report", || drain_host_exits().pop());
    assert_eq!(
        exit,
        HostExit {
            scenes: vec![killed],
            reason: HostExitReason::Exit,
            ..exit.clone()
        }
    );
    if cfg!(unix) {
        assert_eq!(exit.signal, Some(9));
    }
    let lost = eventually("an error for the killed scene", || {
        std::iter::from_fn(|| receiver.try_recv().ok()).find_map(|(scene, response)| {
            let SceneResponse::Error(message) = response else {
                return None;
            };
            message.starts_with("scene host exited").then_some(scene)
        })
    });
    assert_eq!(lost, killed);

    let (later, _later_renderer) = start_scene(3, &responses);
    let later_host = eventually("a host for a later scene", || ready_host_of(later));
    assert_ne!(later_host.pid, killed_host.pid);
    assert_eq!(ready_host_of(kept), Some(kept_host));
    assert!(drain_host_exits().is_empty());
}
