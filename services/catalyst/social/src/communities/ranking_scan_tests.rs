use super::*;
use chrono::Duration;
use serde_json::json;

async fn community(pool: &PgPool, now: NaiveDateTime) -> Uuid {
    let id = Uuid::new_v4();
    sqlx::query("INSERT INTO social.communities(id,name,owner_address,ranking_tracking_since) VALUES($1,'Ranking scanner',$2,$3)")
        .bind(id).bind(format!("0x{}00000000", Uuid::new_v4().simple()))
        .bind(now-Duration::days(30)).execute(pool).await.unwrap();
    id
}

async fn event(
    pool: &PgPool,
    community: Uuid,
    id: &str,
    starts: &[NaiveDateTime],
    duration_ms: i64,
) {
    let dates: Vec<_> = starts
        .iter()
        .map(|start| start.and_utc().to_rfc3339())
        .collect();
    let raw = json!({"start_at":dates[0],"recurrent_dates":dates});
    sqlx::query("INSERT INTO explore.event(id,name,start_at,finish_at,duration_ms,approved,community_id,raw,origin) VALUES($1,'Ranking scanner',$2,$3,$4,true,$5,$6,'local')")
        .bind(id).bind(starts[0]).bind(*starts.iter().max().unwrap()+Duration::milliseconds(duration_ms))
        .bind(duration_ms).bind(community.to_string()).bind(raw).execute(pool).await.unwrap();
}

async fn counts(pool: &PgPool, community: Uuid) -> (i32, i32, i64) {
    sqlx::query_as("SELECT ranking_events_count,ranking_events_total_attendees,(SELECT count(*) FROM social.community_ranking_events r WHERE r.community_id=c.id AND kind='event') FROM social.communities c WHERE id=$1")
        .bind(community).fetch_one(pool).await.unwrap()
}

async fn remove(pool: &PgPool, community: Uuid) {
    sqlx::query("DELETE FROM explore.event_attendance_local WHERE event_id IN (SELECT id FROM explore.event WHERE community_id=$1)")
        .bind(community.to_string()).execute(pool).await.unwrap();
    sqlx::query("DELETE FROM explore.event WHERE community_id=$1")
        .bind(community.to_string())
        .execute(pool)
        .await
        .unwrap();
    sqlx::query("DELETE FROM social.communities WHERE id=$1")
        .bind(community)
        .execute(pool)
        .await
        .unwrap();
}

#[tokio::test]
async fn completed_catalog_pages_rotate_before_long_recurrence_windows_and_restart_safely() {
    let _serial = ENDED_SCAN_TESTS.lock().await;
    let Some(pool) = crate::db::scratch::pool(4).await else {
        return;
    };
    let now = chrono::DateTime::from_timestamp_micros(Utc::now().timestamp_micros())
        .unwrap()
        .naive_utc();
    let community = community(&pool, now).await;
    let prefix = format!("r16-page-{}", Uuid::new_v4());
    let start = now - Duration::hours(2);
    sqlx::query("INSERT INTO explore.event(id,name,start_at,finish_at,duration_ms,approved,community_id,raw,origin) SELECT $1||'-'||lpad(n::text,3,'0'),'Completed page',$2,$2+interval '1 hour',3600000,true,$3,jsonb_build_object('start_at',$4::text),'local' FROM generate_series(0,204) n")
        .bind(&prefix).bind(start).bind(community.to_string()).bind(start.and_utc().to_rfc3339()).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO social.community_ranking_events(kind,event_key,community_id,occurred_at) SELECT 'event',id||':'||start_at::text,$2,finish_at FROM explore.event WHERE community_id=$1 ORDER BY id LIMIT 200")
        .bind(community.to_string()).bind(community).execute(&pool).await.unwrap();
    sqlx::query("UPDATE social.communities SET ranking_events_count=200 WHERE id=$1")
        .bind(community)
        .execute(&pool)
        .await
        .unwrap();
    let long = format!("{prefix}-long");
    let starts: Vec<_> = (0..250)
        .map(|n| now - Duration::hours(1) - Duration::minutes(n))
        .collect();
    event(&pool, community, &long, &starts, 1000).await;
    sqlx::query("INSERT INTO explore.event_attendance_local(event_id,signer,action) SELECT $1,'0x'||replace(gen_random_uuid()::text,'-','')||'00000000',action FROM unnest(ARRAY['going','going','cancel']) action")
        .bind(&long).execute(&pool).await.unwrap();
    let mut cursor = EndedScan {
        after_event: prefix.clone(),
        ..Default::default()
    };
    assert_eq!(
        scan_ended_page(&pool, now, &mut cursor, 100).await.unwrap(),
        0
    );
    assert_eq!(cursor.after_event, format!("{prefix}-099"));
    assert_eq!(
        scan_ended_page(&pool, now, &mut cursor, 100).await.unwrap(),
        0
    );
    assert_eq!(cursor.after_event, format!("{prefix}-199"));
    assert!(scan_ended_page(&pool, now, &mut cursor, 100).await.unwrap() <= 100);
    assert_eq!(counts(&pool, community).await.0, 300);
    // A lease/process restart loses only this scalar work position.
    cursor = EndedScan::default();
    let mut saw_later_window = false;
    for _ in 0..200 {
        assert!(scan_ended_page(&pool, now, &mut cursor, 100).await.unwrap() <= 100);
        saw_later_window |= cursor.occurrence_offset >= 200;
        if counts(&pool, community).await.0 == 455 {
            break;
        }
    }
    assert!(saw_later_window);
    assert_eq!(counts(&pool, community).await, (455, 500, 455));
    remove(&pool, community).await;
}

#[tokio::test]
async fn cyclic_scan_reconsiders_duration_edits_new_backdated_rows_and_recurrence_changes() {
    let _serial = ENDED_SCAN_TESTS.lock().await;
    let Some(pool) = crate::db::scratch::pool(4).await else {
        return;
    };
    let now = chrono::DateTime::from_timestamp_micros(Utc::now().timestamp_micros())
        .unwrap()
        .naive_utc();
    let community = community(&pool, now).await;
    sqlx::query("UPDATE social.communities SET ranking_tracking_since=$2 WHERE id=$1")
        .bind(community)
        .bind(now - Duration::hours(1))
        .execute(&pool)
        .await
        .unwrap();
    let prefix = format!("r16-edit-{}", Uuid::new_v4());
    let edited = format!("{prefix}-z");
    let start = now - Duration::minutes(30);
    event(&pool, community, &edited, &[start], 7_200_000).await;
    let mut cursor = EndedScan {
        after_event: prefix.clone(),
        ..Default::default()
    };
    scan_ended_page(&pool, now, &mut cursor, 100).await.unwrap();
    assert_eq!(counts(&pool, community).await, (0, 0, 0));
    sqlx::query("UPDATE explore.event SET duration_ms=300000,finish_at=start_at+interval '5 minutes' WHERE id=$1")
        .bind(&edited).execute(&pool).await.unwrap();
    event(
        &pool,
        community,
        &format!("{prefix}-a"),
        &[now - Duration::minutes(20)],
        1000,
    )
    .await;
    event(
        &pool,
        community,
        &format!("{prefix}-old"),
        &[now - Duration::hours(2)],
        1000,
    )
    .await;
    for _ in 0..200 {
        scan_ended_page(&pool, now, &mut cursor, 100).await.unwrap();
        if counts(&pool, community).await.0 == 2 {
            break;
        }
    }
    assert_eq!(counts(&pool, community).await, (2, 0, 2));
    sqlx::query("UPDATE explore.event SET raw=jsonb_set(raw,'{recurrent_dates}',$2),finish_at=$3 WHERE id=$1")
        .bind(&edited).bind(json!([start.and_utc().to_rfc3339(),(now-Duration::minutes(10)).and_utc().to_rfc3339()]))
        .bind(now-Duration::minutes(5)).execute(&pool).await.unwrap();
    for _ in 0..200 {
        scan_ended_page(&pool, now, &mut cursor, 100).await.unwrap();
        if counts(&pool, community).await.0 == 3 {
            break;
        }
    }
    assert_eq!(counts(&pool, community).await, (3, 0, 3));
    // An earlier source key inserted after the first visit was not skipped,
    // activation history was not imported, and the unchanged start counted once.
    remove(&pool, community).await;
}

#[tokio::test]
async fn failed_and_cancelled_pages_retain_the_cursor_and_roll_back_receipts_with_counters() {
    let _serial = ENDED_SCAN_TESTS.lock().await;
    let Some(pool) = crate::db::scratch::pool(4).await else {
        return;
    };
    let now = chrono::DateTime::from_timestamp_micros(Utc::now().timestamp_micros())
        .unwrap()
        .naive_utc();
    let community = community(&pool, now).await;
    let prefix = format!("r16-failure-{}", Uuid::new_v4());
    event(
        &pool,
        community,
        &format!("{prefix}-a"),
        &[now - Duration::hours(2)],
        1000,
    )
    .await;
    let function = format!("r16_failure_{}", Uuid::new_v4().simple());
    sqlx::raw_sql(sqlx::AssertSqlSafe(format!("CREATE FUNCTION social.{function}() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'ranking scan fault'; END $$; CREATE TRIGGER {function} BEFORE UPDATE ON social.communities FOR EACH ROW WHEN (NEW.id='{community}') EXECUTE FUNCTION social.{function}()")))
        .execute(&pool).await.unwrap();
    let mut cursor = EndedScan {
        after_event: prefix,
        ..Default::default()
    };
    let before = cursor.clone();
    assert!(scan_ended_page(&pool, now, &mut cursor, 100).await.is_err());
    assert_eq!(cursor, before);
    assert_eq!(counts(&pool, community).await, (0, 0, 0));
    sqlx::raw_sql(sqlx::AssertSqlSafe(format!(
        "DROP TRIGGER {function} ON social.communities; DROP FUNCTION social.{function}()"
    )))
    .execute(&pool)
    .await
    .unwrap();
    let mut barrier = pool.begin().await.unwrap();
    let pid: i32 = sqlx::query_scalar("SELECT pg_backend_pid()")
        .fetch_one(&mut *barrier)
        .await
        .unwrap();
    sqlx::query("SELECT id FROM social.communities WHERE id=$1 FOR UPDATE")
        .bind(community)
        .execute(&mut *barrier)
        .await
        .unwrap();
    let mut pending = Box::pin(scan_ended_page(&pool, now, &mut cursor, 100));
    let blocked = async {
        loop {
            let waiting: bool = sqlx::query_scalar(
                "SELECT EXISTS(SELECT 1 FROM pg_stat_activity WHERE $1=ANY(pg_blocking_pids(pid)))",
            )
            .bind(pid)
            .fetch_one(&pool)
            .await
            .unwrap();
            if waiting {
                break;
            }
            tokio::task::yield_now().await;
        }
    };
    tokio::select! {
        result=&mut pending=>panic!("scan must reach its row barrier: {result:?}"),
        result=tokio::time::timeout(std::time::Duration::from_secs(5),blocked)=>result.unwrap(),
    }
    drop(pending);
    assert_eq!(cursor, before);
    barrier.commit().await.unwrap();
    scan_ended_page(&pool, now, &mut cursor, 100).await.unwrap();
    assert_ne!(cursor, before);
    assert_eq!(counts(&pool, community).await, (1, 0, 1));
    cursor = before;
    scan_ended_page(&pool, now, &mut cursor, 100).await.unwrap();
    assert_eq!(counts(&pool, community).await, (1, 0, 1));
    remove(&pool, community).await;
}

#[tokio::test]
async fn pass_shares_one_deadline_and_receipt_budget_across_pages_without_losing_progress() {
    let _serial = ENDED_SCAN_TESTS.lock().await;
    let Some(pool) = crate::db::scratch::pool(4).await else {
        return;
    };
    let now = chrono::DateTime::from_timestamp_micros(Utc::now().timestamp_micros())
        .unwrap()
        .naive_utc();
    let community = community(&pool, now).await;
    let prefix = format!("r16-pass-{}", Uuid::new_v4());
    let start = now - Duration::hours(2);
    sqlx::query("INSERT INTO explore.event(id,name,start_at,finish_at,duration_ms,approved,community_id,raw,origin) SELECT $1||'-'||lpad(n::text,3,'0'),'Due pages',$2,$2+interval '1 second',1000,true,$3,jsonb_build_object('start_at',$4::text),'local' FROM generate_series(0,204) n")
        .bind(&prefix).bind(start).bind(community.to_string()).bind(start.and_utc().to_rfc3339()).execute(&pool).await.unwrap();
    let mut cursor = EndedScan {
        after_event: prefix.clone(),
        ..Default::default()
    };
    let before = cursor.clone();
    let mut connections = Vec::new();
    for _ in 0..4 {
        connections.push(pool.acquire().await.unwrap());
    }
    // An occupied real pool leaves the pass waiting at its first acquire.
    // Advance only its clock, rather than sleeping through the three seconds.
    tokio::time::pause();
    let mut pending = Box::pin(scan_ended(&pool, now, &mut cursor));
    assert!(futures::poll!(pending.as_mut()).is_pending());
    tokio::time::advance(std::time::Duration::from_secs(3)).await;
    assert_eq!(pending.await.unwrap(), 0);
    tokio::time::resume();
    drop(connections);
    assert_eq!(cursor, before);
    assert_eq!(scan_ended(&pool, now, &mut cursor).await.unwrap(), 100);
    assert_eq!(cursor.after_event, format!("{prefix}-099"));
    assert_eq!(counts(&pool, community).await, (100, 0, 100));
    assert_eq!(scan_ended(&pool, now, &mut cursor).await.unwrap(), 100);
    assert_eq!(cursor.after_event, format!("{prefix}-199"));
    assert_eq!(scan_ended(&pool, now, &mut cursor).await.unwrap(), 5);
    assert_eq!(counts(&pool, community).await, (205, 0, 205));
    assert!(cursor.after_event.is_empty());
    assert_eq!(scan_ended(&pool, now, &mut cursor).await.unwrap(), 0);
    assert_eq!(counts(&pool, community).await, (205, 0, 205));
    remove(&pool, community).await;
}

#[tokio::test]
async fn hundred_thousand_completed_events_use_bounded_pages_and_finish_a_cycle_without_writes() {
    let Some(admin) = crate::db::scratch::pool(2).await else {
        return;
    };
    // An owned database keeps this history workload and its cursor independent
    // of the other scanner fixtures and their deliberate failure barriers.
    let database = format!(
        "catalyst_test_lib_{}_ranking_{}",
        std::process::id(),
        Uuid::new_v4().simple()
    );
    sqlx::query(sqlx::AssertSqlSafe(format!("CREATE DATABASE {database}")))
        .execute(&admin)
        .await
        .unwrap();
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(2)
        .connect_with((*admin.connect_options()).clone().database(&database))
        .await
        .unwrap();
    crate::db::apply_schema(&pool).await.unwrap();
    let now = chrono::DateTime::from_timestamp_micros(Utc::now().timestamp_micros())
        .unwrap()
        .naive_utc();
    let community = community(&pool, now).await;
    let start = now - Duration::hours(2);
    sqlx::query("INSERT INTO explore.event(id,name,start_at,finish_at,duration_ms,approved,community_id,raw,origin) SELECT 'history-'||lpad(n::text,6,'0'),'Completed history',$1,$1+interval '1 second',1000,true,$2,jsonb_build_object('start_at',$3::text,'description',repeat('x',128)),'local' FROM generate_series(1,100000) n")
        .bind(start).bind(community.to_string()).bind(start.and_utc().to_rfc3339()).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO social.community_ranking_events(kind,event_key,community_id,occurred_at) SELECT 'event',id||':'||start_at::text,$1,finish_at FROM explore.event")
        .bind(community).execute(&pool).await.unwrap();
    sqlx::query("UPDATE social.communities SET ranking_events_count=100000 WHERE id=$1")
        .bind(community)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::raw_sql("ANALYZE explore.event; ANALYZE social.communities; ANALYZE social.community_ranking_events; ANALYZE explore.event_attendance_local")
        .execute(&pool).await.unwrap();
    let mut cursor = EndedScan::default();
    let started = std::time::Instant::now();
    let mut passes = 0;
    loop {
        passes += 1;
        assert_eq!(scan_ended(&pool, now, &mut cursor).await.unwrap(), 0);
        if cursor.after_event.is_empty() {
            break;
        }
        assert!(
            passes < 10,
            "each budgeted pass must retain useful progress"
        );
    }
    let elapsed = started.elapsed();
    assert_eq!(counts(&pool, community).await, (100000, 0, 100000));
    eprintln!(
        "ranking 100k completed history: {passes} passes, {:.3}ms active scan, zero receipt/counter writes",
        elapsed.as_secs_f64() * 1000.0
    );
    pool.close().await;
    sqlx::query(sqlx::AssertSqlSafe(format!("DROP DATABASE {database}")))
        .execute(&admin)
        .await
        .unwrap();
}
