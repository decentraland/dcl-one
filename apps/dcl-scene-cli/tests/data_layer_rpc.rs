use futures::StreamExt;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, SystemTime};
use tokio_tungstenite::tungstenite::Message;

mod common {
    pub mod preview;
    pub mod testgate;
    pub mod wait;
}

const BIN: &str = env!("CARGO_BIN_EXE_dcl-scene-cli");

const CLIENT_SCRIPT: &str = r#"
import { createRequire } from 'node:module'
import path from 'node:path'

const projectDir = path.resolve(process.argv[2])
const wsUrl = process.argv[3]
const req = createRequire(path.join(projectDir, 'package.json'))

const { createRpcClient } = req('@dcl/rpc')
const codegen = req('@dcl/rpc/dist/codegen')
const { WebSocketTransport } = req('@dcl/rpc/dist/transports/WebSocket')
const WebSocket = req('ws')
const { DataServiceDefinition } = req('@dcl/inspector')

const ws = new WebSocket(wsUrl)
ws.binaryType = 'arraybuffer'
const transport = WebSocketTransport(ws)
const client = await createRpcClient(transport)
const port = await client.createPort('scene-ctx')
const service = codegen.loadService(port, DataServiceDefinition)

const prefs = await service.getInspectorPreferences({})
console.log('PREFS', JSON.stringify(prefs))

function crdtPutTransform(entityId, timestamp, x) {
  const data = Buffer.alloc(44)
  data.writeFloatLE(x, 0)
  data.writeFloatLE(1.0, 4)
  data.writeFloatLE(8.0, 8)
  data.writeFloatLE(0, 12)
  data.writeFloatLE(0, 16)
  data.writeFloatLE(0, 20)
  data.writeFloatLE(1.0, 24)
  data.writeFloatLE(1.0, 28)
  data.writeFloatLE(1.0, 32)
  data.writeFloatLE(1.0, 36)
  data.writeUInt32LE(0, 40)
  const msg = Buffer.alloc(24 + data.length)
  msg.writeUInt32LE(24 + data.length, 0)
  msg.writeUInt32LE(1, 4)
  msg.writeUInt32LE(entityId, 8)
  msg.writeUInt32LE(1, 12)
  msg.writeUInt32LE(timestamp, 16)
  msg.writeUInt32LE(data.length, 20)
  data.copy(msg, 24)
  return msg
}

const outgoing = []
let push
const queue = (m) => {
  outgoing.push(m)
  if (push) push()
}
async function* stream() {
  while (true) {
    while (outgoing.length) yield { data: outgoing.shift() }
    await new Promise((r) => (push = r))
  }
}

const incoming = service.crdtStream(stream())
const first = await incoming.next()
// One string argument on purpose: a second, numeric argument goes through
// util.inspect, which wraps it in ANSI colour codes whenever FORCE_COLOR is
// set in the environment — and then the Rust side parses `\e[33m202\e[39m`.
console.log('INITIAL_STATE_BYTES ' + (first.value?.data?.length ?? 0))

// A second stream on the same engine: the host relays to it what the first
// one sent, once the engine has taken it in, so its arrival there says the
// save below will see it. Its first message is its initial state, which says
// it is listening.
const observer = service.crdtStream((async function* () {
  await new Promise(() => {})
})())
await observer.next()

const put = Buffer.from(crdtPutTransform(515, 1, 7.5))
const guard = setTimeout(() => {
  console.error('the host never relayed the edit to a second stream')
  process.exit(3)
}, 60000)
queue(new Uint8Array(put))
for (;;) {
  const relayed = await observer.next()
  if (relayed.done) throw new Error('the second stream ended')
  if (Buffer.from(relayed.value.data ?? []).indexOf(put) >= 0) break
}
clearTimeout(guard)

await service.save({})
console.log('SAVED')
process.exit(0)
"#;

fn sandbox_node_modules() -> Option<PathBuf> {
    match std::env::var_os("DCL_SCENE_CLI_TEST_NODE_MODULES")
        .map(PathBuf::from)
        .filter(|p| p.is_dir())
    {
        Some(p) => Some(p),
        None => common::testgate::unavailable(
            "DCL_SCENE_CLI_TEST_NODE_MODULES",
            "point it at a scene node_modules dir on the same filesystem",
        ),
    }
}

fn write(path: &Path, contents: &str) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, contents).unwrap();
}

fn mtime(path: &Path) -> SystemTime {
    std::fs::metadata(path).unwrap().modified().unwrap()
}

struct ChildGuard(Child);

impl Drop for ChildGuard {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

async fn wait_for_about(base: &str, client: &reqwest::Client) -> Value {
    common::wait::until(Duration::from_secs(120), async || {
        match client.get(format!("{base}/about")).send().await {
            Ok(resp) if resp.status().is_success() => Some(resp.json().await.unwrap()),
            _ => None,
        }
    })
    .await
    .unwrap_or_else(|| panic!("preview server did not come up on {base}"))
}

async fn wait_for_file(path: &Path, guard: Duration) {
    common::wait::until(guard, async || path.is_file().then_some(()))
        .await
        .unwrap_or_else(|| panic!("{} did not appear", path.display()));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "needs DCL_SCENE_CLI_TEST_NODE_MODULES + node: drives a real @dcl/rpc client against the data layer; see docs/testing.md"]
async fn data_layer_rpc_edit_saves_composite_and_reloads() {
    let Some(node_modules) = sandbox_node_modules() else {
        return;
    };
    let Some(node) = dcl_scene_cli::build::find_node().or_else(|| {
        common::testgate::unavailable(
            "node on PATH",
            "the data-layer e2e drives a real node process",
        )
    }) else {
        return;
    };

    let root = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("dcl-scene-cli-dl-e2e-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let scene = root.join("scene");
    write(
        &scene.join("scene.json"),
        &json!({
            "display": { "title": "DL E2E" },
            "main": "bin/index.js",
            "runtimeVersion": "7",
            "scene": { "parcels": ["0,0"], "base": "0,0" }
        })
        .to_string(),
    );
    write(
        &scene.join("tsconfig.json"),
        "{\n  \"compilerOptions\": { \"strict\": true },\n  \"include\": [\"src/**/*.ts\"],\n  \"extends\": \"@dcl/sdk/types/tsconfig.ecs7.json\"\n}",
    );
    write(&scene.join("src/index.ts"), "export function main() {}\n");
    write(&scene.join("package.json"), "{\"name\":\"dl-e2e\"}");
    let status = Command::new("cp")
        .arg("-al")
        .arg(&node_modules)
        .arg(scene.join("node_modules"))
        .status()
        .expect("cp -al node_modules");
    assert!(status.success(), "hardlinking node_modules");
    write(&root.join("client.mjs"), CLIENT_SCRIPT);

    let log_path = root.join("start.log");
    let log = std::fs::File::create(&log_path).unwrap();
    let child = Command::new(BIN)
        .args([
            "start",
            "--dir",
            &scene.display().to_string(),
            "--port",
            "0",
            "--data-layer",
            "--offline-comms",
        ])
        .stdin(Stdio::null())
        .stdout(log.try_clone().unwrap())
        .stderr(log)
        .spawn()
        .unwrap();
    let mut guard = ChildGuard(child);

    let port =
        common::preview::wait_for_port(&log_path, &mut guard.0, Duration::from_secs(120)).await;
    let base = format!("http://127.0.0.1:{port}");
    let client = reqwest::Client::new();
    wait_for_about(&base, &client).await;
    eprintln!("phase: server up on {base}");
    // The data-layer upgrade wants the run's capability token; the landing
    // page hands it to a same-origin navigation on a loopback host.
    let token = {
        let html = client
            .get(format!("{base}/"))
            .header("accept", "text/html")
            .header("sec-fetch-mode", "navigate")
            .header("sec-fetch-dest", "document")
            .send()
            .await
            .unwrap()
            .text()
            .await
            .unwrap();
        let marker = "<meta name=\"dcl-scene-cli-token\" content=\"";
        let at = html
            .find(marker)
            .expect("the landing page carries the token")
            + marker.len();
        html[at..][..html[at..].find('"').unwrap()].to_string()
    };

    let has_ui = scene
        .join("node_modules/@dcl/inspector/public/index.html")
        .is_file();
    let index = client
        .get(format!("{base}/inspector/"))
        .send()
        .await
        .unwrap();
    if has_ui {
        assert!(index.status().is_success());
        let html = index.text().await.unwrap();
        assert!(
            html.contains(&format!(
                "const config = '{{\"dataLayerRpcWsUrl\":\"ws://127.0.0.1:{port}/data-layer?token={token}\"}}'"
            )),
            "config not injected: {html}"
        );
        let bundle = client
            .get(format!("{base}/inspector/bundle.js"))
            .send()
            .await
            .unwrap();
        assert!(bundle.status().is_success());
        assert_eq!(
            bundle.headers()["content-type"].to_str().unwrap(),
            "application/javascript"
        );
    } else {
        assert_eq!(index.status(), reqwest::StatusCode::NOT_FOUND);
        let body = index.text().await.unwrap();
        assert!(body.contains("@dcl/inspector"), "unhelpful 404: {body}");
    }
    eprintln!(
        "phase: inspector UI {}",
        if has_ui { "served" } else { "absent" }
    );

    let composite = scene.join("assets/scene/main.composite");
    wait_for_file(&composite, Duration::from_secs(60)).await;
    eprintln!("phase: data-layer boot wrote {}", composite.display());
    let composite_before = std::fs::read_to_string(&composite).unwrap();
    assert!(!composite_before.contains("7.5"));

    let (ws, _) = tokio_tungstenite::connect_async(format!("ws://127.0.0.1:{port}/"))
        .await
        .expect("scene-update websocket");
    let (_, mut ws_rx) = ws.split();

    let out = Command::new(&node)
        .arg(root.join("client.mjs"))
        .arg(&scene)
        .arg(format!("ws://127.0.0.1:{port}/data-layer?token={token}"))
        .output()
        .expect("node rpc client");
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        out.status.success(),
        "rpc client failed\nstdout: {stdout}\nstderr: {stderr}"
    );
    assert!(stdout.contains("PREFS {"), "stdout: {stdout}");
    assert!(stdout.contains("SAVED"), "stdout: {stdout}");
    let state_bytes: u64 = stdout
        .lines()
        .find_map(|l| l.strip_prefix("INITIAL_STATE_BYTES "))
        .and_then(|v| v.trim().parse().ok())
        .unwrap_or(0);
    assert!(state_bytes > 0, "no initial crdt state\nstdout: {stdout}");
    eprintln!("phase: rpc client done (initial state {state_bytes} bytes)");

    let got_update = tokio::time::timeout(Duration::from_secs(60), async {
        while let Some(msg) = ws_rx.next().await {
            if let Message::Text(text) = msg.expect("scene-update websocket frame") {
                let v: Value = serde_json::from_str(&text).unwrap();
                if v["type"] == json!("SCENE_UPDATE") {
                    return true;
                }
            }
        }
        false
    })
    .await
    .unwrap_or(false);
    assert!(got_update, "no SCENE_UPDATE after the editor save");
    eprintln!("phase: SCENE_UPDATE received");

    let composite_after = std::fs::read_to_string(&composite).unwrap();
    assert_ne!(composite_before, composite_after);
    assert!(
        composite_after.contains("7.5"),
        "saved composite misses the injected transform"
    );

    let crdt = scene.join("main.crdt");
    common::wait::until(Duration::from_secs(60), async || {
        (crdt.is_file() && mtime(&crdt) >= mtime(&composite)).then_some(())
    })
    .await
    .expect("main.crdt was not regenerated after the composite save");
    assert!(std::fs::metadata(&crdt).unwrap().len() > 0);
    eprintln!(
        "phase: main.crdt regenerated ({} bytes)",
        std::fs::metadata(&crdt).unwrap().len()
    );

    drop(guard);
    let _ = std::fs::remove_dir_all(&root);
}
