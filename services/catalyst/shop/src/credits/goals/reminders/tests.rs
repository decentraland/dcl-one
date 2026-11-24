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
fn usage_keys_carry_the_full_expiry() {
    let expires = at("2026-12-03 00:00");
    assert_eq!(
        Kind::Usage.usage_key("0xa", 2, expires),
        "0xa-usage-reminder-2-expiry-1796256000000000"
    );
    assert_eq!(
        Kind::Usage24Hours.usage_key("0xa", 2, expires),
        "0xa-usage-24-hours-reminder-2-expiry-1796256000000000"
    );
    // Expiries on the same day, down to the microsecond, are distinct notices.
    let later = expires + TimeDelta::microseconds(1);
    assert_ne!(
        Kind::Usage.usage_key("0xa", 2, expires),
        Kind::Usage.usage_key("0xa", 2, later)
    );
    let ids = |e| {
        let key = Kind::Usage.usage_key("0xa", 2, e);
        Uuid::new_v5(&NOTIFICATION_NAMESPACE, key.as_bytes())
    };
    assert_ne!(ids(expires), ids(later));
}

#[test]
fn balances_render_like_format_ether() {
    assert_eq!(ether_number("100000000000000000000").unwrap(), json!(100));
    assert_eq!(ether_number("1500000000000000000").unwrap(), json!(1.5));
    assert_eq!(ether_number("0").unwrap(), json!(0));
}

#[test]
fn the_gate_applies_the_switch_and_allowlist() {
    let open = Gate {
        disabled: false,
        allowlist: None,
    };
    assert_eq!(open.refusal("0xa"), None);
    let listed = Gate {
        disabled: false,
        allowlist: Some(vec!["0xa".into()]),
    };
    assert_eq!(listed.refusal("0xa"), None);
    assert_eq!(listed.refusal("0xb"), Some("not-allowed"));
    let off = Gate {
        disabled: true,
        allowlist: None,
    };
    assert_eq!(off.refusal("0xa"), Some("disabled"));
}

const MANAGER: &str = "0x00000000000000000000000000000000000000b1";
const PLAY: &str = "https://decentraland.org/play";
const SHOP: &str = "https://decentraland.org/shop";
/// Thursday of the season's third week (weeks start on Monday 2026-11-02).
fn now() -> NaiveDateTime {
    at("2026-11-19 10:00")
}
fn addr(n: u8) -> String {
    format!("0x{n:040x}")
}
fn state(pool: sqlx::PgPool, dapps: Value, explorer: Value) -> SharedState {
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
    state.goal_flags.set_documents(Some(dapps), Some(explorer));
    state
}
fn enabled(pool: &sqlx::PgPool) -> SharedState {
    state(
        pool.clone(),
        json!({"flags":{"dapps-credits-server":true},"variants":{}}),
        json!({"flags":{},"variants":{}}),
    )
}
async fn season(pool: &sqlx::PgPool, id: i32, start: &str, end: &str) {
    sqlx::query("INSERT INTO credits.seasons(id,name,start_date,end_date,max_mana_wei) VALUES($1,'Test',$2,$3,0)")
        .bind(id).bind(at(start)).bind(at(end)).execute(pool).await.unwrap();
    sqlx::query("INSERT INTO credits.season_goals(season_id,goal_id,title,thumbnail,total_steps,reward_wei,start_week,end_week) VALUES($1,'take-photo','Take a photo','p.png',2,0,1,20)")
        .bind(id).execute(pool).await.unwrap();
}
async fn wallet(pool: &sqlx::PgPool, n: u8) -> String {
    let a = addr(n);
    sqlx::query("INSERT INTO credits.wallets(address) VALUES($1)")
        .bind(&a)
        .execute(pool)
        .await
        .unwrap();
    a
}
async fn progress(pool: &sqlx::PgPool, a: &str, season: i32, week: i32, steps: i32) {
    sqlx::query("INSERT INTO credits.goal_progress(address,season_id,week_id,goal_id,title,thumbnail,total_steps,reward_wei,completed_steps,completed_at) VALUES($1,$2,$3,'take-photo','Take a photo','p.png',2,0,$4,CASE WHEN $4=2 THEN $5 END)")
        .bind(a).bind(season).bind(week).bind(steps).bind(now()).execute(pool).await.unwrap();
}
async fn grant(
    pool: &sqlx::PgPool,
    a: &str,
    season: i32,
    week: i32,
    wei: &str,
    expires: NaiveDateTime,
    claimed: bool,
) {
    let salt = grant_salt(a, season, week);
    sqlx::query("INSERT INTO credits.goal_grants(address,season_id,week_id,goal_id,salt,chain_id,contract,credit_hash,amount_wei,signature,created_at,expires_at,claimed_at) VALUES($1,$2,$3,'take-photo',$4,137,$5,$4,$6::numeric,'0x',$7,$8,$9)")
        .bind(a).bind(season).bind(week).bind(&salt).bind(MANAGER).bind(wei).bind(now() - TimeDelta::days(1)).bind(expires).bind(claimed.then(now)).execute(pool).await.unwrap();
}
fn grant_salt(a: &str, season: i32, week: i32) -> String {
    format!("0x{season:012x}{week:012x}{}", &a[2..])
}
async fn sent(pool: &sqlx::PgPool) -> Vec<(String, String, Value)> {
    sqlx::query_as("SELECT type,address,metadata FROM social.notifications WHERE type LIKE 'credits_reminder_%' ORDER BY type,address")
        .fetch_all(pool).await.unwrap()
}
fn kinds(rows: &[(String, String, Value)]) -> Vec<(String, String)> {
    rows.iter()
        .map(|(t, a, _)| (t.clone(), a.clone()))
        .collect()
}
fn pair(t: &str, a: &str) -> (String, String) {
    (format!("credits_reminder_{t}"), a.to_string())
}

#[tokio::test]
async fn weekly_recipients_depend_on_completion_not_claim_eligibility() {
    let Some(db) = OwnDatabase::create("reminder_recipients").await else {
        return;
    };
    let pool = &db.pool;
    season(pool, 1, "2026-11-02 00:00", "2026-11-30 00:00").await;
    let mut unfinished = vec![];
    let mut unclaimed = vec![];
    // Each claim restriction is independent of whether a reminder is due.
    for restriction in 0..5 {
        for steps in [1, 2] {
            let address = wallet(pool, restriction * 2 + steps).await;
            progress(pool, &address, 1, 3, i32::from(steps)).await;
            match restriction {
                1 => {
                    sqlx::query(
                        "UPDATE credits.wallets SET has_started_program=false WHERE address=$1",
                    )
                    .bind(&address)
                    .execute(pool)
                    .await
                    .unwrap();
                }
                2 => {
                    sqlx::query(
                        "UPDATE credits.wallets SET is_blocked_for_claiming=true WHERE address=$1",
                    )
                    .bind(&address)
                    .execute(pool)
                    .await
                    .unwrap();
                }
                3 => {
                    sqlx::query(
                        "INSERT INTO credits.wallet_flags(address,source) VALUES($1,'test')",
                    )
                    .bind(&address)
                    .execute(pool)
                    .await
                    .unwrap();
                }
                4 => {
                    sqlx::query(
                        "INSERT INTO credits.program_opt_outs(address,opted_out_at) VALUES($1,$2)",
                    )
                    .bind(&address)
                    .bind(now())
                    .execute(pool)
                    .await
                    .unwrap();
                }
                _ => {}
            }
            if steps == 1 {
                unfinished.push(address);
            } else {
                grant(
                    pool,
                    &address,
                    1,
                    3,
                    "1",
                    now() + TimeDelta::days(10),
                    false,
                )
                .await;
                unclaimed.push(address);
            }
        }
    }
    let state = enabled(pool);
    let gate = gate(&state, &mut pool.acquire().await.unwrap())
        .await
        .unwrap();
    for _ in 0..2 {
        for kind in [Kind::DoNotMissOut, Kind::CompleteGoals, Kind::Claim] {
            run(&state, &gate, kind, now(), &CancellationToken::new())
                .await
                .unwrap();
        }
    }
    let mut expected: Vec<_> = unfinished
        .iter()
        .map(|a| pair("do_not_miss_out", a))
        .chain(unclaimed.iter().map(|a| pair("claim_credits", a)))
        .collect();
    expected.sort();
    assert_eq!(kinds(&sent(pool).await), expected);
    db.drop().await;
}

#[tokio::test]
async fn usage_requires_selected_season_progress_and_observes_expiry_boundaries() {
    let Some(db) = OwnDatabase::create("reminder_usage_bounds").await else {
        return;
    };
    let pool = &db.pool;
    season(pool, 1, "2026-10-05 00:00", "2026-11-09 00:00").await;
    season(pool, 2, "2026-11-09 00:00", "2026-12-21 00:00").await;
    for (n, expires, claimed) in [
        (1, now(), true),
        (2, now() + TimeDelta::days(1), true),
        (3, now() + TimeDelta::days(1) + TimeDelta::seconds(1), true),
        (4, now() + TimeDelta::days(7), true),
        (5, now() + TimeDelta::days(7) + TimeDelta::seconds(1), true),
        (6, now() + TimeDelta::hours(12), false),
        (7, now() + TimeDelta::hours(12), true),
        (8, now() + TimeDelta::hours(12), true),
        (9, now() + TimeDelta::hours(12), true),
        (10, now() + TimeDelta::hours(12), true),
    ] {
        let a = wallet(pool, n).await;
        progress(pool, &a, 1, 5, 2).await;
        if n != 9 {
            // Any week of the selected season counts, even before this week.
            progress(pool, &a, 2, 1, 0).await;
        }
        grant(pool, &a, 1, 5, "1000000000000000000", expires, claimed).await;
        if n == 7 || n == 8 {
            sqlx::query("INSERT INTO credits.usages(chain_id,block_number,log_index,contract,tx_hash,sender,credit_id,salt,credit_value,credit_expires_at,value_wei) SELECT chain_id,1,$2,contract,credit_hash,address,credit_hash,salt,amount_wei,extract(epoch FROM expires_at),$3::numeric FROM credits.goal_grants WHERE address=$1")
                .bind(&a).bind(i32::from(n)).bind(if n == 7 { "400000000000000000" } else { "1000000000000000000" }).execute(pool).await.unwrap();
        }
        if n == 10 {
            sqlx::query("UPDATE credits.wallets SET has_started_program=false,is_blocked_for_claiming=true WHERE address=$1")
                .bind(&a).execute(pool).await.unwrap();
            sqlx::query("INSERT INTO credits.program_opt_outs(address,opted_out_at) VALUES($1,$2)")
                .bind(&a)
                .bind(now())
                .execute(pool)
                .await
                .unwrap();
            sqlx::query("INSERT INTO credits.wallet_flags(address,source) VALUES($1,'test')")
                .bind(&a)
                .execute(pool)
                .await
                .unwrap();
        }
    }
    let state = enabled(pool);
    let gate = gate(&state, &mut pool.acquire().await.unwrap())
        .await
        .unwrap();
    for _ in 0..2 {
        for kind in [Kind::Usage, Kind::Usage24Hours] {
            run(&state, &gate, kind, now(), &CancellationToken::new())
                .await
                .unwrap();
        }
    }
    assert_eq!(
        kinds(&sent(pool).await),
        vec![
            pair("usage", &addr(2)),
            pair("usage", &addr(3)),
            pair("usage", &addr(4)),
            pair("usage", &addr(10)),
            pair("usage_24_hours", &addr(2)),
            pair("usage_24_hours", &addr(10)),
        ]
    );
    db.drop().await;
}

#[tokio::test]
async fn all_reminder_kinds_preserve_email_preferences_and_deduplicate_deliveries() {
    let Some(db) = OwnDatabase::create("reminder_preferences").await else {
        return;
    };
    let pool = &db.pool;
    let state = enabled(pool);
    for n in 0..5 {
        let address = addr(n);
        let mut details = json!({"message_type":{}});
        if n == 1 {
            details["ignore_all_email"] = json!(true);
        }
        if n == 2 || n == 4 {
            for kind in Kind::ALL {
                details["message_type"][kind.notification_type()] =
                    json!({"email":n == 4,"in_app":false});
            }
            details["ignore_all_in_app"] = json!(true);
        }
        sqlx::query("INSERT INTO social.subscriptions(address,email,details) VALUES($1,$2,$3)")
            .bind(&address)
            .bind((n != 3).then_some("reminders@example.org"))
            .bind(details)
            .execute(pool)
            .await
            .unwrap();
        // Exercise delivery policy for all five types. Eligibility is tested
        // separately: one supported goal cannot produce CompleteGoals.
        for replay in [false, true] {
            for kind in Kind::ALL {
                let key = kind.key(&address, 1, Some(3));
                assert_eq!(
                    send(&state, kind, &address, &key, json!({}), now())
                        .await
                        .unwrap(),
                    !replay
                );
            }
        }
        let deliveries: Vec<(String, Option<String>)> = sqlx::query_as("SELECT d.state,d.last_error FROM social.notification_deliveries d JOIN social.notifications n ON n.id=d.notification_id WHERE n.address=$1")
            .bind(&address).fetch_all(pool).await.unwrap();
        let expected = match n {
            1 => ("skipped".to_string(), Some("ignore_all_email".to_string())),
            2 => (
                "skipped".to_string(),
                Some("opted out of this type".to_string()),
            ),
            3 => (
                "skipped".to_string(),
                Some("no confirmed email".to_string()),
            ),
            _ => ("pending".to_string(), None),
        };
        assert_eq!(deliveries, vec![expected; Kind::ALL.len()]);
    }
    assert_eq!(sent(pool).await.len(), 5 * Kind::ALL.len());
    db.drop().await;
}

#[tokio::test]
async fn reminders_pick_eligible_wallets_once_with_source_metadata() {
    let Some(db) = OwnDatabase::create("credits_reminders").await else {
        return;
    };
    let pool = &db.pool;
    season(pool, 1, "2026-11-02 00:00", "2026-11-30 00:00").await;
    let untouched = wallet(pool, 1).await;
    progress(pool, &untouched, 1, 3, 0).await;
    let started = wallet(pool, 2).await;
    progress(pool, &started, 1, 3, 1).await;
    let unclaimed = wallet(pool, 3).await;
    progress(pool, &unclaimed, 1, 3, 2).await;
    grant(pool, &unclaimed, 1, 3, "1", at("2026-12-14 00:00"), false).await;
    let opted_out = wallet(pool, 4).await;
    progress(pool, &opted_out, 1, 3, 0).await;
    sqlx::query("INSERT INTO credits.program_opt_outs(address,opted_out_at) VALUES($1,$2)")
        .bind(&opted_out)
        .bind(now())
        .execute(pool)
        .await
        .unwrap();
    let tomorrow = wallet(pool, 5).await;
    progress(pool, &tomorrow, 1, 3, 2).await;
    let in_12h = now() + TimeDelta::hours(12);
    grant(pool, &tomorrow, 1, 3, "1500000000000000000", in_12h, true).await;
    let in_3d = now() + TimeDelta::days(3);
    let later = wallet(pool, 6).await;
    progress(pool, &later, 1, 3, 2).await;
    grant(pool, &later, 1, 3, "100000000000000000000", in_3d, true).await;
    let used = wallet(pool, 7).await;
    progress(pool, &used, 1, 3, 2).await;
    grant(pool, &used, 1, 3, "1000000000000000000", in_3d, true).await;
    let salt = grant_salt(&used, 1, 3);
    sqlx::query("INSERT INTO credits.usages(chain_id,block_number,log_index,contract,tx_hash,sender,credit_id,salt,credit_value,credit_expires_at,value_wei) VALUES(137,1,0,$1,$2,$3,$2,$2,1,0,1)")
        .bind(MANAGER).bind(&salt).bind(&used).execute(pool).await.unwrap();
    // No progress row this week: dormant, or only last week's row.
    wallet(pool, 8).await;
    let last_week = wallet(pool, 9).await;
    progress(pool, &last_week, 1, 2, 0).await;
    let blocked = wallet(pool, 10).await;
    progress(pool, &blocked, 1, 3, 0).await;
    sqlx::query("UPDATE credits.wallets SET is_blocked_for_claiming=true WHERE address=$1")
        .bind(&blocked)
        .execute(pool)
        .await
        .unwrap();
    let flagged = wallet(pool, 11).await;
    progress(pool, &flagged, 1, 3, 0).await;
    sqlx::query("INSERT INTO credits.wallet_flags(address,source) VALUES($1,'test')")
        .bind(&flagged)
        .execute(pool)
        .await
        .unwrap();

    let state = enabled(pool);
    let gate = gate(&state, &mut pool.acquire().await.unwrap())
        .await
        .unwrap();
    let cancel = CancellationToken::new();
    for _ in 0..2 {
        for kind in Kind::ALL {
            run(&state, &gate, kind, now(), &cancel).await.unwrap();
        }
    }
    let rows = sent(pool).await;
    assert_eq!(
        kinds(&rows),
        vec![
            pair("claim_credits", &unclaimed),
            pair("do_not_miss_out", &untouched),
            pair("do_not_miss_out", &started),
            pair("do_not_miss_out", &opted_out),
            pair("do_not_miss_out", &blocked),
            pair("do_not_miss_out", &flagged),
            pair("usage", &tomorrow),
            pair("usage", &later),
            pair("usage_24_hours", &tomorrow),
        ]
    );
    assert_eq!(rows[0].2, json!({"seasonId":1,"weekNumber":3,"link":PLAY}));
    for row in &rows[1..6] {
        assert_eq!(row.2, json!({"link":PLAY}));
    }
    assert_eq!(
        rows[6].2,
        json!({"expirationDate":"November 19","expirationDay":"Thursday","balance":1.5,"link":SHOP})
    );
    assert_eq!(
        rows[7].2,
        json!({"expirationDate":"November 22","expirationDay":"Sunday","balance":100,"link":SHOP})
    );
    assert_eq!(
        rows[8].2,
        json!({"expirationDate":"November 19","balance":1.5,"link":SHOP})
    );
    let keys: Vec<String> = sqlx::query_scalar("SELECT broadcast_key FROM social.notifications WHERE type LIKE 'credits_reminder_%' ORDER BY broadcast_key")
        .fetch_all(pool).await.unwrap();
    assert!(keys.contains(&format!("{untouched}-do-not-miss-out-reminder-1-3")));
    assert!(keys.contains(&Kind::Usage24Hours.usage_key(&tomorrow, 1, in_12h)));
    assert_eq!(keys.len(), 9);

    // Weekly reminders stop once the season is over.
    sqlx::query("DELETE FROM social.notifications")
        .execute(pool)
        .await
        .unwrap();
    let after_end = at("2026-11-30 10:00");
    for kind in [Kind::DoNotMissOut, Kind::CompleteGoals, Kind::Claim] {
        assert_eq!(
            run(&state, &gate, kind, after_end, &cancel).await.unwrap(),
            0
        );
    }
    db.drop().await;
}

#[tokio::test]
async fn usage_reminders_follow_the_credits_season_and_the_allowlist() {
    let Some(db) = OwnDatabase::create("credits_reminders_gate").await else {
        return;
    };
    let pool = &db.pool;
    season(pool, 1, "2026-10-05 00:00", "2026-11-09 00:00").await;
    season(pool, 2, "2026-11-09 00:00", "2026-12-21 00:00").await;
    let listed = wallet(pool, 1).await;
    let other = wallet(pool, 2).await;
    for a in [&listed, &other] {
        progress(pool, a, 1, 5, 2).await;
        grant(
            pool,
            a,
            1,
            5,
            "2000000000000000000",
            now() + TimeDelta::days(2),
            true,
        )
        .await;
        progress(pool, a, 2, 2, 0).await;
    }
    let state = state(
        pool.clone(),
        json!({"flags":{"dapps-credits-server":true},"variants":{}}),
        json!({"flags":{"explorer-alfa-marketplace-credits":true},"variants":{"explorer-alfa-marketplace-credits":{"name":"wallets","enabled":true,"payload":{"type":"string","value":listed}}}}),
    );
    let gate = gate(&state, &mut pool.acquire().await.unwrap())
        .await
        .unwrap();
    let cancel = CancellationToken::new();
    for kind in [Kind::Usage, Kind::DoNotMissOut] {
        run(&state, &gate, kind, now(), &cancel).await.unwrap();
    }
    assert_eq!(
        kinds(&sent(pool).await),
        vec![pair("do_not_miss_out", &listed), pair("usage", &listed)]
    );
    let keys: Vec<String> =
        sqlx::query_scalar("SELECT broadcast_key FROM social.notifications ORDER BY broadcast_key")
            .fetch_all(pool)
            .await
            .unwrap();
    assert_eq!(
        keys,
        vec![
            format!("{listed}-do-not-miss-out-reminder-2-2"),
            Kind::Usage.usage_key(&listed, 2, now() + TimeDelta::days(2)),
        ]
    );
    db.drop().await;
}

#[tokio::test]
async fn a_pass_runs_due_kinds_once_per_day() {
    let Some(db) = OwnDatabase::create("credits_reminders_pass").await else {
        return;
    };
    let pool = &db.pool;
    season(pool, 1, "2026-11-02 00:00", "2026-11-30 00:00").await;
    let a = wallet(pool, 1).await;
    progress(pool, &a, 1, 3, 0).await;
    let ran = Ran::default();
    let cancel = CancellationToken::new();
    let state = enabled(pool);
    let names = |done: Vec<(Kind, u64)>| done.into_iter().map(|(k, _)| k).collect::<Vec<_>>();

    assert_eq!(
        names(
            pass(&state, &ran, at("2026-11-19 08:00"), &cancel)
                .await
                .unwrap()
        ),
        vec![Kind::Usage24Hours]
    );
    let done = pass(&state, &ran, at("2026-11-19 09:05"), &cancel)
        .await
        .unwrap();
    assert_eq!(done, vec![(Kind::DoNotMissOut, 1), (Kind::Usage, 0)]);
    assert!(
        pass(&state, &ran, at("2026-11-19 23:00"), &cancel)
            .await
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        names(
            pass(&state, &ran, at("2026-11-20 00:30"), &cancel)
                .await
                .unwrap()
        ),
        vec![Kind::Usage24Hours]
    );

    let cancelled = CancellationToken::new();
    cancelled.cancel();
    let fresh = Ran::default();
    assert!(
        pass(&state, &fresh, at("2026-11-20 13:00"), &cancelled)
            .await
            .unwrap()
            .is_empty()
    );
    assert!(fresh.lock().is_empty());

    let off = state_off(pool);
    assert!(
        pass(&off, &fresh, at("2026-11-20 13:00"), &cancel)
            .await
            .unwrap()
            .is_empty()
    );
    assert!(fresh.lock().is_empty());
    db.drop().await;
}
fn state_off(pool: &sqlx::PgPool) -> SharedState {
    state(
        pool.clone(),
        json!({"flags":{"dapps-credits-server":false},"variants":{}}),
        json!({"flags":{},"variants":{}}),
    )
}

/// Five wallets due a do-not-miss-out reminder this week.
async fn untouched_wallets(pool: &sqlx::PgPool) {
    season(pool, 1, "2026-11-02 00:00", "2026-11-30 00:00").await;
    for n in 1..=5 {
        let a = wallet(pool, n).await;
        progress(pool, &a, 1, 3, 0).await;
    }
}

#[tokio::test]
async fn a_failing_kind_does_not_stop_the_others() {
    let Some(db) = OwnDatabase::create("credits_reminders_fail").await else {
        return;
    };
    let pool = &db.pool;
    untouched_wallets(pool).await;
    let a = addr(1);
    grant(pool, &a, 1, 3, "1", now() + TimeDelta::days(2), true).await;
    sqlx::query("CREATE FUNCTION pg_temp_fail() RETURNS trigger LANGUAGE plpgsql AS $$BEGIN IF NEW.type='credits_reminder_do_not_miss_out' THEN RAISE EXCEPTION 'injected'; END IF; RETURN NEW; END$$")
        .execute(pool).await.unwrap();
    sqlx::query("CREATE TRIGGER fail_reminder BEFORE INSERT ON social.notifications FOR EACH ROW EXECUTE FUNCTION pg_temp_fail()")
        .execute(pool).await.unwrap();
    let ran = Ran::default();
    let state = enabled(pool);
    let thursday = at("2026-11-19 09:05");
    assert!(
        pass(&state, &ran, thursday, &CancellationToken::new())
            .await
            .is_err()
    );
    assert_eq!(kinds(&sent(pool).await), vec![pair("usage", &a)]);
    assert_eq!(ran.lock().get(&Kind::DoNotMissOut), None);
    assert_eq!(ran.lock().get(&Kind::Usage), Some(&thursday.date()));
    sqlx::query("DROP TRIGGER fail_reminder ON social.notifications")
        .execute(pool)
        .await
        .unwrap();
    let retried = pass(&state, &ran, thursday, &CancellationToken::new())
        .await
        .unwrap();
    assert_eq!(retried, vec![(Kind::DoNotMissOut, 5)]);
    assert_eq!(sent(pool).await.len(), 6);
    assert!(
        pass(&state, &ran, thursday, &CancellationToken::new())
            .await
            .unwrap()
            .is_empty()
    );
    db.drop().await;
}

#[tokio::test]
async fn cancellation_stops_a_kind_between_pages() {
    let Some(db) = OwnDatabase::create("credits_reminders_cancel").await else {
        return;
    };
    let pool = &db.pool;
    untouched_wallets(pool).await;
    sqlx::query("CREATE FUNCTION pg_temp_slow() RETURNS trigger LANGUAGE plpgsql AS $$BEGIN PERFORM pg_sleep(0.3); RETURN NEW; END$$")
        .execute(pool).await.unwrap();
    sqlx::query("CREATE TRIGGER slow_reminder BEFORE INSERT ON social.notifications FOR EACH ROW EXECUTE FUNCTION pg_temp_slow()")
        .execute(pool).await.unwrap();
    let ran = Ran::default();
    let state = enabled(pool);
    let cancel = CancellationToken::new();
    let stop = cancel.clone();
    tokio::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_millis(150)).await;
        stop.cancel();
    });
    let done = pass(&state, &ran, at("2026-11-19 09:05"), &cancel)
        .await
        .unwrap();
    assert!(done.is_empty());
    assert!(ran.lock().is_empty());
    // At most the page in flight when cancelled (two rows under test).
    assert!(sent(pool).await.len() <= 2);
    db.drop().await;
}

/// Inside both usage windows of a batch expiring twelve hours after `now()`.
const IN_WINDOW: &str = "2026-11-19 00:05";

/// What an unqualified-key writer stored for the batch expiring after `now()`.
fn old_usage_metadata() -> Value {
    json!({"expirationDate":"November 19","balance":1})
}

async fn usage_seasons(pool: &sqlx::PgPool) {
    season(pool, 1, "2026-09-01 00:00", "2026-10-01 00:00").await;
    season(pool, 2, "2026-10-01 00:00", "2026-11-09 00:00").await;
    season(pool, 3, "2026-11-09 00:00", "2026-12-21 00:00").await;
}

async fn mixed_usage_grants(pool: &sqlx::PgPool, n: u8) -> String {
    let a = wallet(pool, n).await;
    progress(pool, &a, 3, 1, 0).await;
    for (season, amount) in [(1, "1000000000000000000"), (2, "2000000000000000000")] {
        progress(pool, &a, season, 1, 2).await;
        grant(
            pool,
            &a,
            season,
            1,
            amount,
            now() + TimeDelta::hours(12),
            true,
        )
        .await;
    }
    a
}

#[tokio::test]
async fn usage_combines_origin_seasons_and_serializes_selected_season_delivery() {
    let Some(db) = OwnDatabase::create("usage_combined").await else {
        return;
    };
    let pool = &db.pool;
    usage_seasons(pool).await;
    for n in 1..=5 {
        let address = mixed_usage_grants(pool, n).await;
        if n != 4 {
            let details = match n {
                1 => json!({"ignore_all_email":true}),
                2 => json!({"message_type":{"credits_reminder_usage":{"email":false}}}),
                3 => json!({"message_type":{"credits_reminder_usage_24_hours":{"email":false}}}),
                _ => json!({}),
            };
            sqlx::query("INSERT INTO social.subscriptions(address,email,details) VALUES($1,'recipient@example.invalid',$2)")
                .bind(address).bind(details).execute(pool).await.unwrap();
        }
    }
    let state = enabled(pool);
    let gate = gate(&state, &mut pool.acquire().await.unwrap())
        .await
        .unwrap();
    let cancel = CancellationToken::new();
    for kind in [Kind::Usage, Kind::Usage24Hours] {
        let (a, b) = tokio::join!(
            run(&state, &gate, kind, now(), &cancel),
            run(&state, &gate, kind, now(), &cancel),
        );
        assert_eq!(a.unwrap() + b.unwrap(), 5);
        assert_eq!(run(&state, &gate, kind, now(), &cancel).await.unwrap(), 0);
    }
    let rows: Vec<(String, String, Value, Uuid)> =
        sqlx::query_as("SELECT address,broadcast_key,metadata,id FROM social.notifications")
            .fetch_all(pool)
            .await
            .unwrap();
    assert_eq!(rows.len(), 10);
    for (address, key, metadata, id) in rows {
        let expires = now() + TimeDelta::hours(12);
        assert!(
            key == Kind::Usage.usage_key(&address, 3, expires)
                || key == Kind::Usage24Hours.usage_key(&address, 3, expires)
        );
        assert_eq!(id, Uuid::new_v5(&NOTIFICATION_NAMESPACE, key.as_bytes()));
        assert_eq!(metadata["balance"], json!(3));
        assert_eq!(metadata["expirationDate"], json!("November 19"));
    }
    let deliveries: i64 = sqlx::query_scalar("SELECT count(*) FROM social.notification_deliveries")
        .fetch_one(pool)
        .await
        .unwrap();
    assert_eq!(deliveries, 10);
    let deliveries: Vec<(String, String, String)> = sqlx::query_as(
        "SELECT n.address,n.type,d.state FROM social.notifications n JOIN social.notification_deliveries d ON d.notification_id=n.id",
    ).fetch_all(pool).await.unwrap();
    for (address, kind, state) in deliveries {
        let skipped = address == addr(1)
            || address == addr(4)
            || (address == addr(2) && kind == Kind::Usage.notification_type())
            || (address == addr(3) && kind == Kind::Usage24Hours.notification_type());
        assert_eq!(state, if skipped { "skipped" } else { "pending" });
    }
    db.drop().await;
}

#[tokio::test]
async fn usage_reconciles_old_keys_without_recreating_deliveries() {
    let Some(db) = OwnDatabase::create("usage_reconcile").await else {
        return;
    };
    let pool = &db.pool;
    usage_seasons(pool).await;
    let state = enabled(pool);
    let gate = gate(&state, &mut pool.acquire().await.unwrap())
        .await
        .unwrap();
    let cancel = CancellationToken::new();
    for (kind_index, kind) in [Kind::Usage, Kind::Usage24Hours].into_iter().enumerate() {
        for scenario in 1..=8 {
            let a = mixed_usage_grants(pool, kind_index as u8 * 10 + scenario).await;
            let old_key = kind.key(&a, 1, None);
            send(&state, kind, &a, &old_key, old_usage_metadata(), now())
                .await
                .unwrap();
            let old_id = Uuid::new_v5(&NOTIFICATION_NAMESPACE, old_key.as_bytes());
            let created = if scenario == 6 {
                at("2026-10-15 09:00")
            } else {
                at(IN_WINDOW)
            };
            sqlx::query(
                "UPDATE social.notifications SET created_at=$2,read=true,read_at=$2 WHERE id=$1",
            )
            .bind(old_id)
            .bind(created)
            .execute(pool)
            .await
            .unwrap();
            let delivery_state = match scenario {
                2 => "sent",
                3 => "failed",
                4 => "skipped",
                _ => "pending",
            };
            sqlx::query("UPDATE social.notification_deliveries SET state=$2,attempts=2,last_error='preserved' WHERE notification_id=$1")
                .bind(old_id).bind(delivery_state).execute(pool).await.unwrap();
            if scenario == 8 {
                // Terminal delivery retention must not make the notice new again.
                sqlx::query("DELETE FROM social.notification_deliveries WHERE notification_id=$1")
                    .bind(old_id)
                    .execute(pool)
                    .await
                    .unwrap();
            }
            let selected = kind.usage_key(&a, 3, now() + TimeDelta::hours(12));
            if scenario == 5 {
                send(&state, kind, &a, &selected, json!({"balance":2}), now())
                    .await
                    .unwrap();
            }
            if scenario == 7 {
                // This old key still counts even when its own grant no longer qualifies.
                sqlx::query(
                    "UPDATE credits.goal_grants SET expires_at=$2 WHERE address=$1 AND season_id=1",
                )
                .bind(&a)
                .bind(now())
                .execute(pool)
                .await
                .unwrap();
            }
        }
        let (first, second) = tokio::join!(
            run(&state, &gate, kind, now(), &cancel),
            run(&state, &gate, kind, now(), &cancel),
        );
        first.unwrap();
        second.unwrap();
        assert_eq!(run(&state, &gate, kind, now(), &cancel).await.unwrap(), 0);
        for scenario in 1..=8 {
            let a = addr(kind_index as u8 * 10 + scenario);
            let key = kind.usage_key(&a, 3, now() + TimeDelta::hours(12));
            let (id, metadata): (Uuid, Value) = sqlx::query_as("SELECT id,metadata FROM social.notifications WHERE address=$1 AND broadcast_key=$2")
                .bind(&a).bind(&key).fetch_one(pool).await.unwrap();
            let old_key = kind.key(&a, 1, None);
            let old_id = Uuid::new_v5(&NOTIFICATION_NAMESPACE, old_key.as_bytes());
            assert_eq!(
                metadata["balance"],
                json!(if scenario == 7 { 2 } else { 3 })
            );
            assert_eq!(
                id,
                if scenario == 5 || scenario == 6 {
                    Uuid::new_v5(&NOTIFICATION_NAMESPACE, key.as_bytes())
                } else {
                    old_id
                }
            );
            if scenario == 8 {
                let deliveries: i64 = sqlx::query_scalar(
                    "SELECT count(*) FROM social.notification_deliveries WHERE notification_id=$1",
                )
                .bind(old_id)
                .fetch_one(pool)
                .await
                .unwrap();
                assert_eq!(deliveries, 0);
                continue;
            }
            let (read, created, state, attempts, error): (bool, NaiveDateTime, String, i32, String) = sqlx::query_as(
                "SELECT n.read,n.created_at,d.state,d.attempts,d.last_error FROM social.notifications n JOIN social.notification_deliveries d ON d.notification_id=n.id WHERE n.id=$1",
            ).bind(old_id).fetch_one(pool).await.unwrap();
            assert!(read);
            assert_eq!(
                created,
                if scenario == 6 {
                    at("2026-10-15 09:00")
                } else {
                    at(IN_WINDOW)
                }
            );
            assert_eq!(
                state,
                match scenario {
                    2 => "sent",
                    3 => "failed",
                    4 => "skipped",
                    _ => "pending",
                }
            );
            assert_eq!(attempts, 2);
            assert_eq!(error, "preserved");
            let count: i64 = sqlx::query_scalar("SELECT count(*) FROM social.notification_deliveries d JOIN social.notifications n ON n.id=d.notification_id WHERE n.address=$1 AND n.type=$2")
                .bind(&a).bind(kind.notification_type()).fetch_one(pool).await.unwrap();
            assert_eq!(count, if scenario == 5 || scenario == 6 { 2 } else { 1 });
        }
    }
    db.drop().await;
}

#[tokio::test]
async fn usage_reconciliation_rolls_back_with_the_notification_write() {
    let Some(db) = OwnDatabase::create("usage_reconcile_rollback").await else {
        return;
    };
    let pool = &db.pool;
    usage_seasons(pool).await;
    let a = mixed_usage_grants(pool, 1).await;
    let state = enabled(pool);
    let gate = gate(&state, &mut pool.acquire().await.unwrap())
        .await
        .unwrap();
    let cancel = CancellationToken::new();
    for kind in [Kind::Usage, Kind::Usage24Hours] {
        let old_key = kind.key(&a, 1, None);
        send(&state, kind, &a, &old_key, old_usage_metadata(), now())
            .await
            .unwrap();
    }
    sqlx::query("UPDATE social.notifications SET created_at=$1")
        .bind(at(IN_WINDOW))
        .execute(pool)
        .await
        .unwrap();
    sqlx::query("CREATE FUNCTION fail_usage_refresh() RETURNS trigger LANGUAGE plpgsql AS $$BEGIN IF NEW.metadata->>'balance'='3' THEN RAISE EXCEPTION 'injected aggregate failure'; END IF; RETURN NEW; END$$")
        .execute(pool).await.unwrap();
    sqlx::query("CREATE TRIGGER fail_usage_refresh BEFORE INSERT OR UPDATE ON social.notifications FOR EACH ROW EXECUTE FUNCTION fail_usage_refresh()")
        .execute(pool).await.unwrap();
    for kind in [Kind::Usage, Kind::Usage24Hours] {
        assert!(run(&state, &gate, kind, now(), &cancel).await.is_err());
        let keys: Vec<String> = sqlx::query_scalar(
            "SELECT broadcast_key FROM social.notifications WHERE address=$1 AND type=$2",
        )
        .bind(&a)
        .bind(kind.notification_type())
        .fetch_all(pool)
        .await
        .unwrap();
        assert_eq!(keys, vec![kind.key(&a, 1, None)]);
    }
    sqlx::query("DROP TRIGGER fail_usage_refresh ON social.notifications")
        .execute(pool)
        .await
        .unwrap();
    for kind in [Kind::Usage, Kind::Usage24Hours] {
        assert_eq!(run(&state, &gate, kind, now(), &cancel).await.unwrap(), 0);
    }
    assert_eq!(sent(pool).await.len(), 2);
    db.drop().await;
}

/// A claimed grant whose creation and claim match the simulated timeline.
async fn claimed_grant_at(
    pool: &sqlx::PgPool,
    a: &str,
    season: i32,
    week: i32,
    wei: &str,
    claimed: NaiveDateTime,
    expires: NaiveDateTime,
) {
    grant(pool, a, season, week, wei, expires, true).await;
    sqlx::query("UPDATE credits.goal_grants SET created_at=$2,claimed_at=$2 WHERE salt=$1")
        .bind(grant_salt(a, season, week))
        .bind(claimed)
        .execute(pool)
        .await
        .unwrap();
}

/// Records the simulated run time as the creation time of new notices.
async fn created_during(pool: &sqlx::PgPool, a: &str, kind: Kind, at: NaiveDateTime) {
    sqlx::query("UPDATE social.notifications SET created_at=$3 WHERE address=$1 AND type=$2 AND created_at>$3")
        .bind(a)
        .bind(kind.notification_type())
        .bind(at)
        .execute(pool)
        .await
        .unwrap();
}

#[tokio::test]
async fn usage_delivers_distinct_expiry_batches_within_one_selected_season() {
    for (n, kind, first_run, second_run) in [
        (
            1,
            Kind::Usage,
            at("2026-11-30 09:00"),
            at("2026-12-24 09:00"),
        ),
        (
            2,
            Kind::Usage24Hours,
            at("2026-12-02 12:00"),
            at("2026-12-30 12:00"),
        ),
    ] {
        let Some(db) = OwnDatabase::create("usage_expiry_batches").await else {
            return;
        };
        let pool = &db.pool;
        // Consecutive seasons with three grace days; no later season before January.
        season(pool, 1, "2026-11-02 00:00", "2026-11-30 00:00").await;
        season(pool, 2, "2026-11-30 00:00", "2026-12-28 00:00").await;
        sqlx::query("UPDATE credits.seasons SET grace_days=3")
            .execute(pool)
            .await
            .unwrap();
        let state = enabled(pool);
        let gate = gate(&state, &mut pool.acquire().await.unwrap())
            .await
            .unwrap();
        let cancel = CancellationToken::new();
        let a = wallet(pool, n).await;
        sqlx::query("INSERT INTO social.subscriptions(address,email,details) VALUES($1,'recipient@example.invalid','{}')")
            .bind(&a)
            .execute(pool)
            .await
            .unwrap();
        progress(pool, &a, 1, 4, 2).await;
        progress(pool, &a, 2, 1, 2).await;
        let first_expiry = at("2026-12-03 00:00");
        let second_expiry = at("2026-12-31 00:00");
        claimed_grant_at(
            pool,
            &a,
            1,
            4,
            "1000000000000000000",
            at("2026-11-27 10:00"),
            first_expiry,
        )
        .await;
        assert_eq!(
            current_or_last(pool, first_run).await.unwrap().unwrap().id,
            2
        );
        assert_eq!(
            run(&state, &gate, kind, first_run, &cancel).await.unwrap(),
            1
        );
        created_during(pool, &a, kind, first_run).await;
        assert_eq!(
            run(&state, &gate, kind, first_run, &cancel).await.unwrap(),
            0
        );
        // The first notice was read and its email sent.
        sqlx::query("UPDATE social.notifications SET read=true,read_at=$2 WHERE address=$1")
            .bind(&a)
            .bind(first_run)
            .execute(pool)
            .await
            .unwrap();
        sqlx::query("UPDATE social.notification_deliveries d SET state='sent' FROM social.notifications n WHERE n.id=d.notification_id AND n.address=$1")
            .bind(&a)
            .execute(pool)
            .await
            .unwrap();

        claimed_grant_at(
            pool,
            &a,
            2,
            1,
            "2000000000000000000",
            at("2026-12-07 10:00"),
            second_expiry,
        )
        .await;
        assert_eq!(
            current_or_last(pool, second_run).await.unwrap().unwrap().id,
            2
        );
        assert_eq!(
            run(&state, &gate, kind, second_run, &cancel).await.unwrap(),
            1,
            "{kind:?}: a later expiry batch in the same season must be delivered"
        );
        created_during(pool, &a, kind, second_run).await;
        assert_eq!(
            run(&state, &gate, kind, second_run, &cancel).await.unwrap(),
            0
        );

        let rows: Vec<(bool, Value, String)> = sqlx::query_as(
            "SELECT n.read,n.metadata,d.state FROM social.notifications n JOIN social.notification_deliveries d ON d.notification_id=n.id WHERE n.address=$1 AND n.type=$2 ORDER BY n.created_at",
        )
        .bind(&a)
        .bind(kind.notification_type())
        .fetch_all(pool)
        .await
        .unwrap();
        assert_eq!(rows.len(), 2, "{kind:?}: one notice and delivery per batch");
        assert!(rows[0].0);
        assert_eq!(rows[0].1["expirationDate"], json!("December 3"));
        assert_eq!(rows[0].1["balance"], json!(1));
        assert_eq!(rows[0].2, "sent");
        assert!(!rows[1].0);
        assert_eq!(rows[1].1["expirationDate"], json!("December 31"));
        assert_eq!(rows[1].1["balance"], json!(2));
        assert_eq!(rows[1].2, "pending");
        db.drop().await;
    }
}

/// A notice an unqualified-key writer created for a batch, with its delivery.
async fn old_writer_notice(
    state: &SharedState,
    kind: Kind,
    a: &str,
    key: &str,
    expiration_date: &str,
    created: NaiveDateTime,
    delivery: &str,
) -> Uuid {
    let metadata = json!({"expirationDate":expiration_date,"balance":1});
    assert!(send(state, kind, a, key, metadata, created).await.unwrap());
    let id = Uuid::new_v5(&NOTIFICATION_NAMESPACE, key.as_bytes());
    let pool = &state.credits.pool;
    sqlx::query("UPDATE social.notifications SET created_at=$2,read=$3,read_at=CASE WHEN $3 THEN $2 END WHERE id=$1")
        .bind(id)
        .bind(created)
        .bind(delivery == "sent")
        .execute(pool)
        .await
        .unwrap();
    sqlx::query("UPDATE social.notification_deliveries SET state=$2 WHERE notification_id=$1")
        .bind(id)
        .bind(delivery)
        .execute(pool)
        .await
        .unwrap();
    id
}

/// (key, id, read, delivery state) of a wallet's notices of one kind.
async fn notices(
    pool: &sqlx::PgPool,
    a: &str,
    kind: Kind,
) -> Vec<(String, Uuid, bool, Option<String>)> {
    sqlx::query_as("SELECT n.broadcast_key,n.id,n.read,d.state FROM social.notifications n LEFT JOIN social.notification_deliveries d ON d.notification_id=n.id WHERE n.address=$1 AND n.type=$2 ORDER BY n.broadcast_key")
        .bind(a)
        .bind(kind.notification_type())
        .fetch_all(pool)
        .await
        .unwrap()
}

#[tokio::test]
async fn usage_upgrade_reuses_only_the_matching_batch_alias() {
    for (kind, in_first, in_second) in [
        (Kind::Usage, at("2026-11-30 09:00"), at("2026-12-24 09:00")),
        (
            Kind::Usage24Hours,
            at("2026-12-02 12:00"),
            at("2026-12-30 12:00"),
        ),
    ] {
        let Some(db) = OwnDatabase::create("usage_upgrade_aliases").await else {
            return;
        };
        let pool = &db.pool;
        season(pool, 1, "2026-11-02 00:00", "2026-11-30 00:00").await;
        season(pool, 2, "2026-11-30 00:00", "2026-12-28 00:00").await;
        sqlx::query("UPDATE credits.seasons SET grace_days=3")
            .execute(pool)
            .await
            .unwrap();
        let state = enabled(pool);
        let gate = gate(&state, &mut pool.acquire().await.unwrap())
            .await
            .unwrap();
        let cancel = CancellationToken::new();
        let first_expiry = at("2026-12-03 00:00");
        let second_expiry = at("2026-12-31 00:00");
        let setup = |n: u8| async move {
            let a = wallet(pool, n).await;
            sqlx::query("INSERT INTO social.subscriptions(address,email,details) VALUES($1,'recipient@example.invalid','{}')")
                .bind(&a)
                .execute(pool)
                .await
                .unwrap();
            progress(pool, &a, 1, 4, 2).await;
            progress(pool, &a, 2, 1, 2).await;
            claimed_grant_at(
                pool,
                &a,
                1,
                4,
                "1000000000000000000",
                at("2026-11-27 10:00"),
                first_expiry,
            )
            .await;
            a
        };

        // Upgraded while the first batch is still open: two old-writer
        // aliases announce it; the earliest carries the new key.
        let open = setup(1).await;
        let origin = kind.key(&open, 1, None);
        let unqualified = kind.key(&open, 2, None);
        let carried = old_writer_notice(
            &state,
            kind,
            &open,
            &origin,
            "December 3",
            in_first - TimeDelta::minutes(30),
            "sent",
        )
        .await;
        let leftover = old_writer_notice(
            &state,
            kind,
            &open,
            &unqualified,
            "December 3",
            in_first - TimeDelta::minutes(20),
            "pending",
        )
        .await;
        assert_eq!(
            run(&state, &gate, kind, in_first, &cancel).await.unwrap(),
            0
        );
        // Balance growth with the same expiry refreshes the same notice.
        progress(pool, &open, 1, 5, 2).await;
        claimed_grant_at(
            pool,
            &open,
            1,
            5,
            "1000000000000000000",
            at("2026-11-28 10:00"),
            first_expiry,
        )
        .await;
        assert_eq!(
            run(&state, &gate, kind, in_first, &cancel).await.unwrap(),
            0
        );
        let first_key = kind.usage_key(&open, 2, first_expiry);
        let balance: Value =
            sqlx::query_scalar("SELECT metadata->'balance' FROM social.notifications WHERE id=$1")
                .bind(carried)
                .fetch_one(pool)
                .await
                .unwrap();
        assert_eq!(balance, json!(2));

        // First corrected run after the old batch expired: its notice stays
        // historical under its unqualified key.
        let expired = setup(2).await;
        let historical = kind.key(&expired, 2, None);
        let historical_id = old_writer_notice(
            &state,
            kind,
            &expired,
            &historical,
            "December 3",
            in_first,
            "sent",
        )
        .await;

        for a in [&open, &expired] {
            claimed_grant_at(
                pool,
                a,
                2,
                1,
                "2000000000000000000",
                at("2026-12-07 10:00"),
                second_expiry,
            )
            .await;
        }
        let (x, y) = tokio::join!(
            run(&state, &gate, kind, in_second, &cancel),
            run(&state, &gate, kind, in_second, &cancel),
        );
        assert_eq!(x.unwrap() + y.unwrap(), 2, "{kind:?}");
        // A restarted job (fresh run memory) sends nothing again.
        let again = pass(&state, &Ran::default(), in_second, &cancel)
            .await
            .unwrap();
        assert!(again.contains(&(kind, 0)), "{kind:?}: {again:?}");

        let second_key = kind.usage_key(&open, 2, second_expiry);
        let second_id = Uuid::new_v5(&NOTIFICATION_NAMESPACE, second_key.as_bytes());
        assert_ne!(second_id, carried);
        let mut expected = vec![
            (first_key, carried, true, Some("sent".to_string())),
            (unqualified, leftover, false, Some("pending".to_string())),
            (second_key, second_id, false, Some("pending".to_string())),
        ];
        expected.sort();
        assert_eq!(notices(pool, &open, kind).await, expected, "{kind:?}");

        let new_key = kind.usage_key(&expired, 2, second_expiry);
        let new_id = Uuid::new_v5(&NOTIFICATION_NAMESPACE, new_key.as_bytes());
        let mut expected = vec![
            (historical, historical_id, true, Some("sent".to_string())),
            (new_key, new_id, false, Some("pending".to_string())),
        ];
        expected.sort();
        assert_eq!(notices(pool, &expired, kind).await, expected, "{kind:?}");
        db.drop().await;
    }
}

#[tokio::test]
async fn usage_alias_announcing_another_date_does_not_absorb_a_later_batch() {
    let Some(db) = OwnDatabase::create("usage_alias_other_date").await else {
        return;
    };
    let pool = &db.pool;
    season(pool, 1, "2026-11-02 00:00", "2026-11-30 00:00").await;
    season(pool, 2, "2026-11-30 00:00", "2026-12-28 00:00").await;
    let state = enabled(pool);
    let gate = gate(&state, &mut pool.acquire().await.unwrap())
        .await
        .unwrap();
    let a = wallet(pool, 1).await;
    progress(pool, &a, 2, 1, 2).await;
    progress(pool, &a, 2, 2, 2).await;
    claimed_grant_at(
        pool,
        &a,
        2,
        1,
        "1000000000000000000",
        at("2026-11-30 10:00"),
        at("2026-12-03 00:00"),
    )
    .await;
    // The old writer announced the earlier batch inside the later one's window.
    let old_key = Kind::Usage.key(&a, 2, None);
    let old_id = old_writer_notice(
        &state,
        Kind::Usage,
        &a,
        &old_key,
        "December 3",
        at("2026-12-01 09:00"),
        "sent",
    )
    .await;
    let later = at("2026-12-05 00:00");
    claimed_grant_at(
        pool,
        &a,
        2,
        2,
        "2000000000000000000",
        at("2026-12-02 10:00"),
        later,
    )
    .await;
    let cancel = CancellationToken::new();
    let run_at = at("2026-12-04 09:00");
    assert_eq!(
        run(&state, &gate, Kind::Usage, run_at, &cancel)
            .await
            .unwrap(),
        1
    );
    assert_eq!(
        run(&state, &gate, Kind::Usage, run_at, &cancel)
            .await
            .unwrap(),
        0
    );
    let key = Kind::Usage.usage_key(&a, 2, later);
    let id = Uuid::new_v5(&NOTIFICATION_NAMESPACE, key.as_bytes());
    // Without a subscription the new email is skipped; the old one stays sent.
    let mut expected = vec![
        (old_key, old_id, true, Some("sent".to_string())),
        (key, id, false, Some("skipped".to_string())),
    ];
    expected.sort();
    assert_eq!(notices(pool, &a, Kind::Usage).await, expected);
    db.drop().await;
}
