//! What avatar processing (`process_avatar`) may take of a frame, and what it took.
//!
//! The renderer reads [`AvatarProcessingBudget`] and reports into it. Nothing limits it until
//! something sets `room`, so the renderer alone processes every ready avatar. The Explorer
//! measures each frame's processing ([`AvatarProcessingSlicePlugin`]).

use std::time::Duration;

use bevy::prelude::*;

/// A frame's avatar processing: what it may take and what the last run took.
#[derive(Resource, Default, Clone, Copy, Debug, PartialEq)]
pub struct AvatarProcessingBudget {
    /// What processing may still take this frame, set before `process_avatar` by whoever budgets
    /// the frame. `None`: nothing limits it.
    pub room: Option<Duration>,
    /// How long the last run took.
    pub spent: Duration,
    /// The avatars the last run processed.
    pub processed: usize,
    /// Whether the last run stopped with avatars left unvisited.
    pub stopped: bool,
}

#[cfg(feature = "explorer")]
pub(crate) use explorer::AvatarProcessingSlicePlugin;

#[cfg(feature = "explorer")]
mod explorer {
    use std::time::Duration;

    use bevy::{diagnostic::Diagnostics, prelude::*};
    use scene_runner::scene_spawn_slice::spawn_timing::AVATAR_PROCESSING;

    use super::AvatarProcessingBudget;
    use crate::process_avatar;

    /// A frame's avatar processing is logged from this long.
    const LOGGED_FROM: Duration = Duration::from_millis(1);

    pub(crate) struct AvatarProcessingSlicePlugin;

    impl Plugin for AvatarProcessingSlicePlugin {
        fn build(&self, app: &mut App) {
            app.add_systems(Update, charge_avatar_processing.after(process_avatar));
        }
    }

    /// Measures what `process_avatar` took this frame; with `spawn_timing=debug` in the log
    /// filter, logs a frame that took a millisecond or more.
    fn charge_avatar_processing(
        avatars: Res<AvatarProcessingBudget>,
        mut diagnostics: Diagnostics,
    ) {
        let milliseconds = avatars.spent.as_secs_f64() * 1000.0;
        diagnostics.add_measurement(&AVATAR_PROCESSING, || milliseconds);
        if avatars.spent >= LOGGED_FROM {
            debug!(
                target: "spawn_timing",
                "avatar processing this frame: {milliseconds:.1} ms for {} avatars{}",
                avatars.processed,
                if avatars.stopped { ", more wait" } else { "" },
            );
        }
    }
}

#[cfg(test)]
mod tests;
