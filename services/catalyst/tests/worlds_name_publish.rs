//! A DCL NAME world published through a running `serve` with every subgraph
//! URL set empty (`ETH_COLLECTIONS_SUBGRAPH_URL`,
//! `MATIC_COLLECTIONS_SUBGRAPH_URL`, `MARKETPLACE_SUBGRAPH_URL`,
//! `THIRD_PARTY_REGISTRY_L2_SUBGRAPH_URL`, `LAND_SUBGRAPH_URL`) and the nft
//! index on for mainnet: the names index (the DCLRegistrar rows of
//! `shop.nft` under the published Ethereum fold) is the only NAME source.
//! Under it the NAME's holder publishes the world and the world serves it.
//! Someone else's publish for the same world is refused. While the fold is
//! stale every publish to the world is refused fail-closed as the names
//! index being unavailable (503 `nft-index`, retryable), and the holder's
//! goes through once the fold is republished. The world carries a size override,
//! so its publish reads no on-chain storage allowance and the run dials no
//! outside host; ownership is still the names index's. Needs
//! CATALYST_TEST_DATABASE_URL; skipped without it.

use std::path::Path;
use std::process::Stdio;
use std::time::Duration;

use alloy::signers::{Signer, local::PrivateKeySigner};
use catalyst::auth::hash::hash_bytes_v1;
use catalyst::db::scratch::{OwnedDatabase, Schema};
use reqwest::header::CONTENT_TYPE;
use serde_json::{Value, json};
use tokio::process::{Child, Command};

/// The NAME's holder.
const HOLDER_KEY: &str = "ac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80";
/// A signer who holds no NAME.
const STRANGER_KEY: &str = "59c6995e998f97a5a0044966f0945389dc9e86dae88c7a8412f4603b6b78690d";

/// The subgraph variables, each set empty: an operator turning every
/// subgraph off.
const SUBGRAPH_URLS: [&str; 5] = [
    "ETH_COLLECTIONS_SUBGRAPH_URL",
    "MATIC_COLLECTIONS_SUBGRAPH_URL",
    "MARKETPLACE_SUBGRAPH_URL",
    "THIRD_PARTY_REGISTRY_L2_SUBGRAPH_URL",
    "LAND_SUBGRAPH_URL",
];

const SCENE_JS: &str = "export function onUpdate() {}\n";

fn address(key: &str) -> String {
    format!("{:#x}", key.parse::<PrivateKeySigner>().unwrap().address())
}

fn now_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

fn multipart(parts: &[(String, Option<String>, Vec<u8>)]) -> (Vec<u8>, String) {
    let boundary = format!("----catalyst{}", uuid::Uuid::new_v4().simple());
    let mut body = Vec::new();
    for (name, filename, value) in parts {
        body.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
        match filename {
            Some(f) => body.extend_from_slice(
                format!("Content-Disposition: form-data; name=\"{name}\"; filename=\"{f}\"\r\nContent-Type: application/octet-stream\r\n\r\n").as_bytes(),
            ),
            None => body.extend_from_slice(
                format!("Content-Disposition: form-data; name=\"{name}\"\r\n\r\n").as_bytes(),
            ),
        }
        body.extend_from_slice(value);
        body.extend_from_slice(b"\r\n");
    }
    body.extend_from_slice(format!("--{boundary}--\r\n").as_bytes());
    (body, format!("multipart/form-data; boundary={boundary}"))
}

/// Publishes a one-file scene for `world` signed by `key` through
/// `POST /worlds-content-server/entities`; the answer and the entity id.
async fn publish(
    client: &reqwest::Client,
    base: &str,
    world: &str,
    key: &str,
) -> (u16, Value, String) {
    let script_hash = hash_bytes_v1(SCENE_JS.as_bytes());
    let entity = json!({
        "version": "v3",
        "type": "scene",
        "timestamp": now_ms(),
        "pointers": ["0,0"],
        "content": [{ "file": "bin/index.js", "hash": script_hash }],
        "metadata": {
            "display": { "title": "A NAME world" },
            "main": "bin/index.js",
            "runtimeVersion": "7",
            "worldConfiguration": { "name": world },
            "scene": { "base": "0,0", "parcels": ["0,0"] }
        }
    });
    let entity_bytes = serde_json::to_vec(&entity).unwrap();
    let entity_id = hash_bytes_v1(&entity_bytes);
    let signer: PrivateKeySigner = key.parse().unwrap();
    let signature = signer.sign_message(entity_id.as_bytes()).await.unwrap();
    let chain = [
        ("SIGNER", address(key), String::new()),
        (
            "ECDSA_SIGNED_ENTITY",
            entity_id.clone(),
            signature.to_string(),
        ),
    ];
    let mut parts = vec![("entityId".to_string(), None, entity_id.clone().into_bytes())];
    for (i, (kind, payload, signature)) in chain.into_iter().enumerate() {
        parts.push((
            format!("authChain[{i}][type]"),
            None,
            kind.as_bytes().to_vec(),
        ));
        parts.push((
            format!("authChain[{i}][payload]"),
            None,
            payload.into_bytes(),
        ));
        parts.push((
            format!("authChain[{i}][signature]"),
            None,
            signature.into_bytes(),
        ));
    }
    parts.push((entity_id.clone(), Some(entity_id.clone()), entity_bytes));
    parts.push((
        script_hash.clone(),
        Some(script_hash),
        SCENE_JS.as_bytes().to_vec(),
    ));
    let (body, content_type) = multipart(&parts);
    let resp = client
        .post(format!("{base}/worlds-content-server/entities"))
        .header(CONTENT_TYPE, content_type)
        .body(body)
        .send()
        .await
        .unwrap();
    let status = resp.status().as_u16();
    let bytes = resp.bytes().await.unwrap();
    let value = serde_json::from_slice(&bytes)
        .unwrap_or_else(|_| Value::String(String::from_utf8_lossy(&bytes).into()));
    (status, value, entity_id)
}

/// The serve's log, for a failure message.
fn log_tail(root: &Path) -> String {
    let log = std::fs::read_to_string(root.join("serve.log")).unwrap_or_default();
    let lines: Vec<&str> = log.lines().collect();
    lines[lines.len().saturating_sub(40)..].join("\n")
}

async fn wait_healthy(child: &mut Child, client: &reqwest::Client, base: &str, root: &Path) {
    let ready = tokio::time::timeout(Duration::from_secs(60), async {
        loop {
            assert!(
                child.try_wait().unwrap().is_none(),
                "serve exited before it answered /health:\n{}",
                log_tail(root)
            );
            if let Ok(resp) = client.get(format!("{base}/health")).send().await
                && resp.status().is_success()
            {
                return;
            }
            tokio::time::sleep(Duration::from_millis(200)).await;
        }
    })
    .await;
    assert!(
        ready.is_ok(),
        "serve never answered /health:\n{}",
        log_tail(root)
    );
}

/// Publishes the Ethereum nft fold at block 100, `age_minutes` ago, with the
/// Polygon one beside it; the cursor's update acknowledges the fold's
/// writes.
async fn publish_names(pool: &sqlx::PgPool, age_minutes: i64) {
    sqlx::query(
        "INSERT INTO shop.chain_cursor (indexer, network, start_block, last_block, fold_through, folded_block, folded_at, full_history, chain_id) \
         SELECT 'nft', n.network, 1, 100, 100, 100, timezone('UTC', now()) - make_interval(mins => $1::int), true, n.chain_id \
         FROM (VALUES ('ethereum', 1), ('matic', 137)) AS n(network, chain_id) \
         ON CONFLICT (indexer, network) DO UPDATE SET last_block = 100, fold_through = 100, folded_block = 100, \
             folded_at = EXCLUDED.folded_at, full_history = true, chain_id = EXCLUDED.chain_id, quarantined_at = NULL",
    )
    .bind(i32::try_from(age_minutes).unwrap())
    .execute(pool)
    .await
    .unwrap();
}

/// `label`'s DCLRegistrar token, held by `owner`, in the names index.
async fn mint_name(pool: &sqlx::PgPool, label: &str, owner: &str) {
    sqlx::query(
        "INSERT INTO shop.nft (network, contract_address, token_id, owner, ens_subdomain, minted_block, minted_at, transferred_block, transferred_log, transferred_at) \
         VALUES ('ethereum', $1, 4242, $2, $3, 10, 1700000000, 10, 0, 1700000000)",
    )
    .bind(catalyst::shop::market::nft_index::DCL_REGISTRAR)
    .bind(owner)
    .bind(label)
    .execute(pool)
    .await
    .unwrap();
}

#[tokio::test]
async fn a_name_holder_publishes_a_world_with_every_subgraph_url_unset() {
    let Some(db) = OwnedDatabase::new("names_world", Schema::Full, 2).await else {
        return;
    };
    let root = std::env::temp_dir().join(format!("catalyst-names-world-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir(&root).unwrap();
    let label = format!("e2e{}", &uuid::Uuid::new_v4().simple().to_string()[..12]);
    let world = format!("{label}.dcl.eth");
    let holder = address(HOLDER_KEY);
    mint_name(&db.pool, &label, &holder).await;
    publish_names(&db.pool, 0).await;
    // A size override: the cap is fixed, so the publish asks no RPC for the
    // owner's storage allowance (the node's default RPCs are public).
    sqlx::query("INSERT INTO worlds.world_limit_overrides (world_name, max_size_bytes) VALUES ($1, 1048576)")
        .bind(&world)
        .execute(&db.pool)
        .await
        .unwrap();

    let port = std::net::TcpListener::bind(("127.0.0.1", 0))
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    let base = format!("http://127.0.0.1:{port}");
    let log = std::fs::File::create(root.join("serve.log")).unwrap();
    let log_out = log.try_clone().unwrap();
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_catalyst"));
    cmd.arg("serve")
        .env_clear()
        .current_dir(&root)
        .env("DATABASE_URL", &db.url)
        .env("HTTP_SERVER_HOST", "127.0.0.1")
        .env("HTTP_SERVER_PORT", port.to_string())
        .env("SERVE_METRICS_BIND", "127.0.0.1:0")
        .env("FLEET_JOBS_ENABLED", "false")
        .env("LIVEKIT_ALLOW_DEV_CREDS", "true")
        .env("ENCRYPTION_KEY", "07".repeat(32))
        .env("SESSION_SECRET", uuid::Uuid::new_v4().simple().to_string())
        .env("SERVE_UNSYNCED", "true")
        .env("AB_CDN_CACHE_DIR", root.join("cdn"))
        .env("STORAGE_ROOT_FOLDER", root.join("content"))
        .env("BVIMPOSTERS_STORE_ROOT", root.join("imposters"))
        .env("PROFILE_IMAGES_CACHE_DIR", root.join("profiles"))
        .env("ETH_NETWORK", "mainnet")
        .env("MARKETPLACE_NFT_INDEX_ENABLED", "true")
        .env("WORLDS_WRITES_ENABLED", "true")
        .env("RUST_LOG", "info")
        .kill_on_drop(true)
        .stdout(Stdio::from(log_out))
        .stderr(Stdio::from(log));
    for variable in SUBGRAPH_URLS {
        cmd.env(variable, "");
    }
    let mut child = cmd.spawn().unwrap();
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(20))
        .build()
        .unwrap();
    wait_healthy(&mut child, &client, &base, &root).await;

    // Someone who holds no NAME cannot publish it (the worlds server's 400).
    let (status, body, _) = publish(&client, &base, &world, STRANGER_KEY).await;
    assert_eq!(
        status,
        400,
        "a stranger's publish: {body}\n{}",
        log_tail(&root)
    );
    assert!(
        body["message"]
            .as_str()
            .is_some_and(|m| m.contains(&format!(
                "does not have permission to deploy under \"{world}\""
            ))),
        "{body}"
    );

    // The holder, as the names index has it, publishes; the world serves it.
    let (status, body, entity_id) = publish(&client, &base, &world, HOLDER_KEY).await;
    assert_eq!(
        status,
        200,
        "the holder's publish: {body}\n{}",
        log_tail(&root)
    );
    assert_eq!(
        body["message"].as_str().map(|m| m.contains(&world)),
        Some(true),
        "{body}"
    );
    let about: Value = client
        .get(format!("{base}/world/{world}/about"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let urns = about["configurations"]["scenesUrn"].to_string();
    assert!(urns.contains(&entity_id), "{about}");

    // A stale fold cannot establish the NAME's holder: the publish is still
    // refused (fail-closed), as the names index being unavailable, a
    // retryable 503 naming it, not a denial. Nobody's NAME can be
    // established then, so a stranger's publish waits the same way.
    publish_names(&db.pool, 120).await;
    for key in [HOLDER_KEY, STRANGER_KEY] {
        let (status, body, _) = publish(&client, &base, &world, key).await;
        assert_eq!(
            status,
            503,
            "a stale names index: {body}\n{}",
            log_tail(&root)
        );
        assert_eq!(body["source"], "nft-index", "{body}");
        assert_eq!(body["configure"], "MARKETPLACE_NFT_INDEX_ENABLED", "{body}");
        assert!(
            body["message"]
                .as_str()
                .is_some_and(|m| m.contains("NAME ownership cannot be established")),
            "{body}"
        );
    }
    publish_names(&db.pool, 0).await;
    let (status, body, _) = publish(&client, &base, &world, HOLDER_KEY).await;
    assert_eq!(status, 200, "republished: {body}\n{}", log_tail(&root));

    child.kill().await.unwrap();
    db.drop().await;
    std::fs::remove_dir_all(root).unwrap();
}

/// The write-time NAME check every publish runs in its write's transaction
/// (`Policy::confirm_name_holder_on`) on the production names source, shop's
/// `IndexNames`, over the real tables. It holds the names publication until
/// that transaction ends. A fold withdrawn since is the names index not
/// current (the route's retryable 503), and a refold that moved the NAME
/// refuses the former holder and admits the new one. A fold holding the
/// publication past `HOLD_WAIT` is the names index not current too.
#[tokio::test]
async fn the_write_time_name_check_holds_the_real_names_index() {
    use std::sync::Arc;

    use catalyst::chain_index::names::HOLD_WAIT;
    use catalyst::comms::world_identity::WorldName;
    use catalyst::shared::identity::{Address, AuthorityNotEstablished};
    use catalyst::shop::market::names::IndexNames;
    use catalyst::worlds::policy::{PersonalWorldsPolicy, Policy, names_not_current};

    let Some(db) = OwnedDatabase::new("names_fence", Schema::Full, 4).await else {
        return;
    };
    let label = format!("e2e{}", &uuid::Uuid::new_v4().simple().to_string()[..12]);
    let world = WorldName::parse(&format!("{label}.dcl.eth")).unwrap();
    let (holder, next) = (address(HOLDER_KEY), address(STRANGER_KEY));
    mint_name(&db.pool, &label, &holder).await;
    publish_names(&db.pool, 0).await;
    let (holder, next) = (
        Address::parse(&holder).unwrap(),
        Address::parse(&next).unwrap(),
    );
    let policy = Policy::new(db.pool.clone(), PersonalWorldsPolicy::disabled(), None)
        .with_name_authority(Some(Arc::new(IndexNames::new(db.pool.clone()))));
    const WITHDRAW: &str = "UPDATE shop.chain_cursor SET folded_block = NULL, folded_at = NULL \
                            WHERE indexer = 'nft' AND network = 'ethereum'";

    // Confirmed, and held until the write ends: a withdrawal waits for it.
    let mut write = db.pool.begin().await.unwrap();
    policy
        .confirm_name_holder_on(&mut write, &world, &holder)
        .await
        .unwrap();
    let pool = db.pool.clone();
    let withdrawal = tokio::spawn(async move {
        sqlx::query(WITHDRAW).execute(&pool).await.unwrap();
    });
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert!(
        !withdrawal.is_finished(),
        "a withdrawal waits for the write"
    );
    write.commit().await.unwrap();
    tokio::time::timeout(Duration::from_secs(10), withdrawal)
        .await
        .expect("the withdrawal goes through once the write ends")
        .unwrap();

    // Withdrawn: the names index is not current.
    let mut write = db.pool.begin().await.unwrap();
    let refused = policy
        .confirm_name_holder_on(&mut write, &world, &holder)
        .await
        .unwrap_err();
    assert!(names_not_current(&refused).is_some(), "{refused:?}");
    drop(write);

    // Refolded with the NAME moved: the former holder is refused, the new
    // one confirmed.
    sqlx::query("UPDATE shop.nft SET owner = $1 WHERE network = 'ethereum' AND ens_subdomain = $2")
        .bind(next.as_str())
        .bind(&label)
        .execute(&db.pool)
        .await
        .unwrap();
    publish_names(&db.pool, 0).await;
    let mut write = db.pool.begin().await.unwrap();
    let refused = policy
        .confirm_name_holder_on(&mut write, &world, &holder)
        .await
        .unwrap_err();
    assert!(
        matches!(
            refused,
            AuthorityNotEstablished::RefusedLacksAuthority { .. }
        ),
        "{refused:?}"
    );
    drop(write);
    let mut write = db.pool.begin().await.unwrap();
    policy
        .confirm_name_holder_on(&mut write, &world, &next)
        .await
        .unwrap();
    drop(write);

    // A fold holding the publication past the bounded wait: not current.
    let mut fold = db.pool.begin().await.unwrap();
    sqlx::query(
        "SELECT 1 FROM shop.chain_cursor WHERE indexer = 'nft' AND network = 'ethereum' FOR UPDATE",
    )
    .execute(&mut *fold)
    .await
    .unwrap();
    let mut write = db.pool.begin().await.unwrap();
    let refused = tokio::time::timeout(
        HOLD_WAIT * 3,
        policy.confirm_name_holder_on(&mut write, &world, &next),
    )
    .await
    .expect("the check gives up after its bounded wait")
    .unwrap_err();
    assert!(names_not_current(&refused).is_some(), "{refused:?}");
    drop(write);
    fold.rollback().await.unwrap();
    db.drop().await;
}
