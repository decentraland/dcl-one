//! Operator moderation of a catalogue row: disable a place, highlight a place
//! or a world, set a place's or a world's content rating (the console's
//! `places/{place-disable,place-highlight,place-rating,world-highlight,
//! world-rating}`, `ops::admin::explore`).
//!
//! The old service wrote the three columns of its `place` row, and the next
//! mirror pass wrote upstream's values back over them. Here the decision is
//! a fact of its own, `explore.place_moderation` (keyed by the catalogue id,
//! one nullable field per decision: NULL is no override), and the columns of
//! `explore.place` / `explore.place_world_local` are its projection: the
//! write updates both in one transaction, and the places mirror's upsert
//! re-applies the overlay to every row it writes (`mirror::UPSERT`), so a
//! pass never undoes an operator. The read path is unchanged (it reads the
//! projected columns through `place_indexed`, the partial like-score
//! indexes' `disabled IS FALSE` included). `disabled_at` / `disabled_reason`
//! in the answers stay upstream's (`raw`); the overlay keeps the operator's
//! reason and the audit row keeps the rest.

use serde_json::Value;
use sqlx::PgPool;

use super::rows::PlaceRow;
use crate::shared::errors::ApiError;

/// The old route's accepted ratings (`ALLOWED_RATINGS`).
pub const RATINGS: &[&str] = &["RP", "E", "T", "A", "R"];

/// What the operator decided about one row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Decision {
    Disabled {
        disabled: bool,
        reason: Option<String>,
    },
    Highlighted(bool),
    Rating(String),
}

impl Decision {
    /// The rating body rule of the old route: one of [`RATINGS`].
    pub fn rating(raw: &str) -> Result<Self, ApiError> {
        let r = raw.trim();
        if RATINGS.contains(&r) {
            Ok(Self::Rating(r.to_string()))
        } else {
            Err(ApiError::bad_request("content rating body needed"))
        }
    }

    fn label(&self) -> &'static str {
        match self {
            Self::Disabled { .. } => "disabled",
            Self::Highlighted(_) => "highlighted",
            Self::Rating(_) => "content_rating",
        }
    }
}

/// The row a decision is about: a place by its id, a world by its id or name.
#[derive(Debug, Clone)]
pub enum Target<'a> {
    Place(&'a str),
    World(&'a str),
}

/// The catalogue id of `target`, from either leg; `None` when no row matches.
async fn resolve(pool: &PgPool, target: &Target<'_>) -> Result<Option<String>, ApiError> {
    let row: Option<(String,)> = match target {
        Target::Place(id) => {
            sqlx::query_as("SELECT id FROM explore.place_indexed WHERE world IS FALSE AND id = $1 LIMIT 1").bind(id).fetch_optional(pool).await?
        }
        Target::World(id) => {
            sqlx::query_as(
                "SELECT id FROM explore.place_indexed WHERE world IS TRUE AND (id = $1 OR lower(world_name) = lower($1)) ORDER BY (id = $1) DESC LIMIT 1",
            )
            .bind(id)
            .fetch_optional(pool)
            .await?
        }
    };
    Ok(row.map(|(id,)| id))
}

/// The overlay upsert: only the decided field moves, the others keep their
/// previous override (or none).
const OVERLAY: &str = "INSERT INTO explore.place_moderation (place_id, disabled, highlighted, content_rating, reason, set_by, updated_at)
     VALUES ($1, $2, $3, $4, $5, $6, timezone('UTC', now()))
     ON CONFLICT (place_id) DO UPDATE SET
         disabled = COALESCE(EXCLUDED.disabled, explore.place_moderation.disabled),
         highlighted = COALESCE(EXCLUDED.highlighted, explore.place_moderation.highlighted),
         content_rating = COALESCE(EXCLUDED.content_rating, explore.place_moderation.content_rating),
         reason = CASE WHEN EXCLUDED.disabled IS NOT NULL THEN EXCLUDED.reason ELSE explore.place_moderation.reason END,
         set_by = EXCLUDED.set_by,
         updated_at = EXCLUDED.updated_at";

/// Records `decision` for `target` as `actor` and projects it onto the row.
/// `Ok(None)` when no catalogue row matches (the old routes' 404).
pub async fn apply(
    pool: &PgPool,
    target: Target<'_>,
    decision: &Decision,
    actor: &str,
) -> Result<Option<String>, ApiError> {
    let Some(id) = resolve(pool, &target).await? else {
        return Ok(None);
    };
    let (disabled, highlighted, rating, reason) = match decision {
        Decision::Disabled { disabled, reason } => (Some(*disabled), None, None, reason.clone()),
        Decision::Highlighted(h) => (None, Some(*h), None, None),
        Decision::Rating(r) => (None, None, Some(r.clone()), None),
    };
    let mut tx = pool.begin().await?;
    sqlx::query(OVERLAY)
        .bind(&id)
        .bind(disabled)
        .bind(highlighted)
        .bind(rating.as_deref())
        .bind(reason.as_deref())
        .bind(actor)
        .execute(&mut *tx)
        .await?;
    for table in ["explore.place", "explore.place_world_local"] {
        let sql = format!(
            "UPDATE {table} SET disabled = COALESCE($2, disabled), highlighted = COALESCE($3, highlighted), \
             content_rating = COALESCE($4, content_rating) WHERE id = $1"
        );
        sqlx::query(sqlx::AssertSqlSafe(sql))
            .bind(&id)
            .bind(disabled)
            .bind(highlighted)
            .bind(rating.as_deref())
            .execute(&mut *tx)
            .await?;
    }
    tx.commit().await?;
    tracing::info!(place = %id, field = decision.label(), actor, "place moderation recorded");
    Ok(Some(id))
}

/// The row as the read routes serve it after the decision.
pub async fn answer(
    places: &super::PlacesComponent,
    target: &Target<'_>,
    id: &str,
) -> Result<Option<PlaceRow>, ApiError> {
    match target {
        Target::Place(_) => places.find_by_id_for(id, None).await,
        Target::World(_) => places.find_world_by_id_for(id, None).await,
    }
}

/// The override in force for `id`, as the mirror re-applies it (for the
/// console's detail and the tests).
pub async fn overlay(pool: &PgPool, id: &str) -> Result<Option<Value>, ApiError> {
    let row: Option<(Value,)> = sqlx::query_as(
        "SELECT jsonb_build_object('disabled', disabled, 'highlighted', highlighted, 'content_rating', content_rating, \
                'reason', reason, 'set_by', set_by, 'updated_at', updated_at) \
         FROM explore.place_moderation WHERE place_id = $1",
    )
    .bind(id)
    .fetch_optional(pool)
    .await?;
    Ok(row.map(|(v,)| v))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_rating_rule_is_the_old_routes() {
        assert_eq!(
            Decision::rating(" T ").unwrap(),
            Decision::Rating("T".into())
        );
        for bad in ["", "X", "teen", "t"] {
            assert!(Decision::rating(bad).is_err(), "{bad:?}");
        }
    }

    /// The mirror's upsert keeps an operator's decision over upstream's value
    /// and leaves every undecided field to upstream.
    #[tokio::test]
    async fn a_mirror_pass_reapplies_the_overlay_and_the_projection_serves_it() {
        let Some(pool) = crate::db::scratch::pool(2).await else {
            return;
        };
        let id = format!("console-moderation-{}", uuid::Uuid::new_v4());
        let row = |highlighted: bool, rating: &str| super::super::mirror::MirrorRow {
            id: id.clone(),
            base_position: "-7,7".into(),
            title: "moderated".into(),
            description: String::new(),
            creator_address: None,
            content_rating: Some(rating.into()),
            categories: Vec::new(),
            likes: 0,
            dislikes: 0,
            favorites: 0,
            deployed_at: None,
            disabled: false,
            highlighted,
            raw: serde_json::json!({ "id": id, "base_position": "-7,7" }),
        };
        let upsert = |rows: Vec<super::super::mirror::MirrorRow>| {
            let pool = pool.clone();
            async move {
                super::super::mirror::upsert_rows(&pool, &rows)
                    .await
                    .unwrap()
            }
        };
        upsert(vec![row(false, "E")]).await;

        let actor = "0x00000000000000000000000000000000000000c0";
        assert_eq!(
            apply(
                &pool,
                Target::Place(&id),
                &Decision::Disabled {
                    disabled: true,
                    reason: Some("spam".into())
                },
                actor
            )
            .await
            .unwrap(),
            Some(id.clone())
        );
        assert_eq!(
            apply(
                &pool,
                Target::Place(&id),
                &Decision::rating("A").unwrap(),
                actor
            )
            .await
            .unwrap(),
            Some(id.clone())
        );
        assert_eq!(
            apply(
                &pool,
                Target::World(&id),
                &Decision::Highlighted(true),
                actor
            )
            .await
            .unwrap(),
            None,
            "a place is not a world"
        );
        assert_eq!(
            apply(
                &pool,
                Target::Place("no-such-place"),
                &Decision::Highlighted(true),
                actor
            )
            .await
            .unwrap(),
            None
        );

        let cols = |pool: PgPool, id: String| async move {
            sqlx::query_as::<_, (bool, bool, Option<String>)>(
                "SELECT disabled, highlighted, content_rating FROM explore.place WHERE id = $1",
            )
            .bind(id)
            .fetch_one(&pool)
            .await
            .unwrap()
        };
        assert_eq!(
            cols(pool.clone(), id.clone()).await,
            (true, false, Some("A".into())),
            "projected at once"
        );

        // Upstream says: enabled, highlighted, rated E. The pass keeps the
        // operator's disable and rating and takes upstream's highlight.
        upsert(vec![row(true, "E")]).await;
        assert_eq!(
            cols(pool.clone(), id.clone()).await,
            (true, true, Some("A".into()))
        );

        let overlay = overlay(&pool, &id).await.unwrap().unwrap();
        assert_eq!(
            (
                overlay["disabled"].as_bool(),
                overlay["content_rating"].as_str(),
                overlay["reason"].as_str()
            ),
            (Some(true), Some("A"), Some("spam"))
        );
        assert!(overlay["highlighted"].is_null(), "undecided: no override");

        apply(
            &pool,
            Target::Place(&id),
            &Decision::Disabled {
                disabled: false,
                reason: None,
            },
            actor,
        )
        .await
        .unwrap();
        upsert(vec![row(true, "E")]).await;
        assert_eq!(
            cols(pool.clone(), id.clone()).await,
            (false, true, Some("A".into())),
            "a re-enable is an override too"
        );

        sqlx::query("DELETE FROM explore.place_moderation WHERE place_id = $1")
            .bind(&id)
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("DELETE FROM explore.place WHERE id = $1")
            .bind(&id)
            .execute(&pool)
            .await
            .unwrap();
    }
}
