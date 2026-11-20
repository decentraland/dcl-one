//! How far a Pulse server's clock is behind its clients', read from its handshake answers.
//!
//! Pulse refuses a handshake signed after its own now ("timestamp is ahead of this server"), with
//! no margin. A server whose clock lags its clients by more than the network delay therefore
//! refuses every client whose clock is right. This probe signs one handshake at this machine's
//! clock and then each further behind it, on a connection of its own and a wallet of its own,
//! until one is accepted. A handshake signed `B` ms behind and sent at `t` reaches the server at
//! `t + d` (one-way delay) and is accepted when `t - B <= t + d - L` (`L`, the server's lag), so
//! the first accepted `B` after a refused `A` puts the lag in `(A + d, B + d]`.

use std::time::Instant;

use anyhow::{Result, bail};
use serde_json::json;

use crate::engine::{Ctx, Needs, Scenario, ScenarioFuture};
use crate::identity::now_ms;
use crate::pulse::{ConnectOptions, HandshakeOptions, HandshakeOutcome, PulseBot, messages};

/// How far behind this machine's clock each handshake is signed, in milliseconds.
pub const BEHIND_MS: &[i64] = &[0, 100, 250, 500, 1_000, 2_000, 4_000, 8_000, 16_000, 32_000];

pub static HANDSHAKE_CLOCK: Scenario = Scenario {
    name: "pulse-handshake-clock",
    tags: &["pulse", "refusals", "probe"],
    guards: &["probe: a Pulse whose clock lags its clients refuses every handshake"],
    summary: "Signs a handshake at this machine's clock, then 100 ms to 32 s behind it, one connection and wallet each, until the server accepts one: notes each answer and its round trip, and the first lag accepted, which bounds how far the server's clock is behind; fails only if none is accepted",
    needs: Needs::NONE,
    hostile: false,
    default_repeat: 1,
    run: |ctx| -> ScenarioFuture { Box::pin(probe(ctx)) },
};

/// The scenarios of this group, in the order a run takes them.
pub fn all() -> Vec<&'static Scenario> {
    vec![&HANDSHAKE_CLOCK]
}

async fn probe(ctx: Ctx) -> Result<()> {
    let mut attempts = Vec::new();
    let mut accepted = None;
    for (index, behind) in BEHIND_MS.iter().enumerate() {
        let identity = ctx.identity(index as u32);
        let bot = PulseBot::dial(&ctx, &format!("probe-{behind}ms"), ConnectOptions::default()).await?;
        let sent = Instant::now();
        bot.send_handshake(
            &identity,
            &HandshakeOptions {
                timestamp_ms: Some(now_ms() - behind),
                ..Default::default()
            },
        );
        let outcome = bot.handshake_outcome(&ctx, None).await?;
        let round_trip_ms = sent.elapsed().as_millis() as u64;
        let answer = match &outcome {
            HandshakeOutcome::Accepted => "accepted".to_owned(),
            HandshakeOutcome::Refused { error } => {
                format!("refused: {}", error.as_deref().unwrap_or("no reason"))
            }
            HandshakeOutcome::Disconnected { code } => {
                format!("disconnected: {}", messages::reason(*code))
            }
        };
        attempts.push(json!({ "behind_ms": behind, "outcome": answer, "round_trip_ms": round_trip_ms }));
        if outcome == HandshakeOutcome::Accepted {
            super::support::leave(&ctx, bot).await?;
            accepted = Some(*behind);
            break;
        }
        bot.disconnect(catalyst_comms::pulse::hardening::DisconnectReason::Graceful.code());
        bot.close();
    }
    ctx.note("handshake_attempts", &attempts);
    ctx.note("accepted_behind_ms", accepted);
    match accepted {
        Some(_) => Ok(()),
        None => bail!(
            "the server accepted no handshake, up to {} ms behind this machine's clock",
            BEHIND_MS.last().copied().unwrap_or_default()
        ),
    }
}
