//! Actual source handlers, components and SQL adapter captured with PG triggers.
use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};
struct VoiceCalls(Arc<AtomicUsize>);
impl super::super::VoiceEviction for VoiceCalls {
    fn evict<'a>(
        &'a self,
        _: CommunityId,
        _: &'a str,
    ) -> futures::future::BoxFuture<'a, Result<(), String>> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Box::pin(async { Ok(()) })
    }
}
#[tokio::test]
async fn ban_and_privacy_write_failures_preserve_source_committed_stages() {
    let source:Value=serde_json::from_str(include_str!(concat!(env!("DCL_ONE_ROOT"),"/services/catalyst/tests/fixtures/conformance/social/source-contracts/community-write-failures.json"))).unwrap();
    let Some(rig) = Rig::new(Settings::for_tests()).await else {
        return;
    };
    rig.state.pass_markers.insert(None, None);
    let voice = Arc::new(AtomicUsize::new(0));
    rig.state
        .set_voice_eviction(Arc::new(VoiceCalls(voice.clone())));
    let updates = crate::rpc::pubsub::PubSub::new();
    rig.state.set_updates(updates.clone());
    let (send, mut analytics) = tokio::sync::mpsc::unbounded_channel();
    let provider = Router::new().route(
        "/events",
        axum::routing::post(move |body: axum::body::Bytes| {
            let send = send.clone();
            async move {
                send.send(serde_json::from_slice::<Value>(&body).unwrap())
                    .unwrap();
                StatusCode::OK
            }
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = format!("http://{}/events", listener.local_addr().unwrap());
    let server = tokio::spawn(async move { axum::serve(listener, provider).await.unwrap() });
    rig.state.set_analytics(
        crate::analytics::Analytics::new(
            &crate::http::Outbound::for_tests(),
            crate::analytics::Settings {
                context: Some("write-fixture".into()),
                api_url: Some(endpoint),
                api_token: Some("fixture-token".into()),
                environment: Some("fixture".into()),
                request_timeout_ms: None,
            },
        )
        .unwrap()
        .unwrap(),
    );
    let (owner, member) = (wallet(), wallet());
    for case in source["cases"].as_array().unwrap() {
        let name = case["name"].as_str().unwrap();
        let id = rig.community(&owner, true).await;
        sqlx::query("UPDATE social.communities SET name='Fixture community',description='Description' WHERE id=$1").bind(id).execute(&rig.pool).await.unwrap();
        if name.starts_with("ban-") {
            rig.member(id, &member, "member").await
        }
        let post = rig.post(id, &owner).await;
        sqlx::query("INSERT INTO social.community_post_likes(post_id,user_address) VALUES($1,$2)")
            .bind(post)
            .bind(&member.address)
            .execute(&rig.pool)
            .await
            .unwrap();
        let request = rig.request(id, &member, "request_to_join").await;
        let old_place = source["oldPlace"].as_str().unwrap();
        let new_place = source["newPlace"].as_str().unwrap();
        for place in std::iter::once(old_place)
            .chain((name == "update-place-insert-failure").then_some(new_place))
        {
            // An already-held replacement passes real ownership preflight.
            sqlx::query("INSERT INTO social.community_places(place_id,community_id,added_by) VALUES($1,$2,$3)").bind(place).bind(id).bind(&owner.address).execute(&rig.pool).await.unwrap();
        }
        let fault = match name {
            "ban-kick-failure" => Some((
                "community_members",
                "DELETE",
                format!("OLD.community_id='{id}'::uuid"),
            )),
            "ban-unlike-failure" => Some((
                "community_post_likes",
                "DELETE",
                format!("OLD.post_id='{post}'::uuid"),
            )),
            "ban-insert-failure" => Some((
                "community_bans",
                "INSERT",
                format!("NEW.community_id='{id}'::uuid"),
            )),
            "ban-requests-failure" | "update-accept-delete-failure" => Some((
                "community_requests",
                "DELETE",
                format!("OLD.community_id='{id}'::uuid"),
            )),
            "update-place-insert-failure" => Some((
                "community_places",
                "INSERT",
                format!("NEW.community_id='{id}'::uuid"),
            )),
            "update-accept-insert-failure" => Some((
                "community_members",
                "INSERT",
                format!("NEW.community_id='{id}'::uuid"),
            )),
            _ => None,
        };
        let suffix = Uuid::new_v4().simple().to_string();
        let function = format!("social.reject_{suffix}");
        let trigger = format!("reject_{suffix}");
        // Identifiers are random UUID hex or fixed table/event variants; all
        // predicate values are UUIDs and messages are fixed fixture names.
        if let Some((table, event, predicate)) = &fault {
            sqlx::raw_sql(sqlx::AssertSqlSafe(format!("CREATE FUNCTION {function}() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION '%',TG_ARGV[0]; END $$; CREATE TRIGGER {trigger} BEFORE {event} ON social.{table} FOR EACH ROW WHEN ({predicate}) EXECUTE FUNCTION {function}('fixture {name}')"))).execute(&rig.pool).await.unwrap();
        }
        let mut owner_updates = updates.subscribe(owner.address.as_str());
        let mut member_updates = updates.subscribe(member.address.as_str());
        let old_voice = voice.load(Ordering::SeqCst);
        let path = format!("/social-api/v1/communities/{id}");
        let call = if name.starts_with("ban-") {
            Case::new("post", format!("{path}/members/{}/bans", member.address))
        } else {
            let fields = if name == "update-place-insert-failure" {
                format!("privacy=public&placeIds=%5B%22{new_place}%22%5D")
            } else {
                "privacy=public".to_owned()
            };
            Case::new("put", path).body(fields.into_bytes(), "application/x-www-form-urlencoded")
        };
        let (status, body) = hit(&rig.app, call.signed(&owner).await).await;
        assert_eq!(
            status.as_u16(),
            case["response"]["status"].as_u64().unwrap() as u16,
            "{name}: {body}"
        );
        if let Some(expected) = case["response"].get("body") {
            let expected = expected
                .to_string()
                .replace(source["id"].as_str().unwrap(), &id.to_string())
                .replace(source["owner"].as_str().unwrap(), owner.address.as_str());
            assert_eq!(
                body,
                serde_json::from_str::<Value>(&expected).unwrap(),
                "{name}"
            );
        }
        for event in case["events"].as_array().unwrap() {
            let mut props: Value = serde_json::from_str(
                &event["properties"]
                    .to_string()
                    .replace(source["id"].as_str().unwrap(), &id.to_string())
                    .replace(source["owner"].as_str().unwrap(), owner.address.as_str())
                    .replace(source["member"].as_str().unwrap(), member.address.as_str())
                    .replace("00000000-0000-4000-8000-000000000003", &request.to_string()),
            )
            .unwrap();
            props["env"] = json!("fixture");
            let actual = tokio::time::timeout(std::time::Duration::from_secs(5), analytics.recv())
                .await
                .unwrap()
                .unwrap();
            assert_eq!(
                actual,
                json!({"event":event["event"],"body":props,"context":"write-fixture"}),
                "{name}"
            );
        }
        assert!(
            tokio::time::timeout(std::time::Duration::from_millis(100), analytics.recv())
                .await
                .is_err(),
            "extra analytics for {name}"
        );
        let state:(bool,i64,i64,i64,i64,Option<Vec<String>>)=sqlx::query_as("SELECT (SELECT private FROM social.communities WHERE id=$1),(SELECT count(*) FROM social.community_members WHERE community_id=$1 AND member_address=$2),(SELECT count(*) FROM social.community_post_likes WHERE post_id=$3 AND user_address=$2),(SELECT count(*) FROM social.community_bans WHERE community_id=$1 AND active),(SELECT count(*) FROM social.community_requests WHERE community_id=$1),(SELECT array_agg(place_id ORDER BY place_id) FROM social.community_places WHERE community_id=$1)").bind(id).bind(&member.address).bind(post).fetch_one(&rig.pool).await.unwrap();
        assert_eq!(
            json!({"private":state.0,"members":state.1,"likes":state.2,"bans":state.3,"requests":state.4,"places":state.5}),
            case["state"],
            "{name}"
        );
        if case["response"]["status"] == 500 {
            assert_eq!(
                voice.load(Ordering::SeqCst),
                old_voice,
                "voice after failed {name}"
            );
            assert!(
                owner_updates.try_recv().is_err(),
                "owner pubsub after failed {name}"
            );
            assert!(
                member_updates.try_recv().is_err(),
                "member pubsub after failed {name}"
            );
            let inbox: i64 = sqlx::query_scalar(
                "SELECT count(*) FROM social.notifications WHERE metadata->>'communityId'=$1",
            )
            .bind(id.to_string())
            .fetch_one(&rig.pool)
            .await
            .unwrap();
            assert_eq!(inbox, 0, "inbox after failed {name}");
        } else if name == "ban-success" {
            assert_eq!(voice.load(Ordering::SeqCst), old_voice + 1);
            assert!(owner_updates.try_recv().is_ok());
            assert!(member_updates.try_recv().is_ok());
            let rows: Vec<(String,String,Value)> = sqlx::query_as("SELECT n.address,n.type,n.metadata FROM social.notifications n JOIN social.notification_deliveries d ON d.notification_id=n.id WHERE n.metadata->>'communityId'=$1").bind(id.to_string()).fetch_all(&rig.pool).await.unwrap();
            assert_eq!(
                rows,
                vec![(
                    member.address.to_string(),
                    "community_member_banned".into(),
                    json!({"communityId":id.as_uuid(), "communityName":"Fixture community", "thumbnailUrl":rig.state.thumbnail_url(id)})
                )]
            );
        }
        if let Some((table, _, _)) = fault {
            sqlx::raw_sql(sqlx::AssertSqlSafe(format!(
                "DROP TRIGGER {trigger} ON social.{table}; DROP FUNCTION {function}()"
            )))
            .execute(&rig.pool)
            .await
            .unwrap();
        }
    }
    server.abort();
    rig.finish();
}
