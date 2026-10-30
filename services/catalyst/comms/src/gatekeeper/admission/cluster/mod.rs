//! The island half of comms: Pulse's cluster feed (`peer.*.cluster_change`)
//! and the `/ws` connector's session announcements turned into LiveKit island
//! credentials. Pulse (`comms::pulse`, its own process) derives the clusters
//! and publishes one change per peer on the NATS broker; archipelago's sockets
//! (`comms::archipelago`, in this process) hand the island to the explorer; in
//! between, this module gates the wallet on the platform ban store, evicts a
//! displaced session, mints the island token and delivers the assignment to
//! the connector's sockets by call (`subscriber`, through `sockets`). The
//! connector announces each session the same way, by call
//! ([`CommsState::announce_session`](crate::gatekeeper::CommsState::announce_session)),
//! and the subscriber asks Pulse for its current cluster
//! (`peer.*.cluster_lookup`). A wallet a configured legacy socket
//! owns goes through the realm-lane fence instead (`fence`, over the
//! `social.archipelago_v4_*` rows `comms::archipelago::control_v4` claims):
//! the token carries the fence and the assignment is committed to the row
//! the legacy socket reads. `/island-refresh` (`gatekeeper::admission::island_refresh`)
//! re-mints the fenced credential of the current row.
//!
//! `serve` starts it in the primary only, the one process that holds the
//! connector's sockets, so it takes the feed on one unqueued subscription. It
//! runs over the gatekeeper's NATS link (`gatekeeper::bus`); without
//! `NATS_URL` it subscribes to nothing. With
//! `ARCHIPELAGO_CONTROL_V4_AUDIENCE` set a broker is required (protected sockets
//! would otherwise never receive an island), and `serve` refuses to start
//! without `NATS_URL`.

pub mod fence;
pub mod gateway;
pub mod metrics;
pub mod peer_state;
pub mod safety;
pub mod sockets;
pub mod subscriber;

use std::sync::Arc;

use sqlx::PgPool;

/// Short on purpose: the client uses the string within a second of receiving it, and the
/// self-hosted takeover quarantine must wait out this lifetime plus LiveKit's verifier leeway.
pub const DEFAULT_ISLAND_TOKEN_TTL_SECONDS: u64 = 60;
/// Keeps the maximum same-room quarantine at 122 seconds, within the wallet-work deadline.
pub const MAX_ISLAND_TOKEN_TTL_SECONDS: u64 = 60;
pub const DEFAULT_TAKEOVER_RETRY_DELAY_MS: u64 = 100;
/// Long enough for a mint and the lookups around it to finish, short enough that one request with
/// no timeout of its own cannot hold shutdown open.
pub const DEFAULT_DRAIN_TIMEOUT_MS: u64 = 5_000;

#[derive(Debug, Clone)]
pub struct ClusterConfig {
    /// `NATS_URL`; `None` leaves the whole feed inert: nothing connects and nothing subscribes.
    pub nats_url: Option<String>,
    /// `CLUSTER_TAKEOVER_RETRY_DELAY_MS`: base delay between displaced-session removal attempts;
    /// 0 is a real value (retry with no sleep).
    pub takeover_retry_delay_ms: u64,
    /// `CLUSTER_ISLAND_TOKEN_TTL_SECONDS`, capped at [`MAX_ISLAND_TOKEN_TTL_SECONDS`].
    pub island_token_ttl_seconds: u64,
    /// `CLUSTER_SELF_HOSTED_TOKEN_QUARANTINE`: wait out old island JWTs when a same-room owner is
    /// replaced. Disable only with LiveKit Cloud's cutoff-based token revocation.
    pub self_hosted_token_quarantine: bool,
    /// `CLUSTER_DRAIN_TIMEOUT_MS`: graceful shutdown budget; 0 cancels immediately.
    pub drain_timeout_ms: u64,
    pub peer_state_max: usize,
    pub peer_state_ttl_ms: u64,
    /// `ARCHIPELAGO_CONTROL_V4_AUDIENCE`: the realm-lane fence's audience (the old
    /// `COMMS_CONTROL_V4_AUDIENCE`, one value now that comms and archipelago share a process).
    pub control_v4_audience: Option<String>,
}

impl Default for ClusterConfig {
    fn default() -> Self {
        Self {
            nats_url: None,
            takeover_retry_delay_ms: DEFAULT_TAKEOVER_RETRY_DELAY_MS,
            island_token_ttl_seconds: DEFAULT_ISLAND_TOKEN_TTL_SECONDS,
            self_hosted_token_quarantine: true,
            drain_timeout_ms: DEFAULT_DRAIN_TIMEOUT_MS,
            peer_state_max: peer_state::DEFAULT_PEER_STATE_MAX,
            peer_state_ttl_ms: peer_state::DEFAULT_PEER_STATE_TTL_MS,
            control_v4_audience: None,
        }
    }
}

impl ClusterConfig {
    /// Sizing knobs whose zero would disable what they size fall back to the default; the token
    /// lifetime is capped.
    pub fn normalized(mut self) -> Self {
        let d = Self::default();
        if self.island_token_ttl_seconds == 0 {
            self.island_token_ttl_seconds = d.island_token_ttl_seconds;
        }
        self.island_token_ttl_seconds = self
            .island_token_ttl_seconds
            .min(MAX_ISLAND_TOKEN_TTL_SECONDS);
        if self.peer_state_max == 0 {
            self.peer_state_max = d.peer_state_max;
        }
        if self.peer_state_ttl_ms == 0 {
            self.peer_state_ttl_ms = d.peer_state_ttl_ms;
        }
        self.nats_url = self
            .nats_url
            .map(|u| u.trim().to_string())
            .filter(|u| !u.is_empty());
        self.control_v4_audience = self
            .control_v4_audience
            .map(|a| a.trim().to_string())
            .filter(|a| !a.is_empty());
        self
    }

    /// A v4 audience (protected mode) needs a broker: without one the v4 sockets would never
    /// receive an island and nothing would say why.
    pub fn validate(&self) -> anyhow::Result<()> {
        if self.control_v4_audience.is_some() && self.nats_url.is_none() {
            anyhow::bail!("ARCHIPELAGO_CONTROL_V4_AUDIENCE requires NATS_URL");
        }
        Ok(())
    }
}

/// The subscriber and the realm-lane fence of one process.
pub struct ClusterRuntime {
    pub subscriber: Arc<subscriber::ClusterSubscriber>,
    pub fence: Option<Arc<fence::PgAssignmentFence>>,
}

/// The realm-lane fence of a (normalized) config: `None` without a v4 audience.
pub fn fence_for(
    pool: PgPool,
    config: &ClusterConfig,
) -> anyhow::Result<Option<Arc<fence::PgAssignmentFence>>> {
    config.validate()?;
    match config.control_v4_audience.clone() {
        Some(audience) => Ok(Some(Arc::new(
            fence::PgAssignmentFence::new(pool, audience)
                .map_err(|e| anyhow::anyhow!("ARCHIPELAGO_CONTROL_V4_AUDIENCE: {e}"))?,
        ))),
        None => Ok(None),
    }
}

impl ClusterRuntime {
    /// The subscriber over `bus` and `gateway` (the ban store and the LiveKit API it mints and
    /// checks rooms through).
    pub(crate) fn with_gateway(
        bus: Arc<dyn crate::gatekeeper::bus::NatsBus>,
        gateway: Arc<dyn subscriber::ClusterGateway>,
        config: ClusterConfig,
        fence: Option<Arc<fence::PgAssignmentFence>>,
    ) -> Self {
        let peers = Arc::new(peer_state::ClusterPeerState::new(
            config.peer_state_max,
            config.peer_state_ttl_ms,
        ));
        let subscriber = subscriber::ClusterSubscriber::new_with_assignment_fence(
            bus,
            gateway,
            peers,
            config,
            fence.clone().map(|f| f as Arc<dyn fence::AssignmentFence>),
        );
        Self { subscriber, fence }
    }

    /// Starts the subscriber and a job that drains it when `serve` shuts down. Attach the
    /// connector's sockets first ([`subscriber::ClusterSubscriber::attach_sockets`]).
    pub fn start(&self, jobs: &crate::jobs::Jobs) {
        self.subscriber.start();
        let subscriber = self.subscriber.clone();
        jobs.spawn("comms-cluster-subscriber", move |ctx| async move {
            loop {
                tokio::select! {
                    _=ctx.cancel.cancelled()=>break,
                    _=tokio::time::sleep(std::time::Duration::from_secs(1))=>subscriber.replay_due().await,
                }
            }
            subscriber.stop().await;
            Ok(())
        });
    }
}

#[cfg(test)]
mod config_tests {
    use super::*;

    #[test]
    fn the_legacy_mode_keeps_the_inert_defaults() {
        ClusterConfig::default().validate().unwrap();
    }

    #[test]
    fn a_v4_audience_requires_a_broker() {
        let mut config = ClusterConfig {
            control_v4_audience: Some("catalyst.test".into()),
            ..ClusterConfig::default()
        };
        assert_eq!(
            config.validate().unwrap_err().to_string(),
            "ARCHIPELAGO_CONTROL_V4_AUDIENCE requires NATS_URL"
        );
        config.nats_url = Some("nats://127.0.0.1:4222".into());
        config.validate().unwrap();
    }

    #[test]
    fn zero_sizes_fall_back_and_the_token_lifetime_is_capped() {
        let config = ClusterConfig {
            island_token_ttl_seconds: 600,
            peer_state_max: 0,
            nats_url: Some(" ".into()),
            ..ClusterConfig::default()
        }
        .normalized();
        assert_eq!(
            config.island_token_ttl_seconds,
            MAX_ISLAND_TOKEN_TTL_SECONDS
        );
        assert_eq!(config.peer_state_max, peer_state::DEFAULT_PEER_STATE_MAX);
        assert_eq!(config.nats_url, None);
    }
}
