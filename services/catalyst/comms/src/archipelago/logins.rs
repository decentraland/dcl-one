//! Login credit for a completed `/ws` handshake. The wallet signed this
//! connector's single-use challenge, so the login cannot be replayed. The
//! credit never delays or fails the handshake: it runs on a spawned task,
//! bounded in concurrency and backlog, and retries a transient failure with
//! the same login id.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use tokio::sync::Semaphore;

use crate::interfaces::{LoginRecordError, LoginRecorder};

/// Namespace of the login ids derived from a handshake's challenge.
const NAMESPACE: uuid::Uuid = uuid::uuid!("8c1f2b6e-4a7d-5e93-b0c2-71d5e4f8a6b3");
/// Concurrent recordings on the shared pool.
const CONCURRENCY: usize = 16;
/// Credits waiting or running; past it a credit is dropped (a later login that day counts).
const BACKLOG: usize = 4096;
const ATTEMPTS: u32 = 3;
const ATTEMPT_TIMEOUT: Duration = Duration::from_secs(5);
const RETRY_DELAY: Duration = Duration::from_secs(2);

pub struct LoginCredits {
    recorder: Arc<dyn LoginRecorder>,
    slots: Arc<Semaphore>,
    backlog: Arc<AtomicUsize>,
}

impl LoginCredits {
    pub fn new(recorder: Arc<dyn LoginRecorder>) -> Self {
        Self {
            recorder,
            slots: Arc::new(Semaphore::new(CONCURRENCY)),
            backlog: Arc::new(AtomicUsize::new(0)),
        }
    }

    /// The login id of the handshake that signed `challenge`.
    pub fn login_id(challenge: &str) -> uuid::Uuid {
        uuid::Uuid::new_v5(&NAMESPACE, challenge.as_bytes())
    }

    /// Credits `wallet`'s login for the handshake that signed `challenge`.
    pub fn credit(&self, wallet: &str, challenge: &str) {
        if self.backlog.fetch_add(1, Ordering::AcqRel) >= BACKLOG {
            self.backlog.fetch_sub(1, Ordering::AcqRel);
            metrics::counter!("archipelago_login_credits_dropped_total").increment(1);
            return;
        }
        let (recorder, slots, backlog) = (
            self.recorder.clone(),
            self.slots.clone(),
            self.backlog.clone(),
        );
        let (wallet, login_id) = (wallet.to_string(), Self::login_id(challenge));
        crate::shared::task::spawn(async move {
            if let Ok(_slot) = slots.acquire_owned().await {
                record(recorder.as_ref(), &wallet, login_id).await;
            }
            backlog.fetch_sub(1, Ordering::AcqRel);
        });
    }
}

async fn record(recorder: &dyn LoginRecorder, wallet: &str, login_id: uuid::Uuid) {
    for attempt in 1..=ATTEMPTS {
        match tokio::time::timeout(ATTEMPT_TIMEOUT, recorder.record_login(wallet, login_id)).await {
            Ok(Ok(())) => return,
            Ok(Err(LoginRecordError::Refused(reason))) => {
                tracing::debug!(wallet, reason, "login not credited");
                return;
            }
            Ok(Err(LoginRecordError::Transient(reason))) => {
                tracing::debug!(wallet, attempt, reason, "login credit failed");
            }
            Err(_) => tracing::debug!(wallet, attempt, "login credit timed out"),
        }
        if attempt < ATTEMPTS {
            tokio::time::sleep(RETRY_DELAY * attempt).await;
        }
    }
    metrics::counter!("archipelago_login_credits_failed_total").increment(1);
    tracing::warn!(wallet, "login not credited after {ATTEMPTS} attempts");
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    /// Fails transiently `failures` times, then records.
    struct Flaky {
        failures: AtomicUsize,
        calls: Mutex<Vec<uuid::Uuid>>,
    }

    #[async_trait::async_trait]
    impl LoginRecorder for Flaky {
        async fn record_login(
            &self,
            _: &str,
            login_id: uuid::Uuid,
        ) -> Result<(), LoginRecordError> {
            self.calls.lock().unwrap().push(login_id);
            if self.failures.load(Ordering::SeqCst) > 0 {
                self.failures.fetch_sub(1, Ordering::SeqCst);
                return Err(LoginRecordError::Transient("busy".into()));
            }
            Ok(())
        }
    }

    #[tokio::test(start_paused = true)]
    async fn a_transient_failure_retries_with_the_same_login_id() {
        let flaky = Arc::new(Flaky {
            failures: AtomicUsize::new(1),
            calls: Mutex::new(Vec::new()),
        });
        let credits = LoginCredits::new(flaky.clone());
        credits.credit("0x0000000000000000000000000000000000000001", "dcl-abc");
        tokio::time::sleep(Duration::from_secs(10)).await;
        let calls = flaky.calls.lock().unwrap().clone();
        assert_eq!(calls, vec![LoginCredits::login_id("dcl-abc"); 2]);
        assert_eq!(credits.backlog.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn each_challenge_is_its_own_login() {
        assert_ne!(
            LoginCredits::login_id("dcl-a"),
            LoginCredits::login_id("dcl-b")
        );
    }
}
