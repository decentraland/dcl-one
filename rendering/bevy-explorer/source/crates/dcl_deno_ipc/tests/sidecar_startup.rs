use dcl_deno_ipc::{Landlock, SandboxStatus, SceneToEngine};
use interprocess::local_socket::{
    tokio::Stream, traits::tokio::Listener as _, GenericFilePath, ListenerOptions, ToFsName,
};
use std::{
    collections::BTreeMap,
    process::{Command, ExitStatus},
    sync::atomic::{AtomicU32, Ordering},
    time::Duration,
};
use tokio::io::AsyncReadExt;

const TIMEOUT: Duration = Duration::from_secs(30);

#[test]
fn sidecar_reports_a_missing_socket_argument() {
    let output = Command::new(env!("CARGO_BIN_EXE_dcl_deno_ipc"))
        .output()
        .expect("failed to start the scene sidecar");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("No socket name provided"), "{stderr}");
}

#[test]
fn sidecar_rejects_a_zero_v8_thread_count() {
    let output = Command::new(env!("CARGO_BIN_EXE_dcl_deno_ipc"))
        .args(["unused-socket", "--v8-threads", "0"])
        .output()
        .expect("failed to start the scene sidecar");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("--v8-threads takes a positive count"),
        "{stderr}"
    );
}

fn socket_name() -> String {
    static NEXT: AtomicU32 = AtomicU32::new(0);
    let name = format!(
        "dcl_test_{:x}_{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    );
    if cfg!(windows) {
        format!(r"\\.\pipe\{name}")
    } else {
        std::env::temp_dir()
            .join(format!("{name}.sock"))
            .to_string_lossy()
            .into_owned()
    }
}

async fn read_frame(stream: &mut Stream) -> Option<SceneToEngine> {
    let len = stream.read_u64_le().await.ok()?;
    let mut frame = vec![0; len as usize];
    stream.read_exact(&mut frame).await.ok()?;
    Some(rmp_serde::from_slice(&frame).expect("an undecodable frame"))
}

/// Starts the scene host with `args` after its socket and returns its first frame, if it sent one
/// before closing, and how it exited once the connection closed.
fn first_frame(args: &[&str]) -> (Option<SceneToEngine>, ExitStatus) {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    rt.block_on(async {
        let endpoint = socket_name();
        let name = endpoint.as_str().to_fs_name::<GenericFilePath>().unwrap();
        let listener = ListenerOptions::new().name(name).create_tokio().unwrap();
        let mut host = tokio::process::Command::new(env!("CARGO_BIN_EXE_dcl_deno_ipc"))
            .arg(&endpoint)
            .args(args)
            .kill_on_drop(true)
            .spawn()
            .expect("failed to start the scene host");
        let mut stream = tokio::time::timeout(TIMEOUT, listener.accept())
            .await
            .expect("the scene host never connected")
            .unwrap();
        let frame = tokio::time::timeout(TIMEOUT, read_frame(&mut stream))
            .await
            .expect("the scene host neither reported nor closed");
        drop(stream);
        let status = tokio::time::timeout(TIMEOUT, host.wait())
            .await
            .expect("the scene host did not exit once the engine closed")
            .unwrap();
        (frame, status)
    })
}

fn reported_sandbox(frame: Option<SceneToEngine>) -> SandboxStatus {
    match frame {
        Some(SceneToEngine::HostReady { sandbox }) => sandbox,
        Some(_) => panic!("the first frame is not the sandbox report"),
        None => panic!("the scene host closed without reporting"),
    }
}

#[test]
fn an_unsandboxed_scene_host_reports_no_sandbox() {
    let (frame, _) = first_frame(&[]);

    let sandbox = reported_sandbox(frame);

    let landlock = match cfg!(target_os = "linux") {
        true => Landlock::NotEnforced,
        false => Landlock::Unsupported,
    };
    assert_eq!(
        sandbox,
        SandboxStatus {
            landlock,
            seccomp: false,
            no_new_privs: false
        }
    );
}

#[cfg(target_os = "linux")]
#[test]
fn the_scene_host_reports_its_sandbox_before_any_scene() {
    let (frame, _) = first_frame(&["--sandbox", "require"]);

    let sandbox = reported_sandbox(frame);

    assert!(
        sandbox.satisfies(dcl_deno_ipc::SandboxMode::Require),
        "{sandbox:?}"
    );
    assert!(sandbox.seccomp && sandbox.no_new_privs, "{sandbox:?}");
}

#[cfg(not(target_os = "linux"))]
#[test]
fn a_required_sandbox_without_kernel_support_ends_the_host_before_it_reports() {
    let (frame, status) = first_frame(&["--sandbox", "require"]);

    assert!(frame.is_none(), "the host reported without its sandbox");
    assert!(!status.success());
}

#[cfg(not(target_os = "linux"))]
#[test]
fn sandbox_on_without_kernel_support_reports_unsupported() {
    let (frame, _) = first_frame(&["--sandbox", "on"]);

    let sandbox = reported_sandbox(frame);

    assert_eq!(sandbox.landlock, Landlock::Unsupported);
    assert!(!sandbox.seccomp);
}

#[derive(serde::Deserialize)]
struct SelfTest {
    status: Option<SandboxStatus>,
    // each check by name, true when it holds
    report: Option<BTreeMap<String, bool>>,
    holds: Option<bool>,
    error: Option<String>,
}

fn self_test() -> (SelfTest, bool) {
    let output = Command::new(env!("CARGO_BIN_EXE_dcl_deno_ipc"))
        .args(["--sandbox-self-test", "--require"])
        .output()
        .expect("failed to start the scene host");
    let report = serde_json::from_slice(&output.stdout).unwrap_or_else(|e| {
        panic!(
            "the self-test printed no report ({e}): {}",
            String::from_utf8_lossy(&output.stdout)
        )
    });
    (report, output.status.success())
}

#[cfg(target_os = "linux")]
#[test]
fn the_sandbox_self_test_holds_every_check() {
    let (test, passed) = self_test();

    assert_eq!(test.error, None);
    assert!(test
        .status
        .unwrap()
        .satisfies(dcl_deno_ipc::SandboxMode::Require));
    let report = test.report.unwrap();
    let failed = report
        .iter()
        .filter(|(_, holds)| !**holds)
        .collect::<Vec<_>>();
    assert!(failed.is_empty(), "{failed:?}");
    assert!(report.len() >= 16, "{report:?}");
    assert_eq!(test.holds, Some(true));
    assert!(passed);
}

#[cfg(not(target_os = "linux"))]
#[test]
fn the_sandbox_self_test_fails_without_kernel_support() {
    let (test, passed) = self_test();

    assert!(test.error.is_some());
    assert!(test.status.is_none() && test.report.is_none() && test.holds.is_none());
    assert!(!passed);
}
