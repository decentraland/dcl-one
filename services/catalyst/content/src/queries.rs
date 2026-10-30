//! The read queries of the content API, schema-qualified against
//! `content.*`. Every statement goes straight to Postgres: the old in-process
//! entity cache (bin/live/cache.rs, fed by the `new_deployment` NOTIFY) is not
//! copied yet; when it is, it must LISTEN to the trigger schema.sql already
//! installs, not re-create it.
//!
//! Timestamp columns are `timestamp without time zone` holding UTC and the
//! pool pins `TimeZone=UTC`, so `date_part('epoch', col) * 1000` is the wire
//! millisecond value and `to_timestamp($n / 1000.0)` the matching bound.

use bytes::Bytes;
use serde_json::{Value, json};
use sqlx::{AssertSqlSafe, PgPool};

use super::wire_types::{
    AuditInfo, ControllerDeployment, DeploymentContent, DeploymentsFilters,
    FailedDeploymentResponse, PointerChangeDelta, PointerChangesFilters, SnapshotMetadata,
    TimeRange,
};

pub const MAX_HISTORY_LIMIT: i64 = 500;

pub fn curate_limit(limit: Option<i64>) -> i64 {
    match limit {
        Some(l) if l > 0 && l <= MAX_HISTORY_LIMIT => l,
        _ => MAX_HISTORY_LIMIT,
    }
}

pub fn curate_offset(offset: Option<i64>) -> i64 {
    match offset {
        Some(o) if o >= 0 => o,
        _ => 0,
    }
}

/// An active entity as the JSON document a response carries, with its id
/// alongside so the denylist check needs no parse.
#[derive(Debug, Clone)]
pub struct EntityDoc {
    pub id: String,
    pub json: Bytes,
}

impl EntityDoc {
    pub fn from_value(value: &Value) -> Self {
        Self {
            id: value
                .get("id")
                .and_then(|id| id.as_str())
                .unwrap_or_default()
                .to_string(),
            json: Bytes::from(serde_json::to_vec(value).unwrap_or_default()),
        }
    }

    pub fn to_value(&self) -> Option<Value> {
        serde_json::from_slice(&self.json).ok()
    }

    /// `[doc,doc,...]` by concatenation, the same bytes `Json(Vec<Value>)` would produce.
    pub fn json_array(docs: &[Self]) -> Bytes {
        let mut out = Vec::with_capacity(docs.iter().map(|d| d.json.len() + 1).sum::<usize>() + 2);
        out.push(b'[');
        for (i, doc) in docs.iter().enumerate() {
            if i > 0 {
                out.push(b',');
            }
            out.extend_from_slice(&doc.json);
        }
        out.push(b']');
        Bytes::from(out)
    }
}

/// Fail closed: an entity carrying no string `id` cannot be checked against
/// the denylist, so it is dropped rather than served.
pub fn retain_non_denylisted(docs: &mut Vec<EntityDoc>, denylist: &super::Denylist) {
    docs.retain(|doc| !doc.id.is_empty() && !denylist.contains(&doc.id));
}

#[derive(Debug, sqlx::FromRow)]
struct ActiveEntityRow {
    entity_id: String,
    entity_type: String,
    entity_pointers: Vec<String>,
    entity_metadata: Option<Value>,
    entity_timestamp: f64,
    version: String,
    content_json: Value,
}

fn parse_content_json(v: &Value) -> Vec<(String, String)> {
    match v.as_array() {
        Some(arr) => arr
            .iter()
            .filter_map(|entry| {
                let key = entry.get("key")?.as_str()?;
                let hash = entry.get("hash")?.as_str()?;
                Some((key.to_string(), hash.to_string()))
            })
            .collect(),
        None => Vec::new(),
    }
}

/// The `entity_metadata` column wraps the deployed metadata as `{"v": ...}`.
fn unwrap_metadata(metadata: Option<&Value>) -> Option<Value> {
    metadata.and_then(|m| m.get("v").cloned())
}

fn row_to_doc(row: ActiveEntityRow) -> EntityDoc {
    let content: Vec<Value> = parse_content_json(&row.content_json)
        .into_iter()
        .map(|(key, hash)| json!({"file": key, "hash": hash}))
        .collect();
    let mut obj = json!({
        "version": row.version,
        "id": row.entity_id,
        "type": row.entity_type,
        "timestamp": row.entity_timestamp as i64,
        "pointers": row.entity_pointers,
        "content": content,
    });
    if let Some(m) = unwrap_metadata(row.entity_metadata.as_ref()) {
        obj["metadata"] = m;
    }
    EntityDoc::from_value(&obj)
}

// Content lists sort by file name under upstream's collation: upstream reads
// content_files with no ORDER BY, and the planner's scan of the (deployment,
// key) index hands the rows back in en_US order (a fresh deploy sits in its
// cache in the entity's own order until evicted). The heap (ctid) order this
// used to follow is the entity's order only until a prune leaves holes.
const CONTENT_ORDER: &str = r#"ORDER BY cf.key COLLATE "en_US.utf8""#;
const ACTIVE_ENTITY_PROJECTION: &str = r#"
    dep.entity_id,
    dep.entity_type,
    dep.entity_pointers,
    dep.entity_metadata,
    date_part('epoch', dep.entity_timestamp) * 1000 AS entity_timestamp,
    dep.version,
    COALESCE(
        (SELECT json_agg(json_build_object('key', cf.key, 'hash', cf.content_hash) ORDER BY cf.key COLLATE "en_US.utf8")
         FROM content.content_files cf WHERE cf.deployment = dep.id),
        '[]'::json
    ) AS content_json"#;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MaskedColumns {
    pub content: bool,
    pub metadata: bool,
}

impl MaskedColumns {
    pub(super) fn projection(self) -> String {
        if self.content {
            return ACTIVE_ENTITY_PROJECTION.to_string();
        }
        let metadata = if self.metadata {
            "dep.entity_metadata"
        } else {
            "NULL::json AS entity_metadata"
        };
        format!(
            r#"
    dep.entity_id,
    dep.entity_type,
    dep.entity_pointers,
    {metadata},
    date_part('epoch', dep.entity_timestamp) * 1000 AS entity_timestamp,
    dep.version,
    '[]'::json AS content_json"#
        )
    }
}

async fn entity_docs_by_pointers(
    pool: &PgPool,
    pointers: &[String],
    projection: &str,
) -> Result<Vec<EntityDoc>, sqlx::Error> {
    if pointers.is_empty() {
        return Ok(vec![]);
    }
    let lower: Vec<String> = pointers.iter().map(|p| p.to_lowercase()).collect();
    // The matching entity keys first, one per entity however many of its
    // pointers the request names, then the projection (metadata, files) once
    // per entity: a 100-parcel scene used to come back 100 times and be
    // deduplicated here. Ordered by each entity's smallest matching pointer
    // in byte order, so the answer does not depend on the plan or the
    // cluster's collation.
    let rows: Vec<ActiveEntityRow> = sqlx::query_as(AssertSqlSafe(format!(
        "WITH keys AS (
             SELECT ap.entity_id, min(ap.pointer COLLATE \"C\") AS first_pointer
             FROM content.active_pointers ap
             WHERE ap.pointer = ANY($1)
             GROUP BY ap.entity_id
         )
         SELECT {projection}
         FROM keys
         INNER JOIN content.deployments dep ON dep.entity_id = keys.entity_id
         WHERE dep.deleter_deployment IS NULL
         ORDER BY keys.first_pointer"
    )))
    .bind(&lower)
    .fetch_all(pool)
    .await?;
    Ok(rows.into_iter().map(row_to_doc).collect())
}

async fn entity_docs_by_ids(
    pool: &PgPool,
    ids: &[String],
    projection: &str,
) -> Result<Vec<EntityDoc>, sqlx::Error> {
    if ids.is_empty() {
        return Ok(vec![]);
    }
    let rows: Vec<ActiveEntityRow> = sqlx::query_as(AssertSqlSafe(format!(
        "SELECT {projection}
         FROM content.deployments dep
         WHERE dep.entity_id = ANY($1)
           AND dep.deleter_deployment IS NULL"
    )))
    .bind(ids)
    .fetch_all(pool)
    .await?;
    Ok(rows.into_iter().map(row_to_doc).collect())
}

pub async fn active_entity_docs_by_pointers(
    pool: &PgPool,
    pointers: &[String],
) -> Result<Vec<EntityDoc>, sqlx::Error> {
    entity_docs_by_pointers(pool, pointers, ACTIVE_ENTITY_PROJECTION).await
}

pub async fn active_entity_docs_by_ids(
    pool: &PgPool,
    ids: &[String],
) -> Result<Vec<EntityDoc>, sqlx::Error> {
    entity_docs_by_ids(pool, ids, ACTIVE_ENTITY_PROJECTION).await
}

pub async fn masked_entity_docs_by_pointers(
    pool: &PgPool,
    pointers: &[String],
    columns: MaskedColumns,
) -> Result<Vec<EntityDoc>, sqlx::Error> {
    entity_docs_by_pointers(pool, pointers, &columns.projection()).await
}

pub async fn masked_entity_docs_by_ids(
    pool: &PgPool,
    ids: &[String],
    columns: MaskedColumns,
) -> Result<Vec<EntityDoc>, sqlx::Error> {
    entity_docs_by_ids(pool, ids, &columns.projection()).await
}

#[derive(Debug, Clone)]
pub struct PrefixDocsResult {
    pub total: i64,
    pub entities: Vec<EntityDoc>,
}

fn escape_like(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_")
}

/// Entities whose active pointer starts with `prefix` (a collection URN);
/// `total` counts matching pointers, the page walks them in pointer order.
pub async fn active_entity_docs_by_prefix(
    pool: &PgPool,
    prefix: &str,
    offset: i64,
    limit: i64,
) -> Result<PrefixDocsResult, sqlx::Error> {
    let pattern = format!("{}%", escape_like(prefix));
    let total: i64 = sqlx::query_scalar(
        r"SELECT count(*) FROM content.active_pointers p WHERE p.pointer LIKE $1 ESCAPE '\'",
    )
    .bind(&pattern)
    .fetch_one(pool)
    .await?;
    let rows: Vec<ActiveEntityRow> = sqlx::query_as(AssertSqlSafe(format!(
        r"WITH ids AS (
              SELECT p.entity_id, row_number() OVER (ORDER BY p.pointer) AS ord
              FROM content.active_pointers p WHERE p.pointer LIKE $1 ESCAPE '\'
          )
          SELECT {ACTIVE_ENTITY_PROJECTION}
          FROM ids
          INNER JOIN content.deployments dep ON dep.entity_id = ids.entity_id
          WHERE ids.ord > $2 AND ids.ord <= $2 + $3
            AND dep.deleter_deployment IS NULL
          ORDER BY ids.ord"
    )))
    .bind(&pattern)
    .bind(offset)
    .bind(limit)
    .fetch_all(pool)
    .await?;
    Ok(PrefixDocsResult {
        total,
        entities: rows.into_iter().map(row_to_doc).collect(),
    })
}

pub async fn active_entity_ids_by_content_hash(
    pool: &PgPool,
    content_hash: &str,
) -> Result<Vec<String>, sqlx::Error> {
    sqlx::query_scalar(
        r#"
        SELECT deployment.entity_id
        FROM content.deployments AS deployment
        INNER JOIN content.content_files ON content_files.deployment = deployment.id
        WHERE content_hash = $1
          AND deployment.deleter_deployment IS NULL
        LIMIT 15000
        "#,
    )
    .bind(content_hash)
    .fetch_all(pool)
    .await
}

#[derive(Debug, Clone, Default)]
pub struct DeploymentQueryOptions {
    pub entity_types: Vec<String>,
    pub entity_ids: Vec<String>,
    pub pointers: Vec<String>,
    pub deployed_by: Vec<String>,
    pub from: Option<i64>,
    pub to: Option<i64>,
    pub only_currently_pointed: Option<bool>,
    pub fields: Vec<String>,
    pub sorting_field: Option<String>,
    pub sorting_order: Option<String>,
    pub offset: Option<i64>,
    pub limit: Option<i64>,
    pub last_id: Option<String>,
}

#[derive(Debug, Clone)]
pub struct PaginationResult {
    pub offset: i64,
    pub limit: i64,
    pub more_data: bool,
    pub last_id: Option<String>,
}

#[derive(Debug, Clone)]
pub struct DeploymentQueryResult {
    pub deployments: Vec<ControllerDeployment>,
    pub filters: DeploymentsFilters,
    pub pagination: PaginationResult,
}

fn sort_spec(
    sorting_field: Option<&str>,
    sorting_order: Option<&str>,
) -> (&'static str, &'static str) {
    let ts_col = match sorting_field {
        Some("entity_timestamp") => "entity_timestamp",
        _ => "local_timestamp",
    };
    let order = match sorting_order {
        Some("ASC") => "ASC",
        _ => "DESC",
    };
    (ts_col, order)
}

/// Keyset window on the sort column. A `lastId` cursor turns the bound in
/// the direction of travel into a `(timestamp, lower(entity_id))` tuple
/// comparison; the other bound stays a plain inequality. Returns the SQL
/// condition and the binds it consumes, in order.
///
/// The cursor's timestamp is the `lastId` row's own: rows carry microseconds
/// (`now()`) while the wire carries whole milliseconds, so comparing with
/// the millisecond alone re-reads, or skips, the rest of a page that ends
/// inside one flush's timestamp. The lookup stays within that millisecond
/// (its upper edge included, for a value the float conversion floored one
/// short) and falls back to the millisecond when no such row is there.
struct TimeBound {
    condition: String,
    binds: Vec<Bind>,
}

enum Bind {
    Text(String),
    Millis(f64),
}

fn time_bound(
    ts_col: &str,
    ms: i64,
    plain_cmp: &str,
    tuple: Option<(&str, &str)>,
    param_idx: &mut usize,
) -> TimeBound {
    let ms = ms as f64;
    match tuple {
        Some((cmp, last_id)) => {
            let next = *param_idx;
            let ts = *param_idx + 1;
            *param_idx += 2;
            TimeBound {
                condition: format!(
                    "(dep1.{ts_col}, LOWER(dep1.entity_id)) {cmp} (COALESCE((SELECT max(k.{ts_col}) FROM content.deployments k \
                     WHERE LOWER(k.entity_id) = LOWER(${next}) AND k.{ts_col} >= to_timestamp(${ts} / 1000.0) \
                     AND k.{ts_col} <= to_timestamp((${ts} + 1) / 1000.0)), to_timestamp(${ts} / 1000.0)), LOWER(${next}))"
                ),
                binds: vec![Bind::Text(last_id.to_string()), Bind::Millis(ms)],
            }
        }
        None => {
            let idx = *param_idx;
            *param_idx += 1;
            TimeBound {
                condition: format!("dep1.{ts_col} {plain_cmp} to_timestamp(${idx} / 1000.0)"),
                binds: vec![Bind::Millis(ms)],
            }
        }
    }
}

fn from_bound(
    ts_col: &str,
    from: i64,
    order: &str,
    last_id: Option<&str>,
    idx: &mut usize,
) -> TimeBound {
    let tuple = last_id.filter(|_| order == "ASC").map(|lid| (">", lid));
    time_bound(ts_col, from, ">=", tuple, idx)
}

fn to_bound(
    ts_col: &str,
    to: i64,
    order: &str,
    last_id: Option<&str>,
    idx: &mut usize,
) -> TimeBound {
    let tuple = last_id.filter(|_| order == "DESC").map(|lid| ("<", lid));
    time_bound(ts_col, to, "<=", tuple, idx)
}

pub async fn get_deployments(
    pool: &PgPool,
    options: &DeploymentQueryOptions,
) -> Result<DeploymentQueryResult, sqlx::Error> {
    let offset = curate_offset(options.offset);
    let limit = curate_limit(options.limit);
    let fetch_limit = limit + 1;

    let needs_audit = options.fields.iter().any(|f| f == "auditInfo");
    let needs_content = options.fields.iter().any(|f| f == "content");
    let (ts_col, order) = sort_spec(
        options.sorting_field.as_deref(),
        options.sorting_order.as_deref(),
    );

    let auth_select = if needs_audit {
        "dep1.auth_chain,"
    } else {
        "NULL::json AS auth_chain,"
    };
    let content_select = if needs_content {
        format!(
            "COALESCE((SELECT json_agg(json_build_object('key', cf.key, 'hash', cf.content_hash) {CONTENT_ORDER}) \
              FROM content.content_files cf WHERE cf.deployment = dep1.id), '[]'::json) AS content_json"
        )
    } else {
        "'[]'::json AS content_json".to_string()
    };

    let mut sql = format!(
        r#"
        SELECT
            dep1.entity_type,
            dep1.entity_id,
            dep1.entity_pointers,
            date_part('epoch', dep1.entity_timestamp) * 1000 AS entity_timestamp,
            dep1.entity_metadata,
            {auth_select}
            dep1.deployer_address,
            dep1.version,
            date_part('epoch', dep1.local_timestamp) * 1000 AS local_timestamp,
            {content_select}
        FROM content.deployments AS dep1
        "#
    );

    let mut conditions: Vec<String> = Vec::new();
    let mut binds: Vec<Bind> = Vec::new();
    let mut param_idx: usize = 1;
    let last_id = options.last_id.as_deref();

    if let Some(from) = options.from {
        let bound = from_bound(ts_col, from, order, last_id, &mut param_idx);
        conditions.push(bound.condition);
        binds.extend(bound.binds);
    }
    if let Some(to) = options.to {
        let bound = to_bound(ts_col, to, order, last_id, &mut param_idx);
        conditions.push(bound.condition);
        binds.extend(bound.binds);
    }
    if !options.entity_types.is_empty() {
        conditions.push(format!("dep1.entity_type = ANY(${param_idx})"));
        param_idx += 1;
    }
    if !options.entity_ids.is_empty() {
        conditions.push(format!("dep1.entity_id = ANY(${param_idx})"));
        param_idx += 1;
    }
    if options.only_currently_pointed == Some(true) {
        conditions.push("dep1.deleter_deployment IS NULL".into());
    }
    if !options.pointers.is_empty() {
        conditions.push(format!("dep1.entity_pointers && ${param_idx}"));
        param_idx += 1;
    }
    if !options.deployed_by.is_empty() {
        conditions.push(format!("LOWER(dep1.deployer_address) = ANY(${param_idx})"));
        param_idx += 1;
    }

    if !conditions.is_empty() {
        sql.push_str(" WHERE ");
        sql.push_str(&conditions.join(" AND "));
    }
    sql.push_str(&format!(
        " ORDER BY dep1.\"{ts_col}\" {order}, LOWER(dep1.entity_id) {order}"
    ));
    sql.push_str(&format!(" LIMIT ${param_idx} OFFSET ${}", param_idx + 1));

    #[derive(sqlx::FromRow)]
    struct DepRow {
        entity_type: String,
        entity_id: String,
        entity_pointers: Vec<String>,
        entity_timestamp: f64,
        entity_metadata: Option<Value>,
        deployer_address: String,
        version: String,
        auth_chain: Option<Value>,
        local_timestamp: f64,
        content_json: Value,
    }

    let mut query = sqlx::query_as::<_, DepRow>(AssertSqlSafe(sql));
    for bind in binds {
        query = match bind {
            Bind::Text(s) => query.bind(s),
            Bind::Millis(ms) => query.bind(ms),
        };
    }
    if !options.entity_types.is_empty() {
        query = query.bind(options.entity_types.clone());
    }
    if !options.entity_ids.is_empty() {
        query = query.bind(options.entity_ids.clone());
    }
    if !options.pointers.is_empty() {
        let lower: Vec<String> = options.pointers.iter().map(|p| p.to_lowercase()).collect();
        query = query.bind(lower);
    }
    if !options.deployed_by.is_empty() {
        let lower: Vec<String> = options
            .deployed_by
            .iter()
            .map(|a| a.to_lowercase())
            .collect();
        query = query.bind(lower);
    }
    query = query.bind(fetch_limit).bind(offset);

    let mut rows = query.fetch_all(pool).await?;
    let more_data = rows.len() as i64 > limit;
    rows.truncate(limit as usize);

    let deployments = rows
        .into_iter()
        .map(|d| {
            let content_items = parse_content_json(&d.content_json)
                .into_iter()
                .map(|(key, hash)| DeploymentContent { key, hash })
                .collect();
            ControllerDeployment {
                entity_version: d.version.clone(),
                entity_type: d.entity_type,
                entity_id: d.entity_id,
                entity_timestamp: d.entity_timestamp as i64,
                deployed_by: d.deployer_address,
                pointers: Some(d.entity_pointers),
                content: Some(content_items),
                metadata: unwrap_metadata(d.entity_metadata.as_ref()),
                audit_info: Some(AuditInfo {
                    version: d.version,
                    auth_chain: d.auth_chain.unwrap_or_else(|| Value::Array(vec![])),
                    local_timestamp: d.local_timestamp as i64,
                }),
                local_timestamp: d.local_timestamp as i64,
            }
        })
        .collect();

    Ok(DeploymentQueryResult {
        deployments,
        filters: DeploymentsFilters {
            pointers: options.pointers.clone(),
            entity_types: options.entity_types.clone(),
            entity_ids: options.entity_ids.clone(),
            from: options.from,
            to: options.to,
            only_currently_pointed: options.only_currently_pointed,
            deployed_by: options.deployed_by.clone(),
        },
        pagination: PaginationResult {
            offset,
            limit,
            more_data,
            last_id: options.last_id.clone(),
        },
    })
}

#[derive(Debug, Clone, Default)]
pub struct PointerChangesQueryOptions {
    pub entity_types: Vec<String>,
    pub from: Option<i64>,
    pub to: Option<i64>,
    pub include_auth_chain: bool,
    pub sorting_field: Option<String>,
    pub sorting_order: Option<String>,
    pub offset: Option<i64>,
    pub limit: Option<i64>,
    pub last_id: Option<String>,
}

#[derive(Debug, Clone)]
pub struct PointerChangesQueryResult {
    pub deltas: Vec<PointerChangeDelta>,
    pub filters: PointerChangesFilters,
    pub pagination: PaginationResult,
}

const POINTER_CHANGES_SELECT: &str = r#"
    SELECT
        dep1.id AS deployment_id,
        dep1.entity_type,
        dep1.entity_id,
        dep1.entity_pointers,
        date_part('epoch', dep1.local_timestamp) * 1000 AS local_timestamp,
        date_part('epoch', dep1.entity_timestamp) * 1000 AS entity_timestamp,
        dep1.deployer_address,
        dep1.version,
        dep1.auth_chain
    FROM content.deployments AS dep1
    "#;

pub async fn get_pointer_changes(
    pool: &PgPool,
    options: &PointerChangesQueryOptions,
) -> Result<PointerChangesQueryResult, sqlx::Error> {
    let offset = curate_offset(options.offset);
    let limit = curate_limit(options.limit);
    let fetch_limit = limit + 1;
    let (ts_col, order) = sort_spec(
        options.sorting_field.as_deref(),
        options.sorting_order.as_deref(),
    );

    let mut sql = String::from(POINTER_CHANGES_SELECT);
    let mut conditions: Vec<String> = Vec::new();
    let mut binds: Vec<Bind> = Vec::new();
    let mut param_idx: usize = 1;

    if let Some(from) = options.from {
        let bound = from_bound(
            ts_col,
            from,
            order,
            options.last_id.as_deref(),
            &mut param_idx,
        );
        conditions.push(bound.condition);
        binds.extend(bound.binds);
    }
    if let Some(to) = options.to {
        let bound = to_bound(
            ts_col,
            to,
            order,
            options.last_id.as_deref(),
            &mut param_idx,
        );
        conditions.push(bound.condition);
        binds.extend(bound.binds);
    }
    if !options.entity_types.is_empty() {
        conditions.push(format!("dep1.entity_type = ANY(${param_idx})"));
        param_idx += 1;
    }
    // A scene this node unpublished is not history a peer should replay:
    // the catalyst protocol has no unpublish, so a peer that synced it would
    // serve it as active. Leaving it out (as the snapshot windows do) makes a
    // fresh peer converge on this node's active set, the older scenes the
    // unpublish restored included; a peer that synced it before keeps it.
    conditions.push(
        "NOT EXISTS (SELECT 1 FROM content.local_entities le WHERE le.entity_id = dep1.entity_id AND le.tombstoned_at IS NOT NULL)"
            .to_string(),
    );
    if !conditions.is_empty() {
        sql.push_str(" WHERE ");
        sql.push_str(&conditions.join(" AND "));
    }
    sql.push_str(&format!(
        " ORDER BY dep1.\"{ts_col}\" {order}, LOWER(dep1.entity_id) {order}"
    ));
    sql.push_str(&format!(" LIMIT ${param_idx} OFFSET ${}", param_idx + 1));

    #[derive(sqlx::FromRow)]
    struct PointerChangeRow {
        deployment_id: i32,
        entity_type: String,
        entity_id: String,
        entity_pointers: Vec<String>,
        local_timestamp: f64,
        entity_timestamp: f64,
        deployer_address: String,
        version: String,
        auth_chain: Value,
    }

    let mut query = sqlx::query_as::<_, PointerChangeRow>(AssertSqlSafe(sql));
    for bind in binds {
        query = match bind {
            Bind::Text(s) => query.bind(s),
            Bind::Millis(ms) => query.bind(ms),
        };
    }
    if !options.entity_types.is_empty() {
        query = query.bind(&options.entity_types);
    }
    query = query.bind(fetch_limit).bind(offset);

    let mut rows = query.fetch_all(pool).await?;
    let more_data = rows.len() as i64 > limit;
    rows.truncate(limit as usize);

    let deltas = rows
        .into_iter()
        .map(|r| PointerChangeDelta {
            deployment_id: r.deployment_id as i64,
            entity_type: r.entity_type,
            entity_id: r.entity_id,
            pointers: r.entity_pointers,
            entity_timestamp: r.entity_timestamp as i64,
            deployer_address: r.deployer_address,
            version: r.version,
            auth_chain: r.auth_chain,
            local_timestamp: r.local_timestamp as i64,
        })
        .collect();

    Ok(PointerChangesQueryResult {
        deltas,
        filters: PointerChangesFilters {
            entity_types: options.entity_types.clone(),
            from: options.from,
            to: options.to,
            include_auth_chain: options.include_auth_chain,
        },
        pagination: PaginationResult {
            offset,
            limit,
            more_data,
            last_id: options.last_id.clone(),
        },
    })
}

/// `limit: None` returns every row (the unpaginated legacy answer).
pub async fn failed_deployments(
    pool: &PgPool,
    offset: i64,
    limit: Option<i64>,
) -> Result<Vec<FailedDeploymentResponse>, sqlx::Error> {
    #[derive(sqlx::FromRow)]
    struct Row {
        entity_id: String,
        entity_type: String,
        failure_timestamp: f64,
        reason: String,
        auth_chain: Value,
        error_description: String,
        snapshot_hash: String,
        retry_count: i32,
        next_retry_at: f64,
    }
    let rows: Vec<Row> = sqlx::query_as(
        r#"
        SELECT
            entity_id,
            entity_type,
            date_part('epoch', failure_time) * 1000 AS failure_timestamp,
            reason,
            auth_chain,
            error_description,
            snapshot_hash,
            retry_count,
            date_part('epoch', next_retry_at) * 1000 AS next_retry_at
        FROM content.failed_deployments
        ORDER BY failure_time, entity_id
        OFFSET $1 LIMIT $2
        "#,
    )
    .bind(offset.max(0))
    .bind(limit.map(|l| l.max(0)))
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(|r| FailedDeploymentResponse {
            entity_id: r.entity_id,
            entity_type: r.entity_type,
            failure_timestamp: r.failure_timestamp as i64,
            reason: r.reason,
            auth_chain: r.auth_chain,
            error_description: r.error_description,
            snapshot_hash: r.snapshot_hash,
            retry_count: r.retry_count,
            next_retry_at: r.next_retry_at as i64,
        })
        .collect())
}

pub fn provenance_json(
    signer: String,
    origin: String,
    published_at: f64,
    tombstoned_at: Option<f64>,
    superseded: bool,
) -> Value {
    let status = if tombstoned_at.is_some() {
        "unpublished"
    } else if superseded {
        "superseded"
    } else {
        "active"
    };
    json!({
        "signer": signer,
        "origin": origin,
        "publishedAt": published_at as i64,
        "tombstonedAt": tombstoned_at.map(|t| t as i64),
        "superseded": superseded,
        "status": status,
    })
}

/// The `/audit/{type}/{id}` document: version, auth chain and local
/// timestamp, plus `localProvenance` when the entity was published against
/// this node's write API.
pub async fn audit_info(pool: &PgPool, entity_id: &str) -> Result<Option<Value>, sqlx::Error> {
    #[derive(sqlx::FromRow)]
    struct AuditRow {
        version: String,
        auth_chain: Value,
        local_timestamp: f64,
        signer: Option<String>,
        origin: Option<String>,
        published_at: Option<f64>,
        tombstoned_at: Option<f64>,
        superseded: bool,
    }
    let row: Option<AuditRow> = sqlx::query_as(
        r#"
        SELECT d.version, d.auth_chain,
               date_part('epoch', d.local_timestamp) * 1000 AS local_timestamp,
               le.signer, le.origin,
               date_part('epoch', le.published_at) * 1000 AS published_at,
               date_part('epoch', le.tombstoned_at) * 1000 AS tombstoned_at,
               d.deleter_deployment IS NOT NULL AS superseded
        FROM content.deployments d
        LEFT JOIN content.local_entities le ON le.entity_id = d.entity_id
        WHERE d.entity_id = $1
        LIMIT 1
        "#,
    )
    .bind(entity_id)
    .fetch_optional(pool)
    .await?;

    Ok(row.map(|r| {
        let mut value = json!({
            "version": r.version,
            "authChain": r.auth_chain,
            "localTimestamp": r.local_timestamp as i64,
        });
        if let (Some(signer), Some(origin), Some(published_at)) =
            (r.signer, r.origin, r.published_at)
        {
            value["localProvenance"] =
                provenance_json(signer, origin, published_at, r.tombstoned_at, r.superseded);
        }
        value
    }))
}

/// Every generated snapshot, newest window first; a row without a hash (a
/// window still being written) lists with an empty hash, as before.
pub async fn all_snapshots(pool: &PgPool) -> Result<Vec<SnapshotMetadata>, sqlx::Error> {
    #[derive(sqlx::FromRow)]
    struct Row {
        hash: Option<String>,
        init_ts_ms: f64,
        end_ts_ms: f64,
        number_of_entities: i32,
        replaced_hashes: Vec<String>,
        gen_ts_ms: f64,
    }
    let rows: Vec<Row> = sqlx::query_as(
        r#"
        SELECT hash,
               date_part('epoch', init_timestamp) * 1000 AS init_ts_ms,
               date_part('epoch', end_timestamp) * 1000 AS end_ts_ms,
               number_of_entities,
               replaced_hashes,
               date_part('epoch', generation_time) * 1000 AS gen_ts_ms
        FROM content.snapshots
        ORDER BY end_timestamp DESC
        "#,
    )
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(|r| SnapshotMetadata {
            hash: r.hash.unwrap_or_default(),
            time_range: TimeRange {
                init_timestamp: r.init_ts_ms as i64,
                end_timestamp: r.end_ts_ms as i64,
            },
            number_of_entities: r.number_of_entities.max(0) as u64,
            replaced_snapshot_hashes: Some(r.replaced_hashes),
            generation_timestamp: r.gen_ts_ms as i64,
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pointer_changes_does_not_select_entity_metadata() {
        assert!(!POINTER_CHANGES_SELECT.contains("entity_metadata"));
        assert!(POINTER_CHANGES_SELECT.contains("dep1.auth_chain"));
        assert!(POINTER_CHANGES_SELECT.contains("content.deployments"));
    }

    #[test]
    fn keyset_bounds_follow_the_direction_of_travel() {
        let mut idx = 1;
        let b = to_bound("local_timestamp", 5, "DESC", Some("BafyX"), &mut idx);
        assert_eq!(
            b.condition,
            "(dep1.local_timestamp, LOWER(dep1.entity_id)) < (COALESCE((SELECT max(k.local_timestamp) \
             FROM content.deployments k WHERE LOWER(k.entity_id) = LOWER($1) \
             AND k.local_timestamp >= to_timestamp($2 / 1000.0) \
             AND k.local_timestamp <= to_timestamp(($2 + 1) / 1000.0)), \
             to_timestamp($2 / 1000.0)), LOWER($1))"
        );
        assert_eq!(idx, 3);
        assert!(
            matches!(&b.binds[..], [Bind::Text(s), Bind::Millis(ms)] if s == "BafyX" && *ms == 5.0)
        );

        let mut idx = 1;
        let b = to_bound("local_timestamp", 5, "ASC", Some("BafyX"), &mut idx);
        assert_eq!(
            b.condition,
            "dep1.local_timestamp <= to_timestamp($1 / 1000.0)"
        );
        assert_eq!(idx, 2);

        let mut idx = 3;
        let b = from_bound("entity_timestamp", 7, "ASC", Some("id"), &mut idx);
        assert_eq!(
            b.condition,
            "(dep1.entity_timestamp, LOWER(dep1.entity_id)) > (COALESCE((SELECT max(k.entity_timestamp) \
             FROM content.deployments k WHERE LOWER(k.entity_id) = LOWER($3) \
             AND k.entity_timestamp >= to_timestamp($4 / 1000.0) \
             AND k.entity_timestamp <= to_timestamp(($4 + 1) / 1000.0)), \
             to_timestamp($4 / 1000.0)), LOWER($3))"
        );
        let mut idx = 3;
        let b = from_bound("entity_timestamp", 7, "DESC", Some("id"), &mut idx);
        assert_eq!(
            b.condition,
            "dep1.entity_timestamp >= to_timestamp($3 / 1000.0)"
        );
    }

    #[test]
    fn entity_doc_json_array_matches_serde() {
        let a = EntityDoc::from_value(&json!({"id": "a", "x": 1}));
        let b = EntityDoc::from_value(&json!({"id": "b"}));
        let bytes = EntityDoc::json_array(&[a.clone(), b.clone()]);
        let expected = serde_json::to_vec(&json!([{"id": "a", "x": 1}, {"id": "b"}])).unwrap();
        assert_eq!(bytes.as_ref(), expected.as_slice());
        assert_eq!(EntityDoc::json_array(&[]).as_ref(), b"[]");
        assert_eq!(a.id, "a");
        assert_eq!(a.to_value().unwrap()["x"], 1);
    }

    #[test]
    fn provenance_status_follows_tombstone_then_supersession() {
        assert_eq!(
            provenance_json("s".into(), "o".into(), 1.0, None, false)["status"],
            "active"
        );
        assert_eq!(
            provenance_json("s".into(), "o".into(), 1.0, None, true)["status"],
            "superseded"
        );
        assert_eq!(
            provenance_json("s".into(), "o".into(), 1.0, Some(2.0), true)["status"],
            "unpublished"
        );
    }

    #[test]
    fn like_prefix_is_escaped() {
        assert_eq!(escape_like("a_b%c\\"), "a\\_b\\%c\\\\");
    }
}
