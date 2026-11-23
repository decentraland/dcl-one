//! Login credit for a completed `/ws` handshake. The wallet signed this
//! connector's single-use challenge, so the login cannot be replayed. The
//! credit never delays or fails the handshake: it runs on a spawned task,
//! bounded in concurrency and backlog, and retries a transient failure with
//! the same login id. The job `archipelago-login-credits` ends the pending
//! credits at shutdown and counts them as dropped.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use tokio::sync::{Notify, Semaphore};
use tokio_util::sync::CancellationToken;

use crate::interfaces::{LoginRecordError, LoginRecorder};
use crate::jobs::Jobs;

/// Namespace of the login ids derived from a handshake's challenge.
const NAMESPACE: uuid::Uuid = uuid::uuid!("8c1f2b6e-4a7d-5e93-b0c2-71d5e4f8a6b3");

#[derive(Clone, Copy)]
struct Limits {
    /// Attempts running at once on the shared pool; a retry's wait holds no slot.
    concurrency: usize,
    /// Credits waiting or running; past it a credit is dropped (a later login that day counts).
    backlog: usize,
    attempts: u32,
    attempt_timeout: Duration,
    retry_delay: Duration,
}

const LIMITS: Limits = Limits {
    concurrency: 16,
    backlog: 4096,
    attempts: 3,
    attempt_timeout: Duration::from_secs(5),
    retry_delay: Duration::from_secs(2),
};

pub struct LoginCredits {
    inner: Arc<Inner>,
}

struct Inner {
    recorder: Arc<dyn LoginRecorder>,
    limits: Limits,
    slots: Semaphore,
    backlog: AtomicUsize,
    /// Signalled when the backlog empties.
    idle: Notify,
    stop: CancellationToken,
}

impl LoginCredits {
    /// Credits logins to `recorder`; `jobs` stops them at shutdown.
    pub fn start(recorder: Arc<dyn LoginRecorder>, jobs: &Jobs) -> Self {
        Self::with_limits(recorder, jobs, LIMITS)
    }

    fn with_limits(recorder: Arc<dyn LoginRecorder>, jobs: &Jobs, limits: Limits) -> Self {
        let inner = Arc::new(Inner {
            recorder,
            limits,
            slots: Semaphore::new(limits.concurrency),
            backlog: AtomicUsize::new(0),
            idle: Notify::new(),
            stop: CancellationToken::new(),
        });
        let job = inner.clone();
        jobs.spawn("archipelago-login-credits", move |ctx| async move {
            ctx.cancel.cancelled().await;
            job.stop.cancel();
            while job.backlog.load(Ordering::Acquire) > 0 {
                job.idle.notified().await;
            }
            Ok(())
        });
        Self { inner }
    }

    /// The login id of the handshake that signed `challenge`.
    pub fn login_id(challenge: &str) -> uuid::Uuid {
        uuid::Uuid::new_v5(&NAMESPACE, challenge.as_bytes())
    }

    /// Credits `wallet`'s login for the handshake that signed `challenge`.
    pub fn credit(&self, wallet: &str, challenge: &str) {
        let inner = &self.inner;
        if inner.stop.is_cancelled() {
            dropped("shutdown");
            return;
        }
        if inner.backlog.fetch_add(1, Ordering::AcqRel) >= inner.limits.backlog {
            inner.backlog.fetch_sub(1, Ordering::AcqRel);
            dropped("backlog");
            return;
        }
        let inner = inner.clone();
        let (wallet, login_id) = (wallet.to_string(), Self::login_id(challenge));
        crate::shared::task::spawn(async move {
            tokio::select! {
                biased;
                _ = inner.stop.cancelled() => dropped("shutdown"),
                _ = inner.record(&wallet, login_id) => {}
            }
            if inner.backlog.fetch_sub(1, Ordering::AcqRel) == 1 {
                inner.idle.notify_one();
            }
        });
    }
}

fn dropped(reason: &'static str) {
    metrics::counter!("archipelago_login_credits_dropped_total", "reason" => reason).increment(1);
}

impl Inner {
    async fn record(&self, wallet: &str, login_id: uuid::Uuid) {
        let limits = self.limits;
        for attempt in 1..=limits.attempts {
            let outcome = {
                let Ok(_slot) = self.slots.acquire().await else {
                    return;
                };
                tokio::time::timeout(
                    limits.attempt_timeout,
                    self.recorder.record_login(wallet, login_id),
                )
                .await
            };
            match outcome {
                Ok(Ok(())) => return,
                Ok(Err(LoginRecordError::Refused(reason))) => {
                    metrics::counter!("archipelago_login_credits_refused_total").increment(1);
                    tracing::info!(wallet, reason, "login not credited");
                    return;
                }
                Ok(Err(LoginRecordError::Transient(reason))) => {
                    tracing::debug!(wallet, attempt, reason, "login credit failed");
                }
                Err(_) => tracing::debug!(wallet, attempt, "login credit timed out"),
            }
            if attempt < limits.attempts {
                tokio::time::sleep(limits.retry_delay * attempt).await;
            }
        }
        metrics::counter!("archipelago_login_credits_failed_total").increment(1);
        tracing::warn!(
            wallet,
            "login not credited after {} attempts",
            limits.attempts
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::VecDeque;
    use std::sync::Mutex;

    enum Step {
        Transient,
        Refused,
        Hang,
    }

    /// Plays `steps` in order, one per call, then records.
    #[derive(Default)]
    struct Scripted {
        steps: Mutex<VecDeque<Step>>,
        calls: Mutex<Vec<uuid::Uuid>>,
    }

    impl Scripted {
        fn new(steps: impl IntoIterator<Item = Step>) -> Arc<Self> {
            Arc::new(Self {
                steps: Mutex::new(steps.into_iter().collect()),
                calls: Mutex::default(),
            })
        }

        fn calls(&self) -> Vec<uuid::Uuid> {
            self.calls.lock().unwrap().clone()
        }
    }

    #[async_trait::async_trait]
    impl LoginRecorder for Scripted {
        async fn record_login(
            &self,
            _: &str,
            login_id: uuid::Uuid,
        ) -> Result<(), LoginRecordError> {
            self.calls.lock().unwrap().push(login_id);
            let step = self.steps.lock().unwrap().pop_front();
            match step {
                None => Ok(()),
                Some(Step::Transient) => Err(LoginRecordError::Transient("busy".into())),
                Some(Step::Refused) => Err(LoginRecordError::Refused("quota".into())),
                Some(Step::Hang) => std::future::pending().await,
            }
        }
    }

    const WALLET: &str = "0x0000000000000000000000000000000000000001";

    fn id(challenge: &str) -> uuid::Uuid {
        LoginCredits::login_id(challenge)
    }

    fn backlog(credits: &LoginCredits) -> usize {
        credits.inner.backlog.load(Ordering::SeqCst)
    }

    #[tokio::test(start_paused = true)]
    async fn a_transient_failure_retries_with_the_same_login_id() {
        let recorder = Scripted::new([Step::Transient]);
        let credits = LoginCredits::start(recorder.clone(), &Jobs::new());
        credits.credit(WALLET, "dcl-abc");
        tokio::time::sleep(Duration::from_secs(10)).await;
        assert_eq!(recorder.calls(), vec![id("dcl-abc"); 2]);
        assert_eq!(backlog(&credits), 0);
    }

    #[tokio::test(start_paused = true)]
    async fn a_refused_login_is_not_retried() {
        let recorder = Scripted::new([Step::Refused]);
        let credits = LoginCredits::start(recorder.clone(), &Jobs::new());
        credits.credit(WALLET, "dcl-abc");
        tokio::time::sleep(Duration::from_secs(10)).await;
        assert_eq!(recorder.calls(), vec![id("dcl-abc")]);
        assert_eq!(backlog(&credits), 0);
    }

    #[tokio::test(start_paused = true)]
    async fn an_attempt_that_times_out_is_retried() {
        let recorder = Scripted::new([Step::Hang]);
        let credits = LoginCredits::start(recorder.clone(), &Jobs::new());
        credits.credit(WALLET, "dcl-abc");
        tokio::time::sleep(LIMITS.attempt_timeout + LIMITS.retry_delay / 2).await;
        assert_eq!(recorder.calls(), vec![id("dcl-abc")]);
        tokio::time::sleep(LIMITS.retry_delay).await;
        assert_eq!(recorder.calls(), vec![id("dcl-abc"); 2]);
        assert_eq!(backlog(&credits), 0);
    }

    #[tokio::test(start_paused = true)]
    async fn a_credit_waiting_to_retry_holds_no_slot() {
        let recorder = Scripted::new([Step::Transient]);
        let limits = Limits {
            concurrency: 1,
            ..LIMITS
        };
        let credits = LoginCredits::with_limits(recorder.clone(), &Jobs::new(), limits);
        credits.credit(WALLET, "dcl-a");
        tokio::time::sleep(Duration::from_millis(10)).await;
        credits.credit(WALLET, "dcl-b");
        tokio::time::sleep(Duration::from_millis(10)).await;
        assert_eq!(recorder.calls(), vec![id("dcl-a"), id("dcl-b")]);
    }

    #[tokio::test(start_paused = true)]
    async fn a_credit_past_the_backlog_is_dropped() {
        let recorder = Scripted::new([Step::Hang, Step::Hang, Step::Hang]);
        let limits = Limits {
            backlog: 2,
            ..LIMITS
        };
        let credits = LoginCredits::with_limits(recorder.clone(), &Jobs::new(), limits);
        for challenge in ["dcl-a", "dcl-b", "dcl-c"] {
            credits.credit(WALLET, challenge);
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
        assert_eq!(backlog(&credits), 2);
        assert_eq!(recorder.calls(), vec![id("dcl-a"), id("dcl-b")]);
    }

    #[tokio::test(start_paused = true)]
    async fn shutdown_ends_the_pending_credits() {
        let recorder = Scripted::new([Step::Hang, Step::Hang]);
        let jobs = Jobs::new();
        let credits = LoginCredits::start(recorder.clone(), &jobs);
        credits.credit(WALLET, "dcl-a");
        credits.credit(WALLET, "dcl-b");
        tokio::time::sleep(Duration::from_millis(10)).await;
        assert_eq!(backlog(&credits), 2);
        assert!(jobs.shutdown(Duration::from_secs(1)).await.is_empty());
        assert_eq!(backlog(&credits), 0);
        credits.credit(WALLET, "dcl-c");
        tokio::time::sleep(Duration::from_millis(10)).await;
        assert_eq!(backlog(&credits), 0);
        assert_eq!(recorder.calls().len(), 2);
    }

    #[test]
    fn each_challenge_is_its_own_login() {
        assert_ne!(id("dcl-a"), id("dcl-b"));
    }
}
