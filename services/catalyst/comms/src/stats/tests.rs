//! Source-derived fixtures: decentraland/archipelago-workers at
//! 64dc515987175c679e4009fe557ea60fdf50bc6c (Stats handlers and locked HTTP middleware).
//! These assertions do not substitute for deployed-host differential captures.
use super::*;
use crate::gatekeeper::bus::InProcessBus;
use crate::protocol::bus::statistics::{Peer, SNAPSHOTS_SUBJECT, frames};

#[test]
fn indexed_changes_preserve_order_and_rebuild_after_removal_and_recovery() {
    use crate::protocol::bus::statistics::Frames;
    let bus = InProcessBus::new();
    let reader = Arc::new(Reader::default());
    let _subscription = reader.subscribe(&bus);
    let producer = uuid::Uuid::new_v4();
    let peer = |address: &str, time| Peer {
        address: address.into(),
        position: [0.; 3],
        time,
    };
    let send = |sequence, update| {
        for frame in Frames::new(&update, producer, sequence, 1024).unwrap() {
            bus.inject(SNAPSHOTS_SUBJECT, &frame);
        }
    };
    send(
        1,
        Update::Snapshot {
            revision: 0,
            state: Snapshot {
                peers: vec![peer("A", 1), peer("b", 2), peer("c", 3)],
                ..Default::default()
            },
        },
    );
    let retained = reader.snapshot();
    for (revision, change) in [
        (1, Change::Peer(peer("b", 4))),
        (2, Change::Remove("A".into())),
        (3, Change::Peer(peer("c", 5))),
        (4, Change::Remove("b".into())),
        (5, Change::Peer(peer("A", 6))),
    ] {
        send(revision + 1, Update::Change { revision, change });
    }
    assert_eq!(reader.snapshot().peers, vec![peer("c", 5), peer("A", 6)]);
    assert_eq!(
        retained.peers,
        vec![peer("A", 1), peer("b", 2), peer("c", 3)]
    );
    send(
        7,
        Update::Snapshot {
            revision: 5,
            state: Snapshot {
                peers: vec![peer("Z", 7), peer("c", 8)],
                ..Default::default()
            },
        },
    );
    send(
        8,
        Update::Change {
            revision: 6,
            change: Change::Peer(peer("c", 9)),
        },
    );
    assert_eq!(reader.snapshot().peers, vec![peer("Z", 7), peer("c", 9)]);
    send(
        9,
        Update::Snapshot {
            revision: 6,
            state: Snapshot {
                peers: vec![peer("c", 10), peer("c", 11), peer("A", 12)],
                ..Default::default()
            },
        },
    );
    send(
        10,
        Update::Change {
            revision: 7,
            change: Change::Peer(peer("c", 13)),
        },
    );
    assert_eq!(
        reader.snapshot().peers,
        vec![peer("c", 13), peer("c", 11), peer("A", 12)]
    );
    send(
        11,
        Update::Change {
            revision: 8,
            change: Change::Remove("c".into()),
        },
    );
    send(
        12,
        Update::Change {
            revision: 9,
            change: Change::Peer(peer("A", 14)),
        },
    );
    assert_eq!(reader.snapshot().peers, vec![peer("A", 14)]);
    send(
        13,
        Update::Snapshot {
            revision: 9,
            state: Snapshot {
                peers: vec![
                    peer("x", 15),
                    peer("d", 16),
                    peer("x", 17),
                    peer("b", 18),
                    peer("d", 19),
                    peer("z", 20),
                ],
                ..Default::default()
            },
        },
    );
    for (revision, change) in [
        (10, Change::Remove("x".into())),
        (11, Change::Peer(peer("d", 21))),
        (12, Change::Peer(peer("b", 22))),
        (13, Change::Remove("missing".into())),
        (14, Change::Peer(peer("d", 23))),
        (15, Change::Remove("z".into())),
        (16, Change::Peer(peer("new", 24))),
    ] {
        send(revision + 4, Update::Change { revision, change });
    }
    assert_eq!(
        reader.snapshot().peers,
        vec![peer("d", 23), peer("b", 22), peer("d", 19), peer("new", 24)]
    );
}

#[test]
fn every_contiguous_change_is_visible_at_callback_completion_and_gaps_require_recovery() {
    use crate::protocol::bus::statistics::{Change, Frames, Update};
    let bus = InProcessBus::new();
    let reader = Arc::new(Reader::default());
    let _subscription = reader.subscribe(&bus);
    let epoch = uuid::Uuid::new_v4();
    let send = |sequence, update| {
        for frame in Frames::new(&update, epoch, sequence, 100).unwrap() {
            bus.inject(SNAPSHOTS_SUBJECT, &frame);
        }
    };
    send(
        1,
        Update::Snapshot {
            revision: 0,
            state: Snapshot::default(),
        },
    );
    let peer = Peer {
        address: "AbC".into(),
        position: [-0., f64::INFINITY, 16.],
        time: -123,
    };
    send(
        2,
        Update::Change {
            revision: 1,
            change: Change::Peer(peer.clone()),
        },
    );
    let held = reader.snapshot();
    assert_eq!(held.peers[0], peer);
    send(
        3,
        Update::Change {
            revision: 2,
            change: Change::Remove("AbC".into()),
        },
    );
    assert!(reader.snapshot().peers.is_empty());
    assert_eq!(
        held.peers.len(),
        1,
        "retained readers keep one coherent version"
    );
    send(
        4,
        Update::Change {
            revision: 3,
            change: Change::Peer(peer.clone()),
        },
    );
    send(
        6,
        Update::Change {
            revision: 5,
            change: Change::Remove("AbC".into()),
        },
    );
    assert_eq!(
        reader.snapshot().peers.len(),
        1,
        "gap retains complete state"
    );
    send(
        7,
        Update::Change {
            revision: 4,
            change: Change::Remove("AbC".into()),
        },
    );
    assert_eq!(
        reader.snapshot().peers.len(),
        1,
        "late update cannot repair an unknown gap"
    );
    send(
        8,
        Update::Snapshot {
            revision: 5,
            state: Snapshot::default(),
        },
    );
    assert!(reader.snapshot().peers.is_empty());
    send(
        9,
        Update::Change {
            revision: 6,
            change: Change::Peer(peer),
        },
    );
    assert_eq!(reader.snapshot().peers.len(), 1);
}

#[test]
fn reader_exposes_only_complete_views_and_retains_them_across_outage() {
    let bus = InProcessBus::new();
    let reader = Arc::new(Reader::default());
    assert!(reader.snapshot().peers.is_empty());
    let _sub = reader.subscribe(&bus);
    let snapshot = Snapshot {
        peers: vec![Peer {
            address: "AbC".into(),
            position: [-0.1, 2., 32.],
            time: 123,
        }],
        ..Default::default()
    };
    let epoch = uuid::Uuid::new_v4();
    let packets = frames(&snapshot, epoch, 1, 70).unwrap();
    for packet in &packets[..packets.len() - 1] {
        bus.inject(SNAPSHOTS_SUBJECT, packet);
    }
    assert!(reader.snapshot().peers.is_empty());
    bus.inject(SNAPSHOTS_SUBJECT, packets.last().unwrap());
    assert_eq!(*reader.snapshot(), snapshot);
    bus.set_connected(false);
    assert_eq!(*reader.snapshot(), snapshot);
    bus.set_connected(true);
    for packet in frames(&Snapshot::default(), epoch, 2, 70).unwrap() {
        bus.inject(SNAPSHOTS_SUBJECT, &packet);
    }
    assert!(reader.snapshot().peers.is_empty());
    drop(_sub);
    for packet in frames(&snapshot, epoch, 3, 70).unwrap() {
        bus.inject(SNAPSHOTS_SUBJECT, &packet);
    }
    assert!(reader.snapshot().peers.is_empty());
}

#[path = "render_tests.rs"]
mod render_tests;

#[path = "content_tests.rs"]
mod content_tests;

#[path = "http_tests.rs"]
mod http_tests;
#[test]
fn reader_retains_last_valid_view_on_corrupt_topology_and_interrupted_replacement() {
    let bus = InProcessBus::new();
    let reader = Arc::new(Reader::default());
    let _sub = reader.subscribe(&bus);
    let epoch = uuid::Uuid::new_v4();
    let first = Snapshot {
        peers: vec![Peer {
            address: "retained".into(),
            position: [16777217., 0., -68719476737.],
            time: 7,
        }],
        ..Default::default()
    };
    for bytes in frames(&first, epoch, 1, 100).unwrap() {
        bus.inject(SNAPSHOTS_SUBJECT, &bytes);
    }
    let corrupt = Snapshot {
        islands: vec![255],
        ..Default::default()
    };
    for bytes in frames(&corrupt, epoch, 2, 100).unwrap() {
        bus.inject(SNAPSHOTS_SUBJECT, &bytes);
    }
    assert_eq!(*reader.snapshot(), first);
    let replacement = frames(&Snapshot::default(), epoch, 3, 50).unwrap();
    bus.inject(SNAPSHOTS_SUBJECT, &replacement[0]);
    assert_eq!(*reader.snapshot(), first);
    assert!(render::peers(&reader.snapshot(), &[]).contains(r#""parcel":[1048576,-4294967297]"#));
    for bytes in frames(&Snapshot::default(), epoch, 4, 100).unwrap() {
        bus.inject(SNAPSHOTS_SUBJECT, &bytes);
    }
    assert!(reader.snapshot().peers.is_empty());
}
