//! Shared fixtures of the builder's tests: the process's
//! scratch database (`db::scratch`, skipped without
//! `CATALYST_TEST_DATABASE_URL`) with a throwaway blob store, an
//! explorer-style auth chain minted from a known key, and the row cleanup
//! every DB-backed test runs at its end (rows are uuid-scoped, the scratch
//! database is shared with the other lib tests).

use std::collections::BTreeMap;
use std::sync::Arc;

use alloy::signers::{Signer as _, local::PrivateKeySigner};
use uuid::Uuid;

use crate::items::ItemsComponent;
use crate::shared::signed_fetch::{
    AUTH_CHAIN_HEADER_PREFIX, AUTH_METADATA_HEADER, AUTH_TIMESTAMP_HEADER, build_legacy_payload,
    build_payload_v6,
};
use crate::storage::Storage;

/// `0x19e7e376e7c213b7e7e7e46cc70a5dd086daff2a`, the wallet of the old
/// fixtures' key.
pub const OWNER_KEY: &str = "0x1111111111111111111111111111111111111111111111111111111111111111";

/// Whether a request of this database waits for the blob store's lock
/// ([`crate::storage::gc::STORE_LOCK_KEY`]), held by a collector's unlink
/// batch.
pub async fn waiting_for_the_store(pool: &sqlx::PgPool) -> bool {
    sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM pg_locks l JOIN pg_database d ON d.oid = l.database \
         WHERE l.locktype = 'advisory' AND NOT l.granted AND d.datname = current_database() \
           AND l.objsubid = 1 AND ((l.classid::bigint << 32) | l.objid::bigint) = $1)",
    )
    .bind(crate::storage::gc::STORE_LOCK_KEY)
    .fetch_one(pool)
    .await
    .unwrap()
}

pub async fn temp_storage() -> Arc<Storage> {
    let dir = std::env::temp_dir().join(format!("catalyst-builder-test-{}", Uuid::new_v4()));
    tokio::fs::create_dir_all(&dir).await.unwrap();
    Arc::new(Storage::from_config(&dir).await.unwrap())
}

/// The process's scratch database with a fresh complete pass of the
/// Polygon collections copy, so derived chain state answers; `None` (skip)
/// without `CATALYST_TEST_DATABASE_URL`.
pub async fn scratch_pool(max_connections: u32) -> Option<sqlx::PgPool> {
    let pool = crate::db::scratch::pool(max_connections).await?;
    sqlx::query(
        "INSERT INTO shop.mirror_state (mirror, network, source_url, last_complete_pass_at, pass_started_at) \
         VALUES ('collections', 'matic', 'https://subgraph.example', timezone('UTC', now()), timezone('UTC', now())) \
         ON CONFLICT (mirror, network) DO UPDATE SET last_complete_pass_at = EXCLUDED.last_complete_pass_at, \
           pass_started_at = EXCLUDED.pass_started_at, cursor_id = NULL",
    )
    .execute(&pool)
    .await
    .unwrap();
    Some(pool)
}

/// Puts collection `id` on chain the way the shop's copy shows it: a
/// contract address on the builder row and its Polygon row in
/// `shop.collections`, created by the row's owner. Returns the contract for
/// [`off_chain`].
pub async fn on_chain(pool: &sqlx::PgPool, id: Uuid, approved: bool) -> String {
    let contract = format!("0x{}", Uuid::new_v4().simple().to_string()[..20].repeat(2));
    sqlx::query("UPDATE builder.collections SET contract_address = $2 WHERE id = $1")
        .bind(id)
        .bind(&contract)
        .execute(pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO shop.collections (network, contract_address, chain_id, name, creator, owner, urn, \
           is_completed, is_approved, is_editable, created_at, updated_at, reviewed_at, mirrored_at) \
         SELECT 'matic', $1, 137, 'On chain', eth_address, eth_address, 'urn:c', true, $2, true, 1, 1, 1, timezone('UTC', now()) \
         FROM builder.collections WHERE id = $3",
    )
    .bind(&contract)
    .bind(approved)
    .bind(id)
    .execute(pool)
    .await
    .unwrap();
    contract
}

/// A client-shaped `PUT /v1/collections/:id` body for `signer`: the
/// schema's required fields around `collection`, and `initialize` calldata
/// naming `signer` unless `collection` brings its own `data`.
pub fn collection_body(collection: serde_json::Value, signer: &str) -> serde_json::Value {
    let mut fields =
        serde_json::json!({ "eth_address": signer, "salt": null, "contract_address": null });
    let mut collection = collection.as_object().cloned().unwrap_or_default();
    let data = collection
        .remove("data")
        .unwrap_or_else(|| crate::collections::contract::initialize_calldata(signer).into());
    fields.as_object_mut().unwrap().extend(collection);
    serde_json::json!({ "collection": fields, "data": data })
}

/// Saves `collection` (its `id`, `name` and, for a third-party one, `urn`)
/// as `PUT /v1/collections/:id` saves it for `signer`, with `initialize`
/// calldata naming `signer` for a standard one, under a test grant.
pub async fn save_collection(
    store: &ItemsComponent,
    signer: &str,
    collection: serde_json::Value,
) -> Result<serde_json::Value, crate::http::ApiError> {
    use crate::collections::save;
    let request = save::parse(&collection_body(collection, signer))?;
    let provider = request.urn.as_deref().and_then(save::third_party_urn);
    let deployment = match provider {
        Some(_) => None,
        None => Some(save::standard_deployment(&store.network, &request, signer)?),
    };
    let id = Uuid::parse_str(&request.id).unwrap();
    store
        .save_collection(
            &crate::access::granted::edit(signer, id, provider.map(|(p, _)| p)),
            &request,
            deployment,
        )
        .await
}

/// `item` as a client sends it for `signer`: the keys the item schema
/// requires that `item` leaves out (an empty description, the signer, a
/// wearable's metrics and data, each representation's body shape and
/// overrides, no contents) and, for an item of a third-party collection
/// without one, a URN under that collection.
pub async fn item_body(
    store: &ItemsComponent,
    signer: &str,
    item: &serde_json::Value,
) -> serde_json::Value {
    let mut item = item.as_object().cloned().unwrap_or_default();
    let defaults = serde_json::json!({
        "description": "", "eth_address": signer, "type": "wearable", "contents": {},
        "metrics": { "meshes": 1, "bodies": 1, "materials": 1, "textures": 1, "triangles": 1, "entities": 1 },
    });
    for (key, value) in defaults.as_object().unwrap() {
        item.entry(key.clone()).or_insert(value.clone());
    }
    let data = item
        .entry("data")
        .or_insert(serde_json::json!({}))
        .as_object_mut()
        .unwrap();
    for (key, value) in [
        ("category", serde_json::json!("hat")),
        ("replaces", serde_json::json!([])),
        ("hides", serde_json::json!([])),
        ("tags", serde_json::json!([])),
        (
            "representations",
            serde_json::json!([{ "mainFile": "model.glb", "contents": ["model.glb"] }]),
        ),
    ] {
        data.entry(key).or_insert(value);
    }
    for representation in data["representations"].as_array_mut().unwrap() {
        let representation = representation.as_object_mut().unwrap();
        representation
            .entry("bodyShapes")
            .or_insert(serde_json::json!([
                "urn:decentraland:off-chain:base-avatars:BaseMale"
            ]));
        representation
            .entry("overrideHides")
            .or_insert(serde_json::json!([]));
        representation
            .entry("overrideReplaces")
            .or_insert(serde_json::json!([]));
    }
    if !item.contains_key("urn")
        && let Some(collection) = item
            .get("collection_id")
            .and_then(|c| c.as_str())
            .and_then(|c| Uuid::parse_str(c).ok())
    {
        let urn: Option<Option<String>> = sqlx::query_scalar(
            "SELECT urn_suffix FROM builder.collections WHERE id = $1 AND third_party_id IS NOT NULL",
        )
        .bind(collection)
        .fetch_optional(store.pool())
        .await
        .unwrap();
        if let Some(Some(urn)) = urn {
            let id = item["id"].as_str().unwrap().replace('-', "");
            item.insert("urn".into(), format!("{urn}:{id}").into());
        }
    }
    serde_json::json!({ "item": item })
}

/// Saves `item` ([`item_body`]) as `PUT /v1/items/:id` saves a body with an
/// id for `signer`, under a test grant.
pub async fn save_item(
    store: &ItemsComponent,
    signer: &str,
    item: &serde_json::Value,
) -> Result<crate::items::body::FullItemOut, crate::http::ApiError> {
    use crate::access::{SaveIntent, UrnField};
    use crate::items::urn::ThirdPartyItemUrn;
    let body = item_body(store, signer, item).await;
    let input = crate::items::schema::parse(body.to_string().as_bytes())?;
    let id = Uuid::parse_str(input.id.as_deref().unwrap()).unwrap();
    let collection = input
        .collection_id
        .clone()
        .flatten()
        .map(|c| Uuid::parse_str(&c).unwrap());
    let intent = SaveIntent {
        collection,
        rarity: input.rarity.clone().flatten(),
        urn: match &input.urn {
            None => UrnField::Absent,
            Some(None) => UrnField::Null,
            Some(Some(urn)) => {
                ThirdPartyItemUrn::parse(urn).map_or(UrnField::Other, UrnField::ThirdParty)
            }
        },
        id_given: true,
        path_id: Some(id.to_string()),
    };
    store
        .save_item(
            &crate::access::granted::save(signer, id, collection),
            input,
            &intent,
        )
        .await
}

/// [`save_item`]'s body as JSON.
pub async fn saved_item(
    store: &ItemsComponent,
    signer: &str,
    item: &serde_json::Value,
) -> serde_json::Value {
    serde_json::to_value(save_item(store, signer, item).await.unwrap()).unwrap()
}

/// Item `id` as its owner side reads it, as JSON.
pub async fn read_item(
    store: &ItemsComponent,
    id: Uuid,
) -> Result<serde_json::Value, crate::http::ApiError> {
    Ok(serde_json::to_value(store.item(&crate::access::granted::item(id)).await?).unwrap())
}

/// The contents map naming each `(file, bytes)` by its content hash.
pub fn contents_of(files: &[(&str, &[u8])]) -> serde_json::Value {
    files
        .iter()
        .map(|(file, bytes)| (file.to_string(), dcl_auth::hash_bytes_v1(bytes).into()))
        .collect::<serde_json::Map<_, _>>()
        .into()
}

/// Uploads each of `files` for item `id` as `signer`, under its content
/// hash; a file the item names only as its `video.mp4` is left as a video
/// upload leaves it (its bytes stored and recorded), whatever its bytes.
pub async fn upload(
    store: &ItemsComponent,
    signer: &str,
    id: Uuid,
    files: &[&[u8]],
) -> Result<(), crate::http::ApiError> {
    let videos: Vec<String> = sqlx::query_scalar(
        "SELECT hash FROM builder.item_contents WHERE item_id = $1 GROUP BY hash \
         HAVING bool_and(file = 'video.mp4')",
    )
    .bind(id)
    .fetch_all(store.pool())
    .await
    .unwrap();
    let mut others = Vec::new();
    for bytes in files {
        let hash = dcl_auth::hash_bytes_v1(bytes);
        let bytes = bytes::Bytes::copy_from_slice(bytes);
        if !videos.contains(&hash) {
            others.push((hash, bytes));
            continue;
        }
        let size = bytes.len() as i64;
        store.blobs.store(&hash, bytes).await.unwrap();
        sqlx::query(
            "INSERT INTO builder.uploaded_contents (hash, size, uploaded_by, video) VALUES ($1, $2, $3, true) \
             ON CONFLICT (hash, uploaded_by) DO UPDATE SET video = true",
        )
        .bind(&hash)
        .bind(size)
        .bind(signer)
        .execute(store.pool())
        .await
        .unwrap();
    }
    store
        .save_item_files(&crate::access::granted::change(signer, id), others)
        .await
}

/// Undoes [`on_chain`].
pub async fn off_chain(pool: &sqlx::PgPool, contract: &str) {
    sqlx::query("DELETE FROM shop.collections WHERE network = 'matic' AND contract_address = $1")
        .bind(contract)
        .execute(pool)
        .await
        .unwrap();
    sqlx::query(
        "UPDATE builder.collections SET contract_address = NULL WHERE contract_address = $1",
    )
    .bind(contract)
    .execute(pool)
    .await
    .unwrap();
}

/// `None` (skip) without `CATALYST_TEST_DATABASE_URL`.
pub async fn scratch_component() -> Option<ItemsComponent> {
    Some(ItemsComponent::new(
        scratch_pool(6).await?,
        temp_storage().await,
    ))
}

/// Deletes the test's collections (items, contents and publications cascade)
/// and the uploads it stored.
pub async fn cleanup(store: &ItemsComponent, collections: &[Uuid], hashes: &[String]) {
    sqlx::query("DELETE FROM builder.collections WHERE id = ANY($1)")
        .bind(collections)
        .execute(store.pool())
        .await
        .unwrap();
    if !hashes.is_empty() {
        sqlx::query("DELETE FROM builder.item_contents WHERE hash = ANY($1)")
            .bind(hashes)
            .execute(store.pool())
            .await
            .unwrap();
        sqlx::query("DELETE FROM builder.uploaded_contents WHERE hash = ANY($1)")
            .bind(hashes)
            .execute(store.pool())
            .await
            .unwrap();
    }
}

/// Bytes no other test hashes to: the old fixtures reused `b"model"`, which
/// two tests sharing one database would race on through `uploaded_contents`.
pub fn unique(label: &str) -> Vec<u8> {
    format!("{label}-{}", Uuid::new_v4()).into_bytes()
}

pub fn wallet(key: &str) -> PrivateKeySigner {
    key.parse().unwrap()
}

/// The two-link chain (SIGNER + ECDSA_SIGNED_ENTITY) an explorer signs over
/// `method:path:timestamp:metadata`, in the 6.x or the legacy payload shape,
/// as a header map.
pub async fn signed_map(
    key: &str,
    method: &str,
    path: &str,
    metadata: &str,
    legacy: bool,
) -> BTreeMap<String, String> {
    let timestamp = chrono::Utc::now().timestamp_millis().to_string();
    let payload = if legacy {
        build_legacy_payload(method, path, &timestamp, metadata)
    } else {
        build_payload_v6(method, path, &timestamp, metadata)
    };
    let wallet = wallet(key);
    let signature = wallet
        .sign_message(payload.as_bytes())
        .await
        .unwrap()
        .to_string();
    let mut map = BTreeMap::from([
        (AUTH_TIMESTAMP_HEADER.to_string(), timestamp),
        (AUTH_METADATA_HEADER.to_string(), metadata.to_string()),
    ]);
    map.insert(
        format!("{AUTH_CHAIN_HEADER_PREFIX}0"),
        serde_json::json!({ "type": "SIGNER", "payload": format!("{:#x}", wallet.address()), "signature": "" }).to_string(),
    );
    map.insert(
        format!("{AUTH_CHAIN_HEADER_PREFIX}1"),
        serde_json::json!({ "type": "ECDSA_SIGNED_ENTITY", "payload": payload, "signature": signature }).to_string(),
    );
    map
}

pub(crate) fn lazy_pool() -> sqlx::PgPool {
    sqlx::postgres::PgPoolOptions::new()
        .acquire_timeout(std::time::Duration::from_millis(250))
        .connect_lazy("postgres://nobody:nothing@127.0.0.1:9/none")
        .expect("lazy pool from static url")
}

async fn state_over(
    pool: sqlx::PgPool,
    market: Option<Arc<crate::chain_index::ChainIndex>>,
) -> crate::SharedState {
    state_over_with(pool, market, crate::Settings::default()).await
}

pub(crate) async fn state_over_with(
    pool: sqlx::PgPool,
    market: Option<Arc<crate::chain_index::ChainIndex>>,
    settings: crate::Settings,
) -> crate::SharedState {
    Arc::new(
        crate::State::new(
            pool,
            temp_storage().await,
            Arc::default(),
            dcl_http::Outbound::for_tests(),
            market,
            None,
            settings,
        )
        .unwrap(),
    )
}

/// [`state_without_db`] with `settings`.
pub async fn state_with_settings(settings: crate::Settings) -> crate::SharedState {
    state_over_with(lazy_pool(), None, settings).await
}

/// A builder state over a pool that never connects: for the handler tests
/// that answer before touching the database.
pub async fn state_without_db(
    market: Option<Arc<crate::chain_index::ChainIndex>>,
) -> crate::SharedState {
    state_over(lazy_pool(), market).await
}

/// A loopback Polygon collections subgraph ([`committee_index`]): to a
/// query of `accounts` it answers the committee as The Graph pages it
/// (`first`, `skip`, ordered by id; `with` lists its members, `down` answers
/// a GraphQL error), to a query of `rarities` the rarities it holds (`None`
/// answers a GraphQL error), and counts the reads of each.
#[derive(Clone, Default)]
pub struct FakeCommittee {
    members: Arc<std::sync::Mutex<Option<Vec<String>>>>,
    rarities: Arc<std::sync::Mutex<Option<Vec<serde_json::Value>>>>,
    rarity_hits: Arc<std::sync::atomic::AtomicUsize>,
    committee_hits: Arc<std::sync::atomic::AtomicUsize>,
}

impl FakeCommittee {
    pub fn with(members: &[&str]) -> Self {
        let fake = Self::default();
        fake.set(Some(members));
        fake
    }

    pub fn down() -> Self {
        Self::default()
    }

    /// Changes the committee the subgraph answers.
    pub fn set(&self, members: Option<&[&str]>) {
        *self.members.lock().unwrap() = members.map(|m| m.iter().map(|a| a.to_string()).collect());
    }

    /// Drops `member` from the committee the subgraph answers.
    pub fn remove(&self, member: &str) {
        if let Some(members) = self.members.lock().unwrap().as_mut() {
            members.retain(|m| m != member);
        }
    }

    /// The `rarities` the subgraph answers, each `(name, usd price, max
    /// supply)`; `None` answers a GraphQL error.
    pub fn set_rarities(&self, rarities: Option<&[(&str, &str, &str)]>) {
        *self.rarities.lock().unwrap() = rarities.map(|r| {
            r.iter()
                .map(|(name, price, max_supply)| {
                    serde_json::json!({ "id": name, "name": name, "price": price, "maxSupply": max_supply })
                })
                .collect()
        });
    }

    pub fn rarity_hits(&self) -> usize {
        self.rarity_hits.load(std::sync::atomic::Ordering::SeqCst)
    }

    pub fn committee_hits(&self) -> usize {
        self.committee_hits
            .load(std::sync::atomic::Ordering::SeqCst)
    }

    async fn answer(
        axum::extract::State(fake): axum::extract::State<Self>,
        axum::Json(body): axum::Json<serde_json::Value>,
    ) -> axum::Json<serde_json::Value> {
        if body["query"]
            .as_str()
            .is_some_and(|q| q.contains("accounts"))
        {
            fake.committee_hits
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            let Some(mut members) = fake.members.lock().unwrap().clone() else {
                return axum::Json(
                    serde_json::json!({ "errors": [{ "message": "indexing error" }] }),
                );
            };
            members.sort();
            let skip = body["variables"]["skip"].as_u64().unwrap_or(0) as usize;
            let first = body["variables"]["first"].as_u64().unwrap_or(100) as usize;
            let page: Vec<_> = members
                .into_iter()
                .skip(skip)
                .take(first)
                .map(|m| serde_json::json!({ "id": m, "address": m }))
                .collect();
            return axum::Json(serde_json::json!({ "data": { "accounts": page } }));
        }
        fake.rarity_hits
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        axum::Json(match fake.rarities.lock().unwrap().clone() {
            Some(rarities) => serde_json::json!({ "data": { "rarities": rarities } }),
            None => serde_json::json!({ "errors": [{ "message": "indexing error" }] }),
        })
    }
}

/// A chain index whose Polygon collections subgraph is `fake` and whose
/// third-party registry lists no provider, so a third-party collection's
/// managers are its virtual third party's.
pub async fn committee_index(fake: &FakeCommittee) -> Arc<crate::chain_index::ChainIndex> {
    registry_index(fake, &FakeRegistry::up()).await
}

/// A loopback third-party registry subgraph that answers the providers it
/// holds as The Graph does for the node's walk (`id_gt`, `first`, ordered by
/// id); `down` answers a GraphQL error. Counts its reads.
#[derive(Clone, Default)]
pub struct FakeRegistry {
    providers: Arc<std::sync::Mutex<Option<BTreeMap<String, serde_json::Value>>>>,
    receipts: Arc<std::sync::Mutex<std::collections::BTreeSet<String>>>,
    receipts_down: Arc<std::sync::atomic::AtomicBool>,
    hits: Arc<std::sync::atomic::AtomicUsize>,
}

impl FakeRegistry {
    pub fn up() -> Self {
        let fake = Self::default();
        *fake.providers.lock().unwrap() = Some(BTreeMap::new());
        fake
    }

    /// Lists provider `id` with `managers`, approved or not.
    pub fn put(&self, id: &str, managers: &[&str], approved: bool, programmatic: bool) {
        self.providers
            .lock()
            .unwrap()
            .get_or_insert_default()
            .insert(
                id.to_string(),
                serde_json::json!({
                    "id": id,
                    "root": format!("0x{}", "ab".repeat(32)),
                    "managers": managers,
                    "maxItems": "10",
                    "isApproved": approved,
                    "isProgrammatic": programmatic,
                    "metadata": { "type": "third_party_v2", "thirdParty": {
                        "name": "Registered", "description": "", "contracts": []
                    } }
                }),
            );
    }

    /// Drops provider `id`.
    pub fn remove(&self, id: &str) {
        if let Some(providers) = self.providers.lock().unwrap().as_mut() {
            providers.remove(id);
        }
    }

    /// Changes one field of provider `id`.
    pub fn set(&self, id: &str, field: &str, value: serde_json::Value) {
        if let Some(provider) = self
            .providers
            .lock()
            .unwrap()
            .as_mut()
            .and_then(|p| p.get_mut(id))
        {
            provider[field] = value;
        }
    }

    /// Answers a GraphQL error (`true`) or the providers it holds.
    pub fn fail(&self, down: bool) {
        let mut providers = self.providers.lock().unwrap();
        match (down, providers.is_some()) {
            (true, true) => *providers = None,
            (false, false) => *providers = Some(BTreeMap::new()),
            _ => {}
        }
    }

    pub fn hits(&self) -> usize {
        self.hits.load(std::sync::atomic::Ordering::SeqCst)
    }

    /// Holds the receipt `id`: the registry consumed that cheque.
    pub fn consume(&self, id: &str) {
        self.receipts.lock().unwrap().insert(id.to_string());
    }

    /// Answers a query of receipts with a GraphQL error (`true`) while the
    /// providers still answer.
    pub fn fail_receipts(&self, down: bool) {
        self.receipts_down
            .store(down, std::sync::atomic::Ordering::SeqCst);
    }

    async fn answer(
        axum::extract::State(fake): axum::extract::State<Self>,
        axum::Json(body): axum::Json<serde_json::Value>,
    ) -> axum::Json<serde_json::Value> {
        fake.hits.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let Some(providers) = fake.providers.lock().unwrap().clone() else {
            return axum::Json(serde_json::json!({ "errors": [{ "message": "indexing error" }] }));
        };
        if body["query"]
            .as_str()
            .is_some_and(|q| q.contains("receipts"))
        {
            if fake.receipts_down.load(std::sync::atomic::Ordering::SeqCst) {
                return axum::Json(
                    serde_json::json!({ "errors": [{ "message": "indexing error" }] }),
                );
            }
            let id = body["variables"]["id"].as_str().unwrap_or_default();
            let held: Vec<serde_json::Value> = fake
                .receipts
                .lock()
                .unwrap()
                .iter()
                .filter(|r| *r == id)
                .map(|r| serde_json::json!({ "id": r }))
                .collect();
            return axum::Json(serde_json::json!({ "data": { "receipts": held } }));
        }
        let from = body["variables"]["idFrom"]
            .as_str()
            .unwrap_or("")
            .to_string();
        let first = body["variables"]["first"].as_u64().unwrap_or(100) as usize;
        let page: Vec<_> = providers
            .into_iter()
            .filter(|(id, _)| *id > from)
            .take(first)
            .map(|(_, p)| p)
            .collect();
        axum::Json(serde_json::json!({ "data": { "thirdParties": page } }))
    }
}

/// A chain index whose Polygon collections subgraph is `committee` and whose
/// third-party registry is `registry`.
pub async fn registry_index(
    committee: &FakeCommittee,
    registry: &FakeRegistry,
) -> Arc<crate::chain_index::ChainIndex> {
    let app = axum::Router::new()
        .route(
            "/subgraph",
            axum::routing::post(FakeCommittee::answer).with_state(committee.clone()),
        )
        .route(
            "/registry",
            axum::routing::post(FakeRegistry::answer).with_state(registry.clone()),
        );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let index = market_without_db(crate::chain_index::IndexSettings {
        matic_collections_subgraph_url: Some(format!("http://{addr}/subgraph")),
        third_party_registry_subgraph_url: Some(format!("http://{addr}/registry")),
        ..crate::chain_index::IndexSettings::for_tests()
    });
    index.allow_loopback_for_tests();
    index
}

/// A chain index whose only source is a third-party registry that lists
/// no provider: a third-party collection's managers are its virtual third
/// party's.
pub async fn registry_only_index() -> Arc<crate::chain_index::ChainIndex> {
    let app = axum::Router::new()
        .route("/registry", axum::routing::post(FakeRegistry::answer))
        .with_state(FakeRegistry::up());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let index = market_without_db(crate::chain_index::IndexSettings {
        third_party_registry_subgraph_url: Some(format!("http://{addr}/registry")),
        ..crate::chain_index::IndexSettings::for_tests()
    });
    index.allow_loopback_for_tests();
    index
}

/// A chain index with `settings` (no subgraph configured by default).
pub fn market_without_db(
    settings: crate::chain_index::IndexSettings,
) -> Arc<crate::chain_index::ChainIndex> {
    Arc::new(crate::chain_index::ChainIndex::new(
        dcl_http::Outbound::for_tests(),
        settings,
    ))
}

alloy_sol_types::sol! {
    struct OracleRarity { string name; uint256 maxSupply; uint256 price; }
    function getRarityByName(string rarity) external view returns (OracleRarity);
}

/// A loopback JSON-RPC of a Polygon chain (mainnet unless [`FakeOracle::on`])
/// that answers its chain id and `eth_call`s of `getRarityByName` on that
/// chain's rarities-with-oracle contract with the MANA price it holds for
/// the name (an unknown name reverts, `None` answers HTTP 502),
/// and records the names it was asked.
#[derive(Clone, Default)]
pub struct FakeOracle {
    prices: Arc<std::sync::Mutex<Option<BTreeMap<String, String>>>>,
    asked: Arc<std::sync::Mutex<Vec<String>>>,
    deployed: Arc<std::sync::Mutex<BTreeMap<String, u64>>>,
    code_reads: Arc<std::sync::atomic::AtomicUsize>,
    down: Arc<std::sync::atomic::AtomicBool>,
    other_chain: Arc<std::sync::atomic::AtomicBool>,
    rarity_calls: Arc<RarityCalls>,
    network: crate::network::Network,
}

#[derive(Default)]
struct RarityCalls {
    open: std::sync::atomic::AtomicUsize,
    most_open: std::sync::atomic::AtomicUsize,
    hold_until: std::sync::atomic::AtomicUsize,
    opened: tokio::sync::Notify,
}

impl FakeOracle {
    /// Holds each rarity answer, for at most two seconds, until `open`
    /// rarity calls are open at once.
    pub fn hold_rarities_until(&self, open: usize) {
        self.rarity_calls
            .hold_until
            .store(open, std::sync::atomic::Ordering::SeqCst);
    }

    /// The most rarity calls that were open at once.
    pub fn most_rarities_at_once(&self) -> usize {
        self.rarity_calls
            .most_open
            .load(std::sync::atomic::Ordering::SeqCst)
    }

    pub fn with(prices: &[(&str, &str)]) -> Self {
        let fake = Self::default();
        fake.set(Some(prices));
        fake
    }

    pub fn set(&self, prices: Option<&[(&str, &str)]>) {
        *self.prices.lock().unwrap() = prices.map(|p| {
            p.iter()
                .map(|(name, price)| (name.to_string(), price.to_string()))
                .collect()
        });
    }

    pub fn asked(&self) -> Vec<String> {
        self.asked.lock().unwrap().clone()
    }

    /// A collection contract at `contract` holding `items` items: its code
    /// answers `eth_getCode` and its `itemsCount()` answers `items`.
    pub fn deploy(&self, contract: &str, items: u64) {
        self.deployed
            .lock()
            .unwrap()
            .insert(contract.to_ascii_lowercase(), items);
    }

    /// Every request answers HTTP 502 while `down`.
    pub fn set_down(&self, down: bool) {
        self.down.store(down, std::sync::atomic::Ordering::SeqCst);
    }

    /// The fake of `network`'s chain, its chain id and oracle.
    pub fn on(mut self, network: crate::network::Network) -> Self {
        self.network = network;
        self
    }

    /// Answers another network's chain id (Ethereum's) while `other`.
    pub fn set_other_chain(&self, other: bool) {
        self.other_chain
            .store(other, std::sync::atomic::Ordering::SeqCst);
    }

    /// How many `eth_getCode` reads the fake answered.
    pub fn code_reads(&self) -> usize {
        self.code_reads.load(std::sync::atomic::Ordering::SeqCst)
    }

    async fn answer(
        axum::extract::State(fake): axum::extract::State<Self>,
        axum::Json(body): axum::Json<serde_json::Value>,
    ) -> axum::response::Response {
        use alloy_sol_types::SolCall;
        use axum::response::IntoResponse;
        if fake.down.load(std::sync::atomic::Ordering::SeqCst) {
            return axum::http::StatusCode::BAD_GATEWAY.into_response();
        }
        let id = body["id"].clone();
        let call = &body["params"][0];
        let target = call["to"].as_str().map(str::to_ascii_lowercase);
        let deployed = |address: Option<String>| {
            address.and_then(|a| fake.deployed.lock().unwrap().get(&a).copied())
        };
        match body["method"].as_str().unwrap_or_default() {
            "eth_chainId" => {
                let chain = if fake.other_chain.load(std::sync::atomic::Ordering::SeqCst) {
                    "0x1".to_string()
                } else {
                    format!("{:#x}", fake.network.chain_id)
                };
                return axum::Json(
                    serde_json::json!({ "jsonrpc": "2.0", "id": id, "result": chain }),
                )
                .into_response();
            }
            "eth_getCode" => {
                fake.code_reads
                    .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                let code = match deployed(body["params"][0].as_str().map(str::to_ascii_lowercase)) {
                    Some(_) => "0x6080",
                    None => "0x",
                };
                return axum::Json(
                    serde_json::json!({ "jsonrpc": "2.0", "id": id, "result": code }),
                )
                .into_response();
            }
            "eth_call"
                if call["data"].as_str().is_some_and(|d| {
                    d.eq_ignore_ascii_case(&alloy_primitives::hex::encode_prefixed(
                        crate::publication_chain::itemsCountCall::SELECTOR,
                    ))
                }) =>
            {
                let answer = match deployed(target) {
                    Some(items) => serde_json::json!({ "jsonrpc": "2.0", "id": id,
                        "result": format!("0x{items:064x}") }),
                    None => serde_json::json!({ "jsonrpc": "2.0", "id": id, "result": "0x" }),
                };
                return axum::Json(answer).into_response();
            }
            _ => {}
        }
        let Some(prices) = fake.prices.lock().unwrap().clone() else {
            return axum::http::StatusCode::BAD_GATEWAY.into_response();
        };
        let data = call["data"].as_str().unwrap_or_default();
        let decoded = alloy_primitives::hex::decode(data)
            .ok()
            .and_then(|bytes| getRarityByNameCall::abi_decode(&bytes).ok());
        let answer = match decoded {
            Some(asked)
                if body["method"] == "eth_call"
                    && call["to"].as_str().map(str::to_ascii_lowercase).as_deref()
                        == Some(fake.network.rarities_with_oracle) =>
            {
                use std::sync::atomic::Ordering::SeqCst;
                let calls = &fake.rarity_calls;
                let open = calls.open.fetch_add(1, SeqCst) + 1;
                calls.most_open.fetch_max(open, SeqCst);
                calls.opened.notify_waiters();
                let _ = tokio::time::timeout(std::time::Duration::from_secs(2), async {
                    loop {
                        let opened = calls.opened.notified();
                        tokio::pin!(opened);
                        opened.as_mut().enable();
                        if calls.open.load(SeqCst) >= calls.hold_until.load(SeqCst) {
                            break;
                        }
                        opened.await;
                    }
                })
                .await;
                calls.open.fetch_sub(1, SeqCst);
                fake.asked.lock().unwrap().push(asked.rarity.clone());
                match prices.get(&asked.rarity) {
                    Some(price) => {
                        serde_json::json!({ "jsonrpc": "2.0", "id": id, "result": alloy_primitives::hex::encode_prefixed(
                        getRarityByNameCall::abi_encode_returns(&OracleRarity {
                            name: asked.rarity.clone(),
                            maxSupply: alloy_primitives::U256::from(1u8),
                            price: price.parse().unwrap(),
                        }),
                    ) })
                    }
                    None => {
                        serde_json::json!({ "jsonrpc": "2.0", "id": id, "error": { "code": 3, "message": "execution reverted" } })
                    }
                }
            }
            _ => {
                serde_json::json!({ "jsonrpc": "2.0", "id": id, "error": { "code": -32601, "message": "not faked" } })
            }
        };
        axum::Json(answer).into_response()
    }

    /// The fake's URL, serving from now on.
    pub async fn url(&self) -> String {
        let app = axum::Router::new()
            .route("/rpc", axum::routing::post(Self::answer))
            .with_state(self.clone());
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        format!("http://{addr}/rpc")
    }
}
