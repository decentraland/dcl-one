use super::SocialServiceImpl;
use super::helpers::{
    Refusal, SocialError, internal_err, normalize, start_voice_conflict, start_voice_forbidden,
    start_voice_internal, start_voice_invalid,
};
use crate::comms::gatekeeper::voice::db::PendingPrivateCall;
use crate::comms::gatekeeper::voice::rooms::VoiceUnavailable;
use crate::rpc::context::Context;
use crate::rpc::drpc::service_module_definition::ProcedureContext;
use crate::rpc::proto::errors::{ForbiddenError, InternalServerError, NotFoundError};
use crate::rpc::proto::v2::*;
use crate::rpc::pubsub::SocialEvent;
use uuid::Uuid;

const VOICE_RUNTIME_UNAVAILABLE: &str = "voice runtime unavailable";
/// Upstream `VoiceChatNotAllowedError`.
const VOICE_CHAT_NOT_ALLOWED: &str =
    "The callee or the caller are not accepting voice calls from users that are not friends";
const BUSY: &str = "One of the users is busy calling someone else";

/// The source passes callId to PostgreSQL before privileged identifier checks.
/// PostgreSQL accepts more UUID spellings than the media adapter and its syntax
/// errors are application oneofs, including their original message.
async fn database_call_id(ctx: &Context, raw: &str) -> Result<Uuid, Refusal> {
    sqlx::query_scalar("SELECT $1::text::uuid")
        .bind(raw)
        .fetch_one(ctx.db().pool())
        .await
        .map_err(|error| {
            Refusal::Internal(error.as_database_error().map_or_else(
                || error.to_string(),
                |database| database.message().to_string(),
            ))
        })
}
fn canonical_call_id(raw: &str, id: Uuid) -> bool {
    raw.eq_ignore_ascii_case(&id.to_string())
}

/// Upstream `VoiceChatNotFoundError`.
pub(crate) fn call_not_found(id: impl std::fmt::Display) -> Refusal {
    Refusal::NotFound(format!("The voice chat with id {id} was not found"))
}

/// Upstream `IncomingVoiceChatNotFoundError`.
pub(crate) fn incoming_not_found(address: &str) -> String {
    format!("The incoming voice chat for the address {address} was not found")
}

/// The gates run in the old precedence, all from one read: self-call,
/// block, the privacy settings (`all` on both sides or friends), either
/// party in a community room, busy, either party in a private room; then the
/// locked insert, and the callee is told REQUESTED. The runtime availability check follows the block and privacy gates.
/// Shared by the procedure and `POST /v1/private-voice-chats`.
pub(crate) async fn start_private_voice_chat(
    ctx: &Context,
    me: &str,
    callee: &str,
) -> Result<Uuid, Refusal> {
    // Upstream `startPrivateVoiceChat` (`src/logic/voice/voice.ts:36-58`): a
    // self-call, a blocked pair and the privacy refusal are all
    // `VoiceChatNotAllowedError`.
    if callee == me {
        return Err(Refusal::Forbidden(VOICE_CHAT_NOT_ALLOWED.into()));
    }
    let expiration_ms = ctx.voice_settings().private_voice_chat_expiration_ms;
    let pre = ctx
        .db()
        .private_voice_preflight(me, callee, expiration_ms, ctx.voice().presence_config())
        .await?;
    if pre.blocked {
        return Err(Refusal::Forbidden(VOICE_CHAT_NOT_ALLOWED.into()));
    }
    if (pre.callee_privacy != "all" || pre.caller_privacy != "all") && !pre.friends.unwrap_or(false)
    {
        return Err(Refusal::Forbidden(VOICE_CHAT_NOT_ALLOWED.into()));
    }
    if !ctx.voice().is_configured() {
        return Err(Refusal::Internal(VOICE_RUNTIME_UNAVAILABLE.into()));
    }
    // Community-call presence is disclosed only once block and privacy allow the call.
    if pre.caller_in_community_voice {
        return Err(Refusal::Conflict(
            "Cannot start private voice chat while in a community voice chat".into(),
        ));
    }
    if pre.callee_in_community_voice {
        return Err(Refusal::Conflict(
            "Cannot start private voice chat: the callee is in a community voice chat".into(),
        ));
    }
    if pre.busy {
        return Err(Refusal::Conflict(BUSY.into()));
    }
    for (in_voice, address) in [(pre.callee_in_voice, callee), (pre.caller_in_voice, me)] {
        if in_voice {
            return Err(Refusal::Conflict(format!(
                "One of the users is already in a voice chat: {address}"
            )));
        }
    }

    let outcome = ctx
        .voice()
        .create_pending_private_call(me, callee, expiration_ms)
        .await
        .map_err(|e| Refusal::Internal(e.to_string()))?;
    if outcome.started.is_some() {
        ctx.db()
            .note_private_voice_due(chrono::Utc::now().timestamp_millis() + expiration_ms);
    }
    for expired in &outcome.expired {
        ctx.publish_private_voice(expired, PrivateVoiceChatStatus::VoiceChatExpired)
            .await;
    }
    let Some(id) = outcome.started else {
        return Err(Refusal::Conflict(BUSY.into()));
    };
    ctx.pubsub()
        .publish_mutation(
            callee,
            SocialEvent::PrivateVoice(PrivateVoiceChatUpdate {
                stream_closed: None,
                call_id: id.to_string(),
                status: PrivateVoiceChatStatus::VoiceChatRequested as i32,
                caller: Some(User {
                    address: me.to_string(),
                }),
                callee: None,
                credentials: None,
            }),
        )
        .await;
    Ok(id)
}

/// Only the callee accepts; the grants come from the voice runtime (a
/// missing one is an explicit internal error, never an empty URL) and the
/// caller receives ACCEPTED with their own grant. Answers the callee's URL.
pub(crate) async fn accept_private_voice_chat(
    ctx: &Context,
    me: &str,
    id: Uuid,
) -> Result<String, Refusal> {
    accept_private_voice_chat_with_id(ctx, me, id, &id.to_string()).await
}
async fn accept_private_voice_chat_with_id(
    ctx: &Context,
    me: &str,
    id: Uuid,
    wire_id: &str,
) -> Result<String, Refusal> {
    let Some(chat) = ctx.db().get_private_voice_chat(id).await? else {
        return Err(call_not_found(wire_id));
    };
    if chat.callee_address != me {
        return Err(Refusal::Forbidden(VOICE_CHAT_NOT_ALLOWED.into()));
    }
    if !canonical_call_id(wire_id, id) {
        return Err(Refusal::Internal(
            "Invalid Gatekeeper private voice room ID".into(),
        ));
    }
    let urls = ctx
        .voice()
        .private_voice_credentials(id, &chat.callee_address, &chat.caller_address)
        .await
        .map_err(|e| match e {
            VoiceUnavailable::NotFound => call_not_found(wire_id),
            other => Refusal::Internal(format!("private voice credentials: {other}")),
        })?;
    let url_of = |address: &str| {
        urls.get(&address.to_lowercase()).cloned().ok_or_else(|| {
            Refusal::Internal(
                "Cannot read properties of undefined (reading 'connectionUrl')".into(),
            )
        })
    };
    ctx.pubsub()
        .publish_mutation(
            &chat.caller_address,
            SocialEvent::PrivateVoice(PrivateVoiceChatUpdate {
                stream_closed: None,
                call_id: wire_id.to_string(),
                status: PrivateVoiceChatStatus::VoiceChatAccepted as i32,
                caller: None,
                callee: None,
                credentials: Some(PrivateVoiceChatCredentials {
                    connection_url: url_of(&chat.caller_address)?,
                }),
            }),
        )
        .await;
    // Upstream reads the callee's grant after notifying the caller. Preserve
    // that already committed event if only the callee's credential is missing.
    url_of(&chat.callee_address)
}

/// Deletes the pending call and tells the caller. Only the callee rejects,
/// as upstream (`voice.ts:156-157`); anyone else, and an accepted call, get
/// the unknown call's not-found and nothing changes.
pub(crate) async fn reject_private_voice_chat(
    ctx: &Context,
    me: &str,
    id: Uuid,
) -> Result<(), Refusal> {
    reject_private_voice_chat_with_id(ctx, me, id, &id.to_string()).await
}
async fn reject_private_voice_chat_with_id(
    ctx: &Context,
    me: &str,
    id: Uuid,
    wire_id: &str,
) -> Result<(), Refusal> {
    let call = PendingPrivateCall::Rejected {
        id,
        callee: me.to_string(),
    };
    let Some(chat) = ctx.voice().finish_pending_private_call(call).await? else {
        return Err(call_not_found(wire_id));
    };
    ctx.pubsub()
        .publish_mutation(
            &chat.caller_address,
            SocialEvent::PrivateVoice(PrivateVoiceChatUpdate {
                stream_closed: None,
                call_id: wire_id.to_string(),
                status: PrivateVoiceChatStatus::VoiceChatRejected as i32,
                caller: None,
                callee: None,
                credentials: None,
            }),
        )
        .await;
    Ok(())
}

/// Ends the call and tells both parties. Only a party ends it, as upstream
/// (`voice.ts:189-191`); anyone else gets the unknown call's not-found and
/// nothing changes. An accepted call ends through the voice runtime, which
/// deletes the row and then the room; a pending one has no room.
pub(crate) async fn end_private_voice_chat(
    ctx: &Context,
    me: &str,
    id: Uuid,
) -> Result<(), Refusal> {
    end_private_voice_chat_with_id(ctx, me, id, &id.to_string()).await
}
async fn end_private_voice_chat_with_id(
    ctx: &Context,
    me: &str,
    id: Uuid,
    wire_id: &str,
) -> Result<(), Refusal> {
    if !canonical_call_id(wire_id, id) {
        match ctx.db().get_private_voice_chat(id).await? {
            Some(chat) if chat.caller_address == me || chat.callee_address == me => {}
            Some(_) => return Err(call_not_found(wire_id)),
            None => {
                return Err(Refusal::Internal(
                    "Invalid Gatekeeper private voice call ID".into(),
                ));
            }
        }
    }

    let chat = match ctx.voice().end_private_voice_chat(Some(id), me).await? {
        Some(chat) => chat,
        None => {
            let pending = PendingPrivateCall::Ended {
                id,
                party: me.to_string(),
            };
            ctx.voice()
                .finish_pending_private_call(pending)
                .await?
                .ok_or_else(|| call_not_found(wire_id))?
        }
    };
    let other = if chat.caller_address.eq_ignore_ascii_case(me) {
        &chat.callee_address
    } else {
        &chat.caller_address
    };
    ctx.pubsub()
        .publish_mutation(
            other,
            SocialEvent::PrivateVoice(PrivateVoiceChatUpdate {
                stream_closed: None,
                call_id: wire_id.to_string(),
                status: PrivateVoiceChatStatus::VoiceChatEnded as i32,
                caller: None,
                callee: None,
                credentials: None,
            }),
        )
        .await;
    Ok(())
}

impl SocialServiceImpl {
    /// The gates run in the old precedence, all from one read: self-call,
    /// block, the privacy settings (`all` on both sides or friends), either
    /// party in a community room, busy, either party in a private
    /// room; then the locked insert. With no voice runtime the call is refused
    /// up front rather than rung with nothing to connect to.
    pub(crate) async fn start_private_voice_chat(
        &self,
        request: StartPrivateVoiceChatPayload,
        context: ProcedureContext<Context>,
    ) -> Result<StartPrivateVoiceChatResponse, SocialError> {
        let me = Self::caller(&context)?;
        let callee = match request.callee.as_ref() {
            Some(u) if !u.address.trim().is_empty() => normalize(&u.address),
            _ => {
                return Ok(start_voice_invalid(
                    "Callee address is missing in the request payload",
                ));
            }
        };
        match start_private_voice_chat(&context.server_context, &me, &callee).await {
            Ok(id) => Ok(StartPrivateVoiceChatResponse {
                response: Some(start_private_voice_chat_response::Response::Ok(
                    start_private_voice_chat_response::Ok {
                        call_id: id.to_string(),
                    },
                )),
            }),
            Err(Refusal::Forbidden(m)) => Ok(start_voice_forbidden(m)),
            Err(Refusal::Conflict(m)) => Ok(start_voice_conflict(m)),
            Err(Refusal::Invalid(m)) => Ok(start_voice_invalid(m)),
            Err(refusal) => Ok(start_voice_internal(refusal.message())),
        }
    }

    /// Only the callee accepts; the grants come from the voice runtime (a
    /// missing one is an explicit internal error, never an empty URL) and the
    /// caller receives ACCEPTED with their own grant.
    pub(crate) async fn accept_private_voice_chat(
        &self,
        request: AcceptPrivateVoiceChatPayload,
        context: ProcedureContext<Context>,
    ) -> Result<AcceptPrivateVoiceChatResponse, SocialError> {
        let me = Self::caller(&context)?;
        let id = match database_call_id(&context.server_context, &request.call_id).await {
            Ok(id) => id,
            Err(refusal) => {
                return Ok(AcceptPrivateVoiceChatResponse {
                    response: Some(
                        accept_private_voice_chat_response::Response::InternalServerError(
                            internal_err(refusal.message()),
                        ),
                    ),
                });
            }
        };
        let response = match accept_private_voice_chat_with_id(
            &context.server_context,
            &me,
            id,
            &request.call_id,
        )
        .await
        {
            Ok(connection_url) => accept_private_voice_chat_response::Response::Ok(
                accept_private_voice_chat_response::Ok {
                    call_id: request.call_id,
                    credentials: Some(PrivateVoiceChatCredentials { connection_url }),
                },
            ),
            Err(Refusal::NotFound(m)) => {
                accept_private_voice_chat_response::Response::NotFound(NotFoundError {
                    message: Some(m),
                })
            }
            Err(Refusal::Forbidden(m)) => {
                accept_private_voice_chat_response::Response::ForbiddenError(ForbiddenError {
                    message: Some(m),
                })
            }
            Err(refusal) => accept_private_voice_chat_response::Response::InternalServerError(
                internal_err(refusal.message()),
            ),
        };
        Ok(AcceptPrivateVoiceChatResponse {
            response: Some(response),
        })
    }

    /// Deletes the pending call and tells the caller. Only the callee rejects,
    /// as upstream (`voice.ts:156-157`); anyone else, and an accepted call,
    /// get the unknown call's not-found and nothing changes.
    pub(crate) async fn reject_private_voice_chat(
        &self,
        request: RejectPrivateVoiceChatPayload,
        context: ProcedureContext<Context>,
    ) -> Result<RejectPrivateVoiceChatResponse, SocialError> {
        let me = Self::caller(&context)?;
        let id = match database_call_id(&context.server_context, &request.call_id).await {
            Ok(id) => id,
            Err(refusal) => {
                return Ok(RejectPrivateVoiceChatResponse {
                    response: Some(
                        reject_private_voice_chat_response::Response::InternalServerError(
                            internal_err(refusal.message()),
                        ),
                    ),
                });
            }
        };
        let response = match reject_private_voice_chat_with_id(
            &context.server_context,
            &me,
            id,
            &request.call_id,
        )
        .await
        {
            Ok(()) => reject_private_voice_chat_response::Response::Ok(
                reject_private_voice_chat_response::Ok {
                    call_id: request.call_id,
                },
            ),
            Err(Refusal::NotFound(m)) => {
                reject_private_voice_chat_response::Response::NotFound(NotFoundError {
                    message: Some(m),
                })
            }
            Err(refusal) => reject_private_voice_chat_response::Response::InternalServerError(
                internal_err(refusal.message()),
            ),
        };
        Ok(RejectPrivateVoiceChatResponse {
            response: Some(response),
        })
    }

    /// Ends the call and tells both parties. Only a party ends it, as upstream
    /// (`voice.ts:189-191`); anyone else gets the unknown call's not-found and
    /// nothing changes. An accepted call ends through the voice runtime, which
    /// deletes the row and then the room; a pending one has no room.
    pub(crate) async fn end_private_voice_chat(
        &self,
        request: EndPrivateVoiceChatPayload,
        context: ProcedureContext<Context>,
    ) -> Result<EndPrivateVoiceChatResponse, SocialError> {
        let me = Self::caller(&context)?;
        let id = match database_call_id(&context.server_context, &request.call_id).await {
            Ok(id) => id,
            Err(refusal) => {
                return Ok(EndPrivateVoiceChatResponse {
                    response: Some(
                        end_private_voice_chat_response::Response::InternalServerError(
                            internal_err(refusal.message()),
                        ),
                    ),
                });
            }
        };
        let response = match end_private_voice_chat_with_id(
            &context.server_context,
            &me,
            id,
            &request.call_id,
        )
        .await
        {
            Ok(()) => {
                end_private_voice_chat_response::Response::Ok(end_private_voice_chat_response::Ok {
                    call_id: request.call_id,
                })
            }
            Err(Refusal::NotFound(m)) => {
                end_private_voice_chat_response::Response::NotFound(NotFoundError {
                    message: Some(m),
                })
            }
            Err(refusal) => end_private_voice_chat_response::Response::InternalServerError(
                internal_err(refusal.message()),
            ),
        };
        Ok(EndPrivateVoiceChatResponse {
            response: Some(response),
        })
    }

    pub(crate) async fn get_incoming_private_voice_chat_request(
        &self,
        context: ProcedureContext<Context>,
    ) -> Result<GetIncomingPrivateVoiceChatRequestResponse, SocialError> {
        let me = Self::caller(&context)?;
        match context
            .server_context
            .db()
            .incoming_private_voice_chat(&me)
            .await
        {
            Ok(Some(chat)) => Ok(GetIncomingPrivateVoiceChatRequestResponse {
                response: Some(
                    get_incoming_private_voice_chat_request_response::Response::Ok(
                        get_incoming_private_voice_chat_request_response::Ok {
                            caller: Some(User {
                                address: chat.caller_address,
                            }),
                            call_id: chat.id.to_string(),
                        },
                    ),
                ),
            }),
            Ok(None) => Ok(GetIncomingPrivateVoiceChatRequestResponse {
                response: Some(
                    get_incoming_private_voice_chat_request_response::Response::NotFound(
                        NotFoundError {
                            message: Some(incoming_not_found(&me)),
                        },
                    ),
                ),
            }),
            Err(e) => Ok(GetIncomingPrivateVoiceChatRequestResponse {
                response: Some(
                    get_incoming_private_voice_chat_request_response::Response::InternalServerError(
                        InternalServerError {
                            message: Some(internal_err(e.to_string()).message.unwrap_or_default()),
                        },
                    ),
                ),
            }),
        }
    }
}
