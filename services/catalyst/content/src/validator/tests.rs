//! The live [`Sources`] behind the rules: the public [`validate`] over a
//! signed entity and a real blob store, and every access source against a
//! stand-in on the loopback (the marketplace and collections subgraphs, the
//! L2 blocks and third-party registry subgraphs), with the old error strings
//! compared byte for byte and every missing or failing source answering
//! `Unavailable`. The escrow overlay runs on the process's scratch database
//! and the LAND index on a database of its own (its cursor row is global);
//! both skip without `CATALYST_TEST_DATABASE_URL`.

use std::collections::HashMap;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::sync::Arc;
use std::time::Duration;

use axum::extract::State;
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{Json, Router};
use bytes::Bytes;
use serde_json::{Value, json};
use sqlx::PgPool;
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use tower::ServiceExt;

use super::checker::{BlockchainChecker, validate_item_access, validate_profile_access};
use super::types::OwnedDeployment;
use super::*;
use crate::auth::hash_bytes_v1;
use crate::http::Outbound;
use crate::land_authz::events::AuthzEvent;
use crate::land_authz::indexer;
use crate::tests::support::require_test_db;

const OWNER: &str = "0xaaaa000000000000000000000000000000000001";
const OPERATOR: &str = "0xbbbb000000000000000000000000000000000002";
const STRANGER: &str = "0xcccc000000000000000000000000000000000003";
const STORED_HASH: &str = "bafkreihdwdcefgh4dqkjv67uzcmw7ojee6xedzdetojuzjevtenosa7776";

fn lazy_pool() -> PgPool {
    PgPoolOptions::new()
        .acquire_timeout(Duration::from_millis(500))
        .connect_lazy("postgres://nobody@127.0.0.1:1/nowhere")
        .unwrap()
}

async fn temp_storage(tag: &str) -> (Arc<Storage>, std::path::PathBuf) {
    let tmp = std::env::temp_dir().join(format!(
        "catalyst-validator-{tag}-{}-{}",
        std::process::id(),
        uuid::Uuid::new_v4().simple()
    ));
    let storage = Storage::from_config(&tmp).await.unwrap();
    storage
        .store(STORED_HASH, Bytes::from_static(b"content"))
        .await
        .unwrap();
    (Arc::new(storage), tmp)
}

type Answer = Arc<dyn Fn(&str, &Value) -> Result<Value, u16> + Send + Sync>;

/// A GraphQL endpoint on the loopback: `answer(query, variables)` is the
/// `data`, or the HTTP status to fail with.
async fn serve_graph(
    answer: impl Fn(&str, &Value) -> Result<Value, u16> + Send + Sync + 'static,
) -> String {
    async fn handle(
        State(answer): State<Answer>,
        Json(body): Json<Value>,
    ) -> axum::response::Response {
        let query = body["query"].as_str().unwrap_or_default().to_string();
        match answer(&query, &body["variables"]) {
            Ok(data) => Json(json!({ "data": data })).into_response(),
            Err(status) => axum::http::StatusCode::from_u16(status)
                .unwrap()
                .into_response(),
        }
    }
    let answer: Answer = Arc::new(answer);
    let app = Router::new().route("/", post(handle)).with_state(answer);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    format!("http://{addr}/")
}

#[derive(Default)]
struct Readers {
    eth: Option<String>,
    matic: Option<String>,
    marketplace: Option<String>,
}

fn market_over(readers: Readers) -> Arc<ChainIndex> {
    let state = Arc::new(ChainIndex::new(
        Outbound::for_tests(),
        crate::chain_index::IndexSettings {
            eth_collections_subgraph_url: readers.eth,
            matic_collections_subgraph_url: readers.matic,
            marketplace_subgraph_url: readers.marketplace,
            ..crate::chain_index::IndexSettings::for_tests()
        },
    ));
    state.allow_loopback_for_tests();
    state
}

fn land_over(pool: PgPool, market: Option<Arc<ChainIndex>>) -> Arc<LandAuthzState> {
    Arc::new(LandAuthzState::new(
        pool,
        Outbound::for_tests(),
        crate::land_authz::Settings::for_tests(),
        market,
    ))
}

fn sources(
    pool: PgPool,
    storage: Arc<Storage>,
    land: Option<Arc<LandAuthzState>>,
    market: Option<Arc<ChainIndex>>,
    settings: Settings,
) -> Sources {
    Sources::new(
        pool,
        storage,
        Outbound::for_tests(),
        None,
        land,
        market,
        settings,
    )
}

/// The marketplace subgraph's `parcelsAt` answer for `(x, y, owner)` rows.
fn parcels_answer(
    parcels: &'static [(i32, i32, &'static str)],
) -> impl Fn(&str, &Value) -> Result<Value, u16> + Send + Sync + 'static {
    move |query, _| {
        assert!(query.contains("parcelsAt"), "unexpected query {query}");
        Ok(json!({ "nfts": parcels.iter().map(|(x, y, owner)| json!({
            "id": format!("0xland-{x}-{y}"), "searchParcelX": x.to_string(), "searchParcelY": y.to_string(),
            "owner": { "id": owner }, "parcel": { "estate": null }
        })).collect::<Vec<_>>() }))
    }
}

/// An entity file signed by a fixed key: the entity, its files (the entity
/// file under its id, `uploads` under their hashes) and the auth chain.
async fn signed(
    entity: Value,
    uploads: &[Vec<u8>],
) -> (
    Entity,
    HashMap<String, Vec<u8>>,
    crate::auth::AuthChain,
    String,
) {
    use alloy::signers::{Signer, local::PrivateKeySigner};
    let bytes = serde_json::to_vec(&entity).unwrap();
    let id = hash_bytes_v1(&bytes);
    let key: PrivateKeySigner = "ac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80"
        .parse()
        .unwrap();
    let signature = key.sign_message(id.as_bytes()).await.unwrap().to_string();
    let signer = format!("{:#x}", key.address());
    let chain = serde_json::from_value(json!([
        {"type": "SIGNER", "payload": signer},
        {"type": "ECDSA_SIGNED_ENTITY", "payload": id, "signature": signature}
    ]))
    .unwrap();
    let parsed = entity_parser::parse_entity_from_bytes(&bytes, &id).unwrap();
    let mut files = HashMap::from([(id.clone(), bytes)]);
    for upload in uploads {
        files.insert(hash_bytes_v1(upload), upload.clone());
    }
    (parsed, files, chain, signer)
}

fn scene(parcels: &[&str], model: &[u8]) -> Value {
    json!({
        "version": "v3",
        "type": "scene",
        "pointers": parcels,
        "timestamp": chrono::Utc::now().timestamp_millis(),
        "content": [{"file": "model.glb", "hash": hash_bytes_v1(model)}],
        "metadata": {"scene": {"base": parcels[0], "parcels": parcels}}
    })
}

fn rejected(messages: &[&str]) -> Result<(), Vec<ValidationError>> {
    Err(messages
        .iter()
        .map(|m| ValidationError::Rejected(m.to_string()))
        .collect())
}

fn unavailable(messages: &[&str]) -> Result<(), Vec<ValidationError>> {
    Err(messages
        .iter()
        .map(|m| ValidationError::Unavailable(m.to_string()))
        .collect())
}

#[tokio::test]
async fn a_signed_scene_validates_end_to_end_and_its_land_leg_decides() {
    let (storage, tmp) = temp_storage("scene").await;
    let signer = "0xf39fd6e51aad88f6f4ce6ab8827279cfffb92266";
    let graph = serve_graph(parcels_answer(&[
        (5, 5, "0xf39fd6e51aad88f6f4ce6ab8827279cfffb92266"),
        (5, 6, STRANGER),
    ]))
    .await;
    let market = market_over(Readers {
        marketplace: Some(graph),
        ..Default::default()
    });
    let with_land = sources(
        lazy_pool(),
        storage.clone(),
        Some(land_over(lazy_pool(), Some(market.clone()))),
        Some(market),
        Settings::default(),
    );
    let model = b"glb bytes".to_vec();

    let (entity, files, chain, deployer) =
        signed(scene(&["5,5"], &model), std::slice::from_ref(&model)).await;
    assert_eq!(deployer, signer);
    let ctx = Context {
        sources: &with_land,
        auth_chain: &chain,
    };
    assert_eq!(
        validate(&entity, &files, &ctx).await,
        Ok(()),
        "the owner's signed scene with its file at hand"
    );

    for (parcel, message) in [
        (
            "5,6",
            "The provided Eth Address does not have access to the following parcel: (5,6)",
        ),
        (
            "9,9",
            "The provided Eth Address does not have access to the following parcel: (9,9)",
        ),
    ] {
        let (entity, files, chain, _) =
            signed(scene(&[parcel], &model), std::slice::from_ref(&model)).await;
        let ctx = Context {
            sources: &with_land,
            auth_chain: &chain,
        };
        assert_eq!(
            validate(&entity, &files, &ctx).await,
            rejected(&[message]),
            "{parcel}"
        );
    }

    let (entity, files, chain, _) = signed(scene(&["5,5"], &model), &[]).await;
    let ctx = Context {
        sources: &with_land,
        auth_chain: &chain,
    };
    assert_eq!(
        validate(&entity, &files, &ctx).await,
        rejected(&[&format!(
            "Couldn't fetch content file with hash: {}",
            hash_bytes_v1(&model)
        )]),
        "a file neither uploaded nor stored fails the size step, which runs before the cross-check"
    );

    let referencing_stored = json!({
        "version": "v3", "type": "scene", "pointers": ["5,5"], "timestamp": chrono::Utc::now().timestamp_millis(),
        "content": [{"file": "stored.bin", "hash": STORED_HASH}],
        "metadata": {"scene": {"base": "5,5", "parcels": ["5,5"]}}
    });
    let (entity, files, chain, _) = signed(referencing_stored.clone(), &[]).await;
    let ctx = Context {
        sources: &with_land,
        auth_chain: &chain,
    };
    assert_eq!(
        validate(&entity, &files, &ctx).await,
        Ok(()),
        "a file this node already stores need not be uploaded"
    );
    let (entity, files, chain, _) = signed(referencing_stored, std::slice::from_ref(&model)).await;
    let ctx = Context {
        sources: &with_land,
        auth_chain: &chain,
    };
    assert_eq!(
        validate(&entity, &files, &ctx).await,
        rejected(&[&format!(
            "This hash was uploaded but is not referenced in the entity: {}",
            hash_bytes_v1(&model)
        )])
    );

    let without_land = sources(lazy_pool(), storage, None, None, Settings::default());
    let (entity, files, chain, _) =
        signed(scene(&["5,5"], &model), std::slice::from_ref(&model)).await;
    let ctx = Context {
        sources: &without_land,
        auth_chain: &chain,
    };
    let verdict = validate(&entity, &files, &ctx).await;
    assert_eq!(
        verdict,
        unavailable(&[
            "Error checking parcel access: no LAND rights source: the land_authz state is not attached"
        ])
    );
    assert!(matches!(
        into_deploy_failure(verdict.unwrap_err()),
        super::super::write_deployer::DeployFailure::Unavailable(_)
    ));

    let ignoring = sources(
        lazy_pool(),
        without_land.storage.clone(),
        None,
        None,
        Settings {
            ignore_blockchain_access: true,
            ..Settings::default()
        },
    );
    let ctx = Context {
        sources: &ignoring,
        auth_chain: &chain,
    };
    assert_eq!(
        validate(&entity, &files, &ctx).await,
        Ok(()),
        "IGNORE_BLOCKCHAIN_ACCESS_CHECKS skips the LAND leg"
    );
    let _ = std::fs::remove_dir_all(&tmp);
}

#[tokio::test]
async fn the_structure_signature_and_size_steps_answer_the_old_sentences() {
    let (storage, tmp) = temp_storage("steps").await;
    let s = sources(
        lazy_pool(),
        storage,
        None,
        None,
        Settings {
            ignore_blockchain_access: true,
            ..Settings::default()
        },
    );
    let model = b"glb".to_vec();

    let (mut entity, files, chain, _) =
        signed(scene(&["1,1"], &model), std::slice::from_ref(&model)).await;
    let ctx = Context {
        sources: &s,
        auth_chain: &chain,
    };
    entity.pointers = vec!["1,1".into(), "1,1".into()];
    assert_eq!(
        validate(&entity, &files, &ctx).await,
        rejected(&["There are repeated pointers in your request."])
    );
    entity.pointers.clear();
    assert_eq!(
        validate(&entity, &files, &ctx).await,
        rejected(&["The entity needs to be pointed by one or more pointers."])
    );
    entity.pointers = vec!["1,1".into()];
    entity.version = "v2".into();
    assert_eq!(
        validate(&entity, &files, &ctx).await,
        rejected(&[
            "Only entities v3 are allowed after the ADR-45. Check http://adr.decentraland.org/adr/ADR-45 for more information"
        ])
    );
    entity.version = "v3".into();

    let (other, _, other_chain, _) = signed(scene(&["2,2"], &model), &[]).await;
    let forged = Context {
        sources: &s,
        auth_chain: &other_chain,
    };
    let verdict = validate(&entity, &files, &forged).await.unwrap_err();
    assert!(
        matches!(&verdict[..], [ValidationError::Rejected(m)] if m.starts_with("The signature is invalid. ")),
        "a chain signing another entity ({}) is the depositor's: {verdict:?}",
        other.id
    );

    let big = vec![0u8; 16 * 1024 * 1024];
    let (entity, files, chain, _) = signed(scene(&["1,1"], &big), std::slice::from_ref(&big)).await;
    let ctx = Context {
        sources: &s,
        auth_chain: &chain,
    };
    assert_eq!(
        validate(&entity, &files, &ctx).await,
        rejected(&[
            "The deployment is too big. The maximum allowed size per pointer is 15 MB for scene. You can upload up to 15728640 bytes but you tried to upload 16777216."
        ])
    );

    let (entity, files, chain, _) = signed(
        json!({
            "version": "v3", "type": "scene", "pointers": ["1,1"], "timestamp": chrono::Utc::now().timestamp_millis(),
            "content": [{"file": "model.glb", "hash": hash_bytes_v1(&model)}],
            "metadata": {"scene": {"base": "1,1", "parcels": ["1,1"]}, "worldConfiguration": {"name": "x.dcl.eth"}}
        }),
        std::slice::from_ref(&model),
    )
    .await;
    let ctx = Context {
        sources: &s,
        auth_chain: &chain,
    };
    assert_eq!(
        validate(&entity, &files, &ctx).await,
        rejected(&[
            "The scene.json contains a worldConfiguration section, which is not allowed for Genesis City scenes (see ADR-173: http://adr.decentraland.org/adr/ADR-173). Please remove it and try again."
        ])
    );
    let _ = std::fs::remove_dir_all(&tmp);
}

/// A contract-wallet link this node cannot ask about is undecided: without
/// `ETH_RPC_URL` the verifier answers "not implemented", which is not the
/// depositor's fault.
#[tokio::test]
async fn a_contract_wallet_chain_without_an_rpc_validator_is_unavailable() {
    let (storage, tmp) = temp_storage("eip1654").await;
    let s = sources(
        lazy_pool(),
        storage,
        None,
        None,
        Settings {
            ignore_blockchain_access: true,
            ..Settings::default()
        },
    );
    let model = b"glb".to_vec();
    let (entity, files, _, _) = signed(scene(&["1,1"], &model), std::slice::from_ref(&model)).await;
    let chain: crate::auth::AuthChain = serde_json::from_value(json!([
        {"type": "SIGNER", "payload": "0x1111111111111111111111111111111111111111"},
        {"type": "ECDSA_EIP_1654_SIGNED_ENTITY", "payload": entity.id, "signature": "0x00"}
    ]))
    .unwrap();
    let ctx = Context {
        sources: &s,
        auth_chain: &chain,
    };
    let verdict = validate(&entity, &files, &ctx).await.unwrap_err();
    assert!(
        matches!(&verdict[..], [ValidationError::Unavailable(m)] if m.starts_with("The signature is invalid. EIP-1654")),
        "{verdict:?}"
    );
    let _ = std::fs::remove_dir_all(&tmp);
}

#[cfg(unix)]
fn seal_shard(storage: &Storage, hash: &str) -> std::path::PathBuf {
    let shard = storage.root().join(crate::storage::hex_prefix(hash));
    std::fs::set_permissions(&shard, std::fs::Permissions::from_mode(0o000)).unwrap();
    shard
}

#[cfg(unix)]
fn unseal_shard(shard: &std::path::Path) {
    let _ = std::fs::set_permissions(shard, std::fs::Permissions::from_mode(0o755));
}

#[cfg(unix)]
#[tokio::test]
async fn an_unreadable_shard_faults_the_size_probe_instead_of_answering_absent() {
    let (storage, tmp) = temp_storage("size").await;
    let s = sources(
        lazy_pool(),
        storage.clone(),
        None,
        None,
        Settings::default(),
    );
    assert_eq!(
        s.fetch_content_file_size(STORED_HASH).await.unwrap(),
        Some(7)
    );

    let shard = seal_shard(&storage, STORED_HASH);
    let size = s.fetch_content_file_size(STORED_HASH).await;
    let exist = s
        .is_content_stored_already(&[STORED_HASH.to_string()])
        .await;
    unseal_shard(&shard);
    let _ = std::fs::remove_dir_all(&tmp);

    let err = size.expect_err("damage this node cannot read is never a depositor's miss");
    assert!(err.contains("I/O error"), "unexpected cause: {err}");
    assert!(
        exist
            .expect_err("the existence probe faults too")
            .contains("I/O error")
    );
}

#[tokio::test]
async fn an_id_that_cannot_name_content_is_absent_rather_than_a_fault() {
    let (storage, tmp) = temp_storage("invalid").await;
    let s = sources(lazy_pool(), storage, None, None, Settings::default());
    let bogus = "../../etc/passwd".to_string();
    assert_eq!(s.fetch_content_file_size(&bogus).await.unwrap(), None);
    let stored = s
        .is_content_stored_already(std::slice::from_ref(&bogus))
        .await
        .unwrap();
    assert_eq!(stored.get(&bogus), Some(&false));
    let _ = std::fs::remove_dir_all(&tmp);
}

fn profile(owner: &str, name: &str, wearables: &[&str]) -> OwnedDeployment {
    OwnedDeployment::new(Entity {
        id: "bafkreiprofile".into(),
        entity_type: EntityType::Profile,
        pointers: vec![owner.to_string()],
        timestamp: 1_700_000_000_000,
        content: vec![],
        version: "v3".into(),
        metadata: Some(json!({ "avatars": [{
            "name": name, "hasClaimedName": true,
            "avatar": { "wearables": wearables, "emotes": [{ "slot": 0, "urn": "urn:decentraland:off-chain:base-emotes:wave" }] }
        }]})),
    })
}

fn names_answer(
    owner: &'static str,
    names: &'static [&'static str],
) -> impl Fn(&str, &Value) -> Result<Value, u16> + Send + Sync + 'static {
    move |query, vars| {
        assert!(query.contains("query names("), "unexpected query {query}");
        let rows: Vec<Value> = if vars["owner"] == owner {
            names.iter().enumerate().map(|(i, n)| json!({ "id": format!("ens-{i}"), "name": n, "contractAddress": "0x2a18", "tokenId": i.to_string(), "activeOrder": null })).collect()
        } else {
            vec![]
        };
        Ok(json!({ "nfts": rows }))
    }
}

#[tokio::test]
async fn claimed_names_come_from_the_marketplace_subgraph() {
    let (storage, tmp) = temp_storage("names").await;
    let graph = serve_graph(names_answer(OWNER, &["alice"])).await;
    let market = market_over(Readers {
        marketplace: Some(graph),
        ..Default::default()
    });
    let s = sources(
        lazy_pool(),
        storage.clone(),
        None,
        Some(market),
        Settings::default(),
    );

    let owned = profile(OWNER, "alice", &[]);
    assert!(
        validate_profile_access(&owned.view(), &s, OWNER)
            .await
            .is_ok()
    );
    let other = profile(OWNER, "bob", &[]);
    assert_eq!(
        validate_profile_access(&other.view(), &s, OWNER)
            .await
            .into_result(),
        rejected(&[&format!(
            "The following names (bob) are not owned by the address {OWNER}."
        )])
    );
    assert_eq!(
        validate_profile_access(
            &owned.view(),
            &s,
            "0xAAAA000000000000000000000000000000000009"
        )
        .await
        .into_result(),
        rejected(&[
            "You can only alter your own profile. The pointer address and the signer address are different (pointer:0xaaaa000000000000000000000000000000000001 signer: 0xaaaa000000000000000000000000000000000009)"
        ])
    );

    let no_subgraph = sources(
        lazy_pool(),
        storage.clone(),
        None,
        Some(market_over(Readers::default())),
        Settings::default(),
    );
    assert_eq!(
        validate_profile_access(&owned.view(), &no_subgraph, OWNER)
            .await
            .into_result(),
        unavailable(&[
            "Error checking name ownership: no marketplace subgraph is configured (MARKETPLACE_SUBGRAPH_URL)"
        ])
    );
    let failing = serve_graph(|_, _| Err(400)).await;
    let down = sources(
        lazy_pool(),
        storage,
        None,
        Some(market_over(Readers {
            marketplace: Some(failing),
            ..Default::default()
        })),
        Settings::default(),
    );
    let verdict = validate_profile_access(&owned.view(), &down, OWNER)
        .await
        .into_result()
        .unwrap_err();
    assert!(
        matches!(&verdict[..], [ValidationError::Unavailable(m)] if m.starts_with("Error checking name ownership: subgraph query failed: ")),
        "{verdict:?}"
    );
    let _ = std::fs::remove_dir_all(&tmp);
}

/// Owned copies `(owner, urn, token id)` as the collections subgraph
/// answers `ownedCopies` (`urn_in`) and `prefixed` (`urn_starts_with`).
fn copies_answer(
    copies: Vec<(String, &'static str, &'static str)>,
) -> impl Fn(&str, &Value) -> Result<Value, u16> + Send + Sync + 'static {
    let alias = regex::Regex::new(r#"p(\d+): nfts\(first: 1, where: \{ owner: \$owner, urn_starts_with: ("(?:[^"\\]|\\.)*") \}\)"#).unwrap();
    move |query, vars| {
        let owner = vars["owner"].as_str().unwrap_or_default();
        let mine = copies.iter().filter(|(o, ..)| o == owner);
        if query.contains("ownedCopies") {
            let urns: Vec<&str> = vars["urns"]
                .as_array()
                .unwrap()
                .iter()
                .filter_map(Value::as_str)
                .collect();
            let rows: Vec<Value> = mine
                .filter(|(_, urn, _)| urns.contains(urn))
                .enumerate()
                .map(|(i, (_, urn, token))| json!({ "id": format!("nft-{i}"), "urn": urn, "tokenId": token }))
                .collect();
            return Ok(json!({ "nfts": rows }));
        }
        assert!(query.contains("prefixed"), "unexpected query {query}");
        let mut data = serde_json::Map::new();
        for cap in alias.captures_iter(query) {
            let prefix: String = serde_json::from_str(&cap[2]).unwrap();
            let hit: Vec<Value> = copies
                .iter()
                .filter(|(o, urn, _)| o == owner && urn.starts_with(&prefix))
                .map(|(_, urn, _)| json!({ "urn": urn }))
                .take(1)
                .collect();
            data.insert(format!("p{}", &cap[1]), Value::Array(hit));
        }
        Ok(Value::Object(data))
    }
}

fn random_address() -> String {
    let hex = format!(
        "{}{}",
        uuid::Uuid::new_v4().simple(),
        uuid::Uuid::new_v4().simple()
    );
    format!("0x{}", &hex[..40])
}

/// The squid's three tiers over the collections subgraphs, and the escrow
/// lease overlay unioned into each (the overlay's SQL runs on the scratch
/// database, so the whole check skips without it).
#[tokio::test]
async fn owned_items_come_from_the_collections_subgraphs_and_the_escrow_overlay() {
    require_test_db!(Some(pool) = crate::db::scratch::pool(2).await);
    let (storage, tmp) = temp_storage("items").await;
    let owner = random_address();
    let token_item =
        "urn:decentraland:matic:collections-v2:0xaaa0000000000000000000000000000000000001:1";
    let v1_item = "urn:decentraland:ethereum:collections-v1:halloween_2019:hat";
    let prefix_item =
        "urn:decentraland:matic:collections-v2:0xbbb0000000000000000000000000000000000002:3";
    let copies = vec![
        (owner.clone(), token_item, "42"),
        (owner.clone(), v1_item, "7"),
        (owner.clone(), prefix_item, "1"),
    ];
    let graph = serve_graph(copies_answer(copies)).await;
    let market = market_over(Readers {
        eth: Some(graph.clone()),
        matic: Some(graph.clone()),
        marketplace: None,
    });
    let s = sources(
        pool.clone(),
        storage.clone(),
        None,
        Some(market),
        Settings::default(),
    );

    let unowned =
        "urn:decentraland:matic:collections-v2:0xccc0000000000000000000000000000000000003:0";
    let leased_token =
        "urn:decentraland:matic:collections-v2:0xddd0000000000000000000000000000000000004:0:9";
    let wearables = [
        "urn:decentraland:matic:collections-v2:0xaaa0000000000000000000000000000000000001:1:42",
        "urn:decentraland:mainnet:collections-v1:halloween_2019:hat",
        "urn:decentraland:matic:collections-v2:0xbbb0000000000000000000000000000000000002",
        "urn:decentraland:off-chain:base-avatars:eyes_00",
        unowned,
        leased_token,
    ];
    let mut d = profile(&owner, "", &wearables);
    d.entity.metadata.as_mut().unwrap()["avatars"][0]["hasClaimedName"] = json!(false);
    assert_eq!(
        validate_profile_access(&d.view(), &s, &owner)
            .await
            .into_result(),
        rejected(&[&format!(
            "The following items ({unowned}, {leased_token}) are not owned by the address {owner}."
        )]),
        "the token, exact (mainnet spelled ethereum) and prefix tiers resolve; off-chain items are never asked"
    );

    sqlx::query(
        "INSERT INTO shop.usage_grants (grantee_address, urn, token_id, category, unlock_at) VALUES \
         ($1, $2, NULL, 'wearable', timezone('UTC', now()) + interval '2 days'), \
         ($1, 'urn:decentraland:matic:collections-v2:0xddd0000000000000000000000000000000000004:0', '9', 'wearable', timezone('UTC', now()) + interval '2 days')",
    )
    .bind(&owner)
    .bind(unowned)
    .execute(&pool)
    .await
    .unwrap();
    assert!(
        validate_profile_access(&d.view(), &s, &owner).await.is_ok(),
        "an active escrow lease is ownership"
    );

    sqlx::query("UPDATE shop.usage_grants SET status = 'revoked' WHERE grantee_address = $1 AND token_id IS NULL")
        .bind(&owner)
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        validate_profile_access(&d.view(), &s, &owner)
            .await
            .into_result(),
        rejected(&[&format!(
            "The following items ({unowned}) are not owned by the address {owner}."
        )]),
        "a revoked lease grants nothing"
    );

    let matic_unset = sources(
        pool.clone(),
        storage,
        None,
        Some(market_over(Readers {
            eth: Some(graph),
            ..Default::default()
        })),
        Settings::default(),
    );
    assert_eq!(
        validate_profile_access(&d.view(), &matic_unset, &owner)
            .await
            .into_result(),
        unavailable(&[
            "Error checking item ownership: no matic collections subgraph is configured (MATIC_COLLECTIONS_SUBGRAPH_URL)"
        ])
    );

    sqlx::query("DELETE FROM shop.usage_grants WHERE grantee_address = $1")
        .bind(&owner)
        .execute(&pool)
        .await
        .unwrap();
    let _ = std::fs::remove_dir_all(&tmp);
    pool.close().await;
}

fn wearable(pointer: &str) -> OwnedDeployment {
    OwnedDeployment::new(Entity {
        id: "bafkreiwearable".into(),
        entity_type: EntityType::Wearable,
        pointers: vec![pointer.to_string()],
        timestamp: 1_700_000_000_000,
        content: vec![],
        version: "v3".into(),
        metadata: None,
    })
}

#[tokio::test]
async fn collection_access_reads_the_collections_subgraph_of_the_items_layer() {
    let (storage, tmp) = temp_storage("collections").await;
    const COLLECTION: &str = "0xc011000000000000000000000000000000000001";
    let graph = serve_graph(|query, vars| {
        assert!(
            query.contains("collectionAccess"),
            "unexpected query {query}"
        );
        if vars["collection"] != COLLECTION {
            return Ok(json!({ "collections": [], "items": [] }));
        }
        let items = if vars["itemId"] == "3" {
            json!([{ "creator": OWNER, "managers": [OPERATOR], "minters": [] }])
        } else {
            json!([])
        };
        Ok(json!({
            "collections": [{ "creator": OWNER, "owner": OWNER, "managers": [], "minters": [] }],
            "items": items
        }))
    })
    .await;
    let s = sources(
        lazy_pool(),
        storage.clone(),
        None,
        Some(market_over(Readers {
            matic: Some(graph.clone()),
            ..Default::default()
        })),
        Settings::default(),
    );
    let pointer = format!(
        "urn:decentraland:matic:collections-v2:{}:3",
        COLLECTION.to_uppercase().replace("0X", "0x")
    );
    let d = wearable(&pointer);

    assert!(
        validate_item_access(&d.view(), &s, OWNER).await.is_ok(),
        "the collection's creator"
    );
    assert!(
        validate_item_access(&d.view(), &s, OPERATOR).await.is_ok(),
        "the deployed item's manager"
    );
    assert_eq!(
        validate_item_access(
            &wearable(&format!(
                "urn:decentraland:matic:collections-v2:{COLLECTION}:4"
            ))
            .view(),
            &s,
            OPERATOR
        )
        .await
        .into_result(),
        rejected(&[&format!(
            "The provided Eth Address '{OPERATOR}' does not have access to the following item: ({COLLECTION}, 4)"
        )]),
        "another item's manager"
    );
    assert_eq!(
        validate_item_access(&d.view(), &s, STRANGER)
            .await
            .into_result(),
        rejected(&[&format!(
            "The provided Eth Address '{STRANGER}' does not have access to the following item: ({COLLECTION}, 3)"
        )])
    );

    let v1 = wearable(
        "urn:decentraland:ethereum:collections-v1:community_contest:cw_bell_attendant_hat",
    );
    assert!(
        validate_item_access(&v1.view(), &s, DECENTRALAND_ADDRESS)
            .await
            .is_ok(),
        "a v1 item by name: the Decentraland address"
    );
    assert_eq!(
        validate_item_access(&v1.view(), &s, STRANGER)
            .await
            .into_result(),
        rejected(&[&format!(
            "The provided Eth Address '{STRANGER}' does not have access to the following item: (community_contest, cw_bell_attendant_hat)"
        )])
    );
    assert_eq!(
        validate_item_access(
            &wearable("urn:decentraland:off-chain:base-avatars:hat").view(),
            &s,
            STRANGER
        )
        .await
        .into_result(),
        rejected(&[&format!(
            "The provided Eth Address '{STRANGER}' does not have access to the following item: 'urn:decentraland:off-chain:base-avatars:hat'"
        )])
    );

    let eth_unset = wearable(&format!(
        "urn:decentraland:ethereum:collections-v2:{COLLECTION}:3"
    ));
    assert_eq!(
        validate_item_access(&eth_unset.view(), &s, OWNER)
            .await
            .into_result(),
        unavailable(&[
            "Error checking collection access: no ethereum collections subgraph is configured (ETH_COLLECTIONS_SUBGRAPH_URL)"
        ])
    );
    let failing = serve_graph(|_, _| Err(400)).await;
    let down = sources(
        lazy_pool(),
        storage,
        None,
        Some(market_over(Readers {
            matic: Some(failing),
            ..Default::default()
        })),
        Settings::default(),
    );
    let verdict = validate_item_access(&d.view(), &down, OWNER)
        .await
        .into_result()
        .unwrap_err();
    assert!(
        matches!(&verdict[..], [ValidationError::Unavailable(m)] if m.starts_with("Error checking collection access: subgraph query failed: ")),
        "{verdict:?}"
    );
    let _ = std::fs::remove_dir_all(&tmp);
}

/// The production woodie wearable's own proof reproduces the root its
/// registry held; the check reads that root at the deploy's L2 block.
#[tokio::test]
async fn third_party_items_are_proven_against_the_registry_root_at_the_deploy_block() {
    const WOODIE_0: &[u8] = include_bytes!("third_party_fixtures/woodie_0.entity.json");
    let (storage, tmp) = temp_storage("tp").await;
    let entity = entity_parser::parse_entity_from_bytes(
        WOODIE_0,
        "bafkreigxzm4duawatfhiylmawwdr4yezskltd3ygkup6nsl4yyif5t57i4",
    )
    .unwrap();
    let proof: third_party::MerkleProof =
        serde_json::from_value(entity.metadata.as_ref().unwrap()["merkleProof"].clone()).unwrap();
    let decoded: Vec<[u8; 32]> = proof
        .proof
        .iter()
        .map(|p| merkle::decode_hash32(p).unwrap())
        .collect();
    let root = format!(
        "0x{}",
        hex::encode(merkle::generate_root(
            proof.index,
            &proof.entity_hash,
            &decoded
        ))
    );
    let mut d = OwnedDeployment::new(entity);
    d.files.insert(d.entity.id.clone(), WOODIE_0.to_vec());
    let pointer = d.entity.pointers[0].clone();

    let blocks = serve_graph(|query, vars| {
        assert!(query.contains("getBlockForTimestampRange"), "{query}");
        assert!(vars["timestamp"].as_i64().unwrap() > vars["timestamp5Min"].as_i64().unwrap());
        Ok(json!({ "max": [{ "number": "70000000" }], "min": [{ "number": "69999900" }] }))
    })
    .await;
    let registry_for = |answer: Value| {
        move |query: &str, vars: &Value| {
            assert!(query.contains("MerkleRoot"), "{query}");
            assert_eq!(vars["block"], 70_000_000);
            assert_eq!(
                vars["id"],
                "urn:decentraland:matic:collections-thirdparty:woodies"
            );
            Ok(answer.clone())
        }
    };
    let tp_settings = |registry: String, blocks: &str| Settings {
        third_party_registry_url: Some(registry),
        blocks_l2_url: Some(blocks.to_string()),
        ..Settings::default()
    };

    let good = serve_graph(registry_for(json!({ "thirdParties": [{ "root": root }] }))).await;
    let s = sources(
        lazy_pool(),
        storage.clone(),
        None,
        None,
        tp_settings(good, &blocks),
    );
    assert!(
        validate_item_access(&d.view(), &s, STRANGER).await.is_ok(),
        "the proof, not the deployer, authorizes a third-party item ({pointer})"
    );

    let wrong = serve_graph(registry_for(
        json!({ "thirdParties": [{ "root": format!("0x{}", "11".repeat(32)) }] }),
    ))
    .await;
    let unapproved = serve_graph(registry_for(json!({ "thirdParties": [] }))).await;
    for registry in [wrong, unapproved] {
        let s = sources(
            lazy_pool(),
            storage.clone(),
            None,
            None,
            tp_settings(registry, &blocks),
        );
        assert_eq!(
            validate_item_access(&d.view(), &s, STRANGER)
                .await
                .into_result(),
            rejected(&["Couldn't verify merkle proofed entity for third-party wearable"])
        );
    }

    let failing = serve_graph(|_, _| Err(400)).await;
    let s = sources(
        lazy_pool(),
        storage.clone(),
        None,
        None,
        tp_settings(failing, &blocks),
    );
    assert_eq!(
        validate_item_access(&d.view(), &s, STRANGER)
            .await
            .into_result(),
        unavailable(&[
            "Error checking third-party access: subgraph query failed: third-party registry: HTTP 400 Bad Request"
        ])
    );
    let s = sources(lazy_pool(), storage, None, None, Settings::default());
    assert_eq!(
        validate_item_access(&d.view(), &s, STRANGER)
            .await
            .into_result(),
        unavailable(&[
            "Error checking third-party access: no third-party registry is configured (THIRD_PARTY_REGISTRY_L2_SUBGRAPH_URL, BLOCKS_L2_SUBGRAPH_URL)"
        ])
    );
    let _ = std::fs::remove_dir_all(&tmp);
}

/// The decimal token id of LAND (x, y), as the registry packs it.
fn land_token(x: i32, y: i32) -> String {
    let hex = format!("{:032x}{:032x}", x as i128 as u128, y as i128 as u128);
    let mut digits: Vec<u8> = vec![0];
    for c in hex.chars() {
        let mut carry = c.to_digit(16).unwrap();
        for d in digits.iter_mut() {
            let v = (*d as u32) * 16 + carry;
            *d = (v % 10) as u8;
            carry = v / 10;
        }
        while carry > 0 {
            digits.push((carry % 10) as u8);
            carry /= 10;
        }
    }
    while digits.len() > 1 && *digits.last().unwrap() == 0 {
        digits.pop();
    }
    digits.iter().rev().map(|d| (b'0' + d) as char).collect()
}

fn land_event(
    block: i64,
    kind: &'static str,
    token: &str,
    account: Option<&str>,
    operator: Option<&str>,
) -> AuthzEvent {
    AuthzEvent {
        block_number: block,
        log_index: 0,
        block_time: block * 12,
        token_address: crate::land_authz::LAND_REGISTRY_MAINNET.to_string(),
        kind,
        token_id: Some(token.to_string()),
        estate_id: None,
        account: account.map(str::to_string),
        operator: operator.map(str::to_string),
        approved: None,
    }
}

/// The LAND leg reads the index once its fold is published: an update
/// operator, whom the subgraph fallback cannot see, deploys. The index's
/// cursor row is global, so this runs on a database of its own.
#[tokio::test]
async fn the_land_leg_reads_the_index_once_it_is_ready() {
    require_test_db!(Ok(base) = std::env::var("CATALYST_TEST_DATABASE_URL"));
    let opts: PgConnectOptions = base.parse().unwrap();
    let admin = PgPoolOptions::new()
        .max_connections(1)
        .connect_with(opts.database("postgres"))
        .await
        .unwrap();
    let name = format!(
        "catalyst_test_validator_{}",
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

    let (storage, tmp) = temp_storage("index").await;
    let s = sources(
        pool.clone(),
        storage,
        Some(land_over(pool.clone(), None)),
        None,
        Settings::default(),
    );
    let token = land_token(1, 1);
    let settings = crate::land_authz::Settings::for_tests();
    let start = settings.start_block as i64;

    let not_ready = s
        .check_land_access(OPERATOR, &[(1, 1)], 0)
        .await
        .unwrap_err()
        .to_string();
    assert!(
        not_ready.starts_with("no LAND rights source"),
        "{not_ready}"
    );

    indexer::ensure_cursor(&pool, &settings).await.unwrap();
    {
        let mut conn = pool.acquire().await.unwrap();
        indexer::insert_events(
            &mut conn,
            &[
                land_event(
                    start + 10,
                    crate::land_authz::KIND_TRANSFER,
                    &token,
                    Some(OWNER),
                    None,
                ),
                land_event(
                    start + 20,
                    crate::land_authz::KIND_UPDATE_OPERATOR,
                    &token,
                    None,
                    Some(OPERATOR),
                ),
            ],
        )
        .await
        .unwrap();
        indexer::set_cursor(&mut conn, crate::land_authz::NETWORK_MAINNET, start + 100)
            .await
            .unwrap();
    }
    indexer::fold(&pool, &settings, None).await.unwrap();
    assert!(
        indexer::mark_folded(
            &pool,
            crate::land_authz::NETWORK_MAINNET,
            (start + 100) as u64
        )
        .await
        .unwrap()
    );

    assert_eq!(
        s.check_land_access(OWNER, &[(1, 1), (2, 2)], 0)
            .await
            .unwrap(),
        vec![true, false]
    );
    assert_eq!(
        s.check_land_access(OPERATOR, &[(1, 1)], 0).await.unwrap(),
        vec![true],
        "the update operator leg"
    );
    assert_eq!(
        s.check_land_access(STRANGER, &[(1, 1)], 0).await.unwrap(),
        vec![false]
    );

    pool.close().await;
    let _ = sqlx::query(sqlx::AssertSqlSafe(format!(
        "DROP DATABASE \"{name}\" WITH (FORCE)"
    )))
    .execute(&admin)
    .await;
    let _ = std::fs::remove_dir_all(&tmp);
}

#[tokio::test]
async fn the_decentraland_addresses_are_the_constant_and_the_configured_one() {
    let (storage, tmp) = temp_storage("dcl").await;
    let s = sources(
        lazy_pool(),
        storage,
        None,
        None,
        Settings {
            additional_dcl_address: Some("0x00000000000000000000000000000000000000ee".into()),
            ..Settings::default()
        },
    );
    assert!(s.is_address_owned_by_decentraland(DECENTRALAND_ADDRESS));
    assert!(s.is_address_owned_by_decentraland("0x1337E0507EB4AB47E08A179573ED4533D9E22A7B"));
    assert!(s.is_address_owned_by_decentraland("0x00000000000000000000000000000000000000EE"));
    assert!(!s.is_address_owned_by_decentraland("0xrandom"));
    let _ = std::fs::remove_dir_all(&tmp);
}

#[test]
fn settings_refuse_plaintext_private_half_configured_sources_and_bad_addresses() {
    let hosts = vec!["catalyst.example".to_string()];
    let parse = |tpr: Option<&str>, blocks: Option<&str>, extra: Option<&str>| {
        Settings::parse(false, extra, tpr, blocks, &hosts)
    };
    let ok = parse(
        Some(" https://api.example/tpr/KEY "),
        Some("https://api.example/blocks"),
        Some("0x00000000000000000000000000000000000000EE"),
    )
    .unwrap();
    assert_eq!(
        ok.third_party_registry_url.as_deref(),
        Some("https://api.example/tpr/KEY")
    );
    assert_eq!(
        ok.additional_dcl_address.as_deref(),
        Some("0x00000000000000000000000000000000000000ee")
    );
    assert!(
        !format!("{ok:?}").contains("KEY"),
        "the Debug form never prints a subgraph URL"
    );
    assert!(
        parse(None, None, Some(""))
            .unwrap()
            .additional_dcl_address
            .is_none()
    );

    for (tpr, blocks, extra, needle) in [
        (
            Some("http://api.example/tpr/KEY"),
            Some("https://api.example/blocks"),
            None,
            "THIRD_PARTY_REGISTRY_L2_SUBGRAPH_URL",
        ),
        (
            Some("https://127.0.0.1/tpr"),
            Some("https://api.example/blocks"),
            None,
            "THIRD_PARTY_REGISTRY_L2_SUBGRAPH_URL",
        ),
        (
            Some("https://api.example/tpr"),
            Some("https://catalyst.example/blocks"),
            None,
            "BLOCKS_L2_SUBGRAPH_URL",
        ),
        (Some("https://api.example/tpr"), None, None, "set together"),
        (
            None,
            None,
            Some("not-an-address"),
            "ADDITIONAL_DECENTRALAND_ADDRESS",
        ),
    ] {
        let err = format!("{:#}", parse(tpr, blocks, extra).unwrap_err());
        assert!(err.contains(needle), "{err}");
        assert!(
            !err.contains("KEY"),
            "an error never echoes the value: {err}"
        );
    }
}

#[test]
fn a_refusal_keeps_its_kind_in_the_deploy_failure() {
    use super::super::write_deployer::DeployFailure;
    let rejected = into_deploy_failure(vec![
        ValidationError::Rejected("a".into()),
        ValidationError::Rejected("b".into()),
    ]);
    assert!(
        matches!(&rejected, DeployFailure::Rejected(e) if e == &["a".to_string(), "b".to_string()])
    );
    let unavailable = into_deploy_failure(vec![ValidationError::Unavailable("store".into())]);
    assert!(matches!(&unavailable, DeployFailure::Unavailable(e) if e == &["store".to_string()]));
}

/// `GET /queries/erc721/...` over the content database: the active entity
/// at the built URN, 400 for an unknown chain or a non-standard wearable,
/// 404 for none or a denylisted one.
#[tokio::test]
async fn the_erc721_route_formats_the_active_wearable() {
    require_test_db!(Some(pool) = crate::db::scratch::pool(2).await);
    let (storage, tmp) = temp_storage("erc721").await;
    let tag = uuid::Uuid::new_v4().simple().to_string();
    let contract = format!("0x{}", &format!("{tag}{tag}")[..40]);
    let seed = |item: &str, metadata: Value| {
        let pool = pool.clone();
        let pointer = format!("urn:decentraland:matic:collections-v2:{contract}:{item}");
        let id = format!("bafkreierc721{tag}{item}");
        async move {
            let dep: i32 = sqlx::query_scalar(
                "INSERT INTO content.deployments (deployer_address, version, entity_type, entity_id, entity_metadata, \
                 entity_timestamp, entity_pointers, local_timestamp, auth_chain) \
                 VALUES ('0xdeployer', 'v3', 'wearable', $1, $2::json, to_timestamp(1700000000), $3, now(), '[]'::json) RETURNING id",
            )
            .bind(&id)
            .bind(json!({ "v": metadata }))
            .bind(vec![pointer.clone()])
            .fetch_one(&pool)
            .await
            .unwrap();
            sqlx::query("INSERT INTO content.content_files (deployment, content_hash, key) VALUES ($1, 'bafkreithumb', 'thumbnail.png')")
                .bind(dep)
                .execute(&pool)
                .await
                .unwrap();
            sqlx::query("INSERT INTO content.active_pointers (pointer, entity_id, entity_type) VALUES ($1, $2, 'wearable')")
                .bind(&pointer)
                .bind(&id)
                .execute(&pool)
                .await
                .unwrap();
            id
        }
    };
    let standard = seed(
        "0",
        json!({ "i18n": [{"code": "en", "text": "Hat"}], "rarity": "epic", "thumbnail": "thumbnail.png",
                "data": { "category": "hat", "tags": ["a"], "representations": [{ "bodyShapes": ["urn:decentraland:off-chain:base-avatars:BaseMale"] }] } }),
    )
    .await;
    let _plain = seed("1", json!({ "name": "no rarity" })).await;

    let state = Arc::new(crate::ContentState::new(
        pool.clone(),
        crate::Settings::for_tests(),
        storage,
    ));
    let app = Router::new()
        .route(
            "/queries/erc721/{chainId}/{contract}/{option}",
            get(erc721::get_erc721_entity),
        )
        .route(
            "/queries/erc721/{chainId}/{contract}/{option}/{emission}",
            get(erc721::get_erc721_entity),
        )
        .with_state(state.clone());
    let get_json = |path: String| {
        let app = app.clone();
        async move {
            let resp = app
                .oneshot(
                    axum::http::Request::get(path)
                        .body(axum::body::Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            let status = resp.status().as_u16();
            let body = axum::body::to_bytes(resp.into_body(), 1 << 20)
                .await
                .unwrap();
            (status, serde_json::from_slice::<Value>(&body).unwrap())
        }
    };

    let (status, body) = get_json(format!("/queries/erc721/137/{contract}/0/12")).await;
    assert_eq!(status, 200, "{body}");
    assert_eq!(
        body,
        json!({
            "id": format!("urn:decentraland:matic:collections-v2:{contract}:0"),
            "name": "Hat",
            "description": "DCL Wearable 12/1000",
            "language": "en-US",
            "thumbnail": "http://localhost/content/contents/bafkreithumb",
            "attributes": [
                {"trait_type": "Rarity", "value": "epic"},
                {"trait_type": "Category", "value": "hat"},
                {"trait_type": "Tag", "value": "a"},
                {"trait_type": "Body Shape", "value": "BaseMale"}
            ]
        })
    );
    assert_eq!(
        get_json(format!("/queries/erc721/137/{contract}/0"))
            .await
            .1["description"],
        ""
    );
    assert_eq!(
        get_json(format!("/queries/erc721/999/{contract}/0")).await,
        (400, json!({"error": "Invalid chainId '999'"}))
    );
    assert_eq!(
        get_json(format!("/queries/erc721/137/{contract}/1")).await,
        (400, json!({"error": "Wearable is not standard."}))
    );
    assert_eq!(
        get_json(format!("/queries/erc721/137/{contract}/2")).await,
        (404, json!({"error": "Entity does not exist"}))
    );
    state.denylist.add(&standard);
    assert_eq!(
        get_json(format!("/queries/erc721/137/{contract}/0")).await,
        (404, json!({"error": "Entity does not exist"}))
    );

    sqlx::query("DELETE FROM content.active_pointers WHERE entity_id LIKE $1")
        .bind(format!("bafkreierc721{tag}%"))
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("DELETE FROM content.content_files WHERE deployment IN (SELECT id FROM content.deployments WHERE entity_id LIKE $1)")
        .bind(format!("bafkreierc721{tag}%"))
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("DELETE FROM content.deployments WHERE entity_id LIKE $1")
        .bind(format!("bafkreierc721{tag}%"))
        .execute(&pool)
        .await
        .unwrap();
    let _ = std::fs::remove_dir_all(&tmp);
    pool.close().await;
}
