//! The content server's Prometheus families that keep a legacy content server series: the
//! names, label names, label values and histogram buckets the content server's dashboards and
//! alerts read (`docs/metrics/content.md` maps each one to where it is observed; the sync
//! series are in `docs/metrics/sync.md`). The families under dcl-one's own names, those that
//! replace a legacy series included, are in [`crate::metrics`]; keeping the two apart keeps
//! this list the legacy contract. `catalyst serve` lists both `FAMILIES` in its registry, which
//! applies the declared buckets.
//!
//! Every label value comes from a closed set: an entity type a client or a peer names goes
//! through [`entity_type`] first. Recording only touches the process recorder, a no-op
//! without one, so it never fails or blocks a request or a job.

use crate::shared::casts::ApproxF64;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use dcl_metrics::{CounterFamily, Descriptor, GaugeFamily, HistogramFamily, SharedString};

/// The buckets of a legacy histogram declared without its own.
pub(crate) const DEFAULT_BUCKETS: &[f64] = dcl_metrics::PROM_CLIENT_DEFAULT_BUCKETS;

/// A counter family with no unit.
pub(crate) const fn counter<const N: usize>(
    name: &'static str,
    help: &'static str,
    keys: &'static [&'static str; N],
) -> CounterFamily<N> {
    CounterFamily::new(name, help, None, keys)
}

/// A gauge family with no unit.
pub(crate) const fn gauge<const N: usize>(
    name: &'static str,
    help: &'static str,
    keys: &'static [&'static str; N],
) -> GaugeFamily<N> {
    GaugeFamily::new(name, help, None, keys)
}

pub(crate) const BUILD_INFO: GaugeFamily<3> = gauge(
    "dcl_content_server_build_info",
    "Content server static build info.",
    &["version", "commitHash", "ethNetwork"],
);
pub(crate) const SNAPSHOT_GENERATION_TIME: HistogramFamily<3> = HistogramFamily::new(
    "dcl_content_server_snapshot_generation_time",
    "Histogram of time spent generating full snapshots",
    None,
    &["result", "interval_size", "reason"],
    DEFAULT_BUCKETS,
);
pub(crate) const DEPLOYMENTS_ENDPOINT: CounterFamily<1> = counter(
    "dcl_deployments_endpoint_counter",
    "Total deployments through HTTP",
    &["kind"],
);
pub(crate) const TOTAL_DEPLOYMENTS: CounterFamily<2> = counter(
    "total_deployments_count",
    "Total number of deployments made to the content server",
    &["entity_type", "deployment_context"],
);
pub(crate) const RATE_LIMITED: CounterFamily<2> = counter(
    "dcl_content_rate_limited_deployments_total",
    "Total failed deployments due rate limit",
    &["entity_type", "reason"],
);
pub(crate) const RATE_LIMITER_KEYS: GaugeFamily<2> = gauge(
    "dcl_content_rate_limiter_cache_keys",
    "Current number of keys in rate limiter caches",
    &["entity_type", "cache_type"],
);
pub(crate) const RATE_LIMITER_MAX_SIZE: GaugeFamily<1> = gauge(
    "dcl_content_rate_limiter_cache_max_size",
    "Configured max size for deployment rate limiter cache",
    &["entity_type"],
);
pub(crate) const BLOCK_FETCH_RETRIES: CounterFamily<1> = counter(
    "dcl_block_fetch_retries_total",
    "Total retried block RPC reads during on-chain validation (per network)",
    &["network"],
);
pub(crate) const BLOOM_FILTER_CHECKS: CounterFamily<1> = counter(
    "dcl_deployed_entities_bloom_filter_checks_total",
    "Total number of deployments existence checks to the deployment list filter",
    &["hit"],
);
pub(crate) const GC_RUNS: CounterFamily<1> = counter(
    "dcl_content_garbage_collection_runs_total",
    "Garbage collection sweeps",
    &["outcome"],
);
pub(crate) const PARTIAL_STAGING: CounterFamily<1> = counter(
    "dcl_partial_deployments_staging_total",
    "Partial (multi-request) deployment staging requests through HTTP",
    &["kind"],
);
pub(crate) const PARTIAL_THROTTLED: CounterFamily<1> = counter(
    "dcl_partial_upload_throttled_total",
    "Partial upload batches answered 429, by the quota or limit that rejected them",
    &["reason"],
);
pub(crate) const PARTIAL_STARTED: CounterFamily<0> = counter(
    "dcl_partial_uploads_started_total",
    "Partial uploads whose first batch was admitted",
    &[],
);
pub(crate) const PARTIAL_COMPLETED: CounterFamily<0> = counter(
    "dcl_partial_uploads_completed_total",
    "Partial uploads published by their final batch",
    &[],
);
pub(crate) const PARTIAL_PENDING: GaugeFamily<1> = gauge(
    "dcl_partial_uploads_pending",
    "Partial uploads in the database at the last cleanup run (server-wide)",
    &["state"],
);
pub(crate) const PARTIAL_DURATION: HistogramFamily<0> = HistogramFamily::new(
    "dcl_partial_upload_duration_seconds",
    "Time from a partial upload first batch arrival to its publication",
    None,
    &[],
    &[
        5.0, 15.0, 30.0, 60.0, 120.0, 300.0, 600.0, 900.0, 1800.0, 2700.0, 3600.0,
    ],
);
pub(crate) const PARTIAL_BATCHES_PER_UPLOAD: HistogramFamily<0> = HistogramFamily::new(
    "dcl_partial_upload_batches_per_upload",
    "Stored batches a partial upload took until publication",
    None,
    &[],
    &[1.0, 2.0, 3.0, 5.0, 10.0, 20.0, 50.0, 100.0, 250.0],
);
pub(crate) const PARTIAL_STAGE_DURATION: HistogramFamily<1> = HistogramFamily::new(
    "dcl_partial_upload_stage_duration_seconds",
    "Time spent in each stage of a partial upload batch, including the finalize of the completing batch",
    None,
    &["stage"],
    &[
        0.005, 0.025, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0, 30.0, 60.0, 120.0,
    ],
);
pub(crate) const PARTIAL_CAPACITY: GaugeFamily<0> = gauge(
    "dcl_partial_upload_capacity_bytes",
    "Server-wide staging byte cap for partial uploads (MAX_PENDING_BYTES)",
    &[],
);
pub(crate) const PARTIAL_EXPIRED: CounterFamily<0> = counter(
    "dcl_pending_deployments_expired_total",
    "Expired pending deployments reclaimed by the cleanup job",
    &[],
);
pub(crate) const PARTIAL_CLEANUP_RUNS: CounterFamily<1> = counter(
    "dcl_partial_upload_cleanup_runs_total",
    "Expired partial upload cleanup runs",
    &["outcome"],
);
pub(crate) const PARTIAL_CLEANUP_FAILURES: CounterFamily<1> = counter(
    "dcl_partial_upload_cleanup_failures_total",
    "Expired partial uploads a cleanup run failed to reclaim, skipped until the next run",
    &["reason"],
);
pub(crate) const PARTIAL_CLEANUP_LAST_SUCCESS: GaugeFamily<0> = gauge(
    "dcl_partial_upload_cleanup_last_success_timestamp_seconds",
    "Unix time of the last cleanup run that reclaimed every expired upload it listed",
    &[],
);
pub(crate) const PARTIAL_METADATA_CHECKS: CounterFamily<0> = counter(
    "dcl_partial_upload_metadata_checks_total",
    "Content metadata checks performed by partial uploads",
    &[],
);
pub(crate) const PARTIAL_BATCHES: CounterFamily<1> = counter(
    "dcl_partial_upload_batches_total",
    "Accepted partial upload batches",
    &["outcome"],
);
pub(crate) const PARTIAL_RESERVED: GaugeFamily<0> = gauge(
    "dcl_partial_upload_reserved_bytes",
    "Staging bytes reserved, including expired uploads awaiting cleanup",
    &[],
);
pub(crate) const PARTIAL_CLEANUP_BACKLOG: GaugeFamily<0> = gauge(
    "dcl_partial_upload_cleanup_backlog_bytes",
    "Expired staging bytes awaiting successful cleanup",
    &[],
);
pub(crate) const UPLOAD_BUDGET_CAPACITY: GaugeFamily<1> = gauge(
    "dcl_upload_budget_capacity_bytes",
    "Capacity of a POST /entities upload budget",
    &["budget"],
);
pub(crate) const UPLOAD_BUDGET_RESERVED: GaugeFamily<1> = gauge(
    "dcl_upload_budget_reserved_bytes",
    "POST /entities bytes reserved in an upload budget",
    &["budget"],
);
pub(crate) const UPLOAD_BUDGET_ACTIVE: GaugeFamily<1> = gauge(
    "dcl_upload_budget_active",
    "POST /entities uploads holding an upload budget",
    &["budget"],
);
pub(crate) const UPLOAD_BUDGET_REJECTIONS: CounterFamily<2> = counter(
    "dcl_upload_budget_rejections_total",
    "POST /entities uploads shed because an upload budget, or its source share, was full",
    &["budget", "reason"],
);
pub(crate) const UPLOAD_SPOOL_RECLAIMED: CounterFamily<0> = counter(
    "dcl_upload_spool_reclaimed_folders_total",
    "Spool folders of exited processes reclaimed at startup",
    &[],
);
pub(crate) const MULTIPART_TIMEOUTS: CounterFamily<0> = counter(
    "dcl_multipart_upload_timeouts_total",
    "POST /entities bodies that did not arrive within MULTIPART_UPLOAD_TIMEOUT_MS (408)",
    &[],
);
pub(crate) const MULTIPART_ABORTS: CounterFamily<1> = counter(
    "dcl_multipart_upload_aborts_total",
    "POST /entities bodies aborted while received because the client closed the connection (499)",
    &["reason"],
);

pub const FAMILIES: &[Descriptor] = &[
    BUILD_INFO.descriptor,
    SNAPSHOT_GENERATION_TIME.descriptor,
    DEPLOYMENTS_ENDPOINT.descriptor,
    TOTAL_DEPLOYMENTS.descriptor,
    RATE_LIMITED.descriptor,
    RATE_LIMITER_KEYS.descriptor,
    RATE_LIMITER_MAX_SIZE.descriptor,
    BLOCK_FETCH_RETRIES.descriptor,
    BLOOM_FILTER_CHECKS.descriptor,
    GC_RUNS.descriptor,
    PARTIAL_STAGING.descriptor,
    PARTIAL_THROTTLED.descriptor,
    PARTIAL_STARTED.descriptor,
    PARTIAL_COMPLETED.descriptor,
    PARTIAL_PENDING.descriptor,
    PARTIAL_DURATION.descriptor,
    PARTIAL_BATCHES_PER_UPLOAD.descriptor,
    PARTIAL_STAGE_DURATION.descriptor,
    PARTIAL_CAPACITY.descriptor,
    PARTIAL_EXPIRED.descriptor,
    PARTIAL_CLEANUP_RUNS.descriptor,
    PARTIAL_CLEANUP_FAILURES.descriptor,
    PARTIAL_CLEANUP_LAST_SUCCESS.descriptor,
    PARTIAL_METADATA_CHECKS.descriptor,
    PARTIAL_BATCHES.descriptor,
    PARTIAL_RESERVED.descriptor,
    PARTIAL_CLEANUP_BACKLOG.descriptor,
    UPLOAD_BUDGET_CAPACITY.descriptor,
    UPLOAD_BUDGET_RESERVED.descriptor,
    UPLOAD_BUDGET_ACTIVE.descriptor,
    UPLOAD_BUDGET_REJECTIONS.descriptor,
    UPLOAD_SPOOL_RECLAIMED.descriptor,
    MULTIPART_TIMEOUTS.descriptor,
    MULTIPART_ABORTS.descriptor,
];

/// The entity types a label may name.
pub(crate) const ENTITY_TYPES: [&str; 6] =
    ["scene", "profile", "wearable", "store", "emote", "outfits"];

/// `entity_type` as a label value: one of [`ENTITY_TYPES`], else `unknown`.
pub fn entity_type(entity_type: &str) -> &'static str {
    ENTITY_TYPES
        .iter()
        .find(|t| **t == entity_type)
        .copied()
        .unwrap_or("unknown")
}

fn seconds_since(started: Instant) -> f64 {
    started.elapsed().as_secs_f64()
}

fn unix_seconds_now() -> f64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0.0, |d| d.as_secs_f64())
}

/// The process's version, commit and network, at 1.
pub fn build_info(version: &str, commit_hash: Option<&str>, eth_network: &str) {
    BUILD_INFO
        .handle_owned([
            SharedString::from(version.to_string()),
            SharedString::from(commit_hash.unwrap_or("Unknown").to_string()),
            SharedString::from(eth_network.to_string()),
        ])
        .set(1.0);
}

/// One snapshot `window` written (`result` success) or not (error), `reason` why it was
/// generated.
pub fn snapshot_generated(
    started: Instant,
    ok: bool,
    window: &crate::wire_types::TimeRange,
    reason: &'static str,
) {
    let interval_size = crate::time_range::interval_size_label(window);
    SNAPSHOT_GENERATION_TIME
        .handle([if ok { "success" } else { "error" }, interval_size, reason])
        .record(seconds_since(started));
}

/// A `POST /entities` regular deploy answered: `success`, `validation_error` or `error`.
pub fn deployment_answered(kind: &'static str) {
    DEPLOYMENTS_ENDPOINT.handle([kind]).increment(1);
}

/// An entity newly stored, by the context it was deployed in.
pub fn deployment_stored(entity_type: &str, context: &'static str) {
    TOTAL_DEPLOYMENTS
        .handle([self::entity_type(entity_type), context])
        .increment(1);
}

pub fn rate_limited(entity_type: &str, reason: &'static str) {
    RATE_LIMITED
        .handle([self::entity_type(entity_type), reason])
        .increment(1);
}

/// The keys a rate-limit window holds; `cache_type` is `deployment` or `unchanged`.
pub fn rate_limiter_keys(entity_type: &str, cache_type: &'static str, keys: usize) {
    RATE_LIMITER_KEYS
        .handle([self::entity_type(entity_type), cache_type])
        .set(keys.approx_f64());
}

pub fn rate_limiter_max_size(entity_type: &str, max_size: usize) {
    RATE_LIMITER_MAX_SIZE
        .handle([self::entity_type(entity_type)])
        .set(max_size.approx_f64());
}

/// A block read asked again; `network` is `mainnet`, `sepolia`, `polygon` or `amoy`.
pub fn block_fetch_retried(network: &'static str) {
    BLOCK_FETCH_RETRIES.handle([network]).increment(1);
}

/// `checks` "already deployed" probes that went through the bloom filter: `hit` when the entity
/// is deployed, a miss for a filter miss and for a false positive the database refuted.
pub fn bloom_filter_checked(deployed: bool, checks: usize) {
    BLOOM_FILTER_CHECKS
        .handle([if deployed { "true" } else { "false" }])
        .increment(checks as u64);
}

/// A history-prune pass ended: `success`, `deferred` (the store lock stayed busy) or `error`.
pub fn gc_run(outcome: &'static str) {
    GC_RUNS.handle([outcome]).increment(1);
}

/// A partial upload batch answered, by its staging kind, and for a 429 of a quota or limit,
/// which one.
pub fn partial_answered(kind: &'static str, throttled: Option<&'static str>) {
    PARTIAL_STAGING.handle([kind]).increment(1);
    if let Some(reason) = throttled {
        PARTIAL_THROTTLED.handle([reason]).increment(1);
    }
}

pub fn partial_started() {
    PARTIAL_STARTED.handle([]).increment(1);
}

/// A partial upload published by its final batch, `batches` stored batches after its first
/// batch arrived at `created_at_ms`.
pub fn partial_completed(created_at_ms: i64, batches: i64) {
    PARTIAL_COMPLETED.handle([]).increment(1);
    let elapsed_ms = chrono::Utc::now().timestamp_millis() - created_at_ms;
    PARTIAL_DURATION
        .handle([])
        .record(elapsed_ms.max(0).approx_f64() / 1000.0);
    PARTIAL_BATCHES_PER_UPLOAD
        .handle([])
        .record(batches.max(0).approx_f64());
}

/// One timed stage of a partial upload batch.
pub fn partial_stage(stage: &'static str, started: Instant) {
    PARTIAL_STAGE_DURATION
        .handle([stage])
        .record(seconds_since(started));
}

/// Times a partial upload stage until dropped, so a stage that fails or is cancelled is timed
/// too.
pub struct StageTimer {
    stage: &'static str,
    started: Instant,
}

impl StageTimer {
    pub fn start(stage: &'static str) -> Self {
        Self {
            stage,
            started: Instant::now(),
        }
    }
}

impl Drop for StageTimer {
    fn drop(&mut self) {
        partial_stage(self.stage, self.started);
    }
}

pub fn partial_metadata_checks(hashes: usize) {
    PARTIAL_METADATA_CHECKS.handle([]).increment(hashes as u64);
}

/// An accepted batch stored: `incomplete` while content is missing, `finalizing` once it
/// completes the upload.
pub fn partial_batch(complete: bool) {
    let outcome = if complete { "finalizing" } else { "incomplete" };
    PARTIAL_BATCHES.handle([outcome]).increment(1);
}

pub fn partial_capacity(bytes: i64) {
    PARTIAL_CAPACITY.handle([]).set(bytes.approx_f64());
}

pub fn partial_reserved(bytes: i64) {
    PARTIAL_RESERVED.handle([]).set(bytes.approx_f64());
}

/// What `content.pending_deployments` holds at the end of a cleanup run.
pub fn partial_totals(reserved: i64, expired_bytes: i64, live: i64, expired: i64) {
    partial_reserved(reserved);
    PARTIAL_CLEANUP_BACKLOG
        .handle([])
        .set(expired_bytes.approx_f64());
    PARTIAL_PENDING.handle(["live"]).set(live.approx_f64());
    PARTIAL_PENDING
        .handle(["expired"])
        .set(expired.approx_f64());
}

/// A cleanup run ended: `success`, `deferred` or `error`, `removed` uploads reclaimed.
pub fn partial_cleanup(outcome: &'static str, removed: usize) {
    PARTIAL_CLEANUP_RUNS.handle([outcome]).increment(1);
    PARTIAL_EXPIRED.handle([]).increment(removed as u64);
    if outcome == "success" {
        PARTIAL_CLEANUP_LAST_SUCCESS
            .handle([])
            .set(unix_seconds_now());
    }
}

/// Expired uploads a run could not reclaim: `storage` (an unlink) or `database`.
pub fn partial_cleanup_failures(reason: &'static str, uploads: usize) {
    PARTIAL_CLEANUP_FAILURES
        .handle([reason])
        .increment(uploads as u64);
}

/// An upload budget's series, bound once so a reservation that grows with every received chunk
/// only stores into them.
pub struct UploadBudgetSeries {
    reserved: dcl_metrics::Gauge,
    active: dcl_metrics::Gauge,
    rejected: dcl_metrics::Counter,
}

impl UploadBudgetSeries {
    /// The series of `budget` (`memory` or `disk`), its capacity set and nothing reserved.
    pub fn new(budget: &'static str, capacity: u64) -> Self {
        UPLOAD_BUDGET_CAPACITY
            .handle([budget])
            .set(capacity.approx_f64());
        let series = Self {
            reserved: UPLOAD_BUDGET_RESERVED.handle([budget]),
            active: UPLOAD_BUDGET_ACTIVE.handle([budget]),
            rejected: UPLOAD_BUDGET_REJECTIONS.handle([budget, "bytes"]),
        };
        series.usage(0, 0);
        series
    }

    /// The bytes reserved and the uploads holding some of them.
    pub fn usage(&self, reserved: u64, active: u64) {
        self.reserved.set(reserved.approx_f64());
        self.active.set(active.approx_f64());
    }

    pub fn rejected(&self) {
        self.rejected.increment(1);
    }
}

/// An upload shed: `budget` `source` with `source_bytes`/`source_concurrency` (a byte budget
/// counts through [`UploadBudgetSeries`]).
pub fn upload_budget_rejected(budget: &'static str, reason: &'static str) {
    UPLOAD_BUDGET_REJECTIONS
        .handle([budget, reason])
        .increment(1);
}

pub fn upload_spool_reclaimed(folders: usize) {
    UPLOAD_SPOOL_RECLAIMED.handle([]).increment(folders as u64);
}

pub fn multipart_timed_out() {
    MULTIPART_TIMEOUTS.handle([]).increment(1);
}

/// A `POST /entities` client closed the connection while its body was received, once per
/// request (`counted`).
pub fn body_aborted(counted: &AtomicBool) {
    if !counted.swap(true, Ordering::Relaxed) {
        MULTIPART_ABORTS.handle(["client_closed"]).increment(1);
    }
}

/// Armed while a `POST /entities` body is read: dropped armed, the request was dropped with
/// it (the client went away), which counts one abort unless the body's stream already
/// reported the close ([`body_aborted`]).
pub struct ReceivingBody {
    counted: Arc<AtomicBool>,
    armed: bool,
}

impl ReceivingBody {
    pub fn start(counted: Arc<AtomicBool>) -> Self {
        Self {
            counted,
            armed: true,
        }
    }

    /// The read ended (read, failed or timed out): nothing is counted on drop.
    pub fn settled(mut self) {
        self.armed = false;
    }
}

impl Drop for ReceivingBody {
    fn drop(&mut self) {
        if self.armed {
            body_aborted(&self.counted);
        }
    }
}

#[cfg(test)]
pub(crate) mod test_support {
    /// A recorder with the serve registry's content and storage declarations.
    pub fn recorder() -> dcl_metrics::PrometheusRecorder {
        dcl_metrics::Registry::new(&[
            super::FAMILIES,
            crate::metrics::FAMILIES,
            crate::storage::FAMILIES,
        ])
        .unwrap()
        .builder(dcl_metrics::PrometheusBuilder::new())
        .unwrap()
        .build_recorder()
    }

    /// The rendered sample of `name` with exactly `labels`, if any.
    pub fn sample(rendered: &str, name: &str, labels: &[(&str, &str)]) -> Option<f64> {
        rendered.lines().find_map(|line| {
            let (series, value) = line.rsplit_once(' ')?;
            let (found, found_labels) = match series.split_once('{') {
                Some((found, rest)) => (found, rest.strip_suffix('}')?),
                None => (series, ""),
            };
            if found != name {
                return None;
            }
            let mut parsed: Vec<(&str, &str)> = found_labels
                .split(',')
                .filter(|l| !l.is_empty())
                .filter_map(|l| {
                    let (k, v) = l.split_once('=')?;
                    Some((k, v.trim_matches('"')))
                })
                .collect();
            let mut wanted = labels.to_vec();
            parsed.sort_unstable();
            wanted.sort_unstable();
            (parsed == wanted).then(|| value.parse().ok()).flatten()
        })
    }
}

#[cfg(test)]
mod tests {
    use super::test_support::{recorder, sample};
    use super::*;
    use crate::time_range::{MS_PER_DAY, MS_PER_WEEK};

    fn window(size_ms: i64) -> crate::wire_types::TimeRange {
        crate::wire_types::TimeRange {
            init_timestamp: 0,
            end_timestamp: size_ms,
        }
    }

    fn render(record: impl FnOnce()) -> String {
        let recorder = recorder();
        let handle = recorder.handle();
        dcl_metrics::with_local_recorder(&recorder, record);
        handle.render()
    }

    #[test]
    fn every_histogram_renders_its_declared_buckets() {
        let histograms = [
            (SNAPSHOT_GENERATION_TIME.descriptor.name, DEFAULT_BUCKETS),
            (
                PARTIAL_DURATION.descriptor.name,
                &[
                    5.0, 15.0, 30.0, 60.0, 120.0, 300.0, 600.0, 900.0, 1800.0, 2700.0, 3600.0,
                ][..],
            ),
            (
                PARTIAL_BATCHES_PER_UPLOAD.descriptor.name,
                &[1.0, 2.0, 3.0, 5.0, 10.0, 20.0, 50.0, 100.0, 250.0][..],
            ),
            (
                PARTIAL_STAGE_DURATION.descriptor.name,
                &[
                    0.005, 0.025, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0, 30.0, 60.0, 120.0,
                ][..],
            ),
        ];
        let rendered = render(|| {
            snapshot_generated(
                Instant::now(),
                true,
                &window(MS_PER_DAY),
                "cover_time_range",
            );
            partial_completed(chrono::Utc::now().timestamp_millis(), 2);
            partial_stage("hash", Instant::now());
        });
        for (name, buckets) in histograms {
            assert!(
                rendered.contains(&format!("# TYPE {name} histogram")),
                "{name}: {rendered}"
            );
            let le: Vec<&str> = rendered
                .lines()
                .filter(|line| line.starts_with(&format!("{name}_bucket{{")))
                .filter_map(|line| line.split("le=\"").nth(1)?.split('"').next())
                .collect();
            let mut expected: Vec<String> = buckets.iter().map(f64::to_string).collect();
            expected.push("+Inf".to_string());
            assert_eq!(le, expected, "{name}");
        }
    }

    #[test]
    fn label_values_stay_in_their_closed_sets() {
        let rendered = render(|| {
            deployment_stored("a-peer-chosen-type", "SYNCED");
            snapshot_generated(
                Instant::now(),
                true,
                &window(2 * MS_PER_WEEK),
                "cover_time_range",
            );
            build_info("1.2.3", None, "sepolia");
        });
        assert_eq!(
            sample(
                &rendered,
                TOTAL_DEPLOYMENTS.descriptor.name,
                &[("entity_type", "unknown"), ("deployment_context", "SYNCED")]
            ),
            Some(1.0)
        );
        assert!(rendered.contains("interval_size=\"unknown\""), "{rendered}");
        assert_eq!(
            sample(
                &rendered,
                BUILD_INFO.descriptor.name,
                &[
                    ("version", "1.2.3"),
                    ("commitHash", "Unknown"),
                    ("ethNetwork", "sepolia")
                ]
            ),
            Some(1.0)
        );
        assert!(rendered.contains("# TYPE dcl_content_server_build_info gauge"));
    }

    #[test]
    fn a_stage_left_early_is_still_timed() {
        let failed = || -> Result<(), ()> {
            let _timer = StageTimer::start("progress");
            Err(())
        };
        let rendered = render(|| {
            let _ = failed();
        });
        assert_eq!(
            sample(
                &rendered,
                &format!("{}_count", PARTIAL_STAGE_DURATION.descriptor.name),
                &[("stage", "progress")]
            ),
            Some(1.0)
        );
    }

    #[test]
    fn a_body_dropped_while_received_counts_one_abort() {
        let rendered = render(|| {
            drop(ReceivingBody::start(Arc::default()));
            ReceivingBody::start(Arc::default()).settled();
            let counted = Arc::new(AtomicBool::new(false));
            body_aborted(&counted);
            drop(ReceivingBody::start(counted));
        });
        assert_eq!(
            sample(
                &rendered,
                MULTIPART_ABORTS.descriptor.name,
                &[("reason", "client_closed")]
            ),
            Some(2.0),
            "a close the stream reported is not counted again on drop"
        );
    }
}
