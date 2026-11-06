//! The outbox worker for the out-of-process channels (`email` today):
//! DATABASE.md's "poll indexed, bounded due sets ... short atomic claim with
//! lease/recovery semantics; do network work outside the transaction;
//! preserve idempotency on redelivery", over `social.notification_deliveries`
//! The old crate had no out-of-process channel (rows were in-app only), so this
//! is new behaviour with an explicit degraded mode: without
//! `SENDGRID_NOTIFICATION_TEMPLATE_ID` every claimed row ends `skipped`;
//! without `SIGNING_KEY` rows stay `pending` (backing off) until it is set.
//! Opt-outs are checked again at claim time, so an unsubscribe stops queued rows.
//! Terminal rows are pruned after [`RETENTION`].

use std::time::Duration;

use chrono::NaiveDateTime;
use serde_json::Value as Json;
use sqlx::PgPool;
use uuid::Uuid;

use super::email::{EmailSender, SendFailure};
use super::first_wear::creator_profile_name;
use super::templates;
use crate::shared::now_utc;

pub const BATCH: i64 = 100;
pub const MAX_ATTEMPTS: i32 = 5;
pub const LEASE: Duration = Duration::from_secs(300);
const BACKOFF_BASE: Duration = Duration::from_secs(30);
const BACKOFF_MAX: Duration = Duration::from_secs(3600);
pub const RETENTION: Duration = Duration::from_secs(30 * 86_400);
pub const PRUNE_BATCH: i64 = 1000;
/// The `last_error` of a row held for a missing `SIGNING_KEY`.
const UNSIGNED: &str = "SIGNING_KEY unset";

/// A claimed `email` row with what the send needs, read in the claim.
#[derive(Debug, Clone)]
pub struct Claimed {
    pub notification_id: Uuid,
    pub attempts: i32,
    pub address: String,
    pub kind: String,
    pub metadata: Json,
    pub email: Option<String>,
    /// Why the recipient no longer takes this email, read in the claim.
    pub opted_out: Option<String>,
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
    type Row = (
        Uuid,
        i32,
        String,
        String,
        Json,
        Option<String>,
        Option<String>,
    );
    let rows: Vec<Row> = sqlx::query_as(
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
        RETURNING d.notification_id, d.attempts, n.address, n.type, n.metadata, s.email,
            CASE
                WHEN COALESCE((s.details->>'ignore_all_email')::boolean, false) THEN 'ignore_all_email'
                WHEN NOT COALESCE((s.details->'message_type'->n.type->>'email')::boolean, true) THEN 'opted out of this type'
                WHEN EXISTS (SELECT 1 FROM social.subscription_opt_outs o
                    WHERE o.address = n.address AND o.scope = 'community'
                      AND CASE WHEN pg_input_is_valid(o.scope_id, 'uuid') THEN o.scope_id::uuid END
                        = CASE WHEN pg_input_is_valid(n.metadata->>'communityId', 'uuid') THEN (n.metadata->>'communityId')::uuid END)
                    THEN 'opted out of this community'
            END
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
            |(notification_id, attempts, address, kind, metadata, email, opted_out)| Claimed {
                notification_id,
                attempts,
                address,
                kind,
                metadata,
                email,
                opted_out,
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
        Err(SendFailure::NoSigningKey) => Outcome::Retry {
            after: backoff(claimed.attempts),
            error: UNSIGNED.to_string(),
        },
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

/// The old processor's render-time names, never persisted: `userName` is the
/// recipient's profile name and a tip's `senderUsername` the sender's, else
/// `Unknown`.
async fn enrich_names(pool: &PgPool, row: &mut Claimed) -> anyhow::Result<()> {
    let name = |n: Option<String>| Json::String(n.unwrap_or_else(|| "Unknown".into()));
    let user = creator_profile_name(pool, &row.address.to_lowercase()).await?;
    let sender = match row.metadata.get("senderAddress").and_then(Json::as_str) {
        Some(s) if row.kind == "tip_received" => {
            Some(creator_profile_name(pool, &s.to_lowercase()).await?)
        }
        _ => None,
    };
    if let Some(m) = row.metadata.as_object_mut() {
        m.insert("userName".into(), name(user));
        if let Some(sender) = sender {
            m.insert("senderUsername".into(), name(sender));
        }
    }
    Ok(())
}

/// Deletes up to [`PRUNE_BATCH`] terminal rows last touched before
/// `now - RETENTION`, oldest first over `notification_deliveries_terminal_idx`.
pub async fn prune(pool: &PgPool, now: NaiveDateTime) -> Result<u64, sqlx::Error> {
    let cutoff = now - chrono::Duration::from_std(RETENTION).expect("RETENTION fits");
    let res = sqlx::query(
        r#"
        DELETE FROM social.notification_deliveries d
        USING (
            SELECT notification_id, channel FROM social.notification_deliveries
            WHERE state <> 'pending' AND updated_at < $1
            ORDER BY updated_at
            LIMIT $2
        ) old
        WHERE d.notification_id = old.notification_id AND d.channel = old.channel
          AND d.state <> 'pending'
        "#,
    )
    .bind(cutoff)
    .bind(PRUNE_BATCH)
    .execute(pool)
    .await?;
    Ok(res.rows_affected())
}

/// One pass: claim, send each outside any transaction, record. A row whose
/// recipient lost their email between insert and send, or whose type has no
/// template (rows written before the allowlist), is `skipped` without a send.
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
    let mut unsigned = 0usize;
    for row in &claimed {
        let outcome = match (&row.email, configured) {
            (None, _) => Outcome::Skipped("no confirmed email at send time".to_string()),
            _ if !templates::is_emailable(&row.kind) => {
                Outcome::Skipped("no email template".to_string())
            }
            _ if row.opted_out.is_some() => {
                Outcome::Skipped(row.opted_out.clone().unwrap_or_default())
            }
            (Some(_), false) => outcome_of(row, Err(SendFailure::NotConfigured)),
            (Some(to), true) => {
                let mut row = row.clone();
                let result = match enrich_names(pool, &mut row).await {
                    Ok(()) => channel.send(to, &row).await,
                    Err(e) => Err(SendFailure::Transient(format!(
                        "profile lookup failed: {e}"
                    ))),
                };
                outcome_of(&row, result)
            }
        };
        if matches!(&outcome, Outcome::Retry { error, .. } if error == UNSIGNED) {
            unsigned += 1;
        }
        match &outcome {
            Outcome::Sent => summary.sent += 1,
            Outcome::Skipped(_) => summary.skipped += 1,
            Outcome::Failed(_) => summary.failed += 1,
            Outcome::Retry { .. } => summary.retried += 1,
        }
        record(pool, row.notification_id, now_utc(), &outcome).await?;
    }
    if unsigned > 0 {
        tracing::error!(
            held = unsigned,
            "SIGNING_KEY unset: notification emails stay pending until it is set"
        );
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
            opted_out: None,
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
        assert_eq!(
            outcome_of(&claimed(MAX_ATTEMPTS + 3), Err(SendFailure::NoSigningKey)),
            Outcome::Retry {
                after: backoff(MAX_ATTEMPTS + 3),
                error: "SIGNING_KEY unset".into()
            },
            "a missing signing key never ends the row"
        );
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
        metadata: Mutex<Vec<(Uuid, Json)>>,
        fail_with: Mutex<Option<SendFailure>>,
    }

    impl Recorder {
        fn new() -> Self {
            Self {
                sent: Mutex::new(Vec::new()),
                metadata: Mutex::new(Vec::new()),
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
            self.metadata
                .lock()
                .push((claimed.notification_id, claimed.metadata.clone()));
            Ok(())
        }
    }

    async fn pending_for(
        pool: &PgPool,
        address: &crate::identity::Address,
    ) -> Vec<NewNotification> {
        notify(
            pool,
            address,
            "bid_received",
            serde_json::json!({"title": "t"}),
        )
        .await
    }

    async fn notify(
        pool: &PgPool,
        address: &crate::identity::Address,
        kind: &str,
        metadata: Json,
    ) -> Vec<NewNotification> {
        sqlx::query("INSERT INTO social.subscriptions (address, email) VALUES ($1, $2)")
            .bind(address)
            .bind(format!("{}@example.com", &address.as_str()[2..10]))
            .execute(pool)
            .await
            .unwrap();
        let batch = vec![NewNotification::now(address.clone(), kind, metadata)];
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

    async fn deploy_profile(pool: &PgPool, address: &str, name: &str) {
        sqlx::query(
            "INSERT INTO content.deployments (deployer_address, version, entity_type, entity_id, entity_metadata, entity_timestamp, entity_pointers, local_timestamp, auth_chain)
             VALUES ($1, 'v3', 'profile', $2, $3::json, $4, ARRAY[$1::text], $4, '[]'::json)",
        )
        .bind(address)
        .bind(format!("bafy-{}", Uuid::new_v4().simple()))
        .bind(serde_json::json!({"v": {"avatars": [{"name": name}]}}))
        .bind(now_utc())
        .execute(pool)
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn a_type_without_a_template_gets_a_skipped_row_and_is_never_sent() {
        let Some(pool) = private_pool("dlv-tpl").await else {
            return;
        };
        let a = fresh_address();
        let rows = notify(&pool, &a, "land_transfer_received", serde_json::json!({})).await;
        assert_eq!(
            delivery_state(&pool, rows[0].id).await.unwrap(),
            ("skipped".into(), 0, Some("no email template".into()))
        );
        // A row written before the allowlist is skipped at send time.
        sqlx::query("UPDATE social.notification_deliveries SET state = 'pending', last_error = NULL WHERE notification_id = $1")
            .bind(rows[0].id)
            .execute(&pool)
            .await
            .unwrap();
        let recorder = Recorder::new();
        run_pass(&pool, &recorder, true).await.unwrap();
        assert_eq!(
            delivery_state(&pool, rows[0].id).await.unwrap(),
            ("skipped".into(), 1, Some("no email template".into()))
        );
        assert!(recorder.sent.lock().iter().all(|(_, n)| *n != rows[0].id));
    }

    #[tokio::test]
    async fn names_are_enriched_at_send_time_and_never_persisted() {
        let Some(pool) = private_pool("dlv-names").await else {
            return;
        };
        let named = fresh_address();
        let sender = fresh_address();
        deploy_profile(&pool, named.as_str(), "Recipient").await;
        deploy_profile(&pool, sender.as_str(), "Tipper").await;
        let tip = notify(
            &pool,
            &named,
            "tip_received",
            serde_json::json!({"senderAddress": sender.as_str().to_uppercase().replacen("0X", "0x", 1)}),
        )
        .await;
        let anon = pending_for(&pool, &fresh_address()).await;
        let recorder = Recorder::new();
        run_pass(&pool, &recorder, true).await.unwrap();
        let seen: std::collections::HashMap<Uuid, Json> =
            recorder.metadata.lock().iter().cloned().collect();
        assert_eq!(seen[&tip[0].id]["userName"], "Recipient");
        assert_eq!(seen[&tip[0].id]["senderUsername"], "Tipper");
        assert_eq!(seen[&anon[0].id]["userName"], "Unknown");
        assert!(seen[&anon[0].id].get("senderUsername").is_none());
        let stored: Json =
            sqlx::query_scalar("SELECT metadata FROM social.notifications WHERE id = $1")
                .bind(tip[0].id)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert!(
            stored.get("userName").is_none(),
            "enrichment is not persisted"
        );
    }

    #[tokio::test]
    async fn prune_deletes_only_terminal_rows_past_retention() {
        let Some(pool) = private_pool("dlv-prune").await else {
            return;
        };
        let old = now_utc() - chrono::Duration::days(31);
        let mut ids = Vec::new();
        for state in ["sent", "skipped", "failed", "pending"] {
            let rows = pending_for(&pool, &fresh_address()).await;
            sqlx::query("UPDATE social.notification_deliveries SET state = $2, updated_at = $3 WHERE notification_id = $1")
                .bind(rows[0].id)
                .bind(state)
                .bind(old)
                .execute(&pool)
                .await
                .unwrap();
            ids.push(rows[0].id);
        }
        let recent = pending_for(&pool, &fresh_address()).await;
        sqlx::query(
            "UPDATE social.notification_deliveries SET state = 'sent' WHERE notification_id = $1",
        )
        .bind(recent[0].id)
        .execute(&pool)
        .await
        .unwrap();

        assert_eq!(prune(&pool, now_utc()).await.unwrap(), 3);
        for id in &ids[..3] {
            assert_eq!(delivery_state(&pool, *id).await, None);
        }
        assert_eq!(delivery_state(&pool, ids[3]).await.unwrap().0, "pending");
        assert_eq!(delivery_state(&pool, recent[0].id).await.unwrap().0, "sent");
        assert_eq!(prune(&pool, now_utc()).await.unwrap(), 0);
    }

    #[tokio::test]
    async fn a_claim_rechecks_opt_outs_and_skips_rows_queued_before_them() {
        let Some(pool) = private_pool("dlv-optout").await else {
            return;
        };
        let all = fresh_address();
        let typed = fresh_address();
        let member = fresh_address();
        let kept = fresh_address();
        let community = Uuid::new_v4();
        let rows_all = pending_for(&pool, &all).await;
        let rows_typed = pending_for(&pool, &typed).await;
        let rows_member = notify(
            &pool,
            &member,
            "events_started",
            serde_json::json!({"communityId": community.to_string().to_uppercase()}),
        )
        .await;
        let rows_kept = pending_for(&pool, &kept).await;
        for id in [
            rows_all[0].id,
            rows_typed[0].id,
            rows_member[0].id,
            rows_kept[0].id,
        ] {
            assert_eq!(delivery_state(&pool, id).await.unwrap().0, "pending");
        }
        let store = crate::notifications::NotificationsStore::new(pool.clone());
        store.unsubscribe_email(&all, None).await.unwrap();
        store
            .unsubscribe_email(&typed, Some("bid_received"))
            .await
            .unwrap();
        sqlx::query("INSERT INTO social.subscription_opt_outs (address, scope, scope_id) VALUES ($1, 'community', $2)")
            .bind(&member)
            .bind(community.to_string())
            .execute(&pool)
            .await
            .unwrap();

        let recorder = Recorder::new();
        run_pass(&pool, &recorder, true).await.unwrap();
        for (id, reason) in [
            (rows_all[0].id, "ignore_all_email"),
            (rows_typed[0].id, "opted out of this type"),
            (rows_member[0].id, "opted out of this community"),
        ] {
            assert_eq!(
                delivery_state(&pool, id).await.unwrap(),
                ("skipped".into(), 1, Some(reason.into()))
            );
        }
        let sent: Vec<Uuid> = recorder.sent.lock().iter().map(|(_, id)| *id).collect();
        assert_eq!(
            sent,
            vec![rows_kept[0].id],
            "only the row still wanted is sent"
        );
    }

    #[tokio::test]
    async fn without_a_signing_key_rows_stay_pending_and_back_off() {
        let Some(pool) = private_pool("dlv-unsigned").await else {
            return;
        };
        let rows = pending_for(&pool, &fresh_address()).await;
        let recorder = Recorder::new();
        *recorder.fail_with.lock() = Some(SendFailure::NoSigningKey);
        for _ in 0..MAX_ATTEMPTS + 1 {
            sqlx::query("UPDATE social.notification_deliveries SET next_attempt_at = $2 WHERE notification_id = $1")
                .bind(rows[0].id)
                .bind(now_utc() - chrono::Duration::seconds(1))
                .execute(&pool)
                .await
                .unwrap();
            run_pass(&pool, &recorder, true).await.unwrap();
        }
        let (state, attempts, err) = delivery_state(&pool, rows[0].id).await.unwrap();
        assert_eq!(
            (state.as_str(), attempts, err.as_deref()),
            ("pending", MAX_ATTEMPTS + 1, Some("SIGNING_KEY unset"))
        );
        let next: NaiveDateTime = sqlx::query_scalar(
            "SELECT next_attempt_at FROM social.notification_deliveries WHERE notification_id = $1",
        )
        .bind(rows[0].id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert!(
            next > now_utc() + chrono::Duration::seconds(600),
            "backed off"
        );

        *recorder.fail_with.lock() = None;
        sqlx::query("UPDATE social.notification_deliveries SET next_attempt_at = $2 WHERE notification_id = $1")
            .bind(rows[0].id)
            .bind(now_utc() - chrono::Duration::seconds(1))
            .execute(&pool)
            .await
            .unwrap();
        run_pass(&pool, &recorder, true).await.unwrap();
        assert_eq!(delivery_state(&pool, rows[0].id).await.unwrap().0, "sent");
    }

    /// Prune and claim racing on the same table: prune takes only terminal
    /// rows, a claimed (pending) row survives however old, and neither blocks.
    #[tokio::test]
    async fn prune_alongside_a_claim_never_deletes_a_pending_row() {
        let Some(pool) = private_pool("dlv-race").await else {
            return;
        };
        let old = now_utc() - chrono::Duration::days(40);
        let mut terminal = Vec::new();
        let mut pending = Vec::new();
        for i in 0..20 {
            let rows = pending_for(&pool, &fresh_address()).await;
            let state = if i % 2 == 0 { "sent" } else { "pending" };
            sqlx::query("UPDATE social.notification_deliveries SET state = $2, updated_at = $3, next_attempt_at = $3 WHERE notification_id = $1")
                .bind(rows[0].id)
                .bind(state)
                .bind(old)
                .execute(&pool)
                .await
                .unwrap();
            if state == "sent" {
                terminal.push(rows[0].id);
            } else {
                pending.push(rows[0].id);
            }
        }
        let now = now_utc();
        let (claimed, pruned) = tokio::join!(claim(&pool, now, BATCH), prune(&pool, now));
        let claimed: std::collections::HashSet<Uuid> = claimed
            .unwrap()
            .into_iter()
            .map(|c| c.notification_id)
            .collect();
        pruned.unwrap();
        for id in &pending {
            assert!(claimed.contains(id), "every due pending row is claimed");
            let (state, attempts, _) = delivery_state(&pool, *id).await.expect("never pruned");
            assert_eq!((state.as_str(), attempts), ("pending", 1));
        }
        assert!(terminal.iter().all(|id| !claimed.contains(id)));
        prune(&pool, now).await.unwrap();
        for id in &terminal {
            assert_eq!(delivery_state(&pool, *id).await, None);
        }
        for id in &pending {
            assert!(delivery_state(&pool, *id).await.is_some());
        }
    }
}
