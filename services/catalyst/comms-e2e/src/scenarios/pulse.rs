//! Scenarios for Pulse.
//!
//! Every scenario reaches Pulse through `PulseBot::dial` or `PulseBot::enter`, so each runs on
//! whichever transport the run uses. Players of one scenario stand within a few metres of
//! [`meeting_place`], inside each other's area of interest; a scenario that depends on a larger
//! distance says so where it does.

use std::net::IpAddr;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{Duration, Instant};

use anyhow::{Context as _, Result, anyhow, bail};
use catalyst_comms::pulse::decentraland::pulse::PlayerInitialState;
use catalyst_comms::pulse::hardening::{DEFAULT_CORRUPT_BURST, DisconnectReason};
use futures::future::{join_all, try_join_all};
use parking_lot::Mutex;

use super::support::{
    PACE, SOON, enter_at, expect_gone, expect_roster, expect_sees, expect_sees_at, leave,
    leave_all, listener_identity, meeting_place, metric_total, offset, refusal_code_in,
    sorted_wallets, stay_connected, walk, while_alive,
};
use crate::engine::{Ctx, Needs, Scenario, ScenarioFuture, expect_or, holds_for, together};
use crate::fault::{Direction, Fault, FaultLink};
use crate::identity::{Identity, now_ms};
use crate::pulse::{
    Channel, ConnectOptions, HandshakeOptions, HandshakeOutcome, PulseBot, messages,
};

/// How far a position the server stored may differ from the one sent: one quantization step.
const QUANTIZATION: f32 = 0.15;

macro_rules! runner {
    ($name:ident, $body:path) => {
        fn $name(ctx: Ctx) -> ScenarioFuture {
            Box::pin($body(ctx))
        }
    };
}

runner!(run_smoke, smoke);
runner!(run_refusal_codes, refusal_codes);
runner!(run_auth_timeout, auth_timeout);
runner!(run_rate_limits, rate_limits);
runner!(run_same_wallet_race, same_wallet_race);
runner!(run_handshake_swarm, handshake_swarm);
runner!(run_churn, churn);
runner!(run_realm_round_trip, realm_round_trip);
runner!(run_emote_lifecycle, emote_lifecycle);
runner!(run_resync_requests, resync_requests);
runner!(run_stall_and_resume, stall_and_resume);
runner!(run_scene_listener, scene_listener);

pub static SMOKE: Scenario = Scenario {
    name: "pulse-smoke",
    tags: &["pulse", "smoke"],
    guards: &["baseline"],
    summary: "A traveller handshakes, teleports, walks and leaves; an observer sees the join, the position within one quantization step and the leave",
    needs: Needs::NONE,
    hostile: false,
    default_repeat: 1,
    run: run_smoke,
};

pub static REFUSAL_CODES: Scenario = Scenario {
    name: "pulse-refusal-codes",
    tags: &["pulse", "refusals"],
    guards: &[
        "dcl-one #1205",
        "Pulse #27",
        "Pulse a0d0e17",
        "Pulse 4102862",
    ],
    summary: "One refused frame per fresh connection, each ending with its exact disconnect code, while a bystander in the realm stays connected and unaffected",
    needs: Needs::NONE,
    hostile: true,
    default_repeat: 1,
    run: run_refusal_codes,
};

pub static AUTH_TIMEOUT: Scenario = Scenario {
    name: "pulse-auth-timeout",
    tags: &["pulse", "refusals", "slow"],
    guards: &["Pulse a0d0e17"],
    summary: "A connection that never handshakes is ended with the authentication timeout code (about 30 s), while a bystander stays connected",
    needs: Needs::NONE,
    hostile: false,
    default_repeat: 1,
    run: run_auth_timeout,
};

pub static RATE_LIMITS: Scenario = Scenario {
    name: "pulse-rate-limits",
    tags: &["pulse", "limits"],
    guards: &["dcl-one #1208", "dcl-one #999", "Pulse 89ed3e1"],
    summary: "Inputs and discrete events under, at and over the budget of the stack file: connected for a 3 s window at or under it, ended with code 9 or 10 over it",
    needs: Needs::NONE,
    hostile: true,
    default_repeat: 1,
    run: run_rate_limits,
};

pub static SAME_WALLET_RACE: Scenario = Scenario {
    name: "pulse-same-wallet-race",
    tags: &["pulse", "races"],
    guards: &[
        "Pulse 3164f21",
        "Pulse 007fe24",
        "Pulse fa0aa4c",
        "unity-explorer #10460",
        "dcl-one #201",
    ],
    summary: "Two devices of one wallet handshake within 100 ms of each other: the connection the server handles last survives (the older is ended with DuplicateSession), observers hold one subject for the wallet, the survivor never sees itself join",
    needs: Needs::NONE,
    hostile: false,
    default_repeat: 20,
    run: run_same_wallet_race,
};

pub static HANDSHAKE_SWARM: Scenario = Scenario {
    name: "pulse-handshake-swarm",
    tags: &["pulse", "limits", "swarm"],
    guards: &[
        "dcl-one #943",
        "dcl-one #949",
        "dcl-one #951",
        "Pulse 6318f48",
    ],
    summary: "Many handshakes from one barrier: within the per-address pre-auth budget all are accepted, over it from one address the excess gets PreAuthIpLimitExhausted, spread over addresses it is accepted; an observer keeps receiving a walker throughout",
    needs: Needs::NONE,
    hostile: true,
    default_repeat: 1,
    run: run_handshake_swarm,
};

pub static CHURN: Scenario = Scenario {
    name: "pulse-churn",
    tags: &["pulse", "churn"],
    guards: &["dcl-one #5", "Pulse fa0aa4c"],
    summary: "Bots join and leave in loops for several seconds while two observers watch: at the end their rosters equal the bots still connected, with no ghost and no wallet twice",
    needs: Needs::NONE,
    hostile: false,
    default_repeat: 1,
    run: run_churn,
};

pub static REALM_ROUND_TRIP: Scenario = Scenario {
    name: "pulse-realm-round-trip",
    tags: &["pulse", "realms"],
    guards: &["Pulse #50", "Pulse #31", "unity-explorer d2f314d081"],
    summary: "A traveller teleports from realm A to B and back with an observer in each: each observer sees one leave and one join at the right time, and the movement after the return is seen in A",
    needs: Needs::NONE,
    hostile: false,
    default_repeat: 1,
    run: run_realm_round_trip,
};

pub static EMOTE_LIFECYCLE: Scenario = Scenario {
    name: "pulse-emote-lifecycle",
    tags: &["pulse", "emotes"],
    guards: &["Pulse #22", "Pulse #16", "Pulse 0b869e5"],
    summary: "While walking, a player starts an emote with a mask, restarts it and stops it: the observer sees each event once, in order, with the mask, and the emote cleared after the stop",
    needs: Needs::NONE,
    hostile: false,
    default_repeat: 1,
    run: run_emote_lifecycle,
};

pub static RESYNC_REQUESTS: Scenario = Scenario {
    name: "pulse-resync-requests",
    tags: &["pulse", "resync"],
    guards: &["dcl-one #780", "Pulse #42"],
    summary: "Resync requests with a valid, a stale, a future and an absurd known sequence, and for an unknown subject: never a disconnect, the picture stays correct, a bystander is unaffected",
    needs: Needs::NONE,
    hostile: false,
    default_repeat: 1,
    run: run_resync_requests,
};

pub static STALL_AND_RESUME: Scenario = Scenario {
    name: "pulse-stall-and-resume",
    tags: &["pulse", "faults", "slow"],
    guards: &["dcl-one #1208", "dcl-one #1205"],
    summary: "A walker behind a fault link survives 2 s stalls of the uplink, the downlink and both; a stall past the transport timeout ends its connection cleanly and a new handshake is accepted",
    needs: Needs::NONE,
    hostile: false,
    default_repeat: 1,
    run: run_stall_and_resume,
};

pub static SCENE_LISTENER: Scenario = Scenario {
    name: "pulse-scene-listener",
    tags: &["pulse", "listeners"],
    guards: &["Pulse #29", "Pulse #39", "Pulse #43", "Pulse #46"],
    summary: "A scene listener hears only the players inside its area of interest and in its realm, receives no profile announcements, follows an update of the area, and gets the exact code for an invalid one",
    needs: Needs {
        scene_listener: true,
        ..Needs::NONE
    },
    hostile: false,
    default_repeat: 1,
    run: run_scene_listener,
};

/// The scenarios of this group, in the order a run takes them.
pub fn all() -> Vec<&'static Scenario> {
    vec![
        &SMOKE,
        &REFUSAL_CODES,
        &AUTH_TIMEOUT,
        &RATE_LIMITS,
        &SAME_WALLET_RACE,
        &HANDSHAKE_SWARM,
        &CHURN,
        &REALM_ROUND_TRIP,
        &EMOTE_LIFECYCLE,
        &RESYNC_REQUESTS,
        &STALL_AND_RESUME,
        &SCENE_LISTENER,
    ]
}

async fn dial(ctx: &Ctx, name: &str) -> Result<PulseBot> {
    PulseBot::dial(ctx, name, ConnectOptions::default()).await
}

// ---------------------------------------------------------------------------------------------
// pulse-smoke

async fn smoke(ctx: Ctx) -> Result<()> {
    let realm = ctx.realm("plaza");
    let here = meeting_place(&ctx);
    let (observer_id, traveller_id) = (ctx.identity(0), ctx.identity(1));
    let traveller_wallet = traveller_id.address();

    let observer = enter_at(&ctx, "observer", &observer_id, &realm, here).await?;
    let traveller = PulseBot::enter(&ctx, "traveller", &traveller_id).await?;
    while_alive(&[&observer, &traveller], async {
        let arrival = offset(here, 2.0, 1.0);
        traveller.teleport(&realm, arrival)?;

        let seen = expect_sees_at(
            &ctx,
            &observer,
            "the traveller arriving",
            &traveller_wallet,
            arrival,
            QUANTIZATION,
            SOON,
        )
        .await?;
        if seen.realm != realm {
            bail!(
                "the observer holds the traveller in realm {:?}, expected {realm:?}",
                seen.realm
            );
        }

        let destination = offset(here, 6.0, 1.0);
        walk(&traveller, arrival, destination, 10).await?;
        expect_sees_at(
            &ctx,
            &observer,
            "the traveller after walking 4 m",
            &traveller_wallet,
            destination,
            QUANTIZATION,
            SOON,
        )
        .await?;
        // the traveller sees the observer and is never told about itself
        expect_sees(
            &ctx,
            &traveller,
            "the observer",
            &observer_id.address(),
            SOON,
        )
        .await?;
        if traveller.view().self_joins != 0 {
            bail!("the traveller was told that it joined");
        }

        Ok::<(), anyhow::Error>(())
    })
    .await?;
    traveller.close();
    expect_gone(
        &ctx,
        &observer,
        "the traveller after it left",
        &traveller_wallet,
        SOON,
    )
    .await?;
    let view = observer.view();
    if (view.joins, view.leaves) != (1, 1) {
        bail!(
            "the observer saw {} joins and {} leaves, expected one of each",
            view.joins,
            view.leaves
        );
    }
    Ok(())
}

// ---------------------------------------------------------------------------------------------
// pulse-refusal-codes

/// Where the refused connections of one scenario stand.
struct Arena<'a> {
    ctx: &'a Ctx,
    realm: String,
    here: [f32; 3],
    bystander: &'a PulseBot,
}

impl Arena<'_> {
    /// A bot of `index` in the realm, visible to the bystander.
    async fn visible_bot(&self, label: &str, index: u32) -> Result<(PulseBot, Identity)> {
        let identity = self.ctx.identity(index);
        let bot = enter_at(
            self.ctx,
            &format!("refused-{label}"),
            &identity,
            &self.realm,
            offset(self.here, 1.0, 1.0),
        )
        .await?;
        expect_sees(self.ctx, self.bystander, label, &identity.address(), SOON).await?;
        Ok((bot, identity))
    }

    /// After a refusal: the server removed the refused player, and the bystander is untouched.
    async fn bystander_unaffected(&self, label: &str, wallet: &str) -> Result<()> {
        expect_gone(
            self.ctx,
            self.bystander,
            &format!("the player refused for {label}"),
            wallet,
            SOON,
        )
        .await?;
        self.bystander.expect_alive()
    }

    /// One frame on one connection, which must end it with `expected`.
    async fn refuse(
        &self,
        index: u32,
        label: &str,
        expected: DisconnectReason,
        frames: impl FnOnce(&PulseBot),
    ) -> Result<()> {
        let (bot, identity) = self.visible_bot(label, index).await?;
        frames(&bot);
        bot.expect_disconnect_with(self.ctx, expected, SOON)
            .await
            .with_context(|| {
                format!(
                    "the frame for {label} must end the connection with {}",
                    messages::reason(expected.code())
                )
            })?;
        self.bystander_unaffected(label, &identity.address()).await
    }
}

async fn refusal_codes(ctx: Ctx) -> Result<()> {
    let limits = ctx.stack().limits;
    let grid = ctx.stack().grid;
    let outside_index = grid.encoder().max_index_exclusive();
    let realm = ctx.realm("arena");
    let here = meeting_place(&ctx);
    let standing =
        messages::state_at(&grid, offset(here, 1.0, 1.0)).context("a position inside the grid")?;

    let bystander_id = ctx.identity(0);
    let bystander = enter_at(&ctx, "bystander", &bystander_id, &realm, here).await?;
    let arena = Arena {
        ctx: &ctx,
        realm: realm.clone(),
        here,
        bystander: &bystander,
    };

    arena
        .refuse(
            1,
            "input-outside-grid",
            DisconnectReason::InvalidInputField,
            |bot| {
                bot.send(
                    messages::input(messages::state_in_parcel(outside_index, [1.0, 0.0, 1.0])),
                    Channel::UnreliableSequenced,
                );
            },
        )
        .await?;
    arena
        .refuse(
            2,
            "input-without-state",
            DisconnectReason::InvalidInputField,
            |bot| {
                bot.send(
                    messages::input_without_state(),
                    Channel::UnreliableSequenced,
                );
            },
        )
        .await?;
    arena
        .refuse(
            3,
            "emote-too-long",
            DisconnectReason::InvalidEmoteField,
            |bot| {
                bot.send(
                    messages::emote_start(
                        "wave",
                        Some(limits.max_emote_duration_ms.saturating_add(1)),
                        Some(standing),
                        None,
                    ),
                    Channel::Reliable,
                );
            },
        )
        .await?;
    arena
        .refuse(
            4,
            "emote-without-state",
            DisconnectReason::InvalidEmoteField,
            |bot| {
                bot.send(
                    messages::emote_start("wave", Some(1000), None, None),
                    Channel::Reliable,
                );
            },
        )
        .await?;
    arena
        .refuse(
            5,
            "teleport-empty-realm",
            DisconnectReason::InvalidTeleportField,
            |bot| {
                bot.send(
                    messages::teleport_to_parcel("", standing.parcel_index, &standing),
                    Channel::Reliable,
                );
            },
        )
        .await?;
    arena
        .refuse(
            6,
            "teleport-long-realm",
            DisconnectReason::InvalidTeleportField,
            |bot| {
                let long_realm = "r".repeat(limits.max_realm_length + 1);
                bot.send(
                    messages::teleport_to_parcel(&long_realm, standing.parcel_index, &standing),
                    Channel::Reliable,
                );
            },
        )
        .await?;
    arena
        .refuse(
            7,
            "teleport-outside-grid",
            DisconnectReason::InvalidTeleportField,
            |bot| {
                bot.send(
                    messages::teleport_to_parcel(&realm, outside_index, &standing),
                    Channel::Reliable,
                );
            },
        )
        .await?;

    // A handshake with a bad initial state is answered with a refusal, then the connection ends.
    {
        let identity = ctx.identity(8);
        let bot = dial(&ctx, "refused-bad-initial-state").await?;
        let initial_state = PlayerInitialState {
            state: Some(messages::state_in_parcel(outside_index, [1.0, 0.0, 1.0])),
            realm: realm.clone(),
            ..Default::default()
        };
        let outcome = bot
            .handshake(
                &ctx,
                &identity,
                HandshakeOptions {
                    initial_state: Some(initial_state),
                    ..Default::default()
                },
            )
            .await?;
        // The server sends a refusal that says why and then ends the connection. On ENet the
        // refusal can be lost when the disconnect follows it, so either arrival is accepted; the
        // code is exact, and whether the reason text arrived is noted.
        match outcome {
            HandshakeOutcome::Refused { error: Some(error) } if !error.is_empty() => {
                ctx.note("bad_initial_state_reply_delivered", true);
            }
            HandshakeOutcome::Disconnected { code }
                if code == DisconnectReason::InvalidHandshakeField.code() =>
            {
                ctx.note("bad_initial_state_reply_delivered", false);
            }
            other => bail!(
                "a handshake with a bad initial state must be refused with a reason or end with InvalidHandshakeField, got {other:?}"
            ),
        }
        bot.expect_disconnect_with(&ctx, DisconnectReason::InvalidHandshakeField, SOON)
            .await
            .context("a refused initial state ends the connection with InvalidHandshakeField")?;
        bystander.expect_alive()?;
    }

    // Handshake attempts: each stale one is refused and counted; the next is cut off whatever it is.
    {
        let attempts = u32::from(limits.max_handshake_attempts);
        if attempts == 0 {
            bail!("the stack limits no handshake attempts, so there is no extra attempt to refuse");
        }
        let identity = ctx.identity(9);
        let bot = dial(&ctx, "refused-third-handshake").await?;
        for attempt in 0..attempts {
            // an hour old: far outside the skew the server accepts
            let stale = now_ms() - 3_600_000 - i64::from(attempt);
            let outcome = bot
                .handshake(
                    &ctx,
                    &identity,
                    HandshakeOptions {
                        timestamp_ms: Some(stale),
                        ..Default::default()
                    },
                )
                .await?;
            if !matches!(outcome, HandshakeOutcome::Refused { .. }) {
                bail!(
                    "attempt {} with a stale timestamp must be refused, got {outcome:?}",
                    attempt + 1
                );
            }
        }
        let outcome = bot
            .handshake(&ctx, &identity, HandshakeOptions::default())
            .await?;
        let expected = DisconnectReason::AuthFailed.code();
        if outcome != (HandshakeOutcome::Disconnected { code: expected }) {
            bail!(
                "attempt {} must end the connection with {}, got {outcome:?}",
                attempts + 1,
                messages::reason(expected)
            );
        }
        bystander.expect_alive()?;
    }

    // A replayed handshake: the same wallet, device and timestamp, on a second connection.
    {
        let identity = ctx.identity(10);
        let timestamp = now_ms();
        let first = dial(&ctx, "replay-first").await?;
        let options = HandshakeOptions {
            timestamp_ms: Some(timestamp),
            ..Default::default()
        };
        if first.handshake(&ctx, &identity, options.clone()).await? != HandshakeOutcome::Accepted {
            bail!("the first handshake of the replay case must be accepted");
        }
        first.teleport(&realm, offset(here, 1.0, 2.0))?;
        let replay = dial(&ctx, "refused-replayed-handshake").await?;
        let outcome = replay
            .handshake(&ctx, &identity.same_device(), options)
            .await?;
        let expected = DisconnectReason::HandshakeReplayRejected.code();
        if outcome != (HandshakeOutcome::Disconnected { code: expected }) {
            bail!(
                "a replayed handshake must end with {}, got {outcome:?}",
                messages::reason(expected)
            );
        }
        // the replay did not touch the session it copied
        stay_connected(
            &ctx,
            "the session whose handshake was replayed",
            &[&first, &bystander],
            Duration::from_millis(300),
        )
        .await?;
    }

    // Corrupt frames are tolerated up to the budget, the next one ends the connection.
    {
        let (bot, identity) = arena.visible_bot("corrupt-frames", 11).await?;
        let corrupt = || vec![0xff_u8; 16];
        for _ in 0..DEFAULT_CORRUPT_BURST {
            bot.send_raw(corrupt(), Channel::Reliable);
        }
        stay_connected(
            &ctx,
            "corrupt frames inside the budget",
            &[&bot],
            Duration::from_millis(400),
        )
        .await?;
        bot.send_raw(corrupt(), Channel::Reliable);
        bot.expect_disconnect_with(&ctx, DisconnectReason::PacketCorrupted, SOON)
            .await
            .context(
                "the corrupt frame past the budget ends the connection with PacketCorrupted",
            )?;
        arena
            .bystander_unaffected("corrupt-frames", &identity.address())
            .await?;
    }

    // One grossly oversized frame ends the connection at once.
    arena
        .refuse(
            12,
            "oversized-frame",
            DisconnectReason::PacketCorrupted,
            |bot| {
                bot.send_raw(vec![0_u8; 20_000], Channel::Reliable);
            },
        )
        .await?;

    // The server still serves: the bystander moves, and a newcomer sees it there.
    let moved = offset(here, 0.0, 3.0);
    bystander.move_to(moved)?;
    let newcomer_id = ctx.identity(13);
    let newcomer = enter_at(
        &ctx,
        "newcomer",
        &newcomer_id,
        &realm,
        offset(here, 2.0, 0.0),
    )
    .await?;
    expect_sees_at(
        &ctx,
        &newcomer,
        "the bystander",
        &bystander_id.address(),
        moved,
        QUANTIZATION,
        SOON,
    )
    .await?;
    bystander.expect_alive()
}

// ---------------------------------------------------------------------------------------------
// pulse-auth-timeout

/// The server ends a connection without a handshake after 30 s; waiting for it takes up to 45 s.
async fn auth_timeout(ctx: Ctx) -> Result<()> {
    let realm = ctx.realm("arena");
    let here = meeting_place(&ctx);
    let bystander = enter_at(&ctx, "bystander", &ctx.identity(0), &realm, here).await?;
    let silent = dial(&ctx, "silent").await?;
    let started = Instant::now();
    silent
        .expect_disconnect_with(&ctx, DisconnectReason::AuthTimeout, Duration::from_secs(45))
        .await?;
    ctx.note("auth_timeout_ms", started.elapsed().as_millis() as u64);
    bystander.expect_alive()
}

// ---------------------------------------------------------------------------------------------
// pulse-rate-limits

/// How long a steady rate is held: long enough for the burst to be spent and the refill rate to
/// be what is tested.
const STEADY_WINDOW: Duration = Duration::from_secs(3);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Budget {
    Input,
    Discrete,
}

impl Budget {
    fn label(self) -> &'static str {
        match self {
            Budget::Input => "input",
            Budget::Discrete => "discrete",
        }
    }

    /// One event of this budget: a movement input, or in turn a teleport, an emote start and an
    /// emote stop. `number` tells events apart so none repeats the last.
    fn send(self, bot: &PulseBot, realm: &str, here: [f32; 3], number: u32) -> Result<()> {
        let spot = offset(here, (number % 2) as f32, 0.0);
        match self {
            Budget::Input => bot.move_to(spot),
            Budget::Discrete => match number % 3 {
                0 => bot.teleport(realm, spot),
                1 => {
                    let state = messages::state_at(bot.grid(), spot);
                    bot.send(
                        messages::emote_start("wave", Some(500), state, None),
                        Channel::Reliable,
                    );
                    Ok(())
                }
                _ => {
                    bot.send(messages::emote_stop(), Channel::Reliable);
                    Ok(())
                }
            },
        }
    }
}

/// Where a rate probe stands and who it is.
struct Probe<'a> {
    ctx: &'a Ctx,
    realm: &'a str,
    here: [f32; 3],
    index: u32,
    budget: Budget,
    kind: &'a str,
}

impl Probe<'_> {
    async fn bot(&self) -> Result<PulseBot> {
        let name = format!("{}-{}", self.budget.label(), self.kind);
        PulseBot::enter(self.ctx, &name, &self.ctx.identity(self.index)).await
    }

    /// A bot that sends `rate_hz` events a second for [`STEADY_WINDOW`] and must stay connected.
    async fn steady(&self, rate_hz: f64) -> Result<()> {
        let bot = self.bot().await?;
        let period = Duration::from_secs_f64(1.0 / rate_hz);
        let started = Instant::now();
        let mut sent = 0_u32;
        while started.elapsed() < STEADY_WINDOW {
            bot.expect_alive()?;
            self.budget.send(&bot, self.realm, self.here, sent)?;
            sent += 1;
            // each event is due at its own time, so a late wake-up is made up for and the
            // average stays exactly at the rate
            tokio::time::sleep_until(tokio::time::Instant::from_std(started + period * sent)).await;
        }
        bot.expect_alive()?;
        self.ctx.note(
            &format!("{}-{} events sent", self.budget.label(), self.kind),
            sent,
        );
        Ok(())
    }

    /// A bot that sends `count` events back to back. `reason` is what must end its connection,
    /// or `None` when it must stay connected.
    async fn at_once(&self, count: u32, reason: Option<DisconnectReason>) -> Result<()> {
        let bot = self.bot().await?;
        for number in 0..count {
            if reason.is_none() {
                bot.expect_alive()?;
            }
            self.budget.send(&bot, self.realm, self.here, number)?;
        }
        let label = self.budget.label();
        match reason {
            Some(reason) => bot
                .expect_disconnect_with(self.ctx, reason, SOON)
                .await
                .with_context(|| {
                    format!("{count} {label} events back to back are over the budget")
                }),
            None => {
                stay_connected(
                    self.ctx,
                    &format!("{count} {label} events back to back, at the budget"),
                    &[&bot],
                    Duration::from_millis(500),
                )
                .await
            }
        }
    }
}

async fn rate_limits(ctx: Ctx) -> Result<()> {
    let limits = ctx.stack().limits;
    if limits.input_max_hz == 0
        || limits.input_burst == 0
        || limits.discrete_rate_per_sec == 0
        || limits.discrete_burst == 0
    {
        bail!("the stack file switches a rate limit off, so there is no budget to test");
    }
    let realm = ctx.realm("arena");
    let here = meeting_place(&ctx);
    let bystander = enter_at(&ctx, "bystander", &ctx.identity(0), &realm, here).await?;

    let (input_hz, discrete_hz) = (
        f64::from(limits.input_max_hz),
        f64::from(limits.discrete_rate_per_sec),
    );
    // the buckets hold at most 255, as the server caps them
    let (input_burst, discrete_burst) =
        (limits.input_burst.min(255), limits.discrete_burst.min(255));
    let probe = |index, budget, kind| Probe {
        ctx: &ctx,
        realm: &realm,
        here,
        index,
        budget,
        kind,
    };
    let probes = [
        probe(1, Budget::Input, "under"),
        probe(2, Budget::Input, "at"),
        probe(3, Budget::Input, "burst-at"),
        probe(4, Budget::Input, "over"),
        probe(5, Budget::Discrete, "under"),
        probe(6, Budget::Discrete, "at"),
        probe(7, Budget::Discrete, "burst-at"),
        probe(8, Budget::Discrete, "over"),
    ];
    tokio::try_join!(
        probes[0].steady(input_hz * 0.8),
        probes[1].steady(input_hz),
        probes[2].at_once(input_burst, None),
        probes[3].at_once(
            input_burst * 2 + limits.input_max_hz,
            Some(DisconnectReason::InputRateExceeded)
        ),
        probes[4].steady(discrete_hz * 0.8),
        probes[5].steady(discrete_hz),
        probes[6].at_once(discrete_burst, None),
        probes[7].at_once(
            discrete_burst * 2 + limits.discrete_rate_per_sec,
            Some(DisconnectReason::DiscreteEventRateExceeded)
        ),
    )?;
    // each probe checked its own reason; the bystander was never part of it
    bystander.expect_alive()
}

// ---------------------------------------------------------------------------------------------
// pulse-same-wallet-race

/// Handshakes sent further apart than this reach the server in the order they were sent, so the
/// later one must be the one that survives; closer together either order is possible.
const ORDER_RESOLUTION: Duration = Duration::from_millis(25);

async fn same_wallet_race(ctx: Ctx) -> Result<()> {
    let realm = ctx.realm("arena");
    let here = meeting_place(&ctx);
    let grid = ctx.stack().grid;
    let identity = ctx.identity(0);
    let wallet = identity.address();
    let devices = [identity.same_device(), identity.second_device()];
    // each device stands somewhere of its own, so what the observers hold says which one is live
    let spots = [offset(here, 1.0, 1.0), offset(here, 3.0, 1.0)];

    let observer_ids = [ctx.identity(1), ctx.identity(2)];
    let observers = [
        enter_at(
            &ctx,
            "observer-1",
            &observer_ids[0],
            &realm,
            offset(here, 0.0, 2.0),
        )
        .await?,
        enter_at(
            &ctx,
            "observer-2",
            &observer_ids[1],
            &realm,
            offset(here, 2.0, 2.0),
        )
        .await?,
    ];

    let connections = [dial(&ctx, "device-1").await?, dial(&ctx, "device-2").await?];
    let first = ctx.first_of_two();
    let second = 1 - first;
    let gap = ctx.offset_within(Duration::from_millis(100));
    ctx.note("handshake_gap_us", gap.as_micros() as u64);
    let signed_at = now_ms();
    let options = |which: usize, timestamp_ms: i64| HandshakeOptions {
        initial_state: messages::initial_state(&grid, &realm, spots[which]),
        timestamp_ms: Some(timestamp_ms),
        ..Default::default()
    };
    // distinct timestamps: two handshakes of one wallet signed in the same millisecond would be
    // refused as a replay, which is not what this scenario is about
    connections[first].send_handshake(&devices[first], &options(first, signed_at));
    tokio::time::sleep(gap).await;
    connections[second].send_handshake(&devices[second], &options(second, signed_at + 1));

    // exactly one connection is ended, and it is ended as a duplicate
    let (loser, code) = expect_or(
        &ctx,
        "exactly one of the two connections is ended",
        SOON,
        || {
            let ended: Vec<(usize, u32)> = (0..2)
                .filter_map(|which| {
                    connections[which]
                        .view()
                        .disconnect
                        .map(|code| (which, code))
                })
                .collect();
            match ended.as_slice() {
                [] => Err("both connections are alive".to_owned()),
                [(which, code)] => Ok((*which, *code)),
                _ => Err(format!("fatal: both connections were ended: {ended:?}")),
            }
        },
    )
    .await?;
    let survivor = 1 - loser;
    if code != DisconnectReason::DuplicateSession.code() {
        bail!(
            "the older session of the wallet must be ended with {}, got {}",
            messages::reason(DisconnectReason::DuplicateSession.code()),
            messages::reason(code)
        );
    }
    if gap >= ORDER_RESOLUTION && survivor != second {
        bail!(
            "the handshakes were {} ms apart, so the later one must win: connection {} was sent last but connection {} survived",
            gap.as_millis(),
            second + 1,
            survivor + 1
        );
    }
    ctx.note("survivor_was_sent_last", survivor == second);
    expect_or(&ctx, "the surviving connection is accepted", SOON, || {
        connections[survivor].alive()?;
        match connections[survivor].view().handshake {
            Some(true) => Ok(()),
            Some(false) => Err("fatal: the surviving connection was refused".to_owned()),
            None => Err("no answer yet".to_owned()),
        }
    })
    .await?;

    // the survivor walks; every observer ends up with one subject, standing where the survivor is
    let target = offset(spots[survivor], 4.0, 0.0);
    walk(&connections[survivor], spots[survivor], target, 8).await?;
    for observer in &observers {
        expect_sees_at(
            &ctx,
            observer,
            "the wallet",
            &wallet,
            target,
            QUANTIZATION,
            SOON,
        )
        .await?;
    }
    for (position, observer) in observers.iter().enumerate() {
        let mut roster = vec![wallet.clone(), observer_ids[1 - position].address()];
        roster.sort();
        expect_roster(&ctx, observer, &roster, SOON).await?;
    }
    // and it stays that way
    holds_for(
        &ctx,
        "every observer holds one subject for the wallet",
        Duration::from_millis(400),
        || {
            for observer in &observers {
                observer.alive()?;
                let count = observer.view().count_of(&wallet);
                if count != 1 {
                    return Err(format!(
                        "{} holds {count} subjects for the wallet",
                        observer.name()
                    ));
                }
            }
            if connections[survivor].view().self_joins != 0 {
                return Err("the survivor was told that its own wallet joined".to_owned());
            }
            connections[survivor].alive()
        },
    )
    .await
}

// ---------------------------------------------------------------------------------------------
// pulse-handshake-swarm

/// Players over the per-address budget that the swarm adds from one address.
const EXTRA_BOTS: usize = 8;

/// What a swarm of bots met: the ones the server admitted, the codes of the ones it refused, and
/// what hung.
struct Swarm {
    admitted: Vec<PulseBot>,
    refused: Vec<u32>,
    hung: Vec<String>,
}

impl Swarm {
    fn refusals(&self) -> Vec<String> {
        self.refused
            .iter()
            .map(|code| messages::reason(*code))
            .collect()
    }
}

/// `count` bots dial from one barrier, with every connection pending at once, then all
/// handshake. Bot `i` binds `addresses[i % addresses.len()]` (an empty list leaves the choice to
/// the system) and signs as identity `first_index + i`.
async fn swarm(ctx: &Ctx, first_index: u32, count: usize, addresses: &[IpAddr]) -> Swarm {
    let addresses = addresses.to_vec();
    let dialed = together(count, |position| {
        let ctx = ctx.clone();
        let bind = (!addresses.is_empty()).then(|| addresses[position % addresses.len()]);
        async move {
            let options = ConnectOptions {
                bind,
                deadline: Some(SOON),
                ..Default::default()
            };
            PulseBot::dial(&ctx, &format!("swarm-{first_index}-{position}"), options)
                .await
                .map_err(|error| format!("{error:#}"))
        }
    })
    .await;

    let mut result = Swarm {
        admitted: Vec::new(),
        refused: Vec::new(),
        hung: Vec::new(),
    };
    let mut pending = Vec::new();
    for (position, dialed) in dialed.into_iter().enumerate() {
        match dialed {
            Ok(bot) => pending.push((position, bot)),
            Err(message) => match refusal_code_in(&message) {
                Some(code) => result.refused.push(code),
                None => result.hung.push(message),
            },
        }
    }
    let outcomes = join_all(pending.iter().map(|(position, bot)| {
        let identity = ctx.identity(first_index + *position as u32);
        async move {
            bot.handshake(
                ctx,
                &identity,
                HandshakeOptions {
                    deadline: Some(SOON),
                    ..Default::default()
                },
            )
            .await
        }
    }))
    .await;
    for ((_, bot), outcome) in pending.into_iter().zip(outcomes) {
        match outcome {
            Ok(HandshakeOutcome::Accepted) => result.admitted.push(bot),
            Ok(HandshakeOutcome::Disconnected { code }) => result.refused.push(code),
            Ok(HandshakeOutcome::Refused { error }) => {
                result
                    .hung
                    .push(format!("{}: handshake refused: {error:?}", bot.name()));
            }
            Err(error) => result.hung.push(format!("{error:#}")),
        }
    }
    result
}

/// The longest the observer of a walker may go without any update of it.
const STREAM_STALL: Duration = Duration::from_millis(1500);

/// A walker that never stops while the swarm runs, and a watch that its observer keeps receiving
/// it. Ends only with an error.
async fn walk_and_watch(
    walker: &PulseBot,
    observer: &PulseBot,
    here: [f32; 3],
    last_sent: &Mutex<[f32; 3]>,
    inputs_sent: &AtomicU32,
    updates_seen: &AtomicU32,
) -> Result<()> {
    let updates = || {
        let view = observer.view();
        view.deltas + view.full_states
    };
    let mut last_update = updates();
    let mut last_progress = Instant::now();
    let mut step = 0_u32;
    loop {
        walker.expect_alive()?;
        observer.expect_alive()?;
        let spot = offset(here, 1.0 + (step % 40) as f32 * 0.15, 1.0);
        *last_sent.lock() = spot;
        walker.move_to(spot)?;
        step += 1;
        inputs_sent.store(step, Ordering::SeqCst);
        tokio::time::sleep(PACE).await;
        let now = updates();
        updates_seen.store(now, Ordering::SeqCst);
        if now != last_update {
            last_update = now;
            last_progress = Instant::now();
        } else if last_progress.elapsed() > STREAM_STALL {
            bail!(
                "the observer received nothing of the walker for {} ms during the swarm",
                last_progress.elapsed().as_millis()
            );
        }
    }
}

async fn handshake_swarm(ctx: Ctx) -> Result<()> {
    let cap = ctx.stack().limits.max_concurrent_pre_auth_per_ip;
    if cap <= 0 {
        bail!("the stack file has no per-address pre-auth budget, so there is none to test");
    }
    let cap = cap as usize;
    let realm = ctx.realm("arena");
    let here = meeting_place(&ctx);
    let (observer_id, walker_id) = (ctx.identity(0), ctx.identity(1));
    let observer = enter_at(
        &ctx,
        "observer",
        &observer_id,
        &realm,
        offset(here, 0.0, 2.0),
    )
    .await?;
    let walker = enter_at(&ctx, "walker", &walker_id, &realm, offset(here, 1.0, 1.0)).await?;
    expect_sees(&ctx, &observer, "the walker", &walker_id.address(), SOON).await?;

    let last_sent = Mutex::new(offset(here, 1.0, 1.0));
    let (inputs_sent, updates_seen) = (AtomicU32::new(0), AtomicU32::new(0));
    let updates_before = {
        let view = observer.view();
        view.deltas + view.full_states
    };

    let phases = async {
        // (a) within the per-address budget: everything is accepted
        let within = swarm(&ctx, 100, cap, &[]).await;
        if !within.hung.is_empty() || !within.refused.is_empty() || within.admitted.len() != cap {
            bail!(
                "{cap} handshakes from one address are within the budget and must all be accepted: {} accepted, refused with {:?}, hung: {:?}",
                within.admitted.len(),
                within.refusals(),
                within.hung
            );
        }
        leave_all(&ctx, within.admitted).await?;

        // (b) over the budget from one address: the excess is refused by address, nothing hangs
        let over = swarm(&ctx, 200, cap + EXTRA_BOTS, &[]).await;
        let refusal = DisconnectReason::PreAuthIpLimitExhausted.code();
        if !over.hung.is_empty() {
            bail!(
                "{} of {} handshakes hung or were refused without a code: {:?}",
                over.hung.len(),
                cap + EXTRA_BOTS,
                over.hung
            );
        }
        if over.admitted.len() != cap
            || over.refused.len() != EXTRA_BOTS
            || over.refused.iter().any(|code| *code != refusal)
        {
            bail!(
                "{} handshakes from one address, {cap} allowed: expected {cap} accepted and {EXTRA_BOTS} refused with {}, got {} accepted and refusals {:?}",
                cap + EXTRA_BOTS,
                messages::reason(refusal),
                over.admitted.len(),
                over.refusals()
            );
        }
        leave_all(&ctx, over.admitted).await?;

        // (c) the same number again, spread over two addresses: the limit is per address
        let addresses: [IpAddr; 2] = ["127.0.0.2".parse()?, "127.0.0.3".parse()?];
        let spread = swarm(&ctx, 300, cap * 2, &addresses).await;
        if !spread.hung.is_empty() || !spread.refused.is_empty() || spread.admitted.len() != cap * 2
        {
            bail!(
                "{} handshakes over {} addresses, {cap} each, are within the budget of each: {} accepted, refused with {:?}, hung: {:?}",
                cap * 2,
                addresses.len(),
                spread.admitted.len(),
                spread.refusals(),
                spread.hung
            );
        }
        leave_all(&ctx, spread.admitted).await?;
        Ok(())
    };
    let pump = walk_and_watch(
        &walker,
        &observer,
        here,
        &last_sent,
        &inputs_sent,
        &updates_seen,
    );
    tokio::select! {
        biased;
        done = phases => done?,
        stopped = pump => stopped?,
    }

    let sent = inputs_sent.load(Ordering::SeqCst);
    let received = updates_seen
        .load(Ordering::SeqCst)
        .saturating_sub(updates_before);
    ctx.note("walker_inputs_during_swarm", sent);
    ctx.note("observer_updates_during_swarm", received);
    if received < sent / 3 {
        bail!(
            "during the swarm the walker sent {sent} inputs but the observer received only {received} updates"
        );
    }
    let final_spot = *last_sent.lock();
    expect_sees_at(
        &ctx,
        &observer,
        "the walker where it stopped",
        &walker_id.address(),
        final_spot,
        QUANTIZATION,
        SOON,
    )
    .await?;
    stay_connected(
        &ctx,
        "the walker and the observer after the swarm",
        &[&walker, &observer],
        Duration::from_millis(200),
    )
    .await
}

// ---------------------------------------------------------------------------------------------
// pulse-churn

/// How long the churners join and leave.
const CHURN_WINDOW: Duration = Duration::from_secs(6);
const CHURNERS: u32 = 4;

/// The bots a churn leaves connected, with their wallets.
type Staying = Vec<Option<(PulseBot, String)>>;

/// One churner: joins as a fresh wallet, stays a little while, leaves, and does it again until
/// the window ends. An even slot stays connected at the end; its bot and wallet are returned.
async fn churner(
    ctx: &Ctx,
    realm: &str,
    here: [f32; 3],
    slot: u32,
    until: Instant,
) -> Result<Option<(PulseBot, String)>> {
    let mut cycle = 0_u32;
    loop {
        let identity = ctx.identity(10 + slot * 1000 + cycle);
        let spot = offset(here, 1.0 + (slot % 3) as f32, 1.0 + (slot / 3) as f32);
        let bot = enter_at(
            ctx,
            &format!("churn-{slot}-{cycle}"),
            &identity,
            realm,
            spot,
        )
        .await?;
        let stay = Duration::from_millis(100 + ctx.random_below(300));
        let last = Instant::now() + stay >= until;
        tokio::time::sleep(stay).await;
        bot.expect_alive()?;
        if last && slot.is_multiple_of(2) {
            return Ok(Some((bot, identity.address())));
        }
        leave(ctx, bot).await?;
        if last {
            return Ok(None);
        }
        cycle += 1;
    }
}

async fn churn(ctx: Ctx) -> Result<()> {
    let realm = ctx.realm("arena");
    let here = meeting_place(&ctx);
    let observer_ids = [ctx.identity(0), ctx.identity(1)];
    let observers = [
        enter_at(
            &ctx,
            "observer-1",
            &observer_ids[0],
            &realm,
            offset(here, 0.0, 2.0),
        )
        .await?,
        enter_at(
            &ctx,
            "observer-2",
            &observer_ids[1],
            &realm,
            offset(here, 2.0, 2.0),
        )
        .await?,
    ];

    let until = Instant::now() + CHURN_WINDOW;
    let work = try_join_all((0..CHURNERS).map(|slot| churner(&ctx, &realm, here, slot, until)));
    // no wallet is ever held twice by an observer, at any moment of the churn
    let watch = async {
        loop {
            for observer in &observers {
                if let Err(reason) = observer.alive() {
                    return Err::<Staying, _>(anyhow!("{}: {}", observer.name(), reason));
                }
                let view = observer.view();
                let mut wallets = view.wallets();
                let total = wallets.len();
                wallets.dedup();
                if wallets.len() != total {
                    return Err(anyhow!(
                        "{} holds a wallet twice: {:?}",
                        observer.name(),
                        view.wallets()
                    ));
                }
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    };
    let finished: Staying = tokio::select! {
        biased;
        done = work => done?,
        broken = watch => return broken.map(|_| ()),
    };

    let staying: Vec<(PulseBot, String)> = finished.into_iter().flatten().collect();
    ctx.note("bots_still_connected", staying.len() as u32);
    for (position, observer) in observers.iter().enumerate() {
        let mut roster = vec![observer_ids[1 - position].address()];
        roster.extend(staying.iter().map(|(_, wallet)| wallet.clone()));
        roster.sort();
        expect_roster(&ctx, observer, &roster, Duration::from_secs(10)).await?;
    }
    // nobody was ended on the way
    let mut everyone: Vec<&PulseBot> = staying.iter().map(|(bot, _)| bot).collect();
    everyone.extend(observers.iter());
    stay_connected(
        &ctx,
        "the survivors and the observers",
        &everyone,
        Duration::from_millis(300),
    )
    .await
}

// ---------------------------------------------------------------------------------------------
// pulse-realm-round-trip

async fn realm_round_trip(ctx: Ctx) -> Result<()> {
    let (realm_a, realm_b) = (ctx.realm("a"), ctx.realm("b"));
    let here = meeting_place(&ctx);
    let (observer_a_id, observer_b_id, traveller_id) =
        (ctx.identity(0), ctx.identity(1), ctx.identity(2));
    let wallet = traveller_id.address();
    let observer_a = enter_at(
        &ctx,
        "observer-a",
        &observer_a_id,
        &realm_a,
        offset(here, 0.0, 2.0),
    )
    .await?;
    let observer_b = enter_at(
        &ctx,
        "observer-b",
        &observer_b_id,
        &realm_b,
        offset(here, 2.0, 2.0),
    )
    .await?;
    let traveller = PulseBot::enter(&ctx, "traveller", &traveller_id).await?;

    while_alive(&[&observer_a, &observer_b, &traveller], async {
    let in_a = offset(here, 1.0, 1.0);
    let in_b = offset(here, 3.0, 1.0);
    let back_in_a = offset(here, 1.0, 3.0);

    // arrives in A: A's observer sees it, B's does not
    traveller.teleport(&realm_a, in_a)?;
    let seen = expect_sees_at(
        &ctx,
        &observer_a,
        "the traveller arriving in A",
        &wallet,
        in_a,
        QUANTIZATION,
        SOON,
    )
    .await?;
    if seen.realm != realm_a {
        bail!(
            "observer A holds the traveller in realm {:?}, expected {realm_a:?}",
            seen.realm
        );
    }
    expect_roster(&ctx, &traveller, &[observer_a_id.address()], SOON).await?;
    stay_connected(
        &ctx,
        "nobody in B hears of a traveller in A",
        &[&observer_b],
        Duration::from_millis(200),
    )
    .await?;
    if observer_b.view().count_of(&wallet) != 0 {
        bail!("observer B holds the traveller although it is in realm A");
    }

    // goes to B: A's observer sees one leave, B's observer one join, in B
    traveller.teleport(&realm_b, in_b)?;
    expect_gone(
        &ctx,
        &observer_a,
        "the traveller after it went to B",
        &wallet,
        SOON,
    )
    .await?;
    let seen = expect_sees_at(
        &ctx,
        &observer_b,
        "the traveller arriving in B",
        &wallet,
        in_b,
        QUANTIZATION,
        SOON,
    )
    .await?;
    if seen.realm != realm_b {
        bail!(
            "observer B holds the traveller in realm {:?}, expected {realm_b:?}",
            seen.realm
        );
    }
    // the traveller's own picture follows: B's observer is there, A's is gone
    expect_roster(&ctx, &traveller, &[observer_b_id.address()], SOON).await?;

    // returns to A
    traveller.teleport(&realm_a, back_in_a)?;
    expect_gone(
        &ctx,
        &observer_b,
        "the traveller after it left B",
        &wallet,
        SOON,
    )
    .await?;
    expect_sees_at(
        &ctx,
        &observer_a,
        "the traveller back in A",
        &wallet,
        back_in_a,
        QUANTIZATION,
        SOON,
    )
    .await?;
    expect_roster(&ctx, &traveller, &[observer_a_id.address()], SOON).await?;

    // movement after the return is accepted and seen in A
    let walked_to = offset(back_in_a, 4.0, 0.0);
    walk(&traveller, back_in_a, walked_to, 8).await?;
    expect_sees_at(
        &ctx,
        &observer_a,
        "the traveller walking after the return",
        &wallet,
        walked_to,
        QUANTIZATION,
        SOON,
    )
    .await?;

    // each event happened once: A saw the traveller join twice and leave once, B once each
    holds_for(&ctx, "each observer saw each event once", Duration::from_millis(300), || {
        for bot in [&observer_a, &observer_b, &traveller] {
            bot.alive()?;
        }
        let (a, b) = (observer_a.view(), observer_b.view());
        let counts = [(a.joins, 2, a.leaves, 1, "A"), (b.joins, 1, b.leaves, 1, "B")];
        for (joins, want_joins, leaves, want_leaves, label) in counts {
            if joins != want_joins || leaves != want_leaves {
                return Err(format!(
                    "observer {label} saw {joins} joins and {leaves} leaves of the traveller, expected {want_joins} and {want_leaves}"
                ));
            }
        }
        Ok(())
    })
    .await
    })
    .await
}

// ---------------------------------------------------------------------------------------------
// pulse-emote-lifecycle

/// Moves `walker` back and forth without end, recording where it was last sent; ends only with
/// an error.
async fn walk_forever(walker: &PulseBot, here: [f32; 3], current: &Mutex<[f32; 3]>) -> Result<()> {
    let mut step = 0_u32;
    loop {
        walker.expect_alive()?;
        let spot = offset(here, 1.0 + (step % 40) as f32 * 0.1, 1.0);
        *current.lock() = spot;
        walker.move_to(spot)?;
        step += 1;
        tokio::time::sleep(PACE).await;
    }
}

async fn emote_lifecycle(ctx: Ctx) -> Result<()> {
    let realm = ctx.realm("arena");
    let here = meeting_place(&ctx);
    let grid = ctx.stack().grid;
    let (observer_id, walker_id) = (ctx.identity(0), ctx.identity(1));
    let wallet = walker_id.address();
    let observer = enter_at(
        &ctx,
        "observer",
        &observer_id,
        &realm,
        offset(here, 0.0, 2.0),
    )
    .await?;
    let walker = enter_at(&ctx, "walker", &walker_id, &realm, offset(here, 1.0, 1.0)).await?;
    expect_sees(&ctx, &observer, "the walker", &wallet, SOON).await?;

    let duration = ctx.stack().limits.max_emote_duration_ms.min(30_000);
    let (first_mask, second_mask) = (0b101, 0b010);
    let current = Mutex::new(offset(here, 1.0, 1.0));
    let updates_before = {
        let view = observer.view();
        view.deltas + view.full_states
    };

    let observed = || {
        let view = observer.view();
        format!(
            "emote {:?}, starts and stops {:?}",
            view.of(&wallet).map(|subject| subject.emote.clone()),
            (view.emote_starts, view.emote_stops)
        )
    };
    let observer_holds = |emote: Option<&str>, mask: Option<i32>, starts: u32, stops: u32| {
        let view = observer.view();
        (view.emote_starts, view.emote_stops) == (starts, stops)
            && view.of(&wallet).is_some_and(|subject| {
                subject.emote.as_deref() == emote && subject.emote_mask == mask
            })
    };
    let lifecycle = async {
        let state = messages::state_at(&grid, *current.lock());
        walker.send(
            messages::emote_start("wave", Some(duration), state, Some(first_mask)),
            Channel::Reliable,
        );
        expect_or(
            &ctx,
            "the observer sees the emote start with its mask",
            SOON,
            || {
                observer.alive()?;
                observer_holds(Some("wave"), Some(first_mask), 1, 0)
                    .then_some(())
                    .ok_or_else(observed)
            },
        )
        .await?;

        // a second start while the first plays replaces it: a start for the new one, no stop
        let state = messages::state_at(&grid, *current.lock());
        walker.send(
            messages::emote_start("clap", Some(duration), state, Some(second_mask)),
            Channel::Reliable,
        );
        expect_or(
            &ctx,
            "the observer sees the restart with the new mask",
            SOON,
            || {
                observer.alive()?;
                observer_holds(Some("clap"), Some(second_mask), 2, 0)
                    .then_some(())
                    .ok_or_else(observed)
            },
        )
        .await?;

        walker.send(messages::emote_stop(), Channel::Reliable);
        expect_or(
            &ctx,
            "the observer sees the stop and the emote cleared",
            SOON,
            || {
                observer.alive()?;
                observer_holds(None, None, 2, 1)
                    .then_some(())
                    .ok_or_else(observed)
            },
        )
        .await?;

        // nothing arrives twice
        holds_for(
            &ctx,
            "each emote event arrived once",
            Duration::from_millis(400),
            || {
                observer.alive()?;
                if observer_holds(None, None, 2, 1) {
                    Ok(())
                } else {
                    Err(observed())
                }
            },
        )
        .await
    };
    tokio::select! {
        biased;
        done = lifecycle => done?,
        stopped = walk_forever(&walker, here, &current) => return stopped,
    }

    // the walker was moving all along, and the observer followed it
    let view = observer.view();
    let received = (view.deltas + view.full_states).saturating_sub(updates_before);
    ctx.note("observer_updates_during_lifecycle", received);
    if received == 0 {
        bail!("the observer received no movement of the walker during the emote");
    }
    let spot = *current.lock();
    expect_sees_at(
        &ctx,
        &observer,
        "the walker where it moved to",
        &wallet,
        spot,
        3.0,
        SOON,
    )
    .await?;
    Ok(())
}

// ---------------------------------------------------------------------------------------------
// pulse-resync-requests

async fn resync_requests(ctx: Ctx) -> Result<()> {
    let realm = ctx.realm("arena");
    let here = meeting_place(&ctx);
    let (asker_id, subject_id, bystander_id) = (ctx.identity(0), ctx.identity(1), ctx.identity(2));
    let wallet = subject_id.address();
    let asker = enter_at(&ctx, "asker", &asker_id, &realm, offset(here, 0.0, 2.0)).await?;
    let subject = enter_at(&ctx, "subject", &subject_id, &realm, offset(here, 1.0, 1.0)).await?;
    let bystander = enter_at(
        &ctx,
        "bystander",
        &bystander_id,
        &realm,
        offset(here, 2.0, 2.0),
    )
    .await?;
    while_alive(&[&asker, &subject, &bystander], async {
        let held = expect_sees(&ctx, &asker, "the subject", &wallet, SOON).await?;
        expect_sees(&ctx, &bystander, "the subject", &wallet, SOON).await?;

        let series = "pulse_resync_seq_gap_count";
        let before = metric_total(&ctx, series).await;

        // the subject's id and the sequence the asker holds of it, then four claims about what the
        // asker knows, and one request for somebody who is not there
        let id = held.subject_id;
        let sequence = held.sequence;
        let requests: [(&str, u32, u32); 5] = [
            ("valid", id, sequence),
            ("stale", id, sequence.saturating_sub(100)),
            ("future", id, sequence.saturating_add(1000)),
            ("absurd", id, u32::MAX),
            ("unknown-subject", id.wrapping_add(50_000), 0),
        ];
        let mut spot = offset(here, 1.0, 1.0);
        for (label, requested, known) in requests {
            asker.send(messages::resync(requested, known), Channel::Reliable);
            // the subject keeps walking: the asker's picture must stay correct whatever it was sent
            let next = offset(spot, 3.0, 0.0);
            walk(&subject, spot, next, 6).await?;
            spot = next;
            expect_sees_at(
                &ctx,
                &asker,
                &format!("the subject after a {label} request"),
                &wallet,
                spot,
                QUANTIZATION,
                SOON,
            )
            .await
            .with_context(|| format!("after a {label} resync request"))?;
            stay_connected(
                &ctx,
                &format!("every player after a {label} resync request"),
                &[&asker, &subject, &bystander],
                Duration::from_millis(100),
            )
            .await?;
            // walk back so the next request starts near the same place
            let back = offset(spot, -3.0, 0.0);
            walk(&subject, spot, back, 6).await?;
            spot = back;
        }

        expect_sees_at(
            &ctx,
            &bystander,
            "the subject for the bystander",
            &wallet,
            spot,
            QUANTIZATION,
            SOON,
        )
        .await?;
        if let (Some(before), Some(after)) = (before, metric_total(&ctx, series).await) {
            ctx.note("resync_metric_before", before);
            ctx.note("resync_metric_after", after);
            if after < before {
                bail!("the resync metric went down from {before} to {after}");
            }
        }
        Ok(())
    })
    .await
}

// ---------------------------------------------------------------------------------------------
// pulse-stall-and-resume

/// A stall shorter than the transport's timeout (its least is 5 s).
const SHORT_STALL: Duration = Duration::from_secs(2);
/// The transport's own longest timeout is 30 s; a stall is given this long to end the connection.
const LONG_STALL_DEADLINE: Duration = Duration::from_secs(45);

/// A walker that reaches Pulse through `link`, standing at `spot` and seen by `observer`. Each
/// stall gets a connection of its own: the transport counts how long its oldest unacknowledged
/// packet has waited, so stalls that follow one another on one connection add up.
async fn walker_behind(
    ctx: &Ctx,
    link: &FaultLink,
    observer: &PulseBot,
    realm: &str,
    index: u32,
    spot: [f32; 3],
) -> Result<(PulseBot, Identity)> {
    let identity = ctx.identity(index);
    let walker = PulseBot::dial(
        ctx,
        &format!("walker-{index}"),
        ConnectOptions {
            via: Some(link.address()),
            ..Default::default()
        },
    )
    .await?;
    walker.accepted(ctx, &identity).await?;
    walker.teleport(realm, spot)?;
    expect_sees_at(
        ctx,
        observer,
        "the walker",
        &identity.address(),
        spot,
        QUANTIZATION,
        SOON,
    )
    .await?;
    Ok((walker, identity))
}

async fn stall_and_resume(ctx: Ctx) -> Result<()> {
    let realm = ctx.realm("arena");
    let here = meeting_place(&ctx);
    let observer = enter_at(
        &ctx,
        "observer",
        &ctx.identity(0),
        &realm,
        offset(here, 0.0, 2.0),
    )
    .await?;
    let link = FaultLink::udp(
        &ctx,
        "pulse-link",
        "127.0.0.1:0".parse()?,
        ctx.stack().pulse_enet,
    )
    .await?;

    // stalls shorter than the timeout: the session lives and the observer sees the walker move
    for (number, direction) in [Direction::Uplink, Direction::Downlink, Direction::Both]
        .into_iter()
        .enumerate()
    {
        let spot = offset(here, 1.0, 1.0);
        let (walker, identity) =
            walker_behind(&ctx, &link, &observer, &realm, 1 + number as u32, spot).await?;
        link.set(direction, Fault::Stall);
        stay_connected(
            &ctx,
            &format!(
                "the walker survives a {} s stall ({direction:?})",
                SHORT_STALL.as_secs()
            ),
            &[&walker, &observer],
            SHORT_STALL,
        )
        .await?;
        link.restore();
        let next = offset(spot, 4.0, 0.0);
        walk(&walker, spot, next, 6).await?;
        expect_sees_at(
            &ctx,
            &observer,
            &format!("the walker moving again after the {direction:?} stall"),
            &identity.address(),
            next,
            QUANTIZATION,
            SOON,
        )
        .await?;
        leave(&ctx, walker).await?;
        expect_gone(
            &ctx,
            &observer,
            "the walker that left",
            &identity.address(),
            SOON,
        )
        .await?;
    }

    // a stall past the timeout: a clean end of the connection, then a new handshake is accepted
    let (walker, identity) =
        walker_behind(&ctx, &link, &observer, &realm, 10, offset(here, 1.0, 1.0)).await?;
    let wallet = identity.address();
    let stalled_at = Instant::now();
    link.set(Direction::Both, Fault::Stall);
    let code = expect_or(
        &ctx,
        "the walker's connection ends within the transport timeout",
        LONG_STALL_DEADLINE,
        || {
            walker
                .view()
                .disconnect
                .ok_or_else(|| "still connected".to_owned())
        },
    )
    .await;
    // the link is passing again whatever happened, so the connection can end its own way
    ctx.note(
        "transport_timeout_ms",
        stalled_at.elapsed().as_millis() as u64,
    );
    let code = code?;
    if code != 0 {
        bail!(
            "a timed-out connection must end with no code, got {}",
            messages::reason(code)
        );
    }
    // the server drops the silent session as well
    expect_gone(
        &ctx,
        &observer,
        "the walker after the long stall",
        &wallet,
        LONG_STALL_DEADLINE,
    )
    .await?;
    link.restore();

    let again = PulseBot::enter(&ctx, "walker-again", &identity).await?;
    let back = offset(here, 1.0, 3.0);
    again.teleport(&realm, back)?;
    expect_sees_at(
        &ctx,
        &observer,
        "the walker after its new handshake",
        &wallet,
        back,
        QUANTIZATION,
        SOON,
    )
    .await?;
    stay_connected(
        &ctx,
        "the new connection",
        &[&again, &observer],
        Duration::from_millis(300),
    )
    .await
}

// ---------------------------------------------------------------------------------------------
// pulse-scene-listener

async fn scene_listener(ctx: Ctx) -> Result<()> {
    let grid = ctx.stack().grid;
    let listener_wallet = listener_identity(&ctx)?;
    let (realm, elsewhere) = (ctx.realm("scene"), ctx.realm("elsewhere"));
    // The area of interest is the four parcels from (0, 0) to (1, 1); the outside player stands
    // in parcel (4, 0). A listener is exact by parcel, so the distance between players does not
    // matter to it; the witnesses are regular players, who see within 200 m.
    let inside_1 = grid.parcel_centre(0, 0);
    let inside_2 = grid.parcel_centre(1, 1);
    let outside = grid.parcel_centre(4, 0);
    let area = |max: (i32, i32)| messages::area_of_interest(&realm, (0, 0), max);

    let ids: Vec<Identity> = (0..6).map(|index| ctx.identity(index)).collect();
    let (witness_id, other_witness_id) = (&ids[0], &ids[1]);
    let (first_id, second_id, outside_id, away_id) = (&ids[2], &ids[3], &ids[4], &ids[5]);
    ctx.record(
        "scenario",
        "wallets",
        serde_json::json!({
            "witness": witness_id.address(), "other-witness": other_witness_id.address(),
            "inside-1": first_id.address(), "inside-2": second_id.address(),
            "outsider": outside_id.address(), "away": away_id.address(),
        }),
    );

    // the witness stands in parcel (4, 3): in neither rectangle the listener is given
    let witness = enter_at(
        &ctx,
        "witness",
        witness_id,
        &realm,
        grid.parcel_centre(4, 3),
    )
    .await?;
    let other_witness = enter_at(
        &ctx,
        "other-witness",
        other_witness_id,
        &elsewhere,
        offset(inside_1, 1.0, 0.0),
    )
    .await?;
    let outsider = enter_at(&ctx, "outsider", outside_id, &realm, outside).await?;
    let away = enter_at(&ctx, "away", away_id, &elsewhere, inside_1).await?;
    // the server has processed both before anyone moves in
    expect_sees(&ctx, &witness, "the outsider", &outside_id.address(), SOON).await?;
    expect_sees(
        &ctx,
        &other_witness,
        "the player in the other realm",
        &away_id.address(),
        SOON,
    )
    .await?;

    while_alive(&[&witness, &other_witness, &outsider, &away], async {
        let listener = dial(&ctx, "listener").await?;
        listener.send_listener_handshake(&listener_wallet, vec![area((1, 1))]);
        match listener.handshake_outcome(&ctx, Some(SOON)).await? {
            HandshakeOutcome::Accepted => {}
            other => {
                bail!(
                    "the scene listener of the stack file's wallet must be accepted, got {other:?}"
                )
            }
        }

        let first = enter_at(&ctx, "inside-1", first_id, &realm, inside_1).await?;
        let second = enter_at(&ctx, "inside-2", second_id, &realm, inside_2).await?;
        let inside_wallets = sorted_wallets(&[first_id, second_id]);
        expect_roster(&ctx, &listener, &inside_wallets, SOON).await?;
        let heard = expect_sees_at(
            &ctx,
            &listener,
            "the first player inside",
            &first_id.address(),
            inside_1,
            QUANTIZATION,
            SOON,
        )
        .await?;
        if heard.realm != realm {
            bail!(
                "the listener holds the first player in realm {:?}, expected {realm:?}",
                heard.realm
            );
        }
        // nobody outside the area or in another realm reaches it
        holds_for(
            &ctx,
            "the listener hears only the players inside",
            Duration::from_secs(1),
            || {
                listener.alive()?;
                let held = listener.view().wallets();
                if held == inside_wallets {
                    Ok(())
                } else {
                    Err(format!("the listener holds {held:?}"))
                }
            },
        )
        .await?;

        // a profile announcement reaches a regular player but never the listener
        first.send(messages::profile_announcement(7), Channel::Reliable);
        expect_or(
            &ctx,
            "a regular player receives the profile announcement",
            SOON,
            || {
                witness.alive()?;
                witness
                    .view()
                    .of(&first_id.address())
                    .is_some_and(|subject| subject.profile_version == 7)
                    .then_some(())
                    .ok_or_else(|| "not announced yet".to_owned())
            },
        )
        .await?;
        holds_for(
            &ctx,
            "the listener receives no profile announcement",
            Duration::from_millis(500),
            || {
                listener.alive()?;
                match listener.view().profile_announcements {
                    0 => Ok(()),
                    count => Err(format!("{count} announcements reached the listener")),
                }
            },
        )
        .await?;

        // what it hears follows the players
        let moved = offset(inside_1, 3.0, 0.0);
        walk(&first, inside_1, moved, 8).await?;
        expect_sees_at(
            &ctx,
            &listener,
            "the first player after it moved",
            &first_id.address(),
            moved,
            QUANTIZATION,
            SOON,
        )
        .await?;

        // an update of the area takes effect: parcels (0, 0) to (4, 0) hold the first player and
        // the outsider now, and no longer the second player, who stands in parcel (1, 1); the one
        // in the other realm stays out
        listener.send(
            messages::scene_listener_update(vec![area((4, 0))]),
            Channel::Reliable,
        );
        let wanted = sorted_wallets(&[first_id, outside_id]);
        expect_roster(&ctx, &listener, &wanted, Duration::from_secs(10)).await?;

        // an invalid area gets its code: at the handshake, and as an update
        let bad = dial(&ctx, "listener-with-invalid-area").await?;
        bad.send_listener_handshake(
            &listener_wallet,
            vec![messages::area_of_interest(&realm, (1, 1), (0, 0))],
        );
        let expected = DisconnectReason::InvalidHandshakeField.code();
        let outcome = bad.handshake_outcome(&ctx, Some(SOON)).await?;
        if outcome != (HandshakeOutcome::Disconnected { code: expected }) {
            bail!(
                "a listener handshake with an invalid area must end with {}, got {outcome:?}",
                messages::reason(expected)
            );
        }
        listener.send(
            messages::scene_listener_update(vec![messages::area_of_interest(
                &realm,
                (1, 1),
                (0, 0),
            )]),
            Channel::Reliable,
        );
        listener
            .expect_disconnect_with(&ctx, DisconnectReason::InvalidSceneListenerField, SOON)
            .await
            .context(
                "an update with an invalid area ends the listener with InvalidSceneListenerField",
            )?;
        stay_connected(
            &ctx,
            "the players",
            &[&first, &second, &outsider, &away, &witness, &other_witness],
            Duration::from_millis(200),
        )
        .await
    })
    .await
}
