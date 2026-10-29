//! Social settings through the RPC handlers, the cases of upstream's
//! `get-social-settings`, `upsert-social-settings`,
//! `get-private-messages-settings` and `logic/settings` specs
//! (`decentraland/social-service-ea` at `fb3dcb8`): the defaults of a wallet
//! without a row, the round trip of every value, and the per-address reads.

use super::*;

fn settings(
    pmp: PrivateMessagePrivacySetting,
    bvis: BlockedUsersMessagesVisibilitySetting,
    sreact: SituationReactionsVisibility,
) -> SocialSettings {
    SocialSettings {
        private_messages_privacy: pmp as i32,
        blocked_users_messages_visibility: bvis as i32,
        show_situation_reactions: sreact as i32,
    }
}

const DEFAULTS: (
    PrivateMessagePrivacySetting,
    BlockedUsersMessagesVisibilitySetting,
    SituationReactionsVisibility,
) = (
    PrivateMessagePrivacySetting::All,
    BlockedUsersMessagesVisibilitySetting::ShowMessages,
    SituationReactionsVisibility::Show,
);

async fn read(ctx: &Context, me: &str) -> SocialSettings {
    match SocialServiceImpl
        .get_social_settings(procedure_ctx(ctx, me, 41))
        .await
        .unwrap()
        .response
    {
        Some(get_social_settings_response::Response::Ok(ok)) => ok.settings.unwrap(),
        other => panic!("{other:?}"),
    }
}

async fn upsert(ctx: &Context, me: &str, payload: UpsertSocialSettingsPayload) -> SocialSettings {
    match SocialServiceImpl
        .upsert_social_settings(payload, procedure_ctx(ctx, me, 42))
        .await
        .unwrap()
        .response
    {
        Some(upsert_social_settings_response::Response::Ok(s)) => s,
        other => panic!("{other:?}"),
    }
}

async fn row_count(db: &Db, me: &str) -> i64 {
    sqlx::query_scalar("SELECT count(*) FROM social.user_settings WHERE address = $1")
        .bind(me)
        .fetch_one(db.pool())
        .await
        .unwrap()
}

async fn forget(db: &Db, me: &str) {
    sqlx::query("DELETE FROM social.user_settings WHERE address = $1")
        .bind(me)
        .execute(db.pool())
        .await
        .unwrap();
}

/// A wallet without a row reads the defaults and the read inserts nothing;
/// each value written reads back, and a partial write keeps the others.
#[tokio::test]
async fn settings_read_their_defaults_and_round_trip_every_value() {
    let Some(pool) = scratch_pool().await else {
        return;
    };
    let db = Db::new(pool.clone());
    let ctx = test_ctx(db.clone(), Arc::new(FakeVoiceRooms::new(pool)));
    let me = addr("ss1");
    let (pmp, bvis, sreact) = DEFAULTS;
    assert_eq!(read(&ctx, &me).await, settings(pmp, bvis, sreact));
    assert_eq!(row_count(&db, &me).await, 0, "a read inserts nothing");

    let written = upsert(
        &ctx,
        &me,
        UpsertSocialSettingsPayload {
            private_messages_privacy: Some(PrivateMessagePrivacySetting::OnlyFriends as i32),
            ..Default::default()
        },
    )
    .await;
    let only_friends = settings(PrivateMessagePrivacySetting::OnlyFriends, bvis, sreact);
    assert_eq!(written, only_friends);
    assert_eq!(read(&ctx, &me).await, only_friends);

    let every_other = settings(
        PrivateMessagePrivacySetting::All,
        BlockedUsersMessagesVisibilitySetting::DoNotShowMessages,
        SituationReactionsVisibility::Hide,
    );
    let written = upsert(
        &ctx,
        &me,
        UpsertSocialSettingsPayload {
            private_messages_privacy: Some(every_other.private_messages_privacy),
            blocked_users_messages_visibility: Some(every_other.blocked_users_messages_visibility),
            show_situation_reactions: Some(every_other.show_situation_reactions),
        },
    )
    .await;
    assert_eq!(written, every_other);
    assert_eq!(read(&ctx, &me).await, every_other);

    let written = upsert(
        &ctx,
        &me,
        UpsertSocialSettingsPayload {
            show_situation_reactions: Some(SituationReactionsVisibility::Show as i32),
            ..Default::default()
        },
    )
    .await;
    assert_eq!(
        written,
        settings(
            PrivateMessagePrivacySetting::All,
            BlockedUsersMessagesVisibilitySetting::DoNotShowMessages,
            SituationReactionsVisibility::Show,
        )
    );
    forget(&db, &me).await;
}

/// Per address, in the order asked: the stored privacy (or `all` without a
/// row) and whether the caller is their friend.
#[tokio::test]
async fn private_message_settings_read_per_address_with_friendship() {
    let Some(pool) = scratch_pool().await else {
        return;
    };
    let db = Db::new(pool.clone());
    let ctx = test_ctx(db.clone(), Arc::new(FakeVoiceRooms::new(pool)));
    let (me, strict, friend) = (addr("pm1"), addr("pm2"), addr("pm3"));
    upsert(
        &ctx,
        &strict,
        UpsertSocialSettingsPayload {
            private_messages_privacy: Some(PrivateMessagePrivacySetting::OnlyFriends as i32),
            ..Default::default()
        },
    )
    .await;
    db.apply_friendship_action(&me, &friend, "accept", true, None, None)
        .await
        .unwrap();

    let ask = |users: Vec<String>| {
        let payload = GetPrivateMessagesSettingsPayload {
            user: users.into_iter().map(|address| User { address }).collect(),
        };
        let pctx = procedure_ctx(&ctx, &me, 43);
        async move {
            SocialServiceImpl
                .get_private_messages_settings(payload, pctx)
                .await
                .unwrap()
                .response
                .unwrap()
        }
    };
    match ask(vec![
        friend.clone(),
        strict.to_uppercase().replacen("0X", "0x", 1),
    ])
    .await
    {
        get_private_messages_settings_response::Response::Ok(ok) => {
            let got: Vec<(String, i32, bool)> = ok
                .settings
                .into_iter()
                .map(|s| {
                    (
                        s.user.unwrap().address,
                        s.private_messages_privacy,
                        s.is_friend,
                    )
                })
                .collect();
            assert_eq!(
                got,
                [
                    (
                        friend.clone(),
                        PrivateMessagePrivacySetting::All as i32,
                        true
                    ),
                    (
                        strict.clone(),
                        PrivateMessagePrivacySetting::OnlyFriends as i32,
                        false
                    ),
                ]
            );
        }
        other => panic!("{other:?}"),
    }
    match ask(Vec::new()).await {
        get_private_messages_settings_response::Response::Ok(ok) => assert!(ok.settings.is_empty()),
        other => panic!("{other:?}"),
    }
    match ask((0..51).map(|i| addr(&format!("x{i}"))).collect()).await {
        get_private_messages_settings_response::Response::InvalidRequest(e) => {
            assert_eq!(e.message.as_deref(), Some("Too many user addresses: 51"))
        }
        other => panic!("{other:?}"),
    }
    forget(&db, &strict).await;
    cleanup_pair(&db, &me, &friend).await;
}
