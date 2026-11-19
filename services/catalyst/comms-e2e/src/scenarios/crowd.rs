//! A crowd in one place: every player sees every other player.
//!
//! The players enter a realm of the run's own at once and stand on a grid a few metres apart,
//! well inside each other's area of interest. "Sees" means Pulse: an observer holds exactly one
//! subject for the other player's wallet, in this realm, within one quantization step of where
//! that player stands.

use std::time::{Duration, Instant};

use anyhow::{Context as _, Result, anyhow, bail};
use futures::future::join_all;

use super::support::{distance, enter_at, leave_all, meeting_place, offset, refusal_code_in};
use crate::engine::{Ctx, Needs, Scenario, ScenarioFuture};
use crate::pulse::{PulseBot, PulseView, messages};

/// The environment variable that sets how many players the crowd has.
pub const PLAYERS_VARIABLE: &str = "CE2E_CROWD_PLAYERS";

/// How many players the crowd has unless [`PLAYERS_VARIABLE`] says otherwise.
pub const DEFAULT_PLAYERS: usize = 20;

/// The most players one crowd may have: a few hundred handshakes from one machine is a load
/// test, which this scenario is not.
const MOST_PLAYERS: usize = 100;

/// Players per row of the grid, and the distance between two neighbours, in metres.
const COLUMNS: usize = 5;
const SPACING: f32 = 2.0;

/// How far a position the server stored may differ from the one sent: one quantization step.
const QUANTIZATION: f32 = 0.15;

/// How long, from the first dial, every player may take to see every other player.
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
    summary: "A crowd (20 players, or CE2E_CROWD_PLAYERS) enters one realm at once, 2 m apart: every player must hold every other at its position within 20 s; notes record how many each saw and when the last sighting came",
    needs: Needs::NONE,
    hostile: false,
    default_repeat: 1,
    run: run_crowd,
};

/// The scenarios of this group, in the order a run takes them.
pub fn all() -> Vec<&'static Scenario> {
    vec![&CROWD]
}

/// The crowd size `value` names, or [`DEFAULT_PLAYERS`] when it names none.
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

/// Where player `index` stands: row by row on a grid of [`COLUMNS`], [`SPACING`] apart.
fn spot(here: [f32; 3], index: usize) -> [f32; 3] {
    offset(
        here,
        (index % COLUMNS) as f32 * SPACING,
        (index / COLUMNS) as f32 * SPACING,
    )
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
struct Sightings {
    /// For each player, how many of the others it held at the end.
    seen: Vec<usize>,
    /// For each player, when (from the first dial) it first held every other player.
    complete_at: Vec<Option<Duration>>,
}

impl Sightings {
    fn everyone(&self) -> usize {
        self.complete_at.iter().filter(|at| at.is_some()).count()
    }
}

/// Reads every view until each player holds every other, `deadline` passes, or a connection
/// ends.
async fn watch(
    bots: &[PulseBot],
    wallets: &[String],
    spots: &[[f32; 3]],
    realm: &str,
    started: Instant,
    deadline: Instant,
) -> Result<Sightings> {
    let players = bots.len();
    let mut sightings = Sightings {
        seen: vec![0; players],
        complete_at: vec![None; players],
    };
    loop {
        for (observer, bot) in bots.iter().enumerate() {
            bot.alive().map_err(|reason| {
                anyhow!("{}: {}", bot.name(), reason.trim_start_matches("fatal: "))
            })?;
            let view = bot.view();
            let mut seen = 0;
            for (other, wallet) in wallets.iter().enumerate() {
                if other == observer {
                    continue;
                }
                if holds(&view, wallet, realm, spots[other])
                    .map_err(|reason| anyhow!("{} {reason}", bot.name()))?
                {
                    seen += 1;
                }
            }
            sightings.seen[observer] = seen;
            if seen == players - 1 && sightings.complete_at[observer].is_none() {
                sightings.complete_at[observer] = Some(started.elapsed());
            }
        }
        if sightings.everyone() == players || Instant::now() >= deadline {
            return Ok(sightings);
        }
        tokio::time::sleep(POLL).await;
    }
}

async fn crowd(ctx: Ctx) -> Result<()> {
    let players = players_from(std::env::var(PLAYERS_VARIABLE).ok().as_deref())?;
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
    let entered = join_all(identities.iter().enumerate().map(|(index, identity)| {
        let (ctx, realm, spot) = (&ctx, &realm, spots[index]);
        async move { enter_at(ctx, &format!("player-{index}"), identity, realm, spot).await }
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
                refused.push(format!("player-{index}: {}", code.unwrap_or(message)));
            }
        }
    }
    ctx.note("players_admitted", bots.len());
    if !refused.is_empty() {
        ctx.note("players_refused", &refused);
        let admitted = bots.len();
        leave_all(&ctx, bots).await?;
        bail!(
            "{} of {players} players could not enter: {refused:?} ({admitted} admitted, who left again)",
            refused.len()
        );
    }
    ctx.note("all_entered_ms", started.elapsed().as_millis() as u64);

    let deadline = started + SIGHTING_DEADLINE;
    let watched = watch(&bots, &wallets, &spots, &realm, started, deadline).await;
    let outcome = match &watched {
        Ok(sightings) => {
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
                Err(anyhow!(
                    "{everyone} of {players} players saw every other player within {} s; the fewest any player held was {fewest} of {}; held per player: {:?}",
                    SIGHTING_DEADLINE.as_secs(),
                    players - 1,
                    sightings.seen
                ))
            }
        }
        Err(error) => Err(anyhow!(
            "a player's connection ended while the crowd stood together: {error:#}"
        )),
    };

    // every player leaves the way a client does, whatever the outcome
    let left = leave_all(&ctx, bots).await;
    outcome?;
    left
}
