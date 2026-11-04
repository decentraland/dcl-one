//! Differential replay of responses produced by the real, pinned upstream
//! handlers. Regenerate with tooling/social-service-source-cases.cjs.

use super::*;
use serde_json::Value;

#[tokio::test]
async fn rpc_wire_answers_match_executed_upstream_source_cases() {
    let Some(pool) = scratch_pool().await else {
        return;
    };
    let fixture: Value = serde_json::from_str(include_str!(concat!(
        env!("DCL_ONE_ROOT"),
        "/services/catalyst/tests/fixtures/conformance/social/source-cases.json"
    )))
    .unwrap();
    let me = fixture["caller"].as_str().unwrap();
    let other = fixture["target"].as_str().unwrap();
    let db = Db::new(pool.clone());
    for case in fixture["cases"].as_array().unwrap() {
        cleanup_pair(&db, me, other).await;
        let ctx = test_ctx(db.clone(), Arc::new(FakeVoiceRooms::new(pool.clone())));
        let pctx = procedure_ctx(&ctx, me, 5501);
        let request = &case["request"];
        let id = case["id"].as_str().unwrap();
        let user = |value: &Value| {
            value["address"].as_str().map(|address| User {
                address: address.to_string(),
            })
        };
        let pagination = request.get("pagination").map(|p| Pagination {
            limit: p["limit"].as_i64().unwrap_or_default() as i32,
            offset: p["offset"].as_i64().unwrap_or_default() as i32,
        });
        if id.ends_with("missing-profile") {
            if id.starts_with("get-blocked") {
                db.block_user(me, other).await.unwrap();
            } else {
                let pending = id.contains("friendship-requests");
                let (actor, target) = if id.starts_with("get-pending") {
                    (other, me)
                } else {
                    (me, other)
                };
                db.apply_friendship_action(
                    actor,
                    target,
                    if pending { "request" } else { "accept" },
                    !pending,
                    None,
                    None,
                )
                .await
                .unwrap();
            }
        }
        if id == "get-friendship-status.synthesized-block-action" {
            db.block_user(me, other).await.unwrap();
        }
        let encoded = match case["method"].as_str().unwrap() {
            "blockUser" => SocialServiceImpl
                .block_user(
                    BlockUserPayload {
                        user: user(&request["user"]),
                    },
                    pctx,
                )
                .await
                .unwrap()
                .encode_to_vec(),
            "unblockUser" => SocialServiceImpl
                .unblock_user(
                    UnblockUserPayload {
                        user: user(&request["user"]),
                    },
                    pctx,
                )
                .await
                .unwrap()
                .encode_to_vec(),
            "upsertFriendship" => {
                let action = request.get("action").map(|a| {
                    assert_eq!(a["$case"], "request");
                    upsert_friendship_payload::Action::Request(
                        upsert_friendship_payload::RequestPayload {
                            user: user(&a["request"]["user"]),
                            message: a["request"]["message"].as_str().map(str::to_string),
                        },
                    )
                });
                let result = SocialServiceImpl
                    .upsert_friendship(UpsertFriendshipPayload { action }, pctx)
                    .await
                    .unwrap();
                if id.ends_with("missing-profiles-after-commit") {
                    let last = db.last_friendship_action(me, other).await.unwrap().unwrap();
                    assert_eq!(
                        last.action, "request",
                        "the response error must not roll back the action"
                    );
                }
                result.encode_to_vec()
            }
            "getFriendshipStatus" => SocialServiceImpl
                .get_friendship_status(
                    GetFriendshipStatusPayload {
                        user: user(&request["user"]),
                    },
                    pctx,
                )
                .await
                .unwrap()
                .encode_to_vec(),
            "getFriends" => SocialServiceImpl
                .get_friends(GetFriendsPayload { pagination }, pctx)
                .await
                .unwrap()
                .encode_to_vec(),
            "getMutualFriends" => {
                // The fixture's injected domain read returns a row count of one
                // with no profiles. Give both callers the same absent-profile friend.
                let shared = "0x3333333333333333333333333333333333333333";
                db.apply_friendship_action(me, shared, "accept", true, None, None)
                    .await
                    .unwrap();
                db.apply_friendship_action(other, shared, "accept", true, None, None)
                    .await
                    .unwrap();
                let result = SocialServiceImpl
                    .get_mutual_friends(
                        GetMutualFriendsPayload {
                            user: user(&request["user"]),
                            pagination,
                        },
                        pctx,
                    )
                    .await
                    .unwrap()
                    .encode_to_vec();
                cleanup_pair(&db, me, shared).await;
                cleanup_pair(&db, other, shared).await;
                result
            }
            "getPendingFriendshipRequests" => SocialServiceImpl
                .get_pending_friendship_requests(GetFriendshipRequestsPayload { pagination }, pctx)
                .await
                .unwrap()
                .encode_to_vec(),
            "getSentFriendshipRequests" => SocialServiceImpl
                .get_sent_friendship_requests(GetFriendshipRequestsPayload { pagination }, pctx)
                .await
                .unwrap()
                .encode_to_vec(),
            "getBlockedUsers" => SocialServiceImpl
                .get_blocked_users(GetBlockedUsersPayload { pagination }, pctx)
                .await
                .unwrap()
                .encode_to_vec(),
            "getSocialSettings" => SocialServiceImpl
                .get_social_settings(pctx)
                .await
                .unwrap()
                .encode_to_vec(),
            "getPrivateMessagesSettings" => SocialServiceImpl
                .get_private_messages_settings(
                    GetPrivateMessagesSettingsPayload {
                        user: request["user"]
                            .as_array()
                            .unwrap()
                            .iter()
                            .map(|u| user(u).unwrap())
                            .collect(),
                    },
                    pctx,
                )
                .await
                .unwrap()
                .encode_to_vec(),
            "upsertSocialSettings" => SocialServiceImpl
                .upsert_social_settings(
                    UpsertSocialSettingsPayload {
                        private_messages_privacy: request["privateMessagesPrivacy"]
                            .as_i64()
                            .map(|v| v as i32),
                        blocked_users_messages_visibility:
                            request["blockedUsersMessagesVisibility"]
                                .as_i64()
                                .map(|v| v as i32),
                        show_situation_reactions: request["showSituationReactions"]
                            .as_i64()
                            .map(|v| v as i32),
                    },
                    pctx,
                )
                .await
                .unwrap()
                .encode_to_vec(),
            unknown => panic!("unhandled source case method {unknown}"),
        };
        assert_eq!(
            hex::encode(encoded),
            case["expected_hex"].as_str().unwrap(),
            "{id}: upstream response {}",
            case["response"]
        );
    }
    cleanup_pair(&db, me, other).await;
}
