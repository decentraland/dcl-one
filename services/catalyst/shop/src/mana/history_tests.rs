use super::*;
use axum::{Json, Router, extract::State, routing::post};
use serde_json::Value;
use std::sync::atomic::{AtomicBool, Ordering};

const HASH: B256 = B256::repeat_byte(3);

struct Feed {
    fail_reads: AtomicBool,
    deny_empty_phase: AtomicBool,
    start: i64,
}

fn data(words: &[u64]) -> Value {
    let body: String = words.iter().map(|value| format!("{value:064x}")).collect();
    json!({ "result": format!("0x{body}") })
}

fn round(id: u64, start: i64) -> Value {
    let time = u64::try_from(start).unwrap() + 90 * id;
    data(&[id, 125_000_000, time, time, id])
}

async fn call(State(feed): State<Arc<Feed>>, Json(request): Json<Value>) -> Json<Value> {
    let params = &request["params"];
    let reply = match request["method"].as_str().unwrap() {
        "eth_chainId" => json!({"result":"0x89"}),
        "eth_getBlockByNumber" => json!({
            "result":{"hash":format!("{HASH:#x}"),"timestamp":format!("0x{:x}", feed.start + 86_400 * 90)}
        }),
        "eth_call" => {
            assert_eq!(
                params[1],
                json!({"blockHash":format!("{HASH:#x}"),"requireCanonical":true})
            );
            let to: Address = params[0]["to"].as_str().unwrap().parse().unwrap();
            let call = decode_hex(params[0]["data"].as_str().unwrap()).unwrap();
            let selector = &call[..4];
            let argument = || u64::from_be_bytes(call[28..36].try_into().unwrap());
            match selector {
                s if s == phaseIdCall::SELECTOR => data(&[3]),
                s if s == phaseAggregatorsCall::SELECTOR => data(&[argument()]),
                s if s == decimalsCall::SELECTOR => data(&[8]),
                s if s == latestRoundDataCall::SELECTOR => {
                    if to[19] == 1 {
                        round(65_536, feed.start)
                    } else {
                        round(2, feed.start + 86_400 * 30)
                    }
                }
                s if s == getRoundDataCall::SELECTOR => {
                    let id = argument();
                    if to[19] == 2 {
                        let message = if feed.deny_empty_phase.load(Ordering::SeqCst) {
                            "execution reverted: No access"
                        } else {
                            "execution reverted: No data present"
                        };
                        json!({"error":{"code":3,"message":message}})
                    } else if id > 1 && feed.fail_reads.load(Ordering::SeqCst) {
                        json!({"error":{"code":-32005,"message":"upstream timeout"}})
                    } else {
                        if id > 1 {
                            tokio::time::sleep(Duration::from_millis(12)).await;
                        }
                        let start = if to[19] == 1 {
                            feed.start
                        } else {
                            feed.start + 86_400 * 30
                        };
                        round(id, start)
                    }
                }
                _ => panic!("unexpected oracle selector"),
            }
        }
        _ => panic!("unexpected RPC method"),
    };
    let mut reply = reply;
    reply["jsonrpc"] = json!("2.0");
    reply["id"] = request["id"].clone();
    Json(reply)
}

#[tokio::test]
async fn history_keeps_completed_days_across_latency_and_rpc_failure() {
    let Some(db) = crate::market::tests::support::OwnDb::create("mana_usd_history").await else {
        return;
    };
    let feed = Arc::new(Feed {
        fail_reads: AtomicBool::new(false),
        deny_empty_phase: AtomicBool::new(true),
        start: FIRST_DAY
            .and_hms_opt(0, 0, 0)
            .unwrap()
            .and_utc()
            .timestamp(),
    });
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let app = Router::new()
        .route("/", post(call))
        .with_state(feed.clone());
    let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let rpc = Rpc::new(
        crate::http::Outbound::for_tests(),
        &format!("http://{address}"),
        Duration::from_secs(2),
    );
    let oracle = Oracle {
        rpc: &rpc,
        hash: HASH,
    };
    let today = FIRST_DAY
        .checked_add_days(chrono::Days::new(BATCH_DAYS as u64))
        .unwrap();
    let count = || sqlx::query_scalar::<_, i64>("SELECT count(*) FROM shop.mana_usd_daily");
    assert!(
        fill(&db.pool, &rpc, today).await.is_err(),
        "access refusal is not missing data"
    );
    assert_eq!(count().fetch_one(&db.pool).await.unwrap(), 0);
    feed.deny_empty_phase.store(false, Ordering::SeqCst);
    let phases = oracle.phases(feed.start + 86_400 * 90).await.unwrap();
    assert_eq!(
        phases.iter().map(|phase| phase.id).collect::<Vec<_>>(),
        [1, 3]
    );
    let bounded = tokio::time::timeout(Duration::from_secs(1), fill(&db.pool, &rpc, today)).await;
    assert!(bounded.is_err(), "the slow batch should exceed its budget");
    let completed = count().fetch_one(&db.pool).await.unwrap();
    assert!(
        (1..BATCH_DAYS as i64).contains(&completed),
        "completed {completed}"
    );
    feed.fail_reads.store(true, Ordering::SeqCst);
    assert!(fill(&db.pool, &rpc, today).await.is_err());
    assert_eq!(count().fetch_one(&db.pool).await.unwrap(), completed);
    feed.fail_reads.store(false, Ordering::SeqCst);
    fill(&db.pool, &rpc, today).await.unwrap();
    assert_eq!(
        count().fetch_one(&db.pool).await.unwrap(),
        BATCH_DAYS as i64
    );
    let first: (NaiveDate, String) =
        sqlx::query_as("SELECT day,usd::text FROM shop.mana_usd_daily ORDER BY day LIMIT 1")
            .fetch_one(&db.pool)
            .await
            .unwrap();
    assert_eq!(first, (FIRST_DAY, "1.25".to_owned()));
    server.abort();
    db.drop().await;
}

#[tokio::test]
async fn the_history_job_accepts_mainnet_aliases() {
    for network in ["mainnet", "ethereum", "ethereum-mainnet", "sepolia"] {
        let mut settings = crate::market::Settings::for_tests();
        settings.eth_network = network.to_string();
        settings.market.indexes.sales_enabled = true;
        settings.polygon_rpc_url = Some("https://polygon.example".to_owned());
        let state = crate::market::tests::support::state_without_db_settings(settings);
        assert_eq!(
            history_endpoint(&state).is_some(),
            network != "sepolia",
            "{network}"
        );
    }
}
