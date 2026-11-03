//! Real row/advisory barriers hold requests at their mutation boundary.
use super::super::write_support::membership_lock;
use super::*;

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
        guard.release_now().await.unwrap();
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
    barrier.release_now().await.unwrap();
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
