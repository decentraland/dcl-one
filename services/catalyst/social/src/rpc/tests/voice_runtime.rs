//! The voice cases of upstream's `logic/voice-logic.spec.ts` and the
//! community-voice specs (`decentraland/social-service-ea` at `fb3dcb8`) that
//! need the runtime to say no: a reservation lost to a competing call, and a
//! grant that cannot be minted. [`Faulty`] is the fake runtime with those two
//! answers switchable.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};

use tokio::sync::broadcast::Receiver;

use super::*;
use crate::comms::gatekeeper::voice::db::{PrivateVoiceChatRow, StartOutcome};
use crate::comms::gatekeeper::voice::rooms::{CommunityVoiceAction, VoiceRooms, VoiceUnavailable};

/// [`FakeVoiceRooms`] that can lose every reservation (`started: None`, as a
/// competing call that locked first leaves it) or fail every grant.
struct Faulty {
    inner: FakeVoiceRooms,
    lose_reservations: AtomicBool,
    fail_grants: AtomicBool,
    omit_caller: AtomicBool,
    omit_callee: AtomicBool,
}

impl Faulty {
    fn failed() -> VoiceUnavailable {
        VoiceUnavailable::Failed("test runtime refused".into())
    }
}

#[async_trait::async_trait]
impl VoiceRooms for Faulty {
    fn presence_config(&self) -> crate::comms::gatekeeper::voice::db::VoiceDbConfig {
        self.inner.presence_config()
    }
    async fn create_pending_private_call(
        &self,
        caller: &str,
        callee: &str,
        expiration_ms: i64,
    ) -> Result<StartOutcome, VoiceUnavailable> {
        if self.lose_reservations.load(Ordering::SeqCst) {
            return Ok(StartOutcome {
                started: None,
                expired: Vec::new(),
            });
        }
        self.inner
            .create_pending_private_call(caller, callee, expiration_ms)
            .await
    }

    async fn finish_pending_private_call(
        &self,
        call: PendingPrivateCall,
    ) -> Result<Option<PrivateVoiceChatRow>, VoiceUnavailable> {
        self.inner.finish_pending_private_call(call).await
    }

    async fn expire_pending_private_calls(
        &self,
        expiration_ms: i64,
        limit: i64,
    ) -> Result<Vec<PrivateVoiceChatRow>, VoiceUnavailable> {
        self.inner
            .expire_pending_private_calls(expiration_ms, limit)
            .await
    }

    async fn private_voice_credentials(
        &self,
        call_id: Uuid,
        callee: &str,
        caller: &str,
    ) -> Result<HashMap<String, String>, VoiceUnavailable> {
        if self.fail_grants.load(Ordering::SeqCst) {
            return Err(Self::failed());
        }
        let mut credentials = self
            .inner
            .private_voice_credentials(call_id, callee, caller)
            .await?;
        if self.omit_caller.load(Ordering::SeqCst) {
            credentials.remove(caller);
        }
        if self.omit_callee.load(Ordering::SeqCst) {
            credentials.remove(callee);
        }
        Ok(credentials)
    }

    async fn end_private_voice_chat(
        &self,
        call_id: Option<Uuid>,
        address: &str,
    ) -> Result<Option<PrivateVoiceChatRow>, VoiceUnavailable> {
        self.inner.end_private_voice_chat(call_id, address).await
    }

    async fn community_voice_credentials(
        &self,
        community_id: Uuid,
        address: &str,
        role: &str,
        action: CommunityVoiceAction,
        profile: Option<serde_json::Value>,
    ) -> Result<String, VoiceUnavailable> {
        if self.fail_grants.load(Ordering::SeqCst) {
            return Err(Self::failed());
        }
        self.inner
            .community_voice_credentials(community_id, address, role, action, profile)
            .await
    }

    async fn community_voice_chat_active(
        &self,
        community_id: Uuid,
    ) -> Result<bool, VoiceUnavailable> {
        self.inner.community_voice_chat_active(community_id).await
    }

    async fn end_community_voice_chat(
        &self,
        community_id: Uuid,
        address: &str,
    ) -> Result<(), VoiceUnavailable> {
        self.inner
            .end_community_voice_chat(community_id, address)
            .await
    }

    async fn request_to_speak(
        &self,
        community_id: Uuid,
        address: &str,
        raising_hand: bool,
    ) -> Result<(), VoiceUnavailable> {
        self.inner
            .request_to_speak(community_id, address, raising_hand)
            .await
    }

    async fn reject_speak_request(
        &self,
        community_id: Uuid,
        address: &str,
    ) -> Result<(), VoiceUnavailable> {
        self.inner.reject_speak_request(community_id, address).await
    }

    async fn set_speaker(
        &self,
        community_id: Uuid,
        address: &str,
        promote: bool,
    ) -> Result<(), VoiceUnavailable> {
        self.inner.set_speaker(community_id, address, promote).await
    }

    async fn kick_player(&self, community_id: Uuid, address: &str) -> Result<(), VoiceUnavailable> {
        self.inner.kick_player(community_id, address).await
    }

    async fn mute_speaker(
        &self,
        community_id: Uuid,
        address: &str,
        muted: bool,
    ) -> Result<(), VoiceUnavailable> {
        self.inner.mute_speaker(community_id, address, muted).await
    }
}

struct Rig {
    pool: PgPool,
    ctx: Context,
    runtime: Arc<Faulty>,
}

impl Rig {
    async fn new() -> Option<Self> {
        let pool = scratch_pool().await?;
        let runtime = Arc::new(Faulty {
            inner: FakeVoiceRooms::new(pool.clone()),
            lose_reservations: AtomicBool::new(false),
            fail_grants: AtomicBool::new(false),
            omit_caller: AtomicBool::new(false),
            omit_callee: AtomicBool::new(false),
        });
        let ctx = test_ctx(Db::new(pool.clone()), runtime.clone());
        Some(Self { pool, ctx, runtime })
    }

    async fn forget_calls(&self, wallets: &[&str]) {
        for w in wallets {
            sqlx::query(
                "DELETE FROM social.private_voice_chats WHERE caller_address = $1 OR callee_address = $1",
            )
            .bind(w)
            .execute(&self.pool)
            .await
            .unwrap();
        }
    }
}

fn drained(rx: &mut Receiver<Arc<SocialEvent>>) -> usize {
    std::iter::from_fn(|| rx.try_recv().ok()).count()
}

/// A reservation lost to a competing call is the busy conflict, and the
/// callee is never rung.
#[tokio::test]
async fn a_lost_reservation_is_busy_and_rings_no_one() {
    let Some(rig) = Rig::new().await else {
        return;
    };
    let _turn = VOICE_TABLE.lock().await;
    let (caller, callee) = (addr("vrr1"), addr("vrr2"));
    rig.runtime.lose_reservations.store(true, Ordering::SeqCst);
    let mut to_callee = rig.ctx.pubsub().subscribe(&callee);
    let answer = SocialServiceImpl
        .start_private_voice_chat(
            StartPrivateVoiceChatPayload {
                callee: Some(User {
                    address: callee.clone(),
                }),
            },
            procedure_ctx(&rig.ctx, &caller, 71),
        )
        .await
        .unwrap()
        .response;
    match answer {
        Some(start_private_voice_chat_response::Response::ConflictingError(e)) => assert_eq!(
            e.message.as_deref(),
            Some("One of the users is busy calling someone else")
        ),
        other => panic!("{other:?}"),
    }
    assert_eq!(drained(&mut to_callee), 0);
}

/// A grant the runtime cannot mint answers an internal error on accept, and
/// the caller hears no ACCEPTED.
#[tokio::test]
async fn a_failed_private_grant_is_an_internal_error_without_accepted() {
    let Some(rig) = Rig::new().await else {
        return;
    };
    let _turn = VOICE_TABLE.lock().await;
    let (caller, callee) = (addr("vrg1"), addr("vrg2"));
    let call_id = match SocialServiceImpl
        .start_private_voice_chat(
            StartPrivateVoiceChatPayload {
                callee: Some(User {
                    address: callee.clone(),
                }),
            },
            procedure_ctx(&rig.ctx, &caller, 72),
        )
        .await
        .unwrap()
        .response
    {
        Some(start_private_voice_chat_response::Response::Ok(ok)) => ok.call_id,
        other => panic!("{other:?}"),
    };
    rig.runtime.fail_grants.store(true, Ordering::SeqCst);
    let mut to_caller = rig.ctx.pubsub().subscribe(&caller);
    let answer = SocialServiceImpl
        .accept_private_voice_chat(
            AcceptPrivateVoiceChatPayload { call_id },
            procedure_ctx(&rig.ctx, &callee, 73),
        )
        .await
        .unwrap()
        .response;
    let heard = drained(&mut to_caller);
    // Cleaned up before asserting: a failure leaves no call row to the sweep.
    rig.forget_calls(&[&caller, &callee]).await;
    assert!(
        matches!(
            answer,
            Some(accept_private_voice_chat_response::Response::InternalServerError(_))
        ),
        "{answer:?}"
    );
    assert_eq!(heard, 0);
}

/// A community grant the runtime cannot mint answers an internal error on
/// start and on join (upstream "Failed to start community voice chat",
/// "Failed to join community voice chat"), and no member hears STARTED. The
/// join is tried on a live room: without one it is not found before any grant.
#[tokio::test]
async fn a_failed_community_grant_is_an_internal_error() {
    let Some(rig) = Rig::new().await else {
        return;
    };
    let (owner, member) = (addr("vrc1"), addr("vrc2"));
    let community = seed_community(&rig.pool, true, &owner, "owner").await;
    sqlx::query(
        "INSERT INTO social.community_members (community_id, member_address, role) VALUES ($1, $2, 'member')",
    )
    .bind(community)
    .bind(&member)
    .execute(&rig.pool)
    .await
    .unwrap();
    rig.runtime.fail_grants.store(true, Ordering::SeqCst);
    let mut to_member = rig.ctx.pubsub().subscribe(&member);
    let start = SocialServiceImpl
        .start_community_voice_chat(
            StartCommunityVoiceChatPayload {
                community_id: community.to_string(),
            },
            procedure_ctx(&rig.ctx, &owner, 74),
        )
        .await
        .unwrap()
        .response;
    assert!(
        matches!(
            &start,
            Some(start_community_voice_chat_response::Response::InternalServerError(e))
                if e.message.as_deref() == Some("Failed to start community voice chat")
        ),
        "{start:?}"
    );
    assert_eq!(drained(&mut to_member), 0);
    rig.runtime.inner.live.lock().insert(community);
    let join = SocialServiceImpl
        .join_community_voice_chat(
            JoinCommunityVoiceChatPayload {
                community_id: community.to_string(),
            },
            procedure_ctx(&rig.ctx, &member, 75),
        )
        .await
        .unwrap()
        .response;
    assert!(
        matches!(
            &join,
            Some(join_community_voice_chat_response::Response::InternalServerError(e))
                if e.message.as_deref() == Some("Failed to join community voice chat")
        ),
        "{join:?}"
    );
    drop_community(&rig.pool, community).await;
}

/// The actual upstream media adapter accepts incomplete maps. The domain deletes
/// the pending intent first, reads the caller grant before publishing ACCEPTED,
/// then reads the callee grant. HTTP hides the unexpected error; RPC preserves it.
#[tokio::test]
async fn incomplete_private_credentials_match_source_errors_and_committed_events() {
    use axum::{body::Body, http::Request};
    use tower::ServiceExt;
    let Some(rig) = Rig::new().await else {
        return;
    };
    let _turn = VOICE_TABLE.lock().await;
    let fixture: serde_json::Value = serde_json::from_str(include_str!(concat!(
        env!("DCL_ONE_ROOT"),
        "/services/catalyst/tests/fixtures/conformance/social/private-grant-source-cases.json"
    )))
    .unwrap();
    assert_eq!(
        fixture["baseline"]["revision"],
        "b6b2cefefdf727b2ed47ddb887e77d226a41eea6"
    );
    let source_caller = fixture["caller"].as_str().unwrap();
    let source_callee = fixture["target"].as_str().unwrap();
    let id: Uuid = fixture["call"].as_str().unwrap().parse().unwrap();
    let key = PrivateKeySigner::random();
    let http_callee = format!("{:#x}", key.address());
    let http_caller = addr("incomplete-grant-caller");
    let cases = fixture["cases"].as_array().unwrap();
    assert_eq!(cases.len(), 8);
    for source in cases {
        let is_http = source["protocol"] == "http";
        let (caller, callee) = if is_http {
            (&*http_caller, &*http_callee)
        } else {
            (source_caller, source_callee)
        };
        let case: serde_json::Value = serde_json::from_str(
            &source
                .to_string()
                .replace(source_caller, caller)
                .replace(source_callee, callee),
        )
        .unwrap();
        let label = case["id"].as_str().unwrap();
        let scenario = case["scenario"].as_str().unwrap();
        rig.forget_calls(&[caller, callee]).await;
        sqlx::query("INSERT INTO social.private_voice_chats(id,caller_address,callee_address) VALUES($1,$2,$3)")
            .bind(id).bind(caller).bind(callee).execute(&rig.pool).await.unwrap();
        rig.runtime.omit_caller.store(
            matches!(scenario, "missing-caller" | "missing-both"),
            Ordering::SeqCst,
        );
        rig.runtime.omit_callee.store(
            matches!(scenario, "missing-callee" | "missing-both"),
            Ordering::SeqCst,
        );
        let mut to_caller = rig.ctx.pubsub().subscribe(caller);
        let mut to_callee = rig.ctx.pubsub().subscribe(callee);
        if !is_http {
            let response = SocialServiceImpl
                .accept_private_voice_chat(
                    AcceptPrivateVoiceChatPayload {
                        call_id: id.to_string(),
                    },
                    procedure_ctx(&rig.ctx, callee, 78),
                )
                .await
                .unwrap();
            assert_eq!(
                hex::encode(response.encode_to_vec()),
                case["expected_hex"].as_str().unwrap(),
                "{label}"
            );
        } else {
            let app = crate::rpc::http::router(Arc::new(State::with_context(
                Settings::default(),
                rig.ctx.clone(),
                None,
            )));
            let path = format!("/v1/private-voice-chats/{id}");
            let timestamp = chrono::Utc::now().timestamp_millis();
            let payload = crate::shared::signed_fetch::build_payload_v6(
                "patch",
                &path,
                &timestamp.to_string(),
                "{}",
            );
            let (_, auth) = frame_signed(
                &key,
                &PrivateKeySigner::random(),
                FAR_EXPIRATION,
                timestamp,
                &payload,
                "{}",
            )
            .await;
            let mut request = Request::builder().method("PATCH").uri(&path);
            for (name, value) in
                serde_json::from_str::<serde_json::Map<String, serde_json::Value>>(&auth).unwrap()
            {
                request = request.header(name, value.as_str().unwrap());
            }
            let response = app
                .oneshot(
                    request
                        .header("content-type", "application/json")
                        .body(Body::from(r#"{"intention":"accepted"}"#))
                        .unwrap(),
                )
                .await
                .unwrap();
            let status = response.status().as_u16();
            let bytes = axum::body::to_bytes(response.into_body(), 1 << 20)
                .await
                .unwrap();
            let body: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
            assert_eq!(
                serde_json::json!({"status":status,"body":body}),
                case["response"],
                "{label}"
            );
        }
        let pending: i64 = sqlx::query_scalar("SELECT count(*) FROM social.private_voice_chats c WHERE id=$1 AND NOT EXISTS(SELECT 1 FROM social.private_voice_chat_participants p WHERE p.call_id=c.id)")
            .bind(id).fetch_one(&rig.pool).await.unwrap();
        assert_eq!(
            pending,
            case["remaining_rows"].as_i64().unwrap(),
            "{label}: committed pending deletion"
        );
        let received: Vec<_> = std::iter::from_fn(|| to_caller.try_recv().ok()).collect();
        let expected = case["updates"].as_array().unwrap();
        assert_eq!(received.len(), expected.len(), "{label}: caller events");
        assert_eq!(
            drained(&mut to_callee),
            0,
            "{label}: callee receives no ACCEPTED"
        );
        for (event, expected) in received.iter().zip(expected) {
            let SocialEvent::PrivateVoice(update) = &**event else {
                panic!("{label}: wrong event")
            };
            assert_eq!(
                update.status,
                PrivateVoiceChatStatus::VoiceChatAccepted as i32,
                "{label}"
            );
            assert_eq!(
                update.call_id,
                expected["callId"].as_str().unwrap(),
                "{label}"
            );
            assert_eq!(
                update.credentials.as_ref().unwrap().connection_url,
                expected["credentials"]["connectionUrl"].as_str().unwrap(),
                "{label}"
            );
        }
        rig.forget_calls(&[caller, callee]).await;
    }
}
