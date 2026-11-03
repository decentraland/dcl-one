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
    /// The audience each live community voice room was started for, `true`
    /// for every subscriber (a public, listed community), `false` for its
    /// members: upstream's `communityVoiceChatCache` `notificationScope`,
    /// which ENDED reuses so a later privacy change neither widens nor
    /// narrows it.
    voice_audiences: Arc<DashMap<Uuid, bool>>,
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

    /// The addresses holding a subscription on this process (upstream
    /// `getLocalSubscribersAddresses`).
    pub fn subscribed_addresses(&self) -> Vec<String> {
        self.channels
            .iter()
            .filter(|channel| channel.value().receiver_count() > 0)
            .map(|channel| channel.key().clone())
            .collect()
    }

    /// A membership change of `member` in `community`: the community's
    /// members subscribed here but `member`, then `member` itself (upstream
    /// `communityMemberStatusHandler`, `src/logic/updates.ts`). A join is
    /// ONLINE; a leave, kick or ban OFFLINE, published once the member row
    /// is gone, so the departed wallet is told as the affected member.
    pub async fn community_member_status(
        &self,
        pool: &PgPool,
        community: Uuid,
        member: &str,
        status: ConnectivityStatus,
    ) {
        let member = member.to_lowercase();
        let others: Vec<String> = self
            .subscribed_addresses()
            .into_iter()
            .filter(|address| *address != member)
            .collect();
        let update = CommunityMemberConnectivityUpdate {
            stream_closed: None,
            community_id: community.to_string(),
            member: Some(User {
                address: member.clone(),
            }),
            status: status as i32,
        };
        match subscribed_members(pool, community, &others).await {
            Ok(members) => {
                for recipient in members {
                    self.publish(
                        &recipient,
                        SocialEvent::CommunityMemberStatus(update.clone()),
                    );
                }
            }
            Err(error) => {
                tracing::warn!(%error, %community, "community member status: member read failed");
            }
        }
        self.publish(&member, SocialEvent::CommunityMemberStatus(update));
    }

    /// A deleted community: each of its members subscribed here hears
    /// itself OFFLINE in it (upstream `communityDeletedUpdateHandler`).
    pub async fn community_deleted(&self, pool: &PgPool, community: Uuid) {
        let subscribed = self.subscribed_addresses();
        match subscribed_members(pool, community, &subscribed).await {
            Ok(members) => {
                for member in members {
                    let update = CommunityMemberConnectivityUpdate {
                        stream_closed: None,
                        community_id: community.to_string(),
                        member: Some(User {
                            address: member.clone(),
                        }),
                        status: ConnectivityStatus::Offline as i32,
                    };
                    self.publish(&member, SocialEvent::CommunityMemberStatus(update));
                }
            }
            Err(error) => {
                tracing::warn!(%error, %community, "community deleted update: member read failed");
            }
        }
    }

    /// Records the audience of a community voice room that just started.
    pub fn record_voice_audience(&self, community: Uuid, everyone: bool) {
        self.voice_audiences.insert(community, everyone);
    }

    /// Takes the audience recorded at start; `None` when no start on this
    /// process recorded one.
    pub fn take_voice_audience(&self, community: Uuid) -> Option<bool> {
        self.voice_audiences
            .remove(&community)
            .map(|(_, everyone)| everyone)
    }

    /// A community voice update (upstream `communityVoiceChatUpdateHandler`,
    /// `src/logic/updates.ts`): `everyone` reaches every subscriber here,
    /// otherwise the community's members among them; `exclude` (the starter)
    /// never. `is_member` is set per recipient, except on an ENDED for
    /// everyone, which reads no membership (every `is_member` false).
    pub async fn community_voice(
        &self,
        pool: &PgPool,
        community: Uuid,
        update: CommunityVoiceChatUpdate,
        everyone: bool,
        exclude: Option<&str>,
    ) {
        let subscribed: Vec<String> = self
            .subscribed_addresses()
            .into_iter()
            .filter(|address| exclude.is_none_or(|e| !e.eq_ignore_ascii_case(address)))
            .collect();
        let ended = update.status == CommunityVoiceChatStatus::CommunityVoiceChatEnded as i32;
        let members: std::collections::HashSet<String> = if everyone && ended {
            Default::default()
        } else {
            match subscribed_members(pool, community, &subscribed).await {
                Ok(members) => members.into_iter().collect(),
                Err(error) => {
                    tracing::warn!(%error, %community, "community voice update: member read failed");
                    // a member-only update cannot be delivered safely without
                    // the read; a public start is announced as to non-members
                    if !everyone {
                        return;
                    }
                    Default::default()
                }
            }
        };
        for recipient in subscribed {
            let is_member = members.contains(&recipient);
            if everyone || is_member {
                self.publish(
                    &recipient,
                    SocialEvent::CommunityVoice(CommunityVoiceChatUpdate {
                        is_member,
                        ..update.clone()
                    }),
                );
            }
        }
    }

    /// A room that closed (ended, expired or finished in the runtime): the
    /// audience its start recorded hears ENDED (upstream's community-voice
    /// polling, `sendEndedNotification`). Nothing when no start recorded
    /// one, or when the end procedure already took it.
    pub async fn community_voice_closed(&self, pool: &PgPool, community: Uuid) {
        if let Some(everyone) = self.take_voice_audience(community) {
            self.community_voice(
                pool,
                community,
                community_voice_ended(community),
                everyone,
                None,
            )
            .await;
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
