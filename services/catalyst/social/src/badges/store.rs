//! The SQL and the assembly of the badges routes over `social.badge_*` and
//! `social.user_*`. The published catalog (active and archived badges, in
//! `created_at, id` order) is read on every request, as upstream reads it:
//! one statement of ~0.3 ms over the 34-badge, 84-tier catalog, so a
//! backfill, an import or a console edit shows at once; a wallet's two
//! tables come in one statement. Timestamps are
//! `timestamp without time zone` in UTC and go out as epoch ms. The
//! assembly follows upstream's `badge-service.ts` and `get-user-badges.ts`.

use std::collections::HashMap;
use std::sync::Arc;

use chrono::NaiveDateTime;
use serde_json::Value;
use sqlx::Row;
use sqlx::postgres::PgPool;

use super::evaluators::has_evaluator;
use super::progress::{Applied, ApplyOptions, apply_progress};
use super::types::{
    AchievedTier, Assets, BadgeData, BadgeProgress, CatalogBadge, LastCompletedTier,
    LatestAchievedBadge, RawAchievedTier, TierCriteria, TierData, UserBadgeRow,
};
use crate::identity::Address;
use crate::notifications::errors::ApiError;

/// Upstream's production `CDN_HOST`: the default base every asset path
/// resolves against, so the URLs are upstream's.
pub const DEFAULT_ASSET_BASE_URL: &str = "https://assets-cdn.decentraland.org";
/// The prefix earlier catalog rows (and the TS database import) wrote in
/// front of the same relative paths; it resolves like a relative path.
pub const LEGACY_ASSET_PREFIX: &str = "https://badges.decentraland.org/assets/";
/// Upstream's category enum, in its order (`GET /categories`).
pub const CATEGORIES: [&str; 5] = ["Explorer", "Collector", "Creator", "Socializer", "Builder"];
const BADGE_NOTIFICATION_NS: uuid::Uuid =
    uuid::Uuid::from_u128(0x3b1e_6f0a_92c4_4d57_8e21_b6a0_c9d3_7f14);

/// Every string of an `assets` document as a URL: a relative path (or one
/// under [`LEGACY_ASSET_PREFIX`]) becomes `{base}/{path}`; an absolute URL
/// and an empty string pass through.
pub fn resolve_assets(value: &Value, base: &str) -> Value {
    match value {
        Value::String(s)
            if s.is_empty() || (s.contains("://") && !s.starts_with(LEGACY_ASSET_PREFIX)) =>
        {
            value.clone()
        }
        Value::String(s) => {
            let path = s.strip_prefix(LEGACY_ASSET_PREFIX).unwrap_or(s);
            Value::String(format!("{base}/{}", path.trim_start_matches('/')))
        }
        Value::Object(map) => Value::Object(
            map.iter()
                .map(|(k, v)| (k.clone(), resolve_assets(v, base)))
                .collect(),
        ),
        Value::Array(arr) => Value::Array(arr.iter().map(|v| resolve_assets(v, base)).collect()),
        other => other.clone(),
    }
}

fn epoch_ms(ts: NaiveDateTime) -> i64 {
    ts.and_utc().timestamp_millis()
}

fn image(assets: &Assets) -> Option<String> {
    assets
        .get("2d")
        .and_then(|d| d.get("normal"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
}

/// The `badge_granted` notification upstream's processor published, as the
/// notifications parser stored it; keyed per (address, badge, tier) so a
/// repeated award never notifies twice. `assets` are already resolved.
pub(super) fn badge_granted_notification(
    address: &Address,
    badge_id: &str,
    tier_id: Option<&str>,
    description: &str,
    assets: &Assets,
    at: NaiveDateTime,
) -> crate::notifications::NewNotification {
    let key = format!(
        "badge:{}:{badge_id}:{}",
        address.as_str(),
        tier_id.unwrap_or_default()
    );
    crate::notifications::NewNotification {
        id: uuid::Uuid::new_v5(&BADGE_NOTIFICATION_NS, key.as_bytes()),
        address: address.clone(),
        kind: "badge_granted".into(),
        metadata: serde_json::json!({
            "id": badge_id,
            "title": "New Badge Unlocked!",
            "description": description,
            "image": image(assets),
        }),
        occurred_at: at,
    }
}

pub struct BadgesStore {
    pool: PgPool,
    public_asset_base_url: String,
}

struct Catalog {
    /// Published badges (active and archived) in catalog order.
    defs: Vec<DefRow>,
}

impl Catalog {
    fn get(&self, id: &str) -> Option<&DefRow> {
        self.defs.iter().find(|d| d.id == id)
    }
}

struct DefRow {
    id: String,
    name: String,
    description: Option<String>,
    category: Option<String>,
    is_tier: bool,
    criteria_steps: Option<i32>,
    archived: bool,
    assets: Assets,
    tiers: Vec<TierRow>,
}

impl DefRow {
    /// A single badge's target.
    fn target(&self) -> i32 {
        self.criteria_steps.unwrap_or(1)
    }

    fn tier(&self, tier_id: &str) -> Option<(usize, &TierRow)> {
        self.tiers
            .iter()
            .enumerate()
            .find(|(_, t)| t.tier_id == tier_id)
    }

    fn tier_data(&self) -> Vec<TierData> {
        self.tiers
            .iter()
            .map(|t| TierData {
                tier_id: t.tier_id.clone(),
                tier_name: t.tier_name.clone(),
                description: t.description.clone(),
                criteria: TierCriteria {
                    steps: t.criteria_steps,
                },
                assets: t.assets.clone(),
            })
            .collect()
    }

    fn catalog_badge(&self) -> CatalogBadge {
        let single = !self.is_tier;
        CatalogBadge {
            id: self.id.clone(),
            name: self.name.clone(),
            category: self.category.clone(),
            description: self.description.clone(),
            criteria: single.then(|| TierCriteria {
                steps: self.target(),
            }),
            assets: single.then(|| self.assets.clone()),
            tiers: self.is_tier.then(|| self.tier_data()),
            archived: self.archived,
        }
    }
}

struct TierRow {
    tier_id: String,
    tier_name: String,
    description: Option<String>,
    assets: Assets,
    criteria_steps: i32,
}

struct ProgressRow {
    steps_done: i32,
    completed_at: Option<NaiveDateTime>,
}

/// A wallet's earned tiers per badge, in earning order (stamp, then tier order).
type AchievedMap = HashMap<String, Vec<(String, NaiveDateTime)>>;

impl BadgesStore {
    pub fn new(pool: PgPool, public_asset_base_url: String) -> Self {
        Self {
            pool,
            public_asset_base_url,
        }
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    /// `BADGES_PUBLIC_ASSET_BASE_URL` without a trailing slash: the base
    /// [`ApplyOptions::notify`] takes.
    pub fn asset_base_url(&self) -> &str {
        &self.public_asset_base_url
    }

    /// Nothing to drop: the catalog is read per request. Kept for callers
    /// written against the earlier snapshot.
    pub fn invalidate_catalog(&self) {}

    async fn catalog(&self) -> Result<Arc<Catalog>, ApiError> {
        Ok(Arc::new(self.load_catalog().await?))
    }

    /// One statement, so a publish or archive never splits the snapshot. A
    /// tiered row without tiers is not a valid definition and is left out,
    /// as upstream's catalog leaves out invalid rows.
    async fn load_catalog(&self) -> Result<Catalog, ApiError> {
        let rows = sqlx::query(
            "SELECT d.id, d.name, d.description, d.category, d.is_tier, d.criteria_steps, \
                    d.archived_at IS NOT NULL AS archived, d.assets, \
                    COALESCE((SELECT jsonb_agg(jsonb_build_object( \
                                 'tier_id', t.tier_id, 'tier_name', t.tier_name, \
                                 'description', t.description, 'assets', t.assets, \
                                 'steps', t.criteria_steps) ORDER BY t.ordinal) \
                              FROM social.badge_tiers t WHERE t.badge_id = d.id), '[]'::jsonb) AS tiers \
             FROM social.badge_definitions d \
             WHERE d.published_at IS NOT NULL \
             ORDER BY d.created_at, d.id",
        )
        .fetch_all(&self.pool)
        .await?;
        let base = &self.public_asset_base_url;
        let mut defs = Vec::with_capacity(rows.len());
        for r in rows {
            let is_tier: bool = r.get("is_tier");
            let tiers: Vec<TierRow> = if is_tier {
                r.get::<Value, _>("tiers")
                    .as_array()
                    .into_iter()
                    .flatten()
                    .map(|t| TierRow {
                        tier_id: t["tier_id"].as_str().unwrap_or_default().to_string(),
                        tier_name: t["tier_name"].as_str().unwrap_or_default().to_string(),
                        description: t["description"].as_str().map(str::to_string),
                        assets: resolve_assets(&t["assets"], base),
                        criteria_steps: t["steps"].as_i64().unwrap_or(0) as i32,
                    })
                    .collect()
            } else {
                Vec::new()
            };
            let id: String = r.get("id");
            if is_tier && tiers.is_empty() {
                tracing::warn!(badge = %id, "badges: a tiered badge without tiers is left out");
                continue;
            }
            defs.push(DefRow {
                id,
                name: r.get("name"),
                description: r.get("description"),
                category: r.get("category"),
                is_tier,
                criteria_steps: r.get("criteria_steps"),
                archived: r.get("archived"),
                assets: resolve_assets(&r.get::<Value, _>("assets"), base),
                tiers,
            });
        }
        Ok(Catalog { defs })
    }

    /// Startup check (upstream's observer registry): warns about every active
    /// badge this build cannot grant automatically; it is still served.
    pub async fn warn_ahead_of_code(&self) -> Result<Vec<String>, sqlx::Error> {
        let ids: Vec<String> = sqlx::query_scalar(
            "SELECT d.id FROM social.badge_definitions d \
             WHERE d.published_at IS NOT NULL AND d.archived_at IS NULL \
               AND NOT EXISTS (SELECT 1 FROM social.presence_badges p WHERE p.badge_id = d.id) \
             ORDER BY d.id",
        )
        .fetch_all(&self.pool)
        .await?;
        let ahead: Vec<String> = ids.into_iter().filter(|id| !has_evaluator(id)).collect();
        for id in &ahead {
            tracing::warn!(badge = %id, "a published badge has no evaluator, backfill import or presence configuration in this build; only console grants award it");
        }
        Ok(ahead)
    }

    /// `GET /badges`: every published badge, archived ones marked.
    pub async fn list_badges(&self) -> Result<Vec<CatalogBadge>, ApiError> {
        Ok(self
            .catalog()
            .await?
            .defs
            .iter()
            .map(DefRow::catalog_badge)
            .collect())
    }

    /// `GET /badges/{id}`: `None` for an unknown or unpublished id.
    pub async fn badge(&self, badge_id: &str) -> Result<Option<CatalogBadge>, ApiError> {
        Ok(self
            .catalog()
            .await?
            .get(badge_id)
            .map(DefRow::catalog_badge))
    }

    /// `None` for an unknown or unpublished id; `[]` for a single badge.
    pub async fn list_tiers(&self, badge_id: &str) -> Result<Option<Vec<TierData>>, ApiError> {
        Ok(self.catalog().await?.get(badge_id).map(DefRow::tier_data))
    }

    /// Both per-address tables in one statement.
    async fn load_user_state(
        &self,
        catalog: &Catalog,
        address: &str,
    ) -> Result<(HashMap<String, ProgressRow>, AchievedMap), ApiError> {
        let rows = sqlx::query_as::<
            _,
            (
                String,
                String,
                Option<i32>,
                Option<NaiveDateTime>,
                Option<String>,
            ),
        >(
            "SELECT 'p' AS kind, badge_id, steps_done, completed_at, NULL::text AS tier_id \
             FROM social.user_badge_progress WHERE address = $1 \
             UNION ALL \
             SELECT 't', badge_id, NULL::integer, completed_at, tier_id \
             FROM social.user_achieved_tiers WHERE address = $1",
        )
        .bind(address)
        .fetch_all(&self.pool)
        .await?;
        let mut progress = HashMap::new();
        let mut achieved: AchievedMap = HashMap::new();
        for (kind, badge_id, steps_done, completed_at, tier_id) in rows {
            match (kind.as_str(), tier_id, completed_at) {
                ("t", Some(tier_id), Some(at)) => {
                    achieved.entry(badge_id).or_default().push((tier_id, at))
                }
                ("p", _, completed_at) => {
                    progress.insert(
                        badge_id,
                        ProgressRow {
                            steps_done: steps_done.unwrap_or(0),
                            completed_at,
                        },
                    );
                }
                _ => {}
            }
        }
        for (badge_id, tiers) in achieved.iter_mut() {
            let def = catalog.get(badge_id);
            let position = |tier_id: &str| {
                def.and_then(|d| d.tier(tier_id))
                    .map_or(usize::MAX, |(i, _)| i)
            };
            tiers.sort_by(|a, b| a.1.cmp(&b.1).then(position(&a.0).cmp(&position(&b.0))));
        }
        Ok((progress, achieved))
    }

    /// `address` is whatever the path carried, lowercased when it parses:
    /// a malformed one reads no rows and answers empty, as upstream does.
    /// An archived badge is listed only as achieved, with nothing left to
    /// reach; the merged list keeps catalog order.
    pub async fn user_badges(
        &self,
        address: &str,
        include_not_achieved: bool,
    ) -> Result<(Vec<BadgeData>, Vec<BadgeData>), ApiError> {
        let catalog = self.catalog().await?;
        let (progress, achieved_tiers) = self.load_user_state(&catalog, address).await?;
        let mut achieved = Vec::new();
        let mut not_achieved = Vec::new();
        for def in &catalog.defs {
            let prog = progress.get(&def.id);
            let user_tiers = achieved_tiers
                .get(&def.id)
                .map(Vec::as_slice)
                .unwrap_or(&[]);
            match assemble_badge(def, prog, user_tiers) {
                Some(badge) => achieved.push(badge),
                None if include_not_achieved && !def.archived => {
                    not_achieved.push(not_achieved_badge(def, prog))
                }
                None => {}
            }
        }
        Ok((achieved, not_achieved))
    }

    /// Upstream's preview: one row per achievement instant of a badge (each
    /// earned tier, and the completion), newest first, paged. A completion
    /// that shares its instant with an earned tier is that tier's row.
    pub async fn latest_achieved(
        &self,
        address: &str,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<LatestAchievedBadge>, ApiError> {
        let catalog = self.catalog().await?;
        let (progress, achieved_tiers) = self.load_user_state(&catalog, address).await?;
        // (badge position, instant) -> the highest known tier earned then
        let mut rows: HashMap<(usize, NaiveDateTime), Option<usize>> = HashMap::new();
        for (index, def) in catalog.defs.iter().enumerate() {
            for (tier_id, at) in achieved_tiers.get(&def.id).into_iter().flatten() {
                let tier = def.tier(tier_id).map(|(i, _)| i);
                let slot = rows.entry((index, *at)).or_insert(None);
                *slot = (*slot).max(tier);
            }
            if let Some(at) = progress.get(&def.id).and_then(|p| p.completed_at) {
                rows.entry((index, at)).or_insert(None);
            }
        }
        let mut rows: Vec<_> = rows.into_iter().collect();
        rows.sort_by(|((ia, a), _), ((ib, b), _)| b.cmp(a).then(ia.cmp(ib)));
        Ok(rows
            .into_iter()
            .skip(offset.max(0) as usize)
            .take(limit.max(0) as usize)
            .map(|((index, _), tier)| {
                let def = &catalog.defs[index];
                let tier = tier.map(|i| &def.tiers[i]);
                let fallback = match def.tiers.first() {
                    Some(first) => image(&first.assets),
                    None => image(&def.assets),
                };
                LatestAchievedBadge {
                    id: def.id.clone(),
                    name: def.name.clone(),
                    tier_name: tier.map(|t| t.tier_name.clone()),
                    image: tier.and_then(|t| image(&t.assets)).or(fallback),
                }
            })
            .collect())
    }

    /// The admin read of upstream's raw progress row: the outer `None` is an
    /// unknown or unpublished badge, the inner one a wallet with no row.
    pub async fn user_badge(
        &self,
        address: &str,
        badge_id: &str,
    ) -> Result<Option<Option<UserBadgeRow>>, ApiError> {
        let catalog = self.catalog().await?;
        let Some(def) = catalog.get(badge_id) else {
            return Ok(None);
        };
        let row: Option<(i32, Value, Option<NaiveDateTime>, NaiveDateTime)> = sqlx::query_as(
            "SELECT steps_done, progress, completed_at, updated_at FROM social.user_badge_progress \
             WHERE address = $1 AND badge_id = $2",
        )
        .bind(address)
        .bind(badge_id)
        .fetch_optional(&self.pool)
        .await?;
        let Some((steps, document, completed_at, updated_at)) = row else {
            return Ok(Some(None));
        };
        let achieved_tiers = if def.is_tier {
            let (_, achieved) = self.load_user_state(&catalog, address).await?;
            Some(
                achieved
                    .get(badge_id)
                    .into_iter()
                    .flatten()
                    .map(|(tier_id, at)| RawAchievedTier {
                        tier_id: tier_id.clone(),
                        completed_at: epoch_ms(*at),
                    })
                    .collect(),
            )
        } else {
            None
        };
        let mut progress = match document {
            Value::Object(map) => map,
            _ => serde_json::Map::new(),
        };
        progress.insert("steps".into(), Value::from(steps));
        Ok(Some(Some(UserBadgeRow {
            badge_id: badge_id.to_string(),
            user_address: address.to_string(),
            progress: Value::Object(progress),
            achieved_tiers,
            completed_at: completed_at.map(|t| epoch_ms(t).to_string()),
            updated_at: epoch_ms(updated_at).to_string(),
        })))
    }

    /// The console grant: raises the wallet's steps to the target of
    /// `tier_id` (required for a tiered badge, refused on a single one), so
    /// every tier up to it is earned and the badge completes only at its last
    /// tier, then audits it, through [`apply_progress`] with notifications.
    /// `Ok(false)` is "no such published badge"; an archived badge, a missing
    /// or unknown tier keep their own errors and write nothing. The definition
    /// is read `FOR SHARE`, so an archive or catalog write waits for the grant
    /// or is seen by it, and the target and audited tier are the awarded ones.
    pub async fn grant_badge(
        &self,
        address: &Address,
        badge_id: &str,
        tier_id: Option<&str>,
        granted_by: &str,
    ) -> Result<bool, ApiError> {
        let now = chrono::Utc::now().naive_utc();
        let mut tx = self.pool.begin().await?;
        let def: Option<(bool, Option<i32>, bool, bool)> = sqlx::query_as(
            "SELECT is_tier, criteria_steps, published_at IS NOT NULL, archived_at IS NOT NULL \
             FROM social.badge_definitions WHERE id = $1 FOR SHARE",
        )
        .bind(badge_id)
        .fetch_optional(&mut *tx)
        .await?;
        let Some((is_tier, criteria_steps, true, archived)) = def else {
            return Ok(false);
        };
        if archived {
            return Err(ApiError::bad_request(format!(
                "badge '{badge_id}' is archived and can no longer be granted"
            )));
        }
        let target = if is_tier {
            let Some(tid) = tier_id else {
                return Err(ApiError::bad_request(format!(
                    "tiered badge '{badge_id}' needs a tierId"
                )));
            };
            let steps: Option<i32> = sqlx::query_scalar(
                "SELECT criteria_steps FROM social.badge_tiers WHERE badge_id = $1 AND tier_id = $2",
            )
            .bind(badge_id)
            .bind(tid)
            .fetch_optional(&mut *tx)
            .await?;
            steps.ok_or_else(|| {
                ApiError::not_found(format!("no tier '{tid}' on badge '{badge_id}'"))
            })?
        } else if let Some(tid) = tier_id {
            return Err(ApiError::bad_request(format!(
                "badge '{badge_id}' has no tiers; tierId '{tid}' does not apply"
            )));
        } else {
            criteria_steps.unwrap_or(1)
        };
        let options = ApplyOptions {
            at: now,
            notify: Some(&self.public_asset_base_url),
            granted_by: Some(granted_by),
        };
        let applied = apply_progress(&mut tx, address, badge_id, &options, |p| {
            p.steps = p.steps.max(target)
        })
        .await?;
        if applied == Applied::NotAwardable {
            return Ok(false);
        }
        sqlx::query(
            "INSERT INTO social.badge_admin_audit (action, address, badge_id, tier_id, actor, acted_at) \
             VALUES ('grant', $1, $2, $3, $4, $5)",
        )
        .bind(address)
        .bind(badge_id)
        .bind(tier_id)
        .bind(granted_by)
        .bind(now)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(true)
    }

    /// Deletes the wallet's rows of any defined badge (published or not),
    /// serialized with [`apply_progress`] on its advisory lock.
    pub async fn revoke_badge(
        &self,
        address: &Address,
        badge_id: &str,
        revoked_by: &str,
    ) -> Result<bool, ApiError> {
        let mut tx = self.pool.begin().await?;
        sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1, 0))")
            .bind(format!("badge-progress:{}:{badge_id}", address.as_str()))
            .execute(&mut *tx)
            .await?;
        let exists: bool = sqlx::query_scalar(
            "WITH def AS ( \
                 SELECT id FROM social.badge_definitions WHERE id = $2 \
             ), tiers AS ( \
                 DELETE FROM social.user_achieved_tiers USING def \
                 WHERE address = $1 AND badge_id = def.id \
             ), prog AS ( \
                 DELETE FROM social.user_badge_progress USING def \
                 WHERE address = $1 AND badge_id = def.id \
             ), audit AS ( \
                 INSERT INTO social.badge_admin_audit (action, address, badge_id, actor, acted_at) \
                 SELECT 'revoke', $1, id, $3, $4 FROM def \
             ) \
             SELECT EXISTS (SELECT 1 FROM def)",
        )
        .bind(address)
        .bind(badge_id)
        .bind(revoked_by)
        .bind(chrono::Utc::now().naive_utc())
        .fetch_one(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(exists)
    }
}

/// An achieved badge as upstream's `calculateUserProgress` builds it, or
/// `None` when the wallet has not achieved it (no completion and no earned
/// tier the catalog knows).
fn assemble_badge(
    def: &DefRow,
    prog: Option<&ProgressRow>,
    user_tiers: &[(String, NaiveDateTime)],
) -> Option<BadgeData> {
    let steps_done = prog.map(|p| p.steps_done).unwrap_or(0);
    let completed_at = prog.and_then(|p| p.completed_at);
    let last = user_tiers.last().filter(|_| def.is_tier);
    let current = last.and_then(|(tier_id, _)| def.tier(tier_id));
    if completed_at.is_none() && current.is_none() {
        return None;
    }
    let (next, total, assets, last_completed, achieved_tiers) = if def.is_tier {
        let total = def.tiers.last().map_or(0, |t| t.criteria_steps);
        let next_index = match last {
            Some((tier_id, _)) => def.tier(tier_id).map_or(0, |(i, _)| i + 1),
            None => 0,
        };
        let next = def
            .tiers
            .get(next_index)
            .map_or(total, |t| t.criteria_steps);
        let current = current.map(|(_, t)| t);
        (
            next,
            total,
            current.map(|t| t.assets.clone()),
            LastCompletedTier {
                at: last.map(|(_, at)| epoch_ms(*at)),
                name: current.map(|t| t.tier_name.clone()),
                image: current.and_then(|t| image(&t.assets)),
            },
            Some(
                user_tiers
                    .iter()
                    .map(|(tier_id, at)| AchievedTier {
                        tier_id: tier_id.clone(),
                        completed_at: epoch_ms(*at),
                    })
                    .collect(),
            ),
        )
    } else {
        (
            def.target(),
            def.target(),
            Some(def.assets.clone()),
            LastCompletedTier {
                at: None,
                name: None,
                image: None,
            },
            None,
        )
    };
    Some(BadgeData {
        id: def.id.clone(),
        name: def.name.clone(),
        description: def.description.clone(),
        category: def.category.clone(),
        is_tier: def.is_tier,
        completed_at: completed_at.map(|t| epoch_ms(t).to_string()),
        assets,
        progress: BadgeProgress {
            achieved_tiers,
            steps_done,
            next_steps_target: (completed_at.is_none() && !def.archived).then_some(next),
            total_steps_target: total,
            last_completed: Some(last_completed),
        },
    })
}

fn not_achieved_badge(def: &DefRow, prog: Option<&ProgressRow>) -> BadgeData {
    let (next, total, assets) = match (def.tiers.first(), def.tiers.last()) {
        (Some(first), Some(last)) if def.is_tier => (
            first.criteria_steps,
            last.criteria_steps,
            first.assets.clone(),
        ),
        _ => (def.target(), def.target(), def.assets.clone()),
    };
    BadgeData {
        id: def.id.clone(),
        name: def.name.clone(),
        description: def.description.clone(),
        category: def.category.clone(),
        is_tier: def.is_tier,
        completed_at: None,
        assets: Some(assets),
        progress: BadgeProgress {
            achieved_tiers: None,
            steps_done: prog.map(|p| p.steps_done).unwrap_or(0),
            next_steps_target: Some(next),
            total_steps_target: total,
            last_completed: None,
        },
    }
}

#[cfg(test)]
mod resolve_assets_tests {
    use super::{DEFAULT_ASSET_BASE_URL, resolve_assets};
    use serde_json::json;

    #[test]
    fn relative_and_legacy_paths_resolve_against_the_base() {
        let assets = json!({
            "2d": {"normal": "traveler/starter/2d/normal.png"},
            "3d": {
                "normal": "https://badges.decentraland.org/assets/traveler/starter/3d/normal.png",
                "hrm": "https://elsewhere.example/x.png",
                "basecolor": ""
            }
        });
        assert_eq!(
            resolve_assets(&assets, DEFAULT_ASSET_BASE_URL),
            json!({
                "2d": {"normal": "https://assets-cdn.decentraland.org/traveler/starter/2d/normal.png"},
                "3d": {
                    "normal": "https://assets-cdn.decentraland.org/traveler/starter/3d/normal.png",
                    "hrm": "https://elsewhere.example/x.png",
                    "basecolor": ""
                }
            })
        );
        assert_eq!(
            resolve_assets(
                &json!({"2d": {"normal": "/a/2d/normal.png"}}),
                "https://node.example/badges/assets"
            ),
            json!({"2d": {"normal": "https://node.example/badges/assets/a/2d/normal.png"}})
        );
    }
}
