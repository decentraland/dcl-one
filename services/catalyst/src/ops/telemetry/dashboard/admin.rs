//! The console's own writes: `POST /dash/issue/state` (per-fingerprint
//! workflow state, audited) and `GET`/`POST /dash/experiment` (the
//! per-experiment override, resolved group target > override > empty).
//! `POST /dash/sql` moved to [`super::super::sql`], on the reader's own
//! login.

use axum::Json;
use axum::extract::{Query, State};
use axum::http::StatusCode;
use serde::Deserialize;
use serde_json::{Value, json};

use super::super::admin::{TelemetryAdmin, audited};
use super::super::{SharedState, db_err, groups};

fn deserialize_some<'de, T, D>(d: D) -> Result<Option<T>, D::Error>
where
    T: Deserialize<'de>,
    D: serde::Deserializer<'de>,
{
    T::deserialize(d).map(Some)
}

#[derive(Deserialize)]
pub struct IssueStateBody {
    fingerprint: String,
    status: Option<String>,
    /// Absent = keep; `null` = clear; a string = assign.
    #[serde(default, deserialize_with = "deserialize_some")]
    assignee: Option<Option<String>>,
    note: Option<String>,
}

pub async fn set_issue_state(
    admin: TelemetryAdmin,
    State(st): State<SharedState>,
    Json(b): Json<IssueStateBody>,
) -> Result<Json<Value>, (StatusCode, String)> {
    if b.fingerprint.is_empty() {
        return Err((StatusCode::BAD_REQUEST, "fingerprint required".into()));
    }
    if let Some(s) = b.status.as_deref()
        && !matches!(s, "unresolved" | "resolved" | "ignored")
    {
        return Err((
            StatusCode::BAD_REQUEST,
            "status must be unresolved|resolved|ignored".into(),
        ));
    }

    let assignee_present = b.assignee.is_some();
    let assignee_val = b.assignee.flatten();

    let row = sqlx::query_as::<_, (String, Option<String>)>(
        "WITH w AS ( \
           INSERT INTO telemetry.issue_state (fingerprint, status, assignee, note, updated_at) \
           VALUES ($1, COALESCE($2, 'unresolved'), $3, $5, timezone('UTC', now())) \
           ON CONFLICT (fingerprint) DO UPDATE SET \
             status = COALESCE($2, telemetry.issue_state.status), \
             assignee = CASE WHEN $4 THEN $3 ELSE telemetry.issue_state.assignee END, \
             note = COALESCE($5, telemetry.issue_state.note), \
             updated_at = timezone('UTC', now()) \
           RETURNING status, assignee), \
         a AS (INSERT INTO ops.admin_audit (actor, domain, action, target, detail) \
               VALUES ($6, 'telemetry', 'issue.state', $1, $7)) \
         SELECT status, assignee FROM w",
    )
    .bind(&b.fingerprint)
    .bind(&b.status)
    .bind(&assignee_val)
    .bind(assignee_present)
    .bind(&b.note)
    .bind(admin.actor())
    .bind(json!({ "fingerprint": b.fingerprint, "status": b.status, "assignee": assignee_val, "assignee_set": assignee_present, "note": b.note }))
    .fetch_one(&st.pool)
    .await
    .map_err(|e| db_err("telemetry dashboard", e))?;
    Ok(Json(json!({
        "ok": true,
        "fingerprint": b.fingerprint,
        "status": row.0,
        "assignee": row.1,
    })))
}

#[derive(Deserialize)]
pub struct ExperimentsQuery {
    pub key: Option<String>,
    #[serde(default)]
    pub user: Option<String>,
}

pub async fn experiments_get(
    State(st): State<SharedState>,
    Query(p): Query<ExperimentsQuery>,
) -> Result<Json<Value>, (StatusCode, String)> {
    if let Some(key) = p.key.filter(|v| !v.is_empty()) {
        let user_key = p.user.unwrap_or_default();
        let (groups, targets, row): (Value, Value, Option<Value>) = sqlx::query_as(sqlx::AssertSqlSafe(format!(
            "SELECT CASE WHEN $2::text = '' THEN '[]'::jsonb ELSE {groups_json} END AS groups, \
               (SELECT COALESCE(jsonb_agg(jsonb_build_array(group_name, killed, forced_variant, flags)), '[]'::jsonb) \
                  FROM telemetry.experiment_group_targets WHERE exp_key = $1 AND $2::text <> '') AS targets, \
               (SELECT jsonb_build_array(killed, forced_variant, flags) \
                  FROM telemetry.experiment_overrides WHERE exp_key = $1) AS override",
            groups_json = groups::GROUPS_JSON,
        )))
        .bind(&key)
        .bind(&user_key)
        .fetch_one(&st.pool)
        .await
        .map_err(|e| db_err("telemetry dashboard", e))?;
        let decode_err =
            |e: serde_json::Error| db_err("telemetry dashboard", sqlx::Error::decode(e));
        let groups = groups::groups_for(
            serde_json::from_value(groups).map_err(decode_err)?,
            &user_key,
        );
        let targets: Vec<(String, bool, Option<String>, Value)> =
            serde_json::from_value(targets).map_err(decode_err)?;
        for name in &groups {
            if let Some((_, killed, variant, flags)) = targets.iter().find(|r| &r.0 == name) {
                return Ok(Json(
                    json!({ "killed": killed, "variant": variant, "flags": flags }),
                ));
            }
        }
        let row: Option<(bool, Option<String>, Value)> = match row {
            Some(v) => Some(serde_json::from_value(v).map_err(decode_err)?),
            None => None,
        };
        Ok(Json(match row {
            Some((killed, variant, flags)) => {
                json!({ "killed": killed, "variant": variant, "flags": flags })
            }
            None => json!({}),
        }))
    } else {
        let rows = sqlx::query_as::<_, (String, bool, Option<String>, Value)>(
            "SELECT exp_key, killed, forced_variant, flags FROM telemetry.experiment_overrides ORDER BY exp_key",
        )
        .fetch_all(&st.pool)
        .await
        .map_err(|e| db_err("telemetry dashboard", e))?;
        let mut out = serde_json::Map::new();
        for (exp_key, killed, variant, flags) in rows {
            out.insert(
                exp_key,
                json!({ "killed": killed, "variant": variant, "flags": flags }),
            );
        }
        Ok(Json(Value::Object(out)))
    }
}

#[derive(Deserialize)]
pub struct ExperimentSetBody {
    exp_key: String,
    #[serde(default)]
    killed: bool,
    #[serde(default)]
    variant: Option<String>,
    #[serde(default)]
    flags: Option<Value>,
    #[serde(default)]
    clear: bool,
}

pub async fn experiment_set(
    admin: TelemetryAdmin,
    State(st): State<SharedState>,
    Json(b): Json<ExperimentSetBody>,
) -> Result<Json<Value>, (StatusCode, String)> {
    if b.exp_key.is_empty() {
        return Err((StatusCode::BAD_REQUEST, "exp_key required".into()));
    }
    let action = if b.clear {
        "experiment.clear"
    } else {
        "experiment.set"
    };
    let detail = json!({
        "exp_key": b.exp_key,
        "killed": b.killed,
        "variant": b.variant,
        "flags": b.flags,
        "clear": b.clear,
    });
    if b.clear {
        sqlx::query(sqlx::AssertSqlSafe(audited(
            "DELETE FROM telemetry.experiment_overrides WHERE exp_key = $1",
            1,
        )))
        .bind(&b.exp_key)
        .bind(admin.actor())
        .bind(action)
        .bind(&b.exp_key)
        .bind(detail)
        .execute(&st.pool)
        .await
        .map_err(|e| db_err("telemetry dashboard", e))?;
    } else {
        let flags = b.flags.clone().unwrap_or_else(|| json!({}));
        sqlx::query(sqlx::AssertSqlSafe(audited(
            "INSERT INTO telemetry.experiment_overrides (exp_key, killed, forced_variant, flags, updated_at) \
             VALUES ($1, $2, $3, $4, timezone('UTC', now())) \
             ON CONFLICT (exp_key) DO UPDATE SET \
               killed = $2, forced_variant = $3, flags = $4, updated_at = timezone('UTC', now())",
            4,
        )))
        .bind(&b.exp_key)
        .bind(b.killed)
        .bind(&b.variant)
        .bind(flags)
        .bind(admin.actor())
        .bind(action)
        .bind(&b.exp_key)
        .bind(detail)
        .execute(&st.pool)
        .await
        .map_err(|e| db_err("telemetry dashboard", e))?;
    }
    Ok(Json(json!({ "ok": true, "exp_key": b.exp_key })))
}
