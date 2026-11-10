use alloy_core::primitives::Address;
use anyhow::anyhow;
use bevy::{
    log::{debug, error, info, warn},
    platform::collections::{HashMap, HashSet},
};
use common::util::AsH160;
#[cfg(target_arch = "wasm32")]
use dcl_component::proto_components::social_service::v2::GetFriendshipStatusPayload;
use dcl_component::proto_components::social_service::v2::{
    friendship_update, get_private_messages_settings_response, get_social_settings_response,
    paginated_friendship_requests_response,
    upsert_friendship_payload::{
        self, AcceptPayload, CancelPayload, DeletePayload, RejectPayload, RequestPayload,
    },
    upsert_friendship_response, upsert_social_settings_response, BlockUserPayload,
    ConnectivityStatus, FriendProfile, FriendshipRequestResponse, GetBlockedUsersPayload,
    GetBlockingStatusResponse, GetFriendsPayload, GetFriendshipRequestsPayload,
    GetMutualFriendsPayload, GetPrivateMessagesSettingsPayload, Pagination,
    PrivateMessagePrivacySetting, SocialServiceClient, SocialServiceClientDefinition,
    UnblockUserPayload, UpsertFriendshipPayload, UpsertSocialSettingsPayload, User,
};
use dcl_rpc::{
    client::{ClientResult, RpcClient},
    transports::web_sockets::{Message, WebSocket, WebSocketTransport},
};
use futures_util::{pin_mut, select, FutureExt};
use std::{
    future::Future,
    sync::atomic::{AtomicBool, Ordering},
};
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};
use web_time::Duration;

use crate::rpc_websocket::PlatformRpcWebSocket;
use crate::runtime::SocialRuntime;
use crate::{DirectChatMessage, DmPrivacy, DmPrivacyOf};

pub enum SocialQuery {
    GetMutualFriends {
        address: String,
        response: tokio::sync::oneshot::Sender<Result<Vec<FriendProfile>, String>>,
    },
    BlockUser {
        address: String,
        response: tokio::sync::oneshot::Sender<Result<(), String>>,
    },
    UnblockUser {
        address: String,
        response: tokio::sync::oneshot::Sender<Result<(), String>>,
    },
    GetBlockedUsers {
        response: tokio::sync::oneshot::Sender<Result<Vec<FriendProfile>, String>>,
    },
    GetBlockingStatus {
        response: tokio::sync::oneshot::Sender<BlockingStatusResult>,
    },
    GetSocialSettings {
        response: tokio::sync::oneshot::Sender<Result<DmPrivacy, String>>,
    },
    UpsertSocialSettings {
        privacy: DmPrivacy,
        response: tokio::sync::oneshot::Sender<Result<DmPrivacy, String>>,
    },
    GetPrivateMessagesSettings {
        addresses: Vec<String>,
        response: tokio::sync::oneshot::Sender<Result<Vec<DmPrivacyOf>, String>>,
    },
}

enum FriendData {
    Init {
        sent_requests: HashMap<Address, FriendshipRequestResponse>,
        received_requests: HashMap<Address, FriendshipRequestResponse>,
        friends: HashMap<Address, FriendProfile>,
        blocks: Option<BlockSnapshot>,
        dm_privacy: DmPrivacy,
    },
    BlockSnapshot(BlockSnapshot),
    FriendshipEvent(friendship_update::Update),
    ConnectivityEvent {
        address: Address,
        status: ConnectivityStatus,
    },
    /// A member of a community the local user is in came online or went offline (from
    /// SubscribeToCommunityMemberConnectivityUpdates); never `Away`.
    CommunityConnectivityEvent {
        community_id: String,
        address: Address,
        status: ConnectivityStatus,
    },
    OwnRequestSent {
        address: Address,
        req: FriendshipRequestResponse,
    },
    OwnRequestAccepted {
        address: Address,
        profile: FriendProfile,
    },
    OwnBlock {
        address: Address,
    },
    OwnUnblock {
        address: Address,
    },
    /// The local user's DM privacy as the server stored it.
    OwnDmPrivacy(DmPrivacy),
    /// Someone blocked / unblocked the local user (from SubscribeToBlockUpdates).
    BlockUpdate {
        address: String,
        is_blocked: bool,
    },
    Disconnected,
}

/// Friendship action sent over the action channel together with a oneshot
/// reply channel: the dispatcher forwards the RPC response (Ok / explicit
/// error variant) back so callers can surface backend rejections like
/// `BlockedUserError`, moderation failures, etc.
pub type UpsertFriendshipRequest = (
    UpsertFriendshipPayload,
    tokio::sync::oneshot::Sender<Result<(), String>>,
);

/// `(addresses I blocked, addresses that blocked me)` — the payload of
/// `GetBlockingStatus`.
pub type BlockingStatus = (Vec<String>, Vec<String>);

/// Result carried back over a oneshot reply for `GetBlockingStatus`.
pub type BlockingStatusResult = Result<BlockingStatus, String>;

struct BlockSnapshot {
    blocked: HashSet<Address>,
    blocked_by: HashSet<Address>,
    complete: bool,
}

/// Callback invoked when someone blocks / unblocks the local user.
type BlockUpdateCallback = Box<dyn Fn(&str, bool) + Send + Sync + 'static>;
/// Called with a community's id when one of its members comes online or goes offline.
type CommunityConnectivityCallback =
    Box<dyn Fn(&str, Address, ConnectivityStatus) + Send + Sync + 'static>;

pub struct SocialClientHandler {
    sender: UnboundedSender<UpsertFriendshipRequest>,
    query_sender: UnboundedSender<SocialQuery>,
    friendship_receiver: UnboundedReceiver<FriendData>,
    owner: Address,
    blocks_ready: bool,

    pub is_initialized: bool,
    pub sent_requests: HashMap<Address, FriendshipRequestResponse>,
    pub received_requests: HashMap<Address, FriendshipRequestResponse>,
    pub friends: HashMap<Address, FriendProfile>,
    pub friend_status: HashMap<Address, ConnectivityStatus>,
    /// Addresses the local user has blocked.
    pub blocked: HashSet<Address>,
    /// Addresses that have blocked the local user.
    pub blocked_by: HashSet<Address>,
    /// The local user's own DM privacy.
    pub dm_privacy: DmPrivacy,

    pub unread_messages: HashMap<Address, usize>,

    friend_event_callback: Box<dyn Fn(&friendship_update::Update) + Send + Sync + 'static>,
    connectivity_callback: Box<dyn Fn(Address, ConnectivityStatus) + Send + Sync + 'static>,
    block_update_callback: BlockUpdateCallback,
    community_connectivity_callback: CommunityConnectivityCallback,
    #[allow(dead_code)]
    chat_event_callback: Box<dyn Fn(DirectChatMessage) + Send + Sync + 'static>,
}

impl SocialClientHandler {
    pub fn connect(
        wallet: wallet::Wallet,
        runtime: &SocialRuntime,
        friend_callback: impl Fn(&friendship_update::Update) + Send + Sync + 'static,
        connectivity_callback: impl Fn(Address, ConnectivityStatus) + Send + Sync + 'static,
        block_update_callback: impl Fn(&str, bool) + Send + Sync + 'static,
        chat_callback: impl Fn(DirectChatMessage) + Send + Sync + 'static,
        community_connectivity_callback: impl Fn(&str, Address, ConnectivityStatus)
            + Send
            + Sync
            + 'static,
    ) -> Option<Self> {
        let wallet = wallet.snapshot();
        let owner = wallet.address()?;
        let (event_sx, event_rx) = mpsc::unbounded_channel();
        let (response_sx, response_rx) = mpsc::unbounded_channel();
        let (query_sx, query_rx) = mpsc::unbounded_channel();

        runtime.spawn(async move {
            if let Err(e) =
                social_socket_handler_inner(wallet, event_rx, query_rx, response_sx).await
            {
                error!("[social] socket handler error: {e}");
            } else {
                debug!("[social] socket handler finished");
            }
        });

        Some(Self {
            is_initialized: false,
            sender: event_sx,
            query_sender: query_sx,
            friendship_receiver: response_rx,
            owner,
            blocks_ready: false,
            sent_requests: Default::default(),
            received_requests: Default::default(),
            friends: Default::default(),
            friend_status: Default::default(),
            blocked: Default::default(),
            blocked_by: Default::default(),
            dm_privacy: Default::default(),
            unread_messages: Default::default(),
            friend_event_callback: Box::new(friend_callback),
            connectivity_callback: Box::new(connectivity_callback),
            block_update_callback: Box::new(block_update_callback),
            chat_event_callback: Box::new(chat_callback),
            community_connectivity_callback: Box::new(community_connectivity_callback),
        })
    }

    pub fn live(&self) -> bool {
        !self.friendship_receiver.is_closed()
    }

    pub(super) fn block_snapshot_owner(&self) -> Option<Address> {
        (self.blocks_ready && self.is_initialized && self.live()).then_some(self.owner)
    }

    fn apply_block_snapshot(&mut self, snapshot: BlockSnapshot) {
        self.blocked = snapshot.blocked;
        self.blocked_by = snapshot.blocked_by;
        self.blocks_ready = snapshot.complete;
    }

    fn make_user(address: Address) -> Option<User> {
        Some(User {
            address: format!("{address:#x}"),
        })
    }

    fn send_action(
        &self,
        action: upsert_friendship_payload::Action,
    ) -> Result<tokio::sync::oneshot::Receiver<Result<(), String>>, anyhow::Error> {
        let (reply_sx, reply_rx) = tokio::sync::oneshot::channel();
        self.sender.send((
            UpsertFriendshipPayload {
                action: Some(action),
            },
            reply_sx,
        ))?;
        Ok(reply_rx)
    }

    pub fn friend_request(
        &mut self,
        address: Address,
        message: Option<String>,
    ) -> Result<tokio::sync::oneshot::Receiver<Result<(), String>>, anyhow::Error> {
        self.send_action(upsert_friendship_payload::Action::Request(RequestPayload {
            user: Self::make_user(address),
            message,
        }))
    }

    pub fn cancel_request(
        &mut self,
        address: Address,
    ) -> Result<tokio::sync::oneshot::Receiver<Result<(), String>>, anyhow::Error> {
        let rx = self.send_action(upsert_friendship_payload::Action::Cancel(CancelPayload {
            user: Self::make_user(address),
        }))?;
        self.sent_requests.remove(&address);
        Ok(rx)
    }

    pub fn accept_request(
        &mut self,
        address: Address,
    ) -> Result<tokio::sync::oneshot::Receiver<Result<(), String>>, anyhow::Error> {
        if self.received_requests.remove(&address).is_none() {
            return Err(anyhow!("no request"));
        };

        self.send_action(upsert_friendship_payload::Action::Accept(AcceptPayload {
            user: Self::make_user(address),
        }))
    }

    pub fn reject_request(
        &mut self,
        address: Address,
    ) -> Result<tokio::sync::oneshot::Receiver<Result<(), String>>, anyhow::Error> {
        if self.received_requests.remove(&address).is_none() {
            return Err(anyhow!("no request"));
        };

        self.send_action(upsert_friendship_payload::Action::Reject(RejectPayload {
            user: Self::make_user(address),
        }))
    }

    pub fn delete_friend(
        &mut self,
        address: Address,
    ) -> Result<tokio::sync::oneshot::Receiver<Result<(), String>>, anyhow::Error> {
        if self.friends.remove(&address).is_none() {
            return Err(anyhow!("no request"));
        };

        let reply_rx =
            self.send_action(upsert_friendship_payload::Action::Delete(DeletePayload {
                user: Self::make_user(address),
            }))?;
        // The server doesn't echo own actions back over the connectivity stream, so
        // emit a synthetic offline transition so subscribers (UI online list) drop
        // them. No-op if we never had a status for them.
        if self.friend_status.remove(&address).is_some() {
            (self.connectivity_callback)(address, ConnectivityStatus::Offline);
        }
        Ok(reply_rx)
    }

    pub fn get_mutual_friends(
        &self,
        address: String,
    ) -> Result<tokio::sync::oneshot::Receiver<Result<Vec<FriendProfile>, String>>, anyhow::Error>
    {
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.query_sender.send(SocialQuery::GetMutualFriends {
            address,
            response: tx,
        })?;
        Ok(rx)
    }

    pub fn block_user(
        &self,
        address: String,
    ) -> Result<tokio::sync::oneshot::Receiver<Result<(), String>>, anyhow::Error> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.query_sender.send(SocialQuery::BlockUser {
            address,
            response: tx,
        })?;
        Ok(rx)
    }

    pub fn unblock_user(
        &self,
        address: String,
    ) -> Result<tokio::sync::oneshot::Receiver<Result<(), String>>, anyhow::Error> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.query_sender.send(SocialQuery::UnblockUser {
            address,
            response: tx,
        })?;
        Ok(rx)
    }

    pub fn get_blocked_users(
        &self,
    ) -> Result<tokio::sync::oneshot::Receiver<Result<Vec<FriendProfile>, String>>, anyhow::Error>
    {
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.query_sender
            .send(SocialQuery::GetBlockedUsers { response: tx })?;
        Ok(rx)
    }

    /// Addresses the local user has blocked + addresses that have blocked
    /// the local user (`(blocked_users, blocked_by_users)`).
    pub fn get_blocking_status(
        &self,
    ) -> Result<tokio::sync::oneshot::Receiver<BlockingStatusResult>, anyhow::Error> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.query_sender
            .send(SocialQuery::GetBlockingStatus { response: tx })?;
        Ok(rx)
    }

    /// The local user's own DM privacy setting.
    pub fn get_social_settings(
        &self,
    ) -> Result<tokio::sync::oneshot::Receiver<Result<DmPrivacy, String>>, anyhow::Error> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.query_sender
            .send(SocialQuery::GetSocialSettings { response: tx })?;
        Ok(rx)
    }

    /// Sets the local user's DM privacy; resolves to the setting the server stored.
    pub fn upsert_social_settings(
        &self,
        privacy: DmPrivacy,
    ) -> Result<tokio::sync::oneshot::Receiver<Result<DmPrivacy, String>>, anyhow::Error> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.query_sender.send(SocialQuery::UpsertSocialSettings {
            privacy,
            response: tx,
        })?;
        Ok(rx)
    }

    /// DM privacy of other users, batched.
    pub fn get_private_messages_settings(
        &self,
        addresses: Vec<String>,
    ) -> Result<tokio::sync::oneshot::Receiver<Result<Vec<DmPrivacyOf>, String>>, anyhow::Error>
    {
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.query_sender
            .send(SocialQuery::GetPrivateMessagesSettings {
                addresses,
                response: tx,
            })?;
        Ok(rx)
    }

    pub fn chat(&self, _address: Address, _message: String) -> Result<(), anyhow::Error> {
        // DM chat not supported in V2 (no Matrix)
        Err(anyhow!("chat not available in V2"))
    }

    pub fn get_chat_history(
        &self,
        _address: Address,
    ) -> Result<tokio::sync::mpsc::Receiver<DirectChatMessage>, anyhow::Error> {
        // Chat history via Matrix is no longer supported in V2
        Err(anyhow!("chat history not available in V2"))
    }

    pub fn mark_as_read(&mut self, address: Address) {
        self.unread_messages.remove(&address);
    }

    pub fn unread_messages(&self) -> &HashMap<Address, usize> {
        &self.unread_messages
    }

    /// Applies pending updates; true if any arrived.
    pub fn update(&mut self) -> bool {
        let mut changed = false;
        while let Ok(rec) = self.friendship_receiver.try_recv() {
            changed = true;
            match rec {
                FriendData::Init {
                    sent_requests,
                    received_requests,
                    friends,
                    blocks,
                    dm_privacy,
                } => {
                    self.received_requests = received_requests;
                    self.sent_requests = sent_requests;
                    self.friends = friends;
                    if let Some(snapshot) = blocks {
                        self.apply_block_snapshot(snapshot);
                    } else {
                        self.blocks_ready = false;
                        self.blocked.clear();
                        self.blocked_by.clear();
                    }
                    self.dm_privacy = dm_privacy;
                    self.is_initialized = true;
                }
                FriendData::BlockSnapshot(snapshot) => self.apply_block_snapshot(snapshot),
                FriendData::FriendshipEvent(ev) => {
                    (self.friend_event_callback)(&ev);
                    match ev {
                        friendship_update::Update::Request(body) => {
                            let Some(friend_profile) = body.friend.as_ref() else {
                                warn!("invalid friend request (no friend profile): {body:?}");
                                continue;
                            };
                            let Some(address) = friend_profile.address.as_h160() else {
                                warn!("invalid friend request (no address): {body:?}");
                                continue;
                            };
                            self.received_requests.insert(
                                address,
                                FriendshipRequestResponse {
                                    friend: body.friend.clone(),
                                    created_at: body.created_at,
                                    message: body.message.clone(),
                                    id: body.id.clone(),
                                },
                            );
                        }
                        friendship_update::Update::Accept(body) => {
                            let Some(address) =
                                body.user.as_ref().and_then(|u| u.address.as_h160())
                            else {
                                warn!("invalid friend accept (no address): {body:?}");
                                continue;
                            };
                            // Move from sent_requests to friends
                            if let Some(req) = self.sent_requests.remove(&address) {
                                if let Some(profile) = req.friend {
                                    self.friends.insert(address, profile);
                                }
                            }
                        }
                        friendship_update::Update::Reject(body) => {
                            let Some(address) =
                                body.user.as_ref().and_then(|u| u.address.as_h160())
                            else {
                                warn!("invalid friend reject (no address): {body:?}");
                                continue;
                            };
                            self.sent_requests.remove(&address);
                        }
                        friendship_update::Update::Delete(body) => {
                            let Some(address) =
                                body.user.as_ref().and_then(|u| u.address.as_h160())
                            else {
                                warn!("invalid friend delete (no address): {body:?}");
                                continue;
                            };
                            self.friends.remove(&address);
                            self.friend_status.remove(&address);
                        }
                        friendship_update::Update::Cancel(body) => {
                            let Some(address) =
                                body.user.as_ref().and_then(|u| u.address.as_h160())
                            else {
                                warn!("invalid friend cancel (no address): {body:?}");
                                continue;
                            };
                            self.received_requests.remove(&address);
                        }
                        friendship_update::Update::Block(body) => {
                            let Some(address) =
                                body.user.as_ref().and_then(|u| u.address.as_h160())
                            else {
                                warn!("invalid block event (no address): {body:?}");
                                continue;
                            };
                            // When someone blocks us, remove them from friends and requests
                            self.friends.remove(&address);
                            self.friend_status.remove(&address);
                            self.sent_requests.remove(&address);
                            self.received_requests.remove(&address);
                        }
                    }
                }
                FriendData::ConnectivityEvent { address, status } => {
                    if self.friends.contains_key(&address) {
                        self.friend_status.insert(address, status);
                        (self.connectivity_callback)(address, status);
                    }
                }
                FriendData::CommunityConnectivityEvent {
                    community_id,
                    address,
                    status,
                } => {
                    (self.community_connectivity_callback)(&community_id, address, status);
                }
                FriendData::OwnRequestSent { address, req } => {
                    self.sent_requests.insert(address, req);
                }
                FriendData::OwnRequestAccepted { address, profile } => {
                    self.received_requests.remove(&address);
                    self.friends.insert(address, profile);
                }
                FriendData::OwnBlock { address } => {
                    self.blocked.insert(address);
                    self.friends.remove(&address);
                    // Synthesize an offline transition for subscribers (UI online
                    // list) since the server doesn't echo own actions over the
                    // connectivity stream.
                    if self.friend_status.remove(&address).is_some() {
                        (self.connectivity_callback)(address, ConnectivityStatus::Offline);
                    }
                    self.sent_requests.remove(&address);
                    self.received_requests.remove(&address);
                }
                FriendData::OwnUnblock { address } => {
                    self.blocked.remove(&address);
                }
                FriendData::OwnDmPrivacy(privacy) => {
                    self.dm_privacy = privacy;
                }
                FriendData::BlockUpdate {
                    address,
                    is_blocked,
                } => {
                    if let Some(address) = address.as_h160() {
                        if is_blocked {
                            self.blocked_by.insert(address);
                        } else {
                            self.blocked_by.remove(&address);
                        }
                    }
                    (self.block_update_callback)(&address, is_blocked);
                }
                FriendData::Disconnected => {
                    self.is_initialized = false;
                    self.blocks_ready = false;
                    self.friend_status.clear();
                }
            }
        }
        changed
    }
}

fn dbgerr<E: std::fmt::Debug>(e: E) -> anyhow::Error {
    anyhow!(format!("{e:?}"))
}

fn social_url() -> String {
    common::base_domain::service(common::base_domain::Service::SocialRpc)
}

const PAGE_SIZE: i32 = 100;
const BLOCK_SNAPSHOT_RETRY_DELAY: Duration = Duration::from_secs(5);
const BLOCK_SNAPSHOT_TIMEOUT: Duration = Duration::from_secs(10);

fn block_snapshot_from_response(
    result: ClientResult<GetBlockingStatusResponse>,
) -> Option<BlockSnapshot> {
    match result {
        Ok(resp) => {
            let mut complete = true;
            let mut parse_addresses = |addresses: Vec<String>| {
                addresses
                    .into_iter()
                    .filter_map(|address| {
                        let parsed = address.as_h160();
                        complete &= parsed.is_some();
                        parsed
                    })
                    .collect()
            };
            let blocked = parse_addresses(resp.blocked_users);
            let blocked_by = parse_addresses(resp.blocked_by_users);
            if !complete {
                warn!("[social] get_blocking_status returned an invalid address");
            }
            // Direct messages use the known blocks. Voice waits for both complete lists.
            Some(BlockSnapshot {
                blocked,
                blocked_by,
                complete,
            })
        }
        Err(e) => {
            warn!("[social] get_blocking_status error: {e:?}");
            None
        }
    }
}

async fn retry_block_snapshot(
    dirty: &AtomicBool,
    events: &UnboundedSender<FriendData>,
    fetch: impl Future<Output = Result<Option<BlockSnapshot>, anyhow::Error>>,
) -> Result<bool, anyhow::Error> {
    dirty.store(false, Ordering::Relaxed);
    let Some(snapshot) = fetch.await? else {
        return Ok(false);
    };
    // The stream and retry futures are polled by the same connection task. A
    // delta forwarded while this request was pending can postdate the snapshot.
    if dirty.load(Ordering::Relaxed) {
        return Ok(false);
    }
    let complete = snapshot.complete;
    events.send(FriendData::BlockSnapshot(snapshot))?;
    Ok(complete)
}

fn forward_block_update(
    dirty: &AtomicBool,
    events: &UnboundedSender<FriendData>,
    address: String,
    is_blocked: bool,
) -> Result<(), anyhow::Error> {
    dirty.store(true, Ordering::Relaxed);
    events.send(FriendData::BlockUpdate {
        address,
        is_blocked,
    })?;
    Ok(())
}

async fn social_socket_handler_inner(
    wallet: wallet::Wallet,
    mut rx: UnboundedReceiver<UpsertFriendshipRequest>,
    mut query_rx: UnboundedReceiver<SocialQuery>,
    response_sx: UnboundedSender<FriendData>,
) -> Result<(), anyhow::Error> {
    let initial_backoff = Duration::from_secs(1);
    let max_backoff = Duration::from_secs(30);
    let mut backoff = initial_backoff;

    loop {
        let start = web_time::Instant::now();
        let result = run_one_connection(&wallet, &mut rx, &mut query_rx, &response_sx).await;
        let lasted = start.elapsed();
        match &result {
            Ok(()) => info!("[social] connection closed after {lasted:?}"),
            Err(e) => error!("[social] connection error after {lasted:?}: {e}"),
        }

        // If the client handle was dropped, exit cleanly — no point reconnecting.
        if response_sx.is_closed() || rx.is_closed() {
            info!("[social] handler shutting down (client dropped)");
            return Ok(());
        }

        // Notify subscribers of the disconnect so JS can gate UI off `is_initialized`.
        if response_sx.send(FriendData::Disconnected).is_err() {
            return Ok(());
        }

        // Reset backoff once a connection has lasted long enough to be considered healthy.
        if lasted > Duration::from_secs(60) {
            backoff = initial_backoff;
        }

        info!("[social] reconnecting in {backoff:?}...");
        async_std::task::sleep(backoff).await;
        backoff = (backoff * 2).min(max_backoff);
    }
}

async fn social_auth_headers(
    wallet: &wallet::Wallet,
    connection_uri: &http::Uri,
) -> Result<Vec<(String, String)>, anyhow::Error> {
    // Social verifies the protocol root even when the connection uses a gateway prefix.
    let mut parts = connection_uri.clone().into_parts();
    parts.path_and_query = Some(http::uri::PathAndQuery::from_static("/"));
    let signed_uri = http::Uri::from_parts(parts)?;
    wallet::sign_request("get", &signed_uri, wallet, "{}".to_owned()).await
}

async fn run_one_connection(
    wallet: &wallet::Wallet,
    rx: &mut UnboundedReceiver<UpsertFriendshipRequest>,
    query_rx: &mut UnboundedReceiver<SocialQuery>,
    response_sx: &UnboundedSender<FriendData>,
) -> Result<(), anyhow::Error> {
    // Connect WebSocket
    info!("[social] Connecting to social service at {}", social_url());
    let (ws, ws_closed) = PlatformRpcWebSocket::connect(&social_url())
        .await
        .map_err(dbgerr)?;
    info!(
        "[social] Successfully connected to social service at {}",
        social_url()
    );

    // V2 auth: send signed headers as first WS message
    let uri: http::Uri = social_url().parse().map_err(dbgerr)?;
    let signed_headers = social_auth_headers(wallet, &uri).await.map_err(dbgerr)?;
    let headers_map: std::collections::HashMap<String, String> =
        signed_headers.into_iter().collect();
    let auth_json = serde_json::to_string(&headers_map)?;
    debug!("[social] Sending auth headers: {auth_json}");
    ws.send(Message::Text(auth_json)).await.map_err(dbgerr)?;

    // Keep an Arc handle to the websocket so the keepalive task can send pings while the
    // RPC transport owns its own clone for normal traffic. Only used on native — wasm
    // can't send WS Ping frames and uses an RPC-level heartbeat instead.
    #[cfg(not(target_arch = "wasm32"))]
    let ws_for_ping = ws.clone();

    // Create RPC client
    let service_transport = WebSocketTransport::new(ws);
    let mut service_client = RpcClient::new(service_transport)
        .await
        .map_err(|e| anyhow!("[social] Failed to create RPC client: {e:?}"))?;
    let port = service_client
        .create_port("social")
        .await
        .map_err(|e| anyhow!("[social] Failed to create port: {e:?}"))?;
    let service_module = port
        .load_module::<SocialServiceClient<_>>("SocialService")
        .await
        .map_err(dbgerr)?;
    let service_module = std::sync::Arc::new(service_module);
    #[cfg(target_arch = "wasm32")]
    let service_module_keepalive = service_module.clone();

    // Gather initial data: friends list (paginated)
    debug!("[social] Fetching friends list...");
    let mut friends = HashMap::default();
    let mut offset = 0;
    loop {
        let resp = service_module
            .get_friends(GetFriendsPayload {
                pagination: Some(Pagination {
                    limit: PAGE_SIZE,
                    offset,
                }),
            })
            .await
            .map_err(dbgerr)?;

        debug!(
            "[social] get_friends(offset={offset}): got {} friends, raw response: {:?}",
            resp.friends.len(),
            resp.pagination_data
        );

        for friend in resp.friends {
            if let Some(address) = friend.address.as_h160() {
                friends.insert(address, friend);
            }
        }

        let total = resp.pagination_data.as_ref().map(|p| p.total).unwrap_or(0);
        offset += PAGE_SIZE;
        if offset >= total || friends.is_empty() {
            break;
        }
    }
    debug!("[social] Total friends loaded: {}", friends.len());

    // Gather initial data: received (pending) requests
    debug!("[social] Fetching pending friendship requests...");
    let mut received_requests = HashMap::new();
    let pending_resp = service_module
        .get_pending_friendship_requests(GetFriendshipRequestsPayload {
            pagination: Some(Pagination {
                limit: PAGE_SIZE,
                offset: 0,
            }),
        })
        .await
        .map_err(dbgerr)?;
    debug!(
        "[social] get_pending_friendship_requests response: {:?}",
        pending_resp.response
    );

    if let Some(paginated_friendship_requests_response::Response::Requests(reqs)) =
        pending_resp.response
    {
        for req in reqs.requests {
            if let Some(friend) = &req.friend {
                if let Some(address) = friend.address.as_h160() {
                    received_requests.insert(address, req);
                }
            }
        }
    }
    debug!(
        "[social] Pending requests loaded: {}",
        received_requests.len()
    );

    // Gather initial data: sent requests
    debug!("[social] Fetching sent friendship requests...");
    let mut sent_requests = HashMap::new();
    let sent_resp = service_module
        .get_sent_friendship_requests(GetFriendshipRequestsPayload {
            pagination: Some(Pagination {
                limit: PAGE_SIZE,
                offset: 0,
            }),
        })
        .await
        .map_err(dbgerr)?;
    debug!(
        "[social] get_sent_friendship_requests response: {:?}",
        sent_resp.response
    );

    if let Some(paginated_friendship_requests_response::Response::Requests(reqs)) =
        sent_resp.response
    {
        for req in reqs.requests {
            if let Some(friend) = &req.friend {
                if let Some(address) = friend.address.as_h160() {
                    sent_requests.insert(address, req);
                }
            }
        }
    }
    debug!("[social] Sent requests loaded: {}", sent_requests.len());

    // Preserve the DM fallback if the server does not return a privacy setting.
    let dm_privacy = match service_module.get_social_settings().await {
        Ok(resp) => match resp.response {
            Some(get_social_settings_response::Response::Ok(ok)) => ok
                .settings
                .unwrap_or_default()
                .private_messages_privacy()
                .into(),
            other => {
                warn!("[social] get_social_settings error: {other:?}");
                Default::default()
            }
        },
        Err(e) => {
            warn!("[social] get_social_settings error: {e:?}");
            Default::default()
        }
    };

    // Subscribe to friendship updates
    debug!("[social] Subscribing to friendship updates...");
    let mut inbound_updates = service_module
        .subscribe_to_friendship_updates()
        .await
        .map_err(dbgerr)?;
    debug!("[social] Subscribed to friendship updates");

    // Subscribe to friend connectivity updates
    debug!("[social] Subscribing to friend connectivity updates...");
    let mut connectivity_updates = service_module
        .subscribe_to_friend_connectivity_updates()
        .await
        .map_err(dbgerr)?;
    debug!("[social] Subscribed to friend connectivity updates");

    // Subscribe to block updates (someone blocking / unblocking the local user)
    debug!("[social] Subscribing to block updates...");
    let mut block_updates = service_module
        .subscribe_to_block_updates()
        .await
        .map_err(dbgerr)?;
    debug!("[social] Subscribed to block updates");

    // Subscribe before fetching the snapshot so updates during startup are buffered.
    let load_blocks = |service: std::sync::Arc<SocialServiceClient<_>>| async move {
        // Restart the connection on a hung request so its outstanding RPC listener is discarded.
        let result =
            async_std::future::timeout(BLOCK_SNAPSHOT_TIMEOUT, service.get_blocking_status())
                .await
                .map_err(|_| anyhow!("[social] get_blocking_status timed out"))?;
        Ok::<_, anyhow::Error>(block_snapshot_from_response(result))
    };

    // Subscribe to community member connectivity updates. Unlike the subscriptions above, a
    // refusal is not fatal: a server without them costs community presence, not friends, DMs or
    // blocks.
    debug!("[social] Subscribing to community member connectivity updates...");
    let community_connectivity_updates = match service_module
        .subscribe_to_community_member_connectivity_updates()
        .await
    {
        Ok(updates) => {
            debug!("[social] Subscribed to community member connectivity updates");
            Some(updates)
        }
        Err(e) => {
            warn!("[social] no community member connectivity updates until the next connection: {e:?}");
            None
        }
    };

    let blocks = load_blocks(service_module.clone()).await?;
    let blocks_ready = blocks.as_ref().is_some_and(|snapshot| snapshot.complete);
    let mut block_retry =
        (!blocks_ready).then(|| Box::pin(async_std::task::sleep(BLOCK_SNAPSHOT_RETRY_DELAY)));
    debug!(
        "[social] Init complete — friends: {}, received_requests: {}, sent_requests: {}, blocks ready: {}, dm privacy: {dm_privacy:?}",
        friends.len(),
        received_requests.len(),
        sent_requests.len(),
        blocks_ready,
    );
    response_sx.send(FriendData::Init {
        sent_requests,
        received_requests,
        friends,
        blocks,
        dm_privacy,
    })?;

    let block_snapshot_dirty = &AtomicBool::new(false);
    // Outbound: send friendship actions + handle queries
    let response_sx_write = response_sx.clone();
    let f_service_write = async move {
        let response_sx = response_sx_write;
        loop {
            tokio::select! {
                _ = async {
                    match block_retry.as_mut() {
                        Some(delay) => delay.await,
                        None => std::future::pending().await,
                    }
                } => {
                    // Serialize retries with local block/unblock writes.
                    if retry_block_snapshot(
                        block_snapshot_dirty,
                        &response_sx,
                        load_blocks(service_module.clone()),
                    ).await? {
                        block_retry = None;
                    } else {
                        block_retry = Some(Box::pin(async_std::task::sleep(BLOCK_SNAPSHOT_RETRY_DELAY)));
                    }
                }
                req = rx.recv() => {
                    let Some((req, reply)) = req else { return Result::<(), anyhow::Error>::Ok(()); };
                    debug!("[social] upsert_friendship request: {req:?}");
                    let action = req.action.clone();
                    let resp = service_module
                        .upsert_friendship(req)
                        .await
                        .map_err(|e| anyhow!("[social] upsert_friendship transport error: {e:?}"))?;
                    debug!("[social] upsert_friendship response: {resp:?}");

                    // The server doesn't echo our own actions back via the subscription,
                    // so update local state from the RPC response. The error variants
                    // are forwarded to the caller so the UI can show a meaningful
                    // message instead of a silent no-op.
                    match resp.response {
                        Some(upsert_friendship_response::Response::Accepted(accepted)) => {
                            match action {
                                Some(upsert_friendship_payload::Action::Request(_)) => {
                                    if let Some(friend) = accepted.friend.as_ref() {
                                        if let Some(address) = friend.address.as_h160() {
                                            let req = FriendshipRequestResponse {
                                                friend: accepted.friend.clone(),
                                                created_at: accepted.created_at,
                                                message: accepted.message,
                                                id: accepted.id,
                                            };
                                            let _ = response_sx.send(FriendData::OwnRequestSent { address, req });
                                        }
                                    }
                                }
                                Some(upsert_friendship_payload::Action::Accept(AcceptPayload { user: Some(user) })) => {
                                    if let (Some(address), Some(profile)) = (user.address.as_h160(), accepted.friend) {
                                        let _ = response_sx.send(FriendData::OwnRequestAccepted { address, profile });
                                    }
                                }
                                _ => {}
                            }
                            let _ = reply.send(Ok(()));
                        }
                        Some(upsert_friendship_response::Response::InvalidFriendshipAction(e)) => {
                            let msg = e.message.unwrap_or_default();
                            warn!("[social] upsert_friendship rejected: invalidFriendshipAction: {msg}");
                            let _ = reply.send(Err(format!("invalidFriendshipAction: {msg}")));
                        }
                        Some(upsert_friendship_response::Response::InvalidRequest(e)) => {
                            let msg = e.message.unwrap_or_default();
                            warn!("[social] upsert_friendship rejected: invalidRequest: {msg}");
                            let _ = reply.send(Err(format!("invalidRequest: {msg}")));
                        }
                        Some(upsert_friendship_response::Response::InternalServerError(e)) => {
                            let msg = e.message.unwrap_or_default();
                            warn!("[social] upsert_friendship internal server error: {msg}");
                            let _ = reply.send(Err(format!("internalServerError: {msg}")));
                        }
                        None => {
                            warn!("[social] upsert_friendship returned empty response");
                            let _ = reply.send(Err("empty response from upsert_friendship".to_string()));
                        }
                    }
                }
                query = query_rx.recv() => {
                    let Some(query) = query else { return Result::<(), anyhow::Error>::Ok(()); };
                    match query {
                        SocialQuery::GetMutualFriends { address, response } => {
                            debug!("[social] getMutualFriends request for {address}");
                            let mut all_friends = Vec::new();
                            let mut offset = 0;
                            let mut result: Result<Vec<FriendProfile>, String> = Ok(Vec::new());
                            loop {
                                match service_module.get_mutual_friends(GetMutualFriendsPayload {
                                    user: Some(User { address: address.clone() }),
                                    pagination: Some(Pagination { limit: PAGE_SIZE, offset }),
                                }).await {
                                    Ok(resp) => {
                                        let count = resp.friends.len();
                                        all_friends.extend(resp.friends);
                                        let total = resp.pagination_data.as_ref().map(|p| p.total).unwrap_or(0);
                                        offset += PAGE_SIZE;
                                        if offset >= total || count == 0 { break; }
                                    }
                                    Err(e) => {
                                        warn!("[social] getMutualFriends error: {e:?}");
                                        result = Err(format!("{e:?}"));
                                        break;
                                    }
                                }
                            }
                            if result.is_ok() {
                                debug!("[social] getMutualFriends: {} mutual friends", all_friends.len());
                                let _ = response.send(Ok(all_friends));
                            } else {
                                let _ = response.send(result);
                            }
                        }
                        SocialQuery::BlockUser { address, response } => {
                            debug!("[social] blockUser request for {address}");
                            match service_module.block_user(BlockUserPayload {
                                user: Some(User { address: address.clone() }),
                            }).await {
                                Ok(resp) => {
                                    use dcl_component::proto_components::social_service::v2::block_user_response::Response;
                                    match resp.response {
                                        Some(Response::Ok(_)) => {
                                            debug!("[social] blockUser success for {address}");
                                            if let Some(addr) = address.as_h160() {
                                                let _ = response_sx.send(FriendData::OwnBlock { address: addr });
                                            }
                                            let _ = response.send(Ok(()));
                                        }
                                        Some(Response::InternalServerError(e)) => {
                                            let msg = e.message.unwrap_or_default();
                                            warn!("[social] blockUser internal error: {msg}");
                                            let _ = response.send(Err(msg));
                                        }
                                        Some(Response::InvalidRequest(e)) => {
                                            let msg = e.message.unwrap_or_default();
                                            warn!("[social] blockUser invalid request: {msg}");
                                            let _ = response.send(Err(msg));
                                        }
                                        Some(Response::ProfileNotFound(e)) => {
                                            let msg = e.message.unwrap_or_else(|| "profile not found".to_string());
                                            warn!("[social] blockUser profile not found: {msg}");
                                            let _ = response.send(Err(msg));
                                        }
                                        None => {
                                            let _ = response.send(Err("empty response".to_string()));
                                        }
                                    }
                                }
                                Err(e) => {
                                    warn!("[social] blockUser error: {e:?}");
                                    let _ = response.send(Err(format!("{e:?}")));
                                }
                            }
                        }
                        SocialQuery::UnblockUser { address, response } => {
                            debug!("[social] unblockUser request for {address}");
                            match service_module.unblock_user(UnblockUserPayload {
                                user: Some(User { address: address.clone() }),
                            }).await {
                                Ok(resp) => {
                                    use dcl_component::proto_components::social_service::v2::unblock_user_response::Response;
                                    match resp.response {
                                        Some(Response::Ok(_)) => {
                                            debug!("[social] unblockUser success for {address}");
                                            if let Some(addr) = address.as_h160() {
                                                let _ = response_sx.send(FriendData::OwnUnblock { address: addr });
                                            }
                                            let _ = response.send(Ok(()));
                                        }
                                        Some(Response::InternalServerError(e)) => {
                                            let msg = e.message.unwrap_or_default();
                                            warn!("[social] unblockUser internal error: {msg}");
                                            let _ = response.send(Err(msg));
                                        }
                                        Some(Response::InvalidRequest(e)) => {
                                            let msg = e.message.unwrap_or_default();
                                            warn!("[social] unblockUser invalid request: {msg}");
                                            let _ = response.send(Err(msg));
                                        }
                                        Some(Response::ProfileNotFound(e)) => {
                                            let msg = e.message.unwrap_or_else(|| "profile not found".to_string());
                                            warn!("[social] unblockUser profile not found: {msg}");
                                            let _ = response.send(Err(msg));
                                        }
                                        None => {
                                            let _ = response.send(Err("empty response".to_string()));
                                        }
                                    }
                                }
                                Err(e) => {
                                    warn!("[social] unblockUser error: {e:?}");
                                    let _ = response.send(Err(format!("{e:?}")));
                                }
                            }
                        }
                        SocialQuery::GetBlockedUsers { response } => {
                            debug!("[social] getBlockedUsers request");
                            let mut all_profiles = Vec::new();
                            let mut offset = 0;
                            let mut result: Result<Vec<FriendProfile>, String> = Ok(Vec::new());
                            loop {
                                match service_module.get_blocked_users(GetBlockedUsersPayload {
                                    pagination: Some(Pagination { limit: PAGE_SIZE, offset }),
                                }).await {
                                    Ok(resp) => {
                                        let count = resp.profiles.len();
                                        // Convert BlockedUserProfile to FriendProfile
                                        for blocked in &resp.profiles {
                                            all_profiles.push(FriendProfile {
                                                address: blocked.address.clone(),
                                                name: blocked.name.clone(),
                                                has_claimed_name: blocked.has_claimed_name,
                                                profile_picture_url: blocked.profile_picture_url.clone(),
                                                name_color: blocked.name_color,
                                            });
                                        }
                                        let total = resp.pagination_data.as_ref().map(|p| p.total).unwrap_or(0);
                                        offset += PAGE_SIZE;
                                        if offset >= total || count == 0 { break; }
                                    }
                                    Err(e) => {
                                        warn!("[social] getBlockedUsers error: {e:?}");
                                        result = Err(format!("{e:?}"));
                                        break;
                                    }
                                }
                            }
                            if result.is_ok() {
                                debug!("[social] getBlockedUsers: {} blocked users", all_profiles.len());
                                let _ = response.send(Ok(all_profiles));
                            } else {
                                let _ = response.send(result);
                            }
                        }
                        SocialQuery::GetBlockingStatus { response } => {
                            debug!("[social] getBlockingStatus request");
                            match service_module.get_blocking_status().await {
                                Ok(resp) => {
                                    debug!(
                                        "[social] getBlockingStatus: {} blocked, {} blocked-by",
                                        resp.blocked_users.len(),
                                        resp.blocked_by_users.len()
                                    );
                                    let _ = response.send(Ok((resp.blocked_users, resp.blocked_by_users)));
                                }
                                Err(e) => {
                                    warn!("[social] getBlockingStatus error: {e:?}");
                                    let _ = response.send(Err(format!("{e:?}")));
                                }
                            }
                        }
                        SocialQuery::GetSocialSettings { response } => {
                            debug!("[social] getSocialSettings request");
                            let result = match service_module.get_social_settings().await {
                                Ok(resp) => match resp.response {
                                    Some(get_social_settings_response::Response::Ok(ok)) => {
                                        Ok(ok
                                            .settings
                                            .unwrap_or_default()
                                            .private_messages_privacy()
                                            .into())
                                    }
                                    other => Err(format!("{other:?}")),
                                },
                                Err(e) => Err(format!("{e:?}")),
                            };
                            if let Err(e) = &result {
                                warn!("[social] getSocialSettings error: {e}");
                            }
                            let _ = response.send(result);
                        }
                        SocialQuery::UpsertSocialSettings { privacy, response } => {
                            debug!("[social] upsertSocialSettings {privacy:?}");
                            let payload = UpsertSocialSettingsPayload {
                                private_messages_privacy: Some(PrivateMessagePrivacySetting::from(privacy) as i32),
                                ..Default::default()
                            };
                            let result = match service_module.upsert_social_settings(payload).await
                            {
                                Ok(resp) => match resp.response {
                                    Some(upsert_social_settings_response::Response::Ok(
                                        settings,
                                    )) => {
                                        Ok(settings.private_messages_privacy().into())
                                    }
                                    other => Err(format!("{other:?}")),
                                },
                                Err(e) => Err(format!("{e:?}")),
                            };
                            match &result {
                                Ok(privacy) => {
                                    let _ = response_sx.send(FriendData::OwnDmPrivacy(*privacy));
                                }
                                Err(e) => warn!("[social] upsertSocialSettings error: {e}"),
                            }
                            let _ = response.send(result);
                        }
                        SocialQuery::GetPrivateMessagesSettings {
                            addresses,
                            response,
                        } => {
                            debug!("[social] getPrivateMessagesSettings for {addresses:?}");
                            let payload = GetPrivateMessagesSettingsPayload {
                                user: addresses
                                    .into_iter()
                                    .map(|address| User { address })
                                    .collect(),
                            };
                            let result = match service_module
                                .get_private_messages_settings(payload)
                                .await
                            {
                                Ok(resp) => match resp.response {
                                    Some(get_private_messages_settings_response::Response::Ok(
                                        ok,
                                    )) => Ok(ok
                                        .settings
                                        .into_iter()
                                        .map(|s| DmPrivacyOf {
                                            privacy: s.private_messages_privacy().into(),
                                            address: s.user.unwrap_or_default().address,
                                        })
                                        .collect()),
                                    other => Err(format!("{other:?}")),
                                },
                                Err(e) => Err(format!("{e:?}")),
                            };
                            if let Err(e) = &result {
                                warn!("[social] getPrivateMessagesSettings error: {e}");
                            }
                            let _ = response.send(result);
                        }
                    }
                }
            }
        }
    }
    .fuse();

    // Inbound: receive friendship update events
    let sx_friendship = response_sx.clone();
    let f_service_read = async move {
        while let Some(update) = inbound_updates.next().await {
            debug!("[social] Received friendship update: {update:?}");
            if let Some(ev) = update.update {
                sx_friendship
                    .send(FriendData::FriendshipEvent(ev))
                    .map_err(dbgerr)?;
            }
        }
        Err(anyhow!("[social] friendship update stream ended"))
    }
    .fuse();

    // Inbound: receive friend connectivity update events
    let sx_connectivity = response_sx.clone();
    let f_connectivity_read = async move {
        while let Some(update) = connectivity_updates.next().await {
            debug!("[social] Received connectivity update: {update:?}");
            if let Some(friend) = &update.friend {
                if let Some(address) = friend.address.as_h160() {
                    let status = match update.status {
                        0 => ConnectivityStatus::Online,
                        2 => ConnectivityStatus::Away,
                        _ => ConnectivityStatus::Offline,
                    };
                    sx_connectivity
                        .send(FriendData::ConnectivityEvent { address, status })
                        .map_err(dbgerr)?;
                }
            }
        }
        Err(anyhow!("[social] connectivity update stream ended"))
    }
    .fuse();

    // Inbound: receive community member connectivity updates. `Away` is not a presence change
    // for community chat. A server-closed stream ends the connection, so every subscription is
    // made again on reconnect.
    let sx_community = response_sx.clone();
    let f_community_connectivity_read = async move {
        let Some(mut community_connectivity_updates) = community_connectivity_updates else {
            return std::future::pending::<Result<(), anyhow::Error>>().await;
        };
        while let Some(update) = community_connectivity_updates.next().await {
            debug!("[social] Received community connectivity update: {update:?}");
            if let Some(closed) = &update.stream_closed {
                return Err(anyhow!(
                    "[social] community connectivity stream closed by the server: {closed:?}"
                ));
            }
            let status = match update.status {
                0 => ConnectivityStatus::Online,
                1 => ConnectivityStatus::Offline,
                _ => continue,
            };
            let Some(address) = update
                .member
                .as_ref()
                .and_then(|member| member.address.as_h160())
            else {
                continue;
            };
            sx_community
                .send(FriendData::CommunityConnectivityEvent {
                    community_id: update.community_id,
                    address,
                    status,
                })
                .map_err(dbgerr)?;
        }
        Err(anyhow!(
            "[social] community connectivity update stream ended"
        ))
    }
    .fuse();

    // Inbound: receive block updates (someone blocked / unblocked the local user)
    let sx_block = response_sx.clone();
    let f_block_read = async move {
        while let Some(update) = block_updates.next().await {
            debug!("[social] Received block update: {update:?}");
            forward_block_update(
                block_snapshot_dirty,
                &sx_block,
                update.address,
                update.is_blocked,
            )?;
        }
        Err(anyhow!("[social] block update stream ended"))
    }
    .fuse();

    // Keepalive every 30s. Both proxies / load balancers and the server itself may
    // close idle connections; this also surfaces broken sockets promptly (the send
    // will fail rather than appearing healthy).
    //
    // On native we send a WS-level Ping. On wasm browsers don't expose ping frames,
    // so we issue a cheap unary RPC against our own address (semantically a no-op).
    // The wallet may be disconnected mid-session, so re-fetch the address each tick
    // and just skip if it's not currently available.
    #[cfg(target_arch = "wasm32")]
    let wallet_keepalive = wallet.clone();
    let f_keepalive = async move {
        loop {
            async_std::task::sleep(Duration::from_secs(30)).await;
            #[cfg(not(target_arch = "wasm32"))]
            {
                ws_for_ping
                    .send(Message::Ping)
                    .await
                    .map_err(|e| anyhow!("[social] keepalive ping failed: {e:?}"))?;
            }
            #[cfg(target_arch = "wasm32")]
            {
                let Some(addr) = wallet_keepalive.address() else {
                    continue;
                };
                service_module_keepalive
                    .get_friendship_status(GetFriendshipStatusPayload {
                        user: Some(User {
                            address: format!("{addr:#x}"),
                        }),
                    })
                    .await
                    .map_err(|e| anyhow!("[social] keepalive rpc failed: {e:?}"))?;
            }
        }
        #[allow(unreachable_code)]
        Result::<(), anyhow::Error>::Ok(())
    }
    .fuse();

    // Watch for the underlying websocket closing. dcl-rpc's dispatcher does not
    // surface transport closure to its per-subscription channels, so without this
    // the read futures would hang forever after a server-side disconnect.
    let f_ws_closed = async move {
        ws_closed.wait().await;
        Err(anyhow!("[social] websocket transport closed"))
    }
    .fuse();

    // Run until a stream breaks, the keepalive fails, or the websocket closes
    pin_mut!(
        f_service_read,
        f_service_write,
        f_connectivity_read,
        f_community_connectivity_read,
        f_block_read,
        f_keepalive,
        f_ws_closed
    );
    select! {
        r = f_service_read => r,
        r = f_service_write => r,
        r = f_connectivity_read => r,
        r = f_community_connectivity_read => r,
        r = f_block_read => r,
        r = f_keepalive => r,
        r = f_ws_closed => r,
    }
}

pub type FriendshipEventBody = friendship_update::Update;

#[cfg(test)]
mod block_snapshot_tests {
    use super::*;
    use crate::{update_social_client, SocialClient, SocialStateChanged};
    use bevy::prelude::*;
    use common::access_status::AccessStatus;

    fn app() -> (App, UnboundedSender<FriendData>, Address) {
        let mut wallet = wallet::Wallet::default();
        wallet.finalize_as_guest_with_seed([12; 32]);
        let owner = wallet.address().unwrap();
        let (events, friendship_receiver) = mpsc::unbounded_channel();
        let mut app = App::new();
        app.insert_resource(wallet)
            .insert_resource(SocialClient(Some(SocialClientHandler {
                sender: mpsc::unbounded_channel().0,
                query_sender: mpsc::unbounded_channel().0,
                friendship_receiver,
                owner,
                blocks_ready: false,
                is_initialized: false,
                sent_requests: Default::default(),
                received_requests: Default::default(),
                friends: Default::default(),
                friend_status: Default::default(),
                blocked: Default::default(),
                blocked_by: Default::default(),
                dm_privacy: Default::default(),
                unread_messages: Default::default(),
                friend_event_callback: Box::new(|_| {}),
                connectivity_callback: Box::new(|_, _| {}),
                block_update_callback: Box::new(|_, _| {}),
                community_connectivity_callback: Box::new(|_, _, _| {}),
                chat_event_callback: Box::new(|_| {}),
            })))
            .init_resource::<AccessStatus>()
            .add_event::<SocialStateChanged>()
            .add_systems(PostUpdate, update_social_client);
        (app, events, owner)
    }

    fn allows_voice(app: &App, owner: Address, peer: Address) -> bool {
        app.world()
            .resource::<AccessStatus>()
            .allows_voice_from(Some(owner), peer)
    }

    fn init(blocks: Option<BlockSnapshot>) -> FriendData {
        FriendData::Init {
            sent_requests: Default::default(),
            received_requests: Default::default(),
            friends: Default::default(),
            blocks,
            dm_privacy: Default::default(),
        }
    }

    fn partial_snapshot(blocked: Address, blocked_by: Address) -> Option<BlockSnapshot> {
        block_snapshot_from_response(Ok(GetBlockingStatusResponse {
            blocked_users: vec!["invalid".to_owned(), format!("{blocked:#x}")],
            blocked_by_users: vec![format!("{blocked_by:#x}"), "also-invalid".to_owned()],
        }))
    }

    fn assert_dm_blocks_and_no_voice(
        app: &App,
        owner: Address,
        blocked: Address,
        blocked_by: Address,
    ) {
        let handler = app.world().resource::<SocialClient>().0.as_ref().unwrap();
        assert!(handler.is_initialized);
        assert!(handler.blocked.contains(&blocked));
        assert!(handler.blocked_by.contains(&blocked_by));
        assert!(!app
            .world()
            .resource::<AccessStatus>()
            .blocks_ready_for(Some(owner)));
        assert!(!allows_voice(app, owner, Address::from([9; 20])));
    }

    #[test]
    fn incomplete_initial_snapshot_keeps_dm_blocks_but_voice_waits() {
        let (mut app, events, owner) = app();
        let blocked = Address::from([3; 20]);
        let blocked_by = Address::from([4; 20]);
        events
            .send(init(partial_snapshot(blocked, blocked_by)))
            .unwrap();
        app.update();
        app.update();

        assert_dm_blocks_and_no_voice(&app, owner, blocked, blocked_by);
    }

    #[test]
    fn incomplete_retry_applies_dm_blocks_and_complete_retry_enables_voice() {
        let (mut app, events, owner) = app();
        let blocked = Address::from([3; 20]);
        let blocked_by = Address::from([4; 20]);
        events.send(init(None)).unwrap();
        app.update();
        let dirty = AtomicBool::new(false);

        let completed = async_std::task::block_on(retry_block_snapshot(&dirty, &events, async {
            Ok(partial_snapshot(blocked, blocked_by))
        }))
        .unwrap();
        assert!(!completed);
        app.update();
        assert_dm_blocks_and_no_voice(&app, owner, blocked, blocked_by);

        let newer_block = Address::from([5; 20]);
        let completed = async_std::task::block_on(retry_block_snapshot(&dirty, &events, async {
            forward_block_update(&dirty, &events, format!("{newer_block:#x}"), true)?;
            Ok(partial_snapshot(blocked, blocked_by))
        }))
        .unwrap();
        assert!(!completed);
        app.update();
        assert!(app
            .world()
            .resource::<SocialClient>()
            .0
            .as_ref()
            .unwrap()
            .blocked_by
            .contains(&newer_block));

        events
            .send(FriendData::OwnUnblock { address: blocked })
            .unwrap();
        let completed = async_std::task::block_on(retry_block_snapshot(&dirty, &events, async {
            Ok(block_snapshot_from_response(Ok(
                GetBlockingStatusResponse {
                    blocked_users: vec![],
                    blocked_by_users: vec![
                        format!("{blocked_by:#x}"),
                        format!("{newer_block:#x}"),
                        format!("{newer_block:#x}"),
                    ],
                },
            )))
        }))
        .unwrap();
        assert!(completed);
        app.update();
        assert!(allows_voice(&app, owner, Address::from([9; 20])));
        assert!(allows_voice(&app, owner, blocked));
        assert!(!allows_voice(&app, owner, blocked_by));
        assert!(!allows_voice(&app, owner, newer_block));
    }

    #[test]
    fn failed_initial_snapshot_can_recover_but_disconnect_invalidates_voice() {
        let (mut app, events, owner) = app();
        let peer = Address::from([3; 20]);
        events.send(init(None)).unwrap();
        app.update();
        app.update();
        assert!(
            app.world()
                .resource::<SocialClient>()
                .0
                .as_ref()
                .unwrap()
                .is_initialized
        );
        assert!(!allows_voice(&app, owner, peer));

        let empty_snapshot = || BlockSnapshot {
            blocked: HashSet::new(),
            blocked_by: HashSet::new(),
            complete: true,
        };
        let dirty = AtomicBool::new(false);
        let retried = async_std::task::block_on(async {
            let (request_started, started) = tokio::sync::oneshot::channel();
            let (snapshot_reply, reply) = tokio::sync::oneshot::channel();
            let fetch_snapshot = async {
                request_started.send(()).unwrap();
                Ok(Some(reply.await.unwrap()))
            };
            let receive_block = async {
                started.await.unwrap();
                forward_block_update(&dirty, &events, format!("{peer:#x}"), true).unwrap();
                assert!(snapshot_reply.send(empty_snapshot()).is_ok());
            };
            let (retried, ()) = futures_util::join!(
                retry_block_snapshot(&dirty, &events, fetch_snapshot),
                receive_block,
            );
            retried.unwrap()
        });
        app.update();
        assert!(!allows_voice(&app, owner, peer));
        assert!(!retried, "the block update raced the snapshot reply");

        let blocked_snapshot = BlockSnapshot {
            blocked: HashSet::new(),
            blocked_by: [peer].into_iter().collect(),
            complete: true,
        };
        assert!(
            async_std::task::block_on(retry_block_snapshot(&dirty, &events, async {
                Ok(Some(blocked_snapshot))
            },))
            .unwrap()
        );
        app.update();
        assert!(app
            .world()
            .resource::<AccessStatus>()
            .blocks_ready_for(Some(owner)));
        assert!(!allows_voice(&app, owner, peer));

        events
            .send(FriendData::BlockUpdate {
                address: format!("{peer:#x}"),
                is_blocked: false,
            })
            .unwrap();
        app.update();
        assert!(allows_voice(&app, owner, peer));

        events.send(FriendData::Disconnected).unwrap();
        app.update();
        assert!(!allows_voice(&app, owner, peer));

        events.send(init(Some(empty_snapshot()))).unwrap();
        app.update();
        assert!(allows_voice(&app, owner, peer));

        drop(events);
        app.update();
        assert!(!allows_voice(&app, owner, peer));
    }
}

#[cfg(test)]
mod auth_tests {
    use super::*;

    #[test]
    fn social_signature_uses_source_root_for_mounted_and_standalone_connections() {
        async_std::task::block_on(async {
            let mut wallet = wallet::Wallet::default();
            wallet.finalize_as_guest();
            for url in [
                "wss://social.example/",
                "wss://social.example/social-rpc",
                "wss://social.example/edge/social-rpc?region=test",
            ] {
                let connection: http::Uri = url.parse().unwrap();
                let original = connection.clone();
                let headers: std::collections::HashMap<_, _> =
                    social_auth_headers(&wallet, &connection)
                        .await
                        .unwrap()
                        .into_iter()
                        .collect();
                assert_eq!(connection, original, "connection URI remains unchanged");
                let entity: serde_json::Value =
                    serde_json::from_str(&headers["x-identity-auth-chain-1"]).unwrap();
                let expected = format!("get:/:{}:{{}}", headers["x-identity-timestamp"]);
                assert_eq!(entity["payload"], expected);
                let signature: alloy_core::primitives::Signature =
                    entity["signature"].as_str().unwrap().parse().unwrap();
                assert_eq!(
                    signature
                        .recover_address_from_msg(expected.as_bytes())
                        .unwrap(),
                    wallet.address().unwrap()
                );
            }
        });
    }
}
