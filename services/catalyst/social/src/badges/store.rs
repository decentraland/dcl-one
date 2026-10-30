//! The SQL and the assembly of the badges routes over `social.badge_*` and
//! `social.user_*`. Definitions and tiers are reference data (seeded by the
//! schema), so every read shares one 300 s catalogue snapshot; a wallet's
//! two tables come in one statement. Timestamps are `timestamp without time
//! zone` in UTC and go out as epoch ms.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use chrono::NaiveDateTime;
use sqlx::Row;
use sqlx::postgres::PgPool;

use super::types::{
    AchievedTier, Assets, BadgeData, BadgeProgress, LatestAchievedBadge, TierCriteria, TierData,
};
use crate::identity::Address;
use crate::notifications::errors::ApiError;
use crate::shared::cache::TtlMap;

/// The host baked into the seed rows; `BADGES_PUBLIC_ASSET_BASE_URL`
/// replaces it at serve time, the rows are never rewritten.
pub const DEFAULT_ASSET_BASE_URL: &str = "https://badges.decentraland.org";
const CATALOG_TTL: Duration = Duration::from_secs(300);

/// Walks arbitrarily nested JSON so it need not know the `{"2d":{...},"3d":{...}}` shape;
/// empty strings (the unfilled fields in the seed) never match a non-empty `from`
/// prefix and pass through unchanged.
pub fn rewrite_base(value: &serde_json::Value, from: &str, to: &str) -> serde_json::Value {
    if from == to {
        return value.clone();
    }
    match value {
        serde_json::Value::String(s) => match s.strip_prefix(from) {
            Some(rest) => serde_json::Value::String(format!("{to}{rest}")),
            None => value.clone(),
        },
        serde_json::Value::Object(map) => serde_json::Value::Object(
            map.iter()
                .map(|(k, v)| (k.clone(), rewrite_base(v, from, to)))
                .collect(),
        ),
        serde_json::Value::Array(arr) => {
            serde_json::Value::Array(arr.iter().map(|v| rewrite_base(v, from, to)).collect())
        }
        other => other.clone(),
    }
}

fn epoch_ms(ts: NaiveDateTime) -> i64 {
    ts.and_utc().timestamp_millis()
}

pub struct BadgesStore {
    pool: PgPool,
    public_asset_base_url: String,
    catalog: TtlMap<(), Arc<Catalog>>,
}

struct Catalog {
    defs: Vec<DefRow>,
    tiers: HashMap<String, Vec<TierRow>>,
}

type AchievedMap = HashMap<String, Vec<(String, NaiveDateTime)>>;

struct DefRow {
    id: String,
    name: String,
    description: Option<String>,
    category: Option<String>,
    is_tier: bool,
    assets: Assets,
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

impl BadgesStore {
    pub fn new(pool: PgPool, public_asset_base_url: String) -> Self {
        Self {
            pool,
            public_asset_base_url,
            catalog: TtlMap::new(CATALOG_TTL),
        }
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    /// Drops the catalogue snapshot (a test that inserts a definition).
    pub fn invalidate_catalog(&self) {
        self.catalog.clear();
    }

    async fn catalog(&self) -> Result<Arc<Catalog>, ApiError> {
        self.catalog
            .get_or_fetch((), || async {
                let (defs, tiers) =
                    tokio::try_join!(self.load_definitions(), self.load_all_tiers())?;
                Ok::<_, ApiError>(Arc::new(Catalog { defs, tiers }))
            })
            .await
    }

    fn rewrite_assets(&self, assets: Assets) -> Assets {
        rewrite_base(&assets, DEFAULT_ASSET_BASE_URL, &self.public_asset_base_url)
    }

    pub async fn list_categories(&self) -> Result<Vec<String>, ApiError> {
        let catalog = self.catalog().await?;
        let mut categories: Vec<String> = catalog
            .defs
            .iter()
            .filter_map(|d| d.category.clone())
            .collect();
        categories.sort();
        categories.dedup();
        Ok(categories)
    }

    async fn load_definitions(&self) -> Result<Vec<DefRow>, ApiError> {
        let rows = sqlx::query(
            "SELECT id, name, description, category, is_tier, assets \
             FROM social.badge_definitions ORDER BY id",
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .into_iter()
            .map(|r| DefRow {
                id: r.get("id"),
                name: r.get("name"),
                description: r.get("description"),
                category: r.get("category"),
                is_tier: r.get("is_tier"),
                assets: self.rewrite_assets(r.get("assets")),
            })
            .collect())
    }

    async fn load_all_tiers(&self) -> Result<HashMap<String, Vec<TierRow>>, ApiError> {
        let rows = sqlx::query(
            "SELECT badge_id, tier_id, tier_name, description, assets, criteria_steps \
             FROM social.badge_tiers ORDER BY badge_id, ordinal",
        )
        .fetch_all(&self.pool)
        .await?;
        let mut map: HashMap<String, Vec<TierRow>> = HashMap::new();
        for r in rows {
            let badge_id: String = r.get("badge_id");
            map.entry(badge_id).or_default().push(TierRow {
                tier_id: r.get("tier_id"),
                tier_name: r.get("tier_name"),
                description: r.get("description"),
                assets: self.rewrite_assets(r.get("assets")),
                criteria_steps: r.get("criteria_steps"),
            });
        }
        Ok(map)
    }

    pub async fn list_tiers(&self, badge_id: &str) -> Result<Vec<TierData>, ApiError> {
        let catalog = self.catalog().await?;
        if !catalog.defs.iter().any(|d| d.id == badge_id) {
            return Err(ApiError::not_found("Badge not found"));
        }
        Ok(catalog
            .tiers
            .get(badge_id)
            .map(|ts| {
                ts.iter()
                    .map(|t| TierData {
                        tier_id: t.tier_id.clone(),
                        tier_name: t.tier_name.clone(),
                        description: t.description.clone(),
                        assets: t.assets.clone(),
                        criteria: TierCriteria {
                            steps: t.criteria_steps,
                        },
                    })
                    .collect()
            })
            .unwrap_or_default())
    }

    /// Both per-address tables in one statement; tier rows arrive in
    /// `completed_at` order, which is the order the assemblers rely on.
    async fn load_user_state(
        &self,
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
             FROM social.user_achieved_tiers WHERE address = $1 \
             ORDER BY 1, 4",
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
        Ok((progress, achieved))
    }

    /// `address` is whatever the path carried, lowercased when it parses:
    /// a malformed one reads no rows and answers empty, as upstream does.
    pub async fn user_badges(
        &self,
        address: &str,
        include_not_achieved: bool,
    ) -> Result<(Vec<BadgeData>, Vec<BadgeData>), ApiError> {
        let catalog = self.catalog().await?;
        let (progress, achieved_tiers) = self.load_user_state(address).await?;

        let mut achieved = Vec::new();
        let mut not_achieved = Vec::new();
        for def in &catalog.defs {
            let prog = progress.get(&def.id);
            let badge_tiers = catalog.tiers.get(&def.id);
            let user_tiers = achieved_tiers.get(&def.id);
            let is_achieved = match prog {
                _ if def.is_tier => user_tiers.map(|t| !t.is_empty()).unwrap_or(false),
                Some(p) => p.completed_at.is_some(),
                None => false,
            };
            if !is_achieved && !include_not_achieved {
                continue;
            }
            let badge = assemble_badge(def, badge_tiers, prog, user_tiers);
            if is_achieved {
                achieved.push(badge)
            } else {
                not_achieved.push(badge)
            }
        }
        Ok((achieved, not_achieved))
    }

    /// One statement: resolves the badge and (for tiered badges) the tier, and
    /// only when that resolves writes the tier, the progress row and the audit
    /// row. `Ok(false)` is "no such badge"; an unresolvable tier keeps its own
    /// error and writes nothing.
    pub async fn grant_badge(
        &self,
        address: &Address,
        badge_id: &str,
        tier_id: Option<&str>,
        granted_by: &str,
    ) -> Result<bool, ApiError> {
        let now = chrono::Utc::now().naive_utc();
        let row: Option<(bool, bool)> = sqlx::query_as(
            "WITH def AS ( \
                 SELECT d.is_tier, t.tier_id, t.criteria_steps \
                 FROM social.badge_definitions d \
                 LEFT JOIN LATERAL ( \
                     SELECT tier_id, criteria_steps FROM social.badge_tiers \
                     WHERE badge_id = d.id AND ($3::text IS NULL OR tier_id = $3) \
                     ORDER BY ordinal DESC LIMIT 1 \
                 ) t ON TRUE \
                 WHERE d.id = $2 \
             ), ok AS ( \
                 SELECT * FROM def WHERE NOT is_tier OR tier_id IS NOT NULL \
             ), tier AS ( \
                 INSERT INTO social.user_achieved_tiers \
                   (address, badge_id, tier_id, completed_at, granted_by, granted_at) \
                 SELECT $1, $2, tier_id, $5, $4, $5 FROM ok WHERE is_tier \
                 ON CONFLICT (address, badge_id, tier_id) DO UPDATE \
                   SET granted_by = EXCLUDED.granted_by, granted_at = EXCLUDED.granted_at \
             ), prog AS ( \
                 INSERT INTO social.user_badge_progress \
                   (address, badge_id, steps_done, completed_at, last_completed_tier_id, \
                    updated_at, granted_by) \
                 SELECT $1, $2, \
                        CASE WHEN is_tier THEN criteria_steps ELSE 1 END, \
                        $5, \
                        CASE WHEN is_tier THEN tier_id END, \
                        $5, $4 \
                 FROM ok \
                 ON CONFLICT (address, badge_id) DO UPDATE SET \
                   steps_done = GREATEST(social.user_badge_progress.steps_done, EXCLUDED.steps_done), \
                   completed_at = COALESCE(social.user_badge_progress.completed_at, EXCLUDED.completed_at), \
                   last_completed_tier_id = COALESCE(EXCLUDED.last_completed_tier_id, \
                                                     social.user_badge_progress.last_completed_tier_id), \
                   updated_at = EXCLUDED.updated_at, \
                   granted_by = EXCLUDED.granted_by \
             ), audit AS ( \
                 INSERT INTO social.badge_admin_audit (action, address, badge_id, tier_id, actor, acted_at) \
                 SELECT 'grant', $1, $2, $3, $4, $5 FROM ok \
             ) \
             SELECT is_tier, tier_id IS NOT NULL FROM def",
        )
        .bind(address)
        .bind(badge_id)
        .bind(tier_id)
        .bind(granted_by)
        .bind(now)
        .fetch_optional(&self.pool)
        .await?;

        match row {
            None => Ok(false),
            Some((true, false)) => Err(match tier_id {
                Some(tid) => ApiError::not_found(format!("no tier '{tid}' on badge '{badge_id}'")),
                None => ApiError::bad_request(format!(
                    "tiered badge '{badge_id}' has no tiers; specify tierId"
                )),
            }),
            Some(_) => Ok(true),
        }
    }

    pub async fn revoke_badge(
        &self,
        address: &Address,
        badge_id: &str,
        revoked_by: &str,
    ) -> Result<bool, ApiError> {
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
        .fetch_one(&self.pool)
        .await?;
        Ok(exists)
    }

    pub async fn latest_achieved(
        &self,
        address: &str,
        limit: i64,
    ) -> Result<Vec<LatestAchievedBadge>, ApiError> {
        let catalog = self.catalog().await?;
        let (progress, achieved_tiers) = self.load_user_state(address).await?;
        let mut rows: Vec<(NaiveDateTime, LatestAchievedBadge)> = Vec::new();
        for def in &catalog.defs {
            if def.is_tier {
                let Some((tier_id, at)) = achieved_tiers.get(&def.id).and_then(|t| t.last()) else {
                    continue;
                };
                let tier_def = catalog
                    .tiers
                    .get(&def.id)
                    .and_then(|defs| defs.iter().find(|t| &t.tier_id == tier_id));
                rows.push((
                    *at,
                    LatestAchievedBadge {
                        id: def.id.clone(),
                        name: def.name.clone(),
                        tier_name: tier_def.map(|t| t.tier_name.clone()),
                        image: tier_def
                            .and_then(|t| tier_image(&t.assets))
                            .or_else(|| tier_image(&def.assets)),
                    },
                ));
            } else if let Some(at) = progress.get(&def.id).and_then(|p| p.completed_at) {
                rows.push((
                    at,
                    LatestAchievedBadge {
                        id: def.id.clone(),
                        name: def.name.clone(),
                        tier_name: None,
                        image: tier_image(&def.assets),
                    },
                ));
            }
        }
        rows.sort_by_key(|b| std::cmp::Reverse(b.0));
        Ok(rows
            .into_iter()
            .take(limit.max(0) as usize)
            .map(|(_, b)| b)
            .collect())
    }
}

fn assemble_badge(
    def: &DefRow,
    badge_tiers: Option<&Vec<TierRow>>,
    prog: Option<&ProgressRow>,
    user_tiers: Option<&Vec<(String, NaiveDateTime)>>,
) -> BadgeData {
    let steps_done = prog.map(|p| p.steps_done).unwrap_or(0);
    let total_steps_target: i32 = match badge_tiers {
        Some(ts) if def.is_tier => ts.iter().map(|t| t.criteria_steps).max().unwrap_or(0),
        _ => 1,
    };
    let achieved_list: Vec<AchievedTier> = user_tiers
        .map(|ts| {
            ts.iter()
                .map(|(tier_id, at)| AchievedTier {
                    tier_id: tier_id.clone(),
                    completed_at: Some(epoch_ms(*at)),
                })
                .collect()
        })
        .unwrap_or_default();
    let last = user_tiers.and_then(|ts| ts.last());
    let last_tier_def = last.and_then(|(tier_id, _)| {
        badge_tiers.and_then(|defs| defs.iter().find(|t| &t.tier_id == tier_id))
    });
    let next_steps_target: Option<i32> = match badge_tiers {
        Some(ts) if def.is_tier => ts
            .iter()
            .map(|t| t.criteria_steps)
            .filter(|&s| s > steps_done)
            .min(),
        _ => {
            if steps_done >= total_steps_target {
                None
            } else {
                Some(total_steps_target)
            }
        }
    };
    BadgeData {
        id: def.id.clone(),
        name: def.name.clone(),
        description: def.description.clone(),
        category: def.category.clone(),
        is_tier: def.is_tier,
        completed_at: prog.and_then(|p| p.completed_at.map(|t| epoch_ms(t).to_string())),
        assets: def.assets.clone(),
        progress: BadgeProgress {
            steps_done,
            next_steps_target,
            total_steps_target,
            last_completed_tier_at: last.map(|(_, at)| epoch_ms(*at)),
            last_completed_tier_name: last_tier_def.map(|t| t.tier_name.clone()),
            last_completed_tier_image: last_tier_def.and_then(|t| tier_image(&t.assets)),
            achieved_tiers: achieved_list,
        },
    }
}

fn tier_image(assets: &Assets) -> Option<String> {
    assets
        .get("2d")
        .and_then(|d| d.get("normal"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
}

#[cfg(test)]
mod rewrite_base_tests {
    use super::rewrite_base;
    use serde_json::json;

    #[test]
    fn swaps_the_host_prefix_on_every_nested_url() {
        let assets = json!({
            "2d": {"normal": "https://badges.decentraland.org/assets/open_for_business/2d/normal.png", "hrm": "", "baseColor": ""},
            "3d": {"normal": "https://badges.decentraland.org/assets/open_for_business/3d/normal.png", "hrm": "", "baseColor": ""}
        });
        let rewritten = rewrite_base(
            &assets,
            "https://badges.decentraland.org",
            "https://badges.interconnected.online",
        );
        assert_eq!(
            rewritten["2d"]["normal"],
            "https://badges.interconnected.online/assets/open_for_business/2d/normal.png"
        );
        assert_eq!(
            rewritten["3d"]["normal"],
            "https://badges.interconnected.online/assets/open_for_business/3d/normal.png"
        );
    }

    #[test]
    fn leaves_empty_strings_untouched() {
        let assets = json!({"2d": {"normal": "", "hrm": "", "baseColor": ""}});
        assert_eq!(
            rewrite_base(
                &assets,
                "https://badges.decentraland.org",
                "https://badges.interconnected.online"
            ),
            assets
        );
    }

    #[test]
    fn is_a_no_op_when_from_and_to_match() {
        let assets = json!({"2d": {"normal": "https://badges.decentraland.org/x.png"}});
        assert_eq!(
            rewrite_base(
                &assets,
                "https://badges.decentraland.org",
                "https://badges.decentraland.org"
            ),
            assets
        );
    }
}
