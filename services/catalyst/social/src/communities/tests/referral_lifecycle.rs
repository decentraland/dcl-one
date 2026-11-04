use super::*;

async fn create(
    rig: &Rig,
    referrer: &Wallet,
    invited: &Wallet,
    ip: Option<&str>,
) -> (StatusCode, Value) {
    let mut case = Case::new("post", "/v1/referral-progress")
        .signed(invited)
        .await
        .json(&json!({"referrer":referrer.address.as_str()}));
    if let Some(ip) = ip {
        case = case.peer(ip);
    }
    hit(&rig.app, case).await
}
#[tokio::test]
async fn referral_http_persists_rejected_ip_and_same_referrer_retry_is_a_noop() {
    let Some(rig) = Rig::new(Settings::for_tests()).await else {
        return;
    };
    let referrer = wallet();
    let invited = [wallet(), wallet(), wallet()];
    let (status, value) = create(&rig, &referrer, &invited[0], None).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(value["message"], "Unable to determine client IP address");
    for invite in &invited[..2] {
        assert_eq!(
            create(&rig, &referrer, invite, Some("192.0.2.1")).await.0,
            StatusCode::NO_CONTENT
        );
    }
    let (status, value) = create(&rig, &referrer, &invited[2], Some("192.0.2.1")).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(
        value["message"],
        "Invited user has already reached the maximum number of 2 referrals from the same IP: 192.0.2.1"
    );
    let (status, ip): (String, String) = sqlx::query_as(
        "SELECT status,invited_user_ip FROM social.referral_progress WHERE invited_user=$1",
    )
    .bind(&invited[2].address)
    .fetch_one(&rig.pool)
    .await
    .unwrap();
    assert_eq!(
        (status, ip),
        ("rejected_ip_match".into(), "192.0.2.1".into())
    );
    assert_eq!(
        create(&rig, &referrer, &invited[2], Some("192.0.2.1"))
            .await
            .0,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        create(&rig, &wallet(), &invited[2], Some("192.0.2.1"))
            .await
            .0,
        StatusCode::BAD_REQUEST
    );
    rig.finish();
}
#[tokio::test]
async fn registration_kill_flag_only_blocks_new_posts_and_preserves_stats_and_signup() {
    let mut settings = Settings::for_tests();
    settings
        .features
        .overrides
        .insert("referral_registration_disabled".into(), "1".into());
    let Some(rig) = Rig::new(settings).await else {
        return;
    };
    let (referrer, invited) = (wallet(), wallet());
    rig.state
        .referral
        .create(&referrer.address, &invited.address, "192.0.2.2")
        .await
        .unwrap();
    let (status, value) = create(&rig, &referrer, &invited, Some("192.0.2.2")).await;
    assert_eq!(
        (status, value),
        (
            StatusCode::SERVICE_UNAVAILABLE,
            json!({"error":"Service Unavailable","message":"Referral registration is temporarily disabled"})
        )
    );
    assert_eq!(
        hit(
            &rig.app,
            Case::new("patch", "/v1/referral-progress")
                .signed(&invited)
                .await
        )
        .await
        .0,
        StatusCode::NO_CONTENT
    );
    let (status, value) = hit(
        &rig.app,
        Case::new("get", "/v1/referral-progress")
            .signed(&referrer)
            .await,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        value,
        json!({"invitedUsersAccepted":0,"invitedUsersAcceptedViewed":0,"rewardImages":[]})
    );
    rig.finish();
}
#[tokio::test]
async fn referral_stats_include_reward_snapshots_and_mark_the_current_count_viewed() {
    let Some(rig) = Rig::new(Settings::for_tests()).await else {
        return;
    };
    let (referrer, invited) = (wallet(), wallet());
    let app = crate::writes::guard(rig.app.clone(), true);
    sqlx::query("INSERT INTO social.referral_progress(invited_user,referrer,status,tier_granted) VALUES($1,$2,'tier_granted',true)").bind(&invited.address).bind(&referrer.address).execute(&rig.pool).await.unwrap();
    sqlx::query("INSERT INTO social.referral_reward_images(referrer,tier,reward_image_url) VALUES($1,5,'https://reward.example.org/art.png')").bind(&referrer.address).execute(&rig.pool).await.unwrap();
    for viewed in [0, 1] {
        let (status, value) = hit(
            &app,
            Case::new("get", "/v1/referral-progress")
                .signed(&referrer)
                .await,
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            value,
            json!({"invitedUsersAccepted":1,"invitedUsersAcceptedViewed":viewed,"rewardImages":[{"tier":5,"url":"https://reward.example.org/art.png"}]})
        );
    }
    rig.finish();
}

#[tokio::test]
async fn referral_network_peer_prevents_spoofing_and_trusted_gateway_keeps_client_buckets() {
    for trusted_gateway in [false, true] {
        let mut settings = Settings::for_tests();
        settings.referral.suspicious_window_ms = 0.0;
        if trusted_gateway {
            settings.referral.trusted_proxy_cidrs = vec!["127.0.0.1/32".parse().unwrap()];
        }
        let Some(rig) = Rig::new(settings).await else {
            return;
        };
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let endpoint = format!(
            "http://{}/v1/referral-progress",
            listener.local_addr().unwrap()
        );
        // Match serve's outer normalization and connection metadata boundary.
        let app = tower_http::normalize_path::NormalizePath::trim_trailing_slash(rig.app.clone());
        let server = tokio::spawn(async move {
            axum::serve(
                listener,
                axum::ServiceExt::<axum::extract::Request>::into_make_service_with_connect_info::<
                    std::net::SocketAddr,
                >(app),
            )
            .await
            .unwrap()
        });
        let referrer = wallet();
        let http = crate::http::Outbound::for_tests();
        let mut invited = Vec::new();
        for (index, client) in [
            "2001:0DB8:0:0:0:0:0:1",
            "2001:db8::1",
            "2001:db8::1",
            "2001:db8::2",
        ]
        .into_iter()
        .enumerate()
        {
            let user = wallet();
            let mut request = http
                .post(&endpoint)
                .header("cf-connecting-ip", format!("198.51.100.{}", index + 1))
                .header("x-forwarded-for", format!("198.51.100.{}", index + 11))
                .header(
                    "x-real-ip",
                    if trusted_gateway {
                        client.to_string()
                    } else {
                        format!("198.51.100.{}", index + 21)
                    },
                )
                .json(&json!({"referrer":referrer.address.as_str()}));
            for (name, value) in signed_headers(&user, "post", "/v1/referral-progress").await {
                request = request.header(name.as_str(), value.as_str());
            }
            let response = request.send().await.unwrap();
            let accepted = index < 2 || (trusted_gateway && index == 3);
            assert_eq!(
                response.status(),
                if accepted {
                    StatusCode::NO_CONTENT
                } else {
                    StatusCode::BAD_REQUEST
                }
            );
            invited.push(user.address.to_string());
        }
        let rows: Vec<(String, String)> = sqlx::query_as("SELECT invited_user_ip,status FROM social.referral_progress WHERE invited_user=ANY($1) ORDER BY created_at,id")
            .bind(&invited).fetch_all(&rig.pool).await.unwrap();
        assert_eq!(
            rows.iter()
                .filter(|(_, status)| status == "pending")
                .count(),
            if trusted_gateway { 3 } else { 2 }
        );
        if trusted_gateway {
            assert_eq!(rows.iter().filter(|(ip, _)| ip == "2001:db8::1").count(), 3);
            assert_eq!(rows.iter().filter(|(ip, _)| ip == "2001:db8::2").count(), 1);
        } else {
            assert!(rows.iter().all(|(ip, _)| ip == "127.0.0.1"));
        }
        let no_peer = Case::new("post", "/v1/referral-progress")
            .signed(&wallet())
            .await
            .header("cf-connecting-ip", "198.51.100.1")
            .header("x-forwarded-for", "198.51.100.2")
            .header("x-real-ip", "198.51.100.3")
            .json(&json!({"referrer":referrer.address.as_str()}));
        let (status, body) = hit(&rig.app, no_peer).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["message"], "Unable to determine client IP address");
        server.abort();
        rig.finish();
    }
}

#[tokio::test]
async fn referral_http_response_and_committed_rejection_survive_a_stalled_slack_body() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let Some(pool) = crate::db::scratch::pool(4).await else {
        return;
    };
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = format!("http://{}/chat.postMessage", listener.local_addr().unwrap());
    let (entered, received) = tokio::sync::oneshot::channel();
    let (release, released) = tokio::sync::oneshot::channel();
    let server = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut bytes = [0; 8192];
        socket.read(&mut bytes).await.unwrap();
        socket.write_all(b"HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: 11\r\nconnection: close\r\n\r\n{\"ok\":").await.unwrap();
        entered.send(()).unwrap();
        released.await.unwrap();
    });
    let dir = std::env::temp_dir().join(format!("referral-http-budget-{}", Uuid::new_v4()));
    let storage = Arc::new(crate::storage::Storage::from_config(&dir).await.unwrap());
    let mut settings = Settings::for_tests();
    settings.referral.max_ip_matches = 0.0;
    settings.referral.suspicious_window_ms = 0.0;
    settings.referral.slack.token = Some("fixture-token".into());
    let mut state = State::new(
        pool.clone(),
        storage,
        settings,
        None,
        &crate::http::Outbound::for_tests(),
        State::explore_for_tests(&pool),
    );
    state.referral.set_test_slack_endpoint(endpoint);
    let app = router(Arc::new(state));
    let (referrer, invited) = (wallet(), wallet());
    let case = Case::new("post", "/v1/referral-progress")
        .peer("192.0.2.1")
        .signed(&invited)
        .await
        .json(&json!({"referrer":referrer.address.as_str()}));
    let running = app.clone();
    let started = tokio::time::Instant::now();
    let pending = tokio::spawn(async move { hit(&running, case).await });
    received.await.unwrap();
    let state: (String, String) = sqlx::query_as(
        "SELECT status,invited_user_ip FROM social.referral_progress WHERE invited_user=$1",
    )
    .bind(&invited.address)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(state, ("rejected_ip_match".into(), "192.0.2.1".into()));
    let (status, body) = tokio::time::timeout(std::time::Duration::from_secs(15), pending)
        .await
        .unwrap()
        .unwrap();
    assert!(started.elapsed() < std::time::Duration::from_secs(15));
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(
        body["message"]
            .as_str()
            .unwrap()
            .contains("maximum number of 0 referrals")
    );
    let retry = Case::new("post", "/v1/referral-progress")
        .peer("192.0.2.1")
        .signed(&invited)
        .await
        .json(&json!({"referrer":referrer.address.as_str()}));
    assert_eq!(hit(&app, retry).await.0, StatusCode::NO_CONTENT);
    release.send(()).unwrap();
    server.await.unwrap();
    std::fs::remove_dir_all(dir).unwrap();
}
