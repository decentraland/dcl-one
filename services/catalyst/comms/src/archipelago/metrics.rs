//! Process-lifetime measurements of the connector state exposed by the diagnostics endpoint.
use super::AppState;
use dcl_metrics::{CounterFamily, Descriptor, GaugeFamily};
use serde::Serialize;
use std::time::Duration;

const UPTIME_SECONDS: GaugeFamily<0> = GaugeFamily::new(
    "dcl_connector_uptime_seconds",
    "Connector uptime in seconds.",
    Some(dcl_metrics::Unit::Seconds),
    &[],
);
const PEERS: GaugeFamily<0> = GaugeFamily::new(
    "dcl_connector_peers",
    "Peers in the published statistics snapshot.",
    None,
    &[],
);
const ISLANDS: GaugeFamily<0> = GaugeFamily::new(
    "dcl_connector_islands",
    "Public islands in the statistics snapshot.",
    None,
    &[],
);
const FEED_CONNECTED: GaugeFamily<0> = GaugeFamily::new(
    "dcl_connector_feed_connected",
    "Whether the feed bus is connected.",
    None,
    &[],
);
const FEED_DELIVERED: CounterFamily<0> = CounterFamily::new(
    "dcl_connector_feed_delivered_total",
    "Assignments delivered to socket mailboxes.",
    None,
    &[],
);
const ISLAND_CHANGED_NO_SESSION_SOCKET: CounterFamily<0> = CounterFamily::new(
    "dcl_ws_connector_island_changed_no_session_socket_total",
    "Session-addressed messages whose wallet is held under a different session.",
    None,
    &[],
);
const ISLAND_CHANGED_DEDUPLICATED: CounterFamily<0> = CounterFamily::new(
    "dcl_ws_connector_island_changed_deduplicated_total",
    "Repeated island notifications suppressed by the socket.",
    None,
    &[],
);
const FEED_UNDECODABLE: CounterFamily<0> = CounterFamily::new(
    "dcl_connector_feed_undecodable_total",
    "Feed messages that failed decoding.",
    None,
    &[],
);
const FEED_PUBLISH_DROPPED: CounterFamily<0> = CounterFamily::new(
    "dcl_connector_feed_publish_dropped_total",
    "Locally dropped feed announcements.",
    None,
    &[],
);
const STATISTICS_PUBLISH_DROPPED: CounterFamily<0> = CounterFamily::new(
    "dcl_connector_statistics_publish_dropped_total",
    "Locally dropped statistics observations.",
    None,
    &[],
);
const FEED_ASSIGNMENT_COALESCED: CounterFamily<0> = CounterFamily::new(
    "dcl_connector_feed_assignment_coalesced_total",
    "Assignments coalesced in a socket mailbox.",
    None,
    &[],
);
const FEED_ASSIGNMENT_OVERSIZED: CounterFamily<0> = CounterFamily::new(
    "dcl_connector_feed_assignment_oversized_total",
    "Assignments rejected for exceeding the mailbox size limit.",
    None,
    &[],
);
const SOCKET_WRITE_TIMEOUT: CounterFamily<0> = CounterFamily::new(
    "dcl_connector_socket_write_timeout_total",
    "Socket writes that timed out.",
    None,
    &[],
);
const SOCKET_ASSIGNMENTS_WRITTEN: CounterFamily<0> = CounterFamily::new(
    "dcl_connector_socket_assignments_written_total",
    "Assignments written to the socket.",
    None,
    &[],
);
const FEED_ASSIGNMENT_FULL: CounterFamily<0> = CounterFamily::new(
    "dcl_connector_feed_assignment_full_total",
    "Assignments rejected because the mailbox was full.",
    None,
    &[],
);
const ISLAND_CHANGED_DROPPED_CLOSE: CounterFamily<0> = CounterFamily::new(
    "dcl_ws_connector_island_changed_dropped_close_total",
    "Sockets closed after an island assignment was dropped.",
    None,
    &[],
);

pub const FAMILIES: &[Descriptor] = &[
    UPTIME_SECONDS.descriptor,
    PEERS.descriptor,
    ISLANDS.descriptor,
    FEED_CONNECTED.descriptor,
    FEED_DELIVERED.descriptor,
    ISLAND_CHANGED_NO_SESSION_SOCKET.descriptor,
    ISLAND_CHANGED_DEDUPLICATED.descriptor,
    FEED_UNDECODABLE.descriptor,
    FEED_PUBLISH_DROPPED.descriptor,
    STATISTICS_PUBLISH_DROPPED.descriptor,
    FEED_ASSIGNMENT_COALESCED.descriptor,
    FEED_ASSIGNMENT_OVERSIZED.descriptor,
    SOCKET_WRITE_TIMEOUT.descriptor,
    SOCKET_ASSIGNMENTS_WRITTEN.descriptor,
    FEED_ASSIGNMENT_FULL.descriptor,
    ISLAND_CHANGED_DROPPED_CLOSE.descriptor,
];

pub(super) fn record_diagnostics(state: &AppState) {
    let snapshot = health(state);
    UPTIME_SECONDS.handle([]).set(snapshot.uptime_secs as f64);
    PEERS.handle([]).set(snapshot.peers_total as f64);
    ISLANDS.handle([]).set(snapshot.islands_total as f64);
    FEED_CONNECTED
        .handle([])
        .set(u8::from(snapshot.feed_connected) as f64);
    FEED_DELIVERED.handle([]).absolute(snapshot.feed_delivered);
    ISLAND_CHANGED_NO_SESSION_SOCKET
        .handle([])
        .absolute(snapshot.feed_no_session_socket);
    ISLAND_CHANGED_DEDUPLICATED
        .handle([])
        .absolute(snapshot.feed_deduplicated);
    FEED_UNDECODABLE
        .handle([])
        .absolute(snapshot.feed_undecodable);
    FEED_PUBLISH_DROPPED
        .handle([])
        .absolute(snapshot.feed_publish_dropped);
    STATISTICS_PUBLISH_DROPPED
        .handle([])
        .absolute(snapshot.statistics_publish_dropped);
    FEED_ASSIGNMENT_COALESCED
        .handle([])
        .absolute(snapshot.feed_assignment_coalesced);
    FEED_ASSIGNMENT_OVERSIZED
        .handle([])
        .absolute(snapshot.feed_assignment_oversized);
    SOCKET_WRITE_TIMEOUT
        .handle([])
        .absolute(snapshot.socket_write_timeout);
    SOCKET_ASSIGNMENTS_WRITTEN
        .handle([])
        .absolute(snapshot.socket_assignments_written);
    FEED_ASSIGNMENT_FULL
        .handle([])
        .absolute(snapshot.feed_assignment_full);
    ISLAND_CHANGED_DROPPED_CLOSE
        .handle([])
        .absolute(state.feed.dropped_close_count());
}
/// Emit the same snapshot as the public JSON diagnostics on the process recorder.
/// This sampler is independent of public-host scrapes and their midnight resets.
pub(super) fn start_metrics(state: &AppState, jobs: &crate::jobs::Jobs) {
    let state = state.clone();
    jobs.spawn_periodic(
        "archipelago-metrics",
        crate::jobs::PeriodicCfg::every(Duration::from_secs(15)),
        move |_| {
            record_diagnostics(&state);
            std::future::ready(Ok(()))
        },
    );
}

/// The feed counters are appended, never reordered: the byte-exact shape assertions upstream
/// parity keeps are written against the first four fields.
#[derive(Serialize)]
struct HealthResp {
    healthy: bool,
    uptime_secs: i64,
    peers_total: usize,
    islands_total: usize,
    feed_connected: bool,
    feed_delivered: u64,
    feed_no_session_socket: u64,
    feed_deduplicated: u64,
    feed_undecodable: u64,
    feed_publish_dropped: u64,
    statistics_publish_dropped: u64,
    feed_assignment_coalesced: u64,
    feed_assignment_oversized: u64,
    socket_write_timeout: u64,
    socket_assignments_written: u64,
    feed_assignment_full: u64,
}

fn health(s: &AppState) -> HealthResp {
    let snapshot = s.statistics.snapshot();
    let uptime = s.started_at.elapsed().as_secs() as i64;
    HealthResp {
        healthy: true,
        uptime_secs: uptime,
        peers_total: snapshot.peers.len(),
        islands_total: crate::stats::render::public_island_count(&snapshot),
        feed_connected: s.outbox.is_linked(),
        feed_delivered: s.feed.delivered_count(),
        feed_no_session_socket: s.feed.no_session_socket_count(),
        feed_deduplicated: s.feed.deduplicated_count(),
        feed_undecodable: s.feed.undecodable_count(),
        feed_publish_dropped: s.outbox.dropped(),
        statistics_publish_dropped: s.observations.dropped(),
        feed_assignment_coalesced: s.feed.coalesced_count(),
        feed_assignment_oversized: s.feed.assignment_oversized_count(),
        socket_write_timeout: s.feed.socket_write_timeout_count(),
        socket_assignments_written: s.feed.socket_assignments_written_count(),
        feed_assignment_full: s.feed.assignment_full_count(),
    }
}

pub(super) fn diagnostics(s: &AppState) -> String {
    crate::stats::render::json(&health(s))
}
