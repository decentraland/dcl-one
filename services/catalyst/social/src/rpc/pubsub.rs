//! Local social subscription fanout. Mutation hints cross the existing core-NATS link;
//! every replica independently derives Pulse connectivity and never republishes it.

use crate::rpc::proto::v2::{
    BlockUpdate, CommunityMemberConnectivityUpdate, CommunityVoiceChatUpdate,
    FriendConnectivityUpdate, FriendshipUpdate, PrivateVoiceChatUpdate,
};
use crate::rpc::proto::v2::{CommunityVoiceChatStatus, ConnectivityStatus, User};

/// The ENDED update (upstream's end and polling): no places, name or image.
pub fn community_voice_ended(community: Uuid) -> CommunityVoiceChatUpdate {
    let now = chrono::Utc::now().timestamp_millis();
    CommunityVoiceChatUpdate {
        stream_closed: None,
        community_id: community.to_string(),
        created_at: now,
        status: CommunityVoiceChatStatus::CommunityVoiceChatEnded as i32,
        ended_at: Some(now),
        positions: Vec::new(),
        is_member: false,
        community_name: String::new(),
        community_image: None,
        worlds: Vec::new(),
    }
}
use dashmap::DashMap;
use sqlx::PgPool;
use std::sync::Arc;
use tokio::sync::broadcast;
use uuid::Uuid;

/// The members of `community` among `addresses` (upstream
/// `getOnlineMembersFromCommunity` over the local subscribers): one
/// primary-key probe per address.
async fn subscribed_members(
    pool: &PgPool,
    community: Uuid,
    addresses: &[String],
) -> sqlx::Result<Vec<String>> {
    if addresses.is_empty() {
        return Ok(Vec::new());
    }
    sqlx::query_scalar(
        "SELECT member_address FROM social.community_members \
         WHERE community_id = $1 AND member_address = ANY($2)",
    )
    .bind(community)
    .bind(addresses)
    .fetch_all(pool)
    .await
}

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
    /// A membership change (join ONLINE; leave, kick, ban OFFLINE) or a
    /// deletion (each member OFFLINE): its recipients were chosen when it was
    /// published, so it is delivered as published, with no presence or
    /// co-member check (upstream `communityMemberStatusHandler`,
    /// `communityDeletedUpdateHandler`).
    CommunityMemberStatus(CommunityMemberConnectivityUpdate),
}

#[derive(Clone, Default)]
pub struct PubSub {
    channels: Arc<DashMap<String, broadcast::Sender<Arc<SocialEvent>>>>,
    outbound: Arc<std::sync::OnceLock<tokio::sync::mpsc::Sender<super::fanout::Event>>>,
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
        self.publish_local(address, event.clone());
        if let Some(event) = super::fanout::Event::target(address, &event) {
            self.forward(event);
        }
    }

    pub(crate) fn attach_outbound(
        &self,
        outbound: tokio::sync::mpsc::Sender<super::fanout::Event>,
    ) {
        assert!(
            self.outbound.set(outbound).is_ok(),
            "social fanout attached once"
        );
    }

    pub(crate) fn forward(&self, event: super::fanout::Event) {
        if let Some(outbound) = self.outbound.get() {
            if !event.fits_queue() {
                metrics::counter!("social_event_delivery_dropped", "reason" => "oversized_outbound").increment(1);
                return;
            }
            if outbound.try_send(event).is_err() {
                metrics::counter!("social_event_delivery_dropped", "reason" => "outbound_queue")
                    .increment(1);
            }
        }
    }

    pub(crate) async fn forward_community_voice(
        &self,
        pool: &sqlx::PgPool,
        scope: super::fanout::Scope,
        exclude: Option<&str>,
        update: &CommunityVoiceChatUpdate,
    ) {
        if self.outbound.get().is_none() {
            return;
        }
        let Some(event) = super::fanout::Event::community_voice(scope, exclude, update) else {
            metrics::counter!("social_event_delivery_dropped", "reason" => "invalid_outbound")
                .increment(1);
            return;
        };
        if event.fits_queue() {
            self.forward(event);
            return;
        }
        let Some(bytes) = event.encode_stored() else {
            metrics::counter!("social_event_delivery_dropped", "reason" => "invalid_outbound")
                .increment(1);
            return;
        };
        match crate::rpc::db::Db::new(pool.clone())
            .store_social_event_payload(bytes)
            .await
        {
            Ok(event_id) => self.forward(super::fanout::Event::Reference { event_id }),
            Err(error) => {
                tracing::warn!(%error, "social event overflow storage failed");
                metrics::counter!("social_event_delivery_dropped", "reason" => "payload_storage")
                    .increment(1);
            }
        }
    }

    pub(crate) fn publish_local(&self, address: &str, event: SocialEvent) {
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

    /// The addresses holding a subscription on this process (upstream
    /// `getLocalSubscribersAddresses`).
    pub fn subscribed_addresses(&self) -> Vec<String> {
        self.channels
            .iter()
            .filter(|channel| channel.value().receiver_count() > 0)
            .map(|channel| channel.key().clone())
            .collect()
    }

    pub async fn community_member_status(
        &self,
        pool: &PgPool,
        community: Uuid,
        member: &str,
        status: ConnectivityStatus,
    ) {
        self.deliver_community_member(pool, community, member, status)
            .await;
        self.forward(super::fanout::Event::CommunityMember {
            community,
            actor: member.to_lowercase(),
            status: status as i32,
        });
    }
    pub(crate) async fn deliver_community_member(
        &self,
        pool: &PgPool,
        community: Uuid,
        member: &str,
        status: ConnectivityStatus,
    ) {
        let member = member.to_lowercase();
        let update = CommunityMemberConnectivityUpdate {
            stream_closed: None,
            community_id: community.to_string(),
            member: Some(User {
                address: member.clone(),
            }),
            status: status as i32,
        };
        let others: Vec<_> = self
            .subscribed_addresses()
            .into_iter()
            .filter(|address| *address != member)
            .collect();
        for page in others.chunks(256) {
            match subscribed_members(pool, community, page).await {
                Ok(members) => {
                    for recipient in members {
                        self.publish_local(
                            &recipient,
                            SocialEvent::CommunityMemberStatus(update.clone()),
                        );
                    }
                }
                Err(error) => {
                    tracing::warn!(%error, %community, "community member status read failed")
                }
            }
        }
        self.publish_local(&member, SocialEvent::CommunityMemberStatus(update));
    }
    pub async fn community_deleted(&self, pool: &PgPool, community: Uuid) {
        self.deliver_community_deleted(pool, community).await;
        self.forward(super::fanout::Event::CommunityDeleted { community });
    }
    pub(crate) async fn deliver_community_deleted(&self, pool: &PgPool, community: Uuid) {
        for page in self.subscribed_addresses().chunks(256) {
            match subscribed_members(pool, community, page).await {
                Ok(members) => {
                    for member in members {
                        self.publish_local(
                            &member,
                            SocialEvent::CommunityMemberStatus(CommunityMemberConnectivityUpdate {
                                stream_closed: None,
                                community_id: community.to_string(),
                                member: Some(User {
                                    address: member.clone(),
                                }),
                                status: ConnectivityStatus::Offline as i32,
                            }),
                        );
                    }
                }
                Err(error) => tracing::warn!(%error, %community, "community deleted read failed"),
            }
        }
    }

    pub async fn community_voice(
        &self,
        pool: &PgPool,
        _community: Uuid,
        update: CommunityVoiceChatUpdate,
        everyone: bool,
        exclude: Option<&str>,
    ) {
        let scope = if everyone {
            super::fanout::Scope::All
        } else {
            super::fanout::Scope::Members
        };
        self.deliver_community_voice(pool, scope, exclude, update.clone())
            .await;
        self.forward_community_voice(pool, scope, exclude, &update)
            .await;
    }
    pub(crate) async fn deliver_community_voice(
        &self,
        pool: &PgPool,
        scope: super::fanout::Scope,
        exclude: Option<&str>,
        update: CommunityVoiceChatUpdate,
    ) {
        let Ok(community) = update.community_id.parse() else {
            return;
        };
        let ended = update.status == CommunityVoiceChatStatus::CommunityVoiceChatEnded as i32;
        let scope = if ended {
            scope
        } else {
            match crate::rpc::db::Db::new(pool.clone())
                .community_voice_scope(community)
                .await
            {
                Ok(Some(scope)) => scope,
                _ => return,
            }
        };
        let everyone = scope == super::fanout::Scope::All;
        for page in self.subscribed_addresses().chunks(256) {
            let members: std::collections::HashSet<String> = if everyone && ended {
                Default::default()
            } else {
                match subscribed_members(pool, community, page).await {
                    Ok(members) => members.into_iter().collect(),
                    Err(error) => {
                        tracing::warn!(%error, %community, "community voice member read failed");
                        if !everyone {
                            continue;
                        }
                        Default::default()
                    }
                }
            };
            for recipient in page {
                if exclude.is_some_and(|excluded| excluded.eq_ignore_ascii_case(recipient)) {
                    continue;
                }
                let is_member = members.contains(recipient);
                if everyone || is_member {
                    self.publish_local(
                        recipient,
                        SocialEvent::CommunityVoice(CommunityVoiceChatUpdate {
                            is_member,
                            ..update.clone()
                        }),
                    );
                }
            }
        }
    }

    /// Runtime closure uses the shared generation, so a different serve or a restart can
    /// notify the recorded audience. Compare-and-clear prevents duplicate cleanup hints.
    pub async fn community_voice_closed(&self, pool: &PgPool, community: Uuid) {
        let db = crate::rpc::db::Db::new(pool.clone());
        match db.voice_notification(community).await {
            Ok(Some(entry)) => {
                if db.clear_voice_notification(&entry).await.unwrap_or(false)
                    && entry.fresh
                    && db
                        .voice_generation_is(community, None)
                        .await
                        .unwrap_or(false)
                {
                    self.community_voice(
                        pool,
                        community,
                        community_voice_ended(community),
                        entry.scope == super::fanout::Scope::All,
                        None,
                    )
                    .await;
                }
            }
            Ok(None) => {}
            Err(error) => tracing::warn!(%error, "community voice close scope read failed"),
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
                stream_closed: None,
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
