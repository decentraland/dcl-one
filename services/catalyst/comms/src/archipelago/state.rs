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
    pub sockets_per_ip: Arc<IpSlots>,
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

/// Sockets held per client source (an IPv4 address or an IPv6 /64); a request
/// that cannot be attributed to one client is not counted.
pub struct IpSlots {
    max: usize,
    held: std::sync::Mutex<std::collections::HashMap<String, usize>>,
}

/// One socket's share of its source; released on drop.
pub struct IpSlot {
    slots: Arc<IpSlots>,
    source: String,
}

impl IpSlots {
    pub fn new(max: u32) -> Arc<Self> {
        Arc::new(Self {
            max: max as usize,
            held: Default::default(),
        })
    }

    /// `Err` when the address already holds its limit; `Ok(None)` when it is not counted.
    #[allow(clippy::result_unit_err)]
    pub fn try_acquire(self: &Arc<Self>, client_ip: Option<&str>) -> Result<Option<IpSlot>, ()> {
        let Some(ip) = client_ip.filter(|_| self.max > 0) else {
            return Ok(None);
        };
        let source = crate::shared::client_ip::source_key(ip);
        let mut held = self.held.lock().unwrap_or_else(|e| e.into_inner());
        let count = held.entry(source.clone()).or_default();
        if *count >= self.max {
            return Err(());
        }
        *count += 1;
        Ok(Some(IpSlot {
            slots: Arc::clone(self),
            source,
        }))
    }

    /// Whether this cap counts anything at all.
    pub fn enabled(&self) -> bool {
        self.max > 0
    }
}

impl Drop for IpSlot {
    fn drop(&mut self) {
        let mut held = self.slots.held.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(count) = held.get_mut(&self.source) {
            *count -= 1;
            if *count == 0 {
                held.remove(&self.source);
            }
        }
    }
}

#[cfg(test)]
mod ip_slot_tests {
    use super::IpSlots;

    #[test]
    fn an_address_holds_at_most_its_limit_and_frees_on_drop() {
        let slots = IpSlots::new(2);
        let a = Some("203.0.113.7");
        let b = Some("203.0.113.8");
        let first = slots.try_acquire(a).unwrap();
        let _second = slots.try_acquire(a).unwrap();
        assert!(slots.try_acquire(a).is_err());
        assert!(slots.try_acquire(b).unwrap().is_some());
        assert!(slots.try_acquire(None).unwrap().is_none());
        drop(first);
        assert!(slots.try_acquire(a).unwrap().is_some());
        let unlimited = IpSlots::new(0);
        for _ in 0..4 {
            assert!(unlimited.try_acquire(a).unwrap().is_none());
        }
    }

    #[test]
    fn an_ipv6_client_is_counted_by_its_64() {
        let slots = IpSlots::new(1);
        let _held = slots.try_acquire(Some("2001:db8:1:2::1")).unwrap();
        assert!(slots.try_acquire(Some("2001:db8:1:2::ffff")).is_err());
        assert!(
            slots
                .try_acquire(Some("2001:db8:1:3::1"))
                .unwrap()
                .is_some()
        );
    }
}
