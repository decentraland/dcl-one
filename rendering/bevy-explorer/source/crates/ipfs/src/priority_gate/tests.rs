use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use bevy::math::FloatOrd;
use common::partition::{LoadPriority, Tier};

use super::*;

fn key(parcel_distance: u16, is_behind: bool, sqr_distance: f32) -> LoadPriority {
    LoadPriority {
        tier: Tier::Scene,
        parcel_distance,
        is_behind,
        sqr_distance: FloatOrd(sqr_distance),
    }
}

fn scene(name: &str) -> Option<String> {
    Some(name.to_owned())
}

/// Register each hash once, as its scene root or avatar would.
fn register(gate: &PriorityGate, entries: &[(&str, LoadPriority)]) {
    for (name, key) in entries {
        gate.register(name, *key);
    }
}

#[test]
fn has_waiters_only_while_a_ticket_is_queued() {
    let gate = PriorityGate::new(1);
    assert!(!gate.has_waiters());
    // the first ticket takes the one slot at once; the second waits for it
    let first = gate.ticket(scene("a"));
    assert!(!gate.has_waiters());
    let second = gate.ticket(scene("b"));
    assert!(gate.has_waiters());
    // the slot comes back with the first ticket and goes to the second
    drop(first);
    assert!(!gate.has_waiters());
    drop(second);
    assert!(!gate.has_waiters());
}

/// Yield until `count` tickets are queued; a ticket that never queues fails the test.
async fn settle(gate: &Arc<PriorityGate>, count: usize) {
    for _ in 0..1000 {
        if gate.waiting() == count {
            return;
        }
        tokio::task::yield_now().await;
    }
    panic!("{} tickets never queued", count);
}

type Log = Arc<Mutex<Vec<&'static str>>>;

/// A reader that takes a slot on `ticket`, records `name`, and frees the slot.
fn reader(
    gate: &Arc<PriorityGate>,
    name: &'static str,
    ticket: Arc<Ticket>,
    log: &Log,
) -> tokio::task::JoinHandle<()> {
    let gate = gate.clone();
    let log = log.clone();
    tokio::spawn(async move {
        let permit = gate.acquire(ticket).await.unwrap();
        log.lock().unwrap().push(name);
        drop(permit);
    })
}

#[tokio::test]
async fn grants_the_nearest_in_front_scene_first() {
    let gate = PriorityGate::new(1);
    register(
        &gate,
        &[
            ("far", key(5, false, 6400.0)),
            ("near", key(1, false, 100.0)),
            ("near_behind", key(1, true, 100.0)),
        ],
    );
    let holder = gate.try_acquire().unwrap();
    let log = Log::default();
    let tasks = ["far", "near", "near_behind"]
        .map(|name| reader(&gate, name, gate.ticket(scene(name)), &log));
    settle(&gate, 3).await;

    drop(holder);
    for task in tasks {
        task.await.unwrap();
    }

    assert_eq!(*log.lock().unwrap(), vec!["near", "near_behind", "far"]);
}

#[tokio::test]
async fn a_key_change_is_honoured_at_grant_time() {
    let gate = PriorityGate::new(1);
    register(
        &gate,
        &[("a", key(5, false, 6400.0)), ("b", key(1, false, 100.0))],
    );
    let holder = gate.try_acquire().unwrap();
    let log = Log::default();
    let tasks = ["a", "b"].map(|name| reader(&gate, name, gate.ticket(scene(name)), &log));
    settle(&gate, 2).await;

    // the player turned round: a is now the near one
    gate.publish([("a", key(1, false, 100.0)), ("b", key(5, false, 6400.0))]);
    drop(holder);
    for task in tasks {
        task.await.unwrap();
    }

    assert_eq!(*log.lock().unwrap(), vec!["a", "b"]);
}

#[tokio::test]
async fn an_unregistered_hash_ranks_after_the_registered_ones() {
    let gate = PriorityGate::new(1);
    register(&gate, &[("known", key(1, false, 100.0))]);
    let holder = gate.try_acquire().unwrap();
    let log = Log::default();
    let tasks =
        ["known", "unknown"].map(|name| reader(&gate, name, gate.ticket(scene(name)), &log));
    settle(&gate, 2).await;

    drop(holder);
    for task in tasks {
        task.await.unwrap();
    }

    assert_eq!(*log.lock().unwrap(), vec!["known", "unknown"]);
}

#[tokio::test]
async fn requests_without_a_scene_rank_after_live_scenes_and_before_departed_ones() {
    let gate = PriorityGate::new(1);
    register(
        &gate,
        &[
            ("left", key(1, false, 100.0)),
            ("live", key(5, true, 6400.0)),
        ],
    );
    let holder = gate.try_acquire().unwrap();
    let log = Log::default();
    let tasks = [
        reader(&gate, "left", gate.ticket(scene("left")), &log),
        reader(&gate, "imposter", gate.ticket(None), &log),
        reader(&gate, "live", gate.ticket(scene("live")), &log),
    ];
    settle(&gate, 3).await;

    // "left" has despawned: its root released the hash
    gate.unregister("left");
    drop(holder);
    for task in tasks {
        task.await.unwrap();
    }

    assert_eq!(*log.lock().unwrap(), vec!["live", "imposter", "left"]);
}

#[tokio::test]
async fn same_key_is_first_come_first_served() {
    let gate = PriorityGate::new(1);
    let holder = gate.try_acquire().unwrap();
    let log = Log::default();
    let tasks = ["first", "second"].map(|name| reader(&gate, name, gate.ticket(None), &log));
    settle(&gate, 2).await;

    drop(holder);
    for task in tasks {
        task.await.unwrap();
    }

    assert_eq!(*log.lock().unwrap(), vec!["first", "second"]);
}

#[tokio::test]
async fn one_scenes_requests_keep_their_order() {
    let gate = PriorityGate::new(1);
    register(&gate, &[("s", key(1, false, 100.0))]);
    let holder = gate.try_acquire().unwrap();
    let log = Log::default();
    let tasks = ["s1", "s2", "s3"].map(|name| reader(&gate, name, gate.ticket(scene("s")), &log));
    settle(&gate, 3).await;

    drop(holder);
    for task in tasks {
        task.await.unwrap();
    }

    assert_eq!(*log.lock().unwrap(), vec!["s1", "s2", "s3"]);
}

#[tokio::test]
async fn a_cancelled_waiter_does_not_hold_a_slot() {
    let gate = PriorityGate::new(1);
    register(
        &gate,
        &[
            ("far", key(5, false, 6400.0)),
            ("near", key(1, false, 100.0)),
        ],
    );
    let holder = gate.try_acquire().unwrap();
    let log = Log::default();
    let far = reader(&gate, "far", gate.ticket(scene("far")), &log);
    let near = reader(&gate, "near", gate.ticket(scene("near")), &log);
    settle(&gate, 2).await;

    // the near reader is dropped before it is granted
    near.abort();
    assert!(near.await.unwrap_err().is_cancelled());
    drop(holder);
    far.await.unwrap();

    assert_eq!(*log.lock().unwrap(), vec!["far"]);
    assert_eq!(gate.waiting(), 0);
    assert!(gate.try_acquire().is_some(), "the slot came back");
}

#[tokio::test]
async fn a_ticket_nobody_awaits_does_not_hold_a_slot() {
    let gate = PriorityGate::new(1);
    register(
        &gate,
        &[
            ("far", key(5, false, 6400.0)),
            ("near", key(1, false, 100.0)),
        ],
    );
    let holder = gate.try_acquire().unwrap();
    let log = Log::default();
    let far = reader(&gate, "far", gate.ticket(scene("far")), &log);
    // a read that queued a ticket and then went away without ever awaiting it
    drop(gate.ticket(scene("near")));
    settle(&gate, 1).await;

    drop(holder);
    far.await.unwrap();

    assert_eq!(*log.lock().unwrap(), vec!["far"]);
    assert_eq!(gate.waiting(), 0);
    assert!(gate.try_acquire().is_some(), "the slot came back");
}

#[tokio::test]
async fn forget_permits_frees_only_what_is_available() {
    let gate = PriorityGate::new(2);
    let holder = gate.try_acquire().unwrap();

    assert_eq!(gate.forget_permits(2), 1);
    assert!(gate.try_acquire().is_none());

    drop(holder);
    let returned = gate.try_acquire();
    assert!(returned.is_some(), "the held slot returns");
    assert!(gate.try_acquire().is_none());

    drop(returned);
    gate.add_permits(1);
    let first = gate.try_acquire();
    let second = gate.try_acquire();
    assert!(first.is_some() && second.is_some());
}

#[tokio::test]
async fn a_joined_reader_raises_the_download() {
    let gate = PriorityGate::new(1);
    register(
        &gate,
        &[
            ("far", key(5, false, 6400.0)),
            ("mid", key(3, false, 2500.0)),
            ("near", key(1, false, 100.0)),
        ],
    );
    let holder = gate.try_acquire().unwrap();
    let log = Log::default();
    let far_ticket = gate.ticket(scene("far"));
    let tasks = [
        reader(&gate, "far-download", far_ticket.clone(), &log),
        reader(&gate, "mid-download", gate.ticket(scene("mid")), &log),
    ];
    settle(&gate, 2).await;

    // a reader from the near scene joins the far scene's download
    gate.join(&far_ticket, scene("near"));
    drop(holder);
    for task in tasks {
        task.await.unwrap();
    }

    // granted once, as the near scene's; the far queue's copy is skipped, not granted again
    assert_eq!(*log.lock().unwrap(), vec!["far-download", "mid-download"]);
    assert_eq!(gate.waiting(), 0);
    assert!(gate.try_acquire().is_some());
}

#[tokio::test]
async fn a_departed_scenes_downloads_sink_to_the_bottom() {
    let gate = PriorityGate::new(1);
    register(
        &gate,
        &[
            ("left", key(1, false, 100.0)),
            ("ahead", key(2, false, 1000.0)),
        ],
    );
    let holder = gate.try_acquire().unwrap();
    let log = Log::default();
    let tasks = ["left", "ahead"].map(|name| reader(&gate, name, gate.ticket(scene(name)), &log));
    settle(&gate, 2).await;

    // the player walked out of "left": its root despawned and released the hash
    gate.unregister("left");
    drop(holder);
    for task in tasks {
        task.await.unwrap();
    }

    assert_eq!(*log.lock().unwrap(), vec!["ahead", "left"]);
}

#[tokio::test]
async fn a_publish_updates_registered_keys_only() {
    let gate = PriorityGate::new(1);
    register(&gate, &[("ahead", key(2, false, 1000.0))]);
    let holder = gate.try_acquire().unwrap();
    let log = Log::default();
    let tasks = [
        reader(&gate, "ahead", gate.ticket(scene("ahead")), &log),
        reader(&gate, "unknown", gate.ticket(scene("unknown")), &log),
        reader(&gate, "no-hash", gate.ticket(None), &log),
    ];
    settle(&gate, 3).await;

    // a publish that names "unknown" does not register it: it stays with the no-hash requests,
    // oldest first
    gate.publish([
        ("ahead", key(2, false, 1000.0)),
        ("unknown", key(0, false, 0.0)),
    ]);
    assert_eq!(gate.scene_priority("unknown"), None);
    drop(holder);
    for task in tasks {
        task.await.unwrap();
    }

    assert_eq!(*log.lock().unwrap(), vec!["ahead", "unknown", "no-hash"]);
}

#[tokio::test]
async fn a_request_after_a_departed_queue_drained_ranks_with_the_unregistered_ones() {
    let gate = PriorityGate::new(1);
    gate.register("s", key(1, false, 100.0));
    let holder = gate.try_acquire().unwrap();
    let old = gate.ticket(scene("s"));
    gate.unregister("s");
    // the old request is granted and holds the slot; its queue is drained but not yet dropped
    drop(holder);
    assert_eq!(gate.waiting(), 0);

    // a new request under the same, unregistered, hash: it is not a departed scene's
    let log = Log::default();
    let tasks = [
        reader(&gate, "s-new", gate.ticket(scene("s")), &log),
        reader(&gate, "no-hash", gate.ticket(None), &log),
    ];
    settle(&gate, 2).await;
    drop(old);
    for task in tasks {
        task.await.unwrap();
    }

    assert_eq!(*log.lock().unwrap(), vec!["s-new", "no-hash"]);
}

#[test]
fn a_shared_hash_stays_registered_until_its_last_holder_releases_it() {
    let gate = PriorityGate::new(1);
    gate.register("shared", key(5, false, 6400.0));
    gate.register("shared", key(1, false, 100.0));
    assert_eq!(
        gate.scene_priority("shared"),
        Some(key(1, false, 100.0)),
        "the best of its holders"
    );

    gate.unregister("shared");
    assert_eq!(gate.scene_priority("shared"), Some(key(1, false, 100.0)));
    gate.unregister("shared");
    assert_eq!(gate.scene_priority("shared"), None);
    // releasing again is harmless
    gate.unregister("shared");
    assert_eq!(gate.scene_priority("shared"), None);
}

#[test]
fn a_publish_takes_the_best_key_of_the_frame_and_replaces_the_last_frames() {
    let gate = PriorityGate::new(1);
    gate.register("w", key(1, false, 100.0));

    // the holder walked away: the frame's only key replaces the old one, however good it was
    gate.publish([("w", key(5, false, 6400.0))]);
    assert_eq!(gate.scene_priority("w"), Some(key(5, false, 6400.0)));

    // two holders in one frame: the best wins
    gate.publish([("w", key(5, false, 6400.0)), ("w", key(2, false, 1000.0))]);
    assert_eq!(gate.scene_priority("w"), Some(key(2, false, 1000.0)));
}

#[tokio::test]
async fn a_hash_registered_again_ranks_by_its_key_while_its_old_requests_wait() {
    let gate = PriorityGate::new(1);
    register(
        &gate,
        &[
            ("s", key(1, false, 100.0)),
            ("ahead", key(2, false, 1000.0)),
        ],
    );
    let holder = gate.try_acquire().unwrap();
    let log = Log::default();
    let tasks = ["s", "ahead"].map(|name| reader(&gate, name, gate.ticket(scene(name)), &log));
    settle(&gate, 2).await;

    // the scene left and came back before its downloads were granted
    gate.unregister("s");
    gate.register("s", key(1, false, 100.0));
    drop(holder);
    for task in tasks {
        task.await.unwrap();
    }

    assert_eq!(*log.lock().unwrap(), vec!["s", "ahead"]);
}

/// Per-frame and per-grant cost at a dense-area load: hundreds of distinct hashes queued
/// (scenes plus wearables), thousands of waiting requests. Run with `--ignored --nocapture`.
#[tokio::test]
#[ignore]
async fn overhead_at_hundreds_of_queues() {
    const HASHES: usize = 500;
    const REQUESTS_PER_HASH: usize = 10;
    let gate = PriorityGate::new(1);
    let holder = gate.try_acquire().unwrap();
    let keys: HashMap<String, LoadPriority> = (0..HASHES)
        .map(|i| {
            (
                format!("hash-{i}"),
                key((i % 40) as u16, i % 3 == 0, i as f32),
            )
        })
        .collect();
    for (hash, key) in &keys {
        gate.register(hash, *key);
    }
    let mut tickets = Vec::new();
    for i in 0..HASHES {
        for _ in 0..REQUESTS_PER_HASH {
            tickets.push(gate.ticket(scene(&format!("hash-{i}"))));
        }
    }
    assert_eq!(gate.waiting(), HASHES * REQUESTS_PER_HASH);

    // a frame's publish: every registered key written in place
    let start = std::time::Instant::now();
    for _ in 0..100 {
        gate.publish(keys.iter().map(|(hash, key)| (hash.as_str(), *key)));
    }
    let publish = start.elapsed() / 100;

    // every request awaits its ticket, as downloads do; freeing the one slot then drains the
    // whole queue, one grant per request, each permit dropped as soon as it arrives
    let tasks: Vec<_> = tickets
        .into_iter()
        .map(|ticket| {
            let gate = gate.clone();
            tokio::spawn(async move {
                drop(gate.acquire(ticket).await.unwrap());
            })
        })
        .collect();
    drop(holder);
    let start = std::time::Instant::now();
    for task in tasks {
        task.await.unwrap();
    }
    let per_grant = start.elapsed() / (HASHES * REQUESTS_PER_HASH) as u32;
    println!(
        "gate: {HASHES} queues, {} requests: publish {:?} per frame, grant {:?} each (task wake included)",
        HASHES * REQUESTS_PER_HASH,
        publish,
        per_grant
    );
}

#[tokio::test]
async fn late_join_keeps_the_shared_tickets_original_fifo_sequence() {
    let gate = PriorityGate::new(1);
    gate.register("near", key(0, false, 0.0));
    let holder = gate.try_acquire().unwrap();
    let shared = gate.ticket(scene("far"));
    let later = gate.ticket(scene("near"));
    gate.join(&shared, scene("near"));
    let log = Log::default();
    let a = reader(&gate, "older-shared", shared, &log);
    let b = reader(&gate, "later", later, &log);
    drop(holder);
    a.await.unwrap();
    b.await.unwrap();
    assert_eq!(*log.lock().unwrap(), vec!["older-shared", "later"]);
}

#[tokio::test]
async fn abandoned_receivers_return_a_slot_without_recursing() {
    use std::{
        future::{poll_fn, Future},
        task::Poll,
    };
    let gate = PriorityGate::new(1);
    let blocker = gate.try_acquire().unwrap();
    let mut held = Vec::new();
    for _ in 0..4096 {
        let ticket = gate.ticket(None);
        let mut acquire = Box::pin(gate.acquire(ticket.clone()));
        poll_fn(|cx| {
            assert!(acquire.as_mut().poll(cx).is_pending());
            Poll::Ready(())
        })
        .await;
        drop(acquire);
        held.push(ticket);
    }
    drop(blocker);
    assert!(!gate.has_waiters());
    assert!(gate.try_acquire().is_some());
}

#[tokio::test]
async fn removing_the_last_reader_wakes_a_queued_attempt_without_a_free_slot() {
    use std::future::{poll_fn, Future};
    use std::task::Poll;

    let gate = PriorityGate::new(0);
    let (registration, mut attempts) = DownloadReaders::first(&gate, scene("first"), None, None);
    let mut attempt = Box::pin(attempts.acquire());
    poll_fn(|cx| {
        assert!(attempt.as_mut().poll(cx).is_pending());
        Poll::Ready(())
    })
    .await;
    drop(registration);
    assert!(!gate.has_waiters());
    poll_fn(|cx| {
        assert!(matches!(
            attempt.as_mut().poll(cx),
            Poll::Ready(Err(GateClosed))
        ));
        Poll::Ready(())
    })
    .await;
}

#[tokio::test]
async fn keys_are_read_while_a_slot_is_in_use() {
    let gate = PriorityGate::new(1);
    assert!(!gate.reads_keys());
    // nothing waits, but the release of this slot can go to a request that arrives meanwhile
    let permit = gate.acquire(gate.ticket(None)).await.unwrap();
    assert!(!gate.has_waiters());
    assert!(gate.reads_keys());
    drop(permit);
    assert!(!gate.reads_keys());

    let permit = gate.try_acquire().unwrap();
    assert!(gate.reads_keys());
    drop(permit);
    assert!(!gate.reads_keys());
}

#[tokio::test]
async fn a_departed_hash_ranks_last_for_a_retry_after_its_queue_drained() {
    let gate = PriorityGate::new(1);
    gate.register("left", key(1, false, 100.0));
    let (registration, mut attempts) = DownloadReaders::first(&gate, scene("left"), None, None);
    // the first attempt holds the slot; nothing of "left" waits when its scene goes
    let permit = attempts.acquire().await.unwrap();
    gate.unregister("left");
    drop(permit);
    let blocker = gate.try_acquire().unwrap();

    let log = Log::default();
    let retry = {
        let log = log.clone();
        tokio::spawn(async move {
            let permit = attempts.acquire().await.unwrap();
            log.lock().unwrap().push("left-retry");
            drop(permit);
        })
    };
    settle(&gate, 1).await;
    // younger than the retry, but its scene has not left
    let other = reader(&gate, "no-hash", gate.ticket(None), &log);
    settle(&gate, 2).await;
    drop(blocker);
    retry.await.unwrap();
    other.await.unwrap();
    assert_eq!(*log.lock().unwrap(), vec!["no-hash", "left-retry"]);

    // the mark lasts only while a request of the hash is alive
    assert!(gate.state().departed.contains("left"));
    drop(registration);
    assert!(gate.state().departed.is_empty());
}

#[tokio::test]
async fn a_poisoned_gate_keeps_granting_and_releasing() {
    let gate = PriorityGate::new(1);
    let poisoner = gate.clone();
    let _ = std::thread::spawn(move || {
        let _held = poisoner.state.lock().unwrap();
        panic!("poison the gate");
    })
    .join();
    assert!(gate.state.is_poisoned());

    let permit = gate.acquire(gate.ticket(None)).await.unwrap();
    let waiting = gate.ticket(scene("s"));
    drop(permit);
    let permit = gate.acquire(waiting).await.unwrap();
    drop(permit);
    assert!(gate.try_acquire().is_some());
}

#[tokio::test]
async fn a_queue_entry_without_its_ticket_is_skipped_inside_a_release() {
    let gate = PriorityGate::new(1);
    gate.register("near", key(0, false, 0.0));
    let blocker = gate.try_acquire().unwrap();
    let ticket = gate.ticket(None);
    // a stale entry that outranks the real ticket
    gate.state()
        .scenes
        .entry("near".to_owned())
        .or_default()
        .insert(u64::MAX);
    // the release runs the grant inside the permit's destructor
    drop(blocker);
    let permit = gate.acquire(ticket).await.unwrap();
    assert_eq!(gate.waiting(), 0);
    drop(permit);
}
