use super::*;
use serde_json::{Value, json};
use std::collections::BTreeMap;

fn config(value: &Value) -> OrdinaryQueueConfig {
    let default = OrdinaryQueueConfig::default();
    let number = |name: &str, fallback: f64| {
        value[name]
            .as_f64()
            .or_else(|| value[name].as_str().map(|v| v.parse().unwrap()))
            .unwrap_or(fallback)
    };
    OrdinaryQueueConfig {
        max_queue_size: number("max_queue_size", default.max_queue_size),
        min_queue_size: number("min_queue_size", default.min_queue_size),
        max_queue_size_limit: number("max_queue_size_limit", default.max_queue_size_limit),
        estimated_message_size: number("estimated_message_size", default.estimated_message_size),
        retry_delay_ms: number("retry_delay_ms", default.retry_delay_ms),
        max_retry_attempts: number("max_retry_attempts", default.max_retry_attempts),
        max_backoff_delay_ms: number("max_backoff_delay_ms", default.max_backoff_delay_ms),
    }
}
fn poll_futures(futures: &mut BTreeMap<u64, (oneshot::Receiver<Result<(), String>>, String)>) {
    for (receiver, state) in futures.values_mut() {
        if state != "pending" {
            continue;
        }
        match receiver.try_recv() {
            Ok(Ok(())) => *state = "resolved".into(),
            Ok(Err(error)) => *state = format!("rejected:{error}"),
            Err(oneshot::error::TryRecvError::Empty) => {}
            Err(error) => panic!("completion lost: {error}"),
        }
    }
}

#[test]
fn source_transport_timer_drain_errors_and_capacity_traces() {
    let fixture: Value = serde_json::from_str(include_str!(
        "../../../../../tests/fixtures/conformance/social/ws-transport-source-cases.json"
    ))
    .unwrap();
    assert_eq!(
        fixture["baseline"]["revision"],
        "b6b2cefefdf727b2ed47ddb887e77d226a41eea6"
    );
    for case in fixture["cases"].as_array().unwrap() {
        let id = case["id"].as_str().unwrap();
        let max = case["config"]["WS_MAX_BACKPRESSURE"]
            .as_str()
            .map(|v| v.parse().unwrap())
            .unwrap_or(131072.0);
        if case["capacity"].is_string() || max == 0.0 {
            assert!(
                config(&case["configuration"]).capacity(max).is_err(),
                "explicit bounded-mode startup difference: {id}"
            );
            continue;
        }
        let mut queue = Queue::new(config(&case["configuration"]), max);
        assert_eq!(queue.capacity, case["capacity"].as_f64().unwrap(), "{id}");
        let start = Instant::now();
        let mut now = start;
        let mut connected = true;
        let mut index = 0;
        let mut buffered = 0usize;
        let mut sends = Vec::new();
        let mut closes = Vec::new();
        let mut received = Vec::new();
        let mut futures = BTreeMap::new();
        let script = case["script"].as_array().unwrap();
        let threshold = case["threshold"].as_bool().unwrap();
        for (action_index, action) in case["actions"].as_array().unwrap().iter().enumerate() {
            match action["op"].as_str().unwrap() {
                "send" => {
                    let label = action["label"].as_u64().unwrap();
                    let mut bytes = vec![0; action["bytes"].as_u64().unwrap_or(1) as usize];
                    bytes[0] = label as u8;
                    futures.insert(
                        label,
                        (queue.enqueue(bytes, connected), "pending".to_owned()),
                    );
                }
                "buffer" => buffered = action["bytes"].as_u64().unwrap() as usize,
                "disconnect" => connected = false,
                "close" => queue.close(),
                "receive" if queue.active => received.push(action["bytes"].clone()),
                "receive" => {}
                "drain" => {
                    buffered = action["buffered"].as_u64().unwrap_or(0) as usize;
                    queue.retry = None;
                }
                "tick" => {}
                op => panic!("unexpected action {op}"),
            }
            let until = now + Duration::from_millis(action["ms"].as_u64().unwrap_or(0));
            loop {
                if connected {
                    queue.process(now, |bytes| {
                        let result = if threshold {
                            let result = if buffered as f64 > max { 2 } else { 0 };
                            if result != 2 { buffered += bytes.len(); }
                            json!(result)
                        } else {
                            let result = script.get(index.min(script.len().saturating_sub(1))).cloned().unwrap_or(json!(1));
                            index += 1;
                            result
                        };
                        sends.push(json!({"at":now.duration_since(start).as_millis(), "label":bytes[0], "bytes":bytes.len(), "result":result}));
                        if result == "throw" { SendOutcome::Failure("synthetic send failure".into()) }
                        else { SendOutcome::Code(result.as_i64().unwrap() as i32) }
                    }, || 0);
                }
                if !queue.active && connected {
                    closes.push(json!({"code":1011,"reason":"RPC transport closed"}));
                    connected = false;
                }
                if let Some(retry) = queue.retry.filter(|retry| *retry <= until && *retry > now) {
                    now = retry;
                } else {
                    now = until;
                    break;
                }
            }
            poll_futures(&mut futures);
            let checkpoint = json!({"at":now.duration_since(start).as_millis(), "sends":sends.len(), "errors":queue.errors.len(), "closed":closes.len(),
                "pending":futures.values().filter(|(_,s)|s=="pending").count(),
                "resolved":futures.values().filter(|(_,s)|s=="resolved").count(),
                "rejected":futures.values().filter(|(_,s)|s.starts_with("rejected:")).count()});
            assert_eq!(
                checkpoint, case["checkpoints"][action_index],
                "{id}: action {action_index} {action}"
            );
        }
        assert_eq!(json!(sends), case["sends"], "{id}");
        assert_eq!(json!(queue.errors), case["errors"], "{id}");
        assert_eq!(json!(queue.metrics), case["metrics"], "{id}");
        assert_eq!(json!(closes), case["closes"], "{id}");
        assert_eq!(json!(received), case["received"], "{id}");
        let futures: BTreeMap<String, String> = futures
            .into_iter()
            .map(|(key, (_, value))| (key.to_string(), value))
            .collect();
        assert_eq!(json!(futures), case["futures"], "{id}");
    }
}

#[test]
fn nonfinite_computed_capacity_is_rejected_and_finite_fraction_is_preserved() {
    let config = OrdinaryQueueConfig {
        max_queue_size: f64::INFINITY,
        min_queue_size: f64::INFINITY,
        ..Default::default()
    };
    assert!(config.capacity(131072.0).is_err());
    assert!(
        OrdinaryQueueConfig::default()
            .capacity(0.0)
            .unwrap_err()
            .contains("zero disables")
    );
    assert_eq!(
        OrdinaryQueueConfig {
            max_queue_size: 1.5,
            ..Default::default()
        }
        .capacity(131072.0)
        .unwrap(),
        1.5
    );
    assert_eq!(
        OrdinaryQueueConfig::default().capacity(131072.0).unwrap(),
        128.0
    );
}

#[test]
fn backpressure_uses_source_raw_queue_capacity_and_native_wrapped_writer_limit() {
    let fixture: Value = serde_json::from_str(include_str!(concat!(
        env!("DCL_ONE_ROOT"),
        "/services/catalyst/tests/fixtures/conformance/social/ws-backpressure-source-cases.json"
    )))
    .unwrap();
    for case in fixture["cases"].as_array().unwrap() {
        if case.get("error").is_some() {
            continue;
        }
        let raw = &case["raw"];
        let number = case["number"]
            .as_f64()
            .unwrap_or_else(|| case["number"].as_str().unwrap().parse().unwrap());
        let config = OrdinaryQueueConfig::default();
        if case["bounded_target_supported"] == false {
            assert!(native_backpressure_limit(number).is_err(), "{raw}");
            assert!(config.capacity(number).is_err(), "{raw}");
            continue;
        }
        let native = native_backpressure_limit(number).unwrap();
        assert_eq!(native as u64, case["native_u32"].as_u64().unwrap(), "{raw}");
        let mut queue = Queue::new(config, number);
        assert_eq!(queue.capacity, case["capacity"].as_f64().unwrap(), "{raw}");
        // Retain the raw capacity through the production queue, then compare
        // actual admission/overflow and send count with the source execution.
        let mut completions = Vec::new();
        let mut sends = 0;
        let mut overflow_at = None;
        let now = Instant::now();
        for index in 1..=1001 {
            completions.push(queue.enqueue(vec![1], true));
            queue.process(
                now,
                |_| {
                    sends += 1;
                    SendOutcome::Code(2)
                },
                || 0,
            );
            if !queue.active {
                overflow_at = Some(index);
                break;
            }
        }
        assert_eq!(json!(overflow_at), case["overflow_at"], "{raw}");
        assert_eq!(json!(sends), case["send_calls"], "{raw}");
        assert_eq!(json!(queue.errors), case["errors"], "{raw}");
        let budget = ByteBudget::new(native);
        let crossing = budget.try_reserve_buffered(native + 1).unwrap();
        assert!(crossing.1, "{raw}");
        assert!(budget.try_reserve_buffered(1).is_none(), "{raw}");
        drop(crossing);
    }
}

#[tokio::test(start_paused = true)]
async fn writer_budget_admits_crossing_frame_drains_promptly_and_does_not_duplicate_it() {
    let (output, mut receiver) = tokio::sync::mpsc::unbounded_channel();
    let budget = ByteBudget::new(1024);
    let close = Arc::new(SocketClose::default());
    let lane = OrdinaryLane::new(
        OrdinaryQueueConfig::default(),
        output,
        budget.clone(),
        close.clone(),
        1024.0,
    );
    // Bytes exactly at the threshold still admit the next frame.
    let at_limit = budget.reserve(1024).await;
    lane.send(vec![1; 2048]).await.unwrap().unwrap();
    assert_eq!(budget.used(), 3072);
    let second = lane.send(vec![2]);
    let first = receiver.try_recv().unwrap();
    assert_eq!(first.bytes.len(), 2048);
    assert!(receiver.try_recv().is_err());
    drop(first);
    drop(at_limit);
    for _ in 0..20 {
        tokio::task::yield_now().await;
    }
    assert!(second.await.unwrap().is_ok());
    assert_eq!(receiver.try_recv().unwrap().bytes, vec![2]);
    assert!(receiver.try_recv().is_err());
    assert!(!close.is_ended());
    let weak = Arc::downgrade(&lane);
    drop(lane);
    for _ in 0..20 {
        tokio::task::yield_now().await;
    }
    assert!(
        weak.upgrade().is_none(),
        "worker cannot retain transport queue"
    );
}

#[tokio::test(start_paused = true)]
async fn stopped_writer_bounds_pending_queue_and_retires_socket() {
    let (output, _receiver) = tokio::sync::mpsc::unbounded_channel();
    let budget = ByteBudget::new(1024);
    let close = Arc::new(SocketClose::default());
    let config = OrdinaryQueueConfig {
        max_queue_size: 2.0,
        min_queue_size: 2.0,
        max_queue_size_limit: 2.0,
        ..Default::default()
    };
    let lane = OrdinaryLane::new(config, output, budget, close.clone(), 1024.0);
    lane.send(vec![1; 2048]).await.unwrap().unwrap();
    let one = lane.send(vec![2]);
    let two = lane.send(vec![3]);
    assert!(lane.send(vec![4]).await.unwrap().is_ok());
    assert!(one.await.unwrap().is_err());
    assert!(two.await.unwrap().is_err());
    assert_eq!(close.frame(), Some(RPC_TRANSPORT_CLOSED));
}

#[test]
fn source_declared_metric_kinds_match_actual_queue_registration() {
    #[derive(Default)]
    struct Kinds(std::sync::Mutex<std::collections::HashMap<String, &'static str>>);
    impl metrics::Recorder for Kinds {
        fn describe_counter(
            &self,
            _: metrics::KeyName,
            _: Option<metrics::Unit>,
            _: metrics::SharedString,
        ) {
        }
        fn describe_gauge(
            &self,
            _: metrics::KeyName,
            _: Option<metrics::Unit>,
            _: metrics::SharedString,
        ) {
        }
        fn describe_histogram(
            &self,
            _: metrics::KeyName,
            _: Option<metrics::Unit>,
            _: metrics::SharedString,
        ) {
        }
        fn register_counter(
            &self,
            key: &metrics::Key,
            _: &metrics::Metadata<'_>,
        ) -> metrics::Counter {
            self.0.lock().unwrap().insert(key.name().into(), "counter");
            metrics::Counter::noop()
        }
        fn register_gauge(&self, key: &metrics::Key, _: &metrics::Metadata<'_>) -> metrics::Gauge {
            self.0.lock().unwrap().insert(key.name().into(), "gauge");
            metrics::Gauge::noop()
        }
        fn register_histogram(
            &self,
            key: &metrics::Key,
            _: &metrics::Metadata<'_>,
        ) -> metrics::Histogram {
            self.0
                .lock()
                .unwrap()
                .insert(key.name().into(), "histogram");
            metrics::Histogram::noop()
        }
    }
    let fixture: Value = serde_json::from_str(include_str!(
        "../../../../../tests/fixtures/conformance/social/ws-transport-source-cases.json"
    ))
    .unwrap();
    let recorder = Kinds::default();
    metrics::with_local_recorder(&recorder, || {
        let mut queue = Queue::new(OrdinaryQueueConfig::default(), 131072.0);
        let _completion = queue.enqueue(vec![1], true);
        queue.process(Instant::now(), |_| SendOutcome::Code(1), || 4096);
    });
    let actual = recorder.0.lock().unwrap();
    for name in [
        "ws_messages_sent",
        "ws_message_size_bytes",
        "ws_queue_vs_backpressure_ratio",
    ] {
        assert_eq!(
            actual.get(name).copied().unwrap(),
            fixture["metric_declarations"][name]["type"]
                .as_str()
                .unwrap(),
            "{name}"
        );
    }
}
