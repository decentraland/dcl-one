//! The accepted private call's lifecycle through the real voice runtime
//! (`CommsState`, not `FakeVoiceRooms`, so accepting records participant
//! rows) with LiveKit's room service replaced by a local stub that records
//! `DeleteRoom`: ending deletes the room, an accepted call never expires and
//! keeps its parties busy, and only a party ends it. Each test states what
//! upstream (`decentraland/social-service-ea` at `fb3dcb8`) does.

use std::sync::Mutex as StdMutex;
use std::sync::atomic::{AtomicBool, Ordering};

use tokio::sync::broadcast::Receiver;

use super::*;
use crate::comms::gatekeeper::{self as gatekeeper, CommsState};
use crate::comms::interfaces::test_support::{StubDirectory, StubPolicy, StubScenes};
use crate::http::Outbound;

struct Rig {
    db: Db,
    ctx: Context,
    runtime: Arc<CommsState>,
    deleted_rooms: Arc<StdMutex<Vec<String>>>,
    /// Set, the stub answers every `DeleteRoom` with a 500 (still recorded).
    refuse_deletes: Arc<AtomicBool>,
    livekit: tokio::task::JoinHandle<()>,
}

impl Rig {
    async fn new() -> Option<Self> {
        let pool = scratch_pool().await?;
        let deleted_rooms = Arc::new(StdMutex::new(Vec::new()));
        let refuse_deletes = Arc::new(AtomicBool::new(false));
        let (seen, refuse) = (deleted_rooms.clone(), refuse_deletes.clone());
        let stub = axum::Router::new().route(
            "/twirp/livekit.RoomService/DeleteRoom",
            axum::routing::post(move |axum::Json(body): axum::Json<serde_json::Value>| {
                let (seen, refuse) = (seen.clone(), refuse.clone());
                async move {
                    seen.lock()
                        .unwrap()
                        .push(body["room"].as_str().unwrap_or_default().to_string());
                    let status = if refuse.load(Ordering::SeqCst) {
                        axum::http::StatusCode::INTERNAL_SERVER_ERROR
                    } else {
                        axum::http::StatusCode::OK
                    };
                    (status, axum::Json(serde_json::json!({})))
                }
            }),
        );
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let mut settings = gatekeeper::Settings::for_tests();
        settings.livekit.api_url = format!("http://{}", listener.local_addr().unwrap());
        let livekit = tokio::spawn(async move { axum::serve(listener, stub).await.unwrap() });
        let runtime = Arc::new(CommsState::new(
            pool.clone(),
            Outbound::for_tests(),
            settings,
            None,
            None,
            Arc::new(StubPolicy::default()),
            Arc::new(StubScenes::default()),
            Arc::new(crate::comms::presence::Presence::new()),
            Arc::new(StubDirectory::default()),
        ));
        let db = Db::new(pool);
        let ctx = test_ctx(db.clone(), runtime.clone());
        Some(Self {
            db,
            ctx,
            runtime,
            deleted_rooms,
            refuse_deletes,
            livekit,
        })
    }

    /// A call from `caller` that `callee` accepted; its id.
    async fn accepted_call(&self, caller: &str, callee: &str) -> Uuid {
        let call_id = match SocialServiceImpl
            .start_private_voice_chat(
                StartPrivateVoiceChatPayload {
                    callee: Some(User {
                        address: callee.to_string(),
                    }),
                },
                procedure_ctx(&self.ctx, caller, 81),
            )
            .await
            .unwrap()
            .response
        {
            Some(start_private_voice_chat_response::Response::Ok(ok)) => ok.call_id,
            other => panic!("{other:?}"),
        };
        match SocialServiceImpl
            .accept_private_voice_chat(
                AcceptPrivateVoiceChatPayload {
                    call_id: call_id.clone(),
                },
                procedure_ctx(&self.ctx, callee, 82),
            )
            .await
            .unwrap()
            .response
        {
            Some(accept_private_voice_chat_response::Response::Ok(_)) => {}
            other => panic!("{other:?}"),
        }
        call_id.parse().unwrap()
    }

    async fn end(
        &self,
        me: &str,
        call_id: Uuid,
    ) -> Option<end_private_voice_chat_response::Response> {
        SocialServiceImpl
            .end_private_voice_chat(
                EndPrivateVoiceChatPayload {
                    call_id: call_id.to_string(),
                },
                procedure_ctx(&self.ctx, me, 84),
            )
            .await
            .unwrap()
            .response
    }

    fn room_deleted(&self, call_id: Uuid) -> bool {
        let room = crate::comms::rooms::private_voice_chat_room_name(&call_id);
        self.deleted_rooms.lock().unwrap().contains(&room)
    }

    async fn finish(self, wallets: &[&str]) {
        for w in wallets {
            sqlx::query(
                "DELETE FROM social.private_voice_chats WHERE caller_address = $1 OR callee_address = $1",
            )
            .bind(w)
            .execute(self.db.pool())
            .await
            .unwrap();
        }
        self.livekit.abort();
    }
}

fn statuses(rx: &mut Receiver<Arc<SocialEvent>>) -> Vec<i32> {
    std::iter::from_fn(|| rx.try_recv().ok())
        .filter_map(|e| match e.as_ref() {
            SocialEvent::PrivateVoice(u) => Some(u.status),
            _ => None,
        })
        .collect()
}

/// Explicit end deletes the media room. Disconnecting the Social socket only
/// looks up pending calls upstream, so an accepted media call survives it.
#[tokio::test]
async fn accepted_media_survives_social_disconnect_until_explicit_end() {
    let Some(rig) = Rig::new().await else {
        return;
    };
    let _turn = VOICE_TABLE.lock().await;
    let (caller, callee) = (addr("vb1a"), addr("vb1b"));
    let call = rig.accepted_call(&caller, &callee).await;
    let (mut to_caller, mut to_callee) = (
        rig.ctx.pubsub().subscribe(&caller),
        rig.ctx.pubsub().subscribe(&callee),
    );
    rig.ctx
        .end_private_voice_chat_on_disconnect(&caller.to_uppercase())
        .await;
    rig.ctx.end_private_voice_chat_on_disconnect(&callee).await;
    let kept = call_exists(&rig.db, call).await;
    let participants: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM social.private_voice_chat_participants WHERE call_id=$1",
    )
    .bind(call)
    .fetch_one(rig.db.pool())
    .await
    .unwrap();
    let disconnected_deleted = rig.room_deleted(call);
    let (caller_heard, callee_heard) = (statuses(&mut to_caller), statuses(&mut to_callee));
    let answer = rig.end(&caller, call).await;
    let ended_deleted = rig.room_deleted(call);
    rig.finish(&[&caller, &callee]).await;
    assert!(kept, "Social disconnect removed the accepted call");
    assert_eq!(
        participants, 2,
        "Social disconnect removed accepted participants"
    );
    assert!(
        !disconnected_deleted,
        "Social disconnect deleted the media room"
    );
    assert!(
        caller_heard.is_empty() && callee_heard.is_empty(),
        "disconnect published call updates: {caller_heard:?} {callee_heard:?}"
    );
    assert!(
        matches!(
            answer,
            Some(end_private_voice_chat_response::Response::Ok(_))
        ),
        "{answer:?}"
    );
    assert!(ended_deleted, "explicit End left the media room open");
}

/// An accepted call is never swept as EXPIRED, however long it lasts: the
/// sweep skips a call with participant rows (`expire_pending_private_calls`).
/// Upstream deletes the pending call when it is accepted
/// (`logic/voice/voice.ts`, `acceptPrivateVoiceChat`), so the sweep cannot
/// reach it. Backdating stands in for the TTL passing.
#[tokio::test]
async fn an_accepted_call_is_never_swept_as_expired() {
    let Some(rig) = Rig::new().await else {
        return;
    };
    let _turn = VOICE_TABLE.lock().await;
    let (caller, callee) = (addr("vb2a"), addr("vb2b"));
    let call = rig.accepted_call(&caller, &callee).await;
    backdate(&rig.db, call, EXPIRATION_MS + 1_000).await;
    let (mut to_caller, mut to_callee) = (
        rig.ctx.pubsub().subscribe(&caller),
        rig.ctx.pubsub().subscribe(&callee),
    );
    rig.ctx.expire_private_voice_chats(EXPIRATION_MS, 20).await;
    let expired = PrivateVoiceChatStatus::VoiceChatExpired as i32;
    let (caller_heard, callee_heard) = (statuses(&mut to_caller), statuses(&mut to_callee));
    rig.finish(&[&caller, &callee]).await;
    assert!(!caller_heard.contains(&expired), "{caller_heard:?}");
    assert!(!callee_heard.contains(&expired), "{callee_heard:?}");
}

/// An accepted call older than the pending TTL remains unavailable while its
/// media presence is live. It is no longer part of the pending busy relation.
#[tokio::test]
async fn an_old_accepted_call_with_live_presence_refuses_new_calls() {
    let Some(rig) = Rig::new().await else {
        return;
    };
    let _turn = VOICE_TABLE.lock().await;
    let (caller, callee, other) = (addr("vb3a"), addr("vb3b"), addr("vb3c"));
    let call = rig.accepted_call(&caller, &callee).await;
    backdate(&rig.db, call, EXPIRATION_MS + 1_000).await;
    let outcome =
        crate::rpc::service::ops::start_private_voice_chat(&rig.ctx, &other, &callee).await;
    let kept = call_exists(&rig.db, call).await;
    rig.finish(&[&caller, &callee, &other]).await;
    assert_eq!(
        outcome.unwrap_err().message(),
        format!("One of the users is already in a voice chat: {callee}")
    );
    assert!(kept, "a refused start preserves live media");
}

/// An accepted call older than the pending TTL does not arm the sweep: the
/// scheduler's earliest expiry (`private_voice_next_expiry_ms`) ignores it,
/// so a long call does not make every tick sweep.
#[tokio::test]
async fn an_old_accepted_call_does_not_arm_the_sweep() {
    const YEAR_MS: i64 = 365 * 24 * 3_600_000;
    let Some(rig) = Rig::new().await else {
        return;
    };
    let _turn = VOICE_TABLE.lock().await;
    let (caller, callee) = (addr("vb5a"), addr("vb5b"));
    let call = rig.accepted_call(&caller, &callee).await;
    // Older than any other row the shared scratch database can hold.
    backdate(&rig.db, call, 10 * YEAR_MS).await;
    let next = rig.db.private_voice_next_expiry_ms(EXPIRATION_MS).await;
    rig.ctx
        .sweep_private_voice_chats_if_due(EXPIRATION_MS, 20)
        .await;
    let due_soon = rig
        .db
        .private_voice_sweep_due(chrono::Utc::now().timestamp_millis() + 1_000);
    rig.finish(&[&caller, &callee]).await;
    let next = next.unwrap();
    assert!(next.is_none_or(|ms| ms > -YEAR_MS), "{next:?}");
    assert!(
        !due_soon,
        "the sweep is due again before the fallback interval"
    );
}

/// Only a party ends an accepted call (upstream `voice.ts:189-191`): anyone
/// else holding the id gets the unknown call's not-found, and the call, its
/// room and both parties are left alone.
#[tokio::test]
async fn a_stranger_cannot_end_an_accepted_call() {
    let Some(rig) = Rig::new().await else {
        return;
    };
    let _turn = VOICE_TABLE.lock().await;
    let (caller, callee, stranger) = (addr("vb4a"), addr("vb4b"), addr("vb4c"));
    let call = rig.accepted_call(&caller, &callee).await;
    let (mut to_caller, mut to_callee) = (
        rig.ctx.pubsub().subscribe(&caller),
        rig.ctx.pubsub().subscribe(&callee),
    );
    let refused = rig.end(&stranger, call).await;
    let (kept, deleted) = (call_exists(&rig.db, call).await, rig.room_deleted(call));
    let heard = statuses(&mut to_caller).len() + statuses(&mut to_callee).len();
    rig.finish(&[&caller, &callee, &stranger]).await;
    assert!(
        matches!(
            refused,
            Some(end_private_voice_chat_response::Response::NotFound(_))
        ),
        "{refused:?}"
    );
    assert!(kept, "a stranger's end deleted the call");
    assert!(!deleted, "a stranger's end deleted the room");
    assert_eq!(heard, 0, "a stranger's end told a party");
}

/// Regression guard: a failed LiveKit room delete is logged, never leaves the
/// call row behind, and the other party still hears ENDED (the ender, as
/// upstream, does not).
#[tokio::test]
async fn a_failed_room_delete_still_removes_the_call() {
    let Some(rig) = Rig::new().await else {
        return;
    };
    let _turn = VOICE_TABLE.lock().await;
    let (caller, callee) = (addr("vb6a"), addr("vb6b"));
    let call = rig.accepted_call(&caller, &callee).await;
    rig.refuse_deletes.store(true, Ordering::SeqCst);
    let (mut to_caller, mut to_callee) = (
        rig.ctx.pubsub().subscribe(&caller),
        rig.ctx.pubsub().subscribe(&callee),
    );
    let answer = rig.end(&callee, call).await;
    let (kept, asked) = (call_exists(&rig.db, call).await, rig.room_deleted(call));
    let ended = PrivateVoiceChatStatus::VoiceChatEnded as i32;
    let heard = (statuses(&mut to_caller), statuses(&mut to_callee));
    rig.finish(&[&caller, &callee]).await;
    assert!(
        matches!(
            answer,
            Some(end_private_voice_chat_response::Response::Ok(_))
        ),
        "{answer:?}"
    );
    assert!(!kept, "the refused room delete left the call row");
    assert!(asked, "the room delete was never asked for");
    assert!(heard.0.contains(&ended) && heard.1.is_empty(), "{heard:?}");
}

/// Unlike the gatekeeper's direct create endpoint, social acceptance must not recreate a
/// rejected intent or accept an already accepted one. The call-row lock chooses one winner.
#[tokio::test]
async fn accept_and_reject_race_has_one_winner_without_resurrection() {
    let Some(rig) = Rig::new().await else {
        return;
    };
    let _turn = VOICE_TABLE.lock().await;
    let (caller, callee) = (addr("arace1"), addr("arace2"));
    let caller_address = crate::shared::identity::Address::parse(&caller).unwrap();
    let callee_address = crate::shared::identity::Address::parse(&callee).unwrap();
    let voice_db = VoiceDb::new(rig.db.pool().clone(), Default::default());
    for _ in 0..8 {
        let id = voice_db
            .seed_pending_private_call(&caller, &callee)
            .await
            .unwrap();
        let (accepted, rejected) = tokio::join!(
            voice_db.accept_pending_private_call(id, &caller_address, &callee_address),
            voice_db.finish_pending_private_call(PendingPrivateCall::Rejected {
                id,
                callee: callee.clone()
            }),
        );
        assert_ne!(accepted.unwrap(), rejected.unwrap().is_some());
        assert!(
            !voice_db
                .accept_pending_private_call(id, &caller_address, &callee_address)
                .await
                .unwrap()
        );
        assert!(
            rig.db
                .incoming_private_voice_chat(&callee)
                .await
                .unwrap()
                .is_none()
        );
        sqlx::query("DELETE FROM social.private_voice_chats WHERE id=$1")
            .bind(id)
            .execute(rig.db.pool())
            .await
            .unwrap();
    }
    rig.finish(&[&caller, &callee]).await;
}

/// Execute the source's pending and media-presence distinction through the
/// real target comms runtime and SQL, before its periodic media cleanup.
#[tokio::test]
async fn new_pending_calls_match_source_while_historical_media_is_retained() {
    let fixture: serde_json::Value = serde_json::from_str(include_str!(concat!(
        env!("DCL_ONE_ROOT"),
        "/services/catalyst/tests/fixtures/conformance/social/private-lifecycle-source-cases.json"
    )))
    .unwrap();
    let Some(rig) = Rig::new().await else {
        return;
    };
    let _turn = VOICE_TABLE.lock().await;
    let caller = addr("lifecycle-caller");
    let callee = addr("lifecycle-callee");
    let cases = fixture["entries"].as_array().unwrap();
    assert_eq!(cases.len(), 6);
    for case in cases {
        let old = rig.accepted_call(&caller, &callee).await;
        let status = case["status"].as_str().unwrap();
        let stale = case["stale"].as_bool().unwrap();
        sqlx::query("UPDATE social.private_voice_chat_participants SET status=$2,joined_at=CURRENT_TIMESTAMP-($3::bigint*interval '1 second'),status_updated_at=CURRENT_TIMESTAMP-($3::bigint*interval '1 second') WHERE call_id=$1")
            .bind(old).bind(status).bind(if stale { 3600_i64 } else { 0 }).execute(rig.db.pool()).await.unwrap();
        let pre = rig
            .ctx
            .db()
            .private_voice_preflight(
                &caller,
                &callee,
                EXPIRATION_MS,
                rig.ctx.voice().presence_config(),
            )
            .await
            .unwrap();
        assert_eq!(pre.caller_in_voice, case["source_presence"], "{case}");
        assert!(
            !pre.busy,
            "accepted media is absent from the source pending relation"
        );
        let result =
            crate::rpc::service::ops::start_private_voice_chat(&rig.ctx, &caller, &callee).await;
        assert_eq!(result.is_ok(), case["started"], "{case}: {result:?}");
        if let Err(error) = result {
            let message = error
                .message()
                .replace(&callee, "0x2222222222222222222222222222222222222222");
            assert_eq!(message, case["error"], "{case}");
        }
        let pending: i64 = sqlx::query_scalar("SELECT count(*) FROM social.private_voice_chats c WHERE caller_address=$1 AND NOT EXISTS(SELECT 1 FROM social.private_voice_chat_participants p WHERE p.call_id=c.id)").bind(&caller).fetch_one(rig.db.pool()).await.unwrap();
        let media: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM social.private_voice_chat_participants WHERE call_id=$1",
        )
        .bind(old)
        .fetch_one(rig.db.pool())
        .await
        .unwrap();
        assert_eq!(pending, case["pending_rows"], "{case}");
        assert_eq!(media, case["media_rows"], "{case}");
        assert!(
            call_exists(&rig.db, old).await,
            "start must preserve historical media: {case}"
        );
        assert!(
            !rig.room_deleted(old),
            "start must not delete media: {case}"
        );
        sqlx::query(
            "DELETE FROM social.private_voice_chats WHERE caller_address=$1 OR callee_address=$1",
        )
        .bind(&caller)
        .execute(rig.db.pool())
        .await
        .unwrap();
    }
    rig.finish(&[&caller, &callee]).await;
}

#[tokio::test]
async fn issuing_media_credentials_preserves_pending_and_prior_media_until_join() {
    let fixture: serde_json::Value = serde_json::from_str(include_str!(concat!(
        env!("DCL_ONE_ROOT"),
        "/services/catalyst/tests/fixtures/conformance/social/private-lifecycle-source-cases.json"
    )))
    .unwrap();
    let Some(rig) = Rig::new().await else {
        return;
    };
    let _turn = VOICE_TABLE.lock().await;
    let (a, b, c) = (addr("writer-a"), addr("writer-b"), addr("writer-c"));
    let pending = crate::rpc::service::ops::start_private_voice_chat(&rig.ctx, &a, &b)
        .await
        .unwrap();
    let first = Uuid::new_v4();
    let second = Uuid::new_v4();
    for (position, id, parties) in [
        (0, first, [c.clone(), a.clone()]),
        (1, second, [a.clone(), b.clone()]),
    ] {
        let addresses =
            parties.map(|address| crate::shared::identity::Address::parse(&address).unwrap());
        let credentials = gatekeeper::voice::get_private_voice_chat_room_credentials(
            &rig.runtime,
            &id.to_string(),
            &addresses,
        )
        .await
        .unwrap();
        assert_eq!(credentials.len(), 2);
        let pending_retained = rig
            .ctx
            .db()
            .get_private_voice_chat(pending)
            .await
            .unwrap()
            .is_some();
        let expected = &fixture["writerSequences"][position];
        assert_eq!(pending_retained, expected["pending_retained"], "{expected}");
        let media_rows: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM social.private_voice_chat_participants WHERE call_id=ANY($1)",
        )
        .bind(vec![first, second])
        .fetch_one(rig.db.pool())
        .await
        .unwrap();
        assert_eq!(media_rows, expected["media_rows"], "{expected}");
        assert!(
            rig.deleted_rooms.lock().unwrap().is_empty(),
            "issuing credentials deletes no media room"
        );
        if position == 0 {
            gatekeeper::voice::handle_private_participant_joined(&rig.runtime, &a, first)
                .await
                .unwrap();
        }
    }
    // The actual join, not the earlier credential issuance, displaces the
    // user's previously connected media room through the existing lifecycle.
    gatekeeper::voice::handle_private_participant_joined(&rig.runtime, &a, second)
        .await
        .unwrap();
    assert!(rig.room_deleted(first));
    assert!(!rig.room_deleted(second));
    assert!(
        call_exists(&rig.db, pending).await,
        "media join preserves the independently pending intent"
    );
    let response = rig.end(&a, pending).await;
    assert!(matches!(
        response,
        Some(end_private_voice_chat_response::Response::Ok(_))
    ));
    assert!(
        call_exists(&rig.db, first).await && call_exists(&rig.db, second).await,
        "ending a pending call does not select either media call"
    );
    let response = rig.end(&a, second).await;
    assert!(matches!(
        response,
        Some(end_private_voice_chat_response::Response::Ok(_))
    ));
    assert!(!call_exists(&rig.db, second).await);
    assert!(
        call_exists(&rig.db, first).await,
        "explicit media end selects its exact call ID"
    );
    rig.finish(&[&a, &b, &c]).await;
}

#[tokio::test]
async fn concurrent_media_writers_preserve_both_rooms_and_the_pending_call() {
    let Some(rig) = Rig::new().await else {
        return;
    };
    let _turn = VOICE_TABLE.lock().await;
    let (a, b, c) = (
        addr("media-race-a"),
        addr("media-race-b"),
        addr("media-race-c"),
    );
    let pending = crate::rpc::service::ops::start_private_voice_chat(&rig.ctx, &a, &b)
        .await
        .unwrap();
    let (first, second) = (Uuid::new_v4(), Uuid::new_v4());
    let addresses = |parties: [String; 2]| {
        parties.map(|party| crate::shared::identity::Address::parse(&party).unwrap())
    };
    let one = addresses([c.clone(), a.to_uppercase()]);
    let two = addresses([a.clone(), b.to_uppercase()]);
    let first_id = first.to_string();
    let second_id = second.to_string();
    let (one, two) = tokio::join!(
        gatekeeper::voice::get_private_voice_chat_room_credentials(&rig.runtime, &first_id, &one),
        gatekeeper::voice::get_private_voice_chat_room_credentials(&rig.runtime, &second_id, &two)
    );
    assert_eq!(one.unwrap().len(), 2);
    assert_eq!(two.unwrap().len(), 2);
    assert!(
        rig.ctx
            .db()
            .get_private_voice_chat(pending)
            .await
            .unwrap()
            .is_some()
    );
    let rows: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM social.private_voice_chat_participants WHERE call_id=ANY($1)",
    )
    .bind(vec![first, second])
    .fetch_one(rig.db.pool())
    .await
    .unwrap();
    assert_eq!(
        rows, 4,
        "source independent media writers retain both rooms"
    );
    assert!(rig.deleted_rooms.lock().unwrap().is_empty());
    rig.finish(&[&a, &b, &c]).await;
}

#[tokio::test]
async fn starts_acceptance_media_expiry_and_explicit_ends_keep_their_call_identity() {
    let Some(rig) = Rig::new().await else {
        return;
    };
    let _turn = VOICE_TABLE.lock().await;
    let db = VoiceDb::new(rig.db.pool().clone(), Default::default());
    for reversed in [false, true] {
        for _ in 0..8 {
            let (a, b, c, d) = (
                addr("transition-a"),
                addr("transition-b"),
                addr("transition-c"),
                addr("transition-d"),
            );
            let (caller, callee) = if reversed { (&b, &a) } else { (&a, &b) };
            let original = db
                .create_pending_private_call(caller, callee, EXPIRATION_MS)
                .await
                .unwrap()
                .started
                .unwrap();
            backdate(&rig.db, original, EXPIRATION_MS + 1_000).await;
            let caller_address = crate::shared::identity::Address::parse(caller).unwrap();
            let callee_address = crate::shared::identity::Address::parse(callee).unwrap();
            let media = Uuid::new_v4();
            let media_caller = crate::shared::identity::Address::parse(&d).unwrap();
            let media_callee = crate::shared::identity::Address::parse(&a.to_uppercase()).unwrap();
            let next_caller = if reversed {
                a.to_uppercase()
            } else {
                c.to_uppercase()
            };
            let next_callee = if reversed { c.clone() } else { a.clone() };
            let barrier = tokio::sync::Barrier::new(4);
            let (accepted, expired, next, created) = tokio::join!(
                async {
                    barrier.wait().await;
                    db.accept_pending_private_call(original, &caller_address, &callee_address)
                        .await
                },
                async {
                    barrier.wait().await;
                    db.expire_pending_private_calls(EXPIRATION_MS, 64).await
                },
                async {
                    barrier.wait().await;
                    db.create_pending_private_call(&next_caller, &next_callee, EXPIRATION_MS)
                        .await
                },
                async {
                    barrier.wait().await;
                    db.create_private_call(media, &media_caller, &media_callee)
                        .await
                }
            );
            let (accepted, expired, next) = (accepted.unwrap(), expired.unwrap(), next.unwrap());
            assert!(
                created.unwrap().is_empty(),
                "issuing credentials displaces no call"
            );
            assert_eq!(db.participants_in(media).await.unwrap().len(), 2);
            if accepted {
                assert!(!expired.iter().any(|call| call.id == original));
                assert_eq!(db.participants_in(original).await.unwrap().len(), 2);
            } else {
                assert!(expired.iter().any(|call| call.id == original));
                assert!(
                    !call_exists(&rig.db, original).await,
                    "late acceptance must not resurrect an expired intent"
                );
            }
            let pending:i64=sqlx::query_scalar("SELECT count(*) FROM social.private_voice_chats c WHERE (caller_address=$1 OR callee_address=$1) AND NOT EXISTS(SELECT 1 FROM social.private_voice_chat_participants p WHERE p.call_id=c.id)").bind(&a).fetch_one(rig.db.pool()).await.unwrap();
            assert_eq!(
                pending,
                i64::from(next.started.is_some()),
                "at most one pending survives across both roles"
            );
            if let Some(next) = next.started {
                let ended = db
                    .finish_pending_private_call(PendingPrivateCall::Ended {
                        id: next,
                        party: a.to_uppercase(),
                    })
                    .await
                    .unwrap()
                    .unwrap();
                assert_eq!(ended.id, next);
                assert_eq!(
                    db.participants_in(media).await.unwrap().len(),
                    2,
                    "ending the pending selects no media room"
                );
            }
            if accepted {
                let ended = db
                    .delete_private_call_if_member(Some(original), &caller_address)
                    .await
                    .unwrap();
                assert_eq!(
                    ended.call.id, original,
                    "explicit media end selects its exact call ID"
                );
                assert_eq!(db.participants_in(media).await.unwrap().len(), 2);
            }
            sqlx::query("DELETE FROM social.private_voice_chats WHERE id=$1")
                .bind(media)
                .execute(rig.db.pool())
                .await
                .unwrap();
        }
    }
    assert!(
        rig.deleted_rooms.lock().unwrap().is_empty(),
        "SQL transitions do not invent media teardown"
    );
    rig.finish(&[]).await;
}
