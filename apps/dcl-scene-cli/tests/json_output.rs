//! `--json`: the command's result as one JSON document on stdout, nothing
//! else there, for the two commands that have a result.

use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

const BIN: &str = env!("CARGO_BIN_EXE_dcl-scene-cli");
const SCENE: &str =
    r#"{"main":"bin/index.js","runtimeVersion":"7","scene":{"parcels":["0,0"],"base":"0,0"}}"#;

struct Fixture(PathBuf);

impl Fixture {
    fn new(tag: &str) -> Self {
        let dir =
            std::env::temp_dir().join(format!("dcl-scene-cli-json-{tag}-{}", std::process::id()));
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

fn run(args: &[&str]) -> Output {
    run_in(&std::env::current_dir().unwrap(), args)
}

fn run_in(cwd: &Path, args: &[&str]) -> Output {
    let mut cmd = Command::new(BIN);
    cmd.args(args).current_dir(cwd);
    for k in [
        "DCL_PRIVATE_KEY",
        "RUST_LOG",
        "DCL_SCENE_CLI_TARGET_SERVER",
        "CI",
    ] {
        cmd.env_remove(k);
    }
    cmd.env("NO_COLOR", "1")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .unwrap()
}

/// The whole of stdout parses as one document.
fn document(out: &Output) -> Value {
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        out.status.success(),
        "exit {:?}\nstdout: {stdout}\nstderr: {}",
        out.status,
        String::from_utf8_lossy(&out.stderr)
    );
    serde_json::from_str(stdout.trim()).unwrap_or_else(|e| panic!("{e}: {stdout}"))
}

#[test]
fn export_static_reports_the_urns_and_entities_as_upstream_does() {
    let f = Fixture::new("export");
    let dir = f.0.display().to_string();
    let dest = f.0.join("out").display().to_string();
    let out = run(&[
        "export-static",
        "--dir",
        &dir,
        "--destination",
        &dest,
        "--skip-build",
        "--baseUrl",
        "https://static.example/scenes/",
        "--json",
    ]);
    let doc = document(&out);
    let urns = doc["scenesUrn"].as_array().unwrap();
    let entities = doc["entities"].as_array().unwrap();
    assert_eq!(urns.len(), 1);
    assert_eq!(entities.len(), 1);
    let entity = entities[0].as_str().unwrap();
    assert!(entity.starts_with("bafkrei"), "{doc}");
    assert_eq!(
        urns[0],
        format!("urn:decentraland:entity:{entity}?=&baseUrl=https://static.example/scenes/")
    );
    let destination = PathBuf::from(doc["destination"].as_str().unwrap());
    assert!(
        destination.join(entity).is_file(),
        "the entity file is at the destination"
    );
}

/// The default flags in a project folder: the destination is the project,
/// so the export would hold no file. The command fails and writes nothing.
#[test]
fn export_static_with_the_default_flags_is_refused_in_a_project_folder() {
    let f = Fixture::new("export-default");
    let out = run_in(&f.0, &["export-static", "--skip-build", "--json"]);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(!out.status.success(), "{stderr}");
    assert!(stderr.contains("holds the project"), "{stderr}");
    assert!(out.stdout.is_empty(), "no result document");
    let mut left: Vec<_> = std::fs::read_dir(&f.0)
        .unwrap()
        .map(|e| e.unwrap().file_name())
        .collect();
    left.sort();
    assert_eq!(left, ["bin", "scene.json"], "nothing is written");
}

#[test]
fn a_dry_run_deploy_reports_the_entity_it_packed() {
    let f = Fixture::new("deploy");
    let dir = f.0.display().to_string();
    let out = run(&[
        "--json",
        "deploy",
        "--dir",
        &dir,
        "--skip-build",
        "--dry-run",
    ]);
    let doc = document(&out);
    assert!(
        doc["entityId"].as_str().unwrap().starts_with("bafkrei"),
        "{doc}"
    );
    assert!(
        doc["message"].as_str().unwrap().contains("dry run"),
        "{doc}"
    );
}

/// A custom `scripts.build` writes to stdout; under `--json` that output
/// joins stderr, so the document is still the whole of stdout.
#[cfg(unix)]
#[test]
fn a_custom_build_script_cannot_write_into_the_json_document() {
    let f = Fixture::new("custom-build");
    std::fs::write(
        f.0.join("package.json"),
        r#"{"name":"custom","private":true,"scripts":{"build":"echo build chatter; echo more chatter >&2"}}"#,
    )
    .unwrap();
    let dir = f.0.display().to_string();
    let out = run(&["--json", "deploy", "--dir", &dir, "--dry-run"]);
    let doc = document(&out);
    assert!(
        doc["entityId"].as_str().unwrap().starts_with("bafkrei"),
        "{doc}"
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("build chatter"), "{stderr}");
    assert!(stderr.contains("more chatter"), "{stderr}");
}

/// `deploy --programmatic` gives stdout to the lines a program parses: the
/// output of a custom `scripts.build` goes to stderr there too.
#[cfg(unix)]
#[test]
fn a_custom_build_script_cannot_write_into_the_programmatic_lines() {
    let f = Fixture::new("custom-build-programmatic");
    std::fs::write(
        f.0.join("package.json"),
        r#"{"name":"custom","private":true,"scripts":{"build":"echo build chatter"}}"#,
    )
    .unwrap();
    let dir = f.0.display().to_string();
    let out = run(&["deploy", "--dir", &dir, "--dry-run", "--programmatic"]);
    let stdout = String::from_utf8_lossy(&out.stdout);
    let lines: Vec<&str> = stdout.lines().collect();
    assert!(!lines.is_empty(), "no state line");
    for line in lines {
        let parsed: Value = serde_json::from_str(line)
            .unwrap_or_else(|e| panic!("stdout line is not JSON ({e}): {line}"));
        assert_eq!(parsed["event"], "state", "{line}");
    }
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("build chatter"), "{stderr}");
}
