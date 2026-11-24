// from: atlas-server@6dfd733fc0a225f5a46eb67f978dbe3825a171a9:src/modules/map/data/specialTiles.json (special_tiles.json, byte for byte), src/modules/api/component.ts (buildTile), src/modules/map/component.ts (applyTrades, expireOrders), src/modules/map/utils.ts (addSpecialTiles, getEstateFlags)
//! The tile grid built from this node's own database, the atlas's
//! `buildTile`, `addSpecialTiles`, `getEstateFlags` and `applyTrades` over
//! the indexes that replace its marketplace subgraph and squid reads:
//!
//! Every input is read in one REPEATABLE READ, READ ONLY snapshot, the one
//! the land-authz index opens once it finds itself published
//! (`indexer::ready_snapshot`), so the LAND facts, Shop's index
//! cursors and Shop's rows are one state of the database:
//!
//! - **LAND and estates** ([`read_land`]): every LAND token with its
//!   coordinates and owner (`token_right`), its estate (`estate_land`), the
//!   estate's owner and the name of its latest data document
//!   (`estate_data_event`). A LAND in an estate takes the estate's owner
//!   and name, as the atlas does.
//! - **Listings, names and rentals** ([`LandMarket`], given the snapshot's
//!   connection and the grid's chain): Shop's readers,
//!   handed in by `serve` (Shop sits above Explore): the legacy order of
//!   each LAND and Estate token (`shop.nft_order`), the open off-chain
//!   `public_nft_order` trades that sell one (`shop.trades` under Shop's
//!   open-trade rule, closed by the `shop.trade_chain_state` verdicts while
//!   the trade-events index is published), the LAND data names
//!   (`shop.land_data`) and this node's open rental listings
//!   (`signatures.*`). Shop decides whether its sources are ready.
//!
//! The rules ([`compose`]): a tile is every LAND token; its type is the
//! road or plaza of `special_tiles.json` at its coordinates, else `owned` with
//! an owner and `unowned` without one; a road or plaza no LAND row holds
//! is added as the atlas's `addSpecialTiles` does. An owned estate tile
//! joins its neighbours of the same estate (`top`, `left`, `topLeft`). The
//! price is the estate's live legacy order, else the parcel's (live: open,
//! unexpired, and its seller still owns the token, the rule Shop's readers
//! apply to `shop.nft_order`), in whole MANA rounded as the atlas rounds;
//! an open trade signed by the token's owner replaces it when the tile
//! has no price or the trade is newer than the price it holds. `updatedAt`
//! is the latest of the token's (and its estate's) last rights event, last
//! data document and last legacy order event, its rental listing's
//! update, or the trade that priced it.
//!
//! Known differences from the atlas: an order whose seller no longer owns
//! the token shows no price (the subgraph keeps a transferred order as the
//! token's `activeOrder`, and the atlas prices it until it expires); a
//! trade priced in USD-pegged MANA prices nothing (Shop reads MANA prices
//! only); a bid does not move `updatedAt`; rental listings are this node's
//! own (`signatures.*`), not another signatures server's, and are not
//! checked against the token's current owner (Shop's `rental-listing-status`
//! job cancels a listing whose asset left its lessor).

use std::collections::{BTreeMap, HashMap};
use std::sync::OnceLock;

use futures::future::BoxFuture;
use serde::Deserialize;
use sqlx::PgConnection;

use super::tiles::{RentalPeriod, Tile, TileRentalListing, TileType, coords_to_id};
use crate::chain_index::registries::{NETWORK_MAINNET, NETWORK_SEPOLIA};
use crate::chain_index::source::Source;

/// The legacy Marketplace order of a LAND or Estate token: `shop.nft_order`,
/// the token's latest order event folded, whatever its status.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LandOrder {
    pub contract_address: String,
    pub token_id: String,
    /// `status = 'open'`.
    pub open: bool,
    pub seller: String,
    /// Wei, decimal.
    pub price: String,
    pub expires_at_ms: i64,
    /// Unix seconds.
    pub created_at: i64,
    /// Unix seconds: the token's last legacy order event.
    pub updated_at: i64,
}

/// An open off-chain listing that sells a LAND or Estate token for MANA.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LandTrade {
    pub id: String,
    pub contract_address: String,
    pub token_id: String,
    pub signer: String,
    /// Wei, decimal.
    pub price: String,
    pub created_at_ms: i64,
}

/// An open rental listing of a LAND or Estate token.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LandRental {
    pub contract_address: String,
    pub token_id: String,
    pub expiration_ms: i64,
    pub updated_at_ms: i64,
    pub periods: Vec<(i64, i64, String)>,
}

/// A LAND or Estate token's latest data document (`shop.land_data`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LandData {
    pub contract_address: String,
    pub token_id: String,
    pub name: Option<String>,
    /// Unix seconds of the document's log, when known.
    pub updated_at: Option<i64>,
}

/// What Shop knows of the LAND and Estate tokens, read once per build.
#[derive(Debug, Clone, Default)]
pub struct LandMarketSnapshot {
    pub orders: Vec<LandOrder>,
    pub trades: Vec<LandTrade>,
    pub rentals: Vec<LandRental>,
    pub data: Vec<LandData>,
}

#[derive(Debug)]
pub enum LandMarketError {
    /// A source the grid needs is not published: no grid is built.
    Unready(Source, String),
    Failed(anyhow::Error),
}

/// Shop's LAND and Estate listings, names and rentals; `serve` implements it
/// over Shop's readers (`src/serve_adapters.rs`), as Shop sits above
/// Explore. Every read, readiness included, runs on `conn`, the build's
/// snapshot. `chain_id` is the chain the LAND facts are of; `land` and
/// `estate` its lowercase registry addresses; `now` the build's clock, which
/// decides expiry.
pub trait LandMarket: Send + Sync {
    fn snapshot<'a>(
        &'a self,
        conn: &'a mut PgConnection,
        chain_id: i64,
        land: &'a str,
        estate: &'a str,
        now: chrono::DateTime<chrono::Utc>,
    ) -> BoxFuture<'a, Result<LandMarketSnapshot, LandMarketError>>;
}

/// The chain of a land-authz network (`index_cursor.network`).
pub fn land_chain_id(network: &str) -> Option<i64> {
    match network {
        NETWORK_MAINNET => Some(1),
        NETWORK_SEPOLIA => Some(11_155_111),
        _ => None,
    }
}

/// A LAND token as the land-authz index holds it.
#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct LandRow {
    pub token_id: String,
    pub x: i32,
    pub y: i32,
    pub owner: Option<String>,
    pub estate_id: Option<String>,
    /// Unix seconds of its last rights event, when known.
    pub updated_at: Option<i64>,
}

/// An Estate token that holds LAND, as the land-authz index holds it.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct EstateRow {
    pub owner: Option<String>,
    /// Its latest data document's name.
    pub name: Option<String>,
    pub updated_at: Option<i64>,
}

/// An Estate row as [`ESTATES_SQL`] reads it.
#[derive(sqlx::FromRow)]
struct EstateRead {
    token_id: String,
    owner: Option<String>,
    updated_at: Option<i64>,
    name: Option<String>,
}

/// What [`read_land`] read.
pub struct LandSnapshot {
    pub lands: Vec<LandRow>,
    pub estates: HashMap<String, EstateRow>,
}

#[derive(Debug, Clone, Copy, Deserialize)]
struct SpecialTile {
    #[serde(rename = "type")]
    kind: TileType,
    top: bool,
    left: bool,
    #[serde(rename = "topLeft")]
    top_left: bool,
}

#[derive(Deserialize)]
struct SpecialTileEntry {
    id: String,
    #[serde(flatten)]
    tile: SpecialTile,
    #[serde(default)]
    name: Option<String>,
}

/// Coordinates -> the road or plaza there, with the atlas's name for it.
type SpecialTiles = HashMap<(i32, i32), (SpecialTile, Option<String>)>;

/// The atlas's roads and plazas by coordinates, with its name for one no
/// LAND row holds.
fn special_tiles() -> &'static SpecialTiles {
    static T: OnceLock<SpecialTiles> = OnceLock::new();
    T.get_or_init(|| {
        let entries: HashMap<String, SpecialTileEntry> =
            serde_json::from_str(include_str!("special_tiles.json"))
                .expect("special_tiles.json must parse");
        entries
            .into_values()
            .map(|e| {
                let (x, y) =
                    e.id.split_once(',')
                        .and_then(|(x, y)| Some((x.parse().ok()?, y.parse().ok()?)))
                        .expect("special tile ids are x,y");
                ((x, y), (e.tile, e.name))
            })
            .collect()
    })
}

/// Wei to whole MANA as the atlas rounds it (`Math.round(parseInt(wei) / 1e18)`).
fn whole_mana(wei: &str) -> Option<f64> {
    wei.trim()
        .parse::<f64>()
        .ok()
        .filter(|w| w.is_finite())
        .map(|w| (w / 1e18).round())
}

fn same_address(a: Option<&str>, b: &str) -> bool {
    a.is_some_and(|a| a.eq_ignore_ascii_case(b))
}

fn non_empty(s: Option<&String>) -> Option<String> {
    s.filter(|s| !s.is_empty()).cloned()
}

/// An order's expiry in seconds, rounded as the atlas rounds a
/// millisecond `expiresAt`; the nft index saturates a far expiry at
/// `i64::MAX`, which rounds without overflowing.
fn expires_secs(order: &LandOrder) -> i64 {
    let ms = order.expires_at_ms;
    ms.div_euclid(1000) + i64::from(ms.rem_euclid(1000) >= 500)
}

/// The order when it is live at `now_s`: open, unexpired (the atlas's
/// `expiresAt > now`, both in rounded seconds) and its seller still the
/// token's owner.
fn live<'a>(
    order: Option<&'a LandOrder>,
    owner: Option<&str>,
    now_s: i64,
) -> Option<&'a LandOrder> {
    order.filter(|o| o.open && same_address(owner, &o.seller) && expires_secs(o) > now_s)
}

/// The grid: every LAND row as a tile, the roads and plazas no row holds,
/// the estate joins, then the trades (see the module docs). Pure, so the
/// rules are tested without a database. `now_ms` decides expiry.
pub fn compose(
    land_registry: &str,
    estate_registry: &str,
    land: &LandSnapshot,
    market: &LandMarketSnapshot,
    now_ms: i64,
) -> HashMap<String, Tile> {
    let now_s = (now_ms + 500).div_euclid(1000);
    let key = |c: &str, t: &str| (c.to_ascii_lowercase(), t.to_string());
    let orders: HashMap<(String, String), &LandOrder> = market
        .orders
        .iter()
        .map(|o| (key(&o.contract_address, &o.token_id), o))
        .collect();
    let data: HashMap<(String, String), &LandData> = market
        .data
        .iter()
        .map(|d| (key(&d.contract_address, &d.token_id), d))
        .collect();
    // One open listing per asset; should two arrive, the newest wins.
    let mut rentals: HashMap<(String, String), &LandRental> = HashMap::new();
    for r in &market.rentals {
        let slot = rentals
            .entry(key(&r.contract_address, &r.token_id))
            .or_insert(r);
        if r.updated_at_ms > slot.updated_at_ms {
            *slot = r;
        }
    }
    let land_key = |token: &str| (land_registry.to_string(), token.to_string());
    let estate_key = |token: &str| (estate_registry.to_string(), token.to_string());

    let mut tiles: HashMap<String, Tile> = HashMap::with_capacity(land.lands.len());
    // Tile id -> when its legacy order was created (ms), which a trade must beat.
    let mut order_created: HashMap<String, i64> = HashMap::new();
    let specials = special_tiles();
    for row in &land.lands {
        let estate = row
            .estate_id
            .as_deref()
            .and_then(|id| land.estates.get(id).map(|e| (id, e)));
        let owner = estate
            .and_then(|(_, e)| e.owner.clone())
            .or_else(|| row.owner.clone());
        let parcel_data = data.get(&land_key(&row.token_id)).copied();
        let estate_data = estate.and_then(|(id, _)| data.get(&estate_key(id)).copied());
        let name = estate
            .and_then(|(_, e)| non_empty(e.name.as_ref()))
            .or_else(|| parcel_data.and_then(|d| non_empty(d.name.as_ref())));
        let parcel_order = orders.get(&land_key(&row.token_id)).copied();
        let estate_order = estate.and_then(|(id, _)| orders.get(&estate_key(id)).copied());
        let order = live(
            estate_order,
            estate.and_then(|(_, e)| e.owner.as_deref()),
            now_s,
        )
        .or_else(|| live(parcel_order, row.owner.as_deref(), now_s));
        let rental = match estate {
            Some((id, _)) => rentals.get(&estate_key(id)),
            None => rentals.get(&land_key(&row.token_id)),
        }
        .copied()
        .filter(|r| r.expiration_ms >= now_ms);
        let updated_secs = [
            row.updated_at,
            estate.and_then(|(_, e)| e.updated_at),
            parcel_data.and_then(|d| d.updated_at),
            estate_data.and_then(|d| d.updated_at),
            parcel_order.map(|o| o.updated_at),
            estate_order.map(|o| o.updated_at),
        ]
        .into_iter()
        .flatten()
        .max()
        .unwrap_or(0);
        let updated_at =
            (updated_secs.saturating_mul(1000)).max(rental.map_or(0, |r| r.updated_at_ms));
        let id = coords_to_id(row.x, row.y);
        let special = specials.get(&(row.x, row.y)).map(|(s, _)| *s);
        if let Some(o) = order {
            order_created.insert(id.clone(), o.created_at.saturating_mul(1000));
        }
        let tile = Tile {
            id: id.clone(),
            x: row.x,
            y: row.y,
            nft_id: Some(format!("parcel-{land_registry}-{}", row.token_id)),
            tile_type: match (special, &owner) {
                (Some(s), _) => s.kind,
                (None, Some(_)) => TileType::Owned,
                (None, None) => TileType::Unowned,
            },
            top: special.is_some_and(|s| s.top),
            left: special.is_some_and(|s| s.left),
            top_left: special.is_some_and(|s| s.top_left),
            updated_at,
            name,
            owner,
            estate_id: row.estate_id.clone(),
            token_id: Some(row.token_id.clone()),
            price: order.and_then(|o| whole_mana(&o.price)),
            expires_at: order.map(expires_secs),
            rental_listing: rental.map(|r| TileRentalListing {
                expiration: r.expiration_ms,
                periods: r
                    .periods
                    .iter()
                    .map(|(min_days, max_days, price)| RentalPeriod {
                        min_days: *min_days,
                        max_days: *max_days,
                        price_per_day: price.clone(),
                    })
                    .collect(),
                updated_at: r.updated_at_ms,
            }),
        };
        tiles.insert(id, tile);
    }

    // A road or plaza no LAND row holds (never on mainnet, where every one
    // is minted): the atlas's name for it, `updatedAt` 0 so an unchanged
    // grid builds the same body.
    for (&(x, y), (special, name)) in specials {
        let id = coords_to_id(x, y);
        tiles.entry(id.clone()).or_insert_with(|| Tile {
            id,
            x,
            y,
            nft_id: None,
            tile_type: special.kind,
            top: special.top,
            left: special.left,
            top_left: special.top_left,
            updated_at: 0,
            name: Some(name.clone().unwrap_or_else(|| {
                let kind = serde_json::to_value(special.kind)
                    .ok()
                    .and_then(|v| v.as_str().map(str::to_string))
                    .unwrap_or_default();
                let mut chars = kind.chars();
                chars
                    .next()
                    .map(|c| c.to_uppercase().chain(chars).collect())
                    .unwrap_or_default()
            })),
            owner: None,
            estate_id: None,
            token_id: None,
            price: None,
            expires_at: None,
            rental_listing: None,
        });
    }

    // The estate joins: an owned estate tile's neighbours above, to its
    // left and above-left carrying the same estate (`getEstateFlags`).
    let joins: Vec<(String, bool, bool, bool)> = tiles
        .values()
        .filter(|t| t.tile_type == TileType::Owned)
        .filter_map(|t| {
            let estate = t.estate_id.as_deref()?;
            let same = |x: i32, y: i32| {
                tiles
                    .get(&coords_to_id(x, y))
                    .is_some_and(|n| n.estate_id.as_deref() == Some(estate))
            };
            Some((
                t.id.clone(),
                same(t.x, t.y + 1),
                same(t.x - 1, t.y),
                same(t.x - 1, t.y + 1),
            ))
        })
        .collect();
    for (id, top, left, top_left) in joins {
        if let Some(t) = tiles.get_mut(&id) {
            (t.top, t.left, t.top_left) = (top, left, top_left);
        }
    }

    apply_trades(
        land_registry,
        estate_registry,
        land,
        &market.trades,
        &mut tiles,
        &order_created,
    );
    tiles
}

/// The atlas's `applyTrades`: a trade selling an estate prices every tile
/// of the estate, one selling a LAND outside any estate prices its tile;
/// the signer must own the token. It replaces a tile's price when the tile
/// has none or the trade is newer than what priced it (the legacy order's
/// creation, else the tile's `updatedAt`), drops the order's expiry and
/// stamps the tile with the trade's creation. Oldest first, so the newest
/// open trade of a token decides.
fn apply_trades(
    land_registry: &str,
    estate_registry: &str,
    land: &LandSnapshot,
    trades: &[LandTrade],
    tiles: &mut HashMap<String, Tile>,
    order_created: &HashMap<String, i64>,
) {
    let mut estate_tiles: HashMap<&str, Vec<String>> = HashMap::new();
    let mut parcel_tiles: HashMap<&str, (String, Option<&str>)> = HashMap::new();
    for row in &land.lands {
        let id = coords_to_id(row.x, row.y);
        match row.estate_id.as_deref() {
            Some(estate) => estate_tiles.entry(estate).or_default().push(id),
            None => {
                parcel_tiles.insert(row.token_id.as_str(), (id, row.owner.as_deref()));
            }
        }
    }
    let mut sorted: Vec<&LandTrade> = trades.iter().collect();
    sorted.sort_by(|a, b| (a.created_at_ms, a.id.as_str()).cmp(&(b.created_at_ms, b.id.as_str())));
    let mut priced_at: HashMap<String, i64> = HashMap::new();
    for trade in sorted {
        let contract = trade.contract_address.to_ascii_lowercase();
        let ids: Vec<String> = if contract == estate_registry {
            let owner = land
                .estates
                .get(&trade.token_id)
                .and_then(|e| e.owner.as_deref());
            if !same_address(owner, &trade.signer) {
                continue;
            }
            estate_tiles
                .get(trade.token_id.as_str())
                .cloned()
                .unwrap_or_default()
        } else if contract == land_registry {
            match parcel_tiles.get(trade.token_id.as_str()) {
                Some((id, owner)) if same_address(*owner, &trade.signer) => vec![id.clone()],
                _ => continue,
            }
        } else {
            continue;
        };
        let price = whole_mana(&trade.price);
        for id in ids {
            let Some(tile) = tiles.get_mut(&id) else {
                continue;
            };
            let current = priced_at
                .get(&id)
                .or_else(|| order_created.get(&id))
                .copied()
                .unwrap_or(tile.updated_at);
            if tile.price.is_none_or(|p| p == 0.0) || trade.created_at_ms > current {
                tile.price = price;
                tile.expires_at = None;
                tile.updated_at = trade.created_at_ms;
                priced_at.insert(id, trade.created_at_ms);
            }
        }
    }
}

/// The grid's JSON body, `{ok, data}` with the tiles in key order, so an
/// unchanged grid serializes to the same bytes.
pub fn canonical_body(tiles: &HashMap<String, Tile>) -> Vec<u8> {
    let sorted: BTreeMap<&String, &Tile> = tiles.iter().collect();
    serde_json::to_vec(&serde_json::json!({ "ok": true, "data": sorted })).expect("tiles serialize")
}

const LANDS_SQL: &str = "\
SELECT p.token_id::text AS token_id, p.x, p.y, p.owner, el.estate_token_id::text AS estate_id,
       NULLIF(pe.block_time, 0) AS updated_at
FROM land_authz.token_right p
LEFT JOIN land_authz.estate_land el ON el.land_token_id = p.token_id
LEFT JOIN land_authz.authz_event pe ON pe.block_number = p.updated_block AND pe.log_index = p.updated_log
WHERE p.token_address = $1 AND p.x IS NOT NULL AND p.y IS NOT NULL";

const ESTATES_SQL: &str = "\
SELECT e.token_id::text AS token_id, e.owner, NULLIF(ee.block_time, 0) AS updated_at, d.name
FROM land_authz.token_right e
LEFT JOIN land_authz.authz_event ee ON ee.block_number = e.updated_block AND ee.log_index = e.updated_log
LEFT JOIN LATERAL (
    SELECT dd.name FROM land_authz.estate_data_event dd
    WHERE dd.token_address = e.token_address AND dd.estate_id = e.token_id
    ORDER BY dd.block_number DESC, dd.log_index DESC LIMIT 1
) d ON true
WHERE e.token_address = $1
  AND EXISTS (SELECT 1 FROM land_authz.estate_land el WHERE el.estate_token_id = e.token_id)";

/// Every LAND token of `land_registry` and every estate of
/// `estate_registry` holding one, on `conn`: the published snapshot of the
/// land-authz index (`indexer::ready_snapshot`).
pub async fn read_land(
    conn: &mut PgConnection,
    land_registry: &str,
    estate_registry: &str,
) -> anyhow::Result<LandSnapshot> {
    use anyhow::Context as _;
    let lands: Vec<LandRow> = sqlx::query_as(LANDS_SQL)
        .bind(land_registry)
        .fetch_all(&mut *conn)
        .await
        .context("reading the LAND tokens")?;
    let estates: Vec<EstateRead> = sqlx::query_as(ESTATES_SQL)
        .bind(estate_registry)
        .fetch_all(&mut *conn)
        .await
        .context("reading the estates")?;
    Ok(LandSnapshot {
        lands,
        estates: estates
            .into_iter()
            .map(|e| {
                (
                    e.token_id,
                    EstateRow {
                        owner: e.owner,
                        name: e.name,
                        updated_at: e.updated_at,
                    },
                )
            })
            .collect(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const LAND: &str = "0xf87e31492faf9a91b02ee0deaad50d51d56d5d4d";
    const ESTATE: &str = "0x959e104e1a4db6317fa58f8295f586e1a978c297";
    const NOW_MS: i64 = 1_800_000_000_000;

    fn row(token: &str, x: i32, y: i32, owner: Option<&str>, estate: Option<&str>) -> LandRow {
        LandRow {
            token_id: token.into(),
            x,
            y,
            owner: owner.map(str::to_string),
            estate_id: estate.map(str::to_string),
            updated_at: Some(1_700_000_000),
        }
    }

    fn order(contract: &str, token: &str, seller: &str, mana: u64, expires_ms: i64) -> LandOrder {
        LandOrder {
            contract_address: contract.into(),
            token_id: token.into(),
            open: true,
            seller: seller.into(),
            price: format!("{mana}000000000000000000"),
            expires_at_ms: expires_ms,
            created_at: 1_700_000_100,
            updated_at: 1_700_000_100,
        }
    }

    fn trade(
        id: &str,
        contract: &str,
        token: &str,
        signer: &str,
        mana: u64,
        at_ms: i64,
    ) -> LandTrade {
        LandTrade {
            id: id.into(),
            contract_address: contract.into(),
            token_id: token.into(),
            signer: signer.into(),
            price: format!("{mana}000000000000000000"),
            created_at_ms: at_ms,
        }
    }

    /// Two parcels of estate 7 side by side at (10,13)-(11,13) owned
    /// through it, a lone parcel at (10,14) and an unowned one at (12,13);
    /// all away from every road and plaza.
    fn land() -> LandSnapshot {
        LandSnapshot {
            lands: vec![
                row("101", 10, 13, Some(ESTATE), Some("7")),
                row("102", 11, 13, Some(ESTATE), Some("7")),
                row("103", 10, 14, Some("0xaaa"), None),
                row("104", 12, 13, None, None),
            ],
            estates: [(
                "7".to_string(),
                EstateRow {
                    owner: Some("0xbbb".into()),
                    name: Some("Seven".into()),
                    updated_at: Some(1_700_000_050),
                },
            )]
            .into(),
        }
    }

    fn special_count() -> usize {
        special_tiles()
            .keys()
            .filter(|c| ![(10, 13), (11, 13), (10, 14), (12, 13)].contains(c))
            .count()
    }

    #[test]
    fn the_special_tiles_are_the_atlas_roads_and_plazas() {
        let t = special_tiles();
        assert_eq!(t.len(), 13_026);
        let (genesis, _) = t.get(&(0, 0)).unwrap();
        assert_eq!(genesis.kind, TileType::Plaza);
        assert!(
            t.values()
                .all(|(s, _)| matches!(s.kind, TileType::Road | TileType::Plaza))
        );
        assert!(!t.contains_key(&(10, 13)), "the fixtures sit off the roads");
    }

    #[test]
    fn a_tile_takes_its_estate_owner_name_and_joins() {
        let tiles = compose(
            LAND,
            ESTATE,
            &land(),
            &LandMarketSnapshot::default(),
            NOW_MS,
        );
        assert_eq!(tiles.len(), 4 + special_count());
        let a = &tiles["10,13"];
        assert_eq!(a.tile_type, TileType::Owned);
        assert_eq!(a.owner.as_deref(), Some("0xbbb"), "the estate's owner");
        assert_eq!(a.name.as_deref(), Some("Seven"));
        assert_eq!(a.estate_id.as_deref(), Some("7"));
        assert_eq!(a.token_id.as_deref(), Some("101"));
        assert_eq!(
            a.nft_id.as_deref(),
            Some(format!("parcel-{LAND}-101").as_str())
        );
        assert_eq!((a.top, a.left, a.top_left), (false, false, false));
        let b = &tiles["11,13"];
        assert_eq!(
            (b.top, b.left, b.top_left),
            (false, true, false),
            "joins its left neighbour"
        );
        assert_eq!(b.updated_at, 1_700_000_050_000, "the estate's later event");
        let lone = &tiles["10,14"];
        assert_eq!(lone.owner.as_deref(), Some("0xaaa"));
        assert_eq!(lone.name, None);
        assert_eq!((lone.top, lone.left, lone.top_left), (false, false, false));
        let unowned = &tiles["12,13"];
        assert_eq!(unowned.tile_type, TileType::Unowned);
        assert_eq!(unowned.owner, None);
    }

    #[test]
    fn a_parcel_name_fills_a_tile_whose_estate_has_none() {
        let mut land = land();
        land.estates.get_mut("7").unwrap().name = Some(String::new());
        let market = LandMarketSnapshot {
            data: vec![
                LandData {
                    contract_address: LAND.into(),
                    token_id: "101".into(),
                    name: Some("Corner".into()),
                    updated_at: Some(1_700_000_900),
                },
                LandData {
                    contract_address: LAND.into(),
                    token_id: "103".into(),
                    name: Some("Lone".into()),
                    updated_at: None,
                },
            ],
            ..Default::default()
        };
        let tiles = compose(LAND, ESTATE, &land, &market, NOW_MS);
        assert_eq!(tiles["10,13"].name.as_deref(), Some("Corner"));
        assert_eq!(tiles["10,13"].updated_at, 1_700_000_900_000);
        assert_eq!(tiles["11,13"].name, None);
        assert_eq!(tiles["10,14"].name.as_deref(), Some("Lone"));
    }

    #[test]
    fn a_live_order_prices_the_tile_and_a_dead_one_does_not() {
        let later = NOW_MS + 86_400_000;
        let market = LandMarketSnapshot {
            orders: vec![
                order(ESTATE, "7", "0xbbb", 1500, later),
                order(LAND, "103", "0xaaa", 2_000_000, later),
            ],
            ..Default::default()
        };
        let tiles = compose(LAND, ESTATE, &land(), &market, NOW_MS);
        for id in ["10,13", "11,13"] {
            assert_eq!(tiles[id].price, Some(1500.0), "{id}: the estate's order");
            assert_eq!(tiles[id].expires_at, Some(later / 1000));
        }
        assert_eq!(tiles["10,14"].price, Some(2_000_000.0));

        for (dead, why) in [
            (order(LAND, "103", "0xaaa", 9, NOW_MS - 1000), "expired"),
            (
                order(LAND, "103", "0xccc", 9, later),
                "its seller no longer owns it",
            ),
            (
                LandOrder {
                    open: false,
                    ..order(LAND, "103", "0xaaa", 9, later)
                },
                "sold or cancelled",
            ),
        ] {
            let market = LandMarketSnapshot {
                orders: vec![dead],
                ..Default::default()
            };
            let tiles = compose(LAND, ESTATE, &land(), &market, NOW_MS);
            assert_eq!(tiles["10,14"].price, None, "{why}");
            assert_eq!(tiles["10,14"].expires_at, None, "{why}");
            assert_eq!(
                tiles["10,14"].updated_at, 1_700_000_100_000,
                "{why}: an order event still moves updatedAt"
            );
        }
        let market = LandMarketSnapshot {
            orders: vec![order(LAND, "101", "0xbbb", 5, later)],
            ..Default::default()
        };
        let tiles = compose(LAND, ESTATE, &land(), &market, NOW_MS);
        assert_eq!(
            tiles["10,13"].price, None,
            "the order its seller left behind when moving the parcel into the estate"
        );
    }

    #[test]
    fn a_saturated_expiry_rounds_without_overflow() {
        let far = LandOrder {
            expires_at_ms: i64::MAX,
            ..order(LAND, "1", "0xaaa", 1, 0)
        };
        assert_eq!(expires_secs(&far), i64::MAX / 1000 + 1);
        let at = |ms| {
            expires_secs(&LandOrder {
                expires_at_ms: ms,
                ..order(LAND, "1", "0xaaa", 1, 0)
            })
        };
        assert_eq!((at(1_499), at(1_500), at(0)), (1, 2, 0));
        let market = LandMarketSnapshot {
            orders: vec![LandOrder {
                expires_at_ms: i64::MAX,
                ..order(LAND, "103", "0xaaa", 7, 0)
            }],
            ..Default::default()
        };
        let tiles = compose(LAND, ESTATE, &land(), &market, NOW_MS);
        assert_eq!(
            tiles["10,14"].price,
            Some(7.0),
            "a never-expiring order is live"
        );
        assert_eq!(tiles["10,14"].expires_at, Some(i64::MAX / 1000 + 1));
    }

    #[test]
    fn whole_mana_rounds_as_the_atlas_does() {
        assert_eq!(whole_mana("1499999999999999999999"), Some(1500.0));
        assert_eq!(whole_mana("400000000000000000"), Some(0.0));
        assert_eq!(whole_mana("500000000000000000"), Some(1.0));
        assert_eq!(whole_mana("not a number"), None);
    }

    #[test]
    fn a_newer_trade_by_the_owner_replaces_the_price_and_drops_the_expiry() {
        let later = NOW_MS + 86_400_000;
        let order_ms = 1_700_000_100_000;
        let market = LandMarketSnapshot {
            orders: vec![order(ESTATE, "7", "0xbbb", 1500, later)],
            trades: vec![
                trade("t-old", ESTATE, "7", "0xbbb", 900, order_ms - 1),
                trade("t-new", ESTATE, "7", "0xBBB", 1200, order_ms + 5),
                trade("t-stranger", LAND, "103", "0xccc", 1, order_ms + 9),
                trade("t-lone", LAND, "103", "0xaaa", 42, 1_600_000_000_000),
                trade("t-in-estate", LAND, "101", ESTATE, 1, order_ms + 9),
            ],
            ..Default::default()
        };
        let tiles = compose(LAND, ESTATE, &land(), &market, NOW_MS);
        for id in ["10,13", "11,13"] {
            assert_eq!(tiles[id].price, Some(1200.0), "{id}");
            assert_eq!(tiles[id].expires_at, None, "{id}");
            assert_eq!(tiles[id].updated_at, order_ms + 5, "{id}");
        }
        let lone = &tiles["10,14"];
        assert_eq!(
            lone.price,
            Some(42.0),
            "an unpriced tile takes any open trade of its owner"
        );
        assert_eq!(
            lone.updated_at, 1_600_000_000_000,
            "stamped with the trade, as the atlas does"
        );
    }

    #[test]
    fn a_rental_listing_follows_the_estate_and_expires() {
        let rental = |contract: &str, token: &str, expiration_ms: i64| LandRental {
            contract_address: contract.into(),
            token_id: token.into(),
            expiration_ms,
            updated_at_ms: 1_750_000_000_000,
            periods: vec![(1, 7, "500".into()), (30, 30, "10".into())],
        };
        let market = LandMarketSnapshot {
            rentals: vec![
                rental(ESTATE, "7", NOW_MS + 1),
                rental(LAND, "101", NOW_MS + 1),
                rental(LAND, "103", NOW_MS - 1),
            ],
            ..Default::default()
        };
        let tiles = compose(LAND, ESTATE, &land(), &market, NOW_MS);
        let listing = tiles["10,13"].rental_listing.as_ref().unwrap();
        assert_eq!(listing.expiration, NOW_MS + 1);
        assert_eq!(listing.periods.len(), 2);
        assert_eq!(listing.max_price_per_day(), "500");
        assert_eq!(tiles["10,13"].updated_at, 1_750_000_000_000);
        assert!(
            tiles["11,13"].rental_listing.is_some(),
            "every tile of the estate"
        );
        assert!(tiles["10,14"].rental_listing.is_none(), "expired");
    }

    #[test]
    fn a_road_takes_the_special_type_and_flags_and_an_unheld_one_is_added() {
        let (x, y) = (0, 10);
        let (road, _) = special_tiles()[&(x, y)];
        let land = LandSnapshot {
            lands: vec![row("10", x, y, Some("0x9a6e"), Some("1813"))],
            estates: [(
                "1813".to_string(),
                EstateRow {
                    owner: Some("0x9a6e".into()),
                    name: Some("Roads".into()),
                    updated_at: None,
                },
            )]
            .into(),
        };
        let tiles = compose(LAND, ESTATE, &land, &LandMarketSnapshot::default(), NOW_MS);
        let t = &tiles["0,10"];
        assert_eq!(t.tile_type, TileType::Road);
        assert_eq!(
            (t.top, t.left, t.top_left),
            (road.top, road.left, road.top_left)
        );
        assert_eq!(t.name.as_deref(), Some("Roads"));
        let added = &tiles["1,10"];
        assert_eq!(added.nft_id, None, "no LAND row holds it");
        assert_eq!(added.owner, None);
        assert_eq!(added.updated_at, 0);
        assert!(added.name.as_deref().is_some_and(|n| !n.is_empty()));
    }

    #[test]
    fn an_unchanged_grid_builds_the_same_body() {
        let market = LandMarketSnapshot {
            orders: vec![order(ESTATE, "7", "0xbbb", 1500, NOW_MS + 1000)],
            ..Default::default()
        };
        let a = canonical_body(&compose(LAND, ESTATE, &land(), &market, NOW_MS));
        let b = canonical_body(&compose(LAND, ESTATE, &land(), &market, NOW_MS));
        assert_eq!(a, b);
        let expired = canonical_body(&compose(LAND, ESTATE, &land(), &market, NOW_MS + 2000));
        assert_ne!(a, expired, "an expiry changes the body");
    }
}
