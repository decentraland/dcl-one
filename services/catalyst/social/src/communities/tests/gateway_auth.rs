//! Gateway-only preauth precedes the legacy federation body discriminator.
//! The signed body-ignoring join/envelope ambiguity is explicitly outside
//! the matched source cases; no new client marker header is introduced.
use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};
use tokio::io::{AsyncReadExt, AsyncWriteExt, BufReader};

#[tokio::test]
async fn gateway_real_http_auth_precedes_sniff_and_client_json_uses_http_cap() {
    let source:Value=serde_json::from_str(include_str!(concat!(env!("DCL_ONE_ROOT"),"/services/catalyst/tests/fixtures/conformance/social/source-contracts/community-federation-boundary.json"))).unwrap();
    let actor = wallet();
    let mut settings = Settings::for_tests();
    settings.http_max_request_body_bytes = source["cap"].as_u64().unwrap() as usize;
    settings.global_moderators = vec![actor.address.clone()];
    let Some(rig) = Rig::new(settings).await else {
        return;
    };
    let id = rig.community(&actor, false).await;
    let (addr, server) = super::body_limit::serve(&rig).await;
    let mut compared = 0;
    for case in source["cases"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|case| case["name"] != "signed-join-json-header-only" && case["fault"] != true)
    {
        compared += 1;
        rig.state.pass_markers.insert(None, None);
        let source_path = case["path"]
            .as_str()
            .unwrap()
            .replace("00000000-0000-4000-8000-000000000001", &id.to_string());
        let path = format!("/social-api{source_path}");
        let method = case["method"].as_str().unwrap();
        let mut head = format!(
            "{method} {path} HTTP/1.1\r\nHost: social.test\r\nConnection: keep-alive\r\nOrigin: https://decentraland.org\r\n"
        );
        if let Some(ct) = case["contentType"].as_str() {
            head.push_str(&format!("Content-Type: {ct}\r\n"));
        }
        if case["signed"] == true {
            for (name, value) in signed_headers(&actor, &method.to_lowercase(), &path).await {
                head.push_str(&format!("{name}: {value}\r\n"));
            }
        }
        let length = case["length"].as_u64().unwrap_or(10) as usize;
        let chunked = case["chunked"] == true;
        head.push_str(&if chunked {
            "Transfer-Encoding: chunked\r\n\r\n".to_owned()
        } else {
            format!("Content-Length: {length}\r\n\r\n")
        });
        if case["sendBody"] == true {
            let mut data = "{\"editorsChoice\":true}".to_owned();
            data.push_str(&" ".repeat(length - data.len()));
            head.push_str(&if chunked {
                format!("{length:x}\r\n{data}\r\n0\r\n\r\n")
            } else {
                data
            });
        }
        let mut socket = BufReader::new(tokio::net::TcpStream::connect(addr).await.unwrap());
        socket.get_mut().write_all(head.as_bytes()).await.unwrap();
        let wait = case["responseTimeoutAfterMs"].as_u64().unwrap_or(5000);
        let response = tokio::time::timeout(
            std::time::Duration::from_millis(wait),
            super::body_limit::response(&mut socket),
        )
        .await;
        if case.get("responseTimeoutAfterMs").is_some() {
            assert!(
                response.is_err(),
                "source observed no response for {wait}ms: {case}"
            );
            continue;
        }
        let (status, headers, body) = response.unwrap();
        assert_eq!(
            status,
            case["status"].as_u64().unwrap() as u16,
            "{case}: {body}"
        );
        let expected = case["body"].as_str().unwrap();
        if let Ok(expected) = serde_json::from_str::<Value>(expected) {
            assert_eq!(serde_json::from_str::<Value>(&body).unwrap(), expected)
        } else {
            assert_eq!(body, expected)
        }
        assert_eq!(
            super::social_api_parity::application_headers(&headers),
            super::social_api_parity::source_application_headers(&case["headers"]),
            "{case}"
        );
        let mut extra = [0u8; 1];
        let eof = matches!(
            tokio::time::timeout(
                std::time::Duration::from_millis(200),
                socket.read(&mut extra)
            )
            .await,
            Ok(Ok(0))
        );
        assert_eq!(
            eof,
            case["closedWithinObservation"].as_bool().unwrap(),
            "EOF after response for {case}"
        );
    }
    assert_eq!(
        compared, 8,
        "the three fault cases have real SQL-role coverage in dependency_errors; signed join remains an explicit discriminator gap"
    );
    server.abort();
    rig.finish();
}

#[tokio::test]
async fn gateway_auth_precedes_mirrored_row_refusal_and_native_order_stays_unchanged() {
    let actor = wallet();
    let Some(rig) = Rig::new(Settings::for_tests()).await else {
        return;
    };
    let id = rig.community(&actor, false).await;
    sqlx::query("UPDATE social.communities SET origin='upstream' WHERE id=$1")
        .bind(id)
        .execute(&rig.pool)
        .await
        .unwrap();
    for (method, tail) in [
        ("put", ""),
        ("post", "/requests"),
        ("delete", "/posts/00000000-0000-4000-8000-000000000002"),
    ] {
        let path = format!("/v1/communities/{id}{tail}");
        let (status, body) = hit(
            &rig.app,
            Case::new(method, format!("/social-api{path}")).json(&json!({})),
        )
        .await;
        assert_gate_400(
            status,
            &body,
            "gateway authenticates before mirror authority",
        );
        let (status, body) = hit(
            &rig.app,
            Case::new(method, format!("/social-api{path}"))
                .signed(&actor)
                .await
                .json(&json!({})),
        )
        .await;
        assert_eq!(status, StatusCode::CONFLICT, "{body}");
        let (status, _) = hit(&rig.app, Case::new(method, path).json(&json!({}))).await;
        assert_eq!(
            status,
            StatusCode::CONFLICT,
            "native mirror refusal still precedes its handler auth"
        );
    }
    // Neither a header nor the URL spelling can manufacture the private marker.
    let request = Request::builder()
        .method("POST")
        .uri("/v1/communities")
        .header("x-social-api", "true")
        .body(Body::from_stream(futures::stream::pending::<
            Result<axum::body::Bytes, std::io::Error>,
        >()))
        .unwrap();
    assert!(
        tokio::time::timeout(
            std::time::Duration::from_millis(150),
            router(rig.state.clone()).oneshot(request)
        )
        .await
        .is_err(),
        "native buffering precedes auth without the server-owned marker"
    );
    rig.finish();
}

struct CountChecks(Arc<AtomicUsize>);
#[async_trait::async_trait]
impl crate::auth::eip1654::Eip1654Validator for CountChecks {
    async fn validate_signature(
        &self,
        _: &str,
        _: &[u8],
        _: &[u8],
    ) -> Result<bool, crate::auth::AuthError> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Ok(true)
    }
}

#[tokio::test]
async fn gateway_reuses_the_same_request_contract_wallet_verification_once() {
    let Some(rig) = Rig::new(Settings::for_tests()).await else {
        return;
    };
    let checks = Arc::new(AtomicUsize::new(0));
    let state = Arc::new(State::new(
        rig.pool.clone(),
        rig.state.storage.clone(),
        Settings::for_tests(),
        Some(Arc::new(CountChecks(checks.clone()))),
        &crate::http::Outbound::for_tests(),
        rig.state.explore.clone(),
    ));
    let app = Router::new()
        .merge(router(state.clone()))
        .nest("/social-api", crate::social_api_gateway(router(state)));
    let (owner, member) = (wallet(), wallet());
    let id = rig.community(&owner, false).await;
    for prefix in ["/social-api", ""] {
        let path = format!("{prefix}/v1/communities/{id}/members");
        let time = chrono::Utc::now().timestamp_millis().to_string();
        let payload = build_payload_v6("post", &path, &time, "{}");
        let links = [
            AuthLink {
                link_type: AuthLinkType::SIGNER,
                payload: member.address.to_string(),
                signature: None,
            },
            AuthLink {
                link_type: AuthLinkType::EcdsaEip1654SignedEntity,
                payload,
                signature: Some("0x00".into()),
            },
        ];
        let mut call = Case::new("post", path)
            .header(AUTH_TIMESTAMP_HEADER, &time)
            .header(AUTH_METADATA_HEADER, "{}");
        for (i, link) in links.iter().enumerate() {
            call = call.header(
                &format!("{AUTH_CHAIN_HEADER_PREFIX}{i}"),
                &serde_json::to_string(link).unwrap(),
            );
        }
        let before = checks.load(Ordering::SeqCst);
        let (status, body) = hit(&app, call).await;
        assert_eq!(status, StatusCode::NO_CONTENT, "{body}");
        assert_eq!(
            checks.load(Ordering::SeqCst),
            before + 1,
            "one verifier call, including gateway ingress and client handler"
        );
    }
    rig.finish();
}
