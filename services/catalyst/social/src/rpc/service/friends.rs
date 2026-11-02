use super::SocialServiceImpl;
use super::domain::{
    Action, friendship_action_status, friendship_requests, friendship_update_for, status_ok,
    user_action_valid,
};
use super::helpers::{
    FRIENDSHIP_RATE_LIMIT_MESSAGE, FRIENDSHIP_REQUEST_MESSAGE_MAX_LENGTH, Refusal, SocialError,
    empty_friends_profiles, friendship_status_invalid, internal_err, invalid_req, normalize,
    page_friends, page_of, upsert_internal_error,
};
use crate::auth::is_eth_address;
use crate::notifications::{NewNotification, insert_notifications};
use crate::rpc::context::{Context, PairScope};
use crate::rpc::drpc::service_module_definition::ProcedureContext;
use crate::rpc::proto::errors::InvalidFriendshipAction;
use crate::rpc::proto::v2::*;
use crate::rpc::pubsub::SocialEvent;

/// Upstream `BlockedUserError`.
pub(crate) const BLOCKED_MESSAGE: &str =
    "This action is not allowed because either you blocked this user or this user blocked you";
/// Upstream `InvalidFriendshipActionError`'s default.
pub(crate) const INVALID_FRIENDSHIP_ACTION_MESSAGE: &str =
    "The friendship action is not valid for the current friendship status";

/// One friendship action, whichever transport asked (upstream
/// `applyFriendshipAction`, behind both `upsertFriendship` variants): the
/// message bound, the rate limiter, the block check, the state machine, the
/// write (row + action in one statement), then the update to the other
/// party. Answers the friendship id and the action's time in milliseconds.
pub(crate) async fn apply_friendship_action(
    ctx: &Context,
    me: &str,
    other: &str,
    action: Action,
    message: Option<&str>,
) -> Result<(uuid::Uuid, i64), Refusal> {
    if message.is_some_and(|m| m.chars().count() > FRIENDSHIP_REQUEST_MESSAGE_MAX_LENGTH) {
        return Err(Refusal::Invalid(format!(
            "The friendship request message cannot be longer than {FRIENDSHIP_REQUEST_MESSAGE_MAX_LENGTH} characters"
        )));
    }
    if !ctx
        .friendship_limiter()
        .allow(me, other, PairScope::Symmetric)
    {
        tracing::warn!(actor = %me, target = %other, "friendship mutation rate limited");
        return Err(Refusal::RateLimited(FRIENDSHIP_RATE_LIMIT_MESSAGE.into()));
    }
    let db = ctx.db();
    let probe = db.friendship_probe(me, other).await?;
    if probe.blocked || probe.blocked_by {
        return Err(Refusal::Forbidden(BLOCKED_MESSAGE.into()));
    }
    let last = probe.last;
    if !user_action_valid(me, action, other, last.as_ref()) {
        return Err(Refusal::Conflict(INVALID_FRIENDSHIP_ACTION_MESSAGE.into()));
    }
    let (id, created_at, action_id) = db
        .apply_friendship_action(
            me,
            other,
            action.as_str(),
            action.implies_active(),
            last.as_ref().map(|l| l.friendship_id),
            message,
        )
        .await?;
    let created_ms = created_at.timestamp_millis();
    let my_profile = ctx.profiles().friend_profile(me).await;
    let update = friendship_update_for(action, me, &id, created_ms, message, my_profile.clone());
    ctx.pubsub().publish(other, SocialEvent::Friendship(update));
    if action == Action::Accept {
        announce_accepted_connectivity(ctx, me, other, my_profile).await;
    }
    notify_friendship_action(ctx, action, action_id, me, other, message).await;
    Ok((id, created_ms))
}

/// An accept changes the recipients of connectivity even when the Pulse wallet
/// set stays unchanged. Queue only current ONLINE peers; the stream's existing
/// relationship and topology guards recheck them before delivery.
async fn announce_accepted_connectivity(
    ctx: &Context,
    me: &str,
    other: &str,
    my_profile: FriendProfile,
) {
    let other_profile = ctx.profiles().friend_profile(other).await;
    let snapshot = ctx.topology().snapshot();
    for (recipient, friend) in [(other, my_profile), (me, other_profile)] {
        if snapshot.is_online(&friend.address) {
            let friend = crate::rpc::profiles::connectivity_profile(friend);
            let address = friend.address.clone();
            ctx.topology()
                .deliver_if_current(snapshot.generation, &address, true, || {
                    ctx.pubsub().publish(
                        recipient,
                        SocialEvent::TopologyFriendConnectivity(
                            FriendConnectivityUpdate {
                                stream_closed: None,
                                friend: Some(friend),
                                status: ConnectivityStatus::Online as i32,
                            },
                            snapshot.generation,
                        ),
                    );
                });
        }
    }
}

/// The request/accept notification is best effort after the action commits,
/// like upstream's notification worker. Reuse its action UUID for deduplication
/// and the existing inbox/email outbox; missing profiles never fail the action.
async fn notify_friendship_action(
    ctx: &Context,
    action: Action,
    action_id: uuid::Uuid,
    me: &str,
    other: &str,
    message: Option<&str>,
) {
    let kind = match action {
        Action::Request => "social_service_friendship_request",
        Action::Accept => "social_service_friendship_accepted",
        _ => return,
    };
    let profiles = ctx
        .profiles()
        .get_profiles(&[me.to_string(), other.to_string()])
        .await;
    let (Some(sender), Some(receiver)) = (profiles.get(me), profiles.get(other)) else {
        tracing::warn!(
            action = action.as_str(),
            "friendship notification skipped: profile unavailable"
        );
        return;
    };
    let Ok(address) = crate::Address::parse(other) else {
        return;
    };
    let profile_metadata = |address: &str, profile: &crate::rpc::profiles::ProfileInfo| {
        serde_json::json!({
            "address": address,
            "name": profile.name,
            "profileImageUrl": profile.profile_picture_url,
            "hasClaimedName": profile.has_claimed_name,
        })
    };
    let mut metadata = serde_json::json!({
        "requestId": action_id.to_string(),
        "sender": profile_metadata(me, sender),
        "receiver": profile_metadata(other, receiver),
    });
    if action == Action::Request
        && let Some(message) = message.filter(|m| !m.is_empty())
    {
        metadata["message"] = serde_json::json!(message);
    }
    let notification = NewNotification {
        id: action_id,
        ..NewNotification::now(address, kind, metadata)
    };
    if let Err(error) = insert_notifications(ctx.db().pool(), &[notification]).await {
        tracing::warn!(%error, kind, "friendship notification not written");
    }
}

/// The status `me` reads for the pair: the last action, or the block rows
/// when there is none (the RPC procedure and the HTTP route alike).
pub(crate) async fn friendship_status(
    ctx: &Context,
    me: &str,
    other: &str,
) -> Result<FriendshipStatus, crate::rpc::db::DbError> {
    let probe = ctx.db().friendship_probe(me, other).await?;
    Ok(probe
        .last
        .as_ref()
        .and_then(|last| friendship_action_status(last, me))
        .unwrap_or(if probe.blocked {
            FriendshipStatus::Blocked
        } else if probe.blocked_by {
            FriendshipStatus::BlockedBy
        } else {
            FriendshipStatus::None
        }))
}

impl SocialServiceImpl {
    pub(crate) async fn get_friends(
        &self,
        request: GetFriendsPayload,
        context: ProcedureContext<Context>,
    ) -> Result<PaginatedFriendsProfilesResponse, SocialError> {
        let me = Self::caller(&context)?;
        let db = context.server_context.db();
        let (limit, offset) = page_friends(&request.pagination);
        let (friends, total) = match db.get_friends(&me, limit, offset).await {
            Ok(v) => v,
            Err(e) => {
                tracing::error!(error = %e, "get_friends failed");
                return Ok(empty_friends_profiles());
            }
        };
        let profiles = context
            .server_context
            .profiles()
            .friend_profiles(&friends)
            .await;
        Ok(PaginatedFriendsProfilesResponse {
            friends: profiles,
            pagination_data: Some(PaginatedResponse {
                total: total as i32,
                page: page_of(limit, offset),
            }),
        })
    }

    pub(crate) async fn get_mutual_friends(
        &self,
        request: GetMutualFriendsPayload,
        context: ProcedureContext<Context>,
    ) -> Result<PaginatedFriendsProfilesResponse, SocialError> {
        let me = Self::caller(&context)?;
        let other = match request.user.as_ref().map(|u| normalize(&u.address)) {
            Some(a) if is_eth_address(&a) => a,
            _ => return Ok(empty_friends_profiles()),
        };
        let db = context.server_context.db();
        let (limit, offset) = page_friends(&request.pagination);
        let (friends, total) = match db.get_mutual_friends(&me, &other, limit, offset).await {
            Ok(v) => v,
            Err(e) => {
                tracing::error!(error = %e, "get_mutual_friends failed");
                return Ok(empty_friends_profiles());
            }
        };
        let profiles = context
            .server_context
            .profiles()
            .friend_profiles(&friends)
            .await;
        Ok(PaginatedFriendsProfilesResponse {
            friends: profiles,
            pagination_data: Some(PaginatedResponse {
                total: total as i32,
                page: page_of(limit, offset),
            }),
        })
    }

    pub(crate) async fn get_pending_friendship_requests(
        &self,
        request: GetFriendshipRequestsPayload,
        context: ProcedureContext<Context>,
    ) -> Result<PaginatedFriendshipRequestsResponse, SocialError> {
        friendship_requests(&context, request, true).await
    }

    pub(crate) async fn get_sent_friendship_requests(
        &self,
        request: GetFriendshipRequestsPayload,
        context: ProcedureContext<Context>,
    ) -> Result<PaginatedFriendshipRequestsResponse, SocialError> {
        friendship_requests(&context, request, false).await
    }

    pub(crate) async fn get_friendship_status(
        &self,
        request: GetFriendshipStatusPayload,
        context: ProcedureContext<Context>,
    ) -> Result<GetFriendshipStatusResponse, SocialError> {
        let me = Self::caller(&context)?;
        let other = match request.user.as_ref().map(|u| u.address.clone()) {
            Some(a) if !a.trim().is_empty() => {
                let n = normalize(&a);
                if !is_eth_address(&n) {
                    return Ok(friendship_status_invalid(
                        "Invalid user address in the request payload",
                    ));
                }
                n
            }
            _ => {
                return Ok(friendship_status_invalid(
                    "User address is missing in the request payload",
                ));
            }
        };
        match friendship_status(&context.server_context, &me, &other).await {
            Ok(status) => Ok(status_ok(status)),
            Err(e) => Ok(GetFriendshipStatusResponse {
                response: Some(
                    get_friendship_status_response::Response::InternalServerError(internal_err(
                        e.to_string(),
                    )),
                ),
            }),
        }
    }

    /// One friendship action: the rate limiter, the block check, the state
    /// machine, the write (row + action in one statement), then the update to
    /// the other party.
    pub(crate) async fn upsert_friendship(
        &self,
        request: UpsertFriendshipPayload,
        context: ProcedureContext<Context>,
    ) -> Result<UpsertFriendshipResponse, SocialError> {
        let me = Self::caller(&context)?;

        let (action, other, message) = match request.action {
            Some(upsert_friendship_payload::Action::Request(p)) => (
                Action::Request,
                p.user.map(|u| normalize(&u.address)).unwrap_or_default(),
                p.message,
            ),
            Some(upsert_friendship_payload::Action::Accept(p)) => (
                Action::Accept,
                p.user.map(|u| normalize(&u.address)).unwrap_or_default(),
                None,
            ),
            Some(upsert_friendship_payload::Action::Reject(p)) => (
                Action::Reject,
                p.user.map(|u| normalize(&u.address)).unwrap_or_default(),
                None,
            ),
            Some(upsert_friendship_payload::Action::Delete(p)) => (
                Action::Delete,
                p.user.map(|u| normalize(&u.address)).unwrap_or_default(),
                None,
            ),
            Some(upsert_friendship_payload::Action::Cancel(p)) => (
                Action::Cancel,
                p.user.map(|u| normalize(&u.address)).unwrap_or_default(),
                None,
            ),
            None => {
                return Ok(UpsertFriendshipResponse {
                    response: Some(upsert_friendship_response::Response::InvalidRequest(
                        invalid_req("missing action"),
                    )),
                });
            }
        };

        if other.is_empty() || other == me {
            return Ok(UpsertFriendshipResponse {
                response: Some(upsert_friendship_response::Response::InvalidRequest(
                    invalid_req("invalid target user"),
                )),
            });
        }

        let (id, created_ms) = match apply_friendship_action(
            &context.server_context,
            &me,
            &other,
            action,
            message.as_deref(),
        )
        .await
        {
            Ok(v) => v,
            Err(Refusal::Invalid(m) | Refusal::RateLimited(m)) => {
                return Ok(UpsertFriendshipResponse {
                    response: Some(upsert_friendship_response::Response::InvalidRequest(
                        invalid_req(m),
                    )),
                });
            }
            Err(Refusal::Internal(m) | Refusal::Failed(m)) => {
                return Ok(upsert_internal_error(m));
            }
            Err(refusal) => {
                return Ok(UpsertFriendshipResponse {
                    response: Some(
                        upsert_friendship_response::Response::InvalidFriendshipAction(
                            InvalidFriendshipAction {
                                message: Some(refusal.message().to_string()),
                            },
                        ),
                    ),
                });
            }
        };
        let other_profile = context
            .server_context
            .profiles()
            .friend_profile(&other)
            .await;

        Ok(UpsertFriendshipResponse {
            response: Some(upsert_friendship_response::Response::Accepted(
                upsert_friendship_response::Accepted {
                    id: id.to_string(),
                    created_at: created_ms,
                    friend: Some(other_profile),
                    message,
                },
            )),
        })
    }
}
