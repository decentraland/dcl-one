//! `GET /v1/community-voice-chats/active`: the live community voice rooms
//! the signed caller may see, derived from `social.community_voice_chat_users`
//! (`queries::active_voice_for_user`). `positions`/`worlds` stay empty, as
//! the old service sent them.

use axum::Json;
use axum::extract::{OriginalUri, State};
use axum::http::HeaderMap;

use crate::CommunityId;

use super::dto::{ActiveCommunityVoiceChat, ActiveVoiceChatsData};
use super::http::{CommError, EnvelopeData};
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
    let active: Vec<ActiveCommunityVoiceChat> = rows
        .into_iter()
        .map(|r| ActiveCommunityVoiceChat {
            community_image: r
                .has_thumbnail
                .then(|| state.thumbnail_url(CommunityId::from_uuid(r.community_id))),
            community_id: r.community_id,
            community_name: r.community_name,
            is_member: r.is_member,
            positions: Vec::new(),
            worlds: Vec::new(),
            participant_count: r.participant_count,
            moderator_count: r.moderator_count,
        })
        .collect();
    let total = active.len();
    Ok(Json(EnvelopeData {
        data: ActiveVoiceChatsData {
            active_chats: active,
            total,
        },
    }))
}
