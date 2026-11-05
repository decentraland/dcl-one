//! `GET /v1/community-voice-chats/active`: the live community voice rooms
//! the signed caller may see, derived from `social.community_voice_chat_users`
//! (`queries::active_voice_for_user`). As upstream's
//! `getActiveCommunityVoiceChatsForUser` (`logic/community-voice/
//! community-voice.ts` at `b6b2cef`): a member's room carries no
//! `positions`/`worlds`; a non-member's carries those of the community's
//! places and is listed only when it has one.

use axum::Json;
use axum::extract::{OriginalUri, State};
use axum::http::{HeaderMap, StatusCode};

use crate::CommunityId;

use super::dto::{ActiveCommunityVoiceChat, ActiveVoiceChatsData};
use super::http::{CommError, EnvelopeData};
use super::places::{destinations_of, positions_and_worlds};
use super::{SharedState, queries, signed_path};

/// What a community voice STARTED update carries beyond the name (upstream
/// `startCommunityVoiceChat`): the positions and worlds of the community's
/// places and its thumbnail when one is stored. A failed lookup is none, as
/// upstream logs it and goes on.
pub(crate) async fn started_enrichment(
    state: &SharedState,
    id: CommunityId,
) -> (Vec<String>, Vec<String>, Option<String>) {
    let row: Result<(Vec<String>, bool), _> = sqlx::query_as(
        "SELECT ARRAY(SELECT place_id FROM social.community_places WHERE community_id = $1 ORDER BY place_id), \
                EXISTS (SELECT 1 FROM social.community_thumbnails WHERE community_id = $1)",
    )
    .bind(id)
    .fetch_one(&state.pool)
    .await;
    let (place_ids, has_thumbnail) = match row {
        Ok(row) => row,
        Err(e) => {
            tracing::warn!(error = %e, community_id = %id, "voice chat started: places lookup failed");
            return (Vec::new(), Vec::new(), None);
        }
    };
    let image = has_thumbnail.then(|| state.thumbnail_url(id));
    let (positions, worlds) = match destinations_of(state, place_ids.iter().map(String::as_str))
        .await
    {
        Ok(destinations) => positions_and_worlds(&destinations),
        Err(e) => {
            tracing::warn!(error = %e, community_id = %id, "voice chat started: destinations lookup failed");
            (Vec::new(), Vec::new())
        }
    };
    (positions, worlds, image)
}

pub async fn get_active_voice_chats(
    State(state): State<SharedState>,
    headers: HeaderMap,
    original: OriginalUri,
) -> Result<Json<EnvelopeData<ActiveVoiceChatsData>>, CommError> {
    let signer = state
        .require_signer(
            &headers,
            "get",
            &signed_path(&original, "/v1/community-voice-chats/active"),
        )
        .await?;
    let stats = crate::comms::gatekeeper::voice::db::VoiceDb::new(
        state.pool.clone(),
        state.voice_presence_config(),
    )
    .all_active_community_voice_chats()
    .await
    .unwrap_or_else(|error| {
        // The companion voice component catches its SQL failures and returns
        // an empty provider result before Social reads any community rows.
        tracing::warn!(%error,"active voice: comms status SQL unavailable");
        Vec::new()
    });
    if stats.is_empty() {
        return Ok(Json(EnvelopeData {
            data: ActiveVoiceChatsData {
                active_chats: Vec::new(),
                total: 0,
            },
        }));
    }
    let rows = queries::active_voice_for_user(&state.pool, &signer, &stats)
        .await
        .map_err(|error| {
            CommError::MessageOnly(
                StatusCode::INTERNAL_SERVER_ERROR,
                super::http::database_message(&error),
            )
        })?;
    let ids: Vec<uuid::Uuid> = rows.iter().map(|row| row.community_id).collect();
    let nonmembers: Vec<uuid::Uuid> = rows
        .iter()
        .filter(|row| !row.is_member)
        .map(|row| row.community_id)
        .collect();
    let (thumbnails, places) = tokio::join!(
        queries::voice_thumbnail_hashes(&state.pool, &ids),
        queries::voice_place_ids(&state.pool, &nonmembers)
    );
    let thumbnails = thumbnails.unwrap_or_else(|error| {
        tracing::warn!(%error,"active voice thumbnails lookup failed");
        Default::default()
    });
    let places = places.unwrap_or_else(|error| {
        tracing::warn!(%error,"active voice places lookup failed");
        Default::default()
    });
    let mut active: Vec<ActiveCommunityVoiceChat> = Vec::with_capacity(rows.len());
    for r in rows {
        let (positions, worlds) = if r.is_member {
            (Vec::new(), Vec::new())
        } else {
            // upstream `getPlacesWithPositionsAndWorlds`: a failed lookup is
            // no places
            let ids = places.get(&r.community_id).cloned().unwrap_or_default();
            match destinations_of(&state, ids.iter().map(String::as_str)).await {
                Ok(destinations) => positions_and_worlds(&destinations),
                Err(e) => {
                    tracing::warn!(error = %e, community_id = %r.community_id, "active voice chat: destinations lookup failed");
                    (Vec::new(), Vec::new())
                }
            }
        };
        if !r.is_member && positions.is_empty() && worlds.is_empty() {
            continue;
        }
        let community_image = if let Some(hash) = thumbnails.get(&r.community_id) {
            match state.storage.exist(hash).await {
                Ok(true) => Some(state.thumbnail_url(CommunityId::from_uuid(r.community_id))),
                Ok(false) => None,
                Err(error) => {
                    tracing::warn!(%error,community_id=%r.community_id,"active voice thumbnail existence lookup failed");
                    None
                }
            }
        } else {
            None
        };
        active.push(ActiveCommunityVoiceChat {
            community_image,
            community_id: r.community_id,
            community_name: r.community_name,
            is_member: r.is_member,
            positions,
            worlds,
            participant_count: r.participant_count,
            moderator_count: r.moderator_count,
        });
    }
    let total = active.len();
    Ok(Json(EnvelopeData {
        data: ActiveVoiceChatsData {
            active_chats: active,
            total,
        },
    }))
}
