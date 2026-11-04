//! Voice chat signalling over the social service: the private-call and community-voice
//! procedures of `SocialService` and the two update subscriptions, as plain types the engine's
//! voice module consumes without the proto. The server side of every procedure is the catalyst's
//! social RPC (`services/catalyst/social/src/rpc/service/{private_voice,community_voice}.rs`),
//! which mirrors upstream; the client behaviour follows unity-explorer's
//! `Assets/DCL/VoiceChat/Services/RPC{Private,Community}VoiceChatService.cs`.

use tokio::sync::oneshot;

/// Why a voice procedure was refused, by the server's `oneof` or by the transport.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum VoiceRefusal {
    /// `InvalidRequest`
    Invalid(String),
    /// `ConflictingError`: busy, already in a call, a room already live
    Conflict(String),
    /// `ForbiddenError`: privacy, blocks, not a moderator, a guest
    Forbidden(String),
    /// `NotFoundError`: the call or room is gone
    NotFound(String),
    /// `InternalServerError`, or an empty response
    Internal(String),
    /// The RPC never reached the server, timed out, or the social client is not connected.
    Transport(String),
}

impl VoiceRefusal {
    pub fn message(&self) -> &str {
        match self {
            VoiceRefusal::Invalid(m)
            | VoiceRefusal::Conflict(m)
            | VoiceRefusal::Forbidden(m)
            | VoiceRefusal::NotFound(m)
            | VoiceRefusal::Internal(m)
            | VoiceRefusal::Transport(m) => m,
        }
    }
}

impl std::fmt::Display for VoiceRefusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let kind = match self {
            VoiceRefusal::Invalid(_) => "invalid request",
            VoiceRefusal::Conflict(_) => "conflict",
            VoiceRefusal::Forbidden(_) => "forbidden",
            VoiceRefusal::NotFound(_) => "not found",
            VoiceRefusal::Internal(_) => "internal error",
            VoiceRefusal::Transport(_) => "transport",
        };
        write!(f, "{kind}: {}", self.message())
    }
}

pub type VoiceReply<T> = oneshot::Sender<Result<T, VoiceRefusal>>;

/// An incoming private call the server holds for the local user.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IncomingPrivateCall {
    pub call_id: String,
    pub caller: String,
}

/// One voice procedure and where its answer goes.
#[derive(Debug)]
pub enum VoiceQuery {
    /// `StartPrivateVoiceChat`: answers the call id.
    StartPrivate {
        callee: String,
        response: VoiceReply<String>,
    },
    /// `AcceptPrivateVoiceChat`: answers the callee's LiveKit connection url.
    AcceptPrivate {
        call_id: String,
        response: VoiceReply<String>,
    },
    RejectPrivate {
        call_id: String,
        response: VoiceReply<()>,
    },
    EndPrivate {
        call_id: String,
        response: VoiceReply<()>,
    },
    /// `GetIncomingPrivateVoiceChatRequest`: `None` when nobody is calling.
    GetIncomingPrivate {
        response: VoiceReply<Option<IncomingPrivateCall>>,
    },
    /// `StartCommunityVoiceChat` (owner or moderator): answers the connection url.
    StartCommunity {
        community_id: String,
        response: VoiceReply<String>,
    },
    /// `JoinCommunityVoiceChat`: answers the connection url.
    JoinCommunity {
        community_id: String,
        response: VoiceReply<String>,
    },
    /// `RequestToSpeakInCommunityVoiceChat`: raise (`true`) or lower the hand.
    RequestToSpeak {
        community_id: String,
        raising_hand: bool,
        response: VoiceReply<()>,
    },
    PromoteSpeaker {
        community_id: String,
        address: String,
        response: VoiceReply<()>,
    },
    DemoteSpeaker {
        community_id: String,
        address: String,
        response: VoiceReply<()>,
    },
    KickPlayer {
        community_id: String,
        address: String,
        response: VoiceReply<()>,
    },
    RejectSpeakRequest {
        community_id: String,
        address: String,
        response: VoiceReply<()>,
    },
    /// `MuteSpeakerFromCommunityVoiceChat`: answers the resulting mute state.
    MuteSpeaker {
        community_id: String,
        address: String,
        muted: bool,
        response: VoiceReply<bool>,
    },
    EndCommunity {
        community_id: String,
        response: VoiceReply<()>,
    },
}

/// `PrivateVoiceChatStatus`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PrivateVoiceStatus {
    Requested,
    Accepted,
    Rejected,
    Ended,
    Expired,
}

/// A `PrivateVoiceChatUpdate`: the server sends the caller only on REQUESTED and the
/// credentials only on ACCEPTED (to the caller).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PrivateVoiceUpdate {
    pub call_id: String,
    pub status: PrivateVoiceStatus,
    pub caller: Option<String>,
    pub callee: Option<String>,
    pub connection_url: Option<String>,
}

/// `CommunityVoiceChatStatus`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CommunityVoiceStatus {
    Started,
    Ended,
}

/// A `CommunityVoiceChatUpdate`: a community room opened or closed. `positions` (parcels) and
/// `worlds` are the community's places, sent to non-members so the client can offer the
/// stream happening where they stand.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommunityVoiceUpdate {
    pub community_id: String,
    pub status: CommunityVoiceStatus,
    pub created_at: i64,
    pub ended_at: Option<i64>,
    pub positions: Vec<String>,
    pub worlds: Vec<String>,
    pub is_member: bool,
    pub community_name: String,
    pub community_image: Option<String>,
}

/// What the social connection tells the voice module.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum VoiceEvent {
    /// The voice subscriptions are open on a (re)connected social link.
    Connected,
    Private(PrivateVoiceUpdate),
    Community(CommunityVoiceUpdate),
    /// The social link dropped; the subscriptions are gone until `Connected`.
    Disconnected,
}

#[cfg(feature = "social")]
pub(crate) mod rpc {
    use super::*;
    use dcl_component::proto_components::social_service::v2::{
        accept_private_voice_chat_response, demote_speaker_in_community_voice_chat_response,
        end_community_voice_chat_response, end_private_voice_chat_response,
        get_incoming_private_voice_chat_request_response, join_community_voice_chat_response,
        kick_player_from_community_voice_chat_response,
        mute_speaker_from_community_voice_chat_response,
        promote_speaker_in_community_voice_chat_response,
        reject_private_voice_chat_response,
        reject_speak_request_in_community_voice_chat_response,
        request_to_speak_in_community_voice_chat_response, start_community_voice_chat_response,
        start_private_voice_chat_response, AcceptPrivateVoiceChatPayload,
        CommunityVoiceChatStatus as ProtoCommunityStatus, CommunityVoiceChatUpdate,
        DemoteSpeakerInCommunityVoiceChatPayload, EndCommunityVoiceChatPayload,
        EndPrivateVoiceChatPayload, JoinCommunityVoiceChatPayload,
        KickPlayerFromCommunityVoiceChatPayload, MuteSpeakerFromCommunityVoiceChatPayload,
        PrivateVoiceChatStatus as ProtoPrivateStatus, PrivateVoiceChatUpdate,
        PromoteSpeakerInCommunityVoiceChatPayload, RejectPrivateVoiceChatPayload,
        RejectSpeakRequestInCommunityVoiceChatPayload,
        RequestToSpeakInCommunityVoiceChatPayload, SocialServiceClient,
        SocialServiceClientDefinition, StartCommunityVoiceChatPayload,
        StartPrivateVoiceChatPayload, User,
    };
    use dcl_rpc::transports::Transport;
    use web_time::Duration;

    /// Unity's `RPCSocialServices` unary timeout.
    const CALL_TIMEOUT: Duration = Duration::from_secs(10);

    /// Maps a response `oneof` to the plain result: `Ok` through the given expression, each
    /// error variant to its refusal, an empty response to an internal error.
    macro_rules! answer {
        ($resp:expr, $module:ident, |$ok:ident| $body:expr, [$($variant:ident => $kind:ident),* $(,)?]) => {
            match $resp.response {
                Some($module::Response::Ok($ok)) => Ok($body),
                $(Some($module::Response::$variant(e)) => {
                    Err(VoiceRefusal::$kind(e.message.unwrap_or_default()))
                })*
                None => Err(VoiceRefusal::Internal("empty response".to_owned())),
            }
        };
    }

    fn transport<E: std::fmt::Debug>(e: E) -> VoiceRefusal {
        VoiceRefusal::Transport(format!("{e:?}"))
    }

    async fn call<F, T>(future: F) -> Result<T, VoiceRefusal>
    where
        F: std::future::Future<Output = Result<T, VoiceRefusal>>,
    {
        match async_std::future::timeout(CALL_TIMEOUT, future).await {
            Ok(result) => result,
            Err(_) => Err(VoiceRefusal::Transport("timed out".to_owned())),
        }
    }

    fn user(address: &str) -> Option<User> {
        Some(User {
            address: address.to_owned(),
        })
    }

    fn url_of(credentials: Option<impl Into<String>>, what: &str) -> Result<String, VoiceRefusal> {
        let url = credentials.map(Into::into).unwrap_or_default();
        if url.is_empty() {
            Err(VoiceRefusal::Internal(format!(
                "{what}: no credentials in the response"
            )))
        } else {
            Ok(url)
        }
    }

    /// Runs one query against the service and answers it. Replies nobody waits for are dropped.
    pub(crate) async fn dispatch<T: Transport + 'static>(
        service: &SocialServiceClient<T>,
        query: VoiceQuery,
    ) {
        match query {
            VoiceQuery::StartPrivate { callee, response } => {
                let result = call(async {
                    let resp = service
                        .start_private_voice_chat(StartPrivateVoiceChatPayload {
                            callee: user(&callee),
                        })
                        .await
                        .map_err(transport)?;
                    answer!(resp, start_private_voice_chat_response, |ok| ok.call_id, [
                        InternalServerError => Internal,
                        InvalidRequest => Invalid,
                        ConflictingError => Conflict,
                        ForbiddenError => Forbidden,
                    ])
                })
                .await;
                let _ = response.send(result);
            }
            VoiceQuery::AcceptPrivate { call_id, response } => {
                let result = call(async {
                    let resp = service
                        .accept_private_voice_chat(AcceptPrivateVoiceChatPayload { call_id })
                        .await
                        .map_err(transport)?;
                    answer!(resp, accept_private_voice_chat_response, |ok| ok.credentials.map(|c| c.connection_url), [
                        InternalServerError => Internal,
                        InvalidRequest => Invalid,
                        NotFound => NotFound,
                        ForbiddenError => Forbidden,
                    ])
                    .and_then(|credentials| url_of(credentials, "accept"))
                })
                .await;
                let _ = response.send(result);
            }
            VoiceQuery::RejectPrivate { call_id, response } => {
                let result = call(async {
                    let resp = service
                        .reject_private_voice_chat(RejectPrivateVoiceChatPayload { call_id })
                        .await
                        .map_err(transport)?;
                    answer!(resp, reject_private_voice_chat_response, |_ok| (), [
                        InternalServerError => Internal,
                        InvalidRequest => Invalid,
                        NotFound => NotFound,
                    ])
                })
                .await;
                let _ = response.send(result);
            }
            VoiceQuery::EndPrivate { call_id, response } => {
                let result = call(async {
                    let resp = service
                        .end_private_voice_chat(EndPrivateVoiceChatPayload { call_id })
                        .await
                        .map_err(transport)?;
                    answer!(resp, end_private_voice_chat_response, |_ok| (), [
                        InternalServerError => Internal,
                        NotFound => NotFound,
                    ])
                })
                .await;
                let _ = response.send(result);
            }
            VoiceQuery::GetIncomingPrivate { response } => {
                let result = call(async {
                    let resp = service
                        .get_incoming_private_voice_chat_request()
                        .await
                        .map_err(transport)?;
                    match resp.response {
                        Some(get_incoming_private_voice_chat_request_response::Response::Ok(ok)) => {
                            Ok(Some(IncomingPrivateCall {
                                call_id: ok.call_id,
                                caller: ok.caller.map(|u| u.address).unwrap_or_default(),
                            }))
                        }
                        Some(get_incoming_private_voice_chat_request_response::Response::NotFound(_)) => {
                            Ok(None)
                        }
                        Some(get_incoming_private_voice_chat_request_response::Response::InternalServerError(e)) => {
                            Err(VoiceRefusal::Internal(e.message.unwrap_or_default()))
                        }
                        None => Err(VoiceRefusal::Internal("empty response".to_owned())),
                    }
                })
                .await;
                let _ = response.send(result);
            }
            VoiceQuery::StartCommunity {
                community_id,
                response,
            } => {
                let result = call(async {
                    let resp = service
                        .start_community_voice_chat(StartCommunityVoiceChatPayload { community_id })
                        .await
                        .map_err(transport)?;
                    answer!(resp, start_community_voice_chat_response, |ok| ok.credentials.map(|c| c.connection_url), [
                        InvalidRequest => Invalid,
                        ForbiddenError => Forbidden,
                        ConflictingError => Conflict,
                        InternalServerError => Internal,
                    ])
                    .and_then(|credentials| url_of(credentials, "start community voice chat"))
                })
                .await;
                let _ = response.send(result);
            }
            VoiceQuery::JoinCommunity {
                community_id,
                response,
            } => {
                let result = call(async {
                    let resp = service
                        .join_community_voice_chat(JoinCommunityVoiceChatPayload { community_id })
                        .await
                        .map_err(transport)?;
                    answer!(resp, join_community_voice_chat_response, |ok| ok.credentials.map(|c| c.connection_url), [
                        InvalidRequest => Invalid,
                        ForbiddenError => Forbidden,
                        NotFoundError => NotFound,
                        ConflictingError => Conflict,
                        InternalServerError => Internal,
                    ])
                    .and_then(|credentials| url_of(credentials, "join community voice chat"))
                })
                .await;
                let _ = response.send(result);
            }
            VoiceQuery::RequestToSpeak {
                community_id,
                raising_hand,
                response,
            } => {
                let result = call(async {
                    let resp = service
                        .request_to_speak_in_community_voice_chat(
                            RequestToSpeakInCommunityVoiceChatPayload {
                                community_id,
                                is_raising_hand: raising_hand,
                            },
                        )
                        .await
                        .map_err(transport)?;
                    answer!(resp, request_to_speak_in_community_voice_chat_response, |_ok| (), [
                        InvalidRequest => Invalid,
                        ForbiddenError => Forbidden,
                        NotFoundError => NotFound,
                        InternalServerError => Internal,
                    ])
                })
                .await;
                let _ = response.send(result);
            }
            VoiceQuery::PromoteSpeaker {
                community_id,
                address,
                response,
            } => {
                let result = call(async {
                    let resp = service
                        .promote_speaker_in_community_voice_chat(
                            PromoteSpeakerInCommunityVoiceChatPayload {
                                community_id,
                                user_address: address,
                            },
                        )
                        .await
                        .map_err(transport)?;
                    answer!(resp, promote_speaker_in_community_voice_chat_response, |_ok| (), [
                        InvalidRequest => Invalid,
                        ForbiddenError => Forbidden,
                        NotFoundError => NotFound,
                        InternalServerError => Internal,
                    ])
                })
                .await;
                let _ = response.send(result);
            }
            VoiceQuery::DemoteSpeaker {
                community_id,
                address,
                response,
            } => {
                let result = call(async {
                    let resp = service
                        .demote_speaker_in_community_voice_chat(
                            DemoteSpeakerInCommunityVoiceChatPayload {
                                community_id,
                                user_address: address,
                            },
                        )
                        .await
                        .map_err(transport)?;
                    answer!(resp, demote_speaker_in_community_voice_chat_response, |_ok| (), [
                        InvalidRequest => Invalid,
                        ForbiddenError => Forbidden,
                        NotFoundError => NotFound,
                        InternalServerError => Internal,
                    ])
                })
                .await;
                let _ = response.send(result);
            }
            VoiceQuery::KickPlayer {
                community_id,
                address,
                response,
            } => {
                let result = call(async {
                    let resp = service
                        .kick_player_from_community_voice_chat(
                            KickPlayerFromCommunityVoiceChatPayload {
                                community_id,
                                user_address: address,
                            },
                        )
                        .await
                        .map_err(transport)?;
                    answer!(resp, kick_player_from_community_voice_chat_response, |_ok| (), [
                        InvalidRequest => Invalid,
                        ForbiddenError => Forbidden,
                        NotFoundError => NotFound,
                        InternalServerError => Internal,
                    ])
                })
                .await;
                let _ = response.send(result);
            }
            VoiceQuery::RejectSpeakRequest {
                community_id,
                address,
                response,
            } => {
                let result = call(async {
                    let resp = service
                        .reject_speak_request_in_community_voice_chat(
                            RejectSpeakRequestInCommunityVoiceChatPayload {
                                community_id,
                                user_address: address,
                            },
                        )
                        .await
                        .map_err(transport)?;
                    answer!(resp, reject_speak_request_in_community_voice_chat_response, |_ok| (), [
                        InvalidRequest => Invalid,
                        ForbiddenError => Forbidden,
                        NotFoundError => NotFound,
                        InternalServerError => Internal,
                    ])
                })
                .await;
                let _ = response.send(result);
            }
            VoiceQuery::MuteSpeaker {
                community_id,
                address,
                muted,
                response,
            } => {
                let result = call(async {
                    let resp = service
                        .mute_speaker_from_community_voice_chat(
                            MuteSpeakerFromCommunityVoiceChatPayload {
                                community_id,
                                user_address: address,
                                muted,
                            },
                        )
                        .await
                        .map_err(transport)?;
                    answer!(resp, mute_speaker_from_community_voice_chat_response, |ok| ok.muted, [
                        ForbiddenError => Forbidden,
                        NotFoundError => NotFound,
                        InvalidRequest => Invalid,
                        InternalServerError => Internal,
                    ])
                })
                .await;
                let _ = response.send(result);
            }
            VoiceQuery::EndCommunity {
                community_id,
                response,
            } => {
                let result = call(async {
                    let resp = service
                        .end_community_voice_chat(EndCommunityVoiceChatPayload { community_id })
                        .await
                        .map_err(transport)?;
                    answer!(resp, end_community_voice_chat_response, |_ok| (), [
                        InvalidRequest => Invalid,
                        ForbiddenError => Forbidden,
                        NotFoundError => NotFound,
                        InternalServerError => Internal,
                    ])
                })
                .await;
                let _ = response.send(result);
            }
        }
    }

    /// `None` for the stream's closing message, which carries no update.
    pub(crate) fn private_update(update: PrivateVoiceChatUpdate) -> Option<PrivateVoiceUpdate> {
        if update.stream_closed.is_some() {
            return None;
        }
        let status = match ProtoPrivateStatus::from_i32(update.status)? {
            ProtoPrivateStatus::VoiceChatRequested => PrivateVoiceStatus::Requested,
            ProtoPrivateStatus::VoiceChatAccepted => PrivateVoiceStatus::Accepted,
            ProtoPrivateStatus::VoiceChatRejected => PrivateVoiceStatus::Rejected,
            ProtoPrivateStatus::VoiceChatEnded => PrivateVoiceStatus::Ended,
            ProtoPrivateStatus::VoiceChatExpired => PrivateVoiceStatus::Expired,
        };
        Some(PrivateVoiceUpdate {
            call_id: update.call_id,
            status,
            caller: update.caller.map(|u| u.address),
            callee: update.callee.map(|u| u.address),
            connection_url: update
                .credentials
                .map(|c| c.connection_url)
                .filter(|url| !url.is_empty()),
        })
    }

    /// `None` for the stream's closing message, which carries no update.
    pub(crate) fn community_update(
        update: CommunityVoiceChatUpdate,
    ) -> Option<CommunityVoiceUpdate> {
        if update.stream_closed.is_some() {
            return None;
        }
        let status = match ProtoCommunityStatus::from_i32(update.status)? {
            ProtoCommunityStatus::CommunityVoiceChatStarted => CommunityVoiceStatus::Started,
            ProtoCommunityStatus::CommunityVoiceChatEnded => CommunityVoiceStatus::Ended,
        };
        Some(CommunityVoiceUpdate {
            community_id: update.community_id,
            status,
            created_at: update.created_at,
            ended_at: update.ended_at,
            positions: update.positions,
            worlds: update.worlds,
            is_member: update.is_member,
            community_name: update.community_name,
            community_image: update.community_image.filter(|i| !i.is_empty()),
        })
    }
}
