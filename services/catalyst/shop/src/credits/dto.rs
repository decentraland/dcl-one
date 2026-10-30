//! The wire shapes of the user routes (the sites' generated `credits/*.ts` bindings were derived from these; the `ts-rs` export is not carried).

use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "credits/"))]
pub struct SeasonsData {
    #[serde(rename = "lastSeason")]
    pub last_season: SeasonData,
    #[serde(rename = "currentSeason")]
    pub current_season: CurrentSeasonInfo,
    #[serde(rename = "nextSeason")]
    pub next_season: SeasonData,
}

#[derive(Debug, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "credits/"))]
pub struct CurrentSeasonInfo {
    pub season: SeasonData,
    pub week: Week,
}

#[derive(Debug, Default, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "credits/"))]
pub struct SeasonData {
    pub id: i32,
    pub name: String,
    #[serde(rename = "startDate")]
    pub start_date: String,
    #[serde(rename = "endDate")]
    pub end_date: String,
    #[serde(rename = "maxMana")]
    pub max_mana: String,
    #[serde(rename = "timeLeft")]
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub time_left: i64,
    #[serde(rename = "amountOfWeeks")]
    pub amount_of_weeks: i32,
    pub state: String,
}

#[derive(Debug, Default, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "credits/"))]
pub struct Week {
    #[serde(rename = "weekNumber")]
    pub week_number: i32,
    #[serde(rename = "timeLeft")]
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub time_left: u64,
    #[serde(rename = "startDate")]
    pub start_date: String,
    #[serde(rename = "endDate")]
    pub end_date: String,
    #[serde(rename = "secondsRemaining")]
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub seconds_remaining: u64,
}

#[derive(Debug, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "credits/"))]
pub struct CreditsProgramProgressResponse {
    pub user: UserData,
    pub credits: CreditsData,
    pub goals: Vec<GoalData>,
}

#[derive(Debug, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "credits/"))]
pub struct UserData {
    #[serde(rename = "hasStartedProgram")]
    pub has_started_program: bool,
}

/// Whole credits (the balance's floor at the peg; upstream renders whole
/// credits too). Integer on the wire: the peg makes a credit ten cents and
/// no money path keeps a fraction of one.
#[derive(Debug, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "credits/"))]
pub struct CreditsData {
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub available: i64,
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub earned: i64,
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub paid: i64,
    #[serde(rename = "expiresIn")]
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub expires_in: u64,
    #[serde(rename = "isBlockedForClaiming")]
    pub is_blocked_for_claiming: bool,
}

#[derive(Debug, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "credits/"))]
pub struct GoalData {
    pub title: String,
    pub description: String,
    pub thumbnail: String,
    pub progress: GoalProgressData,
    pub reward: f64,
    #[serde(rename = "isClaimed")]
    pub is_claimed: bool,
}

#[derive(Debug, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "credits/"))]
pub struct GoalProgressData {
    #[serde(rename = "totalSteps")]
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub total_steps: u64,
    #[serde(rename = "completedSteps")]
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub completed_steps: u64,
}

#[derive(Debug, Deserialize)]
pub struct ClaimCreditsBody {
    pub x: f64,

    #[serde(default)]
    pub token: Option<String>,
}

#[derive(Debug, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "credits/"))]
pub struct ClaimCreditsResponse {
    pub ok: bool,
    pub credits_granted: f64,
    #[serde(rename = "isBlockedForClaiming")]
    pub is_blocked_for_claiming: bool,
}

#[derive(Debug, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "credits/"))]
pub struct UserCreditsResponse {
    pub credits: Vec<UserCreditItem>,
    #[serde(rename = "totalCredits")]
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub total_credits: i64,
    pub totals: CreditsTotals,
    pub usd: UsdCredits,
}

#[derive(Debug, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "credits/"))]
pub struct UserCreditItem {
    pub id: String,
    #[serde(rename = "userAddress")]
    pub user_address: String,
    pub amount: String,
    #[serde(rename = "availableAmount")]
    pub available_amount: String,
    pub status: String,
    pub contract: String,
    pub timestamp: String,
    pub signature: String,
    #[serde(rename = "seasonId")]
    pub season_id: i32,
    #[serde(rename = "goalId")]
    pub goal_id: String,
    #[serde(rename = "weekId")]
    pub week_id: i32,
    #[serde(rename = "claimedAt")]
    pub claimed_at: String,
    #[serde(rename = "expiresAt")]
    pub expires_at: String,
    #[serde(rename = "creditSource")]
    pub credit_source: String,
}

#[derive(Debug, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "credits/"))]
pub struct CreditsTotals {
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub expiring: i64,
    #[serde(rename = "nonExpiring")]
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub non_expiring: i64,
}

/// The USD block (upstream `get-user-credits.ts`): `balanceCents` is the
/// SPENDABLE balance (the live pledges already subtracted), `credits` its
/// floor, `held` the pledges themselves when there are any (what the Shop's
/// credits page shows as "on hold").
#[derive(Debug, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, export_to = "credits/"))]
pub struct UsdCredits {
    #[serde(rename = "balanceCents")]
    pub balance_cents: i64,
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub credits: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub held: Option<super::history::HeldOut>,
}
