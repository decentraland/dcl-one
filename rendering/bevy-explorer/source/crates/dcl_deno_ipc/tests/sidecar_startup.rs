use std::process::Command;

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
