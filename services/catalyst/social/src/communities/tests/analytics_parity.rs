//! The seven community hook locations use the existing analytics adapter.
//! Seventeen normal/no-op scenarios replay the pinned source mutators. The
//! captured late BAN failure is covered by the separate write-boundary work.
use super::*;

#[tokio::test]
async fn community_analytics_payloads_and_no_ops_match_executed_source() {
    let source:Value=serde_json::from_str(include_str!(concat!(env!("DCL_ONE_ROOT"),"/services/catalyst/tests/fixtures/conformance/social/source-contracts/community-analytics.json"))).unwrap();
    let Some(rig) = Rig::new(Settings::for_tests()).await else {
        return;
    };
    let (sender, mut received) = tokio::sync::mpsc::unbounded_channel();
    let provider = Router::new().route(
        "/events",
        axum::routing::post(
            move |headers: axum::http::HeaderMap, body: axum::body::Bytes| {
                let sender = sender.clone();
                async move {
                    sender
                        .send((headers, serde_json::from_slice::<Value>(&body).unwrap()))
                        .unwrap();
                    // A rejected analytics request must not change the domain result.
                    StatusCode::SERVICE_UNAVAILABLE
                }
            },
        ),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = format!("http://{}/events", listener.local_addr().unwrap());
    let server = tokio::spawn(async move { axum::serve(listener, provider).await.unwrap() });
    rig.state.set_analytics(
        crate::analytics::Analytics::new(
            &crate::http::Outbound::for_tests(),
            crate::analytics::Settings {
                context: Some("community-fixture".into()),
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
    let mut expected = Vec::new();
    let mut scenarios = 0;
    for case in source["cases"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|case| case.get("error").is_none())
    {
        scenarios += 1;
        let name = case["name"].as_str().unwrap();
        let private =
            name.starts_with("privacy-") || name.ends_with("-request") && !name.starts_with("join");
        let id = rig.community(&owner, private).await;
        let route_id = id.to_string().to_uppercase();
        let mut request_id = Uuid::new_v4();
        let mut other_request_id = Uuid::new_v4();
        if name == "join" {
            request_id = rig.request(id, &member, "invite").await;
        }
        if name == "join-existing" || name == "kick" || name == "leave" || name == "ban" {
            rig.member(id, &member, "member").await;
        }
        if name == "opposite-request" {
            request_id = rig.request(id, &member, "invite").await;
        }
        if name == "accept-request"
            || name == "duplicate-request"
            || name == "reject-request"
            || name == "reject-uppercase-request"
            || name == "cancel-request"
        {
            request_id = rig.request(id, &member, "request_to_join").await;
        }
        let mut banned_request = None;
        if name == "privacy-accept-all" {
            request_id = rig.request(id, &member, "request_to_join").await;
            let historical = wallet();
            other_request_id = rig.request(id, &historical, "request_to_join").await;
            sqlx::query("UPDATE social.community_requests SET status='accepted' WHERE id=$1")
                .bind(other_request_id)
                .execute(&rig.pool)
                .await
                .unwrap();
            let banned = wallet();
            let request = rig.request(id, &banned, "request_to_join").await;
            sqlx::query("INSERT INTO social.community_bans(community_id,banned_address,banned_by,active) VALUES($1,$2,$3,true)").bind(id).bind(&banned.address).bind(&owner.address).execute(&rig.pool).await.unwrap();
            banned_request = Some(request);
        }
        for event in case["analytics"].as_array().unwrap() {
            let properties: Value = serde_json::from_str(
                &event["properties"]
                    .to_string()
                    .replace(source["id"].as_str().unwrap(), &route_id)
                    .replace(
                        &source["id"].as_str().unwrap().to_lowercase(),
                        &id.to_string(),
                    )
                    .replace(
                        &source["requestId"].as_str().unwrap().to_uppercase(),
                        &request_id.to_string().to_uppercase(),
                    )
                    .replace(
                        source["requestId"].as_str().unwrap(),
                        &request_id.to_string(),
                    )
                    .replace(
                        source["otherRequestId"].as_str().unwrap(),
                        &other_request_id.to_string(),
                    )
                    .replace(source["owner"].as_str().unwrap(), owner.address.as_str())
                    .replace(source["member"].as_str().unwrap(), member.address.as_str()),
            )
            .unwrap();
            let mut body = properties;
            body["env"] = json!("fixture");
            // Going public accepts pending requests only; the source also
            // accepts and reports a request already decided (a deliberate fix).
            if name == "privacy-accept-all"
                && let Some(ids) = body["requests_ids"].as_array_mut()
            {
                ids.retain(|value| value != &json!(other_request_id.to_string()));
            }
            expected
                .push(json!({"event":event["event"],"body":body,"context":"community-fixture"}));
        }
        let (call, wallet, expected_status) = match name {
            "join" | "join-no-request" | "join-existing" => (
                Case::new("post", format!("/v1/communities/{route_id}/members")),
                &member,
                StatusCode::NO_CONTENT,
            ),
            "kick" | "kick-nonmember" => (
                Case::new(
                    "delete",
                    format!("/v1/communities/{route_id}/members/{}", member.address),
                ),
                &owner,
                StatusCode::NO_CONTENT,
            ),
            "leave" | "leave-nonmember" => (
                Case::new(
                    "delete",
                    format!("/v1/communities/{route_id}/members/{}", member.address),
                ),
                &member,
                StatusCode::NO_CONTENT,
            ),
            "ban" | "ban-nonmember" => (
                Case::new(
                    "post",
                    format!("/v1/communities/{route_id}/members/{}/bans", member.address),
                ),
                &owner,
                StatusCode::NO_CONTENT,
            ),
            "privacy-accept-all" | "privacy-no-requests" => (
                Case::new("put", format!("/v1/communities/{route_id}")).body(
                    b"privacy=public".to_vec(),
                    "application/x-www-form-urlencoded",
                ),
                &owner,
                StatusCode::OK,
            ),
            "opposite-request" | "duplicate-request" => (
                Case::new("post", format!("/v1/communities/{route_id}/requests")).json(
                    &json!({"type":"request_to_join","targetedAddress":member.address.as_str()}),
                ),
                &member,
                StatusCode::OK,
            ),
            "accept-request" => (
                Case::new(
                    "patch",
                    format!("/v1/communities/{route_id}/requests/{request_id}"),
                )
                .json(&json!({"intention":"accepted"})),
                &owner,
                StatusCode::NO_CONTENT,
            ),
            "reject-uppercase-request" => (
                Case::new(
                    "patch",
                    format!(
                        "/v1/communities/{route_id}/requests/{}",
                        request_id.to_string().to_uppercase()
                    ),
                )
                .json(&json!({"intention":"rejected"})),
                &owner,
                StatusCode::NO_CONTENT,
            ),
            "reject-request" => (
                Case::new(
                    "patch",
                    format!("/v1/communities/{route_id}/requests/{request_id}"),
                )
                .json(&json!({"intention":"rejected"})),
                &owner,
                StatusCode::NO_CONTENT,
            ),
            "cancel-request" => (
                Case::new(
                    "patch",
                    format!("/v1/communities/{route_id}/requests/{request_id}"),
                )
                .json(&json!({"intention":"cancelled"})),
                &member,
                StatusCode::NO_CONTENT,
            ),
            other => panic!("unhandled executed source scenario {other}"),
        };
        let (status, body) = hit(&rig.app, call.signed(wallet).await).await;
        assert_eq!(status, expected_status, "{name}: {body}");
        if let Some(banned_request) = banned_request {
            let mut ids: Vec<Uuid> = sqlx::query_scalar(
                "SELECT id FROM social.community_requests WHERE community_id=$1",
            )
            .bind(id)
            .fetch_all(&rig.pool)
            .await
            .unwrap();
            ids.sort();
            let mut kept = vec![banned_request, other_request_id];
            kept.sort();
            assert_eq!(
                ids, kept,
                "banned and already decided join requests remain after accept-all"
            );
            let members: i64 = sqlx::query_scalar(
                "SELECT count(*) FROM social.community_members WHERE community_id=$1",
            )
            .bind(id)
            .fetch_one(&rig.pool)
            .await
            .unwrap();
            assert_eq!(members, 2, "owner and pending requester");
        }
    }
    assert_eq!(scenarios, 17);
    assert_eq!(expected.len(), 10);
    // Independent fire-and-forget fetches have no completion-order contract.
    // Match complete payloads, preserving the request-id array order within one.
    while !expected.is_empty() {
        let (headers, payload) =
            tokio::time::timeout(std::time::Duration::from_secs(5), received.recv())
                .await
                .unwrap()
                .unwrap();
        assert_eq!(headers["x-token"], "fixture-token");
        assert_eq!(headers["content-type"], "application/json");
        let index = expected
            .iter()
            .position(|value| value == &payload)
            .unwrap_or_else(|| {
                panic!("unexpected analytics payload {payload}; remaining {expected:?}")
            });
        expected.remove(index);
    }
    assert!(
        tokio::time::timeout(std::time::Duration::from_millis(300), received.recv())
            .await
            .is_err(),
        "no duplicate/no-op analytics"
    );
    server.abort();
    rig.finish();
}
