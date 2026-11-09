//! Each source end to end on a database of its own (the cursors are one row
//! per source and a pass reads the whole table past it): source rows in,
//! a batch, then the progress document, tiers, notifications and cursor.
//! Skipped without `CATALYST_TEST_DATABASE_URL`.

use chrono::{Duration as ChronoDuration, NaiveDateTime};
use serde_json::{Value, json};
use sqlx::PgPool;
use uuid::Uuid;

use super::recount::{self, Recount};
use super::sales::{self, Log};
use super::{Pass, ProcessorSettings, client_events, deployments};
use crate::notifications::store::test_support::{fresh_address, private_pool};

const BASE: &str = "https://cdn.example";

fn pass(pool: &PgPool) -> Pass {
    Pass {
        pool: pool.clone(),
        asset_base: BASE.into(),
        settings: ProcessorSettings {
            enabled: true,
            ..ProcessorSettings::default()
        },
    }
}

fn now() -> NaiveDateTime {
    crate::shared::now_utc()
}

async fn cursor(pool: &PgPool, source: &str) -> Option<(i64, i32)> {
    sqlx::query_as("SELECT position, log_index FROM social.badge_cursors WHERE source = $1")
        .bind(source)
        .fetch_optional(pool)
        .await
        .unwrap()
}

async fn progress(pool: &PgPool, address: &str, badge: &str) -> Option<(i32, Value, bool)> {
    sqlx::query_as(
        "SELECT steps_done, progress, completed_at IS NOT NULL FROM social.user_badge_progress \
         WHERE address = $1 AND badge_id = $2",
    )
    .bind(address)
    .bind(badge)
    .fetch_optional(pool)
    .await
    .unwrap()
}

async fn tiers(pool: &PgPool, address: &str, badge: &str) -> Vec<String> {
    sqlx::query_scalar(
        "SELECT t.tier_id FROM social.user_achieved_tiers a \
         JOIN social.badge_tiers t USING (badge_id, tier_id) \
         WHERE a.address = $1 AND a.badge_id = $2 ORDER BY t.ordinal",
    )
    .bind(address)
    .bind(badge)
    .fetch_all(pool)
    .await
    .unwrap()
}

async fn granted(pool: &PgPool, address: &str) -> Vec<String> {
    sqlx::query_scalar(
        "SELECT metadata->>'description' FROM social.notifications \
         WHERE address = $1 AND type = 'badge_granted' ORDER BY metadata->>'description'",
    )
    .bind(address)
    .fetch_all(pool)
    .await
    .unwrap()
}

/// Batches until every row is passed: the settled rule waits out write
/// transactions open anywhere on the shared test cluster.
async fn drain_client_events(p: &Pass) -> u64 {
    let mut passed = 0;
    for _ in 0..100 {
        let tail: Option<i64> = sqlx::query_scalar("SELECT max(seq) FROM social.client_events")
            .fetch_one(&p.pool)
            .await
            .unwrap();
        if cursor(&p.pool, client_events::SOURCE).await.map(|c| c.0) >= tail {
            return passed;
        }
        passed += client_events::batch(p).await.unwrap().0;
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    panic!("client events never settled");
}

async fn drain_deployments(p: &Pass) -> u64 {
    let mut passed = 0;
    for _ in 0..100 {
        let tail: Option<i64> =
            sqlx::query_scalar("SELECT max(id)::bigint FROM content.deployments")
                .fetch_one(&p.pool)
                .await
                .unwrap();
        if cursor(&p.pool, deployments::SOURCE).await.map(|c| c.0) >= tail {
            return passed;
        }
        passed += deployments::batch(p).await.unwrap().0;
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    panic!("deployments never settled");
}

async fn client_event(
    pool: &PgPool,
    address: &str,
    sub_type: &str,
    session: Option<&str>,
    at: NaiveDateTime,
    metadata: Value,
) {
    sqlx::query(
        "INSERT INTO social.client_events (key, source, sub_type, address, session_id, reported_at, received_at, metadata) \
         VALUES ($1, 'client', $2, $3, $4, $5, $5, $6)",
    )
    .bind(Uuid::new_v4().to_string())
    .bind(sub_type)
    .bind(address)
    .bind(session)
    .bind(at)
    .bind(metadata)
    .execute(pool)
    .await
    .unwrap();
}

#[tokio::test]
async fn client_events_start_at_the_head_then_grant_with_notifications() {
    let Some(pool) = private_pool("badges_client").await else {
        return;
    };
    let p = pass(&pool);
    let a = fresh_address();
    let a = a.as_str();
    client_event(
        &pool,
        a,
        "vertical-height-reached",
        None,
        now(),
        json!({"height": 900}),
    )
    .await;
    assert_eq!(client_events::batch(&p).await.unwrap(), (0, false));
    let head: i64 = sqlx::query_scalar("SELECT max(seq) FROM social.client_events")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(cursor(&pool, client_events::SOURCE).await, Some((head, 0)));
    assert_eq!(
        progress(&pool, a, "vertical_voyager").await,
        None,
        "no history replay"
    );

    let t = now();
    client_event(
        &pool,
        a,
        "move-to-parcel",
        Some("s"),
        t,
        json!({"parcel": {"newParcel": "5,5", "isEmptyParcel": true}}),
    )
    .await;
    client_event(
        &pool,
        a,
        "walked-distance",
        None,
        t,
        json!({"stepCount": 6000}),
    )
    .await;
    client_event(
        &pool,
        a,
        "walked-distance",
        None,
        t,
        json!({"stepCount": "4000"}),
    )
    .await;
    client_event(
        &pool,
        a,
        "walked-distance",
        None,
        t,
        json!({"stepCount": null}),
    )
    .await;
    client_event(
        &pool,
        a,
        "vertical-height-reached",
        None,
        t,
        json!({"height": 499}),
    )
    .await;
    client_event(
        &pool,
        a,
        "vertical-height-reached",
        None,
        t,
        json!({"height": 501}),
    )
    .await;
    client_event(&pool, a, "used-emote", None, t, json!({"emote": {}})).await;
    client_event(
        &pool,
        a,
        "passport-opened",
        None,
        t,
        json!({"passport": {"receiver": "0x00000000000000000000000000000000000000Bb"}}),
    )
    .await;
    client_event(
        &pool,
        a,
        "passport-opened",
        None,
        t,
        json!({"passport": {"receiver": a}}),
    )
    .await;
    assert_eq!(drain_client_events(&p).await, 9);
    let tail: i64 = sqlx::query_scalar("SELECT max(seq) FROM social.client_events")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(cursor(&pool, client_events::SOURCE).await, Some((tail, 0)));

    assert_eq!(
        progress(&pool, a, "dclcitizen").await,
        Some((1, json!({"steps": 1, "visited": "5,5"}), true))
    );
    assert_eq!(
        progress(&pool, a, "walkabout_wanderer").await.unwrap().0,
        10_000
    );
    assert_eq!(
        tiers(&pool, a, "walkabout_wanderer").await,
        ["walkabout-wanderer-starter"]
    );
    assert_eq!(
        progress(&pool, a, "vertical_voyager").await,
        Some((1, json!({"steps": 1, "height_reached": 501.0}), true))
    );
    assert_eq!(progress(&pool, a, "moves_master").await.unwrap().0, 1);
    assert_eq!(
        progress(&pool, a, "social_butterfly").await.unwrap().1,
        json!({"steps": 1, "profiles_visited": ["0x00000000000000000000000000000000000000bb"]})
    );
    assert_eq!(
        granted(&pool, a).await,
        [
            "Decentraland Citizen",
            "Social Butterfly Starter",
            "Vertical Voyager",
            "Walkabout Wanderer Starter"
        ],
        "one notification per new tier or completed single badge"
    );
    assert_eq!(
        client_events::batch(&p).await.unwrap(),
        (0, false),
        "nothing past the cursor"
    );
}

async fn scene(pool: &PgPool, parcel: &str, title: &str) {
    let entity = format!("bafyscene{}", Uuid::new_v4().simple());
    sqlx::query(
        "INSERT INTO content.deployments (deployer_address, version, entity_type, entity_id, entity_metadata, entity_timestamp, entity_pointers, local_timestamp, auth_chain) \
         VALUES ('0x0', 'v3', 'scene', $1, $2::json, timezone('UTC', now()), ARRAY[$3], timezone('UTC', now()), '[]'::json)",
    )
    .bind(&entity)
    .bind(json!({"v": {"display": {"title": title}}}))
    .bind(parcel)
    .execute(pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO content.active_pointers (pointer, entity_id, entity_type) VALUES ($1, $2, 'scene')")
        .bind(parcel)
        .bind(&entity)
        .execute(pool)
        .await
        .unwrap();
}

#[tokio::test]
async fn traveler_reads_the_session_back_and_resolves_titles_locally() {
    let Some(pool) = private_pool("badges_traveler").await else {
        return;
    };
    let p = pass(&pool);
    client_events::batch(&p).await.unwrap();
    scene(&pool, "100,100", "Plaza").await;
    scene(&pool, "101,100", "Market").await;
    let a = fresh_address();
    let a = a.as_str();
    let t0 = now() - ChronoDuration::hours(5);
    let mv = |parcel: &str| json!({"parcel": {"newParcel": parcel, "isEmptyParcel": false}});
    // an earlier session, separated by more than two hours, does not count
    client_event(&pool, a, "move-to-parcel", Some("s1"), t0, mv("(100, 100)")).await;
    let t1 = t0 + ChronoDuration::hours(3);
    client_event(&pool, a, "move-to-parcel", Some("s1"), t1, mv("101,100")).await;
    // 90 s in the Market, then an untitled parcel and the Plaza
    client_event(
        &pool,
        a,
        "move-to-parcel",
        Some("s1"),
        t1 + ChronoDuration::seconds(90),
        mv("7,7"),
    )
    .await;
    client_event(
        &pool,
        a,
        "move-to-parcel",
        Some("s1"),
        t1 + ChronoDuration::seconds(91),
        mv("100,100"),
    )
    .await;
    drain_client_events(&p).await;
    assert_eq!(
        progress(&pool, a, "traveler").await,
        Some((
            1,
            json!({"steps": 1, "scenes_titles_visited": ["Market"]}),
            false
        ))
    );
    assert_eq!(tiers(&pool, a, "traveler").await, ["traveler-starter"]);
}

#[tokio::test]
async fn a_database_error_leaves_the_cursor_and_the_batch_retries() {
    let Some(pool) = private_pool("badges_retry").await else {
        return;
    };
    let p = pass(&pool);
    client_events::batch(&p).await.unwrap();
    let before = cursor(&pool, client_events::SOURCE).await;
    let a = fresh_address();
    client_event(
        &pool,
        a.as_str(),
        "vertical-height-reached",
        None,
        now(),
        json!({"height": 600}),
    )
    .await;
    sqlx::raw_sql(
        "CREATE FUNCTION social.test_refuse() RETURNS trigger AS $$ BEGIN RAISE EXCEPTION 'refused'; END $$ LANGUAGE plpgsql; \
         CREATE TRIGGER test_refuse BEFORE INSERT ON social.user_badge_progress FOR EACH ROW EXECUTE FUNCTION social.test_refuse();",
    )
    .execute(&pool)
    .await
    .unwrap();
    let mut failed = false;
    for _ in 0..100 {
        if client_events::batch(&p).await.is_err() {
            failed = true;
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    assert!(failed, "the write fails once the row is settled");
    assert_eq!(
        cursor(&pool, client_events::SOURCE).await,
        before,
        "not advanced"
    );
    assert_eq!(progress(&pool, a.as_str(), "vertical_voyager").await, None);
    sqlx::raw_sql("DROP TRIGGER test_refuse ON social.user_badge_progress")
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(drain_client_events(&p).await, 1);
    assert!(
        progress(&pool, a.as_str(), "vertical_voyager")
            .await
            .unwrap()
            .2
    );
}

#[tokio::test]
async fn an_unsettled_row_holds_the_cursor_before_it() {
    let Some(pool) = private_pool("badges_settle").await else {
        return;
    };
    let p = pass(&pool);
    client_events::batch(&p).await.unwrap();
    let a = fresh_address();
    // An open transaction holds an id older than every later insert.
    let mut open = pool.begin().await.unwrap();
    sqlx::query("SELECT txid_current()")
        .execute(&mut *open)
        .await
        .unwrap();
    client_event(
        &pool,
        a.as_str(),
        "vertical-height-reached",
        None,
        now(),
        json!({"height": 600}),
    )
    .await;
    let before = cursor(&pool, client_events::SOURCE).await;
    assert_eq!(client_events::batch(&p).await.unwrap(), (0, false));
    assert_eq!(cursor(&pool, client_events::SOURCE).await, before);
    open.rollback().await.unwrap();
    assert_eq!(drain_client_events(&p).await, 1);
}

#[tokio::test]
async fn an_imported_document_resumes() {
    let Some(pool) = private_pool("badges_resume").await else {
        return;
    };
    let p = pass(&pool);
    client_events::batch(&p).await.unwrap();
    let a = fresh_address();
    let a = a.as_str();
    // what the TS import writes: the document verbatim, its steps, its tiers
    sqlx::query(
        "INSERT INTO social.user_badge_progress (address, badge_id, steps_done, progress, last_completed_tier_id) \
         VALUES ($1, 'social_butterfly', 1, $2, 'social-butterfly-starter'), \
                ($1, 'moves_master', 3, $3, NULL)",
    )
    .bind(a)
    .bind(json!({"steps": 1, "profiles_visited": ["0x00000000000000000000000000000000000000bb"]}))
    .bind(json!({"steps": 3, "last_used_emote_timestamp": 4_000_000_000_000_i64, "last_day_used_emotes_timestamps": [4_000_000_000_000_i64]}))
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO social.user_achieved_tiers (address, badge_id, tier_id) VALUES ($1, 'social_butterfly', 'social-butterfly-starter')")
        .bind(a)
        .execute(&pool)
        .await
        .unwrap();
    let t = now();
    client_event(
        &pool,
        a,
        "passport-opened",
        None,
        t,
        json!({"passport": {"receiver": "0x00000000000000000000000000000000000000BB"}}),
    )
    .await;
    client_event(
        &pool,
        a,
        "passport-opened",
        None,
        t,
        json!({"passport": {"receiver": "0x00000000000000000000000000000000000000cc"}}),
    )
    .await;
    client_event(&pool, a, "used-emote", None, t, json!({})).await;
    drain_client_events(&p).await;
    assert_eq!(
        progress(&pool, a, "social_butterfly").await.unwrap().1,
        json!({"steps": 2, "profiles_visited": [
            "0x00000000000000000000000000000000000000bb",
            "0x00000000000000000000000000000000000000cc"
        ]})
    );
    assert_eq!(
        progress(&pool, a, "moves_master").await.unwrap().0,
        3,
        "an emote before the imported last minute is not counted"
    );
    assert!(granted(&pool, a).await.is_empty(), "no tier newly reached");
}

async fn deploy(
    pool: &PgPool,
    entity_type: &str,
    deployer: &str,
    pointers: &[&str],
    metadata: Value,
) {
    sqlx::query(
        "INSERT INTO content.deployments (deployer_address, version, entity_type, entity_id, entity_metadata, entity_timestamp, entity_pointers, local_timestamp, auth_chain) \
         VALUES ($1, 'v3', $2, $3, $4::json, timezone('UTC', now()), $5, timezone('UTC', now()), '[]'::json)",
    )
    .bind(deployer)
    .bind(entity_type)
    .bind(format!("bafy{}", Uuid::new_v4().simple()))
    .bind(metadata)
    .bind(pointers.iter().map(|s| s.to_string()).collect::<Vec<_>>())
    .execute(pool)
    .await
    .unwrap();
}

async fn wearable(pool: &PgPool, urn: &str, rarity: &str) {
    let entity = format!("bafyw{}", Uuid::new_v4().simple());
    sqlx::query(
        "INSERT INTO content.deployments (deployer_address, version, entity_type, entity_id, entity_metadata, entity_timestamp, entity_pointers, local_timestamp, auth_chain) \
         VALUES ('0x0', 'v3', 'wearable', $1, $2::json, timezone('UTC', now()), ARRAY[$3], timezone('UTC', now()), '[]'::json)",
    )
    .bind(&entity)
    .bind(json!({"v": {"id": urn, "rarity": rarity}}))
    .bind(urn)
    .execute(pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO content.active_pointers (pointer, entity_id, entity_type) VALUES ($1, $2, 'wearable')")
        .bind(urn)
        .bind(&entity)
        .execute(pool)
        .await
        .unwrap();
}

#[tokio::test]
async fn deployments_start_at_the_head_then_grant_by_entity_type() {
    let Some(pool) = private_pool("badges_deploy").await else {
        return;
    };
    let p = pass(&pool);
    let a = fresh_address();
    let a = a.as_str();
    deploy(&pool, "scene", a, &["1,1"], json!({})).await;
    assert_eq!(deployments::batch(&p).await.unwrap(), (0, false));
    let head: i64 = sqlx::query_scalar("SELECT max(id)::bigint FROM content.deployments")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(cursor(&pool, deployments::SOURCE).await, Some((head, 0)));
    assert_eq!(
        progress(&pool, a, "land_architect").await,
        None,
        "no history replay"
    );

    let urns: Vec<String> = (0..3)
        .map(|i| format!("urn:decentraland:matic:collections-v2:0x{:040x}:{i}", 0xabc))
        .collect();
    for urn in &urns {
        wearable(&pool, urn, "legendary").await;
    }
    let worn: Vec<String> = urns.iter().map(|u| format!("{u}:77")).collect();
    let upper = a.to_uppercase().replace("0X", "0x");
    deploy(
        &pool,
        "profile",
        &upper,
        &[&upper],
        json!({"v": {"avatars": [{"description": "builder", "avatar": {"wearables": worn}}]}}),
    )
    .await;
    deploy(
        &pool,
        "store",
        &upper,
        &[&format!(
            "urn:decentraland:off-chain:marketplace-stores:{a}"
        )],
        json!({}),
    )
    .await;
    deploy(&pool, "scene", a, &["2,2"], json!({})).await;
    // history a syncing node stores now: an old entity timestamp
    let b = fresh_address();
    sqlx::query(
        "INSERT INTO content.deployments (deployer_address, version, entity_type, entity_id, entity_metadata, entity_timestamp, entity_pointers, local_timestamp, auth_chain) \
         VALUES ($1, 'v3', 'scene', $2, '{}'::json, timezone('UTC', now()) - interval '30 days', ARRAY['3,3'], timezone('UTC', now()), '[]'::json)",
    )
    .bind(b.as_str())
    .bind(format!("bafyold{}", Uuid::new_v4().simple()))
    .execute(&pool)
    .await
    .unwrap();
    drain_deployments(&p).await;

    assert!(progress(&pool, a, "land_architect").await.unwrap().2);
    assert_eq!(
        progress(&pool, a, "profile_pro").await,
        Some((1, json!({"steps": 1, "description_added": "builder"}), true))
    );
    let legendary = progress(&pool, a, "legendary_look").await.unwrap();
    assert!(legendary.2);
    assert_eq!(legendary.1["completed_with"], json!(urns));
    assert_eq!(progress(&pool, a, "epic_ensemble").await, None);
    assert_eq!(
        progress(&pool, a, "open_for_business").await,
        Some((1, json!({"steps": 1, "store_completed": true}), false))
    );
    assert_eq!(
        progress(&pool, b.as_str(), "land_architect").await,
        None,
        "history is passed"
    );
    assert_eq!(
        granted(&pool, a).await,
        ["LAND Architect", "Legendary Look", "Profile Pro"]
    );
}

#[tokio::test]
async fn an_archived_badge_is_not_awarded() {
    let Some(pool) = private_pool("badges_archived").await else {
        return;
    };
    let p = pass(&pool);
    deployments::batch(&p).await.unwrap();
    sqlx::query("UPDATE social.badge_definitions SET archived_at = timezone('UTC', now()) WHERE id = 'land_architect'")
        .execute(&pool)
        .await
        .unwrap();
    let a = fresh_address();
    deploy(&pool, "scene", a.as_str(), &["9,9"], json!({})).await;
    assert_eq!(drain_deployments(&p).await, 1, "the row is passed");
    assert_eq!(progress(&pool, a.as_str(), "land_architect").await, None);
    assert!(granted(&pool, a.as_str()).await.is_empty());
}

async fn chain_ready(pool: &PgPool, indexer: &str, block: i64) {
    sqlx::query(
        "INSERT INTO shop.chain_cursor (indexer, network, start_block, last_block, fold_through, folded_block, folded_at, full_history) \
         VALUES ($1, 'matic', 0, $2, $2, $2, timezone('UTC', now()), true) \
         ON CONFLICT (indexer, network) DO UPDATE SET last_block = $2, fold_through = $2, folded_block = $2, folded_at = timezone('UTC', now())",
    )
    .bind(indexer)
    .bind(block)
    .execute(pool)
    .await
    .unwrap();
}

async fn item(pool: &PgPool, contract: &str, item_id: i64, item_type: &str, creator: &str) {
    sqlx::query(
        "INSERT INTO shop.items (network, contract_address, item_id, chain_id, item_type, urn, creator, rarity, total_supply, max_supply, available, price, \
            name, category, search_is_collection_approved, created_at, updated_at, reviewed_at, mirrored_at) \
         VALUES ('matic', $1, $2, 137, $3, $4, $5, 'common', 0, 10, 10, 0, 'Item', 'hat', true, 1700000000, 1, 1, timezone('UTC', now()))",
    )
    .bind(contract)
    .bind(item_id)
    .bind(item_type)
    .bind(format!("urn:decentraland:matic:collections-v2:{contract}:{item_id}"))
    .bind(creator)
    .execute(pool)
    .await
    .unwrap();
}

fn contract() -> String {
    format!("0x{}", &Uuid::new_v4().simple().to_string()[..32]) + "00000000"
}

#[tokio::test]
async fn sales_count_each_transaction_once_per_category() {
    let Some(pool) = private_pool("badges_sales").await else {
        return;
    };
    let p = pass(&pool);
    for (log, indexer) in [(Log::Sale, "sales"), (Log::Trade, "trade_events")] {
        assert_eq!(
            sales::batch(&p, log).await.unwrap(),
            (0, false),
            "not published"
        );
        chain_ready(&pool, indexer, 100).await;
        sales::batch(&p, log).await.unwrap();
        assert_eq!(cursor(&pool, log.source()).await, Some((100, i32::MAX)));
    }
    let c = contract();
    let creator = fresh_address();
    item(&pool, &c, 1, "emote_v1", creator.as_str()).await;
    item(&pool, &c, 2, "wearable_v2", creator.as_str()).await;
    let buyer = fresh_address();
    let b = buyer.as_str();
    let tx1 = format!("0x{}", "1".repeat(64));
    let tx2 = format!("0x{}", "2".repeat(64));
    for (block, log, item_id, tx) in [
        (101, 0, 1, &tx1),
        (101, 1, 1, &tx1),
        (101, 2, 2, &tx1),
        (102, 0, 1, &tx2),
    ] {
        sqlx::query(
            "INSERT INTO shop.sale_event (network, block_number, log_index, block_time, kind, emitter, contract_address, token_id, item_id, buyer, tx_hash) \
             VALUES ('matic', $1, $2, 1700000000, 'mint', $3, $3, 1, $4, $5, $6)",
        )
        .bind(block)
        .bind(log)
        .bind(&c)
        .bind(item_id)
        .bind(b)
        .bind(tx)
        .execute(&pool)
        .await
        .unwrap();
    }
    assert_eq!(
        sales::batch(&p, Log::Sale).await.unwrap().0,
        0,
        "past the published block"
    );
    chain_ready(&pool, "sales", 102).await;
    assert_eq!(sales::batch(&p, Log::Sale).await.unwrap().0, 4);
    assert_eq!(cursor(&pool, Log::Sale.source()).await, Some((102, 0)));
    let emotes = progress(&pool, b, "emotionista").await.unwrap();
    assert_eq!(emotes.0, 2, "two transactions");
    assert_eq!(
        emotes.1["transactions_emotes_purchase"],
        json!([
            {"transactionHash": tx1, "saleAt": 1_700_000_000_000_i64},
            {"transactionHash": tx2, "saleAt": 1_700_000_000_000_i64}
        ])
    );
    assert_eq!(progress(&pool, b, "fashionista").await.unwrap().0, 1);

    // an off-chain listing: the caller buys a wearable
    let seller = fresh_address();
    let tx3 = format!("0x{}", "3".repeat(64));
    sqlx::query(
        "INSERT INTO shop.trade_event (network, block_number, log_index, block_time, marketplace_address, kind, caller, hashed_signature, signer, assets, tx_hash) \
         VALUES ('matic', 103, 0, 1700000100, '0xm', 'traded', $1, '0xsig', $2, $3, $4)",
    )
    .bind(b)
    .bind(seller.as_str())
    .bind(json!({"sent": [{"assetType": 4, "contractAddress": c, "value": "2"}],
                 "received": [{"assetType": 1, "contractAddress": "0xmana", "value": "5"}]}))
    .bind(&tx3)
    .execute(&pool)
    .await
    .unwrap();
    chain_ready(&pool, "trade_events", 103).await;
    assert_eq!(sales::batch(&p, Log::Trade).await.unwrap().0, 1);
    assert_eq!(progress(&pool, b, "fashionista").await.unwrap().0, 2);
    assert_eq!(progress(&pool, seller.as_str(), "fashionista").await, None);
    assert_eq!(
        granted(&pool, b).await,
        ["Emotionista Starter", "Fashionista Starter"]
    );
}

async fn mirror_ready(pool: &PgPool, mirror: &str) {
    sqlx::query(
        "INSERT INTO shop.mirror_state (mirror, network, source_url, last_complete_pass_at) \
         VALUES ($1, 'matic', 'https://subgraph.example', timezone('UTC', now()))",
    )
    .bind(mirror)
    .execute(pool)
    .await
    .unwrap();
}

#[tokio::test]
async fn recounts_bring_progress_up_silently_first_then_notify() {
    let Some(pool) = private_pool("badges_recount").await else {
        return;
    };
    let p = pass(&pool);
    let creator = fresh_address();
    let cr = creator.as_str();
    let c = contract();
    item(&pool, &c, 0, "wearable_v2", cr).await;
    assert_eq!(
        recount::run_one(&p, Recount::WearableDesigner)
            .await
            .unwrap(),
        0,
        "the mirror is not ready"
    );
    mirror_ready(&pool, "items").await;
    mirror_ready(&pool, "collections").await;
    assert_eq!(
        recount::run_one(&p, Recount::WearableDesigner)
            .await
            .unwrap(),
        1
    );
    assert_eq!(
        progress(&pool, cr, "wearable_designer").await.unwrap().1,
        json!({"steps": 1, "published_wearables": [{"itemId": format!("{c}-0"), "createdAt": 1_700_000_000_000_i64}]})
    );
    assert_eq!(
        tiers(&pool, cr, "wearable_designer").await,
        ["wearable-designer-starter"]
    );
    assert!(
        granted(&pool, cr).await.is_empty(),
        "the first pass notifies nothing"
    );
    assert_eq!(
        cursor(&pool, Recount::WearableDesigner.source()).await,
        Some((1, 0))
    );

    for i in 1..5 {
        item(&pool, &c, i, "smart_wearable_v1", cr).await;
    }
    item(&pool, &c, 9, "emote_v1", cr).await;
    assert_eq!(
        recount::run_one(&p, Recount::WearableDesigner)
            .await
            .unwrap(),
        1
    );
    assert_eq!(
        recount::run_one(&p, Recount::WearableDesigner)
            .await
            .unwrap(),
        0,
        "converged"
    );
    assert_eq!(progress(&pool, cr, "wearable_designer").await.unwrap().0, 5);
    assert_eq!(granted(&pool, cr).await, ["Wearable Designer Bronze"]);
    recount::run_one(&p, Recount::EmoteCreator).await.unwrap();
    assert_eq!(progress(&pool, cr, "emote_creator").await.unwrap().0, 1);

    // a collection completes open for business with the store already done
    sqlx::query(
        "INSERT INTO social.user_badge_progress (address, badge_id, steps_done, progress) \
         VALUES ($1, 'open_for_business', 1, '{\"steps\": 1, \"store_completed\": true}')",
    )
    .bind(cr)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO shop.collections (network, contract_address, chain_id, name, creator, owner, urn, is_completed, is_approved, is_editable, created_at, updated_at, reviewed_at, mirrored_at) \
         VALUES ('matic', $1, 137, 'C', $2, $2, 'urn:c', true, true, false, 1, 1, 1, timezone('UTC', now()))",
    )
    .bind(&c)
    .bind(cr)
    .execute(&pool)
    .await
    .unwrap();
    recount::run_one(&p, Recount::Collections).await.unwrap();
    assert_eq!(
        progress(&pool, cr, "open_for_business").await,
        Some((
            2,
            json!({"steps": 2, "store_completed": true, "collection_submitted": true}),
            true
        ))
    );

    // referrals: the accepted count, not one per event
    let referrer = fresh_address();
    for _ in 0..5 {
        sqlx::query("INSERT INTO social.referral_progress (invited_user, referrer, status, tier_granted) VALUES ($1, $2, 'tier_granted', true)")
            .bind(fresh_address().as_str())
            .bind(referrer.as_str())
            .execute(&pool)
            .await
            .unwrap();
    }
    sqlx::query("INSERT INTO social.referral_progress (invited_user, referrer) VALUES ($1, $2)")
        .bind(fresh_address().as_str())
        .bind(referrer.as_str())
        .execute(&pool)
        .await
        .unwrap();
    recount::run_one(&p, Recount::Referrals).await.unwrap();
    assert_eq!(
        progress(&pool, referrer.as_str(), "referral")
            .await
            .unwrap()
            .0,
        5
    );
    assert_eq!(
        tiers(&pool, referrer.as_str(), "referral").await,
        ["referral-starter"]
    );
}
