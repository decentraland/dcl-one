//! The rentals listing store and routes over the process's scratch database
//! (`db::scratch`; without `CATALYST_TEST_DATABASE_URL` the DB-backed tests
//! skip), with random token ids so every row is this test's and deleted at
//! the end (the asset row cascades to the rental, the listing and the
//! periods). Asset lookup tests seed an isolated land-authz index.

use std::sync::Arc;

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
const ESTATE: &str = crate::land_authz::ESTATE_REGISTRY_MAINNET;
const RENTALS: &str = super::index::RENTALS_MAINNET;
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
        chain_id: 1,
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
        rental_contract_address: RENTALS.into(),
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
    assert_eq!(listing.chain_id, 1);
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
        Some((1, LAND.to_string(), token.clone()))
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

fn market() -> Arc<MarketplaceState> {
    Arc::new(MarketplaceState::new(
        lazy_pool(),
        crate::http::Outbound::for_tests(),
        crate::market::Settings::for_tests(),
    ))
}

async fn publish_land(pool: &sqlx::PgPool) {
    sqlx::query("INSERT INTO land_authz.index_cursor (network, land_registry, estate_registry, last_block, folded_block, folded_at, full_replay_started)
                 VALUES ('ethereum-mainnet', $1, $2, 100, 100, timezone('UTC', now()), true)")
        .bind(LAND).bind(ESTATE).execute(pool).await.unwrap();
    sqlx::query("INSERT INTO shop.chain_cursor (indexer, network, start_block, last_block, folded_block, folded_at, full_history, chain_id)
        VALUES ('nft', 'ethereum', 0, 100, 100, timezone('UTC', now()), true, 1)").execute(pool).await.unwrap();
}

async fn transfer(
    pool: &sqlx::PgPool,
    contract: &str,
    token: &str,
    owner: &str,
    block: i64,
    time: i64,
) {
    sqlx::query("INSERT INTO land_authz.authz_event (block_number, log_index, block_time, token_address, token_id, account, kind)
                 VALUES ($1, 0, $2, $3, $4::numeric, $5, 'transfer')")
        .bind(block).bind(time).bind(contract).bind(token).bind(owner).execute(pool).await.unwrap();
    let coords = (contract == LAND).then_some((-10, -140));
    sqlx::query("INSERT INTO land_authz.token_right (token_address, token_id, owner, x, y, updated_block, updated_log)
                 VALUES ($1, $2::numeric, $3, $4, $5, $6, 0)
                 ON CONFLICT (token_address, token_id) DO UPDATE SET owner = EXCLUDED.owner, updated_block = EXCLUDED.updated_block")
        .bind(contract).bind(token).bind(owner).bind(coords.map(|c| c.0)).bind(coords.map(|c| c.1)).bind(block)
        .execute(pool).await.unwrap();
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
async fn a_signed_rental_listing_is_refused_while_the_marketplace_is_off() {
    let w = wallet();
    let mut body = creation(&random_token_id(), "");
    body.signature = sign_listing(&w, &body).await;
    let doc = creation_json(&body);
    let path = "/v1/rentals-listings";
    let market = market();
    market.set_marketplace_features(false);
    for state in [
        state_over(lazy_pool(), Some(market), true),
        state_over(lazy_pool(), None, true),
    ] {
        // Off, or no marketplace state to read the rollout from: refused after the signature, before any read.
        let (status, _, out) = send(
            router(state),
            "POST",
            path,
            signed_headers(&w, "post", path).await,
            Some(doc.clone()),
        )
        .await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{out}");
        assert!(
            out.to_string().contains("dapps-shop-marketplace-features"),
            "{out}"
        );
    }
}

#[tokio::test]
async fn create_refresh_and_list_from_the_published_land_index() {
    let Some(pool) = crate::db::scratch::isolated_pool(4).await else {
        return;
    };
    let w = wallet();
    let token = random_token_id();
    let mut body = creation(&token, "");
    body.signature = sign_listing(&w, &body).await;
    let doc = creation_json(&body);
    let path = "/v1/rentals-listings";

    // An unpublished index returns the source envelope.
    let app = router(state_over(pool.clone(), Some(market()), true));
    let (status, headers, out) = send(
        app,
        "POST",
        path,
        signed_headers(&w, "post", path).await,
        Some(doc.clone()),
    )
    .await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{out}");
    assert_eq!(out["source"], "land-authz-index");
    assert_eq!(out["configure"], "LAND_AUTHZ_INDEX_ENABLED");
    assert_eq!(headers.get(header::RETRY_AFTER).unwrap(), "60");

    publish_land(&pool).await;
    let state = state_over(pool.clone(), Some(market()), true);
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

    transfer(&pool, LAND, &token, LESSOR, 1, 1_600_000_000).await;
    let (status, _, out) = send(
        app(),
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

    transfer(&pool, ESTATE, &token, &w.address, 2, 1_600_000_100).await;
    let mut estate = body.clone();
    estate.contract_address = ESTATE.into();
    estate.signature = sign_listing(&w, &estate).await;
    let (status, _, out) = send(
        app(),
        "POST",
        path,
        signed_headers(&w, "post", path).await,
        Some(creation_json(&estate)),
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
    transfer(&pool, LAND, &token, &w.address, 3, 1_700_000_000).await;
    // Legacy LAND cursors can be published before the transfer times are repaired.
    sqlx::query("UPDATE land_authz.authz_event SET block_time=0 WHERE block_number=1")
        .execute(&pool)
        .await
        .unwrap();
    let (code, _, out) = send(
        app(),
        "POST",
        path,
        signed_headers(&w, "post", path).await,
        Some(doc.clone()),
    )
    .await;
    assert_eq!(code, StatusCode::SERVICE_UNAVAILABLE, "{out}");
    assert_eq!(out["source"], "land-authz-index");
    assert_eq!(counts(&pool, &token).await, (0, 0, 0, 0));
    sqlx::query("UPDATE land_authz.authz_event SET block_time=1600000000 WHERE block_number=1")
        .execute(&pool)
        .await
        .unwrap();

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
    assert_eq!(listing["searchText"], "-10,-140");
    let stored: (Option<i16>, Option<bool>, Option<i16>, i64, i64) = sqlx::query_as(
        "SELECT distance_to_plaza, adjacent_to_road, estate_size, extract(epoch FROM nft_created_at)::bigint,
         extract(epoch FROM nft_updated_at)::bigint FROM signatures.rental_assets WHERE contract_address=$1 AND token_id=$2::numeric"
    ).bind(LAND).bind(&token).fetch_one(&pool).await.unwrap();
    assert_eq!(
        stored,
        (Some(9), Some(true), None, 1_600_000_000, 1_700_000_000)
    );
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
    sqlx::query(
        "INSERT INTO shop.land_data (token_address,token_id,name,updated_block,updated_log)
                 VALUES ($1,$2::numeric,'Renamed plaza',4,1)",
    )
    .bind(LAND)
    .bind(&token)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO shop.nft_event (network,block_number,log_index,block_time,kind,emitter,contract_address,token_id)
                 VALUES ('ethereum',4,1,1700000050,'land_data',$1,$1,$2::numeric)").bind(LAND).bind(&token).execute(&pool).await.unwrap();
    // The owner is published through block 3. A block-4 name must not date that owner.
    sqlx::query("UPDATE land_authz.index_cursor SET folded_block=3, last_block=3")
        .execute(&pool)
        .await
        .unwrap();
    let (code, headers, envelope) = send(
        app(),
        "PATCH",
        &format!("/v1/rentals-listings/{id}"),
        vec![],
        None,
    )
    .await;
    assert_eq!(code, StatusCode::SERVICE_UNAVAILABLE, "{envelope}");
    assert_eq!(envelope["source"], "land-authz-index");
    assert_eq!(headers.get(header::RETRY_AFTER).unwrap(), "60");
    sqlx::query("UPDATE land_authz.index_cursor SET folded_block=100, last_block=100")
        .execute(&pool)
        .await
        .unwrap();
    let (status, _, out) = send(
        app(),
        "PATCH",
        &format!("/v1/rentals-listings/{id}"),
        vec![],
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{out}");
    assert_eq!(out["data"]["searchText"], "-10,-140,renamed plaza");
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
    let nested = Router::new().nest("/signatures-api", app());
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
        Router::new().nest("/signatures-api", app()),
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
        Router::new().nest("/signatures-api", app()),
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

    // A metadata rebuild must leave the stored name intact, even at equal asset times.
    sqlx::raw_sql(
        "UPDATE shop.chain_cursor SET folded_block=NULL WHERE indexer='nft';
                   DELETE FROM shop.land_data",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query("UPDATE signatures.rental_assets SET nft_updated_at=to_timestamp(1700000000) AT TIME ZONE 'UTC'
        WHERE contract_address=$1 AND token_id=$2::numeric").bind(LAND).bind(&token).execute(&pool).await.unwrap();
    let patch = format!("/v1/rentals-listings/{id}");
    let (code, _, out) = send(app(), "PATCH", &patch, vec![], None).await;
    assert_eq!(code, StatusCode::SERVICE_UNAVAILABLE, "{out}");
    assert_eq!(out["source"], "nft-index");
    assert_eq!(
        state
            .db
            .get_listing_by_id(&id)
            .await
            .unwrap()
            .unwrap()
            .search_text,
        "-10,-140,renamed plaza"
    );
    sqlx::query("UPDATE shop.chain_cursor SET folded_block=last_block WHERE indexer='nft'")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("UPDATE signatures.rentals SET chain_id=11155111 WHERE id=$1::text::uuid")
        .bind(&id)
        .execute(&pool)
        .await
        .unwrap();
    let (code, _, out) = send(app(), "PATCH", &patch, vec![], None).await;
    assert_eq!(code, StatusCode::SERVICE_UNAVAILABLE, "{out}");
    assert_eq!(out["source"], "land-authz-index");
    sqlx::query("UPDATE signatures.rentals SET chain_id=1 WHERE id=$1::text::uuid")
        .bind(&id)
        .execute(&pool)
        .await
        .unwrap();

    // Refresh after a transfer: a newer reader row with another owner
    // cancels the listing, and the route answers the synced status.
    transfer(&pool, LAND, &token, LESSOR, 5, 1_700_000_100).await;
    let (status, _, out) = send(
        app(),
        "PATCH",
        &format!("/v1/rentals-listings/{id}"),
        vec![],
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{out}");
    assert_eq!(out["data"]["status"], "cancelled", "{out}");

    cleanup(&pool, &[token]).await;
}

/// A reader that lags answers from older blocks: neither read replaces the
/// newer stored asset row, so the stored time stays and the second, less
/// stale read (the previous owner's) is still too old to close the listing.
#[tokio::test]
async fn a_stale_refresh_keeps_the_stored_asset_row_and_the_listing() {
    let Some(pool) = crate::db::scratch::isolated_pool(4).await else {
        return;
    };
    let db = Database::new(pool.clone());
    publish_land(&pool).await;
    let state = state_over(pool.clone(), Some(market()), true);
    let token = random_token_id();
    let listing = insert(&db, &token, LESSOR, &format!("0xsig-{token}"))
        .await
        .unwrap();
    // The lessor received the asset at T5.
    let (t3, t4, t5) = (1_700_000_300i64, 1_700_000_400i64, 1_700_000_500i64);
    sqlx::query(
        "UPDATE signatures.rental_assets SET nft_updated_at = to_timestamp($3) AT TIME ZONE 'UTC' \
         WHERE contract_address = $1 AND token_id = $2::numeric",
    )
    .bind(LAND)
    .bind(&token)
    .bind(t5 as f64)
    .execute(&pool)
    .await
    .unwrap();

    let previous_owner = format!("0x{}", "c3".repeat(20));
    for (block, read_at, label) in [(1, t3, "a stale read"), (2, t4, "a later, less stale read")] {
        transfer(&pool, LAND, &token, &previous_owner, block, read_at).await;
        let (status, _, out) = send(
            router(state.clone()),
            "PATCH",
            &format!("/v1/rentals-listings/{}", listing.id),
            vec![],
            None,
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{label}: {out}");
        assert_eq!(out["data"]["status"], "open", "{label}: {out}");
        assert_eq!(out["data"]["searchText"], "-10,20", "{label}: {out}");
        let (stored_at, search_text): (i64, String) = sqlx::query_as(
            "SELECT extract(epoch FROM nft_updated_at)::bigint, search_text FROM signatures.rental_assets \
             WHERE contract_address = $1 AND token_id = $2::numeric",
        )
        .bind(LAND)
        .bind(&token)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!((stored_at, search_text.as_str()), (t5, "-10,20"), "{label}");
    }
    // The name is newer than the stored asset, but the previous owner's transfer is older.
    sqlx::query(
        "INSERT INTO shop.land_data (token_address,token_id,name,updated_block,updated_log)
        VALUES ($1,$2::numeric,'New document',3,0)",
    )
    .bind(LAND)
    .bind(&token)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO shop.nft_event (network,block_number,log_index,block_time,kind,emitter,contract_address,token_id)
        VALUES ('ethereum',3,0,$3,'land_data',$1,$1,$2::numeric)").bind(LAND).bind(&token).bind(t5+100).execute(&pool).await.unwrap();
    let patch = format!("/v1/rentals-listings/{}", listing.id);
    let (code, _, out) = send(router(state.clone()), "PATCH", &patch, vec![], None).await;
    assert_eq!(code, StatusCode::OK, "{out}");
    assert_eq!(out["data"]["status"], "open");
    transfer(&pool, LAND, &token, &previous_owner, 4, t5 + 200).await;
    let (code, _, out) = send(router(state), "PATCH", &patch, vec![], None).await;
    assert_eq!(code, StatusCode::OK, "{out}");
    assert_eq!(out["data"]["status"], "cancelled");
    cleanup(&pool, &[token]).await;
}

/// A relisting whose reader row is older than the stored asset row keeps
/// the stored time, which the refresh route's guard compares against, and
/// the stored details; the listing is created all the same. A reader row
/// that is not older replaces them.
#[tokio::test]
async fn a_relisting_never_lowers_the_stored_asset_time() {
    let Some(pool) = crate::db::scratch::pool(4).await else {
        return;
    };
    let db = Database::new(pool.clone());
    let token = random_token_id();
    let first = insert(&db, &token, LESSOR, &format!("0xsig-a-{token}"))
        .await
        .unwrap();
    type Stored = (
        i64,
        String,
        String,
        Option<i16>,
        Option<bool>,
        Option<i16>,
        i64,
    );
    let stored = || async {
        sqlx::query_as::<_, Stored>(
            "SELECT extract(epoch FROM nft_updated_at)::bigint, category, search_text, distance_to_plaza, \
                    adjacent_to_road, estate_size, extract(epoch FROM fetched_at)::bigint \
             FROM signatures.rental_assets WHERE contract_address = $1 AND token_id = $2::numeric",
        )
        .bind(LAND)
        .bind(&token)
        .fetch_one(&pool)
        .await
        .unwrap()
    };
    let cancel = |id: String| async {
        sqlx::query("UPDATE signatures.rentals SET status = 'cancelled' WHERE id = $1::uuid")
            .bind(id)
            .execute(&pool)
            .await
            .unwrap();
    };
    let ahead = chrono::Utc::now().timestamp() + 3_600;
    let newer: Stored = (
        ahead,
        "estate".into(),
        "A newer name".into(),
        Some(7),
        Some(false),
        Some(4),
        ahead + 60,
    );
    sqlx::query(
        "UPDATE signatures.rental_assets SET nft_updated_at = to_timestamp($3) AT TIME ZONE 'UTC', \
                category = $4, search_text = $5, distance_to_plaza = $6, adjacent_to_road = $7, estate_size = $8, \
                fetched_at = to_timestamp($9) AT TIME ZONE 'UTC' \
         WHERE contract_address = $1 AND token_id = $2::numeric",
    )
    .bind(LAND)
    .bind(&token)
    .bind(newer.0 as f64)
    .bind(&newer.1)
    .bind(&newer.2)
    .bind(newer.3)
    .bind(newer.4)
    .bind(newer.5)
    .bind(newer.6 as f64)
    .execute(&pool)
    .await
    .unwrap();
    cancel(first.id.clone()).await;
    let second = insert(&db, &token, LESSOR, &format!("0xsig-b-{token}"))
        .await
        .unwrap();
    assert_eq!(stored().await, newer);
    assert_eq!(
        (
            second.status.as_str(),
            second.category.as_str(),
            second.search_text.as_str()
        ),
        ("open", "estate", "A newer name")
    );
    assert_eq!(counts(&pool, &token).await.1, 2);

    sqlx::query(
        "UPDATE signatures.rental_assets SET nft_updated_at = to_timestamp(1600000000) AT TIME ZONE 'UTC' \
         WHERE contract_address = $1 AND token_id = $2::numeric",
    )
    .bind(LAND)
    .bind(&token)
    .execute(&pool)
    .await
    .unwrap();
    cancel(second.id.clone()).await;
    let before = chrono::Utc::now().timestamp();
    let third = insert(&db, &token, LESSOR, &format!("0xsig-c-{token}"))
        .await
        .unwrap();
    let (updated_at, category, search_text, distance, road, size, fetched_at) = stored().await;
    assert!(updated_at >= before && fetched_at >= before && fetched_at < ahead);
    assert_eq!(
        (
            category.as_str(),
            search_text.as_str(),
            distance,
            road,
            size
        ),
        ("parcel", "-10,20", Some(3), Some(true), None)
    );
    assert_eq!(
        (third.category.as_str(), third.search_text.as_str()),
        ("parcel", "-10,20")
    );
    cleanup(&pool, &[token]).await;
}

/// A relisting whose reader row has the stored time exactly replaces the
/// stored details: only a strictly older row keeps them.
#[tokio::test]
async fn a_relisting_at_the_stored_asset_time_replaces_the_details() {
    let Some(pool) = crate::db::scratch::pool(4).await else {
        return;
    };
    let db = Database::new(pool.clone());
    let token = random_token_id();
    let first = insert(&db, &token, LESSOR, &format!("0xsig-a-{token}"))
        .await
        .unwrap();
    let at = chrono::DateTime::from_timestamp(chrono::Utc::now().timestamp() + 3_600, 0)
        .unwrap()
        .naive_utc();
    sqlx::query(
        "UPDATE signatures.rental_assets SET nft_updated_at = $3, category = 'estate', search_text = 'A stored name' \
         WHERE contract_address = $1 AND token_id = $2::numeric",
    )
    .bind(LAND)
    .bind(&token)
    .bind(at)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query("UPDATE signatures.rentals SET status = 'cancelled' WHERE id = $1::uuid")
        .bind(&first.id)
        .execute(&pool)
        .await
        .unwrap();
    let now = chrono::Utc::now().naive_utc();
    let signature = format!("0xsig-b-{token}");
    let second = db
        .insert_listing(
            &format!("{LAND}-{token}"),
            &ListingMetadata {
                category: "parcel",
                search_text: "-10,20",
                distance_to_plaza: Some(3),
                adjacent_to_road: Some(true),
                estate_size: None,
                updated_at: at,
            },
            now,
            &creation(&token, &signature),
            LESSOR,
            now,
        )
        .await
        .unwrap();
    let stored: (chrono::NaiveDateTime, String, String) = sqlx::query_as(
        "SELECT nft_updated_at, category, search_text FROM signatures.rental_assets \
         WHERE contract_address = $1 AND token_id = $2::numeric",
    )
    .bind(LAND)
    .bind(&token)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(stored, (at, "parcel".to_string(), "-10,20".to_string()));
    assert_eq!(
        (second.category.as_str(), second.search_text.as_str()),
        ("parcel", "-10,20")
    );
    cleanup(&pool, &[token]).await;
}

#[tokio::test]
async fn a_signed_mainnet_listing_cannot_use_sepolia_registry_rights() {
    use crate::land_authz::LAND_SEPOLIA;
    let Some(pool) = crate::db::scratch::isolated_pool(4).await else {
        return;
    };
    publish_land(&pool).await;
    sqlx::query(
        "UPDATE land_authz.index_cursor SET network=$1, land_registry=$2, estate_registry=$3",
    )
    .bind(LAND_SEPOLIA.network)
    .bind(LAND_SEPOLIA.land_registry)
    .bind(LAND_SEPOLIA.estate_registry)
    .execute(&pool)
    .await
    .unwrap();
    let w = wallet();
    let mut body = creation("17", "");
    body.contract_address = LAND_SEPOLIA.land_registry.into();
    body.signature = sign_listing(&w, &body).await;
    transfer(&pool, &body.contract_address, "17", &w.address, 1, 100).await;
    let mut settings = crate::market::Settings::for_tests();
    settings.eth_network = "sepolia".into();
    let market = Arc::new(MarketplaceState::new(
        pool.clone(),
        crate::http::Outbound::for_tests(),
        settings,
    ));
    let app = router(state_over(pool.clone(), Some(market), true));
    let path = "/v1/rentals-listings";
    let (code, _, out) = send(
        app,
        "POST",
        path,
        signed_headers(&w, "post", path).await,
        Some(creation_json(&body)),
    )
    .await;
    assert_eq!(code, StatusCode::SERVICE_UNAVAILABLE, "{out}");
    assert_eq!(out["source"], "land-authz-index");
    // Matching Sepolia rights cannot borrow the published mainnet NFT metadata.
    assert!(matches!(
        super::nft::nft_by_contract_token(
            &pool,
            Some(LAND_SEPOLIA),
            11155111,
            &body.contract_address,
            "17"
        )
        .await,
        Err(super::nft::NftReadError::MetadataUnavailable)
    ));
}

#[tokio::test]
async fn an_estate_reads_its_members_size_and_proximity() {
    let Some(pool) = crate::db::scratch::isolated_pool(4).await else {
        return;
    };
    publish_land(&pool).await;
    transfer(&pool, ESTATE, "1", LESSOR, 1, 100).await;
    transfer(&pool, LAND, "2", ESTATE, 2, 110).await;
    transfer(&pool, LAND, "3", ESTATE, 3, 120).await;
    sqlx::raw_sql("UPDATE land_authz.token_right SET y=19 WHERE token_id=3;
        INSERT INTO land_authz.estate_land VALUES (2,1,4,0),(3,1,5,0);
        INSERT INTO shop.land_data (token_address,token_id,name,updated_block,updated_log)
        SELECT token_address,token_id,'Estate Name',1,1 FROM land_authz.token_right WHERE token_id=1;")
        .execute(&pool).await.unwrap();
    let state = state_over(pool, Some(market()), true);
    let nft = state.nft(1, ESTATE, "1").await.unwrap().unwrap();
    assert_eq!(nft.owner_address, LESSOR);
    assert_eq!(nft.search_text, "estate name");
    assert_eq!(
        (nft.estate_size, nft.distance_to_plaza, nft.adjacent_to_road),
        (Some(2), Some(9), Some(true))
    );
}

#[tokio::test]
async fn a_listing_is_refused_off_the_served_chain_or_rentals_contract() {
    let w = wallet();
    let path = "/v1/rentals-listings";
    let app = router(state_over(lazy_pool(), Some(market()), true));
    let chain_refusal =
        "The chain id and network of the listing are not the ones supported by the server";
    let contract_refusal =
        "The rental contract address does not match the Rentals contract of the given chain";
    let goerli = |b: &mut RentalListingCreation| {
        b.chain_id = 5;
        b.rental_contract_address = "0x92159c78f0f4523b9c60382bb888f30f10a46b3b".into();
    };
    let matic = |b: &mut RentalListingCreation| b.network = "MATIC".into();
    let other_contract = |b: &mut RentalListingCreation| b.rental_contract_address = LESSOR.into();
    type Change<'a> = &'a dyn Fn(&mut RentalListingCreation);
    let cases: [(Change, &str); 3] = [
        (&goerli, chain_refusal),
        (&matic, chain_refusal),
        (&other_contract, contract_refusal),
    ];
    for (change, expected) in cases {
        let mut body = creation(&random_token_id(), "");
        change(&mut body);
        body.signature = sign_listing(&w, &body).await;
        let (status, _, out) = send(
            app.clone(),
            "POST",
            path,
            signed_headers(&w, "post", path).await,
            Some(creation_json(&body)),
        )
        .await;
        assert_eq!(
            (status, out["error"].as_str().unwrap_or_default()),
            (StatusCode::BAD_REQUEST, expected),
            "{out}"
        );
    }
}

/// The asset's rents at the served Rentals contract: a running one refuses
/// the listing; once it ended, the contract still holding the asset lets its
/// last lessor list it, read from a published rentals index only.
#[tokio::test]
async fn a_running_rent_conflicts_and_an_ended_one_lets_its_lessor_relist() {
    let Some(pool) = crate::db::scratch::isolated_pool(4).await else {
        return;
    };
    publish_land(&pool).await;
    let w = wallet();
    let token = random_token_id();
    transfer(&pool, LAND, &token, RENTALS, 1, 1_700_000_000).await;
    let mut body = creation(&token, "");
    body.signature = sign_listing(&w, &body).await;
    let doc = creation_json(&body);
    let path = "/v1/rentals-listings";
    let state = state_over(pool.clone(), Some(market()), true);
    let post = || async {
        send(
            router(state.clone()),
            "POST",
            path,
            signed_headers(&w, "post", path).await,
            Some(doc.clone()),
        )
        .await
    };
    let now = chrono::Utc::now().timestamp();
    let rent = |block: i64, lessor: String, started: i64| {
        let pool = pool.clone();
        let token = token.clone();
        async move {
            sqlx::query(
                "INSERT INTO shop.rental_event (network, block_number, log_index, block_time, rentals_address, kind, contract_address, token_id, signer, tenant, rental_days, price_per_day, signature, tx_hash) \
                 VALUES ('ethereum', $1, 0, $2, $3, 'rented', $4, $5::numeric, $6, $7, 30, 1, $8, '0x')",
            )
            .bind(block)
            .bind(started)
            .bind(RENTALS)
            .bind(LAND)
            .bind(&token)
            .bind(lessor)
            .bind(LESSOR)
            .bind(format!("0x{block:x}"))
            .execute(&pool)
            .await
            .unwrap();
        }
    };

    rent(1, w.address.clone(), now - 86_400).await;
    let (status, _, out) = post().await;
    assert_eq!(
        (status, out["error"].as_str().unwrap_or_default()),
        (
            StatusCode::CONFLICT,
            "An open rental already exists for this token"
        )
    );

    rent(2, w.address.clone(), now - 40 * 86_400).await;
    let (status, _, out) = post().await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{out}");
    assert_eq!(out["source"], "rentals-index");

    sqlx::query(
        "INSERT INTO shop.chain_cursor (indexer, network, start_block, last_block, folded_block, folded_at, full_history, chain_id) \
         VALUES ('rentals', 'ethereum', 0, 10, 10, timezone('UTC', now()), true, 1)",
    )
    .execute(&pool)
    .await
    .unwrap();
    rent(3, LESSOR.to_string(), now - 40 * 86_400).await;
    let (status, _, out) = post().await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "{out}");

    rent(4, w.address.clone(), now - 40 * 86_400).await;
    let (status, _, out) = post().await;
    assert_eq!(status, StatusCode::CREATED, "{out}");
    assert_eq!(out["data"]["rentalContractAddress"], RENTALS);
}

/// The index walks only the mainnet contract: off it no lessor is ever named,
/// so an asset that contract holds is refused at once rather than waiting on
/// a readiness that never comes; at mainnet's it waits for the index.
#[tokio::test]
async fn a_held_asset_off_the_indexed_contract_is_not_relisted() {
    let sepolia = super::signature::rentals_contract(11_155_111).unwrap();
    let now = chrono::Utc::now();
    let last_rent = (LESSOR.to_string(), now.timestamp() - 86_400);
    assert_eq!(
        super::handlers::held_for_lessor(&lazy_pool(), now, sepolia, Some(&last_rent), LESSOR)
            .await,
        Some(false)
    );

    let Some(pool) = crate::db::scratch::isolated_pool(2).await else {
        return;
    };
    assert_eq!(
        super::handlers::held_for_lessor(&pool, now, RENTALS, Some(&last_rent), LESSOR).await,
        None
    );
}

#[tokio::test]
async fn rental_totals_survive_exact_past_end_and_zero_size_pages() {
    use super::db::{ListingFilters, ListingQuery};
    let Some(own) = crate::test_db::OwnDatabase::create("rental_page_totals").await else {
        return;
    };
    let db = Database::new(own.pool.clone());
    let mut ids = Vec::new();
    for token in ["1", "2", "3"] {
        let listing = insert(&db, token, LESSOR, &format!("signature-{token}"))
            .await
            .unwrap();
        ids.push(listing.id);
    }
    // Each sort has its own order. Two listings share a maximum price across
    // the page boundary, so the listing ID decides which one appears first.
    sqlx::raw_sql("UPDATE signatures.rental_assets SET
            search_text = CASE token_id WHEN 1 THEN 'B' WHEN 2 THEN 'A' ELSE 'C' END,
            nft_created_at = timestamp '2020-01-01' - token_id::int * interval '1 day';
        UPDATE signatures.rentals SET created_at = timestamp '2020-01-01' + token_id::int * interval '1 day';
        UPDATE signatures.rental_periods p SET price_per_day = CASE
            WHEN p.min_days = 30 AND r.token_id = 2 THEN 30
            WHEN p.min_days = 30 THEN 20
            WHEN r.token_id = 1 THEN 3 WHEN r.token_id = 2 THEN 1 ELSE 2 END
        FROM signatures.rentals r WHERE r.id = p.rental_id;")
        .execute(&own.pool).await.unwrap();
    for history in [false, true] {
        for (sort_by, ranks) in [
            ("land_creation_date", [3, 2, 1]),
            ("name", [2, 1, 3]),
            ("max_rental_price", [20, 30, 20]),
            ("min_rental_price", [3, 1, 2]),
            ("created_at", [1, 2, 3]),
        ] {
            for direction in ["asc", "desc"] {
                let mut expected = [0, 1, 2];
                expected.sort_by(|&a, &b| {
                    let order = if direction == "asc" {
                        ranks[a].cmp(&ranks[b])
                    } else {
                        ranks[b].cmp(&ranks[a])
                    };
                    order.then_with(|| ids[a].cmp(&ids[b]))
                });
                for (offset, limit) in [(0, 2), (2, 2), (3, 2), (4, 2), (0, 0)] {
                    let page = db
                        .get_listings(&ListingQuery {
                            sort_by: Some(sort_by.to_string()),
                            sort_direction: Some(direction.to_string()),
                            offset,
                            limit,
                            history,
                            filter: ListingFilters {
                                rental_days: vec![10],
                                ..Default::default()
                            },
                        })
                        .await
                        .unwrap();
                    assert_eq!(page.total, 3);
                    let actual: Vec<_> = page.results.iter().map(|r| &r.id).collect();
                    let wanted: Vec<_> = expected
                        .iter()
                        .skip(offset as usize)
                        .take(limit as usize)
                        .map(|&i| &ids[i])
                        .collect();
                    assert_eq!(
                        actual, wanted,
                        "{sort_by} {direction}, history {history}, offset {offset}, limit {limit}"
                    );
                }
            }
        }
    }
    let matched = db
        .get_listings(&ListingQuery {
            sort_by: None,
            sort_direction: None,
            offset: 0,
            limit: 10,
            history: false,
            filter: ListingFilters {
                token_id: Some("2".to_string()),
                ..Default::default()
            },
        })
        .await
        .unwrap();
    assert_eq!(matched.total, 1);
    assert_eq!(matched.results.len(), 1);
    assert_eq!(matched.results[0].id, ids[1]);
    let empty = db
        .get_listings(&ListingQuery {
            sort_by: None,
            sort_direction: None,
            offset: 0,
            limit: 10,
            history: false,
            filter: ListingFilters {
                token_id: Some("4".to_string()),
                ..Default::default()
            },
        })
        .await
        .unwrap();
    assert_eq!(empty.total, 0);
    assert!(empty.results.is_empty());
    own.drop().await;
}
