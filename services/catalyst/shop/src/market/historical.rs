//! Block-pinned holdings for the subgraph worker. Positions and every read
//! they admit come from the same read-only database snapshot.

use std::time::Duration;

use axum::{
    Json, Router,
    body::to_bytes,
    extract::{Request, State},
    http::{StatusCode, header::RETRY_AFTER},
    response::{IntoResponse, Response},
};
use chrono::Utc;
use serde::{Deserialize, de::DeserializeOwned};
use serde_json::{Value, json};
use sqlx::{PgConnection, PgPool};

use super::chain_walk::{self, ChainNet, INDEXER_NFT, INDEXER_THIRD_PARTY_REGISTRY};
use super::wire::{ApiError, NftCategory};
use super::{MarketplaceState, SharedState, nft_index};
use crate::auth::admin_bearer::{bearer_token, timing_safe_eq};
use crate::chain_index::rpc::{BlockHeader, HEADER_BATCH, Rpc};
use crate::land_authz::{ESTATE_REGISTRY_MAINNET, LAND_MAINNET, LAND_REGISTRY_MAINNET};
use crate::rentals::index::RentalsChain;
use crate::shared::routing::post;

const ETH: ChainNet = ChainNet::Ethereum;
const POLYGON: ChainNet = ChainNet::Matic;
const RENTALS: RentalsChain = RentalsChain::Mainnet;
const MAX_BODY: usize = 6 * 1024 * 1024;
const MAX_FILTERS: usize = 10_000;
const MAX_CONTRACTS: usize = 100;
const MIN_ETH_BLOCK_AGE: i64 = 180;
const ITEM_TYPES: [&str; 5] = [
    "undefined",
    "wearable_v1",
    "wearable_v2",
    "smart_wearable_v1",
    "emote_v1",
];

const PATHS: [(&str, Resource); 5] = [
    ("/v1/historical/nfts", Resource::Nfts),
    ("/v1/historical/estates", Resource::Estates),
    ("/v1/historical/rental-assets", Resource::Rentals),
    ("/v1/historical/third-parties", Resource::ThirdParties),
    ("/v1/historical/head", Resource::Head),
];

pub(super) const READ_PATHS: &[&str] =
    &[PATHS[0].0, PATHS[1].0, PATHS[2].0, PATHS[3].0, PATHS[4].0];

#[derive(Clone, Copy)]
enum Resource {
    Nfts,
    Estates,
    Rentals,
    ThirdParties,
    Head,
}

impl Resource {
    fn name(self) -> &'static str {
        match self {
            Self::Nfts => "NFTs",
            Self::Estates => "estates",
            Self::Rentals => "rental assets",
            Self::ThirdParties => "third parties",
            Self::Head => "indexed head",
        }
    }
}

pub(super) fn routes(enabled: bool) -> Router<SharedState> {
    let mut routes = Router::new();
    if enabled {
        for (path, resource) in PATHS {
            routes = routes.route(
                path,
                post(move |State(state), req| respond(state, req, resource)),
            );
        }
    }
    routes
}

enum Failure {
    BadRequest(String),
    TooRecent(i64),
    Busy,
    Unavailable,
    SourceUnavailable(&'static str),
}

impl From<sqlx::Error> for Failure {
    fn from(error: sqlx::Error) -> Self {
        tracing::warn!(%error, "historical marketplace read failed");
        Self::Unavailable
    }
}

impl Failure {
    fn response(self, resource: Resource) -> Response {
        let source = match &self {
            Self::SourceUnavailable(source) => Some(*source),
            _ => None,
        };
        let (code, retry, message) = match self {
            Self::BadRequest(message) => return ApiError::bad_request(message).into_response(),
            Self::TooRecent(block) => (
                "block-too-recent",
                "60",
                format!("Block {block} is too recent to read historical holdings."),
            ),
            Self::Busy => ("busy", "5", "Too many historical reads are running.".into()),
            Self::Unavailable | Self::SourceUnavailable(_) => (
                "unavailable",
                "30",
                format!("Could not fetch the {} right now", resource.name()),
            ),
        };
        (
            StatusCode::SERVICE_UNAVAILABLE,
            [(RETRY_AFTER, retry)],
            Json({
                let mut body = json!({"ok":false, "code":code, "message":message});
                if let Some(source) = source {
                    body["source"] = json!(source);
                }
                body
            }),
        )
            .into_response()
    }
}

async fn respond(state: SharedState, req: Request, resource: Resource) -> Response {
    let Some(expected) = state.settings.historical_admin_token.as_deref() else {
        return StatusCode::NOT_FOUND.into_response();
    };
    if !bearer_token(req.headers())
        .is_some_and(|token| timing_safe_eq(expected.as_bytes(), token.as_bytes()))
    {
        return ApiError::unauthorized("Unauthorized").into_response();
    }
    let Ok(_slot) = state.historical_slots.try_acquire() else {
        return Failure::Busy.response(resource);
    };
    let read = async {
        let bytes = to_bytes(req.into_body(), MAX_BODY)
            .await
            .map_err(|_| Failure::BadRequest("Could not read request body".into()))?;
        let data: Value = serde_json::from_slice(&bytes)
            .map_err(|_| Failure::BadRequest("Invalid historical request".into()))?;
        lookup(&state, resource, data).await
    };
    match read.await {
        Ok(data) => Json(json!({"ok":true, "data":data})).into_response(),
        Err(error) => error.response(resource),
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Nfts {
    owners: Vec<String>,
    category: Option<NftCategory>,
    contract_addresses: Option<Vec<String>>,
    item_types: Option<Vec<String>>,
    estate_size_gt: Option<i64>,
    id_gt: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Estates {
    token_ids: Vec<String>,
    size_gt: Option<i64>,
    with_parcels: Option<bool>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Rentals {
    lessors: Vec<String>,
    contract_addresses: Option<Vec<String>>,
    is_claimed: Option<bool>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ThirdParties {
    ids: Vec<String>,
    is_approved: Option<bool>,
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum Squid {
    Marketplace,
    Registry,
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum Chain {
    Ethereum,
    Polygon,
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum Processor {
    Vesting,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Head {
    squid: Squid,
    chain: Chain,
    processor: Option<Processor>,
}

#[derive(Deserialize)]
struct Page {
    block: i64,
    first: Option<i64>,
    skip: Option<i64>,
}

fn parse<T: DeserializeOwned>(value: Value) -> Result<T, Failure> {
    serde_json::from_value(value).map_err(|error| Failure::BadRequest(error.to_string()))
}

fn page_body(mut body: Value) -> Result<(Page, Value), Failure> {
    let Some(object) = body.as_object_mut() else {
        return Err(Failure::BadRequest("Expected an object".into()));
    };
    let block = object.remove("block");
    let first = object.remove("first");
    let skip = object.remove("skip");
    let mut page: Page = parse(json!({ "block":block, "first":first, "skip":skip }))?;
    let first = page.first.get_or_insert(100);
    let skip = page.skip.get_or_insert(0);
    if !(1..=i64::from(i32::MAX)).contains(&page.block) || !(0..=1000).contains(first) || *skip < 0
    {
        return Err(Failure::BadRequest("Invalid block, first or skip".into()));
    }
    Ok((page, body))
}

fn check_values(
    values: &[String],
    max: usize,
    name: &str,
    valid: impl Fn(&str) -> bool,
) -> Result<(), Failure> {
    if values.len() > max || values.iter().any(|value| !valid(value)) {
        return Err(Failure::BadRequest(format!("Invalid {name}")));
    }
    Ok(())
}

fn addresses(values: &mut [String], max: usize, name: &str) -> Result<(), Failure> {
    check_values(values, max, name, |value| {
        value
            .strip_prefix("0x")
            .is_some_and(|hex| hex.len() == 40 && hex.bytes().all(|c| c.is_ascii_hexdigit()))
    })?;
    for value in values {
        value.make_ascii_lowercase();
    }
    Ok(())
}

struct Position {
    number: i64,
    hash: Option<String>,
    timestamp: Option<i64>,
}

impl Position {
    fn covers(&self, block: i64) -> Result<(), Failure> {
        (block <= self.number)
            .then_some(())
            .ok_or(Failure::TooRecent(block))
    }
}

async fn snapshot(pool: &PgPool) -> Result<sqlx::Transaction<'_, sqlx::Postgres>, Failure> {
    let mut tx = pool.begin().await?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY")
        .execute(&mut *tx)
        .await?;
    sqlx::query("SET LOCAL statement_timeout = '10s'")
        .execute(&mut *tx)
        .await?;
    Ok(tx)
}

async fn position(
    db: &mut PgConnection,
    net: ChainNet,
    indexer: &str,
) -> Result<Position, Failure> {
    let source = match indexer {
        INDEXER_NFT => "nft-index",
        INDEXER_THIRD_PARTY_REGISTRY => "third-party-registry-index",
        _ => "rentals-index",
    };
    let ready = chain_walk::published_sql_on("c", "timezone('UTC', now())", 1, 137);
    let (number, hash, timestamp) = sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "SELECT c.folded_block, p.block_hash, p.block_time
         FROM shop.chain_cursor c LEFT JOIN shop.chain_checkpoint p
           ON (p.indexer,p.network,p.block_number)=(c.indexer,c.network,c.folded_block)
         WHERE c.network=$1 AND c.indexer=$2 AND {ready}
           AND c.diverged_at IS NULL AND c.rebuild_requested_at IS NULL"
    )))
    .bind(net.db_name())
    .bind(indexer)
    .fetch_optional(db)
    .await?
    .ok_or(Failure::SourceUnavailable(source))?;
    Ok(Position {
        number,
        hash,
        timestamp,
    })
}

async fn land_position(db: &mut PgConnection) -> Result<Position, Failure> {
    let (number, hash) = sqlx::query_as(
        "SELECT folded_block, folded_block_hash FROM land_authz.index_cursor
         WHERE network=$1 AND land_registry=$2 AND estate_registry=$3
           AND full_replay_started AND history_timestamps_ready AND folded_block IS NOT NULL
           AND folded_at BETWEEN timezone('UTC', now()) - interval '30 minutes' AND timezone('UTC', now())"
    )
    .bind(LAND_MAINNET.network).bind(LAND_REGISTRY_MAINNET).bind(ESTATE_REGISTRY_MAINNET)
    .fetch_optional(db).await?.ok_or(Failure::SourceUnavailable("land-authz-index"))?;
    Ok(Position {
        number,
        hash,
        timestamp: None,
    })
}

async fn reviewed_position(db: &mut PgConnection, block: Option<i64>) -> Result<Position, Failure> {
    let position = position(db, POLYGON, INDEXER_THIRD_PARTY_REGISTRY).await?;
    let block = block.unwrap_or(position.number);
    position.covers(block)?;
    let incomplete: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM shop.third_party_registry_event WHERE block_number <= $1 AND is_review IS NULL)"
    ).bind(block).fetch_one(db).await?;
    if incomplete {
        return Err(Failure::SourceUnavailable("third-party-registry-index"));
    }
    Ok(position)
}

fn rpc_endpoint(state: &MarketplaceState, net: ChainNet) -> Option<String> {
    match net {
        ETH => state.chain_rpc_url(ETH),
        POLYGON => state.polygon_mainnet_rpc_url(),
    }
}

async fn eth_block(rpc: &Rpc<'_>, block: i64) -> Result<BlockHeader, Failure> {
    tokio::time::timeout(Duration::from_secs(5), rpc.block_header(block as u64))
        .await
        .map_err(|_| Failure::SourceUnavailable("ethereum-rpc"))?
        .map_err(|_| Failure::SourceUnavailable("ethereum-rpc"))
}

async fn old_enough(state: &MarketplaceState, block: i64) -> Result<(), Failure> {
    let endpoint = rpc_endpoint(state, ETH).ok_or(Failure::SourceUnavailable("ethereum-rpc"))?;
    let header = eth_block(&Rpc::at(&state.http, &endpoint), block).await?;
    if header.timestamp > Utc::now().timestamp() - MIN_ETH_BLOCK_AGE {
        return Err(Failure::TooRecent(block));
    }
    Ok(())
}

async fn hashed_block(rpc: &Rpc<'_>, hash: &str) -> Option<(Position, String)> {
    let result = rpc
        .call("eth_getBlockByHash", json!([hash, false]))
        .await
        .ok()?;
    let (position, parent) = header_with_parent(&result["result"])?;
    position
        .hash
        .as_deref()?
        .eq_ignore_ascii_case(hash)
        .then_some((position, parent))
}

fn header_with_parent(block: &Value) -> Option<(Position, String)> {
    let hash = block["hash"].as_str()?;
    let parent = block["parentHash"].as_str()?;
    hash.parse::<alloy_primitives::B256>().ok()?;
    parent.parse::<alloy_primitives::B256>().ok()?;
    Some((
        Position {
            number: crate::chain_index::rpc::hex_to_i64(block["number"].as_str()?)?,
            hash: Some(hash.to_owned()),
            timestamp: Some(crate::chain_index::rpc::hex_to_i64(
                block["timestamp"].as_str()?,
            )?),
        },
        parent.to_owned(),
    ))
}

async fn linked_timestamp(
    rpc: &Rpc<'_>,
    low: &Position,
    high: &Position,
    low_hash: &str,
    high_hash: &str,
) -> Option<i64> {
    let mut hash = high_hash.to_owned();
    let numbers = (low.number..=high.number).rev();
    if high.number - low.number <= 32 {
        for number in numbers {
            let (header, parent) = hashed_block(rpc, &hash).await?;
            if number != header.number {
                return None;
            }
            if number == low.number {
                return header
                    .timestamp
                    .filter(|_| hash.eq_ignore_ascii_case(low_hash));
            }
            hash = parent;
        }
        return None;
    }
    // Numbered replies are candidates only. The recorded high hash and every
    // parent link must lead all the way to the recorded low hash.
    let numbers: Vec<_> = numbers.collect();
    for chunk in numbers.chunks(HEADER_BATCH) {
        let requests: Vec<_> = chunk
            .iter()
            .map(|number| {
                crate::http::jsonrpc::request(
                    *number as u64,
                    "eth_getBlockByNumber",
                    json!([format!("0x{number:x}"), false]),
                )
            })
            .collect();
        let mut headers = rpc
            .batch(&requests)
            .await
            .ok()?
            .iter()
            .map(|reply| header_with_parent(&reply["result"]))
            .collect::<Option<Vec<_>>>()?;
        if headers.len() != chunk.len() {
            return None;
        }
        headers.sort_unstable_by_key(|(header, _)| std::cmp::Reverse(header.number));
        for (&number, (header, parent)) in chunk.iter().zip(headers) {
            if header.number != number || !header.hash.as_deref()?.eq_ignore_ascii_case(&hash) {
                return None;
            }
            if number == low.number {
                return header
                    .timestamp
                    .filter(|_| hash.eq_ignore_ascii_case(low_hash));
            }
            hash = parent;
        }
    }
    None
}

async fn common_prefix(
    state: &MarketplaceState,
    net: ChainNet,
    left: Position,
    right: Position,
) -> Result<Position, Failure> {
    let (low, high) = if left.number <= right.number {
        (left, right)
    } else {
        (right, left)
    };
    let source = match net {
        ETH => "ethereum-publication",
        POLYGON => "collections-index",
    };
    let unavailable = Failure::SourceUnavailable(source);
    let low_hash = low
        .hash
        .as_deref()
        .ok_or(Failure::SourceUnavailable(source))?;
    let high_hash = high
        .hash
        .as_deref()
        .ok_or(Failure::SourceUnavailable(source))?;
    if low.number == high.number {
        return low_hash
            .eq_ignore_ascii_case(high_hash)
            .then_some(low)
            .ok_or(unavailable);
    }
    const MAX_LINK_BLOCKS: i64 = 1024;
    if high.number - low.number > MAX_LINK_BLOCKS {
        return Err(unavailable);
    }
    let endpoint = rpc_endpoint(state, net).ok_or(Failure::SourceUnavailable(source))?;
    let rpc = Rpc::at(&state.http, &endpoint);
    let timestamp = tokio::time::timeout(
        Duration::from_secs(5),
        linked_timestamp(&rpc, &low, &high, low_hash, high_hash),
    )
    .await
    .ok()
    .flatten()
    .ok_or(unavailable)?;
    Ok(Position {
        timestamp: Some(timestamp),
        ..low
    })
}

async fn catalogue_position(
    state: &MarketplaceState,
    db: &mut PgConnection,
    net: ChainNet,
    nft: Position,
    mirror: &str,
) -> Result<Position, Failure> {
    let source = if mirror == "collections" {
        "collections-index"
    } else {
        "items-mirror"
    };
    let (native_coverage, native_hash, complete_hash): (Option<i64>, Option<String>, Option<String>) = sqlx::query_as(
        "WITH native AS (
            SELECT CASE WHEN projection.pending=0 AND c.chain_id=$2 AND c.full_history
                 AND c.quarantined_at IS NULL AND c.diverged_at IS NULL
                 AND c.rebuild_requested_at IS NULL
                 AND c.folded_at BETWEEN timezone('UTC',now()) - interval '30 minutes' AND timezone('UTC',now())
                 AND c.folded_block IS NOT NULL AND projection.projected_through IS NOT NULL
                 THEN least(projection.projected_through,c.folded_block) ELSE -1 END AS covered,
                 p.block_hash
            FROM shop.collection_projection projection
            LEFT JOIN shop.chain_cursor c ON c.network=projection.network AND c.indexer='collections'
            LEFT JOIN shop.chain_checkpoint p
               ON (p.indexer,p.network,p.block_number)=
                  (c.indexer,c.network,least(projection.projected_through,c.folded_block))
            WHERE projection.network=$1 AND projection.owned_at IS NOT NULL
         )
         SELECT (SELECT covered FROM native), (SELECT block_hash FROM native), (
            SELECT m.pass_block_hash FROM shop.mirror_state m
            WHERE m.mirror=$3 AND m.network=$1 AND m.chain_id=$2
              AND m.last_complete_pass_at >= m.pass_started_at
         )",
    ).bind(net.db_name()).bind(net.chain_id()).bind(mirror).fetch_one(db).await?;
    let complete_hash = match native_coverage {
        Some(covered) if covered >= 0 => native_hash,
        Some(_) => return Err(Failure::SourceUnavailable(source)),
        None => complete_hash,
    };
    let hash = complete_hash.ok_or(Failure::SourceUnavailable(source))?;
    let endpoint = rpc_endpoint(state, net).ok_or(Failure::SourceUnavailable(source))?;
    let rpc = Rpc::at(&state.http, &endpoint);
    let (catalogue, _) = tokio::time::timeout(Duration::from_secs(5), hashed_block(&rpc, &hash))
        .await
        .ok()
        .flatten()
        .ok_or(Failure::SourceUnavailable(source))?;
    common_prefix(state, net, nft, catalogue).await
}

async fn require_nft_backfill(
    db: &mut PgConnection,
    net: ChainNet,
    block: i64,
    contracts: &[String],
) -> Result<(), Failure> {
    let missing: bool = sqlx::query_scalar(
        "SELECT EXISTS (
            SELECT 1 FROM shop.nft_contract n
            WHERE n.network=$1 AND (cardinality($4::text[])=0 OR n.contract_address=ANY($4))
              AND n.backfill_from_block <= $3 AND coalesce(n.backfilled_to_block,n.backfill_from_block-1)
                  < least($3,n.walked_from_block-1)
         ) OR EXISTS (
            SELECT 1 FROM shop.collections c LEFT JOIN shop.nft_contract n USING(network,contract_address)
            WHERE c.network=$1 AND c.chain_id=$2 AND n.contract_address IS NULL
              AND (cardinality($4::text[])=0 OR c.contract_address=ANY($4))
         )",
    )
    .bind(net.db_name()).bind(net.chain_id() as i32).bind(block).bind(contracts)
    .fetch_one(db).await?;
    if missing {
        return Err(Failure::SourceUnavailable("nft-index"));
    }
    Ok(())
}

async fn lookup(
    state: &MarketplaceState,
    resource: Resource,
    body: Value,
) -> Result<Value, Failure> {
    if state.settings.catalogue_chain(ETH) != Some(ETH.chain_id()) {
        return Err(Failure::Unavailable);
    }
    if matches!(resource, Resource::Head) {
        return head(state, parse(body)?).await;
    }
    let (page, body) = page_body(body)?;
    match resource {
        Resource::Nfts => nfts(state, page, parse(body)?).await,
        Resource::Estates => estates(state, page, parse(body)?).await,
        Resource::Rentals => rentals(state, page, parse(body)?).await,
        Resource::ThirdParties => third_parties(state, page, parse(body)?).await,
        Resource::Head => unreachable!("head has no page"),
    }
}

async fn head(state: &MarketplaceState, req: Head) -> Result<Value, Failure> {
    if req.processor.is_some() {
        return Err(Failure::Unavailable);
    }
    let mut db = snapshot(&state.pool).await?;
    let (net, position) = match (req.squid, req.chain) {
        (Squid::Marketplace, Chain::Ethereum) => {
            let nft = position(&mut db, ETH, INDEXER_NFT).await?;
            let land = land_position(&mut db).await?;
            let common = common_prefix(state, ETH, nft, land).await?;
            let common = catalogue_position(state, &mut db, ETH, common, "collections").await?;
            let common = catalogue_position(state, &mut db, ETH, common, "items").await?;
            require_nft_backfill(&mut db, ETH, common.number, &[]).await?;
            (ETH, common)
        }
        (Squid::Marketplace, Chain::Polygon) => {
            let nft = position(&mut db, POLYGON, INDEXER_NFT).await?;
            let nft = catalogue_position(state, &mut db, POLYGON, nft, "collections").await?;
            let nft = catalogue_position(state, &mut db, POLYGON, nft, "items").await?;
            require_nft_backfill(&mut db, POLYGON, nft.number, &[]).await?;
            (POLYGON, nft)
        }
        (Squid::Registry, Chain::Ethereum) => {
            let rentals = position(&mut db, ETH, RENTALS.indexer()).await?;
            let land = land_position(&mut db).await?;
            (ETH, common_prefix(state, ETH, rentals, land).await?)
        }
        (Squid::Registry, Chain::Polygon) => {
            let registry = reviewed_position(&mut db, None).await?;
            if registry.hash.is_none() {
                return Err(Failure::SourceUnavailable("third-party-registry-index"));
            }
            (POLYGON, registry)
        }
    };
    db.commit().await?;
    let hash = position.hash.ok_or(Failure::Unavailable)?;
    let mut timestamp = position.timestamp;
    if timestamp.is_none()
        && let Some(endpoint) = rpc_endpoint(state, net)
        && let Ok(Ok(reply)) = tokio::time::timeout(
            Duration::from_secs(5),
            Rpc::at(&state.http, &endpoint).call("eth_getBlockByHash", json!([hash, false])),
        )
        .await
        && reply["result"]["hash"]
            .as_str()
            .is_some_and(|h| h.eq_ignore_ascii_case(&hash))
    {
        timestamp = reply["result"]["timestamp"]
            .as_str()
            .and_then(crate::chain_index::rpc::hex_to_i64);
    }
    Ok(json!({"number": position.number, "hash": hash, "timestamp": timestamp}))
}

async fn nfts(state: &MarketplaceState, page: Page, mut req: Nfts) -> Result<Value, Failure> {
    let contracts = req.contract_addresses.get_or_insert_default();
    let types = req.item_types.get_or_insert_default();
    addresses(&mut req.owners, MAX_FILTERS, "owners")?;
    addresses(contracts, MAX_CONTRACTS, "contractAddresses")?;
    check_values(types, ITEM_TYPES.len(), "itemTypes", |v| {
        ITEM_TYPES.contains(&v)
    })?;
    if let Some(id) = &mut req.id_gt {
        if id.chars().count() > 128 || id.contains('\0') {
            return Err(Failure::BadRequest("Invalid idGt".into()));
        }
        *id = id.to_lowercase();
    }
    if req.category.is_none() && contracts.is_empty() {
        return Err(Failure::BadRequest(
            "A category or some contractAddresses are required.".into(),
        ));
    }
    if req.owners.is_empty() {
        return Ok(json!([]));
    }
    let land = [LAND_REGISTRY_MAINNET, ESTATE_REGISTRY_MAINNET];
    let may_read = |address: &str, category| {
        req.category.is_none_or(|wanted| wanted == category)
            && (contracts.is_empty() || contracts.iter().any(|c| c == address))
    };
    let parcel =
        may_read(land[0], NftCategory::Parcel) && types.is_empty() && req.estate_size_gt.is_none();
    let estate = may_read(land[1], NftCategory::Estate) && types.is_empty();
    let name = may_read(nft_index::DCL_REGISTRAR, NftCategory::Ens)
        && types.is_empty()
        && req.estate_size_gt.is_none();
    let wearables = matches!(
        req.category,
        None | Some(NftCategory::Wearable | NftCategory::Emote)
    ) && (contracts.is_empty()
        || contracts
            .iter()
            .any(|c| !land.contains(&c.as_str()) && c != nft_index::DCL_REGISTRAR))
        && req.estate_size_gt.is_none();
    let mut db = snapshot(&state.pool).await?;
    let land_head = if parcel || estate {
        let position = land_position(&mut db).await?;
        position.covers(page.block)?;
        Some(position)
    } else {
        None
    };
    let nft = if name || wearables {
        let position = position(&mut db, ETH, INDEXER_NFT).await?;
        position.covers(page.block)?;
        Some(position)
    } else {
        None
    };
    let nft = match (land_head, nft) {
        (Some(land), Some(nft)) => Some(common_prefix(state, ETH, nft, land).await?),
        (_, nft) => nft,
    };
    old_enough(state, page.block).await?;
    if let Some(nft) = nft.filter(|_| wearables) {
        let covered = catalogue_position(state, &mut db, ETH, nft, "collections").await?;
        if covered.number < page.block {
            return Err(Failure::SourceUnavailable("collections-index"));
        }
        let covered = catalogue_position(state, &mut db, ETH, covered, "items").await?;
        if covered.number < page.block {
            return Err(Failure::SourceUnavailable("items-mirror"));
        }
        require_nft_backfill(&mut db, ETH, page.block, contracts).await?;
    }
    let ids: Vec<_> = [(parcel, land[0]), (estate, land[1])]
        .into_iter()
        .filter_map(|(read, address)| read.then_some(address))
        .collect();
    let rows = sqlx::query_scalar(include_str!("historical_nfts.sql"))
        .bind(page.block)
        .bind(&req.owners)
        .bind(contracts.as_slice())
        .bind(types.as_slice())
        .bind(req.category.map(NftCategory::as_str))
        .bind(req.estate_size_gt)
        .bind(req.id_gt)
        .bind(page.first)
        .bind(page.skip)
        .bind(ids)
        .bind(land[1])
        .bind(nft_index::DCL_REGISTRAR)
        .bind(name)
        .bind(wearables)
        .fetch_all(&mut *db)
        .await?;
    Ok(Value::Array(rows))
}

async fn estates(state: &MarketplaceState, page: Page, req: Estates) -> Result<Value, Failure> {
    check_values(&req.token_ids, MAX_FILTERS, "tokenIds", |v| {
        (1..=78).contains(&v.len()) && v.bytes().all(|c| c.is_ascii_digit())
    })?;
    if req.token_ids.is_empty() {
        return Ok(json!([]));
    }
    let mut db = snapshot(&state.pool).await?;
    land_position(&mut db).await?.covers(page.block)?;
    old_enough(state, page.block).await?;
    let rows = sqlx::query_scalar(include_str!("historical_estates.sql"))
        .bind(page.block)
        .bind(req.token_ids)
        .bind(req.size_gt)
        .bind(req.with_parcels.unwrap_or(false))
        .bind(page.first)
        .bind(page.skip)
        .bind(ESTATE_REGISTRY_MAINNET)
        .fetch_all(&mut *db)
        .await?;
    Ok(Value::Array(rows))
}

async fn rentals(state: &MarketplaceState, page: Page, mut req: Rentals) -> Result<Value, Failure> {
    let contracts = req.contract_addresses.get_or_insert_default();
    addresses(&mut req.lessors, MAX_FILTERS, "lessors")?;
    addresses(contracts, MAX_CONTRACTS, "contractAddresses")?;
    if req.lessors.is_empty() {
        return Ok(json!([]));
    }
    let mut db = snapshot(&state.pool).await?;
    position(&mut db, ETH, RENTALS.indexer())
        .await?
        .covers(page.block)?;
    old_enough(state, page.block).await?;
    // A claimed rental has no lessor, so it cannot match the required filter.
    if req.is_claimed == Some(true) {
        return Ok(json!([]));
    }
    let rows = sqlx::query_scalar(include_str!("historical_rentals.sql"))
        .bind(page.block)
        .bind(req.lessors)
        .bind(contracts.as_slice())
        .bind(page.first)
        .bind(page.skip)
        .bind(RENTALS.venue())
        .fetch_all(&mut *db)
        .await?;
    Ok(Value::Array(rows))
}

async fn third_parties(
    state: &MarketplaceState,
    page: Page,
    req: ThirdParties,
) -> Result<Value, Failure> {
    check_values(&req.ids, MAX_FILTERS, "ids", |v| {
        (1..=256).contains(&v.len()) && v.bytes().all(|c| (b'!'..=b'~').contains(&c))
    })?;
    if req.ids.is_empty() {
        return Ok(json!([]));
    }
    let mut db = snapshot(&state.pool).await?;
    reviewed_position(&mut db, Some(page.block)).await?;
    let rows = sqlx::query_scalar(
        "SELECT jsonb_build_object('id', p.id,'root',review.root,'isApproved',review.approved)
         FROM (SELECT DISTINCT unnest($1::text[]) AS id) p
         CROSS JOIN LATERAL (
           SELECT coalesce(e.state->>'root','') AS root,
                  (e.state->>'approved')::boolean AND coalesce(e.state->>'root','')<>'' AS approved
           FROM shop.third_party_registry_event e
           WHERE e.provider_id=p.id AND e.block_number<=$2 AND e.is_review
           ORDER BY e.block_number DESC,e.log_index DESC LIMIT 1
         ) review
         WHERE $3::boolean IS NULL OR review.approved=$3
         ORDER BY p.id COLLATE \"C\" LIMIT $4 OFFSET $5",
    )
    .bind(req.ids)
    .bind(page.block)
    .bind(req.is_approved)
    .bind(page.first)
    .bind(page.skip)
    .fetch_all(&mut *db)
    .await?;
    Ok(Value::Array(rows))
}
