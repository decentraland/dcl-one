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
    pub fn finish(&mut self, id: u32) -> bool {
        self.0.remove(&id) == Some(false)
    }
}

#[derive(Default, Clone, Debug)]
pub struct RetryState {
    pub attempts: u32,
    pub due: f64,
}

impl RetryState {
    pub fn failed(&mut self, now: f64, retry_after: Option<f64>) {
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

    pub fn reset(&mut self) {
        *self = Self::default();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_cancelled_page_connect_can_never_be_installed() {
        let mut pending = PendingConnections::default();
        pending.start(7);
        pending.cancel(7);
        assert!(!pending.finish(7));
        pending.start(8);
        assert!(pending.finish(8));
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
