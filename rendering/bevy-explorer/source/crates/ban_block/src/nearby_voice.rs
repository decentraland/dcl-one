//! Who is not heard in nearby voice: a player blocked in either direction, or banned from the
//! connected scene room. Comms reads the set when it subscribes to nearby streams; calls ignore
//! it, so a call the player joined still carries everyone in it.

use alloy_core::primitives::Address;
use bevy::{
    app::{App, Plugin, Update},
    ecs::{
        change_detection::DetectChangesMut,
        schedule::{common_conditions::resource_exists, IntoScheduleConfigs},
        system::{Query, ResMut},
    },
    platform::collections::HashSet,
};
use comms::global_crdt::{handle_foreign_audio, ForeignPlayer, NearbyVoiceDenied};

use crate::BanBlock;

pub(crate) struct NearbyVoiceFilterPlugin;

impl Plugin for NearbyVoiceFilterPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            sync_nearby_voice_denied
                .run_if(resource_exists::<NearbyVoiceDenied>)
                .before(handle_foreign_audio),
        );
    }
}

/// Rebuilt every frame, so a block, an unblock, a late block list or a changed scene ban list
/// takes effect on the next subscription pass.
fn sync_nearby_voice_denied(
    ban_block: BanBlock,
    players: Query<&ForeignPlayer>,
    mut denied: ResMut<NearbyVoiceDenied>,
) {
    let next = denied_players(players.iter().map(|player| player.address), |addr| {
        ban_block.is_blocked_either_way(addr) || ban_block.is_addr_banned_in_scene(addr)
    });
    denied.set_if_neq(NearbyVoiceDenied(next));
}

fn denied_players(
    players: impl Iterator<Item = Address>,
    is_denied: impl Fn(Address) -> bool,
) -> HashSet<Address> {
    players.filter(|addr| is_denied(*addr)).collect()
}

#[cfg(test)]
mod tests {
    use bevy::{ecs::system::RunSystemOnce, prelude::*};
    use social::SocialClient;
    use wallet::Wallet;

    use super::*;
    use crate::{Blocks, PlatformBanStatus, SceneAdmission, SceneRoomView};

    fn address(n: u8) -> Address {
        Address::from_slice(&[n; 20])
    }

    #[test]
    fn blocked_either_way_and_scene_banned_players_are_denied() {
        let blocked = HashSet::from_iter([address(1)]);
        let blocked_by = HashSet::from_iter([address(2)]);
        let banned = HashSet::from_iter([address(3)]);
        let none = HashSet::new();
        let blocks = Blocks::from_client(true, &blocked, &blocked_by);
        let room = SceneRoomView::Connected {
            scene_id: "scene",
            banned_addresses: &banned,
            scene_admins: &none,
        };
        let denied = denied_players((1..=4).map(address), |addr| {
            blocks.is_blocked_either_way(addr) || room.is_addr_banned_in_scene(addr)
        });
        assert_eq!(
            denied,
            HashSet::from_iter([address(1), address(2), address(3)])
        );

        // before the block list arrives, and with the scene room down, nobody is denied
        let unavailable = Blocks::from_client(false, &blocked, &blocked_by);
        let down = SceneRoomView::NotConnected;
        let denied = denied_players((1..=4).map(address), |addr| {
            unavailable.is_blocked_either_way(addr) || down.is_addr_banned_in_scene(addr)
        });
        assert!(denied.is_empty());
    }

    #[test]
    fn the_system_leaves_the_set_unchanged_when_nothing_changed() {
        let mut world = World::new();
        world.init_resource::<Wallet>();
        world.init_resource::<PlatformBanStatus>();
        world.init_resource::<SceneAdmission>();
        world.insert_resource(SocialClient(None));
        world.init_resource::<NearbyVoiceDenied>();
        world.run_system_once(sync_nearby_voice_denied).unwrap();
        world.clear_trackers();
        world.run_system_once(sync_nearby_voice_denied).unwrap();
        assert!(!world.resource_ref::<NearbyVoiceDenied>().is_changed());
        assert!(world.resource::<NearbyVoiceDenied>().0.is_empty());
    }
}
