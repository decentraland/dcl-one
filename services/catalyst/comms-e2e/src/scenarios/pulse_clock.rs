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
//!
//! Only the server's skew-window refusal is read as the clock's ([`answer_of`]). Any other
//! refusal, or a connection the server ends, ends the probe: it fails with the server's own
//! reason, and its notes say nothing about the clock.

use std::time::Instant;

use anyhow::Result;
use catalyst_comms::pulse::handshake::{HandshakeError, MAX_TIMESTAMP_SKEW_MS};
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
/// window itself for one signed at this machine's now), how the server answered, and its round
/// trip.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Attempt {
    pub behind_edge_ms: i64,
    pub answer: Answer,
    pub round_trip_ms: u64,
}

/// How the server answered a handshake, as the probe reads it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Answer {
    Accepted,
    /// Refused for its timestamp: the server's skew-window refusal
    /// ([`HandshakeError::StaleTimestamp`]), the only answer read as the clock's.
    SkewRefused,
    /// Any other refusal, or the connection ended: the server's reason, which says nothing about
    /// its clock.
    Other(String),
}

impl Attempt {
    pub fn accepted(&self) -> bool {
        self.answer == Answer::Accepted
    }
}

/// How the probe reads `outcome`: a refusal is the clock's only when its error is the server
/// crate's own skew-window message.
pub fn answer_of(outcome: &HandshakeOutcome) -> Answer {
    match outcome {
        HandshakeOutcome::Accepted => Answer::Accepted,
        HandshakeOutcome::Refused { error }
            if error.as_deref() == Some(HandshakeError::StaleTimestamp.message().as_str()) =>
        {
            Answer::SkewRefused
        }
        HandshakeOutcome::Refused { error } => Answer::Other(format!(
            "refused: {}",
            error.as_deref().unwrap_or("no reason")
        )),
        HandshakeOutcome::Disconnected { code } => {
            Answer::Other(format!("disconnected: {}", messages::reason(*code)))
        }
    }
}

/// What the attempts say of the server's clock, in milliseconds: the bounds on one side, or, when
/// no rung is accepted, how far off it is on either side, one or the other. Nothing when an
/// answer was not about the clock.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Reading {
    pub behind_more_than_ms: Option<i64>,
    pub behind_at_most_ms: Option<u64>,
    pub ahead_more_than_ms: Option<i64>,
    pub ahead_at_most_ms: Option<i64>,
    /// Behind by more than the first, or ahead by more than the second.
    pub behind_or_ahead_more_than_ms: Option<(i64, i64)>,
}

/// The first answer of the attempts that is not about the clock, with the attempt it answered.
fn other_answer<'a>(at_now: &'a Attempt, ladder: &'a [Attempt]) -> Option<(&'a Attempt, &'a str)> {
    std::iter::once(at_now)
        .chain(ladder)
        .find_map(|attempt| match &attempt.answer {
            Answer::Other(reason) => Some((attempt, reason.as_str())),
            _ => None,
        })
}

/// Whether the attempts read as a server ahead of this machine past the window: the handshake
/// signed at this machine's now refused, the first rung, at the window's far edge, accepted.
fn reads_ahead(at_now: &Attempt, ladder: &[Attempt]) -> bool {
    !at_now.accepted()
        && ladder
            .first()
            .is_some_and(|first| first.behind_edge_ms == 0 && first.accepted())
}

/// What the attempts say of the server's clock: `at_now`, signed at this machine's own now, and
/// the ladder up to its first answer that is not a skew-window refusal.
pub fn reading(at_now: &Attempt, ladder: &[Attempt]) -> Reading {
    let window = MAX_TIMESTAMP_SKEW_MS;
    if other_answer(at_now, ladder).is_some() {
        return Reading::default();
    }
    if reads_ahead(at_now, ladder) {
        return Reading {
            ahead_more_than_ms: Some(
                window - i64::try_from(at_now.round_trip_ms).unwrap_or(i64::MAX),
            ),
            ahead_at_most_ms: Some(2 * window),
            ..Reading::default()
        };
    }
    let refused_behind = ladder
        .iter()
        .take_while(|attempt| !attempt.accepted())
        .map(|attempt| attempt.behind_edge_ms)
        .last();
    match ladder.iter().find(|attempt| attempt.accepted()) {
        Some(first) => Reading {
            behind_more_than_ms: refused_behind,
            behind_at_most_ms: Some(
                u64::try_from(first.behind_edge_ms).unwrap_or(0) + first.round_trip_ms,
            ),
            ..Reading::default()
        },
        None => Reading {
            behind_or_ahead_more_than_ms: Some((
                ladder.last().map_or(0, |attempt| attempt.behind_edge_ms),
                2 * window
                    - ladder.first().map_or(0, |attempt| {
                        i64::try_from(attempt.round_trip_ms).unwrap_or(i64::MAX)
                    }),
            )),
            ..Reading::default()
        },
    }
}

/// The probe's verdict on its attempts: `at_now`, signed at this machine's own now, and the
/// ladder up to its first answer that is not a skew-window refusal. A client that signs at this
/// machine's now must get in: `at_now` accepted and the first accepted rung no deeper than the
/// window, so not signed behind that now. A failure says which way the server's clock is off and
/// how far the attempts bound it ([`reading`]); an answer that is not about the clock fails the
/// probe with the server's own reason and no word on the clock.
pub fn verdict(at_now: &Attempt, ladder: &[Attempt]) -> Result<(), String> {
    let window = MAX_TIMESTAMP_SKEW_MS;
    if let Some((attempt, reason)) = other_answer(at_now, ladder) {
        let which = if std::ptr::eq(attempt, at_now) {
            "the handshake signed at this machine's now".to_owned()
        } else {
            format!(
                "the handshake signed {} ms behind the far edge of its {window} ms window",
                attempt.behind_edge_ms
            )
        };
        return Err(format!(
            "the server answered {which} for another reason than its clock ({reason}): the probe \
             cannot read the clock"
        ));
    }
    let read = reading(at_now, ladder);
    if let Some((behind, ahead)) = read.behind_or_ahead_more_than_ms {
        return Err(format!(
            "the server accepted no handshake signed from the far edge of its {window} ms window \
             to {behind} ms behind it, {}: its clock is behind this machine's by more than \
             {behind} ms or ahead of it by more than {ahead} ms",
            if at_now.accepted() {
                "though it accepted one signed at this machine's now"
            } else {
                "nor one signed at this machine's now"
            }
        ));
    }
    if let (Some(more_than), Some(at_most)) = (read.ahead_more_than_ms, read.ahead_at_most_ms) {
        return Err(format!(
            "the server refused a handshake signed at this machine's now and accepted one signed \
             {window} ms ahead of it: its clock is ahead of this machine's by more than \
             {more_than} ms and at most {at_most} ms, past its {window} ms window, so it refuses \
             every client whose clock is right"
        ));
    }
    let Some(first) = ladder.iter().find(|attempt| attempt.accepted()) else {
        return Err("the probe recorded no rung".to_owned());
    };
    if at_now.accepted() && first.behind_edge_ms <= window {
        return Ok(());
    }
    let at_most = read.behind_at_most_ms.unwrap_or_default();
    let behind = match read.behind_more_than_ms {
        Some(refused) => {
            format!("behind this machine's by more than {refused} ms and at most {at_most} ms")
        }
        None => format!("behind this machine's by at most {at_most} ms"),
    };
    if at_now.accepted() {
        return Err(format!(
            "the server accepted a handshake signed at this machine's now, but the first rung it \
             accepted is {} ms behind the far edge of its {window} ms window, behind that now: its \
             clock is {behind}, at the edge of the window",
            first.behind_edge_ms
        ));
    }
    Err(format!(
        "the server refused a handshake signed at this machine's now and first accepted one \
         {} ms behind the far edge of its {window} ms window: its clock is {behind}, so it refuses \
         every client whose clock is right",
        first.behind_edge_ms
    ))
}

pub static HANDSHAKE_CLOCK: Scenario = Scenario {
    name: "pulse-handshake-clock",
    tags: &["pulse", "refusals", "probe"],
    guards: &[
        "probe: a Pulse whose clock is off its clients' by more than its window refuses every handshake signed at the client's own now",
    ],
    summary: "Signs one handshake at this machine's now, then one at the far edge of Pulse's 60 s window (this machine's clock plus 60 s) and each further behind it, 100 ms to 92 s behind that edge, one connection and wallet each, until the server accepts one: notes each answer and its round trip, the first rung accepted and the bounds they put on how far the server's clock is behind or ahead; fails when the server refuses the handshake signed at this machine's now or first accepts a rung signed behind it, saying which way its clock is off; any answer but the skew-window refusal fails it with the server's own reason",
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
    let round_trip_ms = crate::casts::millis(sent.elapsed());
    let outcome = match outcome {
        Ok(outcome) => outcome,
        Err(error) => {
            bot.close();
            return Err(error);
        }
    };
    let answer = match answer_of(&outcome) {
        Answer::Accepted => "accepted".to_owned(),
        Answer::SkewRefused => format!("refused: {}", HandshakeError::StaleTimestamp.message()),
        Answer::Other(reason) => reason,
    };
    let attempt = Attempt {
        behind_edge_ms: behind,
        answer: answer_of(&outcome),
        round_trip_ms,
    };
    if attempt.accepted() {
        super::support::leave(ctx, bot).await?;
    } else {
        bot.disconnect(catalyst_comms::pulse::hardening::DisconnectReason::Graceful.code());
        bot.close();
    }
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
        crate::casts::index_u32(BEHIND_MS.len()),
        "probe-now",
        MAX_TIMESTAMP_SKEW_MS,
    )
    .await?;
    ctx.note("signed_at_now", described(&at_now, &answer));
    let mut ladder = Vec::new();
    let mut attempts = Vec::new();
    // a handshake at now answered for another reason than the clock leaves nothing to measure
    if !matches!(at_now.answer, Answer::Other(_)) {
        for (index, behind) in BEHIND_MS.iter().enumerate() {
            let (rung, answer) = attempt(
                &ctx,
                crate::casts::index_u32(index),
                &format!("probe-{behind}ms"),
                *behind,
            )
            .await?;
            attempts.push(described(&rung, &answer));
            // the ladder ends at the first answer that is not a skew-window refusal
            let skew_refused = rung.answer == Answer::SkewRefused;
            ladder.push(rung);
            if !skew_refused {
                break;
            }
        }
    }
    let read = reading(&at_now, &ladder);
    ctx.note("window_ms", MAX_TIMESTAMP_SKEW_MS);
    ctx.note("handshake_attempts", &attempts);
    ctx.note(
        "accepted_behind_edge_ms",
        ladder
            .iter()
            .find(|rung| rung.accepted())
            .map(|rung| rung.behind_edge_ms),
    );
    // only what the verdict claims: one way's bounds, or that the clock is off past the ladder's
    // reach one way or the other, or nothing when an answer was not about the clock
    ctx.note("server_behind_more_than_ms", read.behind_more_than_ms);
    ctx.note("server_behind_at_most_ms", read.behind_at_most_ms);
    ctx.note("server_ahead_more_than_ms", read.ahead_more_than_ms);
    ctx.note("server_ahead_at_most_ms", read.ahead_at_most_ms);
    ctx.note(
        "server_behind_or_ahead_more_than_ms",
        read.behind_or_ahead_more_than_ms
            .map(|(behind, ahead)| json!({ "behind": behind, "ahead": ahead })),
    );
    verdict(&at_now, &ladder).map_err(anyhow::Error::msg)
}
