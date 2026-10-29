//! Newtypes that carry provenance: a value that came from a peer and a value that came
//! from a local request path must not be the same Rust type, because a `bool` on a
//! shared type means every reader has to remember to check it.
//!
//! The local side of the barrier is [`crate::comms::world_identity::WorldName`] (the old
//! `LocalWorldName`): the only type the permission core, the room resolver and the
//! local read routes accept. Nothing under `fed/` constructs one -- the
//! provenance gate in [`super::wire`] fails if a line here ever names its constructor
//! -- so a peer-reported name cannot reach `policy.rs` without somebody writing a line
//! that says so outside this module.

/// An admitted peer's id, in the canonical form defined by
/// [`super::registry::canonical_peer_id`].
///
/// Minted only by [`super::peers::AdmittedPeer::admit`], so holding one is evidence
/// that the entry cleared every gate in [`super::peers::PeerNotAdmitted`], and evidence
/// of *which* entry: the registry refuses a file naming two case-variant entries, so one
/// `PeerId` is exactly one line of the peer file. (Previously the shared registry keyed
/// on the raw id while the worlds crate lowercased it, and whichever of the two polled
/// second erased the other's mirror.)
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Serialize)]
pub struct PeerId(String);

impl PeerId {
    /// The only constructor, `pub(crate)` so no caller outside this crate can mint one.
    /// Named for its provenance so a reviewer sees at the call site that the value came
    /// out of admission.
    pub(crate) fn from_admitted(canonical: &str) -> Self {
        debug_assert_eq!(
            canonical,
            super::registry::canonical_peer_id(canonical),
            "PeerId::from_admitted must be handed an id already folded by \
             canonical_peer_id; the federation_worlds CHECK constraints and the \
             registry's own keying both assume that exact form"
        );
        Self(canonical.to_string())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for PeerId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// A world name as a *peer reported it*. Not verified, resolved, or agreed to serve.
///
/// Deliberately absent: `Deref<Target = str>`, `AsRef<str>`, `Display`,
/// `Into<String>`, `Into<WorldName>`. The single escape hatch is
/// `as_peer_reported_str`, greppable by name and gated by
/// [`super::wire::provenance_gate`] to `fed/{names,wire,store,handlers}.rs`.
///
/// Needed here where the social federation needs nothing of the kind because
/// `community_id_hex(creator, name, nonce)` puts the creator inside the identifier,
/// whereas nothing inside `foo.dcl.eth` distinguishes our record from a peer's.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct RemoteWorldName(String);

/// Longer than any real `.dcl.eth` name; the cap stops a peer spending our row width on
/// a megabyte of its own choosing.
const MAX_WORLD_NAME_LEN: usize = 255;

impl RemoteWorldName {
    /// Fails closed on anything that is not a plausible world name: `/`, `%`,
    /// whitespace, control bytes and non-ASCII are rejected, so `../`, `%00` and an
    /// embedded URL cannot survive.
    pub(crate) fn from_peer_listing(raw: &str) -> Option<Self> {
        Self::shaped(raw)
    }

    /// An operator naming a row *already in the mirror*, on the admin veto route. Same
    /// shape rules, different provenance: it asserts nothing about the peer and reaches
    /// nothing but a primary-key lookup in `worlds.federation_worlds`.
    ///
    /// `pub` where [`Self::from_peer_listing`] is `pub(crate)`, because this one cannot
    /// introduce a peer's claim (only an authenticated admin route and tests reach it)
    /// while `from_peer_listing` is the trust boundary and stays sealed inside `fed`.
    /// Neither yields a `WorldName`, so neither reaches the permission core.
    pub fn from_operator_veto_path(raw: &str) -> Option<Self> {
        Self::shaped(raw)
    }

    fn shaped(raw: &str) -> Option<Self> {
        let n = raw.trim().to_ascii_lowercase();
        let shaped = !n.is_empty()
            && n.len() <= MAX_WORLD_NAME_LEN
            && n.bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b'_'));
        shaped.then_some(Self(n))
    }

    /// The one way to get the bytes out. Greppable on purpose.
    pub fn as_peer_reported_str(&self) -> &str {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn remote_world_name_rejects_shapes() {
        assert_eq!(
            RemoteWorldName::from_peer_listing("FOO.DCL.ETH")
                .expect("a plain name is accepted")
                .as_peer_reported_str(),
            "foo.dcl.eth"
        );
        assert_eq!(
            RemoteWorldName::from_peer_listing("  spaced.dcl.eth  ")
                .expect("surrounding whitespace is trimmed, not rejected")
                .as_peer_reported_str(),
            "spaced.dcl.eth"
        );

        for hostile in [
            "",
            "   ",
            "../../etc/passwd",
            "a/b",
            "name%00.dcl.eth",
            "name with spaces.dcl.eth",
            "n\u{E4}me.dcl.eth",
            "name\n.dcl.eth",
            "http://evil.example/x",
            "name:8080",
            "name?query=1",
        ] {
            assert!(
                RemoteWorldName::from_peer_listing(hostile).is_none(),
                "peer-reported name {hostile:?} must be refused, not stored"
            );
        }

        let too_long = "a".repeat(MAX_WORLD_NAME_LEN + 1);
        assert!(RemoteWorldName::from_peer_listing(&too_long).is_none());
        let at_limit = "a".repeat(MAX_WORLD_NAME_LEN);
        assert!(RemoteWorldName::from_peer_listing(&at_limit).is_some());
    }

    #[test]
    fn the_operator_veto_constructor_applies_the_same_shape_rules() {
        assert!(RemoteWorldName::from_operator_veto_path("../x").is_none());
        assert_eq!(
            RemoteWorldName::from_operator_veto_path("A.dcl.eth")
                .unwrap()
                .as_peer_reported_str(),
            "a.dcl.eth"
        );
    }

    /// The local type is `world_identity::WorldName`; equal bytes, different types. The
    /// local constructor is named here through a string only, so the provenance gate
    /// (which bans it in code lines under `fed/`) stays honest about this file too.
    #[test]
    fn a_local_name_and_a_remote_name_are_not_the_same_type() {
        let remote = RemoteWorldName::from_peer_listing("Foo.dcl.eth").unwrap();
        assert_eq!(remote.as_peer_reported_str(), "foo.dcl.eth");
        assert_ne!(
            std::any::TypeId::of::<crate::comms::world_identity::WorldName>(),
            std::any::TypeId::of::<RemoteWorldName>(),
            "equal bytes, different types \u{2014} that is the whole point"
        );
    }
}
