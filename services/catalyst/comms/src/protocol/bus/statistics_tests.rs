use super::*;
#[test]
fn producer_churn_keeps_recent_fences_and_never_refuses_a_new_incarnation() {
    let mut assembler = Assembler::default();
    let mut now = std::time::Instant::now();
    let id = |n: usize| uuid::Uuid::from_u128(n as u128);
    for producer in 0..=MAX_PRODUCERS + 1 {
        let frame = frames(&Snapshot::default(), id(producer), 1, 1024).unwrap();
        assert!(assembler.receive_update(&frame[0], now).is_some());
        now += PRODUCER_QUIET;
    }
    assert_eq!(assembler.retired.len(), MAX_PRODUCERS);
    assert_eq!(assembler.retired_order.len(), MAX_PRODUCERS);
    assert!(
        !assembler.retired.contains(&id(0)),
        "the oldest fence is forgotten at capacity"
    );
    // Past the fence capacity, a new Pulse incarnation still replaces a quiet one.
    let extra = id(100_000);
    let frame = frames(&Snapshot::default(), extra, 1, 1024).unwrap();
    assert!(assembler.receive_update(&frame[0], now).is_some());
    assert_eq!(assembler.retired.len(), MAX_PRODUCERS);
    assert!(assembler.retired.contains(&id(MAX_PRODUCERS + 1)));
    assert!(!assembler.retired.contains(&id(1)));
    // Recently retired incarnations stay fenced, even after the active one goes quiet.
    now += PRODUCER_QUIET;
    for retired in [MAX_PRODUCERS + 1, MAX_PRODUCERS, 2] {
        let stale = frames(&Snapshot::default(), id(retired), 999, 1024).unwrap();
        assert!(assembler.receive_update(&stale[0], now).is_none());
    }
    let current = frames(&snapshot(1), extra, 2, 1024).unwrap();
    assert!(assembler.receive_update(&current[0], now).is_some());
    for producer in 100_001..100_101 {
        let partial = frames(&snapshot(10), id(producer), 1, 100).unwrap();
        assert!(assembler.receive_update(&partial[0], now).is_none());
        assert!(assembler.pending.iter().flatten().count() <= 2);
    }
}

#[test]
fn transfer_byte_limit_discards_the_whole_partial_population() {
    let mut assembler = Assembler::default();
    let mut frame = vec![b' '; MAX_FRAME_BYTES];
    frame[..4].copy_from_slice(MAGIC);
    frame[4..20].copy_from_slice(uuid::Uuid::nil().as_bytes());
    frame[20..28].copy_from_slice(&1_u64.to_le_bytes());
    let chunks = MAX_TRANSFER_BYTES.div_ceil(MAX_FRAME_BYTES - HEADER_BYTES) as u64 + 1;
    frame[36..44].copy_from_slice(&chunks.to_le_bytes());
    for index in 0..chunks {
        frame[28..36].copy_from_slice(&index.to_le_bytes());
        assert!(assembler.receive(&frame).is_none());
        assert!(
            assembler
                .pending
                .iter()
                .flatten()
                .all(|t| t.bytes.len() <= MAX_TRANSFER_BYTES)
        );
    }
    assert!(assembler.pending.iter().all(Option::is_none));
    let recovery = frames(&snapshot(1), uuid::Uuid::nil(), 2, 1024).unwrap();
    assert_eq!(assembler.receive(&recovery[0]), Some(snapshot(1)));
}
#[test]
fn incomplete_transfer_expires_without_more_frames_and_overlap_keeps_active_writer() {
    let a = uuid::Uuid::new_v4();
    let b = uuid::Uuid::new_v4();
    let now = std::time::Instant::now();
    let mut assembler = Assembler::default();
    let partial = frames(&snapshot(100), a, 1, 100).unwrap();
    assert!(assembler.receive_update(&partial[0], now).is_none());
    assert!(assembler.pending.iter().any(Option::is_some));
    assembler.expire(now + TRANSFER_TIMEOUT);
    assert!(assembler.pending.iter().all(Option::is_none));
    let empty_a = frames(&Snapshot::default(), a, 2, 1024).unwrap();
    assert!(
        assembler
            .receive_update(&empty_a[0], now + TRANSFER_TIMEOUT)
            .is_some()
    );
    let empty_b = frames(&Snapshot::default(), b, 1, 1024).unwrap();
    assert!(
        assembler
            .receive_update(&empty_b[0], now + TRANSFER_TIMEOUT)
            .is_none()
    );
    assert!(
        assembler
            .receive_update(&empty_b[0], now + TRANSFER_TIMEOUT + PRODUCER_QUIET)
            .is_some()
    );
    let stale = frames(&Snapshot::default(), a, 3, 1024).unwrap();
    assert!(
        assembler
            .receive_update(&stale[0], now + TRANSFER_TIMEOUT + PRODUCER_QUIET * 3)
            .is_none()
    );
    assert_eq!(assembler.retired.len(), 1);
}

#[test]
fn advertised_unbounded_chunks_are_refused_before_allocation() {
    let mut frame = frames(&Snapshot::default(), uuid::Uuid::new_v4(), 1, 1024)
        .unwrap()
        .remove(0);
    frame[36..44].copy_from_slice(&u64::MAX.to_le_bytes());
    let mut assembler = Assembler::default();
    assert!(assembler.receive(&frame).is_none());
    assert!(assembler.pending.iter().all(Option::is_none));
}
#[test]
fn invalid_candidate_cannot_retire_a_working_statistics_producer() {
    let producer = uuid::Uuid::new_v4();
    let mut assembler = Assembler::default();
    assert!(
        assembler
            .receive(&frames(&Snapshot::default(), producer, 1, 1024).unwrap()[0])
            .is_some()
    );
    for _ in 0..5000 {
        let mut invalid =
            frames(&Snapshot::default(), uuid::Uuid::new_v4(), 1, 1024).unwrap()[0].clone();
        invalid[HEADER_BYTES] = b'!';
        assert!(assembler.receive(&invalid).is_none());
    }
    assert!(
        assembler
            .receive(&frames(&Snapshot::default(), producer, 2, 1024).unwrap()[0])
            .is_some()
    );
}
fn snapshot(count: usize) -> Snapshot {
    Snapshot {
        peers: (0..count)
            .map(|i| Peer {
                address: format!("0x{i:040x}"),
                position: [-0.01, 1.25, 16.0],
                time: 1234,
            })
            .collect(),
        islands: vec![],
        discovery: vec![],
    }
}
#[test]
fn statistics_observation_roundtrip_preserves_identity_and_float_bits() {
    let message = Observation {
        version: 2,
        incarnation: uuid::Uuid::new_v4(),
        generation: 123,
        producer: uuid::Uuid::nil(),
        sequence: 4,
        event: Event::Heartbeat {
            address: "0xAbCd".into(),
            position: [f64::INFINITY, -0.0, f64::NAN],
        },
    };
    let decoded = Observation::decode(&message.encode().unwrap()).unwrap();
    assert_eq!(decoded.producer, message.producer);
    assert_eq!(decoded.incarnation, message.incarnation);
    assert_eq!(decoded.generation, message.generation);
    let Event::Heartbeat { address, position } = decoded.event else {
        panic!()
    };
    assert_eq!(address, "0xAbCd");
    assert_eq!(
        position.map(f64::to_bits),
        [f64::INFINITY, -0.0, f64::NAN].map(f64::to_bits)
    );
    let mut bad = message;
    bad.version = 1;
    assert!(bad.encode().is_none());
    assert!(Observation::decode(&serde_json::to_vec(&bad).unwrap()).is_none());
    assert!(Observation::decode(&vec![0; MAX_FRAME_BYTES + 1]).is_none());
    let grant = ProducerGrant {
        incarnation: uuid::Uuid::new_v4(),
        producer: uuid::Uuid::new_v4(),
        generation: 1,
        challenge: uuid::Uuid::new_v4(),
    };
    for request in [
        ProducerRequest::Status,
        ProducerRequest::Begin {
            incarnation: grant.incarnation,
            producer: grant.producer,
        },
        ProducerRequest::Confirm(grant.clone()),
    ] {
        assert_eq!(
            ProducerRequest::decode(&request.encode().unwrap()),
            Some(request)
        );
    }
    for reply in [
        ProducerReply::Status {
            incarnation: grant.incarnation,
        },
        ProducerReply::Grant(grant.clone()),
        ProducerReply::Active(grant.clone()),
        ProducerReply::Rejected {
            incarnation: grant.incarnation,
        },
    ] {
        assert_eq!(ProducerReply::decode(&reply.encode().unwrap()), Some(reply));
    }
    assert!(ProducerRequest::decode(&vec![0; MAX_FRAME_BYTES + 1]).is_none());
    assert!(ProducerReply::decode(&vec![0; MAX_FRAME_BYTES + 1]).is_none());
    let frame = frames(&Snapshot::default(), grant.incarnation, 1, 1024).unwrap();
    assert_eq!(frame_producer(&frame[0]), Some(grant.incarnation));
    assert!(frame_producer(&[]).is_none());
}
#[test]
fn statistics_large_full_transfer_is_atomic_and_not_native_capacity_limited() {
    let expected = snapshot(30_000);
    let started = std::time::Instant::now();
    let frames = frames(&expected, uuid::Uuid::nil(), 1, 1024).unwrap();
    let encoded = started.elapsed();
    let bytes: usize = frames.iter().map(Vec::len).sum();
    assert!(frames.len() > 1000);
    let mut receiver = Assembler::default();
    for frame in &frames[..frames.len() - 1] {
        assert!(frame.len() <= 1024);
        assert!(receiver.receive(frame).is_none());
    }
    assert_eq!(receiver.receive(frames.last().unwrap()).unwrap(), expected);
    eprintln!(
        "statistics diagnostic: peers=30000 frame_limit=1024 frames={} wire_bytes={bytes} encode_us={} assemble_us={} max_incomplete_bytes={} max_slots=2; authority-loop impact not measured",
        frames.len(),
        encoded.as_micros(),
        started.elapsed().saturating_sub(encoded).as_micros(),
        MAX_TRANSFER_BYTES
    );
    for frame in &frames {
        assert!(receiver.receive(frame).is_none(), "duplicate transfer");
    }
}
#[test]
fn statistics_interruption_stale_frames_empty_state_and_new_epoch() {
    let epoch = uuid::Uuid::nil();
    let old = frames(&snapshot(20), epoch, 1, 256).unwrap();
    let new = frames(&Snapshot::default(), epoch, 2, 256).unwrap();
    let mut reader = Assembler::default();
    assert!(reader.receive(&old[0]).is_none());
    assert_eq!(reader.receive(&new[0]), Some(Snapshot::default()));
    for part in old {
        assert!(reader.receive(&part).is_none());
    }
    let restarted = frames(&snapshot(1), uuid::Uuid::from_u128(1), 1, 1024).unwrap();
    assert_eq!(
        reader
            .receive_update(&restarted[0], std::time::Instant::now() + PRODUCER_QUIET)
            .map(|(_, update)| update),
        Some(Update::Snapshot {
            revision: 1,
            state: snapshot(1)
        })
    );
    assert!(
        reader.receive(&new[0]).is_none(),
        "retired producer cannot replace current state"
    );
}
#[test]
fn statistics_missing_or_reordered_chunks_never_commit() {
    let parts = frames(&snapshot(30), uuid::Uuid::nil(), 1, 256).unwrap();
    let mut reader = Assembler::default();
    assert!(reader.receive(&parts[0]).is_none());
    for part in parts.iter().skip(2) {
        assert!(reader.receive(part).is_none());
    }
    let retry = frames(&snapshot(30), uuid::Uuid::nil(), 2, 256).unwrap();
    let mut result = None;
    for part in retry {
        result = reader.receive(&part);
    }
    assert_eq!(result, Some(snapshot(30)));
    assert!(frames(&snapshot(1), uuid::Uuid::nil(), 3, 1).is_none());
}

#[test]
fn statistics_snapshot_retains_nonfinite_and_signed_zero_bits() {
    let mut view = snapshot(1);
    view.peers[0].position = [f64::NAN, f64::NEG_INFINITY, -0.0];
    let parts = frames(&view, uuid::Uuid::nil(), 1, 1024).unwrap();
    let decoded = Assembler::default().receive(&parts[0]).unwrap();
    assert_eq!(
        decoded.peers[0].position.map(f64::to_bits),
        view.peers[0].position.map(f64::to_bits)
    );
}
