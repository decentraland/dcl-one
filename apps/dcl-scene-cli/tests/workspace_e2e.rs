use dcl_scene_cli::scene::{b64_hash, machine_id};
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

fn scene_json(title: &str, parcel: &str) -> String {
    json!({
        "display": { "title": title },
        "main": "bin/index.js",
        "runtimeVersion": "7",
        "scene": { "parcels": [parcel], "base": parcel }
    })
    .to_string()
}

const TSCONFIG: &str = r#"{
  "compilerOptions": { "strict": true, "baseUrl": "." },
  "include": ["src/**/*.ts"],
  "extends": "@dcl/sdk/types/tsconfig.ecs7.json"
}"#;

fn write(path: &Path, contents: &str) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, contents).unwrap();
}

fn make_member(root: &Path, name: &str, title: &str, parcel: &str, node_modules: &Path) {
    let member = root.join(name);
    write(&member.join("scene.json"), &scene_json(title, parcel));
    write(&member.join("tsconfig.json"), TSCONFIG);
    write(&member.join("src/index.ts"), "export function main() {}\n");
    let status = Command::new("cp")
        .arg("-al")
        .arg(node_modules)
        .arg(member.join("node_modules"))
        .status()
        .expect("cp -al node_modules");
    assert!(status.success(), "hardlinking node_modules into {name}");
}

/// The scene a `SCENE_UPDATE` text frame names; each one comes right before
/// the binary frame saying what changed.
fn scene_update(msg: Message) -> Option<String> {
    let Message::Text(text) = msg else {
        return None;
    };
    let v: Value = serde_json::from_str(&text).unwrap();
    (v["type"] == json!("SCENE_UPDATE"))
        .then(|| v["payload"]["sceneId"].as_str().unwrap().to_string())
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
    common::wait::until(Duration::from_secs(60), async || {
        match client.get(format!("{base}/about")).send().await {
            Ok(resp) if resp.status().is_success() => Some(resp.json().await.unwrap()),
            _ => None,
        }
    })
    .await
    .unwrap_or_else(|| panic!("preview server did not come up on {base}"))
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "needs DCL_SCENE_CLI_TEST_NODE_MODULES: builds two real workspace members; see docs/testing.md"]
async fn two_member_workspace_builds_serves_and_reloads_per_member() {
    let Some(node_modules) = sandbox_node_modules() else {
        return;
    };

    let root = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("dcl-scene-cli-ws-e2e-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    write(
        &root.join("dcl-workspace.json"),
        r#"{"folders":[{"path":"scene-a"},{"path":"scene-b"}]}"#,
    );
    make_member(&root, "scene-a", "WS A", "0,0", &node_modules);
    make_member(&root, "scene-b", "WS B", "1,0", &node_modules);
    let root_str = root.display().to_string();

    let out = Command::new(BIN)
        .args(["build", "--dir", &root_str, "--skip-type-check"])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        out.status.success(),
        "workspace build failed\nstdout: {stdout}\nstderr: {stderr}"
    );
    assert!(stdout.contains("[1/2] in scene-a:"), "stdout: {stdout}");
    assert!(stdout.contains("[2/2] in scene-b:"), "stdout: {stdout}");
    let a_bin = root.join("scene-a/bin/index.js");
    let b_bin = root.join("scene-b/bin/index.js");
    assert!(a_bin.is_file());
    assert!(b_bin.is_file());

    let canon = root.canonicalize().unwrap();
    let machine = machine_id();
    let id_a = b64_hash(&canon.join("scene-a").display().to_string(), &machine);
    let id_b = b64_hash(&canon.join("scene-b").display().to_string(), &machine);

    let log_path = root.join("start.log");
    let log = std::fs::File::create(&log_path).unwrap();
    let child = Command::new(BIN)
        .args([
            "start",
            "--dir",
            &root_str,
            "--port",
            "0",
            "--skip-build",
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

    let scenes_urn: Vec<String> = about["configurations"]["scenesUrn"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap().to_string())
        .collect();
    assert_eq!(scenes_urn.len(), 2, "about: {about}");
    assert!(scenes_urn[0].contains(&id_a));
    assert!(scenes_urn[1].contains(&id_b));
    let parcels: Vec<String> = about["configurations"]["localSceneParcels"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap().to_string())
        .collect();
    assert!(parcels.contains(&"0,0".to_string()));
    assert!(parcels.contains(&"1,0".to_string()));

    let both: Value = client
        .post(format!("{base}/content/entities/active"))
        .json(&json!({ "pointers": ["0,0", "1,0"] }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let both = both.as_array().unwrap();
    assert_eq!(both.len(), 2, "entities/active union: {both:?}");
    assert_eq!(both[0]["id"], json!(id_a));
    assert_eq!(both[1]["id"], json!(id_b));
    assert!(
        both[1]["content"]
            .as_array()
            .unwrap()
            .iter()
            .any(|c| c["file"] == json!("bin/index.js"))
    );

    let only_b: Value = client
        .post(format!("{base}/content/entities/active"))
        .json(&json!({ "pointers": ["1,0"] }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let only_b = only_b.as_array().unwrap();
    assert_eq!(only_b.len(), 1);
    assert_eq!(only_b[0]["id"], json!(id_b));

    let only_a: Value = client
        .get(format!("{base}/content/entities/scene?pointer=0,0"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let only_a = only_a.as_array().unwrap();
    assert_eq!(only_a.len(), 1);
    assert_eq!(only_a[0]["id"], json!(id_a));

    let entity_b: Value = client
        .get(format!("{base}/content/contents/{id_b}"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(entity_b["id"], json!(id_b));
    assert_eq!(entity_b["pointers"], json!(["1,0"]));
    let b_bundle_hash = entity_b["content"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["file"] == json!("bin/index.js"))
        .unwrap()["hash"]
        .as_str()
        .unwrap()
        .to_string();
    let served = client
        .get(format!("{base}/content/contents/{b_bundle_hash}"))
        .send()
        .await
        .unwrap();
    assert!(served.status().is_success());
    let served_bytes = served.bytes().await.unwrap();
    assert_eq!(served_bytes.as_ref(), std::fs::read(&b_bin).unwrap());

    let (ws, _) = tokio_tungstenite::connect_async(format!("ws://127.0.0.1:{port}/"))
        .await
        .expect("scene-update websocket");
    let (_, mut ws_rx) = ws.split();

    let a_chunk = root.join("scene-a/bin/scene.js");
    let b_chunk = root.join("scene-b/bin/scene.js");
    assert!(a_chunk.is_file(), "scene-a has no bin/scene.js");
    assert!(b_chunk.is_file(), "scene-b has no bin/scene.js");
    let a_before = mtime(&a_chunk);
    let b_before = mtime(&b_chunk);
    // The handshake is the subscription (the preview subscribes before it
    // answers), and every member's watcher is armed before the server serves.
    let b_src = root.join("scene-b/src/index.ts");
    std::fs::write(
        &b_src,
        "export function main() {}\nexport const workspaceEdit = 2\n",
    )
    .unwrap();

    let first = tokio::time::timeout(Duration::from_secs(45), async {
        while let Some(msg) = ws_rx.next().await {
            if let Some(scene) = scene_update(msg.expect("scene-update websocket frame")) {
                return Some(scene);
            }
        }
        None
    })
    .await
    .ok()
    .flatten();
    assert_eq!(
        first.as_deref(),
        Some(id_b.as_str()),
        "expected the first SCENE_UPDATE to be scene-b's"
    );
    let mut scene_of_frame = first;

    // scene-a's watcher handles its batches in order, so its answer to a model
    // dropped in after scene-b rebuilt comes after anything the scene-b edit
    // made it do. A model save reloads the model alone, never the scene.
    std::fs::write(root.join("scene-a/settled.glb"), b"glTF").unwrap();
    let a_rebuilt = dcl_scene_cli::live_reload::update_scene_frame(&id_a);
    tokio::time::timeout(Duration::from_secs(45), async {
        while let Some(msg) = ws_rx.next().await {
            match msg.expect("scene-update websocket frame") {
                Message::Binary(frame) if scene_of_frame.as_deref() == Some(id_a.as_str()) => {
                    assert_ne!(
                        frame.as_ref(),
                        a_rebuilt.as_slice(),
                        "scene-a must not rebuild on a scene-b edit"
                    );
                    return;
                }
                msg => {
                    if let Some(scene) = scene_update(msg) {
                        scene_of_frame = Some(scene);
                    }
                }
            }
        }
        panic!("the scene-update websocket closed");
    })
    .await
    .expect("scene-a never reported the model dropped into it");

    assert!(mtime(&b_chunk) > b_before, "scene-b bundle must be rebuilt");
    assert_eq!(
        mtime(&a_chunk),
        a_before,
        "scene-a bundle must stay untouched"
    );

    drop(guard);
    let _ = std::fs::remove_dir_all(&root);
}
