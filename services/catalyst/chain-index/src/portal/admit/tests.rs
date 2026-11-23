use super::*;
use crate::portal::tests::{A, B, T0, T1, group, h};
use serde_json::{Value, json};
use std::collections::VecDeque;
use std::sync::atomic::AtomicBool;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

const X: &str = B;
const R1: &str = "1111111111111111111111111111111111111111111111111111111111111111";
const R2: &str = "2222222222222222222222222222222222222222222222222222222222222222";
const R9: &str = "9999999999999999999999999999999999999999999999999999999999999999";

fn time_of(n: u64) -> u64 {
    1_700_000_000 + n
}

/// A hash no RPC block carries: the lake's own branch.
fn other_branch(n: u64) -> String {
    format!("0x{:064x}", 0xa000_0000_u64 + n)
}

/// A log the fake lake holds: (block, index, address, topics).
type LakeLog = (u64, u64, String, Vec<String>);

/// What the fake lake holds and states.
#[derive(Clone)]
struct Lake {
    revision: &'static str,
    finalized: Option<u64>,
    finalized_hash: Option<String>,
    from: Vec<(String, u64)>,
    logs: Vec<LakeLog>,
    /// Pages end at most here.
    page_end: Option<u64>,
}

impl Lake {
    fn new(revision: &'static str, finalized: u64, from: &[(&str, u64)]) -> Self {
        Self {
            revision,
            finalized: Some(finalized),
            finalized_hash: None,
            from: from.iter().map(|(a, b)| (a.to_string(), *b)).collect(),
            logs: Vec::new(),
            page_end: None,
        }
    }

    fn log(mut self, block: u64, index: u64, address: &str, topics: &[&str]) -> Self {
        self.logs.push((
            block,
            index,
            address.to_string(),
            topics.iter().map(|t| t.to_string()).collect(),
        ));
        self
    }
}

#[derive(Default)]
struct Portal {
    lake: Mutex<Option<Lake>>,
    /// Once this many pages were served, the next page and every answer
    /// after it are this lake's.
    swap: Mutex<Option<(usize, Lake)>>,
    pages: AtomicUsize,
    /// Every page names this revision instead of the lake's.
    page_revision: Mutex<Option<&'static str>>,
    /// The coverage answer as raw HTTP, instead of the lake's.
    contracts_reply: Mutex<Option<String>>,
    stall_contracts: AtomicBool,
    stall_pages: AtomicBool,
    /// The page at this count (from one) waits for a permit of this gate.
    gate: Mutex<Option<(usize, Arc<tokio::sync::Semaphore>)>>,
    /// This page (counted from one) gives every block another time.
    skew_page: Mutex<Option<usize>>,
    fail_pages: AtomicBool,
    /// What a failed page answers, with its headers; a 500 unless set.
    fail_with: Mutex<Option<(u16, &'static str)>>,
    /// Logs every page carries, whatever it asked.
    inject: Mutex<Vec<LakeLog>>,
    seen: Mutex<Vec<String>>,
}

fn http(status: u16, headers: &str, body: &str) -> String {
    format!(
        "HTTP/1.1 {status} X\r\ncontent-length: {}\r\nconnection: close\r\n{headers}\r\n{body}",
        body.len()
    )
}

fn wanted(requests: &[Value], address: &str, topics: &[String]) -> bool {
    requests.iter().any(|r| {
        r["address"]
            .as_array()
            .is_some_and(|a| a.iter().any(|x| x.as_str() == Some(address)))
            && (0..4).all(|i| match r.get(format!("topic{i}")) {
                None => true,
                Some(alts) => topics.get(i).is_some_and(|t| {
                    alts.as_array()
                        .is_some_and(|a| a.iter().any(|x| x.as_str() == Some(t.as_str())))
                }),
            })
    })
}

impl Portal {
    fn page_count(&self) -> usize {
        self.seen
            .lock()
            .unwrap()
            .iter()
            .filter(|p| p.ends_with("/finalized-stream"))
            .count()
    }

    async fn reply(&self, path: &str, body: &str) -> String {
        self.seen.lock().unwrap().push(path.to_string());
        if path.ends_with("/contracts") {
            if self.stall_contracts.load(Ordering::SeqCst) {
                tokio::time::sleep(Duration::from_secs(60)).await;
            }
            if let Some(r) = self.contracts_reply.lock().unwrap().clone() {
                return r;
            }
            let lake = self.lake.lock().unwrap().clone().unwrap();
            let answer = json!({
                "schemaVersion": 1, "dataset": "polygon-mainnet", "chainId": 137,
                "coverageRevision": lake.revision,
                "finalized": lake.finalized.map(|n| json!({
                    "number": n,
                    "hash": lake.finalized_hash.clone().unwrap_or_else(|| h(n)),
                })),
                "contracts": lake.from.iter().map(|(a, f)| json!({"address": a, "from": f})).collect::<Vec<_>>(),
            });
            return http(
                200,
                "content-type: application/json\r\n",
                &answer.to_string(),
            );
        }
        if self.stall_pages.load(Ordering::SeqCst) {
            tokio::time::sleep(Duration::from_secs(60)).await;
        }
        let gate = self.gate.lock().unwrap().clone();
        if let Some((_, gate)) = gate.filter(|(at, _)| self.page_count() == *at) {
            let _ = gate.acquire().await;
        }
        if self.fail_pages.load(Ordering::SeqCst) {
            let (status, headers) = self.fail_with.lock().unwrap().unwrap_or((500, ""));
            return http(status, headers, "");
        }
        let served = self.pages.fetch_add(1, Ordering::SeqCst) + 1;
        {
            let mut swap = self.swap.lock().unwrap();
            if swap.as_ref().is_some_and(|(after, _)| served > *after) {
                let (_, next) = swap.take().unwrap();
                *self.lake.lock().unwrap() = Some(next);
            }
        }
        let lake = self.lake.lock().unwrap().clone().unwrap();
        let skew = u64::from(*self.skew_page.lock().unwrap() == Some(served));
        let q: Value = serde_json::from_str(body).unwrap();
        let from = q["fromBlock"].as_u64().unwrap();
        let mut to = q["toBlock"].as_u64().unwrap();
        if let Some(end) = lake.page_end {
            to = to.min(end);
        }
        let requests = q["logs"].as_array().cloned().unwrap_or_default();
        let mut blocks: std::collections::BTreeMap<u64, Vec<Value>> = Default::default();
        let injected = self.inject.lock().unwrap().clone();
        let held = lake
            .logs
            .iter()
            .filter(|(b, _, a, t)| *b >= from && *b <= to && wanted(&requests, a, t));
        let extra = injected.iter().filter(|(b, ..)| *b >= from && *b <= to);
        for (b, i, a, t) in held.chain(extra) {
            blocks.entry(*b).or_default().push(json!({
                "logIndex": i, "transactionIndex": 0, "address": a, "topics": t,
                "data": "0x", "transactionHash": h(9_000 + b * 10 + i),
            }));
        }
        blocks.entry(to).or_default();
        let body: String = blocks
            .into_iter()
            .map(|(b, logs)| {
                let mut line = json!({"header": {
                    "number": b, "hash": h(b), "parentHash": h(b - 1), "timestamp": time_of(b) + skew,
                }});
                if !logs.is_empty() {
                    line["logs"] = json!(logs);
                }
                format!("{line}\n")
            })
            .collect();
        let revision = self.page_revision.lock().unwrap().unwrap_or(lake.revision);
        http(
            200,
            &format!(
                "content-type: application/x-ndjson\r\nx-lake-coverage-revision: {revision}\r\n"
            ),
            &body,
        )
    }
}

async fn serve(portal: Arc<Portal>) -> String {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        loop {
            let Ok((mut sock, _)) = listener.accept().await else {
                return;
            };
            let portal = portal.clone();
            tokio::spawn(async move {
                let mut buf = Vec::new();
                let mut tmp = [0u8; 65536];
                let head_end = loop {
                    let n = sock.read(&mut tmp).await.unwrap_or(0);
                    if n == 0 {
                        return;
                    }
                    buf.extend_from_slice(&tmp[..n]);
                    if let Some(i) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
                        break i + 4;
                    }
                };
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
                let path = head.split_whitespace().nth(1).unwrap_or("").to_string();
                let body = String::from_utf8_lossy(&buf[head_end..]).to_string();
                let reply = portal.reply(&path, &body).await;
                let _ = sock.write_all(reply.as_bytes()).await;
                let _ = sock.shutdown().await;
            });
        }
    });
    format!("http://127.0.0.1:{}", addr.port())
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Call {
    Header(u64),
    Window(u64, u64, Vec<String>),
}

/// The walk's RPC: canonical hashes `h(n)`, its own logs, and every call
/// recorded.
#[derive(Default)]
struct FakeRpc {
    /// The chain it serves; 137 when unset.
    chain: u64,
    /// Headers on another branch than `h(n)`.
    branch: HashMap<u64, String>,
    logs: Vec<RpcLog>,
    header_delay: HashMap<u64, Duration>,
    /// Each window reaches at most the next of these (then `to`).
    caps: Mutex<VecDeque<u64>>,
    fail_windows: AtomicBool,
    calls: Mutex<Vec<Call>>,
}

fn rpc_log(block: u64, index: u64, address: &str, topics: &[&str]) -> RpcLog {
    RpcLog {
        address: address.to_string(),
        topics: topics.iter().map(|t| t.to_string()).collect(),
        data: "0x".into(),
        block_number: format!("0x{block:x}"),
        log_index: format!("0x{index:x}"),
        block_timestamp: Some(format!("0x{:x}", time_of(block))),
        block_hash: Some(h(block)),
        transaction_hash: Some(h(9_000 + block * 10 + index)),
    }
}

impl FakeRpc {
    fn calls(&self) -> Vec<Call> {
        self.calls.lock().unwrap().clone()
    }

    fn windows(&self) -> Vec<Call> {
        self.calls()
            .into_iter()
            .filter(|c| matches!(c, Call::Window(..)))
            .collect()
    }

    fn headers(&self) -> Vec<u64> {
        self.calls()
            .into_iter()
            .filter_map(|c| match c {
                Call::Header(n) => Some(n),
                _ => None,
            })
            .collect()
    }
}

#[async_trait::async_trait]
impl LogSource for &FakeRpc {
    async fn chain_id(&self) -> Result<Read<u64>, RpcError> {
        Ok(Read {
            value: if self.chain == 0 { 137 } else { self.chain },
            requests: 1,
        })
    }
    async fn head(&self) -> Result<Read<u64>, RpcError> {
        Ok(Read {
            value: 1_000,
            requests: 1,
        })
    }
    async fn finalized(&self) -> Result<Read<BlockHeader>, RpcError> {
        self.header(1_000).await
    }
    async fn header(&self, number: u64) -> Result<Read<BlockHeader>, RpcError> {
        self.calls.lock().unwrap().push(Call::Header(number));
        if let Some(d) = self.header_delay.get(&number) {
            tokio::time::sleep(*d).await;
        }
        Ok(Read {
            value: BlockHeader {
                number,
                hash: self
                    .branch
                    .get(&number)
                    .cloned()
                    .unwrap_or_else(|| h(number)),
                timestamp: time_of(number) as i64,
            },
            requests: 1,
        })
    }
    async fn window(
        &self,
        from: u64,
        to: u64,
        groups: &[LogGroup],
        chunk: &mut usize,
    ) -> Result<Window, RpcError> {
        let addresses: Vec<String> = groups.iter().flat_map(|g| g.addresses.clone()).collect();
        self.calls
            .lock()
            .unwrap()
            .push(Call::Window(from, to, addresses));
        if self.fail_windows.load(Ordering::SeqCst) {
            return Err(RpcError::Rpc("down".into()));
        }
        let reached = self.caps.lock().unwrap().pop_front().unwrap_or(to).min(to);
        let filters = compare::filters(groups).unwrap();
        let logs: Vec<RpcLog> = self
            .logs
            .iter()
            .filter(|l| {
                let b = log_position(l).0 as u64;
                b >= from
                    && b <= reached
                    && filters.iter().any(|f| f.matches(&l.address, &l.topics))
            })
            .cloned()
            .collect();
        let requests = requests_for(groups, *chunk);
        Ok(Window {
            logs,
            reached,
            requests,
            charge: requests,
            portal_sends: 0,
        })
    }
    async fn block_times(&self, _logs: &[RpcLog]) -> Result<Read<HashMap<i64, i64>>, RpcError> {
        Ok(Read {
            value: HashMap::new(),
            requests: 0,
        })
    }
}

const DEADLINE: Duration = Duration::from_millis(400);

async fn lake_at(lake: Lake) -> (Arc<Portal>, String) {
    let portal = Arc::new(Portal::default());
    *portal.lake.lock().unwrap() = Some(lake);
    let origin = serve(portal.clone()).await;
    (portal, origin)
}

fn admission(origin: &str, rps: f64, cooldown: Duration) -> Admission {
    Admission::with_cooldown(
        PortalReader::new(origin, "polygon-mainnet", None, rps).unwrap(),
        "matic",
        DEADLINE,
        cooldown,
    )
}

/// One window from 100 to 200 of a source whose walk read its chain id.
async fn window_of(rpc: &FakeRpc, adm: &Admission, groups: &[LogGroup]) -> Window {
    verified(rpc, adm)
        .await
        .window(100, 200, groups, &mut 1_000)
        .await
        .unwrap()
}

/// A source whose walk read its chain id through it, as a pass does first.
async fn verified<'a>(rpc: &'a FakeRpc, adm: &'a Admission) -> AdmittedPortal<'a, &'a FakeRpc> {
    let source = AdmittedPortal::new(rpc, Some(adm));
    source.chain_id().await.unwrap();
    source
}

fn positions(w: &Window) -> Vec<(i64, i64)> {
    w.logs.iter().map(log_position).collect()
}

#[tokio::test]
async fn a_covered_window_reads_its_logs_from_the_portal_and_one_anchor_from_the_rpc() {
    let (portal, origin) = lake_at(Lake::new(R1, 150, &[(A, 0)]).log(115, 0, A, &[T0])).await;
    let adm = admission(&origin, 1_000.0, COOLDOWN);
    let rpc = FakeRpc::default();
    let source = verified(&rpc, &adm).await;
    let mut chunk = 1_000;
    let w = source
        .window(100, 200, &[group(&[A], json!([T0]))], &mut chunk)
        .await
        .unwrap();
    assert_eq!(w.reached, 150, "clamped to the answer's finalized block");
    assert_eq!(positions(&w), [(115, 0)]);
    let log = &w.logs[0];
    assert_eq!(log.block_hash.as_deref(), Some(h(115).as_str()));
    assert_eq!(log.block_timestamp, Some(format!("0x{:x}", time_of(115))));
    assert_eq!(
        rpc.calls(),
        [Call::Header(150)],
        "the anchor, no RPC log read"
    );
    assert_eq!((w.requests, w.charge, w.portal_sends), (1, 1, 2));
    assert_eq!(portal.page_count(), 1);
}

/// The lake on another branch answers an empty page whose last header is
/// the RPC's own (a header the lake took from an RPC of its own): only the
/// durable anchor at its finalized block, 150, tells, and the event at 115
/// it never held is read from the RPC.
#[tokio::test]
async fn the_durable_anchor_not_the_page_upper_binds_the_lake_to_the_rpc() {
    for (lake_hash, from_portal) in [(other_branch(150), false), (h(150), true)] {
        let mut lake = Lake::new(R1, 150, &[(A, 0)]);
        lake.finalized_hash = Some(lake_hash);
        lake.page_end = Some(120);
        let (_portal, origin) = lake_at(lake).await;
        let adm = admission(&origin, 1_000.0, COOLDOWN);
        let rpc = FakeRpc {
            logs: vec![rpc_log(115, 0, A, &[T0])],
            ..Default::default()
        };
        let source = verified(&rpc, &adm).await;
        let mut chunk = 1_000;
        let w = source
            .window(100, 200, &[group(&[A], json!([T0]))], &mut chunk)
            .await
            .unwrap();
        assert_eq!(
            rpc.headers(),
            [150],
            "the anchor is the answer's finalized block"
        );
        if from_portal {
            assert_eq!((w.reached, positions(&w)), (120, vec![]));
            assert!(rpc.windows().is_empty());
        } else {
            assert_eq!((w.reached, positions(&w)), (200, vec![(115, 0)]));
            assert_eq!(rpc.windows(), [Call::Window(100, 200, vec![A.into()])]);
        }
    }
}

#[tokio::test]
async fn the_anchor_read_falls_inside_the_deadline_including_on_readmission() {
    // The anchor itself stalls.
    let (_portal, origin) = lake_at(Lake::new(R1, 150, &[(A, 0)])).await;
    let adm = admission(&origin, 1_000.0, COOLDOWN);
    let rpc = FakeRpc {
        header_delay: HashMap::from([(150, Duration::from_secs(30))]),
        ..Default::default()
    };
    let started = std::time::Instant::now();
    let w = verified(&rpc, &adm)
        .await
        .window(100, 200, &[group(&[A], json!([T0]))], &mut 1_000)
        .await
        .unwrap();
    assert!(started.elapsed() < DEADLINE * 3, "{:?}", started.elapsed());
    assert_eq!(w.reached, 200);
    assert_eq!(
        rpc.calls(),
        [Call::Header(150), Call::Window(100, 200, vec![A.into()])]
    );
    // The anchor the RPC received and held past the deadline is counted.
    assert_eq!(w.requests, rpc.calls().len());

    // The answer moves to a lower block between two pages: the new anchor
    // is read, inside the same deadline, and stalls.
    let (portal, origin) = lake_at(Lake::new(R1, 150, &[(A, 0)])).await;
    *portal.swap.lock().unwrap() = Some((0, Lake::new(R2, 140, &[(A, 0)])));
    let adm = admission(&origin, 1_000.0, COOLDOWN);
    let rpc = FakeRpc {
        header_delay: HashMap::from([(140, Duration::from_secs(30))]),
        ..Default::default()
    };
    let started = std::time::Instant::now();
    let w = verified(&rpc, &adm)
        .await
        .window(100, 200, &[group(&[A], json!([T0]))], &mut 1_000)
        .await
        .unwrap();
    assert!(started.elapsed() < DEADLINE * 3, "{:?}", started.elapsed());
    assert_eq!(w.reached, 200);
    assert_eq!(rpc.headers(), [150, 140]);
    assert_eq!(rpc.windows(), [Call::Window(100, 200, vec![A.into()])]);
}

/// Two batches (two filters of ten thousand addresses each). The second
/// page names a revision that no longer lists X and stops at 140: every
/// page is dropped, the window is admitted again from the original filters,
/// and X is read from the RPC.
#[tokio::test]
async fn a_revision_change_on_the_second_batch_admits_the_window_again() {
    let fill =
        |base: u64| -> Vec<String> { (0..9_999).map(|i| format!("0x{:040x}", base + i)).collect() };
    let (mut first, mut second) = (fill(0x10_0000), fill(0x20_0000));
    first.push(A.into());
    second.push(X.into());
    let every: Vec<(String, u64)> = first
        .iter()
        .chain(&second)
        .map(|a| (a.clone(), 0))
        .collect();
    let mut before = Lake::new(R1, 150, &[]);
    before.from = every.clone();
    let before = before.log(110, 0, A, &[T0]).log(130, 0, X, &[T1]);
    let mut after = Lake::new(R2, 140, &[]);
    after.from = every.into_iter().filter(|(a, _)| a != X).collect();
    let after = after.log(110, 0, A, &[T0]);
    let (portal, origin) = lake_at(before).await;
    *portal.swap.lock().unwrap() = Some((1, after));
    let adm = admission(&origin, 1_000.0, COOLDOWN);
    let rpc = FakeRpc {
        logs: vec![rpc_log(130, 0, X, &[T1])],
        ..Default::default()
    };
    let groups = [
        LogGroup {
            addresses: first,
            topics: json!([T0]),
        },
        LogGroup {
            addresses: second,
            topics: json!([T1]),
        },
    ];
    let w = verified(&rpc, &adm)
        .await
        .window(100, 200, &groups, &mut 1_000)
        .await
        .unwrap();
    assert_eq!(w.reached, 140, "the new answer's finalized block");
    assert_eq!(positions(&w), [(110, 0), (130, 0)], "X's log, from the RPC");
    assert_eq!(rpc.headers(), [150, 140], "each revision's own anchor");
    assert_eq!(rpc.windows(), [Call::Window(100, 140, vec![X.into()])]);
    assert_eq!(portal.page_count(), 4, "two batches, then two again");
}

/// The portal reaches 120 with a log at 115; the RPC, for the address not
/// admitted, reaches 110: the window ends at 110 without the 115 log, and
/// the next window delivers it, once.
#[tokio::test]
async fn a_window_reaches_the_lower_end_and_the_next_one_delivers_the_rest_once() {
    let mut lake = Lake::new(R1, 150, &[(A, 0)]).log(115, 0, A, &[T0]);
    lake.page_end = Some(120);
    let (_portal, origin) = lake_at(lake).await;
    let adm = admission(&origin, 1_000.0, COOLDOWN);
    let rpc = FakeRpc {
        caps: Mutex::new(VecDeque::from([110])),
        ..Default::default()
    };
    let source = verified(&rpc, &adm).await;
    let groups = [group(&[A, X], json!([T0]))];
    let w = source.window(100, 200, &groups, &mut 1_000).await.unwrap();
    assert_eq!((w.reached, positions(&w)), (110, vec![]));
    let w = source.window(111, 200, &groups, &mut 1_000).await.unwrap();
    assert_eq!((w.reached, positions(&w)), (120, vec![(115, 0)]));
    assert_eq!(
        rpc.windows(),
        [
            Call::Window(100, 120, vec![X.into()]),
            Call::Window(111, 120, vec![X.into()])
        ]
    );
}

#[tokio::test]
async fn pages_naming_another_revision_twice_read_the_window_from_the_rpc() {
    let (portal, origin) = lake_at(Lake::new(R1, 150, &[(A, 0)]).log(115, 0, A, &[T0])).await;
    *portal.page_revision.lock().unwrap() = Some(R9);
    let adm = admission(&origin, 1_000.0, COOLDOWN);
    let rpc = FakeRpc {
        logs: vec![rpc_log(116, 0, A, &[T0])],
        ..Default::default()
    };
    let w = verified(&rpc, &adm)
        .await
        .window(100, 200, &[group(&[A], json!([T0]))], &mut 1_000)
        .await
        .unwrap();
    assert_eq!(
        (w.reached, positions(&w)),
        (200, vec![(116, 0)]),
        "no portal log"
    );
    assert_eq!(rpc.headers(), [150, 150]);
    assert_eq!(portal.page_count(), 2);
}

#[tokio::test]
async fn a_log_the_filters_do_not_keep_is_drift_and_an_arity_only_log_is_dropped() {
    // Drift: a log of A with another first topic.
    let (portal, origin) = lake_at(Lake::new(R1, 150, &[(A, 0)])).await;
    portal
        .inject
        .lock()
        .unwrap()
        .push((115, 1, A.into(), vec![T1.into()]));
    let adm = admission(&origin, 1_000.0, COOLDOWN);
    let rpc = FakeRpc::default();
    let w = verified(&rpc, &adm)
        .await
        .window(100, 200, &[group(&[A], json!([T0]))], &mut 1_000)
        .await
        .unwrap();
    assert_eq!((w.reached, positions(&w)), (200, vec![]));
    assert_eq!(rpc.windows().len(), 1, "read from the RPC");

    // `[T0, null]` needs a second topic: the portal, asked T0 alone, also
    // returns the one-topic log, which eth_getLogs would not.
    let lake = Lake::new(R1, 150, &[(A, 0)])
        .log(115, 0, A, &[T0])
        .log(116, 0, A, &[T0, T1]);
    let (_portal, origin) = lake_at(lake).await;
    let adm = admission(&origin, 1_000.0, COOLDOWN);
    let rpc = FakeRpc::default();
    let w = verified(&rpc, &adm)
        .await
        .window(100, 200, &[group(&[A], json!([T0, null]))], &mut 1_000)
        .await
        .unwrap();
    assert_eq!((w.reached, positions(&w)), (150, vec![(116, 0)]));
    assert!(rpc.windows().is_empty());
}

#[tokio::test]
async fn an_rpc_failure_after_a_portal_failure_fails_the_window() {
    let (portal, origin) = lake_at(Lake::new(R1, 150, &[(A, 0)])).await;
    portal.fail_pages.store(true, Ordering::SeqCst);
    let adm = admission(&origin, 1_000.0, COOLDOWN);
    let rpc = FakeRpc::default();
    rpc.fail_windows.store(true, Ordering::SeqCst);
    let r = verified(&rpc, &adm)
        .await
        .window(100, 200, &[group(&[A], json!([T0]))], &mut 1_000)
        .await;
    assert!(r.is_err());
}

#[tokio::test]
async fn a_stalled_coverage_read_ends_at_the_deadline_and_the_rpc_window_advances() {
    let (portal, origin) = lake_at(Lake::new(R1, 150, &[(A, 0)])).await;
    portal.stall_contracts.store(true, Ordering::SeqCst);
    let adm = admission(&origin, 1_000.0, COOLDOWN);
    let rpc = FakeRpc {
        logs: vec![rpc_log(115, 0, A, &[T0])],
        ..Default::default()
    };
    let started = std::time::Instant::now();
    let w = verified(&rpc, &adm)
        .await
        .window(100, 200, &[group(&[A], json!([T0]))], &mut 1_000)
        .await
        .unwrap();
    assert!(started.elapsed() < DEADLINE * 3, "{:?}", started.elapsed());
    assert_eq!((w.reached, positions(&w)), (200, vec![(115, 0)]));
    assert!(rpc.headers().is_empty(), "no anchor without an answer");
}

/// One token every hundred seconds: the first walk takes it for the
/// answer, and every page waits on the limiter. Three walks at once all
/// read from the RPC within the deadline.
#[tokio::test]
async fn a_held_limiter_with_concurrent_walks_falls_back_within_the_deadline() {
    let (_portal, origin) = lake_at(Lake::new(R1, 150, &[(A, 0)])).await;
    let adm = admission(&origin, 0.01, COOLDOWN);
    let rpc = FakeRpc::default();
    let started = std::time::Instant::now();
    let read = || async {
        verified(&rpc, &adm)
            .await
            .window(100, 200, &[group(&[A], json!([T0]))], &mut 1_000)
            .await
            .unwrap()
    };
    let (a, b, c) = tokio::join!(read(), read(), read());
    assert!(started.elapsed() < DEADLINE * 3, "{:?}", started.elapsed());
    assert!([a, b, c].iter().all(|w| w.reached == 200));
    assert_eq!(rpc.windows().len(), 3);
}

/// Three failed portal phases cool the portal: windows go straight to the
/// RPC. Once it expires one window probes; the others use the RPC while it
/// runs. A probe dropped mid-flight (a stop, a lost lease) lets the next
/// window probe, and a probe that succeeds ends the cooling.
#[tokio::test]
async fn three_failures_cool_the_portal_and_one_probe_runs_after() {
    let (portal, origin) = lake_at(Lake::new(R1, 150, &[(A, 0)]).log(115, 0, A, &[T0])).await;
    portal.fail_pages.store(true, Ordering::SeqCst);
    let cooldown = Duration::from_millis(300);
    let adm = admission(&origin, 1_000.0, cooldown);
    let rpc = FakeRpc::default();
    let groups = [group(&[A], json!([T0]))];
    let read = || async {
        verified(&rpc, &adm)
            .await
            .window(100, 200, &groups, &mut 1_000)
            .await
            .unwrap()
    };
    for _ in 0..3 {
        assert_eq!(read().await.reached, 200);
    }
    assert_eq!(portal.page_count(), 3);
    let seen = portal.seen.lock().unwrap().len();
    assert_eq!(read().await.reached, 200);
    assert_eq!(
        portal.seen.lock().unwrap().len(),
        seen,
        "cooling: no portal request"
    );

    tokio::time::sleep(cooldown + Duration::from_millis(50)).await;
    portal.fail_pages.store(false, Ordering::SeqCst);
    portal.stall_pages.store(true, Ordering::SeqCst);
    let probing = async {
        verified(&rpc, &adm)
            .await
            .window(100, 200, &groups, &mut 1_000)
            .await
    };
    let concurrent = async {
        while portal.page_count() < 4 {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
        let started = std::time::Instant::now();
        let w = read().await;
        (started.elapsed(), w, portal.page_count())
    };
    // The probe is dropped once the other window has read from the RPC.
    let (probe, (took, w, pages)) = tokio::join!(
        tokio::time::timeout(Duration::from_millis(250), probing),
        concurrent
    );
    assert!(probe.is_err(), "the probe was dropped");
    assert!(took < Duration::from_millis(100), "{took:?}");
    assert_eq!((w.reached, pages), (200, 4), "the RPC, no second probe");

    portal.stall_pages.store(false, Ordering::SeqCst);
    let w = read().await;
    assert_eq!(
        (w.reached, positions(&w)),
        (150, vec![(115, 0)]),
        "the next window probes"
    );
    let w = read().await;
    assert_eq!(w.reached, 150, "and the cooling is over");
}

#[tokio::test]
async fn coverage_answers_that_name_another_chain_or_dataset_or_fail_read_from_the_rpc() {
    let answer = |chain: u64, dataset: &str, version: u64| {
        let body = json!({"schemaVersion": version, "dataset": dataset, "chainId": chain,
            "coverageRevision": R1, "finalized": {"number": 150, "hash": h(150)},
            "contracts": [{"address": A, "from": 0}]})
        .to_string();
        http(200, "content-type: application/json\r\n", &body)
    };
    for reply in [
        answer(1, "polygon-mainnet", 1),
        answer(137, "ethereum-mainnet", 1),
        answer(137, "polygon-mainnet", 2),
        http(404, "", ""),
        http(500, "", ""),
    ] {
        let (portal, origin) = lake_at(Lake::new(R1, 150, &[(A, 0)])).await;
        *portal.contracts_reply.lock().unwrap() = Some(reply);
        let adm = admission(&origin, 1_000.0, COOLDOWN);
        let rpc = FakeRpc::default();
        let w = verified(&rpc, &adm)
            .await
            .window(100, 200, &[group(&[A], json!([T0]))], &mut 1_000)
            .await
            .unwrap();
        assert_eq!(w.reached, 200);
        assert_eq!(rpc.calls(), [Call::Window(100, 200, vec![A.into()])]);
        assert_eq!(portal.page_count(), 0);
    }
}

#[tokio::test]
async fn a_split_window_charges_what_its_filters_cost_as_rpc_logs() {
    let (_portal, origin) = lake_at(Lake::new(R1, 150, &[(A, 0), (X, 120)])).await;
    let adm = admission(&origin, 1_000.0, COOLDOWN);
    let rpc = FakeRpc::default();
    // X starts inside the window: the RPC reads it for the whole window.
    let mut chunk = 1;
    let w = verified(&rpc, &adm)
        .await
        .window(100, 200, &[group(&[A, X], json!([T0]))], &mut chunk)
        .await
        .unwrap();
    assert_eq!(rpc.windows(), [Call::Window(100, 150, vec![X.into()])]);
    assert_eq!(
        (w.requests, w.charge),
        (2, 2),
        "anchor plus X; two chunks charged"
    );
    // `from == a` is admitted.
    let rpc = FakeRpc::default();
    let w = verified(&rpc, &adm)
        .await
        .window(120, 200, &[group(&[A, X], json!([T0]))], &mut 1)
        .await
        .unwrap();
    assert!(rpc.windows().is_empty());
    assert_eq!((w.reached, w.requests, w.charge), (150, 1, 2));
}

#[tokio::test]
async fn without_an_admission_it_is_the_rpc() {
    let rpc = FakeRpc::default();
    let w = AdmittedPortal::new(&rpc, None)
        .window(100, 200, &[group(&[A], json!([T0]))], &mut 1_000)
        .await
        .unwrap();
    assert_eq!((w.reached, w.requests, w.charge), (200, 1, 1));
    assert_eq!(rpc.calls(), [Call::Window(100, 200, vec![A.into()])]);
}

/// A window above the answer's finalized block (the Ethereum tail) is the
/// RPC's: no anchor, no page.
#[tokio::test]
async fn a_window_above_the_coverage_is_read_from_the_rpc() {
    let (portal, origin) = lake_at(Lake::new(R1, 150, &[(A, 0)])).await;
    let adm = admission(&origin, 1_000.0, COOLDOWN);
    let rpc = FakeRpc::default();
    let w = verified(&rpc, &adm)
        .await
        .window(151, 200, &[group(&[A], json!([T0]))], &mut 1_000)
        .await
        .unwrap();
    assert_eq!(w.reached, 200);
    assert_eq!(rpc.calls(), [Call::Window(151, 200, vec![A.into()])]);
    assert_eq!(portal.page_count(), 0);
}

/// A 503 holds the origin's limiter: the next window goes straight to the
/// RPC, without waiting on the portal.
#[tokio::test]
async fn a_cooling_origin_sends_windows_straight_to_the_rpc() {
    let (portal, origin) = lake_at(Lake::new(R1, 150, &[(A, 0)])).await;
    *portal.contracts_reply.lock().unwrap() = Some(http(503, "retry-after: 60\r\n", ""));
    let adm = admission(&origin, 1_000.0, COOLDOWN);
    let rpc = FakeRpc::default();
    let read = || async {
        verified(&rpc, &adm)
            .await
            .window(100, 200, &[group(&[A], json!([T0]))], &mut 1_000)
            .await
            .unwrap()
    };
    assert_eq!(read().await.reached, 200);
    let seen = portal.seen.lock().unwrap().len();
    let started = std::time::Instant::now();
    assert_eq!(read().await.reached, 200);
    assert!(
        started.elapsed() < Duration::from_millis(100),
        "{:?}",
        started.elapsed()
    );
    assert_eq!(portal.seen.lock().unwrap().len(), seen, "no portal request");
}

/// A projection whose RPC serves another chain (Amoy) never tries the
/// mainnet portal, even where every hash through N is the same: no
/// coverage read, no anchor, and the portal's state is left alone. Nor does
/// a source whose walk read no chain id.
#[tokio::test]
async fn a_walk_on_another_chain_reads_everything_from_the_rpc() {
    let (portal, origin) = lake_at(Lake::new(R1, 150, &[(A, 0)]).log(115, 0, A, &[T0])).await;
    portal.fail_pages.store(true, Ordering::SeqCst);
    let adm = admission(&origin, 1_000.0, COOLDOWN);
    let amoy = FakeRpc {
        chain: 80_002,
        ..Default::default()
    };
    for _ in 0..4 {
        let w = verified(&amoy, &adm)
            .await
            .window(100, 200, &[group(&[A], json!([T0]))], &mut 1_000)
            .await
            .unwrap();
        assert_eq!(w.reached, 200);
    }
    let unread = FakeRpc::default();
    let w = AdmittedPortal::new(&unread, Some(&adm))
        .window(100, 200, &[group(&[A], json!([T0]))], &mut 1_000)
        .await
        .unwrap();
    assert_eq!((w.reached, w.requests), (200, 1));
    assert!(portal.seen.lock().unwrap().is_empty(), "no portal request");
    assert!(
        amoy.headers().is_empty() && unread.headers().is_empty(),
        "no anchor"
    );
    // Four failing windows would have cooled it: the mainnet walk still
    // reads from the portal.
    portal.fail_pages.store(false, Ordering::SeqCst);
    let rpc = FakeRpc::default();
    let w = verified(&rpc, &adm)
        .await
        .window(100, 200, &[group(&[A], json!([T0]))], &mut 1_000)
        .await
        .unwrap();
    assert_eq!((w.reached, positions(&w)), (150, vec![(115, 0)]));
}

/// Two batches name block 110 with one hash and two times: the window is
/// read from the RPC, and neither time reaches a log.
#[tokio::test]
async fn one_block_with_two_times_reads_the_window_from_the_rpc() {
    let fill =
        |base: u64| -> Vec<String> { (0..9_999).map(|i| format!("0x{:040x}", base + i)).collect() };
    let (mut first, mut second) = (fill(0x10_0000), fill(0x20_0000));
    first.push(A.into());
    second.push(X.into());
    let mut lake = Lake::new(R1, 150, &[]);
    lake.from = first
        .iter()
        .chain(&second)
        .map(|a| (a.clone(), 0))
        .collect();
    let lake = lake.log(110, 0, A, &[T0]).log(110, 1, X, &[T1]);
    let (portal, origin) = lake_at(lake).await;
    *portal.skew_page.lock().unwrap() = Some(2);
    let adm = admission(&origin, 1_000.0, COOLDOWN);
    let rpc = FakeRpc {
        logs: vec![rpc_log(110, 0, A, &[T0]), rpc_log(110, 1, X, &[T1])],
        ..Default::default()
    };
    let groups = [
        LogGroup {
            addresses: first,
            topics: json!([T0]),
        },
        LogGroup {
            addresses: second,
            topics: json!([T1]),
        },
    ];
    let w = verified(&rpc, &adm)
        .await
        .window(100, 200, &groups, &mut 1_000)
        .await
        .unwrap();
    assert_eq!(portal.page_count(), 2);
    assert_eq!((w.reached, positions(&w)), (200, vec![(110, 0), (110, 1)]));
    let windows = rpc.windows();
    assert!(
        matches!(&windows[..], [Call::Window(100, 200, a)] if a.len() == 20_000),
        "the whole window from the RPC"
    );
}

/// A window that entered before the portal cooled, and succeeds after,
/// does not end that cooling.
#[tokio::test]
async fn a_success_older_than_the_cooling_does_not_end_it() {
    let (portal, origin) = lake_at(Lake::new(R1, 150, &[(A, 0)]).log(115, 0, A, &[T0])).await;
    let gate = Arc::new(tokio::sync::Semaphore::new(0));
    *portal.gate.lock().unwrap() = Some((1, gate.clone()));
    let adm = admission(&origin, 1_000.0, Duration::from_secs(60));
    let rpc = FakeRpc::default();
    let forked = FakeRpc {
        branch: HashMap::from([(150, other_branch(150))]),
        ..Default::default()
    };
    let groups = [group(&[A], json!([T0]))];
    let older = async {
        verified(&rpc, &adm)
            .await
            .window(100, 200, &groups, &mut 1_000)
            .await
            .unwrap()
    };
    let newer = async {
        while portal.page_count() < 1 {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
        // An anchor on another hash cools the portal at once.
        let w = verified(&forked, &adm)
            .await
            .window(100, 200, &groups, &mut 1_000)
            .await
            .unwrap();
        gate.add_permits(10);
        w
    };
    let (older, newer) = tokio::join!(older, newer);
    assert_eq!(older.reached, 150, "the older window read its page");
    assert_eq!(newer.reached, 200);
    let seen = portal.seen.lock().unwrap().len();
    let w = verified(&rpc, &adm)
        .await
        .window(100, 200, &groups, &mut 1_000)
        .await
        .unwrap();
    assert_eq!(w.reached, 200, "still cooling");
    assert_eq!(portal.seen.lock().unwrap().len(), seen);
}

/// After a cooling, a probe that reads no page (its window above the
/// coverage) leaves the portal cooled: the next window probes, and its
/// failure cools it again at once.
#[tokio::test]
async fn a_probe_that_reads_no_page_leaves_the_cooling() {
    let (portal, origin) = lake_at(Lake::new(R1, 150, &[(A, 0)])).await;
    portal.fail_pages.store(true, Ordering::SeqCst);
    let cooldown = Duration::from_millis(200);
    let adm = admission(&origin, 1_000.0, cooldown);
    let rpc = FakeRpc::default();
    let groups = [group(&[A], json!([T0]))];
    let read = |from: u64| {
        let (rpc, adm, groups) = (&rpc, &adm, &groups);
        async move {
            verified(rpc, adm)
                .await
                .window(from, 200, groups, &mut 1_000)
                .await
                .unwrap()
        }
    };
    for _ in 0..3 {
        read(100).await;
    }
    tokio::time::sleep(cooldown + Duration::from_millis(50)).await;
    assert_eq!(read(151).await.reached, 200, "above the coverage: neutral");
    let pages = portal.page_count();
    read(100).await;
    assert_eq!(portal.page_count(), pages + 1, "this window probed");
    let seen = portal.seen.lock().unwrap().len();
    read(100).await;
    assert_eq!(
        portal.seen.lock().unwrap().len(),
        seen,
        "the failed probe cooled it again at once"
    );
}

/// A hold the origin's limiter keeps sends windows to the RPC; once it
/// passes, of three windows at once exactly one probes the portal.
#[tokio::test]
async fn an_origin_hold_ends_with_one_probe() {
    let (portal, origin) = lake_at(Lake::new(R1, 150, &[(A, 0)]).log(115, 0, A, &[T0])).await;
    let adm = admission(&origin, 1_000.0, COOLDOWN);
    adm.reader.limiter.state.lock().unwrap().cooldown_until =
        Some(Instant::now() + Duration::from_millis(200));
    let rpc = FakeRpc::default();
    let groups = [group(&[A], json!([T0]))];
    let read = || async {
        verified(&rpc, &adm)
            .await
            .window(100, 200, &groups, &mut 1_000)
            .await
            .unwrap()
    };
    let (a, b, c) = tokio::join!(read(), read(), read());
    assert!([a, b, c].iter().all(|w| w.reached == 200));
    assert!(
        portal.seen.lock().unwrap().is_empty(),
        "held: no portal request"
    );
    tokio::time::sleep(Duration::from_millis(250)).await;
    let gate = Arc::new(tokio::sync::Semaphore::new(0));
    *portal.gate.lock().unwrap() = Some((1, gate.clone()));
    let release = async {
        tokio::time::sleep(Duration::from_millis(100)).await;
        gate.add_permits(10);
    };
    let (a, b, c, ()) = tokio::join!(read(), read(), read(), release);
    let reached: Vec<u64> = [a, b, c].iter().map(|w| w.reached).collect();
    assert_eq!(
        reached.iter().filter(|r| **r == 150).count(),
        1,
        "{reached:?}"
    );
    assert_eq!(
        reached.iter().filter(|r| **r == 200).count(),
        2,
        "{reached:?}"
    );
    assert_eq!(portal.page_count(), 1, "one probe");
}

/// The portal's log of A and the RPC's log of X at one position: the window
/// is read again from the RPC, charging the split's RPC log too, and settles
/// one duplicate; three such windows cool the portal.
#[tokio::test]
async fn a_duplicate_charges_the_discarded_split_and_counts_once() {
    let (portal, origin) = lake_at(Lake::new(R1, 150, &[(A, 0)]).log(115, 0, A, &[T0])).await;
    let adm = admission(&origin, 1_000.0, COOLDOWN);
    let rpc = FakeRpc {
        logs: vec![rpc_log(115, 0, X, &[T0])],
        ..Default::default()
    };
    let groups = [group(&[A, X], json!([T0]))];
    let w = verified(&rpc, &adm)
        .await
        .window(100, 200, &groups, &mut 1)
        .await
        .unwrap();
    assert_eq!(
        rpc.windows(),
        [
            Call::Window(100, 150, vec![X.into()]),
            Call::Window(100, 200, vec![A.into(), X.into()])
        ]
    );
    assert_eq!(positions(&w), [(115, 0)]);
    assert_eq!(
        (w.charge, w.requests),
        (3, 4),
        "logical 2; one split and two whole log sends"
    );
    for _ in 0..2 {
        verified(&rpc, &adm)
            .await
            .window(100, 200, &groups, &mut 1)
            .await
            .unwrap();
    }
    let seen = portal.seen.lock().unwrap().len();
    verified(&rpc, &adm)
        .await
        .window(100, 200, &groups, &mut 1)
        .await
        .unwrap();
    assert_eq!(
        portal.seen.lock().unwrap().len(),
        seen,
        "three duplicates cool it"
    );
}

/// A window that entered before the portal cooled settles nothing when it
/// ends: after the cooling, a probe and two failures, its late success does
/// not reset the failures, and the third failure cools the portal.
#[tokio::test]
async fn a_window_older_than_the_latest_cooling_settles_nothing() {
    let (portal, origin) = lake_at(Lake::new(R1, 150, &[(A, 0)]).log(115, 0, A, &[T0])).await;
    let gate = Arc::new(tokio::sync::Semaphore::new(0));
    *portal.gate.lock().unwrap() = Some((1, gate.clone()));
    let cooldown = Duration::from_millis(100);
    let adm = Admission::with_cooldown(
        PortalReader::new(&origin, "polygon-mainnet", None, 1_000.0).unwrap(),
        "matic",
        Duration::from_secs(5),
        cooldown,
    );
    let rpc = FakeRpc::default();
    let forked = FakeRpc {
        branch: HashMap::from([(150, other_branch(150))]),
        ..Default::default()
    };
    let groups = [group(&[A], json!([T0]))];
    let read = |rpc| window_of(rpc, &adm, &groups);
    let older = read(&rpc);
    let rest = async {
        while portal.page_count() < 1 {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
        // The older window waits on its page. The portal cools, and a probe
        // ends the cooling.
        assert_eq!(read(&forked).await.reached, 200);
        tokio::time::sleep(cooldown + Duration::from_millis(50)).await;
        assert_eq!(read(&rpc).await.reached, 150, "the probe");
        portal.fail_pages.store(true, Ordering::SeqCst);
        for _ in 0..2 {
            assert_eq!(read(&rpc).await.reached, 200);
        }
        // The older window's page is answered.
        portal.fail_pages.store(false, Ordering::SeqCst);
        gate.add_permits(10);
    };
    let (older, ()) = tokio::join!(older, rest);
    assert_eq!(older.reached, 150, "the older window read its page");
    portal.fail_pages.store(true, Ordering::SeqCst);
    assert_eq!(read(&rpc).await.reached, 200, "the third failure");
    let seen = portal.seen.lock().unwrap().len();
    assert_eq!(read(&rpc).await.reached, 200);
    assert_eq!(
        portal.seen.lock().unwrap().len(),
        seen,
        "it cooled the portal"
    );
}

/// An RPC log and a portal log of one block, at two positions, that give
/// it another time or another hash: the window is read again whole from the
/// RPC, the split's send charged.
#[tokio::test]
async fn a_block_the_two_sources_disagree_on_reads_the_window_from_the_rpc() {
    for (time, hash) in [
        (time_of(110) + 1, h(110)),
        (time_of(110), other_branch(110)),
    ] {
        let (_portal, origin) = lake_at(Lake::new(R1, 150, &[(A, 0)]).log(110, 1, A, &[T0])).await;
        let adm = admission(&origin, 1_000.0, COOLDOWN);
        let mut x = rpc_log(110, 0, X, &[T0]);
        x.block_timestamp = Some(format!("0x{time:x}"));
        x.block_hash = Some(hash);
        let rpc = FakeRpc {
            logs: vec![x],
            ..Default::default()
        };
        let w = verified(&rpc, &adm)
            .await
            .window(100, 200, &[group(&[A, X], json!([T0]))], &mut 1)
            .await
            .unwrap();
        assert_eq!(
            rpc.windows(),
            [
                Call::Window(100, 150, vec![X.into()]),
                Call::Window(100, 200, vec![A.into(), X.into()])
            ]
        );
        assert_eq!(positions(&w), [(110, 0)], "the RPC's logs alone");
        assert_eq!((w.charge, w.requests), (3, 4));
    }
}

/// A real 503 sets a hold; it passes while no window looks; three windows
/// then come at once: exactly one probes, the others read from the RPC.
#[tokio::test]
async fn a_hold_that_passed_unseen_still_ends_with_one_probe() {
    let (portal, origin) = lake_at(Lake::new(R1, 150, &[(A, 0)]).log(115, 0, A, &[T0])).await;
    *portal.contracts_reply.lock().unwrap() = Some(http(503, "", ""));
    let adm = admission(&origin, 1_000.0, COOLDOWN);
    let rpc = FakeRpc::default();
    let groups = [group(&[A], json!([T0]))];
    assert_eq!(window_of(&rpc, &adm, &groups).await.reached, 200);
    assert!(adm.reader.cooling(), "the 503 set a hold");
    // The hold, shortened so the test is quick, passes unseen.
    adm.reader.limiter.state.lock().unwrap().cooldown_until =
        Some(Instant::now() + Duration::from_millis(50));
    tokio::time::sleep(Duration::from_millis(150)).await;
    *portal.contracts_reply.lock().unwrap() = None;
    let gate = Arc::new(tokio::sync::Semaphore::new(0));
    *portal.gate.lock().unwrap() = Some((1, gate.clone()));
    let release = async {
        tokio::time::sleep(Duration::from_millis(100)).await;
        gate.add_permits(10);
    };
    let (a, b, c, ()) = tokio::join!(
        window_of(&rpc, &adm, &groups),
        window_of(&rpc, &adm, &groups),
        window_of(&rpc, &adm, &groups),
        release
    );
    let reached: Vec<u64> = [a, b, c].iter().map(|w| w.reached).collect();
    assert_eq!(
        reached.iter().filter(|r| **r == 150).count(),
        1,
        "{reached:?}"
    );
    assert_eq!(portal.page_count(), 1, "one probe");
}

/// A JSON-RPC endpoint answering each request, by its id, with the next
/// scripted result or error (`{"hold": true}` takes it and never answers),
/// then 500s; and how many requests it saw.
async fn scripted_rpc(script: Vec<Value>) -> (String, Arc<AtomicUsize>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let seen = Arc::new(AtomicUsize::new(0));
    let count = seen.clone();
    let script = Arc::new(Mutex::new(VecDeque::from(script)));
    tokio::spawn(async move {
        loop {
            let Ok((mut sock, _)) = listener.accept().await else {
                return;
            };
            let (count, script) = (count.clone(), script.clone());
            tokio::spawn(async move {
                let mut buf = Vec::new();
                let mut tmp = [0u8; 65536];
                let head_end = loop {
                    let n = sock.read(&mut tmp).await.unwrap_or(0);
                    if n == 0 {
                        return;
                    }
                    buf.extend_from_slice(&tmp[..n]);
                    if let Some(i) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
                        break i + 4;
                    }
                };
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
                count.fetch_add(1, Ordering::SeqCst);
                let request: Value = serde_json::from_slice(&buf[head_end..]).unwrap_or_default();
                let next = script.lock().unwrap().pop_front();
                if next.as_ref().is_some_and(|a| a["hold"] == json!(true)) {
                    // Received, never answered.
                    tokio::time::sleep(Duration::from_secs(60)).await;
                    return;
                }
                let reply = match next {
                    Some(mut answer) => {
                        answer["jsonrpc"] = json!("2.0");
                        answer["id"] = request["id"].clone();
                        http(
                            200,
                            "content-type: application/json\r\n",
                            &answer.to_string(),
                        )
                    }
                    None => http(500, "", ""),
                };
                let _ = sock.write_all(reply.as_bytes()).await;
                let _ = sock.shutdown().await;
            });
        }
    });
    (format!("http://127.0.0.1:{}", addr.port()), seen)
}

fn narrowing() -> Value {
    json!({"error": {"code": -32005, "message":
        "query returned more than 10000 results. Try with this block range [0x64, 0x96]."}})
}

/// Over the real transport, a window that first narrows its range, is then
/// refused the address count and reads each address alone reports and
/// charges the four requests the endpoint saw.
#[tokio::test]
async fn an_rpc_window_counts_every_request_it_sent() {
    let (url, seen) = scripted_rpc(vec![
        narrowing(),
        json!({"error": {"code": -32602, "message": "exceed max addresses or topics per search position"}}),
        json!({"result": []}),
        json!({"result": []}),
    ])
    .await;
    let http = crate::http::Outbound::for_tests();
    let source = crate::logs::RpcSource::new(crate::rpc::Rpc::at(&http, &url));
    let mut chunk = 2;
    let w = source
        .window(100, 200, &[group(&[A, X], json!([T0]))], &mut chunk)
        .await
        .unwrap();
    // The refusal drops the narrowed range: each address is read over the
    // whole window.
    assert_eq!((w.reached, w.requests, w.charge, chunk), (200, 4, 4, 1));
    assert_eq!(seen.load(Ordering::SeqCst), 4);
}

/// A counter recorder for one test: the value of every counter by name and
/// labels.
#[derive(Default)]
struct Counters(Arc<Mutex<HashMap<String, Arc<std::sync::atomic::AtomicU64>>>>);

struct CounterCell(Arc<std::sync::atomic::AtomicU64>);

impl metrics::CounterFn for CounterCell {
    fn increment(&self, value: u64) {
        self.0.fetch_add(value, Ordering::SeqCst);
    }
    fn absolute(&self, value: u64) {
        self.0.store(value, Ordering::SeqCst);
    }
}

impl metrics::Recorder for Counters {
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
    fn register_counter(&self, key: &metrics::Key, _: &metrics::Metadata<'_>) -> metrics::Counter {
        let mut labels: Vec<String> = key
            .labels()
            .map(|l| format!("{}={}", l.key(), l.value()))
            .collect();
        labels.sort();
        let name = format!("{}{{{}}}", key.name(), labels.join(","));
        let cell = self.0.lock().unwrap().entry(name).or_default().clone();
        metrics::Counter::from_arc(Arc::new(CounterCell(cell)))
    }
    fn register_gauge(&self, _: &metrics::Key, _: &metrics::Metadata<'_>) -> metrics::Gauge {
        metrics::Gauge::noop()
    }
    fn register_histogram(
        &self,
        _: &metrics::Key,
        _: &metrics::Metadata<'_>,
    ) -> metrics::Histogram {
        metrics::Histogram::noop()
    }
}

/// A window read whole from the RPC that fails after its range was narrowed
/// still counts both requests it sent in the RPC send metric.
#[test]
fn a_failed_rpc_read_counts_the_requests_it_sent() {
    let recorder = Counters::default();
    let values = recorder.0.clone();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    metrics::with_local_recorder(&recorder, || {
        runtime.block_on(async {
            let (portal, origin) = lake_at(Lake::new(R1, 150, &[(A, 0)])).await;
            *portal.contracts_reply.lock().unwrap() = Some(http(404, "", ""));
            let adm = admission(&origin, 1_000.0, COOLDOWN);
            let (url, seen) = scripted_rpc(vec![json!({"result": "0x89"}), narrowing()]).await;
            let http = crate::http::Outbound::for_tests();
            let source = AdmittedPortal::new(
                crate::logs::RpcSource::new(crate::rpc::Rpc::at(&http, &url)),
                Some(&adm),
            );
            source.chain_id().await.unwrap();
            let r = source
                .window(100, 200, &[group(&[A], json!([T0]))], &mut 1_000)
                .await;
            assert!(r.is_err(), "the second log request got a 500");
            assert_eq!(
                seen.load(Ordering::SeqCst),
                3,
                "chain id and two log requests"
            );
        })
    });
    let values = values.lock().unwrap();
    let fallback = values
        .get("chain_logs_source_rpc_sends_total{kind=fallback,net=matic}")
        .map(|c| c.load(Ordering::SeqCst));
    assert_eq!(fallback, Some(2));
}

/// A window read whole from the RPC (its walk read no chain id) is dropped
/// once the endpoint received its eth_getLogs: the send metric and the
/// shared transport count each hold that one request.
#[test]
fn a_dropped_rpc_read_counts_the_request_it_sent() {
    let recorder = Counters::default();
    let values = recorder.0.clone();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let shared = AtomicUsize::new(0);
    metrics::with_local_recorder(&recorder, || {
        runtime.block_on(async {
            let (_portal, origin) = lake_at(Lake::new(R1, 150, &[(A, 0)])).await;
            let adm = admission(&origin, 1_000.0, COOLDOWN);
            let (url, seen) = scripted_rpc(vec![json!({"hold": true})]).await;
            let http = crate::http::Outbound::for_tests();
            let source = AdmittedPortal::new(
                crate::logs::RpcSource::new(crate::rpc::Rpc::at(&http, &url).counting(&shared)),
                Some(&adm),
            );
            let groups = [group(&[A], json!([T0]))];
            let mut chunk = 1_000;
            let window = source.window(100, 200, &groups, &mut chunk);
            tokio::pin!(window);
            let received = async {
                while seen.load(Ordering::SeqCst) < 1 {
                    tokio::time::sleep(Duration::from_millis(5)).await;
                }
            };
            tokio::select! {
                _ = &mut window => panic!("the endpoint holds the request"),
                () = received => {}
            }
        })
    });
    assert_eq!(
        shared.load(Ordering::SeqCst),
        1,
        "the shared transport count"
    );
    let values = values.lock().unwrap();
    let fallback = values
        .get("chain_logs_source_rpc_sends_total{kind=fallback,net=matic}")
        .map(|c| c.load(Ordering::SeqCst));
    assert_eq!(fallback, Some(1));
}

/// A hold set while a probe is in flight outlives that probe: its success
/// settles nothing, and once the hold passes, of three windows at once
/// exactly one probes again.
#[tokio::test]
async fn a_newer_hold_outlives_an_older_probe() {
    let (portal, origin) = lake_at(Lake::new(R1, 150, &[(A, 0)]).log(115, 0, A, &[T0])).await;
    let cooldown = Duration::from_millis(100);
    let adm = admission(&origin, 1_000.0, cooldown);
    let rpc = FakeRpc::default();
    let forked = FakeRpc {
        branch: HashMap::from([(150, other_branch(150))]),
        ..Default::default()
    };
    let groups = [group(&[A], json!([T0]))];
    // An anchor on another hash cools the portal; it passes.
    assert_eq!(window_of(&forked, &adm, &groups).await.reached, 200);
    tokio::time::sleep(cooldown + Duration::from_millis(50)).await;
    let first = Arc::new(tokio::sync::Semaphore::new(0));
    *portal.gate.lock().unwrap() = Some((1, first.clone()));
    let probe = window_of(&rpc, &adm, &groups);
    let rest = async {
        while portal.page_count() < 1 {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
        // A newer hold, shortened so the test is quick; a window entering
        // during it reads from the RPC.
        adm.reader.limiter.cool(None);
        adm.reader.limiter.state.lock().unwrap().cooldown_until =
            Some(Instant::now() + Duration::from_millis(150));
        assert_eq!(window_of(&rpc, &adm, &groups).await.reached, 200);
        first.add_permits(10);
    };
    let (probe, ()) = tokio::join!(probe, rest);
    assert_eq!(probe.reached, 150, "the older probe read its page");
    tokio::time::sleep(Duration::from_millis(200)).await;
    let second = Arc::new(tokio::sync::Semaphore::new(0));
    *portal.gate.lock().unwrap() = Some((2, second.clone()));
    let release = async {
        tokio::time::sleep(Duration::from_millis(100)).await;
        second.add_permits(10);
    };
    let (a, b, c, ()) = tokio::join!(
        window_of(&rpc, &adm, &groups),
        window_of(&rpc, &adm, &groups),
        window_of(&rpc, &adm, &groups),
        release
    );
    let reached: Vec<u64> = [a, b, c].iter().map(|w| w.reached).collect();
    assert_eq!(
        reached.iter().filter(|r| **r == 150).count(),
        1,
        "{reached:?}"
    );
    assert_eq!(portal.page_count(), 2, "one fresh probe");
}

/// A probe whose own page gets a 429 sees the hold, not a portal failure:
/// once the hold passes, the next window probes again, well before the
/// network's cooldown would have ended.
#[tokio::test]
async fn a_hold_a_probe_receives_is_not_its_failure() {
    let (portal, origin) = lake_at(Lake::new(R1, 150, &[(A, 0)]).log(115, 0, A, &[T0])).await;
    let cooldown = Duration::from_millis(200);
    let adm = admission(&origin, 1_000.0, cooldown);
    let rpc = FakeRpc::default();
    let forked = FakeRpc {
        branch: HashMap::from([(150, other_branch(150))]),
        ..Default::default()
    };
    let groups = [group(&[A], json!([T0]))];
    assert_eq!(window_of(&forked, &adm, &groups).await.reached, 200);
    tokio::time::sleep(cooldown + Duration::from_millis(50)).await;
    // The probe's page is answered 429.
    *portal.fail_with.lock().unwrap() = Some((429, "retry-after: 1\r\n"));
    portal.fail_pages.store(true, Ordering::SeqCst);
    assert_eq!(window_of(&rpc, &adm, &groups).await.reached, 200);
    assert!(adm.reader.cooling(), "the 429 set a hold");
    portal.fail_pages.store(false, Ordering::SeqCst);
    // The hold, shortened so the test is quick, passes; a cooling of the
    // network (had the 429 counted as the probe's failure) would last
    // another 200 ms.
    adm.reader.limiter.state.lock().unwrap().cooldown_until =
        Some(Instant::now() + Duration::from_millis(20));
    tokio::time::sleep(Duration::from_millis(60)).await;
    let w = window_of(&rpc, &adm, &groups).await;
    assert_eq!(
        (w.reached, positions(&w)),
        (150, vec![(115, 0)]),
        "probed again"
    );
}

/// Anchors the RPC does not answer in time are the RPC's fault: three of
/// them leave the portal open.
#[tokio::test]
async fn an_anchor_the_rpc_fails_does_not_cool_the_portal() {
    let (_portal, origin) = lake_at(Lake::new(R1, 150, &[(A, 0)]).log(115, 0, A, &[T0])).await;
    let adm = admission(&origin, 1_000.0, COOLDOWN);
    let slow = FakeRpc {
        header_delay: HashMap::from([(150, Duration::from_secs(30))]),
        ..Default::default()
    };
    let groups = [group(&[A], json!([T0]))];
    for _ in 0..3 {
        assert_eq!(window_of(&slow, &adm, &groups).await.reached, 200);
    }
    let rpc = FakeRpc::default();
    let w = window_of(&rpc, &adm, &groups).await;
    assert_eq!((w.reached, positions(&w)), (150, vec![(115, 0)]));
}

/// A window above an answer older than its age limit reads it again, once:
/// a long pass follows the lake as it finalizes. A recent answer is kept.
#[tokio::test]
async fn a_window_above_an_old_answer_reads_it_again() {
    let contracts = |portal: &Portal| {
        portal
            .seen
            .lock()
            .unwrap()
            .iter()
            .filter(|p| p.ends_with("/contracts"))
            .count()
    };
    for (ttl, follows) in [(Duration::ZERO, true), (COVERAGE_TTL, false)] {
        let (portal, origin) = lake_at(Lake::new(R1, 150, &[(A, 0)]).log(115, 0, A, &[T0]).log(
            200,
            0,
            A,
            &[T0],
        ))
        .await;
        let adm = admission(&origin, 1_000.0, COOLDOWN).with_coverage_ttl(ttl);
        let rpc = FakeRpc::default();
        let source = verified(&rpc, &adm).await;
        let groups = [group(&[A], json!([T0]))];
        let w = source.window(100, 200, &groups, &mut 1_000).await.unwrap();
        assert_eq!(w.reached, 150);
        // The lake finalizes further.
        *portal.lake.lock().unwrap() = Some(
            Lake::new(R2, 300, &[(A, 0)])
                .log(115, 0, A, &[T0])
                .log(200, 0, A, &[T0]),
        );
        let w = source.window(151, 300, &groups, &mut 1_000).await.unwrap();
        if follows {
            assert_eq!((w.reached, positions(&w)), (300, vec![(200, 0)]));
            assert_eq!(contracts(&portal), 2);
        } else {
            assert_eq!(w.reached, 300, "from the RPC");
            assert_eq!(contracts(&portal), 1, "the recent answer is kept");
            assert_eq!(rpc.windows(), [Call::Window(151, 300, vec![A.into()])]);
        }
    }
}

/// A hold another reader of the origin set (a shadow, a comparison) while
/// no window was in flight, passed unseen: of three windows at once,
/// exactly one probes.
#[tokio::test]
async fn a_hold_another_reader_set_ends_with_one_probe() {
    let (portal, origin) = lake_at(Lake::new(R1, 150, &[(A, 0)]).log(115, 0, A, &[T0])).await;
    let adm = admission(&origin, 1_000.0, COOLDOWN);
    let rpc = FakeRpc::default();
    let groups = [group(&[A], json!([T0]))];
    // Another reader's 429, its hold shortened so the test is quick.
    let other = PortalReader::new(&origin, "polygon-mainnet", None, 1_000.0).unwrap();
    other.limiter.cool(None);
    other.limiter.state.lock().unwrap().cooldown_until =
        Some(Instant::now() + Duration::from_millis(30));
    tokio::time::sleep(Duration::from_millis(80)).await;
    let gate = Arc::new(tokio::sync::Semaphore::new(0));
    *portal.gate.lock().unwrap() = Some((1, gate.clone()));
    let release = async {
        tokio::time::sleep(Duration::from_millis(100)).await;
        gate.add_permits(10);
    };
    let (a, b, c, ()) = tokio::join!(
        window_of(&rpc, &adm, &groups),
        window_of(&rpc, &adm, &groups),
        window_of(&rpc, &adm, &groups),
        release
    );
    let reached: Vec<u64> = [a, b, c].iter().map(|w| w.reached).collect();
    assert_eq!(
        reached.iter().filter(|r| **r == 150).count(),
        1,
        "{reached:?}"
    );
    assert_eq!(portal.page_count(), 1, "one probe");
}
