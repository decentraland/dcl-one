//! What a relay room admission means, room by room. The room name says the
//! purpose (`scene:<realm>:<id>`, `world-<name>`, `world-<name>-<id>`,
//! `island-<id>`) and the purpose says which store decides: a Genesis scene
//! room is the connection-ban predicate and the scene's ban list; a world
//! room is `Policy::can_access` (bans, the owner, the access setting) over
//! the world's active scene; an island room is the realm-lane fence: the
//! claims' `catalyrstIsland` owner (version 2) must be the wallet's current
//! owner session, audience, epoch, revision and fencing token for exactly
//! that island, then the platform-ban gate (`Unavailable` without the
//! fence, the fail-closed answer, never a grant). The `authoritative-server`
//! subject is admitted only with the configured authoritative wallet in its
//! claims and never into an island room.

use std::sync::Arc;

use async_trait::async_trait;
use serde::Deserialize;

use super::proof::VerifiedRoom;
use crate::access::{Access, SceneBanKey, WorldStanding};
use crate::gatekeeper::moderation::{BanStatus, admission_ban};
use crate::interfaces::{AccessPolicy, SceneIdentity};
use crate::protocol::room_authority::address_bytes;
use crate::world_identity::{SceneId, WorldName};
use crate::{Address, AuthorityNotEstablished};

use super::{DenialReason, RelayPolicy};
use crate::gatekeeper::admission::cluster::fence::{
    FenceError, RealmAssignmentReader, RealmAssignmentSnapshot,
};

pub struct CurrentRoomPolicy {
    scenes: Arc<dyn SceneIdentity>,
    policy: Arc<dyn AccessPolicy>,
    bans: Arc<dyn BanStatus>,
    authoritative_server_address: Option<Address>,
    assignments: Option<Arc<dyn RealmAssignmentReader>>,
}

impl CurrentRoomPolicy {
    pub fn new(
        scenes: Arc<dyn SceneIdentity>,
        policy: Arc<dyn AccessPolicy>,
        bans: Arc<dyn BanStatus>,
        authoritative_server_address: Option<Address>,
    ) -> Self {
        Self {
            scenes,
            policy,
            bans,
            authoritative_server_address,
            assignments: None,
        }
    }

    /// The realm-lane fence island rooms are decided by.
    pub fn with_assignments(mut self, assignments: Option<Arc<dyn RealmAssignmentReader>>) -> Self {
        self.assignments = assignments;
        self
    }

    /// The connection-ban predicate and the scene's ban list for `principal`.
    async fn scene_gate(
        &self,
        principal: &Address,
        scene: &SceneBanKey,
    ) -> Result<(), DenialReason> {
        let now = chrono::Utc::now().naive_utc();
        match admission_ban(
            self.policy.as_ref(),
            self.bans.as_ref(),
            principal,
            Some(scene),
            None,
            now,
        )
        .await
        {
            Ok(None) => Ok(()),
            Ok(Some(_)) => Err(DenialReason::NotAuthorized),
            Err(_) => Err(DenialReason::Unavailable),
        }
    }

    async fn platform_gate(&self, wallet: &Address) -> Result<(), DenialReason> {
        let now = chrono::Utc::now().naive_utc();
        match admission_ban(
            self.policy.as_ref(),
            self.bans.as_ref(),
            wallet,
            None,
            None,
            now,
        )
        .await
        {
            Ok(None) => Ok(()),
            Ok(Some(_)) => Err(DenialReason::NotAuthorized),
            Err(_) => Err(DenialReason::Unavailable),
        }
    }

    async fn world_gate(
        &self,
        principal: &Address,
        world: &WorldName,
        scene: &SceneBanKey,
    ) -> Result<(), DenialReason> {
        match self.policy.world_standing(world).await {
            Ok(WorldStanding::Open(_)) => {}
            Ok(_) | Err(AuthorityNotEstablished::RefusedLacksAuthority { .. }) => {
                return Err(DenialReason::NotAuthorized);
            }
            Err(_) => return Err(DenialReason::Unavailable),
        }
        let now = chrono::Utc::now().naive_utc();
        match self
            .policy
            .can_access(world, principal, None, Some(scene), None, now)
            .await
        {
            Ok(Access::Allowed(_)) => {}
            Ok(Access::Banned(_) | Access::NotAllowed | Access::InvalidSecret)
            | Err(AuthorityNotEstablished::RefusedLacksAuthority { .. }) => {
                return Err(DenialReason::NotAuthorized);
            }
            Err(_) => return Err(DenialReason::Unavailable),
        }
        let by_name = SceneBanKey::scene(
            &SceneId::parse(world.key()).map_err(|_| DenialReason::NotAuthorized)?,
        );
        self.scene_gate(principal, &by_name).await
    }
}

#[async_trait]
impl RelayPolicy for CurrentRoomPolicy {
    async fn authorize(&self, room: &VerifiedRoom) -> Result<(), DenialReason> {
        let scope = scope(&room.claims.video.room).ok_or(DenialReason::NotAuthorized)?;
        let metadata: serde_json::Value = if room.claims.metadata.is_empty() {
            serde_json::Value::Null
        } else {
            serde_json::from_str(&room.claims.metadata).map_err(|_| DenialReason::Invalid)?
        };
        if !matches!(scope, Scope::Island(_)) && metadata.get("catalyrstIsland").is_some() {
            return Err(DenialReason::NotAuthorized);
        }
        let wallet = Address::parse(&room.wallet).map_err(|_| DenialReason::Invalid)?;
        let principal = if room.claims.sub == "authoritative-server" {
            if matches!(scope, Scope::Island(_)) {
                return Err(DenialReason::NotAuthorized);
            }
            let expected = self
                .authoritative_server_address
                .as_ref()
                .ok_or(DenialReason::NotAuthorized)?;
            let author: AuthoritativeServer = serde_json::from_value(
                metadata
                    .get("catalyrstAuthoritativeServer")
                    .cloned()
                    .ok_or(DenialReason::NotAuthorized)?,
            )
            .map_err(|_| DenialReason::NotAuthorized)?;
            if address_bytes(&author.wallet) != address_bytes(expected.as_str()) {
                return Err(DenialReason::NotAuthorized);
            }
            expected.clone()
        } else {
            if metadata.get("catalyrstAuthoritativeServer").is_some() {
                return Err(DenialReason::NotAuthorized);
            }
            wallet.clone()
        };
        if principal != wallet {
            self.platform_gate(&wallet).await?;
        }
        match scope {
            Scope::Island(island) => {
                let owner: IslandMetadata =
                    serde_json::from_value(metadata).map_err(|_| DenialReason::NotAuthorized)?;
                let reader = self.assignments.as_ref().ok_or(DenialReason::Unavailable)?;
                let current = reader
                    .current_realm_assignment(&room.wallet, &room.session)
                    .await
                    .map_err(|error| match error {
                        FenceError::Unavailable => DenialReason::Unavailable,
                        _ => DenialReason::NotAuthorized,
                    })?;
                if !owner.catalyrst_island.matches(room, island, &current) {
                    return Err(DenialReason::NotAuthorized);
                }
                self.platform_gate(&wallet).await
            }
            Scope::WorldScene(name) => {
                let resolved = crate::rooms::resolve_room_name(self.scenes.as_ref(), name)
                    .await
                    .map_err(|_| DenialReason::Unavailable)?
                    .ok_or(DenialReason::NotAuthorized)?;
                match resolved.realm.world() {
                    Some(world) => {
                        let scene = match resolved.ban_key() {
                            Some(scene) => scene,
                            None => SceneBanKey::world(
                                world,
                                self.scenes
                                    .world_active_scene(world)
                                    .await
                                    .map_err(|_| DenialReason::Unavailable)?
                                    .ok_or(DenialReason::NotAuthorized)?
                                    .base,
                            ),
                        };
                        self.world_gate(&principal, world, &scene).await
                    }
                    None => {
                        self.scene_gate(
                            &principal,
                            &resolved.ban_key().ok_or(DenialReason::NotAuthorized)?,
                        )
                        .await
                    }
                }
            }
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct IslandMetadata {
    catalyrst_island: IslandOwner,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct IslandOwner {
    version: u8,
    wallet: String,
    session: String,
    audience: String,
    lane: String,
    owner_epoch: u64,
    assignment_revision: u64,
    fencing_token: u64,
}

impl IslandOwner {
    fn matches(
        &self,
        room: &VerifiedRoom,
        island: &str,
        current: &RealmAssignmentSnapshot,
    ) -> bool {
        self.version == 2
            && address_bytes(&self.wallet) == address_bytes(&room.wallet)
            && address_bytes(&self.session) == address_bytes(&room.session)
            && current.wallet == room.wallet
            && current.owner_session == room.session
            && self.audience == current.audience
            && self.lane == "realm"
            && self.lane == current.lane
            && self.owner_epoch > 0
            && self.owner_epoch == current.owner_epoch
            && self.assignment_revision > 0
            && self.assignment_revision == current.assignment_revision
            && self.fencing_token > 0
            && self.fencing_token == current.fencing_token
            && island == current.island_id
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AuthoritativeServer {
    wallet: String,
}

use crate::rooms::{RoomScope as Scope, room_scope as scope};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn room_scope_only_prefilters_purpose_without_selecting_identity() {
        assert_eq!(scope("island-123"), Some(Scope::Island("island-123")));
        for name in [
            "scene:main:cid-123",
            "world-foo.eth",
            "world-foo.eth-cid",
            "world-foo.eth-../cid",
            "scene::cid",
        ] {
            assert_eq!(scope(name), Some(Scope::WorldScene(name)));
        }
        for name in ["island-", "unknown", "voice-chat-private-1"] {
            assert_eq!(scope(name), None);
        }
    }
}

#[cfg(test)]
mod moderation_boundary_tests {
    use super::*;
    use crate::gatekeeper::moderation::test_support::StubBanStatus;
    use crate::interfaces::test_support::{StubPolicy, StubScenes};
    #[tokio::test]
    async fn relay_keeps_platform_scene_and_authority_fault_denials() {
        let actor = Address::parse("0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa").unwrap();
        let scene = SceneBanKey::scene(&SceneId::parse("test-scene").unwrap());
        for ban in [crate::access::Ban::Platform, crate::access::Ban::Scene] {
            let policy = CurrentRoomPolicy::new(
                Arc::new(StubScenes::default()),
                Arc::new(StubPolicy::default()),
                Arc::new(StubBanStatus(Ok(Some(ban)))),
                None,
            );
            assert_eq!(
                policy.scene_gate(&actor, &scene).await,
                Err(DenialReason::NotAuthorized)
            );
        }
        let unavailable = CurrentRoomPolicy::new(
            Arc::new(StubScenes::default()),
            Arc::new(StubPolicy::default()),
            Arc::new(StubBanStatus(Err(
                AuthorityNotEstablished::store_unavailable(
                    "social.user_bans",
                    &sqlx::Error::PoolClosed,
                ),
            ))),
            None,
        );
        assert_eq!(
            unavailable.scene_gate(&actor, &scene).await,
            Err(DenialReason::Unavailable)
        );
        assert_eq!(
            unavailable.platform_gate(&actor).await,
            Err(DenialReason::Unavailable)
        );
    }
}
