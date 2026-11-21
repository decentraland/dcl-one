use super::*;
use serde_json::json;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

pub(crate) const A: &str = "0x00000000000000000000000000000000000000a1";
pub(crate) const B: &str = "0x00000000000000000000000000000000000000b2";
pub(crate) const T0: &str = "0x1111111111111111111111111111111111111111111111111111111111111111";
pub(crate) const T1: &str = "0x2222222222222222222222222222222222222222222222222222222222222222";

pub(crate) fn group(addresses: &[&str], topics: Value) -> LogGroup {
    LogGroup {
        addresses: addresses.iter().map(|a| a.to_string()).collect(),
        topics,
    }
}

pub(crate) fn h(n: u64) -> String {
    format!("0x{n:064x}")
}

pub(crate) fn header_line(n: u64) -> Value {
    json!({"number": n, "hash": h(n), "parentHash": h(n.saturating_sub(1)), "timestamp": 1_700_000_000 + n})
}

pub(crate) fn log_line(index: u64, address: &str, topics: &[&str]) -> Value {
    json!({"logIndex": index, "transactionIndex": 0, "address": address, "topics": topics,
           "data": "0x", "transactionHash": h(9_000 + index)})
}

fn lines(blocks: &[Value]) -> Vec<Vec<u8>> {
    blocks.iter().map(|b| b.to_string().into_bytes()).collect()
}

#[test]
fn filters_keep_eth_get_logs_semantics() {
    // [] asks nothing of the topics.
    let f = Filter::from_group(&group(&[A], json!([]))).unwrap();
    assert!(f.matches(A, &[]));
    assert_eq!(f.requests(10)[0].topic0, None);
    // [null] requires a first topic, any value: the portal is asked without
    // topic0, and the log without topics is dropped locally.
    let f = Filter::from_group(&group(&[A], json!([null]))).unwrap();
    assert_eq!(f.requests(10)[0].topic0, None);
    assert!(!f.matches(A, &[]));
    assert!(f.matches(A, &[T0.to_string()]));
    // A trailing null requires the position to exist.
    let f = Filter::from_group(&group(&[A], json!([T0, null]))).unwrap();
    assert!(!f.matches(A, &[T0.to_string()]));
    assert!(f.matches(A, &[T0.to_string(), T1.to_string()]));
    assert!(!f.matches(A, &[T1.to_string(), T0.to_string()]));
    // Alternatives at a position; case does not matter.
    let f = Filter::from_group(&group(&[A], json!([[T0, T1]]))).unwrap();
    assert!(f.matches(
        &A.to_uppercase().replace("0X", "0x"),
        &[T1.to_uppercase().replace("0X", "0x")]
    ));
    assert_eq!(f.requests(10)[0].topic0.as_ref().map(Vec::len), Some(2));
    // Another address never matches.
    assert!(!f.matches(B, &[T0.to_string()]));
    // Refused shapes.
    for topics in [
        json!([[]]),
        json!([T0, []]),
        json!([null, null, null, null, null]),
        json!("x"),
        json!([7]),
    ] {
        assert!(
            matches!(
                Filter::from_group(&group(&[A], topics.clone())),
                Err(PortalError::Unsupported(_))
            ),
            "{topics}"
        );
    }
    assert!(matches!(
        Filter::from_group(&group(&["0xnope"], json!([]))),
        Err(PortalError::Unsupported(_))
    ));
    // An empty address group asks nothing: never a request without addresses.
    let f = Filter::from_group(&group(&[], json!([T0]))).unwrap();
    assert!(f.requests(10).is_empty());
}

#[test]
fn packing_respects_every_limit_and_splitting_halves_the_batch() {
    let addresses: Vec<String> = (0..25_000u64).map(|i| format!("0x{i:040x}")).collect();
    let f = Filter::from_group(&LogGroup {
        addresses: addresses.clone(),
        topics: json!([T0]),
    })
    .unwrap();
    let requests = f.requests(MAX_LIST);
    assert_eq!(requests.len(), 3);
    let batches = pack(1, 2, requests);
    for b in &batches {
        let values: usize = b.iter().map(LogRequest::values).sum();
        assert!(values <= MAX_VALUES, "{values}");
        assert!(b.len() <= MAX_LOG_REQUESTS);
        assert!(query_body(1, 2, b).to_string().len() <= MAX_REQUEST_BYTES);
    }
    assert_eq!(batches.iter().map(Vec::len).sum::<usize>(), 3);
    // 1,500 one-address requests: no batch over 1,000 requests.
    let many: Vec<LogRequest> = (0..1_500u64)
        .map(|i| LogRequest {
            address: vec![format!("0x{i:040x}")],
            topic0: None,
            topic1: None,
            topic2: None,
            topic3: None,
        })
        .collect();
    assert!(
        pack(1, 2, many.clone())
            .iter()
            .all(|b| b.len() <= MAX_LOG_REQUESTS)
    );
    // Halving: the batch by requests, one request by its addresses, then irreducible.
    let (l, r) = split(&many[..4]).unwrap();
    assert_eq!((l.len(), r.len()), (2, 2));
    let one = &f.requests(4)[..1];
    let (l, r) = split(one).unwrap();
    assert_eq!((l[0].address.len(), r[0].address.len()), (2, 2));
    assert!(split(&many[..1]).is_none());
}

#[test]
fn a_page_counts_only_when_complete_and_never_hides_a_conflict() {
    assert!(matches!(decode_page(&[], 1, 10), Err(PortalError::NoUpper)));
    let page = decode_page(
        &lines(&[
            json!({"header": header_line(3), "logs": [log_line(0, A, &[T0]), log_line(0, A, &[T0]), log_line(1, A, &[T0])]}),
            json!({"header": header_line(5), "logs": [log_line(0, A, &[T0]), log_line(0, B, &[T0])]}),
            json!({"header": header_line(8)}),
        ]),
        1,
        10,
    )
    .unwrap();
    assert_eq!(page.upper.number, 8, "the last line is the block covered");
    assert_eq!(page.headers.len(), 3);
    // An identical repeat is one log; two different logs at (5, 0) are a conflict.
    assert_eq!(
        page.logs
            .iter()
            .map(|l| (l.block, l.log_index))
            .collect::<Vec<_>>(),
        [(3, 0), (3, 1)]
    );
    assert_eq!(page.conflicts, [(5, 0)]);
    for bad in [
        vec![
            json!({"header": header_line(5)}),
            json!({"header": header_line(5)}),
        ],
        vec![json!({"header": header_line(11)})],
        vec![json!({"header": header_line(0)})],
        vec![json!({"header": {"number": 4, "hash": h(4)}})],
        vec![json!({"header": header_line(4), "logs": [{"logIndex": 0}]})],
        vec![json!({"header": header_line(4), "logs": [log_line(0, A, &[T0, T0, T0, T0, T0])]})],
        vec![json!({"header": header_line(4), "logs": {}})],
    ] {
        assert!(
            matches!(
                decode_page(&lines(&bad), 1, 10),
                Err(PortalError::Malformed(_))
            ),
            "{bad:?}"
        );
    }
    assert!(matches!(
        decode_page(&[b"{\"header\":".to_vec()], 1, 10),
        Err(PortalError::Malformed(_))
    ));
}

#[test]
fn origins_are_bare_and_secrets_never_print() {
    assert_eq!(
        parse_origin("https://portal.example/").unwrap(),
        "https://portal.example"
    );
    assert!(
        parse_origin("http://portal.example").is_err(),
        "https off loopback"
    );
    assert!(parse_origin("http://127.0.0.1:9").is_ok());
    for bad in [
        "https://portal.example/x",
        "https://u:p@portal.example",
        "https://portal.example?k=1",
        "ftp://x",
    ] {
        assert!(parse_origin(bad).is_err(), "{bad}");
    }
    let key = Secret::new("sk-example123").unwrap();
    assert_eq!(format!("{key:?}"), "<redacted>");
    let reader =
        PortalReader::new("https://portal.example", "polygon-mainnet", Some(key), 1.0).unwrap();
    assert!(!format!("{reader:?}").contains("sk-example123"));
    assert!(PortalReader::new("https://portal.example", "base-mainnet", None, 1.0).is_err());
    assert!(Secret::new("  ").is_none());
}

#[test]
fn readers_of_one_origin_and_credential_share_their_limiter() {
    let key = Secret::new("sk-example-shared").unwrap();
    let a = OriginLimiter::shared("https://shared.example", Some(&key), 1.0);
    let b = OriginLimiter::shared("https://shared.example", Some(&key), 5.0);
    let other = OriginLimiter::shared("https://shared.example", None, 1.0);
    assert!(Arc::ptr_eq(&a, &b));
    assert!(!Arc::ptr_eq(&a, &other));
    a.cool(Some(Duration::from_secs(1)));
    let until = b.state.lock().unwrap().cooldown_until.unwrap();
    assert!(
        until >= Instant::now() + Duration::from_secs(9),
        "at least the minimum cooldown"
    );
    // A shorter hold never shortens a longer one.
    b.cool(None);
    assert!(b.state.lock().unwrap().cooldown_until.unwrap() >= until);
}

/// One request a fake portal received.
#[derive(Clone, Debug)]
pub(crate) struct Seen {
    pub(crate) head: String,
    pub(crate) body: String,
}

/// A loopback server answering each request with the next canned raw
/// response, recording what it received.
pub(crate) async fn fake(responses: Vec<String>) -> (String, Arc<Mutex<Vec<Seen>>>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let seen: Arc<Mutex<Vec<Seen>>> = Arc::default();
    let log = seen.clone();
    tokio::spawn(async move {
        for response in responses {
            let Ok((mut sock, _)) = listener.accept().await else {
                return;
            };
            let mut buf = Vec::new();
            let mut tmp = [0u8; 4096];
            let head_end = loop {
                let n = sock.read(&mut tmp).await.unwrap_or(0);
                if n == 0 {
                    break None;
                }
                buf.extend_from_slice(&tmp[..n]);
                if let Some(i) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
                    break Some(i + 4);
                }
            };
            let Some(head_end) = head_end else { continue };
            let head = String::from_utf8_lossy(&buf[..head_end]).to_string();
            let len = head
                .lines()
                .find_map(|l| {
                    l.to_ascii_lowercase()
                        .strip_prefix("content-length:")
                        .map(|v| v.trim().parse::<usize>().unwrap_or(0))
                })
                .unwrap_or(0);
            while buf.len() < head_end + len {
                let n = sock.read(&mut tmp).await.unwrap_or(0);
                if n == 0 {
                    break;
                }
                buf.extend_from_slice(&tmp[..n]);
            }
            let body = String::from_utf8_lossy(&buf[head_end..]).to_string();
            log.lock().unwrap().push(Seen { head, body });
            let _ = sock.write_all(response.as_bytes()).await;
            let _ = sock.shutdown().await;
        }
    });
    (format!("http://127.0.0.1:{}", addr.port()), seen)
}

pub(crate) fn ok_ndjson(blocks: &[Value]) -> String {
    let body: String = blocks.iter().map(|b| format!("{b}\n")).collect();
    format!(
        "HTTP/1.1 200 OK\r\ncontent-type: application/x-ndjson\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
        body.len()
    )
}

pub(crate) fn status(code: u16, extra: &str, body: &str) -> String {
    format!(
        "HTTP/1.1 {code} X\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n{extra}\r\n{body}",
        body.len()
    )
}

#[tokio::test]
async fn the_reader_speaks_the_protocol_and_counts_every_send() {
    let (origin, seen) = fake(vec![
        status(200, "", &json!({"number": 50, "hash": h(50)}).to_string()),
        status(200, "", &json!({"number": 40, "hash": h(40)}).to_string()),
        status(404, "", "{\"error\":\"not found\"}"),
        ok_ndjson(&[
            json!({"header": header_line(12), "logs": [log_line(3, A, &[T0])]}),
            json!({"header": header_line(20)}),
        ]),
        status(204, "x-sqd-finalized-head-number: 40\r\n", ""),
        ok_ndjson(&[]),
        ok_ndjson(&[json!({"header": header_line(7)})]),
        status(429, "retry-after: 1\r\n", "{\"error\":\"slow down\"}"),
    ])
    .await;
    let key = Secret::new("sk-example-proto").unwrap();
    let r = PortalReader::new(&origin, "polygon-mainnet", Some(key), 100.0).unwrap();
    assert_eq!(
        r.latest().await.unwrap(),
        Head {
            number: 50,
            hash: h(50)
        }
    );
    assert_eq!(r.finalized().await.unwrap().number, 40);
    assert_eq!(
        r.advertised_start().await.unwrap(),
        None,
        "chain-lake publishes no metadata"
    );
    let f = Filter::from_group(&group(&[A], json!([T0]))).unwrap();
    let page = r.page(10, 30, &f.requests(MAX_LIST), false).await.unwrap();
    assert_eq!(
        (page.upper.number, page.logs.len()),
        (20, 1),
        "an early end at 20"
    );
    assert!(matches!(
        r.page(41, 60, &f.requests(MAX_LIST), false).await,
        Err(PortalError::AboveHead {
            finalized: Some(40)
        })
    ));
    assert!(matches!(
        r.page(10, 30, &f.requests(MAX_LIST), true).await,
        Err(PortalError::NoUpper)
    ));
    assert_eq!(r.header(7).await.unwrap().number, 7);
    assert!(matches!(
        r.header(8).await,
        Err(PortalError::Unavailable { status: 429, .. })
    ));
    assert_eq!(
        r.sends(),
        SendCounts {
            admission: 3,
            pages: 2,
            retries: 1,
            headers: 2
        },
        "refused sends count too"
    );
    let seen = seen.lock().unwrap().clone();
    assert!(
        seen[0]
            .head
            .starts_with("GET /datasets/polygon-mainnet/head ")
    );
    assert!(
        seen[3]
            .head
            .starts_with("POST /datasets/polygon-mainnet/finalized-stream ")
    );
    assert!(seen.iter().all(|s| {
        s.head
            .to_ascii_lowercase()
            .contains("x-api-key: sk-example-proto")
    }));
    let q: Value = serde_json::from_str(&seen[3].body).unwrap();
    assert_eq!(
        (q["fromBlock"].as_u64(), q["toBlock"].as_u64()),
        (Some(10), Some(30))
    );
    assert_eq!(q["logs"][0]["address"], json!([A]));
    assert_eq!(q["logs"][0]["topic0"], json!([T0]));
    assert!(q["logs"][0].get("topic1").is_none());
}

#[tokio::test]
async fn a_cut_body_never_counts_and_a_redirect_is_never_followed() {
    let (elsewhere, elsewhere_seen) =
        fake(vec![ok_ndjson(&[json!({"header": header_line(5)})])]).await;
    let line = format!("{}\n", json!({"header": header_line(5)}));
    let cut = format!(
        "HTTP/1.1 200 OK\r\ncontent-type: application/x-ndjson\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{line}{{\"header\":",
        line.len() + 500
    );
    let (origin, _) = fake(vec![
        cut,
        status(
            302,
            &format!("location: {elsewhere}/datasets/polygon-mainnet/finalized-stream\r\n"),
            "",
        ),
        status(
            413,
            "",
            "{\"error\":\"the request body is larger than 1048576 bytes\"}",
        ),
    ])
    .await;
    let r = PortalReader::new(
        &origin,
        "polygon-mainnet",
        Secret::new("sk-example-redirect"),
        100.0,
    )
    .unwrap();
    assert!(matches!(
        r.page(1, 9, &[], false).await,
        Err(PortalError::Truncated)
    ));
    assert!(matches!(
        r.page(1, 9, &[], false).await,
        Err(PortalError::Unavailable { status: 302, .. })
    ));
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert!(
        elsewhere_seen.lock().unwrap().is_empty(),
        "the credential never left its origin"
    );
    let err = r.page(1, 9, &[], false).await.unwrap_err();
    assert!(err.is_size_rejection(), "{err}");
}

#[tokio::test]
async fn metadata_bounds_the_dataset_only() {
    let (origin, _) = fake(vec![
        status(
            200,
            "",
            &json!({"dataset": "ethereum-mainnet", "start_block": 0, "real_time": true})
                .to_string(),
        ),
        status(200, "", &json!({"dataset": "ethereum-mainnet"}).to_string()),
    ])
    .await;
    let r = PortalReader::new(&origin, "ethereum-mainnet", None, 100.0).unwrap();
    assert_eq!(r.advertised_start().await.unwrap(), Some(0));
    assert_eq!(r.advertised_start().await.unwrap(), None);
}
