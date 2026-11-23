//! How Pulse lets go of its ENet peers: when it is told to stop, and when a peer goes silent.
//!
//! Both are ENet's own behaviour, so these scenarios dial Pulse over ENet whatever transport the
//! run names. Each states its rule as a function of what the clients observed
//! ([`judge_stop`], [`judge_silence`]), so the rule is tested without a server.

use std::net::SocketAddr;
use std::time::{Duration, Instant};

use anyhow::{Result, anyhow, bail};
use catalyst_comms::pulse::hardening::DisconnectReason;
use catalyst_comms::pulse::simulation::DEFAULT_PENDING_AUTH_CLEAN_TIMEOUT_MS;
use serde::Serialize;

use super::support::{
    SOON, expect_sees, leave, meeting_place, offset, stack_command, stay_connected,
};
use crate::engine::{Ctx, Needs, Scenario, ScenarioFuture};
use crate::fault::{Direction, Fault, FaultLink};
use crate::identity::Identity;
use crate::pulse::{ConnectOptions, PulseBot, messages};
use crate::stack::Stack;

/// How long the stop command may take to stop Pulse, start it again and return.
const COMMAND_BUDGET: Duration = Duration::from_secs(180);
/// How long the peers are still watched once the stop command has returned: past the 30 s a
/// client's own ENet timeout can take, so a peer that was never told still shows how it ended.
const AFTER_COMMAND: Duration = Duration::from_secs(35);
/// What the peers may take to enter before the stop command starts.
const STOP_SETUP: Duration = Duration::from_secs(20);
/// How long a peer has been connected, as it measures it, when Pulse ends it for not signing in:
/// Pulse's window less two seconds, since Pulse counts from before the peer knows it is connected.
const SIGN_IN_WINDOW_MS: u64 = DEFAULT_PENDING_AUTH_CLEAN_TIMEOUT_MS as u64 - 2_000;
/// Peers signed in to Pulse when it is told to stop; one more has connected and not signed in.
const SIGNED_IN: u32 = 3;
/// How long Pulse may take to answer a handshake again after the restart command.
const BACK_WITHIN: Duration = Duration::from_secs(60);

/// How long after its link goes silent a peer may stay on the server: ENet's 5 s bound, plus up
/// to 3 s for the ping that starts the count after the silence, the retransmission timer the
/// bound is checked on and the tick that tells the observer.
pub const SILENT_PEER_DROPPED_WITHIN: Duration = Duration::from_secs(8);
/// How long a silent peer is watched for: past a 30 s upper bound, so a server that keeps it
/// that long still gives a number.
const SILENCE_WATCH: Duration = Duration::from_secs(45);
/// How long ENet measures the round trip of the path before the path goes silent.
const MEASURE_ROUND_TRIP: Duration = Duration::from_secs(5);

fn run_graceful_stop(ctx: Ctx) -> ScenarioFuture {
    Box::pin(graceful_stop(ctx))
}

fn run_silent_peer(ctx: Ctx) -> ScenarioFuture {
    Box::pin(silent_peer(ctx, Duration::ZERO))
}

fn run_silent_peer_120ms(ctx: Ctx) -> ScenarioFuture {
    Box::pin(silent_peer(ctx, Duration::from_millis(120)))
}

fn run_silent_peer_300ms(ctx: Ctx) -> ScenarioFuture {
    Box::pin(silent_peer(ctx, Duration::from_millis(300)))
}

pub static GRACEFUL_STOP: Scenario = Scenario {
    name: "pulse-graceful-stop",
    tags: &["pulse", "restart"],
    guards: &["dcl-one #1553"],
    summary: "Three ENet peers signed in and one connected but not signed in when the stack file's `pulse_restart_command` stops Pulse: each is told Graceful (1) before the command returns, and Pulse accepts a handshake again afterwards",
    needs: Needs {
        pulse_restart: true,
        ..Needs::NONE
    },
    hostile: true,
    default_repeat: 1,
    run: run_graceful_stop,
};

pub static SILENT_PEER: Scenario = Scenario {
    name: "pulse-silent-peer",
    tags: &["pulse", "faults", "timeouts", "slow"],
    guards: &["dcl-one #1553"],
    summary: "An ENet peer on a plain loopback path goes silent (its link drops everything both ways): an observer sees it leave within 8 s, and again for a second peer on the slot the first left. With a round trip near zero, ENet's retransmission count already ends a silent peer at its 5 s lower bound, whatever the upper one",
    needs: Needs::NONE,
    hostile: false,
    default_repeat: 1,
    run: run_silent_peer,
};

pub static SILENT_PEER_120MS: Scenario = Scenario {
    name: "pulse-silent-peer-120ms-round-trip",
    tags: &["pulse", "faults", "timeouts", "slow"],
    guards: &["dcl-one #1553"],
    summary: "As `pulse-silent-peer` on a path with a 120 ms round trip that ENet measures for 5 s before the silence: an observer sees each silent peer, on its first use of a slot and on the recycled slot, leave within 8 s",
    needs: Needs::NONE,
    hostile: false,
    default_repeat: 1,
    run: run_silent_peer_120ms,
};

pub static SILENT_PEER_300MS: Scenario = Scenario {
    name: "pulse-silent-peer-300ms-round-trip",
    tags: &["pulse", "faults", "timeouts", "slow"],
    guards: &["dcl-one #1553"],
    summary: "As `pulse-silent-peer` on a path with a 300 ms round trip that ENet measures for 5 s before the silence: an observer sees each silent peer, on its first use of a slot and on the recycled slot, leave within 8 s",
    needs: Needs::NONE,
    hostile: false,
    default_repeat: 1,
    run: run_silent_peer_300ms,
};

/// The scenarios of this group, in the order a run takes them.
pub fn all() -> Vec<&'static Scenario> {
    vec![
        &SILENT_PEER,
        &SILENT_PEER_120MS,
        &SILENT_PEER_300MS,
        &GRACEFUL_STOP,
    ]
}

/// The stack file's `pulse_restart_command`, ready to start.
pub fn pulse_restart_command(stack: &Stack) -> Result<tokio::process::Command> {
    stack_command(
        stack,
        stack.pulse_restart_command.as_deref(),
        "pulse_restart_command",
    )
}

/// Dials Pulse over ENet at `target` (Pulse itself or a fault link in front of it), signs in and
/// stands at `world` in `realm`.
async fn enter_on_enet(
    ctx: &Ctx,
    name: &str,
    identity: &Identity,
    target: SocketAddr,
    realm: &str,
    world: [f32; 3],
) -> Result<PulseBot> {
    let bot = connect_on_enet(ctx, name, target, None).await?;
    bot.accepted(ctx, identity).await?;
    bot.teleport(realm, world)?;
    Ok(bot)
}

async fn connect_on_enet(
    ctx: &Ctx,
    name: &str,
    target: SocketAddr,
    deadline: Option<Duration>,
) -> Result<PulseBot> {
    PulseBot::connect(
        ctx,
        name,
        target,
        ConnectOptions {
            bind: ctx.source_address(),
            deadline,
            ..ConnectOptions::default()
        },
    )
    .await
}

// ---------------------------------------------------------------------------------------------
// pulse-graceful-stop

/// How one peer's connection ended after the stop command started, as the peer saw it.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct StopSeen {
    pub peer: String,
    /// The peer had signed in to Pulse; `false` for one that had only connected.
    pub signed_in: bool,
    /// The disconnect code the peer received; 0 is its own ENet timeout. `None`: still connected
    /// when the watch ended.
    pub code: Option<u32>,
    /// When the peer saw its connection end, from the start of the stop command.
    pub after_ms: Option<u64>,
    /// How long the peer had been connected when its connection ended.
    pub connected_for_ms: Option<u64>,
}

/// The stop rule: every peer watched was told `Graceful` before the stop command returned. The
/// command returns once Pulse listens again, so a notice Pulse sends as it stops arrives while
/// the command runs. How long the command takes to reach Pulse is the stack's own and is not
/// judged.
pub fn judge_stop(seen: &[StopSeen], command_took: Duration) -> Result<(), String> {
    if seen.is_empty() {
        return Err("no peer was watched through the stop".to_owned());
    }
    let graceful = DisconnectReason::Graceful.code();
    let command_ms = command_took.as_millis() as u64;
    // A Pulse that stopped leaves no signed-in peer connected: told or not, each one's
    // connection ends within its own timeout, which the watch outlasts.
    let mut signed_in = seen.iter().filter(|peer| peer.signed_in).peekable();
    if signed_in.peek().is_some() && signed_in.all(|peer| peer.code.is_none()) {
        return Err(format!(
            "the stop command did not stop Pulse: it returned after {command_ms} ms and no signed-in peer's connection had ended {} s later",
            AFTER_COMMAND.as_secs()
        ));
    }
    let wrong: Vec<String> = seen
        .iter()
        .filter(|peer| {
            peer.code != Some(graceful) || peer.after_ms.is_none_or(|after| after > command_ms)
        })
        .map(|peer| {
            let state = if peer.signed_in {
                "signed in"
            } else {
                "not signed in"
            };
            let ended = match (peer.code, peer.after_ms) {
                (Some(code), Some(after)) => {
                    format!("{} after {after} ms", messages::reason(code))
                }
                _ => "still connected at the end of the watch".to_owned(),
            };
            // Pulse ends a connection that has not signed in on its own after a while: a command
            // slower than that leaves this peer nothing to be told
            let slow_command = !peer.signed_in
                && peer.code == Some(DisconnectReason::AuthTimeout.code())
                && peer
                    .connected_for_ms
                    .is_some_and(|connected_for| connected_for >= SIGN_IN_WINDOW_MS);
            let cause = if slow_command {
                ": the command took too long to stop Pulse, which had by then ended this connection for not signing in"
            } else {
                ""
            };
            format!("{} ({state}): {ended}{cause}", peer.peer)
        })
        .collect();
    if wrong.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "every ENet peer must be told {} before the stop command returns ({command_ms} ms after it started); {}",
            messages::reason(graceful),
            wrong.join("; ")
        ))
    }
}

/// When the stop command returned, and whether it succeeded; `None` while it runs.
type CommandEnd = parking_lot::Mutex<Option<(Instant, bool)>>;

/// Watches `peers` from `started` until each has seen its connection end. While the command runs
/// the watch goes on; it ends at once when the command fails, and [`AFTER_COMMAND`] after it
/// succeeds.
async fn watch_disconnects(
    peers: &[(PulseBot, bool, Instant)],
    started: Instant,
    command: &CommandEnd,
) -> Vec<StopSeen> {
    let mut ended: Vec<Option<(u32, u64, u64)>> = vec![None; peers.len()];
    loop {
        for ((bot, _, connected), slot) in peers.iter().zip(ended.iter_mut()) {
            if slot.is_none()
                && let Some(code) = bot.view().disconnect
            {
                *slot = Some((
                    code,
                    started.elapsed().as_millis() as u64,
                    connected.elapsed().as_millis() as u64,
                ));
            }
        }
        let over = match *command.lock() {
            None => false,
            Some((_, false)) => true,
            Some((returned, true)) => returned.elapsed() >= AFTER_COMMAND,
        };
        if ended.iter().all(Option::is_some) || over {
            break;
        }
        tokio::time::sleep(Duration::from_millis(2)).await;
    }
    peers
        .iter()
        .zip(ended)
        .map(|((bot, signed_in, _), ended)| StopSeen {
            peer: bot.name().to_owned(),
            signed_in: *signed_in,
            code: ended.map(|(code, _, _)| code),
            after_ms: ended.map(|(_, after, _)| after),
            connected_for_ms: ended.map(|(_, _, connected_for)| connected_for),
        })
        .collect()
}

/// Whether `left` of the scenario's deadline holds the stop command and what follows it: the
/// command's budget, the watch after it and Pulse answering again, and before the peers enter
/// also their setup. A command cut short between its stop and its start leaves Pulse down.
pub fn room_for_the_command(left: Duration, peers_entered: bool) -> Result<(), String> {
    let setup = if peers_entered {
        Duration::ZERO
    } else {
        STOP_SETUP
    };
    let needed = setup + COMMAND_BUDGET + AFTER_COMMAND + BACK_WITHIN;
    if left >= needed {
        return Ok(());
    }
    let (when, deadline) = if peers_entered {
        (
            ", the peers having entered",
            (STOP_SETUP + needed).as_secs(),
        )
    } else {
        ("", needed.as_secs())
    };
    Err(format!(
        "the scenario's deadline leaves {} s{when}, less than the {} s the Pulse restart command may take with the {} s the peers are watched after it and the {} s Pulse may take to answer again{}: give --deadline {deadline} or more",
        left.as_secs(),
        COMMAND_BUDGET.as_secs(),
        AFTER_COMMAND.as_secs(),
        BACK_WITHIN.as_secs(),
        if peers_entered {
            String::new()
        } else {
            format!(", and {} s to set up", STOP_SETUP.as_secs())
        }
    ))
}

async fn graceful_stop(ctx: Ctx) -> Result<()> {
    let mut command = pulse_restart_command(ctx.stack())?;
    let program = command
        .as_std()
        .get_program()
        .to_string_lossy()
        .into_owned();
    // The command has to run to its end: refused here, before anyone enters, and again right
    // before it starts, when the setup has taken what it took.
    let left = |ctx: &Ctx| {
        ctx.ends_at()
            .map(|ends_at| ends_at.saturating_duration_since(Instant::now()))
    };
    if let Some(left) = left(&ctx) {
        room_for_the_command(left, false).map_err(|error| anyhow!(error))?;
    }
    let realm = ctx.realm("stop");
    let here = meeting_place(&ctx);
    let target = ctx.stack().pulse_enet;

    let mut peers: Vec<(PulseBot, bool, Instant)> = Vec::new();
    for index in 0..SIGNED_IN {
        let bot = enter_on_enet(
            &ctx,
            &format!("signed-in-{index}"),
            &ctx.identity(index),
            target,
            &realm,
            offset(here, index as f32, 1.0),
        )
        .await?;
        peers.push((bot, true, Instant::now()));
    }
    // the server has handled every handshake and teleport: the first peer holds the others
    for index in 1..SIGNED_IN {
        expect_sees(
            &ctx,
            &peers[0].0,
            &format!("signed-in-{index}"),
            &ctx.identity(index).address(),
            SOON,
        )
        .await?;
    }
    // connected and never signed in: Pulse holds it as awaiting authentication
    let pending = connect_on_enet(&ctx, "not-signed-in", target, None).await?;
    peers.push((pending, false, Instant::now()));
    {
        let bots: Vec<&PulseBot> = peers.iter().map(|(bot, _, _)| bot).collect();
        stay_connected(
            &ctx,
            "every peer is connected before the stop",
            &bots,
            Duration::from_millis(200),
        )
        .await?;
    }

    if let Some(left) = left(&ctx)
        && let Err(error) = room_for_the_command(left, true)
    {
        for (bot, _, _) in peers {
            bot.close();
        }
        bail!(error);
    }
    ctx.record("runner", "pulse restart", &program);
    let started = Instant::now();
    let mut child = command
        .spawn()
        .map_err(|error| anyhow!("starting the Pulse restart command `{program}`: {error}"))?;
    let command_end = CommandEnd::new(None);
    let (seen, status) = tokio::join!(watch_disconnects(&peers, started, &command_end), async {
        // The child is killed if this future is dropped (the scenario deadline).
        let status = tokio::time::timeout(COMMAND_BUDGET, child.wait()).await;
        let succeeded = matches!(&status, Ok(Ok(status)) if status.success());
        *command_end.lock() = Some((Instant::now(), succeeded));
        (status, started.elapsed())
    });
    let (status, command_took) = status;
    ctx.note("stop", &seen);
    ctx.note("restart_command_ms", command_took.as_millis() as u64);
    for (bot, _, _) in peers {
        bot.close();
    }

    // whatever the peers saw, the stack is left answering as it was found
    let back = pulse_answers_again(&ctx, &ctx.identity(SIGNED_IN + 1)).await;
    // a command that did not do its work is the stack's own failure: nothing is said of Pulse
    let status = status.map_err(|_| {
        anyhow!(
            "the Pulse restart command did not return within {} s: the stop was not judged, and Pulse may be down",
            COMMAND_BUDGET.as_secs()
        )
    })??;
    if !status.success() {
        bail!("the Pulse restart command failed ({status}): the stop was not judged");
    }
    judge_stop(&seen, command_took).map_err(|error| anyhow!(error))?;
    back
}

/// Waits until Pulse accepts a handshake again, then leaves.
async fn pulse_answers_again(ctx: &Ctx, identity: &Identity) -> Result<()> {
    let started = Instant::now();
    let mut attempt = 0;
    loop {
        attempt += 1;
        let entered = async {
            let bot = connect_on_enet(
                ctx,
                &format!("after-restart-{attempt}"),
                ctx.stack().pulse_enet,
                Some(Duration::from_secs(3)),
            )
            .await?;
            bot.accepted(ctx, identity).await?;
            Ok::<PulseBot, anyhow::Error>(bot)
        }
        .await;
        match entered {
            Ok(bot) => {
                ctx.note("pulse_back_after_ms", started.elapsed().as_millis() as u64);
                return leave(ctx, bot).await;
            }
            Err(error) if started.elapsed() < BACK_WITHIN => {
                ctx.record("runner", "Pulse not back yet", format!("{error:#}"));
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
            Err(error) => {
                bail!(
                    "Pulse does not accept a handshake {} s after the restart command: {error:#}",
                    BACK_WITHIN.as_secs()
                )
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------
// pulse-silent-peer

/// What the observer saw of one silent peer.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct SilenceSeen {
    pub peer: String,
    /// The subject id the observer knew the peer by: Pulse's slot for it.
    pub subject: u32,
    /// When the observer stopped holding the peer, from the moment its link went silent.
    /// `None`: still held when the watch ended.
    pub dropped_after_ms: Option<u64>,
}

/// The silence rule: two silent peers, the second on the slot the first left, each dropped from
/// the observer's view within `within` of its link going silent.
pub fn judge_silence(seen: &[SilenceSeen], within: Duration) -> Result<(), String> {
    let [first, recycled] = seen else {
        return Err(format!(
            "two silent peers must be watched, the second on the first one's slot; {} were",
            seen.len()
        ));
    };
    let mut wrong = Vec::new();
    if recycled.subject != first.subject {
        wrong.push(format!(
            "{} took subject {}, not the slot {} {} left, so the recycled slot was not exercised",
            recycled.peer, recycled.subject, first.subject, first.peer
        ));
    }
    let within_ms = within.as_millis() as u64;
    for peer in seen {
        match peer.dropped_after_ms {
            Some(after) if after <= within_ms => {}
            Some(after) => wrong.push(format!(
                "{} (subject {}) was dropped {after} ms after its link went silent",
                peer.peer, peer.subject
            )),
            None => wrong.push(format!(
                "{} (subject {}) was still held when the watch ended",
                peer.peer, peer.subject
            )),
        }
    }
    if wrong.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "the observer must see a silent peer leave within {within_ms} ms of its link going silent, on its slot's first use and on the recycled slot; {}",
            wrong.join("; ")
        ))
    }
}

/// When `observer` stopped holding `wallet`, or `None` if it still does after `watch`.
async fn until_gone(observer: &PulseBot, wallet: &str, watch: Duration) -> Result<Option<Instant>> {
    let end = Instant::now() + watch;
    loop {
        observer.expect_alive()?;
        if observer.view().count_of(wallet) == 0 {
            return Ok(Some(Instant::now()));
        }
        if Instant::now() >= end {
            return Ok(None);
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}

async fn silent_peer(ctx: Ctx, round_trip: Duration) -> Result<()> {
    let realm = ctx.realm("silence");
    let here = meeting_place(&ctx);
    let pulse = ctx.stack().pulse_enet;
    ctx.note("round_trip_ms", round_trip.as_millis() as u64);
    let observer = enter_on_enet(
        &ctx,
        "observer",
        &ctx.identity(0),
        pulse,
        &realm,
        offset(here, 0.0, 2.0),
    )
    .await?;

    let mut seen = Vec::new();
    for (index, label) in [(1, "first-use"), (2, "recycled-slot")] {
        let link = FaultLink::udp(
            &ctx,
            &format!("{label}-link"),
            "127.0.0.1:0".parse()?,
            pulse,
        )
        .await?;
        if !round_trip.is_zero() {
            link.set(
                Direction::Both,
                Fault::Link {
                    delay: round_trip / 2,
                    jitter: Duration::ZERO,
                    loss: 0.0,
                },
            );
        }
        let identity = ctx.identity(index);
        let wallet = identity.address();
        let peer = enter_on_enet(
            &ctx,
            label,
            &identity,
            link.address(),
            &realm,
            offset(here, 1.0, 1.0),
        )
        .await?;
        let subject = expect_sees(&ctx, &observer, label, &wallet, SOON + round_trip * 4).await?;
        stay_connected(
            &ctx,
            &format!("{label} stays connected while ENet measures the round trip"),
            &[&peer, &observer],
            MEASURE_ROUND_TRIP,
        )
        .await?;

        link.set(Direction::Both, Fault::Stall);
        let silent = Instant::now();
        let gone = until_gone(&observer, &wallet, SILENCE_WATCH).await?;
        let dropped_after_ms = gone.map(|at| at.duration_since(silent).as_millis() as u64);
        ctx.note(&format!("{label}_subject"), subject.subject_id);
        ctx.note(&format!("{label}_dropped_after_ms"), dropped_after_ms);
        seen.push(SilenceSeen {
            peer: label.to_owned(),
            subject: subject.subject_id,
            dropped_after_ms,
        });
        // its farewell is lost on the silent link, which goes with it
        peer.close();
        drop(link);
    }
    let left = leave(&ctx, observer).await;
    judge_silence(&seen, SILENT_PEER_DROPPED_WITHIN).map_err(|error| anyhow!(error))?;
    left
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A peer that had been connected for a second when the stop command started.
    fn stopped(peer: &str, signed_in: bool, code: Option<u32>, after_ms: Option<u64>) -> StopSeen {
        StopSeen {
            peer: peer.to_owned(),
            signed_in,
            code,
            after_ms,
            connected_for_ms: after_ms.map(|after| after + 1_000),
        }
    }

    #[test]
    fn the_deadline_must_hold_the_command_before_and_after_the_peers_enter() {
        let seconds = Duration::from_secs;
        assert_eq!(room_for_the_command(seconds(295), false), Ok(()));
        let before = room_for_the_command(seconds(294), false).unwrap_err();
        assert!(before.contains("--deadline 295 or more"), "{before}");
        assert!(before.contains("20 s to set up"), "{before}");
        // once the peers are in, the setup is behind: what is left has to hold the rest
        assert_eq!(room_for_the_command(seconds(275), true), Ok(()));
        let after = room_for_the_command(seconds(274), true).unwrap_err();
        assert!(after.contains("the peers having entered"), "{after}");
        assert!(after.contains("--deadline 295 or more"), "{after}");
    }

    #[test]
    fn a_stop_passes_only_when_every_peer_is_told_graceful_before_the_command_returns() {
        let graceful = DisconnectReason::Graceful.code();
        // a command that took 5.6 s to stop Pulse and bring it back, the notices 2.1 s into it
        let command_took = Duration::from_millis(5617);
        let told = vec![
            stopped("signed-in-0", true, Some(graceful), Some(2102)),
            stopped("signed-in-1", true, Some(graceful), Some(5617)),
            stopped("not-signed-in", false, Some(graceful), Some(2102)),
        ];
        assert_eq!(judge_stop(&told, command_took), Ok(()));

        // a server that closes without a word: each client times out on its own (code 0)
        let silent = vec![
            stopped("signed-in-0", true, Some(0), Some(5400)),
            stopped("not-signed-in", false, Some(0), Some(5410)),
        ];
        let failure = judge_stop(&silent, command_took).unwrap_err();
        assert!(
            failure.contains("signed-in-0 (signed in): 0 (no reason given) after 5400 ms"),
            "{failure}"
        );
        assert!(
            failure.contains("not-signed-in (not signed in)"),
            "{failure}"
        );

        // told after the command returned, told another reason, never told
        for wrong in [
            stopped("late", true, Some(graceful), Some(5618)),
            stopped(
                "other",
                true,
                Some(DisconnectReason::DuplicateSession.code()),
                Some(10),
            ),
            stopped("never", false, None, None),
        ] {
            let mut seen = told.clone();
            seen.push(wrong.clone());
            let failure = judge_stop(&seen, command_took).unwrap_err();
            assert!(failure.contains(&wrong.peer), "{failure}");
            assert!(!failure.contains("signed-in-0"), "{failure}");
        }
        assert!(judge_stop(&[], command_took).is_err());
    }

    #[test]
    fn a_peer_pulse_ended_for_not_signing_in_names_the_slow_command() {
        let graceful = DisconnectReason::Graceful.code();
        let seen = vec![
            stopped("signed-in-0", true, Some(graceful), Some(31_000)),
            stopped(
                "not-signed-in",
                false,
                Some(DisconnectReason::AuthTimeout.code()),
                Some(30_100),
            ),
        ];
        let failure = judge_stop(&seen, Duration::from_secs(33)).unwrap_err();
        assert!(
            failure.contains("the command took too long to stop Pulse"),
            "{failure}"
        );
        assert!(!failure.contains("signed-in-0"), "{failure}");

        // the same code long before Pulse's sign-in window has run out is Pulse's own answer to
        // the stop, and no word on the command
        let early = vec![
            stopped("signed-in-0", true, Some(graceful), Some(400)),
            StopSeen {
                connected_for_ms: Some(2_400),
                ..stopped(
                    "not-signed-in",
                    false,
                    Some(DisconnectReason::AuthTimeout.code()),
                    Some(400),
                )
            },
        ];
        let failure = judge_stop(&early, Duration::from_secs(2)).unwrap_err();
        assert!(failure.contains("AuthTimeout"), "{failure}");
        assert!(!failure.contains("took too long"), "{failure}");
    }

    #[test]
    fn a_command_after_which_no_connection_ended_did_not_stop_pulse() {
        // Pulse was never stopped: the signed-in peers stay, and the one that never signs in is
        // ended by Pulse's own timeout while the watch goes on
        let untouched = vec![
            stopped("signed-in-0", true, None, None),
            stopped("signed-in-1", true, None, None),
            stopped(
                "not-signed-in",
                false,
                Some(DisconnectReason::AuthTimeout.code()),
                Some(29_833),
            ),
        ];
        let failure = judge_stop(&untouched, Duration::from_millis(23)).unwrap_err();
        assert!(
            failure.contains("the stop command did not stop Pulse"),
            "{failure}"
        );
        assert!(!failure.contains("must be told"), "{failure}");
        assert!(!failure.contains("took too long"), "{failure}");
        // one peer left connected among others that ended is still a word on Pulse
        let graceful = DisconnectReason::Graceful.code();
        let mixed = vec![
            stopped("signed-in-0", true, Some(graceful), Some(10)),
            stopped("signed-in-1", true, None, None),
        ];
        let failure = judge_stop(&mixed, Duration::from_millis(23)).unwrap_err();
        assert!(failure.contains("must be told"), "{failure}");
    }

    #[test]
    fn an_empty_restart_command_is_a_missing_one() {
        let pulse =
            Stack::parse("pulse_enet = \"127.0.0.1:9\"\npulse_restart_command = []\n").unwrap();
        let missing = GRACEFUL_STOP.needs.missing(&pulse).unwrap();
        assert!(missing.contains("pulse_restart_command"), "{missing}");
        let api = Stack::parse("pulse_enet = \"127.0.0.1:9\"\nrestart_command = []\n").unwrap();
        let restarts = Needs {
            restart: true,
            ..Needs::NONE
        };
        let missing = restarts.missing(&api).unwrap();
        assert!(missing.contains("restart_command"), "{missing}");
    }

    fn silence(peer: &str, subject: u32, dropped_after_ms: Option<u64>) -> SilenceSeen {
        SilenceSeen {
            peer: peer.to_owned(),
            subject,
            dropped_after_ms,
        }
    }

    #[test]
    fn a_silent_peer_must_go_within_the_bound_on_both_uses_of_a_slot() {
        let within = SILENT_PEER_DROPPED_WITHIN;
        let fixed = [
            silence("first-use", 4, Some(5_300)),
            silence("recycled-slot", 4, Some(5_250)),
        ];
        assert_eq!(judge_silence(&fixed, within), Ok(()));

        // a 30 s upper bound on a path with a round trip: kept well past 8 s
        let kept = [
            silence("first-use", 4, Some(21_900)),
            silence("recycled-slot", 4, Some(8_001)),
        ];
        let failure = judge_silence(&kept, within).unwrap_err();
        assert!(
            failure.contains("first-use (subject 4) was dropped 21900 ms"),
            "{failure}"
        );
        assert!(
            failure.contains("recycled-slot (subject 4) was dropped 8001 ms"),
            "{failure}"
        );

        // the second peer did not take the first one's slot: the case was not exercised
        let elsewhere = [
            silence("first-use", 4, Some(5_000)),
            silence("recycled-slot", 7, Some(5_000)),
        ];
        let failure = judge_silence(&elsewhere, within).unwrap_err();
        assert!(failure.contains("not the slot 4"), "{failure}");

        let held = [
            silence("first-use", 4, Some(5_000)),
            silence("recycled-slot", 4, None),
        ];
        assert!(
            judge_silence(&held, within)
                .unwrap_err()
                .contains("still held")
        );
        assert!(judge_silence(&fixed[..1], within).is_err());
    }

    #[test]
    fn the_stop_scenario_is_hostile_and_needs_the_pulse_restart_command() {
        assert!(GRACEFUL_STOP.hostile);
        let bare = Stack::pulse_only("127.0.0.1:9".parse().unwrap());
        let missing = GRACEFUL_STOP.needs.missing(&bare).unwrap();
        assert!(missing.contains("pulse_restart_command"), "{missing}");
        let named = Stack::parse(
            "pulse_enet = \"127.0.0.1:9\"\npulse_restart_command = [\"restart-pulse\"]\n",
        )
        .unwrap();
        assert!(GRACEFUL_STOP.needs.missing(&named).is_none());
        let command = pulse_restart_command(&named).unwrap();
        assert_eq!(command.as_std().get_program(), "restart-pulse");
        assert!(pulse_restart_command(&bare).is_err());
        for scenario in [&SILENT_PEER, &SILENT_PEER_120MS, &SILENT_PEER_300MS] {
            assert!(!scenario.hostile, "{}", scenario.name);
            assert!(scenario.needs.missing(&bare).is_none(), "{}", scenario.name);
        }
    }
}
