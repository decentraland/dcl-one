//! Invites (by an owner/moderator) and requests to join (by the wallet, to a
//! private community). At most one pending request per (community, wallet)
//! is a schema invariant (`community_requests_pending_uidx`): an opposite
//! pending type auto-accepts, the same type answers the existing row, a
//! lost race re-reads the row instead of failing.

use axum::Json;
use axum::body::Bytes;
use axum::extract::{OriginalUri, Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use serde::Deserialize;
use serde_json::json;
use uuid::Uuid;

use crate::{Address, CommunityId, CommunityPermission};

use super::http::{CommError, Paginated, err_json, get_first, get_pagination_params};
use super::profiles::enrich_with_profiles;
use super::standing::load_standing;
use super::write_support::{
    Refusal, load_client_standing, map_db, parse_community_id, refusal_response, refuse,
};
use super::{SharedState, queries, signed_path};

macro_rules! try_resp {
    ($e:expr) => {
        match $e {
            Ok(v) => v,
            Err(resp) => return *resp,
        }
    };
}

fn parse_type(v: Option<&str>) -> Option<&'static str> {
    match v {
        Some("invite") => Some("invite"),
        Some("request_to_join") => Some("request_to_join"),
        _ => None,
    }
}

async fn community_requests_page(
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
    let id = CommunityId::parse(id_str)
        .map_err(|_| CommError::not_found(format!("Community not found: {id_str}")))?;
    let signer = state
        .require_signer(
            headers,
            "get",
            &signed_path(
                original,
                format!("/{version}/communities/{id_str}/requests"),
            ),
        )
        .await?;
    let standing = load_standing(&state.pool, id, &signer).await?;
    if !standing.has(CommunityPermission::ViewRequests) {
        return Err(CommError::not_authorized(format!(
            "The user {signer} doesn't have permission to view requests"
        )));
    }
    let pagination = get_pagination_params(pairs);
    let type_filter = parse_type(get_first(pairs, "type").as_deref());
    let (rows, total) =
        queries::list_requests_by_community(&state.pool, id, type_filter, &pagination).await?;
    let json_rows = rows
        .into_iter()
        .map(|r| serde_json::to_value(r).unwrap_or(serde_json::Value::Null))
        .collect();
    Ok((json_rows, total, pagination, signer))
}

pub async fn get_community_requests(
    State(state): State<SharedState>,
    headers: HeaderMap,
    original: OriginalUri,
    Path(id_str): Path<String>,
    Query(pairs): Query<Vec<(String, String)>>,
) -> Result<Json<serde_json::Value>, CommError> {
    let (mut rows, total, pagination, _) =
        community_requests_page(&state, &headers, &original, &id_str, "v1", &pairs).await?;
    enrich_with_profiles(&state.profiles, &mut rows, "memberAddress").await;
    Ok(Json(
        json!({ "data": Paginated::new(rows, total, &pagination) }),
    ))
}

pub async fn get_community_requests_v2(
    State(state): State<SharedState>,
    headers: HeaderMap,
    original: OriginalUri,
    Path(id_str): Path<String>,
    Query(pairs): Query<Vec<(String, String)>>,
) -> Result<Json<serde_json::Value>, CommError> {
    let (mut rows, total, pagination, signer) =
        community_requests_page(&state, &headers, &original, &id_str, "v2", &pairs).await?;
    super::members::enrich_with_friendship_status(&state, Some(&signer), &mut rows).await;
    Ok(Json(
        json!({ "data": Paginated::new(rows, total, &pagination) }),
    ))
}

async fn member_requests_page(
    state: &SharedState,
    headers: &HeaderMap,
    original: &OriginalUri,
    address: &str,
    version: &str,
    pairs: &[(String, String)],
) -> Result<(Vec<serde_json::Value>, i64, super::http::Pagination), CommError> {
    let signer = state
        .require_signer(
            headers,
            "get",
            &signed_path(original, format!("/{version}/members/{address}/requests")),
        )
        .await?;
    if !signer.as_str().eq_ignore_ascii_case(address) {
        return Err(CommError::not_authorized(
            "You are not authorized to get requests for this member",
        ));
    }
    let pagination = get_pagination_params(pairs);
    let type_filter = parse_type(get_first(pairs, "type").as_deref());
    let (mut rows, total) =
        queries::list_requests_by_member(&state.pool, &signer, type_filter, &pagination).await?;
    for row in rows.iter_mut() {
        if let Some(map) = row.as_object_mut() {
            let community_id = map
                .get("communityId")
                .and_then(|v| v.as_str())
                .map(str::to_string)
                .unwrap_or_default();
            map.remove("_hasThumbnail");
            map.insert(
                "thumbnailUrl".to_string(),
                serde_json::Value::String(super::thumbnail_url(
                    &state.settings.cdn_url,
                    &community_id,
                )),
            );
        }
    }
    Ok((rows, total, pagination))
}

pub async fn get_member_requests(
    State(state): State<SharedState>,
    headers: HeaderMap,
    original: OriginalUri,
    Path(address): Path<String>,
    Query(pairs): Query<Vec<(String, String)>>,
) -> Result<Json<serde_json::Value>, CommError> {
    let (mut rows, total, pagination) =
        member_requests_page(&state, &headers, &original, &address, "v1", &pairs).await?;
    let owner_addrs: Vec<String> = rows
        .iter()
        .filter_map(|r| {
            r.get("ownerAddress")
                .and_then(|v| v.as_str())
                .map(str::to_string)
        })
        .collect();
    let owner_names = state.profiles.get_owner_names(&owner_addrs).await;
    for row in rows.iter_mut() {
        if let Some(map) = row.as_object_mut() {
            let owner = map
                .get("ownerAddress")
                .and_then(|v| v.as_str())
                .map(|s| s.to_lowercase())
                .unwrap_or_default();
            map.insert(
                "ownerName".to_string(),
                serde_json::Value::String(owner_names.get(&owner).cloned().unwrap_or_default()),
            );
        }
    }
    Ok(Json(
        json!({ "data": Paginated::new(rows, total, &pagination) }),
    ))
}

pub async fn get_member_requests_v2(
    State(state): State<SharedState>,
    headers: HeaderMap,
    original: OriginalUri,
    Path(address): Path<String>,
    Query(pairs): Query<Vec<(String, String)>>,
) -> Result<Json<serde_json::Value>, CommError> {
    let (rows, total, pagination) =
        member_requests_page(&state, &headers, &original, &address, "v2", &pairs).await?;
    Ok(Json(
        json!({ "data": Paginated::new(rows, total, &pagination) }),
    ))
}

/// The signer's communities the invitee is not in yet.
pub async fn get_invites(
    State(state): State<SharedState>,
    headers: HeaderMap,
    original: OriginalUri,
    Path(invitee): Path<String>,
) -> Result<Json<serde_json::Value>, CommError> {
    let inviter = state
        .require_signer(
            &headers,
            "get",
            &signed_path(&original, format!("/v1/members/{invitee}/invites")),
        )
        .await?;
    let invitee =
        Address::parse(&invitee).map_err(|_| CommError::bad_request("invalid address"))?;
    if inviter == invitee {
        return Err(CommError::bad_request("Users cannot invite themselves"));
    }
    let invites = queries::list_invites(&state.pool, &inviter, &invitee).await?;
    Ok(Json(json!({ "data": invites })))
}

#[derive(Debug, Deserialize)]
pub struct CreateRequestBody {
    #[serde(rename = "targetedAddress", default)]
    pub targeted_address: Option<String>,
    #[serde(rename = "type")]
    pub kind: String,
}

fn request_json(
    id: Uuid,
    community_id: CommunityId,
    member_address: &Address,
    kind: &str,
    status: &str,
) -> Response {
    (
        StatusCode::OK,
        Json(json!({
            "data": {
                "id": id,
                "communityId": community_id.as_uuid(),
                "memberAddress": member_address.as_str(),
                "type": kind,
                "status": status,
            }
        })),
    )
        .into_response()
}

fn is_unique_violation(e: &sqlx::Error) -> bool {
    matches!(e, sqlx::Error::Database(db) if db.code().as_deref() == Some("23505"))
}

pub async fn create_request(
    State(state): State<SharedState>,
    headers: HeaderMap,
    original: OriginalUri,
    Path(id): Path<String>,
    body: Bytes,
) -> Response {
    let community_id = try_resp!(parse_community_id(&id));
    let signer = match state
        .require_signer(
            &headers,
            "post",
            &signed_path(&original, format!("/v1/communities/{id}/requests")),
        )
        .await
    {
        Ok(s) => s,
        Err(e) => return e.into_response(),
    };
    let req: CreateRequestBody = match serde_json::from_slice(&body) {
        Ok(r) => r,
        Err(e) => return err_json(StatusCode::BAD_REQUEST, format!("invalid body: {e}")),
    };
    let kind = match req.kind.as_str() {
        "invite" => "invite",
        "request_to_join" => "request_to_join",
        other => {
            return err_json(
                StatusCode::BAD_REQUEST,
                format!("invalid request type: {other}"),
            );
        }
    };
    let member_address = match req.targeted_address.as_deref() {
        Some(raw) => match Address::parse(raw) {
            Ok(a) => a,
            Err(_) => return err_json(StatusCode::BAD_REQUEST, "invalid targetedAddress"),
        },
        None => signer.clone(),
    };

    let community: Option<(String, bool)> = try_resp!(map_db(
        sqlx::query_as("SELECT name, private FROM social.communities WHERE id = $1 AND active")
            .bind(community_id)
            .fetch_optional(&state.pool)
            .await
    ));
    let Some((community_name, private)) = community else {
        return err_json(StatusCode::NOT_FOUND, format!("Community not found: {id}"));
    };

    if kind == "invite" {
        let caller = match load_standing(&state.pool, community_id, &signer).await {
            Ok(s) => s,
            Err(refusal) => {
                return refusal_response(&refusal, StatusCode::FORBIDDEN, || {
                    "Forbidden: authority could not be established".to_string()
                });
            }
        };
        if !caller.has(CommunityPermission::InviteUsers) {
            return err_json(
                StatusCode::UNAUTHORIZED,
                format!("The user {signer} doesn't have permission to invite users"),
            );
        }
    } else if member_address != signer {
        return err_json(
            StatusCode::BAD_REQUEST,
            "User trying to impersonate another user",
        );
    }
    if !private && kind == "request_to_join" {
        return err_json(
            StatusCode::BAD_REQUEST,
            "Public communities do not accept requests to join",
        );
    }

    let banned: Option<bool> = try_resp!(map_db(
        sqlx::query_scalar("SELECT active FROM social.community_bans WHERE community_id = $1 AND banned_address = $2")
            .bind(community_id)
            .bind(&member_address)
            .fetch_optional(&state.pool)
            .await
    ));
    if banned.unwrap_or(false) {
        return err_json(
            StatusCode::UNAUTHORIZED,
            format!("The user {member_address} is banned from the community {id}"),
        );
    }
    let subject = try_resp!(load_client_standing(&state, community_id, &member_address).await);
    if subject.stored_role_or_none() != "none" {
        return err_json(
            StatusCode::BAD_REQUEST,
            format!(
                "User cannot join since it is already a member of the community: {community_name} ({community_id})"
            ),
        );
    }

    let pending: Vec<(Uuid, String)> = try_resp!(map_db(
        sqlx::query_as(
            "SELECT id, type FROM social.community_requests \
             WHERE community_id = $1 AND member_address = $2 AND status = 'pending' \
             ORDER BY created_at ASC LIMIT 2",
        )
        .bind(community_id)
        .bind(&member_address)
        .fetch_all(&state.pool)
        .await
    ));
    if let Some((rid, _)) = pending.iter().find(|(_, t)| t == kind) {
        return request_json(*rid, community_id, &member_address, kind, "pending");
    }
    if let Some((opp_id, _)) = pending.iter().find(|(_, t)| t != kind) {
        // The two sides met: the pending request of the opposite type is
        // accepted only if a competing decision has not already closed it.
        try_resp!(
            transition_request(
                &state.pool,
                community_id,
                *opp_id,
                &member_address,
                "accepted"
            )
            .await
        );
        return request_json(*opp_id, community_id, &member_address, kind, "accepted");
    }

    let request_id = Uuid::new_v4();
    let inserted = sqlx::query(
        "INSERT INTO social.community_requests (id, community_id, member_address, status, type) \
         VALUES ($1, $2, $3, 'pending', $4)",
    )
    .bind(request_id)
    .bind(community_id)
    .bind(&member_address)
    .bind(kind)
    .execute(&state.pool)
    .await;
    match inserted {
        Ok(_) => request_json(request_id, community_id, &member_address, kind, "pending"),
        Err(e) if is_unique_violation(&e) => {
            // Lost the race against a concurrent request for the same pair:
            // the row that won is the answer.
            let existing: Option<(Uuid, String)> = try_resp!(map_db(
                sqlx::query_as(
                    "SELECT id, type FROM social.community_requests \
                     WHERE community_id = $1 AND member_address = $2 AND status = 'pending'",
                )
                .bind(community_id)
                .bind(&member_address)
                .fetch_optional(&state.pool)
                .await
            ));
            match existing {
                Some((rid, t)) => request_json(
                    rid,
                    community_id,
                    &member_address,
                    if t == "invite" {
                        "invite"
                    } else {
                        "request_to_join"
                    },
                    "pending",
                ),
                None => err_json(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "failed to create request",
                ),
            }
        }
        Err(e) => {
            tracing::error!(error = %e, "failed to create community request");
            err_json(
                StatusCode::INTERNAL_SERVER_ERROR,
                "failed to create request",
            )
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct RequestStatusBody {
    pub intention: String,
}

/// Invites are accepted/rejected by the invitee and cancelled by a manager;
/// requests to join are cancelled by the requester and decided by a
/// manager (AcceptRequests + RejectRequests). Accepting inserts the member
/// row in the same transaction, refused for a banned wallet.
pub async fn update_request_status(
    State(state): State<SharedState>,
    headers: HeaderMap,
    original: OriginalUri,
    Path((id, request_id)): Path<(String, String)>,
    body: Bytes,
) -> Response {
    let uuid = try_resp!(parse_community_id(&id));
    let Ok(req_uuid) = Uuid::parse_str(&request_id) else {
        return err_json(StatusCode::BAD_REQUEST, "invalid request id");
    };
    let signer = match state
        .require_signer(
            &headers,
            "patch",
            &signed_path(
                &original,
                format!("/v1/communities/{id}/requests/{request_id}"),
            ),
        )
        .await
    {
        Ok(s) => s,
        Err(e) => return e.into_response(),
    };
    let parsed: RequestStatusBody = match serde_json::from_slice(&body) {
        Ok(b) => b,
        Err(e) => return err_json(StatusCode::BAD_REQUEST, format!("invalid body: {e}")),
    };
    let status = match parsed.intention.to_lowercase().as_str() {
        "accepted" => "accepted",
        "rejected" => "rejected",
        "cancelled" => "cancelled",
        other => {
            return err_json(
                StatusCode::BAD_REQUEST,
                format!("invalid intention: {other}"),
            );
        }
    };
    let row: Option<(Address, String)> = try_resp!(map_db(
        sqlx::query_as("SELECT member_address, type FROM social.community_requests WHERE id = $1 AND community_id = $2")
            .bind(req_uuid)
            .bind(uuid)
            .fetch_optional(&state.pool)
            .await
    ));
    let Some((member_address, kind)) = row else {
        return err_json(StatusCode::NOT_FOUND, "Request not found");
    };
    let self_caller = member_address == signer;
    let manager_check = || async {
        let standing = load_client_standing(&state, uuid, &signer)
            .await
            .map_err(|e| *e)?;
        if standing.has(CommunityPermission::AcceptRequests)
            && standing.has(CommunityPermission::RejectRequests)
        {
            Ok(())
        } else {
            Err(err_json(
                StatusCode::UNAUTHORIZED,
                format!("The user {signer} doesn't have permission to accept and reject requests"),
            ))
        }
    };
    let auth_err: Option<Response> = match (kind.as_str(), status) {
        ("invite", "cancelled") => {
            if self_caller {
                Some(err_json(
                    StatusCode::UNAUTHORIZED,
                    "Invited user cannot cancel their invite",
                ))
            } else {
                manager_check().await.err()
            }
        }
        ("invite", _) => {
            if self_caller {
                None
            } else {
                Some(err_json(
                    StatusCode::UNAUTHORIZED,
                    "Only invited user can accept or reject invites",
                ))
            }
        }
        ("request_to_join", "cancelled") => {
            if self_caller {
                None
            } else {
                Some(err_json(
                    StatusCode::UNAUTHORIZED,
                    "Only requesting user can cancel their request",
                ))
            }
        }
        ("request_to_join", _) => {
            if self_caller {
                Some(err_json(
                    StatusCode::UNAUTHORIZED,
                    "Requesting user cannot accept or reject their own request",
                ))
            } else {
                manager_check().await.err()
            }
        }
        _ => None,
    };
    if let Some(e) = auth_err {
        return e;
    }
    if status == "accepted" {
        let banned: Option<bool> = try_resp!(map_db(
            sqlx::query_scalar("SELECT active FROM social.community_bans WHERE community_id = $1 AND banned_address = $2")
                .bind(uuid)
                .bind(&member_address)
                .fetch_optional(&state.pool)
                .await
        ));
        if banned.unwrap_or(false) {
            return err_json(
                StatusCode::UNAUTHORIZED,
                format!("The user {member_address} is banned from the community {uuid}"),
            );
        }
    }
    try_resp!(transition_request(&state.pool, uuid, req_uuid, &member_address, status).await);
    StatusCode::NO_CONTENT.into_response()
}

/// A request has one terminal decision. Only the transaction that closes a
/// pending row may insert membership; retries acknowledge without restoring
/// access after a subsequent leave. UPDATE serializes competing decisions on
/// this row, including the opposite-type auto-accept path above.
async fn transition_request(
    pool: &sqlx::PgPool,
    community: CommunityId,
    request: Uuid,
    member: &Address,
    status: &str,
) -> Result<(), Refusal> {
    let mut tx = map_db(pool.begin().await)?;
    let changed = map_db(
        sqlx::query("UPDATE social.community_requests SET status = $2, updated_at = timezone('UTC', now()) WHERE id = $1 AND community_id = $3 AND status = 'pending'")
            .bind(request)
            .bind(status)
            .bind(community)
            .execute(&mut *tx)
            .await,
    )?.rows_affected();
    if changed == 0 {
        // A new READ COMMITTED statement sees the concurrent winner's commit.
        let current: Option<String> = map_db(
            sqlx::query_scalar(
                "SELECT status FROM social.community_requests WHERE id = $1 AND community_id = $2",
            )
            .bind(request)
            .bind(community)
            .fetch_optional(&mut *tx)
            .await,
        )?;
        return match current.as_deref() {
            Some(current) if current == status => Ok(()),
            Some(_) => Err(refuse(StatusCode::CONFLICT, "Request already decided")),
            None => Err(refuse(StatusCode::NOT_FOUND, "Request not found")),
        };
    }
    if status == "accepted" {
        map_db(
            sqlx::query(
                "INSERT INTO social.community_members (community_id, member_address, role) \
                 VALUES ($1, $2, 'member') ON CONFLICT (community_id, member_address) DO NOTHING",
            )
            .bind(community)
            .bind(member)
            .execute(&mut *tx)
            .await,
        )?;
    }
    map_db(tx.commit().await)
}
