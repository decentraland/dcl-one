//! What the gatekeeper and the scene room say about bans: the local user's admission to the
//! current scene's room, and the room's banned and admin lists.

use alloy_core::primitives::Address;
use bevy::{platform::collections::HashSet, prelude::*};
use common::util::AsH160;
use serde::Serialize;

/// The gatekeeper's answer to the local user's request to join the current scene's room.
#[derive(Resource, Debug, Clone, PartialEq, Eq, Default)]
pub enum SceneAdmission {
    #[default]
    Unknown,
    Allowed {
        scene_id: String,
    },
    Refused {
        scope: BanScope,
        message: String,
        scene_id: String,
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

/// The refusal bodies the gatekeeper sends (`services/catalyst/comms/src/gatekeeper`).
const PLATFORM_BANNED: &str = "Access denied, platform-banned user";
const SCENE_BANNED: &str = "User is banned from this scene";
const WORLD_BANNED_PREFIX: &str = "You are banned from world";

impl SceneAdmission {
    /// The admission a scene-adapter refusal means. A refusal that names no ban, a 503 and a 401
    /// without the world-ban message are `Unknown`.
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
            401 if is_world && message.starts_with(WORLD_BANNED_PREFIX) => BanScope::World,
            _ => return SceneAdmission::Unknown,
        };
        SceneAdmission::Refused {
            scope,
            message: message.trim_end_matches('.').to_owned(),
            scene_id: scene_id.to_owned(),
        }
    }

    pub fn scene_id(&self) -> Option<&str> {
        match self {
            SceneAdmission::Unknown => None,
            SceneAdmission::Allowed { scene_id } | SceneAdmission::Refused { scene_id, .. } => {
                Some(scene_id)
            }
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
    fn a_refusal_names_its_scope_or_is_unknown() {
        let refused = |status, message: Option<&str>, world| {
            SceneAdmission::from_refusal(status, message, world, "scene-1")
        };
        assert_eq!(
            refused(403, Some(PLATFORM_BANNED), false),
            SceneAdmission::Refused {
                scope: BanScope::Platform,
                message: PLATFORM_BANNED.to_owned(),
                scene_id: "scene-1".to_owned()
            }
        );
        assert!(matches!(
            refused(403, Some(SCENE_BANNED), false),
            SceneAdmission::Refused {
                scope: BanScope::Scene,
                ..
            }
        ));
        assert!(matches!(
            refused(
                401,
                Some("You are banned from world \"abc.dcl.eth\"."),
                true
            ),
            SceneAdmission::Refused {
                scope: BanScope::World,
                ..
            }
        ));
        // a 401 outside a world is a bad signature, not a ban
        assert_eq!(
            refused(401, Some("You are banned from world \"x\"."), false),
            SceneAdmission::Unknown
        );
        assert_eq!(
            refused(403, Some("Forbidden"), false),
            SceneAdmission::Unknown
        );
        assert_eq!(refused(403, None, false), SceneAdmission::Unknown);
        assert_eq!(
            refused(503, Some(SCENE_BANNED), false),
            SceneAdmission::Unknown
        );
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
