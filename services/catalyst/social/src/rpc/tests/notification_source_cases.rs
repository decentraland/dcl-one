//! Source event creator/retrier captures replayed through actual mutations and
//! the official SNS SDK against a local HTTP stub. No external topic is used.
use super::*;
use axum::response::IntoResponse;
use serde_json::Value;

#[tokio::test]
async fn friendship_sns_events_and_retries_match_executed_source() {
    let Some(pool) = scratch_pool().await else {
        return;
    };
    let fixture: Value = serde_json::from_str(include_str!(concat!(
        env!("DCL_ONE_ROOT"),
        "/services/catalyst/tests/fixtures/conformance/social/notification-cases.json"
    )))
    .unwrap();
    let sender = fixture["context"]["senderAddress"].as_str().unwrap();
    let receiver = fixture["context"]["receiverAddress"].as_str().unwrap();
    let db = Db::new(pool.clone());
    let mut deployed = Vec::new();
    for (address, key) in [(sender, "senderProfile"), (receiver, "receiverProfile")] {
        let entity = format!("bafkreisnsfixture{}", Uuid::new_v4().simple());
        sqlx::query("INSERT INTO content.deployments(deployer_address,version,entity_type,entity_id,entity_metadata,entity_timestamp,entity_pointers,local_timestamp,auth_chain) VALUES($1,'v3','profile',$2,$3,now(),ARRAY[$1],now(),'[]'::json)")
            .bind(address).bind(&entity).bind(serde_json::json!({"v":fixture["context"][key]})).execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO content.active_pointers(pointer,entity_id,entity_type) VALUES($1,$2,'profile')")
            .bind(address).bind(&entity).execute(&pool).await.unwrap();
        deployed.push((address, entity));
    }
    for case in fixture["cases"].as_array().unwrap() {
        cleanup_pair(&db, sender, receiver).await;
        let captured = Arc::new(parking_lot::Mutex::new(Vec::<Value>::new()));
        let requests = captured.clone();
        let failures = case["failures"].as_u64().unwrap() as usize;
        let app = axum::Router::new().fallback(move |headers:axum::http::HeaderMap,body:axum::body::Bytes| {
            let requests=requests.clone();
            async move {
                assert!(headers["authorization"].to_str().unwrap().starts_with("AWS4-HMAC-SHA256 "));
                let fields=url::form_urlencoded::parse(&body).into_owned().collect::<std::collections::HashMap<_,_>>();
                assert_eq!(fields["Action"],"Publish");
                let event:Value=serde_json::from_str(&fields["Message"]).unwrap();
                for (name,value) in [("type",event["type"].as_str().unwrap()),("subType",event["subType"].as_str().unwrap())] {
                    let index=(1..=2).find(|index|fields.get(&format!("MessageAttributes.entry.{index}.Name")).is_some_and(|actual|actual==name)).unwrap();
                    assert_eq!(fields[&format!("MessageAttributes.entry.{index}.Value.StringValue")],value);
                    assert_eq!(fields[&format!("MessageAttributes.entry.{index}.Value.DataType")],"String");
                }
                let count={let mut requests=requests.lock();requests.push(event);requests.len()};
                if count<=failures {
                    (axum::http::StatusCode::BAD_REQUEST,[("content-type","text/xml")],"<ErrorResponse><Error><Type>Sender</Type><Code>InvalidParameter</Code><Message>fixture rejection</Message></Error><RequestId>fixture</RequestId></ErrorResponse>").into_response()
                } else {
                    (axum::http::StatusCode::OK,[("content-type","text/xml")],"<PublishResponse xmlns=\"http://sns.amazonaws.com/doc/2010-03-31/\"><PublishResult><MessageId>fixture</MessageId></PublishResult><ResponseMetadata><RequestId>fixture</RequestId></ResponseMetadata></PublishResponse>").into_response()
                }
            }
        });
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let endpoint = format!("http://{}", listener.local_addr().unwrap());
        let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        let dir = std::env::temp_dir().join(format!("friendship-sns-{}", Uuid::new_v4()));
        let state = Arc::new(crate::communities::State::new(
            pool.clone(),
            Arc::new(crate::storage::Storage::from_config(&dir).await.unwrap()),
            crate::communities::Settings::for_tests(),
            None,
            &crate::http::Outbound::for_tests(),
            crate::communities::State::explore_for_tests(&pool),
        ));
        state
            .referral
            .set_publisher(crate::legacy_events::LegacyEvents::test_publisher(
                &endpoint,
            ));
        let ctx = test_ctx(db.clone(), Arc::new(FakeVoiceRooms::new(pool.clone())));
        ctx.set_communities(state);
        let action = if case["action"] == "accept" {
            db.apply_friendship_action(receiver, sender, "request", false, None, None)
                .await
                .unwrap();
            upsert_friendship_payload::Action::Accept(upsert_friendship_payload::AcceptPayload {
                user: Some(User {
                    address: receiver.into(),
                }),
            })
        } else {
            upsert_friendship_payload::Action::Request(upsert_friendship_payload::RequestPayload {
                user: Some(User {
                    address: receiver.into(),
                }),
                message: case["message"].as_str().map(str::to_string),
            })
        };
        let started = chrono::Utc::now().timestamp_millis();
        let response = SocialServiceImpl
            .upsert_friendship(
                UpsertFriendshipPayload {
                    action: Some(action),
                },
                procedure_ctx(&ctx, sender, 5801),
            )
            .await
            .unwrap();
        assert!(
            matches!(
                response.response,
                Some(upsert_friendship_response::Response::Accepted(_))
            ),
            "SNS failure cannot undo the committed mutation"
        );
        let friendship_id = db
            .last_friendship_action(sender, receiver)
            .await
            .unwrap()
            .unwrap()
            .friendship_id;
        let action_id = sqlx::query_scalar::<_, Uuid>(
            "SELECT id FROM social.friendship_actions WHERE friendship_id=$1 AND action=$2",
        )
        .bind(friendship_id)
        .bind(case["action"].as_str().unwrap())
        .fetch_one(&pool)
        .await
        .unwrap()
        .to_string();
        let expected = case["events"].as_array().unwrap();
        tokio::time::timeout(Duration::from_secs(5), async {
            while captured.lock().len() < expected.len() {
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        })
        .await
        .unwrap();
        let finished = chrono::Utc::now().timestamp_millis();
        let events = captured.lock().clone();
        assert_eq!(events.len(), expected.len());
        for (event, source) in events.iter().zip(expected) {
            let timestamp = event["timestamp"].as_i64().unwrap();
            assert!((started..=finished).contains(&timestamp));
            assert_eq!(event["key"], action_id);
            assert_eq!(event["metadata"]["requestId"], action_id);
            let mut normalized = event.clone();
            normalized["timestamp"] = source["timestamp"].clone();
            normalized["key"] = source["key"].clone();
            normalized["metadata"]["requestId"] = source["metadata"]["requestId"].clone();
            assert_eq!(&normalized, source, "{} source event", case["id"]);
        }
        for pair in events.windows(2) {
            assert!(
                pair[1]["timestamp"].as_i64().unwrap() - pair[0]["timestamp"].as_i64().unwrap()
                    >= 290,
                "source waits 300ms between publish attempts"
            );
        }
        server.abort();
        std::fs::remove_dir_all(dir).unwrap();
    }
    cleanup_pair(&db, sender, receiver).await;
    for (address, entity) in deployed {
        drop_profile(&pool, address, &entity).await;
    }
}
