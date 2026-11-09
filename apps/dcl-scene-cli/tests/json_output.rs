//! `--json`: the command's result as one JSON document on stdout, nothing
//! else there, for the two commands that have a result.

use serde_json::Value;
use std::path::PathBuf;
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
    let mut cmd = Command::new(BIN);
    cmd.args(args);
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
