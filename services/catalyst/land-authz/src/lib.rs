//! LAND authorization: the one fresh chain index (`schemas/33_land_authz.sql`).
//! Who may deploy to a parcel, folded from the LAND and Estate registries'
//! logs: `authz_event` (the log, `eth_getLogs` over `ETH_RPC_URL` through the
//! ONE client, a `jobs::spawn_periodic` job with `index_cursor` as its
//! durable progress: [`indexer`]), `token_right` / `estate_land` /
//! `account_right` (the fold), read by [`resolve::Store`] and served by
//! [`routes`]. The same pass appends the Estate registry's version-0 data
//! documents (`CreateEstate`, `Update`) to `estate_data_event`, which
//! [`LandAuthzState::dissolved_estate`] reads for the map metadata of an
//! estate left without LAND. A node whose index predates that table holds
//! only the documents indexed since: `LAND_AUTHZ_REPLAY_FROM_BLOCK` at the
//! Estate registry's deployment block (6,236,547 on mainnet) re-reads the
//! earlier ones. Consumers: the content deploy validator and `worlds::policy`
//! through [`can_deploy`] / [`can_deploy_batch`] / [`owns_land_at`], comms'
//! scene permissions and the `lands-permissions` route through the old
//! lambdas paths.
//!
//! Where each consumer reads: the **index** alone, once it is ready
//! ([`indexer::index_ready`]); before that, or with no published fold, no
//! source answers (there is no subgraph fallback):
//! - `GET /lambdas/parcels/{x}/{y}/operators` (comms' `COMMS_LAND_OPERATORS_URL`
//!   shape): the index; the unavailable envelope (`Source::LandAuthzIndex`)
//!   before it is ready.
//! - `GET /lambdas/users/{address}/lands-permissions`: the index (the
//!   `token_right_update_operator_idx` reverse lookup, paged); the envelope
//!   before it is ready.
//! - `GET /lambdas/users/{address}/parcels/{x}/{y}/permissions`: five
//!   booleans from the index; the envelope before it is ready. The
//!   `x-land-authz-source` header names the source.
//! - `POST .../parcels/permissions` (the batch, this tree's own): every leg
//!   from the index; the envelope before it is ready.
//! - `GET /lambdas/land-authz/status`: the cursor row, readiness and lag.
//! - [`can_deploy`]: the decision the batch route reports, for the write
//!   paths, and [`owns_land_at`]: the LAND leg of `worlds::policy`. Before
//!   the index is ready both answer [`Unavailable::NotReady`] and the write
//!   is refused (fail-closed).
//!
//! What changed against the old crate: its resolver joined
//! `squid_marketplace.parcel` and `.estate` for coordinates, ownership and
//! estate membership; here coordinates are the token id, ownership is the
//! latest `Transfer` (already indexed: a transfer clears the operators) and
//! estate membership is the Estate registry's `AddLand`/`RemoveLand` logs,
//! two topics the indexer adds ([`KIND_ADD_LAND`], [`KIND_REMOVE_LAND`]).
//! The publish rule is the old one: a reader uses this index only after
//! `index_cursor.folded_block` is set (a completed full replay, folded within
//! 25 blocks of the confirmed head, which trails the chain head by
//! `LAND_AUTHZ_CONFIRMATIONS`, 12 by default). The fold job runs every
//! `LAND_AUTHZ_INDEX_INTERVAL_SECS` (120 by default). The public LAND Lambdas
//! routes use folds up to ten minutes old on the database clock;
//! `/lambdas/land-authz/status` reports their window and the fold's age;
//! outside it they answer the unavailable envelope. Deploy authorization and
//! its batch permissions preview use the 30-minute fold window. The job runs
//! only with `LAND_AUTHZ_INDEX_ENABLED=true` and `ETH_RPC_URL`; off, the
//! readers answer from a fold another process publishes into the same
//! database, and are unavailable without one.

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

use crate::jobs::{Jobs, PeriodicCfg};
pub use indexer::{Cursor, HEAD_LAG_BLOCKS, READY_WINDOW, index_ready};

// Public reads allow ten minutes. At the maximum configured interval,
// 240 seconds to the first pass and two retries capped at 150 seconds
// leave one minute for pass execution. Sustained failures age out the fold.
const LAMBDAS_FOLD_WINDOW: chrono::Duration = chrono::Duration::minutes(10);

fn index_schedule(interval: Duration) -> PeriodicCfg {
    PeriodicCfg::every(interval)
        .with_jitter(Duration::from_secs(15))
        .with_error_backoff(Duration::from_secs(150))
}
pub use resolve::{LandRightLeg, ParcelPermissionFlags, ParcelRights, Store};

pub const SCHEMA: &str = "land_authz";

pub use crate::chain_index::registries::{
    ESTATE_REGISTRY_MAINNET, ESTATE_REGISTRY_SEPOLIA, LAND_MAINNET, LAND_REGISTRY_MAINNET,
    LAND_REGISTRY_SEPOLIA, LAND_SEPOLIA, LAND_START_BLOCK_MAINNET, LAND_START_BLOCK_SEPOLIA,
    LandNetwork, NETWORK_MAINNET, NETWORK_SEPOLIA, land_network, land_network_of_registry,
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
/// `operator` and `update_operator` are what the land-manager subgraph
/// answers: `None` for a leg never set, the zero address for one cleared
/// on chain. `deploy_update_operator` is the update operator the deploy
/// check reads, where a cleared leg is no leg (the chain cannot tell the
/// two apart), so a parcel's cleared update operator falls back to its
/// estate's.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParcelSubject {
    pub owner: String,
    pub registry: String,
    pub operator: Option<String>,
    pub update_operator: Option<String>,
    pub deploy_update_operator: Option<String>,
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
/// `LAND_AUTHZ_INDEX_ENABLED` attaches the job, and the knobs below
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
    /// timestamp-header and repair calls per pass, excluding head probes and
    /// the block headers that check a range's fork.
    pub requests_per_pass: usize,
    /// `LAND_AUTHZ_REPLAY_FROM_BLOCK`: rewind durably once at the first pass
    /// (the start block for a full repair, which may also switch registries).
    /// Held per process: while it stays set, every restart replays again, so
    /// unset it once the replay has published.
    pub replay_from_block: Option<u64>,
    /// `LAND_AUTHZ_STOP_BLOCK`: index no further (a development bound); an
    /// index held short of the head is never published.
    pub stop_block: Option<u64>,
    /// `LAND_AUTHZ_CONFIRMATIONS` (12): blocks the index stays behind the
    /// head, so a reorg shallower than this never reaches the log.
    pub confirmations: u64,
    /// [`indexer::REBIND_LOCK_TIMEOUT`]; no environment variable sets it.
    pub rebind_lock_timeout: Duration,
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
            confirmations: 12,
            rebind_lock_timeout: indexer::REBIND_LOCK_TIMEOUT,
        })
    }

    /// Mainnet, no RPC, job off, indexing to the head (no confirmation
    /// depth): what the tests start from.
    pub fn for_tests() -> Self {
        Self {
            confirmations: 0,
            ..Self::for_network("mainnet", None).expect("mainnet is known")
        }
    }

    pub fn is_configured(&self) -> bool {
        self.index_enabled && self.rpc_url.is_some()
    }
}

/// The module's state: the ONE pool and client, and the settings.
pub struct LandAuthzState {
    pub pool: PgPool,
    pub http: crate::http::Outbound,
    pub settings: Settings,
    pub(crate) replay_requested: AtomicBool,
    // Retry range hint only; losing it on restart cannot advance durable progress.
    pub(crate) retry_upper: AtomicU64,
    // Fetches remaining until the next batch probe; durable progress stays in PostgreSQL.
    pub(crate) single_timestamp_headers: AtomicUsize,
}

pub type SharedState = Arc<LandAuthzState>;

/// The source every LAND answer names: the `x-land-authz-source` header and
/// the unavailable envelope's `source`.
pub const INDEX_SOURCE: &str = "land-authz-index";

/// What a LAND answer says while the index has no fresh published fold.
pub const NOT_READY_MSG: &str =
    "LAND rights are unavailable on this node: the land-authz index has no fresh published fold";

/// Which source decided: the local index, the only one.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DecisionSource {
    /// The local index, folded at this block.
    Index { folded_block: i64 },
}

impl DecisionSource {
    pub fn header_value(&self) -> &'static str {
        match self {
            Self::Index { .. } => INDEX_SOURCE,
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

/// A published parcel-membership answer. `None` inside this answer means the
/// parcel belongs to no estate; absence of an answer means the index is unready.
pub struct EstateMembership {
    pub estate_id: Option<String>,
}

/// An estate's latest data document: its name and description, each empty
/// when the document has none.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct EstateData {
    pub name: String,
    pub description: String,
}

/// What the index says of an Estate token
/// ([`LandAuthzState::dissolved_estate`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DissolvedEstate {
    /// Minted and holding no LAND: its latest data document.
    Dissolved(EstateData),
    /// Never minted, or still holding LAND.
    NotDissolved,
    /// The index could not answer: it is not published, or the database
    /// failed. Not an answer about the estate, so never one to cache.
    Unknown,
}

/// Why no decision could be made: the caller refuses the write (never
/// grants), and a route answers the unavailable envelope.
#[derive(Debug, thiserror::Error)]
pub enum Unavailable {
    #[error(
        "no LAND rights source: the land-authz index has no published fold (replaying, rewound, or not yet indexed)"
    )]
    NotReady,
    #[error("land-authz index: {0}")]
    Index(#[from] sqlx::Error),
}

impl LandAuthzState {
    pub fn new(pool: PgPool, http: crate::http::Outbound, settings: Settings) -> Self {
        Self {
            pool,
            http,
            settings,
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

    /// The cursor row when the local index of the configured registries may
    /// answer on the database clock. A yes/no only: a read that depends on
    /// it runs on [`Self::ready_snapshot`] instead.
    pub async fn ready(&self) -> Option<Cursor> {
        indexer::ready_cursor(
            &self.pool,
            &self.settings.network,
            &self.settings.land_registry,
            &self.settings.estate_registry,
        )
        .await
    }

    /// The configured registries' index when it may answer, as a snapshot
    /// the dependent reads run on ([`indexer::ReadySnapshot`]): the check and
    /// the reads see one state, so a rebind between them cannot mix two
    /// registries' rows. A database error reads as not ready (unavailable,
    /// never a grant).
    pub async fn ready_snapshot(&self) -> Option<indexer::ReadySnapshot> {
        self.ready_snapshot_within(READY_WINDOW).await
    }

    async fn ready_snapshot_within(
        &self,
        max_age: chrono::Duration,
    ) -> Option<indexer::ReadySnapshot> {
        match indexer::ready_snapshot_within(
            &self.pool,
            &self.settings.network,
            &self.settings.land_registry,
            &self.settings.estate_registry,
            max_age,
        )
        .await
        {
            Ok(snapshot) => snapshot,
            Err(e) => {
                tracing::warn!(error = %e, "land-authz: cannot open the index snapshot; LAND rights are unavailable");
                None
            }
        }
    }

    /// One indexed coordinate lookup on the caller's connection, including the
    /// publication check in the same SQL snapshot on the database clock (as
    /// `ready`). Event edits already hold a connection and must not acquire
    /// another from a potentially full pool.
    pub async fn estate_at(
        &self,
        conn: &mut sqlx::PgConnection,
        x: i32,
        y: i32,
    ) -> Result<Option<EstateMembership>, sqlx::Error> {
        let answer: Option<(Option<String>,)> = sqlx::query_as(
            "SELECT el.estate_token_id::text FROM land_authz.index_cursor c \
             LEFT JOIN land_authz.token_right p ON p.token_address=c.land_registry AND p.x=$4 AND p.y=$5 \
             LEFT JOIN land_authz.estate_land el ON el.land_token_id=p.token_id \
             WHERE c.network=$1 AND c.land_registry=$2 AND c.estate_registry=$3 \
               AND c.full_replay_started AND c.folded_block=c.last_block \
               AND c.folded_at <= timezone('UTC', now()) \
               AND c.folded_at >= timezone('UTC', now()) - make_interval(secs => $6)",
        )
        .bind(&self.settings.network)
        .bind(&self.settings.land_registry)
        .bind(&self.settings.estate_registry)
        .bind(x)
        .bind(y)
        .bind(indexer::READY_WINDOW.num_seconds() as f64)
        .fetch_optional(conn)
        .await?;
        Ok(answer.map(|(estate_id,)| EstateMembership { estate_id }))
    }

    /// The Estate token `estate_id` (decimal) when the configured Estate
    /// registry minted it and it holds no LAND now: a dissolved estate, the
    /// atlas's `searchEstateSize: 0` lookup, with the name and description
    /// of its latest data document (`estate_data_event`; empty when it has
    /// none). [`DissolvedEstate::NotDissolved`] when it was never minted or
    /// still holds LAND; [`DissolvedEstate::Unknown`] when the index may not
    /// answer (a database error reads as not ready).
    pub async fn dissolved_estate(&self, estate_id: &str) -> DissolvedEstate {
        let Some(mut snapshot) = self.ready_snapshot().await else {
            return DissolvedEstate::Unknown;
        };
        let row: Result<Option<(Option<String>, Option<String>)>, _> = sqlx::query_as(
            "SELECT d.name, d.description FROM land_authz.token_right e \
             LEFT JOIN LATERAL (SELECT x.name, x.description FROM land_authz.estate_data_event x \
                                 WHERE x.token_address = e.token_address AND x.estate_id = e.token_id \
                                 ORDER BY x.block_number DESC, x.log_index DESC LIMIT 1) d ON true \
             WHERE e.token_address = $1 AND e.token_id = $2::numeric \
               AND NOT EXISTS (SELECT 1 FROM land_authz.estate_land el \
                                WHERE el.estate_token_id = $2::numeric)",
        )
        .bind(&self.settings.estate_registry)
        .bind(estate_id)
        .fetch_optional(&mut *snapshot.tx)
        .await
        .inspect_err(|e| tracing::warn!(error = %e, "land-authz: dissolved-estate lookup failed"));
        match row {
            Ok(Some((name, description))) => DissolvedEstate::Dissolved(EstateData {
                name: name.unwrap_or_default(),
                description: description.unwrap_or_default(),
            }),
            Ok(None) => DissolvedEstate::NotDissolved,
            Err(_) => DissolvedEstate::Unknown,
        }
    }

    /// Attaches the indexer when `LAND_AUTHZ_INDEX_ENABLED` and `ETH_RPC_URL`
    /// are set: one periodic job with a 15 s first-fire jitter and a 1 h
    /// error cap (an RPC outage doubles the pause, never hammers).
    pub fn spawn_jobs(self: &Arc<Self>, jobs: &Jobs) {
        if !self.settings.index_enabled {
            tracing::info!(
                "LAND_AUTHZ_INDEX_ENABLED is false: the LAND authorization indexer is off; the readers answer \
                 from the fold another process publishes into this database, and are unavailable without one"
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
            index_schedule(self.settings.interval),
            move |ctx| {
                let state = state.clone();
                async move { indexer::run_pass(&state, &ctx).await.map(|_| ()) }
            },
        );
    }
}

/// Positional by `parcels`: for every parcel, whether `actor` holds a
/// deploy-granting leg, from the index once it is ready. Before that no
/// source answers: [`Unavailable::NotReady`], and the caller refuses the
/// write (fail-closed).
pub async fn can_deploy_batch(
    state: &LandAuthzState,
    parcels: &[(i32, i32)],
    actor: &str,
) -> Result<Vec<Decision>, Unavailable> {
    let Some(mut snapshot) = state.ready_snapshot().await else {
        return Err(Unavailable::NotReady);
    };
    // An empty set reads no row, and still waits for the index.
    if parcels.is_empty() {
        return Ok(Vec::new());
    }
    let cursor = &snapshot.cursor;
    let source = DecisionSource::Index {
        folded_block: cursor.folded_block.unwrap_or(cursor.last_block),
    };
    let rights = state
        .store()
        .deploy_rights_batch_on(&mut snapshot.tx, parcels)
        .await?;
    Ok(rights
        .into_iter()
        .map(|r| match r {
            Some(rights) => {
                let flags = rights.deploy_flags_for(actor);
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
/// directly or through an estate, from the index once it is ready;
/// [`Unavailable::NotReady`] before that (the same rule as `can_deploy`).
pub async fn owns_land_at(
    state: &LandAuthzState,
    owner: &str,
    coords: &[(i32, i32)],
) -> Result<bool, Unavailable> {
    if let Some(mut snapshot) = state.ready_snapshot().await {
        return Ok(state
            .store()
            .owns_land_at_on(&mut snapshot.tx, owner, coords)
            .await?);
    }
    Err(Unavailable::NotReady)
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
        let exception_block = "0x000000000000000000000000000000a30000000000000000000000000000003b";
        assert_eq!(decode_land_token_id(exception_block).unwrap(), (163, 59));
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
        assert_eq!(mainnet.confirmations, 12);
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
