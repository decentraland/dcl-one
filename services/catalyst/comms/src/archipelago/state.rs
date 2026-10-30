use crate::archipelago::auth::ChallengeStore;
use crate::archipelago::ban::BanChecker;
use crate::archipelago::config::Settings;
use crate::archipelago::content::ContentResolver;
use crate::archipelago::control_v4::AssignmentAuthority;
use crate::archipelago::feed::{FeedCache, Outbox};
use crate::archipelago::peers::PeerDirectory;
use crate::archipelago::registry::PeersRegistry;
use crate::deny_list::DenyList;
use std::sync::Arc;

pub struct AppStateInner {
    pub settings: Settings,
    pub sockets: Arc<tokio::sync::Semaphore>,
    pub restart: tokio_util::sync::CancellationToken,
    pub peers: Arc<PeerDirectory>,
    pub registry: Arc<PeersRegistry>,
    pub feed: Arc<FeedCache>,
    pub outbox: Outbox,
    /// The gatekeeper this connector runs beside: its NATS link (`bus`) carries the feed, and its
    /// LiveKit removes a kicked wallet's participant.
    pub gatekeeper: crate::gatekeeper::SharedState,
    pub challenges: Arc<ChallengeStore>,

    pub content: Arc<ContentResolver>,
    pub control_v4: Arc<AssignmentAuthority>,
    pub replica_id: String,

    pub ban_checker: Arc<BanChecker>,
    pub deny_list: Arc<DenyList>,
}

pub type AppState = Arc<AppStateInner>;
