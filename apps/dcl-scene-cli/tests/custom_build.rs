#![cfg(unix)]

mod common {
    pub mod wait;
}

use dcl_scene_cli::{
    build::{self, BuildOptions},
    build_script, deploy,
    live_reload::ReloadEvent,
    scene::Project,
    ux,
    watch::{FsWatcher, WatchSession},
};
use serde_json::json;
use std::{
    path::PathBuf,
    process::Command,
    sync::atomic::{AtomicUsize, Ordering},
    time::Duration,
};

static NEXT: AtomicUsize = AtomicUsize::new(0);
struct Scene(PathBuf);
impl Scene {
    fn new(script: &str) -> Self {
        let root = std::env::temp_dir().join(format!(
            "sdk-custom-build-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&root).unwrap();
        let scene = Self(root);
        scene.write("scene.json", &json!({"runtimeVersion":"7", "main":"bin/index.js", "scene":{"parcels":["0,0"], "base":"0,0"}}).to_string());
        scene.script(script);
        scene
    }
    fn write(&self, name: &str, text: &str) {
        let p = self.0.join(name);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, text).unwrap();
    }
    fn script(&self, script: &str) {
        self.write(
            "package.json",
            &json!({"scripts":{"build":script}}).to_string(),
        );
    }
    fn opts(&self) -> BuildOptions {
        BuildOptions {
            dir: self.0.clone(),
            built_in: false,
            production: false,
            ignore_composite: false,
            custom_entry_point: false,
            skip_type_check: false,
            out_root: None,
            quiet: true,
        }
    }
    fn project(&self) -> Project {
        Project::load(&self.0).unwrap()
    }
    /// Saves a model, which the watcher answers with that model's reload
    /// alone, after everything it was handed before; see [`next_event`].
    fn mark(&self, name: &str) {
        self.write(name, "glTF");
    }
    fn cli(&self, args: &[&str]) -> std::process::Output {
        Command::new(env!("CARGO_BIN_EXE_dcl-scene-cli"))
            .args(args)
            .arg("--dir")
            .arg(&self.0)
            .output()
            .unwrap()
    }
}
impl Drop for Scene {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[tokio::test]
async fn script_owns_output_checks_and_assets_without_sdk_toolchain() {
    let s = Scene::new("custom-tool");
    // Written the way the SDK writes what it runs, so a child another test
    // forks meanwhile cannot hold it open for writing (ETXTBSY at its exec).
    let tool = s.0.join("node_modules/.bin/custom-tool");
    std::fs::create_dir_all(tool.parent().unwrap()).unwrap();
    dcl_scene_cli::exec_file::write_new(
        &tool,
        b"#!/bin/sh\nmkdir -p bin\nprintf '%s:%s' \"$DCL_SCENE_CLI_PRODUCTION\" \"$DCL_SCENE_CLI_MAIN\" > bin/index.js\nprintf 'custom crdt' > main.crdt\n",
        true,
    )
    .unwrap();
    let mut opts = s.opts();
    opts.production = true;
    opts.out_root = Some(s.0.join(build::RELEASE_OUT));
    let built = build::build(&opts).await.unwrap();
    assert_eq!(built.outfile, s.0.join("bin/index.js"));
    assert_eq!(
        std::fs::read_to_string(&built.outfile).unwrap(),
        "1:bin/index.js"
    );
    assert_eq!(
        std::fs::read_to_string(s.0.join("main.crdt")).unwrap(),
        "custom crdt"
    );
    assert!(
        !s.0.join(".dcl-one").exists(),
        "no SDK staging or release output"
    );
}

#[tokio::test]
async fn script_is_told_when_the_caller_narrates_the_build() {
    let s = Scene::new("mkdir -p bin && printf '%s' \"$DCL_SCENE_CLI_QUIET\" > bin/index.js");
    let mut opts = s.opts();
    for (quiet, expected) in [(true, "1"), (false, "0")] {
        opts.quiet = quiet;
        let built = build::build(&opts).await.unwrap();
        assert_eq!(std::fs::read_to_string(&built.outfile).unwrap(), expected);
    }
    let built = build::build(&s.opts()).await.unwrap();
    let summary = build_script::summary(&built, Duration::from_millis(1100), "");
    assert_eq!(summary, "bin/index.js (1b, 1.10 sec)");
}

#[tokio::test]
async fn failure_and_missing_output_never_fall_back() {
    let s = Scene::new("exit 17");
    s.write("bin/index.js", "existing custom bundle");
    let error = build::build(&s.opts()).await.err().unwrap().to_string();
    assert!(error.contains("17"), "{error}");
    assert_eq!(
        std::fs::read_to_string(s.0.join("bin/index.js")).unwrap(),
        "existing custom bundle"
    );
    std::fs::remove_file(s.0.join("bin/index.js")).unwrap();
    s.script("true");
    assert!(
        build::build(&s.opts())
            .await
            .err()
            .unwrap()
            .to_string()
            .contains("did not produce")
    );
    s.script("mkdir -p bin; : > bin/index.js");
    assert!(
        build::build(&s.opts())
            .await
            .err()
            .unwrap()
            .to_string()
            .contains("nonempty")
    );
    assert!(!s.0.join(".dcl-one").exists());
}

#[test]
fn selection_validates_scripts_and_preserves_scaffold_aliases() {
    let s = Scene::new("dcl-scene-cli build");
    assert!(build_script::command(&s.0).unwrap().is_none());
    s.script("dcl-scene-cli build --built-in");
    assert!(build_script::command(&s.0).unwrap().is_none());
    for value in [json!(""), json!(null), json!(12)] {
        s.write(
            "package.json",
            &json!({"scripts":{"build":value}}).to_string(),
        );
        assert!(build_script::command(&s.0).is_err());
    }
    s.write("package.json", "{");
    assert!(build_script::command(&s.0).is_err());
    let mut opts = s.opts();
    opts.built_in = true;
    assert!(
        build_script::selected(&s.project(), &opts)
            .unwrap()
            .is_none()
    );
    std::fs::remove_file(s.0.join("package.json")).unwrap();
    assert!(build_script::command(&s.0).unwrap().is_none());
}

#[test]
fn cli_recursion_reports_override_and_preserves_output() {
    let s = Scene::new("\"$SDK_TEST_BIN\" build");
    let output = Command::new(env!("CARGO_BIN_EXE_dcl-scene-cli"))
        .args(["build", "--dir"])
        .arg(&s.0)
        .env("SDK_TEST_BIN", env!("CARGO_BIN_EXE_dcl-scene-cli"))
        .output()
        .unwrap();
    let logs = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!output.status.success());
    assert!(logs.contains("recursive"), "{logs}");
    assert!(logs.contains("--built-in"), "{logs}");
    assert!(!s.0.join("bin/index.js").exists());
    let overridden = s.cli(&["build", "--built-in"]);
    assert!(!overridden.status.success(), "fixture has no SDK tsconfig");
    assert!(!String::from_utf8_lossy(&overridden.stderr).contains("recursive"));
}

#[tokio::test]
async fn deploy_uses_custom_tree_even_with_stale_release_artifacts() {
    let s = Scene::new("mkdir -p bin; printf custom > bin/index.js");
    s.write(".dcl-one/release/bin/index.js", "stale loader");
    s.write(".dcl-one/release/bin/sdk-runtime.js", "stale runtime");
    build::build(&s.opts()).await.unwrap();
    let prepared = deploy::prepare(&s.project()).unwrap();
    assert_eq!(
        prepared
            .files
            .iter()
            .find(|(name, _, _)| name == "bin/index.js")
            .unwrap()
            .2,
        b"custom"
    );
    assert!(
        !prepared
            .files
            .iter()
            .any(|(name, _, _)| name == "bin/sdk-runtime.js")
    );
    assert_eq!(
        deploy::payload_path(&s.0, "bin/index.js"),
        s.0.join("bin/index.js")
    );
    let preview = deploy::preview(&s.project()).unwrap();
    assert!(
        preview
            .files
            .iter()
            .any(|(name, size)| name == "bin/index.js" && *size == Some(6))
    );
    assert!(
        !preview
            .files
            .iter()
            .any(|(name, _)| name == "bin/sdk-runtime.js")
    );
}

/// The watch session's next notification. Batches are handled one after
/// another, so the answer to a [`Scene::mark`] proves every batch before it
/// is done: a reload that should not happen arrives first or never.
async fn next_event(rx: &mut tokio::sync::mpsc::UnboundedReceiver<ReloadEvent>) -> ReloadEvent {
    tokio::time::timeout(Duration::from_secs(30), rx.recv())
        .await
        .expect("the watch session went quiet")
        .expect("the watch session ended")
}

fn is_mark(event: &ReloadEvent, name: &str) -> bool {
    matches!(event, ReloadEvent::Model { path, removed: false } if path.ends_with(name))
}

#[tokio::test]
async fn skipped_watch_does_not_stage_loader_and_rebuild_recovers() {
    let s = Scene::new("printf x > attempted.flag; exit 3");
    s.write("bin/index.js", "original");
    s.write("src/index.ts", "initial");
    let session = WatchSession::create(s.project(), &s.opts(), false, &mut ux::Steps::silent())
        .await
        .unwrap();
    assert!(session.is_custom());
    assert_eq!(
        std::fs::read_to_string(s.0.join("bin/index.js")).unwrap(),
        "original"
    );
    assert!(!s.0.join(".dcl-one").exists());
    let fs = FsWatcher::new(&s.0).unwrap();
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    let handle = tokio::spawn(session.run(fs, move |event| {
        tx.send(event).unwrap();
    }));
    s.write("src/index.ts", "fail");
    let attempted = s.0.join("attempted.flag");
    common::wait::until(Duration::from_secs(30), async || {
        attempted.exists().then_some(())
    })
    .await
    .expect("the edit never ran the build script");
    s.mark("failed.glb");
    let event = next_event(&mut rx).await;
    assert!(
        is_mark(&event, "failed.glb"),
        "a failed build must not reload, got {event:?}"
    );
    s.script("mkdir -p bin; printf recovered > bin/index.js");
    assert_eq!(next_event(&mut rx).await, ReloadEvent::Scene);
    assert_eq!(
        std::fs::read_to_string(s.0.join("bin/index.js")).unwrap(),
        "recovered"
    );
    // Two marks: the first may share a batch with the build's own output, in
    // which case a rebuild that output caused still lands before the second.
    for mark in ["after-build.glb", "after-that.glb"] {
        s.mark(mark);
        let event = next_event(&mut rx).await;
        assert!(
            is_mark(&event, mark),
            "output must not cause rebuild loops, got {event:?}"
        );
    }
    handle.abort();
    let _ = handle.await;
}

#[test]
fn build_help_documents_custom_contract() {
    let s = Scene::new("exit 1");
    let output = s.cli(&["build", "--help"]);
    assert!(output.status.success());
    let help = String::from_utf8_lossy(&output.stdout);
    for expected in [
        "scripts.build",
        "--built-in",
        "DCL_SCENE_CLI_PRODUCTION",
        "scene.json",
        "No npm",
    ] {
        assert!(help.contains(expected), "{help}");
    }
}

#[test]
fn cli_build_and_deploy_dry_run_share_the_contract() {
    let s = Scene::new(
        "mkdir -p bin; printf 'exports.onStart = () => {}; // %s' \"$DCL_SCENE_CLI_PRODUCTION\" > bin/index.js",
    );
    let output = s.cli(&["build"]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        std::fs::read_to_string(s.0.join("bin/index.js"))
            .unwrap()
            .ends_with("// 0")
    );
    let output = s.cli(&["deploy", "--dry-run", "--ci", "--yes"]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        std::fs::read_to_string(s.0.join("bin/index.js"))
            .unwrap()
            .ends_with("// 1")
    );
    s.script("exit 29");
    let output = s.cli(&["deploy", "--dry-run", "--ci", "--yes"]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("29"));
    assert!(!s.0.join(".dcl-one/release").exists());
}

#[tokio::test]
async fn retried_custom_watch_reads_updated_scene_main() {
    let s = Scene::new("exit 3");
    let original = s.project();
    assert!(
        WatchSession::create(original.clone(), &s.opts(), true, &mut ux::Steps::silent())
            .await
            .is_err()
    );
    s.write(
        "scene.json",
        &json!({"runtimeVersion":"7", "main":"dist/recovered.js", "scene":{"parcels":["0,0"], "base":"0,0"}}).to_string(),
    );
    s.script("mkdir -p dist; printf recovered > \"$DCL_SCENE_CLI_MAIN\"");
    let session = WatchSession::create(original, &s.opts(), true, &mut ux::Steps::silent())
        .await
        .unwrap();
    assert_eq!(
        session.project().main_output().unwrap(),
        "dist/recovered.js"
    );
    assert_eq!(
        std::fs::read_to_string(s.0.join("dist/recovered.js")).unwrap(),
        "recovered"
    );
    assert!(!s.0.join("bin/index.js").exists());
}

#[tokio::test]
async fn failed_initial_watch_and_preview_never_stage_the_sdk() {
    let s = Scene::new("exit 31");
    assert!(
        WatchSession::create(s.project(), &s.opts(), true, &mut ux::Steps::silent())
            .await
            .is_err()
    );
    let output = tokio::time::timeout(
        Duration::from_secs(15),
        tokio::process::Command::new(env!("CARGO_BIN_EXE_dcl-scene-cli"))
            .args([
                "start",
                "--no-watch",
                "--no-host",
                "--no-mcp",
                "--offline-comms",
                "--port",
                "0",
                "--dir",
            ])
            .arg(&s.0)
            .kill_on_drop(true)
            .output(),
    )
    .await
    .expect("failed preview should exit")
    .unwrap();
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("31"),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!s.0.join("bin/index.js").exists());
    assert!(!s.0.join(".dcl-one/entrypoint.ts").exists());
}

#[tokio::test]
async fn output_path_and_symlinks_cannot_escape_the_scene() {
    use std::os::unix::fs::symlink;
    let s = Scene::new("true");
    let other = Scene::new("true");
    other.write("outside.js", "outside");
    s.write("bin/placeholder", "");
    symlink(other.0.join("outside.js"), s.0.join("bin/index.js")).unwrap();
    assert!(
        build::build(&s.opts())
            .await
            .err()
            .unwrap()
            .to_string()
            .contains("outside")
    );
    s.write(
        "scene.json",
        &json!({"runtimeVersion":"7", "main":"../outside.js"}).to_string(),
    );
    assert!(
        build::build(&s.opts())
            .await
            .err()
            .unwrap()
            .to_string()
            .contains("relative path")
    );
}
