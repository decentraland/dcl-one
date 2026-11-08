//! What the main and render worlds share about the video players.

use std::{
    collections::HashMap,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering},
    },
};

use bevy::prelude::*;
use parking_lot::Mutex;

use super::ffi::PlayerId;

/// A video player's target image and how many frames the render world has presented into it.
pub(super) struct Target {
    pub image: AssetId<Image>,
    pub presented: AtomicU64,
    /// The size of the planes the render world presents, [`pack_size`](super::size::pack_size)d, once it has read them;
    /// 0 before.
    pub presented_size: AtomicU64,
    /// Set by the render world when the player's frames cannot be presented (a size the device
    /// refuses).
    pub failed: AtomicBool,
}

/// What the main and render worlds share: the video players to present, and the closed ones
/// the render world still has to let go of.
#[derive(Default)]
pub(super) struct VideoPresentation {
    pub targets: Mutex<HashMap<PlayerId, Arc<Target>>>,
    /// Closed players whose planes the render world may still be reading, left for it to free.
    retired: Mutex<Vec<PlayerId>>,
    /// Whether a render world presents frames.
    pub render_world: AtomicBool,
    /// The render device's texture edge limit, once the render world has read it; 0 before.
    pub max_dimension: AtomicU32,
}

impl VideoPresentation {
    pub fn take_retired(&self) -> Vec<PlayerId> {
        std::mem::take(&mut *self.retired.lock())
    }

    /// Ends a player's presentation and frees it: at once when no render world can be reading
    /// its planes, otherwise from the render world ([`take_retired`](Self::take_retired)).
    /// Returns whether the free was deferred.
    pub(super) fn retire(&self, id: PlayerId) -> bool {
        let had_target = self.targets.lock().remove(&id).is_some();
        if had_target && self.render_world.load(Ordering::Acquire) {
            self.retired.lock().push(id);
            true
        } else {
            uuav::uuav_player_free(id);
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn target() -> Arc<Target> {
        Arc::new(Target {
            image: AssetId::default(),
            presented: AtomicU64::new(0),
            presented_size: AtomicU64::new(0),
            failed: AtomicBool::new(false),
        })
    }

    #[test]
    fn without_a_render_world_a_closed_player_is_freed_at_once() {
        let presentation = VideoPresentation::default();
        presentation.targets.lock().insert(7, target());
        assert!(!presentation.retire(7));
        assert!(presentation.targets.lock().is_empty());
        assert!(presentation.take_retired().is_empty());
    }

    #[test]
    fn under_a_render_world_the_free_waits_for_it() {
        let presentation = VideoPresentation::default();
        presentation.render_world.store(true, Ordering::Release);
        presentation.targets.lock().insert(7, target());
        assert!(presentation.retire(7));
        assert!(presentation.targets.lock().is_empty());
        assert_eq!(presentation.take_retired(), vec![7]);
        assert!(presentation.take_retired().is_empty());
    }

    #[test]
    fn a_player_without_frames_is_freed_at_once() {
        let presentation = VideoPresentation::default();
        presentation.render_world.store(true, Ordering::Release);
        assert!(!presentation.retire(9));
        assert!(presentation.take_retired().is_empty());
    }
}
