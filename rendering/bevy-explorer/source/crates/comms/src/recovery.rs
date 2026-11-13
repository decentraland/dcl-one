/// Page-owned connection attempts may finish after their engine requester was dropped.
#[derive(Default)]
pub struct PendingConnections(std::collections::HashMap<u32, bool>);

impl PendingConnections {
    pub fn start(&mut self, id: u32) {
        self.0.insert(id, false);
    }
    pub fn cancel(&mut self, id: u32) {
        if let Some(cancelled) = self.0.get_mut(&id) {
            *cancelled = true;
        }
    }

    pub fn accepts_events(&self, id: u32) -> bool {
        self.0.get(&id) == Some(&false)
    }
    pub fn finish(&mut self, id: u32) -> bool {
        self.0.remove(&id) == Some(false)
    }
}

const STABLE_CONNECTION_SECS: f64 = 30.0;

#[derive(Default, Clone, Debug)]
pub struct RetryState {
    pub attempts: u32,
    pub due: f64,
    connected_since: Option<f64>,
}

impl RetryState {
    pub fn failed(&mut self, now: f64, retry_after: Option<f64>) {
        self.connected_since = None;
        let delay = (1u32 << self.attempts.min(3)).min(5) as f64;
        self.attempts = self.attempts.saturating_add(1);
        self.due = now
            + retry_after
                .filter(|v| v.is_finite())
                .unwrap_or_default()
                .max(delay);
    }

    pub fn ready(&self, now: f64) -> bool {
        now >= self.due
    }

    pub fn observe_connection(&mut self, healthy: bool, now: f64) {
        match (healthy, self.connected_since) {
            (false, _) => self.connected_since = None,
            (true, None) => self.connected_since = Some(now),
            (true, Some(since)) if now - since >= STABLE_CONNECTION_SECS => self.reset(),
            _ => {}
        }
    }

    pub fn reset(&mut self) {
        *self = Self::default();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn brief_connections_keep_backoff_until_thirty_seconds_healthy() {
        let mut retry = RetryState::default();
        let mut now = 0.0;
        for delay in [1.0, 2.0, 4.0, 5.0] {
            retry.observe_connection(true, now);
            now += 10.0;
            retry.observe_connection(true, now);
            retry.failed(now, None);
            assert_eq!(retry.due, now + delay);
            now = retry.due;
        }
        retry.observe_connection(true, now);
        retry.observe_connection(true, now + 29.999);
        assert_eq!(retry.attempts, 4);
        retry.observe_connection(true, now + 30.0);
        assert_eq!(retry.attempts, 0);
        retry.failed(now + 30.0, None);
        assert_eq!(retry.due, now + 31.0);
    }

    #[test]
    fn interrupted_health_starts_a_new_stability_window() {
        let mut retry = RetryState::default();
        retry.failed(0.0, Some(60.0));
        retry.observe_connection(true, 1.0);
        retry.observe_connection(false, 30.0);
        retry.observe_connection(true, 31.0);
        retry.observe_connection(true, 60.999);
        assert_eq!(retry.attempts, 1);
        assert_eq!(retry.due, 60.0);
        retry.observe_connection(true, 61.0);
        assert_eq!(retry.attempts, 0);
        assert_eq!(retry.due, 0.0);
    }

    #[test]
    fn a_cancelled_page_connect_can_never_be_installed() {
        let mut pending = PendingConnections::default();
        pending.start(7);
        assert!(pending.accepts_events(7));
        pending.cancel(7);
        assert!(!pending.accepts_events(7));
        assert!(!pending.finish(7));
        pending.start(8);
        assert!(pending.finish(8));
        assert!(!pending.accepts_events(8));
        pending.cancel(9);
        assert!(!pending.finish(9));
        assert!(!pending.finish(8));
    }

    #[test]
    fn failures_back_off_without_giving_up_and_success_resets() {
        let mut retry = RetryState::default();
        let mut now = 0.0;
        for delay in [1.0, 2.0, 4.0, 5.0, 5.0, 5.0] {
            retry.failed(now, None);
            assert!(!retry.ready(now + delay - 0.01));
            now += delay;
            assert!(retry.ready(now));
        }
        retry.reset();
        retry.failed(now, Some(30.0));
        assert!(!retry.ready(now + 29.0));
        assert!(retry.ready(now + 30.0));
        retry.reset();
        retry.failed(now, None);
        assert!(retry.ready(now + 1.0));
    }
}
