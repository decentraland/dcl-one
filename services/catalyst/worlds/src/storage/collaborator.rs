//! `GET /collaborator`: the active scenes whose `metadata.logsPermissions`
//! lists the signer, self-scoped by the signed fetch (upstream
//! world-storage-service's collaborations listing). Derived at read time from
//! the local world scenes and the live Genesis scenes through
//! `worlds.logs_permissions` and its GIN indexes. Genesis membership also
//! requires the shared ownership-history predicate, applied before counting
//! and pagination; the grant is re-checked on every storage request.

use axum::Json;
use axum::extract::{OriginalUri, RawQuery, State};
use axum::http::{HeaderMap, Uri};
use serde_json::Value;
use sqlx::{PgPool, Row};

use crate::comms::world_identity::{Parcel, ParcelSet};
use crate::shared::identity::Address;

use super::auth::verify_request;
use super::errors::ApiError;
use super::handlers::{PaginationInfo, parse_pagination, query_params};
use super::{SharedState, auth_chain_to_api, signed_paths};

const MAX_TITLE_CHARS: usize = 255;

// `str::trim`'s Unicode White_Space characters, also used by Parcel::parse.
// Binding these to btrim keeps SQL eligibility identical to the Rust parser.
const PARCEL_WHITESPACE: &str = "\u{0009}\u{000a}\u{000b}\u{000c}\u{000d}\u{0020}\u{0085}\u{00a0}\u{1680}\u{2000}\u{2001}\u{2002}\u{2003}\u{2004}\u{2005}\u{2006}\u{2007}\u{2008}\u{2009}\u{200a}\u{2028}\u{2029}\u{202f}\u{205f}\u{3000}";

/// One scene the signer collaborates on.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CollaboratorScene {
    pub scene_id: String,
    pub world_name: String,
    pub base_parcel: String,
    pub title: Option<String>,
    /// `world` or `genesis`.
    pub realm_kind: String,
}

/// Body of `GET /collaborator`.
#[derive(Debug, Clone, serde::Serialize)]
pub struct CollaboratorPage {
    pub data: Vec<CollaboratorScene>,
    pub pagination: PaginationInfo,
}

/// Upstream's title rule: a string without control characters, trimmed,
/// at most 255 characters; anything else is `null`.
fn normalize_title(raw: Option<&str>) -> Option<String> {
    let Ok(Value::String(title)) = serde_json::from_str(raw?) else {
        return None;
    };
    let cleaned: String = title
        .chars()
        .filter(|c| !matches!(*c as u32, 0..=0x1f | 0x7f..=0x9f))
        .collect();
    let cleaned = cleaned.trim();
    (!cleaned.is_empty()).then(|| cleaned.chars().take(MAX_TITLE_CHARS).collect())
}

/// The signer's page, newest deployment first, and the exact total. Bases and
/// titles come back as raw JSON text so no stored escape can fail the read.
pub async fn list(
    pool: &PgPool,
    land: Option<&crate::land_authz::Settings>,
    signer: &Address,
    limit: i64,
    offset: i64,
) -> Result<(Vec<CollaboratorScene>, i64), ApiError> {
    let rows = sqlx::query(
        "WITH candidates AS (
             SELECT entity_id AS scene_id, world_name, 'world' AS realm_kind, CASE WHEN entity ->> 'timestamp' ~ '^[0-9]{1,15}$'
                         THEN (entity ->> 'timestamp')::double precision
                         ELSE extract(epoch FROM created_at) * 1000 END AS deployed_at,
                    parcels,
                    (entity #> '{metadata,scene,base}')::text AS base,
                    (entity #> '{metadata,display,title}')::text AS title
             FROM worlds.world_scenes
             WHERE mirror_source IS NULL AND cardinality(parcels) > 0
               AND worlds.logs_permissions((entity #> '{metadata,logsPermissions}')::text) @> ARRAY[$1::text]

             UNION ALL
             SELECT d.entity_id, 'main', 'genesis', extract(epoch FROM d.entity_timestamp) * 1000,
                    d.entity_pointers,
                    (d.entity_metadata #> '{scene,base}')::text,
                    (d.entity_metadata #> '{display,title}')::text
             FROM content.deployments d
             WHERE d.entity_type = 'scene' AND d.deleter_deployment IS NULL AND cardinality(d.entity_pointers) > 0
               AND worlds.logs_permissions((d.entity_metadata -> 'logsPermissions')::text) @> ARRAY[$1::text]

               AND CASE WHEN worlds.logs_permissions((d.entity_metadata -> 'logsPermissions')::text) @> ARRAY[$1::text]
                   THEN land_authz.scene_collaborators_current($4, $5, $6, d.entity_pointers, d.entity_timestamp)
                   ELSE false END
               AND EXISTS (SELECT 1 FROM content.active_pointers ap WHERE ap.entity_id = d.entity_id)
         ), matched AS (
             SELECT * FROM candidates c WHERE EXISTS (
                 SELECT 1 FROM unnest(c.parcels) p(pointer)
                 CROSS JOIN LATERAL (SELECT btrim(split_part(pointer, ',', 1), $7) AS x,
                                            btrim(split_part(pointer, ',', 2), $7) AS y) axes
                 WHERE cardinality(string_to_array(pointer, ',')) = 2
                   AND x ~ '^-?[0-9]+$' AND y ~ '^-?[0-9]+$'
                   AND pg_input_is_valid(x, 'integer') AND pg_input_is_valid(y, 'integer')
             )
         )
         SELECT t.total, m.scene_id, m.world_name, m.realm_kind, m.base, m.title, m.parcels
         FROM (SELECT count(*)::bigint AS total FROM matched) t
         LEFT JOIN LATERAL (
             SELECT * FROM matched ORDER BY deployed_at DESC, scene_id LIMIT $2 OFFSET $3
         ) m ON true",
    )
    .bind(signer)
    .bind(limit)
    .bind(offset)
    .bind(land.map(|s| s.network.as_str()))
    .bind(land.map(|s| s.land_registry.as_str()))
    .bind(land.map(|s| s.estate_registry.as_str()))
    .bind(PARCEL_WHITESPACE)
    .fetch_all(pool)
    .await?;
    let total = rows.first().map_or(0, |r| r.get("total"));
    let scenes = rows
        .iter()
        .filter_map(|r| {
            let scene_id: String = r.get::<Option<String>, _>("scene_id")?;
            let base: Option<String> = r.get("base");
            let declared: Option<String> = base.and_then(|b| serde_json::from_str(&b).ok());
            let stored: Vec<Option<String>> = r.get("parcels");
            let parcels: ParcelSet = stored
                .iter()
                .flatten()
                .filter_map(|p| Parcel::parse(p).ok())
                .collect();
            let base_parcel = parcels.effective_base(declared.as_deref())?.canonical();
            Some(CollaboratorScene {
                scene_id,
                world_name: r.get("world_name"),
                base_parcel,
                title: normalize_title(r.get::<Option<&str>, _>("title")),
                realm_kind: r.get("realm_kind"),
            })
        })
        .collect();
    Ok((scenes, total))
}

pub async fn get_collaborator(
    State(state): State<SharedState>,
    RawQuery(query): RawQuery,
    OriginalUri(original): OriginalUri,
    headers: HeaderMap,
    uri: Uri,
) -> Result<Json<CollaboratorPage>, ApiError> {
    let paths = signed_paths(&headers, &uri, Some(&original));
    let verified = verify_request(&headers, "get", &paths, state.eip1654.as_deref())
        .await
        .map_err(auth_chain_to_api)?;
    let p = parse_pagination(&query_params(query.as_deref()));
    let (data, total) = list(
        &state.pool,
        state.policy.land_authz_settings(),
        &verified.signer,
        p.limit,
        p.offset,
    )
    .await?;
    Ok(Json(CollaboratorPage {
        data,
        pagination: PaginationInfo {
            limit: p.limit,
            offset: p.offset,
            total,
        },
    }))
}

#[cfg(test)]
mod tests {
    use super::normalize_title;

    #[test]
    fn titles_follow_the_upstream_rule() {
        assert_eq!(
            normalize_title(Some(r#"" My\u0000 Scene\n ""#)).as_deref(),
            Some("My Scene")
        );
        assert_eq!(normalize_title(Some(r#""\t ""#)), None);
        assert_eq!(normalize_title(Some("42")), None);
        assert_eq!(normalize_title(None), None);
        let long = format!("\"{}\"", "é".repeat(300));
        assert_eq!(normalize_title(Some(&long)).unwrap().chars().count(), 255);
    }
}
