//! The curation tables: a collection's reviews, an item's submissions, a
//! collection's review events and the slot cheques of a third-party
//! publication, and what each table refuses by itself.

use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

use super::support::scratch_pool;

const OWNER: &str = "0x1111111111111111111111111111111111111111";
const CURATOR: &str = "0x2222222222222222222222222222222222222222";
const PROVIDER: &str = "urn:decentraland:matic:collections-thirdparty:curations";

fn content_hash() -> String {
    format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple())
}

async fn collection(pool: &PgPool, third_party: bool) -> Uuid {
    let id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO builder.collections (id, name, eth_address, third_party_id, urn_suffix) \
         VALUES ($1, 'Curated', $2, $3, $4)",
    )
    .bind(id)
    .bind(OWNER)
    .bind(third_party.then_some(PROVIDER))
    .bind(third_party.then(|| format!("{PROVIDER}:{}", id.simple())))
    .execute(pool)
    .await
    .unwrap();
    id
}

async fn item(pool: &PgPool, collection: Uuid) -> Uuid {
    let id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO builder.items (id, collection_id, eth_address, name, type) \
         VALUES ($1, $2, $3, 'Curated item', 'wearable')",
    )
    .bind(id)
    .bind(collection)
    .bind(OWNER)
    .execute(pool)
    .await
    .unwrap();
    id
}

async fn accepted(pool: &PgPool, sql: &str) {
    sqlx::raw_sql(sqlx::AssertSqlSafe(sql.to_owned()))
        .execute(pool)
        .await
        .unwrap_or_else(|e| panic!("refused {sql}: {e}"));
}

async fn refused(pool: &PgPool, sql: &str, constraint: &str) {
    let error = sqlx::raw_sql(sqlx::AssertSqlSafe(sql.to_owned()))
        .execute(pool)
        .await
        .expect_err(sql);
    let named = error
        .as_database_error()
        .and_then(|e| e.constraint())
        .map(str::to_string);
    assert_eq!(named.as_deref(), Some(constraint), "{sql}: {error}");
}

#[tokio::test]
async fn an_item_stores_no_curation_state_of_its_own() {
    let Some(pool) = scratch_pool(2).await else {
        return;
    };
    let columns: Vec<String> = sqlx::query_scalar(
        "SELECT column_name::text FROM information_schema.columns \
         WHERE table_schema = 'builder' AND table_name = 'items' \
           AND column_name IN ('curation_status', 'is_approved')",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(columns, Vec::<String>::new());
}

#[tokio::test]
async fn a_collection_curation_keeps_its_review_and_refuses_what_upstream_never_writes() {
    let Some(pool) = scratch_pool(2).await else {
        return;
    };
    let c = collection(&pool, false).await;
    let insert = |values: &str| {
        format!(
            "INSERT INTO builder.collection_curations \
               (id, collection_id, status, assignee, reviewed_by, rejection_reasons, rejection_message, created_at, updated_at) \
             VALUES ('{}', '{c}', {values})",
            Uuid::new_v4()
        )
    };
    accepted(
        &pool,
        &insert(&format!(
            "'rejected', '{CURATOR}', '{CURATOR}', ARRAY['clipping', 'other'], 'Fix the hat', '2025-01-01', '2025-01-02'"
        )),
    )
    .await;
    accepted(
        &pool,
        &insert(
            "'rejected', NULL, 'validator', ARRAY['rigging'], NULL, '2025-01-03', '2025-01-03'",
        ),
    )
    .await;
    accepted(
        &pool,
        &insert("'pending', NULL, NULL, NULL, NULL, '2025-02-01', '2025-02-01'"),
    )
    .await;
    refused(
        &pool,
        &insert("'pending', NULL, NULL, NULL, NULL, '2025-02-01', '2025-02-01'"),
        "collection_curations_collection_created_key",
    )
    .await;
    refused(
        &pool,
        &insert("'withdrawn', NULL, NULL, NULL, NULL, '2025-03-01', '2025-03-01'"),
        "collection_curations_status_check",
    )
    .await;
    refused(
        &pool,
        &insert("'approved', '0xNotAnAddress', NULL, NULL, NULL, '2025-03-02', '2025-03-02'"),
        "collection_curations_assignee_check",
    )
    .await;
    refused(
        &pool,
        &insert(&format!(
            "'approved', NULL, '0x{}', NULL, NULL, '2025-03-03', '2025-03-03'",
            "aB".repeat(20)
        )),
        "collection_curations_reviewed_by_check",
    )
    .await;
    refused(
        &pool,
        &insert("'rejected', NULL, NULL, ARRAY['clipping', 'too_ugly'], NULL, '2025-03-04', '2025-03-04'"),
        "collection_curations_rejection_reasons_check",
    )
    .await;
    refused(
        &pool,
        &insert("'rejected', NULL, NULL, ARRAY[]::text[], NULL, '2025-03-05', '2025-03-05'"),
        "collection_curations_rejection_reasons_check",
    )
    .await;
    refused(
        &pool,
        &insert(&format!(
            "'rejected', NULL, NULL, NULL, '{}', '2025-03-06', '2025-03-06'",
            "x".repeat(3001)
        )),
        "collection_curations_rejection_message_check",
    )
    .await;
    refused(
        &pool,
        &insert("'pending', NULL, NULL, NULL, NULL, '2025-03-08', '2025-03-07'"),
        "collection_curations_updated_check",
    )
    .await;
}

#[tokio::test]
async fn an_item_curation_names_the_reviewed_content_once_per_instant() {
    let Some(pool) = scratch_pool(2).await else {
        return;
    };
    let i = item(&pool, collection(&pool, true).await).await;
    let insert = |status: &str, hash: &str, at: &str| {
        format!(
            "INSERT INTO builder.item_curations (id, item_id, status, content_hash, is_mapping_complete, created_at, updated_at) \
             VALUES ('{}', '{i}', '{status}', '{hash}', NULL, '{at}', '{at}')",
            Uuid::new_v4()
        )
    };
    accepted(&pool, &insert("pending", &content_hash(), "2025-01-01")).await;
    accepted(&pool, &insert("approved", &content_hash(), "2025-01-02")).await;
    refused(
        &pool,
        &insert("approved", &content_hash(), "2025-01-02"),
        "item_curations_item_created_key",
    )
    .await;
    refused(
        &pool,
        &insert("bogus", &content_hash(), "2025-01-03"),
        "item_curations_status_check",
    )
    .await;
    refused(
        &pool,
        &insert("pending", "bafkreinotahexdigest", "2025-01-04"),
        "item_curations_content_hash_check",
    )
    .await;
    refused(
        &pool,
        &insert("pending", &content_hash().to_uppercase(), "2025-01-05"),
        "item_curations_content_hash_check",
    )
    .await;
}

#[tokio::test]
async fn a_collection_event_is_one_of_upstreams_and_a_verdict_is_recorded_once() {
    let Some(pool) = scratch_pool(2).await else {
        return;
    };
    let c = collection(&pool, false).await;
    let insert = |kind: &str, actor: &str, address: &str, payload: &str| {
        format!(
            "INSERT INTO builder.collection_events (id, collection_id, type, actor, actor_address, payload) \
             VALUES ('{}', '{c}', '{kind}', '{actor}', {address}, '{payload}')",
            Uuid::new_v4()
        )
    };
    for kind in [
        "collection.published",
        "review.human_required",
        "review.automated_started",
        "review.automated_passed",
        "review.automated_rejected",
        "review.automated_error",
        "review.appeal_requested",
        "review.assigned",
        "review.approved",
        "review.rejected",
        "changes.submitted",
        "review.comment",
    ] {
        accepted(
            &pool,
            &insert(
                kind,
                "system",
                "NULL",
                &format!("{{\"validationId\": \"{kind}\"}}"),
            ),
        )
        .await;
    }
    for actor in ["creator", "curator", "validator"] {
        accepted(
            &pool,
            &insert("review.comment", actor, &format!("'{CURATOR}'"), "{}"),
        )
        .await;
    }
    refused(
        &pool,
        &insert(
            "review.automated_passed",
            "validator",
            "NULL",
            "{\"validationId\": \"review.automated_passed\"}",
        ),
        "collection_events_verdict_validation_idx",
    )
    .await;
    accepted(
        &pool,
        &insert(
            "review.automated_error",
            "validator",
            "NULL",
            "{\"validationId\": \"review.automated_error\", \"reason\": \"sweep_exhausted\"}",
        ),
    )
    .await;
    refused(
        &pool,
        &insert("review.withdrawn", "system", "NULL", "{}"),
        "collection_events_type_check",
    )
    .await;
    refused(
        &pool,
        &insert("review.comment", "operator", "NULL", "{}"),
        "collection_events_actor_check",
    )
    .await;
    refused(
        &pool,
        &insert("review.comment", "curator", "'not an address'", "{}"),
        "collection_events_actor_address_check",
    )
    .await;
    refused(
        &pool,
        &insert("review.comment", "curator", "NULL", "[]"),
        "collection_events_payload_object_check",
    )
    .await;
}

#[tokio::test]
async fn a_slot_cheque_is_a_signed_unused_salt_of_its_collections_provider() {
    let Some(pool) = scratch_pool(2).await else {
        return;
    };
    let c = collection(&pool, true).await;
    let salt = format!("0x{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple());
    let signature = format!("0x{}1b", "ab".repeat(64));
    let insert = |salt: &str, signature: &str, qty: i32| {
        format!(
            "INSERT INTO builder.slot_usage_cheques (id, collection_id, third_party_id, signature, salt, qty) \
             VALUES ('{}', '{c}', '{PROVIDER}', '{signature}', '{salt}', {qty})",
            Uuid::new_v4()
        )
    };
    accepted(&pool, &insert(&salt, &signature, 3)).await;
    refused(
        &pool,
        &insert(&salt, &signature, 3),
        "slot_usage_cheques_third_party_salt_key",
    )
    .await;
    let other = format!("0x{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple());
    refused(
        &pool,
        &insert(&other, &signature, 0),
        "slot_usage_cheques_qty_check",
    )
    .await;
    refused(
        &pool,
        &insert("0x1234", &signature, 1),
        "slot_usage_cheques_salt_check",
    )
    .await;
    refused(
        &pool,
        &insert(&other, "0xnot-a-signature", 1),
        "slot_usage_cheques_signature_check",
    )
    .await;
}

#[tokio::test]
async fn a_deleted_collection_takes_its_curations_and_events_and_keeps_a_cheque() {
    let Some(pool) = scratch_pool(2).await else {
        return;
    };
    let c = collection(&pool, true).await;
    let i = item(&pool, c).await;
    accepted(
        &pool,
        &format!(
            "INSERT INTO builder.collection_curations (id, collection_id, status) VALUES ('{}', '{c}', 'pending'); \
             INSERT INTO builder.item_curations (id, item_id, status, content_hash) VALUES ('{}', '{i}', 'pending', '{}'); \
             INSERT INTO builder.collection_events (id, collection_id, type, actor) VALUES ('{}', '{c}', 'collection.published', 'creator')",
            Uuid::new_v4(),
            Uuid::new_v4(),
            content_hash(),
            Uuid::new_v4()
        ),
    )
    .await;
    accepted(
        &pool,
        &format!("DELETE FROM builder.items WHERE id = '{i}'"),
    )
    .await;
    let left: i64 =
        sqlx::query_scalar("SELECT count(*) FROM builder.item_curations WHERE item_id = $1")
            .bind(i)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(left, 0, "an item's curations go with it");
    accepted(
        &pool,
        &format!("DELETE FROM builder.collections WHERE id = '{c}'"),
    )
    .await;
    let left: (i64, i64) = sqlx::query_as(
        "SELECT (SELECT count(*) FROM builder.collection_curations WHERE collection_id = $1), \
                (SELECT count(*) FROM builder.collection_events WHERE collection_id = $1)",
    )
    .bind(c)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        left,
        (0, 0),
        "a collection's curations and events go with it"
    );

    let cheque = collection(&pool, true).await;
    accepted(
        &pool,
        &format!(
            "INSERT INTO builder.slot_usage_cheques (id, collection_id, third_party_id, signature, salt, qty) \
             VALUES ('{}', '{cheque}', '{PROVIDER}', '0x{}1c', '0x{}{}', 1)",
            Uuid::new_v4(),
            "cd".repeat(64),
            Uuid::new_v4().simple(),
            Uuid::new_v4().simple()
        ),
    )
    .await;
    accepted(
        &pool,
        &format!("DELETE FROM builder.collections WHERE id = '{cheque}'"),
    )
    .await;
    let kept: Vec<Option<Uuid>> = sqlx::query_scalar(
        "SELECT collection_id FROM builder.slot_usage_cheques WHERE third_party_id = $1",
    )
    .bind(PROVIDER)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert!(
        kept.contains(&None),
        "a deleted collection's cheque stays, with no collection: {kept:?}"
    );
}

#[tokio::test]
async fn a_forum_link_is_an_https_url_and_a_forum_id_comes_with_one() {
    let Some(pool) = scratch_pool(2).await else {
        return;
    };
    let c = collection(&pool, false).await;
    let set = |link: &str, id: &str| {
        format!(
            "UPDATE builder.collections SET forum_link = {link}, forum_id = {id} WHERE id = '{c}'"
        )
    };
    accepted(
        &pool,
        &set("'https://forum.example.org/t/hat/41395'", "41395"),
    )
    .await;
    accepted(&pool, &set("'https://forum.example.org/t/hat'", "NULL")).await;
    accepted(&pool, &set("'http://forum.localhost/t/hat/1'", "1")).await;
    refused(
        &pool,
        &set("'https://forum.example.org/t/a b'", "NULL"),
        "collections_forum_link_check",
    )
    .await;
    refused(
        &pool,
        &set("'javascript:alert(1)'", "NULL"),
        "collections_forum_link_check",
    )
    .await;
    refused(
        &pool,
        &set(
            &format!("'https://forum.example.org/{}'", "a".repeat(2048)),
            "NULL",
        ),
        "collections_forum_link_check",
    )
    .await;
    refused(&pool, &set("NULL", "41395"), "collections_forum_id_check").await;
    refused(
        &pool,
        &set("'https://forum.example.org/t/hat'", "0"),
        "collections_forum_id_check",
    )
    .await;
}

/// A standard collection's contract address is kept lowercase, so its
/// unique key holds for the lowercased address every read joins on.
#[tokio::test]
async fn a_collection_contract_address_is_stored_lowercase() {
    let Some(pool) = scratch_pool(2).await else {
        return;
    };
    let c = collection(&pool, false).await;
    let contract = format!("0x{}abcdef01", c.simple());
    let set = |address: &str| {
        format!("UPDATE builder.collections SET contract_address = '{address}' WHERE id = '{c}'")
    };
    refused(
        &pool,
        &set(&format!("0x{}", contract[2..].to_uppercase())),
        "collections_contract_address_lowercase_check",
    )
    .await;
    accepted(&pool, &set(&contract)).await;
}

#[tokio::test]
async fn a_forum_claim_is_held_only_by_a_collection_without_a_link() {
    let Some(pool) = scratch_pool(2).await else {
        return;
    };
    let c = collection(&pool, false).await;
    let set = |link: &str, claim: &str| {
        format!(
            "UPDATE builder.collections SET forum_link = {link}, forum_claimed_at = {claim} WHERE id = '{c}'"
        )
    };
    accepted(&pool, &set("NULL", "timezone('UTC', now())")).await;
    accepted(
        &pool,
        &set("'https://forum.example.org/t/hat/41395'", "NULL"),
    )
    .await;
    refused(
        &pool,
        &set(
            "'https://forum.example.org/t/hat/41395'",
            "timezone('UTC', now())",
        ),
        "collections_forum_claim_check",
    )
    .await;
    let linked: bool =
        sqlx::query_scalar("SELECT to_regclass('builder.linked_collection_publications') IS NULL")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(linked, "the schema keeps no linked publication of its own");
}

/// The linked publication table as the releases before its retirement
/// left it, with its forum link when `forum` (before the linked-forum
/// release).
async fn legacy_linked_publications(pool: &PgPool, forum: bool) {
    accepted(
        pool,
        &format!(
            "CREATE TABLE builder.linked_collection_publications (
                collection_id uuid NOT NULL,
                preparation jsonb NOT NULL,
                snapshot jsonb NOT NULL,
                salt text NOT NULL,
                status text NOT NULL,
                cheque jsonb,
                {}
                created_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
                updated_at timestamp without time zone NOT NULL DEFAULT timezone('UTC', now()),
                CONSTRAINT linked_collection_publications_pkey PRIMARY KEY (collection_id),
                CONSTRAINT linked_collection_publications_collection_id_fkey FOREIGN KEY (collection_id)
                    REFERENCES builder.collections (id) ON DELETE CASCADE,
                CONSTRAINT linked_collection_publications_salt_key UNIQUE (salt),
                CONSTRAINT linked_collection_publications_salt_check CHECK (salt ~ '^0x[0-9a-f]{{64}}$'),
                CONSTRAINT linked_collection_publications_status_check
                    CHECK (status IN ('prepared', 'signing', 'authorized', 'submitted')),
                CONSTRAINT linked_collection_publications_cheque_state_check
                    CHECK ((status IN ('prepared', 'signing')) = (cheque IS NULL)),
                CONSTRAINT linked_collection_publications_preparation_check
                    CHECK (jsonb_typeof(preparation) = 'object' AND preparation ? 'revision')
            )",
            if forum { "forum_url text," } else { "" }
        ),
    )
    .await;
}

fn linked_release() -> String {
    include_str!(concat!(
        env!("DCL_ONE_ROOT"),
        "/schemas/releases/builder-linked-publications-retired.sql"
    ))
    .replace("\\set ON_ERROR_STOP on", "")
}

#[tokio::test]
async fn the_linked_publications_release_keeps_each_submitted_cheque_and_repeats() {
    let Some(pool) = crate::db::scratch::isolated_pool(2).await else {
        return;
    };
    legacy_linked_publications(&pool, false).await;
    let submitted = collection(&pool, true).await;
    let authorized = collection(&pool, true).await;
    let prepared = collection(&pool, true).await;
    let standard = collection(&pool, false).await;
    let salt = |n: u8| format!("0x{}", format!("{n:02x}").repeat(32));
    let signature = format!("0x{}00", "AB".repeat(64));
    for (id, status, cheque) in [
        (
            submitted,
            "submitted",
            Some(json!({ "qty": 2, "salt": salt(1), "signature": signature })),
        ),
        (
            authorized,
            "authorized",
            Some(json!({ "qty": 1, "salt": salt(2), "signature": signature })),
        ),
        (prepared, "prepared", None),
    ] {
        sqlx::query(
            "INSERT INTO builder.linked_collection_publications (collection_id, preparation, snapshot, salt, status, cheque, updated_at) \
             VALUES ($1, '{\"revision\":\"r\"}', '{}', $2, $3, $4, '2026-02-03 04:05:06.789')",
        )
        .bind(id)
        .bind(cheque.as_ref().map_or_else(|| salt(3), |c| c["salt"].as_str().unwrap().to_string()))
        .bind(status)
        .bind(cheque)
        .execute(&pool)
        .await
        .unwrap();
    }
    sqlx::query("UPDATE builder.collections SET publication_pending = true WHERE id = ANY($1)")
        .bind(vec![authorized, prepared, standard])
        .execute(&pool)
        .await
        .unwrap();
    sqlx::raw_sql(include_str!(concat!(
        env!("DCL_ONE_ROOT"),
        "/schemas/migrations/0090_builder_linked_publications_retired.sql"
    )))
    .execute(&pool)
    .await
    .unwrap();
    let pending: Vec<Uuid> = sqlx::query_scalar(
        "SELECT id FROM builder.collections WHERE publication_pending ORDER BY id",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(pending, [standard]);
    for _ in 0..2 {
        sqlx::raw_sql(sqlx::AssertSqlSafe(linked_release()))
            .execute(&pool)
            .await
            .unwrap();
    }
    let gone: bool =
        sqlx::query_scalar("SELECT to_regclass('builder.linked_collection_publications') IS NULL")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(gone);
    let cheques: Vec<(Uuid, String, String, String, i32, chrono::NaiveDateTime)> = sqlx::query_as(
        "SELECT collection_id, third_party_id, signature, salt, qty, created_at FROM builder.slot_usage_cheques",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(
        cheques,
        [(
            submitted,
            PROVIDER.to_string(),
            format!("0x{}1b", "ab".repeat(64)),
            salt(1),
            2,
            chrono::NaiveDateTime::parse_from_str(
                "2026-02-03 04:05:06.789",
                "%Y-%m-%d %H:%M:%S%.f"
            )
            .unwrap()
        )]
    );
}

/// A submitted linked publication whose cheque lacks its salt or its
/// signature is refused by name, and the release changes nothing.
#[tokio::test]
async fn the_linked_publications_release_refuses_a_cheque_missing_a_field() {
    let Some(pool) = crate::db::scratch::isolated_pool(2).await else {
        return;
    };
    legacy_linked_publications(&pool, false).await;
    let submitted = collection(&pool, true).await;
    let salt = format!("0x{}", "01".repeat(32));
    let signature = format!("0x{}1b", "ab".repeat(64));
    for cheque in [
        json!({ "qty": 1, "signature": signature }),
        json!({ "qty": 1, "salt": salt }),
    ] {
        sqlx::query(
            "INSERT INTO builder.linked_collection_publications (collection_id, preparation, snapshot, salt, status, cheque) \
             VALUES ($1, '{\"revision\":\"r\"}', '{}', $2, 'submitted', $3) \
             ON CONFLICT (collection_id) DO UPDATE SET cheque = EXCLUDED.cheque",
        )
        .bind(submitted)
        .bind(&salt)
        .bind(&cheque)
        .execute(&pool)
        .await
        .unwrap();
        let mut conn = pool.acquire().await.unwrap();
        let error = sqlx::raw_sql(sqlx::AssertSqlSafe(linked_release()))
            .execute(&mut *conn)
            .await
            .unwrap_err();
        sqlx::raw_sql("ROLLBACK").execute(&mut *conn).await.unwrap();
        assert!(
            error.to_string().contains(
                "1 submitted linked publications hold a cheque a slot cheque row cannot keep"
            ),
            "{cheque}: {error}"
        );
    }
    let kept: (bool, i64) = sqlx::query_as(
        "SELECT to_regclass('builder.linked_collection_publications') IS NOT NULL, \
                (SELECT count(*) FROM builder.slot_usage_cheques)",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(kept, (true, 0));
}

#[tokio::test]
async fn the_linked_publications_release_needs_its_step_and_the_forum_release() {
    let Some(pool) = crate::db::scratch::isolated_pool(2).await else {
        return;
    };
    legacy_linked_publications(&pool, true).await;
    let mut conn = pool.acquire().await.unwrap();
    let error = sqlx::raw_sql(sqlx::AssertSqlSafe(linked_release()))
        .execute(&mut *conn)
        .await
        .unwrap_err();
    sqlx::raw_sql("ROLLBACK").execute(&mut *conn).await.unwrap();
    assert!(
        error
            .to_string()
            .contains("builder-linked-forum-retired.sql"),
        "{error}"
    );
    accepted(
        &pool,
        "DELETE FROM meta.schema_steps WHERE step = '0090_builder_linked_publications_retired'",
    )
    .await;
    let error = sqlx::raw_sql(sqlx::AssertSqlSafe(linked_release()))
        .execute(&mut *conn)
        .await
        .unwrap_err();
    sqlx::raw_sql("ROLLBACK").execute(&mut *conn).await.unwrap();
    assert!(
        error.to_string().contains("run catalyst schema migrate"),
        "{error}"
    );
    let kept: bool = sqlx::query_scalar(
        "SELECT to_regclass('builder.linked_collection_publications') IS NOT NULL",
    )
    .fetch_one(&mut *conn)
    .await
    .unwrap();
    assert!(kept);
}

#[tokio::test]
async fn a_linked_forum_release_preserves_links_and_repeats_without_a_second_source() {
    let Some(pool) = crate::db::scratch::isolated_pool(2).await else {
        return;
    };
    let copied = collection(&pool, true).await;
    let kept = collection(&pool, true).await;
    legacy_linked_publications(&pool, true).await;
    for id in [copied, kept] {
        sqlx::query("INSERT INTO builder.linked_collection_publications (collection_id, preparation, snapshot, salt, status, forum_url) VALUES ($1, '{\"revision\":\"revision\"}', '{}', '0x' || replace($1::text, '-', '') || replace($1::text, '-', ''), 'prepared', 'https://forum.example.org/t/linked/7')")
            .bind(id).execute(&pool).await.unwrap();
    }
    sqlx::query("UPDATE builder.collections SET forum_link = 'https://forum.example.org/t/current/8', forum_id = 9 WHERE id = $1")
        .bind(kept).execute(&pool).await.unwrap();
    sqlx::query("UPDATE builder.collections SET forum_claimed_at = now() WHERE id = $1")
        .bind(copied)
        .execute(&pool)
        .await
        .unwrap();
    accepted(
        &pool,
        "ALTER TABLE builder.collections DROP CONSTRAINT collections_forum_claim_check",
    )
    .await;
    let migration = include_str!(concat!(
        env!("DCL_ONE_ROOT"),
        "/schemas/migrations/0089_builder_collection_forum_claim.sql"
    ));
    sqlx::raw_sql(migration).execute(&pool).await.unwrap();
    let backfilled: Option<String> =
        sqlx::query_scalar("SELECT forum_link FROM builder.collections WHERE id = $1")
            .bind(copied)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        backfilled.as_deref(),
        Some("https://forum.example.org/t/linked/7")
    );
    sqlx::query(
        "UPDATE builder.collections SET forum_link = NULL, forum_claimed_at = now() WHERE id = $1",
    )
    .bind(copied)
    .execute(&pool)
    .await
    .unwrap();
    let release = include_str!(concat!(
        env!("DCL_ONE_ROOT"),
        "/schemas/releases/builder-linked-forum-retired.sql"
    ))
    .replace("\\set ON_ERROR_STOP on", "");
    for _ in 0..2 {
        sqlx::raw_sql(sqlx::AssertSqlSafe(release.clone()))
            .execute(&pool)
            .await
            .unwrap();
    }
    let links: Vec<(Uuid, Option<String>, Option<i32>)> =
        sqlx::query_as("SELECT id, forum_link, forum_id FROM builder.collections ORDER BY id")
            .fetch_all(&pool)
            .await
            .unwrap();
    assert!(links.contains(&(
        copied,
        Some("https://forum.example.org/t/linked/7".into()),
        None
    )));
    assert!(links.contains(&(
        kept,
        Some("https://forum.example.org/t/current/8".into()),
        Some(9)
    )));
    let legacy: i64 = sqlx::query_scalar("SELECT count(*) FROM information_schema.columns WHERE table_schema='builder' AND table_name='linked_collection_publications' AND column_name='forum_url'")
        .fetch_one(&pool).await.unwrap();
    assert_eq!(legacy, 0);
    accepted(
        &pool,
        "ALTER TABLE builder.collections DROP CONSTRAINT collections_forum_claim_check",
    )
    .await;
    sqlx::raw_sql(migration).execute(&pool).await.unwrap();
}

#[tokio::test]
async fn linked_forum_retirement_requires_the_additive_migration() {
    let Some(pool) = crate::db::scratch::isolated_pool(2).await else {
        return;
    };
    legacy_linked_publications(&pool, true).await;
    accepted(
        &pool,
        "DELETE FROM meta.schema_steps WHERE step = '0089_builder_collection_forum_claim'",
    )
    .await;
    let release = include_str!(concat!(
        env!("DCL_ONE_ROOT"),
        "/schemas/releases/builder-linked-forum-retired.sql"
    ))
    .replace("\\set ON_ERROR_STOP on", "");
    let mut conn = pool.acquire().await.unwrap();
    let error = sqlx::raw_sql(sqlx::AssertSqlSafe(release))
        .execute(&mut *conn)
        .await
        .unwrap_err();
    sqlx::raw_sql("ROLLBACK").execute(&mut *conn).await.unwrap();
    assert!(
        error.to_string().contains("run catalyst schema migrate"),
        "{error}"
    );
    let legacy: i64 = sqlx::query_scalar("SELECT count(*) FROM information_schema.columns WHERE table_schema='builder' AND table_name='linked_collection_publications' AND column_name='forum_url'")
        .fetch_one(&mut *conn).await.unwrap();
    assert_eq!(legacy, 1);
}

async fn curate(pool: &PgPool, item: Uuid, status: &str, at: &str) {
    sqlx::query(
        "INSERT INTO builder.item_curations (id, item_id, status, content_hash, created_at, updated_at) \
         VALUES ($1, $2, $3, $4, $5::timestamp, $5::timestamp)",
    )
    .bind(Uuid::new_v4())
    .bind(item)
    .bind(status)
    .bind(content_hash())
    .bind(at)
    .execute(pool)
    .await
    .unwrap();
}

async fn statuses(pool: &PgPool, item: Uuid) -> Vec<String> {
    sqlx::query_scalar(
        "SELECT status FROM builder.item_curations WHERE item_id = $1 ORDER BY created_at",
    )
    .bind(item)
    .fetch_all(pool)
    .await
    .unwrap()
}

async fn admin_patch(
    app: &axum::Router,
    path: String,
    body: serde_json::Value,
) -> (u16, serde_json::Value) {
    use tower::ServiceExt;
    let response = app
        .clone()
        .oneshot(
            axum::http::Request::builder()
                .method("PATCH")
                .uri(path)
                .header("authorization", "Bearer example-admin-token")
                .header("content-type", "application/json")
                .body(axum::body::Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status().as_u16();
    let bytes = axum::body::to_bytes(response.into_body(), 1 << 16)
        .await
        .unwrap();
    (status, serde_json::from_slice(&bytes).unwrap_or_default())
}

#[tokio::test]
async fn the_operators_status_patch_decides_the_newest_curation_of_a_submitted_item() {
    let Some(pool) = scratch_pool(4).await else {
        return;
    };
    let state = super::support::state_over_with(
        pool.clone(),
        None,
        crate::Settings {
            writes_enabled: true,
            admin_token: Some("example-admin-token".into()),
            ..crate::Settings::default()
        },
    )
    .await;
    let app = crate::router(state);
    let c = collection(&pool, true).await;
    let (submitted, unsubmitted) = (item(&pool, c).await, item(&pool, c).await);
    sqlx::query("UPDATE builder.items SET local_content_hash = $2 WHERE collection_id = $1")
        .bind(c)
        .bind(content_hash())
        .execute(&pool)
        .await
        .unwrap();
    curate(&pool, submitted, "approved", "2025-01-01").await;
    curate(&pool, submitted, "pending", "2025-01-02").await;

    let (status, body) = admin_patch(
        &app,
        format!("/v1/collections/{c}/items/{submitted}/status"),
        serde_json::json!({ "status": "rejected" }),
    )
    .await;
    assert_eq!(
        (status, &body["data"]["updated"]),
        (200, &serde_json::json!(1)),
        "{body}"
    );
    assert_eq!(statuses(&pool, submitted).await, ["approved", "rejected"]);

    let (status, body) = admin_patch(
        &app,
        format!("/v1/collections/{c}/items/{unsubmitted}/status"),
        serde_json::json!({ "status": "approved" }),
    )
    .await;
    assert_eq!(
        status, 404,
        "an item never submitted has no curation to decide: {body}"
    );
    assert_eq!(statuses(&pool, unsubmitted).await, Vec::<String>::new());

    let (status, body) = admin_patch(
        &app,
        format!("/v1/collections/{c}/items/status"),
        serde_json::json!({ "status": "approved", "itemIds": [submitted, unsubmitted] }),
    )
    .await;
    assert_eq!(
        (status, &body["data"]["requested"], &body["data"]["updated"]),
        (200, &serde_json::json!(2), &serde_json::json!(1)),
        "{body}"
    );
    assert_eq!(statuses(&pool, submitted).await, ["approved", "approved"]);
    assert_eq!(statuses(&pool, unsubmitted).await, Vec::<String>::new());

    let other = collection(&pool, true).await;
    let anchor = item(&pool, other).await;
    curate(&pool, anchor, "approved", "2025-01-01").await;
    let (status, _) = admin_patch(
        &app,
        format!("/v1/collections/{other}/items/{submitted}/status"),
        serde_json::json!({ "status": "pending" }),
    )
    .await;
    assert_eq!(
        status, 404,
        "an item is decided only through its own collection"
    );
    assert_eq!(statuses(&pool, submitted).await, ["approved", "approved"]);
}

#[tokio::test]
async fn a_recorded_item_curation_is_the_items_newest_even_twice_in_one_transaction() {
    let Some(pool) = scratch_pool(2).await else {
        return;
    };
    let i = item(&pool, collection(&pool, true).await).await;
    curate(&pool, i, "approved", "2999-01-01").await;
    let mut tx = pool.begin().await.unwrap();
    crate::curation::record_item_curation(&mut tx, i, "pending", &content_hash(), false)
        .await
        .unwrap();
    crate::curation::record_item_curation(&mut tx, i, "rejected", &content_hash(), true)
        .await
        .unwrap();
    tx.commit().await.unwrap();
    assert_eq!(
        statuses(&pool, i).await,
        ["approved", "pending", "rejected"]
    );
}

#[tokio::test]
async fn a_collection_is_mapping_complete_only_while_no_item_curation_says_otherwise() {
    let Some(store) = super::support::scratch_component().await else {
        return;
    };
    let pool = store.pool().clone();
    let c = collection(&pool, true).await;
    let i = item(&pool, c).await;
    sqlx::query("UPDATE builder.items SET mappings = '{\"mainnet\": {}}' WHERE id = $1")
        .bind(i)
        .execute(&pool)
        .await
        .unwrap();
    let complete = async || -> serde_json::Value {
        store
            .collection(&crate::access::granted::read(c))
            .await
            .unwrap()["is_mapping_complete"]
            .clone()
    };
    assert_eq!(
        complete().await,
        serde_json::json!(true),
        "mapped and never submitted"
    );
    curate(&pool, i, "approved", "2025-01-01").await;
    assert_eq!(
        complete().await,
        serde_json::json!(false),
        "submitted with no mapping flag"
    );
    sqlx::query("UPDATE builder.item_curations SET is_mapping_complete = true WHERE item_id = $1")
        .bind(i)
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(complete().await, serde_json::json!(true));
    curate(&pool, i, "pending", "2025-02-01").await;
    sqlx::query(
        "UPDATE builder.item_curations SET is_mapping_complete = false, updated_at = '2025-03-01' \
         WHERE item_id = $1 AND created_at = '2025-02-01'",
    )
    .bind(i)
    .execute(&pool)
    .await
    .unwrap();
    assert_eq!(
        complete().await,
        serde_json::json!(false),
        "the curation changed last says the mapping is incomplete"
    );
}

#[tokio::test]
async fn an_address_listing_of_published_collections_counts_a_curated_third_party_one() {
    let Some(store) = super::support::scratch_component().await else {
        return;
    };
    let pool = store.pool().clone();
    let (curated, uncurated) = (collection(&pool, true).await, collection(&pool, true).await);
    let i = item(&pool, curated).await;
    item(&pool, curated).await;
    item(&pool, uncurated).await;
    curate(&pool, i, "pending", "2025-01-01").await;
    let wallet = crate::access::granted::managing(OWNER, &[PROVIDER]);
    let listed = async |q: &str| -> Vec<String> {
        let query = crate::collections::listing::ListQuery::parse(Some(q));
        let request = crate::collections::listing::OwnListing::new(&wallet, &query).unwrap();
        let body = store.address_collections(&request).await.unwrap();
        let rows = body
            .as_array()
            .cloned()
            .unwrap_or_else(|| body["results"].as_array().cloned().unwrap_or_default());
        rows.iter()
            .filter_map(|r| r["id"].as_str().map(str::to_string))
            .filter(|id| *id == curated.to_string() || *id == uncurated.to_string())
            .collect()
    };
    assert_eq!(listed("is_published=true").await, [curated.to_string()]);
    assert_eq!(listed("is_published=false").await, [uncurated.to_string()]);
}

fn scans(plan: &serde_json::Value, alias: &str) -> f64 {
    let own = if plan["Alias"] == alias {
        plan["Actual Loops"].as_f64().unwrap_or(0.0)
    } else {
        0.0
    };
    own + plan["Plans"]
        .as_array()
        .map(|plans| plans.iter().map(|p| scans(p, alias)).sum())
        .unwrap_or(0.0)
}

#[tokio::test]
async fn a_list_of_curated_items_reads_each_items_curation_once_and_its_collection_never() {
    let Some(pool) = scratch_pool(2).await else {
        return;
    };
    let c = collection(&pool, true).await;
    let mut ids = Vec::new();
    for n in 0..40 {
        let i = item(&pool, c).await;
        curate(&pool, i, "approved", &format!("2025-01-01 00:00:{n:02}")).await;
        ids.push(i);
    }
    sqlx::raw_sql("ANALYZE builder.items, builder.item_curations")
        .execute(&pool)
        .await
        .unwrap();
    let mut conn = pool.acquire().await.unwrap();
    let view = crate::chain_state::for_read(&mut conn, &crate::chain_state::Clock::default())
        .await
        .unwrap();
    let plan: serde_json::Value = sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
        "EXPLAIN (ANALYZE, FORMAT JSON) {}",
        crate::items::listing::list_statement(&view)
    )))
    .bind(&ids)
    .fetch_one(&mut *conn)
    .await
    .unwrap();
    assert_eq!(
        (scans(&plan[0]["Plan"], "i"), scans(&plan[0]["Plan"], "ki")),
        (1.0, 0.0),
        "a list body reads its items once and never its collection per row: {plan}"
    );
    assert!(
        scans(&plan[0]["Plan"], "k") <= ids.len() as f64,
        "at most one probe of the curations per listed item: {plan}"
    );
}

#[tokio::test]
async fn every_read_of_an_items_newest_curation_takes_the_last_created_one() {
    let Some(store) = super::support::scratch_component().await else {
        return;
    };
    let pool = store.pool().clone();
    let c = collection(&pool, true).await;
    let i = item(&pool, c).await;
    sqlx::query(
        "UPDATE builder.items SET mappings = '{\"mainnet\": {}}', urn_suffix = $2 || ':hat' WHERE id = $1",
    )
    .bind(i)
    .bind(format!("{PROVIDER}:{}", c.simple()))
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO builder.item_curations (id, item_id, status, content_hash, is_mapping_complete, created_at, updated_at) VALUES \
           (gen_random_uuid(), $1, 'approved', repeat('ab', 32), true, '2025-01-01', '2025-03-01'), \
           (gen_random_uuid(), $1, 'pending', repeat('cd', 32), false, '2025-02-01', '2025-02-01')",
    )
    .bind(i)
    .execute(&pool)
    .await
    .unwrap();
    let collection_complete = store
        .collection(&crate::access::granted::read(c))
        .await
        .unwrap()["is_mapping_complete"]
        .clone();
    let mut conn = pool.acquire().await.unwrap();
    let view = crate::chain_state::for_read(&mut conn, &crate::chain_state::Clock::default())
        .await
        .unwrap();
    let listed: Vec<crate::items::body::ItemRow> = sqlx::query_as(sqlx::AssertSqlSafe(
        crate::items::listing::list_statement(&view),
    ))
    .bind(vec![i])
    .fetch_all(&mut *conn)
    .await
    .unwrap();
    let in_list = listed
        .into_iter()
        .next()
        .unwrap()
        .out(
            &crate::network::POLYGON,
            crate::items::body::Reached::InList,
            true,
        )
        .unwrap();
    drop(conn);
    assert_eq!(
        (collection_complete, in_list.is_mapping_complete),
        (serde_json::json!(false), Some(false)),
        "the collection and the item read the same newest curation"
    );
}

async fn created(pool: &PgPool, table: &str, key: &str, id: Uuid) -> Vec<chrono::NaiveDateTime> {
    sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
        "SELECT created_at FROM builder.{table} WHERE {key} = $1 AND updated_at >= created_at ORDER BY created_at"
    )))
    .bind(id)
    .fetch_all(pool)
    .await
    .unwrap()
}

#[tokio::test]
async fn collection_curations_recorded_together_are_each_newer_than_the_last() {
    let Some(pool) = scratch_pool(4).await else {
        return;
    };
    let c = collection(&pool, true).await;
    let grant = crate::access::granted::third_party_publish(OWNER, c, PROVIDER);
    accepted(
        &pool,
        &format!(
            "INSERT INTO builder.collection_curations (id, collection_id, status, created_at, updated_at) \
             VALUES ('{}', '{c}', 'approved', '2999-01-01', '2999-01-02')",
            Uuid::new_v4()
        ),
    )
    .await;
    let mut tx = pool.begin().await.unwrap();
    for status in ["pending", "pending", "rejected"] {
        crate::curation::record_collection_curation(&mut tx, &grant, status, None)
            .await
            .unwrap();
    }
    tx.commit().await.unwrap();
    let stamps = created(&pool, "collection_curations", "collection_id", c).await;
    assert_eq!(stamps.len(), 4, "every row kept updated_at >= created_at");
    let order: Vec<String> = sqlx::query_scalar(
        "SELECT status FROM builder.collection_curations WHERE collection_id = $1 ORDER BY created_at",
    )
    .bind(c)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(order, ["approved", "pending", "pending", "rejected"]);
}

/// A second writer of `id`'s curations waits for the first's transaction and
/// stamps its row after the first's.
async fn writers_queue(
    pool: &PgPool,
    table: &str,
    key: &str,
    id: Uuid,
    write: impl AsyncFn(&mut sqlx::PgConnection),
) {
    let mut first = pool.begin().await.unwrap();
    write(&mut first).await;
    let mut second = pool.begin().await.unwrap();
    let waited =
        tokio::time::timeout(std::time::Duration::from_millis(300), write(&mut second)).await;
    assert!(
        waited.is_err(),
        "the second writer waits for the first's transaction"
    );
    drop(second);
    let mut second = pool.begin().await.unwrap();
    let pending = async {
        write(&mut second).await;
        second.commit().await.unwrap();
    };
    let commit_first = async {
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        first.commit().await.unwrap();
    };
    tokio::join!(pending, commit_first);
    let stamps = created(pool, table, key, id).await;
    assert_eq!(stamps.len(), 2);
    assert!(stamps[0] < stamps[1]);
}

#[tokio::test]
async fn concurrent_writers_of_one_collections_curations_queue_behind_its_lock() {
    let Some(pool) = scratch_pool(4).await else {
        return;
    };
    let c = collection(&pool, true).await;
    let grant = crate::access::granted::third_party_publish(OWNER, c, PROVIDER);
    writers_queue(
        &pool,
        "collection_curations",
        "collection_id",
        c,
        async |conn| {
            crate::curation::record_collection_curation(conn, &grant, "pending", None)
                .await
                .unwrap();
        },
    )
    .await;
    let i = item(&pool, c).await;
    writers_queue(&pool, "item_curations", "item_id", i, async |conn| {
        crate::curation::record_item_curation(conn, i, "pending", &content_hash(), false)
            .await
            .unwrap();
    })
    .await;
}
