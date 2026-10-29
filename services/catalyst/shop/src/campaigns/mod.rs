// from: shop@94584a5c:app/src/lib/campaign.ts (the campaign's collections by Builder tag), app/src/lib/contentful.ts (the ADMIN entry: the seasonal campaign and its banner slots); marketplace@f3956826:webapp/src/modules/event/{actions,sagas}.ts (FETCH_EVENT), webapp/src/modules/vendor/decentraland/builder/api.ts (fetchAddressesByTag: GET /v1/addresses?tag=, the {ok, data: string[]} envelope)
//! Campaigns: the seasonal storefront (`/shop/event`, the Overview hero and
//! the campaign tab) and the collections it selects. Upstream read two
//! services for it: the marketing CMS (Contentful) for the campaign entry
//! and its banners, and the Builder for the collections carrying the
//! campaign's tags (`GET /v1/addresses?tag=`). Neither is a Shop authority
//! v2 has: the Builder here keeps no collection tags, and a CMS is not
//! configured on every node. So the Shop owns the campaign as data
//! (`shop.campaigns`: one row per campaign, its window, its tags and the
//! banner content; `shop.campaign_collections`: the approved collections a
//! tag selects), written by the operator behind the market's admin bearer
//! (`MARKET_ADMIN_TOKEN`) and `DATA_WRITES_ENABLED`, read publicly:
//!
//! | route | answer |
//! | --- | --- |
//! | `GET /v1/campaigns/active` | the campaign whose window contains now (`data: null` when none: an empty campaign is not a filter) |
//! | `GET /v1/campaigns/{id}` | one campaign, window or not |
//! | `GET /v1/campaign-collections?tag=` | the Builder's `{ok, data: [address]}`: the collections of every tag named, deduplicated; also served as the Builder alias `GET /builder-api/v1/addresses` |
//! | `PUT`/`DELETE /admin/campaigns/{id}` | the operator upserts or removes a campaign |
//! | `PUT /admin/campaigns/{id}/collections` | the operator sets the collections of the campaign's main tag, each an approved collection of the mirror |
//!
//! A campaign's collections feed the catalog's own `contractAddress`
//! selection (`GET /v3/catalog/unified?contractAddress=`), which fails
//! closed when the set is empty; nothing here filters the catalog itself.

use axum::Json;
use axum::Router;
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, put};
use chrono::{NaiveDateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::{PgPool, Row};

use super::market::admin::require_admin;
use super::market::mirrors::{Mirror, require_ready};
use super::market::wire::ApiError;
use super::market::{MarketplaceState, SharedState, is_valid_eth_address};

pub const MAX_TAGS: usize = 16;
pub const MAX_COLLECTIONS_PER_TAG: usize = 120;
pub const MAX_BANNER_BYTES: usize = 32 * 1024;

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CampaignCollection {
    pub tag: String,
    pub network: String,
    pub contract_address: String,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Campaign {
    pub id: String,
    pub name: String,
    pub tab_name: Option<String>,
    pub main_tag: String,
    pub tags: Vec<String>,
    /// Milliseconds; `None` = open-ended.
    pub starts_at: Option<i64>,
    pub ends_at: Option<i64>,
    pub enabled: bool,
    /// Whether the window contains now and the campaign is enabled.
    pub active: bool,
    /// The banner content as the operator stored it (the CMS fields the
    /// upstream `CampaignBanner` reads; opaque here).
    pub banner: Value,
    pub collections: Vec<CampaignCollection>,
    pub updated_at: i64,
}

fn ms(t: Option<NaiveDateTime>) -> Option<i64> {
    t.map(|t| t.and_utc().timestamp_millis())
}

fn active_now(
    enabled: bool,
    starts_at: Option<NaiveDateTime>,
    ends_at: Option<NaiveDateTime>,
    now: NaiveDateTime,
) -> bool {
    enabled && starts_at.is_none_or(|s| s <= now) && ends_at.is_none_or(|e| now < e)
}

const CAMPAIGN_COLUMNS: &str =
    "id, name, tab_name, main_tag, tags, starts_at, ends_at, enabled, banner, updated_at";

fn campaign_row(r: &sqlx::postgres::PgRow, now: NaiveDateTime) -> Campaign {
    let starts_at: Option<NaiveDateTime> = r.get("starts_at");
    let ends_at: Option<NaiveDateTime> = r.get("ends_at");
    let enabled: bool = r.get("enabled");
    Campaign {
        id: r.get("id"),
        name: r.get("name"),
        tab_name: r.get("tab_name"),
        main_tag: r.get("main_tag"),
        tags: r.get("tags"),
        starts_at: ms(starts_at),
        ends_at: ms(ends_at),
        enabled,
        active: active_now(enabled, starts_at, ends_at, now),
        banner: r.get("banner"),
        collections: Vec::new(),
        updated_at: r
            .get::<NaiveDateTime, _>("updated_at")
            .and_utc()
            .timestamp_millis(),
    }
}

async fn with_collections(pool: &PgPool, mut campaign: Campaign) -> Result<Campaign, ApiError> {
    let mut tags = campaign.tags.clone();
    if !tags.contains(&campaign.main_tag) {
        tags.insert(0, campaign.main_tag.clone());
    }
    let rows = sqlx::query("SELECT tag, network, contract_address FROM shop.campaign_collections WHERE tag = ANY($1) ORDER BY tag, network, contract_address")
        .bind(&tags)
        .fetch_all(pool)
        .await?;
    campaign.collections = rows
        .iter()
        .map(|r| CampaignCollection {
            tag: r.get("tag"),
            network: r.get("network"),
            contract_address: r.get("contract_address"),
        })
        .collect();
    Ok(campaign)
}

pub async fn get_campaign(pool: &PgPool, id: &str) -> Result<Option<Campaign>, ApiError> {
    let now = Utc::now().naive_utc();
    let row = sqlx::query(sqlx::AssertSqlSafe(format!(
        "SELECT {CAMPAIGN_COLUMNS} FROM shop.campaigns WHERE id = $1"
    )))
    .bind(id)
    .fetch_optional(pool)
    .await?;
    match row {
        Some(r) => Ok(Some(with_collections(pool, campaign_row(&r, now)).await?)),
        None => Ok(None),
    }
}

/// The enabled campaign whose window contains now; the most recently
/// updated when several overlap (one storefront at a time).
pub async fn active_campaign(pool: &PgPool) -> Result<Option<Campaign>, ApiError> {
    let now = Utc::now().naive_utc();
    let row = sqlx::query(sqlx::AssertSqlSafe(format!(
        "SELECT {CAMPAIGN_COLUMNS} FROM shop.campaigns WHERE enabled AND (starts_at IS NULL OR starts_at <= $1) AND (ends_at IS NULL OR $1 < ends_at) ORDER BY updated_at DESC LIMIT 1"
    )))
    .bind(now)
    .fetch_optional(pool)
    .await?;
    match row {
        Some(r) => Ok(Some(with_collections(pool, campaign_row(&r, now)).await?)),
        None => Ok(None),
    }
}

/// The Builder's answer: every collection of any of the tags, once each,
/// in address order.
pub async fn collections_by_tags(pool: &PgPool, tags: &[String]) -> Result<Vec<String>, ApiError> {
    if tags.is_empty() {
        return Ok(Vec::new());
    }
    let rows: Vec<(String,)> = sqlx::query_as("SELECT DISTINCT contract_address FROM shop.campaign_collections WHERE tag = ANY($1) ORDER BY contract_address").bind(tags).fetch_all(pool).await?;
    Ok(rows.into_iter().map(|(a,)| a).collect())
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CampaignUpsert {
    pub name: String,
    #[serde(default)]
    pub tab_name: Option<String>,
    pub main_tag: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub starts_at: Option<i64>,
    #[serde(default)]
    pub ends_at: Option<i64>,
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub banner: Option<Value>,
}

pub fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 64
        && id
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        && !id.starts_with('-')
}

pub fn valid_tag(tag: &str) -> bool {
    !tag.is_empty()
        && tag.len() <= 64
        && tag
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_' || b == b'.')
}

fn ms_to_naive(v: Option<i64>, field: &str) -> Result<Option<NaiveDateTime>, ApiError> {
    match v {
        None => Ok(None),
        Some(ms) => chrono::DateTime::from_timestamp_millis(ms)
            .map(|t| Some(t.naive_utc()))
            .ok_or_else(|| ApiError::bad_request(format!("{field} must be epoch milliseconds"))),
    }
}

pub fn validate_upsert(body: &CampaignUpsert) -> Result<(), ApiError> {
    if body.name.trim().is_empty() || body.name.chars().count() > 120 {
        return Err(ApiError::bad_request(
            "name must be between 1 and 120 characters",
        ));
    }
    if !valid_tag(&body.main_tag) {
        return Err(ApiError::bad_request(
            "mainTag must be a tag (letters, digits, - _ .)",
        ));
    }
    if body.tags.len() > MAX_TAGS || body.tags.iter().any(|t| !valid_tag(t)) {
        return Err(ApiError::bad_request(format!(
            "tags must be at most {MAX_TAGS} tags"
        )));
    }
    if let (Some(s), Some(e)) = (body.starts_at, body.ends_at)
        && s >= e
    {
        return Err(ApiError::bad_request("startsAt must be before endsAt"));
    }
    if let Some(banner) = &body.banner {
        if !banner.is_object() {
            return Err(ApiError::bad_request("banner must be an object"));
        }
        if serde_json::to_vec(banner)
            .map(|b| b.len())
            .unwrap_or(usize::MAX)
            > MAX_BANNER_BYTES
        {
            return Err(ApiError::bad_request(format!(
                "banner must be at most {MAX_BANNER_BYTES} bytes"
            )));
        }
    }
    Ok(())
}

pub async fn upsert_campaign(
    pool: &PgPool,
    id: &str,
    body: &CampaignUpsert,
    actor: &str,
) -> Result<Campaign, ApiError> {
    validate_upsert(body)?;
    let starts_at = ms_to_naive(body.starts_at, "startsAt")?;
    let ends_at = ms_to_naive(body.ends_at, "endsAt")?;
    let mut tags = body.tags.clone();
    tags.dedup();
    sqlx::query(
        "INSERT INTO shop.campaigns (id, name, tab_name, main_tag, tags, starts_at, ends_at, enabled, banner, updated_by, updated_at) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, timezone('UTC', now())) \
         ON CONFLICT (id) DO UPDATE SET name = EXCLUDED.name, tab_name = EXCLUDED.tab_name, main_tag = EXCLUDED.main_tag, tags = EXCLUDED.tags, \
             starts_at = EXCLUDED.starts_at, ends_at = EXCLUDED.ends_at, enabled = EXCLUDED.enabled, banner = EXCLUDED.banner, \
             updated_by = EXCLUDED.updated_by, updated_at = timezone('UTC', now())",
    )
    .bind(id)
    .bind(body.name.trim())
    .bind(body.tab_name.as_deref().map(str::trim).filter(|s| !s.is_empty()))
    .bind(&body.main_tag)
    .bind(&tags)
    .bind(starts_at)
    .bind(ends_at)
    .bind(body.enabled)
    .bind(body.banner.clone().unwrap_or_else(|| json!({})))
    .bind(actor)
    .execute(pool)
    .await?;
    get_campaign(pool, id)
        .await?
        .ok_or_else(|| ApiError::internal("the campaign vanished after its upsert"))
}

pub async fn delete_campaign(pool: &PgPool, id: &str) -> Result<bool, ApiError> {
    Ok(sqlx::query("DELETE FROM shop.campaigns WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await?
        .rows_affected()
        > 0)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CollectionsBody {
    pub contracts: Vec<String>,
}

/// Replaces the tag's set with the approved collections named, in one
/// transaction; a contract the mirror does not hold as an approved
/// collection is a 400 naming it (a campaign never points at a collection
/// the catalog cannot show).
pub async fn set_tag_collections(
    pool: &PgPool,
    tag: &str,
    contracts: &[String],
    actor: &str,
) -> Result<Vec<CampaignCollection>, ApiError> {
    if contracts.len() > MAX_COLLECTIONS_PER_TAG {
        return Err(ApiError::bad_request(format!(
            "at most {MAX_COLLECTIONS_PER_TAG} collections per tag"
        )));
    }
    let mut wanted: Vec<String> = contracts.iter().map(|c| c.trim().to_lowercase()).collect();
    wanted.sort();
    wanted.dedup();
    if let Some(bad) = wanted.iter().find(|c| !is_valid_eth_address(c)) {
        return Err(ApiError::bad_request(format!(
            "{bad} is not a contract address"
        )));
    }
    let approved: Vec<(String, String)> = sqlx::query_as("SELECT contract_address, network FROM shop.collections WHERE contract_address = ANY($1) AND is_approved ORDER BY contract_address, network")
        .bind(&wanted)
        .fetch_all(pool)
        .await?;
    if let Some(missing) = wanted
        .iter()
        .find(|c| !approved.iter().any(|(a, _)| a == *c))
    {
        return Err(ApiError::bad_request(format!(
            "{missing} is not an approved collection in this node's mirror"
        )));
    }
    let (addresses, networks): (Vec<String>, Vec<String>) = approved.iter().cloned().unzip();
    let mut tx = pool.begin().await?;
    sqlx::query("DELETE FROM shop.campaign_collections WHERE tag = $1")
        .bind(tag)
        .execute(&mut *tx)
        .await?;
    sqlx::query(
        "INSERT INTO shop.campaign_collections (tag, network, contract_address, added_by) \
         SELECT $1, n, a, $4 FROM UNNEST($2::text[], $3::text[]) AS s(a, n) ON CONFLICT DO NOTHING",
    )
    .bind(tag)
    .bind(&addresses)
    .bind(&networks)
    .bind(actor)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(approved
        .into_iter()
        .map(|(contract_address, network)| CampaignCollection {
            tag: tag.to_string(),
            network,
            contract_address,
        })
        .collect())
}

fn ok<T: Serialize>(data: T) -> Response {
    Json(json!({ "ok": true, "data": data })).into_response()
}

/// `GET /v1/campaigns/active`.
pub async fn get_active(State(state): State<SharedState>) -> Response {
    match active_campaign(&state.pool).await {
        Ok(c) => (
            [(axum::http::header::CACHE_CONTROL, "public, max-age=60")],
            ok(c),
        )
            .into_response(),
        Err(e) => e.into_response(),
    }
}

/// `GET /v1/campaigns/{id}`.
pub async fn get_one(State(state): State<SharedState>, Path(id): Path<String>) -> Response {
    if !valid_id(&id) {
        return ApiError::not_found("campaign not found").into_response();
    }
    match get_campaign(&state.pool, &id).await {
        Ok(Some(c)) => ok(c),
        Ok(None) => ApiError::not_found("campaign not found").into_response(),
        Err(e) => e.into_response(),
    }
}

/// `GET /v1/campaign-collections?tag=a&tag=b` and the Builder alias
/// `GET /builder-api/v1/addresses?tag=`: `{ok, data: [address]}`, an empty
/// list for a tag nobody set (upstream's Builder answered the same).
pub async fn get_collections_by_tag(
    State(state): State<SharedState>,
    Query(pairs): Query<Vec<(String, String)>>,
) -> Response {
    let tags: Vec<String> = pairs
        .iter()
        .filter(|(k, _)| k == "tag")
        .map(|(_, v)| v.trim().to_string())
        .filter(|t| valid_tag(t))
        .take(MAX_TAGS)
        .collect();
    if tags.is_empty() {
        return ApiError::bad_request("at least one tag query parameter is required")
            .into_response();
    }
    match collections_by_tags(&state.pool, &tags).await {
        Ok(addresses) => (
            [(axum::http::header::CACHE_CONTROL, "public, max-age=60")],
            ok(addresses),
        )
            .into_response(),
        Err(e) => e.into_response(),
    }
}

async fn admin(state: &MarketplaceState, headers: &HeaderMap) -> Result<String, Response> {
    require_admin(state, headers)
}

/// `PUT /admin/campaigns/{id}` (operator bearer, behind the write switch).
pub async fn put_campaign(
    State(state): State<SharedState>,
    Path(id): Path<String>,
    headers: HeaderMap,
    body: Option<Json<CampaignUpsert>>,
) -> Response {
    let actor = match admin(&state, &headers).await {
        Ok(a) => a,
        Err(r) => return r,
    };
    if !valid_id(&id) {
        return ApiError::bad_request(
            "the campaign id is a lowercase slug of at most 64 characters",
        )
        .into_response();
    }
    let Some(Json(body)) = body else {
        return ApiError::bad_request("a campaign body is required").into_response();
    };
    match upsert_campaign(&state.pool, &id, &body, &actor).await {
        Ok(c) => ok(c),
        Err(e) => e.into_response(),
    }
}

/// `DELETE /admin/campaigns/{id}`.
pub async fn delete_campaign_route(
    State(state): State<SharedState>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Response {
    if let Err(r) = admin(&state, &headers).await {
        return r;
    }
    match delete_campaign(&state.pool, &id).await {
        Ok(true) => ok(Value::Null),
        Ok(false) => ApiError::not_found("campaign not found").into_response(),
        Err(e) => e.into_response(),
    }
}

/// `PUT /admin/campaigns/{id}/collections`: the campaign's main tag gets
/// exactly the approved collections in `contracts`.
pub async fn put_collections(
    State(state): State<SharedState>,
    Path(id): Path<String>,
    headers: HeaderMap,
    body: Option<Json<CollectionsBody>>,
) -> Response {
    let actor = match admin(&state, &headers).await {
        Ok(a) => a,
        Err(r) => return r,
    };
    if let Err(r) = require_ready(&state, Mirror::Collections).await {
        return r;
    }
    let campaign = match get_campaign(&state.pool, &id).await {
        Ok(Some(c)) => c,
        Ok(None) => return ApiError::not_found("campaign not found").into_response(),
        Err(e) => return e.into_response(),
    };
    let Some(Json(body)) = body else {
        return ApiError::bad_request("contracts is required").into_response();
    };
    match set_tag_collections(&state.pool, &campaign.main_tag, &body.contracts, &actor).await {
        Ok(rows) => (StatusCode::OK, ok(rows)).into_response(),
        Err(e) => e.into_response(),
    }
}

/// The public routes, for `market::market_router` (both mounts).
pub fn router() -> Router<SharedState> {
    Router::new()
        .route("/v1/campaigns/active", get(get_active))
        .route("/v1/campaigns/{id}", get(get_one))
        .route("/v1/campaign-collections", get(get_collections_by_tag))
        .route(
            "/v1/admin/campaigns/{id}",
            put(put_campaign).delete(delete_campaign_route),
        )
        .route("/v1/admin/campaigns/{id}/collections", put(put_collections))
}

/// The operator routes under the canonical `/shop-api/admin` (`market::shop_admin_router`).
pub fn admin_router() -> Router<SharedState> {
    Router::new()
        .route(
            "/admin/campaigns/{id}",
            put(put_campaign).delete(delete_campaign_route),
        )
        .route("/admin/campaigns/{id}/collections", put(put_collections))
}

/// `GET /builder-api/v1/addresses?tag=`: the Builder path the upstream
/// campaign code calls, answered by the Shop's campaign collections
/// (`main.rs` merges it beside the Builder's nest).
pub fn builder_alias_router(state: SharedState) -> Router {
    Router::new()
        .route("/builder-api/v1/addresses", get(get_collections_by_tag))
        .with_state(state)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_tags_and_windows_are_validated() {
        assert!(
            valid_id("summer-2026")
                && valid_id("x")
                && !valid_id("")
                && !valid_id("-a")
                && !valid_id("Summer")
                && !valid_id(&"a".repeat(65))
        );
        assert!(
            valid_tag("summer_2026")
                && valid_tag("dcl.event")
                && !valid_tag("")
                && !valid_tag("a b")
        );
        let body: CampaignUpsert = serde_json::from_value(
            json!({ "name": "Summer", "mainTag": "summer", "startsAt": 10, "endsAt": 5 }),
        )
        .unwrap();
        assert!(
            validate_upsert(&body).is_err(),
            "a window that ends before it starts"
        );
        let body: CampaignUpsert = serde_json::from_value(
            json!({ "name": "Summer", "mainTag": "summer", "tags": ["a"], "banner": [] }),
        )
        .unwrap();
        assert!(validate_upsert(&body).is_err(), "a banner is an object");
        let body: CampaignUpsert = serde_json::from_value(
            json!({ "name": " Summer ", "mainTag": "summer", "enabled": true }),
        )
        .unwrap();
        assert!(validate_upsert(&body).is_ok());
        assert!(!body.tags.iter().any(|t| t.is_empty()));
    }

    #[test]
    fn the_window_decides_active() {
        let now = chrono::DateTime::from_timestamp(1_700_000_000, 0)
            .unwrap()
            .naive_utc();
        let before = chrono::DateTime::from_timestamp(1_600_000_000, 0)
            .unwrap()
            .naive_utc();
        let after = chrono::DateTime::from_timestamp(1_800_000_000, 0)
            .unwrap()
            .naive_utc();
        assert!(active_now(true, None, None, now));
        assert!(active_now(true, Some(before), Some(after), now));
        assert!(!active_now(false, Some(before), Some(after), now));
        assert!(!active_now(true, Some(after), None, now));
        assert!(!active_now(true, None, Some(before), now));
        assert!(
            !active_now(true, None, Some(now), now),
            "the end is exclusive"
        );
    }

    #[test]
    fn the_wire_is_camel_case_with_the_builder_envelope_for_addresses() {
        let c = Campaign {
            id: "summer".into(),
            name: "Summer".into(),
            tab_name: None,
            main_tag: "summer".into(),
            tags: vec!["summer".into()],
            starts_at: None,
            ends_at: Some(5),
            enabled: true,
            active: true,
            banner: json!({ "desktopTitle": "Hi" }),
            collections: vec![CampaignCollection {
                tag: "summer".into(),
                network: "matic".into(),
                contract_address: "0xabc".into(),
            }],
            updated_at: 1,
        };
        let v = serde_json::to_value(&c).unwrap();
        assert_eq!(v["mainTag"], "summer");
        assert_eq!(v["collections"][0]["contractAddress"], "0xabc");
        assert!(v["tabName"].is_null());
    }
}
