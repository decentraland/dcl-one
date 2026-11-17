// from: tests/social_live.rs (the wallet, the node client and the multipart helper), catalyrst/crates/catalyrst-builder/src/ports/publication/tests.rs (the wearable vector), catalyrst/crates/catalyrst-camera-reel/src/handlers/tests (the upload metadata shape)
//! The stage 5 creator-plane live checks against a RUNNING node with
//! `CREATE_WRITES_ENABLED=true` (`tests/create/live.sh` starts one, builds
//! the asset catalogue with `catalyst builder catalog` and runs this). A
//! random wallet saves a collection and the old publication vector's item
//! over signed fetch, uploads its model and thumbnail (served back from the
//! ONE blob store), reads its drafts, a curator approves the item (the
//! operator bearer and the `BUILDER_ADMIN_ADDRESSES` wallet), and the
//! prepared publication matches the vector byte for byte
//! (`1:w:Hat:A blue hat:hat:BaseFemale,BaseMale`, `DCL-MYCLLCTN`, chain
//! 137, the salt keccak256 of the id). The asset pack the script built is
//! listed by `/v1/assetPacks` and its model is served by `/contents/{hash}`.
//! Camera reel: the wallet uploads a photo with its metadata, lists it as
//! owner (private) and as anyone (after the visibility flip), fetches the
//! bytes and the thumbnail, a stranger is refused, the moderator rejects a
//! second photo, and a delete leaves the hash unservable. Skips unless
//! `CATALYST_CREATE_LIVE_URL` is set.
//!
//! Environment (the script exports every one):
//! `CATALYST_CREATE_LIVE_URL` (`http://127.0.0.1:5432`),
//! `CATALYST_CREATE_LIVE_BUILDER_TOKEN` (`CATALYRST_BUILDER_ADMIN_TOKEN`),
//! `CATALYST_CREATE_LIVE_CAMERA_TOKEN` (`CATALYRST_CAMERA_REEL_ADMIN_TOKEN`),
//! `CATALYST_CREATE_LIVE_CURATOR_KEY` (the private key of the one
//! `BUILDER_ADMIN_ADDRESSES` wallet), `CATALYST_CREATE_LIVE_PACKS_DIR` (the
//! `@dcl/asset-packs`-shaped tree the catalogue was built from).

use std::path::Path;

use alloy::signers::{Signer, local::PrivateKeySigner};
use alloy_primitives::keccak256;
use reqwest::header::{AUTHORIZATION, CONTENT_TYPE, HeaderMap};
use serde_json::{Value, json};
use uuid::Uuid;

use catalyst::auth::hash_bytes_v1;
use catalyst::auth::{AuthLink, AuthLinkType};
use catalyst::shared::signed_fetch::{
    AUTH_CHAIN_HEADER_PREFIX, AUTH_METADATA_HEADER, AUTH_TIMESTAMP_HEADER, build_payload_v6,
};

const METADATA: &str = r#"{"signer":"dcl:explorer"}"#;
const BUILDER: &str = "/builder-api";
const REEL: &str = "/camera-reel-service";

fn env(name: &str) -> String {
    std::env::var(name).unwrap_or_else(|_| {
        panic!("{name} must be set for the live checks (tests/create/live.sh exports it)")
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

    /// A signed request: the wallet signs the path it requests (prefix included).
    async fn signed(
        &self,
        w: &Wallet,
        method: &str,
        path: &str,
        body: Option<(Vec<u8>, &str)>,
    ) -> Answer {
        let route = path.split('?').next().unwrap();
        let headers = w.signed(method, route).await;
        self.send(method, path, headers, body).await
    }

    async fn signed_json(&self, w: &Wallet, method: &str, path: &str, json: Value) -> Answer {
        self.signed(
            w,
            method,
            path,
            Some((json.to_string().into_bytes(), "application/json")),
        )
        .await
    }

    async fn bearer(&self, token: &str, method: &str, path: &str, json: Value) -> Answer {
        let headers = vec![(AUTHORIZATION.to_string(), format!("Bearer {token}"))];
        self.send(
            method,
            path,
            headers,
            Some((json.to_string().into_bytes(), "application/json")),
        )
        .await
    }
}

struct Part<'a> {
    name: &'a str,
    filename: Option<&'a str>,
    content_type: Option<&'a str>,
    bytes: &'a [u8],
}

fn multipart(parts: &[Part<'_>]) -> (Vec<u8>, String) {
    let boundary = format!("----catalyst{}", Uuid::new_v4().simple());
    let mut body = Vec::new();
    for p in parts {
        body.extend_from_slice(
            format!(
                "--{boundary}\r\nContent-Disposition: form-data; name=\"{}\"",
                p.name
            )
            .as_bytes(),
        );
        if let Some(f) = p.filename {
            body.extend_from_slice(format!("; filename=\"{f}\"").as_bytes());
        }
        body.extend_from_slice(b"\r\n");
        if let Some(ct) = p.content_type {
            body.extend_from_slice(format!("Content-Type: {ct}\r\n").as_bytes());
        }
        body.extend_from_slice(b"\r\n");
        body.extend_from_slice(p.bytes);
        body.extend_from_slice(b"\r\n");
    }
    body.extend_from_slice(format!("--{boundary}--\r\n").as_bytes());
    (body, format!("multipart/form-data; boundary={boundary}"))
}

/// A `w` x `h` PNG whose pixels depend on `seed`, so two uploads never share a hash.
fn png(w: u32, h: u32, seed: u8) -> Vec<u8> {
    let img = image::RgbaImage::from_fn(w, h, |x, y| {
        image::Rgba([(x as u8).wrapping_mul(seed), (y as u8) ^ seed, seed, 255])
    });
    let mut out = std::io::Cursor::new(Vec::new());
    img.write_to(&mut out, image::ImageFormat::Png).unwrap();
    out.into_inner()
}

fn photo_metadata(address: &str, place_id: &str) -> Value {
    json!({
        "userName": "tester",
        "userAddress": address,
        "dateTime": "1700000000",
        "realm": "main",
        "scene": { "name": "Genesis Plaza", "location": { "x": "0", "y": "0" } },
        "visiblePeople": [],
        "placeId": place_id
    })
}

#[tokio::test]
async fn the_creator_plane_end_to_end_against_a_running_node() {
    let Ok(url) = std::env::var("CATALYST_CREATE_LIVE_URL") else {
        eprintln!("CATALYST_CREATE_LIVE_URL unset: skipping the creator-plane live checks");
        return;
    };
    let node = Node {
        url: url.trim_end_matches('/').to_string(),
        http: reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .unwrap(),
    };
    let builder_token = env("CATALYST_CREATE_LIVE_BUILDER_TOKEN");
    let camera_token = env("CATALYST_CREATE_LIVE_CAMERA_TOKEN");
    let curator = Wallet::from_key(&env("CATALYST_CREATE_LIVE_CURATOR_KEY"));
    let packs_dir = env("CATALYST_CREATE_LIVE_PACKS_DIR");
    let owner = Wallet::random();
    let stranger = Wallet::random();

    // ── the asset catalogue the operator command built ──────────────────────
    let packs = node.get(&format!("{BUILDER}/v1/assetPacks")).await;
    assert_eq!(packs.status, 200, "{}", packs.body);
    assert_eq!(packs.header("cache-control"), "public, max-age=3600");
    assert_eq!(packs.body["ok"], true);
    let pack = packs.body["data"]
        .as_array()
        .and_then(|p| p.iter().find(|p| p["id"] == "live-pack"))
        .cloned()
        .unwrap_or_else(|| panic!("live-pack missing: {}", packs.body));
    assert_eq!(pack["title"], "Live pack");
    let asset = &pack["assets"][0];
    assert_eq!(
        (asset["asset_pack_id"].as_str(), asset["model"].as_str()),
        (Some("live-pack"), Some("model.glb"))
    );
    let model_bytes =
        std::fs::read(Path::new(&packs_dir).join("live-pack/assets/cube/model.glb")).unwrap();
    let model_hash = hash_bytes_v1(&model_bytes);
    assert_eq!(
        asset["contents"]["model.glb"], model_hash,
        "the catalogue names the CIDv1 of the stored file"
    );
    let served = node.get(&format!("{BUILDER}/contents/{model_hash}")).await;
    assert_eq!(served.status, 200);
    assert_eq!(
        served.bytes, model_bytes,
        "the asset file is served from the ONE blob store"
    );
    assert_eq!(
        node.get(&format!("/contents/{model_hash}")).await.status,
        200,
        "the same hash through the content server's root route"
    );

    // ── a collection and the publication vector's item, over signed fetch ───
    let collection = Uuid::new_v4();
    let item = Uuid::new_v4();
    let c_path = format!("{BUILDER}/v1/collections/{collection}");
    let saved = node
        .signed_json(
            &owner,
            "put",
            &c_path,
            json!({ "collection": { "id": collection, "name": "My collection" } }),
        )
        .await;
    assert_eq!(saved.status, 200, "{}", saved.body);
    assert_eq!(
        (
            saved.body["ok"].as_bool(),
            saved.body["data"]["name"].as_str(),
            saved.body["data"]["is_published"].as_bool()
        ),
        (Some(true), Some("My collection"), Some(false))
    );
    assert_eq!(
        saved.body["data"]["eth_address"]
            .as_str()
            .map(str::to_lowercase)
            .as_deref(),
        Some(owner.address().as_str())
    );
    let forged = node
        .signed_json(
            &stranger,
            "put",
            &c_path,
            json!({ "collection": { "id": collection, "name": "Stolen" } }),
        )
        .await;
    assert!(
        matches!(forged.status, 401 | 403),
        "another wallet cannot save the owner's draft: {} {}",
        forged.status,
        forged.body
    );
    let unsigned = node
        .send(
            "put",
            &c_path,
            Vec::new(),
            Some((
                json!({ "collection": { "id": collection, "name": "x" } })
                    .to_string()
                    .into_bytes(),
                "application/json",
            )),
        )
        .await;
    assert_eq!(
        unsigned.status, 400,
        "no chain is the ADR-44 400: {}",
        unsigned.body
    );
    assert_eq!(unsigned.body["ok"], false);

    let mut draft = json!({
        "id": item, "name": "Hat", "collection_id": collection, "type": "wearable",
        "description": "A blue hat", "rarity": "rare", "price": "1000000000000000001", "beneficiary": owner.address(),
        "thumbnail": "thumbnail.png",
        "data": { "category": "hat", "representations": [{ "mainFile": "model.glb", "contents": ["model.glb"],
            "bodyShapes": ["urn:decentraland:off-chain:base-avatars:BaseFemale", "urn:decentraland:off-chain:base-avatars:BaseMale"] }] },
        "metrics": { "triangles": 12, "materials": 1, "meshes": 1, "bodies": 1, "entities": 1, "textures": 1 }
    });
    let i_path = format!("{BUILDER}/v1/items/{item}");
    let saved = node
        .signed_json(&owner, "put", &i_path, json!({ "item": draft }))
        .await;
    assert_eq!(saved.status, 200, "{}", saved.body);
    assert_eq!(saved.body["data"]["name"], "Hat");

    // Files first (the FK rule: a draft may only name a hash this node accepted).
    let glb = format!("glTF-live-{}", Uuid::new_v4()).into_bytes();
    let thumb = png(64, 64, 7);
    let (body, ct) = multipart(&[
        Part {
            name: "model.glb",
            filename: Some("model.glb"),
            content_type: Some("model/gltf-binary"),
            bytes: &glb,
        },
        Part {
            name: "thumbnail.png",
            filename: Some("thumbnail.png"),
            content_type: Some("image/png"),
            bytes: &thumb,
        },
    ]);
    let uploaded = node
        .signed(
            &owner,
            "post",
            &format!("{i_path}/files"),
            Some((body, &ct)),
        )
        .await;
    assert_eq!(uploaded.status, 200, "{}", uploaded.body);
    let glb_hash = hash_bytes_v1(&glb);
    let thumb_hash = hash_bytes_v1(&thumb);
    assert_eq!(uploaded.body["data"]["model.glb"], glb_hash);
    assert_eq!(uploaded.body["data"]["thumbnail.png"], thumb_hash);

    let stored = node
        .get(&format!("{BUILDER}/v1/storage/contents/{thumb_hash}"))
        .await;
    assert_eq!(stored.status, 200);
    assert_eq!(
        stored.bytes, thumb,
        "the thumbnail is served back from the ONE blob store"
    );
    assert_eq!(stored.header("content-type"), "image/png");
    assert_eq!(
        node.get(&format!("/v1/storage/contents/{thumb_hash}/exists"))
            .await
            .status,
        200,
        "the root path (the old builder-api vhost) answers too"
    );
    assert_eq!(
        node.get(&format!(
            "{BUILDER}/v1/storage/contents/{}/exists",
            hash_bytes_v1(b"never uploaded")
        ))
        .await
        .status,
        404
    );

    let forged = json!({ "item": { "id": item, "name": "Hat", "collection_id": collection, "type": "wearable", "data": {}, "contents": { "missing.glb": hash_bytes_v1(b"never uploaded") } } });
    assert_eq!(
        node.signed_json(&owner, "put", &i_path, forged)
            .await
            .status,
        400,
        "a hash this node never accepted is refused"
    );
    draft["contents"] = json!({ "model.glb": glb_hash, "thumbnail.png": thumb_hash });
    let saved = node
        .signed_json(&owner, "put", &i_path, json!({ "item": draft }))
        .await;
    assert_eq!(saved.status, 200, "{}", saved.body);
    let read = node.signed(&owner, "get", &i_path, None).await;
    assert_eq!(read.status, 200);
    assert_eq!(read.body["data"]["contents"]["thumbnail.png"], thumb_hash);
    assert_eq!(read.body["data"]["description"], "A blue hat");
    assert_eq!(
        node.signed(&stranger, "get", &i_path, None).await.status,
        404,
        "another wallet has no such draft"
    );

    let listing = node
        .signed(&owner, "get", &format!("{BUILDER}/v1/collections"), None)
        .await;
    assert_eq!(listing.status, 200, "{}", listing.body);
    let mine = listing.body["data"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["id"] == collection.to_string())
        .cloned()
        .unwrap();
    assert_eq!(mine["item_count"], 1);
    assert_eq!(
        node.signed(&stranger, "get", &c_path, None).await.status,
        401,
        "a non-owner, non-admin collection read is refused"
    );
    let items = node
        .signed(&owner, "get", &format!("{c_path}/items"), None)
        .await;
    assert_eq!(items.status, 200, "{}", items.body);
    assert_eq!(items.body["data"][0]["is_approved"], false);
    assert_eq!(items.body["data"][0]["contents"]["model.glb"], glb_hash);

    // ── curation: the operator bearer, then the admin-address wallet ────────
    let status_path = format!("{c_path}/items/{item}/status");
    let refused = node
        .signed_json(
            &stranger,
            "patch",
            &status_path,
            json!({ "status": "approved" }),
        )
        .await;
    assert!(
        matches!(refused.status, 401 | 403),
        "a wallet outside BUILDER_ADMIN_ADDRESSES cannot curate: {}",
        refused.status
    );
    let refused = node
        .bearer(
            "wrong",
            "patch",
            &status_path,
            json!({ "status": "approved" }),
        )
        .await;
    assert!(
        matches!(refused.status, 401 | 403),
        "a wrong bearer cannot curate: {}",
        refused.status
    );
    let approved = node
        .bearer(
            &builder_token,
            "patch",
            &status_path,
            json!({ "status": "approved" }),
        )
        .await;
    assert_eq!(approved.status, 200, "{}", approved.body);
    assert_eq!(
        (
            approved.body["data"]["status"].as_str(),
            approved.body["data"]["updated"].as_u64()
        ),
        (Some("approved"), Some(1))
    );
    let items = node
        .signed(
            &owner,
            "get",
            &format!("{c_path}/items?page=1&limit=10"),
            None,
        )
        .await;
    assert_eq!(items.status, 200);
    assert_eq!(
        (
            items.body["data"]["total"].as_i64(),
            items.body["data"]["results"][0]["is_approved"].as_bool()
        ),
        (Some(1), Some(true))
    );
    let bulk = node
        .signed_json(
            &curator,
            "patch",
            &format!("{c_path}/items/status"),
            json!({ "status": "pending", "itemIds": [item] }),
        )
        .await;
    assert_eq!(
        bulk.status, 200,
        "the BUILDER_ADMIN_ADDRESSES wallet curates over signed fetch: {}",
        bulk.body
    );
    assert_eq!(bulk.body["data"]["updated"], 1);
    let by_admin = node.signed(&curator, "get", &c_path, None).await;
    assert_eq!(
        by_admin.status, 200,
        "an admin address reads any collection"
    );
    let curation = node
        .bearer(
            &builder_token,
            "get",
            &format!("{BUILDER}/v1/collections/curation"),
            json!({}),
        )
        .await;
    assert_eq!(curation.status, 200, "{}", curation.body);
    assert!(
        curation.body["data"]["committee"].is_array()
            && curation.body["data"]["collections"].is_array(),
        "empty lists without a collections subgraph"
    );

    // ── the prepared publication is the old vector ──────────────────────────
    let prepared = node
        .signed(&owner, "get", &format!("{c_path}/publication"), None)
        .await;
    assert_eq!(prepared.status, 200, "{}", prepared.body);
    let p = &prepared.body["data"];
    assert_eq!(
        p["items"][0]["metadata"],
        "1:w:Hat:A blue hat:hat:BaseFemale,BaseMale"
    );
    assert_eq!(p["items"][0]["price"], "1000000000000000001");
    assert_eq!(p["items"][0]["rarity"], "rare");
    assert_eq!(
        p["items"][0]["beneficiary"]
            .as_str()
            .map(str::to_lowercase)
            .as_deref(),
        Some(owner.address().as_str())
    );
    assert_eq!(p["symbol"], "DCL-MYCLLCTN");
    assert_eq!(p["chain_id"], 137);
    assert_eq!(p["manager"], "0x9d32aac179153a991e832550d9f96441ea27763a");
    assert_eq!(p["factory"], "0x3195e88ae10704b359764cb38e429d24f1c2f781");
    assert_eq!(p["forwarder"], "0xbf6755a83c0dcdbb2933a96ea778e00b717d7004");
    assert_eq!(
        p["base_uri"],
        "https://peer.decentraland.org/lambdas/collections/standard/erc721/"
    );
    assert_eq!(
        p["salt"],
        format!("{:#x}", keccak256(collection.to_string()))
    );
    assert_eq!(
        p["creator"].as_str().map(str::to_lowercase).as_deref(),
        Some(owner.address().as_str())
    );
    assert!(
        p["revision"].as_str().unwrap().starts_with("bafkrei"),
        "the revision is the CIDv1 of the snapshot"
    );
    assert_eq!(
        node.signed(&stranger, "get", &format!("{c_path}/publication"), None)
            .await
            .status,
        404,
        "another wallet has no such collection"
    );
    let begun = node
        .signed_json(
            &owner,
            "post",
            &format!("{c_path}/publication"),
            json!({ "revision": p["revision"] }),
        )
        .await;
    assert_eq!(
        begun.status, 503,
        "no BUILDER_POLYGON_RPC_URL: the chain half answers 503 ({})",
        begun.body
    );
    let status = node
        .signed(&owner, "get", &format!("{c_path}/publication/status"), None)
        .await;
    assert_eq!(status.status, 200, "{}", status.body);

    // ── camera reel ─────────────────────────────────────────────────────────
    let place = Uuid::new_v4().to_string();
    let shot = png(1280, 720, 3);
    let (body, ct) = multipart(&[
        Part {
            name: "image",
            filename: Some("shot.png"),
            content_type: Some("image/png"),
            bytes: &shot,
        },
        Part {
            name: "metadata",
            filename: None,
            content_type: Some("application/json"),
            bytes: photo_metadata(&owner.address(), &place)
                .to_string()
                .as_bytes(),
        },
        Part {
            name: "is_public",
            filename: None,
            content_type: None,
            bytes: b"false",
        },
    ]);
    let (forged_body, forged_ct) = multipart(&[
        Part {
            name: "image",
            filename: Some("shot.png"),
            content_type: Some("image/png"),
            bytes: &shot,
        },
        Part {
            name: "metadata",
            filename: None,
            content_type: Some("application/json"),
            bytes: photo_metadata(&owner.address(), &place)
                .to_string()
                .as_bytes(),
        },
    ]);
    let forged = node
        .signed(
            &stranger,
            "post",
            &format!("{REEL}/api/images"),
            Some((forged_body, &forged_ct)),
        )
        .await;
    assert!(
        matches!(forged.status, 400 | 401 | 403),
        "metadata.userAddress must be the signer: {} {}",
        forged.status,
        forged.body
    );
    let uploaded = node
        .signed(
            &owner,
            "post",
            &format!("{REEL}/api/images"),
            Some((body, &ct)),
        )
        .await;
    assert_eq!(uploaded.status, 200, "{}", uploaded.body);
    let image = &uploaded.body["image"];
    let image_id = image["id"].as_str().unwrap().to_string();
    let image_hash = hash_bytes_v1(&shot);
    assert_eq!(
        image["url"],
        format!("{}{REEL}/api/images/{image_hash}", node.url),
        "the url renders from the stored hash under CAMERA_REEL_API_URL"
    );
    assert_eq!(image["isPublic"], false);
    assert_eq!(image["metadata"]["placeId"], place);
    assert_eq!(uploaded.body["currentImages"], 1);
    assert_eq!(uploaded.body["maxImages"], 500);

    let bytes = node.get(&format!("{REEL}/api/images/{image_hash}")).await;
    assert_eq!(bytes.status, 200);
    assert_eq!(
        bytes.bytes, shot,
        "the photo is served from the ONE blob store"
    );
    assert_eq!(bytes.header("content-type"), "image/png");
    let thumb_url = image["thumbnailUrl"].as_str().unwrap().to_string();
    let thumb = node.get(&thumb_url[node.url.len()..]).await;
    assert_eq!(thumb.status, 200);
    let decoded = image::load_from_memory(&thumb.bytes).unwrap();
    assert_eq!(
        (decoded.width(), decoded.height()),
        (640, 360),
        "the generated thumbnail (never upscaled: the photo is 1280x720)"
    );

    let user_path = format!("{REEL}/api/users/{}", owner.address());
    let anon = node.get(&format!("{user_path}/images")).await;
    assert_eq!(anon.status, 200, "{}", anon.body);
    assert_eq!(
        anon.body["images"].as_array().map(Vec::len),
        Some(0),
        "a private photo is not listed to anyone"
    );
    assert_eq!(anon.body["currentImages"], 0);
    let mine = node
        .signed(
            &owner,
            "get",
            &format!("{user_path}/images?compact=true"),
            None,
        )
        .await;
    assert_eq!(mine.status, 200, "{}", mine.body);
    assert_eq!(
        mine.body["images"][0]["id"], image_id,
        "the owner lists the private photo"
    );
    assert_eq!(mine.body["currentImages"], 1);
    assert_eq!(
        node.signed(&stranger, "get", &format!("{user_path}/images"), None)
            .await
            .body["images"]
            .as_array()
            .map(Vec::len),
        Some(0)
    );
    let count = node.get(&user_path).await;
    assert_eq!(count.status, 200);
    assert_eq!(
        (
            count.body["currentImages"].as_u64(),
            count.body["maxImages"].as_u64()
        ),
        (Some(0), Some(500)),
        "the boot corpus row 34 shape"
    );
    let meta = node
        .get(&format!("{REEL}/api/images/{image_id}/metadata"))
        .await;
    assert_eq!(meta.status, 200);
    assert_eq!(
        meta.body["id"], image_id,
        "the metadata route answers the Image document"
    );
    assert_eq!(
        meta.body["metadata"]["userAddress"]
            .as_str()
            .map(str::to_lowercase)
            .as_deref(),
        Some(owner.address().as_str())
    );

    let vis_path = format!("{REEL}/api/images/{image_id}/visibility");
    let refused = node
        .signed_json(&stranger, "patch", &vis_path, json!({ "is_public": true }))
        .await;
    assert_eq!(
        refused.status, 403,
        "only the owner flips visibility: {}",
        refused.body
    );
    let flipped = node
        .signed_json(&owner, "patch", &vis_path, json!({ "is_public": true }))
        .await;
    assert_eq!(flipped.status, 200, "{}", flipped.body);
    let anon = node.get(&format!("{user_path}/images")).await;
    assert_eq!(
        anon.body["images"][0]["id"], image_id,
        "public now: listed to anyone"
    );
    assert_eq!(anon.body["images"][0]["isPublic"], true);
    let gallery = node.get(&format!("{REEL}/api/places/{place}/images")).await;
    assert_eq!(gallery.status, 200, "{}", gallery.body);
    assert_eq!(
        gallery.body["images"][0]["id"], image_id,
        "the place gallery (boot corpus row 2 shape)"
    );
    assert_eq!(gallery.body["maxImages"], 1);
    let batch = node
        .send(
            "post",
            &format!("{REEL}/api/places/images"),
            Vec::new(),
            Some((
                json!({ "placesIds": [place] }).to_string().into_bytes(),
                "application/json",
            )),
        )
        .await;
    assert_eq!(batch.status, 200, "{}", batch.body);
    assert_eq!(batch.body["images"][0]["placeId"], place);
    let keyset = node
        .get(&format!("{user_path}/images?limit=1&cursor="))
        .await;
    assert_eq!(keyset.status, 200, "{}", keyset.body);
    assert!(
        keyset.body.get("nextCursor").is_some(),
        "a keyset request carries nextCursor: {}",
        keyset.body
    );

    // The moderator: a second photo rejected disappears from every read.
    let second = png(640, 360, 5);
    let (body, ct) = multipart(&[
        Part {
            name: "image",
            filename: Some("shot.png"),
            content_type: Some("image/png"),
            bytes: &second,
        },
        Part {
            name: "metadata",
            filename: None,
            content_type: Some("application/json"),
            bytes: photo_metadata(&owner.address(), &place)
                .to_string()
                .as_bytes(),
        },
        Part {
            name: "is_public",
            filename: None,
            content_type: None,
            bytes: b"true",
        },
    ]);
    let uploaded = node
        .signed(
            &owner,
            "post",
            &format!("{REEL}/api/images"),
            Some((body, &ct)),
        )
        .await;
    assert_eq!(uploaded.status, 200, "{}", uploaded.body);
    let second_id = uploaded.body["image"]["id"].as_str().unwrap().to_string();
    let second_hash = hash_bytes_v1(&second);
    let review_path = format!("{REEL}/admin/images/{second_id}/review");
    assert_eq!(
        node.bearer(
            "wrong",
            "patch",
            &review_path,
            json!({ "reviewStatus": "rejected" })
        )
        .await
        .status,
        401
    );
    assert_eq!(
        node.send(
            "patch",
            &review_path,
            Vec::new(),
            Some((
                json!({ "reviewStatus": "rejected" })
                    .to_string()
                    .into_bytes(),
                "application/json"
            ))
        )
        .await
        .status,
        401
    );
    let rejected = node
        .bearer(
            &camera_token,
            "patch",
            &review_path,
            json!({ "reviewStatus": "rejected" }),
        )
        .await;
    assert_eq!(rejected.status, 200, "{}", rejected.body);
    assert_eq!(
        node.get(&format!("{REEL}/api/images/{second_id}/metadata"))
            .await
            .status,
        404,
        "a rejected row reads as absent"
    );
    assert_eq!(
        node.get(&format!("{REEL}/api/images/{second_hash}"))
            .await
            .status,
        404,
        "and its bytes are not served"
    );
    assert_eq!(
        node.get(&format!("/contents/{second_hash}")).await.status,
        404,
        "nor by the content server (camera_reel.withheld_blobs)"
    );
    assert_eq!(
        node.get(&format!("/worlds-content-server/contents/{second_hash}"))
            .await
            .status,
        404
    );
    assert_eq!(
        node.get(&format!("/builder-api/contents/{second_hash}"))
            .await
            .status,
        404,
        "nor by the builder"
    );
    assert_eq!(
        node.get(&format!("{user_path}/images")).await.body["images"]
            .as_array()
            .map(Vec::len),
        Some(1)
    );

    // Delete: the row goes, the hash stops being served (the blob stays in the shared store).
    let del_path = format!("{REEL}/api/images/{image_id}");
    assert_eq!(
        node.signed(&stranger, "delete", &del_path, None)
            .await
            .status,
        403
    );
    let deleted = node.signed(&owner, "delete", &del_path, None).await;
    assert_eq!(deleted.status, 200, "{}", deleted.body);
    assert_eq!(
        node.get(&format!("{REEL}/api/images/{image_hash}"))
            .await
            .status,
        404,
        "a hash no row names is not served"
    );
    assert_eq!(
        node.get(&format!("/contents/{image_hash}")).await.status,
        404,
        "a deleted photo's bytes are withheld on the content routes too"
    );
    assert_eq!(
        node.get(&format!("/builder-api/contents/{image_hash}"))
            .await
            .status,
        404
    );
    assert_eq!(node.get(&user_path).await.body["currentImages"], 0);
    assert_eq!(
        node.get(&format!("{REEL}/health/live")).await.body,
        json!("alive")
    );
}
