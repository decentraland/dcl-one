//! The asynchronous wait the explorer scenarios use.

use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;

use catalyst_comms_e2e::engine::Ctx;
use catalyst_comms_e2e::engine::holds_for_async;
use catalyst_comms_e2e::stack::Stack;

fn ctx() -> Ctx {
    Ctx::new(
        Arc::new(Stack::pulse_only("127.0.0.1:9".parse().unwrap())),
        &format!("t{}", std::process::id()),
        "waits",
        0,
        1,
        std::env::temp_dir().join("comms-e2e-tests"),
    )
}

#[tokio::test]
async fn an_asynchronous_probe_that_holds_passes_after_the_whole_window() {
    let asked = Arc::new(AtomicU32::new(0));
    let counter = asked.clone();
    holds_for_async(
        &ctx(),
        "it stays fine",
        Duration::from_millis(100),
        Duration::from_millis(10),
        move || {
            let counter = counter.clone();
            async move {
                counter.fetch_add(1, Ordering::SeqCst);
                Ok(())
            }
        },
    )
    .await
    .unwrap();
    assert!(asked.load(Ordering::SeqCst) >= 5);
}

#[tokio::test]
async fn an_asynchronous_probe_that_breaks_fails_with_the_standard_text() {
    let asked = Arc::new(AtomicU32::new(0));
    let counter = asked.clone();
    let error = holds_for_async(
        &ctx(),
        "it stays fine",
        Duration::from_secs(5),
        Duration::from_millis(10),
        move || {
            let counter = counter.clone();
            async move {
                if counter.fetch_add(1, Ordering::SeqCst) >= 3 {
                    Err("it went wrong".to_owned())
                } else {
                    Ok(())
                }
            }
        },
    )
    .await
    .unwrap_err();
    let text = error.to_string();
    assert!(
        text.starts_with("expected for 5000 ms: it stays fine; broken after "),
        "{text}"
    );
    assert!(text.ends_with(": it went wrong"), "{text}");
}
