//! The rentals listing store and routes over the process's scratch database
//! (`db::scratch`; without `CATALYST_TEST_DATABASE_URL` the DB-backed tests
//! skip), with random token ids so every row is this test's and deleted at
//! the end (the asset row cascades to the rental, the listing and the
//! periods), and an in-process stub standing in for the marketplace
//! subgraph. `CATALYST_LIVE_MARKETPLACE_SUBGRAPH_URL` gates the one real
//! upstream read.

use std::sync::{Arc, Mutex};

use alloy::primitives::B256;
use alloy::signers::{Signer as _, local::PrivateKeySigner};
use axum::Router;
use axum::body::{Body, to_bytes};
use axum::http::{HeaderMap, Request, StatusCode, header};
use serde_json::{Value, json};
use tower::ServiceExt;

use super::db::{Database, ListingMetadata};
use super::signature::listing_digest;
use super::types::{ContractRentalListing, RentalListingCreation, RentalListingPeriodInput};
use super::{Settings, SharedState, State, router};
use crate::auth::auth_chain::{AuthLink, AuthLinkType};
use crate::market::MarketplaceState;
use crate::shared::signed_fetch::{
    AUTH_CHAIN_HEADER_PREFIX, AUTH_METADATA_HEADER, AUTH_TIMESTAMP_HEADER, build_payload_v6,
};

const LAND: &str = "0xf87e31492faf9a91b02ee0deaad50d51d56d5d4d";
const RENTALS_GOERLI: &str = "0x92159c78f0f4523b9c60382bb888f30f10a46b3b";
const LESSOR: &str = "0xf39fd6e51aad88f6f4ce6ab8827279cfffb92266";

fn lazy_pool() -> sqlx::PgPool {
    sqlx::postgres::PgPoolOptions::new()
        .acquire_timeout(std::time::Duration::from_millis(500))
        .connect_lazy("postgres://nobody@127.0.0.1:1/nowhere")
        .unwrap()
}

fn random_token_id() -> String {
    let bytes = uuid::Uuid::new_v4().as_u128();
    (bytes >> 4).to_string()
}

fn creation(token_id: &str, signature: &str) -> RentalListingCreation {
    RentalListingCreation {
        network: "ETHEREUM".into(),
        chain_id: 5,
        expiration: 4_102_444_800_000,
        contract_address: LAND.into(),
        token_id: token_id.into(),
        nonces: vec!["0".into(), "1".into(), "2".into()],
        periods: vec![
            RentalListingPeriodInput {
                min_days: 30,
                max_days: 60,
                price_per_day: "2000000000000000000".into(),
            },
            RentalListingPeriodInput {
                min_days: 7,
                max_days: 14,
                price_per_day: "1000000000000000000".into(),
            },
        ],
        rental_contract_address: RENTALS_GOERLI.into(),
        signature: signature.into(),
        target: "0x0000000000000000000000000000000000000000".into(),
    }
}

async fn insert(
    db: &Database,
    token_id: &str,
    lessor: &str,
    signature: &str,
) -> Result<super::types::RentalListing, sqlx::Error> {
    let nft_id = format!("{LAND}-{token_id}");
    let now = chrono::Utc::now().naive_utc();
    db.insert_listing(
        &nft_id,
        &ListingMetadata {
            category: "parcel",
            search_text: "-10,20",
            distance_to_plaza: Some(3),
            adjacent_to_road: Some(true),
            estate_size: None,
            updated_at: now,
        },
        now,
        &creation(token_id, signature),
        lessor,
        now,
    )
    .await
}

async fn cleanup(pool: &sqlx::PgPool, token_ids: &[String]) {
    for token in token_ids {
        sqlx::query("DELETE FROM signatures.rental_assets WHERE contract_address = $1 AND token_id = $2::numeric")
            .bind(LAND)
            .bind(token)
            .execute(pool)
            .await
            .unwrap();
    }
}

async fn counts(pool: &sqlx::PgPool, token_id: &str) -> (i64, i64, i64, i64) {
    sqlx::query_as(
        "SELECT \
           (SELECT count(*) FROM signatures.rental_assets WHERE contract_address = $1 AND token_id = $2::numeric), \
           (SELECT count(*) FROM signatures.rentals WHERE contract_address = $1 AND token_id = $2::numeric), \
           (SELECT count(*) FROM signatures.rental_listings l JOIN signatures.rentals r ON r.id = l.id WHERE r.contract_address = $1 AND r.token_id = $2::numeric), \
           (SELECT count(*) FROM signatures.rental_periods p JOIN signatures.rentals r ON r.id = p.rental_id WHERE r.contract_address = $1 AND r.token_id = $2::numeric)",
    )
    .bind(LAND)
    .bind(token_id)
    .fetch_one(pool)
    .await
    .unwrap()
}

#[tokio::test]
async fn insert_listing_lands_all_rows_in_one_statement() {
    let Some(pool) = crate::db::scratch::pool(4).await else {
        return;
    };
    let db = Database::new(pool.clone());
    let token = random_token_id();

    let listing = insert(&db, &token, LESSOR, &format!("0xsig-{token}"))
        .await
        .unwrap();
    assert_eq!(listing.nft_id, format!("{LAND}-{token}"));
    assert_eq!(listing.category, "parcel");
    assert_eq!(listing.search_text, "-10,20");
    assert_eq!(listing.lessor.as_deref(), Some(LESSOR));
    assert_eq!(listing.tenant, None);
    assert_eq!(listing.status, "open");
    assert_eq!(listing.chain_id, 5);
    assert_eq!(listing.expiration, 4_102_444_800_000);
    assert_eq!(listing.nonces, vec!["0", "1", "2"]);
    assert_eq!(listing.token_id, token);
    assert_eq!(listing.contract_address, LAND);
    assert!(listing.created_at > 0 && listing.updated_at > 0);
    let periods: Vec<(i64, i64, &str)> = listing
        .periods
        .iter()
        .map(|p| (p.min_days, p.max_days, p.price_per_day.as_str()))
        .collect();
    assert_eq!(
        periods,
        vec![
            (7, 14, "1000000000000000000"),
            (30, 60, "2000000000000000000")
        ]
    );

    let same = db.get_listing_by_id(&listing.id).await.unwrap().unwrap();
    assert_eq!(
        serde_json::to_value(&same).unwrap(),
        serde_json::to_value(&listing).unwrap()
    );
    assert_eq!(counts(&pool, &token).await, (1, 1, 1, 2));

    let err = insert(&db, &token, LESSOR, &format!("0xsig-{token}-again"))
        .await
        .unwrap_err();
    assert!(Database::is_open_conflict(&err), "{err}");
    assert_eq!(
        counts(&pool, &token).await,
        (1, 1, 1, 2),
        "a conflicting insert rolls back every row"
    );
    let other = random_token_id();
    let err = insert(&db, &other, LESSOR, &format!("0xsig-{token}"))
        .await
        .unwrap_err();
    assert_eq!(
        Database::conflict(&err),
        Some(super::db::Conflict::Signature),
        "{err}"
    );
    assert_eq!(
        counts(&pool, &other).await,
        (0, 0, 0, 0),
        "the asset upsert rolls back with the rental"
    );

    cleanup(&pool, std::slice::from_ref(&token)).await;
    assert_eq!(
        counts(&pool, &token).await,
        (0, 0, 0, 0),
        "the asset row cascades"
    );
}

#[tokio::test]
async fn refresh_updates_metadata_and_returns_the_listing() {
    let Some(pool) = crate::db::scratch::pool(4).await else {
        return;
    };
    let db = Database::new(pool.clone());
    let token = random_token_id();
    let listing = insert(&db, &token, LESSOR, &format!("0xsig-{token}"))
        .await
        .unwrap();

    assert_eq!(
        db.listing_asset(&listing.id).await.unwrap(),
        Some((LAND.to_string(), token.clone()))
    );
    assert_eq!(db.listing_asset("not-a-uuid").await.unwrap(), None);
    assert_eq!(
        db.listing_asset(&uuid::Uuid::new_v4().to_string())
            .await
            .unwrap(),
        None
    );

    let later = chrono::Utc::now().naive_utc();
    let meta = ListingMetadata {
        category: "estate",
        search_text: "Big estate",
        distance_to_plaza: Some(1),
        adjacent_to_road: Some(false),
        estate_size: Some(4),
        updated_at: later,
    };
    let refreshed = db
        .refresh_metadata_for_rental(&listing.id, &meta, later)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(refreshed.id, listing.id);
    assert_eq!(refreshed.category, "estate");
    assert_eq!(refreshed.search_text, "Big estate");
    assert_eq!(refreshed.periods.len(), 2);
    assert_eq!(refreshed.lessor.as_deref(), Some(LESSOR));

    let stored = db.get_listing_by_id(&listing.id).await.unwrap().unwrap();
    assert_eq!(
        serde_json::to_value(&stored).unwrap(),
        serde_json::to_value(&refreshed).unwrap()
    );
    let (distance, adjacent, size): (Option<i16>, Option<bool>, Option<i16>) = sqlx::query_as(
        "SELECT distance_to_plaza, adjacent_to_road, estate_size FROM signatures.rental_assets WHERE contract_address = $1 AND token_id = $2::numeric",
    )
    .bind(LAND)
    .bind(&token)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!((distance, adjacent, size), (Some(1), Some(false), Some(4)));

    assert!(
        db.refresh_metadata_for_rental(&uuid::Uuid::new_v4().to_string(), &meta, later)
            .await
            .unwrap()
            .is_none()
    );
    cleanup(&pool, &[token]).await;
}

/// A stub marketplace subgraph: every POST answers the current document.
struct StubGraph {
    answer: Arc<Mutex<Value>>,
    url: String,
}

async fn stub_graph() -> StubGraph {
    let answer = Arc::new(Mutex::new(json!({ "data": { "nfts": [] } })));
    let served = answer.clone();
    let app = Router::new().route(
        "/",
        axum::routing::post(move || {
            let served = served.clone();
            async move { axum::Json(served.lock().unwrap().clone()) }
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    StubGraph { answer, url }
}

fn market_with(subgraph_url: Option<String>) -> Arc<MarketplaceState> {
    let mut settings = crate::market::Settings::for_tests();
    settings.marketplace_subgraph_url = subgraph_url;
    let state = MarketplaceState::new(lazy_pool(), crate::http::Outbound::for_tests(), settings);
    state
        .test_allow_loopback
        .store(true, std::sync::atomic::Ordering::Relaxed);
    Arc::new(state)
}

fn state_over(
    pool: sqlx::PgPool,
    market: Option<Arc<MarketplaceState>>,
    writes_enabled: bool,
) -> SharedState {
    Arc::new(State::new(pool, market, Settings { writes_enabled }, None))
}

struct Wallet {
    signer: PrivateKeySigner,
    address: String,
}

fn wallet() -> Wallet {
    let signer = PrivateKeySigner::random();
    let address = format!("{:#x}", signer.address());
    Wallet { signer, address }
}

async fn signed_headers(w: &Wallet, method: &str, path: &str) -> Vec<(String, String)> {
    let timestamp = chrono::Utc::now().timestamp_millis().to_string();
    let payload = build_payload_v6(method, path, &timestamp, "{}");
    let signature = w
        .signer
        .sign_message(payload.as_bytes())
        .await
        .unwrap()
        .to_string();
    let chain = [
        AuthLink {
            link_type: AuthLinkType::SIGNER,
            payload: w.address.clone(),
            signature: None,
        },
        AuthLink {
            link_type: AuthLinkType::EcdsaSignedEntity,
            payload,
            signature: Some(signature),
        },
    ];
    let mut out = vec![
        (AUTH_TIMESTAMP_HEADER.to_string(), timestamp),
        (AUTH_METADATA_HEADER.to_string(), "{}".to_string()),
    ];
    for (i, link) in chain.iter().enumerate() {
        out.push((
            format!("{AUTH_CHAIN_HEADER_PREFIX}{i}"),
            serde_json::to_string(link).unwrap(),
        ));
    }
    out
}

/// The lessor's EIP-712 signature over the listing, as the marketplace UI produces it.
async fn sign_listing(w: &Wallet, body: &RentalListingCreation) -> String {
    let contract = ContractRentalListing::from_creation(&w.address, body);
    let digest = listing_digest(&contract, body.chain_id as u64).unwrap();
    let sig = w.signer.sign_hash(&B256::from(digest)).await.unwrap();
    format!("0x{}", hex::encode(sig.as_bytes()))
}

fn creation_json(c: &RentalListingCreation) -> Value {
    json!({
        "network": c.network,
        "chainId": c.chain_id,
        "expiration": c.expiration,
        "contractAddress": c.contract_address,
        "tokenId": c.token_id,
        "nonces": c.nonces,
        "periods": c.periods,
        "rentalContractAddress": c.rental_contract_address,
        "signature": c.signature,
        "target": c.target,
    })
}

async fn send(
    app: Router,
    method: &str,
    path: &str,
    headers: Vec<(String, String)>,
    body: Option<Value>,
) -> (StatusCode, HeaderMap, Value) {
    let mut req = Request::builder().method(method).uri(path);
    for (k, v) in &headers {
        req = req.header(k.as_str(), v.as_str());
    }
    let req = match body {
        Some(v) => req
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(v.to_string()))
            .unwrap(),
        None => req.body(Body::empty()).unwrap(),
    };
    let resp = app.oneshot(req).await.unwrap();
    let status = resp.status();
    let headers = resp.headers().clone();
    let bytes = to_bytes(resp.into_body(), 1 << 20).await.unwrap();
    let body = serde_json::from_slice(&bytes)
        .unwrap_or_else(|_| Value::String(String::from_utf8_lossy(&bytes).into_owned()));
    (status, headers, body)
}

fn nft_row(owner: &str, category: &str, estate_size: Option<i32>) -> Value {
    json!({ "data": { "nfts": [ {
        "id": "x", "category": category, "owner": { "id": owner }, "searchText": "-10,20",
        "searchDistanceToPlaza": 2, "searchAdjacentToRoad": false, "searchEstateSize": estate_size,
        "createdAt": "1600000000", "updatedAt": "1700000000"
    } ] } })
}

#[tokio::test]
async fn writes_are_gated_and_the_reads_never_are() {
    let app = router(state_over(lazy_pool(), None, false));
    let (status, headers, body) = send(
        app.clone(),
        "POST",
        "/v1/rentals-listings",
        vec![],
        Some(json!({})),
    )
    .await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(headers.get(header::RETRY_AFTER).unwrap(), "3600");
    assert!(
        body["message"]
            .as_str()
            .unwrap()
            .contains("DATA_WRITES_ENABLED=false"),
        "{body}"
    );
    let (status, _, _) = send(
        app.clone(),
        "PATCH",
        &format!("/v1/rentals-listings/{}", uuid::Uuid::new_v4()),
        vec![],
        None,
    )
    .await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    // The reads reach the handler (and fail on the unreachable pool, not on the gate).
    let (status, _, body) = send(
        app.clone(),
        "GET",
        "/v1/rentals-listings?status=bogus",
        vec![],
        None,
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert_eq!(body["ok"], false);
    let (status, _, body) =
        send(app, "GET", "/v1/rentals-listings?tokenId=4_2", vec![], None).await;
    assert_eq!(
        (status, body["error"].as_str().unwrap()),
        (StatusCode::BAD_REQUEST, "tokenId must be a number")
    );
}

#[tokio::test]
async fn an_enabled_write_without_a_chain_is_401_with_the_old_body() {
    let app = router(state_over(lazy_pool(), None, true));
    let doc = creation_json(&creation(&random_token_id(), "0xsig"));
    let (status, _, body) = send(app, "POST", "/v1/rentals-listings", vec![], Some(doc)).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "{body}");
    assert_eq!(body["ok"], false);
    assert!(
        body["error"]
            .as_str()
            .unwrap()
            .starts_with("Invalid Auth Chain"),
        "{body}"
    );
}

#[tokio::test]
async fn create_refresh_and_list_over_the_stub_subgraph() {
    let Some(pool) = crate::db::scratch::pool(4).await else {
        return;
    };
    let graph = stub_graph().await;
    let w = wallet();
    let token = random_token_id();
    let mut body = creation(&token, "");
    body.signature = sign_listing(&w, &body).await;
    let doc = creation_json(&body);
    let path = "/v1/rentals-listings";

    // No reader configured: the envelope, not a guess.
    let app = router(state_over(pool.clone(), Some(market_with(None)), true));
    let (status, headers, out) = send(
        app,
        "POST",
        path,
        signed_headers(&w, "post", path).await,
        Some(doc.clone()),
    )
    .await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{out}");
    assert_eq!(out["source"], "marketplace-subgraph");
    assert_eq!(out["configure"], "MARKETPLACE_SUBGRAPH_URL");
    assert_eq!(headers.get(header::RETRY_AFTER).unwrap(), "60");

    let state = state_over(
        pool.clone(),
        Some(market_with(Some(graph.url.clone()))),
        true,
    );
    let app = || router(state.clone());

    let (status, _, out) = send(
        app(),
        "POST",
        path,
        signed_headers(&w, "post", path).await,
        Some(doc.clone()),
    )
    .await;
    assert_eq!(
        (status, out["error"].as_str().unwrap()),
        (StatusCode::NOT_FOUND, "NFT not found")
    );

    // Somebody else's parcel: 401 (a fresh cache key: the stub answer changed, so wait out the TTL by using a new state).
    *graph.answer.lock().unwrap() = nft_row(LESSOR, "parcel", None);
    let state2 = state_over(
        pool.clone(),
        Some(market_with(Some(graph.url.clone()))),
        true,
    );
    let (status, _, out) = send(
        router(state2),
        "POST",
        path,
        signed_headers(&w, "post", path).await,
        Some(doc.clone()),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "{out}");
    assert!(
        out["error"].as_str().unwrap().contains("is not the lessor"),
        "{out}"
    );

    *graph.answer.lock().unwrap() = nft_row(&w.address, "estate", Some(0));
    let state3 = state_over(
        pool.clone(),
        Some(market_with(Some(graph.url.clone()))),
        true,
    );
    let (status, _, out) = send(
        router(state3),
        "POST",
        path,
        signed_headers(&w, "post", path).await,
        Some(doc.clone()),
    )
    .await;
    assert_eq!(
        (status, out["error"].as_str().unwrap()),
        (
            StatusCode::BAD_REQUEST,
            "The provided Estate does not have any parcels"
        )
    );

    // The lessor's own parcel: 201, then 409 on the second open listing.
    *graph.answer.lock().unwrap() = nft_row(&w.address, "parcel", None);
    let state4 = state_over(
        pool.clone(),
        Some(market_with(Some(graph.url.clone()))),
        true,
    );
    let app = || router(state4.clone());
    let (status, _, out) = send(
        app(),
        "POST",
        path,
        signed_headers(&w, "post", path).await,
        Some(doc.clone()),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{out}");
    assert_eq!(out["ok"], true);
    let listing = &out["data"];
    assert_eq!(listing["nftId"], format!("{LAND}-{token}"));
    assert_eq!(listing["lessor"], w.address);
    assert_eq!(listing["status"], "open");
    assert_eq!(listing["category"], "parcel");
    assert_eq!(listing["periods"][0]["minDays"], 7);
    let id = listing["id"].as_str().unwrap().to_string();

    let (status, _, out) = send(
        app(),
        "POST",
        path,
        signed_headers(&w, "post", path).await,
        Some(doc.clone()),
    )
    .await;
    assert_eq!(
        (status, out["error"].as_str().unwrap()),
        (
            StatusCode::CONFLICT,
            "There is already a rental listing with this signature"
        )
    );
    let mut relisted = creation(&token, "");
    relisted.nonces = vec!["0".into(), "1".into(), "3".into()];
    relisted.signature = sign_listing(&w, &relisted).await;
    let (status, _, out) = send(
        app(),
        "POST",
        path,
        signed_headers(&w, "post", path).await,
        Some(creation_json(&relisted)),
    )
    .await;
    assert_eq!(
        (status, out["error"].as_str().unwrap()),
        (
            StatusCode::CONFLICT,
            "There is already an open rental listing for the asset"
        )
    );

    // A tampered body fails the EIP-712 check before any read.
    let mut tampered = doc.clone();
    tampered["tokenId"] = Value::String(random_token_id());
    let (status, _, out) = send(
        app(),
        "POST",
        path,
        signed_headers(&w, "post", path).await,
        Some(tampered),
    )
    .await;
    assert_eq!(
        (status, out["error"].as_str().unwrap()),
        (StatusCode::BAD_REQUEST, "The provided signature is invalid")
    );

    // The listing page: by nftId, by lessor, the default target, the price histogram.
    let (status, _, out) = send(
        app(),
        "GET",
        &format!("/v1/rentals-listings?nftIds={LAND}-{token}"),
        vec![],
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{out}");
    assert_eq!(out["data"]["total"], 1);
    assert_eq!(out["data"]["results"][0]["id"], id);
    assert_eq!(
        (
            out["data"]["page"].as_i64(),
            out["data"]["pages"].as_i64(),
            out["data"]["limit"].as_i64()
        ),
        (Some(0), Some(1), Some(100))
    );
    let (_, _, out) = send(
        app(),
        "GET",
        &format!(
            "/v1/rentals-listings?lessor={}&tokenId={token}&status=open&category=parcel",
            w.address.to_uppercase()
        ),
        vec![],
        None,
    )
    .await;
    assert_eq!(out["data"]["total"], 1, "{out}");
    let (_, _, out) = send(
        app(),
        "GET",
        &format!("/v1/rentals-listings?tokenId={token}&status=executed"),
        vec![],
        None,
    )
    .await;
    assert_eq!(out["data"]["total"], 0);
    let (_, _, out) = send(
        app(),
        "GET",
        &format!(
            "/v1/rentals-listings?tokenId={token}&rentalDays=10&minPricePerDay=1000000000000000000"
        ),
        vec![],
        None,
    )
    .await;
    assert_eq!(out["data"]["total"], 1, "{out}");
    let (_, _, out) = send(
        app(),
        "GET",
        &format!("/v1/rentals-listings?tokenId={token}&rentalDays=20"),
        vec![],
        None,
    )
    .await;
    assert_eq!(out["data"]["total"], 0, "no period covers 20 days");
    let (status, _, out) = send(
        app(),
        "GET",
        "/v1/rental-listings/prices?category=parcel&rentalDays=7",
        vec![],
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        out["data"]["1000000000000000000"].as_i64().unwrap_or(0) >= 1,
        "{out}"
    );

    // Refresh: the reader's new row lands on the asset.
    *graph.answer.lock().unwrap() = nft_row(&w.address, "parcel", None);
    graph.answer.lock().unwrap()["data"]["nfts"][0]["searchText"] = json!("Renamed plaza");
    let state5 = state_over(
        pool.clone(),
        Some(market_with(Some(graph.url.clone()))),
        true,
    );
    let (status, _, out) = send(
        router(state5),
        "PATCH",
        &format!("/v1/rentals-listings/{id}"),
        vec![],
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{out}");
    assert_eq!(out["data"]["searchText"], "Renamed plaza");
    let (status, _, out) = send(
        app(),
        "PATCH",
        &format!("/v1/rentals-listings/{}", uuid::Uuid::new_v4()),
        vec![],
        None,
    )
    .await;
    assert_eq!(
        (status, out["error"].as_str().unwrap()),
        (StatusCode::NOT_FOUND, "Rental listing was not found")
    );

    // The nested copy verifies the signature over the request path (the ONE path rule).
    let nested = Router::new().nest("/signatures-api", router(state4.clone()));
    let nested_path = "/signatures-api/v1/rentals-listings";
    let (status, _, out) = send(
        nested,
        "POST",
        nested_path,
        signed_headers(&w, "post", nested_path).await,
        Some(doc.clone()),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::CONFLICT,
        "signed over the public path, refused by the listing rule not the chain: {out}"
    );
    let local = signed_headers(&w, "post", "/v1/rentals-listings").await;
    let (status, _, out) = send(
        Router::new().nest("/signatures-api", router(state4.clone())),
        "POST",
        nested_path,
        local.clone(),
        Some(doc.clone()),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::UNAUTHORIZED,
        "the route-local path alone is not what the client asked for: {out}"
    );
    let mut forwarded = local;
    forwarded.push((
        "x-original-path".to_string(),
        "/v1/rentals-listings".to_string(),
    ));
    let (status, _, out) = send(
        Router::new().nest("/signatures-api", router(state4.clone())),
        "POST",
        nested_path,
        forwarded,
        Some(doc.clone()),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::CONFLICT,
        "signed over the route-local path behind the proxy's x-original-path: {out}"
    );

    cleanup(&pool, &[token]).await;
}

/// `CATALYST_LIVE_MARKETPLACE_SUBGRAPH_URL=https://...` reads Genesis
/// Plaza's parcel (token 0 of the LAND registry) from the real subgraph.
#[tokio::test]
async fn live_marketplace_subgraph_answers_a_land_parcel() {
    let Ok(url) = std::env::var("CATALYST_LIVE_MARKETPLACE_SUBGRAPH_URL") else {
        eprintln!("CATALYST_LIVE_MARKETPLACE_SUBGRAPH_URL unset; skipping");
        return;
    };
    let mut settings = crate::market::Settings::for_tests();
    settings.marketplace_subgraph_url = Some(url);
    let market = Arc::new(MarketplaceState::new(
        lazy_pool(),
        crate::http::Outbound::new(crate::http::HttpClientCfg::default()).unwrap(),
        settings,
    ));
    let cache = crate::shared::cache::TtlMap::bounded(super::nft::NFT_TTL, 4);
    let nft = super::nft::nft_by_contract_token(Some(&market), &cache, LAND, "0")
        .await
        .unwrap()
        .expect("parcel 0,0 exists");
    assert_eq!(nft.category, "parcel");
    assert!(
        crate::market::is_valid_eth_address(&nft.owner_address),
        "{nft:?}"
    );
    assert_eq!(nft.nft_id, format!("{LAND}-0"));
}
