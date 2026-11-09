//! Materialize bounded, finite event recurrence rules. Keeping the accepted
//! dates makes read-time occurrence selection independent of a rollover job.
//! The RFC 5545 engine applies weekday/month masks and calendar positions.
//! Upstream (`Event/utils.ts:138-233`, `toRRule`/`futureRecurrentDates`)
//! kept only the next `MAX_EVENT_RECURRENT` dates and re-rolled them from a
//! cron; this node stores a series of at most [`MAX_OCCURRENCES`], anchored
//! on the future when the rule is longer, and the reads project upstream's
//! ten (`events::MAX_EVENT_RECURRENT`). `count` and `until` may coexist as
//! upstream accepted them (`toRRule`, `:152-166`): the series stops at
//! whichever comes first. A rule bounded by `until` can outlive the stored
//! window; [`run_once`], the pass [`spawn`] schedules, expands such a local
//! series again from the present before its stored window runs out (upstream's
//! `updateNextStartAt` cron, `cron.ts:23-48`).

use chrono::{DateTime, Duration, Timelike, Utc};
use serde_json::{Map, Value, json};
use sqlx::PgPool;

use super::json_date;
use super::moderation::{finish_at_of, parse_rfc3339};
use crate::shared::errors::ApiError;

const MAX_OCCURRENCES: u16 = 1_000;
/// Ended occurrences a re-anchored window keeps, for the metrics job.
const ENDED_OCCURRENCE_GRACE_HOURS: i64 = 24;

/// `MAX_RECURRENT_PAST_ITERATIONS` (`Event/types.ts:110`).
pub(super) const MAX_RECURRENT_PAST_ITERATIONS: f64 = 50_000.0;
/// `createEvent.ts:121`, `updateEvent.ts:285`.
pub(super) const TOO_MANY_PAST_ITERATIONS_MESSAGE: &str = "Recurrence rule expands to too many past occurrences from the start date; choose a later start date or a coarser frequency";

/// `FREQUENCY_PERIOD_MS` (`Event/types.ts:115-123`), the allowed input
/// frequencies only (`AllowedInputFrequencies`, `:27-33`).
fn period_ms(frequency: &str) -> Option<f64> {
    Some(match frequency {
        "HOURLY" => 60.0 * 60.0 * 1_000.0,
        "DAILY" => 24.0 * 60.0 * 60.0 * 1_000.0,
        "WEEKLY" => 7.0 * 24.0 * 60.0 * 60.0 * 1_000.0,
        "MONTHLY" => 30.0 * 24.0 * 60.0 * 60.0 * 1_000.0,
        "YEARLY" => 365.0 * 24.0 * 60.0 * 60.0 * 1_000.0,
        _ => return None,
    })
}

fn number(raw: &Map<String, Value>, key: &str) -> Option<i64> {
    raw.get(key).and_then(Value::as_i64)
}

/// `estimateRecurrentPastIterations` (`Event/utils.ts:256-292`): the upper
/// bound of the past steps an expansion from `start_at` to `now` walks;
/// `0` when the rule would not iterate.
pub(super) fn estimate_past_iterations(raw: &Map<String, Value>, now: DateTime<Utc>) -> f64 {
    let recurrent = raw
        .get("recurrent")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let Some(frequency) = raw.get("recurrent_frequency").and_then(Value::as_str) else {
        return 0.0;
    };
    let count = number(raw, "recurrent_count").filter(|c| *c != 0);
    let until = raw
        .get("recurrent_until")
        .and_then(Value::as_str)
        .and_then(parse_rfc3339);
    if !recurrent || (count.is_none() && until.is_none()) {
        return 0.0;
    }
    let Some(start) = raw
        .get("start_at")
        .and_then(Value::as_str)
        .and_then(parse_rfc3339)
    else {
        return 0.0;
    };
    let Some(period) = period_ms(frequency) else {
        return 0.0;
    };
    let interval = number(raw, "recurrent_interval")
        .filter(|i| *i != 0)
        .unwrap_or(1) as f64;
    let bound = match until {
        Some(until) => now.min(until),
        None => now,
    };
    let span = (bound.timestamp_millis() - start.timestamp_millis()).max(0) as f64;
    let by_time = span / (period * interval);
    let by_count = count.map(|c| c as f64).unwrap_or(f64::INFINITY);
    by_time.min(by_count)
}

pub(super) fn recompute(raw: &mut Map<String, Value>, now: DateTime<Utc>) -> Result<(), ApiError> {
    let start = raw
        .get("start_at")
        .and_then(Value::as_str)
        .and_then(parse_rfc3339)
        .ok_or_else(|| ApiError::bad_request("invalid start_at"))?;
    let duration = number(raw, "duration").unwrap_or(0);
    if duration < 0 {
        return Err(ApiError::bad_request("duration must be positive"));
    }
    let frequency = raw.get("recurrent_frequency").and_then(Value::as_str);
    if frequency.is_some_and(|f| !matches!(f, "YEARLY" | "MONTHLY" | "WEEKLY" | "DAILY" | "HOURLY"))
    {
        return Err(ApiError::bad_request("unsupported recurrent_frequency"));
    }
    let interval = number(raw, "recurrent_interval").unwrap_or(1);
    let count = number(raw, "recurrent_count").filter(|c| *c != 0);
    let until = match raw.get("recurrent_until").and_then(Value::as_str) {
        Some(until) => Some(
            parse_rfc3339(until).ok_or_else(|| ApiError::bad_request("invalid recurrent_until"))?,
        ),
        None => None,
    };
    if !(1..=1000).contains(&interval) || count.is_some_and(|c| !(1..=1000).contains(&c)) {
        return Err(ApiError::bad_request(
            "recurrence interval/count out of range",
        ));
    }
    let weekdays = number(raw, "recurrent_weekday_mask").unwrap_or(0);
    let months = number(raw, "recurrent_month_mask").unwrap_or(0);
    if !(0..=127).contains(&weekdays) || !(0..=4095).contains(&months) {
        return Err(ApiError::bad_request("recurrence mask out of range"));
    }
    let recurrent = raw
        .get("recurrent")
        .and_then(Value::as_bool)
        .unwrap_or(false)
        && frequency.is_some()
        && (count.is_some() || until.is_some());
    let mut dates = vec![start];
    // `futureRecurrentDates` (`utils.ts:204-211`): a rule that already ended
    // expands to nothing and the series falls back to the start.
    let ended = until.is_some_and(|until| until < now || until < start);
    if recurrent && !ended {
        let mut rule = format!(
            "DTSTART:{}\nRRULE:FREQ={};INTERVAL={interval}",
            start.format("%Y%m%dT%H%M%SZ"),
            frequency.unwrap_or("DAILY")
        );
        // `until` bounds the rule; a `count` beside it truncates the result
        // below (RFC 5545 forbids both in one RRULE; rrule.js took both).
        match (count, until) {
            (_, Some(until)) => {
                rule.push_str(&format!(";UNTIL={}", until.format("%Y%m%dT%H%M%SZ")))
            }
            (Some(count), None) => rule.push_str(&format!(";COUNT={count}")),
            (None, None) => {}
        }
        if weekdays != 0 {
            let days = ["SU", "MO", "TU", "WE", "TH", "FR", "SA"]
                .iter()
                .enumerate()
                .filter_map(|(i, day)| (weekdays & (1 << i) != 0).then_some(*day))
                .collect::<Vec<_>>();
            rule.push_str(&format!(";BYDAY={}", days.join(",")));
        }
        if months != 0 {
            let selected = (0..12)
                .filter(|i| months & (1 << i) != 0)
                .map(|i| (i + 1).to_string())
                .collect::<Vec<_>>();
            rule.push_str(&format!(";BYMONTH={}", selected.join(",")));
        }
        for (field, parameter, max) in [
            ("recurrent_setpos", "BYSETPOS", 366),
            ("recurrent_monthday", "BYMONTHDAY", 31),
        ] {
            if let Some(value) = number(raw, field).filter(|v| *v != 0) {
                if !(-max..=max).contains(&value) {
                    return Err(ApiError::bad_request("recurrence position out of range"));
                }
                rule.push_str(&format!(";{parameter}={value}"));
            }
        }
        let set = rule
            .parse::<rrule::RRuleSet>()
            .map_err(|_| ApiError::bad_request("invalid recurrence rule"))?;
        let budget = count
            .map(|c| (c as u16).min(MAX_OCCURRENCES))
            .unwrap_or(MAX_OCCURRENCES);
        let mut result = set.clone().all(budget);
        if result.limited && count.is_none() {
            // Longer than the stored window: keep the occurrences from now on
            // (`futureRecurrentDates` kept only future ones) so the series
            // does not end before it reaches the present. An occurrence under
            // way (started less than `duration` ago) stays in the series, and
            // so do the occurrences of the last day: the reads project only
            // the unfinished ones, and the community metrics job
            // (`social/notifications/event_ended.rs`) still finds an
            // occurrence that ended since its last pass.
            let anchor = now
                - Duration::milliseconds(duration)
                - Duration::hours(ENDED_OCCURRENCE_GRACE_HOURS);
            let future = set.after(anchor.with_timezone(&rrule::Tz::UTC)).all(budget);
            if !future.dates.is_empty() {
                result = future;
            }
        }
        if !result.dates.is_empty() {
            dates = result
                .dates
                .into_iter()
                .map(|d| {
                    d.with_timezone(&Utc) + Duration::nanoseconds(i64::from(start.nanosecond()))
                })
                .collect();
        }
    }
    let finish = finish_at_of(*dates.last().unwrap_or(&start), duration)
        .ok_or_else(|| ApiError::bad_request("event finish out of range"))?;
    let next = dates
        .iter()
        .find(|d| finish_at_of(**d, duration).is_some_and(|f| f > now))
        .copied()
        .unwrap_or_else(|| *dates.last().unwrap_or(&start));
    raw.insert("recurrent".into(), json!(recurrent));
    // Stored as upstream's rows serialised (`Date#toJSON`, `.000Z`): the
    // form the mirror holds and the federation feed passes on.
    raw.insert(
        "recurrent_dates".into(),
        json!(dates.iter().map(json_date::wire).collect::<Vec<_>>()),
    );
    raw.insert("finish_at".into(), json!(json_date::wire(&finish)));
    raw.insert("next_start_at".into(), json!(json_date::wire(&next)));
    raw.insert(
        "next_finish_at".into(),
        json!(finish_at_of(next, duration).as_ref().map(json_date::wire)),
    );
    Ok(())
}

/// A stored window that ends within this many days is expanded again.
const REROLL_HORIZON_DAYS: i64 = 7;
/// Rows one pass expands at most; the next pass takes the rest.
const REROLL_BATCH: i64 = 500;

/// Expands again the local series whose stored window ends before the rule's
/// `until` and within [`REROLL_HORIZON_DAYS`]: a rule longer than
/// [`MAX_OCCURRENCES`] is stored from the present on, so its next occurrences
/// are materialized before the stored ones run out. A rule bounded by `count`
/// never exceeds the window, a rule whose `until` passed has nothing left to
/// expand (`getRecurrentFinishedEvents` reads live series only), and a
/// mirrored row is the mirror's to refresh. A rule that would walk more than
/// [`MAX_RECURRENT_PAST_ITERATIONS`] steps from `start_at` to the present is
/// left as stored (`cron.ts:27-38`). Answers the number of rows changed.
pub(crate) async fn run_once(pool: &PgPool, now: DateTime<Utc>) -> anyhow::Result<usize> {
    let mut changed = 0;
    for id in due_series(pool, now).await? {
        if reroll(pool, &id, now).await? {
            changed += 1;
        }
    }
    Ok(changed)
}

/// The ids of the series [`run_once`] expands, the oldest window first; the
/// next pass takes the rest. Only a window holding [`MAX_OCCURRENCES`] dates
/// is truncated: a shorter one already reaches the rule's end, so it is not
/// read again and cannot fill the batch.
async fn due_series(pool: &PgPool, now: DateTime<Utc>) -> anyhow::Result<Vec<String>> {
    let until = format!(
        "{}(raw->>'recurrent_until')",
        crate::schema_functions::UTC_TIMESTAMP.name
    );
    Ok(sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
        "SELECT id FROM explore.event \
         WHERE origin = 'local' AND recurrent AND NOT rejected AND NOT deleted \
           AND (jsonb_typeof(raw->'recurrent_count') IS DISTINCT FROM 'number' OR (raw->>'recurrent_count')::float8 = 0) \
           AND finish_at < $1::timestamptz AT TIME ZONE 'UTC' AND {until} > finish_at \
           AND {until} > $3::timestamptz AT TIME ZONE 'UTC' \
           AND jsonb_typeof(raw->'recurrent_dates') = 'array' AND jsonb_array_length(raw->'recurrent_dates') >= $4 \
         ORDER BY finish_at LIMIT $2"
    )))
    .bind(now + Duration::days(REROLL_HORIZON_DAYS))
    .bind(REROLL_BATCH)
    .bind(now)
    .bind(i32::from(MAX_OCCURRENCES))
    .fetch_all(pool)
    .await?)
}

/// Expands one series again under its row lock: the row is read afresh and
/// held until the write commits, so a moderation write (approve, reject,
/// delete: `raw` changes, the series does not) lands before or after this
/// one, never under it, and a row rejected or deleted since the batch was
/// read is left alone. Answers whether the row changed.
async fn reroll(pool: &PgPool, id: &str, now: DateTime<Utc>) -> anyhow::Result<bool> {
    let mut tx = pool.begin().await?;
    let raw: Option<Value> = sqlx::query_scalar(
        "SELECT raw FROM explore.event \
         WHERE id = $1 AND origin = 'local' AND NOT rejected AND NOT deleted FOR UPDATE",
    )
    .bind(id)
    .fetch_optional(&mut *tx)
    .await?;
    let Some(Value::Object(mut raw)) = raw else {
        return Ok(false);
    };
    if estimate_past_iterations(&raw, now) > MAX_RECURRENT_PAST_ITERATIONS {
        tracing::warn!(event = %id, "recurrence rule walks too many past steps; series kept as stored");
        return Ok(false);
    }
    let before = raw.get("recurrent_dates").cloned().unwrap_or(Value::Null);
    if recompute(&mut raw, now).is_err() {
        tracing::warn!(event = %id, "stored recurrence rule does not expand; series kept as stored");
        return Ok(false);
    }
    if raw.get("recurrent_dates") == Some(&before) {
        return Ok(false);
    }
    let text = |key: &str| raw.get(key).and_then(Value::as_str).map(str::to_owned);
    let (finish_at, next_start_at, next_finish_at) = (
        text("finish_at"),
        text("next_start_at"),
        text("next_finish_at"),
    );
    sqlx::query(
        "UPDATE explore.event SET raw = $2, finish_at = $3::timestamptz AT TIME ZONE 'UTC', \
             next_start_at = $4::timestamptz AT TIME ZONE 'UTC', next_finish_at = $5::timestamptz AT TIME ZONE 'UTC' \
         WHERE id = $1",
    )
    .bind(id)
    .bind(Value::Object(raw))
    .bind(finish_at)
    .bind(next_start_at)
    .bind(next_finish_at)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(true)
}

/// Runs [`run_once`] every hour (the first pass jittered), waiting longer
/// after a failed pass.
pub(crate) fn spawn(pool: PgPool, jobs: &crate::jobs::Jobs) {
    let cfg = crate::jobs::PeriodicCfg::every(std::time::Duration::from_secs(3600))
        .with_jitter(std::time::Duration::from_secs(30))
        .with_error_backoff(std::time::Duration::from_secs(3600));
    jobs.spawn_periodic_leased("explore-event-recurrence", cfg, move |_ctx| {
        let pool = pool.clone();
        async move {
            run_once(&pool, Utc::now()).await?;
            Ok(())
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn event() -> Map<String, Value> {
        json!({"start_at":"2099-06-01T18:00:00.123Z", "duration":3600000, "recurrent":true, "recurrent_frequency":"WEEKLY", "recurrent_count":3})
            .as_object().unwrap().clone()
    }

    #[test]
    fn weekly_series_preserves_future_occurrences_and_precision() {
        let mut raw = event();
        recompute(&mut raw, parse_rfc3339("2099-06-02T00:00:00Z").unwrap()).unwrap();
        let dates = raw["recurrent_dates"].as_array().unwrap();
        assert_eq!(dates.len(), 3);
        assert_eq!(dates[1], "2099-06-08T18:00:00.123Z");
        assert_eq!(raw["next_start_at"], dates[1]);
        assert_eq!(raw["finish_at"], "2099-06-15T19:00:00.123Z");
    }

    /// The re-anchored window keeps the last day of ended occurrences: the
    /// reads project only the unfinished ones, and the community metrics job
    /// still finds an occurrence that ended since its last pass.
    #[test]
    fn re_anchoring_keeps_the_last_day_of_ended_occurrences() {
        let mut raw = hourly_year();
        recompute(&mut raw, parse_rfc3339("2099-07-10T12:30:00Z").unwrap()).unwrap();
        let dates = raw["recurrent_dates"].as_array().unwrap();
        assert_eq!(dates.len(), usize::from(MAX_OCCURRENCES));
        assert_eq!(dates[0], "2099-07-09T12:00:00.000Z");
        assert_eq!(raw["next_start_at"], "2099-07-10T12:00:00.000Z");
    }

    #[test]
    fn monthly_last_weekday_and_month_masks_use_calendar_rules() {
        let mut raw = event();
        raw.extend(json!({"recurrent_frequency":"MONTHLY", "recurrent_weekday_mask":62, "recurrent_setpos":-1, "recurrent_month_mask":160}).as_object().unwrap().clone());
        recompute(&mut raw, Utc::now()).unwrap();
        assert_eq!(raw["recurrent_dates"][0], "2099-06-30T18:00:00.123Z");
        assert_eq!(raw["recurrent_dates"][1], "2099-08-31T18:00:00.123Z");
    }

    /// `toRRule` (`Event/utils.ts:152-166`) passes both `until` and `count`;
    /// the series stops at whichever comes first. Fixture
    /// `events.create.recurrent_success_dates_capped` keeps `count`.
    #[test]
    fn count_and_until_coexist_and_the_earlier_bound_wins() {
        let mut raw = event();
        raw.insert("recurrent_until".into(), json!("2099-06-10T18:00:00Z"));
        recompute(&mut raw, Utc::now()).unwrap();
        assert_eq!(raw["recurrent_dates"].as_array().unwrap().len(), 2);
        raw.insert("recurrent_until".into(), json!("2199-06-10T18:00:00Z"));
        recompute(&mut raw, Utc::now()).unwrap();
        assert_eq!(raw["recurrent_dates"].as_array().unwrap().len(), 3);
    }

    #[test]
    fn invalid_frequencies_are_rejected_and_long_rules_are_bounded() {
        let mut raw = event();
        raw.insert("recurrent_frequency".into(), json!("SECONDLY"));
        assert!(recompute(&mut raw, Utc::now()).is_err());
        raw.insert("recurrent_frequency".into(), json!("HOURLY"));
        raw.remove("recurrent_count");
        raw.insert("recurrent_until".into(), json!("2199-06-01T18:00:00Z"));
        recompute(&mut raw, Utc::now()).unwrap();
        assert_eq!(raw["recurrent_dates"].as_array().unwrap().len(), 1000);
    }

    /// A rule longer than the stored window is anchored on the present once
    /// its start is behind us, so the stored series still reaches the future.
    #[test]
    fn a_long_series_from_the_past_keeps_its_future_occurrences() {
        let mut raw = json!({"start_at":"2020-01-01T18:00:00Z", "duration":3600000, "recurrent":true, "recurrent_frequency":"DAILY", "recurrent_until":"2040-01-01T18:00:00Z"})
            .as_object().unwrap().clone();
        let now = parse_rfc3339("2030-06-01T00:00:00Z").unwrap();
        recompute(&mut raw, now).unwrap();
        let dates = raw["recurrent_dates"].as_array().unwrap();
        assert_eq!(dates.len(), 1000);
        assert_eq!(dates[0], "2030-05-31T18:00:00.000Z", "the last day kept");
        assert_eq!(raw["next_start_at"], dates[1]);
        let steps = estimate_past_iterations(&raw, now);
        assert!(
            (3790.0..3810.0).contains(&steps),
            "about ten and a half years of daily steps: {steps}"
        );
    }

    fn hourly_year() -> Map<String, Value> {
        json!({"start_at":"2099-06-01T18:00:00Z", "duration":3600000, "recurrent":true, "recurrent_frequency":"HOURLY", "recurrent_until":"2100-06-01T18:00:00Z"})
            .as_object().unwrap().clone()
    }

    /// An hourly rule over one year has 8761 occurrences. Written ahead of its
    /// start the series holds the first `MAX_OCCURRENCES`; expanded again from
    /// inside the rule it holds the next ones from the present, the occurrence
    /// under way included, and near the end the remaining ones.
    #[test]
    fn an_hourly_year_long_rule_is_stored_from_the_present_on() {
        let mut raw = hourly_year();
        recompute(&mut raw, parse_rfc3339("2099-01-01T00:00:00Z").unwrap()).unwrap();
        let dates = raw["recurrent_dates"].as_array().unwrap();
        assert_eq!(dates.len(), 1000);
        assert_eq!(dates[0], "2099-06-01T18:00:00.000Z");
        assert_eq!(raw["finish_at"], "2099-07-13T10:00:00.000Z");
        recompute(&mut raw, parse_rfc3339("2099-08-01T18:30:00Z").unwrap()).unwrap();
        let dates = raw["recurrent_dates"].as_array().unwrap();
        assert_eq!(dates.len(), 1000);
        assert_eq!(dates[0], "2099-07-31T18:00:00.000Z", "the last day kept");
        assert_eq!(raw["next_start_at"], "2099-08-01T18:00:00.000Z");
        assert_eq!(raw["next_finish_at"], "2099-08-01T19:00:00.000Z");
        assert_eq!(raw["finish_at"], "2099-09-11T10:00:00.000Z");
        recompute(&mut raw, parse_rfc3339("2100-05-31T00:00:00Z").unwrap()).unwrap();
        let dates = raw["recurrent_dates"].as_array().unwrap();
        assert!((67..=68).contains(&dates.len()), "{}", dates.len());
        assert_eq!(dates.last().unwrap(), "2100-06-01T18:00:00.000Z");
        assert_eq!(raw["finish_at"], "2100-06-01T19:00:00.000Z");
    }

    async fn insert(pool: &PgPool, id: &str, origin: &str, raw: &Map<String, Value>) {
        let text = |key: &str| raw.get(key).and_then(Value::as_str).map(str::to_owned);
        sqlx::query(
            "INSERT INTO explore.event (id, name, start_at, finish_at, next_start_at, next_finish_at, duration_ms, recurrent, approved, raw, origin) \
             VALUES ($1, 'series', $2::timestamptz AT TIME ZONE 'UTC', $3::timestamptz AT TIME ZONE 'UTC', $4::timestamptz AT TIME ZONE 'UTC', \
                     $5::timestamptz AT TIME ZONE 'UTC', $6, $7, true, $8, $9)",
        )
        .bind(id)
        .bind(text("start_at"))
        .bind(text("finish_at"))
        .bind(text("next_start_at"))
        .bind(text("next_finish_at"))
        .bind(number(raw, "duration"))
        .bind(raw.get("recurrent").and_then(Value::as_bool).unwrap_or(false))
        .bind(Value::Object(raw.clone()))
        .bind(origin)
        .execute(pool)
        .await
        .unwrap();
    }
    async fn stored(pool: &PgPool, id: &str) -> (String, String, String) {
        sqlx::query_as(
            "SELECT raw->'recurrent_dates'->>0, next_start_at::text, finish_at::text FROM explore.event WHERE id = $1",
        )
        .bind(id)
        .fetch_one(pool)
        .await
        .unwrap()
    }
    /// The pass expands a local series whose window ends within the horizon;
    /// a mirrored row, a counted rule, a series that already reaches its
    /// `until` and one whose `until` passed stay as stored, and a second pass
    /// changes nothing.
    #[tokio::test]
    async fn run_once_expands_the_truncated_local_series_again() {
        let Some(pool) = crate::db::scratch::isolated_pool(2).await else {
            return;
        };
        let written = parse_rfc3339("2099-01-01T00:00:00Z").unwrap();
        let run = uuid::Uuid::new_v4().simple().to_string();
        let id = |name: &str| format!("recurrence-{run}-{name}");
        let mut long = hourly_year();
        recompute(&mut long, written).unwrap();
        insert(&pool, &id("local"), "local", &long).await;
        insert(&pool, &id("mirrored"), "upstream", &long).await;
        let mut counted = event();
        recompute(&mut counted, written).unwrap();
        insert(&pool, &id("counted"), "local", &counted).await;
        let mut short = json!({"start_at":"2099-06-01T18:00:00Z", "duration":3600000, "recurrent":true, "recurrent_frequency":"DAILY", "recurrent_until":"2099-06-11T18:00:00Z"})
            .as_object().unwrap().clone();
        recompute(&mut short, written).unwrap();
        assert_eq!(short["recurrent_dates"].as_array().unwrap().len(), 11);
        insert(&pool, &id("short"), "local", &short).await;
        // Ended a month before `now`: `until` passed, nothing is left to expand.
        let mut ended = json!({"start_at":"2099-06-01T18:00:00Z", "duration":3600000, "recurrent":true, "recurrent_frequency":"DAILY", "recurrent_until":"2099-06-11T23:59:00Z"})
            .as_object().unwrap().clone();
        recompute(&mut ended, written).unwrap();
        assert_eq!(ended["recurrent_dates"].as_array().unwrap().len(), 11);
        insert(&pool, &id("ended"), "local", &ended).await;
        let mut count_until = event();
        count_until.insert("recurrent_until".into(), json!("2199-06-10T18:00:00Z"));
        recompute(&mut count_until, written).unwrap();
        insert(&pool, &id("count_until"), "local", &count_until).await;
        // Complete: the last occurrence (Jul 13) ends within the horizon and
        // before `until` (Jul 19), but the seven stored dates are the whole
        // series, so there is nothing to expand on any pass.
        let mut complete = json!({"start_at":"2099-06-01T18:00:00Z", "duration":3600000, "recurrent":true, "recurrent_frequency":"WEEKLY", "recurrent_until":"2099-07-19T18:00:00Z"})
            .as_object().unwrap().clone();
        recompute(&mut complete, written).unwrap();
        assert_eq!(complete["recurrent_dates"].as_array().unwrap().len(), 7);
        insert(&pool, &id("complete"), "local", &complete).await;

        let now = parse_rfc3339("2099-07-10T12:30:00Z").unwrap();
        let due: Vec<String> = due_series(&pool, now)
            .await
            .unwrap()
            .into_iter()
            .filter(|i| i.contains(&run))
            .collect();
        assert_eq!(
            due,
            vec![id("local")],
            "only the truncated local series is due"
        );
        assert_eq!(run_once(&pool, now).await.unwrap(), 1);
        assert_eq!(
            stored(&pool, &id("local")).await,
            (
                "2099-07-09T12:00:00.000Z".to_string(),
                "2099-07-10 12:00:00".to_string(),
                "2099-08-20 04:00:00".to_string()
            )
        );
        assert_eq!(
            run_once(&pool, now).await.unwrap(),
            0,
            "a second pass changes nothing"
        );
        let kept: i32 = sqlx::query_scalar(
            "SELECT jsonb_array_length(raw->'recurrent_dates') FROM explore.event WHERE id = $1",
        )
        .bind(id("ended"))
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(kept, 11, "an ended series keeps its stored occurrences");
        for (name, raw) in [
            ("mirrored", &long),
            ("counted", &counted),
            ("short", &short),
            ("ended", &ended),
            ("count_until", &count_until),
            ("complete", &complete),
        ] {
            let first = raw["recurrent_dates"][0].as_str().unwrap();
            assert_eq!(
                stored(&pool, &id(name)).await.0,
                first,
                "{name} kept as stored"
            );
        }
    }

    /// A moderation write that lands between the batch read and the row's
    /// expansion wins: the row is read again under its lock, so a rejection
    /// (an approval, a deletion) is never written over.
    #[tokio::test]
    async fn a_moderation_write_between_the_batch_read_and_the_reroll_wins() {
        let Some(pool) = crate::db::scratch::isolated_pool(2).await else {
            return;
        };
        let written = parse_rfc3339("2099-01-01T00:00:00Z").unwrap();
        let id = format!("recurrence-{}-raced", uuid::Uuid::new_v4().simple());
        let mut long = hourly_year();
        recompute(&mut long, written).unwrap();
        insert(&pool, &id, "local", &long).await;
        let now = parse_rfc3339("2099-07-10T12:30:00Z").unwrap();
        assert!(due_series(&pool, now).await.unwrap().contains(&id));
        sqlx::query("UPDATE explore.event SET raw = raw || '{\"rejected\": true}' WHERE id = $1")
            .bind(&id)
            .execute(&pool)
            .await
            .unwrap();
        assert!(
            !reroll(&pool, &id, now).await.unwrap(),
            "the row rejected meanwhile is left alone"
        );
        let (first, rejected): (String, bool) = sqlx::query_as(
            "SELECT raw->'recurrent_dates'->>0, rejected FROM explore.event WHERE id = $1",
        )
        .bind(&id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(
            (first.as_str(), rejected),
            ("2099-06-01T18:00:00.000Z", true)
        );
    }

    /// `estimateRecurrentPastIterations` (`Event/utils.ts:256-292`): the
    /// fixture `events.create.recurrence_too_many_past_iterations` rule.
    #[test]
    fn past_iteration_estimate_follows_upstream() {
        let raw = json!({"start_at":"2000-01-01T00:00:00Z", "recurrent":true, "recurrent_frequency":"HOURLY", "recurrent_until":"2040-01-01T00:00:00Z"})
            .as_object().unwrap().clone();
        let now = parse_rfc3339("2026-11-05T00:00:00Z").unwrap();
        assert!(estimate_past_iterations(&raw, now) > MAX_RECURRENT_PAST_ITERATIONS);
        let mut counted = raw.clone();
        counted.insert("recurrent_count".into(), json!(100));
        assert_eq!(estimate_past_iterations(&counted, now), 100.0);
        let mut off = raw.clone();
        off.insert("recurrent".into(), json!(false));
        assert_eq!(estimate_past_iterations(&off, now), 0.0);
        let mut unterminated = raw.clone();
        unterminated.remove("recurrent_until");
        assert_eq!(estimate_past_iterations(&unterminated, now), 0.0);
    }
}
