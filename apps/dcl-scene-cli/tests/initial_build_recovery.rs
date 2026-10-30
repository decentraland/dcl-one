use dcl_scene_cli::scene::{b64_hash, machine_id};
use futures::StreamExt;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::Duration;
use tokio_tungstenite::tungstenite::Message;

mod common {
    pub mod preview;
    pub mod testgate;
    pub mod wait;
}

const BIN: &str = env!("CARGO_BIN_EXE_dcl-scene-cli");

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

const TSCONFIG: &str = r#"{
  "compilerOptions": { "strict": true, "baseUrl": "." },
  "include": ["src/**/*.ts"],
  "extends": "@dcl/sdk/types/tsconfig.ecs7.json"
}"#;

const BROKEN_MAIN: &str = "export function main() { const x = = 1 }\n";

const FIXED_MAIN: &str = "export function main() {}\nexport const recovered = 1\n";

fn write(path: &Path, contents: &str) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, contents).unwrap();
}

struct ChildGuard(Child);

impl Drop for ChildGuard {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

async fn wait_for_about(base: &str, client: &reqwest::Client) -> Value {
    common::wait::until(Duration::from_secs(60), async || {
        match client.get(format!("{base}/about")).send().await {
            Ok(resp) if resp.status().is_success() => Some(resp.json().await.unwrap()),
            _ => None,
        }
    })
    .await
    .unwrap_or_else(|| panic!("preview server did not come up on {base}"))
}

async fn wait_for_log_marker(log_path: &Path, marker: &str) {
    let read = || std::fs::read_to_string(log_path).unwrap_or_default();
    common::wait::until(Duration::from_secs(10), async || {
        read().contains(marker).then_some(())
    })
    .await
    .unwrap_or_else(|| panic!("log never mentioned {marker:?}:\n{}", read()));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "needs DCL_SCENE_CLI_TEST_NODE_MODULES: type-checks a real scene through the preview server; see docs/testing.md"]
async fn failed_initial_build_keeps_serving_and_recovers_on_fix() {
    let Some(node_modules) = sandbox_node_modules() else {
        return;
    };

    let tmp = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!(
        "dcl-scene-cli-initial-recovery-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).unwrap();
    let root = tmp.join("scene");
    write(
        &root.join("scene.json"),
        &json!({
            "display": { "title": "Recovery" },
            "main": "bin/index.js",
            "runtimeVersion": "7",
            "scene": { "parcels": ["0,0"], "base": "0,0" }
        })
        .to_string(),
    );
    write(&root.join("tsconfig.json"), TSCONFIG);
    write(&root.join("src/index.ts"), BROKEN_MAIN);
    let status = Command::new("cp")
        .arg("-al")
        .arg(&node_modules)
        .arg(root.join("node_modules"))
        .status()
        .expect("cp -al node_modules");
    assert!(status.success(), "hardlinking node_modules");
    let root_str = root.display().to_string();

    let log_path = tmp.join("start.log");
    let log = std::fs::File::create(&log_path).unwrap();
    let child = Command::new(BIN)
        .args([
            "start",
            "--dir",
            &root_str,
            "--port",
            "0",
            "--skip-type-check",
            "--offline-comms",
        ])
        .stdin(Stdio::null())
        .stdout(log.try_clone().unwrap())
        .stderr(log)
        .spawn()
        .unwrap();
    let mut guard = ChildGuard(child);

    let port =
        common::preview::wait_for_port(&log_path, &mut guard.0, Duration::from_secs(60)).await;
    let base = format!("http://127.0.0.1:{port}");
    let client = reqwest::Client::new();
    let about = wait_for_about(&base, &client).await;

    assert!(
        guard.0.try_wait().unwrap().is_none(),
        "start must survive a failed initial build"
    );
    wait_for_log_marker(&log_path, "save any file to retry the initial build").await;
    assert!(
        !root.join("bin/scene.js").exists(),
        "no scene chunk may exist while the initial build is broken"
    );

    let canon = root.canonicalize().unwrap();
    let id = b64_hash(&canon.display().to_string(), &machine_id());
    let scenes_urn: Vec<String> = about["configurations"]["scenesUrn"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap().to_string())
        .collect();
    assert_eq!(scenes_urn.len(), 1, "about: {about}");
    assert!(scenes_urn[0].contains(&id));

    let (ws, _) = tokio_tungstenite::connect_async(format!("ws://127.0.0.1:{port}/"))
        .await
        .expect("scene-update websocket");
    let (_, mut ws_rx) = ws.split();

    // The handshake is the subscription (the preview subscribes before it
    // answers), and the watcher was armed before the retry note printed.
    write(&root.join("src/index.ts"), FIXED_MAIN);

    let update = tokio::time::timeout(Duration::from_secs(60), async {
        while let Some(msg) = ws_rx.next().await {
            if let Message::Text(text) = msg.expect("scene-update websocket frame") {
                let v: Value = serde_json::from_str(&text).unwrap();
                if v["type"] == json!("SCENE_UPDATE") {
                    return Some(v);
                }
            }
        }
        None
    })
    .await
    .ok()
    .flatten()
    .expect("no SCENE_UPDATE after fixing the broken initial build");
    assert_eq!(update["payload"]["sceneId"], json!(id));
    assert!(
        root.join("bin/scene.js").is_file(),
        "recovered build must produce the scene chunk"
    );

    let active: Value = client
        .post(format!("{base}/content/entities/active"))
        .json(&json!({ "pointers": ["0,0"] }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let active = active.as_array().unwrap();
    assert_eq!(active.len(), 1, "entities/active: {active:?}");
    assert_eq!(active[0]["id"], json!(id));
    assert!(
        active[0]["content"]
            .as_array()
            .unwrap()
            .iter()
            .any(|c| c["file"] == json!("bin/scene.js"))
    );

    drop(guard);
    let _ = std::fs::remove_dir_all(&tmp);
}
