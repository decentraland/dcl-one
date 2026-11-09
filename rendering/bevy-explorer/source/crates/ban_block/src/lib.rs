//! One answer for the whole client: is this user banned or blocked?
//!
//! A facade over the crates that produce each fact. Social blocks live in the social client,
//! the local user's scene admission and the scene room's ban and admin lists in comms, and the
//! only state this crate owns is the platform-ban check. [`BanBlock`] reads them all as one
//! system parameter; [`BanBlockEvent`] says when any of them changed.

mod debug;
mod events;
mod platform_ban;

use alloy_core::primitives::Address;
use bevy::{
    app::{App, Plugin, PostUpdate},
    ecs::{
        query::Has,
        schedule::IntoScheduleConfigs,
        system::{Query, Res, SystemParam},
    },
    platform::collections::HashSet,
};
use serde::Serialize;
use social::SocialClient;

pub use comms::{BanScope, SceneAdmission, SceneRoomModeration};
pub use events::BanBlockEvent;
pub use platform_ban::{PlatformBanInfo, PlatformBanSource, PlatformBanState, PlatformBanStatus};

pub struct BanBlockPlugin;

impl Plugin for BanBlockPlugin {
    fn build(&self, app: &mut App) {
        app.add_event::<BanBlockEvent>();
        // without the social plugin there is no social update system to order after
        if app.world().contains_resource::<SocialClient>() {
            app.add_systems(
                PostUpdate,
                events::emit_ban_block_events.after(social::apply_social_updates),
            );
        } else {
            app.add_systems(PostUpdate, events::emit_ban_block_events);
        }
        platform_ban::register(app);
        app.add_systems(
            PostUpdate,
            events::cross_feed_admission_refusal.before(events::emit_ban_block_events),
        );
        debug::register(app);
    }
}

/// The read side. Take it as a system parameter: `fn my_system(ban_block: BanBlock)`.
#[derive(SystemParam)]
pub struct BanBlock<'w, 's> {
    social: Option<Res<'w, SocialClient>>,
    platform_ban: Res<'w, PlatformBanStatus>,
    admission: Res<'w, SceneAdmission>,
    #[cfg(feature = "livekit")]
    room: Query<
        'w,
        's,
        (
            &'static comms::SceneRoom,
            &'static SceneRoomModeration,
            Has<comms::livekit::room::Connected>,
        ),
    >,
    #[cfg(not(feature = "livekit"))]
    _no_room: std::marker::PhantomData<&'s ()>,
}

/// What the connected scene room says about other players, borrowed from comms.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SceneRoomView<'a> {
    NotConnected,
    Connected {
        scene_id: &'a str,
        banned_addresses: &'a HashSet<Address>,
        scene_admins: &'a HashSet<Address>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AdminPermissionsStatus {
    Admin,
    NotAdmin,
    NotLoadedYet,
}

impl SceneRoomView<'_> {
    /// True only for a connected room that lists `addr`; a room that is down answers false.
    pub fn is_addr_banned_in_scene(&self, addr: Address) -> bool {
        matches!(self, SceneRoomView::Connected { banned_addresses, .. } if banned_addresses.contains(&addr))
    }

    pub fn admin_permissions_for_addr(&self, addr: Address) -> AdminPermissionsStatus {
        match self {
            SceneRoomView::NotConnected => AdminPermissionsStatus::NotLoadedYet,
            SceneRoomView::Connected { scene_admins, .. } if scene_admins.contains(&addr) => {
                AdminPermissionsStatus::Admin
            }
            SceneRoomView::Connected { .. } => AdminPermissionsStatus::NotAdmin,
        }
    }
}

/// The local user's social blocks, borrowed from the social client. The sets exist only once a
/// snapshot has been received.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Blocks<'a> {
    Unavailable {
        reason: BlocksUnavailableReason,
    },
    Live {
        /// Addresses the local user has blocked.
        blocked: &'a HashSet<Address>,
        /// Addresses that have blocked the local user.
        blocked_by: &'a HashSet<Address>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BlocksUnavailableReason {
    SocialFeatureOff,
    NotSignedIn,
    SocialNotConnected,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BlockDirection {
    None,
    LocalUserBlockedThem,
    TheyBlockedLocalUser,
    Mutual,
}

impl<'a> Blocks<'a> {
    /// `Live` with the sets as they are once the client has had a snapshot, whether or not it
    /// is still connected; `Unavailable` before that.
    pub fn from_client(
        initialized: bool,
        blocked: &'a HashSet<Address>,
        blocked_by: &'a HashSet<Address>,
    ) -> Self {
        if initialized {
            Blocks::Live {
                blocked,
                blocked_by,
            }
        } else {
            Blocks::Unavailable {
                reason: BlocksUnavailableReason::SocialNotConnected,
            }
        }
    }

    /// True when either side blocked the other; `Unavailable` answers false.
    pub fn is_blocked_either_way(&self, addr: Address) -> bool {
        self.block_direction(addr) != BlockDirection::None
    }

    pub fn block_direction(&self, addr: Address) -> BlockDirection {
        let Blocks::Live {
            blocked,
            blocked_by,
        } = self
        else {
            return BlockDirection::None;
        };
        match (blocked.contains(&addr), blocked_by.contains(&addr)) {
            (true, true) => BlockDirection::Mutual,
            (true, false) => BlockDirection::LocalUserBlockedThem,
            (false, true) => BlockDirection::TheyBlockedLocalUser,
            (false, false) => BlockDirection::None,
        }
    }
}

impl BanBlock<'_, '_> {
    pub fn blocks(&self) -> Blocks<'_> {
        if !social::SOCIAL_FEATURE {
            return Blocks::Unavailable {
                reason: BlocksUnavailableReason::SocialFeatureOff,
            };
        }
        let Some(social) = self.social.as_deref() else {
            return Blocks::Unavailable {
                reason: BlocksUnavailableReason::SocialNotConnected,
            };
        };
        match social.0.as_ref() {
            None => Blocks::Unavailable {
                reason: BlocksUnavailableReason::NotSignedIn,
            },
            Some(client) => {
                Blocks::from_client(client.is_initialized, &client.blocked, &client.blocked_by)
            }
        }
    }

    pub fn is_blocked_either_way(&self, addr: Address) -> bool {
        self.blocks().is_blocked_either_way(addr)
    }

    pub fn block_direction(&self, addr: Address) -> BlockDirection {
        self.blocks().block_direction(addr)
    }

    pub fn platform_ban(&self) -> &PlatformBanState {
        self.platform_ban.state()
    }

    /// `Some` only while a ban is known.
    pub fn platform_ban_source(&self) -> Option<PlatformBanSource> {
        self.platform_ban.source()
    }

    /// `Some` only for a checked ban; unknown and failed answer `None`.
    pub fn platform_ban_of_local_user(&self) -> Option<&PlatformBanInfo> {
        self.platform_ban.ban()
    }

    /// The local user's admission to the current scene's room.
    pub fn scene_admission(&self) -> &SceneAdmission {
        &self.admission
    }

    /// The scene room's lists while it is connected; a reconnecting or absent room is
    /// `NotConnected`.
    pub fn scene_room(&self) -> SceneRoomView<'_> {
        #[cfg(feature = "livekit")]
        if let Ok((scene, moderation, true)) = self.room.single() {
            return SceneRoomView::Connected {
                scene_id: &scene.0,
                banned_addresses: &moderation.banned_addresses,
                scene_admins: &moderation.scene_admins,
            };
        }
        SceneRoomView::NotConnected
    }

    pub fn is_addr_banned_in_scene(&self, addr: Address) -> bool {
        self.scene_room().is_addr_banned_in_scene(addr)
    }

    pub fn admin_permissions_for_addr(&self, addr: Address) -> AdminPermissionsStatus {
        self.scene_room().admin_permissions_for_addr(addr)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use common::util::AsH160;

    fn address(n: u8) -> Address {
        Address::from_slice(&[n; 20])
    }

    #[test]
    fn a_pair_is_blocked_when_either_side_blocked_the_other() {
        let blocked = HashSet::from_iter([address(1), address(3)]);
        let blocked_by = HashSet::from_iter([address(2), address(3)]);
        let blocks = Blocks::from_client(true, &blocked, &blocked_by);
        assert!(blocks.is_blocked_either_way(address(1)));
        assert!(blocks.is_blocked_either_way(address(2)));
        assert!(!blocks.is_blocked_either_way(address(4)));
        assert_eq!(
            blocks.block_direction(address(1)),
            BlockDirection::LocalUserBlockedThem
        );
        assert_eq!(
            blocks.block_direction(address(2)),
            BlockDirection::TheyBlockedLocalUser
        );
        assert_eq!(blocks.block_direction(address(3)), BlockDirection::Mutual);
        assert_eq!(blocks.block_direction(address(4)), BlockDirection::None);
    }

    #[test]
    fn an_uninitialised_client_has_no_sets() {
        let blocked = HashSet::from_iter([address(1)]);
        let blocks = Blocks::from_client(false, &blocked, &blocked);
        assert_eq!(
            blocks,
            Blocks::Unavailable {
                reason: BlocksUnavailableReason::SocialNotConnected
            }
        );
        assert!(!blocks.is_blocked_either_way(address(1)));
        assert_eq!(blocks.block_direction(address(1)), BlockDirection::None);
    }

    #[test]
    fn a_room_answers_only_while_connected() {
        let banned = HashSet::from_iter([address(1)]);
        let admins = HashSet::from_iter([address(2)]);
        let connected = SceneRoomView::Connected {
            scene_id: "scene",
            banned_addresses: &banned,
            scene_admins: &admins,
        };
        assert!(connected.is_addr_banned_in_scene(address(1)));
        assert!(!connected.is_addr_banned_in_scene(address(2)));
        assert_eq!(
            connected.admin_permissions_for_addr(address(2)),
            AdminPermissionsStatus::Admin
        );
        assert_eq!(
            connected.admin_permissions_for_addr(address(1)),
            AdminPermissionsStatus::NotAdmin
        );
        let down = SceneRoomView::NotConnected;
        assert!(!down.is_addr_banned_in_scene(address(1)));
        assert_eq!(
            down.admin_permissions_for_addr(address(2)),
            AdminPermissionsStatus::NotLoadedYet
        );
    }

    #[test]
    fn addresses_compare_without_case() {
        let upper = "0xABCDEFABCDEFABCDEFABCDEFABCDEFABCDEFABCD"
            .as_h160()
            .unwrap();
        let lower = "0xabcdefabcdefabcdefabcdefabcdefabcdefabcd"
            .as_h160()
            .unwrap();
        let blocked = HashSet::from_iter([upper]);
        let none = HashSet::new();
        let blocks = Blocks::from_client(true, &blocked, &none);
        assert!(blocks.is_blocked_either_way(lower));
        assert_eq!(format!("{lower:#x}"), format!("{upper:#x}"));
    }
}
