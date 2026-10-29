//! The production [`ClusterGateway`]: the platform ban store and the LiveKit server API of
//! the gatekeeper's state (`gatekeeper::CommsState`).

use async_trait::async_trait;

use crate::Address;
use crate::gatekeeper::SharedState;
use crate::gatekeeper::admission::cluster::fence::TokenFence;
use crate::gatekeeper::admission::cluster::subscriber::{
    ClusterGateway, GatewayError, IslandOccupant,
};
use crate::gatekeeper::livekit::Removal;

mod island;
pub use island::{LivekitCredentials, island_occupant, mint_island_connection_string};

pub struct StateGateway {
    state: SharedState,
}

impl StateGateway {
    pub fn new(state: SharedState) -> Self {
        Self { state }
    }

    async fn checked_lease(
        &self,
        admission: &super::fence::PulseAdmissionStamp,
        fence: Option<&TokenFence>,
        unowned: bool,
    ) -> Result<Option<super::safety::SafetyLease>, GatewayError> {
        if !self.state.verify_island_stamp(admission).await {
            return Err(GatewayError::Unknown);
        }
        self.state
            .island_safety
            .authorize_stamp(
                admission,
                self.state.cluster.control_v4_audience.as_deref(),
                fence,
                unowned,
            )
            .await
            .map(Some)
    }

    fn credentials(&self) -> LivekitCredentials<'_> {
        LivekitCredentials {
            api_key: &self.state.livekit.api_key,
            api_secret: &self.state.livekit.api_secret,
            ws_url: &self.state.livekit.client_url,
        }
    }
}

#[async_trait]
impl ClusterGateway for StateGateway {
    fn safety_store(&self) -> Option<super::safety::SafetyStore> {
        Some(self.state.island_safety.clone())
    }
    async fn ensure_recovery_room(&self, room: &str) -> Result<(), GatewayError> {
        self.state
            .room_service()
            .ensure_cutoff_room(room)
            .await
            .map_err(|_| GatewayError::Unknown)
    }
    async fn is_denied(&self, wallet: &str) -> Result<bool, GatewayError> {
        let wallet = Address::parse(wallet).map_err(|e| GatewayError::new(e.to_string()))?;
        crate::gatekeeper::moderation::admission_ban(
            self.state.policy.as_ref(),
            self.state.bans.as_ref(),
            &wallet,
            None,
            None,
            chrono::Utc::now().naive_utc(),
        )
        .await
        .map(|ban| ban.is_some())
        .map_err(|e| GatewayError::new(e.to_string()))
    }

    async fn mint_connection_string(
        &self,
        _admission: &crate::gatekeeper::admission::cluster::fence::PulseAdmissionStamp,
        wallet: &str,
        session: Option<&str>,
        room: &str,
        ttl_seconds: u64,
        not_before_unix: Option<u64>,
    ) -> Result<String, GatewayError> {
        if ttl_seconds > super::MAX_ISLAND_TOKEN_TTL_SECONDS
            || _admission.wallet != wallet
            || Some(_admission.session.as_str()) != session
            || crate::rooms::island_room_name(&_admission.cluster_id) != room
            || not_before_unix != Some(_admission.credential_not_before)
        {
            return Err(GatewayError::Unknown);
        }
        let lease = self.checked_lease(_admission, None, true).await?;
        let result = mint_island_connection_string(
            self.credentials(),
            wallet,
            session,
            room,
            ttl_seconds,
            not_before_unix,
        );
        if let Some(lease) = lease {
            lease.commit().await?;
        }
        result
    }

    async fn mint_fenced_connection_string(
        &self,
        _admission: &crate::gatekeeper::admission::cluster::fence::PulseAdmissionStamp,
        wallet: &str,
        room: &str,
        ttl_seconds: u64,
        fence: &TokenFence,
        not_before_unix: Option<u64>,
    ) -> Result<String, GatewayError> {
        if ttl_seconds > super::MAX_ISLAND_TOKEN_TTL_SECONDS
            || _admission.wallet != wallet
            || _admission.session != fence.owner_session
            || crate::rooms::island_room_name(&_admission.cluster_id) != room
            || not_before_unix != Some(_admission.credential_not_before)
        {
            return Err(GatewayError::Unknown);
        }
        let lease = self.checked_lease(_admission, Some(fence), false).await?;
        let result = island::mint_fenced_island_connection_string(
            self.credentials(),
            wallet,
            room,
            ttl_seconds,
            fence,
            not_before_unix,
        );
        if let Some(lease) = lease {
            lease.commit().await?;
        }
        result
    }

    async fn remove_participant(
        &self,
        room: &str,
        identity: &str,
        revoke_tokens_minted_before: Option<i64>,
    ) -> Result<Removal, GatewayError> {
        self.state
            .room_service()
            .remove_participant_with_cutoff(room, identity, revoke_tokens_minted_before)
            .await
            .map_err(|error| match error {
                crate::gatekeeper::livekit::RoomServiceError::RemovalRoomMissing => {
                    GatewayError::RoomMissing
                }
                crate::gatekeeper::livekit::RoomServiceError::RemovalRejected => {
                    GatewayError::DefiniteRejection
                }
                _ => GatewayError::Unknown,
            })
    }

    async fn holds_participant(&self, room: &str, identity: &str) -> Result<bool, GatewayError> {
        self.state
            .room_service()
            .holds_participant(room, identity)
            .await
            .map_err(|e| GatewayError::new(e.to_string()))
    }

    async fn island_occupant(
        &self,
        room: &str,
        wallet: &str,
    ) -> Result<IslandOccupant, GatewayError> {
        island_occupant(&self.state.room_service(), room, wallet).await
    }
}

#[cfg(test)]
mod moderation_boundary_tests {
    use super::*;
    use crate::gatekeeper::moderation::test_support::{StubBanStatus, state};
    use crate::interfaces::test_support::StubPolicy;
    use std::sync::Arc;
    #[tokio::test]
    async fn operator_block_denies_cluster_gateway_before_token_mint() {
        let gateway = StateGateway::new(Arc::new(state(
            StubPolicy {
                blocked: Ok(true),
                ..Default::default()
            },
            StubBanStatus(Err(crate::AuthorityNotEstablished::store_unavailable(
                "unexpected raw read",
                &sqlx::Error::PoolClosed,
            ))),
        )));
        assert!(
            gateway
                .is_denied("0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")
                .await
                .unwrap()
        );
    }
    #[tokio::test]
    async fn gateway_keeps_authority_fault_distinct_for_subscriber_failure_policy() {
        let gateway = StateGateway::new(Arc::new(state(
            StubPolicy {
                blocked: Err(crate::AuthorityNotEstablished::store_unavailable(
                    "worlds.blocked_wallets",
                    &sqlx::Error::PoolClosed,
                )),
                ..Default::default()
            },
            StubBanStatus::default(),
        )));
        assert!(
            gateway
                .is_denied("0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")
                .await
                .is_err()
        );
    }
}
