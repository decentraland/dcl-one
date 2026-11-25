use crate::{
    badges::report_badge_events,
    crash::refresh_crash_snapshot,
    frame::FrameMetricPlugin,
    in_world::report_in_world,
    loading::report_loading_screen,
    primary_player_parcel_position::report_parcel_changes,
    reactions::{report_reaction_visualization, report_reactions},
    segment_system::SegmentMetricPlugin,
};
use bevy::prelude::*;
use common::structs::{
    PlayerEmoteTriggered, PlayerParcelChanged, PlayerReacted, PlayerTeleported, ReturnedToWorld,
};
pub struct MetricsPlugin;

impl Plugin for MetricsPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((FrameMetricPlugin, SegmentMetricPlugin));
        // written by restricted_actions, avatar and scene_runner; registered here too so the readers
        // never depend on them
        app.add_event::<ReturnedToWorld>();
        app.add_event::<PlayerEmoteTriggered>();
        app.add_event::<PlayerReacted>();
        app.add_event::<PlayerTeleported>();
        app.add_event::<PlayerParcelChanged>();
        app.add_systems(
            Update,
            (
                report_parcel_changes,
                report_in_world,
                report_loading_screen,
                refresh_crash_snapshot,
                report_badge_events,
                report_reactions,
                report_reaction_visualization
                    .run_if(resource_exists_and_changed::<common::structs::AppConfig>),
            ),
        );
        app.add_plugins((
            crate::page_events::PageEventsPlugin,
            crate::feature_flags::FeatureFlagsReportPlugin,
        ));
    }
}
