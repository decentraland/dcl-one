//! A scene on a sandboxed scene host keeps the network access the sandbox leaves it: fetch and
//! WebSocket. Linux only, where the sandbox exists.
#![cfg(target_os = "linux")]

use bevy::prelude::{Entity, Vec3};
use common::rpc::RpcCall;
use common::structs::SceneJsFile;
use dcl_deno_ipc::{
    init_runtime_with, scene_hosts, spawn_scene, HostOptions, Placement, SandboxMode,
};
use dcl_types::{
    interface::crdt_context::CrdtContext, scene_response_channel, RendererResponse, SceneCensus,
    SceneId, SceneResponse, SceneResponseSender,
};
use std::{
    io::{BufRead, BufReader, Write},
    net::TcpListener,
    sync::Arc,
    time::{Duration, Instant},
};

const TIMEOUT: Duration = Duration::from_secs(60);

// Fetches `%HTTP%` and echoes a message over `%WS%`, then logs the outcome as JSON. Each request
// waits for the engine's permission, which travels with the scene's updates.
const SCENE_JS: &str = r#"
const { crdtSendToRenderer } = require("~system/EngineApi");
let network;
async function probe() {
    const result = {};
    try {
        const response = await fetch("%HTTP%");
        result.fetch = { status: response.status, body: await response.text() };
    } catch (e) {
        result.fetch = { error: String(e) };
    }
    try {
        result.websocket = await new Promise((resolve, reject) => {
            const socket = new WebSocket("%WS%");
            socket.onopen = () => socket.send("ping");
            socket.onmessage = (event) => {
                resolve({ echo: String(event.data) });
                socket.close();
            };
            socket.onerror = () => reject(new Error("websocket failed"));
        });
    } catch (e) {
        result.websocket = { error: String(e) };
    }
    return result;
}
exports.onStart = async () => {
    probe().then((result) => { network = result; });
};
exports.onUpdate = async () => {
    if (network) {
        console.log(JSON.stringify({ network }));
        network = undefined;
    }
    await crdtSendToRenderer({ data: new Uint8Array(0) });
};
"#;

#[derive(serde::Deserialize, Debug, PartialEq)]
struct Logged {
    network: Network,
}

#[derive(serde::Deserialize, Debug, PartialEq)]
struct Network {
    fetch: serde_json::Value,
    websocket: serde_json::Value,
}

// A `console.log` line carries its level and the logged string as JSON.
fn logged(log: &dcl_types::SceneLogMessage) -> Option<Logged> {
    let (_, logged) = log.message.split_once(' ')?;
    let logged = serde_json::from_str::<String>(logged).ok()?;
    serde_json::from_str(&logged).ok()
}

// Answers one HTTP request with `ok`.
fn http_server() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    std::thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        let mut reader = BufReader::new(stream);
        let mut line = String::new();
        while reader.read_line(&mut line).unwrap() > 0 && line != "\r\n" {
            line.clear();
        }
        let response = "HTTP/1.1 200 OK\r\ncontent-length: 2\r\nconnection: close\r\n\r\nok";
        reader.get_mut().write_all(response.as_bytes()).unwrap();
    });
    port
}

// Echoes one WebSocket message.
fn websocket_server() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    std::thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        let mut socket = tungstenite::accept(stream).unwrap();
        let message = socket.read().unwrap();
        socket.send(message).unwrap();
        let _ = socket.close(None);
        while socket.read().is_ok() {}
    });
    port
}

#[test]
fn a_scene_on_a_sandboxed_host_can_still_fetch_and_open_a_websocket() {
    init_runtime_with(HostOptions {
        placement: Placement::PerScene,
        sandbox: SandboxMode::Require,
        ..Default::default()
    })
    .unwrap();
    let scene_js = SCENE_JS
        .replace("%HTTP%", &format!("http://localhost:{}/", http_server()))
        .replace("%WS%", &format!("ws://localhost:{}/", websocket_server()));
    let scene = SceneId(Entity::from_raw(1));
    // a preview scene, which may reach the local network
    let context = CrdtContext::new(
        scene,
        "scene".to_owned(),
        "title".to_owned(),
        false,
        true,
        true,
    );
    let (responses, mut receiver) = scene_response_channel();

    let (renderer, _) = spawn_scene(
        Default::default(),
        context,
        0,
        SceneJsFile(Arc::new(scene_js)),
        Default::default(),
        SceneResponseSender::new(scene, responses),
        tokio::sync::broadcast::channel(16).1,
        String::new(),
        false,
        None,
        Vec3::ZERO,
    );

    // the engine's side: grant each permission and answer each update
    let deadline = Instant::now() + TIMEOUT;
    let logged = loop {
        assert!(Instant::now() < deadline, "the scene logged no result");
        let Ok((_, response)) = receiver.try_recv() else {
            std::thread::sleep(Duration::from_millis(10));
            continue;
        };
        match response {
            SceneResponse::Ok(_, _, _, logs, calls) => {
                for call in calls {
                    if let RpcCall::RequestGenericPermission { response, .. } = call {
                        response.send(true);
                    }
                }
                if let Some(logged) = logs.iter().find_map(logged) {
                    break logged;
                }
                let census = SceneCensus {
                    born: Default::default(),
                    died: Default::default(),
                };
                let update = RendererResponse::Ok(Default::default(), census, 0.0);
                renderer.send(update).unwrap();
            }
            SceneResponse::Error(e) => panic!("the scene failed: {e}"),
            _ => {}
        }
    };

    let sandbox = scene_hosts()
        .into_iter()
        .find(|host| host.scenes == [scene])
        .and_then(|host| host.sandbox)
        .expect("the scene's host reported no sandbox");
    assert!(sandbox.satisfies(SandboxMode::Require), "{sandbox:?}");
    assert_eq!(
        logged.network,
        Network {
            fetch: serde_json::json!({"status": 200, "body": "ok"}),
            websocket: serde_json::json!({"echo": "ping"}),
        }
    );
}
