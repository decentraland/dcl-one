//! Discovery of the durable world-scene places used by storage. The current
//! deployment is joined by its exact world/entity binding, never by a cached
//! catalogue row or by a second overlap lookup. World aggregate listings and
//! upstream place IDs remain separate from this scene-level read surface.

use axum::Json;
use axum::extract::{OriginalUri, Path, Query, State};
use axum::http::{HeaderMap, Method};
use sqlx::Row;

use super::rows::{place_columns, row_to_place, viewer_columns};
use super::{PlaceRow, PlacesComponent};
use crate::SharedState;
use crate::shared::errors::{ApiDataTotal, ApiError};

// The same columns consumed by the catalogue's guarded projection and
// sanitizers. $3 is the configured public content URL, not scene input.
// Parent visibility always wins over a scene-specific moderation override.
const PARENT_VISIBILITY: &str = "NOT EXISTS (SELECT 1 FROM explore.place_indexed parent LEFT JOIN explore.place_moderation pm ON pm.place_id = parent.id WHERE parent.world AND lower(parent.world_name) = w.name AND (parent.disabled OR COALESCE(pm.disabled, false) OR NOT COALESCE(NULLIF(parent.raw->'show_in_places', 'null'::jsonb) = 'true'::jsonb, true)))";
const PARENT_PRESENT_OR_INCOMPLETE: &str = "(NOT EXISTS (SELECT 1 FROM explore.mirror_state WHERE mirror = 'worlds' AND last_complete_pass_at IS NOT NULL) OR EXISTS (SELECT 1 FROM explore.place_indexed parent WHERE parent.world AND lower(parent.world_name) = w.name))";

pub(super) fn scene_row(row: sqlx::postgres::PgRow, places: &PlacesComponent) -> Option<PlaceRow> {
    let mut place = row_to_place(row, places.content_origin());
    place.scene_place = true;
    let parse = |value: &str| -> Option<(i32, i32)> {
        let (x, y) = crate::auth::pointer::parse_pointer(value)?;
        Some((i32::try_from(x).ok()?, i32::try_from(y).ok()?))
    };
    // The same numeric ordering and i32 coordinate bounds as the room
    // resolver's ParcelSet, including mirrored, unsorted parcel arrays.
    let parcels: std::collections::BTreeSet<_> =
        place.positions.iter().filter_map(|p| parse(p)).collect();
    let base = parse(&place.base_position)
        .filter(|p| parcels.contains(p))
        .or_else(|| parcels.first().copied())?;
    place.base_position = format!("{},{}", base.0, base.1);
    place.positions = parcels
        .into_iter()
        .map(|(x, y)| format!("{x},{y}"))
        .collect();
    Some(place)
}

// Reject malformed footprints before pagination/counting. pg_input_is_valid
// never throws on oversized or malformed mirrored coordinates. The syntax
// excludes plus signs, matching dcl_auth::pointer::parse_pointer.
const VALID_PARCELS: &str = "EXISTS (SELECT 1 FROM unnest(s.parcels) AS parcel(value) \
    WHERE value ~ '^[[:space:]]*-?[0-9]+[[:space:]]*,[[:space:]]*-?[0-9]+[[:space:]]*$' \
    AND pg_input_is_valid(btrim(split_part(value, ',', 1)), 'integer') \
    AND pg_input_is_valid(btrim(split_part(value, ',', 2)), 'integer'))";

impl PlacesComponent {
    fn scene_rows_sql(&self, content_index: usize) -> String {
        let binding = super::scene_identity::BOUND_WORLD_SCENE_FROM;
        let local = super::scene_identity::LOCAL_WORLD_SCENE;
        let valid_parcels = VALID_PARCELS;
        let parent_visibility = PARENT_VISIBILITY;
        let content_parameter = format!("${content_index}");
        let mut sql = format!(
            r#"
    SELECT p.identity::text AS id, p.identity AS place_identity,
           COALESCE(s.entity #>> '{{metadata,scene,base}}', '') AS base_position,
           COALESCE(NULLIF(btrim(s.entity #>> '{{metadata,display,title}}'), ''), w.title) AS title,
           COALESCE(s.entity #>> '{{metadata,display,description}}', w.description) AS description,
           w.owner AS creator_address,
           COALESCE(m.content_rating, wm.content_rating, w.content_rating) AS content_rating,
           COALESCE(w.categories, '{{}}') AS categories,
           0::integer AS likes, 0::integer AS dislikes, 0::integer AS favorites,
           s.created_at AS deployed_at, false AS disabled,
           COALESCE(m.highlighted, wm.highlighted, false) AS highlighted,
           true AS world, w.name AS world_name,
           {local} AS local,
           jsonb_build_object(
               'positions', to_jsonb(s.parcels),
               'world_id', w.name, 'deployment_id', s.entity_id,
               'contact_name', s.entity #>> '{{metadata,contact,name}}',
               'contact_email', s.entity #>> '{{metadata,contact,email}}',
               'image', CASE WHEN w.thumbnail_hash IS NOT NULL
                             THEN rtrim({content_parameter}, '/') || '/contents/' || w.thumbnail_hash END,
               'created_at', p.created_at, 'updated_at', GREATEST(p.updated_at, s.updated_at, w.updated_at),
               'is_private', w.access_type <> 'unrestricted', 'show_in_places', w.show_in_places,
               'single_player', w.single_player, 'skybox_time', w.skybox_time,
               'settings_version', w.settings_version
           ) || COALESCE(r.overlay, '{{}}'::jsonb) AS raw
    {binding}
    LEFT JOIN explore.place_moderation m ON m.place_id = p.identity::text
    LEFT JOIN explore.place_moderation wm ON wm.place_id = w.name
    LEFT JOIN explore.place_ranking r ON r.place_id = p.identity::text
    WHERE {valid_parcels}
      AND w.blocked_since IS NULL AND w.show_in_places
      AND NOT COALESCE(m.disabled, false) AND NOT COALESCE(wm.disabled, false)
      AND {parent_visibility}
"#
        );
        if self.worlds_mirror_configured {
            sql.push_str(&format!(" AND ({local} OR {PARENT_PRESENT_OR_INCOMPLETE})"));
        }
        sql
    }

    /// Candidate keys are selected before joining deployments and validating
    /// parcels. UUID probes use the identity primary key; favorites use the
    /// signed viewer's membership keys, never all active scene identities.
    pub(super) fn scene_list_source(
        &self,
        f: &super::PlaceListFilters,
        scenes: &[String],
    ) -> super::ListSource {
        use super::query::Bind;
        if f.only_worlds || (!f.viewer_favorites_only && scenes.is_empty()) {
            return super::ListSource::catalogue();
        }
        let mut binds = vec![Bind::Text(self.content_public_url.clone())];
        let mut predicates = Vec::new();
        if !scenes.is_empty() {
            binds.push(Bind::UuidArray(
                scenes
                    .iter()
                    .filter_map(|id| uuid::Uuid::parse_str(id).ok())
                    .collect(),
            ));
            predicates.push(format!("p.identity = ANY(${}::uuid[])", binds.len()));
        }
        if f.viewer_favorites_only {
            binds.push(Bind::Text(
                f.viewer.clone().unwrap_or_default().to_lowercase(),
            ));
            predicates.push(format!("p.identity = ANY(ARRAY(SELECT CASE WHEN pg_input_is_valid(uf.entity_id, 'uuid') THEN uf.entity_id::uuid END FROM explore.user_favorites uf WHERE uf.\"user\" = ${}))", binds.len()));
        }
        let rows = self.scene_rows_sql(1);
        let predicate = predicates.join(" AND ");
        let columns = "id, base_position, title, description, creator_address, content_rating, categories, likes, dislikes, favorites, deployed_at, disabled, highlighted, raw, world_name, world";
        let local = crate::copies::origin::world_scene_rows_sql();
        super::ListSource {
            prefix: format!(
                "WITH scene_catalogue AS (SELECT {columns}, false AS canonical_scene FROM explore.place_indexed WHERE NOT EXISTS (SELECT 1 FROM explore.world_scene_place p WHERE p.identity = CASE WHEN pg_input_is_valid(regexp_replace(explore.place_indexed.id, '^urn:uuid:', '', 'i'), 'uuid') THEN regexp_replace(explore.place_indexed.id, '^urn:uuid:', '', 'i')::uuid END) UNION ALL SELECT {columns}, true AS canonical_scene FROM ({rows} AND {predicate}) scene_rows WHERE true {local}) "
            ),
            relation: "scene_catalogue AS place_indexed",
            scene_projection: true,
            binds,
        }
    }

    pub(crate) async fn has_bound_scene_ids(&self, ids: &[String]) -> Result<bool, ApiError> {
        let ids: Vec<uuid::Uuid> = ids
            .iter()
            .filter_map(|id| uuid::Uuid::parse_str(id).ok())
            .collect();
        sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
            "SELECT EXISTS (SELECT 1 {} WHERE p.identity = ANY($1))",
            super::scene_identity::BOUND_WORLD_SCENE_FROM
        )))
        .bind(ids)
        .fetch_one(&self.pool)
        .await
        .map_err(Into::into)
    }

    /// Canonicalize all UUID spellings before deciding which authority owns
    /// a requested ID. Upstream IDs which are not ours keep their spelling.
    pub(crate) async fn partition_scene_ids(
        &self,
        ids: &[String],
    ) -> Result<(Vec<String>, Vec<String>), ApiError> {
        if ids.is_empty() {
            return Ok((Vec::new(), Vec::new()));
        }
        let parsed: Vec<uuid::Uuid> = ids
            .iter()
            .filter_map(|id| uuid::Uuid::parse_str(id).ok())
            .collect();
        let known: Vec<uuid::Uuid> = sqlx::query_scalar(
            "SELECT identity FROM explore.world_scene_place WHERE identity = ANY($1)",
        )
        .bind(&parsed)
        .fetch_all(&self.pool)
        .await?;
        let known: std::collections::HashSet<_> = known.into_iter().collect();
        let mut catalogue = Vec::new();
        let mut scenes = std::collections::BTreeSet::new();
        for id in ids {
            match uuid::Uuid::parse_str(id)
                .ok()
                .filter(|id| known.contains(id))
            {
                Some(id) => {
                    scenes.insert(id.to_string());
                }
                None => catalogue.push(id.clone()),
            }
        }
        Ok((catalogue, scenes.into_iter().collect()))
    }

    pub(super) async fn world_scene_places_by_ids(
        &self,
        ids: &[String],
        viewer: Option<&str>,
    ) -> Result<Vec<PlaceRow>, ApiError> {
        self.world_scene_places_by_ids_scoped(
            ids,
            viewer,
            crate::copies::origin::world_scenes_local_only(),
        )
        .await
    }

    pub(super) async fn world_scene_places_by_ids_scoped(
        &self,
        ids: &[String],
        viewer: Option<&str>,
        local_only: bool,
    ) -> Result<Vec<PlaceRow>, ApiError> {
        let identities: Vec<uuid::Uuid> = ids
            .iter()
            .filter_map(|id| uuid::Uuid::parse_str(id).ok())
            .collect();
        let scene_rows = self.scene_rows_sql(3);
        let sql = format!(
            "SELECT {}{} FROM ({scene_rows}) place_indexed WHERE place_identity = ANY($1){}",
            place_columns(),
            viewer_columns(2),
            if local_only { " AND local" } else { "" }
        );
        Ok(sqlx::query(sqlx::AssertSqlSafe(sql))
            .bind(&identities)
            .bind(viewer.map(str::to_lowercase))
            .bind(&self.content_public_url)
            .fetch_all(&self.pool)
            .await?
            .into_iter()
            .filter_map(|row| scene_row(row, self))
            .collect())
    }

    /// Outer `None` means this is not a canonical scene ID. `Some(None)`
    /// means the ID exists but has no currently discoverable deployment.
    pub(super) async fn find_world_scene_place(
        &self,
        id: &str,
        viewer: Option<&str>,
    ) -> Result<Option<Option<PlaceRow>>, ApiError> {
        let Ok(id) = uuid::Uuid::parse_str(id) else {
            return Ok(None);
        };
        // Both identity ownership and visibility are observed in one snapshot,
        // so a concurrent retirement cannot cause a fallback to upstream.
        let scene_rows = self.scene_rows_sql(3);
        let sql = format!(
            "WITH scene AS (SELECT {}{} FROM ({scene_rows}) place_indexed WHERE place_identity = $1) \
             SELECT scene.*, p.identity::text AS canonical_id FROM explore.world_scene_place p \
             LEFT JOIN scene ON scene.id = p.identity::text WHERE p.identity = $1",
            place_columns(),
            viewer_columns(2)
        );
        let row = sqlx::query(sqlx::AssertSqlSafe(sql))
            .bind(id)
            .bind(viewer.map(str::to_lowercase))
            .bind(&self.content_public_url)
            .fetch_optional(&self.pool)
            .await?;
        Ok(row.map(|row| {
            row.get::<Option<String>, _>("id")
                .and_then(|_| scene_row(row, self))
        }))
    }

    pub(crate) async fn require_discoverable_world(&self, world: &str) -> Result<(), ApiError> {
        let world = world.trim().to_lowercase();
        let parent = if self.worlds_mirror_configured {
            format!("AND (w.mirror_source IS NULL OR {PARENT_PRESENT_OR_INCOMPLETE})")
        } else {
            String::new()
        };
        let exists: bool = sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
            "SELECT EXISTS (SELECT 1 FROM worlds.worlds w LEFT JOIN explore.place_moderation m ON m.place_id = w.name WHERE w.name = $1 AND w.blocked_since IS NULL AND w.show_in_places AND NOT COALESCE(m.disabled, false) AND {PARENT_VISIBILITY} {parent})"
        ))).bind(&world).fetch_one(&self.pool).await?;
        if exists {
            Ok(())
        } else {
            Err(ApiError::not_found(format!("Not found world \"{world}\"")))
        }
    }

    /// A bounded per-world scene page; aggregate `/api/worlds` and global
    /// place/map listings do not gain a second row for every world scene.
    pub async fn world_scene_places(
        &self,
        world: &str,
        viewer: Option<&str>,
        limit: i64,
        offset: i64,
    ) -> Result<(Vec<PlaceRow>, i64), ApiError> {
        let world = world.trim().to_lowercase();
        self.require_discoverable_world(&world).await?;
        let scene_rows = self.scene_rows_sql(3);
        let sql = format!(
            "WITH visible AS (SELECT {}{} FROM ({scene_rows}) place_indexed WHERE world_name = $1{}), \
             page AS (SELECT * FROM visible ORDER BY deployed_at DESC NULLS LAST, id LIMIT $4 OFFSET $5) \
             SELECT page.*, totals.total FROM (SELECT count(*)::bigint AS total FROM visible) totals \
             LEFT JOIN page ON true ORDER BY deployed_at DESC NULLS LAST, id",
            place_columns(),
            viewer_columns(2),
            crate::copies::origin::world_scene_rows_sql()
        );
        let rows = sqlx::query(sqlx::AssertSqlSafe(sql))
            .bind(&world)
            .bind(viewer.map(str::to_lowercase))
            .bind(&self.content_public_url)
            .bind(limit.clamp(0, 100))
            .bind(offset.clamp(0, 100_000))
            .fetch_all(&self.pool)
            .await?;
        let total = rows.first().map(|r| r.get("total")).unwrap_or(0);
        let places = rows
            .into_iter()
            .filter_map(|row| {
                row.get::<Option<String>, _>("id")
                    .and_then(|_| scene_row(row, self))
            })
            .collect();
        Ok((places, total))
    }
}

pub async fn get_world_scene_places(
    State(state): State<SharedState>,
    method: Method,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
    Path(world): Path<String>,
    Query(pairs): Query<Vec<(String, String)>>,
) -> Result<Json<ApiDataTotal<PlaceRow>>, ApiError> {
    let get = |key: &str, default: i64| -> Result<i64, ApiError> {
        match pairs.iter().find(|(k, _)| k == key) {
            Some((_, value)) => value
                .parse::<i64>()
                .map_err(|_| ApiError::bad_request(format!("Invalid {key}: expected an integer"))),
            None => Ok(default),
        }
    };
    let viewer = state.viewer(&headers, method.as_str(), uri.path()).await;
    let (places, total) = state
        .places
        .world_scene_places(
            &world,
            viewer.as_deref(),
            get("limit", 100)?,
            get("offset", 0)?,
        )
        .await?;
    Ok(Json(ApiDataTotal::ok(places, total)))
}

#[cfg(test)]
mod tests {
    use axum::body::{Body, to_bytes};
    use axum::http::{Request, StatusCode};
    use serde_json::{Value, json};
    use sqlx::PgPool;
    use std::sync::Arc;
    use tower::ServiceExt;

    async fn deploy(pool: &PgPool, world: &str, entity: &str, parcels: &[&str]) -> String {
        let mut tx = pool.begin().await.unwrap();
        sqlx::query("SELECT name FROM worlds.worlds WHERE name = $1 FOR UPDATE")
            .bind(world)
            .fetch_one(&mut *tx)
            .await
            .unwrap();
        let parcels: Vec<String> = parcels.iter().map(|p| p.to_string()).collect();
        sqlx::query("DELETE FROM worlds.world_scenes WHERE world_name = $1 AND parcels && $2")
            .bind(world)
            .bind(&parcels)
            .execute(&mut *tx)
            .await
            .unwrap();
        sqlx::query("INSERT INTO worlds.world_scenes (world_name, entity_id, deployment_auth_chain, entity, deployer, parcels, size) VALUES ($1, $2, '[]', $3, '0xaa', $4, 1)")
            .bind(world).bind(entity).bind(json!({"metadata": {"scene": {"base": parcels[0]}, "display": {"title": entity, "description": "<script>bad()</script>Scene"}}}))
            .bind(&parcels).execute(&mut *tx).await.unwrap();
        let id = super::super::scene_identity::assign(&mut tx, world, entity, &parcels)
            .await
            .unwrap()
            .unwrap();
        super::super::scene_identity::retire_orphans(&mut tx, &[world.to_string()])
            .await
            .unwrap();
        tx.commit().await.unwrap();
        id.to_string()
    }

    async fn get(app: &axum::Router, path: &str) -> (StatusCode, Value) {
        let response = app
            .clone()
            .oneshot(Request::builder().uri(path).body(Body::empty()).unwrap())
            .await
            .unwrap();
        let status = response.status();
        let bytes = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
        (status, serde_json::from_slice(&bytes).unwrap())
    }

    async fn signed(
        app: &axum::Router,
        method: &str,
        path: &str,
        body: Option<Value>,
    ) -> (StatusCode, Value) {
        let mut request = Request::builder().method(method).uri(path);
        let signed_path = path.split('?').next().unwrap();
        for (key, value) in crate::shared::signed_fetch::test_support::signed_headers(
            &method.to_lowercase(),
            signed_path,
            "{}",
            true,
        )
        .await
        .iter()
        {
            request = request.header(key, value);
        }
        let request = request
            .header("content-type", "application/json")
            .body(
                body.map(|b| Body::from(b.to_string()))
                    .unwrap_or_else(Body::empty),
            )
            .unwrap();
        let response = app.clone().oneshot(request).await.unwrap();
        let status = response.status();
        let value =
            serde_json::from_slice(&to_bytes(response.into_body(), 1024 * 1024).await.unwrap())
                .unwrap();
        (status, value)
    }

    async fn fixture(url: &str) -> (PgPool, PgPool, String) {
        let options: sqlx::postgres::PgConnectOptions = url.parse().unwrap();
        let admin = sqlx::postgres::PgPoolOptions::new()
            .max_connections(1)
            .connect_with(options.clone().database("postgres"))
            .await
            .unwrap();
        let database = format!("dcl_scene_discovery_{}", uuid::Uuid::new_v4().simple());
        sqlx::query(sqlx::AssertSqlSafe(format!(
            "CREATE DATABASE \"{database}\""
        )))
        .execute(&admin)
        .await
        .unwrap();
        let pool = sqlx::postgres::PgPoolOptions::new()
            .max_connections(4)
            .connect_with(options.database(&database))
            .await
            .unwrap();
        crate::db::apply_schema(&pool).await.unwrap();
        (admin, pool, database)
    }

    #[tokio::test]
    async fn scene_discovery_uses_the_deployment_place_and_preserves_world_aggregates() {
        let Ok(url) = std::env::var("CATALYST_TEST_DATABASE_URL") else {
            return;
        };
        let (admin, pool, database) = fixture(&url).await;
        let empty_app = crate::router(Arc::new(crate::ExploreState::new(
            pool.clone(),
            crate::Settings {
                places_upstream: Some(crate::MirrorSource::unchecked("https://places.example")),
                worlds_upstream: Some(crate::MirrorSource::unchecked("https://places.example")),
                ..crate::Settings::for_tests()
            },
            None,
        )));
        for body in [
            None,
            Some(Value::Null),
            Some(json!(123)),
            Some(json!("bad")),
            Some(json!({"positions": ["0,0"]})),
            Some(json!({"world_names": ["unknown.dcl.eth"]})),
            Some(json!(["catalogue-only"])),
        ] {
            assert_eq!(
                signed(&empty_app, "POST", "/api/destinations", body.clone())
                    .await
                    .0,
                StatusCode::SERVICE_UNAVAILABLE,
                "body {body:?}"
            );
        }
        let world = format!("places-{}.dcl.eth", uuid::Uuid::new_v4().simple());
        sqlx::query("INSERT INTO worlds.worlds (name, title, thumbnail_hash) VALUES ($1, 'World aggregate', 'thumb')")
            .bind(&world).execute(&pool).await.unwrap();
        let first = deploy(&pool, &world, "first", &["0,0", "1,0"]).await;
        let second = deploy(&pool, &world, "second", &["5,0"]).await;
        let malformed = deploy(
            &pool,
            &world,
            "malformed",
            &["bad", "999999999999999999999999999999999,0"],
        )
        .await;
        // Existing aggregate records stay world-shaped: no scene enters that view.
        sqlx::query("INSERT INTO explore.place_world_local (id, world_name, world, base_position, title, raw) VALUES ($1, $1, true, '0,0', 'World aggregate', '{}')")
            .bind(&world).execute(&pool).await.unwrap();
        let settings = crate::Settings {
            user_writes_enabled: true,
            places_upstream: Some(crate::MirrorSource::unchecked("https://places.example")),
            worlds_upstream: Some(crate::MirrorSource::unchecked("https://places.example")),
            content_public_url: "https://content.example".into(),
            ..crate::Settings::for_tests()
        };
        let state = Arc::new(crate::ExploreState::new(pool.clone(), settings, None));
        let app = crate::router(state.clone());
        let (status, page) = get(&app, &format!("/api/worlds/{world}/scenes?limit=1")).await;
        assert_eq!(status, StatusCode::OK, "{page}");
        assert_eq!(page["total"], 2);
        assert_eq!(page["data"].as_array().unwrap().len(), 1);
        for spelling in [world.to_uppercase(), world.replacen('p', "%70", 1)] {
            assert_eq!(
                get(&app, &format!("/api/worlds/{spelling}/scenes")).await.0,
                StatusCode::OK
            );
        }
        let (_, empty) = get(
            &app,
            &format!("/api/worlds/{world}/scenes?offset=100&limit=1"),
        )
        .await;
        assert_eq!(empty["total"], 2);
        assert_eq!(empty["data"], json!([]));
        let (_, second_page) = get(
            &app,
            &format!("/api/worlds/{world}/scenes?offset=1&limit=1"),
        )
        .await;
        assert_eq!(second_page["total"], 2);
        assert_eq!(second_page["data"].as_array().unwrap().len(), 1);
        assert_eq!(
            get(&app, &format!("/api/worlds/{world}/scenes?limit=wat"))
                .await
                .0,
            StatusCode::BAD_REQUEST
        );
        assert_eq!(
            get(&app, "/api/worlds/no-such-world.dcl.eth/scenes")
                .await
                .0,
            StatusCode::SERVICE_UNAVAILABLE
        );
        assert_eq!(
            get(&app, &format!("/api/places/{malformed}")).await.0,
            StatusCode::NOT_FOUND
        );
        let valid_batch = state
            .places
            .find_by_ids_for(&[malformed.clone(), first.clone()], None)
            .await
            .unwrap();
        assert_eq!(valid_batch.len(), 1);
        assert_eq!(valid_batch[0].id, first);
        let empty_world = format!("empty-{}.dcl.eth", uuid::Uuid::new_v4().simple());
        sqlx::query("INSERT INTO worlds.worlds (name) VALUES ($1)")
            .bind(&empty_world)
            .execute(&pool)
            .await
            .unwrap();
        let (status, empty_world_page) =
            get(&app, &format!("/api/worlds/{empty_world}/scenes")).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(empty_world_page["total"], 0);
        let (status, detail) = get(&app, &format!("/api/places/{first}")).await;
        assert_eq!(status, StatusCode::OK, "{detail}");
        assert_eq!(detail["data"]["id"], first);
        assert_eq!(detail["data"]["deployment_id"], "first");
        assert_eq!(detail["data"]["world_id"], world);
        let (status, liked) = signed(
            &app,
            "PATCH",
            &format!("/api/places/{first}/likes"),
            Some(json!({"like": true})),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{liked}");
        assert_eq!(liked["data"]["likes"], 1);
        let (status, favorite) = signed(
            &app,
            "PATCH",
            &format!("/api/places/{first}/favorites"),
            Some(json!({"favorites": true})),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{favorite}");
        assert_eq!(favorite["data"]["favorites"], 1);
        let (status, favorites) =
            signed(&app, "GET", "/api/places?only_favorites=true&limit=1", None).await;
        assert_eq!(status, StatusCode::OK, "{favorites}");
        assert_eq!(favorites["total"], 1);
        assert_eq!(favorites["data"][0]["id"], first);
        for path in [
            "/api/places?only_favorites=true&only_places=true".to_string(),
            format!("/api/destinations?ids={first}&only_places=true"),
        ] {
            let (status, body) = signed(&app, "GET", &path, None).await;
            assert_eq!(status, StatusCode::OK, "{body}");
            assert_eq!(body["data"][0]["id"], first, "{body}");
        }

        let (_, favorites_offpage) = signed(
            &app,
            "GET",
            "/api/places?only_favorites=true&offset=10",
            None,
        )
        .await;
        assert_eq!(favorites_offpage["total"], 1);
        assert_eq!(favorites_offpage["data"], json!([]));
        let (status, destinations) =
            signed(&app, "GET", "/v1/destinations?only_favorites=true", None).await;
        assert_eq!(status, StatusCode::OK, "{destinations}");
        assert_eq!(destinations["data"][0]["id"], first);
        assert_eq!(destinations["data"][0]["kind"], "place");
        assert_eq!(
            get(&app, &format!("/v1/destinations/{first}")).await.0,
            StatusCode::OK
        );
        let (status, batch_destinations) = signed(
            &app,
            "POST",
            "/api/destinations",
            Some(json!({"ids": [first]})),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{batch_destinations}");
        assert_eq!(batch_destinations["data"][0]["id"], first);
        let (status, categories) = get(&app, &format!("/api/places/{first}/categories")).await;
        assert_eq!(status, StatusCode::OK, "{categories}");
        assert_eq!(categories["data"]["categories"], json!([]));

        assert!(
            !detail["data"]["description"]
                .as_str()
                .unwrap()
                .contains("<script>")
        );
        assert_eq!(
            detail["data"]["image"],
            "https://content.example/contents/thumb"
        );
        assert_eq!(
            deploy(&pool, &world, "replacement", &["1,0", "2,0"]).await,
            first
        );
        let (_, changed) = get(&app, &format!("/api/places/{first}")).await;
        assert_eq!(changed["data"]["deployment_id"], "replacement");
        assert_eq!(changed["data"]["base_position"], "1,0");
        sqlx::query("UPDATE worlds.world_scenes SET entity = jsonb_set(entity, '{metadata,scene,base}', '\"99,99\"') WHERE world_name = $1 AND entity_id = 'replacement'")
            .bind(&world).execute(&pool).await.unwrap();
        assert_eq!(
            state
                .places
                .find_by_id_for(&first, None)
                .await
                .unwrap()
                .unwrap()
                .base_position,
            "1,0"
        );
        let aggregate_count: i64 =
            sqlx::query_scalar("SELECT count(*) FROM explore.place_indexed WHERE world_name = $1")
                .bind(&world)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(aggregate_count, 1);
        let (_, aggregate) = get(&app, &format!("/api/worlds/{world}")).await;
        assert_eq!(aggregate["data"]["id"], world);
        // Mirrored pointers need not arrive sorted, and a declared base can
        // have a noncanonical spelling. Discovery matches room resolution.
        let mirrored = format!("mirror-{}.dcl.eth", uuid::Uuid::new_v4().simple());
        sqlx::query(
            "INSERT INTO worlds.worlds (name, mirror_source) VALUES ($1, 'https://worlds.example')",
        )
        .bind(&mirrored)
        .execute(&pool)
        .await
        .unwrap();
        let mirrored_id = deploy(
            &pool,
            &mirrored,
            "mirrored",
            &["5,0", "bad", "2147483648,0", "0,1"],
        )
        .await;
        sqlx::query("UPDATE worlds.world_scenes SET mirror_source = 'https://worlds.example', entity = jsonb_set(entity, '{metadata,scene,base}', '\"99,99\"') WHERE world_name = $1")
            .bind(&mirrored).execute(&pool).await.unwrap();
        assert_eq!(
            state
                .places
                .find_by_id_for(&mirrored_id, None)
                .await
                .unwrap()
                .unwrap()
                .base_position,
            "0,1"
        );
        sqlx::query("UPDATE worlds.world_scenes SET entity = jsonb_set(entity, '{metadata,scene,base}', '\" 00 , 01 \"') WHERE world_name = $1")
            .bind(&mirrored).execute(&pool).await.unwrap();
        assert_eq!(
            state
                .places
                .find_by_id_for(&mirrored_id, None)
                .await
                .unwrap()
                .unwrap()
                .base_position,
            "0,1"
        );
        sqlx::query("INSERT INTO explore.place (id,base_position,raw) VALUES ('upstream-only','0,0','{\"positions\":[\"0,0\"]}')").execute(&pool).await.unwrap();
        for (ids, expected) in [
            (json!(["upstream-only"]), 0),
            (json!([mirrored_id]), 0),
            (json!([first]), 1),
            (json!([first, mirrored_id, "upstream-only"]), 1),
        ] {
            for route in ["/api/places", "/api/places/status", "/api/destinations"] {
                let (status, body) = signed(&app, "POST", route, Some(ids.clone())).await;
                assert_eq!(status, StatusCode::OK, "{route}: {body}");
                assert_eq!(
                    body["data"].as_array().unwrap().len(),
                    expected,
                    "{route}: {body}"
                );
            }
        }
        for selectors in [
            json!({"positions": ["0,0"]}),
            json!({"world_names": [mirrored]}),
            json!({"ids": [mirrored_id], "positions": ["0,0"]}),
        ] {
            let (status, body) = signed(&app, "POST", "/api/destinations", Some(selectors)).await;
            assert_eq!(status, StatusCode::OK);
            assert!(
                body["data"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .all(|row| row["id"] != "upstream-only" && row["id"] != mirrored_id)
            );
        }
        sqlx::query("UPDATE worlds.worlds SET show_in_places = false WHERE name = $1")
            .bind(&mirrored)
            .execute(&pool)
            .await
            .unwrap();
        let (_, hidden_status) = signed(
            &app,
            "POST",
            "/api/places/status",
            Some(json!([mirrored_id])),
        )
        .await;
        assert_eq!(hidden_status["data"][0]["disabled"], true);
        assert_eq!(hidden_status["data"][0]["world_name"], Value::Null);
        sqlx::query("UPDATE worlds.worlds SET show_in_places = true WHERE name = $1")
            .bind(&mirrored)
            .execute(&pool)
            .await
            .unwrap();
        // Places counts string IDs, ignoring other array entries. Padding
        // cannot change the selected source authority in the middleware.
        let mut padded_ids = vec![Value::Null; super::super::handlers::MAX_BATCH_ITEMS];
        padded_ids.push(json!(mirrored_id));
        let (_, padded) = signed(&app, "POST", "/api/places", Some(json!(padded_ids))).await;
        assert_eq!(padded["data"], json!([]));
        // Before the configured mirror's first complete pass, a mixed batch
        // exposes local deployment places only; direct local detail worked
        // above without a catalogue refresh.
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/places")
                    .header("content-type", "application/json")
                    .body(Body::from(json!([first, mirrored_id]).to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let batch: Value =
            serde_json::from_slice(&to_bytes(response.into_body(), 1024 * 1024).await.unwrap())
                .unwrap();
        assert_eq!(batch["data"].as_array().unwrap().len(), 1, "{batch}");
        assert_eq!(batch["data"][0]["id"], first);
        sqlx::query("UPDATE worlds.world_scenes SET mirror_source = 'https://worlds.example' WHERE world_name = $1 AND entity_id = 'second'")
            .bind(&world).execute(&pool).await.unwrap();
        let (status, local_only) = get(&app, &format!("/api/worlds/{world}/scenes")).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(local_only["total"], 1);
        assert_eq!(local_only["data"][0]["id"], first);
        // The Worlds source controls canonical UUIDs even when URI encoded
        // and the unrelated Places catalogue has completed its first pass.
        sqlx::query("INSERT INTO explore.mirror_state (mirror, source_url, last_complete_pass_at) VALUES ('places', 'https://places.example', now())")
            .execute(&pool).await.unwrap();
        let fresh_app = crate::router(Arc::new(crate::ExploreState::new(
            pool.clone(),
            state.settings.clone(),
            None,
        )));
        let encoded_id = format!("%{:02x}{}", mirrored_id.as_bytes()[0], &mirrored_id[1..]);
        assert_eq!(
            get(&fresh_app, &format!("/api/places/{encoded_id}"))
                .await
                .0,
            StatusCode::SERVICE_UNAVAILABLE
        );
        // Parent absence only becomes authoritative after a configured Worlds
        // catalogue completes. Standalone discovery and local scenes remain.
        sqlx::query("INSERT INTO explore.mirror_state (mirror, source_url, last_complete_pass_at) VALUES ('worlds', 'https://places.example', now())")
            .execute(&pool).await.unwrap();
        let complete_app = crate::router(Arc::new(crate::ExploreState::new(
            pool.clone(),
            state.settings.clone(),
            None,
        )));
        let (status, complete_page) =
            get(&complete_app, &format!("/api/worlds/{world}/scenes")).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(complete_page["total"], 2);
        assert_eq!(
            get(&complete_app, "/api/worlds/no-such-world.dcl.eth/scenes")
                .await
                .0,
            StatusCode::NOT_FOUND
        );
        // Worlds copy is ready while Places has no copy: mirrored child rows
        // are eligible despite the Places-only fallback on the mixed batch.
        sqlx::query("DELETE FROM explore.mirror_state WHERE mirror = 'places'")
            .execute(&pool)
            .await
            .unwrap();
        let worlds_ready_app = crate::router(Arc::new(crate::ExploreState::new(
            pool.clone(),
            state.settings.clone(),
            None,
        )));
        let (_, worlds_ready_batch) = signed(
            &worlds_ready_app,
            "POST",
            "/api/places",
            Some(json!([second, "upstream-only"])),
        )
        .await;
        assert_eq!(
            worlds_ready_batch["data"].as_array().unwrap().len(),
            1,
            "{worlds_ready_batch}"
        );
        assert_eq!(worlds_ready_batch["data"][0]["id"], second);

        assert!(
            state
                .places
                .find_by_id_for(&mirrored_id, None)
                .await
                .unwrap()
                .is_none()
        );
        let standalone =
            super::super::PlacesComponent::new(pool.clone(), "https://content.example".into());
        assert!(
            standalone
                .find_by_id_for(&mirrored_id, None)
                .await
                .unwrap()
                .is_some()
        );
        assert!(
            state
                .places
                .find_by_id_for(&first, None)
                .await
                .unwrap()
                .is_some()
        );
        sqlx::query("DELETE FROM explore.mirror_state")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("UPDATE worlds.world_scenes SET mirror_source = NULL WHERE world_name = $1")
            .bind(&world)
            .execute(&pool)
            .await
            .unwrap();
        let upstream_world_id = uuid::Uuid::new_v4().to_string();
        sqlx::query("INSERT INTO explore.place (id, base_position, raw) VALUES ($1, '0,1', $2)")
            .bind(&upstream_world_id)
            .bind(json!({"world": true, "world_name": mirrored}))
            .execute(&pool)
            .await
            .unwrap();
        super::super::moderation::apply(
            &pool,
            super::super::moderation::Target::World(&mirrored),
            &super::super::moderation::Decision::Disabled {
                disabled: true,
                reason: None,
            },
            "test",
        )
        .await
        .unwrap();
        assert!(
            state
                .places
                .find_by_id_for(&mirrored_id, None)
                .await
                .unwrap()
                .is_none()
        );
        super::super::moderation::apply(
            &pool,
            super::super::moderation::Target::World(&mirrored),
            &super::super::moderation::Decision::Disabled {
                disabled: false,
                reason: None,
            },
            "test",
        )
        .await
        .unwrap();
        sqlx::query("UPDATE explore.place SET raw = raw || '{\"show_in_places\": false}'::jsonb WHERE id = $1")
            .bind(&upstream_world_id).execute(&pool).await.unwrap();
        assert!(
            state
                .places
                .find_by_id_for(&mirrored_id, None)
                .await
                .unwrap()
                .is_none()
        );
        // Even an upstream row with the UUID cannot shadow or revive a place.
        sqlx::query("INSERT INTO explore.place (id, base_position, title, raw) VALUES ($1, '99,99', 'Unrelated upstream', '{}')")
            .bind(&first).execute(&pool).await.unwrap();
        let (_, still_local) = get(&app, &format!("/api/places/{first}")).await;
        assert_eq!(still_local["data"]["deployment_id"], "replacement");
        sqlx::query("INSERT INTO explore.place (id, base_position, title, raw) VALUES ($1, '99,99', 'Uppercase upstream', '{}')")
            .bind(first.to_uppercase()).execute(&pool).await.unwrap();
        let ids = vec![
            first.to_uppercase(),
            uuid::Uuid::parse_str(&second).unwrap().simple().to_string(),
        ];
        let batch = state.places.find_by_ids_for(&ids, None).await.unwrap();
        assert_eq!(batch.len(), 2);
        assert!(batch.iter().all(|p| p.deployment_id.is_some()));
        assert_eq!(
            state.places.find_by_ids_status(&ids).await.unwrap().len(),
            2
        );
        let merged = deploy(&pool, &world, "merged", &["1,0", "5,0"]).await;
        let (status, retired) =
            signed(&app, "POST", "/api/places/status", Some(json!([first]))).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(retired["data"][0]["disabled"], true);

        assert_ne!(merged, first);
        assert_ne!(merged, second);
        assert_eq!(
            get(&app, &format!("/api/places/{first}")).await.0,
            StatusCode::NOT_FOUND
        );
        let (rows, total) = state
            .places
            .world_scene_places(&world, None, 100, 0)
            .await
            .unwrap();
        assert_eq!(total, 1);
        assert_eq!(rows[0].id, merged);
        assert!(
            state
                .places
                .find_by_ids_for(&ids, None)
                .await
                .unwrap()
                .is_empty()
        );
        let retired_status = state.places.find_by_ids_status(&ids).await.unwrap();
        assert_eq!(retired_status.len(), 2);
        assert!(
            retired_status.iter().all(|row| row.disabled
                && row.base_position.is_empty()
                && row.world_name.is_none())
        );
        // Parent moderation hides every scene, even if a scene override says
        // enabled. The existing admin routes continue targeting aggregates.
        sqlx::query("INSERT INTO explore.place_moderation (place_id, disabled, set_by) VALUES ($1, true, 'test'), ($2, false, 'test')")
            .bind(&world).bind(&merged).execute(&pool).await.unwrap();
        assert!(
            state
                .places
                .find_by_id_for(&merged, None)
                .await
                .unwrap()
                .is_none()
        );
        sqlx::query("DELETE FROM explore.place_moderation WHERE place_id = $1")
            .bind(&world)
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("UPDATE worlds.worlds SET show_in_places = false WHERE name = $1")
            .bind(&world)
            .execute(&pool)
            .await
            .unwrap();
        assert!(
            state
                .places
                .find_by_id_for(&merged, None)
                .await
                .unwrap()
                .is_none()
        );
        assert!(
            state
                .places
                .world_scene_places(&world, None, 100, 0)
                .await
                .is_err()
        );
        sqlx::query("UPDATE worlds.worlds SET show_in_places = true, blocked_since = timezone('UTC', now()) WHERE name = $1")
            .bind(&world).execute(&pool).await.unwrap();
        assert!(
            state
                .places
                .find_by_id_for(&merged, None)
                .await
                .unwrap()
                .is_none()
        );
        pool.close().await;
        sqlx::query(sqlx::AssertSqlSafe(format!("DROP DATABASE \"{database}\"")))
            .execute(&admin)
            .await
            .unwrap();
        admin.close().await;
    }
    #[tokio::test]
    #[ignore = "disposable 10,000-scene EXPLAIN measurement; run explicitly with --ignored --nocapture"]
    async fn scene_candidate_query_profile() {
        let url = std::env::var("CATALYST_TEST_DATABASE_URL").expect("disposable PostgreSQL URL");
        let (admin, pool, database) = fixture(&url).await;
        sqlx::query("INSERT INTO worlds.worlds (name) SELECT 'profile-' || i || '.dcl.eth' FROM generate_series(0, 99) i").execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO worlds.world_scenes (world_name, entity_id, deployment_auth_chain, entity, deployer, parcels, size) SELECT 'profile-' || (i / 100) || '.dcl.eth', 'scene-' || i, '[]', '{}', '0xaa', ARRAY[CASE WHEN i % 100 = 99 THEN 'invalid' ELSE (i % 100)::text || ',0' END], 1 FROM generate_series(0,9999) i").execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO explore.world_scene_place (world_name, identity, entity_id, parcels, active) SELECT world_name, md5(entity_id)::uuid, entity_id, parcels, true FROM worlds.world_scenes").execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO explore.place_world_local (id,world_name,world,base_position,raw) SELECT name,name,true,'0,0','{}' FROM worlds.worlds").execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO explore.user_favorites (\"user\",entity_id) SELECT '0xprofile',md5('scene-' || i)::uuid::text FROM generate_series(0,9) i").execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO explore.user_favorites (\"user\",entity_id) VALUES ('0xprofile','not-a-uuid')").execute(&pool).await.unwrap();
        sqlx::query("ANALYZE").execute(&pool).await.unwrap();
        let places = super::super::PlacesComponent::new(pool.clone(), "https://content.example");
        let selected: String = sqlx::query_scalar("SELECT md5('scene-1')::uuid::text")
            .fetch_one(&pool)
            .await
            .unwrap();
        for (label, filters) in [
            (
                "one-scene",
                super::super::PlaceListFilters {
                    ids: vec![selected],
                    limit: 100,
                    only_places: true,
                    ..Default::default()
                },
            ),
            (
                "ten-favorites",
                super::super::PlaceListFilters {
                    viewer: Some("0xprofile".into()),
                    viewer_favorites_only: true,
                    limit: 100,
                    only_places: true,
                    ..Default::default()
                },
            ),
        ] {
            let (filters, scenes) = places
                .normalized_scene_filters(&filters)
                .await
                .unwrap()
                .unwrap();
            let source = places.scene_list_source(&filters, &scenes);
            let (sql, binds, live) = super::super::list_sql_from(&filters, true, source);
            assert!(live.is_empty());
            for sample in 0..25 {
                let explain =
                    format!("EXPLAIN (ANALYZE, BUFFERS, WAL, SETTINGS, FORMAT JSON) {sql}");
                let mut query = sqlx::query(sqlx::AssertSqlSafe(explain));
                for bind in &binds {
                    query = super::super::query::bind_param(query, bind);
                }
                if let Some(viewer) = &filters.viewer {
                    query = query.bind(viewer);
                }
                let row = query.fetch_one(&pool).await.unwrap();
                let plan: Value = sqlx::Row::get(&row, 0);
                fn deployment_work(node: &Value) -> f64 {
                    let own = if node["Relation Name"] == "world_scenes" {
                        let rows = node["Actual Rows"].as_f64().unwrap_or(0.0)
                            + node["Rows Removed by Filter"].as_f64().unwrap_or(0.0);
                        rows.max(1.0) * node["Actual Loops"].as_f64().unwrap_or(0.0)
                    } else {
                        0.0
                    };
                    own + node["Plans"]
                        .as_array()
                        .map(|children| children.iter().map(deployment_work).sum::<f64>())
                        .unwrap_or(0.0)
                }
                let bound = if label == "one-scene" { 1.0 } else { 10.0 };
                assert!(
                    deployment_work(&plan[0]["Plan"]) <= bound,
                    "{label} must only inspect candidate deployments: {plan}"
                );

                if sample >= 5 {
                    eprintln!("SCENE_PROFILE {label} {}", plan[0]);
                }
            }
        }
        pool.close().await;
        sqlx::query(sqlx::AssertSqlSafe(format!("DROP DATABASE \"{database}\"")))
            .execute(&admin)
            .await
            .unwrap();
        admin.close().await;
    }
}
