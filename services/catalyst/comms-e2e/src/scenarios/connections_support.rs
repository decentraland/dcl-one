//! Helpers the connection scenarios share: where bots stand, how a device (an archipelago socket
//! and a Pulse connection of one wallet) is brought up, and what to wait for.

use std::time::Duration;

use anyhow::{Result, anyhow, bail};
use url::Url;

use crate::archipelago::{ArchipelagoBot, ArchipelagoOptions, Assignment, read_connection_string};
use crate::engine::{Ctx, expect_async, expect_or};
use crate::identity::Identity;
use crate::oracle::{self, Peer};
use crate::pulse::PulseBot;

/// How long an island assignment may take to arrive: Pulse to the message bus to the connector to
/// the gatekeeper to the socket.
pub const ASSIGNMENT_DEADLINE: Duration = Duration::from_secs(20);
/// How long the statistics route may take to show what the sockets have said.
pub const STATISTICS_DEADLINE: Duration = Duration::from_secs(15);
const POLL: Duration = Duration::from_millis(250);

pub fn api(ctx: &Ctx) -> Result<&Url> {
    ctx.stack()
        .api
        .as_ref()
        .ok_or_else(|| anyhow!("the stack file has no API address (`api`)"))
}

/// A spot on the grid, in world coordinates, that this scenario repeat picked at random: two
/// calls with different `slot`s are far apart (further than an island reaches), and the spots of
/// one repeat differ from the spots of other runs, so bots of other users of the stack seldom
/// stand next to them.
pub fn spot(ctx: &Ctx, slot: u32) -> [f32; 3] {
    let grid = &ctx.stack().grid;
    let span_x = (grid.max_parcel_x - grid.min_parcel_x).max(8) as u64;
    let span_z = (grid.max_parcel_z - grid.min_parcel_z).max(8) as u64;
    // slot 0 in the first third of the grid, slot 1 in the second, and so on in thirds: far apart
    let third_x = span_x / 3;
    let third_z = span_z / 3;
    let x = grid.min_parcel_x as i64
        + (third_x * (slot as u64 % 3)) as i64
        + ctx.random_below(third_x.max(1)) as i64;
    let z = grid.min_parcel_z as i64
        + (third_z * (slot as u64 % 3)) as i64
        + ctx.random_below(third_z.max(1)) as i64;
    grid.parcel_centre(x as i32, z as i32)
}

/// `spot` moved a few metres: a neighbour on the same parcel.
pub fn near(spot: [f32; 3], step: f32) -> [f32; 3] {
    [spot[0] + step, spot[1], spot[2] + step]
}

/// A wallet's device on both services: an archipelago socket and a Pulse connection.
pub struct Device {
    pub archipelago: ArchipelagoBot,
    pub pulse: PulseBot,
}

/// Brings a device up in `realm` at `world`: Pulse first (handshake, teleport, first movement),
/// then the archipelago socket (handshake, first heartbeat).
pub async fn bring_up(
    ctx: &Ctx,
    label: &str,
    identity: &Identity,
    realm: &str,
    world: [f32; 3],
) -> Result<Device> {
    let pulse = pulse_up(ctx, &format!("{label}-pulse"), identity, realm, world).await?;
    let archipelago = ArchipelagoBot::open(ctx, &format!("{label}-archipelago"), identity).await?;
    archipelago.heartbeat(world);
    Ok(Device { archipelago, pulse })
}

/// A Pulse connection that has handshaken, announced `realm` and moved to `world`.
pub async fn pulse_up(
    ctx: &Ctx,
    name: &str,
    identity: &Identity,
    realm: &str,
    world: [f32; 3],
) -> Result<PulseBot> {
    let pulse = PulseBot::enter(ctx, name, identity).await?;
    pulse.teleport(realm, world)?;
    pulse.move_to(world)?;
    Ok(pulse)
}

/// The socket address of the API, for a fault link to relay to.
pub fn api_socket_address(ctx: &Ctx) -> Result<std::net::SocketAddr> {
    let url = api(ctx)?;
    url.socket_addrs(|| None)?
        .into_iter()
        .next()
        .ok_or_else(|| anyhow!("{url} names no address"))
}

/// Waits for the first island assignment of `bot`.
pub async fn first_assignment(ctx: &Ctx, bot: &ArchipelagoBot) -> Result<Assignment> {
    next_assignment(ctx, bot, 0).await
}

/// Waits until `bot` holds more than `already` assignments and returns the next one.
pub async fn next_assignment(
    ctx: &Ctx,
    bot: &ArchipelagoBot,
    already: usize,
) -> Result<Assignment> {
    let name = bot.name().to_owned();
    expect_or(
        ctx,
        &format!("{name} receives island assignment number {}", already + 1),
        ASSIGNMENT_DEADLINE,
        || {
            bot.alive()?;
            bot.assignments()
                .get(already)
                .cloned()
                .ok_or_else(|| "none yet".to_owned())
        },
    )
    .await
}

/// Checks that `assignment` names a room and carries a token for `identity`'s wallet, and returns
/// the room.
pub fn check_credentials(assignment: &Assignment, identity: &Identity) -> Result<String> {
    let connection = read_connection_string(&assignment.connection_string).map_err(|error| {
        anyhow!(
            "the connection string of island {}: {error}",
            assignment.island
        )
    })?;
    if connection.claims.room.is_empty() {
        bail!(
            "the token of island {} names an empty room",
            assignment.island
        );
    }
    if !connection
        .claims
        .identity
        .eq_ignore_ascii_case(&identity.address())
    {
        bail!(
            "the token of island {} is for {}, not for {}",
            assignment.island,
            connection.claims.identity,
            identity.address()
        );
    }
    let now = crate::identity::now_ms() as u64 / 1000;
    if connection.claims.expires_at <= now {
        bail!(
            "the token of island {} expired {} s ago",
            assignment.island,
            now - connection.claims.expires_at
        );
    }
    Ok(connection.claims.room)
}

/// The hook where a LiveKit client joins the room a connection string names. It checks the token
/// and returns the room; joining the room is the LiveKit bot's part and is added here.
pub async fn join_room(
    ctx: &Ctx,
    actor: &str,
    identity: &Identity,
    assignment: &Assignment,
) -> Result<String> {
    let room = check_credentials(assignment, identity)?;
    ctx.record(
        actor,
        "room named",
        serde_json::json!({ "room": room, "island": assignment.island }),
    );
    Ok(room)
}

/// Waits until the statistics route lists `address` exactly `count` times and returns those
/// entries.
pub async fn expect_peer_count(ctx: &Ctx, address: &str, count: usize) -> Result<Vec<Peer>> {
    let api = api(ctx)?.clone();
    expect_async(
        ctx,
        &format!("the statistics route lists {address} {count} time(s)"),
        STATISTICS_DEADLINE,
        POLL,
        || {
            let api = api.clone();
            async move {
                let peers = oracle::peers(&api)
                    .await
                    .map_err(|error| format!("{error:#}"))?;
                let held: Vec<Peer> = oracle::of_wallet(&peers, address)
                    .into_iter()
                    .cloned()
                    .collect();
                if held.len() == count {
                    Ok(held)
                } else {
                    Err(format!("it lists {} of them", held.len()))
                }
            }
        },
    )
    .await
}

/// Waits until `address` is in some island of the statistics route and returns its id.
pub async fn expect_island_of(ctx: &Ctx, address: &str) -> Result<String> {
    let api = api(ctx)?.clone();
    expect_async(
        ctx,
        &format!("{address} is in an island of the statistics route"),
        STATISTICS_DEADLINE,
        POLL,
        || {
            let api = api.clone();
            async move {
                let peers = oracle::peers(&api)
                    .await
                    .map_err(|error| format!("{error:#}"))?;
                oracle::of_wallet(&peers, address)
                    .into_iter()
                    .find_map(|peer| peer.island.clone())
                    .ok_or_else(|| "no island lists it".to_owned())
            }
        },
    )
    .await
}

/// An island as the statistics route names it. An assignment calls the same island
/// `island-<id>`.
pub fn island_key(id: &str) -> &str {
    id.strip_prefix("island-").unwrap_or(id)
}

/// Options for an archipelago bot that dials through a fault link.
pub fn via(link: &crate::fault::FaultLink) -> ArchipelagoOptions {
    ArchipelagoOptions {
        via: Some(link.address()),
        deadline: None,
    }
}
