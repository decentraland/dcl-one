//! Prometheus series of peer sync, under the names, types, labels and buckets that
//! snapshots-fetcher, deployments-to-sqs and the TypeScript content server's sync exported, so
//! their dashboards and alerts read this node unchanged. `docs/metrics/sync.md` maps every
//! legacy series to where it is observed here.
//!
//! Every function is a counter, gauge or histogram update through the `metrics` facade: no I/O,
//! no lock beyond the recorder's own, and a no-op when no recorder is installed.

use std::time::Duration;

use metrics::{counter, gauge, histogram};

use crate::SyncState;

/// The `source` label of a snapshot pass.
pub const SNAPSHOTS: &str = "snapshots";
/// The `source` label of a `/pointer-changes` pass or stream.
pub const POINTER_CHANGES: &str = "pointer-changes";

const DOWNLOAD_BYTES: &str = "dcl_content_download_bytes_total";
const DOWNLOAD_DURATION: &str = "dcl_content_download_duration_seconds";
const DOWNLOAD_ERRORS: &str = "dcl_content_download_errors_total";
const DOWNLOAD_HASH_ERRORS: &str = "dcl_content_download_hash_errors_total";
const CORRUPT_EVICTED: &str = "dcl_corrupt_entity_files_evicted_total";
const PROCESSED: &str = "dcl_entities_deployments_processed_total";
const STREAMED: &str = "dcl_entities_deployments_streamed_total";
const POINTER_CHANGES_RESPONSE_TIME: &str = "dcl_catalysts_pointer_changes_response_time_seconds";
const STREAM_RECONNECTIONS: &str = "dcl_deployments_stream_reconnection_count";
const STREAM_FAILURES: &str = "dcl_deployments_stream_failure_count";
const JOB_SUCCEED_RETRIES: &str = "dcl_content_download_job_succeed_retries";
const AVAILABLE_SERVERS: &str = "dcl_available_servers_histogram";
const BOOTSTRAPPING_SERVERS: &str = "dcl_bootstrapping_servers";
const SYNCING_SERVERS: &str = "dcl_syncing_servers";
const PROCESSED_SNAPSHOTS: &str = "dcl_processed_snapshots_total";

const IGNORED: &str = "dcl_ignored_sync_deployments";
const PENDING_DOWNLOADS: &str = "dcl_pending_download_gauge";
const PENDING_DEPLOYMENTS: &str = "dcl_pending_deployment_gauge";
const DEPLOYMENT_TIME: &str = "dcl_deployment_time";
const SYNC_STATE: &str = "dcl_content_server_sync_state";
const FAILED_DEPLOYMENTS: &str = "dcl_content_server_failed_deployments";

const SCHEDULE_ATTEMPTS: &str = "schedule_entity_deployment_attempt";
const ALREADY_STORED: &str = "entity_already_stored";
const SKIPPED_OLD: &str = "entity_skipped_old";
const DOWNLOAD_SUCCESS: &str = "entity_download_success";
const DOWNLOAD_FAILURE: &str = "entity_download_failure";
const DEPLOYMENT_SUCCESS: &str = "entity_deployment_success";
const DEPLOYMENT_FAILURE: &str = "entity_deployment_failure";
const QUEUE_PENDING: &str = "download_queue_pending";

/// snapshots-fetcher's request histograms.
const REQUEST_BUCKETS: [f64; 12] = [
    0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0, 30.0, 60.0,
];
/// prom-client's default, which `dcl_deployment_time` used by declaring none.
const PROM_CLIENT_DEFAULT_BUCKETS: [f64; 11] = [
    0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0,
];

/// Every sync histogram with its legacy buckets, for the exporter builder: without them the
/// exporter renders a summary.
pub const HISTOGRAM_BUCKETS: [(&str, &[f64]); 5] = [
    (DOWNLOAD_DURATION, &REQUEST_BUCKETS),
    (POINTER_CHANGES_RESPONSE_TIME, &REQUEST_BUCKETS),
    (
        JOB_SUCCEED_RETRIES,
        &[
            1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0, 15.0, 20.0, 30.0,
        ],
    ),
    (
        AVAILABLE_SERVERS,
        &[
            1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0, 11.0, 12.0, 13.0, 14.0, 15.0,
        ],
    ),
    (DEPLOYMENT_TIME, &PROM_CLIENT_DEFAULT_BUCKETS),
];

/// The entity types `@dcl/schemas` names; deployments-to-sqs reported any other as `other`.
const ENTITY_TYPES: [&str; 6] = ["scene", "wearable", "profile", "store", "emote", "outfits"];

/// `entity_type` from a peer's JSON, clamped to a known type or `other`.
pub fn entity_type_label(entity_type: &str) -> &'static str {
    ENTITY_TYPES
        .into_iter()
        .find(|known| *known == entity_type)
        .unwrap_or("other")
}

/// `remote_server`: the origin of the peer URL asked (`https://peer.example` for
/// `https://peer.example/content`), as snapshots-fetcher's `url.origin`.
pub fn remote_server(url: &str) -> String {
    url::Url::parse(url)
        .map(|url| url.origin().ascii_serialization())
        .unwrap_or_default()
}

/// Renders the series whose label sets are fixed at 0 from process start.
pub fn register_series() {
    for name in [IGNORED, CORRUPT_EVICTED] {
        counter!(name).increment(0);
    }
    for state in ["stream_end", "saved", "incomplete"] {
        counter!(PROCESSED_SNAPSHOTS, "state" => state).increment(0);
    }
    server_phases(0, 0, 0);
    gauge!(QUEUE_PENDING).set(0.0);
}

/// One `GET /contents/{hash}` from `remote_server`, over its whole body: `bytes` decoded once it
/// arrived whole, `None` when the request, its status or its body failed.
pub fn download_finished(remote_server: &str, elapsed: Duration, bytes: Option<u64>) {
    let labels = [("remote_server", remote_server.to_string())];
    histogram!(DOWNLOAD_DURATION, &labels).record(elapsed.as_secs_f64());
    match bytes {
        Some(bytes) => counter!(DOWNLOAD_BYTES, &labels).increment(bytes),
        None => counter!(DOWNLOAD_ERRORS, &labels).increment(1),
    }
}

/// A body that arrived whole but does not hash to its name.
pub fn download_hash_error(remote_server: &str) {
    counter!(DOWNLOAD_HASH_ERRORS, "remote_server" => remote_server.to_string()).increment(1);
}

/// A file download job starts with `servers` candidate peers.
pub fn download_job_started(servers: usize) {
    histogram!(AVAILABLE_SERVERS).record(servers as f64);
}

/// A file download job stored its file on its `attempts`th request.
pub fn download_job_succeeded(attempts: u32) {
    histogram!(JOB_SUCCEED_RETRIES).record(f64::from(attempts));
}

pub fn corrupt_entity_file_evicted() {
    counter!(CORRUPT_EVICTED).increment(1);
}

/// Where a [`crate::batch_deployer::DeploymentReport`]'s entities come from.
#[derive(Debug, Clone)]
pub struct SyncSource {
    remote_server: String,
    source: &'static str,
}

impl SyncSource {
    /// A snapshot has no single origin: several peers advertise it, so `remote_server` is empty.
    pub fn snapshots() -> Self {
        Self {
            remote_server: String::new(),
            source: SNAPSHOTS,
        }
    }

    pub fn pointer_changes(server: &str) -> Self {
        Self {
            remote_server: remote_server(server),
            source: POINTER_CHANGES,
        }
    }

    fn labels(&self) -> [(&'static str, String); 2] {
        [
            ("remote_server", self.remote_server.clone()),
            ("source", self.source.to_string()),
        ]
    }

    /// An entity read from the source and handed to the deployer.
    pub fn streamed(&self, entity_type: &str) {
        counter!(STREAMED, &self.labels()).increment(1);
        counter!(SCHEDULE_ATTEMPTS, "entityType" => entity_type_label(entity_type)).increment(1);
    }

    /// An entity accounted for: deployed, recorded as failed, or skipped.
    pub fn processed(&self) {
        counter!(PROCESSED, &self.labels()).increment(1);
    }
}

/// Why the deployer skipped an announced entity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Skip {
    /// A type outside `SYNC_ENTITY_TYPES`.
    IgnoredType,
    /// A profile older than `SYNC_PROFILE_MAX_AGE_DAYS`.
    OldProfile,
    AlreadyDeployed,
}

pub fn skipped(entity_type: &str, skip: Skip) {
    counter!(IGNORED).increment(1);
    let entity_type = entity_type_label(entity_type);
    match skip {
        Skip::IgnoredType => {}
        Skip::OldProfile => counter!(SKIPPED_OLD, "entityType" => entity_type).increment(1),
        Skip::AlreadyDeployed => counter!(ALREADY_STORED, "entityType" => entity_type).increment(1),
    }
}

/// Counts up while it lives: an entity the batch deployer holds a queue slot for
/// (`dcl_pending_deployment_gauge`, `download_queue_pending`), or one whose entity and content
/// files are downloading (`dcl_pending_download_gauge`).
pub struct InFlight {
    gauge: &'static str,
    entity_type: &'static str,
}

impl InFlight {
    pub fn deployment(entity_type: &str) -> Self {
        gauge!(QUEUE_PENDING).increment(1.0);
        Self::labeled(PENDING_DEPLOYMENTS, entity_type)
    }

    pub fn download(entity_type: &str) -> Self {
        Self::labeled(PENDING_DOWNLOADS, entity_type)
    }

    fn labeled(gauge: &'static str, entity_type: &str) -> Self {
        let entity_type = entity_type_label(entity_type);
        gauge!(gauge, "entity_type" => entity_type).increment(1.0);
        Self { gauge, entity_type }
    }
}

impl Drop for InFlight {
    fn drop(&mut self) {
        if self.gauge == PENDING_DEPLOYMENTS {
            gauge!(QUEUE_PENDING).decrement(1.0);
        }
        gauge!(self.gauge, "entity_type" => self.entity_type).decrement(1.0);
    }
}

/// The download of an entity file and its content files ended: `Ok` with everything stored,
/// or the failure. Every failure is retried (the failed-deployments record, or the read-through's
/// next miss), so `retryable` is always `true`.
pub fn entity_downloaded(entity_type: &str, ok: bool) {
    let entity_type = entity_type_label(entity_type);
    if ok {
        counter!(DOWNLOAD_SUCCESS, "entityType" => entity_type).increment(1);
    } else {
        counter!(DOWNLOAD_FAILURE, "entityType" => entity_type, "retryable" => "true").increment(1);
    }
}

/// A downloaded entity's validation and hand-off to the batch writer ended after `elapsed`.
pub fn entity_deployed(entity_type: &str, elapsed: Duration, ok: bool) {
    let entity_type = entity_type_label(entity_type);
    let failed = if ok { "false" } else { "true" };
    histogram!(DEPLOYMENT_TIME, "entity_type" => entity_type, "failed" => failed)
        .record(elapsed.as_secs_f64());
    if ok {
        counter!(DEPLOYMENT_SUCCESS, "entityType" => entity_type).increment(1);
    } else {
        counter!(DEPLOYMENT_FAILURE, "entityType" => entity_type, "retryable" => "true")
            .increment(1);
    }
}

/// One `/pointer-changes` page request to `server`, answered or not.
pub fn pointer_changes_page(server: &str, elapsed: Duration) {
    histogram!(POINTER_CHANGES_RESPONSE_TIME, "remote_server" => remote_server(server))
        .record(elapsed.as_secs_f64());
}

/// A steady-state `/pointer-changes` stream to `server` (re)starts.
pub fn stream_connected(server: &str) {
    counter!(STREAM_RECONNECTIONS, "remote_server" => remote_server(server)).increment(1);
}

/// A steady-state `/pointer-changes` stream to `server` ended in an error.
pub fn stream_failed(server: &str) {
    counter!(STREAM_FAILURES, "remote_server" => remote_server(server)).increment(1);
}

/// The peers in each sync phase.
pub fn server_phases(from_snapshots: usize, from_pointer_changes: usize, syncing: usize) {
    gauge!(BOOTSTRAPPING_SERVERS, "from" => SNAPSHOTS).set(from_snapshots as f64);
    gauge!(BOOTSTRAPPING_SERVERS, "from" => POINTER_CHANGES).set(from_pointer_changes as f64);
    gauge!(SYNCING_SERVERS).set(syncing as f64);
}

/// `stream_end`: a snapshot file was read to its end; `incomplete`: it had unreadable lines;
/// `saved`: it was marked processed.
pub fn snapshots_processed(state: &'static str, snapshots: u64) {
    counter!(PROCESSED_SNAPSHOTS, "state" => state).increment(snapshots);
}

/// Legacy's 0 (bootstrapping) and 1 (syncing). A partially synced node is still bootstrapping
/// its profiles, and a failed one is not syncing: both are 0.
pub fn sync_state(state: &SyncState) {
    let value = if *state == SyncState::Syncing {
        1.0
    } else {
        0.0
    };
    gauge!(SYNC_STATE).set(value);
}

/// The rows of `content.failed_deployments`.
pub fn failed_deployments(rows: u64) {
    gauge!(FAILED_DEPLOYMENTS).set(rows as f64);
}

/// A recorder for this thread until the guard drops, and its handle to render.
#[cfg(test)]
pub(crate) fn local_recorder() -> (
    metrics::LocalRecorderGuard<'static>,
    metrics_exporter_prometheus::PrometheusHandle,
) {
    let recorder = metrics_exporter_prometheus::PrometheusBuilder::new().build_recorder();
    let handle = recorder.handle();
    let recorder: &'static _ = Box::leak(Box::new(recorder));
    (metrics::set_default_local_recorder(recorder), handle)
}

/// The value of the sample `series` (name and labels as rendered), or `None` when absent.
#[cfg(test)]
pub(crate) fn sample(rendered: &str, series: &str) -> Option<f64> {
    rendered
        .lines()
        .find_map(|line| line.strip_prefix(series)?.strip_prefix(' ')?.parse().ok())
}

#[cfg(test)]
mod tests;
