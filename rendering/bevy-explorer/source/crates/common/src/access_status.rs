use alloy_core::primitives::Address;
use bevy::{platform::collections::HashSet, prelude::Resource};

/// Admission scopes are independent, even when their identifiers match.
/// A denial never establishes a platform ban or changes personal block state.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum AccessScope {
    Realm(String),
    Scene(String),
    PrivateChat,
    Voice(String),
}

/// Client access observations for the active identity. Room admission owns
/// denied scopes; the social client's authoritative snapshot owns both block lists.
/// An absent denial means no denial is cached, not that admission is guaranteed.
#[derive(Resource, Default, Debug)]
pub struct AccessStatus {
    identity: Option<Address>,
    denied: HashSet<AccessScope>,
    blocks_ready: bool,
    blocked: HashSet<Address>,
    blocked_by: HashSet<Address>,
}

impl AccessStatus {
    /// Returns whether the identity changed. Consumers synchronize this before
    /// publishing observations so a new account cannot inherit cached access.
    pub fn set_identity(&mut self, identity: Option<Address>) -> bool {
        if self.identity == identity {
            return false;
        }
        self.identity = identity;
        self.denied.clear();
        self.invalidate_blocks();
        true
    }

    pub fn identity(&self) -> Option<Address> {
        self.identity
    }

    pub fn set_denied(&mut self, scope: AccessScope) {
        self.denied.insert(scope);
    }

    /// A successful admission or explicit access change clears this scope only.
    pub fn set_allowed(&mut self, scope: &AccessScope) {
        self.denied.remove(scope);
    }

    pub fn remove_scope(&mut self, scope: &AccessScope) {
        self.denied.remove(scope);
    }

    pub fn is_denied(&self, scope: &AccessScope) -> bool {
        self.denied.contains(scope)
    }

    /// Publish both directions from a successful snapshot and its subsequent updates.
    pub fn replace_blocks(
        &mut self,
        owner: Address,
        blocked: impl IntoIterator<Item = Address>,
        blocked_by: impl IntoIterator<Item = Address>,
    ) {
        if self.identity != Some(owner) {
            return;
        }
        self.blocked = blocked.into_iter().collect();
        self.blocked_by = blocked_by.into_iter().collect();
        self.blocks_ready = true;
    }

    pub fn invalidate_blocks(&mut self) {
        self.blocks_ready = false;
        self.blocked.clear();
        self.blocked_by.clear();
    }

    pub fn blocks_ready_for(&self, owner: Option<Address>) -> bool {
        self.blocks_ready && owner.is_some() && self.identity == owner
    }

    /// Voice must wait for the current owner's authoritative block state.
    pub fn allows_voice_from(&self, owner: Option<Address>, peer: Address) -> bool {
        self.blocks_ready_for(owner) && !self.is_blocked(peer) && !self.is_blocked_by(peer)
    }

    /// Whether the local user blocked this address.
    pub fn is_blocked(&self, address: Address) -> bool {
        self.blocked.contains(&address)
    }

    /// Whether this address blocked the local user.
    pub fn is_blocked_by(&self, address: Address) -> bool {
        self.blocked_by.contains(&address)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn voice_waits_for_current_wallet_blocks_even_when_the_lists_are_empty() {
        let owner = Address::from([1; 20]);
        let other_owner = Address::from([2; 20]);
        let peer = Address::from([3; 20]);
        let mut status = AccessStatus::default();
        status.set_identity(Some(owner));

        assert!(!status.allows_voice_from(Some(owner), peer));
        status.replace_blocks(owner, [], []);
        assert!(status.allows_voice_from(Some(owner), peer));
        assert!(!status.allows_voice_from(Some(other_owner), peer));
        assert!(!status.allows_voice_from(None, peer));

        status.set_identity(Some(other_owner));
        status.replace_blocks(owner, [], []);
        assert!(!status.allows_voice_from(Some(other_owner), peer));
        status.replace_blocks(other_owner, [], []);
        assert!(status.allows_voice_from(Some(other_owner), peer));
    }

    #[test]
    fn voice_applies_both_block_directions_and_waits_again_after_invalidation() {
        let owner = Address::from([1; 20]);
        let blocked = Address::from([2; 20]);
        let blocked_by = Address::from([3; 20]);
        let peer = Address::from([4; 20]);
        let mut status = AccessStatus::default();
        status.set_identity(Some(owner));

        status.replace_blocks(owner, [blocked], [blocked_by]);
        assert!(!status.allows_voice_from(Some(owner), blocked));
        assert!(!status.allows_voice_from(Some(owner), blocked_by));
        assert!(status.allows_voice_from(Some(owner), peer));

        status.invalidate_blocks();
        assert!(!status.allows_voice_from(Some(owner), peer));
        status.replace_blocks(owner, [], []);
        assert!(status.allows_voice_from(Some(owner), blocked));
        assert!(status.allows_voice_from(Some(owner), blocked_by));
    }

    #[test]
    fn denial_applies_only_to_the_named_room_scope() {
        let mut status = AccessStatus::default();
        let denied = AccessScope::Scene("shared-id".to_owned());
        status.set_denied(denied.clone());

        assert!(status.is_denied(&denied));
        for unrelated in [
            AccessScope::Scene("other-scene".to_owned()),
            AccessScope::Realm("shared-id".to_owned()),
            AccessScope::Voice("shared-id".to_owned()),
            AccessScope::PrivateChat,
        ] {
            assert!(!status.is_denied(&unrelated));
        }
    }

    #[test]
    fn allowing_or_removing_a_scope_preserves_other_denials() {
        let mut status = AccessStatus::default();
        let scene = AccessScope::Scene("scene".to_owned());
        let voice = AccessScope::Voice("call".to_owned());
        status.set_denied(scene.clone());
        status.set_denied(voice.clone());

        status.set_allowed(&scene);
        assert!(!status.is_denied(&scene));
        assert!(status.is_denied(&voice));

        status.set_denied(scene.clone());
        status.remove_scope(&voice);
        assert!(!status.is_denied(&voice));
        assert!(status.is_denied(&scene));
    }

    #[test]
    fn block_snapshots_preserve_direction_and_apply_unblocks() {
        let mut status = AccessStatus::default();
        let owner = Address::from([4; 20]);
        let blocked = Address::from([1; 20]);
        let blocked_by = Address::from([2; 20]);
        let scene = AccessScope::Scene("scene".to_owned());
        status.set_identity(Some(owner));
        status.set_denied(scene.clone());

        status.replace_blocks(owner, [blocked], [blocked_by]);
        assert!(status.is_blocked(blocked));
        assert!(!status.is_blocked_by(blocked));
        assert!(!status.is_blocked(blocked_by));
        assert!(status.is_blocked_by(blocked_by));

        status.replace_blocks(owner, [], [blocked_by]);
        assert!(!status.is_blocked(blocked));
        assert!(status.is_blocked_by(blocked_by));
        assert!(status.is_denied(&scene));
    }

    #[test]
    fn identity_change_and_logout_clear_cached_access() {
        let mut status = AccessStatus::default();
        let first = Address::from([1; 20]);
        let second = Address::from([2; 20]);
        let blocked = Address::from([3; 20]);
        let room = AccessScope::PrivateChat;
        assert!(status.set_identity(Some(first)));
        status.set_denied(room.clone());
        status.replace_blocks(first, [blocked], [blocked]);

        assert!(!status.set_identity(Some(first)));
        assert!(status.is_denied(&room));
        assert!(status.is_blocked(blocked));
        assert!(status.is_blocked_by(blocked));

        for identity in [Some(second), None] {
            assert!(status.set_identity(identity));
            assert_eq!(status.identity(), identity);
            assert!(!status.is_denied(&room));
            assert!(!status.is_blocked(blocked));
            assert!(!status.is_blocked_by(blocked));
            status.set_denied(room.clone());
            if let Some(owner) = identity {
                status.replace_blocks(owner, [blocked], [blocked]);
            }
        }
    }
}
