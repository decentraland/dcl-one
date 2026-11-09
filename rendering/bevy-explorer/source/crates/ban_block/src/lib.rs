//! One answer for the whole client: is this user banned or blocked?
//!
//! A facade over the crates that produce each fact. Social blocks live in the social client,
//! the local user's scene admission and the scene room's ban and admin lists in comms, and the
//! only state this crate owns is the platform-ban check. [`BanBlock`] reads them all as one
//! system parameter; [`BanBlockEvent`] says when any of them changed.

mod debug;
mod events;

use alloy_core::primitives::Address;
use bevy::{ecs::system::SystemParam, platform::collections::HashSet, prelude::*};
use serde::Serialize;
use social::SocialClient;

pub use events::BanBlockEvent;

pub struct BanBlockPlugin;

impl Plugin for BanBlockPlugin {
    fn build(&self, app: &mut App) {
        app.add_event::<BanBlockEvent>();
        // the headless client has no social plugin: nothing to order after, and the facade
        // reads the missing resource as unavailable
        if app.world().contains_resource::<SocialClient>() {
            app.add_systems(
                PostUpdate,
                events::emit_ban_block_events.after(social::apply_social_updates),
            );
        } else {
            app.add_systems(PostUpdate, events::emit_ban_block_events);
        }
        debug::register(app);
    }
}

/// The read side. Take it as a system parameter: `fn my_system(ban_block: BanBlock)`.
#[derive(SystemParam)]
pub struct BanBlock<'w> {
    social: Option<Res<'w, SocialClient>>,
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
    /// The view of a social client's sets. A client that has disconnected after its first
    /// snapshot keeps answering `Live` with its last sets: only the next snapshot replaces them.
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

impl BanBlock<'_> {
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
