//! Durable removal feed offered to the gatekeeper. Queue acknowledgements follow
//! external revocation and notification; retry keys never change after a crash.
use chrono::NaiveDateTime;
use sqlx::{PgConnection, PgPool};
use uuid::Uuid;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct RemovedPlace {
    pub event_id: Uuid,
    pub place_id: String,
    pub owner_address: Option<String>,
    pub world: bool,
    pub world_name: Option<String>,
    pub base_position: String,
    pub positions: serde_json::Value,
    pub removed_at: NaiveDateTime,
    pub cleaned_at: Option<NaiveDateTime>,
}
#[derive(Debug, Clone, sqlx::FromRow, serde::Serialize, serde::Deserialize)]
pub struct PlaceContext {
    pub place_id: String,
    pub owner_address: Option<String>,
    pub world: bool,
    pub world_name: Option<String>,
    pub base_position: String,
    pub positions: serde_json::Value,
}
#[derive(Clone)]
pub struct RemovedPlaces {
    pool: PgPool,
}
impl RemovedPlaces {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
    pub async fn place(&self, id: &str) -> Result<Option<PlaceContext>, sqlx::Error> {
        sqlx::query_as("SELECT id AS place_id, CASE WHEN world THEN COALESCE(raw->>'owner', creator_address) ELSE raw->>'owner' END AS owner_address, world, world_name, base_position, COALESCE(raw->'positions', jsonb_build_array(base_position)) AS positions FROM explore.place_indexed WHERE id = $1 ORDER BY (raw->>'local' = 'true') DESC NULLS LAST LIMIT 1").bind(id).fetch_optional(&self.pool).await
    }
    /// Enrich an already established world identity. A Place's base parcel
    /// does not identify a world deployment: sibling scenes may stand elsewhere.
    /// Distinct Place IDs remain ambiguous; local/mirrored copies of one ID
    /// use the same preference as `place`.
    pub async fn world(&self, world: &str) -> Result<Option<PlaceContext>, sqlx::Error> {
        let mut contexts: Vec<PlaceContext> = sqlx::query_as(
            "SELECT DISTINCT ON (id) id AS place_id, COALESCE(raw->>'owner', creator_address) AS owner_address, world, world_name, base_position, COALESCE(raw->'positions', jsonb_build_array(base_position)) AS positions \
             FROM explore.place_indexed WHERE world AND lower(world_name) = $1 \
             ORDER BY id, (raw->>'local' = 'true') DESC NULLS LAST LIMIT 2"
        ).bind(world).fetch_all(&self.pool).await?;
        Ok((contexts.len() == 1).then(|| contexts.remove(0)))
    }
    /// Enrich an exact current scene only after its identity has been resolved.
    /// A single world Place serves sibling scenes regardless of its base. If
    /// several Places exist, bind only a unique base inside this footprint.
    /// Place availability selects context, never world/scene identity.
    pub async fn world_scene(
        &self,
        world: &str,
        parcels: &[String],
    ) -> Result<Option<PlaceContext>, sqlx::Error> {
        let mut contexts: Vec<PlaceContext> = sqlx::query_as(
            "WITH places AS ( \
                SELECT DISTINCT ON (id) id AS place_id, COALESCE(raw->>'owner', creator_address) AS owner_address, world, world_name, base_position, COALESCE(raw->'positions', jsonb_build_array(base_position)) AS positions \
                FROM explore.place_indexed WHERE world AND lower(world_name) = $1 \
                ORDER BY id, (raw->>'local' = 'true') DESC NULLS LAST \
            ), counted AS (SELECT *, count(*) OVER () AS total FROM places) \
            SELECT place_id, owner_address, world, world_name, base_position, positions FROM counted \
            WHERE total = 1 OR base_position = ANY($2::text[]) ORDER BY place_id LIMIT 2"
        ).bind(world).bind(parcels).fetch_all(&self.pool).await?;
        Ok((contexts.len() == 1).then(|| contexts.remove(0)))
    }
    /// Shared/Genesis lookup only. World scene identity is decoded by
    /// comms and its exact active tuple is validated before Places enrichment.
    /// Retired ambiguous world scene spellings never supply metadata.
    pub async fn room(&self, room: &str) -> Result<Option<PlaceContext>, sqlx::Error> {
        let candidates: Vec<(String, Option<String>, String)> = sqlx::query_as(
            "WITH candidates AS ( \
                SELECT DISTINCT p.id AS place_id, NULL::text AS world_name, a.entity_id AS scene_id \
                FROM explore.place_indexed p \
                INNER JOIN content.active_pointers a ON p.base_position = a.pointer OR p.raw->'positions' @> jsonb_build_array(a.pointer) \
                INNER JOIN content.deployments d ON d.entity_id = a.entity_id AND d.deleter_deployment IS NULL AND d.entity_type = 'scene' \
                WHERE NOT p.world AND left($1, 6) = 'scene:' AND length($1) > length(a.entity_id) + 7 \
                  AND right($1, length(a.entity_id) + 1) = ':' || a.entity_id \
                  AND lower(substring($1 FROM 7 FOR length($1) - length(a.entity_id) - 7)) NOT IN ('preview', 'localpreview') \
            ) SELECT place_id, world_name, scene_id FROM candidates ORDER BY place_id, world_name, scene_id LIMIT 2"
        ).bind(room).fetch_all(&self.pool).await?;
        if candidates.len() == 1 {
            self.place(&candidates[0].0).await
        } else {
            Ok(None)
        }
    }
    pub async fn pending(&self) -> Result<Vec<RemovedPlace>, sqlx::Error> {
        sqlx::query_as("SELECT event_id, place_id, owner_address, world, world_name, base_position, positions, removed_at, cleaned_at FROM explore.place_removal_pending WHERE retry_at <= timezone('UTC', now()) ORDER BY retry_at, event_id LIMIT 100").fetch_all(&self.pool).await
    }
    pub async fn save_owner(&self, id: Uuid, owner: &str) -> Result<(), sqlx::Error> {
        sqlx::query("UPDATE explore.place_removal_pending SET owner_address = $2 WHERE event_id = $1 AND owner_address IS NULL").bind(id).bind(owner).execute(&self.pool).await?;
        Ok(())
    }
    /// Same transaction as comms' cleanup; a retry cannot replay destructive work.
    pub async fn mark_cleaned(connection: &mut PgConnection, id: Uuid) -> Result<(), sqlx::Error> {
        sqlx::query("UPDATE explore.place_removal_pending SET cleaned_at = timezone('UTC', now()) WHERE event_id = $1").bind(id).execute(connection).await?;
        Ok(())
    }
    pub async fn retry(&self, id: Uuid) -> Result<(), sqlx::Error> {
        sqlx::query("UPDATE explore.place_removal_pending SET attempts = LEAST(attempts + 1, 20), retry_at = timezone('UTC', now()) + make_interval(secs => LEAST(3600, 60 * power(2, LEAST(attempts, 6)))::int) WHERE event_id = $1").bind(id).execute(&self.pool).await?;
        Ok(())
    }
    pub async fn acknowledge(&self, id: Uuid) -> Result<(), sqlx::Error> {
        let mut tx = self.pool.begin().await?;
        let place: Option<String> = sqlx::query_scalar("DELETE FROM explore.place_removal_pending WHERE event_id = $1 AND cleaned_at IS NOT NULL RETURNING place_id").bind(id).fetch_optional(&mut *tx).await?;
        if let Some(place) = place {
            sqlx::query("DELETE FROM explore.place_removal_state s WHERE place_id = $1 AND NOT EXISTS (SELECT 1 FROM explore.place WHERE id = s.place_id) AND NOT EXISTS (SELECT 1 FROM explore.place_world_local WHERE id = s.place_id) AND NOT EXISTS (SELECT 1 FROM explore.place_removal_pending WHERE place_id = s.place_id)").bind(place).execute(&mut *tx).await?;
        }
        tx.commit().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    async fn insert(pool: &PgPool, id: &str, disabled: bool, reason: Option<&str>) {
        sqlx::query("INSERT INTO explore.place (id, base_position, disabled, raw) VALUES ($1, '0,0', $2, $3)").bind(id).bind(disabled).bind(json!({"positions":["0,0"],"owner":"0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","disabled_reason":reason})).execute(pool).await.unwrap();
    }
    async fn events(pool: &PgPool, id: &str) -> Vec<RemovedPlace> {
        sqlx::query_as("SELECT event_id, place_id, owner_address, world, world_name, base_position, positions, removed_at, cleaned_at FROM explore.place_removal_pending WHERE place_id = $1 ORDER BY removed_at, event_id").bind(id).fetch_all(pool).await.unwrap()
    }
    #[tokio::test]
    async fn opt_out_is_not_removal_and_delete_preserves_owner_context() {
        let Some(pool) = crate::db::scratch::pool(4).await else {
            return;
        };
        let id = format!("removed-{}", Uuid::new_v4());
        insert(&pool, &id, true, Some("opt_out")).await;
        sqlx::query("DELETE FROM explore.place WHERE id = $1")
            .bind(&id)
            .execute(&pool)
            .await
            .unwrap();
        assert!(events(&pool, &id).await.is_empty());
        insert(&pool, &id, false, None).await;
        sqlx::query("DELETE FROM explore.place WHERE id = $1")
            .bind(&id)
            .execute(&pool)
            .await
            .unwrap();
        let rows = events(&pool, &id).await;
        assert_eq!(rows.len(), 1);
        assert_eq!(
            rows[0].owner_address.as_deref(),
            Some("0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")
        );
        assert_eq!(rows[0].positions, json!(["0,0"]));
        assert!(rows[0].cleaned_at.is_none());
    }
    #[tokio::test]
    async fn disable_rewrites_deduplicate_but_readd_starts_a_new_removal() {
        let Some(pool) = crate::db::scratch::pool(4).await else {
            return;
        };
        let id = format!("removed-{}", Uuid::new_v4());
        insert(&pool, &id, false, None).await;
        for _ in 0..2 {
            sqlx::query("UPDATE explore.place SET disabled = true WHERE id = $1")
                .bind(&id)
                .execute(&pool)
                .await
                .unwrap();
        }
        assert_eq!(events(&pool, &id).await.len(), 1);
        sqlx::query("DELETE FROM explore.place WHERE id = $1")
            .bind(&id)
            .execute(&pool)
            .await
            .unwrap();
        assert_eq!(
            events(&pool, &id).await.len(),
            1,
            "delete after disable belongs to the same removal"
        );
        insert(&pool, &id, false, None).await;
        sqlx::query("DELETE FROM explore.place WHERE id = $1")
            .bind(&id)
            .execute(&pool)
            .await
            .unwrap();
        let rows = events(&pool, &id).await;
        assert_eq!(rows.len(), 2);
        assert_ne!(rows[0].event_id, rows[1].event_id);
    }
    #[tokio::test]
    async fn source_handover_keeps_the_place_and_pending_work_requires_clean_checkpoint() {
        let Some(pool) = crate::db::scratch::pool(4).await else {
            return;
        };
        let id = format!("removed-{}", Uuid::new_v4());
        insert(&pool, &id, false, None).await;
        sqlx::query("INSERT INTO explore.place_world_local (id, base_position, raw) VALUES ($1, '0,0', '{}')").bind(&id).execute(&pool).await.unwrap();
        sqlx::query("DELETE FROM explore.place WHERE id = $1")
            .bind(&id)
            .execute(&pool)
            .await
            .unwrap();
        assert!(events(&pool, &id).await.is_empty());
        sqlx::query("DELETE FROM explore.place_world_local WHERE id = $1")
            .bind(&id)
            .execute(&pool)
            .await
            .unwrap();
        let event = events(&pool, &id).await.pop().unwrap();
        let feed = RemovedPlaces::new(pool.clone());
        feed.acknowledge(event.event_id).await.unwrap();
        assert_eq!(
            events(&pool, &id).await.len(),
            1,
            "unprocessed removal cannot be acknowledged"
        );
        feed.retry(event.event_id).await.unwrap();
        let future: bool = sqlx::query_scalar("SELECT retry_at > timezone('UTC', now()) AND attempts = 1 FROM explore.place_removal_pending WHERE event_id = $1").bind(event.event_id).fetch_one(&pool).await.unwrap();
        assert!(future);
        let mut tx = pool.begin().await.unwrap();
        RemovedPlaces::mark_cleaned(&mut tx, event.event_id)
            .await
            .unwrap();
        tx.commit().await.unwrap();
        feed.acknowledge(event.event_id).await.unwrap();
        assert!(events(&pool, &id).await.is_empty());
    }
    #[tokio::test]
    async fn world_scene_lookup_never_returns_sibling_scene_metadata() {
        let Some(pool) = crate::db::scratch::pool(4).await else {
            return;
        };
        let world = format!("removed-{}.dcl.eth", Uuid::new_v4().simple());
        sqlx::query("INSERT INTO worlds.worlds (name) VALUES ($1)")
            .bind(&world)
            .execute(&pool)
            .await
            .unwrap();
        for (scene, parcel) in [("scene-a", "0,0"), ("scene-b", "1,0")] {
            let place_id = format!("{world}-{scene}");
            sqlx::query("INSERT INTO worlds.world_scenes (world_name, entity_id, deployment_auth_chain, entity, deployer, parcels, size, mirror_source) VALUES ($1, $2, '[]'::json, '{}'::jsonb, '0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa', $3, 1, NULL)").bind(&world).bind(scene).bind(vec![parcel]).execute(&pool).await.unwrap();
            sqlx::query("INSERT INTO explore.place (id, base_position, raw) VALUES ($1, $2, $3)")
                .bind(place_id)
                .bind(parcel)
                .bind(serde_json::json!({"world":true,"world_name":world,"positions":[parcel]}))
                .execute(&pool)
                .await
                .unwrap();
        }
        let feed = RemovedPlaces::new(pool);
        assert!(
            feed.room(&format!("world-{world}"))
                .await
                .unwrap()
                .is_none(),
            "ambiguous lobby is not a scene"
        );
        assert!(
            feed.world(&world).await.unwrap().is_none(),
            "a lobby cannot guess a distinct scene Place"
        );
        for (parcel, suffix) in [("0,0", "scene-a"), ("1,0", "scene-b")] {
            assert_eq!(
                feed.world_scene(&world, &[parcel.to_string()])
                    .await
                    .unwrap()
                    .unwrap()
                    .place_id,
                format!("{world}-{suffix}"),
                "exact footprint binds its own scene Place"
            );
        }
        for retired in [
            format!("world-{world}-scene-a"),
            format!("scene-{world}-scene-b"),
        ] {
            assert!(
                feed.room(&retired).await.unwrap().is_none(),
                "retired world-scene spelling is not a metadata authority"
            );
        }
    }
    #[tokio::test]
    async fn room_identity_cannot_be_certified_by_a_competitors_missing_place_or_footprint() {
        let Some(pool) = crate::db::scratch::pool(4).await else {
            return;
        };
        let first = format!("metadata-{}", Uuid::new_v4().simple());
        let second = format!("{first}-cid");
        for (world, scene, parcels) in [
            (&first, "Cid-with-hyphen", vec!["3,4"]),
            (&second, "with-hyphen", vec!["invalid"]),
        ] {
            sqlx::query("INSERT INTO worlds.worlds (name) VALUES ($1)")
                .bind(world)
                .execute(&pool)
                .await
                .unwrap();
            sqlx::query("INSERT INTO worlds.world_scenes (world_name, entity_id, deployment_auth_chain, entity, deployer, parcels, size) VALUES ($1, $2, '[]'::json, '{}'::jsonb, '0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa', $3, 1)")
                .bind(world).bind(scene).bind(parcels).execute(&pool).await.unwrap();
        }
        sqlx::query("INSERT INTO explore.place (id, base_position, raw) VALUES ($1, '3,4', $2)")
            .bind(&first)
            .bind(json!({"world":true,"world_name":first,"positions":["3,4"]}))
            .execute(&pool)
            .await
            .unwrap();
        let room = format!("scene-{first}-cid-with-hyphen");
        let feed = RemovedPlaces::new(pool.clone());
        assert!(
            feed.room(&room).await.unwrap().is_none(),
            "all complete identity candidates count before Places enrichment"
        );
        sqlx::query("INSERT INTO explore.place (id, base_position, raw) VALUES ($1, '5,6', $2)")
            .bind(&second)
            .bind(json!({"world":true,"world_name":second,"positions":["5,6"]}))
            .execute(&pool)
            .await
            .unwrap();
        assert!(
            feed.room(&room).await.unwrap().is_none(),
            "an unusable competing footprint cannot make a room falsely unique"
        );
    }

    #[tokio::test]
    async fn world_place_enrichment_uses_exact_world_and_retired_rooms_supply_no_identity() {
        let Some(pool) = crate::db::scratch::pool(4).await else {
            return;
        };
        let tag = Uuid::new_v4().simple().to_string();
        let world = format!("custom-world-{tag}");
        let other_world = format!("{world}-cid");
        let scene = "Cid-with-hyphen";
        let room = format!("scene-{world}-{scene}");
        for (name, entity, parcel) in [(&world, scene, "3,4"), (&other_world, "with-hyphen", "5,6")]
        {
            sqlx::query("INSERT INTO worlds.worlds (name) VALUES ($1)")
                .bind(name)
                .execute(&pool)
                .await
                .unwrap();
            sqlx::query("INSERT INTO worlds.world_scenes (world_name, entity_id, deployment_auth_chain, entity, deployer, parcels, size) VALUES ($1, $2, '[]'::json, '{}'::jsonb, '0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa', $3, 1)").bind(name).bind(entity).bind(vec![parcel]).execute(&pool).await.unwrap();
            sqlx::query("INSERT INTO explore.place (id, base_position, raw) VALUES ($1, $2, $3)")
                .bind(name)
                .bind(parcel)
                .bind(json!({"world":true,"world_name":name,"positions":[parcel]}))
                .execute(&pool)
                .await
                .unwrap();
            if name == &world {
                let resolved = RemovedPlaces::new(pool.clone())
                    .world(&world)
                    .await
                    .unwrap()
                    .unwrap();
                assert_eq!(
                    resolved.place_id, world,
                    "exact world key selects its Place"
                );
            }
        }
        assert!(
            RemovedPlaces::new(pool.clone())
                .room(&room)
                .await
                .unwrap()
                .is_none(),
            "retired delimiter spelling supplies no metadata identity"
        );
        let world = format!("uppercase-{tag}.dcl.eth");
        sqlx::query("INSERT INTO worlds.worlds (name) VALUES ($1)")
            .bind(&world)
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO worlds.world_scenes (world_name, entity_id, deployment_auth_chain, entity, deployer, parcels, size) VALUES ($1, 'CidMixed', '[]'::json, '{}'::jsonb, '0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa', ARRAY['7,8'], 1)").bind(&world).execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO explore.place (id, base_position, raw) VALUES ($1, '7,8', $2)")
            .bind(&world)
            .bind(json!({"world":true,"world_name":world,"positions":["7,8"]}))
            .execute(&pool)
            .await
            .unwrap();
        let feed = RemovedPlaces::new(pool);
        assert_eq!(feed.world(&world).await.unwrap().unwrap().place_id, world);
        assert!(feed.room("scene:PREVIEW:CidMixed").await.unwrap().is_none());
    }
}
