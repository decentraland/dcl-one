//! The G1 lambdas over a scratch database of their own (the index cursors
//! they seed are one row per network): the catalogue, collections and
//! outfits, the explorer backpack with its three legs, the third-party
//! wearables over a fake registry subgraph and a fake JSON-RPC chain, the
//! LAND and name routes over the land and nft indexes, the contract lists,
//! each before its source is ready (the unavailable envelope) and after.

use std::sync::Arc;

use alloy_primitives::{Address, Bytes, U256};
use alloy_sol_types::{SolCall, SolValue};
use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use serde_json::{Value, json};
use sqlx::PgPool;
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use tower::ServiceExt;

use super::listings::RpcEndpoints;
use super::third_party::{IERC721, IMulticall3};
use super::{MarketSettings, MarketplaceState, Settings, SharedState, router};
use crate::land_authz::{ESTATE_REGISTRY_MAINNET, LAND_REGISTRY_MAINNET};

const OWNER: &str = "0xaaaa000000000000000000000000000000000001";
const COFFEE: &str = "0xc0ffee0000000000000000000000000000000000";
const TP_CONTRACT: &str = "0x7700000000000000000000000000000000000077";
const TP_URN: &str = "urn:decentraland:matic:collections-thirdparty:tp:c1:hat";

struct ScratchDb {
    name: String,
    pool: PgPool,
    admin: PgPool,
}

impl ScratchDb {
    async fn create() -> Option<Self> {
        let Ok(base) = std::env::var("CATALYST_TEST_DATABASE_URL") else {
            eprintln!("skipping: CATALYST_TEST_DATABASE_URL not set");
            return None;
        };
        let opts: PgConnectOptions = base
            .parse()
            .expect("CATALYST_TEST_DATABASE_URL must be a postgres DSN");
        let admin = PgPoolOptions::new()
            .max_connections(1)
            .connect_with(opts.clone().database("postgres"))
            .await
            .unwrap();
        let name = format!(
            "catalyst_z_lmb_{}",
            &uuid::Uuid::new_v4().simple().to_string()[..12]
        );
        sqlx::query(sqlx::AssertSqlSafe(format!("CREATE DATABASE \"{name}\"")))
            .execute(&admin)
            .await
            .unwrap();
        let mut url = url::Url::parse(&base).unwrap();
        url.set_path(&format!("/{name}"));
        let pool = crate::db::connect_pool(
            url.as_str(),
            &crate::db::PoolSettings {
                max_connections: 4,
                ..Default::default()
            },
        )
        .await
        .unwrap();
        crate::db::apply_schema(&pool).await.unwrap();
        Some(Self { name, pool, admin })
    }

    async fn drop(self) {
        self.pool.close().await;
        let _ = sqlx::query(sqlx::AssertSqlSafe(format!(
            "DROP DATABASE \"{}\" WITH (FORCE)",
            self.name
        )))
        .execute(&self.admin)
        .await;
    }
}

async fn serve(app: Router) -> String {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    format!("http://{addr}/")
}

/// One GraphQL fake per role, answering by the document it receives.
async fn serve_graph(answer: fn(&str) -> Value) -> String {
    use axum::routing::post;
    serve(Router::new().route(
        "/",
        post(move |axum::Json(body): axum::Json<Value>| async move {
            let query = body["query"].as_str().unwrap_or("").to_string();
            axum::Json(json!({ "data": answer(&query) }))
        }),
    ))
    .await
}

fn matic_collections(query: &str) -> Value {
    if query.contains("category: \"wearable\"") {
        json!({ "nfts": [
            { "id": "a1", "urn": format!("urn:decentraland:matic:collections-v2:{COFFEE}:1"), "tokenId": "10", "transferredAt": "200",
              "item": { "rarity": "epic", "price": "5" }, "metadata": { "wearable": { "name": "Hat", "category": "hat" } } },
            { "id": "a2", "urn": format!("urn:decentraland:matic:collections-v2:{COFFEE}:1"), "tokenId": "11", "transferredAt": "300",
              "item": { "rarity": "epic", "price": "5" }, "metadata": { "wearable": { "name": "Hat", "category": "hat" } } }
        ] })
    } else {
        json!({ "nfts": [] })
    }
}

fn eth_collections(_: &str) -> Value {
    json!({ "nfts": [] })
}

fn marketplace_graph(query: &str) -> Value {
    if query.contains("category: ens") {
        json!({ "nfts": [{ "id": "n1", "name": "alice", "contractAddress": "0x2a187453064356c898cae034eaed119e1663acb8", "tokenId": "77", "activeOrder": null }] })
    } else {
        json!({ "nfts": [] })
    }
}

/// A GraphQL fake that also sees the variables.
async fn serve_graph_vars(answer: fn(&str, &Value) -> Value) -> String {
    use axum::routing::post;
    serve(Router::new().route(
        "/",
        post(move |axum::Json(body): axum::Json<Value>| async move {
            let query = body["query"].as_str().unwrap_or("").to_string();
            let vars = body.get("variables").cloned().unwrap_or(Value::Null);
            axum::Json(json!({ "data": answer(&query, &vars) }))
        }),
    ))
    .await
}

/// The matic collections subgraph's `items` documents (the catalogue's
/// on-chain leg before the mirror), the owned copies otherwise.
fn matic_items(query: &str) -> Value {
    if query.contains("itemType_in: [emote_v1]") {
        json!({ "items": [{ "urn": format!("urn:decentraland:matic:collections-v2:{COFFEE}:2"), "createdAt": "2", "metadata": { "emote": { "name": "Dance" } } }] })
    } else if query.contains("itemType_in: [wearable_v1") {
        json!({ "items": [{ "urn": format!("urn:decentraland:matic:collections-v2:{COFFEE}:1"), "createdAt": "1", "metadata": { "wearable": { "name": "Hat" } } }] })
    } else {
        matic_collections(query)
    }
}

/// The marketplace subgraph before the nft and land indexes: the name
/// holder and the owner's lands.
fn marketplace_reader(query: &str, vars: &Value) -> Value {
    if query.contains("category: ens, searchText") {
        if vars["name"] == "alice" {
            json!({ "nfts": [{ "id": "ens-1", "owner": { "id": OWNER } }] })
        } else {
            json!({ "nfts": [] })
        }
    } else if query.contains("category_in: [parcel, estate]") {
        if vars["owner"] != OWNER {
            return json!({ "nfts": [] });
        }
        json!({ "nfts": [
            { "id": "parcel-1", "name": "Plot", "contractAddress": LAND_REGISTRY_MAINNET, "tokenId": "1", "category": "parcel", "searchParcelX": "1", "searchParcelY": "2",
              "transferredAt": "5", "parcel": { "data": { "description": "desc" } }, "estate": null, "activeOrder": { "price": "1000" } },
            { "id": "estate-5", "name": null, "contractAddress": ESTATE_REGISTRY_MAINNET, "tokenId": "5", "category": "estate", "searchParcelX": null, "searchParcelY": null,
              "transferredAt": "8", "parcel": null, "estate": { "data": null }, "activeOrder": null }
        ] })
    } else {
        marketplace_graph(query)
    }
}

const MANAGER: &str = "0x462ab09f1a75786e0c54ced394e0fb537af03734";
const UPDATE_OP: &str = "0xbbbb000000000000000000000000000000000002";

/// The land-manager subgraph: parcel 0,0 sits in an estate of OWNER whose
/// update manager is MANAGER (an approved-then-revoked operator drops out);
/// UPDATE_OP is the update operator of two parcels.
fn land_reader(query: &str, vars: &Value) -> Value {
    if query.contains("fetchOperatorsOfParcel") {
        if vars["x"] == 0 && vars["y"] == 0 {
            json!({ "parcels": [{ "x": "0", "y": "0", "owner": { "address": ESTATE_REGISTRY_MAINNET }, "operator": null, "updateOperator": null }],
                    "estates": [{ "owner": { "address": OWNER }, "operator": null, "updateOperator": null }] })
        } else {
            json!({ "parcels": [], "estates": [] })
        }
    } else if query.contains("fetchAuthorizations") {
        if vars["tokenAddress"] != ESTATE_REGISTRY_MAINNET || vars["address"] != OWNER {
            return json!({ "authorizations": [] });
        }
        json!({ "authorizations": [
            { "type": "UpdateManager", "operator": MANAGER, "isApproved": true, "timestamp": "1" },
            { "type": "Operator", "operator": "0xdead", "isApproved": true, "timestamp": "2" },
            { "type": "Operator", "operator": "0xdead", "isApproved": false, "timestamp": "3" }
        ] })
    } else if query.contains("fetchParcelsByUpdateOperator") {
        if vars["updateOperator"] != UPDATE_OP {
            return json!({ "parcels": [] });
        }
        json!({ "parcels": [
            { "id": "1", "x": "1", "y": "2", "owner": { "id": OWNER }, "updateOperator": UPDATE_OP },
            { "id": "2", "x": "3", "y": "4", "owner": { "id": OWNER }, "updateOperator": UPDATE_OP }
        ] })
    } else {
        json!({})
    }
}

fn registry_graph(_: &str) -> Value {
    json!({ "thirdParties": [{
        "id": "urn:decentraland:matic:collections-thirdparty:tp",
        "metadata": { "thirdParty": { "name": "TP", "description": "A provider", "contracts": [{ "network": "matic", "address": TP_CONTRACT }] } }
    }] })
}

fn word_string(s: &str) -> String {
    let mut out = format!("{:064x}", 32);
    out.push_str(&format!("{:064x}", s.len()));
    let mut data = hex::encode(s);
    while !data.len().is_multiple_of(64) {
        data.push('0');
    }
    out.push_str(&data);
    format!("0x{out}")
}

/// A JSON-RPC chain: the owner holds TP_CONTRACT token 2 (a plain ERC721),
/// the list contracts hold one entry each.
fn answer_call(to: &str, data: &str) -> String {
    let data = data.trim_start_matches("0x");
    let owner: Address = OWNER.parse().unwrap();
    match &data[..8] {
        "70a08231" => format!("0x{:064x}", 1),
        "01ffc9a7" => format!("0x{:064x}", 0),
        "949d225d" => format!("0x{:064x}", 1),
        "9507d39a" => word_string(
            if to.eq_ignore_ascii_case("0x0c4c90a4f29872a2e9ef4c4be3d419792bca9a36") {
                "badname"
            } else {
                "10,20"
            },
        ),
        "82ad56cb" => {
            let call =
                IMulticall3::aggregate3Call::abi_decode(&hex::decode(data).unwrap()).unwrap();
            let results: Vec<IMulticall3::Result3> = call
                .calls
                .iter()
                .map(|c| {
                    let token = IERC721::ownerOfCall::abi_decode(&c.callData)
                        .map(|o| o.tokenId)
                        .unwrap_or_default();
                    let holder = if token == U256::from(2) {
                        owner
                    } else {
                        Address::ZERO
                    };
                    IMulticall3::Result3 {
                        success: true,
                        returnData: Bytes::from(holder.abi_encode()),
                    }
                })
                .collect();
            format!(
                "0x{}",
                hex::encode(IMulticall3::aggregate3Call::abi_encode_returns(&results))
            )
        }
        _ => "0x".to_string(),
    }
}

async fn serve_rpc() -> String {
    use axum::routing::post;
    serve(Router::new().route(
        "/",
        post(|axum::Json(body): axum::Json<Value>| async move {
            let one = |req: &Value| -> Value {
                let p = &req["params"][0];
                json!({ "jsonrpc": "2.0", "id": req["id"], "result": answer_call(p["to"].as_str().unwrap_or(""), p["data"].as_str().unwrap_or("0x00000000")) })
            };
            axum::Json(match &body {
                Value::Array(reqs) => Value::Array(reqs.iter().map(one).collect()),
                req => one(req),
            })
        }),
    ))
    .await
}

async fn get_raw(app: Router, uri: &str) -> (StatusCode, Vec<u8>) {
    let resp = app
        .oneshot(Request::get(uri).body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = resp.status();
    (
        status,
        axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
}

async fn get_via(app: Router, uri: &str) -> (StatusCode, Value) {
    let (status, bytes) = get_raw(app, uri).await;
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

async fn get(state: &SharedState, uri: &str) -> (StatusCode, Value) {
    get_via(router(state.clone()), uri).await
}

async fn deploy(
    pool: &PgPool,
    entity_id: &str,
    entity_type: &str,
    pointer: &str,
    metadata: Value,
    files: &[(&str, &str)],
) {
    let id: i32 = sqlx::query_scalar(
        "INSERT INTO content.deployments (deployer_address, version, entity_type, entity_id, entity_metadata, \
             entity_timestamp, entity_pointers, local_timestamp, auth_chain) \
         VALUES ($1, 'v3', $2, $3, $4, to_timestamp(1700000000), ARRAY[$5], timezone('UTC', now()), '[]'::json) RETURNING id",
    )
    .bind(OWNER)
    .bind(entity_type)
    .bind(entity_id)
    .bind(json!({ "v": metadata }))
    .bind(pointer)
    .fetch_one(pool)
    .await
    .unwrap();
    for (key, hash) in files {
        sqlx::query(
            "INSERT INTO content.content_files (deployment, content_hash, key) VALUES ($1, $2, $3)",
        )
        .bind(id)
        .bind(*hash)
        .bind(*key)
        .execute(pool)
        .await
        .unwrap();
    }
    sqlx::query(
        "INSERT INTO content.active_pointers (pointer, entity_id, entity_type) VALUES ($1, $2, $3)",
    )
    .bind(pointer)
    .bind(entity_id)
    .bind(entity_type)
    .execute(pool)
    .await
    .unwrap();
}

async fn seed(pool: &PgPool) {
    let wearable = |urn: &str, name: &str, cat: &str| {
        json!({ "id": urn, "name": name, "data": { "category": cat, "representations": [{ "bodyShapes": ["BaseMale"], "mainFile": "m.glb", "contents": ["m.glb"] }] },
                "thumbnail": "thumbnail.png", "image": "image.png" })
    };
    let files = [
        ("m.glb", "bafyglb"),
        ("thumbnail.png", "bafythumb"),
        ("image.png", "bafyimg"),
    ];
    deploy(
        pool,
        "bafyeyes",
        "wearable",
        "urn:decentraland:off-chain:base-avatars:eyes_00",
        wearable(
            "urn:decentraland:off-chain:base-avatars:eyes_00",
            "Eyes",
            "eyes",
        ),
        &files,
    )
    .await;
    deploy(
        pool,
        "bafyfemale",
        "wearable",
        "urn:decentraland:off-chain:base-avatars:basefemale",
        wearable(
            "urn:decentraland:off-chain:base-avatars:BaseFemale",
            "Female",
            "body_shape",
        ),
        &files,
    )
    .await;
    let hat_urn = format!("urn:decentraland:matic:collections-v2:{COFFEE}:1");
    deploy(
        pool,
        "bafyhat",
        "wearable",
        &hat_urn,
        wearable(&hat_urn, "Hat", "hat"),
        &files,
    )
    .await;
    let dance_urn = format!("urn:decentraland:matic:collections-v2:{COFFEE}:2");
    deploy(pool, "bafydance", "emote", &dance_urn, json!({ "id": dance_urn, "name": "Dance", "emoteDataADR74": { "category": "dance", "loop": true, "representations": [] } }), &[]).await;
    let mut tp = wearable(TP_URN, "TP Hat", "hat");
    tp["mappings"] =
        json!({ "matic": { TP_CONTRACT: [{ "type": "range", "from": "1", "to": "3" }] } });
    deploy(pool, "bafytphat", "wearable", TP_URN, tp, &files).await;
    deploy(
        pool,
        "bafyoutfits",
        "outfits",
        &format!("{OWNER}:outfits"),
        json!({ "outfits": [{ "slot": 0 }, { "slot": 6 }] }),
        &[],
    )
    .await;

    sqlx::query(
        "INSERT INTO shop.collections (network, contract_address, chain_id, name, creator, owner, urn, is_completed, is_approved, is_editable, \
             created_at, updated_at, reviewed_at, mirrored_at) \
         VALUES ('matic', $1, 137, 'Coffee', $2, $2, $3, true, true, false, 1, 1, 1, timezone('UTC', now()))",
    )
    .bind(COFFEE)
    .bind(OWNER)
    .bind(format!("urn:decentraland:matic:collections-v2:{COFFEE}"))
    .execute(pool)
    .await
    .unwrap();
    for (item, kind, name, cat, rarity) in [
        ("1", "wearable_v2", "Hat", "hat", "epic"),
        ("2", "emote_v1", "Dance", "dance", "rare"),
    ] {
        sqlx::query(
            "INSERT INTO shop.items (network, contract_address, item_id, chain_id, item_type, urn, creator, rarity, total_supply, max_supply, \
                 available, price, name, category, created_at, updated_at, reviewed_at, mirrored_at) \
             VALUES ('matic', $1, $2::numeric, 137, $3, $4, $5, $6, 1, 10, 9, 5, $7, $8, 1, 1, 1, timezone('UTC', now()))",
        )
        .bind(COFFEE)
        .bind(item)
        .bind(kind)
        .bind(format!("urn:decentraland:matic:collections-v2:{COFFEE}:{item}"))
        .bind(OWNER)
        .bind(rarity)
        .bind(name)
        .bind(cat)
        .execute(pool)
        .await
        .unwrap();
    }
    for mirror in ["collections", "items"] {
        for network in ["ethereum", "matic"] {
            sqlx::query("INSERT INTO shop.mirror_state (mirror, network, source_url, last_complete_pass_at) VALUES ($1, $2, 'test', timezone('UTC', now()))")
                .bind(mirror)
                .bind(network)
                .execute(pool)
                .await
                .unwrap();
        }
    }
}

async fn seed_indexes(pool: &PgPool) {
    let now = chrono::Utc::now().naive_utc();
    sqlx::query(
        "INSERT INTO land_authz.index_cursor (network, land_registry, estate_registry, last_block, folded_block, folded_at) \
         VALUES ('ethereum-mainnet', $1, $2, 10, 10, $3)",
    )
    .bind(LAND_REGISTRY_MAINNET)
    .bind(ESTATE_REGISTRY_MAINNET)
    .bind(now)
    .execute(pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO shop.chain_cursor (indexer, network, start_block, last_block, folded_block, folded_at, full_history) \
         VALUES ('nft', 'ethereum', 0, 10, 10, $1, true)",
    )
    .bind(now)
    .execute(pool)
    .await
    .unwrap();
    for (addr, token, x, y, owner, block) in [
        (LAND_REGISTRY_MAINNET, "1", Some(1), Some(2), OWNER, 5i64),
        (
            LAND_REGISTRY_MAINNET,
            "2",
            Some(3),
            Some(4),
            ESTATE_REGISTRY_MAINNET,
            6,
        ),
        (ESTATE_REGISTRY_MAINNET, "5", None, None, OWNER, 8),
        (
            LAND_REGISTRY_MAINNET,
            "9",
            Some(9),
            Some(9),
            "0xbbbb000000000000000000000000000000000002",
            7,
        ),
    ] {
        sqlx::query(
            "INSERT INTO land_authz.token_right (token_address, token_id, x, y, owner, updated_block, updated_log) VALUES ($1, $2::numeric, $3, $4, $5, $6, 0)",
        )
        .bind(addr)
        .bind(token)
        .bind(x)
        .bind(y)
        .bind(owner)
        .bind(block)
        .execute(pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO land_authz.authz_event (block_number, log_index, block_time, token_address, kind, token_id, account) \
             VALUES ($1, 0, $2, $3, 'transfer', $4::numeric, $5)",
        )
        .bind(block)
        .bind(1_600_000_000 + block)
        .bind(addr)
        .bind(token)
        .bind(owner)
        .execute(pool)
        .await
        .unwrap();
    }
    sqlx::query("INSERT INTO land_authz.estate_land (land_token_id, estate_token_id, updated_block, updated_log) VALUES (2, 5, 6, 1)").execute(pool).await.unwrap();
    sqlx::query("INSERT INTO shop.land_data (token_address, token_id, name, description, updated_block, updated_log) VALUES ($1, 1, 'Plot', 'desc', 5, 1)")
        .bind(LAND_REGISTRY_MAINNET)
        .execute(pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO shop.nft_order (network, contract_address, token_id, order_id, marketplace_address, status, seller, price, expires_at, created_at, updated_at, updated_block, updated_log) \
         VALUES ('ethereum', $1, 1, '0x01', '0x8e5660b4ab70168b5a6feea0e0315cb49c8cd539', 'open', $2, 1000, $3, 1, 1, 5, 2)",
    )
    .bind(LAND_REGISTRY_MAINNET)
    .bind(OWNER)
    .bind(chrono::Utc::now().timestamp_millis() + 86_400_000)
    .execute(pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO shop.nft (network, contract_address, token_id, owner, ens_subdomain, minted_block, minted_at, transferred_block, transferred_log, transferred_at) \
         VALUES ('ethereum', $1, 77, $2, 'Alice', 1, 1, 1, 0, 1)",
    )
    .bind(super::nft_index::DCL_REGISTRAR)
    .bind(OWNER)
    .execute(pool)
    .await
    .unwrap();
}

#[tokio::test]
async fn the_g1_lambdas_answer_from_the_mirrors_indexes_and_readers() {
    let Some(db) = ScratchDb::create().await else {
        return;
    };
    let rpc = serve_rpc().await;
    let settings = Settings {
        eth_collections_subgraph_url: Some(serve_graph(eth_collections).await),
        matic_collections_subgraph_url: Some(serve_graph(matic_collections).await),
        marketplace_subgraph_url: Some(serve_graph(marketplace_graph).await),
        third_party_registry_subgraph_url: Some(serve_graph(registry_graph).await),
        eth_rpc_url: Some(rpc.clone()),
        polygon_rpc_url: Some(rpc.clone()),
        market: MarketSettings {
            trade_rpc_urls: RpcEndpoints::from_env(Some(&format!("137={rpc}"))),
            ..MarketSettings::default()
        },
        ..Settings::for_tests()
    };
    let state = Arc::new(MarketplaceState::new(
        db.pool.clone(),
        crate::http::Outbound::for_tests(),
        settings,
    ));
    state
        .test_allow_loopback
        .store(true, std::sync::atomic::Ordering::Relaxed);

    // Before the sources exist: a route with no reader answers the envelope;
    // lands, the name owner and the catalogue read their subgraphs (the
    // fakes hold nothing for them), and answer the envelope on a node with
    // no subgraph configured either.
    for (uri, source) in [
        (
            format!("/lambdas/users/{OWNER}/genesis-parcels"),
            "land-authz-index",
        ),
        ("/lambdas/collections".to_string(), "collections-mirror"),
    ] {
        let (status, body) = get(&state, &uri).await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{uri}: {body}");
        assert_eq!(body["source"], source, "{uri}: {body}");
    }
    let (status, body) = get(&state, &format!("/lambdas/users/{OWNER}/lands")).await;
    assert_eq!(
        (status, body),
        (
            StatusCode::OK,
            json!({ "elements": [], "totalAmount": 0, "pageNum": 1, "pageSize": 100 })
        )
    );
    let (status, _) = get(&state, "/lambdas/names/alice/owner").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, body) = get(&state, "/lambdas/collections/emotes?textSearch=nothing").await;
    assert_eq!(
        (status, body["emotes"].as_array().map(Vec::len)),
        (StatusCode::OK, Some(0)),
        "{body}"
    );
    let bare = Arc::new(MarketplaceState::new(
        db.pool.clone(),
        crate::http::Outbound::for_tests(),
        Settings::for_tests(),
    ));
    for (uri, source) in [
        (format!("/lambdas/users/{OWNER}/lands"), "land-authz-index"),
        ("/lambdas/names/alice/owner".to_string(), "nft-index"),
        (
            "/lambdas/collections/emotes?textSearch=dance".to_string(),
            "collections-subgraph:matic",
        ),
    ] {
        let (status, body) = get(&bare, &uri).await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{uri}: {body}");
        assert_eq!(body["source"], source, "{uri}: {body}");
    }

    seed(&db.pool).await;
    seed_indexes(&db.pool).await;
    state.mirror_ready.clear();

    // Comms consumes Shop's same name owner result, including the ready local
    // index when no subgraph exists and case-insensitive optional suffixes.
    use crate::comms::interfaces::NameOwnerReader;
    for name in ["Alice", "ALICE.dcl.eth", "ALICE.DCL.ETH"] {
        let owner = NameOwnerReader::name_owner(bare.as_ref(), name)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(owner.as_str(), OWNER);
        let (status, body) = get(&bare, &format!("/lambdas/names/{name}/owner")).await;
        assert_eq!((status, body), (StatusCode::OK, json!({"owner":OWNER})));
    }
    assert_eq!(
        NameOwnerReader::name_owner(bare.as_ref(), "nobody")
            .await
            .unwrap(),
        None
    );

    let (status, body) = get(&state, "/lambdas/collections").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let ids: Vec<&str> = body["collections"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["id"].as_str().unwrap())
        .collect();
    assert_eq!(
        ids,
        [
            "urn:decentraland:off-chain:base-avatars",
            "urn:decentraland:off-chain:base-emotes",
            &format!("urn:decentraland:matic:collections-v2:{COFFEE}")
        ]
    );
    let (_, nfts) = get(&state, "/lambdas/nfts/collections").await;
    assert_eq!(nfts, body);

    let (status, body) = get(&state, &format!("/lambdas/collections/wearables?collectionId=urn:decentraland:matic:collections-v2:{COFFEE}")).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["wearables"].as_array().unwrap().len(), 1, "{body}");
    assert_eq!(
        body["wearables"][0]["thumbnail"],
        "http://localhost/content/contents/bafythumb"
    );
    assert_eq!(
        body["filters"],
        json!({ "collectionIds": [format!("urn:decentraland:matic:collections-v2:{COFFEE}")] })
    );
    let (_, body) = get(
        &state,
        "/lambdas/collections/wearables?collectionType=base-wearable&category=eyes",
    )
    .await;
    let wearables = body["wearables"].as_array().unwrap();
    assert_eq!(wearables.len(), 1, "{body}");
    assert_eq!(
        wearables[0]["id"],
        "urn:decentraland:off-chain:base-avatars:eyes_00"
    );
    let (_, body) = get(
        &state,
        "/lambdas/collections/wearables?textSearch=a&limit=1",
    )
    .await;
    assert_eq!(
        body["message"],
        "The text search must be at least 3 characters long"
    );
    let (_, body) = get(
        &state,
        "/lambdas/collections/wearables?collectionType=on-chain&limit=1&textSearch=hat",
    )
    .await;
    assert_eq!(body["wearables"].as_array().unwrap().len(), 1, "{body}");
    assert!(body["pagination"].get("next").is_none(), "{body}");
    let (_, body) = get(&state, "/lambdas/collections/emotes?textSearch=dance").await;
    assert_eq!(
        body["emotes"][0]["emoteDataADR74"]["category"], "dance",
        "{body}"
    );

    let (status, body) = get(&state, &format!("/lambdas/outfits/{OWNER}")).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["metadata"]["namesForExtraSlots"], json!(["alice"]));
    assert_eq!(
        body["metadata"]["outfits"].as_array().unwrap().len(),
        2,
        "a name owner keeps the extra slots"
    );
    let (status, _) = get(
        &state,
        "/outfits/0xbbbb000000000000000000000000000000000002",
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    let (status, body) = get(&state, &format!("/explorer/{OWNER}/wearables?pageSize=10")).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let elements = body["elements"].as_array().unwrap();
    let kinds: Vec<(&str, &str)> = elements
        .iter()
        .map(|e| (e["type"].as_str().unwrap(), e["urn"].as_str().unwrap()))
        .collect();
    assert_eq!(body["totalAmount"], 4, "{kinds:?}");
    assert_eq!(
        kinds[0],
        (
            "on-chain",
            format!("urn:decentraland:matic:collections-v2:{COFFEE}:1").as_str()
        ),
        "the rarest first"
    );
    assert_eq!(elements[0]["amount"], 2);
    assert_eq!(elements[0]["itemType"], "wearable_v2");
    assert_eq!(elements[0]["rarity"], "epic");
    assert!(kinds.contains(&("third-party", TP_URN)), "{kinds:?}");
    assert!(
        kinds.contains(&(
            "base-wearable",
            "urn:decentraland:off-chain:base-avatars:eyes_00"
        )),
        "{kinds:?}"
    );
    let tp = elements
        .iter()
        .find(|e| e["type"] == "third-party")
        .unwrap();
    assert_eq!(
        tp["individualData"][0]["tokenId"],
        format!("matic:{TP_CONTRACT}:2")
    );
    let (_, body) = get(
        &state,
        &format!("/lambdas/explorer/{OWNER}/wearables?collectionType=on-chain&trimmed=true"),
    )
    .await;
    assert_eq!(body["totalAmount"], 1);
    assert_eq!(body["elements"][0]["entity"]["thumbnail"], "bafythumb");
    let (_, body) = get(&state, &format!("/explorer/{OWNER}/emotes")).await;
    assert_eq!(
        body["totalAmount"], 0,
        "the owner holds no emote copy: {body}"
    );
    let (status, body) = get(&state, "/explorer/not-an-address/wearables").await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");

    let (status, body) = get(
        &state,
        &format!("/lambdas/users/{OWNER}/third-party-wearables?includeDefinitions"),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["totalAmount"], 1);
    assert_eq!(body["elements"][0]["urn"], TP_URN);
    assert!(body["elements"][0]["definition"].is_object(), "{body}");
    let (_, body) = get(&state, &format!("/lambdas/users/{OWNER}/third-party-wearables/urn:decentraland:matic:collections-thirdparty:tp")).await;
    assert_eq!(body["totalAmount"], 1, "{body}");
    let (_, body) = get(&state, &format!("/lambdas/users/{OWNER}/third-party-wearables/urn:decentraland:matic:collections-thirdparty:other")).await;
    assert_eq!(body["totalAmount"], 0, "{body}");
    let (_, body) = get(&state, &format!("/lambdas/collections/wearables-by-owner/{OWNER}?collectionId=urn:decentraland:matic:collections-thirdparty:tp")).await;
    assert_eq!(body, json!([{ "urn": TP_URN, "amount": 1 }]));
    let (_, body) = get(&state, "/lambdas/third-party-integrations").await;
    assert_eq!(
        body,
        json!({ "data": [{ "name": "TP", "description": "A provider", "urn": "urn:decentraland:matic:collections-thirdparty:tp" }] })
    );

    let (status, body) = get(&state, &format!("/lambdas/users/{OWNER}/lands")).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["totalAmount"], 2);
    assert_eq!(
        body["elements"][0]["category"], "estate",
        "the newest transfer first: {body}"
    );
    assert_eq!(
        body["elements"][0]["image"],
        "https://api.decentraland.org/v1/estates/5/map.png"
    );
    let parcel = &body["elements"][1];
    assert_eq!(
        parcel,
        &json!({ "contractAddress": LAND_REGISTRY_MAINNET, "tokenId": "1", "category": "parcel", "name": "Plot", "x": "1", "y": "2",
                 "description": "desc", "price": "1000", "image": "https://api.decentraland.org/v1/parcels/1/2/map.png" })
    );
    let (_, body) = get(
        &state,
        &format!("/lambdas/users/{OWNER}/lands?pageNum=3&pageSize=1"),
    )
    .await;
    assert_eq!(body["elements"], json!([]));
    assert_eq!(
        body["totalAmount"], 2,
        "an empty page past the end still counts"
    );

    let (status, body) = get(&state, &format!("/lambdas/users/{OWNER}/genesis-parcels")).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        body["elements"],
        json!([{ "tokenId": "1", "x": "1", "y": "2", "viaEstate": false }, { "tokenId": "2", "x": "3", "y": "4", "viaEstate": true }])
    );
    assert_eq!(body["totalAmount"], 2);

    let (status, body) = get(&state, "/lambdas/names/ALICE.dcl.eth/owner").await;
    assert_eq!((status, body), (StatusCode::OK, json!({ "owner": OWNER })));
    let (status, _) = get(&state, "/lambdas/names/nobody/owner").await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    let (_, body) = get(&state, "/lambdas/contracts/pois").await;
    assert_eq!(body, json!(["10,20"]));
    let (_, body) = get(&state, "/lambdas/contracts/denylisted-names").await;
    assert_eq!(body, json!(["badname"]));

    db.drop().await;
}

/// Before the indexes and the items mirror exist, the routes the old lambdas
/// answered from the subgraphs answer from them here: the catalogue's on-chain
/// leg from the collections subgraphs, the name holder and the owner's lands
/// from the marketplace subgraph, the parcel operators and the update
/// operator's parcels from the land-manager subgraph; without
/// `LAND_SUBGRAPH_URL` the two land routes keep the index envelope.
#[tokio::test]
async fn the_lambdas_read_the_subgraphs_before_their_indexes_and_mirrors() {
    let Some(db) = ScratchDb::create().await else {
        return;
    };
    let hat_urn = format!("urn:decentraland:matic:collections-v2:{COFFEE}:1");
    let dance_urn = format!("urn:decentraland:matic:collections-v2:{COFFEE}:2");
    let files = [
        ("m.glb", "bafyglb"),
        ("thumbnail.png", "bafythumb"),
        ("image.png", "bafyimg"),
    ];
    let hat = json!({ "id": hat_urn, "name": "Hat", "data": { "category": "hat", "representations": [{ "bodyShapes": ["BaseMale"], "mainFile": "m.glb", "contents": ["m.glb"] }] },
                      "thumbnail": "thumbnail.png", "image": "image.png" });
    deploy(&db.pool, "bafyhat", "wearable", &hat_urn, hat, &files).await;
    deploy(&db.pool, "bafydance", "emote", &dance_urn, json!({ "id": dance_urn, "name": "Dance", "emoteDataADR74": { "category": "dance", "loop": true, "representations": [] } }), &[]).await;

    let land_url = serve_graph_vars(land_reader).await;
    let settings = Settings {
        eth_collections_subgraph_url: Some(serve_graph(eth_collections).await),
        matic_collections_subgraph_url: Some(serve_graph(matic_items).await),
        marketplace_subgraph_url: Some(serve_graph_vars(marketplace_reader).await),
        land_subgraph_url: Some(land_url.clone()),
        ..Settings::for_tests()
    };
    let state = Arc::new(MarketplaceState::new(
        db.pool.clone(),
        crate::http::Outbound::for_tests(),
        settings,
    ));
    state
        .test_allow_loopback
        .store(true, std::sync::atomic::Ordering::Relaxed);

    let (status, body) = get(
        &state,
        "/lambdas/collections/wearables?textSearch=hat&limit=10000",
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["wearables"][0]["id"], hat_urn, "{body}");
    assert_eq!(body["pagination"], json!({ "limit": 500 }));
    assert_eq!(body["filters"], json!({ "textSearch": "hat" }));
    let (status, body) = get(&state, "/lambdas/collections/emotes?textSearch=dance").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        body["emotes"][0]["emoteDataADR74"]["category"], "dance",
        "{body}"
    );
    let (status, body) = get(
        &state,
        "/lambdas/collections/wearables?collectionType=on-chain&orderBy=rarity&textSearch=hat",
    )
    .await;
    assert_eq!(
        (status, body["source"].as_str()),
        (StatusCode::SERVICE_UNAVAILABLE, Some("items-mirror")),
        "{body}"
    );

    let (status, body) = get(&state, "/lambdas/names/ALICE.dcl.eth/owner").await;
    assert_eq!((status, body), (StatusCode::OK, json!({ "owner": OWNER })));
    let (status, bytes) = get_raw(router(state.clone()), "/lambdas/names/nobody/owner").await;
    assert_eq!(
        (status, bytes.len()),
        (StatusCode::NOT_FOUND, 0),
        "the old empty 404"
    );

    let (status, body) = get(&state, &format!("/lambdas/users/{OWNER}/lands")).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        (
            body["totalAmount"].as_i64(),
            body["pageNum"].as_i64(),
            body["pageSize"].as_i64()
        ),
        (Some(2), Some(1), Some(100))
    );
    assert_eq!(
        body["elements"][0]["category"], "estate",
        "the newest transfer first: {body}"
    );
    assert_eq!(
        body["elements"][0]["image"],
        "https://api.decentraland.org/v1/estates/5/map.png"
    );
    assert!(
        body["elements"][0].get("description").is_none(),
        "an estate without data: {body}"
    );
    let (_, body) = get(
        &state,
        &format!("/lambdas/users/{OWNER}/lands?pageSize=1&pageNum=2"),
    )
    .await;
    assert_eq!(
        body["elements"][0],
        json!({ "contractAddress": LAND_REGISTRY_MAINNET, "tokenId": "1", "category": "parcel", "name": "Plot", "x": "1", "y": "2",
                "description": "desc", "price": "1000", "image": "https://api.decentraland.org/v1/parcels/1/2/map.png" })
    );
    let (_, body) = get(
        &state,
        "/lambdas/users/0xdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef/lands?pageNum=99999999999",
    )
    .await;
    assert_eq!(
        body,
        json!({ "elements": [], "totalAmount": 0, "pageNum": 99999999999i64, "pageSize": 100 })
    );

    let land = crate::land_authz::router(Arc::new(crate::land_authz::LandAuthzState::new(
        db.pool.clone(),
        crate::http::Outbound::for_tests(),
        crate::land_authz::Settings::for_tests(),
        Some(state.index.clone()),
    )));
    let (status, body) = get_via(land.clone(), "/lambdas/parcels/0/0/operators").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        body,
        json!({ "owner": OWNER, "operator": null, "updateOperator": null, "updateManagers": [MANAGER], "approvedForAll": [] })
    );
    let (status, body) = get_via(land.clone(), "/lambdas/parcels/5/5/operators").await;
    assert_eq!(
        (status, body["error"].as_str()),
        (StatusCode::NOT_FOUND, Some("Bad Request")),
        "{body}"
    );
    let (status, body) = get_via(
        land.clone(),
        &format!("/lambdas/users/{UPDATE_OP}/lands-permissions?pageSize=1&pageNum=2"),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["totalAmount"], 2);
    assert_eq!(
        body["elements"],
        json!([{ "id": "2", "x": "3", "y": "4", "owner": OWNER, "updateOperator": UPDATE_OP }])
    );
    let (_, body) = get_via(
        land,
        "/lambdas/users/0xdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef/lands-permissions",
    )
    .await;
    assert_eq!(
        body,
        json!({ "elements": [], "totalAmount": 0, "pageNum": 1, "pageSize": 100 })
    );

    let without_land = Arc::new(MarketplaceState::new(
        db.pool.clone(),
        crate::http::Outbound::for_tests(),
        Settings {
            land_subgraph_url: None,
            ..Settings::for_tests()
        },
    ));
    let land = crate::land_authz::router(Arc::new(crate::land_authz::LandAuthzState::new(
        db.pool.clone(),
        crate::http::Outbound::for_tests(),
        crate::land_authz::Settings::for_tests(),
        Some(without_land.index.clone()),
    )));
    for uri in [
        "/lambdas/parcels/0/0/operators",
        &format!("/lambdas/users/{UPDATE_OP}/lands-permissions"),
    ] {
        let (status, body) = get_via(land.clone(), uri).await;
        assert_eq!(
            (status, body["configure"].as_str()),
            (
                StatusCode::SERVICE_UNAVAILABLE,
                Some("LAND_AUTHZ_INDEX_ENABLED")
            ),
            "{uri}: {body}"
        );
    }

    db.drop().await;
}
