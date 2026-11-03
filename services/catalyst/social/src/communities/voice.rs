//! `GET /v1/community-voice-chats/active`: the live community voice rooms
//! the signed caller may see, derived from `social.community_voice_chat_users`
//! (`queries::active_voice_for_user`). As upstream's
//! `getActiveCommunityVoiceChatsForUser` (`logic/community-voice/
//! community-voice.ts` at `b6b2cef`): a member's room carries no
//! `positions`/`worlds`; a non-member's carries those of the community's
//! places and is listed only when it has one.

use axum::Json;
use axum::extract::{OriginalUri, State};
use axum::http::HeaderMap;

use crate::CommunityId;

use super::dto::{ActiveCommunityVoiceChat, ActiveVoiceChatsData};
use super::http::{CommError, EnvelopeData};
use super::places::{destinations_of, positions_and_worlds};
use super::{SharedState, queries, signed_path};

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
    let rows = queries::active_voice_for_user(&state.pool, &signer).await?;
    let mut active: Vec<ActiveCommunityVoiceChat> = Vec::with_capacity(rows.len());
    for r in rows {
        let (positions, worlds) = if r.is_member {
            (Vec::new(), Vec::new())
        } else {
            // upstream `getPlacesWithPositionsAndWorlds`: a failed lookup is
            // no places
            match destinations_of(&state, r.place_ids.iter().map(String::as_str)).await {
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
        active.push(ActiveCommunityVoiceChat {
            community_image: r
                .has_thumbnail
                .then(|| state.thumbnail_url(CommunityId::from_uuid(r.community_id))),
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
