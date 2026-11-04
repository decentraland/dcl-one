//! Real row/advisory barriers hold requests at their mutation boundary.
use super::super::write_support::{acquire_membership, membership_lock};
use super::*;

// Both barrier fixtures create/drop triggers on community_bans. PostgreSQL's
// table-level DDL locks would otherwise mix their unrelated blocking chains.
static BAN_TRIGGERS: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

async fn blocked_by(pool: &PgPool, blocker: i32) -> i32 {
    tokio::time::timeout(std::time::Duration::from_secs(10), async {
        loop {
            let waiting: Option<i32> = sqlx::query_scalar(
                "SELECT pid FROM pg_stat_activity WHERE $1=ANY(pg_blocking_pids(pid)) LIMIT 1",
            )
            .bind(blocker)
            .fetch_optional(pool)
            .await
            .unwrap();
            if let Some(pid) = waiting {
                return pid;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("the real request must reach the PostgreSQL barrier")
}

#[tokio::test]
async fn role_and_kick_writes_recheck_a_concurrent_owner_promotion() {
    let Some(rig) = Rig::new(Settings::for_tests()).await else {
        return;
    };
    let (owner, target) = (wallet(), wallet());
    for role_update in [false, true] {
        let id = rig.community(&owner, false).await;
        rig.member(id, &target, "member").await;
        let mut promotion = rig.pool.begin().await.unwrap();
        let pid: i32 = sqlx::query_scalar("SELECT pg_backend_pid()")
            .fetch_one(&mut *promotion)
            .await
            .unwrap();
        // The actual ownership transaction locks the community then updates
        // both membership rows. Authorization still sees the committed owner.
        sqlx::query("UPDATE social.communities SET owner_address=$2 WHERE id=$1")
            .bind(id)
            .bind(&target.address)
            .execute(&mut *promotion)
            .await
            .unwrap();
        sqlx::query("UPDATE social.community_members SET role='moderator' WHERE community_id=$1 AND member_address=$2")
            .bind(id).bind(&owner.address).execute(&mut *promotion).await.unwrap();
        sqlx::query("UPDATE social.community_members SET role='owner' WHERE community_id=$1 AND member_address=$2")
            .bind(id).bind(&target.address).execute(&mut *promotion).await.unwrap();
        let path = format!("/v1/communities/{id}/members/{}", target.address);
        let case = if role_update {
            Case::new("patch", path).json(&json!({"role":"moderator"}))
        } else {
            Case::new("delete", path)
        }
        .signed(&owner)
        .await;
        let app = rig.app.clone();
        let write = tokio::spawn(async move { hit(&app, case).await });
        blocked_by(&rig.pool, pid).await;
        promotion.commit().await.unwrap();
        let (status, body) = tokio::time::timeout(std::time::Duration::from_secs(10), write)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            status,
            if role_update {
                StatusCode::NOT_FOUND
            } else {
                StatusCode::NO_CONTENT
            },
            "{body}"
        );
        assert_eq!(rig.role_of(id, &target).await.as_deref(), Some("owner"));
        let stored_owner: Address =
            sqlx::query_scalar("SELECT owner_address FROM social.communities WHERE id=$1")
                .bind(id)
                .fetch_one(&rig.pool)
                .await
                .unwrap();
        assert_eq!(stored_owner, target.address);
        assert_eq!(rig.role_of(id, &owner).await.as_deref(), Some("moderator"));
    }
    rig.finish();
}

// The trigger pauses only this community's INSERT, after the independent kick
// and unlike statements have already committed. It does not alter application SQL.
async fn ban_barrier(pool: &PgPool, id: CommunityId, key: i64, fail: bool) -> (String, String) {
    let suffix = Uuid::new_v4().simple().to_string();
    let function = format!("social.ban_barrier_{suffix}");
    let trigger = format!("ban_barrier_{suffix}");
    let tail = if fail {
        "RAISE EXCEPTION 'ban barrier fault';"
    } else {
        "RETURN NEW;"
    };
    sqlx::raw_sql(sqlx::AssertSqlSafe(format!(
        "CREATE FUNCTION {function}() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN PERFORM pg_advisory_xact_lock({key}); {tail} END $$; CREATE TRIGGER {trigger} BEFORE INSERT ON social.community_bans FOR EACH ROW WHEN (NEW.community_id='{id}'::uuid) EXECUTE FUNCTION {function}()"
    ))).execute(pool).await.unwrap();
    (function, trigger)
}
async fn remove_barrier(pool: &PgPool, function: &str, trigger: &str) {
    sqlx::raw_sql(sqlx::AssertSqlSafe(format!(
        "DROP TRIGGER {trigger} ON social.community_bans; DROP FUNCTION {function}()"
    )))
    .execute(pool)
    .await
    .unwrap();
}

#[tokio::test]
async fn ban_serializes_join_and_both_request_acceptance_paths_across_its_commits() {
    let _triggers = BAN_TRIGGERS.lock().await;
    let Some(rig) = Rig::new(Settings::for_tests()).await else {
        return;
    };
    let (owner, member) = (wallet(), wallet());
    for mode in ["join", "accept", "opposite", "bulk"] {
        let id = rig.community(&owner, mode != "join").await;
        rig.member(id, &member, "member").await;
        let request = rig
            .request(
                id,
                &member,
                if mode == "bulk" {
                    "request_to_join"
                } else {
                    "invite"
                },
            )
            .await;
        let innocent = wallet();
        if mode == "bulk" {
            rig.request(id, &innocent, "request_to_join").await;
        }
        let key = (Uuid::new_v4().as_u128() as i64) & i64::MAX;
        let (function, trigger) = ban_barrier(&rig.pool, id, key, false).await;
        let mut barrier = rig.pool.begin().await.unwrap();
        let pid: i32 = sqlx::query_scalar("SELECT pg_backend_pid()")
            .fetch_one(&mut *barrier)
            .await
            .unwrap();
        sqlx::query("SELECT pg_advisory_xact_lock($1)")
            .bind(key)
            .execute(&mut *barrier)
            .await
            .unwrap();
        let case = Case::new(
            "post",
            format!("/v1/communities/{id}/members/{}/bans", member.address),
        )
        .signed(&owner)
        .await;
        let app = rig.app.clone();
        let ban = tokio::spawn(async move { hit(&app, case).await });
        let ban_pid = blocked_by(&rig.pool, pid).await;
        assert_eq!(
            rig.role_of(id, &member).await,
            None,
            "kick commits before the ban insertion"
        );
        let banned: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM social.community_bans WHERE community_id=$1 AND active)",
        )
        .bind(id)
        .fetch_one(&rig.pool)
        .await
        .unwrap();
        assert!(!banned, "ban INSERT has not committed at the barrier");
        let case = match mode {
            "join" => Case::new("post", format!("/v1/communities/{id}/members")),
            "accept" => Case::new("patch", format!("/v1/communities/{id}/requests/{request}"))
                .json(&json!({"intention":"accepted"})),
            "bulk" => Case::new("put", format!("/v1/communities/{id}")).body(
                b"privacy=public".to_vec(),
                "application/x-www-form-urlencoded",
            ),
            _ => Case::new("post", format!("/v1/communities/{id}/requests"))
                .json(&json!({"type":"request_to_join","targetedAddress":member.address.as_str()})),
        }
        .signed(if mode == "bulk" { &owner } else { &member })
        .await;
        let app = rig.app.clone();
        let join = tokio::spawn(async move { hit(&app, case).await });
        blocked_by(&rig.pool, ban_pid).await;
        barrier.commit().await.unwrap();
        let (status, body) = tokio::time::timeout(std::time::Duration::from_secs(10), ban)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(status, StatusCode::NO_CONTENT, "{body}");
        let (status, body) = tokio::time::timeout(std::time::Duration::from_secs(10), join)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            status,
            if mode == "bulk" {
                StatusCode::OK
            } else {
                StatusCode::UNAUTHORIZED
            },
            "{mode}: {body}"
        );
        if mode == "bulk" {
            assert_eq!(
                rig.role_of(id, &innocent).await.as_deref(),
                Some("member"),
                "bulk acceptance still admits eligible other wallets"
            );
            assert_eq!(body["data"]["privacy"], "public");
        }
        assert_eq!(rig.role_of(id, &member).await, None);
        let requests:i64=sqlx::query_scalar("SELECT count(*) FROM social.community_requests WHERE community_id=$1 AND member_address=$2").bind(id).bind(&member.address).fetch_one(&rig.pool).await.unwrap();
        assert_eq!(requests, 0);
        remove_barrier(&rig.pool, &function, &trigger).await;
    }
    rig.finish();
}

#[tokio::test]
async fn interrupted_and_failed_bans_release_the_native_session_lock_and_keep_committed_kicks() {
    let _triggers = BAN_TRIGGERS.lock().await;
    let Some(rig) = Rig::new(Settings::for_tests()).await else {
        return;
    };
    let (owner, member) = (wallet(), wallet());
    for cancel in [false, true] {
        let id = rig.community(&owner, false).await;
        rig.member(id, &member, "member").await;
        let key = (Uuid::new_v4().as_u128() as i64) & i64::MAX;
        let (function, trigger) = ban_barrier(&rig.pool, id, key, !cancel).await;
        let mut barrier = rig.pool.begin().await.unwrap();
        let pid: i32 = sqlx::query_scalar("SELECT pg_backend_pid()")
            .fetch_one(&mut *barrier)
            .await
            .unwrap();
        sqlx::query("SELECT pg_advisory_xact_lock($1)")
            .bind(key)
            .execute(&mut *barrier)
            .await
            .unwrap();
        let case = Case::new(
            "post",
            format!("/v1/communities/{id}/members/{}/bans", member.address),
        )
        .signed(&owner)
        .await;
        let app = rig.app.clone();
        let ban = tokio::spawn(async move { hit(&app, case).await });
        blocked_by(&rig.pool, pid).await;
        assert_eq!(rig.role_of(id, &member).await, None);
        if cancel {
            ban.abort();
        }
        barrier.commit().await.unwrap();
        if cancel {
            assert!(ban.await.unwrap_err().is_cancelled());
        } else {
            assert_eq!(ban.await.unwrap().0, StatusCode::INTERNAL_SERVER_ERROR);
        }
        let connection = rig.pool.acquire().await.unwrap();
        let guard = tokio::time::timeout(
            std::time::Duration::from_secs(10),
            membership_lock(id).acquire(connection),
        )
        .await
        .expect("request cancellation and SQL errors must release the session lock")
        .unwrap();
        drop(guard);
        assert_eq!(
            rig.role_of(id, &member).await,
            None,
            "the already committed kick is never rolled back"
        );
        remove_barrier(&rig.pool, &function, &trigger).await;
    }
    rig.finish();
}

#[tokio::test]
async fn ownership_transfer_and_ban_share_the_guard_before_owner_authorization() {
    let Some(rig) = Rig::new(Settings::for_tests()).await else {
        return;
    };
    let (owner, target) = (wallet(), wallet());
    let id = rig.community(&owner, false).await;
    rig.member(id, &target, "member").await;
    rig.profile(&target, "Named target", true).await;
    rig.name_owner(&target, "Named target");
    let connection = rig.pool.acquire().await.unwrap();
    let mut barrier = membership_lock(id).acquire(connection).await.unwrap();
    let pid: i32 = sqlx::query_scalar("SELECT pg_backend_pid()")
        .fetch_one(&mut *barrier)
        .await
        .unwrap();
    let case = Case::new(
        "patch",
        format!("/v1/communities/{id}/members/{}", target.address),
    )
    .json(&json!({"role":"owner"}))
    .signed(&owner)
    .await;
    let app = rig.app.clone();
    let transfer = tokio::spawn(async move { hit(&app, case).await });
    let transfer_pid = blocked_by(&rig.pool, pid).await;
    let case = Case::new(
        "post",
        format!("/v1/communities/{id}/members/{}/bans", target.address),
    )
    .signed(&owner)
    .await;
    let app = rig.app.clone();
    let ban = tokio::spawn(async move { hit(&app, case).await });
    blocked_by(&rig.pool, transfer_pid).await;
    drop(barrier);
    let (status, body) = tokio::time::timeout(std::time::Duration::from_secs(10), transfer)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(status, StatusCode::NO_CONTENT, "{body}");
    let (status, body) = tokio::time::timeout(std::time::Duration::from_secs(10), ban)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        status,
        StatusCode::UNAUTHORIZED,
        "BAN rechecks the promoted owner under its guard: {body}"
    );
    assert_eq!(rig.role_of(id, &target).await.as_deref(), Some("owner"));
    assert_eq!(rig.role_of(id, &owner).await.as_deref(), Some("moderator"));
    let bans: i64 =
        sqlx::query_scalar("SELECT count(*) FROM social.community_bans WHERE community_id=$1")
            .bind(id)
            .fetch_one(&rig.pool)
            .await
            .unwrap();
    assert_eq!(bans, 0);
    rig.finish();
}

#[tokio::test]
async fn cancelling_successful_join_at_unlock_keeps_the_reused_backend_unlocked() {
    use std::sync::atomic::{AtomicU8, Ordering};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    let Some(rig) = Rig::new(Settings::for_tests()).await else {
        return;
    };
    let (owner, member) = (wallet(), wallet());
    let id = rig.community(&owner, false).await;
    let options = (*rig.pool.connect_options()).clone();
    let endpoint = (options.get_host().to_owned(), options.get_port());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let unlock = Arc::new(AtomicU8::new(0));
    let resume = Arc::new(tokio::sync::Notify::new());
    let (seen_tx, seen_rx) = tokio::sync::oneshot::channel();
    let gate = unlock.clone();
    let release = resume.clone();
    // Relay actual PostgreSQL traffic unchanged, withholding only the first
    // unlock response. A prepared unlock has not executed at this boundary;
    // the native guard's queued simple query has already executed.
    let proxy = tokio::spawn(async move {
        let (client, _) = listener.accept().await?;
        let server = tokio::net::TcpStream::connect(endpoint).await?;
        let (mut client_read, mut client_write) = client.into_split();
        let (mut server_read, mut server_write) = server.into_split();
        let frontend = async move {
            let length = client_read.read_u32().await?;
            let mut startup = vec![0; length as usize - 4];
            client_read.read_exact(&mut startup).await?;
            server_write.write_u32(length).await?;
            server_write.write_all(&startup).await?;
            while let Ok(kind) = client_read.read_u8().await {
                let length = client_read.read_u32().await?;
                let mut body = vec![0; length as usize - 4];
                client_read.read_exact(&mut body).await?;
                if body
                    .windows(b"pg_advisory_unlock".len())
                    .any(|part| part == b"pg_advisory_unlock")
                {
                    gate.compare_exchange(0, kind, Ordering::SeqCst, Ordering::SeqCst)
                        .ok();
                }
                server_write.write_u8(kind).await?;
                server_write.write_u32(length).await?;
                server_write.write_all(&body).await?;
            }
            Ok::<(), std::io::Error>(())
        };
        let backend = async move {
            let mut buffer = vec![0; 8192];
            let mut seen = Some(seen_tx);
            loop {
                let length = server_read.read(&mut buffer).await?;
                if length == 0 {
                    return Ok::<(), std::io::Error>(());
                }
                let kind = unlock.load(Ordering::SeqCst);
                if kind != 0
                    && let Some(seen) = seen.take()
                {
                    seen.send(kind).ok();
                    release.notified().await;
                }
                client_write.write_all(&buffer[..length]).await?;
            }
        };
        tokio::try_join!(frontend, backend)
    });
    let writers = sqlx::postgres::PgPoolOptions::new()
        .max_connections(1)
        .connect_with(
            options
                .host("127.0.0.1")
                .port(address.port())
                .ssl_mode(sqlx::postgres::PgSslMode::Disable),
        )
        .await
        .unwrap();
    let before: i32 = sqlx::query_scalar("SELECT pg_backend_pid()")
        .fetch_one(&writers)
        .await
        .unwrap();
    let state = Arc::new(State::new(
        writers.clone(),
        rig.state.storage.clone(),
        Settings::for_tests(),
        None,
        &crate::http::Outbound::for_tests(),
        State::explore_for_tests(&rig.pool),
    ));
    let updates = crate::rpc::pubsub::PubSub::new();
    let _owner_stream = updates.subscribe(owner.address.as_str());
    state.set_updates(updates);
    let app = router(state);
    let case = Case::new("post", format!("/v1/communities/{id}/members"))
        .signed(&member)
        .await;
    let request = tokio::spawn(async move { hit(&app, case).await });
    let kind = tokio::time::timeout(std::time::Duration::from_secs(10), seen_rx)
        .await
        .expect("successful join must reach its native unlock")
        .unwrap();
    request.abort();
    assert!(request.await.unwrap_err().is_cancelled());
    resume.notify_one();
    let after: i32 = tokio::time::timeout(
        std::time::Duration::from_secs(10),
        sqlx::query_scalar("SELECT pg_backend_pid()").fetch_one(&writers),
    )
    .await
    .expect("the same native pooled connection must remain reusable")
    .unwrap();
    let available = match membership_lock(id)
        .try_acquire(rig.pool.acquire().await.unwrap())
        .await
        .unwrap()
    {
        sqlx::Either::Left(guard) => {
            drop(guard);
            true
        }
        sqlx::Either::Right(_) => false,
    };
    assert_eq!(rig.role_of(id, &member).await.as_deref(), Some("member"));
    writers.close().await;
    proxy.abort();
    rig.finish();
    assert_eq!(before, after, "reconnection must not hide a leaked lock");
    assert!(available, "cancellation must not leak a live session lock");
    assert_eq!(kind, b'Q', "native Drop queues a simple unlock query");
}

#[tokio::test]
async fn cancelling_blocked_native_acquisition_reuses_the_same_backend_without_a_lock() {
    let Some(rig) = Rig::new(Settings::for_tests()).await else {
        return;
    };
    let id = CommunityId::new();
    let mut holder = membership_lock(id)
        .acquire(rig.pool.acquire().await.unwrap())
        .await
        .unwrap();
    let holder_pid: i32 = sqlx::query_scalar("SELECT pg_backend_pid()")
        .fetch_one(&mut *holder)
        .await
        .unwrap();
    let writers = sqlx::postgres::PgPoolOptions::new()
        .max_connections(1)
        .connect_with((*rig.pool.connect_options()).clone())
        .await
        .unwrap();
    let before: i32 = sqlx::query_scalar("SELECT pg_backend_pid()")
        .fetch_one(&writers)
        .await
        .unwrap();
    let waiting_pool = writers.clone();
    let waiter = tokio::spawn(async move {
        let guard = membership_lock(id)
            .acquire(waiting_pool.acquire().await.unwrap())
            .await
            .unwrap();
        drop(guard);
    });
    assert_eq!(blocked_by(&rig.pool, holder_pid).await, before);
    waiter.abort();
    assert!(waiter.await.unwrap_err().is_cancelled());
    drop(holder);
    let after: i32 = tokio::time::timeout(
        std::time::Duration::from_secs(10),
        sqlx::query_scalar("SELECT pg_backend_pid()").fetch_one(&writers),
    )
    .await
    .expect("cancelled acquisition must return its native connection")
    .unwrap();
    assert_eq!(before, after, "reconnection must not hide a leaked lock");
    let probe = membership_lock(id)
        .try_acquire(rig.pool.acquire().await.unwrap())
        .await
        .unwrap();
    assert!(
        matches!(probe, sqlx::Either::Left(_)),
        "an independent backend must acquire the cancelled waiter's lock"
    );
    drop(probe);
    writers.close().await;
    rig.finish();
}

#[tokio::test]
async fn blocked_membership_requests_release_the_shared_pool_with_one_or_two_connections() {
    let Some(rig) = Rig::new(Settings::for_tests()).await else {
        return;
    };
    let owner = wallet();
    for capacity in [1, 2] {
        let id = rig.community(&owner, false).await;
        let mut holder = membership_lock(id)
            .acquire(rig.pool.acquire().await.unwrap())
            .await
            .unwrap();
        let holder_pid: i32 = sqlx::query_scalar("SELECT pg_backend_pid()")
            .fetch_one(&mut *holder)
            .await
            .unwrap();
        let application = format!("membership-waiters-{}", Uuid::new_v4());
        let writers = sqlx::postgres::PgPoolOptions::new()
            .max_connections(capacity)
            .connect_with(
                (*rig.pool.connect_options())
                    .clone()
                    .application_name(&application),
            )
            .await
            .unwrap();
        let state = Arc::new(State::new(
            writers.clone(),
            rig.state.storage.clone(),
            Settings::for_tests(),
            None,
            &crate::http::Outbound::for_tests(),
            State::explore_for_tests(&rig.pool),
        ));
        let app = router(state);
        let mut requests = Vec::new();
        for _ in 0..capacity {
            let member = wallet();
            let case = Case::new("post", format!("/v1/communities/{id}/members"))
                .signed(&member)
                .await;
            let app = app.clone();
            requests.push(tokio::spawn(async move { hit(&app, case).await }));
        }
        tokio::time::timeout(std::time::Duration::from_secs(10), async {
            loop {
                let waiting: i64 = sqlx::query_scalar(
                    "SELECT count(*) FROM pg_stat_activity WHERE application_name=$1 AND wait_event_type='Lock'",
                )
                .bind(&application)
                .fetch_one(&rig.pool)
                .await
                .unwrap();
                if waiting == i64::from(capacity) {
                    return;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("every real request must occupy a pool slot waiting for membership");
        // Keep the holder locked throughout: a cancelled/timed-out acquire must
        // not leave SQLx's pool-return ping waiting behind that lock forever.
        let reusable = tokio::time::timeout(
            std::time::Duration::from_secs(3),
            sqlx::query_scalar::<_, i32>("SELECT 1").fetch_one(&writers),
        )
        .await;
        tokio::time::timeout(std::time::Duration::from_secs(2), async {
            loop {
                let waiting: i64 = sqlx::query_scalar(
                    "SELECT count(*) FROM pg_stat_activity WHERE application_name=$1 AND wait_event_type='Lock'",
                )
                .bind(&application)
                .fetch_one(&rig.pool)
                .await
                .unwrap();
                if waiting == 0 {
                    return;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("server waiters must also end while the holder remains locked");
        drop(holder);
        assert_eq!(
            reusable
                .expect("membership waiters must bound their use of the shared pool")
                .unwrap(),
            1,
            "capacity {capacity}, holder backend {holder_pid}"
        );
        for request in requests {
            let (status, body) = request.await.unwrap();
            assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR, "{body}");
            assert!(body["message"].is_string(), "{body}");
            assert_eq!(body.as_object().unwrap().len(), 1, "{body}");
        }
        writers.close().await;
    }
    rig.finish();
}

#[tokio::test]
async fn cancelled_membership_acquisition_frees_the_pool_and_ends_the_server_wait() {
    let Some(rig) = Rig::new(Settings::for_tests()).await else {
        return;
    };
    let id = CommunityId::new();
    let mut holder = membership_lock(id)
        .acquire(rig.pool.acquire().await.unwrap())
        .await
        .unwrap();
    let holder_pid: i32 = sqlx::query_scalar("SELECT pg_backend_pid()")
        .fetch_one(&mut *holder)
        .await
        .unwrap();
    let writers = sqlx::postgres::PgPoolOptions::new()
        .max_connections(1)
        .connect_with((*rig.pool.connect_options()).clone())
        .await
        .unwrap();
    let before: i32 = sqlx::query_scalar("SELECT pg_backend_pid()")
        .fetch_one(&writers)
        .await
        .unwrap();
    let pool = writers.clone();
    let request = tokio::spawn(async move {
        let guard = acquire_membership(&pool, id).await.unwrap();
        drop(guard);
    });
    assert_eq!(blocked_by(&rig.pool, holder_pid).await, before);
    request.abort();
    assert!(request.await.unwrap_err().is_cancelled());
    let after: i32 = tokio::time::timeout(
        std::time::Duration::from_secs(2),
        sqlx::query_scalar("SELECT pg_backend_pid()").fetch_one(&writers),
    )
    .await
    .expect("a cancelled acquiring connection must not monopolize the pool")
    .unwrap();
    assert_ne!(
        before, after,
        "only incomplete acquisition closes its backend"
    );
    tokio::time::timeout(std::time::Duration::from_secs(2), async {
        loop {
            let exists: bool =
                sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM pg_stat_activity WHERE pid=$1)")
                    .bind(before)
                    .fetch_one(&rig.pool)
                    .await
                    .unwrap();
            if !exists {
                return;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("SET LOCAL lock_timeout must end the disconnected backend with holder still locked");
    // No acquisition transaction or its timeout can reach a replacement backend.
    let (timeout, in_transaction): (String, bool) = sqlx::query_as(
        "SELECT current_setting('lock_timeout'), txid_current_if_assigned() IS NOT NULL",
    )
    .fetch_one(&writers)
    .await
    .unwrap();
    assert_eq!(timeout, "0");
    assert!(!in_transaction);
    drop(holder);
    let probe = tokio::time::timeout(
        std::time::Duration::from_secs(2),
        membership_lock(id).acquire(rig.pool.acquire().await.unwrap()),
    )
    .await
    .expect("independent acquisition must succeed after the holder's queued unlock")
    .unwrap();
    drop(probe);
    writers.close().await;
    rig.finish();
}

#[tokio::test]
async fn successful_membership_acquisition_restores_autocommit_and_the_original_timeout() {
    use sqlx::Connection as _;

    let Some(rig) = Rig::new(Settings::for_tests()).await else {
        return;
    };
    let (owner, member, rolled_back) = (wallet(), wallet(), wallet());
    let id = rig.community(&owner, false).await;
    let writers = sqlx::postgres::PgPoolOptions::new()
        .max_connections(1)
        .connect_with((*rig.pool.connect_options()).clone())
        .await
        .unwrap();
    sqlx::query("SET lock_timeout = '5s'")
        .execute(&writers)
        .await
        .unwrap();
    let (before, timeout): (i32, String) =
        sqlx::query_as("SELECT pg_backend_pid(), current_setting('lock_timeout')")
            .fetch_one(&writers)
            .await
            .unwrap();
    let mut guard = acquire_membership(&writers, id).await.unwrap();
    let restored: String = sqlx::query_scalar("SHOW lock_timeout")
        .fetch_one(&mut *guard)
        .await
        .unwrap();
    assert_eq!(restored, timeout);
    sqlx::query("INSERT INTO social.community_members (community_id,member_address,role) VALUES ($1,$2,'member')")
        .bind(id).bind(&member.address).execute(&mut *guard).await.unwrap();
    assert_eq!(
        rig.role_of(id, &member).await.as_deref(),
        Some("member"),
        "source autocommit writes must be visible while the session guard remains held"
    );
    let mut tx = guard.begin().await.unwrap();
    sqlx::query("INSERT INTO social.community_members (community_id,member_address,role) VALUES ($1,$2,'member')")
        .bind(id).bind(&rolled_back.address).execute(&mut *tx).await.unwrap();
    tx.rollback().await.unwrap();
    assert_eq!(rig.role_of(id, &rolled_back).await, None);
    drop(guard);
    let after: i32 = sqlx::query_scalar("SELECT pg_backend_pid()")
        .fetch_one(&writers)
        .await
        .unwrap();
    assert_eq!(
        before, after,
        "completed acquisitions retain the same pooled backend"
    );
    writers.close().await;
    rig.finish();
}

#[tokio::test]
async fn membership_acquisition_budget_also_bounds_pool_checkout() {
    let Some(rig) = Rig::new(Settings::for_tests()).await else {
        return;
    };
    let writers = sqlx::postgres::PgPoolOptions::new()
        .max_connections(1)
        .connect_with((*rig.pool.connect_options()).clone())
        .await
        .unwrap();
    let connection = writers.acquire().await.unwrap();
    let result = tokio::time::timeout(
        std::time::Duration::from_secs(2),
        acquire_membership(&writers, CommunityId::new()),
    )
    .await
    .expect("the acquisition budget must include checkout from a full pool");
    assert!(matches!(result, Err(sqlx::Error::PoolTimedOut)));
    drop(connection);
    writers.close().await;
    rig.finish();
}

#[tokio::test]
async fn interrupted_acquisition_commit_closes_its_backend_before_any_membership_write() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    let Some(rig) = Rig::new(Settings::for_tests()).await else {
        return;
    };
    let owner = wallet();
    for outcome in ["cancel", "timeout", "error"] {
        let member = wallet();
        let id = rig.community(&owner, false).await;
        let options = (*rig.pool.connect_options()).clone();
        let endpoint = (options.get_host().to_owned(), options.get_port());
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let resume = Arc::new(tokio::sync::Notify::new());
        let release = resume.clone();
        let (seen_tx, seen_rx) = tokio::sync::oneshot::channel();
        // Read complete protocol frames so the barrier is the acquisition's
        // COMMIT completion, after the native lock succeeded. The server has
        // committed, but the client still owns an armed acquisition guard.
        // The error case replaces only COMMIT's SQL with SELECT 1/0, causing a
        // real PostgreSQL error at that same await without changing the app.
        let proxy = tokio::spawn(async move {
            let mut sessions = tokio::task::JoinSet::new();
            let (client, _) = listener.accept().await?;
            let server = tokio::net::TcpStream::connect(endpoint.clone()).await?;
            sessions.spawn(async move {
                let (mut client_read, mut client_write) = client.into_split();
                let (mut server_read, mut server_write) = server.into_split();
                let frontend = async move {
                    let length = client_read.read_u32().await?;
                    let mut startup = vec![0; length as usize - 4];
                    client_read.read_exact(&mut startup).await?;
                    server_write.write_u32(length).await?;
                    server_write.write_all(&startup).await?;
                    while let Ok(kind) = client_read.read_u8().await {
                        let length = client_read.read_u32().await?;
                        let mut body = vec![0; length as usize - 4];
                        client_read.read_exact(&mut body).await?;
                        if outcome == "error" && kind == b'P' {
                            let start = body.iter().position(|byte| *byte == 0).unwrap() + 1;
                            let end =
                                start + body[start..].iter().position(|byte| *byte == 0).unwrap();
                            if &body[start..end] == b"COMMIT" {
                                body.splice(start..end, b"SELECT 1/0".iter().copied());
                            }
                        }
                        server_write.write_u8(kind).await?;
                        server_write.write_u32(body.len() as u32 + 4).await?;
                        server_write.write_all(&body).await?;
                    }
                    Ok::<(), std::io::Error>(())
                };
                let backend = async move {
                    let mut seen = Some(seen_tx);
                    while let Ok(kind) = server_read.read_u8().await {
                        let length = server_read.read_u32().await?;
                        let mut body = vec![0; length as usize - 4];
                        server_read.read_exact(&mut body).await?;
                        let boundary = if outcome == "error" {
                            kind == b'E'
                        } else {
                            kind == b'C' && body == b"COMMIT\0"
                        };
                        if boundary && let Some(seen) = seen.take() {
                            seen.send(()).ok();
                            release.notified().await;
                        }
                        client_write.write_u8(kind).await?;
                        client_write.write_u32(length).await?;
                        client_write.write_all(&body).await?;
                    }
                    Ok::<(), std::io::Error>(())
                };
                tokio::try_join!(frontend, backend)
            });
            // The failed acquisition must close its first session. Permit the
            // maxpool1 pool to connect a replacement through this same relay.
            let (mut client, _) = listener.accept().await?;
            let mut server = tokio::net::TcpStream::connect(endpoint).await?;
            tokio::io::copy_bidirectional(&mut client, &mut server).await?;
            sessions.abort_all();
            Ok::<(), std::io::Error>(())
        });
        let writers = sqlx::postgres::PgPoolOptions::new()
            .max_connections(1)
            .connect_with(
                options
                    .host("127.0.0.1")
                    .port(address.port())
                    .ssl_mode(sqlx::postgres::PgSslMode::Disable),
            )
            .await
            .unwrap();
        let (before, original_timeout): (i32, String) =
            sqlx::query_as("SELECT pg_backend_pid(), current_setting('lock_timeout')")
                .fetch_one(&writers)
                .await
                .unwrap();
        let state = Arc::new(State::new(
            writers.clone(),
            rig.state.storage.clone(),
            Settings::for_tests(),
            None,
            &crate::http::Outbound::for_tests(),
            State::explore_for_tests(&rig.pool),
        ));
        let app = router(state);
        let case = Case::new("post", format!("/v1/communities/{id}/members"))
            .signed(&member)
            .await;
        let request = tokio::spawn(async move { hit(&app, case).await });
        tokio::time::timeout(std::time::Duration::from_secs(3), seen_rx)
            .await
            .expect("the request must reach acquisition COMMIT with its lock already held")
            .unwrap();
        let held: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM pg_locks WHERE pid=$1 AND locktype='advisory' AND granted)",
        )
        .bind(before)
        .fetch_one(&rig.pool)
        .await
        .unwrap();
        assert!(
            held,
            "{outcome}: acquisition COMMIT must still own the native lock"
        );
        assert_eq!(rig.role_of(id, &member).await, None);
        if outcome == "cancel" {
            request.abort();
            assert!(request.await.unwrap_err().is_cancelled());
        } else {
            if outcome == "error" {
                resume.notify_one();
            }
            let (status, body) = tokio::time::timeout(std::time::Duration::from_secs(2), request)
                .await
                .expect("acquisition COMMIT must fail within its original budget")
                .unwrap();
            assert_eq!(
                status,
                StatusCode::INTERNAL_SERVER_ERROR,
                "{outcome}: {body}"
            );
            assert_eq!(body.as_object().unwrap().len(), 1, "{body}");
            assert_eq!(
                body["message"],
                if outcome == "error" {
                    "division by zero"
                } else {
                    "pool timed out while waiting for an open connection"
                },
                "{outcome}: {body}"
            );
        }
        // For cancellation and timeout, COMMIT's completion remains withheld
        // while the armed owner closes. Releasing the relay cannot mask a leak.
        tokio::time::timeout(std::time::Duration::from_secs(2), async {
            loop {
                let exists: bool = sqlx::query_scalar(
                    "SELECT EXISTS(SELECT 1 FROM pg_stat_activity WHERE pid=$1)",
                )
                .bind(before)
                .fetch_one(&rig.pool)
                .await
                .unwrap();
                if !exists {
                    return;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("an unconfirmed acquisition COMMIT must close its armed backend");
        let probe = acquire_membership(&rig.pool, id)
            .await
            .expect("an independent same-key caller must obtain the released native lock");
        drop(probe);
        let (after, restored_timeout): (i32, String) = tokio::time::timeout(
            std::time::Duration::from_secs(2),
            sqlx::query_as("SELECT pg_backend_pid(), current_setting('lock_timeout')")
                .fetch_one(&writers),
        )
        .await
        .expect("maxpool1 must replace the armed backend and remain healthy")
        .unwrap();
        assert_ne!(
            before, after,
            "{outcome}: a pooled return must not hide failed cleanup"
        );
        assert_eq!(restored_timeout, original_timeout, "{outcome}");
        let clean: bool =
            sqlx::query_scalar("SELECT xact_start IS NULL FROM pg_stat_activity WHERE pid=$1")
                .bind(after)
                .fetch_one(&rig.pool)
                .await
                .unwrap();
        assert!(
            clean,
            "{outcome}: no acquisition transaction may reach the replacement"
        );
        assert_eq!(
            rig.role_of(id, &member).await,
            None,
            "{outcome}: protected writes must not run"
        );
        resume.notify_one();
        writers.close().await;
        proxy.abort();
    }
    rig.finish();
}
