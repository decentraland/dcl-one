use super::SocialServiceImpl;
use super::domain::{
    fan_community_voice, fan_community_voice_to, require_moderator, require_moderator_over_member,
    require_moderator_protecting_owner, validate_community_voice_participation,
};
use super::helpers::{SocialError, internal_err, normalize};
use crate::comms::gatekeeper::voice::rooms::{CommunityVoiceAction, VoiceUnavailable};
use crate::rpc::context::Context;
use crate::rpc::drpc::service_module_definition::ProcedureContext;
use crate::rpc::proto::errors::ForbiddenError;
use crate::rpc::proto::v2::*;
use crate::{CommunityPermission, CommunityStanding};
use uuid::Uuid;

/// The moderation actions were fire-and-forget against the gatekeeper (a
/// failure answered `ok`); they stay so, logged, with the runtime's verdict.
fn note_runtime(action: &str, community_id: &str, outcome: Result<(), VoiceUnavailable>) {
    if let Err(e) = outcome {
        tracing::warn!(error = %e, %community_id, "voice runtime: {action}");
    }
}

fn community_uuid(community_id: &str) -> Option<Uuid> {
    Uuid::parse_str(community_id).ok()
}

impl SocialServiceImpl {
    /// Owner or moderator of an active community opens the room; every member
    /// but the opener is told STARTED.
    pub(crate) async fn start_community_voice_chat(
        &self,
        request: StartCommunityVoiceChatPayload,
        context: ProcedureContext<Context>,
    ) -> Result<StartCommunityVoiceChatResponse, SocialError> {
        let me = Self::caller(&context)?;
        let ctx = &context.server_context;
        let forbidden = |message: &str| StartCommunityVoiceChatResponse {
            response: Some(
                start_community_voice_chat_response::Response::ForbiddenError(ForbiddenError {
                    message: Some(message.into()),
                }),
            ),
        };
        let (role, community_name, members) = match ctx
            .db()
            .community_voice_start(&request.community_id, &me)
            .await?
        {
            Some((Some(role), name, members))
                if CommunityStanding::from_rows(false, Some(&role))
                    .has(CommunityPermission::BanPlayers) =>
            {
                (role, name, members)
            }
            _ => return Ok(forbidden("requires moderator or owner role")),
        };
        let Some(cid) = community_uuid(&request.community_id) else {
            return Ok(forbidden("requires moderator or owner role"));
        };
        let conn = match ctx
            .voice()
            .community_voice_credentials(cid, &me, &role, CommunityVoiceAction::Create)
            .await
        {
            Ok(url) => url,
            Err(e) => {
                return Ok(StartCommunityVoiceChatResponse {
                    response: Some(
                        start_community_voice_chat_response::Response::InternalServerError(
                            internal_err(format!("community voice credentials: {e}")),
                        ),
                    ),
                });
            }
        };
        fan_community_voice_to(
            ctx,
            &request.community_id,
            CommunityVoiceChatStatus::CommunityVoiceChatStarted,
            Some(me.as_str()),
            &community_name,
            &members,
        );
        Ok(StartCommunityVoiceChatResponse {
            response: Some(start_community_voice_chat_response::Response::Ok(
                start_community_voice_chat_response::Ok {
                    credentials: Some(CommunityVoiceChatCredentials {
                        connection_url: conn,
                    }),
                },
            )),
        })
    }

    /// A member of an active community joins; a banned wallet is refused
    /// before any grant is minted, from the same read as the role.
    pub(crate) async fn join_community_voice_chat(
        &self,
        request: JoinCommunityVoiceChatPayload,
        context: ProcedureContext<Context>,
    ) -> Result<JoinCommunityVoiceChatResponse, SocialError> {
        let me = Self::caller(&context)?;
        let ctx = &context.server_context;
        let forbidden = |message: &str| JoinCommunityVoiceChatResponse {
            response: Some(
                join_community_voice_chat_response::Response::ForbiddenError(ForbiddenError {
                    message: Some(message.into()),
                }),
            ),
        };
        let (role, banned) = match ctx
            .db()
            .community_membership(&request.community_id, &me)
            .await?
        {
            Some((Some(r), banned)) => (r, banned),
            _ => return Ok(forbidden("not a community member")),
        };
        if banned {
            return Ok(forbidden("banned from this community"));
        }
        let Some(cid) = community_uuid(&request.community_id) else {
            return Ok(forbidden("not a community member"));
        };
        let conn = match ctx
            .voice()
            .community_voice_credentials(cid, &me, &role, CommunityVoiceAction::Join)
            .await
        {
            Ok(url) => url,
            Err(e) => {
                return Ok(JoinCommunityVoiceChatResponse {
                    response: Some(
                        join_community_voice_chat_response::Response::InternalServerError(
                            internal_err(format!("community voice credentials: {e}")),
                        ),
                    ),
                });
            }
        };
        Ok(JoinCommunityVoiceChatResponse {
            response: Some(join_community_voice_chat_response::Response::Ok(
                join_community_voice_chat_response::Ok {
                    voice_chat_id: request.community_id,
                    credentials: Some(CommunityVoiceChatCredentials {
                        connection_url: conn,
                    }),
                },
            )),
        })
    }

    pub(crate) async fn request_to_speak_in_community_voice_chat(
        &self,
        request: RequestToSpeakInCommunityVoiceChatPayload,
        context: ProcedureContext<Context>,
    ) -> Result<RequestToSpeakInCommunityVoiceChatResponse, SocialError> {
        let me = Self::caller(&context)?;
        let ctx = &context.server_context;
        if request.is_raising_hand
            && let Err(f) =
                validate_community_voice_participation(ctx.db(), &request.community_id, &me).await?
        {
            return Ok(RequestToSpeakInCommunityVoiceChatResponse {
                response: Some(
                    request_to_speak_in_community_voice_chat_response::Response::ForbiddenError(f),
                ),
            });
        }
        if let Some(cid) = community_uuid(&request.community_id) {
            note_runtime(
                "request to speak",
                &request.community_id,
                ctx.voice()
                    .request_to_speak(cid, &me, request.is_raising_hand)
                    .await,
            );
        }
        Ok(RequestToSpeakInCommunityVoiceChatResponse {
            response: Some(
                request_to_speak_in_community_voice_chat_response::Response::Ok(
                    request_to_speak_in_community_voice_chat_response::Ok {
                        message: "ok".into(),
                    },
                ),
            ),
        })
    }

    pub(crate) async fn promote_speaker_in_community_voice_chat(
        &self,
        request: PromoteSpeakerInCommunityVoiceChatPayload,
        context: ProcedureContext<Context>,
    ) -> Result<PromoteSpeakerInCommunityVoiceChatResponse, SocialError> {
        let me = Self::caller(&context)?;
        let ctx = &context.server_context;
        if let Err(f) = require_moderator_over_member(
            ctx.db(),
            &request.community_id,
            &me,
            &request.user_address,
            "promote speakers",
        )
        .await?
        {
            return Ok(PromoteSpeakerInCommunityVoiceChatResponse {
                response: Some(
                    promote_speaker_in_community_voice_chat_response::Response::ForbiddenError(f),
                ),
            });
        }
        if let Some(cid) = community_uuid(&request.community_id) {
            note_runtime(
                "promote speaker",
                &request.community_id,
                ctx.voice()
                    .set_speaker(cid, &normalize(&request.user_address), true)
                    .await,
            );
        }
        Ok(PromoteSpeakerInCommunityVoiceChatResponse {
            response: Some(
                promote_speaker_in_community_voice_chat_response::Response::Ok(
                    promote_speaker_in_community_voice_chat_response::Ok {
                        message: "ok".into(),
                    },
                ),
            ),
        })
    }

    pub(crate) async fn demote_speaker_in_community_voice_chat(
        &self,
        request: DemoteSpeakerInCommunityVoiceChatPayload,
        context: ProcedureContext<Context>,
    ) -> Result<DemoteSpeakerInCommunityVoiceChatResponse, SocialError> {
        let me = Self::caller(&context)?;
        let ctx = &context.server_context;
        if let Err(f) = require_moderator_over_member(
            ctx.db(),
            &request.community_id,
            &me,
            &request.user_address,
            "demote other speakers",
        )
        .await?
        {
            return Ok(DemoteSpeakerInCommunityVoiceChatResponse {
                response: Some(
                    demote_speaker_in_community_voice_chat_response::Response::ForbiddenError(f),
                ),
            });
        }
        if let Some(cid) = community_uuid(&request.community_id) {
            note_runtime(
                "demote speaker",
                &request.community_id,
                ctx.voice()
                    .set_speaker(cid, &normalize(&request.user_address), false)
                    .await,
            );
        }
        Ok(DemoteSpeakerInCommunityVoiceChatResponse {
            response: Some(
                demote_speaker_in_community_voice_chat_response::Response::Ok(
                    demote_speaker_in_community_voice_chat_response::Ok {
                        message: "ok".into(),
                    },
                ),
            ),
        })
    }

    pub(crate) async fn kick_player_from_community_voice_chat(
        &self,
        request: KickPlayerFromCommunityVoiceChatPayload,
        context: ProcedureContext<Context>,
    ) -> Result<KickPlayerFromCommunityVoiceChatResponse, SocialError> {
        let me = Self::caller(&context)?;
        let ctx = &context.server_context;
        if let Err(f) = require_moderator_protecting_owner(
            ctx.db(),
            &request.community_id,
            &me,
            &request.user_address,
            "kick players",
        )
        .await?
        {
            return Ok(KickPlayerFromCommunityVoiceChatResponse {
                response: Some(
                    kick_player_from_community_voice_chat_response::Response::ForbiddenError(f),
                ),
            });
        }
        if let Some(cid) = community_uuid(&request.community_id) {
            note_runtime(
                "kick player",
                &request.community_id,
                ctx.voice()
                    .kick_player(cid, &normalize(&request.user_address))
                    .await,
            );
        }
        Ok(KickPlayerFromCommunityVoiceChatResponse {
            response: Some(
                kick_player_from_community_voice_chat_response::Response::Ok(
                    kick_player_from_community_voice_chat_response::Ok {
                        message: "ok".into(),
                    },
                ),
            ),
        })
    }

    pub(crate) async fn reject_speak_request_in_community_voice_chat(
        &self,
        request: RejectSpeakRequestInCommunityVoiceChatPayload,
        context: ProcedureContext<Context>,
    ) -> Result<RejectSpeakRequestInCommunityVoiceChatResponse, SocialError> {
        let me = Self::caller(&context)?;
        let ctx = &context.server_context;
        if let Err(f) = require_moderator_over_member(
            ctx.db(),
            &request.community_id,
            &me,
            &request.user_address,
            "reject speak requests",
        )
        .await?
        {
            return Ok(RejectSpeakRequestInCommunityVoiceChatResponse {
                response: Some(
                    reject_speak_request_in_community_voice_chat_response::Response::ForbiddenError(
                        f,
                    ),
                ),
            });
        }
        if let Some(cid) = community_uuid(&request.community_id) {
            note_runtime(
                "reject speak request",
                &request.community_id,
                ctx.voice()
                    .reject_speak_request(cid, &normalize(&request.user_address))
                    .await,
            );
        }
        Ok(RejectSpeakRequestInCommunityVoiceChatResponse {
            response: Some(
                reject_speak_request_in_community_voice_chat_response::Response::Ok(
                    reject_speak_request_in_community_voice_chat_response::Ok {
                        message: "ok".into(),
                    },
                ),
            ),
        })
    }

    pub(crate) async fn end_community_voice_chat(
        &self,
        request: EndCommunityVoiceChatPayload,
        context: ProcedureContext<Context>,
    ) -> Result<EndCommunityVoiceChatResponse, SocialError> {
        let me = Self::caller(&context)?;
        let ctx = &context.server_context;
        if let Err(f) = require_moderator(ctx.db(), &request.community_id, &me).await? {
            return Ok(EndCommunityVoiceChatResponse {
                response: Some(end_community_voice_chat_response::Response::ForbiddenError(
                    f,
                )),
            });
        }
        if let Some(cid) = community_uuid(&request.community_id) {
            note_runtime(
                "end community voice chat",
                &request.community_id,
                ctx.voice().end_community_voice_chat(cid, &me).await,
            );
        }
        fan_community_voice(
            ctx,
            &request.community_id,
            CommunityVoiceChatStatus::CommunityVoiceChatEnded,
            None,
        )
        .await;
        Ok(EndCommunityVoiceChatResponse {
            response: Some(end_community_voice_chat_response::Response::Ok(
                end_community_voice_chat_response::Ok {
                    message: "ok".into(),
                },
            )),
        })
    }

    /// Self-mute is free; self-unmute needs participation; muting another
    /// needs the moderator gate with owner protection.
    pub(crate) async fn mute_speaker_from_community_voice_chat(
        &self,
        request: MuteSpeakerFromCommunityVoiceChatPayload,
        context: ProcedureContext<Context>,
    ) -> Result<MuteSpeakerFromCommunityVoiceChatResponse, SocialError> {
        let me = Self::caller(&context)?;
        let ctx = &context.server_context;
        let target = normalize(&request.user_address);
        let forbidden = |f: ForbiddenError| MuteSpeakerFromCommunityVoiceChatResponse {
            response: Some(
                mute_speaker_from_community_voice_chat_response::Response::ForbiddenError(f),
            ),
        };
        if target == me {
            if !request.muted
                && let Err(f) =
                    validate_community_voice_participation(ctx.db(), &request.community_id, &me)
                        .await?
            {
                return Ok(forbidden(f));
            }
        } else if let Err(f) = require_moderator_protecting_owner(
            ctx.db(),
            &request.community_id,
            &me,
            &target,
            "mute/unmute speakers",
        )
        .await?
        {
            return Ok(forbidden(f));
        }
        if let Some(cid) = community_uuid(&request.community_id) {
            note_runtime(
                "mute speaker",
                &request.community_id,
                ctx.voice().mute_speaker(cid, &target, request.muted).await,
            );
        }
        Ok(MuteSpeakerFromCommunityVoiceChatResponse {
            response: Some(
                mute_speaker_from_community_voice_chat_response::Response::Ok(
                    mute_speaker_from_community_voice_chat_response::Ok {
                        muted: request.muted,
                    },
                ),
            ),
        })
    }
}
