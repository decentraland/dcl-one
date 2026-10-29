//! LAND authorization: the one fresh chain index (`schemas/33_land_authz.sql`).
//! Who may deploy to a parcel, folded from the LAND and Estate registries'
//! logs: `authz_event` (the log, `eth_getLogs` over `ETH_RPC_URL` through the
//! ONE client, a `jobs::spawn_periodic` job with `index_cursor` as its
//! durable progress: [`indexer`]), `token_right` / `estate_land` /
//! `account_right` (the fold), read by [`resolve::Store`] and served by
//! [`routes`]. Consumers: the content deploy validator and `worlds::policy`
//! through [`can_deploy`] / [`can_deploy_batch`] / [`owns_land_at`], comms'
//! scene permissions and the `lands-permissions` route through the old
//! lambdas paths.
//!
//! Where each consumer reads:
//! - `GET /lambdas/parcels/{x}/{y}/operators` (comms' `COMMS_LAND_OPERATORS_URL`
//!   shape): **index** once [`indexer::index_ready`]; before that the
//!   unavailable envelope (`Source::LandAuthzIndex`), never owner-only
//!   presented as the operators.
//! - `GET /lambdas/users/{address}/lands-permissions`: **index** (the
//!   `token_right_update_operator_idx` reverse lookup, paged); envelope
//!   before readiness. The old route's second source, the land-manager
//!   subgraph (`LAND_SUBGRAPH_URL`), is not configured in v2.
//! - `GET /lambdas/users/{address}/parcels/{x}/{y}/permissions` and the
//!   `POST .../parcels/permissions` batch: **index** for every leg once
//!   ready; before that the owner leg from the marketplace subgraph reader
//!   (`marketplace::graph::parcels_at`) and the four operator legs
//!   undecided: denied for the write decision (fail-closed, the old
//!   validator's behaviour with no resolver attached) and serialized as
//!   `null` on the permissions routes (the marketplace subgraph's Parcel
//!   and Estate carry no operator, updateOperator, updateManager or
//!   approvedForAll; the old fallback read them from the land-manager
//!   subgraph, `LAND_SUBGRAPH_URL`, which v2 does not configure); the
//!   `x-land-authz-source` header says which. Envelope when neither source
//!   exists.
//! - `GET /lambdas/land-authz/status`: the cursor row, readiness and lag.
//! - [`can_deploy`]: the same decision as the permissions route, for the
//!   write paths; [`owns_land_at`]: the LAND leg of `worlds::policy`.
//!
//! What changed against the old crate: its resolver joined
//! `squid_marketplace.parcel` and `.estate` for coordinates, ownership and
//! estate membership; here coordinates are the token id, ownership is the
//! latest `Transfer` (already indexed: a transfer clears the operators) and
//! estate membership is the Estate registry's `AddLand`/`RemoveLand` logs,
//! two topics the indexer adds ([`KIND_ADD_LAND`], [`KIND_REMOVE_LAND`]).
//! The publish rule is the old one: a reader uses this index only after
//! `index_cursor.folded_block` is set (a completed full replay, folded within
//! 25 blocks of the chain head) and `folded_at` is at most 30 minutes old;
//! until then the marketplace subgraph reader (`marketplace::graph::parcels_at`,
//! `owns_land_at`) answers the owner leg and nothing answers the operator
//! legs. The job runs only with `LAND_AUTHZ_INDEX_ENABLED=true` and
//! `ETH_RPC_URL`; off, the readers keep using the fallback.

// The crates this one builds on, under their module names (AGENTS.md).
pub(crate) use catalyst_chain_index as chain_index;
#[cfg(test)]
pub(crate) use catalyst_db as db;
pub(crate) use catalyst_jobs as jobs;
pub(crate) use catalyst_shared as shared;
pub(crate) use dcl_http as http;

pub mod events;
pub mod indexer;
pub mod resolve;
pub mod routes;

#[cfg(test)]
mod tests;

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize};
use std::time::Duration;

use sqlx::PgPool;

use crate::chain_index::ChainIndex;
use crate::jobs::{Jobs, PeriodicCfg};
pub use indexer::{Cursor, HEAD_LAG_BLOCKS, READY_WINDOW, index_ready};
pub use resolve::{LandRightLeg, ParcelPermissionFlags, ParcelRights, Store};

pub const SCHEMA: &str = "land_authz";

pub use crate::chain_index::registries::{
    ESTATE_REGISTRY_MAINNET, ESTATE_REGISTRY_SEPOLIA, LAND_REGISTRY_MAINNET, LAND_REGISTRY_SEPOLIA,
    LAND_START_BLOCK_MAINNET, LAND_START_BLOCK_SEPOLIA, NETWORK_MAINNET, NETWORK_SEPOLIA,
};

pub const KIND_UPDATE_OPERATOR: &str = "update_operator";
pub const KIND_APPROVAL: &str = "approval";
pub const KIND_TRANSFER: &str = "transfer";
pub const KIND_UPDATE_MANAGER: &str = "update_manager";
pub const KIND_APPROVED_FOR_ALL: &str = "approved_for_all";
/// `AddLand(uint256 indexed _estateId, uint256 indexed _landId)` on the Estate registry.
pub const KIND_ADD_LAND: &str = "add_land";
/// `RemoveLand(uint256 indexed _estateId, uint256 indexed _landId, address indexed _destinatary)`.
pub const KIND_REMOVE_LAND: &str = "remove_land";

pub const ALL_KINDS: [&str; 7] = [
    KIND_UPDATE_OPERATOR,
    KIND_APPROVAL,
    KIND_TRANSFER,
    KIND_UPDATE_MANAGER,
    KIND_APPROVED_FOR_ALL,
    KIND_ADD_LAND,
    KIND_REMOVE_LAND,
];

/// A parcel's resolved subject: whose parcel it is (the estate's owner when
/// it sits in one), which registry decides, and the two per-token legs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParcelSubject {
    pub owner: String,
    pub registry: String,
    pub operator: Option<String>,
    pub update_operator: Option<String>,
    pub belongs_to_estate: bool,
}

/// A parcel an address may update, directly or through its estate.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UpdatableParcel {
    pub token_id: String,
    pub x: i32,
    pub y: i32,
    pub owner: String,
    pub via_estate: bool,
}

/// The four operator legs the deploy validator checks for one parcel.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LandOperators {
    pub operator: Option<String>,
    pub update_operator: Option<String>,
    pub update_managers: Vec<String>,
    pub approved_for_all: Vec<String>,
}

/// LAND packs its signed coordinates into the two 128-bit halves of the
/// token id (the SQL twin is `land_authz.token_x/token_y`).
pub fn decode_land_token_id(token_id_hex: &str) -> Option<(i32, i32)> {
    let hex = token_id_hex.strip_prefix("0x")?;
    if hex.len() != 64 {
        return None;
    }
    let hi = u128::from_str_radix(&hex[..32], 16).ok()?;
    let lo = u128::from_str_radix(&hex[32..], 16).ok()?;
    let sign_extend = |v: u128| -> i32 { (v & 0xffff_ffff) as u32 as i32 };
    Some((sign_extend(hi), sign_extend(lo)))
}

pub(crate) fn is_address(addr: &str) -> bool {
    crate::shared::identity::is_valid_eth_address(addr)
}

/// The indexer's configuration. `main.rs` fills it from `Config`:
/// `ETH_NETWORK` picks the registries and the cursor row, `ETH_RPC_URL`
/// (`Config::eth_rpc_url()`, https only) is the log source,
/// `LAND_AUTHZ_INDEX_ENABLED` attaches the job, and the four knobs below
/// bound it.
#[derive(Clone, Debug)]
pub struct Settings {
    /// `ethereum-mainnet` | `ethereum-sepolia` (`index_cursor.network`).
    pub network: String,
    pub land_registry: String,
    pub estate_registry: String,
    /// The first block a full replay reads.
    pub start_block: u64,
    /// `ETH_RPC_URL`; `None` leaves the job off.
    pub rpc_url: Option<String>,
    /// `LAND_AUTHZ_INDEX_ENABLED` (default false).
    pub index_enabled: bool,
    /// `LAND_AUTHZ_INDEX_INTERVAL_SECS` (120): pause between passes.
    pub interval: Duration,
    /// `LAND_AUTHZ_INDEX_REQUESTS_PER_PASS` (50, effective minimum 2): log,
    /// timestamp-header and repair calls per pass, excluding head probes.
    pub requests_per_pass: usize,
    /// `LAND_AUTHZ_REPLAY_FROM_BLOCK`: rewind durably once at the first pass
    /// (the start block for a full repair).
    pub replay_from_block: Option<u64>,
    /// `LAND_AUTHZ_STOP_BLOCK`: index no further (a development bound); an
    /// index held short of the head is never published.
    pub stop_block: Option<u64>,
}

impl Settings {
    /// The registries and start block of `eth_network` (`mainnet` |
    /// `sepolia`, the `ETH_NETWORK` spellings); `None` for another network.
    pub fn for_network(eth_network: &str, rpc_url: Option<String>) -> Option<Self> {
        let net = crate::chain_index::registries::land_network(eth_network)?;
        Some(Self {
            network: net.network.to_string(),
            land_registry: net.land_registry.to_string(),
            estate_registry: net.estate_registry.to_string(),
            start_block: net.start_block,
            rpc_url,
            index_enabled: false,
            interval: Duration::from_secs(120),
            requests_per_pass: 50,
            replay_from_block: None,
            stop_block: None,
        })
    }

    /// Mainnet, no RPC, job off: what the tests start from.
    pub fn for_tests() -> Self {
        Self::for_network("mainnet", None).expect("mainnet is known")
    }

    pub fn is_configured(&self) -> bool {
        self.index_enabled && self.rpc_url.is_some()
    }
}

/// The module's state: the ONE pool and client, the settings, and the
/// marketplace reader that answers the owner leg while the index is not
/// ready (`None`: no fallback, the readers answer unavailable).
pub struct LandAuthzState {
    pub pool: PgPool,
    pub http: crate::http::Outbound,
    pub settings: Settings,
    pub market: Option<Arc<ChainIndex>>,
    pub(crate) replay_requested: AtomicBool,
    // Retry range hint only; losing it on restart cannot advance durable progress.
    pub(crate) retry_upper: AtomicU64,
    // Fetches remaining until the next batch probe; durable progress stays in PostgreSQL.
    pub(crate) single_timestamp_headers: AtomicUsize,
}

pub type SharedState = Arc<LandAuthzState>;

/// Which source decided.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DecisionSource {
    /// The local index, folded at this block.
    Index { folded_block: i64 },
    /// The marketplace subgraph reader: the owner leg only; the four
    /// operator legs are undecided (denied for a write, `null` on the wire)
    /// because nothing can answer them yet.
    SubgraphOwnerOnly,
}

impl DecisionSource {
    pub fn header_value(&self) -> &'static str {
        match self {
            Self::Index { .. } => "land-authz-index",
            Self::SubgraphOwnerOnly => "marketplace-subgraph:owner-only",
        }
    }
}

/// One parcel's answer for one actor.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Verdict {
    /// The parcel is indexed (or found by the reader) and the actor holds this leg.
    Granted(LandRightLeg),
    /// The parcel is known and the actor holds no granting leg.
    Denied,
    /// No parcel at these coordinates in the source (unminted, or not indexed).
    Unknown,
}

impl Verdict {
    pub fn allows(&self) -> bool {
        matches!(self, Self::Granted(_))
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Decision {
    pub verdict: Verdict,
    pub source: DecisionSource,
    /// Every leg, for the permissions routes (`Unknown` carries the default).
    pub flags: ParcelPermissionFlags,
}

/// Why no decision could be made: the caller refuses the write (never
/// grants), and a route answers the unavailable envelope.
#[derive(Debug, thiserror::Error)]
pub enum Unavailable {
    #[error(
        "no LAND rights source: the land-authz index is not ready and no marketplace subgraph is configured"
    )]
    NoSource,
    #[error("land-authz index: {0}")]
    Index(#[from] sqlx::Error),
    #[error("marketplace LAND reader: {0}")]
    Reader(String),
}

impl LandAuthzState {
    pub fn new(
        pool: PgPool,
        http: crate::http::Outbound,
        settings: Settings,
        market: Option<Arc<ChainIndex>>,
    ) -> Self {
        Self {
            pool,
            http,
            settings,
            market,
            replay_requested: AtomicBool::new(false),
            retry_upper: AtomicU64::new(u64::MAX),
            single_timestamp_headers: AtomicUsize::new(0),
        }
    }

    pub fn store(&self) -> Store {
        Store::new(
            self.pool.clone(),
            &self.settings.land_registry,
            &self.settings.estate_registry,
        )
    }

    /// The cursor row when the local index may answer on the database clock.
    pub async fn ready(&self) -> Option<Cursor> {
        indexer::ready_cursor(&self.pool, &self.settings.network).await
    }

    /// The marketplace reader, when one with a subgraph URL is attached.
    fn fallback_reader(&self) -> Option<&Arc<ChainIndex>> {
        self.market
            .as_ref()
            .filter(|m| m.settings.marketplace_subgraph_url.is_some())
    }

    /// Attaches the indexer when `LAND_AUTHZ_INDEX_ENABLED` and `ETH_RPC_URL`
    /// are set: one periodic job with a 15 s first-fire jitter and a 1 h
    /// error cap (an RPC outage doubles the pause, never hammers).
    pub fn spawn_jobs(self: &Arc<Self>, jobs: &Jobs) {
        if !self.settings.index_enabled {
            tracing::info!(
                "LAND_AUTHZ_INDEX_ENABLED is false: the LAND authorization indexer is off; the readers use the \
                 marketplace subgraph for the owner leg and no operator leg is granted"
            );
            return;
        }
        if self.settings.rpc_url.is_none() {
            tracing::warn!(
                "LAND_AUTHZ_INDEX_ENABLED is set but ETH_RPC_URL is not: the LAND authorization indexer has no log source and stays off"
            );
            return;
        }
        let state = self.clone();
        jobs.spawn_periodic_leased(
            "land-authz-index",
            PeriodicCfg::every(self.settings.interval)
                .with_jitter(Duration::from_secs(15))
                .with_error_backoff(Duration::from_secs(3600)),
            move |ctx| {
                let state = state.clone();
                async move { indexer::run_pass(&state, &ctx).await.map(|_| ()) }
            },
        );
    }
}

/// Positional by `parcels`: for every parcel, whether `actor` holds a
/// deploy-granting leg. The index answers every leg once ready;
/// otherwise the marketplace reader answers the owner leg and the operator
/// legs are denied (fail-closed, the old validator with no resolver).
pub async fn can_deploy_batch(
    state: &LandAuthzState,
    parcels: &[(i32, i32)],
    actor: &str,
) -> Result<Vec<Decision>, Unavailable> {
    if parcels.is_empty() {
        return Ok(Vec::new());
    }
    if let Some(cursor) = state.ready().await {
        let source = DecisionSource::Index {
            folded_block: cursor.folded_block.unwrap_or(cursor.last_block),
        };
        let rights = state.store().rights_batch(parcels).await?;
        return Ok(rights
            .into_iter()
            .map(|r| match r {
                Some(rights) => {
                    let flags = rights.flags_for(actor);
                    let verdict = flags
                        .granting_leg()
                        .map_or(Verdict::Denied, Verdict::Granted);
                    Decision {
                        verdict,
                        source: source.clone(),
                        flags,
                    }
                }
                None => Decision {
                    verdict: Verdict::Unknown,
                    source: source.clone(),
                    flags: ParcelPermissionFlags::default(),
                },
            })
            .collect());
    }
    let Some(market) = state.fallback_reader() else {
        return Err(Unavailable::NoSource);
    };
    let found = crate::chain_index::graph::parcels_at(market, parcels)
        .await
        .map_err(Unavailable::Reader)?;
    let actor_lower = actor.to_ascii_lowercase();
    Ok(parcels
        .iter()
        .map(|&(x, y)| {
            let parcel = found.iter().find(|p| p.x == x && p.y == y);
            let (verdict, flags) = match parcel {
                None => (Verdict::Unknown, ParcelPermissionFlags::default()),
                Some(p) => {
                    let owner = is_address(actor)
                        && (p.owner == actor_lower
                            || p.estate_owner.as_deref() == Some(actor_lower.as_str()));
                    let flags = ParcelPermissionFlags {
                        owner,
                        ..Default::default()
                    };
                    (
                        if owner {
                            Verdict::Granted(LandRightLeg::Owner)
                        } else {
                            Verdict::Denied
                        },
                        flags,
                    )
                }
            };
            Decision {
                verdict,
                source: DecisionSource::SubgraphOwnerOnly,
                flags,
            }
        })
        .collect())
}

/// One parcel: may `actor` deploy to it?
pub async fn can_deploy(
    state: &LandAuthzState,
    parcel: (i32, i32),
    actor: &str,
) -> Result<Decision, Unavailable> {
    Ok(can_deploy_batch(state, &[parcel], actor)
        .await?
        .pop()
        .expect("one parcel in, one decision out"))
}

/// The LAND leg of `worlds::policy`: whether `owner` holds any of `coords`,
/// directly or through an estate, from the index when ready, else the
/// marketplace reader (the same rule `can_deploy` applies to the owner leg).
pub async fn owns_land_at(
    state: &LandAuthzState,
    owner: &str,
    coords: &[(i32, i32)],
) -> Result<bool, Unavailable> {
    if state.ready().await.is_some() {
        return Ok(state.store().owns_land_at(owner, coords).await?);
    }
    let Some(market) = state.fallback_reader() else {
        return Err(Unavailable::NoSource);
    };
    crate::chain_index::graph::owns_land_at(market, owner, coords)
        .await
        .map_err(Unavailable::Reader)
}

/// The routes this module serves under the old lambdas paths; `main.rs`
/// merges it beside the marketplace router.
pub fn router(state: SharedState) -> axum::Router {
    routes::router(state)
}

#[cfg(test)]
mod unit_tests {
    use super::*;
    use crate::db::SCHEMA_SQL;

    #[test]
    fn land_token_id_decodes_to_signed_coordinates() {
        let id = "0x0000000000000000000000000000002500000000000000000000000000000037";
        assert_eq!(decode_land_token_id(id).unwrap(), (37, 55));
        let negative = "0xfffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffe";
        assert_eq!(decode_land_token_id(negative).unwrap(), (-1, -2));
        assert_eq!(decode_land_token_id("0x12"), None);
    }

    #[test]
    fn the_schema_names_every_event_kind_and_the_fold_tables() {
        let check = format!(
            "CHECK (kind IN (\n        {}))",
            ALL_KINDS
                .iter()
                .map(|k| format!("'{k}'"))
                .collect::<Vec<_>>()
                .join(", ")
        );
        assert!(SCHEMA_SQL.contains(&check), "{check}");
        for table in [
            "authz_event",
            "token_right",
            "estate_land",
            "account_right",
            "index_cursor",
        ] {
            assert!(
                SCHEMA_SQL.contains(&format!("CREATE TABLE {SCHEMA}.{table} (")),
                "{table}"
            );
        }
        assert!(SCHEMA_SQL.contains(&format!("'{NETWORK_MAINNET}', '{NETWORK_SEPOLIA}'")));
        assert!(SCHEMA_SQL.contains("CREATE FUNCTION land_authz.token_x"));
    }

    #[test]
    fn settings_follow_the_eth_network_spelling() {
        let mainnet =
            Settings::for_network("mainnet", Some("https://rpc.example/x".into())).unwrap();
        assert_eq!(
            (mainnet.network.as_str(), mainnet.land_registry.as_str()),
            (NETWORK_MAINNET, LAND_REGISTRY_MAINNET)
        );
        assert_eq!(mainnet.start_block, LAND_START_BLOCK_MAINNET);
        assert!(
            !mainnet.is_configured(),
            "off until LAND_AUTHZ_INDEX_ENABLED"
        );
        let sepolia = Settings::for_network("SEPOLIA", None).unwrap();
        assert_eq!(
            (sepolia.network.as_str(), sepolia.estate_registry.as_str()),
            (NETWORK_SEPOLIA, ESTATE_REGISTRY_SEPOLIA)
        );
        assert!(Settings::for_network("goerli", None).is_none());
    }
}
