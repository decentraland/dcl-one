use crate::access::granted;
use alloy_primitives::keccak256;
use axum::{Json, Router, extract::State, routing::post};
use serde_json::{Value, json};
use std::sync::{Arc, Mutex};
use uuid::Uuid;

use crate::drafts::CollectionDraft;
use crate::items::ItemsComponent;
use crate::network::{AMOY, POLYGON};
use crate::publication::PublicationPreparation;
use crate::publication_chain::{PublicationChain, publication_calldata};
use dcl_http::Outbound;

use super::limits::{Call, node_over, send};
use super::support::{
    OWNER_KEY, cleanup, contents_of, read_item, save_item, scratch_component, unique, upload,
};

const OWNER: &str = "0x1111111111111111111111111111111111111111";
const OTHER: &str = "0x2222222222222222222222222222222222222222";
const CONTRACT: &str = "0x3333333333333333333333333333333333333333";
const HASH: &str = "0x4444444444444444444444444444444444444444444444444444444444444444";
const BLOCK: &str = "0x5555555555555555555555555555555555555555555555555555555555555555";
const REPLACEMENT: &str = "0x7777777777777777777777777777777777777777777777777777777777777777";

async fn rpc(State(state): State<Arc<Mutex<Value>>>, Json(req): Json<Value>) -> Json<Value> {
    let s = state.lock().unwrap();
    let result = match req["method"].as_str().unwrap() {
        "eth_chainId" => s.get("chain").cloned().unwrap_or(json!("0x89")),
        "eth_getTransactionByHash" => [&s["tx"], &s["replacement"]]
            .into_iter()
            .find(|tx| tx["hash"] == req["params"][0])
            .cloned()
            .unwrap_or(Value::Null),
        "eth_getTransactionReceipt" => s["receipt"].clone(),
        "eth_getBlockByNumber" => {
            if req["params"][0] == "finalized" {
                json!({"number":s["finalized"], "hash":BLOCK})
            } else {
                json!({"number":"0x10", "hash":s["canonical"]})
            }
        }
        "eth_call" => {
            let selector = format!("0x{}", &keccak256("creator()").to_string()[2..10]);
            if req["params"][0]["data"] == selector {
                json!(format!(
                    "0x{:0>64}",
                    s["creator"].as_str().unwrap().trim_start_matches("0x")
                ))
            } else {
                json!(format!("0x{:064x}", s["count"].as_u64().unwrap()))
            }
        }
        method => panic!("unexpected RPC method {method}"),
    };
    Json(json!({"jsonrpc":"2.0", "id":req["id"], "result":result}))
}

/// [`rpc`] over `state` on a loopback port: its URL and task.
async fn serve_rpc(state: Arc<Mutex<Value>>) -> (String, tokio::task::JoinHandle<()>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}/", listener.local_addr().unwrap());
    let router = Router::new().route("/", post(rpc)).with_state(state);
    let task = tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });
    (url, task)
}

struct Draft {
    collection: CollectionDraft,
    item: Value,
    model: Vec<u8>,
}

async fn draft(store: &ItemsComponent) -> Draft {
    draft_of(store, OWNER).await
}

async fn draft_of(store: &ItemsComponent, owner: &str) -> Draft {
    let id = Uuid::new_v4();
    let collection: CollectionDraft = serde_json::from_value(
        json!({"id":id, "name":format!("Transaction {}", &id.simple().to_string()[..8])}),
    )
    .unwrap();
    super::support::save_collection(
        store,
        owner,
        serde_json::json!({"id": collection.id, "name": collection.name, "urn": collection.urn}),
    )
    .await
    .unwrap();
    let (model, image) = (unique("model"), unique("image"));
    let item = json!({"id":Uuid::new_v4(), "name":"Hat", "type":"wearable", "collection_id":id,
        "price":"1000000000000000001", "rarity":"rare", "thumbnail":"thumbnail.png",
        "contents": contents_of(&[("model.glb", &model), ("thumbnail.png", &image)]),
        "data":{"category":"hat", "representations":[{"mainFile":"model.glb", "contents":["model.glb"], "bodyShapes":["urn:decentraland:off-chain:base-avatars:BaseMale"]}]}});
    save_item(store, owner, &item).await.unwrap();
    upload(store, owner, item_id(&item), &[&model, &image])
        .await
        .unwrap();
    Draft {
        collection,
        item,
        model,
    }
}

fn item_id(item: &Value) -> Uuid {
    Uuid::parse_str(item["id"].as_str().unwrap()).unwrap()
}

fn hashes_of(items: &[&Value]) -> Vec<String> {
    items
        .iter()
        .flat_map(|i| i["contents"].as_object().unwrap().values())
        .map(|h| h.as_str().unwrap().to_string())
        .collect()
}

/// builder-server with `CHAIN_NAME=Sepolia`: the owner's transaction goes to
/// Amoy's collection manager and the receipt's event comes from Amoy's
/// factory, over an RPC that answers Amoy; mainnet's contracts or chain
/// publish nothing.
#[tokio::test]
async fn an_amoy_publication_is_verified_against_amoys_contracts_and_chain() {
    let Some(store) = scratch_component().await else {
        return;
    };
    let store = store.with_network(AMOY);
    let Draft {
        collection, item, ..
    } = draft(&store).await;
    let id = collection.id;
    let publisher = || granted::publisher(OWNER, id);
    let prepared = store.prepare_publication(&publisher()).await.unwrap();
    store
        .begin_publication(&publisher(), &prepared.revision)
        .await
        .unwrap();
    let transaction = json!({"hash":HASH, "from":OWNER, "to":AMOY.collection_manager, "value":"0x0",
        "input":publication_calldata(&prepared).unwrap(), "blockNumber":"0x10", "blockHash":BLOCK});
    let receipt = json!({"transactionHash":HASH, "from":OWNER, "to":AMOY.collection_manager, "status":"0x1",
        "blockNumber":"0x10", "blockHash":BLOCK,
        "logs":[{"address":AMOY.collection_factory, "topics":[format!("{:#x}", keccak256("ProxyCreated(address,bytes32)")),
            format!("0x{:0>64}", &CONTRACT[2..])], "data":prepared.salt}]});
    let state = Arc::new(Mutex::new(
        json!({"chain":"0x13882", "tx":transaction, "receipt":receipt,
        "finalized":"0x10", "canonical":BLOCK, "creator":OWNER, "count":1}),
    ));
    let (url, task) = serve_rpc(state.clone()).await;
    let http = Outbound::for_tests();
    let chain = PublicationChain {
        http: &http,
        url: &url,
    };
    state.lock().unwrap()["chain"] = json!("0x89");
    assert_eq!(
        chain
            .verify(&store, &publisher(), HASH)
            .await
            .unwrap_err()
            .status(),
        503,
        "a Polygon mainnet RPC can't verify an Amoy publication"
    );
    state.lock().unwrap()["chain"] = json!("0x13882");
    for (path, mainnet) in [
        ("/tx/to", POLYGON.collection_manager),
        ("/receipt/to", POLYGON.collection_manager),
        ("/receipt/logs/0/address", POLYGON.collection_factory),
    ] {
        let previous = state.lock().unwrap().pointer(path).unwrap().clone();
        *state.lock().unwrap().pointer_mut(path).unwrap() = json!(mainnet);
        assert!(
            chain.verify(&store, &publisher(), HASH).await.is_err(),
            "accepted mainnet's contract at {path}"
        );
        *state.lock().unwrap().pointer_mut(path).unwrap() = previous;
    }
    let published = chain.verify(&store, &publisher(), HASH).await.unwrap();
    assert_eq!(
        (
            published.status.as_str(),
            published.contract_address.as_deref()
        ),
        ("published", Some(CONTRACT))
    );
    assert_eq!(
        read_item(&store, item_id(&item)).await.unwrap()["urn"],
        format!("urn:decentraland:amoy:collections-v2:{CONTRACT}:0")
    );
    let stored: (Option<String>, Option<String>) = sqlx::query_as(
        "SELECT i.urn_suffix, c.urn_suffix FROM builder.items i \
         JOIN builder.collections c ON c.id = i.collection_id WHERE i.id = $1",
    )
    .bind(item_id(&item))
    .fetch_one(store.pool())
    .await
    .unwrap();
    assert_eq!(
        stored,
        (
            Some(format!("urn:decentraland:amoy:collections-v2:{CONTRACT}:0")),
            Some(format!("urn:decentraland:amoy:collections-v2:{CONTRACT}")),
        )
    );
    task.abort();
    cleanup(&store, &[id], &hashes_of(&[&item])).await;
}

#[tokio::test]
async fn frozen_revision_requires_a_matching_finalized_receipt_and_recovers_after_revert() {
    let Some(store) = scratch_component().await else {
        return;
    };
    let Draft {
        collection,
        mut item,
        model,
    } = draft(&store).await;
    let id = collection.id;
    let prepared = store
        .prepare_publication(&granted::publisher(OWNER, id))
        .await
        .unwrap();
    assert!(
        store
            .begin_publication(&granted::publisher(OTHER, id), &prepared.revision)
            .await
            .is_err()
    );
    assert!(
        store
            .begin_publication(&granted::publisher(OWNER, id), "outdated")
            .await
            .is_err()
    );
    let active = store
        .begin_publication(&granted::publisher(OWNER, id), &prepared.revision)
        .await
        .unwrap();
    assert_eq!(active.status, "prepared");
    assert_eq!(
        store
            .begin_publication(&granted::publisher(OWNER, id), &prepared.revision)
            .await
            .unwrap()
            .preparation
            .revision,
        prepared.revision
    );
    assert!(
        super::support::save_collection(
            &store,
            OWNER,
            serde_json::json!({"id": collection.id, "name": collection.name, "urn": collection.urn})
        )
        .await
        .is_err()
    );
    assert_eq!(
        save_item(&store, OWNER, &item).await.unwrap_err().status(),
        423
    );
    upload(&store, OWNER, item_id(&item), &[&model])
        .await
        .expect("an upload of a file the item names takes no lock");
    item["collection_id"] = json!(null);
    assert_eq!(
        save_item(&store, OWNER, &item).await.unwrap_err().status(),
        423,
        "detaching cannot bypass a publication lock"
    );
    item["collection_id"] = json!(id);
    assert!(
        store
            .cancel_publication(&granted::publisher(OTHER, id))
            .await
            .is_err()
    );
    let (one, two) = (granted::publisher(OWNER, id), granted::publisher(OWNER, id));
    let (first_claim, second_claim) = tokio::join!(
        store.claim_publication(&one, &prepared.revision),
        store.claim_publication(&two, &prepared.revision),
    );
    assert_ne!(
        first_claim.is_ok(),
        second_claim.is_ok(),
        "only one tab may request the collection transaction"
    );
    assert_eq!(
        store
            .publication_state(&granted::publisher(OWNER, id))
            .await
            .unwrap()
            .unwrap()
            .status,
        "signing"
    );
    store
        .cancel_publication(&granted::publisher(OWNER, id))
        .await
        .unwrap();
    save_item(&store, OWNER, &item).await.unwrap();
    assert!(
        store
            .begin_publication(&granted::publisher(OWNER, id), &prepared.revision)
            .await
            .is_err(),
        "an edit invalidates the original review"
    );
    let prepared = store
        .prepare_publication(&granted::publisher(OWNER, id))
        .await
        .unwrap();
    store
        .begin_publication(&granted::publisher(OWNER, id), &prepared.revision)
        .await
        .unwrap();
    let transaction = json!({"hash":HASH, "from":OWNER, "to":POLYGON.collection_manager, "value":"0x0", "input":publication_calldata(&prepared).unwrap(), "blockNumber":"0x10", "blockHash":BLOCK});
    let receipt = json!({"transactionHash":HASH, "from":OWNER, "to":POLYGON.collection_manager, "status":"0x1", "blockNumber":"0x10", "blockHash":BLOCK,
        "logs":[{"address":POLYGON.collection_factory, "topics":[format!("{:#x}", keccak256("ProxyCreated(address,bytes32)")), format!("0x{:0>64}", &CONTRACT[2..])], "data":prepared.salt}]});
    let state = Arc::new(Mutex::new(
        json!({"tx":transaction, "receipt":null, "finalized":"0x10", "canonical":BLOCK, "creator":OWNER, "count":1}),
    ));
    let (url, task) = serve_rpc(state.clone()).await;
    let http = Outbound::for_tests();
    let chain = PublicationChain {
        http: &http,
        url: &url,
    };
    for (field, value) in [
        ("from", json!(OTHER)),
        ("to", json!(OTHER)),
        ("input", json!("0xdeadbeef")),
        ("value", json!("0x1")),
    ] {
        state.lock().unwrap()["tx"][field] = value;
        assert!(
            chain
                .verify(&store, &granted::publisher(OWNER, id), HASH)
                .await
                .is_err(),
            "accepted wrong transaction {field}"
        );
        assert!(
            store
                .publication_state(&granted::publisher(OWNER, id))
                .await
                .unwrap()
                .unwrap()
                .tx_hash
                .is_none()
        );
        state.lock().unwrap()["tx"] = transaction.clone();
    }
    assert!(
        chain
            .verify(&store, &granted::publisher(OTHER, id), HASH)
            .await
            .is_err()
    );
    assert_eq!(
        chain
            .verify(&store, &granted::publisher(OWNER, id), HASH)
            .await
            .unwrap()
            .status,
        "submitted"
    );
    assert!(
        store
            .cancel_publication(&granted::publisher(OWNER, id))
            .await
            .is_err()
    );
    state.lock().unwrap()["receipt"] = receipt.clone();
    state.lock().unwrap()["finalized"] = json!("0xf");
    assert_eq!(
        chain
            .verify(&store, &granted::publisher(OWNER, id), HASH)
            .await
            .unwrap()
            .status,
        "submitted"
    );
    state.lock().unwrap()["finalized"] = json!("0x10");
    for (path, value) in [
        ("/canonical", json!(HASH)),
        ("/creator", json!(OTHER)),
        ("/count", json!(2)),
        ("/receipt/logs/0/address", json!(OTHER)),
        ("/receipt/logs/0/data", json!(HASH)),
        ("/receipt/transactionHash", json!(BLOCK)),
    ] {
        let previous = state.lock().unwrap().pointer(path).unwrap().clone();
        *state.lock().unwrap().pointer_mut(path).unwrap() = value;
        assert!(
            chain
                .verify(&store, &granted::publisher(OWNER, id), HASH)
                .await
                .is_err(),
            "accepted invalid receipt {path}"
        );
        assert!(
            !store
                .collection_by_id(&granted::read(id))
                .await
                .unwrap()
                .unwrap()
                .is_published
        );
        *state.lock().unwrap().pointer_mut(path).unwrap() = previous;
    }
    let published = chain
        .verify(&store, &granted::publisher(OWNER, id), HASH)
        .await
        .unwrap();
    assert_eq!(published.status, "published");
    assert_eq!(published.contract_address.as_deref(), Some(CONTRACT));
    assert_eq!(
        chain
            .verify(&store, &granted::publisher(OWNER, id), HASH)
            .await
            .unwrap()
            .status,
        "published"
    );
    let saved = read_item(&store, item_id(&item)).await.unwrap();
    assert_eq!(
        saved["is_published"], true,
        "an item the node published reads as published before the chain copy holds it"
    );
    assert_eq!(saved["blockchain_item_id"], "0");
    assert_eq!(
        saved["urn"],
        format!("urn:decentraland:matic:collections-v2:{CONTRACT}:0")
    );
    assert_eq!(saved["beneficiary"], OWNER);
    let mut rarer = item.clone();
    rarer["rarity"] = json!("epic");
    assert_eq!(
        save_item(&store, OWNER, &rarer).await.unwrap_err().status(),
        409,
        "a published item keeps its rarity"
    );
    save_item(&store, OWNER, &item)
        .await
        .expect("the owner edits an item of a published collection");
    let Draft {
        collection: second,
        item: second_item,
        ..
    } = draft(&store).await;
    let preparation = store
        .prepare_publication(&granted::publisher(OWNER, second.id))
        .await
        .unwrap();
    store
        .begin_publication(&granted::publisher(OWNER, second.id), &preparation.revision)
        .await
        .unwrap();
    let failed_hash = format!("0x{}", "66".repeat(32));
    {
        let mut s = state.lock().unwrap();
        s["tx"]["hash"] = json!(failed_hash);
        s["tx"]["input"] = json!(publication_calldata(&preparation).unwrap());
        s["receipt"]["transactionHash"] = json!(failed_hash);
        s["receipt"]["status"] = json!("0x0");
    }
    assert_eq!(
        chain
            .verify(&store, &granted::publisher(OWNER, second.id), &failed_hash)
            .await
            .unwrap()
            .status,
        "reverted"
    );
    save_item(&store, OWNER, &second_item).await.unwrap();
    let preparation = store
        .prepare_publication(&granted::publisher(OWNER, second.id))
        .await
        .unwrap();
    assert_eq!(
        store
            .begin_publication(&granted::publisher(OWNER, second.id), &preparation.revision)
            .await
            .unwrap()
            .status,
        "prepared"
    );
    task.abort();
    cleanup(&store, &[id, second.id], &hashes_of(&[&item, &second_item])).await;
}

#[tokio::test]
async fn concurrent_edit_and_publication_cannot_both_accept_the_old_revision() {
    let Some(store) = scratch_component().await else {
        return;
    };
    let Draft {
        collection,
        mut item,
        ..
    } = draft(&store).await;
    let prepared = store
        .prepare_publication(&granted::publisher(OWNER, collection.id))
        .await
        .unwrap();
    item["name"] = json!("Changed while paying");
    let publisher = granted::publisher(OWNER, collection.id);
    let (begin, edit) = tokio::join!(
        store.begin_publication(&publisher, &prepared.revision),
        save_item(&store, OWNER, &item)
    );
    assert_ne!(
        begin.is_ok(),
        edit.is_ok(),
        "exactly one operation may accept the reviewed revision"
    );
    let saved = read_item(&store, item_id(&item)).await.unwrap();
    if begin.is_ok() {
        assert_eq!(saved["name"], "Hat");
        assert_eq!(
            store
                .prepare_publication(&granted::publisher(OWNER, collection.id))
                .await
                .unwrap()
                .revision,
            prepared.revision
        );
    } else {
        assert_eq!(saved["name"], "Changed while paying");
        assert!(
            store
                .publication_state(&granted::publisher(OWNER, collection.id))
                .await
                .unwrap()
                .is_none()
        );
    }
    cleanup(&store, &[collection.id], &hashes_of(&[&item])).await;
}

#[tokio::test]
async fn a_waiting_detach_rechecks_the_committed_publication_lock_and_an_upload_takes_none() {
    let Some(store) = scratch_component().await else {
        return;
    };
    let url = crate::db::scratch::url().await.unwrap();
    let mut created = Vec::new();
    let mut hashes = Vec::new();
    for detach in [true, false] {
        let Draft {
            collection,
            mut item,
            model,
        } = draft(&store).await;
        created.push(collection.id);
        hashes.extend(hashes_of(&[&item]));
        let item_id = item_id(&item);
        let original = read_item(&store, item_id).await.unwrap();
        let writer_pool = sqlx::postgres::PgPoolOptions::new()
            .max_connections(1)
            .connect(&url)
            .await
            .unwrap();
        let writer_pid: i32 = sqlx::query_scalar("SELECT pg_backend_pid()")
            .fetch_one(&writer_pool)
            .await
            .unwrap();
        let mut blocker = store.pool().begin().await.unwrap();
        sqlx::query("SELECT id FROM builder.collections WHERE id=$1 FOR UPDATE")
            .bind(collection.id)
            .fetch_all(&mut *blocker)
            .await
            .unwrap();
        sqlx::query("SELECT id FROM builder.items WHERE id=$1 FOR UPDATE")
            .bind(item_id)
            .fetch_all(&mut *blocker)
            .await
            .unwrap();
        sqlx::query("UPDATE builder.collections SET publication_pending=true WHERE id=$1")
            .bind(collection.id)
            .execute(&mut *blocker)
            .await
            .unwrap();
        let writer = ItemsComponent::new(writer_pool.clone(), store.blobs.clone());
        let attempt = tokio::spawn(async move {
            if detach {
                item["collection_id"] = json!(null);
                save_item(&writer, OWNER, &item).await.map(|_| ())
            } else {
                upload(&writer, OWNER, item_id, &[&model]).await
            }
        });
        assert!(
            crate::shared::test_wait::until(async || {
                sqlx::query_scalar("SELECT COALESCE(wait_event_type='Lock',false) FROM pg_stat_activity WHERE pid=$1")
                    .bind(writer_pid).fetch_one(store.pool()).await.unwrap()
            })
            .await,
            "writer must actually wait on the publication transaction"
        );
        blocker.commit().await.unwrap();
        assert_eq!(
            attempt.await.unwrap().is_err(),
            detach,
            "a waiting detach meets the committed publication lock; an upload of a named file takes none"
        );
        assert_eq!(read_item(&store, item_id).await.unwrap(), original);
        writer_pool.close().await;
    }
    cleanup(&store, &created, &hashes).await;
}

/// A wallet that speeds up or replaces the publication's transaction sends
/// the same reviewed call under a new hash. Once the Polygon RPC no longer
/// knows the recorded hash, the node records the new one and publishes the
/// collection from its receipt. While the recorded hash is still known, or
/// when the new transaction calls something else (a wallet's cancel), the
/// recorded hash stays.
#[tokio::test]
async fn a_replaced_publication_transaction_still_publishes_the_collection() {
    let Some(pool) = super::validator::own_pool().await else {
        return;
    };
    let rpc_state = Arc::new(Mutex::new(json!({"receipt":null, "finalized":"0x10",
        "canonical":BLOCK, "count":1})));
    let (url, task) = serve_rpc(rpc_state.clone()).await;
    let (app, state) = node_over(pool, |state| {
        state.settings.polygon_rpc_url = Some(url);
    })
    .await;
    let owner = super::validator::address(OWNER_KEY);
    rpc_state.lock().unwrap()["creator"] = json!(owner);
    let Draft { collection, .. } = draft_of(&state.items, &owner).await;
    let path = format!("/v1/collections/{}/publication", collection.id);
    let call = |method, body: Value| Call {
        method,
        key: Some(OWNER_KEY),
        body: body.to_string().into_bytes(),
        ..Call::get(path.clone())
    };
    let answer = |(status, body, _): (u16, String, Option<String>)| {
        let body: Value = serde_json::from_str(&body).unwrap();
        (status, body)
    };
    let (status, prepared) = answer(
        send(
            &app,
            &Call {
                key: Some(OWNER_KEY),
                ..Call::get(path.clone())
            },
        )
        .await,
    );
    assert_eq!(status, 200, "{prepared}");
    let prepared: PublicationPreparation =
        serde_json::from_value(prepared["data"].clone()).unwrap();
    let revision = json!({"revision": prepared.revision});
    assert_eq!(send(&app, &call("POST", revision.clone())).await.0, 200);
    assert_eq!(send(&app, &call("PATCH", revision)).await.0, 200);
    let sent = json!({"hash":HASH, "from":owner, "to":POLYGON.collection_manager, "value":"0x0",
        "input":publication_calldata(&prepared).unwrap(), "blockNumber":"0x10", "blockHash":BLOCK});
    rpc_state.lock().unwrap()["tx"] = sent.clone();
    let (status, recorded) = answer(send(&app, &call("PUT", json!({"tx_hash": HASH}))).await);
    assert_eq!(
        (status, recorded["data"]["status"].as_str()),
        (200, Some("submitted")),
        "{recorded}"
    );
    let mut faster = sent.clone();
    faster["hash"] = json!(REPLACEMENT);
    rpc_state.lock().unwrap()["replacement"] = faster.clone();
    let (status, refused) = answer(send(&app, &call("PUT", json!({"tx_hash": REPLACEMENT}))).await);
    assert_eq!(
        status, 409,
        "the recorded transaction is still known: {refused}"
    );
    let mut canceled = faster.clone();
    canceled["to"] = json!(owner);
    canceled["input"] = json!("0x");
    {
        let mut rpc = rpc_state.lock().unwrap();
        rpc["tx"] = Value::Null;
        rpc["replacement"] = canceled;
    }
    let (status, refused) = answer(send(&app, &call("PUT", json!({"tx_hash": REPLACEMENT}))).await);
    assert_eq!(
        status, 400,
        "a wallet's cancel publishes nothing: {refused}"
    );
    let recorded = |state: &crate::SharedState| {
        let pool = state.items.pool().clone();
        let id = collection.id;
        async move {
            sqlx::query_scalar::<_, Option<String>>(
                "SELECT tx_hash FROM builder.collection_publications WHERE collection_id = $1",
            )
            .bind(id)
            .fetch_one(&pool)
            .await
            .unwrap()
        }
    };
    assert_eq!(recorded(&state).await.as_deref(), Some(HASH));
    rpc_state.lock().unwrap()["replacement"] = faster;
    let (status, taken) = answer(send(&app, &call("PUT", json!({"tx_hash": REPLACEMENT}))).await);
    assert_eq!(
        (status, taken["data"]["status"].as_str()),
        (200, Some("submitted")),
        "{taken}"
    );
    assert_eq!(recorded(&state).await.as_deref(), Some(REPLACEMENT));
    rpc_state.lock().unwrap()["receipt"] = json!({"transactionHash":REPLACEMENT, "from":owner,
        "to":POLYGON.collection_manager, "status":"0x1", "blockNumber":"0x10", "blockHash":BLOCK,
        "logs":[{"address":POLYGON.collection_factory, "topics":[format!("{:#x}", keccak256("ProxyCreated(address,bytes32)")),
            format!("0x{:0>64}", &CONTRACT[2..])], "data":prepared.salt}]});
    let (status, published) =
        answer(send(&app, &call("PUT", json!({"tx_hash": REPLACEMENT}))).await);
    assert_eq!(
        (
            status,
            published["data"]["status"].as_str(),
            published["data"]["contract_address"].as_str()
        ),
        (200, Some("published"), Some(CONTRACT)),
        "{published}"
    );
    let pending: bool =
        sqlx::query_scalar("SELECT publication_pending FROM builder.collections WHERE id = $1")
            .bind(collection.id)
            .fetch_one(state.items.pool())
            .await
            .unwrap();
    assert!(!pending, "the collection stays locked");
    task.abort();
}
