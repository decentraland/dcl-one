//! Test-only load fixture: real AppState -> feed outbox -> BrokerBus -> loopback NATS.
use super::*;
use crate::archipelago::{Settings, feed::ControlPositionAnnouncement, test_support::state_on};
use crate::protocol::bus::control_position::{ControlPosition, SUBJECT};
use std::sync::atomic::AtomicUsize;

struct TimedBus {
    inner: Arc<BrokerBus>,
    handoffs: Mutex<Vec<u64>>,
    capacity: usize,
    outcomes: [AtomicUsize; 4],
}
#[async_trait::async_trait]
impl NatsBus for TimedBus {
    fn is_enabled(&self) -> bool {
        self.inner.is_enabled()
    }
    fn is_connected(&self) -> bool {
        self.inner.is_connected()
    }
    fn is_ready(&self) -> bool {
        self.inner.is_ready()
    }
    fn connect(&self) {
        self.inner.connect();
    }
    fn subscribe(
        &self,
        subject: &str,
        queue: Option<&str>,
        handler: NatsHandler,
    ) -> NatsSubscription {
        self.inner.subscribe(subject, queue, handler)
    }
    async fn publish(&self, subject: &str, payload: Vec<u8>) -> PublishOutcome {
        let started = Instant::now();
        let outcome = self.inner.publish(subject, payload).await;
        {
            let mut values = self.handoffs.lock().unwrap();
            if values.len() < self.capacity {
                values.push(0);
                let last = values.len() - 1;
                values[last] = started.elapsed().as_micros() as u64;
            }
        }
        // Completion is published only after its sample is visible. Warmup/final barriers
        // acquire this counter, so clearing/snapshotting cannot race a trailing sample append.
        self.outcomes[match outcome {
            PublishOutcome::Submitted => 0,
            PublishOutcome::NoConnection => 1,
            PublishOutcome::Failed => 2,
            PublishOutcome::Unconfirmed => 3,
        }]
        .fetch_add(1, Ordering::Release);
        outcome
    }
    async fn request(&self, subject: &str, payload: Vec<u8>) -> Result<Vec<u8>, RequestError> {
        self.inner.request(subject, payload).await
    }
}

fn percentile(values: &mut [u64], percentile: usize) -> u64 {
    if values.is_empty() {
        return 0;
    }
    values.sort_unstable();
    values[(values.len() - 1) * percentile / 100]
}

fn unix_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
}

fn phase(profile: &str, name: &str) -> u64 {
    let timestamp = unix_ms();
    println!(
        "BUS_PHASE {}",
        serde_json::json!({
            "schema_version":1,"profile":profile,"phase":name,"unix_ms":timestamp
        })
    );
    use std::io::Write;
    std::io::stdout().flush().unwrap();
    timestamp
}

#[tokio::test(flavor = "multi_thread", worker_threads = 6)]
#[ignore = "requires isolated BUS_PUBLISH_TEST_URL and explicit load parameters"]
async fn production_feed_position_load() {
    let url = std::env::var("BUS_PUBLISH_TEST_URL").expect("owned broker URL");
    let rate: usize = std::env::var("BUS_POSITION_RATE")
        .unwrap_or("10000".into())
        .parse()
        .unwrap();
    let seconds: usize = std::env::var("BUS_LOAD_SECONDS")
        .unwrap_or("60".into())
        .parse()
        .unwrap();
    let lookup_rate: usize = std::env::var("BUS_LOOKUP_RATE")
        .unwrap_or("5000".into())
        .parse()
        .unwrap();
    assert!(
        seconds > 0
            && seconds <= 120
            && rate > 0
            && rate <= 40000
            && rate % 1000 == 0
            && lookup_rate <= 5000
            && lookup_rate % 1000 == 0
    );
    let observer = async_nats::connect(url.clone()).await.unwrap();
    let mut positions = observer.subscribe(SUBJECT).await.unwrap();
    let mut inboxes = observer.subscribe("_INBOX.>").await.unwrap();
    let mut lookups = observer.subscribe("test.profile.lookup").await.unwrap();
    observer.flush().await.unwrap();
    let responder = observer.clone();
    let responder = tokio::spawn(async move {
        while let Some(message) = lookups.next().await {
            if let Some(reply) = message.reply {
                responder.publish(reply, "ok".into()).await.unwrap();
            }
        }
    });
    let bus = Arc::new(BrokerBus::new(Some(url), "position-profile"));
    let jobs = crate::jobs::Jobs::new();
    let timed = Arc::new(TimedBus {
        inner: bus.clone(),
        handoffs: Mutex::new(Vec::with_capacity(rate * seconds.max(5))),
        capacity: rate * seconds.max(5),
        outcomes: std::array::from_fn(|_| AtomicUsize::new(0)),
    });
    let state = state_on(Settings::for_tests(), timed.clone(), &jobs).await;
    timeout(Duration::from_secs(12), async {
        while !bus.is_ready() {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();
    let audience = "a".repeat(256);
    let realm = "r".repeat(256);
    let mut owners = Vec::with_capacity(10000);
    // Each player has its own exact current session/realm ownership permit.
    for player in 1..=10000 {
        let wallet = format!("0x{player:040x}");
        let session = format!("0x{:040x}", player + 10000);
        let (link, events) = state.registry.on_v4_peer_connected_with_realm_epoch(
            &wallet,
            &session,
            &format!("connection-{player}"),
            true,
            1,
        );
        let permit = state
            .registry
            .control_position_permit(&wallet, &session, 1, false)
            .unwrap();
        owners.push((wallet, session, permit, link, events));
    }
    while timeout(Duration::from_millis(50), inboxes.next())
        .await
        .is_ok()
    {}
    let fences = Arc::new(AtomicUsize::new(0));
    let fence_counter = fences.clone();
    let fence_task = tokio::spawn(async move {
        while let Some(message) = inboxes.next().await {
            // Readiness fences carry an eight-byte sequence; lookup replies are two bytes.
            if message.payload.len() == 8 {
                fence_counter.fetch_add(1, Ordering::Relaxed);
            }
        }
    });
    let received = Arc::new(AtomicUsize::new(0));
    let received_counter = received.clone();
    let warmup_received = Arc::new(AtomicUsize::new(0));
    let warmup_counter = warmup_received.clone();
    let origin = Instant::now();
    // Fixed-size metadata, published before the frame enters the outbox. Sequence IDs index
    // these arrays; no payload padding, wire fields or production policy change is needed.
    let created_us = Arc::new(
        (0..rate * seconds)
            .map(|_| AtomicU64::new(0))
            .collect::<Vec<_>>(),
    );
    let scheduled_us = Arc::new(
        (0..rate * seconds)
            .map(|_| AtomicU64::new(0))
            .collect::<Vec<_>>(),
    );
    let created = created_us.clone();
    let scheduled = scheduled_us.clone();
    let position_latencies = Arc::new(Mutex::new(Vec::<u64>::with_capacity(rate * seconds)));
    let position_scheduled_latencies =
        Arc::new(Mutex::new(Vec::<u64>::with_capacity(rate * seconds)));
    let latencies = position_latencies.clone();
    let scheduled_latencies = position_scheduled_latencies.clone();
    let receiver = tokio::spawn(async move {
        while let Some(message) = positions.next().await {
            let record =
                ControlPosition::decode(&message.payload).expect("valid production control record");
            if record.sequence == 1 {
                warmup_counter.fetch_add(1, Ordering::Relaxed);
                continue;
            }
            let index = (record.sequence - 2) as usize;
            assert!(
                index < rate * seconds,
                "measurement sequence belongs to this finite run"
            );
            let now = origin.elapsed().as_micros() as u64;
            let latency = now.saturating_sub(created[index].load(Ordering::Acquire));
            let scheduled_latency = now.saturating_sub(scheduled[index].load(Ordering::Acquire));
            let mut values = latencies.lock().unwrap();
            if values.len() < rate * seconds {
                values.push(latency);
                scheduled_latencies.lock().unwrap().push(scheduled_latency);
            }
            drop(values);
            received_counter.fetch_add(1, Ordering::Release);
        }
    });
    // Warm the real feed for five seconds. Sequence 1 marks samples excluded by the receiver.
    let warmup_started = Instant::now();
    let mut warmup_accepted = 0;
    for tick in 0..5000 {
        tokio::time::sleep_until(warmup_started + Duration::from_millis(tick)).await;
        for offset in 0..rate / 1000 {
            let (wallet, session, permit, _, _) =
                &owners[(tick as usize * (rate / 1000) + offset) % owners.len()];
            let record = ControlPosition {
                audience: audience.clone(),
                wallet: wallet.clone(),
                session: session.clone(),
                epoch: 1,
                sequence: 1,
                realm: realm.clone(),
                position: Some([1.0, 2.0, 3.0]),
            };
            warmup_accepted += usize::from(state.outbox.publish_control_position(
                ControlPositionAnnouncement::new(record, permit.clone()).unwrap(),
            ));
        }
    }
    timeout(Duration::from_secs(3), async {
        loop {
            let completed: usize = timed
                .outcomes
                .iter()
                .map(|value| value.load(Ordering::Acquire))
                .sum();
            let submitted = timed.outcomes[0].load(Ordering::Relaxed);
            if state.outbox.queued() == 0
                && completed == warmup_accepted
                && warmup_received.load(Ordering::Relaxed) == submitted
            {
                break;
            }
            tokio::time::sleep(Duration::from_millis(1)).await;
        }
    })
    .await
    .expect("warmup must settle through local completion and actual receipt");
    let warmup_outcomes: Vec<usize> = timed
        .outcomes
        .iter()
        .map(|value| value.load(Ordering::Acquire))
        .collect();
    let warmup_receipts = warmup_received.load(Ordering::Acquire);
    let warmup_handoffs = timed.handoffs.lock().unwrap().len();
    assert!(
        warmup_accepted > 0 && warmup_receipts > 0,
        "warmup requires real admitted and delivered traffic"
    );
    assert_eq!(
        warmup_outcomes[0], warmup_accepted,
        "warmup must have healthy local SDK outcomes"
    );
    assert_eq!(&warmup_outcomes[1..], &[0, 0, 0]);
    assert_eq!(warmup_receipts, warmup_outcomes[0]);
    assert_eq!(warmup_handoffs, warmup_accepted);
    let warmup_evidence = serde_json::json!({"offered":rate*5,"admitted":warmup_accepted,"submitted":warmup_outcomes[0],"no_connection":warmup_outcomes[1],"failed":warmup_outcomes[2],"unconfirmed":warmup_outcomes[3],"received":warmup_receipts,"handoff_samples":warmup_handoffs,"receipt_sample_count":warmup_receipts,"handoff_residual":0,"receipt_residual":0});
    timed.handoffs.lock().unwrap().clear();
    for outcome in &timed.outcomes {
        outcome.store(0, Ordering::Relaxed);
    }
    let dropped_before = state.outbox.dropped();
    fences.store(0, Ordering::Relaxed);
    let run_started = Instant::now();
    let steady_start_unix_ms = phase("feed", "steady_start");
    let lookup_latencies = Arc::new(Mutex::new(Vec::<u64>::with_capacity(lookup_rate * seconds)));
    let lookup_errors = Arc::new(AtomicUsize::new(0));
    let lookup_overload = Arc::new(AtomicUsize::new(0));
    let lookup_timeouts = Arc::new(AtomicUsize::new(0));
    let lookup_scheduled_latencies =
        Arc::new(Mutex::new(Vec::<u64>::with_capacity(lookup_rate * seconds)));
    let lookup_driver_lags = Arc::new(Mutex::new(Vec::<u64>::with_capacity(seconds * 1000)));
    let lookup_offered_bins = Arc::new(Mutex::new(vec![0_usize; seconds]));
    let lookup_overflow_offers = Arc::new(AtomicUsize::new(0));
    let lookup_bus = bus.clone();
    let lookup_values = lookup_latencies.clone();
    let errors = lookup_errors.clone();
    let overload = lookup_overload.clone();
    let timeouts = lookup_timeouts.clone();
    let scheduled_lookup_values = lookup_scheduled_latencies.clone();
    let lookup_lags = lookup_driver_lags.clone();
    let lookup_bins = lookup_offered_bins.clone();
    let lookup_overflow = lookup_overflow_offers.clone();
    let lookup_task = tokio::spawn(async move {
        let mut pending = tokio::task::JoinSet::new();
        for tick in 0..seconds * 1000 {
            tokio::time::sleep_until(run_started + Duration::from_millis(tick as u64)).await;
            let planned = run_started + Duration::from_millis(tick as u64);
            lookup_lags.lock().unwrap().push(
                Instant::now()
                    .saturating_duration_since(planned)
                    .as_micros() as u64,
            );
            while pending.try_join_next().is_some() {}
            for _ in 0..lookup_rate / 1000 {
                let bin = run_started.elapsed().as_secs() as usize;
                if bin < seconds {
                    lookup_bins.lock().unwrap()[bin] += 1;
                } else {
                    lookup_overflow.fetch_add(1, Ordering::Relaxed);
                }
                if pending.len() >= MAX_PENDING_REQUESTS {
                    overload.fetch_add(1, Ordering::Relaxed);
                    continue;
                }
                let bus = lookup_bus.clone();
                let values = lookup_values.clone();
                let errors = errors.clone();
                let timeouts = timeouts.clone();
                let scheduled_values = scheduled_lookup_values.clone();
                pending.spawn(async move {
                    let started = Instant::now();
                    let result = bus.request("test.profile.lookup", vec![1]).await;
                    if result == Err(RequestError::TimedOut) {
                        timeouts.fetch_add(1, Ordering::Relaxed);
                    }
                    if result.as_deref() != Ok(b"ok") {
                        errors.fetch_add(1, Ordering::Relaxed);
                    }
                    values
                        .lock()
                        .unwrap()
                        .push(started.elapsed().as_micros() as u64);
                    scheduled_values.lock().unwrap().push(
                        Instant::now()
                            .saturating_duration_since(planned)
                            .as_micros() as u64,
                    );
                });
            }
        }
        while pending.join_next().await.is_some() {}
    });
    let mut accepted = 0;
    let mut maximum_queue = 0;
    let mut payload_bytes = 0;
    let mut driver_lags = Vec::with_capacity(seconds * 1000);
    let mut offered_bins = vec![0_usize; seconds];
    let mut offered_overflow = 0_usize;
    let scheduled_origin_us = run_started.saturating_duration_since(origin).as_micros() as u64;
    for tick in 0..seconds * 1000 {
        tokio::time::sleep_until(run_started + Duration::from_millis(tick as u64)).await;
        driver_lags.push(
            Instant::now()
                .saturating_duration_since(run_started + Duration::from_millis(tick as u64))
                .as_micros() as u64,
        );
        for offset in 0..rate / 1000 {
            let player = (tick * (rate / 1000) + offset) % owners.len();
            let (wallet, session, permit, _, _) = &owners[player];
            let index = tick * (rate / 1000) + offset;
            created_us[index].store(origin.elapsed().as_micros() as u64, Ordering::Release);
            scheduled_us[index].store(scheduled_origin_us + tick as u64 * 1000, Ordering::Release);
            let bin = run_started.elapsed().as_secs() as usize;
            if bin < seconds {
                offered_bins[bin] += 1;
            } else {
                offered_overflow += 1;
            }
            let record = ControlPosition {
                audience: audience.clone(),
                wallet: wallet.clone(),
                session: session.clone(),
                epoch: 1,
                sequence: index as u64 + 2,
                realm: realm.clone(),
                position: Some([1.0, 2.0, 3.0]),
            };
            payload_bytes = record.encode().unwrap().len();
            let announcement = ControlPositionAnnouncement::new(record, permit.clone()).unwrap();
            accepted += usize::from(state.outbox.publish_control_position(announcement));
        }
        maximum_queue = maximum_queue.max(state.outbox.queued());
    }
    tokio::time::sleep_until(run_started + Duration::from_secs(seconds as u64)).await;
    let offering_elapsed = run_started.elapsed();
    let steady_end_unix_ms = phase("feed", "steady_end");
    let drain_started = Instant::now();
    lookup_task.await.unwrap();
    let drain = timeout(Duration::from_secs(3), async {
        loop {
            let completed: usize = timed
                .outcomes
                .iter()
                .map(|value| value.load(Ordering::Acquire))
                .sum();
            let submitted = timed.outcomes[0].load(Ordering::Relaxed);
            if state.outbox.queued() == 0
                && completed == accepted
                && received.load(Ordering::Acquire) == submitted
            {
                break;
            }
            tokio::time::sleep(Duration::from_millis(1)).await;
        }
    })
    .await
    .is_ok();
    let drain_elapsed_seconds = drain_started.elapsed().as_secs_f64();
    let mut positions = position_latencies.lock().unwrap();
    let mut lookups = lookup_latencies.lock().unwrap();
    let mut handoffs = timed.handoffs.lock().unwrap();
    let mut scheduled_positions = position_scheduled_latencies.lock().unwrap();
    let mut scheduled_lookups = lookup_scheduled_latencies.lock().unwrap();
    let lookup_lags = lookup_driver_lags.lock().unwrap();
    let driver_late_ticks = driver_lags.iter().filter(|value| **value > 1000).count();
    let max_lateness_us = driver_lags.iter().copied().max().unwrap_or(0);
    let mut profile = serde_json::json!({
            "schema_version":1,"steady_start_unix_ms":steady_start_unix_ms,"steady_end_unix_ms":steady_end_unix_ms,"warmup_seconds":5,"warmup":warmup_evidence,
            "offered_rate": rate, "players": 10000, "seconds": seconds,
            "offered": rate * seconds, "outbox_accepted": accepted,
            "sdk_outcomes": {"submitted":timed.outcomes[0].load(Ordering::Relaxed),"no_connection":timed.outcomes[1].load(Ordering::Relaxed),"failed":timed.outcomes[2].load(Ordering::Relaxed),"unconfirmed":timed.outcomes[3].load(Ordering::Relaxed)},
            "received": received.load(Ordering::Relaxed), "offering_elapsed_seconds": offering_elapsed.as_secs_f64(),
            "actual_offered_rate":(rate*seconds) as f64/offering_elapsed.as_secs_f64(),
            "actual_accepted_rate":accepted as f64/offering_elapsed.as_secs_f64(),
            "actual_received_rate":received.load(Ordering::Relaxed) as f64/offering_elapsed.as_secs_f64(),
            "offered_per_second":offered_bins,"offered_after_full_seconds":offered_overflow,"driver_late_ticks":driver_late_ticks,"max_lateness_us":max_lateness_us,
            "driver_lateness_us":[percentile(&mut driver_lags,50),percentile(&mut driver_lags,95),percentile(&mut driver_lags,99)],
            "pacing":"absolute 1ms batches; catch-up bursts and scheduled latency are explicitly measured",
    });
    let outcomes = serde_json::json!({
            "payload_bytes": payload_bytes, "feed_dropped": state.outbox.dropped() - dropped_before,
            "handoff_samples": handoffs.len(), "handoff_us": [percentile(&mut handoffs, 50), percentile(&mut handoffs, 95), percentile(&mut handoffs, 99)],
            "handoff_measurement": "test-only delegator, clock and bounded recording overhead; first N completions",
            "maximum_outbox": maximum_queue, "outbox_capacity": crate::archipelago::feed::OUTBOUND_CAPACITY,
            "drained": drain, "receipt_residual":timed.outcomes[0].load(Ordering::Relaxed).saturating_sub(received.load(Ordering::Relaxed)),
            "drain_elapsed_seconds":drain_elapsed_seconds,"latency_sample_count":positions.len(),"scheduled_latency_sample_count":scheduled_positions.len(),
            "handoff_residual":accepted.saturating_sub(timed.outcomes.iter().map(|value|value.load(Ordering::Relaxed)).sum()),
            "fence_messages": fences.load(Ordering::Relaxed),
            "position_end_to_end_us": [percentile(&mut positions, 50), percentile(&mut positions, 95), percentile(&mut positions, 99)],
            "position_scheduled_to_receipt_us":[percentile(&mut scheduled_positions,50),percentile(&mut scheduled_positions,95),percentile(&mut scheduled_positions,99)],
            "latency_origin":"position_end_to_end_us begins at actual frame creation; scheduled_to_receipt includes generator lag",
            "lookup_offered": lookup_rate * seconds, "lookup_completed": lookups.len(),"lookup_latency_sample_count":lookups.len(),"lookup_scheduled_latency_sample_count":scheduled_lookups.len(),
            "lookup_errors": lookup_errors.load(Ordering::Relaxed), "lookup_timeouts": lookup_timeouts.load(Ordering::Relaxed), "lookup_driver_overload": lookup_overload.load(Ordering::Relaxed),
            "lookup_us": [percentile(&mut lookups, 50), percentile(&mut lookups, 95), percentile(&mut lookups, 99)]
            ,"lookup_scheduled_to_completion_us":[percentile(&mut scheduled_lookups,50),percentile(&mut scheduled_lookups,95),percentile(&mut scheduled_lookups,99)],
            "lookup_offered_per_second":*lookup_offered_bins.lock().unwrap(),
            "lookup_offered_after_full_seconds":lookup_overflow_offers.load(Ordering::Relaxed),
            "lookup_driver_late_ticks":lookup_lags.iter().filter(|value|**value>1000).count(),
            "lookup_max_lateness_us":lookup_lags.iter().copied().max().unwrap_or(0)
    });
    profile
        .as_object_mut()
        .unwrap()
        .extend(outcomes.as_object().unwrap().clone());
    println!("BUS_PROFILE {profile}");
    drop(positions);
    drop(lookups);
    drop(handoffs);
    drop(scheduled_positions);
    drop(scheduled_lookups);
    drop(lookup_lags);
    receiver.abort();
    responder.abort();
    fence_task.abort();
    jobs.shutdown(Duration::from_secs(3)).await;
}

/// Complements the CLI saturation experiment with explicit 40,000/s pacing; this is not nats bench.
#[tokio::test(flavor = "multi_thread", worker_threads = 6)]
#[ignore = "explicit paced Core NATS measurement against an owned loopback broker"]
async fn paced_core_nats_40000() {
    let url = std::env::var("BUS_PUBLISH_TEST_URL").expect("owned broker URL");
    let seconds: usize = std::env::var("BUS_LOAD_SECONDS")
        .unwrap_or("60".into())
        .parse()
        .unwrap();
    assert!(seconds > 0 && seconds <= 120);
    let client = async_nats::connect(url).await.unwrap();
    let mut subscriber = client.subscribe("test.paced.40000").await.unwrap();
    client.flush().await.unwrap();
    let received = Arc::new(AtomicUsize::new(0));
    let counter = received.clone();
    let warmup_received = Arc::new(AtomicUsize::new(0));
    let warmup_counter = warmup_received.clone();
    let measurements = Arc::new(Mutex::new(Vec::<u64>::with_capacity(seconds * 40000)));
    let values = measurements.clone();
    let scheduled_measurements = Arc::new(Mutex::new(Vec::<u64>::with_capacity(seconds * 40000)));
    let scheduled_values = scheduled_measurements.clone();
    let origin = Instant::now();
    let receiver = tokio::spawn(async move {
        while let Some(message) = subscriber.next().await {
            assert_eq!(message.payload.len(), 640);
            let sent = u64::from_le_bytes(message.payload[..8].try_into().unwrap());
            if sent == 0 {
                warmup_counter.fetch_add(1, Ordering::Release);
                continue;
            }
            let mut samples = values.lock().unwrap();
            if samples.len() < seconds * 40000 {
                samples.push((origin.elapsed().as_micros() as u64).saturating_sub(sent));
                let planned = u64::from_le_bytes(message.payload[8..16].try_into().unwrap());
                scheduled_values
                    .lock()
                    .unwrap()
                    .push((origin.elapsed().as_micros() as u64).saturating_sub(planned));
            }
            drop(samples);
            counter.fetch_add(1, Ordering::Release);
        }
    });
    let warmup = Instant::now();
    for tick in 0..5000 {
        tokio::time::sleep_until(warmup + Duration::from_millis(tick)).await;
        for _ in 0..40 {
            client
                .publish("test.paced.40000", vec![0_u8; 640].into())
                .await
                .unwrap();
        }
    }
    client.flush().await.unwrap();
    timeout(Duration::from_secs(3), async {
        while warmup_received.load(Ordering::Acquire) != 200000 {
            tokio::time::sleep(Duration::from_millis(1)).await;
        }
    })
    .await
    .expect("paced warmup requires all real subscribed deliveries");
    let warmup_evidence = serde_json::json!({"offered":200000,"admitted":200000,"submitted":200000,"no_connection":0,"failed":0,"unconfirmed":0,"received":warmup_received.load(Ordering::Acquire),"handoff_samples":200000,"receipt_sample_count":warmup_received.load(Ordering::Acquire),"handoff_residual":0,"receipt_residual":0});
    let start = Instant::now();
    let steady_start_unix_ms = phase("paced", "steady_start");
    let planned_origin_us = start.saturating_duration_since(origin).as_micros() as u64;
    let mut offered_bins = vec![0_usize; seconds];
    let mut offered_overflow = 0_usize;
    let mut driver_lags = Vec::with_capacity(seconds * 1000);
    let mut accepted = 0_usize;
    for tick in 0..seconds * 1000 {
        tokio::time::sleep_until(start + Duration::from_millis(tick as u64)).await;
        driver_lags.push(
            Instant::now()
                .saturating_duration_since(start + Duration::from_millis(tick as u64))
                .as_micros() as u64,
        );
        for _ in 0..40 {
            let mut payload = vec![0_u8; 640];
            payload[..8]
                .copy_from_slice(&(origin.elapsed().as_micros() as u64).max(1).to_le_bytes());
            payload[8..16].copy_from_slice(&(planned_origin_us + tick as u64 * 1000).to_le_bytes());
            let bin = start.elapsed().as_secs() as usize;
            if bin < seconds {
                offered_bins[bin] += 1;
            } else {
                offered_overflow += 1;
            }
            client
                .publish("test.paced.40000", payload.into())
                .await
                .unwrap();
            accepted += 1;
        }
    }
    tokio::time::sleep_until(start + Duration::from_secs(seconds as u64)).await;
    let elapsed = start.elapsed();
    let steady_end_unix_ms = phase("paced", "steady_end");
    let drain_started = Instant::now();
    let drained = timeout(Duration::from_secs(3), async {
        client.flush().await.unwrap();
        while received.load(Ordering::Acquire) < seconds * 40000 {
            tokio::time::sleep(Duration::from_millis(1)).await;
        }
    })
    .await
    .is_ok();
    let drain_elapsed_seconds = drain_started.elapsed().as_secs_f64();
    let mut samples = measurements.lock().unwrap();
    let mut scheduled_samples = scheduled_measurements.lock().unwrap();
    let late_ticks = driver_lags.iter().filter(|value| **value > 1000).count();
    let max_lateness_us = driver_lags.iter().copied().max().unwrap_or(0);
    println!(
        "PACED_CORE_PROFILE {}",
        serde_json::json!({
            "schema_version":1,"steady_start_unix_ms":steady_start_unix_ms,"steady_end_unix_ms":steady_end_unix_ms,"warmup_seconds":5,"warmup":warmup_evidence,
            "kind":"paced async-nats equivalent, not official CLI", "offered_rate":40000,
            "seconds":seconds,"offered":seconds*40000,"accepted":accepted,"received":received.load(Ordering::Relaxed),
            "elapsed_seconds":elapsed.as_secs_f64(),"payload_bytes":640,"drained":drained,
            "drain_elapsed_seconds":drain_elapsed_seconds,"receipt_residual":accepted.saturating_sub(received.load(Ordering::Relaxed)),"latency_sample_count":samples.len(),"scheduled_latency_sample_count":scheduled_samples.len(),"handoff_samples":accepted,
            "actual_offered_rate":(seconds*40000) as f64/elapsed.as_secs_f64(),"actual_accepted_rate":accepted as f64/elapsed.as_secs_f64(),"actual_received_rate":received.load(Ordering::Relaxed) as f64/elapsed.as_secs_f64(),
            "offered_per_second":offered_bins,"offered_after_full_seconds":offered_overflow,"driver_late_ticks":late_ticks,"max_lateness_us":max_lateness_us,
            "driver_lateness_us":[percentile(&mut driver_lags,50),percentile(&mut driver_lags,95),percentile(&mut driver_lags,99)],
            "pacing":"absolute 1ms batches; catch-up bursts and scheduled latency are explicitly measured",
            "end_to_end_us":[percentile(&mut samples,50),percentile(&mut samples,95),percentile(&mut samples,99)],
            "scheduled_to_receipt_us":[percentile(&mut scheduled_samples,50),percentile(&mut scheduled_samples,95),percentile(&mut scheduled_samples,99)],
            "latency_origin":"end_to_end_us begins at actual payload creation; scheduled_to_receipt includes generator lag"
        })
    );
    receiver.abort();
}

#[tokio::test]
#[ignore = "requires BUS_PUBLISH_TEST_URL pointing to an isolated test-owned broker"]
async fn pending_submission_deadline_cancels_the_bounded_queue() {
    let url = std::env::var("BUS_PUBLISH_TEST_URL").expect("test-owned broker URL");
    let client = async_nats::connect(url.clone()).await.unwrap();
    let bus = Arc::new(BrokerBus::new(Some(url), "pending-submission-test"));
    // Keep the real SDK connected but deliberately do not start the supervisor writer.
    let link = Arc::new(Link::default());
    link.client.set(client).ok().unwrap();
    link.ready_revision.store(1, Ordering::SeqCst);
    *bus.shared.link.lock().unwrap() = Some(link);
    let mut pending = tokio::task::JoinSet::new();
    for _ in 0..OUTBOUND_CAPACITY {
        let bus = bus.clone();
        pending.spawn(async move { bus.publish("test.pending", vec![1]).await });
    }
    timeout(Duration::from_secs(1), async {
        while bus.shared.tx.capacity() != 0 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    let rejected = Instant::now();
    assert_eq!(
        bus.publish("test.pending", vec![2]).await,
        PublishOutcome::Failed
    );
    assert!(rejected.elapsed() < Duration::from_millis(100));
    while let Some(result) = pending.join_next().await {
        assert_eq!(result.unwrap(), PublishOutcome::Failed);
    }
    let mut receiver = bus.shared.rx.lock().unwrap().take().unwrap();
    let mut cancelled = 0;
    while let Ok(outbound) = receiver.try_recv() {
        let Outbound::Publish(publish) = outbound else {
            panic!("unexpected request")
        };
        assert_eq!(publish.phase.load(Ordering::SeqCst), CANCELLED);
        assert!(publish.result.is_closed());
        assert!(
            publish
                .phase
                .compare_exchange(PENDING, SUBMITTING, Ordering::SeqCst, Ordering::SeqCst)
                .is_err()
        );
        cancelled += 1;
    }
    assert_eq!(cancelled, OUTBOUND_CAPACITY);
}

#[tokio::test]
#[ignore = "requires BUS_PUBLISH_TEST_URL pointing to an isolated test-owned broker"]
async fn local_submission_rejects_invalid_and_oversize_commands() {
    let url = std::env::var("BUS_PUBLISH_TEST_URL").expect("test-owned broker URL");
    let bus = BrokerBus::new(Some(url), "publish-boundary-test");
    bus.connect();
    timeout(Duration::from_secs(12), async {
        while !bus.is_ready() {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();
    let subject = "test.boundary";
    assert_eq!(
        bus.publish(subject, vec![0; MAX_PUBLISH_BYTES - subject.len()])
            .await,
        PublishOutcome::Submitted
    );
    assert_eq!(
        bus.publish(subject, vec![0; MAX_PUBLISH_BYTES - subject.len() + 1])
            .await,
        PublishOutcome::Failed
    );
    // The pinned SDK checks protocol framing locally; broker subject acceptance is asynchronous.
    for invalid in ["", "test subject", "test\r\nsubject"] {
        assert_eq!(bus.publish(invalid, vec![1]).await, PublishOutcome::Failed);
    }
    assert!(bus.is_ready());
}

struct OwnedBroker(std::process::Child, u16);
impl OwnedBroker {
    async fn start(port: Option<u16>) -> Self {
        Self::start_with_config(port, None).await
    }
    async fn start_with_config(port: Option<u16>, config: Option<&std::path::Path>) -> Self {
        let binary = std::env::var("BUS_PUBLISH_SERVER_BIN").expect("test-owned server binary");
        let port = port.unwrap_or_else(|| {
            std::net::TcpListener::bind("127.0.0.1:0")
                .unwrap()
                .local_addr()
                .unwrap()
                .port()
        });
        let mut command = std::process::Command::new(binary);
        command
            .args(["--addr", "127.0.0.1", "--port", &port.to_string()])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x08000000); // CREATE_NO_WINDOW
        }
        if let Some(config) = config {
            command.arg("--config").arg(config);
        }
        let fixture = Self(command.spawn().expect("start owned loopback broker"), port);
        timeout(Duration::from_secs(5), async {
            loop {
                if let Ok(client) = tokio::net::TcpStream::connect(("127.0.0.1", fixture.1)).await {
                    drop(client);
                    break;
                }
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        })
        .await
        .expect("owned broker ready");
        fixture
    }
    fn url(&self) -> String {
        format!("nats://127.0.0.1:{}", self.1)
    }
}
impl Drop for OwnedBroker {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

#[tokio::test]
#[ignore = "requires BUS_PUBLISH_SERVER_BIN; starts and reaps its own hidden loopback broker"]
async fn restart_rejects_disconnected_calls_and_restores_fresh_positions_once() {
    let broker = OwnedBroker::start(None).await;
    let port = broker.1;
    let bus = BrokerBus::new(Some(broker.url()), "restart-submission-test");
    let deliveries = Arc::new(AtomicUsize::new(0));
    let count = deliveries.clone();
    let _subscription = bus.subscribe(
        "test.restored",
        None,
        Arc::new(move |_, _| {
            count.fetch_add(1, Ordering::SeqCst);
        }),
    );
    bus.connect();
    timeout(Duration::from_secs(12), async {
        while !bus.is_ready() {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();
    let old = bus.shared.link().unwrap();
    let old_revision = old.revision.load(Ordering::SeqCst);
    let old_weak = Arc::downgrade(&old);
    drop(old);
    drop(broker);
    timeout(Duration::from_secs(5), async {
        while bus.is_ready() {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();
    for _ in 0..32 {
        assert_eq!(
            bus.publish("test.position", vec![1]).await,
            PublishOutcome::NoConnection
        );
    }
    let restarted = OwnedBroker::start(Some(port)).await;
    let observer = async_nats::connect(restarted.url()).await.unwrap();
    let mut positions = observer.subscribe("test.position").await.unwrap();
    observer.flush().await.unwrap();
    timeout(Duration::from_secs(12), async {
        while !bus.is_ready() {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();
    let current = bus.shared.link().unwrap();
    if let Some(old) = old_weak.upgrade() {
        if Arc::ptr_eq(&old, &current) {
            assert!(current.revision.load(Ordering::SeqCst) > old_revision);
        }
    }
    drop(current);
    assert_eq!(
        bus.publish("test.position", vec![2]).await,
        PublishOutcome::Submitted
    );
    assert_eq!(
        timeout(Duration::from_secs(2), positions.next())
            .await
            .unwrap()
            .unwrap()
            .payload
            .as_ref(),
        &[2]
    );
    assert!(
        timeout(Duration::from_millis(100), positions.next())
            .await
            .is_err()
    );
    observer
        .publish("test.restored", vec![3].into())
        .await
        .unwrap();
    observer.flush().await.unwrap();
    timeout(Duration::from_secs(2), async {
        while deliveries.load(Ordering::SeqCst) == 0 {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert_eq!(
        deliveries.load(Ordering::SeqCst),
        1,
        "subscription activated exactly once on new incarnation"
    );
}

/// One owned TCP bridge. The first wire can be held without consuming bytes, then severed;
/// later connections remain available so a client-side SDK reconnect can be distinguished.
struct WireGate {
    url: String,
    hold: tokio_util::sync::CancellationToken,
    held: Arc<AtomicBool>,
    sever: tokio_util::sync::CancellationToken,
    connections: Arc<AtomicUsize>,
    worker: JoinHandle<()>,
}
impl WireGate {
    async fn start(upstream_port: u16) -> Self {
        use tokio::io::{AsyncWriteExt, copy_bidirectional};
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("nats://{}", listener.local_addr().unwrap());
        let hold = tokio_util::sync::CancellationToken::new();
        let held = Arc::new(AtomicBool::new(false));
        let sever = tokio_util::sync::CancellationToken::new();
        let connections = Arc::new(AtomicUsize::new(0));
        let stop = hold.clone();
        let held_flag = held.clone();
        let cut = sever.clone();
        let count = connections.clone();
        let worker = tokio::spawn(async move {
            loop {
                let Ok((mut downstream, _)) = listener.accept().await else {
                    break;
                };
                let Ok(mut upstream) =
                    tokio::net::TcpStream::connect(("127.0.0.1", upstream_port)).await
                else {
                    break;
                };
                let connection = count.fetch_add(1, Ordering::SeqCst);
                if connection == 0 {
                    tokio::select! {
                        _ = copy_bidirectional(&mut downstream, &mut upstream) => {},
                        _ = stop.cancelled() => {
                            held_flag.store(true, Ordering::SeqCst);
                            cut.cancelled().await;
                            let _ = downstream.shutdown().await;
                            let _ = upstream.shutdown().await;
                        }
                    }
                } else {
                    let _ = copy_bidirectional(&mut downstream, &mut upstream).await;
                }
            }
        });
        Self {
            url,
            hold,
            held,
            sever,
            connections,
            worker,
        }
    }
    async fn hold_first_wire(&self) {
        self.hold.cancel();
        timeout(Duration::from_secs(1), async {
            while !self.held.load(Ordering::SeqCst) {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("proxy reached controlled hold");
    }
}
impl Drop for WireGate {
    fn drop(&mut self) {
        self.worker.abort();
    }
}

// Pass a real periodic probe, then a subsequent supervisor request before holding the wire.
// This avoids racing the interval's immediate startup tick with the fixture's first publish.
async fn settle_periodic_probe(bus: &BrokerBus, observer: &async_nats::Client) {
    let mut probes = observer.subscribe("_INBOX.>").await.unwrap();
    observer.flush().await.unwrap();
    let started = Instant::now();
    timeout(Duration::from_secs(8), async {
        while probes.next().await.is_some() {
            if started.elapsed() >= Duration::from_millis(4500) {
                break;
            }
        }
    })
    .await
    .expect("periodic probe observed before controlled hold");
    assert_eq!(
        bus.request("test.fixture.setup.no-responder", vec![]).await,
        Err(RequestError::NoResponders)
    );
}
#[tokio::test(flavor = "multi_thread", worker_threads = 6)]
#[ignore = "real-broker bounded SDK buffering across reconnect; requires owned server binary"]
async fn sdk_accepted_positions_may_arrive_after_reconnect() {
    let broker = OwnedBroker::start(None).await;
    let proxy = WireGate::start(broker.1).await;
    let observer = async_nats::connect(broker.url()).await.unwrap();
    let production_frames = std::env::var_os("BUS_REPLAY_PRODUCTION_FRAMES").is_some();
    let subject = if production_frames {
        SUBJECT
    } else {
        "test.queued.position"
    };
    let payload_for = |id: u64| {
        if production_frames {
            ControlPosition {
                audience: "a".repeat(256),
                wallet: format!("0x{:040x}", 1),
                session: format!("0x{:040x}", 2),
                epoch: 1,
                sequence: id,
                realm: "r".repeat(256),
                position: Some([1.0, 2.0, 3.0]),
            }
            .encode()
            .unwrap()
        } else {
            let mut bytes = vec![0_u8; MAX_PUBLISH_BYTES - subject.len()];
            bytes[..8].copy_from_slice(&id.to_le_bytes());
            bytes
        }
    };
    let id_from = |bytes: &[u8]| {
        if production_frames {
            ControlPosition::decode(bytes).unwrap().sequence
        } else {
            u64::from_le_bytes(bytes[..8].try_into().unwrap())
        }
    };
    let mut records = observer.subscribe(subject).await.unwrap();
    observer.flush().await.unwrap();
    let bus = BrokerBus::new(Some(proxy.url.clone()), "sdk-queued-reconnect-test");
    bus.connect();
    timeout(Duration::from_secs(12), async {
        while !bus.is_ready() {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();
    settle_periodic_probe(&bus, &observer).await;
    let original = bus.shared.link().unwrap();
    let statistics = original.client.get().unwrap().statistics();
    let original_weak = Arc::downgrade(&original);
    drop(original); // The observer must not keep an SDK client or retired link alive.
    proxy.hold_first_wire().await;
    let consumed_before = statistics.out_messages.load(Ordering::SeqCst);
    let mut accepted = 0_u64;
    // Bound the driver and stop while SDK commands are pending, before a two-second ambiguous
    // submission can itself force teardown. out_messages counts SDK command consumption.
    let fill_started = Instant::now();
    while accepted < if production_frames { 65536 } else { 2048 } {
        let payload = payload_for(accepted + 1);
        let outcome = timeout(Duration::from_millis(200), bus.publish(subject, payload))
            .await
            .expect("stop before SDK capacity is exhausted");
        assert_eq!(outcome, PublishOutcome::Submitted);
        accepted += 1;
        let consumed = statistics.out_messages.load(Ordering::SeqCst) - consumed_before;
        if accepted.saturating_sub(consumed) >= 32 {
            tokio::time::sleep(Duration::from_millis(20)).await;
            let stable = statistics.out_messages.load(Ordering::SeqCst) - consumed_before;
            if accepted.saturating_sub(stable) >= 32 {
                break;
            }
        }
    }
    tokio::time::sleep(Duration::from_millis(30)).await;
    // Statistics include any private fence PUB too: this is a conservative upper bound
    // on consumed position IDs, so IDs above it are definitely still SDK-queued.
    let consumed_at_cut = statistics.out_messages.load(Ordering::SeqCst) - consumed_before;
    assert!(
        accepted > consumed_at_cut,
        "fixture must prove commands remain in the SDK receiver"
    );
    assert!(
        fill_started.elapsed() < Duration::from_secs(4),
        "must sever before the periodic probe while no publish remains ambiguous"
    );
    proxy.sever.cancel();
    let mut replayed = Vec::new();
    let observe_until = Instant::now() + Duration::from_secs(3);
    while let Ok(Some(message)) = timeout_at(observe_until, records.next()).await {
        let id = id_from(&message.payload);
        if id > consumed_at_cut {
            replayed.push(id);
        }
    }
    // Keep observing through readiness/fresh submission, so a delayed SDK reconnect cannot
    // hide merely by occurring after the first three-second observation interval.
    timeout(Duration::from_secs(12), async {
        while !bus.is_ready() {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .expect("fresh current link recovers");
    let fresh_id = accepted + 1;
    assert_eq!(
        bus.publish(subject, payload_for(fresh_id)).await,
        PublishOutcome::Submitted
    );
    timeout(Duration::from_secs(2), async {
        loop {
            let message = records.next().await.expect("observer remains live");
            let id = id_from(&message.payload);
            if id == fresh_id {
                break;
            }
            if id > consumed_at_cut {
                replayed.push(id);
            }
        }
    })
    .await
    .expect("fresh position arrives after recovered readiness");
    let tail = Instant::now() + Duration::from_millis(200);
    while let Ok(Some(message)) = timeout_at(tail, records.next()).await {
        let id = id_from(&message.payload);
        if id > consumed_at_cut && id <= accepted {
            replayed.push(id);
        }
    }
    println!(
        "SDK_REPLAY {}",
        serde_json::json!({
            "production_frames":production_frames,"frame_bytes":payload_for(1).len(),
            "accepted":accepted,"consumed_position_upper_bound_before_cut":consumed_at_cut,
            "proven_sdk_queued_lower_bound":accepted-consumed_at_cut,"new_wire_connections":proxy.connections.load(Ordering::SeqCst),
            "replayed_sdk_ids":replayed,"same_link_object":bus.shared.link().is_some_and(|link|original_weak.upgrade().is_some_and(|old|Arc::ptr_eq(&old,&link)))
        })
    );
    // Human-accepted contract: SDK acceptance can be followed by buffered delivery on a new
    // wire. It promises neither delivery nor purge. Every observed marker was locally accepted.
    // This deliberately created 32 queued markers: allowed-delay demonstration, not a
    // measurement of SDK capacity. Separate SDK-full backpressure coverage checks saturation.
    assert!(
        replayed
            .iter()
            .all(|id| *id > consumed_at_cut && *id <= accepted)
    );
    assert!(replayed.len() <= (accepted - consumed_at_cut) as usize);
    assert!(proxy.connections.load(Ordering::SeqCst) >= 2);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 6)]
#[ignore = "real-broker controlled blackhole and submitting ambiguity; requires owned server binary"]
async fn controlled_stall_bounds_probe_detection_and_submitting_ambiguity() {
    let broker = OwnedBroker::start(None).await;
    let proxy = WireGate::start(broker.1).await;
    let bus = BrokerBus::new(Some(proxy.url.clone()), "blackhole-probe-test");
    bus.connect();
    timeout(Duration::from_secs(12), async {
        while !bus.is_ready() {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();
    proxy.hold_first_wire().await;
    let started = Instant::now();
    timeout(Duration::from_secs(8), async {
        while bus.is_ready() {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .expect("five-second probe plus bounded exchange must invalidate a blackhole");
    println!(
        "BLACKHOLE_DETECTION_SECONDS {}",
        started.elapsed().as_secs_f64()
    );
    assert_eq!(
        bus.publish("test.position", vec![1]).await,
        PublishOutcome::NoConnection
    );
    drop(bus);
    drop(proxy);

    let proxy = WireGate::start(broker.1).await;
    let bus = BrokerBus::new(Some(proxy.url.clone()), "submitting-ambiguity-test");
    bus.connect();
    timeout(Duration::from_secs(12), async {
        while !bus.is_ready() {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();
    let observer = async_nats::connect(broker.url()).await.unwrap();
    settle_periodic_probe(&bus, &observer).await;
    let original = bus.shared.link().unwrap();
    let sdk = original.client.get().unwrap();
    proxy.hold_first_wire().await;
    let fill_started = Instant::now();
    let mut full = false;
    let consumed_before_fill = sdk.statistics().out_messages.load(Ordering::SeqCst);
    let mut attempted = 0_u64;
    let mut accepted = 0_u64;
    for _ in 0..4096 {
        attempted += 1;
        match timeout(
            Duration::from_millis(50),
            sdk.publish("test.fill", vec![0_u8; MAX_PUBLISH_BYTES - 9].into()),
        )
        .await
        {
            Err(_) => {
                full = true;
                break;
            }
            Ok(Ok(())) => {
                accepted += 1;
            }
            Ok(Err(error)) => {
                panic!("SDK fill rejected locally instead of reaching backpressure: {error}")
            }
        }
    }
    let consumed_at_plateau = sdk.statistics().out_messages.load(Ordering::SeqCst);
    attempted += 1;
    assert!(
        timeout(
            Duration::from_millis(50),
            sdk.publish("test.fill", vec![0_u8; MAX_PUBLISH_BYTES - 9].into())
        )
        .await
        .is_err(),
        "second bounded offer remains blocked"
    );
    assert_eq!(
        sdk.statistics().out_messages.load(Ordering::SeqCst),
        consumed_at_plateau,
        "SDK consumption plateau while wire held"
    );
    println!(
        "SDK_BACKPRESSURE {}",
        serde_json::json!({
            "attempted":attempted,"sdk_accepted":accepted,"sdk_processed_publish_delta":sdk.statistics().out_messages.load(Ordering::SeqCst)-consumed_before_fill,
            "stable_blocked_ms":100,"sdk_channel_capacity":OUTBOUND_CAPACITY,"frame_bytes":MAX_PUBLISH_BYTES-9,
            "boundary":"SDK channel plus writer/socket backpressure; not a total128frame or memory cap", "fill_seconds":fill_started.elapsed().as_secs_f64()
        })
    );
    assert!(
        full && fill_started.elapsed() < Duration::from_secs(2),
        "fixture stalls the bounded SDK writer promptly"
    );
    let started = Instant::now();
    let outcome = bus.publish("test.ambiguous", vec![1]).await;
    assert_eq!(
        outcome,
        PublishOutcome::Unconfirmed,
        "submitting timeout cannot prove nondelivery"
    );
    assert!(started.elapsed() >= BROKER_DEADLINE && started.elapsed() < Duration::from_secs(3));
    assert!(!bus.is_ready());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 6)]
#[ignore = "test-only broker permission rejection; requires owned server binary"]
async fn asynchronous_permission_error_invalidates_without_claiming_delivery() {
    let config = std::env::temp_dir().join(format!(
        "comms-bus-permission-{}.conf",
        uuid::Uuid::new_v4()
    ));
    // These are test-only fixture credentials, not service credentials.
    std::fs::write(&config,r#"no_auth_user: fixture
authorization { users: [{ user: fixture, password: fixture, permissions: { publish: { allow: [">"], deny: ["test.denied"] }, subscribe: ">" } }] }"#).unwrap();
    let broker = OwnedBroker::start_with_config(None, Some(&config)).await;
    let _ = std::fs::remove_file(&config);
    let url = broker.url();
    // Pinned SDK builds CONNECT credentials from explicit auth options, not URL user-info.
    // The test-only anonymous mapping gives unmodified BrokerBus the same deny permissions.
    let probe =
        async_nats::ConnectOptions::with_user_and_password("fixture".into(), "fixture".into())
            .connect(url.clone())
            .await
            .expect("fixture credentials connect");
    let mut probe_subscription = probe.subscribe("_INBOX.fixture.permission").await.unwrap();
    probe.flush().await.unwrap();
    probe
        .publish("_INBOX.fixture.permission", vec![1].into())
        .await
        .unwrap();
    assert!(
        timeout(Duration::from_secs(2), probe_subscription.next())
            .await
            .is_ok(),
        "fixture permissions allow readiness inbox exchange"
    );
    let bus = BrokerBus::new(Some(url), "permission-error-test");
    bus.connect();
    timeout(Duration::from_secs(12), async {
        while !bus.is_ready() {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();
    let original = bus.shared.link().unwrap();
    let outcome = bus.publish("test.denied", vec![1]).await;
    assert!(
        matches!(
            outcome,
            PublishOutcome::Submitted | PublishOutcome::Unconfirmed
        ),
        "broker permission is asynchronous, not local SDK rejection"
    );
    timeout(Duration::from_secs(2), async {
        while !original.broken.load(Ordering::SeqCst) {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .expect("permission event invalidates old incarnation");
    assert!(!original.ready());
    println!("PERMISSION_OUTCOME {outcome:?}; old incarnation asynchronously broken");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 6)]
#[ignore = "real-broker bounded slow subscription fault; requires owned server binary"]
async fn asynchronous_slow_consumer_invalidates_the_incarnation() {
    let broker = OwnedBroker::start(None).await;
    let bus = BrokerBus::new(Some(broker.url()), "slow-consumer-test");
    let callbacks = Arc::new(AtomicUsize::new(0));
    let count = callbacks.clone();
    let _subscription = bus.subscribe(
        "test.slow",
        None,
        Arc::new(move |_, _| {
            count.fetch_add(1, Ordering::Relaxed);
            // Deliberately stall only this bounded subscription callback, not a runtime-wide gate.
            std::thread::sleep(Duration::from_millis(20));
        }),
    );
    bus.connect();
    timeout(Duration::from_secs(12), async {
        while !bus.is_ready() {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();
    let original = bus.shared.link().unwrap();
    let publisher = async_nats::connect(broker.url()).await.unwrap();
    for _ in 0..4096 {
        publisher
            .publish("test.slow", vec![0_u8; 256].into())
            .await
            .unwrap();
    }
    publisher.flush().await.unwrap();
    timeout(Duration::from_secs(3), async {
        while !original.broken.load(Ordering::SeqCst) {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .expect("bounded SDK subscription overflow breaks incarnation asynchronously");
    assert!(!original.ready());
    println!(
        "SLOW_CONSUMER old incarnation broken; callback_count={}",
        callbacks.load(Ordering::Relaxed)
    );
}
