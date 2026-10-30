//! The one condition wait the node's tests share, for state that cannot signal: a row
//! another transaction writes, a file a child process leaves, a thread with no channel
//! back. Anything the code under test can signal is awaited directly instead.
//!
//! [`HANG_GUARD`] only turns a hang into a failure; nothing is expected to come close to
//! it, so a slow or loaded machine waits longer instead of failing.

use std::time::Duration;

/// How long a wait may last before it is a hang.
pub const HANG_GUARD: Duration = Duration::from_secs(60);

const RECHECK: Duration = Duration::from_millis(10);

/// Re-checks `ready` until it holds; `false` only once [`HANG_GUARD`] has passed. Real
/// time: a test on a paused clock advances it instead of polling.
#[must_use]
pub async fn until(mut ready: impl AsyncFnMut() -> bool) -> bool {
    tokio::time::timeout(HANG_GUARD, async {
        while !ready().await {
            tokio::time::sleep(RECHECK).await;
        }
    })
    .await
    .is_ok()
}

/// [`until`] for a test that runs on a plain thread.
#[must_use]
pub fn until_blocking(mut ready: impl FnMut() -> bool) -> bool {
    let deadline = std::time::Instant::now() + HANG_GUARD;
    while !ready() {
        if std::time::Instant::now() >= deadline {
            return false;
        }
        std::thread::sleep(RECHECK);
    }
    true
}
