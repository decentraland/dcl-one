//! A crowd that finds each other the way clients do, each layer judged by what every client was
//! itself told.
//!
//! 1. Discovery: every client reads the realm description (`<api>/about`) and dials the comms
//!    adapter it names. The Pulse address is the stack file's: a client is given it by its launch
//!    configuration, not by the realm.
//! 2. Pulse, the primary evidence: every client catches one `PlayerJoined` event for each other
//!    player of the crowd, in the run's own realm, and none for anyone else; then holds each at its
//!    position (see [`super::crowd`]).
//! 3. The archipelago: every client is given an island, all the same one. When the island
//!    message lists the island's peers, every client's list must hold the others.
//! 4. The island's room: the clients join the LiveKit room their assignment names (join only, no
//!    tracks) and every one must be told that each other player joined it.
//!
//! The crowd stands on one parcel near a corner of the grid, where visitors seldom are: the
//! archipelago groups players by position, and a visitor nearby would share the island and the
//! room. Anyone outside the crowd that a client hears of is reported, and fails the run.

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::time::{Duration, Instant};

use anyhow::{Result, anyhow, bail};
use futures::future::join_all;
use url::Url;

use super::connections_support::{api, api_socket_address, check_credentials, realm_description};
use super::crowd::{
    SIGHTING_DEADLINE, enter_crowd, note_joins, note_sightings, player_name, players_of_run, spot,
    watch,
};
use super::support::leave_all;
use crate::archipelago::{ArchipelagoBot, ArchipelagoOptions};
use crate::engine::{Ctx, Needs, Scenario, ScenarioFuture};
use crate::livekit::{JoinOptions, LiveKitJoin};
use crate::pulse::PulseBot;

/// The environment variable that sets how many of the crowd join the room: all of them unless
/// it says fewer, `0` for none. Each joining client is a process with its own WebRTC stack.
pub const ROOM_PLAYERS_VARIABLE: &str = "CE2E_CROWD_ROOM_PLAYERS";

/// The prefix of an archipelago adapter in a realm description.
const ARCHIPELAGO_ADAPTER: &str = "archipelago:archipelago:";

/// Archipelago handshakes in flight at once: the connector admits a few pending handshakes per
/// address (8 by default), and a crowd from one machine stays under that.
const ARCHIPELAGO_WAVE: usize = 4;

/// Room clients started at once.
const ROOM_WAVE: usize = 5;

/// How long, from the first dial, the island assignments and the room meetings may take.
const ISLAND_DEADLINE: Duration = Duration::from_secs(40);
const ROOM_DEADLINE: Duration = Duration::from_secs(40);

/// How often an archipelago client repeats its position while it waits.
const HEARTBEAT: Duration = Duration::from_secs(1);

const POLL: Duration = Duration::from_millis(50);

pub static CROWD_DISCOVERY: Scenario = Scenario {
    name: "crowd-discovery",
    tags: &["connections", "crowd"],
    guards: &["claim: 20 players in one place see each other"],
    summary: "A crowd (20, or CE2E_CROWD_PLAYERS) near a corner of the grid finds the comms adapter in the realm description, then each client must catch a Pulse join event for every other player and hold it at its place, be given the same island (whose peer list, when sent, holds the others) and be told each other player joined the island's room; notes give each layer per player",
    needs: Needs {
        api: true,
        ..Needs::NONE
    },
    hostile: false,
    default_repeat: 1,
    run: |ctx| -> ScenarioFuture { Box::pin(crowd_discovery(ctx)) },
};

/// The scenarios of this group, in the order a run takes them.
pub fn all() -> Vec<&'static Scenario> {
    vec![&CROWD_DISCOVERY]
}

/// The WebSocket address of an archipelago adapter as a realm description names it
/// (`archipelago:archipelago:wss://host/ws`).
pub fn adapter_socket(adapter: &str) -> Result<Url> {
    let Some(address) = adapter.strip_prefix(ARCHIPELAGO_ADAPTER) else {
        bail!("the realm names the adapter {adapter:?}, which is not an archipelago one");
    };
    let url = Url::parse(address)
        .map_err(|error| anyhow!("the adapter address {address:?} is not a URL: {error}"))?;
    if !matches!(url.scheme(), "ws" | "wss") {
        bail!("the adapter address {address:?} is not a WebSocket one");
    }
    Ok(url)
}

/// What one client of the crowd was told, layer by layer.
#[derive(Debug, Clone, Default)]
pub struct ClientView {
    pub wallet: String,
    /// The island of its latest assignment.
    pub island: Option<String>,
    /// The island's peers as its assignment and the join and leave messages after it listed them.
    pub island_peers: Vec<String>,
    /// Everyone the room told it had joined.
    pub room_participants: Vec<String>,
    /// How many of the other players it held on Pulse.
    pub pulse_held: usize,
}

fn others_of<'a>(views: &'a [ClientView], view: &ClientView) -> Vec<&'a str> {
    views
        .iter()
        .filter(|other| !other.wallet.eq_ignore_ascii_case(&view.wallet))
        .map(|other| other.wallet.as_str())
        .collect()
}

fn missing_from(listed: &[String], wanted: &[&str]) -> Vec<String> {
    wanted
        .iter()
        .filter(|wallet| !listed.iter().any(|held| held.eq_ignore_ascii_case(wallet)))
        .map(|wallet| (*wallet).to_owned())
        .collect()
}

/// Every client was given one island, the same one; when an island message listed its peers,
/// each list holds every other client. Returns the island.
pub fn island_verdict(views: &[ClientView]) -> Result<String, String> {
    let without: Vec<&str> = views
        .iter()
        .filter(|view| view.island.is_none())
        .map(|view| view.wallet.as_str())
        .collect();
    if !without.is_empty() {
        return Err(format!("no island was given to {without:?}"));
    }
    let mut islands: BTreeMap<&str, usize> = BTreeMap::new();
    for view in views {
        *islands
            .entry(view.island.as_deref().unwrap_or_default())
            .or_default() += 1;
    }
    if islands.len() != 1 {
        return Err(format!(
            "the crowd was given {} islands: {islands:?}",
            islands.len()
        ));
    }
    let mut short = Vec::new();
    for view in views.iter().filter(|view| !view.island_peers.is_empty()) {
        let missing = missing_from(&view.island_peers, &others_of(views, view));
        if !missing.is_empty() {
            short.push(format!("{}'s island lists no {missing:?}", view.wallet));
        }
    }
    if !short.is_empty() {
        return Err(short.join("; "));
    }
    Ok(islands.into_keys().next().unwrap_or_default().to_owned())
}

/// Every client was told that each other client joined the room.
pub fn room_verdict(views: &[ClientView]) -> Result<(), String> {
    let mut complete = 0;
    let mut short = Vec::new();
    for view in views {
        let missing = missing_from(&view.room_participants, &others_of(views, view));
        if missing.is_empty() {
            complete += 1;
        } else {
            short.push(format!("{} misses {}", view.wallet, missing.len()));
        }
    }
    if short.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "{complete} of {} clients were told that every other player joined the room; {}",
            views.len(),
            short.join(", ")
        ))
    }
}

/// Every client held every other client on Pulse.
pub fn pulse_verdict(views: &[ClientView]) -> Result<(), String> {
    let expected = views.len().saturating_sub(1);
    let complete = views
        .iter()
        .filter(|view| view.pulse_held == expected)
        .count();
    if complete == views.len() && expected > 0 {
        Ok(())
    } else {
        Err(format!(
            "{complete} of {} clients held every other player on Pulse",
            views.len()
        ))
    }
}

/// Everyone a client heard of, in its island or its room, who is not one of the crowd, sorted.
pub fn strangers(views: &[ClientView]) -> Vec<String> {
    let crowd: BTreeSet<String> = views
        .iter()
        .map(|view| view.wallet.to_lowercase())
        .collect();
    let mut found: BTreeSet<String> = BTreeSet::new();
    for view in views {
        for peer in view.island_peers.iter().chain(&view.room_participants) {
            if !crowd.contains(&peer.to_lowercase()) {
                found.insert(peer.clone());
            }
        }
    }
    found.into_iter().collect()
}

/// The LiveKit client the stack file names: `[livekit] client`, else `livekit_client`.
fn room_client(ctx: &Ctx) -> Option<PathBuf> {
    let stack = ctx.stack();
    stack
        .livekit
        .as_ref()
        .and_then(|target| target.client.clone())
        .or_else(|| stack.livekit_client.clone())
}

/// How many of `players` join the room: [`ROOM_PLAYERS_VARIABLE`], or all of them.
fn room_players(players: usize) -> Result<usize> {
    match std::env::var(ROOM_PLAYERS_VARIABLE) {
        Err(_) => Ok(players),
        Ok(text) => {
            let count: usize = text.trim().parse().map_err(|_| {
                anyhow!("{ROOM_PLAYERS_VARIABLE} must be a count of players, got {text:?}")
            })?;
            if count == 1 || count > players {
                bail!("{ROOM_PLAYERS_VARIABLE} must be 0, or between 2 and {players}");
            }
            Ok(count)
        }
    }
}

/// A spot near the south-west corner of the grid, the parcel two in from each edge.
fn corner(ctx: &Ctx) -> [f32; 3] {
    let grid = &ctx.stack().grid;
    grid.parcel_centre(grid.min_parcel_x + 2, grid.min_parcel_z + 2)
}

/// Every client reads the realm description and takes the adapter it names; all must agree.
async fn discover(ctx: &Ctx, players: usize) -> Result<Url> {
    let api = api(ctx)?.clone();
    let described = join_all((0..players).map(|_| realm_description(&api))).await;
    let mut adapters = BTreeSet::new();
    let mut realm_names = BTreeSet::new();
    for (index, description) in described.into_iter().enumerate() {
        let description = description.map_err(|error| {
            anyhow!(
                "{} could not read the realm description: {error}",
                player_name(index)
            )
        })?;
        adapters.insert(description.adapter);
        realm_names.insert(description.realm_name.unwrap_or_default());
    }
    ctx.note("realm_name", &realm_names);
    ctx.note("comms_adapter", &adapters);
    if adapters.len() != 1 {
        bail!("the clients were told of different adapters: {adapters:?}");
    }
    adapter_socket(adapters.first().expect("one adapter"))
}

/// The address a client dials for `url`: the API's own when `url` is the explorer's front for
/// it, which the stack advertises and relays only while an explorer runs.
fn dial_options(ctx: &Ctx, url: &Url) -> Result<ArchipelagoOptions> {
    let front = ctx
        .stack()
        .explorer
        .as_ref()
        .map(|explorer| explorer.api_front);
    let named = url.socket_addrs(|| None).ok().unwrap_or_default();
    let via = match front {
        Some(front) if named.contains(&front) => Some(api_socket_address(ctx)?),
        _ => None,
    };
    if let Some(real) = via {
        ctx.note(
            "adapter_dialed_through",
            format!("the realm names the explorer front {url}; dialed at {real}"),
        );
    }
    Ok(ArchipelagoOptions {
        via,
        deadline: None,
    })
}

/// Opens every client's archipelago socket, [`ARCHIPELAGO_WAVE`] handshakes at a time, each
/// followed by its first heartbeat.
async fn open_sockets(
    ctx: &Ctx,
    url: &Url,
    identities: &[crate::identity::Identity],
    spots: &[[f32; 3]],
) -> Result<Vec<ArchipelagoBot>> {
    let options = dial_options(ctx, url)?;
    let mut sockets = Vec::with_capacity(identities.len());
    let mut refused = Vec::new();
    let indices: Vec<usize> = (0..identities.len()).collect();
    for wave in indices.chunks(ARCHIPELAGO_WAVE) {
        let opened = join_all(wave.iter().map(|&index| {
            let options = options.clone();
            async move {
                let name = format!("{}-archipelago", player_name(index));
                ArchipelagoBot::connect(ctx, &name, url, &identities[index], options).await
            }
        }))
        .await;
        for (&index, outcome) in wave.iter().zip(opened) {
            match outcome {
                Ok(socket) => {
                    socket.heartbeat(spots[index]);
                    sockets.push(socket);
                }
                Err(error) => refused.push(format!("{}: {error:#}", player_name(index))),
            }
        }
    }
    ctx.note("archipelago_sockets_open", sockets.len());
    if !refused.is_empty() {
        let opened = sockets.len();
        for socket in sockets {
            socket.close();
        }
        bail!(
            "{} of {} archipelago sockets were not welcomed: {refused:?} ({opened} welcomed, closed again)",
            refused.len(),
            identities.len()
        );
    }
    Ok(sockets)
}

/// Waits until every socket's latest assignment names one island, repeating each position while
/// it waits, or until `deadline`.
async fn wait_for_one_island(sockets: &[ArchipelagoBot], spots: &[[f32; 3]], deadline: Instant) {
    let mut beat = Instant::now();
    loop {
        let latest: BTreeSet<Option<String>> = sockets
            .iter()
            .map(|socket| socket.assignments().last().map(|held| held.island.clone()))
            .collect();
        let one = latest.len() == 1 && latest.iter().all(Option::is_some);
        let ended = sockets.iter().any(|socket| socket.alive().is_err());
        if one || ended || Instant::now() >= deadline {
            return;
        }
        if beat.elapsed() >= HEARTBEAT {
            for (socket, spot) in sockets.iter().zip(spots) {
                socket.heartbeat(*spot);
            }
            beat = Instant::now();
        }
        tokio::time::sleep(POLL).await;
    }
}

/// Joins the first `count` clients to the room their latest assignment names, [`ROOM_WAVE`] at
/// a time, with no tracks.
async fn join_rooms(
    ctx: &Ctx,
    client: &std::path::Path,
    sockets: &[ArchipelagoBot],
    identities: &[crate::identity::Identity],
    count: usize,
) -> Result<Vec<LiveKitJoin>> {
    let mut rooms = BTreeSet::new();
    let mut connections = Vec::with_capacity(count);
    for (index, socket) in sockets.iter().take(count).enumerate() {
        let assignment = socket
            .assignments()
            .last()
            .cloned()
            .ok_or_else(|| anyhow!("{} has no island to join", player_name(index)))?;
        rooms.insert(check_credentials(&assignment, &identities[index])?);
        connections.push(assignment.connection_string);
    }
    ctx.note("rooms_named", &rooms);
    let mut joined = Vec::with_capacity(count);
    let mut failed = Vec::new();
    let indices: Vec<usize> = (0..count).collect();
    for wave in indices.chunks(ROOM_WAVE) {
        let started = join_all(wave.iter().map(|&index| {
            let options = JoinOptions {
                binary: Some(client.to_path_buf()),
                ..Default::default()
            };
            let connection = &connections[index];
            async move {
                let name = format!("{}-room", player_name(index));
                LiveKitJoin::start(ctx, &name, connection, options).await
            }
        }))
        .await;
        for (&index, outcome) in wave.iter().zip(started) {
            match outcome {
                Ok(client) => joined.push(client),
                Err(error) => failed.push(format!("{}: {error:#}", player_name(index))),
            }
        }
    }
    ctx.note("room_clients_connected", joined.len());
    if !failed.is_empty() {
        leave_rooms(&joined).await;
        bail!(
            "{} of {count} room clients did not connect: {failed:?}",
            failed.len()
        );
    }
    Ok(joined)
}

/// Everyone `client` was told joined its room, by identity, in arrival order without repeats.
fn room_arrivals(client: &LiveKitJoin) -> Vec<String> {
    let mut seen = Vec::new();
    for arrival in client.view().arrivals.into_iter().filter(|a| a.joined) {
        if !seen.contains(&arrival.identity) {
            seen.push(arrival.identity);
        }
    }
    seen
}

/// Waits until every client was told that every other one of `wallets` joined, or `deadline`.
/// Returns, per client, when that happened.
async fn wait_for_room_meetings(
    clients: &[LiveKitJoin],
    wallets: &[String],
    started: Instant,
    deadline: Instant,
) -> Vec<Option<u64>> {
    let mut met_at = vec![None; clients.len()];
    loop {
        for (index, client) in clients.iter().enumerate() {
            if met_at[index].is_some() {
                continue;
            }
            let arrivals = room_arrivals(client);
            let others: Vec<&str> = wallets
                .iter()
                .enumerate()
                .filter(|(other, _)| *other != index)
                .map(|(_, wallet)| wallet.as_str())
                .collect();
            if missing_from(&arrivals, &others).is_empty() {
                met_at[index] = Some(started.elapsed().as_millis() as u64);
            }
        }
        let ended = clients.iter().any(|client| client.alive().is_err());
        if met_at.iter().all(Option::is_some) || ended || Instant::now() >= deadline {
            return met_at;
        }
        tokio::time::sleep(POLL).await;
    }
}

async fn leave_rooms(clients: &[LiveKitJoin]) {
    join_all(clients.iter().map(LiveKitJoin::leave)).await;
}

/// Ends every connection of the crowd the way a client does.
async fn leave_everything(
    ctx: &Ctx,
    rooms: Vec<LiveKitJoin>,
    sockets: Vec<ArchipelagoBot>,
    bots: Vec<PulseBot>,
) -> Result<()> {
    leave_rooms(&rooms).await;
    drop(rooms);
    for socket in sockets {
        socket.close();
    }
    leave_all(ctx, bots).await
}

async fn crowd_discovery(ctx: Ctx) -> Result<()> {
    let players = players_of_run()?;
    let room_count = room_players(players)?;
    let client = room_client(&ctx);
    if room_count > 0 && client.is_none() {
        bail!(
            "the stack file names no LiveKit client (`[livekit] client` or `livekit_client`); set {ROOM_PLAYERS_VARIABLE}=0 to leave the room out"
        );
    }
    let realm = ctx.realm("crowd");
    let here = corner(&ctx);
    let identities: Vec<_> = (0..players)
        .map(|index| ctx.identity(index as u32))
        .collect();
    let wallets: Vec<String> = identities
        .iter()
        .map(|identity| identity.address())
        .collect();
    let spots: Vec<[f32; 3]> = (0..players).map(|index| spot(here, index)).collect();
    ctx.note("players", players);
    ctx.note("room_players", room_count);
    ctx.note("transport", format!("{:?}", ctx.transport()));
    ctx.note("place", here);

    // 1. the adapter, from the realm description
    let started = Instant::now();
    let adapter = discover(&ctx, players).await?;
    ctx.note("discovery_ms", started.elapsed().as_millis() as u64);

    // 2. Pulse: everyone at once, then the archipelago sockets while Pulse tells of the joins
    let bots = enter_crowd(&ctx, &identities, &realm, &spots).await?;
    for (bot, spot) in bots.iter().zip(&spots) {
        bot.move_to(*spot)?;
    }
    ctx.note("pulse_entered_ms", started.elapsed().as_millis() as u64);
    let sockets = match open_sockets(&ctx, &adapter, &identities, &spots).await {
        Ok(sockets) => sockets,
        Err(error) => {
            leave_all(&ctx, bots).await?;
            return Err(error);
        }
    };
    ctx.note("archipelago_open_ms", started.elapsed().as_millis() as u64);

    let watched = watch(
        &bots,
        &wallets,
        &spots,
        &realm,
        started,
        started + SIGHTING_DEADLINE,
    )
    .await;
    let (_, joins) = note_joins(&ctx, &wallets, &realm);
    let held = match &watched {
        Ok(sightings) => note_sightings(&ctx, sightings),
        Err(error) => Err(format!("{error:#}")),
    };

    // 3. the island each client was given
    wait_for_one_island(&sockets, &spots, started + ISLAND_DEADLINE).await;
    let island_at: Vec<Option<u64>> = sockets
        .iter()
        .map(|socket| socket.assignments().last().map(|held| held.at_ms))
        .collect();
    ctx.note("island_assigned_ms_by_player", &island_at);

    // 4. the island's room
    let rooms = match (&client, room_count) {
        (Some(client), count) if count > 0 => {
            match join_rooms(&ctx, client, &sockets, &identities, count).await {
                Ok(rooms) => rooms,
                Err(error) => {
                    leave_everything(&ctx, Vec::new(), sockets, bots).await?;
                    return Err(error);
                }
            }
        }
        _ => Vec::new(),
    };
    if !rooms.is_empty() {
        let met = wait_for_room_meetings(
            &rooms,
            &wallets[..room_count],
            started,
            Instant::now() + ROOM_DEADLINE,
        )
        .await;
        ctx.note("room_met_everyone_ms_by_player", &met);
    }

    // what each client was told, layer by layer
    let held_by_player: Vec<usize> = match &watched {
        Ok(sightings) => sightings.seen.clone(),
        Err(_) => vec![0; players],
    };
    let views: Vec<ClientView> = (0..players)
        .map(|index| {
            let socket_view = sockets[index].view();
            ClientView {
                wallet: wallets[index].clone(),
                island: socket_view
                    .assignments
                    .last()
                    .map(|held| held.island.clone()),
                island_peers: socket_view.island_peers,
                room_participants: rooms.get(index).map(room_arrivals).unwrap_or_default(),
                pulse_held: held_by_player[index],
            }
        })
        .collect();
    let crowd_index: BTreeMap<String, usize> = wallets
        .iter()
        .enumerate()
        .map(|(index, wallet)| (wallet.to_lowercase(), index))
        .collect();
    ctx.note(
        "island_by_player",
        views
            .iter()
            .map(|view| view.island.clone())
            .collect::<Vec<_>>(),
    );
    ctx.note(
        "island_peers_listed_by_player",
        views
            .iter()
            .map(|view| view.island_peers.len())
            .collect::<Vec<_>>(),
    );
    ctx.note(
        "room_joins_caught_by_player",
        views
            .iter()
            .take(room_count)
            .map(|view| view.room_participants.len())
            .collect::<Vec<_>>(),
    );
    // per room client, the crowd players it was told joined, by index
    ctx.note(
        "room_joins_by_player",
        views
            .iter()
            .take(room_count)
            .map(|view| {
                view.room_participants
                    .iter()
                    .filter_map(|peer| crowd_index.get(&peer.to_lowercase()).copied())
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>(),
    );
    let outsiders = strangers(&views);
    ctx.note("others_outside_the_crowd", outsiders.len());

    let island = island_verdict(&views);
    if let Ok(island) = &island {
        ctx.note("island", island);
    }
    let room = if room_count > 0 {
        room_verdict(&views[..room_count])
    } else {
        Ok(())
    };
    let left = leave_everything(&ctx, rooms, sockets, bots).await;

    // the join events first: they are the direct evidence; then the rest in layer order
    if let Err(joins) = joins {
        bail!("join events: {joins}");
    }
    if let Err(held) = held {
        bail!("held state: {held}");
    }
    if !outsiders.is_empty() {
        bail!(
            "{} players outside the crowd were in the island or the room: the place was not the crowd's alone, so the run is not clean",
            outsiders.len()
        );
    }
    island.map_err(|error| anyhow!("island: {error}"))?;
    room.map_err(|error| anyhow!("room: {error}"))?;
    left
}
