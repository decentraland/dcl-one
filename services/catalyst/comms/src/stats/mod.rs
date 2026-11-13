//! Read-only statistics projections from complete Pulse snapshots.
use crate::gatekeeper::bus::{NatsBus, NatsSubscription};
use crate::protocol::bus::statistics::{Assembler, Change, SNAPSHOTS_SUBJECT, Snapshot, Update};
use parking_lot::{Mutex, RwLock};
use std::collections::HashMap;
use std::sync::Arc;

pub mod content;
pub mod http;
pub mod render;
pub mod runtime;

/// Last complete snapshot. A subscription never clears it on link interruption.
#[derive(Default)]
pub struct Reader {
    transfer: Mutex<Replica>,
    visible: RwLock<Arc<Snapshot>>,
}
#[derive(Default)]
struct Replica {
    assembler: Assembler,
    producer: Option<uuid::Uuid>,
    revision: u64,
    gap: bool,
    peer_indexes: HashMap<String, usize>,
}
impl Replica {
    fn index_peers(&mut self, state: &Snapshot) {
        self.peer_indexes.clear();
        for (index, peer) in state.peers.iter().enumerate() {
            // Keep the same first-match behavior even for a duplicate input identifier.
            self.peer_indexes
                .entry(peer.address.clone())
                .or_insert(index);
        }
    }
}
impl Reader {
    /// Subscribe on serve's existing bus. The caller retains the guard for the reader lifetime.
    pub fn subscribe(self: &Arc<Self>, bus: &dyn NatsBus) -> NatsSubscription {
        if let Ok(runtime) = tokio::runtime::Handle::try_current() {
            let reader = Arc::downgrade(self);
            runtime.spawn(async move {
                let mut interval = tokio::time::interval(std::time::Duration::from_secs(1));
                loop {
                    interval.tick().await;
                    let Some(reader) = reader.upgrade() else {
                        break;
                    };
                    reader
                        .transfer
                        .lock()
                        .assembler
                        .expire(std::time::Instant::now());
                }
            });
        }
        let reader = self.clone();
        bus.subscribe(
            SNAPSHOTS_SUBJECT,
            None,
            Arc::new(move |_, bytes| {
                let mut replica = reader.transfer.lock();
                let Some((producer, update)) = replica
                    .assembler
                    .receive_update(bytes, std::time::Instant::now())
                else {
                    return;
                };
                let revision = update.revision();
                match update {
                    Update::Snapshot { state, .. } => {
                        if replica.producer == Some(producer) && revision < replica.revision {
                            return;
                        }
                        replica.index_peers(&state);
                        *reader.visible.write() = Arc::new(state);
                        replica.producer = Some(producer);
                        replica.revision = revision;
                        replica.gap = false;
                    }
                    Update::Change { change, .. } => {
                        if replica.producer != Some(producer) || revision <= replica.revision {
                            return;
                        }
                        if replica.gap || replica.revision.checked_add(1) != Some(revision) {
                            replica.gap = true;
                            return;
                        }
                        let mut visible = reader.visible.write();
                        let state = Arc::make_mut(&mut visible);
                        match change {
                            Change::Peer(peer) => {
                                if let Some(&index) = replica.peer_indexes.get(&peer.address) {
                                    state.peers[index] = peer;
                                } else {
                                    replica
                                        .peer_indexes
                                        .insert(peer.address.clone(), state.peers.len());
                                    state.peers.push(peer);
                                }
                            }
                            Change::Remove(address) => {
                                state.peers.retain(|p| p.address != address);
                                replica.peer_indexes.remove(&address);
                                for (index, peer) in state.peers.iter().enumerate() {
                                    if let Some(previous) =
                                        replica.peer_indexes.get_mut(&peer.address)
                                    {
                                        // Shift left without replacing keys; duplicates keep their first match.
                                        *previous = (*previous).min(index);
                                    }
                                }
                            }
                            Change::Islands(bytes) => state.islands = bytes,
                            Change::Discovery(bytes) => state.discovery = bytes,
                        }
                        replica.revision = revision;
                    }
                }
            }),
        )
    }
    pub fn snapshot(&self) -> Arc<Snapshot> {
        self.visible.read().clone()
    }
}
#[cfg(test)]
mod tests;

impl crate::content::about_comms::CommsCore for Reader {
    fn core_status(&self) -> (bool, u64) {
        let snapshot = self.snapshot();
        let core = render::core(&snapshot, chrono::Utc::now().timestamp_millis());
        (core.healthy, core.user_count as u64)
    }
    fn parcel_counts(&self) -> Vec<((i32, i32), u32)> {
        self.realm_view().1
    }
    fn realm_view(&self) -> (u64, Vec<((i32, i32), u32)>) {
        let snapshot = self.snapshot();
        let core = render::core(&snapshot, chrono::Utc::now().timestamp_millis());
        let mut counts = std::collections::BTreeMap::new();
        for peer in &snapshot.peers {
            let [x, y] = render::parcel(peer);
            if x.is_finite()
                && y.is_finite()
                && x >= i32::MIN as f64
                && x <= i32::MAX as f64
                && y >= i32::MIN as f64
                && y <= i32::MAX as f64
            {
                *counts.entry((x as i32, y as i32)).or_insert(0) += 1;
            }
        }
        (core.user_count as u64, counts.into_iter().collect())
    }
}
