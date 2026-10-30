//! The favorites: the lists a wallet keeps (`GET`/`POST /v1/lists`,
//! `GET`/`PUT`/`DELETE /v1/lists/{id}`), their picks (`GET`/`POST
//! /v1/lists/{id}/picks`, `DELETE /v1/lists/{id}/picks/{item_id}`,
//! `POST`/`DELETE /v1/picks/{item_id}`), their sharing (`POST`/`DELETE
//! /v1/lists/{id}/access`: a `view` or `edit` grant to one wallet or to
//! everyone, `*`) and the per-item reads (`GET /v1/picks/{item_id}`, who
//! picked it; `GET /v1/picks/{item_id}/stats` and `GET /v1/picks/stats`)
//! over the local `shop.lists`, `picks` and `list_acl` (the old
//! `favorites` schema). Local authority: the writes sit behind
//! `DATA_WRITES_ENABLED` (`market::DataWrites`, applied by the router).
//! The item-exists check of a pick reads the `items` mirror (the old
//! `squid.item` probe); the statement invariants (a pick lands only in a
//! list the caller owns, the shared Wishlist or a list with an `edit`
//! grant; every picks read is scoped to the caller; `is_private` derives
//! from the ACL; a list is read by its owner, by a wallet the ACL names or
//! by everyone when `*` is granted, never otherwise; the shared default
//! list is never renamed, re-shared or deleted; two lists of one wallet
//! never share a name) are the old port's, re-keyed under `shop`.
//! Upstream kept `lists.updated_at` with row triggers on picks and the
//! ACL; here every statement that adds or removes a pick or a grant
//! touches it in the same statement, so a rebuild needs no trigger.

use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Row};

use super::signed::{optional_signer, require_not_scene_signer, require_signer};
use super::wire::{ApiError, Params, get_number_parameter, get_pagination_params, is_address};
use super::{MarketplaceState, SharedState};

pub const DEFAULT_LIST_NAME: &str = "Favorites";
/// Upstream's globally shared default list (`1678303321034_default-list`,
/// renamed by `1687172729802`), seeded by `schemas/30_shop.sql`.
pub const DEFAULT_LIST_ID: &str = "70ab6873-4a03-4eb2-b331-4b8be0e0b8af";
pub const DEFAULT_LIST_USER_ADDRESS: &str = "0x0000000000000000000000000000000000000000";
pub const GRANTED_TO_ALL: &str = "*";

#[derive(Clone, Debug, Serialize, PartialEq)]
#[cfg_attr(
    feature = "ts",
    derive(ts_rs::TS),
    ts(export, export_to = "market/", rename_all = "camelCase")
)]
pub struct PickStats {
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub count: i64,
    #[serde(rename = "itemId")]
    pub item_id: String,
    #[serde(rename = "pickedByUser", skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub picked_by_user: Option<bool>,
}

pub fn is_uuid(s: &str) -> bool {
    let b = s.as_bytes();
    if b.len() != 36 {
        return false;
    }
    b.iter().enumerate().all(|(i, c)| match i {
        8 | 13 | 18 | 23 => *c == b'-',
        _ => c.is_ascii_hexdigit(),
    })
}

/// Binds: $1 list ids, $2 caller, $3 default-list owner, $4 grantee wildcard.
const NON_EDITABLE_LISTS_SQL: &str = "SELECT l.id::text AS id FROM shop.lists l \
     WHERE l.id = ANY($1::uuid[]) AND l.user_address != $2 AND l.user_address != $3 \
     AND NOT EXISTS (SELECT 1 FROM shop.list_acl a WHERE a.list_id = l.id AND a.permission = 'edit' AND a.grantee IN ($2, $4))";

/// Binds: $1 item id, $2 caller, $3 pick list ids, $4 default-list owner,
/// $5 grantee wildcard, $6 unpick list ids. One statement: the INSERT and
/// the DELETE share a snapshot, so the remaining-pick check excludes the
/// unpicked lists itself.
const PICK_AND_UNPICK_IN_BULK_SQL: &str = "WITH picked AS (\
       INSERT INTO shop.picks (item_id, user_address, list_id) \
       SELECT $1, $2, l.id FROM shop.lists l \
       WHERE l.id = ANY($3::uuid[]) AND (l.user_address = $2 OR l.user_address = $4 \
         OR EXISTS (SELECT 1 FROM shop.list_acl a WHERE a.list_id = l.id AND a.permission = 'edit' AND a.grantee IN ($2, $5))) \
       ON CONFLICT (item_id, user_address, list_id) DO NOTHING\
     ), unpicked AS (\
       DELETE FROM shop.picks WHERE item_id = $1 AND user_address = $2 AND list_id = ANY($6::uuid[])\
     ), touched AS (\
       UPDATE shop.lists SET updated_at = timezone('UTC', now()) WHERE id = ANY($3::uuid[]) OR id = ANY($6::uuid[])\
     ) \
     SELECT (cardinality($3::uuid[]) > 0 \
             OR EXISTS (SELECT 1 FROM shop.picks WHERE item_id = $1 AND user_address = $2 AND NOT (list_id = ANY($6::uuid[])))) AS picked_by_user";

/// Upstream's row-level DISTINCT: two users' picks of one item in a shared
/// list are two rows; the window carries the pre-DISTINCT total.
const PICKS_BY_LIST_SQL: &str = "SELECT DISTINCT p.item_id, p.user_address, p.list_id, \
       (EXTRACT(EPOCH FROM p.created_at) * 1000)::int8 AS created_at, COUNT(*) OVER() AS picks_count \
     FROM shop.picks p LEFT JOIN shop.list_acl a ON p.list_id = a.list_id \
     WHERE p.list_id = $1::uuid AND (p.user_address = $2 OR a.grantee = $2 OR a.grantee = $3) \
     ORDER BY created_at DESC LIMIT $4 OFFSET $5";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ListSortBy {
    CreatedAt,
    Name,
    UpdatedAt,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ListSortDirection {
    Asc,
    Desc,
}

pub struct GetListsOptions<'a> {
    pub limit: i64,
    pub offset: i64,
    pub sort_by: ListSortBy,
    pub sort_direction: ListSortDirection,
    pub item_id: Option<&'a str>,
    pub q: Option<&'a str>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[cfg_attr(
    feature = "ts",
    derive(ts_rs::TS),
    ts(export, export_to = "market/", rename_all = "camelCase")
)]
pub struct FavoriteList {
    pub id: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub description: Option<String>,
    #[serde(rename = "userAddress")]
    pub user_address: String,
    #[serde(rename = "createdAt")]
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub created_at: i64,
    #[serde(rename = "updatedAt", skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(type = "number", optional))]
    pub updated_at: Option<i64>,
    #[serde(rename = "isPrivate")]
    pub is_private: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub permission: Option<String>,
    #[serde(rename = "isDefaultList")]
    pub is_default_list: bool,
    #[serde(rename = "itemsCount")]
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub items_count: i64,
    #[serde(rename = "previewOfItemIds")]
    pub preview_of_item_ids: Vec<String>,
    #[serde(rename = "isItemInList", skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub is_item_in_list: Option<bool>,
}

/// Binds: `$1` the caller, then q, then itemId when present, then limit and offset.
pub(crate) fn build_get_lists_sql(opts: &GetListsOptions<'_>) -> (String, String) {
    let mut next = 2;
    let mut take = || {
        let i = next;
        next += 1;
        i
    };
    let q_idx = opts.q.map(|_| take());
    let item_idx = opts.item_id.map(|_| take());
    let limit_idx = take();
    let offset_idx = take();
    let q_clause = q_idx
        .map(|i| format!("AND l.name ILIKE '%' || ${i} || '%'"))
        .unwrap_or_default();
    let item_select = item_idx
        .map(|i| format!(",\n  EXISTS(SELECT 1 FROM shop.picks ip WHERE ip.list_id = l.id AND ip.item_id = ${i} AND ip.user_address = $1) AS is_item_in_list"))
        .unwrap_or_default();
    let dir = match opts.sort_direction {
        ListSortDirection::Asc => "ASC",
        ListSortDirection::Desc => "DESC",
    };
    let order = match opts.sort_by {
        ListSortBy::CreatedAt => format!("l.created_at {dir}"),
        ListSortBy::UpdatedAt => format!("l.updated_at {dir} NULLS LAST"),
        ListSortBy::Name => format!("l.name {dir}"),
    };
    let sql = format!(
        "SELECT l.id::text AS id, l.name, l.description, l.user_address, \
  (EXTRACT(EPOCH FROM l.created_at) * 1000)::int8 AS created_at, \
  (EXTRACT(EPOCH FROM l.updated_at) * 1000)::int8 AS updated_at, \
  (SELECT COUNT(1) FROM shop.list_acl a WHERE a.list_id = l.id AND (a.grantee = $1 OR a.grantee = '{GRANTED_TO_ALL}')) = 0 AS is_private, \
  l.permission, (l.user_address = '{DEFAULT_LIST_USER_ADDRESS}') AS is_default_list, \
  COALESCE(pc.cnt, 0)::int8 AS items_count, COALESCE(pp.preview, ARRAY[]::text[]) AS preview, \
  COUNT(*) OVER()::int8 AS total{item_select} \
FROM shop.lists l \
LEFT JOIN (SELECT list_id, COUNT(*) AS cnt FROM shop.picks WHERE user_address = $1 GROUP BY list_id) pc ON pc.list_id = l.id \
LEFT JOIN (SELECT list_id, (ARRAY_AGG(item_id ORDER BY created_at ASC))[:4] AS preview FROM shop.picks WHERE user_address = $1 GROUP BY list_id) pp ON pp.list_id = l.id \
WHERE (l.user_address = $1 OR l.user_address = '{DEFAULT_LIST_USER_ADDRESS}') {q_clause} \
ORDER BY is_default_list DESC, {order}, l.id ASC \
LIMIT ${limit_idx} OFFSET ${offset_idx}"
    );
    let count_sql = format!(
        "SELECT COUNT(*)::int8 AS total FROM shop.lists l WHERE (l.user_address = $1 OR l.user_address = '{DEFAULT_LIST_USER_ADDRESS}') {q_clause}"
    );
    (sql, count_sql)
}

pub async fn get_lists_rows(
    pool: &PgPool,
    user_address: &str,
    opts: &GetListsOptions<'_>,
) -> Result<(Vec<FavoriteList>, i64), ApiError> {
    let (sql, count_sql) = build_get_lists_sql(opts);
    let mut q = sqlx::query(sqlx::AssertSqlSafe(sql)).bind(user_address.to_lowercase());
    if let Some(needle) = opts.q {
        q = q.bind(needle.to_string());
    }
    if let Some(item) = opts.item_id {
        q = q.bind(item.to_string());
    }
    let rows = q.bind(opts.limit).bind(opts.offset).fetch_all(pool).await?;
    let lists = rows
        .iter()
        .map(|r| FavoriteList {
            id: r.get("id"),
            name: r.get("name"),
            description: r.get("description"),
            user_address: r.get("user_address"),
            created_at: r.get("created_at"),
            updated_at: r.get("updated_at"),
            is_private: r.get("is_private"),
            permission: r.get("permission"),
            is_default_list: r.get("is_default_list"),
            items_count: r.get("items_count"),
            preview_of_item_ids: r.get("preview"),
            is_item_in_list: if opts.item_id.is_some() {
                r.try_get("is_item_in_list").ok()
            } else {
                None
            },
        })
        .collect();
    let total = match rows.first() {
        Some(r) => r.get::<i64, _>("total"),
        None if opts.offset > 0 => {
            let mut cq =
                sqlx::query(sqlx::AssertSqlSafe(count_sql)).bind(user_address.to_lowercase());
            if let Some(needle) = opts.q {
                cq = cq.bind(needle.to_string());
            }
            cq.fetch_one(pool).await?.get("total")
        }
        None => 0,
    };
    Ok((lists, total))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListPick {
    pub item_id: String,
    pub created_at: i64,
}

pub async fn get_picks_by_list_id(
    pool: &PgPool,
    list_id: &str,
    user_address: Option<&str>,
    limit: i64,
    offset: i64,
) -> Result<(Vec<ListPick>, i64), ApiError> {
    let rows = sqlx::query(PICKS_BY_LIST_SQL)
        .bind(list_id)
        .bind(user_address.map(|u| u.to_lowercase()))
        .bind(GRANTED_TO_ALL)
        .bind(limit)
        .bind(offset)
        .fetch_all(pool)
        .await?;
    let total = rows
        .first()
        .map(|r| r.get::<i64, _>("picks_count"))
        .unwrap_or(0);
    Ok((
        rows.iter()
            .map(|r| ListPick {
                item_id: r.get("item_id"),
                created_at: r.get("created_at"),
            })
            .collect(),
        total,
    ))
}

/// One row per requested id (never-picked ids at zero), `picked_by_user`
/// only when a checking address was given.
pub async fn picks_stats(
    pool: &PgPool,
    item_ids: &[String],
    user_address: Option<&str>,
) -> Result<Vec<PickStats>, ApiError> {
    if item_ids.is_empty() {
        return Ok(Vec::new());
    }
    let rows = sqlx::query(
        "SELECT items_to_find.item_id AS item_id, COUNT(DISTINCT p.user_address)::int8 AS count, \
                CASE WHEN $2::text IS NULL THEN CAST(NULL AS boolean) ELSE COALESCE(BOOL_OR(p.user_address = $2), false) END AS picked_by_user \
         FROM (SELECT unnest($1::text[]) AS item_id) items_to_find \
         LEFT JOIN shop.picks p ON p.item_id = items_to_find.item_id \
         GROUP BY items_to_find.item_id",
    )
    .bind(item_ids)
    .bind(user_address.map(|u| u.to_lowercase()))
    .fetch_all(pool)
    .await?;
    Ok(rows
        .iter()
        .map(|r| PickStats {
            count: r.get("count"),
            item_id: r.get("item_id"),
            picked_by_user: r.get("picked_by_user"),
        })
        .collect())
}

pub async fn check_non_editable_lists(
    pool: &PgPool,
    list_ids: &[String],
    user_address: &str,
) -> Result<Vec<String>, ApiError> {
    if list_ids.is_empty() {
        return Ok(Vec::new());
    }
    let rows = sqlx::query(NON_EDITABLE_LISTS_SQL)
        .bind(list_ids)
        .bind(user_address.to_lowercase())
        .bind(DEFAULT_LIST_USER_ADDRESS)
        .bind(GRANTED_TO_ALL)
        .fetch_all(pool)
        .await?;
    Ok(rows.iter().map(|r| r.get::<String, _>("id")).collect())
}

/// The old `squid.item` probe over the `items` mirror.
pub async fn item_exists(state: &MarketplaceState, item_id: &str) -> Result<bool, ApiError> {
    let Some((contract, id)) = super::wire::split_item_wire_id(item_id) else {
        return Ok(false);
    };
    Ok(sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM shop.items WHERE contract_address = $1 AND item_id = $2::numeric)")
        .bind(contract)
        .bind(id)
        .fetch_one(&state.pool)
        .await?)
}

pub async fn get_or_create_default_list(
    pool: &PgPool,
    user_address: &str,
) -> Result<String, ApiError> {
    let row = sqlx::query(
        "WITH existing AS (SELECT id FROM shop.lists WHERE user_address = $2 AND name = $1 ORDER BY created_at ASC LIMIT 1), \
              created AS (INSERT INTO shop.lists (name, user_address, is_private) SELECT $1, $2, true WHERE NOT EXISTS (SELECT 1 FROM existing) RETURNING id) \
         SELECT id::text AS id FROM existing UNION ALL SELECT id::text AS id FROM created",
    )
    .bind(DEFAULT_LIST_NAME)
    .bind(user_address.to_lowercase())
    .fetch_one(pool)
    .await?;
    Ok(row.get("id"))
}

pub async fn pick_and_unpick_in_bulk(
    pool: &PgPool,
    item_id: &str,
    user_address: &str,
    pick_ids: &[String],
    unpick_ids: &[String],
) -> Result<bool, ApiError> {
    if pick_ids.is_empty() && unpick_ids.is_empty() {
        return is_picked_by_user(pool, item_id, user_address).await;
    }
    let row = sqlx::query(PICK_AND_UNPICK_IN_BULK_SQL)
        .bind(item_id)
        .bind(user_address.to_lowercase())
        .bind(pick_ids)
        .bind(DEFAULT_LIST_USER_ADDRESS)
        .bind(GRANTED_TO_ALL)
        .bind(unpick_ids)
        .fetch_one(pool)
        .await?;
    Ok(row.get("picked_by_user"))
}

pub async fn unpick_everywhere_rows(
    pool: &PgPool,
    item_id: &str,
    user_address: &str,
) -> Result<u64, ApiError> {
    let rows: Vec<(String,)> = sqlx::query_as(
        "WITH gone AS (DELETE FROM shop.picks WHERE item_id = $1 AND user_address = $2 RETURNING list_id), \
              touched AS (UPDATE shop.lists SET updated_at = timezone('UTC', now()) WHERE id IN (SELECT list_id FROM gone)) \
         SELECT list_id::text FROM gone",
    )
    .bind(item_id)
    .bind(user_address.to_lowercase())
    .fetch_all(pool)
    .await?;
    Ok(rows.len() as u64)
}

pub async fn is_picked_by_user(
    pool: &PgPool,
    item_id: &str,
    user_address: &str,
) -> Result<bool, ApiError> {
    Ok(sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM shop.picks WHERE item_id = $1 AND user_address = $2)",
    )
    .bind(item_id)
    .bind(user_address.to_lowercase())
    .fetch_one(pool)
    .await?)
}

#[derive(Debug, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "market/"))]
pub struct ListsPage {
    pub results: Vec<FavoriteList>,
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub total: i64,
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub page: i64,
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub pages: i64,
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub limit: i64,
}

#[derive(Debug, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "market/"))]
pub struct ListPickItem {
    #[serde(rename = "itemId")]
    #[cfg_attr(feature = "ts", ts(rename = "itemId"))]
    pub item_id: String,
    #[serde(rename = "createdAt")]
    #[cfg_attr(feature = "ts", ts(rename = "createdAt", type = "number"))]
    pub created_at: i64,
}

#[derive(Debug, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "market/"))]
pub struct PicksPage {
    pub results: Vec<ListPickItem>,
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub total: i64,
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub page: i64,
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub pages: i64,
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub limit: i64,
}

fn ok<T: Serialize>(data: T) -> Response {
    Json(serde_json::json!({ "ok": true, "data": data })).into_response()
}

/// The `ok(..)` envelopes the list and pick routes answer, as ts-rs shapes.
#[cfg(feature = "ts")]
pub mod envelope_shapes {
    use super::{ListsPage, PickStats, PicksPage};

    #[derive(ts_rs::TS)]
    #[ts(export, export_to = "market/")]
    pub struct ListsEnvelope {
        pub ok: bool,
        pub data: ListsPage,
    }

    #[derive(ts_rs::TS)]
    #[ts(export, export_to = "market/")]
    pub struct PicksEnvelope {
        pub ok: bool,
        pub data: PicksPage,
    }

    #[derive(ts_rs::TS)]
    #[ts(export, export_to = "market/")]
    pub struct PicksStatsEnvelope {
        pub ok: bool,
        pub data: Vec<PickStats>,
    }

    #[derive(ts_rs::TS)]
    #[ts(export, export_to = "market/", rename_all = "camelCase")]
    pub struct PickUnpickResult {
        pub picked_by_user: bool,
    }

    #[derive(ts_rs::TS)]
    #[ts(export, export_to = "market/")]
    pub struct PickUnpickEnvelope {
        pub ok: bool,
        pub data: PickUnpickResult,
    }
}

fn parse_sort_by(raw: Option<&str>) -> Result<ListSortBy, ApiError> {
    match raw {
        None | Some("createdAt") => Ok(ListSortBy::CreatedAt),
        Some("name") => Ok(ListSortBy::Name),
        Some("updatedAt") => Ok(ListSortBy::UpdatedAt),
        Some(_) => Err(ApiError::bad_request(
            "The sort by parameter is not defined as createdAt, name, or updatedAt.",
        )),
    }
}

fn parse_sort_direction(raw: Option<&str>) -> Result<ListSortDirection, ApiError> {
    match raw {
        None | Some("desc") => Ok(ListSortDirection::Desc),
        Some("asc") => Ok(ListSortDirection::Asc),
        Some(_) => Err(ApiError::bad_request(
            "The sort direction parameter is not defined as asc or desc.",
        )),
    }
}

async fn lists_inner(
    state: &MarketplaceState,
    headers: &HeaderMap,
    pairs: &[(String, String)],
) -> Result<Response, ApiError> {
    require_not_scene_signer(headers)?;
    let user = require_signer(headers, "get", "/v1/lists").await?;
    let pg = get_pagination_params(pairs);
    let p = Params::new(pairs);
    let sort_by = parse_sort_by(p.get_string("sortBy", None).as_deref())?;
    let sort_direction = parse_sort_direction(p.get_string("sortDirection", None).as_deref())?;
    let item_id = p.get_string("itemId", None);
    let q = p.get_string("q", None);
    let (results, total) = get_lists_rows(
        &state.pool,
        &user,
        &GetListsOptions {
            limit: pg.limit,
            offset: pg.offset,
            sort_by,
            sort_direction,
            item_id: item_id.as_deref(),
            q: q.as_deref(),
        },
    )
    .await?;
    let page = if pg.limit > 0 {
        pg.offset / pg.limit
    } else {
        0
    };
    let pages = if total > 0 && pg.limit > 0 {
        (total + pg.limit - 1) / pg.limit
    } else {
        0
    };
    Ok(ok(ListsPage {
        results,
        total,
        page,
        pages,
        limit: pg.limit,
    }))
}

pub async fn get_lists(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Query(pairs): Query<Vec<(String, String)>>,
) -> Response {
    lists_inner(&state, &headers, &pairs)
        .await
        .unwrap_or_else(IntoResponse::into_response)
}

/// Upstream's envelope math: an empty page zeroes `total` and `pages`.
pub(crate) fn picks_page(picks: Vec<ListPick>, count: i64, limit: i64, offset: i64) -> PicksPage {
    let page = if limit > 0 { offset / limit } else { 0 };
    let pages = if !picks.is_empty() && limit > 0 {
        (count + limit - 1) / limit
    } else {
        0
    };
    let total = if picks.is_empty() { 0 } else { count };
    PicksPage {
        results: picks
            .into_iter()
            .map(|p| ListPickItem {
                item_id: p.item_id,
                created_at: p.created_at,
            })
            .collect(),
        total,
        page,
        pages,
        limit,
    }
}

async fn list_picks_inner(
    state: &MarketplaceState,
    id: &str,
    headers: &HeaderMap,
    pairs: &[(String, String)],
) -> Result<Response, ApiError> {
    let user = optional_signer(headers, "get", &format!("/v1/lists/{id}/picks")).await?;
    if !is_uuid(id) {
        return Err(ApiError::bad_request("The list id must be a UUID."));
    }
    let pg = get_pagination_params(pairs);
    let (picks, count) =
        get_picks_by_list_id(&state.pool, id, user.as_deref(), pg.limit, pg.offset).await?;
    Ok(ok(picks_page(picks, count, pg.limit, pg.offset)))
}

pub async fn get_list_picks(
    State(state): State<SharedState>,
    Path(id): Path<String>,
    headers: HeaderMap,
    Query(pairs): Query<Vec<(String, String)>>,
) -> Response {
    list_picks_inner(&state, &id, &headers, &pairs)
        .await
        .unwrap_or_else(IntoResponse::into_response)
}

pub(crate) const LISTS_NOT_FOUND_MESSAGE: &str = "Some lists were not found.";
pub(crate) const ITEM_NOT_FOUND_MESSAGE: &str = "The item trying to get saved doesn't exist.";
pub(crate) const CHECKING_USER_ADDRESS_MESSAGE: &str =
    "The checking user address parameter must be an Ethereum Address.";
pub(crate) const NO_ITEM_IDS_MESSAGE: &str = "The request must include at least one item id.";

#[derive(Debug, Default, Deserialize)]
pub struct PickUnpickInBulkBody {
    #[serde(default, rename = "pickedFor")]
    pub picked_for: Option<Vec<String>>,
    #[serde(default, rename = "unpickedFrom")]
    pub unpicked_from: Option<Vec<String>>,
}

/// Upstream's `ListsNotFoundError` / `ItemNotFoundError` 404s carry the
/// offending ids in a `data` payload the plain envelope cannot express.
#[derive(Debug)]
pub enum PicksError {
    Api(ApiError),
    ListsNotFound(Vec<String>),
    ItemNotFound(String),
}

impl From<ApiError> for PicksError {
    fn from(e: ApiError) -> Self {
        PicksError::Api(e)
    }
}

impl IntoResponse for PicksError {
    fn into_response(self) -> Response {
        match self {
            PicksError::Api(e) => e.into_response(),
            PicksError::ListsNotFound(list_ids) => (StatusCode::NOT_FOUND, Json(serde_json::json!({ "ok": false, "message": LISTS_NOT_FOUND_MESSAGE, "data": { "listIds": list_ids } }))).into_response(),
            PicksError::ItemNotFound(item_id) => (StatusCode::NOT_FOUND, Json(serde_json::json!({ "ok": false, "message": ITEM_NOT_FOUND_MESSAGE, "data": { "itemId": item_id } }))).into_response(),
        }
    }
}

fn validate_list_ids(ids: &[String]) -> Result<(), ApiError> {
    if ids.iter().any(|id| !is_uuid(id)) {
        return Err(ApiError::bad_request("list ids must be UUIDs"));
    }
    Ok(())
}

async fn authenticate(headers: &HeaderMap, method: &str, path: &str) -> Result<String, ApiError> {
    require_not_scene_signer(headers)?;
    require_signer(headers, method, path).await
}

async fn pick_inner(
    state: &MarketplaceState,
    item_id: &str,
    headers: &HeaderMap,
    body: Option<PickUnpickInBulkBody>,
) -> Result<Response, PicksError> {
    let user = authenticate(headers, "post", &format!("/v1/picks/{item_id}")).await?;
    let body = body.unwrap_or_default();
    let picked_for = body.picked_for.unwrap_or_default();
    let unpicked_from = body.unpicked_from.unwrap_or_default();
    if picked_for.iter().any(|id| unpicked_from.contains(id)) {
        return Err(ApiError::bad_request(
            "The item cannot be be picked and unpicked from a list at the same time.",
        )
        .into());
    }
    validate_list_ids(&picked_for)?;
    validate_list_ids(&unpicked_from)?;
    let (pick_ids, unpick_ids) = if picked_for.is_empty() && unpicked_from.is_empty() {
        if !item_exists(state, item_id).await? {
            return Err(PicksError::ItemNotFound(item_id.to_string()));
        }
        (
            vec![get_or_create_default_list(&state.pool, &user).await?],
            Vec::new(),
        )
    } else {
        let mut all = picked_for.clone();
        all.extend(unpicked_from.iter().cloned());
        all.sort();
        all.dedup();
        let (found, non_editable) = tokio::try_join!(
            item_exists(state, item_id),
            check_non_editable_lists(&state.pool, &all, &user)
        )?;
        if !found {
            return Err(PicksError::ItemNotFound(item_id.to_string()));
        }
        if !non_editable.is_empty() {
            return Err(PicksError::ListsNotFound(non_editable));
        }
        (picked_for, unpicked_from)
    };
    let picked_by_user =
        pick_and_unpick_in_bulk(&state.pool, item_id, &user, &pick_ids, &unpick_ids).await?;
    state.catalog_cache.invalidate();
    Ok(ok(serde_json::json!({ "pickedByUser": picked_by_user })))
}

pub async fn pick_unpick_in_bulk(
    State(state): State<SharedState>,
    Path(item_id): Path<String>,
    headers: HeaderMap,
    body: Option<Json<PickUnpickInBulkBody>>,
) -> Response {
    pick_inner(&state, &item_id, &headers, body.map(|Json(b)| b))
        .await
        .unwrap_or_else(IntoResponse::into_response)
}

pub async fn unpick_everywhere(
    State(state): State<SharedState>,
    Path(item_id): Path<String>,
    headers: HeaderMap,
) -> Response {
    let user = match authenticate(&headers, "delete", &format!("/v1/picks/{item_id}")).await {
        Ok(u) => u,
        Err(e) => return e.into_response(),
    };
    if let Err(e) = unpick_everywhere_rows(&state.pool, &item_id, &user).await {
        return e.into_response();
    }
    state.catalog_cache.invalidate();
    ok(serde_json::json!({ "pickedByUser": false }))
}

/// The one picks route upstream registers unsigned: `checkingUserAddress`
/// is a plain query string, lower-cased before the address test; `power` is
/// parsed for its malformed-value 400 and not applied (no `voting` table).
pub async fn get_picks_stats(
    State(state): State<SharedState>,
    Query(pairs): Query<Vec<(String, String)>>,
) -> Response {
    if let Err(e) = get_number_parameter("power", &pairs) {
        return ApiError::from(e).into_response();
    }
    let item_ids: Vec<String> = pairs
        .iter()
        .filter(|(k, _)| k == "itemId")
        .map(|(_, v)| v.clone())
        .collect();
    let user = pairs
        .iter()
        .find(|(k, _)| k == "checkingUserAddress")
        .map(|(_, v)| v.to_lowercase())
        .filter(|v| !v.is_empty());
    if let Some(address) = &user
        && !is_address(address)
    {
        return ApiError::bad_request(CHECKING_USER_ADDRESS_MESSAGE).into_response();
    }
    if item_ids.is_empty() {
        return ApiError::bad_request(NO_ITEM_IDS_MESSAGE).into_response();
    }
    match picks_stats(&state.pool, &item_ids, user.as_deref()).await {
        Ok(data) => ok(data),
        Err(e) => e.into_response(),
    }
}

pub const LIST_NAME_MAX: usize = 32;
pub const LIST_DESCRIPTION_MAX: usize = 100;
pub const LIST_NOT_FOUND_MESSAGE: &str = "The list was not found.";
pub const PICK_ALREADY_EXISTS_MESSAGE: &str = "The item was already favorited.";
pub const PICK_NOT_FOUND_MESSAGE: &str =
    "The pick does not exist or is not accessible by this user.";
pub const ACCESS_NOT_FOUND_MESSAGE: &str = "The access doesn't exist.";
pub const DEFAULT_LIST_IMMUTABLE_MESSAGE: &str = "The default list cannot be modified.";
/// The unique constraint two lists of one wallet with one name violate
/// (upstream `name_user_address_unique`).
pub const LIST_NAME_UNIQUE_CONSTRAINT: &str = "lists_name_user_unique";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Permission {
    View,
    Edit,
}

impl Permission {
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "view" => Some(Self::View),
            "edit" => Some(Self::Edit),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::View => "view",
            Self::Edit => "edit",
        }
    }

    /// The grants that satisfy a requirement: `edit` implies `view`.
    fn satisfied_by(self) -> &'static [&'static str] {
        match self {
            Self::View => &["view", "edit"],
            Self::Edit => &["edit"],
        }
    }
}

/// Upstream `isDefaultList`: Postgres compares a `uuid` ignoring case,
/// hyphens and braces, so the guard reduces any spelling to its hex digits.
pub fn is_default_list(list_id: &str) -> bool {
    let hex = |s: &str| {
        s.to_lowercase()
            .chars()
            .filter(|c| c.is_ascii_hexdigit())
            .collect::<String>()
    };
    hex(list_id) == hex(DEFAULT_LIST_ID)
}

/// One list as `GET /v1/lists/{id}`, `POST /v1/lists` and `PUT /v1/lists/{id}`
/// answer it (upstream `fromDBListToList` / `...WithItemsCount`): `updatedAt`
/// is `0` while the list was never touched (upstream's `Number(null)`),
/// `permission` and `previewOfItemIds` only where the statement produced them.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ListOut {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub user_address: String,
    pub created_at: i64,
    pub updated_at: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub permission: Option<String>,
    pub is_private: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preview_of_item_ids: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub items_count: Option<i64>,
}

/// Upstream `getListQuery`: the list when the caller owns it, when it is the
/// shared default list (unless `consider_default` is off, as a grant needs
/// an owner) or when the ACL grants the caller (or everyone) the required
/// permission; the picks counted are the caller's and the owner's; the
/// default list's `updated_at` is its newest pick. `None` = not found or
/// not accessible: the two are one 404, so a private list is not probed.
pub async fn get_list(
    pool: &PgPool,
    list_id: &str,
    user: Option<&str>,
    required: Option<Permission>,
    consider_default: bool,
) -> Result<Option<ListOut>, ApiError> {
    if !is_uuid(list_id) {
        return Ok(None);
    }
    let user = user.map(|u| u.to_lowercase()).unwrap_or_default();
    let updated_at = if is_default_list(list_id) {
        "(EXTRACT(EPOCH FROM MAX(p.created_at)) * 1000)::int8"
    } else {
        "(EXTRACT(EPOCH FROM l.updated_at) * 1000)::int8"
    };
    let default_arm = if consider_default {
        format!(" OR l.user_address = '{DEFAULT_LIST_USER_ADDRESS}'")
    } else {
        String::new()
    };
    let grant_arm = match required {
        Some(p) => format!(
            " OR ((a.grantee = $2 OR a.grantee = '{GRANTED_TO_ALL}') AND a.permission = ANY(ARRAY[{}]::text[]))",
            p.satisfied_by()
                .iter()
                .map(|s| format!("'{s}'"))
                .collect::<Vec<_>>()
                .join(",")
        ),
        None => String::new(),
    };
    let sql = format!(
        "SELECT l.id::text AS id, l.name, l.description, l.user_address, \
                (EXTRACT(EPOCH FROM l.created_at) * 1000)::int8 AS created_at, {updated_at} AS updated_at, \
                a.permission AS permission, COUNT(p.item_id)::int8 AS items_count, COUNT(a.permission) = 0 AS is_private, \
                (ARRAY_REMOVE(ARRAY_AGG(p.item_id ORDER BY p.created_at), NULL))[:4] AS preview \
         FROM shop.lists l \
         LEFT JOIN shop.picks p ON l.id = p.list_id AND (p.user_address = $2 OR p.user_address = l.user_address) \
         LEFT JOIN shop.list_acl a ON l.id = a.list_id \
         WHERE l.id = $1::uuid AND (l.user_address = $2{default_arm}{grant_arm}) \
         GROUP BY l.id, a.permission ORDER BY a.permission ASC LIMIT 1"
    );
    let row = sqlx::query(sqlx::AssertSqlSafe(sql))
        .bind(list_id)
        .bind(&user)
        .fetch_optional(pool)
        .await?;
    Ok(row.map(|r| ListOut {
        id: r.get("id"),
        name: r.get("name"),
        description: r.get("description"),
        user_address: r.get("user_address"),
        created_at: r.get("created_at"),
        updated_at: r
            .try_get::<Option<i64>, _>("updated_at")
            .ok()
            .flatten()
            .unwrap_or(0),
        permission: r.get("permission"),
        is_private: r.get("is_private"),
        preview_of_item_ids: Some(r.get("preview")),
        items_count: Some(r.get("items_count")),
    }))
}

/// The list as the write statements return it (no picks, no grant column):
/// `is_private` is what the write decided, as upstream's `addList` answered.
fn list_row(r: &sqlx::postgres::PgRow, is_private: bool) -> ListOut {
    ListOut {
        id: r.get("id"),
        name: r.get("name"),
        description: r.get("description"),
        user_address: r.get("user_address"),
        created_at: r.get("created_at"),
        updated_at: r
            .try_get::<Option<i64>, _>("updated_at")
            .ok()
            .flatten()
            .unwrap_or(0),
        permission: None,
        is_private,
        preview_of_item_ids: None,
        items_count: None,
    }
}

fn is_duplicate_name(e: &sqlx::Error) -> bool {
    matches!(e, sqlx::Error::Database(d) if d.constraint() == Some(LIST_NAME_UNIQUE_CONSTRAINT))
}

/// Upstream `addList`: the row, plus the everyone-`view` grant unless private,
/// in one statement. A second list with the name is [`ListsError::DuplicatedList`].
pub async fn add_list(
    pool: &PgPool,
    name: &str,
    description: Option<&str>,
    user: &str,
    private: bool,
) -> Result<ListOut, ListsError> {
    let row = sqlx::query(
        "WITH l AS (INSERT INTO shop.lists (name, description, user_address, is_private) VALUES ($1, $2, $3, $4) RETURNING *), \
              g AS (INSERT INTO shop.list_acl (list_id, permission, grantee) SELECT id, 'view', $5 FROM l WHERE NOT $4 ON CONFLICT DO NOTHING) \
         SELECT l.id::text AS id, l.name, l.description, l.user_address, (EXTRACT(EPOCH FROM l.created_at) * 1000)::int8 AS created_at, \
                (EXTRACT(EPOCH FROM l.updated_at) * 1000)::int8 AS updated_at FROM l",
    )
    .bind(name)
    .bind(description)
    .bind(user.to_lowercase())
    .bind(private)
    .bind(GRANTED_TO_ALL)
    .fetch_one(pool)
    .await
    .map_err(|e| if is_duplicate_name(&e) { ListsError::DuplicatedList(name.to_string()) } else { ListsError::Api(e.into()) })?;
    Ok(list_row(&row, private))
}

#[derive(Debug, Default, Deserialize)]
pub struct ListUpdate {
    pub name: Option<String>,
    pub description: Option<String>,
    pub private: Option<bool>,
}

/// Upstream `updateList`: the fields given, the everyone-`view` grant added
/// or removed when `private` is given, the owner's list only, in one
/// transaction; the answer is the list re-read as [`get_list`] reads it.
pub async fn update_list(
    pool: &PgPool,
    list_id: &str,
    user: &str,
    update: &ListUpdate,
) -> Result<ListOut, ListsError> {
    if !is_uuid(list_id) {
        return Err(ListsError::ListNotFound(list_id.to_string()));
    }
    let user = user.to_lowercase();
    let mut tx = pool.begin().await.map_err(ApiError::from)?;
    let owned: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM shop.lists WHERE id = $1::uuid AND user_address = $2)",
    )
    .bind(list_id)
    .bind(&user)
    .fetch_one(&mut *tx)
    .await
    .map_err(ApiError::from)?;
    if !owned {
        return Err(ListsError::ListNotFound(list_id.to_string()));
    }
    if update.name.is_some() || update.description.is_some() {
        sqlx::query(
            "UPDATE shop.lists SET name = COALESCE($3, name), description = COALESCE($4, description), updated_at = timezone('UTC', now()) \
             WHERE id = $1::uuid AND user_address = $2",
        )
        .bind(list_id)
        .bind(&user)
        .bind(update.name.as_deref())
        .bind(update.description.as_deref())
        .execute(&mut *tx)
        .await
        .map_err(|e| if is_duplicate_name(&e) { ListsError::DuplicatedList(update.name.clone().unwrap_or_default()) } else { ListsError::Api(e.into()) })?;
    }
    match update.private {
        Some(true) => {
            sqlx::query(
                "WITH gone AS (DELETE FROM shop.list_acl WHERE list_id = $1::uuid AND permission = 'view' AND grantee = $2 RETURNING list_id) \
                 UPDATE shop.lists SET is_private = true, updated_at = timezone('UTC', now()) WHERE id = $1::uuid",
            )
            .bind(list_id)
            .bind(GRANTED_TO_ALL)
            .execute(&mut *tx)
            .await
            .map_err(ApiError::from)?;
        }
        Some(false) => {
            sqlx::query(
                "WITH granted AS (INSERT INTO shop.list_acl (list_id, permission, grantee) VALUES ($1::uuid, 'view', $2) ON CONFLICT DO NOTHING) \
                 UPDATE shop.lists SET is_private = false, updated_at = timezone('UTC', now()) WHERE id = $1::uuid",
            )
            .bind(list_id)
            .bind(GRANTED_TO_ALL)
            .execute(&mut *tx)
            .await
            .map_err(ApiError::from)?;
        }
        None => {}
    }
    tx.commit().await.map_err(ApiError::from)?;
    get_list(pool, list_id, Some(&user), None, true)
        .await?
        .ok_or_else(|| ListsError::ListNotFound(list_id.to_string()))
}

pub async fn delete_list(pool: &PgPool, list_id: &str, user: &str) -> Result<(), ListsError> {
    if !is_uuid(list_id) || is_default_list(list_id) {
        return Err(ListsError::ListNotFound(list_id.to_string()));
    }
    let done = sqlx::query("DELETE FROM shop.lists WHERE id = $1::uuid AND user_address = $2")
        .bind(list_id)
        .bind(user.to_lowercase())
        .execute(pool)
        .await
        .map_err(ApiError::from)?;
    if done.rows_affected() == 0 {
        return Err(ListsError::ListNotFound(list_id.to_string()));
    }
    Ok(())
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PickOut {
    pub item_id: String,
    pub user_address: String,
    pub list_id: String,
    pub created_at: i64,
}

/// Upstream `addPickToList`: the list must grant the caller `edit` (its
/// owner, the shared Wishlist, or a grant), the item must exist in the
/// mirror, and the pick must be new.
pub async fn add_pick_to_list(
    state: &MarketplaceState,
    list_id: &str,
    item_id: &str,
    user: &str,
) -> Result<PickOut, ListsError> {
    let user = user.to_lowercase();
    let list = get_list(
        &state.pool,
        list_id,
        Some(&user),
        Some(Permission::Edit),
        true,
    )
    .await?
    .ok_or_else(|| ListsError::ListNotFound(list_id.to_string()))?;
    if !item_exists(state, item_id).await? {
        return Err(ListsError::ItemNotFound(item_id.to_string()));
    }
    let row = sqlx::query(
        "WITH p AS (INSERT INTO shop.picks (item_id, user_address, list_id) VALUES ($1, $2, $3::uuid) ON CONFLICT DO NOTHING RETURNING *), \
              touched AS (UPDATE shop.lists SET updated_at = timezone('UTC', now()) WHERE id = $3::uuid AND EXISTS (SELECT 1 FROM p)) \
         SELECT p.item_id, p.user_address, p.list_id::text AS list_id, (EXTRACT(EPOCH FROM p.created_at) * 1000)::int8 AS created_at FROM p",
    )
    .bind(item_id)
    .bind(&user)
    .bind(&list.id)
    .fetch_optional(&state.pool)
    .await
    .map_err(ApiError::from)?;
    match row {
        Some(r) => Ok(PickOut {
            item_id: r.get("item_id"),
            user_address: r.get("user_address"),
            list_id: r.get("list_id"),
            created_at: r.get("created_at"),
        }),
        None => Err(ListsError::PickAlreadyExists {
            list_id: list.id,
            item_id: item_id.to_string(),
        }),
    }
}

pub async fn delete_pick_in_list(
    pool: &PgPool,
    list_id: &str,
    item_id: &str,
    user: &str,
) -> Result<(), ListsError> {
    if !is_uuid(list_id) {
        return Err(ListsError::PickNotFound {
            list_id: list_id.to_string(),
            item_id: item_id.to_string(),
        });
    }
    let rows: Vec<(String,)> = sqlx::query_as(
        "WITH gone AS (DELETE FROM shop.picks WHERE list_id = $1::uuid AND item_id = $2 AND user_address = $3 RETURNING list_id), \
              touched AS (UPDATE shop.lists SET updated_at = timezone('UTC', now()) WHERE id IN (SELECT list_id FROM gone)) \
         SELECT list_id::text FROM gone",
    )
    .bind(list_id)
    .bind(item_id)
    .bind(user.to_lowercase())
    .fetch_all(pool)
    .await
    .map_err(ApiError::from)?;
    if rows.is_empty() {
        return Err(ListsError::PickNotFound {
            list_id: list_id.to_string(),
            item_id: item_id.to_string(),
        });
    }
    Ok(())
}

/// Upstream `createAccess`: only the list's owner shares it, and the shared
/// default list is nobody's to share; a grant that exists already is kept
/// (`ON CONFLICT DO NOTHING`, answered like a new one, as upstream did).
pub async fn create_access(
    pool: &PgPool,
    list_id: &str,
    permission: Permission,
    grantee: &str,
    owner: &str,
) -> Result<(), ListsError> {
    let owner = owner.to_lowercase();
    if get_list(pool, list_id, Some(&owner), None, false)
        .await?
        .is_none()
    {
        return Err(ListsError::ListNotFound(list_id.to_string()));
    }
    sqlx::query(
        "WITH granted AS (INSERT INTO shop.list_acl (list_id, permission, grantee) VALUES ($1::uuid, $2, $3) ON CONFLICT DO NOTHING RETURNING list_id) \
         UPDATE shop.lists SET updated_at = timezone('UTC', now()) WHERE id IN (SELECT list_id FROM granted)",
    )
    .bind(list_id)
    .bind(permission.as_str())
    .bind(grantee.to_lowercase())
    .execute(pool)
    .await
    .map_err(ApiError::from)?;
    Ok(())
}

pub async fn delete_access(
    pool: &PgPool,
    list_id: &str,
    permission: Permission,
    grantee: &str,
    owner: &str,
) -> Result<(), ListsError> {
    let not_found = || ListsError::AccessNotFound {
        list_id: list_id.to_string(),
        permission: permission.as_str().to_string(),
        grantee: grantee.to_lowercase(),
    };
    if !is_uuid(list_id) {
        return Err(not_found());
    }
    let rows: Vec<(String,)> = sqlx::query_as(
        "WITH gone AS (DELETE FROM shop.list_acl a USING shop.lists l \
              WHERE a.list_id = l.id AND a.list_id = $1::uuid AND l.user_address = $4 AND a.permission = $2 AND a.grantee = $3 RETURNING a.list_id), \
              touched AS (UPDATE shop.lists SET updated_at = timezone('UTC', now()) WHERE id IN (SELECT list_id FROM gone)) \
         SELECT list_id::text FROM gone",
    )
    .bind(list_id)
    .bind(permission.as_str())
    .bind(grantee.to_lowercase())
    .bind(owner.to_lowercase())
    .fetch_all(pool)
    .await
    .map_err(ApiError::from)?;
    if rows.is_empty() {
        return Err(not_found());
    }
    Ok(())
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ItemPicker {
    pub user_address: String,
}

/// Upstream `getPicksByItemId` without the `voting` table (v2 has none, as
/// `get_picks_stats` notes): every distinct picker of the item, the caller
/// first, then newest first.
pub async fn picks_by_item(
    pool: &PgPool,
    item_id: &str,
    user: Option<&str>,
    limit: i64,
    offset: i64,
) -> Result<(Vec<ItemPicker>, i64), ApiError> {
    let rows = sqlx::query(
        "SELECT user_address, COUNT(*) OVER()::int8 AS picks_count FROM ( \
             SELECT DISTINCT ON (p.user_address) p.user_address, p.created_at, (p.user_address = $2) AS picked_by_user \
             FROM shop.picks p WHERE p.item_id = $1 ORDER BY p.user_address, p.created_at DESC \
         ) AS pickers ORDER BY picked_by_user DESC, created_at DESC LIMIT $3 OFFSET $4",
    )
    .bind(item_id)
    .bind(user.map(|u| u.to_lowercase()).unwrap_or_default())
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await?;
    let total = rows
        .first()
        .map(|r| r.get::<i64, _>("picks_count"))
        .unwrap_or(0);
    Ok((
        rows.iter()
            .map(|r| ItemPicker {
                user_address: r.get("user_address"),
            })
            .collect(),
        total,
    ))
}

/// Upstream's list-shaped 404s and 422s: each carries the ids it is about.
#[derive(Debug)]
pub enum ListsError {
    Api(ApiError),
    ListNotFound(String),
    DuplicatedList(String),
    ItemNotFound(String),
    PickAlreadyExists {
        list_id: String,
        item_id: String,
    },
    PickNotFound {
        list_id: String,
        item_id: String,
    },
    AccessNotFound {
        list_id: String,
        permission: String,
        grantee: String,
    },
}

impl From<ApiError> for ListsError {
    fn from(e: ApiError) -> Self {
        ListsError::Api(e)
    }
}

impl IntoResponse for ListsError {
    fn into_response(self) -> Response {
        let (status, message, data) = match self {
            ListsError::Api(e) => return e.into_response(),
            ListsError::ListNotFound(list_id) => (
                StatusCode::NOT_FOUND,
                LIST_NOT_FOUND_MESSAGE.to_string(),
                serde_json::json!({ "listId": list_id }),
            ),
            ListsError::DuplicatedList(name) => (
                StatusCode::UNPROCESSABLE_ENTITY,
                format!("There is already a list with the same name: {name}."),
                serde_json::json!({ "name": name }),
            ),
            ListsError::ItemNotFound(item_id) => (
                StatusCode::NOT_FOUND,
                ITEM_NOT_FOUND_MESSAGE.to_string(),
                serde_json::json!({ "itemId": item_id }),
            ),
            ListsError::PickAlreadyExists { list_id, item_id } => (
                StatusCode::UNPROCESSABLE_ENTITY,
                PICK_ALREADY_EXISTS_MESSAGE.to_string(),
                serde_json::json!({ "listId": list_id, "itemId": item_id }),
            ),
            ListsError::PickNotFound { list_id, item_id } => (
                StatusCode::NOT_FOUND,
                PICK_NOT_FOUND_MESSAGE.to_string(),
                serde_json::json!({ "listId": list_id, "itemId": item_id }),
            ),
            ListsError::AccessNotFound {
                list_id,
                permission,
                grantee,
            } => (
                StatusCode::NOT_FOUND,
                ACCESS_NOT_FOUND_MESSAGE.to_string(),
                serde_json::json!({ "listId": list_id, "permission": permission, "grantee": grantee }),
            ),
        };
        (
            status,
            Json(serde_json::json!({ "ok": false, "message": message, "data": data })),
        )
            .into_response()
    }
}

#[derive(Debug, Deserialize)]
pub struct ListCreationBody {
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    pub private: bool,
}

/// Upstream's `ListCreationSchema` / `ListUpdateSchema` bounds (a `400` with
/// the field named, as the schema validator answered).
fn check_list_fields(name: Option<&str>, description: Option<&str>) -> Result<(), ApiError> {
    if let Some(name) = name
        && (name.is_empty() || name.chars().count() > LIST_NAME_MAX)
    {
        return Err(ApiError::bad_request(format!(
            "name must be between 1 and {LIST_NAME_MAX} characters"
        )));
    }
    if let Some(description) = description
        && description.chars().count() > LIST_DESCRIPTION_MAX
    {
        return Err(ApiError::bad_request(format!(
            "description must be at most {LIST_DESCRIPTION_MAX} characters"
        )));
    }
    Ok(())
}

#[derive(Debug, Deserialize)]
pub struct AccessBody {
    pub permission: String,
    pub grantee: String,
}

fn parse_access(body: &AccessBody) -> Result<(Permission, String), ApiError> {
    let permission = Permission::parse(&body.permission)
        .ok_or_else(|| ApiError::bad_request("permission must be view or edit"))?;
    let grantee = body.grantee.trim().to_lowercase();
    if grantee != GRANTED_TO_ALL && !is_address(&grantee) {
        return Err(ApiError::bad_request(
            "grantee must be an Ethereum address or *",
        ));
    }
    Ok((permission, grantee))
}

fn require_uuid(id: &str) -> Result<(), ApiError> {
    if is_uuid(id) {
        Ok(())
    } else {
        Err(ApiError::bad_request("The list id must be a UUID."))
    }
}

async fn get_list_inner(
    state: &MarketplaceState,
    id: &str,
    headers: &HeaderMap,
) -> Result<Response, ListsError> {
    let user = optional_signer(headers, "get", &format!("/v1/lists/{id}")).await?;
    require_uuid(id)?;
    let list = get_list(
        &state.pool,
        id,
        user.as_deref(),
        Some(Permission::View),
        true,
    )
    .await?
    .ok_or_else(|| ListsError::ListNotFound(id.to_string()))?;
    Ok(ok(list))
}

/// `GET /v1/lists/{id}` (signature optional): the owner, a grantee, or anyone for a public list.
pub async fn get_list_route(
    State(state): State<SharedState>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Response {
    get_list_inner(&state, &id, &headers)
        .await
        .unwrap_or_else(IntoResponse::into_response)
}

async fn create_list_inner(
    state: &MarketplaceState,
    headers: &HeaderMap,
    body: Option<ListCreationBody>,
) -> Result<Response, ListsError> {
    let user = authenticate(headers, "post", "/v1/lists").await?;
    let body = body.ok_or_else(|| ApiError::bad_request("name and private are required"))?;
    check_list_fields(Some(&body.name), body.description.as_deref())?;
    let list = add_list(
        &state.pool,
        &body.name,
        body.description.as_deref(),
        &user,
        body.private,
    )
    .await?;
    Ok((
        StatusCode::CREATED,
        Json(serde_json::json!({ "ok": true, "data": list })),
    )
        .into_response())
}

/// `POST /v1/lists` (signed): `{name, description?, private}`; `201` with the list.
pub async fn create_list(
    State(state): State<SharedState>,
    headers: HeaderMap,
    body: Option<Json<ListCreationBody>>,
) -> Response {
    create_list_inner(&state, &headers, body.map(|Json(b)| b))
        .await
        .unwrap_or_else(IntoResponse::into_response)
}

async fn update_list_inner(
    state: &MarketplaceState,
    id: &str,
    headers: &HeaderMap,
    body: Option<ListUpdate>,
) -> Result<Response, ListsError> {
    let user = authenticate(headers, "put", &format!("/v1/lists/{id}")).await?;
    if is_default_list(id) {
        return Err(ApiError::bad_request(DEFAULT_LIST_IMMUTABLE_MESSAGE).into());
    }
    require_uuid(id)?;
    let body = body
        .ok_or_else(|| ApiError::bad_request("one of name, description or private is required"))?;
    if body.name.is_none() && body.description.is_none() && body.private.is_none() {
        return Err(
            ApiError::bad_request("one of name, description or private is required").into(),
        );
    }
    check_list_fields(body.name.as_deref(), body.description.as_deref())?;
    let list = update_list(&state.pool, id, &user, &body).await?;
    Ok(ok(list))
}

/// `PUT /v1/lists/{id}` (signed): the name, description and/or privacy of the caller's list.
pub async fn update_list_route(
    State(state): State<SharedState>,
    Path(id): Path<String>,
    headers: HeaderMap,
    body: Option<Json<ListUpdate>>,
) -> Response {
    update_list_inner(&state, &id, &headers, body.map(|Json(b)| b))
        .await
        .unwrap_or_else(IntoResponse::into_response)
}

async fn delete_list_inner(
    state: &MarketplaceState,
    id: &str,
    headers: &HeaderMap,
) -> Result<Response, ListsError> {
    let user = authenticate(headers, "delete", &format!("/v1/lists/{id}")).await?;
    delete_list(&state.pool, id, &user).await?;
    Ok(Json(serde_json::json!({ "ok": true })).into_response())
}

/// `DELETE /v1/lists/{id}` (signed): the caller's list, its picks and grants with it.
pub async fn delete_list_route(
    State(state): State<SharedState>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Response {
    delete_list_inner(&state, &id, &headers)
        .await
        .unwrap_or_else(IntoResponse::into_response)
}

#[derive(Debug, Deserialize)]
pub struct AddPickBody {
    #[serde(rename = "itemId")]
    pub item_id: String,
}

async fn add_pick_inner(
    state: &MarketplaceState,
    id: &str,
    headers: &HeaderMap,
    body: Option<AddPickBody>,
) -> Result<Response, ListsError> {
    let user = authenticate(headers, "post", &format!("/v1/lists/{id}/picks")).await?;
    let body = body
        .filter(|b| !b.item_id.is_empty())
        .ok_or_else(|| ApiError::bad_request("itemId is required"))?;
    require_uuid(id)?;
    let pick = add_pick_to_list(state, id, &body.item_id.to_lowercase(), &user).await?;
    state.catalog_cache.invalidate();
    Ok((
        StatusCode::CREATED,
        Json(serde_json::json!({ "ok": true, "data": pick })),
    )
        .into_response())
}

/// `POST /v1/lists/{id}/picks` (signed): `{itemId}` into a list the caller may edit.
pub async fn add_pick_to_list_route(
    State(state): State<SharedState>,
    Path(id): Path<String>,
    headers: HeaderMap,
    body: Option<Json<AddPickBody>>,
) -> Response {
    add_pick_inner(&state, &id, &headers, body.map(|Json(b)| b))
        .await
        .unwrap_or_else(IntoResponse::into_response)
}

async fn delete_pick_inner(
    state: &MarketplaceState,
    id: &str,
    item_id: &str,
    headers: &HeaderMap,
) -> Result<Response, ListsError> {
    let user = authenticate(
        headers,
        "delete",
        &format!("/v1/lists/{id}/picks/{item_id}"),
    )
    .await?;
    delete_pick_in_list(&state.pool, id, &item_id.to_lowercase(), &user).await?;
    state.catalog_cache.invalidate();
    Ok(Json(serde_json::json!({ "ok": true })).into_response())
}

/// `DELETE /v1/lists/{id}/picks/{item_id}` (signed): the caller's own pick in that list.
pub async fn delete_pick_in_list_route(
    State(state): State<SharedState>,
    Path((id, item_id)): Path<(String, String)>,
    headers: HeaderMap,
) -> Response {
    delete_pick_inner(&state, &id, &item_id, &headers)
        .await
        .unwrap_or_else(IntoResponse::into_response)
}

async fn access_inner(
    state: &MarketplaceState,
    id: &str,
    headers: &HeaderMap,
    body: Option<AccessBody>,
    grant: bool,
) -> Result<Response, ListsError> {
    let method = if grant { "post" } else { "delete" };
    let owner = authenticate(headers, method, &format!("/v1/lists/{id}/access")).await?;
    let body = body.ok_or_else(|| ApiError::bad_request("permission and grantee are required"))?;
    let (permission, grantee) = parse_access(&body)?;
    if grant {
        if is_default_list(id) {
            return Err(ListsError::ListNotFound(id.to_string()));
        }
        create_access(&state.pool, id, permission, &grantee, &owner).await?;
        Ok((StatusCode::CREATED, Json(serde_json::json!({ "ok": true }))).into_response())
    } else {
        delete_access(&state.pool, id, permission, &grantee, &owner).await?;
        Ok(Json(serde_json::json!({ "ok": true })).into_response())
    }
}

/// `POST /v1/lists/{id}/access` (signed, the owner): `{permission: view|edit, grantee: address|*}`.
pub async fn create_access_route(
    State(state): State<SharedState>,
    Path(id): Path<String>,
    headers: HeaderMap,
    body: Option<Json<AccessBody>>,
) -> Response {
    access_inner(&state, &id, &headers, body.map(|Json(b)| b), true)
        .await
        .unwrap_or_else(IntoResponse::into_response)
}

/// `DELETE /v1/lists/{id}/access` (signed, the owner): revokes one grant.
pub async fn delete_access_route(
    State(state): State<SharedState>,
    Path(id): Path<String>,
    headers: HeaderMap,
    body: Option<Json<AccessBody>>,
) -> Response {
    access_inner(&state, &id, &headers, body.map(|Json(b)| b), false)
        .await
        .unwrap_or_else(IntoResponse::into_response)
}

#[derive(Debug, Serialize)]
pub struct PickersPage {
    pub results: Vec<ItemPicker>,
    pub total: i64,
    pub page: i64,
    pub pages: i64,
    pub limit: i64,
}

async fn picks_by_item_inner(
    state: &MarketplaceState,
    item_id: &str,
    headers: &HeaderMap,
    pairs: &[(String, String)],
) -> Result<Response, ApiError> {
    let user = optional_signer(headers, "get", &format!("/v1/picks/{item_id}")).await?;
    get_number_parameter("power", pairs)?;
    let pg = get_pagination_params(pairs);
    let (results, count) = picks_by_item(
        &state.pool,
        &item_id.to_lowercase(),
        user.as_deref(),
        pg.limit,
        pg.offset,
    )
    .await?;
    let page = if pg.limit > 0 {
        pg.offset / pg.limit
    } else {
        0
    };
    let pages = if !results.is_empty() && pg.limit > 0 {
        (count + pg.limit - 1) / pg.limit
    } else {
        0
    };
    let total = if results.is_empty() { 0 } else { count };
    Ok(ok(PickersPage {
        results,
        total,
        page,
        pages,
        limit: pg.limit,
    }))
}

/// `GET /v1/picks/{item_id}` (signature optional): who picked the item, the caller first.
pub async fn get_picks_by_item(
    State(state): State<SharedState>,
    Path(item_id): Path<String>,
    headers: HeaderMap,
    Query(pairs): Query<Vec<(String, String)>>,
) -> Response {
    picks_by_item_inner(&state, &item_id, &headers, &pairs)
        .await
        .unwrap_or_else(IntoResponse::into_response)
}

async fn item_pick_stats_inner(
    state: &MarketplaceState,
    item_id: &str,
    headers: &HeaderMap,
    pairs: &[(String, String)],
) -> Result<Response, ApiError> {
    let user = optional_signer(headers, "get", &format!("/v1/picks/{item_id}/stats")).await?;
    get_number_parameter("power", pairs)?;
    let stats = picks_stats(
        &state.pool,
        std::slice::from_ref(&item_id.to_lowercase()),
        user.as_deref(),
    )
    .await?;
    let first = stats.into_iter().next().unwrap_or(PickStats {
        count: 0,
        item_id: item_id.to_lowercase(),
        picked_by_user: user.map(|_| false),
    });
    Ok(ok(first))
}

/// `GET /v1/picks/{item_id}/stats` (signature optional): the item's pick count, and whether the caller picked it.
pub async fn get_item_pick_stats(
    State(state): State<SharedState>,
    Path(item_id): Path<String>,
    headers: HeaderMap,
    Query(pairs): Query<Vec<(String, String)>>,
) -> Response {
    item_pick_stats_inner(&state, &item_id, &headers, &pairs)
        .await
        .unwrap_or_else(IntoResponse::into_response)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn opts<'a>(item_id: Option<&'a str>, q: Option<&'a str>) -> GetListsOptions<'a> {
        GetListsOptions {
            limit: 24,
            offset: 0,
            sort_by: ListSortBy::CreatedAt,
            sort_direction: ListSortDirection::Desc,
            item_id,
            q,
        }
    }

    #[test]
    fn get_lists_sql_scopes_every_picks_read_to_the_caller_and_surfaces_the_wishlist() {
        let (sql, count_sql) = build_get_lists_sql(&opts(Some("0xitem-1"), None));
        for (i, _) in sql.match_indices("shop.picks") {
            assert!(
                sql[i..].contains("user_address = $1"),
                "unscoped picks read at byte {i}:\n{sql}"
            );
        }
        let arm = format!(
            "WHERE (l.user_address = $1 OR l.user_address = '{DEFAULT_LIST_USER_ADDRESS}')"
        );
        assert!(sql.contains(&arm) && count_sql.contains(&arm));
        assert!(sql.contains("ORDER BY is_default_list DESC, l.created_at DESC, l.id ASC"));
        assert!(sql.contains("(ARRAY_AGG(item_id ORDER BY created_at ASC))[:4] AS preview"));
        assert!(
            sql.contains("(SELECT COUNT(1) FROM shop.list_acl a"),
            "is_private derives from the ACL"
        );
        assert!(!sql.contains("l.is_private"));
        let (sql, count_sql) = build_get_lists_sql(&opts(Some("item"), Some("needle")));
        assert!(
            sql.contains("l.name ILIKE '%' || $2 || '%'")
                && sql.contains("ip.item_id = $3")
                && sql.contains("LIMIT $4 OFFSET $5")
        );
        assert!(count_sql.contains("$2"));
        assert!(
            NON_EDITABLE_LISTS_SQL.contains("NOT EXISTS")
                && !NON_EDITABLE_LISTS_SQL.contains("LEFT JOIN")
        );
        assert!(
            PICK_AND_UNPICK_IN_BULK_SQL
                .contains("ON CONFLICT (item_id, user_address, list_id) DO NOTHING")
        );
        assert!(
            PICKS_BY_LIST_SQL.contains("SELECT DISTINCT p.item_id, p.user_address, p.list_id,")
        );
    }

    #[test]
    fn envelopes_match_the_upstream_wire() {
        let row = FavoriteList {
            id: "6a0e4b1e-0f6e-4c7a-9d2b-2f1c9a1a0001".into(),
            name: "Summer fits".into(),
            description: Some("wearables I like".into()),
            user_address: "0x57d1721f6223eb434e20f5b4a88e494008ea0542".into(),
            created_at: 1_782_326_697_937,
            updated_at: Some(1_782_326_700_000),
            is_private: false,
            permission: Some("edit".into()),
            is_default_list: false,
            items_count: 3,
            preview_of_item_ids: vec!["0xf1483f042614105cb943d3dd67157256cd003028-15".into()],
            is_item_in_list: None,
        };
        assert_eq!(
            serde_json::to_value(&row).unwrap(),
            json!({ "id": "6a0e4b1e-0f6e-4c7a-9d2b-2f1c9a1a0001", "name": "Summer fits", "description": "wearables I like", "userAddress": "0x57d1721f6223eb434e20f5b4a88e494008ea0542", "createdAt": 1_782_326_697_937_i64, "updatedAt": 1_782_326_700_000_i64, "isPrivate": false, "permission": "edit", "isDefaultList": false, "itemsCount": 3, "previewOfItemIds": ["0xf1483f042614105cb943d3dd67157256cd003028-15"] })
        );
        let env = serde_json::to_value(picks_page(
            vec![ListPick {
                item_id: "0xa-1".into(),
                created_at: 5,
            }],
            5,
            2,
            2,
        ))
        .unwrap();
        assert_eq!(
            env,
            json!({ "results": [{ "itemId": "0xa-1", "createdAt": 5 }], "total": 5, "page": 1, "pages": 3, "limit": 2 })
        );
        let empty = picks_page(Vec::new(), 5, 2, 10);
        assert_eq!((empty.total, empty.pages, empty.page), (0, 0, 5));
        let stats = serde_json::to_value(PickStats {
            count: 0,
            item_id: "x".into(),
            picked_by_user: None,
        })
        .unwrap();
        assert_eq!(stats, json!({ "count": 0, "itemId": "x" }));
        assert!(
            is_uuid("6a0e4b1e-0f6e-4c7a-9d2b-2f1c9a1a0001")
                && !is_uuid("not-a-uuid")
                && !is_uuid("6a0e4b1e-0f6e-4c7a-9d2b-2f1c9a1a000g")
        );
        let b: PickUnpickInBulkBody =
            serde_json::from_value(json!({ "pickedFor": ["a"], "unpickedFrom": null })).unwrap();
        assert_eq!(b.picked_for.as_deref(), Some(&["a".to_string()][..]));
    }

    #[tokio::test]
    async fn the_404_payloads_carry_their_ids() {
        let ids = vec!["6a0e4b1e-0f6e-4c7a-9d2b-2f1c9a1a0001".to_string()];
        let resp = PicksError::ListsNotFound(ids.clone()).into_response();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
        let v: serde_json::Value = serde_json::from_slice(
            &axum::body::to_bytes(resp.into_body(), usize::MAX)
                .await
                .unwrap(),
        )
        .unwrap();
        assert_eq!(
            v,
            json!({ "ok": false, "message": LISTS_NOT_FOUND_MESSAGE, "data": { "listIds": ids } })
        );
        let resp = PicksError::ItemNotFound("0xf-15".into()).into_response();
        let v: serde_json::Value = serde_json::from_slice(
            &axum::body::to_bytes(resp.into_body(), usize::MAX)
                .await
                .unwrap(),
        )
        .unwrap();
        assert_eq!(
            v,
            json!({ "ok": false, "message": ITEM_NOT_FOUND_MESSAGE, "data": { "itemId": "0xf-15" } })
        );
        let resp = PicksError::from(ApiError::bad_request("nope")).into_response();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }
}
