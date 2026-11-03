//! Executed source creators, broadcaster and SNS adapter, replayed through the
//! official SDK against a local stub. No external topic or paid service is used.
use super::*;
use crate::notifications::community::{Audience, CommunityEvent};
use axum::response::IntoResponse;
use std::sync::atomic::{AtomicUsize, Ordering};

fn fixture(name: &str) -> Value {
    let raw = match name {
        "producers" => include_str!(concat!(
            env!("DCL_ONE_ROOT"),
            "/services/catalyst/tests/fixtures/conformance/social/source-contracts/community-producers.json"
        )),
        "broadcast" => include_str!(concat!(
            env!("DCL_ONE_ROOT"),
            "/services/catalyst/tests/fixtures/conformance/social/source-contracts/community-broadcast.json"
        )),
        "post" => include_str!(concat!(
            env!("DCL_ONE_ROOT"),
            "/services/catalyst/tests/fixtures/conformance/social/source-contracts/community-post-profile.json"
        )),
        _ => panic!("unknown fixture"),
    };
    serde_json::from_str(raw).unwrap()
}

fn address(n: usize) -> String {
    format!("0x{n:040x}")
}

struct Sns {
    publisher: Arc<crate::legacy_events::LegacyEvents>,
    requests: Arc<parking_lot::Mutex<Vec<Value>>>,
    mode: Arc<AtomicUsize>,
    server: tokio::task::JoinHandle<()>,
}
impl Sns {
    async fn new() -> Self {
        let requests = Arc::new(parking_lot::Mutex::new(Vec::new()));
        let mode = Arc::new(AtomicUsize::new(0));
        let (captured, response_mode) = (requests.clone(), mode.clone());
        let app = Router::new().fallback(move |headers: axum::http::HeaderMap, body: axum::body::Bytes| {
            let captured = captured.clone(); let response_mode = response_mode.clone();
            async move {
                assert!(headers["authorization"].to_str().unwrap().starts_with("AWS4-HMAC-SHA256 "));
                let fields = url::form_urlencoded::parse(&body).into_owned().collect::<std::collections::HashMap<_,_>>();
                let batch = fields["Action"] == "PublishBatch";
                let attributes = |prefix: &str| -> Value {
                    let mut attrs = serde_json::Map::new();
                    for i in 1..=2 {
                        let base = format!("{prefix}MessageAttributes.entry.{i}.");
                        if let Some(name) = fields.get(&format!("{base}Name")) {
                            attrs.insert(name.clone(), json!({"DataType":fields[&format!("{base}Value.DataType")],"StringValue":fields[&format!("{base}Value.StringValue")]}));
                        }
                    }
                    Value::Object(attrs)
                };
                let mut ids = Vec::new();
                let input = if batch {
                    let mut entries = Vec::new();
                    for i in 1..=10 {
                        let prefix = format!("PublishBatchRequestEntries.member.{i}.");
                        let Some(id) = fields.get(&format!("{prefix}Id")) else { break };
                        ids.push(id.clone());
                        entries.push(json!({"Id":id,"Message":serde_json::from_str::<Value>(&fields[&format!("{prefix}Message")]).unwrap(),"MessageAttributes":attributes(&prefix)}));
                    }
                    assert!(!entries.is_empty());
                    json!({"TopicArn":fields["TopicArn"],"PublishBatchRequestEntries":entries})
                } else {
                    assert_eq!(fields["Action"], "Publish");
                    json!({"TopicArn":fields["TopicArn"],"Message":serde_json::from_str::<Value>(&fields["Message"]).unwrap(),"MessageAttributes":attributes("")})
                };
                captured.lock().push(json!({"command":if batch {"PublishBatchCommand"} else {"PublishCommand"},"input":input}));
                if response_mode.load(Ordering::SeqCst) == 1 {
                    return (StatusCode::BAD_REQUEST, [("content-type","text/xml")], "<ErrorResponse><Error><Type>Sender</Type><Code>InvalidParameter</Code><Message>fixture SNS failure</Message></Error><RequestId>fixture</RequestId></ErrorResponse>".to_owned()).into_response();
                }
                let xml = if batch {
                    let (successful, failed) = if response_mode.load(Ordering::SeqCst) == 2 {
                        ("<member><Id>msg_0</Id><MessageId>accepted</MessageId></member><member><Id>msg_2</Id></member><member><Id>msg_3</Id><MessageId></MessageId></member>".to_owned(),
                         ["msg_1","msg_01","msg_2junk","msg_-1","msg_99","msg_"].iter().map(|id|format!("<member><Id>{id}</Id><Code>Fixture</Code><Message>rejected</Message><SenderFault>true</SenderFault></member>")).collect::<String>())
                    } else {
                        (ids.iter().map(|id|format!("<member><Id>{id}</Id><MessageId>fixture-{id}</MessageId></member>")).collect(), String::new())
                    };
                    format!("<PublishBatchResponse xmlns=\"http://sns.amazonaws.com/doc/2010-03-31/\"><PublishBatchResult><Successful>{successful}</Successful><Failed>{failed}</Failed></PublishBatchResult><ResponseMetadata><RequestId>fixture</RequestId></ResponseMetadata></PublishBatchResponse>")
                } else {
                    "<PublishResponse xmlns=\"http://sns.amazonaws.com/doc/2010-03-31/\"><PublishResult><MessageId>fixture</MessageId></PublishResult><ResponseMetadata><RequestId>fixture</RequestId></ResponseMetadata></PublishResponse>".to_owned()
                };
                (StatusCode::OK,[("content-type","text/xml")],xml).into_response()
            }
        });
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let endpoint = format!("http://{}", listener.local_addr().unwrap());
        let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        Self {
            publisher: crate::legacy_events::LegacyEvents::test_publisher(&endpoint),
            requests,
            mode,
            server,
        }
    }
    async fn wait(&self, count: usize) {
        tokio::time::timeout(std::time::Duration::from_secs(5), async {
            while self.requests.lock().len() < count {
                tokio::time::sleep(std::time::Duration::from_millis(5)).await;
            }
        })
        .await
        .unwrap();
    }
    fn events(&self) -> Vec<Value> {
        self.requests
            .lock()
            .iter()
            .flat_map(|request| {
                let input = &request["input"];
                if let Some(entries) = input["PublishBatchRequestEntries"].as_array() {
                    entries
                        .iter()
                        .map(|entry| entry["Message"].clone())
                        .collect::<Vec<_>>()
                } else {
                    vec![input["Message"].clone()]
                }
            })
            .collect()
    }
}
impl Drop for Sns {
    fn drop(&mut self) {
        self.server.abort();
    }
}

fn normalize_requests(requests: &Value) -> Vec<Value> {
    let mut requests = requests.as_array().unwrap().clone();
    for request in &mut requests {
        request["input"]["TopicArn"] = json!("fixture-topic");
        let input = &mut request["input"];
        if let Some(entries) = input["PublishBatchRequestEntries"].as_array_mut() {
            for entry in entries {
                if let Some(message) = entry["Message"].as_str() {
                    entry["Message"] = serde_json::from_str(message).unwrap();
                }
            }
        } else if let Some(message) = input["Message"].as_str() {
            input["Message"] = serde_json::from_str(message).unwrap();
        }
    }
    requests.sort_by_cached_key(Value::to_string);
    requests
}

#[test]
fn all_twelve_raw_community_event_shapes_match_source_creators() {
    let source = fixture("producers");
    let one = Address::parse(&address(4)).unwrap();
    let owner = Address::parse(&address(1)).unwrap();
    for capture in source["events"].as_array().unwrap() {
        let raw = &capture["event"];
        let kind = [
            "community_deleted",
            "community_deleted_content_violation",
            "community_renamed",
            "community_member_removed",
            "community_member_left",
            "community_ownership_transferred",
            "community_member_banned",
            "community_post_added",
            "community_request_to_join_received",
            "community_invite_received",
            "community_request_to_join_accepted",
            "community_voice_chat_started",
        ]
        .into_iter()
        .find(|kind| kind.replace('_', "-") == raw["subType"].as_str().unwrap())
        .unwrap();
        let meta = &raw["metadata"];
        let mut metadata = meta.clone();
        if let Some(name) = meta.get("name") {
            metadata["communityName"] = name.clone();
        }
        let event = CommunityEvent {
            kind,
            key: raw["key"].as_str().unwrap().to_owned(),
            community_id: CommunityId::from_uuid(
                Uuid::parse_str("00000000-0000-4000-8000-000000000001").unwrap(),
            ),
            metadata,
        };
        let audience = Audience::One(if kind == "community_deleted_content_violation" {
            &owner
        } else {
            &one
        });
        assert_eq!(
            crate::notifications::community::legacy_event(
                &event,
                &audience,
                raw["timestamp"].as_i64().unwrap()
            ),
            *raw,
            "{}",
            capture["operation"]
        );
    }
}

#[tokio::test]
async fn broadcaster_recipient_pages_and_official_sns_commands_match_source() {
    let Some(rig) = Rig::new(Settings::for_tests()).await else {
        return;
    };
    let sns = Sns::new().await;
    let source = fixture("broadcast");
    for case in source["cases"].as_array().unwrap() {
        let name = case["name"].as_str().unwrap();
        let id = CommunityId::new();
        sqlx::query("INSERT INTO social.communities(id,name,description,owner_address) VALUES($1,'Fixture community','fixture',$2)").bind(id).bind(address(1)).execute(&rig.pool).await.unwrap();
        let count = if name == "RENAMED_1101" {
            1101
        } else if name == "EMPTY_MANAGERS" {
            0
        } else {
            205
        };
        sqlx::query("INSERT INTO social.community_members(community_id,member_address,role,joined_at) SELECT $1,'0x'||lpad(to_hex(n),40,'0'),CASE WHEN n=1 THEN 'owner' WHEN n<4 THEN 'moderator' ELSE 'member' END,timestamp '2026-01-01'+n*interval '1 second' FROM generate_series(1,$2) n").bind(id).bind(count).execute(&rig.pool).await.unwrap();
        // An external event must retain these opted-out recipients. Only the
        // later inbox consumer is allowed to filter subscription preferences.
        sqlx::query("INSERT INTO social.subscription_opt_outs(address,scope,scope_id) VALUES($1,'community',$2)").bind(address(2)).bind(id.to_string()).execute(&rig.pool).await.unwrap();
        let replace = |value: &Value| -> Value {
            serde_json::from_str(
                &value
                    .to_string()
                    .replace("00000000-0000-4000-8000-000000000001", &id.to_string()),
            )
            .unwrap()
        };
        let event = replace(&case["event"]);
        sns.requests.lock().clear();
        sns.mode.store(
            usize::from(name == "SNS_FAILURE" || name == "BATCH_FAILURE"),
            Ordering::SeqCst,
        );
        let online = (event["subType"] == "community-voice-chat-started").then(|| {
            if name == "VOICE_OFFLINE" {
                Vec::new()
            } else {
                (1..=count as usize)
                    .filter(|n| n % 2 == 1)
                    .map(address)
                    .collect()
            }
        });
        let excluded = if online.is_some() {
            vec![address(1)]
        } else {
            Vec::new()
        };
        if name == "DATABASE_FAILURE" {
            let closed = sqlx::postgres::PgPoolOptions::new()
                .connect_lazy("postgres://fixture@127.0.0.1:1/fixture")
                .unwrap();
            closed.close().await;
            crate::notifications::community::broadcast_legacy(
                &closed,
                &sns.publisher,
                event,
                online,
                excluded,
            )
            .await;
        } else {
            crate::notifications::community::broadcast_legacy(
                &rig.pool,
                &sns.publisher,
                event,
                online,
                excluded,
            )
            .await;
        }
        assert_eq!(
            normalize_requests(&json!(*sns.requests.lock())),
            normalize_requests(&replace(&case["requests"])),
            "{name}"
        );
    }
    rig.finish();
}

#[tokio::test]
async fn sns_batch_partial_ids_and_failed_chunks_match_source_adapter() {
    let sns = Sns::new().await;
    for case in fixture("broadcast")["adapterCases"].as_array().unwrap() {
        sns.mode.store(
            if case["name"] == "partial" { 2 } else { 1 },
            Ordering::SeqCst,
        );
        sns.requests.lock().clear();
        let result = sns
            .publisher
            .publish_batch(case["events"].as_array().unwrap())
            .await
            .unwrap();
        assert_eq!(
            json!({"successfulMessageIds":result.successful_message_ids,"failedEvents":result.failed_events}),
            case["result"],
            "{}",
            case["name"]
        );
        assert_eq!(
            normalize_requests(&json!(*sns.requests.lock())),
            normalize_requests(&case["requests"])
        );
    }
}

#[tokio::test]
async fn create_post_resolves_strict_author_after_insert_and_before_broadcast() {
    let Some(rig) = Rig::new(Settings::for_tests()).await else {
        return;
    };
    let sns = Sns::new().await;
    rig.state.referral.set_publisher(sns.publisher.clone());
    let source = fixture("post");
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
        sns.requests.lock().clear();
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
            sns.wait(1).await;
            let events = sns.events();
            assert_eq!(events.len(), 1);
            let mut event = remap(&case["events"][0]);
            event["key"] = json!(format!("{}-batch-1", body["data"]["id"].as_str().unwrap()));
            event["timestamp"] = events[0]["timestamp"].clone();
            event["metadata"]["postId"] = body["data"]["id"].clone();
            event["metadata"]["thumbnailUrl"] = json!(rig.state.thumbnail_url(id));
            event["metadata"]["addressesToNotify"] = json!([member.address.as_str()]);
            assert_eq!(events[0], event);
        } else {
            tokio::task::yield_now().await;
            assert!(sns.requests.lock().is_empty());
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
async fn real_mutations_publish_all_twelve_source_community_subtypes() {
    let _voice = VOICE_ROWS.lock().await;
    let global = wallet();
    let Some(rig) = Rig::new(admin_settings(Some(&global))).await else {
        return;
    };
    let sns = Sns::new().await;
    rig.state.referral.set_publisher(sns.publisher.clone());
    let source = fixture("producers");
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
        sns.requests.lock().clear();
        // HTTP ban and awaited voice START keep their successful mutation
        // when the external SNS dependency rejects the captured event.
        sns.mode.store(
            usize::from(matches!(operation, "ban" | "start-voice")),
            Ordering::SeqCst,
        );
        let mut request_id = String::new();
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
                request_id = sqlx::query_scalar::<_, Uuid>(
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
                request_id = rig
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
        let captures = source["events"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|capture| capture["operation"] == operation)
            .collect::<Vec<_>>();
        sns.wait(captures.len()).await;
        let events = sns.events();
        assert_eq!(events.len(), captures.len(), "{operation}");
        for (actual, capture) in events.iter().zip(captures) {
            let source_event = &capture["event"];
            let mut expected: Value = serde_json::from_str(
                &source_event
                    .to_string()
                    .replace("00000000-0000-4000-8000-000000000001", &id.to_string())
                    .replace(
                        "0x0000000000000000000000000000000000000001",
                        owner.address.as_str(),
                    )
                    .replace(
                        "0x0000000000000000000000000000000000000004",
                        target.address.as_str(),
                    ),
            )
            .unwrap();
            let mut actual = actual.clone();
            let stamp = actual["timestamp"].as_i64().unwrap();
            assert!(stamp > source["timestamp"].as_i64().unwrap() - 86_400_000);
            expected["timestamp"] = actual["timestamp"].clone();
            let key_clock = if operation == "start-voice" {
                let actual_key = actual["key"].as_str().unwrap();
                let created = actual_key
                    .strip_prefix(&format!("{id}-"))
                    .unwrap()
                    .strip_suffix("-batch-1")
                    .unwrap()
                    .parse::<i64>()
                    .unwrap();
                assert!(created <= stamp);
                created
            } else {
                stamp
            };
            let mut key = expected["key"]
                .as_str()
                .unwrap()
                .replace("1793721600000", &key_clock.to_string());
            if !request_id.is_empty() {
                key = key.replace("00000000-0000-4000-8000-000000000003", &request_id);
            }
            if !post_id.is_empty() {
                key = key.replace("00000000-0000-4000-8000-000000000002", &post_id);
                expected["metadata"]["postId"] = json!(post_id);
            }
            if matches!(operation, "rename" | "post" | "start-voice")
                || actual["subType"] == "community-deleted"
            {
                key.push_str("-batch-1");
            }
            expected["key"] = json!(key);
            if matches!(
                operation,
                "kick" | "transfer" | "ban" | "post" | "request" | "invite" | "accept-request"
            ) {
                expected["metadata"]["thumbnailUrl"] = json!(rig.state.thumbnail_url(id));
            }
            actual["metadata"]
                .as_object_mut()
                .unwrap()
                .remove("memberAddresses");
            if operation == "post" {
                actual["metadata"]["addressesToNotify"] = json!([]);
            } else {
                actual["metadata"]
                    .as_object_mut()
                    .unwrap()
                    .remove("addressesToNotify");
            }
            assert_eq!(actual, expected, "{operation}");
            exercised.insert(actual["subType"].as_str().unwrap().to_owned());
        }
    }
    assert_eq!(exercised.len(), 12);
    rig.finish();
}
