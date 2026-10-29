//! Complete Pulse wallet topology for social consumers. Receipt age is local liveness,
//! not proof of source age: the existing wire has no source timestamp/incarnation.
//! Deploy one Pulse publisher per node; snapshots replace, never union publisher state.

use std::collections::BTreeSet;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::watch;
use tokio::time::Instant;

use crate::protocol::bus::{BusError, topology::decode_islands, wallet_token};

pub const TOPOLOGY_RECEIPT_TTL: Duration = Duration::from_secs(5);

#[derive(Clone, Debug)]
pub struct WalletTopology {
    pub wallets: Arc<BTreeSet<String>>,
    pub received_at: Option<Instant>,
    pub generation: u64,
}

impl WalletTopology {
    pub fn is_online(&self, wallet: &str) -> bool {
        self.received_at
            .is_some_and(|at| at.elapsed() < TOPOLOGY_RECEIPT_TTL)
            && self.wallets.contains(&wallet.to_ascii_lowercase())
    }
}

pub struct Topology {
    current: Mutex<WalletTopology>,
    changed: watch::Sender<u64>,
}

impl Default for Topology {
    fn default() -> Self {
        Self {
            current: Mutex::new(WalletTopology {
                wallets: Arc::new(BTreeSet::new()),
                received_at: None,
                generation: 0,
            }),
            changed: watch::channel(0).0,
        }
    }
}

impl Topology {
    pub fn changes(&self) -> watch::Receiver<u64> {
        self.changed.subscribe()
    }

    pub fn receive(&self, payload: &[u8]) -> Result<(), BusError> {
        let received_at = Instant::now();
        let message = decode_islands(payload)?;
        let wallets = Arc::new(
            message
                .data
                .into_iter()
                .flat_map(|island| island.peers)
                .filter_map(|peer| wallet_token(&peer))
                .collect::<BTreeSet<_>>(),
        );
        if received_at.elapsed() >= TOPOLOGY_RECEIPT_TTL {
            return Ok(());
        }
        let mut current = self.current.lock().unwrap();
        if current.received_at.is_some_and(|at| at > received_at) {
            return Ok(());
        }
        // Expiry is an effective transition even when the next frame repeats the set.
        let expired = current
            .received_at
            .is_some_and(|at| at.elapsed() >= TOPOLOGY_RECEIPT_TTL);
        if expired || current.wallets != wallets {
            current.generation += 1;
        }
        *current = WalletTopology {
            wallets,
            received_at: Some(received_at),
            generation: current.generation,
        };
        self.changed.send_replace(current.generation);
        Ok(())
    }

    pub fn snapshot(&self) -> WalletTopology {
        let mut current = self.current.lock().unwrap();
        if current
            .received_at
            .is_some_and(|at| at.elapsed() >= TOPOLOGY_RECEIPT_TTL)
            && !current.wallets.is_empty()
        {
            current.wallets = Arc::new(BTreeSet::new());
            current.generation += 1;
            self.changed.send_replace(current.generation);
        }
        current.clone()
    }

    /// Hold the same short lock as replacement through synchronous queue publication.
    /// A queued online update cannot pass a newer offline replacement.
    pub fn deliver_if_current(
        &self,
        generation: u64,
        wallet: &str,
        online: bool,
        deliver: impl FnOnce(),
    ) -> bool {
        let current = self.current.lock().unwrap();
        if current.generation != generation || current.is_online(wallet) != online {
            return false;
        }
        deliver();
        true
    }

    /// Unrelated wallet changes need not erase an already queued, still-true update.
    /// Compare its generation and current status again at queue consumption.
    pub fn queued_status_is_current(&self, generation: u64, wallet: &str, online: bool) -> bool {
        let current = self.current.lock().unwrap();
        generation <= current.generation && current.is_online(wallet) == online
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::pulse::decentraland::kernel::comms::v3::{
        IslandData, IslandStatusMessage,
    };
    use prost::Message;
    fn frame(peers: &[&str]) -> Vec<u8> {
        IslandStatusMessage {
            data: vec![IslandData {
                peers: peers.iter().map(|s| s.to_string()).collect(),
                ..Default::default()
            }],
        }
        .encode_to_vec()
    }
    const A: &str = "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    #[tokio::test]
    async fn initial_friend_remains_eligible_when_unrelated_topology_changes_during_profile_wait() {
        let state = Arc::new(Topology::default());
        state.receive(&frame(&[A])).unwrap();
        let initial = state.snapshot();
        let (resume, pending_profile) = tokio::sync::oneshot::channel();
        let reader = state.clone();
        let pending = tokio::spawn(async move {
            pending_profile.await.unwrap();
            reader.queued_status_is_current(initial.generation, A, true)
        });
        state
            .receive(&frame(&[A, "0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"]))
            .unwrap();
        resume.send(()).unwrap();
        assert!(
            pending.await.unwrap(),
            "unrelated wallet changes cannot discard stable initial ONLINE"
        );
    }

    #[tokio::test(start_paused = true)]
    async fn complete_replacement_invalid_retention_and_quiet_expiry() {
        let state = Topology::default();
        state
            .receive(&frame(&[A, &A.to_uppercase(), "not-a-wallet"]))
            .unwrap();
        let first = state.snapshot();
        assert_eq!(first.wallets.len(), 1);
        assert!(first.is_online(A));
        state.receive(&frame(&[A])).unwrap();
        assert_eq!(state.snapshot().generation, first.generation);
        let receipt = state.snapshot().received_at;
        assert!(state.receive(&[0xff]).is_err());
        assert!(
            state
                .receive(&vec![
                    0;
                    crate::protocol::bus::topology::MAX_ISLANDS_BYTES + 1
                ])
                .is_err()
        );
        assert_eq!(state.snapshot().received_at, receipt);
        tokio::time::advance(TOPOLOGY_RECEIPT_TTL).await;
        assert!(!state.snapshot().is_online(A));
        assert!(state.receive(&[0xff]).is_err());
        assert!(
            !state.snapshot().is_online(A),
            "invalid data cannot revive an expired receipt"
        );
        assert!(!state.deliver_if_current(first.generation, A, true, || panic!("stale delivery")));
        state.receive(&frame(&[A])).unwrap();
        assert!(state.snapshot().is_online(A));
        let online_generation = state.snapshot().generation;
        state
            .receive(&frame(&["0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"]))
            .unwrap();
        assert!(
            !state.snapshot().is_online(A),
            "valid nonempty omission clears immediately"
        );
        assert!(!state.queued_status_is_current(online_generation, A, true));
        state.receive(&[]).unwrap();
        assert!(state.snapshot().wallets.is_empty());
        assert!(state.snapshot().received_at.is_some());
    }
}
