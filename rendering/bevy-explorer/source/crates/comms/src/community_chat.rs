//! Community text chat on the wire, as unity-explorer speaks it (`LiveKitChatMessagesBus.cs`,
//! `MultiplayerReactionMessageBus.cs`): a community channel is `community:<id>`, which is also the
//! data-packet topic on the private chat room. A client never addresses members itself; it sends
//! to the server's message router participant, which fans the packet out to the community's
//! members and stamps the author into `Chat.forwarded_from` (a reaction carries the author in its
//! own `address` field). A receiver trusts that stamped author only when the packet really came
//! from the router.

use alloy_core::primitives::Address;
use common::util::AsH160;

/// A community channel id: this prefix and the community id.
pub const CHANNEL_PREFIX: &str = "community:";

/// The longest community id accepted: it lands in a packet topic and a history key.
const MAX_ID_LEN: usize = 64;

/// The router's environment: `prd` on decentraland.org, `dev` anywhere else (unity-explorer maps
/// zone and custom base domains both to `dev`; its `local` needs a domain without a dot, which
/// `base_domain::set` rejects).
pub fn router_env(base_domain: &str) -> &'static str {
    if base_domain == common::base_domain::DEFAULT {
        "prd"
    } else {
        "dev"
    }
}

/// The LiveKit identity of the server's message router on the private chat room.
pub fn message_router_identity() -> String {
    format!(
        "message-router-{}-0",
        router_env(common::base_domain::get())
    )
}

/// The community id a channel or topic names, if it is a well-formed community channel.
pub fn community_id(channel: &str) -> Option<&str> {
    let id = channel.strip_prefix(CHANNEL_PREFIX)?;
    let well_formed = !id.is_empty()
        && id.len() <= MAX_ID_LEN
        && id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-');
    well_formed.then_some(id)
}

/// The canonical channel (and topic, and history key) of a community.
pub fn channel_for(community_id: &str) -> String {
    format!("{CHANNEL_PREFIX}{}", community_id.to_ascii_lowercase())
}

/// Where a chat event on a non-Nearby channel goes: a DM partner or a community.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PrivateChatTarget {
    Dm(Address),
    /// The community id, lowercased.
    Community(String),
}

impl PrivateChatTarget {
    pub fn parse(channel: &str) -> Option<Self> {
        if let Some(address) = channel.as_h160() {
            return Some(Self::Dm(address));
        }
        community_id(channel).map(|id| Self::Community(id.to_ascii_lowercase()))
    }
}

/// The author of a packet on the private chat room. A packet from the router carries its author
/// in `claimed` (`Chat.forwarded_from`, or `ChatReaction.address`), trusted only because the
/// router put it there; from anyone else `claimed` is ignored and the author is the sender's own
/// LiveKit identity, which the server authenticated.
pub fn resolve_author(identity: &str, router: &str, claimed: Option<&str>) -> Option<Address> {
    if identity.eq_ignore_ascii_case(router) {
        claimed.and_then(|claimed| claimed.as_h160())
    } else {
        identity.as_h160()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALICE: &str = "0x1111111111111111111111111111111111111111";
    const BOB: &str = "0x2222222222222222222222222222222222222222";
    const ROUTER: &str = "message-router-prd-0";

    #[test]
    fn router_env_is_prd_only_on_the_default_domain() {
        assert_eq!(router_env("decentraland.org"), "prd");
        assert_eq!(router_env("decentraland.zone"), "dev");
        assert_eq!(router_env("example.test"), "dev");
    }

    #[test]
    fn community_ids_are_bounded_ascii() {
        assert_eq!(
            community_id("community:3f2a0c1e-9b8d-4e7f-a6c5-0123456789ab"),
            Some("3f2a0c1e-9b8d-4e7f-a6c5-0123456789ab")
        );
        assert_eq!(community_id("community:"), None);
        assert_eq!(community_id("community:has space"), None);
        assert_eq!(community_id("community:a/b"), None);
        assert_eq!(community_id(&format!("community:{}", "a".repeat(65))), None);
        assert_eq!(community_id("Nearby"), None);
        assert_eq!(community_id(ALICE), None);
    }

    #[test]
    fn channels_parse_to_a_dm_or_a_community() {
        assert_eq!(
            PrivateChatTarget::parse(ALICE),
            Some(PrivateChatTarget::Dm(ALICE.as_h160().unwrap()))
        );
        assert_eq!(
            PrivateChatTarget::parse("community:ABC-1"),
            Some(PrivateChatTarget::Community("abc-1".to_owned()))
        );
        assert_eq!(channel_for("ABC-1"), "community:abc-1");
        assert_eq!(PrivateChatTarget::parse("Nearby"), None);
        assert_eq!(PrivateChatTarget::parse("System"), None);
    }

    #[test]
    fn the_router_names_the_author_and_nobody_else_can() {
        let alice = ALICE.as_h160().unwrap();
        let bob = BOB.as_h160().unwrap();
        // from the router: the stamped author, when it is a wallet
        assert_eq!(resolve_author(ROUTER, ROUTER, Some(ALICE)), Some(alice));
        assert_eq!(
            resolve_author("MESSAGE-ROUTER-PRD-0", ROUTER, Some(ALICE)),
            Some(alice)
        );
        assert_eq!(resolve_author(ROUTER, ROUTER, Some("not a wallet")), None);
        assert_eq!(resolve_author(ROUTER, ROUTER, None), None);
        // from a peer: their own identity, whatever they claim
        assert_eq!(resolve_author(BOB, ROUTER, Some(ALICE)), Some(bob));
        assert_eq!(resolve_author(BOB, ROUTER, None), Some(bob));
        assert_eq!(
            resolve_author("message-router-dev-0", ROUTER, Some(ALICE)),
            None
        );
    }
}
