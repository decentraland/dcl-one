// from: tests/create_live.rs (the wallet, the node client), tests/social_live.rs (the RPC socket transport), src/ops/admin/tests.rs (the SIWE round trip)
//! The stage 5 operator-plane live checks against a RUNNING node
//! (`tests/ops/live.sh` starts one on its own database with a bounded peer
//! sync attached and runs this twice). Phase `open` (`OPS_WRITES_ENABLED=true`,
//! `TELEMETRY_INGEST_ENABLED=true`): a Sentry envelope and a Segment batch
//! land in `telemetry.events` and the console lists them; an old row is
//! purged through the operator route with its audit row; a random wallet
//! enrols in the credits program over signed fetch, reads its progress,
//! tops up through the mock card and the operator reads its ledger; the relay answers
//! `eth_blockNumber` from mainnet and refuses `eth_sendRawTransaction`; the
//! admin wallet signs in with SIWE, pauses and resumes the sync worker with
//! two audit rows, and reads the job, sync, cache, mirror and audit reports;
//! the imposter and asset-bundle CDN routes answer their shapes. Phase
//! `gated` (both switches off, the same database): every write answers the
//! 503 envelope, the reads still serve, the sync controls and the relay's
//! admin edits stay open by design. Skips unless `CATALYST_OPS_LIVE_URL` is
//! set.
//!
//! Environment (the script exports every one): `CATALYST_OPS_LIVE_URL`,
//! `CATALYST_OPS_LIVE_DATABASE_URL`, `CATALYST_OPS_LIVE_PHASE` (`open` |
//! `gated`), `CATALYST_OPS_LIVE_TELEMETRY_TOKEN`, `CATALYST_OPS_LIVE_CREDITS_TOKEN`,
//! `CATALYST_OPS_LIVE_RPC_TOKEN`, `CATALYST_OPS_LIVE_ADMIN_KEY` (the private key
//! of the one `ADMIN_ADDRESSES` wallet), `CATALYST_OPS_LIVE_SYNC` (`1` when a
//! sync worker is attached, so the controls answer 200 rather than 501).

use std::time::Duration;

use alloy::signers::{Signer, local::PrivateKeySigner};
use reqwest::header::{AUTHORIZATION, CONTENT_TYPE, HeaderMap};
use serde_json::{Value, json};
use sqlx::postgres::PgPoolOptions;
use uuid::Uuid;

use catalyst::auth::{AuthLink, AuthLinkType};
use catalyst::shared::signed_fetch::{
    AUTH_CHAIN_HEADER_PREFIX, AUTH_METADATA_HEADER, AUTH_TIMESTAMP_HEADER, build_payload_v6,
};

const METADATA: &str = r#"{"signer":"dcl:explorer"}"#;
const MANIFEST: &str =
    "/ab-cdn/manifest/bafkreidnionrlsubg63yjdeadaeyk3m5p6gofewq6kfqsju42kfx5cniwu_mac.json";

fn env(name: &str) -> String {
    std::env::var(name).unwrap_or_else(|_| {
        panic!("{name} must be set for the live checks (tests/ops/live.sh exports it)")
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

    /// The signed-fetch header bag as the explorer sends it: a three-link
    /// chain (SIGNER, ECDSA_EPHEMERAL, ECDSA_SIGNED_ENTITY over the 6.x payload).
    async fn signed(&self, method: &str, path: &str) -> Vec<(String, String)> {
        let timestamp = chrono::Utc::now().timestamp_millis().to_string();
        let payload = build_payload_v6(method, path, &timestamp, METADATA);
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
        let chain = [
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
                payload,
                signature: Some(entity_sig),
            },
        ];
        let mut out = vec![
            (AUTH_TIMESTAMP_HEADER.to_string(), timestamp),
            (AUTH_METADATA_HEADER.to_string(), METADATA.to_string()),
        ];
        for (i, link) in chain.iter().enumerate() {
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
        body: Option<(Vec<u8>, &str)>,
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
            Some((json.to_string().into_bytes(), "application/json")),
        )
        .await
    }

    /// A signed request: the wallet signs the path it requests (prefix included).
    async fn signed(&self, w: &Wallet, method: &str, path: &str, json: Option<Value>) -> Answer {
        let route = path.split('?').next().unwrap();
        let headers = w.signed(method, route).await;
        let body = json.map(|j| (j.to_string().into_bytes(), "application/json"));
        self.send(method, path, headers, body).await
    }

    async fn bearer(&self, token: &str, method: &str, path: &str, json: Option<Value>) -> Answer {
        let headers = vec![(AUTHORIZATION.to_string(), format!("Bearer {token}"))];
        let body = json.map(|j| (j.to_string().into_bytes(), "application/json"));
        self.send(method, path, headers, body).await
    }

    async fn cookie(&self, cookie: &str, method: &str, path: &str, json: Option<Value>) -> Answer {
        let headers = vec![("cookie".to_string(), cookie.to_string())];
        let body = json.map(|j| (j.to_string().into_bytes(), "application/json"));
        self.send(method, path, headers, body).await
    }
}

fn is_503_envelope(a: &Answer, message_part: &str) {
    assert_eq!(a.status, 503, "{}", a.body);
    let text = a.body.to_string();
    assert!(
        text.contains(message_part),
        "the envelope names the switch: {text}"
    );
}

/// SIWE sign-in as the admin wallet: the nonce message, the EIP-191
/// signature, the `cat_admin` cookie out of `Set-Cookie`.
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
    assert!(
        message.contains("Nonce:") && message.contains(&admin.address()),
        "{message}"
    );
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
    assert_eq!(v.body["address"], json!(admin.address()));
    let set_cookie = v.header("set-cookie");
    let pair = set_cookie.split(';').next().unwrap().trim().to_string();
    assert!(pair.starts_with("cat_admin="), "{set_cookie}");
    pair
}

struct Live {
    node: Node,
    pool: sqlx::PgPool,
    telemetry_token: String,
    credits_token: String,
    rpc_token: String,
    admin: Wallet,
    sync_attached: bool,
}

async fn live(phase: &str) -> Option<Live> {
    let Ok(url) = std::env::var("CATALYST_OPS_LIVE_URL") else {
        eprintln!("CATALYST_OPS_LIVE_URL unset: skipping the ops live checks");
        return None;
    };
    if std::env::var("CATALYST_OPS_LIVE_PHASE").unwrap_or_else(|_| "open".into()) != phase {
        eprintln!("CATALYST_OPS_LIVE_PHASE is not {phase}: skipping");
        return None;
    }
    let pool = PgPoolOptions::new()
        .max_connections(2)
        .connect(&env("CATALYST_OPS_LIVE_DATABASE_URL"))
        .await
        .expect("the node's database");
    Some(Live {
        node: Node {
            url: url.trim_end_matches('/').to_string(),
            http: reqwest::Client::builder()
                .timeout(Duration::from_secs(120))
                .build()
                .unwrap(),
        },
        pool,
        telemetry_token: env("CATALYST_OPS_LIVE_TELEMETRY_TOKEN"),
        credits_token: env("CATALYST_OPS_LIVE_CREDITS_TOKEN"),
        rpc_token: env("CATALYST_OPS_LIVE_RPC_TOKEN"),
        admin: Wallet::from_key(&env("CATALYST_OPS_LIVE_ADMIN_KEY")),
        sync_attached: std::env::var("CATALYST_OPS_LIVE_SYNC")
            .map(|v| v == "1")
            .unwrap_or(false),
    })
}

async fn count(pool: &sqlx::PgPool, sql: &str) -> i64 {
    sqlx::query_scalar(sqlx::AssertSqlSafe(sql.to_string()))
        .fetch_one(pool)
        .await
        .unwrap()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_operator_plane_end_to_end_against_a_running_node() {
    let Some(l) = live("open").await else { return };
    let node = &l.node;
    let tag = Uuid::new_v4().simple().to_string();
    let project = format!("live{}", &tag[..8]);

    // 1. Telemetry: a Sentry envelope (two items) and a Segment batch (two
    // tracks) land through the bounded writer; the console lists them.
    let envelope = format!(
        "{{\"event_id\":\"{tag}\",\"sent_at\":\"2026-09-25T00:00:00Z\"}}\n\
         {{\"type\":\"event\"}}\n\
         {{\"event_id\":\"{tag}\",\"level\":\"error\",\"message\":\"live check {tag}\",\"release\":\"live\"}}\n\
         {{\"type\":\"session\"}}\n\
         {{\"sid\":\"{tag}\",\"init\":true,\"status\":\"ok\",\"attrs\":{{\"release\":\"live\"}}}}\n"
    );
    let a = node
        .send(
            "post",
            &format!("/telemetry/api/{project}/envelope"),
            Vec::new(),
            Some((envelope.into_bytes(), "application/x-sentry-envelope")),
        )
        .await;
    assert_eq!(a.status, 200, "{}", a.body);
    assert_eq!(a.body["id"], json!(tag));
    let batch = json!({ "writeKey": project, "batch": [
        { "type": "track", "event": "live_check", "userId": "u1", "properties": { "tag": tag } },
        { "type": "track", "event": "client_error", "userId": "u1", "properties": { "tag": tag } },
    ]});
    let a = node
        .post_json("/telemetry/v1/batch", Vec::new(), batch)
        .await;
    assert_eq!(a.status, 200, "{}", a.body);
    assert_eq!(a.body["success"], json!(true));
    let a = node.get("/telemetry/dash/events?hours=1").await;
    assert_eq!(a.status, 403, "no bearer, no cookie: {}", a.body);
    let mut items = Vec::new();
    for _ in 0..50 {
        let a = node
            .bearer(
                &l.telemetry_token,
                "get",
                &format!("/telemetry/dash/events?hours=1&limit=50&q={tag}"),
                None,
            )
            .await;
        assert_eq!(a.status, 200, "{}", a.body);
        items = a.body["items"].as_array().cloned().unwrap_or_default();
        if items.len() >= 3 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
    let sources: Vec<&str> = items.iter().filter_map(|i| i["source"].as_str()).collect();
    assert!(
        sources.contains(&"sentry") && sources.contains(&"segment"),
        "both sinks landed: {items:?}"
    );
    assert!(
        items.iter().all(|i| i["project"] == json!(project)),
        "{items:?}"
    );
    let stored = count(
        &l.pool,
        &format!("SELECT count(*) FROM telemetry.events WHERE project = '{project}'"),
    )
    .await;
    assert_eq!(stored, 4, "two envelope items + two tracks");
    let a = node
        .bearer(
            &l.telemetry_token,
            "get",
            "/telemetry/dash/stats?hours=1",
            None,
        )
        .await;
    assert_eq!(a.status, 200, "{}", a.body);
    let a = node
        .bearer(
            &l.telemetry_token,
            "get",
            &format!("/telemetry/dash/events?hours=1&group=1&q={tag}"),
            None,
        )
        .await;
    assert_eq!(a.status, 200);
    assert!(
        a.body["items"].as_array().is_some_and(|v| !v.is_empty()),
        "the issues aggregate: {}",
        a.body
    );

    // 2. The operator purge over a 100-day-old row, with its audit row; the
    // retention job is attached (its first pass is after one interval).
    sqlx::query(
        "INSERT INTO telemetry.events (received_at, source, project, event_kind, body) \
         VALUES (timezone('UTC', now()) - interval '100 days', 'segment', $1, 'track', '{\"event\":\"old\"}'::jsonb)",
    )
    .bind(&project)
    .execute(&l.pool)
    .await
    .unwrap();
    let a = node
        .bearer(
            &l.telemetry_token,
            "post",
            "/telemetry/dash/admin/purge",
            Some(json!({ "older_than_days": 30, "project": project })),
        )
        .await;
    assert_eq!(a.status, 200, "{}", a.body);
    assert_eq!(a.body["deleted"], json!(1), "{}", a.body);
    let audited = count(&l.pool, &format!("SELECT count(*) FROM ops.admin_audit WHERE domain = 'telemetry' AND action = 'purge' AND target = '{project}'")).await;
    assert_eq!(audited, 1);
    let a = node
        .bearer(
            &l.telemetry_token,
            "get",
            "/telemetry/dash/admin/audit?action=purge&limit=5",
            None,
        )
        .await;
    assert_eq!(a.status, 200, "{}", a.body);
    assert!(a.body.to_string().contains(&project), "{}", a.body);

    // 3. Credits: enrol, progress, the mock card top-up, the balance, the
    // operator's ledger; an unsigned read answers the ADR-44 envelope.
    let w = Wallet::random();
    let a = node.signed(&w, "post", "/credits/users", None).await;
    assert!(
        (200..300).contains(&a.status),
        "enrol: {} {}",
        a.status,
        a.body
    );
    let a = node
        .signed(
            &w,
            "get",
            &format!("/credits/users/{}/progress", w.address()),
            None,
        )
        .await;
    assert_eq!(a.status, 200, "{}", a.body);
    assert_eq!(
        a.body["user"]["hasStartedProgram"],
        json!(true),
        "{}",
        a.body
    );
    let a = node
        .get(&format!("/credits/users/{}/progress", w.address()))
        .await;
    assert_eq!(a.status, 400, "ADR-44 without a chain: {}", a.body);
    let a = node
        .signed(
            &w,
            "post",
            "/credits/topup/mock-card",
            Some(json!({ "credits": "5" })),
        )
        .await;
    assert_eq!(a.status, 200, "{}", a.body);
    assert_eq!(a.body["mock"], json!(true));
    let a = node
        .signed(
            &w,
            "get",
            &format!("/credits/wallet/{}/balance", w.address()),
            None,
        )
        .await;
    assert_eq!(a.status, 200, "{}", a.body);
    let available: f64 = a.body["available"]
        .as_str()
        .unwrap_or("0")
        .parse()
        .unwrap_or(0.0);
    assert!((available - 5.0).abs() < 1e-9, "{}", a.body);
    let a = node
        .signed(
            &w,
            "get",
            &format!("/credits/users/{}/credits", w.address()),
            None,
        )
        .await;
    assert_eq!(a.status, 200, "{}", a.body);
    let a = node
        .get(&format!("/credits/admin/ledger?address={}", w.address()))
        .await;
    assert_eq!(a.status, 403, "{}", a.body);
    let a = node
        .bearer(
            &l.credits_token,
            "get",
            &format!("/credits/admin/ledger?address={}", w.address()),
            None,
        )
        .await;
    assert_eq!(a.status, 200, "{}", a.body);
    assert!(
        a.body.to_string().contains("purchase") || a.body.to_string().contains("mock"),
        "{}",
        a.body
    );
    let a = node.get("/credits/packs").await;
    assert_eq!(a.status, 200, "{}", a.body);
    let a = node.get("/credits/captcha").await;
    assert_eq!(
        a.status, 400,
        "the slider captcha is a signed route too: {}",
        a.body
    );
    let a = node.signed(&w, "get", "/credits/captcha", None).await;
    assert_eq!(a.status, 200, "{}", a.body);
    assert!(
        a.header("content-type").starts_with("image/png"),
        "{}",
        a.header("content-type")
    );
    // The canonical /shop-api paths are the same handlers as the /credits
    // aliases (shop/src/paths.rs): a signed
    // request verifies over the path it signed, the operator routes sit
    // under /shop-api/admin, and a path the Shop UI calls that no handler
    // serves yet answers the unavailable envelope, not the content 404.
    let a = node.get("/shop-api/v1/packs").await;
    assert_eq!(a.status, 200, "{}", a.body);
    assert_eq!(
        a.bytes,
        node.get("/credits/credits/packs").await.bytes,
        "one packs handler, two paths"
    );
    let a = node
        .signed(
            &w,
            "get",
            &format!("/shop-api/v1/users/{}/credits", w.address()),
            None,
        )
        .await;
    assert_eq!(a.status, 200, "{}", a.body);
    assert_eq!(
        a.body,
        node.signed(
            &w,
            "get",
            &format!("/credits/users/{}/credits", w.address()),
            None
        )
        .await
        .body
    );
    let a = node
        .get(&format!("/shop-api/v1/users/{}/progress", w.address()))
        .await;
    assert_eq!(
        a.status, 400,
        "ADR-44 without a chain on the canonical path too: {}",
        a.body
    );
    let a = node
        .bearer(
            &l.credits_token,
            "get",
            &format!("/shop-api/admin/credits/ledger?address={}", w.address()),
            None,
        )
        .await;
    assert_eq!(a.status, 200, "{}", a.body);
    let a = node.get("/shop-api/v1/outfits/all").await;
    assert_eq!(a.status, 503, "{}", a.body);
    assert_eq!(a.body["source"], json!("shop-outfits"), "{}", a.body);
    assert_eq!(a.header("retry-after"), "60");

    // 6. The relay: eth_blockNumber from mainnet, a state-changing method
    // refused, the networks index, the admin edits with their audit rows.
    let a = node
        .post_json(
            "/rpc/mainnet",
            Vec::new(),
            json!({ "jsonrpc": "2.0", "id": 1, "method": "eth_blockNumber", "params": [] }),
        )
        .await;
    assert_eq!(a.status, 200, "{}", a.body);
    let block = a.body["result"].as_str().unwrap_or_default();
    assert!(
        block.starts_with("0x") && u64::from_str_radix(&block[2..], 16).unwrap_or(0) > 20_000_000,
        "{}",
        a.body
    );
    let a = node.post_json("/rpc/mainnet", Vec::new(), json!({ "jsonrpc": "2.0", "id": 2, "method": "eth_sendRawTransaction", "params": ["0x00"] })).await;
    assert_eq!(a.status, 200);
    assert_eq!(a.body["error"]["code"], json!(-32601), "{}", a.body);
    let a = node
        .post_json(
            "/rpc/nope",
            Vec::new(),
            json!({ "jsonrpc": "2.0", "id": 3, "method": "eth_blockNumber", "params": [] }),
        )
        .await;
    assert_eq!(
        a.body["error"]["code"],
        json!(-32602),
        "an unconfigured network: {}",
        a.body
    );
    let a = node.get("/rpc").await;
    assert_eq!(a.status, 200);
    assert!(
        a.body["mainnet"]
            .as_str()
            .is_some_and(|u| u.ends_with("/rpc/mainnet")),
        "{}",
        a.body
    );
    let a = node.get("/rpc/admin/rpc/config").await;
    assert_eq!(a.status, 403);
    let a = node
        .bearer(&l.rpc_token, "get", "/rpc/admin/rpc/config", None)
        .await;
    assert_eq!(a.status, 200, "{}", a.body);
    // The relay admits read methods only (eth_get*).
    let method = format!("eth_getLive{}", &tag[..6]);
    let a = node
        .bearer(
            &l.rpc_token,
            "post",
            "/rpc/admin/rpc/methods",
            Some(json!({ "method": method })),
        )
        .await;
    assert_eq!(a.status, 200, "{}", a.body);
    let a = node
        .bearer(&l.rpc_token, "get", "/rpc/admin/rpc/methods", None)
        .await;
    assert!(a.body.to_string().contains(&method), "{}", a.body);
    let a = node
        .bearer(&l.rpc_token, "post", "/rpc/admin/rpc/methods/reset", None)
        .await;
    assert_eq!(a.status, 200, "{}", a.body);
    let audited = count(
        &l.pool,
        "SELECT count(*) FROM ops.admin_audit WHERE domain = 'rpc'",
    )
    .await;
    assert!(audited >= 2, "the allowlist edit and the reset: {audited}");

    // 7. The operator console: SIWE sign-in, the sync controls with their
    // audit rows, the reports; every route 403 without the cookie.
    let a = node.get("/admin/api/jobs").await;
    assert_eq!(a.status, 403);
    let cookie = admin_cookie(node, &l.admin).await;
    let a = node.cookie(&cookie, "get", "/admin/auth/me", None).await;
    assert_eq!(a.status, 200, "{}", a.body);
    assert_eq!(a.body["address"], json!(l.admin.address()));
    let a = node
        .cookie(&cookie, "post", "/admin/api/content/sync/pause", None)
        .await;
    if l.sync_attached {
        assert_eq!(a.status, 200, "{}", a.body);
        assert_eq!(a.body["body"]["control"], json!("paused"), "{}", a.body);
        let a = node.cookie(&cookie, "get", "/admin/api/sync", None).await;
        assert_eq!(a.body["body"]["control"], json!("paused"), "{}", a.body);
        let a = node
            .cookie(&cookie, "post", "/admin/api/content/sync/resume", None)
            .await;
        assert_eq!(a.status, 200, "{}", a.body);
        assert_eq!(a.body["body"]["control"], json!("run"), "{}", a.body);
        let a = node.cookie(&cookie, "get", "/admin/api/sync", None).await;
        assert_eq!(a.body["body"]["control"], json!("run"), "{}", a.body);
        assert!(
            a.body["body"]["peers"]
                .as_array()
                .is_some_and(|p| !p.is_empty()),
            "the peer cursor: {}",
            a.body
        );
    } else {
        assert_eq!(a.status, 501, "no sync worker: {}", a.body);
    }
    let a = node
        .cookie(
            &cookie,
            "get",
            "/admin/api/audit?domain=content&limit=10",
            None,
        )
        .await;
    assert_eq!(a.status, 200, "{}", a.body);
    let rows = a.body["body"]["rows"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    let actions: Vec<&str> = rows.iter().filter_map(|r| r["action"].as_str()).collect();
    assert!(
        actions.contains(&"sync.pause") && actions.contains(&"sync.resume"),
        "{actions:?}"
    );
    assert!(
        rows.iter().all(|r| r["actor"] == json!(l.admin.address())),
        "the actor is the wallet: {rows:?}"
    );
    let a = node.cookie(&cookie, "get", "/admin/api/jobs", None).await;
    assert_eq!(a.status, 200, "{}", a.body);
    let names: Vec<String> = a.body["body"]["jobs"]
        .as_array()
        .unwrap_or(&vec![])
        .iter()
        .filter_map(|j| j["name"].as_str().map(str::to_string))
        .collect();
    for job in [
        "telemetry-purge",
        "telemetry-ingest-writer",
        "credits-fulfillment",
        "imposters-maintenance",
        "ab-cdn-maintenance",
    ] {
        assert!(
            names.iter().any(|n| n == job),
            "{job} in the job report: {names:?}"
        );
    }
    for path in ["/admin/api/cache", "/admin/api/mirrors"] {
        let a = node.cookie(&cookie, "get", path, None).await;
        assert_eq!(a.status, 200, "{path}: {}", a.body);
    }
    let a = node
        .cookie(&cookie, "post", "/admin/api/content/flush-cache", None)
        .await;
    assert_eq!(a.status, 200, "{}", a.body);
    let a = node
        .cookie(
            &cookie,
            "post",
            "/admin/api/content/failed-deployments/retry",
            Some(json!({ "entityId": "bafynope" })),
        )
        .await;
    assert_eq!(a.status, 404, "{}", a.body);
    let stranger = Wallet::random();
    let a = node
        .get(&format!("/admin/auth/nonce?address={}", stranger.address()))
        .await;
    assert_eq!(
        a.status, 200,
        "the nonce mints for anyone; verify admits the allowlist: {}",
        a.body
    );
    let message = a.body["message"].as_str().unwrap().to_string();
    let signature = stranger
        .root
        .sign_message(message.as_bytes())
        .await
        .unwrap()
        .to_string();
    let a = node
        .post_json(
            "/admin/auth/verify",
            Vec::new(),
            json!({ "message": message, "signature": signature }),
        )
        .await;
    assert_eq!(a.status, 403, "{}", String::from_utf8_lossy(&a.bytes));

    // 8. The imposter supply and the asset-bundle CDN: their shapes.
    let a = node.get("/bvimposters/status").await;
    assert_eq!(a.status, 200, "{}", a.body);
    assert_eq!(a.body["bake_enabled"], json!(false), "{}", a.body);
    assert!(
        a.body.get("readthrough_enabled").is_none(),
        "no upstream imposter store"
    );
    let a = node.get("/bvimposters/ping").await;
    assert_eq!(a.status, 200);
    let a = node
        .get("/bvimposters/imposters/realms/main/9/0,0.1.zip")
        .await;
    assert_eq!(a.status, 404, "a level past 5 is not a key");
    let a = node
        .get("/bvimposters/imposters/realms/main/0/0,0.1.zip")
        .await;
    assert!(
        a.status == 404 || a.status == 200,
        "a store miss or a hit: {}",
        a.status
    );
    eprintln!(
        "imposter 0/0,0.1: {} x-bvi-source={}",
        a.status,
        a.header("x-bvi-source")
    );

    let a = node.get(MANIFEST).await;
    assert_eq!(a.status, 200, "{}", String::from_utf8_lossy(&a.bytes));
    assert_eq!(a.header("x-ab-cdn-source"), "upstream");
    assert!(
        a.header("cache-control").contains("immutable"),
        "{}",
        a.header("cache-control")
    );
    assert!(a.body.is_object() || a.body.is_array(), "a JSON manifest");
    let a = node.get(MANIFEST).await;
    assert_eq!(a.status, 200);
    assert_eq!(
        a.header("x-ab-cdn-source"),
        "store",
        "the second read is the file cache"
    );
    let a = node.send("head", MANIFEST, Vec::new(), None).await;
    assert_eq!(a.status, 200);
    let a = node
        .get("/ab-cdn/v49/assets/bafybeiavdidhzkd4qnzesiwzsh6kjbtyezmwbfcj2w67nminlqmvfjikja_mac")
        .await;
    assert_eq!(
        a.status, 200,
        "the gateway's assets rewrite (boot corpus row 29)"
    );
    let a = node.get("/ab-cdn/nope").await;
    assert_eq!(a.status, 404);
    let a = node.get("/ab-cdn/manifest/does-not-exist_mac.json").await;
    assert_eq!(a.status, 404, "an upstream miss");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_gates_hold_when_the_switches_are_off() {
    let Some(l) = live("gated").await else { return };
    let node = &l.node;

    // Telemetry: the sinks 503 with the ingest message; the console reads
    // serve; every console mutation 503 with Retry-After; /dash/sql stays open.
    let a = node
        .post_json(
            "/telemetry/v1/track",
            Vec::new(),
            json!({ "writeKey": "live", "event": "x" }),
        )
        .await;
    is_503_envelope(&a, "TELEMETRY_INGEST_ENABLED");
    let a = node
        .send(
            "post",
            "/telemetry/api/live/envelope",
            Vec::new(),
            Some((b"{}\n".to_vec(), "application/x-sentry-envelope")),
        )
        .await;
    is_503_envelope(&a, "TELEMETRY_INGEST_ENABLED");
    let a = node
        .bearer(
            &l.telemetry_token,
            "get",
            "/telemetry/dash/events?hours=24&limit=5",
            None,
        )
        .await;
    assert_eq!(a.status, 200, "{}", a.body);
    assert!(
        a.body["items"].as_array().is_some_and(|v| !v.is_empty()),
        "the open phase's rows still serve: {}",
        a.body
    );
    let a = node
        .bearer(
            &l.telemetry_token,
            "post",
            "/telemetry/dash/admin/purge",
            Some(json!({ "older_than_days": 30 })),
        )
        .await;
    is_503_envelope(&a, "OPS_WRITES_ENABLED");
    assert_eq!(a.header("retry-after"), "3600");
    let a = node
        .bearer(
            &l.telemetry_token,
            "post",
            "/telemetry/dash/flag",
            Some(json!({ "name": "x", "enabled": true })),
        )
        .await;
    is_503_envelope(&a, "OPS_WRITES_ENABLED");
    let a = node
        .bearer(
            &l.telemetry_token,
            "post",
            "/telemetry/dash/sql",
            Some(json!({ "sql": "SELECT count(*) AS n FROM telemetry.events" })),
        )
        .await;
    assert_eq!(a.status, 200, "the READ ONLY query stays open: {}", a.body);
    let a = node
        .bearer(
            &l.telemetry_token,
            "post",
            "/telemetry/dash/sql",
            Some(json!({ "sql": "SELECT count(*) AS n FROM credits.wallets" })),
        )
        .await;
    assert_eq!(
        a.status, 400,
        "the reader login sees telemetry alone: {}",
        a.body
    );
    assert!(
        String::from_utf8_lossy(&a.bytes).contains("permission denied"),
        "{}",
        String::from_utf8_lossy(&a.bytes)
    );
    let escape = "select set_config('role', 'none', true) as r, query_to_xml('select string_agg(sku, chr(44)) as skus from credits.packs', true, false, '')::text as x";
    let a = node
        .bearer(
            &l.telemetry_token,
            "post",
            "/telemetry/dash/sql",
            Some(json!({ "sql": escape })),
        )
        .await;
    assert_eq!(
        a.status, 400,
        "the 2026-09-25 recheck's role reset reaches no wider role on the reader login: {}",
        a.body
    );
    assert!(
        String::from_utf8_lossy(&a.bytes).contains("permission denied for schema credits"),
        "{}",
        String::from_utf8_lossy(&a.bytes)
    );
    let a = node
        .bearer(
            &l.telemetry_token,
            "post",
            "/telemetry/dash/sql",
            Some(json!({ "sql": "select current_user::text as u, session_user::text as s" })),
        )
        .await;
    assert_eq!(a.status, 200, "{}", a.body);
    let who = &a.body["rows"][0];
    assert!(
        who["u"] == who["s"] && who["u"].as_str().is_some_and(|u| u.ends_with("_reader")),
        "the statement runs as the reader's own login: {}",
        a.body
    );
    let a = node
        .bearer(
            &l.telemetry_token,
            "get",
            "/telemetry/dash/admin/ingest",
            None,
        )
        .await;
    assert_eq!(a.status, 200, "{}", a.body);
    assert_eq!(
        a.body["bounds"]["max_items_per_request"],
        json!(100),
        "{}",
        a.body
    );

    // Credits: every write 503, the reads and the quote serve.
    let w = Wallet::random();
    let a = node.signed(&w, "post", "/credits/users", None).await;
    is_503_envelope(&a, "OPS_WRITES_ENABLED");
    let a = node
        .signed(
            &w,
            "post",
            "/credits/topup/mock-card",
            Some(json!({ "credits": "5" })),
        )
        .await;
    is_503_envelope(&a, "OPS_WRITES_ENABLED");
    let a = node
        .send(
            "post",
            "/credits/stripe/webhook",
            Vec::new(),
            Some((b"{}".to_vec(), "application/json")),
        )
        .await;
    is_503_envelope(&a, "OPS_WRITES_ENABLED");
    let a = node
        .signed(
            &w,
            "get",
            &format!("/credits/users/{}/progress", w.address()),
            None,
        )
        .await;
    assert_eq!(a.status, 200, "{}", a.body);
    assert_eq!(a.body["user"]["hasStartedProgram"], json!(false));
    let a = node.get("/credits/packs").await;
    assert_eq!(a.status, 200);
    let a = node
        .post_json("/credits/prices/quote", Vec::new(), json!({ "items": [] }))
        .await;
    assert_ne!(a.status, 503, "the quote is a read: {}", a.body);
    // The same gate on the canonical paths: the checkout write 503, the quote read open.
    let a = node
        .signed(
            &w,
            "post",
            "/shop-api/v1/checkouts",
            Some(json!({ "items": [] })),
        )
        .await;
    is_503_envelope(&a, "OPS_WRITES_ENABLED");
    let a = node
        .post_json(
            "/shop-api/v1/credit-quotes",
            Vec::new(),
            json!({ "items": [] }),
        )
        .await;
    assert_ne!(a.status, 503, "the canonical quote is a read: {}", a.body);

    // The relay's admin edits are process state and stay open (audited);
    // the console's sync controls too; its cache flush is gated.
    let a = node
        .bearer(&l.rpc_token, "get", "/rpc/admin/rpc/config", None)
        .await;
    assert_eq!(a.status, 200, "{}", a.body);
    let a = node
        .bearer(&l.rpc_token, "post", "/rpc/admin/rpc/methods/reset", None)
        .await;
    assert_eq!(a.status, 200, "{}", a.body);
    let cookie = admin_cookie(node, &l.admin).await;
    let a = node
        .cookie(&cookie, "post", "/admin/api/content/flush-cache", None)
        .await;
    is_503_envelope(&a, "OPS_WRITES_ENABLED");
    assert_eq!(a.header("retry-after"), "3600");
    let a = node
        .cookie(&cookie, "post", "/admin/api/content/sync/pause", None)
        .await;
    if l.sync_attached {
        assert_eq!(
            a.status, 200,
            "the sync control is open while writes are disabled: {}",
            a.body
        );
        let a = node
            .cookie(&cookie, "post", "/admin/api/content/sync/resume", None)
            .await;
        assert_eq!(a.status, 200, "{}", a.body);
    } else {
        assert_eq!(a.status, 501);
    }
    let a = node.cookie(&cookie, "get", "/admin/api/sync", None).await;
    assert_eq!(a.status, 200, "{}", a.body);
    assert_eq!(a.body["body"]["writesEnabled"], json!(false));
    let a = node
        .cookie(
            &cookie,
            "get",
            "/admin/api/audit?domain=telemetry&action=purge&limit=5",
            None,
        )
        .await;
    assert_eq!(a.status, 200, "{}", a.body);
    assert!(
        a.body["body"]["rows"]
            .as_array()
            .is_some_and(|r| !r.is_empty()),
        "the open phase's purge is in the log: {}",
        a.body
    );
}
