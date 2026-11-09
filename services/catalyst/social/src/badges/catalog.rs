//! The catalog writes of the bearer-gated admin routes: `PUT /badges/{id}`
//! upserts a definition (and a tiered one's tiers) in one transaction, and
//! `PATCH /badges/{id}` publishes, unpublishes, archives or unarchives it.
//! The body follows upstream's catalog seed (id, name, description,
//! category, and either `criteria.steps` or tiers of `tierId`, `tierName`,
//! `description`, `criteria.steps`), in the shape `GET /badges/{id}` reads,
//! with `assets` as paths relative to `BADGES_PUBLIC_ASSET_BASE_URL`
//! (upstream's CDN layout, the default when omitted). A new badge is
//! unpublished. The definition row is locked `FOR UPDATE`, so a write waits
//! for an in-flight [`super::apply_progress`] (which holds it `FOR SHARE`).
//! Player rows are never written: a held tier cannot be removed, a badge with
//! player rows cannot change kind (single/tiered) nor be unpublished (archive
//! it instead). Each write lands in `social.badge_catalog_audit`.

use std::collections::HashSet;

use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::PgConnection;

use super::assets::sanitize;
use super::store::{BadgesStore, CATEGORIES};
use super::types::{Assets, TierCriteria, TierData};
use crate::notifications::errors::ApiError;

/// The single-tier slot of upstream's layout: the badge directory itself.
const SINGLE_TIER_SLOT: &str = "root";
/// Tiers per badge (upstream's catalog has at most 6); each is one `INSERT`
/// under the definition's `FOR UPDATE`.
pub const MAX_TIERS: usize = 32;
/// Files under `2d` or `3d` of one assets object (upstream's layout has 1 and 3).
pub const MAX_ASSET_FILES: usize = 8;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CriteriaBody {
    pub steps: i64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TierBody {
    #[serde(rename = "tierId")]
    pub tier_id: String,
    #[serde(rename = "tierName")]
    pub tier_name: String,
    pub description: String,
    pub criteria: CriteriaBody,
    #[serde(default)]
    pub assets: Option<Value>,
}

/// `PUT /badges/{id}`: a single badge carries `criteria` (and optionally
/// `assets`), a tiered one `tiers`, in tier order.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DefinitionBody {
    #[serde(default)]
    pub id: Option<String>,
    pub name: String,
    pub description: String,
    pub category: String,
    #[serde(default)]
    pub criteria: Option<CriteriaBody>,
    #[serde(default)]
    pub assets: Option<Value>,
    #[serde(default)]
    pub tiers: Option<Vec<TierBody>>,
}

/// `PATCH /badges/{id}`: the target state; an absent key is left as is.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StateBody {
    #[serde(default)]
    pub published: Option<bool>,
    #[serde(default)]
    pub archived: Option<bool>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Tier {
    pub tier_id: String,
    pub tier_name: String,
    pub description: String,
    pub steps: i32,
    pub assets: Assets,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Single { steps: i32, assets: Assets },
    Tiered(Vec<Tier>),
}

/// A definition that passed [`validate`].
#[derive(Debug, Clone, Serialize)]
pub struct Definition {
    pub id: String,
    pub name: String,
    pub description: String,
    pub category: String,
    pub kind: Kind,
}

impl Definition {
    fn is_tier(&self) -> bool {
        matches!(self.kind, Kind::Tiered(_))
    }
}

/// A definition as stored, published or not, with its assets unresolved
/// (the paths a `PUT` takes back).
#[derive(Debug, Serialize)]
pub struct AdminBadge {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub category: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub criteria: Option<TierCriteria>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub assets: Option<Assets>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tiers: Option<Vec<TierData>>,
    #[serde(rename = "publishedAt")]
    pub published_at: Option<i64>,
    #[serde(rename = "archivedAt")]
    pub archived_at: Option<i64>,
    #[serde(rename = "createdAt")]
    pub created_at: i64,
}

#[derive(Debug, Serialize)]
pub struct AdminBadgeBody {
    pub badge: AdminBadge,
}

/// An id usable as a CDN directory: `[a-z0-9]`, then `[a-z0-9_-]`, 64 at most.
fn valid_id(id: &str) -> bool {
    let b = id.as_bytes();
    !b.is_empty()
        && b.len() <= 64
        && (b[0].is_ascii_lowercase() || b[0].is_ascii_digit())
        && b.iter()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, b'_' | b'-'))
}

/// Upstream's layout under `dir` (`<badge>` or `<badge>/<tier slot>`).
pub fn layout_assets(dir: &str) -> Value {
    json!({
        "2d": { "normal": format!("{dir}/2d/normal.png") },
        "3d": {
            "normal": format!("{dir}/3d/normal.png"),
            "hrm": format!("{dir}/3d/hrm.png"),
            "basecolor": format!("{dir}/3d/basecolor.png")
        }
    })
}

/// `{"2d": {...}, "3d": {...}}` of relative, traversal-free paths, with a
/// `2d.normal` (the image of the lists and the notification).
fn check_assets(at: &str, assets: &Value, errors: &mut Vec<String>) {
    let Some(map) = assets.as_object() else {
        errors.push(format!("{at} must be an object"));
        return;
    };
    for (key, files) in map {
        if key != "2d" && key != "3d" {
            errors.push(format!("{at} takes only \"2d\" and \"3d\", not \"{key}\""));
            continue;
        }
        let Some(files) = files.as_object() else {
            errors.push(format!("{at}.{key} must be an object"));
            continue;
        };
        if files.len() > MAX_ASSET_FILES {
            errors.push(format!(
                "{at}.{key} has {} files, at most {MAX_ASSET_FILES}",
                files.len()
            ));
            continue;
        }
        for (name, path) in files {
            match path.as_str() {
                Some(p) if sanitize(p) == Some(p) => {}
                _ => errors.push(format!(
                    "{at}.{key}.{name} must be a relative path of [A-Za-z0-9_-.] segments \
                     (no URL, no leading '/', no '..')"
                )),
            }
        }
    }
    if assets
        .pointer("/2d/normal")
        .and_then(Value::as_str)
        .is_none()
    {
        errors.push(format!("{at}.2d.normal is required"));
    }
}

fn steps(at: &str, raw: i64, errors: &mut Vec<String>) -> i32 {
    match i32::try_from(raw) {
        Ok(s) if s > 0 => s,
        _ => {
            errors.push(format!("{at} must be a positive integer"));
            1
        }
    }
}

fn text(at: &str, raw: &str, errors: &mut Vec<String>) -> String {
    let t = raw.trim();
    if t.is_empty() {
        errors.push(format!("{at} is required"));
    }
    t.to_string()
}

/// Upstream's schema rules on a `PUT` body for `path_id`; every problem is
/// reported at once, joined by `"; "`.
pub fn validate(path_id: &str, body: DefinitionBody) -> Result<Definition, ApiError> {
    let mut errors = Vec::new();
    let id = path_id.trim().to_string();
    if !valid_id(&id) {
        errors.push(
            "badge id must be 1-64 of [a-z0-9_-], starting with a letter or digit".to_string(),
        );
    }
    if let Some(body_id) = body.id.as_deref()
        && body_id != id
    {
        errors.push(format!("id '{body_id}' does not match the path's '{id}'"));
    }
    let name = text("name", &body.name, &mut errors);
    let description = text("description", &body.description, &mut errors);
    if !CATEGORIES.contains(&body.category.as_str()) {
        errors.push(format!("category must be one of {}", CATEGORIES.join(", ")));
    }
    let kind = match (body.criteria, body.tiers) {
        (Some(_), Some(_)) => {
            errors.push(
                "a badge takes either criteria (single) or tiers (tiered), not both".to_string(),
            );
            None
        }
        (None, None) => {
            errors.push("a badge needs criteria.steps (single) or tiers (tiered)".to_string());
            None
        }
        (Some(criteria), None) => {
            let steps = steps("criteria.steps", criteria.steps, &mut errors);
            let assets = body.assets.unwrap_or_else(|| layout_assets(&id));
            check_assets("assets", &assets, &mut errors);
            Some(Kind::Single { steps, assets })
        }
        (None, Some(tiers)) => {
            if body.assets.is_some() {
                errors.push("a tiered badge's assets are its tiers'".to_string());
            }
            if tiers.is_empty() {
                errors.push("tiers must not be empty".to_string());
            }
            if tiers.len() > MAX_TIERS {
                return Err(ApiError::bad_request(format!(
                    "a badge has at most {MAX_TIERS} tiers, not {}",
                    tiers.len()
                )));
            }
            let prefix = format!("{}-", id.replace('_', "-"));
            let mut seen = HashSet::new();
            let mut previous = 0;
            let mut out = Vec::with_capacity(tiers.len());
            for (i, t) in tiers.into_iter().enumerate() {
                let at = format!("tiers[{i}]");
                let tier_id = t.tier_id.trim().to_string();
                if !valid_id(&tier_id) {
                    errors.push(format!(
                        "{at}.tierId must be 1-64 of [a-z0-9_-], starting with a letter or digit"
                    ));
                } else if !seen.insert(tier_id.clone()) {
                    errors.push(format!("{at}.tierId '{tier_id}' is repeated"));
                }
                let tier_name = text(&format!("{at}.tierName"), &t.tier_name, &mut errors);
                let description = text(&format!("{at}.description"), &t.description, &mut errors);
                let steps = steps(
                    &format!("{at}.criteria.steps"),
                    t.criteria.steps,
                    &mut errors,
                );
                if steps <= previous {
                    errors.push(format!(
                        "{at}.criteria.steps must be above the previous tier's ({previous})"
                    ));
                }
                previous = previous.max(steps);
                let assets = match t.assets {
                    Some(a) => a,
                    None => match tier_id.strip_prefix(&prefix) {
                        Some(slot) if !slot.is_empty() && slot != SINGLE_TIER_SLOT => {
                            layout_assets(&format!("{id}/{slot}"))
                        }
                        _ => {
                            errors.push(format!(
                                "{at}.assets is required unless tierId is '{prefix}<slot>' \
                                 (upstream's <badge>/<slot> layout)"
                            ));
                            Value::Null
                        }
                    },
                };
                if !assets.is_null() {
                    check_assets(&format!("{at}.assets"), &assets, &mut errors);
                }
                out.push(Tier {
                    tier_id,
                    tier_name,
                    description,
                    steps,
                    assets,
                });
            }
            Some(Kind::Tiered(out))
        }
    };
    match kind {
        Some(kind) if errors.is_empty() => Ok(Definition {
            id,
            name,
            description,
            category: body.category,
            kind,
        }),
        _ => Err(ApiError::bad_request(errors.join("; "))),
    }
}

/// A request body as `T`, or a 400 naming what serde refused.
pub fn parse_body<T: serde::de::DeserializeOwned>(bytes: &[u8]) -> Result<T, ApiError> {
    serde_json::from_slice(bytes).map_err(|e| ApiError::bad_request(format!("invalid body: {e}")))
}

fn epoch_ms(ts: NaiveDateTime) -> i64 {
    ts.and_utc().timestamp_millis()
}

async fn audit(
    conn: &mut PgConnection,
    action: &str,
    badge_id: &str,
    definition: Option<Value>,
    actor: &str,
    at: NaiveDateTime,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO social.badge_catalog_audit (action, badge_id, definition, actor, acted_at) \
         VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(action)
    .bind(badge_id)
    .bind(definition)
    .bind(actor)
    .bind(at)
    .execute(conn)
    .await?;
    Ok(())
}

/// Whether any wallet has a progress or tier row of the badge.
async fn has_player_rows(conn: &mut PgConnection, badge_id: &str) -> Result<bool, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM social.user_badge_progress WHERE badge_id = $1) \
             OR EXISTS (SELECT 1 FROM social.user_achieved_tiers WHERE badge_id = $1)",
    )
    .bind(badge_id)
    .fetch_one(conn)
    .await
}

async fn load(conn: &mut PgConnection, badge_id: &str) -> Result<Option<AdminBadge>, sqlx::Error> {
    type Row = (
        String,
        String,
        Option<String>,
        Option<String>,
        bool,
        Value,
        Option<i32>,
        Option<NaiveDateTime>,
        Option<NaiveDateTime>,
        NaiveDateTime,
    );
    let row: Option<Row> = sqlx::query_as(
        "SELECT id, name, description, category, is_tier, assets, criteria_steps, \
                published_at, archived_at, created_at \
         FROM social.badge_definitions WHERE id = $1",
    )
    .bind(badge_id)
    .fetch_optional(&mut *conn)
    .await?;
    let Some((
        id,
        name,
        description,
        category,
        is_tier,
        assets,
        steps,
        published,
        archived,
        created,
    )) = row
    else {
        return Ok(None);
    };
    let tiers = if is_tier {
        let rows: Vec<(String, String, Option<String>, i32, Value)> = sqlx::query_as(
            "SELECT tier_id, tier_name, description, criteria_steps, assets \
             FROM social.badge_tiers WHERE badge_id = $1 ORDER BY ordinal",
        )
        .bind(badge_id)
        .fetch_all(&mut *conn)
        .await?;
        Some(
            rows.into_iter()
                .map(
                    |(tier_id, tier_name, description, steps, assets)| TierData {
                        tier_id,
                        tier_name,
                        description,
                        criteria: TierCriteria { steps },
                        assets,
                    },
                )
                .collect(),
        )
    } else {
        None
    };
    Ok(Some(AdminBadge {
        id,
        name,
        description,
        category,
        criteria: (!is_tier).then(|| TierCriteria {
            steps: steps.unwrap_or(1),
        }),
        assets: (!is_tier).then_some(assets),
        tiers,
        published_at: published.map(epoch_ms),
        archived_at: archived.map(epoch_ms),
        created_at: epoch_ms(created),
    }))
}

impl BadgesStore {
    /// Creates `def` unpublished, or replaces an existing definition's
    /// content (never its publication, archive or creation order). Returns
    /// the stored badge and whether it was created; 409 when the change
    /// would strand player rows.
    pub async fn upsert_definition(
        &self,
        def: &Definition,
        actor: &str,
    ) -> Result<(AdminBadge, bool), ApiError> {
        let now = chrono::Utc::now().naive_utc();
        let (assets, criteria_steps) = match &def.kind {
            Kind::Single { steps, assets } => (assets.clone(), Some(*steps)),
            Kind::Tiered(_) => (json!({}), None),
        };
        let mut tx = self.pool().begin().await?;
        let created: bool = sqlx::query_scalar(
            "INSERT INTO social.badge_definitions \
               (id, name, description, category, is_tier, assets, criteria_steps, created_at) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8) \
             ON CONFLICT (id) DO NOTHING RETURNING true",
        )
        .bind(&def.id)
        .bind(&def.name)
        .bind(&def.description)
        .bind(&def.category)
        .bind(def.is_tier())
        .bind(&assets)
        .bind(criteria_steps)
        .bind(now)
        .fetch_optional(&mut *tx)
        .await?
        .unwrap_or(false);
        if !created {
            let was_tier: bool = sqlx::query_scalar(
                "SELECT is_tier FROM social.badge_definitions WHERE id = $1 FOR UPDATE",
            )
            .bind(&def.id)
            .fetch_one(&mut *tx)
            .await?;
            if was_tier != def.is_tier() && has_player_rows(&mut tx, &def.id).await? {
                return Err(ApiError::conflict(format!(
                    "badge '{}' has player progress, so it cannot change between single and tiered",
                    def.id
                )));
            }
            let kept: Vec<String> = match &def.kind {
                Kind::Tiered(tiers) => tiers.iter().map(|t| t.tier_id.clone()).collect(),
                Kind::Single { .. } => Vec::new(),
            };
            let held: Vec<String> = sqlx::query_scalar(
                "SELECT DISTINCT tier_id FROM social.user_achieved_tiers \
                 WHERE badge_id = $1 AND NOT (tier_id = ANY($2)) ORDER BY tier_id",
            )
            .bind(&def.id)
            .bind(&kept)
            .fetch_all(&mut *tx)
            .await?;
            if !held.is_empty() {
                return Err(ApiError::conflict(format!(
                    "tiers of badge '{}' held by players cannot be removed: {}",
                    def.id,
                    held.join(", ")
                )));
            }
            sqlx::query(
                "UPDATE social.badge_definitions \
                 SET name = $2, description = $3, category = $4, is_tier = $5, assets = $6, \
                     criteria_steps = $7 \
                 WHERE id = $1",
            )
            .bind(&def.id)
            .bind(&def.name)
            .bind(&def.description)
            .bind(&def.category)
            .bind(def.is_tier())
            .bind(&assets)
            .bind(criteria_steps)
            .execute(&mut *tx)
            .await?;
            // catalog rows only: a tier no player holds
            sqlx::query(
                "DELETE FROM social.badge_tiers WHERE badge_id = $1 AND NOT (tier_id = ANY($2))",
            )
            .bind(&def.id)
            .bind(&kept)
            .execute(&mut *tx)
            .await?;
        }
        if let Kind::Tiered(tiers) = &def.kind {
            for (ordinal, t) in tiers.iter().enumerate() {
                sqlx::query(
                    "INSERT INTO social.badge_tiers \
                       (badge_id, tier_id, tier_name, description, assets, criteria_steps, ordinal) \
                     VALUES ($1, $2, $3, $4, $5, $6, $7) \
                     ON CONFLICT (badge_id, tier_id) DO UPDATE SET \
                       tier_name = EXCLUDED.tier_name, description = EXCLUDED.description, \
                       assets = EXCLUDED.assets, criteria_steps = EXCLUDED.criteria_steps, \
                       ordinal = EXCLUDED.ordinal",
                )
                .bind(&def.id)
                .bind(&t.tier_id)
                .bind(&t.tier_name)
                .bind(&t.description)
                .bind(&t.assets)
                .bind(t.steps)
                .bind(ordinal as i32)
                .execute(&mut *tx)
                .await?;
            }
        }
        let recorded = serde_json::to_value(def).map_err(|e| ApiError::internal(e.to_string()))?;
        audit(&mut tx, "upsert", &def.id, Some(recorded), actor, now).await?;
        let badge = load(&mut tx, &def.id)
            .await?
            .ok_or_else(|| ApiError::internal("upserted badge vanished"))?;
        tx.commit().await?;
        self.invalidate_catalog();
        Ok((badge, created))
    }

    /// Moves a badge's publication and archive to `target`, auditing each
    /// transition; `None` for an unknown id. Unpublishing a badge with player
    /// rows is a 409: those players would lose it (archive instead).
    pub async fn set_definition_state(
        &self,
        badge_id: &str,
        target: &StateBody,
        actor: &str,
    ) -> Result<Option<AdminBadge>, ApiError> {
        let now = chrono::Utc::now().naive_utc();
        let mut tx = self.pool().begin().await?;
        let row: Option<(bool, bool)> = sqlx::query_as(
            "SELECT published_at IS NOT NULL, archived_at IS NOT NULL \
             FROM social.badge_definitions WHERE id = $1 FOR UPDATE",
        )
        .bind(badge_id)
        .fetch_optional(&mut *tx)
        .await?;
        let Some((published, archived)) = row else {
            return Ok(None);
        };
        let publish = target.published.filter(|p| *p != published);
        let archive = target.archived.filter(|a| *a != archived);
        if publish == Some(false) && has_player_rows(&mut tx, badge_id).await? {
            return Err(ApiError::conflict(format!(
                "badge '{badge_id}' has player progress, so unpublishing would hide it from \
                 them; archive it instead"
            )));
        }
        if publish.is_some() || archive.is_some() {
            sqlx::query(
                "UPDATE social.badge_definitions SET \
                   published_at = CASE WHEN $2::boolean IS NULL THEN published_at \
                                       WHEN $2 THEN $4 ELSE NULL END, \
                   archived_at = CASE WHEN $3::boolean IS NULL THEN archived_at \
                                      WHEN $3 THEN $4 ELSE NULL END \
                 WHERE id = $1",
            )
            .bind(badge_id)
            .bind(publish)
            .bind(archive)
            .bind(now)
            .execute(&mut *tx)
            .await?;
        }
        for action in [
            publish.map(|p| if p { "publish" } else { "unpublish" }),
            archive.map(|a| if a { "archive" } else { "unarchive" }),
        ]
        .into_iter()
        .flatten()
        {
            audit(&mut tx, action, badge_id, None, actor, now).await?;
        }
        let badge = load(&mut tx, badge_id).await?;
        tx.commit().await?;
        self.invalidate_catalog();
        Ok(badge)
    }
}

#[cfg(test)]
mod validate_tests {
    use super::*;

    fn body(v: Value) -> DefinitionBody {
        serde_json::from_value(v).unwrap()
    }

    fn error(id: &str, v: Value) -> String {
        validate(id, body(v)).unwrap_err().wire().1
    }

    #[test]
    fn a_single_badge_defaults_to_upstreams_layout() {
        let def = validate(
            "fright_night_2026",
            body(json!({
                "name": "Fright Night 2026", "description": "Jumped in",
                "category": "Socializer", "criteria": {"steps": 1}
            })),
        )
        .unwrap();
        match def.kind {
            Kind::Single { steps, assets } => {
                assert_eq!(steps, 1);
                assert_eq!(assets, layout_assets("fright_night_2026"));
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn a_tiered_badge_derives_each_tier_slot() {
        let def = validate(
            "step_counter",
            body(json!({
                "name": "Step Counter", "description": "Walk", "category": "Explorer",
                "tiers": [
                    {"tierId": "step-counter-starter", "tierName": "Starter", "description": "1", "criteria": {"steps": 1}},
                    {"tierId": "custom", "tierName": "Bronze", "description": "5", "criteria": {"steps": 5},
                     "assets": {"2d": {"normal": "step_counter/bronze/2d/normal.png"}}}
                ]
            })),
        )
        .unwrap();
        let Kind::Tiered(tiers) = def.kind else {
            panic!()
        };
        assert_eq!(tiers[0].assets, layout_assets("step_counter/starter"));
        assert_eq!(
            tiers[1].assets["2d"]["normal"],
            "step_counter/bronze/2d/normal.png"
        );
    }

    #[test]
    fn every_rule_is_reported() {
        let msg = error(
            "Bad Id",
            json!({
                "id": "other", "name": " ", "description": "", "category": "Gamer",
                "criteria": {"steps": 0}, "assets": {"2d": {"normal": "../x.png"}, "4d": {}}
            }),
        );
        for part in [
            "badge id must be",
            "does not match the path",
            "name is required",
            "description is required",
            "category must be one of Explorer, Collector, Creator, Socializer, Builder",
            "criteria.steps must be a positive integer",
            "assets.2d.normal must be a relative path",
            "assets takes only \"2d\" and \"3d\"",
        ] {
            assert!(msg.contains(part), "{part} in {msg}");
        }
        assert!(
            error(
                "x",
                json!({"name": "n", "description": "d", "category": "Builder"})
            )
            .contains("needs criteria.steps (single) or tiers (tiered)")
        );
        assert!(
            error(
                "x",
                json!({"name": "n", "description": "d", "category": "Builder",
                       "criteria": {"steps": 1}, "tiers": []})
            )
            .contains("not both")
        );
        let tiered = error(
            "x",
            json!({"name": "n", "description": "d", "category": "Builder",
            "assets": {"2d": {"normal": "x/2d/normal.png"}},
            "tiers": [
                {"tierId": "x-a", "tierName": "A", "description": "a", "criteria": {"steps": 5}},
                {"tierId": "x-a", "tierName": "", "description": "b", "criteria": {"steps": 5}},
                {"tierId": "y", "tierName": "C", "description": "c", "criteria": {"steps": 9}},
                {"tierId": "x-d", "tierName": "D", "description": "d", "criteria": {"steps": 10},
                 "assets": {"2d": {"normal": "https://cdn.example/x.png"}}},
                {"tierId": "x-e", "tierName": "E", "description": "e", "criteria": {"steps": 11},
                 "assets": {"3d": {"normal": "x/e/3d/normal.png"}}}
            ]}),
        );
        for part in [
            "a tiered badge's assets are its tiers'",
            "tiers[1].tierId 'x-a' is repeated",
            "tiers[1].tierName is required",
            "tiers[1].criteria.steps must be above the previous tier's (5)",
            "tiers[2].assets is required unless tierId is 'x-<slot>'",
            "tiers[3].assets.2d.normal must be a relative path",
            "tiers[4].assets.2d.normal is required",
        ] {
            assert!(tiered.contains(part), "{part} in {tiered}");
        }
        assert!(
            error(
                "x",
                json!({"name": "n", "description": "d", "category": "Builder", "tiers": []})
            )
            .contains("tiers must not be empty")
        );
        assert!(
            error(
                "x",
                json!({"name": "n", "description": "d", "category": "Builder",
                       "criteria": {"steps": 3_000_000_000_i64}})
            )
            .contains("criteria.steps must be a positive integer")
        );
    }

    #[test]
    fn unknown_keys_are_refused() {
        let err = parse_body::<DefinitionBody>(
            br#"{"name":"n","description":"d","category":"Builder","criteria":{"steps":1},"tier":[]}"#,
        )
        .unwrap_err()
        .wire();
        assert_eq!(err.0, 400);
        assert!(
            err.1.starts_with("invalid body: unknown field `tier`"),
            "{}",
            err.1
        );
    }
}
