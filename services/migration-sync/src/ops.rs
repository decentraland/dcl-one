//! Payload-free diagnostics and explicit, non-skipping recovery.
use anyhow::{Context, Result, ensure};
use serde_json::{Value, json};
use sqlx::{PgPool, Row};

use crate::{
    model::{parse_lsn, validate_stream},
    store,
};

pub async fn status(pool: &PgPool, stream: &str) -> Result<Value> {
    let mut result = store::status(pool, stream).await?;
    let capture = sqlx::query("SELECT adapter,captured_position,observed_position,
        GREATEST(0,extract(epoch FROM clock_timestamp()-last_contact_at))::double precision AS contact_age
        FROM migration_sync.capture_sources WHERE stream=$1")
        .bind(stream).fetch_optional(pool).await?;
    if let Some(row) = capture {
        let applied = parse_lsn(
            result["applied_lsn"]
                .as_str()
                .context("missing applied cursor")?,
        )?;
        let captured: i64 = row.try_get("captured_position")?;
        let observed: i64 = row.try_get("observed_position")?;
        result["capture"] = json!({
            "adapter":row.try_get::<String,_>("adapter")?,
            "applied_position":applied,
            "captured_position":captured,
            "observed_position":observed,
            "capture_lag":observed.saturating_sub(captured),
            "apply_lag":(captured as u64).saturating_sub(applied),
            "contact_age_seconds":row.try_get::<f64,_>("contact_age")?
        });
    }
    let row = sqlx::query("SELECT count(*) FILTER(WHERE i.status='pending' AND i.payload->>'mode'='dual' AND r.operation_id IS NULL) AS awaiting_receipt,
        COALESCE(max(extract(epoch FROM clock_timestamp()-i.received_at)) FILTER(WHERE i.status<>'applied'),0)::double precision AS oldest_age
        FROM migration_sync.inbox i LEFT JOIN migration_sync.command_receipts r
        ON r.stream=i.stream AND r.generation=i.generation AND r.commit_lsn=i.commit_lsn WHERE i.stream=$1")
        .bind(stream).fetch_one(pool).await?;
    result["awaiting_receipt"] = json!(row.try_get::<i64, _>("awaiting_receipt")?);
    result["oldest_unapplied_age_seconds"] = json!(row.try_get::<f64, _>("oldest_age")?);
    Ok(result)
}

/// Fail closed for unattended rehearsal sampling. Contact freshness alone is
/// insufficient: capture and application must also have reached the observed head.
pub fn check_health(value: &Value, max_age_seconds: u64) -> Result<()> {
    ensure!(value["paused"] == false, "stream is paused");
    ensure!(
        value["pending"] == 0 && value["blocked"] == 0,
        "stream has unapplied transactions"
    );
    let capture = value
        .get("capture")
        .context("stream has no capture bootstrap")?;
    ensure!(
        capture["capture_lag"] == 0 && capture["apply_lag"] == 0,
        "stream is not caught up"
    );
    let age = capture["contact_age_seconds"]
        .as_f64()
        .context("missing source contact age")?;
    ensure!(
        age.is_finite() && age >= 0.0 && age <= max_age_seconds as f64,
        "source contact is stale"
    );
    Ok(())
}

/// Recovery only resets the current blocked head, while paused. Payload,
/// generation, receipts and checkpoints remain immutable. The next run must
/// revalidate everything and may block again if the underlying issue persists.
pub async fn retry_blocked(pool: &PgPool, stream: &str, expected_cursor: &str) -> Result<()> {
    validate_stream(stream)?;
    parse_lsn(expected_cursor)?;
    let mut tx = pool.begin().await?;
    let paused: bool =
        sqlx::query_scalar("SELECT paused FROM migration_sync.streams WHERE name=$1 FOR UPDATE")
            .bind(stream)
            .fetch_optional(&mut *tx)
            .await?
            .context("stream is not registered")?;
    ensure!(paused, "pause the stream before retrying a blocked head");
    let updated = sqlx::query("UPDATE migration_sync.inbox i SET status='pending',error_code=NULL
        FROM migration_sync.streams s WHERE s.name=$1 AND i.stream=s.name AND i.generation=s.generation
        AND i.previous_lsn=s.applied_lsn AND i.commit_lsn=$2::text::pg_lsn AND i.status='blocked'")
        .bind(stream).bind(expected_cursor).execute(&mut *tx).await?;
    ensure!(
        updated.rows_affected() == 1,
        "expected cursor is not the current blocked head"
    );
    tx.commit().await?;
    Ok(())
}

/// Prometheus text exposition, suitable for a supervised textfile collector.
/// Labels are restricted stream identifiers; no source rows or credentials.
pub fn metrics(value: &Value) -> Result<String> {
    let stream = value["stream"].as_str().context("missing stream")?;
    validate_stream(stream)?;
    let mut output = String::new();
    let mut gauge = |name: &str, number: f64| {
        output.push_str(&format!("# TYPE migration_sync_{name} gauge\nmigration_sync_{name}{{stream=\"{stream}\"}} {number}\n"));
    };
    gauge("paused", if value["paused"] == true { 1.0 } else { 0.0 });
    for key in [
        "pending",
        "blocked",
        "awaiting_receipt",
        "oldest_unapplied_age_seconds",
    ] {
        gauge(key, value[key].as_f64().context("invalid status metric")?);
    }
    gauge(
        "capture_configured",
        if value.get("capture").is_some() {
            1.0
        } else {
            0.0
        },
    );
    if let Some(capture) = value.get("capture") {
        for key in ["capture_lag", "apply_lag", "contact_age_seconds"] {
            gauge(
                key,
                capture[key].as_f64().context("invalid capture metric")?,
            );
        }
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metrics_include_capture_health_without_payloads() {
        let value = json!({"stream":"places","paused":false,"pending":2,"blocked":0,
            "awaiting_receipt":1,"oldest_unapplied_age_seconds":4,
            "capture":{"capture_lag":2,"apply_lag":1,"contact_age_seconds":0.5},
            "payload":"private-canary"});
        let output = metrics(&value).unwrap();
        assert!(output.contains("migration_sync_apply_lag{stream=\"places\"} 1"));
        assert!(output.contains("migration_sync_capture_configured{stream=\"places\"} 1"));
        assert!(!output.contains("private-canary"));
    }

    #[test]
    fn health_requires_capture_progress_and_freshness() {
        let mut value = json!({"paused":false,"pending":0,"blocked":0,"capture":{
            "capture_lag":0,"apply_lag":0,"contact_age_seconds":2}});
        assert!(check_health(&value, 30).is_ok());
        value["capture"]["contact_age_seconds"] = json!(31);
        assert!(check_health(&value, 30).is_err());
        value["capture"]["contact_age_seconds"] = json!(0);
        value["pending"] = json!(1);
        assert!(check_health(&value, 30).is_err());
        value["pending"] = json!(0);
        value["paused"] = json!(true);
        assert!(check_health(&value, 30).is_err());
    }

    #[test]
    fn metrics_reject_untrusted_labels() {
        assert!(metrics(&json!({"stream":"places\"\\\nsecret"})).is_err());
    }
}
