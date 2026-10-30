use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, Weak};

use tokio::sync::Notify;

#[cfg(test)]
use crate::protocol::pulse::decentraland::pulse::PendingTakeover;

#[cfg(test)]
const RECEIPT_CAPACITY: usize = 10_000;

#[cfg(test)]
#[derive(Clone, Debug, PartialEq)]
pub(super) enum CleanupState {
    Dispatched,
    Quarantine(tokio::time::Instant),
    Confirmed,
}

#[cfg(test)]
#[derive(Clone)]
pub(super) struct CleanupReceipt {
    pub(super) operation: PendingTakeover,
    pub(super) cutoff: u64,
    pub(super) state: CleanupState,
}

/// Safety state outlives cancelled tasks and stopped subscriptions. Unknown dispatches never
/// expire or yield capacity. Confirmed work retires on positive acknowledgment, or only under
/// capacity pressure once its cutoff, token lifetime and clock allowance have elapsed.
#[cfg(test)]
#[derive(Default)]
pub(super) struct CleanupReceipts(Mutex<HashMap<(String, String, String), CleanupReceipt>>);

#[cfg(test)]
impl CleanupReceipts {
    pub(super) fn safe_to_mint(&self, wallet: &str) -> bool {
        !self
            .0
            .lock()
            .unwrap()
            .iter()
            .any(|((_, w, _), receipt)| w == wallet && receipt.state != CleanupState::Confirmed)
    }
    pub(super) fn reserve(
        &self,
        source: &str,
        wallet: &str,
        operation: &PendingTakeover,
        floor: u64,
        now: u64,
        token_ttl_seconds: u64,
    ) -> Option<(CleanupReceipt, bool)> {
        let mut receipts = self.0.lock().unwrap();
        let key = (source.into(), wallet.into(), operation.takeover_id.clone());
        if let Some(receipt) = receipts.get(&key) {
            return (receipt.operation == *operation).then(|| (receipt.clone(), false));
        }
        if receipts
            .iter()
            .any(|((_, w, _), receipt)| w == wallet && receipt.state == CleanupState::Dispatched)
        {
            return None;
        }
        if receipts.len() >= RECEIPT_CAPACITY {
            receipts.retain(|_, receipt| {
                receipt.state != CleanupState::Confirmed
                    || now
                        < receipt
                            .cutoff
                            .saturating_add(token_ttl_seconds)
                            .saturating_add(1)
            });
        }
        if receipts.len() >= RECEIPT_CAPACITY {
            return None;
        }
        let previous = receipts
            .iter()
            .filter(|((_, w, _), _)| w == wallet)
            .map(|(_, r)| r.cutoff)
            .max()
            .unwrap_or(0);
        let cutoff = now
            .checked_add(5)?
            .max(floor.checked_add(1)?)
            .max(previous.checked_add(1)?);
        if cutoff > crate::protocol::bus::cluster::MAX_CONSUMER_SECONDS
            || super::floor_wait_ms(cutoff, now.saturating_mul(1000)).is_none()
        {
            return None;
        }
        let receipt = CleanupReceipt {
            operation: operation.clone(),
            cutoff,
            state: CleanupState::Dispatched,
        };
        receipts.insert(key, receipt.clone());
        Some((receipt, true))
    }

    pub(super) fn set(&self, source: &str, wallet: &str, id: &str, state: CleanupState) {
        if let Some(receipt) =
            self.0
                .lock()
                .unwrap()
                .get_mut(&(source.into(), wallet.into(), id.into()))
        {
            receipt.state = state;
        }
    }

    pub(super) fn reject(&self, source: &str, wallet: &str, id: &str) {
        self.0
            .lock()
            .unwrap()
            .remove(&(source.into(), wallet.into(), id.into()));
    }

    pub(super) fn acknowledge(
        &self,
        snapshot: &crate::protocol::pulse::decentraland::pulse::PeerClusterSnapshot,
    ) {
        self.0
            .lock()
            .unwrap()
            .retain(|(source, wallet, id), receipt| {
                source != &snapshot.source_incarnation
                    || wallet != &snapshot.wallet
                    || receipt.state != CleanupState::Confirmed
                    || snapshot
                        .pending_takeovers
                        .iter()
                        .any(|operation| &operation.takeover_id == id)
            });
    }
}

#[derive(Default)]
pub(super) struct RecoveryOwners(Mutex<HashMap<String, HashMap<String, Weak<Attempt>>>>);

struct Attempt {
    session: String,
    cancelled: AtomicBool,
    changed: Notify,
}

impl Attempt {
    fn cancel(&self) {
        self.cancelled.store(true, Ordering::SeqCst);
        self.changed.notify_waiters();
    }
}

impl RecoveryOwners {
    pub(super) fn begin(self: &Arc<Self>, wallet: &str, session: &str) -> Option<RecoveryTicket> {
        let mut owners = self.0.lock().unwrap();
        let sessions = owners.entry(wallet.into()).or_default();
        if sessions
            .get(session)
            .and_then(Weak::upgrade)
            .is_some_and(|attempt| !attempt.cancelled.load(Ordering::SeqCst))
        {
            return None;
        }
        let attempt = Arc::new(Attempt {
            session: session.into(),
            cancelled: AtomicBool::new(false),
            changed: Notify::new(),
        });
        // An announcement identifies a socket, not Pulse's current owner. Another
        // device must not cancel a valid mint merely by reannouncing itself.
        sessions.insert(session.into(), Arc::downgrade(&attempt));
        Some(RecoveryTicket {
            wallet: wallet.into(),
            attempt,
            owners: self.clone(),
        })
    }

    pub(super) fn invalidate(&self, wallet: &str) {
        if let Some(sessions) = self.0.lock().unwrap().remove(wallet) {
            for attempt in sessions.into_values().filter_map(|a| a.upgrade()) {
                attempt.cancel();
            }
        }
    }

    pub(super) fn clear(&self) {
        for (_, sessions) in self.0.lock().unwrap().drain() {
            for attempt in sessions.into_values().filter_map(|a| a.upgrade()) {
                attempt.cancel();
            }
        }
    }
}

pub(super) struct RecoveryTicket {
    wallet: String,
    attempt: Arc<Attempt>,
    owners: Arc<RecoveryOwners>,
}

impl RecoveryTicket {
    pub(super) fn is_cancelled(&self) -> bool {
        self.attempt.cancelled.load(Ordering::SeqCst)
    }
    pub(super) async fn cancelled(&self) {
        loop {
            let changed = self.attempt.changed.notified();
            tokio::pin!(changed);
            changed.as_mut().enable();
            if self.attempt.cancelled.load(Ordering::SeqCst) {
                return;
            }
            changed.await;
        }
    }
}

impl Drop for RecoveryTicket {
    fn drop(&mut self) {
        let mut owners = self.owners.0.lock().unwrap();
        let Some(sessions) = owners.get_mut(&self.wallet) else {
            return;
        };
        if sessions
            .get(&self.attempt.session)
            .is_some_and(|owner| owner.ptr_eq(&Arc::downgrade(&self.attempt)))
        {
            sessions.remove(&self.attempt.session);
        }
        if sessions.is_empty() {
            owners.remove(&self.wallet);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn operation(id: &str) -> PendingTakeover {
        PendingTakeover {
            takeover_id: id.into(),
            displaced_session: "old".into(),
            displaced_cluster_id: "room".into(),
            displaced_realm: "realm".into(),
        }
    }

    #[test]
    fn dispatch_receipt_is_immutable_and_unknown_work_never_expires_or_releases() {
        let receipts = CleanupReceipts::default();
        let op = operation("one");
        let (initial, fresh) = receipts
            .reserve("source", "wallet", &op, 40, 100, 60)
            .unwrap();
        assert!(fresh);
        assert_eq!(initial.cutoff, 105);
        let (retry, fresh) = receipts
            .reserve("source", "wallet", &op, 9000, 100_000_000, 60)
            .unwrap();
        assert!(!fresh);
        assert_eq!(retry.cutoff, initial.cutoff);
        assert_eq!(retry.state, CleanupState::Dispatched);
        assert!(!receipts.safe_to_mint("wallet"));
        let mut retargeted = op.clone();
        retargeted.displaced_cluster_id = "other".into();
        assert!(
            receipts
                .reserve("source", "wallet", &retargeted, 0, 100, 60)
                .is_none()
        );
    }

    #[test]
    fn receipt_capacity_fails_closed_without_evicting_unknown_work() {
        let receipts = CleanupReceipts::default();
        for n in 0..RECEIPT_CAPACITY {
            assert!(
                receipts
                    .reserve("source", &format!("w{n}"), &operation("op"), 0, 100, 60)
                    .is_some()
            );
        }
        assert!(
            receipts
                .reserve("source", "excess", &operation("op"), 0, 100, 60)
                .is_none()
        );
        assert!(!receipts.safe_to_mint("w0"));
        assert_eq!(receipts.0.lock().unwrap().len(), RECEIPT_CAPACITY);
        receipts.set("source", "w0", "op", CleanupState::Confirmed);
        assert!(
            receipts
                .reserve("source", "excess", &operation("op"), 0, 165, 60)
                .is_none()
        );
        assert!(
            receipts
                .reserve("source", "excess", &operation("op"), 0, 166, 60)
                .is_some()
        );
        assert!(!receipts.safe_to_mint("w1"));
        assert_eq!(receipts.0.lock().unwrap().len(), RECEIPT_CAPACITY);
    }

    #[test]
    fn confirmed_receipt_resends_same_cutoff_until_authoritative_acknowledgment() {
        let receipts = CleanupReceipts::default();
        let op = operation("one");
        let (first, _) = receipts
            .reserve("source", "wallet", &op, 0, 100, 60)
            .unwrap();
        receipts.set("source", "wallet", "one", CleanupState::Confirmed);
        let (retry, fresh) = receipts
            .reserve("source", "wallet", &op, 10000, 50000, 60)
            .unwrap();
        assert!(!fresh);
        assert_eq!(first.cutoff, retry.cutoff);
        let mut snapshot = crate::protocol::pulse::decentraland::pulse::PeerClusterSnapshot {
            wallet: "wallet".into(),
            source_incarnation: "source".into(),
            pending_takeovers: vec![op],
            ..Default::default()
        };
        receipts.acknowledge(&snapshot);
        assert_eq!(receipts.0.lock().unwrap().len(), 1);
        snapshot.pending_takeovers.clear();
        receipts.acknowledge(&snapshot);
        assert!(receipts.0.lock().unwrap().is_empty());
    }

    #[test]
    fn distinct_sessions_do_not_cancel_each_other_and_each_coalesces() {
        let owners = Arc::new(RecoveryOwners::default());
        let first = owners.begin("wallet", "first").unwrap();
        let second = owners.begin("wallet", "second").unwrap();
        assert!(!first.attempt.cancelled.load(Ordering::SeqCst));
        assert!(!second.attempt.cancelled.load(Ordering::SeqCst));
        assert!(owners.begin("wallet", "first").is_none());
        assert!(owners.begin("wallet", "second").is_none());
        drop(first);
        assert_eq!(owners.0.lock().unwrap()["wallet"].len(), 1);
        drop(second);
        assert!(owners.0.lock().unwrap().is_empty());
    }

    #[test]
    fn invalidation_cancels_every_session_of_only_the_named_wallet() {
        let owners = Arc::new(RecoveryOwners::default());
        let first = owners.begin("wallet", "first").unwrap();
        let second = owners.begin("wallet", "second").unwrap();
        let other = owners.begin("other", "first").unwrap();
        owners.invalidate("wallet");
        assert!(first.attempt.cancelled.load(Ordering::SeqCst));
        assert!(second.attempt.cancelled.load(Ordering::SeqCst));
        assert!(!other.attempt.cancelled.load(Ordering::SeqCst));
        assert_eq!(owners.0.lock().unwrap().len(), 1);
        owners.clear();
        assert!(other.attempt.cancelled.load(Ordering::SeqCst));
        assert!(owners.0.lock().unwrap().is_empty());
    }

    #[test]
    fn stale_ticket_drop_cannot_release_the_replacement_attempt() {
        let owners = Arc::new(RecoveryOwners::default());
        let first = owners.begin("wallet", "session").unwrap();
        owners.invalidate("wallet");
        let second = owners.begin("wallet", "session").unwrap();
        drop(first);
        assert!(owners.begin("wallet", "session").is_none());
        drop(second);
        assert!(owners.0.lock().unwrap().is_empty());
    }
}
