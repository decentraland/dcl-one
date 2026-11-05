//! Native community producers persist recipient inboxes and email delivery intent.
use super::*;
use crate::notifications::community::{Audience, CommunityEvent, publish};

fn post_fixture() -> Value {
    serde_json::from_str(include_str!(concat!(env!("DCL_ONE_ROOT"), "/services/catalyst/tests/fixtures/conformance/social/source-contracts/community-post-profile.json"))).unwrap()
}

async fn inbox(rig: &Rig, id: CommunityId, count: usize) -> Vec<(String, String, Value)> {
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        loop {
            let rows: Vec<(String,String,Value)> = sqlx::query_as("SELECT address,type,metadata FROM social.notifications WHERE metadata->>'communityId'=$1 ORDER BY address,type").bind(id.to_string()).fetch_all(&rig.pool).await.unwrap();
            if rows.len() >= count { return rows; }
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        }
    }).await.expect("native notification rows")
}

async fn assert_deliveries(rig: &Rig, id: CommunityId, count: i64) {
    let rows: Vec<(String,String,i32,Option<String>)> = sqlx::query_as("SELECT d.channel,d.state,d.attempts,d.last_error FROM social.notification_deliveries d JOIN social.notifications n ON n.id=d.notification_id WHERE n.metadata->>'communityId'=$1").bind(id.to_string()).fetch_all(&rig.pool).await.unwrap();
    assert_eq!(rows.len() as i64, count);
    assert!(rows.iter().all(|row| row
        == &(
            "email".into(),
            "skipped".into(),
            0,
            Some("no confirmed email".into())
        )));
}

#[tokio::test]
async fn create_post_resolves_strict_author_before_native_notification() {
    let Some(rig) = Rig::new(Settings::for_tests()).await else {
        return;
    };
    let source = post_fixture();
    for case in source["cases"].as_array().unwrap() {
        let author = wallet();
        let id = rig.community(&author, false).await;
        let member = wallet();
        rig.member(id, &member, "member").await;
        sqlx::query("UPDATE social.communities SET name='Fixture community' WHERE id=$1")
            .bind(id)
            .execute(&rig.pool)
            .await
            .unwrap();
        let remap = |value: &Value| -> Value {
            serde_json::from_str(
                &value
                    .to_string()
                    .replace(source["author"].as_str().unwrap(), author.address.as_str())
                    .replace("00000000-0000-4000-8000-000000000001", &id.to_string()),
            )
            .unwrap()
        };
        if let Some(document) = case["documents"].as_array().unwrap().first() {
            sqlx::query("UPDATE content.deployments SET entity_metadata=$2 WHERE entity_id=(SELECT entity_id FROM content.active_pointers WHERE pointer=$1)").bind(&author.address).bind(json!({"v":remap(document)})).execute(&rig.pool).await.unwrap();
        } else {
            sqlx::query("DELETE FROM content.active_pointers WHERE pointer=$1")
                .bind(&author.address)
                .execute(&rig.pool)
                .await
                .unwrap();
        }
        rig.state.profiles.clear().await;
        let (status, body) = hit(
            &rig.app,
            Case::new("post", format!("/v1/communities/{id}/posts"))
                .signed(&author)
                .await
                .json(&json!({"content":"Content"})),
        )
        .await;
        assert_eq!(
            status.as_u16(),
            case["response"]["status"].as_u64().unwrap() as u16,
            "{} {body}",
            case["name"]
        );
        let mut expected = remap(&case["response"]["body"]);
        if status == StatusCode::CREATED {
            expected["data"]["id"] = body["data"]["id"].clone();
            expected["data"]["createdAt"] = body["data"]["createdAt"].clone();
            let rows = inbox(&rig, id, 1).await;
            assert_eq!(
                rows,
                vec![(
                    member.address.to_string(),
                    "community_post_added".into(),
                    json!({
                        "communityId":id.as_uuid(), "communityName":"Fixture community",
                        "thumbnailUrl":rig.state.thumbnail_url(id), "postId":body["data"]["id"],
                        "authorAddress":author.address.as_str()
                    })
                )]
            );
            assert_deliveries(&rig, id, 1).await;
        } else {
            let notifications: i64 = sqlx::query_scalar(
                "SELECT count(*) FROM social.notifications WHERE metadata->>'communityId'=$1",
            )
            .bind(id.to_string())
            .fetch_one(&rig.pool)
            .await
            .unwrap();
            assert_eq!(notifications, 0);
        }
        assert_eq!(body, expected, "{}", case["name"]);
        let posts: i64 =
            sqlx::query_scalar("SELECT count(*) FROM social.community_posts WHERE community_id=$1")
                .bind(id)
                .fetch_one(&rig.pool)
                .await
                .unwrap();
        assert_eq!(posts, 1, "source commits before profile lookup");
    }
    rig.finish();
}

#[tokio::test]
async fn real_mutations_write_native_inbox_and_delivery_rows() {
    let _voice = VOICE_ROWS.lock().await;
    let global = wallet();
    let Some(rig) = Rig::new(admin_settings(Some(&global))).await else {
        return;
    };
    let mut exercised = std::collections::BTreeSet::new();
    for operation in [
        "delete-by-global-moderator",
        "rename",
        "kick",
        "leave",
        "transfer",
        "ban",
        "post",
        "request",
        "invite",
        "accept-request",
        "start-voice",
    ] {
        let (owner, target) = (wallet(), wallet());
        let id = rig
            .community(
                &owner,
                operation == "request" || operation == "accept-request",
            )
            .await;
        rig.profile(&target, "Member", true).await;
        sqlx::query("UPDATE social.communities SET name='Fixture community' WHERE id=$1")
            .bind(id)
            .execute(&rig.pool)
            .await
            .unwrap();
        if !matches!(operation, "request" | "invite" | "accept-request") {
            rig.member(id, &target, "member").await;
        }
        let mut post_id = String::new();
        let response = match operation {
            "delete-by-global-moderator" => {
                hit(
                    &rig.app,
                    Case::new("delete", format!("/v1/communities/{id}"))
                        .signed(&global)
                        .await,
                )
                .await
            }
            "rename" => {
                let (body, ct) = multipart(&[("name", b"After")]);
                hit(
                    &rig.app,
                    Case::new("put", format!("/v1/communities/{id}"))
                        .signed(&owner)
                        .await
                        .body(body, &ct),
                )
                .await
            }
            "kick" | "leave" => {
                hit(
                    &rig.app,
                    Case::new(
                        "delete",
                        format!("/v1/communities/{id}/members/{}", target.address),
                    )
                    .signed(if operation == "leave" {
                        &target
                    } else {
                        &owner
                    })
                    .await,
                )
                .await
            }
            "transfer" => {
                hit(
                    &rig.app,
                    Case::new(
                        "patch",
                        format!("/v1/communities/{id}/members/{}", target.address),
                    )
                    .signed(&owner)
                    .await
                    .json(&json!({"role":"owner"})),
                )
                .await
            }
            "ban" => {
                hit(
                    &rig.app,
                    Case::new(
                        "post",
                        format!("/v1/communities/{id}/members/{}/bans", target.address),
                    )
                    .signed(&owner)
                    .await,
                )
                .await
            }
            "post" => {
                let answer = hit(
                    &rig.app,
                    Case::new("post", format!("/v1/communities/{id}/posts"))
                        .signed(&owner)
                        .await
                        .json(&json!({"content":"Content"})),
                )
                .await;
                post_id = answer.1["data"]["id"].as_str().unwrap().to_owned();
                answer
            }
            "request" | "invite" => {
                let answer=hit(&rig.app,Case::new("post",format!("/v1/communities/{id}/requests")).signed(if operation=="request" {&target} else {&owner}).await.json(&json!({"targetedAddress":target.address.as_str(),"type":if operation=="request" {"request_to_join"} else {"invite"}}))).await;
                let _request_id = sqlx::query_scalar::<_, Uuid>(
                    "SELECT id FROM social.community_requests WHERE community_id=$1",
                )
                .bind(id)
                .fetch_one(&rig.pool)
                .await
                .unwrap()
                .to_string();
                answer
            }
            "accept-request" => {
                let request_id = rig
                    .request(id, &target, "request_to_join")
                    .await
                    .to_string();
                hit(
                    &rig.app,
                    Case::new(
                        "patch",
                        format!("/v1/communities/{id}/requests/{request_id}"),
                    )
                    .signed(&owner)
                    .await
                    .json(&json!({"intention":"accepted"})),
                )
                .await
            }
            "start-voice" => {
                let ctx = crate::rpc::context::Context::new(
                    Default::default(),
                    crate::rpc::db::Db::new(rig.pool.clone()),
                    crate::rpc::profiles::Profiles::new(None),
                    Arc::new(crate::rpc::voice::fake::FakeVoiceRooms::new(
                        rig.pool.clone(),
                    )),
                );
                ctx.set_communities(rig.state.clone());
                use crate::comms::protocol::pulse::decentraland::kernel::comms::v3::{
                    IslandData, IslandStatusMessage,
                };
                use prost::Message as _;
                ctx.topology()
                    .receive(
                        &IslandStatusMessage {
                            data: vec![IslandData {
                                peers: vec![target.address.to_string()],
                                ..Default::default()
                            }],
                        }
                        .encode_to_vec(),
                    )
                    .unwrap();
                crate::rpc::service::ops::start_community_voice_chat(
                    &ctx,
                    owner.address.as_str(),
                    &id.to_string(),
                    false,
                )
                .await
                .unwrap();
                (StatusCode::OK, Value::Null)
            }
            _ => unreachable!(),
        };
        assert!(response.0.is_success(), "{operation}: {response:?}");
        let mut metadata = json!({"communityId":id.as_uuid(), "communityName":"Fixture community", "thumbnailUrl":rig.state.thumbnail_url(id)});
        let kinds: Vec<(&str, &Address)> = match operation {
            "delete-by-global-moderator" => {
                metadata["thumbnailUrl"] = json!("N/A");
                vec![
                    ("community_deleted", &target.address),
                    ("community_deleted_content_violation", &owner.address),
                ]
            }
            "rename" => {
                metadata = json!({"communityId":id.as_uuid(), "oldName":"Fixture community", "newName":"After", "thumbnailUrl":"N/A"});
                vec![
                    ("community_renamed", &owner.address),
                    ("community_renamed", &target.address),
                ]
            }
            "kick" => vec![("community_member_removed", &target.address)],
            "leave" => {
                let member: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM social.community_members WHERE community_id=$1 AND member_address=$2)").bind(id).bind(&target.address).fetch_one(&rig.pool).await.unwrap();
                assert!(
                    !member,
                    "leave commits membership removal without an inbox notice"
                );
                vec![]
            }
            "transfer" => {
                metadata["oldOwnerAddress"] = json!(owner.address.as_str());
                vec![("community_ownership_transferred", &target.address)]
            }
            "ban" => vec![("community_member_banned", &target.address)],
            "post" => {
                metadata["postId"] = json!(post_id);
                metadata["authorAddress"] = json!(owner.address.as_str());
                vec![("community_post_added", &target.address)]
            }
            "request" => {
                metadata["memberAddress"] = json!(target.address.as_str());
                metadata["memberName"] = json!("Member");
                vec![("community_request_to_join_received", &owner.address)]
            }
            "invite" => vec![("community_invite_received", &target.address)],
            "accept-request" => {
                metadata["memberAddress"] = json!(target.address.as_str());
                vec![("community_request_to_join_accepted", &target.address)]
            }
            "start-voice" => {
                metadata["thumbnailUrl"] = json!("");
                vec![("community_voice_chat_started", &target.address)]
            }
            _ => unreachable!(),
        };
        let mut expected = kinds
            .iter()
            .map(|(kind, address)| (address.to_string(), kind.to_string(), metadata.clone()))
            .collect::<Vec<_>>();
        expected.sort_by(|a, b| (&a.0, &a.1).cmp(&(&b.0, &b.1)));
        assert_eq!(
            inbox(&rig, id, expected.len()).await,
            expected,
            "{operation}"
        );
        assert_deliveries(&rig, id, expected.len() as i64).await;
        exercised.extend(kinds.iter().map(|(kind, _)| kind.to_string()));
    }
    assert_eq!(exercised.len(), 11);
    rig.finish();
}

#[tokio::test]
async fn native_audiences_filter_opt_outs_and_replays_preserve_delivery_intent() {
    let Some(rig) = Rig::new(Settings::for_tests()).await else {
        return;
    };
    let owner = wallet();
    let id = rig.community(&owner, false).await;
    let address = |n: usize| format!("0x{}{:08x}", id.as_uuid().simple(), n);
    sqlx::query("INSERT INTO social.community_members(community_id,member_address,role,joined_at) SELECT $1,'0x'||replace($1::text,'-','')||lpad(to_hex(n),8,'0'),CASE WHEN n<4 THEN 'moderator' ELSE 'member' END,timestamp '2026-01-01' FROM generate_series(1101,1,-1) n").bind(id).execute(&rig.pool).await.unwrap();
    sqlx::query("INSERT INTO social.subscription_opt_outs(address,scope,scope_id) VALUES($1,'community',$2),($3,'community','invalid-id'),($3,'community',$4)").bind(address(2)).bind(id.to_string().to_uppercase()).bind(address(3)).bind(CommunityId::new().to_string()).execute(&rig.pool).await.unwrap();
    sqlx::query("INSERT INTO social.subscriptions(address,email,details) VALUES($1,'recipient@example.org','{}'),($2,'ignored@example.org','{\"ignore_all_email\":true}')").bind(address(4)).bind(address(5)).execute(&rig.pool).await.unwrap();
    let online = [
        owner.address.to_string(),
        address(2),
        address(4),
        address(5),
        address(2000),
    ]
    .into_iter()
    .collect();
    let opted_out = Address::parse(&address(2)).unwrap();
    let direct = Address::parse(&address(2000)).unwrap();
    for (kind, audience, mut expected) in [
        (
            "community_renamed",
            Audience::Members,
            std::iter::once(owner.address.to_string())
                .chain((1..=1101).filter(|n| *n != 2).map(&address))
                .collect::<Vec<_>>(),
        ),
        (
            "community_deleted",
            Audience::MembersButOwner,
            (1..=1101).filter(|n| *n != 2).map(&address).collect(),
        ),
        (
            "community_request_to_join_received",
            Audience::Managers,
            vec![owner.address.to_string(), address(1), address(3)],
        ),
        (
            "community_post_added",
            Audience::MembersExcept(&owner.address),
            (1..=1101).filter(|n| *n != 2).map(&address).collect(),
        ),
        (
            "community_voice_chat_started",
            Audience::OnlineMembersExcept {
                online: &online,
                except: owner.address.as_str(),
            },
            vec![address(4), address(5)],
        ),
        (
            "community_member_removed",
            Audience::One(&direct),
            vec![direct.to_string()],
        ),
        ("community_member_banned", Audience::One(&opted_out), vec![]),
    ] {
        let event = CommunityEvent {
            kind,
            key: format!("{id}-{kind}"),
            community_id: id,
            metadata: json!({"communityId":id.as_uuid(),"communityName":"Audience fixture"}),
        };
        assert_eq!(
            publish(&rig.pool, &event, audience).await.unwrap(),
            expected.len() as u64,
            "{kind}"
        );
        expected.sort();
        let rows: Vec<(Uuid,String,Value)> = sqlx::query_as("SELECT id,address,metadata FROM social.notifications WHERE type=$1 AND metadata->>'communityId'=$2 ORDER BY address").bind(kind).bind(id.to_string()).fetch_all(&rig.pool).await.unwrap();
        assert_eq!(
            rows.iter().map(|row| row.1.clone()).collect::<Vec<_>>(),
            expected,
            "{kind}"
        );
        assert!(rows.iter().all(|row| row.2 == event.metadata));
        let replay_audience = match kind {
            "community_renamed" => Audience::Members,
            "community_deleted" => Audience::MembersButOwner,
            "community_request_to_join_received" => Audience::Managers,
            "community_post_added" => Audience::MembersExcept(&owner.address),
            "community_voice_chat_started" => Audience::OnlineMembersExcept {
                online: &online,
                except: owner.address.as_str(),
            },
            "community_member_removed" => Audience::One(&direct),
            "community_member_banned" => Audience::One(&opted_out),
            _ => unreachable!(),
        };
        assert_eq!(
            publish(&rig.pool, &event, replay_audience).await.unwrap(),
            0,
            "{kind}: replay"
        );
        let deliveries: Vec<(Uuid,String,String,i32,Option<String>)> = sqlx::query_as("SELECT n.id,n.address,d.state,d.attempts,d.last_error FROM social.notifications n JOIN social.notification_deliveries d ON d.notification_id=n.id WHERE n.type=$1 AND n.metadata->>'communityId'=$2 ORDER BY n.address").bind(kind).bind(id.to_string()).fetch_all(&rig.pool).await.unwrap();
        assert_eq!(deliveries.len(), rows.len());
        for ((id, recipient, state, attempts, error), row) in deliveries.iter().zip(&rows) {
            assert_eq!(*id, row.0);
            assert_eq!(*attempts, 0);
            let expected = if *recipient == address(4) {
                ("pending", None)
            } else if *recipient == address(5) {
                ("skipped", Some("ignore_all_email"))
            } else {
                ("skipped", Some("no confirmed email"))
            };
            assert_eq!((state.as_str(), error.as_deref()), expected);
        }
    }
    rig.finish();
}
