use alloy_core::primitives::Address;
use bevy::{platform::collections::HashSet, prelude::Resource};

/// Admission scopes are independent, even when their identifiers match.
/// A denial never establishes a platform ban.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum AccessScope {
    Realm(String),
    Scene(String),
    PrivateChat,
}

/// Room admission observations for the active identity.
/// An absent denial means no denial is cached, not that admission is guaranteed.
#[derive(Resource, Default, Debug)]
pub struct AccessStatus {
    identity: Option<Address>,
    denied: HashSet<AccessScope>,
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn denial_applies_only_to_the_named_room_scope() {
        let mut status = AccessStatus::default();
        let denied = AccessScope::Scene("shared-id".to_owned());
        status.set_denied(denied.clone());

        assert!(status.is_denied(&denied));
        for unrelated in [
            AccessScope::Scene("other-scene".to_owned()),
            AccessScope::Realm("shared-id".to_owned()),
            AccessScope::PrivateChat,
        ] {
            assert!(!status.is_denied(&unrelated));
        }
    }

    #[test]
    fn allowing_or_removing_a_scope_preserves_other_denials() {
        let mut status = AccessStatus::default();
        let scene = AccessScope::Scene("scene".to_owned());
        let private_chat = AccessScope::PrivateChat;
        status.set_denied(scene.clone());
        status.set_denied(private_chat.clone());

        status.set_allowed(&scene);
        assert!(!status.is_denied(&scene));
        assert!(status.is_denied(&private_chat));

        status.set_denied(scene.clone());
        status.remove_scope(&private_chat);
        assert!(!status.is_denied(&private_chat));
        assert!(status.is_denied(&scene));
    }

    #[test]
    fn identity_change_and_logout_clear_cached_access() {
        let mut status = AccessStatus::default();
        let first = Address::from([1; 20]);
        let second = Address::from([2; 20]);
        let room = AccessScope::PrivateChat;
        assert!(status.set_identity(Some(first)));
        status.set_denied(room.clone());

        assert!(!status.set_identity(Some(first)));
        assert!(status.is_denied(&room));

        for identity in [Some(second), None] {
            assert!(status.set_identity(identity));
            assert_eq!(status.identity(), identity);
            assert!(!status.is_denied(&room));
            status.set_denied(room.clone());
        }
    }
}
