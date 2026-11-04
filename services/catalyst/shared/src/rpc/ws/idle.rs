//! Shared socket traffic timer. Automatic ping is a probe, not application activity.
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Notify;
use tokio::time::Instant;

#[derive(Clone)]
pub struct IdleTimer(Arc<Inner>);
struct Inner {
    timeout: Duration,
    state: std::sync::Mutex<(Instant, bool)>,
    changed: Notify,
}
pub enum IdleAction {
    Ping,
    Expired,
}
impl IdleTimer {
    /// A zero timeout disables both idle expiry and automatic ping.
    pub fn new(seconds: u64) -> Self {
        Self(Arc::new(Inner {
            timeout: Duration::from_secs(seconds),
            state: std::sync::Mutex::new((Instant::now(), false)),
            changed: Notify::new(),
        }))
    }
    pub fn activity(&self) {
        *self.0.state.lock().unwrap() = (Instant::now(), false);
        self.0.changed.notify_waiters();
    }
    /// Observes expiry without consuming the writer's Ping action. Social's
    /// reader uses this while a physical write can be stalled independently.
    pub async fn expired(&self) {
        if self.0.timeout.is_zero() {
            std::future::pending::<()>().await;
        }
        loop {
            let changed = self.0.changed.notified();
            tokio::pin!(changed);
            changed.as_mut().enable();
            let at = self.0.state.lock().unwrap().0;
            let Some(deadline) = at.checked_add(self.0.timeout) else {
                changed.await;
                continue;
            };
            tokio::select! {
                _ = changed => continue,
                _ = tokio::time::sleep_until(deadline) => {
                    if self.0.state.lock().unwrap().0 == at { return; }
                },
            }
        }
    }
    pub async fn next(&self) -> IdleAction {
        if self.0.timeout.is_zero() {
            std::future::pending::<()>().await;
        }
        loop {
            let changed = self.0.changed.notified();
            let (at, pinged) = *self.0.state.lock().unwrap();
            let Some(deadline) = at.checked_add(if pinged {
                self.0.timeout
            } else {
                self.0.timeout / 2
            }) else {
                changed.await;
                continue;
            };
            tokio::select! {
                _=changed=>continue,
                _=tokio::time::sleep_until(deadline)=> {
                    let mut state=self.0.state.lock().unwrap();
                    if *state!=(at,pinged) { continue; }
                    if pinged { return IdleAction::Expired; }
                    state.1=true;return IdleAction::Ping;
                }
            }
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test(start_paused = true)]
    async fn ping_then_expiry_and_incoming_activity_resets_the_clock() {
        let idle = IdleTimer::new(4);
        assert!(matches!(idle.next().await, IdleAction::Ping));
        tokio::time::advance(Duration::from_secs(1)).await;
        idle.activity();
        assert!(matches!(idle.next().await, IdleAction::Ping));
        assert!(matches!(idle.next().await, IdleAction::Expired));
        let disabled = IdleTimer::new(0);
        assert!(
            tokio::time::timeout(Duration::from_secs(3600), disabled.next())
                .await
                .is_err()
        );
    }

    #[tokio::test(start_paused = true)]
    async fn passive_expiry_preserves_ping_ownership_and_follows_activity() {
        let idle = IdleTimer::new(4);
        let observed = idle.clone();
        let expiry = tokio::spawn(async move { observed.expired().await });
        tokio::task::yield_now().await;
        tokio::time::advance(Duration::from_secs(2)).await;
        assert!(matches!(idle.next().await, IdleAction::Ping));
        assert!(!expiry.is_finished());
        tokio::time::advance(Duration::from_secs(1)).await;
        idle.activity();
        tokio::time::advance(Duration::from_millis(3999)).await;
        tokio::task::yield_now().await;
        assert!(!expiry.is_finished());
        assert!(matches!(idle.next().await, IdleAction::Ping));
        tokio::time::advance(Duration::from_millis(1)).await;
        expiry.await.unwrap();
        assert!(
            tokio::time::timeout(Duration::from_secs(3600), IdleTimer::new(0).expired())
                .await
                .is_err()
        );
    }
}
