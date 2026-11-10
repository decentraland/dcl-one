//! What the gatekeeper and the scene room say about bans: the local user's admission to the
//! current scene's room, and the room's banned and admin lists.

use alloy_core::primitives::Address;
use bevy::{
    ecs::{component::Component, resource::Resource},
    platform::collections::HashSet,
};
use common::util::AsH160;
use serde::Serialize;

/// The gatekeeper's answer to the local user's request to join the current scene's room. Built
/// only through its constructors; read through [`SceneAdmission::view`].
#[derive(Resource, Debug, Clone, PartialEq, Eq, Default)]
pub struct SceneAdmission(Admission);

#[derive(Debug, Clone, PartialEq, Eq, Default)]
enum Admission {
    #[default]
    Unknown,
    Allowed {
        scene_id: String,
    },
    Refused {
        scope: BanScope,
        message: String,
        scene_id: String,
        session: Option<(String, Address)>,
    },
}

/// A [`SceneAdmission`] as it reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdmissionView<'a> {
    Unknown,
    Allowed {
        scene_id: &'a str,
    },
    Refused {
        scope: BanScope,
        message: &'a str,
        scene_id: &'a str,
    },
}

/// Why a scene admission was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BanScope {
    Platform,
    World,
    Scene,
}

/// The refusal bodies of the comms gatekeeper and the worlds server.
const PLATFORM_BANNED: &str = "Access denied, platform-banned user";
const SCENE_BANNED: &str = "User is banned from this scene";
const WORLD_BANNED_PREFIX: &str = "You are banned from world";

impl SceneAdmission {
    /// No answer for the current scene.
    pub fn unknown() -> Self {
        Self::default()
    }

    /// The gatekeeper minted the scene's room.
    pub fn allowed(scene_id: &str) -> Self {
        Self(Admission::Allowed {
            scene_id: scene_id.to_owned(),
        })
    }

    /// The admission a scene-adapter refusal means: the gatekeeper's 403s, and the worlds
    /// server's 401s for a platform or world ban. A refusal that names no ban and a 503 are
    /// unknown.
    pub fn from_refusal(
        status: u16,
        message: Option<&str>,
        is_world: bool,
        scene_id: &str,
    ) -> Self {
        let message = message.map(str::trim).unwrap_or_default();
        let scope = match status {
            403 if message.starts_with(PLATFORM_BANNED) => BanScope::Platform,
            403 if message.starts_with(SCENE_BANNED) => BanScope::Scene,
            401 if is_world && message.starts_with(PLATFORM_BANNED) => BanScope::Platform,
            401 if is_world && message.starts_with(WORLD_BANNED_PREFIX) => BanScope::World,
            _ => return Self::unknown(),
        };
        Self(Admission::Refused {
            scope,
            message: message.trim_end_matches('.').to_owned(),
            scene_id: scene_id.to_owned(),
            session: None,
        })
    }

    pub(crate) fn from_room_ban(scene_id: &str) -> Self {
        Self(Admission::Refused {
            scope: BanScope::Scene,
            message: SCENE_BANNED.to_owned(),
            scene_id: scene_id.to_owned(),
            session: None,
        })
    }

    pub fn for_session(mut self, realm_about_url: &str, address: Address) -> Self {
        // The worlds adapter reports the same refusal for a world-wide ban and a scene ban.
        // In either case the refusal applies to the scene whose adapter was requested.
        if let Admission::Refused {
            scope: BanScope::Scene | BanScope::World,
            session,
            ..
        } = &mut self.0
        {
            *session = Some((realm_about_url.to_owned(), address));
        }
        self
    }

    pub fn banned_scene(&self, realm_about_url: &str, address: Address) -> Option<&str> {
        match &self.0 {
            Admission::Refused {
                scene_id,
                session: Some((realm, user)),
                ..
            } if realm == realm_about_url && *user == address => Some(scene_id),
            _ => None,
        }
    }

    pub fn view(&self) -> AdmissionView<'_> {
        match &self.0 {
            Admission::Unknown => AdmissionView::Unknown,
            Admission::Allowed { scene_id } => AdmissionView::Allowed { scene_id },
            Admission::Refused {
                scope,
                message,
                scene_id,
                ..
            } => AdmissionView::Refused {
                scope: *scope,
                message,
                scene_id,
            },
        }
    }

    pub fn scene_id(&self) -> Option<&str> {
        match &self.0 {
            Admission::Unknown => None,
            Admission::Allowed { scene_id } | Admission::Refused { scene_id, .. } => Some(scene_id),
        }
    }
}

/// The scene room's moderation lists, parsed from its LiveKit metadata. Lives on the room entity
/// beside [`crate::SceneRoom`] and goes with it.
#[derive(Component, Debug, Clone, PartialEq, Eq, Default)]
pub struct SceneRoomModeration {
    pub banned_addresses: HashSet<Address>,
    pub scene_admins: HashSet<Address>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum MetadataError {
    NotAnObject,
}

impl SceneRoomModeration {
    /// Parses `{"bannedAddresses":[..],"sceneAdmins":[..]}`. A missing or non-array field is
    /// empty; an entry that is not an address is skipped.
    pub fn from_metadata(metadata: &str) -> Result<Self, MetadataError> {
        if metadata.trim().is_empty() {
            return Ok(Self::default());
        }
        let value: serde_json::Value =
            serde_json::from_str(metadata).map_err(|_| MetadataError::NotAnObject)?;
        let object = value.as_object().ok_or(MetadataError::NotAnObject)?;
        let addresses = |key: &str| -> HashSet<Address> {
            object
                .get(key)
                .and_then(|v| v.as_array())
                .map(|list| {
                    list.iter()
                        .filter_map(|entry| entry.as_str().and_then(|s| s.as_h160()))
                        .collect()
                })
                .unwrap_or_default()
        };
        Ok(Self {
            banned_addresses: addresses("bannedAddresses"),
            scene_admins: addresses("sceneAdmins"),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scene_bans_apply_only_to_the_session_that_received_them() {
        let realm = "https://example.invalid/about";
        let address = Address::repeat_byte(1);
        let ban = SceneAdmission::from_room_ban("scene-1").for_session(realm, address);

        assert_eq!(ban.banned_scene(realm, address), Some("scene-1"));
        assert_eq!(ban.banned_scene(realm, Address::repeat_byte(2)), None);
        assert_eq!(
            ban.banned_scene("https://another.invalid/about", address),
            None
        );
        assert_eq!(
            SceneAdmission::allowed("scene-1").banned_scene(realm, address),
            None
        );

        let platform_ban =
            SceneAdmission::from_refusal(403, Some(PLATFORM_BANNED), false, "scene-1")
                .for_session(realm, address);
        assert_eq!(platform_ban.banned_scene(realm, address), None);
    }

    #[test]
    fn world_adapter_refusal_applies_to_the_requested_scene_only() {
        let realm = "https://example.invalid/world/example.dcl.eth/about";
        let address = Address::repeat_byte(1);
        let refusal = SceneAdmission::from_refusal(
            401,
            Some("You are banned from world \"example.dcl.eth\"."),
            true,
            "world-scene-1",
        )
        .for_session(realm, address);

        assert!(matches!(
            refusal.view(),
            AdmissionView::Refused {
                scope: BanScope::World,
                ..
            }
        ));
        assert_eq!(refusal.banned_scene(realm, address), Some("world-scene-1"));
        assert_eq!(
            refusal.banned_scene("https://another.invalid/about", address),
            None
        );
        assert_eq!(refusal.banned_scene(realm, Address::repeat_byte(2)), None);
    }

    #[test]
    fn a_refusal_names_its_scope_or_is_unknown() {
        let refused = |status, message: Option<&str>, world| {
            SceneAdmission::from_refusal(status, message, world, "scene-1")
        };
        assert_eq!(
            refused(403, Some(PLATFORM_BANNED), false).view(),
            AdmissionView::Refused {
                scope: BanScope::Platform,
                message: PLATFORM_BANNED,
                scene_id: "scene-1",
            }
        );
        assert!(matches!(
            refused(403, Some(SCENE_BANNED), false).view(),
            AdmissionView::Refused {
                scope: BanScope::Scene,
                ..
            }
        ));
        assert!(matches!(
            refused(
                401,
                Some("You are banned from world \"abc.dcl.eth\"."),
                true
            )
            .view(),
            AdmissionView::Refused {
                scope: BanScope::World,
                ..
            }
        ));
        assert!(matches!(
            refused(401, Some("Access denied, platform-banned user."), true).view(),
            AdmissionView::Refused {
                scope: BanScope::Platform,
                ..
            }
        ));
        // a 401 outside a world is a bad signature, not a ban
        assert_eq!(
            refused(401, Some("You are banned from world \"x\"."), false),
            SceneAdmission::unknown()
        );
        assert_eq!(
            refused(403, Some("Forbidden"), false),
            SceneAdmission::unknown()
        );
        assert_eq!(refused(403, None, false), SceneAdmission::unknown());
        assert_eq!(
            refused(503, Some(SCENE_BANNED), false),
            SceneAdmission::unknown()
        );
        assert_eq!(SceneAdmission::allowed("s").scene_id(), Some("s"));
        assert_eq!(SceneAdmission::unknown().scene_id(), None);
        assert_eq!(SceneAdmission::default(), SceneAdmission::unknown());
    }

    #[test]
    fn room_metadata_parses_leniently() {
        let full = r#"{"bannedAddresses":["0xABCDEFABCDEFABCDEFABCDEFABCDEFABCDEFABCD","nope"],"sceneAdmins":["0x1111111111111111111111111111111111111111"]}"#;
        let parsed = SceneRoomModeration::from_metadata(full).unwrap();
        assert_eq!(parsed.banned_addresses.len(), 1);
        assert!(parsed.banned_addresses.contains(
            &"0xabcdefabcdefabcdefabcdefabcdefabcdefabcd"
                .as_h160()
                .unwrap()
        ));
        assert_eq!(parsed.scene_admins.len(), 1);

        let partial =
            SceneRoomModeration::from_metadata(r#"{"sceneAdmins":"not a list"}"#).unwrap();
        assert_eq!(partial, SceneRoomModeration::default());
        assert_eq!(
            SceneRoomModeration::from_metadata("").unwrap(),
            SceneRoomModeration::default()
        );
        assert_eq!(
            SceneRoomModeration::from_metadata("{}").unwrap(),
            SceneRoomModeration::default()
        );
        assert_eq!(
            SceneRoomModeration::from_metadata("[1,2]"),
            Err(MetadataError::NotAnObject)
        );
        assert_eq!(
            SceneRoomModeration::from_metadata("garbage"),
            Err(MetadataError::NotAnObject)
        );
    }
}
