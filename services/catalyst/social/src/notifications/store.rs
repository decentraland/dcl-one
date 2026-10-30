//! The SQL of the notifications routes and producers over `social.*`.
//! `occurred_at` is `timestamp(3)` (the wire's epoch-ms `timestamp`/`from`,
//! converted here); `read`/`read_at` are the in-app delivery state; every
//! insert writes its `email` delivery row in the same statement
//! (`pending` or `skipped` with the reason), which the outbox worker in
//! `delivery.rs` drains. The time is always a bind parameter.

use crate::shared::now_utc;
use std::sync::Arc;
use std::time::Instant;

use chrono::{DateTime, NaiveDateTime, Utc};
use serde_json::Value as Json;
use sqlx::PgPool;
use uuid::Uuid;

use super::email::{EmailSource, make_code};
use super::errors::ApiError;
use super::seen::SeenDebounce;
use super::types::{NotificationItem, Subscription, normalize_details};
use crate::identity::Address;

pub fn ms_to_naive(ms: i64) -> Option<NaiveDateTime> {
    DateTime::<Utc>::from_timestamp_millis(ms).map(|t| t.naive_utc())
}

pub fn naive_to_ms(t: NaiveDateTime) -> i64 {
    t.and_utc().timestamp_millis()
}

/// The list statement for one flag combination. The `from` bound and the
/// unread filter are spelled only when set, never folded as `($2 IS NULL OR
/// occurred_at > $2)` / `($3 = FALSE OR read = FALSE)`: once PostgreSQL
/// switched the prepared statement to a generic plan those folds left the
/// index condition and the 30-second bell poll with nothing new read the
/// wallet's whole history (METRICS.md, "Stage 5 review: the notifications
/// poll through the plan cache"). With the flags in the text, the generic
/// plan keeps `occurred_at > $2` in the index condition and the unread poll
/// matches the partial `notifications_unread_idx`. Binds: `$1` address,
/// then `from` when present, then the limit, then the touch time when
/// `touch` (the reader_seen upsert rides along as a data-modifying CTE;
/// GREATEST keeps concurrent replicas from moving the mark backwards).
fn list_sql(with_from: bool, only_unread: bool, touch: bool) -> String {
    let mut next = 2;
    let mut bind = || {
        let n = next;
        next += 1;
        format!("${n}")
    };
    let from_cond = if with_from {
        format!(" AND occurred_at > {}", bind())
    } else {
        String::new()
    };
    let unread_cond = if only_unread { " AND NOT read" } else { "" };
    let limit = bind();
    let touch_cte = if touch {
        format!(
            "WITH seen AS (
                INSERT INTO social.notification_reader_seen (address, last_fetch_at)
                VALUES ($1, {}::timestamp)
                ON CONFLICT (address) DO UPDATE
                  SET last_fetch_at = GREATEST(social.notification_reader_seen.last_fetch_at,
                                               EXCLUDED.last_fetch_at)
            ) ",
            bind()
        )
    } else {
        String::new()
    };
    format!(
        "{touch_cte}SELECT id, type, address,
                   (EXTRACT(EPOCH FROM occurred_at) * 1000)::bigint AS timestamp,
                   read,
                   to_char(created_at, 'YYYY-MM-DD\"T\"HH24:MI:SS.MS\"Z\"') AS created_at,
                   to_char(COALESCE(read_at, created_at), 'YYYY-MM-DD\"T\"HH24:MI:SS.MS\"Z\"') AS updated_at,
                   metadata
            FROM social.notifications
            WHERE address = $1{from_cond}{unread_cond}
              AND in_app IS TRUE
            ORDER BY occurred_at DESC, id DESC
            LIMIT {limit}"
    )
}

/// The `email` delivery row of every notification a CTE named `ins`
/// (columns `id, address, type`) inserted: `pending` when the recipient can
/// receive it, else `skipped` with the reason. `rows_affected` of the whole
/// statement is therefore the number of notifications inserted.
const DELIVERY_ROWS_SQL: &str = r#"
    INSERT INTO social.notification_deliveries (notification_id, channel, state, last_error)
    SELECT n.id, 'email',
           CASE WHEN d.reason IS NULL THEN 'pending' ELSE 'skipped' END,
           d.reason
    FROM ins n
    LEFT JOIN social.subscriptions s ON s.address = n.address
    CROSS JOIN LATERAL (
        SELECT CASE
            WHEN s.email IS NULL THEN 'no confirmed email'
            WHEN COALESCE((s.details->>'ignore_all_email')::boolean, false) THEN 'ignore_all_email'
            WHEN NOT COALESCE((s.details->'message_type'->n.type->>'email')::boolean, true) THEN 'opted out of this type'
        END AS reason
    ) d
    ON CONFLICT (notification_id, channel) DO NOTHING
"#;

type ListRow = (Uuid, String, String, i64, bool, String, String, Json);

/// One notification a producer wants delivered. `id` is the producer's
/// idempotency key (a v5 uuid over the event for replayable sources).
#[derive(Debug, Clone)]
pub struct NewNotification {
    pub id: Uuid,
    pub address: Address,
    pub kind: String,
    pub metadata: Json,
    pub occurred_at: NaiveDateTime,
}

impl NewNotification {
    pub fn now(address: Address, kind: &str, metadata: Json) -> Self {
        Self {
            id: Uuid::new_v4(),
            address,
            kind: kind.to_string(),
            metadata,
            occurred_at: now_utc(),
        }
    }
}

/// Inserts the batch (`ON CONFLICT (id) DO NOTHING`) with its delivery
/// rows in one statement; returns how many were new.
pub async fn insert_notifications<'e, E>(
    executor: E,
    batch: &[NewNotification],
) -> Result<u64, sqlx::Error>
where
    E: sqlx::Executor<'e, Database = sqlx::Postgres>,
{
    if batch.is_empty() {
        return Ok(0);
    }
    let ids: Vec<Uuid> = batch.iter().map(|n| n.id).collect();
    let addresses: Vec<&str> = batch.iter().map(|n| n.address.as_str()).collect();
    let kinds: Vec<&str> = batch.iter().map(|n| n.kind.as_str()).collect();
    let metadata: Vec<Json> = batch.iter().map(|n| n.metadata.clone()).collect();
    let occurred: Vec<NaiveDateTime> = batch.iter().map(|n| n.occurred_at).collect();
    let sql = format!(
        r#"
        WITH ins AS (
            INSERT INTO social.notifications (id, address, type, metadata, occurred_at, in_app)
            SELECT n.*, NOT COALESCE((s.details->>'ignore_all_in_app')::boolean, false)
                        AND COALESCE((s.details->'message_type'->n.type->>'in_app')::boolean, true)
            FROM UNNEST($1::uuid[], $2::text[], $3::text[], $4::jsonb[], $5::timestamp[])
                AS n(id,address,type,metadata,occurred_at)
            LEFT JOIN social.subscriptions s ON s.address = n.address
            ON CONFLICT (id) DO NOTHING
            RETURNING id, address, type
        )
        {DELIVERY_ROWS_SQL}
        "#
    );
    let res = sqlx::query(sqlx::AssertSqlSafe(sql))
        .bind(&ids)
        .bind(&addresses)
        .bind(&kinds)
        .bind(&metadata)
        .bind(&occurred)
        .execute(executor)
        .await?;
    Ok(res.rows_affected())
}

#[derive(Debug)]
pub enum SetEmailOutcome {
    NoEmailSent,
    SendConfirmation { source: EmailSource, code: String },
}

#[derive(Clone)]
pub struct NotificationsStore {
    pool: PgPool,
    seen: Arc<SeenDebounce>,
}

impl NotificationsStore {
    pub fn new(pool: PgPool) -> Self {
        Self {
            pool,
            seen: Arc::new(SeenDebounce::default()),
        }
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    /// Lists and, at most once per address per `SEEN_WINDOW`, records the fetch in
    /// `notification_reader_seen` inside the same statement. The touch stays
    /// non-fatal: if the combined form fails the plain list runs once.
    pub async fn list_and_touch(
        &self,
        address: &Address,
        limit: i64,
        from_ms: Option<i64>,
        only_unread: bool,
    ) -> Result<Vec<NotificationItem>, ApiError> {
        if self.seen.claim(address.as_str(), Instant::now()) {
            match self
                .fetch_list(address, limit, from_ms, only_unread, Some(now_utc()))
                .await
            {
                Ok(items) => return Ok(items),
                Err(err) => {
                    self.seen.release(address.as_str());
                    tracing::warn!(error = %err, "reader_seen touch failed; listing without it");
                }
            }
        }
        self.list(address, limit, from_ms, only_unread).await
    }

    pub async fn list(
        &self,
        address: &Address,
        limit: i64,
        from_ms: Option<i64>,
        only_unread: bool,
    ) -> Result<Vec<NotificationItem>, ApiError> {
        self.fetch_list(address, limit, from_ms, only_unread, None)
            .await
            .map_err(Into::into)
    }

    async fn fetch_list(
        &self,
        address: &Address,
        limit: i64,
        from_ms: Option<i64>,
        only_unread: bool,
        touch_at: Option<NaiveDateTime>,
    ) -> Result<Vec<NotificationItem>, sqlx::Error> {
        let from = from_ms.and_then(ms_to_naive);
        let sql = list_sql(from.is_some(), only_unread, touch_at.is_some());
        let mut query = sqlx::query_as::<_, ListRow>(sqlx::AssertSqlSafe(sql)).bind(address);
        if let Some(from) = from {
            query = query.bind(from);
        }
        query = query.bind(limit);
        if let Some(at) = touch_at {
            query = query.bind(at);
        }
        let rows = query.fetch_all(&self.pool).await?;
        Ok(rows
            .into_iter()
            .map(
                |(id, kind, address, timestamp, read, created_at, updated_at, metadata)| {
                    NotificationItem {
                        id,
                        kind,
                        address,
                        timestamp,
                        read,
                        created_at,
                        updated_at,
                        metadata,
                    }
                },
            )
            .collect())
    }

    /// One row per recipient (every subscription, or the explicit list),
    /// idempotent per `(address, broadcast_key)`; returns the new rows.
    pub async fn broadcast(
        &self,
        broadcast_key: &str,
        kind: &str,
        metadata: &Json,
        addresses: Option<&[Address]>,
    ) -> Result<u64, ApiError> {
        let now = now_utc();
        let Some(list) = addresses else {
            let sql = format!(
                r#"
                WITH ins AS (
                    INSERT INTO social.notifications (id, address, type, metadata, broadcast_key, occurred_at)
                    SELECT gen_random_uuid(), address, $1, $2, $3, $4 FROM social.subscriptions
                    ON CONFLICT (address, broadcast_key) WHERE broadcast_key IS NOT NULL DO NOTHING
                    RETURNING id, address, type
                )
                {DELIVERY_ROWS_SQL}
                "#
            );
            let res = sqlx::query(sqlx::AssertSqlSafe(sql))
                .bind(kind)
                .bind(metadata)
                .bind(broadcast_key)
                .bind(now)
                .execute(&self.pool)
                .await?;
            return Ok(res.rows_affected());
        };
        if list.is_empty() {
            return Ok(0);
        }
        let ids: Vec<Uuid> = (0..list.len()).map(|_| Uuid::new_v4()).collect();
        let targets: Vec<&str> = list.iter().map(Address::as_str).collect();
        let sql = format!(
            r#"
            WITH ins AS (
                INSERT INTO social.notifications (id, address, type, metadata, broadcast_key, occurred_at)
                SELECT u.id, u.address, $3, $4, $5, $6 FROM UNNEST($1::uuid[], $2::text[]) AS u(id, address)
                ON CONFLICT (address, broadcast_key) WHERE broadcast_key IS NOT NULL DO NOTHING
                RETURNING id, address, type
            )
            {DELIVERY_ROWS_SQL}
            "#
        );
        let res = sqlx::query(sqlx::AssertSqlSafe(sql))
            .bind(&ids)
            .bind(&targets)
            .bind(kind)
            .bind(metadata)
            .bind(broadcast_key)
            .bind(now)
            .execute(&self.pool)
            .await?;
        Ok(res.rows_affected())
    }

    pub async fn mark_read(&self, address: &Address, ids: &[Uuid]) -> Result<u64, ApiError> {
        let res = sqlx::query(
            r#"
            UPDATE social.notifications
            SET read = TRUE, read_at = $3
            WHERE address = $1 AND id = ANY($2) AND read = FALSE
            "#,
        )
        .bind(address)
        .bind(ids)
        .bind(now_utc())
        .execute(&self.pool)
        .await?;
        Ok(res.rows_affected())
    }

    pub async fn get_subscription(
        &self,
        address: &Address,
    ) -> Result<Option<Subscription>, ApiError> {
        let row = sqlx::query_as::<_, (String, Option<String>, Option<String>, Json)>(
            r#"
            SELECT s.address, s.email, u.email, s.details
            FROM social.subscriptions s
            LEFT JOIN social.unconfirmed_emails u ON u.address = s.address
            WHERE s.address = $1
            "#,
        )
        .bind(address)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(
            |(address, email, unconfirmed_email, details)| Subscription {
                address,
                email,
                unconfirmed_email,
                details: normalize_details(&details),
            },
        ))
    }

    pub async fn put_subscription_details(
        &self,
        address: &Address,
        details: &Json,
    ) -> Result<Subscription, ApiError> {
        let row = sqlx::query_as::<_, (String, Option<String>, Option<String>, Json)>(
            r#"
            WITH up AS (
                INSERT INTO social.subscriptions (address, details, updated_at)
                VALUES ($1, $2, $3)
                ON CONFLICT (address) DO UPDATE
                  SET details = EXCLUDED.details, updated_at = EXCLUDED.updated_at
                RETURNING address, email, details
            )
            SELECT up.address, up.email, u.email, up.details
            FROM up LEFT JOIN social.unconfirmed_emails u ON u.address = up.address
            "#,
        )
        .bind(address)
        .bind(details)
        .bind(now_utc())
        .fetch_one(&self.pool)
        .await?;
        Ok(Subscription {
            address: row.0,
            email: row.1,
            unconfirmed_email: row.2,
            details: normalize_details(&row.3),
        })
    }

    pub async fn set_email(
        &self,
        address: &Address,
        email: &str,
        is_credits_workflow: bool,
    ) -> Result<SetEmailOutcome, ApiError> {
        let email = email.trim();
        let now = now_utc();

        if email.is_empty() {
            sqlx::query(
                r#"
                WITH cleared AS (
                    DELETE FROM social.unconfirmed_emails WHERE address = $1
                )
                INSERT INTO social.subscriptions (address, email, details, updated_at)
                VALUES ($1, NULL, jsonb_build_object('ignore_all_email', true), $2)
                ON CONFLICT (address) DO UPDATE
                  SET email = NULL,
                      details = jsonb_set(
                          COALESCE(social.subscriptions.details, '{}'::jsonb),
                          '{ignore_all_email}', 'true'::jsonb, true),
                      updated_at = EXCLUDED.updated_at
                "#,
            )
            .bind(address)
            .bind(now)
            .execute(&self.pool)
            .await?;
            return Ok(SetEmailOutcome::NoEmailSent);
        }

        let email_lc = email.to_lowercase();
        let (current_email, taken_by_other): (Option<String>, bool) = sqlx::query_as(
            r#"
            SELECT (SELECT email FROM social.subscriptions WHERE address = $2),
                   EXISTS (SELECT 1 FROM social.subscriptions
                           WHERE lower(email) = $1 AND address <> $2)
            "#,
        )
        .bind(&email_lc)
        .bind(address)
        .fetch_one(&self.pool)
        .await?;

        if current_email
            .as_deref()
            .map(|e| e.eq_ignore_ascii_case(email))
            .unwrap_or(false)
        {
            return Ok(SetEmailOutcome::NoEmailSent);
        }
        if taken_by_other {
            return Err(ApiError::bad_request("Email already registered"));
        }

        let source = EmailSource::from_credits_workflow(is_credits_workflow);
        let code = make_code();
        sqlx::query(
            r#"
            WITH pending AS (
                INSERT INTO social.unconfirmed_emails (address, email, code, source, created_at)
                VALUES ($1, $2, $3, $4, $6)
                ON CONFLICT (address) DO UPDATE
                  SET email = EXCLUDED.email,
                      code = EXCLUDED.code,
                      source = EXCLUDED.source,
                      created_at = EXCLUDED.created_at
            )
            INSERT INTO social.subscriptions (address, is_credits_workflow, details, updated_at)
            VALUES ($1, $5, '{}'::jsonb, $6)
            ON CONFLICT (address) DO UPDATE
              SET is_credits_workflow = EXCLUDED.is_credits_workflow,
                  updated_at = EXCLUDED.updated_at
            "#,
        )
        .bind(address)
        .bind(email)
        .bind(&code)
        .bind(source.as_str())
        .bind(is_credits_workflow)
        .bind(now)
        .execute(&self.pool)
        .await?;

        Ok(SetEmailOutcome::SendConfirmation { source, code })
    }

    /// Promotes the pending email when the code matches, in one statement;
    /// a wrong code consumes nothing.
    pub async fn confirm_email(
        &self,
        address: &Address,
        code: &str,
    ) -> Result<Option<EmailSource>, ApiError> {
        let row: Option<(String,)> = sqlx::query_as(
            r#"
            WITH claimed AS (
                DELETE FROM social.unconfirmed_emails
                WHERE address = $1 AND code = $2
                RETURNING email, source
            ), promoted AS (
                INSERT INTO social.subscriptions (address, email, details, updated_at)
                SELECT $1, email, '{}'::jsonb, $3 FROM claimed
                ON CONFLICT (address) DO UPDATE
                  SET email = EXCLUDED.email,
                      updated_at = EXCLUDED.updated_at
            )
            SELECT source FROM claimed
            "#,
        )
        .bind(address)
        .bind(code)
        .bind(now_utc())
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(|(source,)| EmailSource::parse(&source)))
    }

    pub async fn is_opted_out(
        &self,
        address: &Address,
        scope: &str,
        scope_id: &str,
    ) -> Result<bool, ApiError> {
        let exists = sqlx::query_scalar::<_, bool>(
            r#"
            SELECT EXISTS (
              SELECT 1 FROM social.subscription_opt_outs
              WHERE address = $1 AND scope = $2 AND scope_id = $3
            )
            "#,
        )
        .bind(address)
        .bind(scope)
        .bind(scope_id)
        .fetch_one(&self.pool)
        .await?;
        Ok(exists)
    }

    pub async fn create_opt_out(
        &self,
        address: &Address,
        scope: &str,
        scope_id: &str,
    ) -> Result<(), ApiError> {
        sqlx::query(
            r#"
            INSERT INTO social.subscription_opt_outs (address, scope, scope_id, created_at)
            VALUES ($1, $2, $3, $4)
            ON CONFLICT (address, scope, scope_id) DO NOTHING
            "#,
        )
        .bind(address)
        .bind(scope)
        .bind(scope_id)
        .bind(now_utc())
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn delete_opt_out(
        &self,
        address: &Address,
        scope: &str,
        scope_id: &str,
    ) -> Result<bool, ApiError> {
        let res = sqlx::query(
            r#"
            DELETE FROM social.subscription_opt_outs
            WHERE address = $1 AND scope = $2 AND scope_id = $3
            "#,
        )
        .bind(address)
        .bind(scope)
        .bind(scope_id)
        .execute(&self.pool)
        .await?;
        Ok(res.rows_affected() > 0)
    }
}

#[cfg(test)]
mod sql_shape_tests {
    use super::list_sql;

    /// The flags are in the statement text, the values are binds: no
    /// `IS NULL OR` fold the generic plan would have to filter after the
    /// index (the review's 3,166-buffer poll).
    #[test]
    fn the_list_statement_spells_only_the_set_flags_and_numbers_its_binds() {
        let plain = list_sql(false, false, false);
        assert!(
            plain.contains("WHERE address = $1\n") && plain.contains("LIMIT $2"),
            "{plain}"
        );
        assert!(!plain.contains("IS NULL OR") && !plain.contains("NOT read"));
        let from = list_sql(true, false, false);
        assert!(
            from.contains("WHERE address = $1 AND occurred_at > $2") && from.contains("LIMIT $3"),
            "{from}"
        );
        let unread = list_sql(false, true, false);
        assert!(
            unread.contains("WHERE address = $1 AND NOT read") && unread.contains("LIMIT $2"),
            "{unread}"
        );
        let both = list_sql(true, true, true);
        assert!(both.starts_with("WITH seen AS"), "{both}");
        assert!(both.contains("VALUES ($1, $4::timestamp)"), "{both}");
        assert!(
            both.contains("WHERE address = $1 AND occurred_at > $2 AND NOT read")
                && both.contains("LIMIT $3"),
            "{both}"
        );
    }
}

#[cfg(test)]
pub(crate) mod test_support {
    use super::*;

    /// A fresh lowercase wallet address per call: the lib tests share one
    /// scratch database per process, so every row is scoped by it.
    pub fn fresh_address() -> Address {
        let hex = format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple());
        Address::parse(&format!("0x{}", &hex[..40])).unwrap()
    }

    pub async fn store() -> Option<NotificationsStore> {
        let pool = crate::db::scratch::pool(4).await?;
        Some(NotificationsStore::new(pool))
    }

    /// A database of this test's own (`catalyst_test_lib_<pid>_<tag>_<rand>`,
    /// swept with the process's scratch database by the next run) for the
    /// tests whose subject is a whole-table pass: the outbox claim takes
    /// every due row it can see, so two such tests on one database race.
    pub async fn private_pool(tag: &str) -> Option<PgPool> {
        use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
        let base = std::env::var("CATALYST_TEST_DATABASE_URL").ok()?;
        let opts: PgConnectOptions = base
            .parse()
            .expect("CATALYST_TEST_DATABASE_URL must be a postgres DSN");
        let admin = PgPoolOptions::new()
            .max_connections(1)
            .connect_with(opts.clone().database("postgres"))
            .await
            .expect("admin connect");
        let name = format!(
            "catalyst_test_lib_{}_{tag}_{}",
            std::process::id(),
            &Uuid::new_v4().simple().to_string()[..8]
        );
        sqlx::query(sqlx::AssertSqlSafe(format!("CREATE DATABASE \"{name}\"")))
            .execute(&admin)
            .await
            .expect("create database");
        let mut url = url::Url::parse(&base).expect("URL");
        url.set_path(&format!("/{name}"));
        let pool = crate::db::connect_pool(
            url.as_str(),
            &crate::db::PoolSettings {
                max_connections: 4,
                ..Default::default()
            },
        )
        .await
        .expect("connect private database");
        crate::db::apply_schema(&pool).await.expect("apply schema");
        Some(pool)
    }

    pub async fn seed_notification(
        pool: &PgPool,
        address: &Address,
        occurred_ms: i64,
        read: bool,
    ) -> Uuid {
        let id = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO social.notifications (id, address, type, occurred_at, read, read_at) VALUES ($1, $2, 'bid_received', $3, $4, CASE WHEN $4 THEN $3 END)",
        )
        .bind(id)
        .bind(address)
        .bind(ms_to_naive(occurred_ms).unwrap())
        .bind(read)
        .execute(pool)
        .await
        .unwrap();
        id
    }

    pub async fn last_fetch_at_ms(pool: &PgPool, address: &Address) -> Option<i64> {
        sqlx::query_scalar::<_, NaiveDateTime>(
            "SELECT last_fetch_at FROM social.notification_reader_seen WHERE address = $1",
        )
        .bind(address)
        .fetch_optional(pool)
        .await
        .unwrap()
        .map(naive_to_ms)
    }

    pub async fn delivery_state(pool: &PgPool, id: Uuid) -> Option<(String, i32, Option<String>)> {
        sqlx::query_as("SELECT state, attempts, last_error FROM social.notification_deliveries WHERE notification_id = $1 AND channel = 'email'")
            .bind(id)
            .fetch_optional(pool)
            .await
            .unwrap()
    }
}

#[cfg(test)]
mod pg_tests {
    use super::test_support::*;
    use super::*;

    #[tokio::test]
    async fn list_and_touch_records_the_fetch_once_per_window() {
        let Some(c) = store().await else { return };
        let pool = c.pool().clone();
        let (a, b) = (fresh_address(), fresh_address());
        let a_old = seed_notification(&pool, &a, 100_000, true).await;
        let a_new = seed_notification(&pool, &a, 200_000, false).await;
        seed_notification(&pool, &b, 300_000, false).await;

        let before = Utc::now().timestamp_millis();
        let items = c.list_and_touch(&a, 50, None, false).await.unwrap();
        assert_eq!(
            items.iter().map(|i| i.id).collect::<Vec<_>>(),
            vec![a_new, a_old]
        );
        assert_eq!(items[0].timestamp, 200_000);
        assert_eq!(items[0].address, a.as_str());
        assert!(
            items[0].created_at.ends_with('Z') && items[0].created_at.len() == 24,
            "{}",
            items[0].created_at
        );
        assert!(items[1].read);
        assert_eq!(
            items[1].updated_at, "1970-01-01T00:01:40.000Z",
            "updated_at is read_at when read"
        );
        let first = last_fetch_at_ms(&pool, &a).await.expect("touched");
        assert!(first >= before - 1, "{first} vs {before}");
        assert_eq!(last_fetch_at_ms(&pool, &b).await, None);

        sqlx::query("UPDATE social.notification_reader_seen SET last_fetch_at = to_timestamp(0.001) WHERE address = $1")
            .bind(&a)
            .execute(&pool)
            .await
            .unwrap();
        let items = c.list_and_touch(&a, 50, Some(150_000), true).await.unwrap();
        assert_eq!(items.iter().map(|i| i.id).collect::<Vec<_>>(), vec![a_new]);
        assert_eq!(
            last_fetch_at_ms(&pool, &a).await,
            Some(1),
            "second poll inside the window must not touch"
        );

        let plain = c.list(&a, 50, Some(150_000), true).await.unwrap();
        let touch_at = ms_to_naive(before).unwrap();
        let combined = c
            .fetch_list(&a, 50, Some(150_000), true, Some(touch_at))
            .await
            .unwrap();
        let shape = |v: &Vec<NotificationItem>| {
            v.iter()
                .map(|i| {
                    (
                        i.id,
                        i.timestamp,
                        i.read,
                        i.created_at.clone(),
                        i.updated_at.clone(),
                    )
                })
                .collect::<Vec<_>>()
        };
        assert_eq!(shape(&plain), shape(&combined));
        assert_eq!(last_fetch_at_ms(&pool, &a).await, Some(before));

        let future = before + 10_000_000;
        sqlx::query(
            "UPDATE social.notification_reader_seen SET last_fetch_at = $2 WHERE address = $1",
        )
        .bind(&a)
        .bind(ms_to_naive(future).unwrap())
        .execute(&pool)
        .await
        .unwrap();
        NotificationsStore::new(pool.clone())
            .list_and_touch(&a, 50, None, false)
            .await
            .unwrap();
        assert_eq!(
            last_fetch_at_ms(&pool, &a).await,
            Some(future),
            "GREATEST never moves the mark back"
        );
    }

    #[tokio::test]
    async fn mark_read_is_scoped_to_the_signer_and_idempotent() {
        let Some(c) = store().await else { return };
        let pool = c.pool().clone();
        let (a, b) = (fresh_address(), fresh_address());
        let mine = seed_notification(&pool, &a, 1_000, false).await;
        let theirs = seed_notification(&pool, &b, 1_000, false).await;
        assert_eq!(c.mark_read(&a, &[mine, theirs]).await.unwrap(), 1);
        assert_eq!(c.mark_read(&a, &[mine, theirs]).await.unwrap(), 0);
        let listed = c.list(&b, 10, None, true).await.unwrap();
        assert_eq!(listed.len(), 1, "the other wallet's row stays unread");
        let (read, read_at): (bool, Option<NaiveDateTime>) =
            sqlx::query_as("SELECT read, read_at FROM social.notifications WHERE id = $1")
                .bind(mine)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert!(read && read_at.is_some());
    }

    #[tokio::test]
    async fn confirm_email_promotes_and_clears_in_one_statement() {
        let Some(c) = store().await else { return };
        let pool = c.pool().clone();
        let a = fresh_address();
        sqlx::query("INSERT INTO social.unconfirmed_emails (address, email, code, source) VALUES ($1, 'Me@Example.com', 'right-code', 'credits')")
            .bind(&a)
            .execute(&pool)
            .await
            .unwrap();

        assert_eq!(c.confirm_email(&a, "wrong-code").await.unwrap(), None);
        let pending: i64 =
            sqlx::query_scalar("SELECT count(*) FROM social.unconfirmed_emails WHERE address = $1")
                .bind(&a)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(pending, 1, "a wrong code must not consume the pending row");
        assert!(c.get_subscription(&a).await.unwrap().is_none());

        assert_eq!(
            c.confirm_email(&a, "right-code").await.unwrap(),
            Some(EmailSource::Credits)
        );
        let sub = c.get_subscription(&a).await.unwrap().expect("promoted");
        assert_eq!(sub.email.as_deref(), Some("Me@Example.com"));
        assert_eq!(sub.unconfirmed_email, None);
        let pending: i64 =
            sqlx::query_scalar("SELECT count(*) FROM social.unconfirmed_emails WHERE address = $1")
                .bind(&a)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(pending, 0);
        assert_eq!(c.confirm_email(&a, "right-code").await.unwrap(), None);
    }

    #[tokio::test]
    async fn set_email_flow_keeps_its_outcomes() {
        let Some(c) = store().await else { return };
        let pool = c.pool().clone();
        let (a, b) = (fresh_address(), fresh_address());
        let unique = format!("Foo-{}@Example.com", Uuid::new_v4().simple());

        let SetEmailOutcome::SendConfirmation { source, code } =
            c.set_email(&a, &format!(" {unique} "), true).await.unwrap()
        else {
            panic!("a new email must send a confirmation")
        };
        assert_eq!(source, EmailSource::Credits);
        let (pending_email, pending_code, pending_source): (String, String, String) =
            sqlx::query_as(
                "SELECT email, code, source FROM social.unconfirmed_emails WHERE address = $1",
            )
            .bind(&a)
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(
            (
                pending_email.as_str(),
                pending_code.as_str(),
                pending_source.as_str()
            ),
            (unique.as_str(), code.as_str(), "credits")
        );
        let sub = c
            .get_subscription(&a)
            .await
            .unwrap()
            .expect("the subscription row exists while pending");
        assert_eq!(
            (sub.email.as_deref(), sub.unconfirmed_email.as_deref()),
            (None, Some(unique.as_str()))
        );
        let credits: bool = sqlx::query_scalar(
            "SELECT is_credits_workflow FROM social.subscriptions WHERE address = $1",
        )
        .bind(&a)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert!(credits);

        assert_eq!(
            c.confirm_email(&a, &code).await.unwrap(),
            Some(EmailSource::Credits)
        );
        assert!(matches!(
            c.set_email(&a, &unique.to_lowercase(), false)
                .await
                .unwrap(),
            SetEmailOutcome::NoEmailSent
        ));
        let err = c
            .set_email(&b, &unique.to_uppercase(), false)
            .await
            .expect_err("taken by another address");
        assert_eq!(err.status(), 400);
        assert!(
            c.get_subscription(&b).await.unwrap().is_none(),
            "a refused set-email writes nothing"
        );

        assert!(matches!(
            c.set_email(&a, "", false).await.unwrap(),
            SetEmailOutcome::NoEmailSent
        ));
        let sub = c.get_subscription(&a).await.unwrap().unwrap();
        assert_eq!(sub.email, None);
        assert_eq!(sub.details["ignore_all_email"], serde_json::json!(true));
        let pending: i64 =
            sqlx::query_scalar("SELECT count(*) FROM social.unconfirmed_emails WHERE address = $1")
                .bind(&a)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(pending, 0);
    }

    #[tokio::test]
    async fn broadcast_is_idempotent_per_recipient_and_writes_delivery_rows() {
        let Some(c) = store().await else { return };
        let pool = c.pool().clone();
        let (a, b) = (fresh_address(), fresh_address());
        let key_all = format!("bc-all-{}", Uuid::new_v4());
        for addr in [&a, &b] {
            sqlx::query("INSERT INTO social.subscriptions (address) VALUES ($1)")
                .bind(addr)
                .execute(&pool)
                .await
                .unwrap();
        }
        sqlx::query("UPDATE social.subscriptions SET email = 'a@example.com' WHERE address = $1")
            .bind(&a)
            .execute(&pool)
            .await
            .unwrap();
        let meta = serde_json::json!({"title": "hi"});
        let inserted = c
            .broadcast(&key_all, "bid_received", &meta, None)
            .await
            .unwrap();
        assert!(inserted >= 2, "every subscription gets one row: {inserted}");
        type Row = (String, String, Json, Option<String>, String, Option<String>);
        let rows: Vec<Row> = sqlx::query_as(
            "SELECT n.address, n.type, n.metadata, n.broadcast_key, d.state, d.last_error
             FROM social.notifications n JOIN social.notification_deliveries d ON d.notification_id = n.id
             WHERE n.broadcast_key = $1 AND n.address = ANY($2) ORDER BY n.address",
        )
        .bind(&key_all)
        .bind(vec![a.as_str(), b.as_str()])
        .fetch_all(&pool)
        .await
        .unwrap();
        assert_eq!(rows.len(), 2);
        let by_addr: std::collections::HashMap<&str, &Row> =
            rows.iter().map(|r| (r.0.as_str(), r)).collect();
        assert!(rows.iter().all(|r| r.1 == "bid_received"
            && r.2 == meta
            && r.3.as_deref() == Some(key_all.as_str())));
        assert_eq!(
            (
                by_addr[a.as_str()].4.as_str(),
                by_addr[a.as_str()].5.as_deref()
            ),
            ("pending", None),
            "a confirmed email is deliverable"
        );
        assert_eq!(
            (
                by_addr[b.as_str()].4.as_str(),
                by_addr[b.as_str()].5.as_deref()
            ),
            ("skipped", Some("no confirmed email"))
        );

        // The all-subscribers re-run is asserted on this test's own recipients:
        // the lib tests share one scratch database, so a sibling test can add
        // a subscription between the two calls and the re-run legitimately
        // inserts one row for that new subscriber.
        c.broadcast(&key_all, "bid_received", &meta, None)
            .await
            .unwrap();
        let again: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM social.notifications n JOIN social.notification_deliveries d ON d.notification_id = n.id
             WHERE n.broadcast_key = $1 AND n.address = ANY($2)",
        )
        .bind(&key_all)
        .bind(vec![a.as_str(), b.as_str()])
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(
            again, 2,
            "a re-run broadcast inserts nothing for a recipient it already reached"
        );

        let key_some = format!("bc-some-{}", Uuid::new_v4());
        assert_eq!(
            c.broadcast(
                &key_some,
                "bid_received",
                &meta,
                Some(&[a.clone(), a.clone()])
            )
            .await
            .unwrap(),
            1
        );
        assert_eq!(
            c.broadcast(
                &key_some,
                "bid_received",
                &meta,
                Some(std::slice::from_ref(&a))
            )
            .await
            .unwrap(),
            0
        );
        let listed = c.list(&a, 50, None, false).await.unwrap();
        assert_eq!(listed.len(), 2);
    }

    #[tokio::test]
    async fn producer_inserts_honour_the_subscription_flags() {
        let Some(c) = store().await else { return };
        let pool = c.pool().clone();
        let (opted_out, ignoring, fine, nobody) = (
            fresh_address(),
            fresh_address(),
            fresh_address(),
            fresh_address(),
        );
        sqlx::query("INSERT INTO social.subscriptions (address, email, details) VALUES ($1, 'o@example.com', '{\"message_type\":{\"bid_received\":{\"email\":false,\"in_app\":true}}}'::jsonb)")
            .bind(&opted_out)
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO social.subscriptions (address, email, details) VALUES ($1, 'i@example.com', '{\"ignore_all_email\":true}'::jsonb)")
            .bind(&ignoring)
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query(
            "INSERT INTO social.subscriptions (address, email) VALUES ($1, 'f@example.com')",
        )
        .bind(&fine)
        .execute(&pool)
        .await
        .unwrap();
        let batch: Vec<NewNotification> = [&opted_out, &ignoring, &fine, &nobody]
            .into_iter()
            .map(|a| NewNotification::now(a.clone(), "bid_received", serde_json::json!({})))
            .collect();
        assert_eq!(insert_notifications(&pool, &batch).await.unwrap(), 4);
        assert_eq!(
            insert_notifications(&pool, &batch).await.unwrap(),
            0,
            "the id is the idempotency key"
        );
        let state = |i: usize| delivery_state(&pool, batch[i].id);
        assert_eq!(
            state(0).await.unwrap(),
            ("skipped".into(), 0, Some("opted out of this type".into()))
        );
        assert_eq!(
            state(1).await.unwrap(),
            ("skipped".into(), 0, Some("ignore_all_email".into()))
        );
        assert_eq!(state(2).await.unwrap(), ("pending".into(), 0, None));
        assert_eq!(
            state(3).await.unwrap(),
            ("skipped".into(), 0, Some("no confirmed email".into()))
        );
    }

    #[tokio::test]
    async fn opt_outs_round_trip() {
        let Some(c) = store().await else { return };
        let a = fresh_address();
        let community = Uuid::new_v4().to_string();
        assert!(!c.is_opted_out(&a, "community", &community).await.unwrap());
        c.create_opt_out(&a, "community", &community).await.unwrap();
        c.create_opt_out(&a, "community", &community).await.unwrap();
        assert!(c.is_opted_out(&a, "community", &community).await.unwrap());
        assert!(c.delete_opt_out(&a, "community", &community).await.unwrap());
        assert!(!c.delete_opt_out(&a, "community", &community).await.unwrap());
        assert!(!c.is_opted_out(&a, "community", &community).await.unwrap());
    }
}
