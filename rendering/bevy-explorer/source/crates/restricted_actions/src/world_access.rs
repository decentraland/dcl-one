//! Whether the local user may enter a world, by the check a realm change runs before entering
//! (`ipfs::world_access`).

use bevy::{prelude::*, tasks::IoTaskPool};
use common::{sets::SceneSets, util::TaskCompat};
use ipfs::{
    world_access::{world_entry, WorldEntry},
    IpfsResource,
};
use system_bridge::{SystemApi, WorldAccessVerdict};
use wallet::Wallet;

pub struct WorldAccessPlugin;

impl Plugin for WorldAccessPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            answer_world_access.in_set(SceneSets::RestrictedActions),
        );
    }
}

/// A world name is one path segment of the worlds server's routes.
fn is_world_name(world: &str) -> bool {
    !world.is_empty()
        && !world.starts_with('.')
        && world
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'.')
}

fn verdict(entry: Result<WorldEntry, String>) -> WorldAccessVerdict {
    match entry {
        Ok(WorldEntry::Open) => WorldAccessVerdict::Allowed,
        Ok(WorldEntry::Listed) => WorldAccessVerdict::Invited,
        Ok(WorldEntry::NotListed) => WorldAccessVerdict::Denied,
        Ok(WorldEntry::Password) => WorldAccessVerdict::Password,
        Err(_) => WorldAccessVerdict::Failed,
    }
}

fn answer_world_access(
    mut events: EventReader<SystemApi>,
    wallet: Res<Wallet>,
    ipfs: Res<IpfsResource>,
) {
    for event in events.read() {
        let SystemApi::GetWorldAccess(world, response) = event else {
            continue;
        };
        // without an identity there is nothing to check against: the world decides on arrival
        if wallet.address().is_none() || !is_world_name(world) {
            response.send(WorldAccessVerdict::Failed);
            continue;
        }
        let world = world.to_ascii_lowercase();
        let wallet = wallet.clone();
        let client = ipfs.client();
        let response = response.clone();
        IoTaskPool::get()
            .spawn_compat(async move {
                let entry = world_entry(&world, &wallet, &client).await;
                if let Err(e) = &entry {
                    debug!("world access of {world}: {e}");
                }
                response.send(verdict(entry));
            })
            .detach();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_a_single_path_segment_is_a_world_name() {
        for world in ["boedo.dcl.eth", "name.eth", "a-b.dcl.eth"] {
            assert!(is_world_name(world), "{world}");
        }
        for world in ["", "..", ".hidden", "a/b", "a?b", "a#b", "a%2fb", "a b"] {
            assert!(!is_world_name(world), "{world}");
        }
    }

    #[test]
    fn each_entry_has_its_verdict() {
        assert_eq!(verdict(Ok(WorldEntry::Open)), WorldAccessVerdict::Allowed);
        assert_eq!(verdict(Ok(WorldEntry::Listed)), WorldAccessVerdict::Invited);
        assert_eq!(
            verdict(Ok(WorldEntry::NotListed)),
            WorldAccessVerdict::Denied
        );
        assert_eq!(
            verdict(Ok(WorldEntry::Password)),
            WorldAccessVerdict::Password
        );
        assert_eq!(verdict(Err("down".to_owned())), WorldAccessVerdict::Failed);
    }
}
