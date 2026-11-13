//! `deploy --programmatic` end to end: the binary's two stdout lines and the
//! `/api/deploy` resource a program drives between them, against a target
//! that never answers so nothing can be published.

use serde_json::Value;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::time::Duration;

mod common {
    pub mod child;
    pub mod wait;
}
use common::child::{Tail, terminate, wait_bounded};
use common::wait::HANG_GUARD;

const BIN: &str = env!("CARGO_BIN_EXE_dcl-scene-cli");
const TOKEN: &str = "0123456789abcdef";
const SCENE: &str = r#"{"main":"bin/index.js","runtimeVersion":"7","scene":{"parcels":["0,0"],"base":"0,0"},"display":{"title":"Gather","description":"a square"}}"#;
/// Nothing listens here, so a signed upload can only fail and a wait for a
/// signature can only be cancelled.
const DEAD_TARGET: &str = "http://127.0.0.1:9";

struct Fixture(PathBuf);

impl Fixture {
    fn new(tag: &str) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "dcl-scene-cli-deploy-api-{tag}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("bin")).unwrap();
        std::fs::write(dir.join("scene.json"), SCENE).unwrap();
        std::fs::write(dir.join("bin/index.js"), "module.exports = {}\n").unwrap();
        Fixture(dir)
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// The child with both streams tailed; the linker wait is only a hang guard.
struct Run {
    child: Child,
    out: Tail,
    err: Tail,
}

fn spawn(dir: &Fixture, extra: &[&str], envs: &[(&str, &str)]) -> Run {
    spawn_as(&["--programmatic"], dir, extra, envs)
}

/// `mode` is the output contract under test: `--programmatic`'s two lines,
/// or `--json`'s single document.
fn spawn_as(mode: &[&str], dir: &Fixture, extra: &[&str], envs: &[(&str, &str)]) -> Run {
    let mut cmd = Command::new(BIN);
    cmd.args(["deploy", "--dir"])
        .arg(&dir.0)
        .args(["--skip-build", "--target-content", DEAD_TARGET])
        .args(mode)
        .args(extra);
    for k in ["DCL_PRIVATE_KEY", "RUST_LOG", "DCL_SCENE_CLI_TARGET_SERVER"] {
        cmd.env_remove(k);
    }
    cmd.env("NO_COLOR", "1")
        .env("DCL_SCENE_CLI_LINKER_TIMEOUT_SECS", "25")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    for (k, v) in envs {
        cmd.env(k, v);
    }
    let mut child = cmd.spawn().unwrap();
    let out = Tail::spawn(child.stdout.take().unwrap());
    let err = Tail::spawn(child.stderr.take().unwrap());
    Run { child, out, err }
}

impl Drop for Run {
    fn drop(&mut self) {
        if self.child.try_wait().ok().flatten().is_none() {
            terminate(&mut self.child);
        }
    }
}

impl Run {
    /// Stops the child and fails with everything it said.
    fn fail(&mut self, why: impl std::fmt::Display) -> ! {
        let status = wait_bounded(&mut self.child, Duration::from_millis(50))
            .map(|s| s.to_string())
            .unwrap_or_else(|| "still running".to_string());
        if status == "still running" {
            terminate(&mut self.child);
        }
        panic!(
            "{why}\nchild: {status}\nstdout: {}\nstderr: {}",
            self.out.text(),
            self.err.text()
        );
    }

    /// Sends a request to the child's server; a refused connection is the
    /// child's failure, reported with its streams.
    async fn send(&mut self, req: reqwest::RequestBuilder) -> reqwest::Response {
        match req.send().await {
            Ok(resp) => resp,
            Err(e) => self.fail(format!("request failed: {e}")),
        }
    }

    /// Blocks until stdout carries a whole line holding `needle`, and parses it.
    fn line_with(&self, needle: &str) -> Value {
        assert!(
            self.out.wait_for(needle, HANG_GUARD),
            "stdout never carried {needle}: {}\nstderr: {}",
            self.out.text(),
            self.err.text()
        );
        let text = self.out.text();
        let line = text
            .lines()
            .find(|l| l.contains(needle))
            .unwrap_or_else(|| panic!("no line with {needle} in {text}"));
        serde_json::from_str(line).unwrap_or_else(|e| panic!("{line}: {e}"))
    }

    /// The exit and both streams; the child is stopped when it outlives the guard.
    fn finish(mut self, guard: Duration) -> (bool, String, String) {
        let status = wait_bounded(&mut self.child, guard).unwrap_or_else(|| {
            let (out, err) = (self.out.text(), self.err.text());
            terminate(&mut self.child);
            panic!("the deploy outlived {guard:?}\nstdout: {out}\nstderr: {err}");
        });
        (
            status.success(),
            String::from_utf8_lossy(&self.out.finish()).into_owned(),
            String::from_utf8_lossy(&self.err.finish()).into_owned(),
        )
    }
}

fn client() -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(HANG_GUARD)
        .build()
        .unwrap()
}

impl Run {
    /// Sends a request and reads its JSON answer, status first.
    async fn json(&mut self, req: reqwest::RequestBuilder) -> (u16, Value) {
        let resp = self.send(req).await;
        let status = resp.status().as_u16();
        let body = resp.text().await.unwrap();
        (status, serde_json::from_str(&body).unwrap_or(Value::Null))
    }
}

/// Every chunk of a server-sent stream until it ends, or `guard` passes.
async fn drain_sse(resp: reqwest::Response, guard: Duration) -> (String, bool) {
    let mut resp = resp;
    let mut text = String::new();
    let ended = tokio::time::timeout(guard, async {
        while let Some(chunk) = resp.chunk().await.unwrap() {
            text.push_str(&String::from_utf8_lossy(&chunk));
        }
    })
    .await
    .is_ok();
    (text, ended)
}

#[tokio::test(flavor = "multi_thread")]
async fn a_program_reads_the_ready_line_drives_the_resource_and_cancels() {
    let f = Fixture::new("drive");
    let mut run = spawn(&f, &["--deploy-token", TOKEN], &[]);
    let ready = run.line_with(r#""event":"ready""#);
    let api = ready["api"].as_str().unwrap().to_string();
    let entity_id = ready["entityId"].as_str().unwrap().to_string();
    assert_eq!(ready["deployToken"], TOKEN);
    assert!(entity_id.starts_with("bafkrei"), "{ready}");
    assert!(
        ready["url"]
            .as_str()
            .unwrap()
            .ends_with(&format!("/deploy#token={TOKEN}")),
        "{ready}"
    );
    assert!(api.ends_with("/api/deploy"), "{ready}");
    assert!(ready["deleteScenesFromWorldPayload"].is_null());
    let base = api.trim_end_matches("/api/deploy").to_string();
    let http = client();

    let (status, state) = run.json(http.get(&api)).await;
    assert_eq!(status, 200);
    assert_eq!(state["phase"], "awaiting-signature");
    assert_eq!(state["cancelRequested"], false);

    let (status, info) = run.json(http.get(format!("{base}/api/info"))).await;
    assert_eq!(status, 200);
    assert_eq!(
        info["rootCID"], entity_id,
        "the id to sign is the one minted"
    );
    assert_eq!(info["isWorld"], false);
    assert_eq!(info["title"], "Gather");
    assert_eq!(info["description"], "a square");
    assert_eq!(info["targetContent"], DEAD_TARGET);
    assert_eq!(info["parcels"], serde_json::json!(["0,0"]));

    let (status, files) = run.json(http.get(format!("{base}/api/files"))).await;
    assert_eq!(status, 200);
    let names: Vec<&str> = files
        .as_array()
        .unwrap()
        .iter()
        .map(|f| f["name"].as_str().unwrap())
        .collect();
    assert!(
        names.contains(&"scene.json") && names.contains(&"bin/index.js"),
        "{files}"
    );

    let (status, body) = run
        .json(
            http.post(&api)
                .header("content-type", "application/json")
                .body("not json"),
        )
        .await;
    assert_eq!(status, 400);
    assert_eq!(body["message"], "Invalid payload: the body must be JSON");
    let (status, body) = run
        .json(
            http.post(&api)
                .json(&serde_json::json!({ "address": "0x1" })),
        )
        .await;
    assert_eq!(status, 400);
    assert_eq!(body["message"], "Invalid payload: address");

    let (status, body) = run.json(http.delete(&api)).await;
    assert_eq!(status, 403);
    assert_eq!(body["message"], "Invalid or missing deploy token");
    let (status, _) = run
        .json(
            http.delete(&api)
                .header("x-deploy-token", "0123456789abcdeF"),
        )
        .await;
    assert_eq!(status, 403);
    let (status, state) = run.json(http.get(&api)).await;
    assert_eq!(
        (status, state["phase"].as_str()),
        (200, Some("awaiting-signature"))
    );

    let stream = run
        .send(http.get(&api).header("accept", "text/event-stream"))
        .await;
    assert_eq!(stream.status(), 200);
    assert!(
        stream.headers()["content-type"]
            .to_str()
            .unwrap()
            .starts_with("text/event-stream")
    );
    let (status, state) = run
        .json(http.delete(&api).header("x-deploy-token", TOKEN))
        .await;
    assert_eq!(status, 202, "{state}");
    assert_eq!(state["phase"], "cancelled");
    assert_eq!(state["cancelRequested"], true);
    assert!(
        state["error"].is_null(),
        "a cancel before the signature is no error"
    );
    let (events, ended) = drain_sse(stream, HANG_GUARD).await;
    assert!(ended, "the stream ends after the final state: {events}");
    let states: Vec<Value> = events
        .lines()
        .filter_map(|l| l.strip_prefix("data: "))
        .map(|d| serde_json::from_str(d).unwrap())
        .collect();
    assert_eq!(
        states.first().unwrap()["phase"],
        "awaiting-signature",
        "{events}"
    );
    assert_eq!(states.last().unwrap()["phase"], "cancelled", "{events}");
    assert!(events.contains("event: state"), "{events}");

    let (ok, stdout, stderr) = run.finish(Duration::from_secs(15));
    assert!(!ok, "a cancelled run fails: {stdout}");
    let last: Value = serde_json::from_str(stdout.lines().last().unwrap()).unwrap();
    assert_eq!(last["event"], "state");
    assert_eq!(last["phase"], "cancelled");
    assert_eq!(
        stdout.lines().count(),
        2,
        "ready and the final state only: {stdout}"
    );
    assert!(stderr.contains("Deployment cancelled"), "{stderr}");
    assert!(
        http.get(&api).send().await.is_err(),
        "the server is gone with the run"
    );
}

/// `--json` without `--programmatic`: stdout is the result document alone,
/// so the signing prompt goes to stderr, and the run ends with the signing
/// outcome instead of keeping the preview up for a Ctrl+C nobody sends.
#[tokio::test(flavor = "multi_thread")]
async fn a_json_run_prompts_on_stderr_and_ends_with_the_outcome() {
    let f = Fixture::new("json-prompt");
    let mut run = spawn_as(&["--json"], &f, &["--deploy-token", TOKEN], &[]);
    assert!(
        run.err.wait_for("/deploy#token=", HANG_GUARD),
        "stderr never carried the signing url: {}\nstdout: {}",
        run.err.text(),
        run.out.text()
    );
    let err = run.err.text();
    let url = err
        .lines()
        .map(str::trim)
        .find(|l| l.starts_with("http://") && l.contains("/deploy#token="))
        .unwrap_or_else(|| panic!("no signing url line in {err}"))
        .to_string();
    assert!(
        run.out.text().is_empty(),
        "stdout stays clear for the document: {}",
        run.out.text()
    );
    let api = format!("{}/api/deploy", url.split("/deploy#").next().unwrap());
    let http = client();
    let (status, state) = run
        .json(http.delete(&api).header("x-deploy-token", TOKEN))
        .await;
    assert_eq!(status, 202, "{state}");
    assert_eq!(state["phase"], "cancelled");
    let (ok, stdout, stderr) = run.finish(Duration::from_secs(15));
    assert!(!ok, "a cancelled run fails: {stdout}");
    assert!(
        stdout.trim().is_empty(),
        "no document for a deployment that did not happen: {stdout}"
    );
    assert!(stderr.contains("Sign the deployment"), "{stderr}");
    assert!(stderr.contains("cancelled"), "{stderr}");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_short_token_is_refused_before_any_server_starts() {
    let f = Fixture::new("short");
    let run = spawn(&f, &["--deploy-token", "short"], &[]);
    let (ok, stdout, stderr) = run.finish(Duration::from_secs(15));
    assert!(!ok);
    assert!(stderr.contains("at least 16 characters"), "{stderr}");
    let last: Value = serde_json::from_str(stdout.lines().last().unwrap()).unwrap();
    assert_eq!(last["phase"], "failed", "{stdout}");
    assert!(!stdout.contains(r#""event":"ready""#), "{stdout}");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_headless_run_prints_only_the_final_state() {
    let f = Fixture::new("headless");
    let key = "0x59c6995e998f97a5a0044966f0945389dc9e86dae88c7a8412f4603b6b78690d";
    let run = spawn(&f, &[], &[("DCL_PRIVATE_KEY", key)]);
    let (ok, stdout, stderr) = run.finish(HANG_GUARD);
    assert!(!ok);
    assert_eq!(stdout.lines().count(), 1, "{stdout}");
    let last: Value = serde_json::from_str(stdout.trim()).unwrap();
    assert_eq!(last["event"], "state");
    assert_eq!(last["phase"], "failed");
    assert_eq!(last["error"]["code"], "DEPLOY_FAILED");
    assert!(
        last["error"]["message"]
            .as_str()
            .unwrap()
            .contains("could not reach the content server"),
        "{last}"
    );
    assert!(
        stderr.contains("could not reach the content server"),
        "{stderr}"
    );
}
