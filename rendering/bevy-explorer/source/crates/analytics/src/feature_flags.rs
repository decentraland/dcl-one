//! The session's rollout flags (`common::feature_flags`) as analytics and the console see them:
//! one `Feature Flags Resolved` event once the read settles, and `/feature_flags` with its debug
//! panel section for checking the read end to end.

use bevy::prelude::*;
use bevy_console::ConsoleCommand;
use common::feature_flags::{FeatureFlags, FlagsSource, FLAGS};
use console::{AddDebugSection, DebugSection, DoAddConsoleCommand};

use crate::{
    data_definition::{SegmentEvent, SegmentEventFeatureFlagsResolved},
    segment_system::{SegmentConfig, SegmentMetricsEvents},
};

pub(crate) struct FeatureFlagsReportPlugin;

impl Plugin for FeatureFlagsReportPlugin {
    fn build(&self, app: &mut App) {
        app.add_console_command::<FeatureFlagsCommand, _>(feature_flags_command)
            .add_debug_section(DebugSection {
                title: "Feature flags",
                query: "/feature_flags",
                fields: &[
                    ("Source", "source"),
                    ("Flags on", "report"),
                    ("Launch override", "override"),
                ],
                actions: &[],
                inputs: &[],
            })
            .add_systems(
                Update,
                report_resolved.run_if(resource_changed::<FeatureFlags>),
            );
    }
}

/// Queues the event once, when the read leaves `Pending`; runs only when the flags change.
fn report_resolved(
    flags: Res<FeatureFlags>,
    mut metrics: ResMut<SegmentMetricsEvents>,
    config: Res<SegmentConfig>,
    mut reported: Local<bool>,
) {
    if *reported || flags.source() == FlagsSource::Pending {
        return;
    }
    *reported = true;
    metrics.add_event(
        SegmentEvent::FeatureFlagsResolved(SegmentEventFeatureFlagsResolved {
            feature_flags: flags.report(),
            feature_flags_override: flags.overrides_report(),
            feature_flags_source: flags.source().as_str().to_owned(),
        }),
        config.common(),
    );
}

/// Show where the session's feature flags came from and how each flag this client reads resolved.
#[derive(clap::Parser, ConsoleCommand)]
#[command(name = "/feature_flags")]
struct FeatureFlagsCommand;

fn feature_flags_command(mut cmd: ConsoleCommand<FeatureFlagsCommand>, flags: Res<FeatureFlags>) {
    if cmd.take().is_none() {
        return;
    }
    let on: serde_json::Map<String, serde_json::Value> = FLAGS
        .iter()
        .map(|name| ((*name).to_owned(), flags.is_on(name).into()))
        .collect();
    let variants: serde_json::Map<String, serde_json::Value> = FLAGS
        .iter()
        .filter_map(|name| {
            Some((
                (*name).to_owned(),
                serde_json::to_value(flags.variant(name)?).ok()?,
            ))
        })
        .collect();
    cmd.reply_ok(
        serde_json::json!({
            "source": flags.source().as_str(),
            "report": flags.report(),
            "override": flags.overrides_report(),
            "flags": on,
            "variants": variants,
        })
        .to_string(),
    );
}
