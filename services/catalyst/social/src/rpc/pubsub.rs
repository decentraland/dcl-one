//! Per-address fan-out of the subscription streams. One process, so a
//! `tokio::sync::broadcast` channel per online address replaces the Redis
//! pub/sub upstream social-service-ea used between instances: an event
//! published here reaches only the subscriptions held by THIS process. A
//! second `catalyst serve` sharing the database would not see its peers'
//! friendship events. Pulse connectivity is independently derived on each replica;
//! other multi-instance events require an out-of-process
//! fan-out (NATS or PostgreSQL `LISTEN/NOTIFY` carrying the same encoded
//! events), which is the documented multi-instance consequence, not a change
//! in the per-connection contract.

use crate::rpc::proto::v2::{
    BlockUpdate, CommunityMemberConnectivityUpdate, CommunityVoiceChatUpdate,
    FriendConnectivityUpdate, FriendshipUpdate, PrivateVoiceChatUpdate,
};
use dashmap::DashMap;
use std::sync::Arc;
use tokio::sync::broadcast;

/// Events a slow subscriber can fall behind by before it is told it lagged.
pub(crate) const CHANNEL_CAP: usize = 256;

#[derive(Clone)]
pub enum SocialEvent {
    /// Existing stream closure/resubscription, used when a connectivity value exceeds its byte budget.
    ConnectivityResync,
    Friendship(FriendshipUpdate),
    FriendConnectivity(FriendConnectivityUpdate),
    TopologyFriendConnectivity(FriendConnectivityUpdate, u64),
    Block(BlockUpdate),
    PrivateVoice(PrivateVoiceChatUpdate),
    CommunityVoice(CommunityVoiceChatUpdate),
    CommunityMember(CommunityMemberConnectivityUpdate),
    TopologyCommunityMember(CommunityMemberConnectivityUpdate, u64),
}

#[derive(Clone, Default)]
pub struct PubSub {
    channels: Arc<DashMap<String, broadcast::Sender<Arc<SocialEvent>>>>,
}

impl PubSub {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn subscribe(&self, address: &str) -> broadcast::Receiver<Arc<SocialEvent>> {
        let sender = self
            .channels
            .entry(address.to_lowercase())
            .or_insert_with(|| broadcast::channel(CHANNEL_CAP).0)
            .clone();
        sender.subscribe()
    }

    /// A publish to an address nobody subscribed is dropped; the channel of an
    /// address whose last subscriber went away is removed so the map tracks
    /// the online set rather than everyone ever addressed.
    pub fn publish(&self, address: &str, event: SocialEvent) {
        let key = address.to_lowercase();
        let Some(sender) = self.channels.get(&key) else {
            return;
        };
        if sender.send(Arc::new(event)).is_err() {
            drop(sender);
            self.channels
                .remove_if(&key, |_, s| s.receiver_count() == 0);
        }
    }

    /// The live subscriptions of `address`: a stream's server half subscribes
    /// only once the client acknowledged its opening, after the client's call
    /// returned, so a test waits on this before publishing.
    #[cfg(test)]
    pub fn subscriber_count(&self, address: &str) -> usize {
        self.channels
            .get(&address.to_lowercase())
            .map_or(0, |sender| sender.receiver_count())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn publish_is_shared_by_every_receiver() {
        let ps = PubSub::new();
        let mut rxs: Vec<_> = (0..6).map(|_| ps.subscribe("0xabc")).collect();
        ps.publish(
            "0xABC",
            SocialEvent::CommunityMember(CommunityMemberConnectivityUpdate {
                community_id: "c".into(),
                member: None,
                status: 0,
            }),
        );

        let got: Vec<_> = rxs.iter_mut().map(|rx| rx.try_recv().unwrap()).collect();

        assert_eq!(got.len(), 6);
        assert!(
            got.iter().all(|e| Arc::ptr_eq(e, &got[0])),
            "receivers did not share one Arc allocation"
        );
    }

    /// Upstream's "separate subscribers for different addresses": an event
    /// for one address never reaches another's receiver.
    #[test]
    fn each_address_has_its_own_channel() {
        let ps = PubSub::new();
        let mut a = ps.subscribe("0xaaa");
        let mut b = ps.subscribe("0xbbb");
        ps.publish("0xAAA", SocialEvent::Block(BlockUpdate::default()));
        assert!(a.try_recv().is_ok());
        assert!(b.try_recv().is_err());
        assert_eq!(ps.channels.len(), 2);
    }

    #[test]
    fn a_channel_without_subscribers_is_forgotten_on_the_next_publish() {
        let ps = PubSub::new();
        let rx = ps.subscribe("0xabc");
        assert_eq!(ps.channels.len(), 1);
        drop(rx);
        ps.publish("0xabc", SocialEvent::Block(BlockUpdate::default()));
        assert!(ps.channels.is_empty());
        ps.publish("0xnobody", SocialEvent::Block(BlockUpdate::default()));
        assert!(ps.channels.is_empty());
    }
}
