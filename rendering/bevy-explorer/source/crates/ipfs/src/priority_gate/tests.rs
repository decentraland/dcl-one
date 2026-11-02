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

fn priorities(entries: &[(&str, LoadPriority)]) -> HashMap<String, LoadPriority> {
    entries
        .iter()
        .map(|(name, key)| ((*name).to_owned(), *key))
        .collect()
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
    gate.set_priorities(priorities(&[
        ("far", key(5, false, 6400.0)),
        ("near", key(1, false, 100.0)),
        ("near_behind", key(1, true, 100.0)),
    ]));
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
    gate.set_priorities(priorities(&[
        ("a", key(5, false, 6400.0)),
        ("b", key(1, false, 100.0)),
    ]));
    let holder = gate.try_acquire().unwrap();
    let log = Log::default();
    let tasks = ["a", "b"].map(|name| reader(&gate, name, gate.ticket(scene(name)), &log));
    settle(&gate, 2).await;

    // the player turned round: a is now the near one
    gate.set_priorities(priorities(&[
        ("a", key(1, false, 100.0)),
        ("b", key(5, false, 6400.0)),
    ]));
    drop(holder);
    for task in tasks {
        task.await.unwrap();
    }

    assert_eq!(*log.lock().unwrap(), vec!["a", "b"]);
}

#[tokio::test]
async fn unknown_scene_is_top_priority() {
    let gate = PriorityGate::new(1);
    gate.set_priorities(priorities(&[("known", key(1, false, 100.0))]));
    let holder = gate.try_acquire().unwrap();
    let log = Log::default();
    let tasks =
        ["known", "unknown"].map(|name| reader(&gate, name, gate.ticket(scene(name)), &log));
    settle(&gate, 2).await;

    drop(holder);
    for task in tasks {
        task.await.unwrap();
    }

    assert_eq!(*log.lock().unwrap(), vec!["unknown", "known"]);
}

#[tokio::test]
async fn requests_without_a_scene_rank_after_live_scenes_and_before_departed_ones() {
    let gate = PriorityGate::new(1);
    gate.set_priorities(priorities(&[
        ("left", key(1, false, 100.0)),
        ("live", key(5, true, 6400.0)),
    ]));
    let holder = gate.try_acquire().unwrap();
    let log = Log::default();
    let tasks = [
        reader(&gate, "left", gate.ticket(scene("left")), &log),
        reader(&gate, "imposter", gate.ticket(None), &log),
        reader(&gate, "live", gate.ticket(scene("live")), &log),
    ];
    settle(&gate, 3).await;

    // "left" has despawned: it is no longer published
    gate.set_priorities(priorities(&[("live", key(5, true, 6400.0))]));
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
    gate.set_priorities(priorities(&[("s", key(1, false, 100.0))]));
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
    gate.set_priorities(priorities(&[
        ("far", key(5, false, 6400.0)),
        ("near", key(1, false, 100.0)),
    ]));
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
    gate.set_priorities(priorities(&[
        ("far", key(5, false, 6400.0)),
        ("near", key(1, false, 100.0)),
    ]));
    let holder = gate.try_acquire().unwrap();
    let log = Log::default();
    let far = reader(&gate, "far", gate.ticket(scene("far")), &log);
    // a read that queued a ticket and then went away without ever awaiting it
    drop(gate.ticket(scene("near")));
    settle(&gate, 2).await;

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
    gate.set_priorities(priorities(&[
        ("far", key(5, false, 6400.0)),
        ("mid", key(3, false, 2500.0)),
        ("near", key(1, false, 100.0)),
    ]));
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
    gate.set_priorities(priorities(&[
        ("left", key(1, false, 100.0)),
        ("ahead", key(2, false, 1000.0)),
    ]));
    let holder = gate.try_acquire().unwrap();
    let log = Log::default();
    let tasks = ["left", "ahead"].map(|name| reader(&gate, name, gate.ticket(scene(name)), &log));
    settle(&gate, 2).await;

    // the player walked out of "left": it is no longer a live scene
    gate.set_priorities(priorities(&[("ahead", key(2, false, 1000.0))]));
    drop(holder);
    for task in tasks {
        task.await.unwrap();
    }

    assert_eq!(*log.lock().unwrap(), vec!["ahead", "left"]);
}

#[tokio::test]
async fn an_unknown_scene_stays_on_top_after_a_refresh() {
    let gate = PriorityGate::new(1);
    gate.set_priorities(priorities(&[("ahead", key(2, false, 1000.0))]));
    let holder = gate.try_acquire().unwrap();
    let log = Log::default();
    let tasks =
        ["ahead", "unknown"].map(|name| reader(&gate, name, gate.ticket(scene(name)), &log));
    settle(&gate, 2).await;

    // a refresh that never knew "unknown" leaves it on top
    gate.set_priorities(priorities(&[("ahead", key(2, false, 1000.0))]));
    drop(holder);
    for task in tasks {
        task.await.unwrap();
    }

    assert_eq!(*log.lock().unwrap(), vec!["unknown", "ahead"]);
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
    gate.set_priorities(keys.clone());
    let mut tickets = Vec::new();
    for i in 0..HASHES {
        for _ in 0..REQUESTS_PER_HASH {
            tickets.push(gate.ticket(scene(&format!("hash-{i}"))));
        }
    }
    assert_eq!(gate.waiting(), HASHES * REQUESTS_PER_HASH);

    // a frame's publish: the whole key map replaced
    let start = std::time::Instant::now();
    for _ in 0..100 {
        gate.set_priorities(keys.clone());
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
        "gate: {HASHES} queues, {} requests: publish {:?} per frame (map clone included), grant {:?} each (task wake included)",
        HASHES * REQUESTS_PER_HASH,
        publish,
        per_grant
    );
}
