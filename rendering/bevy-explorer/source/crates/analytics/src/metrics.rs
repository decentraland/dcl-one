use crate::{
    badges::report_badge_events, crash::refresh_crash_snapshot, frame::FrameMetricPlugin,
    in_world::report_in_world, loading::report_loading_screen,
    primary_player_parcel_position::primary_player_parcel_position_system,
    segment_system::SegmentMetricPlugin,
};
use bevy::prelude::*;
use common::structs::{PlayerEmoteTriggered, PlayerTeleported, ReturnedToWorld};
pub struct MetricsPlugin;

impl Plugin for MetricsPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((FrameMetricPlugin, SegmentMetricPlugin));
        // written by restricted_actions and avatar; registered here too so the readers never depend on them
        app.add_event::<ReturnedToWorld>();
        app.add_event::<PlayerEmoteTriggered>();
        app.add_event::<PlayerTeleported>();
        app.add_systems(
            Update,
            (
                primary_player_parcel_position_system,
                report_in_world,
                report_loading_screen,
                refresh_crash_snapshot,
                report_badge_events,
            ),
        );
        app.add_plugins((
            crate::page_events::PageEventsPlugin,
            crate::feature_flags::FeatureFlagsReportPlugin,
        ));
    }
}
