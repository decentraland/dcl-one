//! The price store and routes over the process's scratch database
//! (`db::scratch`; without `CATALYST_TEST_DATABASE_URL` the DB-backed tests
//! skip). Override rows are keyed by a per-test token id and deleted at the
//! end; the one test that writes `price_snapshots` runs the poller against
//! an in-process stub and deletes its rows by their distinctive quotes.

use std::sync::{Arc, Mutex};

use axum::Router;
use axum::http::{StatusCode, header};
use serde_json::{Value, json};

use super::store::{Overrides, PriceSnapshot, Prices};
use super::test_support::{SNAPSHOTS, send, state_over};
use super::{RawSettings, Settings, poller, router};

fn lazy_pool() -> sqlx::PgPool {
    sqlx::postgres::PgPoolOptions::new()
        .acquire_timeout(std::time::Duration::from_millis(500))
        .connect_lazy("postgres://nobody@127.0.0.1:1/nowhere")
        .unwrap()
}

fn tag() -> String {
    format!("test-{}", uuid::Uuid::new_v4().simple())
}

async fn audit(pool: &sqlx::PgPool, token: &str) -> Vec<(String, Option<String>, String, Value)> {
    sqlx::query_as(
        "SELECT action, value::text, admin, detail FROM economy.price_override_audit \
         WHERE token_id = $1 AND vs_currency = 'usd' ORDER BY id",
    )
    .bind(token)
    .fetch_all(pool)
    .await
    .unwrap()
}

async fn cleanup_override(pool: &sqlx::PgPool, token: &str) {
    sqlx::query("DELETE FROM economy.price_overrides WHERE token_id = $1")
        .bind(token)
        .execute(pool)
        .await
        .unwrap();
    sqlx::query("DELETE FROM economy.price_override_audit WHERE token_id = $1")
        .bind(token)
        .execute(pool)
        .await
        .unwrap();
}

#[test]
fn debug_output_never_carries_the_console_bearer() {
    let settings = Settings {
        admin_token: Some("s3cret-bearer".into()),
        ..Settings::for_tests()
    };
    let printed = format!("{settings:?}");
    assert!(
        !printed.contains("s3cret-bearer") && printed.contains("admin_token: true"),
        "{printed}"
    );
    let raw = RawSettings {
        poll_enabled: false,
        coingecko_url: String::new(),
        poll_interval_secs: 1,
        fallback_sources: String::new(),
        admin_token: Some("s3cret-bearer".into()),
        writes_enabled: false,
    };
    assert!(!format!("{raw:?}").contains("s3cret-bearer"));
}

#[test]
fn settings_check_the_coingecko_base_only_when_polling() {
    let raw = |enabled: bool, url: &str| RawSettings {
        poll_enabled: enabled,
        coingecko_url: url.to_string(),
        poll_interval_secs: 0,
        fallback_sources: String::new(),
        admin_token: Some("  ".to_string()),
        writes_enabled: false,
    };
    let s = Settings::resolve(raw(false, ""), &[]).unwrap();
    assert_eq!(
        s.coingecko_url, "",
        "no production default: that is the mirror posture's"
    );
    let err = Settings::resolve(raw(true, " "), &[])
        .unwrap_err()
        .to_string();
    assert!(err.contains("with no COINGECKO_URL"), "{err}");
    assert_eq!(
        s.poll_interval,
        std::time::Duration::from_secs(1),
        "floored to one second"
    );
    assert!(s.admin_token.is_none(), "a blank token is unset");
    assert!(
        Settings::resolve(raw(false, "http://127.0.0.1:1/"), &[]).is_ok(),
        "an unused base is not checked"
    );
    let err = Settings::resolve(raw(true, "http://127.0.0.1:1/"), &[])
        .unwrap_err()
        .to_string();
    assert!(err.contains("COINGECKO_URL"), "{err}");
    let err = Settings::resolve(raw(true, "http://api.example.com/api/v3"), &[])
        .unwrap_err()
        .to_string();
    assert!(err.contains("https://"), "{err}");
    let err = Settings::resolve(
        raw(true, "https://node.example.com/api/v3"),
        &["node.example.com".to_string()],
    )
    .unwrap_err()
    .to_string();
    assert!(err.contains("own host"), "{err}");
    let s = Settings::resolve(raw(true, "https://api.coingecko.com/api/v3/"), &[]).unwrap();
    assert_eq!(s.coingecko_url, "https://api.coingecko.com/api/v3");
}

#[tokio::test]
async fn the_envelope_and_the_gate_need_no_database() {
    let app = router(state_over(lazy_pool(), Settings::for_tests()));
    let (status, _, body) = send(
        app.clone(),
        "GET",
        "/api/v3/simple/price?ids=decentraland",
        vec![],
        None,
    )
    .await;
    assert_eq!(
        (status, body["error"].as_str().unwrap()),
        (
            StatusCode::UNPROCESSABLE_ENTITY,
            "Missing parameter vs_currencies"
        )
    );
    let (status, _, body) = send(
        app.clone(),
        "GET",
        "/api/v3/simple/price?ids=decentraland&vs_currencies=usd&precision=40",
        vec![],
        None,
    )
    .await;
    assert_eq!(
        (status, body["error"].as_str().unwrap()),
        (StatusCode::BAD_REQUEST, "Invalid value for precision")
    );
    let (status, headers, body) = send(
        app.clone(),
        "PUT",
        "/admin/api/price/overrides/decentraland/usd",
        vec![],
        Some(json!({ "value": "1" })),
    )
    .await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(headers.get(header::RETRY_AFTER).unwrap(), "3600");
    assert!(
        body["message"]
            .as_str()
            .unwrap()
            .contains("DATA_WRITES_ENABLED=false")
    );
    let (status, _, _) = send(
        app,
        "DELETE",
        "/admin/api/price/overrides/decentraland/usd",
        vec![],
        None,
    )
    .await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);

    let enabled = Settings {
        writes_enabled: true,
        ..Settings::for_tests()
    };
    let app = router(state_over(lazy_pool(), enabled));
    let (status, _, body) = send(
        app.clone(),
        "PUT",
        "/admin/api/price/overrides/decentraland/usd",
        vec![],
        Some(json!({ "value": "1" })),
    )
    .await;
    assert_eq!(
        (status, body["error"].as_str().unwrap()),
        (StatusCode::FORBIDDEN, "Forbidden"),
        "no token configured fails closed"
    );
    let bearer = vec![("authorization", "Bearer x".to_string())];
    let (status, _, _) = send(
        app,
        "PUT",
        "/admin/api/price/overrides/decentraland/usd",
        bearer,
        Some(json!({ "value": "1" })),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    let resp = super::unavailable("nothing yet");
    assert_eq!(resp.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(resp.headers().get(header::RETRY_AFTER).unwrap(), "60");
}

#[tokio::test]
async fn override_set_and_clear_write_the_audit_in_the_same_statement() {
    let Some(pool) = crate::db::scratch::pool(4).await else {
        return;
    };
    let overrides = Overrides::new(pool.clone());
    let token = tag();
    let now = chrono::Utc::now();

    let set = overrides
        .set(&token, "usd", "0.25", Some("manual"), "alice", now)
        .await
        .unwrap();
    assert_eq!(set.token_id, token);
    assert_eq!(set.value, "0.25");
    assert_eq!(set.note.as_deref(), Some("manual"));
    assert_eq!(set.updated_by.as_deref(), Some("alice"));
    assert_eq!(set.updated_at.timestamp_micros(), now.timestamp_micros());

    let later = now + chrono::Duration::seconds(1);
    let again = overrides
        .set(&token, "usd", "0.5", None, "bob", later)
        .await
        .unwrap();
    assert_eq!(again.value, "0.5");
    assert_eq!(again.note, None);
    assert_eq!(again.updated_by.as_deref(), Some("bob"));
    assert!(again.updated_at > set.updated_at);
    let listed: Vec<_> = overrides
        .list()
        .await
        .unwrap()
        .into_iter()
        .filter(|o| o.token_id == token)
        .collect();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].value, "0.5");
    let pairs = overrides
        .for_pairs(
            &[token.clone(), "other".into()],
            &["usd".into(), "eth".into()],
        )
        .await
        .unwrap();
    assert_eq!(pairs.len(), 1);
    assert!(
        overrides
            .for_pairs(&[], &["usd".into()])
            .await
            .unwrap()
            .is_empty()
    );

    assert!(
        overrides
            .clear(&token, "usd", "carol", later)
            .await
            .unwrap()
    );
    assert!(
        !overrides
            .clear(&token, "usd", "carol", later)
            .await
            .unwrap()
    );
    assert!(
        overrides
            .list()
            .await
            .unwrap()
            .iter()
            .all(|o| o.token_id != token)
    );

    let rows = audit(&pool, &token).await;
    assert_eq!(
        rows.len(),
        3,
        "no audit row for a clear that removed nothing"
    );
    assert_eq!(rows[0].0, "override.set");
    assert_eq!(rows[0].1.as_deref(), Some("0.25"));
    assert_eq!(rows[0].2, "alice");
    assert_eq!(rows[0].3["note"], json!("manual"));
    assert_eq!(rows[1].1.as_deref(), Some("0.5"));
    assert_eq!(rows[1].3["note"], Value::Null);
    assert_eq!(rows[2].0, "override.clear");
    assert_eq!(rows[2].1, None);
    assert_eq!(rows[2].2, "carol");
    assert_eq!(
        rows[2].3,
        json!({ "token_id": token, "vs_currency": "usd" })
    );

    cleanup_override(&pool, &token).await;
}

#[tokio::test]
async fn the_console_routes_take_the_bearer_and_the_price_route_overlays() {
    let Some(pool) = crate::db::scratch::pool(4).await else {
        return;
    };
    let token = tag();
    let settings = Settings {
        writes_enabled: true,
        admin_token: Some("s3cret".into()),
        ..Settings::for_tests()
    };
    let state = state_over(pool.clone(), settings);
    let app = || router(state.clone());
    let path = format!("/admin/api/price/overrides/{}/USD", token.to_uppercase());
    let bearer = || {
        vec![
            ("authorization", "Bearer s3cret".to_string()),
            ("x-catalyrst-admin", "ops".to_string()),
        ]
    };

    let (status, _, body) = send(
        app(),
        "PUT",
        &path,
        vec![("authorization", "Bearer nope".to_string())],
        Some(json!({ "value": "1" })),
    )
    .await;
    assert_eq!(
        (status, body["error"].as_str().unwrap()),
        (StatusCode::FORBIDDEN, "Forbidden")
    );
    let (status, _, body) = send(
        app(),
        "PUT",
        &path,
        bearer(),
        Some(json!({ "value": "1e3" })),
    )
    .await;
    assert_eq!(
        (status, body["error"].as_str().unwrap()),
        (StatusCode::BAD_REQUEST, "value must be a decimal number")
    );
    let (status, _, body) = send(
        app(),
        "PUT",
        &path,
        bearer(),
        Some(json!({ "value": "0.75", "note": "hand-set" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        (
            body["token_id"].as_str().unwrap(),
            body["vs_currency"].as_str().unwrap(),
            body["value"].as_str().unwrap()
        ),
        (token.as_str(), "usd", "0.75")
    );
    assert_eq!(body["updated_by"], "ops");
    let (status, _, body) = send(app(), "GET", "/admin/api/price/overrides", vec![], None).await;
    assert_eq!(
        (status, body["error"].as_str()),
        (StatusCode::FORBIDDEN, Some("Forbidden")),
        "the listing carries operator identities: bearer required"
    );
    let (status, _, body) = send(
        app(),
        "GET",
        "/admin/api/price/overrides",
        vec![("authorization", "Bearer nope".to_string())],
        None,
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{body}");
    let (status, _, body) = send(app(), "GET", "/admin/api/price/overrides", bearer(), None).await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        body["overrides"]
            .as_array()
            .unwrap()
            .iter()
            .any(|o| o["token_id"] == token),
        "{body}"
    );

    // The price route serves the override for a token the archive never held...
    let (status, _, body) = send(
        app(),
        "GET",
        &format!("/api/v3/simple/price?ids={token}&vs_currencies=usd,eth&precision=2"),
        vec![],
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body, json!({ token.clone(): { "usd": 0.75 } }));
    // ...and nothing for an unknown token without one.
    let (status, _, body) = send(
        app(),
        "GET",
        "/api/v3/simple/price?ids=nothing-here&vs_currencies=usd",
        vec![],
        None,
    )
    .await;
    assert_eq!((status, body), (StatusCode::OK, json!({})));

    let (status, _, body) = send(app(), "DELETE", &path, bearer(), None).await;
    assert_eq!(
        (status, body["removed"].as_bool()),
        (StatusCode::OK, Some(true))
    );
    let (_, _, body) = send(app(), "DELETE", &path, bearer(), None).await;
    assert_eq!(body["removed"], false);
    assert_eq!(audit(&pool, &token).await.len(), 2);
    cleanup_override(&pool, &token).await;
}

/// A stub CoinGecko answering the configured document.
async fn stub_coingecko(answer: Arc<Mutex<Value>>) -> String {
    let app = Router::new().route(
        "/simple/price",
        axum::routing::get(move || {
            let answer = answer.clone();
            async move { axum::Json(answer.lock().unwrap().clone()) }
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    url
}

fn quote(usd: f64, ts: i64) -> Value {
    json!({
        "decentraland": { "usd": usd, "eth": 0.00012, "btc": 0.0000065, "usd_market_cap": 850000000.0,
                          "usd_24h_vol": 12345678.0, "usd_24h_change": -3.21, "last_updated_at": ts },
        "matic-network": { "usd": 0.84, "last_updated_at": ts }
    })
}

const MARKER_A: f64 = 0.42424242;
const MARKER_B: f64 = 0.51515151;

#[tokio::test]
async fn the_poller_appends_publishes_and_the_route_serves_the_newest_row() {
    let Some(pool) = crate::db::scratch::pool(4).await else {
        return;
    };
    let _serial = SNAPSHOTS.lock().await;
    let cleanup = || async {
        sqlx::query("DELETE FROM economy.price_snapshots WHERE source = 'coingecko' AND mana_usd IN ($1::numeric, $2::numeric)")
            .bind(MARKER_A.to_string())
            .bind(MARKER_B.to_string())
            .execute(&pool)
            .await
            .unwrap();
    };
    cleanup().await;

    let answer = Arc::new(Mutex::new(quote(MARKER_A, 1_700_000_000)));
    let base = stub_coingecko(answer.clone()).await;
    let http = crate::http::Outbound::for_tests();
    let live = Prices::new(pool.clone(), true);
    let db_only = Prices::new(pool.clone(), false);

    poller::run_pass(&http, &poller::Sources::coingecko_only(&base), &live)
        .await
        .unwrap();
    let published = live.live().expect("the pass publishes");
    assert_eq!(published.mana_usd, Some(MARKER_A));
    assert_eq!(
        published.source_updated_at.unwrap().timestamp(),
        1_700_000_000
    );
    let row = db_only.latest_snapshot().await.unwrap().unwrap();
    assert_eq!(
        row, *published,
        "the published snapshot is exactly the stored row"
    );

    // The cold-start DB read seeds the live cell; only the poller replaces it.
    let cold = Prices::new(pool.clone(), true);
    assert_eq!(
        cold.latest().await.unwrap().unwrap().mana_usd,
        Some(MARKER_A)
    );
    *answer.lock().unwrap() = quote(MARKER_B, 1_700_000_300);
    poller::run_pass(&http, &poller::Sources::coingecko_only(&base), &live)
        .await
        .unwrap();
    assert_eq!(
        cold.latest().await.unwrap().unwrap().mana_usd,
        Some(MARKER_A)
    );
    assert_eq!(
        live.latest().await.unwrap().unwrap().mana_usd,
        Some(MARKER_B)
    );
    assert_eq!(
        db_only.latest().await.unwrap().unwrap().mana_usd,
        Some(MARKER_B)
    );
    cold.publish(PriceSnapshot {
        mana_usd: Some(0.6),
        ..(*published).clone()
    });
    assert_eq!(cold.latest().await.unwrap().unwrap().mana_usd, Some(0.6));

    // A partial quote is refused before the insert.
    *answer.lock().unwrap() = json!({ "decentraland": { "usd": 0.1 } });
    let err = poller::run_pass(&http, &poller::Sources::coingecko_only(&base), &live)
        .await
        .unwrap_err()
        .to_string();
    assert!(err.contains("coingecko fetch failed"), "{err}");
    assert_eq!(
        db_only.latest().await.unwrap().unwrap().mana_usd,
        Some(MARKER_B)
    );

    // The route over a state without the poller reads the row.
    let app = router(state_over(pool.clone(), Settings::for_tests()));
    let (status, headers, body) = send(
        app.clone(),
        "GET",
        "/api/v3/simple/price?ids=decentraland,matic-network&vs_currencies=usd,eth,xyz&include_market_cap=true&include_24hr_vol=true&include_24hr_change=true&include_last_updated_at=true",
        vec![],
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        headers.get(header::CACHE_CONTROL).unwrap(),
        "max-age=30, public, must-revalidate, s-maxage=60"
    );
    let mana = &body["decentraland"];
    assert_eq!(mana["usd"], json!(MARKER_B));
    assert_eq!(mana["eth"], json!(0.00012));
    assert!(mana.get("xyz").is_none(), "an unknown currency is omitted");
    assert_eq!(mana["usd_market_cap"], json!(850000000.0));
    assert_eq!(mana["usd_24h_vol"], json!(12345678.0));
    assert_eq!(mana["usd_24h_change"], json!(-3.21));
    assert!(
        mana.get("eth_market_cap").is_none(),
        "market data is usd only"
    );
    assert_eq!(mana["last_updated_at"], json!(1_700_000_300));
    assert!(body.get("matic-network").is_none(), "only MANA is archived");
    let (_, _, body) = send(
        app.clone(),
        "GET",
        "/api/v3/simple/price?ids=decentraland&vs_currencies=usd&precision=2",
        vec![],
        None,
    )
    .await;
    assert_eq!(body["decentraland"]["usd"], json!(0.52));
    let (status, _, body) = send(
        app,
        "GET",
        "/api/v3/simple/price?ids=decentraland&vs_currencies=jpy",
        vec![],
        None,
    )
    .await;
    assert_eq!(
        (status, body),
        (StatusCode::OK, json!({ "decentraland": {} })),
        "a currency the archive never held keeps the old empty object"
    );

    cleanup().await;
}

#[test]
fn a_poll_interval_the_budget_cannot_cover_is_refused() {
    let raw = |secs: u64| RawSettings {
        poll_enabled: true,
        coingecko_url: "https://coingecko.example.org/api/v3".to_string(),
        poll_interval_secs: secs,
        fallback_sources: String::new(),
        admin_token: None,
        writes_enabled: false,
    };
    assert!(Settings::resolve(raw(300), &[]).is_ok());
    assert!(
        Settings::resolve(raw(900), &[]).is_ok(),
        "half the budget is the ceiling"
    );
    let err = Settings::resolve(raw(901), &[]).unwrap_err().to_string();
    assert!(
        err.contains("PRICE_POLL_INTERVAL_SECS=901") && err.contains("1800 s"),
        "{err}"
    );
    assert!(
        Settings::resolve(
            RawSettings {
                poll_enabled: false,
                ..raw(3600)
            },
            &[]
        )
        .is_ok(),
        "no poller, no interval to check"
    );
}

#[tokio::test]
async fn without_any_snapshot_the_mana_quote_is_the_envelope_not_an_empty_object() {
    let Some(pool) = crate::db::scratch::pool(2).await else {
        return;
    };
    let _serial = SNAPSHOTS.lock().await;
    let none: Option<i64> = sqlx::query_scalar(
        "SELECT id FROM economy.price_snapshots WHERE source = 'coingecko' LIMIT 1",
    )
    .fetch_optional(&pool)
    .await
    .unwrap();
    if none.is_some() {
        eprintln!(
            "another test's snapshot is present; the empty-table case is covered by the envelope unit check"
        );
        return;
    }
    let app = router(state_over(pool, Settings::for_tests()));
    let (status, headers, body) = send(
        app,
        "GET",
        "/api/v3/simple/price?ids=decentraland&vs_currencies=usd",
        vec![],
        None,
    )
    .await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{body}");
    assert_eq!(body["source"], "mana-price-poll");
    assert_eq!(body["configure"], "PRICE_POLL_ENABLED");
    assert_eq!(headers.get(header::RETRY_AFTER).unwrap(), "60");
}

#[test]
fn fallback_sources_resolve_in_order_and_refuse_an_unknown_exchange() {
    let raw = |sources: &str| RawSettings {
        poll_enabled: true,
        coingecko_url: "https://coingecko.example.org/api/v3".to_string(),
        poll_interval_secs: 300,
        fallback_sources: sources.to_string(),
        admin_token: None,
        writes_enabled: false,
    };
    let s = Settings::resolve(
        raw("binance=https://binance.example.org/, coinbase=https://coinbase.example.org"),
        &[],
    )
    .unwrap();
    assert_eq!(
        s.fallbacks,
        vec![
            (
                poller::Exchange::Binance,
                "https://binance.example.org".to_string()
            ),
            (
                poller::Exchange::Coinbase,
                "https://coinbase.example.org".to_string()
            ),
        ]
    );
    // No base is built in: a bare exchange name is refused.
    let err = Settings::resolve(raw("coinbase"), &[])
        .unwrap_err()
        .to_string();
    assert!(err.contains("names no API base"), "{err}");
    assert!(
        Settings::resolve(raw(""), &[])
            .unwrap()
            .fallbacks
            .is_empty()
    );
    let err = Settings::resolve(
        raw("coinbase=https://c.example.org,gemini=https://g.example.org"),
        &[],
    )
    .unwrap_err()
    .to_string();
    assert!(err.contains("gemini"), "{err}");
}

/// A stub CoinGecko refusing every request (its CDN's 403) beside a stub
/// exchange: `routes` maps each exchange path to its document.
async fn stub_upstreams(routes: Vec<(&'static str, Value)>) -> String {
    let mut app = Router::new().route(
        "/simple/price",
        axum::routing::get(|| async { StatusCode::FORBIDDEN }),
    );
    for (path, doc) in routes {
        app = app.route(
            path,
            axum::routing::get(move || {
                let doc = doc.clone();
                async move { axum::Json(doc) }
            }),
        );
    }
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    url
}

const MARKER_FALLBACK: f64 = 0.36363636;

#[tokio::test]
async fn a_refused_coingecko_falls_back_to_the_exchanges_in_order() {
    let Some(pool) = crate::db::scratch::pool(4).await else {
        return;
    };
    let _serial = SNAPSHOTS.lock().await;
    let cleanup = || async {
        sqlx::query("DELETE FROM economy.price_snapshots WHERE mana_usd = $1::numeric")
            .bind(MARKER_FALLBACK.to_string())
            .execute(&pool)
            .await
            .unwrap();
    };
    cleanup().await;
    let http = crate::http::Outbound::for_tests();
    let live = Prices::new(pool.clone(), true);

    // coinbase answers nothing usable (no route: 404), binance answers MANA and ETH
    let base = stub_upstreams(vec![(
        "/api/v3/ticker/price",
        json!([
            {"symbol": "MANAUSDT", "price": MARKER_FALLBACK.to_string()},
            {"symbol": "ETHUSDT", "price": "3000"}
        ]),
    )])
    .await;
    let sources = poller::Sources {
        coingecko: base.clone(),
        fallbacks: vec![
            (poller::Exchange::Coinbase, base.clone()),
            (poller::Exchange::Binance, base.clone()),
        ],
    };
    poller::run_pass(&http, &sources, &live).await.unwrap();
    let published = live.live().expect("the fallback row is published");
    assert_eq!(published.mana_usd, Some(MARKER_FALLBACK));
    assert_eq!(published.mana_eth, Some(MARKER_FALLBACK / 3000.0));
    assert_eq!(published.mana_btc, None);
    assert_eq!(published.market_cap_usd, None);
    let db_only = Prices::new(pool.clone(), false);
    assert_eq!(
        db_only.latest_snapshot().await.unwrap().unwrap(),
        *published
    );
    let source: String = sqlx::query_scalar(
        "SELECT source FROM economy.price_snapshots WHERE mana_usd = $1::numeric",
    )
    .bind(MARKER_FALLBACK.to_string())
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(source, "binance");

    // every source failing names each failure and writes nothing
    let err = poller::run_pass(
        &http,
        &poller::Sources {
            coingecko: base.clone(),
            fallbacks: vec![(poller::Exchange::Kraken, base.clone())],
        },
        &live,
    )
    .await
    .unwrap_err()
    .to_string();
    assert!(
        err.contains("coingecko fetch failed") && err.contains("kraken fetch failed"),
        "{err}"
    );
    cleanup().await;
}
