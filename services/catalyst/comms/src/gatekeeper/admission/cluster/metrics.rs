//! Prometheus series for the Pulse cluster feed, over the `metrics` facade (exporter-agnostic:
//! a no-op recorder when none is installed, so these are safe to call from tests).
//!
//! Series carry the `comms_` prefix, not upstream's `dcl_gatekeeper_`, matching this workspace's
//! per-crate prefix convention (`pulse_`, `catalyrst_http_`); keep every new one on it so a single
//! exposition never needs two prefixes on one dashboard.
//!
//! `serve` installs the shared exporter and registers these series before starting
//! the subscriber; tests may use a thread-local recorder.

use crate::metrics::declarations::*;
use dcl_metrics::CounterFamily;

const COUNTERS: [CounterFamily<0>; 23] = [
    EVENTS_RECEIVED,
    SELECTED_REFUSED,
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
    WS_UNATTRIBUTED,
    WS_PENDING_EVICTED,
];

/// Renders every cluster series at 0 so a dashboard and an alert both have a value to compare
/// against from process start, instead of a panel that stays empty until the first event.
pub fn register_series() {
    crate::gatekeeper::bus::metrics::nats_connected(false);
    for family in COUNTERS {
        family.handle([]).increment(0);
    }
    for reason in ["wallet_full", "global_full", "retired"] {
        WORK_REJECTED.handle([reason]).increment(0);
    }
    cluster_work_depths(0, 0);
    selected_slot_occupancy(0);
    ws_pending_handshakes(0);
}

pub(crate) fn selected_slot_occupancy(slots: usize) {
    SELECTED_SLOTS.handle([]).set(slots as f64);
}
pub(crate) fn selected_slot_refused() {
    SELECTED_REFUSED.handle([]).increment(1);
}

pub fn cluster_event_received() {
    EVENTS_RECEIVED.handle([]).increment(1);
}

pub fn cluster_connect_received() {
    CONNECTS_RECEIVED.handle([]).increment(1);
}

pub fn cluster_token_minted() {
    TOKENS_MINTED.handle([]).increment(1);
}

/// Assignments a socket in this process took, or the realm-lane fence committed.
pub fn cluster_published() {
    PUBLISHED.handle([]).increment(1);
}

/// Minted assignments no socket in this process took: none of that wallet (and session) here,
/// none attached, or one that refused it. Island assignments reach only sockets in the minting
/// process, so a second `serve` serving `/ws` beside this one shows here.
pub fn cluster_island_undelivered() {
    ISLAND_UNDELIVERED.handle([]).increment(1);
}

/// Fenced assignments the realm-lane authority refused to commit.
pub fn cluster_publish_failed() {
    PUBLISH_FAILED.handle([]).increment(1);
}

pub fn cluster_banned_skipped() {
    BANNED_SKIPPED.handle([]).increment(1);
}

pub fn cluster_access_check_failed() {
    ACCESS_CHECK_FAILED.handle([]).increment(1);
}

pub fn cluster_takeover_evicted() {
    TAKEOVER_EVICTED.handle([]).increment(1);
}

pub fn cluster_takeover_absent() {
    TAKEOVER_ABSENT.handle([]).increment(1);
}

pub fn cluster_takeover_failed() {
    TAKEOVER_FAILED.handle([]).increment(1);
}

pub fn cluster_reannounce_attempted() {
    REANNOUNCE_ATTEMPTED.handle([]).increment(1);
}

pub fn cluster_reannounce_suppressed() {
    REANNOUNCE_SUPPRESSED.handle([]).increment(1);
}

pub fn cluster_reannounce_unresolved() {
    REANNOUNCE_UNRESOLVED.handle([]).increment(1);
}

pub fn cluster_reannounce_check_failed() {
    REANNOUNCE_CHECK_FAILED.handle([]).increment(1);
}

pub fn cluster_recovery_stale() {
    RECOVERY_STALE.handle([]).increment(1);
}

pub(crate) fn cluster_work_rejected(reason: &'static str) {
    WORK_REJECTED.handle([reason]).increment(1);
}

pub(crate) fn cluster_work_expired() {
    WORK_EXPIRED.handle([]).increment(1);
}

pub(crate) fn cluster_work_cancelled() {
    WORK_CANCELLED.handle([]).increment(1);
}

pub(crate) fn cluster_work_depths(running: usize, queued: usize) {
    WORK_RUNNING.handle([]).set(running as f64);
    WORK_QUEUED.handle([]).set(queued as f64);
}

pub(crate) fn cluster_feed_oversized() {
    FEED_OVERSIZED.handle([]).increment(1);
}

pub(crate) fn cluster_feed_invalid_subject() {
    FEED_INVALID_SUBJECT.handle([]).increment(1);
}

/// A pending `/ws` handshake closed because a newer socket took its place in a full
/// `WS_MAX_PENDING_HANDSHAKES` share. A steady rate under normal load means the share is too small
/// for the node's join rate.
pub(crate) fn ws_pending_eviction() {
    WS_PENDING_EVICTED.handle([]).increment(1);
}

/// `/ws` sockets currently in the handshake, out of `WS_MAX_PENDING_HANDSHAKES`.
pub(crate) fn ws_pending_handshakes(held: usize) {
    WS_PENDING_HELD.handle([]).set(held as f64);
}

/// A `/ws` upgrade the per-client cap cannot attribute; warns on the first
/// one in this process and answers whether it did.
pub(crate) fn ws_unattributed_upgrade() -> bool {
    static WARNED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    note_unattributed_upgrade(&WARNED)
}

fn note_unattributed_upgrade(warned: &std::sync::atomic::AtomicBool) -> bool {
    WS_UNATTRIBUTED.handle([]).increment(1);
    let first = !warned.swap(true, std::sync::atomic::Ordering::Relaxed);
    if first {
        tracing::warn!(
            "a /ws upgrade has no attributable client address (gateway peer outside HTTP_TRUSTED_PROXY_CIDRS, or no X-Real-IP); WS_MAX_SOCKETS_PER_IP does not count such sockets"
        );
    }
    first
}

#[cfg(test)]
mod tests {
    use super::*;
    use dcl_metrics::PrometheusBuilder;

    /// Renders what `f` recorded through a recorder local to this thread, so a series can be
    /// asserted without installing a global one.
    pub(crate) fn render_recorded<T>(f: impl FnOnce() -> T) -> String {
        let recorder = PrometheusBuilder::new().build_recorder();
        let handle = recorder.handle();
        dcl_metrics::with_local_recorder(&recorder, f);
        handle.render()
    }

    #[test]
    fn pending_handshake_evictions_and_depth_are_exported() {
        let rendered = render_recorded(|| {
            let pending = crate::archipelago::state::PendingHandshakes::new(2);
            let _oldest = pending.admit();
            let _middle = pending.admit();
            let _newest = pending.admit();
            let _next = pending.admit();
        });
        assert!(
            rendered.contains("comms_ws_pending_evictions_total 2"),
            "{rendered}"
        );
        // The last sample is taken as the slots drop at the end of the closure.
        assert!(
            rendered.contains("comms_ws_pending_handshakes 0"),
            "{rendered}"
        );
    }

    #[test]
    fn unattributed_upgrades_count_every_one_and_warn_once() {
        let once = std::sync::atomic::AtomicBool::new(false);
        let mut warned = Vec::new();
        let rendered = render_recorded(|| {
            for _ in 0..3 {
                warned.push(note_unattributed_upgrade(&once));
            }
        });
        assert_eq!(warned, [true, false, false]);
        assert!(
            rendered.contains("comms_ws_unattributed_upgrades_total 3"),
            "{rendered}"
        );
    }

    #[test]
    fn every_cluster_series_renders_before_the_first_event() {
        let rendered = render_recorded(register_series);
        for name in COUNTERS.iter().map(|family| family.descriptor.name).chain([
            NATS_CONNECTED.descriptor.name,
            WORK_RUNNING.descriptor.name,
            WORK_QUEUED.descriptor.name,
            SELECTED_SLOTS.descriptor.name,
            WS_PENDING_HELD.descriptor.name,
        ]) {
            assert!(
                rendered.contains(name),
                "{name} must render at 0 from process start, got:\n{rendered}"
            );
        }
        for reason in ["wallet_full", "global_full", "retired"] {
            assert!(rendered.contains(&format!(
                "comms_cluster_work_rejected_total{{reason=\"{reason}\"}} 0"
            )));
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
            selected_slot_occupancy(20_000);
            selected_slot_refused();
        });
        for sample in [
            "comms_cluster_work_rejected_total{reason=\"wallet_full\"} 1",
            "comms_cluster_work_rejected_total{reason=\"global_full\"} 1",
            "comms_cluster_work_expired_total 1",
            "comms_cluster_work_cancelled_total 1",
            "comms_cluster_feed_oversized_total 1",
            "comms_cluster_feed_invalid_subject_total 1",
            "comms_cluster_selected_slots 20000",
            "comms_cluster_selected_slot_refused_total 1",
            "comms_cluster_work_running 32",
            "comms_cluster_work_queued 992",
        ] {
            assert!(rendered.contains(sample), "missing {sample}: {rendered}");
        }
    }
}
