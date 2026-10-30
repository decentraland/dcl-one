//! Ctrl-C always ends `start`. Against the real binary serving a real scaffold: one SIGINT
//! while the preview is up has the process gone within seconds, cleanly (the cooperative
//! path), and SIGTERM the same. The second-signal and no-listener exits are decided by
//! `dcl_scene_cli::shutdown::decide`, unit-tested there, because a stalled teardown cannot be
//! produced on demand from outside the process.
// Unix only: the test delivers SIGINT and SIGTERM with libc::kill.
#![cfg(unix)]

mod common {
    pub mod testgate;
    pub mod wait;
}

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

fn logged(log: &Path, needle: &str) -> bool {
    std::fs::read_to_string(log).is_ok_and(|text| text.contains(needle))
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

fn scaffold(name: &str) -> (PathBuf, PathBuf) {
    let tmp = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!(
        "dcl-scene-cli-shutdown-{name}-{}",
        std::process::id()
    ));
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
    (tmp, scene)
}

fn start_preview(scene: &Path, log: &Path) -> ChildGuard {
    let out = std::fs::File::create(log).unwrap();
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
    ChildGuard(child)
}

/// The exit status, or None if the process is still running when `within` is up.
fn exit_within(child: &mut Child, within: Duration) -> Option<std::process::ExitStatus> {
    common::wait::until_blocking(within, || child.try_wait().ok().flatten())
}

async fn signal_ends_the_preview(name: &str, signal: libc::c_int, expected_code: i32) {
    if dcl_scene_cli::build::find_node().is_none() {
        let _: Option<()> = common::testgate::unavailable(
            "node",
            "install node; the preview builds the scaffold under it",
        );
        return;
    }
    let (tmp, scene) = scaffold(name);
    let log = tmp.join("start.log");
    let mut guard = start_preview(&scene, &log);
    wait_until("the preview", &log, || logged(&log, "Preview server ready")).await;

    unsafe { libc::kill(guard.0.id() as libc::pid_t, signal) };
    let status = exit_within(&mut guard.0, Duration::from_secs(15)).unwrap_or_else(|| {
        panic!(
            "the preview was still running 15s after signal {signal}:\n{}",
            std::fs::read_to_string(&log).unwrap_or_default()
        )
    });
    assert_eq!(
        status.code(),
        Some(expected_code),
        "exit status {status:?}:\n{}",
        std::fs::read_to_string(&log).unwrap_or_default()
    );
    let _ = std::fs::remove_dir_all(&tmp);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn one_ctrl_c_ends_a_running_preview_cleanly() {
    signal_ends_the_preview("sigint", libc::SIGINT, 0).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn sigterm_ends_a_running_preview_cleanly() {
    signal_ends_the_preview("sigterm", libc::SIGTERM, 0).await;
}
