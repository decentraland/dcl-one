//! Writes and reads as upstream (`decentraland/social-service-ea` at
//! `b6b2cef`) answers them: the `placeIds` replacement and the private to
//! public switch of `PUT /v1/communities/{id}`, the ownership transfer, the
//! `onlyMemberOf` order, the bans rows, `isHostingLiveEvent`, the active
//! voice rooms of a non-member, `POST /v1/referral-email` and the
//! notification rows each community event writes.

use super::*;

/// A world place in the catalogue owned by `owner` (`raw.owner`, what the
/// ownership check reads for a world).
async fn owned_world(rig: &Rig, owner: &Wallet) -> String {
    let id = Uuid::new_v4().to_string();
    let name = format!("o{}.dcl.eth", &id[..8]);
    sqlx::query(
        "INSERT INTO explore.place (id, base_position, title, raw) VALUES ($1, '0,0', 'owned world', $2)",
    )
    .bind(&id)
    .bind(json!({ "world": true, "world_name": name, "owner": owner.address.as_str() }))
    .execute(&rig.pool)
    .await
    .unwrap();
    id
}

async fn place_ids(rig: &Rig, id: CommunityId) -> Vec<String> {
    sqlx::query_scalar(
        "SELECT place_id FROM social.community_places WHERE community_id = $1 ORDER BY place_id",
    )
    .bind(id)
    .fetch_all(&rig.pool)
    .await
    .unwrap()
}

async fn put(
    rig: &Rig,
    id: CommunityId,
    w: &Wallet,
    parts: &[(&str, &[u8])],
) -> (StatusCode, Value) {
    let (body, ct) = multipart(parts);
    hit(
        &rig.app,
        Case::new("put", format!("/v1/communities/{id}"))
            .signed(w)
            .await
            .body(body, &ct),
    )
    .await
}

/// `(type, metadata)` of every notification row of `w` about community `id`.
async fn notifications(rig: &Rig, w: &Wallet, id: CommunityId) -> Vec<(String, Value)> {
    sqlx::query_as(
        "SELECT type, metadata FROM social.notifications \
         WHERE address = $1 AND metadata->>'communityId' = $2 ORDER BY occurred_at, type",
    )
    .bind(&w.address)
    .bind(id.to_string())
    .fetch_all(&rig.pool)
    .await
    .unwrap()
}

async fn transfer(rig: &Rig, c: CommunityId, by: &Wallet, to: &Wallet) -> (StatusCode, Value) {
    hit(
        &rig.app,
        Case::new(
            "patch",
            format!("/v1/communities/{c}/members/{}", to.address),
        )
        .signed(by)
        .await
        .json(&json!({ "role": "owner" })),
    )
    .await
}

/// The caller's row of community `c` in the active voice-chat listing.
async fn room(rig: &Rig, c: CommunityId, w: &Wallet) -> Option<Value> {
    let (_, v) = hit(
        &rig.app,
        Case::new("get", "/v1/community-voice-chats/active")
            .signed(w)
            .await,
    )
    .await;
    v["data"]["activeChats"]
        .as_array()
        .unwrap()
        .iter()
        .find(|x| x["communityId"] == c.to_string())
        .cloned()
}

async fn send_email(rig: &Rig, w: &Wallet, body: Value) -> (StatusCode, Value) {
    hit(
        &rig.app,
        Case::new("post", "/v1/referral-email")
            .signed(w)
            .await
            .json(&body),
    )
    .await
}

async fn of_kind(rig: &Rig, w: &Wallet, id: CommunityId, kind: &str) -> Vec<Value> {
    notifications(rig, w, id)
        .await
        .into_iter()
        .filter(|(k, _)| k == kind)
        .map(|(_, m)| m)
        .collect()
}

async fn kinds(rig: &Rig, w: &Wallet, id: CommunityId) -> Vec<String> {
    notifications(rig, w, id)
        .await
        .into_iter()
        .map(|(kind, _)| kind)
        .collect()
}

#[tokio::test]
async fn place_ids_replace_the_places_checking_only_the_new_ones() {
    let Some(rig) = Rig::new(Settings {
        places_ownership_check: true,
        ..Settings::for_tests()
    })
    .await
    else {
        return;
    };
    let (owner, member) = (wallet(), wallet());
    let c = rig.community(&owner, false).await;
    rig.member(c, &member, "member").await;
    // held before the check applied: kept without a catalogue row
    let held = format!("{}", Uuid::new_v4());
    sqlx::query("INSERT INTO social.community_places (place_id, community_id, added_by) VALUES ($1, $2, $3)")
        .bind(&held)
        .bind(c)
        .bind(&owner.address)
        .execute(&rig.pool)
        .await
        .unwrap();
    let mine = owned_world(&rig, &owner).await;
    let theirs = owned_world(&rig, &member).await;

    let ids = serde_json::to_vec(&json!([held, mine, mine])).unwrap();
    let (status, v) = put(&rig, c, &owner, &[("placeIds", &ids)]).await;
    assert_eq!(status, StatusCode::OK, "{v}");
    assert_eq!(v["message"], "Community updated successfully");
    let mut expected = vec![held.clone(), mine.clone()];
    expected.sort();
    assert_eq!(place_ids(&rig, c).await, expected);

    let ids = serde_json::to_vec(&json!([mine, theirs])).unwrap();
    let (status, v) = put(&rig, c, &owner, &[("placeIds", &ids)]).await;
    assert_eq!(
        (status, v),
        (
            StatusCode::UNAUTHORIZED,
            json!({ "error": "Not Authorized", "message": format!("The user {} doesn't own all the places", owner.address) })
        )
    );
    assert_eq!(
        place_ids(&rig, c).await,
        expected,
        "a refusal changes nothing"
    );

    let (status, v) = put(&rig, c, &owner, &[("placeIds", b"not json")]).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{v}");
    assert_eq!(v["message"], "placeIds must be a valid JSON array");

    let ids = serde_json::to_vec(&json!([mine])).unwrap();
    let (status, v) = put(&rig, c, &member, &[("placeIds", &ids)]).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "{v}");
    assert_eq!(
        v["message"],
        format!(
            "The user {} doesn't have permission to edit the community",
            member.address
        )
    );

    // an empty field changes nothing; `[]` removes every place
    let (status, _) = put(&rig, c, &owner, &[("placeIds", b""), ("description", b"d")]).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(place_ids(&rig, c).await, expected);
    let (status, _) = put(&rig, c, &owner, &[("placeIds", b"[]")]).await;
    assert_eq!(status, StatusCode::OK);
    assert!(place_ids(&rig, c).await.is_empty());

    sqlx::query("DELETE FROM explore.place WHERE id = ANY($1)")
        .bind(&[mine, theirs][..])
        .execute(&rig.pool)
        .await
        .unwrap();
    rig.finish();
}

async fn insert_place(pool: &PgPool, c: Uuid, place: &str, added_by: &str, added_at: &str) {
    sqlx::query("INSERT INTO social.community_places (place_id, community_id, added_by, added_at) VALUES ($1, $2, $3, $4::timestamp)")
        .bind(place)
        .bind(c)
        .bind(added_by)
        .bind(added_at)
        .execute(pool)
        .await
        .unwrap();
}

/// An owned database as one stamped before the lowercase check: mixed-case
/// rows can still be there until the release script runs.
async fn before_the_place_id_release(tag: &str) -> Option<crate::db::scratch::OwnedDatabase> {
    let database =
        crate::db::scratch::OwnedDatabase::new(tag, crate::db::scratch::Schema::Full, 4).await?;
    sqlx::query(
        "ALTER TABLE social.community_places DROP CONSTRAINT community_places_place_id_lowercase",
    )
    .execute(&database.pool)
    .await
    .unwrap();
    Some(database)
}

async fn place_rows(pool: &PgPool, c: CommunityId) -> Vec<(String, String, String)> {
    sqlx::query_as(
        "SELECT place_id, added_by, added_at::text FROM social.community_places \
         WHERE community_id = $1 ORDER BY place_id",
    )
    .bind(c)
    .fetch_all(pool)
    .await
    .unwrap()
}

#[tokio::test]
async fn place_ids_are_stored_lowercased_and_once_per_case_variant() {
    let Some(rig) = Rig::new(Settings {
        places_ownership_check: false,
        ..Settings::for_tests()
    })
    .await
    else {
        return;
    };
    let owner = wallet();
    rig.profile(&owner, "Named owner", true).await;
    let place = Uuid::new_v4().to_string();
    let upper = place.to_uppercase();

    // create
    let ids = serde_json::to_vec(&json!([upper, place, "Mixed.DCL.eth"])).unwrap();
    let (bytes, content_type) = multipart(&[
        ("name", b"Cased places"),
        ("description", b"description"),
        ("placeIds", &ids),
    ]);
    let (status, created) = hit(
        &rig.app,
        Case::new("post", "/v1/communities")
            .signed(&owner)
            .await
            .body(bytes, &content_type),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{created}");
    let c = CommunityId::parse(created["data"]["id"].as_str().unwrap()).unwrap();
    let mut expected = vec![place.clone(), "mixed.dcl.eth".to_string()];
    expected.sort();
    assert_eq!(place_ids(&rig, c).await, expected);

    // add
    let (status, v) = hit(
        &rig.app,
        Case::new("post", format!("/v1/communities/{c}/places"))
            .signed(&owner)
            .await
            .json(&json!({ "placeIds": ["Other.DCL.eth", "OTHER.dcl.eth", upper] })),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT, "{v}");
    expected.push("other.dcl.eth".to_string());
    expected.sort();
    assert_eq!(place_ids(&rig, c).await, expected);

    // remove in another spelling
    let (status, v) = hit(
        &rig.app,
        Case::new(
            "delete",
            format!("/v1/communities/{c}/places/OTHER.DCL.ETH"),
        )
        .signed(&owner)
        .await,
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT, "{v}");
    expected.retain(|p| p != "other.dcl.eth");
    assert_eq!(place_ids(&rig, c).await, expected);

    // the table refuses any other case
    let refused = sqlx::query("INSERT INTO social.community_places (place_id, community_id, added_by) VALUES ('Upper.dcl.eth', $1, 'w')")
        .bind(c)
        .execute(&rig.pool)
        .await;
    assert!(refused.is_err(), "{refused:?}");
    rig.finish();
}

#[tokio::test]
async fn rows_stored_in_another_case_keep_working_until_the_release() {
    let Some(database) = before_the_place_id_release("place_ids_legacy").await else {
        return;
    };
    let rig = Rig::with_pool(
        Settings {
            places_ownership_check: false,
            ..Settings::for_tests()
        },
        State::explore_for_tests,
        database.pool.clone(),
    )
    .await;
    let owner = wallet();
    let c = rig.community(&owner, false).await;
    let by = owner.address.as_str();
    for (place, at) in [
        ("Kept.DCL.eth", "2026-01-01 00:00:00"),
        ("kept.dcl.ETH", "2026-01-02 00:00:00"),
        ("Gone.DCL.eth", "2026-01-03 00:00:00"),
        ("Dropped.DCL.eth", "2026-01-04 00:00:00"),
    ] {
        insert_place(&rig.pool, c.as_uuid(), place, by, at).await;
    }

    // an add of a held place writes no second row
    let (status, v) = hit(
        &rig.app,
        Case::new("post", format!("/v1/communities/{c}/places"))
            .signed(&owner)
            .await
            .json(&json!({ "placeIds": ["gone.dcl.eth"] })),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT, "{v}");
    let gone: Vec<String> = sqlx::query_scalar(
        "SELECT place_id FROM social.community_places WHERE community_id = $1 AND lower(place_id) = 'gone.dcl.eth'",
    )
    .bind(c)
    .fetch_all(&rig.pool)
    .await
    .unwrap();
    assert_eq!(gone, ["Gone.DCL.eth"]);
    // a removal in any spelling reaches the stored row
    let (status, v) = hit(
        &rig.app,
        Case::new("delete", format!("/v1/communities/{c}/places/gone.dcl.eth"))
            .signed(&owner)
            .await,
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT, "{v}");

    // an edit lowercases the kept place in place: one row, its first adder and time
    let ids = serde_json::to_vec(&json!(["KEPT.dcl.eth", "new.dcl.eth"])).unwrap();
    let (status, v) = put(&rig, c, &owner, &[("placeIds", &ids)]).await;
    assert_eq!(status, StatusCode::OK, "{v}");
    let rows = place_rows(&rig.pool, c).await;
    assert_eq!(
        rows.iter().map(|r| r.0.as_str()).collect::<Vec<_>>(),
        ["kept.dcl.eth", "new.dcl.eth"]
    );
    assert_eq!(rows[0].2, "2026-01-01 00:00:00");
    assert_eq!(rows[0].1, by);
    rig.finish();
    database.drop().await;
}

#[tokio::test]
async fn an_edit_keeps_a_held_place_stored_in_another_case() {
    let Some(database) = before_the_place_id_release("place_ids_held").await else {
        return;
    };
    let rig = Rig::with_pool(
        Settings {
            places_ownership_check: true,
            ..Settings::for_tests()
        },
        State::explore_for_tests,
        database.pool.clone(),
    )
    .await;
    let owner = wallet();
    let c = rig.community(&owner, false).await;
    // held, with no catalogue row: only its standing passes the ownership check
    let held = Uuid::new_v4().to_string();
    insert_place(
        &rig.pool,
        c.as_uuid(),
        &held.to_uppercase(),
        owner.address.as_str(),
        "2026-01-01",
    )
    .await;
    let ids = serde_json::to_vec(&json!([held])).unwrap();
    let (status, v) = put(&rig, c, &owner, &[("placeIds", &ids)]).await;
    assert_eq!(status, StatusCode::OK, "{v}");
    assert_eq!(place_ids(&rig, c).await, vec![held]);
    rig.finish();
    database.drop().await;
}

#[tokio::test]
async fn the_place_id_release_merges_case_variants_validates_the_check_and_can_run_again() {
    // its own database: the release rewrites the whole table
    let Some(database) = before_the_place_id_release("place_ids_release").await else {
        return;
    };
    let pool = database.pool.clone();
    let (c, d) = (Uuid::new_v4(), Uuid::new_v4());
    sqlx::query("INSERT INTO social.communities (id, name, description, owner_address) VALUES ($1, 'c', '', 'o'), ($2, 'd', '', 'o')")
        .bind(c)
        .bind(d)
        .execute(&pool)
        .await
        .unwrap();
    for (community, place, by, at) in [
        // the lowercase spelling wins over an earlier variant
        (c, "A.dcl.eth", "other", "2026-01-01"),
        (c, "a.dcl.eth", "owner", "2026-01-02"),
        // no lowercase spelling: the earliest wins and is lowercased
        (c, "B.DCL.eth", "owner", "2026-01-03"),
        (c, "b.DCL.ETH", "other", "2026-01-04"),
        // one community's variant leaves another's alone
        (d, "B.dcl.eth", "other", "2026-01-05"),
        (d, "c.dcl.eth", "owner", "2026-01-06"),
    ] {
        insert_place(&pool, community, place, by, at).await;
    }
    let release = include_str!(concat!(
        env!("DCL_ONE_ROOT"),
        "/schemas/releases/community-place-ids-lowercase.sql"
    ));
    let expected: Vec<(Uuid, String, String)> = [
        (c, "a.dcl.eth", "owner"),
        (c, "b.dcl.eth", "owner"),
        (d, "b.dcl.eth", "other"),
        (d, "c.dcl.eth", "owner"),
    ]
    .into_iter()
    .map(|(community, place, by)| (community, place.to_string(), by.to_string()))
    .collect();
    for run in 0..2 {
        sqlx::raw_sql(release).execute(&pool).await.unwrap();
        let rows: Vec<(Uuid, String, String)> = sqlx::query_as(
            "SELECT community_id, place_id, added_by FROM social.community_places \
             ORDER BY community_id = $1 DESC, place_id",
        )
        .bind(c)
        .fetch_all(&pool)
        .await
        .unwrap();
        assert_eq!(rows, expected, "run {run}");
        let validated: bool = sqlx::query_scalar(
            "SELECT convalidated FROM pg_constraint \
             WHERE conname = 'community_places_place_id_lowercase' \
             AND conrelid = 'social.community_places'::regclass",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert!(validated, "run {run}");
    }
    database.drop().await;
}

#[tokio::test]
async fn going_public_accepts_the_pending_requests_to_join_of_unbanned_wallets() {
    let Some(rig) = Rig::new(Settings::for_tests()).await else {
        return;
    };
    let (owner, asker, banned, invited) = (wallet(), wallet(), wallet(), wallet());
    let c = rig.community(&owner, true).await;
    let asked = rig.request(c, &asker, "request_to_join").await;
    let refused = rig.request(c, &banned, "request_to_join").await;
    let invite = rig.request(c, &invited, "invite").await;
    // Federation decisions keep the decided row; going public must not accept it.
    let (rejected_wallet, cancelled_wallet) = (wallet(), wallet());
    let rejected = rig.request(c, &rejected_wallet, "request_to_join").await;
    let cancelled = rig.request(c, &cancelled_wallet, "request_to_join").await;
    for (id, decided) in [(rejected, "rejected"), (cancelled, "cancelled")] {
        sqlx::query("UPDATE social.community_requests SET status = $2 WHERE id = $1")
            .bind(id)
            .bind(decided)
            .execute(&rig.pool)
            .await
            .unwrap();
    }
    sqlx::query("INSERT INTO social.community_bans (community_id, banned_address, banned_by) VALUES ($1, $2, $3)")
        .bind(c)
        .bind(&banned.address)
        .bind(&owner.address)
        .execute(&rig.pool)
        .await
        .unwrap();

    let (status, v) = put(&rig, c, &owner, &[("privacy", b"public")]).await;
    assert_eq!(status, StatusCode::OK, "{v}");
    assert_eq!(v["data"]["privacy"], "public");
    let status_of = |id: Uuid| {
        let pool = rig.pool.clone();
        async move {
            sqlx::query_scalar::<_, String>(
                "SELECT status FROM social.community_requests WHERE id = $1",
            )
            .bind(id)
            .fetch_optional(&pool)
            .await
            .unwrap()
        }
    };
    assert_eq!(status_of(asked).await, None);
    assert_eq!(rig.role_of(c, &asker).await.as_deref(), Some("member"));
    assert_eq!(
        status_of(refused).await.as_deref(),
        Some("pending"),
        "a ban keeps it pending"
    );
    assert_eq!(rig.role_of(c, &banned).await, None);
    assert_eq!(
        status_of(invite).await.as_deref(),
        Some("pending"),
        "invites are not requests to join"
    );
    assert_eq!(rig.role_of(c, &invited).await, None);
    assert_eq!(status_of(rejected).await.as_deref(), Some("rejected"));
    assert_eq!(rig.role_of(c, &rejected_wallet).await, None);
    assert_eq!(status_of(cancelled).await.as_deref(), Some("cancelled"));
    assert_eq!(rig.role_of(c, &cancelled_wallet).await, None);

    // public to public accepts nothing
    let late = wallet();
    let pending = rig.request(c, &late, "request_to_join").await;
    let (status, _) = put(&rig, c, &owner, &[("privacy", b"public")]).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(status_of(pending).await.as_deref(), Some("pending"));
    rig.finish();
}

#[tokio::test]
async fn the_owner_role_transfers_ownership_to_a_named_member() {
    let Some(rig) = Rig::new(Settings::for_tests()).await else {
        return;
    };
    let (owner, moderator, member, stranger) = (wallet(), wallet(), wallet(), wallet());
    let c = rig.community(&owner, false).await;
    rig.member(c, &moderator, "moderator").await;
    rig.member(c, &member, "member").await;
    rig.profile(&member, "named", true).await;
    rig.profile(&stranger, "named too", true).await;
    let (status, v) = transfer(&rig, c, &moderator, &member).await;
    assert_eq!(
        (status, v),
        (
            StatusCode::UNAUTHORIZED,
            json!({ "error": "Not Authorized", "message": format!(
                "The user {} doesn't have permission to transfer ownership in community {c}",
                moderator.address
            ) })
        )
    );
    let (status, v) = transfer(&rig, c, &owner, &stranger).await;
    assert_eq!(
        (status, v),
        (
            StatusCode::UNAUTHORIZED,
            json!({ "error": "Not Authorized", "message": format!(
                "The target user {} is not a member of community {c}",
                stranger.address
            ) })
        )
    );
    let (status, v) = transfer(&rig, c, &owner, &moderator).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "no NAME: {v}");

    let (status, v) = transfer(&rig, c, &owner, &member).await;
    assert_eq!(status, StatusCode::NO_CONTENT, "{v}");
    assert_eq!(rig.role_of(c, &member).await.as_deref(), Some("owner"));
    assert_eq!(rig.role_of(c, &owner).await.as_deref(), Some("moderator"));
    let stored: String =
        sqlx::query_scalar("SELECT owner_address FROM social.communities WHERE id = $1")
            .bind(c)
            .fetch_one(&rig.pool)
            .await
            .unwrap();
    assert_eq!(stored, member.address.as_str());
    let rows = notifications(&rig, &member, c).await;
    assert_eq!(rows.len(), 1, "{rows:?}");
    assert_eq!(rows[0].0, "community_ownership_transferred");
    assert_eq!(rows[0].1["oldOwnerAddress"], owner.address.as_str());
    assert_eq!(
        rows[0].1["communityName"],
        format!("community {}", &c.to_string()[..8])
    );
    assert_eq!(rows[0].1["thumbnailUrl"], rig.state.thumbnail_url(c));

    // the old owner is now refused, the new one may hand it back
    let (status, _) = transfer(&rig, c, &owner, &member).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    rig.finish();
}

#[tokio::test]
async fn only_member_of_lists_by_role_then_name() {
    let Some(rig) = Rig::new(Settings::for_tests()).await else {
        return;
    };
    let (me, other) = (wallet(), wallet());
    let tag = Uuid::new_v4().simple().to_string()[..8].to_string();
    let mut ids = Vec::new();
    for (name, role) in [
        ("a member", "member"),
        ("z owner", "owner"),
        ("b member", "member"),
        ("m moderator", "moderator"),
    ] {
        let c = if role == "owner" {
            rig.community(&me, false).await
        } else {
            let c = rig.community(&other, false).await;
            rig.member(c, &me, role).await;
            c
        };
        sqlx::query("UPDATE social.communities SET name = $2, ranking_score = $3 WHERE id = $1")
            .bind(c)
            .bind(format!("{tag} {name}"))
            .bind(if name == "b member" { 100.0f32 } else { 0.0 })
            .execute(&rig.pool)
            .await
            .unwrap();
        ids.push((name, c));
    }
    let (status, v) = hit(
        &rig.app,
        Case::new("get", "/v1/communities?onlyMemberOf=true&limit=100")
            .signed(&me)
            .await,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{v}");
    let names: Vec<&str> = v["data"]["results"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|r| r["name"].as_str())
        .filter(|n| n.starts_with(&tag))
        .collect();
    assert_eq!(
        names,
        [
            format!("{tag} z owner"),
            format!("{tag} m moderator"),
            format!("{tag} a member"),
            format!("{tag} b member"),
        ],
        "the rank does not reorder a member's own listing"
    );
    rig.finish();
}

#[tokio::test]
async fn ban_rows_carry_millisecond_banned_at_and_the_friendship_status() {
    let Some(rig) = Rig::new(Settings::for_tests()).await else {
        return;
    };
    let (owner, friend) = (wallet(), wallet());
    let c = rig.community(&owner, false).await;
    rig.profile(&friend, "Friend", true).await;
    rig.friends(&owner, &friend).await;
    sqlx::query("INSERT INTO social.community_bans (community_id, banned_address, banned_by, banned_at) VALUES ($1, $2, $3, '2025-03-04 05:06:07.891234')")
        .bind(c)
        .bind(&friend.address)
        .bind(&owner.address)
        .execute(&rig.pool)
        .await
        .unwrap();
    for version in ["v1", "v2"] {
        let (status, v) = hit(
            &rig.app,
            Case::new("get", format!("/{version}/communities/{c}/bans"))
                .signed(&owner)
                .await,
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{version}: {v}");
        let row = &v["data"]["results"][0];
        assert_eq!(row["bannedAt"], "2025-03-04T05:06:07.891Z", "{version}");
        assert_eq!(row["friendshipStatus"], 3, "{version}: ACCEPTED");
    }
    rig.finish();
}

#[tokio::test]
async fn banning_again_after_a_lift_records_a_new_ban() {
    let Some(rig) = Rig::new(Settings::for_tests()).await else {
        return;
    };
    let (owner, earlier, member) = (wallet(), wallet(), wallet());
    let c = rig.community(&owner, false).await;
    rig.member(c, &member, "member").await;
    sqlx::query(
        "INSERT INTO social.community_bans (community_id, banned_address, banned_by, banned_at, active, unbanned_by, unbanned_at) \
         VALUES ($1, $2, $3, '2025-01-01', false, $3, '2025-01-02')",
    )
    .bind(c)
    .bind(&member.address)
    .bind(&earlier.address)
    .execute(&rig.pool)
    .await
    .unwrap();
    let (status, v) = hit(
        &rig.app,
        Case::new(
            "post",
            format!("/v1/communities/{c}/members/{}/bans", member.address),
        )
        .signed(&owner)
        .await,
    )
    .await;
    assert!(status.is_success(), "{status}: {v}");
    let (active, by, fresh): (bool, String, bool) = sqlx::query_as(
        "SELECT active, banned_by, banned_at > '2025-01-02' FROM social.community_bans \
         WHERE community_id = $1 AND banned_address = $2",
    )
    .bind(c)
    .bind(&member.address)
    .fetch_one(&rig.pool)
    .await
    .unwrap();
    assert_eq!(
        (active, by.as_str(), fresh),
        (true, owner.address.as_str(), true)
    );
    rig.finish();
}

#[tokio::test]
async fn a_signed_detail_says_whether_the_community_hosts_a_live_event() {
    let Some(rig) = Rig::new(Settings::for_tests()).await else {
        return;
    };
    let (owner, viewer) = (wallet(), wallet());
    let c = rig.community(&owner, false).await;
    let hosting = |path: String| {
        let app = rig.app.clone();
        let viewer = &viewer;
        async move {
            let (status, v) = hit(&app, Case::new("get", path).signed(viewer).await).await;
            assert_eq!(status, StatusCode::OK, "{v}");
            v["data"]["isHostingLiveEvent"].clone()
        }
    };
    for version in ["v1", "v2"] {
        assert_eq!(hosting(format!("/{version}/communities/{c}")).await, false);
    }
    let event = format!("ev-{}", Uuid::new_v4());
    sqlx::query(
        "INSERT INTO explore.event (id, name, start_at, finish_at, next_start_at, next_finish_at, duration_ms, approved, community_id, raw) \
         VALUES ($1, 'live now', timezone('UTC', now()) - interval '1 hour', timezone('UTC', now()) + interval '1 hour', \
                 timezone('UTC', now()) - interval '1 hour', timezone('UTC', now()) + interval '1 hour', 7200000, TRUE, $2, '{}'::jsonb)",
    )
    .bind(&event)
    .bind(c.to_string())
    .execute(&rig.pool)
    .await
    .unwrap();
    for version in ["v1", "v2"] {
        assert_eq!(hosting(format!("/{version}/communities/{c}")).await, true);
    }
    let (_, v) = hit(&rig.app, Case::new("get", format!("/v1/communities/{c}"))).await;
    assert!(
        v["data"].get("isHostingLiveEvent").is_none(),
        "the public information has no such field: {v}"
    );
    sqlx::query("DELETE FROM explore.event WHERE id = $1")
        .bind(&event)
        .execute(&rig.pool)
        .await
        .unwrap();
    rig.finish();
}

#[tokio::test]
async fn a_non_member_sees_a_room_only_through_its_places_and_never_when_banned() {
    let _voice = VOICE_ROWS.lock().await;
    let Some(rig) = Rig::new(Settings::for_tests()).await else {
        return;
    };
    let (owner, member, viewer, banned) = (wallet(), wallet(), wallet(), wallet());
    let c = rig.community(&owner, false).await;
    rig.member(c, &member, "member").await;
    rig.voice(c, &owner, "connected", true).await;
    assert!(
        room(&rig, c, &viewer).await.is_none(),
        "no places, nothing to show a non-member"
    );
    let chat = room(&rig, c, &member)
        .await
        .expect("a member always sees it");
    assert_eq!(
        (chat["positions"].clone(), chat["worlds"].clone()),
        (json!([]), json!([]))
    );

    let world = rig.world_place(c, &owner).await;
    let chat = room(&rig, c, &viewer)
        .await
        .expect("listed through its world");
    assert_eq!(chat["worlds"], json!([world]));
    assert_eq!(chat["isMember"], false);
    let chat = room(&rig, c, &member).await.unwrap();
    assert_eq!(
        chat["worlds"],
        json!([]),
        "a member's room carries no places"
    );

    sqlx::query("INSERT INTO social.community_bans (community_id, banned_address, banned_by) VALUES ($1, $2, $3)")
        .bind(c)
        .bind(&banned.address)
        .bind(&owner.address)
        .execute(&rig.pool)
        .await
        .unwrap();
    assert!(
        room(&rig, c, &banned).await.is_none(),
        "a banned wallet does not see it"
    );
    rig.clear_world_place(&world).await;
    rig.finish();
}

#[tokio::test]
async fn referral_email_is_checked_and_kept_once_a_day() {
    let Some(rig) = Rig::new(Settings::for_tests()).await else {
        return;
    };
    let app = &rig.app;
    let referrer = wallet();
    let (status, v) = hit(
        app,
        Case::new("post", "/v1/referral-email").json(&json!({ "email": "jane.doe@example.com" })),
    )
    .await;
    assert_gate_400(status, &v, "referral email unsigned");

    let (status, v) = send_email(&rig, &referrer, json!({ "email": "jane.doe@example.com" })).await;
    assert_eq!(
        (status, v),
        (
            StatusCode::BAD_REQUEST,
            json!({ "error": "Bad request", "message": "You must have at least 100 accepted invites to set an email" })
        )
    );
    sqlx::query(
        "INSERT INTO social.referral_progress (invited_user, referrer, status, tier_granted) \
         SELECT '0x' || substr(md5(random()::text) || md5(random()::text), 1, 40), $1, 'tier_granted', true \
         FROM generate_series(1, 100)",
    )
    .bind(&referrer.address)
    .execute(&rig.pool)
    .await
    .unwrap();
    let (status, v) = send_email(&rig, &referrer, json!({ "email": "not-an-email" })).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(v["message"], "Invalid JSON body");
    assert_eq!(v["data"][0]["keyword"], "format");
    let (status, v) = send_email(&rig, &referrer, json!({})).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(v["message"], "Invalid JSON body");
    assert_eq!(v["data"][0]["keyword"], "required");

    let (status, v) = send_email(&rig, &referrer, json!({ "email": "Jane.Doe@Example.com" })).await;
    assert_eq!(status, StatusCode::NO_CONTENT, "{v}");
    let stored: String =
        sqlx::query_scalar("SELECT email FROM social.referral_emails WHERE referrer = $1")
            .bind(&referrer.address)
            .fetch_one(&rig.pool)
            .await
            .unwrap();
    assert_eq!(stored, "jane.doe@example.com");
    let (status, v) = send_email(&rig, &referrer, json!({ "email": "john.doe@example.com" })).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(
        v["message"],
        format!(
            "Email can only be updated once per day. Last update was less than 24 hours ago for user: {}",
            referrer.address
        )
    );
    rig.finish();
}

#[tokio::test]
async fn community_events_write_the_notification_rows_upstream_publishes() {
    let Some(rig) = Rig::new(admin_settings(None)).await else {
        return;
    };
    let app = &rig.app;
    let (owner, moderator, member, quiet, asker, invitee, kicked) = (
        wallet(),
        wallet(),
        wallet(),
        wallet(),
        wallet(),
        wallet(),
        wallet(),
    );
    let c = rig.community(&owner, true).await;
    for (w, role) in [
        (&moderator, "moderator"),
        (&member, "member"),
        (&quiet, "member"),
        (&kicked, "member"),
    ] {
        rig.member(c, w, role).await;
    }
    rig.profile(&asker, "Asker", false).await;
    // `quiet` opted out of this community
    sqlx::query("INSERT INTO social.subscription_opt_outs (address, scope, scope_id) VALUES ($1, 'community', $2)")
        .bind(&quiet.address)
        .bind(c.to_string().to_uppercase())
        .execute(&rig.pool)
        .await
        .unwrap();

    // a request to join reaches the owner and the moderators
    let (status, v) = hit(
        app,
        Case::new("post", format!("/v1/communities/{c}/requests"))
            .signed(&asker)
            .await
            .json(&json!({ "targetedAddress": asker.address.as_str(), "type": "request_to_join" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{v}");
    let request = v["data"]["id"].as_str().unwrap().to_string();
    for w in [&owner, &moderator] {
        let rows = notifications(&rig, w, c).await;
        assert_eq!(rows.len(), 1, "{rows:?}");
        assert_eq!(rows[0].0, "community_request_to_join_received");
        assert_eq!(rows[0].1["memberAddress"], asker.address.as_str());
        assert_eq!(rows[0].1["memberName"], "Asker");
    }
    assert!(kinds(&rig, &member, c).await.is_empty());

    // accepting it tells the requester
    let (status, _) = hit(
        app,
        Case::new("patch", format!("/v1/communities/{c}/requests/{request}"))
            .signed(&owner)
            .await
            .json(&json!({ "intention": "accepted" })),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    assert_eq!(
        kinds(&rig, &asker, c).await,
        ["community_request_to_join_accepted"]
    );

    // an invite tells the invitee
    let (status, v) = hit(
        app,
        Case::new("post", format!("/v1/communities/{c}/requests"))
            .signed(&owner)
            .await
            .json(&json!({ "targetedAddress": invitee.address.as_str(), "type": "invite" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{v}");
    let rows = notifications(&rig, &invitee, c).await;
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].0, "community_invite_received");
    assert!(rows[0].1.get("memberAddress").is_none(), "{rows:?}");

    // a post reaches everyone but its author and the opted-out member
    let (status, v) = hit(
        app,
        Case::new("post", format!("/v1/communities/{c}/posts"))
            .signed(&owner)
            .await
            .json(&json!({ "content": "hello" })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{v}");
    let rows = of_kind(&rig, &member, c, "community_post_added").await;
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0]["postId"], v["data"]["id"]);
    assert_eq!(rows[0]["authorAddress"], owner.address.as_str());
    assert!(
        of_kind(&rig, &owner, c, "community_post_added")
            .await
            .is_empty(),
        "not the author"
    );
    assert!(kinds(&rig, &quiet, c).await.is_empty(), "opted out");

    // a kick and a ban tell the removed wallet; a repeat ban is the same row
    let (status, _) = hit(
        app,
        Case::new(
            "delete",
            format!("/v1/communities/{c}/members/{}", kicked.address),
        )
        .signed(&owner)
        .await,
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    assert_eq!(
        of_kind(&rig, &kicked, c, "community_member_removed")
            .await
            .len(),
        1
    );
    let ban = format!("/v1/communities/{c}/members/{}/bans", member.address);
    for _ in 0..2 {
        let (status, _) = hit(app, Case::new("post", &ban).signed(&owner).await).await;
        assert_eq!(status, StatusCode::NO_CONTENT);
        let (status, _) = hit(app, Case::new("delete", &ban).signed(&owner).await).await;
        assert_eq!(status, StatusCode::NO_CONTENT);
        rig.member(c, &member, "member").await;
    }
    assert_eq!(
        of_kind(&rig, &member, c, "community_member_banned")
            .await
            .len(),
        1,
        "keyed per banned wallet"
    );

    // a rename reaches every member, the renamer included
    let (status, _) = put(&rig, c, &owner, &[("name", b"Renamed Club")]).await;
    assert_eq!(status, StatusCode::OK);
    // Source updateCommunity launches the rename broadcast without awaiting
    // delivery. Wait for this fixture's inbox row before inspecting its exact
    // contents and proceeding to deletion; the HTTP response is no delivery ack.
    let renamed = tokio::time::timeout(std::time::Duration::from_secs(5), async {
        loop {
            let rows = of_kind(&rig, &owner, c, "community_renamed").await;
            if !rows.is_empty() {
                return rows;
            }
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();
    assert_eq!(renamed.len(), 1);
    assert_eq!(
        renamed[0]["oldName"],
        format!("community {}", &c.to_string()[..8])
    );
    assert_eq!(renamed[0]["newName"], "Renamed Club");
    assert_eq!(renamed[0]["thumbnailUrl"], "N/A");

    // the owner's deletion reaches every member but the owner
    let (status, _) = hit(
        app,
        Case::new("delete", format!("/v1/communities/{c}"))
            .signed(&owner)
            .await,
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    assert!(
        kinds(&rig, &moderator, c)
            .await
            .contains(&"community_deleted".to_string())
    );
    let owner_kinds = kinds(&rig, &owner, c).await;
    assert!(
        !owner_kinds.contains(&"community_deleted".to_string()),
        "{owner_kinds:?}"
    );
    assert!(!owner_kinds.contains(&"community_deleted_content_violation".to_string()));
    rig.finish();
}

#[tokio::test]
async fn a_global_moderators_deletion_tells_the_owner_of_the_violation() {
    let moderator = wallet();
    let Some(rig) = Rig::new(admin_settings(Some(&moderator))).await else {
        return;
    };
    let (owner, member) = (wallet(), wallet());
    let c = rig.community(&owner, false).await;
    rig.member(c, &member, "member").await;
    let (status, _) = hit(
        &rig.app,
        Case::new("delete", format!("/v1/communities/{c}"))
            .signed(&moderator)
            .await,
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let rows = notifications(&rig, &owner, c).await;
    assert_eq!(rows.len(), 1, "{rows:?}");
    assert_eq!(rows[0].0, "community_deleted_content_violation");
    assert_eq!(
        rows[0].1["communityName"],
        format!("community {}", &c.to_string()[..8])
    );
    assert_eq!(kinds(&rig, &member, c).await, ["community_deleted"]);
    rig.finish();
}

/// Upstream's `COMMUNITY_MEMBER_STATUS_UPDATES_CHANNEL` and
/// `COMMUNITY_DELETED_UPDATES_CHANNEL` (`src/logic/community/members.ts`,
/// `requests.ts`, `bans.ts`, `communities.ts`; delivered by
/// `src/logic/updates.ts` `communityMemberStatusHandler` and
/// `communityDeletedUpdateHandler` at `b6b2cef`): a join (directly or by an
/// accepted request) is ONLINE, a leave, kick or ban OFFLINE, to the
/// community's members on an open stream and to the wallet itself, member or
/// no longer; a deletion tells each member on an open stream it went
/// OFFLINE. Wallets outside the community hear nothing.
#[tokio::test]
async fn membership_changes_and_deletion_reach_the_open_member_streams() {
    use crate::rpc::proto::v2::ConnectivityStatus;
    use crate::rpc::pubsub::{PubSub, SocialEvent};
    let Some(rig) = Rig::new(admin_settings(None)).await else {
        return;
    };
    let updates = PubSub::new();
    rig.state.set_updates(updates.clone());
    let (owner, joiner, kicked, banned, asker, stranger) =
        (wallet(), wallet(), wallet(), wallet(), wallet(), wallet());
    let c = rig.community(&owner, false).await;
    rig.member(c, &kicked, "member").await;
    let mut rx: Vec<_> = [&owner, &joiner, &kicked, &banned, &asker, &stranger]
        .iter()
        .map(|w| updates.subscribe(w.address.as_str()))
        .collect();
    let (owner_rx, joiner_rx, kicked_rx, banned_rx, asker_rx, stranger_rx) = (0, 1, 2, 3, 4, 5);
    let mut heard = |i: usize| -> Vec<(String, String, i32)> {
        std::iter::from_fn(|| rx[i].try_recv().ok())
            .map(|e| match &*e {
                SocialEvent::CommunityMemberStatus(u) => (
                    u.community_id.clone(),
                    u.member.as_ref().unwrap().address.clone(),
                    u.status,
                ),
                _ => panic!("unexpected event"),
            })
            .collect()
    };
    let (online, offline) = (
        ConnectivityStatus::Online as i32,
        ConnectivityStatus::Offline as i32,
    );
    let about = |w: &Wallet, status| vec![(c.to_string(), w.address.to_string(), status)];
    let member_path = |w: &Wallet| format!("/v1/communities/{c}/members/{}", w.address);
    let join = format!("/v1/communities/{c}/members");

    // a join
    let (status, body) = hit(&rig.app, Case::new("post", &join).signed(&joiner).await).await;
    assert_eq!(status, StatusCode::NO_CONTENT, "{body}");
    assert_eq!(heard(owner_rx), about(&joiner, online), "the owner");
    assert_eq!(heard(kicked_rx), about(&joiner, online), "a member");
    assert_eq!(
        heard(joiner_rx),
        about(&joiner, online),
        "the joiner itself"
    );
    assert!(heard(stranger_rx).is_empty(), "not a member");
    // joining again changes nothing and tells no one
    let (status, _) = hit(&rig.app, Case::new("post", &join).signed(&joiner).await).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    assert!(heard(owner_rx).is_empty());

    // a leave: the members and the wallet that left
    let (status, _) = hit(
        &rig.app,
        Case::new("delete", member_path(&joiner))
            .signed(&joiner)
            .await,
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    assert_eq!(heard(owner_rx), about(&joiner, offline));
    assert_eq!(heard(kicked_rx), about(&joiner, offline));
    assert_eq!(
        heard(joiner_rx),
        about(&joiner, offline),
        "no longer a member"
    );
    // a kick
    let (status, _) = hit(
        &rig.app,
        Case::new("delete", member_path(&kicked))
            .signed(&owner)
            .await,
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    assert_eq!(heard(owner_rx), about(&kicked, offline));
    assert_eq!(heard(kicked_rx), about(&kicked, offline));
    assert!(heard(joiner_rx).is_empty(), "a former member");
    // a ban of a wallet that never joined still tells it
    let (status, _) = hit(
        &rig.app,
        Case::new("post", format!("{}/bans", member_path(&banned)))
            .signed(&owner)
            .await,
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    assert_eq!(heard(owner_rx), about(&banned, offline));
    assert_eq!(heard(banned_rx), about(&banned, offline));

    // an accepted request to join
    let request = rig.request(c, &asker, "request_to_join").await;
    let (status, body) = hit(
        &rig.app,
        Case::new("patch", format!("/v1/communities/{c}/requests/{request}"))
            .signed(&owner)
            .await
            .json(&json!({ "intention": "accepted" })),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT, "{body}");
    assert_eq!(heard(owner_rx), about(&asker, online));
    assert_eq!(heard(asker_rx), about(&asker, online));

    // the deletion: each member hears itself OFFLINE
    let (status, _) = hit(
        &rig.app,
        Case::new("delete", format!("/v1/communities/{c}"))
            .signed(&owner)
            .await,
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    // Deletion's source helper broadcasts SNS and then realtime in the
    // background. The committed HTTP response does not wait for delivery.
    for (index, wallet) in [(owner_rx, &owner), (asker_rx, &asker)] {
        let events = tokio::time::timeout(std::time::Duration::from_secs(5), async {
            loop {
                let events = heard(index);
                if !events.is_empty() {
                    break events;
                }
                tokio::time::sleep(std::time::Duration::from_millis(5)).await;
            }
        })
        .await
        .unwrap();
        assert_eq!(events, about(wallet, offline));
    }
    for i in [joiner_rx, kicked_rx, banned_rx, stranger_rx] {
        assert!(heard(i).is_empty(), "{i}");
    }
    rig.finish();
}

/// Upstream `startCommunityVoiceChat` (`src/logic/community-voice/community-voice.ts:156-230`
/// at `b6b2cef`): STARTED carries the positions and worlds of the
/// community's places and its thumbnail, and the `community_voice_chat_started`
/// notification the thumbnail as `thumbnailUrl`.
#[tokio::test]
async fn a_started_room_carries_the_places_and_the_thumbnail() {
    use crate::rpc::proto::v2::CommunityVoiceChatStatus;
    use crate::rpc::pubsub::SocialEvent;
    let _voice = VOICE_ROWS.lock().await;
    // Every voice poll sweeps all tracked rooms of its database; fixtures that
    // poll the shared scratch database would clear this START's row between
    // its write and its generation check, so it publishes nothing.
    let Some(database) = crate::rpc::tests::fanout::OwnedDatabase::new(
        "fanout",
        crate::db::scratch::Schema::Full,
        8,
    )
    .await
    else {
        return;
    };
    let rig = Rig::with_pool(
        Settings::for_tests(),
        State::explore_for_tests,
        database.pool.clone(),
    )
    .await;
    let (owner, member) = (wallet(), wallet());
    let c = rig.community(&owner, false).await;
    rig.member(c, &member, "member").await;
    let world = rig.world_place(c, &owner).await;
    sqlx::query("INSERT INTO social.community_thumbnails (community_id, thumbnail_hash) VALUES ($1, 'bafythumb')")
        .bind(c)
        .execute(&rig.pool)
        .await
        .unwrap();
    let voice = Arc::new(crate::rpc::voice::fake::FakeVoiceRooms::new(
        rig.pool.clone(),
    ));
    let ctx = crate::rpc::context::Context::new(
        Default::default(),
        crate::rpc::db::Db::new(rig.pool.clone()),
        crate::rpc::profiles::Profiles::new(None),
        voice,
    );
    ctx.set_communities(rig.state.clone());
    {
        use crate::comms::protocol::pulse::decentraland::kernel::comms::v3::{
            IslandData, IslandStatusMessage,
        };
        use prost::Message as _;
        let frame = IslandStatusMessage {
            data: vec![IslandData {
                peers: vec![member.address.to_string()],
                ..Default::default()
            }],
        }
        .encode_to_vec();
        ctx.topology().receive(&frame).unwrap();
    }
    let mut to_member = ctx.pubsub().subscribe(member.address.as_str());
    crate::rpc::service::ops::start_community_voice_chat(
        &ctx,
        owner.address.as_str(),
        &c.to_string(),
        true,
    )
    .await
    .unwrap();
    let image = format!("http://cdn.test/social/communities/{c}/raw-thumbnail.png");
    let started = match &*to_member.try_recv().unwrap() {
        SocialEvent::CommunityVoice(u) => u.clone(),
        _ => panic!("not a voice update"),
    };
    assert_eq!(
        started.status,
        CommunityVoiceChatStatus::CommunityVoiceChatStarted as i32
    );
    assert_eq!(started.worlds, vec![world.clone()]);
    assert!(started.positions.is_empty());
    assert_eq!(started.community_image.as_deref(), Some(image.as_str()));
    assert!(started.is_member);
    let thumbnail: Vec<String> = sqlx::query_scalar(
        "SELECT metadata->>'thumbnailUrl' FROM social.notifications \
         WHERE type = 'community_voice_chat_started' AND metadata->>'communityId' = $1",
    )
    .bind(c.to_string())
    .fetch_all(&rig.pool)
    .await
    .unwrap();
    assert_eq!(thumbnail, vec![image]);
    rig.clear_world_place(&world).await;
    rig.finish();
    database.drop().await;
}
