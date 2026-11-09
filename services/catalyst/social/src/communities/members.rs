//! Members: the listing (private communities are invisible to anonymous
//! viewers, refused to signed non-members, open to the admin bearer), the
//! join/leave/kick/role/ban/unban writes and the per-wallet community
//! listings. A ban deletes the member row and the wallet's pending requests
//! in source-defined independent commits; a leave, kick or ban of a private
//! community's member reaches the voice-eviction seam.

use std::collections::HashMap;

use crate::rpc::http::Kind;
use axum::Json;
use axum::extract::{OriginalUri, Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use serde::Deserialize;
use serde_json::json;
use sqlx::Connection as _;

use crate::identity::can_act_on_member;
use crate::{Address, CommunityId, CommunityPermission, CommunityRole, CommunityStanding};

use super::dto::{CommunityMember, CommunityMemberV2Wire, CommunityMemberWire};
use super::friendship::{friendship_status, try_friendship_actions, try_friendship_statuses};
use super::http::{
    CommError, Paginated, get_first, get_pagination_params, handler_error as err_json,
};
use super::profiles::enrich_with_profiles;
use super::standing::{
    CommunityBanAuthority, CommunityUnbanAuthority, load_standing_from_database as load_standing,
    load_standing_pair_from_database,
};
use super::write_support::{
    acquire_membership, load_client_standing, load_client_standing_pair, map_db,
    parse_community_id, refusal_response,
};
use super::{SharedState, queries, signed_path};
use crate::notifications::community::{Audience, now_ms};

macro_rules! try_resp {
    ($e:expr) => {
        match $e {
            Ok(v) => v,
            Err(resp) => return *resp,
        }
    };
}

/// One query for existence, privacy and the viewer's membership: anonymous
/// viewers see a private community as missing; signed non-members are
/// refused; an admin bearer bypasses.
async fn gate_member_listing(
    state: &SharedState,
    id: CommunityId,
    id_str: &str,
    signer: Option<&Address>,
    bypass_privacy: bool,
) -> Result<(), CommError> {
    let only_public = signer.is_none() && !bypass_privacy;
    let (private, viewer_has_row) = queries::members_gate(&state.pool, id, signer)
        .await
        .map_err(super::http::database_error)?
        .filter(|(private, _)| !(only_public && *private))
        .ok_or_else(|| CommError::not_found(format!("Community not found: {id_str}")))?;
    if !bypass_privacy && signer.is_some() && private && !viewer_has_row {
        return Err(CommError::not_authorized(
            "The user doesn't have permission to get community members",
        ));
    }
    Ok(())
}

async fn member_friendship_statuses(
    state: &SharedState,
    signer: Option<&Address>,
    members: &[CommunityMember],
) -> sqlx::Result<HashMap<String, i32>> {
    match signer {
        Some(user) => {
            let addresses: Vec<String> = members
                .iter()
                .map(|m| m.member_address.to_lowercase())
                .collect();
            try_friendship_statuses(&state.pool, user, &addresses).await
        }
        _ => Ok(HashMap::new()),
    }
}

async fn to_member_wire_rows(
    state: &SharedState,
    signer: Option<&Address>,
    members: Vec<CommunityMember>,
) -> sqlx::Result<Vec<CommunityMemberWire>> {
    let addresses: Vec<String> = members.iter().map(|m| m.member_address.clone()).collect();
    let actions = match signer {
        Some(viewer) => try_friendship_actions(&state.pool, viewer, &addresses).await?,
        None => HashMap::new(),
    };
    let profiles = state.profiles.try_get_profiles(&addresses).await?;
    Ok(members
        .into_iter()
        .filter_map(|m| {
            let addr = m.member_address.to_lowercase();
            let info = profiles.get(&addr)?;
            Some(CommunityMemberWire {
                name_color: info.name_color.clone(),
                friendship_status: actions
                    .get(&addr)
                    .zip(signer)
                    .map(|(action, viewer)| action.status(viewer))
                    .unwrap_or(friendship_status::NONE),
                last_friendship_action: signer
                    .map(|_| actions.get(&addr).map(|a| a.action.clone())),
                acting_user: signer.map(|_| actions.get(&addr).map(|a| a.acting_user.clone())),
                base: m,
                name: info.name.clone(),
                profile_picture_url: info.profile_picture_url.clone(),
                has_claimed_name: info.has_claimed_name,
            })
        })
        .collect())
}

async fn to_member_v2_wire_rows(
    state: &SharedState,
    signer: Option<&Address>,
    members: Vec<CommunityMember>,
) -> sqlx::Result<Vec<CommunityMemberV2Wire>> {
    let statuses = member_friendship_statuses(state, signer, &members).await?;
    Ok(members
        .into_iter()
        .map(|m| CommunityMemberV2Wire {
            friendship_status: statuses
                .get(&m.member_address.to_lowercase())
                .copied()
                .unwrap_or(friendship_status::NONE),
            base: m,
        })
        .collect())
}

/// Bans use numeric status in both versions; V1 also retains the SQL aliases.
pub(crate) async fn enrich_with_friendship_status(
    state: &SharedState,
    signer: Option<&Address>,
    rows: &mut [serde_json::Value],
    include_action: bool,
) -> sqlx::Result<()> {
    let actions = match signer {
        Some(user) => {
            let addresses: Vec<String> = rows
                .iter()
                .filter_map(|r| {
                    r.get("memberAddress")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_lowercase())
                })
                .collect();
            try_friendship_actions(&state.pool, user, &addresses).await?
        }
        None => HashMap::new(),
    };
    for row in rows.iter_mut() {
        let Some(obj) = row.as_object_mut() else {
            continue;
        };
        let addr = obj
            .get("memberAddress")
            .and_then(|v| v.as_str())
            .map(|s| s.to_lowercase())
            .unwrap_or_default();
        let status = actions
            .get(&addr)
            .zip(signer)
            .map(|(action, viewer)| action.status(viewer))
            .unwrap_or(friendship_status::NONE);
        if include_action && signer.is_some() {
            obj.insert(
                "lastFriendshipAction".into(),
                json!(actions.get(&addr).map(|a| &a.action)),
            );
            obj.insert(
                "actingUser".into(),
                json!(actions.get(&addr).map(|a| &a.acting_user)),
            );
        }
        obj.insert(
            "friendshipStatus".to_string(),
            serde_json::Value::Number(status.into()),
        );
    }
    Ok(())
}

async fn members_page(
    state: &SharedState,
    id: CommunityId,
    pairs: &[(String, String)],
) -> Result<(Vec<CommunityMember>, i64, super::http::Pagination), CommError> {
    let pagination = get_pagination_params(pairs);
    let only_online = get_first(pairs, "onlyOnline")
        .map(|v| v.eq_ignore_ascii_case("true"))
        .unwrap_or(false);
    let (members, total) = if only_online {
        let online = state.presence.connected_peers().await;
        queries::list_online_members(&state.pool, id, &online, &pagination)
            .await
            .map_err(super::http::database_error)?
    } else {
        queries::list_members(&state.pool, id, &pagination)
            .await
            .map_err(super::http::database_error)?
    };
    Ok((members, total, pagination))
}

pub async fn get_members(
    State(state): State<SharedState>,
    headers: HeaderMap,
    original: OriginalUri,
    Path(id_str): Path<String>,
    Query(pairs): Query<Vec<(String, String)>>,
) -> Result<Json<serde_json::Value>, CommError> {
    let id = CommunityId::parse(&id_str)
        .map_err(|_| CommError::bad_request(super::http::INVALID_IDENTIFIER))?;
    let signer = state
        .optional_signer(
            &headers,
            "get",
            &signed_path(&original, format!("/v1/communities/{id_str}/members")),
        )
        .await;
    gate_member_listing(
        &state,
        id,
        &id_str,
        signer.as_ref(),
        state.admin_bearer(&headers),
    )
    .await?;
    let (members, total, pagination) = members_page(&state, id, &pairs).await?;
    let rows = to_member_wire_rows(&state, signer.as_ref(), members)
        .await
        .map_err(super::http::database_error)?;
    Ok(Json(
        json!({ "data": Paginated::new(rows, total, &pagination) }),
    ))
}

pub async fn get_members_v2(
    State(state): State<SharedState>,
    headers: HeaderMap,
    original: OriginalUri,
    Path(id_str): Path<String>,
    Query(pairs): Query<Vec<(String, String)>>,
) -> Result<Json<serde_json::Value>, CommError> {
    let id = CommunityId::parse(&id_str)
        .map_err(|_| CommError::bad_request(super::http::INVALID_IDENTIFIER))?;
    let signer = state
        .optional_signer(
            &headers,
            "get",
            &signed_path(&original, format!("/v2/communities/{id_str}/members")),
        )
        .await;
    gate_member_listing(
        &state,
        id,
        &id_str,
        signer.as_ref(),
        state.admin_bearer(&headers),
    )
    .await?;
    let (members, total, pagination) = members_page(&state, id, &pairs).await?;
    let rows = to_member_v2_wire_rows(&state, signer.as_ref(), members)
        .await
        .map_err(super::http::database_error)?;
    Ok(Json(
        json!({ "data": Paginated::new(rows, total, &pagination) }),
    ))
}

async fn bans_listing(
    state: &SharedState,
    headers: &HeaderMap,
    original: &OriginalUri,
    id_str: &str,
    version: &str,
    pairs: &[(String, String)],
) -> Result<
    (
        Vec<serde_json::Value>,
        i64,
        super::http::Pagination,
        Address,
    ),
    CommError,
> {
    let signer = state
        .require_signer(
            headers,
            "get",
            &signed_path(original, format!("/{version}/communities/{id_str}/bans")),
        )
        .await?;
    let id = CommunityId::parse(id_str)
        .map_err(|_| CommError::bad_request(super::http::INVALID_IDENTIFIER))?;

    if !queries::community_exists(&state.pool, id, false)
        .await
        .map_err(super::http::database_error)?
    {
        return Err(CommError::not_found(format!(
            "Community not found: {id_str}"
        )));
    }
    let standing = load_standing(&state.pool, id, &signer)
        .await
        .map_err(super::http::database_error)?;
    if !standing.has(CommunityPermission::BanPlayers) {
        return Err(CommError::not_authorized(format!(
            "The user {signer} doesn't have permission to get banned members from the community"
        )));
    }
    let pagination = get_pagination_params(pairs);
    let (bans, total) = queries::list_bans(&state.pool, id, &pagination)
        .await
        .map_err(super::http::database_error)?;
    let rows = bans
        .into_iter()
        .map(|b| serde_json::to_value(b).unwrap_or(serde_json::Value::Null))
        .collect();
    Ok((rows, total, pagination, signer))
}

pub async fn get_bans(
    State(state): State<SharedState>,
    headers: HeaderMap,
    original: OriginalUri,
    Path(id_str): Path<String>,
    Query(pairs): Query<Vec<(String, String)>>,
) -> Result<Json<serde_json::Value>, CommError> {
    let (mut rows, total, pagination, signer) =
        bans_listing(&state, &headers, &original, &id_str, "v1", &pairs).await?;
    // upstream `mapMembersWithProfiles`: the profile fields and the
    // viewer's `friendshipStatus`
    enrich_with_friendship_status(&state, Some(&signer), &mut rows, true)
        .await
        .map_err(super::http::database_error)?;
    enrich_with_profiles(&state.profiles, &mut rows, "memberAddress")
        .await
        .map_err(super::http::database_error)?;
    Ok(Json(
        json!({ "data": Paginated::new(rows, total, &pagination) }),
    ))
}

pub async fn get_bans_v2(
    State(state): State<SharedState>,
    headers: HeaderMap,
    original: OriginalUri,
    Path(id_str): Path<String>,
    Query(pairs): Query<Vec<(String, String)>>,
) -> Result<Json<serde_json::Value>, CommError> {
    let (mut rows, total, pagination, signer) =
        bans_listing(&state, &headers, &original, &id_str, "v2", &pairs).await?;
    enrich_with_friendship_status(&state, Some(&signer), &mut rows, false)
        .await
        .map_err(super::http::database_error)?;
    Ok(Json(
        json!({ "data": Paginated::new(rows, total, &pagination) }),
    ))
}

/// Anyone may list a wallet's public, listed memberships; the wallet itself
/// sees all of them.
pub async fn get_member_communities(
    State(state): State<SharedState>,
    headers: HeaderMap,
    original: OriginalUri,
    Path(address): Path<String>,
    Query(pairs): Query<Vec<(String, String)>>,
) -> Result<Json<serde_json::Value>, CommError> {
    let signer = state
        .optional_signer(
            &headers,
            "get",
            &signed_path(&original, format!("/v1/members/{address}/communities")),
        )
        .await;
    let pagination = get_pagination_params(&pairs);
    // upstream queries with the raw address: one that is not a wallet is a
    // member of nothing (capture `social/member-communities-bad-address`)
    let Ok(member) = Address::parse(&address) else {
        return Ok(Json(
            json!({ "data": Paginated::<serde_json::Value>::new(Vec::new(), 0, &pagination) }),
        ));
    };
    let only_public_visible = signer.as_ref().is_none_or(|s| *s != member);
    let (rows, total) =
        queries::member_communities(&state.pool, &member, &pagination, None, only_public_visible)
            .await
            .map_err(super::http::database_error)?;
    Ok(Json(
        json!({ "data": Paginated::new(rows, total, &pagination) }),
    ))
}

/// The refusal of upstream's `bearerTokenMiddleware` (`@dcl/http-commons`
/// 2.0.1, `bearer-token-middleware.ts`): no or an empty `authorization`
/// header is "Authorization header is missing" (capture
/// `social/managed-communities-401-no-bearer`), any other that is not
/// `Bearer <token>` "Invalid authorization header".
fn bearer_refusal(headers: &HeaderMap) -> CommError {
    let missing = headers
        .get(axum::http::header::AUTHORIZATION)
        .is_none_or(|v| v.is_empty());
    CommError::not_authorized(if missing {
        "Authorization header is missing"
    } else {
        "Invalid authorization header"
    })
}

/// Upstream mounts the bearer routes only when `API_ADMIN_TOKEN` is set.
fn admin_token_configured(state: &SharedState) -> bool {
    state
        .settings
        .admin_token
        .as_deref()
        .is_some_and(|t| !t.is_empty())
}

/// Admin bearer: the communities a wallet owns or moderates. Hidden (404)
/// when no token is configured.
pub async fn get_managed_communities(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(address): Path<String>,
    Query(pairs): Query<Vec<(String, String)>>,
) -> Result<Json<serde_json::Value>, CommError> {
    if !admin_token_configured(&state) {
        return Err(CommError::not_found("Not found"));
    }
    if !state.admin_bearer(&headers) {
        return Err(bearer_refusal(&headers));
    }
    let pagination = get_pagination_params(&pairs);
    let Ok(member) = Address::parse(&address) else {
        return Ok(Json(
            json!({ "data": Paginated::<serde_json::Value>::new(Vec::new(), 0, &pagination) }),
        ));
    };
    let (rows, total) = queries::member_communities(
        &state.pool,
        &member,
        &pagination,
        Some(&["owner", "moderator"]),
        false,
    )
    .await
    .map_err(super::http::database_error)?;
    Ok(Json(
        json!({ "data": Paginated::new(rows, total, &pagination) }),
    ))
}

const MAX_COMMUNITY_IDS: usize = 50;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MemberCommunitiesByIdsBody {
    #[serde(rename = "communityIds")]
    community_ids: Vec<String>,
}

/// Admin bearer: filters a batch of community ids down to the ones the
/// address is a member of (active, not banned), with the role it holds.
/// Hidden (404) when no token is configured.
pub async fn member_communities_by_ids(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(address): Path<String>,
    body: super::http::DeferredBody,
) -> Response {
    if !admin_token_configured(&state) {
        return CommError::not_found("Not found").into_response();
    }
    if !state.admin_bearer(&headers) {
        return bearer_refusal(&headers).into_response();
    }
    let body = match body.schema().await {
        Ok(body) => body,
        Err(error) => return error.into_response(),
    };
    if let Err(error) = crate::rpc::http::body(
        &body,
        &[(
            "communityIds",
            Kind::Strings {
                min: 1,
                max: 50,
                element_max: usize::MAX,
                uuid: true,
            },
        )],
        &["communityIds"],
        0,
    ) {
        return error.into_response();
    }
    if body.is_empty() {
        return err_json(StatusCode::BAD_REQUEST, "communityIds is required");
    }
    let b: MemberCommunitiesByIdsBody = match serde_json::from_slice(&body) {
        Ok(b) => b,
        Err(e) => {
            return err_json(
                StatusCode::BAD_REQUEST,
                format!("invalid MemberCommunitiesByIds body: {e}"),
            );
        }
    };
    if b.community_ids.is_empty() {
        return err_json(
            StatusCode::BAD_REQUEST,
            "communityIds must contain at least 1 item",
        );
    }
    if b.community_ids.len() > MAX_COMMUNITY_IDS {
        return err_json(
            StatusCode::BAD_REQUEST,
            format!("communityIds must contain at most {MAX_COMMUNITY_IDS} items"),
        );
    }
    let mut ids = Vec::with_capacity(b.community_ids.len());
    for s in &b.community_ids {
        match (s.len() == 36).then(|| CommunityId::parse(s)) {
            Some(Ok(u)) => ids.push(u),
            _ => {
                return err_json(
                    StatusCode::BAD_REQUEST,
                    format!("invalid community id: {s}"),
                );
            }
        }
    }
    let Ok(member) = Address::parse(&address) else {
        return (
            StatusCode::OK,
            Json(json!({ "data": { "communities": [] } })),
        )
            .into_response();
    };
    let memberships = match queries::member_communities_by_ids(&state.pool, &ids, &member).await {
        Ok(v) => v,
        Err(e) => return *map_db::<()>(Err(e)).unwrap_err(),
    };
    let communities: Vec<serde_json::Value> = memberships
        .into_iter()
        .map(|(id, role)| json!({ "id": id.to_string(), "role": role }))
        .collect();
    (
        StatusCode::OK,
        Json(json!({ "data": { "communities": communities } })),
    )
        .into_response()
}

async fn signer_or_refusal(
    caller: &super::auth::Caller,
    state: &SharedState,
    headers: &HeaderMap,
    method: &str,
    path: super::auth::SignedPath,
) -> Result<Address, Response> {
    caller
        .require_signer(state, headers, method, &path)
        .await
        .map_err(IntoResponse::into_response)
}

fn parse_address(raw: &str) -> Result<Address, Response> {
    Address::parse(raw).map_err(|_| err_json(StatusCode::BAD_REQUEST, "invalid address"))
}

/// Join a public community. The guards and the insert are one statement;
/// the insert only fires when every guard holds.
pub async fn add_member(
    State(state): State<SharedState>,
    caller: super::auth::Caller,
    headers: HeaderMap,
    original: OriginalUri,
    Path(id): Path<String>,
) -> Response {
    let signer = match signer_or_refusal(
        &caller,
        &state,
        &headers,
        "post",
        signed_path(&original, format!("/v1/communities/{id}/members")),
    )
    .await
    {
        Ok(s) => s,
        Err(r) => return r,
    };
    let uuid = try_resp!(parse_community_id(&id));
    let mut guard = try_resp!(map_db(acquire_membership(&state.pool, uuid).await));
    let mut tx = try_resp!(map_db(guard.begin().await));
    try_resp!(map_db(
        sqlx::query("SELECT id FROM social.communities WHERE id = $1 FOR UPDATE")
            .bind(uuid)
            .execute(&mut *tx)
            .await
    ));
    let gate: Option<(bool, bool, bool, bool, Option<uuid::Uuid>)> = try_resp!(map_db(
        sqlx::query_as(
            "WITH c AS (SELECT active, private FROM social.communities WHERE id = $1), \
                  b AS (SELECT COALESCE(bool_or(active), FALSE) AS banned FROM social.community_bans \
                        WHERE community_id = $1 AND banned_address = $2), \
                  ins AS (INSERT INTO social.community_members (community_id, member_address, role) \
                          SELECT $1, $2, 'member' FROM c, b \
                          WHERE c.active AND NOT c.private AND NOT b.banned \
                          ON CONFLICT (community_id, member_address) DO NOTHING RETURNING community_id), \
                  requests AS (DELETE FROM social.community_requests \
                               WHERE community_id = $1 AND member_address = $2 \
                                 AND EXISTS (SELECT 1 FROM ins) RETURNING id) \
             SELECT c.active, c.private, b.banned, EXISTS (SELECT 1 FROM ins), (SELECT id FROM requests LIMIT 1) FROM c, b",
        )
        .bind(uuid)
        .bind(&signer)
        .fetch_optional(&mut *tx)
        .await
    ));
    let Some((active, private, banned, inserted, request_id)) = gate else {
        return err_json(
            StatusCode::NOT_FOUND,
            format!("Community not found: {uuid}"),
        );
    };
    if !active {
        return CommError::not_found(format!("Community not found: {uuid}")).into_response();
    }
    if private {
        return err_json(
            StatusCode::UNAUTHORIZED,
            format!(
                "Cannot join private community {uuid} directly; a join request or invite is required"
            ),
        );
    }
    if banned {
        return CommError::not_authorized(format!(
            "The user {signer} is banned from community {uuid}"
        ))
        .into_response();
    }
    try_resp!(map_db(tx.commit().await));
    drop(guard);
    if inserted {
        let mut properties = json!({"community_id":id,"user_id":signer.as_str()});
        if let Some(request_id) = request_id {
            properties["request_id"] = json!(request_id);
        }
        state.fire_analytics("JOIN_COMMUNITY", properties);
        // The new membership is visible before the shared fan-out reads its audience.
        state.member_status_changed(uuid, &signer, true).await;
    }
    StatusCode::NO_CONTENT.into_response()
}

/// Leave (self; the owner cannot) or kick (owner kicks anyone but the owner,
/// a moderator kicks members; a non-member target is treated as a member).
pub async fn remove_member(
    State(state): State<SharedState>,
    caller: super::auth::Caller,
    headers: HeaderMap,
    original: OriginalUri,
    Path((id, address)): Path<(String, String)>,
) -> Response {
    let signer = match signer_or_refusal(
        &caller,
        &state,
        &headers,
        "delete",
        signed_path(&original, format!("/v1/communities/{id}/members/{address}")),
    )
    .await
    {
        Ok(s) => s,
        Err(r) => return r,
    };
    let uuid = try_resp!(parse_community_id(&id));
    if !try_resp!(map_db(
        queries::community_exists(&state.pool, uuid, false).await
    )) {
        return CommError::not_found(format!("Community not found: {uuid}")).into_response();
    }
    let target = match parse_address(&address) {
        Ok(a) => a,
        Err(_) => {
            return CommError::bad_request(format!(
                "Invalid address to remove {}",
                address.to_lowercase()
            ))
            .into_response();
        }
    };
    if target == signer {
        let standing = try_resp!(load_client_standing(&state, uuid, &signer).await);
        if standing.tier() == CommunityStanding::Owner {
            return err_json(
                StatusCode::UNAUTHORIZED,
                format!("The owner cannot leave the community {uuid}"),
            );
        }
    } else {
        let (kicker, target_standing) =
            try_resp!(load_client_standing_pair(&state, uuid, &signer, &target).await);
        let target_tier = target_standing.tier();
        let effective_target_tier = if target_tier.is_member() {
            target_tier
        } else {
            CommunityStanding::Member
        };
        if !can_act_on_member(kicker.tier(), effective_target_tier) {
            return err_json(
                StatusCode::UNAUTHORIZED,
                format!(
                    "The user {signer} doesn't have permission to kick {target} from community {uuid}"
                ),
            );
        }
    }
    let removed = !try_resp!(map_db(
        sqlx::query(
            "WITH removed AS (DELETE FROM social.community_members \
                              WHERE community_id = $1 AND member_address = $2 AND role <> 'owner' RETURNING community_id), \
                  likes AS (DELETE FROM social.community_post_likes l \
                            USING social.community_posts p \
                            WHERE l.post_id = p.id AND p.community_id = $1 AND l.user_address = $2 \
                              AND EXISTS (SELECT 1 FROM removed)) \
             SELECT community_id FROM removed"
        )
        .bind(uuid)
        .bind(&target)
        .fetch_all(&state.pool)
        .await
    ))
    .is_empty();
    if removed {
        if target == signer {
            state.fire_analytics(
                "LEAVE_COMMUNITY",
                json!({"community_id":id,"user_id":target.as_str()}),
            );
        } else {
            state.fire_analytics("KICK_MEMBER_FROM_COMMUNITY",json!({"community_id":id,"kicker_user_id":signer.as_str(),"target_user_id":target.as_str()}));
        }
        tracing::info!(community_id = %uuid, member = %target, "community member left");
        state.member_removed_from_worlds(uuid, &target);
        // upstream `leaveCommunity` / `kickMember`: OFFLINE to the members
        // and the wallet that left
        state.member_status_changed(uuid, &target, false).await;
        state
            .evict_from_private_community_voice(uuid, &target)
            .await;
        if target != signer {
            // upstream `kickMember`: MEMBER_REMOVED to the kicked wallet
            state
                .notify_community_event(
                    "community_member_removed",
                    format!("{uuid}-{target}-{}", now_ms()),
                    uuid,
                    json!({}),
                    Audience::One(&target),
                )
                .await;
        }
    }
    StatusCode::NO_CONTENT.into_response()
}

#[derive(Debug, Deserialize)]
pub struct RoleBody {
    pub role: String,
}

/// Owner only (AssignRoles), never on oneself; the one stored spelling of
/// a moderator is written. `owner` is the ownership transfer.
pub async fn update_member_role(
    State(state): State<SharedState>,
    caller: super::auth::Caller,
    headers: HeaderMap,
    original: OriginalUri,
    Path((id, address)): Path<(String, String)>,
    body: super::http::DeferredBody,
) -> Response {
    let signer = match signer_or_refusal(
        &caller,
        &state,
        &headers,
        "patch",
        signed_path(&original, format!("/v1/communities/{id}/members/{address}")),
    )
    .await
    {
        Ok(s) => s,
        Err(r) => return r,
    };
    let body = match body.schema().await {
        Ok(body) => body,
        Err(error) => return error.into_response(),
    };
    if let Err(error) = crate::rpc::http::body(
        &body,
        &[("role", Kind::OneOf(&["owner", "moderator", "member"]))],
        &["role"],
        0,
    ) {
        return error.into_response();
    }
    let uuid = try_resp!(parse_community_id(&id));
    if !try_resp!(map_db(
        queries::community_exists(&state.pool, uuid, false).await
    )) {
        return CommError::not_found(format!("Community not found: {uuid}")).into_response();
    }
    let target = match parse_address(&address) {
        Ok(a) => a,
        Err(_) => {
            return CommError::bad_request("Invalid address provided".to_string()).into_response();
        }
    };
    let parsed: RoleBody = match serde_json::from_slice(&body) {
        Ok(b) => b,
        Err(e) => return err_json(StatusCode::BAD_REQUEST, format!("invalid body: {e}")),
    };
    let Some(new_role) = CommunityRole::parse(&parsed.role) else {
        return err_json(StatusCode::BAD_REQUEST, "invalid role");
    };
    if new_role == CommunityRole::Owner {
        return transfer_ownership(&state, uuid, &signer, &target).await;
    }
    if signer == target {
        return err_json(
            StatusCode::UNAUTHORIZED,
            format!("The user {signer} cannot update their own role in community {uuid}"),
        );
    }
    let (updater, target_standing) =
        try_resp!(load_client_standing_pair(&state, uuid, &signer, &target).await);
    if !updater.has(CommunityPermission::AssignRoles)
        || !can_act_on_member(updater.tier(), target_standing.tier())
    {
        return err_json(
            StatusCode::UNAUTHORIZED,
            format!(
                "The user {signer} doesn't have permission to assign roles in community {uuid}"
            ),
        );
    }
    let upd = try_resp!(map_db(
        sqlx::query("UPDATE social.community_members SET role = $3 WHERE community_id = $1 AND member_address = $2 AND role <> 'owner'")
            .bind(uuid)
            .bind(&target)
            .bind(new_role.as_stored_text())
            .execute(&state.pool)
            .await
    ));
    if upd.rows_affected() == 0 {
        return err_json(StatusCode::NOT_FOUND, "member not found in community");
    }
    StatusCode::NO_CONTENT.into_response()
}

/// Upstream `transferOwnership` (`logic/community/members.ts`, `roles.ts`
/// `validatePermissionToTransferOwnership` at `b6b2cef`): `role: "owner"`
/// hands the community from its owner to a member holding a NAME; the old
/// owner stays as a moderator and the new one is notified.
async fn transfer_ownership(
    state: &SharedState,
    uuid: CommunityId,
    signer: &Address,
    target: &Address,
) -> Response {
    if !try_resp!(map_db(
        queries::community_exists(&state.pool, uuid, false).await
    )) {
        return CommError::not_found(format!("Community not found: {uuid}")).into_response();
    }
    let (owner, target_standing) =
        try_resp!(load_client_standing_pair(state, uuid, signer, target).await);
    if owner.tier() != CommunityStanding::Owner {
        return CommError::not_authorized(format!(
            "The user {signer} doesn't have permission to transfer ownership in community {uuid}"
        ))
        .into_response();
    }
    if !target_standing.tier().is_member() {
        return CommError::not_authorized(format!(
            "The target user {target} is not a member of community {uuid}"
        ))
        .into_response();
    }
    if !match state.has_owned_name(target).await {
        Ok(owns) => owns,
        Err(error) => return error.into_response(),
    } {
        return CommError::bad_request(format!("The user {target} doesn't have any names"))
            .into_response();
    }
    let mut guard = try_resp!(map_db(acquire_membership(&state.pool, uuid).await));
    let mut tx = try_resp!(map_db(guard.begin().await));
    let old_owner: Address = try_resp!(map_db(
        sqlx::query_scalar("SELECT owner_address FROM social.communities WHERE id = $1 FOR UPDATE")
            .bind(uuid)
            .fetch_one(&mut *tx)
            .await
    ));
    if &old_owner != signer {
        // a concurrent transfer moved it after the standing read
        return CommError::not_authorized(format!(
            "The user {signer} doesn't have permission to transfer ownership in community {uuid}"
        ))
        .into_response();
    }
    try_resp!(map_db(
        sqlx::query("UPDATE social.communities SET owner_address = $2, updated_at = timezone('UTC', now()) WHERE id = $1")
            .bind(uuid)
            .bind(target)
            .execute(&mut *tx)
            .await
    ));
    // the old owner's row first: one owner row per community
    // (`community_members_owner_uidx`)
    for (member, role) in [(signer, "moderator"), (target, "owner")] {
        let updated = try_resp!(map_db(
            sqlx::query("UPDATE social.community_members SET role = $3 WHERE community_id = $1 AND member_address = $2")
                .bind(uuid)
                .bind(member)
                .bind(role)
                .execute(&mut *tx)
                .await
        ))
        .rows_affected();
        if updated == 0 {
            // the target left after the standing read; dropping `tx` rolls
            // the owner change back
            return CommError::not_authorized(format!(
                "The target user {target} is not a member of community {uuid}"
            ))
            .into_response();
        }
    }
    try_resp!(map_db(tx.commit().await));
    drop(guard);
    state
        .notify_community_event(
            "community_ownership_transferred",
            format!("{uuid}-{signer}-{target}-{}", now_ms()),
            uuid,
            json!({ "oldOwnerAddress": signer.as_str() }),
            Audience::One(target),
        )
        .await;
    StatusCode::NO_CONTENT.into_response()
}

pub async fn ban_member(
    State(state): State<SharedState>,
    caller: super::auth::Caller,
    headers: HeaderMap,
    original: OriginalUri,
    Path((id, address)): Path<(String, String)>,
) -> Response {
    let signer = match signer_or_refusal(
        &caller,
        &state,
        &headers,
        "post",
        signed_path(
            &original,
            format!("/v1/communities/{id}/members/{address}/bans"),
        ),
    )
    .await
    {
        Ok(s) => s,
        Err(r) => return r,
    };
    let uuid = try_resp!(parse_community_id(&id));
    if !try_resp!(map_db(
        queries::community_exists(&state.pool, uuid, false).await
    )) {
        return CommError::not_found(format!("Community not found: {uuid}")).into_response();
    }
    let target = match parse_address(&address) {
        Ok(a) => a,
        Err(_) => {
            return CommError::bad_request("Invalid member address".to_string()).into_response();
        }
    };
    let mut guard = try_resp!(map_db(acquire_membership(&state.pool, uuid).await));
    let (actor_standing, target_standing) = try_resp!(map_db(
        load_standing_pair_from_database(&mut *guard, uuid, &signer, &target).await
    ));
    if let Err(refusal) = CommunityBanAuthority::from_standings(actor_standing, target_standing) {
        return refusal_response(&refusal, StatusCode::UNAUTHORIZED, || {
            format!(
                "The user {signer} doesn't have permission to ban {target} from community {uuid}"
            )
        });
    }
    // The source commits each stage independently: a later ban/request failure
    // must retain an earlier kick and unlike, including the analytics event.
    let removed: bool = try_resp!(map_db(
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM social.community_members WHERE community_id=$1 AND member_address=$2)")
            .bind(uuid).bind(&target).fetch_one(&mut *guard).await
    ));
    if removed {
        try_resp!(map_db(
            sqlx::query(
                "DELETE FROM social.community_members WHERE community_id=$1 AND member_address=$2"
            )
            .bind(uuid)
            .bind(&target)
            .execute(&mut *guard)
            .await
        ));
        try_resp!(map_db(
            sqlx::query(
                "DELETE FROM social.community_post_likes l USING social.community_posts p \
                         WHERE l.post_id = p.id AND p.community_id = $1 AND l.user_address = $2"
            )
            .bind(uuid)
            .bind(&target)
            .execute(&mut *guard)
            .await
        ));
        state.fire_analytics("BAN_MEMBER_FROM_COMMUNITY",json!({"community_id":id,"banner_user_id":signer.as_str(),"target_user_id":target.as_str()}));
    }
    try_resp!(map_db(
        sqlx::query(
            "INSERT INTO social.community_bans (community_id, banned_address, banned_by, active, banned_at) \
             VALUES ($1, $2, $3, TRUE, timezone('UTC', now())) \
             ON CONFLICT (community_id, banned_address) DO UPDATE \
               SET active = TRUE, unbanned_by = NULL, unbanned_at = NULL",
        )
        .bind(uuid)
        .bind(&target)
        .bind(&signer)
        .execute(&mut *guard)
        .await
    ));
    try_resp!(map_db(
        sqlx::query(
            "DELETE FROM social.community_requests WHERE community_id=$1 AND member_address=$2"
        )
        .bind(uuid)
        .bind(&target)
        .execute(&mut *guard)
        .await
    ));

    drop(guard);
    state
        .evict_from_private_community_voice(uuid, &target)
        .await;
    // upstream `banMember`: OFFLINE to the members and the banned wallet,
    // a member or not
    state.member_status_changed(uuid, &target, false).await;
    if removed {
        state.member_removed_from_worlds(uuid, &target);
        // upstream `banMember`: only a member is told, under a key stable
        // per banned wallet so a repeat ban is the same notification
        state
            .notify_community_event(
                "community_member_banned",
                format!("{uuid}-{target}"),
                uuid,
                json!({}),
                Audience::One(&target),
            )
            .await;
    }
    StatusCode::NO_CONTENT.into_response()
}

pub async fn unban_member(
    State(state): State<SharedState>,
    caller: super::auth::Caller,
    headers: HeaderMap,
    original: OriginalUri,
    Path((id, address)): Path<(String, String)>,
) -> Response {
    let signer = match signer_or_refusal(
        &caller,
        &state,
        &headers,
        "delete",
        signed_path(
            &original,
            format!("/v1/communities/{id}/members/{address}/bans"),
        ),
    )
    .await
    {
        Ok(s) => s,
        Err(r) => return r,
    };
    let uuid = try_resp!(parse_community_id(&id));
    if !try_resp!(map_db(
        queries::community_exists(&state.pool, uuid, false).await
    )) {
        return CommError::not_found(format!("Community not found: {uuid}")).into_response();
    }
    let target = match parse_address(&address) {
        Ok(a) => a,
        Err(_) => {
            return CommError::bad_request("Invalid member address".to_string()).into_response();
        }
    };
    let (actor_standing, target_standing) =
        try_resp!(load_client_standing_pair(&state, uuid, &signer, &target).await);
    if let Err(refusal) = CommunityUnbanAuthority::from_standings(actor_standing, target_standing) {
        return refusal_response(&refusal, StatusCode::UNAUTHORIZED, || {
            format!(
                "The user {signer} doesn't have permission to unban {target} from community {uuid}"
            )
        });
    }
    try_resp!(map_db(
        sqlx::query(
            "UPDATE social.community_bans SET active = FALSE, unbanned_by = $3, unbanned_at = timezone('UTC', now()) \
             WHERE community_id = $1 AND banned_address = $2 AND active",
        )
        .bind(uuid)
        .bind(&target)
        .bind(&signer)
        .execute(&state.pool)
        .await
    ));
    StatusCode::NO_CONTENT.into_response()
}
