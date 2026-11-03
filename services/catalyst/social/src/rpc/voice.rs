//! Test voice transport backed by comms-owned call persistence.

/// A runtime that mints a recognisable URL per grant and records every call;
/// the tests drive the voice RPCs through it.
#[cfg(test)]
pub mod fake {
    use std::collections::HashMap;

    use async_trait::async_trait;
    use parking_lot::Mutex;
    use uuid::Uuid;

    use crate::comms::gatekeeper::voice::rooms::{
        CommunityVoiceAction, VoiceRooms, VoiceUnavailable,
    };

    pub struct FakeVoiceRooms {
        db: crate::comms::gatekeeper::voice::db::VoiceDb,
        pub calls: Mutex<Vec<String>>,
        /// Addresses the live room does not hold: an action on one is the
        /// runtime's not-found.
        pub absent: Mutex<Vec<String>>,
        /// The communities whose room is live: a `Create` grant opens one,
        /// an end closes it.
        pub live: Mutex<std::collections::HashSet<Uuid>>,
        /// The profile each community grant carried, by address.
        pub profiles: Mutex<HashMap<String, Option<serde_json::Value>>>,
        /// Every room action (end, speak requests, speakers, kick, mute)
        /// fails as a runtime error, not a not-found.
        pub fail_actions: std::sync::atomic::AtomicBool,
        /// A wallet banned from the community while its join grant is
        /// minted: the race a join re-reads the ban for.
        pub ban_during_grant: Mutex<Option<String>>,
        pool: sqlx::PgPool,
    }

    impl FakeVoiceRooms {
        pub fn new(pool: sqlx::PgPool) -> Self {
            Self {
                db: crate::comms::gatekeeper::voice::db::VoiceDb::new(
                    pool.clone(),
                    Default::default(),
                ),
                ban_during_grant: Mutex::new(None),
                pool,
                calls: Mutex::new(Vec::new()),
                absent: Mutex::new(Vec::new()),
                live: Mutex::new(Default::default()),
                profiles: Mutex::new(HashMap::new()),
                fail_actions: Default::default(),
            }
        }

        fn note(&self, line: String) {
            self.calls.lock().push(line);
        }

        fn present(&self, address: &str) -> Result<(), VoiceUnavailable> {
            if self.fail_actions.load(std::sync::atomic::Ordering::SeqCst) {
                return Err(VoiceUnavailable::Failed("test runtime failed".into()));
            }
            if self.absent.lock().iter().any(|a| a == address) {
                Err(VoiceUnavailable::NotFound)
            } else {
                Ok(())
            }
        }
    }

    #[async_trait]
    impl VoiceRooms for FakeVoiceRooms {
        fn presence_config(&self) -> crate::comms::gatekeeper::voice::db::VoiceDbConfig {
            self.db.config()
        }
        async fn create_pending_private_call(
            &self,
            caller: &str,
            callee: &str,
            expiration_ms: i64,
        ) -> Result<crate::comms::gatekeeper::voice::db::StartOutcome, VoiceUnavailable> {
            Ok(self
                .db
                .create_pending_private_call(caller, callee, expiration_ms)
                .await?)
        }
        async fn finish_pending_private_call(
            &self,
            call: crate::comms::gatekeeper::voice::db::PendingPrivateCall,
        ) -> Result<
            Option<crate::comms::gatekeeper::voice::db::PrivateVoiceChatRow>,
            VoiceUnavailable,
        > {
            Ok(self.db.finish_pending_private_call(call).await?)
        }
        async fn expire_pending_private_calls(
            &self,
            expiration_ms: i64,
            limit: i64,
        ) -> Result<Vec<crate::comms::gatekeeper::voice::db::PrivateVoiceChatRow>, VoiceUnavailable>
        {
            Ok(self
                .db
                .expire_pending_private_calls(expiration_ms, limit)
                .await?)
        }

        async fn private_voice_credentials(
            &self,
            call_id: Uuid,
            callee: &str,
            caller: &str,
        ) -> Result<HashMap<String, String>, VoiceUnavailable> {
            self.note(format!("private-credentials {call_id} {callee} {caller}"));
            let caller_address = crate::shared::identity::Address::parse(caller)
                .map_err(|e| VoiceUnavailable::Failed(e.to_string()))?;
            let callee_address = crate::shared::identity::Address::parse(callee)
                .map_err(|e| VoiceUnavailable::Failed(e.to_string()))?;
            if !self
                .db
                .accept_pending_private_call(call_id, &caller_address, &callee_address)
                .await?
            {
                return Err(VoiceUnavailable::NotFound);
            }
            Ok([callee, caller]
                .into_iter()
                .map(|a| {
                    (
                        a.to_string(),
                        format!("wss://livekit.test/voice-chat-private-{call_id}?for={a}"),
                    )
                })
                .collect())
        }

        /// Records the ask and ends an accepted call through the same persistence owner.
        async fn end_private_voice_chat(
            &self,
            call_id: Option<Uuid>,
            address: &str,
        ) -> Result<
            Option<crate::comms::gatekeeper::voice::db::PrivateVoiceChatRow>,
            VoiceUnavailable,
        > {
            let call = call_id.map_or_else(|| "latest".to_string(), |id| id.to_string());
            self.note(format!("end-private {call} {address}"));
            let address = crate::shared::identity::Address::parse(address)
                .map_err(|e| VoiceUnavailable::Failed(e.to_string()))?;
            match self
                .db
                .delete_private_call_if_member(call_id, &address)
                .await
            {
                Ok(ended) => Ok(Some(ended.call)),
                Err(crate::comms::gatekeeper::voice::db::DeleteRoomError::RoomDoesNotExist) => {
                    Ok(None)
                }
                Err(crate::comms::gatekeeper::voice::db::DeleteRoomError::Db(error)) => {
                    Err(error.into())
                }
            }
        }

        async fn community_voice_credentials(
            &self,
            community_id: Uuid,
            address: &str,
            role: &str,
            action: CommunityVoiceAction,
            profile: Option<serde_json::Value>,
        ) -> Result<String, VoiceUnavailable> {
            self.note(format!(
                "community-credentials {community_id} {address} {role} {}",
                action.as_str()
            ));
            self.profiles.lock().insert(address.to_string(), profile);
            let racing_ban = self.ban_during_grant.lock().take();
            if let Some(banned) = racing_ban.filter(|b| b == address) {
                sqlx::query("INSERT INTO social.community_bans (community_id, banned_address, banned_by) VALUES ($1, $2, $2)")
                    .bind(community_id)
                    .bind(&banned)
                    .execute(&self.pool)
                    .await?;
            }
            if action == CommunityVoiceAction::Create {
                self.live.lock().insert(community_id);
            }
            Ok(format!(
                "wss://livekit.test/voice-chat-community-{community_id}?for={address}"
            ))
        }

        async fn community_voice_chat_active(
            &self,
            community_id: Uuid,
        ) -> Result<bool, VoiceUnavailable> {
            Ok(self.live.lock().contains(&community_id))
        }

        async fn end_community_voice_chat(
            &self,
            community_id: Uuid,
            address: &str,
        ) -> Result<(), VoiceUnavailable> {
            self.note(format!("end-community {community_id} {address}"));
            if self.fail_actions.load(std::sync::atomic::Ordering::SeqCst) {
                return Err(VoiceUnavailable::Failed("test runtime failed".into()));
            }
            self.live.lock().remove(&community_id);
            Ok(())
        }

        async fn request_to_speak(
            &self,
            community_id: Uuid,
            address: &str,
            raising_hand: bool,
        ) -> Result<(), VoiceUnavailable> {
            self.note(format!(
                "request-to-speak {community_id} {address} {raising_hand}"
            ));
            self.present(address)
        }

        async fn reject_speak_request(
            &self,
            community_id: Uuid,
            address: &str,
        ) -> Result<(), VoiceUnavailable> {
            self.note(format!("reject-speak-request {community_id} {address}"));
            self.present(address)
        }

        async fn set_speaker(
            &self,
            community_id: Uuid,
            address: &str,
            promote: bool,
        ) -> Result<(), VoiceUnavailable> {
            self.note(format!("set-speaker {community_id} {address} {promote}"));
            self.present(address)
        }

        async fn kick_player(
            &self,
            community_id: Uuid,
            address: &str,
        ) -> Result<(), VoiceUnavailable> {
            self.note(format!("kick {community_id} {address}"));
            self.present(address)
        }

        async fn mute_speaker(
            &self,
            community_id: Uuid,
            address: &str,
            muted: bool,
        ) -> Result<(), VoiceUnavailable> {
            self.note(format!("mute {community_id} {address} {muted}"));
            self.present(address)
        }
    }
}
