//! The places read port over `explore.place_indexed`: one pool, one statement
//! per page. The old component's writer pool, squid pool and interaction-table
//! probes are gone: v2 has one database, so the viewer's favorites and likes
//! always fold into the page statement, and the `owner` filter matches the
//! row's own owner plus the parcels the LAND index says the wallet holds or
//! operates (`Place/model.ts:186-197`; the parcel leg is empty until a chain
//! reader lands).

pub mod admin;
pub mod content_quality;
pub mod derive;
pub mod destinations;
pub mod handlers;
pub mod interactions;
pub mod identity;
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
pub mod voting_power;
pub mod world_scenes;

use std::collections::HashMap;
use std::sync::{Arc, OnceLock};
use std::time::Duration;

use sqlx::{PgPool, Row};

use crate::sanitize::ContentOrigin;
use crate::shared::cache::{TtlCache, cache_get, cache_put};
use crate::shared::errors::ApiError;
use query::{
    Bind, bind_param, build_active_lead, build_live_user_count_order, build_order_by,
    build_where_from, destinations_lead_prefix, destinations_ranking_prefix,
    like_score_recount_join, order_tail,
};
pub use rows::{
    ActiveRows, CategoryTarget, PlaceListFilters, PlaceOrderBy, PlaceRow, PlaceStatusRow, WorldRow,
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

    /// Current native estate membership for event authoring. An unattached or
    /// unpublished authority leaves the caller's known source facts unchanged.
    pub async fn estate_at(
        &self,
        conn: &mut sqlx::PgConnection,
        x: i32,
        y: i32,
    ) -> Result<Option<crate::land_authz::EstateMembership>, ApiError> {
        let Some(land) = self.land_authz.get() else {
            return Ok(None);
        };
        Ok(land.estate_at(conn, x, y).await?)
    }

    /// Indexed LAND owner, estate-owner and update-operator parcels that a
    /// catalogue place covers (`query::operated_positions_in_catalogue`). An
    /// unavailable index never turns creator-authored catalogue metadata into
    /// ownership authority.
    pub async fn operated_positions(&self, owner: &str) -> Result<Vec<String>, ApiError> {
        let Some(land) = self.land_authz.get() else {
            return Ok(vec![]);
        };
        let Some(mut snapshot) = land.ready_snapshot().await else {
            return Ok(vec![]);
        };
        Ok(query::operated_positions_in_catalogue(
            &mut *snapshot.tx,
            &land.settings.land_registry,
            &land.settings.estate_registry,
            owner,
        )
        .await?)
    }

    /// Authorization for attaching a catalogue place to a community. Genesis ownership
    /// comes from the fresh LAND authority (or its trusted fallback), never scene metadata.
    pub async fn owns_places(&self, place_ids: &[String], owner: &str) -> Result<bool, ApiError> {
        let keys: Vec<String> = place_ids.iter().map(|id| id.to_lowercase()).collect();
        let rows: Vec<(String, bool, String, Option<String>)> = sqlx::query_as(
            "SELECT CASE WHEN id = ANY($1) THEN id ELSE lower(world_name) END, \
             world, base_position, lower(COALESCE(raw->>'owner', creator_address)) FROM explore.place_indexed \
             WHERE id = ANY($1) OR lower(world_name) = ANY($1)"
        ).bind(&keys).fetch_all(&self.pool).await?;
        let mut owned = std::collections::HashSet::new();
        let mut parcels = Vec::new();
        let mut parcel_keys = Vec::new();
        for (key, world, base, world_owner) in rows {
            if world {
                if world_owner
                    .as_deref()
                    .is_some_and(|value| value.eq_ignore_ascii_case(owner))
                {
                    owned.insert(key);
                }
            } else if let Some((x, y)) = base
                .split_once(',')
                .and_then(|(x, y)| Some((x.parse::<i32>().ok()?, y.parse::<i32>().ok()?)))
            {
                parcels.push((x, y));
                parcel_keys.push(key);
            }
        }
        if !parcels.is_empty() {
            let land = self
                .land_authz
                .get()
                .ok_or_else(|| ApiError::http(503, "LAND ownership unavailable"))?;
            let rights = crate::land_authz::can_deploy_batch(land, &parcels, owner)
                .await
                .map_err(|_| ApiError::http(503, "LAND ownership unavailable"))?;
            for (key, right) in parcel_keys.into_iter().zip(rights) {
                if right.flags.owner {
                    owned.insert(key);
                }
            }
        }
        Ok(keys.iter().all(|key| owned.contains(key)))
    }

    /// One statement per page: rows plus the windowed total, falling back to a COUNT
    /// only when the page ran past the offset.
    pub async fn list_page(&self, f: &PlaceListFilters) -> Result<(Vec<PlaceRow>, i64), ApiError> {
        let Some((filters, scenes)) = self.normalized_scene_filters(f).await? else {
            return Ok((vec![], 0));
        };
        let f = &filters;
        if search_too_short(f.search.as_deref()) {
            return Ok((vec![], 0));
        }
        let source = self.scene_list_source(f, &scenes);
        // The live-positions lead is `most_active`'s alone, which re-sorts
        // and recounts its cut (`busiest_first`): no catalogue count for it.
        let with_total = !matches!(f.active, ActiveRows::Live(_));
        let (sql, binds, live_binds) = list_sql_from(f, with_total, source);
        let mut q = sqlx::query(sqlx::AssertSqlSafe(sql));
        for b in &binds {
            q = bind_param(q, b);
        }
        if let Some(s) = &f.search {
            q = q.bind(query::tsquery(s));
        }
        for b in &live_binds {
            q = bind_param(q, b);
        }
        if let Some(v) = &f.viewer {
            q = q.bind(v.clone());
        }
        let (limit, offset) = page_bounds(f);
        let rows = q.bind(limit).bind(offset).fetch_all(&self.pool).await?;
        let total = match rows.first() {
            Some(r) if with_total => r.try_get::<i64, _>("total_count")?,
            Some(_) => 0,
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
        if search_too_short(f.search.as_deref()) {
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
        // Both lookups run at once; the scene answer wins.
        let (scene, row) = tokio::join!(
            self.find_world_scene_place(place_id, viewer),
            self.find_row_by_id_for(place_id, viewer)
        );
        if let Some(scene) = scene? {
            return Ok(scene);
        }
        if let Some(row) = row? {
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

    /// The places-table rows an id set names, as `POST /api/places` lists them
    /// with the default query (`getPlaceListById.ts:53-65`): enabled rows only,
    /// world scenes included, at most 100.
    pub async fn find_by_ids_for(
        &self,
        ids: &[String],
        viewer: Option<&str>,
    ) -> Result<Vec<PlaceRow>, ApiError> {
        let filters = PlaceListFilters {
            ids: ids.to_vec(),
            places_table: true,
            limit: 100,
            viewer: viewer.map(str::to_lowercase),
            ..Default::default()
        };
        Ok(self.list_page(&filters).await?.0)
    }

    /// `PlaceModel.countByIds` (`Place/model.ts:579-592`): every places-table
    /// row an id names, disabled or not, plus the canonical scenes among the
    /// ids (rows of this node's own that upstream never had).
    pub async fn count_by_ids(&self, ids: &[String]) -> Result<i64, ApiError> {
        if ids.is_empty() {
            return Ok(0);
        }
        let (catalogue_ids, scene_ids) = self.partition_scene_ids(ids).await?;
        let catalogue: i64 = sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
            "SELECT count(*)::bigint FROM explore.place_indexed \
             WHERE id = ANY($1) AND NOT {}{}",
            query::WORLD_CATALOGUE_SQL,
            and_own_rows()
        )))
        .bind(&catalogue_ids)
        .fetch_one(&self.pool)
        .await?;
        Ok(catalogue + scene_ids.len() as i64)
    }

    /// `PlaceModel.findByIds` (`Place/model.ts:556-577`): the status columns,
    /// no `disabled` filter. Upstream read its places table, which never held
    /// the worlds catalogue, so the catalogue rows stay out here as they do
    /// in [`Self::count_by_ids`] (`data` and `total` agree).
    pub async fn find_by_ids_status(
        &self,
        ids: &[String],
    ) -> Result<Vec<PlaceStatusRow>, ApiError> {
        if ids.is_empty() {
            return Ok(vec![]);
        }
        let (catalogue_ids, scene_ids) = self.partition_scene_ids(ids).await?;
        let rows = sqlx::query(sqlx::AssertSqlSafe(format!(
            "SELECT id, disabled, \
             CASE WHEN jsonb_typeof(raw->'disabled_reason') = 'string' THEN raw->>'disabled_reason' END AS disabled_reason, \
             base_position, world, world_name FROM explore.place_indexed \
             WHERE id = ANY($1) AND NOT {}{}",
            query::WORLD_CATALOGUE_SQL,
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
                disabled_reason: r
                    .try_get::<Option<String>, _>("disabled_reason")
                    .unwrap_or(None),
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
                    disabled_reason: None,
                    base_position: row.base_position.clone(),
                    world: true,
                    world_name: row.world_name.clone(),
                }),
                None => status.push(PlaceStatusRow {
                    id,
                    disabled: true,
                    disabled_reason: None,
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
        // `Category/model.ts:34-51`: a LEFT JOIN over every place row, disabled
        // ones included; `target` narrows to `p.world IS true|false`.
        let world_filter = match target {
            CategoryTarget::Worlds => "WHERE p.world IS TRUE",
            CategoryTarget::Places => "WHERE p.world IS FALSE",
            CategoryTarget::All => "",
        };
        let sql = format!(
            "SELECT cat AS name, count(*)::bigint AS count \
             FROM explore.place_indexed p, unnest(p.categories) AS cat \
             {world_filter} \
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

    /// A world row by its id, or by its name folded as `WorldModel.findByWorldName`
    /// folds it (`World/model.ts:255-267`, the interaction routes' lookup), the
    /// catalogue row first; with the latest scene's fields
    /// (`fill_world_place_fields`) and the viewer's interactions. `GET
    /// /api/worlds/:id` applies `findByIdWithAggregates`' verbatim-id rule on
    /// top (`:333`, `handlers::get_world`).
    pub async fn find_world_by_id_for(
        &self,
        world_id: &str,
        viewer: Option<&str>,
    ) -> Result<Option<PlaceRow>, ApiError> {
        let sql = format!(
            "SELECT {}{} FROM explore.place_indexed \
             WHERE world IS TRUE AND (id = $1 OR lower(world_name) = lower($1)) \
             ORDER BY (id = $1) DESC, {} DESC LIMIT 1",
            place_columns(),
            viewer.map(|_| viewer_columns(2)).unwrap_or_default(),
            query::WORLD_CATALOGUE_SQL
        );
        let mut q = sqlx::query(sqlx::AssertSqlSafe(sql)).bind(world_id);
        if let Some(v) = viewer {
            q = q.bind(v.to_lowercase());
        }
        let mut row = q
            .fetch_optional(&self.pool)
            .await?
            .map(|r| row_to_place(r, self.content_origin()));
        if let Some(row) = row.as_mut() {
            self.fill_world_place_fields(std::slice::from_mut(row))
                .await?;
        }
        Ok(row)
    }

    /// Upstream's world rows take `image` (as a fallback), `contact_name`,
    /// `contact_email`, `creator_address`, `sdk` and `deployed_at` from the
    /// world's latest enabled place (`World/model.ts:54-64`, the lateral join of
    /// `:207-215` and `Destination/model.ts:39-49`). A mirrored world carries
    /// those values already; a row missing any of them reads them here from
    /// the newest enabled scene of the same world, one statement per page.
    pub async fn fill_world_place_fields(&self, rows: &mut [PlaceRow]) -> Result<(), ApiError> {
        fn incomplete(row: &PlaceRow) -> bool {
            row.image.is_none()
                || row.contact_name.is_none()
                || row.contact_email.is_none()
                || row.creator_address.is_none()
                || row.sdk.is_none()
                || row.deployed_at.is_none()
        }
        let names: Vec<String> = rows
            .iter()
            .filter(|row| row.is_world_catalogue_row() && incomplete(row))
            .filter_map(|row| row.world_name.as_deref().map(str::to_lowercase))
            .collect();
        if names.is_empty() {
            return Ok(());
        }
        let latest = sqlx::query(sqlx::AssertSqlSafe(format!(
            "SELECT DISTINCT ON (lower(world_name)) lower(world_name) AS name, \
             raw->>'image' AS image, raw->>'contact_name' AS contact_name, \
             raw->>'contact_email' AS contact_email, creator_address, raw->>'sdk' AS sdk, deployed_at \
             FROM explore.place_indexed \
             WHERE world IS TRUE AND id <> lower(world_name) AND lower(world_name) = ANY($1) \
             AND disabled IS FALSE{} \
             ORDER BY lower(world_name), deployed_at DESC NULLS LAST",
            and_own_rows()
        )))
        .bind(&names)
        .fetch_all(&self.pool)
        .await?;
        if latest.is_empty() {
            return Ok(());
        }
        let latest: HashMap<String, sqlx::postgres::PgRow> = latest
            .into_iter()
            .map(|r| (r.get::<String, _>("name"), r))
            .collect();
        for row in rows.iter_mut().filter(|row| row.is_world_catalogue_row()) {
            let Some(lp) = row
                .world_name
                .as_deref()
                .and_then(|name| latest.get(&name.to_lowercase()))
            else {
                continue;
            };
            let text = |col: &str| lp.try_get::<Option<String>, _>(col).unwrap_or(None);
            if row.image.is_none() {
                row.image = crate::sanitize::sanitize_image_url(
                    text("image").as_deref(),
                    self.content_origin(),
                );
            }
            if row.contact_name.is_none() {
                row.contact_name = text("contact_name");
            }
            if row.contact_email.is_none() {
                row.contact_email = text("contact_email");
            }
            if row.creator_address.is_none() {
                row.creator_address = text("creator_address");
            }
            if row.sdk.is_none() {
                row.sdk = text("sdk");
            }
            if row.deployed_at.is_none() {
                row.deployed_at = lp
                    .try_get::<Option<chrono::NaiveDateTime>, _>("deployed_at")
                    .unwrap_or(None)
                    .map(|n| n.and_utc());
            }
        }
        Ok(())
    }

    /// `WorldModel.findWorldNames` (`World/model.ts:434-443`): the listed,
    /// enabled catalogue worlds' names, ascending. Upstream also requires an
    /// enabled place of the world; a mirrored row met that upstream, a local
    /// one is written from its newest scene (`derive.rs`).
    pub async fn world_names(&self) -> Result<Vec<String>, ApiError> {
        let sql = format!(
            "SELECT DISTINCT world_name FROM explore.place_indexed \
             WHERE {} AND disabled IS FALSE AND world_name IS NOT NULL \
             AND {} IS TRUE{} ORDER BY 1",
            query::WORLD_CATALOGUE_SQL,
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

/// `places-repository/component.ts:175,206`: a search under three UTF-16
/// units answers nothing and counts nothing.
fn search_too_short(search: Option<&str>) -> bool {
    search.is_some_and(|s| s.encode_utf16().count() < 3)
}

#[cfg(test)]
mod search_gate_tests {
    use super::search_too_short;

    #[test]
    fn search_gate_counts_utf16_units() {
        assert!(search_too_short(Some("\u{f1}a")));
        assert!(!search_too_short(Some("\u{1f600}a")));
        assert!(!search_too_short(Some("abc")));
        assert!(!search_too_short(None));
    }
}

/// The page the routes clamp to their upstream caps (100 for places, worlds
/// and destinations, 500 for the map; this is only the ceiling any of them
/// needs) and an offset the like-score cut can add to it.
fn page_bounds(f: &PlaceListFilters) -> (i64, i64) {
    (f.limit.clamp(0, 500), f.offset.clamp(0, i64::MAX - 500))
}

/// SQL for `list_page`: `binds` go first, then the search term when set, then
/// `live_binds` (the live counts, then the active positions), then the
/// lowercased viewer when `f.viewer` is set, then the
/// page's limit and offset (`page_bounds`). A `page` CTE sorts and cuts the
/// keys; the columns are read for the page alone. `with_total` adds
/// `total_count`: a window count over the keys, or an uncorrelated COUNT of
/// the same predicate beside the per-leg cut.
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
    let order = f.order_by.order_expr();
    let dir = if f.order_desc { "DESC" } else { "ASC" };
    // `Place/model.ts:323-328,631`: `ts_rank_cd(textsearch, query) as rank`,
    // selected and sorted first when a search is set.
    let rank_expr = f.search.as_ref().map(|_| {
        format!(
            "ts_rank_cd(textsearch, to_tsquery('english', NULLIF(${}, '')))",
            binds.len() + 1
        )
    });
    let rank_prefix = rank_expr
        .as_ref()
        .map(|rank| format!("{rank} DESC, "))
        .unwrap_or_default();
    let rank_col = rank_expr
        .as_ref()
        .map(|rank| format!(", {rank} AS rank"))
        .unwrap_or_default();
    let search_count = if f.search.is_some() { 1 } else { 0 };
    let live_start = binds.len() + search_count + 1;
    let (live_prefix, mut live_binds) = build_live_user_count_order(f, live_start);
    let (active_prefix, active_binds) = build_active_lead(f, live_start + live_binds.len());
    live_binds.extend(active_binds);
    let order_clause = build_order_by(
        &format!(
            "{active_prefix}{}",
            destinations_lead_prefix(f, &live_prefix)
        ),
        destinations_ranking_prefix(f),
        &rank_prefix,
        order,
        dir,
        order_tail(f),
    );
    let like_score_top_n = matches!(f.order_by, PlaceOrderBy::LikeScore)
        && f.order_desc
        && f.search.is_none()
        && !f.destinations_mode
        && !f.viewer_favorites_only
        && f.ids.is_empty()
        && !source.scene_projection;
    let viewer_count = usize::from(f.viewer.is_some());
    let viewer_cols = match &f.viewer {
        Some(_) => viewer_columns(binds.len() + search_count + live_binds.len() + 1),
        None => String::new(),
    };
    let limit = binds.len() + search_count + live_binds.len() + viewer_count + 1;
    let offset = limit + 1;
    let sql = if like_score_top_n {
        let total = if with_total {
            format!(
                ", (SELECT count(*) FROM explore.place_indexed WHERE {where_clause}) AS total_count"
            )
        } else {
            String::new()
        };
        // Cut each leg of place_indexed to the page, order the page among those keys, then
        // read the columns of the page alone. Each cut sorts the leg's rows on the score
        // they answer, with the recounts joined beside them.
        format!(
            r#"
            WITH cand AS (
                (SELECT id FROM explore.place{place_join} WHERE ({where_clause}) AND world IS FALSE ORDER BY {order_clause} LIMIT {n})
                UNION ALL
                (SELECT id FROM explore.place{place_join} WHERE ({where_clause}) AND world IS TRUE ORDER BY {order_clause} LIMIT {n})
                UNION ALL
                (SELECT id FROM explore.place_world_local{local_join} WHERE {where_clause} ORDER BY {order_clause} LIMIT {n})
            ), page AS (
                SELECT id FROM explore.place_indexed{indexed_join}
                WHERE id = ANY (ARRAY(SELECT id FROM cand)) AND {where_clause}
                ORDER BY {order_clause}
                LIMIT ${limit} OFFSET ${offset}
            )
            SELECT {cols}{rank_col}{viewer_cols}{total}
            FROM explore.place_indexed{indexed_join}
            JOIN page USING (id)
            ORDER BY {order_clause}
            "#,
            cols = place_columns(),
            n = format!("${limit}::bigint + ${offset}::bigint"),
            place_join = like_score_recount_join(f.order_by, "place"),
            local_join = like_score_recount_join(f.order_by, "place_world_local"),
            indexed_join = like_score_recount_join(f.order_by, "place_indexed"),
        )
    } else {
        let (window, total) = if with_total {
            (", count(*) OVER () AS total_count", ", page.total_count")
        } else {
            ("", "")
        };
        // One pass over the keys sorts, counts and cuts the page; the columns are read for the
        // page alone. A scene source opens the WITH list itself.
        let cte_lead = if source.prefix.is_empty() {
            "WITH"
        } else {
            ","
        };
        format!(
            r#"
            {cte_lead} page AS (
                SELECT id{window}
                FROM {relation}{recount_join}
                WHERE {where_clause}
                ORDER BY {order_clause}
                LIMIT ${limit} OFFSET ${offset}
            )
            SELECT {projection}{cols}{rank_col}{viewer_cols}{total}
            FROM {relation}{recount_join}
            JOIN page USING (id)
            ORDER BY {order_clause}
            "#,
            cols = place_columns(),
            // A scene source is aliased `place_indexed` too.
            recount_join = like_score_recount_join(f.order_by, "place_indexed"),
        )
    };
    (format!("{}{sql}", source.prefix), binds, live_binds)
}

/// `query::own_rows_sql`, each as an ` AND` clause.
fn and_own_rows() -> String {
    query::own_rows_sql().map(|c| format!(" AND {c}")).collect()
}

/// A derive-era id as this node minted it: lowercase hyphenated UUIDv5.
/// Upstream matched ids verbatim (`Place/model.ts:549`), so any other
/// spelling stays the miss it was there.
fn is_parcel_alias(place_id: &str) -> bool {
    uuid::Uuid::parse_str(place_id)
        .map(|u| u.get_version_num() == 5 && u.hyphenated().to_string() == place_id)
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
        assert!(
            !is_parcel_alias("3AE8C888-1C33-5C77-B82F-AB14BD047F13"),
            "upstream matched ids verbatim; an upper-case id was a 404"
        );
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
        let recount = |row: &str| {
            format!(
                "LEFT JOIN (SELECT entity_id, like_score, imported FROM explore.place_interaction_counts WHERE like_score IS NOT NULL OR imported > 0) recount ON recount.entity_id = {row}.id"
            )
        };
        assert!(
            sql.contains(&format!("FROM explore.place {} WHERE (", recount("place")))
                && sql.contains(") AND world IS FALSE ORDER BY"),
            "{sql}"
        );
        assert!(sql.contains(") AND world IS TRUE ORDER BY"), "{sql}");
        assert!(
            sql.contains(&format!(
                "FROM explore.place_world_local {} WHERE",
                recount("place_world_local")
            )),
            "{sql}"
        );
        assert!(
            sql.contains(&format!(
                "FROM explore.place_indexed {}",
                recount("place_indexed")
            )),
            "{sql}"
        );
        assert!(
            sql.contains("WHERE id = ANY (ARRAY(SELECT id FROM cand)) AND"),
            "{sql}"
        );
        // Every sort, the three cuts, the page and the read, orders on the served score.
        assert_eq!(
            sql.matches("ORDER BY CASE WHEN recount.imported > 0 THEN recount.like_score ELSE COALESCE(recount.like_score, like_score_num) END DESC NULLS LAST, deployed_at DESC, id ASC")
                .count(),
            5,
            "{sql}"
        );
        assert!(!sql.contains("ORDER BY like_score_num"), "{sql}");
        assert_eq!(
            sql.matches("LIMIT $1::bigint + $2::bigint)").count(),
            3,
            "{sql}"
        );
        assert!(sql.contains("LIMIT $1 OFFSET $2"), "{sql}");

        let paged = PlaceListFilters {
            limit: 20,
            offset: i64::MAX,
            ..default_list()
        };
        assert_eq!(page_bounds(&paged), (20, i64::MAX - 500));
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
        let recount = "FROM explore.place_indexed LEFT JOIN (SELECT entity_id, like_score, imported FROM explore.place_interaction_counts WHERE like_score IS NOT NULL OR imported > 0) recount ON recount.entity_id = place_indexed.id\n";
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
            // A like-score sort over the whole relation reads the recounts beside the
            // rows; another order reads the relation alone.
            let like_score = matches!(f.order_by, PlaceOrderBy::LikeScore);
            assert_eq!(
                sql.matches(recount).count(),
                if like_score { 2 } else { 0 },
                "{sql}"
            );
            assert_eq!(
                sql.matches("FROM explore.place_indexed\n").count(),
                if like_score { 0 } else { 2 },
                "{sql}"
            );
            assert_eq!(
                sql.contains("CASE WHEN recount.imported > 0 THEN recount.like_score ELSE COALESCE(recount.like_score, like_score_num) END"),
                like_score,
                "{sql}"
            );
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
        assert!(
            sql.contains(") AS total_count") && sql.contains("LIMIT $4 OFFSET $5"),
            "{sql}"
        );
    }
}

pub mod removed;
