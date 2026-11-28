//! Scenarios for Pulse over WebTransport.

use std::time::Duration;

use anyhow::{Result, anyhow, bail};

use crate::engine::{Ctx, Needs, Scenario, ScenarioFuture, expect_or};
use crate::pulse::{ConnectOptions, PulseBot};

/// The scenarios of this group, in the order a run takes them.
pub fn all() -> Vec<&'static Scenario> {
    vec![&BURSTS]
}

/// How long the walkers walk in the scenario.
pub const BURSTS_DURATION: Duration = Duration::from_secs(100);
/// The walkers walk this many at once.
pub const WALKERS: usize = 5;
/// The most resync requests the observer may make in a full run. A healthy in-process server
/// gave 0 in a 100 s run (walkers on ENet, observer on WebTransport) and 0 in 10 s runs; the
/// limit leaves room for a lost datagram or two on a loaded machine and still fails on a
/// stream of lost baselines. The run records what it measured as a note.
pub const MAX_RESYNCS: u32 = 10;

const HOME: [f32; 3] = [8.0, 0.0, 8.0];
/// One tick every 50 ms. A walker sends one movement input every other tick, and a burst of
/// half the server's input burst allowance back to back at the start of every second: about
/// 17 inputs a second against the 20 the server allows.
const TICK: Duration = Duration::from_millis(50);
const TICKS_PER_SECOND: u32 = 20;

pub static BURSTS: Scenario = Scenario {
    name: "pulse-webtransport-bursts",
    tags: &["pulse", "webtransport", "slow"],
    guards: &["dcl-one #966"],
    summary: "Five walkers send bursts of movement while one observer follows them over WebTransport: the observer's resync requests stay under a limit",
    needs: Needs {
        webtransport: true,
        ..Needs::NONE
    },
    hostile: false,
    default_repeat: 1,
    run: |ctx| -> ScenarioFuture { Box::pin(async move { bursts(ctx).await.map(|_| ()) }) },
};

async fn bursts(ctx: Ctx) -> Result<u32> {
    bursts_for(&ctx, BURSTS_DURATION, MAX_RESYNCS).await
}

/// Runs the scenario for `duration` and returns the resyncs the observer requested; fails if
/// that is more than `limit`. The walkers use the transport of the run, the observer WebTransport.
pub async fn bursts_for(ctx: &Ctx, duration: Duration, limit: u32) -> Result<u32> {
    let realm = ctx.realm("plaza");
    let target = ctx
        .stack()
        .pulse_webtransport
        .as_ref()
        .ok_or_else(|| anyhow!("the stack names no Pulse WebTransport endpoint"))?;
    let observer =
        PulseBot::connect_webtransport(ctx, "observer", target, ConnectOptions::default()).await?;
    observer.accepted(ctx, &ctx.identity(100)).await?;
    observer.teleport(&realm, HOME)?;

    let mut walkers = Vec::new();
    for index in 0..WALKERS {
        let walker = PulseBot::enter(
            ctx,
            &format!("walker-{index}"),
            &ctx.identity(crate::casts::index_u32(index)),
        )
        .await?;
        walker.teleport(&realm, HOME)?;
        walkers.push(walker);
    }
    for (index, walker) in walkers.iter().enumerate() {
        let wallet = ctx.identity(crate::casts::index_u32(index)).address();
        expect_or(
            ctx,
            &format!("the observer sees walker-{index}"),
            Duration::from_secs(10),
            || {
                observer.alive()?;
                walker.alive()?;
                observer
                    .view()
                    .of(&wallet)
                    .map(|_| ())
                    .ok_or_else(|| "not yet".to_owned())
            },
        )
        .await?;
    }

    let started = std::time::Instant::now();
    let mut ticker = tokio::time::interval(TICK);
    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    let mut tick: u32 = 0;
    while started.elapsed() < duration {
        ticker.tick().await;
        let second = tick / TICKS_PER_SECOND;
        let within = tick % TICKS_PER_SECOND;
        let inputs = match within {
            0 => (ctx.stack().limits.input_burst / 2).max(1),
            within if within % 2 == 0 => 1,
            _ => 0,
        };
        for (index, walker) in walkers.iter().enumerate() {
            for step in 0..inputs {
                let phase = crate::casts::f32_from_u32(second * TICKS_PER_SECOND + within + step)
                    * 0.05
                    + crate::casts::f32_from_usize(index);
                walker.move_to([
                    HOME[0] + 4.0 * phase.cos(),
                    0.0,
                    HOME[2] + 4.0 * phase.sin(),
                ])?;
            }
            walker.expect_alive()?;
        }
        observer.expect_alive()?;
        tick += 1;
    }

    let view = observer.view();
    let resyncs = view.resyncs_requested;
    ctx.note("observer resyncs", resyncs);
    ctx.note("observer sequence gaps", view.sequence_gaps);
    ctx.note("observer deltas", view.deltas);
    for (index, _) in walkers.iter().enumerate() {
        let wallet = ctx.identity(crate::casts::index_u32(index)).address();
        if observer.view().of(&wallet).is_none() {
            bail!("the observer lost walker-{index} during the run");
        }
    }
    if resyncs > limit {
        bail!(
            "the observer requested {resyncs} resyncs in {duration:?}, over the limit of {limit}"
        );
    }
    Ok(resyncs)
}
