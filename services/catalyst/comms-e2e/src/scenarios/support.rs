//! Helpers the Pulse scenarios share: entering a realm at a place, waiting for what an observer
//! holds, and walking at a pace the servers accept. They sit on the bot's public API and add no
//! policy of their own.

use std::time::Duration;

use anyhow::{Result, bail};

use crate::engine::{Ctx, Transport, expect_or, holds_for};
use crate::identity::Identity;
use crate::pulse::{PulseBot, Subject, messages};

/// How long a state that must arrive may take.
pub const SOON: Duration = Duration::from_secs(5);

/// The gap between two sends of a walking bot: inside the input budget of the server, and longer
/// than one server tick so each input is seen as its own change.
pub const PACE: Duration = Duration::from_millis(60);

/// The horizontal distance between two positions, in metres.
pub fn distance(a: [f32; 3], b: [f32; 3]) -> f32 {
    ((a[0] - b[0]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
}

/// `base` moved by `dx` and `dz` metres.
pub fn offset(base: [f32; 3], dx: f32, dz: f32) -> [f32; 3] {
    [base[0] + dx, base[1], base[2] + dz]
}

/// Where scenarios put their players unless they need another place: the centre of the parcel
/// (0, 0). Players of one scenario stand within a few metres of this, so they are inside each
/// other's area of interest.
pub fn meeting_place(ctx: &Ctx) -> [f32; 3] {
    ctx.stack().grid.parcel_centre(0, 0)
}

/// The wallets of `identities`, sorted: what an observer's roster must equal.
pub fn sorted_wallets(identities: &[&Identity]) -> Vec<String> {
    let mut wallets: Vec<String> = identities
        .iter()
        .map(|identity| identity.address())
        .collect();
    wallets.sort();
    wallets
}

/// Dials, handshakes and announces the realm and position.
pub async fn enter_at(
    ctx: &Ctx,
    name: &str,
    identity: &Identity,
    realm: &str,
    world: [f32; 3],
) -> Result<PulseBot> {
    let bot = PulseBot::enter(ctx, name, identity).await?;
    bot.teleport(realm, world)?;
    Ok(bot)
}

/// Waits until `observer` holds exactly one subject for `wallet`.
pub async fn expect_sees(
    ctx: &Ctx,
    observer: &PulseBot,
    what: &str,
    wallet: &str,
    deadline: Duration,
) -> Result<Subject> {
    let name = observer.name().to_owned();
    expect_or(ctx, &format!("{name} sees {what}"), deadline, || {
        observer.alive()?;
        let view = observer.view();
        match view.count_of(wallet) {
            1 => Ok(view.of(wallet).cloned().expect("exactly one")),
            0 => Err(format!(
                "{name} holds {} subjects: {:?}",
                view.subjects.len(),
                view.wallets()
            )),
            many => Err(format!("{name} holds {many} subjects for {what}")),
        }
    })
    .await
}

/// Waits until `observer` holds `wallet` within `tolerance` metres of `world`.
pub async fn expect_sees_at(
    ctx: &Ctx,
    observer: &PulseBot,
    what: &str,
    wallet: &str,
    world: [f32; 3],
    tolerance: f32,
    deadline: Duration,
) -> Result<Subject> {
    let name = observer.name().to_owned();
    expect_or(
        ctx,
        &format!("{name} sees {what} within {tolerance} m of {world:?}"),
        deadline,
        || {
            observer.alive()?;
            let view = observer.view();
            if view.count_of(wallet) > 1 {
                return Err(format!(
                    "fatal: {name} holds {} subjects for {what}",
                    view.count_of(wallet)
                ));
            }
            let subject = view
                .of(wallet)
                .ok_or_else(|| format!("{name} does not hold {what}"))?;
            if distance(subject.world, world) <= tolerance {
                Ok(subject.clone())
            } else {
                Err(format!("{what} is at {:?}", subject.world))
            }
        },
    )
    .await
}

/// Waits until `observer` no longer holds `wallet`.
pub async fn expect_gone(
    ctx: &Ctx,
    observer: &PulseBot,
    what: &str,
    wallet: &str,
    deadline: Duration,
) -> Result<()> {
    let name = observer.name().to_owned();
    expect_or(
        ctx,
        &format!("{name} no longer holds {what}"),
        deadline,
        || {
            observer.alive()?;
            match observer.view().count_of(wallet) {
                0 => Ok(()),
                _ => Err(format!("{name} still holds {what}")),
            }
        },
    )
    .await
}

/// Waits until `observer` holds exactly the wallets in `expected` (sorted), no more, no fewer.
pub async fn expect_roster(
    ctx: &Ctx,
    observer: &PulseBot,
    expected: &[String],
    deadline: Duration,
) -> Result<()> {
    let name = observer.name().to_owned();
    expect_or(
        ctx,
        &format!("{name} holds exactly {} players", expected.len()),
        deadline,
        || {
            observer.alive()?;
            let held = observer.view().wallets();
            if held == expected {
                return Ok(());
            }
            let missing: Vec<&String> = expected
                .iter()
                .filter(|wallet| !held.contains(wallet))
                .collect();
            let unexpected: Vec<&String> = held
                .iter()
                .filter(|wallet| !expected.contains(wallet))
                .collect();
            Err(format!(
                "{name} holds {} players; missing {missing:?}, unexpected {unexpected:?}",
                held.len()
            ))
        },
    )
    .await
}

/// Checks for `window` that none of `bots` is disconnected.
pub async fn stay_connected(
    ctx: &Ctx,
    what: &str,
    bots: &[&PulseBot],
    window: Duration,
) -> Result<()> {
    holds_for(ctx, what, window, || {
        for bot in bots {
            bot.alive().map_err(|reason| {
                format!("{}: {}", bot.name(), reason.trim_start_matches("fatal: "))
            })?;
        }
        Ok(())
    })
    .await
}

/// Walks `bot` from `from` to `to` in `steps` inputs, one every [`PACE`]; fails at once if the
/// server ends the connection.
pub async fn walk(bot: &PulseBot, from: [f32; 3], to: [f32; 3], steps: u32) -> Result<()> {
    for step in 1..=steps {
        let share = step as f32 / steps as f32;
        bot.expect_alive()?;
        bot.move_to([
            from[0] + (to[0] - from[0]) * share,
            from[1] + (to[1] - from[1]) * share,
            from[2] + (to[2] - from[2]) * share,
        ])?;
        tokio::time::sleep(PACE).await;
    }
    Ok(())
}

/// The UDP address of the Pulse server the bots of this run dial: the ENet address, or the
/// host and port of the WebTransport URL. A fault link in front of Pulse relays to it.
pub async fn pulse_udp_address(ctx: &Ctx) -> Result<std::net::SocketAddr> {
    match ctx.transport() {
        Transport::Enet => Ok(ctx.stack().pulse_enet),
        Transport::WebTransport => {
            let target =
                ctx.stack().pulse_webtransport.as_ref().ok_or_else(|| {
                    anyhow::anyhow!("the stack file has no Pulse over WebTransport")
                })?;
            let host = target
                .url
                .host_str()
                .ok_or_else(|| anyhow::anyhow!("{} has no host", target.url))?;
            let port = target.url.port().unwrap_or(443);
            tokio::net::lookup_host((host, port))
                .await?
                .find(|address| address.is_ipv4())
                .ok_or_else(|| anyhow::anyhow!("{host} has no IPv4 address"))
        }
    }
}

/// The disconnect code a failed dial names, if its message names one: a dial that ends with the
/// server's refusal reports the code as text.
pub fn refusal_code_in(message: &str) -> Option<u32> {
    messages::codes().find(|code| message.contains(&messages::reason(*code)))
}

/// Sums the samples of the metric series `name` (every label set) on the Pulse metrics address,
/// or `None` when the stack file names none or it cannot be read.
pub async fn metric_total(ctx: &Ctx, name: &str) -> Option<f64> {
    let base = ctx.stack().pulse_metrics.as_ref()?;
    let body = reqwest::Client::new()
        .get(base.clone())
        .timeout(Duration::from_secs(3))
        .send()
        .await
        .ok()?
        .text()
        .await
        .ok()?;
    let mut total = 0.0;
    let mut found = false;
    for line in body.lines() {
        let Some(rest) = line.strip_prefix(name) else {
            continue;
        };
        // `name{labels} value` or `name value`; a longer series name sharing the prefix is not it
        if !(rest.starts_with('{') || rest.starts_with(' ')) {
            continue;
        }
        if let Some(value) = line
            .rsplit(' ')
            .next()
            .and_then(|value| value.parse::<f64>().ok())
        {
            total += value;
            found = true;
        }
    }
    found.then_some(total)
}

/// The scene listener's identity: the wallet whose key the stack file's
/// `scene_listener_key_env` variable holds.
pub fn listener_identity(ctx: &Ctx) -> Result<Identity> {
    let Some(variable) = ctx.stack().scene_listener_key_env.as_deref() else {
        bail!("the stack file names no scene listener key (`scene_listener_key_env`)");
    };
    let key = ctx.stack().secret(variable)?;
    Identity::from_wallet_key(&key, &format!("{}/{}", ctx.run_id(), ctx.realm("listener")))
}

/// Leaves the way a client does: asks to disconnect and waits until the server has acknowledged,
/// so nothing is sent to the bot's address after it is gone.
pub async fn leave(ctx: &Ctx, bot: PulseBot) -> Result<()> {
    bot.disconnect(catalyst_comms::pulse::hardening::DisconnectReason::Graceful.code());
    let name = bot.name().to_owned();
    expect_or(
        ctx,
        &format!("{name}'s departure is acknowledged"),
        SOON,
        || {
            bot.view()
                .disconnect
                .map(|_| ())
                .ok_or_else(|| "not acknowledged yet".to_owned())
        },
    )
    .await?;
    bot.close();
    Ok(())
}

/// [`leave`] for every bot of a group at once. Every bot leaves, whichever fails; the first
/// failure is returned.
pub async fn leave_all(ctx: &Ctx, bots: Vec<PulseBot>) -> Result<()> {
    futures::future::join_all(bots.into_iter().map(|bot| leave(ctx, bot)))
        .await
        .into_iter()
        .collect::<Result<Vec<()>>>()?;
    Ok(())
}

/// The result of a scenario whose actors have left (`left`): the scenario's own error when it
/// failed, with a leave error that came too kept in the timeline; else the leave error, if any.
pub fn settle<T>(ctx: &Ctx, outcome: Result<T>, left: Result<()>) -> Result<T> {
    match (outcome, left) {
        (Ok(value), Ok(())) => Ok(value),
        (Ok(_), Err(left)) => Err(left),
        (Err(error), Ok(())) => Err(error),
        (Err(error), Err(left)) => {
            ctx.record("runner", "leave failed", format!("{left:#}"));
            Err(error)
        }
    }
}

/// Runs `work`, and fails at once, naming the code, if the server ends the connection of any of
/// `bots`: a scenario that waits on an observer for what another bot does would otherwise wait
/// out its deadline when that bot is the one the server dropped.
pub async fn while_alive<T>(
    bots: &[&PulseBot],
    work: impl std::future::Future<Output = Result<T>>,
) -> Result<T> {
    let watch = async {
        loop {
            for bot in bots {
                if let Err(reason) = bot.alive() {
                    return Err::<T, _>(anyhow::anyhow!(
                        "{}: {}",
                        bot.name(),
                        reason.trim_start_matches("fatal: ")
                    ));
                }
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    };
    tokio::select! {
        biased;
        done = work => done,
        ended = watch => ended,
    }
}
