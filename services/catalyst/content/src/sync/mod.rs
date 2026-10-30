//! Peer sync: bootstrap from a peer's snapshots and `/pointer-changes`, then
//! follow it. The write half (fetch + hash proof, auth-chain check, batched
//! persist, failure records) lives beside this module in `content::{
//! deploy_remote_entity, sync_deployer, batch_deployer}`; this module adds
//! the snapshot and pointer-changes readers, the per-peer cursor repository,
//! the retry worker and the orchestrator that sequences them, and reports
//! into the one [`SyncStatus`] the status routes read.

pub mod orchestrator;
pub mod pointer_changes;
pub mod repository;
pub mod retry_failed;
pub mod snapshots;
pub mod status_mirror;
pub mod worker;

use std::collections::HashSet;

pub use crate::sync_types::{
    AuthChain, AuthLink, AuthLinkType, DeploymentContext, EntityId, EntityType, FailedDeployment,
    FailureReason, Pointer, SyncDeployment, SyncError,
};
pub use crate::wire_types::{SnapshotMetadata, TimeRange, Timestamp};
pub use crate::{SyncState, SyncStatus};
pub use orchestrator::{SyncControl, SyncOrchestrator, SyncOrchestratorConfig, resume_point};
pub use repository::{LiveDeploymentRepository, LiveProcessedSnapshotStore};
pub use retry_failed::{RetryFailedConfig, RetryFailedDeployments};
pub use worker::{SyncHandles, SyncSettings};

pub const NON_PROFILE_TYPES: &[&str] = &["scene", "wearable", "emote", "store", "outfits"];
pub const SYNCABLE_TYPES: &[&str] = &["scene", "wearable", "emote", "store", "outfits", "profile"];

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum SyncConfigError {
    #[error("sync source '{0}' is not a valid URL: {1}")]
    InvalidUrl(String, String),

    #[error(
        "sync source '{0}' uses plaintext http:// to a non-loopback host ({1}); sync ingests \
         arbitrary entities from this peer, so a MITM could poison the index. Use https:// or \
         point at 127.0.0.1/localhost for development"
    )]
    PlaintextPeer(String, String),

    #[error("sync source '{0}' is this node itself; a node cannot sync from its own content API")]
    SelfReferential(String),

    #[error("sync source '{0}' is listed twice")]
    Duplicate(String),

    #[error("no sync source configured")]
    NoSources,

    #[error(
        "SYNC_ENTITY_TYPES entry '{0}' is not a syncable entity type (scene, wearable, emote, \
         store, outfits, profile)"
    )]
    UnknownEntityType(String),
}

/// A peer URL as the sync code keys it: trimmed, no trailing slash.
fn normalize_source(raw: &str) -> String {
    raw.trim().trim_end_matches('/').to_string()
}

/// A node's own reachable URLs reduced to what a peer entry would look like:
/// `https://host/content` and `https://host` both name this node.
fn own_identities(own_urls: &[String]) -> HashSet<String> {
    let mut ids = HashSet::new();
    for url in own_urls {
        let base = normalize_source(url);
        if base.is_empty() {
            continue;
        }
        ids.insert(base.trim_end_matches("/content").to_string());
        ids.insert(base);
    }
    ids
}

/// The comma-separated `SYNC_SOURCE` list, validated as the old binary did at
/// startup (invalid URL, plaintext http:// to a non-loopback host) plus the
/// mirror-loop guard: a source that names this node (`own_urls`: PUBLIC_URL,
/// CONTENT_URL) or appears twice is refused. Order is preserved.
pub fn parse_sync_sources(raw: &str, own_urls: &[String]) -> Result<Vec<String>, SyncConfigError> {
    let own = own_identities(own_urls);
    let mut seen = HashSet::new();
    let mut sources = Vec::new();
    for entry in raw.split(',') {
        let source = normalize_source(entry);
        if source.is_empty() {
            continue;
        }
        let parsed = url::Url::parse(&source)
            .map_err(|e| SyncConfigError::InvalidUrl(source.clone(), e.to_string()))?;
        let host = parsed.host_str().unwrap_or("").to_string();
        if parsed.scheme() == "http" {
            let is_loopback = matches!(host.as_str(), "localhost" | "127.0.0.1" | "[::1]" | "::1");
            if !is_loopback {
                return Err(SyncConfigError::PlaintextPeer(source, host));
            }
        }
        if own.contains(&source) || own.contains(source.trim_end_matches("/content")) {
            return Err(SyncConfigError::SelfReferential(source));
        }
        if !seen.insert(source.clone()) {
            return Err(SyncConfigError::Duplicate(source));
        }
        sources.push(source);
    }
    if sources.is_empty() {
        return Err(SyncConfigError::NoSources);
    }
    Ok(sources)
}

/// True when `SYNC_SOURCE` names any peer; an empty list is a node with no
/// sync configured at all (an island or a journey node).
pub fn names_a_source(raw: &str) -> bool {
    raw.split(',')
        .any(|entry| !normalize_source(entry).is_empty())
}

/// The `SYNC_ENTITY_TYPES` allowlist: `None` (empty) means every type. The
/// existing implementation marks snapshots processed for the configured subset,
/// so widening this later needs explicit replay/resync handling.
pub fn parse_entity_types(raw: &str) -> Result<Option<HashSet<String>>, SyncConfigError> {
    let mut set = HashSet::new();
    for t in raw.split(',').map(str::trim).filter(|s| !s.is_empty()) {
        if !SYNCABLE_TYPES.contains(&t) {
            return Err(SyncConfigError::UnknownEntityType(t.to_string()));
        }
        set.insert(t.to_string());
    }
    Ok((!set.is_empty()).then_some(set))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sources_are_normalized_deduplicated_and_ordered() {
        let sources = parse_sync_sources(
            " https://peer.decentraland.org/content/ ,, http://127.0.0.1:5140",
            &[],
        )
        .unwrap();
        assert_eq!(
            sources,
            vec![
                "https://peer.decentraland.org/content".to_string(),
                "http://127.0.0.1:5140".to_string()
            ]
        );
    }

    #[test]
    fn a_plaintext_non_loopback_peer_is_refused() {
        let err = parse_sync_sources("http://peer.decentraland.org/content", &[]).unwrap_err();
        assert!(
            matches!(err, SyncConfigError::PlaintextPeer(_, ref host) if host == "peer.decentraland.org")
        );
        assert!(parse_sync_sources("http://localhost:5140", &[]).is_ok());
        assert!(parse_sync_sources("http://[::1]:5140", &[]).is_ok());
    }

    #[test]
    fn garbage_and_empty_lists_are_refused() {
        assert!(matches!(
            parse_sync_sources("not a url", &[]),
            Err(SyncConfigError::InvalidUrl(..))
        ));
        assert_eq!(
            parse_sync_sources(" , ", &[]),
            Err(SyncConfigError::NoSources)
        );
    }

    #[test]
    fn the_loop_guard_refuses_this_node_and_repeats() {
        let own = vec![
            "https://v2.decentraland.zone".to_string(),
            "https://v2.decentraland.zone/content/".to_string(),
        ];
        for me in [
            "https://v2.decentraland.zone/content",
            "https://v2.decentraland.zone/",
            "https://v2.decentraland.zone/content/",
        ] {
            assert_eq!(
                parse_sync_sources(&format!("https://peer.decentraland.org/content,{me}"), &own),
                Err(SyncConfigError::SelfReferential(normalize_source(me))),
                "{me}"
            );
        }
        assert_eq!(
            parse_sync_sources(
                "https://peer.decentraland.org/content,https://peer.decentraland.org/content/",
                &own
            ),
            Err(SyncConfigError::Duplicate(
                "https://peer.decentraland.org/content".to_string()
            ))
        );
        assert!(parse_sync_sources("https://peer.decentraland.org/content", &own).is_ok());
    }

    #[test]
    fn entity_type_allowlist_is_validated() {
        assert_eq!(parse_entity_types("").unwrap(), None);
        assert_eq!(parse_entity_types(" , ").unwrap(), None);
        let set = parse_entity_types("scene, profile").unwrap().unwrap();
        assert!(set.contains("scene") && set.contains("profile") && set.len() == 2);
        assert_eq!(
            parse_entity_types("scene,land"),
            Err(SyncConfigError::UnknownEntityType("land".to_string()))
        );
    }
}
