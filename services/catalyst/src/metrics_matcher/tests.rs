use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

// A queued or running detached closure must retain admission. Moving the permit
// into the caller, or acquiring it inside pool work, breaks these barriers.
#[test]
fn canceled_queued_and_running_matchers_keep_single_submission() {
    let configured = std::thread::available_parallelism().unwrap().get();
    for workers in [1, 2, configured] {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(workers)
            .max_blocking_threads(1)
            .enable_all()
            .build()
            .unwrap();
        runtime.block_on(async {
            tokio::time::timeout(Duration::from_secs(10), async {
                let admission = Arc::new(Semaphore::new(1));
                let calls = Arc::new(AtomicUsize::new(0));
                let (pool_release, pool_wait) = std::sync::mpsc::channel();
                let (pool_started, pool_start) = tokio::sync::oneshot::channel();
                let busy_pool = tokio::task::spawn_blocking(move || {
                    pool_started.send(()).unwrap();
                    pool_wait.recv_timeout(Duration::from_secs(8)).unwrap();
                });
                pool_start.await.unwrap();
                let (release, wait) = std::sync::mpsc::channel();
                let (started, start) = tokio::sync::oneshot::channel();
                let gate = admission.clone();
                let count = calls.clone();
                let first = tokio::spawn(async move {
                    admitted(gate, move || {
                        count.fetch_add(1, Ordering::SeqCst);
                        started.send(()).unwrap();
                        wait.recv_timeout(Duration::from_secs(8)).unwrap();
                        Ok(true)
                    }).await
                });
                while admission.available_permits() != 0 { tokio::task::yield_now().await; }
                first.abort();
                assert!(first.await.unwrap_err().is_cancelled());
                let (ready, mut readies) = tokio::sync::mpsc::unbounded_channel();
                let mut waiters = Vec::new();
                for _ in 0..32 {
                    let gate = admission.clone();
                    let count = calls.clone();
                    let ready = ready.clone();
                    waiters.push(tokio::spawn(async move {
                        ready.send(()).unwrap();
                        admitted(gate, move || { count.fetch_add(1, Ordering::SeqCst); Ok(true) }).await
                    }));
                }
                for _ in 0..32 { readies.recv().await.unwrap(); }
                assert_eq!(calls.load(Ordering::SeqCst), 0);
                assert_eq!(admission.available_permits(), 0, "queued canceled closure lost admission");
                // An unrelated async operation progresses while all matcher callers wait.
                assert_eq!(tokio::spawn(async { 7 }).await.unwrap(), 7);
                pool_release.send(()).unwrap();
                busy_pool.await.unwrap();
                start.await.unwrap();
                assert_eq!(calls.load(Ordering::SeqCst), 1);
                assert_eq!(admission.available_permits(), 0, "running canceled closure lost admission");
                let survivor = waiters.pop().unwrap();
                for waiter in waiters { waiter.abort(); assert!(waiter.await.unwrap_err().is_cancelled()); }
                release.send(()).unwrap();
                assert!(survivor.await.unwrap().unwrap());
                assert_eq!(calls.load(Ordering::SeqCst), 2);
                assert_eq!(admission.available_permits(), 1);
                eprintln!("admission workers={workers} callers=32 queued_cancel=true running_cancel=true executions=2");
            }).await.unwrap();
        });
    }
}

#[tokio::test]
async fn panicked_work_returns_error_and_releases_admission() {
    let gate = Arc::new(Semaphore::new(1));
    let error = admitted::<bool>(gate.clone(), || panic!("controlled evaluator panic"))
        .await
        .unwrap_err();
    assert!(error.0.contains("metrics matcher task failed"));
    assert!(admitted(gate.clone(), || Ok(true)).await.unwrap());
    gate.close();
    assert!(
        admitted(gate, || Ok(true))
            .await
            .unwrap_err()
            .0
            .contains("admission closed")
    );
}
