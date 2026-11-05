//! The pure rules behind the handlers: the friendship state machine
//! (upstream social-service-ea `validateNewFriendshipAction`), the status a
//! pair reads from its last action, the settings vocabulary, the update each
//! action fans out, and the community-voice gates over
//! [`CommunityStanding`] (identity.rs, the one permission matrix shared with
//! the REST module).

use super::SocialServiceImpl;
use super::helpers::{Refusal, SocialError, page_friendship_requests, page_of};
use crate::rpc::context::Context;
use crate::rpc::db::{LastAction, SocialSettingsRow, VoiceGate, VoiceParty};
use crate::rpc::drpc::service_module_definition::ProcedureContext;
use crate::rpc::profiles;
use crate::rpc::proto::errors::*;
use crate::rpc::proto::v2::*;
use crate::{CommunityPermission, CommunityStanding};
use uuid::Uuid;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Action {
    Request,
    Accept,
    Cancel,
    Reject,
    Delete,
    Block,
}

impl Action {
    pub(super) fn as_str(self) -> &'static str {
        match self {
            Action::Request => "request",
            Action::Accept => "accept",
            Action::Cancel => "cancel",
            Action::Reject => "reject",
            Action::Delete => "delete",
            Action::Block => "block",
        }
    }

    fn from_str(s: &str) -> Option<Action> {
        Some(match s {
            "request" => Action::Request,
            "accept" => Action::Accept,
            "cancel" => Action::Cancel,
            "reject" => Action::Reject,
            "delete" => Action::Delete,
            "block" => Action::Block,
            _ => return None,
        })
    }

    /// The `friendships.is_active` projection of this action.
    pub(super) fn implies_active(self) -> bool {
        matches!(self, Action::Accept)
    }
}

fn transition_valid(from: Option<Action>, to: Action) -> bool {
    let allowed: &[Option<Action>] = match to {
        Action::Request => &[
            Some(Action::Cancel),
            Some(Action::Reject),
            Some(Action::Delete),
            None,
        ],
        Action::Accept => &[Some(Action::Request)],
        Action::Cancel => &[Some(Action::Request)],
        Action::Reject => &[Some(Action::Request)],
        Action::Delete => &[Some(Action::Accept), Some(Action::Block)],
        Action::Block => &[
            Some(Action::Request),
            Some(Action::Cancel),
            Some(Action::Reject),
            Some(Action::Delete),
            Some(Action::Accept),
            None,
        ],
    };
    allowed.contains(&from)
}

/// The transition table plus the acting-user rules: the requester cannot
/// accept or reject their own request, the requested party cannot cancel it.
pub(super) fn user_action_valid(
    acting_user: &str,
    new_action: Action,
    new_user: &str,
    last: Option<&LastAction>,
) -> bool {
    let last_act = last.and_then(|l| Action::from_str(&l.action));
    if !transition_valid(last_act, new_action) {
        return false;
    }
    match last {
        None => !(new_action == Action::Request && acting_user == new_user),
        Some(last) => {
            if last.acting_user == acting_user {
                !matches!(new_action, Action::Accept | Action::Reject)
            } else {
                new_action != Action::Cancel
            }
        }
    }
}

pub(super) fn friendship_action_status(last: &LastAction, me: &str) -> Option<FriendshipStatus> {
    let acting_is_me = last.acting_user == me;
    Some(match Action::from_str(&last.action)? {
        Action::Accept => FriendshipStatus::Accepted,
        Action::Cancel => FriendshipStatus::Canceled,
        Action::Delete => FriendshipStatus::Deleted,
        Action::Reject => FriendshipStatus::Rejected,
        Action::Request if acting_is_me => FriendshipStatus::RequestSent,
        Action::Request => FriendshipStatus::RequestReceived,
        Action::Block if acting_is_me => FriendshipStatus::Blocked,
        Action::Block => FriendshipStatus::BlockedBy,
    })
}

pub(super) fn status_ok(status: FriendshipStatus) -> GetFriendshipStatusResponse {
    GetFriendshipStatusResponse {
        response: Some(get_friendship_status_response::Response::Accepted(
            get_friendship_status_response::Ok {
                status: status as i32,
                message: None,
            },
        )),
    }
}

pub(super) fn settings_to_proto(row: &SocialSettingsRow) -> SocialSettings {
    SocialSettings {
        private_messages_privacy: pmp_from_db(&row.private_messages_privacy) as i32,
        blocked_users_messages_visibility: bvis_from_db(&row.blocked_users_messages_visibility)
            as i32,
        show_situation_reactions: sreact_from_db(&row.show_situation_reactions) as i32,
    }
}

pub(super) fn pmp_to_db(v: PrivateMessagePrivacySetting) -> String {
    match v {
        PrivateMessagePrivacySetting::All => "all",
        PrivateMessagePrivacySetting::OnlyFriends => "only_friends",
    }
    .to_string()
}

pub(super) fn pmp_from_db(s: &str) -> PrivateMessagePrivacySetting {
    match s {
        "all" => PrivateMessagePrivacySetting::All,
        _ => PrivateMessagePrivacySetting::OnlyFriends,
    }
}

pub(super) fn bvis_to_db(v: BlockedUsersMessagesVisibilitySetting) -> String {
    match v {
        BlockedUsersMessagesVisibilitySetting::ShowMessages => "show_messages",
        BlockedUsersMessagesVisibilitySetting::DoNotShowMessages => "do_not_show_messages",
    }
    .to_string()
}

fn bvis_from_db(s: &str) -> BlockedUsersMessagesVisibilitySetting {
    match s {
        "do_not_show_messages" => BlockedUsersMessagesVisibilitySetting::DoNotShowMessages,
        _ => BlockedUsersMessagesVisibilitySetting::ShowMessages,
    }
}

pub(super) fn sreact_to_db(v: SituationReactionsVisibility) -> String {
    match v {
        SituationReactionsVisibility::Show => "show",
        SituationReactionsVisibility::Hide => "hide",
    }
    .to_string()
}

fn sreact_from_db(s: &str) -> SituationReactionsVisibility {
    match s {
        "hide" => SituationReactionsVisibility::Hide,
        _ => SituationReactionsVisibility::Show,
    }
}

/// The update the OTHER party's `SubscribeToFriendshipUpdates` receives.
pub(super) fn friendship_update_for(
    action: Action,
    from: &str,
    id: &Uuid,
    created_ms: i64,
    message: Option<&str>,
    from_profile: FriendProfile,
) -> FriendshipUpdate {
    let user = Some(User {
        address: from.to_string(),
    });
    let update = match action {
        Action::Request => friendship_update::Update::Request(friendship_update::RequestResponse {
            friend: Some(from_profile),
            created_at: created_ms,
            message: message.map(|m| m.to_string()),
            id: id.to_string(),
        }),
        Action::Accept => {
            friendship_update::Update::Accept(friendship_update::AcceptResponse { user })
        }
        Action::Reject => {
            friendship_update::Update::Reject(friendship_update::RejectResponse { user })
        }
        Action::Cancel => {
            friendship_update::Update::Cancel(friendship_update::CancelResponse { user })
        }
        Action::Delete => {
            friendship_update::Update::Delete(friendship_update::DeleteResponse { user })
        }
        Action::Block => {
            friendship_update::Update::Block(friendship_update::BlockResponse { user })
        }
    };
    FriendshipUpdate {
        stream_closed: None,
        update: Some(update),
    }
}

pub(super) async fn friendship_requests(
    context: &ProcedureContext<Context>,
    request: GetFriendshipRequestsPayload,
    incoming: bool,
) -> Result<PaginatedFriendshipRequestsResponse, SocialError> {
    let me = SocialServiceImpl::caller(context)?;
    let db = context.server_context.db();
    let (limit, offset) = page_friendship_requests(&request.pagination);
    let (rows, total) = match db
        .get_friendship_requests(&me, incoming, limit, offset)
        .await
    {
        Ok(v) => v,
        Err(e) => {
            tracing::error!(error = %e, "friendship requests read failed");
            return Ok(PaginatedFriendshipRequestsResponse {
                response: Some(
                    paginated_friendship_requests_response::Response::InternalServerError(
                        InternalServerError { message: None },
                    ),
                ),
                pagination_data: None,
            });
        }
    };
    // Upstream `parseFriendshipRequestsToFriendshipRequestResponses`: a
    // request whose counterparty has no profile is left out (the total
    // stays the row count), and an absent message reads `''`.
    let addrs: Vec<String> = rows.iter().map(|r| r.address.clone()).collect();
    let map = match context
        .server_context
        .profiles()
        .get_profiles_result(&addrs)
        .await
    {
        Ok(profiles) => profiles,
        Err(error) => {
            tracing::error!(%error, "friendship request profiles unavailable");
            return Ok(PaginatedFriendshipRequestsResponse {
                response: Some(
                    paginated_friendship_requests_response::Response::InternalServerError(
                        InternalServerError { message: None },
                    ),
                ),
                pagination_data: None,
            });
        }
    };
    let requests = rows
        .into_iter()
        .filter_map(|r| {
            let profile = map.get(&r.address.to_lowercase())?;
            Some(FriendshipRequestResponse {
                friend: Some(profiles::friend_profile(&r.address, Some(profile))),
                created_at: r.timestamp.timestamp_millis(),
                message: Some(r.message.unwrap_or_default()),
                id: r.id.to_string(),
            })
        })
        .collect();
    Ok(PaginatedFriendshipRequestsResponse {
        response: Some(paginated_friendship_requests_response::Response::Requests(
            FriendshipRequests { requests },
        )),
        pagination_data: Some(PaginatedResponse {
            total: total as i32,
            page: page_of(limit, offset),
        }),
    })
}

fn standing(party: &VoiceParty) -> CommunityStanding {
    CommunityStanding::from_rows(false, party.role.as_deref())
}

/// Upstream `validateCommunityVoiceChatHost` (start, end): a role in the
/// community, an owner's or a moderator's, and no ban, in that order.
pub(super) fn host_gate(
    gate: &VoiceGate,
    actor: &str,
    community_id: &str,
    action: &str,
) -> Result<(), Refusal> {
    let actor_standing = standing(&gate.a);
    if actor_standing == CommunityStanding::NotAMember {
        return Err(not_a_member(actor, community_id));
    }
    if !actor_standing.has(CommunityPermission::BanPlayers) {
        return Err(Refusal::Forbidden(format!(
            "Only community owners and moderators can {action}"
        )));
    }
    if gate.a.banned {
        return Err(Refusal::Forbidden(format!("Banned users cannot {action}")));
    }
    Ok(())
}

/// Upstream `validateCommunityVoiceChatModerator` (promote, demote another,
/// reject, kick, mute another): an owner or moderator, not banned, never a
/// non-owner acting on the owner. Peer moderators may act on each other:
/// these actions are confined to the live room and are reversible.
pub(super) fn moderator_gate(
    gate: &VoiceGate,
    actor: &str,
    target: &str,
    action: &str,
) -> Result<(), Refusal> {
    let (actor_standing, target_standing) = (standing(&gate.a), standing(&gate.b));
    if !actor_standing.has(CommunityPermission::BanPlayers) {
        return Err(Refusal::Forbidden(format!(
            "Only community owners and moderators can {action}"
        )));
    }
    if gate.a.banned {
        return Err(Refusal::Forbidden(format!("Banned users cannot {action}")));
    }
    if actor != target && targeting_the_owner_as_a_non_owner(actor_standing, target_standing) {
        return Err(Refusal::Forbidden(format!(
            "Not enough permissions to {action} this user"
        )));
    }
    Ok(())
}

/// Upstream `validateCommunityVoiceChatTargetMembership` (reject): a public
/// community takes anyone the room already holds, a private one requires
/// the target to be a member.
pub(super) fn target_membership_gate(
    private: bool,
    target: &VoiceParty,
    target_address: &str,
    community_id: &str,
) -> Result<(), Refusal> {
    if private && standing(target) == CommunityStanding::NotAMember {
        return Err(not_a_member(target_address, community_id));
    }
    Ok(())
}

/// Upstream `validateCommunityVoiceChatTargetUser` (promote, demote
/// another): a banned target is refused whatever the privacy, then the
/// membership rule above.
pub(super) fn target_user_gate(
    private: bool,
    target: &VoiceParty,
    target_address: &str,
    community_id: &str,
) -> Result<(), Refusal> {
    if target.banned {
        return Err(not_a_member(target_address, community_id));
    }
    target_membership_gate(private, target, target_address, community_id)
}

/// Upstream `UserNotCommunityMemberError`.
pub(super) fn not_a_member(address: &str, community_id: &str) -> Refusal {
    Refusal::Forbidden(format!(
        "User {address} is not a member of community {community_id}"
    ))
}

/// Only the owner may be acted on, and only by the owner.
fn targeting_the_owner_as_a_non_owner(actor: CommunityStanding, target: CommunityStanding) -> bool {
    target == CommunityStanding::Owner && actor != CommunityStanding::Owner
}

#[cfg(test)]
mod tests {
    use super::*;

    fn last(action: &str, acting_user: &str) -> LastAction {
        LastAction {
            friendship_id: Uuid::nil(),
            action: action.to_string(),
            acting_user: acting_user.to_string(),
            is_active: false,
        }
    }

    fn resolve(
        last: Option<&LastAction>,
        me: &str,
        me_blocked_other: bool,
        other_blocked_me: bool,
    ) -> FriendshipStatus {
        match last.and_then(|l| friendship_action_status(l, me)) {
            Some(s) => s,
            None => {
                if me_blocked_other {
                    FriendshipStatus::Blocked
                } else if other_blocked_me {
                    FriendshipStatus::BlockedBy
                } else {
                    FriendshipStatus::None
                }
            }
        }
    }

    const ME: &str = "0xme";
    const OTHER: &str = "0xother";

    #[test]
    fn action_mapping_covers_every_action_and_direction() {
        assert_eq!(
            friendship_action_status(&last("accept", OTHER), ME),
            Some(FriendshipStatus::Accepted)
        );
        assert_eq!(
            friendship_action_status(&last("cancel", ME), ME),
            Some(FriendshipStatus::Canceled)
        );
        assert_eq!(
            friendship_action_status(&last("delete", OTHER), ME),
            Some(FriendshipStatus::Deleted)
        );
        assert_eq!(
            friendship_action_status(&last("reject", OTHER), ME),
            Some(FriendshipStatus::Rejected)
        );
        assert_eq!(
            friendship_action_status(&last("request", ME), ME),
            Some(FriendshipStatus::RequestSent)
        );
        assert_eq!(
            friendship_action_status(&last("request", OTHER), ME),
            Some(FriendshipStatus::RequestReceived)
        );
        assert_eq!(
            friendship_action_status(&last("block", ME), ME),
            Some(FriendshipStatus::Blocked)
        );
        assert_eq!(
            friendship_action_status(&last("block", OTHER), ME),
            Some(FriendshipStatus::BlockedBy)
        );
        assert_eq!(friendship_action_status(&last("", ME), ME), None);
        assert_eq!(friendship_action_status(&last("garbage", ME), ME), None);
    }

    #[test]
    fn friendship_action_takes_precedence_over_blocks() {
        assert_eq!(
            resolve(Some(&last("accept", OTHER)), ME, true, true),
            FriendshipStatus::Accepted
        );
        assert_eq!(
            resolve(Some(&last("request", ME)), ME, true, false),
            FriendshipStatus::RequestSent
        );
        assert_eq!(
            resolve(Some(&last("block", OTHER)), ME, false, true),
            FriendshipStatus::BlockedBy
        );
    }

    #[test]
    fn block_fallback_only_when_no_friendship_action() {
        assert_eq!(resolve(None, ME, true, false), FriendshipStatus::Blocked);
        assert_eq!(resolve(None, ME, false, true), FriendshipStatus::BlockedBy);
        assert_eq!(resolve(None, ME, false, false), FriendshipStatus::None);
        assert_eq!(resolve(None, ME, true, true), FriendshipStatus::Blocked);
    }

    #[test]
    fn no_prior_action_only_admits_request_and_block() {
        assert!(user_action_valid(ME, Action::Request, OTHER, None));
        assert!(user_action_valid(ME, Action::Block, OTHER, None));
        assert!(!user_action_valid(ME, Action::Accept, OTHER, None));
        assert!(!user_action_valid(ME, Action::Reject, OTHER, None));
        assert!(!user_action_valid(ME, Action::Delete, OTHER, None));
        assert!(!user_action_valid(ME, Action::Cancel, OTHER, None));
    }

    #[test]
    fn self_request_is_rejected_even_without_prior_action() {
        assert!(!user_action_valid(ME, Action::Request, ME, None));
    }

    #[test]
    fn full_transition_table_is_enforced() {
        let all = [
            Action::Request,
            Action::Accept,
            Action::Cancel,
            Action::Reject,
            Action::Delete,
            Action::Block,
        ];
        for from in all {
            for to in all {
                let legal = match to {
                    Action::Request => {
                        matches!(from, Action::Cancel | Action::Reject | Action::Delete)
                    }
                    Action::Accept | Action::Cancel | Action::Reject => from == Action::Request,
                    Action::Delete => matches!(from, Action::Accept | Action::Block),
                    Action::Block => from != Action::Block,
                };
                assert_eq!(
                    transition_valid(Some(from), to),
                    legal,
                    "transition {from:?} -> {to:?}"
                );
            }
        }
    }

    #[test]
    fn acting_user_rules_still_apply_after_transition_check() {
        let sent = last("request", ME);
        assert!(!user_action_valid(ME, Action::Accept, OTHER, Some(&sent)));
        assert!(!user_action_valid(ME, Action::Reject, OTHER, Some(&sent)));
        assert!(user_action_valid(ME, Action::Cancel, OTHER, Some(&sent)));

        let received = last("request", OTHER);
        assert!(user_action_valid(
            ME,
            Action::Accept,
            OTHER,
            Some(&received)
        ));
        assert!(user_action_valid(
            ME,
            Action::Reject,
            OTHER,
            Some(&received)
        ));
        assert!(!user_action_valid(
            ME,
            Action::Cancel,
            OTHER,
            Some(&received)
        ));

        let accepted = last("accept", OTHER);
        assert!(!user_action_valid(
            ME,
            Action::Accept,
            OTHER,
            Some(&accepted)
        ));
        assert!(user_action_valid(
            ME,
            Action::Delete,
            OTHER,
            Some(&accepted)
        ));
    }

    #[test]
    fn friendship_row_without_action_falls_through_to_blocks() {
        assert_eq!(
            resolve(Some(&last("", OTHER)), ME, false, true),
            FriendshipStatus::BlockedBy
        );
        assert_eq!(
            resolve(Some(&last("", ME)), ME, true, false),
            FriendshipStatus::Blocked
        );
        assert_eq!(
            resolve(Some(&last("", ME)), ME, false, false),
            FriendshipStatus::None
        );
    }

    #[test]
    fn owner_protection_only_bites_a_non_owner_targeting_the_owner() {
        use CommunityStanding as T;
        assert!(targeting_the_owner_as_a_non_owner(T::Moderator, T::Owner));
        assert!(targeting_the_owner_as_a_non_owner(T::NotAMember, T::Owner));
        assert!(!targeting_the_owner_as_a_non_owner(T::Owner, T::Owner));
        assert!(!targeting_the_owner_as_a_non_owner(
            T::Moderator,
            T::Moderator
        ));
        assert!(!targeting_the_owner_as_a_non_owner(T::Moderator, T::Member));
    }

    fn gate(
        role_a: Option<&str>,
        banned_a: bool,
        role_b: Option<&str>,
        banned_b: bool,
    ) -> VoiceGate {
        VoiceGate {
            community: None,
            a: VoiceParty {
                role: role_a.map(str::to_string),
                banned: banned_a,
            },
            b: VoiceParty {
                role: role_b.map(str::to_string),
                banned: banned_b,
            },
        }
    }

    fn message(r: Result<(), Refusal>) -> String {
        r.unwrap_err().message().to_string()
    }

    #[test]
    fn the_moderator_gate_reads_both_role_spellings_and_refuses_members() {
        let act = "kick players";
        for role in ["owner", "moderator", "mod"] {
            assert!(moderator_gate(&gate(Some(role), false, None, false), ME, OTHER, act).is_ok());
        }
        for role in [Some("member"), Some("admin"), None] {
            assert_eq!(
                message(moderator_gate(
                    &gate(role, false, None, false),
                    ME,
                    OTHER,
                    act
                )),
                "Only community owners and moderators can kick players"
            );
        }
        assert!(target_membership_gate(true, &VoiceParty::default(), OTHER, "c").is_err());
        assert!(target_membership_gate(false, &VoiceParty::default(), OTHER, "c").is_ok());
        let member = VoiceParty {
            role: Some("member".into()),
            banned: false,
        };
        assert!(target_membership_gate(true, &member, OTHER, "c").is_ok());
    }

    /// Upstream `validation.ts`: a ban is read on every gate, since it is
    /// not guaranteed to clear the role row; the host gate names a wallet
    /// with no role; a banned target is never promoted or demoted.
    #[test]
    fn every_gate_reads_the_ban_and_names_the_refusal_as_upstream() {
        assert_eq!(
            message(host_gate(
                &gate(None, false, None, false),
                ME,
                "c",
                "start voice chats"
            )),
            "User 0xme is not a member of community c"
        );
        assert_eq!(
            message(host_gate(
                &gate(Some("member"), false, None, false),
                ME,
                "c",
                "start voice chats"
            )),
            "Only community owners and moderators can start voice chats"
        );
        assert_eq!(
            message(host_gate(
                &gate(Some("owner"), true, None, false),
                ME,
                "c",
                "end voice chats"
            )),
            "Banned users cannot end voice chats"
        );
        assert_eq!(
            message(moderator_gate(
                &gate(Some("moderator"), true, None, false),
                ME,
                OTHER,
                "promote speakers"
            )),
            "Banned users cannot promote speakers"
        );
        assert_eq!(
            message(moderator_gate(
                &gate(Some("moderator"), false, Some("owner"), false),
                ME,
                OTHER,
                "kick players"
            )),
            "Not enough permissions to kick players this user"
        );
        assert!(
            moderator_gate(
                &gate(Some("owner"), false, Some("owner"), false),
                ME,
                ME,
                "mute/unmute speakers"
            )
            .is_ok()
        );
        let banned = VoiceParty {
            role: None,
            banned: true,
        };
        assert_eq!(
            message(target_user_gate(false, &banned, OTHER, "c")),
            "User 0xother is not a member of community c"
        );
        assert!(
            target_membership_gate(false, &banned, OTHER, "c").is_ok(),
            "reject reads no ban"
        );
    }
}
