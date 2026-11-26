//! The readers over the `nft` index (`nft_index`) and the LAND index
//! (`land_authz`): the `/v1/nfts` shapes the subgraph reader cannot serve
//! (`search`, a page past its window), the nft leg of `/v1/prices` (LAND,
//! estates and names on sale, and `assetType=nft`), `/v1/stats/estate/size`
//! and the asset descriptions of the trade notifications.
//!
//! One candidate row per nft from up to four sources, by the old routing:
//! collections (`shop.nft` joined to the mirrored item: a v2 token by
//! `(network, contract, token >> 216)`, a v1 token by its collection URN and
//! the Issue log's wearable id), names (`shop.nft` rows of the
//! DCLRegistrar), and parcels and estates (`land_authz.token_right`, their
//! names from `shop.land_data`, estate sizes from
//! `land_authz.estate_land`, and, when a filter asks, the distance to a plaza
//! and road adjacency from `shop.land_proximity`, the squid's static
//! LAND map). The order legs are the legacy Marketplace
//! order (`shop.nft_order`: open, unexpired at the bound `now`, its
//! seller still the owner) and the open v3 listing (`unified_trades`, the
//! trade-events verdict applied), a row in both belonging to the legacy one,
//! as the old query did. An index a route needs that is not published
//! answers the unavailable envelope naming it: the nft index under
//! `owned_items::Rule::Fresh` (and, for the collections, the items
//! mirror's first complete pass), checked again on the read's own
//! snapshot.

use std::collections::HashMap;

use axum::response::{IntoResponse, Response};
use chrono::Utc;
use sqlx::{PgConnection, PgPool, Row};

use super::catalog::{
    ACCESSORY_CATEGORIES, Builder, HEAD_CATEGORIES, escape_like, unified_trades_cte,
};
use super::chain_walk::ChainNet;
use super::nft_index::Catalogue;
#[cfg(test)]
use super::nft_index::DCL_REGISTRAR;
use super::nfts::{
    EmoteData, EnsData, EstateData, EstateParcel, Nft, NftData, NftFilters, NftResult, NftSortBy,
    ParcelData, ParcelEstate, WearableData,
};
use super::orders::Order;
use super::owned_items::{self, Rule};
use super::wire::{ApiError, Histogram, Network, NftCategory, consolidate, fix_urn};
use super::{MarketplaceState, Source, unavailable};
use crate::land_authz::indexer::ReadySnapshot;
use crate::land_authz::{LandNetwork, land_network_of_registry};

/// The deepest window the index serves (`first + skip`); the total is an
/// exact count up to this bound and the bound past it.
pub const MAX_WINDOW: i64 = 100_000;

/// A LAND row's image as the map service's PNG under `LAND_IMAGE_BASE_URL`
/// (`MarketplaceSettings::land_image_base_url`): the land source emits the
/// `/v1/...` path, the reader puts this node's configured base in front.
pub fn land_image_url(land_image_base: &str, category: &str, image: String) -> String {
    match category {
        "parcel" | "estate" if image.starts_with('/') => {
            format!("{}{image}", land_image_base.trim_end_matches('/'))
        }
        _ => image,
    }
}

/// Which sources a filter set reads, by the old routing: parcels, estates
/// and names for the land shapes, the collections otherwise.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Sources {
    pub collections: bool,
    pub names: bool,
    pub land: bool,
}

pub fn sources_for(f: &NftFilters) -> Sources {
    match f.category {
        Some(NftCategory::Parcel) | Some(NftCategory::Estate) => Sources {
            land: true,
            ..Default::default()
        },
        Some(NftCategory::Ens) => Sources {
            names: true,
            ..Default::default()
        },
        Some(NftCategory::Wearable) | Some(NftCategory::Emote) => Sources {
            collections: true,
            ..Default::default()
        },
        None if f.is_land
            || f.min_estate_size.is_some()
            || f.max_estate_size.is_some()
            || reads_geography(f) =>
        {
            Sources {
                land: true,
                ..Default::default()
            }
        }
        None => Sources {
            collections: true,
            ..Default::default()
        },
    }
}

/// Required sources for a walker page that can return a parcel or estate.
pub(super) fn land_page_sources(f: &NftFilters, land: LandNetwork) -> Option<Sources> {
    let sources = sources_for(f);
    if sources.land {
        return Some(sources);
    }
    if f.category.is_some()
        || matches!(f.network, Some(Network::Matic))
        || super::nfts::filters_collections_only(f)
    {
        return None;
    }
    if super::nfts::merges_land(f) {
        return Some(Sources {
            land: true,
            names: true,
            collections: true,
        });
    }
    let mut contracts = Sources::default();
    for address in &f.contract_addresses {
        if address.eq_ignore_ascii_case(land.land_registry)
            || address.eq_ignore_ascii_case(land.estate_registry)
        {
            contracts.land = true;
        } else if address.eq_ignore_ascii_case(Catalogue::from_land(land).registrar()) {
            contracts.names = true;
        } else {
            contracts.collections = true;
        }
    }
    contracts.land.then_some(contracts)
}

/// The networks a page of `sources` can return for `f`: the collections'
/// networks, and Ethereum for names and LAND.
fn page_networks(f: &NftFilters, sources: Sources) -> Vec<ChainNet> {
    let mut nets = if sources.collections {
        collection_networks(f)
    } else {
        Vec::new()
    };
    if (sources.names || sources.land) && !nets.contains(&ChainNet::Ethereum) {
        nets.push(ChainNet::Ethereum);
    }
    nets
}

/// A page that reads only names, on a walker, at the request clock `now`:
/// the index's page when the Ethereum latest sales are admitted on the
/// snapshot that supplies its rows (`Some`); `None` when the names index or
/// those latest sales are confirmed not ready there (the caller keeps the
/// graph); the read's error when a check cannot run. An admitted page is
/// held to the index path's rules: the `first + skip` window, and the trades
/// mirror's readiness on the same snapshot when it filters or sorts on
/// orders. The snapshot carries the reads' statement timeout.
pub(super) async fn names_page_if_admitted(
    state: &MarketplaceState,
    f: &NftFilters,
    first: i64,
    skip: i64,
    now: chrono::DateTime<Utc>,
) -> Result<Option<(Vec<NftResult>, i64, ReadConn)>, Response> {
    if first.saturating_add(skip) > MAX_WINDOW {
        return Err(
            ApiError::bad_request(format!("first + skip may not exceed {MAX_WINDOW}"))
                .into_response(),
        );
    }
    let sources = Sources {
        names: true,
        ..Default::default()
    };
    let mut conn = match open_read_conn(&state.pool, state.land_network(), f, sources).await {
        Ok(conn) => conn,
        Err(ReadConnError::NftUnavailable(..) | ReadConnError::LandUnavailable) => return Ok(None),
        Err(ReadConnError::Pool(e)) => return Err(read_error(e)),
    };
    let admitted = admitted_last_sales(&mut conn, &[ChainNet::Ethereum], now).await?;
    if !admitted.contains(&ChainNet::Ethereum) {
        return Ok(None);
    }
    if needs_orders(f) {
        super::trades::ready_on(state, &mut conn, now).await?;
    }
    let (data, total) = list_page_at(
        state,
        f,
        first,
        skip,
        sources,
        &mut conn,
        now,
        LastSale::of(&admitted),
    )
    .await?;
    Ok(Some((data, total, conn)))
}

/// The networks the collection source reads for `f`.
fn collection_networks(f: &NftFilters) -> Vec<ChainNet> {
    match f.network {
        Some(Network::Ethereum) => vec![ChainNet::Ethereum],
        Some(Network::Matic) => vec![ChainNet::Matic],
        None => vec![ChainNet::Ethereum, ChainNet::Matic],
    }
}

/// The land_authz index is the LAND/estate source: published, recent, and
/// indexed from `net`'s registries. A yes/no for the gates only: a read of
/// the index runs on [`land_snapshot`].
pub async fn land_ready(pool: &PgPool, net: LandNetwork) -> bool {
    crate::land_authz::indexer::ready_cursor(
        pool,
        net.network,
        net.land_registry,
        net.estate_registry,
    )
    .await
    .is_some()
}

/// [`land_ready`] as the snapshot the LAND/estate reads run on: the check
/// and every read see one state
/// ([`crate::land_authz::indexer::ReadySnapshot`]), so a rebind of the
/// cursor between them cannot join `net`'s rights to another registry's
/// estate links. A database error reads as not ready.
pub async fn land_snapshot(pool: &PgPool, net: LandNetwork) -> Option<ReadySnapshot> {
    match crate::land_authz::indexer::ready_snapshot(
        pool,
        net.network,
        net.land_registry,
        net.estate_registry,
    )
    .await
    {
        Ok(snapshot) => snapshot,
        Err(e) => {
            tracing::warn!(error = %e, "cannot open the land-authz index snapshot");
            None
        }
    }
}

pub(crate) fn land_unavailable() -> Response {
    unavailable(
        Source::LandAuthzIndex,
        "LAND and estate ownership reads the land-authz index, which is not published yet",
    )
}

/// Where a read of [`Sources`] runs: on the land snapshot when it reads
/// LAND ([`land_snapshot`]), else on a read-only snapshot of its own; the
/// nft index is checked again on either.
pub(super) enum ReadConn {
    Land(ReadySnapshot),
    Pool(sqlx::Transaction<'static, sqlx::Postgres>),
}

impl std::ops::Deref for ReadConn {
    type Target = PgConnection;
    fn deref(&self) -> &PgConnection {
        match self {
            Self::Land(snapshot) => &snapshot.tx,
            Self::Pool(conn) => conn,
        }
    }
}

impl std::ops::DerefMut for ReadConn {
    fn deref_mut(&mut self) -> &mut PgConnection {
        match self {
            Self::Land(snapshot) => &mut snapshot.tx,
            Self::Pool(conn) => conn,
        }
    }
}

/// How long one statement of a `/v1/nfts`, prices or estate-size read may
/// run before Postgres cancels it (the pool's own bound is 60 s per
/// statement), answering [`read_error`]'s 503 past it. Each statement has its
/// own bound. A walking node holds the connection across publication checks,
/// the owner's count, page, capped total, estate members, rental reads and
/// order enrichment. Above the slowest
/// statements measured on the seeded index (DATABASE.md): 7 to 8 s for a
/// 32,000-token owner's capped total naming no category, 8.1 s for the
/// deepest page sorted by price naming no category.
pub const READ_STATEMENT_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

/// Past this many tokens, an owner's page sorted by price or listing time
/// is read in two parts like a page without an owner: one part looks up
/// the listing of every token the owner holds (7 to 8 s for 32,000 tokens),
/// two parts start from the listed tokens (0.6 s for the same owner, 240
/// ms for any owner), so the two meet at about a thousand tokens.
pub const LARGE_OWNER_TOKENS: i64 = 1_000;

/// Whether Postgres cancelled the statement ([`READ_STATEMENT_TIMEOUT`]).
fn cancelled(e: &sqlx::Error) -> bool {
    e.as_database_error()
        .is_some_and(|d| d.code().as_deref() == Some("57014"))
}

/// The 503 of a read whose statement ran past [`READ_STATEMENT_TIMEOUT`].
pub(crate) fn timed_out() -> Response {
    super::service_unavailable(&format!(
        "the read ran past its {} s statement bound: narrow the filters or the page window",
        READ_STATEMENT_TIMEOUT.as_secs()
    ))
}

/// A statement of a read failed: a cancelled one is [`timed_out`]'s 503,
/// anything else a 500.
pub(super) fn read_error(e: sqlx::Error) -> Response {
    if cancelled(&e) {
        return timed_out();
    }
    ApiError::internal(e.to_string()).into_response()
}

/// Whether `owner` holds more than [`LARGE_OWNER_TOKENS`] tokens in the nft
/// index (counted up to one past the bound).
async fn holds_many(conn: &mut PgConnection, owner: &str) -> Result<bool, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT count(*) > $2 FROM (SELECT 1 FROM shop.nft WHERE owner = $1 LIMIT $2 + 1) t",
    )
    .bind(owner.to_lowercase())
    .bind(LARGE_OWNER_TOKENS)
    .fetch_one(conn)
    .await
}

/// Why no connection was opened for a read of [`Sources`].
enum ReadConnError {
    /// It reads LAND, and the land-authz index is not ready.
    LandUnavailable,
    /// The nft index (or the items mirror) stopped meeting its rule.
    NftUnavailable(Source, String),
    Pool(sqlx::Error),
}

/// The connection a read of `sources` for `f` runs on: the snapshot of
/// `land`'s index when it reads LAND (unavailable on a network without
/// LAND), else a read-only snapshot; on either the nft index is checked
/// again ([`check_nft`]).
async fn open_read_conn(
    pool: &PgPool,
    land: Option<LandNetwork>,
    f: &NftFilters,
    sources: Sources,
) -> Result<ReadConn, ReadConnError> {
    let mut conn = if sources.land {
        let Some(net) = land else {
            return Err(ReadConnError::LandUnavailable);
        };
        land_snapshot(pool, net)
            .await
            .map(ReadConn::Land)
            .ok_or(ReadConnError::LandUnavailable)?
    } else {
        let mut tx = pool.begin().await.map_err(ReadConnError::Pool)?;
        sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ, READ ONLY")
            .execute(&mut *tx)
            .await
            .map_err(ReadConnError::Pool)?;
        ReadConn::Pool(tx)
    };
    // Transaction-local: the bound ends with the read.
    sqlx::query("SELECT set_config('statement_timeout', $1, true)")
        .bind(format!("{}ms", READ_STATEMENT_TIMEOUT.as_millis()))
        .execute(&mut *conn)
        .await
        .map_err(ReadConnError::Pool)?;
    check_nft(
        &mut conn,
        land.map(Catalogue::from_land).unwrap_or(Catalogue::Mainnet),
        f,
        sources,
    )
    .await
    .map_err(|e| match e {
        owned_items::ReadError::NotReady(source, m) => ReadConnError::NftUnavailable(source, m),
        owned_items::ReadError::Db(e) => ReadConnError::Pool(e),
    })?;
    Ok(conn)
}

/// [`open_read_conn`] for a route: an index that is not ready answers its
/// unavailable envelope.
async fn read_conn(
    pool: &PgPool,
    land: Option<LandNetwork>,
    f: &NftFilters,
    sources: Sources,
) -> Result<ReadConn, Response> {
    open_read_conn(pool, land, f, sources)
        .await
        .map_err(|e| match e {
            ReadConnError::LandUnavailable => land_unavailable(),
            ReadConnError::NftUnavailable(source, m) => unavailable(source, &m),
            ReadConnError::Pool(e) => ApiError::internal(e.to_string()).into_response(),
        })
}

/// The networks whose nft index a read of `sources` for `f` needs: the
/// collections' (the items mirror too), and Ethereum for names and LAND
/// (the names, the LAND data and the legacy orders).
fn nft_networks(f: &NftFilters, sources: Sources) -> (Vec<ChainNet>, Vec<ChainNet>) {
    let collections = if sources.collections {
        collection_networks(f)
    } else {
        Vec::new()
    };
    let others = if (sources.names || sources.land) && !collections.contains(&ChainNet::Ethereum) {
        vec![ChainNet::Ethereum]
    } else {
        Vec::new()
    };
    (collections, others)
}

/// The nft index rule of a read of `sources` for `f`, on `conn`.
async fn check_nft(
    conn: &mut PgConnection,
    catalogue: Catalogue,
    f: &NftFilters,
    sources: Sources,
) -> Result<(), owned_items::ReadError> {
    let (collections, others) = nft_networks(f, sources);
    let now = Utc::now();
    owned_items::check(conn, catalogue, &collections, Rule::Fresh, true, now).await?;
    owned_items::check(conn, catalogue, &others, Rule::Fresh, false, now).await
}

/// [`nft_prices`]'s error when the LAND index stopped being ready after
/// [`require_prices_sources`] passed. It is a string because the prices memo
/// shares errors as strings; the route answers [`land_unavailable`] for it,
/// the envelope `/v1/nfts` answers in the same state.
pub(crate) const NFT_PRICES_LAND_UNAVAILABLE: &str =
    "the nft prices read LAND, and the land-authz index is not published";

/// [`nft_prices`]'s error for a statement [`READ_STATEMENT_TIMEOUT`]
/// cancelled; the route answers [`timed_out`]'s 503 for it.
pub(crate) const NFT_PRICES_TIMED_OUT: &str = "the nft prices read ran past its statement bound";

/// The prefix of [`nft_prices`]'s error for the nft index or the items
/// mirror that stopped meeting its rule after [`require_prices_sources`]:
/// then the source's name, `": "` and the envelope's message, which the
/// route reads back ([`nft_prices_not_ready`]) into the envelope `/v1/nfts`
/// answers in the same state.
const NFT_PRICES_NOT_READY: &str = "the nft prices source stopped being ready: ";

fn nft_prices_not_ready_error(source: Source, message: &str) -> String {
    format!("{NFT_PRICES_NOT_READY}{}: {message}", source.name())
}

/// The source and message an [`nft_prices`] error names when a source
/// stopped being ready after the gate.
pub(crate) fn nft_prices_not_ready(e: &str) -> Option<(Source, &str)> {
    let rest = e.strip_prefix(NFT_PRICES_NOT_READY)?;
    [Source::NftIndex, Source::ItemsMirror]
        .into_iter()
        .find_map(|source| {
            rest.strip_prefix(source.name())
                .and_then(|m| m.strip_prefix(": "))
                .map(|m| (source, m))
        })
}

/// `Err(envelope)` unless every index `sources` needs for `f` is published;
/// LAND reads the index of `land`'s registries (`ETH_NETWORK`'s).
pub async fn require_ready(
    pool: &PgPool,
    land: Option<LandNetwork>,
    f: &NftFilters,
    sources: Sources,
) -> Result<(), Response> {
    let checked = async {
        let mut conn = pool.acquire().await?;
        check_nft(
            &mut conn,
            land.map(Catalogue::from_land).unwrap_or(Catalogue::Mainnet),
            f,
            sources,
        )
        .await
    };
    // A database error reads as not ready: the envelope, never a guess.
    checked.await.map_err(|e| match e {
        owned_items::ReadError::NotReady(source, m) => unavailable(source, &m),
        owned_items::ReadError::Db(e) => {
            tracing::warn!(error = %e, "cannot read shop.chain_cursor; treating the nft index as not ready");
            owned_items::not_ready("the nft index state cannot be read")
        }
    })?;
    let land_ready = match land {
        Some(net) => land_ready(pool, net).await,
        None => false,
    };
    if sources.land && !land_ready {
        return Err(land_unavailable());
    }
    Ok(())
}

/// The networks whose latest sales (`shop.nft_last_sale`) a read folds into
/// `sold_at`: those `sale_fold::last_sale_networks` admits on its snapshot.
/// Elsewhere `sold_at` is the latest legacy `OrderSuccessful` alone.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LastSale {
    pub ethereum: bool,
    pub matic: bool,
}

impl LastSale {
    pub fn of(nets: &[ChainNet]) -> Self {
        Self {
            ethereum: nets.contains(&ChainNet::Ethereum),
            matic: nets.contains(&ChainNet::Matic),
        }
    }

    fn on(self, net: ChainNet) -> bool {
        match net {
            ChainNet::Ethereum => self.ethereum,
            ChainNet::Matic => self.matic,
        }
    }
}

/// `sold_at` from `legacy` and, when `on`, the token's latest sale joined
/// as `ls` by [`last_sale_join`].
fn sold_at(legacy: &str, on: bool) -> String {
    if on {
        format!("greatest({legacy}, ls.sold_at) AS sold_at")
    } else {
        format!("{legacy} AS sold_at")
    }
}

/// The token's latest sale as `ls`, keyed by `(network, contract, token)`
/// expressions, when `on`.
fn last_sale_join(on: bool, network: &str, contract: &str, token: &str) -> String {
    if on {
        format!(
            "LEFT JOIN shop.nft_last_sale ls ON ls.network = {network} AND ls.contract_address = {contract} \
             AND ls.token_id = {token}"
        )
    } else {
        String::new()
    }
}

/// The collection candidates of one network (a v2 token by its item id, a
/// v1 token by its collection URN and wearable id).
fn collection_source(net: ChainNet, catalogue: Catalogue, last_sale: LastSale) -> String {
    let registrar = catalogue.registrar();
    let proof = if catalogue == Catalogue::Testnet {
        let chain = catalogue.chain_id(net);
        format!(
            "AND c.testnet_verified AND i.testnet_verified AND c.chain_id={chain} AND i.chain_id={chain}"
        )
    } else {
        String::new()
    };
    let item_join = match net {
        ChainNet::Matic => {
            "JOIN shop.items i ON i.network = n.network AND i.contract_address = n.contract_address AND i.item_id = n.item_id \
                            JOIN shop.collections c ON c.network = n.network AND c.contract_address = n.contract_address"
        }
        ChainNet::Ethereum => {
            "JOIN shop.collections c ON c.network = n.network AND c.contract_address = n.contract_address \
                               JOIN shop.items i ON i.urn = c.urn || ':' || n.wearable_id"
        }
    };
    format!(
        "SELECT n.network, n.contract_address, n.token_id, CASE WHEN i.item_type = 'emote_v1' THEN 'emote' ELSE 'wearable' END AS category, \
                n.owner, i.name, COALESCE(i.image, '') AS image, n.minted_at AS created_at, n.transferred_at AS updated_at, {sold}, \
                n.issued_id, i.item_id AS item_blockchain_id, i.urn, i.item_type, i.rarity, i.category AS item_category, i.body_shapes, \
                i.description, i.is_smart, i.emote_loop, i.emote_has_sound, i.emote_has_geometry, i.emote_outcome_type, i.creator, \
                c.name AS collection_name, NULL::int AS x, NULL::int AS y, NULL::text AS parcel_estate_id, NULL::text AS parcel_estate_name, NULL::int AS estate_size, \
                NULL::int AS distance_to_plaza, NULL::boolean AS adjacent_to_road \
         FROM shop.nft n {item_join} {last} \
         WHERE n.network = '{net}' AND n.owner IS NOT NULL AND n.contract_address <> '{registrar}' {proof}",
        net = net.db_name(),
        sold = sold_at("n.sold_at", last_sale.on(net)),
        last = last_sale_join(
            last_sale.on(net),
            "n.network",
            "n.contract_address",
            "n.token_id"
        ),
    )
}

fn names_source(catalogue: Catalogue, last_sale: LastSale) -> String {
    let registrar = catalogue.registrar();
    let on = last_sale.on(ChainNet::Ethereum);
    format!(
        "SELECT n.network, n.contract_address, n.token_id, 'ens' AS category, n.owner, n.ens_subdomain AS name, '' AS image, \
                n.minted_at AS created_at, n.transferred_at AS updated_at, {sold}, NULL::numeric AS issued_id, NULL::numeric AS item_blockchain_id, \
                NULL::text AS urn, NULL::text AS item_type, NULL::text AS rarity, NULL::text AS item_category, NULL::text[] AS body_shapes, \
                NULL::text AS description, NULL::boolean AS is_smart, NULL::boolean AS emote_loop, NULL::boolean AS emote_has_sound, \
                NULL::boolean AS emote_has_geometry, NULL::text AS emote_outcome_type, NULL::text AS creator, NULL::text AS collection_name, \
                NULL::int AS x, NULL::int AS y, NULL::text AS parcel_estate_id, NULL::text AS parcel_estate_name, NULL::int AS estate_size, \
                NULL::int AS distance_to_plaza, NULL::boolean AS adjacent_to_road \
         FROM shop.nft n {last} WHERE n.network = 'ethereum' AND n.contract_address = '{registrar}' AND n.owner IS NOT NULL",
        sold = sold_at("n.sold_at", on),
        last = last_sale_join(on, "n.network", "n.contract_address", "n.token_id"),
    )
}

/// Whether the filters read the LAND geography (`land_proximity`).
fn reads_geography(f: &NftFilters) -> bool {
    f.adjacent_to_road || f.min_distance_to_plaza.is_some() || f.max_distance_to_plaza.is_some()
}

/// Parcels and estates from land_authz: ownership from the latest
/// Transfer, the mint time from the first, an estate's size from its
/// current members, a parcel's estate from its membership row; with
/// `geography`, the squid's distance to a plaza and road adjacency from
/// `land_proximity` (an estate: its parcels' least known distance, -1 when
/// none has one, and whether any touches a road), else NULL. The rows are
/// `net`'s registries; with `None` the source matches no row. With
/// `on_sale`, only the tokens of the `on_sale` CTE ([`ON_SALE_CTE`]), named
/// on the registry's own rows so the per-token lookups below run for the
/// listed tokens instead of for every parcel.
fn land_source(
    net: Option<LandNetwork>,
    geography: bool,
    on_sale: bool,
    last_sale: LastSale,
) -> String {
    let on = last_sale.on(ChainNet::Ethereum);
    let (land, estate) = net.map_or(("", ""), |n| (n.land_registry, n.estate_registry));
    let listed = if on_sale {
        " AND (tr.token_address, tr.token_id) IN (SELECT s.contract_address, s.token_id FROM on_sale s WHERE s.network = 'ethereum')"
    } else {
        ""
    };
    let (geo_columns, geo_join) = if geography {
        (
            format!(
                "CASE WHEN tr.token_address = '{land}' THEN COALESCE(lp.distance_to_plaza, -1) \
                      ELSE COALESCE((SELECT min(p.distance_to_plaza) FROM land_authz.estate_land m \
                                     JOIN land_authz.token_right pt ON pt.token_address = '{land}' AND pt.token_id = m.land_token_id \
                                     JOIN shop.land_proximity p ON p.x = pt.x AND p.y = pt.y \
                                     WHERE m.estate_token_id = tr.token_id AND p.distance_to_plaza >= 0), -1) END::int AS distance_to_plaza, \
                 CASE WHEN tr.token_address = '{land}' THEN COALESCE(lp.adjacent_to_road, false) \
                      ELSE EXISTS (SELECT 1 FROM land_authz.estate_land m \
                                   JOIN land_authz.token_right pt ON pt.token_address = '{land}' AND pt.token_id = m.land_token_id \
                                   JOIN shop.land_proximity p ON p.x = pt.x AND p.y = pt.y \
                                   WHERE m.estate_token_id = tr.token_id AND p.adjacent_to_road) END AS adjacent_to_road",
            ),
            format!(
                "LEFT JOIN shop.land_proximity lp ON tr.token_address = '{land}' AND lp.x = tr.x AND lp.y = tr.y"
            ),
        )
    } else {
        (
            "NULL::int AS distance_to_plaza, NULL::boolean AS adjacent_to_road".to_string(),
            String::new(),
        )
    };
    format!(
        "SELECT 'ethereum'::text AS network, tr.token_address AS contract_address, tr.token_id, \
                CASE WHEN tr.token_address = '{land}' THEN 'parcel' ELSE 'estate' END AS category, tr.owner, ld.name, \
                CASE WHEN tr.token_address = '{land}' THEN '/v1/parcels/' || tr.x || '/' || tr.y || '/map.png' \
                     ELSE '/v1/estates/' || tr.token_id || '/map.png' END AS image, \
                COALESCE(minted.block_time, 0) AS created_at, COALESCE(touched.block_time, 0) AS updated_at, \
                {sold}, \
                NULL::numeric AS issued_id, NULL::numeric AS item_blockchain_id, NULL::text AS urn, NULL::text AS item_type, NULL::text AS rarity, \
                NULL::text AS item_category, NULL::text[] AS body_shapes, ld.description, NULL::boolean AS is_smart, NULL::boolean AS emote_loop, \
                NULL::boolean AS emote_has_sound, NULL::boolean AS emote_has_geometry, NULL::text AS emote_outcome_type, NULL::text AS creator, \
                NULL::text AS collection_name, tr.x, tr.y, el.estate_token_id::text AS parcel_estate_id, eld.name AS parcel_estate_name, \
                CASE WHEN tr.token_address = '{estate}' THEN (SELECT count(*)::int FROM land_authz.estate_land m WHERE m.estate_token_id = tr.token_id) END AS estate_size, \
                {geo_columns} \
         FROM land_authz.token_right tr {geo_join} \
         LEFT JOIN shop.land_data ld ON ld.token_address = tr.token_address AND ld.token_id = tr.token_id \
         LEFT JOIN land_authz.estate_land el ON tr.token_address = '{land}' AND el.land_token_id = tr.token_id \
         LEFT JOIN shop.land_data eld ON eld.token_address = '{estate}' AND eld.token_id = el.estate_token_id \
         LEFT JOIN LATERAL (SELECT e.block_time FROM land_authz.authz_event e WHERE e.token_address = tr.token_address AND e.token_id = tr.token_id \
                            AND e.kind = 'transfer' ORDER BY e.block_number, e.log_index LIMIT 1) minted ON true \
         LEFT JOIN LATERAL (SELECT e.block_time FROM land_authz.authz_event e WHERE e.block_number = tr.updated_block AND e.log_index = tr.updated_log) touched ON true \
         {last} \
         WHERE tr.token_address IN ('{land}', '{estate}') AND tr.owner IS NOT NULL{listed}",
        sold = sold_at(
            "(SELECT max(s.block_time) FROM shop.nft_event s WHERE s.network = 'ethereum' AND s.contract_address = tr.token_address \
               AND s.token_id = tr.token_id AND s.kind = 'order_successful')",
            on,
        ),
        last = last_sale_join(on, "'ethereum'", "tr.token_address", "tr.token_id"),
    )
}

/// Whether a filter reads the order legs. The capped total needs them only
/// then: the legs are left joins of at most one row, so they change no
/// count, and a sort does not either.
fn filters_on_orders(f: &NftFilters) -> bool {
    f.is_on_sale.is_some() || f.min_price.is_some() || f.max_price.is_some()
}

/// Whether the filter set reads the order legs (a filter or sort on them).
pub(super) fn needs_orders(f: &NftFilters) -> bool {
    filters_on_orders(f)
        || matches!(
            f.sort_by,
            Some(NftSortBy::RecentlyListed) | Some(NftSortBy::CheapestParcel)
        )
}

fn body_shapes(genders: &[String]) -> Option<Vec<String>> {
    if genders.is_empty() {
        return None;
    }
    let unisex = genders.iter().any(|g| g == "unisex");
    let mut out = Vec::new();
    if unisex || genders.iter().any(|g| g == "male") {
        out.push("BaseMale".to_string());
    }
    if unisex || genders.iter().any(|g| g == "female") {
        out.push("BaseFemale".to_string());
    }
    (!out.is_empty()).then_some(out)
}

/// The WHERE of the candidate set, over the `c` alias.
fn push_filters(b: &mut Builder, f: &NftFilters, wheres: &mut Vec<String>) {
    if let Some(o) = &f.owner {
        let p = b.bind_string(o.to_ascii_lowercase());
        if f.unclaimed_land.is_empty() {
            wheres.push(format!("c.owner = {p}"));
        } else {
            wheres.push(format!("(c.owner = {p} OR c.held_for_lessor)"));
        }
    }
    match f.category {
        // The item's type, a plain column the planner has statistics for:
        // the `category` CASE over it was estimated at a fraction of a
        // percent, and an owner's page drove from the items into every
        // copy of each (2.8 s for a 30,000-token owner on 3.5M tokens,
        // 92 ms with this predicate).
        Some(NftCategory::Wearable) => wheres.push("c.item_type <> 'emote_v1'".into()),
        Some(NftCategory::Emote) => wheres.push("c.item_type = 'emote_v1'".into()),
        Some(cat) => {
            let p = b.bind_string(cat.as_str().to_string());
            wheres.push(format!("c.category = {p}"));
        }
        None => {}
    }
    if f.is_land {
        wheres.push("c.category IN ('parcel', 'estate')".into());
    }
    if let Some(t) = &f.token_id {
        let p = b.bind_string(t.clone());
        wheres.push(format!("c.token_id = {p}::numeric"));
    }
    if !f.contract_addresses.is_empty() {
        let p = b.bind_strings(
            &f.contract_addresses
                .iter()
                .map(|c| c.to_ascii_lowercase())
                .collect::<Vec<_>>(),
        );
        wheres.push(format!("c.contract_address = ANY({p})"));
    }
    if !f.ids.is_empty() {
        let p = b.bind_strings(
            &f.ids
                .iter()
                .map(|i| i.to_ascii_lowercase())
                .collect::<Vec<_>>(),
        );
        wheres.push(format!(
            "(c.contract_address || '-' || c.token_id::text) = ANY({p})"
        ));
    }
    if let Some(i) = &f.item_id {
        let id = i
            .rsplit_once('-')
            .map(|(_, id)| id.to_string())
            .unwrap_or_else(|| i.clone());
        let p = b.bind_string(id);
        wheres.push(format!("c.item_blockchain_id = {p}::numeric"));
    }
    if !f.creator.is_empty() {
        let p = b.bind_strings(
            &f.creator
                .iter()
                .map(|c| c.to_ascii_lowercase())
                .collect::<Vec<_>>(),
        );
        wheres.push(format!("c.creator = ANY({p})"));
    }
    if !f.item_rarities.is_empty() {
        let p = b.bind_strings(&f.item_rarities);
        wheres.push(format!("c.rarity = ANY({p})"));
    }
    if let Some(wc) = &f.wearable_category {
        let p = b.bind_string(wc.clone());
        wheres.push(format!(
            "(c.category = 'wearable' AND c.item_category = {p})"
        ));
    }
    if let Some(ec) = &f.emote_category {
        let p = b.bind_string(ec.clone());
        wheres.push(format!("(c.category = 'emote' AND c.item_category = {p})"));
    }
    if f.is_wearable_head {
        let p = b.bind_strings(
            &HEAD_CATEGORIES
                .iter()
                .map(|s| s.to_string())
                .collect::<Vec<_>>(),
        );
        wheres.push(format!(
            "(c.category = 'wearable' AND c.item_category = ANY({p}))"
        ));
    }
    if f.is_wearable_accessory {
        let p = b.bind_strings(
            &ACCESSORY_CATEGORIES
                .iter()
                .map(|s| s.to_string())
                .collect::<Vec<_>>(),
        );
        wheres.push(format!(
            "(c.category = 'wearable' AND c.item_category = ANY({p}))"
        ));
    }
    if f.is_wearable_smart {
        wheres.push("c.item_type = 'smart_wearable_v1'".into());
    }
    if let Some(shapes) = body_shapes(&f.wearable_genders) {
        let p = b.bind_strings(&shapes);
        wheres.push(format!(
            "(c.category = 'wearable' AND c.body_shapes @> {p})"
        ));
    }
    if let Some(shapes) = body_shapes(&f.emote_genders) {
        let p = b.bind_strings(&shapes);
        wheres.push(format!("(c.category = 'emote' AND c.body_shapes @> {p})"));
    }
    if !f.emote_play_mode.is_empty() && f.emote_play_mode.len() != 2 {
        let looped = f.emote_play_mode.iter().any(|m| m == "loop");
        wheres.push(format!("c.emote_loop = {looped}"));
    }
    if f.emote_has_sound {
        wheres.push("c.emote_has_sound".into());
    }
    if f.emote_has_geometry {
        wheres.push("c.emote_has_geometry".into());
    }
    if f.emote_outcome_type.is_some() {
        // A collections filter: the names and LAND rows of a read that
        // names no category (`item_type` NULL) are not narrowed by it.
        wheres.push("(c.item_type IS NULL OR c.emote_outcome_type IS NOT NULL)".into());
    }
    if f.include_social_emotes == Some(false) {
        wheres.push("c.emote_outcome_type IS NULL".into());
    }
    if let Some(mn) = f.min_estate_size {
        let p = b.bind_i64(mn as i64);
        wheres.push(format!("(c.category <> 'estate' OR c.estate_size >= {p})"));
    }
    if let Some(mx) = f.max_estate_size {
        let p = b.bind_i64(mx as i64);
        wheres.push(format!("(c.category <> 'estate' OR c.estate_size <= {p})"));
    }
    if matches!(f.category, Some(NftCategory::Estate))
        || f.is_land
        || f.min_estate_size.is_some()
        || f.max_estate_size.is_some()
    {
        wheres.push("(c.category <> 'estate' OR c.estate_size > 0)".into());
    }
    if let Some(mn) = f.min_distance_to_plaza {
        let p = b.bind_i64(mn as i64);
        wheres.push(format!("c.distance_to_plaza >= {p}"));
    }
    if let Some(mx) = f.max_distance_to_plaza {
        let p = b.bind_i64(mx as i64);
        wheres.push(format!("c.distance_to_plaza <= {p}"));
    }
    if f.adjacent_to_road {
        wheres.push("c.adjacent_to_road".into());
    }
    if let Some(term) = f.search.as_deref().map(str::trim).filter(|t| !t.is_empty()) {
        let like = b.bind_string(format!("%{}%", escape_like(term)));
        let exact = b.bind_string(term.replace(' ', ""));
        wheres.push(format!(
            "(c.name ILIKE {like} OR c.collection_name ILIKE {like} \
              OR (c.category = 'parcel' AND (c.x::text || ',' || c.y::text) = {exact}) \
              OR (c.category IN ('parcel', 'estate') AND c.description ILIKE {like}))"
        ));
    }
    if let Some(mn) = &f.min_price {
        let p = b.bind_string(mn.clone());
        wheres.push(format!("c.order_price >= {p}::numeric"));
    }
    if let Some(mx) = &f.max_price {
        let p = b.bind_string(mx.clone());
        wheres.push(format!("c.order_price <= {p}::numeric"));
    }
    match f.is_on_sale {
        Some(true) => wheres.push("c.order_price IS NOT NULL".into()),
        Some(false) => wheres.push("c.order_price IS NULL".into()),
        None => {}
    }
}

fn sort_sql(sort: Option<NftSortBy>) -> &'static str {
    match sort {
        Some(NftSortBy::Name) => "ORDER BY c.name ASC NULLS LAST, c.contract_address, c.token_id",
        Some(NftSortBy::RecentlySold) => {
            "ORDER BY c.sold_at DESC NULLS LAST, c.contract_address, c.token_id"
        }
        Some(NftSortBy::CheapestParcel) => {
            "ORDER BY c.order_price ASC NULLS LAST, c.contract_address, c.token_id"
        }
        Some(NftSortBy::RecentlyListed) => {
            "ORDER BY c.order_created_at DESC NULLS LAST, c.contract_address, c.token_id"
        }
        Some(NftSortBy::Newest) | None => {
            "ORDER BY c.created_at DESC, c.contract_address, c.token_id"
        }
    }
}

/// The trades floor of a read now (`trades::mirrored_since`).
fn trades_since(state: &MarketplaceState) -> Option<chrono::NaiveDateTime> {
    super::trades::mirrored_since(&state.settings, Utc::now().naive_utc())
}

/// `require_ready`, and the trades mirror's rule when `f` reads the order
/// legs: the floor above only bounds a local copy, a mirrored one is judged
/// by its marker (`trades::require_ready`).
async fn require_sources(
    state: &MarketplaceState,
    f: &NftFilters,
    sources: Sources,
) -> Result<(), Response> {
    require_ready(&state.pool, state.land_network(), f, sources).await?;
    if needs_orders(f) {
        super::trades::require_ready(state).await?;
    }
    Ok(())
}

/// Whether every row the filters keep has an order (`order_price` set):
/// the candidates are then read from the tokens with an open legacy order
/// or v3 listing ([`ON_SALE_CTE`]) instead of from every token.
fn on_sale_only(f: &NftFilters) -> bool {
    f.is_on_sale == Some(true) || f.min_price.is_some() || f.max_price.is_some()
}

/// The tokens with an open legacy order or a v3 nft listing, whoever signed
/// it: a superset of the rows whose `order_price` is set (that also needs
/// the seller or signer to be the owner), which the filters still apply.
/// `NOW_MS` is the bound clock. A v3 token id that is not a decimal is left
/// out: the order leg compares it as text with the decimal token id, so it
/// never matches a token.
const ON_SALE_CTE: &str = "on_sale AS MATERIALIZED ( \
    SELECT o.network, o.contract_address, o.token_id FROM shop.nft_order o WHERE o.status = 'open' AND o.expires_at > NOW_MS \
    UNION \
    SELECT lower(u.network), u.contract_address_sent, u.token_id::numeric FROM unified_trades u \
    WHERE u.type = 'public_nft_order' AND u.token_id ~ '^[0-9]{1,78}$'), ";

/// Which tokens a source's candidates are read from.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Candidates {
    /// Every held token.
    Every,
    /// The listed tokens ([`ON_SALE_CTE`]) when the filters keep only rows
    /// with an order ([`on_sale_only`]), else every held token.
    ListedWhenFiltered,
    /// The listed tokens.
    Listed,
}

/// Whether the page sorts by an order column, which is NULL on a row with
/// no order (`sort_sql` puts those last, by contract and token).
fn sorts_on_orders(f: &NftFilters) -> bool {
    matches!(
        f.sort_by,
        Some(NftSortBy::RecentlyListed) | Some(NftSortBy::CheapestParcel)
    )
}

/// The candidate legs, one per source, each with its order legs (the
/// `listed` shape), and the CTE prefix they read (`unified_trades`, and
/// `on_sale` for [`on_sale_only`], when the filters read orders): empty, or
/// ending in `, `. LAND reads `land`'s registries, `now_ms` is the bound
/// clock, `since` the trades floor (`trades::mirrored_since`). Without
/// `orders` the order columns are NULL and no order leg is read.
#[allow(
    clippy::too_many_arguments,
    reason = "one argument per input of the candidate SQL"
)]
fn candidate_legs(
    b: &mut Builder,
    f: &NftFilters,
    sources: Sources,
    land: Option<LandNetwork>,
    now_ms: i64,
    since: Option<chrono::NaiveDateTime>,
    from: Candidates,
    orders: bool,
    last_sale: LastSale,
) -> (String, Vec<String>) {
    let catalogue = land.map(Catalogue::from_land).unwrap_or(Catalogue::Mainnet);
    let restricted = orders
        && match from {
            Candidates::Every => false,
            Candidates::ListedWhenFiltered => on_sale_only(f),
            Candidates::Listed => true,
        };
    // On a lessor page only the LAND leg evaluates the extra custody match.
    // FALSE simplifies the other legs to their indexed owner equality.
    let (other_custody, land_custody) = if f.unclaimed_land.is_empty() {
        ("", String::new())
    } else {
        let ids = b.bind_strings(&f.unclaimed_land);
        let venue = land
            .and_then(|net| crate::rentals::index::RentalsChain::from_land_network(net.network))
            .map(|chain| chain.venue())
            .unwrap_or(crate::rentals::index::RENTALS_MAINNET);
        (
            ", FALSE AS held_for_lessor",
            format!(
                ", (b.owner = '{venue}' AND (b.contract_address || '-' || b.token_id::text) = ANY({ids})) AS held_for_lessor"
            ),
        )
    };
    let mut parts = Vec::new();
    if sources.collections {
        parts.extend(collection_networks(f).into_iter().map(|net| {
            (
                collection_source(net, catalogue, last_sale),
                false,
                other_custody,
            )
        }));
    }
    if sources.names {
        parts.push((names_source(catalogue, last_sale), false, other_custody));
    }
    if sources.land {
        parts.push((
            land_source(land, reads_geography(f), restricted, last_sale),
            restricted,
            land_custody.as_str(),
        ));
    }
    let now_ms_p = b.bind_i64(now_ms);
    let legacy = format!(
        "LEFT JOIN LATERAL (SELECT o.* FROM shop.nft_order o WHERE o.network = b.network AND o.contract_address = b.contract_address \
                           AND o.token_id = b.token_id AND o.status = 'open' AND o.expires_at > {now_ms_p} AND o.seller = b.owner) lo ON true"
    );
    let (prefix, columns, v3, restrict) = if orders {
        let now_p = b.bind_time(
            chrono::DateTime::from_timestamp_millis(now_ms)
                .unwrap_or_default()
                .naive_utc(),
        );
        let since_p = b.bind_opt_time(since);
        // Not materialized: each leg's listing lookup then reads the
        // trades its owner signed (`idx_trades_signer_created`) instead
        // of scanning every open listing for every candidate row, which on
        // 8,000 listed tokens and 4,600 listings took 2.9 s.
        let trades = unified_trades_cte(&now_p, &since_p);
        debug_assert!(trades.starts_with("unified_trades AS ("));
        let mut prefix = format!(
            "{}, ",
            trades.replacen(
                "unified_trades AS (",
                "unified_trades AS NOT MATERIALIZED (",
                1
            )
        );
        let restrict = if restricted {
            prefix.push_str(&ON_SALE_CTE.replace("NOW_MS", &now_ms_p));
            " WHERE (b.network, b.contract_address, b.token_id) IN (SELECT network, contract_address, token_id FROM on_sale)"
        } else {
            ""
        };
        (
            prefix,
            "v3.amount_received AS v3_price, v3.created_at AS v3_created_at, \
             COALESCE(lo.price, v3.amount_received) AS order_price, \
             COALESCE(lo.created_at * 1000, (EXTRACT(EPOCH FROM v3.created_at) * 1000)::bigint) AS order_created_at",
            " LEFT JOIN LATERAL (SELECT u.amount_received, u.created_at FROM unified_trades u WHERE u.type = 'public_nft_order' \
                AND lower(u.network) = b.network AND u.contract_address_sent = b.contract_address AND u.token_id = b.token_id::text \
                AND u.signer = b.owner ORDER BY u.created_at DESC LIMIT 1) v3 ON true",
            restrict,
        )
    } else {
        (
            String::new(),
            "NULL::numeric AS v3_price, NULL::timestamp AS v3_created_at, NULL::numeric AS order_price, NULL::bigint AS order_created_at",
            "",
            "",
        )
    };
    let legs = parts
        .into_iter()
        .map(|(source, listed_only, custody)| {
            let restrict = if listed_only { "" } else { restrict };
            format!(
                "SELECT b.*{custody}, lo.order_id, lo.marketplace_address AS order_marketplace, lo.price AS legacy_price, lo.expires_at AS legacy_expires_at, \
                        lo.created_at AS legacy_created_at, lo.updated_at AS legacy_updated_at, {columns} \
                 FROM ({source}) b {legacy}{v3}{restrict}"
            )
        })
        .collect();
    (prefix, legs)
}

/// The candidate set with its order legs as `listed` (a CTE chain), for
/// the total and the histograms. LAND reads `land`'s registries, `now_ms`
/// is the bound clock, `since` the trades floor (`trades::mirrored_since`).
pub fn build_candidates(
    b: &mut Builder,
    f: &NftFilters,
    sources: Sources,
    land: Option<LandNetwork>,
    now_ms: i64,
    since: Option<chrono::NaiveDateTime>,
) -> String {
    candidates_with(b, f, sources, land, now_ms, since, needs_orders(f))
}

/// [`build_candidates`], reading the order legs only with `orders`.
fn candidates_with(
    b: &mut Builder,
    f: &NftFilters,
    sources: Sources,
    land: Option<LandNetwork>,
    now_ms: i64,
    since: Option<chrono::NaiveDateTime>,
    orders: bool,
) -> String {
    let from = Candidates::ListedWhenFiltered;
    let (prefix, legs) = candidate_legs(
        b,
        f,
        sources,
        land,
        now_ms,
        since,
        from,
        orders,
        LastSale::default(),
    );
    format!("WITH {prefix}listed AS ({})", legs.join(" UNION ALL "))
}

/// The page SQL and the capped total SQL (their own argument lists).
///
/// The page takes the `first + skip` best rows of each source under the
/// filters and the sort, then sorts and pages their union: the window of
/// the whole set is always among them, and a source read alone walks an
/// index in the sort's order (`nft_mint_order_idx` for the newest first)
/// instead of sorting every candidate. A sort by price or listing time
/// without an owner reads each source as its listed rows and its rows
/// without an order, which that sort puts last by contract and token.
/// The capped total reads the order legs only when a filter needs them.
/// Measured times are in DATABASE.md, "Wearable and emote ownership
/// workload".
pub fn build_nfts_query(
    f: &NftFilters,
    sources: Sources,
    land: Option<LandNetwork>,
    first: i64,
    skip: i64,
    now_ms: i64,
    since: Option<chrono::NaiveDateTime>,
) -> (
    (String, sqlx::postgres::PgArguments),
    (String, sqlx::postgres::PgArguments),
) {
    build_nfts_query_for(
        f,
        sources,
        land,
        first,
        skip,
        now_ms,
        since,
        false,
        LastSale::default(),
    )
}

/// [`build_nfts_query`], with `large_owner` when the filter's owner holds
/// more than [`LARGE_OWNER_TOKENS`] tokens: that owner's page sorted by
/// price or listing time is read in two parts too. `last_sale` names the
/// networks whose latest sales the page's `sold_at` takes.
#[allow(
    clippy::too_many_arguments,
    reason = "build_nfts_query's inputs, the owner's size and the latest sales"
)]
pub fn build_nfts_query_for(
    f: &NftFilters,
    sources: Sources,
    land: Option<LandNetwork>,
    first: i64,
    skip: i64,
    now_ms: i64,
    since: Option<chrono::NaiveDateTime>,
    large_owner: bool,
    last_sale: LastSale,
) -> (
    (String, sqlx::postgres::PgArguments),
    (String, sqlx::postgres::PgArguments),
) {
    let page = {
        let mut b = Builder::new();
        let orders = needs_orders(f);
        // A sort on an order column without a filter that keeps only the
        // listed rows: each source is read in two parts, so that no read
        // prices every candidate (below). An owner's page keeps one part:
        // its candidates are that owner's tokens, and reading every listed
        // token first costs a typical owner more than it saves; past
        // [`LARGE_OWNER_TOKENS`] it saves more than it costs.
        let split = sorts_on_orders(f) && !on_sale_only(f) && (f.owner.is_none() || large_owner);
        let (prefix, mut legs) = if split && f.is_on_sale != Some(false) {
            let from = Candidates::Listed;
            candidate_legs(
                &mut b, f, sources, land, now_ms, since, from, orders, last_sale,
            )
        } else {
            let from = Candidates::ListedWhenFiltered;
            candidate_legs(
                &mut b, f, sources, land, now_ms, since, from, orders, last_sale,
            )
        };
        let unlisted = if split {
            let from = Candidates::Every;
            let (_, every) = candidate_legs(
                &mut b, f, sources, land, now_ms, since, from, orders, last_sale,
            );
            if f.is_on_sale == Some(false) {
                // No listed row passes the filter.
                legs.clear();
            }
            every
        } else {
            Vec::new()
        };
        let mut wheres = Vec::new();
        push_filters(&mut b, f, &mut wheres);
        let window = b.bind_i64(first + skip);
        let lim = b.bind_i64(first);
        let off = b.bind_i64(skip);
        let where_of = |extra: &str| {
            let all: Vec<&str> = wheres
                .iter()
                .map(String::as_str)
                .chain((!extra.is_empty()).then_some(extra))
                .collect();
            if all.is_empty() {
                String::new()
            } else {
                format!("WHERE {}", all.join(" AND "))
            }
        };
        let sort = sort_sql(f.sort_by);
        // The rows with an order come from the listed tokens, under the
        // sort; the rows without one have NULL in both order columns, so
        // the sort orders them by contract and token, which their source
        // reads from its key. Each part gives its first `first + skip`
        // rows, and the two parts hold every candidate of the source.
        let listed_where = where_of(if split {
            "c.order_price IS NOT NULL"
        } else {
            ""
        });
        let unlisted_where = where_of("c.order_price IS NULL");
        let tops: Vec<String> = legs
            .iter()
            .map(|leg| format!("(SELECT c.* FROM ({leg}) c {listed_where} {sort} LIMIT {window})"))
            .chain(unlisted.iter().map(|leg| {
                format!(
                    "(SELECT c.* FROM ({leg}) c {unlisted_where} ORDER BY c.contract_address, c.token_id LIMIT {window})"
                )
            }))
            .collect();
        b.push(&format!(
            "WITH {prefix}listed AS ({}) SELECT c.*, c.token_id::text AS token_text, c.issued_id::text AS issued_text, \
                    c.item_blockchain_id::text AS item_text, c.legacy_price::text AS legacy_price_text \
             FROM listed c {sort} LIMIT {lim} OFFSET {off}",
            tops.join(" UNION ALL ")
        ));
        (b.sql, b.args)
    };
    let total = {
        let mut b = Builder::new();
        // A sort on the order legs does not change the count.
        let ctes = candidates_with(
            &mut b,
            f,
            sources,
            land,
            now_ms,
            since,
            filters_on_orders(f),
        );
        let mut wheres = Vec::new();
        push_filters(&mut b, f, &mut wheres);
        let cap = b.bind_i64(MAX_WINDOW + 1);
        let where_sql = if wheres.is_empty() {
            String::new()
        } else {
            format!("WHERE {}", wheres.join(" AND "))
        };
        b.push(&format!(
            "{ctes} SELECT count(*) FROM (SELECT 1 FROM listed c {where_sql} LIMIT {cap}) capped"
        ));
        (b.sql, b.args)
    };
    (page, total)
}

fn network_of(s: &str) -> Network {
    if s == "matic" {
        Network::Matic
    } else {
        Network::Ethereum
    }
}

fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    c.next()
        .map(|f| f.to_uppercase().collect::<String>() + c.as_str())
        .unwrap_or_default()
}

/// One page row as the reader's wire result.
fn row_to_result(
    r: &sqlx::postgres::PgRow,
    estates: &std::collections::HashMap<String, Vec<EstateParcel>>,
    land_image_base: &str,
    catalogue: Catalogue,
) -> NftResult {
    let network = network_of(&r.get::<String, _>("network"));
    let chain_id =
        super::wire::ChainId::try_from(catalogue.chain_id(network.graph().into()) as u64)
            .expect("supported NFT catalogue chain");
    let category: String = r.get("category");
    let contract: String = r.get("contract_address");
    let token_id: String = r.get("token_text");
    let name: Option<String> = r.get("name");
    let description: Option<String> = r.get("description");
    let data = match category.as_str() {
        "parcel" => NftData::Parcel {
            parcel: ParcelData {
                x: r.get::<Option<i32>, _>("x")
                    .map(|v| v.to_string())
                    .unwrap_or_default(),
                y: r.get::<Option<i32>, _>("y")
                    .map(|v| v.to_string())
                    .unwrap_or_default(),
                description: description.clone(),
                estate: r
                    .get::<Option<String>, _>("parcel_estate_id")
                    .map(|id| ParcelEstate {
                        name: r
                            .get::<Option<String>, _>("parcel_estate_name")
                            .unwrap_or_else(|| capitalize("estate")),
                        token_id: id,
                    }),
            },
        },
        "estate" => NftData::Estate {
            estate: EstateData {
                size: r.get::<Option<i32>, _>("estate_size").unwrap_or(0) as i64,
                description: description.clone(),
                parcels: estates.get(&token_id).cloned().unwrap_or_default(),
            },
        },
        "ens" => NftData::Ens {
            ens: EnsData {
                subdomain: name.clone().unwrap_or_default(),
            },
        },
        "emote" => NftData::Emote {
            emote: EmoteData {
                body_shapes: r
                    .get::<Option<Vec<String>>, _>("body_shapes")
                    .unwrap_or_default(),
                category: r
                    .get::<Option<String>, _>("item_category")
                    .unwrap_or_default(),
                description: description.clone().unwrap_or_default(),
                rarity: r.get::<Option<String>, _>("rarity").unwrap_or_default(),
                r#loop: r.get::<Option<bool>, _>("emote_loop").unwrap_or(false),
                has_sound: r.get::<Option<bool>, _>("emote_has_sound").unwrap_or(false),
                has_geometry: r
                    .get::<Option<bool>, _>("emote_has_geometry")
                    .unwrap_or(false),
                outcome_type: r.get("emote_outcome_type"),
            },
        },
        _ => NftData::Wearable {
            wearable: WearableData {
                body_shapes: r
                    .get::<Option<Vec<String>>, _>("body_shapes")
                    .unwrap_or_default(),
                category: r
                    .get::<Option<String>, _>("item_category")
                    .unwrap_or_default(),
                description: description.clone().unwrap_or_default(),
                rarity: r.get::<Option<String>, _>("rarity").unwrap_or_default(),
                is_smart: r.get::<Option<bool>, _>("is_smart").unwrap_or(false),
            },
        },
    };
    let owner: String = r.get::<Option<String>, _>("owner").unwrap_or_default();
    let order = r.get::<Option<String>, _>("order_id").map(|id| Order {
        id,
        marketplace_address: r
            .get::<Option<String>, _>("order_marketplace")
            .unwrap_or_default(),
        contract_address: contract.clone(),
        token_id: Some(token_id.clone()),
        owner: owner.clone(),
        buyer: None,
        price: r
            .get::<Option<String>, _>("legacy_price_text")
            .unwrap_or_default(),
        status: "open".to_string(),
        expires_at: r.get::<Option<i64>, _>("legacy_expires_at").unwrap_or(0),
        created_at: r.get::<Option<i64>, _>("legacy_created_at").unwrap_or(0) * 1000,
        updated_at: r.get::<Option<i64>, _>("legacy_updated_at").unwrap_or(0) * 1000,
        network,
        chain_id,
        issued_id: r.get("issued_text"),
        trade_id: Some(String::new()),
        payee: None,
    });
    let nft = Nft {
        active_order_id: order.as_ref().map(|o| o.id.clone()),
        category: category.clone(),
        chain_id,
        contract_address: contract.clone(),
        created_at: r.get::<i64, _>("created_at") * 1000,
        data,
        id: format!("{contract}-{token_id}"),
        image: land_image_url(
            land_image_base,
            &category,
            fix_urn(&r.get::<String, _>("image")),
        ),
        issued_id: r.get("issued_text"),
        item_id: r.get("item_text"),
        name: name
            .filter(|n| !n.is_empty())
            .unwrap_or_else(|| capitalize(&category)),
        network,
        open_rental_id: None,
        owner,
        token_id: token_id.clone(),
        sold_at: r.get::<Option<i64>, _>("sold_at").unwrap_or(0) * 1000,
        updated_at: r.get::<i64, _>("updated_at") * 1000,
        url: format!("/contracts/{contract}/tokens/{token_id}"),
        urn: r.get::<Option<String>, _>("urn").map(|u| fix_urn(&u)),
    };
    NftResult {
        nft,
        order,
        rental: None,
    }
}

/// The member parcels of the page's estates, in one statement.
pub(super) async fn estate_parcels(
    conn: &mut PgConnection,
    estate_ids: &[String],
) -> Result<std::collections::HashMap<String, Vec<EstateParcel>>, sqlx::Error> {
    let mut out: std::collections::HashMap<String, Vec<EstateParcel>> =
        std::collections::HashMap::new();
    if estate_ids.is_empty() {
        return Ok(out);
    }
    let rows = sqlx::query(
        "SELECT el.estate_token_id::text AS estate, land_authz.token_x(el.land_token_id) AS x, land_authz.token_y(el.land_token_id) AS y \
         FROM land_authz.estate_land el WHERE el.estate_token_id = ANY($1::numeric[]) ORDER BY el.estate_token_id, x, y",
    )
    .bind(estate_ids)
    .fetch_all(conn)
    .await?;
    for r in rows {
        out.entry(r.get("estate")).or_default().push(EstateParcel {
            x: r.get("x"),
            y: r.get("y"),
        });
    }
    Ok(out)
}

pub(super) async fn land_page_snapshot(
    state: &MarketplaceState,
    f: &NftFilters,
    sources: Sources,
    now: chrono::DateTime<Utc>,
) -> Result<ReadConn, Response> {
    let mut conn = read_conn(&state.pool, state.land_network(), f, sources).await?;
    if sources.land && state.settings.catalogue() == Catalogue::Testnet {
        super::rentals::require_sepolia_listings(&mut conn).await?;
    }
    if needs_orders(f) {
        super::trades::ready_on(state, &mut conn, now).await?;
    }
    Ok(conn)
}

/// `/v1/nfts` from the indexes: the page over the `first + skip` window and
/// its total (exact up to [`MAX_WINDOW`]).
pub(super) async fn indexed_page(
    state: &MarketplaceState,
    f: &NftFilters,
    first: i64,
    skip: i64,
    now: chrono::DateTime<Utc>,
) -> Result<(Vec<NftResult>, i64, ReadConn), Response> {
    if first.saturating_add(skip) > MAX_WINDOW {
        return Err(
            ApiError::bad_request(format!("first + skip may not exceed {MAX_WINDOW}"))
                .into_response(),
        );
    }
    let mut sources = sources_for(f);
    if super::owned_items::walks_here(state)
        && f.category.is_none()
        && !f.contract_addresses.is_empty()
        && f.contract_addresses
            .iter()
            .any(|addr| addr.eq_ignore_ascii_case(state.settings.catalogue().registrar()))
    {
        sources.names = true;
        sources.collections = f
            .contract_addresses
            .iter()
            .any(|addr| !addr.eq_ignore_ascii_case(state.settings.catalogue().registrar()));
    }
    if super::nfts::merges_land(f) {
        // On the compatibility route, include the NAME and LAND legs when
        // available. Walking nodes take the strict catalogue snapshot before
        // reaching this path for a mixed page.
        let names = Sources {
            names: true,
            ..Default::default()
        };
        sources.names = require_ready(&state.pool, state.land_network(), f, names)
            .await
            .is_ok();
        sources.land = match state.land_network() {
            Some(net) => land_ready(&state.pool, net).await,
            None => false,
        };
    }
    // The page, its total and the estates' parcels read one state.
    let land = state.land_network();
    let mut conn = match open_read_conn(&state.pool, land, f, sources).await {
        Err(ReadConnError::LandUnavailable) if !sources_for(f).land => {
            // The optional LAND leg stopped being ready after its check.
            sources.land = false;
            read_conn(&state.pool, land, f, sources).await?
        }
        Ok(conn) => conn,
        Err(ReadConnError::LandUnavailable) => {
            // When no LAND snapshot can open, preserve the NFT source's
            // refusal first if that publication is also unavailable.
            require_ready(&state.pool, land, f, sources).await?;
            return Err(land_unavailable());
        }
        Err(ReadConnError::NftUnavailable(source, m)) => return Err(unavailable(source, &m)),
        Err(ReadConnError::Pool(e)) => {
            tracing::warn!(error = %e, "cannot open the NFT index snapshot");
            return Err(owned_items::not_ready("the nft index state cannot be read"));
        }
    };
    if needs_orders(f) {
        super::trades::ready_on(state, &mut conn, now).await?;
    }
    let (data, total) = list_page_on(state, f, first, skip, sources, &mut conn, now).await?;
    Ok((data, total, conn))
}

pub(super) async fn list_page_on(
    state: &MarketplaceState,
    f: &NftFilters,
    first: i64,
    skip: i64,
    sources: Sources,
    conn: &mut PgConnection,
    now: chrono::DateTime<Utc>,
) -> Result<(Vec<NftResult>, i64), Response> {
    // The latest sales of a network whose sales publication is valid on
    // this read's snapshot; elsewhere `sold_at` keeps the legacy orders.
    // Only the networks the page can return are checked.
    let nets = admitted_last_sales(conn, &page_networks(f, sources), now).await?;
    list_page_at(
        state,
        f,
        first,
        skip,
        sources,
        conn,
        now,
        LastSale::of(&nets),
    )
    .await
}

/// The networks of `nets` whose latest sales a read on `conn` takes at
/// `now`; a check that cannot run is the read's error.
async fn admitted_last_sales(
    conn: &mut PgConnection,
    nets: &[ChainNet],
    now: chrono::DateTime<Utc>,
) -> Result<Vec<ChainNet>, Response> {
    super::sale_fold::last_sale_networks(conn, nets, now)
        .await
        .map_err(|e| {
            tracing::warn!(error = %e, "cannot read the sales publication for the nfts' latest sales");
            ApiError::internal("cannot read the sales publication").into_response()
        })
}

/// [`list_page_on`] at the request clock `now`, with the latest sales
/// already admitted on `conn`.
#[allow(
    clippy::too_many_arguments,
    reason = "the page's inputs, its clock and its admitted latest sales"
)]
async fn list_page_at(
    state: &MarketplaceState,
    f: &NftFilters,
    first: i64,
    skip: i64,
    sources: Sources,
    conn: &mut PgConnection,
    now: chrono::DateTime<Utc>,
    last_sale: LastSale,
) -> Result<(Vec<NftResult>, i64), Response> {
    let land = state.land_network();
    let now_ms = now.timestamp_millis();
    let large_owner = match &f.owner {
        Some(owner) if sorts_on_orders(f) && !on_sale_only(f) => {
            holds_many(conn, owner).await.map_err(read_error)?
        }
        _ => false,
    };
    let ((page_sql, page_args), (total_sql, total_args)) = build_nfts_query_for(
        f,
        sources,
        land,
        first,
        skip,
        now_ms,
        super::trades::mirrored_since(&state.settings, now.naive_utc()),
        large_owner,
        last_sale,
    );
    let rows = sqlx::query_with(sqlx::AssertSqlSafe(page_sql), page_args)
        .fetch_all(&mut *conn)
        .await
        .map_err(read_error)?;
    let total: i64 = sqlx::query_scalar_with(sqlx::AssertSqlSafe(total_sql), total_args)
        .fetch_one(&mut *conn)
        .await
        .map_err(read_error)?;
    let estates: Vec<String> = rows
        .iter()
        .filter(|r| r.get::<String, _>("category") == "estate")
        .map(|r| r.get::<String, _>("token_text"))
        .collect();
    let parcels = estate_parcels(conn, &estates).await.map_err(read_error)?;
    Ok((
        rows.iter()
            .map(|r| {
                row_to_result(
                    r,
                    &parcels,
                    &state.settings.land_image_base_url,
                    state.settings.catalogue(),
                )
            })
            .collect(),
        total.min(MAX_WINDOW),
    ))
}

/// `nft_prices` reads every match on sale.
fn on_sale(f: &NftFilters) -> NftFilters {
    NftFilters {
        is_on_sale: Some(true),
        sort_by: None,
        ..f.clone()
    }
}

/// The sources `nft_prices` reads (the indexes, and the trades mirror for
/// its order legs), checked apart so a memo of its answer is never served
/// past their budgets.
pub async fn require_prices_sources(
    state: &MarketplaceState,
    f: &NftFilters,
) -> Result<(), Response> {
    let f = on_sale(f);
    require_sources(state, &f, sources_for(&f)).await
}

/// The nft leg of `/v1/prices`: the order price of every nft on sale that
/// matches (upstream reads the nfts query with `isOnSale` for LAND, estates
/// and names); `category` narrows the sources as `/v1/nfts` does. Read
/// only after [`require_prices_sources`]; if the LAND index stops being
/// ready meanwhile, the error is [`NFT_PRICES_LAND_UNAVAILABLE`].
pub async fn nft_prices(state: &MarketplaceState, f: &NftFilters) -> Result<Histogram, String> {
    let f = on_sale(f);
    let sources = sources_for(&f);
    let land = state.land_network();
    let mut conn = open_read_conn(&state.pool, land, &f, sources)
        .await
        .map_err(|e| match e {
            ReadConnError::LandUnavailable => NFT_PRICES_LAND_UNAVAILABLE.to_string(),
            ReadConnError::NftUnavailable(source, m) => nft_prices_not_ready_error(source, &m),
            ReadConnError::Pool(e) if cancelled(&e) => NFT_PRICES_TIMED_OUT.to_string(),
            ReadConnError::Pool(e) => format!("acquiring a connection for the nft prices: {e}"),
        })?;
    let mut b = Builder::new();
    let ctes = build_candidates(
        &mut b,
        &f,
        sources,
        land,
        Utc::now().timestamp_millis(),
        trades_since(state),
    );
    let mut wheres = Vec::new();
    push_filters(&mut b, &f, &mut wheres);
    b.push(&format!(
        "{ctes} SELECT c.order_price::text AS price FROM listed c WHERE {}",
        wheres.join(" AND ")
    ));
    let rows = sqlx::query_with(sqlx::AssertSqlSafe(b.sql), b.args)
        .fetch_all(&mut *conn)
        .await
        .map_err(|e| {
            if cancelled(&e) {
                NFT_PRICES_TIMED_OUT.to_string()
            } else {
                e.to_string()
            }
        })?;
    Ok(consolidate(
        rows.iter()
            .filter_map(|r| r.get::<Option<String>, _>("price")),
    ))
}

/// `/v1/stats/estate/size`: the sizes of the newest 100 estates that match
/// (the old port's bound), tallied.
pub async fn estate_sizes(state: &MarketplaceState, f: &NftFilters) -> Result<Histogram, Response> {
    let f = NftFilters {
        category: Some(NftCategory::Estate),
        sort_by: Some(NftSortBy::Newest),
        ..f.clone()
    };
    let sources = Sources {
        land: true,
        ..Default::default()
    };
    require_sources(state, &f, sources).await?;
    let land = state.land_network();
    let mut conn = read_conn(&state.pool, land, &f, sources).await?;
    let mut b = Builder::new();
    let ctes = build_candidates(
        &mut b,
        &f,
        sources,
        land,
        Utc::now().timestamp_millis(),
        trades_since(state),
    );
    let mut wheres = Vec::new();
    push_filters(&mut b, &f, &mut wheres);
    b.push(&format!(
        "{ctes} SELECT c.estate_size FROM listed c WHERE {} {} LIMIT 100",
        wheres.join(" AND "),
        sort_sql(f.sort_by)
    ));
    let sizes: Vec<Option<i32>> = sqlx::query_scalar_with(sqlx::AssertSqlSafe(b.sql), b.args)
        .fetch_all(&mut *conn)
        .await
        .map_err(read_error)?;
    Ok(consolidate(
        sizes.into_iter().flatten().map(|s| s.to_string()),
    ))
}

/// What a trade notification says about one nft.
#[derive(Clone, Debug, Default)]
pub struct AssetDescription {
    pub owner: String,
    pub image: String,
    pub category: String,
    pub rarity: Option<String>,
    pub name: Option<String>,
}

fn describe(r: &sqlx::postgres::PgRow, land_image_base: &str) -> AssetDescription {
    AssetDescription {
        owner: r.get::<Option<String>, _>("owner").unwrap_or_default(),
        image: land_image_url(
            land_image_base,
            &r.get::<String, _>("category"),
            r.get::<Option<String>, _>("image").unwrap_or_default(),
        ),
        category: r.get("category"),
        rarity: r.get("rarity"),
        name: r.get("name"),
    }
}

/// The source SQL that describes `contract`'s tokens; a LAND or estate
/// registry of any network reads that network's rows.
fn source_of(net: ChainNet, contract: &str, chain_id: i64) -> String {
    let catalogue = if chain_id == net.testnet_chain_id() {
        Catalogue::Testnet
    } else {
        Catalogue::Mainnet
    };
    if net == ChainNet::Ethereum {
        if let Some(land) = land_network_of_registry(contract)
            && Catalogue::from_land(land) == catalogue
        {
            return land_source(Some(land), false, false, LastSale::default());
        }
        if contract == catalogue.registrar() {
            return names_source(catalogue, LastSale::default());
        }
    }
    collection_source(net, catalogue, LastSale::default())
}

/// [`asset_meta`] of many `(contract, token_id)` at once, one query per
/// source, keyed as asked; non-numeric token ids describe nothing.
pub async fn asset_metas(
    pool: &PgPool,
    land_image_base: &str,
    network: &str,
    chain_id: i64,
    keys: &[(String, String)],
) -> Result<HashMap<(String, String), AssetDescription>, sqlx::Error> {
    let net = ChainNet::parse(network).unwrap_or(ChainNet::Matic);
    let mut groups: HashMap<String, (Vec<String>, Vec<String>)> = HashMap::new();
    for (contract, token) in keys {
        if token.is_empty() || !token.bytes().all(|b| b.is_ascii_digit()) {
            continue;
        }
        let g = groups
            .entry(source_of(net, &contract.to_ascii_lowercase(), chain_id))
            .or_default();
        g.0.push(contract.clone());
        g.1.push(token.clone());
    }
    let mut out = HashMap::new();
    for (sources, (contracts, tokens)) in groups {
        let rows = sqlx::query(sqlx::AssertSqlSafe(format!(
            "SELECT u.contract, u.token, c.owner, c.image, c.category, c.rarity, c.name \
             FROM UNNEST($1::text[], $2::text[]) AS u(contract, token) \
             CROSS JOIN LATERAL (SELECT owner, image, category, rarity, name FROM ({sources}) c \
                 WHERE c.contract_address = lower(u.contract) AND c.token_id = u.token::numeric LIMIT 1) c"
        )))
        .bind(&contracts)
        .bind(&tokens)
        .fetch_all(pool)
        .await?;
        for r in &rows {
            out.insert(
                (r.get("contract"), r.get("token")),
                describe(r, land_image_base),
            );
        }
    }
    Ok(out)
}

/// One nft's description from the indexes (collections, names, LAND);
/// `None` when no index row names it.
pub async fn asset_meta<'e>(
    conn: impl sqlx::PgExecutor<'e>,
    land_image_base: &str,
    network: &str,
    chain_id: i64,
    contract: &str,
    token_id: &str,
) -> Result<Option<AssetDescription>, sqlx::Error> {
    let net = ChainNet::parse(network).unwrap_or(ChainNet::Matic);
    let contract = contract.to_ascii_lowercase();
    let sources = source_of(net, &contract, chain_id);
    let row = sqlx::query(sqlx::AssertSqlSafe(format!(
        "SELECT owner, image, category, rarity, name FROM ({sources}) c WHERE c.contract_address = $1 AND c.token_id = $2::numeric LIMIT 1"
    )))
    .bind(&contract)
    .bind(token_id)
    .fetch_optional(conn)
    .await?;
    Ok(row.map(|r| describe(&r, land_image_base)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::land_authz::{ESTATE_REGISTRY_MAINNET, LAND_MAINNET, LAND_REGISTRY_MAINNET};
    use crate::market::nfts::parse_filters;

    /// A read connection carries the statement bound; a statement cancelled
    /// by it answers the 503 naming it, any other failure a 500; an owner
    /// counts as large past [`LARGE_OWNER_TOKENS`] tokens.
    #[tokio::test]
    async fn reads_are_bounded_and_large_owners_are_counted() {
        let Some(own) = crate::market::tests::support::OwnDb::create("read_bound").await else {
            return;
        };
        let pool = own.pool.clone();
        let mut conn = open_read_conn(&pool, None, &NftFilters::default(), Sources::default())
            .await
            .unwrap_or_else(|_| panic!("a read of no source opens"));
        let bound: String = sqlx::query_scalar("SELECT current_setting('statement_timeout')")
            .fetch_one(&mut *conn)
            .await
            .unwrap();
        assert_eq!(bound, format!("{}s", READ_STATEMENT_TIMEOUT.as_secs()));

        let mut tx = pool.begin().await.unwrap();
        sqlx::query("SELECT set_config('statement_timeout', '20ms', true)")
            .execute(&mut *tx)
            .await
            .unwrap();
        let cancelled = sqlx::query("SELECT pg_sleep(1)")
            .execute(&mut *tx)
            .await
            .unwrap_err();
        tx.rollback().await.unwrap();
        let response = read_error(cancelled);
        assert_eq!(
            response.status(),
            axum::http::StatusCode::SERVICE_UNAVAILABLE
        );
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        assert!(
            String::from_utf8_lossy(&body).contains("statement bound"),
            "{body:?}"
        );
        let failed = sqlx::query("SELECT 1 / 0")
            .execute(&pool)
            .await
            .unwrap_err();
        assert_eq!(
            read_error(failed).status(),
            axum::http::StatusCode::INTERNAL_SERVER_ERROR
        );

        let owner = "0x00000000000000000000000000000000000000aa";
        let insert = |n: i64| {
            let pool = pool.clone();
            async move {
                sqlx::query(
                    "INSERT INTO shop.nft (network, contract_address, token_id, owner, item_id, issued_id, minted_block, minted_at, transferred_block, transferred_log, transferred_at) \
                     SELECT 'matic', '0x00000000000000000000000000000000000000c1', g, $1, 1, g, 1, 1, 1, 0, 1 \
                     FROM generate_series((SELECT count(*) FROM shop.nft) + 1, (SELECT count(*) FROM shop.nft) + $2) g",
                )
                .bind(owner)
                .bind(n)
                .execute(&pool)
                .await
                .unwrap();
            }
        };
        let large = || {
            let pool = pool.clone();
            async move {
                let mut conn = pool.acquire().await.unwrap();
                holds_many(&mut conn, &owner.to_uppercase().replace("0X", "0x"))
                    .await
                    .unwrap()
            }
        };
        insert(LARGE_OWNER_TOKENS).await;
        assert!(!large().await, "exactly the bound is not large");
        insert(1).await;
        assert!(large().await, "one past the bound is large");
        drop(conn);
        own.drop().await;
    }

    fn pairs(list: &[(&str, &str)]) -> Vec<(String, String)> {
        list.iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    #[test]
    fn the_sources_follow_the_old_routing() {
        let f = |p: &[(&str, &str)]| sources_for(&parse_filters(&pairs(p)).unwrap());
        assert_eq!(
            f(&[("category", "parcel")]),
            Sources {
                land: true,
                ..Default::default()
            }
        );
        assert_eq!(
            f(&[("category", "ens")]),
            Sources {
                names: true,
                ..Default::default()
            }
        );
        assert_eq!(
            f(&[("isLand", "true")]),
            Sources {
                land: true,
                ..Default::default()
            }
        );
        assert_eq!(
            f(&[("search", "hat")]),
            Sources {
                collections: true,
                ..Default::default()
            }
        );
        assert_eq!(
            f(&[("category", "emote")]),
            Sources {
                collections: true,
                ..Default::default()
            }
        );
    }

    #[test]
    fn the_page_binds_every_filter_and_reads_orders_only_when_asked() {
        let f = parse_filters(&pairs(&[
            ("search", "pirate hat"),
            ("owner", "0xAbCdEfAbCdEfAbCdEfAbCdEfAbCdEfAbCdEfAbCd"),
            ("itemRarity", "epic"),
            ("network", "MATIC"),
        ]))
        .unwrap();
        let ((page, _), (total, _)) = build_nfts_query(
            &f,
            sources_for(&f),
            None,
            100,
            6_000,
            1_700_000_000_000,
            None,
        );
        assert!(
            page.contains("c.name ILIKE")
                && page.contains("c.owner = $")
                && page.contains("c.rarity = ANY("),
            "{page}"
        );
        assert!(
            page.contains("n.network = 'matic'") && !page.contains("n.network = 'ethereum'"),
            "a network filter reads one collection source"
        );
        assert!(
            !page.contains("unified_trades"),
            "no order filter, no v3 leg"
        );
        assert!(
            page.contains("ORDER BY c.created_at DESC")
                && page.contains("LIMIT $")
                && page.contains("OFFSET $")
        );
        assert!(
            total.contains("SELECT count(*) FROM (SELECT 1 FROM listed c"),
            "{total}"
        );
        let on_sale = parse_filters(&pairs(&[
            ("category", "estate"),
            ("isOnSale", "true"),
            ("sortBy", "cheapest_parcel"),
        ]))
        .unwrap();
        let ((page, _), _) =
            build_nfts_query(&on_sale, sources_for(&on_sale), None, 10, 0, 1, None);
        assert!(
            page.contains("unified_trades")
                && page.contains("shop.nft_order o")
                && page.contains("c.order_price IS NOT NULL"),
            "{page}"
        );
        assert!(
            page.contains("land_authz.token_right tr") && page.contains("c.estate_size > 0"),
            "{page}"
        );
        assert!(page.contains("ORDER BY c.order_price ASC NULLS LAST"));
        // The NAME browse's price range: both bounds on the listing price, which reads the order legs.
        let names = parse_filters(&pairs(&[
            ("category", "ens"),
            ("minPrice", "1000"),
            ("maxPrice", "2000"),
            ("sortBy", "cheapest"),
        ]))
        .unwrap();
        let ((page, _), _) = build_nfts_query(&names, sources_for(&names), None, 10, 0, 1, None);
        assert!(
            page.contains("shop.nft_order o")
                && page.contains("c.order_price >= $")
                && page.contains("c.order_price <= $")
                && page.contains("ORDER BY c.order_price ASC NULLS LAST"),
            "{page}"
        );
    }

    /// The geography filters route to LAND and bind their predicates; the
    /// land source reads `land_proximity` only when one is asked for.
    #[test]
    fn the_geography_filters_read_the_static_land_map() {
        let f = parse_filters(&pairs(&[
            ("adjacentToRoad", "true"),
            ("minDistanceToPlaza", "2"),
            ("maxDistanceToPlaza", "5"),
        ]))
        .unwrap();
        assert_eq!(
            sources_for(&f),
            Sources {
                land: true,
                ..Default::default()
            }
        );
        let ((page, _), _) =
            build_nfts_query(&f, sources_for(&f), Some(LAND_MAINNET), 10, 0, 1, None);
        assert!(
            page.contains("shop.land_proximity")
                && page.contains("c.adjacent_to_road")
                && page.contains("c.distance_to_plaza >= $")
                && page.contains("c.distance_to_plaza <= $"),
            "{page}"
        );
        let plain = parse_filters(&pairs(&[("category", "parcel")])).unwrap();
        let ((page, _), _) = build_nfts_query(
            &plain,
            sources_for(&plain),
            Some(LAND_MAINNET),
            10,
            0,
            1,
            None,
        );
        assert!(!page.contains("land_proximity"), "{page}");
    }

    /// The LAND source names no image host; the reader composes the
    /// configured `LAND_IMAGE_BASE_URL` in front of LAND rows only.
    #[test]
    fn land_images_carry_the_configured_base_only() {
        for geography in [false, true] {
            let sql = land_source(Some(LAND_MAINNET), geography, false, LastSale::default());
            assert!(!sql.contains("://"), "{sql}");
        }
        assert_eq!(
            land_image_url(
                "https://maps.example/",
                "parcel",
                "/v1/parcels/1/2/map.png".into()
            ),
            "https://maps.example/v1/parcels/1/2/map.png"
        );
        assert_eq!(
            land_image_url(
                "https://maps.example",
                "estate",
                "/v1/estates/5/map.png".into()
            ),
            "https://maps.example/v1/estates/5/map.png"
        );
        assert_eq!(
            land_image_url(
                "https://maps.example",
                "wearable",
                "https://cdn/thumb.png".into()
            ),
            "https://cdn/thumb.png"
        );
    }

    /// A parcel near a plaza, one beside a road, one off the map, and an
    /// estate of the first two: the squid's values, the estate's least
    /// known distance and any road.
    #[tokio::test]
    async fn the_land_geography_is_the_squids_static_map() {
        let Some(pool) = crate::db::scratch::pool(2).await else {
            return;
        };
        let base = 900_000_000_000_000_000u64 + rand::random::<u32>() as u64 * 10;
        let ids: Vec<String> = (0..4).map(|i| (base + i).to_string()).collect();
        for (i, (contract, xy)) in [
            (LAND_REGISTRY_MAINNET, Some((-102, -10))),
            (LAND_REGISTRY_MAINNET, Some((-150, 63))),
            (LAND_REGISTRY_MAINNET, Some((160, 160))),
            (ESTATE_REGISTRY_MAINNET, None),
        ]
        .into_iter()
        .enumerate()
        {
            sqlx::query("INSERT INTO land_authz.token_right (token_address, token_id, x, y, owner, updated_block, updated_log) VALUES ($1, $2::numeric, $3, $4, '0x00000000000000000000000000000000000000aa', 1, 0)")
                .bind(contract)
                .bind(&ids[i])
                .bind(xy.map(|p| p.0))
                .bind(xy.map(|p| p.1))
                .execute(&pool)
                .await
                .unwrap();
        }
        for member in &ids[..2] {
            sqlx::query("INSERT INTO land_authz.estate_land (land_token_id, estate_token_id, updated_block, updated_log) VALUES ($1::numeric, $2::numeric, 1, 0)").bind(member).bind(&ids[3]).execute(&pool).await.unwrap();
        }
        let rows: Vec<(String, i32, bool)> = sqlx::query_as(sqlx::AssertSqlSafe(format!(
            "SELECT c.token_id::text, c.distance_to_plaza, c.adjacent_to_road FROM ({}) c WHERE c.token_id = ANY($1::numeric[]) ORDER BY c.token_id",
            land_source(Some(LAND_MAINNET), true, false, LastSale::default())
        )))
        .bind(&ids)
        .fetch_all(&pool)
        .await
        .unwrap();
        let got: Vec<(i32, bool)> = rows.iter().map(|(_, d, r)| (*d, *r)).collect();
        assert_eq!(got, [(9, false), (-1, true), (-1, false), (9, true)]);
        // The LAND images sit under the configured base, not a built-in host.
        let parcel = asset_meta(
            &pool,
            "https://maps.example/",
            "ethereum",
            1,
            LAND_REGISTRY_MAINNET,
            &ids[0],
        )
        .await
        .unwrap()
        .unwrap();
        assert_eq!(
            parcel.image,
            "https://maps.example/v1/parcels/-102/-10/map.png"
        );
        let estate = asset_meta(
            &pool,
            "http://127.0.0.1:5210/api",
            "ethereum",
            1,
            ESTATE_REGISTRY_MAINNET,
            &ids[3],
        )
        .await
        .unwrap()
        .unwrap();
        assert_eq!(
            estate.image,
            format!("http://127.0.0.1:5210/api/v1/estates/{}/map.png", ids[3])
        );
        sqlx::query("DELETE FROM land_authz.estate_land WHERE estate_token_id = $1::numeric")
            .bind(&ids[3])
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("DELETE FROM land_authz.token_right WHERE token_id = ANY($1::numeric[])")
            .bind(&ids)
            .execute(&pool)
            .await
            .unwrap();
    }

    /// The page's per-source top window answers exactly the page of the
    /// whole candidate set sorted and paged, and the on-sale candidates
    /// read from the listed tokens answer what every token filtered on its
    /// order answers: over collections, names and LAND with tied mint
    /// times, for every sort, the on-sale and category filters, and pages
    /// that start inside and past the first source's rows. Under the sorts
    /// by price and listing time the listed rows (legacy orders, v3
    /// listings, tied prices and times across sources, a listing by an
    /// address that does not hold the token) come before the rows without
    /// an order, and the pages cross that boundary and start past it.
    #[tokio::test]
    async fn the_per_source_window_pages_like_the_whole_set() {
        let Some(own) = crate::market::tests::support::OwnDb::create("nftpage").await else {
            return;
        };
        let pool = own.pool.clone();
        let now = 1_700_000_000_000;
        // Current at the reads' fixed clock.
        crate::market::trade_events::tests::publish_cursors_at(
            &pool,
            chrono::DateTime::from_timestamp_millis(now).map(|t| t.naive_utc()),
        )
        .await;
        let c = "0x00000000000000000000000000000000000000c1";
        let a = "0x00000000000000000000000000000000000000aa";
        let b = "0x00000000000000000000000000000000000000bb";
        sqlx::raw_sql(sqlx::AssertSqlSafe(format!(
            "INSERT INTO shop.collections (network, contract_address, chain_id, name, creator, owner, urn, is_completed, is_approved, is_editable, created_at, updated_at, reviewed_at, mirrored_at) \
             VALUES ('matic', '{c}', 137, 'Pirates', '{a}', '{a}', 'urn:decentraland:matic:collections-v2:{c}', true, true, false, 1, 1, 1, now()); \
             INSERT INTO shop.items (network, contract_address, item_id, chain_id, item_type, urn, creator, rarity, total_supply, max_supply, available, price, image, name, category, body_shapes, search_is_collection_approved, created_at, updated_at, reviewed_at, mirrored_at) \
             VALUES ('matic', '{c}', 0, 137, 'wearable_v2', 'urn:decentraland:matic:collections-v2:{c}:0', '{a}', 'rare', 1, 9, 1, 5, 'i', 'Hat', 'hat', ARRAY['BaseMale'], true, 1, 1, 1, now()), \
                    ('matic', '{c}', 1, 137, 'emote_v1', 'urn:decentraland:matic:collections-v2:{c}:1', '{a}', 'rare', 1, 9, 1, 5, 'i', 'Dance', 'dance', ARRAY['BaseMale'], true, 1, 1, 1, now()); \
             INSERT INTO shop.nft (network, contract_address, token_id, owner, item_id, issued_id, minted_block, minted_at, transferred_block, transferred_log, transferred_at, sold_at) \
             SELECT 'matic', '{c}', g, CASE WHEN g % 2 = 0 THEN '{a}' ELSE '{b}' END, g % 2, g, g, 100 + g / 3, g, 0, 100 + g, CASE WHEN g % 4 = 0 THEN 500 + g END \
             FROM generate_series(1, 12) g; \
             INSERT INTO shop.nft (network, contract_address, token_id, owner, ens_subdomain, minted_block, minted_at, transferred_block, transferred_log, transferred_at) \
             SELECT 'ethereum', '{DCL_REGISTRAR}', 1000 + g, '{a}', 'name' || g, g, 101 + g / 2, g, 0, 101 + g FROM generate_series(1, 6) g; \
             INSERT INTO shop.nft_order (network, contract_address, token_id, order_id, marketplace_address, status, seller, price, expires_at, created_at, updated_at, updated_block, updated_log) \
             VALUES ('matic', '{c}', 2, 'o1', '0x8e5660b4ab70168b5a6feea0e0315cb49c8cd539', 'open', '{a}', 30, 4000000000000, 10, 10, 1, 0), \
                    ('matic', '{c}', 3, 'o2', '0x8e5660b4ab70168b5a6feea0e0315cb49c8cd539', 'open', '{a}', 10, 4000000000000, 11, 11, 1, 0), \
                    ('matic', '{c}', 4, 'o3', '0x8e5660b4ab70168b5a6feea0e0315cb49c8cd539', 'open', '{a}', 20, 4000000000000, 12, 12, 1, 0), \
                    ('ethereum', '{DCL_REGISTRAR}', 1002, 'o4', '0x8e5660b4ab70168b5a6feea0e0315cb49c8cd539', 'open', '{a}', 20, 4000000000000, 13, 13, 1, 0), \
                    ('ethereum', '{LAND_REGISTRY_MAINNET}', 2, 'o5', '0x8e5660b4ab70168b5a6feea0e0315cb49c8cd539', 'open', '{a}', 25, 4000000000000, 13, 13, 1, 0); \
             INSERT INTO shop.trades (id, network, chain_id, signature, hashed_signature, checks, signer, type, expires_at, effective_since, contract, created_at) \
             SELECT md5('t' || v.n)::uuid, 'MATIC', 137, 's' || v.n, 'h' || v.n, '{{}}', v.signer, 'public_nft_order', '2100-01-01', '2000-01-01', '0xmarket', \
                    timestamp '2023-01-01' + make_interval(days => v.n) \
             FROM (VALUES (1, '{b}', 5, 7), (2, '{b}', 5, 9), (3, '{b}', 6, 3), (4, '{a}', 4, 5)) v(n, signer, token, price); \
             INSERT INTO shop.trade_assets (id, trade_id, direction, asset_type, contract_address, extra) \
             SELECT md5(d || n)::uuid, md5('t' || n)::uuid, d::shop.asset_direction_type, CASE d WHEN 'sent' THEN 3 ELSE 1 END, \
                    CASE d WHEN 'sent' THEN '{c}' ELSE '0xmana' END, '0x' \
             FROM generate_series(1, 4) n, (VALUES ('sent'), ('received')) x(d); \
             INSERT INTO shop.trade_assets_erc721 (asset_id, token_id) \
             SELECT md5('sent' || v.n)::uuid, v.token::text FROM (VALUES (1, 5), (2, 5), (3, 6), (4, 4)) v(n, token); \
             INSERT INTO shop.trade_assets_erc20 (asset_id, amount) \
             SELECT md5('received' || v.n)::uuid, v.price FROM (VALUES (1, 7), (2, 9), (3, 3), (4, 5)) v(n, price); \
             INSERT INTO land_authz.token_right (token_address, token_id, x, y, owner, updated_block, updated_log) \
             SELECT '{LAND_REGISTRY_MAINNET}', g, g, 0, CASE WHEN g=5 THEN '{venue}' ELSE '{a}' END, g, 0 FROM generate_series(1, 5) g; \
             INSERT INTO land_authz.token_right (token_address, token_id, owner, updated_block, updated_log) \
             SELECT '{ESTATE_REGISTRY_MAINNET}', g, '{b}', g, 1 FROM generate_series(3, 4) g; \
             INSERT INTO land_authz.estate_land (land_token_id, estate_token_id, updated_block, updated_log) VALUES (4, 3, 9, 0), (5, 4, 9, 1); \
             INSERT INTO land_authz.authz_event (block_number, log_index, block_time, token_address, kind, token_id, account) \
             SELECT g, 0, 102 + g / 2, '{LAND_REGISTRY_MAINNET}', 'transfer', g, '{a}' FROM generate_series(1, 4) g; \
             UPDATE shop.items SET emote_outcome_type = 'so' WHERE contract_address = '{c}' AND item_id = 1",
            venue = crate::rentals::index::RENTALS_MAINNET
        )))
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO shop.chain_cursor (indexer, network, start_block, last_block, fold_through, folded_block, folded_at, full_history, chain_id)
             VALUES ('nft', 'matic', 0, 100, 100, 100, $1, true, 137)",
        )
        .bind(chrono::DateTime::from_timestamp_millis(now).unwrap().naive_utc())
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO shop.trade_chain_state (trade_id, network, status, folded_block)
             SELECT id, 'matic', 'open', 0 FROM shop.trades",
        )
        .execute(&pool)
        .await
        .unwrap();
        let land = Some(LAND_MAINNET);
        let ids = |rows: &[sqlx::postgres::PgRow]| -> Vec<(String, String)> {
            rows.iter()
                .map(|r| (r.get("contract_address"), r.get("token_text")))
                .collect()
        };
        let mut compared = 0;
        for filters in [
            vec![],
            vec![("category", "wearable")],
            vec![("isOnSale", "true")],
            // 12 wei, in MANA as the route reads it.
            vec![("minPrice", "0.000000000000000012")],
            vec![("owner", a)],
            vec![("emoteOutcomeType", "so")],
            vec![("isOnSale", "false")],
            vec![("isLand", "true")],
        ] {
            for sort in [
                "newest",
                "name",
                "recently_sold",
                "cheapest",
                "recently_listed",
            ] {
                let mut q = filters.clone();
                q.push(("sortBy", sort));
                let mut f = parse_filters(&pairs(&q)).unwrap();
                if f.owner.is_some() {
                    f.unclaimed_land = vec![format!("{LAND_REGISTRY_MAINNET}-5")];
                }
                let mut s = sources_for(&f);
                if crate::market::nfts::merges_land(&f) {
                    s.names = true;
                    s.land = true;
                }
                for (first, skip) in [
                    (2, 0),
                    (3, 2),
                    (2, 1),
                    (1, 4),
                    (2, 4),
                    (3, 5),
                    (6, 0),
                    (4, 7),
                    (50, 0),
                    (5, 30),
                ] {
                    let ((page, args), _) = build_nfts_query(&f, s, land, first, skip, now, None);
                    let got = sqlx::query_with(sqlx::AssertSqlSafe(page), args)
                        .fetch_all(&pool)
                        .await
                        .unwrap();
                    // An owner past LARGE_OWNER_TOKENS: the same page, read
                    // in two parts.
                    let large = if f.owner.is_some() {
                        let ((page, args), _) = build_nfts_query_for(
                            &f,
                            s,
                            land,
                            first,
                            skip,
                            now,
                            None,
                            true,
                            LastSale::default(),
                        );
                        Some(
                            sqlx::query_with(sqlx::AssertSqlSafe(page), args)
                                .fetch_all(&pool)
                                .await
                                .unwrap(),
                        )
                    } else {
                        None
                    };
                    let mut bld = Builder::new();
                    let every = Candidates::Every;
                    let (prefix, legs) = candidate_legs(
                        &mut bld,
                        &f,
                        s,
                        land,
                        now,
                        None,
                        every,
                        needs_orders(&f),
                        LastSale::default(),
                    );
                    let mut wheres = Vec::new();
                    push_filters(&mut bld, &f, &mut wheres);
                    let lim = bld.bind_i64(first);
                    let off = bld.bind_i64(skip);
                    let where_sql = if wheres.is_empty() {
                        String::new()
                    } else {
                        format!("WHERE {}", wheres.join(" AND "))
                    };
                    bld.push(&format!(
                        "WITH {prefix}listed AS ({}) SELECT c.contract_address, c.token_id::text AS token_text FROM listed c {where_sql} {} LIMIT {lim} OFFSET {off}",
                        legs.join(" UNION ALL "),
                        sort_sql(f.sort_by)
                    ));
                    let want = sqlx::query_with(sqlx::AssertSqlSafe(bld.sql), bld.args)
                        .fetch_all(&pool)
                        .await
                        .unwrap();
                    assert_eq!(ids(&got), ids(&want), "{q:?} first={first} skip={skip}");
                    if let Some(large) = &large {
                        assert_eq!(
                            ids(large),
                            ids(&want),
                            "{q:?} first={first} skip={skip}, a large owner"
                        );
                    }
                    compared += usize::from(!want.is_empty());
                }
            }
        }
        assert!(compared > 200, "the cases read rows: {compared}");
        // The listed rows, cheapest first: a v3 listing by the token's owner
        // (its latest one, 9), the legacy orders (one over a v3 listing of
        // the same token) and the parcel's. Token 6 is listed, and token 3
        // has an order, by an address that does not hold it.
        let f = parse_filters(&pairs(&[("isOnSale", "true"), ("sortBy", "cheapest")])).unwrap();
        let s = Sources {
            names: true,
            land: true,
            ..sources_for(&f)
        };
        let ((page, args), (total, total_args)) = build_nfts_query(&f, s, land, 50, 0, now, None);
        let rows = sqlx::query_with(sqlx::AssertSqlSafe(page), args)
            .fetch_all(&pool)
            .await
            .unwrap();
        let on_sale: Vec<(String, String)> = [
            (c, "5"),
            (c, "4"),
            (DCL_REGISTRAR, "1002"),
            (LAND_REGISTRY_MAINNET, "2"),
            (c, "2"),
        ]
        .iter()
        .map(|(contract, token)| (contract.to_string(), token.to_string()))
        .collect();
        assert_eq!(ids(&rows), on_sale);
        let total: i64 = sqlx::query_scalar_with(sqlx::AssertSqlSafe(total), total_args)
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(total, 5);
        // A sort on the order legs leaves them out of the capped total and
        // counts the same rows.
        let totals = |q: &'static [(&'static str, &'static str)]| {
            let f = parse_filters(&pairs(q)).unwrap();
            let s = Sources {
                names: true,
                land: true,
                ..sources_for(&f)
            };
            let (_, (total, args)) = build_nfts_query(&f, s, land, 50, 0, now, None);
            let pool = pool.clone();
            let reads_trades = total.contains("unified_trades");
            async move {
                let count: i64 = sqlx::query_scalar_with(sqlx::AssertSqlSafe(total), args)
                    .fetch_one(&pool)
                    .await
                    .unwrap();
                (count, reads_trades)
            }
        };
        assert_eq!(totals(&[]).await, (25, false));
        assert_eq!(totals(&[("sortBy", "cheapest")]).await, (25, false));
        assert_eq!(totals(&[("sortBy", "recently_listed")]).await, (25, false));
        assert_eq!(
            totals(&[("isOnSale", "false"), ("sortBy", "cheapest")]).await,
            (20, true)
        );
        // `emoteOutcomeType` narrows the collections to the emotes with an
        // outcome and leaves the names and LAND of the same read alone.
        let f = parse_filters(&pairs(&[("emoteOutcomeType", "so")])).unwrap();
        assert!(crate::market::nfts::merges_land(&f));
        let s = Sources {
            names: true,
            land: true,
            ..sources_for(&f)
        };
        let ((page, args), (total, total_args)) = build_nfts_query(&f, s, land, 50, 0, now, None);
        let rows = sqlx::query_with(sqlx::AssertSqlSafe(page), args)
            .fetch_all(&pool)
            .await
            .unwrap();
        let count = |category: &str| {
            rows.iter()
                .filter(|r| r.get::<String, _>("category") == category)
                .count()
        };
        assert_eq!(
            (
                count("emote"),
                count("wearable"),
                count("ens"),
                count("parcel")
            ),
            (6, 0, 6, 5)
        );
        let total: i64 = sqlx::query_scalar_with(sqlx::AssertSqlSafe(total), total_args)
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(total, 19);
        own.drop().await;
    }

    /// A page checks the latest sales only of the networks it can return:
    /// names and LAND read Ethereum, the collections their networks.
    #[test]
    fn a_page_checks_the_latest_sales_of_its_own_networks() {
        let nets = |q: &[(&str, &str)], names: bool, land: bool| {
            let f = parse_filters(&pairs(q)).unwrap();
            let mut s = sources_for(&f);
            s.names |= names;
            s.land |= land;
            page_networks(&f, s)
        };
        assert_eq!(
            nets(&[("category", "ens")], false, false),
            vec![ChainNet::Ethereum]
        );
        assert_eq!(
            nets(&[("category", "parcel")], false, false),
            vec![ChainNet::Ethereum]
        );
        assert_eq!(
            nets(
                &[("category", "wearable"), ("network", "MATIC")],
                false,
                false
            ),
            vec![ChainNet::Matic]
        );
        assert_eq!(
            nets(&[], true, true),
            vec![ChainNet::Ethereum, ChainNet::Matic]
        );
    }

    /// `sold_at` takes a token's latest sale from `shop.nft_last_sale` on
    /// the networks a read admits, the later of it and the legacy order;
    /// the recently sold sort orders by it across the collections, names and
    /// LAND. On a network not admitted, `sold_at` is the legacy order alone.
    #[tokio::test]
    async fn sold_at_takes_the_latest_sale_of_the_admitted_networks() {
        let Some(own) = crate::market::tests::support::OwnDb::create("nftsold").await else {
            return;
        };
        let pool = own.pool.clone();
        let c = "0x00000000000000000000000000000000000000c1";
        let a = "0x00000000000000000000000000000000000000aa";
        sqlx::raw_sql(sqlx::AssertSqlSafe(format!(
            "INSERT INTO shop.collections (network, contract_address, chain_id, name, creator, owner, urn, is_completed, is_approved, is_editable, created_at, updated_at, reviewed_at, mirrored_at) \
             VALUES ('matic', '{c}', 137, 'Pirates', '{a}', '{a}', 'urn:decentraland:matic:collections-v2:{c}', true, true, false, 1, 1, 1, now()); \
             INSERT INTO shop.items (network, contract_address, item_id, chain_id, item_type, urn, creator, rarity, total_supply, max_supply, available, price, image, name, category, body_shapes, search_is_collection_approved, created_at, updated_at, reviewed_at, mirrored_at) \
             VALUES ('matic', '{c}', 0, 137, 'wearable_v2', 'urn:decentraland:matic:collections-v2:{c}:0', '{a}', 'rare', 1, 9, 1, 5, 'i', 'Hat', 'hat', ARRAY['BaseMale'], true, 1, 1, 1, now()); \
             INSERT INTO shop.nft (network, contract_address, token_id, owner, item_id, issued_id, minted_block, minted_at, transferred_block, transferred_log, transferred_at, sold_at) \
             VALUES ('matic', '{c}', 1, '{a}', 0, 1, 1, 101, 1, 0, 101, 500), ('matic', '{c}', 2, '{a}', 0, 2, 2, 102, 2, 0, 102, NULL), \
                    ('matic', '{c}', 3, '{a}', 0, 3, 3, 103, 3, 0, 103, NULL), ('matic', '{c}', 4, '{a}', 0, 4, 4, 104, 4, 0, 104, 900); \
             INSERT INTO shop.nft (network, contract_address, token_id, owner, ens_subdomain, minted_block, minted_at, transferred_block, transferred_log, transferred_at) \
             VALUES ('ethereum', '{DCL_REGISTRAR}', 1001, '{a}', 'name', 1, 105, 1, 0, 105); \
             INSERT INTO land_authz.token_right (token_address, token_id, x, y, owner, updated_block, updated_log) \
             VALUES ('{LAND_REGISTRY_MAINNET}', 5, 1, 1, '{a}', 1, 0); \
             INSERT INTO shop.nft_event (network, block_number, log_index, block_time, kind, emitter, contract_address, token_id, from_address, to_address, amount) \
             VALUES ('ethereum', 7, 0, 600, 'order_successful', '0x8e5660b4ab70168b5a6feea0e0315cb49c8cd539', '{LAND_REGISTRY_MAINNET}', 5, '{a}', '{a}', 1); \
             INSERT INTO shop.nft_last_sale (network, contract_address, token_id, sold_at, block_number) \
             VALUES ('matic', '{c}', 1, 400, 10), ('matic', '{c}', 2, 800, 20), ('matic', '{c}', 3, 1000, 30), \
                    ('ethereum', '{DCL_REGISTRAR}', 1001, 950, 40), ('ethereum', '{LAND_REGISTRY_MAINNET}', 5, 990, 50);"
        )))
        .execute(&pool)
        .await
        .unwrap();
        let f = parse_filters(&pairs(&[("sortBy", "recently_sold")])).unwrap();
        let page = |last_sale: LastSale| {
            let pool = pool.clone();
            let f = f.clone();
            async move {
                // The compatibility route's sources: collections, names and LAND.
                let mut sources = sources_for(&f);
                sources.names = true;
                sources.land = true;
                let land = Some(LAND_MAINNET);
                let ((sql, args), _) =
                    build_nfts_query_for(&f, sources, land, 10, 0, 1, None, false, last_sale);
                sqlx::query_with(sqlx::AssertSqlSafe(sql), args)
                    .fetch_all(&pool)
                    .await
                    .unwrap()
                    .iter()
                    .map(|r| {
                        (
                            r.get::<String, _>("token_text"),
                            r.get::<Option<i64>, _>("sold_at"),
                        )
                    })
                    .collect::<Vec<_>>()
            }
        };
        let row = |t: &str, s: Option<i64>| (t.to_string(), s);
        assert_eq!(
            page(LastSale {
                ethereum: true,
                matic: true
            })
            .await,
            vec![
                row("3", Some(1000)),
                row("5", Some(990)),
                row("1001", Some(950)),
                row("4", Some(900)),
                row("2", Some(800)),
                row("1", Some(500)),
            ]
        );
        assert_eq!(
            page(LastSale {
                ethereum: false,
                matic: true
            })
            .await,
            vec![
                row("3", Some(1000)),
                row("4", Some(900)),
                row("2", Some(800)),
                row("5", Some(600)),
                row("1", Some(500)),
                row("1001", None),
            ]
        );
        assert_eq!(
            page(LastSale::default()).await,
            vec![
                row("4", Some(900)),
                row("5", Some(600)),
                row("1", Some(500)),
                row("2", None),
                row("3", None),
                row("1001", None),
            ]
        );
        own.drop().await;
    }
}
