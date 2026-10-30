//! The outbox worker for the out-of-process channels (`email` today):
//! DATABASE.md's "poll indexed, bounded due sets ... short atomic claim with
//! lease/recovery semantics; do network work outside the transaction;
//! preserve idempotency on redelivery", over `social.notification_deliveries`
//! The old crate had no out-of-process channel (rows were in-app only), so this
//! is new behaviour with an explicit degraded mode: without
//! `SENDGRID_NOTIFICATION_TEMPLATE_ID` every claimed row ends `skipped`.

use std::time::Duration;

use chrono::NaiveDateTime;
use serde_json::Value as Json;
use sqlx::PgPool;
use uuid::Uuid;

use super::email::{EmailSender, SendFailure};
use crate::shared::now_utc;

pub const BATCH: i64 = 100;
pub const MAX_ATTEMPTS: i32 = 5;
pub const LEASE: Duration = Duration::from_secs(300);
const BACKOFF_BASE: Duration = Duration::from_secs(30);
const BACKOFF_MAX: Duration = Duration::from_secs(3600);

/// A claimed `email` row with what the send needs, read in the claim.
#[derive(Debug, Clone)]
pub struct Claimed {
    pub notification_id: Uuid,
    pub attempts: i32,
    pub address: String,
    pub kind: String,
    pub metadata: Json,
    pub email: Option<String>,
}

/// What a channel does with a claimed row; the SendGrid sender is the
/// production one, tests plug a recorder in.
#[async_trait::async_trait]
pub trait EmailChannel: Send + Sync {
    async fn send(&self, to: &str, claimed: &Claimed) -> Result<(), SendFailure>;
}

#[async_trait::async_trait]
impl EmailChannel for EmailSender {
    async fn send(&self, to: &str, claimed: &Claimed) -> Result<(), SendFailure> {
        self.send_notification(
            to,
            &claimed.address,
            claimed.notification_id,
            &claimed.kind,
            &claimed.metadata,
        )
        .await
    }
}

/// The wait after the `attempts`-th failure: 30 s, 60 s, 120 s ... capped at an hour.
pub fn backoff(attempts: i32) -> Duration {
    let factor = 1u32 << attempts.clamp(1, 16).saturating_sub(1) as u32;
    BACKOFF_BASE.saturating_mul(factor).min(BACKOFF_MAX)
}

/// Claims the due rows (`state = 'pending' AND next_attempt_at <= now`, the
/// `notification_deliveries_due_idx` order, `SKIP LOCKED`) and, in the same
/// statement, counts the attempt and moves `next_attempt_at` to `now + LEASE`
/// so a crash before the outcome lands makes the row due again later.
pub async fn claim(
    pool: &PgPool,
    now: NaiveDateTime,
    limit: i64,
) -> Result<Vec<Claimed>, sqlx::Error> {
    let lease_until = now + chrono::Duration::from_std(LEASE).expect("LEASE fits");
    let rows: Vec<(Uuid, i32, String, String, Json, Option<String>)> = sqlx::query_as(
        r#"
        WITH due AS (
            SELECT d.notification_id
            FROM social.notification_deliveries d
            WHERE d.state = 'pending' AND d.channel = 'email' AND d.next_attempt_at <= $1
            ORDER BY d.next_attempt_at, d.notification_id
            LIMIT $2
            FOR UPDATE SKIP LOCKED
        )
        UPDATE social.notification_deliveries d
        SET attempts = d.attempts + 1, next_attempt_at = $3, updated_at = $1
        FROM due
        JOIN social.notifications n ON n.id = due.notification_id
        LEFT JOIN social.subscriptions s ON s.address = n.address
        WHERE d.notification_id = due.notification_id AND d.channel = 'email'
        RETURNING d.notification_id, d.attempts, n.address, n.type, n.metadata, s.email
        "#,
    )
    .bind(now)
    .bind(limit)
    .bind(lease_until)
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(
            |(notification_id, attempts, address, kind, metadata, email)| Claimed {
                notification_id,
                attempts,
                address,
                kind,
                metadata,
                email,
            },
        )
        .collect())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    Sent,
    Skipped(String),
    Failed(String),
    Retry { after: Duration, error: String },
}

/// The state transition for one claimed row's send result.
pub fn outcome_of(claimed: &Claimed, result: Result<(), SendFailure>) -> Outcome {
    match result {
        Ok(()) => Outcome::Sent,
        Err(SendFailure::NotConfigured) => {
            Outcome::Skipped("email delivery not configured".to_string())
        }
        Err(SendFailure::Permanent(e)) => Outcome::Failed(e),
        Err(SendFailure::Transient(e)) if claimed.attempts >= MAX_ATTEMPTS => {
            Outcome::Failed(format!("{e} (after {} attempts)", claimed.attempts))
        }
        Err(SendFailure::Transient(e)) => Outcome::Retry {
            after: backoff(claimed.attempts),
            error: e,
        },
    }
}

pub async fn record(
    pool: &PgPool,
    id: Uuid,
    now: NaiveDateTime,
    outcome: &Outcome,
) -> Result<(), sqlx::Error> {
    let (state, next_attempt_at, last_error): (&str, NaiveDateTime, Option<&str>) = match outcome {
        Outcome::Sent => ("sent", now, None),
        Outcome::Skipped(reason) => ("skipped", now, Some(reason)),
        Outcome::Failed(error) => ("failed", now, Some(error)),
        Outcome::Retry { after, error } => (
            "pending",
            now + chrono::Duration::from_std(*after).expect("backoff fits"),
            Some(error),
        ),
    };
    sqlx::query(
        r#"
        UPDATE social.notification_deliveries
        SET state = $2, next_attempt_at = $3, last_error = $4, updated_at = $5
        WHERE notification_id = $1 AND channel = 'email'
        "#,
    )
    .bind(id)
    .bind(state)
    .bind(next_attempt_at)
    .bind(last_error)
    .bind(now)
    .execute(pool)
    .await?;
    Ok(())
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct PassSummary {
    pub claimed: usize,
    pub sent: usize,
    pub skipped: usize,
    pub failed: usize,
    pub retried: usize,
}

/// One pass: claim, send each outside any transaction, record. A row whose
/// recipient lost their email between insert and send is `skipped` without
/// a send.
pub async fn run_pass(
    pool: &PgPool,
    channel: &dyn EmailChannel,
    configured: bool,
) -> anyhow::Result<PassSummary> {
    let now = now_utc();
    let claimed = claim(pool, now, BATCH).await?;
    let mut summary = PassSummary {
        claimed: claimed.len(),
        ..Default::default()
    };
    for row in &claimed {
        let result = match (&row.email, configured) {
            (None, _) => Err(SendFailure::Permanent(
                "no confirmed email at send time".to_string(),
            )),
            (Some(_), false) => Err(SendFailure::NotConfigured),
            (Some(to), true) => channel.send(to, row).await,
        };
        let outcome = match (&row.email, result) {
            (None, Err(SendFailure::Permanent(e))) => Outcome::Skipped(e),
            (_, result) => outcome_of(row, result),
        };
        match &outcome {
            Outcome::Sent => summary.sent += 1,
            Outcome::Skipped(_) => summary.skipped += 1,
            Outcome::Failed(_) => summary.failed += 1,
            Outcome::Retry { .. } => summary.retried += 1,
        }
        record(pool, row.notification_id, now_utc(), &outcome).await?;
    }
    Ok(summary)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backoff_doubles_from_30s_to_the_hour_cap() {
        assert_eq!(backoff(1), Duration::from_secs(30));
        assert_eq!(backoff(2), Duration::from_secs(60));
        assert_eq!(backoff(3), Duration::from_secs(120));
        assert_eq!(backoff(7), Duration::from_secs(1920));
        assert_eq!(backoff(8), Duration::from_secs(3600));
        assert_eq!(backoff(40), Duration::from_secs(3600));
        assert_eq!(backoff(0), Duration::from_secs(30));
    }

    fn claimed(attempts: i32) -> Claimed {
        Claimed {
            notification_id: Uuid::nil(),
            attempts,
            address: "0xa".into(),
            kind: "bid_received".into(),
            metadata: serde_json::json!({}),
            email: Some("a@example.com".into()),
        }
    }

    #[test]
    fn outcome_table() {
        assert_eq!(outcome_of(&claimed(1), Ok(())), Outcome::Sent);
        assert_eq!(
            outcome_of(&claimed(1), Err(SendFailure::NotConfigured)),
            Outcome::Skipped("email delivery not configured".into())
        );
        assert_eq!(
            outcome_of(&claimed(1), Err(SendFailure::Permanent("400".into()))),
            Outcome::Failed("400".into())
        );
        assert_eq!(
            outcome_of(&claimed(2), Err(SendFailure::Transient("503".into()))),
            Outcome::Retry {
                after: Duration::from_secs(60),
                error: "503".into()
            }
        );
        assert!(matches!(
            outcome_of(
                &claimed(MAX_ATTEMPTS),
                Err(SendFailure::Transient("503".into()))
            ),
            Outcome::Failed(_)
        ));
    }
}

#[cfg(test)]
mod pg_tests {
    use super::*;
    use crate::notifications::store::test_support::*;
    use crate::notifications::store::{NewNotification, insert_notifications};
    use parking_lot::Mutex;

    struct Recorder {
        sent: Mutex<Vec<(String, Uuid)>>,
        fail_with: Mutex<Option<SendFailure>>,
    }

    impl Recorder {
        fn new() -> Self {
            Self {
                sent: Mutex::new(Vec::new()),
                fail_with: Mutex::new(None),
            }
        }
    }

    #[async_trait::async_trait]
    impl EmailChannel for Recorder {
        async fn send(&self, to: &str, claimed: &Claimed) -> Result<(), SendFailure> {
            if let Some(f) = self.fail_with.lock().clone() {
                return Err(f);
            }
            self.sent
                .lock()
                .push((to.to_string(), claimed.notification_id));
            Ok(())
        }
    }

    async fn pending_for(
        pool: &PgPool,
        address: &crate::identity::Address,
    ) -> Vec<NewNotification> {
        sqlx::query("INSERT INTO social.subscriptions (address, email) VALUES ($1, $2)")
            .bind(address)
            .bind(format!("{}@example.com", &address.as_str()[2..10]))
            .execute(pool)
            .await
            .unwrap();
        let batch = vec![NewNotification::now(
            address.clone(),
            "bid_received",
            serde_json::json!({"title": "t"}),
        )];
        assert_eq!(insert_notifications(pool, &batch).await.unwrap(), 1);
        batch
    }

    #[tokio::test]
    async fn a_pass_sends_pending_rows_once_and_records_the_outcome() {
        let Some(pool) = private_pool("dlv").await else {
            return;
        };
        let a = fresh_address();
        let batch = pending_for(&pool, &a).await;
        let id = batch[0].id;
        let recorder = Recorder::new();

        let summary = run_pass(&pool, &recorder, true).await.unwrap();
        assert_eq!(
            summary,
            PassSummary {
                claimed: 1,
                sent: 1,
                ..Default::default()
            }
        );
        assert_eq!(
            *recorder.sent.lock(),
            vec![(format!("{}@example.com", &a.as_str()[2..10]), id)]
        );
        assert_eq!(
            delivery_state(&pool, id).await.unwrap(),
            ("sent".into(), 1, None)
        );

        let summary = run_pass(&pool, &recorder, true).await.unwrap();
        assert_eq!(
            summary,
            PassSummary::default(),
            "a sent row is never claimed again"
        );
        assert_eq!(recorder.sent.lock().len(), 1);
    }

    #[tokio::test]
    async fn unconfigured_email_skips_explicitly_and_a_missing_recipient_email_skips_without_a_send()
     {
        let Some(pool) = private_pool("dlv").await else {
            return;
        };
        let a = pending_for(&pool, &fresh_address()).await;
        let recorder = Recorder::new();
        run_pass(&pool, &recorder, false).await.unwrap();
        assert_eq!(
            delivery_state(&pool, a[0].id).await.unwrap(),
            (
                "skipped".into(),
                1,
                Some("email delivery not configured".into())
            )
        );
        assert!(recorder.sent.lock().iter().all(|(_, n)| *n != a[0].id));

        let b_addr = fresh_address();
        let b = pending_for(&pool, &b_addr).await;
        sqlx::query("UPDATE social.subscriptions SET email = NULL WHERE address = $1")
            .bind(&b_addr)
            .execute(&pool)
            .await
            .unwrap();
        run_pass(&pool, &recorder, true).await.unwrap();
        assert_eq!(
            delivery_state(&pool, b[0].id).await.unwrap(),
            (
                "skipped".into(),
                1,
                Some("no confirmed email at send time".into())
            )
        );
        assert!(recorder.sent.lock().iter().all(|(_, n)| *n != b[0].id));
    }

    #[tokio::test]
    async fn transient_failures_back_off_then_fail_and_a_lapsed_lease_is_reclaimed() {
        let Some(pool) = private_pool("dlv").await else {
            return;
        };
        let batch = pending_for(&pool, &fresh_address()).await;
        let id = batch[0].id;
        let recorder = Recorder::new();
        *recorder.fail_with.lock() = Some(SendFailure::Transient("503".into()));

        run_pass(&pool, &recorder, true).await.unwrap();
        let (state, attempts, err) = delivery_state(&pool, id).await.unwrap();
        assert_eq!(
            (state.as_str(), attempts, err.as_deref()),
            ("pending", 1, Some("503"))
        );
        let next: NaiveDateTime = sqlx::query_scalar(
            "SELECT next_attempt_at FROM social.notification_deliveries WHERE notification_id = $1",
        )
        .bind(id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert!(
            next > now_utc() + chrono::Duration::seconds(20),
            "backed off 30 s, not due now"
        );

        run_pass(&pool, &recorder, true).await.unwrap();
        assert_eq!(
            delivery_state(&pool, id).await.unwrap().1,
            1,
            "a row in backoff is not reclaimed"
        );

        // A claim whose outcome never landed (crash inside the lease): due again once the lease lapses.
        sqlx::query("UPDATE social.notification_deliveries SET next_attempt_at = $2 WHERE notification_id = $1")
            .bind(id)
            .bind(now_utc() - chrono::Duration::seconds(1))
            .execute(&pool)
            .await
            .unwrap();
        let claimed = claim(&pool, now_utc(), BATCH).await.unwrap();
        assert_eq!(claimed.len(), 1);
        assert_eq!((claimed[0].notification_id, claimed[0].attempts), (id, 2));
        let leased: NaiveDateTime = sqlx::query_scalar(
            "SELECT next_attempt_at FROM social.notification_deliveries WHERE notification_id = $1",
        )
        .bind(id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert!(
            leased > now_utc() + chrono::Duration::seconds(200),
            "the claim leases the row"
        );
        assert!(
            claim(&pool, now_utc(), BATCH).await.unwrap().is_empty(),
            "not claimable inside the lease"
        );

        for _ in 0..MAX_ATTEMPTS {
            sqlx::query("UPDATE social.notification_deliveries SET next_attempt_at = $2 WHERE notification_id = $1 AND state = 'pending'")
                .bind(id)
                .bind(now_utc() - chrono::Duration::seconds(1))
                .execute(&pool)
                .await
                .unwrap();
            run_pass(&pool, &recorder, true).await.unwrap();
        }
        let (state, attempts, err) = delivery_state(&pool, id).await.unwrap();
        assert_eq!(state, "failed");
        assert!(attempts >= MAX_ATTEMPTS, "{attempts}");
        assert!(
            err.unwrap().contains("after"),
            "the terminal error names the attempt count"
        );
        assert!(recorder.sent.lock().is_empty());

        // A permanent refusal is terminal at once.
        let other = pending_for(&pool, &fresh_address()).await;
        *recorder.fail_with.lock() = Some(SendFailure::Permanent("400 bad template".into()));
        run_pass(&pool, &recorder, true).await.unwrap();
        assert_eq!(
            delivery_state(&pool, other[0].id).await.unwrap(),
            ("failed".into(), 1, Some("400 bad template".into()))
        );
    }
}
