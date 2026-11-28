//! Peer sync families, under the names, types, labels and buckets that snapshots-fetcher,
//! deployments-to-sqs and the TypeScript content server's sync exported, so their dashboards
//! and alerts read this node unchanged. `catalyst serve` lists [`FAMILIES`] in its registry,
//! which applies the declared buckets. `docs/metrics/sync.md` maps every legacy series to where
//! it is observed here.
//!
//! Every label value comes from a closed set or from a configured peer's origin. Recording only
//! touches the process recorder, a no-op without one, so it never fails or blocks a job.

use crate::shared::casts::ApproxF64;
use std::time::Duration;

use dcl_metrics::{CounterFamily, Descriptor, GaugeFamily, HistogramFamily, SharedString};

use crate::SyncState;
use crate::series::{DEFAULT_BUCKETS, ENTITY_TYPES, counter, gauge};

/// The `source` label of a snapshot pass.
pub const SNAPSHOTS: &str = "snapshots";
/// The `source` label of a `/pointer-changes` pass or stream.
pub const POINTER_CHANGES: &str = "pointer-changes";

/// snapshots-fetcher's request histograms.
const REQUEST_BUCKETS: &[f64] = &[
    0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0, 30.0, 60.0,
];

const fn histogram<const N: usize>(
    name: &'static str,
    help: &'static str,
    keys: &'static [&'static str; N],
    buckets: &'static [f64],
) -> HistogramFamily<N> {
    HistogramFamily::new(name, help, None, keys, buckets)
}

const DOWNLOAD_BYTES: CounterFamily<1> = counter(
    "dcl_content_download_bytes_total",
    "Total downloaded bytes from other catalysts",
    &["remote_server"],
);
const DOWNLOAD_DURATION: HistogramFamily<1> = histogram(
    "dcl_content_download_duration_seconds",
    "Total download time from other catalysts",
    &["remote_server"],
    REQUEST_BUCKETS,
);
const DOWNLOAD_ERRORS: CounterFamily<1> = counter(
    "dcl_content_download_errors_total",
    "Total downloaded errors in requests",
    &["remote_server"],
);
const DOWNLOAD_HASH_ERRORS: CounterFamily<1> = counter(
    "dcl_content_download_hash_errors_total",
    "Total hasing errors in downloaded files",
    &["remote_server"],
);
const CORRUPT_EVICTED: CounterFamily<0> = counter(
    "dcl_corrupt_entity_files_evicted_total",
    "Total stored entity files evicted after failing content-hash verification",
    &[],
);
const PROCESSED: CounterFamily<2> = counter(
    "dcl_entities_deployments_processed_total",
    "Entities processed from remote catalysts",
    &["remote_server", "source"],
);
const STREAMED: CounterFamily<2> = counter(
    "dcl_entities_deployments_streamed_total",
    "Entities streamed from remote catalysts",
    &["remote_server", "source"],
);
const POINTER_CHANGES_RESPONSE_TIME: HistogramFamily<1> = histogram(
    "dcl_catalysts_pointer_changes_response_time_seconds",
    "Counts the connection of a deployment stream",
    &["remote_server"],
    REQUEST_BUCKETS,
);
const STREAM_RECONNECTIONS: CounterFamily<1> = counter(
    "dcl_deployments_stream_reconnection_count",
    "Counts the connection of a deployment stream",
    &["remote_server"],
);
const STREAM_FAILURES: CounterFamily<1> = counter(
    "dcl_deployments_stream_failure_count",
    "Counts the failures of a deployment stream",
    &["remote_server"],
);
const JOB_SUCCEED_RETRIES: HistogramFamily<0> = histogram(
    "dcl_content_download_job_succeed_retries",
    "Summary of how much retries are required for a download job to succeed",
    &[],
    &[
        1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0, 15.0, 20.0, 30.0,
    ],
);
const AVAILABLE_SERVERS: HistogramFamily<0> = histogram(
    "dcl_available_servers_histogram",
    "Histogram of available content servers in which a content file is present",
    &[],
    &[
        1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0, 11.0, 12.0, 13.0, 14.0, 15.0,
    ],
);
const BOOTSTRAPPING_SERVERS: GaugeFamily<1> = gauge(
    "dcl_bootstrapping_servers",
    "Servers that are in bootstrapping state",
    &["from"],
);
const SYNCING_SERVERS: GaugeFamily<0> = gauge(
    "dcl_syncing_servers",
    "Servers that are in syncing state",
    &[],
);
const PROCESSED_SNAPSHOTS: CounterFamily<1> = counter(
    "dcl_processed_snapshots_total",
    "Total number of processed snapshots that started being streamed.",
    &["state"],
);

const IGNORED: CounterFamily<0> = counter(
    "dcl_ignored_sync_deployments",
    "Entities ignored during the synchronization and bootstrapping",
    &[],
);
const PENDING_DOWNLOADS: GaugeFamily<1> = gauge(
    "dcl_pending_download_gauge",
    "Pending downloading jobs",
    &["entity_type"],
);
const PENDING_DEPLOYMENTS: GaugeFamily<1> = gauge(
    "dcl_pending_deployment_gauge",
    "Pending deployments",
    &["entity_type"],
);
const DEPLOYMENT_TIME: HistogramFamily<2> = histogram(
    "dcl_deployment_time",
    "Time spent deploying an entity",
    &["entity_type", "failed"],
    DEFAULT_BUCKETS,
);
const SYNC_STATE: GaugeFamily<0> = gauge(
    "dcl_content_server_sync_state",
    "Content server sync state.",
    &[],
);
const FAILED_DEPLOYMENTS: GaugeFamily<0> = gauge(
    "dcl_content_server_failed_deployments",
    "Failed deployments.",
    &[],
);

const SCHEDULE_ATTEMPTS: CounterFamily<1> = counter(
    "schedule_entity_deployment_attempt",
    "Count attempts to schedule entity deployment",
    &["entityType"],
);
const ALREADY_STORED: CounterFamily<1> = counter(
    "entity_already_stored",
    "Count entities already stored",
    &["entityType"],
);
const SKIPPED_OLD: CounterFamily<1> = counter(
    "entity_skipped_old",
    "Count entities skipped because they are older than the configured max age",
    &["entityType"],
);
const DOWNLOAD_SUCCESS: CounterFamily<1> = counter(
    "entity_download_success",
    "Count successful entity downloads",
    &["entityType"],
);
const DOWNLOAD_FAILURE: CounterFamily<2> = counter(
    "entity_download_failure",
    "Count failed entity downloads",
    &["entityType", "retryable"],
);
const DEPLOYMENT_SUCCESS: CounterFamily<1> = counter(
    "entity_deployment_success",
    "Count successful entity deployments",
    &["entityType"],
);
const DEPLOYMENT_FAILURE: CounterFamily<2> = counter(
    "entity_deployment_failure",
    "Count failed entity deployments",
    &["entityType", "retryable"],
);
const QUEUE_PENDING: GaugeFamily<0> = gauge(
    "download_queue_pending",
    "Deployment jobs currently executing. Persistently at the queue concurrency means saturated.",
    &[],
);

/// Every peer sync family, for the serve registry.
pub const FAMILIES: &[Descriptor] = &[
    DOWNLOAD_BYTES.descriptor,
    DOWNLOAD_DURATION.descriptor,
    DOWNLOAD_ERRORS.descriptor,
    DOWNLOAD_HASH_ERRORS.descriptor,
    CORRUPT_EVICTED.descriptor,
    PROCESSED.descriptor,
    STREAMED.descriptor,
    POINTER_CHANGES_RESPONSE_TIME.descriptor,
    STREAM_RECONNECTIONS.descriptor,
    STREAM_FAILURES.descriptor,
    JOB_SUCCEED_RETRIES.descriptor,
    AVAILABLE_SERVERS.descriptor,
    BOOTSTRAPPING_SERVERS.descriptor,
    SYNCING_SERVERS.descriptor,
    PROCESSED_SNAPSHOTS.descriptor,
    IGNORED.descriptor,
    PENDING_DOWNLOADS.descriptor,
    PENDING_DEPLOYMENTS.descriptor,
    DEPLOYMENT_TIME.descriptor,
    SYNC_STATE.descriptor,
    FAILED_DEPLOYMENTS.descriptor,
    SCHEDULE_ATTEMPTS.descriptor,
    ALREADY_STORED.descriptor,
    SKIPPED_OLD.descriptor,
    DOWNLOAD_SUCCESS.descriptor,
    DOWNLOAD_FAILURE.descriptor,
    DEPLOYMENT_SUCCESS.descriptor,
    DEPLOYMENT_FAILURE.descriptor,
    QUEUE_PENDING.descriptor,
];

/// `entityType` (the deployments-to-sqs families) from a peer's JSON, clamped to one of
/// [`ENTITY_TYPES`] or, as deployments-to-sqs did, `other`. The content server's `entity_type`
/// label goes through [`crate::series::entity_type`].
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

/// One `GET /contents/{hash}` from `remote_server`, over its whole body: `bytes` decoded once it
/// arrived whole, `None` when the request, its status or its body failed.
pub fn download_finished(remote_server: &str, elapsed: Duration, bytes: Option<u64>) {
    let label = || [SharedString::from(remote_server.to_owned())];
    DOWNLOAD_DURATION
        .handle_owned(label())
        .record(elapsed.as_secs_f64());
    match bytes {
        Some(bytes) => DOWNLOAD_BYTES.handle_owned(label()).increment(bytes),
        None => DOWNLOAD_ERRORS.handle_owned(label()).increment(1),
    }
}

/// A read-through probe that `remote_server` answered 404: timed like any request, not counted
/// as an error, because a client can name any hash.
pub fn download_missed(remote_server: &str, elapsed: Duration) {
    DOWNLOAD_DURATION
        .handle_owned([SharedString::from(remote_server.to_owned())])
        .record(elapsed.as_secs_f64());
}

/// A body that arrived whole but does not hash to its name.
pub fn download_hash_error(remote_server: &str) {
    DOWNLOAD_HASH_ERRORS
        .handle_owned([remote_server.to_owned().into()])
        .increment(1);
}

/// A file download job starts with `servers` candidate peers.
pub fn download_job_started(servers: usize) {
    AVAILABLE_SERVERS.handle([]).record(servers.approx_f64());
}

/// A file download job stored its file on its `attempts`th request.
pub fn download_job_succeeded(attempts: u32) {
    JOB_SUCCEED_RETRIES.handle([]).record(f64::from(attempts));
}

pub fn corrupt_entity_file_evicted() {
    CORRUPT_EVICTED.handle([]).increment(1);
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

    fn labels(&self) -> [SharedString; 2] {
        [
            self.remote_server.clone().into(),
            SharedString::const_str(self.source),
        ]
    }

    /// An entity read from the source and handed to the deployer.
    pub fn streamed(&self, entity_type: &str) {
        STREAMED.handle_owned(self.labels()).increment(1);
        SCHEDULE_ATTEMPTS
            .handle([entity_type_label(entity_type)])
            .increment(1);
    }

    /// An entity accounted for: deployed, recorded as failed, or skipped.
    pub fn processed(&self) {
        PROCESSED.handle_owned(self.labels()).increment(1);
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
    IGNORED.handle([]).increment(1);
    let entity_type = entity_type_label(entity_type);
    match skip {
        Skip::IgnoredType => {}
        Skip::OldProfile => SKIPPED_OLD.handle([entity_type]).increment(1),
        Skip::AlreadyDeployed => ALREADY_STORED.handle([entity_type]).increment(1),
    }
}

/// Counts up while it lives: an entity the batch deployer holds a queue slot for
/// (`dcl_pending_deployment_gauge`, `download_queue_pending`), or one whose entity and content
/// files are downloading (`dcl_pending_download_gauge`).
pub struct InFlight {
    deployment: bool,
    entity_type: &'static str,
}

impl InFlight {
    pub fn deployment(entity_type: &str) -> Self {
        Self::start(true, entity_type)
    }

    pub fn download(entity_type: &str) -> Self {
        Self::start(false, entity_type)
    }

    fn start(deployment: bool, entity_type: &str) -> Self {
        let held = Self {
            deployment,
            entity_type: crate::series::entity_type(entity_type),
        };
        held.gauge().increment(1.0);
        if deployment {
            QUEUE_PENDING.handle([]).increment(1.0);
        }
        held
    }

    fn gauge(&self) -> dcl_metrics::Gauge {
        let family = if self.deployment {
            PENDING_DEPLOYMENTS
        } else {
            PENDING_DOWNLOADS
        };
        family.handle([self.entity_type])
    }
}

impl Drop for InFlight {
    fn drop(&mut self) {
        self.gauge().decrement(1.0);
        if self.deployment {
            QUEUE_PENDING.handle([]).decrement(1.0);
        }
    }
}

/// The download of an entity file and its content files ended: `Ok` with everything stored,
/// or the failure. Every failure is retried (the failed-deployments record, or the read-through's
/// next miss), so `retryable` is always `true`.
pub fn entity_downloaded(entity_type: &str, ok: bool) {
    let entity_type = entity_type_label(entity_type);
    if ok {
        DOWNLOAD_SUCCESS.handle([entity_type]).increment(1);
    } else {
        DOWNLOAD_FAILURE.handle([entity_type, "true"]).increment(1);
    }
}

/// A downloaded entity's validation and hand-off to the batch writer ended after `elapsed`.
pub fn entity_deployed(entity_type: &str, elapsed: Duration, ok: bool) {
    let failed = if ok { "false" } else { "true" };
    DEPLOYMENT_TIME
        .handle([crate::series::entity_type(entity_type), failed])
        .record(elapsed.as_secs_f64());
    let entity_type = entity_type_label(entity_type);
    if ok {
        DEPLOYMENT_SUCCESS.handle([entity_type]).increment(1);
    } else {
        DEPLOYMENT_FAILURE
            .handle([entity_type, "true"])
            .increment(1);
    }
}

/// One `/pointer-changes` page request to `server`, answered or not.
pub fn pointer_changes_page(server: &str, elapsed: Duration) {
    POINTER_CHANGES_RESPONSE_TIME
        .handle_owned([remote_server(server).into()])
        .record(elapsed.as_secs_f64());
}

/// A steady-state `/pointer-changes` stream to `server` (re)starts.
pub fn stream_connected(server: &str) {
    STREAM_RECONNECTIONS
        .handle_owned([remote_server(server).into()])
        .increment(1);
}

/// A steady-state `/pointer-changes` stream to `server` ended in an error.
pub fn stream_failed(server: &str) {
    STREAM_FAILURES
        .handle_owned([remote_server(server).into()])
        .increment(1);
}

/// The peers in each sync phase.
pub fn server_phases(from_snapshots: usize, from_pointer_changes: usize, syncing: usize) {
    BOOTSTRAPPING_SERVERS
        .handle([SNAPSHOTS])
        .set(from_snapshots.approx_f64());
    BOOTSTRAPPING_SERVERS
        .handle([POINTER_CHANGES])
        .set(from_pointer_changes.approx_f64());
    SYNCING_SERVERS.handle([]).set(syncing.approx_f64());
}

/// `stream_end`: a snapshot file was read to its end; `incomplete`: it had unreadable lines;
/// `saved`: it was marked processed.
pub fn snapshots_processed(state: &'static str, snapshots: u64) {
    PROCESSED_SNAPSHOTS.handle([state]).increment(snapshots);
}

/// Legacy's 0 (bootstrapping) and 1 (syncing). A partially synced node is still bootstrapping
/// its profiles, and a failed one is not syncing: both are 0.
pub fn sync_state(state: &SyncState) {
    let value = if *state == SyncState::Syncing {
        1.0
    } else {
        0.0
    };
    SYNC_STATE.handle([]).set(value);
}

/// The rows of `content.failed_deployments`.
pub fn failed_deployments(rows: u64) {
    FAILED_DEPLOYMENTS.handle([]).set(rows.approx_f64());
}

/// Zeroes `dcl_content_server_failed_deployments` when dropped, unless a newer retry worker of
/// this process started since: the worker stopped, and the process that took over reports the
/// rows. A worker that lost the lease mid-cycle can outlive the one this process started when it
/// took the lease back, and its late drop must not zero the newer worker's count.
#[must_use = "the gauge is zeroed when this drops"]
pub struct FailedDeploymentsExport {
    generation: u64,
}

impl FailedDeploymentsExport {
    /// Starts an export newer than every live one, before the first count is set.
    pub fn start() -> Self {
        export_generation(|current| {
            *current += 1;
            Self {
                generation: *current,
            }
        })
    }
}

impl Drop for FailedDeploymentsExport {
    fn drop(&mut self) {
        export_generation(|current| {
            if *current == self.generation {
                failed_deployments(0);
            }
        });
    }
}

/// The generation of the latest [`FailedDeploymentsExport`], held while the guard reads or bumps
/// it so a drop cannot zero the gauge after a newer export started. Process-wide, like the
/// gauge; per thread under test, like [`local_recorder`], so parallel tests don't see each
/// other's exports.
fn export_generation<R>(f: impl FnOnce(&mut u64) -> R) -> R {
    #[cfg(not(test))]
    {
        static GENERATION: std::sync::Mutex<u64> = std::sync::Mutex::new(0);
        f(&mut GENERATION
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner))
    }
    #[cfg(test)]
    {
        thread_local! {
            static GENERATION: std::cell::RefCell<u64> = const { std::cell::RefCell::new(0) };
        }
        GENERATION.with_borrow_mut(f)
    }
}

/// A recorder with the serve registry's content and sync declarations, for this thread until
/// the guard drops, and its handle to render.
#[cfg(test)]
pub(crate) fn local_recorder() -> (
    dcl_metrics::LocalRecorderGuard<'static>,
    dcl_metrics::PrometheusHandle,
) {
    let recorder =
        dcl_metrics::Registry::new(&[FAMILIES, crate::series::FAMILIES, crate::metrics::FAMILIES])
            .unwrap()
            .builder(dcl_metrics::PrometheusBuilder::new())
            .unwrap()
            .build_recorder();
    let handle = recorder.handle();
    let recorder: &'static _ = Box::leak(Box::new(recorder));
    (dcl_metrics::set_default_local_recorder(recorder), handle)
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
