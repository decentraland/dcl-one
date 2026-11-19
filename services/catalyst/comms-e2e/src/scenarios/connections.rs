//! Scenarios for the archipelago, the gatekeeper and LiveKit.

use std::time::Duration;

use anyhow::{Result, anyhow, bail};
use catalyst_comms::pulse::hardening::DisconnectReason;

use super::connections_support::*;
use crate::archipelago::{
    ArchipelagoBot, ArchipelagoOptions, ArchipelagoOutcome, KICKED_NEW_SESSION, socket_url,
};
use crate::engine::holds_for_async;
use crate::engine::{
    Ctx, Needs, Scenario, ScenarioFuture, expect_async, expect_or, holds_for, together,
};
use crate::fault::FaultLink;
use crate::gatekeeper::Gatekeeper;
use crate::pulse::{ConnectOptions, HandshakeOptions, HandshakeOutcome, PulseBot};
use crate::stack::Stack;

/// The scenarios of this group, in the order a run takes them.
pub fn all() -> Vec<&'static Scenario> {
    vec![
        &CONNECT_FULL_STACK,
        &SECOND_DEVICE_TAKEOVER,
        &STALE_SOCKET_RECONNECT,
        &ASSIGNMENT_ORDER,
        &MINTS_UNDER_CHURN,
        &BAN_EVICTION,
        &STATIONARY_CREDENTIALS,
    ]
}

const API: Needs = Needs {
    api: true,
    ..Needs::NONE
};

const MODERATION: Needs = Needs {
    moderator: true,
    ..API
};

const RESTART: Needs = Needs {
    restart: true,
    ..API
};

/// The interval at which the connector sweeps for bans: the server's default unless the stack
/// file says the stack runs with another (`limits.ban_sweep_interval_ms`).
fn ban_sweep(ctx: &Ctx) -> Duration {
    Duration::from_millis(ctx.stack().limits.ban_sweep_interval_ms)
}

pub static CONNECT_FULL_STACK: Scenario = Scenario {
    name: "connect-full-stack",
    tags: &["connections", "connect"],
    guards: &["the .NET connection-string fixture"],
    summary: "One wallet through archipelago and Pulse gets an island whose token is its own; two bots standing close share an island",
    needs: API,
    hostile: false,
    default_repeat: 1,
    run: |ctx| -> ScenarioFuture { Box::pin(connect_full_stack(ctx)) },
};

async fn connect_full_stack(ctx: Ctx) -> Result<()> {
    let realm = ctx.realm("main");
    let here = spot(&ctx, 0);

    let first = ctx.identity(0);
    let alone = bring_up(&ctx, "first", &first, &realm, here).await?;
    let assignment = first_assignment(&ctx, &alone.archipelago).await?;
    let first_room = join_room(&ctx, "first", &first, &assignment).await?;
    let island = expect_island_of(&ctx, &first.address()).await?;
    ctx.note("island", &island);

    // a second wallet a few metres away is put in the same island
    let second = ctx.identity(1);
    let neighbour = bring_up(&ctx, "second", &second, &realm, near(here, 2.0)).await?;
    let assignment_of_second = first_assignment(&ctx, &neighbour.archipelago).await?;
    let second_room = join_room(&ctx, "second", &second, &assignment_of_second).await?;
    expect_or(
        &ctx,
        "both bots end in one island",
        Duration::from_secs(10),
        || {
            let latest_of =
                |bot: &ArchipelagoBot| bot.assignments().last().map(|held| held.island.clone());
            match (
                latest_of(&alone.archipelago),
                latest_of(&neighbour.archipelago),
            ) {
                (Some(one), Some(other)) if one == other => Ok(()),
                (one, other) => Err(format!("their latest islands: {one:?} and {other:?}")),
            }
        },
    )
    .await?;
    let shared = expect_island_of(&ctx, &second.address()).await?;
    let held = expect_island_of(&ctx, &first.address()).await?;
    if shared != held {
        bail!("two bots on one parcel are in different islands: {held} and {shared}");
    }
    alone.pulse.expect_alive()?;
    neighbour.pulse.expect_alive()?;

    // The credentials really open the room: with a LiveKit client in the stack file both bots
    // join, and two bots of one island meet there.
    match (&first_room.client, &second_room.client) {
        (Some(one), Some(other)) if first_room.room == second_room.room => {
            expect_clients_meet(&ctx, (one, &first), (other, &second)).await?;
            ctx.note("LiveKit join", "both joined one room and saw each other");
        }
        (Some(_), Some(_)) => ctx.note(
            "LiveKit join",
            "both joined, in different rooms: the first assignment of each named another island",
        ),
        _ => ctx.note(
            "LiveKit join",
            "not done: the stack file names no LiveKit client (`[livekit] client`)",
        ),
    }
    Ok(())
}

pub static SECOND_DEVICE_TAKEOVER: Scenario = Scenario {
    name: "second-device-takeover",
    tags: &["connections", "session", "race"],
    guards: &["dcl-one #1205", "Pulse #34", "Pulse #54"],
    summary: "The same wallet on a second device, in three orders across archipelago and Pulse: Pulse ends the old device, the new one is welcomed and assigned, the wallet is one peer, and the old device's archipelago socket stays open and gets nothing more",
    needs: API,
    hostile: false,
    default_repeat: 6,
    run: |ctx| -> ScenarioFuture { Box::pin(second_device_takeover(ctx)) },
};

/// The three orders of the second device's handshakes; repeat `n` takes order `n % 3`.
const TAKEOVER_ORDERS: [&str; 3] = [
    "archipelago then Pulse",
    "Pulse then archipelago",
    "archipelago challenge, Pulse, then a seeded gap before the archipelago signature",
];

async fn second_device_takeover(ctx: Ctx) -> Result<()> {
    let order = (ctx.repeat() % 3) as usize;
    ctx.note("order", TAKEOVER_ORDERS[order]);
    let realm = ctx.realm("main");
    let here = spot(&ctx, 0);
    let old_identity = ctx.identity(0);
    let new_identity = old_identity.second_device();
    let wallet = old_identity.address();

    let old = bring_up(&ctx, "old", &old_identity, &realm, here).await?;
    let old_assignment = first_assignment(&ctx, &old.archipelago).await?;
    check_credentials(&old_assignment, &old_identity)?;
    expect_peer_count(&ctx, &wallet, 1).await?;

    let url = socket_url(api(&ctx)?)?;

    let (new_archipelago, new_pulse) = match order {
        0 => {
            let archipelago = ArchipelagoBot::connect(
                &ctx,
                "new-archipelago",
                &url,
                &new_identity,
                ArchipelagoOptions::default(),
            )
            .await?;
            archipelago.heartbeat(here);
            let pulse = pulse_up(&ctx, "new-pulse", &new_identity, &realm, here).await?;
            (archipelago, pulse)
        }
        1 => {
            let pulse = pulse_up(&ctx, "new-pulse", &new_identity, &realm, here).await?;
            let archipelago = ArchipelagoBot::connect(
                &ctx,
                "new-archipelago",
                &url,
                &new_identity,
                ArchipelagoOptions::default(),
            )
            .await?;
            archipelago.heartbeat(here);
            (archipelago, pulse)
        }
        _ => {
            let archipelago = ArchipelagoBot::begin(
                &ctx,
                "new-archipelago",
                &url,
                &new_identity,
                ArchipelagoOptions::default(),
            )
            .await?;
            let gap = ctx.offset_within(Duration::from_millis(400));
            ctx.note("gap before the signature, ms", gap.as_millis() as u64);
            let (pulse, finished) = tokio::join!(
                pulse_up(&ctx, "new-pulse", &new_identity, &realm, here),
                async {
                    tokio::time::sleep(gap).await;
                    archipelago.finish(&ctx, &new_identity).await
                }
            );
            finished?;
            archipelago.heartbeat(here);
            (archipelago, pulse?)
        }
    };

    // Pulse ends the old device's connection at once
    old.pulse
        .expect_disconnect_with(
            &ctx,
            DisconnectReason::DuplicateSession,
            Duration::from_secs(10),
        )
        .await?;

    // the new device is the one the stack serves
    let view = new_archipelago.view();
    if view.already_connected != Some(true) {
        bail!(
            "the new device's challenge said already_connected = {:?}; the old device was connected",
            view.already_connected
        );
    }
    let asked_at = ctx.timeline().elapsed_ms();
    let assignment = first_assignment(&ctx, &new_archipelago).await?;
    ctx.note(
        "new device assigned after, ms",
        assignment.at_ms.saturating_sub(asked_at),
    );
    check_credentials(&assignment, &new_identity)?;
    expect_peer_count(&ctx, &wallet, 1).await?;
    new_pulse.expect_alive()?;
    new_archipelago
        .alive()
        .map_err(|reason| anyhow!("the new archipelago socket: {reason}"))?;

    // The archipelago keeps one socket per wallet and session key and replaces only the same key,
    // so the old device's socket is left open and is sent nothing more: Pulse is what told it.
    let held = old.archipelago.assignments().len();
    holds_for(
        &ctx,
        "the old archipelago socket stays open and receives nothing more",
        TAKEOVER_QUIET_WINDOW,
        || {
            let view = old.archipelago.view();
            if let Some(reason) = view.kicked {
                return Err(format!("it was kicked with reason {reason}"));
            }
            if view.closed {
                return Err("it was closed".to_owned());
            }
            let now = old.archipelago.assignments().len();
            if now != held {
                return Err(format!(
                    "it received {} more assignment(s)",
                    now.saturating_sub(held)
                ));
            }
            Ok(())
        },
    )
    .await?;
    Ok(())
}

/// How long the old device's archipelago socket is watched for a kick, a close or an assignment
/// after the new device has its island.
const TAKEOVER_QUIET_WINDOW: Duration = Duration::from_secs(10);

pub static STALE_SOCKET_RECONNECT: Scenario = Scenario {
    name: "stale-socket-reconnect",
    tags: &["connections", "session", "fault"],
    guards: &["dcl-one #1205"],
    summary: "The archipelago socket goes silent without closing; the same device connects again and is welcomed and assigned although the server still holds the stale socket",
    needs: API,
    hostile: false,
    default_repeat: 1,
    run: |ctx| -> ScenarioFuture { Box::pin(stale_socket_reconnect(ctx)) },
};

async fn stale_socket_reconnect(ctx: Ctx) -> Result<()> {
    let realm = ctx.realm("main");
    let here = spot(&ctx, 0);
    let identity = ctx.identity(0);
    let wallet = identity.address();

    let pulse = PulseBot::enter(&ctx, "pulse", &identity).await?;
    pulse.teleport(&realm, here)?;
    pulse.move_to(here)?;

    let link = FaultLink::tcp(
        &ctx,
        "api-link",
        "127.0.0.1:0".parse()?,
        api_socket_address(&ctx)?,
    )
    .await?;
    let stale = ArchipelagoBot::open_with(&ctx, "stale", &identity, via(&link)).await?;
    stale.heartbeat(here);
    let assignment = first_assignment(&ctx, &stale).await?;
    check_credentials(&assignment, &identity)?;
    expect_peer_count(&ctx, &wallet, 1).await?;

    // the network goes away without a close: neither side is told
    link.half_open();

    // the same device comes back, straight to the stack, and stands still
    let device = identity.same_device();
    let again = ArchipelagoBot::open(&ctx, "again", &device).await?;
    ctx.note(
        "already_connected on the second socket",
        again.view().already_connected,
    );
    let assignment = first_assignment(&ctx, &again).await?;
    check_credentials(&assignment, &device)?;
    again
        .alive()
        .map_err(|reason| anyhow!("the new socket: {reason}"))?;
    pulse.expect_alive()?;

    // what the statistics route says before the new socket has heartbeated is a measurement: the
    // server closes the stale socket and may drop the wallet from the list with it
    let before = crate::oracle::peers(api(&ctx)?).await?;
    ctx.note(
        "listed after the reconnect, before a heartbeat",
        crate::oracle::of_wallet(&before, &wallet).len(),
    );
    // once the new socket heartbeats, the wallet is one peer, and stays one
    again.heartbeat(here);
    expect_peer_count(&ctx, &wallet, 1).await?;
    let api_url = api(&ctx)?.clone();
    holds_for_async(
        &ctx,
        "the statistics route lists the wallet once while its new socket is connected",
        Duration::from_secs(4),
        Duration::from_millis(500),
        || {
            let (api_url, wallet) = (api_url.clone(), wallet.clone());
            async move {
                let peers = crate::oracle::peers(&api_url)
                    .await
                    .map_err(|error| format!("{error:#}"))?;
                let held = crate::oracle::of_wallet(&peers, &wallet).len();
                if held == 1 {
                    Ok(())
                } else {
                    Err(format!("it was listed once and then {held} time(s)"))
                }
            }
        },
    )
    .await?;
    again
        .alive()
        .map_err(|reason| anyhow!("the new socket: {reason}"))?;

    // a socket of the same device that is still alive is replaced the same way, and is told so
    let third = ArchipelagoBot::open(&ctx, "third", &identity.same_device()).await?;
    expect_or(
        &ctx,
        "the replaced socket of the same device is kicked with the new-session reason",
        Duration::from_secs(10),
        || match again.view().kicked {
            Some(reason) if reason == KICKED_NEW_SESSION => Ok(()),
            Some(other) => Err(format!("fatal: it was kicked with reason {other}")),
            None => Err("not kicked yet".to_owned()),
        },
    )
    .await?;
    third
        .alive()
        .map_err(|reason| anyhow!("the replacing socket: {reason}"))?;
    Ok(())
}

pub static ASSIGNMENT_ORDER: Scenario = Scenario {
    name: "assignment-order",
    tags: &["connections", "assignment", "race"],
    guards: &["dcl-one #837", "dcl-one #795", "dcl-one #782"],
    summary: "Bots hop between two distant spots; each bot's assignments carry its own token and the last one matches the island the statistics route lists",
    needs: API,
    hostile: false,
    default_repeat: 2,
    run: |ctx| -> ScenarioFuture { Box::pin(assignment_order(ctx)) },
};

async fn assignment_order(ctx: Ctx) -> Result<()> {
    let realm = ctx.realm("main");
    let (far, home) = (spot(&ctx, 1), spot(&ctx, 0));
    let identities: Vec<_> = (0..3).map(|index| ctx.identity(index)).collect();
    let mut devices = Vec::new();
    for (index, identity) in identities.iter().enumerate() {
        devices.push(bring_up(&ctx, &format!("bot{index}"), identity, &realm, home).await?);
    }
    for device in &devices {
        first_assignment(&ctx, &device.archipelago).await?;
    }
    // the first hop is waited for: moving far away changes the island
    let before = devices[0].archipelago.assignments().len();
    let held = devices[0]
        .archipelago
        .assignments()
        .last()
        .map(|a| a.island.clone());
    move_device(&devices[0], &realm, far)?;
    let moved = next_assignment(&ctx, &devices[0].archipelago, before).await?;
    if Some(&moved.island) == held.as_ref() {
        bail!(
            "bot0 moved {far:?} away and was handed the island {} again",
            moved.island
        );
    }

    // then quick hops of two bots, the third stays where it is
    for hop in 0..6 {
        let target = if hop % 2 == 0 { home } else { far };
        for device in &devices[..2] {
            move_device(device, &realm, target)?;
        }
        tokio::time::sleep(ctx.offset_within(Duration::from_millis(300))).await;
    }
    // the last hop (index 5) went to `far`: put them back where the third bot is
    for device in &devices[..2] {
        move_device(device, &realm, home)?;
    }

    // all three stand at `home`: they end in one island, each bot's last assignment is that island
    // and agrees with the statistics route, and every token is the bot's own
    let api = api(&ctx)?.clone();
    expect_async(
        &ctx,
        "every bot's last assignment is the island the statistics route lists for it, and they agree",
        Duration::from_secs(30),
        Duration::from_millis(300),
        || {
            let api = api.clone();
            let lasts: Vec<_> = devices
                .iter()
                .map(|device| device.archipelago.assignments().last().map(|a| island_key(&a.island).to_owned()))
                .collect();
            let wallets: Vec<String> = identities.iter().map(|identity| identity.address()).collect();
            async move {
                let peers = crate::oracle::peers(&api).await.map_err(|error| format!("{error:#}"))?;
                let mut listed = Vec::new();
                for (index, wallet) in wallets.iter().enumerate() {
                    let island = crate::oracle::of_wallet(&peers, wallet)
                        .into_iter()
                        .find_map(|peer| peer.island.as_deref().map(|id| island_key(id).to_owned()));
                    listed.push(island.clone());
                    if lasts[index].is_none() || lasts[index] != island {
                        return Err(format!(
                            "bot{index}: its last assignment is {:?}, the statistics route lists {island:?}",
                            lasts[index]
                        ));
                    }
                }
                if listed.windows(2).any(|pair| pair[0] != pair[1]) {
                    return Err(format!("the bots are in different islands: {listed:?}"));
                }
                Ok(())
            }
        },
    )
    .await?;
    for (device, identity) in devices.iter().zip(&identities) {
        for assignment in device.archipelago.assignments() {
            check_credentials(&assignment, identity)?;
        }
        device
            .archipelago
            .alive()
            .map_err(|reason| anyhow!("{}: {reason}", device.archipelago.name()))?;
        device.pulse.expect_alive()?;
    }
    ctx.note(
        "assignments per bot",
        devices
            .iter()
            .map(|device| device.archipelago.assignments().len())
            .collect::<Vec<_>>(),
    );
    Ok(())
}

pub static MINTS_UNDER_CHURN: Scenario = Scenario {
    name: "mints-under-churn",
    tags: &["connections", "gatekeeper", "race"],
    guards: &["dcl-one #940", "dcl-one #383"],
    summary: "Scene-room mints in parallel while other bots change islands: every mint answers 200",
    needs: API,
    hostile: false,
    default_repeat: 1,
    run: |ctx| -> ScenarioFuture { Box::pin(mints_under_churn(ctx)) },
};

const MINTS_AT_A_TIME: usize = 8;
const MINT_ROUNDS: usize = 5;

async fn mints_under_churn(ctx: Ctx) -> Result<()> {
    let realm = ctx.realm("main");
    let scene_realm = ctx.realm("scene");
    let (home, far) = (spot(&ctx, 0), spot(&ctx, 1));
    let gatekeeper = Gatekeeper::new(&ctx)?;

    let mut churners = Vec::new();
    for index in 0..3 {
        let identity = ctx.identity(index);
        churners.push(bring_up(&ctx, &format!("churn{index}"), &identity, &realm, home).await?);
    }

    let mut failures: Vec<String> = Vec::new();
    for round in 0..MINT_ROUNDS {
        let target = if round % 2 == 0 { far } else { home };
        for churner in &churners {
            move_device(churner, &realm, target)?;
        }
        let results = together(MINTS_AT_A_TIME, |index| {
            let gatekeeper = gatekeeper.clone();
            let identity = ctx.identity(10 + index as u32);
            let scene_realm = scene_realm.clone();
            let scene_id = format!("e2e-scene-{}", index % 2);
            async move {
                gatekeeper
                    .scene_adapter(&identity, &scene_realm, &scene_id)
                    .await
                    .map(|adapter| (identity, adapter))
            }
        })
        .await;
        for (index, result) in results.into_iter().enumerate() {
            match result {
                Ok((identity, adapter)) => {
                    let connection = crate::archipelago::read_connection_string(&adapter)?;
                    if !connection
                        .claims
                        .identity
                        .eq_ignore_ascii_case(&identity.address())
                    {
                        failures.push(format!(
                            "round {round} mint {index}: the token is for another wallet"
                        ));
                    }
                }
                Err(error) => failures.push(format!("round {round} mint {index}: {error}")),
            }
        }
    }
    ctx.note("mints", MINTS_AT_A_TIME * MINT_ROUNDS);
    if !failures.is_empty() {
        bail!(
            "{} of {} mints did not answer 200 with a token for their wallet; the first: {}",
            failures.len(),
            MINTS_AT_A_TIME * MINT_ROUNDS,
            failures[0]
        );
    }
    for churner in &churners {
        churner.pulse.expect_alive()?;
    }
    Ok(())
}

pub static BAN_EVICTION: Scenario = Scenario {
    name: "ban-eviction",
    tags: &["connections", "moderation", "slow"],
    guards: &["Pulse 01095db", "dcl-one #1184"],
    summary: "Banning a connected wallet disconnects it from Pulse with Banned and from archipelago within the sweep interval; a reconnect is refused; the ban is always lifted",
    needs: MODERATION,
    hostile: true,
    default_repeat: 1,
    run: |ctx| -> ScenarioFuture { Box::pin(ban_eviction(ctx)) },
};

async fn ban_eviction(ctx: Ctx) -> Result<()> {
    let realm = ctx.realm("main");
    let here = spot(&ctx, 0);
    let identity = ctx.identity(0);
    let wallet = identity.address();
    let gatekeeper = Gatekeeper::new(&ctx)?;

    let device = bring_up(&ctx, "banned", &identity, &realm, here).await?;
    first_assignment(&ctx, &device.archipelago).await?;

    // The guard exists before the ban does and lifts it on every way out of this function,
    // including the runner dropping the future at the deadline.
    let ban = BanGuard::place(
        &ctx,
        &gatekeeper,
        &wallet,
        "comms-e2e: the ban eviction scenario",
    )
    .await?;
    let outcome = ban_checks(&ctx, &gatekeeper, &identity, &device).await;
    let lifted = ban.lift().await;
    outcome?;
    lifted.map_err(|error| anyhow!("lifting the ban: {error}"))?;
    if gatekeeper
        .is_banned(&wallet)
        .await
        .map_err(|error| anyhow!("{error}"))?
    {
        bail!("the wallet is still banned after the ban was lifted");
    }
    Ok(())
}

async fn ban_checks(
    ctx: &Ctx,
    gatekeeper: &Gatekeeper,
    identity: &crate::identity::Identity,
    device: &Device,
) -> Result<()> {
    let started = std::time::Instant::now();
    device
        .pulse
        .expect_disconnect_with(
            ctx,
            DisconnectReason::Banned,
            ban_sweep(ctx) + Duration::from_secs(15),
        )
        .await?;
    ctx.note("Pulse eviction, ms", started.elapsed().as_millis() as u64);
    expect_or(
        ctx,
        "the archipelago socket is closed after the ban",
        ban_sweep(ctx) + Duration::from_secs(15),
        || {
            let view = device.archipelago.view();
            if view.closed || view.kicked.is_some() {
                Ok(())
            } else {
                Err("still open".to_owned())
            }
        },
    )
    .await?;
    ctx.note(
        "archipelago eviction, ms",
        started.elapsed().as_millis() as u64,
    );

    // a reconnect is refused on both services
    if !gatekeeper
        .is_banned(&identity.address())
        .await
        .map_err(|error| anyhow!("{error}"))?
    {
        bail!("the gatekeeper does not list the wallet as banned");
    }
    let url = socket_url(api(ctx)?)?;
    // The connector verifies the wallet's signature and then, for a banned wallet, sends its kick
    // message (it has one reason, `KICKED_NEW_SESSION`) and closes: nothing else is the refusal.
    let bot = ArchipelagoBot::begin(
        ctx,
        "banned-again",
        &url,
        identity,
        ArchipelagoOptions::default(),
    )
    .await?;
    match bot.finish_outcome(ctx, identity).await? {
        ArchipelagoOutcome::Kicked(reason) if reason == KICKED_NEW_SESSION => {
            ctx.note(
                "archipelago reconnect",
                format!("kicked with reason {reason}"),
            );
        }
        outcome => bail!(
            "a banned wallet must be kicked with reason {KICKED_NEW_SESSION} by archipelago, got {outcome:?}"
        ),
    }
    // Pulse replies to the handshake with `success: false` and the error `banned`, then ends the
    // connection with `DisconnectReason::Banned`: either of the two reaches the bot first.
    let pulse = PulseBot::dial(ctx, "banned-pulse-again", ConnectOptions::default()).await?;
    match pulse
        .handshake(ctx, identity, HandshakeOptions::default())
        .await?
    {
        HandshakeOutcome::Refused { error } if error.as_deref() == Some("banned") => {
            ctx.note("Pulse reconnect", "refused: banned");
        }
        HandshakeOutcome::Disconnected { code } if code == DisconnectReason::Banned.code() => {
            ctx.note("Pulse reconnect", "disconnected: banned");
        }
        outcome => bail!(
            "a banned wallet must be refused by Pulse as banned (error `banned` or disconnect code {}), got {outcome:?}",
            DisconnectReason::Banned.code()
        ),
    }
    Ok(())
}

pub static STATIONARY_CREDENTIALS: Scenario = Scenario {
    name: "stationary-credentials",
    tags: &["connections", "restart", "slow"],
    guards: &["Pulse #48", "Pulse #51", "dcl-one #941"],
    summary: "A bot stands still while the gatekeeper's process restarts; when it connects again without moving, its credentials still arrive",
    needs: RESTART,
    hostile: true,
    default_repeat: 1,
    run: |ctx| -> ScenarioFuture { Box::pin(stationary_credentials(ctx)) },
};

/// The stack file's `restart_command`, ready to start: killed if it is dropped, and started
/// without the environment variables the stack file says hold secrets, as the explorer and the
/// LiveKit client are.
pub fn restart_command(stack: &Stack) -> Result<tokio::process::Command> {
    let words = stack
        .restart_command
        .as_ref()
        .ok_or_else(|| anyhow!("the stack file has no restart command"))?;
    let (program, arguments) = words
        .split_first()
        .ok_or_else(|| anyhow!("the restart command is empty"))?;
    let mut command = tokio::process::Command::new(program);
    command.args(arguments).kill_on_drop(true);
    for variable in stack.secret_variables() {
        command.env_remove(variable);
    }
    Ok(command)
}

async fn stationary_credentials(ctx: Ctx) -> Result<()> {
    let mut restart = restart_command(ctx.stack())?;
    let program = restart
        .as_std()
        .get_program()
        .to_string_lossy()
        .into_owned();
    let realm = ctx.realm("main");
    let here = spot(&ctx, 0);
    let identity = ctx.identity(0);

    // what the stack answers before the restart is what it must answer again after it
    let before = expect_realm_description(
        &ctx,
        api(&ctx)?,
        "the realm description answers 200 and names the comms adapter before the restart",
        STACK_BACK_DEADLINE,
    )
    .await?;

    let device = bring_up(&ctx, "standing", &identity, &realm, here).await?;
    first_assignment(&ctx, &device.archipelago).await?;

    ctx.record("runner", "restart", program);
    // The child is killed if this future is dropped (the scenario deadline).
    let status = tokio::time::timeout(Duration::from_secs(180), restart.status())
        .await
        .map_err(|_| anyhow!("the restart command did not return within 180 s"))??;
    if !status.success() {
        bail!("the restart command failed: {status}");
    }

    // whatever the checks find, the stack is left as it was found
    let outcome = after_restart(&ctx, &identity, &device).await;
    let back = expect_realm_description(
        &ctx,
        api(&ctx)?,
        "the realm description answers 200 again after the restart, as before it",
        STACK_BACK_DEADLINE,
    )
    .await;
    outcome?;
    let after = back?;
    if after != before {
        bail!(
            "the realm names a different comms adapter after the restart: before {before:?}, after {after:?}"
        );
    }
    Ok(())
}

/// How long a restarted stack may take to answer as it did before the restart.
const STACK_BACK_DEADLINE: Duration = Duration::from_secs(60);

async fn after_restart(
    ctx: &Ctx,
    identity: &crate::identity::Identity,
    device: &Device,
) -> Result<()> {
    // the gatekeeper answers again
    let gatekeeper_url = api(ctx)?.join("comms-gatekeeper/status")?;
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()?;
    expect_async(
        ctx,
        "the gatekeeper answers after the restart",
        Duration::from_secs(120),
        Duration::from_secs(1),
        || {
            let (url, client) = (gatekeeper_url.clone(), client.clone());
            async move {
                match client.get(url).send().await {
                    Ok(response) if response.status().is_success() => Ok(()),
                    Ok(response) => Err(format!("it answers {}", response.status())),
                    Err(error) => Err(error.to_string()),
                }
            }
        },
    )
    .await?;

    // the same device returns and does not move: the room must come without a heartbeat
    let again = ArchipelagoBot::open(ctx, "standing-again", &identity.same_device()).await?;
    let assignment = first_assignment(ctx, &again).await?;
    check_credentials(&assignment, identity)?;
    device.pulse.expect_alive()?;
    Ok(())
}

/// Moves a device: the Pulse connection teleports, the archipelago socket heartbeats.
fn move_device(device: &Device, realm: &str, world: [f32; 3]) -> Result<()> {
    device.pulse.teleport(realm, world)?;
    device.pulse.move_to(world)?;
    device.archipelago.heartbeat(world);
    Ok(())
}
