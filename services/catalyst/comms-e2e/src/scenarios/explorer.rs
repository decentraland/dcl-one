//! Scenarios with the headless explorer as the subject: the real client behind the fault link,
//! watched by bots.

use std::time::{Duration, Instant};

use anyhow::{Result, anyhow, bail};
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::engine::{Ctx, Needs, Scenario, expect_async, expect_or};
use crate::explorer::{Explorer, ExplorerOptions, Fronts};
use crate::fault::{Direction, Fault};
use crate::pulse::PulseBot;

/// The scenarios of this group, in the order a run takes them.
pub fn all() -> Vec<&'static Scenario> {
    vec![&UPLINK_STALL, &OUT_OF_GRID, &STALE_SOCKET, &PORT_UNREACHABLE]
}

const NEEDS: Needs = Needs {
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
    summary: "The explorer leaves the parcel grid and comes back: Pulse stays connected and \
              observers see it where it re-entered.",
    needs: NEEDS,
    hostile: false,
    default_repeat: 1,
    run: |ctx| Box::pin(out_of_grid(ctx)),
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

const SOON: Duration = Duration::from_secs(15);
const CONSOLE_TICK: Duration = Duration::from_millis(200);

/// The explorer under test, the links in front of it and what it says about itself.
struct Subject {
    fronts: Fronts,
    explorer: Explorer,
    wallet: String,
    /// The realm the explorer announces to Pulse.
    realm: String,
    position: [f32; 3],
}

/// A wallet seed no other run, scenario or repeat uses: explorers all enter the realm the stack
/// names, so only their wallets keep runs apart.
fn wallet_seed(ctx: &Ctx, index: u32) -> u64 {
    let digest = Sha256::digest(format!("{}/{}/{index}", ctx.realm("explorer"), ctx.seed()));
    u64::from_be_bytes(digest[..8].try_into().expect("eight bytes")) | 1
}

fn vector(value: &Value) -> Option<[f32; 3]> {
    let list = value.as_array()?;
    Some([
        list.first()?.as_f64()? as f32,
        list.get(1)?.as_f64()? as f32,
        list.get(2)?.as_f64()? as f32,
    ])
}

async fn start(ctx: &Ctx, preview: bool) -> Result<Subject> {
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
            wallet_seed: wallet_seed(ctx, 0),
            extra,
            ..ExplorerOptions::default()
        },
    )
    .await?;
    let world = expect_async(
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
    .await?;
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
    expect_pulse(ctx, &subject.explorer, "the explorer authenticates with Pulse", Duration::from_secs(60)).await?;
    Ok(subject)
}

/// The explorer's own word on its Pulse connection: `authenticated`, `terminal disconnect`,
/// `waiting`, `connecting` and so on.
async fn pulse_state(explorer: &Explorer) -> Result<String, String> {
    let state = explorer.probe("/pulse_state").await?;
    Ok(state["state"].as_str().unwrap_or("unknown").to_owned())
}

/// Waits until the explorer says Pulse is authenticated. Its terminal state ends the wait at
/// once: nothing brings a session back from it but a change of realm.
async fn expect_pulse(ctx: &Ctx, explorer: &Explorer, what: &str, deadline: Duration) -> Result<()> {
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
    let started = Instant::now();
    while started.elapsed() < window {
        let state = pulse_state(explorer).await.map_err(|error| anyhow!("{what}: {error}"))?;
        if state != "authenticated" {
            ctx.record("expect", "broken", serde_json::json!({ "what": what, "state": state }));
            bail!(
                "expected for {} s: {what}; after {} ms the explorer reports Pulse as {state}",
                window.as_secs(),
                started.elapsed().as_millis()
            );
        }
        tokio::time::sleep(CONSOLE_TICK).await;
    }
    ctx.record("expect", "held", serde_json::json!({ "what": what, "for_ms": window.as_millis() as u64 }));
    Ok(())
}

/// A bot standing next to the explorer, in its realm, that has seen it.
async fn observer(ctx: &Ctx, subject: &Subject) -> Result<PulseBot> {
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

/// Waits until the observer holds the explorer within half a metre of `world`.
async fn observer_sees_at(ctx: &Ctx, bot: &PulseBot, wallet: &str, world: [f32; 3], what: &str) -> Result<()> {
    expect_or(ctx, what, SOON, || {
        bot.alive()?;
        let seen = bot
            .view()
            .of(wallet)
            .map(|subject| subject.world)
            .ok_or("the observer no longer holds the explorer")?;
        let near = (seen[0] - world[0]).abs() < 0.5 && (seen[2] - world[2]).abs() < 0.5;
        near.then_some(()).ok_or_else(|| format!("the observer holds it at {seen:?}"))
    })
    .await
}

async fn uplink_stall(ctx: Ctx) -> Result<()> {
    let subject = start(&ctx, true).await?;
    let bot = observer(&ctx, &subject).await?;
    let limits = ctx.stack().limits;
    let home = subject.position;
    let away = [home[0] + 6.0, home[1], home[2]];

    // Each move is a teleport: a reliable message the explorer keeps until Pulse acknowledges it.
    // With the uplink stalled none is acknowledged, so they pile up on the explorer's side.
    subject.fronts.pulse.set(Direction::Uplink, Fault::Stall);
    let stall = Duration::from_secs(3);
    let started = Instant::now();
    let mut moves = 0u32;
    while started.elapsed() < stall {
        move_to(&subject.explorer, if moves % 2 == 0 { away } else { home }).await?;
        moves += 1;
    }
    let last = if moves % 2 == 1 { away } else { home };
    ctx.note("teleports during the stall", moves);
    ctx.note("server discrete burst", limits.discrete_burst);
    subject.fronts.pulse.restore();

    // What was kept is resent when the uplink returns. The session must outlive that.
    expect_pulse(&ctx, &subject.explorer, "Pulse is authenticated after the stall", Duration::from_secs(20)).await?;
    pulse_stays(&ctx, &subject.explorer, "Pulse stays authenticated after the resend", Duration::from_secs(10)).await?;
    observer_sees_at(&ctx, &bot, &subject.wallet, last, "the observer sees the explorer where it last teleported").await?;
    ctx.note("packets dropped by the stall", subject.fronts.pulse.counters().dropped);
    subject.explorer.stop().await;
    Ok(())
}

async fn out_of_grid(ctx: Ctx) -> Result<()> {
    let subject = start(&ctx, true).await?;
    let bot = observer(&ctx, &subject).await?;
    let grid = ctx.stack().grid;
    let home = subject.position;

    // two parcels past the padded edge of the grid
    let outside = [
        (grid.max_parcel_x + grid.padding + 2) as f32 * grid.parcel_size as f32 + 8.0,
        0.0,
        home[2],
    ];
    if grid.locate(outside).is_some() {
        bail!("{outside:?} is inside the grid of this stack; the scenario needs a position outside");
    }
    move_to(&subject.explorer, outside).await?;
    pulse_stays(&ctx, &subject.explorer, "Pulse stays authenticated while the explorer is outside the grid", Duration::from_secs(8)).await?;
    // nobody can be told where it is, but it has not left
    bot.expect_alive()?;
    if bot.view().count_of(&subject.wallet) != 1 {
        bail!(
            "while the explorer is outside the grid the observer holds {} subjects for it, expected 1",
            bot.view().count_of(&subject.wallet)
        );
    }

    let back = [home[0] + 16.0, 0.0, home[2]];
    move_to(&subject.explorer, back).await?;
    observer_sees_at(&ctx, &bot, &subject.wallet, back, "the observer sees the explorer where it re-entered the grid").await?;
    pulse_stays(&ctx, &subject.explorer, "Pulse stays authenticated after re-entry", Duration::from_secs(3)).await?;
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
    ctx.note("connectivity before", subject.explorer.probe("/connectivity_state").await.unwrap_or(Value::Null));

    // The client side of every connection through the API front closes; the server side stays
    // open. The explorer knows its socket is gone; the server still counts it as connected.
    subject.fronts.api.orphan()?;

    let conflict = expect_async(
        &ctx,
        "the explorer reconnects to the archipelago and holds a transport again",
        Duration::from_secs(60),
        Duration::from_millis(500),
        || async {
            let connectivity = subject.explorer.probe("/connectivity_state").await?;
            let text = connectivity.to_string().to_lowercase();
            if text.contains("identity_conflict") || text.contains("identityconflict") || text.contains("identity conflict") {
                return Err(format!("fatal: the explorer reports an identity conflict: {connectivity}"));
            }
            let state = archipelago_transports(&subject.explorer).await?;
            // one new connection through the front means it dialled again
            let dialled = subject.fronts.api.counters().flows;
            (state["total"].as_u64().unwrap_or(0) > 0 && dialled > 1)
                .then_some(connectivity.clone())
                .ok_or_else(|| format!("transports: {state}, connectivity: {connectivity}"))
        },
    )
    .await?;
    ctx.note("connectivity after", &conflict);
    expect_pulse(&ctx, &subject.explorer, "Pulse is still authenticated", SOON).await?;
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
    let (joins, leaves) = (bot.view().joins, bot.view().leaves);

    // The port closes: this machine answers the explorer's packets "port unreachable", which its
    // socket reports as an error. Far shorter than any timeout of the transport.
    subject.fronts.pulse.close_port()?;
    tokio::time::sleep(Duration::from_millis(1500)).await;
    subject.fronts.pulse.restore();

    expect_pulse(&ctx, &subject.explorer, "Pulse is authenticated after the port returns", SOON).await?;
    pulse_stays(&ctx, &subject.explorer, "Pulse stays authenticated", Duration::from_secs(5)).await?;
    // The same session, not a new one: the observer never saw the explorer leave and join.
    let view = bot.view();
    ctx.note("observer joins and leaves during the fault", (view.joins - joins, view.leaves - leaves));
    let now = view.of(&subject.wallet).map(|seen| seen.subject_id);
    if now != Some(held) || view.leaves != leaves {
        bail!(
            "the explorer did not keep its session: the observer held subject {held} and now holds {now:?}, with {} leaves and {} joins during the fault",
            view.leaves - leaves,
            view.joins - joins
        );
    }
    subject.explorer.stop().await;
    Ok(())
}
