//! `NODE_POSTURE`: how this node stands toward Decentraland, as one name that
//! expands into the defaults of every domain's sync, mirror, index, upstream,
//! cache and write keys before the arguments are parsed. A variable set in the
//! environment always wins over its posture default, even when empty (an
//! empty URL is no upstream); a `<NAME>_FILE` counts as set. `catalyst
//! posture show <name>` prints the expansion; this table is its only source.
//!
//! - `mirror` follows Decentraland: peer sync, the mirrors, the chain indexes
//!   and polls, the RPC relay, the asset-bundle CDN and OpenSea fallbacks.
//! - `island` sends no request to Decentraland or any third party: no sync,
//!   mirror, index, poll, relay or fallback, and the map URLs are its own.
//! - `journey` is the scratch-database test stack: an island with every
//!   write domain open and no restore stamp.

use std::fmt::Write as _;

use crate::config::Config;

pub const ENV: &str = "NODE_POSTURE";

/// `WRITES_RESTORE_STAMP_DIR`: where the backup and restore-drill stamps the
/// write gate reads live (the write gate's own key).
pub const RESTORE_STAMP_DIR: &str = "WRITES_RESTORE_STAMP_DIR";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Posture {
    Mirror,
    Island,
    Journey,
}

impl Posture {
    pub const ALL: [Posture; 3] = [Posture::Mirror, Posture::Island, Posture::Journey];

    pub fn name(self) -> &'static str {
        match self {
            Posture::Mirror => "mirror",
            Posture::Island => "island",
            Posture::Journey => "journey",
        }
    }

    pub fn parse(raw: &str) -> anyhow::Result<Self> {
        Self::ALL
            .into_iter()
            .find(|p| p.name() == raw.trim())
            .ok_or_else(|| anyhow::anyhow!("{ENV}={raw:?} is not one of mirror, island, journey"))
    }

    fn summary(self) -> &'static str {
        match self {
            Posture::Mirror => {
                "follows Decentraland: peer sync, mirrors, chain indexes and polls, the RPC relay and \
                 the CDN fallbacks; local writes open behind the restore stamp"
            }
            Posture::Island => {
                "no request to Decentraland or any third party; this node is its own authority and \
                 its map URLs are its own; local writes open behind the restore stamp"
            }
            Posture::Journey => {
                "the scratch-database test stack: no upstream traffic, every write domain open, no \
                 restore stamp"
            }
        }
    }

    /// Mirror and island hold real data: a write switch needs the restore stamp.
    pub fn writes_need_restore_stamp(self) -> bool {
        self != Posture::Journey
    }
}

/// What the environment holds for a key (`None`: unset).
pub type Lookup<'a> = dyn Fn(&str) -> Option<String> + 'a;

#[derive(Clone, Copy)]
enum Value {
    /// This literal.
    Is(&'static str),
    /// Left unset: the binary derives this node's own URL (the text says which).
    Own(&'static str),
    /// Computed from the environment (and the keys expanded before it); the
    /// text is what `show` prints. `None` leaves the key unset.
    From(&'static str, fn(&Lookup) -> Option<String>),
}

struct Row {
    family: &'static str,
    key: &'static str,
    mirror: Value,
    island: Value,
    journey: Value,
}

impl Row {
    fn value(&self, posture: Posture) -> Value {
        match posture {
            Posture::Mirror => self.mirror,
            Posture::Island => self.island,
            Posture::Journey => self.journey,
        }
    }
}

const fn row(family: &'static str, key: &'static str, m: Value, i: Value, j: Value) -> Row {
    Row {
        family,
        key,
        mirror: m,
        island: i,
        journey: j,
    }
}

/// On for mirror, off otherwise.
const fn mirrored(family: &'static str, key: &'static str) -> Row {
    row(family, key, Is("true"), Is("false"), Is("false"))
}

/// The mirror's upstream; island and journey have none.
const fn upstream(family: &'static str, key: &'static str, url: &'static str) -> Row {
    row(family, key, Is(url), Is(""), Is(""))
}

/// Shut under mirror and island until the operator opens it (behind the
/// restore stamp); open on the journey stack's scratch database.
const fn write(key: &'static str) -> Row {
    row("writes", key, Is("false"), Is("false"), Is("true"))
}

const fn same(family: &'static str, key: &'static str, value: &'static str) -> Row {
    row(family, key, Is(value), Is(value), Is(value))
}

use Value::{From, Is, Own};

const DCL_PEERS: &str = "https://peer.decentraland.org/content,https://peer-eu1.decentraland.org/content,https://peer.dclnodes.io/content";
const DCL_RPC: &str = "https://rpc.decentraland.org";
/// The Polygon CreditsManager the credits usage index reads (mainnet).
const CREDITS_MANAGER_MAINNET: &str = "0x8b3a40ca1b6f5cafc99d112a4d02e897d1fd8cc5";
const PROFILE_IMAGES_CACHE_BYTES: &str = "5368709120";

fn set(lookup: &Lookup, key: &str) -> Option<String> {
    lookup(key).filter(|v| !v.trim().is_empty())
}

/// The chain endpoints and contracts the mirror preset knows are mainnet's.
fn mainnet(lookup: &Lookup) -> bool {
    lookup("ETH_NETWORK").is_none_or(|n| n.trim().is_empty() || n.trim() == "mainnet")
}

fn eth_rpc(lookup: &Lookup) -> Option<String> {
    mainnet(lookup).then(|| format!("{DCL_RPC}/mainnet"))
}

fn polygon_rpc(lookup: &Lookup) -> Option<String> {
    mainnet(lookup).then(|| format!("{DCL_RPC}/polygon"))
}

fn credits_manager(lookup: &Lookup) -> Option<String> {
    mainnet(lookup).then(|| CREDITS_MANAGER_MAINNET.to_string())
}

/// History pruning drops superseded peer deployments; a node taking local
/// deploys keeps its history.
fn prune_unless_content_writes(lookup: &Lookup) -> Option<String> {
    let writes = lookup("CONTENT_WRITES_ENABLED").is_some_and(|v| v.trim() == "true");
    Some((!writes).to_string())
}

const ROWS: &[Row] = &[
    mirrored("sync", "SYNC_ENABLED"),
    row(
        "sync",
        "SERVE_UNSYNCED",
        Is("false"),
        Is("true"),
        Is("true"),
    ),
    upstream("sync", "SYNC_SOURCE", DCL_PEERS),
    mirrored("sync", "READ_THROUGH_ENABLED"),
    same("sync", "PHASED_SYNC", "true"),
    row(
        "sync",
        "CONTENT_HISTORY_PRUNE_ENABLED",
        From(
            "true, false while CONTENT_WRITES_ENABLED=true",
            prune_unless_content_writes,
        ),
        Is("false"),
        Is("false"),
    ),
    mirrored("mirrors", "EXPLORER_MIRRORS_ENABLED"),
    mirrored("mirrors", "PLACES_MIRROR_UPSTREAM"),
    mirrored("mirrors", "EVENTS_MIRROR_UPSTREAM"),
    mirrored("mirrors", "WORLDS_MIRROR_UPSTREAM"),
    mirrored("mirrors", "WORLDS_CONTENT_MIRROR_ENABLED"),
    mirrored("mirrors", "WORLDS_READ_THROUGH_TRUST_UPSTREAM_GRANTS"),
    mirrored("mirrors", "COMMUNITIES_MIRROR_UPSTREAM"),
    upstream(
        "mirrors",
        "PLACES_UPSTREAM_URL",
        "https://places.decentraland.org",
    ),
    upstream(
        "mirrors",
        "WORLDS_UPSTREAM_URL",
        "https://places.decentraland.org",
    ),
    upstream(
        "mirrors",
        "EVENTS_UPSTREAM_URL",
        "https://events.decentraland.org",
    ),
    upstream(
        "mirrors",
        "EVENTS_POSTER_UPSTREAM_URL",
        "https://events-assets-099ac00.decentraland.org",
    ),
    upstream(
        "mirrors",
        "WORLDS_CONTENT_MIRROR_SOURCE",
        "https://worlds-content-server.decentraland.org",
    ),
    upstream(
        "mirrors",
        "COMMUNITIES_UPSTREAM_URL",
        "https://social-api.decentraland.org",
    ),
    upstream(
        "mirrors",
        "COMMUNITIES_MIRROR_THUMBNAIL_URL",
        "https://assets-cdn.decentraland.org",
    ),
    upstream(
        "mirrors",
        "EXPLORER_LISTS_SOURCE_URL",
        "https://dcl-lists.decentraland.org",
    ),
    upstream(
        "mirrors",
        "EXPLORER_DENYLIST_SOURCE_URL",
        "https://config.decentraland.org/denylist.json",
    ),
    upstream(
        "mirrors",
        "EXPLORER_FEATURE_FLAGS_SOURCE_URL",
        "https://feature-flags.decentraland.org/explorer.json",
    ),
    upstream(
        "mirrors",
        "EXPLORER_MAP_SOURCE_URL",
        "https://api.decentraland.org",
    ),
    upstream(
        "mirrors",
        "WORLD_MANIFEST_UPSTREAM_URL",
        "https://places-dcf8abb.s3.amazonaws.com/WorldManifest.json",
    ),
    mirrored("mirrors", "MARKETPLACE_MIRRORS_ENABLED"),
    upstream(
        "mirrors",
        "TRADES_SYNC_UPSTREAM_URL",
        "https://marketplace-api.decentraland.org/v1/trades",
    ),
    // Development only: no posture serves an order book before its first complete pass.
    same("mirrors", "TRADES_SERVE_PARTIAL", "false"),
    upstream(
        "mirrors",
        "ETH_COLLECTIONS_SUBGRAPH_URL",
        "https://subgraph.decentraland.org/collections-ethereum-mainnet",
    ),
    upstream(
        "mirrors",
        "MATIC_COLLECTIONS_SUBGRAPH_URL",
        "https://subgraph.decentraland.org/collections-matic-mainnet",
    ),
    upstream(
        "mirrors",
        "MARKETPLACE_SUBGRAPH_URL",
        "https://subgraph.decentraland.org/marketplace",
    ),
    mirrored("indexes", "MARKETPLACE_NFT_INDEX_ENABLED"),
    mirrored("indexes", "MARKETPLACE_TRADE_EVENTS_INDEX_ENABLED"),
    // The MANA transfer walk stays an operator's opt-in under every posture.
    same(
        "indexes",
        "MARKETPLACE_MANA_TRANSFERS_INDEX_ENABLED",
        "false",
    ),
    mirrored("indexes", "LAND_AUTHZ_INDEX_ENABLED"),
    mirrored("indexes", "CREDITS_USAGE_INDEX_ENABLED"),
    row(
        "indexes",
        "CREDITS_MANAGER_CONTRACT",
        From(
            "0x8b3a40ca1b6f5cafc99d112a4d02e897d1fd8cc5 on mainnet",
            credits_manager,
        ),
        Is(""),
        Is(""),
    ),
    mirrored("polls", "PRICE_POLL_ENABLED"),
    upstream("polls", "COINGECKO_URL", "https://api.coingecko.com/api/v3"),
    upstream(
        "polls",
        "PRICE_FALLBACK_SOURCES",
        "coinbase=https://api.coinbase.com,binance=https://api.binance.com,kraken=https://api.kraken.com",
    ),
    row(
        "upstreams",
        "ETH_RPC_URL",
        From("https://rpc.decentraland.org/mainnet on mainnet", eth_rpc),
        Is(""),
        Is(""),
    ),
    row(
        "upstreams",
        "RPC_ENDPOINT_POLYGON",
        From(
            "https://rpc.decentraland.org/polygon on mainnet",
            polygon_rpc,
        ),
        Is(""),
        Is(""),
    ),
    // Derived once the secret files are read (RPC_ENDPOINT_POLYGON_FILE):
    // `derive_after_secret_files`.
    row(
        "upstreams",
        "TRADE_RPC_URLS",
        Own("1=<ETH_RPC_URL>,137=<RPC_ENDPOINT_POLYGON> list, once the secret files are read"),
        Is(""),
        Is(""),
    ),
    upstream(
        "upstreams",
        "RPC_UPSTREAM_MAINNET",
        "https://rpc.decentraland.org/mainnet",
    ),
    upstream(
        "upstreams",
        "RPC_UPSTREAM_ETHEREUM",
        "https://rpc.decentraland.org/mainnet",
    ),
    upstream(
        "upstreams",
        "RPC_UPSTREAM_SEPOLIA",
        "https://rpc.decentraland.org/sepolia",
    ),
    upstream(
        "upstreams",
        "RPC_UPSTREAM_POLYGON",
        "https://rpc.decentraland.org/polygon",
    ),
    upstream(
        "upstreams",
        "RPC_UPSTREAM_MATIC",
        "https://rpc.decentraland.org/polygon",
    ),
    upstream(
        "upstreams",
        "RPC_UPSTREAM_AMOY",
        "https://rpc.decentraland.org/amoy",
    ),
    upstream(
        "upstreams",
        "ABGEN_UPSTREAM_AB_CDN",
        "https://ab-cdn.decentraland.org",
    ),
    // Avatar faces of profiles whose snapshot files this node does not hold
    // (current clients deploy none) when no renderer runs here.
    upstream(
        "upstreams",
        "PROFILE_IMAGES_ORIGIN_URL",
        "https://profile-images.decentraland.org",
    ),
    upstream(
        "upstreams",
        "EXPLORER_REGISTRY_SOURCE_URL",
        "https://asset-bundle-registry.decentraland.org",
    ),
    upstream(
        "upstreams",
        "OPENSEA_FALLBACK_URL",
        "https://opensea.decentraland.org",
    ),
    upstream(
        "upstreams",
        "OPENSEA_CRYPTOKITTIES_API",
        "https://api.cryptokitties.co",
    ),
    upstream(
        "upstreams",
        "OPENSEA_IPFS_GATEWAYS",
        "https://ipfs.io/ipfs/,https://dweb.link/ipfs/,https://w3s.link/ipfs/,https://nftstorage.link/ipfs/",
    ),
    upstream(
        "upstreams",
        "LEASE_AUTHORIZATIONS_URL",
        "https://decentraland.github.io/linker-server-authorizations/authorizations.json",
    ),
    upstream(
        "upstreams",
        "BUILDER_FOUNDATION_URL",
        "https://builder-api.decentraland.org",
    ),
    // `/convert` fetches a client-named URL only on a mirror.
    mirrored("upstreams", "MEDIA_CONVERT_REMOTE_SOURCES"),
    // Only the scratch journey stack runs on the LiveKit dev pair, which
    // never calls a LiveKit API (`configured = false`); an island holds real
    // data and its own SFU, so it names its credentials as a mirror does.
    row(
        "upstreams",
        "LIVEKIT_ALLOW_DEV_CREDS",
        Is("false"),
        Is("false"),
        Is("true"),
    ),
    // Forwarding local deploys upstream is the operator's call under every posture.
    same("upstreams", "CONTENT_FORWARD_UPSTREAM_URL", ""),
    same("upstreams", "WORLDS_FORWARD_UPSTREAM_URL", ""),
    // The ADR-250 scheme (`{base}/{zoom}/{x},{y}.jpg`) is genesis.city's; this
    // node's `/satellite/{z}/{x}/{y}` pyramid is another, so an island or
    // journey /about advertises no satellite view.
    upstream(
        "map",
        "MAP_SATELLITE_BASE_URL",
        "https://genesis.city/map/latest",
    ),
    row(
        "map",
        "MAP_PARCEL_VIEW_URL",
        Own("<PUBLIC_URL>/api/v1/minimap.png"),
        Own("<PUBLIC_URL>/api/v1/minimap.png"),
        Own("<PUBLIC_URL>/api/v1/minimap.png"),
    ),
    row(
        "map",
        "MAP_ESTATE_VIEW_URL",
        Own("<PUBLIC_URL>/api/v1/estatemap.png"),
        Own("<PUBLIC_URL>/api/v1/estatemap.png"),
        Own("<PUBLIC_URL>/api/v1/estatemap.png"),
    ),
    row(
        "map",
        "LAND_IMAGE_BASE_URL",
        Own("<PUBLIC_URL>/api"),
        Own("<PUBLIC_URL>/api"),
        Own("<PUBLIC_URL>/api"),
    ),
    same(
        "caches",
        "PROFILE_IMAGES_CACHE_MAX_BYTES",
        PROFILE_IMAGES_CACHE_BYTES,
    ),
    // A translation backend is a third party or a local service the operator names.
    same("caches", "TRANSLATE_BACKEND", ""),
    write("CONTENT_WRITES_ENABLED"),
    write("CREATE_WRITES_ENABLED"),
    write("DATA_WRITES_ENABLED"),
    write("SOCIAL_WRITES_ENABLED"),
    write("WORLDS_WRITES_ENABLED"),
    write("OPS_WRITES_ENABLED"),
    // PRESENCE_COLLECTOR_ENABLED stays unset: the headcount archive writes
    // to the database, so it follows DATA_WRITES_ENABLED.
];

/// The keys a posture sets, in table order.
pub fn keys() -> impl Iterator<Item = &'static str> {
    ROWS.iter().map(|r| r.key)
}

/// The variables a posture sets, in table order.
pub type Expansion = Vec<(&'static str, String)>;

/// The variables `posture` sets, given what the environment holds: every row
/// whose key is unset (an empty value counts as set) and whose `<KEY>_FILE`
/// is unset or empty, evaluated in table order so a computed value sees the
/// keys expanded before it.
pub fn expand(posture: Posture, env: &Lookup) -> Expansion {
    let mut out = Expansion::new();
    for row in ROWS {
        if env(row.key).is_some() || set(env, &format!("{}_FILE", row.key)).is_some() {
            continue;
        }
        let seen = |key: &str| {
            env(key).or_else(|| out.iter().find(|(k, _)| *k == key).map(|(_, v)| v.clone()))
        };
        let value = match row.value(posture) {
            Is(v) => Some(v.to_string()),
            Own(_) => None,
            From(_, f) => f(&seen),
        };
        if let Some(value) = value {
            out.push((row.key, value));
        }
    }
    out
}

/// Reads `NODE_POSTURE` and returns it with the variables it adds; `None`
/// when unset (the binary's own defaults, every upstream and write off).
pub fn resolve(env: &Lookup) -> anyhow::Result<Option<(Posture, Expansion)>> {
    let Some(raw) = env(ENV).filter(|v| !v.trim().is_empty()) else {
        return Ok(None);
    };
    let posture = Posture::parse(&raw)?;
    Ok(Some((posture, expand(posture, env))))
}

/// Expands `NODE_POSTURE` into this process's environment. Runs first in
/// `main`, before any other thread exists and before clap reads the
/// environment.
pub fn apply_to_process_env() -> anyhow::Result<Option<Posture>> {
    let env = |key: &str| std::env::var_os(key).map(|v| v.to_string_lossy().into_owned());
    let Some((posture, vars)) = resolve(&env)? else {
        return Ok(None);
    };
    for (key, value) in vars {
        // SAFETY: called from `main` before the runtime or any other thread starts.
        unsafe { std::env::set_var(key, value) };
    }
    Ok(Some(posture))
}

/// `catalyst posture show <name>`: the expansion as `KEY=value` lines by
/// family; a computed value prints its rule, and a key left to the binary
/// prints as a comment naming the URL it derives.
pub fn show(posture: Posture) -> String {
    let mut out = format!("# {ENV}={}: {}\n", posture.name(), posture.summary());
    out.push_str(
        "# Every variable set in the environment wins, even empty; <NAME>_FILE counts as set.\n",
    );
    let mut family = "";
    for row in ROWS {
        if row.family != family {
            family = row.family;
            let _ = writeln!(out, "\n# {family}");
        }
        let _ = match row.value(posture) {
            Is(v) => writeln!(out, "{}={v}", row.key),
            Own(how) => writeln!(out, "# {} unset: this node's own {how}", row.key),
            From(how, _) => writeln!(out, "{}=<{how}>", row.key),
        };
    }
    out
}

/// `catalyst posture env <name> [KEY...]`: the `KEY=value` lines `posture`
/// sets in an empty environment, limited to `keys` when any are named.
pub fn env_file(posture: Posture, keys: &[String]) -> anyhow::Result<String> {
    if let Some(unknown) = keys
        .iter()
        .find(|k| !ROWS.iter().any(|r| r.key == k.as_str()))
    {
        anyhow::bail!("{unknown} is not a key of the posture table (`catalyst posture show`)");
    }
    let mut out = String::new();
    for (key, value) in expand(posture, &|_| None) {
        if keys.is_empty() || keys.iter().any(|k| k == key) {
            let _ = writeln!(out, "{key}={value}");
        }
    }
    Ok(out)
}

/// The line serve and the index process log at startup: the posture and
/// what it means, or that none is set.
pub fn startup_line(config: &Config) -> String {
    match config
        .node
        .node_posture
        .as_deref()
        .filter(|p| !p.trim().is_empty())
        .map(Posture::parse)
    {
        None => format!(
            "{ENV} is unset: no preset, so every upstream is off unless set explicitly \
             (`catalyst posture show mirror` lists them)"
        ),
        Some(Ok(posture)) => format!("{ENV}={}: {}", posture.name(), posture.summary()),
        Some(Err(err)) => err.to_string(),
    }
}

/// The combinations serve and the indexer refuse to start with. `env` is the
/// process environment (`NODE_POSTURE` and the write gate's stamp directory).
pub fn check(config: &Config, env: &Lookup) -> anyhow::Result<()> {
    let from_env = env(ENV).filter(|v| !v.trim().is_empty());
    let posture = match (&config.node.node_posture, &from_env) {
        (Some(flag), None) => anyhow::bail!(
            "--node-posture {flag} was given on the command line: the posture expands before the \
             arguments are parsed, so it is read from the {ENV} environment variable alone"
        ),
        (_, Some(raw)) => Some(Posture::parse(raw)?),
        (None, None) => None,
    };
    if let Some(posture) = posture.filter(|p| *p != Posture::Mirror)
        && config.node.sync_enabled
    {
        anyhow::bail!(
            "{ENV}={} syncs from no peer, but SYNC_ENABLED=true: use {ENV}=mirror to follow peers",
            posture.name()
        );
    }
    if config.node.sync_enabled && config.content.sync_source.trim().is_empty() {
        anyhow::bail!("SYNC_ENABLED=true with an empty SYNC_SOURCE: there is no peer to sync from");
    }
    let Some(posture) = posture else {
        return Ok(());
    };
    if posture != Posture::Mirror {
        let name = posture.name();
        for (key, value) in [
            (
                "CONTENT_FORWARD_UPSTREAM_URL",
                &config.writes.content_forward_upstream_url,
            ),
            (
                "WORLDS_FORWARD_UPSTREAM_URL",
                &config.worlds.worlds_forward_upstream_url,
            ),
        ] {
            if Config::non_empty(value).is_some() {
                anyhow::bail!(
                    "{ENV}={name} is its own authority and forwards nothing upstream, but {key} is \
                     set: use {ENV}=mirror to forward local deploys"
                );
            }
        }
    }
    if posture != Posture::Mirror {
        refuse_followers(posture, env)?;
        refuse_remote_hosts(posture, config, env)?;
        refuse_third_party_secrets(posture, env)?;
    }
    if posture.writes_need_restore_stamp() {
        let open: Vec<&str> = [
            (
                "CONTENT_WRITES_ENABLED",
                config.writes.content_writes_enabled,
            ),
            ("CREATE_WRITES_ENABLED", config.create.create_writes_enabled),
            ("DATA_WRITES_ENABLED", config.writes.data_writes_enabled),
            ("SOCIAL_WRITES_ENABLED", config.writes.social_writes_enabled),
            ("WORLDS_WRITES_ENABLED", config.worlds.worlds_writes_enabled),
            ("OPS_WRITES_ENABLED", config.ops.ops_writes_enabled),
        ]
        .into_iter()
        .filter_map(|(key, on)| on.then_some(key))
        .collect();
        if !open.is_empty() && env(RESTORE_STAMP_DIR).is_none_or(|d| d.trim().is_empty()) {
            anyhow::bail!(
                "{ENV}={} holds real data, so local writes sit behind the restore stamp, but {} \
                 {} on with no {RESTORE_STAMP_DIR}: set it (the backup and drill stamps) or turn \
                 the writes off",
                posture.name(),
                open.join(", "),
                if open.len() == 1 { "is" } else { "are" },
            );
        }
    }
    Ok(())
}

/// The follower switches, in table order: the keys island and journey
/// refuse switched on.
pub fn followers() -> impl Iterator<Item = &'static str> {
    ROWS.iter().filter(|row| follows(row)).map(|row| row.key)
}

/// A follower switch: on under mirror, off under island and journey.
fn follows(row: &Row) -> bool {
    matches!(
        (&row.mirror, &row.island, &row.journey),
        (Is("true"), Is("false"), Is("false"))
    )
}

/// Whether a switch's value turns it on, as serve reads it.
pub fn switched_on(value: &str) -> bool {
    matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "yes" | "on"
    )
}

/// Island and journey read from no Decentraland or third-party host: every
/// follower switch of the table stays off (`deploy/check-topology.sh` refuses
/// the same on an island), and the MANA transfer walk and the worlds
/// federation's peer polling with them.
fn refuse_followers(posture: Posture, env: &Lookup) -> anyhow::Result<()> {
    let on: Vec<&str> = ROWS
        .iter()
        .filter(|row| follows(row))
        .map(|row| row.key)
        .chain(["MARKETPLACE_MANA_TRANSFERS_INDEX_ENABLED"])
        .filter(|key| env(key).is_some_and(|v| switched_on(&v)))
        .collect();
    if !on.is_empty() {
        anyhow::bail!(
            "{ENV}={} reads from no Decentraland or third-party host, but {} {} on: turn it \
             off or use {ENV}=mirror",
            posture.name(),
            on.join(", "),
            if on.len() == 1 { "is" } else { "are" },
        );
    }
    if set(env, FED_PEERS_FILE).is_some() {
        anyhow::bail!(
            "{ENV}={} reads from no Decentraland or third-party host, but {FED_PEERS_FILE} names \
             worlds federation peers to poll: unset it or use {ENV}=mirror",
            posture.name()
        );
    }
    Ok(())
}

/// The worlds federation's peer file: set, the node polls every peer it names.
const FED_PEERS_FILE: &str = "WORLDS_FED_PEERS_FILE";

/// Keys that may name any host under island and journey, each with its
/// reason; every other variable the configuration reads is held to
/// loopback and this node's own address.
pub const ANY_HOST: &[(&str, &str)] = &[
    ("DATABASE_URL", DATABASE),
    ("TELEMETRY_SQL_DATABASE_URL", DATABASE),
    ("PUBLIC_URL", "this node's own address"),
    ("HTTP_SERVER_HOST", "the address it listens on"),
    ("ACCOUNT_BASE_URL", LINK),
    ("MARKETPLACE_BASE_URL", LINK),
    ("SHOP_BASE_URL", LINK),
    ("SHOP_ITEM_BASE_URL", LINK),
    ("EVENTS_BASE_URL", LINK),
    ("CREDITS_CHECKOUT_CANCEL_URL", LINK),
    ("CREDITS_CHECKOUT_SUCCESS_URL", LINK),
    ("BADGES_PUBLIC_ASSET_BASE_URL", LINK),
    ("PROFILE_CDN_BASE_URL", LINK),
    ("COMMUNITIES_CDN_URL", LINK),
    ("FIRST_WEAR_IMAGE_BASE", LINK),
    ("MAP_EXTERNAL_BASE_URL", LINK),
    ("MAP_IMAGE_BASE_URL", LINK),
    ("CAMERA_REEL_API_URL", LINK),
    ("CAMERA_REEL_BUCKET_URL", LINK),
    ("BFF_URL", LINK),
];
const DATABASE: &str = "this node's own database";
const LINK: &str = "a link clients follow; the node never dials it";

/// A host as compared: lowercased, without a trailing dot.
fn host_of(url: &url::Url) -> Option<String> {
    Some(match url.host()? {
        url::Host::Domain(d) => d.trim_end_matches('.').to_ascii_lowercase(),
        url::Host::Ipv4(ip) => ip.to_string(),
        url::Host::Ipv6(ip) => ip.to_string(),
    })
}

/// The schemes whose URLs always name a host: their parser reads
/// `https:host`, `https:/host` and `https:\\host` as `https://host`.
const SPECIAL_SCHEMES: [&str; 6] = ["http", "https", "ws", "wss", "ftp", "file"];

fn in_scheme(c: u8) -> bool {
    c.is_ascii_alphanumeric() || b"+-.".contains(&c)
}

/// The scheme `s` starts with (`^[A-Za-z][A-Za-z0-9+.-]*:`), lowercased.
fn scheme_of(s: &str) -> Option<String> {
    let b = s.as_bytes();
    if !b.first()?.is_ascii_alphabetic() {
        return None;
    }
    let end = b.iter().position(|c| !in_scheme(*c))?;
    (b[end] == b':').then(|| s[..end].to_ascii_lowercase())
}

/// The hosts a configured value names: each URL in it, wherever a scheme
/// starts (comma lists, `1=` entries and `fixed-adapter:` prefixes
/// included), and the bare `host[:port]` a `*_HOST` key or `NATS_URL` takes.
/// A URL that does not parse but names a host (a special scheme, or `//`
/// after the scheme) is returned whole, so it is refused rather than passed.
fn named_hosts(key: &str, value: &str) -> Vec<String> {
    let bare = key.ends_with("_HOST") || key.ends_with("NATS_URL");
    let mut hosts = Vec::new();
    for token in value
        .split(|c: char| c == ',' || c.is_whitespace())
        .filter(|t| !t.is_empty())
    {
        let special = scheme_of(token).is_some_and(|s| SPECIAL_SCHEMES.contains(&s.as_str()));
        if bare && !special && !token.contains("://") {
            if !token.starts_with('/') {
                match url::Url::parse(&format!("x://{token}")) {
                    Ok(url) => hosts.extend(host_of(&url)),
                    Err(_) => hosts.push(token.to_string()),
                }
            }
            continue;
        }
        let bytes = token.as_bytes();
        for at in 0..bytes.len() {
            if !token.is_char_boundary(at) || at > 0 && in_scheme(bytes[at - 1]) {
                continue;
            }
            let candidate = &token[at..];
            let Some(scheme) = scheme_of(candidate) else {
                continue;
            };
            match url::Url::parse(candidate) {
                Ok(url) => hosts.extend(host_of(&url)),
                Err(_)
                    if SPECIAL_SCHEMES.contains(&scheme.as_str())
                        || candidate[scheme.len() + 1..].starts_with("//") =>
                {
                    hosts.push(token.to_string())
                }
                Err(_) => {}
            }
        }
    }
    hosts
}

/// Loopback, a reserved `.invalid` name (RFC 6761: never resolves), or one
/// of this node's own hosts.
fn is_local(host: &str, own: &[String]) -> bool {
    if let Ok(ip) = host.parse::<std::net::IpAddr>() {
        return ip.is_loopback() || own.iter().any(|o| o == host);
    }
    host == "localhost"
        || host.ends_with(".localhost")
        || host.ends_with(".invalid")
        || own.iter().any(|o| o == host)
}

/// This node's own hosts: `PUBLIC_URL`'s and the bind address's.
fn own_hosts(config: &Config) -> Vec<String> {
    [
        config.node.public_url.clone(),
        Some(format!("http://{}", config.node.bind_addr())),
    ]
    .into_iter()
    .flatten()
    .filter_map(|u| url::Url::parse(u.trim()).ok())
    .filter_map(|u| host_of(&u))
    .collect()
}

/// The first `(key, host)` among `values` naming a host other than loopback
/// or this node's own, keys of [`ANY_HOST`] aside.
fn remote_host<'a>(
    own: &[String],
    values: impl Iterator<Item = (&'a str, String)>,
) -> Option<(&'a str, String)> {
    values
        .filter(|(key, _)| !ANY_HOST.iter().any(|(k, _)| k == key))
        .find_map(|(key, value)| {
            named_hosts(key, &value)
                .into_iter()
                .find(|host| !is_local(host, own))
                .map(|host| (key, host))
        })
}

fn refuse_remote_hosts(posture: Posture, config: &Config, env: &Lookup) -> anyhow::Result<()> {
    let own = own_hosts(config);
    let values = Config::env_names()
        .iter()
        .filter_map(|key| env(key).map(|value| (key.as_str(), value)));
    match remote_host(&own, values) {
        Some((key, host)) => Err(outside_host(posture, key, &host)),
        None => Ok(()),
    }
}

/// `NODE_POSTURE` as `env` holds it; `None` when unset or empty.
pub fn from_env(env: &Lookup) -> anyhow::Result<Option<Posture>> {
    set(env, ENV).map(|raw| Posture::parse(&raw)).transpose()
}

/// The outside-host refusal [`check`] applies to serve's configuration, for
/// a process that reads its own variables outside [`Config`] (Pulse):
/// under island and journey every value names loopback, a `.localhost` or
/// `.invalid` name, or the host of a `PUBLIC_URL` among `values`.
pub fn check_values<'a>(
    posture: Posture,
    values: impl IntoIterator<Item = (&'a str, String)>,
) -> anyhow::Result<()> {
    if posture == Posture::Mirror {
        return Ok(());
    }
    let values: Vec<(&str, String)> = values.into_iter().collect();
    let own: Vec<String> = values
        .iter()
        .filter(|(key, _)| *key == "PUBLIC_URL")
        .filter_map(|(_, value)| url::Url::parse(value.trim()).ok())
        .filter_map(|url| host_of(&url))
        .collect();
    match remote_host(&own, values.into_iter()) {
        Some((key, host)) => Err(outside_host(posture, key, &host)),
        None => Ok(()),
    }
}

fn outside_host(posture: Posture, key: &str, host: &str) -> anyhow::Error {
    anyhow::anyhow!(
        "{ENV}={} reaches no host but loopback and its own PUBLIC_URL, but {key} names {host}: \
         unset it, point it at a local service, or use {ENV}=mirror",
        posture.name()
    )
}

/// The same refusal over the secrets, once their `<NAME>_FILE`s are read: an
/// RPC endpoint or token URL held in a credential file.
pub fn refuse_remote_secrets(config: &mut Config) -> anyhow::Result<()> {
    let Some(posture) = config
        .node
        .node_posture
        .as_deref()
        .and_then(|p| Posture::parse(p).ok())
        .filter(|p| *p != Posture::Mirror)
    else {
        return Ok(());
    };
    let own = own_hosts(config);
    let values: Vec<(&'static str, String)> = config
        .secret_slots()
        .into_iter()
        .filter_map(|(key, slot)| slot.clone().map(|value| (key, value)))
        .collect();
    if let Some((key, host)) = remote_host(&own, values.into_iter()) {
        anyhow::bail!(
            "{ENV}={} reaches no host but loopback and its own PUBLIC_URL, but the secret {key} \
             names {host}: unset it, point it at a local service, or use {ENV}=mirror",
            posture.name()
        );
    }
    Ok(())
}

/// The secrets that switch on a client of a fixed third-party API, with the
/// key that can point it elsewhere (then held to loopback above) and the API.
const THIRD_PARTY_SECRETS: &[(&str, Option<&str>, &str)] = &[
    ("SENDGRID_API_KEY", None, "SendGrid's mail API"),
    (
        "TURNSTILE_SECRET_KEY",
        None,
        "Cloudflare Turnstile's siteverify",
    ),
    (
        "STRIPE_SECRET_KEY",
        Some("STRIPE_API_BASE"),
        "Stripe's API (STRIPE_API_BASE)",
    ),
    (
        "CREDITS_CAPTCHA_SECRET",
        Some("CREDITS_CAPTCHA_VERIFY_URL"),
        "hCaptcha's siteverify (CREDITS_CAPTCHA_VERIFY_URL)",
    ),
];

fn refuse_third_party_secrets(posture: Posture, env: &Lookup) -> anyhow::Result<()> {
    for (secret, base, api) in THIRD_PARTY_SECRETS {
        let given = set(env, secret).is_some() || set(env, &format!("{secret}_FILE")).is_some();
        let redirected = base.is_some_and(|b| set(env, b).is_some());
        if given && !redirected {
            anyhow::bail!(
                "{ENV}={} reaches no third party, but {secret} is set, which calls {api}: unset \
                 it (and its _FILE) or use {ENV}=mirror",
                posture.name()
            );
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;
