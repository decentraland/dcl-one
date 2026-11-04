//! A JSON-RPC chain on the loopback for the access checks: blocks every
//! [`INTERVAL`] seconds from [`GENESIS`] up to now, `eth_call` to the Checker
//! (only with its bytecode as the state override, decoded through
//! `multicall`) and to item contracts, each question answered by the test.
//! A JSON-RPC batch is answered element by element unless batches are off.

use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use alloy_primitives::{Address, B256, Bytes, U256};
use alloy_sol_types::SolCall;
use axum::extract::State;
use axum::response::IntoResponse;
use axum::routing::post;
use axum::{Json, Router};
use serde_json::{Value, json};

use super::onchain::{Checker, Erc721, Erc1155};

pub(crate) const GENESIS: i64 = 1_500_000_000;
pub(crate) const INTERVAL: i64 = 2;

/// The block an entity at `timestamp_ms` is checked at first.
pub(crate) fn at_deployment(timestamp_ms: i64) -> u64 {
    let (upper, _) = super::onchain::timestamp_bounds(timestamp_ms);
    ((upper - GENESIS) / INTERVAL) as u64
}

/// The block five minutes before it.
pub(crate) fn five_min_before(timestamp_ms: i64) -> u64 {
    let (_, lower) = super::onchain::timestamp_bounds(timestamp_ms);
    let last = (lower - GENESIS) / INTERVAL;
    (last + i64::from(GENESIS + last * INTERVAL < lower)) as u64
}

fn head() -> u64 {
    ((chrono::Utc::now().timestamp() - GENESIS) / INTERVAL) as u64
}

/// One question the chain was asked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Query {
    Land {
        sender: Address,
        land: Address,
        estate: Address,
        x: i64,
        y: i64,
    },
    Name {
        sender: Address,
        registrar: Address,
        name: String,
    },
    Wearable {
        sender: Address,
        factories: Vec<Address>,
        collection: Address,
        item: U256,
        hash: String,
        committees: Vec<Address>,
    },
    ThirdParty {
        registry: Address,
        tp_id: String,
        root: B256,
    },
    OwnerOf {
        contract: Address,
        token: U256,
    },
    BalanceOf {
        contract: Address,
        account: Address,
        id: U256,
    },
}

/// The chain's answer to one question.
#[derive(Debug, Clone)]
pub(crate) enum Reply {
    Yes(bool),
    Owner(Address),
    Balance(U256),
    /// The call returns no data (an address without code).
    Empty,
    /// The call reverts.
    Revert,
    /// The node cannot answer (a pruned node asked about an old block).
    Fail,
}

type Answer = Arc<dyn Fn(&Query, u64) -> Reply + Send + Sync>;

#[derive(Clone, Default)]
struct Switches {
    down: Arc<AtomicBool>,
    no_batch: Arc<AtomicBool>,
    chain_id: Arc<AtomicU64>,
    requests: Arc<AtomicUsize>,
    /// `eth_getCode` answers empty while set.
    no_code: Arc<AtomicBool>,
}

#[derive(Clone)]
struct Fake {
    answer: Answer,
    switches: Switches,
    log: Arc<Mutex<Vec<String>>>,
}

pub(crate) struct FakeChain {
    pub url: String,
    switches: Switches,
    log: Arc<Mutex<Vec<String>>>,
}

impl FakeChain {
    /// Every request answers HTTP 503 while set.
    pub fn set_down(&self, down: bool) {
        self.switches.down.store(down, Ordering::SeqCst);
    }

    /// A batch request answers a single JSON-RPC error while set, as an
    /// endpoint without batches does.
    pub fn set_no_batch(&self, no_batch: bool) {
        self.switches.no_batch.store(no_batch, Ordering::SeqCst);
    }

    /// Every address holds no code while set (`eth_getCode` answers `0x`).
    pub fn set_no_code(&self, no_code: bool) {
        self.switches.no_code.store(no_code, Ordering::SeqCst);
    }

    /// The `eth_chainId` this chain answers (1 by default).
    pub fn set_chain_id(&self, id: u64) {
        self.switches.chain_id.store(id, Ordering::SeqCst);
    }

    /// HTTP requests received so far (a batch is one).
    pub fn requests(&self) -> usize {
        self.switches.requests.load(Ordering::SeqCst)
    }

    /// The methods called so far, `eth_call` as `eth_call@<block>`.
    pub fn calls(&self) -> Vec<String> {
        self.log.lock().unwrap().clone()
    }

    pub fn clear(&self) {
        self.log.lock().unwrap().clear();
        self.switches.requests.store(0, Ordering::SeqCst);
    }
}

fn decode_checker(data: &[u8]) -> Option<Query> {
    let selector: [u8; 4] = data.get(..4)?.try_into().ok()?;
    Some(match selector {
        Checker::checkLANDCall::SELECTOR => {
            let c = Checker::checkLANDCall::abi_decode(data).ok()?;
            Query::Land {
                sender: c._sender,
                land: c._land,
                estate: c._estate,
                x: i64::try_from(c._x).ok()?,
                y: i64::try_from(c._y).ok()?,
            }
        }
        Checker::checkNameCall::SELECTOR => {
            let c = Checker::checkNameCall::abi_decode(data).ok()?;
            Query::Name {
                sender: c._sender,
                registrar: c._registrar,
                name: c._name,
            }
        }
        Checker::validateWearablesCall::SELECTOR => {
            let c = Checker::validateWearablesCall::abi_decode(data).ok()?;
            Query::Wearable {
                sender: c._sender,
                factories: c._factories,
                collection: c._collection,
                item: c._itemId,
                hash: c._contentHash,
                committees: c._committees,
            }
        }
        Checker::validateThirdPartyCall::SELECTOR => {
            let c = Checker::validateThirdPartyCall::abi_decode(data).ok()?;
            Query::ThirdParty {
                registry: c._tpRegistry,
                tp_id: c._tpId,
                root: c._root,
            }
        }
        _ => return None,
    })
}

fn decode_item(to: Address, data: &[u8]) -> Option<Query> {
    let selector: [u8; 4] = data.get(..4)?.try_into().ok()?;
    Some(match selector {
        Erc721::ownerOfCall::SELECTOR => Query::OwnerOf {
            contract: to,
            token: Erc721::ownerOfCall::abi_decode(data).ok()?.tokenId,
        },
        Erc1155::balanceOfCall::SELECTOR => {
            let c = Erc1155::balanceOfCall::abi_decode(data).ok()?;
            Query::BalanceOf {
                contract: to,
                account: c.account,
                id: c.id,
            }
        }
        _ => return None,
    })
}

fn quantity(v: &Value) -> Option<u64> {
    u64::from_str_radix(v.as_str()?.trim_start_matches("0x"), 16).ok()
}

/// A block number or `latest` (the head).
fn block_param(v: &Value) -> Option<u64> {
    match v.as_str() {
        Some("latest") => Some(head()),
        _ => quantity(v),
    }
}

/// One eth_call: `Ok(result bytes)` or `Err((code, message))`.
fn eth_call(fake: &Fake, params: &Value) -> Result<Vec<u8>, (i64, String)> {
    let block = block_param(&params[1]).ok_or((-32602, "bad block".to_string()))?;
    if block > head() {
        return Err((-32000, "header not found".into()));
    }
    let to: Address = params[0]["to"].as_str().unwrap().parse().unwrap();
    let data = hex::decode(params[0]["data"].as_str().unwrap().trim_start_matches("0x")).unwrap();
    let reply_of = |q: &Query| (fake.answer)(q, block);
    let overridden = params[2]
        .as_object()
        .and_then(|o| {
            o.iter()
                .find(|(k, _)| k.parse::<Address>().ok() == Some(to))
        })
        .and_then(|(_, v)| v["code"].as_str())
        == Some(super::onchain::checker_bytecode());
    let replies: Vec<Reply> = if data.get(..4) == Some(&Checker::multicallCall::SELECTOR[..]) {
        if !overridden {
            // The deployed checker is not what the override injects.
            return Ok(Vec::new());
        }
        let calls = Checker::multicallCall::abi_decode(&data).unwrap().data;
        calls
            .iter()
            .map(|c| reply_of(&decode_checker(c).expect("a checker call")))
            .collect()
    } else {
        match decode_item(to, &data) {
            Some(q) => vec![reply_of(&q)],
            None => return Ok(Vec::new()),
        }
    };
    if replies.iter().any(|r| matches!(r, Reply::Fail)) {
        return Err((-32000, "missing trie node".into()));
    }
    if replies.iter().any(|r| matches!(r, Reply::Revert)) {
        return Err((3, "execution reverted".into()));
    }
    let single = |r: &Reply| match r {
        Reply::Yes(b) => alloy_sol_types::SolValue::abi_encode(b),
        Reply::Owner(a) => alloy_sol_types::SolValue::abi_encode(a),
        Reply::Balance(n) => alloy_sol_types::SolValue::abi_encode(n),
        _ => Vec::new(),
    };
    if data.get(..4) == Some(&Checker::multicallCall::SELECTOR[..]) {
        let bools: Vec<bool> = replies
            .iter()
            .map(|r| matches!(r, Reply::Yes(true)))
            .collect();
        Ok(Checker::multicallCall::abi_encode_returns(&bools))
    } else {
        Ok(single(&replies[0]))
    }
}

/// The envelope answering one request.
fn answer_one(fake: &Fake, body: &Value) -> Value {
    let method = body["method"].as_str().unwrap_or_default().to_string();
    let params = &body["params"];
    let answer = match method.as_str() {
        "eth_getBlockByNumber" => {
            let head = head();
            let n = match params[0].as_str() {
                Some("latest") => Some(head),
                _ => quantity(&params[0]).filter(|n| *n <= head),
            };
            fake.log.lock().unwrap().push(method.clone());
            Ok(n.map_or(Value::Null, |n| {
                json!({ "number": format!("0x{n:x}"), "timestamp": format!("0x{:x}", GENESIS + n as i64 * INTERVAL) })
            }))
        }
        "eth_call" => {
            fake.log.lock().unwrap().push(format!(
                "eth_call@{}",
                block_param(&params[1]).unwrap_or_default()
            ));
            eth_call(fake, params).map(|out| json!(Bytes::from(out)))
        }
        "eth_getCode" => {
            fake.log.lock().unwrap().push(method.clone());
            Ok(json!(if fake.switches.no_code.load(Ordering::SeqCst) {
                "0x"
            } else {
                "0x6001"
            }))
        }
        "eth_chainId" => Ok(json!(format!(
            "0x{:x}",
            fake.switches.chain_id.load(Ordering::SeqCst)
        ))),
        other => Err((-32601, format!("{other} not found"))),
    };
    let mut envelope = match answer {
        Ok(result) => json!({ "result": result }),
        Err((code, message)) => json!({ "error": { "code": code, "message": message } }),
    };
    envelope["jsonrpc"] = "2.0".into();
    envelope["id"] = body["id"].clone();
    envelope
}

async fn handle(State(fake): State<Fake>, Json(body): Json<Value>) -> axum::response::Response {
    fake.switches.requests.fetch_add(1, Ordering::SeqCst);
    if fake.switches.down.load(Ordering::SeqCst) {
        return axum::http::StatusCode::SERVICE_UNAVAILABLE.into_response();
    }
    match body {
        Value::Array(_) if fake.switches.no_batch.load(Ordering::SeqCst) => Json(json!({
            "jsonrpc": "2.0", "id": null,
            "error": { "code": -32600, "message": "batch requests are not supported" }
        }))
        .into_response(),
        Value::Array(requests) => Json(Value::Array(
            requests.iter().map(|r| answer_one(&fake, r)).collect(),
        ))
        .into_response(),
        single => Json(answer_one(&fake, &single)).into_response(),
    }
}

/// A chain on the loopback whose every answer is `answer(question, block)`.
pub(crate) async fn serve(
    answer: impl Fn(&Query, u64) -> Reply + Send + Sync + 'static,
) -> FakeChain {
    let switches = Switches::default();
    switches.chain_id.store(1, Ordering::SeqCst);
    let fake = Fake {
        answer: Arc::new(answer),
        switches: switches.clone(),
        log: Arc::new(Mutex::new(Vec::new())),
    };
    let chain = FakeChain {
        url: String::new(),
        switches,
        log: fake.log.clone(),
    };
    let app = Router::new().route("/", post(handle)).with_state(fake);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}/", listener.local_addr().unwrap());
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    FakeChain { url, ..chain }
}

/// The LAND answer granting `grants` as `(sender, x, y)`.
pub(crate) fn land_owners(
    grants: Vec<(String, i64, i64)>,
) -> impl Fn(&Query, u64) -> Reply + Send + Sync {
    move |q, _| match q {
        Query::Land { sender, x, y, .. } => {
            Reply::Yes(grants.iter().any(|(s, gx, gy)| {
                s.parse::<Address>().ok() == Some(*sender) && gx == x && gy == y
            }))
        }
        other => panic!("unexpected question {other:?}"),
    }
}
