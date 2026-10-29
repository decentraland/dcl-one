//! The traits `catalyst-comms` needs from the crates above it: comms declares
//! the question, the higher layer answers it, and `serve` hands the answer in.
//!
//! - [`AccessPolicy`]: the worlds permission core (who may join which world or
//!   scene, who is operator-blocked, who administers a scene, whom a scene ban may not
//!   target). Implemented by worlds' `policy::Policy`; the archipelago
//!   connector's ban check and the gatekeeper take it.
//! - [`SceneIdentity`]: the ONE room resolver over worlds' and content's
//!   rows (a request to its active deployment and room). Implemented by
//!   worlds' `rooms::SceneResolver`; the gatekeeper and its relay authority
//!   take it.

use std::collections::BTreeSet;
use std::sync::Arc;

use chrono::NaiveDateTime;

use crate::access::{Access, AccessType, LandOperators, SceneRef};
use crate::rooms::{ActiveDeployment, ResolveError, ResolvedRoom, RoomRequest};
use crate::shared::identity::{Address, AuthorityNotEstablished, PermissionDecision};
use crate::world_identity::{Parcel, ParcelSet, SceneId, WorldName};

/// The worlds permission core as comms asks it; every method is the worlds
/// `Policy` method of the same name.
#[async_trait::async_trait]
pub trait AccessPolicy: Send + Sync {
    /// The access setting of `world`, `None` when it is unknown.
    async fn access_type(
        &self,
        world: &WorldName,
    ) -> Result<Option<AccessType>, AuthorityNotEstablished>;
    /// The stored owner of a known world row, not a fresh chain ownership verdict.
    /// An unknown world or unset owner is `None`; failed reads remain unavailable.
    async fn world_owner(
        &self,
        world: &WorldName,
    ) -> Result<Option<Address>, AuthorityNotEstablished>;
    /// The LAND manager document of a Genesis parcel, `None` without a reader.
    async fn land_document(
        &self,
        parcel: Parcel,
    ) -> Result<Option<Arc<Option<LandOperators>>>, AuthorityNotEstablished>;
    /// Whether `actor` may join `world` (and `scene` in it).
    async fn can_access(
        &self,
        world: &WorldName,
        actor: &Address,
        secret: Option<&str>,
        scene: Option<&SceneId>,
        device_id: Option<&str>,
        now: NaiveDateTime,
    ) -> Result<Access, AuthorityNotEstablished>;
    /// Whether the worlds operator block list holds `actor` out.
    async fn is_blocked(&self, actor: &Address) -> Result<bool, AuthorityNotEstablished>;
    /// Whether `actor` administers `scene`.
    async fn is_admin(&self, scene: &SceneRef<'_>, actor: &Address) -> PermissionDecision;
    /// The owner and every deployment/streaming grantee of `world` whose
    /// grant is world-wide or touches `parcels`: the wallets a scene ban may
    /// not target.
    async fn protected_addresses(
        &self,
        world: &WorldName,
        parcels: &ParcelSet,
    ) -> Result<BTreeSet<Address>, AuthorityNotEstablished>;
}

/// The ONE room resolver as comms asks it; every method is the worlds
/// `rooms` function of the same name over the ONE pool.
#[async_trait::async_trait]
pub trait SceneIdentity: Send + Sync {
    /// Classify complete room bytes. Refusal is distinct from unrelated rooms;
    /// catalogue failures remain errors. Implementations must not infer lobbies.
    async fn classify_room(
        &self,
        room: &str,
    ) -> Result<crate::rooms::RoomClassification, ResolveError>;

    /// Resolves `request`: the room and, when this node
    /// holds one, the active deployment behind it.
    async fn resolve(&self, request: RoomRequest<'_>) -> Result<ResolvedRoom, ResolveError>;
    /// A named scene of a world, when deployed there.
    async fn world_scene(
        &self,
        world: &WorldName,
        scene: &SceneId,
    ) -> Result<Option<ActiveDeployment>, ResolveError>;
    /// The world's active scene, `None` when the world has no scene here.
    async fn world_active_scene(
        &self,
        world: &WorldName,
    ) -> Result<Option<ActiveDeployment>, ResolveError>;
}

/// Name ownership is supplied by Shop, which owns both its NFT index and
/// the configured subgraph fallback. Missing evidence is an error; a known
/// absent name is `Ok(None)`.
#[async_trait::async_trait]
pub trait NameOwnerReader: Send + Sync {
    async fn name_owner(&self, name: &str) -> Result<Option<Address>, String>;
}

/// A streaming lifecycle notification, retried with the same event key.
#[derive(Clone, Debug)]
pub struct StreamingNotification {
    pub event_key: String,
    pub recipient: Address,
    pub kind: StreamingNotificationKind,
    pub metadata: serde_json::Value,
    pub occurred_at: NaiveDateTime,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StreamingNotificationKind {
    KeyReset,
    KeyRevoke,
    KeyExpired,
    TimeExceeded,
    PlaceUpdated,
}

impl StreamingNotificationKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::KeyReset => "streaming_key_reset",
            Self::KeyRevoke => "streaming_key_revoke",
            Self::KeyExpired => "streaming_key_expired",
            Self::TimeExceeded => "streaming_time_exceeded",
            Self::PlaceUpdated => "streaming_place_updated",
        }
    }
}

/// One accepted moderation transition. `source_id` names its durable row/version,
/// so retrying notification persistence cannot create another recipient notice.
#[derive(Clone, Debug)]
pub struct ModerationNotification {
    pub source_id: String,
    pub recipient: Address,
    pub occurred_at: NaiveDateTime,
    pub event: ModerationNotificationKind,
}

#[derive(Clone, Debug)]
pub enum ModerationNotificationKind {
    Banned {
        reason: String,
        banned_at: i64,
        expires_at: Option<i64>,
        custom_message: Option<String>,
    },
    BanLifted {
        lifted_at: i64,
    },
    BanWarning {
        reason: String,
        warned_at: i64,
    },
    UserBannedFromScene {
        place_title: String,
    },
    UserUnbannedFromScene {
        place_title: String,
    },
}

/// Social owns membership, privacy and notification storage.
#[async_trait::async_trait]
pub trait SocialDirectory: Send + Sync {
    async fn community_members(
        &self,
        community: crate::CommunityId,
    ) -> Result<Vec<Address>, sqlx::Error>;
    async fn private_messages_privacy(&self, address: &Address) -> Result<String, sqlx::Error>;
    async fn set_private_messages_privacy(
        &self,
        address: &Address,
        privacy: &str,
    ) -> Result<(), sqlx::Error>;
    async fn notify_community_voice_ended(
        &self,
        community: crate::CommunityId,
        event_key: &str,
        event: &serde_json::Value,
        occurred_at: NaiveDateTime,
    ) -> Result<(), sqlx::Error>;
    async fn notify_streaming_notification(
        &self,
        notification: &StreamingNotification,
    ) -> Result<(), sqlx::Error>;
    /// Insert only the in-app notice in the moderation owner's transaction.
    /// Implementations must perform no network work or implicit email delivery.
    async fn notify_moderation_notification(
        &self,
        transaction: &mut sqlx::PgConnection,
        notification: &ModerationNotification,
    ) -> Result<(), sqlx::Error>;
}

#[cfg(any(test, feature = "test-support"))]
pub mod test_support {
    use super::*;
    use crate::access::AccessGrant;

    /// In-memory social seam; records notifications for assertions.
    #[derive(Default)]
    pub struct StubDirectory {
        pub privacy: std::sync::Mutex<std::collections::BTreeMap<Address, String>>,
        pub streaming_notifications: std::sync::Mutex<Vec<StreamingNotification>>,
        pub community_events: std::sync::Mutex<Vec<serde_json::Value>>,
        pub fail_community_notifications: std::sync::atomic::AtomicUsize,
        pub fail_moderation_notifications: std::sync::atomic::AtomicBool,
    }

    #[async_trait::async_trait]
    impl SocialDirectory for StubDirectory {
        async fn notify_moderation_notification(
            &self,
            _: &mut sqlx::PgConnection,
            _: &ModerationNotification,
        ) -> Result<(), sqlx::Error> {
            if self
                .fail_moderation_notifications
                .load(std::sync::atomic::Ordering::SeqCst)
            {
                Err(sqlx::Error::Protocol(
                    "injected moderation notification persistence failure".into(),
                ))
            } else {
                Ok(())
            }
        }
        async fn community_members(
            &self,
            _: crate::CommunityId,
        ) -> Result<Vec<Address>, sqlx::Error> {
            Ok(Vec::new())
        }
        async fn private_messages_privacy(&self, address: &Address) -> Result<String, sqlx::Error> {
            Ok(self
                .privacy
                .lock()
                .unwrap()
                .get(address)
                .cloned()
                .unwrap_or_else(|| "all".to_string()))
        }
        async fn set_private_messages_privacy(
            &self,
            address: &Address,
            privacy: &str,
        ) -> Result<(), sqlx::Error> {
            self.privacy
                .lock()
                .unwrap()
                .insert(address.clone(), privacy.to_string());
            Ok(())
        }
        async fn notify_community_voice_ended(
            &self,
            _: crate::CommunityId,
            _: &str,
            event: &serde_json::Value,
            _: NaiveDateTime,
        ) -> Result<(), sqlx::Error> {
            use std::sync::atomic::Ordering;
            if self
                .fail_community_notifications
                .try_update(Ordering::SeqCst, Ordering::SeqCst, |remaining| {
                    remaining.checked_sub(1)
                })
                .is_ok()
            {
                return Err(sqlx::Error::Protocol(
                    "injected notification persistence failure".into(),
                ));
            }
            let mut events = self.community_events.lock().unwrap();
            if !events.iter().any(|saved| saved["key"] == event["key"]) {
                events.push(event.clone());
            }
            Ok(())
        }
        async fn notify_streaming_notification(
            &self,
            notification: &StreamingNotification,
        ) -> Result<(), sqlx::Error> {
            let mut notifications = self.streaming_notifications.lock().unwrap();
            if !notifications.iter().any(|n| {
                n.event_key == notification.event_key && n.recipient == notification.recipient
            }) {
                notifications.push(notification.clone());
            }
            Ok(())
        }
    }

    /// An [`AccessPolicy`] that gives every question the answer its field
    /// holds. The default is an unknown world, access allowed, not blocked, not
    /// an admin, no LAND document and no protected wallet.
    pub struct StubPolicy {
        pub access_type: Result<Option<AccessType>, AuthorityNotEstablished>,
        pub world_owner: Result<Option<Address>, AuthorityNotEstablished>,
        pub access: Result<Access, AuthorityNotEstablished>,
        pub blocked: Result<bool, AuthorityNotEstablished>,
        pub admin: PermissionDecision,
        pub land: Result<Option<Arc<Option<LandOperators>>>, AuthorityNotEstablished>,
        pub protected: Result<BTreeSet<Address>, AuthorityNotEstablished>,
    }

    impl Default for StubPolicy {
        fn default() -> Self {
            Self {
                access_type: Ok(None),
                world_owner: Ok(None),
                access: Ok(Access::Allowed(AccessGrant::Unrestricted)),
                blocked: Ok(false),
                admin: PermissionDecision::refused("stub: not an admin"),
                land: Ok(None),
                protected: Ok(BTreeSet::new()),
            }
        }
    }

    #[async_trait::async_trait]
    impl AccessPolicy for StubPolicy {
        async fn access_type(
            &self,
            _world: &WorldName,
        ) -> Result<Option<AccessType>, AuthorityNotEstablished> {
            self.access_type.as_ref().copied().map_err(copy)
        }

        async fn world_owner(
            &self,
            _world: &WorldName,
        ) -> Result<Option<Address>, AuthorityNotEstablished> {
            self.world_owner.as_ref().cloned().map_err(copy)
        }

        async fn land_document(
            &self,
            _parcel: Parcel,
        ) -> Result<Option<Arc<Option<LandOperators>>>, AuthorityNotEstablished> {
            self.land.as_ref().cloned().map_err(copy)
        }

        async fn can_access(
            &self,
            _world: &WorldName,
            _actor: &Address,
            _secret: Option<&str>,
            _scene: Option<&SceneId>,
            _device_id: Option<&str>,
            _now: NaiveDateTime,
        ) -> Result<Access, AuthorityNotEstablished> {
            self.access.as_ref().cloned().map_err(copy)
        }

        async fn is_blocked(&self, _actor: &Address) -> Result<bool, AuthorityNotEstablished> {
            self.blocked.as_ref().copied().map_err(copy)
        }

        async fn is_admin(&self, _scene: &SceneRef<'_>, _actor: &Address) -> PermissionDecision {
            match &self.admin {
                PermissionDecision::Allowed => PermissionDecision::Allowed,
                PermissionDecision::Refused(error) => PermissionDecision::Refused(copy(error)),
            }
        }

        async fn protected_addresses(
            &self,
            _world: &WorldName,
            _parcels: &ParcelSet,
        ) -> Result<BTreeSet<Address>, AuthorityNotEstablished> {
            self.protected.as_ref().cloned().map_err(copy)
        }
    }

    pub async fn classify_fixture(
        scenes: &dyn SceneIdentity,
        room: &str,
    ) -> Result<crate::rooms::RoomClassification, ResolveError> {
        use crate::rooms::{ParsedRoom, RoomClassification as C, RoomRefusal as R, refused_room};
        let request = match crate::rooms::parse_room(room) {
            Some(ParsedRoom::Local(resolved)) => return Ok(C::Canonical(resolved)),
            Some(ParsedRoom::WorldScene(room)) => RoomRequest::NamedWorldScene { room },
            Some(ParsedRoom::WorldLobby(_)) => {
                let world = WorldName::parse(room.strip_prefix("world-").unwrap()).unwrap();
                return match scenes
                    .resolve(RoomRequest::WorldLobby { world: &world })
                    .await
                {
                    Ok(resolved) if resolved.room == room => Ok(C::Canonical(resolved)),
                    Ok(_)
                    | Err(ResolveError::WorldHasNoScene(_) | ResolveError::UnresolvedRoom(_)) => {
                        Ok(refused_room(room, R::Unresolved))
                    }
                    Err(error) => Err(error),
                };
            }
            None => {
                return Ok(
                    if room.starts_with("world-")
                        || room.starts_with(crate::rooms::SCENE_ROOM_PREFIX)
                        || room.starts_with("scene-")
                        || room.starts_with("scene:")
                    {
                        refused_room(room, R::Malformed)
                    } else {
                        C::Unrelated
                    },
                );
            }
        };
        match scenes.resolve(request).await {
            Ok(resolved) if resolved.room == room => Ok(C::Canonical(resolved)),
            Ok(_) | Err(ResolveError::UnresolvedRoom(_)) => Ok(refused_room(room, R::Unresolved)),
            Err(error) => Err(error),
        }
    }

    /// A [`SceneIdentity`] that gives every question the answer its field
    /// holds, whatever was asked. The default is an unknown world: no room
    /// resolves (`WorldHasNoScene`) and no scene is deployed.
    pub struct StubScenes {
        pub room: Result<ResolvedRoom, ResolveError>,
        pub scene: Result<Option<ActiveDeployment>, ResolveError>,
    }

    impl Default for StubScenes {
        fn default() -> Self {
            Self {
                room: Err(ResolveError::WorldHasNoScene("stub".to_string())),
                scene: Ok(None),
            }
        }
    }

    #[async_trait::async_trait]
    impl SceneIdentity for StubScenes {
        async fn classify_room(
            &self,
            room: &str,
        ) -> Result<crate::rooms::RoomClassification, ResolveError> {
            classify_fixture(self, room).await
        }

        async fn resolve(&self, _request: RoomRequest<'_>) -> Result<ResolvedRoom, ResolveError> {
            self.room.as_ref().cloned().map_err(copy_resolve)
        }

        async fn world_scene(
            &self,
            _world: &WorldName,
            _scene: &SceneId,
        ) -> Result<Option<ActiveDeployment>, ResolveError> {
            self.scene.as_ref().cloned().map_err(copy_resolve)
        }

        async fn world_active_scene(
            &self,
            _world: &WorldName,
        ) -> Result<Option<ActiveDeployment>, ResolveError> {
            self.scene.as_ref().cloned().map_err(copy_resolve)
        }
    }

    /// `ResolveError` is not `Clone` (it carries a `sqlx::Error`); each call
    /// hands out a copy, a database error as a protocol error of its text.
    fn copy_resolve(error: &ResolveError) -> ResolveError {
        use ResolveError::*;
        match error {
            UnresolvedRoom(room) => UnresolvedRoom(room.clone()),
            SceneNotInWorld { world, scene } => SceneNotInWorld {
                world: world.clone(),
                scene: scene.clone(),
            },
            WorldHasNoScene(world) => WorldHasNoScene(world.clone()),
            NoSceneAtParcel { realm, parcel } => NoSceneAtParcel {
                realm: realm.clone(),
                parcel: parcel.clone(),
            },
            NoParcels { scene } => NoParcels {
                scene: scene.clone(),
            },
            Db(error) => Db(sqlx::Error::Protocol(error.to_string())),
        }
    }

    /// `AuthorityNotEstablished` is not `Clone`; each call hands out a copy.
    pub(crate) fn copy(error: &AuthorityNotEstablished) -> AuthorityNotEstablished {
        use AuthorityNotEstablished::*;
        match error {
            AuthenticationMissingOrInvalid { detail } => {
                AuthenticationMissingOrInvalid { detail: *detail }
            }
            PresentedSharedSecretDidNotMatch => PresentedSharedSecretDidNotMatch,
            RefusedLacksAuthority { detail } => RefusedLacksAuthority {
                detail: detail.clone(),
            },
            UndeterminedStoreUnavailable {
                store,
                reason_for_operators,
            } => UndeterminedStoreUnavailable {
                store: *store,
                reason_for_operators: reason_for_operators.clone(),
            },
            CredentialNotConfigured {
                environment_variable,
            } => CredentialNotConfigured {
                environment_variable: *environment_variable,
            },
        }
    }
}
