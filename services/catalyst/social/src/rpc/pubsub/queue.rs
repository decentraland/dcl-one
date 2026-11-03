//! The source emitter queue is per subscription and event type, exactly 1,000
//! pending updates. RPC drops oldest; JSON terminates on the first overflow.
use super::{PubSub, SocialEvent};
use crate::rpc::proto::v2::*;
use parking_lot::Mutex;
use std::{
    any::TypeId,
    collections::VecDeque,
    sync::{Arc, Weak},
};
use tokio::sync::{Notify, broadcast::error::RecvError};

pub(crate) const CAPACITY: usize = 1000;
#[derive(Clone, Copy, Debug)]
pub(crate) enum StreamKind {
    All,
    Friendship,
    FriendConnectivity,
    Block,
    PrivateVoice,
    CommunityVoice,
    CommunityMember,
}
impl StreamKind {
    pub(crate) fn message<T: 'static>() -> Self {
        let ty = TypeId::of::<T>();
        if ty == TypeId::of::<FriendshipUpdate>() {
            Self::Friendship
        } else if ty == TypeId::of::<FriendConnectivityUpdate>() {
            Self::FriendConnectivity
        } else if ty == TypeId::of::<BlockUpdate>() {
            Self::Block
        } else if ty == TypeId::of::<PrivateVoiceChatUpdate>() {
            Self::PrivateVoice
        } else if ty == TypeId::of::<CommunityVoiceChatUpdate>() {
            Self::CommunityVoice
        } else if ty == TypeId::of::<CommunityMemberConnectivityUpdate>() {
            Self::CommunityMember
        } else {
            Self::All
        }
    }
    fn matches(self, event: &SocialEvent) -> bool {
        match self {
            Self::All => true,
            Self::Friendship => matches!(event, SocialEvent::Friendship(_)),
            Self::Block => matches!(event, SocialEvent::Block(_)),
            Self::PrivateVoice => matches!(event, SocialEvent::PrivateVoice(_)),
            Self::CommunityVoice => matches!(event, SocialEvent::CommunityVoice(_)),
            Self::FriendConnectivity => matches!(
                event,
                SocialEvent::FriendConnectivity(_)
                    | SocialEvent::TopologyFriendConnectivity(..)
                    | SocialEvent::ConnectivityResync
            ),
            Self::CommunityMember => matches!(
                event,
                SocialEvent::CommunityMember(_)
                    | SocialEvent::CommunityMemberStatus(_)
                    | SocialEvent::TopologyCommunityMember(..)
                    | SocialEvent::ConnectivityResync
            ),
        }
    }
}
#[derive(Clone)]
pub(crate) enum OverflowPolicy {
    DropOldest,
    Close {
        socket: Arc<crate::shared::rpc::ws::SocketClose>,
        code: u16,
        reason: &'static str,
    },
}
pub(super) type Registry = dashmap::DashMap<String, Mutex<Vec<Weak<Queue>>>>;
struct Pending {
    values: VecDeque<Arc<SocialEvent>>,
    overflow: bool,
    closed: bool,
}
pub(super) struct Queue {
    kind: StreamKind,
    policy: OverflowPolicy,
    pending: Mutex<Pending>,
    changed: Notify,
}
impl Queue {
    pub(super) fn push(&self, event: &Arc<SocialEvent>) {
        if !self.kind.matches(event) {
            return;
        }
        let mut pending = self.pending.lock();
        if pending.closed || pending.overflow {
            return;
        }
        if pending.values.len() == CAPACITY {
            metrics::counter!("social_subscription_queue_overflow").increment(1);
            match &self.policy {
                OverflowPolicy::DropOldest => {
                    pending.values.pop_front();
                }
                OverflowPolicy::Close {
                    socket,
                    code,
                    reason,
                } => {
                    pending.overflow = true;
                    pending.values.clear();
                    metrics::counter!("stream_ws_errors", "type" => "subscription_overflow")
                        .increment(1);
                    socket.end(*code, reason);
                    self.changed.notify_one();
                    return;
                }
            }
        }
        pending.values.push_back(event.clone());
        self.changed.notify_one();
    }
}
pub(crate) struct StreamReceiver {
    queue: Arc<Queue>,
    registry: Weak<Registry>,
    address: String,
}
impl StreamReceiver {
    #[cfg(test)]
    pub(crate) fn len(&self) -> usize {
        self.queue.pending.lock().values.len()
    }
    pub(crate) async fn recv(&mut self) -> Result<Arc<SocialEvent>, RecvError> {
        loop {
            let changed = self.queue.changed.notified();
            {
                let mut pending = self.queue.pending.lock();
                if pending.overflow {
                    return Err(RecvError::Lagged(1));
                }
                if let Some(value) = pending.values.pop_front() {
                    return Ok(value);
                }
                if pending.closed {
                    return Err(RecvError::Closed);
                }
            }
            changed.await;
        }
    }
}
impl Drop for StreamReceiver {
    fn drop(&mut self) {
        let mut pending = self.queue.pending.lock();
        pending.closed = true;
        pending.values.clear();
        drop(pending);
        let Some(registry) = self.registry.upgrade() else {
            return;
        };
        if let Some(queues) = registry.get(&self.address) {
            queues.lock().retain(|queue| {
                queue.strong_count() > 0 && !queue.ptr_eq(&Arc::downgrade(&self.queue))
            });
        }
        registry.remove_if(&self.address, |_, queues| queues.lock().is_empty());
    }
}
impl PubSub {
    pub(crate) fn subscribe_updates<T: 'static>(&self, address: &str) -> StreamReceiver {
        self.subscribe_stream(
            address,
            StreamKind::message::<T>(),
            OverflowPolicy::DropOldest,
        )
    }
    pub(crate) fn subscribe_stream(
        &self,
        address: &str,
        kind: StreamKind,
        policy: OverflowPolicy,
    ) -> StreamReceiver {
        let address = address.to_lowercase();
        let queue = Arc::new(Queue {
            kind,
            policy,
            pending: Mutex::new(Pending {
                values: VecDeque::new(),
                overflow: false,
                closed: false,
            }),
            changed: Notify::new(),
        });
        self.streams
            .entry(address.clone())
            .or_default()
            .lock()
            .push(Arc::downgrade(&queue));
        StreamReceiver {
            queue,
            registry: Arc::downgrade(&self.streams),
            address,
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn event(n: usize) -> SocialEvent {
        SocialEvent::Block(BlockUpdate {
            address: n.to_string(),
            ..Default::default()
        })
    }
    #[tokio::test]
    async fn rpc_retains_exact_latest_thousand_and_drops_its_registration() {
        let bus = PubSub::new();
        let mut rx = bus.subscribe_updates::<BlockUpdate>("me");
        for n in 0..1100 {
            bus.publish_local("me", event(n));
        }
        assert_eq!(rx.len(), CAPACITY);
        for n in 100..1100 {
            assert!(
                matches!(&*rx.recv().await.unwrap(),SocialEvent::Block(u) if u.address==n.to_string())
            );
        }
        drop(rx);
        assert!(bus.streams.is_empty());
    }
    #[tokio::test]
    async fn json_overflows_only_its_own_event_type_at_the_exact_source_bound() {
        let bus = PubSub::new();
        let close = Arc::new(crate::shared::rpc::ws::SocketClose::default());
        let mut rx = bus.subscribe_stream(
            "me",
            StreamKind::Block,
            OverflowPolicy::Close {
                socket: close.clone(),
                code: 1008,
                reason: "overflow",
            },
        );
        for _ in 0..10_000 {
            bus.publish_local(
                "me",
                SocialEvent::PrivateVoice(PrivateVoiceChatUpdate::default()),
            );
        }
        assert_eq!(rx.len(), 0);
        for n in 0..1000 {
            bus.publish_local("me", event(n));
        }
        assert_eq!(rx.len(), 1000);
        bus.publish_local("me", event(1000));
        assert!(matches!(rx.recv().await, Err(RecvError::Lagged(1))));
        assert!(close.is_ended());
        drop(rx);
        assert!(bus.streams.is_empty());
    }
}
