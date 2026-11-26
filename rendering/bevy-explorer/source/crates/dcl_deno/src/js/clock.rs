use deno_core::{op2, OpDecl, OpState};
use deno_web::StartTime;

/// deno_web's `op_now`, at the resolution scenes have always read: 2 ms.
pub fn override_ops() -> Vec<OpDecl> {
    vec![op_now()]
}

const CLOCK_GRANULARITY: std::time::Duration = std::time::Duration::from_millis(2);

fn coarsen(time: std::time::Duration) -> std::time::Duration {
    to_step(time.as_nanos() / CLOCK_GRANULARITY.as_nanos())
}

fn to_step(steps: u128) -> std::time::Duration {
    let nanos = steps * CLOCK_GRANULARITY.as_nanos();
    std::time::Duration::new(
        (nanos / 1_000_000_000) as u64,
        (nanos % 1_000_000_000) as u32,
    )
}

fn expose_time(time: std::time::Duration, out: &mut [u8]) {
    if out.len() >= 8 {
        out[0..4].copy_from_slice(&(time.as_secs() as u32).to_ne_bytes());
        out[4..8].copy_from_slice(&time.subsec_nanos().to_ne_bytes());
    }
}

fn elapsed(state: &OpState) -> std::time::Duration {
    state
        .try_borrow::<StartTime>()
        .map(|start| start.elapsed())
        .unwrap_or_default()
}

#[op2(fast)]
fn op_now(state: &mut OpState, #[buffer] buf: &mut [u8]) {
    expose_time(coarsen(elapsed(state)), buf);
}
