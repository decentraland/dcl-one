use std::sync::{Arc, Mutex};

use axum::Router;
use axum::body::Body;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, Method, Request, StatusCode};
use serde_json::{Value, json};
use tower::ServiceExt as _;

use super::*;
use crate::market::tests::support::{OwnDb, state_without_db_settings};
use crate::market::{MarketplaceState, Settings, market_router};
use crate::shared::signed_fetch::test_support::{address, signed_headers};

const MARKETPLACE: &str = r#"{"signer":"dcl:marketplace"}"#;
const WIDGET: &str = "https://global.transak.com/?session=example";

/// Transak as its docs describe it: each refresh issues a token and
/// invalidates the one before, and a call with any other token is a 401.
#[derive(Default)]
struct Fake {
    issued: Vec<String>,
    orders: Vec<HeaderMap>,
    widgets: Vec<Value>,
    widget_status: Option<StatusCode>,
    /// Refresh calls received, answered or not.
    refreshes: usize,
    refresh_delay: std::time::Duration,
    refresh_status: Option<StatusCode>,
    /// A 401 to every call, whatever the token (a key or environment mismatch).
    reject_all: bool,
}

type Shared = Arc<Mutex<Fake>>;

impl Fake {
    fn current(&self) -> Option<&str> {
        self.issued.last().map(String::as_str)
    }

    /// A refresh by another holder of the partner account (marketplace-server).
    fn refresh_elsewhere(fake: &Shared) {
        let mut fake = fake.lock().unwrap();
        let n = fake.issued.len();
        fake.issued.push(format!("token-elsewhere-{n}"));
    }
}

fn authorized(fake: &Fake, headers: &HeaderMap) -> bool {
    !fake.reject_all
        && headers.get("access-token").and_then(|v| v.to_str().ok()) == fake.current()
        && headers["x-api-key"] == "transak-api-key-example"
}

async fn fake_transak() -> (String, Shared) {
    let fake: Shared = Arc::default();
    async fn refresh(
        State(fake): State<Shared>,
        headers: HeaderMap,
        axum::Json(body): axum::Json<Value>,
    ) -> (StatusCode, axum::Json<Value>) {
        assert_eq!(headers["api-secret"], "transak-api-secret-example");
        assert_eq!(headers["x-api-key"], "transak-api-key-example");
        assert_eq!(body, json!({ "apiKey": "transak-api-key-example" }));
        let (delay, status) = {
            let mut fake = fake.lock().unwrap();
            fake.refreshes += 1;
            (fake.refresh_delay, fake.refresh_status)
        };
        tokio::time::sleep(delay).await;
        if let Some(status) = status {
            return (status, axum::Json(json!({ "error": "refresh-detail" })));
        }
        let mut fake = fake.lock().unwrap();
        let token = format!("token-{}", fake.issued.len());
        fake.issued.push(token.clone());
        let expires_at = chrono::Utc::now().timestamp() + 7 * 24 * 3600;
        (
            StatusCode::OK,
            axum::Json(json!({ "data": { "accessToken": token, "expiresAt": expires_at } })),
        )
    }
    async fn order(
        State(fake): State<Shared>,
        Path(id): Path<String>,
        headers: HeaderMap,
    ) -> (StatusCode, axum::Json<Value>) {
        let mut fake = fake.lock().unwrap();
        if !authorized(&fake, &headers) {
            return (
                StatusCode::UNAUTHORIZED,
                axum::Json(json!({ "error": "token-detail" })),
            );
        }
        fake.orders.push(headers);
        if id == "missing-order" {
            return (StatusCode::NOT_FOUND, axum::Json(json!({})));
        }
        let wallet = if id == "someone-elses" {
            "0x00000000000000000000000000000000000000ff".to_string()
        } else {
            address().to_uppercase().replacen("0X", "0x", 1)
        };
        (
            StatusCode::OK,
            axum::Json(json!({
                "meta": { "orderId": id },
                "data": {
                    "id": id,
                    "status": "COMPLETED",
                    "transactionHash": "0xabc",
                    "walletAddress": wallet,
                    "isNFTOrder": false,
                    "fiatAmount": 25,
                    "email": "jane.doe@example.com",
                    "paymentOptionId": "credit_debit_card",
                    "referenceCode": 123456,
                },
            })),
        )
    }
    async fn session(
        State(fake): State<Shared>,
        headers: HeaderMap,
        axum::Json(body): axum::Json<Value>,
    ) -> (StatusCode, axum::Json<Value>) {
        let mut fake = fake.lock().unwrap();
        if !authorized(&fake, &headers) {
            return (StatusCode::UNAUTHORIZED, axum::Json(json!({})));
        }
        fake.widgets.push(body);
        match fake.widget_status {
            Some(status) => (status, axum::Json(json!({ "error": "widget-detail" }))),
            None => (
                StatusCode::OK,
                axum::Json(json!({ "data": { "widgetUrl": WIDGET } })),
            ),
        }
    }
    let app = Router::new()
        .route("/v2/refresh-token", axum::routing::post(refresh))
        .route("/v2/order/{id}", axum::routing::get(order))
        .route("/gateway/v2/auth/session", axum::routing::post(session))
        .with_state(fake.clone());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    (url, fake)
}

fn settings(transak: Option<TransakSettings>) -> Settings {
    let mut settings = Settings::for_tests();
    settings.market.transak = transak;
    assert!(
        !settings.data_writes_enabled,
        "the routes answer while writes are off"
    );
    settings
}

/// One replica: a router over `pool`.
fn replica(pool: &sqlx::PgPool, url: &str) -> Router {
    market_router(Arc::new(MarketplaceState::new(
        pool.clone(),
        crate::http::Outbound::for_tests(),
        settings(Some(TransakSettings::for_tests(url))),
    )))
}

async fn send(
    app: Router,
    method: Method,
    path: &str,
    headers: HeaderMap,
    body: Option<Value>,
) -> (StatusCode, Value) {
    let mut request = Request::builder().method(method).uri(path);
    for (name, value) in &headers {
        request = request.header(name, value);
    }
    let body = match body {
        Some(body) => {
            request = request.header("content-type", "application/json");
            Body::from(body.to_string())
        }
        None => Body::empty(),
    };
    let response = app.oneshot(request.body(body).unwrap()).await.unwrap();
    let status = response.status();
    let bytes = axum::body::to_bytes(response.into_body(), 1 << 20)
        .await
        .unwrap();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

async fn read_order(app: Router, id: &str) -> (StatusCode, Value) {
    let path = format!("/v1/transak/orders/{id}");
    let headers = signed_headers("get", &path, MARKETPLACE, true).await;
    send(app, Method::GET, &path, headers, None).await
}

async fn widget(app: Router, body: Value) -> (StatusCode, Value) {
    send(app, Method::POST, WIDGET_PATH, HeaderMap::new(), Some(body)).await
}

/// The stored token, refreshed long enough ago that a 401 may replace it.
async fn age(pool: &sqlx::PgPool) {
    sqlx::query(
        "UPDATE shop.transak_access_token SET refreshed_at = refreshed_at - interval '1 minute'",
    )
    .execute(pool)
    .await
    .unwrap();
}

async fn stored(pool: &sqlx::PgPool) -> String {
    sqlx::query_scalar("SELECT access_token FROM shop.transak_access_token")
        .fetch_one(pool)
        .await
        .unwrap()
}

#[tokio::test]
async fn replicas_share_one_token_and_refresh_it_once() {
    let Some(db) = OwnDb::create("transak_shared").await else {
        return;
    };
    let (url, fake) = fake_transak().await;
    let (one, two) = (replica(&db.pool, &url), replica(&db.pool, &url));
    let (a, b, c) = tokio::join!(
        read_order(one.clone(), "order-1"),
        read_order(two.clone(), "order-2"),
        widget(one.clone(), json!({ "defaultNetwork": "polygon" })),
    );
    assert_eq!(
        (a.0, b.0, c.0),
        (StatusCode::OK, StatusCode::OK, StatusCode::OK),
        "{a:?} {b:?} {c:?}"
    );
    assert_eq!(
        fake.lock().unwrap().issued.len(),
        1,
        "concurrent first calls refresh once"
    );
    assert_eq!(read_order(two.clone(), "order-3").await.0, StatusCode::OK);
    assert_eq!(
        fake.lock().unwrap().issued.len(),
        1,
        "the other replica reads the stored token"
    );

    sqlx::query("UPDATE shop.transak_access_token SET expires_at = timezone('UTC', now()) + interval '30 seconds'")
        .execute(&db.pool)
        .await
        .unwrap();
    assert_eq!(read_order(one, "order-4").await.0, StatusCode::OK);
    assert_eq!(
        fake.lock().unwrap().issued.len(),
        2,
        "a token 60 s from its expiry is refreshed"
    );
    assert_eq!(stored(&db.pool).await, "token-1");
    db.drop().await;
}

#[tokio::test]
async fn a_refused_token_is_replaced_once_across_replicas() {
    let Some(db) = OwnDb::create("transak_refused").await else {
        return;
    };
    let (url, fake) = fake_transak().await;
    let (one, two) = (replica(&db.pool, &url), replica(&db.pool, &url));
    assert_eq!(read_order(one.clone(), "order-1").await.0, StatusCode::OK);

    age(&db.pool).await;
    Fake::refresh_elsewhere(&fake);
    let (status, body) = read_order(two.clone(), "order-2").await;
    assert_eq!(
        status,
        StatusCode::OK,
        "a 401 refreshes and sends once more: {body}"
    );
    assert_eq!(fake.lock().unwrap().issued.len(), 3);

    age(&db.pool).await;
    Fake::refresh_elsewhere(&fake);
    let (a, b) = tokio::join!(read_order(one, "order-3"), read_order(two, "order-4"));
    assert_eq!((a.0, b.0), (StatusCode::OK, StatusCode::OK), "{a:?} {b:?}");
    assert_eq!(
        fake.lock().unwrap().issued.len(),
        5,
        "two replicas refused the same token: one replaces it, the other uses the replacement"
    );
    assert_eq!(
        stored(&db.pool).await,
        fake.lock().unwrap().current().unwrap()
    );
    db.drop().await;
}

#[tokio::test]
async fn the_operator_forces_a_refresh_with_upstreams_bearer_answers() {
    let Some(db) = OwnDb::create("transak_force").await else {
        return;
    };
    let (url, fake) = fake_transak().await;
    let app = replica(&db.pool, &url);
    let put = |authorization: Option<&str>| {
        let mut headers = HeaderMap::new();
        if let Some(value) = authorization {
            headers.insert("authorization", value.parse().unwrap());
        }
        send(app.clone(), Method::PUT, TOKEN_PATH, headers, None)
    };
    for (authorization, message) in [
        (None, "Authorization header is missing"),
        (Some("Bearer wrong"), "Invalid authorization header"),
        (
            Some("Basic transak-refresh-bearer-example-0123456789"),
            "Invalid authorization header",
        ),
    ] {
        let (status, body) = put(authorization).await;
        assert_eq!(
            (status, body["message"].as_str()),
            (StatusCode::UNAUTHORIZED, Some(message)),
            "{authorization:?}"
        );
    }
    assert!(fake.lock().unwrap().issued.is_empty());
    for expected in ["token-0", "token-1"] {
        let (status, body) = put(Some("Bearer transak-refresh-bearer-example-0123456789")).await;
        assert_eq!((status, body), (StatusCode::OK, json!({ "ok": true })));
        assert_eq!(
            stored(&db.pool).await,
            expected,
            "a fresh token is replaced all the same"
        );
    }
    db.drop().await;
}

#[tokio::test]
async fn the_order_read_answers_its_wallet_the_fields_clients_read() {
    let Some(db) = OwnDb::create("transak_order").await else {
        return;
    };
    let (url, fake) = fake_transak().await;
    let app = replica(&db.pool, &url);

    let (status, body) = send(
        app.clone(),
        Method::GET,
        "/v1/transak/orders/order-1",
        HeaderMap::new(),
        None,
    )
    .await;
    assert_eq!(
        (status, body["message"].as_str()),
        (StatusCode::UNAUTHORIZED, Some("Unauthorized"))
    );
    let scene = signed_headers(
        "get",
        "/v1/transak/orders/order-1",
        r#"{"signer":"decentraland-kernel-scene"}"#,
        true,
    )
    .await;
    assert_eq!(
        send(
            app.clone(),
            Method::GET,
            "/v1/transak/orders/order-1",
            scene,
            None
        )
        .await
        .0,
        StatusCode::UNAUTHORIZED
    );
    let (status, body) = read_order(app.clone(), "order.1").await;
    assert_eq!(
        (status, body["message"].as_str()),
        (StatusCode::BAD_REQUEST, Some("Invalid order id"))
    );
    assert!(
        fake.lock().unwrap().issued.is_empty(),
        "no refusal reaches Transak, not even for a token"
    );

    let mut headers = signed_headers("get", "/v1/transak/orders/order-1", MARKETPLACE, true).await;
    headers.insert("x-real-ip", "198.51.100.7".parse().unwrap());
    let (status, body) = send(
        app.clone(),
        Method::GET,
        "/v1/transak/orders/order-1",
        headers,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        body,
        json!({ "ok": true, "data": {
            "meta": { "orderId": "order-1" },
            "data": {
                "id": "order-1",
                "status": "COMPLETED",
                "transactionHash": "0xabc",
                "walletAddress": address().to_uppercase().replacen("0X", "0x", 1),
                "errorMessage": null,
                "isNFTOrder": false,
            },
        }}),
        "projected: no payment or personal field"
    );
    assert_eq!(
        fake.lock().unwrap().orders.last().unwrap()["x-user-ip"],
        "198.51.100.7"
    );

    let (status, body) = read_order(app.clone(), "someone-elses").await;
    assert_eq!(
        (status, body["message"].as_str()),
        (StatusCode::UNAUTHORIZED, Some("Unauthorized"))
    );
    let (status, body) = read_order(app, "missing-order").await;
    assert_eq!(
        (status, body["message"].as_str()),
        (StatusCode::NOT_FOUND, Some("Order not found"))
    );
    db.drop().await;
}

#[tokio::test]
async fn the_widget_sends_the_callers_options_under_the_servers_own() {
    let Some(db) = OwnDb::create("transak_widget").await else {
        return;
    };
    let (url, fake) = fake_transak().await;
    let app = replica(&db.pool, &url);

    let (status, body) = send(
        app.clone(),
        Method::POST,
        WIDGET_PATH,
        [("content-type", "text/plain")]
            .into_iter()
            .map(|(k, v)| (k.parse().unwrap(), v.parse().unwrap()))
            .collect(),
        None,
    )
    .await;
    assert_eq!(
        (status, body["message"].as_str()),
        (
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            Some("Content-Type must be application/json")
        )
    );
    for invalid in [
        json!({ "apiKey": "the-callers" }),
        json!({ "fiatAmount": "25" }),
        json!({ "nftData": [{ "imageURL": "i", "nftName": "n", "collectionAddress": "c", "tokenID": ["1"], "price": [1], "quantity": 1 }] }),
        json!({ "nftData": [{ "imageURL": "i", "nftName": "n", "collectionAddress": "c", "tokenID": ["1"], "price": [1], "quantity": 1, "nftType": "ERC20" }] }),
        json!([]),
    ] {
        let (status, body) = widget(app.clone(), invalid.clone()).await;
        assert_eq!(
            (status, body["message"].as_str()),
            (StatusCode::BAD_REQUEST, Some("Invalid JSON body")),
            "{invalid}"
        );
    }
    assert!(fake.lock().unwrap().issued.is_empty());

    let wallet = address();
    let nft = json!({ "imageURL": "i", "nftName": "n", "collectionAddress": "c", "tokenID": ["1"], "price": [1.5], "quantity": 1, "nftType": "ERC721", "extra": true });
    let (status, body) = widget(
        app.clone(),
        json!({ "defaultNetwork": "polygon", "walletAddress": wallet, "fiatAmount": 25, "cryptoCurrencyCode": "ETH", "email": null, "nftData": [nft] }),
    )
    .await;
    assert_eq!(
        (status, body),
        (StatusCode::OK, json!({ "ok": true, "data": WIDGET }))
    );
    assert_eq!(
        fake.lock().unwrap().widgets.pop().unwrap(),
        json!({ "widgetParams": {
            "defaultNetwork": "polygon",
            "walletAddress": wallet,
            "fiatAmount": 25,
            "email": null,
            "nftData": [nft],
            "apiKey": "transak-api-key-example",
            "referrerDomain": "shop.example.com",
            "networks": "ethereum,polygon",
            "cryptoCurrencyCode": "MANA",
            "defaultCryptoCurrency": "MANA",
            "cyptoCurrencyList": "MANA",
        }})
    );

    fake.lock().unwrap().widget_status = Some(StatusCode::BAD_REQUEST);
    let (status, body) = widget(app, json!({})).await;
    assert_eq!(
        (status, body["message"].as_str()),
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Some("Error getting widget, status: 400")
        )
    );
    assert!(!body.to_string().contains("widget-detail"));
    db.drop().await;
}

#[tokio::test]
async fn an_unconfigured_node_names_the_six_settings_on_every_route() {
    let app = market_router(state_without_db_settings(settings(None)));
    for (method, path) in [
        (Method::POST, WIDGET_PATH),
        (Method::GET, "/v1/transak/orders/order-1"),
        (Method::PUT, TOKEN_PATH),
    ] {
        let body = (method == Method::POST).then(|| json!({}));
        let (status, body) = send(app.clone(), method.clone(), path, HeaderMap::new(), body).await;
        assert_eq!(
            status,
            StatusCode::SERVICE_UNAVAILABLE,
            "{method} {path}: {body}"
        );
        assert_eq!(body["source"], "transak");
        assert_eq!(
            body["configure"],
            "TRANSAK_API_URL, TRANSAK_API_GATEWAY_URL, TRANSAK_API_KEY, TRANSAK_API_SECRET, TRANSAK_REFRESH_ACCESS_TOKEN_AUTH, TRANSAK_REFERRER_DOMAIN"
        );
    }
}

#[tokio::test]
async fn both_mounts_serve_the_routes_and_bind_the_signature_to_the_incoming_path() {
    for mount in ["/shop-api", "/marketplace-api"] {
        // No database: a request past the signature fails at the token store (502).
        let state = state_without_db_settings(settings(Some(TransakSettings::for_tests(
            "http://127.0.0.1:9",
        ))));
        let app = Router::new().nest(mount, market_router(state));
        let path = format!("{mount}/v1/transak/orders/order-1");
        let headers = signed_headers("get", &path, MARKETPLACE, true).await;
        assert_eq!(
            send(app.clone(), Method::GET, &path, headers, None).await.0,
            StatusCode::BAD_GATEWAY,
            "{mount}"
        );
        let headers = signed_headers("get", "/shop-api/v1/other", MARKETPLACE, true).await;
        assert_eq!(
            send(app, Method::GET, &path, headers, None).await.0,
            StatusCode::UNAUTHORIZED,
            "{mount}"
        );
    }
}

#[tokio::test]
async fn the_pending_router_no_longer_claims_the_routes() {
    // `shop::router` merges both: two routers owning one path panic there.
    let app = Router::new()
        .merge(market_router(state_without_db_settings(settings(None))))
        .merge(crate::pending_router());
    let (status, body) = send(
        app,
        Method::POST,
        WIDGET_PATH,
        HeaderMap::new(),
        Some(json!({})),
    )
    .await;
    assert_eq!(
        (status, body["source"].as_str()),
        (StatusCode::SERVICE_UNAVAILABLE, Some("transak"))
    );
}

#[test]
fn settings_need_all_six_and_never_echo_a_secret() {
    let all = |referrer: &str, url: &str| {
        TransakSettings::parse(
            Some(url),
            Some("https://gateway.example.com/"),
            Some("api-key-example"),
            Some("api-secret-example"),
            Some("refresh-bearer-example"),
            Some(referrer),
            &[],
        )
    };
    assert!(
        TransakSettings::parse(None, None, Some(" "), None, None, None, &[])
            .unwrap()
            .is_none()
    );
    let partial = TransakSettings::parse(
        Some("https://api.example.com"),
        None,
        Some("api-key-example"),
        None,
        None,
        None,
        &[],
    )
    .unwrap_err()
    .to_string();
    assert!(partial.contains("missing: TRANSAK_API_GATEWAY_URL, TRANSAK_API_SECRET, TRANSAK_REFRESH_ACCESS_TOKEN_AUTH, TRANSAK_REFERRER_DOMAIN"), "{partial}");
    assert!(!partial.contains("api-key-example"), "{partial}");
    assert!(
        all("shop.example.com", "http://api.example.com")
            .unwrap_err()
            .to_string()
            .contains("TRANSAK_API_URL")
    );
    for referrer in [
        "https://shop.example.com",
        "shop.example.com/path",
        "shop.example.com:443",
        "localhost",
        "-bad.example.com",
    ] {
        let error = all(referrer, "https://api.example.com")
            .unwrap_err()
            .to_string();
        assert!(
            error.contains("TRANSAK_REFERRER_DOMAIN"),
            "{referrer}: {error}"
        );
    }
    let settings = all("Shop.Example.com", "https://api.example.com/")
        .unwrap()
        .unwrap();
    assert_eq!(
        (settings.api_url.as_str(), settings.gateway_url.as_str()),
        ("https://api.example.com", "https://gateway.example.com")
    );
    assert_eq!(settings.referrer_domain, "shop.example.com");
    let debug = format!("{settings:?}");
    for secret in [
        "api-key-example",
        "api-secret-example",
        "refresh-bearer-example",
    ] {
        assert!(!debug.contains(secret), "{debug}");
    }
}

#[test]
fn json_content_types_and_order_ids_follow_upstream() {
    let with = |value: &str| -> HeaderMap {
        [(axum::http::header::CONTENT_TYPE, value.parse().unwrap())]
            .into_iter()
            .collect()
    };
    for json in [
        "application/json",
        "application/json; charset=utf-8",
        "Application/JSON",
        "application/vnd.api+json",
    ] {
        assert!(is_json(&with(json)), "{json}");
    }
    for other in [
        "text/plain",
        "application/+json",
        "+json",
        "application/jsonp",
    ] {
        assert!(!is_json(&with(other)), "{other}");
    }
    assert!(!is_json(&HeaderMap::new()));
    assert!(is_order_id("0f8e1c2a-7b5d-4e2f-9a3b-1c2d3e4f5a6b"));
    for invalid in ["", "a.b", "a/b", "a b", &"x".repeat(65)] {
        assert!(!is_order_id(invalid), "{invalid}");
    }
}

#[test]
fn the_open_paths_keep_every_historical_read() {
    for path in crate::market::historical::READ_PATHS {
        assert!(crate::market::OPEN_PATHS.contains(path), "{path}");
    }
    assert!(
        crate::market::OPEN_PATHS.contains(&WIDGET_PATH)
            && crate::market::OPEN_PATHS.contains(&TOKEN_PATH)
    );
}

#[tokio::test]
async fn a_persistent_401_refreshes_at_most_once_per_30_seconds() {
    let Some(db) = OwnDb::create("transak_401").await else {
        return;
    };
    let (url, fake) = fake_transak().await;
    fake.lock().unwrap().reject_all = true;
    let app = replica(&db.pool, &url);
    for id in ["order-1", "order-2", "order-3"] {
        let (status, body) = read_order(app.clone(), id).await;
        assert_eq!(
            (status, body["message"].as_str()),
            (
                StatusCode::BAD_GATEWAY,
                Some("Transak order could not be read")
            ),
            "{id}"
        );
    }
    assert_eq!(
        fake.lock().unwrap().refreshes,
        1,
        "a 401 on a token minted moments ago is not a stale token: no refresh loop"
    );
    age(&db.pool).await;
    assert_eq!(read_order(app, "order-4").await.0, StatusCode::BAD_GATEWAY);
    assert_eq!(
        fake.lock().unwrap().refreshes,
        2,
        "30 s later, one more try"
    );
    db.drop().await;
}

#[tokio::test]
async fn a_failed_refresh_is_remembered_and_backs_the_other_replicas_off() {
    let Some(db) = OwnDb::create("transak_failed").await else {
        return;
    };
    let (url, fake) = fake_transak().await;
    fake.lock().unwrap().refresh_status = Some(StatusCode::INTERNAL_SERVER_ERROR);
    let app = replica(&db.pool, &url);
    let calls = (0..6).map(|_| widget(app.clone(), json!({})));
    for (status, body) in futures::future::join_all(calls).await {
        assert_eq!(
            (status, body["message"].as_str()),
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Some("Error refreshing access token, status: 500")
            ),
        );
        assert!(!body.to_string().contains("refresh-detail"));
    }
    assert_eq!(
        fake.lock().unwrap().refreshes,
        1,
        "the queued callers fail without calling Transak"
    );
    assert_eq!(
        widget(app, json!({})).await.0,
        StatusCode::INTERNAL_SERVER_ERROR
    );
    assert_eq!(
        fake.lock().unwrap().refreshes,
        1,
        "nor does the next caller, for 10 s"
    );
    let backing_off: bool = sqlx::query_scalar(
        "SELECT refreshing_until > timezone('UTC', now()) + interval '5 seconds' FROM shop.transak_access_token",
    )
    .fetch_one(&db.pool)
    .await
    .unwrap();
    assert!(
        backing_off,
        "the claim stays, so the other replicas back off too"
    );
    db.drop().await;
}

#[tokio::test]
async fn callers_waiting_on_a_slow_refresh_hold_no_connection() {
    let Some(db) = OwnDb::create("transak_slow").await else {
        return;
    };
    let (url, fake) = fake_transak().await;
    fake.lock().unwrap().refresh_delay = std::time::Duration::from_millis(2500);
    // Two connections in all, and a caller that cannot get one within a second fails.
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(2)
        .acquire_timeout(std::time::Duration::from_secs(1))
        .connect_with((*db.pool.connect_options()).clone())
        .await
        .unwrap();
    let (one, two) = (replica(&pool, &url), replica(&pool, &url));
    let calls = (0..8).map(|i| {
        let app = if i % 2 == 0 { one.clone() } else { two.clone() };
        read_order(app, ["order-a", "order-b", "order-c", "order-d"][i % 4])
    });
    for (status, body) in futures::future::join_all(calls).await {
        assert_eq!(status, StatusCode::OK, "{body}");
    }
    assert_eq!(fake.lock().unwrap().refreshes, 1);
    pool.close().await;
    db.drop().await;
}

#[test]
fn expires_at_in_any_shape_keeps_the_new_token() {
    let now = chrono::Utc::now().timestamp();
    let at = |v: Option<Value>| expiry(v.as_ref()).and_utc().timestamp();
    let week = now + 7 * 24 * 3600;
    assert_eq!(at(Some(json!(week))), week);
    assert_eq!(at(Some(json!(week as f64 + 0.75))), week);
    assert_eq!(at(Some(json!(week.to_string()))), week);
    assert_eq!(at(Some(json!(week * 1000))), week, "milliseconds");
    for unreadable in [
        None,
        Some(json!(null)),
        Some(json!("soon")),
        Some(json!(now - 60)),
        Some(json!(-1)),
    ] {
        let hour = at(unreadable.clone()) - now;
        assert!((3590..=3610).contains(&hour), "{unreadable:?}: {hour}");
    }
}
