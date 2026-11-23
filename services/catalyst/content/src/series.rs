//! The content server's Prometheus series, under the names, label names, label values and
//! histogram buckets the content server's dashboards and alerts read
//! (`docs/metrics/content.md` maps each one to where it is observed; the sync series are
//! in `docs/metrics/sync.md`).
//!
//! Every label value comes from a closed set: an entity type a client or a peer names goes
//! through [`entity_type`] first. The helpers only touch the `metrics` facade, a no-op
//! without an installed recorder, so they never fail or block a request or a job.
//! [`register_series`] renders the counters of each closed label set at 0, and the
//! last-success gauges, before the first event.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use metrics::{counter, gauge, histogram};

pub const BUILD_INFO: &str = "dcl_content_server_build_info";
pub const SNAPSHOT_GENERATION_TIME: &str = "dcl_content_server_snapshot_generation_time";

pub const DEPLOYMENTS_ENDPOINT: &str = "dcl_deployments_endpoint_counter";
pub const TOTAL_DEPLOYMENTS: &str = "total_deployments_count";

pub const RATE_LIMITED: &str = "dcl_content_rate_limited_deployments_total";
pub const RATE_LIMITER_KEYS: &str = "dcl_content_rate_limiter_cache_keys";
pub const RATE_LIMITER_MAX_SIZE: &str = "dcl_content_rate_limiter_cache_max_size";

pub const BLOCK_FETCH_RETRIES: &str = "dcl_block_fetch_retries_total";
pub const BLOOM_FILTER_CHECKS: &str = "dcl_deployed_entities_bloom_filter_checks_total";

pub const GC_ITEMS: &str = "dcl_content_garbage_collection_items_total";
pub const GC_TIME: &str = "dcl_content_garbage_collection_time";
pub const GC_RUNS: &str = "dcl_content_garbage_collection_runs_total";
pub const GC_LAST_SUCCESS: &str = "dcl_content_garbage_collection_last_success_timestamp_seconds";
pub use crate::storage::gc::LOCK_WRITER_TIMEOUTS;

pub const PARTIAL_STAGING: &str = "dcl_partial_deployments_staging_total";
pub const PARTIAL_THROTTLED: &str = "dcl_partial_upload_throttled_total";
pub const PARTIAL_STARTED: &str = "dcl_partial_uploads_started_total";
pub const PARTIAL_COMPLETED: &str = "dcl_partial_uploads_completed_total";
pub const PARTIAL_PENDING: &str = "dcl_partial_uploads_pending";
pub const PARTIAL_DURATION: &str = "dcl_partial_upload_duration_seconds";
pub const PARTIAL_BATCHES_PER_UPLOAD: &str = "dcl_partial_upload_batches_per_upload";
pub const PARTIAL_STAGE_DURATION: &str = "dcl_partial_upload_stage_duration_seconds";
pub const PARTIAL_CAPACITY: &str = "dcl_partial_upload_capacity_bytes";
pub const PARTIAL_EXPIRED: &str = "dcl_pending_deployments_expired_total";
pub const PARTIAL_CLEANUP_RUNS: &str = "dcl_partial_upload_cleanup_runs_total";
pub const PARTIAL_CLEANUP_FAILURES: &str = "dcl_partial_upload_cleanup_failures_total";
pub const PARTIAL_CLEANUP_DURATION: &str = "dcl_partial_upload_cleanup_duration_seconds";
pub const PARTIAL_CLEANUP_LAST_SUCCESS: &str =
    "dcl_partial_upload_cleanup_last_success_timestamp_seconds";
pub const PARTIAL_METADATA_CHECKS: &str = "dcl_partial_upload_metadata_checks_total";
pub const PARTIAL_BATCHES: &str = "dcl_partial_upload_batches_total";
pub const PARTIAL_RESERVED: &str = "dcl_partial_upload_reserved_bytes";
pub const PARTIAL_CLEANUP_BACKLOG: &str = "dcl_partial_upload_cleanup_backlog_bytes";

pub const UPLOAD_BUDGET_CAPACITY: &str = "dcl_upload_budget_capacity_bytes";
pub const UPLOAD_BUDGET_RESERVED: &str = "dcl_upload_budget_reserved_bytes";
pub const UPLOAD_BUDGET_ACTIVE: &str = "dcl_upload_budget_active";
pub const UPLOAD_BUDGET_REJECTIONS: &str = "dcl_upload_budget_rejections_total";
pub const UPLOAD_SPOOL_FAILURES: &str = "dcl_upload_spool_failures_total";
pub const UPLOAD_SPOOL_RECLAIMED: &str = "dcl_upload_spool_reclaimed_folders_total";
pub const MULTIPART_TIMEOUTS: &str = "dcl_multipart_upload_timeouts_total";
pub const MULTIPART_ABORTS: &str = "dcl_multipart_upload_aborts_total";

/// The Prometheus client's default buckets, for the histograms declared without their own.
const DEFAULT_BUCKETS: &[f64] = &[
    0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0,
];

/// Every histogram here and its buckets; the exporter renders a histogram without an entry
/// as a summary.
pub const BUCKETS: [(&str, &[f64]); 6] = [
    (SNAPSHOT_GENERATION_TIME, DEFAULT_BUCKETS),
    (GC_TIME, DEFAULT_BUCKETS),
    (
        PARTIAL_DURATION,
        &[
            5.0, 15.0, 30.0, 60.0, 120.0, 300.0, 600.0, 900.0, 1800.0, 2700.0, 3600.0,
        ],
    ),
    (
        PARTIAL_BATCHES_PER_UPLOAD,
        &[1.0, 2.0, 3.0, 5.0, 10.0, 20.0, 50.0, 100.0, 250.0],
    ),
    (
        PARTIAL_STAGE_DURATION,
        &[
            0.005, 0.025, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0, 30.0, 60.0, 120.0,
        ],
    ),
    (
        PARTIAL_CLEANUP_DURATION,
        &[0.1, 0.5, 1.0, 5.0, 15.0, 30.0, 60.0, 120.0, 300.0],
    ),
];

/// The entity types a label may name.
pub const ENTITY_TYPES: [&str; 6] = ["scene", "profile", "wearable", "store", "emote", "outfits"];

/// `entity_type` as a label value: one of [`ENTITY_TYPES`], else `unknown`.
pub fn entity_type(entity_type: &str) -> &'static str {
    ENTITY_TYPES
        .iter()
        .find(|t| **t == entity_type)
        .copied()
        .unwrap_or("unknown")
}

const ENDPOINT_KINDS: [&str; 3] = ["success", "validation_error", "error"];
const DEPLOYMENT_CONTEXTS: [&str; 3] = ["LOCAL", "SYNCED", "FIX_ATTEMPT"];
const RATE_LIMIT_REASONS: [&str; 3] = ["ttl", "max_size", "unchanged_ttl"];
const GC_OUTCOMES: [&str; 3] = ["success", "deferred", "error"];
const PARTIAL_KINDS: [&str; 7] = [
    "accepted",
    "finalized",
    "replayed",
    "validation_error",
    "throttled",
    "busy",
    "error",
];
const THROTTLE_REASONS: [&str; 8] = [
    "uploads_per_account",
    "uploads_per_pointer",
    "uploads_per_pointer_account",
    "bytes_per_account",
    "bytes_per_server",
    "bytes_per_minute",
    "entity_rate_limit",
    "pointer_conflict",
];
const SPOOL_FAILURE_REASONS: [&str; 5] = [
    "write_error",
    "read_error",
    "flush_timeout",
    "cleanup",
    "folder",
];

/// Renders every series with a closed label set at 0.
pub fn register_series() {
    for name in [
        GC_ITEMS,
        LOCK_WRITER_TIMEOUTS,
        PARTIAL_STARTED,
        PARTIAL_COMPLETED,
        PARTIAL_EXPIRED,
        PARTIAL_METADATA_CHECKS,
        UPLOAD_SPOOL_RECLAIMED,
        MULTIPART_TIMEOUTS,
    ] {
        counter!(name).increment(0);
    }
    for kind in ENDPOINT_KINDS {
        counter!(DEPLOYMENTS_ENDPOINT, "kind" => kind).increment(0);
    }
    for entity_type in ENTITY_TYPES {
        for context in DEPLOYMENT_CONTEXTS {
            counter!(TOTAL_DEPLOYMENTS, "entity_type" => entity_type, "deployment_context" => context)
                .increment(0);
        }
        for reason in RATE_LIMIT_REASONS {
            counter!(RATE_LIMITED, "entity_type" => entity_type, "reason" => reason).increment(0);
        }
    }
    for outcome in GC_OUTCOMES {
        counter!(GC_RUNS, "outcome" => outcome).increment(0);
        counter!(PARTIAL_CLEANUP_RUNS, "outcome" => outcome).increment(0);
    }
    for kind in PARTIAL_KINDS {
        counter!(PARTIAL_STAGING, "kind" => kind).increment(0);
    }
    for reason in THROTTLE_REASONS {
        counter!(PARTIAL_THROTTLED, "reason" => reason).increment(0);
    }
    for reason in ["storage", "database"] {
        counter!(PARTIAL_CLEANUP_FAILURES, "reason" => reason).increment(0);
    }
    for outcome in ["incomplete", "finalizing"] {
        counter!(PARTIAL_BATCHES, "outcome" => outcome).increment(0);
    }
    for reason in SPOOL_FAILURE_REASONS {
        counter!(UPLOAD_SPOOL_FAILURES, "reason" => reason).increment(0);
    }
    counter!(MULTIPART_ABORTS, "reason" => "client_closed").increment(0);
    for (budget, reason) in [
        ("memory", "bytes"),
        ("disk", "bytes"),
        ("source", "source_bytes"),
        ("source", "source_concurrency"),
    ] {
        counter!(UPLOAD_BUDGET_REJECTIONS, "budget" => budget, "reason" => reason).increment(0);
    }
    for hit in ["true", "false"] {
        counter!(BLOOM_FILTER_CHECKS, "hit" => hit).increment(0);
    }
    for name in [
        GC_TIME,
        PARTIAL_DURATION,
        PARTIAL_BATCHES_PER_UPLOAD,
        PARTIAL_CLEANUP_DURATION,
    ] {
        let _ = histogram!(name);
    }
    gauge!(GC_LAST_SUCCESS).set(0.0);
    gauge!(PARTIAL_CLEANUP_LAST_SUCCESS).set(0.0);
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
    gauge!(
        BUILD_INFO,
        "version" => version.to_string(),
        "commitHash" => commit_hash.unwrap_or("Unknown").to_string(),
        "ethNetwork" => eth_network.to_string()
    )
    .set(1.0);
}

/// One snapshot window written (`result` success) or not (error), `interval_size` its window
/// (`day`, `week`, `month`, `year`, else `unknown`), `reason` why it was generated.
pub fn snapshot_generated(started: Instant, ok: bool, interval_size: &str, reason: &'static str) {
    let interval_size = match interval_size {
        "day" | "week" | "month" | "year" => interval_size,
        _ => "unknown",
    }
    .to_string();
    histogram!(
        SNAPSHOT_GENERATION_TIME,
        "result" => if ok { "success" } else { "error" },
        "interval_size" => interval_size,
        "reason" => reason
    )
    .record(seconds_since(started));
}

/// A `POST /entities` regular deploy answered: `success`, `validation_error` or `error`.
pub fn deployment_answered(kind: &'static str) {
    counter!(DEPLOYMENTS_ENDPOINT, "kind" => kind).increment(1);
}

/// An entity newly stored, by the context it was deployed in.
pub fn deployment_stored(entity_type: &str, context: &'static str) {
    counter!(
        TOTAL_DEPLOYMENTS,
        "entity_type" => self::entity_type(entity_type),
        "deployment_context" => context
    )
    .increment(1);
}

pub fn rate_limited(entity_type: &str, reason: &'static str) {
    counter!(RATE_LIMITED, "entity_type" => self::entity_type(entity_type), "reason" => reason)
        .increment(1);
}

/// The keys a rate-limit window holds; `cache_type` is `deployment` or `unchanged`.
pub fn rate_limiter_keys(entity_type: &str, cache_type: &'static str, keys: usize) {
    gauge!(
        RATE_LIMITER_KEYS,
        "entity_type" => self::entity_type(entity_type),
        "cache_type" => cache_type
    )
    .set(keys as f64);
}

pub fn rate_limiter_max_size(entity_type: &str, max_size: usize) {
    gauge!(RATE_LIMITER_MAX_SIZE, "entity_type" => self::entity_type(entity_type))
        .set(max_size as f64);
}

/// A block read asked again; `network` is `mainnet`, `sepolia`, `polygon` or `amoy`.
pub fn block_fetch_retried(network: &'static str) {
    counter!(BLOCK_FETCH_RETRIES, "network" => network).increment(1);
}

/// `checks` "already deployed" probes that went through the bloom filter: `hit` when the entity
/// is deployed, a miss for a filter miss and for a false positive the database refuted.
pub fn bloom_filter_checked(deployed: bool, checks: usize) {
    counter!(BLOOM_FILTER_CHECKS, "hit" => if deployed { "true" } else { "false" })
        .increment(checks as u64);
}

/// Blobs a collection batch unlinked.
pub fn gc_items(unlinked: u64) {
    counter!(GC_ITEMS).increment(unlinked);
}

/// A collection run ended: `success`, `deferred` (the store lock stayed busy) or `error`.
pub fn gc_run(started: Instant, outcome: &'static str) {
    histogram!(GC_TIME).record(seconds_since(started));
    counter!(GC_RUNS, "outcome" => outcome).increment(1);
    if outcome == "success" {
        gauge!(GC_LAST_SUCCESS).set(unix_seconds_now());
    }
}

/// A partial upload batch answered, by `kind` (see `PARTIAL_KINDS`), and for a 429 of a
/// quota or limit, which one.
pub fn partial_answered(kind: &'static str, throttled: Option<&'static str>) {
    counter!(PARTIAL_STAGING, "kind" => kind).increment(1);
    if let Some(reason) = throttled {
        counter!(PARTIAL_THROTTLED, "reason" => reason).increment(1);
    }
}

pub fn partial_started() {
    counter!(PARTIAL_STARTED).increment(1);
}

/// A partial upload published by its final batch, `batches` stored batches after its first
/// batch arrived at `created_at_ms`.
pub fn partial_completed(created_at_ms: i64, batches: i64) {
    counter!(PARTIAL_COMPLETED).increment(1);
    let elapsed_ms = chrono::Utc::now().timestamp_millis() - created_at_ms;
    histogram!(PARTIAL_DURATION).record(elapsed_ms.max(0) as f64 / 1000.0);
    histogram!(PARTIAL_BATCHES_PER_UPLOAD).record(batches.max(0) as f64);
}

/// One timed stage of a partial upload batch.
pub fn partial_stage(stage: &'static str, started: Instant) {
    histogram!(PARTIAL_STAGE_DURATION, "stage" => stage).record(seconds_since(started));
}

pub fn partial_metadata_checks(hashes: usize) {
    counter!(PARTIAL_METADATA_CHECKS).increment(hashes as u64);
}

/// An accepted batch stored: `incomplete` while content is missing, `finalizing` once it
/// completes the upload.
pub fn partial_batch(complete: bool) {
    let outcome = if complete { "finalizing" } else { "incomplete" };
    counter!(PARTIAL_BATCHES, "outcome" => outcome).increment(1);
}

pub fn partial_capacity(bytes: i64) {
    gauge!(PARTIAL_CAPACITY).set(bytes as f64);
}

pub fn partial_reserved(bytes: i64) {
    gauge!(PARTIAL_RESERVED).set(bytes as f64);
}

/// What `content.pending_deployments` holds at the end of a cleanup run.
pub fn partial_totals(reserved: i64, expired_bytes: i64, live: i64, expired: i64) {
    partial_reserved(reserved);
    gauge!(PARTIAL_CLEANUP_BACKLOG).set(expired_bytes as f64);
    gauge!(PARTIAL_PENDING, "state" => "live").set(live as f64);
    gauge!(PARTIAL_PENDING, "state" => "expired").set(expired as f64);
}

/// A cleanup run ended: `success`, `deferred` or `error`, `removed` uploads reclaimed.
pub fn partial_cleanup(started: Instant, outcome: &'static str, removed: usize) {
    histogram!(PARTIAL_CLEANUP_DURATION).record(seconds_since(started));
    counter!(PARTIAL_CLEANUP_RUNS, "outcome" => outcome).increment(1);
    counter!(PARTIAL_EXPIRED).increment(removed as u64);
    if outcome == "success" {
        gauge!(PARTIAL_CLEANUP_LAST_SUCCESS).set(unix_seconds_now());
    }
}

/// Expired uploads a run could not reclaim: `storage` (an unlink) or `database`.
pub fn partial_cleanup_failures(reason: &'static str, uploads: usize) {
    counter!(PARTIAL_CLEANUP_FAILURES, "reason" => reason).increment(uploads as u64);
}

pub fn upload_budget_capacity(budget: &'static str, bytes: u64) {
    gauge!(UPLOAD_BUDGET_CAPACITY, "budget" => budget).set(bytes as f64);
}

/// A budget's reserved bytes and the uploads holding a share of it.
pub fn upload_budget_usage(budget: &'static str, reserved: u64, active: u64) {
    gauge!(UPLOAD_BUDGET_RESERVED, "budget" => budget).set(reserved as f64);
    gauge!(UPLOAD_BUDGET_ACTIVE, "budget" => budget).set(active as f64);
}

/// An upload shed: `budget` `memory`/`disk` with reason `bytes`, or `source` with
/// `source_bytes`/`source_concurrency`.
pub fn upload_budget_rejected(budget: &'static str, reason: &'static str) {
    counter!(UPLOAD_BUDGET_REJECTIONS, "budget" => budget, "reason" => reason).increment(1);
}

/// A spool failure, by `SPOOL_FAILURE_REASONS`.
pub fn upload_spool_failed(reason: &'static str) {
    counter!(UPLOAD_SPOOL_FAILURES, "reason" => reason).increment(1);
}

/// The `POST /entities` spool's cleanup hook ([`crate::storage::upload_spool::UploadSpool`]).
pub fn upload_spool_cleanup_failed() {
    upload_spool_failed("cleanup");
}

pub fn upload_spool_reclaimed(folders: usize) {
    counter!(UPLOAD_SPOOL_RECLAIMED).increment(folders as u64);
}

pub fn multipart_timed_out() {
    counter!(MULTIPART_TIMEOUTS).increment(1);
}

/// A `POST /entities` client closed the connection while its body was received, once per
/// request (`counted`).
pub fn body_aborted(counted: &AtomicBool) {
    if !counted.swap(true, Ordering::Relaxed) {
        counter!(MULTIPART_ABORTS, "reason" => "client_closed").increment(1);
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
    /// A recorder with this module's buckets, as the serve exporter has them.
    pub fn recorder() -> metrics_exporter_prometheus::PrometheusRecorder {
        let mut builder = metrics_exporter_prometheus::PrometheusBuilder::new();
        for (name, buckets) in super::BUCKETS {
            builder = builder
                .set_buckets_for_metric(
                    metrics_exporter_prometheus::Matcher::Full(name.into()),
                    buckets,
                )
                .unwrap();
        }
        builder.build_recorder()
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

    fn render(record: impl FnOnce()) -> String {
        let recorder = recorder();
        let handle = recorder.handle();
        metrics::with_local_recorder(&recorder, record);
        handle.render()
    }

    #[test]
    fn every_closed_series_renders_before_the_first_event() {
        let rendered = render(register_series);
        for (name, labels) in [
            (DEPLOYMENTS_ENDPOINT, vec![("kind", "validation_error")]),
            (
                TOTAL_DEPLOYMENTS,
                vec![
                    ("entity_type", "scene"),
                    ("deployment_context", "FIX_ATTEMPT"),
                ],
            ),
            (
                RATE_LIMITED,
                vec![("entity_type", "profile"), ("reason", "unchanged_ttl")],
            ),
            (GC_RUNS, vec![("outcome", "deferred")]),
            (GC_ITEMS, vec![]),
            (LOCK_WRITER_TIMEOUTS, vec![]),
            (GC_LAST_SUCCESS, vec![]),
            (PARTIAL_STAGING, vec![("kind", "busy")]),
            (PARTIAL_THROTTLED, vec![("reason", "pointer_conflict")]),
            (PARTIAL_CLEANUP_FAILURES, vec![("reason", "storage")]),
            (PARTIAL_BATCHES, vec![("outcome", "finalizing")]),
            (PARTIAL_CLEANUP_LAST_SUCCESS, vec![]),
            (UPLOAD_SPOOL_FAILURES, vec![("reason", "folder")]),
            (MULTIPART_ABORTS, vec![("reason", "client_closed")]),
            (
                UPLOAD_BUDGET_REJECTIONS,
                vec![("budget", "source"), ("reason", "source_concurrency")],
            ),
            (BLOOM_FILTER_CHECKS, vec![("hit", "false")]),
        ] {
            assert_eq!(
                sample(&rendered, name, &labels),
                Some(0.0),
                "{name}{labels:?} must render at 0:\n{rendered}"
            );
        }
        for name in [GC_ITEMS, PARTIAL_STARTED, MULTIPART_TIMEOUTS] {
            assert!(
                rendered.contains(&format!("# TYPE {name} counter")),
                "{name}"
            );
        }
        for name in [GC_LAST_SUCCESS, PARTIAL_CLEANUP_LAST_SUCCESS] {
            assert!(rendered.contains(&format!("# TYPE {name} gauge")), "{name}");
        }
    }

    #[test]
    fn every_histogram_renders_the_declared_buckets() {
        let rendered = render(|| {
            for (name, _) in BUCKETS {
                histogram!(name).record(1.0);
            }
        });
        for (name, buckets) in BUCKETS {
            assert!(
                rendered.contains(&format!("# TYPE {name} histogram")),
                "{name}: {rendered}"
            );
            let prefix = format!("{name}_bucket{{le=\"");
            let rendered_le: Vec<&str> = rendered
                .lines()
                .filter_map(|line| line.strip_prefix(&prefix))
                .filter_map(|line| line.split('"').next())
                .collect();
            let mut expected: Vec<String> = buckets.iter().map(f64::to_string).collect();
            expected.push("+Inf".to_string());
            assert_eq!(rendered_le, expected, "{name}");
        }
    }

    #[test]
    fn label_values_stay_in_their_closed_sets() {
        let rendered = render(|| {
            deployment_stored("a-peer-chosen-type", "SYNCED");
            snapshot_generated(Instant::now(), true, "fortnight", "cover_time_range");
            build_info("1.2.3", None, "sepolia");
        });
        assert_eq!(
            sample(
                &rendered,
                TOTAL_DEPLOYMENTS,
                &[("entity_type", "unknown"), ("deployment_context", "SYNCED")]
            ),
            Some(1.0)
        );
        assert!(rendered.contains("interval_size=\"unknown\""), "{rendered}");
        assert_eq!(
            sample(
                &rendered,
                BUILD_INFO,
                &[
                    ("version", "1.2.3"),
                    ("commitHash", "Unknown"),
                    ("ethNetwork", "sepolia")
                ]
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
            sample(&rendered, MULTIPART_ABORTS, &[("reason", "client_closed")]),
            Some(2.0),
            "a close the stream reported is not counted again on drop"
        );
    }
}
