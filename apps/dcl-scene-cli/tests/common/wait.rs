//! The crate's one wait for state a test can only poll: another process, a
//! port, a log file, a cache another task fills. Where the code under test can
//! signal, tests await that signal instead.
//!
//! Every `guard` here is a hang guard on the wall clock, never a race: a test
//! asks "did this ever happen", and the guard only bounds how long a broken
//! build may keep the suite waiting. It stays on the wall clock under a paused
//! tokio clock too, where the naps between checks cost no real time.
#![allow(dead_code)]

use std::future::Future;
use std::time::{Duration, Instant};

/// The guard on a frame or event that must arrive. Long enough that no load
/// on the machine trips it, so only a broken build ever waits it out.
pub const HANG_GUARD: Duration = Duration::from_secs(30);

const FIRST_NAP: Duration = Duration::from_millis(1);
const LONGEST_NAP: Duration = Duration::from_millis(100);

/// Checks until `check` answers `Some`, or `None` once `guard` has passed.
pub async fn until<T>(guard: Duration, mut check: impl AsyncFnMut() -> Option<T>) -> Option<T> {
    let deadline = Instant::now() + guard;
    let mut nap = FIRST_NAP;
    loop {
        if let Some(found) = check().await {
            return Some(found);
        }
        if Instant::now() >= deadline {
            return None;
        }
        tokio::time::sleep(nap).await;
        nap = (nap * 2).min(LONGEST_NAP);
    }
}

/// [`until`] for a thread with no runtime.
pub fn until_blocking<T>(guard: Duration, mut check: impl FnMut() -> Option<T>) -> Option<T> {
    let deadline = Instant::now() + guard;
    let mut nap = FIRST_NAP;
    loop {
        if let Some(found) = check() {
            return Some(found);
        }
        if Instant::now() >= deadline {
            return None;
        }
        std::thread::sleep(nap);
        nap = (nap * 2).min(LONGEST_NAP);
    }
}

/// Runs `test` on a current-thread runtime whose clock starts paused, so the
/// timers under test (a heartbeat, a backoff) fire as soon as nothing else can
/// run instead of after their real duration. `guard` is on the wall clock: a
/// tokio timeout inside a paused runtime would be advanced past as soon as the
/// test waits on real IO.
pub fn on_paused_clock<F, T>(guard: Duration, test: F)
where
    F: FnOnce() -> T + Send + 'static,
    T: Future<Output = ()>,
{
    let (done_tx, done_rx) = std::sync::mpsc::channel::<()>();
    let runner = std::thread::spawn(move || {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .start_paused(true)
            .build()
            .expect("a paused test runtime")
            .block_on(test());
        let _ = done_tx.send(());
    });
    match done_rx.recv_timeout(guard) {
        Ok(()) => runner.join().expect("the paused test thread"),
        Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
            if let Err(panic) = runner.join() {
                std::panic::resume_unwind(panic);
            }
        }
        Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
            panic!("the test was still running after {guard:?} on the wall clock")
        }
    }
}
