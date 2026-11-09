use alloy_core::primitives::Address;
use bevy::{
    ecs::{
        change_detection::DetectChanges,
        event::{Event, EventWriter},
        system::{Local, Res, ResMut},
    },
    log::debug,
    platform::collections::HashSet,
    time::Time,
};

use crate::{
    BanBlock, BanScope, Blocks, PlatformBanInfo, PlatformBanSource, PlatformBanStatus,
    SceneAdmission, SceneRoomView,
};

/// Emitted once per change, after the sources already hold the new state.
#[derive(Event, Clone, PartialEq, Eq, Debug)]
pub enum BanBlockEvent {
    /// The local user blocked `target`.
    LocalUserBlocked { target: Address },
    /// The local user unblocked `target`.
    LocalUserUnblocked { target: Address },
    /// `blocker` blocked the local user.
    LocalUserBlockedBy { blocker: Address },
    /// `blocker` unblocked the local user.
    LocalUserUnblockedBy { blocker: Address },
    /// The blocks switched between unavailable and live.
    BlocksAvailabilityChanged,
    /// The platform ban changed.
    PlatformBanChanged,
    /// The local user's scene admission changed.
    SceneAdmissionChanged,
    /// The scene room's connection or lists changed.
    SceneRoomChanged,
}

/// The ban and block status as of the last events written; the next frame's diff is taken
/// against it.
#[derive(Default)]
pub(crate) struct LastEmittedBlockStatus {
    blocks: Option<(HashSet<Address>, HashSet<Address>)>,
    admission: SceneAdmission,
    room: Option<(String, HashSet<Address>, HashSet<Address>)>,
}

pub(crate) fn emit_ban_block_events(
    ban_block: BanBlock,
    mut last_emitted: Local<LastEmittedBlockStatus>,
    mut events: EventWriter<BanBlockEvent>,
) {
    let current = match ban_block.blocks() {
        Blocks::Live {
            blocked,
            blocked_by,
        } => Some((blocked, blocked_by)),
        Blocks::Unavailable { .. } => None,
    };
    let mut changed = diff_blocks(last_emitted.blocks.as_ref(), current);
    if !changed.is_empty() {
        last_emitted.blocks =
            current.map(|(blocked, blocked_by)| (blocked.clone(), blocked_by.clone()));
    }

    let admission = ban_block.scene_admission();
    if *admission != last_emitted.admission {
        last_emitted.admission = admission.clone();
        changed.push(BanBlockEvent::SceneAdmissionChanged);
    }

    let room = match ban_block.scene_room() {
        SceneRoomView::Connected {
            scene_id,
            banned_addresses,
            scene_admins,
        } => Some((scene_id, banned_addresses, scene_admins)),
        SceneRoomView::NotConnected => None,
    };
    let same_room = match (&last_emitted.room, room) {
        (None, None) => true,
        (Some((id, banned, admins)), Some((cur_id, cur_banned, cur_admins))) => {
            id == cur_id && banned == cur_banned && admins == cur_admins
        }
        _ => false,
    };
    if !same_room {
        last_emitted.room =
            room.map(|(id, banned, admins)| (id.to_owned(), banned.clone(), admins.clone()));
        changed.push(BanBlockEvent::SceneRoomChanged);
    }

    for event in changed {
        debug!("[ban_block] {event:?}");
        events.write(event);
    }
}

/// Records a platform-scoped admission refusal as the platform ban while none is known.
pub(crate) fn cross_feed_admission_refusal(
    admission: Res<SceneAdmission>,
    mut status: ResMut<PlatformBanStatus>,
    time: Res<Time>,
    mut events: EventWriter<BanBlockEvent>,
) {
    if !admission.is_changed() {
        return;
    }
    let SceneAdmission::Refused {
        scope: BanScope::Platform,
        message,
        ..
    } = &*admission
    else {
        return;
    };
    if status.ban().is_some() {
        return;
    }
    for event in status.apply_platform_ban(
        Some(PlatformBanInfo::from_refusal(message)),
        PlatformBanSource::SceneAdmissionRefusal,
        time.elapsed_secs_f64(),
    ) {
        events.write(event);
    }
}

/// The events that take `previous` to `current`: one per address that entered or left a set,
/// and one when the sets appeared or disappeared.
pub(crate) fn diff_blocks(
    previous: Option<&(HashSet<Address>, HashSet<Address>)>,
    current: Option<(&HashSet<Address>, &HashSet<Address>)>,
) -> Vec<BanBlockEvent> {
    let empty = HashSet::new();
    let (prev_blocked, prev_blocked_by) = previous.map(|(a, b)| (a, b)).unwrap_or((&empty, &empty));
    let (cur_blocked, cur_blocked_by) = current.unwrap_or((&empty, &empty));

    let mut events = Vec::new();
    if previous.is_some() != current.is_some() {
        events.push(BanBlockEvent::BlocksAvailabilityChanged);
    }
    for target in cur_blocked.difference(prev_blocked) {
        events.push(BanBlockEvent::LocalUserBlocked { target: *target });
    }
    for target in prev_blocked.difference(cur_blocked) {
        events.push(BanBlockEvent::LocalUserUnblocked { target: *target });
    }
    for blocker in cur_blocked_by.difference(prev_blocked_by) {
        events.push(BanBlockEvent::LocalUserBlockedBy { blocker: *blocker });
    }
    for blocker in prev_blocked_by.difference(cur_blocked_by) {
        events.push(BanBlockEvent::LocalUserUnblockedBy { blocker: *blocker });
    }
    events
}

#[cfg(test)]
mod tests {
    use super::*;

    fn address(n: u8) -> Address {
        Address::from_slice(&[n; 20])
    }

    fn sets(blocked: &[u8], blocked_by: &[u8]) -> (HashSet<Address>, HashSet<Address>) {
        (
            blocked.iter().copied().map(address).collect(),
            blocked_by.iter().copied().map(address).collect(),
        )
    }

    #[test]
    fn the_first_snapshot_announces_availability_and_every_address() {
        let current = sets(&[1], &[2]);
        let events = diff_blocks(None, Some((&current.0, &current.1)));
        assert_eq!(
            events,
            vec![
                BanBlockEvent::BlocksAvailabilityChanged,
                BanBlockEvent::LocalUserBlocked { target: address(1) },
                BanBlockEvent::LocalUserBlockedBy {
                    blocker: address(2)
                },
            ]
        );
    }

    #[test]
    fn a_change_in_either_set_is_one_event_per_address() {
        let previous = sets(&[1, 2], &[5]);
        let current = sets(&[2, 3], &[]);
        let mut events = diff_blocks(Some(&previous), Some((&current.0, &current.1)));
        events.sort_by_key(|e| format!("{e:?}"));
        let mut expected = vec![
            BanBlockEvent::LocalUserBlocked { target: address(3) },
            BanBlockEvent::LocalUserUnblocked { target: address(1) },
            BanBlockEvent::LocalUserUnblockedBy {
                blocker: address(5),
            },
        ];
        expected.sort_by_key(|e| format!("{e:?}"));
        assert_eq!(events, expected);
    }

    #[test]
    fn equal_sets_are_no_change_and_losing_them_unblocks_everyone() {
        let same = sets(&[1], &[2]);
        assert!(diff_blocks(Some(&same), Some((&same.0, &same.1))).is_empty());
        assert!(diff_blocks(None, None).is_empty());
        let events = diff_blocks(Some(&same), None);
        assert_eq!(events.len(), 3);
        assert_eq!(events[0], BanBlockEvent::BlocksAvailabilityChanged);
    }
}
