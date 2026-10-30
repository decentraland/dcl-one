//! The peer file (`federation-peers.toml`): the one definition of what a peer id is,
//! the entry shape every federating domain shares, and the parse that refuses a file
//! naming one peer twice. Worlds is the first (and so far only) v2 consumer, so only
//! the reader is here: no `reload` (the peer set is fixed for the process lifetime; a
//! trust boundary that moves without a deploy is the thing admission exists to
//! prevent), no `audit`, no gossip key use.

use std::collections::HashMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

/// Canonical form: trimmed and ASCII-lowercased. Every id reaching the registry has
/// passed through [`canonical_peer_id`] once, at parse time, so there is one spelling
/// of a peer per process and a lookup cannot miss on case.
///
/// Never write `to_ascii_lowercase` at a call site: two callers each holding a private
/// idea of "the id" is how two peer-file entries came to share one mirror namespace,
/// the second poll erasing the first's rows. Peer ids are host names, hence
/// case-insensitive (RFC 4343); ASCII-only on purpose: a Unicode fold is locale-shaped
/// and not idempotent, and the `federation_worlds` CHECK constraints assert
/// `peer_id = lower(peer_id)` against Postgres's ASCII-for-ASCII `lower()`.
pub fn canonical_peer_id(raw: &str) -> String {
    raw.trim().to_ascii_lowercase()
}

fn default_version() -> u32 {
    1
}

/// One `[[peer]]` entry as the file spells it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerCert {
    #[serde(default = "default_version")]
    pub version: u32,
    /// As written in the file when the struct is built by hand; canonical in every
    /// `PeerCert` from [`FederationRegistry::from_file`], so the registry holds no raw
    /// ids at all.
    pub peer_id: String,
    pub catalyst_url: String,
    /// Distinct from `catalyst_url`, which is a *content* server base. Empty means the
    /// peer runs no worlds server; worlds federation then omits it rather than guessing
    /// a URL from `catalyst_url`.
    #[serde(default)]
    pub worlds_url: String,
    pub gossip_pubkey: [u8; 32],
    #[serde(default)]
    pub mtls_root_pem: String,
    #[serde(default)]
    pub dao_proposal: String,
    #[serde(default)]
    pub added_at: String,
}

#[derive(Debug, thiserror::Error)]
pub enum RegistryError {
    #[error("malformed payload: {0}")]
    Malformed(String),

    #[error(
        "peer file names {first:?} and {second:?}, which are the same peer: both canonicalise to \
         {canonical:?}. Peer ids are host names, and host names are case-insensitive (RFC 4343), \
         so these are one identity written twice \u{2014} not two peers. Whichever entry loaded second \
         would silently take the first's identity, along with its DAO proposal, its pinned root \
         and its mirror namespace. Delete one entry, or give them genuinely distinct ids."
    )]
    DuplicatePeerId {
        canonical: String,
        first: String,
        second: String,
    },
}

#[derive(Debug, Deserialize)]
struct PeerFile {
    #[serde(default)]
    peer: Vec<PeerCert>,
}

/// The parsed peer file, keyed by canonical id.
#[derive(Debug, Default)]
pub struct FederationRegistry {
    peers: HashMap<String, PeerCert>,
}

impl FederationRegistry {
    pub fn from_file(path: &Path) -> Result<Self, RegistryError> {
        Ok(Self {
            peers: Self::parse_file(path)?,
        })
    }

    /// Two entries whose ids canonicalise to the same value are a refusal naming both,
    /// never a merge: keeping either would make "we federate with X" and "we federate
    /// with the *other* X" the same observable state, settled by TOML document order,
    /// with no later moment at which anyone finds out which happened.
    fn parse_file(path: &Path) -> Result<HashMap<String, PeerCert>, RegistryError> {
        let raw = std::fs::read_to_string(path)
            .map_err(|e| RegistryError::Malformed(format!("peer file {}: {e}", path.display())))?;
        let parsed: PeerFile = toml::from_str(&raw)
            .map_err(|e| RegistryError::Malformed(format!("peer file {}: {e}", path.display())))?;

        let mut map = HashMap::with_capacity(parsed.peer.len());
        let mut as_written: HashMap<String, String> = HashMap::with_capacity(parsed.peer.len());
        for mut p in parsed.peer {
            if p.peer_id.trim().is_empty() {
                return Err(RegistryError::Malformed("peer_id is empty".into()));
            }
            if p.catalyst_url.trim().is_empty() {
                return Err(RegistryError::Malformed(format!(
                    "peer {}: catalyst_url is empty",
                    p.peer_id
                )));
            }
            if p.dao_proposal.trim().is_empty() {
                return Err(RegistryError::Malformed(format!(
                    "peer {}: dao_proposal is required (link to snapshot.dcl.eth proposal)",
                    p.peer_id
                )));
            }
            if p.added_at.trim().is_empty() {
                return Err(RegistryError::Malformed(format!(
                    "peer {}: added_at is required",
                    p.peer_id
                )));
            }
            let canonical = canonical_peer_id(&p.peer_id);
            if let Some(first) = as_written.get(&canonical) {
                return Err(RegistryError::DuplicatePeerId {
                    canonical,
                    first: first.clone(),
                    second: p.peer_id.clone(),
                });
            }
            as_written.insert(canonical.clone(), p.peer_id.clone());
            p.peer_id = canonical.clone();
            map.insert(canonical, p);
        }
        Ok(map)
    }

    /// Case-insensitive: [`canonical_peer_id`] is applied to both the needle and the key.
    pub fn contains(&self, peer: &str) -> bool {
        self.peers.contains_key(&canonical_peer_id(peer))
    }

    pub fn get(&self, peer: &str) -> Option<&PeerCert> {
        self.peers.get(&canonical_peer_id(peer))
    }

    pub fn all(&self) -> Vec<PeerCert> {
        self.peers.values().cloned().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(name: &str, body: &str) -> std::path::PathBuf {
        let dir =
            std::env::temp_dir().join(format!("catalyst-fed-registry-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(name);
        std::fs::write(&path, body).unwrap();
        path
    }

    const ENTRY: &str = "[[peer]]\npeer_id = \"{ID}\"\ncatalyst_url = \"https://p.org/content\"\n\
                         gossip_pubkey = [1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1]\n\
                         dao_proposal = \"https://snapshot.org/#/snapshot.dcl.eth/proposal/0x1\"\nadded_at = \"2026-01-02\"\n";

    #[test]
    fn canonical_id_is_trimmed_and_ascii_lowercased() {
        assert_eq!(canonical_peer_id("  Peer.Example.ORG "), "peer.example.org");
        assert_eq!(canonical_peer_id("peer.example.org"), "peer.example.org");
    }

    #[test]
    fn ids_are_canonical_in_the_registry_and_lookups_fold_case() {
        let path = write("one.toml", &ENTRY.replace("{ID}", " Peer.Example.ORG "));
        let reg = FederationRegistry::from_file(&path).unwrap();
        assert!(reg.contains("PEER.example.org"));
        assert_eq!(
            reg.get("peer.example.org").unwrap().peer_id,
            "peer.example.org"
        );
        assert_eq!(reg.all().len(), 1);
    }

    #[test]
    fn a_case_variant_duplicate_is_refused_naming_both_spellings() {
        let body = format!(
            "{}\n{}",
            ENTRY.replace("{ID}", "peer.example.org"),
            ENTRY.replace("{ID}", "Peer.Example.ORG")
        );
        let path = write("twins.toml", &body);
        let err = FederationRegistry::from_file(&path).unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("peer.example.org") && msg.contains("Peer.Example.ORG"),
            "{msg}"
        );
        assert!(matches!(err, RegistryError::DuplicatePeerId { .. }));
    }

    #[test]
    fn empty_required_fields_are_malformed_and_a_missing_file_is_too() {
        let path = write(
            "nodao.toml",
            &ENTRY.replace("{ID}", "p.org").replace(
                "dao_proposal = \"https://snapshot.org/#/snapshot.dcl.eth/proposal/0x1\"",
                "dao_proposal = \"\"",
            ),
        );
        let msg = FederationRegistry::from_file(&path)
            .unwrap_err()
            .to_string();
        assert!(msg.contains("dao_proposal is required"), "{msg}");

        let missing = std::env::temp_dir().join("catalyst-fed-registry-no-such-file.toml");
        let _ = std::fs::remove_file(&missing);
        assert!(matches!(
            FederationRegistry::from_file(&missing),
            Err(RegistryError::Malformed(_))
        ));

        let empty = write("empty.toml", "# nobody\n");
        assert!(
            FederationRegistry::from_file(&empty)
                .unwrap()
                .all()
                .is_empty()
        );
    }
}
