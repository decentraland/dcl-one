// from: credits-server@origin/main:src/logic/workers/reminder-email-sender.ts, worker-manager.ts; notifications-workers@origin/main:processor/src/logic/event-parser.ts
//! Credits reminders: five scheduled notices per user, keyed per season (and
//! week) so reruns, restarts and lease handovers never send one twice.
use super::{NOTIFICATION_NAMESPACE, Season, SharedState, policy};
use crate::credits::errors::ApiError;
use crate::jobs::{Jobs, PeriodicCfg};
use alloy_primitives::U256;
use chrono::{NaiveDate, NaiveDateTime, TimeDelta, Timelike, Weekday};
use serde_json::{Value, json};
use sqlx::Row;
use std::{collections::HashMap, str::FromStr, sync::Arc, time::Duration};
use uuid::Uuid;

const PAGE: i64 = 500;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Kind {
    DoNotMissOut,
    CompleteGoals,
    Claim,
    Usage,
    Usage24Hours,
}

impl Kind {
    pub(crate) const ALL: [Kind; 5] = [
        Kind::DoNotMissOut,
        Kind::CompleteGoals,
        Kind::Claim,
        Kind::Usage,
        Kind::Usage24Hours,
    ];
    fn notification_type(self) -> &'static str {
        match self {
            Kind::DoNotMissOut => "credits_reminder_do_not_miss_out",
            Kind::CompleteGoals => "credits_reminder_complete_goals",
            Kind::Claim => "credits_reminder_claim_credits",
            Kind::Usage => "credits_reminder_usage",
            Kind::Usage24Hours => "credits_reminder_usage_24_hours",
        }
    }
    /// The source's event sub-type, kept in the dedup key.
    fn sub_type(self) -> &'static str {
        match self {
            Kind::DoNotMissOut => "do-not-miss-out-reminder",
            Kind::CompleteGoals => "complete-goals-reminder",
            Kind::Claim => "claim-credits-reminder",
            Kind::Usage => "usage-reminder",
            Kind::Usage24Hours => "usage-24-hours-reminder",
        }
    }
    /// Source schedule (UTC): Thursday 09, Friday 12, Sunday 09, daily 09, daily 00.
    fn schedule(self) -> (Option<Weekday>, u32) {
        match self {
            Kind::DoNotMissOut => (Some(Weekday::Thu), 9),
            Kind::CompleteGoals => (Some(Weekday::Fri), 12),
            Kind::Claim => (Some(Weekday::Sun), 9),
            Kind::Usage => (None, 9),
            Kind::Usage24Hours => (None, 0),
        }
    }
    pub(crate) fn due(self, now: NaiveDateTime) -> bool {
        use chrono::Datelike;
        let (day, hour) = self.schedule();
        day.is_none_or(|d| now.weekday() == d) && now.hour() >= hour
    }
    fn key(self, address: &str, season: i32, week: Option<i32>) -> String {
        match week {
            Some(week) => format!("{address}-{}-{season}-{week}", self.sub_type()),
            None => format!("{address}-{}-{season}", self.sub_type()),
        }
    }
}

/// The source's `getCurrentOrLastSeason`.
async fn current_or_last(
    pool: &sqlx::PgPool,
    now: NaiveDateTime,
) -> Result<Option<Season>, ApiError> {
    Ok(sqlx::query_as("SELECT id,name,start_date,end_date,max_mana_wei::text,grace_days FROM credits.seasons WHERE start_date <= $1 ORDER BY start_date DESC LIMIT 1")
        .bind(now).fetch_optional(pool).await?)
}

/// Wei as the source's `Number(formatEther(x))`: integral values carry no fraction.
pub(crate) fn ether_number(wei: &str) -> Result<Value, ApiError> {
    let v =
        U256::from_str(wei).map_err(|_| ApiError::Internal("invalid stored MANA wei".into()))?;
    let unit = U256::from(10u64).pow(U256::from(18u64));
    let frac = format!("{:018}", v % unit);
    let frac = frac.trim_end_matches('0');
    let text = if frac.is_empty() {
        (v / unit).to_string()
    } else {
        format!("{}.{frac}", v / unit)
    };
    serde_json::from_str(&text).map_err(|_| ApiError::Internal("invalid MANA presentation".into()))
}

fn links(shop_base_url: &str) -> (String, String) {
    let shop = shop_base_url.trim_end_matches('/').to_string();
    let origin = url::Url::parse(&shop)
        .map(|u| u.origin().ascii_serialization())
        .unwrap_or_else(|_| "https://decentraland.org".into());
    (format!("{origin}/play"), shop)
}

async fn send(
    state: &SharedState,
    kind: Kind,
    address: &str,
    key: &str,
    metadata: Value,
    now: NaiveDateTime,
) -> Result<bool, ApiError> {
    let mut conn = state.credits.pool.acquire().await?;
    // The source's dapps program switch and Explorer wallet allowlist.
    if policy(state, &mut conn, address).await?.is_some() {
        return Ok(false);
    }
    let notification = catalyst_social::notifications::NewNotification {
        id: Uuid::new_v5(&NOTIFICATION_NAMESPACE, key.as_bytes()),
        address: catalyst_social::Address::parse(address)
            .map_err(|_| ApiError::Internal("invalid reminder address".into()))?,
        kind: kind.notification_type().into(),
        occurred_at: now,
        metadata,
    };
    let n = catalyst_social::notifications::store::insert_keyed_notification(
        &mut *conn,
        key,
        &notification,
    )
    .await?;
    Ok(n > 0)
}

/// Weekly goal reminders over enrolled wallets. dcl-one only creates progress
/// rows on a counted photo, so a missing row reads as an untouched goal.
async fn weekly(
    state: &SharedState,
    kind: Kind,
    season: &Season,
    now: NaiveDateTime,
) -> Result<u64, ApiError> {
    let week = season.week(now).0;
    let play = links(&state.settings.shop_base_url).0;
    let mut after = String::new();
    let mut sent = 0;
    loop {
        let rows = if kind == Kind::Claim {
            sqlx::query("SELECT DISTINCT address FROM credits.goal_grants WHERE season_id=$1 AND week_id=$2 AND claimed_at IS NULL AND expires_at>$3 AND address>$4 ORDER BY address LIMIT $5")
                .bind(season.id).bind(week).bind(now).bind(&after).bind(PAGE).fetch_all(&state.credits.pool).await?
        } else {
            sqlx::query("SELECT w.address, count(*) FILTER (WHERE p.completed_at IS NOT NULL) AS completed, count(*) FILTER (WHERE p.completed_at IS NULL) AS incomplete, bool_or(COALESCE(p.completed_steps,0)>0) AS started, COALESCE(array_agg(d.goal_id ORDER BY d.position) FILTER (WHERE p.completed_at IS NULL),'{}') AS pending FROM credits.wallets w JOIN credits.season_goals d ON d.season_id=$1 LEFT JOIN credits.goal_progress p ON p.address=w.address AND p.season_id=d.season_id AND p.week_id=$2 AND p.goal_id=d.goal_id WHERE w.has_started_program AND NOT EXISTS(SELECT 1 FROM credits.program_opt_outs o WHERE o.address=w.address) AND (p.address IS NOT NULL OR $2 BETWEEN d.start_week AND d.end_week) AND w.address>$3 GROUP BY w.address ORDER BY w.address LIMIT $4")
                .bind(season.id).bind(week).bind(&after).bind(PAGE).fetch_all(&state.credits.pool).await?
        };
        let Some(last) = rows.last() else { break };
        after = last.get("address");
        let done = (rows.len() as i64) < PAGE;
        for row in rows {
            let address: String = row.get("address");
            let metadata = match kind {
                Kind::Claim => json!({"seasonId":season.id,"weekNumber":week,"link":play}),
                _ => {
                    let completed: i64 = row.get("completed");
                    let incomplete: i64 = row.get("incomplete");
                    let started =
                        completed > 0 || row.get::<Option<bool>, _>("started") == Some(true);
                    // One goal per week here, so "some goals done" also counts steps made.
                    match kind {
                        Kind::DoNotMissOut if !started => json!({"link":play}),
                        Kind::CompleteGoals if started && incomplete > 0 => {
                            json!({"seasonId":season.id,"weekNumber":week,"pendingGoalIds":row.get::<Vec<String>, _>("pending"),"link":play})
                        }
                        _ => continue,
                    }
                }
            };
            let key = kind.key(&address, season.id, Some(week));
            sent += u64::from(send(state, kind, &address, &key, metadata, now).await?);
        }
        if done {
            break;
        }
    }
    Ok(sent)
}

/// Claimed, unused, unexpired goal credits expiring within the window, for
/// wallets with progress in the season (the source's `user_progress` scope).
async fn usage(
    state: &SharedState,
    kind: Kind,
    season: &Season,
    now: NaiveDateTime,
) -> Result<u64, ApiError> {
    let window = if kind == Kind::Usage {
        TimeDelta::days(7)
    } else {
        TimeDelta::days(1)
    };
    let marketplace = links(&state.settings.shop_base_url).1;
    let mut after = String::new();
    let mut sent = 0;
    loop {
        let rows = sqlx::query("SELECT g.address, sum(g.amount_wei)::text AS amount, max(g.expires_at) AS expires FROM credits.goal_grants g WHERE g.claimed_at IS NOT NULL AND g.expires_at>$1 AND g.expires_at<=$2 AND NOT EXISTS(SELECT 1 FROM credits.usages u WHERE u.salt=g.salt AND u.chain_id=g.chain_id AND u.contract=g.contract AND u.credit_id=g.credit_hash AND u.sender=g.address) AND EXISTS(SELECT 1 FROM credits.goal_progress p WHERE p.address=g.address AND p.season_id=$3) AND g.address>$4 GROUP BY g.address ORDER BY g.address LIMIT $5")
            .bind(now).bind(now + window).bind(season.id).bind(&after).bind(PAGE).fetch_all(&state.credits.pool).await?;
        let Some(last) = rows.last() else { break };
        after = last.get("address");
        let done = (rows.len() as i64) < PAGE;
        for row in rows {
            let address: String = row.get("address");
            let expires: NaiveDateTime = row.get("expires");
            let balance = ether_number(&row.get::<String, _>("amount"))?;
            let date = expires.format("%B %-d").to_string();
            let metadata = if kind == Kind::Usage {
                json!({"expirationDate":date,"expirationDay":expires.format("%A").to_string(),"balance":balance,"link":marketplace})
            } else {
                json!({"expirationDate":date,"balance":balance,"link":marketplace})
            };
            let key = kind.key(&address, season.id, None);
            sent += u64::from(send(state, kind, &address, &key, metadata, now).await?);
        }
        if done {
            break;
        }
    }
    Ok(sent)
}

/// One reminder pass; weekly goal reminders stop once the season has ended.
pub(crate) async fn run(
    state: &SharedState,
    kind: Kind,
    now: NaiveDateTime,
) -> Result<u64, ApiError> {
    let Some(season) = current_or_last(&state.credits.pool, now).await? else {
        return Ok(0);
    };
    match kind {
        Kind::Usage | Kind::Usage24Hours => usage(state, kind, &season, now).await,
        _ if season.end_date <= now => Ok(0),
        _ => weekly(state, kind, &season, now).await,
    }
}

/// Each reminder runs once per UTC day at or after its source hour; the keys
/// make a rerun after a restart or lease handover a no-op.
pub fn spawn(state: &SharedState, jobs: &Jobs) {
    let state = Arc::clone(state);
    let ran: Arc<parking_lot::Mutex<HashMap<Kind, NaiveDate>>> = Arc::default();
    jobs.spawn_periodic_leased(
        "credits-reminders",
        PeriodicCfg::every(Duration::from_secs(5 * 60))
            .with_error_backoff(Duration::from_secs(30 * 60)),
        move |ctx| {
            let (state, ran) = (state.clone(), ran.clone());
            async move {
                if crate::shared::writes::ops_refusal(state.settings.writes_enabled).is_some() {
                    return Ok(());
                }
                let now = crate::credits::store::now();
                for kind in Kind::ALL {
                    if !kind.due(now) || ran.lock().get(&kind) == Some(&now.date()) {
                        continue;
                    }
                    let sent = run(&state, kind, now).await?;
                    ran.lock().insert(kind, now.date());
                    ctx.progress(format!("{} sent {sent}", kind.notification_type()));
                }
                Ok(())
            }
        },
    );
}

#[cfg(test)]
mod tests;
