use super::SocialServiceImpl;
use super::domain::{
    fan_community_voice, fan_community_voice_to, require_moderator, require_moderator_over_member,
    require_moderator_protecting_owner, validate_community_voice_participation,
};
use super::helpers::{Refusal, SocialError, internal_err, normalize};
use crate::comms::gatekeeper::voice::rooms::{CommunityVoiceAction, VoiceUnavailable};
use crate::notifications::community::{Audience, CommunityEvent, notify, now_ms};
use crate::rpc::context::Context;
use crate::rpc::drpc::service_module_definition::ProcedureContext;
use crate::rpc::proto::errors::{ForbiddenError, NotFoundError};
use crate::rpc::proto::v2::*;
use crate::{CommunityPermission, CommunityStanding};
use uuid::Uuid;

/// Upstream `CommunityVoiceChatResourceNotFoundError`: the gatekeeper found
/// no room, or no such participant in it, for an authorized action.
pub(crate) fn resource_not_found(community_id: &str) -> Refusal {
    Refusal::NotFound(format!(
        "Community voice chat or participant not found for community {community_id}"
    ))
}

/// Missing room resources are not-found; other executor failures reach the
/// caller as an internal error rather than acknowledging an action that failed.
fn runtime(
    action: &str,
    community_id: &str,
    outcome: Result<(), VoiceUnavailable>,
) -> Result<(), Refusal> {
    match outcome {
        Ok(()) => Ok(()),
        Err(VoiceUnavailable::NotFound) => Err(resource_not_found(community_id)),
        Err(e) => {
            tracing::warn!(error = %e, %community_id, "voice runtime: {action}");
            Err(Refusal::Internal(e.to_string()))
        }
    }
}

fn community_uuid(community_id: &str) -> Option<Uuid> {
    Uuid::parse_str(community_id).ok()
}

fn forbidden(f: ForbiddenError) -> Refusal {
    Refusal::Forbidden(f.message.unwrap_or_default())
}

const REQUIRES_MODERATOR: &str = "requires moderator or owner role";
const NOT_A_MEMBER: &str = "not a community member";

/// Owner or moderator of an active community opens the room; every member
/// but the opener is told STARTED. Answers the opener's connection URL.
pub(crate) async fn start_community_voice_chat(
    ctx: &Context,
    me: &str,
    community_id: &str,
) -> Result<String, Refusal> {
    let (role, community_name, members) =
        match ctx.db().community_voice_start(community_id, me).await? {
            Some((Some(role), name, members))
                if CommunityStanding::from_rows(false, Some(&role))
                    .has(CommunityPermission::BanPlayers) =>
            {
                (role, name, members)
            }
            _ => return Err(Refusal::Forbidden(REQUIRES_MODERATOR.into())),
        };
    let Some(cid) = community_uuid(community_id) else {
        return Err(Refusal::Forbidden(REQUIRES_MODERATOR.into()));
    };
    let conn = ctx
        .voice()
        .community_voice_credentials(cid, me, &role, CommunityVoiceAction::Create)
        .await
        .map_err(|e| Refusal::Internal(format!("community voice credentials: {e}")))?;
    fan_community_voice_to(
        ctx,
        community_id,
        CommunityVoiceChatStatus::CommunityVoiceChatStarted,
        Some(me),
        &community_name,
        &members,
    );
    // upstream `broadcastVoiceChatStarted` (`logic/community/broadcaster.ts`
    // at `b6b2cef`): the online members but the opener; the community
    // image is empty here, as on the update above
    let online = ctx.topology().snapshot().wallets;
    notify(
        ctx.db().pool(),
        CommunityEvent {
            kind: "community_voice_chat_started",
            key: format!("{cid}-{}", now_ms()),
            community_id: crate::CommunityId::from_uuid(cid),
            metadata: serde_json::json!({
                "communityId": cid,
                "communityName": community_name,
                "thumbnailUrl": "",
            }),
        },
        Audience::OnlineMembersExcept {
            online: &online,
            except: me,
        },
    )
    .await;
    Ok(conn)
}

/// An active public community admits guests; private communities require
/// membership. A ban racing the grant is refused by a second check, which
/// attempts to evict the newly created seat before returning.
pub(crate) async fn join_community_voice_chat(
    ctx: &Context,
    me: &str,
    community_id: &str,
) -> Result<String, Refusal> {
    let role = match ctx.db().community_membership(community_id, me).await? {
        Some((_, _, true)) => {
            return Err(Refusal::Forbidden("banned from this community".into()));
        }
        Some((_, Some(role), false)) => role,
        Some((false, None, false)) => "none".into(),
        Some((true, None, false)) => return Err(Refusal::Forbidden(NOT_A_MEMBER.into())),
        None => return Err(resource_not_found(community_id)),
    };
    let Some(cid) = community_uuid(community_id) else {
        return Err(Refusal::Forbidden(NOT_A_MEMBER.into()));
    };
    let credentials = ctx
        .voice()
        .community_voice_credentials(cid, me, &role, CommunityVoiceAction::Join)
        .await
        .map_err(|e| match e {
            VoiceUnavailable::NotFound => resource_not_found(community_id),
            other => Refusal::Internal(format!("community voice credentials: {other}")),
        })?;
    if ctx.db().is_member_banned(community_id, me).await? {
        if let Err(e) = ctx.voice().kick_player(cid, me).await {
            tracing::warn!(error = %e, %community_id, "voice runtime: evict join racing a ban");
        }
        return Err(Refusal::Forbidden("banned from this community".into()));
    }
    Ok(credentials)
}

/// Raising a hand needs participation; lowering it is idempotent and never
/// discloses whether the room or the participant still exists.
pub(crate) async fn request_to_speak(
    ctx: &Context,
    me: &str,
    community_id: &str,
    raising_hand: bool,
) -> Result<(), Refusal> {
    if raising_hand {
        validate_community_voice_participation(ctx.db(), community_id, me)
            .await?
            .map_err(forbidden)?;
    }
    let Some(cid) = community_uuid(community_id) else {
        return Ok(());
    };
    let outcome = ctx.voice().request_to_speak(cid, me, raising_hand).await;
    match runtime("request to speak", community_id, outcome) {
        Err(Refusal::NotFound(_)) if !raising_hand => Ok(()),
        other => other,
    }
}

/// The speaker change, `promote` or not, on a member of the room.
pub(crate) async fn set_speaker(
    ctx: &Context,
    me: &str,
    community_id: &str,
    target: &str,
    promote: bool,
) -> Result<(), Refusal> {
    let action = if promote {
        "promote speakers"
    } else {
        "demote other speakers"
    };
    let target = normalize(target);
    if !promote && target == me {
        let Some((private, role, _)) = ctx.db().community_membership(community_id, me).await?
        else {
            return Err(resource_not_found(community_id));
        };
        if private
            && CommunityStanding::from_rows(false, role.as_deref()) == CommunityStanding::NotAMember
        {
            return Err(Refusal::Forbidden(NOT_A_MEMBER.into()));
        }
    } else {
        require_moderator_over_member(ctx.db(), community_id, me, &target, action, true)
            .await?
            .map_err(forbidden)?;
    }
    let Some(cid) = community_uuid(community_id) else {
        return Ok(());
    };
    let outcome = ctx.voice().set_speaker(cid, &target, promote).await;
    runtime(action, community_id, outcome)
}

pub(crate) async fn kick_player(
    ctx: &Context,
    me: &str,
    community_id: &str,
    target: &str,
) -> Result<(), Refusal> {
    require_moderator_protecting_owner(ctx.db(), community_id, me, target, "kick players")
        .await?
        .map_err(forbidden)?;
    let Some(cid) = community_uuid(community_id) else {
        return Ok(());
    };
    let outcome = ctx.voice().kick_player(cid, &normalize(target)).await;
    runtime("kick player", community_id, outcome)
}

pub(crate) async fn reject_speak_request(
    ctx: &Context,
    me: &str,
    community_id: &str,
    target: &str,
) -> Result<(), Refusal> {
    require_moderator_over_member(
        ctx.db(),
        community_id,
        me,
        target,
        "reject speak requests",
        false,
    )
    .await?
    .map_err(forbidden)?;
    let Some(cid) = community_uuid(community_id) else {
        return Ok(());
    };
    let outcome = ctx
        .voice()
        .reject_speak_request(cid, &normalize(target))
        .await;
    runtime("reject speak request", community_id, outcome)
}

/// An owner or moderator ends the room; every member is told ENDED.
pub(crate) async fn end_community_voice_chat(
    ctx: &Context,
    me: &str,
    community_id: &str,
) -> Result<(), Refusal> {
    require_moderator(ctx.db(), community_id, me)
        .await?
        .map_err(forbidden)?;
    if let Some(cid) = community_uuid(community_id) {
        runtime(
            "end community voice chat",
            community_id,
            ctx.voice().end_community_voice_chat(cid, me).await,
        )?;
    }
    fan_community_voice(
        ctx,
        community_id,
        CommunityVoiceChatStatus::CommunityVoiceChatEnded,
        None,
    )
    .await;
    Ok(())
}

/// Self-mute is free; self-unmute needs participation; muting another
/// needs the moderator gate with owner protection.
pub(crate) async fn mute_speaker(
    ctx: &Context,
    me: &str,
    community_id: &str,
    target: &str,
    muted: bool,
) -> Result<(), Refusal> {
    let target = normalize(target);
    if target == me {
        if !muted {
            validate_community_voice_participation(ctx.db(), community_id, me)
                .await?
                .map_err(forbidden)?;
        }
    } else {
        require_moderator_protecting_owner(
            ctx.db(),
            community_id,
            me,
            &target,
            "mute/unmute speakers",
        )
        .await?
        .map_err(forbidden)?;
    }
    let Some(cid) = community_uuid(community_id) else {
        return Ok(());
    };
    let outcome = ctx.voice().mute_speaker(cid, &target, muted).await;
    runtime("mute speaker", community_id, outcome)
}

/// The `oneof` arms every community-voice procedure shares.
enum Answer {
    Forbidden(ForbiddenError),
    NotFound(NotFoundError),
    Internal(crate::rpc::proto::errors::InternalServerError),
}

fn answer(refusal: Refusal) -> Result<Answer, SocialError> {
    Ok(match refusal {
        Refusal::Forbidden(m) => Answer::Forbidden(ForbiddenError { message: Some(m) }),
        Refusal::NotFound(m) => Answer::NotFound(NotFoundError { message: Some(m) }),
        Refusal::Failed(m) => return Err(SocialError::Internal(m)),
        other => Answer::Internal(internal_err(other.message())),
    })
}

/// One procedure's response: `ok` from the operation's value, a refusal
/// onto the `oneof` arms every community-voice response has.
macro_rules! respond {
    ($module:ident, $response:ident, $result:expr, $ok:expr) => {{
        let response = match $result {
            Ok(value) => $module::Response::Ok($ok(value)),
            Err(refusal) => match answer(refusal)? {
                Answer::Forbidden(f) => $module::Response::ForbiddenError(f),
                Answer::NotFound(n) => $module::Response::NotFoundError(n),
                Answer::Internal(i) => $module::Response::InternalServerError(i),
            },
        };
        Ok($response {
            response: Some(response),
        })
    }};
}

impl SocialServiceImpl {
    pub(crate) async fn start_community_voice_chat(
        &self,
        request: StartCommunityVoiceChatPayload,
        context: ProcedureContext<Context>,
    ) -> Result<StartCommunityVoiceChatResponse, SocialError> {
        let me = Self::caller(&context)?;
        let result =
            start_community_voice_chat(&context.server_context, &me, &request.community_id).await;
        let response = match result {
            Ok(connection_url) => start_community_voice_chat_response::Response::Ok(
                start_community_voice_chat_response::Ok {
                    credentials: Some(CommunityVoiceChatCredentials { connection_url }),
                },
            ),
            Err(refusal) => match answer(refusal)? {
                Answer::Forbidden(f) => {
                    start_community_voice_chat_response::Response::ForbiddenError(f)
                }
                Answer::NotFound(NotFoundError { message }) => {
                    start_community_voice_chat_response::Response::InternalServerError(
                        internal_err(message.unwrap_or_default()),
                    )
                }
                Answer::Internal(i) => {
                    start_community_voice_chat_response::Response::InternalServerError(i)
                }
            },
        };
        Ok(StartCommunityVoiceChatResponse {
            response: Some(response),
        })
    }

    pub(crate) async fn join_community_voice_chat(
        &self,
        request: JoinCommunityVoiceChatPayload,
        context: ProcedureContext<Context>,
    ) -> Result<JoinCommunityVoiceChatResponse, SocialError> {
        let me = Self::caller(&context)?;
        let community_id = request.community_id;
        respond!(
            join_community_voice_chat_response,
            JoinCommunityVoiceChatResponse,
            join_community_voice_chat(&context.server_context, &me, &community_id).await,
            |connection_url| join_community_voice_chat_response::Ok {
                voice_chat_id: community_id,
                credentials: Some(CommunityVoiceChatCredentials { connection_url }),
            }
        )
    }

    pub(crate) async fn request_to_speak_in_community_voice_chat(
        &self,
        request: RequestToSpeakInCommunityVoiceChatPayload,
        context: ProcedureContext<Context>,
    ) -> Result<RequestToSpeakInCommunityVoiceChatResponse, SocialError> {
        let me = Self::caller(&context)?;
        respond!(
            request_to_speak_in_community_voice_chat_response,
            RequestToSpeakInCommunityVoiceChatResponse,
            request_to_speak(
                &context.server_context,
                &me,
                &request.community_id,
                request.is_raising_hand,
            )
            .await,
            |()| request_to_speak_in_community_voice_chat_response::Ok {
                message: "ok".into(),
            }
        )
    }

    pub(crate) async fn promote_speaker_in_community_voice_chat(
        &self,
        request: PromoteSpeakerInCommunityVoiceChatPayload,
        context: ProcedureContext<Context>,
    ) -> Result<PromoteSpeakerInCommunityVoiceChatResponse, SocialError> {
        let me = Self::caller(&context)?;
        respond!(
            promote_speaker_in_community_voice_chat_response,
            PromoteSpeakerInCommunityVoiceChatResponse,
            set_speaker(
                &context.server_context,
                &me,
                &request.community_id,
                &request.user_address,
                true,
            )
            .await,
            |()| promote_speaker_in_community_voice_chat_response::Ok {
                message: "ok".into(),
            }
        )
    }

    pub(crate) async fn demote_speaker_in_community_voice_chat(
        &self,
        request: DemoteSpeakerInCommunityVoiceChatPayload,
        context: ProcedureContext<Context>,
    ) -> Result<DemoteSpeakerInCommunityVoiceChatResponse, SocialError> {
        let me = Self::caller(&context)?;
        respond!(
            demote_speaker_in_community_voice_chat_response,
            DemoteSpeakerInCommunityVoiceChatResponse,
            set_speaker(
                &context.server_context,
                &me,
                &request.community_id,
                &request.user_address,
                false,
            )
            .await,
            |()| demote_speaker_in_community_voice_chat_response::Ok {
                message: "ok".into(),
            }
        )
    }

    pub(crate) async fn kick_player_from_community_voice_chat(
        &self,
        request: KickPlayerFromCommunityVoiceChatPayload,
        context: ProcedureContext<Context>,
    ) -> Result<KickPlayerFromCommunityVoiceChatResponse, SocialError> {
        let me = Self::caller(&context)?;
        respond!(
            kick_player_from_community_voice_chat_response,
            KickPlayerFromCommunityVoiceChatResponse,
            kick_player(
                &context.server_context,
                &me,
                &request.community_id,
                &request.user_address,
            )
            .await,
            |()| kick_player_from_community_voice_chat_response::Ok {
                message: "ok".into(),
            }
        )
    }

    pub(crate) async fn reject_speak_request_in_community_voice_chat(
        &self,
        request: RejectSpeakRequestInCommunityVoiceChatPayload,
        context: ProcedureContext<Context>,
    ) -> Result<RejectSpeakRequestInCommunityVoiceChatResponse, SocialError> {
        let me = Self::caller(&context)?;
        respond!(
            reject_speak_request_in_community_voice_chat_response,
            RejectSpeakRequestInCommunityVoiceChatResponse,
            reject_speak_request(
                &context.server_context,
                &me,
                &request.community_id,
                &request.user_address,
            )
            .await,
            |()| reject_speak_request_in_community_voice_chat_response::Ok {
                message: "ok".into(),
            }
        )
    }

    pub(crate) async fn end_community_voice_chat(
        &self,
        request: EndCommunityVoiceChatPayload,
        context: ProcedureContext<Context>,
    ) -> Result<EndCommunityVoiceChatResponse, SocialError> {
        let me = Self::caller(&context)?;
        respond!(
            end_community_voice_chat_response,
            EndCommunityVoiceChatResponse,
            end_community_voice_chat(&context.server_context, &me, &request.community_id).await,
            |()| end_community_voice_chat_response::Ok {
                message: "ok".into(),
            }
        )
    }

    pub(crate) async fn mute_speaker_from_community_voice_chat(
        &self,
        request: MuteSpeakerFromCommunityVoiceChatPayload,
        context: ProcedureContext<Context>,
    ) -> Result<MuteSpeakerFromCommunityVoiceChatResponse, SocialError> {
        let me = Self::caller(&context)?;
        let muted = request.muted;
        respond!(
            mute_speaker_from_community_voice_chat_response,
            MuteSpeakerFromCommunityVoiceChatResponse,
            mute_speaker(
                &context.server_context,
                &me,
                &request.community_id,
                &request.user_address,
                muted,
            )
            .await,
            |()| mute_speaker_from_community_voice_chat_response::Ok { muted }
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn executor_failures_are_not_acknowledged_as_success() {
        for error in [
            VoiceUnavailable::NotConfigured,
            VoiceUnavailable::Failed("executor failed".into()),
        ] {
            assert!(matches!(
                runtime("mute", "community", Err(error)),
                Err(Refusal::Internal(_))
            ));
        }
        assert_eq!(
            runtime("mute", "community", Err(VoiceUnavailable::NotFound)),
            Err(resource_not_found("community"))
        );
    }
}
