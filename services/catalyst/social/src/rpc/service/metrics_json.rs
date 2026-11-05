//! Source decoded-object JSON used only for RPC size/code observations.
//! Keep scalar defaults and omit absent optional fields, as the pinned JS decoder does.
//! Each declaration follows the corresponding protobuf message/oneof.
use serde_json::{Map, Value, json};
fn source_float(value: f64) -> Value {
    if value.is_finite() && value.fract() == 0.0 && value.abs() < 9_007_199_254_740_992.0 {
        json!(value as i64)
    } else {
        json!(value)
    }
}
macro_rules! field_json {
    ($object:ident, $this:ident, $field:ident, $key:literal, scalar) => {
        $object.insert($key.into(), json!($this.$field));
    };
    ($object:ident, $this:ident, $field:ident, $key:literal, float) => {
        $object.insert($key.into(), source_float($this.$field as f64));
    };
    ($object:ident, $this:ident, $field:ident, $key:literal, optional_scalar) => {
        if let Some(value) = &$this.$field {
            $object.insert($key.into(), json!(value));
        }
    };
    ($object:ident, $this:ident, $field:ident, $key:literal, optional_message) => {
        if let Some(value) = &$this.$field {
            $object.insert($key.into(), value.metrics_json());
        }
    };
    ($object:ident, $this:ident, $field:ident, $key:literal, repeated_message) => {
        $object.insert(
            $key.into(),
            Value::Array(
                $this
                    .$field
                    .iter()
                    .map(|value| value.metrics_json())
                    .collect(),
            ),
        );
    };
    ($object:ident, $this:ident, $field:ident, $key:literal, repeated_scalar) => {
        $object.insert($key.into(), json!($this.$field));
    };
}
macro_rules! message_json {
    ($ty:path; $($field:ident: $key:literal => $kind:ident),* $(,)?) => {
        impl $ty {
            #[allow(dead_code)]
            pub(crate) fn metrics_json(&self) -> Value {
                let mut object = Map::new();
                $(field_json!(object, self, $field, $key, $kind);)*
                Value::Object(object)
            }
        }
    };
}
macro_rules! oneof_json {
    ($ty:path; $($variant:ident => $key:literal),* $(,)?) => {
        impl $ty {
            #[allow(dead_code)]
            pub(crate) fn metrics_json(&self) -> Value {
                match self { $(Self::$variant(value) => json!({"$case": $key, $key: value.metrics_json()})),* }
            }
        }
    };
}

message_json!(crate::rpc::proto::decentraland::common::Color3; r: "r" => float, g: "g" => float, b: "b" => float);
message_json!(crate::rpc::proto::decentraland::common::Color4; r: "r" => float, g: "g" => float, b: "b" => float, a: "a" => float);
message_json!(crate::rpc::proto::decentraland::common::ColorRange; start: "start" => optional_message, end: "end" => optional_message);
message_json!(crate::rpc::proto::decentraland::social_service::InvalidFriendshipAction; message: "message" => optional_scalar);
message_json!(crate::rpc::proto::decentraland::social_service::InternalServerError; message: "message" => optional_scalar);
message_json!(crate::rpc::proto::decentraland::social_service::InvalidRequest; message: "message" => optional_scalar);
message_json!(crate::rpc::proto::decentraland::social_service::ProfileNotFound; message: "message" => optional_scalar);
message_json!(crate::rpc::proto::decentraland::social_service::ConflictingError; message: "message" => optional_scalar);
message_json!(crate::rpc::proto::decentraland::social_service::ForbiddenError; message: "message" => optional_scalar);
message_json!(crate::rpc::proto::decentraland::social_service::NotFoundError; message: "message" => optional_scalar);
message_json!(crate::rpc::proto::decentraland::social_service::v2::User; address: "address" => scalar);
message_json!(crate::rpc::proto::decentraland::social_service::v2::FriendProfile; address: "address" => scalar, name: "name" => scalar, has_claimed_name: "hasClaimedName" => scalar, profile_picture_url: "profilePictureUrl" => scalar, name_color: "nameColor" => optional_message);
message_json!(crate::rpc::proto::decentraland::social_service::v2::BlockedUserProfile; address: "address" => scalar, name: "name" => scalar, has_claimed_name: "hasClaimedName" => scalar, profile_picture_url: "profilePictureUrl" => scalar, blocked_at: "blockedAt" => optional_scalar, name_color: "nameColor" => optional_message);
message_json!(crate::rpc::proto::decentraland::social_service::v2::Pagination; limit: "limit" => scalar, offset: "offset" => scalar);
message_json!(crate::rpc::proto::decentraland::social_service::v2::FriendshipRequestResponse; friend: "friend" => optional_message, created_at: "createdAt" => scalar, message: "message" => optional_scalar, id: "id" => scalar);
message_json!(crate::rpc::proto::decentraland::social_service::v2::FriendshipRequests; requests: "requests" => repeated_message);
message_json!(crate::rpc::proto::decentraland::social_service::v2::GetFriendsPayload; pagination: "pagination" => optional_message);
message_json!(crate::rpc::proto::decentraland::social_service::v2::GetFriendshipRequestsPayload; pagination: "pagination" => optional_message);
message_json!(crate::rpc::proto::decentraland::social_service::v2::UpsertFriendshipPayload; action: "action" => optional_message);
message_json!(crate::rpc::proto::decentraland::social_service::v2::upsert_friendship_payload::RequestPayload; user: "user" => optional_message, message: "message" => optional_scalar);
message_json!(crate::rpc::proto::decentraland::social_service::v2::upsert_friendship_payload::AcceptPayload; user: "user" => optional_message);
message_json!(crate::rpc::proto::decentraland::social_service::v2::upsert_friendship_payload::RejectPayload; user: "user" => optional_message);
message_json!(crate::rpc::proto::decentraland::social_service::v2::upsert_friendship_payload::DeletePayload; user: "user" => optional_message);
message_json!(crate::rpc::proto::decentraland::social_service::v2::upsert_friendship_payload::CancelPayload; user: "user" => optional_message);
oneof_json!(crate::rpc::proto::decentraland::social_service::v2::upsert_friendship_payload::Action; Request => "request", Accept => "accept", Reject => "reject", Delete => "delete", Cancel => "cancel");
message_json!(crate::rpc::proto::decentraland::social_service::v2::GetMutualFriendsPayload; user: "user" => optional_message, pagination: "pagination" => optional_message);
message_json!(crate::rpc::proto::decentraland::social_service::v2::PaginatedResponse; total: "total" => scalar, page: "page" => scalar);
message_json!(crate::rpc::proto::decentraland::social_service::v2::PaginatedFriendsProfilesResponse; friends: "friends" => repeated_message, pagination_data: "paginationData" => optional_message);
message_json!(crate::rpc::proto::decentraland::social_service::v2::PaginatedFriendshipRequestsResponse; pagination_data: "paginationData" => optional_message, response: "response" => optional_message);
oneof_json!(crate::rpc::proto::decentraland::social_service::v2::paginated_friendship_requests_response::Response; Requests => "requests", InternalServerError => "internalServerError");
message_json!(crate::rpc::proto::decentraland::social_service::v2::UpsertFriendshipResponse; response: "response" => optional_message);
message_json!(crate::rpc::proto::decentraland::social_service::v2::upsert_friendship_response::Accepted; id: "id" => scalar, created_at: "createdAt" => scalar, friend: "friend" => optional_message, message: "message" => optional_scalar);
oneof_json!(crate::rpc::proto::decentraland::social_service::v2::upsert_friendship_response::Response; Accepted => "accepted", InvalidFriendshipAction => "invalidFriendshipAction", InternalServerError => "internalServerError", InvalidRequest => "invalidRequest");
message_json!(crate::rpc::proto::decentraland::social_service::v2::SubscriptionStreamClosed; reason: "reason" => scalar, message: "message" => optional_scalar);
message_json!(crate::rpc::proto::decentraland::social_service::v2::FriendshipUpdate; stream_closed: "streamClosed" => optional_message, update: "update" => optional_message);
message_json!(crate::rpc::proto::decentraland::social_service::v2::friendship_update::RequestResponse; friend: "friend" => optional_message, created_at: "createdAt" => scalar, message: "message" => optional_scalar, id: "id" => scalar);
message_json!(crate::rpc::proto::decentraland::social_service::v2::friendship_update::AcceptResponse; user: "user" => optional_message);
message_json!(crate::rpc::proto::decentraland::social_service::v2::friendship_update::RejectResponse; user: "user" => optional_message);
message_json!(crate::rpc::proto::decentraland::social_service::v2::friendship_update::DeleteResponse; user: "user" => optional_message);
message_json!(crate::rpc::proto::decentraland::social_service::v2::friendship_update::CancelResponse; user: "user" => optional_message);
message_json!(crate::rpc::proto::decentraland::social_service::v2::friendship_update::BlockResponse; user: "user" => optional_message);
oneof_json!(crate::rpc::proto::decentraland::social_service::v2::friendship_update::Update; Request => "request", Accept => "accept", Reject => "reject", Delete => "delete", Cancel => "cancel", Block => "block");
message_json!(crate::rpc::proto::decentraland::social_service::v2::FriendConnectivityUpdate; friend: "friend" => optional_message, status: "status" => scalar, stream_closed: "streamClosed" => optional_message);
message_json!(crate::rpc::proto::decentraland::social_service::v2::GetFriendshipStatusPayload; user: "user" => optional_message);
message_json!(crate::rpc::proto::decentraland::social_service::v2::GetFriendshipStatusResponse; response: "response" => optional_message);
message_json!(crate::rpc::proto::decentraland::social_service::v2::get_friendship_status_response::Ok; status: "status" => scalar, message: "message" => optional_scalar);
oneof_json!(crate::rpc::proto::decentraland::social_service::v2::get_friendship_status_response::Response; Accepted => "accepted", InternalServerError => "internalServerError", InvalidRequest => "invalidRequest");
message_json!(crate::rpc::proto::decentraland::social_service::v2::BlockUserPayload; user: "user" => optional_message);
message_json!(crate::rpc::proto::decentraland::social_service::v2::BlockUserResponse; response: "response" => optional_message);
message_json!(crate::rpc::proto::decentraland::social_service::v2::block_user_response::Ok; profile: "profile" => optional_message);
oneof_json!(crate::rpc::proto::decentraland::social_service::v2::block_user_response::Response; Ok => "ok", InternalServerError => "internalServerError", InvalidRequest => "invalidRequest", ProfileNotFound => "profileNotFound");
message_json!(crate::rpc::proto::decentraland::social_service::v2::UnblockUserPayload; user: "user" => optional_message);
message_json!(crate::rpc::proto::decentraland::social_service::v2::UnblockUserResponse; response: "response" => optional_message);
message_json!(crate::rpc::proto::decentraland::social_service::v2::unblock_user_response::Ok; profile: "profile" => optional_message);
oneof_json!(crate::rpc::proto::decentraland::social_service::v2::unblock_user_response::Response; Ok => "ok", InternalServerError => "internalServerError", InvalidRequest => "invalidRequest", ProfileNotFound => "profileNotFound");
message_json!(crate::rpc::proto::decentraland::social_service::v2::SocialSettings; private_messages_privacy: "privateMessagesPrivacy" => scalar, blocked_users_messages_visibility: "blockedUsersMessagesVisibility" => scalar, show_situation_reactions: "showSituationReactions" => scalar);
message_json!(crate::rpc::proto::decentraland::social_service::v2::GetSocialSettingsResponse; response: "response" => optional_message);
message_json!(crate::rpc::proto::decentraland::social_service::v2::get_social_settings_response::Ok; settings: "settings" => optional_message);
oneof_json!(crate::rpc::proto::decentraland::social_service::v2::get_social_settings_response::Response; Ok => "ok", InternalServerError => "internalServerError");
message_json!(crate::rpc::proto::decentraland::social_service::v2::UpsertSocialSettingsPayload; private_messages_privacy: "privateMessagesPrivacy" => optional_scalar, blocked_users_messages_visibility: "blockedUsersMessagesVisibility" => optional_scalar, show_situation_reactions: "showSituationReactions" => optional_scalar);
message_json!(crate::rpc::proto::decentraland::social_service::v2::UpsertSocialSettingsResponse; response: "response" => optional_message);
oneof_json!(crate::rpc::proto::decentraland::social_service::v2::upsert_social_settings_response::Response; Ok => "ok", InternalServerError => "internalServerError", InvalidRequest => "invalidRequest");
message_json!(crate::rpc::proto::decentraland::social_service::v2::GetPrivateMessagesSettingsPayload; user: "user" => repeated_message);
message_json!(crate::rpc::proto::decentraland::social_service::v2::GetPrivateMessagesSettingsResponse; response: "response" => optional_message);
message_json!(crate::rpc::proto::decentraland::social_service::v2::get_private_messages_settings_response::PrivateMessagesSettings; user: "user" => optional_message, private_messages_privacy: "privateMessagesPrivacy" => scalar, is_friend: "isFriend" => scalar);
message_json!(crate::rpc::proto::decentraland::social_service::v2::get_private_messages_settings_response::Ok; settings: "settings" => repeated_message);
oneof_json!(crate::rpc::proto::decentraland::social_service::v2::get_private_messages_settings_response::Response; Ok => "ok", InternalServerError => "internalServerError", InvalidRequest => "invalidRequest", ProfileNotFound => "profileNotFound");
message_json!(crate::rpc::proto::decentraland::social_service::v2::GetBlockedUsersPayload; pagination: "pagination" => optional_message);
message_json!(crate::rpc::proto::decentraland::social_service::v2::GetBlockedUsersResponse; profiles: "profiles" => repeated_message, pagination_data: "paginationData" => optional_message);
message_json!(crate::rpc::proto::decentraland::social_service::v2::GetBlockingStatusResponse; blocked_users: "blockedUsers" => repeated_scalar, blocked_by_users: "blockedByUsers" => repeated_scalar);
message_json!(crate::rpc::proto::decentraland::social_service::v2::BlockUpdate; address: "address" => scalar, is_blocked: "isBlocked" => scalar, stream_closed: "streamClosed" => optional_message);
message_json!(crate::rpc::proto::decentraland::social_service::v2::CommunityMemberConnectivityUpdate; community_id: "communityId" => scalar, member: "member" => optional_message, status: "status" => scalar, stream_closed: "streamClosed" => optional_message);
message_json!(crate::rpc::proto::decentraland::social_service::v2::StartPrivateVoiceChatPayload; callee: "callee" => optional_message);
message_json!(crate::rpc::proto::decentraland::social_service::v2::StartPrivateVoiceChatResponse; response: "response" => optional_message);
message_json!(crate::rpc::proto::decentraland::social_service::v2::start_private_voice_chat_response::Ok; call_id: "callId" => scalar);
oneof_json!(crate::rpc::proto::decentraland::social_service::v2::start_private_voice_chat_response::Response; Ok => "ok", InternalServerError => "internalServerError", InvalidRequest => "invalidRequest", ConflictingError => "conflictingError", ForbiddenError => "forbiddenError");
message_json!(crate::rpc::proto::decentraland::social_service::v2::AcceptPrivateVoiceChatPayload; call_id: "callId" => scalar);
message_json!(crate::rpc::proto::decentraland::social_service::v2::AcceptPrivateVoiceChatResponse; response: "response" => optional_message);
message_json!(crate::rpc::proto::decentraland::social_service::v2::accept_private_voice_chat_response::Ok; call_id: "callId" => scalar, credentials: "credentials" => optional_message);
oneof_json!(crate::rpc::proto::decentraland::social_service::v2::accept_private_voice_chat_response::Response; Ok => "ok", InternalServerError => "internalServerError", InvalidRequest => "invalidRequest", NotFound => "notFound", ForbiddenError => "forbiddenError");
message_json!(crate::rpc::proto::decentraland::social_service::v2::RejectPrivateVoiceChatPayload; call_id: "callId" => scalar);
message_json!(crate::rpc::proto::decentraland::social_service::v2::RejectPrivateVoiceChatResponse; response: "response" => optional_message);
message_json!(crate::rpc::proto::decentraland::social_service::v2::reject_private_voice_chat_response::Ok; call_id: "callId" => scalar);
oneof_json!(crate::rpc::proto::decentraland::social_service::v2::reject_private_voice_chat_response::Response; Ok => "ok", InternalServerError => "internalServerError", InvalidRequest => "invalidRequest", NotFound => "notFound");
message_json!(crate::rpc::proto::decentraland::social_service::v2::PrivateVoiceChatCredentials; connection_url: "connectionUrl" => scalar);
message_json!(crate::rpc::proto::decentraland::social_service::v2::PrivateVoiceChatUpdate; call_id: "callId" => scalar, status: "status" => scalar, caller: "caller" => optional_message, callee: "callee" => optional_message, credentials: "credentials" => optional_message, stream_closed: "streamClosed" => optional_message);
message_json!(crate::rpc::proto::decentraland::social_service::v2::EndPrivateVoiceChatPayload; call_id: "callId" => scalar);
message_json!(crate::rpc::proto::decentraland::social_service::v2::EndPrivateVoiceChatResponse; response: "response" => optional_message);
message_json!(crate::rpc::proto::decentraland::social_service::v2::end_private_voice_chat_response::Ok; call_id: "callId" => scalar);
oneof_json!(crate::rpc::proto::decentraland::social_service::v2::end_private_voice_chat_response::Response; Ok => "ok", InternalServerError => "internalServerError", NotFound => "notFound");
message_json!(crate::rpc::proto::decentraland::social_service::v2::GetIncomingPrivateVoiceChatRequestResponse; response: "response" => optional_message);
message_json!(crate::rpc::proto::decentraland::social_service::v2::get_incoming_private_voice_chat_request_response::Ok; caller: "caller" => optional_message, call_id: "callId" => scalar);
oneof_json!(crate::rpc::proto::decentraland::social_service::v2::get_incoming_private_voice_chat_request_response::Response; Ok => "ok", NotFound => "notFound", InternalServerError => "internalServerError");
message_json!(crate::rpc::proto::decentraland::social_service::v2::CommunityVoiceChatCredentials; connection_url: "connectionUrl" => scalar);
message_json!(crate::rpc::proto::decentraland::social_service::v2::StartCommunityVoiceChatPayload; community_id: "communityId" => scalar);
message_json!(crate::rpc::proto::decentraland::social_service::v2::StartCommunityVoiceChatResponse; response: "response" => optional_message);
message_json!(crate::rpc::proto::decentraland::social_service::v2::start_community_voice_chat_response::Ok; credentials: "credentials" => optional_message);
oneof_json!(crate::rpc::proto::decentraland::social_service::v2::start_community_voice_chat_response::Response; Ok => "ok", InvalidRequest => "invalidRequest", ForbiddenError => "forbiddenError", ConflictingError => "conflictingError", InternalServerError => "internalServerError");
message_json!(crate::rpc::proto::decentraland::social_service::v2::JoinCommunityVoiceChatPayload; community_id: "communityId" => scalar);
message_json!(crate::rpc::proto::decentraland::social_service::v2::JoinCommunityVoiceChatResponse; response: "response" => optional_message);
message_json!(crate::rpc::proto::decentraland::social_service::v2::join_community_voice_chat_response::Ok; voice_chat_id: "voiceChatId" => scalar, credentials: "credentials" => optional_message);
oneof_json!(crate::rpc::proto::decentraland::social_service::v2::join_community_voice_chat_response::Response; Ok => "ok", InvalidRequest => "invalidRequest", ForbiddenError => "forbiddenError", NotFoundError => "notFoundError", ConflictingError => "conflictingError", InternalServerError => "internalServerError");
message_json!(crate::rpc::proto::decentraland::social_service::v2::RequestToSpeakInCommunityVoiceChatPayload; community_id: "communityId" => scalar, is_raising_hand: "isRaisingHand" => scalar);
message_json!(crate::rpc::proto::decentraland::social_service::v2::RequestToSpeakInCommunityVoiceChatResponse; response: "response" => optional_message);
message_json!(crate::rpc::proto::decentraland::social_service::v2::request_to_speak_in_community_voice_chat_response::Ok; message: "message" => scalar);
oneof_json!(crate::rpc::proto::decentraland::social_service::v2::request_to_speak_in_community_voice_chat_response::Response; Ok => "ok", InvalidRequest => "invalidRequest", ForbiddenError => "forbiddenError", NotFoundError => "notFoundError", InternalServerError => "internalServerError");
message_json!(crate::rpc::proto::decentraland::social_service::v2::PromoteSpeakerInCommunityVoiceChatPayload; community_id: "communityId" => scalar, user_address: "userAddress" => scalar);
message_json!(crate::rpc::proto::decentraland::social_service::v2::PromoteSpeakerInCommunityVoiceChatResponse; response: "response" => optional_message);
message_json!(crate::rpc::proto::decentraland::social_service::v2::promote_speaker_in_community_voice_chat_response::Ok; message: "message" => scalar);
oneof_json!(crate::rpc::proto::decentraland::social_service::v2::promote_speaker_in_community_voice_chat_response::Response; Ok => "ok", InvalidRequest => "invalidRequest", ForbiddenError => "forbiddenError", NotFoundError => "notFoundError", InternalServerError => "internalServerError");
message_json!(crate::rpc::proto::decentraland::social_service::v2::DemoteSpeakerInCommunityVoiceChatPayload; community_id: "communityId" => scalar, user_address: "userAddress" => scalar);
message_json!(crate::rpc::proto::decentraland::social_service::v2::DemoteSpeakerInCommunityVoiceChatResponse; response: "response" => optional_message);
message_json!(crate::rpc::proto::decentraland::social_service::v2::demote_speaker_in_community_voice_chat_response::Ok; message: "message" => scalar);
oneof_json!(crate::rpc::proto::decentraland::social_service::v2::demote_speaker_in_community_voice_chat_response::Response; Ok => "ok", InvalidRequest => "invalidRequest", ForbiddenError => "forbiddenError", NotFoundError => "notFoundError", InternalServerError => "internalServerError");
message_json!(crate::rpc::proto::decentraland::social_service::v2::KickPlayerFromCommunityVoiceChatPayload; community_id: "communityId" => scalar, user_address: "userAddress" => scalar);
message_json!(crate::rpc::proto::decentraland::social_service::v2::KickPlayerFromCommunityVoiceChatResponse; response: "response" => optional_message);
message_json!(crate::rpc::proto::decentraland::social_service::v2::kick_player_from_community_voice_chat_response::Ok; message: "message" => scalar);
oneof_json!(crate::rpc::proto::decentraland::social_service::v2::kick_player_from_community_voice_chat_response::Response; Ok => "ok", InvalidRequest => "invalidRequest", ForbiddenError => "forbiddenError", NotFoundError => "notFoundError", InternalServerError => "internalServerError");
message_json!(crate::rpc::proto::decentraland::social_service::v2::RejectSpeakRequestInCommunityVoiceChatPayload; community_id: "communityId" => scalar, user_address: "userAddress" => scalar);
message_json!(crate::rpc::proto::decentraland::social_service::v2::RejectSpeakRequestInCommunityVoiceChatResponse; response: "response" => optional_message);
message_json!(crate::rpc::proto::decentraland::social_service::v2::reject_speak_request_in_community_voice_chat_response::Ok; message: "message" => scalar);
oneof_json!(crate::rpc::proto::decentraland::social_service::v2::reject_speak_request_in_community_voice_chat_response::Response; Ok => "ok", InvalidRequest => "invalidRequest", ForbiddenError => "forbiddenError", NotFoundError => "notFoundError", InternalServerError => "internalServerError");
message_json!(crate::rpc::proto::decentraland::social_service::v2::MuteSpeakerFromCommunityVoiceChatPayload; community_id: "communityId" => scalar, user_address: "userAddress" => scalar, muted: "muted" => scalar);
message_json!(crate::rpc::proto::decentraland::social_service::v2::MuteSpeakerFromCommunityVoiceChatResponse; response: "response" => optional_message);
message_json!(crate::rpc::proto::decentraland::social_service::v2::mute_speaker_from_community_voice_chat_response::Ok; muted: "muted" => scalar);
oneof_json!(crate::rpc::proto::decentraland::social_service::v2::mute_speaker_from_community_voice_chat_response::Response; Ok => "ok", ForbiddenError => "forbiddenError", NotFoundError => "notFoundError", InvalidRequest => "invalidRequest", InternalServerError => "internalServerError");
message_json!(crate::rpc::proto::decentraland::social_service::v2::EndCommunityVoiceChatPayload; community_id: "communityId" => scalar);
message_json!(crate::rpc::proto::decentraland::social_service::v2::EndCommunityVoiceChatResponse; response: "response" => optional_message);
message_json!(crate::rpc::proto::decentraland::social_service::v2::end_community_voice_chat_response::Ok; message: "message" => scalar);
oneof_json!(crate::rpc::proto::decentraland::social_service::v2::end_community_voice_chat_response::Response; Ok => "ok", InvalidRequest => "invalidRequest", ForbiddenError => "forbiddenError", NotFoundError => "notFoundError", InternalServerError => "internalServerError");
message_json!(crate::rpc::proto::decentraland::social_service::v2::CommunityVoiceChatUpdate; community_id: "communityId" => scalar, created_at: "createdAt" => scalar, status: "status" => scalar, ended_at: "endedAt" => optional_scalar, positions: "positions" => repeated_scalar, is_member: "isMember" => scalar, community_name: "communityName" => scalar, community_image: "communityImage" => optional_scalar, worlds: "worlds" => repeated_scalar, stream_closed: "streamClosed" => optional_message);
#[cfg(test)]
mod tests {
    use super::*;
    use prost::Message;
    #[test]
    fn decoded_payload_json_matches_every_captured_source_type_and_variant() {
        let capture: Value = serde_json::from_str(include_str!(concat!(env!("DCL_ONE_ROOT"), "/services/catalyst/tests/fixtures/conformance/social/application-metrics-source-cases.json"))).unwrap();
        for case in capture["jsonCases"].as_array().unwrap() {
            let bytes = hex::decode(case["hex"].as_str().unwrap()).unwrap();
            let actual = match case["type"].as_str().unwrap() {
            "Color3" => crate::rpc::proto::decentraland::common::Color3::decode(bytes.as_slice()).unwrap().metrics_json(),
            "Color4" => crate::rpc::proto::decentraland::common::Color4::decode(bytes.as_slice()).unwrap().metrics_json(),
            "ColorRange" => crate::rpc::proto::decentraland::common::ColorRange::decode(bytes.as_slice()).unwrap().metrics_json(),
            "InvalidFriendshipAction" => crate::rpc::proto::decentraland::social_service::InvalidFriendshipAction::decode(bytes.as_slice()).unwrap().metrics_json(),
            "InternalServerError" => crate::rpc::proto::decentraland::social_service::InternalServerError::decode(bytes.as_slice()).unwrap().metrics_json(),
            "InvalidRequest" => crate::rpc::proto::decentraland::social_service::InvalidRequest::decode(bytes.as_slice()).unwrap().metrics_json(),
            "ProfileNotFound" => crate::rpc::proto::decentraland::social_service::ProfileNotFound::decode(bytes.as_slice()).unwrap().metrics_json(),
            "ConflictingError" => crate::rpc::proto::decentraland::social_service::ConflictingError::decode(bytes.as_slice()).unwrap().metrics_json(),
            "ForbiddenError" => crate::rpc::proto::decentraland::social_service::ForbiddenError::decode(bytes.as_slice()).unwrap().metrics_json(),
            "NotFoundError" => crate::rpc::proto::decentraland::social_service::NotFoundError::decode(bytes.as_slice()).unwrap().metrics_json(),
            "User" => crate::rpc::proto::decentraland::social_service::v2::User::decode(bytes.as_slice()).unwrap().metrics_json(),
            "FriendProfile" => crate::rpc::proto::decentraland::social_service::v2::FriendProfile::decode(bytes.as_slice()).unwrap().metrics_json(),
            "BlockedUserProfile" => crate::rpc::proto::decentraland::social_service::v2::BlockedUserProfile::decode(bytes.as_slice()).unwrap().metrics_json(),
            "Pagination" => crate::rpc::proto::decentraland::social_service::v2::Pagination::decode(bytes.as_slice()).unwrap().metrics_json(),
            "FriendshipRequestResponse" => crate::rpc::proto::decentraland::social_service::v2::FriendshipRequestResponse::decode(bytes.as_slice()).unwrap().metrics_json(),
            "FriendshipRequests" => crate::rpc::proto::decentraland::social_service::v2::FriendshipRequests::decode(bytes.as_slice()).unwrap().metrics_json(),
            "GetFriendsPayload" => crate::rpc::proto::decentraland::social_service::v2::GetFriendsPayload::decode(bytes.as_slice()).unwrap().metrics_json(),
            "GetFriendshipRequestsPayload" => crate::rpc::proto::decentraland::social_service::v2::GetFriendshipRequestsPayload::decode(bytes.as_slice()).unwrap().metrics_json(),
            "UpsertFriendshipPayload" => crate::rpc::proto::decentraland::social_service::v2::UpsertFriendshipPayload::decode(bytes.as_slice()).unwrap().metrics_json(),
            "UpsertFriendshipPayload_RequestPayload" => crate::rpc::proto::decentraland::social_service::v2::upsert_friendship_payload::RequestPayload::decode(bytes.as_slice()).unwrap().metrics_json(),
            "UpsertFriendshipPayload_AcceptPayload" => crate::rpc::proto::decentraland::social_service::v2::upsert_friendship_payload::AcceptPayload::decode(bytes.as_slice()).unwrap().metrics_json(),
            "UpsertFriendshipPayload_RejectPayload" => crate::rpc::proto::decentraland::social_service::v2::upsert_friendship_payload::RejectPayload::decode(bytes.as_slice()).unwrap().metrics_json(),
            "UpsertFriendshipPayload_DeletePayload" => crate::rpc::proto::decentraland::social_service::v2::upsert_friendship_payload::DeletePayload::decode(bytes.as_slice()).unwrap().metrics_json(),
            "UpsertFriendshipPayload_CancelPayload" => crate::rpc::proto::decentraland::social_service::v2::upsert_friendship_payload::CancelPayload::decode(bytes.as_slice()).unwrap().metrics_json(),
            "GetMutualFriendsPayload" => crate::rpc::proto::decentraland::social_service::v2::GetMutualFriendsPayload::decode(bytes.as_slice()).unwrap().metrics_json(),
            "PaginatedResponse" => crate::rpc::proto::decentraland::social_service::v2::PaginatedResponse::decode(bytes.as_slice()).unwrap().metrics_json(),
            "PaginatedFriendsProfilesResponse" => crate::rpc::proto::decentraland::social_service::v2::PaginatedFriendsProfilesResponse::decode(bytes.as_slice()).unwrap().metrics_json(),
            "PaginatedFriendshipRequestsResponse" => crate::rpc::proto::decentraland::social_service::v2::PaginatedFriendshipRequestsResponse::decode(bytes.as_slice()).unwrap().metrics_json(),
            "UpsertFriendshipResponse" => crate::rpc::proto::decentraland::social_service::v2::UpsertFriendshipResponse::decode(bytes.as_slice()).unwrap().metrics_json(),
            "UpsertFriendshipResponse_Accepted" => crate::rpc::proto::decentraland::social_service::v2::upsert_friendship_response::Accepted::decode(bytes.as_slice()).unwrap().metrics_json(),
            "SubscriptionStreamClosed" => crate::rpc::proto::decentraland::social_service::v2::SubscriptionStreamClosed::decode(bytes.as_slice()).unwrap().metrics_json(),
            "FriendshipUpdate" => crate::rpc::proto::decentraland::social_service::v2::FriendshipUpdate::decode(bytes.as_slice()).unwrap().metrics_json(),
            "FriendshipUpdate_RequestResponse" => crate::rpc::proto::decentraland::social_service::v2::friendship_update::RequestResponse::decode(bytes.as_slice()).unwrap().metrics_json(),
            "FriendshipUpdate_AcceptResponse" => crate::rpc::proto::decentraland::social_service::v2::friendship_update::AcceptResponse::decode(bytes.as_slice()).unwrap().metrics_json(),
            "FriendshipUpdate_RejectResponse" => crate::rpc::proto::decentraland::social_service::v2::friendship_update::RejectResponse::decode(bytes.as_slice()).unwrap().metrics_json(),
            "FriendshipUpdate_DeleteResponse" => crate::rpc::proto::decentraland::social_service::v2::friendship_update::DeleteResponse::decode(bytes.as_slice()).unwrap().metrics_json(),
            "FriendshipUpdate_CancelResponse" => crate::rpc::proto::decentraland::social_service::v2::friendship_update::CancelResponse::decode(bytes.as_slice()).unwrap().metrics_json(),
            "FriendshipUpdate_BlockResponse" => crate::rpc::proto::decentraland::social_service::v2::friendship_update::BlockResponse::decode(bytes.as_slice()).unwrap().metrics_json(),
            "FriendConnectivityUpdate" => crate::rpc::proto::decentraland::social_service::v2::FriendConnectivityUpdate::decode(bytes.as_slice()).unwrap().metrics_json(),
            "GetFriendshipStatusPayload" => crate::rpc::proto::decentraland::social_service::v2::GetFriendshipStatusPayload::decode(bytes.as_slice()).unwrap().metrics_json(),
            "GetFriendshipStatusResponse" => crate::rpc::proto::decentraland::social_service::v2::GetFriendshipStatusResponse::decode(bytes.as_slice()).unwrap().metrics_json(),
            "GetFriendshipStatusResponse_Ok" => crate::rpc::proto::decentraland::social_service::v2::get_friendship_status_response::Ok::decode(bytes.as_slice()).unwrap().metrics_json(),
            "BlockUserPayload" => crate::rpc::proto::decentraland::social_service::v2::BlockUserPayload::decode(bytes.as_slice()).unwrap().metrics_json(),
            "BlockUserResponse" => crate::rpc::proto::decentraland::social_service::v2::BlockUserResponse::decode(bytes.as_slice()).unwrap().metrics_json(),
            "BlockUserResponse_Ok" => crate::rpc::proto::decentraland::social_service::v2::block_user_response::Ok::decode(bytes.as_slice()).unwrap().metrics_json(),
            "UnblockUserPayload" => crate::rpc::proto::decentraland::social_service::v2::UnblockUserPayload::decode(bytes.as_slice()).unwrap().metrics_json(),
            "UnblockUserResponse" => crate::rpc::proto::decentraland::social_service::v2::UnblockUserResponse::decode(bytes.as_slice()).unwrap().metrics_json(),
            "UnblockUserResponse_Ok" => crate::rpc::proto::decentraland::social_service::v2::unblock_user_response::Ok::decode(bytes.as_slice()).unwrap().metrics_json(),
            "SocialSettings" => crate::rpc::proto::decentraland::social_service::v2::SocialSettings::decode(bytes.as_slice()).unwrap().metrics_json(),
            "GetSocialSettingsResponse" => crate::rpc::proto::decentraland::social_service::v2::GetSocialSettingsResponse::decode(bytes.as_slice()).unwrap().metrics_json(),
            "GetSocialSettingsResponse_Ok" => crate::rpc::proto::decentraland::social_service::v2::get_social_settings_response::Ok::decode(bytes.as_slice()).unwrap().metrics_json(),
            "UpsertSocialSettingsPayload" => crate::rpc::proto::decentraland::social_service::v2::UpsertSocialSettingsPayload::decode(bytes.as_slice()).unwrap().metrics_json(),
            "UpsertSocialSettingsResponse" => crate::rpc::proto::decentraland::social_service::v2::UpsertSocialSettingsResponse::decode(bytes.as_slice()).unwrap().metrics_json(),
            "GetPrivateMessagesSettingsPayload" => crate::rpc::proto::decentraland::social_service::v2::GetPrivateMessagesSettingsPayload::decode(bytes.as_slice()).unwrap().metrics_json(),
            "GetPrivateMessagesSettingsResponse" => crate::rpc::proto::decentraland::social_service::v2::GetPrivateMessagesSettingsResponse::decode(bytes.as_slice()).unwrap().metrics_json(),
            "GetPrivateMessagesSettingsResponse_PrivateMessagesSettings" => crate::rpc::proto::decentraland::social_service::v2::get_private_messages_settings_response::PrivateMessagesSettings::decode(bytes.as_slice()).unwrap().metrics_json(),
            "GetPrivateMessagesSettingsResponse_Ok" => crate::rpc::proto::decentraland::social_service::v2::get_private_messages_settings_response::Ok::decode(bytes.as_slice()).unwrap().metrics_json(),
            "GetBlockedUsersPayload" => crate::rpc::proto::decentraland::social_service::v2::GetBlockedUsersPayload::decode(bytes.as_slice()).unwrap().metrics_json(),
            "GetBlockedUsersResponse" => crate::rpc::proto::decentraland::social_service::v2::GetBlockedUsersResponse::decode(bytes.as_slice()).unwrap().metrics_json(),
            "GetBlockingStatusResponse" => crate::rpc::proto::decentraland::social_service::v2::GetBlockingStatusResponse::decode(bytes.as_slice()).unwrap().metrics_json(),
            "BlockUpdate" => crate::rpc::proto::decentraland::social_service::v2::BlockUpdate::decode(bytes.as_slice()).unwrap().metrics_json(),
            "CommunityMemberConnectivityUpdate" => crate::rpc::proto::decentraland::social_service::v2::CommunityMemberConnectivityUpdate::decode(bytes.as_slice()).unwrap().metrics_json(),
            "StartPrivateVoiceChatPayload" => crate::rpc::proto::decentraland::social_service::v2::StartPrivateVoiceChatPayload::decode(bytes.as_slice()).unwrap().metrics_json(),
            "StartPrivateVoiceChatResponse" => crate::rpc::proto::decentraland::social_service::v2::StartPrivateVoiceChatResponse::decode(bytes.as_slice()).unwrap().metrics_json(),
            "StartPrivateVoiceChatResponse_Ok" => crate::rpc::proto::decentraland::social_service::v2::start_private_voice_chat_response::Ok::decode(bytes.as_slice()).unwrap().metrics_json(),
            "AcceptPrivateVoiceChatPayload" => crate::rpc::proto::decentraland::social_service::v2::AcceptPrivateVoiceChatPayload::decode(bytes.as_slice()).unwrap().metrics_json(),
            "AcceptPrivateVoiceChatResponse" => crate::rpc::proto::decentraland::social_service::v2::AcceptPrivateVoiceChatResponse::decode(bytes.as_slice()).unwrap().metrics_json(),
            "AcceptPrivateVoiceChatResponse_Ok" => crate::rpc::proto::decentraland::social_service::v2::accept_private_voice_chat_response::Ok::decode(bytes.as_slice()).unwrap().metrics_json(),
            "RejectPrivateVoiceChatPayload" => crate::rpc::proto::decentraland::social_service::v2::RejectPrivateVoiceChatPayload::decode(bytes.as_slice()).unwrap().metrics_json(),
            "RejectPrivateVoiceChatResponse" => crate::rpc::proto::decentraland::social_service::v2::RejectPrivateVoiceChatResponse::decode(bytes.as_slice()).unwrap().metrics_json(),
            "RejectPrivateVoiceChatResponse_Ok" => crate::rpc::proto::decentraland::social_service::v2::reject_private_voice_chat_response::Ok::decode(bytes.as_slice()).unwrap().metrics_json(),
            "PrivateVoiceChatCredentials" => crate::rpc::proto::decentraland::social_service::v2::PrivateVoiceChatCredentials::decode(bytes.as_slice()).unwrap().metrics_json(),
            "PrivateVoiceChatUpdate" => crate::rpc::proto::decentraland::social_service::v2::PrivateVoiceChatUpdate::decode(bytes.as_slice()).unwrap().metrics_json(),
            "EndPrivateVoiceChatPayload" => crate::rpc::proto::decentraland::social_service::v2::EndPrivateVoiceChatPayload::decode(bytes.as_slice()).unwrap().metrics_json(),
            "EndPrivateVoiceChatResponse" => crate::rpc::proto::decentraland::social_service::v2::EndPrivateVoiceChatResponse::decode(bytes.as_slice()).unwrap().metrics_json(),
            "EndPrivateVoiceChatResponse_Ok" => crate::rpc::proto::decentraland::social_service::v2::end_private_voice_chat_response::Ok::decode(bytes.as_slice()).unwrap().metrics_json(),
            "GetIncomingPrivateVoiceChatRequestResponse" => crate::rpc::proto::decentraland::social_service::v2::GetIncomingPrivateVoiceChatRequestResponse::decode(bytes.as_slice()).unwrap().metrics_json(),
            "GetIncomingPrivateVoiceChatRequestResponse_Ok" => crate::rpc::proto::decentraland::social_service::v2::get_incoming_private_voice_chat_request_response::Ok::decode(bytes.as_slice()).unwrap().metrics_json(),
            "CommunityVoiceChatCredentials" => crate::rpc::proto::decentraland::social_service::v2::CommunityVoiceChatCredentials::decode(bytes.as_slice()).unwrap().metrics_json(),
            "StartCommunityVoiceChatPayload" => crate::rpc::proto::decentraland::social_service::v2::StartCommunityVoiceChatPayload::decode(bytes.as_slice()).unwrap().metrics_json(),
            "StartCommunityVoiceChatResponse" => crate::rpc::proto::decentraland::social_service::v2::StartCommunityVoiceChatResponse::decode(bytes.as_slice()).unwrap().metrics_json(),
            "StartCommunityVoiceChatResponse_Ok" => crate::rpc::proto::decentraland::social_service::v2::start_community_voice_chat_response::Ok::decode(bytes.as_slice()).unwrap().metrics_json(),
            "JoinCommunityVoiceChatPayload" => crate::rpc::proto::decentraland::social_service::v2::JoinCommunityVoiceChatPayload::decode(bytes.as_slice()).unwrap().metrics_json(),
            "JoinCommunityVoiceChatResponse" => crate::rpc::proto::decentraland::social_service::v2::JoinCommunityVoiceChatResponse::decode(bytes.as_slice()).unwrap().metrics_json(),
            "JoinCommunityVoiceChatResponse_Ok" => crate::rpc::proto::decentraland::social_service::v2::join_community_voice_chat_response::Ok::decode(bytes.as_slice()).unwrap().metrics_json(),
            "RequestToSpeakInCommunityVoiceChatPayload" => crate::rpc::proto::decentraland::social_service::v2::RequestToSpeakInCommunityVoiceChatPayload::decode(bytes.as_slice()).unwrap().metrics_json(),
            "RequestToSpeakInCommunityVoiceChatResponse" => crate::rpc::proto::decentraland::social_service::v2::RequestToSpeakInCommunityVoiceChatResponse::decode(bytes.as_slice()).unwrap().metrics_json(),
            "RequestToSpeakInCommunityVoiceChatResponse_Ok" => crate::rpc::proto::decentraland::social_service::v2::request_to_speak_in_community_voice_chat_response::Ok::decode(bytes.as_slice()).unwrap().metrics_json(),
            "PromoteSpeakerInCommunityVoiceChatPayload" => crate::rpc::proto::decentraland::social_service::v2::PromoteSpeakerInCommunityVoiceChatPayload::decode(bytes.as_slice()).unwrap().metrics_json(),
            "PromoteSpeakerInCommunityVoiceChatResponse" => crate::rpc::proto::decentraland::social_service::v2::PromoteSpeakerInCommunityVoiceChatResponse::decode(bytes.as_slice()).unwrap().metrics_json(),
            "PromoteSpeakerInCommunityVoiceChatResponse_Ok" => crate::rpc::proto::decentraland::social_service::v2::promote_speaker_in_community_voice_chat_response::Ok::decode(bytes.as_slice()).unwrap().metrics_json(),
            "DemoteSpeakerInCommunityVoiceChatPayload" => crate::rpc::proto::decentraland::social_service::v2::DemoteSpeakerInCommunityVoiceChatPayload::decode(bytes.as_slice()).unwrap().metrics_json(),
            "DemoteSpeakerInCommunityVoiceChatResponse" => crate::rpc::proto::decentraland::social_service::v2::DemoteSpeakerInCommunityVoiceChatResponse::decode(bytes.as_slice()).unwrap().metrics_json(),
            "DemoteSpeakerInCommunityVoiceChatResponse_Ok" => crate::rpc::proto::decentraland::social_service::v2::demote_speaker_in_community_voice_chat_response::Ok::decode(bytes.as_slice()).unwrap().metrics_json(),
            "KickPlayerFromCommunityVoiceChatPayload" => crate::rpc::proto::decentraland::social_service::v2::KickPlayerFromCommunityVoiceChatPayload::decode(bytes.as_slice()).unwrap().metrics_json(),
            "KickPlayerFromCommunityVoiceChatResponse" => crate::rpc::proto::decentraland::social_service::v2::KickPlayerFromCommunityVoiceChatResponse::decode(bytes.as_slice()).unwrap().metrics_json(),
            "KickPlayerFromCommunityVoiceChatResponse_Ok" => crate::rpc::proto::decentraland::social_service::v2::kick_player_from_community_voice_chat_response::Ok::decode(bytes.as_slice()).unwrap().metrics_json(),
            "RejectSpeakRequestInCommunityVoiceChatPayload" => crate::rpc::proto::decentraland::social_service::v2::RejectSpeakRequestInCommunityVoiceChatPayload::decode(bytes.as_slice()).unwrap().metrics_json(),
            "RejectSpeakRequestInCommunityVoiceChatResponse" => crate::rpc::proto::decentraland::social_service::v2::RejectSpeakRequestInCommunityVoiceChatResponse::decode(bytes.as_slice()).unwrap().metrics_json(),
            "RejectSpeakRequestInCommunityVoiceChatResponse_Ok" => crate::rpc::proto::decentraland::social_service::v2::reject_speak_request_in_community_voice_chat_response::Ok::decode(bytes.as_slice()).unwrap().metrics_json(),
            "MuteSpeakerFromCommunityVoiceChatPayload" => crate::rpc::proto::decentraland::social_service::v2::MuteSpeakerFromCommunityVoiceChatPayload::decode(bytes.as_slice()).unwrap().metrics_json(),
            "MuteSpeakerFromCommunityVoiceChatResponse" => crate::rpc::proto::decentraland::social_service::v2::MuteSpeakerFromCommunityVoiceChatResponse::decode(bytes.as_slice()).unwrap().metrics_json(),
            "MuteSpeakerFromCommunityVoiceChatResponse_Ok" => crate::rpc::proto::decentraland::social_service::v2::mute_speaker_from_community_voice_chat_response::Ok::decode(bytes.as_slice()).unwrap().metrics_json(),
            "EndCommunityVoiceChatPayload" => crate::rpc::proto::decentraland::social_service::v2::EndCommunityVoiceChatPayload::decode(bytes.as_slice()).unwrap().metrics_json(),
            "EndCommunityVoiceChatResponse" => crate::rpc::proto::decentraland::social_service::v2::EndCommunityVoiceChatResponse::decode(bytes.as_slice()).unwrap().metrics_json(),
            "EndCommunityVoiceChatResponse_Ok" => crate::rpc::proto::decentraland::social_service::v2::end_community_voice_chat_response::Ok::decode(bytes.as_slice()).unwrap().metrics_json(),
            "CommunityVoiceChatUpdate" => crate::rpc::proto::decentraland::social_service::v2::CommunityVoiceChatUpdate::decode(bytes.as_slice()).unwrap().metrics_json(),
                other => panic!("missing source type {other}"),
            };
            assert_eq!(actual, case["decoded"], "{}", case["id"]);
            assert_eq!(
                crate::rpc::metrics::json_bytes(&actual),
                case["jsonBytes"].as_u64().unwrap() as usize,
                "{}",
                case["id"]
            );
        }
    }
}
