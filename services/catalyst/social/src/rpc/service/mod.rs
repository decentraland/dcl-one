//! The `SocialService` module: the 35 procedures of
//! `social_service_v2.proto`, registered on every port the client creates.
//! The old crate generated a trait and this registration from the `.proto`
//! at build time; the same shape is written out here once, so the procedure
//! names below are the wire contract (a client calls a procedure by name and
//! the server answers with the id it assigned).

use std::sync::Arc;

use prost::Message;

use crate::rpc::context::Context;
use crate::rpc::drpc::rpc_protocol::RemoteError;
use crate::rpc::drpc::server::RpcServerPort;
use crate::rpc::drpc::service_module_definition::{ProcedureContext, ServiceModuleDefinition};
use crate::rpc::drpc::stream_protocol::Generator;
use crate::rpc::proto::v2::*;

mod blocks;
mod community_voice;
mod domain;
mod friends;
mod helpers;
mod private_voice;
mod settings;
mod subscriptions;

pub use helpers::SocialError;

pub const MODULE_NAME: &str = "SocialService";

pub type ServerStreamResponse<T> = Generator<T>;

pub struct SocialServiceImpl;

impl SocialServiceImpl {
    /// The identity the handshake bound to the transport; a call from a
    /// transport with none is refused as unauthenticated.
    pub(super) fn caller(ctx: &ProcedureContext<Context>) -> Result<String, SocialError> {
        ctx.server_context
            .identity(ctx.transport_id)
            .map(|a| helpers::normalize(&a))
            .ok_or(SocialError::Unauthenticated)
    }
}

/// Upstream's generated decode unwrapped, so a malformed body panicked the
/// per-request task and the caller waited forever; here it is a 400.
fn malformed(name: &str, err: prost::DecodeError) -> RemoteError {
    RemoteError {
        message_identifier: 0,
        error_code: 400,
        error_message: format!("{name}: malformed request payload: {err}"),
    }
}

/// The procedures that write (friendships, blocks, settings, calls, rooms);
/// refused with `SocialError::WritesDisabled` while `SOCIAL_WRITES_ENABLED`
/// is false (`social::writes`). The reads and the subscriptions stay open.
pub const WRITE_PROCEDURES: &[&str] = &[
    "UpsertFriendship",
    "BlockUser",
    "UnblockUser",
    "UpsertSocialSettings",
    "StartPrivateVoiceChat",
    "AcceptPrivateVoiceChat",
    "RejectPrivateVoiceChat",
    "EndPrivateVoiceChat",
    "StartCommunityVoiceChat",
    "JoinCommunityVoiceChat",
    "RequestToSpeakInCommunityVoiceChat",
    "PromoteSpeakerInCommunityVoiceChat",
    "DemoteSpeakerInCommunityVoiceChat",
    "KickPlayerFromCommunityVoiceChat",
    "RejectSpeakRequestInCommunityVoiceChat",
    "EndCommunityVoiceChat",
    "MuteSpeakerFromCommunityVoiceChat",
];

fn write_gate(name: &str, context: &ProcedureContext<Context>) -> Result<(), RemoteError> {
    if WRITE_PROCEDURES.contains(&name)
        && let Some(refusal) =
            crate::shared::writes::refusal(context.server_context.writes_enabled())
    {
        let message = refusal.message("social", crate::writes::DISABLED_MESSAGE);
        return Err(SocialError::WritesDisabled(message).into());
    }
    Ok(())
}

macro_rules! unary {
    ($def:ident, $svc:ident, $name:literal, $req:ty, $method:ident) => {{
        let service = Arc::clone(&$svc);
        $def.add_unary($name, move |request, context| {
            let service = service.clone();
            Box::pin(async move {
                write_gate($name, &context)?;
                let request =
                    <$req>::decode(request.as_slice()).map_err(|e| malformed($name, e))?;
                match service.$method(request, context).await {
                    Ok(response) => Ok(response.encode_to_vec()),
                    Err(err) => Err(err.into()),
                }
            })
        });
    }};
    ($def:ident, $svc:ident, $name:literal, $method:ident) => {{
        let service = Arc::clone(&$svc);
        $def.add_unary($name, move |_request, context| {
            let service = service.clone();
            Box::pin(async move {
                match service.$method(context).await {
                    Ok(response) => Ok(response.encode_to_vec()),
                    Err(err) => Err(err.into()),
                }
            })
        });
    }};
}

macro_rules! server_stream {
    ($def:ident, $svc:ident, $name:literal, $method:ident) => {{
        let service = Arc::clone(&$svc);
        $def.add_server_streams($name, move |_request, context| {
            let service = service.clone();
            Box::pin(async move {
                match service.$method(context).await {
                    Ok(generator) => Ok(Generator::from_generator(generator, |item| {
                        Some(item.encode_to_vec())
                    })),
                    Err(err) => Err(err.into()),
                }
            })
        });
    }};
}

/// Registers the module on a freshly created port.
pub fn register_service(port: &mut RpcServerPort<Context>) {
    let mut def = ServiceModuleDefinition::new();
    let svc = Arc::new(SocialServiceImpl);

    unary!(def, svc, "GetFriends", GetFriendsPayload, get_friends);
    unary!(
        def,
        svc,
        "GetMutualFriends",
        GetMutualFriendsPayload,
        get_mutual_friends
    );
    unary!(
        def,
        svc,
        "GetPendingFriendshipRequests",
        GetFriendshipRequestsPayload,
        get_pending_friendship_requests
    );
    unary!(
        def,
        svc,
        "GetSentFriendshipRequests",
        GetFriendshipRequestsPayload,
        get_sent_friendship_requests
    );
    unary!(
        def,
        svc,
        "UpsertFriendship",
        UpsertFriendshipPayload,
        upsert_friendship
    );
    server_stream!(
        def,
        svc,
        "SubscribeToFriendshipUpdates",
        subscribe_to_friendship_updates
    );
    unary!(
        def,
        svc,
        "GetFriendshipStatus",
        GetFriendshipStatusPayload,
        get_friendship_status
    );
    server_stream!(
        def,
        svc,
        "SubscribeToFriendConnectivityUpdates",
        subscribe_to_friend_connectivity_updates
    );
    unary!(def, svc, "BlockUser", BlockUserPayload, block_user);
    unary!(def, svc, "UnblockUser", UnblockUserPayload, unblock_user);
    unary!(
        def,
        svc,
        "GetBlockedUsers",
        GetBlockedUsersPayload,
        get_blocked_users
    );
    unary!(def, svc, "GetBlockingStatus", get_blocking_status);
    server_stream!(
        def,
        svc,
        "SubscribeToBlockUpdates",
        subscribe_to_block_updates
    );
    unary!(def, svc, "GetSocialSettings", get_social_settings);
    unary!(
        def,
        svc,
        "UpsertSocialSettings",
        UpsertSocialSettingsPayload,
        upsert_social_settings
    );
    unary!(
        def,
        svc,
        "GetPrivateMessagesSettings",
        GetPrivateMessagesSettingsPayload,
        get_private_messages_settings
    );
    server_stream!(
        def,
        svc,
        "SubscribeToCommunityMemberConnectivityUpdates",
        subscribe_to_community_member_connectivity_updates
    );
    unary!(
        def,
        svc,
        "StartPrivateVoiceChat",
        StartPrivateVoiceChatPayload,
        start_private_voice_chat
    );
    unary!(
        def,
        svc,
        "AcceptPrivateVoiceChat",
        AcceptPrivateVoiceChatPayload,
        accept_private_voice_chat
    );
    unary!(
        def,
        svc,
        "RejectPrivateVoiceChat",
        RejectPrivateVoiceChatPayload,
        reject_private_voice_chat
    );
    unary!(
        def,
        svc,
        "EndPrivateVoiceChat",
        EndPrivateVoiceChatPayload,
        end_private_voice_chat
    );
    unary!(
        def,
        svc,
        "GetIncomingPrivateVoiceChatRequest",
        get_incoming_private_voice_chat_request
    );
    server_stream!(
        def,
        svc,
        "SubscribeToPrivateVoiceChatUpdates",
        subscribe_to_private_voice_chat_updates
    );
    unary!(
        def,
        svc,
        "StartCommunityVoiceChat",
        StartCommunityVoiceChatPayload,
        start_community_voice_chat
    );
    unary!(
        def,
        svc,
        "JoinCommunityVoiceChat",
        JoinCommunityVoiceChatPayload,
        join_community_voice_chat
    );
    unary!(
        def,
        svc,
        "RequestToSpeakInCommunityVoiceChat",
        RequestToSpeakInCommunityVoiceChatPayload,
        request_to_speak_in_community_voice_chat
    );
    unary!(
        def,
        svc,
        "PromoteSpeakerInCommunityVoiceChat",
        PromoteSpeakerInCommunityVoiceChatPayload,
        promote_speaker_in_community_voice_chat
    );
    unary!(
        def,
        svc,
        "DemoteSpeakerInCommunityVoiceChat",
        DemoteSpeakerInCommunityVoiceChatPayload,
        demote_speaker_in_community_voice_chat
    );
    unary!(
        def,
        svc,
        "KickPlayerFromCommunityVoiceChat",
        KickPlayerFromCommunityVoiceChatPayload,
        kick_player_from_community_voice_chat
    );
    unary!(
        def,
        svc,
        "RejectSpeakRequestInCommunityVoiceChat",
        RejectSpeakRequestInCommunityVoiceChatPayload,
        reject_speak_request_in_community_voice_chat
    );
    unary!(
        def,
        svc,
        "EndCommunityVoiceChat",
        EndCommunityVoiceChatPayload,
        end_community_voice_chat
    );
    server_stream!(
        def,
        svc,
        "SubscribeToCommunityVoiceChatUpdates",
        subscribe_to_community_voice_chat_updates
    );
    unary!(
        def,
        svc,
        "MuteSpeakerFromCommunityVoiceChat",
        MuteSpeakerFromCommunityVoiceChatPayload,
        mute_speaker_from_community_voice_chat
    );

    port.register_module(MODULE_NAME.to_string(), def);
}

/// The procedure names of `service SocialService` in `social_service_v2.proto`,
/// in declaration order; a test pins the registration to it.
pub const PROCEDURES: &[&str] = &[
    "GetFriends",
    "GetMutualFriends",
    "GetPendingFriendshipRequests",
    "GetSentFriendshipRequests",
    "UpsertFriendship",
    "SubscribeToFriendshipUpdates",
    "GetFriendshipStatus",
    "SubscribeToFriendConnectivityUpdates",
    "BlockUser",
    "UnblockUser",
    "GetBlockedUsers",
    "GetBlockingStatus",
    "SubscribeToBlockUpdates",
    "GetSocialSettings",
    "UpsertSocialSettings",
    "GetPrivateMessagesSettings",
    "SubscribeToCommunityMemberConnectivityUpdates",
    "StartPrivateVoiceChat",
    "AcceptPrivateVoiceChat",
    "RejectPrivateVoiceChat",
    "EndPrivateVoiceChat",
    "GetIncomingPrivateVoiceChatRequest",
    "SubscribeToPrivateVoiceChatUpdates",
    "StartCommunityVoiceChat",
    "JoinCommunityVoiceChat",
    "RequestToSpeakInCommunityVoiceChat",
    "PromoteSpeakerInCommunityVoiceChat",
    "DemoteSpeakerInCommunityVoiceChat",
    "KickPlayerFromCommunityVoiceChat",
    "RejectSpeakRequestInCommunityVoiceChat",
    "EndCommunityVoiceChat",
    "SubscribeToCommunityVoiceChatUpdates",
    "MuteSpeakerFromCommunityVoiceChat",
];

#[cfg(test)]
mod tests {
    use super::*;

    /// Every `rpc` of the `.proto` is registered under its exact name, and
    /// nothing else is: the client resolves procedures by name.
    #[test]
    fn the_registration_matches_the_proto_service() {
        let proto = include_str!("../proto/decentraland/social_service/social_service_v2.proto");
        let declared: Vec<&str> = proto
            .lines()
            .filter_map(|l| l.trim().strip_prefix("rpc "))
            .map(|l| l.split('(').next().unwrap().trim())
            .collect();
        assert_eq!(declared, PROCEDURES);

        let mut def = ServiceModuleDefinition::<Context>::new();
        let svc = Arc::new(SocialServiceImpl);
        unary!(def, svc, "GetBlockingStatus", get_blocking_status);
        assert!(def.get_definitions().contains_key("GetBlockingStatus"));

        let mut port = crate::rpc::drpc::server::RpcServerPort::<Context>::new_for_tests("p");
        register_service(&mut port);
        let mut names = port.registered_procedure_names(MODULE_NAME);
        names.sort();
        let mut expected: Vec<&str> = PROCEDURES.to_vec();
        expected.sort();
        assert_eq!(names, expected);
    }

    /// The write list names registered procedures only, every mutating one
    /// and no read or subscription; the gate refuses them, and only them,
    /// while writes are disabled.
    #[tokio::test]
    async fn the_write_gate_covers_the_mutating_procedures() {
        for name in WRITE_PROCEDURES {
            assert!(
                PROCEDURES.contains(name),
                "{name} is not a registered procedure"
            );
        }
        for name in PROCEDURES {
            let mutating = name.starts_with("Upsert")
                || name.starts_with("Block")
                || name.starts_with("Unblock")
                || name.starts_with("Start")
                || name.starts_with("Accept")
                || name.starts_with("Reject")
                || name.starts_with("End")
                || name.starts_with("Join")
                || name.starts_with("RequestToSpeak")
                || name.starts_with("Promote")
                || name.starts_with("Demote")
                || name.starts_with("Kick")
                || name.starts_with("Mute");
            assert_eq!(WRITE_PROCEDURES.contains(name), mutating, "{name}");
        }

        let ctx = Context::new(
            crate::rpc::context::VoiceSettings::default(),
            crate::rpc::db::Db::new(
                sqlx::postgres::PgPoolOptions::new()
                    .connect_lazy("postgres://localhost/unused")
                    .unwrap(),
            ),
            crate::rpc::profiles::Profiles::new(None, Default::default()),
            Arc::new(crate::comms::gatekeeper::voice::rooms::NoVoiceRuntime),
        );
        let pctx = ProcedureContext {
            server_context: Arc::new(ctx.clone()),
            transport_id: 1,
        };
        assert!(
            write_gate("UpsertFriendship", &pctx).is_ok(),
            "enabled by default"
        );
        ctx.set_writes_enabled(false);
        let err = write_gate("UpsertFriendship", &pctx).unwrap_err();
        assert_eq!(err.error_code, 503);
        assert!(
            err.error_message.contains("SOCIAL_WRITES_ENABLED=false"),
            "{}",
            err.error_message
        );
        assert!(write_gate("GetFriends", &pctx).is_ok(), "reads stay open");
        assert!(
            write_gate("SubscribeToFriendshipUpdates", &pctx).is_ok(),
            "subscriptions stay open"
        );

        // on, a mutating procedure still waits for the restore stamps
        ctx.set_writes_enabled(true);
        let lab = crate::shared::writes::restore::lab::StampLab::fresh();
        crate::shared::writes::restore::scoped(lab.stamps.clone(), async {
            assert!(write_gate("UpsertFriendship", &pctx).is_ok());
            lab.advance(27 * crate::shared::writes::restore::lab::HOUR);
            let err = write_gate("UpsertFriendship", &pctx).unwrap_err();
            assert_eq!(err.error_code, 503);
            assert_eq!(
                err.error_message,
                "social writes are held on this node: the last complete backup (backup.json) is 28 h old, past WRITES_BACKUP_MAX_AGE_HOURS=26"
            );
            assert!(write_gate("GetFriends", &pctx).is_ok(), "reads stay open");
        })
        .await;
    }
}
