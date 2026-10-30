// from: catalyrst/crates/catalyrst-social-service/tests/contract_gate.rs (the signed-fetch shapes), catalyrst/crates/catalyrst-social-service/src/rpc/upstream/session.rs (the explorer handshake), catalyrst/crates/catalyrst-comms/src/livekit.rs (the HS256 claim check)
//! The stage 5 social live checks against a RUNNING node with social writes
//! enabled (`tests/social/live.sh` starts one and runs this). Two random
//! wallets: one creates a community over the signed-fetch path, the other
//! joins, the owner posts, bans and lists it through `/social-api`; two RPC
//! WebSocket clients exchange a friend request; a private-call LiveKit token
//! is minted through the gatekeeper and its claims verified offline against
//! the configured secret; a notification is broadcast, listed and marked
//! read; a badge preview and a profile face image (seeded as peer sync would
//! leave it) are fetched. Skips unless `CATALYST_SOCIAL_LIVE_URL` is set.
//!
//! Environment (the script exports every one):
//! `CATALYST_SOCIAL_LIVE_URL` (`http://127.0.0.1:5388`),
//! `CATALYST_SOCIAL_LIVE_DATABASE_URL` (the node's database: profile rows are
//! seeded there), `CATALYST_SOCIAL_LIVE_STORAGE_ROOT` (the node's blob root:
//! the face image goes there), `CATALYST_SOCIAL_LIVE_SERVICE_TOKEN`
//! (`COMMS_GATEKEEPER_AUTH_TOKEN`), `CATALYST_SOCIAL_LIVE_ADMIN_TOKEN`
//! (`CATALYRST_NOTIFICATIONS_ADMIN_TOKEN`), `CATALYST_SOCIAL_LIVE_LIVEKIT_KEY`
//! and `_SECRET` (`LIVEKIT_API_KEY` / `LIVEKIT_API_SECRET`).

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use alloy::signers::{Signer, local::PrivateKeySigner};
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use futures::{SinkExt, StreamExt};
use hmac::{Hmac, KeyInit, Mac};
use reqwest::header::{AUTHORIZATION, CONTENT_TYPE};
use serde_json::{Value, json};
use sha2::Sha256;
use sqlx::postgres::PgPoolOptions;
use tokio::sync::Mutex;
use tokio_tungstenite::tungstenite::Message as WsMessage;
use uuid::Uuid;

use catalyst::auth::hash::hash_bytes_v1;
use catalyst::auth::{AuthLink, AuthLinkType};
use catalyst::shared::signed_fetch::{
    AUTH_CHAIN_HEADER_PREFIX, AUTH_METADATA_HEADER, AUTH_TIMESTAMP_HEADER, build_payload_v6,
};
use catalyst::social::rpc::drpc::client::{RpcClient, RpcClientModule, ServiceClient};
use catalyst::social::rpc::drpc::stream_protocol::Generator;
use catalyst::social::rpc::drpc::transport::{Transport, TransportError, TransportMessage};
use catalyst::social::rpc::proto::v2::*;
use catalyst::social::rpc::service::MODULE_NAME;
use catalyst::storage::Storage;

const METADATA: &str = r#"{"signer":"dcl:explorer"}"#;
/// A 1x1 opaque PNG.
const PNG_1X1: &[u8] = &[
    0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44, 0x52,
    0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x02, 0x00, 0x00, 0x00, 0x90, 0x77, 0x53,
    0xDE, 0x00, 0x00, 0x00, 0x0C, 0x49, 0x44, 0x41, 0x54, 0x08, 0xD7, 0x63, 0xF8, 0xCF, 0xC0, 0x00,
    0x00, 0x03, 0x01, 0x01, 0x00, 0x18, 0xDD, 0x8D, 0xB0, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4E,
    0x44, 0xAE, 0x42, 0x60, 0x82,
];

fn env(name: &str) -> String {
    std::env::var(name).unwrap_or_else(|_| {
        panic!("{name} must be set for the live checks (tests/social/live.sh exports it)")
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

    fn address(&self) -> String {
        format!("{:#x}", self.root.address()).to_lowercase()
    }

    /// The signed-fetch header bag as the explorer sends it: a three-link chain
    /// (SIGNER, ECDSA_EPHEMERAL, ECDSA_SIGNED_ENTITY over the 6.x payload).
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

    /// The socket's first frame: the same header bag as JSON, signed over `get:<path>`.
    async fn handshake_frame(&self, path: &str) -> String {
        let headers = self.signed("get", path).await;
        let map: serde_json::Map<String, Value> = headers
            .into_iter()
            .map(|(k, v)| (k, Value::String(v)))
            .collect();
        Value::Object(map).to_string()
    }
}

struct Node {
    url: String,
    http: reqwest::Client,
}

impl Node {
    async fn send(
        &self,
        method: &str,
        path: &str,
        headers: Vec<(String, String)>,
        body: Option<(Vec<u8>, &str)>,
    ) -> (u16, Value, reqwest::header::HeaderMap) {
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
        let bytes = resp.bytes().await.unwrap();
        let value = serde_json::from_slice(&bytes)
            .unwrap_or(Value::String(String::from_utf8_lossy(&bytes).into()));
        (status, value, headers)
    }

    async fn signed(
        &self,
        w: &Wallet,
        method: &str,
        path: &str,
        json: Option<Value>,
    ) -> (u16, Value) {
        let route = path.split('?').next().unwrap();
        let headers = w.signed(method, route).await;
        let body = json.map(|v| (v.to_string().into_bytes(), "application/json"));
        let (status, value, _) = self.send(method, path, headers, body).await;
        (status, value)
    }
}

fn multipart(fields: &[(&str, &str)]) -> (Vec<u8>, String) {
    let boundary = format!("----catalyst{}", Uuid::new_v4().simple());
    let mut body = Vec::new();
    for (name, value) in fields {
        body.extend_from_slice(
            format!(
                "--{boundary}\r\nContent-Disposition: form-data; name=\"{name}\"\r\n\r\n{value}\r\n"
            )
            .as_bytes(),
        );
    }
    body.extend_from_slice(format!("--{boundary}--\r\n").as_bytes());
    (body, format!("multipart/form-data; boundary={boundary}"))
}

fn contains_str(v: &Value, needle: &str) -> bool {
    match v {
        Value::String(s) => s.eq_ignore_ascii_case(needle),
        Value::Array(a) => a.iter().any(|x| contains_str(x, needle)),
        Value::Object(m) => m.values().any(|x| contains_str(x, needle)),
        _ => false,
    }
}

/// An active profile entity for the wallet with a claimed name and, when
/// `face` is given, the `face256.png` content file peer sync would have
/// stored; returns the entity id.
async fn seed_profile(pool: &sqlx::PgPool, w: &Wallet, name: &str, face: Option<&str>) -> String {
    let metadata = json!({ "v": { "avatars": [{
        "name": name, "hasClaimedName": true,
        "avatar": { "bodyShape": "urn:decentraland:off-chain:base-avatars:BaseFemale", "wearables": [], "snapshots": { "face256": face.unwrap_or("") } },
    }] } });
    // Entity ids and content hashes are CIDs everywhere (the routes refuse anything else).
    let entity = hash_bytes_v1(format!("{metadata}{}", Uuid::new_v4()).as_bytes());
    let (id,): (i32,) = sqlx::query_as(
        "INSERT INTO content.deployments (deployer_address, version, entity_type, entity_id, entity_metadata, \
             entity_timestamp, entity_pointers, local_timestamp, auth_chain) \
         VALUES ($1, 'v3', 'profile', $2, $3, timezone('UTC', now()), ARRAY[$1]::text[], timezone('UTC', now()), '[]'::json) RETURNING id",
    )
    .bind(w.address())
    .bind(&entity)
    .bind(metadata)
    .fetch_one(pool)
    .await
    .unwrap();
    if let Some(hash) = face {
        sqlx::query("INSERT INTO content.content_files (deployment, content_hash, key) VALUES ($1, $2, 'face256.png')")
            .bind(id)
            .bind(hash)
            .execute(pool)
            .await
            .unwrap();
    }
    entity
}

// ── the socket client (the lib tests' transport, over a public URL) ─────────

type WsStream =
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

struct TungsteniteTransport {
    read: Mutex<futures::stream::SplitStream<WsStream>>,
    write: Mutex<futures::stream::SplitSink<WsStream, WsMessage>>,
}

#[async_trait::async_trait]
impl Transport for TungsteniteTransport {
    async fn receive(&self) -> Result<TransportMessage, TransportError> {
        loop {
            match self.read.lock().await.next().await {
                Some(Ok(WsMessage::Binary(b))) => return Ok(b.to_vec()),
                Some(Ok(WsMessage::Close(_))) | None => return Err(TransportError::Closed),
                Some(Ok(_)) => continue,
                Some(Err(e)) => return Err(TransportError::Internal(Box::new(e))),
            }
        }
    }

    async fn send(&self, message: TransportMessage) -> Result<(), TransportError> {
        self.write
            .lock()
            .await
            .send(WsMessage::Binary(message.into()))
            .await
            .map_err(|_| TransportError::Closed)
    }

    async fn close(&self) {
        let _ = self.write.lock().await.close().await;
    }
}

struct SocialClient {
    module: RpcClientModule<TungsteniteTransport>,
}

impl ServiceClient<TungsteniteTransport> for SocialClient {
    fn set_client_module(module: RpcClientModule<TungsteniteTransport>) -> Self {
        Self { module }
    }
}

async fn explorer(node: &Node, w: &Wallet) -> (RpcClient<TungsteniteTransport>, SocialClient) {
    let ws_url = format!("{}/social-rpc", node.url.replacen("http", "ws", 1));
    let (mut socket, _) = tokio_tungstenite::connect_async(&ws_url)
        .await
        .unwrap_or_else(|e| panic!("{ws_url}: {e}"));
    socket
        .send(WsMessage::Text(
            w.handshake_frame("/social-rpc").await.into(),
        ))
        .await
        .unwrap();
    let (write, read) = socket.split();
    let transport = TungsteniteTransport {
        read: Mutex::new(read),
        write: Mutex::new(write),
    };
    let mut client = RpcClient::new(transport).await.expect("SERVER_READY");
    let port = client.create_port("social").await.expect("port");
    let social: SocialClient = port.load_module(MODULE_NAME).await.expect("module");
    (client, social)
}

async fn next_update<T: Send + Sync + 'static>(generator: &mut Generator<T>) -> T {
    tokio::time::timeout(Duration::from_secs(10), generator.next())
        .await
        .expect("an update within 10s")
        .expect("stream open")
}

fn verify_hs256(jwt: &str, secret: &str) -> Value {
    let parts: Vec<&str> = jwt.split('.').collect();
    assert_eq!(parts.len(), 3, "a JWT has three parts: {jwt}");
    let header: Value = serde_json::from_slice(&URL_SAFE_NO_PAD.decode(parts[0]).unwrap()).unwrap();
    assert_eq!(header["alg"], json!("HS256"));
    let mut mac = Hmac::<Sha256>::new_from_slice(secret.as_bytes()).unwrap();
    mac.update(format!("{}.{}", parts[0], parts[1]).as_bytes());
    mac.verify_slice(&URL_SAFE_NO_PAD.decode(parts[2]).unwrap())
        .expect("the token is signed with LIVEKIT_API_SECRET");
    serde_json::from_slice(&URL_SAFE_NO_PAD.decode(parts[1]).unwrap()).unwrap()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_social_slice_end_to_end_against_a_running_node() {
    let Ok(url) = std::env::var("CATALYST_SOCIAL_LIVE_URL") else {
        eprintln!("CATALYST_SOCIAL_LIVE_URL unset: skipping the social live checks");
        return;
    };
    let node = Node {
        url: url.trim_end_matches('/').to_string(),
        http: reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .unwrap(),
    };
    let pool = PgPoolOptions::new()
        .max_connections(2)
        .connect(&env("CATALYST_SOCIAL_LIVE_DATABASE_URL"))
        .await
        .expect("the node's database");
    let storage = Arc::new(
        Storage::from_config(Path::new(&env("CATALYST_SOCIAL_LIVE_STORAGE_ROOT")))
            .await
            .expect("the node's blob root"),
    );
    let service_token = env("CATALYST_SOCIAL_LIVE_SERVICE_TOKEN");
    let admin_token = env("CATALYST_SOCIAL_LIVE_ADMIN_TOKEN");
    let livekit_key = env("CATALYST_SOCIAL_LIVE_LIVEKIT_KEY");
    let livekit_secret = env("CATALYST_SOCIAL_LIVE_LIVEKIT_SECRET");

    let alice = Wallet::random();
    let bob = Wallet::random();
    let tag = Uuid::new_v4().simple().to_string();
    let face_hash = hash_bytes_v1(PNG_1X1);
    storage
        .store(&face_hash, bytes::Bytes::from_static(PNG_1X1))
        .await
        .unwrap();
    let alice_profile = seed_profile(
        &pool,
        &alice,
        &format!("alice{}", &tag[..6]),
        Some(&face_hash),
    )
    .await;
    seed_profile(&pool, &bob, &format!("bob{}", &tag[..6]), None).await;

    // 1. Community: create (owner, claimed name), join, post, ban, list; every call through /social-api.
    let name = format!("live {}", &tag[..8]);
    let (body, ct) = multipart(&[
        ("name", &name),
        ("description", "stage 5 live check"),
        ("privacy", "public"),
        ("visibility", "all"),
    ]);
    let (status, created, _) = node
        .send(
            "POST",
            "/social-api/v1/communities",
            alice.signed("post", "/social-api/v1/communities").await,
            Some((body, &ct)),
        )
        .await;
    assert_eq!(status, 201, "create: {created}");
    let community = created["data"]["id"].as_str().unwrap().to_string();
    assert_eq!(created["data"]["ownerAddress"], json!(alice.address()));

    let members = format!("/social-api/v1/communities/{community}/members");
    let (status, joined) = node.signed(&bob, "post", &members, None).await;
    assert!((200..300).contains(&status), "join: {status} {joined}");
    let (status, listed) = node.signed(&alice, "get", &members, None).await;
    assert_eq!(status, 200, "members: {listed}");
    assert!(
        contains_str(&listed, &bob.address()),
        "bob is a member: {listed}"
    );

    let posts = format!("/social-api/v1/communities/{community}/posts");
    let (status, posted) = node
        .signed(
            &alice,
            "post",
            &posts,
            Some(json!({ "content": "hello from the live check" })),
        )
        .await;
    assert!((200..300).contains(&status), "post: {status} {posted}");
    let (status, post_list) = node.signed(&bob, "get", &posts, None).await;
    assert_eq!(status, 200, "posts: {post_list}");
    assert!(
        contains_str(&post_list, "hello from the live check"),
        "{post_list}"
    );

    let ban = format!(
        "/social-api/v1/communities/{community}/members/{}/bans",
        bob.address()
    );
    let (status, banned) = node.signed(&alice, "post", &ban, None).await;
    assert!((200..300).contains(&status), "ban: {status} {banned}");
    let bans = format!("/social-api/v1/communities/{community}/bans");
    let (status, ban_list) = node.signed(&alice, "get", &bans, None).await;
    assert_eq!(status, 200, "bans: {ban_list}");
    assert!(
        contains_str(&ban_list, &bob.address()),
        "bob is banned: {ban_list}"
    );
    let (status, after) = node.signed(&alice, "get", &members, None).await;
    assert_eq!(status, 200);
    assert!(
        !contains_str(&after, &bob.address()),
        "a banned member is gone from the roster: {after}"
    );
    let (status, rejoin) = node.signed(&bob, "post", &members, None).await;
    assert!(
        status == 401 || status == 403,
        "a banned wallet cannot rejoin: {status} {rejoin}"
    );

    let search = format!(
        "/social-api/v1/communities?search={}&limit=10&offset=0",
        urlencoding::encode(&name)
    );
    let (status, list) = node.signed(&alice, "get", &search, None).await;
    assert_eq!(status, 200, "list: {list}");
    assert!(
        contains_str(&list, &community),
        "the listing carries the community: {list}"
    );
    let (status, detail) = node
        .signed(
            &bob,
            "get",
            &format!("/social-api/v1/communities/{community}"),
            None,
        )
        .await;
    assert_eq!(status, 200, "detail: {detail}");
    assert_eq!(detail["data"]["name"], json!(name));

    // 2. Two RPC WebSocket clients exchange a friend request.
    let (_alice_client, alice_social) = explorer(&node, &alice).await;
    let (_bob_client, bob_social) = explorer(&node, &bob).await;
    let mut bob_updates: Generator<FriendshipUpdate> = bob_social
        .module
        .call_server_streams_procedure("SubscribeToFriendshipUpdates", ())
        .await
        .expect("subscribe");
    let response: UpsertFriendshipResponse = alice_social
        .module
        .call_unary_procedure(
            "UpsertFriendship",
            UpsertFriendshipPayload {
                action: Some(upsert_friendship_payload::Action::Request(
                    upsert_friendship_payload::RequestPayload {
                        user: Some(User {
                            address: bob.address(),
                        }),
                        message: Some("hi bob".into()),
                    },
                )),
            },
        )
        .await
        .expect("UpsertFriendship");
    let Some(upsert_friendship_response::Response::Accepted(accepted)) = response.response else {
        panic!("{response:?}")
    };
    assert_eq!(accepted.friend.as_ref().unwrap().address, bob.address());
    let update = next_update(&mut bob_updates).await;
    let Some(friendship_update::Update::Request(request)) = update.update else {
        panic!("{update:?}")
    };
    assert_eq!(request.friend.as_ref().unwrap().address, alice.address());
    assert_eq!(request.message.as_deref(), Some("hi bob"));
    let response: UpsertFriendshipResponse = bob_social
        .module
        .call_unary_procedure(
            "UpsertFriendship",
            UpsertFriendshipPayload {
                action: Some(upsert_friendship_payload::Action::Accept(
                    upsert_friendship_payload::AcceptPayload {
                        user: Some(User {
                            address: alice.address(),
                        }),
                    },
                )),
            },
        )
        .await
        .expect("accept");
    assert!(
        matches!(
            response.response,
            Some(upsert_friendship_response::Response::Accepted(_))
        ),
        "{response:?}"
    );
    let status: GetFriendshipStatusResponse = alice_social
        .module
        .call_unary_procedure(
            "GetFriendshipStatus",
            GetFriendshipStatusPayload {
                user: Some(User {
                    address: bob.address(),
                }),
            },
        )
        .await
        .expect("status");
    let Some(get_friendship_status_response::Response::Accepted(accepted)) = status.response else {
        panic!("{status:?}")
    };
    assert_eq!(accepted.status, FriendshipStatus::Accepted as i32);
    let friends: PaginatedFriendsProfilesResponse = bob_social
        .module
        .call_unary_procedure("GetFriends", GetFriendsPayload { pagination: None })
        .await
        .expect("friends");
    assert!(
        friends.friends.iter().any(|f| f.address == alice.address()),
        "{friends:?}"
    );
    let (status, rest_friends) = node
        .signed(&alice, "get", "/social-api/v1/friends", None)
        .await;
    assert_eq!(status, 200, "{rest_friends}");
    assert!(
        contains_str(&rest_friends, &bob.address()),
        "the REST friends list agrees: {rest_friends}"
    );

    // 3. A private-call LiveKit token through the gatekeeper, claims verified offline.
    let call_id = Uuid::new_v4();
    let (status, minted, _) = node
        .send(
            "POST",
            "/comms-gatekeeper/private-voice-chat",
            vec![(AUTHORIZATION.to_string(), format!("Bearer {service_token}"))],
            Some((json!({ "room_id": call_id.to_string(), "user_addresses": [alice.address(), bob.address()] }).to_string().into_bytes(), "application/json")),
        )
        .await;
    assert_eq!(status, 200, "mint: {minted}");
    let connection_url = minted[alice.address()]["connection_url"]
        .as_str()
        .unwrap_or_else(|| panic!("{minted}"));
    let jwt = connection_url
        .split("access_token=")
        .nth(1)
        .expect("livekit:<ws>?access_token=<jwt>");
    let claims = verify_hs256(jwt, &livekit_secret);
    assert_eq!(claims["iss"], json!(livekit_key));
    assert_eq!(claims["sub"], json!(alice.address()));
    assert_eq!(
        claims["video"]["room"],
        json!(format!("voice-chat-private-{call_id}"))
    );
    assert_eq!(claims["video"]["roomJoin"], json!(true));
    assert_eq!(claims["video"]["canPublish"], json!(true));
    assert_eq!(claims["video"]["canUpdateOwnMetadata"], json!(false));
    assert!(
        claims["exp"].as_i64().unwrap() > chrono::Utc::now().timestamp(),
        "{claims}"
    );
    let (status, presence, _) = node
        .send(
            "GET",
            &format!(
                "/comms-gatekeeper/users/{}/voice-chat-status",
                alice.address()
            ),
            vec![(AUTHORIZATION.to_string(), format!("Bearer {service_token}"))],
            None,
        )
        .await;
    assert_eq!(status, 200, "{presence}");
    assert_eq!(
        presence["is_user_in_voice_chat"],
        json!(true),
        "the mint recorded both participants: {presence}"
    );
    let (status, ended, _) = node
        .send(
            "DELETE",
            &format!("/comms-gatekeeper/private-voice-chat/{call_id}"),
            vec![(AUTHORIZATION.to_string(), format!("Bearer {service_token}"))],
            Some((
                json!({ "address": alice.address() })
                    .to_string()
                    .into_bytes(),
                "application/json",
            )),
        )
        .await;
    assert_eq!(status, 200, "end: {ended}");

    // 4. A notification delivered to Alice, listed and marked read.
    let (status, broadcast, _) = node
        .send(
            "POST",
            "/notifications/notifications/broadcast",
            vec![(AUTHORIZATION.to_string(), format!("Bearer {admin_token}"))],
            Some((json!({ "type": "badge_granted", "metadata": { "badgeId": "welcome", "tag": tag }, "addresses": [alice.address()] }).to_string().into_bytes(), "application/json")),
        )
        .await;
    assert_eq!(status, 200, "broadcast: {broadcast}");
    assert_eq!(broadcast["recipients"], json!(1));
    let (status, unread) = node
        .signed(
            &alice,
            "get",
            "/notifications/notifications?onlyUnread=true&limit=20",
            None,
        )
        .await;
    assert_eq!(status, 200, "{unread}");
    let mine: Vec<&Value> = unread["notifications"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|n| n["metadata"]["tag"] == json!(tag))
        .collect();
    assert_eq!(mine.len(), 1, "{unread}");
    let id = mine[0]["id"].as_str().unwrap().to_string();
    let (status, read) = node
        .signed(
            &alice,
            "put",
            "/notifications/notifications/read",
            Some(json!({ "notificationIds": [id] })),
        )
        .await;
    assert_eq!(status, 200, "{read}");
    assert_eq!(read, json!({ "updated": 1 }));
    let (status, unread) = node
        .signed(
            &alice,
            "get",
            "/notifications/notifications?onlyUnread=true&limit=20",
            None,
        )
        .await;
    assert_eq!(status, 200);
    assert!(
        !unread["notifications"]
            .as_array()
            .unwrap()
            .iter()
            .any(|n| n["metadata"]["tag"] == json!(tag)),
        "read: {unread}"
    );

    // 5. A badge preview and the profile face image.
    let (status, preview, _) = node
        .send(
            "GET",
            &format!("/badges/users/{}/preview", alice.address()),
            vec![],
            None,
        )
        .await;
    assert_eq!(status, 200, "{preview}");
    assert!(
        preview.get("data").is_some() || preview.is_object(),
        "{preview}"
    );
    let (status, categories, _) = node.send("GET", "/badges/categories", vec![], None).await;
    assert_eq!(status, 200, "{categories}");
    let (status, _, headers) = node
        .send(
            "GET",
            &format!("/profile-images/entities/{alice_profile}/face.png"),
            vec![],
            None,
        )
        .await;
    assert_eq!(status, 200, "face.png");
    assert_eq!(headers.get(CONTENT_TYPE).unwrap(), "image/png");

    pool.close().await;
}
