//! The LAND publish rules of the local write path.
//!
//! **Publish.** A scene deployed against this node's write API is local
//! authority: the write deployer admits it only when the signer holds a
//! deploy-granting leg on every parcel it points at (the content
//! validator's LAND leg, `land_authz::can_deploy_batch`: the index once its
//! fold is published, the marketplace subgraph's owner leg before that,
//! `Unavailable` when neither can answer), and the deploy function records
//! its provenance row (`content.local_entities`, signer lowercased, origin
//! `land-publish`) in the same statement as the deployment. Peer data never
//! creates a row. [`local_provenance`] is the row as `/audit` shows it.
//!
//! **Unpublish** ([`tombstone_and_repoint`], `DELETE /scenes/{coord}`):
//! only a locally published, not yet tombstoned scene can be taken down.
//! One call of the hash-named function `content.unpublish_scene_<hash>`
//! (defined in `schemas/00_content.sql`, [`unpublish_function_sql`]) finds
//! the scene active at the pointer, takes the pointer locks the deploy
//! function takes (every pointer of the scene, sorted in byte order), and
//! in one statement off that lock: tombstones the provenance row, clears
//! `deleter_deployment` on the rows the scene had superseded, re-points
//! each pointer the scene held to the newest remaining scene listing it
//! (never a tombstoned local one; the candidates are one GIN-served
//! `entity_pointers && <the scene's pointers>` probe, `metrics/unpublish.md`)
//! or deletes the pointer when nothing is underneath, and
//! wakes the `new_deployment` listeners. A concurrent write that moved the
//! parcel between the lookup and the lock makes the tombstone a no-op
//! ([`UnpublishError::Conflict`]).
//!
//! The functions of the write path live in the schema, not in the binary's
//! startup: the runtime role cannot create them (`deploy/nix/postgresql.nix`
//! grants USAGE + DML + EXECUTE only), so `schemas/00_content.sql` carries
//! them ([`content_functions_sql`], pinned by a test) and the binary checks
//! that the name it computes from its own SQL exists before calling it.

use serde_json::Value;
use sqlx::{PgPool, Row};

use super::write_deployer::deploy_function_sql;

/// The provenance row of `entity_id` as `/audit` renders it (`status`
/// `active`, `superseded` or `unpublished`); `None` for mirrored data.
pub async fn local_provenance(
    pool: &PgPool,
    entity_id: &str,
) -> Result<Option<Value>, sqlx::Error> {
    #[derive(sqlx::FromRow)]
    struct ProvenanceRow {
        signer: String,
        origin: String,
        published_at: f64,
        tombstoned_at: Option<f64>,
        superseded: bool,
    }
    let row: Option<ProvenanceRow> = sqlx::query_as(
        r#"
        SELECT le.signer, le.origin,
               date_part('epoch', le.published_at) * 1000 AS published_at,
               date_part('epoch', le.tombstoned_at) * 1000 AS tombstoned_at,
               COALESCE(d.deleter_deployment IS NOT NULL, false) AS superseded
        FROM content.local_entities le
        LEFT JOIN content.deployments d ON d.entity_id = le.entity_id
        WHERE le.entity_id = $1
        "#,
    )
    .bind(entity_id)
    .fetch_optional(pool)
    .await?;
    Ok(row.map(|r| {
        provenance_json(
            r.signer,
            r.origin,
            r.published_at,
            r.tombstoned_at,
            r.superseded,
        )
    }))
}

pub use super::queries::provenance_json;

#[derive(Debug)]
pub struct UnpublishOutcome {
    pub entity_id: String,
    /// Every pointer the scene held, with the entity now active there
    /// (`None`: the pointer was deleted, nothing underneath).
    pub repointed: Vec<(String, Option<String>)>,
}

#[derive(Debug, thiserror::Error)]
pub enum UnpublishError {
    #[error("no locally published scene at {0}")]
    NotLocal(String),
    #[error("a concurrent deployment changed the parcel; retry")]
    Conflict,
    /// This node cannot run the unpublish (its function is not installed).
    #[error("{0}")]
    Unavailable(String),
    #[error("{0}")]
    Db(String),
}

impl From<sqlx::Error> for UnpublishError {
    fn from(e: sqlx::Error) -> Self {
        UnpublishError::Db(e.to_string())
    }
}

/// The body of the unpublish function. `$1` is the pointer; `v_entity_id`
/// the scene active there, `v_pointers` its pointers.
fn unpublish_function_body() -> String {
    r#"#variable_conflict use_column
DECLARE v_entity_id text; v_pointers text[];
BEGIN
SELECT d.entity_id, d.entity_pointers INTO v_entity_id, v_pointers
  FROM content.active_pointers ap
  JOIN content.deployments d ON d.entity_id = ap.entity_id
  JOIN content.local_entities le ON le.entity_id = d.entity_id AND le.tombstoned_at IS NULL
 WHERE ap.pointer = $1 AND d.entity_type = 'scene';
IF NOT FOUND THEN RETURN; END IF;
PERFORM pg_advisory_xact_lock(hashtext(k)) FROM (SELECT k FROM (SELECT DISTINCT unnest(v_pointers) AS k) d ORDER BY k COLLATE "C") locks;
RETURN QUERY
WITH cur AS (
    SELECT d.id
    FROM content.active_pointers ap
    JOIN content.deployments d ON d.entity_id = ap.entity_id
    JOIN content.local_entities le ON le.entity_id = d.entity_id AND le.tombstoned_at IS NULL
    WHERE ap.pointer = $1 AND d.entity_type = 'scene' AND d.entity_id = v_entity_id
), tomb AS (
    UPDATE content.local_entities SET tombstoned_at = timezone('UTC', now())
    WHERE entity_id = v_entity_id AND tombstoned_at IS NULL AND EXISTS (SELECT 1 FROM cur)
    RETURNING entity_id
), restored AS (
    UPDATE content.deployments SET deleter_deployment = NULL
    WHERE deleter_deployment = (SELECT id FROM cur) AND EXISTS (SELECT 1 FROM tomb)
    RETURNING id
), held AS (
    SELECT ap.pointer
    FROM content.active_pointers ap
    WHERE ap.entity_id = v_entity_id AND EXISTS (SELECT 1 FROM tomb)
), candidates AS MATERIALIZED (
    SELECT d.entity_id, d.entity_pointers, d.entity_timestamp
    FROM content.deployments d
    WHERE d.entity_pointers && v_pointers
      AND d.entity_type = 'scene'
      AND d.entity_id <> v_entity_id
      AND (d.deleter_deployment IS NULL OR d.deleter_deployment = (SELECT id FROM cur))
      AND NOT EXISTS (
          SELECT 1 FROM content.local_entities le
          WHERE le.entity_id = d.entity_id AND le.tombstoned_at IS NOT NULL)
      AND EXISTS (SELECT 1 FROM tomb)
), replacement AS (
    SELECT DISTINCT ON (h.pointer) h.pointer, c.entity_id
    FROM held h
    JOIN candidates c ON c.entity_pointers @> ARRAY[h.pointer::text]
    ORDER BY h.pointer, c.entity_timestamp DESC, lower(c.entity_id) DESC
), repointed AS (
    UPDATE content.active_pointers ap SET entity_id = r.entity_id, entity_type = 'scene'
    FROM replacement r
    WHERE ap.pointer = r.pointer
    RETURNING ap.pointer
), removed AS (
    DELETE FROM content.active_pointers ap
    WHERE ap.entity_id = v_entity_id
      AND ap.pointer IN (SELECT pointer FROM held)
      AND NOT EXISTS (SELECT 1 FROM replacement r WHERE r.pointer = ap.pointer)
    RETURNING ap.pointer
), notified AS (
    SELECT pg_notify('new_deployment', 'scene:' || v_entity_id) AS sent
)
SELECT v_entity_id AS entity_id,
       (SELECT count(*) FROM tomb) AS tombstoned,
       h.pointer::text AS pointer,
       r.entity_id AS replacement
FROM (SELECT 1) AS marker
CROSS JOIN notified
LEFT JOIN held h ON true
LEFT JOIN replacement r ON r.pointer = h.pointer
ORDER BY h.pointer;
END"#
        .to_string()
}

/// Named by a hash of its body, like the deploy function: a binary calls
/// exactly the SQL it was built with, and two binaries mid-rollout meet
/// two functions.
pub fn unpublish_function_name() -> String {
    use sha2::Digest;
    let digest = sha2::Sha256::digest(unpublish_function_body());
    let hex: String = digest.iter().map(|b| format!("{b:02x}")).collect();
    format!("unpublish_scene_{}", &hex[..12])
}

pub fn unpublish_function_signature() -> String {
    format!("content.{}(text)", unpublish_function_name())
}

/// `CREATE [OR REPLACE] FUNCTION content.unpublish_scene_<hash>(text)`.
pub fn unpublish_function_sql(or_replace: bool) -> String {
    format!(
        "CREATE {}FUNCTION content.{}(text) \
         RETURNS TABLE (entity_id text, tombstoned bigint, pointer text, replacement text) \
         LANGUAGE plpgsql AS $unpublish$ {} $unpublish$",
        if or_replace { "OR REPLACE " } else { "" },
        unpublish_function_name(),
        unpublish_function_body()
    )
}

/// Every function of the local write path, as `schemas/00_content.sql`
/// carries them (`or_replace = false`, pinned by a test) and as
/// `catalyst schema functions` prints them for an existing database
/// (`or_replace = true`, idempotent, run as the administrative role).
pub fn content_functions_sql(or_replace: bool) -> String {
    [
        deploy_function_sql(false, or_replace),
        deploy_function_sql(true, or_replace),
        unpublish_function_sql(or_replace),
    ]
    .map(|statement| format!("{statement};\n"))
    .join("\n")
}

/// `Ok` once `signature` (a `to_regprocedure` spelling) exists; the error
/// names the function and how to install it, never a grant to ask for.
pub(crate) async fn require_function(pool: &PgPool, signature: &str) -> Result<(), String> {
    let installed: bool = sqlx::query_scalar("SELECT to_regprocedure($1) IS NOT NULL")
        .bind(signature)
        .fetch_one(pool)
        .await
        .map_err(|e| format!("checking {signature}: {e}"))?;
    if installed {
        Ok(())
    } else {
        Err(format!(
            "{signature} is not installed in this database: the write path's functions are part of \
             schemas/00_content.sql; on a database created before them, run `catalyst schema functions` \
             through psql as the administrative role"
        ))
    }
}

static UNPUBLISH_FN: tokio::sync::OnceCell<String> = tokio::sync::OnceCell::const_new();

async fn unpublish_function(pool: &PgPool) -> Result<&'static str, UnpublishError> {
    UNPUBLISH_FN
        .get_or_try_init(|| async {
            let signature = unpublish_function_signature();
            require_function(pool, &signature)
                .await
                .map_err(UnpublishError::Unavailable)?;
            Ok(unpublish_function_name())
        })
        .await
        .map(String::as_str)
}

/// Takes the local scene active at `pointer` down (module docs).
pub async fn tombstone_and_repoint(
    pool: &PgPool,
    pointer: &str,
) -> Result<UnpublishOutcome, UnpublishError> {
    let pointer = pointer.to_lowercase();
    let unpublish_fn = unpublish_function(pool).await?;
    let rows = sqlx::query(sqlx::AssertSqlSafe(format!(
        "SELECT entity_id, tombstoned, pointer, replacement FROM content.{unpublish_fn}($1)"
    )))
    .bind(&pointer)
    .fetch_all(pool)
    .await?;

    let Some(first) = rows.first() else {
        return Err(UnpublishError::NotLocal(pointer));
    };
    let entity_id: String = first.try_get("entity_id")?;
    let tombstoned: i64 = first.try_get("tombstoned")?;
    if tombstoned != 1 {
        return Err(UnpublishError::Conflict);
    }

    let mut repointed: Vec<(String, Option<String>)> = Vec::with_capacity(rows.len());
    for row in &rows {
        if let Some(pointer) = row.try_get::<Option<String>, _>("pointer")? {
            repointed.push((pointer, row.try_get("replacement")?));
        }
    }
    Ok(UnpublishOutcome {
        entity_id,
        repointed,
    })
}

#[cfg(test)]
mod tests;
