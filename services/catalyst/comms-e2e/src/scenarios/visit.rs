//! A crowd that visits other clients where they are, so a person can watch it arrive in a real
//! client and the crowd records which clients it met.
//!
//! Every other crowd scenario keeps its players away from visitors: a realm of the run's own, a
//! corner parcel. This one goes where the run's visit plan says ([`VisitPlan`]): the realm name
//! the clients there send to Pulse and a parcel they stand on. The players enter at once on a
//! grid around the parcel's centre, walk back and forth a little for the plan's time so a viewer
//! sees them move, and leave the way a client does.
//!
//! What it shows, each from a player's own connection:
//! - the join event of every wallet the plan expects: the `PlayerJoined` message Pulse sent the
//!   player when that client was in its area of interest, in the plan's realm. One player that
//!   caught none fails the run, naming the wallet;
//! - the join events of the crowd itself, as in `pulse-crowd`;
//! - every other wallet it caught a join of (visitors the plan did not name), with when;
//! - samples of how many of the crowd and of the visitors each player held while it stayed.
//!
//! It is the one scenario that meets real people: it is skipped unless the run names a place,
//! and a host that is not loopback needs `--allow-host` as for every scenario.

use std::collections::BTreeMap;
use std::time::{Duration, Instant};

use anyhow::{Context as _, Result, anyhow, bail};
use serde::Serialize;

use super::crowd::{
    PlayerJoins, enter_crowd, fits_one_address, join_evidence, join_verdict, player_name,
    players_of_run, pulse_handshakes_per_address,
};
use super::support::{leave_all, offset, settle};
use crate::engine::{Ctx, Entry, Needs, Scenario, ScenarioFuture, VisitPlan};
use crate::pulse::{Channel, PulseBot, messages};

/// The variables the command line takes a visit plan from.
pub const REALM_VARIABLE: &str = "CE2E_VISIT_REALM";
pub const PARCEL_VARIABLE: &str = "CE2E_VISIT_PARCEL";
pub const SECONDS_VARIABLE: &str = "CE2E_VISIT_SECONDS";
pub const EXPECT_VARIABLE: &str = "CE2E_VISIT_EXPECT";

/// How long the crowd stays unless the plan says otherwise, and the longest it may.
pub const DEFAULT_SECONDS: u64 = 120;
const MOST_SECONDS: u64 = 900;

/// The most players that fit on one parcel on this grid with room to walk.
pub const MOST_PLAYERS_IN_A_PARCEL: usize = 30;

/// Players per row, and the distance between two neighbours, in metres.
const COLUMNS: usize = 5;
const SPACING: f32 = 2.0;

/// The walk: out along +x for [`LEG`] seconds at [`SPEED`], a pause, back, a pause.
const PERIOD: f32 = 6.0;
const LEG: f32 = 2.5;
const SPEED: f32 = 0.8;

/// One movement input per player this often: a walking client's pace.
const TICK: Duration = Duration::from_millis(100);

/// How often what each player holds is sampled into the notes.
const SAMPLE: Duration = Duration::from_secs(10);

pub static CROWD_VISIT: Scenario = Scenario {
    name: "crowd-visit",
    tags: &["pulse", "crowd", "visit"],
    guards: &["claim: real clients and a crowd in one place see each other"],
    summary: "A crowd (20, or CE2E_CROWD_PLAYERS, at most 30) goes to the realm and parcel the run's visit plan names (CE2E_VISIT_REALM, CE2E_VISIT_PARCEL), where other clients stand, walks there for CE2E_VISIT_SECONDS (120 unless given) and leaves as a client does: every player must catch a Pulse join event for each wallet in CE2E_VISIT_EXPECT and for each other player; notes give every join each player caught, visitors included, and samples of what each held. Skipped without a plan",
    needs: Needs {
        visit: true,
        ..Needs::NONE
    },
    hostile: false,
    default_repeat: 1,
    run: |ctx| -> ScenarioFuture { Box::pin(visit(ctx)) },
};

/// The scenarios of this group, in the order a run takes them.
pub fn all() -> Vec<&'static Scenario> {
    vec![&CROWD_VISIT]
}

/// The visit plan the texts of the four variables name: `None` when neither a realm nor a parcel
/// is given. A realm and a parcel come together; every part given must be right.
pub fn plan_from(
    realm: Option<&str>,
    parcel: Option<&str>,
    seconds: Option<&str>,
    expect: Option<&str>,
) -> Result<Option<VisitPlan>> {
    let (realm, parcel) = match (realm, parcel) {
        (None, None) => {
            if seconds.is_some() || expect.is_some() {
                bail!(
                    "{SECONDS_VARIABLE} and {EXPECT_VARIABLE} belong to a visit plan: name the \
                     place with {REALM_VARIABLE} and {PARCEL_VARIABLE}"
                );
            }
            return Ok(None);
        }
        (Some(realm), Some(parcel)) => (realm, parcel),
        _ => bail!("a visit plan names both {REALM_VARIABLE} and {PARCEL_VARIABLE}"),
    };
    let realm = realm.trim();
    if realm.is_empty() {
        bail!("{REALM_VARIABLE} is empty");
    }
    let parcel = parcel
        .split_once(',')
        .and_then(|(x, y)| Some((x.trim().parse().ok()?, y.trim().parse().ok()?)))
        .with_context(|| format!("{PARCEL_VARIABLE} takes x,y, got {parcel:?}"))?;
    let seconds = match seconds {
        None => DEFAULT_SECONDS,
        Some(text) => {
            let seconds: u64 = text.trim().parse().with_context(|| {
                format!("{SECONDS_VARIABLE} must be a count of seconds, got {text:?}")
            })?;
            if !(1..=MOST_SECONDS).contains(&seconds) {
                bail!("{SECONDS_VARIABLE} must be between 1 and {MOST_SECONDS}, got {seconds}");
            }
            seconds
        }
    };
    let mut wallets = Vec::new();
    for wallet in expect
        .unwrap_or_default()
        .split(',')
        .map(str::trim)
        .filter(|text| !text.is_empty())
    {
        let hex = wallet.strip_prefix("0x").unwrap_or_default();
        if hex.len() != 40 || !hex.chars().all(|c| c.is_ascii_hexdigit()) {
            bail!("{EXPECT_VARIABLE} lists wallets as 0x and 40 hex digits, got {wallet:?}");
        }
        wallets.push(wallet.to_lowercase());
    }
    Ok(Some(VisitPlan {
        realm: realm.to_owned(),
        parcel,
        seconds,
        expect: wallets,
    }))
}

/// Where player `index` of `players` stands: on a grid of [`COLUMNS`], [`SPACING`] apart,
/// centred on `here` (west of it by the walk's length, so a walk stays centred too).
pub fn spot(here: [f32; 3], index: usize, players: usize) -> [f32; 3] {
    let rows = players.div_ceil(COLUMNS).max(1);
    let column = (index % COLUMNS) as f32 - (COLUMNS as f32 - 1.0) / 2.0;
    let row = (index / COLUMNS) as f32 - (rows as f32 - 1.0) / 2.0;
    offset(here, column * SPACING - SPEED * LEG / 2.0, row * SPACING)
}

/// Where player `index`, standing at `spot`, is `seconds` into the walk, the way it heads
/// (degrees, 90 along +x) and its speed. Each player is a little out of step with the others.
pub fn walking(spot: [f32; 3], index: usize, seconds: f32) -> ([f32; 3], f32, f32) {
    let phase = (seconds + index as f32 * 0.37).rem_euclid(PERIOD);
    let half = PERIOD / 2.0;
    let length = SPEED * LEG;
    let (along, heading, speed) = if phase < LEG {
        (phase * SPEED, 90.0, SPEED)
    } else if phase < half {
        (length, 90.0, 0.0)
    } else if phase < half + LEG {
        (length - (phase - half) * SPEED, 270.0, SPEED)
    } else {
        (0.0, 270.0, 0.0)
    };
    (offset(spot, along, 0.0), heading, speed)
}

/// A join event a player of the crowd caught of a wallet outside it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct VisitorJoin {
    pub wallet: String,
    pub realm: String,
    pub at_ms: u64,
}

/// Per player (actor `names[i]`), every join event it caught of a wallet not in `crowd`.
pub fn visitor_joins(
    entries: &[Entry],
    names: &[String],
    crowd: &[String],
) -> Vec<Vec<VisitorJoin>> {
    let crowd: Vec<String> = crowd.iter().map(|wallet| wallet.to_lowercase()).collect();
    let mut joins = vec![Vec::new(); names.len()];
    for entry in entries.iter().filter(|entry| entry.kind == "joined") {
        let Some(observer) = names.iter().position(|name| *name == entry.actor) else {
            continue;
        };
        let text = |key: &str| {
            entry
                .detail
                .get(key)
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default()
                .to_owned()
        };
        let wallet = text("wallet").to_lowercase();
        if crowd.contains(&wallet) {
            continue;
        }
        joins[observer].push(VisitorJoin {
            wallet,
            realm: text("realm"),
            at_ms: entry.at_ms,
        });
    }
    joins
}

/// For each expected wallet, the players that caught no join of it in `realm`.
pub fn missed(
    expect: &[String],
    visitors: &[Vec<VisitorJoin>],
    realm: &str,
) -> BTreeMap<String, Vec<usize>> {
    expect
        .iter()
        .map(|wallet| {
            let players = visitors
                .iter()
                .enumerate()
                .filter(|(_, joins)| {
                    !joins.iter().any(|join| {
                        join.wallet == *wallet && join.realm.eq_ignore_ascii_case(realm)
                    })
                })
                .map(|(index, _)| index)
                .collect();
            (wallet.clone(), players)
        })
        .collect()
}

/// One sample of what each player held: how many of the crowd, how many others.
#[derive(Debug, Clone, Serialize)]
struct Held {
    at_ms: u64,
    crowd: Vec<usize>,
    visitors: Vec<usize>,
}

fn sample(bots: &[PulseBot], crowd: &[String], started: Instant) -> Held {
    let mut held = Held {
        at_ms: started.elapsed().as_millis() as u64,
        crowd: Vec::with_capacity(bots.len()),
        visitors: Vec::with_capacity(bots.len()),
    };
    for bot in bots {
        let wallets = bot.view().wallets();
        let mates = wallets
            .iter()
            .filter(|wallet| crowd.contains(wallet))
            .count();
        held.crowd.push(mates);
        held.visitors.push(wallets.len() - mates);
    }
    held
}

/// Walks every player for `stay`, one input per [`TICK`], sampling what each holds; fails at
/// once, naming the player and the code, if the server ends a session.
async fn stay(
    ctx: &Ctx,
    bots: &[PulseBot],
    spots: &[[f32; 3]],
    crowd: &[String],
    stay: Duration,
) -> Result<()> {
    let grid = *bots
        .first()
        .map(PulseBot::grid)
        .ok_or_else(|| anyhow!("no players"))?;
    let started = Instant::now();
    let mut samples = vec![sample(bots, crowd, started)];
    let mut sampled = Instant::now();
    let mut inputs: u64 = 0;
    let outcome = loop {
        if let Some(ended) = bots.iter().find_map(|bot| bot.expect_alive().err()) {
            break Err(anyhow!(
                "a player's connection ended while the crowd stayed: {ended:#}"
            ));
        }
        let seconds = started.elapsed().as_secs_f32();
        for (index, bot) in bots.iter().enumerate() {
            let (at, heading, speed) = walking(spots[index], index, seconds);
            let message = messages::walking_at(&grid, at, heading, speed)
                .ok_or_else(|| anyhow!("{at:?} is outside the parcel grid"))?;
            bot.send(message, Channel::UnreliableSequenced);
            inputs += 1;
        }
        if sampled.elapsed() >= SAMPLE {
            samples.push(sample(bots, crowd, started));
            sampled = Instant::now();
        }
        if started.elapsed() >= stay {
            break Ok(());
        }
        tokio::time::sleep(TICK).await;
    };
    samples.push(sample(bots, crowd, started));
    ctx.note("walk_inputs_sent", inputs);
    ctx.note("held_samples", &samples);
    outcome
}

async fn visit(ctx: Ctx) -> Result<()> {
    let plan = ctx
        .visit()
        .ok_or_else(|| anyhow!("the run names no place to visit"))?;
    let players = players_of_run(&ctx)?;
    if players > MOST_PLAYERS_IN_A_PARCEL {
        bail!(
            "a visit takes at most {MOST_PLAYERS_IN_A_PARCEL} players, so they fit on one parcel; \
             got {players}"
        );
    }
    fits_one_address(players, &[pulse_handshakes_per_address(&ctx)])?;
    let here = ctx.stack().grid.parcel_centre(plan.parcel.0, plan.parcel.1);
    let identities: Vec<_> = (0..players)
        .map(|index| ctx.identity(index as u32))
        .collect();
    let wallets: Vec<String> = identities
        .iter()
        .map(|identity| identity.address().to_lowercase())
        .collect();
    let spots: Vec<[f32; 3]> = (0..players)
        .map(|index| spot(here, index, players))
        .collect();
    ctx.note("players", players);
    ctx.note("transport", format!("{:?}", ctx.transport()));
    ctx.note("visit_realm", &plan.realm);
    ctx.note("visit_parcel", [plan.parcel.0, plan.parcel.1]);
    ctx.note("visit_place", here);
    ctx.note("visit_seconds", plan.seconds);
    ctx.note("expected_wallets", &plan.expect);
    // index i is player-i: the names a viewer sees in a client map back to these
    ctx.note("crowd_wallets", &wallets);

    let started = Instant::now();
    let bots = enter_crowd(&ctx, &identities, &plan.realm, &spots).await?;
    ctx.note("all_entered_ms", started.elapsed().as_millis() as u64);

    let stayed = stay(
        &ctx,
        &bots,
        &spots,
        &wallets,
        Duration::from_secs(plan.seconds),
    )
    .await;

    // what each player caught, from its own connection's timeline, before anyone leaves
    let names: Vec<String> = (0..players).map(player_name).collect();
    let entries = ctx.timeline().snapshot();
    let visitors = visitor_joins(&entries, &names, &wallets);
    let crowd: Vec<PlayerJoins> = join_evidence(&entries, &names, &wallets, &plan.realm)
        .into_iter()
        .map(|joins| PlayerJoins {
            foreign: Vec::new(),
            ..joins
        })
        .collect();
    ctx.note(
        "crowd_join_events_caught_by_player",
        crowd
            .iter()
            .map(|joins| joins.from.len())
            .collect::<Vec<_>>(),
    );
    ctx.note("visitor_joins_by_player", &visitors);
    let mut met: BTreeMap<String, (usize, u64, u64)> = BTreeMap::new();
    for join in visitors.iter().flatten() {
        let seen = met.entry(join.wallet.clone()).or_insert((0, u64::MAX, 0));
        seen.0 += 1;
        seen.1 = seen.1.min(join.at_ms);
        seen.2 = seen.2.max(join.at_ms);
    }
    // per wallet outside the crowd: join events caught, the first and the last, in ms
    ctx.note("visitors", &met);
    let missed = missed(&plan.expect, &visitors, &plan.realm);
    ctx.note(
        "expected_caught_by",
        missed
            .iter()
            .map(|(wallet, without)| (wallet.clone(), bots.len() - without.len()))
            .collect::<BTreeMap<_, _>>(),
    );
    ctx.note("expected_missed_by", &missed);
    if let Some(first) = bots.first() {
        // where player-0 last held each visitor: whether they stood where the crowd did
        let view = first.view();
        let positions: BTreeMap<String, [f32; 3]> = view
            .subjects
            .values()
            .filter(|subject| !wallets.contains(&subject.wallet))
            .map(|subject| (subject.wallet.clone(), subject.world))
            .collect();
        ctx.note("visitor_positions_held_by_player_0", &positions);
    }

    let verdict = (|| {
        stayed?;
        let unmet: Vec<String> = missed
            .iter()
            .filter(|(_, players)| !players.is_empty())
            .map(|(wallet, players)| {
                format!("{wallet} (missed by {} of {})", players.len(), bots.len())
            })
            .collect();
        if !unmet.is_empty() {
            bail!(
                "the crowd did not catch a join event of every expected wallet in {}: {}",
                plan.realm,
                unmet.join(", ")
            );
        }
        join_verdict(&crowd).map_err(|joins| anyhow!("join events within the crowd: {joins}"))
    })();

    // every player leaves the way a client does, whatever the outcome
    let left = leave_all(&ctx, bots).await;
    settle(&ctx, verdict, left)
}
