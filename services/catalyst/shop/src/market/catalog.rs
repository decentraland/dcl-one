//! `GET /v1/catalog` and `GET /v2/catalog` over the admitted item catalogue.
//! Resale pages read the published legacy order book and NFT ownership
//! on the same snapshot as the item rows. V2 also reads backed off-chain
//! listings and mint offers at that snapshot's clock. Mint-only pages skip
//! the legacy book. A node without a local book refuses a resale page
//! with the `nft-index` readiness envelope. Owners and the suggested sort
//! retain their existing catalog behavior.

use std::collections::HashMap;
use std::sync::{Arc, LazyLock};
use std::time::Duration;

use axum::Json;
use axum::extract::{Query, State};
use axum::http::HeaderMap;
use axum::response::{IntoResponse, Response};
use serde::Serialize;
use serde::ser::{SerializeSeq, SerializeStruct, Serializer};
use sqlx::Arguments;
use sqlx::Row;
use sqlx::postgres::PgArguments;

use crate::shared::cache::TtlMap;

use super::catalogue::{self, CatalogueRead, Purpose};
use super::chain_walk::ChainNet;
use super::lists::{PickStats, picks_stats};
use super::mirrors::Mirror;
use super::nft_index::Catalogue;
use super::signed::optional_signer;
use super::wire::{
    ApiError, ChainId, MAX_PAGE_LIMIT, NETWORK_VALUES, NFT_CATEGORY_VALUES, Network, NftCategory,
    Params, clamp_first, clamp_skip,
};
use super::{MarketplaceState, SharedState};

pub const FRAGMENT_WEARABLE_V1: &str = "wearable_v1";
pub const FRAGMENT_WEARABLE_V2: &str = "wearable_v2";
pub const FRAGMENT_SMART_WEARABLE_V1: &str = "smart_wearable_v1";
pub const FRAGMENT_EMOTE_V1: &str = "emote_v1";
pub const WEARABLE_ITEM_TYPES: [&str; 3] = [
    FRAGMENT_WEARABLE_V1,
    FRAGMENT_WEARABLE_V2,
    FRAGMENT_SMART_WEARABLE_V1,
];

/// uint256 max: what the squid wrote into `price` for "no price set", and
/// the `LEAST` sentinel of the old price CASEs. It never reaches the wire:
/// `min_max_price_cases` maps it to NULL, so an item with no open leg
/// carries no `minPrice`/`minListingPrice` (production emits none either).
pub const MAX_NUMERIC_NUMBER: &str =
    "115792089237316195423570985008687907853269984665640564039457584007913129639935";

/// 2^216: a collection token id is `item_id << 216 | issued_id`, the rule
/// that gives an nft-order leg its item without an nft table.
pub use crate::shared::collections::ITEM_SHIFT as ITEM_ID_SHIFT;

/// The off-chain marketplace contracts of every version, mainnets and
/// testnets: a collection listing one of them as a minter is what the squid
/// flagged `search_is_marketplace_v3_minter`.
pub static OFFCHAIN_MARKETPLACE_MINTERS: LazyLock<Vec<&'static str>> = LazyLock::new(|| {
    let mut minters: Vec<&str> = super::trade_events::OFFCHAIN_MARKETPLACE_CHAINS
        .iter()
        .flat_map(|&chain| super::trade_events::deployments(chain))
        .map(|&(address, _, _)| address)
        .collect();
    minters.sort_unstable();
    minters.dedup();
    minters
});

pub const HEAD_CATEGORIES: [&str; 5] = ["eyebrows", "eyes", "facial_hair", "hair", "mouth"];
pub const ACCESSORY_CATEGORIES: [&str; 7] = [
    "earring", "eyewear", "hat", "helmet", "mask", "tiara", "top_head",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CatalogSortBy {
    Newest,
    RecentlyListed,
    RecentlySold,
    Cheapest,
    MostExpensive,
    Suggested,
}

impl CatalogSortBy {
    pub fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "newest" => Self::Newest,
            "recently_listed" => Self::RecentlyListed,
            "recently_sold" => Self::RecentlySold,
            "cheapest" => Self::Cheapest,
            "most_expensive" => Self::MostExpensive,
            "suggested" => Self::Suggested,
            _ => return None,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CatalogSortDirection {
    Asc,
    Desc,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub struct CatalogFilters {
    pub first: Option<i64>,
    pub skip: Option<i64>,
    pub sort_by: Option<CatalogSortBy>,
    pub sort_direction: Option<CatalogSortDirection>,
    pub only_listing: bool,
    pub only_minting: bool,
    pub category: Option<NftCategory>,
    pub creator: Vec<String>,
    pub is_sold_out: bool,
    pub is_on_sale: Option<bool>,
    pub search: Option<String>,
    pub is_wearable_head: bool,
    pub is_wearable_accessory: bool,
    pub is_wearable_smart: bool,
    pub wearable_category: Option<String>,
    pub rarities: Vec<String>,
    pub wearable_genders: Vec<String>,
    pub emote_category: Option<String>,
    pub emote_genders: Vec<String>,
    pub emote_play_mode: Vec<String>,
    pub emote_has_geometry: bool,
    pub emote_has_sound: bool,
    pub emote_outcome_type: Option<String>,
    pub contract_addresses: Vec<String>,
    pub item_id: Option<String>,
    pub network: Option<Network>,
    pub max_price: Option<String>,
    pub min_price: Option<String>,
    pub urns: Vec<String>,
    pub ids: Vec<String>,
    pub include_social_emotes: Option<bool>,
}

const DEFAULT_PAGE_SIZE: i64 = 20;
pub const WEARABLE_CATEGORY_VALUES: &[&str] = &[
    "eyebrows",
    "eyes",
    "facial_hair",
    "hair",
    "body_shape",
    "mouth",
    "upper_body",
    "lower_body",
    "feet",
    "earring",
    "eyewear",
    "hat",
    "helmet",
    "mask",
    "tiara",
    "top_head",
    "skin",
    "hands_wear",
];
pub const EMOTE_CATEGORY_VALUES: &[&str] = &[
    "dance",
    "stunt",
    "greetings",
    "fun",
    "poses",
    "reactions",
    "horror",
    "miscellaneous",
];
pub const RARITY_VALUES: &[&str] = &[
    "unique",
    "mythic",
    "exotic",
    "legendary",
    "epic",
    "rare",
    "uncommon",
    "common",
];
pub const GENDER_VALUES: &[&str] = &["male", "female", "unisex"];
pub const EMOTE_PLAY_MODE_VALUES: &[&str] = &["simple", "loop"];
pub const EMOTE_OUTCOME_VALUES: &[&str] = &["so", "mo", "ro"];
const SORT_BY_VALUES: &[&str] = &[
    "newest",
    "recently_sold",
    "cheapest",
    "most_expensive",
    "recently_listed",
    "suggested",
];

pub fn parse_catalog_filters(pairs: &[(String, String)]) -> Result<CatalogFilters, ApiError> {
    let p = Params::new(pairs);
    let is_on_sale = if p.get_boolean("isOnSale") {
        Some(p.get_string("isOnSale", None).as_deref() == Some("true"))
    } else {
        None
    };
    Ok(CatalogFilters {
        first: p
            .get_number("first", Some(DEFAULT_PAGE_SIZE as f64))
            .map(|n| n as i64),
        skip: p.get_number("skip", Some(0.0)).map(|n| n as i64),
        sort_by: p
            .get_value("sortBy", SORT_BY_VALUES, Some("cheapest"))
            .and_then(|s| CatalogSortBy::parse(&s)),
        sort_direction: p
            .get_value("sortDirection", &["asc", "desc"], Some("asc"))
            .map(|s| {
                if s == "desc" {
                    CatalogSortDirection::Desc
                } else {
                    CatalogSortDirection::Asc
                }
            }),
        only_listing: p.get_boolean("onlyListing"),
        only_minting: p.get_boolean("onlyMinting"),
        category: p
            .get_value("category", NFT_CATEGORY_VALUES, None)
            .and_then(|s| NftCategory::parse(&s)),
        creator: p.get_list("creator", &[]),
        is_sold_out: p.get_boolean("isSoldOut"),
        is_on_sale,
        search: p.get_string("search", None),
        is_wearable_head: p.get_boolean("isWearableHead"),
        is_wearable_accessory: p.get_boolean("isWearableAccessory"),
        is_wearable_smart: p.get_boolean("isWearableSmart"),
        wearable_category: p.get_value("wearableCategory", WEARABLE_CATEGORY_VALUES, None),
        rarities: p.get_list("rarity", RARITY_VALUES),
        wearable_genders: p.get_list("wearableGender", GENDER_VALUES),
        emote_category: p.get_value("emoteCategory", EMOTE_CATEGORY_VALUES, None),
        emote_genders: p.get_list("emoteGender", GENDER_VALUES),
        emote_play_mode: p.get_list("emotePlayMode", EMOTE_PLAY_MODE_VALUES),
        emote_has_geometry: p.get_boolean("emoteHasGeometry"),
        emote_has_sound: p.get_boolean("emoteHasSound"),
        emote_outcome_type: p.get_value("emoteOutcomeType", EMOTE_OUTCOME_VALUES, None),
        contract_addresses: p.get_address_list("contractAddress", false),
        item_id: p.get_string("itemId", None),
        network: p
            .get_value("network", NETWORK_VALUES, None)
            .and_then(|s| Network::parse(&s)),
        max_price: super::items::parse_price_param(&p, "maxPrice")?,
        min_price: super::items::parse_price_param(&p, "minPrice")?,
        urns: p.get_list("urn", &[]),
        ids: p.get_list("id", &[]),
        include_social_emotes: Some(
            p.get_string("includeSocialEmotes", None).as_deref() != Some("false"),
        ),
    })
}

#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct WearableData {
    pub description: Option<String>,
    pub category: Option<String>,
    #[serde(rename = "bodyShapes")]
    pub body_shapes: Vec<String>,
    pub rarity: String,
    #[serde(rename = "isSmart")]
    pub is_smart: bool,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(untagged)]
pub enum ItemData {
    Wearable { wearable: WearableData },
    Emote { emote: serde_json::Value },
}

#[derive(Clone, Debug, Serialize, PartialEq)]
#[cfg_attr(
    feature = "ts",
    derive(ts_rs::TS),
    ts(export, export_to = "market/", rename_all = "camelCase")
)]
pub struct CatalogItem {
    pub id: String,
    pub beneficiary: Option<String>,
    #[serde(rename = "itemId")]
    pub item_id: String,
    pub name: String,
    pub thumbnail: String,
    pub url: String,
    pub urn: String,
    pub category: &'static str,
    #[serde(rename = "contractAddress")]
    pub contract_address: String,
    pub rarity: String,
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub available: i64,
    #[serde(rename = "isOnSale")]
    pub is_on_sale: bool,
    /// Present iff the price came from an open v3 item trade (marketplace-server #387).
    #[serde(rename = "tradeId", skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub trade_id: Option<String>,
    pub creator: String,
    #[cfg_attr(feature = "ts", ts(type = "Record<string, unknown>"))]
    pub data: ItemData,
    pub network: Network,
    #[serde(rename = "chainId")]
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub chain_id: ChainId,
    pub price: String,
    #[serde(rename = "createdAt")]
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub created_at: i64,
    #[serde(rename = "updatedAt")]
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub updated_at: i64,
    #[serde(rename = "reviewedAt")]
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub reviewed_at: i64,
    #[serde(rename = "firstListedAt")]
    #[cfg_attr(feature = "ts", ts(type = "number | null"))]
    pub first_listed_at: Option<i64>,
    #[serde(rename = "soldAt")]
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub sold_at: i64,
    #[serde(rename = "minPrice", skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub min_price: Option<String>,
    #[serde(rename = "maxListingPrice")]
    pub max_listing_price: Option<String>,
    #[serde(rename = "minListingPrice")]
    pub min_listing_price: Option<String>,
    #[cfg_attr(feature = "ts", ts(type = "number | null"))]
    pub listings: Option<i64>,
    #[cfg_attr(feature = "ts", ts(type = "number | null"))]
    pub owners: Option<i64>,
    pub picks: Option<PickStats>,
}

/// `{"data": [...], "total": N}` with the caller's picks folded in; only the
/// items carrying picks are cloned off the cached page.
pub struct CatalogPage {
    page: Arc<(Vec<CatalogItem>, i64)>,
    picks: Vec<Option<PickStats>>,
}

impl Serialize for CatalogPage {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        struct Data<'a>(&'a CatalogPage);
        impl Serialize for Data<'_> {
            fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
                let items = &self.0.page.0;
                let mut seq = serializer.serialize_seq(Some(items.len()))?;
                for (item, pick) in items.iter().zip(&self.0.picks) {
                    if pick.is_none() && item.picks.is_none() {
                        seq.serialize_element(item)?;
                    } else {
                        let mut owned = item.clone();
                        owned.picks = pick.clone();
                        seq.serialize_element(&owned)?;
                    }
                }
                seq.end()
            }
        }
        let mut s = serializer.serialize_struct("DataTotal", 2)?;
        s.serialize_field("data", &Data(self))?;
        s.serialize_field("total", &self.page.1)?;
        s.end()
    }
}

type Page = Arc<(Vec<CatalogItem>, i64)>;

/// Descriptive pages keyed by `(filters, the catalogue read's copy)`
/// ([`CatalogueRead::key`]) and anonymous pick stats keyed by the page's
/// ids, under `MARKETPLACE_CATALOG_CACHE_TTL_SECS` (0 disables).
pub struct Cache {
    enabled: bool,
    pages: TtlMap<(CatalogFilters, String), Page>,
    picks: TtlMap<Vec<String>, Arc<Vec<PickStats>>>,
}

const CACHE_ENTRIES: usize = 256;

impl Cache {
    pub fn new(ttl: Duration) -> Self {
        let ttl_or_min = if ttl.is_zero() {
            Duration::from_secs(1)
        } else {
            ttl
        };
        Self {
            enabled: !ttl.is_zero(),
            pages: TtlMap::bounded(ttl_or_min, CACHE_ENTRIES),
            picks: TtlMap::bounded(ttl_or_min, CACHE_ENTRIES),
        }
    }

    pub fn enabled(&self) -> bool {
        self.enabled
    }

    /// A local pick write happened: every cached page's counts are stale.
    pub fn invalidate(&self) {
        self.pages.clear();
        self.picks.clear();
    }

    /// A page read through the caller's catalogue read, which only the
    /// fetching request keeps
    /// ([`crate::shared::cache::TtlMap::get_or_fetch_holding`]).
    async fn page_or_fetch_holding<F>(
        &self,
        key: (CatalogFilters, String),
        held: &mut Option<CatalogueRead>,
        fetch: F,
    ) -> Result<Option<Page>, Response>
    where
        F: AsyncFnOnce(&mut CatalogueRead) -> Result<Page, Response>,
    {
        if !self.enabled {
            let read = held.as_mut().expect("a request holds its read");
            return fetch(read).await.map(Some);
        }
        self.pages.get_or_fetch_holding(key, held, fetch).await
    }

    async fn picks_or_fetch<F, Fut>(
        &self,
        ids: Vec<String>,
        fetch: F,
    ) -> Result<Arc<Vec<PickStats>>, ApiError>
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = Result<Arc<Vec<PickStats>>, ApiError>>,
    {
        if !self.enabled {
            return fetch().await;
        }
        self.picks.get_or_fetch(ids, fetch).await
    }
}

pub struct Builder {
    pub sql: String,
    pub args: PgArguments,
    next_index: usize,
}

impl Default for Builder {
    fn default() -> Self {
        Self::new()
    }
}

impl Builder {
    pub fn new() -> Self {
        Self {
            sql: String::new(),
            args: PgArguments::default(),
            next_index: 1,
        }
    }
    pub fn push(&mut self, s: &str) -> &mut Self {
        self.sql.push_str(s);
        self
    }
    pub fn bind_string(&mut self, v: String) -> String {
        self.args.add(v).expect("add string arg");
        self.placeholder()
    }
    pub fn bind_strings(&mut self, v: &[String]) -> String {
        self.args.add(v.to_vec()).expect("add string[] arg");
        self.placeholder()
    }
    pub fn bind_bool(&mut self, v: bool) -> String {
        self.args.add(v).expect("add bool arg");
        self.placeholder()
    }
    pub fn bind_i64(&mut self, v: i64) -> String {
        self.args.add(v).expect("add i64 arg");
        self.placeholder()
    }
    pub fn bind_time(&mut self, v: chrono::NaiveDateTime) -> String {
        self.args.add(v).expect("add timestamp arg");
        self.placeholder()
    }
    pub fn bind_opt_time(&mut self, v: Option<chrono::NaiveDateTime>) -> String {
        self.args.add(v).expect("add timestamp arg");
        self.placeholder()
    }
    fn placeholder(&mut self) -> String {
        let p = format!("${}", self.next_index);
        self.next_index += 1;
        p
    }
}

/// The chains this node's trade readers serve, installed once by the binary.
static SERVED: std::sync::OnceLock<(i64, i64)> = std::sync::OnceLock::new();

/// Installs the served chains (`MARKETPLACE_INDEX_CHAIN_ID_*`) for the
/// process; the first call wins. Without it the readers serve mainnet (1, 137).
pub fn install_served_chains(indexes: &super::chain_walk::IndexSettings) {
    use super::chain_walk::ChainNet;
    let _ = SERVED.set((
        indexes.chain_id(ChainNet::Ethereum),
        indexes.chain_id(ChainNet::Matic),
    ));
}

/// `(ethereum, matic)`.
pub fn served_chains() -> (i64, i64) {
    SERVED.get().copied().unwrap_or((1, 137))
}

/// `alias`'s trade is on the chain this node serves for its network.
pub fn served_chain_sql(alias: &str) -> String {
    let (eth, matic) = served_chains();
    format!(
        "{alias}.chain_id = CASE lower({alias}.network) WHEN 'ethereum' THEN {eth} ELSE {matic} END"
    )
}

/// `alias`'s trade is one this node serves: on its chain for the network, and
/// an item order only when the item's creator signed it.
pub fn served_trade_sql(alias: &str) -> String {
    format!(
        "{served} AND ({alias}.type <> 'public_item_order' OR EXISTS ( \
           SELECT 1 FROM shop.trade_assets sa \
           JOIN shop.trade_assets_item si ON si.asset_id = sa.id \
           JOIN shop.items ci ON ci.network = lower({alias}.network) \
             AND ci.contract_address = lower(sa.contract_address) AND ci.item_id = {item_num} \
           WHERE sa.trade_id = {alias}.id AND sa.direction = 'sent' AND lower(ci.creator) = lower({alias}.signer)))",
        served = served_chain_sql(alias),
        item_num = uint_or_null("si.item_id"),
    )
}

/// `expr` (a stored token or item id, text) as numeric, NULL when it is not
/// a canonical decimal (`listings::is_canonical_uint`). The CASE keeps one
/// malformed row from failing the whole read with a cast error, whatever
/// order the planner evaluates the predicates in.
pub fn uint_or_null(expr: &str) -> String {
    format!("(CASE WHEN {expr} ~ '^(0|[1-9][0-9]{{0,77}})$' THEN {expr}::numeric END)")
}

/// Ownership of the sent ERC-721 in the same snapshot as the trade verdict.
fn nft_trade_backed_sql(now_p: &str, token_num: &str, eth: i64, matic: i64) -> String {
    use super::chain_walk::{ChainNet, MAX_CLOCK_SKEW, READY_WINDOW};
    use crate::land_authz::{LAND_MAINNET, LAND_SEPOLIA};
    let land = if eth == 11_155_111 {
        LAND_SEPOLIA
    } else {
        LAND_MAINNET
    };
    let nft_ready = crate::chain_index::catalogue::published_sql_on("c", now_p, eth, matic);
    format!(
        "(sent.asset_type = 3 AND {token_num} IS NOT NULL
          AND NOT EXISTS (
              SELECT FROM shop.trade_assets other
              WHERE other.trade_id = t.id AND other.direction = 'sent' AND other.id <> sent.id
          )
          AND CASE WHEN lower(t.network) = 'ethereum' AND sent.contract_address IN
              ('{main_land}', '{main_estate}', '{test_land}', '{test_estate}')
          THEN EXISTS (
              SELECT FROM land_authz.index_cursor c
              JOIN land_authz.token_right n ON n.token_address = sent.contract_address
                   AND n.token_id = {token_num}
              WHERE c.network = '{land_network}'
                AND c.land_registry = '{land_registry}' AND c.estate_registry = '{estate_registry}'
                AND sent.contract_address IN (c.land_registry, c.estate_registry)
                AND c.full_replay_started AND c.folded_block IS NOT NULL
                AND c.folded_at BETWEEN {now_p} - interval '{window} seconds'
                                    AND {now_p} + interval '{skew} seconds'
                AND n.owner = lower(t.signer) AND n.updated_block <= c.folded_block
                AND NOT EXISTS (
                    SELECT FROM land_authz.authz_event e
                    WHERE e.token_address = n.token_address AND e.token_id = n.token_id
                      AND e.kind = 'transfer' AND e.block_number > c.folded_block
                )
          ) ELSE EXISTS (
              SELECT FROM shop.chain_cursor c
              JOIN shop.nft n ON n.network = c.network AND n.contract_address = sent.contract_address
                   AND n.token_id = {token_num}
              WHERE c.indexer = 'nft' AND c.network = lower(t.network) AND {nft_ready}
                AND n.owner = lower(t.signer) AND n.transferred_block <= c.folded_block
                AND (c.chain_id IN ({ethereum}, {polygon})
                  OR (c.network = 'ethereum' AND n.contract_address = '{registrar}')
                  OR EXISTS (
                      SELECT FROM shop.nft_contract watched
                      JOIN shop.collections active ON active.network = watched.network
                         AND active.contract_address = watched.contract_address
                      WHERE watched.network = n.network AND watched.contract_address = n.contract_address
                        AND active.chain_id = c.chain_id AND active.testnet_verified
                  )
                )
                AND NOT EXISTS (
                    SELECT FROM shop.nft_event e
                    WHERE e.network = n.network AND e.contract_address = n.contract_address
                      AND e.token_id = n.token_id AND e.kind = 'transfer'
                      AND e.block_number > c.folded_block
                )
                AND NOT EXISTS (
                    SELECT FROM shop.nft_contract watched
                    WHERE watched.network = n.network AND watched.contract_address = n.contract_address
                      AND (watched.refold_owed OR ({backfill_pending}))
                )
          ) END)",
        ethereum = ChainNet::Ethereum.chain_id(),
        polygon = ChainNet::Matic.chain_id(),
        registrar = super::nft_index::Catalogue::Testnet.registrar(),
        backfill_pending = super::nft_index::BACKFILL_PENDING_SQL,
        window = READY_WINDOW.num_seconds(),
        skew = MAX_CLOCK_SKEW.num_seconds(),
        main_land = LAND_MAINNET.land_registry,
        main_estate = LAND_MAINNET.estate_registry,
        test_land = LAND_SEPOLIA.land_registry,
        test_estate = LAND_SEPOLIA.estate_registry,
        land_network = land.network,
        land_registry = land.land_registry,
        estate_registry = land.estate_registry,
    )
}

/// `unified_trades`: the open off-chain listings of `shop.trades` as
/// one row per trade with its sent asset resolved to `(contract, item_id,
/// token_id)` and its received amount, asset type and token contract. Open = effective, not expired at
/// `now_p` (a bound timestamp, never `now()`), not in `trade_cancellations`,
/// and listable on chain (`trade_events::listable_on_chain_sql`: its
/// network's trade-events index is current and no verdict sold, cancelled
/// or left it unverifiable; a trade of a network whose index is behind is
/// left out, never listed on a stale verdict); and counted at
/// `since_p` (a bound nullable timestamp, `trades::mirrored_since`). An item
/// order also needs its creator's signature and its marketplace among the
/// collection's minters. An NFT order needs current ownership evidence
/// for its signer from the published NFT or LAND/Estate fold.
pub fn unified_trades_cte(now_p: &str, since_p: &str) -> String {
    unified_trades_cte_of(now_p, since_p, None)
}

/// [`unified_trades_cte`]; with `contracts_p` (a bound `text[]`), only the
/// trades that sell from those contracts, as a semi-join that stays hashed
/// whatever the array's length.
pub fn unified_trades_cte_of(now_p: &str, since_p: &str, contracts_p: Option<&str>) -> String {
    use super::chain_walk::ChainNet;
    let of = contracts_p
        .map(|p| format!(" AND sent.contract_address IN (SELECT unnest({p}::text[]))"))
        .unwrap_or_default();
    let (eth, matic) = served_chains();
    let token_num = uint_or_null("sent_erc721.token_id");
    let mainnet = format!(
        "t.chain_id IN ({}, {})",
        ChainNet::Ethereum.chain_id(),
        ChainNet::Matic.chain_id()
    );
    format!(
        "unified_trades AS ( \
           SELECT t.id, t.type::text AS type, t.signer, t.created_at, t.expires_at, t.network, t.chain_id, t.contract AS trade_contract, \
                  sent.contract_address AS contract_address_sent, \
                  COALESCE({item_num}, div({token_num}, {shift}::numeric)) AS item_id, \
                  sent_erc721.token_id AS token_id, \
                  recv.asset_type::int4 AS received_asset_type, \
                  recv.contract_address AS received_contract, \
                  recv_erc20.amount AS amount_received, \
                  recv.beneficiary AS beneficiary \
           FROM shop.trades t \
           JOIN shop.trade_assets sent ON sent.trade_id = t.id AND sent.direction = 'sent'{of} \
           LEFT JOIN shop.trade_assets_item sent_item ON sent_item.asset_id = sent.id \
           LEFT JOIN shop.trade_assets_erc721 sent_erc721 ON sent_erc721.asset_id = sent.id \
           JOIN shop.trade_assets recv ON recv.trade_id = t.id AND recv.direction = 'received' AND recv.asset_type IN (1, 2) \
           JOIN shop.trade_assets_erc20 recv_erc20 ON recv_erc20.asset_id = recv.id \
           WHERE t.type IN ('public_item_order', 'public_nft_order') \
             AND t.expires_at > {now_p} AND t.effective_since <= {now_p} \
             AND NOT EXISTS (SELECT 1 FROM shop.trade_cancellations c WHERE c.target_hashed_signature = t.hashed_signature) \
             AND {listable} \
             AND {counted} \
             AND {served} \
             AND (sent_item.asset_id IS NULL OR {item_num} IS NOT NULL) \
             AND (sent_erc721.asset_id IS NULL OR {token_num} IS NOT NULL) \
             AND (t.type <> 'public_nft_order' OR {backed}) \
             AND (t.type <> 'public_item_order' OR EXISTS ( \
                   SELECT 1 FROM shop.items ci \
                   WHERE ci.network = lower(t.network) AND ci.contract_address = lower(sent.contract_address) \
                     AND ci.chain_id = t.chain_id AND ({mainnet} OR ci.testnet_verified) \
                     AND ci.item_id = {item_num} AND lower(ci.creator) = lower(t.signer))) \
             AND (t.type <> 'public_item_order' OR EXISTS ( \
                   SELECT 1 FROM shop.collections mc \
                   WHERE mc.network = lower(t.network) AND mc.contract_address = lower(sent.contract_address) \
                     AND mc.chain_id = t.chain_id AND ({mainnet} OR mc.testnet_verified) \
                     AND lower(t.contract) = ANY(mc.minters))) \
         )",
        served = served_chain_sql("t"),
        item_num = uint_or_null("sent_item.item_id"),
        shift = ITEM_ID_SHIFT,
        listable = super::trade_events::listable_on_chain_sql("t", now_p),
        backed = nft_trade_backed_sql(now_p, &token_num, eth, matic),
        counted = super::trades::counted_sql("t.", since_p)
    )
}

/// The `items` alias' v3-minter flag: the collection lists an off-chain marketplace as a minter.
pub fn v3_minter_expr() -> String {
    let list: Vec<String> = OFFCHAIN_MARKETPLACE_MINTERS
        .iter()
        .map(|a| format!("'{a}'"))
        .collect();
    format!("(c.minters && ARRAY[{}]::text[])", list.join(", "))
}

fn build_offchain_orders_cte(b: &mut Builder, f: &CatalogFilters, books: Option<&CatalogBooks>) {
    // Ethereum v1 token IDs are sequential. The collection's Issue gives the
    // wearable name used by its item URN, even for a resale on a v3 venue.
    let (item, joins) = if let Some(books) = books.filter(|_| !f.only_minting) {
        let joins = format!(
            " LEFT JOIN shop.nft n ON lower(u.network) = 'ethereum' AND u.type = 'public_nft_order'
                     AND n.network = 'ethereum' AND n.contract_address = u.contract_address_sent
                     AND n.token_id = {token}
              LEFT JOIN shop.collections c ON c.network = n.network AND c.contract_address = n.contract_address
                     AND {collection_chain}
              LEFT JOIN shop.items i ON i.network = n.network AND i.contract_address = n.contract_address
                     AND i.urn = c.urn || ':' || n.wearable_id AND {item_chain}",
            token = uint_or_null("u.token_id"),
            collection_chain = catalogue_chain_sql("c", books.catalogue),
            item_chain = catalogue_chain_sql("i", books.catalogue),
        );
        (
            "CASE WHEN lower(u.network) = 'ethereum' AND u.type = 'public_nft_order' THEN i.item_id ELSE u.item_id END",
            joins,
        )
    } else {
        ("u.item_id", String::new())
    };
    b.push(&format!(
        ", offchain_orders AS (
           SELECT lower(u.network) AS network, u.chain_id, u.contract_address_sent, {item} AS item_id,
                  COUNT(*) AS count,
                  COUNT(*) FILTER (WHERE u.type = 'public_nft_order') AS nfts_listings_count,
                  COUNT(*) FILTER (WHERE u.type = 'public_item_order') AS items_listings_count,
                  MIN(u.amount_received) FILTER (WHERE u.type = 'public_nft_order') AS min_order_amount_received,
                  MAX(u.amount_received) FILTER (WHERE u.type = 'public_nft_order') AS max_order_amount_received,
                  MAX(u.created_at) AS max_created_at,
                  (array_agg(u.id::text ORDER BY u.amount_received, u.created_at DESC, u.id) FILTER (WHERE u.type = 'public_item_order'))[1] AS open_item_trade_id,
                  MIN(u.amount_received) FILTER (WHERE u.type = 'public_item_order') AS open_item_trade_price,
                  MIN(u.created_at) FILTER (WHERE u.type = 'public_item_order') AS item_first_listed_at
           FROM unified_trades u{joins} WHERE ({item}) IS NOT NULL"
    ));
    if f.only_minting {
        b.push(" AND type = 'public_item_order'");
    }
    if let Some(mn) = &f.min_price {
        let p = b.bind_string(mn.clone());
        b.push(&format!(" AND amount_received >= {p}::numeric"));
    }
    if let Some(mx) = &f.max_price {
        let p = b.bind_string(mx.clone());
        b.push(&format!(" AND amount_received <= {p}::numeric"));
    }
    b.push(" GROUP BY lower(u.network), u.chain_id, u.contract_address_sent, 4 )");
}

struct CatalogBooks {
    catalogue: Catalogue,
    nets: Vec<ChainNet>,
}

fn catalogue_chain_sql(alias: &str, catalogue: Catalogue) -> String {
    let ethereum = catalogue.chain_id(ChainNet::Ethereum);
    let matic = catalogue.chain_id(ChainNet::Matic);
    let verification = if catalogue == Catalogue::Testnet {
        format!(" AND {alias}.testnet_verified")
    } else {
        String::new()
    };
    format!(
        "{alias}.chain_id = CASE {alias}.network WHEN 'ethereum' THEN {ethereum} ELSE {matic} END{verification}"
    )
}

/// Concrete contract filters also bound the order aggregation.
fn listing_contracts(f: &CatalogFilters) -> Vec<String> {
    if !f.contract_addresses.is_empty() {
        return f
            .contract_addresses
            .iter()
            .map(|c| c.to_lowercase())
            .collect();
    }
    f.ids
        .iter()
        .filter_map(|id| super::wire::split_item_wire_id(id).map(|(contract, _)| contract))
        .collect()
}

/// Every legacy listing is bound to a token of the collection's served chain.
/// A v1 token's Issue identifies the wearable by its name in the collection URN.
fn build_legacy_orders_cte(b: &mut Builder, f: &CatalogFilters, books: &CatalogBooks, now_ms: i64) {
    let now = b.bind_i64(now_ms);
    let networks: Vec<String> = books
        .nets
        .iter()
        .map(|net| net.db_name().to_owned())
        .collect();
    let nets = b.bind_strings(&networks);
    b.push(&format!(
        "legacy_orders AS (
            SELECT o.network, o.contract_address,
                   CASE WHEN o.network = 'matic' THEN n.item_id ELSE i.item_id END AS item_id,
                   count(*) AS listings_count, min(o.price) AS min_price, max(o.price) AS max_price,
                   max(o.created_at) AS latest
            FROM shop.legacy_order o
            JOIN shop.nft n USING (network, contract_address, token_id)
            JOIN shop.collections c USING (network, contract_address)
            LEFT JOIN shop.items i
              ON o.network = 'ethereum' AND i.network = o.network
             AND i.contract_address = o.contract_address AND i.urn = c.urn || ':' || n.wearable_id
             AND {item_chain}
            WHERE o.network = ANY({nets}) AND {collection_chain}
              AND o.status = 'open' AND o.expires_at > {now} AND {status} = 'open'
              AND CASE WHEN o.network = 'matic' THEN n.item_id ELSE i.item_id END IS NOT NULL",
        item_chain = catalogue_chain_sql("i", books.catalogue),
        collection_chain = catalogue_chain_sql("c", books.catalogue),
        status = super::orders::book_status(false),
    ));
    let contracts = listing_contracts(f);
    if !contracts.is_empty() {
        let contracts = b.bind_strings(&contracts);
        b.push(&format!(" AND o.contract_address = ANY({contracts})"));
    }
    if let Some(value) = &f.min_price {
        let price = b.bind_string(value.clone());
        b.push(&format!(" AND o.price >= {price}::numeric"));
    }
    if let Some(value) = &f.max_price {
        let price = b.bind_string(value.clone());
        b.push(&format!(" AND o.price <= {price}::numeric"));
    }
    b.push(" GROUP BY o.network, o.contract_address, 3) ");
}

struct ListingValues {
    count: &'static str,
    min: &'static str,
    max: &'static str,
    latest: &'static str,
}

impl ListingValues {
    fn new(legacy: bool, v3: bool) -> Self {
        match (legacy, v3) {
            (true, true) => Self {
                count: "(COALESCE(legacy_orders.listings_count, 0) + COALESCE(offchain_orders.nfts_listings_count, 0))",
                min: "LEAST(legacy_orders.min_price, offchain_orders.min_order_amount_received)",
                max: "GREATEST(legacy_orders.max_price, offchain_orders.max_order_amount_received)",
                latest: "GREATEST(legacy_orders.latest, ROUND(EXTRACT(EPOCH FROM offchain_orders.max_created_at)))",
            },
            (true, false) => Self {
                count: "COALESCE(legacy_orders.listings_count, 0)",
                min: "legacy_orders.min_price",
                max: "legacy_orders.max_price",
                latest: "legacy_orders.latest",
            },
            (false, true) => Self {
                count: "COALESCE(offchain_orders.nfts_listings_count, 0)",
                min: "offchain_orders.min_order_amount_received",
                max: "offchain_orders.max_order_amount_received",
                latest: "ROUND(EXTRACT(EPOCH FROM offchain_orders.max_created_at))",
            },
            (false, false) => Self {
                count: "0",
                min: "NULL::numeric",
                max: "NULL::numeric",
                latest: "NULL::bigint",
            },
        }
    }
}

fn head_or_accessory_list(list: &[&str]) -> String {
    list.iter()
        .map(|c| format!("'{c}'"))
        .collect::<Vec<_>>()
        .join(", ")
}

/// The item-level predicates every catalog shape applies (old `build_collections_where`).
fn build_where(
    b: &mut Builder,
    f: &CatalogFilters,
    is_v2: bool,
    listings: &ListingValues,
    books: Option<&CatalogBooks>,
) {
    let mut parts: Vec<String> = vec!["items.search_is_collection_approved = true".to_string()];
    if let Some(books) = books {
        let networks: Vec<String> = books
            .nets
            .iter()
            .map(|net| net.db_name().to_owned())
            .collect();
        let nets = b.bind_strings(&networks);
        parts.push(format!("items.network = ANY({nets})"));
        parts.push(catalogue_chain_sql("items", books.catalogue));
    }
    if f.include_social_emotes == Some(false) {
        parts.push("items.emote_outcome_type IS NULL".to_string());
    }
    if let Some(cat) = f.category {
        match cat {
            NftCategory::Wearable => {
                if f.is_wearable_smart {
                    parts.push(format!("items.item_type = '{FRAGMENT_SMART_WEARABLE_V1}'"));
                } else {
                    parts.push(format!(
                        "items.item_type IN ({})",
                        WEARABLE_ITEM_TYPES
                            .iter()
                            .map(|t| format!("'{t}'"))
                            .collect::<Vec<_>>()
                            .join(", ")
                    ));
                }
            }
            NftCategory::Emote => parts.push(format!("items.item_type = '{FRAGMENT_EMOTE_V1}'")),
            _ => parts.push("TRUE".to_string()),
        }
    }
    if !f.rarities.is_empty() {
        let p = b.bind_strings(&f.rarities);
        parts.push(format!("items.rarity = ANY({p})"));
    }
    if !f.creator.is_empty() {
        let lower: Vec<String> = f.creator.iter().map(|c| c.to_lowercase()).collect();
        let p = b.bind_strings(&lower);
        parts.push(format!("items.creator = ANY({p})"));
    }
    if f.is_sold_out {
        parts.push("items.available = 0".to_string());
    }
    let store = "(items.search_is_store_minter = true AND items.available > 0)";
    let trade_mint = if is_v2 {
        format!(
            "(COALESCE({}, false) AND COALESCE(offchain_orders.items_listings_count, 0) > 0 AND items.available > 0)",
            v3_minter_expr()
        )
    } else {
        "false".to_owned()
    };
    let mint = format!("({store} OR {trade_mint})");
    if let Some(on_sale) = f.is_on_sale {
        let sale = if f.only_minting {
            mint.clone()
        } else {
            format!("({mint} OR {} > 0)", listings.count)
        };
        parts.push(if on_sale { sale } else { format!("NOT {sale}") });
    }
    if f.is_wearable_head {
        parts.push(format!(
            "items.category IN ({})",
            head_or_accessory_list(&HEAD_CATEGORIES)
        ));
    }
    if f.is_wearable_accessory {
        parts.push(format!(
            "items.category IN ({})",
            head_or_accessory_list(&ACCESSORY_CATEGORIES)
        ));
    }
    if let Some(c) = &f.wearable_category {
        let p = b.bind_string(c.clone());
        parts.push(format!(
            "items.category = {p} AND items.item_type <> '{FRAGMENT_EMOTE_V1}'"
        ));
    }
    if !f.wearable_genders.is_empty() {
        let mut shapes: Vec<String> = Vec::new();
        for g in &f.wearable_genders {
            match g.as_str() {
                "female" => shapes.push("BaseFemale".to_string()),
                "male" => shapes.push("BaseMale".to_string()),
                _ => {}
            }
        }
        if !shapes.is_empty() {
            let p = b.bind_strings(&shapes);
            parts.push(format!("items.body_shapes @> {p}"));
        }
    }
    if let Some(c) = &f.emote_category {
        let p = b.bind_string(c.clone());
        parts.push(format!(
            "items.category = {p} AND items.item_type = '{FRAGMENT_EMOTE_V1}'"
        ));
    }
    if f.emote_play_mode.len() == 1 {
        let p = b.bind_bool(f.emote_play_mode[0] == "loop");
        parts.push(format!("items.emote_loop = {p}"));
    }
    if !f.contract_addresses.is_empty() {
        let lower: Vec<String> = f
            .contract_addresses
            .iter()
            .map(|c| c.to_lowercase())
            .collect();
        let p = b.bind_strings(&lower);
        parts.push(format!("items.contract_address = ANY({p})"));
    }
    if let Some(id) = f.item_id.as_deref().filter(|s| !s.is_empty()) {
        let p = b.bind_string(id.to_string());
        parts.push(format!("items.item_id::text = {p}"));
    }
    let mint_trade_price = if is_v2 {
        "offchain_orders.open_item_trade_price"
    } else {
        "NULL::numeric"
    };
    if let Some(mn) = &f.min_price {
        let p = b.bind_string(mn.clone());
        let minting = format!(
            "(items.price >= {p}::numeric AND items.price IS DISTINCT FROM '{MAX_NUMERIC_NUMBER}'::numeric AND {mint})"
        );
        if f.only_minting {
            parts.push(format!("(items.price >= {p}::numeric AND items.price IS DISTINCT FROM '{MAX_NUMERIC_NUMBER}'::numeric)"));
        } else if f.only_listing {
            parts.push(format!("{} >= {p}::numeric", listings.min));
        } else {
            parts.push(format!(
                "({minting} OR {} >= {p}::numeric OR {mint_trade_price} >= {p}::numeric)",
                listings.min
            ));
        }
    }
    if let Some(mx) = &f.max_price {
        let p = b.bind_string(mx.clone());
        if f.only_minting {
            parts.push(format!("items.price <= {p}::numeric"));
        } else if f.only_listing {
            parts.push(format!("{} <= {p}::numeric", listings.max));
        } else {
            parts.push(format!("((items.price <= {p}::numeric AND {mint}) OR {} <= {p}::numeric OR {mint_trade_price} <= {p}::numeric)", listings.max));
        }
    }
    if f.only_listing {
        parts.push(format!("(NOT {mint} AND {} > 0)", listings.count));
    }
    if f.only_minting {
        parts.push(mint);
    }
    if !f.ids.is_empty() {
        let p = b.bind_strings(&f.ids);
        parts.push(format!(
            "(items.contract_address || '-' || items.item_id::text) = ANY({p})"
        ));
    }
    if f.emote_has_sound {
        parts.push("items.emote_has_sound = true".to_string());
    }
    if f.emote_has_geometry {
        parts.push("items.emote_has_geometry = true".to_string());
    }
    if f.emote_outcome_type.is_some() {
        parts.push("items.emote_outcome_type IS NOT NULL".to_string());
    }
    if !f.urns.is_empty() {
        let p = b.bind_strings(&expand_urn_network_forms(&f.urns));
        parts.push(format!("items.urn = ANY({p})"));
    }
    if let Some(n) = f.network {
        let p = b.bind_string(n.db_name().to_string());
        parts.push(format!("items.network = {p}"));
    }
    if let Some(term) = f.search.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        for word in term.split_whitespace().take(8) {
            let p = b.bind_string(format!("%{}%", escape_like(word)));
            parts.push(format!("(items.name ILIKE {p} OR c.name ILIKE {p})"));
        }
    }
    b.push(&format!(" WHERE {} ", parts.join(" AND ")));
}

pub fn escape_like(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for c in value.chars() {
        if matches!(c, '\\' | '%' | '_') {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

pub fn expand_urn_network_forms(urns: &[String]) -> Vec<String> {
    let mut out: Vec<String> = Vec::with_capacity(urns.len());
    let mut seen = std::collections::HashSet::new();
    for u in urns {
        for v in [
            u.clone(),
            u.replace(":mainnet:", ":ethereum:"),
            u.replace(":ethereum:", ":mainnet:"),
        ] {
            if seen.insert(v.clone()) {
                out.push(v);
            }
        }
    }
    out
}

fn build_order_by(b: &mut Builder, f: &CatalogFilters, is_v2: bool, listings: &ListingValues) {
    let sort_by = f.sort_by.unwrap_or(CatalogSortBy::Newest);
    if f.is_on_sale == Some(false) && sort_by != CatalogSortBy::Newest {
        b.push(" ORDER BY items.contract_address, items.item_id ");
        return;
    }
    b.push(" ORDER BY ");
    if f.search.is_some() && !f.ids.is_empty() {
        let p = b.bind_strings(&f.ids);
        b.push(&format!(
            "array_position({p}::text[], items.contract_address || '-' || items.item_id::text), "
        ));
    }
    let recent = format!(
        "GREATEST({}, items.first_listed_at) DESC NULLS LAST",
        listings.latest
    );
    let clause = match (sort_by, is_v2) {
        (CatalogSortBy::Newest | CatalogSortBy::Suggested, true) => {
            "GREATEST(COALESCE(ROUND(EXTRACT(EPOCH FROM offchain_orders.item_first_listed_at)), 0), items.first_listed_at) DESC NULLS LAST"
        }
        (CatalogSortBy::Newest | CatalogSortBy::Suggested, false) => {
            "items.first_listed_at DESC NULLS LAST"
        }
        (CatalogSortBy::MostExpensive, _) => "max_price_num DESC NULLS LAST",
        (CatalogSortBy::RecentlyListed, _) => &recent,
        (CatalogSortBy::RecentlySold, _) => "items.sold_at DESC",
        (CatalogSortBy::Cheapest, _) => "min_price_num ASC NULLS LAST, items.first_listed_at DESC",
    };
    b.push(clause);
    b.push(", items.contract_address ASC, items.item_id ASC ");
}

fn min_max_price_cases(
    b: &mut Builder,
    f: &CatalogFilters,
    is_v2: bool,
    listings: &ListingValues,
    books: Option<&CatalogBooks>,
) {
    if f.only_listing {
        b.push(&format!(
            " {} AS min_price_num, {}::text AS min_price, {} AS max_price_num, {}::text AS max_price, ",
            listings.min, listings.min, listings.max, listings.max
        ));
        return;
    }
    let v3 = v3_minter_expr();
    let mint = if books.is_some() {
        "items.available > 0 AND items.search_is_store_minter = true".to_owned()
    } else {
        format!("items.available > 0 AND (items.search_is_store_minter = true OR {v3})")
    };
    let mut min_when = mint.clone();
    let mut max_when = mint;
    if let Some(mn) = &f.min_price {
        let p = b.bind_string(mn.clone());
        min_when.push_str(&format!(" AND items.price >= {p}::numeric"));
    }
    if let Some(mx) = &f.max_price {
        let p = b.bind_string(mx.clone());
        max_when.push_str(&format!(" AND items.price <= {p}::numeric"));
    }
    let trade = if is_v2 && books.is_some() {
        format!(
            "CASE WHEN items.available > 0 AND {v3} THEN offchain_orders.open_item_trade_price END"
        )
    } else if is_v2 {
        "offchain_orders.open_item_trade_price".into()
    } else {
        "NULL::numeric".into()
    };
    let min = format!(
        "LEAST(CASE WHEN {min_when} THEN NULLIF(items.price, '{MAX_NUMERIC_NUMBER}'::numeric) END, {}, {trade})",
        listings.min
    );
    let max = format!(
        "GREATEST(CASE WHEN {max_when} THEN NULLIF(items.price, '{MAX_NUMERIC_NUMBER}'::numeric) END, {}, {trade})",
        listings.max
    );
    b.push(&format!(" {min} AS min_price_num, {min}::text AS min_price, {max} AS max_price_num, {max}::text AS max_price, "));
}

/// The page query: `is_v2` joins the off-chain legs, `count` swaps the
/// projection for `COUNT(*)` (the total when the page is empty past the end).
pub fn build_catalog_query(
    f: &CatalogFilters,
    is_v2: bool,
    now: chrono::NaiveDateTime,
    since: Option<chrono::NaiveDateTime>,
    count: bool,
) -> (String, PgArguments) {
    build_catalog_page_query(f, is_v2, now, since, count, None)
}

fn build_catalog_page_query(
    f: &CatalogFilters,
    is_v2: bool,
    now: chrono::NaiveDateTime,
    since: Option<chrono::NaiveDateTime>,
    count: bool,
    books: Option<&CatalogBooks>,
) -> (String, PgArguments) {
    let mut b = Builder::new();
    if is_v2 || books.is_some() {
        b.push("WITH ");
    }
    if is_v2 {
        let now_p = b.bind_time(now);
        let since_p = b.bind_opt_time(since);
        let contracts = listing_contracts(f);
        let contracts_p = (!contracts.is_empty()).then(|| b.bind_strings(&contracts));
        b.push(&unified_trades_cte_of(
            &now_p,
            &since_p,
            contracts_p.as_deref(),
        ));
        build_offchain_orders_cte(&mut b, f, books);
        b.push(if books.is_some() { ", " } else { " " });
    }
    if let Some(books) = books {
        build_legacy_orders_cte(&mut b, f, books, now.and_utc().timestamp_millis());
    }
    let listings = ListingValues::new(books.is_some(), is_v2);
    if count {
        b.push("SELECT COUNT(*)::int8 AS total ");
    } else {
        b.push(
            "SELECT items.contract_address, items.item_id::text AS item_id, items.item_type, items.urn, items.creator, items.beneficiary, \
                    items.rarity, items.available::text AS available, items.price::text AS price, items.image, items.name, items.description, \
                    items.category, items.body_shapes, items.emote_loop, items.emote_has_sound, items.emote_has_geometry, items.emote_outcome_type, \
                    items.search_is_store_minter, items.network, items.created_at, items.updated_at, items.reviewed_at, items.sold_at, ",
        );
        b.push(&format!(
            "COALESCE({}, false) AS v3_minter, ",
            v3_minter_expr()
        ));
        if is_v2 {
            b.push(" offchain_orders.open_item_trade_id::text AS open_item_trade_id, offchain_orders.open_item_trade_price::text AS open_item_trade_price, ");
        } else {
            b.push(" NULL::text AS open_item_trade_id, NULL::text AS open_item_trade_price, ");
        }
        b.push(&format!(
            " {}::text AS min_listing_price, {}::text AS max_listing_price, {}::int8 AS listings_count, ",
            listings.min, listings.max, listings.count
        ));
        if is_v2 && f.is_on_sale == Some(true) {
            b.push(" LEAST(items.first_listed_at, ROUND(EXTRACT(EPOCH FROM offchain_orders.item_first_listed_at))::bigint) AS first_listed_at, ");
        } else {
            b.push(" items.first_listed_at, ");
        }
        min_max_price_cases(&mut b, f, is_v2, &listings, books);
        b.push(" COUNT(*) OVER()::int8 AS total ");
    }
    b.push(" FROM shop.items items LEFT JOIN shop.collections c ON c.network = items.network AND c.contract_address = items.contract_address AND c.chain_id = items.chain_id ");
    if is_v2 {
        b.push(" LEFT JOIN offchain_orders ON offchain_orders.network = items.network AND offchain_orders.chain_id = items.chain_id AND offchain_orders.contract_address_sent = items.contract_address AND offchain_orders.item_id = items.item_id ");
    }
    if books.is_some() {
        b.push(" LEFT JOIN legacy_orders ON legacy_orders.network = items.network AND legacy_orders.contract_address = items.contract_address AND legacy_orders.item_id = items.item_id ");
    }
    build_where(&mut b, f, is_v2, &listings, books);
    if !count {
        build_order_by(&mut b, f, is_v2, &listings);
        if f.first.is_some() || f.skip.is_some() {
            let li = b.bind_i64(clamp_first(f.first, MAX_PAGE_LIMIT));
            let oi = b.bind_i64(clamp_skip(f.skip));
            b.push(&format!(" LIMIT {li} OFFSET {oi}"));
        }
    }
    (b.sql, b.args)
}

/// The old `fix_thumbnail`: the L2 thumbnail URN gets its `0x` and item id.
pub fn fix_thumbnail(thumbnail: &str, blockchain_id: &str) -> String {
    if thumbnail.is_empty() {
        return String::new();
    }
    let t = thumbnail
        .replace("polygon", "matic")
        .replace("mainnet", "ethereum");
    if t.contains("ethereum") {
        return t;
    }
    let mut parts: Vec<String> = t.split(':').map(String::from).collect();
    if parts.len() <= 5 {
        return t;
    }
    if !parts[5].starts_with("0x") {
        let with_prefix = format!("0x{}", parts[5]);
        parts[5] = with_prefix.replace("/thumbnail", &format!(":{blockchain_id}/thumbnail"));
    }
    parts.join(":")
}

fn row_to_item(r: &sqlx::postgres::PgRow) -> CatalogItem {
    let contract: String = r.get("contract_address");
    let item_id: String = r.get("item_id");
    let item_type: String = r.get("item_type");
    let rarity: String = r.get("rarity");
    let available: i64 = r.get::<String, _>("available").parse().unwrap_or(0);
    let store_minter: bool = r.get("search_is_store_minter");
    let v3_minter: bool = r.get("v3_minter");
    let open_item_trade_id: Option<String> = r.get("open_item_trade_id");
    let open_item_trade_price: Option<String> = r.get("open_item_trade_price");
    let priced_by_trade = available > 0 && open_item_trade_id.is_some() && v3_minter;
    let price: String = if available > 0 {
        if priced_by_trade {
            open_item_trade_price.clone().unwrap_or_else(|| "0".into())
        } else if store_minter {
            r.get("price")
        } else {
            "0".into()
        }
    } else {
        "0".into()
    };
    let description: Option<String> = r.get("description");
    let category: String = r.get("category");
    let body_shapes: Vec<String> = r.get("body_shapes");
    let (name, wire_category, data): (String, &'static str, ItemData) = if item_type
        == FRAGMENT_EMOTE_V1
    {
        (
            r.get("name"),
            "emote",
            ItemData::Emote {
                emote: serde_json::json!({
                    "description": description,
                    "category": category.to_lowercase(),
                    "bodyShapes": body_shapes,
                    "rarity": rarity,
                    "loop": r.get::<Option<bool>, _>("emote_loop").unwrap_or(false),
                    "hasGeometry": r.get::<Option<bool>, _>("emote_has_geometry").unwrap_or(false),
                    "hasSound": r.get::<Option<bool>, _>("emote_has_sound").unwrap_or(false),
                    "outcomeType": r.get::<Option<String>, _>("emote_outcome_type"),
                }),
            },
        )
    } else {
        (
            r.get("name"),
            "wearable",
            ItemData::Wearable {
                wearable: WearableData {
                    description,
                    category: Some(category),
                    body_shapes,
                    rarity: rarity.clone(),
                    is_smart: item_type == FRAGMENT_SMART_WEARABLE_V1,
                },
            },
        )
    };
    let (network, chain_id) = super::wire::network_and_chain(&r.get::<String, _>("network"));
    let listings: Option<i64> = r.get("listings_count");
    CatalogItem {
        id: format!("{contract}-{item_id}"),
        beneficiary: r.get("beneficiary"),
        item_id: item_id.clone(),
        name,
        thumbnail: fix_thumbnail(
            &r.get::<Option<String>, _>("image").unwrap_or_default(),
            &item_id,
        ),
        url: format!("/contracts/{contract}/items/{item_id}"),
        urn: r.get("urn"),
        category: wire_category,
        contract_address: contract,
        rarity,
        available,
        is_on_sale: (store_minter || (open_item_trade_id.is_some() && v3_minter)) && available > 0,
        trade_id: if priced_by_trade {
            open_item_trade_id
        } else {
            None
        },
        creator: r.get("creator"),
        data,
        network,
        chain_id,
        price,
        created_at: r.get("created_at"),
        updated_at: r.get("updated_at"),
        reviewed_at: r.get("reviewed_at"),
        first_listed_at: r.get("first_listed_at"),
        sold_at: r.get("sold_at"),
        min_price: r.get("min_price"),
        max_listing_price: r.get("max_listing_price"),
        min_listing_price: r.get("min_listing_price"),
        listings,
        owners: None,
        picks: None,
    }
}

async fn fetch_page(
    state: &MarketplaceState,
    read: &mut CatalogueRead,
    f: &CatalogFilters,
    is_v2: bool,
    books: Option<&CatalogBooks>,
) -> Result<(Vec<CatalogItem>, i64), Response> {
    // The read's clock: the trades check decided on the same instant.
    let now = read.now.naive_utc();
    let since = super::trades::mirrored_since(&state.settings, now);
    let (sql, args) = build_catalog_page_query(f, is_v2, now, since, false, books);
    let rows = sqlx::query_with(sqlx::AssertSqlSafe(sql), args)
        .fetch_all(&mut *read.tx)
        .await
        .map_err(|e| {
            tracing::error!(error = ?e, "catalog query failed");
            if super::nft_reads::cancelled(&e) {
                super::nft_reads::timed_out()
            } else {
                ApiError::bad_request("Couldn't fetch the catalog with the filters provided")
                    .into_response()
            }
        })?;
    let total: i64 = match rows.first() {
        Some(r) => r.get("total"),
        None if clamp_skip(f.skip) > 0 => {
            let (sql, args) = build_catalog_page_query(f, is_v2, now, since, true, books);
            sqlx::query_with(sqlx::AssertSqlSafe(sql), args)
                .fetch_one(&mut *read.tx)
                .await
                .map_err(super::nft_reads::read_error)?
                .get("total")
        }
        None => 0,
    };
    Ok((rows.iter().map(row_to_item).collect(), total))
}

/// Use only the networks that can contribute items on this read's snapshot.
async fn catalog_books(
    state: &MarketplaceState,
    read: &mut CatalogueRead,
    f: &CatalogFilters,
) -> Result<CatalogBooks, Response> {
    if !super::owned_items::walks_here(state) {
        return Err(super::owned_items::not_ready(
            "this node does not walk the nft index required by the catalogue's legacy orders",
        ));
    }
    super::nft_reads::set_read_timeout(&mut read.tx)
        .await
        .map_err(super::nft_reads::read_error)?;
    let catalogue = state.settings.catalogue();
    let mut nets: Vec<_> = ChainNet::ALL
        .into_iter()
        .filter(|&net| {
            read.covers_items(net)
                && f.network
                    .is_none_or(|network| network.db_name() == net.db_name())
        })
        .collect();
    let contracts = listing_contracts(f);
    if f.network.is_none() && nets.len() > 1 && !contracts.is_empty() {
        let matched: Vec<String> = sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
            "SELECT DISTINCT items.network FROM shop.items items
             WHERE items.contract_address = ANY($1) AND items.search_is_collection_approved
               AND {}",
            catalogue_chain_sql("items", catalogue)
        )))
        .bind(&contracts)
        .fetch_all(&mut *read.tx)
        .await
        .map_err(super::nft_reads::read_error)?;
        nets.retain(|net| matched.iter().any(|network| network == net.db_name()));
    }
    super::orders::require_published_book(
        state,
        &mut read.tx,
        &nets,
        read.now,
        super::orders::BookRoute::Catalog,
    )
    .await?;
    Ok(CatalogBooks { catalogue, nets })
}

async fn catalog_inner(
    state: SharedState,
    headers: HeaderMap,
    pairs: Vec<(String, String)>,
    is_v2: bool,
) -> Response {
    let filters = match parse_catalog_filters(&pairs) {
        Ok(filters) => filters,
        Err(e) => return e.into_response(),
    };
    // The page memo holds the read only in the request that fetches it: one
    // that waits on another's fetch drops it first, and starts again if that
    // fetch failed. The stamp is the copy the page was read from.
    let page = loop {
        // Every row carries the item's minter price: the money rule.
        let needs = if filters.only_minting {
            &[Mirror::Items][..]
        } else {
            &[Mirror::Items, Mirror::Collections][..]
        };
        let mut read = match catalogue::preflight(
            &state,
            Purpose::Priced,
            needs,
            filters.network.map(Network::graph),
        )
        .await
        {
            Ok(read) => read,
            Err(envelope) => return envelope,
        };
        let books = if filters.only_minting {
            None
        } else {
            match catalog_books(&state, &mut read, &filters).await {
                Ok(books) => Some(books),
                Err(envelope) => return envelope,
            }
        };
        if is_v2 {
            if let Some(books) = &books {
                let now = read.now;
                if let Err(error) = super::trades::ready_checked_for_networks_on(
                    &state,
                    &mut read.tx,
                    &books.nets,
                    now,
                )
                .await
                {
                    return error.into_response(super::nft_reads::read_error);
                }
            } else if let Err(envelope) = read.trades_ready(&state).await {
                return envelope;
            }
        }
        if is_v2 || books.is_some() {
            match fetch_page(&state, &mut read, &filters, is_v2, books.as_ref()).await {
                Ok(page) => {
                    read.note(&state);
                    break Arc::new(page);
                }
                Err(e) => return e,
            }
        }
        let (key, stamp) = ((filters.clone(), read.key()), read.stamp());
        let mut held = Some(read);
        match state
            .catalog_cache
            .page_or_fetch_holding(key, &mut held, async |read: &mut CatalogueRead| {
                fetch_page(&state, read, &filters, false, None)
                    .await
                    .map(Arc::new)
            })
            .await
        {
            Ok(Some(page)) => {
                stamp.note(&state);
                break page;
            }
            Ok(None) => continue,
            Err(e) => return e.into_response(),
        }
    };
    // The picks are no catalogue rows, and the signer check may call the
    // chain (EIP-1654): the read went back with the loop's scope, so neither
    // waits while this request holds a connection.
    let route = if is_v2 { "/v2/catalog" } else { "/v1/catalog" };
    let picked_by = match optional_signer(state.eip1654.as_deref(), &headers, "get", route).await {
        Ok(s) => s,
        Err(e) => return e.into_response(),
    };
    let ids: Vec<String> = page.0.iter().map(|i| i.id.clone()).collect();
    let stats: Vec<PickStats> = match picked_by.as_deref() {
        Some(user) => match picks_stats(&state.pool, &ids, Some(user)).await {
            Ok(s) => s,
            Err(e) => return e.into_response(),
        },
        None => match state
            .catalog_cache
            .picks_or_fetch(ids.clone(), || async {
                picks_stats(&state.pool, &ids, None).await.map(Arc::new)
            })
            .await
        {
            Ok(s) => s.as_ref().clone(),
            Err(e) => return e.into_response(),
        },
    };
    let mut by_id: HashMap<String, PickStats> =
        stats.into_iter().map(|s| (s.item_id.clone(), s)).collect();
    let picks = page.0.iter().map(|i| by_id.remove(&i.id)).collect();
    Json(CatalogPage { page, picks }).into_response()
}

pub async fn get_catalog_v1(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Query(pairs): Query<Vec<(String, String)>>,
) -> Response {
    catalog_inner(state, headers, pairs, false).await
}

pub async fn get_catalog_v2(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Query(pairs): Query<Vec<(String, String)>>,
) -> Response {
    catalog_inner(state, headers, pairs, true).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn pairs(raw: &[(&str, &str)]) -> Vec<(String, String)> {
        raw.iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    fn wearable_item() -> CatalogItem {
        CatalogItem {
            id: "0xc-1".into(),
            beneficiary: Some("0xbeef".into()),
            item_id: "1".into(),
            name: "hat".into(),
            thumbnail: "thumb".into(),
            url: "/contracts/0xc/items/1".into(),
            urn: "urn:x".into(),
            category: "wearable",
            contract_address: "0xc".into(),
            rarity: "epic".into(),
            available: 2,
            is_on_sale: true,
            trade_id: Some("t-1".into()),
            creator: "0xcr".into(),
            data: ItemData::Wearable {
                wearable: WearableData {
                    description: Some("d".into()),
                    category: Some("hat".into()),
                    body_shapes: vec!["BaseMale".into()],
                    rarity: "epic".into(),
                    is_smart: false,
                },
            },
            network: Network::Ethereum,
            chain_id: ChainId::EthereumMainnet,
            price: "10".into(),
            created_at: 1,
            updated_at: 2,
            reviewed_at: 3,
            first_listed_at: Some(99),
            sold_at: 4,
            min_price: Some("7".into()),
            max_listing_price: Some("20".into()),
            min_listing_price: Some("7".into()),
            listings: Some(3),
            owners: Some(5),
            picks: None,
        }
    }

    fn emote_item() -> CatalogItem {
        CatalogItem {
            id: "0xe-3".into(),
            beneficiary: None,
            item_id: "3".into(),
            name: "dance".into(),
            thumbnail: "et".into(),
            url: "/contracts/0xe/items/3".into(),
            urn: "urn:e".into(),
            category: "emote",
            contract_address: "0xe".into(),
            rarity: "rare".into(),
            available: 1,
            is_on_sale: false,
            trade_id: None,
            creator: "0xec".into(),
            data: ItemData::Emote {
                emote: json!({ "description": "edesc", "category": "dance", "bodyShapes": ["BaseFemale"], "rarity": "rare", "loop": true, "hasGeometry": false, "hasSound": true, "outcomeType": "win" }),
            },
            network: Network::Matic,
            chain_id: ChainId::MaticMainnet,
            price: "0".into(),
            created_at: 10,
            updated_at: 20,
            reviewed_at: 30,
            first_listed_at: None,
            sold_at: 40,
            min_price: None,
            max_listing_price: None,
            min_listing_price: None,
            listings: None,
            owners: None,
            picks: None,
        }
    }

    #[test]
    fn catalog_page_serializes_byte_identical_to_data_total() {
        let stats = PickStats {
            count: 8,
            item_id: "0xc-1".into(),
            picked_by_user: Some(true),
        };
        for picks in [
            vec![None, None],
            vec![Some(stats.clone()), None],
            vec![None, Some(stats.clone())],
        ] {
            let items = vec![wearable_item(), emote_item()];
            let mut expected = items.clone();
            for (item, pick) in expected.iter_mut().zip(&picks) {
                item.picks = pick.clone();
            }
            let expected = serde_json::to_string(&super::super::wire::DataTotal {
                data: expected,
                total: 7,
            })
            .unwrap();
            let got = serde_json::to_string(&CatalogPage {
                page: Arc::new((items, 7)),
                picks,
            })
            .unwrap();
            assert_eq!(got, expected);
        }
    }

    /// The squid's Polygon minter check takes OffChainMarketplace v1, v2 and v3, which
    /// the contract map names MarketplaceV3, V4 and V5.
    #[test]
    fn every_polygon_offchain_marketplace_version_is_a_minter() {
        let map: serde_json::Value =
            serde_json::from_slice(crate::evm::contracts::ADDRESSES_JSON).unwrap();
        for network in ["matic", "amoy"] {
            for key in ["MarketplaceV3", "MarketplaceV4", "MarketplaceV5"] {
                let address = map[network][key]
                    .as_str()
                    .unwrap_or_else(|| panic!("{network} {key} missing"))
                    .to_lowercase();
                assert!(
                    OFFCHAIN_MARKETPLACE_MINTERS.contains(&address.as_str()),
                    "{network} {key}"
                );
            }
        }
    }

    /// The pinned upstream `/v1/catalog?first=1` answer (tests/fixtures/upstream/catalog-first1.json
    /// of the old crate): every key, in kind.
    #[test]
    fn a_catalog_item_carries_every_upstream_key() {
        let v = serde_json::to_value(wearable_item()).unwrap();
        for key in [
            "id",
            "beneficiary",
            "itemId",
            "name",
            "thumbnail",
            "url",
            "urn",
            "category",
            "contractAddress",
            "rarity",
            "available",
            "isOnSale",
            "creator",
            "data",
            "network",
            "chainId",
            "price",
            "createdAt",
            "updatedAt",
            "reviewedAt",
            "firstListedAt",
            "soldAt",
            "minPrice",
            "maxListingPrice",
            "minListingPrice",
            "listings",
            "owners",
        ] {
            assert!(v.get(key).is_some(), "missing {key}");
        }
        assert_eq!(v["data"]["wearable"]["isSmart"], false);
        assert_eq!(v["network"], "ETHEREUM");
        assert_eq!(v["chainId"], 1);
        assert_eq!(v["tradeId"], "t-1");
        let e = serde_json::to_value(emote_item()).unwrap();
        assert!(
            e.get("tradeId").is_none() && e.get("minPrice").is_none(),
            "absent optionals are omitted"
        );
        assert_eq!(e["maxListingPrice"], serde_json::Value::Null);
        assert_eq!(e["data"]["emote"]["loop"], true);
    }

    #[test]
    fn filters_parse_with_upstream_defaults() {
        let f = parse_catalog_filters(&pairs(&[
            ("first", "34"),
            ("id", "0x7dd2c14796d6a411439f3ed465071fb1626084a8-0"),
            ("id", "0x0bf152a83a6fc55066c2b664b164ca2916ad38f5-2"),
        ]))
        .unwrap();
        assert_eq!(f.first, Some(34));
        assert_eq!(f.skip, Some(0));
        assert_eq!(
            f.sort_by,
            Some(CatalogSortBy::Cheapest),
            "cheapest is the default sort"
        );
        assert_eq!(f.ids.len(), 2);
        assert_eq!(f.include_social_emotes, Some(true));
        let f = parse_catalog_filters(&pairs(&[
            ("isOnSale", "false"),
            ("rarity", "epic"),
            ("rarity", "bogus"),
            ("wearableGender", "male"),
            ("includeSocialEmotes", "false"),
            ("network", "MATIC"),
        ]))
        .unwrap();
        assert_eq!(f.is_on_sale, Some(false));
        assert_eq!(f.rarities, vec!["epic"]);
        assert_eq!(f.network, Some(Network::Matic));
        assert_eq!(f.include_social_emotes, Some(false));
        let f = parse_catalog_filters(&pairs(&[])).unwrap();
        assert_eq!((f.first, f.skip, f.is_on_sale), (Some(20), Some(0), None));
        // The price bounds are MANA (the legacy `parseEther`), bound as wei.
        let f = parse_catalog_filters(&pairs(&[("minPrice", "2"), ("maxPrice", "2.75")])).unwrap();
        assert_eq!(
            (f.min_price.as_deref(), f.max_price.as_deref()),
            (Some("2000000000000000000"), Some("2750000000000000000"))
        );
        let f = parse_catalog_filters(&pairs(&[("minPrice", " "), ("maxPrice", "")])).unwrap();
        assert_eq!((f.min_price, f.max_price), (None, None));
        for bad in ["abc", "1e18", "0.0000000000000000001"] {
            let err = parse_catalog_filters(&pairs(&[("minPrice", bad)])).unwrap_err();
            assert_eq!(err.message(), format!("Invalid minPrice value: {bad}"));
        }
    }

    #[test]
    fn the_v2_query_joins_the_open_trades_and_the_v1_query_does_not() {
        let now = chrono::DateTime::from_timestamp(1_700_000_000, 0)
            .unwrap()
            .naive_utc();
        let f = CatalogFilters {
            first: Some(24),
            is_on_sale: Some(true),
            min_price: Some("1".into()),
            ..Default::default()
        };
        let (v2, _) = build_catalog_query(&f, true, now, None, false);
        assert!(v2.contains("WITH unified_trades AS"), "{v2}");
        assert!(
            v2.contains("NOT EXISTS (SELECT 1 FROM shop.trade_cancellations"),
            "{v2}"
        );
        assert!(
            v2.contains("t.expires_at > $1 AND t.effective_since <= $1"),
            "the clock is a bound parameter: {v2}"
        );
        assert!(
            v2.contains(ITEM_ID_SHIFT),
            "nft-order legs map token -> item by the layout: {v2}"
        );
        assert!(v2.contains("items.search_is_collection_approved = true"));
        assert!(v2.contains("LIMIT $"), "{v2}");
        assert!(
            !v2.contains("now()"),
            "no moving clock in the predicate: {v2}"
        );
        assert_eq!(
            v2.matches(&"NULLIF(".to_string()).count(),
            4,
            "the uint256-max sentinel of the LEAST arms never reaches the wire: {v2}"
        );
        assert!(
            v2.contains(&format!(
                "NULLIF(items.price, '{MAX_NUMERIC_NUMBER}'::numeric) END, offchain_orders.min_order_amount_received"
            )),
            "{v2}"
        );
        let (v1, _) = build_catalog_query(&f, false, now, None, false);
        assert!(!v1.contains("unified_trades"), "{v1}");
        assert!(v1.contains("NULL::text AS open_item_trade_id"), "{v1}");
        let (count, _) = build_catalog_query(&f, true, now, None, true);
        assert!(
            count.starts_with("WITH unified_trades")
                && count.contains("SELECT COUNT(*)::int8 AS total")
                && !count.contains("LIMIT"),
            "{count}"
        );
        let ids = CatalogFilters {
            ids: vec!["0xc-1".into()],
            search: Some("hat pirate".into()),
            ..Default::default()
        };
        let (sql, _) = build_catalog_query(&ids, true, now, None, false);
        assert!(
            sql.contains("(items.contract_address || '-' || items.item_id::text) = ANY("),
            "{sql}"
        );
        assert!(
            sql.contains("array_position("),
            "a search with ids keeps the ids' order: {sql}"
        );
        assert_eq!(
            sql.matches("ILIKE").count(),
            4,
            "one name-or-collection arm per word: {sql}"
        );
        let sorted = CatalogFilters {
            sort_by: Some(CatalogSortBy::Suggested),
            ..Default::default()
        };
        let (sql, _) = build_catalog_query(&sorted, false, now, None, false);
        assert!(
            sql.contains("ORDER BY items.first_listed_at DESC NULLS LAST"),
            "suggested falls back to newest: {sql}"
        );
    }

    #[test]
    fn thumbnails_and_urns_normalize_as_before() {
        assert_eq!(
            fix_thumbnail(
                "https://peer.decentraland.org/lambdas/collections/contents/urn:decentraland:polygon:collections-v2:c5aaaa47d4ed16932d9970352afbd5e0df5a4925/thumbnail",
                "0"
            ),
            "https://peer.decentraland.org/lambdas/collections/contents/urn:decentraland:matic:collections-v2:0xc5aaaa47d4ed16932d9970352afbd5e0df5a4925:0/thumbnail"
        );
        assert_eq!(fix_thumbnail("", "1"), "");
        assert_eq!(
            fix_thumbnail(
                "https://x/urn:decentraland:mainnet:collections-v1:atari_launch:hat/thumbnail",
                "1"
            ),
            "https://x/urn:decentraland:ethereum:collections-v1:atari_launch:hat/thumbnail"
        );
        let out = expand_urn_network_forms(&[
            "urn:decentraland:ethereum:collections-v1:0xabc:0".to_string()
        ]);
        assert!(out.contains(&"urn:decentraland:mainnet:collections-v1:0xabc:0".to_string()));
        assert_eq!(escape_like("50%_off\\"), "50\\%\\_off\\\\");
    }

    #[tokio::test]
    async fn sepolia_land_uses_its_ownership_publication_with_clock_skew() {
        use crate::land_authz::{LAND_MAINNET, LAND_SEPOLIA};
        use crate::market::chain_walk::ChainNet;
        let Some(db) = crate::test_db::OwnDatabase::create("listing_sepolia").await else {
            return;
        };
        let pool = &db.pool;
        let now = chrono::Utc::now().naive_utc();
        let signer = "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
        for land in [LAND_MAINNET, LAND_SEPOLIA] {
            sqlx::query(
                "INSERT INTO land_authz.index_cursor (network,land_registry,estate_registry,last_block,folded_block,folded_at,full_replay_started)
                 VALUES ($1, $2, $3, 101, 100, $4, true)",
            )
            .bind(land.network)
            .bind(land.land_registry)
            .bind(land.estate_registry)
            .bind(now + chrono::Duration::minutes(5))
            .execute(pool).await.unwrap();
            sqlx::query(
                "INSERT INTO land_authz.token_right (token_address,token_id,owner,updated_block,updated_log)
                 VALUES ($1, 1, $2, 90, 0)",
            )
            .bind(land.land_registry).bind(signer)
            .execute(pool).await.unwrap();
        }
        let backed = nft_trade_backed_sql(
            "$1::timestamp",
            "1::numeric",
            ChainNet::Ethereum.testnet_chain_id(),
            ChainNet::Matic.testnet_chain_id(),
        );
        let query = format!(
            "SELECT {backed} FROM (SELECT 'ETHEREUM' AS network, $2::text AS signer, gen_random_uuid() AS id) t
             CROSS JOIN (SELECT 3 AS asset_type, $3::text AS contract_address, gen_random_uuid() AS id) sent"
        );
        for (registry, expected) in [
            (LAND_SEPOLIA.land_registry, true),
            (LAND_MAINNET.land_registry, false),
        ] {
            let held: bool = sqlx::query_scalar(sqlx::AssertSqlSafe(query.as_str()))
                .bind(now)
                .bind(signer)
                .bind(registry)
                .fetch_one(pool)
                .await
                .unwrap();
            assert_eq!(held, expected, "{registry}");
        }
        db.drop().await;
    }

    #[tokio::test]
    async fn amoy_listing_ownership_needs_a_verified_watched_collection() {
        use crate::market::chain_walk::ChainNet;
        let Some(db) = crate::test_db::OwnDatabase::create("listing_amoy").await else {
            return;
        };
        let pool = &db.pool;
        let now = chrono::Utc::now().naive_utc();
        let signer = "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
        let contract = "0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
        sqlx::raw_sql(sqlx::AssertSqlSafe(format!(
            "INSERT INTO shop.chain_cursor (indexer, network, chain_id, start_block, last_block, folded_block, folded_at, full_history)
             VALUES ('nft', 'matic', 80002, 0, 100, 100, timezone('UTC', now()), true);
             INSERT INTO shop.nft (network, contract_address, token_id, owner, minted_block, minted_at, transferred_block, transferred_at, transferred_log)
             VALUES ('matic', '{contract}', 1, '{signer}', 2, 2, 2, 2, 0);
             INSERT INTO shop.nft_contract (network, contract_address, walked_from_block) VALUES ('matic', '{contract}', 1);
             INSERT INTO shop.collections
               (network, contract_address, chain_id, name, creator, owner, urn, is_completed, is_approved, is_editable, created_at, updated_at, reviewed_at, mirrored_at)
             VALUES ('matic', '{contract}', 80002, 'Amoy', '{signer}', '{signer}', 'urn:amoy:collection', true, true, false, 1, 1, 1, now())"
        ))).execute(pool).await.unwrap();
        let backed = nft_trade_backed_sql(
            "$1::timestamp",
            "1::numeric",
            ChainNet::Ethereum.testnet_chain_id(),
            ChainNet::Matic.testnet_chain_id(),
        );
        let query = format!(
            "SELECT {backed} FROM (SELECT 'MATIC' AS network, $2::text AS signer, gen_random_uuid() AS id) t
             CROSS JOIN (SELECT 3 AS asset_type, $3::text AS contract_address, gen_random_uuid() AS id) sent"
        );
        let held = || async {
            sqlx::query_scalar::<_, bool>(sqlx::AssertSqlSafe(query.as_str()))
                .bind(now)
                .bind(signer)
                .bind(contract)
                .fetch_one(pool)
                .await
                .unwrap()
        };
        assert!(
            !held().await,
            "an unverified collection cannot prove ownership"
        );
        sqlx::query(
            "UPDATE shop.collections SET testnet_verified = true WHERE contract_address = $1",
        )
        .bind(contract)
        .execute(pool)
        .await
        .unwrap();
        assert!(
            held().await,
            "the verified and watched collection proves ownership"
        );
        sqlx::query(
            "DELETE FROM shop.nft_contract WHERE network = 'matic' AND contract_address = $1",
        )
        .bind(contract)
        .execute(pool)
        .await
        .unwrap();
        assert!(
            !held().await,
            "removing the contract's watch withdraws the ownership proof"
        );
        db.drop().await;
    }

    /// Two open item orders for one item: the catalogue names the cheapest
    /// and its own price, never one order's id with another's price.
    #[tokio::test]
    async fn the_open_item_trade_id_and_price_come_from_one_order() {
        use crate::market::listings::{self, test_support};
        let Some(state) =
            crate::market::tests::support::state_with_db(super::super::Settings::for_tests()).await
        else {
            return;
        };
        let pool = &state.pool;
        crate::market::trade_events::tests::publish_cursors(pool).await;
        let contract = format!("0xc0ffee00{}", uuid::Uuid::new_v4().simple());
        let signer = test_support::signer().address().to_checksum(None);
        sqlx::query(
            "INSERT INTO shop.items (network, contract_address, item_id, chain_id, item_type, urn, creator, rarity, \
                 total_supply, max_supply, available, price, image, name, category, created_at, updated_at, reviewed_at, mirrored_at) \
             VALUES ('matic', $1, 1, 137, 'wearable_v2', $2, lower($3), 'common', 0, 10, 10, 0, '', 'Item', 'hat', 0, 0, 0, now())",
        )
        .bind(&contract)
        .bind(format!("urn:decentraland:matic:collections-v2:{contract}:1"))
        .bind(&signer)
        .execute(pool)
        .await
        .unwrap();
        let marketplace = listings::offchain_marketplace_v3(137).unwrap();
        sqlx::query(
            "INSERT INTO shop.collections (network, contract_address, chain_id, name, creator, owner, urn, is_completed, \
                 is_approved, is_editable, minters, created_at, updated_at, reviewed_at, mirrored_at) \
             VALUES ('matic', $1, 137, 'Minter fixture', lower($2), lower($2), $3, true, true, false, ARRAY[$4], 0, 0, 0, now())",
        )
        .bind(&contract)
        .bind(&signer)
        .bind(format!("urn:decentraland:matic:collections-v2:{contract}"))
        .bind(marketplace.address.to_lowercase())
        .execute(pool)
        .await
        .unwrap();
        let mut ids = Vec::new();
        for salt in ["0x21", "0x22"] {
            let mut order = test_support::parse(
                &test_support::trade_json_on(&signer, "0x00", 137)
                    .replace(
                        r#""type": "public_nft_order""#,
                        r#""type": "public_item_order""#,
                    )
                    .replace(r#""assetType": 3"#, r#""assetType": 4"#)
                    .replace(r#""tokenId": "42""#, r#""itemId": "1""#)
                    .replace(r#""salt": "0x1234""#, &format!(r#""salt": "{salt}""#)),
            );
            order.sent[0].contract_address = contract.clone();
            test_support::sign(&mut order, &marketplace);
            ids.push(
                listings::insert_trade(pool, &order, "MATIC", marketplace.address)
                    .await
                    .unwrap(),
            );
        }
        for id in &ids {
            crate::test_db::open_trade_verdict(pool, id).await;
        }
        // The greater id gets the lower price, so neither MAX pairs them.
        ids.sort();
        let (cheap, dear) = (ids[1].clone(), ids[0].clone());
        for (id, price) in [(&cheap, "5"), (&dear, "9")] {
            sqlx::query(
                "UPDATE shop.trade_assets_erc20 e SET amount = $2::numeric FROM shop.trade_assets a \
                 WHERE e.asset_id = a.id AND a.trade_id = $1::uuid",
            )
            .bind(id)
            .bind(price)
            .execute(pool)
            .await
            .unwrap();
        }
        let open_item_orders = || async {
            let mut b = Builder::new();
            let now_p = b.bind_time(chrono::Utc::now().naive_utc());
            let since_p = b.bind_opt_time(None);
            b.push("WITH ");
            b.push(&unified_trades_cte(&now_p, &since_p));
            build_offchain_orders_cte(&mut b, &CatalogFilters::default(), None);
            let contract_p = b.bind_string(contract.clone());
            b.push(&format!(
                " SELECT open_item_trade_id, open_item_trade_price::text FROM offchain_orders \
                  WHERE contract_address_sent = {contract_p}"
            ));
            sqlx::query_as_with::<_, (String, String), _>(sqlx::AssertSqlSafe(b.sql), b.args)
                .fetch_optional(pool)
                .await
                .unwrap()
        };
        assert_eq!(open_item_orders().await, Some((cheap, "5".to_string())));

        // A marketplace the collection does not list as a minter cannot sell its items.
        sqlx::query(
            "UPDATE shop.collections SET minters = ARRAY['0x0000000000000000000000000000000000000001'] \
             WHERE network = 'matic' AND contract_address = $1",
        )
        .bind(&contract)
        .execute(pool)
        .await
        .unwrap();
        assert_eq!(open_item_orders().await, None);

        for id in &ids {
            sqlx::query("DELETE FROM shop.trades WHERE id = $1::uuid")
                .bind(id)
                .execute(pool)
                .await
                .unwrap();
        }
        for delete in [
            "DELETE FROM shop.items WHERE contract_address = $1",
            "DELETE FROM shop.collections WHERE contract_address = $1",
        ] {
            sqlx::query(delete)
                .bind(&contract)
                .execute(pool)
                .await
                .unwrap();
        }
    }
}
