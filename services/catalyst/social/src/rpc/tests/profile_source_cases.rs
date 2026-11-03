//! Registry and RPC answers executed from the pinned source. Documents with a
//! different public userId are historical accepted content, never new writes.

use super::*;
use serde_json::Value;

#[tokio::test]
async fn historical_and_positive_profiles_match_executed_source_rpc_answers() {
    let Some(pool) = scratch_pool().await else {
        return;
    };
    let fixture: Value = serde_json::from_str(include_str!(concat!(
        env!("DCL_ONE_ROOT"),
        "/services/catalyst/tests/fixtures/conformance/social/profile-cases.json"
    )))
    .unwrap();
    let me = fixture["caller"].as_str().unwrap();
    let owner = fixture["owner"].as_str().unwrap();
    let alias = fixture["alias"].as_str().unwrap();
    let second = fixture["second"].as_str().unwrap();
    let db = Db::new(pool.clone());
    for case in fixture["cases"].as_array().unwrap() {
        let mut deployed = Vec::new();
        for (pointer, document) in case["documents"].as_object().unwrap() {
            let entity = format!("bafkreihistorical{}", Uuid::new_v4().simple());
            sqlx::query("INSERT INTO content.deployments(deployer_address,version,entity_type,entity_id,entity_metadata,entity_timestamp,entity_pointers,local_timestamp,auth_chain) VALUES($1,'v3','profile',$2,$3,to_timestamp(1451606400),ARRAY[$1],now(),'[]'::json)")
                .bind(pointer).bind(&entity).bind(serde_json::json!({"v":document})).execute(&pool).await.unwrap();
            sqlx::query("INSERT INTO content.active_pointers(pointer,entity_id,entity_type) VALUES($1,$2,'profile')")
                .bind(pointer).bind(&entity).execute(&pool).await.unwrap();
            deployed.push((pointer.clone(), entity));
        }
        // One context per source scenario preserves the registry's single/list
        // cache interaction across steps; database reads use matching fixtures.
        let ctx = test_ctx(db.clone(), Arc::new(FakeVoiceRooms::new(pool.clone())));
        for step in case["steps"].as_array().unwrap() {
            for (a, b) in [(me, owner), (me, second), (second, owner)] {
                cleanup_pair(&db, a, b).await;
            }
            let operation = step["operation"].as_str().unwrap();
            let targets = step["targets"].as_array().unwrap();
            if operation.starts_with("getFriends") || operation == "getMutualFriends" {
                for (index, target) in targets.iter().enumerate() {
                    let target = target.as_str().unwrap();
                    let (id, _, _) = db
                        .apply_friendship_action(me, target, "accept", true, None, None)
                        .await
                        .unwrap();
                    sqlx::query(
                        "UPDATE social.friendships SET created_at=to_timestamp($2) WHERE id=$1",
                    )
                    .bind(id)
                    .bind(-(index as f64))
                    .execute(&pool)
                    .await
                    .unwrap();
                }
                if operation == "getMutualFriends" {
                    db.apply_friendship_action(second, owner, "accept", true, None, None)
                        .await
                        .unwrap();
                }
            } else if operation.contains("FriendshipRequests") {
                let (from, to) = if operation == "getPendingFriendshipRequests" {
                    (owner, me)
                } else {
                    (me, owner)
                };
                let (id, _, action) = db
                    .apply_friendship_action(from, to, "request", false, None, None)
                    .await
                    .unwrap();
                sqlx::query("UPDATE social.friendship_actions SET id='00000000-0000-0000-0000-000000000001',timestamp=to_timestamp(0) WHERE id=$1")
                    .bind(action).execute(&pool).await.unwrap();
                sqlx::query(
                    "UPDATE social.friendships SET last_action_at=to_timestamp(0) WHERE id=$1",
                )
                .bind(id)
                .execute(&pool)
                .await
                .unwrap();
            } else {
                sqlx::query("INSERT INTO social.blocks(blocker_address,blocked_address,blocked_at) VALUES($1,$2,to_timestamp(0))")
                    .bind(me).bind(owner).execute(&pool).await.unwrap();
            }
            let pctx = procedure_ctx(&ctx, me, 5701);
            let pagination = Some(Pagination {
                limit: 100,
                offset: 0,
            });
            let encoded = match operation {
                "getFriends" | "getFriendsMixed" => SocialServiceImpl
                    .get_friends(GetFriendsPayload { pagination }, pctx)
                    .await
                    .unwrap()
                    .encode_to_vec(),
                "getMutualFriends" => SocialServiceImpl
                    .get_mutual_friends(
                        GetMutualFriendsPayload {
                            user: Some(User {
                                address: second.into(),
                            }),
                            pagination,
                        },
                        pctx,
                    )
                    .await
                    .unwrap()
                    .encode_to_vec(),
                "blockUser" => SocialServiceImpl
                    .block_user(
                        BlockUserPayload {
                            user: Some(User {
                                address: owner.into(),
                            }),
                        },
                        pctx,
                    )
                    .await
                    .unwrap()
                    .encode_to_vec(),
                "unblockUser" => SocialServiceImpl
                    .unblock_user(
                        UnblockUserPayload {
                            user: Some(User {
                                address: owner.into(),
                            }),
                        },
                        pctx,
                    )
                    .await
                    .unwrap()
                    .encode_to_vec(),
                "getBlockedUsers" => SocialServiceImpl
                    .get_blocked_users(GetBlockedUsersPayload { pagination }, pctx)
                    .await
                    .unwrap()
                    .encode_to_vec(),
                "getPendingFriendshipRequests" => SocialServiceImpl
                    .get_pending_friendship_requests(
                        GetFriendshipRequestsPayload { pagination },
                        pctx,
                    )
                    .await
                    .unwrap()
                    .encode_to_vec(),
                "getSentFriendshipRequests" => SocialServiceImpl
                    .get_sent_friendship_requests(GetFriendshipRequestsPayload { pagination }, pctx)
                    .await
                    .unwrap()
                    .encode_to_vec(),
                unknown => panic!("unknown operation {unknown}"),
            };
            assert_eq!(
                hex::encode(encoded),
                step["expected_hex"].as_str().unwrap(),
                "{} / {operation}: source {}",
                case["id"],
                step["response"]
            );
            if operation.starts_with("getFriends") {
                let requested = targets
                    .iter()
                    .map(|target| target.as_str().unwrap().to_string())
                    .collect::<Vec<_>>();
                let associated = ctx
                    .profiles()
                    .get_profiles_with_actors_result(&requested)
                    .await
                    .unwrap();
                assert!(
                    associated
                        .iter()
                        .all(|(actor, _)| requested.contains(actor))
                );
                if case["id"] == "historical-alias-lists-and-single-cache" {
                    assert!(
                        associated
                            .iter()
                            .all(|(actor, profile)| actor == owner && profile.address == alias),
                        "guard identities stay requested wallets, including cached/fetched aliases"
                    );
                }
            }
            if operation == "blockUser" || operation == "unblockUser" {
                assert_eq!(
                    db.is_blocked(me, owner).await.unwrap(),
                    operation == "blockUser",
                    "the wallet mutation must keep the requested identity"
                );
                assert!(
                    !db.is_blocked(me, alias).await.unwrap(),
                    "public profile aliases never authorize or retarget a mutation"
                );
            }
        }
        if case["id"] == "historical-alias-lists-and-single-cache" {
            cleanup_pair(&db, me, owner).await;
            let mut raw = ctx.pubsub().subscribe(me);
            let _ = SocialServiceImpl
                .upsert_friendship(
                    UpsertFriendshipPayload {
                        action: Some(upsert_friendship_payload::Action::Request(
                            upsert_friendship_payload::RequestPayload {
                                user: Some(User { address: me.into() }),
                                message: None,
                            },
                        )),
                    },
                    procedure_ctx(&ctx, owner, 5702),
                )
                .await
                .unwrap();
            let event = tokio::time::timeout(Duration::from_secs(1), raw.recv())
                .await
                .unwrap()
                .unwrap();
            let SocialEvent::Friendship(update) = &*event else {
                panic!("expected raw friendship update")
            };
            let Some(friendship_update::Update::Request(request)) = &update.update else {
                panic!("expected raw request")
            };
            assert_eq!(
                request.friend.as_ref().unwrap().address,
                owner,
                "raw updates retain the actor; RPC delivery enriches the public alias"
            );
        }
        for (pointer, entity) in deployed {
            drop_profile(&pool, &pointer, &entity).await;
        }
    }
    for (a, b) in [(me, owner), (me, second), (second, owner)] {
        cleanup_pair(&db, a, b).await;
    }
}
