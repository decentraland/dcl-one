// from: tests/create_live.rs (the wallet, the node client), tests/ops_live.rs (the SIWE sign-in), catalyrst/crates/catalyrst-server/tests/land_publish.rs (publish_roundtrip_owner_deploys_stranger_denied_then_unpublish)
//! The content server's local write path against a RUNNING node
//! (`tests/content/live.sh` starts one on its own database, the LAND index
//! seeded with the owner's three parcels, and runs this twice).
//!
//! Phase `open` (`CONTENT_WRITES_ENABLED=true`, `OPS_WRITES_ENABLED=true`):
//! the LAND owner deploys a signed scene over multipart with an ephemeral
//! chain (`POST /content/entities`), `/content/entities/active` returns it,
//! `/content/contents/{hash}` serves its files, `/audit` carries its local
//! provenance; a random wallet is refused on the owner's other parcel; the
//! admin wallet signs in with SIWE, denylists the scene's file (the node
//! stops serving it and refuses a deploy naming it), lists it, lifts it,
//! toggles read-only (403 on the next deploy) and accepting-users (`/about`),
//! refreshes the challenge; the stranger cannot unpublish, the owner can,
//! and the parcel is empty; a second scene's file is left denylisted for the
//! next phase. Phase `gated` (the same database, both switches off, a fresh
//! process): `POST /entities` and `DELETE /scenes/{coord}` answer the 503
//! envelope with `Retry-After`, a console denylist edit the ops 503, the
//! reads still serve, and the file denylisted in phase `open` is still
//! withheld (the durable denylist survived the restart). Skips unless
//! `CATALYST_CONTENT_LIVE_URL` is set.
//!
//! Environment (the script exports every one): `CATALYST_CONTENT_LIVE_URL`,
//! `CATALYST_CONTENT_LIVE_PHASE` (`open` | `gated`),
//! `CATALYST_CONTENT_LIVE_OWNER_KEY` (the wallet the index says owns the
//! parcels), `CATALYST_CONTENT_LIVE_PARCELS` (three, `x,y;x,y;x,y`),
//! `CATALYST_CONTENT_LIVE_ADMIN_KEY` (the one `ADMIN_ADDRESSES` wallet),
//! `CATALYST_CONTENT_LIVE_STATE` (a file phase `open` writes the withheld
//! hash into for phase `gated`). Phase `bench` (`BENCH=N` in the script,
//! after `open`, writes on) times `POST /content/entities` and `DELETE
//! /scenes/{coord}` over the N parcels of
//! `CATALYST_CONTENT_LIVE_BENCH_PARCELS` and prints the percentiles.

use alloy::signers::{Signer, local::PrivateKeySigner};
use reqwest::header::{CONTENT_TYPE, HeaderMap};
use serde_json::{Value, json};

use catalyst::auth::hash_bytes_v1;
use catalyst::auth::{AuthLink, AuthLinkType};
use catalyst::shared::signed_fetch::{
    AUTH_CHAIN_HEADER_PREFIX, AUTH_METADATA_HEADER, AUTH_TIMESTAMP_HEADER, build_payload_v6,
};

const METADATA: &str = r#"{"signer":"dcl:explorer"}"#;

fn env(name: &str) -> String {
    std::env::var(name).unwrap_or_else(|_| {
        panic!("{name} must be set for the live checks (tests/content/live.sh exports it)")
    })
}

struct Wallet {
    root: PrivateKeySigner,
    ephemeral: PrivateKeySigner,
}

impl Wallet {
    fn random() -> Self {
        Self {
            root: PrivateKeySigner::random(),
            ephemeral: PrivateKeySigner::random(),
        }
    }

    fn from_key(key: &str) -> Self {
        Self {
            root: key.parse().unwrap(),
            ephemeral: PrivateKeySigner::random(),
        }
    }

    fn address(&self) -> String {
        format!("{:#x}", self.root.address()).to_lowercase()
    }

    /// SIGNER, ECDSA_EPHEMERAL, ECDSA_SIGNED_ENTITY over `payload`: the
    /// chain an explorer or the SDK builds.
    async fn chain(&self, payload: &str) -> Vec<AuthLink> {
        let ephemeral_payload = format!(
            "Decentraland Login\nEphemeral address: {:#x}\nExpiration: 2099-01-01T00:00:00.000Z",
            self.ephemeral.address()
        );
        let ephemeral_sig = self
            .root
            .sign_message(ephemeral_payload.as_bytes())
            .await
            .unwrap()
            .to_string();
        let entity_sig = self
            .ephemeral
            .sign_message(payload.as_bytes())
            .await
            .unwrap()
            .to_string();
        vec![
            AuthLink {
                link_type: AuthLinkType::SIGNER,
                payload: format!("{:#x}", self.root.address()),
                signature: None,
            },
            AuthLink {
                link_type: AuthLinkType::EcdsaEphemeral,
                payload: ephemeral_payload,
                signature: Some(ephemeral_sig),
            },
            AuthLink {
                link_type: AuthLinkType::EcdsaSignedEntity,
                payload: payload.to_string(),
                signature: Some(entity_sig),
            },
        ]
    }

    /// The signed-fetch headers over the 6.x payload of `method path`.
    async fn signed(&self, method: &str, path: &str) -> Vec<(String, String)> {
        let timestamp = chrono::Utc::now().timestamp_millis().to_string();
        let payload = build_payload_v6(method, path, &timestamp, METADATA);
        let mut out = vec![
            (AUTH_TIMESTAMP_HEADER.to_string(), timestamp),
            (AUTH_METADATA_HEADER.to_string(), METADATA.to_string()),
        ];
        for (i, link) in self.chain(&payload).await.iter().enumerate() {
            out.push((
                format!("{AUTH_CHAIN_HEADER_PREFIX}{i}"),
                serde_json::to_string(link).unwrap(),
            ));
        }
        out
    }
}

struct Node {
    url: String,
    http: reqwest::Client,
}

struct Answer {
    status: u16,
    body: Value,
    bytes: Vec<u8>,
    headers: HeaderMap,
}

impl Answer {
    fn header(&self, name: &str) -> String {
        self.headers
            .get(name)
            .map(|v| v.to_str().unwrap_or("").to_string())
            .unwrap_or_default()
    }
}

impl Node {
    async fn send(
        &self,
        method: &str,
        path: &str,
        headers: Vec<(String, String)>,
        body: Option<(Vec<u8>, String)>,
    ) -> Answer {
        let mut req = self.http.request(
            method.to_uppercase().parse().unwrap(),
            format!("{}{path}", self.url),
        );
        for (k, v) in headers {
            req = req.header(k, v);
        }
        if let Some((bytes, ct)) = body {
            req = req.header(CONTENT_TYPE, ct).body(bytes);
        }
        let resp = req
            .send()
            .await
            .unwrap_or_else(|e| panic!("{method} {path}: {e}"));
        let status = resp.status().as_u16();
        let headers = resp.headers().clone();
        let bytes = resp.bytes().await.unwrap().to_vec();
        let body = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
        Answer {
            status,
            body,
            bytes,
            headers,
        }
    }

    async fn get(&self, path: &str) -> Answer {
        self.send("get", path, Vec::new(), None).await
    }

    async fn post_json(&self, path: &str, headers: Vec<(String, String)>, json: Value) -> Answer {
        self.send(
            "post",
            path,
            headers,
            Some((json.to_string().into_bytes(), "application/json".into())),
        )
        .await
    }

    async fn active(&self, pointer: &str) -> Value {
        let a = self
            .post_json(
                "/content/entities/active",
                Vec::new(),
                json!({ "pointers": [pointer] }),
            )
            .await;
        assert_eq!(a.status, 200, "{}", a.body);
        a.body
    }

    /// `POST /content/entities` as the SDK sends it: `entityId`, the chain
    /// as indexed `authChain[i][...]` fields, every file named by its hash.
    async fn deploy(&self, signer: &Wallet, entity: &Value, files: &[&[u8]]) -> (Answer, String) {
        let (body, content_type, id) = deploy_body(signer, entity, files).await;
        (
            self.send(
                "post",
                "/content/entities",
                Vec::new(),
                Some((body, content_type)),
            )
            .await,
            id,
        )
    }

    async fn unpublish(&self, signer: &Wallet, parcel: &str) -> Answer {
        let path = format!("/content/scenes/{parcel}");
        let headers = signer.signed("delete", &path).await;
        self.send("delete", &path, headers, None).await
    }

    async fn console(&self, cookie: &str, path: &str, json: Value) -> Answer {
        self.post_json(path, vec![("cookie".to_string(), cookie.to_string())], json)
            .await
    }
}

/// The multipart body of a signed deploy, its content type and the entity
/// id: built (and signed) before a timed request.
async fn deploy_body(
    signer: &Wallet,
    entity: &Value,
    files: &[&[u8]],
) -> (Vec<u8>, String, String) {
    let entity_bytes = serde_json::to_vec(entity).unwrap();
    let id = hash_bytes_v1(&entity_bytes);
    let chain = signer.chain(&id).await;
    let boundary = format!("live-{}", uuid::Uuid::new_v4().simple());
    let mut body = Vec::new();
    let mut text = |name: &str, value: &str| {
        body.extend_from_slice(
            format!(
                "--{boundary}\r\nContent-Disposition: form-data; name=\"{name}\"\r\n\r\n{value}\r\n"
            )
            .as_bytes(),
        );
    };
    text("entityId", &id);
    for (i, link) in chain.iter().enumerate() {
        let link = serde_json::to_value(link).unwrap();
        text(
            &format!("authChain[{i}][type]"),
            link["type"].as_str().unwrap(),
        );
        text(
            &format!("authChain[{i}][payload]"),
            link["payload"].as_str().unwrap(),
        );
        text(
            &format!("authChain[{i}][signature]"),
            link["signature"].as_str().unwrap_or(""),
        );
    }
    for file in std::iter::once(entity_bytes.as_slice()).chain(files.iter().copied()) {
        let hash = hash_bytes_v1(file);
        body.extend_from_slice(
            format!("--{boundary}\r\nContent-Disposition: form-data; name=\"{hash}\"; filename=\"{hash}\"\r\nContent-Type: application/octet-stream\r\n\r\n").as_bytes(),
        );
        body.extend_from_slice(file);
        body.extend_from_slice(b"\r\n");
    }
    body.extend_from_slice(format!("--{boundary}--\r\n").as_bytes());
    (
        body,
        format!("multipart/form-data; boundary={boundary}"),
        id,
    )
}

fn scene(parcel: &str, files: &[(&str, &[u8])]) -> Value {
    json!({
        "version": "v3",
        "type": "scene",
        "pointers": [parcel],
        "timestamp": chrono::Utc::now().timestamp_millis(),
        "content": files.iter().map(|(name, bytes)| json!({ "file": name, "hash": hash_bytes_v1(bytes) })).collect::<Vec<_>>(),
        "metadata": {
            "display": { "title": "content live" },
            "main": "bin/index.js",
            "scene": { "base": parcel, "parcels": [parcel] },
        },
    })
}

/// SIWE sign-in as the admin wallet; the `cat_admin` cookie.
async fn admin_cookie(node: &Node, admin: &Wallet) -> String {
    let a = node
        .get(&format!("/admin/auth/nonce?address={}", admin.address()))
        .await;
    assert_eq!(
        a.status,
        200,
        "nonce: {}",
        String::from_utf8_lossy(&a.bytes)
    );
    let message = a.body["message"].as_str().expect("message").to_string();
    let signature = admin
        .root
        .sign_message(message.as_bytes())
        .await
        .unwrap()
        .to_string();
    let v = node
        .post_json(
            "/admin/auth/verify",
            Vec::new(),
            json!({ "message": message, "signature": signature }),
        )
        .await;
    assert_eq!(
        v.status,
        200,
        "verify: {}",
        String::from_utf8_lossy(&v.bytes)
    );
    v.header("set-cookie")
        .split(';')
        .next()
        .unwrap()
        .trim()
        .to_string()
}

async fn phase_open(
    node: &Node,
    owner: &Wallet,
    admin: &Wallet,
    parcels: &[String],
    state_file: &str,
) {
    // One parcel per pointer TTL: a refused attempt still takes its
    // pointer's 20 s slot (the deployer records before it validates).
    let (parcel, neighbour, third) = (&parcels[0], &parcels[1], &parcels[2]);
    let stranger = Wallet::random();
    let about = node.get("/about").await;
    assert_eq!(
        (about.status, about.body["acceptingUsers"].clone()),
        (200, json!(true)),
        "{}",
        about.body
    );

    // The owner deploys; the node serves it back.
    let code = format!(
        "export function onUpdate() {{ /* {} */ }}",
        uuid::Uuid::new_v4()
    )
    .into_bytes();
    let (a, id) = node
        .deploy(owner, &scene(parcel, &[("bin/index.js", &code)]), &[&code])
        .await;
    assert_eq!(a.status, 200, "owner deploy: {}", a.body);
    assert!(a.body["creationTimestamp"].as_i64().unwrap() > 1_700_000_000_000);
    let active = node.active(parcel).await;
    assert_eq!(active[0]["id"], json!(id), "{active}");
    assert_eq!(active[0]["content"][0]["hash"], json!(hash_bytes_v1(&code)));
    let served = node
        .get(&format!("/content/contents/{}", hash_bytes_v1(&code)))
        .await;
    assert_eq!((served.status, served.bytes.clone()), (200, code.clone()));
    let entity_file = node.get(&format!("/contents/{id}")).await;
    assert_eq!(entity_file.status, 200);
    assert_eq!(
        hash_bytes_v1(&entity_file.bytes),
        id,
        "the entity file is served byte for byte"
    );
    let audit = node.get(&format!("/content/audit/scene/{id}")).await;
    assert_eq!(
        audit.body["localProvenance"]["status"], "active",
        "{}",
        audit.body
    );
    assert_eq!(
        audit.body["localProvenance"]["signer"],
        json!(owner.address())
    );
    println!("deployed {id} at {parcel}");

    // A stranger holds no LAND at the owner's other parcel.
    let (a, _) = node.deploy(&stranger, &scene(neighbour, &[]), &[]).await;
    assert_eq!(a.status, 400, "{}", a.body);
    assert!(
        a.body["errors"]
            .to_string()
            .contains("does not have access"),
        "{}",
        a.body
    );

    // The console: the durable denylist, the toggles, the challenge.
    let cookie = admin_cookie(node, admin).await;
    let code_hash = hash_bytes_v1(&code);
    let a = node
        .console(
            &cookie,
            "/admin/api/content/denylist/add",
            json!({ "id": code_hash }),
        )
        .await;
    assert_eq!(
        (a.status, a.body["body"].clone()),
        (200, json!({ "added": true })),
        "{}",
        a.body
    );
    assert_eq!(
        node.get(&format!("/content/contents/{code_hash}"))
            .await
            .status,
        404,
        "a listed hash is not served"
    );
    let (a, _) = node
        .deploy(
            owner,
            &scene(neighbour, &[("bin/index.js", &code)]),
            &[&code],
        )
        .await;
    assert_eq!(a.status, 400);
    assert_eq!(
        a.body["errors"],
        json!([format!(
            "These content files are denylisted on this node: {code_hash}"
        )])
    );
    let a = node
        .console(&cookie, "/admin/api/content/denylist/list", json!({}))
        .await;
    assert!(
        a.body["body"]["ids"]
            .as_array()
            .unwrap()
            .contains(&json!(code_hash)),
        "{}",
        a.body
    );
    let a = node
        .console(
            &cookie,
            "/admin/api/content/denylist/remove",
            json!({ "entityId": code_hash }),
        )
        .await;
    assert_eq!(a.body["body"], json!({ "removed": true }));
    assert_eq!(
        node.get(&format!("/content/contents/{code_hash}"))
            .await
            .status,
        200
    );

    let a = node
        .console(
            &cookie,
            "/admin/api/content/read-only",
            json!({ "enabled": true }),
        )
        .await;
    assert_eq!(a.body["body"], json!({ "readOnly": true }));
    let (a, _) = node.deploy(owner, &scene(neighbour, &[]), &[]).await;
    assert_eq!(
        (a.status, String::from_utf8_lossy(&a.bytes).to_string()),
        (
            403,
            "Content Server is in read-only mode; deployments are disabled".to_string()
        )
    );
    node.console(
        &cookie,
        "/admin/api/content/read-only",
        json!({ "enabled": false }),
    )
    .await;
    let a = node
        .console(
            &cookie,
            "/admin/api/content/accepting-users",
            json!({ "enabled": false }),
        )
        .await;
    assert_eq!(a.body["body"], json!({ "acceptingUsers": false }));
    assert_eq!(
        node.get("/about").await.body["acceptingUsers"],
        json!(false)
    );
    node.console(
        &cookie,
        "/admin/api/content/accepting-users",
        json!({ "enabled": true }),
    )
    .await;
    assert_eq!(node.get("/about").await.body["acceptingUsers"], json!(true));
    let a = node
        .console(&cookie, "/admin/api/content/challenge/refresh", json!({}))
        .await;
    assert!(
        a.body["body"]["challenge"]
            .as_str()
            .unwrap()
            .starts_with("dcl-crypto-"),
        "{}",
        a.body
    );
    let audit = node
        .send(
            "get",
            "/admin/api/audit?domain=content&limit=20",
            vec![("cookie".into(), cookie.clone())],
            None,
        )
        .await;
    let actions: Vec<String> = audit.body["body"]["rows"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["action"].as_str().unwrap().to_string())
        .collect();
    for action in [
        "denylist.add",
        "denylist.remove",
        "denylist.list",
        "read-only",
        "accepting-users",
        "challenge.refresh",
    ] {
        assert!(
            actions.iter().any(|a| a == action),
            "{action} audited: {actions:?}"
        );
    }

    // Unpublish: the stranger cannot, the owner can, once.
    let a = node.unpublish(&stranger, parcel).await;
    assert_eq!(a.status, 403, "{}", a.body);
    let a = node.unpublish(owner, parcel).await;
    assert_eq!(a.status, 200, "{}", a.body);
    assert_eq!(
        a.body,
        json!({ "entityId": id, "unpublished": true, "parcels": [parcel] })
    );
    assert_eq!(node.active(parcel).await, json!([]));
    assert_eq!(
        node.get(&format!("/content/audit/scene/{id}")).await.body["localProvenance"]["status"],
        "unpublished"
    );
    assert_eq!(node.unpublish(owner, parcel).await.status, 404);

    // Left for the next phase: a denylisted file of a deployed scene.
    let kept = format!("kept {}", uuid::Uuid::new_v4()).into_bytes();
    let (a, kept_id) = node
        .deploy(owner, &scene(third, &[("kept.txt", &kept)]), &[&kept])
        .await;
    assert_eq!(a.status, 200, "{}", a.body);
    let kept_hash = hash_bytes_v1(&kept);
    node.console(
        &cookie,
        "/admin/api/content/denylist/add",
        json!({ "id": kept_hash }),
    )
    .await;
    std::fs::write(
        state_file,
        json!({ "hash": kept_hash, "entity": kept_id, "parcel": third }).to_string(),
    )
    .unwrap();
    println!("left {kept_hash} denylisted for the gated phase");
}

async fn phase_gated(
    node: &Node,
    owner: &Wallet,
    admin: &Wallet,
    parcels: &[String],
    state_file: &str,
) {
    let left: Value = serde_json::from_str(&std::fs::read_to_string(state_file).unwrap()).unwrap();
    let (a, _) = node.deploy(owner, &scene(&parcels[0], &[]), &[]).await;
    assert_eq!(a.status, 503, "{}", a.body);
    assert_eq!(a.header("retry-after"), "3600");
    assert!(
        a.body["error"]
            .as_str()
            .unwrap()
            .contains("CONTENT_WRITES_ENABLED=false"),
        "{}",
        a.body
    );
    let a = node
        .unpublish(owner, left["parcel"].as_str().unwrap())
        .await;
    assert_eq!(a.status, 503, "{}", a.body);
    let cookie = admin_cookie(node, admin).await;
    let a = node
        .console(
            &cookie,
            "/admin/api/content/denylist/remove",
            json!({ "id": left["hash"] }),
        )
        .await;
    assert_eq!(
        a.status, 503,
        "a durable console edit follows OPS_WRITES_ENABLED: {}",
        a.body
    );
    assert_eq!(a.header("retry-after"), "3600");

    // The reads serve, and the denylist survived the restart.
    let active = node.active(left["parcel"].as_str().unwrap()).await;
    assert_eq!(active[0]["id"], left["entity"], "{active}");
    let hash = left["hash"].as_str().unwrap();
    assert_eq!(
        node.get(&format!("/content/contents/{hash}")).await.status,
        404,
        "the durable denylist survived the restart"
    );
    let a = node
        .console(&cookie, "/admin/api/content/denylist/list", json!({}))
        .await;
    assert!(
        a.body["body"]["ids"]
            .as_array()
            .unwrap()
            .contains(&left["hash"]),
        "{}",
        a.body
    );
}

/// Phase `bench`: the deploy latency of the local write path (signed
/// multipart, the content validator with its LAND leg over the index, the
/// deploy function) for one scene per parcel of
/// `CATALYST_CONTENT_LIVE_BENCH_PARCELS`, sequential, then the unpublish of
/// each. Signing happens before the clock starts; the time is the request's.
async fn phase_bench(node: &Node, owner: &Wallet, parcels: &[String]) {
    fn summary(label: &str, mut ms: Vec<f64>) {
        ms.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let at = |p: f64| ms[((p * ms.len() as f64).ceil() as usize).clamp(1, ms.len()) - 1];
        let total: f64 = ms.iter().sum();
        println!(
            "bench {label}: n={} p50={:.1}ms p95={:.1}ms p99={:.1}ms max={:.1}ms mean={:.1}ms sequential={:.1}/s",
            ms.len(),
            at(0.50),
            at(0.95),
            at(0.99),
            ms[ms.len() - 1],
            total / ms.len() as f64,
            ms.len() as f64 * 1000.0 / total
        );
    }
    let mut deploys = Vec::with_capacity(parcels.len());
    for (i, parcel) in parcels.iter().enumerate() {
        let code = format!(
            "export function onUpdate() {{ /* bench {i} {} */ }}",
            uuid::Uuid::new_v4()
        )
        .into_bytes();
        let asset: Vec<u8> = (0..4096u32)
            .map(|b| (b.wrapping_mul(2654435761).wrapping_add(i as u32) >> 24) as u8)
            .collect();
        let entity = scene(
            parcel,
            &[("bin/index.js", &code), ("assets/blob.bin", &asset)],
        );
        let (body, content_type, _) = deploy_body(owner, &entity, &[&code, &asset]).await;
        let t0 = std::time::Instant::now();
        let a = node
            .send(
                "post",
                "/content/entities",
                Vec::new(),
                Some((body, content_type)),
            )
            .await;
        deploys.push(t0.elapsed().as_secs_f64() * 1000.0);
        assert_eq!(a.status, 200, "bench deploy at {parcel}: {}", a.body);
    }
    let mut unpublishes = Vec::with_capacity(parcels.len());
    for parcel in parcels {
        let path = format!("/content/scenes/{parcel}");
        let headers = owner.signed("delete", &path).await;
        let t0 = std::time::Instant::now();
        let a = node.send("delete", &path, headers, None).await;
        unpublishes.push(t0.elapsed().as_secs_f64() * 1000.0);
        assert_eq!(a.status, 200, "bench unpublish at {parcel}: {}", a.body);
    }
    summary(
        "POST /content/entities (scene, 3 files, LAND index leg)",
        deploys,
    );
    summary("DELETE /content/scenes/{coord}", unpublishes);
}

#[tokio::test]
async fn content_write_path_live() {
    let Ok(url) = std::env::var("CATALYST_CONTENT_LIVE_URL") else {
        eprintln!("skipping: CATALYST_CONTENT_LIVE_URL not set (tests/content/live.sh runs this)");
        return;
    };
    let node = Node {
        url: url.trim_end_matches('/').to_string(),
        http: reqwest::Client::new(),
    };
    let owner = Wallet::from_key(&env("CATALYST_CONTENT_LIVE_OWNER_KEY"));
    let admin = Wallet::from_key(&env("CATALYST_CONTENT_LIVE_ADMIN_KEY"));
    let parcels: Vec<String> = env("CATALYST_CONTENT_LIVE_PARCELS")
        .split(';')
        .map(str::to_string)
        .collect();
    let state_file = env("CATALYST_CONTENT_LIVE_STATE");
    match env("CATALYST_CONTENT_LIVE_PHASE").as_str() {
        "open" => phase_open(&node, &owner, &admin, &parcels, &state_file).await,
        "gated" => phase_gated(&node, &owner, &admin, &parcels, &state_file).await,
        "bench" => {
            let bench: Vec<String> = env("CATALYST_CONTENT_LIVE_BENCH_PARCELS")
                .split(';')
                .map(str::to_string)
                .collect();
            phase_bench(&node, &owner, &bench).await
        }
        other => panic!("unknown phase {other}"),
    }
}
