//! Waits for scenarios whose probe has to ask something (an HTTP route, the explorer's console).
//!
//! `engine::holds_for` takes a synchronous probe. This is its asynchronous variant, with the same
//! failure text, kept here until the engine has one of its own.

use std::future::Future;
use std::time::{Duration, Instant};

use anyhow::{Result, bail};

use crate::engine::Ctx;

/// Checks that `probe` stays `Ok` for the whole `window`, looked at every `tick`; fails at the
/// first `Err` with the engine's standard text: "expected for N ms: ...; broken after M ms: ...".
pub async fn holds_for_async<F, Fut>(
    ctx: &Ctx,
    what: &str,
    window: Duration,
    tick: Duration,
    mut probe: F,
) -> Result<()>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = Result<(), String>>,
{
    let started = Instant::now();
    while started.elapsed() < window {
        if let Err(reason) = probe().await {
            ctx.record(
                "expect",
                "broken",
                serde_json::json!({ "what": what, "after_ms": started.elapsed().as_millis() as u64, "reason": reason }),
            );
            bail!(
                "expected for {} ms: {what}; broken after {} ms: {reason}",
                window.as_millis(),
                started.elapsed().as_millis()
            );
        }
        tokio::time::sleep(tick).await;
    }
    ctx.record(
        "expect",
        "held",
        serde_json::json!({ "what": what, "for_ms": window.as_millis() as u64 }),
    );
    Ok(())
}
