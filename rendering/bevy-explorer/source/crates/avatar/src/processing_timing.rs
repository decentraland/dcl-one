//! What avatar processing (`process_avatar`) takes of a frame.
//!
//! The renderer reports each run into [`AvatarProcessingRun`]. The Explorer measures it into the
//! `instantiation/avatar_processing` diagnostic ([`AvatarProcessingTimingPlugin`]). Nothing limits
//! avatar processing: `docs/guidelines/decisions.md` has the measurement behind that.

use std::time::Duration;

use bevy::prelude::*;

/// The last run of `process_avatar`.
#[derive(Resource, Default, Clone, Copy, Debug, PartialEq)]
pub struct AvatarProcessingRun {
    /// How long it took.
    pub spent: Duration,
    /// The avatars it processed.
    pub processed: usize,
}

#[cfg(feature = "explorer")]
pub(crate) use explorer::AvatarProcessingTimingPlugin;

#[cfg(feature = "explorer")]
mod explorer {
    use std::time::Duration;

    use bevy::{diagnostic::Diagnostics, prelude::*};
    use scene_runner::scene_spawn_slice::spawn_timing::AVATAR_PROCESSING;

    use super::AvatarProcessingRun;
    use crate::process_avatar;

    /// A frame's avatar processing is logged from this long.
    const LOGGED_FROM: Duration = Duration::from_millis(1);

    pub(crate) struct AvatarProcessingTimingPlugin;

    impl Plugin for AvatarProcessingTimingPlugin {
        fn build(&self, app: &mut App) {
            app.add_systems(Update, measure_avatar_processing.after(process_avatar));
        }
    }

    /// Measures what `process_avatar` took this frame; with `spawn_timing=debug` in the log
    /// filter, logs a frame that took a millisecond or more.
    pub(super) fn measure_avatar_processing(
        run: Res<AvatarProcessingRun>,
        mut diagnostics: Diagnostics,
    ) {
        let milliseconds = run.spent.as_secs_f64() * 1000.0;
        diagnostics.add_measurement(&AVATAR_PROCESSING, || milliseconds);
        if run.spent >= LOGGED_FROM {
            debug!(
                target: "spawn_timing",
                "avatar processing this frame: {milliseconds:.1} ms for {} avatars",
                run.processed,
            );
        }
    }
}

#[cfg(test)]
mod tests;
