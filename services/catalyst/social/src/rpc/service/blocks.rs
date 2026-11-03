use super::SocialServiceImpl;
use super::helpers::{
    FRIENDSHIP_RATE_LIMIT_MESSAGE, Refusal, SocialError, internal_err, invalid_req,
    page_blocked_users, page_of,
};
use crate::rpc::context::{Context, PairScope};
use crate::rpc::drpc::service_module_definition::ProcedureContext;
use crate::rpc::profiles;
use crate::rpc::proto::v2::*;
use crate::rpc::pubsub::SocialEvent;

/// Records the block and, when a friendship exists, appends the `block`
/// action that deactivates it, both in one transaction; the other party gets
/// a BlockUpdate, and a FriendshipUpdate::Block only when a friendship action
/// was written. Shared by the RPC procedure and `POST /v1/blocks` (upstream
/// `applyBlock`). Answers when the block was recorded.
pub(crate) async fn block(
    ctx: &Context,
    me: &str,
    other: &str,
) -> Result<chrono::DateTime<chrono::Utc>, Refusal> {
    if !ctx
        .friendship_limiter()
        .allow(me, other, PairScope::Directional)
        .await
    {
        tracing::warn!(actor = %me, target = %other, "block mutation rate limited");
        return Err(Refusal::RateLimited(FRIENDSHIP_RATE_LIMIT_MESSAGE.into()));
    }
    let (blocked_at, ended_friendship) = ctx.db().block_user_recording_action(me, other).await?;
    let pubsub = ctx.pubsub();
    if ended_friendship {
        ctx.pubsub().publish(
            other,
            SocialEvent::Friendship(FriendshipUpdate {
                stream_closed: None,
                update: Some(friendship_update::Update::Block(
                    friendship_update::BlockResponse {
                        user: Some(User {
                            address: me.to_string(),
                        }),
                    },
                )),
            }),
        );
    }
    pubsub.publish(
        other,
        SocialEvent::Block(BlockUpdate {
            stream_closed: None,
            address: me.to_string(),
            is_blocked: true,
        }),
    );
    Ok(blocked_at)
}

/// Removes the block; a friendship the block had deactivated is closed with a
/// `delete` action (the pair starts over) in the same transaction, and the
/// other party is told. Without a block to remove nothing is recorded or
/// told. Shared by the RPC procedure and `DELETE /v1/blocks/{address}`
/// (upstream `applyUnblock`).
pub(crate) async fn unblock(ctx: &Context, me: &str, other: &str) -> Result<(), Refusal> {
    if !ctx
        .friendship_limiter()
        .allow(me, other, PairScope::Directional)
        .await
    {
        tracing::warn!(actor = %me, target = %other, "block mutation rate limited");
        return Err(Refusal::RateLimited(FRIENDSHIP_RATE_LIMIT_MESSAGE.into()));
    }
    let Some(deleted_friendship) = ctx.db().unblock_user_recording_action(me, other).await? else {
        return Ok(());
    };
    if deleted_friendship {
        ctx.pubsub().publish(
            other,
            SocialEvent::Friendship(FriendshipUpdate {
                stream_closed: None,
                update: Some(friendship_update::Update::Delete(
                    friendship_update::DeleteResponse {
                        user: Some(User {
                            address: me.to_string(),
                        }),
                    },
                )),
            }),
        );
    }
    ctx.pubsub().publish(
        other,
        SocialEvent::Block(BlockUpdate {
            stream_closed: None,
            address: me.to_string(),
            is_blocked: false,
        }),
    );
    Ok(())
}

/// Upstream `block-user.ts` / `unblock-user.ts`: the raw address must be an
/// address, and not the caller's.
fn block_target(
    request_user: Option<&User>,
    me: &str,
    yourself: &'static str,
) -> Result<String, &'static str> {
    let raw = request_user.map(|u| u.address.as_str()).unwrap_or_default();
    if !crate::auth::is_eth_address(raw) {
        return Err("Invalid user address in the request payload");
    }
    let other = raw.to_lowercase();
    if other == me {
        return Err(yourself);
    }
    Ok(other)
}

impl SocialServiceImpl {
    /// Records the block and, when a friendship exists, appends the `block`
    /// action that deactivates it; the other party gets a BlockUpdate and a
    /// FriendshipUpdate::Block.
    pub(crate) async fn block_user(
        &self,
        request: BlockUserPayload,
        context: ProcedureContext<Context>,
    ) -> Result<BlockUserResponse, SocialError> {
        let me = Self::caller(&context)?;
        let other = match block_target(request.user.as_ref(), &me, "Cannot block yourself") {
            Ok(other) => other,
            Err(message) => {
                return Ok(BlockUserResponse {
                    response: Some(block_user_response::Response::InvalidRequest(invalid_req(
                        message,
                    ))),
                });
            }
        };
        let blocked_at = match block(&context.server_context, &me, &other).await {
            Ok(at) => at,
            Err(Refusal::Internal(m) | Refusal::Failed(m)) => {
                return Ok(BlockUserResponse {
                    response: Some(block_user_response::Response::InternalServerError(
                        internal_err(m),
                    )),
                });
            }
            Err(refusal) => {
                return Ok(BlockUserResponse {
                    response: Some(block_user_response::Response::InvalidRequest(invalid_req(
                        refusal.message(),
                    ))),
                });
            }
        };

        let profile = context
            .server_context
            .profiles()
            .blocked_profile(&other, Some(blocked_at.timestamp_millis()))
            .await;
        Ok(BlockUserResponse {
            response: Some(block_user_response::Response::Ok(block_user_response::Ok {
                profile: Some(profile),
            })),
        })
    }

    /// Removes the block; a friendship the block had deactivated is closed with
    /// a `delete` action (the pair starts over), and the other party is told.
    /// Without a block to remove nothing is recorded or told.
    pub(crate) async fn unblock_user(
        &self,
        request: UnblockUserPayload,
        context: ProcedureContext<Context>,
    ) -> Result<UnblockUserResponse, SocialError> {
        let me = Self::caller(&context)?;
        let other = match block_target(request.user.as_ref(), &me, "Cannot unblock yourself") {
            Ok(other) => other,
            Err(message) => {
                return Ok(UnblockUserResponse {
                    response: Some(unblock_user_response::Response::InvalidRequest(
                        invalid_req(message),
                    )),
                });
            }
        };
        if other == me {
            return Ok(UnblockUserResponse {
                response: Some(unblock_user_response::Response::InvalidRequest(
                    invalid_req("Cannot unblock yourself"),
                )),
            });
        }
        match unblock(&context.server_context, &me, &other).await {
            Ok(()) => {}
            Err(Refusal::Internal(m) | Refusal::Failed(m)) => {
                return Ok(UnblockUserResponse {
                    response: Some(unblock_user_response::Response::InternalServerError(
                        internal_err(m),
                    )),
                });
            }
            Err(refusal) => {
                return Ok(UnblockUserResponse {
                    response: Some(unblock_user_response::Response::InvalidRequest(
                        invalid_req(refusal.message()),
                    )),
                });
            }
        }
        let profile = context
            .server_context
            .profiles()
            .blocked_profile(&other, None)
            .await;
        Ok(UnblockUserResponse {
            response: Some(unblock_user_response::Response::Ok(
                unblock_user_response::Ok {
                    profile: Some(profile),
                },
            )),
        })
    }

    pub(crate) async fn get_blocked_users(
        &self,
        request: GetBlockedUsersPayload,
        context: ProcedureContext<Context>,
    ) -> Result<GetBlockedUsersResponse, SocialError> {
        let me = Self::caller(&context)?;
        let db = context.server_context.db();
        let (limit, offset) = page_blocked_users(&request.pagination);
        let (rows, total) = match db.get_blocked_users(&me, limit, offset).await {
            Ok(v) => v,
            Err(e) => {
                tracing::error!(error = %e, "get_blocked_users failed");
                return Ok(GetBlockedUsersResponse {
                    profiles: Vec::new(),
                    pagination_data: Some(PaginatedResponse { total: 0, page: 1 }),
                });
            }
        };
        let addrs: Vec<String> = rows.iter().map(|r| r.address.clone()).collect();
        let map = match context
            .server_context
            .profiles()
            .get_profiles_result(&addrs)
            .await
        {
            Ok(profiles) => profiles,
            Err(error) => {
                tracing::error!(%error, "blocked profiles unavailable");
                return Ok(GetBlockedUsersResponse {
                    profiles: Vec::new(),
                    pagination_data: Some(PaginatedResponse { total: 0, page: 1 }),
                });
            }
        };
        let profiles = rows
            .iter()
            .filter_map(|r| {
                let profile = map.get(&r.address.to_lowercase())?;
                Some(profiles::blocked_profile(
                    &r.address,
                    Some(profile),
                    Some(r.blocked_at.timestamp_millis()),
                ))
            })
            .collect();
        Ok(GetBlockedUsersResponse {
            profiles,
            pagination_data: Some(PaginatedResponse {
                total: total as i32,
                page: page_of(limit, offset),
            }),
        })
    }

    pub(crate) async fn get_blocking_status(
        &self,
        context: ProcedureContext<Context>,
    ) -> Result<GetBlockingStatusResponse, SocialError> {
        let me = Self::caller(&context)?;
        let (blocked_users, blocked_by_users) = context
            .server_context
            .db()
            .get_blocking_status(&me)
            .await
            .unwrap_or_else(|e| {
                tracing::error!(error = %e, "get_blocking_status failed");
                Default::default()
            });
        Ok(GetBlockingStatusResponse {
            blocked_users,
            blocked_by_users,
        })
    }
}
