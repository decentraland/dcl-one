use super::*;
use crate::portal::compare::{MAX_OFFER_LOGS, Verdict};
use crate::portal::tests::{A, T0, fake, group, h, header_line, log_line, ok_ndjson, status};
use serde_json::json;
use std::collections::HashMap;

fn rpc_log(block: u64, index: u64) -> RpcLog {
    RpcLog {
        address: A.into(),
        topics: vec![T0.into()],
        data: "0x".into(),
        block_number: format!("0x{block:x}"),
        log_index: format!("0x{index:x}"),
        block_timestamp: None,
        block_hash: Some(h(block)),
        transaction_hash: Some(h(9_000 + index)),
    }
}

struct Parts {
    groups: Vec<LogGroup>,
    logs: Vec<RpcLog>,
    times: HashMap<i64, i64>,
    hash: String,
}

fn parts(logs: Vec<RpcLog>) -> Parts {
    let times = logs
        .iter()
        .map(|l| {
            let b = crate::logs::log_position(l).0;
            (b, 1_700_000_000 + b)
        })
        .collect();
    Parts {
        groups: vec![group(&[A], json!([T0]))],
        logs,
        times,
        hash: h(150),
    }
}

fn view<'a>(p: &'a Parts, chain_id: u64) -> AcceptedView<'a> {
    AcceptedView {
        chain_id,
        from: 101,
        reached: 150,
        reached_hash: &p.hash,
        groups: &p.groups,
        logs: &p.logs,
        times: &p.times,
    }
}

fn reader(origin: &str) -> PortalReader {
    PortalReader::new(origin, "polygon-mainnet", None, 100.0).unwrap()
}

/// The hand-off never waits and copies nothing it refuses: another chain,
/// a busy worker, or a read past the bounds with the slot free.
#[tokio::test]
async fn an_offer_skips_without_waiting_or_copying() {
    let (shadow, mut worker) = channel(
        reader("http://127.0.0.1:9"),
        "matic",
        AddressHistory::Unknown,
    );
    let small = parts(vec![rpc_log(110, 0)]);
    shadow.offer(&view(&small, 1));
    assert!(
        worker.rx.try_recv().is_err(),
        "another chain is not offered"
    );
    let big = parts(
        (0..MAX_OFFER_LOGS as u64 + 1)
            .map(|i| rpc_log(110, i))
            .collect(),
    );
    shadow.offer(&view(&big, 137));
    assert!(
        worker.rx.try_recv().is_err(),
        "past the bounds, with the slot free, nothing is copied"
    );
    shadow.offer(&view(&small, 137));
    let started = Instant::now();
    shadow.offer(&view(&small, 137));
    assert!(
        started.elapsed() < Duration::from_millis(50),
        "a busy worker never holds the walk"
    );
    let first = worker.rx.try_recv().unwrap();
    assert_eq!((first.from, first.reached, first.logs.len()), (101, 150, 1));
    assert!(
        worker.rx.try_recv().is_err(),
        "the second offer was skipped, not queued"
    );
}

#[tokio::test]
async fn a_sample_compares_an_accepted_read_with_the_portal() {
    let (origin, _) = fake(vec![
        status(404, "", "{\"error\":\"not found\"}"),
        ok_ndjson(&[
            json!({"header": header_line(110), "logs": [log_line(0, A, &[T0])]}),
            json!({"header": header_line(150)}),
        ]),
    ])
    .await;
    let (_shadow, mut worker) = channel(reader(&origin), "matic", AddressHistory::Unknown);
    let p = parts(vec![rpc_log(110, 0)]);
    let read = view(&p, 137).to_owned();
    let out = worker.sample(&read).await;
    assert_eq!(out.verdict, Verdict::Equal);
    assert_eq!(out.compared, Some((101, 150)));
    assert_eq!(out.coverage.advertised_start, None);
    assert_eq!(out.coverage.address_history, AddressHistory::Unknown);
    assert_eq!(worker.reader.sends().admission, 1);
}

/// A dataset that starts after the window is skipped before any page.
#[tokio::test]
async fn a_window_below_the_dataset_start_is_skipped() {
    let (origin, _) = fake(vec![status(
        200,
        "",
        &json!({"start_block": 120}).to_string(),
    )])
    .await;
    let (_shadow, mut worker) = channel(reader(&origin), "matic", AddressHistory::Unknown);
    let p = parts(vec![]);
    let out = worker.sample(&view(&p, 137).to_owned()).await;
    assert_eq!(
        out.verdict,
        Verdict::Skipped {
            reason: "below_dataset_start".into()
        }
    );
    assert_eq!(worker.reader.sends().pages, 0);
}

/// A portal that never answers holds no one: the worker stops on its
/// signal with the sample in flight.
#[tokio::test]
async fn a_stalled_portal_stops_with_the_worker() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://127.0.0.1:{}", listener.local_addr().unwrap().port());
    tokio::spawn(async move {
        let mut held = Vec::new();
        while let Ok((sock, _)) = listener.accept().await {
            held.push(sock);
        }
    });
    let (shadow, worker) = channel(reader(&origin), "matic", AddressHistory::Unknown);
    let p = parts(vec![rpc_log(110, 0)]);
    shadow.offer(&view(&p, 137));
    let (stop_tx, stop_rx) = tokio::sync::oneshot::channel::<()>();
    let run = tokio::spawn(worker.run(async move {
        let _ = stop_rx.await;
    }));
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert!(!run.is_finished(), "the sample waits on the portal");
    let _ = stop_tx.send(());
    tokio::time::timeout(Duration::from_secs(2), run)
        .await
        .expect("the worker stops with its sample in flight")
        .unwrap();
}
