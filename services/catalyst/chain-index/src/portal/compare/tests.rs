use super::*;
use crate::portal::tests::{
    A, B, T0, T1, fake, group, h, header_line, log_line, ok_ndjson, status,
};
use crate::portal::{AddressHistory, PortalReader};
use serde_json::json;

const UNKNOWN: Coverage = Coverage {
    advertised_start: None,
    address_history: AddressHistory::Unknown,
};

fn rpc_log(
    block: u64,
    index: u64,
    address: &str,
    topics: &[&str],
    block_hash: Option<String>,
) -> RpcLog {
    RpcLog {
        address: address.to_string(),
        topics: topics.iter().map(|t| t.to_string()).collect(),
        data: "0x".into(),
        block_number: format!("0x{block:x}"),
        log_index: format!("0x{index:x}"),
        block_timestamp: None,
        block_hash,
        transaction_hash: Some(h(9_000 + index)),
    }
}

fn portal_log(block: u64, index: u32, address: &str, topics: &[&str]) -> PortalLog {
    PortalLog {
        block,
        log_index: index,
        transaction_index: 0,
        address: address.to_string(),
        topics: topics.iter().map(|t| t.to_string()).collect(),
        data: "0x".into(),
        transaction_hash: h(9_000 + index as u64),
    }
}

fn read(
    from: u64,
    reached: u64,
    reached_hash: String,
    logs: Vec<RpcLog>,
    topics: serde_json::Value,
) -> AcceptedRead {
    let times = logs
        .iter()
        .map(|l| {
            let b = log_position(l).0;
            (b, 1_700_000_000 + b)
        })
        .collect();
    AcceptedRead {
        chain_id: 137,
        from,
        reached,
        reached_hash,
        groups: vec![group(&[A], topics)],
        logs,
        times,
    }
}

fn fetched(
    from: u64,
    to: u64,
    common: Option<u64>,
    headers: &[(u64, String)],
    logs: Vec<PortalLog>,
) -> Fetched {
    Fetched {
        from,
        to,
        common_upper: common,
        headers: headers
            .iter()
            .map(|(n, hash)| (*n, (hash.clone(), 1_700_000_000 + *n)))
            .collect(),
        logs: logs
            .into_iter()
            .map(|l| ((l.block, l.log_index), l))
            .collect(),
        ..Default::default()
    }
}

/// The auditor's case: from 101, batches covering 120 and 137, the last
/// matching RPC hash at 110: only 101–110 is compared, the rest skipped.
#[test]
fn only_the_prefix_up_to_the_highest_common_hash_is_compared() {
    let r = read(
        101,
        150,
        h(150),
        vec![rpc_log(110, 0, A, &[T0], Some(h(110)))],
        json!([T0]),
    );
    let f = fetched(
        101,
        150,
        Some(120),
        &[(110, h(110)), (120, h(120)), (137, h(137))],
        vec![portal_log(110, 0, A, &[T0])],
    );
    let out = compare(&r, &f, UNKNOWN);
    assert_eq!(out.verdict, Verdict::Equal);
    assert_eq!(out.fetched, Some((101, 120)));
    assert_eq!(out.compared, Some((101, 110)));
    assert_eq!((out.compared_blocks, out.skipped_blocks), (10, 40));
    assert_eq!(
        out.coverage.address_history,
        AddressHistory::Unknown,
        "diagnostic only"
    );
}

/// A contradictory hash at the highest common height is a branch mismatch,
/// even with an equal hash below it.
#[test]
fn a_contradiction_at_the_anchor_is_never_searched_past() {
    let r = read(
        101,
        150,
        h(150),
        vec![
            rpc_log(110, 0, A, &[T0], Some(h(110))),
            rpc_log(120, 0, A, &[T0], Some(h(999))),
        ],
        json!([T0]),
    );
    let f = fetched(
        101,
        150,
        Some(120),
        &[(110, h(110)), (120, h(120))],
        vec![portal_log(110, 0, A, &[T0]), portal_log(120, 0, A, &[T0])],
    );
    let out = compare(&r, &f, UNKNOWN);
    assert_eq!(out.verdict, Verdict::BranchMismatch { height: 120 });
    assert_eq!(out.compared, None);
}

/// No common height: the predecessor is no anchor, and nothing is compared.
#[test]
fn without_a_common_height_nothing_is_compared() {
    let r = read(101, 150, h(150), vec![], json!([T0]));
    let f = fetched(101, 150, Some(120), &[(120, h(120))], vec![]);
    let out = compare(&r, &f, UNKNOWN);
    assert_eq!(out.verdict, Verdict::Unanchored);
    assert_eq!(
        (out.compared, out.compared_blocks, out.skipped_blocks),
        (None, 0, 50)
    );
}

#[test]
fn an_empty_intersection_is_skipped_whole() {
    let r = read(101, 150, h(150), vec![], json!([T0]));
    let mut f = fetched(101, 150, None, &[], vec![]);
    f.stopped = Some(Stop::AbovePortalFinalized);
    let out = compare(&r, &f, UNKNOWN);
    assert_eq!(
        out.verdict,
        Verdict::Skipped {
            reason: "above_portal_finalized".into()
        }
    );
    assert_eq!((out.fetched, out.skipped_blocks), (None, 50));
}

/// Two empty windows are equal only on one branch: identical observations
/// with unknown coverage are equal (a diagnostic), and the same emptiness on
/// another fork is a mismatch.
#[test]
fn empty_windows_agree_only_on_a_common_branch() {
    let r = read(101, 150, h(150), vec![], json!([T0]));
    let same = compare(
        &r,
        &fetched(101, 150, Some(150), &[(150, h(150))], vec![]),
        UNKNOWN,
    );
    assert_eq!(same.verdict, Verdict::Equal);
    assert_eq!((same.compared, same.skipped_blocks), (Some((101, 150)), 0));
    let fork = compare(
        &r,
        &fetched(101, 150, Some(150), &[(150, h(151))], vec![]),
        UNKNOWN,
    );
    assert_eq!(fork.verdict, Verdict::BranchMismatch { height: 150 });
}

#[test]
fn conflicts_are_reported_never_deduplicated() {
    let r = read(101, 150, h(150), vec![], json!([T0]));
    let mut f = fetched(101, 150, Some(150), &[(150, h(150))], vec![]);
    f.log_conflicts.insert((130, 2));
    assert_eq!(compare(&r, &f, UNKNOWN).verdict, Verdict::Conflict);
    let mut f = fetched(101, 150, Some(150), &[(150, h(150))], vec![]);
    f.hash_conflicts.insert(140);
    assert_eq!(compare(&r, &f, UNKNOWN).verdict, Verdict::Conflict);
    // Two RPC logs of one block naming different hashes.
    let r = read(
        101,
        150,
        h(150),
        vec![
            rpc_log(120, 0, A, &[T0], Some(h(120))),
            rpc_log(120, 1, A, &[T0], Some(h(121))),
        ],
        json!([T0]),
    );
    let f = fetched(101, 150, Some(150), &[(150, h(150))], vec![]);
    assert_eq!(compare(&r, &f, UNKNOWN).verdict, Verdict::Conflict);
}

#[test]
fn differences_are_counted_by_kind() {
    let both = [T0, T1];
    let mut changed = rpc_log(120, 1, A, &both, Some(h(120)));
    changed.data = "0x01".into();
    let mut r = read(
        101,
        150,
        h(150),
        vec![
            rpc_log(110, 0, A, &both, Some(h(110))),
            changed,
            rpc_log(130, 0, A, &both, Some(h(130))),
        ],
        json!([T0, null]),
    );
    r.times.insert(130, 1);
    let f = fetched(
        101,
        150,
        Some(150),
        &[
            (110, h(110)),
            (120, h(120)),
            (130, h(130)),
            (140, h(140)),
            (145, h(145)),
            (150, h(150)),
        ],
        vec![
            // 110 is missing; 120 differs in data; 130's time differs.
            portal_log(120, 1, A, &both),
            portal_log(130, 0, A, &both),
            // An extra log the filter asks for.
            portal_log(140, 0, A, &both),
            // One topic where the trailing null wants two: the portal query
            // cannot say so, so dropping it locally is expected.
            portal_log(145, 0, A, &[T0]),
            // Another address: drift.
            portal_log(145, 1, B, &both),
        ],
    );
    let out = compare(&r, &f, UNKNOWN);
    assert_eq!(out.verdict, Verdict::Different);
    assert_eq!((out.missing, out.extra, out.mismatch), (1, 1, 1));
    assert_eq!(out.timestamp, 1);
    assert_eq!((out.arity_filtered, out.drift_filtered), (1, 1));
    // Expected local filtering alone never makes a read different.
    let r = read(101, 150, h(150), vec![], json!([T0, null]));
    let f = fetched(
        101,
        150,
        Some(150),
        &[(145, h(145)), (150, h(150))],
        vec![portal_log(145, 0, A, &[T0])],
    );
    let out = compare(&r, &f, UNKNOWN);
    assert_eq!((out.verdict, out.arity_filtered), (Verdict::Equal, 1));
}

#[test]
fn missing_fields_are_reported_not_counted_equal() {
    let mut log = rpc_log(120, 0, A, &[T0], None);
    log.transaction_hash = None;
    let mut r = read(101, 150, h(150), vec![log], json!([T0]));
    r.times.clear();
    let f = fetched(
        101,
        150,
        Some(150),
        &[(120, h(120)), (150, h(150))],
        vec![portal_log(120, 0, A, &[T0])],
    );
    let out = compare(&r, &f, UNKNOWN);
    assert_eq!(
        out.verdict,
        Verdict::Incomplete,
        "never equal with fields not compared"
    );
    assert!(out.incomplete);
    assert_eq!(out.unavailable.get("block_hash"), Some(&1));
    assert_eq!(out.unavailable.get("timestamp"), Some(&1));
    assert_eq!(out.unavailable.get("transaction_hash"), Some(&1));
    // A known difference besides: different and incomplete both.
    let mut r2 = r.clone();
    r2.logs[0].data = "0x01".into();
    let out = compare(&r2, &f, UNKNOWN);
    assert_eq!(out.verdict, Verdict::Different);
    assert!(out.incomplete);
}

/// A block's hash comes from whichever of its RPC logs carries one.
#[test]
fn a_block_hash_is_taken_from_any_log_of_the_block() {
    let r = read(
        101,
        150,
        h(150),
        vec![
            rpc_log(120, 0, A, &[T0], None),
            rpc_log(120, 1, A, &[T0], Some(h(120))),
        ],
        json!([T0]),
    );
    let f = fetched(
        101,
        150,
        Some(150),
        &[(120, h(120)), (150, h(150))],
        vec![portal_log(120, 0, A, &[T0]), portal_log(120, 1, A, &[T0])],
    );
    let out = compare(&r, &f, UNKNOWN);
    assert_eq!(out.verdict, Verdict::Equal, "{out:?}");
    assert!(!out.unavailable.contains_key("block_hash"));
}

/// One log a block over a wide window: compared in one pass per side,
/// never a scan of every log for every block.
#[test]
fn a_dense_sample_compares_in_one_pass() {
    let n = 50_000u64;
    let logs: Vec<RpcLog> = (0..n)
        .map(|i| rpc_log(1_000 + i, 0, A, &[T0], Some(h(1_000 + i))))
        .collect();
    let r = read(1_000, 1_000 + n - 1, h(1_000 + n - 1), logs, json!([T0]));
    let headers: Vec<(u64, String)> = (0..n).map(|i| (1_000 + i, h(1_000 + i))).collect();
    let portal: Vec<PortalLog> = (0..n).map(|i| portal_log(1_000 + i, 0, A, &[T0])).collect();
    let f = fetched(1_000, 1_000 + n - 1, Some(1_000 + n - 1), &headers, portal);
    let started = std::time::Instant::now();
    let out = compare(&r, &f, UNKNOWN);
    assert_eq!(out.verdict, Verdict::Equal);
    assert!(
        started.elapsed() < std::time::Duration::from_secs(5),
        "{:?}",
        started.elapsed()
    );
}

/// A missing constrained position is drift; a missing wildcard is expected.
#[test]
fn only_a_missing_wildcard_position_is_expected() {
    let r = read(101, 150, h(150), vec![], json!([T0, T1]));
    let f = fetched(
        101,
        150,
        Some(150),
        &[(140, h(140)), (150, h(150))],
        vec![portal_log(140, 0, A, &[T0])],
    );
    let out = compare(&r, &f, UNKNOWN);
    assert_eq!((out.arity_filtered, out.drift_filtered), (0, 1));
    assert_eq!(out.verdict, Verdict::Different);
    let r = read(101, 150, h(150), vec![], json!([T0, null]));
    let out = compare(&r, &f, UNKNOWN);
    assert_eq!((out.arity_filtered, out.drift_filtered), (1, 0));
    assert_eq!(out.verdict, Verdict::Equal);
}

#[test]
fn an_offer_is_sized_before_it_is_copied() {
    let r = read(
        101,
        150,
        h(150),
        vec![rpc_log(110, 0, A, &[T0], Some(h(110)))],
        json!([T0]),
    );
    let view = AcceptedView {
        chain_id: r.chain_id,
        from: r.from,
        reached: r.reached,
        reached_hash: &r.reached_hash,
        groups: &r.groups,
        logs: &r.logs,
        times: &r.times,
    };
    assert!(view.bounded_size().is_some());
    let many: Vec<RpcLog> = (0..MAX_OFFER_LOGS as u64 + 1)
        .map(|i| rpc_log(110, i, A, &[T0], None))
        .collect();
    assert!(
        AcceptedView {
            logs: &many,
            ..view
        }
        .bounded_size()
        .is_none()
    );
    let wide = vec![LogGroup {
        addresses: vec![A.to_string(); MAX_OFFER_ADDRESSES + 1],
        topics: json!([]),
    }];
    assert!(
        AcceptedView {
            groups: &wide,
            ..view
        }
        .bounded_size()
        .is_none()
    );
    // Every owned string counts, the optional ones and the filter's topics too.
    let mut fat = r.logs.clone();
    fat[0].transaction_hash = Some("0".repeat(MAX_OFFER_BYTES));
    assert!(AcceptedView { logs: &fat, ..view }.bounded_size().is_none());
    let mut fat = r.logs.clone();
    fat[0].block_hash = Some("0".repeat(MAX_OFFER_BYTES));
    assert!(AcceptedView { logs: &fat, ..view }.bounded_size().is_none());
    let huge_topics = vec![LogGroup {
        addresses: vec![A.to_string()],
        topics: json!(["0".repeat(MAX_OFFER_BYTES)]),
    }];
    assert!(
        AcceptedView {
            groups: &huge_topics,
            ..view
        }
        .bounded_size()
        .is_none()
    );
    let long_hash = "0".repeat(MAX_OFFER_BYTES);
    assert!(
        AcceptedView {
            reached_hash: &long_hash,
            ..view
        }
        .bounded_size()
        .is_none()
    );
}

/// A batch the portal rejects for size is halved and resent; each half
/// advances on its own, and a drain round carries both to the end.
#[tokio::test]
async fn batches_split_on_rejection_and_drain_to_the_end() {
    let (origin, seen) = fake(vec![
        status(
            413,
            "",
            "{\"error\":\"the request body is larger than 1048576 bytes\"}",
        ),
        ok_ndjson(&[
            json!({"header": header_line(110), "logs": [log_line(0, A, &[T0])]}),
            json!({"header": header_line(120)}),
        ]),
        ok_ndjson(&[json!({"header": header_line(137)})]),
        ok_ndjson(&[json!({"header": header_line(150)})]),
        ok_ndjson(&[json!({"header": header_line(150)})]),
    ])
    .await;
    let reader = PortalReader::new(&origin, "polygon-mainnet", None, 100.0).unwrap();
    let filters = filters(&[group(&[A, B], json!([T0]))]).unwrap();
    let f = fetch(&reader, 101, 150, &filters, 2).await;
    assert_eq!(f.stopped, None);
    assert_eq!(f.common_upper, Some(150));
    assert!(
        f.headers.contains_key(&137),
        "a header past the first round's common upper is kept"
    );
    let sends = reader.sends();
    assert_eq!(
        (sends.pages, sends.retries),
        (3, 2),
        "the two halves are resends; the drain is pages"
    );
    let seen = seen.lock().unwrap().clone();
    let q: serde_json::Value = serde_json::from_str(&seen[3].body).unwrap();
    assert_eq!(
        q["fromBlock"],
        json!(121),
        "the first half drains from its own upper"
    );
    let q: serde_json::Value = serde_json::from_str(&seen[4].body).unwrap();
    assert_eq!(q["fromBlock"], json!(138));
}

#[tokio::test]
async fn a_stop_keeps_the_complete_pages_already_read() {
    let (origin, _) = fake(vec![
        ok_ndjson(&[json!({"header": header_line(120)})]),
        status(204, "x-sqd-finalized-head-number: 120\r\n", ""),
    ])
    .await;
    let reader = PortalReader::new(&origin, "polygon-mainnet", None, 100.0).unwrap();
    let f = fetch(
        &reader,
        101,
        150,
        &filters(&[group(&[A], json!([]))]).unwrap(),
        2,
    )
    .await;
    assert_eq!(f.stopped, Some(Stop::AbovePortalFinalized));
    assert_eq!(f.common_upper, Some(120));
    let (origin, _) = fake(vec![status(413, "", "{\"error\":\"too large\"}")]).await;
    let reader = PortalReader::new(&origin, "polygon-mainnet", None, 100.0).unwrap();
    let f = fetch(
        &reader,
        101,
        150,
        &filters(&[group(&[A], json!([T0]))]).unwrap(),
        2,
    )
    .await;
    assert_eq!(f.stopped, Some(Stop::Irreducible));
    assert_eq!(f.common_upper, None);
}
