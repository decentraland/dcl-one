//! How far a Pulse server's clock is behind its clients', read from its handshake answers.
//!
//! Pulse accepts a handshake signed up to [`MAX_TIMESTAMP_SKEW_MS`] either side of its own now
//! and refuses one signed further ahead ("timestamp outside ... skew window"). A server whose
//! clock lags its clients by more than that window and the network delay refuses every client
//! whose clock is right. This probe measures how much of the window is left: it signs one
//! handshake at the window's far edge (this machine's now plus the window), then each further
//! behind that edge, on a connection of its own and a wallet of its own, until one is accepted.
//!
//! A handshake signed `B` ms behind the edge and sent at `t` reaches the server at `t + d`
//! (one-way delay); the server, `L` behind, reads its now as `t + d - L` and accepts it when
//! `t + W - B <= t + d - L + W`, that is when `L <= B + d`. So the first accepted `B` after a
//! refused `A` puts the lag in `(A, B + d]`, `d` at most the accepted handshake's round trip. A
//! clock that lags by `L` leaves clients that sign at their own now `W - L` of the window. A
//! server that refuses any handshake signed ahead of its own now reads here as one a whole
//! window behind.

use std::time::Instant;

use anyhow::{Result, bail};
use catalyst_comms::pulse::handshake::MAX_TIMESTAMP_SKEW_MS;
use serde_json::json;

use crate::engine::{Ctx, Needs, Scenario, ScenarioFuture};
use crate::identity::now_ms;
use crate::pulse::{ConnectOptions, HandshakeOptions, HandshakeOutcome, PulseBot, messages};

/// How far behind the window's far edge each handshake is signed, in milliseconds. The rung at
/// [`MAX_TIMESTAMP_SKEW_MS`] signs at this machine's now; the last one 32 s behind it.
pub const BEHIND_MS: &[i64] = &[
    0, 100, 250, 500, 1_000, 2_000, 4_000, 8_000, 16_000, 32_000, 60_000, 92_000,
];

/// The timestamp a handshake `behind` ms behind the window's far edge carries, at `now`.
pub fn signed_at(now: i64, behind: i64) -> i64 {
    now + MAX_TIMESTAMP_SKEW_MS - behind
}

pub static HANDSHAKE_CLOCK: Scenario = Scenario {
    name: "pulse-handshake-clock",
    tags: &["pulse", "refusals", "probe"],
    guards: &[
        "probe: a Pulse whose clock lags its clients past its window refuses every handshake",
    ],
    summary: "Signs a handshake at the far edge of Pulse's 60 s window (this machine's clock plus 60 s), then 100 ms to 92 s behind that edge, one connection and wallet each, until the server accepts one: notes each answer and its round trip, the first rung accepted and the bounds it puts on how far the server's clock is behind; fails only if none is accepted",
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
    let mut refused_behind = None;
    for (index, behind) in BEHIND_MS.iter().enumerate() {
        let identity = ctx.identity(index as u32);
        let bot = PulseBot::dial(
            &ctx,
            &format!("probe-{behind}ms"),
            ConnectOptions::default(),
        )
        .await?;
        let sent = Instant::now();
        let now = now_ms();
        bot.send_handshake(
            &identity,
            &HandshakeOptions {
                timestamp_ms: Some(signed_at(now, *behind)),
                ..Default::default()
            },
        );
        let outcome = bot.handshake_outcome(&ctx, None).await;
        let round_trip_ms = sent.elapsed().as_millis() as u64;
        let outcome = match outcome {
            Ok(outcome) => outcome,
            Err(error) => {
                bot.close();
                return Err(error);
            }
        };
        let answer = match &outcome {
            HandshakeOutcome::Accepted => "accepted".to_owned(),
            HandshakeOutcome::Refused { error } => {
                format!("refused: {}", error.as_deref().unwrap_or("no reason"))
            }
            HandshakeOutcome::Disconnected { code } => {
                format!("disconnected: {}", messages::reason(*code))
            }
        };
        attempts.push(json!({
            "behind_edge_ms": behind,
            "signed_ahead_ms": MAX_TIMESTAMP_SKEW_MS - behind,
            "outcome": answer,
            "round_trip_ms": round_trip_ms,
        }));
        if outcome == HandshakeOutcome::Accepted {
            super::support::leave(&ctx, bot).await?;
            accepted = Some((*behind, round_trip_ms));
            break;
        }
        refused_behind = Some(*behind);
        bot.disconnect(catalyst_comms::pulse::hardening::DisconnectReason::Graceful.code());
        bot.close();
    }
    ctx.note("window_ms", MAX_TIMESTAMP_SKEW_MS);
    ctx.note("handshake_attempts", &attempts);
    ctx.note(
        "accepted_behind_edge_ms",
        accepted.map(|(behind, _)| behind),
    );
    // the lag is more than the last refused rung, and at most the accepted rung and its delay
    ctx.note("server_behind_more_than_ms", refused_behind);
    ctx.note(
        "server_behind_at_most_ms",
        accepted.map(|(behind, round_trip)| behind as u64 + round_trip),
    );
    match accepted {
        Some(_) => Ok(()),
        None => bail!(
            "the server accepted no handshake, up to {} ms behind the far edge of its {} ms \
             window: its clock is behind this machine's by more than that",
            BEHIND_MS.last().copied().unwrap_or_default(),
            MAX_TIMESTAMP_SKEW_MS
        ),
    }
}
