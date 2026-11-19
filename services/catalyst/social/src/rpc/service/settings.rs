use super::SocialServiceImpl;
use super::domain::{bvis_to_db, pmp_from_db, pmp_to_db, settings_to_proto, sreact_to_db};
use super::helpers::{SocialError, internal_err, invalid_req, normalize};
use crate::rpc::context::Context;
use crate::rpc::db::SocialSettingsRow;
use crate::rpc::drpc::service_module_definition::ProcedureContext;
use crate::rpc::proto::v2::*;

/// Upstream `MAX_PRIVATE_MESSAGES_SETTINGS_ADDRESSES`, for both transports.
pub(crate) const MAX_USER_ADDRESSES: usize = 50;

/// `(address, privacy, is_friend)` per distinct target at its first
/// occurrence, as both the procedure and `GET /v1/private-messages-settings`
/// answer it (upstream `settings.getPrivateMessagesSettings`).
pub(crate) async fn private_messages_settings(
    ctx: &Context,
    me: &str,
    mut targets: Vec<String>,
) -> Result<Vec<(String, String, bool)>, crate::rpc::db::DbError> {
    let mut seen = std::collections::HashSet::new();
    targets.retain(|t| seen.insert(t.clone()));
    ctx.db().private_messages_settings(me, &targets).await
}

/// Stores the settings and shows a private-messages privacy change in the
/// live room, for both transports. Tightening to `only_friends` reaches the
/// room first: when that fails only the other settings are stored (a new
/// row takes its `only_friends` default) and the change is refused. Loosening is stored first; a room failure then refuses
/// the change too, and the room keeps the stricter value.
pub(crate) async fn update_social_settings(
    ctx: &Context,
    me: &str,
    privacy: Option<&str>,
    blocked_visibility: Option<&str>,
    reactions: Option<&str>,
) -> Result<SocialSettingsRow, String> {
    let db = ctx.db();
    if let Some(privacy @ "only_friends") = privacy {
        if let Err(error) = ctx
            .voice()
            .publish_private_messages_privacy(me, privacy)
            .await
        {
            if blocked_visibility.is_some() || reactions.is_some() {
                db.upsert_social_settings(me, None, blocked_visibility, reactions)
                    .await
                    .map_err(|e| e.to_string())?;
            }
            return Err(format!(
                "private messages privacy not shown in the room: {error}"
            ));
        }
        return db
            .upsert_social_settings(me, Some(privacy), blocked_visibility, reactions)
            .await
            .map_err(|e| e.to_string());
    }
    let row = db
        .upsert_social_settings(me, privacy, blocked_visibility, reactions)
        .await
        .map_err(|e| e.to_string())?;
    if let Some(privacy) = privacy {
        ctx.voice()
            .publish_private_messages_privacy(me, privacy)
            .await
            .map_err(|error| format!("private messages privacy not shown in the room: {error}"))?;
    }
    Ok(row)
}

impl SocialServiceImpl {
    /// An absent row reads as upstream's defaults (`all`, `show_messages`,
    /// `show`); nothing is inserted by a read.
    pub(crate) async fn get_social_settings(
        &self,
        context: ProcedureContext<Context>,
    ) -> Result<GetSocialSettingsResponse, SocialError> {
        let me = Self::caller(&context)?;
        let row = match context.server_context.db().get_social_settings(&me).await {
            Ok(r) => r.unwrap_or_default(),
            Err(e) => {
                return Ok(GetSocialSettingsResponse {
                    response: Some(get_social_settings_response::Response::InternalServerError(
                        internal_err(e.to_string()),
                    )),
                });
            }
        };
        Ok(GetSocialSettingsResponse {
            response: Some(get_social_settings_response::Response::Ok(
                get_social_settings_response::Ok {
                    settings: Some(settings_to_proto(&row)),
                },
            )),
        })
    }

    pub(crate) async fn upsert_social_settings(
        &self,
        request: UpsertSocialSettingsPayload,
        context: ProcedureContext<Context>,
    ) -> Result<UpsertSocialSettingsResponse, SocialError> {
        let me = Self::caller(&context)?;
        let invalid = if request.private_messages_privacy.is_none()
            && request.blocked_users_messages_visibility.is_none()
            && request.show_situation_reactions.is_none()
        {
            Some("At least one setting to update must be provided")
        } else if request
            .private_messages_privacy
            .is_some_and(|value| PrivateMessagePrivacySetting::try_from(value).is_err())
        {
            Some("Unknown private messages privacy setting")
        } else if request
            .blocked_users_messages_visibility
            .is_some_and(|value| BlockedUsersMessagesVisibilitySetting::try_from(value).is_err())
        {
            Some("Unknown blocked users messages visibility setting")
        } else if request
            .show_situation_reactions
            .is_some_and(|value| SituationReactionsVisibility::try_from(value).is_err())
        {
            Some("Unknown situation reactions visibility setting")
        } else {
            None
        };
        if let Some(message) = invalid {
            return Ok(UpsertSocialSettingsResponse {
                response: Some(upsert_social_settings_response::Response::InvalidRequest(
                    invalid_req(message),
                )),
            });
        }
        let pmp = request
            .private_messages_privacy
            .map(|_| pmp_to_db(request.private_messages_privacy()));
        let bvis = request
            .blocked_users_messages_visibility
            .map(|_| bvis_to_db(request.blocked_users_messages_visibility()));
        let sreact = request
            .show_situation_reactions
            .map(|_| sreact_to_db(request.show_situation_reactions()));

        match update_social_settings(
            &context.server_context,
            &me,
            pmp.as_deref(),
            bvis.as_deref(),
            sreact.as_deref(),
        )
        .await
        {
            Ok(row) => Ok(UpsertSocialSettingsResponse {
                response: Some(upsert_social_settings_response::Response::Ok(
                    settings_to_proto(&row),
                )),
            }),
            Err(e) => {
                tracing::warn!(error = %e, address = %me, "social settings not updated");
                Ok(UpsertSocialSettingsResponse {
                    response: Some(
                        upsert_social_settings_response::Response::InternalServerError(
                            internal_err(e),
                        ),
                    ),
                })
            }
        }
    }

    pub(crate) async fn get_private_messages_settings(
        &self,
        request: GetPrivateMessagesSettingsPayload,
        context: ProcedureContext<Context>,
    ) -> Result<GetPrivateMessagesSettingsResponse, SocialError> {
        let me = Self::caller(&context)?;
        let targets: Vec<String> = request.user.iter().map(|u| normalize(&u.address)).collect();
        if targets.len() > MAX_USER_ADDRESSES {
            return Ok(GetPrivateMessagesSettingsResponse {
                response: Some(
                    get_private_messages_settings_response::Response::InvalidRequest(invalid_req(
                        format!("Too many user addresses: {}", targets.len()),
                    )),
                ),
            });
        }
        // One entry per address, at its first occurrence; the limit counts the request as sent.
        let rows = match private_messages_settings(&context.server_context, &me, targets).await {
            Ok(r) => r,
            Err(e) => {
                return Ok(GetPrivateMessagesSettingsResponse {
                    response: Some(
                        get_private_messages_settings_response::Response::InternalServerError(
                            internal_err(e.to_string()),
                        ),
                    ),
                });
            }
        };
        let settings = rows
            .into_iter()
            .map(|(addr, privacy, is_friend)| {
                get_private_messages_settings_response::PrivateMessagesSettings {
                    user: Some(User { address: addr }),
                    private_messages_privacy: pmp_from_db(&privacy) as i32,
                    is_friend,
                }
            })
            .collect();
        Ok(GetPrivateMessagesSettingsResponse {
            response: Some(get_private_messages_settings_response::Response::Ok(
                get_private_messages_settings_response::Ok { settings },
            )),
        })
    }
}
