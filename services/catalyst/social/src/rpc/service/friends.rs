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
use crate::rpc::db::record_friendship_action;
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
/// party, the notification of a request or an accept, and, for an accept,
/// each party's open connectivity stream told the other is online. An
/// explicit empty message is no message (upstream
/// `parseUpsertFriendshipRequest`). Answers the friendship id and the
/// action's time in milliseconds.
pub(crate) async fn apply_friendship_action(
    ctx: &Context,
    me: &str,
    other: &str,
    action: Action,
    message: Option<&str>,
) -> Result<(uuid::Uuid, i64), Refusal> {
    let message = message.filter(|m| !m.is_empty());
    if message.is_some_and(|m| m.chars().count() > FRIENDSHIP_REQUEST_MESSAGE_MAX_LENGTH) {
        return Err(Refusal::Invalid(format!(
            "The friendship request message cannot be longer than {FRIENDSHIP_REQUEST_MESSAGE_MAX_LENGTH} characters"
        )));
    }
    if !ctx
        .friendship_limiter()
        .allow(me, other, PairScope::Symmetric)
        .await
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
    let action_id = uuid::Uuid::new_v4();
    let (id, created_at) = record_friendship_action(
        db.pool(),
        action_id,
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
    let update = friendship_update_for(
        action,
        me,
        &action_id,
        chrono::Utc::now().timestamp_millis(),
        message,
        my_profile,
    );
    ctx.pubsub().publish(other, SocialEvent::Friendship(update));
    if action == Action::Accept {
        announce_accepted_connectivity(ctx, me, other).await;
    }
    notify_friendship_action(ctx, action, action_id, me, other, message).await;
    Ok((id, created_ms))
}

/// An accept changes the recipients of connectivity even when the Pulse wallet
/// set stays unchanged (upstream `friendshipAcceptedUpdateHandler`). Queue only
/// current ONLINE peers; the stream's existing relationship and topology
/// guards recheck them before delivery. A wallet the profile source holds no
/// profile for is skipped, as upstream's live loop skips it.
async fn announce_accepted_connectivity(ctx: &Context, me: &str, other: &str) {
    let snapshot = ctx.topology().snapshot();
    for (recipient, friend) in [(other, me), (me, other)] {
        if !snapshot.is_online(friend) {
            continue;
        }
        let Some(profile) = ctx.profiles().existing_friend_profile(friend).await else {
            continue;
        };
        let friend = profile;
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

/// Upstream's latest-action query synthesizes a block action from the block
/// table only when no friendship action exists.
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
        let profiles = match context
            .server_context
            .profiles()
            .friend_profiles(&friends)
            .await
        {
            Ok(profiles) => profiles,
            Err(error) => {
                tracing::error!(%error, "friend profiles unavailable");
                return Ok(empty_friends_profiles());
            }
        };
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
        let profiles = match context
            .server_context
            .profiles()
            .friend_profiles(&friends)
            .await
        {
            Ok(profiles) => profiles,
            Err(error) => {
                tracing::error!(%error, "friend profiles unavailable");
                return Ok(empty_friends_profiles());
            }
        };
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
            Some(a) if !a.is_empty() => {
                if !is_eth_address(&a) {
                    return Ok(friendship_status_invalid(
                        "Invalid user address in the request payload",
                    ));
                }
                normalize(&a)
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

    /// One friendship action (upstream `upsert-friendship.ts`): an unknown
    /// action is an invalid request, a request to yourself an invalid
    /// action, an address that is not one an invalid request; then the
    /// shared operation, and the receiver's profile for the answer. With a
    /// profile source, a party without a profile answers
    /// `internalServerError` after the write and the update, as upstream's
    /// `ProfileNotFoundError`.
    pub(crate) async fn upsert_friendship(
        &self,
        request: UpsertFriendshipPayload,
        context: ProcedureContext<Context>,
    ) -> Result<UpsertFriendshipResponse, SocialError> {
        let me = Self::caller(&context)?;
        let missing_user = match request.action.as_ref() {
            Some(upsert_friendship_payload::Action::Request(p)) => p.user.is_none(),
            Some(upsert_friendship_payload::Action::Accept(p)) => p.user.is_none(),
            Some(upsert_friendship_payload::Action::Reject(p)) => p.user.is_none(),
            Some(upsert_friendship_payload::Action::Delete(p)) => p.user.is_none(),
            Some(upsert_friendship_payload::Action::Cancel(p)) => p.user.is_none(),
            None => false,
        };
        if missing_user {
            return Ok(upsert_internal_error(
                "Cannot read properties of undefined (reading 'address')",
            ));
        }
        let invalid_request = |message: &str| UpsertFriendshipResponse {
            response: Some(upsert_friendship_response::Response::InvalidRequest(
                invalid_req(message),
            )),
        };
        let invalid_action = |message: String| UpsertFriendshipResponse {
            response: Some(
                upsert_friendship_response::Response::InvalidFriendshipAction(
                    InvalidFriendshipAction {
                        message: Some(message),
                    },
                ),
            ),
        };

        let target = |user: Option<User>| user.map(|u| normalize(&u.address)).unwrap_or_default();
        let (action, other, message) = match request.action {
            Some(upsert_friendship_payload::Action::Request(p)) => {
                (Action::Request, target(p.user), p.message)
            }
            Some(upsert_friendship_payload::Action::Accept(p)) => {
                (Action::Accept, target(p.user), None)
            }
            Some(upsert_friendship_payload::Action::Reject(p)) => {
                (Action::Reject, target(p.user), None)
            }
            Some(upsert_friendship_payload::Action::Delete(p)) => {
                (Action::Delete, target(p.user), None)
            }
            Some(upsert_friendship_payload::Action::Cancel(p)) => {
                (Action::Cancel, target(p.user), None)
            }
            None => return Ok(invalid_request("Unknown message")),
        };
        if action == Action::Request && other == me {
            return Ok(invalid_action(
                "You cannot send a friendship request to yourself".into(),
            ));
        }
        if !is_eth_address(&other) {
            return Ok(invalid_request(
                "Invalid user address in the request payload",
            ));
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
                return Ok(invalid_request(&m));
            }
            Err(Refusal::Internal(m) | Refusal::Failed(m)) => {
                return Ok(upsert_internal_error(m));
            }
            Err(refusal) => return Ok(invalid_action(refusal.message().to_string())),
        };
        let profiles = match context
            .server_context
            .profiles()
            .get_profiles_result(&[me.clone(), other.clone()])
            .await
        {
            Ok(profiles) => profiles,
            Err(error) => return Ok(upsert_internal_error(error.to_string())),
        };
        for address in [&me, &other] {
            if !profiles.contains_key(address) {
                return Ok(upsert_internal_error(format!(
                    "Profile not found for address {address}"
                )));
            }
        }
        let other_profile = crate::rpc::profiles::friend_profile(&other, profiles.get(&other));

        Ok(UpsertFriendshipResponse {
            response: Some(upsert_friendship_response::Response::Accepted(
                upsert_friendship_response::Accepted {
                    id: id.to_string(),
                    created_at: created_ms,
                    friend: Some(other_profile),
                    // upstream `parseFriendshipRequestToFriendshipRequestResponse`:
                    // `metadata?.message || ''`
                    message: Some(message.unwrap_or_default()),
                },
            )),
        })
    }
}
