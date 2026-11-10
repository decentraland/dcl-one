//! The wire shapes of badges.decentraland.org. Optional keys are omitted
//! where upstream's JSON omits them (an `undefined` there), and `null` where
//! upstream writes `null`.

use serde::Serialize;

pub type Assets = serde_json::Value;

#[derive(Debug, Serialize)]
pub struct Data<T: Serialize> {
    pub data: T,
}

impl<T: Serialize> Data<T> {
    pub fn new(data: T) -> Self {
        Self { data }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct LatestAchievedBadge {
    pub id: String,
    pub name: String,
    #[serde(rename = "tierName", skip_serializing_if = "Option::is_none")]
    pub tier_name: Option<String>,
    pub image: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct AchievedTier {
    #[serde(rename = "tierId")]
    pub tier_id: String,
    #[serde(rename = "completedAt")]
    pub completed_at: i64,
}

/// The last-tier keys of an achieved badge: values for a tiered one, `null`
/// for a single one; absent on a not-achieved badge.
#[derive(Debug, Clone, Serialize)]
pub struct LastCompletedTier {
    #[serde(rename = "lastCompletedTierAt")]
    pub at: Option<i64>,
    #[serde(rename = "lastCompletedTierName")]
    pub name: Option<String>,
    #[serde(rename = "lastCompletedTierImage")]
    pub image: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct BadgeProgress {
    /// Only on an achieved tiered badge.
    #[serde(rename = "achievedTiers", skip_serializing_if = "Option::is_none")]
    pub achieved_tiers: Option<Vec<AchievedTier>>,
    #[serde(rename = "stepsDone")]
    pub steps_done: i32,
    #[serde(rename = "nextStepsTarget")]
    pub next_steps_target: Option<i32>,
    #[serde(rename = "totalStepsTarget")]
    pub total_steps_target: i32,
    #[serde(flatten)]
    pub last_completed: Option<LastCompletedTier>,
}

#[derive(Debug, Clone, Serialize)]
pub struct BadgeData {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub category: Option<String>,
    #[serde(rename = "isTier")]
    pub is_tier: bool,
    /// Epoch ms as a string (upstream's shape), unlike the tier stamps.
    #[serde(rename = "completedAt")]
    pub completed_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub assets: Option<Assets>,
    pub progress: BadgeProgress,
}

#[derive(Debug, Clone, Serialize)]
pub struct TierData {
    #[serde(rename = "tierId")]
    pub tier_id: String,
    #[serde(rename = "tierName")]
    pub tier_name: String,
    pub description: Option<String>,
    pub criteria: TierCriteria,
    pub assets: Assets,
}

#[derive(Debug, Clone, Serialize)]
pub struct TierCriteria {
    pub steps: i32,
}

/// A catalog entry of `GET /badges` and `GET /badges/{id}`: a single badge
/// carries `criteria` and `assets`, a tiered one `tiers`.
#[derive(Debug, Clone, Serialize)]
pub struct CatalogBadge {
    pub id: String,
    pub name: String,
    pub category: Option<String>,
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub criteria: Option<TierCriteria>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub assets: Option<Assets>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tiers: Option<Vec<TierData>>,
    pub archived: bool,
}

#[derive(Debug, Serialize)]
pub struct BadgeBody {
    pub badge: CatalogBadge,
}

#[derive(Debug, Serialize)]
pub struct CategoriesBody {
    pub categories: Vec<&'static str>,
}

#[derive(Debug, Serialize)]
pub struct PreviewBody {
    #[serde(rename = "latestAchievedBadges")]
    pub latest_achieved_badges: Vec<LatestAchievedBadge>,
}

#[derive(Debug, Serialize)]
pub struct UserBadgesBody {
    pub achieved: Vec<BadgeData>,
    #[serde(rename = "notAchieved")]
    pub not_achieved: Vec<BadgeData>,
}

#[derive(Debug, Serialize)]
pub struct TiersBody {
    pub tiers: Vec<TierData>,
}

/// Upstream's raw `user_progress` row (the admin `GET
/// /users/{address}/badges/{id}`): bigint stamps as strings, tier stamps as
/// numbers, `achieved_tiers` null on a single badge.
#[derive(Debug, Serialize)]
pub struct UserBadgeRow {
    pub badge_id: String,
    pub user_address: String,
    pub progress: serde_json::Value,
    pub achieved_tiers: Option<Vec<RawAchievedTier>>,
    pub completed_at: Option<String>,
    pub updated_at: String,
}

#[derive(Debug, Serialize)]
pub struct RawAchievedTier {
    pub tier_id: String,
    pub completed_at: i64,
}

#[derive(Debug, Serialize)]
pub struct StatusBody {
    pub version: &'static str,
    #[serde(rename = "currentTime")]
    pub current_time: i64,
    #[serde(rename = "commitHash")]
    pub commit_hash: String,
}

#[derive(Debug, Serialize)]
pub struct GrantResult {
    pub granted: bool,
    pub address: String,
    #[serde(rename = "badgeId")]
    pub badge_id: String,
    #[serde(rename = "tierId")]
    pub tier_id: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct RevokeResult {
    pub revoked: bool,
    pub address: String,
    #[serde(rename = "badgeId")]
    pub badge_id: String,
}
