//! The /scene page's Multiplayer switch, end to end against a running
//! preview: `POST /scene-json` with `authoritativeMultiplayer` writes the
//! flag, the watcher's rebuild re-arms the loader stub and attaches the server
//! isolate, `/scene` reports it running, and switching it off takes the key,
//! the isolate and the loader's arming away again, with no restart between.
mod common {
    pub mod preview;
    pub mod testgate;
    pub mod wait;
}

use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::Duration;

const BIN: &str = env!("CARGO_BIN_EXE_dcl-scene-cli");

struct ChildGuard(Child);

impl Drop for ChildGuard {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

async fn wait_until(what: &str, log: &Path, mut done: impl FnMut() -> bool) {
    common::wait::until(Duration::from_secs(120), async || done().then_some(()))
        .await
        .unwrap_or_else(|| {
            panic!(
                "never saw {what}:\n{}",
                std::fs::read_to_string(log).unwrap_or_default()
            )
        });
}

fn logged(log: &Path, needle: &str) -> bool {
    std::fs::read_to_string(log).is_ok_and(|text| text.contains(needle))
}

fn flag(scene: &Path) -> Option<Value> {
    let json: Value =
        serde_json::from_str(&std::fs::read_to_string(scene.join("scene.json")).unwrap()).unwrap();
    json.get("authoritativeMultiplayer").cloned()
}

fn loader_armed(scene: &Path) -> Option<bool> {
    let stub = std::fs::read_to_string(scene.join("bin/index.js")).ok()?;
    if stub.contains("var __dclOneMp = true") {
        Some(true)
    } else if stub.contains("var __dclOneMp = false") {
        Some(false)
    } else {
        None
    }
}

/// The run's capability token, as the landing page hands it to its own
/// scripts: a same-origin navigation on a loopback host gets the `<meta>`.
async fn page_token(client: &reqwest::Client, base: &str) -> String {
    let html = client
        .get(format!("{base}/"))
        .header("accept", "text/html")
        .header("sec-fetch-site", "none")
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
}

async fn flip(client: &reqwest::Client, base: &str, on: bool) {
    let token = page_token(client, base).await;
    let resp = client
        .post(format!("{base}/scene-json"))
        .header("authorization", format!("Bearer {token}"))
        .json(&json!({ "authoritativeMultiplayer": on }))
        .send()
        .await
        .unwrap();
    assert!(resp.status().is_success(), "{}", resp.status());
}

async fn server_state(client: &reqwest::Client, base: &str) -> String {
    let html = client
        .get(format!("{base}/scene"))
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap();
    let at = html.find("data-server=\"").expect("the Multiplayer pane") + "data-server=\"".len();
    html[at..][..html[at..].find('"').unwrap()].to_string()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_switch_attaches_and_drops_the_server_under_a_running_preview() {
    if dcl_scene_cli::build::find_node().is_none() {
        let _: Option<()> = common::testgate::unavailable(
            "node",
            "install node; the authoritative host runs the scene under it",
        );
        return;
    }
    let tmp = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("dcl-scene-cli-host-toggle-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp);
    let scene = tmp.join("scene");
    let init = Command::new(BIN)
        .args(["init", "--yes", "--dir", &scene.display().to_string()])
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert!(
        init.status.success(),
        "init failed:\n{}",
        String::from_utf8_lossy(&init.stderr)
    );
    assert_eq!(flag(&scene), None, "a fresh scaffold has no server");

    let log = tmp.join("start.log");
    let out = std::fs::File::create(&log).unwrap();
    let child = Command::new(BIN)
        .args([
            "start",
            "--dir",
            &scene.display().to_string(),
            "--port",
            "0",
            "--no-livekit",
            "--no-asset-bundles",
            "--no-mcp",
            "--skip-type-check",
        ])
        .env_remove("RUST_LOG")
        .stdin(Stdio::null())
        .stdout(out.try_clone().unwrap())
        .stderr(out)
        .spawn()
        .unwrap();
    let mut guard = ChildGuard(child);

    let port = common::preview::wait_for_port(&log, &mut guard.0, Duration::from_secs(120)).await;
    let base = format!("http://127.0.0.1:{port}");
    let client = reqwest::Client::new();
    wait_until("the preview", &log, || logged(&log, "Preview server ready")).await;
    assert_eq!(server_state(&client, &base).await, "off");
    assert_eq!(loader_armed(&scene), Some(false));
    assert!(!logged(&log, "authoritative host attached"));

    flip(&client, &base, true).await;
    assert_eq!(flag(&scene), Some(json!(true)));
    wait_until("the host in the room", &log, || {
        logged(&log, "host joined the room")
    })
    .await;
    assert_eq!(
        loader_armed(&scene),
        Some(true),
        "the rebuild re-arms the loader, or clients never trust the server"
    );
    assert_eq!(server_state(&client, &base).await, "running");
    println!("PASS the switch attached the server with no restart");

    flip(&client, &base, false).await;
    assert_eq!(flag(&scene), None, "off is the absent key");
    wait_until("the host leaving", &log, || logged(&log, "host detached")).await;
    assert_eq!(loader_armed(&scene), Some(false));
    assert_eq!(server_state(&client, &base).await, "off");
}
