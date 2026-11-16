use std::io::Read;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

fn invoke(args: &[&str], variables: &[(&str, &str)]) -> (i32, String, String) {
    let mut command = Command::new(env!("CARGO_BIN_EXE_catalyst"));
    command
        .env_clear()
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    for key in ["SYSTEMROOT", "TEMP", "TMP", "TMPDIR"] {
        if let Some(value) = std::env::var_os(key) {
            command.env(key, value);
        }
    }
    // Deliberately invalid dependencies make these startup checks finite and
    // prevent database access or listener binding if configuration is accepted.
    command
        .env("DATABASE_URL", "invalid-database-url")
        .env("SERVE_METRICS_BIND", "invalid-bind")
        .env("WKC_METRICS_PUBLIC_PATH", ":");
    for (key, value) in variables {
        command.env(key, value);
    }
    let mut child = command.spawn().unwrap();
    let start = Instant::now();
    let mut stdout = child.stdout.take().unwrap();
    let mut stderr = child.stderr.take().unwrap();
    let out = std::thread::spawn(move || {
        let mut text = String::new();
        stdout.read_to_string(&mut text).unwrap();
        text
    });
    let err = std::thread::spawn(move || {
        let mut text = String::new();
        stderr.read_to_string(&mut text).unwrap();
        text
    });
    loop {
        if child.try_wait().unwrap().is_some() {
            break;
        }
        if start.elapsed() > Duration::from_secs(10) {
            child.kill().unwrap();
            child.wait().unwrap();
            let stdout = out.join().unwrap();
            let stderr = err.join().unwrap();
            panic!("startup hung: {args:?}: {stdout}{stderr}");
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    let status = child.wait().unwrap();
    (
        status.code().unwrap_or(-1),
        out.join().unwrap(),
        err.join().unwrap(),
    )
}

#[test]
fn stats_compilation_is_serve_only_and_follows_cli_refusals() {
    for args in [
        &["--help"][..],
        &["serve", "--help"],
        &["pulse", "--help"],
        &["--version"],
        &["schema", "layout"],
    ] {
        let (code, stdout, stderr) = invoke(args, &[]);
        assert_eq!(code, 0, "{args:?}: {stderr}");
        assert!(stderr.is_empty(), "{args:?}: {stderr}");
        assert!(!stdout.is_empty());
        assert!(!stdout.contains("compiling WKC_METRICS_PUBLIC_PATH"));
        eprintln!(
            "CLI {args:?}: exit={code} stdout_bytes={} stderr_bytes={}",
            stdout.len(),
            stderr.len()
        );
    }
    for args in [
        &[][..],
        &["unknown-command"],
        &["serve", "--unknown-argument"],
    ] {
        let (code, stdout, stderr) = invoke(args, &[]);
        assert_eq!(code, 2, "{args:?}: {stderr}");
        assert!(stdout.is_empty());
        assert!(stderr.contains("Usage:"));
        assert!(!stderr.contains("compiling WKC_METRICS_PUBLIC_PATH"));
        eprintln!(
            "CLI {args:?}: exit={code} stdout_bytes={} stderr_bytes={}",
            stdout.len(),
            stderr.len()
        );
    }
    for (args, variables, expected) in [
        (
            &["serve", "--segment-signing-key", "fixture-secret"][..],
            vec![],
            "a secret must come from",
        ),
        (
            &["serve"][..],
            vec![("NODE_POSTURE", "invalid-posture")],
            "is not one of",
        ),
        (
            &["index"][..],
            vec![],
            "connecting index worker to DATABASE_URL",
        ),
        (
            &["pulse"][..],
            vec![("PULSE_LEGACY_HANDSHAKE", "invalid")],
            "PULSE_LEGACY_HANDSHAKE",
        ),
    ] {
        let (code, stdout, stderr) = invoke(args, &variables);
        assert_eq!(code, 1);
        let text = format!("{stdout}{stderr}");
        assert!(text.contains(expected), "{args:?}: {text}");
        assert!(!text.contains("compiling WKC_METRICS_PUBLIC_PATH"));
        assert!(!text.contains("fixture-secret"));
    }
    let (code, stdout, stderr) = invoke(&["serve"], &[]);
    assert_eq!(code, 1);
    assert!(stdout.is_empty());
    let text = stderr;
    assert!(text.contains("compiling WKC_METRICS_PUBLIC_PATH"), "{text}");
    assert!(text.contains("Missing parameter name"));
    assert!(!text.contains("metrics listening"));
    let (code, stdout, stderr) =
        invoke(&["serve"], &[("WKC_METRICS_PUBLIC_PATH", "/scrape/:name?")]);
    assert_eq!(code, 1);
    let text = format!("{stdout}{stderr}");
    assert!(text.contains("SERVE_METRICS_BIND"), "{text}");
    assert!(!text.contains("compiling WKC_METRICS_PUBLIC_PATH"));
    assert!(!text.contains("metrics listening"));
    eprintln!(
        "startup: help/schema unaffected, Pulse/index skip matcher, argv/posture precede compiler, invalid pattern fails before bind, valid optional pattern reaches metrics-bind validation"
    );
}
