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
//! answers the unavailable envelope naming it.

use axum::response::{IntoResponse, Response};
use chrono::Utc;
use sqlx::{PgPool, Row};

use super::catalog::{
    ACCESSORY_CATEGORIES, Builder, HEAD_CATEGORIES, escape_like, unified_trades_cte,
};
use super::chain_walk::{ChainNet, INDEXER_NFT, ready_cursor};
use super::nft_index::DCL_REGISTRAR;
use super::nfts::{
    EmoteData, EnsData, EstateData, EstateParcel, Nft, NftData, NftFilters, NftResult, NftSortBy,
    ParcelData, ParcelEstate, WearableData,
};
use super::orders::Order;
use super::wire::{ApiError, Histogram, Network, NftCategory, consolidate, fix_urn};
use super::{MarketplaceState, Source, unavailable};
use crate::land_authz::{ESTATE_REGISTRY_MAINNET, LAND_REGISTRY_MAINNET};

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

/// The networks the collection source reads for `f`.
fn collection_networks(f: &NftFilters) -> Vec<ChainNet> {
    match f.network {
        Some(Network::Ethereum) => vec![ChainNet::Ethereum],
        Some(Network::Matic) => vec![ChainNet::Matic],
        None => vec![ChainNet::Ethereum, ChainNet::Matic],
    }
}

/// The land_authz index is the LAND/estate source: published and recent.
pub async fn land_ready(pool: &PgPool) -> bool {
    crate::land_authz::indexer::ready_cursor(pool, crate::land_authz::NETWORK_MAINNET)
        .await
        .is_some()
}

/// `Err(envelope)` unless every index `sources` needs for `f` is published.
pub async fn require_ready(
    pool: &PgPool,
    f: &NftFilters,
    sources: Sources,
) -> Result<(), Response> {
    let now = Utc::now();
    let mut nets: Vec<ChainNet> = Vec::new();
    if sources.collections {
        nets.extend(collection_networks(f));
    }
    if sources.names || sources.land {
        nets.push(ChainNet::Ethereum);
    }
    nets.sort();
    nets.dedup();
    for net in nets {
        if ready_cursor(pool, INDEXER_NFT, net, now).await.is_none() {
            return Err(unavailable(
                Source::NftIndex,
                &format!(
                    "the {} nft index is not published yet (catching up, or MARKETPLACE_NFT_INDEX_ENABLED off)",
                    net.db_name()
                ),
            ));
        }
    }
    if sources.land && !land_ready(pool).await {
        return Err(unavailable(
            Source::LandAuthzIndex,
            "LAND and estate ownership reads the land-authz index, which is not published yet",
        ));
    }
    Ok(())
}

/// The collection candidates of one network (a v2 token by its item id, a
/// v1 token by its collection URN and wearable id).
fn collection_source(net: ChainNet) -> String {
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
                n.owner, i.name, COALESCE(i.image, '') AS image, n.minted_at AS created_at, n.transferred_at AS updated_at, n.sold_at, \
                n.issued_id, i.item_id AS item_blockchain_id, i.urn, i.item_type, i.rarity, i.category AS item_category, i.body_shapes, \
                i.description, i.is_smart, i.emote_loop, i.emote_has_sound, i.emote_has_geometry, i.emote_outcome_type, i.creator, \
                c.name AS collection_name, NULL::int AS x, NULL::int AS y, NULL::text AS parcel_estate_id, NULL::text AS parcel_estate_name, NULL::int AS estate_size, \
                NULL::int AS distance_to_plaza, NULL::boolean AS adjacent_to_road \
         FROM shop.nft n {item_join} \
         WHERE n.network = '{net}' AND n.owner IS NOT NULL AND n.contract_address <> '{DCL_REGISTRAR}'",
        net = net.db_name()
    )
}

fn names_source() -> String {
    format!(
        "SELECT n.network, n.contract_address, n.token_id, 'ens' AS category, n.owner, n.ens_subdomain AS name, '' AS image, \
                n.minted_at AS created_at, n.transferred_at AS updated_at, n.sold_at, NULL::numeric AS issued_id, NULL::numeric AS item_blockchain_id, \
                NULL::text AS urn, NULL::text AS item_type, NULL::text AS rarity, NULL::text AS item_category, NULL::text[] AS body_shapes, \
                NULL::text AS description, NULL::boolean AS is_smart, NULL::boolean AS emote_loop, NULL::boolean AS emote_has_sound, \
                NULL::boolean AS emote_has_geometry, NULL::text AS emote_outcome_type, NULL::text AS creator, NULL::text AS collection_name, \
                NULL::int AS x, NULL::int AS y, NULL::text AS parcel_estate_id, NULL::text AS parcel_estate_name, NULL::int AS estate_size, \
                NULL::int AS distance_to_plaza, NULL::boolean AS adjacent_to_road \
         FROM shop.nft n WHERE n.network = 'ethereum' AND n.contract_address = '{DCL_REGISTRAR}' AND n.owner IS NOT NULL"
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
/// none has one, and whether any touches a road), else NULL.
fn land_source(geography: bool) -> String {
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
                land = LAND_REGISTRY_MAINNET
            ),
            format!(
                "LEFT JOIN shop.land_proximity lp ON tr.token_address = '{LAND_REGISTRY_MAINNET}' AND lp.x = tr.x AND lp.y = tr.y"
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
                COALESCE(minted.block_time, 0) AS created_at, COALESCE(touched.block_time, 0) AS updated_at, sold.sold_at, \
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
         LEFT JOIN LATERAL (SELECT max(s.block_time) AS sold_at FROM shop.nft_event s WHERE s.network = 'ethereum' AND s.contract_address = tr.token_address \
                            AND s.token_id = tr.token_id AND s.kind = 'order_successful') sold ON true \
         WHERE tr.token_address IN ('{land}', '{estate}') AND tr.owner IS NOT NULL",
        land = LAND_REGISTRY_MAINNET,
        estate = ESTATE_REGISTRY_MAINNET,
    )
}

/// Whether the filter set reads the order legs (a filter or sort on them).
fn needs_orders(f: &NftFilters) -> bool {
    f.is_on_sale.is_some()
        || f.min_price.is_some()
        || f.max_price.is_some()
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
        wheres.push(format!("c.owner = {p}"));
    }
    if let Some(cat) = f.category {
        let p = b.bind_string(cat.as_str().to_string());
        wheres.push(format!("c.category = {p}"));
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
        wheres.push("c.emote_outcome_type IS NOT NULL".into());
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
    require_ready(&state.pool, f, sources).await?;
    if needs_orders(f) {
        super::trades::require_ready(state).await?;
    }
    Ok(())
}

/// The candidate set with its order legs as `c` (a CTE chain), for the
/// page, the total and the histograms. `now_ms` is the bound clock, `since`
/// the trades floor (`trades::mirrored_since`).
pub fn build_candidates(
    b: &mut Builder,
    f: &NftFilters,
    sources: Sources,
    now_ms: i64,
    since: Option<chrono::NaiveDateTime>,
) -> String {
    let mut parts: Vec<String> = Vec::new();
    if sources.collections {
        parts.extend(collection_networks(f).into_iter().map(collection_source));
    }
    if sources.names {
        parts.push(names_source());
    }
    if sources.land {
        parts.push(land_source(reads_geography(f)));
    }
    let base = parts.join(" UNION ALL ");
    let with_orders = needs_orders(f);
    let now_ms_p = b.bind_i64(now_ms);
    let orders = if with_orders {
        let now_p = b.bind_time(
            chrono::DateTime::from_timestamp_millis(now_ms)
                .unwrap_or_default()
                .naive_utc(),
        );
        let since_p = b.bind_opt_time(since);
        format!(
            ", {trades}, \
             listed AS ( \
               SELECT b.*, lo.order_id, lo.marketplace_address AS order_marketplace, lo.price AS legacy_price, lo.expires_at AS legacy_expires_at, \
                      lo.created_at AS legacy_created_at, lo.updated_at AS legacy_updated_at, v3.amount_received AS v3_price, v3.created_at AS v3_created_at, \
                      COALESCE(lo.price, v3.amount_received) AS order_price, \
                      COALESCE(lo.created_at * 1000, (EXTRACT(EPOCH FROM v3.created_at) * 1000)::bigint) AS order_created_at \
               FROM base b \
               LEFT JOIN LATERAL (SELECT o.* FROM shop.nft_order o WHERE o.network = b.network AND o.contract_address = b.contract_address \
                                  AND o.token_id = b.token_id AND o.status = 'open' AND o.expires_at > {now_ms_p} AND o.seller = b.owner) lo ON true \
               LEFT JOIN LATERAL (SELECT u.amount_received, u.created_at FROM unified_trades u WHERE u.type = 'public_nft_order' \
                                  AND lower(u.network) = b.network AND u.contract_address_sent = b.contract_address AND u.token_id = b.token_id::text \
                                  AND u.signer = b.owner ORDER BY u.created_at DESC LIMIT 1) v3 ON true \
             )",
            trades = unified_trades_cte(&now_p, &since_p)
        )
    } else {
        ", listed AS ( \
             SELECT b.*, lo.order_id, lo.marketplace_address AS order_marketplace, lo.price AS legacy_price, lo.expires_at AS legacy_expires_at, \
                    lo.created_at AS legacy_created_at, lo.updated_at AS legacy_updated_at, NULL::numeric AS v3_price, NULL::timestamp AS v3_created_at, \
                    NULL::numeric AS order_price, NULL::bigint AS order_created_at \
             FROM base b \
             LEFT JOIN LATERAL (SELECT o.* FROM shop.nft_order o WHERE o.network = b.network AND o.contract_address = b.contract_address \
                                AND o.token_id = b.token_id AND o.status = 'open' AND o.expires_at > NOW_MS AND o.seller = b.owner) lo ON true \
           )"
            .replace("NOW_MS", &now_ms_p)
    };
    format!("WITH base AS ({base}){orders}")
}

/// The page SQL and the capped total SQL (their own argument lists).
pub fn build_nfts_query(
    f: &NftFilters,
    sources: Sources,
    first: i64,
    skip: i64,
    now_ms: i64,
    since: Option<chrono::NaiveDateTime>,
) -> (
    (String, sqlx::postgres::PgArguments),
    (String, sqlx::postgres::PgArguments),
) {
    let page = {
        let mut b = Builder::new();
        let ctes = build_candidates(&mut b, f, sources, now_ms, since);
        let mut wheres = Vec::new();
        push_filters(&mut b, f, &mut wheres);
        let lim = b.bind_i64(first);
        let off = b.bind_i64(skip);
        let where_sql = if wheres.is_empty() {
            String::new()
        } else {
            format!("WHERE {}", wheres.join(" AND "))
        };
        b.push(&format!("{ctes} SELECT c.*, c.token_id::text AS token_text, c.issued_id::text AS issued_text, c.item_blockchain_id::text AS item_text, c.legacy_price::text AS legacy_price_text FROM listed c {where_sql} {} LIMIT {lim} OFFSET {off}", sort_sql(f.sort_by)));
        (b.sql, b.args)
    };
    let total = {
        let mut b = Builder::new();
        let ctes = build_candidates(&mut b, f, sources, now_ms, since);
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
) -> NftResult {
    let network = network_of(&r.get::<String, _>("network"));
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
        chain_id: network.chain_id(),
        issued_id: r.get("issued_text"),
        trade_id: Some(String::new()),
    });
    let nft = Nft {
        active_order_id: order.as_ref().map(|o| o.id.clone()),
        category: category.clone(),
        chain_id: network.chain_id(),
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
async fn estate_parcels(
    pool: &PgPool,
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
    .fetch_all(pool)
    .await?;
    for r in rows {
        out.entry(r.get("estate")).or_default().push(EstateParcel {
            x: r.get("x"),
            y: r.get("y"),
        });
    }
    Ok(out)
}

/// `/v1/nfts` from the indexes: the page over the `first + skip` window and
/// its total (exact up to [`MAX_WINDOW`]).
pub async fn list_nfts(
    state: &MarketplaceState,
    f: &NftFilters,
    first: i64,
    skip: i64,
) -> Result<(Vec<NftResult>, i64), Response> {
    if first + skip > MAX_WINDOW {
        return Err(
            ApiError::bad_request(format!("first + skip may not exceed {MAX_WINDOW}"))
                .into_response(),
        );
    }
    let sources = sources_for(f);
    require_sources(state, f, sources).await?;
    let now_ms = Utc::now().timestamp_millis();
    let ((page_sql, page_args), (total_sql, total_args)) =
        build_nfts_query(f, sources, first, skip, now_ms, trades_since(state));
    let rows = sqlx::query_with(sqlx::AssertSqlSafe(page_sql), page_args)
        .fetch_all(&state.pool)
        .await
        .map_err(|e| ApiError::internal(e.to_string()).into_response())?;
    let total: i64 = sqlx::query_scalar_with(sqlx::AssertSqlSafe(total_sql), total_args)
        .fetch_one(&state.pool)
        .await
        .map_err(|e| ApiError::internal(e.to_string()).into_response())?;
    let estates: Vec<String> = rows
        .iter()
        .filter(|r| r.get::<String, _>("category") == "estate")
        .map(|r| r.get::<String, _>("token_text"))
        .collect();
    let parcels = estate_parcels(&state.pool, &estates)
        .await
        .map_err(|e| ApiError::internal(e.to_string()).into_response())?;
    Ok((
        rows.iter()
            .map(|r| row_to_result(r, &parcels, &state.settings.land_image_base_url))
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
/// only after [`require_prices_sources`].
pub async fn nft_prices(
    state: &MarketplaceState,
    f: &NftFilters,
) -> Result<Histogram, sqlx::Error> {
    let f = on_sale(f);
    let sources = sources_for(&f);
    let mut b = Builder::new();
    let ctes = build_candidates(
        &mut b,
        &f,
        sources,
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
        .fetch_all(&state.pool)
        .await?;
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
    let mut b = Builder::new();
    let ctes = build_candidates(
        &mut b,
        &f,
        sources,
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
        .fetch_all(&state.pool)
        .await
        .map_err(|e| ApiError::internal(e.to_string()).into_response())?;
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

/// One nft's description from the indexes (collections, names, LAND);
/// `None` when no index row names it.
pub async fn asset_meta(
    pool: &PgPool,
    land_image_base: &str,
    network: &str,
    contract: &str,
    token_id: &str,
) -> Result<Option<AssetDescription>, sqlx::Error> {
    let net = ChainNet::parse(network).unwrap_or(ChainNet::Matic);
    let contract = contract.to_ascii_lowercase();
    let sources = if contract == LAND_REGISTRY_MAINNET || contract == ESTATE_REGISTRY_MAINNET {
        land_source(false)
    } else if contract == DCL_REGISTRAR {
        names_source()
    } else {
        collection_source(net)
    };
    let row = sqlx::query(sqlx::AssertSqlSafe(format!(
        "SELECT owner, image, category, rarity, name FROM ({sources}) c WHERE c.contract_address = $1 AND c.token_id = $2::numeric LIMIT 1"
    )))
    .bind(&contract)
    .bind(token_id)
    .fetch_optional(pool)
    .await?;
    Ok(row.map(|r| AssetDescription {
        owner: r.get::<Option<String>, _>("owner").unwrap_or_default(),
        image: land_image_url(
            land_image_base,
            &r.get::<String, _>("category"),
            r.get::<Option<String>, _>("image").unwrap_or_default(),
        ),
        category: r.get("category"),
        rarity: r.get("rarity"),
        name: r.get("name"),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::market::nfts::parse_filters;

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
        let ((page, _), (total, _)) =
            build_nfts_query(&f, sources_for(&f), 100, 6_000, 1_700_000_000_000, None);
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
        let ((page, _), _) = build_nfts_query(&on_sale, sources_for(&on_sale), 10, 0, 1, None);
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
        let ((page, _), _) = build_nfts_query(&f, sources_for(&f), 10, 0, 1, None);
        assert!(
            page.contains("shop.land_proximity")
                && page.contains("c.adjacent_to_road")
                && page.contains("c.distance_to_plaza >= $")
                && page.contains("c.distance_to_plaza <= $"),
            "{page}"
        );
        let plain = parse_filters(&pairs(&[("category", "parcel")])).unwrap();
        let ((page, _), _) = build_nfts_query(&plain, sources_for(&plain), 10, 0, 1, None);
        assert!(!page.contains("land_proximity"), "{page}");
    }

    /// The LAND source names no image host; the reader composes the
    /// configured `LAND_IMAGE_BASE_URL` in front of LAND rows only.
    #[test]
    fn land_images_carry_the_configured_base_only() {
        for geography in [false, true] {
            let sql = land_source(geography);
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
            land_source(true)
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
}
