//! Scenarios with the headless explorer as the subject: the real client behind the fault link,
//! watched by bots.

use std::time::{Duration, Instant};

use anyhow::{Result, anyhow, bail};
use catalyst_comms::pulse::hardening::DisconnectReason;
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::archipelago::{ArchipelagoBot, ArchipelagoOptions, socket_url};
use crate::engine::holds_for_async;
use crate::engine::{Ctx, Needs, Scenario, expect_async, expect_or};
use crate::explorer::{Explorer, ExplorerOptions, Fronts};
use crate::fault::{Direction, Fault};
use crate::identity::Identity;
use crate::pulse::{ConnectOptions, HandshakeOptions, HandshakeOutcome, PulseBot};
use crate::scenarios::connections_support::{
    RealmDescription, api, first_assignment, realm_description,
};

/// The scenarios of this group, in the order a run takes them.
pub fn all() -> Vec<&'static Scenario> {
    vec![
        &UPLINK_STALL,
        &OUT_OF_GRID,
        &PAST_THE_EAST_EDGE,
        &STALE_SOCKET,
        &PORT_UNREACHABLE,
        &TWO_DEVICES,
        &RESYNC,
        &REPLACED_BY_ANOTHER_DEVICE,
        &REPLACES_ANOTHER_DEVICE,
        &LONG_UPLINK_STALL,
        &RECONNECT_OUTSIDE_GRID,
    ]
    .into_iter()
    .chain(crate::scenarios::explorer_uplink::all())
    .collect()
}

pub(super) const NEEDS: Needs = Needs {
    api: true,
    explorer: true,
    ..Needs::NONE
};

static UPLINK_STALL: Scenario = Scenario {
    name: "explorer-uplink-stall",
    tags: &["explorer", "battle", "faults"],
    guards: &["dcl-one #1208"],
    summary: "The uplink to Pulse stalls while the explorer teleports; when it returns, the \
              teleports it resends at once must not end the session.",
    needs: NEEDS,
    hostile: false,
    default_repeat: 1,
    run: |ctx| Box::pin(uplink_stall(ctx)),
};

static OUT_OF_GRID: Scenario = Scenario {
    name: "explorer-out-of-grid",
    tags: &["explorer", "battle"],
    guards: &["dcl-one #1205"],
    summary: "The explorer walks off the north edge of the parcel grid, where no parcel index \
              is valid, and comes back: Pulse stays connected and the observer sees it where it \
              re-entered.",
    needs: NEEDS,
    hostile: false,
    default_repeat: 1,
    run: |ctx| Box::pin(out_of_grid(ctx)),
};

static PAST_THE_EAST_EDGE: Scenario = Scenario {
    name: "explorer-past-the-east-edge",
    tags: &["explorer", "battle"],
    guards: &["dcl-one #1205"],
    summary: "The explorer walks off the east edge of the parcel grid, where the parcel index \
              wraps into the next row: it must not appear on the far side of the map, and the \
              observer beside it must not see it leave.",
    needs: NEEDS,
    hostile: false,
    default_repeat: 1,
    run: |ctx| Box::pin(past_the_east_edge(ctx)),
};

static STALE_SOCKET: Scenario = Scenario {
    name: "explorer-sleep-wake",
    tags: &["explorer", "battle", "faults"],
    guards: &["dcl-one #1205"],
    summary: "The explorer loses its archipelago socket while the server still holds it, as \
              after sleep; it must reconnect and keep its island.",
    needs: NEEDS,
    hostile: false,
    default_repeat: 1,
    run: |ctx| Box::pin(stale_socket(ctx)),
};

static PORT_UNREACHABLE: Scenario = Scenario {
    name: "explorer-port-unreachable",
    tags: &["explorer", "battle", "faults"],
    guards: &["dcl-one #1205"],
    summary: "The Pulse port answers unreachable for a moment: a socket error, not the end of \
              the session. The explorer keeps its session without a new handshake.",
    needs: NEEDS,
    hostile: false,
    default_repeat: 1,
    run: |ctx| Box::pin(port_unreachable(ctx)),
};

static TWO_DEVICES: Scenario = Scenario {
    name: "explorer-two-devices",
    tags: &["explorer", "races"],
    guards: &["dcl-one #1205", "dcl-one #201"],
    summary: "A second explorer signs in with the wallet and the signing key of the first (a \
              seeded guest signs with its wallet key), which the servers see as the same device \
              signing in again: the second works, and the first is told it was replaced on Pulse \
              and on the archipelago and stops for good, without dialling again.",
    needs: NEEDS,
    hostile: false,
    default_repeat: 1,
    run: |ctx| Box::pin(two_devices(ctx)),
};

static RESYNC: Scenario = Scenario {
    name: "explorer-resync",
    tags: &["explorer", "faults"],
    guards: &["dcl-one #769", "dcl-one #780"],
    summary: "Packets from Pulse reach the explorer out of order while a bot walks beside it: \
              the explorer's picture of the bot catches up once the link is clean.",
    needs: NEEDS,
    hostile: false,
    default_repeat: 1,
    run: |ctx| Box::pin(resync(ctx)),
};

static REPLACED_BY_ANOTHER_DEVICE: Scenario = Scenario {
    name: "explorer-replaced-by-another-device",
    tags: &["explorer", "probe", "races"],
    guards: &["dcl-one #1205"],
    summary: "Another device signs in with the explorer's wallet and a session key of its own, on \
              another island: the explorer is told it was replaced on Pulse, ends its island \
              connection for good and does not dial again, and the other device stays in.",
    needs: NEEDS,
    hostile: false,
    default_repeat: 1,
    run: |ctx| Box::pin(replaced_by_another_device(ctx)),
};

static REPLACES_ANOTHER_DEVICE: Scenario = Scenario {
    name: "explorer-replaces-another-device",
    tags: &["explorer", "probe", "races"],
    guards: &["dcl-one #1205"],
    summary: "Another device with the explorer's wallet and a session key of its own is signed \
              in first; the explorer then gets in world, connected to its island and to Pulse, \
              and the other device's Pulse session ends as a duplicate.",
    needs: NEEDS,
    hostile: false,
    default_repeat: 1,
    run: |ctx| Box::pin(replaces_another_device(ctx)),
};

static LONG_UPLINK_STALL: Scenario = Scenario {
    name: "explorer-long-uplink-stall",
    tags: &["explorer", "probe", "faults"],
    guards: &["dcl-one #1208"],
    summary: "The uplink to Pulse is dead for longer than the transport waits while the explorer \
              teleports: once it returns the explorer is authenticated again, kept or new, never \
              ended for good, and the observer sees it where it last teleported.",
    needs: NEEDS,
    hostile: false,
    default_repeat: 1,
    run: |ctx| Box::pin(long_uplink_stall(ctx)),
};

static RECONNECT_OUTSIDE_GRID: Scenario = Scenario {
    name: "explorer-reconnect-outside-grid",
    tags: &["explorer", "probe", "faults"],
    guards: &["dcl-one #1205"],
    summary: "The explorer's Pulse session breaks while it stands north of the parcel grid, so \
              it has to connect again from there; back inside it is authenticated and the \
              observer sees it where it re-entered, and it is never ended for good.",
    needs: NEEDS,
    hostile: false,
    default_repeat: 1,
    run: |ctx| Box::pin(reconnect_outside_grid(ctx)),
};

const SOON: Duration = Duration::from_secs(15);
/// How long the link to Pulse is dead in the probes that need the transport to give up: past the
/// five seconds after which the transport ends a connection that no longer answers.
const LONG_OUTAGE: Duration = Duration::from_secs(8);
const CONSOLE_TICK: Duration = Duration::from_millis(200);

/// The explorer under test, the links in front of it and what it says about itself.
pub(super) struct Subject {
    pub(super) fronts: Fronts,
    pub(super) explorer: Explorer,
    pub(super) wallet: String,
    /// The realm the explorer announces to Pulse.
    pub(super) realm: String,
    pub(super) position: [f32; 3],
}

/// A wallet seed no other run, scenario or repeat uses: explorers all enter the realm the stack
/// names, so only their wallets keep runs apart.
fn wallet_seed(ctx: &Ctx, index: u32) -> u64 {
    let digest = Sha256::digest(format!("{}/{}/{index}", ctx.realm("explorer"), ctx.seed()));
    u64::from_be_bytes(digest[..8].try_into().expect("eight bytes")) | 1
}

#[allow(
    clippy::cast_possible_truncation,
    reason = "positions are metres across the map; f32 is the protocol type"
)]
fn vector(value: &Value) -> Option<[f32; 3]> {
    let list = value.as_array()?;
    Some([
        list.first()?.as_f64()? as f32,
        list.get(1)?.as_f64()? as f32,
        list.get(2)?.as_f64()? as f32,
    ])
}

pub(super) async fn start(ctx: &Ctx, preview: bool) -> Result<Subject> {
    start_seeded(ctx, preview, wallet_seed(ctx, 0)).await
}

/// [`start`] with the wallet seed given.
async fn start_seeded(ctx: &Ctx, preview: bool, seed: u64) -> Result<Subject> {
    let fronts = Fronts::start(ctx).await?;
    let mut extra = Vec::new();
    if preview {
        // preview mode registers the commands that move the player directly
        extra.push("--preview".to_owned());
    }
    let explorer = Explorer::start(
        ctx,
        "explorer",
        ExplorerOptions {
            wallet_seed: seed,
            extra,
            ..ExplorerOptions::default()
        },
    )
    .await?;
    let waited = expect_async(
        ctx,
        "the explorer is in the world with a wallet and a realm",
        Duration::from_secs(120),
        Duration::from_millis(500),
        || async {
            let state = explorer.probe("/world_state").await?;
            let ready = state["inWorld"] == true
                && state["address"].is_string()
                && state["realmName"].is_string()
                && vector(&state["position"]).is_some();
            if ready {
                Ok(state)
            } else {
                Err(format!(
                    "inWorld={} realmConnected={} address={}",
                    state["inWorld"], state["realmConnected"], state["address"]
                ))
            }
        },
    )
    .await;
    let world = match waited {
        Ok(world) => world,
        Err(error) => {
            // say what was missing: the explorer's own last word, and what the realm description
            // answers when this scenario asks for it, directly and through the explorer's front
            let state = explorer
                .probe("/world_state")
                .await
                .map_or_else(|reason| reason, |state| state.to_string());
            let direct = match ctx.stack().api.as_ref() {
                Some(api) => describe(realm_description(api).await),
                None => "the stack file has no API address".to_owned(),
            };
            let through_front = match format!("http://{}/", fronts.api.address()).parse() {
                Ok(front) => describe(realm_description(&front).await),
                Err(error) => format!("no front address: {error}"),
            };
            bail!(
                "{error:#}; the explorer's last /world_state: {state}; the realm description fetched by the scenario: directly {direct}, through the explorer's front {through_front}"
            );
        }
    };
    let subject = Subject {
        fronts,
        wallet: world["address"].as_str().unwrap_or_default().to_lowercase(),
        realm: world["realmName"].as_str().unwrap_or_default().to_owned(),
        position: vector(&world["position"]).unwrap_or_default(),
        explorer,
    };
    ctx.record(
        "explorer",
        "in world",
        serde_json::json!({ "wallet": subject.wallet, "realm": subject.realm, "position": subject.position }),
    );
    expect_pulse(
        ctx,
        &subject.explorer,
        "the explorer authenticates with Pulse",
        Duration::from_secs(60),
    )
    .await?;
    Ok(subject)
}

/// A realm description fetch, as text for a failure message.
fn describe(fetched: Result<RealmDescription, String>) -> String {
    match fetched {
        Ok(description) => format!("answers 200, adapter {}", description.adapter),
        Err(reason) => reason,
    }
}

/// The explorer's own word on its Pulse connection: `authenticated`, `terminal disconnect`,
/// `waiting`, `connecting` and so on. Polled quietly; the callers record what matters.
async fn pulse_state(explorer: &Explorer) -> Result<String, String> {
    let state = explorer.poll("/pulse_state").await?;
    Ok(state["state"].as_str().unwrap_or("unknown").to_owned())
}

/// Waits until the explorer says Pulse is authenticated. Its terminal state ends the wait at
/// once: nothing brings a session back from it but a change of realm.
async fn expect_pulse(
    ctx: &Ctx,
    explorer: &Explorer,
    what: &str,
    deadline: Duration,
) -> Result<()> {
    expect_async(ctx, what, deadline, CONSOLE_TICK, || async {
        match pulse_state(explorer).await?.as_str() {
            "authenticated" => Ok(()),
            "terminal disconnect" => {
                Err("fatal: the explorer reports a terminal disconnect: Pulse is over until the realm changes".to_owned())
            }
            other => Err(format!("the explorer reports Pulse as {other}")),
        }
    })
    .await
}

/// Checks that the explorer keeps saying Pulse is authenticated for the whole window.
async fn pulse_stays(ctx: &Ctx, explorer: &Explorer, what: &str, window: Duration) -> Result<()> {
    holds_for_async(ctx, what, window, CONSOLE_TICK, || async {
        match pulse_state(explorer).await?.as_str() {
            "authenticated" => Ok(()),
            other => Err(format!("the explorer reports Pulse as {other}")),
        }
    })
    .await
}

/// A bot standing next to the explorer, in its realm, that has seen it.
pub(super) async fn observer(ctx: &Ctx, subject: &Subject) -> Result<PulseBot> {
    let bot = PulseBot::enter(ctx, "observer", &ctx.identity(0)).await?;
    let beside = [subject.position[0] + 2.0, 0.0, subject.position[2] + 2.0];
    bot.teleport(&subject.realm, beside)?;
    let wallet = subject.wallet.clone();
    expect_or(ctx, "the observer sees the explorer", SOON, || {
        bot.alive()?;
        bot.view()
            .of(&wallet)
            .map(|_| ())
            .ok_or_else(|| format!("the observer holds {:?}", bot.view().wallets()))
    })
    .await?;
    Ok(bot)
}

async fn move_to(explorer: &Explorer, world: [f32; 3]) -> Result<()> {
    let line = format!("/move_player_to {} {} {}", world[0], world[1], world[2]);
    let answer = explorer.command(&line, Duration::from_secs(5)).await?;
    if !answer.ok {
        bail!("the explorer refused `{line}`: {}", answer.output);
    }
    Ok(())
}

/// Moves the explorer and waits until it says it stands there: a move the explorer clamped or
/// ignored would leave a scenario about a position it never took proving nothing.
async fn move_and_confirm(
    ctx: &Ctx,
    explorer: &Explorer,
    world: [f32; 3],
    what: &str,
) -> Result<()> {
    move_to(explorer, world).await?;
    expect_async(ctx, what, SOON, CONSOLE_TICK, || async {
        let state = explorer.poll("/world_state").await?;
        let position =
            vector(&state["position"]).ok_or_else(|| format!("no position in {state}"))?;
        let near = (position[0] - world[0]).abs() < 1.0 && (position[2] - world[2]).abs() < 1.0;
        near.then_some(())
            .ok_or_else(|| format!("it stands at {position:?}, not at {world:?}"))
    })
    .await
}

/// Waits until the observer holds the explorer within half a metre of `world`.
async fn observer_sees_at(
    ctx: &Ctx,
    bot: &PulseBot,
    wallet: &str,
    world: [f32; 3],
    what: &str,
) -> Result<()> {
    expect_or(ctx, what, SOON, || {
        bot.alive()?;
        let seen = bot
            .view()
            .of(wallet)
            .map(|subject| subject.world)
            .ok_or("the observer no longer holds the explorer")?;
        let near = (seen[0] - world[0]).abs() < 0.5 && (seen[2] - world[2]).abs() < 0.5;
        near.then_some(())
            .ok_or_else(|| format!("the observer holds it at {seen:?}"))
    })
    .await
}

async fn uplink_stall(ctx: Ctx) -> Result<()> {
    let subject = start(&ctx, true).await?;
    let bot = observer(&ctx, &subject).await?;
    let limits = ctx.stack().limits;
    let home = subject.position;
    let away = [home[0] + 6.0, home[1], home[2]];
    let wallet = subject.wallet.clone();

    // First, with a healthy link: a console move reaches the observer as a teleport. Without
    // this the rest would prove nothing.
    let before = bot.view().teleports;
    move_to(&subject.explorer, away).await?;
    move_to(&subject.explorer, home).await?;
    expect_or(
        &ctx,
        "the observer sees each console move as a teleport",
        SOON,
        || {
            bot.alive()?;
            let seen = bot.view().teleports - before;
            (seen >= 2)
                .then_some(())
                .ok_or_else(|| format!("{seen} teleports seen for 2 moves"))
        },
    )
    .await?;
    let teleports_before = bot.view().teleports;

    // Each move is a teleport: a reliable message the explorer keeps until Pulse acknowledges it.
    // With the uplink stalled none is acknowledged, so they pile up on the explorer's side. The
    // stall is short: well under what either end waits before it gives the connection up.
    let dropped_before = subject.fronts.pulse.counters().dropped;
    subject.fronts.pulse.set(Direction::Uplink, Fault::Stall);
    let stall = Duration::from_millis(2000);
    let started = Instant::now();
    let mut moves = 0u32;
    while started.elapsed() < stall {
        let target = if moves.is_multiple_of(2) { away } else { home };
        let line = format!("/move_player_to {} {} {}", target[0], target[1], target[2]);
        subject.explorer.fire(&line).await?;
        moves += 1;
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    let last = if moves % 2 == 1 { away } else { home };
    subject.fronts.pulse.restore();
    let dropped = subject.fronts.pulse.counters().dropped - dropped_before;
    ctx.note("moves asked during the stall", moves);
    ctx.note("server discrete burst", limits.discrete_burst);
    ctx.note("packets dropped by the stall", dropped);
    // The fault must have touched something, or the rest proves nothing: the stall dropped the
    // explorer's packets, and it asked for more teleports than the server's burst allows, so the
    // resend after the stall can overrun it.
    if dropped == 0 {
        bail!("the stall dropped no packet of the explorer: the fault touched nothing");
    }
    if moves <= limits.discrete_burst {
        bail!(
            "only {moves} moves were asked during the stall, not more than the server's discrete burst of {}: the resend cannot overrun it",
            limits.discrete_burst
        );
    }

    // What was kept is resent when the uplink returns. The session must outlive that: not ended
    // for good by the server, and not dropped and dialled again either.
    let verdict = async {
        expect_pulse(
            &ctx,
            &subject.explorer,
            "Pulse is authenticated after the stall",
            Duration::from_secs(20),
        )
        .await?;
        pulse_stays(
            &ctx,
            &subject.explorer,
            "Pulse stays authenticated after the resend",
            Duration::from_secs(12),
        )
        .await
    }
    .await;
    let view = bot.view();
    ctx.note(
        "teleports the observer saw from the stall",
        view.teleports - teleports_before,
    );
    ctx.note(
        "explorer log, disconnects",
        subject.explorer.log_lines("pulse: disconnected"),
    );
    verdict?;
    if view.count_of(&wallet) != 1 {
        bail!(
            "after the stall the observer holds {} subjects for the explorer, expected 1",
            view.count_of(&wallet)
        );
    }
    observer_sees_at(
        &ctx,
        &bot,
        &subject.wallet,
        last,
        "the observer sees the explorer where it last teleported",
    )
    .await?;
    subject.explorer.stop().await;
    Ok(())
}

async fn out_of_grid(ctx: Ctx) -> Result<()> {
    let subject = start(&ctx, true).await?;
    let bot = observer(&ctx, &subject).await?;
    let grid = ctx.stack().grid;
    let home = subject.position;

    // Two parcels past the padded north edge: the index a client computes there is past the end
    // of the grid, which the server refuses for good.
    let outside = [
        home[0],
        0.0,
        crate::casts::f32_from_i32(grid.max_parcel_z + grid.padding + 2)
            * crate::casts::f32_from_i32(grid.parcel_size)
            + 8.0,
    ];
    if grid.locate(outside).is_some() {
        bail!(
            "{outside:?} is inside the grid of this stack; the scenario needs a position outside"
        );
    }
    move_and_confirm(
        &ctx,
        &subject.explorer,
        outside,
        "the explorer stands outside the grid",
    )
    .await?;
    // nobody can be told where it is, but it has not left
    holds_for_async(
        &ctx,
        "Pulse stays authenticated while the explorer is outside the grid, and the observer still holds it once",
        Duration::from_secs(8),
        CONSOLE_TICK,
        || async {
            bot.alive()?;
            let held = bot.view().count_of(&subject.wallet);
            if held != 1 {
                return Err(format!("the observer holds {held} subjects for it, expected 1"));
            }
            match pulse_state(&subject.explorer).await?.as_str() {
                "authenticated" => Ok(()),
                other => Err(format!("the explorer reports Pulse as {other}")),
            }
        },
    )
    .await?;

    let back = [home[0] + 16.0, 0.0, home[2]];
    move_to(&subject.explorer, back).await?;
    observer_sees_at(
        &ctx,
        &bot,
        &subject.wallet,
        back,
        "the observer sees the explorer where it re-entered the grid",
    )
    .await?;
    pulse_stays(
        &ctx,
        &subject.explorer,
        "Pulse stays authenticated after re-entry",
        Duration::from_secs(3),
    )
    .await?;
    subject.explorer.stop().await;
    Ok(())
}

async fn past_the_east_edge(ctx: Ctx) -> Result<()> {
    let subject = start(&ctx, true).await?;
    let bot = observer(&ctx, &subject).await?;
    let grid = ctx.stack().grid;
    let size = crate::casts::f32_from_i32(grid.parcel_size);
    let home = subject.position;

    // Two parcels past the padded east edge. A client that computes the parcel index there without
    // a bound gets a valid index: the first parcels of the next row, on the west side of the map.
    let parcel_x = grid.max_parcel_x + grid.padding + 2;
    let parcel_z = crate::casts::trunc_i32((home[2] / size).floor());
    let outside = [
        crate::casts::f32_from_i32(parcel_x) * size + 8.0,
        0.0,
        home[2],
    ];
    if grid.locate(outside).is_some() {
        bail!(
            "{outside:?} is inside the grid of this stack; the scenario needs a position outside"
        );
    }
    let encoder = grid.encoder();
    let (wrapped_x, wrapped_z) = encoder.decode(encoder.encode(parcel_x, parcel_z));
    let far_side = [
        crate::casts::f32_from_i32(wrapped_x) * size + 8.0,
        0.0,
        crate::casts::f32_from_i32(wrapped_z) * size + 8.0,
    ];
    ctx.note(
        "where an unbounded index would place the explorer",
        far_side,
    );

    // a second bot waits where the wrapped index points
    let far = PulseBot::enter(&ctx, "far observer", &ctx.identity(1)).await?;
    far.teleport(&subject.realm, [far_side[0] + 2.0, 0.0, far_side[2] + 2.0])?;

    move_and_confirm(
        &ctx,
        &subject.explorer,
        outside,
        "the explorer stands east of the grid",
    )
    .await?;
    let wallet = subject.wallet.clone();
    holds_for_async(
        &ctx,
        "nobody sees the explorer anywhere else, and the observer beside where it was still holds it once",
        Duration::from_secs(8),
        Duration::from_millis(50),
        || async {
            bot.alive().map_err(|reason| format!("the observer: {reason}"))?;
            far.alive()
                .map_err(|reason| format!("the far observer: {reason}"))?;
            if far.view().count_of(&wallet) > 0 {
                let seen = far.view().of(&wallet).map(|subject| subject.world);
                return Err(format!(
                    "the explorer stands at {outside:?}, east of the grid, and a bot on the far side of the map sees it at {seen:?}"
                ));
            }
            let held = bot.view().count_of(&wallet);
            if held != 1 {
                return Err(format!(
                    "the observer beside where the explorer was holds {held} subjects for it, expected 1 (it must not be told the explorer left)"
                ));
            }
            Ok(())
        },
    )
    .await?;
    expect_pulse(
        &ctx,
        &subject.explorer,
        "Pulse is still authenticated",
        SOON,
    )
    .await?;

    let back = [home[0] + 16.0, 0.0, home[2]];
    move_to(&subject.explorer, back).await?;
    observer_sees_at(
        &ctx,
        &bot,
        &subject.wallet,
        back,
        "the observer sees the explorer where it re-entered the grid",
    )
    .await?;
    subject.explorer.stop().await;
    Ok(())
}

/// How many island transports the explorer holds, from its own transport list.
async fn archipelago_transports(explorer: &Explorer) -> Result<Value, String> {
    explorer.probe("/transport_state").await
}

async fn stale_socket(ctx: Ctx) -> Result<()> {
    let subject = start(&ctx, false).await?;
    let before = expect_async(
        &ctx,
        "the explorer holds a transport from the archipelago",
        Duration::from_secs(60),
        Duration::from_millis(500),
        || async {
            let state = archipelago_transports(&subject.explorer).await?;
            (state["total"].as_u64().unwrap_or(0) > 0)
                .then(|| state.clone())
                .ok_or_else(|| format!("transports: {state}"))
        },
    )
    .await?;
    ctx.note("transports before", &before);
    ctx.note(
        "connectivity before",
        subject
            .explorer
            .probe("/connectivity_state")
            .await
            .unwrap_or(Value::Null),
    );

    // The client side of every connection through the API front closes; the server side stays
    // open. The explorer knows its socket is gone; the server still counts it as connected.
    // The front also carries the explorer's ordinary HTTP requests, so a new connection through
    // it proves nothing: what is asked is the explorer's own report of its realm room, which must
    // leave `connected` and come back.
    let orphaned = Instant::now();
    subject.fronts.api.orphan()?;
    let lost_after = expect_async(
        &ctx,
        "the explorer's realm room leaves the connected state after its socket is orphaned",
        Duration::from_secs(30),
        Duration::from_millis(50),
        || async {
            let connectivity = subject.explorer.poll("/connectivity_state").await?;
            if has_identity_conflict(&connectivity) {
                return Err(format!(
                    "fatal: the explorer reports an identity conflict: {connectivity}"
                ));
            }
            let state = realm_room_of(&connectivity)?;
            (state != "connected")
                .then(|| orphaned.elapsed())
                .ok_or_else(|| format!("its realm room is still connected: {connectivity}"))
        },
    )
    .await?;
    ctx.note(
        "realm room left connected after, ms",
        crate::casts::millis(lost_after),
    );

    let conflict = expect_async(
        &ctx,
        "the explorer reconnects to the archipelago: every room connected and a transport held again",
        Duration::from_secs(60),
        Duration::from_millis(250),
        || async {
            let connectivity = subject.explorer.poll("/connectivity_state").await?;
            if has_identity_conflict(&connectivity) {
                return Err(format!(
                    "fatal: the explorer reports an identity conflict: {connectivity}"
                ));
            }
            let state = subject.explorer.poll("/transport_state").await?;
            let rooms = connectivity["rooms"].as_array().cloned().unwrap_or_default();
            let all_connected =
                !rooms.is_empty() && rooms.iter().all(|room| room["state"] == "connected");
            (state["total"].as_u64().unwrap_or(0) > 0 && all_connected)
                .then_some(connectivity.clone())
                .ok_or_else(|| format!("transports: {state}, connectivity: {connectivity}"))
        },
    )
    .await?;
    ctx.note(
        "reconnected after, ms",
        crate::casts::millis(orphaned.elapsed()),
    );
    ctx.note("connectivity after", &conflict);
    let mut archipelago_log = subject.explorer.log_lines("comms::archipelago");
    let skip = archipelago_log.len().saturating_sub(6);
    ctx.note(
        "explorer log, archipelago (last lines)",
        archipelago_log.split_off(skip),
    );
    // a new island assignment through the new socket restarts the room: the log says so
    ctx.note(
        "explorer log, times the livekit protocol started",
        subject.explorer.log_count("starting livekit protocol"),
    );
    expect_pulse(
        &ctx,
        &subject.explorer,
        "Pulse is still authenticated",
        SOON,
    )
    .await?;
    subject.explorer.stop().await;
    Ok(())
}

async fn port_unreachable(ctx: Ctx) -> Result<()> {
    let subject = start(&ctx, false).await?;
    let bot = observer(&ctx, &subject).await?;
    let held = bot
        .view()
        .of(&subject.wallet)
        .map(|seen| seen.subject_id)
        .ok_or_else(|| anyhow!("the observer lost the explorer before the fault"))?;
    let (joins, leaves) = (
        bot.view().joins_of(&subject.wallet),
        bot.view().leaves_of(&subject.wallet),
    );

    // What is asserted about the fault, and what is only noted. A closed port has no socket, so
    // nothing counts the packets sent to it, and the explorer's own Pulse traffic counters count
    // application messages, not the transport's keep-alive packets. The link's own count of what
    // reached it is the signal:
    //   - asserted: before the close the explorer's packets were reaching the link, so it was
    //     sending to this address on its own;
    //   - asserted: after the reopen its packets reach the link again, within a short time.
    //   - only noted: the explorer's own log lines about a failed Pulse connection (the build
    //     before the fix logs `pulse: failed (enet service error ...)`; a fixed build logs
    //     nothing when it swallows the error), and the state it reported inside the closure.
    let up_at_start = subject.fronts.pulse.counters().up_packets;
    expect_or(
        &ctx,
        "the explorer's packets reach the Pulse front before the port closes",
        Duration::from_secs(10),
        || {
            let up = subject.fronts.pulse.counters().up_packets;
            (up > up_at_start)
                .then_some(())
                .ok_or_else(|| format!("{up} packets so far, {up_at_start} when waiting began"))
        },
    )
    .await?;

    // The port closes: this machine answers the explorer's packets "port unreachable", which its
    // socket reports as an error. Far shorter than any timeout of the transport.
    subject.fronts.pulse.close_port()?;
    tokio::time::sleep(Duration::from_millis(1500)).await;
    let state_inside = pulse_state(&subject.explorer).await;
    subject.fronts.pulse.restore();
    ctx.note(
        "the explorer's Pulse state inside the closure",
        &state_inside,
    );
    let up_at_restore = subject.fronts.pulse.counters().up_packets;

    expect_pulse(
        &ctx,
        &subject.explorer,
        "Pulse is authenticated after the port returns",
        SOON,
    )
    .await?;
    expect_or(
        &ctx,
        "the explorer's packets reach the Pulse front again after the port reopens",
        SOON,
        || {
            let up = subject.fronts.pulse.counters().up_packets;
            (up > up_at_restore)
                .then_some(())
                .ok_or_else(|| format!("{up} packets, {up_at_restore} at the reopening"))
        },
    )
    .await?;
    pulse_stays(
        &ctx,
        &subject.explorer,
        "Pulse stays authenticated",
        Duration::from_secs(5),
    )
    .await?;
    // The same session, not a new one: the observer never saw the explorer leave and join.
    let view = bot.view();
    let (joined, left) = (
        view.joins_of(&subject.wallet) - joins,
        view.leaves_of(&subject.wallet) - leaves,
    );
    ctx.note(
        "times the observer saw the explorer join and leave during the fault",
        (joined, left),
    );
    ctx.note(
        "explorer log, Pulse failures",
        subject.explorer.log_lines("pulse: failed"),
    );
    let now = view.of(&subject.wallet).map(|seen| seen.subject_id);
    if now != Some(held) || joined != 0 || left != 0 {
        bail!(
            "the explorer did not keep its session: the observer held subject {held} and now holds {now:?}, and saw the explorer leave {left} times and join {joined} times during the fault"
        );
    }
    subject.explorer.stop().await;
    Ok(())
}

/// How late a packet from Pulse may arrive in `explorer-resync`.
const REORDER_WINDOW: Duration = Duration::from_millis(150);

/// The resync requests the explorer made in the last minute, by its own count.
async fn resync_requests(explorer: &Explorer) -> Result<u64> {
    let state = explorer
        .probe("/pulse_state")
        .await
        .map_err(|error| anyhow!("{error}"))?;
    Ok(state["resyncs"].as_u64().unwrap_or(0))
}

/// The state of the realm room in a connectivity report.
fn realm_room_of(connectivity: &Value) -> Result<String, String> {
    connectivity["rooms"]
        .as_array()
        .and_then(|rooms| rooms.iter().find(|room| room["scope"] == "realm"))
        .and_then(|room| room["state"].as_str().map(str::to_owned))
        .ok_or_else(|| format!("no realm room in {connectivity}"))
}

/// The state of the explorer's realm room, from its connectivity report.
async fn realm_room(explorer: &Explorer) -> Result<String, String> {
    realm_room_of(&explorer.poll("/connectivity_state").await?)
}

/// Whether a connectivity report says another device replaced this one.
fn has_identity_conflict(connectivity: &Value) -> bool {
    let text = connectivity.to_string().to_lowercase();
    text.contains("identity_conflict")
        || text.contains("identityconflict")
        || text.contains("identity conflict")
}

async fn two_devices(ctx: Ctx) -> Result<()> {
    let first = start(&ctx, false).await?;
    expect_async(
        &ctx,
        "the first device has its realm room connected",
        Duration::from_secs(60),
        CONSOLE_TICK,
        || async {
            let state = realm_room(&first.explorer).await?;
            (state == "connected")
                .then_some(())
                .ok_or(format!("its realm room is {state}"))
        },
    )
    .await?;

    // The same wallet seed in another process. A seeded guest signs with its wallet key, so this
    // is the same wallet and the same session key: to the servers, this device signing in again.
    // Another device of the wallet (another session key) is `explorer-replaced-by-another-device`.
    let second = Explorer::start(
        &ctx,
        "second device",
        ExplorerOptions {
            wallet_seed: wallet_seed(&ctx, 0),
            ..ExplorerOptions::default()
        },
    )
    .await?;
    expect_pulse(
        &ctx,
        &second,
        "the second device authenticates with Pulse",
        Duration::from_secs(60),
    )
    .await?;

    // The first device is told it was replaced and stays down. If it dialled again it would
    // evict the second, and the two would trade the session back and forth.
    expect_async(
        &ctx,
        "the first device reports Pulse as ended for good",
        Duration::from_secs(30),
        CONSOLE_TICK,
        || async {
            let state = pulse_state(&first.explorer).await?;
            (state == "terminal disconnect")
                .then_some(())
                .ok_or(format!("it reports Pulse as {state}"))
        },
    )
    .await?;
    expect_async(
        &ctx,
        "the first device reports its realm room as an identity conflict",
        Duration::from_secs(30),
        CONSOLE_TICK,
        || async {
            let state = realm_room(&first.explorer).await?;
            (state == "identity_conflict")
                .then_some(())
                .ok_or(format!("its realm room is {state}"))
        },
    )
    .await?;
    holds_for_async(
        &ctx,
        "the first device stays down and the second stays up",
        Duration::from_secs(8),
        CONSOLE_TICK,
        || async {
            let old = pulse_state(&first.explorer).await?;
            let new = pulse_state(&second).await?;
            if old != "terminal disconnect" || new != "authenticated" {
                return Err(format!(
                    "the first reports Pulse as {old} and the second as {new}"
                ));
            }
            Ok(())
        },
    )
    .await?;
    let room = realm_room(&second)
        .await
        .map_err(|error| anyhow!("{error}"))?;
    if room != "connected" {
        bail!("the second device's realm room is {room}, expected connected");
    }
    second.stop().await;
    first.explorer.stop().await;
    Ok(())
}

async fn resync(ctx: Ctx) -> Result<()> {
    let subject = start(&ctx, false).await?;
    let walker = observer(&ctx, &subject).await?;
    let walker_wallet = ctx.identity(0).address();
    let from = [subject.position[0] + 2.0, 0.0, subject.position[2] + 2.0];

    // the explorer holds the walker before anything is disturbed
    let select = format!("/movement_profile {walker_wallet}");
    expect_async(
        &ctx,
        "the explorer holds the walker",
        SOON,
        CONSOLE_TICK,
        || async {
            let players = subject
                .explorer
                .command("/connected_players", Duration::from_secs(5))
                .await
                .map_err(|error| format!("{error:#}"))?;
            players
                .output
                .to_lowercase()
                .contains(&walker_wallet)
                .then_some(())
                .ok_or_else(|| format!("its connected players: {}", players.output))
        },
    )
    .await?;
    subject
        .explorer
        .command(&select, Duration::from_secs(5))
        .await?;

    // What Pulse sends the explorer now arrives up to the window late, each packet by its own
    // amount, so later packets overtake earlier ones while the walker moves.
    let resyncs_before = resync_requests(&subject.explorer).await?;
    subject.fronts.pulse.set(
        Direction::Downlink,
        Fault::Reorder {
            window: REORDER_WINDOW,
        },
    );
    let walk = Duration::from_secs(8);
    let started = Instant::now();
    let mut steps = 0u32;
    let mut at = from;
    while started.elapsed() < walk {
        steps += 1;
        // out and back along x, so the walker stays beside the explorer
        let phase = crate::casts::f32_from_u32(steps % 40);
        at = [
            from[0] + if phase < 20.0 { phase } else { 40.0 - phase } * 0.3,
            0.0,
            from[2],
        ];
        walker.move_to(at)?;
        walker
            .alive()
            .map_err(|reason| anyhow!("the walker: {reason}"))?;
        tokio::time::sleep(Duration::from_millis(80)).await;
    }
    subject.fronts.pulse.restore();
    ctx.note("steps walked under reordering", steps);

    // The scenario is about a link that disordered what the explorer received: a run in which the
    // explorer needed no resync proves nothing. (The count covers the last 60 s.)
    let resyncs = resync_requests(&subject.explorer).await?;
    ctx.note("resync requests the explorer made", resyncs);
    if resyncs <= resyncs_before {
        bail!(
            "the explorer made no resync request while its downlink was reordered over {steps} steps ({resyncs} in the last minute, {resyncs_before} before): the reordering did not reach it"
        );
    }

    // With the link clean and the walker standing where it stopped, the explorer's picture of it
    // must settle there. The walker sends nothing more: a client that stands still sends nothing
    // until its state changes, and what the walk sent last has already gone out. The explorer
    // reports positions in its own axes, where z points the other way: it is the negated z.
    let expected = [at[0], at[1], -at[2]];
    let seen = expect_async(
        &ctx,
        "the explorer holds the walker where it stopped",
        Duration::from_secs(15),
        CONSOLE_TICK,
        || async {
            walker.alive()?;
            let report = subject.explorer.poll("/foreign_movement").await?;
            let position =
                vector(&report["position"]).ok_or_else(|| format!("no position in {report}"))?;
            let near = (position[0] - expected[0]).abs() < 0.6
                && (position[2] - expected[2]).abs() < 0.6;
            near.then_some(position).ok_or_else(|| {
                format!(
                    "it holds the walker at {position:?}, the walker is at {at:?} (the explorer's axes: {expected:?})"
                )
            })
        },
    )
    .await?;
    ctx.note("where the explorer holds the walker", seen);
    expect_pulse(
        &ctx,
        &subject.explorer,
        "Pulse is still authenticated",
        SOON,
    )
    .await?;
    subject.explorer.stop().await;
    Ok(())
}

/// States an explorer reported, in order, each change once: what a probe saw on the way.
#[derive(Default)]
struct Trail(parking_lot::Mutex<Vec<String>>);

impl Trail {
    fn push(&self, state: &str) {
        let mut states = self.0.lock();
        if states.last().map(String::as_str) != Some(state) {
            states.push(state.to_owned());
        }
    }

    fn seen(&self, state: &str) -> bool {
        self.0.lock().iter().any(|seen| seen == state)
    }

    fn states(&self) -> Vec<String> {
        self.0.lock().clone()
    }
}

/// [`pulse_state`], kept on `trail`.
async fn pulse_state_onto(explorer: &Explorer, trail: &Trail) -> Result<String, String> {
    let state = pulse_state(explorer).await?;
    trail.push(&state);
    Ok(state)
}

/// [`realm_room`], kept on `trail`.
async fn realm_room_onto(explorer: &Explorer, trail: &Trail) -> Result<String, String> {
    let state = realm_room(explorer).await?;
    trail.push(&state);
    Ok(state)
}

/// Waits until the explorer reports its realm room as `wanted`. An identity conflict ends the
/// wait at once when it is not what is wanted: nothing brings the room back from it.
async fn expect_realm_room(
    ctx: &Ctx,
    explorer: &Explorer,
    what: &str,
    deadline: Duration,
    wanted: &str,
) -> Result<()> {
    expect_async(ctx, what, deadline, CONSOLE_TICK, || async {
        let state = realm_room(explorer).await?;
        if state == wanted {
            Ok(())
        } else if state == "identity_conflict" {
            Err(format!(
                "fatal: the explorer reports its realm room as {state}"
            ))
        } else {
            Err(format!("its realm room is {state}"))
        }
    })
    .await
}

/// Waits until the explorer reports Pulse as `terminal disconnect`.
async fn expect_terminal(ctx: &Ctx, explorer: &Explorer, what: &str) -> Result<()> {
    expect_async(ctx, what, Duration::from_secs(30), CONSOLE_TICK, || async {
        let state = pulse_state(explorer).await?;
        (state == "terminal disconnect")
            .then_some(())
            .ok_or(format!("it reports Pulse as {state}"))
    })
    .await
}

/// A spot inside the grid `distance` metres from `from` along one axis: another island.
fn far_spot(ctx: &Ctx, from: [f32; 3], distance: f32) -> Result<[f32; 3]> {
    let grid = ctx.stack().grid;
    [
        (distance, 0.0),
        (-distance, 0.0),
        (0.0, distance),
        (0.0, -distance),
    ]
    .into_iter()
    .map(|(dx, dz)| [from[0] + dx, 0.0, from[2] + dz])
    .find(|world| grid.locate(*world).is_some())
    .ok_or_else(|| anyhow!("no spot inside the grid is {distance} m from {from:?} along an axis"))
}

/// What the statistics route lists for `wallet`: each entry's island and position, or why it could
/// not be asked.
async fn listed(ctx: &Ctx, wallet: &str) -> Value {
    let peers = match api(ctx) {
        Ok(api) => crate::oracle::peers(api).await,
        Err(error) => Err(error),
    };
    match peers {
        Ok(peers) => crate::oracle::of_wallet(&peers, wallet)
            .into_iter()
            .map(|peer| serde_json::json!({ "island": peer.island, "position": peer.position }))
            .collect(),
        Err(error) => Value::String(format!("{error:#}")),
    }
}

/// The last `count` lines of the explorer's log that contain `needle`.
pub(super) fn last_log_lines(explorer: &Explorer, needle: &str, count: usize) -> Vec<String> {
    let mut lines = explorer.log_lines(needle);
    let skip = lines.len().saturating_sub(count);
    lines.split_off(skip)
}

async fn replaced_by_another_device(ctx: Ctx) -> Result<()> {
    let seed = wallet_seed(&ctx, 0);
    let subject = start_seeded(&ctx, false, seed).await?;
    expect_realm_room(
        &ctx,
        &subject.explorer,
        "the explorer has its realm room connected",
        Duration::from_secs(60),
        "connected",
    )
    .await?;

    // the explorer's wallet on another device: the wallet key, and a session key of its own
    let device = Identity::seeded_guest_wallet(seed, &ctx.realm("another device"));
    if device.address() != subject.wallet {
        bail!(
            "the wallet built from seed {seed} is {}, the explorer reports {}: the other device would not share its wallet",
            device.address(),
            subject.wallet
        );
    }
    let far = far_spot(&ctx, subject.position, 800.0)?;
    ctx.note(
        "the statistics route on the wallet before the other device",
        listed(&ctx, &subject.wallet).await,
    );

    let url = socket_url(api(&ctx)?)?;
    let archipelago = ArchipelagoBot::begin(
        &ctx,
        "other device archipelago",
        &url,
        &device,
        ArchipelagoOptions::default(),
    )
    .await?;
    ctx.note(
        "already_connected in the other device's challenge",
        archipelago.view().already_connected,
    );
    archipelago.finish(&ctx, &device).await?;
    archipelago.heartbeat(far);

    let pulse = PulseBot::dial(&ctx, "other device pulse", ConnectOptions::default()).await?;
    match pulse
        .handshake(&ctx, &device, HandshakeOptions::default())
        .await?
    {
        HandshakeOutcome::Accepted => {}
        other => bail!("the other device's Pulse handshake was not accepted: {other:?}"),
    }
    pulse.teleport(&subject.realm, far)?;
    pulse.move_to(far)?;

    let rooms = Trail::default();
    rooms.push("connected");
    let verdict = async {
        expect_terminal(
            &ctx,
            &subject.explorer,
            "the explorer reports Pulse as ended for good (terminal disconnect)",
        )
        .await?;
        expect_async(
            &ctx,
            "the explorer ends its island connection for good: its realm room reports identity_conflict",
            Duration::from_secs(30),
            CONSOLE_TICK,
            || async {
                let state = realm_room_onto(&subject.explorer, &rooms).await?;
                (state == "identity_conflict")
                    .then_some(())
                    .ok_or(format!("its realm room is {state}"))
            },
        )
        .await?;
        // The archipelago places a wallet where its freshest position says, and a position is
        // fresh for seconds: the other device keeps sending its own, as a client does.
        expect_async(
            &ctx,
            "the other device, sending its position every second, receives an island",
            Duration::from_secs(40),
            Duration::from_secs(1),
            || async {
                archipelago.heartbeat(far);
                archipelago
                    .assignments()
                    .last()
                    .map(|assignment| ctx.note("the other device's island", &assignment.island))
                    .ok_or("none yet".to_owned())
            },
        )
        .await?;
        let packets = subject.fronts.pulse.counters().up_packets;
        holds_for_async(
            &ctx,
            "the explorer stays down without dialling Pulse again and keeps its realm room in identity_conflict, while the other device stays authenticated on Pulse and welcomed on the archipelago with its island",
            Duration::from_secs(8),
            CONSOLE_TICK,
            || async {
                let state = pulse_state(&subject.explorer).await?;
                if state != "terminal disconnect" {
                    return Err(format!("the explorer reports Pulse as {state}"));
                }
                let room = realm_room_onto(&subject.explorer, &rooms).await?;
                if room != "identity_conflict" {
                    return Err(format!("the explorer's realm room is {room}"));
                }
                let sent = subject.fronts.pulse.counters().up_packets - packets;
                if sent > 0 {
                    return Err(format!("the explorer sent {sent} packets towards Pulse"));
                }
                pulse
                    .alive()
                    .map_err(|reason| format!("the other device on Pulse: {reason}"))?;
                archipelago
                    .alive()
                    .map_err(|reason| format!("the other device on the archipelago: {reason}"))?;
                if archipelago.assignments().is_empty() {
                    return Err("the other device holds no island".to_owned());
                }
                Ok(())
            },
        )
        .await
    }
    .await;
    ctx.note("the explorer's realm room states", rooms.states());
    ctx.note(
        "the statistics route on the wallet afterwards",
        listed(&ctx, &subject.wallet).await,
    );
    let view = archipelago.view();
    ctx.note(
        "the other device's archipelago socket: kicked, closed, assignments",
        (view.kicked, view.closed, view.assignments.len()),
    );
    ctx.note(
        "explorer log, archipelago (last lines)",
        last_log_lines(&subject.explorer, "comms::archipelago", 6),
    );
    ctx.note(
        "explorer log, Pulse disconnects",
        last_log_lines(&subject.explorer, "pulse: terminal", 3),
    );
    verdict?;
    subject.explorer.stop().await;
    Ok(())
}

async fn replaces_another_device(ctx: Ctx) -> Result<()> {
    let seed = wallet_seed(&ctx, 0);
    let device = Identity::seeded_guest_wallet(seed, &ctx.realm("another device"));
    let api_url = api(&ctx)?.clone();
    // the other device announces the realm the explorer will enter, where it starts
    let realm = realm_description(&api_url)
        .await
        .ok()
        .and_then(|description| description.realm_name)
        .unwrap_or_else(|| ctx.realm("explorer"));
    ctx.note("the realm the other device announces", &realm);
    let start_parcel = ExplorerOptions::default().position;
    let here = ctx
        .stack()
        .grid
        .parcel_centre(start_parcel.0, start_parcel.1);
    let beside = [here[0] + 3.0, 0.0, here[2] + 3.0];

    let archipelago = ArchipelagoBot::open(&ctx, "other device archipelago", &device).await?;
    archipelago.heartbeat(beside);
    let assignment = first_assignment(&ctx, &archipelago).await?;
    ctx.note("the other device's island", &assignment.island);
    let pulse = PulseBot::enter(&ctx, "other device pulse", &device).await?;
    pulse.teleport(&realm, beside)?;
    pulse.move_to(beside)?;

    let rooms = Trail::default();
    let started = start_seeded(&ctx, false, seed).await;
    let verdict = async {
        let subject = started?;
        if device.address() != subject.wallet {
            bail!(
                "the wallet built from seed {seed} is {}, the explorer reports {}: the other device did not share its wallet",
                device.address(),
                subject.wallet
            );
        }
        expect_async(
            &ctx,
            "the explorer has its realm room connected",
            Duration::from_secs(60),
            CONSOLE_TICK,
            || async {
                let state = realm_room_onto(&subject.explorer, &rooms).await?;
                match state.as_str() {
                    "connected" => Ok(()),
                    "identity_conflict" => Err(format!(
                        "fatal: the explorer reports its realm room as {state}"
                    )),
                    _ => Err(format!("its realm room is {state}")),
                }
            },
        )
        .await?;
        pulse
            .expect_disconnect_with(
                &ctx,
                DisconnectReason::DuplicateSession,
                Duration::from_secs(15),
            )
            .await?;
        expect_pulse(
            &ctx,
            &subject.explorer,
            "the explorer is still authenticated on Pulse",
            SOON,
        )
        .await?;
        Ok(subject)
    }
    .await;
    ctx.note("the explorer's realm room states", rooms.states());
    let view = archipelago.view();
    ctx.note(
        "the other device's archipelago socket: kicked, closed, assignments",
        (view.kicked, view.closed, view.assignments.len()),
    );
    ctx.note(
        "the other device's Pulse disconnect code",
        pulse.view().disconnect,
    );
    ctx.note(
        "the statistics route on the wallet afterwards",
        listed(&ctx, &device.address()).await,
    );
    let subject = verdict?;
    subject.explorer.stop().await;
    Ok(())
}

async fn long_uplink_stall(ctx: Ctx) -> Result<()> {
    let subject = start(&ctx, true).await?;
    let bot = observer(&ctx, &subject).await?;
    let home = subject.position;
    let away = [home[0] + 6.0, home[1], home[2]];
    let wallet = subject.wallet.clone();
    let held = bot
        .view()
        .of(&wallet)
        .map(|seen| seen.subject_id)
        .ok_or_else(|| anyhow!("the observer lost the explorer before the stall"))?;
    let (joins, leaves) = (bot.view().joins_of(&wallet), bot.view().leaves_of(&wallet));
    let accepted_before = subject.explorer.log_count("pulse: handshake accepted");

    // The uplink is dead for longer than the transport waits for an acknowledgement, while the
    // explorer is asked to teleport over and over, as in `explorer-uplink-stall`.
    let states = Trail::default();
    states.push("authenticated");
    let dropped_before = subject.fronts.pulse.counters().dropped;
    subject.fronts.pulse.set(Direction::Uplink, Fault::Stall);
    let started = Instant::now();
    let mut moves = 0u32;
    while started.elapsed() < LONG_OUTAGE {
        let target = if moves.is_multiple_of(2) { away } else { home };
        let line = format!("/move_player_to {} {} {}", target[0], target[1], target[2]);
        subject.explorer.fire(&line).await?;
        moves += 1;
        if moves.is_multiple_of(10) {
            // what it reports inside the stall; a failed look is not the explorer's answer
            if let Ok(state) = pulse_state(&subject.explorer).await {
                states.push(&state);
            }
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    let last = if moves % 2 == 1 { away } else { home };
    subject.fronts.pulse.restore();
    let dropped = subject.fronts.pulse.counters().dropped - dropped_before;
    ctx.note("stall, ms", crate::casts::millis(LONG_OUTAGE));
    ctx.note("moves asked during the stall", moves);
    ctx.note("packets dropped by the stall", dropped);
    if dropped == 0 {
        bail!("the stall dropped no packet of the explorer: the fault touched nothing");
    }

    let verdict = async {
        if states.seen("terminal disconnect") {
            bail!(
                "the explorer reported a terminal disconnect during the stall; its Pulse states: {:?}",
                states.states()
            );
        }
        expect_async(
            &ctx,
            "Pulse is authenticated again within 30 s of the uplink's return",
            Duration::from_secs(30),
            CONSOLE_TICK,
            || async {
                match pulse_state_onto(&subject.explorer, &states).await?.as_str() {
                    "authenticated" => Ok(()),
                    "terminal disconnect" => Err(
                        "fatal: the explorer reports a terminal disconnect: Pulse is over until the realm changes"
                            .to_owned(),
                    ),
                    other => Err(format!("the explorer reports Pulse as {other}")),
                }
            },
        )
        .await?;
        holds_for_async(
            &ctx,
            "the explorer does not report a terminal disconnect afterwards",
            Duration::from_secs(5),
            CONSOLE_TICK,
            || async {
                match pulse_state_onto(&subject.explorer, &states).await?.as_str() {
                    "terminal disconnect" => {
                        Err("the explorer reports a terminal disconnect".to_owned())
                    }
                    _ => Ok(()),
                }
            },
        )
        .await?;
        observer_sees_at(
            &ctx,
            &bot,
            &wallet,
            last,
            "the observer sees the explorer where it last teleported",
        )
        .await
    }
    .await;
    let view = bot.view();
    let now = view.of(&wallet).map(|seen| seen.subject_id);
    let (joined, left) = (
        view.joins_of(&wallet) - joins,
        view.leaves_of(&wallet) - leaves,
    );
    ctx.note(
        "the explorer's Pulse states from the stall on",
        states.states(),
    );
    ctx.note(
        "session after the stall",
        if now == Some(held) && joined == 0 && left == 0 {
            "kept"
        } else {
            "new"
        },
    );
    ctx.note(
        "handshakes the explorer had accepted since the stall began",
        subject.explorer.log_count("pulse: handshake accepted") - accepted_before,
    );
    ctx.note(
        "times the observer saw the explorer leave and join",
        (left, joined),
    );
    ctx.note(
        "explorer log, Pulse disconnects and failures",
        last_log_lines(&subject.explorer, "pulse: disconnected", 4)
            .into_iter()
            .chain(last_log_lines(&subject.explorer, "pulse: failed", 4))
            .collect::<Vec<_>>(),
    );
    verdict?;
    subject.explorer.stop().await;
    Ok(())
}

async fn reconnect_outside_grid(ctx: Ctx) -> Result<()> {
    let subject = start(&ctx, true).await?;
    let bot = observer(&ctx, &subject).await?;
    let grid = ctx.stack().grid;
    let home = subject.position;
    let wallet = subject.wallet.clone();

    // as in `explorer-out-of-grid`: two parcels past the padded north edge
    let outside = [
        home[0],
        0.0,
        crate::casts::f32_from_i32(grid.max_parcel_z + grid.padding + 2)
            * crate::casts::f32_from_i32(grid.parcel_size)
            + 8.0,
    ];
    if grid.locate(outside).is_some() {
        bail!(
            "{outside:?} is inside the grid of this stack; the scenario needs a position outside"
        );
    }
    move_and_confirm(
        &ctx,
        &subject.explorer,
        outside,
        "the explorer stands outside the grid",
    )
    .await?;
    let leaves = bot.view().leaves_of(&wallet);
    let accepted_before = subject.explorer.log_count("pulse: handshake accepted");
    let disconnects_before = subject.explorer.log_count("pulse: disconnected");
    let states = Trail::default();
    if let Ok(state) = pulse_state(&subject.explorer).await {
        states.push(&state);
    }

    // The link to Pulse is dead both ways for longer than the transport waits, while the explorer
    // stands outside the grid: its session ends and it has to connect again from there.
    subject.fronts.pulse.set(Direction::Both, Fault::Stall);
    let started = Instant::now();
    while started.elapsed() < LONG_OUTAGE {
        if let Ok(state) = pulse_state(&subject.explorer).await {
            states.push(&state);
        }
        tokio::time::sleep(CONSOLE_TICK).await;
    }
    subject.fronts.pulse.restore();
    ctx.note("outage, ms", crate::casts::millis(LONG_OUTAGE));

    let back = [home[0] + 16.0, 0.0, home[2]];
    let verdict = async {
        if states.seen("terminal disconnect") {
            bail!(
                "the explorer reported a terminal disconnect during the outage; its Pulse states: {:?}",
                states.states()
            );
        }
        // it stays outside for a while with the link back, whatever it does there to reconnect
        holds_for_async(
            &ctx,
            "outside the grid with the link back, the explorer does not report a terminal disconnect",
            Duration::from_secs(10),
            CONSOLE_TICK,
            || async {
                match pulse_state_onto(&subject.explorer, &states).await?.as_str() {
                    "terminal disconnect" => {
                        Err("the explorer reports a terminal disconnect".to_owned())
                    }
                    _ => Ok(()),
                }
            },
        )
        .await?;
        // the outage must have ended the session, or the rest proves nothing
        let left = bot.view().leaves_of(&wallet) - leaves;
        let disconnects = subject.explorer.log_count("pulse: disconnected") - disconnects_before;
        if states.states().iter().all(|state| state == "authenticated") && left == 0 && disconnects == 0
        {
            bail!(
                "the explorer reported Pulse as authenticated throughout, logged no disconnect and the observer never saw it leave: the outage broke nothing"
            );
        }
        states.push("(walked back inside)");
        move_to(&subject.explorer, back).await?;
        expect_async(
            &ctx,
            "the explorer is authenticated on Pulse within 30 s of walking back inside",
            Duration::from_secs(30),
            CONSOLE_TICK,
            || async {
                match pulse_state_onto(&subject.explorer, &states).await?.as_str() {
                    "authenticated" => Ok(()),
                    "terminal disconnect" => Err(
                        "fatal: the explorer reports a terminal disconnect: Pulse is over until the realm changes"
                            .to_owned(),
                    ),
                    other => Err(format!("the explorer reports Pulse as {other}")),
                }
            },
        )
        .await?;
        observer_sees_at(
            &ctx,
            &bot,
            &wallet,
            back,
            "the observer sees the explorer where it re-entered the grid",
        )
        .await
    }
    .await;
    let view = bot.view();
    ctx.note(
        "the explorer's Pulse states from standing outside on",
        states.states(),
    );
    ctx.note(
        "handshakes the explorer had accepted since the outage began",
        subject.explorer.log_count("pulse: handshake accepted") - accepted_before,
    );
    ctx.note(
        "times the observer saw the explorer leave",
        view.leaves_of(&wallet) - leaves,
    );
    ctx.note(
        "explorer log, Pulse (last lines)",
        last_log_lines(&subject.explorer, "pulse:", 8),
    );
    verdict?;
    subject.explorer.stop().await;
    Ok(())
}
