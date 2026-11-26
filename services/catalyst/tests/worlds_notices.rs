//! Quota notice identity and retained inbox/email state through the production
//! worlds producer and Social directory. PostgreSQL assertions run with a disposable
//! cluster in `CATALYST_TEST_DATABASE_URL`; an unset variable reports a skip.

use std::sync::Arc;

use catalyst::comms::access::QUOTA_BLOCK_GRACE;
use catalyst::comms::interfaces::UserNotification;
use catalyst::shared::identity::Address;
use catalyst::social::comms_directory::Directory;
use catalyst::worlds::core::notices::{
    ACCESS_RESTORED, ACCESS_RESTRICTED, MISSING_RESOURCES, Notices,
};
use chrono::{NaiveDateTime, TimeDelta};
use sqlx::PgPool;
use uuid::Uuid;

use crate::db;

const WALLET: &str = "0x1111111111111111111111111111111111111111";
const OTHER_WALLET: &str = "0x2222222222222222222222222222222222222222";
const EXACT_TIME_NAMESPACE: Uuid = Uuid::from_bytes([
    0x5b, 0x0e, 0x7c, 0x31, 0x9a, 0x4d, 0x4f, 0x8e, 0xa1, 0x52, 0x3c, 0x77, 0x08, 0xd4, 0x6e, 0x19,
]);

async fn scratch_database() -> Option<db::ScratchDb> {
    let scratch = db::scratch_database().await;
    if scratch.is_none() {
        eprintln!(
            "skipping worlds quota PostgreSQL assertions: CATALYST_TEST_DATABASE_URL is unset"
        );
    }
    scratch
}

fn at(value: &str) -> NaiveDateTime {
    NaiveDateTime::parse_from_str(value, "%Y-%m-%d %H:%M:%S%.f").unwrap()
}

fn producer(pool: &PgPool, builder_url: &str) -> Notices {
    Notices::new(Some(Arc::new(Directory::new(pool.clone()))), builder_url)
}

fn lifecycle(notices: &Notices, wallet: &Address, created: NaiveDateTime) -> Vec<UserNotification> {
    let after_grace = created + QUOTA_BLOCK_GRACE + TimeDelta::seconds(1);
    vec![
        notices.quota_block(wallet, created, created),
        notices.quota_block(wallet, created, after_grace),
        notices.quota_restored(wallet, created, after_grace + TimeDelta::hours(1)),
    ]
}

async fn emit(pool: &PgPool, notices: &Notices, batch: &[UserNotification]) {
    let mut tx = pool.begin().await.unwrap();
    notices.send(&mut tx, batch).await.unwrap();
    tx.commit().await.unwrap();
}

async fn subscribe(pool: &PgPool, wallet: &Address) {
    sqlx::query("INSERT INTO social.subscriptions (address,email) VALUES ($1,'quota@example.org')")
        .bind(wallet)
        .execute(pool)
        .await
        .unwrap();
}

#[derive(Debug, PartialEq, sqlx::FromRow)]
struct StoredNotice {
    id: Uuid,
    kind: String,
    metadata: serde_json::Value,
    occurred_at: NaiveDateTime,
    created_at: NaiveDateTime,
    read: bool,
    read_at: Option<NaiveDateTime>,
    channel: String,
    state: String,
    attempts: i32,
    next_attempt_at: NaiveDateTime,
    last_error: Option<String>,
    updated_at: NaiveDateTime,
}

async fn stored(pool: &PgPool, wallet: &Address) -> Vec<StoredNotice> {
    sqlx::query_as(
        "SELECT n.id,n.type AS kind,n.metadata,n.occurred_at,n.created_at,n.read,n.read_at,
                d.channel,d.state,d.attempts,d.next_attempt_at,d.last_error,d.updated_at
         FROM social.notifications n
         JOIN social.notification_deliveries d ON d.notification_id=n.id
         WHERE n.address=$1 ORDER BY n.type,n.id,d.channel",
    )
    .bind(wallet)
    .fetch_all(pool)
    .await
    .unwrap()
}

async fn mark_read_and_sent(pool: &PgPool, wallet: &Address, read_at: NaiveDateTime) {
    sqlx::query("UPDATE social.notifications SET read=true,read_at=$2 WHERE address=$1")
        .bind(wallet)
        .bind(read_at)
        .execute(pool)
        .await
        .unwrap();
    sqlx::query(
        "UPDATE social.notification_deliveries SET state='sent',attempts=2,updated_at=$2
         WHERE notification_id IN (SELECT id FROM social.notifications WHERE address=$1)",
    )
    .bind(wallet)
    .bind(read_at)
    .execute(pool)
    .await
    .unwrap();
}

async fn assert_counts(pool: &PgPool, wallet: &Address, expected: i64) {
    let counts: Vec<(String, i64, i64)> = sqlx::query_as(
        "SELECT n.type,count(DISTINCT n.id),count(d.notification_id)
         FROM social.notifications n
         LEFT JOIN social.notification_deliveries d ON d.notification_id=n.id
         WHERE n.address=$1 GROUP BY n.type ORDER BY n.type",
    )
    .bind(wallet)
    .fetch_all(pool)
    .await
    .unwrap();
    assert_eq!(
        counts,
        [ACCESS_RESTORED, ACCESS_RESTRICTED, MISSING_RESOURCES].map(|kind| (
            kind.to_string(),
            expected,
            expected
        )),
        "one inbox row and one email delivery per kind and UTC block-creation day"
    );
}

async fn seed_exact_time_notice(pool: &PgPool, notice: &UserNotification, created: NaiveDateTime) {
    let mut native = notice.clone();
    native.id = Uuid::new_v5(
        &EXACT_TIME_NAMESPACE,
        format!(
            "{}\ndetected-{}\n{}",
            native.kind,
            created.and_utc().timestamp_micros(),
            native.recipient.as_str()
        )
        .as_bytes(),
    );
    sqlx::query(
        "INSERT INTO social.notifications (id,address,type,metadata,occurred_at)
         VALUES ($1,$2,$3,$4,$5)",
    )
    .bind(native.id)
    .bind(&native.recipient)
    .bind(native.kind)
    .bind(&native.metadata)
    .bind(native.occurred_at)
    .execute(pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO social.notification_deliveries (notification_id,channel,state)
         VALUES ($1,'email','pending')",
    )
    .bind(native.id)
    .execute(pool)
    .await
    .unwrap();
}

#[tokio::test]
async fn same_day_quota_cycles_preserve_first_notice_and_sent_email() {
    let Some(scratch) = scratch_database().await else {
        return;
    };
    eprintln!("worlds quota daily identity: PostgreSQL assertions enabled");
    let pool = &scratch.pool;
    let wallet = Address::parse(WALLET).unwrap();
    let other_wallet = Address::parse(OTHER_WALLET).unwrap();
    subscribe(pool, &wallet).await;
    subscribe(pool, &other_wallet).await;
    let first = at("2027-01-15 08:00:00.123000");
    let original = producer(pool, "https://builder.example.org/first");
    let first_batch = lifecycle(&original, &wallet, first);
    emit(pool, &original, &first_batch).await;
    assert_counts(pool, &wallet, 1).await;
    let initial = stored(pool, &wallet).await;
    assert!(
        initial
            .iter()
            .all(|row| row.state == "pending" && row.attempts == 0)
    );
    for notice in &first_batch {
        let row = initial.iter().find(|row| row.id == notice.id).unwrap();
        assert_eq!(row.metadata, notice.metadata);
        assert_eq!(row.occurred_at, notice.occurred_at);
    }
    mark_read_and_sent(pool, &wallet, first + TimeDelta::days(3)).await;
    let retained = stored(pool, &wallet).await;
    let changed = producer(pool, "https://builder.example.org/changed");
    for created in [
        first,
        first + TimeDelta::hours(1),
        first + TimeDelta::hours(2),
    ] {
        let repeats = lifecycle(&changed, &wallet, created);
        emit(pool, &changed, &repeats).await;
    }
    // Restoration remains associated with the block's creation day even when
    // a later retry observes its lift on another date.
    emit(
        pool,
        &changed,
        &[changed.quota_restored(&wallet, first, first + TimeDelta::days(7))],
    )
    .await;
    assert_counts(pool, &wallet, 1).await;
    assert_eq!(stored(pool, &wallet).await, retained);

    emit(
        pool,
        &changed,
        &lifecycle(&changed, &wallet, first + TimeDelta::days(1)),
    )
    .await;
    assert_counts(pool, &wallet, 2).await;
    emit(pool, &changed, &lifecycle(&changed, &other_wallet, first)).await;
    assert_counts(pool, &other_wallet, 1).await;
    let other = stored(pool, &other_wallet).await;
    assert!(
        other
            .iter()
            .all(|row| retained.iter().all(|first| first.id != row.id))
    );
    scratch.teardown().await;
}

#[tokio::test]
async fn quota_blocks_on_opposite_sides_of_utc_midnight_are_distinct() {
    let Some(scratch) = scratch_database().await else {
        return;
    };
    eprintln!("worlds quota UTC midnight: PostgreSQL assertions enabled");
    let pool = &scratch.pool;
    let wallet = Address::parse(WALLET).unwrap();
    subscribe(pool, &wallet).await;
    let notices = producer(pool, "https://builder.example.org");
    for created in [at("2027-01-15 23:59:59.999000"), at("2027-01-16 00:00:00")] {
        emit(pool, &notices, &lifecycle(&notices, &wallet, created)).await;
    }
    assert_counts(pool, &wallet, 2).await;
    scratch.teardown().await;
}

#[tokio::test]
async fn concurrent_same_day_quota_cycles_create_one_notice_and_email_per_kind() {
    let Some(scratch) = scratch_database().await else {
        return;
    };
    eprintln!("worlds concurrent quota identity: PostgreSQL assertions enabled");
    let pool = &scratch.pool;
    let wallet = Address::parse(WALLET).unwrap();
    subscribe(pool, &wallet).await;
    let first = at("2027-01-15 08:00:00");
    let left = producer(pool, "https://builder.example.org/left");
    let right = producer(pool, "https://builder.example.org/right");
    let left_batch = lifecycle(&left, &wallet, first);
    let right_batch = lifecycle(&right, &wallet, first + TimeDelta::hours(1));
    let mut left_tx = pool.begin().await.unwrap();
    let mut right_tx = pool.begin().await.unwrap();
    tokio::join!(
        async {
            left.send(&mut left_tx, &left_batch).await.unwrap();
            left_tx.commit().await.unwrap();
        },
        async {
            right.send(&mut right_tx, &right_batch).await.unwrap();
            right_tx.commit().await.unwrap();
        },
    );
    assert_counts(pool, &wallet, 1).await;
    let rows = stored(pool, &wallet).await;
    for row in rows {
        assert!(
            left_batch.iter().chain(&right_batch).any(|notice| {
                notice.kind == row.kind
                    && notice.metadata == row.metadata
                    && notice.occurred_at == row.occurred_at
            }),
            "the committed notice preserves one producer's complete payload: {row:?}"
        );
        assert_eq!(row.state, "pending");
        assert_eq!(row.attempts, 0);
    }
    scratch.teardown().await;
}

#[tokio::test]
async fn retained_exact_time_quota_notices_suppress_new_daily_emissions() {
    let Some(scratch) = scratch_database().await else {
        return;
    };
    eprintln!("worlds retained exact-time quota identity: PostgreSQL assertions enabled");
    let pool = &scratch.pool;
    let wallet = Address::parse(WALLET).unwrap();
    subscribe(pool, &wallet).await;
    let created = at("2027-01-15 08:00:00.123456");
    let original = producer(pool, "https://builder.example.org/first");
    for notice in lifecycle(&original, &wallet, created) {
        seed_exact_time_notice(pool, &notice, created).await;
    }
    mark_read_and_sent(pool, &wallet, created + TimeDelta::days(3)).await;
    let retained = stored(pool, &wallet).await;
    let changed = producer(pool, "https://builder.example.org/changed");
    for block in [created, created + TimeDelta::hours(1)] {
        emit(pool, &changed, &lifecycle(&changed, &wallet, block)).await;
    }
    assert_counts(pool, &wallet, 1).await;
    assert_eq!(stored(pool, &wallet).await, retained);
    scratch.teardown().await;
}

#[tokio::test]
async fn a_retained_warning_lifted_before_redetection_suppresses_a_same_day_reblock() {
    let Some(scratch) = scratch_database().await else {
        return;
    };
    eprintln!("worlds lifted retained warning: PostgreSQL assertions enabled");
    let pool = &scratch.pool;
    let wallet = Address::parse(WALLET).unwrap();
    subscribe(pool, &wallet).await;
    let created = at("2027-01-15 08:00:00.123456");
    let original = producer(pool, "https://builder.example.org/first");
    let warning = original.quota_block(&wallet, created, created);
    seed_exact_time_notice(pool, &warning, created).await;
    mark_read_and_sent(pool, &wallet, created + TimeDelta::minutes(15)).await;
    let warning_retained = stored(pool, &wallet).await;
    let changed = producer(pool, "https://builder.example.org/changed");
    // An active retained block can lift before its first quota recheck. The
    // restoration is its first emission through the current producer.
    let restoration = [changed.quota_restored(&wallet, created, created + TimeDelta::minutes(30))];
    let mut tx = pool.begin().await.unwrap();
    changed.send(&mut tx, &restoration).await.unwrap();
    tx.rollback().await.unwrap();
    let warning_key: Option<String> =
        sqlx::query_scalar("SELECT broadcast_key FROM social.notifications WHERE id=$1")
            .bind(warning_retained[0].id)
            .fetch_one(pool)
            .await
            .unwrap();
    assert_eq!(
        warning_key, None,
        "identity adoption rolls back with restoration"
    );
    assert_eq!(stored(pool, &wallet).await, warning_retained);
    emit(pool, &changed, &restoration).await;
    let lifted = stored(pool, &wallet).await;
    assert_eq!(lifted.len(), 2);
    assert_eq!(
        lifted.iter().find(|row| row.id == warning_retained[0].id),
        Some(&warning_retained[0])
    );
    mark_read_and_sent(pool, &wallet, created + TimeDelta::minutes(45)).await;
    let retained = stored(pool, &wallet).await;
    let reblocked = created + TimeDelta::hours(1);
    let batch = lifecycle(&changed, &wallet, reblocked);
    let mut tx = pool.begin().await.unwrap();
    changed.send(&mut tx, &batch).await.unwrap();
    let inside: (i64, i64) = sqlx::query_as(
        "SELECT count(DISTINCT n.id),count(d.notification_id)
         FROM social.notifications n
         LEFT JOIN social.notification_deliveries d ON d.notification_id=n.id
         WHERE n.address=$1",
    )
    .bind(&wallet)
    .fetch_one(&mut *tx)
    .await
    .unwrap();
    tx.rollback().await.unwrap();
    assert_eq!(stored(pool, &wallet).await, retained);
    assert_eq!(
        inside,
        (3, 3),
        "the transaction suppresses retained notices and inserts only the new restriction and email"
    );
    // A rolled-back reblock leaves no restriction or email, so a retry can
    // commit them alongside the retained warning and restoration.
    emit(pool, &changed, &batch).await;
    assert_counts(pool, &wallet, 1).await;
    let rows = stored(pool, &wallet).await;
    for original in retained {
        assert_eq!(
            rows.iter().find(|row| row.id == original.id),
            Some(&original)
        );
    }
    let restricted = rows
        .iter()
        .find(|row| row.kind == ACCESS_RESTRICTED)
        .unwrap();
    assert_eq!(restricted.state, "pending");
    assert_eq!(restricted.attempts, 0);
    scratch.teardown().await;
}
