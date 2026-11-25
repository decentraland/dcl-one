//! Mainnet NFT walkers serve LAND, estate and mixed catalogue pages from a
//! published database snapshot. The page, total, estate members, rentals
//! and orders share that snapshot. Other routes keep their existing readers:
//! the marketplace subgraph serves shallow LAND and NAME queries without
//! a search on the routes that use it. The index window is 100,000 rows.
//! A live legacy order takes precedence over the owner's open v3 listing.

use axum::Json;
use axum::extract::{Query, State};
use axum::response::{IntoResponse, Response};
use serde::Serialize;
use serde_json::{Value, json};

use super::orders::{Order, order_from_graph};
use super::readers::{self, GraphPage, Ttl, seconds_to_ms};
use super::wire::{
    ApiError, ChainId, Network, NftCategory, Params, body_shapes_for_genders,
    emote_play_mode_clause, fix_urn,
};
use super::{MarketplaceState, SharedState};

pub const MAX_ORDER_TIMESTAMP: i64 = 253_378_408_747_000;
pub const DEFAULT_FIRST: i64 = 100;

#[derive(Debug, Clone, Default)]
pub struct NftFilters {
    pub first: Option<i64>,
    pub skip: Option<i64>,
    pub sort_by: Option<NftSortBy>,
    pub category: Option<NftCategory>,
    pub owner: Option<String>,
    pub tenant: Option<String>,
    pub is_on_sale: Option<bool>,
    pub is_on_rent: bool,
    pub search: Option<String>,
    pub is_land: bool,
    pub is_wearable_head: bool,
    pub is_wearable_accessory: bool,
    pub is_wearable_smart: bool,
    pub wearable_category: Option<String>,
    pub wearable_genders: Vec<String>,
    pub emote_category: Option<String>,
    pub emote_genders: Vec<String>,
    pub emote_play_mode: Vec<String>,
    pub contract_addresses: Vec<String>,
    pub creator: Vec<String>,
    pub token_id: Option<String>,
    pub item_rarities: Vec<String>,
    pub item_id: Option<String>,
    pub network: Option<Network>,
    pub rental_status: Vec<String>,
    pub adjacent_to_road: bool,
    pub min_distance_to_plaza: Option<f64>,
    pub max_distance_to_plaza: Option<f64>,
    pub min_estate_size: Option<f64>,
    pub max_estate_size: Option<f64>,
    pub min_price: Option<String>,
    pub max_price: Option<String>,
    pub emote_has_geometry: bool,
    pub emote_has_sound: bool,
    pub emote_outcome_type: Option<String>,
    pub rental_days: Vec<i64>,
    pub ids: Vec<String>,
    /// LAND held by the venue for this owner, resolved on the catalogue snapshot.
    pub(crate) unclaimed_land: Vec<String>,
    pub include_social_emotes: Option<bool>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NftSortBy {
    Name,
    Newest,
    RecentlyListed,
    RecentlySold,
    CheapestParcel,
}

impl NftSortBy {
    pub fn parse_str(s: &str) -> Option<Self> {
        match s {
            "name" => Some(Self::Name),
            "newest" => Some(Self::Newest),
            "recently_listed" => Some(Self::RecentlyListed),
            "recently_sold" => Some(Self::RecentlySold),
            // marketplace-server's `cheapest` (NFTSortBy.CHEAPEST, the ENS and LAND browses) and
            // the older `cheapest_parcel`: both order by the open listing's price.
            "cheapest" | "cheapest_parcel" => Some(Self::CheapestParcel),
            _ => None,
        }
    }
}

pub struct NftErrors;
impl NftErrors {
    pub const INVALID_SEARCH_BY_TENANT_AND_OWNER: &'static str =
        "Owner or tenant can be set, but not both.";
    pub const INVALID_TOKEN_ID: &'static str = "Invalid token id, token ids must be numbers";
    pub const MISSING_CONTRACT_ADDRESS: &'static str =
        "NFTs can't be queried by token id if no contract address is provided";
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "market/"))]
pub struct EstateParcel {
    pub x: i32,
    pub y: i32,
}

#[derive(Debug, Serialize)]
#[cfg_attr(
    feature = "ts",
    derive(ts_rs::TS),
    ts(export, export_to = "market/", rename_all = "camelCase")
)]
pub struct Nft {
    #[serde(rename = "activeOrderId")]
    pub active_order_id: Option<String>,
    pub category: String,
    #[serde(rename = "chainId")]
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub chain_id: ChainId,
    #[serde(rename = "contractAddress")]
    pub contract_address: String,
    #[serde(rename = "createdAt")]
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub created_at: i64,
    pub data: NftData,
    pub id: String,
    pub image: String,
    #[serde(rename = "issuedId")]
    pub issued_id: Option<String>,
    #[serde(rename = "itemId")]
    pub item_id: Option<String>,
    pub name: String,
    pub network: Network,
    #[serde(rename = "openRentalId")]
    pub open_rental_id: Option<String>,
    pub owner: String,
    #[serde(rename = "tokenId")]
    pub token_id: String,
    #[serde(rename = "soldAt")]
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub sold_at: i64,
    #[serde(rename = "updatedAt")]
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub updated_at: i64,
    pub url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub urn: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(untagged)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "market/"))]
pub enum NftData {
    Wearable { wearable: WearableData },
    Emote { emote: EmoteData },
    Parcel { parcel: ParcelData },
    Estate { estate: EstateData },
    Ens { ens: EnsData },
}

#[derive(Debug, Serialize)]
#[cfg_attr(
    feature = "ts",
    derive(ts_rs::TS),
    ts(export, export_to = "market/", rename_all = "camelCase")
)]
pub struct WearableData {
    #[serde(rename = "bodyShapes")]
    pub body_shapes: Vec<String>,
    pub category: String,
    pub description: String,
    pub rarity: String,
    #[serde(rename = "isSmart")]
    pub is_smart: bool,
}

#[derive(Debug, Serialize)]
#[cfg_attr(
    feature = "ts",
    derive(ts_rs::TS),
    ts(export, export_to = "market/", rename_all = "camelCase")
)]
pub struct EmoteData {
    #[serde(rename = "bodyShapes")]
    pub body_shapes: Vec<String>,
    pub category: String,
    pub description: String,
    pub rarity: String,
    #[serde(rename = "loop")]
    #[cfg_attr(feature = "ts", ts(rename = "loop"))]
    pub r#loop: bool,
    #[serde(rename = "hasSound")]
    pub has_sound: bool,
    #[serde(rename = "hasGeometry")]
    pub has_geometry: bool,
    #[serde(rename = "outcomeType")]
    pub outcome_type: Option<String>,
}

#[derive(Debug, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "market/"))]
pub struct ParcelData {
    pub x: String,
    pub y: String,
    pub description: Option<String>,
    pub estate: Option<ParcelEstate>,
}

#[derive(Debug, Serialize)]
#[cfg_attr(
    feature = "ts",
    derive(ts_rs::TS),
    ts(export, export_to = "market/", rename_all = "camelCase")
)]
pub struct ParcelEstate {
    pub name: String,
    #[serde(rename = "tokenId")]
    pub token_id: String,
}

#[derive(Debug, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "market/"))]
pub struct EstateData {
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub size: i64,
    pub description: Option<String>,
    pub parcels: Vec<EstateParcel>,
}

#[derive(Debug, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "market/"))]
pub struct EnsData {
    pub subdomain: String,
}

#[derive(Debug, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "market/"))]
pub struct NftResult {
    pub nft: Nft,
    #[cfg_attr(feature = "ts", ts(as = "Option<crate::market::orders::Order>"))]
    pub order: Option<Order>,
    #[cfg_attr(
        feature = "ts",
        ts(as = "Option<crate::rentals::types::RentalListing>")
    )]
    pub rental: Option<Value>,
}

#[derive(Debug, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "market/"))]
pub struct NftsResponseBody {
    pub data: Vec<NftResult>,
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub total: i64,
}

fn parse_optional_bool(p: &Params, key: &str) -> Option<bool> {
    if p.get_boolean(key) {
        p.get_string(key, None).map(|s| s == "true")
    } else {
        None
    }
}

pub fn parse_filters(pairs: &[(String, String)]) -> Result<NftFilters, ApiError> {
    let p = Params::new(pairs);
    let category = p
        .get_value(
            "category",
            &["parcel", "estate", "wearable", "ens", "emote"],
            None,
        )
        .and_then(|s| NftCategory::parse(&s));
    let network = p
        .get_value("network", &["ETHEREUM", "MATIC"], None)
        .and_then(|s| Network::parse(&s));
    let sort_by = p
        .get_value(
            "sortBy",
            &[
                "name",
                "newest",
                "recently_listed",
                "recently_sold",
                "cheapest",
                "cheapest_parcel",
            ],
            None,
        )
        .and_then(|s| NftSortBy::parse_str(&s));
    let rental_days: Vec<i64> = p
        .get_list("rentalDays", &[])
        .into_iter()
        .filter_map(|d| d.parse::<i64>().ok())
        .collect();
    Ok(NftFilters {
        first: p.get_number("first", None).map(|f| f as i64),
        skip: p.get_number("skip", None).map(|f| f as i64),
        sort_by,
        category,
        owner: p.get_address("owner", true, None),
        tenant: p
            .get_address("tenant", false, None)
            .map(|s| s.to_lowercase()),
        is_on_sale: parse_optional_bool(&p, "isOnSale"),
        is_on_rent: p.get_boolean("isOnRent"),
        search: p.get_string("search", None),
        is_land: p.get_boolean("isLand"),
        is_wearable_head: p.get_boolean("isWearableHead"),
        is_wearable_accessory: p.get_boolean("isWearableAccessory"),
        is_wearable_smart: p.get_boolean("isWearableSmart"),
        wearable_category: p.get_string("wearableCategory", None),
        wearable_genders: p.get_list("wearableGender", &[]),
        emote_category: p.get_string("emoteCategory", None),
        emote_genders: p.get_list("emoteGender", &[]),
        emote_play_mode: p.get_list("emotePlayMode", &[]),
        contract_addresses: p.get_address_list("contractAddress", false),
        creator: p.get_list("creator", &[]),
        token_id: p.get_string("tokenId", None),
        item_rarities: p.get_list("itemRarity", &[]),
        item_id: p.get_string("itemId", None),
        network,
        rental_status: p.get_list("rentalStatus", &[]),
        adjacent_to_road: p.get_boolean("adjacentToRoad"),
        min_distance_to_plaza: p.get_number("minDistanceToPlaza", None),
        max_distance_to_plaza: p.get_number("maxDistanceToPlaza", None),
        min_estate_size: p.get_number("minEstateSize", None),
        max_estate_size: p.get_number("maxEstateSize", None),
        min_price: super::items::parse_price_param(&p, "minPrice")?,
        max_price: super::items::parse_price_param(&p, "maxPrice")?,
        emote_has_geometry: p.get_boolean("emoteHasGeometry"),
        emote_has_sound: p.get_boolean("emoteHasSound"),
        emote_outcome_type: p.get_string("emoteOutcomeType", None),
        rental_days,
        ids: p.get_list("id", &[]),
        unclaimed_land: Vec::new(),
        include_social_emotes: Some(
            p.get_string("includeSocialEmotes", None).as_deref() != Some("false"),
        ),
    })
}

fn is_land_routed(f: &NftFilters) -> bool {
    f.is_land
        || matches!(
            f.category,
            Some(NftCategory::Parcel) | Some(NftCategory::Estate) | Some(NftCategory::Ens)
        )
        || f.adjacent_to_road
        || f.min_distance_to_plaza.is_some()
        || f.max_distance_to_plaza.is_some()
        || f.min_estate_size.is_some()
        || f.max_estate_size.is_some()
        || only_land_registries(&f.contract_addresses)
}

/// A contract filter naming only the LAND, estate and NAME registries reads the
/// marketplace source, as upstream's `?contractAddress&tokenId` lookup does: the
/// collections subgraphs never hold those tokens.
fn only_land_registries(addresses: &[String]) -> bool {
    let registries = super::collections::marketplace_contracts(super::wire::ethereum_chain_id());
    !addresses.is_empty()
        && addresses.iter().all(|a| {
            registries.iter().any(|c| {
                matches!(
                    c.category,
                    NftCategory::Parcel | NftCategory::Estate | NftCategory::Ens
                ) && c.address.eq_ignore_ascii_case(a)
            })
        })
}

/// Whether a read that names no category also reads the names and LAND
/// legs: upstream's `/v1/nfts` answers every category, so an owner's
/// unfiltered list holds their LAND, estates and names next to their
/// wearables and emotes. Only a read whose filters those legs carry joins
/// them (no wearable, emote, item or creator filter, no contract, Ethereum
/// or no network). `emoteOutcomeType` is not one of those: it narrows the
/// collections rows and leaves these legs' rows alone.
pub fn merges_land(f: &NftFilters) -> bool {
    !(f.category.is_some()
        || is_land_routed(f)
        || filters_collections_only(f)
        || !f.contract_addresses.is_empty()
        || matches!(f.network, Some(Network::Matic)))
}

pub(super) fn filters_collections_only(f: &NftFilters) -> bool {
    f.is_wearable_head
        || f.is_wearable_accessory
        || f.is_wearable_smart
        || f.wearable_category.is_some()
        || !f.wearable_genders.is_empty()
        || f.emote_category.is_some()
        || !f.emote_genders.is_empty()
        || !f.emote_play_mode.is_empty()
        || f.emote_has_geometry
        || f.emote_has_sound
        || !f.creator.is_empty()
        || !f.item_rarities.is_empty()
        || f.item_id.is_some()
}

/// The subgraph `where` for the filters; `now_ms` bounds the open order.
pub fn graph_where(f: &NftFilters, now_ms: i64) -> Value {
    let mut w = json!({});
    if let Some(o) = &f.owner {
        w["owner"] = json!(o);
    }
    if let Some(c) = f.category {
        w["category"] = json!(c.as_str());
    }
    if let Some(t) = &f.token_id {
        w["tokenId"] = json!(t);
    }
    if let Some(i) = &f.item_id {
        w["itemBlockchainId"] = json!(
            i.rsplit_once('-')
                .map(|(_, id)| id.to_string())
                .unwrap_or_else(|| i.clone())
        );
    }
    if f.is_wearable_head {
        w["searchIsWearableHead"] = json!(true);
    }
    if f.is_wearable_accessory {
        w["searchIsWearableAccessory"] = json!(true);
    }
    if f.is_land {
        w["searchIsLand"] = json!(true);
    }
    if f.is_wearable_smart {
        w["itemType"] = json!("smart_wearable_v1");
    }
    if !f.contract_addresses.is_empty() {
        w["contractAddress_in"] = json!(
            f.contract_addresses
                .iter()
                .map(|c| c.to_lowercase())
                .collect::<Vec<_>>()
        );
    }
    if !f.ids.is_empty() {
        w["id_in"] = json!(f.ids);
    }
    if let Some(mn) = f.min_distance_to_plaza {
        w["searchDistanceToPlaza_gte"] = json!(mn as i64);
    }
    if let Some(mx) = f.max_distance_to_plaza {
        w["searchDistanceToPlaza_lte"] = json!(mx as i64);
    }
    if f.adjacent_to_road {
        w["searchAdjacentToRoad"] = json!(true);
    }
    if let Some(mn) = f.min_estate_size {
        w["searchEstateSize_gte"] = json!(mn as i64);
    }
    if let Some(mx) = f.max_estate_size {
        w["searchEstateSize_lte"] = json!(mx as i64);
    }
    if let Some(mn) = &f.min_price {
        w["searchOrderPrice_gte"] = json!(mn);
    }
    if let Some(mx) = &f.max_price {
        w["searchOrderPrice_lte"] = json!(mx);
    }
    match f.is_on_sale {
        Some(true) => {
            w["searchOrderStatus"] = json!("open");
            w["searchOrderExpiresAt_gt"] = json!(now_ms.to_string());
        }
        Some(false) => w["searchOrderStatus_not"] = json!("open"),
        None => {}
    }
    if f.emote_has_sound {
        w["searchEmoteHasSound"] = json!(true);
    }
    if f.emote_has_geometry {
        w["searchEmoteHasGeometry"] = json!(true);
    }
    if let Some(ec) = &f.emote_category {
        w["searchEmoteCategory"] = json!(ec);
    }
    if let Some(wc) = &f.wearable_category {
        w["searchWearableCategory"] = json!(wc);
    }
    if let Some(mode) = emote_play_mode_clause(&f.emote_play_mode) {
        w["searchEmoteLoop"] = json!(mode);
    }
    if let Some(shapes) = body_shapes_for_genders(&f.emote_genders) {
        w["searchEmoteBodyShapes_contains"] = json!(shapes);
    }
    if let Some(shapes) = body_shapes_for_genders(&f.wearable_genders) {
        w["searchWearableBodyShapes_contains"] = json!(shapes);
    }
    if !f.creator.is_empty() {
        w["item_"] =
            json!({ "creator_in": f.creator.iter().map(|c| c.to_lowercase()).collect::<Vec<_>>() });
    }
    if !f.item_rarities.is_empty() {
        let key = match f.category {
            Some(NftCategory::Emote) => "searchEmoteRarity_in",
            _ => "searchWearableRarity_in",
        };
        w[key] = json!(f.item_rarities);
    }
    if f.include_social_emotes == Some(false) {
        w["searchEmoteOutcomeType"] = Value::Null;
    }
    w
}

fn graph_sort(sort_by: Option<NftSortBy>) -> (&'static str, &'static str) {
    match sort_by {
        Some(NftSortBy::Name) => ("name", "asc"),
        Some(NftSortBy::RecentlySold) => ("soldAt", "desc"),
        Some(NftSortBy::CheapestParcel) => ("searchOrderPrice", "asc"),
        Some(NftSortBy::RecentlyListed) => ("searchOrderCreatedAt", "desc"),
        Some(NftSortBy::Newest) | None => ("createdAt", "desc"),
    }
}

const LAND_FIELDS: &str = "id tokenId contractAddress category tokenURI image createdAt updatedAt soldAt name owner { address } \
    parcel { x y data { description } estate { id tokenId data { name } } } estate { size data { name description } parcels { x y } } ens { subdomain } \
    searchOrderStatus searchOrderExpiresAt searchOrderPrice searchOrderCreatedAt activeOrder { id marketplaceAddress nftAddress tokenId owner buyer price status expiresAt createdAt updatedAt }";

fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
        None => String::new(),
    }
}

fn parcels_of(v: Option<&Value>) -> Vec<EstateParcel> {
    v.and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .map(|p| EstateParcel {
                    x: readers::int_of(p, "x") as i32,
                    y: readers::int_of(p, "y") as i32,
                })
                .collect()
        })
        .unwrap_or_default()
}

/// A subgraph nft row as the wire result, with its live legacy order.
pub fn nft_from_graph(n: &Value, network: Network, now_ms: i64) -> NftResult {
    let category = readers::str_of(n, "category");
    let contract = readers::str_of(n, "contractAddress");
    let token_id = readers::str_of(n, "tokenId");
    let wearable = readers::at(n, "metadata/wearable").filter(|v| !v.is_null());
    let emote = readers::at(n, "metadata/emote").filter(|v| !v.is_null());
    let meta = wearable.or(emote);
    let rarity = meta
        .map(|m| readers::str_of(m, "rarity"))
        .unwrap_or_default();
    let description = meta.and_then(|m| readers::opt_str_of(m, "description"));
    let data = match category.as_str() {
        "wearable" => NftData::Wearable {
            wearable: WearableData {
                body_shapes: meta
                    .map(|m| readers::strings_of(m, "bodyShapes"))
                    .unwrap_or_default(),
                category: meta
                    .map(|m| readers::str_of(m, "category"))
                    .unwrap_or_default(),
                description: description.clone().unwrap_or_default(),
                rarity: rarity.clone(),
                is_smart: readers::str_of(n, "itemType") == "smart_wearable_v1",
            },
        },
        "parcel" => NftData::Parcel {
            parcel: ParcelData {
                x: readers::at(n, "parcel/x")
                    .map(|v| v.to_string().trim_matches('"').to_string())
                    .unwrap_or_default(),
                y: readers::at(n, "parcel/y")
                    .map(|v| v.to_string().trim_matches('"').to_string())
                    .unwrap_or_default(),
                description: readers::at(n, "parcel/data/description")
                    .and_then(|v| v.as_str().map(str::to_string)),
                estate: readers::at(n, "parcel/estate")
                    .filter(|e| !e.is_null())
                    .map(|e| ParcelEstate {
                        name: readers::at(e, "data/name")
                            .and_then(|v| v.as_str())
                            .map(str::to_string)
                            .unwrap_or_else(|| capitalize("estate")),
                        token_id: readers::str_of(e, "tokenId"),
                    }),
            },
        },
        "ens" => NftData::Ens {
            ens: EnsData {
                subdomain: readers::at(n, "ens/subdomain")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string(),
            },
        },
        "estate" => NftData::Estate {
            estate: EstateData {
                size: readers::at(n, "estate/size")
                    .and_then(|v| {
                        v.as_i64()
                            .or_else(|| v.as_str().and_then(|s| s.parse().ok()))
                    })
                    .unwrap_or(0),
                description: readers::at(n, "estate/data/description")
                    .and_then(|v| v.as_str().map(str::to_string)),
                parcels: parcels_of(readers::at(n, "estate/parcels")),
            },
        },
        _ => NftData::Emote {
            emote: EmoteData {
                body_shapes: meta
                    .map(|m| readers::strings_of(m, "bodyShapes"))
                    .unwrap_or_default(),
                category: meta
                    .map(|m| readers::str_of(m, "category"))
                    .unwrap_or_default(),
                description: description.clone().unwrap_or_default(),
                rarity: rarity.clone(),
                r#loop: meta.map(|m| readers::bool_of(m, "loop")).unwrap_or(false),
                has_sound: meta
                    .map(|m| readers::bool_of(m, "hasSound"))
                    .unwrap_or(false),
                has_geometry: meta
                    .map(|m| readers::bool_of(m, "hasGeometry"))
                    .unwrap_or(false),
                outcome_type: meta.and_then(|m| readers::opt_str_of(m, "outcomeType")),
            },
        },
    };
    let land_name = readers::at(n, "estate/data/name")
        .or_else(|| readers::at(n, "parcel/data/name"))
        .and_then(|v| v.as_str())
        .map(str::to_string);
    let name = meta
        .map(|m| readers::str_of(m, "name"))
        .filter(|s| !s.is_empty())
        .or(land_name)
        .or_else(|| readers::opt_str_of(n, "name"))
        .or_else(|| {
            readers::at(n, "ens/subdomain")
                .and_then(|v| v.as_str())
                .map(str::to_string)
        })
        .unwrap_or_else(|| capitalize(&category));
    let order = readers::at(n, "activeOrder")
        .filter(|o| !o.is_null())
        .map(|o| order_from_graph(o, network))
        .filter(|o| {
            o.status == "open" && o.expires_at > now_ms && o.expires_at < MAX_ORDER_TIMESTAMP
        });
    let nft = Nft {
        active_order_id: order.as_ref().map(|o| o.id.clone()),
        category: category.clone(),
        chain_id: network.chain_id(),
        contract_address: contract.clone(),
        created_at: seconds_to_ms(readers::int_of(n, "createdAt")),
        data,
        id: format!("{contract}-{token_id}"),
        image: fix_urn(&readers::str_of(n, "image")),
        issued_id: readers::opt_str_of(n, "issuedId"),
        item_id: readers::opt_str_of(n, "itemBlockchainId"),
        name,
        network,
        open_rental_id: None,
        owner: readers::at(n, "owner/address")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string(),
        token_id: token_id.clone(),
        sold_at: seconds_to_ms(readers::int_of(n, "soldAt")),
        updated_at: seconds_to_ms(readers::int_of(n, "updatedAt")),
        url: format!("/contracts/{contract}/tokens/{token_id}"),
        urn: readers::opt_str_of(n, "urn").map(|u| fix_urn(&u)),
    };
    NftResult {
        nft,
        order,
        rental: None,
    }
}

/// The old component's rental legs around the page: `isOnRent` over a LAND
/// category (or `isLand`) narrows it to the assets of the newest listings
/// in `rentalStatus`, and its parcels and estates carry their listing. A
/// walker reads the listings on its catalogue snapshot. The compatibility reader
/// logs a failed rental read and serves the page without listings.
pub async fn list_nfts(
    state: &MarketplaceState,
    f: &NftFilters,
) -> Result<(Vec<NftResult>, i64), Response> {
    validate(f)?;
    let chain_id = state.settings.market.indexes.chain_id_ethereum;
    // The pinned Decentraland contracts address book (01b40cff154a046f9ec509ac84be3643b51181c6,
    // addresses.json) has LANDProxy, EstateProxy and DCLRegistrar on Sepolia only on this pair.
    if super::owned_items::walks_here(state)
        && state.settings.catalogue() == super::nft_index::Catalogue::Testnet
        && f.network == Some(Network::Matic)
        && (f.is_land
            || matches!(
                f.category,
                Some(NftCategory::Parcel | NftCategory::Estate | NftCategory::Ens)
            ))
    {
        return Ok((vec![], 0));
    }
    let mut snapshot = None;
    if super::owned_items::walks_here(state)
        && let Some(land) = state.land_network()
        && let Some(sources) = super::nft_reads::land_page_sources(f, land)
    {
        let (first, skip) = index_page(f);
        if first.saturating_add(skip) > super::nft_reads::MAX_WINDOW {
            return Err(ApiError::bad_request(format!(
                "first + skip may not exceed {}",
                super::nft_reads::MAX_WINDOW
            ))
            .into_response());
        }
        let conn = super::nft_reads::land_page_snapshot(state, f, sources).await?;
        snapshot = Some((conn, sources));
    }
    let owner_filters;
    let mut f = f;
    if let Some((conn, _)) = snapshot.as_mut()
        && let Some(owner) = &f.owner
    {
        let super::nft_reads::ReadConn::Land(land) = conn else {
            return Err(super::nft_reads::land_unavailable());
        };
        owner_filters = NftFilters {
            unclaimed_land: super::rentals::unclaimed_land(land, owner).await?,
            ..f.clone()
        };
        f = &owner_filters;
    }
    let statuses = super::rentals::statuses(&f.rental_status);
    let wants_rentals = f.is_on_rent
        && (matches!(
            f.category,
            Some(NftCategory::Estate) | Some(NftCategory::Parcel)
        ) || f.is_land);
    let mut prefetched = None;
    let narrowed;
    if wants_rentals {
        let listings = match snapshot.as_mut() {
            Some((conn, _)) => super::rentals::newest_listings(&mut **conn, chain_id, &statuses)
                .await
                .map_err(super::nft_reads::read_error)?,
            None => super::rentals::newest_listings(&state.pool, chain_id, &statuses)
                .await
                .unwrap_or_else(|e| {
                    tracing::warn!(error = %e, "/v1/nfts: the rentals read failed");
                    Vec::new()
                }),
        };
        if listings.is_empty() {
            return Ok((Vec::new(), 0));
        }
        narrowed = NftFilters {
            ids: listings.iter().map(|l| l.nft_id.clone()).collect(),
            ..f.clone()
        };
        f = &narrowed;
        prefetched = Some(listings);
    }
    let (mut data, total) = match snapshot.as_mut() {
        Some((conn, sources)) => {
            let (first, skip) = index_page(f);
            super::nft_reads::list_page_on(state, f, first, skip, *sources, conn).await?
        }
        None => list_page(state, f).await?,
    };
    let listings = match prefetched {
        Some(listings) => listings,
        None => {
            let land: Vec<String> = data
                .iter()
                .filter(|r| matches!(r.nft.category.as_str(), "parcel" | "estate"))
                .map(|r| r.nft.id.clone())
                .collect();
            match snapshot.as_mut() {
                Some((conn, _)) => {
                    super::rentals::listings_of_nfts(&mut **conn, chain_id, &land, &statuses)
                        .await
                        .map_err(super::nft_reads::read_error)?
                }
                None => super::rentals::listings_of_nfts(&state.pool, chain_id, &land, &statuses)
                    .await
                    .unwrap_or_else(|e| {
                        tracing::warn!(error = %e, "/v1/nfts: the rentals read failed");
                        Vec::new()
                    }),
            }
        }
    };
    let by_nft: std::collections::HashMap<&str, &crate::rentals::types::RentalListing> =
        listings.iter().map(|l| (l.nft_id.as_str(), l)).collect();
    for r in &mut data {
        if let Some(listing) = by_nft.get(r.nft.id.as_str()) {
            r.nft.open_rental_id = Some(listing.id.clone());
            r.rental = serde_json::to_value(listing).ok();
        }
    }
    // Both readers carry only the legacy order. An nft listed through the
    // off-chain marketplace carries its open v3 listing, signed by its
    // owner, as its order, so a client sees it on sale (and relists by
    // cancelling that trade). A node with no source of trades has no such
    // listing. A single token's read (the sell and relist flow) is strict,
    // as `/v1/orders` is: unavailable until the trades mirror is ready, an
    // error if the read fails, never the token shown as unlisted. A list
    // page serves its legacy orders and logs when the mirror cannot answer.
    let has_trades =
        state.settings.trades_sync_upstream_url.is_some() || state.settings.data_writes_enabled;
    let single_token = f.contract_addresses.len() == 1 && f.token_id.is_some();
    let unlisted: Vec<(String, String, String, String)> = data
        .iter()
        .filter(|r| has_trades && r.order.is_none())
        .map(|r| {
            (
                r.nft.network.db_name().to_string(),
                r.nft.contract_address.to_ascii_lowercase(),
                r.nft.token_id.clone(),
                r.nft.owner.to_ascii_lowercase(),
            )
        })
        .collect();
    let mut trade_orders = if unlisted.is_empty() {
        Default::default()
    } else {
        let read = async {
            if let Some((conn, _)) = snapshot.as_mut() {
                // A price/order filter already checked this snapshot before reading the page.
                if !super::nft_reads::needs_orders(f) {
                    super::trades::ready_on(state, conn, chrono::Utc::now()).await?;
                }
                return super::orders::open_trade_orders_on(state, &mut **conn, &unlisted)
                    .await
                    .map_err(super::nft_reads::read_error);
            }
            super::trades::require_ready(state).await?;
            super::orders::open_trade_orders(state, &unlisted)
                .await
                .map_err(IntoResponse::into_response)
        };
        match read.await {
            Ok(orders) => orders,
            Err(response) if single_token => return Err(response),
            Err(response) => {
                tracing::warn!(status = %response.status(), "/v1/nfts: the open v3 listings could not be read; serving legacy orders only");
                Default::default()
            }
        }
    };
    for r in &mut data {
        if r.order.is_none()
            && let Some(order) = trade_orders.remove(&(
                r.nft.network.db_name().to_string(),
                r.nft.contract_address.to_ascii_lowercase(),
                r.nft.token_id.clone(),
            ))
        {
            r.nft.active_order_id = Some(order.id.clone());
            r.order = Some(order);
        }
    }
    Ok((data, total))
}

fn validate(f: &NftFilters) -> Result<(), Response> {
    if f.owner.is_some() && f.tenant.is_some() {
        return Err(
            ApiError::bad_request(NftErrors::INVALID_SEARCH_BY_TENANT_AND_OWNER).into_response(),
        );
    }
    if let Some(t) = &f.token_id {
        if !t.bytes().all(|b| b.is_ascii_digit()) || t.is_empty() {
            return Err(ApiError::bad_request(NftErrors::INVALID_TOKEN_ID).into_response());
        }
        if f.contract_addresses.is_empty() {
            return Err(ApiError::bad_request(NftErrors::MISSING_CONTRACT_ADDRESS).into_response());
        }
    }
    Ok(())
}

/// The order of a row among the merged sources' rows: the sort field (a wei price compared as
/// the whole number it is, never truncated to `i64`), then the name or the id. A collections NFT
/// carries its name in its metadata.
fn merge_key(v: &Value, order_by: &str) -> (u128, String) {
    let number = |key: &str| -> u128 { readers::str_of(v, key).trim().parse().unwrap_or(0) };
    match order_by {
        "name" => {
            let name = readers::opt_str_of(v, "name")
                .filter(|n| !n.is_empty())
                .or_else(|| {
                    ["metadata/wearable/name", "metadata/emote/name"]
                        .iter()
                        .find_map(|p| {
                            readers::at(v, p)
                                .and_then(|n| n.as_str())
                                .map(str::to_string)
                        })
                })
                .unwrap_or_default();
            (0, name)
        }
        field => (number(field), readers::str_of(v, "id")),
    }
}

fn index_page(f: &NftFilters) -> (i64, i64) {
    (
        f.first
            .unwrap_or(DEFAULT_FIRST)
            .clamp(0, readers::READER_MAX_FIRST),
        f.skip.unwrap_or(0).max(0),
    )
}

async fn list_page(
    state: &MarketplaceState,
    f: &NftFilters,
) -> Result<(Vec<NftResult>, i64), Response> {
    if super::owned_items::walks_here(state)
        && state.settings.catalogue() == super::nft_index::Catalogue::Testnet
    {
        let (first, skip) = index_page(f);
        return super::nft_reads::list_nfts(state, f, first, skip).await;
    }
    let reader_page = readers::reader_page(f.first, f.skip, DEFAULT_FIRST);
    let (first, skip) = match reader_page {
        Some(page) if is_land_routed(f) && f.search.is_none() => page,
        _ => {
            let (first, skip) = index_page(f);
            return super::nft_reads::list_nfts(state, f, first, skip).await;
        }
    };
    // Shallow NAME queries and LAND queries on non-walkers use the configured graph.
    let now_ms = readers::now_ms();
    let where_ = graph_where(f, now_ms);
    let (order_by, direction) = graph_sort(f.sort_by);
    let url = readers::require_marketplace(state)?;
    let page = GraphPage {
        entity: "nfts",
        filter_type: "NFT_filter",
        fields: LAND_FIELDS,
        where_: where_.clone(),
        order_by,
        direction,
        first,
        skip,
    };
    let read = async {
        let (got, skip_after) = readers::source_rows(
            state,
            Network::Ethereum.graph(),
            url,
            Ttl::S30,
            &page,
            first,
            skip,
            1,
        )
        .await?;
        let matching = readers::count_capped(
            state,
            Network::Ethereum.graph(),
            url,
            "nfts",
            "NFT_filter",
            where_,
        )
        .await?;
        Ok::<_, String>((got, skip_after, matching))
    };
    let (got, offset, total) = read
        .await
        .map_err(|e| readers::upstream_failed("/v1/nfts", &e))?;
    let key = |v: &Value| merge_key(v, order_by);
    let mut rows = got;
    rows.sort_by(|a, b| {
        let (ka, kb) = (key(a), key(b));
        let primary = if direction == "asc" {
            ka.0.cmp(&kb.0)
        } else {
            kb.0.cmp(&ka.0)
        };
        primary.then(ka.1.cmp(&kb.1))
    });
    let data = rows
        .into_iter()
        .skip(offset as usize)
        .take(first as usize)
        .map(|v| nft_from_graph(&v, Network::Ethereum, now_ms))
        .collect();
    Ok((data, total))
}

pub async fn get_nfts(
    State(state): State<SharedState>,
    Query(pairs): Query<Vec<(String, String)>>,
) -> Response {
    let filters = match parse_filters(&pairs) {
        Ok(f) => f,
        Err(e) => return e.into_response(),
    };
    match list_nfts(&state, &filters).await {
        Ok((data, total)) => Json(NftsResponseBody { data, total }).into_response(),
        Err(r) => r,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pairs(list: &[(&str, &str)]) -> Vec<(String, String)> {
        list.iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    /// The NAME browse's price range and sort: MANA bounds read as wei (the legacy `parsePrice`),
    /// `cheapest` read as the listing-price order, and a bound ethers would not parse refused
    /// with a 400.
    #[test]
    fn the_price_range_is_mana_and_cheapest_sorts_by_price() {
        let f = parse_filters(&pairs(&[
            ("category", "ens"),
            ("minPrice", "1"),
            ("maxPrice", "25"),
            ("sortBy", "cheapest"),
        ]))
        .unwrap();
        assert_eq!(f.min_price.as_deref(), Some("1000000000000000000"));
        assert_eq!(f.max_price.as_deref(), Some("25000000000000000000"));
        assert_eq!(f.sort_by, Some(NftSortBy::CheapestParcel));
        let w = graph_where(&f, 5_000);
        assert_eq!(
            (&w["searchOrderPrice_gte"], &w["searchOrderPrice_lte"]),
            (
                &json!("1000000000000000000"),
                &json!("25000000000000000000")
            )
        );
        assert_eq!(graph_sort(f.sort_by), ("searchOrderPrice", "asc"));
        let blank = parse_filters(&pairs(&[("minPrice", ""), ("maxPrice", " ")])).unwrap();
        assert_eq!((blank.min_price, blank.max_price), (None, None));
        let decimal = parse_filters(&pairs(&[
            ("minPrice", "1.5"),
            ("maxPrice", "0.000000000000000001"),
        ]))
        .unwrap();
        assert_eq!(
            (decimal.min_price.as_deref(), decimal.max_price.as_deref()),
            (Some("1500000000000000000"), Some("1"))
        );
        for bad in ["abc", "1e18", " 1 ", "0x10", "1.0000000000000000001"] {
            let Err(err) = parse_filters(&pairs(&[("minPrice", bad)])) else {
                panic!("minPrice={bad} was accepted")
            };
            assert_eq!(
                err.into_response().status(),
                axum::http::StatusCode::BAD_REQUEST,
                "minPrice={bad}"
            );
            assert!(
                parse_filters(&pairs(&[("maxPrice", bad)])).is_err(),
                "maxPrice={bad}"
            );
        }
    }

    #[test]
    fn the_filters_become_the_graph_where() {
        let f = parse_filters(&pairs(&[
            ("owner", "0xABCDEFABCDEFABCDEFABCDEFABCDEFABCDEFABCD"),
            ("category", "wearable"),
            ("isOnSale", "true"),
            ("itemRarity", "epic"),
            ("wearableGender", "unisex"),
            ("includeSocialEmotes", "false"),
            ("emotePlayMode", "loop"),
        ]))
        .unwrap();
        let w = graph_where(&f, 5_000);
        assert_eq!(w["owner"], "0xabcdefabcdefabcdefabcdefabcdefabcdefabcd");
        assert_eq!(w["category"], "wearable");
        assert_eq!(w["searchOrderStatus"], "open");
        assert_eq!(w["searchOrderExpiresAt_gt"], "5000");
        assert_eq!(w["searchWearableRarity_in"], json!(["epic"]));
        assert_eq!(
            w["searchWearableBodyShapes_contains"],
            json!(["BaseMale", "BaseFemale"])
        );
        assert!(w.get("searchEmoteOutcomeType").is_some_and(Value::is_null));
        assert_eq!(w["searchEmoteLoop"], true);
        assert_eq!(
            graph_sort(Some(NftSortBy::CheapestParcel)),
            ("searchOrderPrice", "asc")
        );
        assert!(is_land_routed(
            &parse_filters(&pairs(&[("category", "estate")])).unwrap()
        ));
        assert!(!is_land_routed(&f));
    }

    #[test]
    fn an_owner_read_without_a_category_also_reads_land_estates_and_names() {
        let merges = |list: &[(&str, &str)]| merges_land(&parse_filters(&pairs(list)).unwrap());
        let owner = ("owner", "0xabcdefabcdefabcdefabcdefabcdefabcdefabcd");
        assert!(merges(&[owner, ("includeSocialEmotes", "false")]));
        assert!(merges(&[owner, ("network", "ETHEREUM")]));
        assert!(merges(&[owner, ("isOnSale", "true")]));
        // Narrows the collections rows only (`nft_reads::push_filters`).
        assert!(merges(&[owner, ("emoteOutcomeType", "so")]));
        // A category, Polygon, or a filter only collections carry keeps the collections read alone.
        for extra in [
            ("category", "wearable"),
            ("category", "ens"),
            ("network", "MATIC"),
            ("itemRarity", "epic"),
            ("creator", "0x1111111111111111111111111111111111111111"),
            ("isWearableSmart", "true"),
            (
                "contractAddress",
                "0x4c290f486bae507719c562b6b524bdb71a2570c9",
            ),
        ] {
            assert!(!merges(&[owner, extra]), "{extra:?}");
        }
    }

    #[test]
    fn merged_rows_sort_by_fields_every_source_carries() {
        let wearable = json!({ "id": "w", "metadata": { "wearable": { "name": "Zebra hat" } }, "searchOrderPrice": "340282366920938463463374607431768211455" });
        let parcel =
            json!({ "id": "p", "name": "Alpha plaza", "searchOrderPrice": "9223372036854775808" });
        let emote = json!({ "id": "e", "metadata": { "emote": { "name": "Bow" } } });
        assert_eq!(merge_key(&wearable, "name").1, "Zebra hat");
        assert_eq!(merge_key(&parcel, "name").1, "Alpha plaza");
        assert_eq!(merge_key(&emote, "name").1, "Bow");
        // Wei past i64::MAX keeps its order; a row with no listing sorts as 0.
        assert!(merge_key(&parcel, "searchOrderPrice") < merge_key(&wearable, "searchOrderPrice"));
        assert!(merge_key(&parcel, "searchOrderPrice").0 > i64::MAX as u128);
        assert_eq!(merge_key(&emote, "searchOrderPrice").0, 0);
        assert_eq!(
            merge_key(&json!({ "id": "x", "createdAt": "1700" }), "createdAt"),
            (1700, "x".to_string())
        );
    }

    #[test]
    fn a_registry_contract_lookup_reads_the_land_subgraph() {
        let token = |contract: &str| {
            parse_filters(&pairs(&[("contractAddress", contract), ("tokenId", "42")])).unwrap()
        };
        for registry in [
            "0xF87E31492FAF9A91B02EE0DEAAD50D51D56D5D4D",
            "0x959e104e1a4db6317fa58f8295f586e1a978c297",
            "0x2a187453064356c898cae034eaed119e1663acb8",
        ] {
            assert!(is_land_routed(&token(registry)), "{registry}");
        }
        assert!(!is_land_routed(&token(
            "0x4c290f486bae507719c562b6b524bdb71a2570c9"
        )));
        assert!(!is_land_routed(
            &parse_filters(&pairs(&[
                (
                    "contractAddress",
                    "0xf87e31492faf9a91b02ee0deaad50d51d56d5d4d"
                ),
                (
                    "contractAddress",
                    "0x4c290f486bae507719c562b6b524bdb71a2570c9"
                ),
            ]))
            .unwrap()
        ));
    }

    #[test]
    fn a_graph_nft_keeps_the_wire_shape() {
        let n = json!({
            "id": "0xc-5", "tokenId": "5", "contractAddress": "0xc", "category": "wearable", "itemType": "smart_wearable_v1", "urn": "urn:decentraland:mainnet:collections-v2:0xc:1",
            "image": "https://peer.decentraland.org/lambdas/collections/contents/urn:decentraland:mainnet:collections-v2:0xc:1/thumbnail", "issuedId": "2", "itemBlockchainId": "1",
            "createdAt": "1700000000", "updatedAt": "1700000001", "soldAt": "0", "owner": { "address": "0xo" },
            "metadata": { "wearable": { "name": "Hat", "description": "d", "category": "hat", "rarity": "epic", "bodyShapes": ["BaseMale"] }, "emote": null },
            "activeOrder": { "id": "o1", "marketplaceAddress": "0xm", "nftAddress": "0xc", "tokenId": "5", "owner": "0xo", "buyer": null, "price": "10", "status": "open", "expiresAt": "1800000000000", "createdAt": "1700000000", "updatedAt": "1700000000" }
        });
        let r = nft_from_graph(&n, Network::Matic, 1_000);
        let v = serde_json::to_value(&r).unwrap();
        assert_eq!(v["nft"]["id"], "0xc-5");
        assert_eq!(
            v["nft"]["urn"],
            "urn:decentraland:ethereum:collections-v2:0xc:1"
        );
        assert_eq!(v["nft"]["data"]["wearable"]["isSmart"], true);
        assert_eq!(v["nft"]["activeOrderId"], "o1");
        assert_eq!(v["nft"]["createdAt"], 1_700_000_000_000i64);
        assert_eq!(v["nft"]["chainId"], 137);
        assert_eq!(v["order"]["price"], "10");
        assert!(v["rental"].is_null() && v["nft"]["openRentalId"].is_null());
        let expired = nft_from_graph(&n, Network::Matic, 1_900_000_000_000);
        assert!(expired.order.is_none() && expired.nft.active_order_id.is_none());
    }

    /// The rental filters no longer answer a deferred envelope: `isOnRent`
    /// over LAND with no listing in the statuses is the old empty page, and
    /// `tenant`/`rentalDays` pass through to the reader as the old builder
    /// let them (here without the nft index: the index's own envelope). A
    /// database of its own, so no other test's published index reaches it.
    #[tokio::test]
    async fn the_rental_filters_are_served() {
        let Some(own) = crate::market::tests::support::OwnDb::create("rental_filters").await else {
            return;
        };
        let state = std::sync::Arc::new(crate::market::MarketplaceState::new(
            own.pool.clone(),
            crate::http::Outbound::for_tests(),
            crate::market::Settings::for_tests(),
        ));
        let get = |q: &[(&str, &str)]| {
            let f = parse_filters(&pairs(q)).unwrap();
            let state = state.clone();
            async move { list_nfts(&state, &f).await }
        };
        let (data, total) = get(&[
            ("isOnRent", "true"),
            ("category", "parcel"),
            ("rentalStatus", "bogus"),
        ])
        .await
        .unwrap();
        assert_eq!((data.len(), total), (0, 0));
        for q in [
            &[("tenant", "0x000000000000000000000000000000000000a11c")][..],
            &[("rentalDays", "7")][..],
            &[("isOnRent", "true")][..],
        ] {
            let Err(resp) = get(q).await else {
                panic!("{q:?}: the nft index is not published")
            };
            let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
                .await
                .unwrap();
            let body: Value = serde_json::from_slice(&body).unwrap();
            assert_ne!(body["source"], "rentals-reader", "{q:?}: {body}");
            assert_eq!(body["source"], "nft-index", "{q:?}: {body}");
        }
        let Err(resp) = get(&[
            ("owner", "0x000000000000000000000000000000000000a11c"),
            ("tenant", "0x000000000000000000000000000000000000a11c"),
        ])
        .await
        else {
            panic!("owner with tenant is refused")
        };
        assert_eq!(resp.status(), axum::http::StatusCode::BAD_REQUEST);
        drop(state);
        own.drop().await;
    }
}
