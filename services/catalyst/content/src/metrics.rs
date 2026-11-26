//! Content write, peer sync and history pruning metrics exposed by serve, under dcl-one's own
//! names. The families that keep a legacy content server series are in [`crate::series`].
use crate::{history_prune::*, sync::retry_failed::UNDECIDED_RECORDS};
use dcl_metrics::{CounterFamily, Descriptor, GaugeFamily, Unit};

pub(crate) const SPOOL_FAILURES: CounterFamily<1> = CounterFamily::new(
    "upload_spool_failures_total",
    "Upload spool failures by content route.",
    None,
    &["route"],
);
pub(crate) const REFUSED_BYTES: &str = "upload_budget_refused_received_bytes";
pub(crate) const SYNCED_SCENE_PARCELS_DIFFER: &str = "content_sync_scene_parcels_differ_total";

pub(crate) const OVERSIZED_POINTER_CHANGES: CounterFamily<0> = CounterFamily::new(
    "content_sync_oversized_pointer_changes_total",
    "Peer pointer changes rejected for exceeding a sync delta limit.",
    None,
    &[],
);
pub(crate) const MANIFEST_REJECTIONS: CounterFamily<1> = CounterFamily::new(
    "content_sync_manifest_rejections_total",
    "Peer content manifests rejected during parsing, by reason.",
    None,
    &["reason"],
);

pub(crate) const SYNC_HEARTBEAT: GaugeFamily<0> = GaugeFamily::new(
    "content_sync_last_heartbeat_timestamp_seconds",
    "Unix timestamp of the last content sync heartbeat persisted by this process.",
    Some(Unit::Seconds),
    &[],
);

pub const FAMILIES: &[Descriptor] = &[
    SPOOL_FAILURES.descriptor,
    SYNC_HEARTBEAT.descriptor,
    OVERSIZED_POINTER_CHANGES.descriptor,
    MANIFEST_REJECTIONS.descriptor,
    Descriptor::summary(
        REFUSED_BYTES,
        "Payload bytes received before upload budget refusal.",
        Some(Unit::Bytes),
        &["route"],
    ),
    Descriptor::counter(
        SYNCED_SCENE_PARCELS_DIFFER,
        "Synced scenes with differing parcel sets.",
        None,
        &[],
    ),
    Descriptor::gauge(
        UNDECIDED_RECORDS,
        "Failed deployments awaiting a retry decision.",
        None,
        &[],
    ),
    Descriptor::counter(
        DEPLOYMENTS_PRUNED,
        "Deployment rows removed by history pruning.",
        None,
        &[],
    ),
    Descriptor::counter(
        CONTENT_FILES_PRUNED,
        "Content file rows removed by history pruning.",
        None,
        &[],
    ),
    Descriptor::counter(
        BLOBS_UNLINKED,
        "Blobs unlinked by history pruning.",
        None,
        &[],
    ),
    Descriptor::counter(
        BLOBS_KEPT,
        "Blobs retained during history pruning.",
        None,
        &[],
    ),
    Descriptor::counter(
        BLOB_FAILURES,
        "Failed blob operations during history pruning.",
        None,
        &[],
    ),
    Descriptor::counter(
        BYTES_FREED,
        "Bytes freed by history pruning.",
        Some(Unit::Bytes),
        &[],
    ),
];
