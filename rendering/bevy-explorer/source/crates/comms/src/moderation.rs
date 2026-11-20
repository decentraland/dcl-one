//! What the gatekeeper and the scene room say about bans: the local user's admission to the
//! current scene's room, and the room's banned and admin lists.

use alloy_core::primitives::Address;
use bevy::{
    ecs::{component::Component, resource::Resource},
    platform::collections::{HashMap, HashSet},
};
use common::util::AsH160;
use serde::Serialize;

/// Admission to the current scene and recognized refusals for this wallet's realm session.
/// Refusals outlive the scene and its room; only fresh admission or a session change clears them.
#[derive(Resource, Debug, Clone, PartialEq, Default)]
pub struct SceneAdmission {
    current: Admission,
    session: Option<(String, Address)>,
    refusals: HashMap<String, SceneRefusal>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
enum Admission {
    #[default]
    Unknown,
    Allowed {
        scene_id: String,
    },
    Refused {
        scene_id: String,
    },
}

#[derive(Debug, Clone, PartialEq)]
struct SceneRefusal {
    scope: BanScope,
    message: String,
    retry_due: f64,
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
        Self {
            current: Admission::Allowed {
                scene_id: scene_id.to_owned(),
            },
            ..Self::default()
        }
    }

    /// Recognizes the gatekeeper's 403s and the worlds server's ban-specific 401s.
    /// A refusal that names no ban, or a 503, is unknown.
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
        Self {
            current: Admission::Refused {
                scene_id: scene_id.to_owned(),
            },
            refusals: HashMap::from_iter([(
                scene_id.to_owned(),
                SceneRefusal {
                    scope,
                    message: message.trim_end_matches('.').to_owned(),
                    retry_due: 0.0,
                },
            )]),
            ..Self::default()
        }
    }

    pub(crate) fn from_room_ban(scene_id: &str) -> Self {
        Self::from_refusal(403, Some(SCENE_BANNED), false, scene_id)
    }

    pub fn for_session(mut self, realm_about_url: &str, address: Address) -> Self {
        self.session = Some((realm_about_url.to_owned(), address));
        self
    }

    pub(crate) fn select_scene(&mut self, realm: &str, address: Option<Address>, scene_id: &str) {
        let session = address.map(|address| (realm.to_owned(), address));
        if self.session != session {
            self.refusals.clear();
            self.session = session;
        }
        // Platform refusals are reported by the platform-ban owner, not retained as scene bans.
        self.refusals
            .retain(|_, refusal| refusal.scope != BanScope::Platform);
        self.current = if self.refusals.contains_key(scene_id) {
            Admission::Refused {
                scene_id: scene_id.to_owned(),
            }
        } else {
            Admission::Unknown
        };
    }

    pub(crate) fn observe(&mut self, next: Self) {
        if self.session != next.session {
            self.refusals.clear();
            self.session = next.session;
        }
        if let Admission::Allowed { scene_id } = &next.current {
            self.refusals.remove(scene_id);
        }
        for (scene_id, mut refusal) in next.refusals {
            if let Some(previous) = self.refusals.get(&scene_id) {
                refusal.retry_due = refusal.retry_due.max(previous.retry_due);
            }
            self.refusals.insert(scene_id, refusal);
        }
        self.current = next.current;
    }

    /// Whether this scene must stay unloaded, even while another scene is current.
    pub fn is_scene_banned(&self, realm_about_url: &str, address: Address, scene_id: &str) -> bool {
        self.session
            .as_ref()
            .is_some_and(|(realm, user)| realm == realm_about_url && *user == address)
            && self
                .refusals
                .get(scene_id)
                .is_some_and(|refusal| matches!(refusal.scope, BanScope::Scene | BanScope::World))
    }

    #[cfg(test)]
    pub(crate) fn banned_scene(&self, realm_about_url: &str, address: Address) -> Option<&str> {
        self.scene_id()
            .filter(|scene_id| self.is_scene_banned(realm_about_url, address, scene_id))
    }

    // The active room owns retries. Checkpoint its deadline only when disposing that room,
    // so returning to a refused parcel cannot bypass a server's longer Retry-After.
    pub(crate) fn remember_retry(&mut self, scene_id: &str, due: f64) {
        if let Some(refusal) = self.refusals.get_mut(scene_id) {
            refusal.retry_due = refusal.retry_due.max(due);
        }
    }

    pub(crate) fn retry_due(&self, scene_id: &str) -> Option<f64> {
        self.refusals.get(scene_id).map(|refusal| refusal.retry_due)
    }

    pub fn view(&self) -> AdmissionView<'_> {
        match &self.current {
            Admission::Unknown => AdmissionView::Unknown,
            Admission::Allowed { scene_id } => AdmissionView::Allowed { scene_id },
            Admission::Refused { scene_id } => {
                let refusal = &self.refusals[scene_id];
                AdmissionView::Refused {
                    scope: refusal.scope,
                    message: &refusal.message,
                    scene_id,
                }
            }
        }
    }

    /// The gatekeeper refused the current scene's player as banned.
    pub fn is_refused(&self) -> bool {
        matches!(self.current, Admission::Refused { .. })
    }

    pub fn scene_id(&self) -> Option<&str> {
        match &self.current {
            Admission::Unknown => None,
            Admission::Allowed { scene_id } | Admission::Refused { scene_id } => Some(scene_id),
        }
    }
}

/// The scene room's moderation lists, parsed from its LiveKit metadata. Lives on the room entity
/// beside [`crate::SceneRoom`].
#[derive(Component, Debug, Clone, PartialEq, Eq, Default)]
pub struct SceneRoomModeration {
    pub banned_addresses: HashSet<Address>,
    pub scene_admins: HashSet<Address>,
}

/// Marks a remote player the local user must neither see nor hear: blocked either way, or banned
/// from the current scene's room. `ban_block` keeps it current. While it is present the player's
/// avatar, nametag and attachments are hidden and their nearby voice is not subscribed (a call
/// still carries them); anything else that must leave the player out reads this marker rather
/// than deciding again.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct BlockedOrBanned;

#[derive(Debug, PartialEq, Eq)]
pub enum MetadataError {
    NotAnObject,
}

impl SceneRoomModeration {
    pub fn is_banned(&self, address: Address) -> bool {
        self.banned_addresses.contains(&address)
    }

    pub fn is_admin(&self, address: Address) -> bool {
        self.scene_admins.contains(&address)
    }

    /// Only explicit arrays replace lists. Missing or non-array fields retain known state;
    /// an empty array clears its list, and entries that are not addresses are skipped.
    pub(crate) fn update_from_metadata(&mut self, metadata: &str) -> Result<(), MetadataError> {
        if metadata.trim().is_empty() {
            return Ok(());
        }
        let value: serde_json::Value =
            serde_json::from_str(metadata).map_err(|_| MetadataError::NotAnObject)?;
        let object = value.as_object().ok_or(MetadataError::NotAnObject)?;
        let addresses = |key: &str| -> Option<HashSet<Address>> {
            object.get(key).and_then(|v| v.as_array()).map(|list| {
                list.iter()
                    .filter_map(|entry| entry.as_str().and_then(|s| s.as_h160()))
                    .collect()
            })
        };
        if let Some(addresses) = addresses("bannedAddresses") {
            self.banned_addresses = addresses;
        }
        if let Some(addresses) = addresses("sceneAdmins") {
            self.scene_admins = addresses;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_metadata(metadata: &str) -> Result<SceneRoomModeration, MetadataError> {
        let mut state = SceneRoomModeration::default();
        state.update_from_metadata(metadata)?;
        Ok(state)
    }

    #[test]
    fn refusals_survive_scene_changes_and_only_their_own_fresh_admission_clears_them() {
        let realm = "https://realm.example/about";
        let address = Address::repeat_byte(1);
        let mut admission = SceneAdmission::from_room_ban("a").for_session(realm, address);
        admission.remember_retry("a", 45.0);
        admission.select_scene(realm, Some(address), "b");
        admission.observe(SceneAdmission::allowed("b").for_session(realm, address));
        assert!(admission.is_scene_banned(realm, address, "a"));
        admission.observe(SceneAdmission::from_room_ban("b").for_session(realm, address));
        admission.select_scene(realm, Some(address), "a");
        assert_eq!(admission.banned_scene(realm, address), Some("a"));
        assert_eq!(admission.retry_due("a"), Some(45.0));
        admission.observe(SceneAdmission::allowed("a").for_session(realm, address));
        assert!(!admission.is_scene_banned(realm, address, "a"));
        assert!(admission.is_scene_banned(realm, address, "b"));
        assert_eq!(admission.retry_due("a"), None);
        admission.select_scene(realm, Some(Address::repeat_byte(2)), "b");
        assert!(!admission.is_scene_banned(realm, address, "b"));
        admission.select_scene(realm, Some(address), "b");
        assert!(!admission.is_scene_banned(realm, address, "b"));
        admission.observe(SceneAdmission::from_room_ban("b").for_session(realm, address));
        admission.select_scene("https://other.example/about", Some(address), "b");
        admission.select_scene(realm, Some(address), "b");
        assert!(!admission.is_scene_banned(realm, address, "b"));
    }

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
        // a 401 outside a world is not read as a ban
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
        let parsed = parse_metadata(full).unwrap();
        assert_eq!(parsed.banned_addresses.len(), 1);
        assert!(parsed.banned_addresses.contains(
            &"0xabcdefabcdefabcdefabcdefabcdefabcdefabcd"
                .as_h160()
                .unwrap()
        ));
        assert_eq!(parsed.scene_admins.len(), 1);

        let partial = parse_metadata(r#"{"sceneAdmins":"not a list"}"#).unwrap();
        assert_eq!(partial, SceneRoomModeration::default());
        assert_eq!(parse_metadata("").unwrap(), SceneRoomModeration::default());
        assert_eq!(
            parse_metadata("{}").unwrap(),
            SceneRoomModeration::default()
        );
        assert_eq!(parse_metadata("[1,2]"), Err(MetadataError::NotAnObject));
        assert_eq!(parse_metadata("garbage"), Err(MetadataError::NotAnObject));
    }
}
