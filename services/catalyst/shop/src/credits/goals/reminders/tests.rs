use super::*;
use crate::credits::{RawSettings, Settings, State};
use crate::test_db::OwnDatabase;

fn at(v: &str) -> NaiveDateTime {
    NaiveDateTime::parse_from_str(v, "%Y-%m-%d %H:%M").unwrap()
}

#[test]
fn reminders_follow_the_source_schedule() {
    // 2026-11-19 is a Thursday.
    assert!(!Kind::DoNotMissOut.due(at("2026-11-19 08:59")));
    assert!(Kind::DoNotMissOut.due(at("2026-11-19 09:00")));
    assert!(!Kind::DoNotMissOut.due(at("2026-11-20 10:00")));
    assert!(!Kind::CompleteGoals.due(at("2026-11-20 11:00")));
    assert!(Kind::CompleteGoals.due(at("2026-11-20 12:00")));
    assert!(Kind::Claim.due(at("2026-11-22 09:30")));
    assert!(!Kind::Claim.due(at("2026-11-21 09:30")));
    assert!(Kind::Usage.due(at("2026-11-17 09:00")));
    assert!(!Kind::Usage.due(at("2026-11-17 08:00")));
    assert!(Kind::Usage24Hours.due(at("2026-11-17 00:00")));
}

#[test]
fn keys_are_per_season_and_week_like_the_source() {
    assert_eq!(
        Kind::Claim.key("0xa", 3, Some(2)),
        "0xa-claim-credits-reminder-3-2"
    );
    assert_eq!(Kind::Usage.key("0xa", 3, None), "0xa-usage-reminder-3");
    assert_eq!(
        Kind::Usage24Hours.key("0xa", 3, None),
        "0xa-usage-24-hours-reminder-3"
    );
}

#[test]
fn balances_render_like_format_ether() {
    assert_eq!(ether_number("100000000000000000000").unwrap(), json!(100));
    assert_eq!(ether_number("1500000000000000000").unwrap(), json!(1.5));
    assert_eq!(ether_number("0").unwrap(), json!(0));
}

const MANAGER: &str = "0x00000000000000000000000000000000000000b1";
fn addr(n: u8) -> String {
    format!("0x{:040x}", n)
}
fn state(pool: sqlx::PgPool, enabled: bool) -> SharedState {
    let settings = Settings::parse(RawSettings {
        writes_enabled: true,
        credits_manager_contract: Some(MANAGER.into()),
        ..RawSettings::default()
    })
    .unwrap();
    let state = Arc::new(State::with_sources(
        pool,
        crate::http::Outbound::for_tests(),
        settings,
        Arc::new(crate::credits::tests::FakeMarket::default()),
        Arc::new(crate::credits::tests::MockBroker::configured()),
        None,
    ));
    state.goal_flags.set_documents(
        Some(json!({"flags":{"dapps-credits-server":enabled},"variants":{}})),
        Some(json!({"flags":{},"variants":{}})),
    );
    state
}
async fn exec(pool: &sqlx::PgPool, sql: &'static str, address: &str) {
    sqlx::query(sql).bind(address).execute(pool).await.unwrap();
}
async fn wallet(pool: &sqlx::PgPool, n: u8) -> String {
    let a = addr(n);
    exec(
        pool,
        "INSERT INTO credits.wallets(address) VALUES($1) ON CONFLICT DO NOTHING",
        &a,
    )
    .await;
    a
}
async fn progress(pool: &sqlx::PgPool, a: &str, week: i32, steps: i32) {
    sqlx::query("INSERT INTO credits.goal_progress(address,season_id,week_id,goal_id,title,thumbnail,total_steps,reward_wei,completed_steps,completed_at) VALUES($1,1,$2,'take-photo','Take a photo','p.png',2,0,$3,CASE WHEN $3=2 THEN timezone('UTC',now()) END)")
        .bind(a).bind(week).bind(steps).execute(pool).await.unwrap();
}
async fn grant(
    pool: &sqlx::PgPool,
    a: &str,
    week: i32,
    wei: &str,
    expires_in: &str,
    claimed: bool,
) {
    let salt = format!("0x{}", "0".repeat(24) + &a[2..]);
    sqlx::query("INSERT INTO credits.goal_grants(address,season_id,week_id,goal_id,salt,chain_id,contract,credit_hash,amount_wei,signature,created_at,expires_at,claimed_at) VALUES($1,1,$2,'take-photo',$3,137,$4,$3,$5::numeric,'0x',timezone('UTC',now()),timezone('UTC',now())+$6::interval,CASE WHEN $7 THEN timezone('UTC',now()) END)")
        .bind(a).bind(week).bind(&salt).bind(MANAGER).bind(wei).bind(expires_in).bind(claimed).execute(pool).await.unwrap();
}
async fn sent(pool: &sqlx::PgPool) -> Vec<(String, String, Value)> {
    sqlx::query_as("SELECT address,type,metadata FROM social.notifications WHERE type LIKE 'credits_reminder_%' ORDER BY type,address")
        .fetch_all(pool).await.unwrap()
}

#[tokio::test]
async fn reminders_pick_eligible_wallets_once_with_source_metadata() {
    let Some(db) = OwnDatabase::create("credits_reminders").await else {
        return;
    };
    let pool = &db.pool;
    sqlx::query("INSERT INTO credits.seasons(id,name,start_date,end_date,max_mana_wei) VALUES(1,'Test',timezone('UTC',now())-interval '1 day',timezone('UTC',now())+interval '21 days',0)").execute(pool).await.unwrap();
    sqlx::query("INSERT INTO credits.season_goals(season_id,goal_id,title,thumbnail,total_steps,reward_wei,start_week,end_week) VALUES(1,'take-photo','Take a photo','p.png',2,0,1,20)").execute(pool).await.unwrap();
    let now = crate::credits::store::now();
    let season = current_or_last(pool, now).await.unwrap().unwrap();
    let week = season.week(now).0;
    let untouched = wallet(pool, 1).await;
    let started = wallet(pool, 2).await;
    progress(pool, &started, week, 1).await;
    let unclaimed = wallet(pool, 3).await;
    progress(pool, &unclaimed, week, 2).await;
    grant(pool, &unclaimed, week, "1", "30 days", false).await;
    let opted_out = wallet(pool, 4).await;
    exec(pool, "INSERT INTO credits.program_opt_outs(address,opted_out_at) VALUES($1,timezone('UTC',now()))", &opted_out).await;
    let tomorrow = wallet(pool, 5).await;
    progress(pool, &tomorrow, week, 2).await;
    grant(
        pool,
        &tomorrow,
        week,
        "1500000000000000000",
        "12 hours",
        true,
    )
    .await;
    let next_week = wallet(pool, 6).await;
    progress(pool, &next_week, week, 2).await;
    grant(
        pool,
        &next_week,
        week,
        "100000000000000000000",
        "3 days",
        true,
    )
    .await;
    let used = wallet(pool, 7).await;
    progress(pool, &used, week, 2).await;
    grant(pool, &used, week, "1000000000000000000", "3 days", true).await;
    let salt = format!("0x{}", "0".repeat(24) + &used[2..]);
    sqlx::query("INSERT INTO credits.usages(chain_id,block_number,log_index,contract,tx_hash,sender,credit_id,salt,credit_value,credit_expires_at,value_wei) VALUES(137,1,0,$1,$2,$3,$2,$2,1,0,1)")
        .bind(MANAGER).bind(&salt).bind(&used).execute(pool).await.unwrap();

    let disabled = state(pool.clone(), false);
    for kind in Kind::ALL {
        assert_eq!(run(&disabled, kind, now).await.unwrap(), 0);
    }
    let state = state(pool.clone(), true);
    for _ in 0..2 {
        for kind in Kind::ALL {
            run(&state, kind, now).await.unwrap();
        }
    }
    let rows = sent(pool).await;
    let ids: Vec<(String, String)> = rows
        .iter()
        .map(|(a, t, _)| (t.clone(), a.clone()))
        .collect();
    let pair = |t: &str, a: &String| (format!("credits_reminder_{t}"), a.clone());
    assert_eq!(
        ids,
        vec![
            pair("claim_credits", &unclaimed),
            pair("complete_goals", &started),
            pair("do_not_miss_out", &untouched),
            pair("usage", &tomorrow),
            pair("usage", &next_week),
            pair("usage_24_hours", &tomorrow),
        ]
    );
    let meta = |i: usize| rows[i].2.clone();
    assert_eq!(
        meta(0),
        json!({"seasonId":1,"weekNumber":week,"link":"https://decentraland.org/play"})
    );
    assert_eq!(
        meta(1),
        json!({"seasonId":1,"weekNumber":week,"pendingGoalIds":["take-photo"],"link":"https://decentraland.org/play"})
    );
    assert_eq!(meta(2), json!({"link":"https://decentraland.org/play"}));
    let day = |e: TimeDelta| (now + e).format("%B %-d").to_string();
    let weekday = |e: TimeDelta| (now + e).format("%A").to_string();
    assert_eq!(
        meta(3),
        json!({"expirationDate":day(TimeDelta::hours(12)),"expirationDay":weekday(TimeDelta::hours(12)),"balance":1.5,"link":"https://decentraland.org/shop"})
    );
    assert_eq!(
        meta(4),
        json!({"expirationDate":day(TimeDelta::days(3)),"expirationDay":weekday(TimeDelta::days(3)),"balance":100,"link":"https://decentraland.org/shop"})
    );
    assert_eq!(
        meta(5),
        json!({"expirationDate":day(TimeDelta::hours(12)),"balance":1.5,"link":"https://decentraland.org/shop"})
    );
    let keys: i64 = sqlx::query_scalar("SELECT count(DISTINCT broadcast_key) FROM social.notifications WHERE type LIKE 'credits_reminder_%'").fetch_one(pool).await.unwrap();
    assert_eq!(keys, 6);

    sqlx::query("UPDATE credits.seasons SET end_date=timezone('UTC',now())-interval '1 minute'")
        .execute(pool)
        .await
        .unwrap();
    sqlx::query("DELETE FROM social.notifications")
        .execute(pool)
        .await
        .unwrap();
    let later = crate::credits::store::now();
    for kind in [Kind::DoNotMissOut, Kind::CompleteGoals, Kind::Claim] {
        assert_eq!(run(&state, kind, later).await.unwrap(), 0);
    }
    db.drop().await;
}
