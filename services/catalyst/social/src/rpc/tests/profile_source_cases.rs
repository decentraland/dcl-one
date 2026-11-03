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
        // The source fixture creates a fresh injected Redis map per scenario.
        // Reset only these fixture-owned keys, preserving cache within steps.
        sqlx::query("DELETE FROM social.registry_profile_cache WHERE profile_key=ANY($1::text[])")
            .bind(vec![me, owner, alias, second])
            .execute(&pool)
            .await
            .unwrap();
        let fail_cache = if case["id"] == "cache-write-failure" {
            let name = format!("registry_refuse_{}", Uuid::new_v4().simple());
            sqlx::query(sqlx::AssertSqlSafe(format!("CREATE FUNCTION social.{name}() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN IF NEW.profile_key='{owner}' THEN RAISE EXCEPTION 'fixture cache write failure'; END IF; RETURN NEW; END $$"))).execute(&pool).await.unwrap();
            sqlx::query(sqlx::AssertSqlSafe(format!("CREATE TRIGGER {name} BEFORE INSERT OR UPDATE ON social.registry_profile_cache FOR EACH ROW EXECUTE FUNCTION social.{name}()"))).execute(&pool).await.unwrap();
            Some(name)
        } else {
            None
        };
        let mut deployed = Vec::new();
        for (pointer, document) in case["documents"].as_object().unwrap() {
            let entity = format!("bafkreihistorical{}", Uuid::new_v4().simple());
            sqlx::query("INSERT INTO content.deployments(deployer_address,version,entity_type,entity_id,entity_metadata,entity_timestamp,entity_pointers,local_timestamp,auth_chain) VALUES($1,'v3','profile',$2,$3,to_timestamp(1451606400),ARRAY[$1],now(),'[]'::json)")
                .bind(pointer).bind(&entity).bind(serde_json::json!({"v":document})).execute(&pool).await.unwrap();
            sqlx::query("INSERT INTO content.active_pointers(pointer,entity_id,entity_type) VALUES($1,$2,'profile')")
                .bind(pointer).bind(&entity).execute(&pool).await.unwrap();
            deployed.push((pointer.clone(), entity));
        }
        // Independent contexts alternate A/B/A against shared cache facts,
        // as two actual source registries share injected Redis in the capture.
        let ctx_a = test_ctx(db.clone(), Arc::new(FakeVoiceRooms::new(pool.clone())));
        let ctx_b = test_ctx(db.clone(), Arc::new(FakeVoiceRooms::new(pool.clone())));
        for step in case["steps"].as_array().unwrap() {
            let ctx = if step["reader"] == "B" {
                &ctx_b
            } else {
                &ctx_a
            };
            for (a, b) in [(me, owner), (me, second), (second, owner)] {
                cleanup_pair(&db, a, b).await;
            }
            let operation = step["operation"].as_str().unwrap();
            if matches!(operation, "getOwnerName" | "getOwnerNames") {
                let community = crate::communities::profiles::ProfilesCache::new(
                    pool.clone(),
                    Default::default(),
                );
                if operation == "getOwnerName" {
                    let result = community.get_owner_name(owner).await;
                    if let Some(error) = step["expected_error"].as_str() {
                        assert_eq!(result.unwrap_err().to_string(), error)
                    } else {
                        assert_eq!(
                            serde_json::to_value(result.unwrap()).unwrap(),
                            step["expected_json"]
                        )
                    }
                } else {
                    assert_eq!(
                        serde_json::to_value(
                            community
                                .get_owner_names(&[owner.to_string()])
                                .await
                                .unwrap()
                        )
                        .unwrap(),
                        step["expected_json"]
                    )
                }
                assert_source_cache_keys(&pool, &fixture, step).await;
                continue;
            }
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
                "getFriends" | "getFriendsMixed" | "getFriendsReverse" => SocialServiceImpl
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
            assert_source_cache_keys(&pool, &fixture, step).await;
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
            let ctx = &ctx_a;
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
        if let Some(name) = fail_cache {
            let count: i64 = sqlx::query_scalar(
                "SELECT count(*) FROM social.registry_profile_cache WHERE profile_key=$1",
            )
            .bind(owner)
            .fetch_one(&pool)
            .await
            .unwrap();
            assert_eq!(
                count, 0,
                "the best-effort cache failure must not block returning fresh source profiles"
            );
            sqlx::query(sqlx::AssertSqlSafe(format!(
                "DROP TRIGGER {name} ON social.registry_profile_cache"
            )))
            .execute(&pool)
            .await
            .unwrap();
            sqlx::query(sqlx::AssertSqlSafe(format!(
                "DROP FUNCTION social.{name}()"
            )))
            .execute(&pool)
            .await
            .unwrap();
        }
        for (pointer, entity) in deployed {
            drop_profile(&pool, &pointer, &entity).await;
        }
    }
    for (a, b) in [(me, owner), (me, second), (second, owner)] {
        cleanup_pair(&db, a, b).await;
    }
}

async fn assert_source_cache_keys(pool: &PgPool, fixture: &Value, step: &Value) {
    let keys = ["caller", "owner", "alias", "second"]
        .map(|field| fixture[field].as_str().unwrap().to_string())
        .to_vec();
    let actual:Vec<String>=sqlx::query_scalar("SELECT profile_key FROM social.registry_profile_cache WHERE profile_key=ANY($1::text[]) AND expires_at>timezone('UTC',clock_timestamp()) ORDER BY profile_key").bind(keys).fetch_all(pool).await.unwrap();
    let expected = step["cache_keys"]
        .as_array()
        .unwrap()
        .iter()
        .map(|key| {
            key.as_str()
                .unwrap()
                .strip_prefix("catalyst:minimal:profile:")
                .unwrap()
                .to_string()
        })
        .collect::<Vec<_>>();
    assert_eq!(
        actual, expected,
        "{} shared cache facts must match actual source Redis",
        step["operation"]
    );
}
