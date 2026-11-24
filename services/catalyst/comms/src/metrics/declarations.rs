//! Gatekeeper, recovery, presence and topology metric contracts.
use dcl_metrics::{CounterFamily, Descriptor, GaugeFamily, Unit};

pub(crate) const WORK_REJECTED: CounterFamily<1> = CounterFamily::new(
    "comms_cluster_work_rejected_total",
    "Cluster work refused on admission, by queue or ownership limit.",
    Some(Unit::Count),
    &["reason"],
);
pub(crate) const SELECTED_SLOTS: GaugeFamily<0> = GaugeFamily::new(
    "comms_cluster_selected_slots",
    "Slots occupied by selected cluster assignments.",
    None,
    &[],
);
pub(crate) const SELECTED_REFUSED: CounterFamily<0> = CounterFamily::new(
    "comms_cluster_selected_slot_refused_total",
    "Cluster selections refused because their slot could not be acquired.",
    Some(Unit::Count),
    &[],
);
pub(crate) const EVENTS_RECEIVED: CounterFamily<0> = CounterFamily::new(
    "comms_cluster_events_received_total",
    "Cluster events received by the subscriber.",
    Some(Unit::Count),
    &[],
);
pub(crate) const CONNECTS_RECEIVED: CounterFamily<0> = CounterFamily::new(
    "comms_cluster_connects_received_total",
    "Cluster connect events received by the subscriber.",
    Some(Unit::Count),
    &[],
);
pub(crate) const TOKENS_MINTED: CounterFamily<0> = CounterFamily::new(
    "comms_cluster_tokens_minted_total",
    "Access tokens minted for cluster assignments.",
    Some(Unit::Count),
    &[],
);
pub(crate) const PUBLISHED: CounterFamily<0> = CounterFamily::new(
    "comms_cluster_published_total",
    "Assignments accepted by a local socket or committed by the realm lane.",
    Some(Unit::Count),
    &[],
);
pub(crate) const ISLAND_UNDELIVERED: CounterFamily<0> = CounterFamily::new(
    "comms_cluster_island_undelivered_total",
    "Minted island assignments accepted by no local socket.",
    Some(Unit::Count),
    &[],
);
pub(crate) const PUBLISH_FAILED: CounterFamily<0> = CounterFamily::new(
    "comms_cluster_publish_failed_total",
    "Fenced assignments refused by the realm lane.",
    Some(Unit::Count),
    &[],
);
pub(crate) const BANNED_SKIPPED: CounterFamily<0> = CounterFamily::new(
    "comms_cluster_banned_skipped_total",
    "Cluster admissions skipped for banned participants.",
    Some(Unit::Count),
    &[],
);
pub(crate) const ACCESS_CHECK_FAILED: CounterFamily<0> = CounterFamily::new(
    "comms_cluster_access_check_failed_total",
    "Failed access checks during cluster admission.",
    Some(Unit::Count),
    &[],
);
pub(crate) const TAKEOVER_EVICTED: CounterFamily<0> = CounterFamily::new(
    "comms_cluster_takeover_evicted_total",
    "Participants evicted during a cluster takeover.",
    Some(Unit::Count),
    &[],
);
pub(crate) const TAKEOVER_ABSENT: CounterFamily<0> = CounterFamily::new(
    "comms_cluster_takeover_absent_total",
    "Takeovers where the previous participant was absent.",
    Some(Unit::Count),
    &[],
);
pub(crate) const TAKEOVER_FAILED: CounterFamily<0> = CounterFamily::new(
    "comms_cluster_takeover_failed_total",
    "Failed attempts to enforce a cluster takeover.",
    Some(Unit::Count),
    &[],
);
pub(crate) const REANNOUNCE_ATTEMPTED: CounterFamily<0> = CounterFamily::new(
    "comms_cluster_reannounce_attempted_total",
    "Cluster recovery announcements attempted.",
    Some(Unit::Count),
    &[],
);
pub(crate) const REANNOUNCE_SUPPRESSED: CounterFamily<0> = CounterFamily::new(
    "comms_cluster_reannounce_suppressed_total",
    "Cluster recovery announcements suppressed by admission checks.",
    Some(Unit::Count),
    &[],
);
pub(crate) const REANNOUNCE_UNRESOLVED: CounterFamily<0> = CounterFamily::new(
    "comms_cluster_reannounce_unresolved_total",
    "Cluster recovery announcements without a resolved assignment.",
    Some(Unit::Count),
    &[],
);
pub(crate) const REANNOUNCE_CHECK_FAILED: CounterFamily<0> = CounterFamily::new(
    "comms_cluster_reannounce_check_failed_total",
    "Failed eligibility checks during cluster reannouncement.",
    Some(Unit::Count),
    &[],
);
pub(crate) const RECOVERY_STALE: CounterFamily<0> = CounterFamily::new(
    "comms_cluster_recovery_stale_total",
    "Recovery operations discarded after the observed assignment changed.",
    Some(Unit::Count),
    &[],
);
pub(crate) const WORK_EXPIRED: CounterFamily<0> = CounterFamily::new(
    "comms_cluster_work_expired_total",
    "Cluster work items whose deadline elapsed.",
    Some(Unit::Count),
    &[],
);
pub(crate) const WORK_CANCELLED: CounterFamily<0> = CounterFamily::new(
    "comms_cluster_work_cancelled_total",
    "Cluster work items cancelled when their owner retired.",
    Some(Unit::Count),
    &[],
);
pub(crate) const WORK_RUNNING: GaugeFamily<0> = GaugeFamily::new(
    "comms_cluster_work_running",
    "Cluster work items currently running.",
    None,
    &[],
);
pub(crate) const WORK_QUEUED: GaugeFamily<0> = GaugeFamily::new(
    "comms_cluster_work_queued",
    "Cluster work items waiting for execution.",
    None,
    &[],
);
pub(crate) const FEED_OVERSIZED: CounterFamily<0> = CounterFamily::new(
    "comms_cluster_feed_oversized_total",
    "Oversized messages rejected by the cluster feed.",
    Some(Unit::Count),
    &[],
);
pub(crate) const FEED_INVALID_SUBJECT: CounterFamily<0> = CounterFamily::new(
    "comms_cluster_feed_invalid_subject_total",
    "Cluster feed messages rejected for an invalid subject.",
    Some(Unit::Count),
    &[],
);
pub(crate) const WS_PENDING_EVICTED: CounterFamily<0> = CounterFamily::new(
    "comms_ws_pending_evictions_total",
    "Pending connector handshakes evicted to admit newer sockets.",
    Some(Unit::Count),
    &[],
);
pub(crate) const WS_PENDING_HELD: GaugeFamily<0> = GaugeFamily::new(
    "comms_ws_pending_handshakes",
    "Connector WebSockets currently holding a pending handshake slot.",
    None,
    &[],
);
pub(crate) const WS_UNATTRIBUTED: CounterFamily<0> = CounterFamily::new(
    "comms_ws_unattributed_upgrades_total",
    "Connector upgrades without an attributable client address.",
    Some(Unit::Count),
    &[],
);
pub(crate) const ISLAND_SAFETY_CAPACITY_REFUSED: CounterFamily<0> = CounterFamily::new(
    "comms_island_safety_capacity_refused_total",
    "Island safety operations refused at capacity.",
    Some(Unit::Count),
    &[],
);
pub(crate) const ISLAND_CLEANUP_MAINTENANCE_FAILED: CounterFamily<0> = CounterFamily::new(
    "comms_island_cleanup_maintenance_failed_total",
    "Failed island cleanup maintenance operations.",
    Some(Unit::Count),
    &[],
);
pub(crate) const NATS_CONNECTED: GaugeFamily<0> = GaugeFamily::new(
    "comms_nats_connected",
    "Whether the current NATS connection has completed a broker round trip (1 or 0).",
    None,
    &[],
);
pub(crate) const LIVEKIT_REMOVAL_FAILURES: CounterFamily<0> = CounterFamily::new(
    "comms_livekit_removal_failures_total",
    "Failed LiveKit removal attempts retained for retry.",
    Some(Unit::Count),
    &[],
);
pub(crate) const LIVEKIT_REMOVAL_PENDING: GaugeFamily<0> = GaugeFamily::new(
    "comms_livekit_removal_pending",
    "Outstanding durable LiveKit removal intents.",
    None,
    &[],
);
pub(crate) const LIVEKIT_REMOVAL_OLDEST_SECONDS: GaugeFamily<0> = GaugeFamily::new(
    "comms_livekit_removal_oldest_seconds",
    "Age in seconds of the oldest outstanding LiveKit removal intent.",
    Some(Unit::Seconds),
    &[],
);
pub(crate) const LIVEKIT_REMOVAL_MAX_ATTEMPTS: GaugeFamily<0> = GaugeFamily::new(
    "comms_livekit_removal_max_attempts",
    "Largest attempt count among outstanding LiveKit removal intents.",
    None,
    &[],
);
pub(crate) const COMMUNITY_VOICE_STATUS_READ_FAILURES: CounterFamily<1> = CounterFamily::new(
    "comms_community_voice_status_read_failures_total",
    "Failed reads of community voice status, by route.",
    Some(Unit::Count),
    &["route"],
);
pub(crate) const PRIVATE_MESSAGES_PRIVACY_ROOM_FAILURES: CounterFamily<0> = CounterFamily::new(
    "comms_private_messages_privacy_room_failures_total",
    "Failed updates to participant privacy metadata in the private messaging room.",
    Some(Unit::Count),
    &[],
);
pub(crate) const ROOM_EVENT_PUBLISH_FAILED: CounterFamily<1> = CounterFamily::new(
    "comms_room_event_publish_failed_total",
    "Room event publications to the enabled bus that were unsuccessful or unconfirmed.",
    Some(Unit::Count),
    &["reason"],
);
pub(crate) const PRESENCE_REFUSED_ROOMS: GaugeFamily<0> = GaugeFamily::new(
    "comms_presence_refused_rooms",
    "Rooms excluded from the latest presence recovery observation because this node could not resolve them.",
    None,
    &[],
);
pub(crate) const PRESENCE_RECOVERY_PASSES: CounterFamily<1> = CounterFamily::new(
    "comms_presence_recovery_passes_total",
    "Completed LiveKit presence recovery passes, by result.",
    Some(Unit::Count),
    &["result"],
);
pub(crate) const WORLD_PRESENCE_DROPPED: CounterFamily<1> = CounterFamily::new(
    "comms_world_presence_dropped_total",
    "World presence updates discarded at a capacity limit.",
    Some(Unit::Count),
    &["reason"],
);
pub(crate) const WORLD_PRESENCE_SESSIONS: GaugeFamily<0> = GaugeFamily::new(
    "comms_world_presence_sessions",
    "Live world presence sessions in the local topology.",
    None,
    &[],
);
pub(crate) const WORLD_PRESENCE_WALLETS: GaugeFamily<0> = GaugeFamily::new(
    "comms_world_presence_wallets",
    "Distinct wallets present in the world topology view.",
    None,
    &[],
);
pub(crate) const ISLAND_CLEANUP_UNCERTAIN: CounterFamily<0> = CounterFamily::new(
    "comms_island_cleanup_uncertain_total",
    "Island cleanup failures with an uncertain dispatch outcome.",
    Some(Unit::Count),
    &[],
);

pub const FAMILIES: &[Descriptor] = &[
    WORK_REJECTED.descriptor,
    SELECTED_SLOTS.descriptor,
    SELECTED_REFUSED.descriptor,
    EVENTS_RECEIVED.descriptor,
    CONNECTS_RECEIVED.descriptor,
    TOKENS_MINTED.descriptor,
    PUBLISHED.descriptor,
    ISLAND_UNDELIVERED.descriptor,
    PUBLISH_FAILED.descriptor,
    BANNED_SKIPPED.descriptor,
    ACCESS_CHECK_FAILED.descriptor,
    TAKEOVER_EVICTED.descriptor,
    TAKEOVER_ABSENT.descriptor,
    TAKEOVER_FAILED.descriptor,
    REANNOUNCE_ATTEMPTED.descriptor,
    REANNOUNCE_SUPPRESSED.descriptor,
    REANNOUNCE_UNRESOLVED.descriptor,
    REANNOUNCE_CHECK_FAILED.descriptor,
    RECOVERY_STALE.descriptor,
    WORK_EXPIRED.descriptor,
    WORK_CANCELLED.descriptor,
    WORK_RUNNING.descriptor,
    WORK_QUEUED.descriptor,
    FEED_OVERSIZED.descriptor,
    FEED_INVALID_SUBJECT.descriptor,
    WS_PENDING_EVICTED.descriptor,
    WS_PENDING_HELD.descriptor,
    WS_UNATTRIBUTED.descriptor,
    ISLAND_SAFETY_CAPACITY_REFUSED.descriptor,
    ISLAND_CLEANUP_MAINTENANCE_FAILED.descriptor,
    NATS_CONNECTED.descriptor,
    LIVEKIT_REMOVAL_FAILURES.descriptor,
    LIVEKIT_REMOVAL_PENDING.descriptor,
    LIVEKIT_REMOVAL_OLDEST_SECONDS.descriptor,
    LIVEKIT_REMOVAL_MAX_ATTEMPTS.descriptor,
    COMMUNITY_VOICE_STATUS_READ_FAILURES.descriptor,
    PRIVATE_MESSAGES_PRIVACY_ROOM_FAILURES.descriptor,
    ROOM_EVENT_PUBLISH_FAILED.descriptor,
    PRESENCE_REFUSED_ROOMS.descriptor,
    PRESENCE_RECOVERY_PASSES.descriptor,
    WORLD_PRESENCE_DROPPED.descriptor,
    WORLD_PRESENCE_SESSIONS.descriptor,
    WORLD_PRESENCE_WALLETS.descriptor,
    ISLAND_CLEANUP_UNCERTAIN.descriptor,
];
