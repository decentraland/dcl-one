//! Social badge, event delivery and Segment webhook families.
use dcl_metrics::{Descriptor, Unit};

pub const FAMILIES: &[Descriptor] = &[
    Descriptor::histogram(
        "entity_deployment_to_image_generation_duration_seconds",
        "Seconds from the profile entity timestamp until both rendered images are stored.",
        Some(Unit::Seconds),
        &[],
        &[
            0.1, 0.5, 1., 2., 5., 10., 15., 30., 60., 120., 300., 600., 1200.,
        ],
    ),
    Descriptor::histogram(
        "image_upload_duration_seconds",
        "Time to store the rendered profile image pair by write result.",
        Some(Unit::Seconds),
        &["status"],
        &[0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1., 2.5, 5., 10.],
    ),
    Descriptor::histogram(
        "snapshot_generation_duration_seconds",
        "Time to render both profile images for one entity.",
        Some(Unit::Seconds),
        &[],
        &[0.1, 0.2, 0.5, 1., 2., 3., 4., 5., 6., 10.],
    ),
    Descriptor::counter(
        "snapshot_generation_count",
        "Profile renderer calls by result, including cancellation.",
        Some(Unit::Count),
        &["status"],
    ),
    Descriptor::gauge(
        "badges_processor_cursor_gaps",
        "Outstanding gaps in a badge worker's source cursor.",
        None,
        &["source"],
    ),
    Descriptor::gauge(
        "badges_processor_cursor_oldest_gap_seconds",
        "Age of the oldest gap in a badge worker's source cursor.",
        Some(Unit::Seconds),
        &["source"],
    ),
    Descriptor::gauge(
        "badges_processor_cursor_lag",
        "Distance between a badge worker's cursor and its source head.",
        None,
        &["source"],
    ),
    Descriptor::counter(
        "badges_presence_grants_total",
        "Badge grants from presence processing.",
        Some(Unit::Count),
        &[],
    ),
    Descriptor::gauge(
        "badges_presence_waiting_events",
        "Presence events awaiting a retry.",
        None,
        &[],
    ),
    Descriptor::gauge(
        "badges_presence_queue_oldest_seconds",
        "Age of the oldest queued badge presence event.",
        Some(Unit::Seconds),
        &[],
    ),
    Descriptor::counter(
        "badges_presence_queue_age_read_failures",
        "Failed reads of badge presence queue age.",
        Some(Unit::Count),
        &[],
    ),
    Descriptor::counter(
        "badges_presence_events_total",
        "Badge presence processing attempts by outcome.",
        Some(Unit::Count),
        &["outcome"],
    ),
    Descriptor::gauge(
        "social_dapps_flags_last_refresh_timestamp_seconds",
        "Unix timestamp of the last refresh of dApps feature flags.",
        Some(Unit::Seconds),
        &[],
    ),
    Descriptor::counter(
        "social_event_delivery_dropped",
        "Social events dropped during delivery by reason.",
        Some(Unit::Count),
        &["reason"],
    ),
    Descriptor::counter(
        "social_subscription_queue_overflow",
        "Social subscription updates refused because their queue was full.",
        Some(Unit::Count),
        &[],
    ),
    Descriptor::counter(
        "handled_explorer_events_count",
        "Explorer events handled by the Segment webhook.",
        Some(Unit::Count),
        &["event_type"],
    ),
    Descriptor::summary(
        "explorer_segment_event_delay_in_seconds",
        "Delay between the explorer event timestamp and webhook handling.",
        Some(Unit::Seconds),
        &["event_type"],
    ),
    Descriptor::summary(
        "segment_webhook_event_delay_in_seconds",
        "Delay between the event timestamp and Segment webhook handling.",
        Some(Unit::Seconds),
        &["event_type"],
    ),
    Descriptor::counter(
        "handled_web_events_count",
        "Web events handled by the Segment webhook.",
        Some(Unit::Count),
        &["event_type"],
    ),
];
