//! A watchdog parent and one V8 initializer confined to a separate child process.
#[test]
fn configured_stats_engine_and_http_contract() {
    use std::io::{Read, Write};
    use std::time::{Duration, Instant};
    // The watchdog parent never initializes V8 or receives native state.
    if std::env::var_os("DCL_STATS_ENGINE_CHILD").is_none() {
        let mut child = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "configured_stats_engine_and_http_contract",
                "--nocapture",
            ])
            .env("DCL_STATS_ENGINE_CHILD", "1")
            .stdout(std::process::Stdio::piped())
            .spawn()
            .unwrap();
        let mut stdout = child.stdout.take().unwrap();
        // Drain concurrently so a receipt larger than a pipe buffer cannot stall.
        let output = std::thread::spawn(move || {
            let mut output = String::new();
            stdout.read_to_string(&mut output).unwrap();
            output
        });
        let deadline = Instant::now() + Duration::from_secs(120);
        loop {
            if let Some(status) = child.try_wait().unwrap() {
                let output = output.join().unwrap();
                assert!(status.success(), "engine child failed: {status}\n{output}");
                let receipts: Vec<_> = output
                    .lines()
                    .filter(|line| line.starts_with("STATS_ENGINE_RECEIPT "))
                    .collect();
                assert_eq!(
                    receipts.len(),
                    1,
                    "child must emit exactly one receipt: {output}"
                );
                writeln!(std::io::stdout().lock(), "{}", receipts[0]).unwrap();
                return;
            }
            if Instant::now() >= deadline {
                child.kill().unwrap();
                child.wait().unwrap();
                let output = output.join().unwrap();
                panic!("finite engine experiment exceeded its 120s test watchdog\n{output}");
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }
    let start = std::time::Instant::now();
    catalyst::worlds::scene_state::jsruntime::ensure_v8_initialized();
    let initialization_ns = start.elapsed().as_nanos();
    // Only this test calls V8 in this binary; dependencies have no initialization
    // hooks. This child inherits from the actual initializing thread. The old
    // libtest thread never constructs or receives native handles or matcher Arcs.
    let child = std::thread::spawn(|| {
        std::panic::catch_unwind(|| {
            // SAFETY: all runtimes, handles, clones and their teardown stay in
            // this permitted subtree. run returns only JSON after runtime drop.
            unsafe { catalyst::metrics_matcher::test_support::run() }
        })
        .map_err(|payload| {
            let message = payload
                .downcast_ref::<String>()
                .cloned()
                .or_else(|| payload.downcast_ref::<&str>().map(|s| s.to_string()))
                .unwrap_or_else(|| "non-string panic payload".into());
            // Even a handle-bearing panic_any payload is destroyed in the child.
            drop(payload);
            message
        })
    });
    let result = child
        .join()
        .expect("child wrapper must return plain errors")
        .unwrap();
    // Direct stdout survives libtest success capture and is retained by the runner.
    writeln!(
        std::io::stdout().lock(),
        "STATS_ENGINE_RECEIPT {}",
        serde_json::json!({"initialization_ns":initialization_ns,"result":result})
    )
    .unwrap();
}
