//! Helpers the connection scenarios share: where bots stand, how a device (an archipelago socket
//! and a Pulse connection of one wallet) is brought up, and what to wait for.

use std::time::Duration;

use anyhow::{Result, anyhow, bail};
use url::Url;

use crate::archipelago::{ArchipelagoBot, ArchipelagoOptions, Assignment, read_connection_string};
use crate::engine::{Ctx, expect_async, expect_or};
use crate::gatekeeper::{Gatekeeper, GatekeeperError};
use crate::identity::Identity;
use crate::livekit::{JoinOptions, LiveKitJoin};
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

/// A room an assignment named, and the client that joined it when the stack file names one.
pub struct JoinedRoom {
    pub room: String,
    /// Absent when the stack file names no LiveKit client: the token was checked, the room was
    /// not joined.
    pub client: Option<LiveKitJoin>,
}

/// Checks the token of an assignment and, when the stack file names a LiveKit client, joins the
/// room with it: the proof that the credentials the stack hands out really open the room.
pub async fn join_room(
    ctx: &Ctx,
    actor: &str,
    identity: &Identity,
    assignment: &Assignment,
) -> Result<JoinedRoom> {
    let room = check_credentials(assignment, identity)?;
    ctx.record(
        actor,
        "room named",
        serde_json::json!({ "room": room, "island": assignment.island }),
    );
    let has_client = ctx
        .stack()
        .livekit
        .as_ref()
        .is_some_and(|target| target.client.is_some());
    let client = if has_client {
        let name = format!("{actor} in the room");
        Some(
            LiveKitJoin::start(
                ctx,
                &name,
                &assignment.connection_string,
                JoinOptions::default(),
            )
            .await?,
        )
    } else {
        None
    };
    Ok(JoinedRoom { room, client })
}

/// Waits until two clients that joined the same room each hold the other, by wallet.
pub async fn expect_clients_meet(
    ctx: &Ctx,
    one: (&LiveKitJoin, &Identity),
    other: (&LiveKitJoin, &Identity),
) -> Result<()> {
    let holds = |client: &LiveKitJoin, wallet: String| {
        client
            .view()
            .participants
            .keys()
            .any(|identity| identity.eq_ignore_ascii_case(&wallet))
    };
    expect_or(
        ctx,
        "the two clients see each other in the LiveKit room",
        Duration::from_secs(20),
        || {
            one.0.alive()?;
            other.0.alive()?;
            let first_sees = holds(one.0, other.1.address());
            let second_sees = holds(other.0, one.1.address());
            (first_sees && second_sees).then_some(()).ok_or_else(|| {
                format!("the first holds the second: {first_sees}; the second holds the first: {second_sees}")
            })
        },
    )
    .await
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

/// A platform-wide ban that is lifted on every way out of a scenario: the explicit
/// [`BanGuard::lift`] on the normal path, and this guard's `Drop` on every other (an early `?`, a
/// panic, or the runner dropping the future at the scenario deadline).
///
/// `Drop` cannot await, so it makes the request on a helper thread with its own runtime and
/// client, and waits for that thread: when the guard is gone the request has completed (or
/// failed, which is recorded), and the process does not move on with the ban in place.
pub struct BanGuard {
    ctx: Ctx,
    address: String,
    lifted: bool,
}

impl BanGuard {
    /// Bans `address`. The guard exists before the request is made: a ban whose answer was lost
    /// may still have been applied.
    pub async fn place(
        ctx: &Ctx,
        gatekeeper: &Gatekeeper,
        address: &str,
        reason: &str,
    ) -> Result<BanGuard> {
        let guard = BanGuard {
            ctx: ctx.clone(),
            address: address.to_owned(),
            lifted: false,
        };
        gatekeeper
            .ban(address, reason)
            .await
            .map_err(|error| anyhow!("banning the wallet: {error}"))?;
        ctx.record("moderator", "banned", address);
        Ok(guard)
    }

    /// Lifts the ban and waits for the answer. A ban that is not there (404) is lifted.
    pub async fn lift(mut self) -> Result<(), GatekeeperError> {
        self.lifted = true;
        let gatekeeper = Gatekeeper::new(&self.ctx)
            .map_err(|error| GatekeeperError::Unexpected(error.to_string()))?;
        let lifted = gatekeeper.unban(&self.address).await;
        self.ctx
            .record("moderator", "unban answered", format!("{lifted:?}"));
        match lifted {
            Err(error) if error.status() == Some(404) => Ok(()),
            other => other,
        }
    }
}

impl Drop for BanGuard {
    fn drop(&mut self) {
        if self.lifted {
            return;
        }
        let ctx = self.ctx.clone();
        let address = self.address.clone();
        let helper = std::thread::Builder::new()
            .name("ban-lift".to_owned())
            .spawn(move || {
                let runtime = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .map_err(|error| error.to_string())?;
                runtime.block_on(async {
                    let gatekeeper = Gatekeeper::new(&ctx).map_err(|error| error.to_string())?;
                    gatekeeper
                        .unban(&address)
                        .await
                        .map_err(|error| error.to_string())
                })
            });
        let outcome = match helper {
            Ok(handle) => handle
                .join()
                .unwrap_or_else(|_| Err("the helper thread panicked".to_owned())),
            Err(error) => Err(format!("no helper thread: {error}")),
        };
        self.ctx
            .record("moderator", "unban on the way out", format!("{outcome:?}"));
    }
}
