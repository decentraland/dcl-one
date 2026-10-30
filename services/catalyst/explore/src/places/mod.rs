//! The places read port over `explore.place_indexed`: one pool, one statement
//! per page. The old component's writer pool, squid pool and interaction-table
//! probes are gone: v2 has one database, so the viewer's favorites and likes
//! always fold into the page statement, and the `owner` filter (LAND
//! ownership from the squid index) answers empty until a chain reader lands.

pub mod admin;
pub mod content_quality;
pub mod derive;
pub mod destinations;
pub mod handlers;
pub mod interactions;
pub mod live;
pub mod mirror;
pub mod moderation;
pub mod query;
pub mod ranking;
pub mod reports;
pub mod rows;
pub mod scene_identity;
pub mod social;
pub mod v1;
pub mod world_scenes;

use std::collections::HashMap;
use std::sync::{Arc, OnceLock};
use std::time::Duration;

use sqlx::{PgPool, Row};

use crate::sanitize::ContentOrigin;
use crate::shared::cache::{TtlCache, cache_get, cache_put};
use crate::shared::errors::ApiError;
use query::{
    Bind, bind_param, build_live_user_count_order, build_order_by, build_where_from,
    description_plain_sql, destinations_lead_prefix, destinations_ranking_prefix, order_tail,
};
pub use rows::{
    CategoryTarget, PlaceListFilters, PlaceOrderBy, PlaceRow, PlaceStatusRow, WorldRow,
};
use rows::{place_columns, row_to_place, viewer_columns};

type CategoryCounts = Vec<(String, i64)>;
type ParcelAliases = Arc<HashMap<String, String>>;

const CATEGORY_CACHE_TTL: Duration = Duration::from_secs(60);
const PARCEL_ALIAS_TTL: Duration = Duration::from_secs(600);
const PARCEL_ALIAS_SQL: &str = "SELECT id, base_position FROM explore.place \
     WHERE world IS FALSE AND disabled IS FALSE \
     ORDER BY deployed_at DESC NULLS LAST, id";

/// The id a content-derived place carried: UUIDv5 of its base parcel. Links
/// minted under that scheme outlive the derived rows, so the read side resolves
/// the same id back to the parcel.
pub fn parcel_place_id(base_position: &str) -> String {
    uuid::Uuid::new_v5(&uuid::Uuid::NAMESPACE_URL, base_position.as_bytes()).to_string()
}

struct ListSource {
    prefix: String,
    relation: &'static str,
    scene_projection: bool,
    binds: Vec<Bind>,
}

impl ListSource {
    fn catalogue() -> Self {
        Self {
            prefix: String::new(),
            relation: "explore.place_indexed",
            scene_projection: false,
            binds: Vec::new(),
        }
    }
}

pub struct PlacesComponent {
    pool: PgPool,
    content_origin: Option<ContentOrigin>,
    content_public_url: String,
    worlds_mirror_configured: bool,
    category_cache: TtlCache<CategoryCounts>,
    parcel_alias_cache: TtlCache<ParcelAliases>,
    land_authz: OnceLock<Arc<crate::land_authz::LandAuthzState>>,
}

impl PlacesComponent {
    /// Thumbnails from `content_public_url` are exempt from the internal-host image filter.
    pub fn new(pool: PgPool, content_public_url: &str) -> Self {
        Self {
            pool,
            content_origin: ContentOrigin::parse(content_public_url),
            content_public_url: content_public_url.to_string(),
            worlds_mirror_configured: false,
            category_cache: Default::default(),
            parcel_alias_cache: Default::default(),
            land_authz: OnceLock::new(),
        }
    }

    pub fn with_worlds_mirror(mut self, configured: bool) -> Self {
        self.worlds_mirror_configured = configured;
        self
    }

    pub fn content_origin(&self) -> Option<&ContentOrigin> {
        self.content_origin.as_ref()
    }

    /// Binds the signer to a page query; false means the page is known empty
    /// (only_favorites without a signer).
    pub fn scope_to_viewer(
        &self,
        f: &mut PlaceListFilters,
        user: Option<&str>,
        only_favorites: bool,
    ) -> bool {
        f.viewer = user.map(str::to_lowercase);
        if !only_favorites {
            return true;
        }
        if user.is_none() {
            return false;
        }
        f.viewer_favorites_only = true;
        true
    }

    /// Attaches the shared LAND authority after its chain readers are initialized.
    pub fn set_land_authz(&self, land: Arc<crate::land_authz::LandAuthzState>) {
        let _ = self.land_authz.set(land);
    }

    /// The direct update-operator parcels used by the legacy lands-permissions
    /// route. A stale/uninitialized index does not supply permissions; the
    /// catalogue's independent owner leg still matches its known owned rows.
    pub async fn operated_positions(&self, owner: &str) -> Result<Vec<String>, ApiError> {
        let Some(land) = self.land_authz.get() else {
            return Ok(vec![]);
        };
        if land.ready().await.is_none() {
            return Ok(vec![]);
        }
        Ok(land
            .store()
            .parcels_with_update_operator(owner)
            .await?
            .into_iter()
            .map(|p| format!("{},{}", p.x, p.y))
            .collect())
    }

    /// One statement per page: rows plus the windowed total, falling back to a COUNT
    /// only when the page ran past the offset.
    pub async fn list_page(&self, f: &PlaceListFilters) -> Result<(Vec<PlaceRow>, i64), ApiError> {
        let Some((filters, scenes)) = self.normalized_scene_filters(f).await? else {
            return Ok((vec![], 0));
        };
        let f = &filters;
        if matches!(&f.search, Some(s) if s.len() < 3) {
            return Ok((vec![], 0));
        }
        let source = self.scene_list_source(f, &scenes);
        let (sql, binds, live_binds) = list_sql_from(f, true, source);
        let mut q = sqlx::query(sqlx::AssertSqlSafe(sql));
        for b in &binds {
            q = bind_param(q, b);
        }
        if let Some(s) = &f.search {
            q = q.bind(s.clone());
        }
        for b in &live_binds {
            q = bind_param(q, b);
        }
        if let Some(v) = &f.viewer {
            q = q.bind(v.clone());
        }
        let rows = q.fetch_all(&self.pool).await?;
        let total = match rows.first() {
            Some(r) => r.try_get::<i64, _>("total_count")?,
            None if f.offset > 0 || f.limit <= 0 => self.count_list(f).await?,
            None => 0,
        };
        let data: Vec<PlaceRow> = rows
            .into_iter()
            .filter_map(|r| {
                if r.try_get::<bool, _>("canonical_scene").unwrap_or(false) {
                    world_scenes::scene_row(r, self)
                } else {
                    Some(row_to_place(r, self.content_origin()))
                }
            })
            .collect();
        Ok((data, total))
    }

    pub async fn count_list(&self, f: &PlaceListFilters) -> Result<i64, ApiError> {
        let Some((filters, scenes)) = self.normalized_scene_filters(f).await? else {
            return Ok(0);
        };
        let f = &filters;
        if matches!(&f.search, Some(s) if s.len() < 3) {
            return Ok(0);
        }
        let source = self.scene_list_source(f, &scenes);
        let (where_clause, where_binds) =
            build_where_from(f, true, source.binds.len() + 1, source.scene_projection);
        let sql = format!(
            "{}SELECT count(*)::bigint AS total FROM {} WHERE {where_clause}",
            source.prefix, source.relation
        );
        let mut binds = source.binds;
        binds.extend(where_binds);
        let mut q = sqlx::query(sqlx::AssertSqlSafe(sql));
        for b in &binds {
            q = bind_param(q, b);
        }
        let row = q.fetch_one(&self.pool).await?;
        Ok(row.get::<i64, _>("total"))
    }

    async fn normalized_scene_filters(
        &self,
        f: &PlaceListFilters,
    ) -> Result<Option<(PlaceListFilters, Vec<String>)>, ApiError> {
        let mut normalized = f.clone();
        if f.ids.is_empty() {
            return Ok(Some((normalized, Vec::new())));
        }
        let (mut catalogue, scenes) = self.partition_scene_ids(&f.ids).await?;
        if f.only_worlds {
            if catalogue.is_empty() {
                return Ok(None);
            }
        } else {
            catalogue.extend(scenes.iter().cloned());
        }
        normalized.ids = catalogue;
        Ok(Some((normalized, scenes)))
    }

    pub async fn ping(&self) -> Result<(), ApiError> {
        sqlx::query("SELECT 1").fetch_one(&self.pool).await?;
        Ok(())
    }

    /// Lookup with the viewer's favorite/like folded into the statement. A miss
    /// on a derive-era id (`parcel_place_id`, UUIDv5) resolves to the row that
    /// serves the same base parcel today: those links outlive the derived rows in
    /// crawler indexes and bookmarks, and the parcel is the one thing both id
    /// schemes share.
    pub async fn find_by_id_for(
        &self,
        place_id: &str,
        viewer: Option<&str>,
    ) -> Result<Option<PlaceRow>, ApiError> {
        // A canonical scene UUID belongs to Places even after retirement.
        // Never let an unrelated upstream row revive or shadow that identity.
        if let Some(scene) = self.find_world_scene_place(place_id, viewer).await? {
            return Ok(scene);
        }
        if let Some(row) = self.find_row_by_id_for(place_id, viewer).await? {
            return Ok(Some(row));
        }
        match self.canonical_id_for_parcel_alias(place_id).await? {
            Some(canonical) => self.find_row_by_id_for(&canonical, viewer).await,
            None => Ok(None),
        }
    }

    async fn canonical_id_for_parcel_alias(
        &self,
        place_id: &str,
    ) -> Result<Option<String>, ApiError> {
        if !is_parcel_alias(place_id) {
            return Ok(None);
        }
        let aliases = match cache_get(&self.parcel_alias_cache, "all") {
            Some(aliases) => aliases,
            None => {
                let rows: Vec<(String, String)> = sqlx::query_as(PARCEL_ALIAS_SQL)
                    .fetch_all(&self.pool)
                    .await?;
                let aliases = Arc::new(parcel_aliases(rows));
                cache_put(
                    &self.parcel_alias_cache,
                    "all".to_string(),
                    aliases.clone(),
                    PARCEL_ALIAS_TTL,
                );
                aliases
            }
        };
        Ok(aliases.get(place_id).cloned())
    }

    async fn find_row_by_id_for(
        &self,
        place_id: &str,
        viewer: Option<&str>,
    ) -> Result<Option<PlaceRow>, ApiError> {
        let sql = format!(
            "SELECT {}{} FROM explore.place_indexed WHERE id = $1",
            place_columns(),
            viewer.map(|_| viewer_columns(2)).unwrap_or_default()
        );
        let mut q = sqlx::query(sqlx::AssertSqlSafe(sql)).bind(place_id);
        if let Some(v) = viewer {
            q = q.bind(v.to_lowercase());
        }
        Ok(q.fetch_optional(&self.pool)
            .await?
            .map(|r| row_to_place(r, self.content_origin())))
    }

    pub async fn find_by_ids_for(
        &self,
        ids: &[String],
        viewer: Option<&str>,
    ) -> Result<Vec<PlaceRow>, ApiError> {
        if ids.is_empty() {
            return Ok(vec![]);
        }
        let (catalogue_ids, scene_ids) = self.partition_scene_ids(ids).await?;
        let sql = format!(
            "SELECT {}{} FROM explore.place_indexed WHERE id = ANY($1){}",
            place_columns(),
            viewer.map(|_| viewer_columns(2)).unwrap_or_default(),
            and_own_rows()
        );
        let mut q = sqlx::query(sqlx::AssertSqlSafe(sql)).bind(&catalogue_ids);
        if let Some(v) = viewer {
            q = q.bind(v.to_lowercase());
        }
        let mut rows: Vec<PlaceRow> = q
            .fetch_all(&self.pool)
            .await?
            .into_iter()
            .map(|r| row_to_place(r, self.content_origin()))
            .collect();
        rows.extend(self.world_scene_places_by_ids(&scene_ids, viewer).await?);
        Ok(rows)
    }

    pub async fn find_by_ids_status(
        &self,
        ids: &[String],
    ) -> Result<Vec<PlaceStatusRow>, ApiError> {
        if ids.is_empty() {
            return Ok(vec![]);
        }
        let (catalogue_ids, scene_ids) = self.partition_scene_ids(ids).await?;
        let rows = sqlx::query(sqlx::AssertSqlSafe(format!(
            "SELECT id, disabled, base_position, world, world_name FROM explore.place_indexed WHERE id = ANY($1){}",
            and_own_rows()
        )))
        .bind(&catalogue_ids)
        .fetch_all(&self.pool)
        .await?;
        let mut status: Vec<PlaceStatusRow> = rows
            .into_iter()
            .map(|r| PlaceStatusRow {
                id: r.get::<String, _>("id"),
                disabled: r.get::<bool, _>("disabled"),
                world: r.get::<bool, _>("world"),
                world_name: r.try_get::<Option<String>, _>("world_name").unwrap_or(None),
                base_position: r.get::<String, _>("base_position"),
            })
            .collect();
        let visible: HashMap<String, PlaceRow> = self
            .world_scene_places_by_ids_scoped(&scene_ids, None, false)
            .await?
            .into_iter()
            .map(|row| (row.id.clone(), row))
            .collect();
        // A missing mirror copy is not a disabled scene. Only suppress live
        // mirrored rows; hidden/retired identities still return minimal status.
        let nonlocal: Vec<uuid::Uuid> = if crate::copies::origin::world_scenes_local_only() {
            let ids: Vec<uuid::Uuid> = scene_ids
                .iter()
                .filter_map(|id| uuid::Uuid::parse_str(id).ok())
                .collect();
            sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
                "SELECT p.identity {} WHERE p.identity = ANY($1) AND NOT {}",
                scene_identity::BOUND_WORLD_SCENE_FROM,
                scene_identity::LOCAL_WORLD_SCENE
            )))
            .bind(ids)
            .fetch_all(&self.pool)
            .await?
        } else {
            Vec::new()
        };
        for id in scene_ids {
            if visible.contains_key(&id)
                && uuid::Uuid::parse_str(&id).is_ok_and(|id| nonlocal.contains(&id))
            {
                continue;
            }
            match visible.get(&id) {
                Some(row) => status.push(PlaceStatusRow {
                    id,
                    disabled: false,
                    base_position: row.base_position.clone(),
                    world: true,
                    world_name: row.world_name.clone(),
                }),
                None => status.push(PlaceStatusRow {
                    id,
                    disabled: true,
                    base_position: String::new(),
                    world: true,
                    world_name: None,
                }),
            }
        }
        Ok(status)
    }

    pub async fn category_counts(
        &self,
        target: CategoryTarget,
    ) -> Result<Vec<(String, i64)>, ApiError> {
        let key = match target {
            CategoryTarget::All => "all",
            CategoryTarget::Places => "places",
            CategoryTarget::Worlds => "worlds",
        };
        if let Some(cached) = cache_get(&self.category_cache, key) {
            return Ok(cached);
        }
        let world_filter = match target {
            CategoryTarget::Worlds => "AND p.world IS TRUE",
            CategoryTarget::Places => "AND p.world IS FALSE",
            CategoryTarget::All => "",
        };
        let sql = format!(
            "SELECT cat AS name, count(*)::bigint AS count \
             FROM explore.place_indexed p, unnest(p.categories) AS cat \
             WHERE p.disabled IS FALSE {world_filter} \
             GROUP BY cat ORDER BY count DESC, name ASC"
        );
        let rows = sqlx::query(sqlx::AssertSqlSafe(sql))
            .fetch_all(&self.pool)
            .await?;
        let counts: Vec<(String, i64)> = rows
            .into_iter()
            .map(|r| (r.get::<String, _>("name"), r.get::<i64, _>("count")))
            .collect();
        cache_put(
            &self.category_cache,
            key.to_string(),
            counts.clone(),
            CATEGORY_CACHE_TTL,
        );
        Ok(counts)
    }

    pub async fn categories_for_place(&self, place_id: &str) -> Result<Vec<String>, ApiError> {
        if let Some(scene) = self.find_world_scene_place(place_id, None).await? {
            return Ok(scene.map(|p| p.categories).unwrap_or_default());
        }
        let row = sqlx::query("SELECT categories FROM explore.place_indexed WHERE id = $1")
            .bind(place_id)
            .fetch_optional(&self.pool)
            .await?;
        Ok(row
            .map(|r| {
                r.try_get::<Vec<String>, _>("categories")
                    .unwrap_or_default()
            })
            .unwrap_or_default())
    }

    pub async fn find_world_by_id_for(
        &self,
        world_id: &str,
        viewer: Option<&str>,
    ) -> Result<Option<PlaceRow>, ApiError> {
        let sql = format!(
            "SELECT {}{} FROM explore.place_indexed \
             WHERE world IS TRUE AND (id = $1 OR lower(world_name) = lower($1))",
            place_columns(),
            viewer.map(|_| viewer_columns(2)).unwrap_or_default()
        );
        let mut q = sqlx::query(sqlx::AssertSqlSafe(sql)).bind(world_id);
        if let Some(v) = viewer {
            q = q.bind(v.to_lowercase());
        }
        Ok(q.fetch_optional(&self.pool)
            .await?
            .map(|r| row_to_place(r, self.content_origin())))
    }

    pub async fn world_names(&self) -> Result<Vec<String>, ApiError> {
        let sql = format!(
            "SELECT DISTINCT world_name FROM explore.place_indexed \
             WHERE world IS TRUE AND world_name IS NOT NULL \
             AND {} IS TRUE{} ORDER BY 1",
            query::SHOW_IN_PLACES_SQL,
            and_own_rows()
        );
        let rows = sqlx::query(sqlx::AssertSqlSafe(sql))
            .fetch_all(&self.pool)
            .await?;
        Ok(rows
            .into_iter()
            .filter_map(|r| r.try_get::<Option<String>, _>("world_name").ok().flatten())
            .collect())
    }
}

/// SQL for `list_page`: `binds` go first, then the search term when set, then
/// `live_binds`, then the lowercased viewer when `f.viewer` is set. `with_total`
/// adds `total_count` (a window count, or an uncorrelated COUNT on the like-score
/// path whose candidate cut would undercount).
#[cfg(test)]
fn list_sql(f: &PlaceListFilters, with_total: bool) -> (String, Vec<Bind>, Vec<Bind>) {
    list_sql_from(f, with_total, ListSource::catalogue())
}

fn list_sql_from(
    f: &PlaceListFilters,
    with_total: bool,
    source: ListSource,
) -> (String, Vec<Bind>, Vec<Bind>) {
    let (where_clause, where_binds) =
        build_where_from(f, true, source.binds.len() + 1, source.scene_projection);
    let mut binds = source.binds;
    binds.extend(where_binds);
    let relation = source.relation;
    let projection = if source.scene_projection {
        "canonical_scene, "
    } else {
        ""
    };
    let order = f.order_by.column();
    let dir = if f.order_desc { "DESC" } else { "ASC" };
    let rank_prefix = if f.search.is_some() {
        format!(
            "ts_rank_cd(to_tsvector('english', coalesce(title,'') || ' ' || ({plain})), \
             plainto_tsquery('english', ${rank}), 32) DESC, ",
            plain = description_plain_sql(),
            rank = binds.len() + 1,
        )
    } else {
        String::new()
    };
    let search_count = if f.search.is_some() { 1 } else { 0 };
    let live_start = binds.len() + search_count + 1;
    let (live_prefix, live_binds) = build_live_user_count_order(f, live_start);
    let order_clause = build_order_by(
        &destinations_lead_prefix(f, &live_prefix),
        destinations_ranking_prefix(f),
        &rank_prefix,
        order,
        dir,
        order_tail(f),
    );
    // The routes clamp to their upstream page caps (100 for places, worlds and
    // destinations, 500 for the map); this is only the ceiling any of them needs.
    let limit = f.limit.clamp(0, 500);
    let offset = f.offset.max(0);
    let like_score_top_n = matches!(f.order_by, PlaceOrderBy::LikeScore)
        && f.order_desc
        && f.search.is_none()
        && !f.destinations_mode
        && !f.viewer_favorites_only
        && f.ids.is_empty()
        && !source.scene_projection;
    let viewer_cols = match &f.viewer {
        Some(_) => viewer_columns(binds.len() + search_count + live_binds.len() + 1),
        None => String::new(),
    };
    let sql = if like_score_top_n {
        let total = if with_total {
            format!(
                ", (SELECT count(*) FROM explore.place_indexed WHERE {where_clause}) AS total_count"
            )
        } else {
            String::new()
        };
        // Cut each leg of place_indexed to the page under its own partial index, then read only those rows.
        format!(
            r#"
            WITH cand AS (
                (SELECT id FROM explore.place WHERE ({where_clause}) AND world IS FALSE ORDER BY {order_clause} LIMIT {n})
                UNION ALL
                (SELECT id FROM explore.place WHERE ({where_clause}) AND world IS TRUE ORDER BY {order_clause} LIMIT {n})
                UNION ALL
                (SELECT id FROM explore.place_world_local WHERE {where_clause} ORDER BY {order_clause} LIMIT {n})
            )
            SELECT {cols}{viewer_cols}{total}
            FROM explore.place_indexed
            WHERE id = ANY (ARRAY(SELECT id FROM cand)) AND {where_clause}
            ORDER BY {order_clause}
            LIMIT {limit} OFFSET {offset}
            "#,
            cols = place_columns(),
            n = limit + offset,
        )
    } else {
        let total = if with_total {
            ", count(*) OVER () AS total_count"
        } else {
            ""
        };
        format!(
            r#"
            SELECT {projection}{cols}{viewer_cols}{total}
            FROM {relation}
            WHERE {where_clause}
            ORDER BY {order_clause}
            LIMIT {limit} OFFSET {offset}
            "#,
            cols = place_columns(),
        )
    };
    (format!("{}{sql}", source.prefix), binds, live_binds)
}

/// `query::own_rows_sql`, each as an ` AND` clause.
fn and_own_rows() -> String {
    query::own_rows_sql().map(|c| format!(" AND {c}")).collect()
}

fn is_parcel_alias(place_id: &str) -> bool {
    uuid::Uuid::parse_str(place_id)
        .map(|u| u.get_version_num() == 5)
        .unwrap_or(false)
}

/// Rows arrive newest deployment first, so the first row at a parcel wins; a row that
/// already carries its parcel id (a derived row) needs no alias.
fn parcel_aliases(rows: Vec<(String, String)>) -> HashMap<String, String> {
    let mut aliases = HashMap::new();
    for (id, base_position) in rows {
        let alias = parcel_place_id(&base_position);
        if alias != id {
            aliases.entry(alias).or_insert(id);
        }
    }
    aliases
}

#[cfg(test)]
mod parcel_alias_tests {
    use super::*;

    #[test]
    fn only_v5_ids_are_parcel_aliases() {
        assert!(is_parcel_alias("3ae8c888-1c33-5c77-b82f-ab14bd047f13"));
        assert!(!is_parcel_alias("fbbc9a64-4d74-4932-90e3-314ea8ca49f2"));
        assert!(!is_parcel_alias("harbour.dcl.eth"));
        assert!(!is_parcel_alias(""));
    }

    /// Pinned to a link production telemetry recorded as page_not_found once the mirror
    /// had retired the derived rows: the parcel 34,17 under the derive-era id.
    #[test]
    fn parcel_id_matches_the_links_the_derive_catalog_minted() {
        assert_eq!(
            parcel_place_id("34,17"),
            "3ae8c888-1c33-5c77-b82f-ab14bd047f13"
        );
        assert_eq!(
            parcel_place_id("-22,136"),
            "15a37659-def0-5880-a819-9ad6916d7735"
        );
    }

    #[test]
    fn the_newest_deployment_at_a_parcel_owns_its_alias() {
        let aliases = parcel_aliases(vec![
            ("newest".to_string(), "34,17".to_string()),
            ("older".to_string(), "34,17".to_string()),
            ("elsewhere".to_string(), "-22,136".to_string()),
        ]);
        assert_eq!(
            aliases.get("3ae8c888-1c33-5c77-b82f-ab14bd047f13"),
            Some(&"newest".to_string())
        );
        assert_eq!(
            aliases.get("15a37659-def0-5880-a819-9ad6916d7735"),
            Some(&"elsewhere".to_string())
        );
        assert_eq!(aliases.len(), 2);
    }

    #[test]
    fn a_derived_row_is_its_own_alias() {
        let derived = parcel_place_id("2,12");
        let aliases = parcel_aliases(vec![(derived.clone(), "2,12".to_string())]);
        assert!(aliases.is_empty(), "{aliases:?}");
    }
}

#[cfg(test)]
mod list_sql_tests {
    use super::*;

    fn default_list() -> PlaceListFilters {
        PlaceListFilters {
            order_by: PlaceOrderBy::LikeScore,
            order_desc: true,
            limit: 100,
            ..Default::default()
        }
    }

    #[test]
    fn like_score_lists_cut_each_leg_to_the_page_first() {
        let (sql, _, _) = list_sql(&default_list(), false);
        assert!(sql.contains("WITH cand AS ("), "{sql}");
        assert!(
            sql.contains("FROM explore.place WHERE (")
                && sql.contains(") AND world IS FALSE ORDER BY"),
            "{sql}"
        );
        assert!(sql.contains(") AND world IS TRUE ORDER BY"), "{sql}");
        assert!(
            sql.contains("FROM explore.place_world_local WHERE"),
            "{sql}"
        );
        assert!(
            sql.contains("WHERE id = ANY (ARRAY(SELECT id FROM cand)) AND"),
            "{sql}"
        );
        assert_eq!(sql.matches("LIMIT 100").count(), 4, "{sql}");

        let paged = PlaceListFilters {
            limit: 20,
            offset: 40,
            ..default_list()
        };
        let (sql, _, _) = list_sql(&paged, false);
        assert_eq!(sql.matches("LIMIT 60)").count(), 3, "{sql}");
        assert!(sql.contains("LIMIT 20 OFFSET 40"), "{sql}");
    }

    #[test]
    fn a_creator_filter_lands_inside_each_leg() {
        let f = PlaceListFilters {
            creator_address: Some("0x17A253C2ac0d5BA92cadBBF665e3390C9913dC5D".into()),
            ..default_list()
        };
        let (sql, binds, live_binds) = list_sql(&f, false);
        assert!(sql.contains("WITH cand AS ("), "{sql}");
        assert_eq!(
            sql.matches("LOWER(creator_address) = $1").count(),
            4,
            "{sql}"
        );
        assert!(live_binds.is_empty());
        match binds.as_slice() {
            [Bind::Text(addr)] => assert_eq!(addr, "0x17a253c2ac0d5ba92cadbbf665e3390c9913dc5d"),
            other => panic!("expected the lowercased creator as the only bind: {other:?}"),
        }
    }

    #[test]
    fn destinations_most_active_lists_live_users_above_the_featured_shelf() {
        let order_of = |order_by| {
            let f = PlaceListFilters {
                destinations_mode: true,
                order_by,
                place_user_counts: vec![("1,1".into(), 5)],
                ..default_list()
            };
            let (sql, _, _) = list_sql(&f, false);
            sql[sql.rfind("ORDER BY").unwrap()..].to_string()
        };
        let most_active = order_of(PlaceOrderBy::MostActive);
        assert!(
            most_active.find("CASE WHEN world THEN").unwrap()
                < most_active.find("highlighted DESC").unwrap(),
            "{most_active}"
        );
        let like_score = order_of(PlaceOrderBy::LikeScore);
        assert!(
            like_score.starts_with("ORDER BY highlighted DESC, "),
            "{like_score}"
        );
        assert!(!like_score.contains("CASE WHEN world THEN"), "{like_score}");
    }

    #[test]
    fn other_orders_read_place_indexed_directly_and_the_viewer_binds_last() {
        for f in [
            PlaceListFilters {
                order_desc: false,
                ..default_list()
            },
            PlaceListFilters {
                search: Some("tower".into()),
                ..default_list()
            },
            PlaceListFilters {
                order_by: PlaceOrderBy::MostActive,
                ..default_list()
            },
            PlaceListFilters {
                destinations_mode: true,
                ..default_list()
            },
        ] {
            let (sql, _, _) = list_sql(&f, false);
            assert!(!sql.contains("cand"), "{sql}");
            assert!(sql.contains("FROM explore.place_indexed\n"), "{sql}");
        }
        let f = PlaceListFilters {
            search: Some("tower".into()),
            viewer: Some("0xabc".into()),
            ..default_list()
        };
        let (sql, binds, _) = list_sql(&f, true);
        let viewer_idx = binds.len() + 2;
        assert!(
            sql.contains(&format!("lower(uf.\"user\") = ${viewer_idx}")),
            "{sql}"
        );
        assert!(sql.contains("count(*) OVER () AS total_count"), "{sql}");
    }
}

pub mod removed;
