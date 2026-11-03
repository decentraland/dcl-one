//! The state every procedure shares: the queries, the fan-out, the voice
//! runtime, profile enrichment, the connection identities and the presence
//! counters. Socket counters are transport bookkeeping; online state comes from
//! the complete Pulse topology shared by every serving replica.

use crate::comms::gatekeeper::voice::rooms::VoiceRooms;
use crate::rpc::db::Db;
use crate::rpc::limits::{Admission, FriendshipMutationLimiter, Protocol};
use crate::rpc::profiles::Profiles;
use crate::rpc::proto::v2::{
    CommunityMemberConnectivityUpdate, ConnectivityStatus, FriendConnectivityUpdate,
    PrivateVoiceChatStatus, PrivateVoiceChatUpdate, User,
};
use crate::rpc::pubsub::{PubSub, SocialEvent};
use crate::shared::rpc::ws::SocketClose;
use dashmap::DashMap;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, Instant};
use tokio_util::sync::CancellationToken;

pub use crate::rpc::limits::PairScope;

/// Idle cadence of the private-voice expiry DELETE when no call started here is pending.
const PRIVATE_VOICE_SWEEP_FALLBACK_MS: i64 = 30_000;

/// The knobs the procedures read; the process-wide `Settings` carries them
/// plus the socket's own (window, caps, admin token).
#[derive(Clone, Debug)]
pub struct VoiceSettings {
    pub private_voice_chat_expiration_ms: i64,
    pub private_voice_chat_expiration_batch_size: i64,
}

impl Default for VoiceSettings {
    fn default() -> Self {
        Self {
            private_voice_chat_expiration_ms: 60_000,
            private_voice_chat_expiration_batch_size: 20,
        }
    }
}

pub struct ContextInner {
    pub voice_settings: VoiceSettings,
    pub db: Db,
    pub pubsub: PubSub,
    pub voice: Arc<dyn VoiceRooms>,
    pub profiles: Profiles,
    friendship_limiter: FriendshipMutationLimiter,
    admission: Admission,
    identities: DashMap<u32, String>,
    subscriptions: DashMap<(u32, &'static str), Arc<CancellationToken>>,
    presence: DashMap<String, u32>,
    topology: std::sync::OnceLock<Arc<crate::comms::topology::Topology>>,
    /// The communities state, attached in `serve`: a community voice start
    /// reads its places and thumbnail through it.
    communities: std::sync::OnceLock<crate::communities::SharedState>,
    connectivity_operations: tokio::sync::Semaphore,
    kill_handles: DashMap<u32, Arc<SocketClose>>,
    /// Every open socket of both protocols, from the moment it opens
    /// (upstream's `ws-pool`): the pool cap counts them and shutdown ends them.
    sockets: parking_lot::Mutex<HashMap<u64, (Protocol, Arc<SocketClose>)>>,
    next_socket: AtomicU64,
    /// `SOCIAL_WRITES_ENABLED` (`social::writes`): false refuses every
    /// mutating procedure with `SocialError::WritesDisabled`.
    writes_enabled: AtomicBool,
}

/// Resume only after a completed bounded recipient page. A status reversal resets it.
#[derive(Clone, Default)]
pub(crate) struct ConnectivityCursor {
    pub online: Option<bool>,
    pub friend_after: String,
    pub member_after: Option<(uuid::Uuid, String)>,
    pub friends_done: bool,
}

#[derive(Clone)]
pub struct Context(Arc<ContextInner>);

/// Releases only the subscription it acquired, including after transport-id reuse.
pub(crate) struct SubscriptionGuard {
    context: Context,
    key: (u32, &'static str),
    token: Arc<CancellationToken>,
}

impl SubscriptionGuard {
    pub(crate) fn cancellation(&self) -> CancellationToken {
        self.token.as_ref().clone()
    }
}

impl Drop for SubscriptionGuard {
    fn drop(&mut self) {
        self.token.cancel();
        self.context
            .0
            .subscriptions
            .remove_if(&self.key, |_, token| Arc::ptr_eq(token, &self.token));
    }
}

/// An open socket's place in the pool; dropping it frees the place.
pub struct OpenSocket {
    ctx: Context,
    id: u64,
    protocol: Protocol,
    opened: Instant,
    /// How the server ends this socket.
    pub close: Arc<SocketClose>,
}

impl Drop for OpenSocket {
    fn drop(&mut self) {
        let mut sockets = self.ctx.0.sockets.lock();
        sockets.remove(&self.id);
        metrics::gauge!("ws_active_connections").set(sockets.len() as f64);
        metrics::gauge!("ws_active_connections_by_protocol", "protocol" => self.protocol.label())
            .set(
                sockets
                    .values()
                    .filter(|(protocol, _)| *protocol == self.protocol)
                    .count() as f64,
            );
        drop(sockets);
        let protocol = self.protocol.label();
        if matches!(self.protocol, Protocol::Rpc) {
            let code = self.close.observed_code().unwrap_or(1006).to_string();
            metrics::counter!("ws_close_codes", "code" => code).increment(1);
        }
        metrics::histogram!("ws_connection_duration_seconds")
            .record(self.opened.elapsed().as_secs_f64());
        super::metrics::log("ws-pool", "debug");
        metrics::histogram!("ws_connection_duration_seconds_by_protocol", "protocol" => protocol)
            .record(self.opened.elapsed().as_secs_f64());
    }
}

impl Context {
    pub fn new(
        voice_settings: VoiceSettings,
        db: Db,
        profiles: Profiles,
        voice: Arc<dyn VoiceRooms>,
    ) -> Self {
        let friendship_limiter = FriendshipMutationLimiter::new(db.pool().clone());
        Self(Arc::new(ContextInner {
            voice_settings,
            db,
            pubsub: PubSub::new(),
            voice,
            profiles,
            friendship_limiter,
            admission: Admission::default(),
            identities: DashMap::new(),
            subscriptions: DashMap::new(),
            presence: DashMap::new(),
            topology: std::sync::OnceLock::new(),
            communities: std::sync::OnceLock::new(),
            connectivity_operations: tokio::sync::Semaphore::new(4),
            kill_handles: DashMap::new(),
            sockets: parking_lot::Mutex::new(HashMap::new()),
            next_socket: AtomicU64::new(0),
            writes_enabled: AtomicBool::new(true),
        }))
    }

    pub fn set_writes_enabled(&self, enabled: bool) {
        self.0.writes_enabled.store(enabled, Ordering::Relaxed);
    }

    pub fn writes_enabled(&self) -> bool {
        self.0.writes_enabled.load(Ordering::Relaxed)
    }

    pub fn voice_settings(&self) -> &VoiceSettings {
        &self.0.voice_settings
    }
    pub fn db(&self) -> &Db {
        &self.0.db
    }
    pub fn pubsub(&self) -> &PubSub {
        &self.0.pubsub
    }
    pub fn voice(&self) -> &dyn VoiceRooms {
        self.0.voice.as_ref()
    }
    pub fn profiles(&self) -> &Profiles {
        &self.0.profiles
    }

    pub fn friendship_limiter(&self) -> &FriendshipMutationLimiter {
        &self.0.friendship_limiter
    }
    /// The per-address socket cap and subscription bucket of both sockets.
    pub fn admission(&self) -> &Admission {
        &self.0.admission
    }

    /// The identity of a transport is fixed at the handshake; a procedure
    /// call on a transport with none is unauthenticated.
    pub fn register_identity(&self, transport_id: u32, address: String) {
        self.0
            .identities
            .insert(transport_id, address.to_lowercase());
    }

    /// Counts a socket that just opened, before its handshake (upstream
    /// `ws-handler.ts:323`, `wsPool.registerConnection` in `open`); `None`
    /// when `max` sockets are already open, checked and counted at once so
    /// concurrent opens cannot pass a full pool.
    pub fn open_socket(&self, protocol: Protocol, max: Option<usize>) -> Option<OpenSocket> {
        let close = Arc::new(SocketClose::default());
        let mut sockets = self.0.sockets.lock();
        if max.is_some_and(|max| sockets.len() >= max) {
            metrics::counter!("ws_connections_rejected").increment(1);
            return None;
        }
        let id = self.0.next_socket.fetch_add(1, Ordering::Relaxed);
        sockets.insert(id, (protocol, close.clone()));
        metrics::gauge!("ws_active_connections_by_protocol", "protocol" => protocol.label()).set(
            sockets
                .values()
                .filter(|(active, _)| *active == protocol)
                .count() as f64,
        );
        metrics::gauge!("ws_active_connections").set(sockets.len() as f64);
        drop(sockets);
        super::metrics::log("ws-pool", "debug");
        Some(OpenSocket {
            ctx: self.clone(),
            id,
            protocol,
            opened: Instant::now(),
            close,
        })
    }

    pub fn live_connections(&self) -> usize {
        self.0.sockets.lock().len()
    }

    /// Ends every open socket, authenticated or not, with `code` and
    /// `reason`, and waits up to `budget` for them to close.
    pub async fn end_sockets(&self, code: u16, reason: &'static str, budget: Duration) {
        for (_, socket) in self.0.sockets.lock().values() {
            socket.end(code, reason);
        }
        let deadline = Instant::now() + budget;
        while self.live_connections() > 0 && Instant::now() < deadline {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }

    pub fn register_kill_handle(&self, transport_id: u32, kill: Arc<SocketClose>) {
        self.0.kill_handles.insert(transport_id, kill);
    }

    pub fn forget_identity(&self, transport_id: u32) {
        self.0.identities.remove(&transport_id);
        self.0.kill_handles.remove(&transport_id);
        self.0.subscriptions.retain(|(id, _), token| {
            if *id == transport_id {
                token.cancel();
                false
            } else {
                true
            }
        });
    }

    /// One active RPC stream of each kind per connection, across all its ports.
    pub(crate) fn try_subscription(
        &self,
        transport_id: u32,
        procedure: &'static str,
    ) -> Option<SubscriptionGuard> {
        let key = (transport_id, procedure);
        match self.0.subscriptions.entry(key) {
            dashmap::mapref::entry::Entry::Occupied(_) => None,
            dashmap::mapref::entry::Entry::Vacant(entry) => {
                let token = Arc::new(CancellationToken::new());
                entry.insert(token.clone());
                Some(SubscriptionGuard {
                    context: self.clone(),
                    key,
                    token,
                })
            }
        }
    }

    pub fn identity(&self, transport_id: u32) -> Option<String> {
        self.0.identities.get(&transport_id).map(|r| r.clone())
    }

    pub fn presence_snapshot(&self) -> Vec<(String, u32)> {
        self.0
            .presence
            .iter()
            .map(|e| (e.key().clone(), *e.value()))
            .collect()
    }

    pub(crate) fn report_subscriber_metrics(&self) {
        metrics::gauge!("subscribers_local_count").set(self.0.presence.len() as f64);
        metrics::gauge!("subscribers_generators_count")
            .set(self.0.pubsub.active_generators() as f64);
    }

    pub fn online_count(&self) -> usize {
        self.topology().snapshot().wallets.len()
    }

    /// Kills every socket of `address`; returns how many were told.
    pub fn disconnect_address(&self, address: &str) -> usize {
        let addr = address.to_lowercase();
        let ids: Vec<u32> = self
            .0
            .identities
            .iter()
            .filter(|e| *e.value() == addr)
            .map(|e| *e.key())
            .collect();
        let mut kicked = 0;
        for id in ids {
            if let Some(handle) = self.0.kill_handles.get(&id) {
                handle.end(1000, "Disconnected by the operator");
                kicked += 1;
            }
        }
        kicked
    }

    /// True when this socket took the address from offline to online.
    pub fn mark_online(&self, address: &str) -> bool {
        let mut entry = self.0.presence.entry(address.to_lowercase()).or_insert(0);
        *entry += 1;
        *entry == 1
    }

    /// True when this socket was the address's last.
    pub fn mark_offline(&self, address: &str) -> bool {
        let addr = address.to_lowercase();
        let mut became_offline = false;
        if let Some(mut entry) = self.0.presence.get_mut(&addr) {
            if *entry > 0 {
                *entry -= 1;
            }
            became_offline = *entry == 0;
        }
        if became_offline {
            self.0.presence.remove(&addr);
        }
        became_offline
    }

    pub fn topology(&self) -> &Arc<crate::comms::topology::Topology> {
        self.0
            .topology
            .get_or_init(|| Arc::new(crate::comms::topology::Topology::default()))
    }

    /// Attaches the communities state (once).
    pub fn set_communities(&self, communities: crate::communities::SharedState) {
        let _ = self.0.communities.set(communities);
    }

    pub(crate) fn communities(&self) -> Option<&crate::communities::SharedState> {
        self.0.communities.get()
    }

    pub(crate) fn set_topology(&self, topology: Arc<crate::comms::topology::Topology>) {
        assert!(
            self.0.topology.set(topology).is_ok(),
            "topology must be injected before serving"
        );
    }

    pub fn is_online(&self, address: &str) -> bool {
        self.topology().snapshot().is_online(address)
    }

    pub(crate) fn friend_delivery_guard(
        &self,
        recipient: String,
        actor: String,
        online: bool,
        generation: u64,
    ) -> crate::rpc::drpc::stream_protocol::DeliveryGuard {
        self.delivery_guard(recipient, actor, online, generation, None)
    }

    fn delivery_guard(
        &self,
        recipient: String,
        actor: String,
        online: bool,
        generation: u64,
        community: Option<uuid::Uuid>,
    ) -> crate::rpc::drpc::stream_protocol::DeliveryGuard {
        let ctx = self.clone();
        Arc::new(move || {
            let ctx = ctx.clone();
            let recipient = recipient.clone();
            let actor = actor.clone();
            Box::pin(async move {
                let Ok(_permit) = ctx.0.connectivity_operations.acquire().await else {
                    return Err(());
                };
                let allowed = match community {
                    Some(community) => {
                        ctx.db()
                            .connectivity_member_allowed(&actor, &recipient, community)
                            .await
                    }
                    None => {
                        ctx.db()
                            .connectivity_friend_allowed(&actor, &recipient)
                            .await
                    }
                };
                let allowed = allowed.map_err(|_| ())?;
                Ok(allowed
                    && ctx
                        .topology()
                        .queued_status_is_current(generation, &actor, online))
            })
        })
    }

    pub(crate) fn event_delivery_guard(
        &self,
        recipient: &str,
        event: &SocialEvent,
    ) -> Option<crate::rpc::drpc::stream_protocol::DeliveryGuard> {
        match event {
            SocialEvent::TopologyCommunityMember(update, generation) => Some(self.delivery_guard(
                recipient.to_string(),
                update.member.as_ref()?.address.clone(),
                update.status == ConnectivityStatus::Online as i32,
                *generation,
                Some(update.community_id.parse().ok()?),
            )),
            SocialEvent::CommunityMember(update) => Some(self.delivery_guard(
                recipient.to_string(),
                update.member.as_ref()?.address.clone(),
                update.status == ConnectivityStatus::Online as i32,
                self.topology().snapshot().generation,
                Some(update.community_id.parse().ok()?),
            )),
            _ => None,
        }
    }

    /// Shared scope survives restart. Each page is bounded; a generation compare-and-clear
    /// retires notification metadata only, without claiming atomicity with media teardown.
    pub async fn poll_community_voice_notifications(&self) {
        if let Err(error) = self.db().expire_social_event_payloads().await {
            tracing::warn!(%error, "social event payload expiry failed");
        }
        let mut after = String::new();
        loop {
            let page = match self.db().voice_notifications_page(&after).await {
                Ok(page) => page,
                Err(error) => {
                    tracing::warn!(%error, "voice notification poll failed");
                    return;
                }
            };
            for entry in &page {
                after.clone_from(&entry.raw_community);
                if !entry.fresh {
                    let _ = self.db().clear_voice_notification(entry).await;
                    continue;
                }
                match self
                    .voice()
                    .community_voice_chat_active(entry.community)
                    .await
                {
                    Ok(true) => {
                        let _ = self.db().refresh_voice_notification(entry).await;
                    }
                    Ok(false) => {
                        if self
                            .db()
                            .clear_voice_notification(entry)
                            .await
                            .unwrap_or(false)
                            && self
                                .db()
                                .voice_generation_is(&entry.raw_community, None)
                                .await
                                .unwrap_or(false)
                        {
                            self.pubsub()
                                .community_voice(
                                    self.db().pool(),
                                    entry.community,
                                    crate::rpc::pubsub::community_voice_ended(&entry.raw_community),
                                    entry.scope == super::fanout::Scope::All,
                                    None,
                                )
                                .await;
                        }
                    }
                    Err(error) => tracing::warn!(%error, "voice status poll failed"),
                }
            }
            if page.len() < 64 {
                return;
            }
            tokio::task::yield_now().await;
        }
    }

    /// Administrative broadcasts also cannot publish a status contrary to current topology.
    pub async fn fan_connectivity(&self, address: &str, status: ConnectivityStatus) {
        let snapshot = self.topology().snapshot();
        let _ = tokio::time::timeout(crate::comms::topology::TOPOLOGY_RECEIPT_TTL, async {
            let mut cursor = ConnectivityCursor::default();
            loop {
                if !self.topology().deliver_if_current(
                    snapshot.generation,
                    address,
                    status == ConnectivityStatus::Online,
                    || {},
                ) {
                    break;
                }
                match self
                    .fan_topology_page(
                        address,
                        status == ConnectivityStatus::Online,
                        snapshot.generation,
                        cursor,
                    )
                    .await
                {
                    Ok((_, true)) | Err(_) => break,
                    Ok((next, false)) => {
                        cursor = next;
                    }
                }
            }
        })
        .await;
    }

    pub(crate) async fn fan_topology_page(
        &self,
        address: &str,
        online: bool,
        generation: u64,
        mut cursor: ConnectivityCursor,
    ) -> Result<(ConnectivityCursor, bool), crate::rpc::db::DbError> {
        let _permit = self
            .0
            .connectivity_operations
            .acquire()
            .await
            .expect("connectivity budget stays open");
        let address = address.to_lowercase();
        let status = if online {
            ConnectivityStatus::Online
        } else {
            ConnectivityStatus::Offline
        } as i32;
        if cursor.online != Some(online) {
            cursor = ConnectivityCursor {
                online: Some(online),
                ..Default::default()
            };
        }
        if !self
            .topology()
            .deliver_if_current(generation, &address, online, || {})
        {
            return Ok((cursor, false));
        }
        if !cursor.friends_done {
            let friends = self
                .0
                .db
                .connectivity_friends_page(&address, &cursor.friend_after, true)
                .await?;
            if !friends.is_empty() {
                // Upstream's live loop skips a connectivity update whose
                // wallet the registry holds no profile for.
                let Some(mut profile) = self.0.profiles.existing_friend_profile(&address).await
                else {
                    cursor.friends_done = true;
                    return Ok((cursor, false));
                };
                // Raw topology identity authorizes and routes the actor. RPC
                // enrichment may expose a historical profile userId alias later.
                profile.address = address.clone();
                for friend in &friends {
                    if !self
                        .topology()
                        .deliver_if_current(generation, &address, online, || {
                            self.0.pubsub.publish(
                                friend,
                                SocialEvent::TopologyFriendConnectivity(
                                    FriendConnectivityUpdate {
                                        stream_closed: None,
                                        friend: Some(profile.clone()),
                                        status,
                                    },
                                    generation,
                                ),
                            );
                        })
                    {
                        return Ok((cursor, false));
                    }
                }
                cursor.friend_after = friends.last().unwrap().clone();
            }
            cursor.friends_done = friends.len() < 256;
            return Ok((cursor, false));
        }
        let members = self
            .0
            .db
            .connectivity_members_page(&address, cursor.member_after.clone())
            .await?;
        for (community_id, member) in &members {
            if !self
                .topology()
                .deliver_if_current(generation, &address, online, || {
                    self.0.pubsub.publish(
                        member,
                        SocialEvent::TopologyCommunityMember(
                            CommunityMemberConnectivityUpdate {
                                stream_closed: None,
                                community_id: community_id.to_string(),
                                member: Some(User {
                                    address: address.clone(),
                                }),
                                status,
                            },
                            generation,
                        ),
                    );
                })
            {
                return Ok((cursor, false));
            }
        }
        if let Some(last) = members.last() {
            cursor.member_after = Some(last.clone());
        }
        let completed = members.len() < 256;
        Ok((cursor, completed))
    }

    /// A Social disconnect ends only its pending call. Accepted media rooms
    /// have their own lifecycle and survive the Social socket disconnect.
    pub async fn end_private_voice_chat_on_disconnect(&self, address: &str) {
        let pending =
            crate::comms::gatekeeper::voice::db::PendingPrivateCall::Address(address.to_string());
        let chat = match self.0.voice.finish_pending_private_call(pending).await {
            Ok(Some(c)) => c,
            Ok(None) => return,
            Err(e) => {
                tracing::warn!(error = %e, %address, "disconnect voice-cleanup failed");
                return;
            }
        };
        self.publish_private_voice_ended_by(&chat, address).await;
        if let Some(state) = self.communities() {
            state.fire_analytics("END_CALL", serde_json::json!({"call_id":chat.id,"user_id":chat.caller_address,"receiver_id":chat.callee_address}));
        }
    }

    /// ENDED to the party that did not end the call (upstream
    /// `endPrivateVoiceChat`, `src/logic/voice/voice.ts:203-208`, `:230-237`:
    /// the acting side is left out of the update), whether it ended or
    /// dropped.
    pub async fn publish_private_voice_ended_by(
        &self,
        chat: &crate::comms::gatekeeper::voice::db::PrivateVoiceChatRow,
        ender: &str,
    ) {
        let update = PrivateVoiceChatUpdate {
            stream_closed: None,
            call_id: chat.id.to_string(),
            status: PrivateVoiceChatStatus::VoiceChatEnded as i32,
            caller: None,
            callee: None,
            credentials: None,
        };
        for party in [&chat.caller_address, &chat.callee_address] {
            if !party.eq_ignore_ascii_case(ender) {
                self.0
                    .pubsub
                    .publish_mutation(party, SocialEvent::PrivateVoice(update.clone()))
                    .await;
            }
        }
    }

    /// The same update to both parties of `chat`.
    pub async fn publish_private_voice(
        &self,
        chat: &crate::comms::gatekeeper::voice::db::PrivateVoiceChatRow,
        status: PrivateVoiceChatStatus,
    ) {
        let update = PrivateVoiceChatUpdate {
            stream_closed: None,
            call_id: chat.id.to_string(),
            status: status as i32,
            caller: None,
            callee: None,
            credentials: None,
        };
        self.0
            .pubsub
            .publish_mutation(
                &chat.caller_address,
                SocialEvent::PrivateVoice(update.clone()),
            )
            .await;
        self.0
            .pubsub
            .publish_mutation(&chat.callee_address, SocialEvent::PrivateVoice(update))
            .await;
    }

    /// The periodic tick. The DELETE runs only once a call can have expired
    /// (the earliest due instant is noted on local starts and re-read from the
    /// table after each sweep) or every [`PRIVATE_VOICE_SWEEP_FALLBACK_MS`]
    /// for rows another process wrote, so a call is still reclaimed within
    /// one tick of its TTL.
    pub async fn sweep_private_voice_chats_if_due(
        &self,
        expiration_ms: i64,
        batch_size: i64,
    ) -> usize {
        let now = chrono::Utc::now().timestamp_millis();
        if !self.0.db.private_voice_sweep_due(now) {
            return 0;
        }
        self.0.db.claim_private_voice_sweep();
        let total = self
            .expire_private_voice_chats(expiration_ms, batch_size)
            .await;
        let next = match self.0.db.private_voice_next_expiry_ms(expiration_ms).await {
            Ok(Some(remaining)) => now + remaining.max(0),
            Ok(None) => now + PRIVATE_VOICE_SWEEP_FALLBACK_MS,
            Err(_) => now,
        };
        self.0
            .db
            .note_private_voice_due(next.min(now + PRIVATE_VOICE_SWEEP_FALLBACK_MS));
        total
    }

    pub async fn expire_private_voice_chats(&self, expiration_ms: i64, batch_size: i64) -> usize {
        let mut total = 0usize;
        loop {
            let expired = match self
                .0
                .voice
                .expire_pending_private_calls(expiration_ms, batch_size)
                .await
            {
                Ok(rows) => rows,
                Err(e) => {
                    tracing::warn!(error = %e, "private voice chat expiry sweep failed");
                    break;
                }
            };
            if expired.is_empty() {
                break;
            }
            for chat in &expired {
                self.publish_private_voice(chat, PrivateVoiceChatStatus::VoiceChatExpired)
                    .await;
                if let Some(state) = self.communities() {
                    state.fire_analytics("EXPIRE_CALL", serde_json::json!({"call_id":chat.id}));
                }
            }
            total += expired.len();
        }
        total
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn presence_counts_sockets_per_address() {
        let ctx = Context::new(
            VoiceSettings::default(),
            Db::new(
                sqlx::postgres::PgPoolOptions::new()
                    .connect_lazy("postgres://localhost/unused")
                    .unwrap(),
            ),
            Profiles::new(None),
            Arc::new(crate::comms::gatekeeper::voice::rooms::NoVoiceRuntime),
        );
        assert!(ctx.mark_online("0xAbC"));
        assert!(
            !ctx.mark_online("0xabc"),
            "a second socket does not re-announce"
        );
        assert!(
            !ctx.is_online("0xABC"),
            "sockets do not establish Pulse presence"
        );
        assert!(!ctx.mark_offline("0xabc"));
        assert!(
            ctx.mark_offline("0xabc"),
            "the last socket announces offline"
        );
        assert!(!ctx.is_online("0xabc"));
        assert!(!ctx.mark_offline("0xnever"));
        assert_eq!(ctx.online_count(), 0);

        ctx.register_identity(7, "0xAbC".into());
        assert_eq!(ctx.identity(7).as_deref(), Some("0xabc"));
        let kill = Arc::new(SocketClose::default());
        ctx.register_kill_handle(7, kill.clone());
        assert_eq!(ctx.disconnect_address("0xABC"), 1);
        assert_eq!(kill.frame(), Some((1000, "Disconnected by the operator")));
        ctx.forget_identity(7);
        assert_eq!(ctx.disconnect_address("0xabc"), 0);
        assert!(ctx.identity(7).is_none());
    }
    #[tokio::test]
    async fn socket_and_subscriber_metrics_follow_source_owned_lifecycle_counts() {
        let recorder = crate::comms::metrics::prometheus_builder()
            .unwrap()
            .build_recorder();
        let handle = recorder.handle();
        let _recording = metrics::set_default_local_recorder(&recorder);
        let ctx = Context::new(
            VoiceSettings::default(),
            Db::new(
                sqlx::postgres::PgPoolOptions::new()
                    .connect_lazy("postgres://localhost/unused")
                    .unwrap(),
            ),
            Profiles::new(None),
            Arc::new(crate::comms::gatekeeper::voice::rooms::NoVoiceRuntime),
        );
        let rpc = ctx.open_socket(Protocol::Rpc, Some(2)).unwrap();
        let stream = ctx.open_socket(Protocol::Stream, Some(2)).unwrap();
        assert!(ctx.open_socket(Protocol::Rpc, Some(2)).is_none());
        let rendered = handle.render();
        assert!(rendered.contains("ws_active_connections 2"));
        assert!(rendered.contains("ws_connections_rejected 1"));
        rpc.close.peer_closed(1000);
        drop(rpc);
        drop(stream);
        let rendered = handle.render();
        assert!(rendered.contains("ws_active_connections 0"));
        assert!(rendered.contains("ws_connection_duration_seconds_count 2"));
        assert!(rendered.contains("ws_close_codes{code=\"1000\"} 1"));
        ctx.mark_online("fixture");
        ctx.mark_online("fixture");
        ctx.mark_online("other");
        let receiver = ctx
            .pubsub()
            .subscribe_updates::<crate::rpc::proto::v2::BlockUpdate>("fixture");
        assert!(
            !handle.render().contains("subscribers_generators_count"),
            "reporting waits for its callback"
        );
        ctx.report_subscriber_metrics();
        let fixture: serde_json::Value = serde_json::from_str(include_str!(concat!(env!("DCL_ONE_ROOT"), "/services/catalyst/tests/fixtures/conformance/social/application-metrics-source-cases.json"))).unwrap();
        for observation in &fixture["subscriberObservations"].as_array().unwrap()[..2] {
            assert!(handle.render().contains(&format!(
                "{} {}",
                observation["name"].as_str().unwrap(),
                observation["value"]
            )));
        }
        drop(receiver);
        ctx.mark_offline("fixture");
        ctx.mark_offline("fixture");
        ctx.mark_offline("other");
        ctx.report_subscriber_metrics();
        for observation in &fixture["subscriberObservations"].as_array().unwrap()[2..] {
            assert!(handle.render().contains(&format!(
                "{} {}",
                observation["name"].as_str().unwrap(),
                observation["value"]
            )));
        }
    }
}
