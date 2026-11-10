//! The places, map, worlds and categories read routes (`/api/...`).
//!
//! Each route first validates the object its upstream handler built from the
//! URL (`validate`, wkc flavour: `Router.validator`), so a malformed query
//! answers upstream's 400 before any lookup; the parsed filters follow.
//! `GET /api/places` follows the discovery server instead: it reads the
//! query like `get-places-handler.ts` and has no schema 400.

use std::collections::HashMap;

use axum::Json;
use axum::extract::{OriginalUri, Path, Query, State};
use axum::http::{HeaderMap, Method};
use axum::response::{IntoResponse, Response};
use serde::Serialize;
use serde::ser::SerializeMap;
use serde_json::json;

use super::{
    ActiveRows, CategoryTarget, PlaceListFilters, PlaceOrderBy, PlaceRow, PlaceStatusRow, WorldRow,
    live,
};
use crate::SharedState;
use crate::shared::errors::{ApiData, ApiDataTotal, ApiError};
use crate::validate::{self, Flavour, QueryBody};

/// `getPlaceParamsSchema` on `:place_id` (places `Place/routes/getPlace.ts:23`,
/// `getPlaceCategories.ts:16`): a non-UUID is a 400, not a lookup miss.
/// Captures `places.get_place.invalid_uuid`, `places.get_place_categories.invalid_uuid`.
fn validate_place_id(place_id: &str) -> Result<(), ApiError> {
    validate::validate(
        &validate::PLACE_PARAMS,
        &json!({ "place_id": place_id }),
        Flavour::Wkc,
    )
}

/// The `order_by` values `POST /api/places` and the map's place list map
/// through `oneOf` (`getPlaceListById.ts:38-49`, `Place/model.ts:1150-1156`).
const LIST_ORDER_BY: &[&str] = &["like_score", "updated_at", "created_at"];
/// The orders `GET /api/places` accepts (discovery server
/// `get-places-handler.ts:10`); any other value is `like_score`.
const PLACE_LIST_ORDER_BY: &[&str] = &["like_score", "updated_at", "created_at", "most_active"];
/// `MAX_BATCH_ITEMS` (`query-params.ts`): more values of a list parameter is a 400.
const LIST_VALUES_MAX: usize = 1000;
/// The map routes' orders: the list's plus `most_active`
/// (`getAllPlacesMostActiveList.ts`, `getPlaceMostActiveList.ts`).
const MAP_ORDER_BY: &[&str] = &["like_score", "updated_at", "created_at", "most_active"];
/// `GET /api/worlds` (`World/schemas.ts:5-81`, `World/model.ts:351-355`):
/// `most_active` is dcl-one's live sort, the rest upstream's `oneOf`; anything
/// else is upstream's `like_score` fallback.
const WORLD_ORDER_BY: &[&str] = &["most_active", "like_score", "created_at"];

/// The query of `GET /api/places` as the discovery server reads it
/// (`get-places-handler.ts:12-34`, `query-params.ts:12-41`). A flag is true
/// only as `true`. `limit` and `offset` are JavaScript non-negative integers;
/// any other value is the default. `categories` and `names` split on commas.
/// More than 1000 positions, categories or names is a 400. `names` turns
/// `positions` off. An empty text parameter is absent.
fn list_filters(pairs: &[(String, String)]) -> Result<PlaceListFilters, ApiError> {
    let get = |k: &str| pairs.iter().find(|(p, _)| p == k).map(|(_, v)| v.as_str());
    let text = |k: &str| get(k).filter(|v| !v.is_empty()).map(str::to_string);
    let number = |k: &str| get(k).and_then(js_uint);
    let list = |k: &str| {
        capped(
            k,
            pairs
                .iter()
                .filter(|(p, _)| p == k)
                .flat_map(|(_, v)| v.split(','))
                .map(str::trim)
                .filter(|v| !v.is_empty())
                .map(str::to_string)
                .collect(),
        )
    };
    let positions = capped(
        "positions",
        pairs
            .iter()
            .filter(|(p, v)| p == "positions" && !v.is_empty())
            .map(|(_, v)| v.clone())
            .collect(),
    )?;
    let categories = list("categories")?;
    let names = list("names")?;
    let only_highlighted = get("only_highlighted") == Some("true");
    Ok(PlaceListFilters {
        limit: number("limit").unwrap_or(100).min(100),
        offset: number("offset").unwrap_or(0),
        positions: if names.is_empty() { positions } else { vec![] },
        categories,
        only_highlighted,
        search: text("search"),
        creator_address: text("creator_address").map(|s| s.to_lowercase()),
        sdk: text("sdk"),
        order_by: PlaceOrderBy::parse_allowed(
            get("order_by"),
            PLACE_LIST_ORDER_BY,
            PlaceOrderBy::LikeScore,
        ),
        order_desc: get("order") != Some("asc"),
        // `places-repository/component.ts:65-67`: `world IS false` unless
        // `only_highlighted` or `names` ask for world scenes too.
        only_places: !only_highlighted && names.is_empty(),
        places_table: true,
        names,
        ..Default::default()
    })
}

fn capped(key: &str, values: Vec<String>) -> Result<Vec<String>, ApiError> {
    if values.len() > LIST_VALUES_MAX {
        return Err(ApiError::bad_request(format!(
            "Too many {key} (max {LIST_VALUES_MAX})"
        )));
    }
    Ok(values)
}

/// `getPlaceMostActiveList.ts:83-99`: the rows by headcount, busiest first
/// (`order=asc`: emptiest first), the stored order on ties, then the requested
/// page; `total` is the size of the set. An offset past the places with
/// players answers nothing, total 0 (`:51-56`): `hot` is the hot-scenes
/// length, the positions with players in the presence snapshot whatever the
/// filters kept; without a snapshot the rows of the set with a mirrored
/// headcount.
fn busiest_first(
    mut rows: Vec<PlaceRow>,
    (offset, limit): (usize, usize),
    desc: bool,
    hot: Option<usize>,
) -> (Vec<PlaceRow>, i64) {
    let count = |row: &PlaceRow| row.user_count.unwrap_or(0);
    let hot = hot.unwrap_or_else(|| rows.iter().filter(|row| count(row) > 0).count());
    if offset > hot {
        return (vec![], 0);
    }
    if desc {
        rows.sort_by_key(|row| std::cmp::Reverse(count(row)));
    } else {
        rows.sort_by_key(count);
    }
    let total = rows.len() as i64;
    (rows.into_iter().skip(offset).take(limit).collect(), total)
}

/// `intParam` (`query-params.ts:27-32`): `Number(raw)` when that is a
/// non-negative integer; blank or any other value is none.
fn js_uint(raw: &str) -> Option<i64> {
    let raw = raw.trim();
    let digits = |radix: u32| {
        let digits = &raw[2..];
        (!digits.is_empty() && digits.chars().all(|c| c.is_digit(radix)))
            .then(|| u64::from_str_radix(digits, radix).ok())
            .flatten()
            .map(|n| n as f64)
    };
    let n = match raw.get(..2).map(str::to_ascii_lowercase).as_deref() {
        Some("0x") => digits(16),
        Some("0o") => digits(8),
        Some("0b") => digits(2),
        _ => raw.parse::<f64>().ok(),
    }?;
    (n.is_finite() && n.fract() == 0.0 && n >= 0.0).then_some(n as i64)
}

/// Explicit favorites requests must not disguise a failed signed login as an empty list.
/// Keep the legacy unsigned read empty; public optional viewer reads remain anonymous.
pub(crate) async fn favorites_viewer(
    state: &SharedState,
    headers: &HeaderMap,
    method: &str,
    path: &str,
    only_favorites: bool,
) -> Result<Option<String>, ApiError> {
    let has_auth = headers
        .keys()
        .any(|name| name.as_str().starts_with("x-identity-"));
    if only_favorites && has_auth {
        return state.signer(headers, method, path).await.map(Some);
    }
    Ok(state.viewer(headers, method, path).await)
}

pub async fn get_place(
    State(state): State<SharedState>,
    method: Method,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
    Query(pairs): Query<Vec<(String, String)>>,
    Path(place_id): Path<String>,
) -> Result<Json<ApiData<PlaceRow>>, ApiError> {
    validate_place_id(&place_id)?;
    let user = state.viewer(&headers, method.as_str(), uri.path()).await;
    match state
        .places
        .find_by_id_for(&place_id, user.as_deref())
        .await?
    {
        Some(mut p) => {
            live::fill_user_counts(
                std::slice::from_mut(&mut p),
                &*state.live.live_user_counts().await,
            );
            p.apply_realms_detail(realms_detail_for(&state, &pairs, false));
            Ok(Json(ApiData::ok(p)))
        }
        None => Err(ApiError::not_found(format!(
            "Not found place \"{place_id}\""
        ))),
    }
}

/// `GET /api/places` as the discovery server serves it
/// (`logic/places/component.ts:43-73`); `most_active` as the places service
/// does (`getPlaceMostActiveList.ts:75-99`, `Place/model.ts:630`): 100 rows,
/// the places with players first and the rest in like-score order, sorted by
/// headcount in memory in the requested direction and paged; `total` is the
/// size of that set, 0 with nothing once the offset passes the places with
/// players (`:51-56`).
pub async fn get_place_list(
    State(state): State<SharedState>,
    method: Method,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
    Query(pairs): Query<Vec<(String, String)>>,
) -> Result<Json<ApiDataTotal<PlaceRow>>, ApiError> {
    let mut filters = list_filters(&pairs)?;
    let get = |k: &str| pairs.iter().find(|(p, _)| p == k).map(|(_, v)| v.as_str());
    let only_favorites = get("only_favorites") == Some("true");
    let user = favorites_viewer(
        &state,
        &headers,
        method.as_str(),
        uri.path(),
        only_favorites,
    )
    .await?;
    if !state
        .places
        .scope_to_viewer(&mut filters, user.as_deref(), only_favorites)
    {
        return Ok(Json(ApiDataTotal::ok(vec![], 0)));
    }
    if let Some(owner) = get("owner").filter(|o| !o.is_empty()) {
        filters.owner_filtered = true;
        filters.owner = Some(owner.to_lowercase());
        filters.operated_positions = state.places.operated_positions(owner).await?;
    }
    let counts = state.live.live_user_counts().await;
    let most_active = matches!(filters.order_by, PlaceOrderBy::MostActive);
    let page = (filters.offset as usize, filters.limit as usize);
    let mut hot = None;
    if most_active {
        filters.active = if counts.snapshot {
            let positions: Vec<String> = counts
                .places
                .iter()
                .filter(|(_, n)| *n > 0)
                .map(|(position, _)| position.clone())
                .collect();
            hot = Some(positions.len());
            ActiveRows::Live(positions)
        } else {
            ActiveRows::Mirrored
        };
        filters.order_by = PlaceOrderBy::LikeScore;
        filters.offset = 0;
        filters.limit = 100;
    }
    let (mut data, mut total) = state.places.list_page(&filters).await?;
    live::fill_user_counts(&mut data, &counts);
    if most_active {
        (data, total) = busiest_first(data, page, filters.order_desc, hot);
    }
    let realms =
        (get("with_realms_detail") == Some("true")).then_some(state.settings.realm_name.as_str());
    for row in &mut data {
        row.apply_realms_detail(realms);
    }
    Ok(Json(ApiDataTotal::ok(data, total)))
}

pub const MAX_BATCH_ITEMS: usize = 100;
const MAX_WORLD_NAME_BYTES: usize = 255;

/// A dcl-one guard beyond upstream's schema (which caps the count at 1000):
/// a name longer than the column cannot match and is refused instead of bound.
fn validate_world_names(names: &[String]) -> Result<(), ApiError> {
    if names.iter().any(|name| name.len() > MAX_WORLD_NAME_BYTES) {
        return Err(ApiError::bad_request(
            "names accepts at most 1000 entries of 255 bytes each",
        ));
    }
    Ok(())
}

/// The JSON body of a batch read as express's `json()` parser left it for
/// the route: a body sent as `application/json` and parsing, else nothing
/// (`Null`), which the route refuses as "not an array". No request shape
/// reaches the framework's text 400/415/422.
pub(super) fn json_body(headers: &HeaderMap, body: &[u8]) -> serde_json::Value {
    let is_json = headers
        .get(axum::http::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .map(|v| {
            v.split(';')
                .next()
                .unwrap_or_default()
                .trim()
                .to_ascii_lowercase()
        })
        .is_some_and(|essence| {
            essence == "application/json"
                || (essence.starts_with("application/") && essence.ends_with("+json"))
        });
    if !is_json {
        return serde_json::Value::Null;
    }
    serde_json::from_slice(body).unwrap_or(serde_json::Value::Null)
}

/// The id array of the batch reads; `too_many` is the status upstream gave an
/// oversize list: 400 on `POST /api/places` (`getPlaceListById.ts:31-36`),
/// 404 on `POST /api/places/status` (`getPlaceStatusListById.ts:24-29`).
fn ids_body(ids: serde_json::Value, too_many: u16) -> Result<Vec<String>, ApiError> {
    let items = ids.as_array().ok_or_else(|| {
        ApiError::bad_request("Invalid request body. Expected an array of place IDs.")
    })?;
    if items.len() > MAX_BATCH_ITEMS {
        return Err(ApiError::http(
            too_many,
            "Cannot request more than 100 places at once",
        ));
    }
    Ok(items
        .iter()
        .filter_map(|v| v.as_str().map(|s| s.to_string()))
        .collect())
}

pub async fn post_place_list_by_id(
    State(state): State<SharedState>,
    method: Method,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
    Query(pairs): Query<Vec<(String, String)>>,
    body: axum::body::Bytes,
) -> Result<Json<ApiDataTotal<PlaceRow>>, ApiError> {
    let ids = ids_body(json_body(&headers, &body), 400)?;
    // getPlaceListById.ts:38-49: the paging and order query, validated after the body.
    let body = QueryBody::new(&pairs)
        .get("offset")
        .get("limit")
        .one_of("order_by", LIST_ORDER_BY, "like_score")
        .one_of("order", &["asc", "desc"], "desc")
        .get("search")
        .finish();
    validate::validate(&validate::PLACE_LIST_QUERY, &body, Flavour::Wkc)?;
    let user = state.viewer(&headers, method.as_str(), uri.path()).await;
    // getPlaceListById.ts:53-70: the ids under the normal listing (`disabled is
    // false`, the order, LIMIT <= 100; `ids` lifts `world is false`,
    // `Place/model.ts:150-156,208-211`), `total` from countByIds (`:579-592`),
    // which counts the disabled ids the page leaves out.
    let get = |k: &str| pairs.iter().find(|(p, _)| p == k).map(|(_, v)| v.as_str());
    let mut filters = PlaceListFilters {
        limit: paging(get("limit"), 100, 100),
        offset: paging(get("offset"), 0, i64::MAX),
        search: get("search").map(str::to_string),
        order_by: PlaceOrderBy::parse(get("order_by")),
        order_desc: !matches!(get("order"), Some("asc")),
        ids: ids.clone(),
        places_table: true,
        ..Default::default()
    };
    state
        .places
        .scope_to_viewer(&mut filters, user.as_deref(), false);
    let (mut data, _) = state.places.list_page(&filters).await?;
    let total = state.places.count_by_ids(&ids).await?;
    live::fill_user_counts(&mut data, &*state.live.live_user_counts().await);
    Ok(Json(ApiDataTotal::ok(data, total)))
}

/// gatsby `numeric(value, {min: 0, max})` on a validated `uint` string.
fn paging(value: Option<&str>, default: i64, max: i64) -> i64 {
    value
        .and_then(|s| s.parse::<i64>().ok())
        .unwrap_or(default)
        .clamp(0, max)
}

/// `getPlaceStatusListById.ts:15-37`: `findByIds` (no `disabled` filter) and
/// `countByIds`.
pub async fn post_place_status_list_by_id(
    State(state): State<SharedState>,
    headers: HeaderMap,
    body: axum::body::Bytes,
) -> Result<Json<ApiDataTotal<PlaceStatusRow>>, ApiError> {
    let ids = ids_body(json_body(&headers, &body), 404)?;
    let data = state.places.find_by_ids_status(&ids).await?;
    let total = state.places.count_by_ids(&ids).await?;
    Ok(Json(ApiDataTotal::ok(data, total)))
}

/// gatsby's `bool()` (`decentraland-gatsby` `Schema/utils.ts:1-22`): the
/// spellings of true in a query value.
pub fn gatsby_bool(value: &str) -> bool {
    matches!(value, "1" | "true" | "True" | "TRUE")
}

/// `with_realms_detail` as the discovery server reads it on the place detail
/// (`get-places-handler.ts:57`, `=== 'true'`) and as gatsby reads it on the
/// map routes (`bool()`).
pub fn with_realms_detail(pairs: &[(String, String)], gatsby: bool) -> bool {
    pairs.iter().any(|(k, v)| {
        k == "with_realms_detail" && if gatsby { gatsby_bool(v) } else { v == "true" }
    })
}

/// This node's realm name when the request asks for `realms_detail`.
fn realms_detail_for<'a>(
    state: &'a SharedState,
    pairs: &[(String, String)],
    gatsby: bool,
) -> Option<&'a str> {
    with_realms_detail(pairs, gatsby).then_some(state.settings.realm_name.as_str())
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "places/"))]
pub struct MapEntry {
    pub id: String,
    pub base_position: String,
    pub title: Option<String>,
    pub description: Option<String>,
    pub image: Option<String>,
    pub contact_name: Option<String>,
    pub categories: Vec<String>,
    pub user_favorite: bool,
    pub user_like: bool,
    pub user_dislike: bool,
    pub user_visits: i32,
    pub user_count: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional, type = "Array<Record<string, unknown>>"))]
    pub realms_detail: Option<Vec<serde_json::Value>>,
}

impl From<&PlaceRow> for MapEntry {
    fn from(p: &PlaceRow) -> Self {
        Self {
            id: p.id.clone(),
            base_position: p.base_position.clone(),
            title: p.title.clone(),
            description: p.description.clone(),
            image: p.image.clone(),
            contact_name: p.contact_name.clone(),
            categories: p.categories.clone(),
            user_favorite: p.user_favorite,
            user_like: p.user_like,
            user_dislike: p.user_dislike,
            user_visits: p.user_visits,
            user_count: p.user_count.unwrap_or(0),
            realms_detail: p.realms_detail.clone(),
        }
    }
}

/// The map queries as upstream built them: `GET /api/map` validates
/// `getPlaceListQuerySchema` over `getMapPlaces.ts:34-52` (most active:
/// `getMapPlacesMostActive.ts:22-36`); `GET /api/map/places` validates
/// `getAllPlacesListQuerySchema` (`Map/schemas.ts:88-177`) over
/// `getAllPlacesList.ts:35-58` (most active: `getAllPlacesMostActiveList.ts:22-36`).
fn validate_map_query(pairs: &[(String, String)], all_places: bool) -> Result<(), ApiError> {
    let most_active = pairs
        .iter()
        .any(|(k, v)| k == "order_by" && v == "most_active");
    let mut q = QueryBody::new(pairs)
        .get_all("positions")
        .get("offset")
        .get("limit")
        .get("only_favorites")
        .get("only_featured")
        .get("only_highlighted")
        .one_of("order", &["asc", "desc"], "desc")
        .get("with_realms_detail")
        .get("search")
        .get_all("categories");
    q = if most_active {
        q.fixed("order_by", "most_active")
    } else {
        q.one_of("order_by", LIST_ORDER_BY, "like_score")
    };
    if all_places {
        q = q.get_all("names");
        if !most_active {
            q = q.get_lower("creator_address").get("sdk");
        }
    }
    let schema = if all_places {
        &*validate::MAP_ALL_PLACES_QUERY
    } else {
        &*validate::PLACE_LIST_QUERY
    };
    validate::validate(schema, &q.finish(), Flavour::Wkc)
}

/// The filters of both map routes; `all_places` is `GET /api/map/places`,
/// which also reads `names` (`getAllPlacesList.ts:42`).
fn map_filters(pairs: &[(String, String)], all_places: bool) -> (PlaceListFilters, bool) {
    let get = |k: &str| pairs.iter().find(|(p, _)| p == k).map(|(_, v)| v.clone());
    let get_all = |k: &str| {
        pairs
            .iter()
            .filter(|(p, _)| p == k)
            .map(|(_, v)| v.clone())
            .collect::<Vec<_>>()
    };
    let only_favorites = get("only_favorites")
        .map(|v| matches!(v.as_str(), "true" | "1"))
        .unwrap_or(false);
    // places.decentraland.org answers 500 map entries by default and clamps there.
    let limit = get("limit")
        .and_then(|s| s.parse::<i64>().ok())
        .unwrap_or(500)
        .clamp(0, 500);
    let offset = get("offset")
        .and_then(|s| s.parse::<i64>().ok())
        .unwrap_or(0)
        .max(0);
    let positions = get_all("positions");
    let only_highlighted = get("only_highlighted")
        .map(|v| matches!(v.as_str(), "true" | "1"))
        .unwrap_or(false);
    let mut f = PlaceListFilters {
        limit,
        offset,
        categories: get_all("categories"),
        only_highlighted,
        search: get("search"),
        creator_address: get("creator_address").map(|s| s.to_lowercase()),
        sdk: get("sdk"),
        // `most_active` is its own route upstream (`getAllPlacesMostActiveList.ts`);
        // the list route orders by `Place/model.ts:1150-1156,1319-1325`, where
        // `user_visits` (or anything else) is `like_score`.
        order_by: PlaceOrderBy::parse_allowed(
            get("order_by").as_deref(),
            MAP_ORDER_BY,
            PlaceOrderBy::LikeScore,
        ),
        order_desc: !matches!(get("order").as_deref(), Some("asc")),
        places_table: true,
        ..Default::default()
    };
    if all_places {
        // Place/model.ts:1331-1354: `positions`, `names` (verbatim
        // `world_name IN`), either when both, `world is false` when neither.
        let names = get_all("names");
        f.only_places = positions.is_empty() && names.is_empty();
        f.exact_world_names = names;
    } else {
        // Place/model.ts:1215-1219: `world is false` unless `only_highlighted`;
        // categorized places only.
        f.only_places = !only_highlighted;
        f.only_categorized = true;
    }
    f.positions = positions;
    (f, only_favorites)
}

/// `GET /api/map`: the page keyed by `base_position` in the query's order, as
/// `placesWithCoordinatesAggregates` built it (`Map/utils.ts:6-46`); a JS
/// client reading the keys sees that order, so it is kept, not sorted.
struct MapPage {
    entries: Vec<MapEntry>,
    total: i64,
}

impl Serialize for MapPage {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        struct Entries<'a>(&'a [MapEntry]);
        impl Serialize for Entries<'_> {
            fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
                let mut map = serializer.serialize_map(Some(self.0.len()))?;
                for entry in self.0 {
                    map.serialize_entry(&entry.base_position, entry)?;
                }
                map.end()
            }
        }
        let mut body = serializer.serialize_map(Some(3))?;
        body.serialize_entry("ok", &true)?;
        body.serialize_entry("data", &Entries(&self.entries))?;
        body.serialize_entry("total", &self.total)?;
        body.end()
    }
}

pub async fn get_map_places(
    State(state): State<SharedState>,
    method: Method,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
    Query(pairs): Query<Vec<(String, String)>>,
) -> Result<Response, ApiError> {
    validate_map_query(&pairs, false)?;
    let (mut filters, only_favorites) = map_filters(&pairs, false);
    let user = state.viewer(&headers, method.as_str(), uri.path()).await;
    if !state
        .places
        .scope_to_viewer(&mut filters, user.as_deref(), only_favorites)
    {
        // getMapPlaces.ts:56-58: the anonymous favorites short-circuit answers
        // an array, not the keyed object (capture `places.get_map.only_favorites_anonymous`).
        return Ok(Json(ApiDataTotal::<MapEntry>::ok(vec![], 0)).into_response());
    }
    let counts = state.live.live_user_counts().await;
    live::apply_count_order(&mut filters, &counts);
    let (mut data, total) = state.places.list_page(&filters).await?;
    live::fill_user_counts(&mut data, &counts);
    let realms = realms_detail_for(&state, &pairs, true);
    let mut entries: Vec<MapEntry> = Vec::with_capacity(data.len());
    let mut slot_of: HashMap<String, usize> = HashMap::with_capacity(data.len());
    for place in &mut data {
        place.apply_realms_detail(realms);
        let entry = MapEntry::from(&*place);
        match slot_of.get(&entry.base_position) {
            Some(&slot) => entries[slot] = entry,
            None => {
                slot_of.insert(entry.base_position.clone(), entries.len());
                entries.push(entry);
            }
        }
    }
    Ok(Json(MapPage { entries, total }).into_response())
}

pub async fn get_all_places_list(
    State(state): State<SharedState>,
    method: Method,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
    Query(pairs): Query<Vec<(String, String)>>,
) -> Result<Json<ApiDataTotal<PlaceRow>>, ApiError> {
    validate_map_query(&pairs, true)?;
    let (mut filters, only_favorites) = map_filters(&pairs, true);
    let user = state.viewer(&headers, method.as_str(), uri.path()).await;
    if !state
        .places
        .scope_to_viewer(&mut filters, user.as_deref(), only_favorites)
    {
        return Ok(Json(ApiDataTotal::ok(vec![], 0)));
    }
    let counts = state.live.live_user_counts().await;
    live::apply_count_order(&mut filters, &counts);
    let (mut data, total) = state.places.list_page(&filters).await?;
    live::fill_user_counts(&mut data, &counts);
    let realms = realms_detail_for(&state, &pairs, true);
    for place in &mut data {
        place.apply_realms_detail(realms);
    }
    Ok(Json(ApiDataTotal::ok(data, total)))
}

/// `GET /api/worlds` as upstream built it (`World/routes/getWorldList.ts:27-47`),
/// The worlds list order: `most_active` is dcl-one's live sort (a superset:
/// upstream drops the value), the other listed values upstream's columns,
/// anything else, an absent value included, upstream's `like_score` fallback
/// (`World/model.ts:351-355`).
fn world_order_by(value: Option<&str>) -> PlaceOrderBy {
    PlaceOrderBy::parse_allowed(value, WORLD_ORDER_BY, PlaceOrderBy::LikeScore)
}

/// validated against `getWorldListQuerySchema` (`World/schemas.ts:5-81`).
/// Captures `places.get_worlds.invalid_limit`, `places.get_worlds.invalid_owner`.
fn validate_world_list_query(pairs: &[(String, String)]) -> Result<(), ApiError> {
    let body = QueryBody::new(pairs)
        .get_all("names")
        .get("offset")
        .get("limit")
        .get("only_favorites")
        .get("only_highlighted")
        .get("only_excluded_from_ranking")
        .one_of("order_by", WORLD_ORDER_BY, "most_active")
        .get("search")
        .one_of("order", &["asc", "desc"], "desc")
        .get_all("categories")
        .get_lower("owner")
        .finish();
    validate::validate(&validate::WORLD_LIST_QUERY, &body, Flavour::Wkc)
}

pub async fn get_world(
    State(state): State<SharedState>,
    method: Method,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
    Path(world_id): Path<String>,
) -> Result<Json<ApiData<WorldRow>>, ApiError> {
    let user = state.viewer(&headers, method.as_str(), uri.path()).await;
    // World/model.ts:333: `WHERE w.id = $1` on the worlds table, the parameter
    // verbatim, so an upper-case name or a world scene's id is a miss
    // (capture `places.get_world.upper_case_name`).
    match state
        .places
        .find_world_by_id_for(&world_id, user.as_deref())
        .await?
        .filter(|w| w.id == world_id && w.is_world_catalogue_row())
    {
        Some(mut w) => {
            live::fill_user_counts(
                std::slice::from_mut(&mut w),
                &*state.live.live_user_counts().await,
            );
            Ok(Json(ApiData::ok(WorldRow::from(w))))
        }
        None => Err(ApiError::not_found(format!(
            "Not found world \"{world_id}\""
        ))),
    }
}

pub async fn get_world_list(
    State(state): State<SharedState>,
    method: Method,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
    Query(pairs): Query<Vec<(String, String)>>,
) -> Result<Json<ApiDataTotal<WorldRow>>, ApiError> {
    validate_world_list_query(&pairs)?;
    let get = |k: &str| pairs.iter().find(|(p, _)| p == k).map(|(_, v)| v.clone());
    let get_all = |k: &str| {
        pairs
            .iter()
            .filter(|(p, _)| p == k)
            .map(|(_, v)| v.clone())
            .collect::<Vec<_>>()
    };
    let bool_q = |k: &str| get(k).is_some_and(|v| gatsby_bool(v.as_str()));
    let only_favorites = bool_q("only_favorites");
    let user = favorites_viewer(
        &state,
        &headers,
        method.as_str(),
        uri.path(),
        only_favorites,
    )
    .await?;
    let limit = get("limit")
        .and_then(|s| s.parse::<i64>().ok())
        .unwrap_or(100)
        .clamp(0, 100);
    let offset = get("offset")
        .and_then(|s| s.parse::<i64>().ok())
        .unwrap_or(0)
        .max(0);
    let names = get_all("names");
    validate_world_names(&names)?;
    // World/model.ts:369,115-120: `names` are the world ids, lowercased and
    // matched exactly (capture `places.get_worlds.names_exact`).
    let mut filters = PlaceListFilters {
        limit,
        offset,
        names,
        categories: get_all("categories"),
        search: get("search"),
        order_by: world_order_by(get("order_by").as_deref()),
        order_desc: !matches!(get("order").as_deref(), Some("asc")),
        only_highlighted: bool_q("only_highlighted"),
        only_excluded_from_ranking: bool_q("only_excluded_from_ranking"),
        only_worlds: true,
        owner: get("owner").map(|s| s.to_lowercase()),
        ..Default::default()
    };
    if !state
        .places
        .scope_to_viewer(&mut filters, user.as_deref(), only_favorites)
    {
        return Ok(Json(ApiDataTotal::ok(vec![], 0)));
    }
    let counts = state.live.live_user_counts().await;
    live::apply_count_order(&mut filters, &counts);
    let (mut data, total) = state.places.list_page(&filters).await?;
    live::fill_user_counts(&mut data, &counts);
    state.places.fill_world_place_fields(&mut data).await?;
    let worlds: Vec<WorldRow> = data.into_iter().map(WorldRow::from).collect();
    Ok(Json(ApiDataTotal::ok(worlds, total)))
}

pub async fn get_world_names_list(
    State(state): State<SharedState>,
) -> Result<Json<ApiDataTotal<String>>, ApiError> {
    let names = state.places.world_names().await?;
    let total = names.len() as i64;
    Ok(Json(ApiDataTotal::ok(names, total)))
}

/// Upstream's `categories` table, active rows in table order
/// (`migrations/1696876576229_update-categories-table.ts:7-30`, then
/// `1743671852197_add-parkour-category.ts:15-18`); `findActiveCategories`
/// reads them without an ORDER BY (`Category/model.ts:20-29`).
pub(crate) const CURATED_CATEGORIES: [&str; 14] = [
    "poi",
    "featured",
    "game",
    "casino",
    "social",
    "music",
    "art",
    "fashion",
    "crypto",
    "education",
    "shop",
    "sports",
    "business",
    "parkour",
];

pub(crate) fn category_i18n_en(name: &str) -> Option<&'static str> {
    Some(match name {
        "poi" => "\u{1F4CD} Point of Interest",
        "featured" => "\u{2728} Featured",
        "art" => "\u{1F3A8} Art",
        "game" => "\u{1F3AE} Game",
        "casino" => "\u{2663}\u{FE0F} Casino",
        "social" => "\u{1F465} Social",
        "music" => "\u{1F3B5} Music",
        "fashion" => "\u{1F460} Fashion",
        "crypto" => "\u{1FA99} Crypto",
        "education" => "\u{1F4DA} Education",
        "shop" => "\u{1F6CD}\u{FE0F} Shop",
        "business" => "\u{1F3E2} Business",
        "sports" => "\u{26BD}\u{FE0F} Sports",
        "parkour" => "\u{1F3C3} Parkour",
        _ => return None,
    })
}

/// `getCategoryList` (`Category/routes.ts:13-41`): every active category in
/// table order with its count, 0 when no place carries it; raw scene tags
/// that are not categories are dropped.
pub(crate) fn curated_counts(counts: Vec<(String, i64)>) -> Vec<(String, i64)> {
    let counts: HashMap<String, i64> = counts.into_iter().collect();
    CURATED_CATEGORIES
        .iter()
        .map(|name| (name.to_string(), counts.get(*name).copied().unwrap_or(0)))
        .collect()
}

/// `{name, count, i18n: {en}}` (`Category/routes.ts:29-38`).
#[derive(Debug, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "places/"))]
pub struct CategoryOut {
    pub name: String,
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub count: i64,
    pub i18n: I18n,
}

#[derive(Debug, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "places/"))]
pub struct I18n {
    pub en: Option<String>,
}

pub async fn get_categories(
    State(state): State<SharedState>,
    Query(pairs): Query<Vec<(String, String)>>,
) -> Result<Json<ApiData<Vec<CategoryOut>>>, ApiError> {
    let target = CategoryTarget::parse(
        pairs
            .iter()
            .find(|(k, _)| k == "target")
            .map(|(_, v)| v.as_str()),
    );
    let counts = curated_counts(state.places.category_counts(target).await?);
    let data = counts
        .into_iter()
        .map(|(name, count)| CategoryOut {
            i18n: I18n {
                en: category_i18n_en(&name).map(|s| s.to_string()),
            },
            count,
            name,
        })
        .collect();
    Ok(Json(ApiData::ok(data)))
}

#[derive(Debug, Serialize)]
pub struct PlaceCategoriesOut {
    pub categories: Vec<String>,
}

pub async fn get_place_categories(
    State(state): State<SharedState>,
    Path(place_id): Path<String>,
) -> Result<Json<ApiData<PlaceCategoriesOut>>, ApiError> {
    validate_place_id(&place_id)?;
    let categories = state.places.categories_for_place(&place_id).await?;
    Ok(Json(ApiData::ok(PlaceCategoriesOut { categories })))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::{Body, to_bytes};
    use axum::http::Request;
    use serde_json::{Value, json};
    use std::sync::Arc;
    use tower::ServiceExt;

    async fn response(state: SharedState, uri: &str) -> Value {
        let response = crate::router(state)
            .oneshot(Request::get(uri).body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status(), axum::http::StatusCode::OK);
        serde_json::from_slice(&to_bytes(response.into_body(), 1024 * 1024).await.unwrap()).unwrap()
    }

    /// `EXPLORE_OWNER_TIMING=1`: times `GET /api/places?owner=` for an owner
    /// whose LAND holding is large (`EXPLORE_OWNER_TIMING_PARCELS`, 100000)
    /// over a scaled catalogue (`EXPLORE_OWNER_TIMING_PLACES`, 10000) of which
    /// `EXPLORE_OWNER_TIMING_COVERED` (500) parcels carry a place. Prints
    /// p50/p95/max in microseconds; passes without the flag.
    #[tokio::test]
    async fn owner_filter_timing_over_a_large_holding() {
        if std::env::var("EXPLORE_OWNER_TIMING").is_err() {
            return;
        }
        let number = |key: &str, default: i64| {
            std::env::var(key)
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(default)
        };
        let (parcels, places, covered) = (
            number("EXPLORE_OWNER_TIMING_PARCELS", 100_000),
            number("EXPLORE_OWNER_TIMING_PLACES", 10_000),
            number("EXPLORE_OWNER_TIMING_COVERED", 500),
        );
        let Some(pool) = crate::db::scratch::pool(4).await else {
            return;
        };
        let owner = "0x00000000000000000000000000000000000000b1";
        // Places on a 300x300 grid; parcel g sits at the same pointer as place
        // g, so the first `covered` parcels have a place and the rest none.
        // Batched: the removal-capture trigger takes a lock per row.
        for from in (1..=places).step_by(1000) {
            sqlx::query(
                "INSERT INTO explore.place (id, base_position, title, disabled, raw) \
                 SELECT gen_random_uuid(), s.pos, 'Timing ' || s.g, false, \
                        jsonb_build_object('positions', jsonb_build_array(s.pos), 'owner', '0x00000000000000000000000000000000000000ee') \
                 FROM (SELECT g, ((g % 300) - 150)::text || ',' || ((g / 300) - 150)::text AS pos \
                       FROM generate_series($1::int, $2::int) g) s",
            )
            .bind(from as i32)
            .bind((from + 999).min(places) as i32)
            .execute(&pool)
            .await
            .unwrap();
        }
        let settings = crate::land_authz::Settings::for_tests();
        sqlx::query("INSERT INTO land_authz.index_cursor (network, land_registry, estate_registry, last_block, folded_block, folded_at) VALUES ($1, $2, $3, 100, 100, timezone('UTC', now())) ON CONFLICT (network) DO UPDATE SET folded_at = EXCLUDED.folded_at, folded_block = 100, last_block = 100")
            .bind(&settings.network).bind(&settings.land_registry).bind(&settings.estate_registry).execute(&pool).await.unwrap();
        sqlx::query(
            "INSERT INTO land_authz.token_right (token_address, token_id, x, y, owner, updated_block, updated_log) \
             SELECT $1, 5000000000 + g, CASE WHEN g <= $3 THEN (g % 300) - 150 ELSE 1000 + g END, \
                    CASE WHEN g <= $3 THEN (g / 300) - 150 ELSE 1000 END, $2, 100, 0 \
             FROM generate_series(1, $4::int) g ON CONFLICT DO NOTHING",
        )
        .bind(&settings.land_registry)
        .bind(owner)
        .bind(covered as i32)
        .bind(parcels as i32)
        .execute(&pool)
        .await
        .unwrap();
        let state = Arc::new(crate::ExploreState::new(
            pool.clone(),
            crate::Settings::for_tests(),
            None,
        ));
        state
            .places
            .set_land_authz(Arc::new(crate::land_authz::LandAuthzState::new(
                pool.clone(),
                crate::http::Outbound::new(Default::default()).unwrap(),
                settings,
                None,
            )));
        let n = 30;
        let mut samples = Vec::with_capacity(n);
        let mut total = 0;
        for i in 0..n + 3 {
            let start = std::time::Instant::now();
            let body = response(state.clone(), &format!("/api/places?owner={owner}")).await;
            let elapsed = start.elapsed().as_micros();
            total = body["total"].as_i64().unwrap();
            if i >= 3 {
                samples.push(elapsed);
            }
        }
        samples.sort_unstable();
        println!(
            "owner filter over {parcels} parcels, {places} places, {covered} covered: total {total}, p50 {} µs, p95 {} µs, max {} µs",
            samples[n / 2],
            samples[n * 95 / 100],
            samples[n - 1]
        );
        assert_eq!(total, covered.min(places));
    }

    #[tokio::test]
    async fn owner_pointer_opt_out_and_world_names_keep_their_independent_meanings() {
        let Some(pool) = crate::db::scratch::pool(4).await else {
            return;
        };
        let token = uuid::Uuid::new_v4().simple().to_string();
        let owner = format!("0x{token}00000000");
        let owned = uuid::Uuid::new_v4().to_string();
        let opted_out = uuid::Uuid::new_v4().to_string();
        let world = format!("{token}.dcl.eth");
        let local_world = format!("local-{token}.dcl.eth");
        for (id, pointer, disabled, raw) in [
            (
                &owned,
                "1,2",
                false,
                json!({"owner": owner, "positions": ["1,2"]}),
            ),
            (
                &opted_out,
                "3,4",
                true,
                json!({"owner": owner, "positions": ["3,4"], "disabled_reason": "opt_out"}),
            ),
            (
                &world,
                "0,0",
                false,
                json!({"world": true, "world_name": world, "owner": owner, "user_count": 99}),
            ),
        ] {
            sqlx::query("INSERT INTO explore.place (id, base_position, title, disabled, raw) VALUES ($1, $2, $3, $4, $5)")
                .bind(id).bind(pointer).bind(&token).bind(disabled).bind(raw).execute(&pool).await.unwrap();
        }
        // A world deployed here (world_scenes.rs) has no `owner` in its document;
        // its deployer is the served owner.
        sqlx::query("INSERT INTO explore.place (id, base_position, title, disabled, creator_address, raw) VALUES ($1, '0,0', $2, false, $3, $4)")
            .bind(&local_world).bind(&token).bind(&owner)
            .bind(json!({"world": true, "world_name": local_world}))
            .execute(&pool).await.unwrap();
        let state = Arc::new(crate::ExploreState::new(
            pool.clone(),
            crate::Settings::for_tests(),
            None,
        ));
        // Place/model.ts:186-188: the row's own owner matches without any
        // LAND index; the catalogue world is not a places-table row.
        let body = response(state.clone(), &format!("/api/places?owner={owner}")).await;
        assert_eq!(body["total"], 1, "{body}");
        assert_eq!(body["data"][0]["id"], owned);
        assert!(
            state
                .places
                .owns_places(std::slice::from_ref(&owned), &owner)
                .await
                .is_err(),
            "the owner column is not LAND authority for community writes"
        );
        let settings = crate::land_authz::Settings::for_tests();
        sqlx::query("INSERT INTO land_authz.index_cursor (network, land_registry, estate_registry, last_block, folded_block, folded_at) VALUES ($1, $2, $3, 100, 100, timezone('UTC', now())) ON CONFLICT (network) DO UPDATE SET folded_at = EXCLUDED.folded_at, folded_block = 100, last_block = 100")
            .bind(&settings.network).bind(&settings.land_registry).bind(&settings.estate_registry).execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO land_authz.token_right (token_address, token_id, x, y, owner, updated_block, updated_log) VALUES ($1, 340282366920938463463374607431768211458, 1, 2, $2, 100, 0), ($1, 1020847100762815390390123822295304634372, 3, 4, $2, 100, 0) ON CONFLICT (token_address, token_id) DO UPDATE SET owner = EXCLUDED.owner")
            .bind(&settings.land_registry).bind(&owner).execute(&pool).await.unwrap();
        state
            .places
            .set_land_authz(Arc::new(crate::land_authz::LandAuthzState::new(
                pool.clone(),
                crate::http::Outbound::new(Default::default()).unwrap(),
                settings,
                None,
            )));
        assert!(
            state
                .places
                .owns_places(std::slice::from_ref(&owned), &owner)
                .await
                .unwrap()
        );
        assert!(
            !state
                .places
                .owns_places(
                    std::slice::from_ref(&owned),
                    "0xffffffffffffffffffffffffffffffffffffffff"
                )
                .await
                .unwrap()
        );
        let body = response(state.clone(), &format!("/api/places?owner={owner}")).await;
        assert_eq!(body["data"].as_array().unwrap().len(), 1);
        assert_eq!(body["data"][0]["id"], owned);
        // `places-repository/component.ts:91-100`: another creator's place on
        // a parcel the owner operates (5,6), or holds through an estate (7,8),
        // is the owner's answer too; an owned parcel without a place (9,9)
        // and the whole holding never reach the list's clause.
        let operated = uuid::Uuid::new_v4().to_string();
        let estate_place = uuid::Uuid::new_v4().to_string();
        for (id, pointer) in [(&operated, "5,6"), (&estate_place, "7,8")] {
            sqlx::query("INSERT INTO explore.place (id, base_position, title, disabled, raw) VALUES ($1, $2, $3, false, $4)")
                .bind(id).bind(pointer).bind(&token)
                .bind(json!({"owner": "0x00000000000000000000000000000000000000ee", "positions": [pointer]}))
                .execute(&pool).await.unwrap();
        }
        let settings = crate::land_authz::Settings::for_tests();
        let estate_token = 700_000 + (uuid::Uuid::new_v4().as_u128() % 100_000) as i64;
        sqlx::query(
            "INSERT INTO land_authz.token_right (token_address, token_id, x, y, owner, update_operator, updated_block, updated_log) VALUES \
             ($1, 1701411834604692317316873037158841057286, 5, 6, '0x00000000000000000000000000000000000000ee', $2, 100, 0), \
             ($1, 2381976568446569244243622252022377480198, 7, 8, '0x00000000000000000000000000000000000000ee', NULL, 100, 0), \
             ($1, 3062541302288446171170371466885913903110, 9, 9, $2, NULL, 100, 0), \
             ($3, $4::numeric, NULL, NULL, $2, NULL, 100, 0) \
             ON CONFLICT (token_address, token_id) DO UPDATE SET owner = EXCLUDED.owner, update_operator = EXCLUDED.update_operator",
        )
        .bind(&settings.land_registry).bind(&owner).bind(&settings.estate_registry).bind(estate_token)
        .execute(&pool).await.unwrap();
        sqlx::query(
            "INSERT INTO land_authz.estate_land (land_token_id, estate_token_id, updated_block, updated_log) \
             VALUES (2381976568446569244243622252022377480198, $1::numeric, 100, 0) \
             ON CONFLICT (land_token_id) DO UPDATE SET estate_token_id = EXCLUDED.estate_token_id",
        )
        .bind(estate_token)
        .execute(&pool).await.unwrap();
        assert_eq!(
            state.places.operated_positions(&owner).await.unwrap(),
            vec!["1,2", "3,4", "5,6", "7,8"],
            "only pointers the catalogue holds"
        );
        let body = response(state.clone(), &format!("/api/places?owner={owner}")).await;
        let mut ids: Vec<String> = body["data"]
            .as_array()
            .unwrap()
            .iter()
            .map(|p| p["id"].as_str().unwrap().to_string())
            .collect();
        ids.sort();
        let mut expected = vec![owned.clone(), operated.clone(), estate_place.clone()];
        expected.sort();
        assert_eq!(ids, expected, "{body}");
        let body = response(
            state.clone(),
            &format!("/api/places?owner={owner}&positions=3,4"),
        )
        .await;
        assert_eq!(body["total"], 0);
        let body = response(state.clone(), &format!("/api/destinations?owner={owner}")).await;
        assert_eq!(
            body["data"].as_array().unwrap().len(),
            5,
            "the local world too"
        );
        // World/model.ts:115-120: `/api/worlds` names are exact ids ...
        let body = response(state.clone(), &format!("/api/worlds?names={token}")).await;
        assert_eq!(body["total"], 0, "a substring is not a world id");
        let upper = world.to_uppercase();
        let body = response(state.clone(), &format!("/api/worlds?names={upper}")).await;
        assert_eq!(body["data"][0]["id"], world, "lowercased before the match");
        let body = response(state.clone(), &format!("/api/worlds?owner={owner}")).await;
        let mut ids: Vec<String> = body["data"]
            .as_array()
            .unwrap()
            .iter()
            .map(|p| p["id"].as_str().unwrap().to_string())
            .collect();
        ids.sort();
        let mut expected = vec![world.clone(), local_world.clone()];
        expected.sort();
        assert_eq!(
            ids, expected,
            "the owner filter matches the served owner: {body}"
        );
        // ... while `/api/destinations` names are a LIKE (`:121-130`), escaped.
        let body = response(state.clone(), &format!("/api/destinations?names={token}")).await;
        assert_eq!(body["data"][0]["id"], world);
        let body = response(state.clone(), "/api/destinations?names=%25").await;
        assert_eq!(body["total"], 0, "percent is literal, not a wildcard");
        let body = response(state.clone(), "/api/destinations?names=%5F").await;
        assert_eq!(body["total"], 0, "underscore is literal, not a wildcard");
        let body = response(state, &format!("/api/destinations?world_names={token}")).await;
        assert_eq!(body["total"], 0);
    }

    async fn fetch(state: SharedState, uri: &str) -> (axum::http::StatusCode, String) {
        let response = crate::router(state)
            .oneshot(Request::get(uri).body(Body::empty()).unwrap())
            .await
            .unwrap();
        let status = response.status();
        let bytes = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
        (status, String::from_utf8(bytes.to_vec()).unwrap())
    }

    async fn post(state: SharedState, uri: &str, body: Value) -> Value {
        let response = crate::router(state)
            .oneshot(
                Request::post(uri)
                    .header("content-type", "application/json")
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), axum::http::StatusCode::CREATED);
        serde_json::from_slice(&to_bytes(response.into_body(), 1024 * 1024).await.unwrap()).unwrap()
    }

    fn ids(body: &Value) -> Vec<String> {
        body["data"]
            .as_array()
            .unwrap()
            .iter()
            .map(|d| d["id"].as_str().unwrap().to_string())
            .collect()
    }

    /// The places read routes against seeded genesis, disabled, catalogue-world
    /// and world-scene rows, each rule cited to the pinned upstream source
    /// (places e660fbc2); the harness fixtures `places.get_places.*`,
    /// `.post_places*`, `.get_place.*`, `.get_categories.*`, `.get_map*`,
    /// `.get_destinations.*`, `.post_destinations.valid_ids`, `.get_worlds.*`,
    /// `.get_world.upper_case_name` cover the same rules over the shared seed.
    #[tokio::test]
    async fn the_read_routes_follow_the_upstream_tables_and_fields() {
        let Some(pool) = crate::db::scratch::pool(4).await else {
            return;
        };
        let tag = format!("t{}", &uuid::Uuid::new_v4().simple().to_string()[..8]);
        let owner = "0x00000000000000000000000000000000000000aa";
        let plaza = uuid::Uuid::new_v4().to_string();
        let closed = uuid::Uuid::new_v4().to_string();
        let featured = uuid::Uuid::new_v4().to_string();
        let scene = uuid::Uuid::new_v4().to_string();
        let world = format!("{tag}.dcl.eth");
        let dark = format!("{tag}dark.dcl.eth");
        let (px, px2, pdis, pfeat) = ("-137,142", "-137,143", "-138,142", "-139,142");
        // (id, base_position, title, disabled, highlighted, categories, raw)
        type Seed<'a> = (&'a str, &'a str, String, bool, bool, Vec<&'a str>, Value);
        let rows: Vec<Seed<'_>> = vec![
            (
                plaza.as_str(),
                px,
                format!("{tag} Plaza"),
                false,
                false,
                vec!["art", "poi"],
                json!({"owner": owner, "positions": [px, px2], "textsearch": null, "sdk": "7"}),
            ),
            (
                closed.as_str(),
                pdis,
                format!("{tag} Closed"),
                true,
                false,
                vec!["art"],
                json!({"owner": owner, "positions": [pdis], "disabled_reason": "moderation"}),
            ),
            (
                featured.as_str(),
                pfeat,
                format!("{tag} Featured"),
                false,
                true,
                vec!["game"],
                json!({"owner": owner, "positions": [pfeat]}),
            ),
            (
                world.as_str(),
                "0,0",
                format!("{tag} World"),
                false,
                false,
                vec!["social"],
                json!({"world": true, "world_name": world, "owner": "0x00000000000000000000000000000000000000bb", "show_in_places": true, "image": "https://cdn.example/w.png", "base_position": "0,0"}),
            ),
            (
                dark.as_str(),
                "0,0",
                format!("{tag} Dark"),
                true,
                false,
                vec![],
                json!({"world": true, "world_name": dark, "show_in_places": true}),
            ),
            (
                scene.as_str(),
                "0,0",
                format!("{tag} Scene"),
                false,
                true,
                vec!["social"],
                json!({"world": true, "world_name": world, "world_id": world, "positions": ["0,0"], "contact_name": "Scenic", "contact_email": "s@example.org", "sdk": "7"}),
            ),
        ];
        // `world` and `world_name` are generated from `raw` (schemas/10_explore.sql).
        for (id, base, title, disabled, highlighted, categories, raw) in &rows {
            sqlx::query(
                "INSERT INTO explore.place (id, base_position, title, description, creator_address, categories, disabled, highlighted, deployed_at, raw) \
                 VALUES ($1, $2, $3, $3 || ' description', $4, $5, $6, $7, timezone('UTC', now()), $8)",
            )
            .bind(id)
            .bind(base)
            .bind(title)
            .bind(raw.get("owner").and_then(Value::as_str).unwrap_or(owner))
            .bind(categories.iter().map(|c| c.to_string()).collect::<Vec<_>>())
            .bind(disabled)
            .bind(highlighted)
            .bind(raw)
            .execute(&pool)
            .await
            .unwrap();
        }
        let state = Arc::new(crate::ExploreState::new(
            pool.clone(),
            crate::Settings::for_tests(),
            None,
        ));
        let get = |uri: String| {
            let state = state.clone();
            async move { response(state, &uri).await }
        };

        // GET /api/places: `p.*` carries textsearch (Place/types.ts:64), no rank
        // without a search; disabled rows are out (model.ts:144-148).
        let body = get(format!("/api/places?positions={px}&positions={pdis}")).await;
        assert_eq!(body["total"], 1, "{body}");
        assert_eq!(body["data"][0]["id"], plaza);
        assert!(
            body["data"][0]["textsearch"]
                .as_str()
                .is_some_and(|t| t.contains("plaza")),
            "{body}"
        );
        assert!(body["data"][0].get("rank").is_none());
        // names (`world_id IN`) and only_highlighted lift `world is false`
        // (model.ts:150-162): world scenes answer, catalogue worlds never do.
        let body = get(format!("/api/places?names={}", world.to_uppercase())).await;
        assert_eq!(ids(&body), vec![scene.clone()], "{body}");
        assert_eq!(body["data"][0]["world"], true);
        let body = get("/api/places?only_highlighted=true&limit=100".to_string()).await;
        let listed = ids(&body);
        assert!(
            listed.contains(&featured) && listed.contains(&scene) && !listed.contains(&world),
            "{listed:?}"
        );
        // owner: the row's own owner, lowercased (model.ts:186-188, getPlaceList.ts:60).
        let body = get(format!(
            "/api/places?owner={}&positions={px}",
            owner.to_uppercase()
        ))
        .await;
        assert_eq!(ids(&body), vec![plaza.clone()], "{body}");
        // search: ts_rank_cd on textsearch, `rank > 0`, rank selected (model.ts:171,305,323-328).
        let body = get(format!("/api/places?search={tag}%20plaz")).await;
        assert_eq!(ids(&body), vec![plaza.clone()], "{body}");
        assert!(
            body["data"][0]["rank"].as_f64().is_some_and(|r| r > 0.0),
            "{body}"
        );
        let body = get(format!("/api/places?search={tag}zzz")).await;
        assert_eq!(body["total"], 0);

        // POST /api/places: the listing over the ids, total = countByIds
        // (getPlaceListById.ts:53-70, model.ts:579-592).
        let body = post(
            state.clone(),
            "/api/places?order_by=created_at&order=asc",
            json!([plaza, closed]),
        )
        .await;
        assert_eq!(ids(&body), vec![plaza.clone()]);
        assert_eq!(body["total"], 2, "{body}");
        let body = post(state.clone(), "/api/places?limit=0", json!([plaza])).await;
        assert_eq!(
            (
                body["data"].as_array().unwrap().len(),
                body["total"].as_i64()
            ),
            (0, Some(1))
        );
        // POST /api/places/status: findByIds with disabled_reason (model.ts:556-577).
        let body = post(state.clone(), "/api/places/status", json!([plaza, closed])).await;
        assert_eq!(body["total"], 2);
        let status: Vec<(String, bool, Value)> = body["data"]
            .as_array()
            .unwrap()
            .iter()
            .map(|d| {
                (
                    d["id"].as_str().unwrap().to_string(),
                    d["disabled"].as_bool().unwrap(),
                    d["disabled_reason"].clone(),
                )
            })
            .collect();
        assert!(
            status.contains(&(plaza.clone(), false, Value::Null)),
            "{status:?}"
        );
        assert!(
            status.contains(&(closed.clone(), true, json!("moderation"))),
            "{status:?}"
        );

        // GET /api/places/:id: ids verbatim (model.ts:549), textsearch present.
        let (status, _) = fetch(
            state.clone(),
            &format!("/api/places/{}", plaza.to_uppercase()),
        )
        .await;
        assert_eq!(status, axum::http::StatusCode::NOT_FOUND);
        let body = get(format!("/api/places/{plaza}")).await;
        assert!(body["data"].get("textsearch").is_some(), "{body}");
        let body = get(format!("/api/places/{closed}")).await;
        assert_eq!(body["data"]["disabled_reason"], "moderation");

        // GET /api/categories: every active category in table order, zero
        // counts shown, counts over disabled rows too, no `active`
        // (Category/routes.ts:13-41, Category/model.ts:34-51).
        let body = get("/api/categories".to_string()).await;
        let names: Vec<&str> = body["data"]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| c["name"].as_str().unwrap())
            .collect();
        assert_eq!(names, CURATED_CATEGORIES.to_vec());
        assert!(body["data"][0].get("active").is_none(), "{body}");
        let count = |body: &Value, name: &str| {
            body["data"]
                .as_array()
                .unwrap()
                .iter()
                .find(|c| c["name"] == name)
                .unwrap()["count"]
                .as_i64()
                .unwrap()
        };
        assert!(
            count(&body, "art") >= 2,
            "the disabled place counts: {body}"
        );
        let places = get("/api/categories?target=places".to_string()).await;
        assert!(
            count(&places, "art") >= 2 && count(&places, "social") <= count(&body, "social") - 2,
            "{places}"
        );
        let worlds = get("/api/categories?target=worlds".to_string()).await;
        assert!(count(&worlds, "social") >= 2, "{worlds}");

        // GET /api/map: `[]` for anonymous favorites (getMapPlaces.ts:56-58);
        // otherwise keyed by base_position in query order.
        let (_, text) = fetch(state.clone(), "/api/map?only_favorites=true").await;
        assert_eq!(text, r#"{"ok":true,"data":[],"total":0}"#);
        let body = get(format!("/api/map?positions={px2}")).await;
        assert_eq!(body["data"][px]["id"], plaza, "{body}");
        // GET /api/map/places: `world is false` by default, names verbatim,
        // positions OR names (Place/model.ts:1331-1354).
        let body = get(format!("/api/map/places?names={world}")).await;
        assert_eq!(ids(&body), vec![scene.clone()], "{body}");
        let body = get(format!("/api/map/places?names={}", world.to_uppercase())).await;
        assert_eq!(body["total"], 0, "{body}");
        let body = get(format!("/api/map/places?names={world}&positions={px}")).await;
        let both = ids(&body);
        assert!(
            both.contains(&plaza) && both.contains(&scene) && both.len() == 2,
            "{both:?}"
        );
        let body = get("/api/map/places?limit=500".to_string()).await;
        assert!(
            body["data"]
                .as_array()
                .unwrap()
                .iter()
                .all(|d| d["world"] == false),
            "{body}"
        );

        // GET /api/destinations: the union of genesis places and catalogue
        // worlds (Destination/model.ts:234-304); a world row carries
        // upstream's constants and the latest scene's fields (:39-49), and
        // none of dcl-one's keys.
        let body = get(format!("/api/destinations?names={tag}&only_worlds=true")).await;
        assert_eq!(ids(&body), vec![world.clone()], "{body}");
        let w = &body["data"][0];
        assert_eq!(
            (
                w["base_position"].as_str(),
                w["positions"].as_array().map(Vec::len)
            ),
            (Some("0,0"), Some(0))
        );
        assert_eq!(
            (w["contact_name"].as_str(), w["sdk"].as_str()),
            (Some("Scenic"), Some("7"))
        );
        assert!(
            w.get("kind").is_none() && w.get("live_event").is_none(),
            "{w}"
        );
        let body = get(format!(
            "/api/destinations?only_places=true&only_worlds=true&pointer={px}"
        ))
        .await;
        assert_eq!(
            ids(&body),
            vec![plaza.clone()],
            "only_places wins (model.ts:234): {body}"
        );
        let body = get(format!(
            "/api/destinations?world_names={}&only_worlds=true",
            world.to_uppercase()
        ))
        .await;
        assert_eq!(ids(&body), vec![world.clone()], "{body}");
        let body = get(format!("/api/destinations?search={tag}%20scene")).await;
        assert_eq!(
            body["total"], 0,
            "a world scene is not a destination: {body}"
        );
        // POST /api/destinations: the ids under the GET query (getDestinationsListById.ts:54-137).
        let body = post(state.clone(), "/api/destinations", json!([plaza, world])).await;
        let mut both = ids(&body);
        both.sort();
        let mut expected = vec![plaza.clone(), world.clone()];
        expected.sort();
        assert_eq!((both, body["total"].as_i64()), (expected, Some(2)));
        let body = post(
            state.clone(),
            "/api/destinations?only_worlds=true",
            json!([plaza]),
        )
        .await;
        assert_eq!(body["total"], 0);
        let body = post(
            state.clone(),
            "/api/destinations?limit=1&order_by=created_at",
            json!([plaza, world]),
        )
        .await;
        assert_eq!(
            (
                body["data"].as_array().unwrap().len(),
                body["total"].as_i64()
            ),
            (1, Some(2))
        );
        // /v1/destinations keeps dcl-one's keys.
        let body = get(format!("/v1/destinations?names={tag}&only_worlds=true")).await;
        assert_eq!(body["data"][0]["kind"], "world");

        // GET /api/worlds: exact lowercased names (World/model.ts:115-120,369);
        // GET /api/worlds/:id verbatim (:333); world_names enabled only (:434-443).
        let body = get(format!("/api/worlds?names={}", world.to_uppercase())).await;
        assert_eq!(ids(&body), vec![world.clone()], "{body}");
        assert_eq!(body["data"][0]["contact_name"], "Scenic");
        let (status, _) = fetch(
            state.clone(),
            &format!("/api/worlds/{}", world.to_uppercase()),
        )
        .await;
        assert_eq!(status, axum::http::StatusCode::NOT_FOUND);
        let (status, _) = fetch(state.clone(), &format!("/api/worlds/{scene}")).await;
        assert_eq!(
            status,
            axum::http::StatusCode::NOT_FOUND,
            "a scene is not a world"
        );
        let body = get(format!("/api/worlds/{world}")).await;
        assert_eq!(
            (
                body["data"]["base_position"].as_str(),
                body["data"]["contact_name"].as_str()
            ),
            (Some("0,0"), Some("Scenic"))
        );
        let body = get("/api/world_names".to_string()).await;
        let names: Vec<&str> = body["data"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(Value::as_str)
            .collect();
        assert!(
            names.contains(&world.as_str()) && !names.contains(&dark.as_str()),
            "{names:?}"
        );
    }

    #[test]
    fn world_name_patterns_are_bounded() {
        assert!(validate_world_names(&["x".repeat(255)]).is_ok());
        assert!(validate_world_names(&["x".repeat(256)]).is_err());
        let many: Vec<(String, String)> = (0..1001)
            .map(|_| ("names".to_string(), "name".to_string()))
            .collect();
        assert!(
            validate_world_list_query(&many).is_err(),
            "schema caps names at 1000"
        );
        assert!(validate_world_list_query(&many[..1000]).is_ok());
    }

    /// The world query upstream validated, and the 400s it answers
    /// (captures `places.get_worlds.*`).
    #[test]
    fn world_queries_validate_like_upstream() {
        let line = |err: ApiError| match err {
            ApiError::HttpData {
                status,
                message,
                data,
            } => (status, message, data),
            other => panic!("{other:?}"),
        };
        let (_, message, _) =
            line(validate_world_list_query(&pairs(&[("limit", "abc")])).unwrap_err());
        assert!(message.starts_with("Error validating input:\n- limit must match format \"uint\""));
        let (_, message, data) =
            line(validate_world_list_query(&pairs(&[("owner", "bad")])).unwrap_err());
        assert!(
            message.contains("owner must match pattern \"^0x[a-fA-F0-9]{40}$\""),
            "{message}"
        );
        assert_eq!(data["body"]["order_by"], "most_active");
        assert!(
            validate_world_list_query(&pairs(&[("names", "a"), ("order_by", "user_visits")]))
                .is_ok()
        );

        assert!(
            validate_map_query(&pairs(&[("positions", "0,0"), ("positions", "1,1")]), false)
                .is_ok()
        );
        assert!(validate_map_query(&pairs(&[("positions", "x")]), false).is_err());
        assert!(validate_map_query(&pairs(&[("creator_address", "nope")]), true).is_err());
        assert!(
            validate_map_query(
                &pairs(&[("creator_address", "nope"), ("order_by", "most_active")]),
                true
            )
            .is_ok(),
            "the most-active handler never reads creator_address"
        );
        assert!(
            validate_map_query(
                &pairs(&[("names", "metadynelabs.dcl.eth"), ("positions", "0,0")]),
                true
            )
            .is_ok()
        );

        assert!(validate_place_id("3ca25728-5b41-48f6-8e24-5cb0e3f2bb5d").is_ok());
        let (status, message, data) = line(validate_place_id("not-a-uuid").unwrap_err());
        assert_eq!(status, 400);
        assert_eq!(
            message,
            "Error validating input:\n- place_id must match format \"uuid\" {\"format\":\"uuid\"}"
        );
        assert_eq!(data["body"], json!({"place_id": "not-a-uuid"}));
    }

    /// `POST /api/places` caps at 100 with a 400, `POST /api/places/status`
    /// with upstream's 404 (`getPlaceStatusListById.ts:24-29`).
    #[test]
    fn batch_ids_follow_upstream_limits_and_statuses() {
        let ids: Vec<Value> = (0..101).map(|i| json!(i.to_string())).collect();
        assert_eq!(
            ids_body(Value::Array(ids.clone()), 400)
                .unwrap_err()
                .status(),
            400
        );
        assert_eq!(ids_body(Value::Array(ids), 404).unwrap_err().status(), 404);
        assert_eq!(
            ids_body(json!(["a", 7, "b"]), 400).unwrap(),
            vec!["a".to_string(), "b".to_string()]
        );
        let err = ids_body(json!({"ids": []}), 400).unwrap_err();
        assert_eq!(
            err.to_string(),
            "Invalid request body. Expected an array of place IDs."
        );
    }

    /// Case-insensitive world ids: upstream reads `WHERE w.id = $1` verbatim
    /// (`World/model.ts:333`), so `PATCH /api/worlds/FOO.dcl.eth/favorites`
    /// and the admin PUTs miss a mixed-case id; dcl-one's resolvers fold the
    /// name on purpose, as a superset. `GET /api/worlds/:id` keeps the
    /// verbatim rule on top (`get_world`).
    #[tokio::test]
    async fn world_resolvers_fold_mixed_case_ids_as_a_documented_superset() {
        let Some(pool) = crate::db::scratch::pool(2).await else {
            return;
        };
        let name = format!("wp10-{}.dcl.eth", uuid::Uuid::new_v4().simple());
        let mixed = name.to_uppercase();
        sqlx::query("INSERT INTO explore.place (id, base_position, title, raw) VALUES ($1, '0,0', 'Folded world', $2)")
            .bind(&name)
            .bind(json!({"world": true, "world_name": name}))
            .execute(&pool)
            .await
            .unwrap();
        let places = super::super::PlacesComponent::new(pool.clone(), "/content");
        let found = places
            .find_world_by_id_for(&mixed, None)
            .await
            .unwrap()
            .expect("the interaction routes' lookup folds the id");
        assert_eq!(found.id, name);
        assert!(found.is_world_catalogue_row(), "{found:?}");
        let resolved = super::super::moderation::resolve(
            &pool,
            &super::super::moderation::Target::World(&mixed),
        )
        .await
        .unwrap()
        .expect("the admin routes' resolver folds the id");
        assert_eq!(resolved.id, name);

        // `World/model.ts:333` reads the worlds table: a world known only
        // through its scenes takes no write.
        let scene_only = format!("wp11-{}.dcl.eth", uuid::Uuid::new_v4().simple());
        sqlx::query("INSERT INTO explore.place (id, base_position, title, raw) VALUES ($1, '1,1', 'A scene of a world without a catalogue row', $2)")
            .bind(uuid::Uuid::new_v4().to_string())
            .bind(json!({"world": true, "world_name": scene_only}))
            .execute(&pool)
            .await
            .unwrap();
        let found = places
            .find_world_by_id_for(&scene_only, None)
            .await
            .unwrap();
        assert!(
            found.is_some_and(|row| !row.is_world_catalogue_row()),
            "the lookup finds the scene row, which the write routes refuse"
        );
        let resolved = super::super::moderation::resolve(
            &pool,
            &super::super::moderation::Target::World(&scene_only),
        )
        .await
        .unwrap();
        assert!(resolved.is_none(), "{resolved:?}");
    }

    fn pairs(kvs: &[(&str, &str)]) -> Vec<(String, String)> {
        kvs.iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    #[test]
    fn with_realms_detail_follows_each_oracle() {
        let p = |v: &str| pairs(&[("with_realms_detail", v)]);
        assert!(with_realms_detail(&p("true"), false));
        assert!(
            !with_realms_detail(&p("1"), false),
            "the discovery server: `=== 'true'`"
        );
        assert!(with_realms_detail(&p("1"), true) && with_realms_detail(&p("True"), true));
        assert!(!with_realms_detail(&p("false"), true) && !with_realms_detail(&p("0"), true));
        assert!(!with_realms_detail(&pairs(&[]), true));
    }

    /// A value outside a route's `oneOf` is the
    /// route default (`Place/model.ts:1150-1156,1319-1325`,
    /// `World/model.ts:351-355`), never a column another route knows.
    #[test]
    fn order_by_outside_a_routes_enum_falls_back_to_its_default() {
        for all_places in [false, true] {
            let order = |value: &str| {
                map_filters(&pairs(&[("order_by", value)]), all_places)
                    .0
                    .order_by
            };
            assert_eq!(order("user_visits"), PlaceOrderBy::LikeScore);
            assert_eq!(order("nope"), PlaceOrderBy::LikeScore);
            assert_eq!(order("updated_at"), PlaceOrderBy::UpdatedAt);
            assert_eq!(order("created_at"), PlaceOrderBy::CreatedAt);
            assert_eq!(order("most_active"), PlaceOrderBy::MostActive);
            assert_eq!(
                map_filters(&pairs(&[]), all_places).0.order_by,
                PlaceOrderBy::LikeScore
            );
        }
        assert_eq!(world_order_by(None), PlaceOrderBy::LikeScore);
        assert_eq!(
            world_order_by(Some("most_active")),
            PlaceOrderBy::MostActive
        );
        assert_eq!(world_order_by(Some("like_score")), PlaceOrderBy::LikeScore);
        assert_eq!(world_order_by(Some("created_at")), PlaceOrderBy::CreatedAt);
        assert_eq!(world_order_by(Some("updated_at")), PlaceOrderBy::LikeScore);
        assert_eq!(world_order_by(Some("user_visits")), PlaceOrderBy::LikeScore);
        // `get-places-handler.ts:10-16`: `/api/places` knows four orders.
        let list_order = |value: &str| {
            list_filters(&pairs(&[("order_by", value)]))
                .unwrap()
                .order_by
        };
        assert_eq!(list_order("user_visits"), PlaceOrderBy::LikeScore);
        assert_eq!(list_order("most_active"), PlaceOrderBy::MostActive);
    }

    /// `get-places-handler.ts:12-34` and `query-params.ts:12-41`.
    #[test]
    fn the_list_query_reads_like_the_discovery_server() {
        let f = list_filters(&pairs(&[
            ("names", "Foo.dcl.eth, Bar.dcl.eth,"),
            ("names", ""),
            ("sdk", "7"),
            ("creator_address", "0xABC"),
            ("only_highlighted", "true"),
            ("positions", "1,2"),
            ("categories", "art,poi"),
            ("limit", "500"),
            ("offset", "2.0"),
        ]))
        .unwrap();
        assert_eq!(f.names, vec!["Foo.dcl.eth", "Bar.dcl.eth"]);
        assert_eq!(f.categories, vec!["art", "poi"]);
        assert_eq!(f.sdk.as_deref(), Some("7"));
        assert_eq!(f.creator_address.as_deref(), Some("0xabc"));
        assert!(f.only_highlighted);
        assert!(f.positions.is_empty(), "names turn positions off");
        assert_eq!((f.limit, f.offset), (100, 2));
        // `places-repository/component.ts:65-67`: names and only_highlighted lift `world IS false`.
        assert!(!f.only_places && f.places_table && !f.only_worlds);

        let default = list_filters(&pairs(&[
            ("limit", "abc"),
            ("offset", "-1"),
            ("search", ""),
            ("sdk", ""),
            ("only_highlighted", "1"),
            ("order", "ASC"),
        ]))
        .unwrap();
        assert_eq!((default.limit, default.offset), (100, 0));
        assert!(default.search.is_none() && default.sdk.is_none());
        assert!(!default.only_highlighted && default.only_places && default.order_desc);
        assert!(
            !list_filters(&pairs(&[("order", "asc")]))
                .unwrap()
                .order_desc
        );

        let f = list_filters(&pairs(&[
            ("positions", "1,2"),
            ("positions", ""),
            ("search", "ab"),
        ]))
        .unwrap();
        assert_eq!(f.positions, vec!["1,2"]);
        assert_eq!(f.search.as_deref(), Some("ab"));

        let many = (0..1001).map(|_| "a").collect::<Vec<_>>().join(",");
        let err = list_filters(&pairs(&[("categories", &many)])).unwrap_err();
        assert!(
            matches!(&err, ApiError::Http { status: 400, message } if message == "Too many categories (max 1000)"),
            "{err:?}"
        );
        let thousand = (0..1000).map(|_| ("positions", "0,0")).collect::<Vec<_>>();
        assert!(list_filters(&pairs(&thousand)).is_ok());
        let mut over = thousand;
        over.push(("positions", "0,0"));
        assert!(list_filters(&pairs(&over)).is_err());
    }

    /// `intParam` (`query-params.ts:27-32`): JavaScript `Number`, kept when a
    /// non-negative integer.
    #[test]
    fn most_active_pages_the_busiest_rows_first() {
        let row = |like: f64, users: i32| PlaceRow {
            like_score: Some(like),
            user_count: Some(users),
            ..Default::default()
        };
        let set = || vec![row(9.0, 1), row(1.0, 1), row(5.0, 3), row(2.0, 0)];
        let likes = |rows: &[PlaceRow]| rows.iter().map(|r| r.like_score).collect::<Vec<_>>();
        let (rows, total) = busiest_first(set(), (0, 10), true, Some(3));
        assert_eq!(
            (likes(&rows), total),
            (vec![Some(5.0), Some(9.0), Some(1.0), Some(2.0)], 4)
        );
        let (page, total) = busiest_first(set(), (1, 1), true, Some(3));
        assert_eq!((page[0].like_score, page.len(), total), (Some(9.0), 1, 4));
        // `getPlaceMostActiveList.ts:51-56`: three places have players, so
        // offset 3 still pages the set and offset 4 answers nothing.
        let (page, total) = busiest_first(set(), (3, 1), true, Some(3));
        assert_eq!((page[0].like_score, total), (Some(2.0), 4));
        let (page, total) = busiest_first(set(), (4, 1), true, Some(3));
        assert_eq!((page.len(), total), (0, 0));
        // `order=asc`: the emptiest first, the stored order still breaking ties.
        let (rows, total) = busiest_first(set(), (0, 10), false, Some(3));
        assert_eq!(
            (likes(&rows), total),
            (vec![Some(2.0), Some(9.0), Some(1.0), Some(5.0)], 4)
        );
        // A search kept one of the three places with players: page 2 of the
        // filtered set still pages it, as `offset` is held to `hotScenes.length`.
        let filtered = || vec![row(5.0, 3), row(2.0, 0), row(1.0, 0)];
        let (page, total) = busiest_first(filtered(), (2, 1), true, Some(3));
        assert_eq!((page[0].like_score, page.len(), total), (Some(1.0), 1, 3));
        // Without a presence snapshot the mirrored headcounts of the set are
        // the only hot list there is.
        let (page, total) = busiest_first(filtered(), (2, 1), true, None);
        assert_eq!((page.len(), total), (0, 0));
    }

    #[test]
    fn the_list_numbers_read_like_javascript() {
        for (raw, want) in [
            ("10", Some(10)),
            (" 7 ", Some(7)),
            ("1.0", Some(1)),
            ("1e1", Some(10)),
            ("0x10", Some(16)),
            ("-0", Some(0)),
            ("", None),
            ("abc", None),
            ("-1", None),
            ("1.5", None),
            ("Infinity", None),
            ("0x", None),
            ("0x+1", None),
        ] {
            assert_eq!(js_uint(raw), want, "{raw:?}");
        }
    }

    /// `Category/routes.ts:13-41`: every active category in table order,
    /// zero counts included (captures `places.get_categories.*`); raw scene
    /// tags are not categories.
    #[test]
    fn the_category_list_keeps_curated_names_in_order_and_drops_raw_scene_tags() {
        let counts = vec![
            ("art".to_string(), 889),
            ("arcade".to_string(), 40),
            ("game".to_string(), 509),
            ("atlas:*".to_string(), 12),
            ("authoritative-server".to_string(), 7),
            ("parkour".to_string(), 14),
        ];
        let curated = curated_counts(counts);
        assert_eq!(curated.len(), CURATED_CATEGORIES.len());
        let names: Vec<&str> = curated.iter().map(|(n, _)| n.as_str()).collect();
        assert_eq!(names, CURATED_CATEGORIES.to_vec());
        assert_eq!(curated[0], ("poi".to_string(), 0));
        assert_eq!(curated[2], ("game".to_string(), 509));
        assert_eq!(curated[6], ("art".to_string(), 889));
        assert_eq!(curated[13], ("parkour".to_string(), 14));
        for name in CURATED_CATEGORIES {
            assert!(category_i18n_en(name).is_some(), "{name} must stay listed");
        }
        let out = CategoryOut {
            name: "poi".into(),
            count: 0,
            i18n: I18n {
                en: category_i18n_en("poi").map(str::to_string),
            },
        };
        let v = serde_json::to_value(out).unwrap();
        let mut keys: Vec<&String> = v.as_object().unwrap().keys().collect();
        keys.sort();
        assert_eq!(
            keys,
            vec!["count", "i18n", "name"],
            "no `active` key (Category/routes.ts:29-38)"
        );
    }

    #[test]
    fn the_map_page_keeps_the_query_order_of_its_keys() {
        let mut first = super::super::rows::sample_row();
        first.base_position = "9,9".into();
        let mut second = super::super::rows::sample_row();
        second.base_position = "-1,3".into();
        let page = MapPage {
            entries: vec![MapEntry::from(&first), MapEntry::from(&second)],
            total: 2,
        };
        let text = serde_json::to_string(&page).unwrap();
        assert!(
            text.starts_with("{\"ok\":true,\"data\":{\"9,9\":"),
            "{text}"
        );
        assert!(text.find("\"9,9\"").unwrap() < text.find("\"-1,3\"").unwrap());
        assert!(text.ends_with(",\"total\":2}"), "{text}");
    }

    #[test]
    fn a_map_entry_reads_the_row_and_an_unknown_count_is_zero() {
        let mut row = super::super::rows::sample_row();
        row.user_count = None;
        row.realms_detail = Some(vec![]);
        let entry = MapEntry::from(&row);
        assert_eq!(entry.user_count, 0);
        assert_eq!(entry.base_position, "1,2");
        assert_eq!(
            serde_json::to_value(&entry).unwrap()["realms_detail"],
            serde_json::json!([])
        );
    }
    // The client signFetch + personalSign shape, for the fixture's own wallet.
    async fn hud_client_headers(
        owner: &alloy_signer_local::PrivateKeySigner,
        path: &str,
        timestamp: &str,
    ) -> HeaderMap {
        use alloy_signer::Signer;
        use alloy_signer_local::PrivateKeySigner;
        use axum::http::{HeaderName, HeaderValue};
        let ephemeral: PrivateKeySigner = format!("0x{}", "11".repeat(32)).parse().unwrap();
        let delegation = format!(
            "Decentraland Login\nEphemeral address: {:#x}\nExpiration: 2099-01-01T00:00:00.000Z",
            ephemeral.address()
        );
        let delegation_signature = owner
            .sign_message(delegation.as_bytes())
            .await
            .unwrap()
            .to_string();
        let payload = format!("GET:{path}:{timestamp}:{{}}").to_lowercase();
        let signature = ephemeral
            .sign_message(payload.as_bytes())
            .await
            .unwrap()
            .to_string();
        let links = [
            serde_json::json!({"type":"SIGNER","payload":format!("{:#x}",owner.address()),"signature":""}),
            serde_json::json!({"type":"ECDSA_EPHEMERAL","payload":delegation,"signature":delegation_signature}),
            serde_json::json!({"type":"ECDSA_SIGNED_ENTITY","payload":payload,"signature":signature}),
        ];
        let mut headers = HeaderMap::new();
        headers.insert(
            "x-identity-timestamp",
            HeaderValue::from_str(timestamp).unwrap(),
        );
        headers.insert("x-identity-metadata", HeaderValue::from_static("{}"));
        for (index, link) in links.iter().enumerate() {
            headers.insert(
                HeaderName::from_bytes(format!("x-identity-auth-chain-{index}").as_bytes())
                    .unwrap(),
                HeaderValue::from_str(&link.to_string()).unwrap(),
            );
        }
        headers
    }

    #[tokio::test]
    async fn hud_favorites_signature_matches_the_actual_client_vector() {
        let owner = crate::shared::signed_fetch::test_support::KEY
            .parse()
            .unwrap();
        let headers = hud_client_headers(&owner, "/places/api/destinations", "1793217600000").await;
        let delegation: serde_json::Value =
            serde_json::from_str(headers["x-identity-auth-chain-1"].to_str().unwrap()).unwrap();
        assert_eq!(
            delegation["signature"],
            "0x9163f5d66c4554b3c3a4c0027a37ec781b89726320742ab6ffac9e368927d5e46642c1c14669675237fe96b6064d072e7f34352a8f61e2c827c3044a386728891c"
        );
        let link: serde_json::Value =
            serde_json::from_str(headers["x-identity-auth-chain-2"].to_str().unwrap()).unwrap();
        assert_eq!(
            link["payload"],
            "get:/places/api/destinations:1793217600000:{}"
        );
        assert_eq!(
            link["signature"],
            "0x30ad979298e9191c99c1f8de0580beefa79f73cd878f1524b494a59f074a90147dbf2912bda2ced98dd8f8efe9fb4ebcfee52105018c847b9db670c3a71d529b1b"
        );
    }

    /// Full gateway path verifies through nesting; a stripping proxy needs its
    /// original-path header. Failed authentication must not mimic an empty wallet.
    #[tokio::test]
    async fn hud_signed_favorites_roundtrip_preserves_gateway_path_and_reports_auth_failure() {
        use alloy_signer_local::PrivateKeySigner;
        use axum::{
            Router,
            body::{Body, to_bytes},
            http::{Request, StatusCode},
        };
        use std::sync::Arc;
        use tower::ServiceExt;
        let pool = crate::db::scratch::pool(3)
            .await
            .expect("signed favorites roundtrip requires CATALYST_TEST_DATABASE_URL");
        let state = Arc::new(crate::ExploreState::new(
            pool.clone(),
            crate::Settings::for_tests(),
            None,
        ));
        let local = crate::router(state);
        let prefixed = Router::new().nest("/places", local.clone());
        let owner = PrivateKeySigner::random();
        let user = format!("{:#x}", owner.address());
        // a catalogue world is keyed by its lowercased name
        for (id, position, world) in [
            ("favorite-native", "23,24", false),
            ("favorite-other", "25,26", false),
            ("favorite-world.dcl.eth", "27,28", true),
        ] {
            let name = world.then_some("favorite-world.dcl.eth");
            sqlx::query("INSERT INTO explore.place (id,base_position,title,highlighted,raw) VALUES ($1,$2,$1,true,$3)")
                .bind(id).bind(position).bind(serde_json::json!({"image":"https://native.example/cover.png","owner":user,"positions":[position],"world":world,"world_name":name}))
                .execute(&pool).await.unwrap();
        }
        sqlx::query("INSERT INTO explore.user_favorites (\"user\",entity_id) VALUES ($1,'favorite-native'),($1,'favorite-world.dcl.eth'),('0xother','favorite-other')")
            .bind(&user).execute(&pool).await.unwrap();
        let timestamp = chrono::Utc::now().timestamp_millis().to_string();
        for (endpoint, expected_ids) in [
            (
                "api/destinations",
                vec!["favorite-native", "favorite-world.dcl.eth"],
            ),
            (
                "v1/destinations",
                vec!["favorite-native", "favorite-world.dcl.eth"],
            ),
            ("api/places", vec!["favorite-native"]),
            ("api/worlds", vec!["favorite-world.dcl.eth"]),
        ] {
            let signed_path = format!("/places/{endpoint}");
            let gateway_uri = format!("{signed_path}?only_favorites=true");
            let local_uri = format!("/{endpoint}?only_favorites=true");
            let signed = hud_client_headers(&owner, &signed_path, &timestamp).await;
            for (app, uri, original) in [
                (&prefixed, gateway_uri.as_str(), None),
                (&local, local_uri.as_str(), Some(gateway_uri.as_str())),
            ] {
                let mut request = Request::builder().uri(uri).body(Body::empty()).unwrap();
                *request.headers_mut() = signed.clone();
                if let Some(original) = original {
                    request
                        .headers_mut()
                        .insert("x-original-path", original.parse().unwrap());
                }
                let response = app.clone().oneshot(request).await.unwrap();
                assert_eq!(response.status(), StatusCode::OK, "{endpoint}");
                let body: serde_json::Value = serde_json::from_slice(
                    &to_bytes(response.into_body(), 1024 * 1024).await.unwrap(),
                )
                .unwrap();
                let mut ids: Vec<_> = body["data"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|row| {
                        assert_eq!(row["user_favorite"], true, "{endpoint}: {row}");
                        row["id"].as_str().unwrap()
                    })
                    .collect();
                ids.sort_unstable();
                assert_eq!(ids, expected_ids, "{endpoint}: {body}");
                assert_eq!(body["total"], expected_ids.len(), "{endpoint}: {body}");
            }
            let mut corrupt = signed.clone();
            let mut link: serde_json::Value =
                serde_json::from_str(corrupt["x-identity-auth-chain-2"].to_str().unwrap()).unwrap();
            link["signature"] = serde_json::json!(format!("0x{}", "00".repeat(65)));
            corrupt.insert("x-identity-auth-chain-2", link.to_string().parse().unwrap());
            for headers in [
                corrupt.clone(),
                hud_client_headers(&owner, &format!("/{endpoint}"), &timestamp).await,
                hud_client_headers(
                    &owner,
                    &format!("/{}", endpoint.rsplit('/').next().unwrap()),
                    &timestamp,
                )
                .await,
                hud_client_headers(
                    &owner,
                    &signed_path,
                    &(chrono::Utc::now().timestamp_millis() - 600_000).to_string(),
                )
                .await,
                HeaderMap::from_iter([(
                    axum::http::HeaderName::from_static("x-identity-timestamp"),
                    "broken".parse().unwrap(),
                )]),
            ] {
                let mut request = Request::builder()
                    .uri(&gateway_uri)
                    .body(Body::empty())
                    .unwrap();
                *request.headers_mut() = headers;
                let response = prefixed.clone().oneshot(request).await.unwrap();
                assert_eq!(response.status(), StatusCode::UNAUTHORIZED, "{endpoint}");
            }
            let response = prefixed
                .clone()
                .oneshot(
                    Request::builder()
                        .uri(&gateway_uri)
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::OK, "{endpoint}");
            let body: serde_json::Value =
                serde_json::from_slice(&to_bytes(response.into_body(), 1024).await.unwrap())
                    .unwrap();
            assert_eq!(body["total"], 0, "{endpoint}: {body}");
            assert_eq!(body["data"], serde_json::json!([]), "{endpoint}: {body}");
            // Optional viewer authentication remains anonymous for public reads.
            let mut request = Request::builder()
                .uri(&signed_path)
                .body(Body::empty())
                .unwrap();
            *request.headers_mut() = corrupt;
            assert_eq!(
                prefixed.clone().oneshot(request).await.unwrap().status(),
                StatusCode::OK,
                "{endpoint}"
            );
        }
    }
}
