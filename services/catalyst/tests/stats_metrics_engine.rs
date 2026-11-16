//! Exactly one V8 initializer in this separate integration-test process.
#[test]
fn configured_stats_engine_and_http_contract() {
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
    println!(
        "STATS_ENGINE_RECEIPT {}",
        serde_json::json!({"initialization_ns":initialization_ns,"result":result})
    );
}
