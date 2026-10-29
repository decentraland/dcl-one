//! One coalescing notification worker per replica. Only four wallet operations may
//! be in flight, each paged and deadline bounded. No task/recipient queue per frame.
use super::State;
use super::context::ConnectivityCursor;
use crate::comms::topology::TOPOLOGY_RECEIPT_TTL;
use crate::comms::topology::Topology;
use crate::jobs::Jobs;
use futures::{StreamExt, stream};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

const FANOUT_CONCURRENCY: usize = 4;

impl State {
    /// Called unconditionally for social replicas, separately from primary socket assignment.
    pub fn start_topology(self: &Arc<Self>, jobs: &Jobs, topology: Arc<Topology>) {
        let state = self.clone();
        self.ctx.set_topology(topology);
        jobs.spawn("social-pulse-topology", move |job| async move {
            tokio::select! {
                _ = job.cancel.cancelled() => {}
                _ = run(&state) => {}
            }
            Ok(())
        });
    }
}

async fn run(state: &State) {
    run_notifications(
        state.ctx.topology(),
        |wallet, online, generation, cursor| {
            let ctx = state.ctx.clone();
            async move {
                match ctx
                    .fan_topology_page(&wallet, online, generation, cursor.clone())
                    .await
                {
                    Ok(result) => result,
                    Err(error) => {
                        tracing::warn!(%error, "connectivity page failed; retaining cursor");
                        (cursor, false)
                    }
                }
            }
        },
    )
    .await;
}

async fn run_notifications<F, Fut>(topology: &Topology, fan: F)
where
    F: Fn(String, bool, u64, ConnectivityCursor) -> Fut,
    Fut: std::future::Future<Output = (ConnectivityCursor, bool)>,
{
    let mut changes = topology.changes();
    // May have reached a subset of recipients before supersession/error. Remember it
    // conservatively until an offline fan-out completes; never lose an owed offline.
    let mut announced = BTreeSet::<String>::new();
    let mut incomplete = BTreeSet::<String>::new();
    let mut progress = BTreeMap::<String, ConnectivityCursor>::new();
    loop {
        let snapshot = topology.snapshot();
        let offline: Vec<_> = announced
            .difference(&snapshot.wallets)
            .take(FANOUT_CONCURRENCY)
            .cloned()
            .collect();
        // Finish old offline work before starting new online work. This bounds retained
        // wallet state by one bounded frame plus at most four superseded operations.
        let online = offline.is_empty();
        let batch = if online {
            snapshot
                .wallets
                .iter()
                .filter(|wallet| !announced.contains(*wallet) || incomplete.contains(*wallet))
                .take(FANOUT_CONCURRENCY)
                .cloned()
                .collect::<Vec<_>>()
        } else {
            offline
        };
        if batch.is_empty() {
            let expiry = snapshot.received_at.map(|at| at + TOPOLOGY_RECEIPT_TTL);
            tokio::select! {
                result = changes.changed() => { if result.is_err() { return; } }
                _ = async { match expiry {
                    Some(at) if at > tokio::time::Instant::now() => tokio::time::sleep_until(at).await,
                    _ => std::future::pending::<()>().await,
                }} => {}
            }
            continue;
        }
        if online {
            announced.extend(batch.iter().cloned());
        }
        // Both statuses may have reached only some recipients before supersession.
        incomplete.extend(batch.iter().cloned());
        let generation = snapshot.generation;
        let mut results = stream::iter(batch.into_iter().map(|wallet| {
            let mut cursor = progress.remove(&wallet).unwrap_or_default();
            if cursor.online != Some(online) {
                cursor = ConnectivityCursor {
                    online: Some(online),
                    ..Default::default()
                };
            }
            let operation = fan(wallet.clone(), online, generation, cursor.clone());
            async move {
                let outcome = tokio::time::timeout(TOPOLOGY_RECEIPT_TTL, operation).await;
                let (cursor, completed) = outcome.unwrap_or((cursor, false));
                (wallet, cursor, completed)
            }
        }))
        .buffer_unordered(FANOUT_CONCURRENCY);
        let mut failed = false;
        let mut next_progress = Vec::new();
        while let Some((wallet, cursor, completed)) = results.next().await {
            if completed {
                incomplete.remove(&wallet);
                if !online {
                    announced.remove(&wallet);
                }
            } else {
                next_progress.push((wallet, cursor));
                failed = true;
            }
        }
        drop(results);
        progress.extend(next_progress);
        if failed {
            // No tight loop while SQL/profile operations are failing. Latest receipt is
            // one watch slot, and effective predicates still expire independently.
            tokio::select! {
                _ = changes.changed() => {}
                _ = tokio::time::sleep(std::time::Duration::from_millis(250)) => {}
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::comms::protocol::pulse::decentraland::kernel::comms::v3::{
        IslandData, IslandStatusMessage,
    };
    use prost::Message;
    use std::sync::{
        Mutex,
        atomic::{AtomicUsize, Ordering},
    };

    fn frame(wallets: &[String]) -> Vec<u8> {
        IslandStatusMessage {
            data: vec![IslandData {
                peers: wallets.to_vec(),
                ..Default::default()
            }],
        }
        .encode_to_vec()
    }
    async fn settle() {
        for _ in 0..40 {
            tokio::task::yield_now().await;
        }
    }

    #[tokio::test(start_paused = true)]
    async fn partial_offline_then_reappearance_replays_online_to_every_recipient() {
        let topology = Arc::new(Topology::default());
        let wallet = format!("0x{:040x}", 7);
        let recipients = Arc::new(Mutex::new([false, false]));
        let partial = Arc::new(tokio::sync::Notify::new());
        let resume = Arc::new(tokio::sync::Notify::new());
        let online_rounds = Arc::new(AtomicUsize::new(0));
        let task = {
            let topology = topology.clone();
            let recipients = recipients.clone();
            let partial = partial.clone();
            let resume = resume.clone();
            let rounds = online_rounds.clone();
            tokio::spawn(async move {
                run_notifications(&topology, |_, online, _, cursor| {
                    let recipients = recipients.clone();
                    let partial = partial.clone();
                    let resume = resume.clone();
                    let rounds = rounds.clone();
                    async move {
                        if online {
                            *recipients.lock().unwrap() = [true, true];
                            rounds.fetch_add(1, Ordering::SeqCst);
                            (cursor, true)
                        } else {
                            recipients.lock().unwrap()[0] = false;
                            partial.notify_one();
                            resume.notified().await;
                            (cursor, false)
                        }
                    }
                })
                .await;
            })
        };
        topology.receive(&frame(&[wallet.clone()])).unwrap();
        settle().await;
        assert_eq!(*recipients.lock().unwrap(), [true, true]);
        topology.receive(&[]).unwrap();
        partial.notified().await;
        assert_eq!(*recipients.lock().unwrap(), [false, true]);
        topology.receive(&frame(&[wallet.clone()])).unwrap();
        resume.notify_one();
        settle().await;
        tokio::time::advance(std::time::Duration::from_millis(250)).await;
        settle().await;
        assert_eq!(*recipients.lock().unwrap(), [true, true]);
        assert_eq!(online_rounds.load(Ordering::SeqCst), 2);
        topology.receive(&frame(&[wallet])).unwrap();
        settle().await;
        assert_eq!(online_rounds.load(Ordering::SeqCst), 2);
        task.abort();
    }

    #[tokio::test(start_paused = true)]
    async fn bounded_page_progress_survives_retry_rounds_without_restarting_first_page() {
        let topology = Arc::new(Topology::default());
        let pages = Arc::new(Mutex::new(Vec::new()));
        let task = {
            let topology = topology.clone();
            let pages = pages.clone();
            tokio::spawn(async move {
                run_notifications(&topology, |_, _, _, mut cursor| {
                    let pages = pages.clone();
                    async move {
                        let page = cursor.friend_after.parse::<usize>().unwrap_or(0);
                        pages.lock().unwrap().push(page);
                        cursor.friend_after = (page + 1).to_string();
                        (cursor, page == 2)
                    }
                })
                .await;
            })
        };
        topology
            .receive(&frame(&[format!("0x{:040x}", 7)]))
            .unwrap();
        settle().await;
        for _ in 0..4 {
            tokio::time::advance(std::time::Duration::from_millis(250)).await;
            settle().await;
        }
        assert_eq!(*pages.lock().unwrap(), vec![0, 1, 2]);
        task.abort();
    }

    #[tokio::test(start_paused = true)]
    async fn no_notifications_for_unchanged_frames_and_omission_and_expiry_diff_once() {
        let topology = Arc::new(Topology::default());
        let wallet = format!("0x{:040x}", 1);
        let events = Arc::new(Mutex::new(Vec::new()));
        let task = {
            let topology = topology.clone();
            let events = events.clone();
            tokio::spawn(async move {
                run_notifications(&topology, |wallet, online, generation, cursor| {
                    let events = events.clone();
                    async move {
                        events.lock().unwrap().push((wallet, online, generation));
                        (cursor, true)
                    }
                })
                .await;
            })
        };
        topology.receive(&frame(&[wallet.clone()])).unwrap();
        settle().await;
        topology.receive(&frame(&[wallet.clone()])).unwrap();
        settle().await;
        assert_eq!(events.lock().unwrap().len(), 1);
        topology.receive(&[]).unwrap();
        settle().await;
        assert_eq!(
            events
                .lock()
                .unwrap()
                .iter()
                .map(|(_, status, _)| *status)
                .collect::<Vec<_>>(),
            vec![true, false]
        );
        topology.receive(&frame(&[wallet])).unwrap();
        settle().await;
        tokio::time::advance(TOPOLOGY_RECEIPT_TTL).await;
        settle().await;
        assert_eq!(
            events
                .lock()
                .unwrap()
                .iter()
                .map(|(_, status, _)| *status)
                .collect::<Vec<_>>(),
            vec![true, false, true, false]
        );
        tokio::time::advance(TOPOLOGY_RECEIPT_TTL).await;
        settle().await;
        assert_eq!(events.lock().unwrap().len(), 4);
        task.abort();
    }

    #[tokio::test(start_paused = true)]
    async fn stalled_fanout_is_bounded_and_latest_generation_converges() {
        let topology = Arc::new(Topology::default());
        let active = Arc::new(AtomicUsize::new(0));
        let peak = Arc::new(AtomicUsize::new(0));
        let delivered = Arc::new(Mutex::new(Vec::new()));
        struct Active(Arc<AtomicUsize>);
        impl Drop for Active {
            fn drop(&mut self) {
                self.0.fetch_sub(1, Ordering::SeqCst);
            }
        }
        let task = {
            let topology = topology.clone();
            let active = active.clone();
            let peak = peak.clone();
            let delivered = delivered.clone();
            tokio::spawn(async move {
                run_notifications(&topology, |wallet, online, generation, cursor| {
                    let active = active.clone();
                    let peak = peak.clone();
                    let delivered = delivered.clone();
                    let topology = topology.clone();
                    async move {
                        let _guard = Active(active.clone());
                        let count = active.fetch_add(1, Ordering::SeqCst) + 1;
                        peak.fetch_max(count, Ordering::SeqCst);
                        if online {
                            tokio::time::sleep(std::time::Duration::from_secs(20)).await;
                        }
                        let complete =
                            topology.deliver_if_current(generation, &wallet, online, || {
                                delivered.lock().unwrap().push((wallet.clone(), online))
                            });
                        (cursor, complete)
                    }
                })
                .await;
            })
        };
        let wallets: Vec<_> = (0..12).map(|n| format!("0x{n:040x}")).collect();
        topology.receive(&frame(&wallets)).unwrap();
        settle().await;
        assert_eq!(active.load(Ordering::SeqCst), 4);
        // Many newer complete snapshots coalesce while the same four operations stall.
        for n in 12..100 {
            topology.receive(&frame(&[format!("0x{n:040x}")])).unwrap();
        }
        topology.receive(&[]).unwrap();
        tokio::time::advance(TOPOLOGY_RECEIPT_TTL).await;
        settle().await;
        tokio::time::advance(std::time::Duration::from_millis(250)).await;
        settle().await;
        assert_eq!(active.load(Ordering::SeqCst), 0);
        assert_eq!(peak.load(Ordering::SeqCst), 4);
        assert!(delivered.lock().unwrap().iter().all(|(_, online)| !online));
        assert!(topology.snapshot().wallets.is_empty());
        task.abort();
    }
}
