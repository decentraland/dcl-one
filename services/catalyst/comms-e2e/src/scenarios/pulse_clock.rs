//! Whether a client that signs its Pulse handshake at its own now gets in, and how far the
//! server's clock is off this machine's, read from its handshake answers.
//!
//! Pulse accepts a handshake signed up to [`MAX_TIMESTAMP_SKEW_MS`] (`W`) either side of its own
//! now and refuses one signed further off ("timestamp outside ... skew window"). A server whose
//! clock is off its clients' by more than that window and the network delay refuses every client
//! whose clock is right. So the probe first signs one handshake at this machine's now, which the
//! verdict ([`verdict`]) needs accepted. Then it measures how much of the window is left: it signs
//! one handshake at the window's far edge (this machine's now plus the window), then each further
//! behind that edge, until one is accepted. Every handshake has a connection and a wallet of its
//! own.
//!
//! A handshake signed `B` ms behind the edge and sent at `t` reaches the server at `t + d`
//! (one-way delay); a server `L` behind reads its now as `t + d - L` and accepts it when
//! `t + W - B <= t + d - L + W`, that is when `L <= B + d`. So the first accepted `B` after a
//! refused `A` puts the lag in `(A, B + d]`, `d` at most the accepted handshake's round trip. A
//! clock that lags by `L` leaves clients that sign at their own now `W - L` of the window. A rung
//! past `W` signs behind this machine's now: when it is the first accepted, the lag is past the
//! window and the probe fails. A server `L` ahead accepts the first rung while `L <= 2W - d` and
//! refuses the handshake signed at this machine's now once `L > W - d`: refused there and accepted
//! at the edge, its clock is ahead by more than `W` less that handshake's round trip, and at most
//! `2W`.

use std::time::Instant;

use anyhow::Result;
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

/// One handshake of the probe: how far behind the window's far edge it was signed (the
/// window itself for one signed at this machine's now), whether the server accepted it, and its
/// round trip.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Attempt {
    pub behind_edge_ms: i64,
    pub accepted: bool,
    pub round_trip_ms: u64,
}

/// Whether the attempts read as a server ahead of this machine past the window: the handshake
/// signed at this machine's now refused, the first rung, at the window's far edge, accepted.
fn reads_ahead(at_now: &Attempt, ladder: &[Attempt]) -> bool {
    !at_now.accepted
        && ladder
            .first()
            .is_some_and(|first| first.behind_edge_ms == 0 && first.accepted)
}

/// The probe's verdict on its attempts: `at_now`, signed at this machine's own now, and the
/// ladder up to its first accepted rung. A client that signs at this machine's now must get in:
/// `at_now` accepted and the first accepted rung no deeper than the window, so not signed behind
/// that now. A failure says which way the server's clock is off and how far the attempts bound it.
pub fn verdict(at_now: &Attempt, ladder: &[Attempt]) -> Result<(), String> {
    let window = MAX_TIMESTAMP_SKEW_MS;
    let first = ladder.iter().find(|attempt| attempt.accepted);
    let refused_behind = ladder
        .iter()
        .take_while(|attempt| !attempt.accepted)
        .map(|attempt| attempt.behind_edge_ms)
        .last();
    let behind = |first: &Attempt| {
        let at_most = first.behind_edge_ms as u64 + first.round_trip_ms;
        match refused_behind {
            Some(refused) => {
                format!("behind this machine's by more than {refused} ms and at most {at_most} ms")
            }
            None => format!("behind this machine's by at most {at_most} ms"),
        }
    };
    let Some(first) = first else {
        let deepest = ladder.last().map_or(0, |attempt| attempt.behind_edge_ms);
        let ahead_more_than = 2 * window
            - ladder
                .first()
                .map_or(0, |attempt| attempt.round_trip_ms as i64);
        return Err(format!(
            "the server accepted no handshake signed from the far edge of its {window} ms window \
             to {deepest} ms behind it, {}: its clock is behind this machine's by more than \
             {deepest} ms or ahead of it by more than {ahead_more_than} ms",
            if at_now.accepted {
                "though it accepted one signed at this machine's now"
            } else {
                "nor one signed at this machine's now"
            }
        ));
    };
    if at_now.accepted && first.behind_edge_ms <= window {
        return Ok(());
    }
    if reads_ahead(at_now, ladder) {
        return Err(format!(
            "the server refused a handshake signed at this machine's now and accepted one signed \
             {window} ms ahead of it: its clock is ahead of this machine's by more than {} ms and \
             at most {} ms, past its {window} ms window, so it refuses every client whose clock \
             is right",
            window - at_now.round_trip_ms as i64,
            2 * window
        ));
    }
    if at_now.accepted {
        return Err(format!(
            "the server accepted a handshake signed at this machine's now, but the first rung it \
             accepted is {} ms behind the far edge of its {window} ms window, behind that now: its \
             clock is {}, at the edge of the window",
            first.behind_edge_ms,
            behind(first)
        ));
    }
    Err(format!(
        "the server refused a handshake signed at this machine's now and first accepted one \
         {} ms behind the far edge of its {window} ms window: its clock is {}, so it refuses every \
         client whose clock is right",
        first.behind_edge_ms,
        behind(first)
    ))
}

pub static HANDSHAKE_CLOCK: Scenario = Scenario {
    name: "pulse-handshake-clock",
    tags: &["pulse", "refusals", "probe"],
    guards: &[
        "probe: a Pulse whose clock is off its clients' by more than its window refuses every handshake signed at the client's own now",
    ],
    summary: "Signs one handshake at this machine's now, then one at the far edge of Pulse's 60 s window (this machine's clock plus 60 s) and each further behind it, 100 ms to 92 s behind that edge, one connection and wallet each, until the server accepts one: notes each answer and its round trip, the first rung accepted and the bounds they put on how far the server's clock is behind or ahead; fails when the server refuses the handshake signed at this machine's now or first accepts a rung signed behind it, saying which way its clock is off",
    needs: Needs::NONE,
    hostile: false,
    default_repeat: 1,
    run: |ctx| -> ScenarioFuture { Box::pin(probe(ctx)) },
};

/// The scenarios of this group, in the order a run takes them.
pub fn all() -> Vec<&'static Scenario> {
    vec![&HANDSHAKE_CLOCK]
}

/// Signs one handshake `behind` ms behind the window's far edge as wallet `index`, on a
/// connection of its own named `name`: the attempt and the server's answer in words. An accepted
/// one leaves the way a client does; a refused one disconnects.
async fn attempt(ctx: &Ctx, index: u32, name: &str, behind: i64) -> Result<(Attempt, String)> {
    let identity = ctx.identity(index);
    let bot = PulseBot::dial(ctx, name, ConnectOptions::default()).await?;
    let sent = Instant::now();
    let now = now_ms();
    bot.send_handshake(
        &identity,
        &HandshakeOptions {
            timestamp_ms: Some(signed_at(now, behind)),
            ..Default::default()
        },
    );
    let outcome = bot.handshake_outcome(ctx, None).await;
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
    let accepted = outcome == HandshakeOutcome::Accepted;
    if accepted {
        super::support::leave(ctx, bot).await?;
    } else {
        bot.disconnect(catalyst_comms::pulse::hardening::DisconnectReason::Graceful.code());
        bot.close();
    }
    let attempt = Attempt {
        behind_edge_ms: behind,
        accepted,
        round_trip_ms,
    };
    Ok((attempt, answer))
}

/// An attempt as the notes give it.
fn described(attempt: &Attempt, answer: &str) -> serde_json::Value {
    json!({
        "behind_edge_ms": attempt.behind_edge_ms,
        "signed_ahead_ms": MAX_TIMESTAMP_SKEW_MS - attempt.behind_edge_ms,
        "outcome": answer,
        "round_trip_ms": attempt.round_trip_ms,
    })
}

async fn probe(ctx: Ctx) -> Result<()> {
    // a client whose clock is this machine's, on a wallet no rung uses
    let (at_now, answer) = attempt(
        &ctx,
        BEHIND_MS.len() as u32,
        "probe-now",
        MAX_TIMESTAMP_SKEW_MS,
    )
    .await?;
    ctx.note("signed_at_now", described(&at_now, &answer));
    let mut ladder = Vec::new();
    let mut attempts = Vec::new();
    for (index, behind) in BEHIND_MS.iter().enumerate() {
        let (rung, answer) =
            attempt(&ctx, index as u32, &format!("probe-{behind}ms"), *behind).await?;
        attempts.push(described(&rung, &answer));
        ladder.push(rung);
        if rung.accepted {
            break;
        }
    }
    let accepted = ladder.iter().find(|rung| rung.accepted).copied();
    let refused_behind = ladder
        .iter()
        .rev()
        .find(|rung| !rung.accepted)
        .map(|rung| rung.behind_edge_ms);
    let ahead = reads_ahead(&at_now, &ladder);
    ctx.note("window_ms", MAX_TIMESTAMP_SKEW_MS);
    ctx.note("handshake_attempts", &attempts);
    ctx.note(
        "accepted_behind_edge_ms",
        accepted.map(|rung| rung.behind_edge_ms),
    );
    // behind: more than the last refused rung, and at most the accepted rung and its delay;
    // a server that reads ahead has only the ahead bounds
    ctx.note(
        "server_behind_more_than_ms",
        refused_behind.filter(|_| !ahead),
    );
    ctx.note(
        "server_behind_at_most_ms",
        accepted
            .filter(|_| !ahead)
            .map(|rung| rung.behind_edge_ms as u64 + rung.round_trip_ms),
    );
    ctx.note(
        "server_ahead_more_than_ms",
        ahead.then(|| MAX_TIMESTAMP_SKEW_MS - at_now.round_trip_ms as i64),
    );
    ctx.note(
        "server_ahead_at_most_ms",
        ahead.then_some(2 * MAX_TIMESTAMP_SKEW_MS),
    );
    verdict(&at_now, &ladder).map_err(anyhow::Error::msg)
}
