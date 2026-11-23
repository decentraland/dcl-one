//! The write side of `social.notifications` that every producer shares: one
//! notification row plus its `email` delivery row (`pending`, or `skipped` with
//! the reason) in the same statement, so a producer commits its notice inside
//! its own transaction. Social owns the tables, reads them, delivers the email
//! and keeps the templates; it re-exports these items from
//! `social::notifications`.

use std::sync::LazyLock;

use chrono::NaiveDateTime;
use serde_json::Value as Json;
use uuid::Uuid;

use crate::identity::Address;
use crate::now_utc;

/// Calls `$callback!` with every notification type that has an email
/// template, so this list and social's templates are one list.
#[macro_export]
macro_rules! with_emailable_kinds {
    ($callback:ident) => {
        $callback!(
            "ban_lifted",
            "ban_warning",
            "banned",
            "bid_accepted",
            "bid_received",
            "credits_on_demand_granted",
            "credits_reminder_claim_credits",
            "credits_reminder_complete_goals",
            "credits_reminder_do_not_miss_out",
            "credits_reminder_usage",
            "credits_reminder_usage_24_hours",
            "event_approved",
            "event_deleted",
            "event_rejected",
            "events_started",
            "events_starts_soon",
            "governance_announcement",
            "governance_authored_proposal_finished",
            "governance_coauthor_requested",
            "governance_new_comment_on_project_update",
            "governance_new_comment_on_proposal",
            "governance_pitch_passed",
            "governance_proposal_enacted",
            "governance_tender_passed",
            "governance_voting_ended_voter",
            "item_sold",
            "referral_invited_users_accepted",
            "referral_new_tier_reached",
            "rental_ended",
            "rental_started",
            "reward_assignment",
            "reward_campaign_out_of_funds",
            "reward_campaign_out_of_stock",
            "royalties_earned",
            "streaming_key_expired",
            "streaming_key_reset",
            "streaming_key_revoke",
            "streaming_place_updated",
            "streaming_time_exceeded",
            "tip_received",
            "worlds_access_restored",
            "worlds_access_restricted",
            "worlds_missing_resources",
            "worlds_permission_granted",
            "worlds_permission_revoked",
        );
    };
}

/// Why a delivery send did not happen; decides retry vs terminal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SendFailure {
    /// Nothing to send with: no key, sender or template.
    NotConfigured,
    /// No `SIGNING_KEY` for the unsubscribe links: held until one is set.
    NoSigningKey,
    /// Transport or 5xx/429: retry later.
    Transient(String),
    /// The provider refused the message (4xx): retrying cannot help.
    Permanent(String),
}

macro_rules! emailable_consts {
    ($first:literal $(, $rest:literal)* $(,)?) => {
        /// The types with a template: the old `EmailableNotificationTypeEnum`.
        pub const EMAILABLE_TYPES: &[&str] = &[$first $(, $rest)*];
        /// [`EMAILABLE_TYPES`] as a SQL list literal.
        pub const EMAILABLE_SQL_LIST: &str = concat!("'", $first, "'" $(, ",'", $rest, "'")*);
    };
}

with_emailable_kinds!(emailable_consts);

/// `NOTIFICATIONS_CDN_URL`'s default: upstream's `CDN_URL`.
pub const DEFAULT_CDN_URL: &str = "https://assets-cdn.decentraland.org";

/// `path` under the notification icons' CDN `base` (upstream `${CDN_URL}<path>`).
pub fn cdn_asset(base: &str, path: &str) -> String {
    format!(
        "{}/{}",
        base.trim().trim_end_matches('/'),
        path.trim_start_matches('/')
    )
}

/// The `details` flag at `path` (a `->` chain on `s.details`) when it is a JSON
/// boolean, else `default`: a malformed flag never fails the statement.
pub fn details_flag(path: &str, default: bool) -> String {
    format!(
        "COALESCE(CASE WHEN jsonb_typeof(s.details{path}) = 'boolean' THEN (s.details{path})::boolean END, {default})"
    )
}

/// The `email` delivery row of every notification a CTE named `ins`
/// (columns `id, address, type`) inserted: `pending` when the type has a
/// template and the recipient can receive it, else `skipped` with the
/// reason (pruned with the other terminal rows). A type absent from
/// `message_type` emails, as the old `autoMigrate` defaults.
pub static DELIVERY_ROWS_SQL: LazyLock<String> = LazyLock::new(|| {
    format!(
        r#"
    INSERT INTO social.notification_deliveries (notification_id, channel, state, last_error)
    SELECT n.id, 'email',
           CASE WHEN d.reason IS NULL THEN 'pending' ELSE 'skipped' END,
           d.reason
    FROM ins n
    LEFT JOIN social.subscriptions s ON s.address = n.address
    CROSS JOIN LATERAL (
        SELECT CASE
            WHEN n.type NOT IN ({}) THEN 'no email template'
            WHEN s.email IS NULL THEN 'no confirmed email'
            WHEN {} THEN 'ignore_all_email'
            WHEN NOT {} THEN 'opted out of this type'
        END AS reason
    ) d
    ON CONFLICT (notification_id, channel) DO NOTHING
"#,
        EMAILABLE_SQL_LIST,
        details_flag("->'ignore_all_email'", false),
        details_flag("->'message_type'->n.type->'email'", true),
    )
});

/// Follows an `ups` CTE (`id, address, type, inserted`): delivery rows for the
/// new rows only, then the count of new rows.
pub static UPSERT_TAIL: LazyLock<String> = LazyLock::new(|| {
    format!(
        "ins AS (SELECT id, address, type FROM ups WHERE inserted),
        delivered AS ({})
        SELECT count(*) FROM ins",
        *DELIVERY_ROWS_SQL
    )
});

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

/// An exact previous notice identity that can carry a source's stable key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NotificationAlias {
    pub legacy_id: Uuid,
    pub daily_id: Uuid,
    pub kind: &'static str,
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

/// Inserts the batch with its delivery rows in one statement; returns how
/// many were new. A repeated id refreshes metadata and time and keeps the
/// read state; only a new row gets a delivery row, so a repeat never emails.
pub async fn insert_notifications<'e, E>(
    executor: E,
    batch: &[NewNotification],
) -> Result<u64, sqlx::Error>
where
    E: sqlx::Executor<'e, Database = sqlx::Postgres>,
{
    insert_batch(executor, batch, false).await
}

/// Social's event notices only; other producers use [`insert_notifications`].
/// Event delivery opts into in-app preferences without silently changing the
/// established behavior of other producers. Hidden rows are already read so
/// they never enter the bell's partial unread index; email remains independent.
pub async fn insert_event_notifications<'e, E>(
    executor: E,
    batch: &[NewNotification],
) -> Result<u64, sqlx::Error>
where
    E: sqlx::Executor<'e, Database = sqlx::Postgres>,
{
    insert_batch(executor, batch, true).await
}

/// One notification deduplicated per `(address, broadcast_key)` (the
/// source's event key) instead of its id, with its delivery row. A repeat of
/// the same type refreshes metadata and time like [`insert_notifications`].
pub async fn insert_keyed_notification<'e, E>(
    executor: E,
    broadcast_key: &str,
    notification: &NewNotification,
) -> Result<u64, sqlx::Error>
where
    E: sqlx::Executor<'e, Database = sqlx::Postgres>,
{
    let sql = format!(
        r#"
        WITH ups AS (
            INSERT INTO social.notifications AS t (id, address, type, metadata, broadcast_key, occurred_at)
            VALUES ($1, $2, $3, $4, $5, $6)
            ON CONFLICT (address, broadcast_key) WHERE broadcast_key IS NOT NULL DO UPDATE
                SET metadata = EXCLUDED.metadata, occurred_at = EXCLUDED.occurred_at
                WHERE t.type = EXCLUDED.type
            RETURNING id, address, type, (xmax = 0) AS inserted
        ),
        {tail}
        "#,
        tail = *UPSERT_TAIL
    );
    let inserted: i64 = sqlx::query_scalar(sqlx::AssertSqlSafe(sql))
        .bind(notification.id)
        .bind(&notification.address)
        .bind(&notification.kind)
        .bind(&notification.metadata)
        .bind(broadcast_key)
        .bind(notification.occurred_at)
        .fetch_one(executor)
        .await?;
    Ok(inserted as u64)
}

/// Inserts a notice and delivery once per source identity. Exact previous
/// identities keep their row, payload and delivery; only their key is attached.
/// The caller must retain the transaction through the source's own write.
pub async fn insert_keyed_notification_once(
    transaction: &mut sqlx::PgConnection,
    notification: &NewNotification,
    aliases: &[NotificationAlias],
) -> Result<u64, sqlx::Error> {
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1, 0))")
        .bind(format!(
            "notifications.user:{}",
            notification.address.as_str()
        ))
        .execute(&mut *transaction)
        .await?;
    for alias in aliases {
        sqlx::query(
            "UPDATE social.notifications SET broadcast_key=$4
             WHERE id=$1 AND address=$2 AND type=$3 AND broadcast_key IS NULL
               AND NOT EXISTS (
                   SELECT 1 FROM social.notifications
                   WHERE address=$2 AND (broadcast_key=$4 OR id=$5)
               )",
        )
        .bind(alias.legacy_id)
        .bind(&notification.address)
        .bind(alias.kind)
        .bind(format!("user-notification:{}", alias.daily_id))
        .bind(alias.daily_id)
        .execute(&mut *transaction)
        .await?;
    }
    let sql = format!(
        r#"
        WITH ups AS (
            INSERT INTO social.notifications (id, address, type, metadata, broadcast_key, occurred_at)
            VALUES ($1, $2, $3, $4, $5, $6)
            ON CONFLICT DO NOTHING
            RETURNING id, address, type, true AS inserted
        ),
        {tail}
        "#,
        tail = *UPSERT_TAIL
    );
    let inserted: i64 = sqlx::query_scalar(sqlx::AssertSqlSafe(sql))
        .bind(notification.id)
        .bind(&notification.address)
        .bind(&notification.kind)
        .bind(&notification.metadata)
        .bind(format!("user-notification:{}", notification.id))
        .bind(notification.occurred_at)
        .fetch_one(transaction)
        .await?;
    Ok(inserted as u64)
}

async fn insert_batch<'e, E>(
    executor: E,
    batch: &[NewNotification],
    event_preferences: bool,
) -> Result<u64, sqlx::Error>
where
    E: sqlx::Executor<'e, Database = sqlx::Postgres>,
{
    if batch.is_empty() {
        return Ok(0);
    }
    // Each statement touches an id once, taking the last entry.
    // Id order keeps row locking consistent.
    let mut seen = std::collections::HashSet::new();
    let mut unique: Vec<&NewNotification> =
        batch.iter().rev().filter(|n| seen.insert(n.id)).collect();
    unique.sort_by_key(|n| n.id);
    let batch = unique;
    let ids: Vec<Uuid> = batch.iter().map(|n| n.id).collect();
    let addresses: Vec<&str> = batch.iter().map(|n| n.address.as_str()).collect();
    let kinds: Vec<&str> = batch.iter().map(|n| n.kind.as_str()).collect();
    let metadata: Vec<Json> = batch.iter().map(|n| n.metadata.clone()).collect();
    let occurred: Vec<NaiveDateTime> = batch.iter().map(|n| n.occurred_at).collect();
    let sql = format!(
        r#"
        WITH ups AS (
            INSERT INTO social.notifications AS t (id, address, type, metadata, occurred_at, in_app, read, read_at)
            SELECT n.*, v.visible, NOT v.visible,
                   CASE WHEN NOT v.visible THEN timezone('UTC', now()) END
            FROM UNNEST($1::uuid[], $2::text[], $3::text[], $4::jsonb[], $5::timestamp[])
                AS n(id,address,type,metadata,occurred_at)
            LEFT JOIN social.subscriptions s ON s.address = n.address
            CROSS JOIN LATERAL (
                SELECT NOT $6::boolean OR (
                    NOT {ignore_all}
                    AND {in_app}
                ) AS visible
            ) v
            ORDER BY n.id
            ON CONFLICT (id) DO UPDATE
                SET metadata = EXCLUDED.metadata, occurred_at = EXCLUDED.occurred_at
                WHERE t.address = EXCLUDED.address AND t.type = EXCLUDED.type
            RETURNING id, address, type, (xmax = 0) AS inserted
        ),
        {tail}
        "#,
        tail = *UPSERT_TAIL,
        ignore_all = details_flag("->'ignore_all_in_app'", false),
        in_app = details_flag("->'message_type'->n.type->'in_app'", true),
    );
    let inserted: i64 = sqlx::query_scalar(sqlx::AssertSqlSafe(sql))
        .bind(&ids)
        .bind(&addresses)
        .bind(&kinds)
        .bind(&metadata)
        .bind(&occurred)
        .bind(event_preferences)
        .fetch_one(executor)
        .await?;
    Ok(inserted as u64)
}

/// `(Number(mana || '0') / 1e18).toFixed(2)`.
pub fn format_mana(mana: &Json) -> String {
    let n = match mana {
        Json::String(s) if !s.is_empty() => match s.trim() {
            "" => 0.0,
            t => t.parse::<f64>().unwrap_or(f64::NAN),
        },
        Json::Number(n) => n.as_f64().unwrap_or(f64::NAN),
        Json::Bool(true) => 1.0,
        Json::Null | Json::Bool(false) | Json::String(_) => 0.0,
        Json::Array(_) | Json::Object(_) => f64::NAN,
    };
    to_fixed_2(n / 1e18)
}

/// `Number.prototype.toFixed(2)`: exact ties round away from zero, not to even.
fn to_fixed_2(x: f64) -> String {
    if x.is_nan() {
        return "NaN".into();
    }
    if x.is_infinite() {
        return if x > 0.0 { "Infinity" } else { "-Infinity" }.into();
    }
    if x == 0.0 {
        return "0.00".into();
    }
    // A two-decimal tie is an odd multiple of 1/8.
    let eighths = x * 8.0;
    if eighths.fract() == 0.0 && eighths % 2.0 != 0.0 {
        let r = (x.abs() * 100.0 + 0.5).floor() / 100.0;
        return format!("{}{r:.2}", if x < 0.0 { "-" } else { "" });
    }
    format!("{x:.2}")
}
