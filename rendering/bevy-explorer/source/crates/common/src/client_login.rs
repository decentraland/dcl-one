//! Durable, bounded Explorer login receipts. Signing keys never enter this queue.
use serde::{Deserialize, Serialize};

pub use platform::client_login_store::{MAX_AGE_MS, MAX_PENDING};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct PendingClientLogin {
    pub wallet: String,
    pub endpoint: String,
    pub login_id: String,
    pub occurred_at: u64,
}

impl PendingClientLogin {
    pub fn eligible(&self, wallet: &str, endpoint: &str, now: u64) -> bool {
        self.wallet == wallet && self.endpoint == endpoint && self.unexpired(now)
    }

    pub fn unexpired(&self, now: u64) -> bool {
        now.saturating_sub(self.occurred_at) < MAX_AGE_MS
    }

    pub fn metadata(&self) -> String {
        serde_json::json!({
            "signer": "dcl:explorer", "loginId": self.login_id, "occurredAt": self.occurred_at
        })
        .to_string()
    }
}

/// Every completed login is distinct, including a cached identity restored after restart.
/// Existing pending events keep their IDs and timestamps until acknowledged or expired.
pub fn enqueue(queue: &mut Vec<PendingClientLogin>, event: PendingClientLogin) -> usize {
    let before = queue.len() + 1;
    let now = event.occurred_at;
    queue.push(event);
    queue.retain(|pending| pending.unexpired(now));
    // Storage iteration order (UUID on native) does not describe occurrence age.
    queue.sort_by(|a, b| (a.occurred_at, &a.login_id).cmp(&(b.occurred_at, &b.login_id)));
    if queue.len() > MAX_PENDING {
        queue.drain(..queue.len() - MAX_PENDING);
    }
    before - queue.len()
}

pub fn retry_delay_ms(attempt: u32, entropy: u64) -> u64 {
    let ceiling = (2_000_u64 << attempt.saturating_sub(1).min(8)).min(300_000);
    let floor = (ceiling / 2).max(1_000);
    floor + entropy % (ceiling - floor + 1)
}

/// Deployment ordering can leave the additive endpoint unavailable temporarily.
pub fn retryable_status(status: u16) -> bool {
    matches!(status, 404 | 405 | 408 | 429) || status >= 500
}

/// Only new occurrences are added; loading a record never schedules another insertion.
#[derive(Clone, Default, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LoginChanges {
    pub additions: Vec<PendingClientLogin>,
    pub removals: Vec<String>,
}

impl LoginChanges {
    fn is_empty(&self) -> bool {
        self.additions.is_empty() && self.removals.is_empty()
    }
}

/// The sender may start only after all local queue mutations have reached storage.
#[derive(Default)]
pub struct DurableLogins {
    pub events: Vec<PendingClientLogin>,
    pub loaded: bool,
    changes: LoginChanges,
    saving: Option<LoginChanges>,
}

impl DurableLogins {
    pub fn load(&mut self, mut stored: Vec<PendingClientLogin>) -> usize {
        // Recovery can happen after memory-only delivery. Never resurrect its acknowledgments
        // or duplicate unsaved local additions when the store becomes available again.
        stored.retain(|event| {
            !self.changes.removals.contains(&event.login_id)
                && !self
                    .events
                    .iter()
                    .any(|local| local.login_id == event.login_id)
        });
        let fresh = std::mem::replace(&mut self.events, stored);
        let mut dropped = 0;
        for event in fresh {
            // Pre-load edits remain in changes; merging must not reinsert restored records.
            let mut before = self.events.clone();
            before.push(event.clone());
            dropped += enqueue(&mut self.events, event);
            self.record_removals(&before);
        }
        self.loaded = true;
        dropped
    }
    fn record_removals(&mut self, before: &[PendingClientLogin]) {
        for event in before {
            if !self
                .events
                .iter()
                .any(|kept| kept.login_id == event.login_id)
            {
                self.changes
                    .additions
                    .retain(|added| added.login_id != event.login_id);
                if !self.changes.removals.contains(&event.login_id) {
                    self.changes.removals.push(event.login_id.clone());
                }
            }
        }
    }
    pub fn push(&mut self, event: PendingClientLogin) -> usize {
        self.changes.additions.push(event.clone());
        let mut before = self.events.clone();
        before.push(event.clone());
        let dropped = enqueue(&mut self.events, event);
        self.record_removals(&before);
        dropped
    }
    pub fn remove(&mut self, id: &str) {
        let before = self.events.clone();
        self.events.retain(|event| event.login_id != id);
        self.record_removals(&before);
    }
    pub fn expire(&mut self, now: u64) -> usize {
        if self.events.iter().all(|event| event.unexpired(now)) {
            return 0;
        }
        let before = self.events.clone();
        self.events.retain(|event| event.unexpired(now));
        self.record_removals(&before);
        before.len() - self.events.len()
    }
    pub fn begin_save(&mut self) -> Option<LoginChanges> {
        if !self.loaded || self.changes.is_empty() || self.saving.is_some() {
            return None;
        }
        let changes = std::mem::take(&mut self.changes);
        self.saving = Some(changes.clone());
        Some(changes)
    }
    pub fn finish_save(&mut self, success: bool) {
        if let Some(mut saving) = self.saving.take().filter(|_| !success) {
            // New removals supersede additions from a failed/uncertain earlier write.
            saving
                .additions
                .retain(|event| !self.changes.removals.contains(&event.login_id));
            saving.additions.append(&mut self.changes.additions);
            saving.removals.append(&mut self.changes.removals);
            self.changes = saving;
        }
    }
    /// A different instance may have acknowledged or capped a record. Never reinsert it.
    /// New local additions made during this save still need their own persistence attempt.
    pub fn observe_saved_ids(&mut self, ids: &[String]) -> usize {
        let before = self.events.len();
        self.events.retain(|event| {
            ids.contains(&event.login_id)
                || self
                    .changes
                    .additions
                    .iter()
                    .any(|added| added.login_id == event.login_id)
        });
        before - self.events.len()
    }
    pub fn ready(&self) -> bool {
        self.loaded && self.changes.is_empty() && self.saving.is_none()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn event(id: &str, occurred_at: u64) -> PendingClientLogin {
        PendingClientLogin {
            wallet: "alice".into(),
            endpoint: "https://example.org/social-api/v1/client-login".into(),
            login_id: id.into(),
            occurred_at,
        }
    }

    #[test]
    fn recovery_does_not_resurrect_memory_ack_or_duplicate_unsaved_events() {
        let mut queue = DurableLogins::default();
        queue.load(Vec::new());
        queue.push(event("acknowledged", 1000));
        queue.push(event("pending", 1001));
        queue.remove("acknowledged");
        queue.load(vec![
            event("acknowledged", 1000),
            event("pending", 1001),
            event("restored", 1002),
        ]);
        assert_eq!(
            queue
                .events
                .iter()
                .map(|e| e.login_id.as_str())
                .collect::<Vec<_>>(),
            ["pending", "restored"]
        );
        let delta = queue.begin_save().unwrap();
        assert_eq!(delta.additions, [event("pending", 1001)]);
        assert_eq!(delta.removals, ["acknowledged"]);
        queue.finish_save(false);
        assert_eq!(queue.begin_save().unwrap(), delta);
    }
    #[test]
    fn restart_and_cached_login_keep_pending_event_and_add_distinct_completion() {
        let original = event("first", 1_000);
        let stored = serde_json::to_string(&vec![original.clone()]).unwrap();
        let mut restored = serde_json::from_str(&stored).unwrap();
        assert_eq!(enqueue(&mut restored, event("cached", 2_000)), 0);
        assert_eq!(restored, vec![original.clone(), event("cached", 2_000)]);
        assert_eq!(restored[0].metadata(), original.metadata());
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&original.metadata()).unwrap(),
            serde_json::json!({"signer":"dcl:explorer", "loginId":"first", "occurredAt":1000})
        );
    }

    #[test]
    fn pending_events_never_cross_account_or_service_and_expire() {
        let pending = event("first", 1_000);
        assert!(pending.eligible("alice", &pending.endpoint, 2_000));
        assert!(!pending.eligible("bob", &pending.endpoint, 2_000));
        assert!(!pending.eligible("alice", "https://other.example/v1/client-login", 2_000));
        assert!(!pending.eligible("alice", &pending.endpoint, 1_000 + MAX_AGE_MS));
    }

    #[test]
    fn queue_bounds_and_expiry_count_discarded_events() {
        let mut queue = vec![event("old", 0)];
        assert_eq!(enqueue(&mut queue, event("live", MAX_AGE_MS)), 1);
        for n in 1..MAX_PENDING {
            assert_eq!(enqueue(&mut queue, event(&n.to_string(), MAX_AGE_MS)), 0);
        }
        assert_eq!(enqueue(&mut queue, event("newest", MAX_AGE_MS)), 1);
        assert_eq!(queue.len(), MAX_PENDING);
        assert_eq!(queue.last().unwrap().login_id, "newest");
    }

    #[test]
    fn restored_uuid_order_does_not_choose_the_eviction_victim() {
        let mut queue = DurableLogins::default();
        // Native BTreeMap restores UUID order, opposite the occurrence order here.
        queue.load(
            (0..MAX_PENDING)
                .map(|i| event(&format!("{i:02}"), 1000 + (MAX_PENDING - i) as u64))
                .collect(),
        );
        assert_eq!(queue.push(event("new", 2000)), 1);
        assert!(queue.events.iter().any(|event| event.login_id == "00"));
        assert!(!queue.events.iter().any(|event| event.login_id == "31"));
        let changes = queue.begin_save().unwrap();
        assert_eq!(changes.removals, ["31"]);
        queue.finish_save(true);
        // A clock adjustment can make the newly completed event the oldest itself.
        assert_eq!(queue.push(event("older", 500)), 1);
        assert!(!queue.events.iter().any(|event| event.login_id == "older"));
        let changes = queue.begin_save().unwrap();
        assert!(changes.additions.is_empty());
        assert_eq!(changes.removals, ["older"]);
    }

    #[test]
    fn delayed_failed_and_overlapping_saves_gate_delivery() {
        let mut queue = DurableLogins::default();
        queue.push(event("first", 1000));
        assert!(!queue.ready());
        assert!(queue.begin_save().is_none());
        queue.load(vec![event("restored", 500)]);
        let first_save = queue.begin_save().unwrap();
        assert!(!queue.ready(), "HTTP cannot precede persistence completion");
        queue.finish_save(false);
        assert!(!queue.ready(), "HTTP cannot follow failed persistence");
        assert_eq!(queue.begin_save().unwrap(), first_save);
        queue.push(event("cached", 2000));
        queue.finish_save(true);
        assert!(!queue.ready(), "older save cannot mark newer login durable");
        let second_save = queue.begin_save().unwrap();
        assert_eq!(second_save.additions.len(), 1);
        assert_eq!(second_save.additions[0].login_id, "cached");
        queue.finish_save(true);
        assert!(queue.ready());
        queue.remove("restored");
        let removal = queue.begin_save().unwrap();
        assert_eq!(removal.removals, ["restored"]);
        queue.push(event("new", 3000));
        queue.finish_save(true);
        assert!(!queue.ready());
        let final_save = queue.begin_save().unwrap();
        assert_eq!(final_save.additions.len(), 1);
        assert_eq!(final_save.additions[0].login_id, "new");
        queue.finish_save(true);
        assert!(queue.ready());
    }

    #[test]
    fn stale_startup_and_storage_receipts_never_reinsert_another_instances_records() {
        let mut a = DurableLogins::default();
        let mut b = DurableLogins::default();
        a.load(Vec::new());
        b.load(Vec::new());
        a.push(event("a", 1000));
        assert!(
            b.begin_save().is_none(),
            "an unchanged stale snapshot makes no write"
        );
        let saved = a.begin_save().unwrap();
        assert_eq!(saved.additions[0].login_id, "a");
        a.push(event("new", 2000));
        a.finish_save(true);
        // Another instance already acknowledged or capped a; new is still awaiting storage.
        assert_eq!(a.observe_saved_ids(&[]), 1);
        assert_eq!(a.events[0].login_id, "new");
        assert!(!a.ready());
        let next = a.begin_save().unwrap();
        assert_eq!(next.additions.len(), 1);
        assert_eq!(next.additions[0].login_id, "new");
        a.remove("new");
        a.finish_save(false);
        let retry = a.begin_save().unwrap();
        assert!(
            retry.additions.is_empty(),
            "removal supersedes an uncertain addition"
        );
        assert_eq!(retry.removals, ["new"]);
    }

    #[test]
    fn backoff_is_bounded_and_permanent_errors_stop() {
        assert_eq!(
            (1..=4)
                .map(|attempt| retry_delay_ms(attempt, 0))
                .collect::<Vec<_>>(),
            vec![1000, 2000, 4000, 8000]
        );
        assert_eq!(
            retry_delay_ms(u32::MAX, u64::MAX),
            150_000 + u64::MAX % 150_001
        );
        for attempt in [1, 2, 4, 9, u32::MAX] {
            for entropy in [0, 1, 999, u64::MAX] {
                assert!((1_000..=300_000).contains(&retry_delay_ms(attempt, entropy)));
            }
            assert_ne!(retry_delay_ms(attempt, 0), retry_delay_ms(attempt, 1));
        }
        for status in [404, 405, 408, 429, 500, 503] {
            assert!(retryable_status(status));
        }
        for status in [204, 400, 401, 403] {
            assert!(!retryable_status(status));
        }
    }
}
