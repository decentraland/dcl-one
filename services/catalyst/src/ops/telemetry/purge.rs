//! The `telemetry-purge` job: rows older than `TELEMETRY_RETENTION_DAYS`
//! and `client_error` diagnostics older than 30 days (the old service's one
//! rule) are deleted in bounded `ctid` batches, each statement its own
//! transaction, the cutoff bound once from the pass clock, at most
//! `TELEMETRY_PURGE_MAX_BATCHES` per pass, so a pass never holds a long
//! lock against the ingest writer and a backlog drains over several days
//! rather than in one transaction. The operator's purge and bulk-delete run
//! the same batches ([`delete_in_batches`]) under the same budget.

use chrono::NaiveDateTime;
use sqlx::PgPool;

use super::{CLIENT_ERROR_RETENTION_DAYS, State};
use crate::jobs::JobContext;

pub const BATCH_ROWS: i64 = 5_000;

const PURGE_OLDER_THAN_SQL: &str = "DELETE FROM telemetry.events WHERE ctid = ANY (ARRAY( \
       SELECT ctid FROM telemetry.events WHERE received_at < $1 ORDER BY received_at LIMIT $2))";

const PURGE_CLIENT_ERRORS_SQL: &str = "DELETE FROM telemetry.events WHERE ctid = ANY (ARRAY( \
       SELECT ctid FROM telemetry.events \
       WHERE source = 'segment' AND event_kind = 'track' AND body->>'event' = 'client_error' \
         AND received_at < $1 ORDER BY received_at LIMIT $2))";

#[derive(Debug, Default, PartialEq, Eq)]
pub struct PassReport {
    pub expired: u64,
    pub client_errors: u64,
    pub batches: u32,
    /// The batch budget ran out with rows still due; the next pass continues.
    pub exhausted: bool,
}

pub fn cutoffs(now: NaiveDateTime, retention_days: u32) -> (NaiveDateTime, NaiveDateTime) {
    (
        now - chrono::Duration::days(i64::from(retention_days)),
        now - chrono::Duration::days(i64::from(CLIENT_ERROR_RETENTION_DAYS)),
    )
}

/// Deletes in `BATCH_ROWS` batches until a short batch or the budget; `None`
/// as the context runs unattended (the operator's manual invocation).
pub async fn purge_bounded(
    pool: &PgPool,
    now: NaiveDateTime,
    retention_days: u32,
    max_batches: u32,
    ctx: Option<&JobContext>,
) -> Result<PassReport, sqlx::Error> {
    let (expired_before, client_error_before) = cutoffs(now, retention_days);
    let mut report = PassReport::default();
    for (sql, cutoff, counter) in [
        (PURGE_OLDER_THAN_SQL, expired_before, 0usize),
        (PURGE_CLIENT_ERRORS_SQL, client_error_before, 1usize),
    ] {
        loop {
            if ctx.is_some_and(|c| c.is_cancelled()) {
                report.exhausted = true;
                return Ok(report);
            }
            if report.batches >= max_batches {
                report.exhausted = true;
                return Ok(report);
            }
            let deleted = sqlx::query(sql)
                .bind(cutoff)
                .bind(BATCH_ROWS)
                .execute(pool)
                .await?
                .rows_affected();
            report.batches += 1;
            if counter == 0 {
                report.expired += deleted;
            } else {
                report.client_errors += deleted;
            }
            if let Some(c) = ctx {
                c.progress(format!(
                    "expired {} client_error {} in {} batches",
                    report.expired, report.client_errors, report.batches
                ));
            }
            if deleted < BATCH_ROWS as u64 {
                break;
            }
        }
    }
    Ok(report)
}

/// What a bounded operator delete did: `deleted` rows in `batches`
/// statements; `exhausted` when the budget ran out with rows still matching
/// (the operator repeats the call; every batch already committed).
#[derive(Debug, Default, PartialEq, Eq)]
pub struct BatchReport {
    pub deleted: u64,
    pub batches: u32,
    pub exhausted: bool,
}

/// Runs `statement(batch_rows)` (a `DELETE ... WHERE ctid = ANY (ARRAY(SELECT
/// ctid ... LIMIT $batch))`) until a short batch or `max_batches`, each
/// statement its own transaction: the operator routes' purge and bulk-delete
/// take this shape so a multi-million-row request is resumable and never
/// meets the pool's statement timeout with nothing deleted.
pub async fn delete_in_batches<F>(
    pool: &PgPool,
    max_batches: u32,
    mut statement: F,
) -> Result<BatchReport, sqlx::Error>
where
    F: FnMut(i64) -> sqlx::query::Query<'static, sqlx::Postgres, sqlx::postgres::PgArguments>,
{
    let mut report = BatchReport::default();
    loop {
        if report.batches >= max_batches.max(1) {
            report.exhausted = true;
            return Ok(report);
        }
        let deleted = statement(BATCH_ROWS).execute(pool).await?.rows_affected();
        report.batches += 1;
        report.deleted += deleted;
        if deleted < BATCH_ROWS as u64 {
            return Ok(report);
        }
    }
}

pub async fn run_pass(state: &State, ctx: &JobContext) -> anyhow::Result<()> {
    let now = chrono::Utc::now().naive_utc();
    let report = purge_bounded(
        &state.pool,
        now,
        state.settings.retention_days,
        state.settings.purge_max_batches,
        Some(ctx),
    )
    .await?;
    if report.expired > 0 || report.client_errors > 0 {
        tracing::info!(
            expired = report.expired,
            client_errors = report.client_errors,
            batches = report.batches,
            exhausted = report.exhausted,
            retention_days = state.settings.retention_days,
            "telemetry purge pass"
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn full_client_error_diagnostics_keep_thirty_days() {
        assert_eq!(CLIENT_ERROR_RETENTION_DAYS, 30);
        let now = chrono::NaiveDate::from_ymd_opt(2026, 9, 25)
            .unwrap()
            .and_hms_opt(12, 0, 0)
            .unwrap();
        let (expired, client) = cutoffs(now, 90);
        assert_eq!(expired, now - chrono::Duration::days(90));
        assert_eq!(client, now - chrono::Duration::days(30));
    }
}
