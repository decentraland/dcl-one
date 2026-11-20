//! A crowd in one place: every player sees every other player.
//!
//! The players enter a realm of the run's own at once and stand on a grid a few metres apart,
//! well inside each other's area of interest. Two things show that one player sees another, both
//! from that player's own connection:
//!
//! - the join event: the `PlayerJoined` message Pulse sends a client when another player enters
//!   its area of interest, recorded as a `joined` entry of the client's timeline. Every player
//!   must catch exactly one for each of the others, in this realm, and none for anyone else;
//! - the held state: the client holds exactly one subject for the other player's wallet, in this
//!   realm, within one quantization step of where that player stands.

use std::collections::BTreeMap;
use std::time::{Duration, Instant};

use anyhow::{Context as _, Result, anyhow, bail};
use futures::future::join_all;
use serde::Serialize;

use super::support::{
    distance, enter_at, leave_all, meeting_place, offset, refusal_code_in, settle,
};
use crate::engine::{Ctx, Entry, Needs, Scenario, ScenarioFuture};
use crate::identity::Identity;
use crate::pulse::{PulseBot, PulseView, messages};

/// The environment variable from which the command line takes the crowd's size.
pub const PLAYERS_VARIABLE: &str = "CE2E_CROWD_PLAYERS";

/// How many players the crowd has unless the run says otherwise.
pub const DEFAULT_PLAYERS: usize = 20;

/// The most players one crowd may have: a few hundred handshakes from one machine is a load
/// test, which this scenario is not.
const MOST_PLAYERS: usize = 100;

/// Players per row of the grid, and the distance between two neighbours, in metres.
const COLUMNS: usize = 5;
const SPACING: f32 = 2.0;

/// How far a position the server stored may differ from the one sent: one quantization step.
const QUANTIZATION: f32 = 0.15;

/// How long every player may take to see every other player, counted from when the whole crowd
/// has entered Pulse: in `pulse-crowd` and `crowd-discovery` alike.
pub const SIGHTING_DEADLINE: Duration = Duration::from_secs(20);

/// How often the views are read while waiting.
const POLL: Duration = Duration::from_millis(20);

fn run_crowd(ctx: Ctx) -> ScenarioFuture {
    Box::pin(crowd(ctx))
}

pub static CROWD: Scenario = Scenario {
    name: "pulse-crowd",
    tags: &["pulse", "crowd"],
    guards: &["claim: 20 players in one place see each other"],
    summary: "A crowd (20 players, or CE2E_CROWD_PLAYERS) enters one realm at once, 2 m apart: within 20 s every player must catch one join event from Pulse for each other player and none for anyone else, and hold each at its position; notes give, per player, whose joins it caught and when",
    needs: Needs::NONE,
    hostile: false,
    default_repeat: 1,
    run: run_crowd,
};

/// The scenarios of this group, in the order a run takes them.
pub fn all() -> Vec<&'static Scenario> {
    vec![&CROWD]
}

/// The crowd size `value` (the text of [`PLAYERS_VARIABLE`]) names, or [`DEFAULT_PLAYERS`] when
/// it names none.
pub fn players_from(value: Option<&str>) -> Result<usize> {
    let Some(text) = value else {
        return Ok(DEFAULT_PLAYERS);
    };
    let players: usize = text
        .trim()
        .parse()
        .with_context(|| format!("{PLAYERS_VARIABLE} must be a count of players, got {text:?}"))?;
    if !(2..=MOST_PLAYERS).contains(&players) {
        bail!("{PLAYERS_VARIABLE} must be between 2 and {MOST_PLAYERS}, got {players}");
    }
    Ok(players)
}

/// The crowd size of this run ([`crate::engine::CrowdSize::players`]), or [`DEFAULT_PLAYERS`].
pub(crate) fn players_of_run(ctx: &Ctx) -> Result<usize> {
    let players = ctx.crowd().players.unwrap_or(DEFAULT_PLAYERS);
    if !(2..=MOST_PLAYERS).contains(&players) {
        bail!(
            "the crowd ({PLAYERS_VARIABLE}) must have 2 to {MOST_PLAYERS} players, got {players}"
        );
    }
    Ok(players)
}

/// A limit of the servers on what one address may hold at once: its key under `[limits]` in
/// the stack file and its value, `None` when it is unlimited.
pub(crate) type PerAddress = (&'static str, Option<usize>);

/// Pulse's limit on handshakes in flight from one address.
pub(crate) fn pulse_handshakes_per_address(ctx: &Ctx) -> PerAddress {
    let limit = ctx.stack().limits.max_concurrent_pre_auth_per_ip;
    (
        "max_concurrent_pre_auth_per_ip",
        (limit > 0).then_some(limit as usize),
    )
}

/// Every player of a crowd dials from one address at the same moment, so a crowd larger than
/// any of `limits` is more than a correct server admits: refused before the first packet.
pub(crate) fn fits_one_address(players: usize, limits: &[PerAddress]) -> Result<()> {
    for (key, limit) in limits {
        if let Some(limit) = limit
            && players > *limit
        {
            bail!(
                "a crowd of {players} is more than a correct server admits from one address: \
                 the stack file's `limits.{key}` is {limit}, and every player dials from one \
                 address at once; ask for {limit} players or fewer ({PLAYERS_VARIABLE})"
            );
        }
    }
    Ok(())
}

/// Where player `index` stands around `here`: row by row on a grid of [`COLUMNS`], [`SPACING`]
/// apart.
pub fn spot(here: [f32; 3], index: usize) -> [f32; 3] {
    offset(
        here,
        (index % COLUMNS) as f32 * SPACING,
        (index / COLUMNS) as f32 * SPACING,
    )
}

/// The Pulse actor name of player `index`: its timeline entries carry it.
pub(crate) fn player_name(index: usize) -> String {
    format!("player-{index}")
}

/// Whether `view` holds `wallet` once, in `realm`, within one quantization step of `world`. A
/// wallet held twice is an error of its own.
fn holds(view: &PulseView, wallet: &str, realm: &str, world: [f32; 3]) -> Result<bool, String> {
    match view.count_of(wallet) {
        0 => Ok(false),
        1 => {
            let subject = view.of(wallet).expect("exactly one");
            Ok(subject.realm == realm && distance(subject.world, world) <= QUANTIZATION)
        }
        many => Err(format!("holds {many} subjects for {wallet}")),
    }
}

/// What the crowd saw by the end of the wait.
pub struct Sightings {
    /// For each player, how many of the others it held at the end.
    pub seen: Vec<usize>,
    /// For each player, when (from the `started` given to [`watch`]) it first held every other
    /// player, as the poll that saw it found it.
    pub complete_at: Vec<Option<Duration>>,
}

impl Sightings {
    pub fn everyone(&self) -> usize {
        self.complete_at.iter().filter(|at| at.is_some()).count()
    }
}

/// Whether `view` has been told of a join of every wallet in `others`.
fn joined_all(view: &PulseView, others: &[&String]) -> bool {
    others
        .iter()
        .all(|wallet| view.joins_of(&wallet.to_lowercase()) > 0)
}

/// Why the wait on the crowd ended before its deadline.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Interrupted {
    /// The server, or the transport, ended a player's session.
    Ended { player: String, reason: String },
    /// A player's view broke a rule of the protocol, such as one subject per wallet.
    Broken { player: String, reason: String },
}

impl std::fmt::Display for Interrupted {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Interrupted::Ended { player, reason } => write!(
                formatter,
                "a player's connection ended while the crowd stood together: {player}: {reason}"
            ),
            Interrupted::Broken { player, reason } => write!(formatter, "{player} {reason}"),
        }
    }
}

impl std::error::Error for Interrupted {}

/// Reads every view until each player holds every other and has been told each one joined,
/// `deadline` passes, or a session ends or a view breaks a rule of the protocol.
pub async fn watch(
    bots: &[PulseBot],
    wallets: &[String],
    spots: &[[f32; 3]],
    realm: &str,
    started: Instant,
    deadline: Instant,
) -> Result<Sightings, Interrupted> {
    let players = bots.len();
    let mut sightings = Sightings {
        seen: vec![0; players],
        complete_at: vec![None; players],
    };
    let mut all_joined = vec![false; players];
    loop {
        for (observer, bot) in bots.iter().enumerate() {
            bot.alive().map_err(|reason| Interrupted::Ended {
                player: bot.name().to_owned(),
                reason: reason.trim_start_matches("fatal: ").to_owned(),
            })?;
            let view = bot.view();
            let mut seen = 0;
            for (other, wallet) in wallets.iter().enumerate() {
                if other == observer {
                    continue;
                }
                if holds(&view, wallet, realm, spots[other]).map_err(|reason| {
                    Interrupted::Broken {
                        player: bot.name().to_owned(),
                        reason,
                    }
                })? {
                    seen += 1;
                }
            }
            sightings.seen[observer] = seen;
            if seen == players - 1 && sightings.complete_at[observer].is_none() {
                sightings.complete_at[observer] = Some(started.elapsed());
            }
            let others: Vec<&String> = wallets
                .iter()
                .enumerate()
                .filter(|(other, _)| *other != observer)
                .map(|(_, wallet)| wallet)
                .collect();
            all_joined[observer] = joined_all(&view, &others);
        }
        let done = sightings.everyone() == players && all_joined.iter().all(|done| *done);
        if done || Instant::now() >= deadline {
            return Ok(sightings);
        }
        tokio::time::sleep(POLL).await;
    }
}

/// The join events one player caught, as its own timeline recorded them.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct PlayerJoins {
    /// `(index of the other player, ms since the scenario started)`, in the order they arrived.
    pub from: Vec<(usize, u64)>,
    /// Joins of a wallet that is not one of the crowd, or of a crowd wallet in another realm:
    /// the wallet and the realm. None may happen.
    pub foreign: Vec<(String, String)>,
    /// Joins that named the player itself.
    pub own: usize,
}

impl PlayerJoins {
    pub fn first_ms(&self) -> Option<u64> {
        self.from.iter().map(|(_, at)| *at).min()
    }

    pub fn last_ms(&self) -> Option<u64> {
        self.from.iter().map(|(_, at)| *at).max()
    }

    /// The other players it caught a join of, each once, sorted.
    pub fn distinct(&self) -> Vec<usize> {
        let mut players: Vec<usize> = self.from.iter().map(|(index, _)| *index).collect();
        players.sort_unstable();
        players.dedup();
        players
    }
}

/// The join events each player caught, read from the `joined` entries its own Pulse connection
/// (actor `names[i]`) recorded. `wallets[i]` is player `i`'s wallet.
pub fn join_evidence(
    entries: &[Entry],
    names: &[String],
    wallets: &[String],
    realm: &str,
) -> Vec<PlayerJoins> {
    let index_of: BTreeMap<String, usize> = wallets
        .iter()
        .enumerate()
        .map(|(index, wallet)| (wallet.to_lowercase(), index))
        .collect();
    let actor_of: BTreeMap<&str, usize> = names
        .iter()
        .enumerate()
        .map(|(index, name)| (name.as_str(), index))
        .collect();
    let mut evidence = vec![PlayerJoins::default(); names.len()];
    for entry in entries.iter().filter(|entry| entry.kind == "joined") {
        let Some(&observer) = actor_of.get(entry.actor.as_str()) else {
            continue;
        };
        let text = |key: &str| {
            entry
                .detail
                .get(key)
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default()
                .to_lowercase()
        };
        let (wallet, joined_realm) = (text("wallet"), text("realm"));
        let joins = &mut evidence[observer];
        match index_of.get(&wallet) {
            Some(&other) if other == observer => joins.own += 1,
            Some(&other) if joined_realm == realm.to_lowercase() => {
                joins.from.push((other, entry.at_ms))
            }
            _ => joins.foreign.push((wallet, joined_realm)),
        }
    }
    evidence
}

/// Every player caught exactly one join event for each other player, none for itself and none
/// for anyone outside the crowd.
pub fn join_verdict(evidence: &[PlayerJoins]) -> Result<(), String> {
    let players = evidence.len();
    let mut problems = Vec::new();
    for (index, joins) in evidence.iter().enumerate() {
        let name = player_name(index);
        let distinct = joins.distinct();
        let expected = players - 1;
        if distinct.len() != expected {
            let missing: Vec<usize> = (0..players)
                .filter(|other| *other != index && !distinct.contains(other))
                .collect();
            problems.push(format!(
                "{name} caught joins of {} of {expected} players, missing {missing:?}",
                distinct.len()
            ));
        }
        if joins.from.len() != distinct.len() {
            problems.push(format!(
                "{name} caught {} join events for {} players: some twice",
                joins.from.len(),
                distinct.len()
            ));
        }
        if joins.own > 0 {
            problems.push(format!(
                "{name} was told {} times that it joined",
                joins.own
            ));
        }
        if !joins.foreign.is_empty() {
            problems.push(format!(
                "{name} caught joins of players outside the crowd: {:?}",
                joins.foreign
            ));
        }
    }
    if problems.is_empty() {
        Ok(())
    } else {
        Err(problems.join("; "))
    }
}

/// The join evidence of this run, noted per player (whose joins it caught, by index, and when),
/// and its verdict.
pub(crate) fn note_joins(
    ctx: &Ctx,
    wallets: &[String],
    realm: &str,
) -> (Vec<PlayerJoins>, Result<(), String>) {
    let names: Vec<String> = (0..wallets.len()).map(player_name).collect();
    let evidence = join_evidence(&ctx.timeline().snapshot(), &names, wallets, realm);
    let counts: Vec<usize> = evidence.iter().map(|joins| joins.from.len()).collect();
    ctx.note("join_events_caught_by_player", &counts);
    ctx.note(
        "join_events_first_ms_by_player",
        evidence
            .iter()
            .map(PlayerJoins::first_ms)
            .collect::<Vec<_>>(),
    );
    ctx.note(
        "join_events_last_ms_by_player",
        evidence
            .iter()
            .map(PlayerJoins::last_ms)
            .collect::<Vec<_>>(),
    );
    // per player, `[other player, ms]` for every join event it caught
    ctx.note(
        "join_events_by_player",
        evidence
            .iter()
            .map(|joins| joins.from.clone())
            .collect::<Vec<_>>(),
    );
    let foreign: usize = evidence.iter().map(|joins| joins.foreign.len()).sum();
    ctx.note("join_events_outside_the_crowd", foreign);
    let verdict = join_verdict(&evidence);
    (evidence, verdict)
}

/// Notes what the held-state wait saw and says whether every player held every other.
pub(crate) fn note_sightings(ctx: &Ctx, sightings: &Sightings) -> Result<(), String> {
    let players = sightings.seen.len();
    let everyone = sightings.everyone();
    ctx.note("others_seen_by_each", &sightings.seen);
    ctx.note("players_seeing_everyone", everyone);
    let times: Vec<Option<u64>> = sightings
        .complete_at
        .iter()
        .map(|at| at.map(|at| at.as_millis() as u64))
        .collect();
    ctx.note("saw_everyone_at_ms_by_player", &times);
    if let Some(last) = times.iter().flatten().max() {
        ctx.note("last_sighting_ms", last);
    }
    if everyone == players {
        Ok(())
    } else {
        let fewest = sightings.seen.iter().min().copied().unwrap_or(0);
        Err(format!(
            "{everyone} of {players} players saw every other player within {} s; the fewest any player held was {fewest} of {}; held per player: {:?}",
            SIGHTING_DEADLINE.as_secs(),
            players - 1,
            sightings.seen
        ))
    }
}

/// Every player of `identities` enters `realm` at its spot, all at once. A refusal leaves the
/// admitted ones and fails, naming each refusal's code.
pub async fn enter_crowd(
    ctx: &Ctx,
    identities: &[Identity],
    realm: &str,
    spots: &[[f32; 3]],
) -> Result<Vec<PulseBot>> {
    let players = identities.len();
    let entered = join_all(identities.iter().enumerate().map(|(index, identity)| {
        let spot = spots[index];
        async move { enter_at(ctx, &player_name(index), identity, realm, spot).await }
    }))
    .await;
    let mut bots = Vec::with_capacity(players);
    let mut refused = Vec::new();
    for (index, outcome) in entered.into_iter().enumerate() {
        match outcome {
            Ok(bot) => bots.push(bot),
            Err(error) => {
                let message = format!("{error:#}");
                let code = refusal_code_in(&message).map(messages::reason);
                refused.push(format!(
                    "{}: {}",
                    player_name(index),
                    code.unwrap_or(message)
                ));
            }
        }
    }
    ctx.note("players_admitted", bots.len());
    if !refused.is_empty() {
        ctx.note("players_refused", &refused);
        let admitted = bots.len();
        let left = leave_all(ctx, bots).await;
        return settle(
            ctx,
            Err(anyhow!(
                "{} of {players} players could not enter: {refused:?} ({admitted} admitted, who left again)",
                refused.len()
            )),
            left,
        );
    }
    Ok(bots)
}

async fn crowd(ctx: Ctx) -> Result<()> {
    let players = players_of_run(&ctx)?;
    fits_one_address(players, &[pulse_handshakes_per_address(&ctx)])?;
    let realm = ctx.realm("crowd");
    let here = meeting_place(&ctx);
    let identities: Vec<_> = (0..players)
        .map(|index| ctx.identity(index as u32))
        .collect();
    let wallets: Vec<String> = identities
        .iter()
        .map(|identity| identity.address())
        .collect();
    let spots: Vec<[f32; 3]> = (0..players).map(|index| spot(here, index)).collect();
    ctx.note("players", players);
    ctx.note("transport", format!("{:?}", ctx.transport()));

    // everyone arrives at once, as a crowd does when an event starts
    let started = Instant::now();
    let bots = enter_crowd(&ctx, &identities, &realm, &spots).await?;
    ctx.note("all_entered_ms", started.elapsed().as_millis() as u64);

    let deadline = Instant::now() + SIGHTING_DEADLINE;
    let watched = watch(&bots, &wallets, &spots, &realm, started, deadline).await;
    let outcome = match watched {
        Ok(sightings) => {
            // the join events first: they are what each client itself was told
            let (_, joins) = note_joins(&ctx, &wallets, &realm);
            let held = note_sightings(&ctx, &sightings);
            match (joins, held) {
                (Ok(()), Ok(())) => Ok(()),
                (Err(joins), _) => Err(anyhow!("join events: {joins}")),
                (Ok(()), Err(held)) => Err(anyhow!(held)),
            }
        }
        Err(interrupted) => Err(anyhow!(interrupted)),
    };

    // every player leaves the way a client does, whatever the outcome
    let left = leave_all(&ctx, bots).await;
    settle(&ctx, outcome, left)
}
