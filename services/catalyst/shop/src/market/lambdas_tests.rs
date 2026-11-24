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
use axum::http::{Method, Request, StatusCode, header};
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
        crate::db::scratch::create_database_with_schema(&admin, &name).await;
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

/// A subgraph that answers every document with a GraphQL error.
async fn serve_failing_graph() -> String {
    use axum::routing::post;
    serve(Router::new().route(
        "/",
        post(|| async { axum::Json(json!({ "errors": [{ "message": "indexer down" }] })) }),
    ))
    .await
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

/// The collections subgraphs answer nothing: ownership is the nft index's
/// (`seed_indexes`), and a query for it here is a test failure.
fn matic_collections(query: &str) -> Value {
    assert!(
        !query.contains("urn_in") && !query.contains("owner:"),
        "an ownership query reached the collections subgraph: {query}"
    );
    json!({ "nfts": [] })
}

fn eth_collections(query: &str) -> Value {
    if query.contains("o0: nfts(") {
        return empty_owner_aliases(query);
    }
    json!({ "nfts": [] })
}

fn empty_owner_aliases(query: &str) -> Value {
    let mut data = json!({});
    for alias in query
        .split_whitespace()
        .filter_map(|word| word.strip_suffix(':'))
    {
        if alias
            .strip_prefix('o')
            .is_some_and(|n| n.parse::<usize>().is_ok())
        {
            data[alias] = json!([]);
        }
    }
    data
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
              "transferredAt": "5", "parcel": { "data": { "description": "desc" } }, "estate": null, "activeOrder": { "price": "1000" },
              "image": "https://maps.example/v1/parcels/1/2/map.png" },
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
        "6352211e" => {
            let token = IERC721::ownerOfCall::abi_decode(&hex::decode(data).unwrap())
                .unwrap()
                .tokenId;
            let holder = if token == U256::from(2) {
                owner
            } else {
                Address::ZERO
            };
            format!("0x{}", hex::encode(holder.abi_encode()))
        }
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
    let linked = |token: u32| format!("{TP_URN}:matic:{TP_CONTRACT}:{token}");
    let outfit = |slot: u32, wearables: Vec<String>| json!({ "slot": slot, "outfit": { "wearables": wearables } });
    deploy(
        pool,
        "bafyoutfits",
        "outfits",
        &format!("{OWNER}:outfits"),
        json!({ "outfits": [
            outfit(0, vec!["urn:decentraland:off-chain:base-avatars:Eyes_00".into(), hat_urn.clone()]),
            outfit(1, vec![format!("{hat_urn}:10")]),
            outfit(2, vec![format!("{hat_urn}:99")]),
            outfit(3, vec![linked(2), format!("urn:decentraland:matic:collections-v2:{COFFEE}:7")]),
            outfit(4, vec![linked(3)]),
            outfit(6, vec![]),
        ] }),
        &[],
    )
    .await;

    seed_mirrors(pool).await;
}

/// The collections and items mirrors' rows and complete passes, without
/// the content entities [`seed`] deploys.
async fn seed_mirrors(pool: &PgPool) {
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
    for (item, kind, name, cat, rarity, price) in [
        ("1", "wearable_v2", "Hat", "hat", "epic", 5),
        ("2", "emote_v1", "Dance", "dance", "rare", 5),
        // An item no deployed entity describes.
        ("9", "wearable_v2", "Orphan", "hat", "common", 1),
    ] {
        sqlx::query(
            "INSERT INTO shop.items (network, contract_address, item_id, chain_id, item_type, urn, creator, rarity, total_supply, max_supply, \
                 available, price, name, category, created_at, updated_at, reviewed_at, mirrored_at) \
             VALUES ('matic', $1, $2::numeric, 137, $3, $4, $5, $6, 1, 10, 9, $9, $7, $8, 1, 1, 1, timezone('UTC', now()))",
        )
        .bind(COFFEE)
        .bind(item)
        .bind(kind)
        .bind(format!("urn:decentraland:matic:collections-v2:{COFFEE}:{item}"))
        .bind(OWNER)
        .bind(rarity)
        .bind(name)
        .bind(cat)
        .bind(price)
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
        "INSERT INTO shop.chain_cursor (indexer, network, start_block, last_block, folded_block, folded_at, full_history, chain_id) \
         VALUES ('nft', 'ethereum', 0, 10, 10, $1, true, 1)",
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
    // The owner's copies: two of the hat (token 11 the newest transfer),
    // one of the orphan item.
    for (token, item, block, at) in [(10, 1, 20, 200), (11, 1, 30, 300), (12, 9, 10, 100)] {
        sqlx::query(
            "INSERT INTO shop.nft (network, contract_address, token_id, owner, item_id, issued_id, minted_block, minted_at, transferred_block, transferred_log, transferred_at) \
             VALUES ('matic', $1, $2, $3, $4, $2, $5, $6, $5, 0, $6)",
        )
        .bind(COFFEE)
        .bind(token as i64)
        .bind(OWNER)
        .bind(item as i64)
        .bind(block as i64)
        .bind(at as i64)
        .execute(pool)
        .await
        .unwrap();
    }
    super::owned_items::test_support::publish(
        pool,
        &super::chain_walk::ChainNet::ALL,
        false,
        std::time::Duration::ZERO,
    )
    .await;
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
    let settings = super::owned_items::test_support::walking(settings);
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
    // This node walks the nft index, so its NAMEs read the index alone:
    // unpublished, the name owner is its envelope, never the subgraph's.
    let (status, body) = get(&state, "/lambdas/names/alice/owner").await;
    assert_eq!(
        (status, body["source"].as_str()),
        (StatusCode::SERVICE_UNAVAILABLE, Some("nft-index")),
        "{body}"
    );
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
    for name in ["Alice", "ALICE.dcl.eth", "ALICE.DCL.ETH"] {
        let owner = bare.name_owner(name).await.unwrap().unwrap();
        assert_eq!(owner.as_str(), OWNER);
        let (status, body) = get(&bare, &format!("/lambdas/names/{name}/owner")).await;
        assert_eq!((status, body), (StatusCode::OK, json!({"owner":OWNER})));
    }
    assert_eq!(bare.name_owner("nobody").await.unwrap(), None);

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
    assert_eq!(
        body["pagination"],
        json!({ "limit": 500 }),
        "no lastId asked, none echoed"
    );
    let (_, body) = get(
        &state,
        "/lambdas/collections/emotes?textSearch=dance&lastId=URN:DECENTRALAND:A",
    )
    .await;
    assert_eq!(
        body["pagination"],
        json!({ "limit": 500, "lastId": "urn:decentraland:a" }),
        "the request's lastId, lowercased"
    );
    // `textSearch` matches the mirrored subgraph `searchText`, case-sensitively,
    // and the lowercased name only where the mirror has not filled it.
    sqlx::query(
        "UPDATE shop.items SET search_text = 'coffee dance moves' WHERE item_type = 'emote_v1'",
    )
    .execute(&db.pool)
    .await
    .unwrap();
    for (text, found) in [("moves", 1), ("dance", 1), ("Dance", 1), ("emote", 0)] {
        let (_, body) = get(
            &state,
            &format!("/lambdas/collections/emotes?textSearch={text}&limit=7"),
        )
        .await;
        assert_eq!(
            body["emotes"].as_array().map(Vec::len),
            Some(found),
            "{text}: {body}"
        );
    }
    sqlx::query("UPDATE shop.items SET search_text = 'renamed' WHERE item_type = 'emote_v1'")
        .execute(&db.pool)
        .await
        .unwrap();
    let (_, body) = get(
        &state,
        "/lambdas/collections/emotes?textSearch=dance&limit=8",
    )
    .await;
    assert_eq!(
        body["emotes"].as_array().map(Vec::len),
        Some(0),
        "the name alone no longer matches: {body}"
    );

    let (status, body) = get(&state, &format!("/lambdas/outfits/{OWNER}")).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    // The NAME as minted (the index's `Alice`), as the subgraph listed it.
    assert_eq!(body["metadata"]["namesForExtraSlots"], json!(["Alice"]));
    let hat = format!("urn:decentraland:matic:collections-v2:{COFFEE}:1");
    assert_eq!(
        body["metadata"]["outfits"],
        json!([
            // The newest copy's token when none is named; lowercased.
            { "slot": 0, "outfit": { "wearables": ["urn:decentraland:off-chain:base-avatars:eyes_00", format!("{hat}:11")] } },
            { "slot": 1, "outfit": { "wearables": [format!("{hat}:10")] } },
            // Slot 2 names a token the owner does not hold. Slot 3 keeps the
            // upstream quirk: the walk stops at the item not owned and the
            // owned third-party item decides. Slot 4's NFT is not held.
            { "slot": 3, "outfit": { "wearables": [format!("{TP_URN}:matic:{TP_CONTRACT}:2")] } },
            // A name owner keeps the extra slots.
            { "slot": 6, "outfit": { "wearables": [] } },
        ]),
        "{body}"
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
    assert_eq!(body["elements"][0]["entity"]["metadata"]["isSmart"], false);
    assert!(
        body["elements"][0].get("amount").is_none(),
        "amount only with includeAmount: {body}"
    );
    // The item-type filters keep the on-chain leg's matching types and drop
    // the base and third-party legs.
    for (filter, total) in [
        ("isSmartWearable=true", 0),
        ("network=MATIC", 1),
        ("network=ETHEREUM", 0),
        ("network=polygon", 4),
    ] {
        let (_, body) = get(&state, &format!("/explorer/{OWNER}/wearables?{filter}")).await;
        assert_eq!(body["totalAmount"], total, "{filter}: {body}");
    }
    let tp_collection = "urn:decentraland:matic:collections-thirdparty:tp";
    for (filter, total) in [
        (format!("thirdPartyCollectionId={tp_collection}"), 1),
        (
            format!(
                "thirdPartyCollectionId={tp_collection}&thirdPartyCollectionId=urn:decentraland:amoy:collections-thirdparty:tp"
            ),
            1,
        ),
        (
            format!(
                "thirdPartyCollectionId={tp_collection}&thirdPartyCollectionId={tp_collection}"
            ),
            1,
        ),
        // Two distinct values naming one provider list it once (lamb2 lists
        // it twice, explorer-handler.ts:119-143), so the value list cannot
        // multiply the answer.
        (
            format!(
                "thirdPartyCollectionId={tp_collection}:c1&thirdPartyCollectionId={tp_collection}"
            ),
            1,
        ),
        (
            (0..500)
                .map(|i| format!("thirdPartyCollectionId={tp_collection}:x{i}"))
                .collect::<Vec<_>>()
                .join("&"),
            1,
        ),
        ("thirdPartyCollectionId=not-a-urn".to_string(), 0),
    ] {
        let (_, body) = get(
            &state,
            &format!("/explorer/{OWNER}/wearables?collectionType=third-party&{filter}"),
        )
        .await;
        assert_eq!(body["totalAmount"], total, "{filter}: {body}");
    }
    let not_found = json!({ "error": "The requested items cannot be fetched right now", "message": "Third Party not found: other" });
    let (status, body) = get(
        &state,
        &format!("/explorer/{OWNER}/wearables?thirdPartyCollectionId=urn:decentraland:matic:collections-thirdparty:other"),
    )
    .await;
    assert_eq!((status, &body), (StatusCode::BAD_GATEWAY, &not_found));
    // No count limit; values that name no provider are dropped.
    let many = "thirdPartyCollectionId=urn:a&".repeat(101);
    let (status, body) = get(
        &state,
        &format!("/explorer/{OWNER}/wearables?collectionType=third-party&{many}"),
    )
    .await;
    assert_eq!(
        (status, &body["totalAmount"]),
        (StatusCode::OK, &json!(0)),
        "{body}"
    );
    let (_, body) = get(&state, &format!("/explorer/{OWNER}/emotes")).await;
    assert_eq!(
        body["totalAmount"], 0,
        "the owner holds no emote copy: {body}"
    );
    // Any `:address`: one that is not an Ethereum address owns nothing, the
    // base wearables still answer.
    let (status, body) = get(&state, "/explorer/not-an-address/wearables").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let types: Vec<&str> = body["elements"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|e| e["type"].as_str())
        .collect();
    assert!(types.iter().all(|t| *t == "base-wearable"), "{body}");
    let (_, body) = get(
        &state,
        "/explorer/not-an-address/wearables?collectionType=on-chain",
    )
    .await;
    assert_eq!(body["totalAmount"], 0, "{body}");

    // `/users/{address}/wearables`: lamb2's filters, order and include keys.
    let hat = format!("urn:decentraland:matic:collections-v2:{COFFEE}:1");
    let orphan = format!("urn:decentraland:matic:collections-v2:{COFFEE}:9");
    let urns_of = |body: &Value| -> Vec<String> {
        body["elements"]
            .as_array()
            .unwrap()
            .iter()
            .map(|e| e["urn"].as_str().unwrap().to_string())
            .collect()
    };
    for (query, entity_key, definition_key) in [
        ("includeEntities", true, false),
        ("includeDefinitions", false, true),
    ] {
        let (status, body) =
            get(&state, &format!("/lambdas/users/{OWNER}/wearables?{query}")).await;
        assert_eq!(status, StatusCode::OK, "{body}");
        assert_eq!(
            urns_of(&body),
            [hat.clone(), orphan.clone()],
            "{query}: {body}"
        );
        let [with, without] = [&body["elements"][0], &body["elements"][1]];
        assert_eq!(with.get("entity").is_some(), entity_key, "{query}: {body}");
        assert_eq!(
            with.get("definition").is_some(),
            definition_key,
            "{query}: {body}"
        );
        assert!(
            without.get("entity").is_none() && without.get("definition").is_none(),
            "{query}: a missing entity or definition leaves the key out, never null: {body}"
        );
    }
    let (status, body) = get(
        &state,
        &format!("/lambdas/users/{OWNER}/wearables?category=HAT&rarity=Epic"),
    )
    .await;
    assert_eq!(
        (status, urns_of(&body)),
        (StatusCode::OK, vec![hat.clone()]),
        "{body}"
    );
    let (status, body) = get(
        &state,
        &format!("/lambdas/users/{OWNER}/wearables?direction=sideways"),
    )
    .await;
    assert_eq!(
        (status, urns_of(&body)),
        (StatusCode::OK, vec![hat.clone(), orphan.clone()]),
        "no orderBy: no validation, no sort: {body}"
    );
    let (_, body) = get(
        &state,
        &format!("/lambdas/users/{OWNER}/wearables?orderBy=rarity&direction=asc"),
    )
    .await;
    assert_eq!(urns_of(&body), [orphan.clone(), hat.clone()], "{body}");

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
    for uri in [
        format!("/lambdas/users/{OWNER}/third-party-wearables?pageSize=5000"),
        format!(
            "/lambdas/users/{OWNER}/third-party-wearables/urn:decentraland:amoy:collections-thirdparty:tp?pageSize=5000"
        ),
    ] {
        let (status, body) = get(&state, &uri).await;
        assert_eq!(status, StatusCode::OK, "no pageSize cap: {uri}: {body}");
        assert_eq!(
            (body["pageSize"].as_i64(), body["totalAmount"].as_i64()),
            (Some(5000), Some(1)),
            "{body}"
        );
    }
    let (status, body) = get(&state, &format!("/lambdas/users/{OWNER}/third-party-wearables/urn:decentraland:matic:collections-thirdparty:other")).await;
    assert_eq!((status, body), (StatusCode::BAD_GATEWAY, not_found));
    let (_, body) = get(&state, &format!("/lambdas/collections/wearables-by-owner/{OWNER}?collectionId=urn:decentraland:matic:collections-thirdparty:tp")).await;
    assert_eq!(body, json!([{ "urn": TP_URN, "amount": 1 }]));
    let (_, body) = get(&state, "/lambdas/third-party-integrations").await;
    assert_eq!(
        body,
        json!({ "data": [{ "name": "TP", "description": "A provider", "urn": "urn:decentraland:matic:collections-thirdparty:tp" }] })
    );

    // Profiles keep the v1 third-party items unchecked and the linked items
    // whose NFT the mapping includes and the owner holds, after the others.
    let hat = format!("urn:decentraland:matic:collections-v2:{COFFEE}:1");
    let linked = |token: u32| format!("{TP_URN}:matic:{TP_CONTRACT}:{token}");
    deploy(
        &db.pool,
        "bafyprofile",
        "profile",
        OWNER,
        json!({ "avatars": [{ "name": "alice", "avatar": { "bodyShape": "urn:decentraland:off-chain:base-avatars:BaseFemale", "wearables": [
            linked(2),
            "urn:decentraland:off-chain:base-avatars:eyes_00",
            linked(3),
            TP_URN,
            hat,
            linked(9),
        ], "emotes": [] } }] }),
        &[],
    )
    .await;
    let (status, body) = get(&state, &format!("/lambdas/profiles/{OWNER}")).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        body["avatars"][0]["avatar"]["wearables"],
        json!([
            "urn:decentraland:off-chain:base-avatars:eyes_00",
            format!("{hat}:11"),
            TP_URN,
            linked(2)
        ]),
        "the newest held token is appended (ENSURE_ERC_721); token 3 is mapped but not held, token 9 is not mapped: {body}"
    );

    // An entity a remote list (`DENYLIST_URLS`) names is left out of the
    // lambdas and explorer reads, as lamb2 reading the content server's
    // filtered `/entities/active` leaves it out.
    let denylist = Arc::new(crate::content::Denylist::default());
    let listed = Arc::new(
        MarketplaceState::new(
            db.pool.clone(),
            crate::http::Outbound::for_tests(),
            state.settings.clone(),
        )
        .with_denylist(denylist.clone()),
    );
    listed
        .test_allow_loopback
        .store(true, std::sync::atomic::Ordering::Relaxed);
    denylist.set_remote(
        "https://registry.example/denylist",
        [
            "bafyhat",
            "bafyeyes",
            "bafydance",
            "bafyprofile",
            "bafyoutfits",
        ]
        .map(String::from),
    );
    let (status, body) = get(&listed, &format!("/explorer/{OWNER}/wearables?pageSize=10")).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let urns: Vec<&str> = body["elements"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|e| e["urn"].as_str())
        .collect();
    assert_eq!(
        urns,
        [TP_URN, "urn:decentraland:off-chain:base-avatars:BaseFemale"],
        "the listed hat and eyes are left out, the others stay: {body}"
    );
    let (_, body) = get(&listed, &format!("/explorer/{OWNER}/emotes")).await;
    assert_eq!(body["totalAmount"], 0, "{body}");
    // The owned item stays (it comes from the index); its definition goes.
    let (_, body) = get(
        &listed,
        &format!("/lambdas/users/{OWNER}/wearables?includeDefinitions"),
    )
    .await;
    assert_eq!(body["elements"][0]["urn"], hat, "{body}");
    assert_eq!(body["elements"][0]["definition"], Value::Null, "{body}");
    let (_, body) = get(&listed, &format!("/lambdas/collections/wearables?collectionId=urn:decentraland:matic:collections-v2:{COFFEE}")).await;
    assert_eq!(body["wearables"], json!([]), "{body}");
    let (_, body) = get(
        &listed,
        "/lambdas/collections/wearables?collectionType=base-wearable&category=eyes",
    )
    .await;
    assert_eq!(body["wearables"], json!([]), "{body}");
    let (status, _) = get(&listed, &format!("/lambdas/profiles/{OWNER}")).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _) = get(&listed, &format!("/lambdas/outfits/{OWNER}")).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    // Lifted on the next good fetch.
    denylist.set_remote("https://registry.example/denylist", Vec::<String>::new());
    listed.profile_cache.clear();
    let (status, _) = get(&listed, &format!("/lambdas/profiles/{OWNER}")).await;
    assert_eq!(status, StatusCode::OK);

    // The chain endpoint down: the explorer and the profiles leave the
    // third-party items they could not check out, the dedicated route and
    // the outfits answer the old lambdas' FetcherError.
    let down = {
        use axum::routing::post;
        serve(Router::new().route("/", post(|| async { StatusCode::BAD_GATEWAY }))).await
    };
    let mut down_settings = state.settings.clone();
    down_settings.eth_rpc_url = Some(down.clone());
    down_settings.polygon_rpc_url = Some(down.clone());
    down_settings.market.trade_rpc_urls = RpcEndpoints::from_env(Some(&format!("137={down}")));
    let down_state = Arc::new(MarketplaceState::new(
        db.pool.clone(),
        crate::http::Outbound::for_tests(),
        down_settings,
    ));
    down_state
        .test_allow_loopback
        .store(true, std::sync::atomic::Ordering::Relaxed);
    let (status, body) = get(&down_state, &format!("/explorer/{OWNER}/wearables")).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["totalAmount"], 3, "{body}");
    assert!(
        body["elements"]
            .as_array()
            .unwrap()
            .iter()
            .all(|e| e["type"] != "third-party"),
        "{body}"
    );
    let fetcher_error = json!({ "error": "The requested items cannot be fetched right now", "message": format!("Cannot fetch elements for {OWNER}") });
    let uri = format!("/lambdas/users/{OWNER}/third-party-wearables");
    let (status, body) = get(&down_state, &uri).await;
    assert_eq!(
        (status, body),
        (StatusCode::BAD_GATEWAY, fetcher_error.clone()),
        "{uri}"
    );
    // lamb2's outfits let the third-party checker's error through to its
    // bare 500.
    let uri = format!("/lambdas/outfits/{OWNER}");
    let (status, body) = get(&down_state, &uri).await;
    assert_eq!(
        (status, body),
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            json!({ "error": "Internal Server Error" })
        ),
        "{uri}"
    );
    // An active escrow grant joins the explorer's on-chain copies only where
    // `EXPLORER_ESCROW_GRANTS_ENABLED` is on (off unless the operator opts
    // in, as lamb2 lists chain-owned tokens only).
    sqlx::query("INSERT INTO shop.usage_grants (grantee_address, urn, token_id, category, unlock_at) VALUES ($1, $2, '99', 'wearable', timezone('UTC', now()) + interval '2 days')")
        .bind(OWNER)
        .bind(format!("urn:decentraland:matic:collections-v2:{COFFEE}:1"))
        .execute(&db.pool)
        .await
        .unwrap();
    for (grants, amount) in [(false, 2), (true, 3)] {
        let mut settings = state.settings.clone();
        settings.explorer_escrow_grants = grants;
        let s = Arc::new(MarketplaceState::new(
            db.pool.clone(),
            crate::http::Outbound::for_tests(),
            settings,
        ));
        s.test_allow_loopback
            .store(true, std::sync::atomic::Ordering::Relaxed);
        let (status, body) = get(
            &s,
            &format!("/explorer/{OWNER}/wearables?collectionType=on-chain"),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{body}");
        assert_eq!(
            body["elements"][0]["amount"], amount,
            "grants={grants}: {body}"
        );
    }
    sqlx::query("DELETE FROM shop.usage_grants WHERE token_id = '99'")
        .execute(&db.pool)
        .await
        .unwrap();

    // The registry and the collections subgraphs down: the third-party
    // reads answer lamb2's 502 FetcherError, never an empty list or a 503.
    let mut dark_settings = state.settings.clone();
    dark_settings.third_party_registry_subgraph_url = Some(down.clone());
    dark_settings.eth_collections_subgraph_url = Some(down.clone());
    dark_settings.matic_collections_subgraph_url = Some(down.clone());
    let dark_state = Arc::new(MarketplaceState::new(
        db.pool.clone(),
        crate::http::Outbound::for_tests(),
        dark_settings,
    ));
    dark_state
        .test_allow_loopback
        .store(true, std::sync::atomic::Ordering::Relaxed);
    for uri in [
        format!("/lambdas/users/{OWNER}/third-party-wearables"),
        format!(
            "/lambdas/users/{OWNER}/third-party-wearables/urn:decentraland:matic:collections-thirdparty:tp"
        ),
        format!(
            "/lambdas/collections/wearables-by-owner/{OWNER}?collectionId=urn:decentraland:matic:collections-thirdparty:tp"
        ),
        format!("/explorer/{OWNER}/wearables?collectionType=third-party"),
    ] {
        let (status, body) = get(&dark_state, &uri).await;
        assert_eq!(
            (status, body),
            (StatusCode::BAD_GATEWAY, fetcher_error.clone()),
            "{uri}"
        );
    }
    // Wearable and emote ownership never asks the collections subgraphs.
    for uri in [
        format!("/lambdas/users/{OWNER}/wearables"),
        format!("/lambdas/users/{OWNER}/emotes"),
        format!("/lambdas/collections/wearables-by-owner/{OWNER}"),
        format!("/lambdas/collections/emotes-by-owner/{OWNER}"),
        format!("/explorer/{OWNER}/wearables?collectionType=on-chain"),
    ] {
        let (status, body) = get(&dark_state, &uri).await;
        assert_eq!(status, StatusCode::OK, "{uri}: {body}");
    }
    let (status, body) = get(&dark_state, "/lambdas/third-party-integrations").await;
    assert_eq!(
        (status, body),
        (
            StatusCode::BAD_GATEWAY,
            json!({ "error": "The requested items cannot be fetched right now", "message": "Cannot fetch third party providers" })
        )
    );

    let (status, body) = get(&down_state, &format!("/lambdas/profiles/{OWNER}")).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        body["avatars"][0]["avatar"]["wearables"],
        json!([
            "urn:decentraland:off-chain:base-avatars:eyes_00",
            format!("{hat}:11"),
            TP_URN
        ]),
        "the v1 item needs no check: {body}"
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
    // The marketplace subgraph keeps an expired order, and one a previous
    // owner placed (status `transferred`), as the `activeOrder`; lamb2
    // answers its price (captures/lambdas-land/LL-13-*.json). A sold or
    // cancelled order is no active order.
    sqlx::query(
        "INSERT INTO shop.nft_order (network, contract_address, token_id, order_id, marketplace_address, status, seller, price, expires_at, created_at, updated_at, updated_block, updated_log) \
         VALUES ('ethereum', $1, 5, '0x02', '0x8e5660b4ab70168b5a6feea0e0315cb49c8cd539', 'open', '0xcccc000000000000000000000000000000000003', 2287, 1694476800000, 1, 1, 4, 0)",
    )
    .bind(ESTATE_REGISTRY_MAINNET)
    .execute(&db.pool)
    .await
    .unwrap();
    let (_, body) = get(&state, &format!("/lambdas/users/{OWNER}/lands")).await;
    assert_eq!(
        (
            body["elements"][0]["tokenId"].as_str(),
            body["elements"][0]["price"].as_str()
        ),
        (Some("5"), Some("2287")),
        "an expired order by a previous owner keeps its price: {body}"
    );
    for status in ["sold", "cancelled"] {
        sqlx::query("UPDATE shop.nft_order SET status = $1 WHERE token_id = 5")
            .bind(status)
            .execute(&db.pool)
            .await
            .unwrap();
        let (_, body) = get(&state, &format!("/lambdas/users/{OWNER}/lands")).await;
        assert!(
            body["elements"][0].get("price").is_none(),
            "a {status} order has no price: {body}"
        );
    }
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
/// from the marketplace subgraph, the parcel operators, the permission flags
/// and the update operator's parcels from the land-manager subgraph; without
/// `LAND_SUBGRAPH_URL` the two land routes keep the index envelope and the
/// flags route answers lamb2's 500.
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
                "description": "desc", "price": "1000", "image": "https://maps.example/v1/parcels/1/2/map.png" }),
        "the subgraph's own image, as lamb2 answers it"
    );
    // `/lands` has no pageSize maximum (lamb2's `Number.MAX_VALUE`).
    let (status, body) = get(
        &state,
        &format!("/lambdas/users/{OWNER}/lands?pageSize=5000"),
    )
    .await;
    assert_eq!(
        (
            status,
            body["pageSize"].as_i64(),
            body["elements"].as_array().map(Vec::len)
        ),
        (StatusCode::OK, Some(5000), Some(2)),
        "{body}"
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
    sqlx::query("INSERT INTO land_authz.index_cursor (network, land_registry, estate_registry, last_block, folded_block, folded_at, full_replay_started) VALUES ('ethereum-mainnet', $1, $2, 1, 1, timezone('UTC', now()), true)")
        .bind(LAND_REGISTRY_MAINNET)
        .bind(ESTATE_REGISTRY_MAINNET)
        .execute(&db.pool)
        .await
        .unwrap();
    assert_eq!(
        get_via(land.clone(), "/lambdas/parcels/0/0/operators")
            .await
            .0,
        StatusCode::NOT_FOUND,
        "the fresh local fold has no parcel"
    );
    sqlx::query("UPDATE land_authz.index_cursor SET folded_at = folded_at - interval '11 minutes'")
        .execute(&db.pool)
        .await
        .unwrap();
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
        land.clone(),
        &format!("/lambdas/users/{UPDATE_OP}/lands-permissions?pageSize=5000"),
    )
    .await;
    assert_eq!(
        (
            body["pageSize"].as_i64(),
            body["elements"].as_array().map(Vec::len)
        ),
        (Some(5000), Some(2)),
        "no pageSize maximum: {body}"
    );
    let (_, body) = get_via(
        land.clone(),
        "/lambdas/users/0xdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef/lands-permissions",
    )
    .await;
    assert_eq!(
        body,
        json!({ "elements": [], "totalAmount": 0, "pageNum": 1, "pageSize": 100 })
    );
    // The flags route reads the same operators and answers booleans, as
    // lamb2's getParcelPermissions compares them.
    for (who, expected) in [
        (
            OWNER,
            json!({ "owner": true, "operator": false, "updateOperator": false, "updateManager": false, "approvedForAll": false }),
        ),
        (
            MANAGER,
            json!({ "owner": false, "operator": false, "updateOperator": false, "updateManager": true, "approvedForAll": false }),
        ),
        (
            UPDATE_OP,
            json!({ "owner": false, "operator": false, "updateOperator": false, "updateManager": false, "approvedForAll": false }),
        ),
    ] {
        let (status, body) = get_via(
            land.clone(),
            &format!("/lambdas/users/{who}/parcels/0/0/permissions"),
        )
        .await;
        assert_eq!((status, body), (StatusCode::OK, expected), "{who}");
    }
    let (status, body) = get_via(
        land.clone(),
        &format!("/lambdas/users/{OWNER}/parcels/160/100/permissions"),
    )
    .await;
    assert_eq!(
        (status, body["message"].as_str()),
        (
            StatusCode::NOT_FOUND,
            Some("Parcel or estate rights not found for x: 160, y: 100")
        ),
        "an exception-block parcel reaches the subgraph"
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
    let (status, body) = get_via(
        land.clone(),
        &format!("/lambdas/users/{OWNER}/parcels/0/0/permissions"),
    )
    .await;
    assert_eq!(
        (status, body),
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            json!({ "error": "Internal Server Error" })
        ),
        "neither source: lamb2's bare 500"
    );

    db.drop().await;
}

#[tokio::test]
async fn profile_routes_answer_as_lamb2_when_content_is_unavailable() {
    let state = crate::market::tests::support::state_without_db();
    state.pool.close().await;
    assert_lambdas_profile_dependency_failure(&state, "entities").await;
}

#[tokio::test]
async fn profile_routes_answer_as_lamb2_when_ownership_is_unavailable() {
    let Some(db) = ScratchDb::create().await else {
        return;
    };
    deploy(
        &db.pool,
        "bafyownershipfailureprofile",
        "profile",
        OWNER,
        json!({ "avatars": [{ "name": "Test", "avatar": { "wearables": [] } }] }),
        &[],
    )
    .await;
    let state = Arc::new(MarketplaceState::new(
        db.pool.clone(),
        crate::http::Outbound::for_tests(),
        Settings {
            marketplace_subgraph_url: Some(serve_failing_graph().await),
            ..Settings::for_tests()
        },
    ));
    state
        .test_allow_loopback
        .store(true, std::sync::atomic::Ordering::Relaxed);
    assert_lambdas_profile_dependency_failure(&state, "ownership").await;
    db.drop().await;
}

async fn assert_lambdas_profile_dependency_failure(state: &SharedState, layer: &str) {
    let recorder = crate::comms::metrics::prometheus_builder()
        .unwrap()
        .build_recorder();
    let _recording = dcl_metrics::set_default_local_recorder(&recorder);
    let app = router(state.clone());
    for modified_since in [None, Some("Wed, 21 Oct 2015 07:28:00 GMT")] {
        let mut request = Request::post("/lambdas/profiles");
        if let Some(date) = modified_since {
            request = request.header("if-modified-since", date);
        }
        let resp = app
            .clone()
            .oneshot(
                request
                    .body(Body::from(json!({ "ids": [OWNER] }).to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        assert_eq!(
            axum::body::to_bytes(resp.into_body(), usize::MAX)
                .await
                .unwrap(),
            "[]"
        );
    }
    assert_eq!(
        get(state, &format!("/lambdas/profiles/{OWNER}")).await,
        (
            StatusCode::NOT_FOUND,
            json!({ "error": "Not Found", "message": "Profile not found" })
        )
    );
    assert_eq!(
        get(state, &format!("/lambdas/profile/{OWNER}")).await,
        (StatusCode::OK, json!({ "avatars": [], "timestamp": 0 }))
    );
    let rendered = recorder.handle().render();
    for (route, count) in [
        ("lambdas_profiles", 2),
        ("lambdas_profile_by_id", 1),
        ("lambdas_profile_alias", 1),
    ] {
        assert!(
            rendered
                .lines()
                .any(|line| line.starts_with("profiles_lookup_failures_total{")
                    && line.contains(&format!("route=\"{route}\""))
                    && line.contains(&format!("layer=\"{layer}\""))
                    && line.ends_with(&format!(" {count}"))),
            "{rendered}"
        );
    }
}

#[tokio::test]
async fn catalogue_and_user_pages_fail_when_the_content_read_fails() {
    // A node that walks the nft index: the user pages read their owner's
    // copies from the same closed pool first, and answer the same 500.
    let state = crate::market::tests::support::state_without_db_settings(
        super::owned_items::test_support::walking(Settings {
            eth_collections_subgraph_url: Some(serve_graph(eth_collections).await),
            matic_collections_subgraph_url: Some(serve_graph(matic_items).await),
            ..Settings::for_tests()
        }),
    );
    state.pool.close().await;
    state
        .test_allow_loopback
        .store(true, std::sync::atomic::Ordering::Relaxed);
    for path in [
        "/lambdas/collections/wearables?textSearch=hat".to_string(),
        "/lambdas/collections/emotes?textSearch=dance".to_string(),
        format!("/lambdas/users/{OWNER}/wearables?includeDefinitions"),
        format!("/lambdas/users/{OWNER}/wearables?includeEntities"),
        format!("/lambdas/collections/wearables-by-owner/{OWNER}?includeDefinitions"),
    ] {
        assert_eq!(
            get(&state, &path).await,
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                json!({ "error": "Internal Server Error" })
            ),
            "{path}"
        );
    }
}

#[tokio::test]
async fn user_item_entities_and_definitions_look_up_missing_pointers_on_peers() {
    let Some(pool) = crate::db::scratch::pool(4).await else {
        return;
    };
    let (asked, requests) = tokio::sync::mpsc::unbounded_channel();
    let peer = serve(Router::new().route(
        "/content/entities/active",
        axum::routing::post(move |axum::Json(body): axum::Json<Value>| {
            asked.send(body).unwrap();
            async { axum::Json(json!([])) }
        }),
    ))
    .await;
    let root = std::env::temp_dir().join(format!("catalyst-catalog-peer-{}", uuid::Uuid::new_v4()));
    let storage = Arc::new(crate::storage::Storage::from_config(&root).await.unwrap());
    let eth = serve_graph(eth_collections).await;
    let matic = serve_graph(matic_collections).await;
    let mut requests = requests;
    // The owner's copies come from the nft index (hat tokens 10 and 11, the
    // orphan 12) and their items from the mirror; no entity is deployed here.
    seed_mirrors(&pool).await;
    seed_indexes(&pool).await;

    for flag in ["includeDefinitions", "includeEntities"] {
        // A separate read-through allows both flags to be tested before a
        // negative result from the other request can enter the cache.
        let read_through = Arc::new(crate::content::read_through::ReadThrough::new(
            pool.clone(),
            storage.clone(),
            crate::content::sync::test_sync_client(reqwest::Client::new()),
            vec![format!("{peer}content")],
            None,
            crate::content::read_through::Fills::new(1, std::time::Duration::from_secs(5)),
            crate::content::sync_deployer::SyncValidation::AuthChainOnly,
        ));
        let state = Arc::new(
            MarketplaceState::new(
                pool.clone(),
                crate::http::Outbound::for_tests(),
                super::owned_items::test_support::walking(Settings {
                    eth_collections_subgraph_url: Some(eth.clone()),
                    matic_collections_subgraph_url: Some(matic.clone()),
                    ..Settings::for_tests()
                }),
            )
            .with_read_through(Some(read_through)),
        );
        state
            .test_allow_loopback
            .store(true, std::sync::atomic::Ordering::Relaxed);

        let (status, body) = get(&state, &format!("/lambdas/users/{OWNER}/wearables?{flag}")).await;
        assert_eq!(status, StatusCode::OK, "{flag}: {body}");
        let mut pointers = requests.try_recv().unwrap()["pointers"]
            .as_array()
            .unwrap()
            .clone();
        pointers.sort_by(|a, b| a.as_str().cmp(&b.as_str()));
        assert_eq!(
            pointers,
            vec![
                json!(format!("urn:decentraland:matic:collections-v2:{COFFEE}:1")),
                json!(format!("urn:decentraland:matic:collections-v2:{COFFEE}:9")),
            ]
        );
    }
    std::fs::remove_dir_all(root).unwrap();
}

/// A configured subgraph that fails: the element routes answer lamb2's 502
/// `FetcherError` naming the key as requested, `/operators` and
/// `/permissions` lamb2's bare 500 (`errorHandler.ts`). No database: the
/// index cursors read as not ready, so every route takes its subgraph leg.
#[tokio::test]
async fn a_failing_subgraph_answers_lamb2s_status() {
    let pool = PgPool::connect_lazy("postgres://postgres@127.0.0.1:1/postgres").unwrap();
    pool.close().await;
    let failing = serve_failing_graph().await;
    let state = Arc::new(MarketplaceState::new(
        pool.clone(),
        crate::http::Outbound::for_tests(),
        Settings {
            marketplace_subgraph_url: Some(failing.clone()),
            land_subgraph_url: Some(failing),
            ..Settings::for_tests()
        },
    ));
    state
        .test_allow_loopback
        .store(true, std::sync::atomic::Ordering::Relaxed);
    let mixed = "0xAaaa000000000000000000000000000000000001";
    let fetcher = |key: &str| {
        json!({
            "error": "The requested items cannot be fetched right now",
            "message": format!("Cannot fetch elements for {key}"),
        })
    };
    for (uri, key) in [
        (format!("/lambdas/users/{mixed}/names"), mixed),
        (format!("/lambdas/users/{mixed}/lands"), mixed),
        ("/lambdas/names/%20Alice.dcl.eth/owner".to_string(), "alice"),
    ] {
        let (status, body) = get(&state, &uri).await;
        assert_eq!(
            (status, body),
            (StatusCode::BAD_GATEWAY, fetcher(key)),
            "{uri}"
        );
    }
    // A names pageSize over 1000 is lamb2's 400, before any source is read.
    let (status, body) = get(
        &state,
        &format!("/lambdas/users/{OWNER}/names?pageSize=1001"),
    )
    .await;
    assert_eq!(
        (status, body),
        (
            StatusCode::BAD_REQUEST,
            json!({ "error": "Bad request", "message": "max allowed pageSize is 1000" })
        )
    );

    let land = crate::land_authz::router(Arc::new(crate::land_authz::LandAuthzState::new(
        pool,
        crate::http::Outbound::for_tests(),
        crate::land_authz::Settings::for_tests(),
        Some(state.index.clone()),
    )));
    let (status, body) = get_via(
        land.clone(),
        &format!("/lambdas/users/{mixed}/lands-permissions"),
    )
    .await;
    assert_eq!((status, body), (StatusCode::BAD_GATEWAY, fetcher(mixed)));
    for method in [Method::GET, Method::OPTIONS] {
        let response = land
            .clone()
            .oneshot(
                Request::builder()
                    .method(method)
                    .uri("/lambdas/parcels/0/0/operators")
                    .header(header::ORIGIN, "https://example.org")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.headers()[header::ACCESS_CONTROL_ALLOW_ORIGIN], "*");
        assert!(
            !response
                .headers()
                .contains_key(header::ACCESS_CONTROL_ALLOW_CREDENTIALS)
        );
    }
    for uri in [
        "/lambdas/parcels/0/0/operators".to_string(),
        format!("/lambdas/users/{OWNER}/parcels/0/0/permissions"),
    ] {
        let (status, body) = get_via(land.clone(), &uri).await;
        assert_eq!(
            (status, body),
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                json!({ "error": "Internal Server Error" })
            ),
            "{uri}"
        );
    }
}

/// A LAND read that checked the land index before its cursor was rebound,
/// then stalled while the new registries folded their links, reads its
/// estates' parcels from its snapshot when it resumes: an estate keeps its
/// own parcels, never the ones another registry's estate of the same id
/// links. A new request after the rebind answers the index envelope.
#[tokio::test]
async fn estate_parcels_read_on_the_land_snapshot_never_join_a_rebound_registry() {
    let Some(db) = ScratchDb::create().await else {
        return;
    };
    seed_indexes(&db.pool).await;
    let state = Arc::new(MarketplaceState::new(
        db.pool.clone(),
        crate::http::Outbound::for_tests(),
        Settings::for_tests(),
    ));
    let estate_5 = || vec!["5".to_string()];
    let coordinates =
        |parcels: std::collections::HashMap<String, Vec<super::nfts::EstateParcel>>| {
            parcels
                .get("5")
                .map(|ps| ps.iter().map(|p| (p.x, p.y)).collect::<Vec<_>>())
                .unwrap_or_default()
        };

    // The read checks the seeded index (estate 5 holds LAND 2) and stalls.
    let mut held = super::nft_reads::land_snapshot(&db.pool, crate::land_authz::LAND_MAINNET)
        .await
        .expect("the seeded land index is published");

    // Another node rebinds the mainnet cursor to other registries, and their
    // fold links LAND 9 into their own estate 5.
    let other = crate::land_authz::Settings {
        land_registry: "0x00000000000000000000000000000000000000aa".into(),
        estate_registry: "0x00000000000000000000000000000000000000bb".into(),
        ..crate::land_authz::Settings::for_tests()
    };
    assert_eq!(
        crate::land_authz::indexer::begin_replay(&db.pool, &other, other.start_block)
            .await
            .unwrap(),
        crate::land_authz::indexer::ReplayStart::Begun
    );
    sqlx::query(
        "INSERT INTO land_authz.authz_event (block_number, log_index, block_time, token_address, kind, token_id, estate_id) \
         VALUES (20, 0, 1600000020, $1, 'add_land', 9, 5)",
    )
    .bind(&other.estate_registry)
    .execute(&db.pool)
    .await
    .unwrap();
    crate::land_authz::indexer::fold(&db.pool, &other, None)
        .await
        .unwrap();

    // Control: outside a snapshot, the shop's estate 5 now lists the other
    // registry's LAND 9.
    let mut conn = db.pool.acquire().await.unwrap();
    let outside = super::nft_reads::estate_parcels(&mut conn, &estate_5())
        .await
        .unwrap();
    assert_eq!(coordinates(outside), vec![(0, 9)]);
    drop(conn);

    // The stalled read resumes on its snapshot: estate 5 still holds LAND 2.
    let resumed = super::nft_reads::estate_parcels(&mut held.tx, &estate_5())
        .await
        .unwrap();
    assert_eq!(coordinates(resumed), vec![(0, 2)]);
    drop(held);

    let (status, body) = get(&state, &format!("/lambdas/users/{OWNER}/genesis-parcels")).await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{body}");
    assert_eq!(body["source"], "land-authz-index", "{body}");
    db.drop().await;
}

/// `/lambdas/users/{a}/lands` holds one connection, its land snapshot, and
/// checks the nft index on it: with a pool of one connection and both
/// indexes published, it answers the page at once, instead of waiting on a
/// second connection and reading the timeout as an unpublished index.
#[tokio::test]
async fn user_lands_answer_on_a_pool_of_one_connection() {
    let Some(db) = ScratchDb::create().await else {
        return;
    };
    seed_indexes(&db.pool).await;
    let one = PgPoolOptions::new()
        .max_connections(1)
        .acquire_timeout(std::time::Duration::from_secs(2))
        .connect_with((*db.pool.connect_options()).clone())
        .await
        .unwrap();
    let state = Arc::new(MarketplaceState::new(
        one.clone(),
        crate::http::Outbound::for_tests(),
        Settings::for_tests(),
    ));
    let (status, body) = tokio::time::timeout(
        std::time::Duration::from_secs(10),
        get(&state, &format!("/lambdas/users/{OWNER}/lands")),
    )
    .await
    .expect("the route answers well within the pool's acquire timeout");
    assert_eq!(status, StatusCode::OK, "{body}");
    let tokens: Vec<&str> = body["elements"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|e| e["tokenId"].as_str())
        .collect();
    assert_eq!(
        (body["totalAmount"].as_i64(), tokens),
        (Some(2), vec!["5", "1"])
    );
    one.close().await;
    db.drop().await;
}

/// LL-15: `/lands`, `/genesis-parcels` and the marketplace `/v1/nfts`
/// read the index of the registries `ETH_NETWORK` names. A Sepolia node answers from its Sepolia index; a
/// mainnet node over the same database finds no mainnet cursor and does not
/// read the Sepolia rows.
#[tokio::test]
async fn user_lands_read_the_registries_of_the_configured_network() {
    let Some(db) = ScratchDb::create().await else {
        return;
    };
    use crate::land_authz::{ESTATE_REGISTRY_SEPOLIA, LAND_REGISTRY_SEPOLIA};
    let now = chrono::Utc::now().naive_utc();
    sqlx::query(
        "INSERT INTO land_authz.index_cursor (network, land_registry, estate_registry, last_block, folded_block, folded_at) \
         VALUES ('ethereum-sepolia', $1, $2, 10, 10, $3)",
    )
    .bind(LAND_REGISTRY_SEPOLIA)
    .bind(ESTATE_REGISTRY_SEPOLIA)
    .bind(now)
    .execute(&db.pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO shop.chain_cursor (indexer, network, start_block, last_block, folded_block, folded_at, full_history, chain_id) \
         VALUES ('nft', 'ethereum', 0, 10, 10, $1, true, 1)",
    )
    .bind(now)
    .execute(&db.pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO land_authz.token_right (token_address, token_id, x, y, owner, updated_block, updated_log) \
         VALUES ($1, 7::numeric, 10, -3, $2, 5, 0)",
    )
    .bind(LAND_REGISTRY_SEPOLIA)
    .bind(OWNER)
    .execute(&db.pool)
    .await
    .unwrap();

    let state_for = |network: &str| {
        Arc::new(MarketplaceState::new(
            db.pool.clone(),
            crate::http::Outbound::for_tests(),
            Settings {
                eth_network: network.to_string(),
                ..Settings::for_tests()
            },
        ))
    };
    let sepolia = state_for("sepolia");
    let (status, body) = get(&sepolia, &format!("/lambdas/users/{OWNER}/lands")).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        (
            body["totalAmount"].as_i64(),
            body["elements"][0]["contractAddress"].as_str(),
            body["elements"][0]["tokenId"].as_str()
        ),
        (Some(1), Some(LAND_REGISTRY_SEPOLIA), Some("7")),
        "{body}"
    );
    let (status, body) = get(&sepolia, &format!("/lambdas/users/{OWNER}/genesis-parcels")).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["totalAmount"].as_i64(), Some(1), "{body}");

    let mainnet = state_for("mainnet");
    let (status, body) = get(&mainnet, &format!("/lambdas/users/{OWNER}/lands")).await;
    assert_eq!(
        (status, body["source"].as_str()),
        (StatusCode::SERVICE_UNAVAILABLE, Some("land-authz-index")),
        "no mainnet cursor: {body}"
    );

    // The marketplace LAND search (the index path of `/v1/nfts`) reads the
    // same registries.
    let (status, body) = get_via(
        super::market_router(sepolia.clone()),
        "/v1/nfts?category=parcel&search=10,-3",
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        (
            body["total"].as_i64(),
            body["data"][0]["nft"]["contractAddress"].as_str(),
            body["data"][0]["nft"]["tokenId"].as_str()
        ),
        (Some(1), Some(LAND_REGISTRY_SEPOLIA), Some("7")),
        "{body}"
    );
    let (status, body) = get_via(
        super::market_router(mainnet.clone()),
        "/v1/nfts?category=parcel&search=10,-3",
    )
    .await;
    assert_eq!(
        (status, body["source"].as_str()),
        (StatusCode::SERVICE_UNAVAILABLE, Some("land-authz-index")),
        "no mainnet cursor: {body}"
    );
    db.drop().await;
}

/// LL-24: on a subgraph outage `/users/{a}/names` answers the last good list
/// for lamb2's 120 s and `/users/{a}/lands` for as long as the outage lasts,
/// while `graph::owned_names`, the read worlds' NAME owner and comms
/// admission take, keeps failing: an ownership decision is never stale.
#[tokio::test]
async fn names_and_lands_serve_stale_on_outage_and_ownership_does_not() {
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::time::Duration;

    use axum::routing::post;

    let pool = PgPool::connect_lazy("postgres://postgres@127.0.0.1:1/postgres").unwrap();
    pool.close().await;
    let down = Arc::new(AtomicBool::new(false));
    let graph = serve(Router::new().route(
        "/",
        post({
            let down = down.clone();
            move |axum::Json(body): axum::Json<Value>| async move {
                if down.load(Ordering::Relaxed) {
                    return axum::Json(json!({ "errors": [{ "message": "indexer down" }] }));
                }
                let query = body["query"].as_str().unwrap_or("");
                let vars = body.get("variables").cloned().unwrap_or(Value::Null);
                axum::Json(json!({ "data": marketplace_reader(query, &vars) }))
            }
        }),
    ))
    .await;
    let state = Arc::new(MarketplaceState::new(
        pool,
        crate::http::Outbound::for_tests(),
        Settings {
            marketplace_subgraph_url: Some(graph),
            ..Settings::for_tests()
        },
    ));
    state
        .test_allow_loopback
        .store(true, std::sync::atomic::Ordering::Relaxed);
    let names_uri = format!("/lambdas/users/{OWNER}/names");
    let lands_uri = format!("/lambdas/users/{OWNER}/lands");
    let (status, names) = get(&state, &names_uri).await;
    assert_eq!(status, StatusCode::OK, "{names}");
    assert_eq!(names["totalAmount"], 1, "{names}");
    let (status, lands) = get(&state, &lands_uri).await;
    assert_eq!(status, StatusCode::OK, "{lands}");
    assert_eq!(lands["totalAmount"], 2, "{lands}");

    // The subgraph goes down and the 30 s entries expire.
    down.store(true, Ordering::Relaxed);
    state
        .index
        .graph
        .backdate_owner_for_tests(OWNER, Duration::from_secs(31));
    assert_eq!(get(&state, &names_uri).await, (StatusCode::OK, names));
    assert_eq!(
        get(&state, &lands_uri).await,
        (StatusCode::OK, lands.clone())
    );
    assert!(
        crate::chain_index::graph::owned_names(&state.index, OWNER)
            .await
            .is_err(),
        "the ownership read fails on the outage, never stale"
    );

    // Past lamb2's 120 s the names answer the 502; the lands still answer.
    state
        .index
        .graph
        .backdate_owner_for_tests(OWNER, Duration::from_secs(3600));
    let (status, body) = get(&state, &names_uri).await;
    assert_eq!(status, StatusCode::BAD_GATEWAY, "{body}");
    assert_eq!(get(&state, &lands_uri).await, (StatusCode::OK, lands));
}

/// The nft leg of `/v1/prices` reads after its gate; if the land index
/// stops being ready in between, the route answers the land index's
/// envelope, as `/v1/nfts` does, and any other failure stays internal.
#[tokio::test]
async fn nft_prices_answer_the_land_index_envelope_when_it_stops_being_ready_after_the_gate() {
    let Some(db) = ScratchDb::create().await else {
        return;
    };
    seed_indexes(&db.pool).await;
    let state = Arc::new(MarketplaceState::new(
        db.pool.clone(),
        crate::http::Outbound::for_tests(),
        Settings::for_tests(),
    ));
    let parcels = super::nfts::NftFilters {
        category: Some(super::wire::NftCategory::Parcel),
        ..Default::default()
    };
    let read = super::nft_reads::nft_prices(&state, &parcels).await;
    assert!(read.is_ok(), "{read:?}");

    // The mainnet cursor is rebound to other registries.
    sqlx::query(
        "UPDATE land_authz.index_cursor SET land_registry = '0x00000000000000000000000000000000000000aa' \
         WHERE network = 'ethereum-mainnet'",
    )
    .execute(&db.pool)
    .await
    .unwrap();
    let refused = super::nft_reads::nft_prices(&state, &parcels)
        .await
        .unwrap_err();
    assert_eq!(refused, super::nft_reads::NFT_PRICES_LAND_UNAVAILABLE);
    let answer = |e: String| async move {
        let response = super::prices::nft_prices_error(e);
        let status = response.status();
        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        (status, serde_json::from_slice::<Value>(&bytes).unwrap())
    };
    let (status, body) = answer(refused).await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{body}");
    assert_eq!(body["source"], "land-authz-index", "{body}");
    let (status, body) = answer("statement timeout".into()).await;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR, "{body}");
    db.drop().await;
}

/// The nft leg of `/v1/prices` keeps the 503s `/v1/nfts` answers through
/// the prices memo: an nft index withdrawn between the gate and the read
/// answers its envelope, and a statement the read bound cancelled the
/// bound's 503, not a 500.
#[tokio::test]
async fn nft_prices_keep_the_index_envelope_and_the_read_bound() {
    let Some(db) = ScratchDb::create().await else {
        return;
    };
    seed_mirrors(&db.pool).await;
    seed_indexes(&db.pool).await;
    let state = Arc::new(MarketplaceState::new(
        db.pool.clone(),
        crate::http::Outbound::for_tests(),
        Settings::for_tests(),
    ));
    let wearables = super::nfts::NftFilters {
        category: Some(super::wire::NftCategory::Wearable),
        ..Default::default()
    };
    let read = super::nft_reads::nft_prices(&state, &wearables).await;
    assert!(read.is_ok(), "{read:?}");
    let answer = |e: String| async move {
        let response = super::prices::nft_prices_error(e);
        let status = response.status();
        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        (status, serde_json::from_slice::<Value>(&bytes).unwrap())
    };

    // A rewind withdraws the Polygon publication after the gate passed.
    set_nft_cursor(&db.pool, "matic", None).await;
    let refused = super::nft_reads::nft_prices(&state, &wearables)
        .await
        .unwrap_err();
    let (status, body) = answer(refused).await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{body}");
    assert_eq!(body["source"], "nft-index", "{body}");
    assert!(
        body["message"]
            .as_str()
            .is_some_and(|m| m.contains("matic")),
        "{body}"
    );

    let (status, body) = answer(super::nft_reads::NFT_PRICES_TIMED_OUT.to_string()).await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{body}");
    assert!(
        body["message"]
            .as_str()
            .is_some_and(|m| m.contains("statement bound")),
        "{body}"
    );
    let (status, body) = answer("another failure".into()).await;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR, "{body}");
    db.drop().await;
}

/// A collections subgraph that counts every request: the ownership routes
/// must never call it.
async fn serve_counting_graph() -> (String, Arc<std::sync::atomic::AtomicUsize>) {
    let calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let counted = calls.clone();
    let url = serve(Router::new().route(
        "/",
        axum::routing::post(move || {
            let counted = counted.clone();
            async move {
                counted.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                axum::Json(json!({ "data": { "nfts": [] } }))
            }
        }),
    ))
    .await;
    (url, calls)
}

/// The profile, inventory and outfits fixture of the ownership tests: the
/// owner's profile wears the hat (token 10) and the dance (token 20), both
/// in the nft index, with a node that does not walk it (`off`) and one that
/// does (`walking`), and a collections subgraph that counts its requests.
struct OwnershipFixture {
    db: ScratchDb,
    off: Arc<MarketplaceState>,
    walking: Arc<MarketplaceState>,
    calls: Arc<std::sync::atomic::AtomicUsize>,
    hat: String,
}

async fn ownership_fixture() -> Option<OwnershipFixture> {
    let db = ScratchDb::create().await?;
    seed(&db.pool).await;
    let hat = format!("urn:decentraland:matic:collections-v2:{COFFEE}:1");
    let dance = format!("urn:decentraland:matic:collections-v2:{COFFEE}:2");
    deploy(
        &db.pool,
        "bafyprofile",
        "profile",
        OWNER,
        json!({ "avatars": [{ "name": "alice", "avatar": { "bodyShape": "urn:decentraland:off-chain:base-avatars:BaseFemale",
            "wearables": ["urn:decentraland:off-chain:base-avatars:eyes_00", hat],
            "emotes": [{ "slot": 0, "urn": dance }] } }] }),
        &[],
    )
    .await;
    for (token, item) in [(10i64, 1i64), (20, 2)] {
        sqlx::query(
            "INSERT INTO shop.nft (network, contract_address, token_id, owner, item_id, issued_id, minted_block, minted_at, transferred_block, transferred_log, transferred_at) \
             VALUES ('matic', $1, $2, $3, $4, 1, 5, 5, 5, 0, 5)",
        )
        .bind(COFFEE)
        .bind(token)
        .bind(OWNER)
        .bind(item)
        .execute(&db.pool)
        .await
        .unwrap();
    }
    let (graph, calls) = serve_counting_graph().await;
    let rpc = serve_rpc().await;
    let settings = Settings {
        eth_collections_subgraph_url: Some(graph.clone()),
        matic_collections_subgraph_url: Some(graph),
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
    let state_of = |settings: Settings| {
        let state = Arc::new(MarketplaceState::new(
            db.pool.clone(),
            crate::http::Outbound::for_tests(),
            settings,
        ));
        state
            .test_allow_loopback
            .store(true, std::sync::atomic::Ordering::Relaxed);
        state
    };
    let off = state_of(settings.clone());
    let walking = state_of(super::owned_items::test_support::walking(settings));
    Some(OwnershipFixture {
        db,
        off,
        walking,
        calls,
        hat,
    })
}

/// Publishes the nft cursor of `network` `published` minutes ago, or
/// withdraws it (`None`), as a pass and a rewind do.
async fn set_nft_cursor(pool: &PgPool, network: &str, published: Option<i64>) {
    sqlx::query(
        "INSERT INTO shop.chain_cursor (indexer, network, start_block, last_block, folded_block, folded_at, full_history, chain_id) \
         VALUES ('nft', $1, 1, 100, CASE WHEN $2::bigint IS NULL THEN NULL ELSE 100 END, \
                 timezone('UTC', now()) - make_interval(mins => $2::int), true, \
                 CASE $1 WHEN 'ethereum' THEN 1 ELSE 137 END) \
         ON CONFLICT (indexer, network) DO UPDATE SET folded_block = EXCLUDED.folded_block, folded_at = EXCLUDED.folded_at",
    )
    .bind(network)
    .bind(published)
    .execute(pool)
    .await
    .unwrap();
}

/// A walking node reads a profile's `hasClaimedName` from the Ethereum
/// index, in the items' snapshot, and files the profile under that
/// publication: once a pass publishes the NAME's transfer, the cached
/// profile is not served again. The marketplace subgraph (whose fake still
/// lists the NAME) is never the source.
#[tokio::test]
async fn a_claimed_name_follows_the_ethereum_publication() {
    let Some(OwnershipFixture { db, walking, .. }) = ownership_fixture().await else {
        return;
    };
    // The deployed avatar's name, as minted.
    sqlx::query(
        "INSERT INTO shop.nft (network, contract_address, token_id, owner, ens_subdomain, minted_block, minted_at, transferred_block, transferred_log, transferred_at) \
         VALUES ('ethereum', $1, 77, $2, 'alice', 1, 1, 1, 0, 1)",
    )
    .bind(super::nft_index::DCL_REGISTRAR)
    .bind(OWNER)
    .execute(&db.pool)
    .await
    .unwrap();
    set_nft_cursor(&db.pool, "ethereum", Some(0)).await;
    set_nft_cursor(&db.pool, "matic", Some(0)).await;
    let claimed = || async {
        let (status, body) = get(&walking, &format!("/lambdas/profiles/{OWNER}")).await;
        assert_eq!(status, StatusCode::OK, "{body}");
        body["avatars"][0]["hasClaimedName"].clone()
    };
    assert_eq!(claimed().await, json!(true));

    sqlx::query("UPDATE shop.nft SET owner = '0xbbbb000000000000000000000000000000000002' WHERE contract_address = $1 AND token_id = 77")
        .bind(super::nft_index::DCL_REGISTRAR)
        .execute(&db.pool)
        .await
        .unwrap();
    sqlx::query(
        "UPDATE shop.chain_cursor SET last_block = 101, folded_block = 101, folded_at = timezone('UTC', now()) \
         WHERE indexer = 'nft' AND network = 'ethereum'",
    )
    .execute(&db.pool)
    .await
    .unwrap();
    assert_eq!(
        claimed().await,
        json!(false),
        "the profile filed under the former publication is not served"
    );
    db.drop().await;
}

/// Every ownership route against each state of the nft index: switched off
/// (profiles and outfits as deployed, the rest the envelope), walked but
/// incomplete (everything fails closed: a direct profile read 503s and a
/// Registry batch is an error the batch leaves out), complete but published
/// an hour ago (profiles serve, the strict routes answer the envelope), only
/// Polygon published (the emotes serve; the wearables need Ethereum too, and
/// so do a profile and its outfits, whose DCL NAMEs the Ethereum index
/// holds), and published. No route asks the collections subgraphs.
#[tokio::test]
async fn each_ownership_route_follows_the_nft_index_state() {
    let Some(OwnershipFixture {
        db,
        off,
        walking,
        calls,
        hat,
    }) = ownership_fixture().await
    else {
        return;
    };
    let dance = format!("urn:decentraland:matic:collections-v2:{COFFEE}:2");
    let set_cursor = |network: &'static str, published: Option<i64>| {
        let pool = db.pool.clone();
        async move { set_nft_cursor(&pool, network, published).await }
    };
    let entity = || async {
        super::profiles::active_entities(&walking, &[OWNER.to_string()])
            .await
            .unwrap()
    };
    struct Expect {
        profile: Option<Value>,
        wearables: StatusCode,
        emotes: StatusCode,
        outfits: StatusCode,
        nfts: StatusCode,
        nfts_matic: StatusCode,
    }
    let unavailable = StatusCode::SERVICE_UNAVAILABLE;
    let ok = StatusCode::OK;
    let owned_profile = json!([
        "urn:decentraland:off-chain:base-avatars:eyes_00",
        format!("{hat}:10")
    ]);
    for (label, state, eth, matic, expect) in [
        (
            "switched off",
            &off,
            None,
            None,
            Expect {
                profile: Some(json!([
                    "urn:decentraland:off-chain:base-avatars:eyes_00",
                    hat.clone()
                ])),
                wearables: unavailable,
                emotes: unavailable,
                outfits: ok,
                nfts: unavailable,
                nfts_matic: unavailable,
            },
        ),
        (
            "incomplete",
            &walking,
            Some(None),
            Some(None),
            Expect {
                profile: None,
                wearables: unavailable,
                emotes: unavailable,
                outfits: unavailable,
                nfts: unavailable,
                nfts_matic: unavailable,
            },
        ),
        (
            "complete, published an hour ago",
            &walking,
            Some(Some(60)),
            Some(Some(60)),
            Expect {
                profile: Some(owned_profile.clone()),
                wearables: unavailable,
                emotes: unavailable,
                outfits: unavailable,
                nfts: unavailable,
                nfts_matic: unavailable,
            },
        ),
        (
            "only Polygon published",
            &walking,
            Some(None),
            Some(Some(0)),
            Expect {
                profile: None,
                wearables: unavailable,
                emotes: ok,
                outfits: unavailable,
                nfts: unavailable,
                nfts_matic: ok,
            },
        ),
        (
            "published",
            &walking,
            Some(Some(0)),
            Some(Some(0)),
            Expect {
                profile: Some(owned_profile.clone()),
                wearables: ok,
                emotes: ok,
                outfits: ok,
                nfts: ok,
                nfts_matic: ok,
            },
        ),
    ] {
        for (network, published) in [("ethereum", eth), ("matic", matic)] {
            if let Some(published) = published {
                set_cursor(network, published).await;
            }
        }
        state.profile_cache.clear();
        let (status, body) = get(state, &format!("/lambdas/profiles/{OWNER}")).await;
        let batch = super::profiles::process_profiles_positional(state, &entity().await).await;
        match &expect.profile {
            Some(wearables) => {
                assert_eq!(status, ok, "{label}: {body}");
                assert_eq!(
                    &body["avatars"][0]["avatar"]["wearables"], wearables,
                    "{label}: {body}"
                );
                let batch = batch.unwrap();
                assert_eq!(
                    &batch[0].as_ref().unwrap()["avatars"][0]["avatar"]["wearables"],
                    wearables,
                    "{label}: the Registry batch"
                );
            }
            None => {
                // A failed lookup answers lamb2's 404, never the profile
                // stripped of the items the index cannot vouch for.
                assert_eq!(
                    (status, body["message"].as_str()),
                    (StatusCode::NOT_FOUND, Some("Profile not found")),
                    "{label}: a direct read fails closed: {body}"
                );
                assert!(
                    batch.is_err(),
                    "{label}: the Registry batch leaves the profile out"
                );
            }
        }
        for (uri, want) in [
            (
                format!("/lambdas/users/{OWNER}/wearables"),
                expect.wearables,
            ),
            (
                format!("/lambdas/collections/wearables-by-owner/{OWNER}"),
                expect.wearables,
            ),
            (
                format!("/explorer/{OWNER}/wearables?collectionType=on-chain"),
                expect.wearables,
            ),
            (format!("/lambdas/users/{OWNER}/emotes"), expect.emotes),
            (format!("/lambdas/outfits/{OWNER}"), expect.outfits),
        ] {
            let (status, body) = get(state, &uri).await;
            assert_eq!(status, want, "{label}: {uri}: {body}");
            if status == unavailable {
                assert_eq!(body["source"], "nft-index", "{label}: {uri}: {body}");
            }
        }
        for (uri, want) in [
            (format!("/v1/users/{OWNER}/wearables"), expect.wearables),
            (
                format!("/v1/nfts?owner={OWNER}&category=wearable"),
                expect.nfts,
            ),
            (
                format!("/v1/nfts?owner={OWNER}&category=wearable&network=MATIC"),
                expect.nfts_matic,
            ),
        ] {
            let (status, body) = get_via(super::market_router(state.clone()), &uri).await;
            assert_eq!(status, want, "{label}: {uri}: {body}");
            if status == unavailable {
                assert_eq!(body["source"], "nft-index", "{label}: {uri}: {body}");
            }
        }
        // The open listings' leg never fails the order book on the index:
        // it is empty until the networks it reads are published, and
        // unguarded on a node that does not walk the index.
        for network in [None, Some(super::wire::Network::Matic)] {
            let f = super::orders::OrderFilters {
                status: Some("open".into()),
                network,
                ..Default::default()
            };
            assert!(
                super::orders::trades_leg(state, &f).await.is_ok(),
                "{label}: the orders leg on {network:?}"
            );
        }
    }
    let (_, body) = get(&walking, &format!("/lambdas/users/{OWNER}/wearables")).await;
    assert_eq!(body["elements"][0]["urn"], hat, "{body}");
    let (_, body) = get_via(
        super::market_router(walking.clone()),
        &format!("/v1/nfts?owner={OWNER}&category=emote"),
    )
    .await;
    assert_eq!(body["data"][0]["nft"]["urn"], dance, "{body}");
    assert_eq!(
        calls.load(std::sync::atomic::Ordering::SeqCst),
        0,
        "an ownership route asked the collections subgraph"
    );
    db.drop().await;
}

/// The memos of every ownership answer follow the nft index's publication,
/// with no cache cleared in between: a warm profile, inventory list,
/// backpack page and outfits entity stop serving when a rewind withdraws
/// the publication (the reads fail closed), and once the refill publishes
/// the hat's transfer to another address, none of the old answers comes
/// back.
#[tokio::test]
async fn ownership_memos_follow_the_index_publication() {
    let Some(OwnershipFixture {
        db, walking, hat, ..
    }) = ownership_fixture().await
    else {
        return;
    };
    let state = &walking;
    let ok = StatusCode::OK;
    for network in ["ethereum", "matic"] {
        set_nft_cursor(&db.pool, network, Some(2)).await;
    }
    let profile = format!("/lambdas/profiles/{OWNER}");
    let inventories = [
        format!("/lambdas/users/{OWNER}/wearables"),
        format!("/lambdas/collections/wearables-by-owner/{OWNER}"),
        format!("/explorer/{OWNER}/wearables?collectionType=on-chain"),
        format!("/lambdas/outfits/{OWNER}"),
    ];
    let holds_hat = |body: &Value| body.to_string().contains(&hat);

    // Warm: every answer names the hat, and a second read is a memo hit.
    for _ in 0..2 {
        let (status, body) = get(state, &profile).await;
        assert_eq!(status, ok, "{body}");
        assert_eq!(
            body["avatars"][0]["avatar"]["wearables"],
            json!([
                "urn:decentraland:off-chain:base-avatars:eyes_00",
                format!("{hat}:10")
            ])
        );
        for uri in &inventories {
            let (status, body) = get(state, uri).await;
            assert_eq!(status, ok, "{uri}: {body}");
            assert!(holds_hat(&body), "{uri}: {body}");
        }
    }

    // A rewind withdraws the Polygon publication: no warm answer is served.
    set_nft_cursor(&db.pool, "matic", None).await;
    let (status, body) = get(state, &profile).await;
    assert_eq!(
        (status, body["message"].as_str()),
        (StatusCode::NOT_FOUND, Some("Profile not found")),
        "the warm profile is not served once withdrawn: {body}"
    );
    for uri in &inventories {
        let (status, body) = get(state, uri).await;
        assert_eq!(
            (status, body["source"].as_str()),
            (StatusCode::SERVICE_UNAVAILABLE, Some("nft-index")),
            "{uri}: the warm answer is not served once withdrawn: {body}"
        );
    }

    // The refill folds the hat's transfer to another address and publishes
    // again, right away: the old branch's answers stay out of every memo.
    sqlx::query(
        "UPDATE shop.nft SET owner = '0xbbbb000000000000000000000000000000000002' \
         WHERE network = 'matic' AND contract_address = $1 AND token_id = 10",
    )
    .bind(COFFEE)
    .execute(&db.pool)
    .await
    .unwrap();
    set_nft_cursor(&db.pool, "matic", Some(0)).await;
    let (status, body) = get(state, &profile).await;
    assert_eq!(status, ok, "{body}");
    assert_eq!(
        body["avatars"][0]["avatar"]["wearables"],
        json!(["urn:decentraland:off-chain:base-avatars:eyes_00"]),
        "the hat left with its token"
    );
    for uri in &inventories {
        let (status, body) = get(state, uri).await;
        assert_eq!(status, ok, "{uri}: {body}");
        assert!(
            !holds_hat(&body),
            "{uri}: an old-branch answer came back: {body}"
        );
    }
    db.drop().await;
}

/// A fill is filed under the publications its own ownership read observed,
/// not under the key looked up before it: the lookup sees Polygon withdrawn,
/// Polygon publishes before the profile and outfits fills read ownership,
/// and once Polygon is withdrawn again (the same key as the lookup) neither
/// answer comes back from its memo.
#[tokio::test]
async fn a_fill_is_filed_under_the_publication_its_read_observed() {
    let Some(OwnershipFixture {
        db, walking, hat, ..
    }) = ownership_fixture().await
    else {
        return;
    };
    let state = &walking;
    set_nft_cursor(&db.pool, "ethereum", Some(2)).await;
    set_nft_cursor(&db.pool, "matic", None).await;
    let looked_up = super::owned_items::Publications::current(state)
        .await
        .unwrap();
    assert_eq!(looked_up.matic, None);
    set_nft_cursor(&db.pool, "matic", Some(2)).await;

    let owner = OWNER.to_string();
    let entities = super::profiles::active_entities(state, std::slice::from_ref(&owner))
        .await
        .unwrap();
    let filled = super::profiles::process_and_cache_profiles(
        state,
        std::slice::from_ref(&owner),
        &entities,
        looked_up,
    )
    .await
    .unwrap();
    assert!(filled[0].1.to_string().contains(&hat), "{filled:?}");
    let outfits = super::lambdas_catalog::fill_outfits(state, OWNER, looked_up)
        .await
        .unwrap_or_else(|_| panic!("the outfits fill reads"));
    assert!(outfits.to_string().contains(&hat), "{outfits}");

    set_nft_cursor(&db.pool, "matic", None).await;
    assert_eq!(
        super::owned_items::Publications::current(state)
            .await
            .unwrap(),
        looked_up,
        "the lookup key recurs"
    );
    let (status, body) = get(state, &format!("/lambdas/profiles/{OWNER}")).await;
    assert_eq!(
        (status, body["message"].as_str()),
        (StatusCode::NOT_FOUND, Some("Profile not found")),
        "{body}"
    );
    let (status, body) = get(state, &format!("/lambdas/outfits/{OWNER}")).await;
    assert_eq!(
        (status, body["source"].as_str()),
        (StatusCode::SERVICE_UNAVAILABLE, Some("nft-index")),
        "{body}"
    );
    db.drop().await;
}

/// A warm outfits answer lasts only while the networks it needed are fresh:
/// Polygon published 3 s short of the freshness window answers, then, with
/// no publication in between, the same memo is not served past the window.
/// Profiles keep the complete rule and still answer.
#[tokio::test]
async fn a_warm_outfit_expires_with_the_freshness_of_its_networks() {
    let Some(OwnershipFixture {
        db, walking, hat, ..
    }) = ownership_fixture().await
    else {
        return;
    };
    let state = &walking;
    set_nft_cursor(&db.pool, "ethereum", Some(2)).await;
    let window = super::chain_walk::READY_WINDOW.num_seconds();
    sqlx::query(
        "INSERT INTO shop.chain_cursor (indexer, network, start_block, last_block, folded_block, folded_at, full_history, chain_id) \
         VALUES ('nft', 'matic', 1, 100, 100, timezone('UTC', now()) - make_interval(secs => $1), true, 137) \
         ON CONFLICT (indexer, network) DO UPDATE SET folded_block = 100, folded_at = EXCLUDED.folded_at",
    )
    .bind((window - 3) as f64)
    .execute(&db.pool)
    .await
    .unwrap();
    let outfits = format!("/lambdas/outfits/{OWNER}");
    for _ in 0..2 {
        let (status, body) = get(state, &outfits).await;
        assert_eq!(status, StatusCode::OK, "{body}");
        assert!(body.to_string().contains(&hat), "{body}");
    }
    tokio::time::sleep(std::time::Duration::from_secs(4)).await;
    let (status, body) = get(state, &outfits).await;
    assert_eq!(
        (status, body["source"].as_str()),
        (StatusCode::SERVICE_UNAVAILABLE, Some("nft-index")),
        "a warm answer past the freshness window: {body}"
    );
    let (status, body) = get(state, &format!("/lambdas/profiles/{OWNER}")).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(body.to_string().contains(&hat), "{body}");
    db.drop().await;
}
