//! The small public reads under both mounts: `GET /v1/info`, the rarities,
//! an item's utility and contents by contract and on-chain id, and the
//! published contract addresses by tag.

use std::sync::Arc;

use axum::Router;
use axum::body::Body;
use axum::http::{HeaderMap, Request, StatusCode, header};
use serde_json::{Value, json};
use sqlx::PgPool;
use tower::ServiceExt;
use uuid::Uuid;

use super::support::{FakeCommittee, FakeOracle, committee_index, temp_storage};
use crate::storage::Storage;

const MOUNTS: [&str; 2] = ["", "/builder-api"];
const OWNER: &str = "0x00000000000000000000000000000000000000a1";
const NAMER: &str = "0x00000000000000000000000000000000000000a2";
const MANAGER: &str = "0x00000000000000000000000000000000000000a3";
const USD: &str = "10000000000000000000";

async fn get(app: &Router, path: &str) -> (StatusCode, HeaderMap, Value) {
    let response = app
        .clone()
        .oneshot(Request::get(path).body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let headers = response.headers().clone();
    let bytes = axum::body::to_bytes(response.into_body(), 64 << 20)
        .await
        .unwrap();
    (
        status,
        headers,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

fn app_over(state: crate::SharedState) -> Router {
    Router::new()
        .merge(crate::root_router(state.clone()))
        .nest("/builder-api", crate::router(state))
}

async fn rarities_app(subgraph: &FakeCommittee, oracle: Option<&FakeOracle>) -> Router {
    rarities_app_on(subgraph, oracle, crate::network::POLYGON).await
}

async fn rarities_app_on(
    subgraph: &FakeCommittee,
    oracle: Option<&FakeOracle>,
    network: crate::network::Network,
) -> Router {
    let rpc = match oracle {
        Some(oracle) => Some(oracle.url().await),
        None => None,
    };
    app_over(Arc::new(
        crate::State::new(
            sqlx::postgres::PgPoolOptions::new()
                .connect_lazy("postgres://nobody:nothing@127.0.0.1:9/none")
                .unwrap(),
            temp_storage().await,
            Arc::default(),
            dcl_http::Outbound::for_tests(),
            Some(committee_index(subgraph).await),
            None,
            crate::Settings {
                network,
                polygon_rpc_url: rpc,
                ..crate::Settings::default()
            },
        )
        .unwrap(),
    ))
}

fn rarity(name: &str, max_supply: &str, mana: &str) -> Value {
    json!({
        "id": name,
        "name": name,
        "price": USD,
        "maxSupply": max_supply,
        "prices": { "MANA": mana, "USD": USD },
    })
}

#[tokio::test]
async fn info_answers_upstreams_body_to_anyone() {
    let app = app_over(super::support::state_without_db(None).await);
    for mount in MOUNTS {
        let (status, _, body) = get(&app, &format!("{mount}/v1/info")).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body, json!({ "ok": true, "data": { "version": "" } }));
    }
}

#[tokio::test]
async fn rarities_merge_the_subgraph_with_the_oracle_and_are_kept() {
    let subgraph = FakeCommittee::with(&[]);
    subgraph.set_rarities(Some(&[("common", USD, "100000"), ("epic", USD, "1000")]));
    let oracle = FakeOracle::with(&[("common", "4000000000000000000"), ("epic", "5")]);
    oracle.hold_rarities_until(2);
    let app = rarities_app(&subgraph, Some(&oracle)).await;
    let all = json!({
        "ok": true,
        "data": [rarity("common", "100000", "4000000000000000000"), rarity("epic", "1000", "5")],
    });
    for mount in MOUNTS {
        let (status, _, body) = get(&app, &format!("{mount}/v1/rarities")).await;
        assert_eq!((status, body), (StatusCode::OK, all.clone()));
        let (status, _, body) = get(&app, &format!("{mount}/v1/rarities/epic")).await;
        assert_eq!(
            (status, body),
            (
                StatusCode::OK,
                json!({ "ok": true, "data": rarity("epic", "1000", "5") })
            )
        );
        let (status, _, body) = get(&app, &format!("{mount}/v1/rarities/invalid")).await;
        assert_eq!(
            (status, body),
            (
                StatusCode::NOT_FOUND,
                json!({ "ok": false, "data": { "name": "invalid" }, "error": "Rarity not found" })
            )
        );
    }
    assert_eq!(subgraph.rarity_hits(), 1, "one read answers the minute");
    let mut asked = oracle.asked();
    asked.sort();
    assert_eq!(
        asked,
        ["common", "epic"],
        "the oracle is asked once per listed rarity, never for a name from the path"
    );
    assert_eq!(
        oracle.most_rarities_at_once(),
        2,
        "the listed rarities are asked at once"
    );
}

/// The `/create` app checks the quoted fee against its own chain's oracle:
/// an Amoy node quotes Amoy's, and only over an RPC that answers Amoy.
#[tokio::test]
async fn an_amoy_node_quotes_amoys_oracle_and_no_other_chains() {
    use crate::network::{AMOY, POLYGON};
    let listed = || {
        let subgraph = FakeCommittee::with(&[]);
        subgraph.set_rarities(Some(&[("common", USD, "100000")]));
        subgraph
    };
    let amoy = FakeOracle::with(&[("common", "7")]).on(AMOY);
    let app = rarities_app_on(&listed(), Some(&amoy), AMOY).await;
    let (status, _, body) = get(&app, "/v1/rarities/common").await;
    assert_eq!(
        (status, body),
        (
            StatusCode::OK,
            json!({ "ok": true, "data": rarity("common", "100000", "7") })
        )
    );
    let mainnet = FakeOracle::with(&[("common", "7")]);
    let other_chain = FakeOracle::with(&[("common", "7")]).on(AMOY);
    other_chain.set_other_chain(true);
    for (case, app) in [
        (
            "a Polygon mainnet RPC on an Amoy node",
            rarities_app_on(&listed(), Some(&mainnet), AMOY).await,
        ),
        (
            "an Amoy RPC on a mainnet node",
            rarities_app_on(&listed(), Some(&amoy), POLYGON).await,
        ),
        (
            "an RPC holding Amoy's oracle that answers another chain",
            rarities_app_on(&listed(), Some(&other_chain), AMOY).await,
        ),
    ] {
        let (status, _, _) = get(&app, "/v1/rarities/common").await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{case}");
    }
}

#[tokio::test]
async fn rarities_that_cannot_be_read_answer_503_never_an_empty_list() {
    let unavailable = json!({
        "ok": false,
        "data": {},
        "error": "The rarities are not available. Try again later.",
    });
    let subgraph_down = FakeCommittee::with(&[]);
    let oracle = FakeOracle::with(&[("common", "4")]);
    let listed = FakeCommittee::with(&[]);
    listed.set_rarities(Some(&[("common", USD, "100000")]));
    let empty = FakeCommittee::with(&[]);
    empty.set_rarities(Some(&[]));
    let malformed = FakeCommittee::with(&[]);
    malformed.set_rarities(Some(&[("common", "ten", "100000")]));
    let oracle_down = FakeOracle::default();
    let oracle_without_common = FakeOracle::with(&[("epic", "4")]);
    let cases = [
        (
            "subgraph down",
            rarities_app(&subgraph_down, Some(&oracle)).await,
        ),
        ("no oracle configured", rarities_app(&listed, None).await),
        (
            "oracle down",
            rarities_app(&listed, Some(&oracle_down)).await,
        ),
        (
            "oracle reverts",
            rarities_app(&listed, Some(&oracle_without_common)).await,
        ),
        (
            "no rarity listed",
            rarities_app(&empty, Some(&oracle)).await,
        ),
        (
            "malformed price",
            rarities_app(&malformed, Some(&oracle)).await,
        ),
        (
            "no subgraph",
            app_over(super::support::state_without_db(None).await),
        ),
    ];
    for (case, app) in cases {
        for path in ["/v1/rarities", "/v1/rarities/common"] {
            let (status, _, body) = get(&app, path).await;
            assert_eq!(
                (status, &body),
                (StatusCode::SERVICE_UNAVAILABLE, &unavailable),
                "{case}: {path}"
            );
        }
    }
    assert_eq!(subgraph_down.rarity_hits(), 1, "a failure is kept too");
}

struct Db {
    pool: PgPool,
    app: Router,
    store: Arc<Storage>,
    state: crate::SharedState,
}

async fn db() -> Option<Db> {
    let pool = crate::db::scratch::isolated_pool(4).await?;
    sqlx::query(
        "INSERT INTO shop.mirror_state (mirror, network, source_url, last_complete_pass_at, pass_started_at) \
         VALUES ('collections', 'matic', 'https://subgraph.example', timezone('UTC', now()), timezone('UTC', now()))",
    )
    .execute(&pool)
    .await
    .unwrap();
    crate::tests::support::publish_registry_index(&mut *pool.acquire().await.unwrap(), false).await;
    let store = temp_storage().await;
    let state = Arc::new(
        crate::State::new(
            pool.clone(),
            store.clone(),
            Arc::default(),
            dcl_http::Outbound::for_tests(),
            None,
            None,
            crate::Settings::default(),
        )
        .unwrap(),
    );
    let app = app_over(state.clone());
    Some(Db {
        pool,
        app,
        store: store.item_files().unwrap().clone(),
        state,
    })
}

fn contract() -> String {
    format!("0x{}", Uuid::new_v4().simple().to_string()[..20].repeat(2))
}

async fn collection(pool: &PgPool, contract: Option<&str>, third_party: bool) -> Uuid {
    let id = Uuid::new_v4();
    let provider = "urn:decentraland:matic:collections-thirdparty:small-reads";
    sqlx::query(
        "INSERT INTO builder.collections (id, name, eth_address, contract_address, third_party_id, urn_suffix) \
         VALUES ($1, $2, $3, $4, $5, $6)",
    )
    .bind(id)
    .bind(format!("Small reads {id}"))
    .bind(OWNER)
    .bind(contract)
    .bind(third_party.then_some(provider))
    .bind(third_party.then(|| format!("{provider}:{}", id.simple())))
    .execute(pool)
    .await
    .unwrap();
    id
}

struct Item<'a> {
    collection: Uuid,
    chain_id: Option<&'a str>,
    utility: Option<&'a str>,
    video: Option<&'a str>,
    tags: &'a [&'a str],
    contents: &'a [(&'a str, &'a str, &'a str)],
}

async fn item(pool: &PgPool, item: Item<'_>) -> Uuid {
    let id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO builder.items (id, collection_id, eth_address, name, type, blockchain_item_id, utility, video, \
           video_named_by, data) \
         VALUES ($1, $2, $3, 'Small reads item', 'wearable', $4, $5, $6, CASE WHEN $6 IS NULL THEN NULL ELSE $3 END, $7::json)",
    )
    .bind(id)
    .bind(item.collection)
    .bind(OWNER)
    .bind(item.chain_id)
    .bind(item.utility)
    .bind(item.video)
    .bind(json!({ "tags": item.tags }).to_string())
    .execute(pool)
    .await
    .unwrap();
    for (position, (file, hash, named_by)) in item.contents.iter().enumerate() {
        sqlx::query(
            "INSERT INTO builder.item_contents (item_id, file, hash, position, named_by) VALUES ($1, $2, $3, $4, $5)",
        )
        .bind(id)
        .bind(file)
        .bind(hash)
        .bind(position as i32)
        .bind(named_by)
        .execute(pool)
        .await
        .unwrap();
    }
    id
}

/// A file in the store; `uploader` holds its upload record.
async fn file(pool: &PgPool, store: &Storage, uploader: Option<&str>) -> String {
    let bytes = super::support::unique("small-reads");
    let hash = dcl_auth::hash_bytes_v1(&bytes);
    store.store(&hash, bytes.clone().into()).await.unwrap();
    if let Some(uploader) = uploader {
        sqlx::query(
            "INSERT INTO builder.uploaded_contents (hash, size, uploaded_by) VALUES ($1, $2, $3)",
        )
        .bind(&hash)
        .bind(bytes.len() as i64)
        .bind(uploader)
        .execute(pool)
        .await
        .unwrap();
    }
    hash
}

#[tokio::test]
async fn an_items_utility_is_found_by_its_exact_contract_and_chain_id() {
    let Some(d) = db().await else {
        return;
    };
    let contract = contract();
    let c = collection(&d.pool, Some(&contract), false).await;
    let empty = Item {
        collection: c,
        chain_id: None,
        utility: None,
        video: None,
        tags: &[],
        contents: &[],
    };
    item(
        &d.pool,
        Item {
            chain_id: Some("3"),
            utility: Some("Opens the door"),
            ..empty
        },
    )
    .await;
    item(
        &d.pool,
        Item {
            utility: Some("Unminted"),
            ..empty
        },
    )
    .await;
    let upper = contract.to_uppercase().replace("0X", "0x");
    for mount in MOUNTS {
        let path = |address: &str, id: &str| {
            format!("{mount}/v1/published-collections/{address}/items/{id}/utility")
        };
        let (status, _, body) = get(&d.app, &path(&contract, "3")).await;
        assert_eq!(
            (status, body),
            (
                StatusCode::OK,
                json!({ "ok": true, "data": { "utility": "Opens the door" } })
            )
        );
        for (address, id) in [
            (upper.as_str(), "3"),
            (contract.as_str(), "4"),
            (contract.as_str(), "03"),
            ("not-an-address", "3"),
            (contract.as_str(), "x"),
        ] {
            let (status, _, body) = get(&d.app, &path(address, id)).await;
            let missing = format!("{address}-{id}");
            assert_eq!(
                (status, body),
                (
                    StatusCode::NOT_FOUND,
                    json!({ "ok": false, "data": { "id": missing }, "error": "The item doesn't exist." })
                ),
                "{address} {id}"
            );
        }
    }
}

#[tokio::test]
async fn an_items_contents_by_contract_carry_only_files_anyone_may_read() {
    let Some(d) = db().await else {
        return;
    };
    let contract = contract();
    let c = collection(&d.pool, Some(&contract), false).await;
    let model = file(&d.pool, &d.store, Some(OWNER)).await;
    let unuploaded = file(&d.pool, &d.store, None).await;
    let script = file(&d.pool, &d.store, Some(OWNER)).await;
    let approved_video = file(&d.pool, &d.store, Some(OWNER)).await;
    let newer_video = file(&d.pool, &d.store, Some(OWNER)).await;
    item(
        &d.pool,
        Item {
            collection: c,
            chain_id: Some("0"),
            utility: None,
            video: None,
            tags: &[],
            contents: &[
                ("male/model.glb", &model, OWNER),
                ("thumbnail.png", &unuploaded, NAMER),
            ],
        },
    )
    .await;
    item(
        &d.pool,
        Item {
            collection: c,
            chain_id: Some("1"),
            utility: None,
            video: Some(&approved_video),
            tags: &[],
            contents: &[
                ("game.js", &script, OWNER),
                ("video.mp4", &newer_video, OWNER),
                ("bin/model.glb", &model, OWNER),
            ],
        },
    )
    .await;
    item(
        &d.pool,
        Item {
            collection: c,
            chain_id: Some("2"),
            utility: None,
            video: None,
            tags: &[],
            contents: &[
                ("game.js", &script, OWNER),
                ("video.mp4", &newer_video, OWNER),
            ],
        },
    )
    .await;
    item(
        &d.pool,
        Item {
            collection: c,
            chain_id: Some("5"),
            utility: None,
            video: Some(&unuploaded),
            tags: &[],
            contents: &[("game.js", &script, OWNER)],
        },
    )
    .await;
    let draft = collection(&d.pool, Some(&self::contract()), false).await;
    item(
        &d.pool,
        Item {
            collection: draft,
            chain_id: None,
            utility: None,
            video: None,
            tags: &[],
            contents: &[("male/model.glb", &unuploaded, OWNER)],
        },
    )
    .await;
    let upper = contract.to_uppercase().replace("0X", "0x");
    for mount in MOUNTS {
        let path = |address: &str, id: &str| format!("{mount}/v1/items/{address}/{id}/contents");
        let (status, _, body) = get(&d.app, &path(&upper, "0")).await;
        assert_eq!(
            (status, &body),
            (
                StatusCode::OK,
                &json!({ "ok": true, "data": { "male/model.glb": model } })
            ),
            "a name nobody who wrote it uploaded is left out"
        );
        let (status, _, body) = get(&d.app, &path(&contract, "1")).await;
        assert_eq!(
            (status, &body),
            (
                StatusCode::OK,
                &json!({ "ok": true, "data": {
                    "game.js": script,
                    "video.mp4": approved_video,
                    "bin/model.glb": model,
                } })
            ),
            "a smart wearable answers its kept video"
        );
        for hash in body["data"].as_object().unwrap().values() {
            let (status, _, _) = get(
                &d.app,
                &format!("{mount}/v1/storage/contents/{}", hash.as_str().unwrap()),
            )
            .await;
            assert_eq!(status, StatusCode::OK, "every hash it names opens unsigned");
        }
        let (_, _, body) = get(&d.app, &path(&contract, "2")).await;
        assert_eq!(
            body,
            json!({ "ok": true, "data": { "game.js": script, "video.mp4": null } })
        );
        let (_, _, body) = get(&d.app, &path(&contract, "5")).await;
        assert_eq!(
            body,
            json!({ "ok": true, "data": { "game.js": script } }),
            "a kept video nobody may read is left out"
        );
        for (address, id) in [
            (contract.as_str(), "9"),
            (contract.as_str(), "1abc"),
            (contract.as_str(), "0x"),
        ] {
            let (status, _, body) = get(&d.app, &path(address, id)).await;
            assert_eq!(
                (status, body),
                (
                    StatusCode::OK,
                    json!({ "ok": false, "data": {}, "error": "The item doesn't exist." })
                ),
                "{address} {id}"
            );
        }
        for (address, id, data, error) in [
            (
                "0xa",
                "0",
                json!({ "contractAddress": "0xa" }),
                "Invalid address 0xa",
            ),
            (
                "Null",
                "0",
                json!({ "contractAddress": "null" }),
                "Invalid address null",
            ),
            (
                contract.as_str(),
                "aItemId",
                json!({ "itemId": "aitemid" }),
                "Invalid Item ID aitemid",
            ),
        ] {
            let (status, _, body) = get(&d.app, &path(address, id)).await;
            assert_eq!(
                (status, body),
                (
                    StatusCode::BAD_REQUEST,
                    json!({ "ok": false, "data": data, "error": error })
                ),
                "{address} {id}"
            );
        }
    }
}

#[tokio::test]
async fn addresses_list_only_published_standard_contracts() {
    let Some(d) = db().await else {
        return;
    };
    let tag = format!("event{}", &Uuid::new_v4().simple().to_string()[..8]);
    let published = contract();
    let other = contract();
    let draft = contract();
    let empty = |collection| Item {
        collection,
        chain_id: None,
        utility: None,
        video: None,
        tags: &[],
        contents: &[],
    };
    let upper_tag = tag.to_uppercase();
    for (contract, chain_id, tags) in [
        (published.as_str(), Some("0"), [upper_tag.as_str()]),
        (other.as_str(), Some("0"), ["unrelated"]),
        (draft.as_str(), None, [tag.as_str()]),
    ] {
        let c = collection(&d.pool, Some(contract), false).await;
        item(
            &d.pool,
            Item {
                chain_id,
                tags: &tags,
                ..empty(c)
            },
        )
        .await;
    }
    let third_party = collection(&d.pool, None, true).await;
    item(
        &d.pool,
        Item {
            tags: &[&tag],
            ..empty(third_party)
        },
    )
    .await;
    for mount in MOUNTS {
        let (status, headers, body) = get(&d.app, &format!("{mount}/v1/addresses?tag={tag}")).await;
        assert_eq!(
            (status, &body),
            (StatusCode::OK, &json!({ "ok": true, "data": [published] }))
        );
        assert_eq!(
            headers.get(header::CACHE_CONTROL).unwrap(),
            "public,max-age=300,s-maxage=300"
        );
        let (_, _, body) = get(
            &d.app,
            &format!("{mount}/v1/addresses?tag={tag}&type=third_party&page=1&limit=1"),
        )
        .await;
        assert_eq!(
            body,
            json!({ "ok": true, "data": { "total": 1, "limit": 1, "pages": 1, "page": 1, "results": [published] } })
        );
        let (_, _, body) = get(
            &d.app,
            &format!("{mount}/v1/addresses?tag={tag}&page=3&limit=1"),
        )
        .await;
        assert_eq!(
            body,
            json!({ "ok": true, "data": { "total": 1, "limit": 1, "pages": 1, "page": 3, "results": [] } }),
            "a page past the last still counts the matches"
        );
        let (_, _, body) = get(
            &d.app,
            &format!("{mount}/v1/addresses?tag={tag}&is_published=false"),
        )
        .await;
        assert_eq!(body, json!({ "ok": true, "data": [] }));
        let sorted = |body: Value| {
            let mut listed: Vec<String> = serde_json::from_value(body["data"].clone()).unwrap();
            listed.sort();
            listed
        };
        let (_, _, every) = get(&d.app, &format!("{mount}/v1/addresses")).await;
        assert_eq!(sorted(every.clone()).len(), 2, "{every}");
        for query in ["tag=", "page=1", "page=2"] {
            let (_, _, body) = get(&d.app, &format!("{mount}/v1/addresses?{query}")).await;
            assert_eq!(sorted(body), sorted(every.clone()), "{query}");
        }
        let (status, _, body) = get(&d.app, &format!("{mount}/v1/addresses?status=nope")).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    }
}

fn hash_of(bytes: &[u8]) -> String {
    dcl_auth::hash_bytes_v1(bytes)
}

fn smart_wearable(id: Uuid, collection: Uuid, video: &[u8], contents: &[(&str, &[u8])]) -> Value {
    json!({
        "id": id,
        "name": "Smart hat",
        "eth_address": OWNER,
        "collection_id": collection,
        "rarity": "common",
        "video": hash_of(video),
        "contents": super::support::contents_of(contents),
    })
}

async fn upload_record(pool: &PgPool, hash: &str, uploader: &str) -> bool {
    sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM builder.uploaded_contents WHERE hash = $1 AND uploaded_by = $2)",
    )
    .bind(hash)
    .bind(uploader)
    .fetch_one(pool)
    .await
    .unwrap()
}

#[tokio::test]
async fn a_published_smart_wearable_answers_its_approved_video_after_a_resave() {
    let Some(d) = db().await else {
        return;
    };
    let store = &d.state.items;
    let contract = contract();
    let c = collection(&d.pool, Some(&contract), false).await;
    let id = Uuid::new_v4();
    let model = super::support::unique("smart-model");
    let script = super::support::unique("smart-script");
    let approved = super::support::unique("smart-approved-video");
    let newer = super::support::unique("smart-newer-video");
    let draft = smart_wearable(
        id,
        c,
        &approved,
        &[
            ("model.glb", &model),
            ("game.js", &script),
            ("video.mp4", &approved),
        ],
    );
    super::support::save_item(store, OWNER, &draft)
        .await
        .unwrap();
    super::support::upload(store, OWNER, id, &[&model, &script, &approved])
        .await
        .unwrap();
    sqlx::query("UPDATE builder.items SET blockchain_item_id = '0' WHERE id = $1")
        .bind(id)
        .execute(&d.pool)
        .await
        .unwrap();
    let resave = smart_wearable(
        id,
        c,
        &newer,
        &[
            ("model.glb", &model),
            ("game.js", &script),
            ("video.mp4", &newer),
        ],
    );
    super::support::save_item(store, OWNER, &resave)
        .await
        .unwrap();
    super::support::upload(store, OWNER, id, &[&newer])
        .await
        .unwrap();
    let approved = hash_of(&approved);
    assert!(
        upload_record(&d.pool, &approved, OWNER).await,
        "the kept video's upload record stays while the item keeps it"
    );
    let mut conn = d.pool.acquire().await.unwrap();
    let referenced =
        crate::storage::gc::referenced_rows_among_on(&mut conn, std::slice::from_ref(&approved))
            .await
            .unwrap();
    drop(conn);
    assert!(
        referenced.contains(&approved),
        "the collector keeps the kept video's bytes"
    );
    for mount in MOUNTS {
        let (status, _, body) =
            get(&d.app, &format!("{mount}/v1/items/{contract}/0/contents")).await;
        assert_eq!(
            (status, &body),
            (
                StatusCode::OK,
                &json!({ "ok": true, "data": {
                    "model.glb": hash_of(&model),
                    "game.js": hash_of(&script),
                    "video.mp4": approved,
                } })
            ),
            "the approved video, as builder-server answers it"
        );
        let (status, _, _) = get(&d.app, &format!("{mount}/v1/storage/contents/{approved}")).await;
        assert_eq!(status, StatusCode::OK, "the approved video opens unsigned");
    }
}

#[tokio::test]
async fn a_wallet_that_saves_an_item_cannot_open_another_wallets_file_through_its_video() {
    let Some(d) = db().await else {
        return;
    };
    let store = &d.state.items;
    let contract = contract();
    let c = collection(&d.pool, Some(&contract), false).await;
    let private = file(&d.pool, &d.store, Some(OWNER)).await;
    let model = super::support::unique("plain-model");
    let script = super::support::unique("manager-script");
    let id = Uuid::new_v4();
    let plain = json!({
        "id": id,
        "name": "Plain hat",
        "eth_address": OWNER,
        "collection_id": c,
        "rarity": "common",
        "contents": super::support::contents_of(&[("model.glb", &model)]),
    });
    super::support::save_item(store, OWNER, &plain)
        .await
        .unwrap();
    super::support::upload(store, OWNER, id, &[&model])
        .await
        .unwrap();
    sqlx::query("UPDATE builder.items SET blockchain_item_id = '0' WHERE id = $1")
        .bind(id)
        .execute(&d.pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO shop.collections (network, contract_address, chain_id, name, creator, owner, urn, \
           is_completed, is_approved, is_editable, managers, minters, created_at, updated_at, reviewed_at, mirrored_at) \
         VALUES ('matic', $1, 137, 'Small reads', $2, $2, 'urn:small-reads', true, true, true, $3, '{}', 1, 1, 1, \
           timezone('UTC', now()))",
    )
    .bind(&contract)
    .bind(OWNER)
    .bind(vec![MANAGER.to_string()])
    .execute(&d.pool)
    .await
    .unwrap();
    let mut named = plain.clone();
    named["video"] = private.clone().into();
    named["contents"] = super::support::contents_of(&[("model.glb", &model), ("game.js", &script)]);
    super::support::save_item(store, MANAGER, &named)
        .await
        .unwrap();
    super::support::upload(store, MANAGER, id, &[&script])
        .await
        .unwrap();
    super::support::save_item(store, OWNER, &named)
        .await
        .unwrap();
    let stored: (Option<String>, Option<String>) =
        sqlx::query_as("SELECT video, video_named_by FROM builder.items WHERE id = $1")
            .bind(id)
            .fetch_one(&d.pool)
            .await
            .unwrap();
    assert_eq!(
        stored,
        (Some(private.clone()), Some(MANAGER.to_string())),
        "the video's writer is the wallet whose save wrote it"
    );
    for mount in MOUNTS {
        let (status, _, _) = get(&d.app, &format!("{mount}/v1/storage/contents/{private}")).await;
        assert_eq!(
            status,
            StatusCode::NOT_FOUND,
            "a video named by a wallet without the bytes opens nothing"
        );
        let (_, _, body) = get(&d.app, &format!("{mount}/v1/items/{contract}/0/contents")).await;
        assert_eq!(
            body,
            json!({ "ok": true, "data": { "model.glb": hash_of(&model), "game.js": hash_of(&script) } })
        );
    }
}
