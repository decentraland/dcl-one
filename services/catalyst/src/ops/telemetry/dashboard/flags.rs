//! `GET /dash/flags`: the upstream flags document (the local
//! `explore_api.feature_flags` mirror row of the explorer app, or
//! `TELEMETRY_FLAGS_URL` through the guarded client; the last good body
//! survives a failed refresh, 30 s memo) merged with the operator's
//! overrides and the caller's group targets, plus the flags observed in
//! Sentry events (last 30 days, 60 s memo) and the product areas.
//! `POST /dash/flag`: one override row with its audit row.

use axum::Json;
use axum::extract::{Query, State};
use axum::http::StatusCode;
use serde::Deserialize;
use serde_json::{Value, json};

use super::super::admin::{TelemetryAdmin, audited};
use super::super::{FLAGS_APP, SharedState, db_err, groups};

#[derive(Deserialize)]
pub struct FlagsQuery {
    pub user: Option<String>,
}

/// Upstream config is fetched at most once per TTL; a failing upstream serves the last good body (or null).
pub const CONFIG_TTL: std::time::Duration = std::time::Duration::from_secs(30);
pub const OBSERVED_TTL: std::time::Duration = std::time::Duration::from_secs(60);

const OBSERVED_SQL: &str = "SELECT f->>'flag' AS k, count(*) c FROM telemetry.events, \
           jsonb_array_elements(CASE WHEN jsonb_typeof(body->'contexts'->'flags'->'values')='array' \
             THEN body->'contexts'->'flags'->'values' ELSE '[]'::jsonb END) f \
         WHERE source='sentry' AND received_at > $1::timestamp - interval '30 days' \
           AND body->'contexts'->'flags'->'values' IS NOT NULL \
         GROUP BY 1 ORDER BY 2 DESC LIMIT 200";

fn decode<T: serde::de::DeserializeOwned>(v: Value) -> Result<T, (StatusCode, String)> {
    serde_json::from_value(v).map_err(|e| db_err("telemetry dashboard", sqlx::Error::decode(e)))
}

async fn upstream_config(st: &SharedState) -> anyhow::Result<Value> {
    match &st.settings.flags_url {
        Some(url) => {
            let resp = st
                .http
                .get(url)
                .timeout(std::time::Duration::from_secs(5))
                .send()
                .await?;
            let bytes = crate::http::read_body_capped(resp, 4 << 20).await?;
            Ok(serde_json::from_slice(&bytes)?)
        }
        None => {
            let doc: Option<Value> =
                sqlx::query_scalar("SELECT document FROM explore_api.feature_flags WHERE app = $1")
                    .bind(FLAGS_APP)
                    .fetch_optional(&st.pool)
                    .await?;
            doc.ok_or_else(|| anyhow::anyhow!("no explore_api.feature_flags row for {FLAGS_APP}"))
        }
    }
}

pub async fn flags(
    State(st): State<SharedState>,
    Query(p): Query<FlagsQuery>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let source_url = st
        .settings
        .flags_url
        .clone()
        .unwrap_or_else(|| format!("explore_api.feature_flags/{FLAGS_APP}"));
    let user_key = p.user.unwrap_or_default();

    let config = st.flags_config.get_or_stale(|| upstream_config(&st));
    let observed = st.flags_observed.get_or_refresh(false, || async {
        sqlx::query_as::<_, (Option<String>, i64)>(OBSERVED_SQL)
            .bind(super::now_utc())
            .fetch_all(&st.pool)
            .await
    });
    let fold = sqlx::query_as::<_, (Value, Value, Value, Value)>(sqlx::AssertSqlSafe(format!(
        "SELECT \
           (SELECT COALESCE(jsonb_agg(jsonb_build_array(flag, state, forced_variant) ORDER BY flag), '[]'::jsonb) \
              FROM telemetry.flag_overrides) AS overrides, \
           CASE WHEN $1::text = '' THEN '[]'::jsonb ELSE {groups_json} END AS groups, \
           (SELECT COALESCE(jsonb_agg(jsonb_build_array(group_name, flag, state, forced_variant)), '[]'::jsonb) \
              FROM telemetry.flag_group_targets WHERE $1::text <> '') AS targets, \
           (SELECT COALESCE(jsonb_agg(jsonb_build_array(name, area)), '[]'::jsonb) \
              FROM telemetry.product_areas WHERE kind = 'flag') AS areas",
        groups_json = groups::GROUPS_JSON,
    )))
    .bind(&user_key)
    .fetch_one(&st.pool);
    let (config, observed, fold) = tokio::join!(config, observed, fold);
    let config = config.unwrap_or(Value::Null);
    let observed = observed.map_err(|e| db_err("telemetry dashboard", e))?;
    let (overrides, groups, targets, areas) = fold.map_err(|e| db_err("telemetry dashboard", e))?;

    let mut overrides: Vec<FlagOverride> = decode(overrides)?;
    let groups = groups::groups_for(decode(groups)?, &user_key);
    let targeted = groups::rank_flag_targets(decode(targets)?, &groups);
    let areas: std::collections::HashMap<String, String> = decode::<Vec<(String, String)>>(areas)?
        .into_iter()
        .collect();
    for (flag, (state, variant)) in &targeted {
        match overrides.iter_mut().find(|(f, _, _)| f == flag) {
            Some(row) => *row = (flag.clone(), state.clone(), variant.clone()),
            None => overrides.push((flag.clone(), state.clone(), variant.clone())),
        }
    }
    let (overrides_json, merged) = merge_flags(&config, &overrides);
    Ok(Json(json!({
        "config": config,
        "flags": merged,
        "overrides": overrides_json,
        "observed": observed.into_iter().map(|(k,c)| json!([k.unwrap_or_default(), c])).collect::<Vec<_>>(),
        "source_url": source_url,
        "user": user_key,
        "groups": groups,
        "group_targeted": targeted.keys().collect::<Vec<_>>(),
        "areas": areas,
    })))
}

type FlagOverride = (String, String, Option<String>);

fn merge_flags(config: &Value, overrides: &[FlagOverride]) -> (Value, Value) {
    let up_flags = config
        .get("flags")
        .and_then(|x| x.as_object())
        .cloned()
        .unwrap_or_default();
    let up_variants = config
        .get("variants")
        .and_then(|x| x.as_object())
        .cloned()
        .unwrap_or_default();
    let ov: std::collections::HashMap<&str, (&str, Option<&str>)> = overrides
        .iter()
        .map(|(f, s, v)| (f.as_str(), (s.as_str(), v.as_deref())))
        .collect();

    let mut names: std::collections::BTreeSet<String> = up_flags.keys().cloned().collect();
    for (f, _, _) in overrides {
        names.insert(f.clone());
    }

    let mut merged = serde_json::Map::new();
    for name in &names {
        let key = name.as_str();
        let upstream_present = up_flags.contains_key(key);
        let upstream_value = up_flags.get(key).and_then(|x| x.as_bool());
        let upstream_variant = up_variants
            .get(key)
            .and_then(|x| x.get("name"))
            .and_then(|x| x.as_str())
            .map(|s| s.to_string());
        let (value, variant, overridden, override_state) = match ov.get(key).copied() {
            Some((state, forced_variant)) => {
                let (v, var) = match state {
                    "off" => (false, upstream_variant.clone()),
                    "forced" => (
                        true,
                        forced_variant
                            .map(|s| s.to_string())
                            .or_else(|| upstream_variant.clone()),
                    ),
                    _ => (true, upstream_variant.clone()),
                };
                (v, var, true, Some(state.to_string()))
            }
            None => (
                upstream_value.unwrap_or(false),
                upstream_variant.clone(),
                false,
                None,
            ),
        };
        let upstream_json = if upstream_present {
            json!(upstream_value.unwrap_or(false))
        } else {
            Value::Null
        };
        merged.insert(
            name.clone(),
            json!({
                "value": value,
                "variant": variant,
                "upstream_value": upstream_json,
                "overridden": overridden,
                "override_state": override_state,
            }),
        );
    }

    let overrides_json: serde_json::Map<String, Value> = overrides
        .iter()
        .map(|(f, s, v)| (f.clone(), json!({ "state": s, "variant": v })))
        .collect();
    (Value::Object(overrides_json), Value::Object(merged))
}

#[derive(Deserialize)]
pub struct FlagSetBody {
    flag: String,
    #[serde(default)]
    state: Option<String>,
    #[serde(default)]
    variant: Option<String>,
    #[serde(default)]
    clear: bool,
}

pub async fn flag_set(
    admin: TelemetryAdmin,
    State(st): State<SharedState>,
    Json(b): Json<FlagSetBody>,
) -> Result<Json<Value>, (StatusCode, String)> {
    if b.flag.is_empty() {
        return Err((StatusCode::BAD_REQUEST, "flag required".into()));
    }
    let state = b.state.clone().unwrap_or_else(|| "on".to_string());
    if !b.clear && !matches!(state.as_str(), "on" | "off" | "forced") {
        return Err((
            StatusCode::BAD_REQUEST,
            "state must be on|off|forced".into(),
        ));
    }
    let action = if b.clear { "flag.clear" } else { "flag.set" };
    let detail = json!({
        "flag": b.flag,
        "state": state,
        "variant": b.variant,
        "clear": b.clear,
    });
    if b.clear {
        sqlx::query(sqlx::AssertSqlSafe(audited(
            "DELETE FROM telemetry.flag_overrides WHERE flag = $1",
            1,
        )))
        .bind(&b.flag)
        .bind(admin.actor())
        .bind(action)
        .bind(&b.flag)
        .bind(detail)
        .execute(&st.pool)
        .await
        .map_err(|e| db_err("telemetry dashboard", e))?;
    } else {
        let variant = b.variant.as_deref().filter(|s| !s.is_empty());
        sqlx::query(sqlx::AssertSqlSafe(audited(
            "INSERT INTO telemetry.flag_overrides (flag, state, forced_variant, updated_at) \
             VALUES ($1, $2, $3, timezone('UTC', now())) \
             ON CONFLICT (flag) DO UPDATE SET \
               state = $2, forced_variant = $3, updated_at = timezone('UTC', now())",
            3,
        )))
        .bind(&b.flag)
        .bind(&state)
        .bind(variant)
        .bind(admin.actor())
        .bind(action)
        .bind(&b.flag)
        .bind(detail)
        .execute(&st.pool)
        .await
        .map_err(|e| db_err("telemetry dashboard", e))?;
    }
    Ok(Json(json!({ "ok": true, "flag": b.flag })))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ov(flag: &str, state: &str, variant: Option<&str>) -> FlagOverride {
        (flag.into(), state.into(), variant.map(|s| s.into()))
    }

    #[test]
    fn merge_forces_value_and_marks_override() {
        let config = json!({ "flags": { "a": false, "b": true } });
        let (overrides, merged) = merge_flags(&config, &[ov("a", "on", None)]);

        assert_eq!(merged["a"]["value"], json!(true));
        assert_eq!(merged["a"]["overridden"], json!(true));
        assert_eq!(merged["a"]["override_state"], json!("on"));
        assert_eq!(merged["a"]["upstream_value"], json!(false));

        assert_eq!(merged["b"]["value"], json!(true));
        assert_eq!(merged["b"]["overridden"], json!(false));

        assert_eq!(overrides["a"]["state"], json!("on"));
    }

    #[test]
    fn merge_off_forces_false() {
        let config = json!({ "flags": { "a": true } });
        let (_, merged) = merge_flags(&config, &[ov("a", "off", None)]);
        assert_eq!(merged["a"]["value"], json!(false));
        assert_eq!(merged["a"]["override_state"], json!("off"));
    }

    #[test]
    fn merge_forced_pins_variant() {
        let config = json!({ "flags": { "a": true }, "variants": { "a": { "name": "control" } } });
        let (_, merged) = merge_flags(&config, &[ov("a", "forced", Some("guided"))]);
        assert_eq!(merged["a"]["value"], json!(true));
        assert_eq!(merged["a"]["variant"], json!("guided"));
        assert_eq!(merged["a"]["override_state"], json!("forced"));
    }

    #[test]
    fn merge_surfaces_override_only_flag() {
        let config = json!({ "flags": { "a": true } });
        let (_, merged) = merge_flags(&config, &[ov("z", "on", None)]);
        assert!(merged.get("z").is_some());
        assert_eq!(merged["z"]["upstream_value"], Value::Null);
        assert_eq!(merged["z"]["overridden"], json!(true));
    }
}
