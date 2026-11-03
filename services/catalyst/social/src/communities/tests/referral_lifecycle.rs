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
        case.headers.push(("cf-connecting-ip".into(), ip.into()));
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
