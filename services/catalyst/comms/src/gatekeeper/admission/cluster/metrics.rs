//! Prometheus series for the Pulse cluster feed, over the `metrics` facade (exporter-agnostic:
//! a no-op recorder when none is installed, so these are safe to call from tests).
//!
//! Series carry the `comms_` prefix, not upstream's `dcl_gatekeeper_`, matching this workspace's
//! per-crate prefix convention (`pulse_`, `catalyrst_http_`); keep every new one on it so a single
//! exposition never needs two prefixes on one dashboard.
//!
//! `serve` installs the shared exporter and registers these series before starting
//! the subscriber; tests may use a thread-local recorder.

use metrics::{counter, gauge};

const EVENTS_RECEIVED: &str = "comms_cluster_events_received_total";
const CONNECTS_RECEIVED: &str = "comms_cluster_connects_received_total";
const TOKENS_MINTED: &str = "comms_cluster_tokens_minted_total";
const PUBLISHED: &str = "comms_cluster_published_total";
const ISLAND_UNDELIVERED: &str = "comms_cluster_island_undelivered_total";
const PUBLISH_FAILED: &str = "comms_cluster_publish_failed_total";
const BANNED_SKIPPED: &str = "comms_cluster_banned_skipped_total";
const ACCESS_CHECK_FAILED: &str = "comms_cluster_access_check_failed_total";
const TAKEOVER_EVICTED: &str = "comms_cluster_takeover_evicted_total";
const TAKEOVER_ABSENT: &str = "comms_cluster_takeover_absent_total";
const TAKEOVER_FAILED: &str = "comms_cluster_takeover_failed_total";
const REANNOUNCE_ATTEMPTED: &str = "comms_cluster_reannounce_attempted_total";
const REANNOUNCE_SUPPRESSED: &str = "comms_cluster_reannounce_suppressed_total";
const REANNOUNCE_UNRESOLVED: &str = "comms_cluster_reannounce_unresolved_total";
const REANNOUNCE_CHECK_FAILED: &str = "comms_cluster_reannounce_check_failed_total";
const RECOVERY_STALE: &str = "comms_cluster_recovery_stale_total";
const WORK_REJECTED: &str = "comms_cluster_work_rejected_total";
const WORK_EXPIRED: &str = "comms_cluster_work_expired_total";
const WORK_CANCELLED: &str = "comms_cluster_work_cancelled_total";
const WORK_RUNNING: &str = "comms_cluster_work_running";
const WORK_QUEUED: &str = "comms_cluster_work_queued";
const FEED_OVERSIZED: &str = "comms_cluster_feed_oversized_total";
const FEED_INVALID_SUBJECT: &str = "comms_cluster_feed_invalid_subject_total";

const COUNTERS: [&str; 20] = [
    EVENTS_RECEIVED,
    CONNECTS_RECEIVED,
    TOKENS_MINTED,
    PUBLISHED,
    ISLAND_UNDELIVERED,
    PUBLISH_FAILED,
    BANNED_SKIPPED,
    ACCESS_CHECK_FAILED,
    TAKEOVER_EVICTED,
    TAKEOVER_ABSENT,
    TAKEOVER_FAILED,
    REANNOUNCE_ATTEMPTED,
    REANNOUNCE_SUPPRESSED,
    REANNOUNCE_UNRESOLVED,
    REANNOUNCE_CHECK_FAILED,
    RECOVERY_STALE,
    WORK_EXPIRED,
    WORK_CANCELLED,
    FEED_OVERSIZED,
    FEED_INVALID_SUBJECT,
];

/// Renders every cluster series at 0 so a dashboard and an alert both have a value to compare
/// against from process start, instead of a panel that stays empty until the first event.
pub fn register_series() {
    crate::gatekeeper::bus::metrics::nats_connected(false);
    for name in COUNTERS {
        counter!(name).increment(0);
    }
    for reason in ["wallet_full", "global_full", "retired"] {
        counter!(WORK_REJECTED, "reason" => reason).increment(0);
    }
    cluster_work_depths(0, 0);
}

pub fn cluster_event_received() {
    counter!(EVENTS_RECEIVED).increment(1);
}

pub fn cluster_connect_received() {
    counter!(CONNECTS_RECEIVED).increment(1);
}

pub fn cluster_token_minted() {
    counter!(TOKENS_MINTED).increment(1);
}

/// Assignments a socket in this process took, or the realm-lane fence committed.
pub fn cluster_published() {
    counter!(PUBLISHED).increment(1);
}

/// Minted assignments no socket in this process took: none of that wallet (and session) here,
/// none attached, or one that refused it. Island assignments reach only sockets in the minting
/// process, so a second `serve` serving `/ws` beside this one shows here.
pub fn cluster_island_undelivered() {
    counter!(ISLAND_UNDELIVERED).increment(1);
}

/// Fenced assignments the realm-lane authority refused to commit.
pub fn cluster_publish_failed() {
    counter!(PUBLISH_FAILED).increment(1);
}

pub fn cluster_banned_skipped() {
    counter!(BANNED_SKIPPED).increment(1);
}

pub fn cluster_access_check_failed() {
    counter!(ACCESS_CHECK_FAILED).increment(1);
}

pub fn cluster_takeover_evicted() {
    counter!(TAKEOVER_EVICTED).increment(1);
}

pub fn cluster_takeover_absent() {
    counter!(TAKEOVER_ABSENT).increment(1);
}

pub fn cluster_takeover_failed() {
    counter!(TAKEOVER_FAILED).increment(1);
}

pub fn cluster_reannounce_attempted() {
    counter!(REANNOUNCE_ATTEMPTED).increment(1);
}

pub fn cluster_reannounce_suppressed() {
    counter!(REANNOUNCE_SUPPRESSED).increment(1);
}

pub fn cluster_reannounce_unresolved() {
    counter!(REANNOUNCE_UNRESOLVED).increment(1);
}

pub fn cluster_reannounce_check_failed() {
    counter!(REANNOUNCE_CHECK_FAILED).increment(1);
}

pub fn cluster_recovery_stale() {
    counter!(RECOVERY_STALE).increment(1);
}

pub(crate) fn cluster_work_rejected(reason: &'static str) {
    counter!(WORK_REJECTED, "reason" => reason).increment(1);
}

pub(crate) fn cluster_work_expired() {
    counter!(WORK_EXPIRED).increment(1);
}

pub(crate) fn cluster_work_cancelled() {
    counter!(WORK_CANCELLED).increment(1);
}

pub(crate) fn cluster_work_depths(running: usize, queued: usize) {
    gauge!(WORK_RUNNING).set(running as f64);
    gauge!(WORK_QUEUED).set(queued as f64);
}

pub(crate) fn cluster_feed_oversized() {
    counter!(FEED_OVERSIZED).increment(1);
}

pub(crate) fn cluster_feed_invalid_subject() {
    counter!(FEED_INVALID_SUBJECT).increment(1);
}

#[cfg(test)]
mod tests {
    use super::*;
    use metrics_exporter_prometheus::PrometheusBuilder;

    /// Renders what `f` recorded through a recorder local to this thread, so a series can be
    /// asserted without installing a global one.
    pub(crate) fn render_recorded<T>(f: impl FnOnce() -> T) -> String {
        let recorder = PrometheusBuilder::new().build_recorder();
        let handle = recorder.handle();
        metrics::with_local_recorder(&recorder, f);
        handle.render()
    }

    #[test]
    fn every_cluster_series_renders_before_the_first_event() {
        let rendered = render_recorded(register_series);
        for name in COUNTERS.iter().chain(
            [
                crate::gatekeeper::bus::metrics::NATS_CONNECTED,
                WORK_RUNNING,
                WORK_QUEUED,
            ]
            .iter(),
        ) {
            assert!(
                rendered.contains(name),
                "{name} must render at 0 from process start, got:\n{rendered}"
            );
        }
        for reason in ["wallet_full", "global_full", "retired"] {
            assert!(rendered.contains(&format!("{WORK_REJECTED}{{reason=\"{reason}\"}} 0")));
        }
    }

    #[test]
    fn work_pressure_series_distinguish_rejection_expiry_and_depth() {
        let rendered = render_recorded(|| {
            register_series();
            cluster_work_rejected("wallet_full");
            cluster_work_rejected("global_full");
            cluster_work_expired();
            cluster_work_cancelled();
            cluster_feed_oversized();
            cluster_feed_invalid_subject();
            cluster_work_depths(32, 992);
        });
        for sample in [
            format!("{WORK_REJECTED}{{reason=\"wallet_full\"}} 1"),
            format!("{WORK_REJECTED}{{reason=\"global_full\"}} 1"),
            format!("{WORK_EXPIRED} 1"),
            format!("{WORK_CANCELLED} 1"),
            format!("{FEED_OVERSIZED} 1"),
            format!("{FEED_INVALID_SUBJECT} 1"),
            format!("{WORK_RUNNING} 32"),
            format!("{WORK_QUEUED} 992"),
        ] {
            assert!(rendered.contains(&sample), "missing {sample}: {rendered}");
        }
    }
}
