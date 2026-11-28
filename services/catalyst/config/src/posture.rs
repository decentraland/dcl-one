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
//!   write domain open and no chain access checks.

use std::fmt::Write as _;

use catalyst_shared::network::ChainPair;

use crate::config::Config;

pub const ENV: &str = "NODE_POSTURE";

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
                 the CDN fallbacks; local writes open when switched on"
            }
            Posture::Island => {
                "no request to Decentraland or any third party; this node is its own authority and \
                 its map URLs are its own; local writes open when switched on"
            }
            Posture::Journey => {
                "the scratch-database test stack: no upstream traffic, every write domain open"
            }
        }
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
    /// Left unset: the binary's default for every posture that dials outside
    /// hosts (the text says which).
    Default(&'static str),
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

/// Shut under mirror and island until the operator opens it; open on the
/// journey stack's scratch database.
const fn write(key: &'static str) -> Row {
    row("writes", key, Is("false"), Is("false"), Is("true"))
}

const fn same(family: &'static str, key: &'static str, value: &'static str) -> Row {
    row(family, key, Is(value), Is(value), Is(value))
}

use Value::{Default, From, Is, Own};

const DCL_PEERS: &str = "https://peer.decentraland.org/content,https://peer-eu1.decentraland.org/content,https://peer.dclnodes.io/content";
/// The Polygon CreditsManager the credits usage index reads (mainnet).
const CREDITS_MANAGER_MAINNET: &str = "0x8b3a40ca1b6f5cafc99d112a4d02e897d1fd8cc5";
const PROFILE_IMAGES_CACHE_BYTES: &str = "5368709120";

fn set(lookup: &Lookup, key: &str) -> Option<String> {
    lookup(key).filter(|v| !v.trim().is_empty())
}

/// Some mirror preset jobs and endpoints cover only mainnet.
fn mainnet(lookup: &Lookup) -> bool {
    set(lookup, "ETH_NETWORK")
        .is_none_or(|n| ChainPair::from_eth_network(&n) == Some(ChainPair::Mainnet))
}

fn sepolia(lookup: &Lookup) -> bool {
    set(lookup, "ETH_NETWORK").and_then(|n| ChainPair::from_eth_network(&n))
        == Some(ChainPair::Testnet)
}

#[derive(Clone, Copy)]
struct CollectionsNetwork {
    names: [&'static str; 2],
    replay_start: &'static str,
    replay_stop: &'static str,
}

const ETH_COLLECTIONS: CollectionsNetwork = CollectionsNetwork {
    names: ["ethereum", "mainnet"],
    replay_start: "MARKETPLACE_INDEX_START_BLOCK_ETHEREUM",
    replay_stop: "MARKETPLACE_INDEX_STOP_BLOCK_ETHEREUM",
};
const MATIC_COLLECTIONS: CollectionsNetwork = CollectionsNetwork {
    names: ["matic", "polygon"],
    replay_start: "MARKETPLACE_INDEX_START_BLOCK_MATIC",
    replay_stop: "MARKETPLACE_INDEX_STOP_BLOCK_MATIC",
};

fn owns_collections(raw: &str, names: [&str; 2]) -> bool {
    raw.split(',')
        .map(str::trim)
        .any(|name| names.iter().any(|alias| name.eq_ignore_ascii_case(alias)))
}

fn walks_mainnet_collections(lookup: &Lookup) -> bool {
    mainnet(lookup)
        && lookup("MARKETPLACE_COLLECTIONS_INDEX_ENABLED")
            .is_none_or(|value| value.trim() == "true")
        && set(lookup, "MARKETPLACE_INDEX_CHAIN_ID_ETHEREUM")
            .is_none_or(|value| value.trim() == "1")
        && set(lookup, "MARKETPLACE_INDEX_CHAIN_ID_MATIC").is_none_or(|value| value.trim() == "137")
}

fn default_collections_ownership(lookup: &Lookup) -> Option<String> {
    Some(
        [MATIC_COLLECTIONS, ETH_COLLECTIONS]
            .into_iter()
            .filter(|network| can_default_to_projection(lookup, *network))
            .map(|network| network.names[0])
            .collect::<Vec<_>>()
            .join(","),
    )
}

fn can_default_to_projection(lookup: &Lookup, network: CollectionsNetwork) -> bool {
    walks_mainnet_collections(lookup)
        && set(lookup, network.replay_start).is_none()
        && set(lookup, network.replay_stop).is_none()
}

/// Whether the collections index owns `network` here: it walks mainnet
/// collections and `MARKETPLACE_COLLECTIONS_INDEX_OWNS` (expanded before the
/// rows that ask) names the network.
fn collections_owned(lookup: &Lookup, network: CollectionsNetwork) -> bool {
    walks_mainnet_collections(lookup)
        && lookup("MARKETPLACE_COLLECTIONS_INDEX_OWNS")
            .is_some_and(|owns| owns_collections(&owns, network.names))
}

/// Why this node keeps the collections subgraphs although the collections
/// index owns both networks: the Builder's on-chain routes
/// (`builder/src/onchain.rs`) read them, and the creator plane
/// (`CREATE_WRITES_ENABLED=true`) is the deployment the Builder serves from.
pub const BUILDER_READS_COLLECTIONS: &str =
    "the Builder on-chain routes read the collections subgraphs (CREATE_WRITES_ENABLED=true)";

fn builder_reads_collections(lookup: &Lookup) -> bool {
    lookup("CREATE_WRITES_ENABLED").is_some_and(|value| switched_on(&value))
}

/// A mainnet collections subgraph is read while the mirror fills that
/// network's catalogue (the collections index does not own it) or the
/// Builder reads it; a testnet chain always reads its own.
fn collections_source(
    lookup: &Lookup,
    network: CollectionsNetwork,
    testnet: Option<&'static str>,
    mainnet_url: &'static str,
) -> Option<String> {
    if let Some(testnet) = testnet {
        return Some(testnet.into());
    }
    (!collections_owned(lookup, network) || builder_reads_collections(lookup))
        .then(|| mainnet_url.into())
}

fn eth_collections_source(lookup: &Lookup) -> Option<String> {
    let testnet = (set(lookup, "MARKETPLACE_INDEX_CHAIN_ID_ETHEREUM").as_deref()
        == Some("11155111"))
    .then_some("https://subgraph.decentraland.org/collections-ethereum-sepolia");
    collections_source(
        lookup,
        ETH_COLLECTIONS,
        testnet,
        "https://subgraph.decentraland.org/collections-ethereum-mainnet",
    )
}

fn matic_collections_source(lookup: &Lookup) -> Option<String> {
    let testnet = (set(lookup, "MARKETPLACE_INDEX_CHAIN_ID_MATIC").as_deref() == Some("80002"))
        .then_some("https://subgraph.decentraland.org/collections-matic-amoy");
    collections_source(
        lookup,
        MATIC_COLLECTIONS,
        testnet,
        "https://subgraph.decentraland.org/collections-matic-mainnet",
    )
}

/// The collections and items mirrors run unless the collections index owns
/// both networks, which then fills the whole catalogue itself.
fn mirrors_default(lookup: &Lookup) -> Option<String> {
    let owned = [ETH_COLLECTIONS, MATIC_COLLECTIONS]
        .into_iter()
        .all(|network| collections_owned(lookup, network));
    Some((!owned).to_string())
}

/// Decentraland's marketplace subgraph, off mainnet: a mainnet node reads
/// LAND and DCL NAMEs from its own indexes.
fn marketplace_subgraph(lookup: &Lookup) -> Option<String> {
    (!mainnet(lookup)).then(|| "https://subgraph.decentraland.org/marketplace".into())
}

fn third_party_registry_subgraph(lookup: &Lookup) -> Option<String> {
    (!mainnet(lookup)).then(|| "https://subgraph.decentraland.org/tpr-matic-mainnet".into())
}

fn credits_manager(lookup: &Lookup) -> Option<String> {
    mainnet(lookup).then(|| CREDITS_MANAGER_MAINNET.to_string())
}

fn denylist_or_decentraland(lookup: &Lookup) -> Option<String> {
    Some(
        set(lookup, "DENYLIST_JSON_URL")
            .unwrap_or_else(|| "https://config.decentraland.org/denylist.json".to_string()),
    )
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
    // The DAO catalyst list over ETH_RPC_URL; SYNC_SOURCE is its fallback.
    mirrored("sync", "SYNC_FROM_DAO"),
    mirrored("sync", "READ_THROUGH_ENABLED"),
    // Decentraland's moderation list, hidden from the content reads as the
    // upstream catalyst hides it (its DENYLIST_URLS default).
    upstream(
        "sync",
        "DENYLIST_URLS",
        "https://asset-bundle-registry.decentraland.org/denylist",
    ),
    same("sync", "PHASED_SYNC", "true"),
    // Independent of CONTENT_WRITES_ENABLED: local deploys are never pruned.
    mirrored("sync", "CONTENT_HISTORY_PRUNE_ENABLED"),
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
    // The comms mints' wallet denylist, upstream worlds-content-server's
    // DENYLIST_JSON_URL default; that name, when set, wins over the default.
    row(
        "mirrors",
        "DENY_LIST_URL",
        From(
            "DENYLIST_JSON_URL, else https://config.decentraland.org/denylist.json",
            denylist_or_decentraland,
        ),
        Is(""),
        Is(""),
    ),
    upstream(
        "mirrors",
        "EXPLORER_FEATURE_FLAGS_SOURCE_URL",
        "https://feature-flags.decentraland.org/explorer.json",
    ),
    upstream(
        "mirrors",
        "FF_URL",
        "https://feature-flags.decentraland.org",
    ),
    upstream(
        "mirrors",
        "EVENTS_FEATURE_FLAGS_URL",
        "https://feature-flags.decentraland.org",
    ),
    upstream(
        "mirrors",
        "SHOP_FEATURE_FLAGS_URL",
        "https://feature-flags.decentraland.org",
    ),
    upstream(
        "mirrors",
        "SHOP_FEATURE_FLAGS_ORIGIN",
        "https://decentraland.org",
    ),
    upstream("social analytics", "ANALYTICS_API_URL", ""),
    upstream(
        "community compliance",
        "OPENAI_BASE_URL",
        "https://api.openai.com/v1",
    ),
    upstream(
        "notification email",
        "SENDGRID_API_URL",
        "https://api.sendgrid.com",
    ),
    upstream(
        "notification email",
        "DISPOSABLE_EMAIL_DOMAINS_URL",
        "https://raw.githubusercontent.com/disposable-email-domains/disposable-email-domains/main/disposable_email_blocklist.conf",
    ),
    upstream(
        "mirrors",
        "WORLD_MANIFEST_UPSTREAM_URL",
        "https://places-dcf8abb.s3.amazonaws.com/WorldManifest.json",
    ),
    upstream(
        "mirrors",
        "TRADES_SYNC_UPSTREAM_URL",
        "https://marketplace-api.decentraland.org/v1/trades",
    ),
    // Development only: no posture serves an order book before its first complete pass.
    same("mirrors", "TRADES_SERVE_PARTIAL", "false"),
    row(
        "mirrors",
        "MARKETPLACE_INDEX_CHAIN_ID_ETHEREUM",
        From("Sepolia when ETH_NETWORK selects it", |env| {
            sepolia(env).then(|| "11155111".into())
        }),
        Default("Ethereum mainnet"),
        Default("Ethereum mainnet"),
    ),
    row(
        "mirrors",
        "MARKETPLACE_INDEX_CHAIN_ID_MATIC",
        From("Amoy when ETH_NETWORK selects Sepolia", |env| {
            sepolia(env).then(|| "80002".into())
        }),
        Default("Polygon mainnet"),
        Default("Polygon mainnet"),
    ),
    row(
        "mirrors",
        "MARKETPLACE_COLLECTIONS_INDEX_OWNS",
        From(
            "Ethereum and Polygon when walking mainnet collections",
            default_collections_ownership,
        ),
        Is(""),
        Is(""),
    ),
    row(
        "mirrors",
        "ETH_COLLECTIONS_SUBGRAPH_URL",
        From(
            "collections subgraph of the served ethereum chain; on mainnet only while the collections index leaves it to the mirror or CREATE_WRITES_ENABLED=true",
            eth_collections_source,
        ),
        Is(""),
        Is(""),
    ),
    row(
        "mirrors",
        "MATIC_COLLECTIONS_SUBGRAPH_URL",
        From(
            "collections subgraph of the served matic chain; on mainnet only while the collections index leaves it to the mirror or CREATE_WRITES_ENABLED=true",
            matic_collections_source,
        ),
        Is(""),
        Is(""),
    ),
    // After the ownership and the sources it follows.
    row(
        "mirrors",
        "MARKETPLACE_MIRRORS_ENABLED",
        From(
            "on, unless the collections index owns both networks",
            mirrors_default,
        ),
        Is("false"),
        Is("false"),
    ),
    row(
        "mirrors",
        "MARKETPLACE_SUBGRAPH_URL",
        From(
            "Decentraland's marketplace subgraph off mainnet",
            marketplace_subgraph,
        ),
        Is(""),
        Is(""),
    ),
    upstream(
        "mirrors",
        "NFT_WORKER_BASE_URL",
        "https://nfts.decentraland.org",
    ),
    // Mirrors configured for testnet can read the Polygon registry before
    // they have a separate mainnet RPC and a published registry index.
    row(
        "mirrors",
        "THIRD_PARTY_REGISTRY_L2_SUBGRAPH_URL",
        From(
            "Polygon registry subgraph when ETH_NETWORK is a testnet",
            third_party_registry_subgraph,
        ),
        Is(""),
        Is(""),
    ),
    // lamb2's explorer backpack lists chain-owned tokens only; listing this
    // node's escrow grants too stays an operator's opt-in under every posture,
    // island and journey included: set `EXPLORER_ESCROW_GRANTS_ENABLED=true`
    // with `CONTENT_ESCROW_LEASE_OWNERSHIP_ENABLED=true`.
    same("lambdas", "EXPLORER_ESCROW_GRANTS_ENABLED", "false"),
    // lamb2's `/collections/wearables` takes `collectionId`, `wearableId`
    // and `textSearch` by default; operators can opt in to extended filters.
    same("lambdas", "LAMBDAS_CATALOG_EXTENSIONS_ENABLED", "false"),
    mirrored("indexes", "MARKETPLACE_NFT_INDEX_ENABLED"),
    mirrored("indexes", "MARKETPLACE_TRADE_EVENTS_INDEX_ENABLED"),
    mirrored("indexes", "MARKETPLACE_SALES_INDEX_ENABLED"),
    mirrored("indexes", "MARKETPLACE_COMMITTEE_INDEX_ENABLED"),
    mirrored("indexes", "MARKETPLACE_THIRD_PARTY_REGISTRY_INDEX_ENABLED"),
    // Account inventory and lifetime creator rankings require the factory walk.
    mirrored("indexes", "MARKETPLACE_COLLECTIONS_INDEX_ENABLED"),
    // The MANA transfer walk stays an operator's opt-in under every posture.
    same(
        "indexes",
        "MARKETPLACE_MANA_TRANSFERS_INDEX_ENABLED",
        "false",
    ),
    mirrored("indexes", "LAND_AUTHZ_INDEX_ENABLED"),
    mirrored("indexes", "CREDITS_USAGE_INDEX_ENABLED"),
    mirrored("indexes", "SHOP_RENTALS_INDEX_ENABLED"),
    // A pass of these walks makes its eth_getLogs calls (seven for Polygon's
    // collections alone) whether or not a block held an event. A mirror pauses
    // 30 s between the end of one pass and the start of the next, not 5 s.
    row(
        "indexes",
        "MARKETPLACE_INDEX_INTERVAL_SECS",
        Is("30"),
        Default("5"),
        Default("5"),
    ),
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
    // The binary's default for every node outside island and journey
    // (`Config::default_rpc_urls`).
    row(
        "upstreams",
        "ETH_RPC_URL",
        Default("https://rpc.decentraland.org/<mainnet|sepolia>?project=dcl-one by ETH_NETWORK"),
        Is(""),
        Is(""),
    ),
    row(
        "upstreams",
        "RPC_ENDPOINT_POLYGON",
        Default("https://rpc.decentraland.org/<polygon|amoy>?project=dcl-one by ETH_NETWORK"),
        Is(""),
        Is(""),
    ),
    // Derived once the secret files are read (RPC_ENDPOINT_POLYGON_FILE):
    // `Config::derive_trade_rpc_urls`.
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
    // Temporary: imposter tiles the /bvimposters store lacks, read through
    // from the community CDN the explorer's tiles were baked into, until this
    // node bakes its own (decentraland/dcl-one#1711); then this row goes.
    upstream(
        "upstreams",
        "BVIMPOSTERS_UPSTREAM_URL",
        "https://bevy-imposters.dclregenesislabs.xyz",
    ),
    // Avatar faces of profiles whose snapshot files this node does not hold
    // (current clients deploy none) when no renderer runs here.
    upstream(
        "upstreams",
        "PROFILE_IMAGES_ORIGIN_URL",
        "https://profile-images.decentraland.org",
    ),
    // lamb2's default snapshot base: a mirror's profiles name Decentraland's
    // profile images, as the Catalyst it replaces does; island and journey
    // name their own `/profile-images`.
    row(
        "upstreams",
        "PROFILE_CDN_BASE_URL",
        Is("https://profile-images.decentraland.org"),
        Own("<PUBLIC_URL>/profile-images"),
        Own("<PUBLIC_URL>/profile-images"),
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
    // lamb2's /about parcel view; island and journey draw their own minimap.
    row(
        "map",
        "MAP_PARCEL_VIEW_URL",
        Is("https://api.decentraland.org/v1/minimap.png"),
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
    // The image lamb2's `/lands` answers is the marketplace subgraph's
    // `nft.image`, under Decentraland's map API.
    row(
        "map",
        "LAND_IMAGE_BASE_URL",
        Is("https://api.decentraland.org"),
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
    // The journey stack reads no chain, so its local deploys skip the chain checks.
    row(
        "writes",
        "IGNORE_BLOCKCHAIN_ACCESS_CHECKS",
        Is("false"),
        Is("false"),
        Is("true"),
    ),
    // Peers check item ownership on chain only and reject an entity that
    // wears an item held only by an escrow lease, so counting leases stays an
    // operator's opt-in under every posture (set it with
    // `EXPLORER_ESCROW_GRANTS_ENABLED`).
    same("writes", "CONTENT_ESCROW_LEASE_OWNERSHIP_ENABLED", "false"),
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
            Own(_) | Default(_) => None,
            From(_, f) => f(&seen),
        };
        if let Some(value) = value {
            out.push((row.key, value));
        }
    }
    out
}

/// Reads `NODE_POSTURE` and returns it with the variables it adds; `None`
/// when unset (the binary's own defaults: every write and every upstream but
/// the chain RPCs off).
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
///
/// # Safety
///
/// No other thread may exist: it writes the process environment.
#[allow(
    unsafe_code,
    reason = "the posture is applied by writing the process environment"
)]
pub unsafe fn apply_to_process_env() -> anyhow::Result<Option<Posture>> {
    let env = |key: &str| std::env::var_os(key).map(|v| v.to_string_lossy().into_owned());
    let Some((posture, vars)) = resolve(&env)? else {
        return Ok(None);
    };
    for (key, value) in vars {
        // SAFETY: the caller guarantees this is the only thread.
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
            Default(how) => writeln!(out, "# {} unset: the binary's default, {how}", row.key),
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
            "{ENV} is unset: no preset, so every upstream but the chain RPCs is off unless set explicitly \
             (`catalyst posture show mirror` lists them)"
        ),
        Some(Ok(posture)) => format!("{ENV}={}: {}", posture.name(), posture.summary()),
        Some(Err(err)) => err.to_string(),
    }
}

/// The line a mainnet `serve` logs at startup about the subgraph fallbacks
/// it still reads: each one set and why it stayed on, or that none is.
/// The mirror posture sets a mainnet collections subgraph only while the
/// collections index leaves that network to the mirror, or for the Builder
/// on the creator plane; it sets no marketplace or registry subgraph and no
/// mirror over owned networks. Anything else on was set in the environment
/// (or the arguments). `None` off mainnet, where the subgraphs are the
/// sources.
pub fn subgraph_fallbacks_line(config: &Config) -> Option<String> {
    if ChainPair::from_eth_network(&config.node.eth_network) != Some(ChainPair::Mainnet) {
        return None;
    }
    let market = &config.marketplace;
    let projects = market.marketplace_collections_index_enabled
        && market.marketplace_index_chain_id_ethereum == 1
        && market.marketplace_index_chain_id_matic == 137;
    let owned = |network: CollectionsNetwork| {
        projects && owns_collections(&market.marketplace_collections_index_owns, network.names)
    };
    let upstreams = &config.market_upstreams;
    let mut kept: Vec<String> = Vec::new();
    for (key, url, network) in [
        (
            "ETH_COLLECTIONS_SUBGRAPH_URL",
            &upstreams.eth_collections_subgraph_url,
            ETH_COLLECTIONS,
        ),
        (
            "MATIC_COLLECTIONS_SUBGRAPH_URL",
            &upstreams.matic_collections_subgraph_url,
            MATIC_COLLECTIONS,
        ),
    ] {
        if Config::non_empty(url).is_none() {
            continue;
        }
        let why = if !owned(network) {
            format!(
                "the collections mirror fills the {} catalogue",
                network.names[0]
            )
        } else if config.create.create_writes_enabled {
            BUILDER_READS_COLLECTIONS.to_string()
        } else {
            "set in the environment".to_string()
        };
        kept.push(format!("{key} ({why})"));
    }
    for (key, url) in [
        (
            "MARKETPLACE_SUBGRAPH_URL",
            &upstreams.marketplace_subgraph_url,
        ),
        (
            "THIRD_PARTY_REGISTRY_L2_SUBGRAPH_URL",
            &config.validator.third_party_registry_l2_subgraph_url,
        ),
    ] {
        if Config::non_empty(url).is_some() {
            kept.push(format!("{key} (set in the environment)"));
        }
    }
    if market.marketplace_mirrors_enabled
        && [ETH_COLLECTIONS, MATIC_COLLECTIONS].into_iter().all(owned)
    {
        kept.push(
            "MARKETPLACE_MIRRORS_ENABLED (set in the environment: the collections index owns both networks)"
                .to_string(),
        );
    }
    Some(if kept.is_empty() {
        "mainnet subgraph fallbacks: none; the marketplace reads its own indexes".to_string()
    } else {
        format!("mainnet subgraph fallbacks on: {}", kept.join("; "))
    })
}

/// The combinations serve and the indexer refuse to start with. `env` is the
/// process environment.
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
    config.content.denylist_urls()?;
    config.content.check_upload_limits()?;
    let Some(posture) = posture else {
        return Ok(());
    };
    if posture == Posture::Mirror {
        refuse_unfiltered_profiles(config, env)?;
    }
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
    if posture == Posture::Mirror {
        refuse_blank_nft_worker(config)?;
    }
    Ok(())
}

fn walks_catalogue(config: &Config) -> bool {
    let market = &config.marketplace;
    market.marketplace_nft_index_enabled
        && ChainPair::from_eth_network(&config.node.eth_network).map(|pair| pair.chain_ids())
            == Some((
                market.marketplace_index_chain_id_ethereum,
                market.marketplace_index_chain_id_matic,
            ))
}

/// A DCL NAME world's owner is its NAME's holder. A node that walks the nft
/// index (`MARKETPLACE_NFT_INDEX_ENABLED` for its chain pair) reads it there, so
/// world writes need nothing more: while the index is not current, a DCL
/// NAME world's owner writes answer 503. A node that does not reads
/// `MARKETPLACE_SUBGRAPH_URL`; without it no DCL NAME world has an owner, so
/// a `serve` taking world writes is refused, unless it skips the chain checks
/// (`IGNORE_BLOCKCHAIN_ACCESS_CHECKS`), which gets the warning returned here
/// and serves personal worlds only. The indexer takes no world writes, so
/// only `serve` runs this.
pub fn worlds_names_reader(config: &Config) -> anyhow::Result<Option<&'static str>> {
    let walks = walks_catalogue(config);
    if !config.worlds.worlds_writes_enabled
        || walks
        || Config::non_empty(&config.market_upstreams.marketplace_subgraph_url).is_some()
    {
        return Ok(None);
    }
    if !config.validator.ignore_blockchain_access_checks {
        anyhow::bail!(
            "WORLDS_WRITES_ENABLED=true needs a DCL NAME holder reader: the nft index \
             (MARKETPLACE_NFT_INDEX_ENABLED=true for the configured chain pair) or MARKETPLACE_SUBGRAPH_URL. Set one, or \
             IGNORE_BLOCKCHAIN_ACCESS_CHECKS=true on a node without chain access"
        );
    }
    Ok(Some(
        "neither the nft index nor MARKETPLACE_SUBGRAPH_URL reads DCL NAMEs here: DCL NAME worlds have no owner, only personal worlds take owner writes",
    ))
}

/// Wearable and emote ownership is read from the nft index alone, which a
/// node walks with `MARKETPLACE_NFT_INDEX_ENABLED` for its catalogue pair. A node that
/// names a collections subgraph and does not walk the index gets the
/// warning returned here: the subgraph checks no ownership.
pub fn unchecked_profile_items(config: &Config) -> Option<&'static str> {
    let walks = walks_catalogue(config);
    let upstreams = &config.market_upstreams;
    let subgraph = Config::non_empty(&upstreams.eth_collections_subgraph_url).is_some()
        || Config::non_empty(&upstreams.matic_collections_subgraph_url).is_some();
    (subgraph && !walks).then_some(
        "a collections subgraph URL is set, but this node does not walk the nft index \
         (MARKETPLACE_NFT_INDEX_ENABLED off, or ETH_NETWORK and the chain pair disagree): the subgraphs check \
         no ownership, so profiles and outfits are served as deployed and the owned wearable and \
         emote routes answer the nft-index unavailable envelope",
    )
}

/// A walking mirror checks wearables, emotes and DCL NAMEs against the
/// catalogue for its chain pair. A testnet mirror without a walk checks
/// names through the configured graph and serves items as deployed.
fn refuse_unfiltered_profiles(config: &Config, env: &Lookup) -> anyhow::Result<()> {
    if !walks_catalogue(config)
        && ChainPair::from_eth_network(&config.node.eth_network) != Some(ChainPair::Mainnet)
    {
        if Config::non_empty(&config.market_upstreams.marketplace_subgraph_url).is_none() {
            anyhow::bail!(
                "{ENV}=mirror off mainnet checks every profile's names against their owner through \
                 the marketplace subgraph, but MARKETPLACE_SUBGRAPH_URL is empty: unset it to read \
                 Decentraland's subgraph"
            );
        }
        return Ok(());
    }
    if !config.marketplace.marketplace_nft_index_enabled {
        anyhow::bail!(
            "{ENV}=mirror checks every profile's wearables, emotes and names against the nft index, \
             but MARKETPLACE_NFT_INDEX_ENABLED is off: unset it to walk the index"
        );
    }
    let market = &config.marketplace;
    let projects = market.marketplace_collections_index_enabled
        && ChainPair::from_eth_network(&config.node.eth_network) == Some(ChainPair::Mainnet)
        && market.marketplace_index_chain_id_ethereum == 1
        && market.marketplace_index_chain_id_matic == 137;
    let upstreams = &config.market_upstreams;
    for (key, url, network, bounded) in [
        (
            "ETH_COLLECTIONS_SUBGRAPH_URL",
            &upstreams.eth_collections_subgraph_url,
            ETH_COLLECTIONS,
            market.marketplace_index_start_block_ethereum.is_some()
                || market.marketplace_index_stop_block_ethereum.is_some(),
        ),
        (
            "MATIC_COLLECTIONS_SUBGRAPH_URL",
            &upstreams.matic_collections_subgraph_url,
            MATIC_COLLECTIONS,
            market.marketplace_index_start_block_matic.is_some()
                || market.marketplace_index_stop_block_matic.is_some(),
        ),
    ] {
        let owns =
            projects && owns_collections(&market.marketplace_collections_index_owns, network.names);
        let has_url = Config::non_empty(url).is_some();
        if owns && bounded && !(market.marketplace_mirrors_enabled && has_url) {
            let [name, _] = network.names;
            let (start, stop) = (network.replay_start, network.replay_stop);
            anyhow::bail!(
                "{ENV}=mirror cannot rely on a bounded {name} projection without a mirror. \
                 Remove the {start} or {stop} override, or remove {name} from \
                 MARKETPLACE_COLLECTIONS_INDEX_OWNS, enable MARKETPLACE_MIRRORS_ENABLED and set {key}"
            );
        }
        // The posture expanded from the environment's ownership: a network it
        // took as owned got no mirror and no source. A command-line override
        // that hands the network back leaves it with no catalogue writer.
        let assumed_owned = walks_mainnet_collections(env)
            && env("MARKETPLACE_COLLECTIONS_INDEX_OWNS")
                .or_else(|| default_collections_ownership(env))
                .is_some_and(|owns| owns_collections(&owns, network.names));
        if assumed_owned && !owns && !(market.marketplace_mirrors_enabled && has_url) {
            let [name, _] = network.names;
            anyhow::bail!(
                "{ENV}=mirror expanded from an environment in which the collections index owns \
                 {name}, so it left {name}'s mirror off, but the arguments hand {name} back \
                 (MARKETPLACE_COLLECTIONS_INDEX_OWNS or MARKETPLACE_COLLECTIONS_INDEX_ENABLED on \
                 the command line): nothing would fill its catalogue. Set them in the environment \
                 instead, or also pass MARKETPLACE_MIRRORS_ENABLED=true and {key}"
            );
        }
        if market.marketplace_mirrors_enabled && !owns && !has_url {
            anyhow::bail!(
                "{ENV}=mirror walks the nft index over the contracts its collections mirror finds, \
                 but {key} is empty: that network's mirror has nothing to read and its ownership \
                 never publishes. Unset it to read Decentraland's subgraph, or set \
                 MARKETPLACE_MIRRORS_ENABLED=false on a node whose catalogue another node fills"
            );
        }
    }
    // The list a mirror derives from its own endpoints, or the explicit
    // one; a `_FILE` is read after this check, and
    // [`refuse_unwalkable_polygon`] checks what it held.
    let from_file = env("TRADE_RPC_URLS_FILE").is_some_and(|v| !v.trim().is_empty());
    if let Some(raw) = config.marketplace.trade_rpc_urls.as_deref()
        && !from_file
        && config.marketplace.marketplace_index_chain_id_matic == 137
        && !names_chain(raw, 137)
    {
        anyhow::bail!(
            "{ENV}=mirror walks the nft index on chains 1 and 137, but TRADE_RPC_URLS names no \
             137= endpoint: add one, or unset it to use RPC_ENDPOINT_POLYGON"
        );
    }
    Ok(())
}

/// Whether the list binds an endpoint to this chain.
fn names_chain(raw: &str, expected: u64) -> bool {
    raw.split(',').any(|entry| {
        entry.split_once('=').is_some_and(|(chain, url)| {
            chain.trim().parse::<u64>() == Ok(expected) && !url.trim().is_empty()
        })
    })
}

/// Check the resolved endpoints after secret files and network defaults
/// have been read. The Amoy walk also accepts the node's Polygon endpoint.
fn refuse_unwalkable_polygon(config: &Config) -> anyhow::Result<()> {
    if !walks_catalogue(config) {
        return Ok(());
    }
    let raw = config.marketplace.trade_rpc_urls.as_deref().unwrap_or("");
    if config.marketplace.marketplace_index_chain_id_matic == 80002 {
        anyhow::ensure!(
            names_chain(raw, 80002)
                || Config::non_empty(&config.economy.rpc_endpoint_polygon).is_some(),
            "{ENV}=mirror needs an Amoy endpoint in TRADE_RPC_URLS or RPC_ENDPOINT_POLYGON"
        );
    } else if !names_chain(raw, 137) {
        anyhow::bail!(
            "{ENV}=mirror walks the nft index on chains 1 and 137, but the TRADE_RPC_URLS it read \
             (TRADE_RPC_URLS_FILE included) names no 137= endpoint: add one, or unset both to use \
             RPC_ENDPOINT_POLYGON"
        );
    }
    Ok(())
}

/// The follower switches, in table order: the keys island and journey
/// refuse switched on.
pub fn followers() -> impl Iterator<Item = &'static str> {
    ROWS.iter()
        .filter(|row| follows(row))
        .map(|row| row.key)
        .chain(COMPUTED_FOLLOWERS.iter().copied())
}

/// Follower switches whose mirror value is computed (on unless something
/// else fills their data) rather than a plain `true`: island and journey
/// refuse them switched on all the same.
const COMPUTED_FOLLOWERS: &[&str] = &["MARKETPLACE_MIRRORS_ENABLED"];

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
        .chain(COMPUTED_FOLLOWERS.iter().copied())
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

/// A mirror needs the NFT worker to include tokens from open mappings.
fn refuse_blank_nft_worker(config: &Config) -> anyhow::Result<()> {
    if Config::non_empty(&config.market_upstreams.nft_worker_base_url).is_none() {
        anyhow::bail!(
            "{ENV}=mirror answers the third-party wearables from the NFT worker, as lamb2 does, \
             but NFT_WORKER_BASE_URL is empty: unset it to read Decentraland's worker"
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
    ("PUBLIC_URL", "this node's own address"),
    ("HTTP_SERVER_HOST", "the address it listens on"),
    ("ACCOUNT_BASE_URL", LINK),
    ("PROFILE_URL", LINK),
    ("MARKETPLACE_BASE_URL", LINK),
    ("NOTIFICATIONS_SERVICE_BASE_URL", LINK),
    ("SHOP_BASE_URL", LINK),
    ("SHOP_EMAIL_ASSETS_URL", LINK),
    ("SHOP_ITEM_BASE_URL", LINK),
    ("EVENTS_BASE_URL", LINK),
    ("EVENTS_SITE_URL", LINK),
    ("JUMP_IN_SITE_URL", LINK),
    ("PLACES_BASE_URL", LINK),
    ("CREDITS_CHECKOUT_CANCEL_URL", LINK),
    ("CREDITS_CHECKOUT_SUCCESS_URL", LINK),
    ("BADGES_PUBLIC_ASSET_BASE_URL", LINK),
    ("PROFILE_CDN_BASE_URL", LINK),
    ("COMMUNITIES_CDN_URL", LINK),
    ("SERVICE_BASE_URL", LINK),
    ("FIRST_WEAR_IMAGE_BASE", LINK),
    ("NOTIFICATIONS_CDN_URL", LINK),
    ("MAP_EXTERNAL_BASE_URL", LINK),
    ("MAP_IMAGE_BASE_URL", LINK),
    ("MAP_DISSOLVED_ESTATE_IMAGE_URL", LINK),
    ("CAMERA_REEL_API_URL", LINK),
    ("CAMERA_REEL_BUCKET_URL", LINK),
    ("BFF_URL", LINK),
    ("BUILDER_URL", LINK),
    ("CAST2_URL", LINK),
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
    else {
        return Ok(());
    };
    if posture == Posture::Mirror {
        return refuse_unwalkable_polygon(config);
    }
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
    (
        "OPEN_AI_API_KEY",
        Some("OPENAI_BASE_URL"),
        "community compliance Responses API",
    ),
    (
        "SENDGRID_API_KEY",
        Some("SENDGRID_API_URL"),
        "SendGrid's mail API",
    ),
    ("SLACK_BOT_TOKEN", None, "Slack Web API"),
    ("MAGIC_SECRET_KEY", None, "Magic account deletion API"),
    (
        "ANALYTICS_API_TOKEN",
        Some("ANALYTICS_API_URL"),
        "Social Analytics API",
    ),
    (
        "REWARDS_API_KEY_BY_REFERRAL_INVITED_USERS_5",
        Some("REWARD_SERVER_URL"),
        "referral rewards",
    ),
    (
        "REWARDS_API_KEY_BY_REFERRAL_INVITED_USERS_10",
        Some("REWARD_SERVER_URL"),
        "referral rewards",
    ),
    (
        "REWARDS_API_KEY_BY_REFERRAL_INVITED_USERS_20",
        Some("REWARD_SERVER_URL"),
        "referral rewards",
    ),
    (
        "REWARDS_API_KEY_BY_REFERRAL_INVITED_USERS_25",
        Some("REWARD_SERVER_URL"),
        "referral rewards",
    ),
    (
        "REWARDS_API_KEY_BY_REFERRAL_INVITED_USERS_30",
        Some("REWARD_SERVER_URL"),
        "referral rewards",
    ),
    (
        "REWARDS_API_KEY_BY_REFERRAL_INVITED_USERS_50",
        Some("REWARD_SERVER_URL"),
        "referral rewards",
    ),
    (
        "REWARDS_API_KEY_BY_REFERRAL_INVITED_USERS_60",
        Some("REWARD_SERVER_URL"),
        "referral rewards",
    ),
    (
        "REWARDS_API_KEY_BY_REFERRAL_INVITED_USERS_75",
        Some("REWARD_SERVER_URL"),
        "referral rewards",
    ),
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
