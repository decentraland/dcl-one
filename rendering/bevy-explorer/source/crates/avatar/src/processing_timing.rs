//! What avatar processing (`process_avatar`) takes of a frame.
//!
//! The renderer reports each run into [`AvatarProcessingRun`]. Its timing plugin records the
//! result in the `instantiation/avatar_processing` diagnostic.
//! `docs/guidelines/decisions.md` explains why avatar processing has no frame budget.

use std::time::Duration;

use bevy::{
    diagnostic::{Diagnostic, Diagnostics, RegisterDiagnostic},
    prelude::*,
};
use common::perf::AVATAR_PROCESSING;

/// The last run of `process_avatar`.
#[derive(Resource, Default)]
pub(crate) struct AvatarProcessingRun {
    /// How long it took.
    pub(crate) spent: Duration,
    /// The avatars it processed.
    pub(crate) processed: usize,
}

/// A frame's avatar processing is logged from this long.
const LOGGED_FROM: Duration = Duration::from_millis(1);

pub(crate) struct AvatarProcessingTimingPlugin;

impl Plugin for AvatarProcessingTimingPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<AvatarProcessingRun>()
            .register_diagnostic(Diagnostic::new(AVATAR_PROCESSING).with_suffix("ms"))
            // `process_avatar` ran in `Update`: no ordering against it is needed here
            .add_systems(PostUpdate, measure_avatar_processing);
    }
}

/// Measures what `process_avatar` took this frame; with `spawn_timing=debug` in the log
/// filter, logs a frame that took a millisecond or more.
fn measure_avatar_processing(run: Res<AvatarProcessingRun>, mut diagnostics: Diagnostics) {
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

#[cfg(test)]
mod tests;
