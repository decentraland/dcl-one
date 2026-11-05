//! Durable, bounded Explorer login receipts. Signing keys never enter this queue.
use serde::{Deserialize, Serialize};

pub const MAX_AGE_MS: u64 = 24 * 60 * 60 * 1000;
pub const MAX_PENDING: usize = 32;

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
    let before = queue.len();
    queue.retain(|pending| pending.unexpired(event.occurred_at));
    let mut dropped = before - queue.len();
    if queue.len() >= MAX_PENDING {
        let overflow = queue.len() - MAX_PENDING + 1;
        queue.drain(..overflow);
        dropped += overflow;
    }
    queue.push(event);
    dropped
}

pub fn retry_delay_ms(attempt: u32) -> u64 {
    (1_000_u64 << attempt.saturating_sub(1).min(9)).min(300_000)
}

/// A receiver that does not implement the additive endpoint (404/405) must not be hammered.
pub fn retryable_status(status: u16) -> bool {
    status == 408 || status == 429 || status >= 500
}

/// The sender may start only after all queue mutations have reached storage.
#[derive(Default)]
pub struct DurableLogins {
    pub events: Vec<PendingClientLogin>,
    pub loaded: bool,
    dirty: bool,
    saving: bool,
}

impl DurableLogins {
    pub fn load(&mut self, stored: Vec<PendingClientLogin>) -> usize {
        let fresh = std::mem::replace(&mut self.events, stored);
        let mut dropped = 0;
        for event in fresh {
            dropped += enqueue(&mut self.events, event);
        }
        self.loaded = true;
        self.dirty = true;
        dropped
    }
    pub fn push(&mut self, event: PendingClientLogin) -> usize {
        self.dirty = true;
        enqueue(&mut self.events, event)
    }
    pub fn remove(&mut self, id: &str) {
        self.events.retain(|event| event.login_id != id);
        self.dirty = true;
    }
    pub fn expire(&mut self, now: u64) -> usize {
        let before = self.events.len();
        self.events.retain(|event| event.unexpired(now));
        let dropped = before - self.events.len();
        self.dirty |= dropped > 0;
        dropped
    }
    pub fn begin_save(&mut self) -> Option<Vec<PendingClientLogin>> {
        if !self.loaded || !self.dirty || self.saving {
            return None;
        }
        self.dirty = false;
        self.saving = true;
        Some(self.events.clone())
    }
    pub fn finish_save(&mut self, success: bool) {
        self.saving = false;
        self.dirty |= !success;
    }
    pub fn ready(&self) -> bool {
        self.loaded && !self.dirty && !self.saving
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
        assert_eq!(second_save.len(), 3);
        queue.finish_save(true);
        assert!(queue.ready());
        queue.remove("restored");
        let removal = queue.begin_save().unwrap();
        queue.push(event("new", 3000));
        queue.finish_save(true);
        assert!(!queue.ready());
        let final_save = queue.begin_save().unwrap();
        assert!(!final_save.iter().any(|event| event.login_id == "restored"));
        assert_eq!(final_save.len(), removal.len() + 1);
        queue.finish_save(true);
        assert!(queue.ready());
    }

    #[test]
    fn backoff_is_bounded_and_permanent_errors_stop() {
        assert_eq!(
            (1..=4).map(retry_delay_ms).collect::<Vec<_>>(),
            vec![1000, 2000, 4000, 8000]
        );
        assert_eq!(retry_delay_ms(u32::MAX), 300_000);
        for status in [408, 429, 500, 503] {
            assert!(retryable_status(status));
        }
        for status in [204, 400, 401, 403, 404, 405] {
            assert!(!retryable_status(status));
        }
    }
}
