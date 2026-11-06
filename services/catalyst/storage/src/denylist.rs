//! The moderation projection every read that hands out bytes consults
//! ([`super::Storage`]). Content loads and edits it; the store owns it so the
//! routes serving blobs by hash cannot forget it.

use std::collections::{HashMap, HashSet};

use parking_lot::RwLock;

/// Entity and content ids this node refuses to serve, on every route that
/// hands out bytes from the ONE store (`/contents/{hash}` here and under the
/// worlds nest, the builder's `/contents/{hash}`) and in the active-entity
/// and pointer-changes reads. Its parts are kept apart so no source can
/// lift another's entry: the camera reel's withheld photo blobs
/// ([`Self::add`]/[`Self::remove`]: the builder and camera-reel states hold
/// the same `Arc`, and the camera reel refills it at boot from
/// `camera_reel.withheld_blobs` and on every rejection, un-rejection and
/// delete, `camera_reel::State::load_withheld`) and the operator's durable
/// list (`content.denylist`, `content::denylist`: [`Self::set_operator`] at boot and
/// on every reload pass, [`Self::operator_insert`]/[`Self::operator_remove`]
/// after a console edit). A third part holds the remote lists
/// (`DENYLIST_URLS`, Decentraland's moderation list on a mirror:
/// `content::denylist::reload_remote`), one set per URL, replaced by each
/// successful fetch of that URL and never written to the table. A local
/// deploy naming a listed id is refused (`content::denylist::refusal`).
#[derive(Default)]
pub struct Denylist {
    ids: RwLock<HashSet<String>>,
    operator: RwLock<OperatorIds>,
    remote: RwLock<HashMap<String, HashSet<String>>>,
}

#[derive(Default)]
struct OperatorIds {
    exact: HashSet<String>,
    /// The same ids lowercased: a pointer is compared case-insensitively.
    lowered: HashSet<String>,
}

impl Denylist {
    pub fn contains(&self, id: &str) -> bool {
        self.ids.read().contains(id)
            || self.operator.read().exact.contains(id)
            || self.remote.read().values().any(|ids| ids.contains(id))
    }

    /// Whether the operator listed this (lowercased) pointer.
    pub fn denies_pointer(&self, pointer: &str) -> bool {
        let operator = self.operator.read();
        operator.lowered.contains(pointer) || operator.exact.contains(pointer)
    }

    /// The camera reel's half: withholds one blob.
    pub fn add(&self, id: &str) -> bool {
        self.ids.write().insert(id.to_string())
    }

    /// The camera reel's half: releases one blob (never an operator entry).
    pub fn remove(&self, id: &str) -> bool {
        self.ids.write().remove(id)
    }

    /// The camera reel's and the operator's halves, sorted and deduplicated
    /// (the remote lists are not this node's: [`Self::remote_len`]).
    pub fn list(&self) -> Vec<String> {
        let mut v: Vec<String> = self.ids.read().iter().cloned().collect();
        v.extend(self.operator.read().exact.iter().cloned());
        v.sort();
        v.dedup();
        v
    }

    /// The operator's half, sorted.
    pub fn operator_ids(&self) -> Vec<String> {
        let mut v: Vec<String> = self.operator.read().exact.iter().cloned().collect();
        v.sort();
        v
    }

    /// Replaces the operator's half with `ids` (the table, loaded).
    pub fn set_operator(&self, ids: impl IntoIterator<Item = String>) {
        let exact: HashSet<String> = ids.into_iter().collect();
        let lowered = exact.iter().map(|id| id.to_lowercase()).collect();
        let previous =
            std::mem::replace(&mut *self.operator.write(), OperatorIds { exact, lowered });
        drop(previous);
    }

    pub fn operator_insert(&self, id: &str) -> bool {
        let mut operator = self.operator.write();
        operator.lowered.insert(id.to_lowercase());
        operator.exact.insert(id.to_string())
    }

    pub fn operator_remove(&self, id: &str) -> bool {
        let mut operator = self.operator.write();
        let removed = operator.exact.remove(id);
        if removed {
            let lowered = id.to_lowercase();
            if !operator
                .exact
                .iter()
                .any(|other| other.to_lowercase() == lowered)
            {
                operator.lowered.remove(&lowered);
            }
        }
        removed
    }

    /// Replaces the ids fetched from `source` (one remote list URL). The set
    /// is built before the write lock is taken and the old one dropped after
    /// it is released, so a read waits only for the swap.
    pub fn set_remote(&self, source: &str, ids: impl IntoIterator<Item = String>) {
        let ids: HashSet<String> = ids.into_iter().collect();
        let previous = self.remote.write().insert(source.to_string(), ids);
        drop(previous);
    }

    /// Distinct ids across the remote lists.
    pub fn remote_len(&self) -> usize {
        let remote = self.remote.read();
        let mut all: HashSet<&str> = HashSet::new();
        for ids in remote.values() {
            all.extend(ids.iter().map(String::as_str));
        }
        all.len()
    }
}
