use axum::{
    Json,
    extract::{Query, State},
};
use serde::Serialize;

use super::SharedState;
use super::wire::{ApiError, DataOnly, Params};

const DAY_MS: i64 = 86_400_000;
const MAX_DATE_MS: i64 = 8_640_000_000_000_000;
const MAX_SPAN_MS: i64 = 15 * 366 * DAY_MS;

#[derive(Serialize, sqlx::FromRow)]
pub(super) struct DailyRate {
    day: String,
    usd: String,
}

fn bound(params: &Params<'_>, key: &str) -> Result<i64, ApiError> {
    params
        .get_string(key, None)
        .filter(|value| value.bytes().all(|byte| byte.is_ascii_digit()))
        .and_then(|value| value.parse::<i64>().ok())
        .filter(|ms| (0..=MAX_DATE_MS).contains(ms))
        .ok_or_else(|| {
            ApiError::bad_request(format!("{key} must be an epoch timestamp in milliseconds"))
        })
}

/// A bounded timestamp's day since the epoch.
fn day_of(ms: i64) -> Result<i32, ApiError> {
    i32::try_from(ms / DAY_MS)
        .map_err(|_| ApiError::bad_request("an epoch timestamp in milliseconds is out of range"))
}

pub(super) async fn get_mana_usd_rates(
    State(state): State<SharedState>,
    Query(pairs): Query<Vec<(String, String)>>,
) -> Result<Json<DataOnly<Vec<DailyRate>>>, ApiError> {
    let params = Params::new(&pairs);
    let from = bound(&params, "from")?;
    let to = bound(&params, "to")?;
    if from > to {
        return Err(ApiError::bad_request(
            "from must be less than or equal to to",
        ));
    }
    if to - from > MAX_SPAN_MS {
        return Err(ApiError::bad_request("The range can span at most 15 years"));
    }
    let data = sqlx::query_as(
        "SELECT day::text AS day, usd::text AS usd FROM shop.mana_usd_daily
         WHERE day BETWEEN DATE '1970-01-01' + $1::integer AND DATE '1970-01-01' + $2::integer
           AND usd IS NOT NULL ORDER BY day",
    )
    .bind(day_of(from)?)
    .bind(day_of(to)?)
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(DataOnly { data }))
}
