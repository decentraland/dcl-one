//! Waiting, the only way a scenario may wait: a condition over observed state and a deadline.
//! Nothing here sleeps for a guessed time.

use std::future::Future;
use std::time::{Duration, Instant};

use anyhow::{Result, bail};

use super::Ctx;
use super::timeline::redact_text;

/// How often a condition is looked at.
const TICK: Duration = Duration::from_millis(5);

/// Waits until `probe` answers, or fails once `deadline` has passed, naming `what` and how long
/// it waited.
pub async fn expect_within<T>(
    ctx: &Ctx,
    what: &str,
    deadline: Duration,
    mut probe: impl FnMut() -> Option<T>,
) -> Result<T> {
    expect_or(ctx, what, deadline, move || probe().ok_or_else(String::new)).await
}

/// [`expect_within`] with a probe that says why it is not there yet; the last reason is part of
/// the failure. A probe may end the wait early by answering `Err` with a reason starting with
/// `fatal:`.
pub async fn expect_or<T>(
    ctx: &Ctx,
    what: &str,
    deadline: Duration,
    mut probe: impl FnMut() -> Result<T, String>,
) -> Result<T> {
    let started = Instant::now();
    loop {
        match probe() {
            Ok(value) => {
                ctx.record("expect", "met", serde_json::json!({ "what": what, "after_ms": started.elapsed().as_millis() as u64 }));
                return Ok(value);
            }
            Err(reason) => {
                let fatal = reason.starts_with("fatal:");
                if fatal || started.elapsed() >= deadline {
                    return Err(not_met(ctx, what, started, &reason));
                }
            }
        }
        tokio::time::sleep(TICK).await;
    }
}

/// [`expect_or`] for a probe that has to ask something (an HTTP route, the explorer's console),
/// looked at every `tick`.
pub async fn expect_async<T, F, Fut>(
    ctx: &Ctx,
    what: &str,
    deadline: Duration,
    tick: Duration,
    mut probe: F,
) -> Result<T>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = Result<T, String>>,
{
    let started = Instant::now();
    loop {
        match probe().await {
            Ok(value) => {
                ctx.record("expect", "met", serde_json::json!({ "what": what, "after_ms": started.elapsed().as_millis() as u64 }));
                return Ok(value);
            }
            Err(reason) => {
                let fatal = reason.starts_with("fatal:");
                if fatal || started.elapsed() >= deadline {
                    return Err(not_met(ctx, what, started, &reason));
                }
            }
        }
        tokio::time::sleep(tick).await;
    }
}

/// Checks that `probe` stays `Ok` for the whole `window`; fails at the first `Err`.
pub async fn holds_for(
    ctx: &Ctx,
    what: &str,
    window: Duration,
    mut probe: impl FnMut() -> Result<(), String>,
) -> Result<()> {
    let started = Instant::now();
    while started.elapsed() < window {
        if let Err(reason) = probe() {
            let reason = redact_text(&reason);
            ctx.record("expect", "broken", serde_json::json!({ "what": what, "after_ms": started.elapsed().as_millis() as u64, "reason": reason }));
            bail!(
                "expected for {} ms: {what}; broken after {} ms: {reason}",
                window.as_millis(),
                started.elapsed().as_millis()
            );
        }
        tokio::time::sleep(TICK).await;
    }
    ctx.record(
        "expect",
        "held",
        serde_json::json!({ "what": what, "for_ms": window.as_millis() as u64 }),
    );
    Ok(())
}

/// [`holds_for`] for a probe that has to ask something (an HTTP route, the console of the
/// explorer), looked at every `tick`. The same failure text.
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
            let reason = redact_text(&reason);
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

fn not_met(ctx: &Ctx, what: &str, started: Instant, reason: &str) -> anyhow::Error {
    let waited = started.elapsed().as_millis() as u64;
    // every failure text of a wait passes here (or through `holds_for`): no secret leaves it
    let reason = redact_text(reason);
    let reason = reason
        .strip_prefix("fatal:")
        .map(str::trim)
        .unwrap_or(&reason);
    ctx.record(
        "expect",
        "not met",
        serde_json::json!({ "what": what, "after_ms": waited, "reason": reason }),
    );
    if reason.is_empty() {
        anyhow::anyhow!("expected: {what}; not met after {waited} ms")
    } else {
        anyhow::anyhow!("expected: {what}; not met after {waited} ms: {reason}")
    }
}
