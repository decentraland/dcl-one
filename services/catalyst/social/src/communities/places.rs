//! A community's places (a places-catalogue id or a world name each): the
//! listing follows the private-community rule and answers each place as
//! production does (its destination's fields, [`with_destinations`]),
//! owner/moderator add and remove, optionally checked against the mirrored
//! catalogue's owner.

use axum::Json;
use axum::body::Bytes;
use axum::extract::{OriginalUri, Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::{CommunityId, CommunityPermission};

use super::dto::CommunityPlace;
use super::http::{CommError, Paginated, err_json, get_pagination_params};
use super::posts::require_membership_of_a_private_community;
use super::write_support::{ClientCommunityWriteAuthority, map_db, parse_community_id};
use super::{SharedState, State as CommunitiesState, queries, signed_path};

macro_rules! try_resp {
    ($e:expr) => {
        match $e {
            Ok(v) => v,
            Err(resp) => return *resp,
        }
    };
}

pub async fn get_places(
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
            &signed_path(&original, format!("/v1/communities/{id_str}/places")),
        )
        .await;
    if !queries::community_exists(&state.pool, id, signer.is_none()).await? {
        return Err(CommError::not_found(format!(
            "Community not found: {id_str}"
        )));
    }
    require_membership_of_a_private_community(&state, id, signer.as_ref(), |addr| {
        format!("The user {addr} doesn't have permission to get places from community {id}")
    })
    .await?;
    let pagination = get_pagination_params(&pairs);
    let (places, total) = queries::list_places(&state.pool, id, &pagination).await?;
    let places = with_destinations(&state, id, places).await;
    Ok(Json(
        json!({ "data": Paginated::new(places, total, &pagination) }),
    ))
}

/// The ids upstream sends to the places API (`places.ts:8`): UUID-shaped,
/// in either case.
fn is_uuid_shaped(id: &str) -> bool {
    id.len() == 36
        && id.char_indices().all(|(i, c)| match i {
            8 | 13 | 18 | 23 => c == '-',
            _ => c.is_ascii_hexdigit(),
        })
}

/// Production's community place row (`src/logic/community/places.ts:131-151`
/// at `fb3dcb8`): the UUID-shaped ids and the `*.eth` names of the page go to
/// the destinations lookup as one array (upstream's `POST /api/destinations`
/// body; here explore's [`destinations_by_ids`](crate::explore::places::destinations::destinations_by_ids)
/// in process), and each row takes the fields of the destination whose `id`
/// is exactly its id, after its own. So a `*.eth` row, an id stored in
/// uppercase or a disabled place stays bare. A failed lookup answers the bare
/// rows. `addedBy` and `addedAt` stay on every row (ui3 reads them); explore's
/// own `kind` does not (production's destination has none, nothing here reads
/// it).
async fn with_destinations(
    state: &CommunitiesState,
    community: CommunityId,
    places: Vec<CommunityPlace>,
) -> Vec<Value> {
    let mut rows: Vec<Value> = places
        .iter()
        .map(|p| serde_json::to_value(p).unwrap_or(Value::Null))
        .collect();
    let (uuids, names): (Vec<&str>, Vec<&str>) = (
        places
            .iter()
            .map(|p| p.id.as_str())
            .filter(|id| is_uuid_shaped(id))
            .collect(),
        places
            .iter()
            .map(|p| p.id.as_str())
            .filter(|id| id.ends_with(".eth"))
            .collect(),
    );
    if uuids.is_empty() && names.is_empty() {
        return rows;
    }
    let ids = uuids.into_iter().chain(names).map(str::to_string).collect();
    let destinations = match crate::explore::places::destinations::destinations_by_ids(
        &state.explore,
        ids,
    )
    .await
    {
        Ok(d) => d,
        Err(e) => {
            tracing::warn!(error = %e, %community, "community places: destinations lookup failed; answering the bare rows");
            return rows;
        }
    };
    let mut details: std::collections::HashMap<String, serde_json::Map<String, Value>> =
        std::collections::HashMap::new();
    for d in destinations {
        if let Ok(Value::Object(mut fields)) = serde_json::to_value(&d) {
            fields.remove("kind");
            details.insert(d.id.clone(), fields);
        }
    }
    for (row, place) in rows.iter_mut().zip(&places) {
        if let (Some(fields), Some(obj)) = (details.get(&place.id), row.as_object_mut()) {
            obj.extend(fields.clone());
        }
    }
    rows
}

#[derive(Debug, Deserialize)]
pub struct PlacesBody {
    #[serde(rename = "placeIds", default)]
    pub place_ids: Vec<String>,
}

pub async fn add_places(
    State(state): State<SharedState>,
    headers: HeaderMap,
    original: OriginalUri,
    Path(id): Path<String>,
    body: Bytes,
) -> Response {
    let uuid = try_resp!(parse_community_id(&id));
    let signer = match state
        .require_signer(
            &headers,
            "post",
            &signed_path(&original, format!("/v1/communities/{id}/places")),
        )
        .await
    {
        Ok(s) => s,
        Err(e) => return e.into_response(),
    };
    try_resp!(
        ClientCommunityWriteAuthority::resolve_requiring_capability(
            &state,
            uuid,
            &signer,
            CommunityPermission::AddPlaces,
            "add places to the community"
        )
        .await
    );
    let parsed: PlacesBody = match serde_json::from_slice(&body) {
        Ok(b) => b,
        Err(e) => return err_json(StatusCode::BAD_REQUEST, format!("invalid body: {e}")),
    };
    if parsed.place_ids.is_empty() {
        return err_json(StatusCode::BAD_REQUEST, "placeIds is required");
    }
    if let Err(e) = state
        .validate_places_ownership(&parsed.place_ids, &signer)
        .await
    {
        return e.into_response();
    }
    try_resp!(map_db(
        sqlx::query(
            "INSERT INTO social.community_places (place_id, community_id, added_by) \
             SELECT p, $2, $3 FROM unnest($1::text[]) AS p \
             ON CONFLICT (place_id, community_id) DO NOTHING",
        )
        .bind(&parsed.place_ids)
        .bind(uuid)
        .bind(&signer)
        .execute(&state.pool)
        .await
    ));
    StatusCode::NO_CONTENT.into_response()
}

pub async fn remove_place(
    State(state): State<SharedState>,
    headers: HeaderMap,
    original: OriginalUri,
    Path((id, place_id)): Path<(String, String)>,
) -> Response {
    let uuid = try_resp!(parse_community_id(&id));
    let signer = match state
        .require_signer(
            &headers,
            "delete",
            &signed_path(&original, format!("/v1/communities/{id}/places/{place_id}")),
        )
        .await
    {
        Ok(s) => s,
        Err(e) => return e.into_response(),
    };
    try_resp!(
        ClientCommunityWriteAuthority::resolve_requiring_capability(
            &state,
            uuid,
            &signer,
            CommunityPermission::RemovePlaces,
            "remove places from the community"
        )
        .await
    );
    if let Err(e) = state
        .validate_places_ownership(std::slice::from_ref(&place_id), &signer)
        .await
    {
        return e.into_response();
    }
    let del = try_resp!(map_db(
        sqlx::query(
            "DELETE FROM social.community_places WHERE place_id = $1 AND community_id = $2"
        )
        .bind(&place_id)
        .bind(uuid)
        .execute(&state.pool)
        .await
    ));
    if del.rows_affected() == 0 {
        return err_json(
            StatusCode::NOT_FOUND,
            format!("Place {place_id} not found in community {uuid}"),
        );
    }
    StatusCode::NO_CONTENT.into_response()
}
